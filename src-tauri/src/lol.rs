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
//   claude  the game's clock
//   grok    the objectives' respawn countdowns, and her own team's summoner spells
//
// Looked for every 5 s (the game's process, by name: League of Legends.exe),
// read once a second in a game; done, all as it was. The Live Client's
// answers are cut to what is shown (view), kept here (GAME), and sent in each
// pet's update (main.rs pet_payload: game).
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

// A game read: when, and what of it the pets show.
#[derive(Clone, Default, PartialEq)]
pub struct Game {
    // The game's clock (s) as last read, and when (ms since 1970): the pages
    // run it on from there.
    pub time: f64,
    pub at: u64,
    pub objectives: Vec<Value>,
    pub allies: Vec<Value>,
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
    let Some(g) = game else { return Value::Null };
    let clock = json!({ "time": g.time, "at": g.at });
    match agent {
        "claude" => json!({ "role": "clock", "clock": clock }),
        "grok" => json!({ "role": "objectives", "clock": clock, "objectives": g.objectives, "allies": g.allies }),
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
            let spell = |key: &str| p["summonerSpells"][key]["displayName"].as_str().unwrap_or("").to_string();
            json!({
                "position": p["position"].as_str().unwrap_or(""),
                "champion": p["championName"].as_str().unwrap_or(""),
                "spells": [spell("summonerSpellOne"), spell("summonerSpellTwo")],
                "me": is_me(p, me),
            })
        })
        .collect()
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
fn read(agent: &ureq::Agent, base: &str, players: &mut Vec<Value>, me: &mut String, nth: u64) -> Option<Game> {
    let stats = get(agent, base, "gamestats")?;
    let events = get(agent, base, "eventdata")?;
    // The players change little: every 5th read (and the first).
    if nth % 5 == 0 || players.is_empty() {
        if let Some(list) = get(agent, base, "playerlist").and_then(|v| v.as_array().cloned()) {
            *players = list;
        }
        if let Some(active) = get(agent, base, "activeplayer") {
            *me = active["riotId"].as_str().or(active["summonerName"].as_str()).unwrap_or("").to_string();
        }
    }
    let events = events["Events"].as_array().cloned().unwrap_or_default();
    Some(Game { time: stats["gameTime"].as_f64().unwrap_or(0.0), at: now_ms(), objectives: objectives(&events), allies: allies(players, me) })
}

// Whether a program of this name runs (a snapshot of the processes: a few ms).
#[cfg(windows)]
fn running(exe: &str) -> bool {
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
            return false;
        }
        let mut e: Entry = std::mem::zeroed();
        e.size = std::mem::size_of::<Entry>() as u32;
        let mut ok = Process32FirstW(snap, &mut e) != 0;
        let mut found = false;
        while ok && !found {
            let len = e.name.iter().position(|c| *c == 0).unwrap_or(e.name.len());
            found = String::from_utf16_lossy(&e.name[..len]).to_lowercase() == exe;
            ok = Process32NextW(snap, &mut e) != 0;
        }
        CloseHandle(snap);
        found
    }
}

#[cfg(not(windows))]
fn running(_exe: &str) -> bool {
    false
}

fn set(sh: &Shared, game: Option<Game>) {
    let changed = {
        let mut kept = GAME.lock().unwrap_or_else(|e| e.into_inner());
        // The clock alone moving on: the pages run it themselves.
        let same = match (&*kept, &game) {
            (Some(a), Some(b)) => a.objectives == b.objectives && a.allies == b.allies && (b.time - a.time).abs() < 30.0,
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
        let (mut players, mut me, mut nth, mut misses) = (Vec::new(), String::new(), 0u64, 0u32);
        loop {
            if sh.setting("lol") == false || (fake.is_none() && !running(GAME_EXE)) {
                set(&sh, None);
                players.clear();
                misses = 0;
                std::thread::sleep(Duration::from_secs(5));
                continue;
            }
            match read(&agent, &base, &mut players, &mut me, nth) {
                Some(game) => {
                    misses = 0;
                    nth += 1;
                    set(&sh, Some(game));
                }
                // Still loading, or over: three in a row, no game.
                None => {
                    misses += 1;
                    if misses >= 3 {
                        set(&sh, None);
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
        assert_eq!(got[0], json!({ "position": "MIDDLE", "champion": "Ahri", "spells": ["Flash", "Ignite"], "me": true }));
        assert_eq!(got[1]["spells"], json!(["Flash", "Teleport"]));
        assert!(!got.iter().any(|p| p["champion"] == "Zed"));
    }
}
