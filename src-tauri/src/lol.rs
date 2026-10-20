// League of Legends in a game: what the pets out on the desktop show beside
// the agents' doings, read only through Riot's own ways in (the Live Client
// Data API on 127.0.0.1:2999 while a game runs). Never the game's memory, no
// injecting, nothing done for the player, no screen read.
//
// Riot's policy (developer.riotgames.com/docs/lol, Game Integrity): no
// information not in the game client that gives an edge, no app dictating
// the player's decisions. So: no enemy summoner spell or ultimate timers,
// and what is said is neutral ("Dragon respawns in 1:00", never "ward now").
//
// Each agent's pet a part (data.rs AGENTS):
//   claude  the next objective to spawn, one line (warned from a minute before)
//   grok    the objectives' respawn countdowns, and her own team's summoner
//           spells, by fixed short names (spell_short), whatever the client's language
//   codex   the other team's key items as they get them; and as she buys
//           something or dies, her own CS a minute, KDA and gold (her own
//           numbers only: Riot's approved "showing (self) player stats");
//           in champion select, out of a game, her rank and her party's (lcu.rs)
//   agy     every 5 minutes the two teams' kills, Dragons and towers so far
//           (the events, tally); while she is dead, the time to her respawn
//
// Looked for every 5 s (the game's process, by name: League of Legends.exe),
// read once a second in a game; done, all as it was. The Live Client's
// answers are cut to what is shown (view), kept here (GAME), and sent in each
// pet's update (main.rs pet_payload: game).
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use crate::{now_ms, Shared};

const GAME_EXE: &str = "league of legends.exe";
const LIVE: &str = "https://127.0.0.1:2999";

// The objectives' timers, game seconds (2026 season: Dragon at 5:00 and 5:00
// after each, the Elder 6:00 after one; Baron at 20:00 and 6:00 after; the
// Herald once, at 14:00). Not read from the game: checked against the
// season's notes, not in a game here; a new season may change them.
const DRAGON_FIRST: f64 = 300.0;
const DRAGON_AGAIN: f64 = 300.0;
const ELDER_AGAIN: f64 = 360.0;
const BARON_FIRST: f64 = 1200.0;
const BARON_AGAIN: f64 = 360.0;
const HERALD_FIRST: f64 = 840.0;

// Items worth a word when the other side gets one (Riot's item ids; their
// names come from the game, in its language): Quicksilver Sash, Mercurial
// Scimitar, Thornmail, Guardian Angel, Zhonya's Hourglass, Banshee's Veil,
// Edge of Night, Maw of Malmortius, Mortal Reminder, Morellonomicon.
const KEY_ITEMS: [u64; 10] = [3140, 3139, 3075, 3026, 3157, 3102, 3814, 3156, 3033, 3165];
// The last of these kept (the page shows each a while).
const NEWS_KEPT: usize = 8;

// A game read: when, and what of it the pets show.
#[derive(Clone, Default, PartialEq)]
pub struct Game {
    // The game's clock (s) as last read, and when (ms since 1970): the pages
    // run it on from there.
    pub time: f64,
    pub at: u64,
    pub objectives: Vec<Value>,
    pub allies: Vec<Value>,
    // The other team's key items as they came: { champion, item, at }.
    pub news: Vec<Value>,
    // Her own numbers as she last bought something or died (coach_of).
    pub coach: Option<Value>,
    // The two teams' kills, Dragons and towers so far (tally); when she
    // respawns (game seconds), while dead.
    pub tally: Value,
    pub respawn: Option<f64>,
}

static GAME: Mutex<Option<Game>> = Mutex::new(None);

// Whether a game is on (for full screen: fullscreen.rs lets the game's window be).
pub fn in_game() -> bool {
    GAME.lock().unwrap_or_else(|e| e.into_inner()).is_some()
}

// What an agent's pet shows of the game now; none out of a game, or for an
// agent with no part.
pub fn view(agent: &str) -> Value {
    let game = GAME.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let Some(g) = game else { return select_view(agent) };
    let clock = json!({ "time": g.time, "at": g.at });
    match agent {
        "claude" => json!({ "role": "clock", "clock": clock, "objectives": g.objectives }),
        "grok" => json!({ "role": "objectives", "clock": clock, "objectives": g.objectives, "allies": g.allies }),
        "codex" => json!({ "role": "intel", "clock": clock, "news": g.news, "coach": g.coach }),
        "agy" => json!({ "role": "summary", "clock": clock, "tally": g.tally, "respawn": g.respawn }),
        _ => Value::Null,
    }
}

// Out of a game, Codex's pet in champion select (lcu.rs).
fn select_view(agent: &str) -> Value {
    match (agent, crate::lcu::view()) {
        ("codex", Some(s)) => json!({ "role": "select", "players": s["players"], "keyless": s["keyless"] }),
        _ => Value::Null,
    }
}

// --- What the Live Client says, made what the pets show (pure: tested) --------------------

// Each objective's next spawn (game seconds) from the events so far: the
// last kill of each, or its first spawn; the Herald only until it is taken.
pub fn objectives(events: &[Value]) -> Vec<Value> {
    let last = |name: &str| events.iter().filter(|e| e["EventName"] == name).max_by(|a, b| time_of(a).total_cmp(&time_of(b)));
    let mut out = Vec::new();
    let dragon = last("DragonKill");
    let (next, elder) = match dragon {
        Some(e) if e["DragonType"] == "Elder" => (time_of(e) + ELDER_AGAIN, true),
        Some(e) => (time_of(e) + DRAGON_AGAIN, false),
        None => (DRAGON_FIRST, false),
    };
    out.push(json!({ "what": if elder { "elder" } else { "dragon" }, "at": next }));
    let baron = last("BaronKill").map_or(BARON_FIRST, |e| time_of(e) + BARON_AGAIN);
    out.push(json!({ "what": "baron", "at": baron }));
    if last("HeraldKill").is_none() {
        out.push(json!({ "what": "herald", "at": HERALD_FIRST }));
    }
    out
}

fn time_of(e: &Value) -> f64 {
    e["EventTime"].as_f64().unwrap_or(0.0)
}

// The player's own team in the game (playerlist, activeplayer: by Riot ID),
// each with their role and the two summoner spells they took: what the
// scoreboard shows, no cooldowns (the Live Client has none).
pub fn allies(players: &[Value], me: &str) -> Vec<Value> {
    let team = players.iter().find(|p| is_me(p, me)).and_then(|p| p["team"].as_str()).unwrap_or("ORDER");
    players
        .iter()
        .filter(|p| p["team"] == team)
        .map(|p| {
            let spell = |key: &str| spell_short(&p["summonerSpells"][key]);
            // No role (ARAM, a practice game): "NONE" or none at all.
            let position = p["position"].as_str().filter(|r| !r.is_empty() && *r != "NONE").unwrap_or("");
            json!({
                "position": position,
                "champion": p["championName"].as_str().unwrap_or(""),
                "spells": [spell("summonerSpellOne"), spell("summonerSpellTwo")],
                "me": is_me(p, me),
            })
        })
        .collect()
}

// A summoner spell by a short name of its own, whatever the client's
// language (its displayName is in it: Japanese on a Japanese client): by its
// rawDescription, which names it as the game does (GeneratedTip_SummonerSpell_
// SummonerFlash_Description). One not known: as the client names it.
pub fn spell_short(spell: &Value) -> String {
    const SHORT: [(&str, &str); 9] = [
        ("SummonerFlash", "閃"),
        ("SummonerTeleport", "傳"),
        ("SummonerDot", "點"),
        ("Smite", "懲"),
        ("SummonerHeal", "治"),
        ("SummonerBarrier", "盾"),
        ("SummonerExhaust", "弱"),
        ("SummonerBoost", "淨"),
        ("SummonerHaste", "疾"),
    ];
    let raw = spell["rawDescription"].as_str().unwrap_or("");
    SHORT.iter().find(|(key, _)| raw.contains(key)).map_or_else(|| spell["displayName"].as_str().unwrap_or("").to_string(), |(_, short)| short.to_string())
}

// The other team's key items new since the last look (seen: each player's
// items, by Riot ID, kept from look to look). A player seen the first time
// (a game joined late, she started late) is taken as they are: no news.
pub fn item_news(seen: &mut HashMap<String, HashSet<u64>>, players: &[Value], me: &str) -> Vec<Value> {
    let team = players.iter().find(|p| is_me(p, me)).and_then(|p| p["team"].as_str()).unwrap_or("ORDER").to_string();
    let mut news = Vec::new();
    for p in players.iter().filter(|p| p["team"] != team.as_str()) {
        let id = p["riotId"].as_str().or(p["summonerName"].as_str()).unwrap_or("").to_string();
        let items: Vec<&Value> = p["items"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
        let now: HashSet<u64> = items.iter().filter_map(|i| i["itemID"].as_u64()).collect();
        if let Some(before) = seen.get(&id) {
            for item in items.iter().filter(|i| i["itemID"].as_u64().is_some_and(|n| KEY_ITEMS.contains(&n) && !before.contains(&n))) {
                news.push(json!({ "champion": p["championName"].as_str().unwrap_or(""), "item": item["displayName"].as_str().unwrap_or("") }));
            }
        }
        seen.insert(id, now);
    }
    news
}

// Whether to show her own numbers now: she bought something (more of an
// item, or a new one, and her gold down with it), or she just died. Her items
// all changed with her gold as it was is no buying: a champion who takes
// another's form and items (Viego) or turns into another. Her items not
// known before (the first look): no.
pub fn coach_moment(before: Option<(&HashMap<u64, usize>, f64)>, items: &HashMap<u64, usize>, gold: f64, was_dead: bool, dead: bool) -> Option<&'static str> {
    if dead && !was_dead {
        return Some("dead");
    }
    let (had, gold_before) = before?;
    let more = items.iter().any(|(id, n)| *n > had.get(id).copied().unwrap_or(0));
    (more && gold < gold_before - 0.5).then_some("bought")
}

// Her items, each by how many she has.
fn counted(player: &Value) -> HashMap<u64, usize> {
    let mut items = HashMap::new();
    for id in player["items"].as_array().into_iter().flatten().filter_map(|i| i["itemID"].as_u64()) {
        *items.entry(id).or_insert(0) += 1;
    }
    items
}

// The two teams' kills, Dragons and towers from the events so far, hers
// ("us") and the other ("them"). A kill counted by whose champion died; a
// Dragon by whose player took it; a tower by whose it was (Turret_T1_...:
// the blue side's, ORDER's).
pub fn tally(events: &[Value], players: &[Value], me: &str) -> Value {
    let team_of = |name: &str| {
        players
            .iter()
            .find(|p| ["riotId", "summonerName", "riotIdGameName"].iter().any(|k| p[*k] == name))
            .and_then(|p| p["team"].as_str())
            .map(str::to_string)
    };
    let mine = players.iter().find(|p| is_me(p, me)).and_then(|p| p["team"].as_str()).unwrap_or("ORDER").to_string();
    let (mut us, mut them) = ([0u64; 3], [0u64; 3]);
    let mut count = |team: Option<String>, against: bool, i: usize| {
        if let Some(team) = team {
            let ours = (team == mine) != against;
            if ours { us[i] += 1 } else { them[i] += 1 }
        }
    };
    for e in events {
        let s = |k: &str| e[k].as_str().unwrap_or("").to_string();
        match e["EventName"].as_str().unwrap_or("") {
            "ChampionKill" => count(team_of(&s("VictimName")), true, 0),
            "DragonKill" => count(team_of(&s("KillerName")), false, 1),
            "TurretKilled" => {
                let t = s("TurretKilled");
                let owner = if t.contains("_T1_") { Some("ORDER".to_string()) } else if t.contains("_T2_") { Some("CHAOS".to_string()) } else { None };
                count(owner, true, 2);
            }
            _ => {}
        }
    }
    json!({ "us": { "kills": us[0], "dragons": us[1], "towers": us[2] }, "them": { "kills": them[0], "dragons": them[1], "towers": them[2] } })
}

// Her own numbers: CS and CS a minute, kills, deaths, assists, gold.
pub fn coach_of(me: &Value, gold: f64, time: f64, why: &str) -> Value {
    let s = &me["scores"];
    let cs = s["creepScore"].as_f64().unwrap_or(0.0);
    let per_min = if time >= 60.0 { (cs / (time / 60.0) * 10.0).round() / 10.0 } else { cs };
    json!({
        "why": why, "at": time, "cs": cs as u64, "perMin": per_min,
        "k": s["kills"].as_u64().unwrap_or(0), "d": s["deaths"].as_u64().unwrap_or(0), "a": s["assists"].as_u64().unwrap_or(0),
        "gold": gold.round() as u64,
    })
}

fn is_me(p: &Value, me: &str) -> bool {
    !me.is_empty() && (p["riotId"] == me || p["summonerName"] == me)
}

// --- Reading it ----------------------------------------------------------------------------

// The Live Client answers only on this computer, under a certificate of
// Riot's own, not one a TLS library can check against 127.0.0.1: not checked
// (nothing goes off this computer; one in the middle of it would be on it).
fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_millis(1500)))
        .tls_config(ureq::tls::TlsConfig::builder().disable_verification(true).build())
        .build()
        .new_agent()
}

fn get(agent: &ureq::Agent, base: &str, path: &str) -> Option<Value> {
    agent.get(&format!("{base}/liveclientdata/{path}")).call().ok()?.body_mut().read_json::<Value>().ok()
}

// One read of the game: its clock, its events, its players, who is playing.
// What is kept from one read to the next.
#[derive(Default)]
struct Reading {
    players: Vec<Value>,
    me: String,
    nth: u64,
    seen: HashMap<String, HashSet<u64>>,
    news: Vec<Value>,
    // Her own: gold, items (each by how many) and whether dead as last read,
    // her numbers shown last, when she respawns while dead.
    gold: f64,
    items: Option<(HashMap<u64, usize>, f64)>,
    dead: bool,
    coach: Option<Value>,
    respawn: Option<f64>,
}

fn read(agent: &ureq::Agent, base: &str, r: &mut Reading) -> Option<Game> {
    let nth = r.nth;
    let stats = get(agent, base, "gamestats")?;
    let events = get(agent, base, "eventdata")?;
    // The players change little: every 5th read (and the first).
    if nth % 5 == 0 || r.players.is_empty() {
        if let Some(list) = get(agent, base, "playerlist").and_then(|v| v.as_array().cloned()) {
            r.players = list;
        }
        if let Some(active) = get(agent, base, "activeplayer") {
            r.me = active["riotId"].as_str().or(active["summonerName"].as_str()).unwrap_or("").to_string();
            r.gold = active["currentGold"].as_f64().unwrap_or(r.gold);
        }
    }
    let events = events["Events"].as_array().cloned().unwrap_or_default();
    let time = stats["gameTime"].as_f64().unwrap_or(0.0);
    let (objectives, allies, tally) = (objectives(&events), allies(&r.players, &r.me), tally(&events, &r.players, &r.me));
    if nth % 5 == 0 {
        let fresh = item_news(&mut r.seen, &r.players, &r.me);
        for mut n in fresh {
            n["at"] = json!(time);
            r.news.push(n);
        }
        let over = r.news.len().saturating_sub(NEWS_KEPT);
        r.news.drain(..over);
        // Her own numbers, as she buys something or dies.
        if let Some(mine) = r.players.iter().find(|p| is_me(p, &r.me)).cloned() {
            let items = counted(&mine);
            let dead = mine["isDead"].as_bool().unwrap_or(false);
            if let Some(why) = coach_moment(r.items.as_ref().map(|(i, g)| (i, *g)), &items, r.gold, r.dead, dead) {
                r.coach = Some(coach_of(&mine, r.gold, time, why));
            }
            // While dead: when she is back (her respawn timer as read, on from now).
            r.respawn = dead.then(|| time + mine["respawnTimer"].as_f64().unwrap_or(0.0));
            r.items = Some((items, r.gold));
            r.dead = dead;
        }
    }
    Some(Game { time, at: now_ms(), objectives, allies, news: r.news.clone(), coach: r.coach.clone(), tally, respawn: r.respawn })
}

// Whether a program of this name runs (a snapshot of the processes: a few ms).
fn running(exe: &str) -> bool {
    pid_of(exe).is_some()
}

// The process of a program of this name (lower case), if one runs.
#[cfg(windows)]
pub fn pid_of(exe: &str) -> Option<u32> {
    #[repr(C)]
    struct Entry {
        size: u32,
        usage: u32,
        pid: u32,
        heap: usize,
        module: u32,
        threads: u32,
        parent: u32,
        priority: i32,
        flags: u32,
        name: [u16; 260],
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> isize;
        fn Process32FirstW(snap: isize, entry: *mut Entry) -> i32;
        fn Process32NextW(snap: isize, entry: *mut Entry) -> i32;
        fn CloseHandle(handle: isize) -> i32;
    }
    const TH32CS_SNAPPROCESS: u32 = 2;
    // SAFETY: our own entry, sized; the snapshot closed.
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == -1 {
            return None;
        }
        let mut e: Entry = std::mem::zeroed();
        e.size = std::mem::size_of::<Entry>() as u32;
        let mut ok = Process32FirstW(snap, &mut e) != 0;
        let mut found = None;
        while ok && found.is_none() {
            let len = e.name.iter().position(|c| *c == 0).unwrap_or(e.name.len());
            if String::from_utf16_lossy(&e.name[..len]).to_lowercase() == exe {
                found = Some(e.pid);
            }
            ok = Process32NextW(snap, &mut e) != 0;
        }
        CloseHandle(snap);
        found
    }
}

#[cfg(not(windows))]
pub fn pid_of(_exe: &str) -> Option<u32> {
    None
}

fn set(sh: &Shared, game: Option<Game>) {
    let changed = {
        let mut kept = GAME.lock().unwrap_or_else(|e| e.into_inner());
        // The clock alone moving on: the pages run it themselves.
        let same = match (&*kept, &game) {
            (Some(a), Some(b)) => a.objectives == b.objectives && a.allies == b.allies && a.news == b.news && a.coach == b.coach && a.tally == b.tally && a.respawn == b.respawn && (b.time - a.time).abs() < 30.0,
            (None, None) => true,
            _ => false,
        };
        let started = kept.is_none() && game.is_some();
        let ended = kept.is_some() && game.is_none();
        if !same {
            *kept = game;
        }
        if started {
            sh.log("lol: a game is on");
        }
        if ended {
            sh.log("lol: the game is over");
        }
        !same
    };
    if changed {
        crate::fullscreen::set_game(in_game());
        sh.redraw();
    }
}

// The watch: the game's process looked for every 5 s; in a game, read once a
// second. WAKUWAKU_LOL_URL (tests: a Live Client of our own, tools/lol_fake.py)
// stands for both: read from there, no process looked for.
pub fn start(sh: &Arc<Shared>) {
    let sh = sh.clone();
    crate::spawn_named("lol", move || {
        let fake = std::env::var("WAKUWAKU_LOL_URL").ok();
        let base = fake.clone().unwrap_or_else(|| LIVE.to_string());
        let agent = agent();
        let (mut reading, mut misses, mut looking) = (Reading::default(), 0u32, crate::lcu::Looking::default());
        loop {
            if sh.setting("lol") == false || (fake.is_none() && !running(GAME_EXE)) {
                set(&sh, None);
                reading = Reading::default();
                misses = 0;
                // No game: the client, in champion select (lcu.rs).
                if sh.setting("lol") != false {
                    crate::lcu::look(&sh, &mut looking);
                }
                std::thread::sleep(Duration::from_secs(5));
                continue;
            }
            // The game's ten looked up as it loads, once a game (scout.rs).
            crate::scout::on_game(&sh, pid_of(GAME_EXE));
            match read(&agent, &base, &mut reading) {
                Some(game) => {
                    // Loaded and under way: the panel of the ten goes.
                    if reading.nth == 0 {
                        crate::scout::on_game_started(&sh);
                    }
                    misses = 0;
                    reading.nth += 1;
                    set(&sh, Some(game));
                }
                // Still loading, or over: three in a row, no game (and a
                // next one read from the start).
                None => {
                    misses += 1;
                    if misses >= 3 {
                        set(&sh, None);
                        reading = Reading::default();
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    // Riot's sample of the Live Client's events (fixtures/lol/events.json,
    // static.developer.riotgames.com/docs/lol/liveclientdata_events.json):
    // its times all 0, given times here.
    fn sample() -> Vec<Value> {
        let all: Value = serde_json::from_str(include_str!("../fixtures/lol/events.json")).unwrap();
        all["Events"].as_array().unwrap().clone()
    }

    fn at(name: &str, time: f64) -> Value {
        let mut e = sample().into_iter().find(|e| e["EventName"] == name).unwrap();
        e["EventTime"] = json!(time);
        e
    }

    #[test]
    fn the_objectives_spawn_first_then_after_each_kill() {
        // Riot's sample read as it is: each event of a kind it knows.
        let names: Vec<String> = sample().iter().map(|e| e["EventName"].as_str().unwrap().to_string()).collect();
        assert!(names.contains(&"DragonKill".into()) && names.contains(&"BaronKill".into()) && names.contains(&"HeraldKill".into()));
        // Nothing killed yet: their first spawns.
        let none = objectives(&[]);
        assert_eq!(none, vec![json!({ "what": "dragon", "at": 300.0 }), json!({ "what": "baron", "at": 1200.0 }), json!({ "what": "herald", "at": 840.0 })]);
        // A Dragon at 6:10, another at 11:40: the next 5:00 after the last.
        let mut first = at("DragonKill", 370.0);
        first["DragonType"] = json!("Fire");
        let mut second = at("DragonKill", 700.0);
        second["DragonType"] = json!("Earth");
        let got = objectives(&[first, second, at("HeraldKill", 900.0), at("BaronKill", 1500.0)]);
        assert_eq!(got, vec![json!({ "what": "dragon", "at": 1000.0 }), json!({ "what": "baron", "at": 1860.0 })]);
        // The Elder: 6:00 after.
        let mut elder = at("DragonKill", 2000.0);
        elder["DragonType"] = json!("Elder");
        assert_eq!(objectives(&[elder])[0], json!({ "what": "elder", "at": 2360.0 }));
    }

    #[test]
    fn the_other_team_s_key_items_as_they_come() {
        let foe = |items: &[(u64, &str)]| {
            json!([
                { "riotId": "Me#TW1", "team": "ORDER", "championName": "Ahri", "items": [] },
                { "riotId": "Foe#KR1", "team": "CHAOS", "championName": "Zed",
                  "items": items.iter().map(|(id, name)| json!({ "itemID": id, "displayName": name })).collect::<Vec<_>>() },
                { "riotId": "Mate#TW1", "team": "ORDER", "championName": "Garen", "items": [{ "itemID": 3075, "displayName": "Thornmail" }] }
            ])
            .as_array()
            .unwrap()
            .clone()
        };
        let mut seen = HashMap::new();
        // The first look: as they are, no news (her own team's never).
        assert!(item_news(&mut seen, &foe(&[(3140, "Quicksilver Sash")]), "Me#TW1").is_empty());
        // A Zhonya's and a Long Sword since: the Zhonya's alone.
        let got = item_news(&mut seen, &foe(&[(3140, "Quicksilver Sash"), (3157, "Zhonya's Hourglass"), (1036, "Long Sword")]), "Me#TW1");
        assert_eq!(got, vec![json!({ "champion": "Zed", "item": "Zhonya's Hourglass" })]);
        // Looked at again, nothing new.
        assert!(item_news(&mut seen, &foe(&[(3140, "Quicksilver Sash"), (3157, "Zhonya's Hourglass")]), "Me#TW1").is_empty());
        // Sold and bought again: news again.
        item_news(&mut seen, &foe(&[]), "Me#TW1");
        assert_eq!(item_news(&mut seen, &foe(&[(3157, "Zhonya's Hourglass")]), "Me#TW1").len(), 1);
    }

    #[test]
    fn her_own_numbers_as_she_buys_or_dies() {
        let set = |ids: &[u64]| {
            let mut m = HashMap::new();
            for id in ids {
                *m.entry(*id).or_insert(0usize) += 1;
            }
            m
        };
        // The first look: nothing to compare with.
        assert_eq!(coach_moment(None, &set(&[1055]), 500.0, false, false), None);
        // Something new and her gold down: bought. Two of one where there was one: bought.
        assert_eq!(coach_moment(Some((&set(&[1055]), 900.0)), &set(&[1055, 1001]), 600.0, false, false), Some("bought"));
        assert_eq!(coach_moment(Some((&set(&[1036]), 900.0)), &set(&[1036, 1036]), 550.0, false, false), Some("bought"));
        // The same, or one sold (gold up): no.
        assert_eq!(coach_moment(Some((&set(&[1055, 1001]), 600.0)), &set(&[1055, 1001]), 620.0, false, false), None);
        assert_eq!(coach_moment(Some((&set(&[1055, 1001]), 600.0)), &set(&[1055]), 840.0, false, false), None);
        // Viego takes another's form: all his items another's, his gold as it was (on with the clock): no.
        assert_eq!(coach_moment(Some((&set(&[1055, 3006]), 700.0)), &set(&[3031, 3094, 3006]), 712.0, false, false), None);
        // Just died: dead; still dead, no more.
        assert_eq!(coach_moment(Some((&set(&[1055]), 600.0)), &set(&[1055]), 600.0, false, true), Some("dead"));
        assert_eq!(coach_moment(Some((&set(&[1055]), 600.0)), &set(&[1055]), 600.0, true, true), None);
        let me = json!({ "scores": { "creepScore": 87, "kills": 3, "deaths": 1, "assists": 4 } });
        // 87 CS at 12:00: 7.3 a minute.
        assert_eq!(coach_of(&me, 1234.6, 720.0, "bought"), json!({ "why": "bought", "at": 720.0, "cs": 87, "perMin": 7.3, "k": 3, "d": 1, "a": 4, "gold": 1235 }));
    }

    #[test]
    fn her_team_and_their_spells_not_the_other_s() {
        let player = |id: &str, team: &str, pos: &str, champ: &str, one: &str, two: &str| {
            json!({ "riotId": id, "summonerName": id, "team": team, "position": pos, "championName": champ,
                    "summonerSpells": { "summonerSpellOne": { "displayName": one }, "summonerSpellTwo": { "displayName": two } } })
        };
        let players = vec![
            player("Me#TW1", "CHAOS", "MIDDLE", "Ahri", "Flash", "Ignite"),
            player("Top#TW1", "CHAOS", "TOP", "Garen", "Flash", "Teleport"),
            player("Foe#TW1", "ORDER", "MIDDLE", "Zed", "Flash", "Ignite"),
        ];
        let got = allies(&players, "Me#TW1");
        assert_eq!(got.len(), 2);
        // No rawDescription: as the client names them.
        assert_eq!(got[0], json!({ "position": "MIDDLE", "champion": "Ahri", "spells": ["Flash", "Ignite"], "me": true }));
        assert_eq!(got[1]["spells"], json!(["Flash", "Teleport"]));
        assert!(!got.iter().any(|p| p["champion"] == "Zed"));
    }

    #[test]
    fn spells_by_short_names_whatever_the_client_s_language_and_no_none_role() {
        // A Japanese client: its names in Japanese, its rawDescription as the game names them.
        let jp = |name: &str, key: &str| json!({ "displayName": name, "rawDescription": format!("GeneratedTip_SummonerSpell_{key}_Description") });
        assert_eq!(spell_short(&jp("フラッシュ", "SummonerFlash")), "閃");
        assert_eq!(spell_short(&jp("テレポート", "S12_SummonerTeleportUpgrade")), "傳");
        assert_eq!(spell_short(&jp("スマイト", "S5_SummonerSmitePlayerGanker")), "懲");
        assert_eq!(spell_short(&jp("クレンズ", "SummonerBoost")), "淨");
        assert_eq!(spell_short(&jp("ゴースト", "SummonerHaste")), "疾");
        assert_eq!(spell_short(&jp("イグナイト", "SummonerDot")), "點");
        assert_eq!(spell_short(&jp("マーク", "SummonerSnowball")), "マーク");
        // No role (ARAM): none shown.
        let aram = json!([{ "riotId": "Me#JP1", "team": "ORDER", "position": "NONE", "championName": "Viego",
                            "summonerSpells": { "summonerSpellOne": jp("フラッシュ", "SummonerFlash"), "summonerSpellTwo": jp("マーク", "SummonerSnowball") } },
                          { "riotId": "Mate#JP1", "team": "ORDER", "position": "", "championName": "Lux",
                            "summonerSpells": { "summonerSpellOne": jp("フラッシュ", "SummonerFlash"), "summonerSpellTwo": jp("バリア", "SummonerBarrier") } }]);
        let got = allies(aram.as_array().unwrap(), "Me#JP1");
        assert_eq!((got[0]["position"].as_str(), got[1]["position"].as_str()), (Some(""), Some("")));
        assert_eq!(got[1]["spells"], json!(["閃", "盾"]));
    }

    #[test]
    fn the_two_teams_kills_dragons_and_towers() {
        let p = |id: &str, name: &str, team: &str| json!({ "riotId": id, "riotIdGameName": name, "summonerName": name, "team": team });
        let players = vec![p("Me#TW1", "Me", "ORDER"), p("Mate#TW1", "Mate", "ORDER"), p("Foe#KR1", "Foe", "CHAOS")];
        let kill = |victim: &str| { let mut e = at("ChampionKill", 100.0); e["VictimName"] = json!(victim); e };
        let mut ours = at("DragonKill", 400.0);
        ours["KillerName"] = json!("Mate");
        let mut tower = at("TurretKilled", 600.0);
        tower["TurretKilled"] = json!("Turret_T2_L_03_A");
        let mut lost = at("TurretKilled", 700.0);
        lost["TurretKilled"] = json!("Turret_T1_C_05_A");
        let events = vec![kill("Foe"), kill("Foe"), kill("Me"), ours, tower, lost.clone(), lost];
        assert_eq!(tally(&events, &players, "Me#TW1"), json!({ "us": { "kills": 2, "dragons": 1, "towers": 1 }, "them": { "kills": 1, "dragons": 0, "towers": 2 } }));
        // Seen from the other side: the other way round.
        assert_eq!(tally(&events, &players, "Foe#KR1")["us"], json!({ "kills": 1, "dragons": 0, "towers": 2 }));
    }
}
