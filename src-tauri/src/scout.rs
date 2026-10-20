// The game's ten players at a glance, as Porofessor shows them: as a game
// loads, each one's rank, this season's wins, their mastery of the champion
// they play and how their last games went, in a panel in the middle of the
// screen (src/scout/), shown as the game loads and on a key (scoutKey,
// Alt+X unless set) after, or a click on it puts it away.
//
// From Riot's API with the person's own key (riotApiKey): spectator-v5 for
// who is in the game (a game shows its players once it loads: no one hidden
// by champion select is found out), league-v4, champion-mastery-v4,
// match-v5. A personal key may ask 100 times in two minutes (20 in a
// second): every request waits its turn (limit), the quick ones (ranks,
// mastery) first, the last games after, a little at a time for each player;
// a game read once is kept on disk (lol/matches/) and never asked for again.
// Champion names from the client's own list (in its language).
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::utils::config::BackgroundThrottlingPolicy;
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::Shared;

pub const LABEL: &str = "scout";
// The panel, logical; kept inside the work area.
const SIZE: (f64, f64) = (1240.0, 800.0);
// Of each player's games, how many are read.
const GAMES: usize = 10;
// A personal key's limits, kept a little under.
const PER_2_MIN: usize = 95;
const PER_SECOND: usize = 18;
// Not Alt+`: Microsoft's Japanese IME takes it (half / full width), and
// pressed in a window with the IME on it never came; nor Alt+Z, NVIDIA's
// overlay's (2026-10-21).
const DEFAULT_KEY: &str = "Alt+X";

// What the page draws (view), and whether its window is up.
static VIEW: Mutex<Option<Value>> = Mutex::new(None);
static SHOWN: AtomicBool = AtomicBool::new(false);
static READY: AtomicBool = AtomicBool::new(false);
static CREATED: AtomicBool = AtomicBool::new(false);
static SCOUTING: AtomicBool = AtomicBool::new(false);
// The game process scouted last (its pid), so each game is looked up once.
static SCOUTED: Mutex<Option<u32>> = Mutex::new(None);

// --- Riot's API, its limits kept ----------------------------------------------------------------

static SENT: Mutex<VecDeque<Instant>> = Mutex::new(VecDeque::new());

// Waits until one more request is within the limits, and counts it.
fn wait_turn() {
    loop {
        let wait = {
            let mut sent = SENT.lock().unwrap_or_else(|e| e.into_inner());
            let now = Instant::now();
            while sent.front().is_some_and(|t| now.duration_since(*t) > Duration::from_secs(120)) {
                sent.pop_front();
            }
            let last_second = sent.iter().filter(|t| now.duration_since(**t) < Duration::from_secs(1)).count();
            if sent.len() >= PER_2_MIN {
                Some(Duration::from_secs(120).saturating_sub(now.duration_since(sent[0])) + Duration::from_millis(50))
            } else if last_second >= PER_SECOND {
                Some(Duration::from_millis(100))
            } else {
                sent.push_back(now);
                None
            }
        };
        match wait {
            Some(d) => std::thread::sleep(d),
            None => return,
        }
    }
}

// Why a request came to nothing.
#[derive(Debug, PartialEq)]
pub enum Miss {
    // 404: none (no game going on, no ranked games).
    None,
    // 401 / 403: the key is wrong or has run out (a development key lasts a day).
    Key,
    Other,
}

// One GET of Riot's API, in turn; a 429 waited out and asked again.
pub fn riot(key: &str, url: &str) -> Result<Value, Miss> {
    let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(10))).build().new_agent();
    for _ in 0..3 {
        wait_turn();
        match agent.get(url).header("X-Riot-Token", key).call() {
            Ok(mut r) => return r.body_mut().read_json::<Value>().map_err(|_| Miss::Other),
            Err(ureq::Error::StatusCode(404)) => return Err(Miss::None),
            Err(ureq::Error::StatusCode(401 | 403)) => return Err(Miss::Key),
            Err(ureq::Error::StatusCode(429)) => std::thread::sleep(Duration::from_secs(10)),
            Err(_) => return Err(Miss::Other),
        }
    }
    Err(Miss::Other)
}

// The regional host account-v1 lives on: Asia's for Asia and the Pacific.
pub fn account_region_of(platform: &str) -> &'static str {
    match region_of(platform) {
        "americas" => "americas",
        "europe" => "europe",
        _ => "asia",
    }
}

// The PUUID Riot's API knows a player by, from their Riot ID (name#tag):
// the client's own is another (Riot's API answers it "Exception
// decrypting", 2026-10-20). Kept while she runs.
pub fn puuid_of(key: &str, platform: &str, riot_id: &str) -> Option<String> {
    static KNOWN: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);
    let (name, tag) = riot_id.rsplit_once('#').filter(|(n, t)| !n.is_empty() && !t.is_empty())?;
    if let Some(p) = KNOWN.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|k| k.get(riot_id)) {
        return Some(p.clone());
    }
    let base = std::env::var("WAKUWAKU_RIOT_API_URL").unwrap_or_else(|_| format!("https://{}.api.riotgames.com", account_region_of(platform)));
    let enc = |s: &str| s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>();
    let found = riot(key, &format!("{base}/riot/account/v1/accounts/by-riot-id/{}/{}", enc(name), enc(tag))).ok()?;
    let puuid = found["puuid"].as_str()?.to_string();
    KNOWN.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(riot_id.to_string(), puuid.clone());
    Some(puuid)
}

// The regional host match-v5 lives on, for a platform.
pub fn region_of(platform: &str) -> &'static str {
    match platform {
        "na1" | "br1" | "la1" | "la2" => "americas",
        "kr" | "jp1" => "asia",
        "euw1" | "eun1" | "tr1" | "ru" | "me1" => "europe",
        _ => "sea",
    }
}

// Riot's hosts; WAKUWAKU_RIOT_API_URL (tests: tools/lol_fake.py) stands for both.
fn hosts(platform: &str) -> (String, String) {
    match std::env::var("WAKUWAKU_RIOT_API_URL") {
        Ok(url) => (url.clone(), url),
        Err(_) => (format!("https://{platform}.api.riotgames.com"), format!("https://{}.api.riotgames.com", region_of(platform))),
    }
}

// --- What is made of what Riot says (pure: tested) ------------------------------------------------

// What a kept game holds; one kept with less (an older version) is read again.
const KEPT: u64 = 2;

// A game as kept: who played what, where, and how it went for each (their
// kills, deaths, assists, minions, items, spells, keystone and second rune
// tree, their part in their team's kills). Remakes (under five minutes) are none.
pub fn trim_match(m: &Value) -> Option<Value> {
    let info = &m["info"];
    if info["gameDuration"].as_f64().unwrap_or(0.0) < 300.0 {
        return None;
    }
    let kills = |team: &Value| info["teams"].as_array().into_iter().flatten().find(|t| t["teamId"] == *team).and_then(|t| t["objectives"]["champion"]["kills"].as_f64()).unwrap_or(0.0);
    let players: Vec<Value> = info["participants"]
        .as_array()?
        .iter()
        .map(|p| {
            let n = |k: &str| p[k].as_u64().unwrap_or(0);
            let team_kills = kills(&p["teamId"]);
            let kp = p["challenges"]["killParticipation"].as_f64().unwrap_or_else(|| if team_kills > 0.0 { (n("kills") + n("assists")) as f64 / team_kills } else { 0.0 });
            json!({
                "puuid": p["puuid"], "champion": p["championId"], "team": p["teamId"], "pos": p["teamPosition"], "win": p["win"],
                "k": p["kills"], "d": p["deaths"], "a": p["assists"], "cs": n("totalMinionsKilled") + n("neutralMinionsKilled"),
                "items": (0..7).map(|i| n(&format!("item{i}"))).collect::<Vec<_>>(),
                "spells": [n("summoner1Id"), n("summoner2Id")],
                "keystone": p["perks"]["styles"][0]["selections"][0]["perk"], "sub": p["perks"]["styles"][1]["style"],
                "kp": (kp * 100.0).round(), "level": p["champLevel"],
            })
        })
        .collect();
    Some(json!({ "v": KEPT, "id": m["metadata"]["matchId"], "queue": info["queueId"], "end": info["gameEndTimestamp"], "duration": info["gameDuration"], "players": players }))
}

// How one player's last games went: how many, won, their kills, deaths and
// assists a game, on this champion how many and won, and the streak they are
// on (+3 three won in a row, -2 two lost). games: as kept, the latest first.
pub fn recent_of(puuid: &str, champion: u64, games: &[Value]) -> Value {
    let both: Vec<(&Value, &Value)> = games.iter().filter_map(|g| Some((g, g["players"].as_array()?.iter().find(|p| p["puuid"] == puuid)?))).collect();
    let mine: Vec<&Value> = both.iter().map(|(_, p)| *p).collect();
    if mine.is_empty() {
        return Value::Null;
    }
    let n = mine.len() as f64;
    let won = |p: &Value| p["win"].as_bool().unwrap_or(false);
    let sum = |k: &str| mine.iter().map(|p| p[k].as_f64().unwrap_or(0.0)).sum::<f64>();
    let on: Vec<&&Value> = mine.iter().filter(|p| p["champion"].as_u64() == Some(champion)).collect();
    let first = won(mine[0]);
    let run = mine.iter().take_while(|p| won(p) == first).count() as i64;
    let round = |x: f64| (x * 10.0).round() / 10.0;
    json!({
        "games": mine.len(),
        "wins": mine.iter().filter(|p| won(p)).count(),
        "k": round(sum("k") / n), "d": round(sum("d") / n), "a": round(sum("a") / n),
        "champGames": on.len(),
        "champWins": on.iter().filter(|p| won(p)).count(),
        "streak": if first { run } else { -run },
        // Each game, the latest first: for her own panel (her alone): hers,
        // the game's, and the ten's champions by team.
        "list": both.iter().map(|(g, p)| {
            let mut row = (*p).clone();
            row.as_object_mut().map(|o| o.remove("puuid"));
            row["win"] = json!(won(p));
            for k in ["queue", "end", "duration"] {
                row[k] = g[k].clone();
            }
            let side = |team: u64| g["players"].as_array().into_iter().flatten().filter(|q| q["team"].as_u64() == Some(team)).map(|q| q["champion"].clone()).collect::<Vec<_>>();
            row["blue"] = json!(side(100));
            row["red"] = json!(side(200));
            row
        }).collect::<Vec<_>>(),
    })
}

// Where she plays in ranked Solo/Duo (420), by the games kept: each
// position, how many and won, the most played first.
pub fn roles_of(puuid: &str, games: &[Value]) -> Value {
    let mut by: Vec<(String, u64, u64)> = Vec::new();
    for g in games.iter().filter(|g| g["queue"] == 420) {
        let Some(p) = g["players"].as_array().and_then(|ps| ps.iter().find(|p| p["puuid"] == puuid)) else { continue };
        let pos = p["pos"].as_str().filter(|s| !s.is_empty()).unwrap_or("NONE").to_string();
        let won = p["win"].as_bool().unwrap_or(false) as u64;
        match by.iter_mut().find(|(r, _, _)| *r == pos) {
            Some(row) => {
                row.1 += 1;
                row.2 += won;
            }
            None => by.push((pos, 1, won)),
        }
    }
    by.sort_by(|a, b| b.1.cmp(&a.1));
    Value::Array(by.into_iter().map(|(pos, games, wins)| json!({ "pos": pos, "games": games, "wins": wins })).collect())
}

// One queue's entry of league-v4's: tier, rank, points, won and lost; none, unranked.
pub fn entry_of(entries: &Value, queue: &str) -> Value {
    entries.as_array().into_iter().flatten().find(|e| e["queueType"] == queue).map_or(Value::Null, |e| {
        json!({ "tier": e["tier"], "rank": e["rank"], "lp": e["leaguePoints"], "wins": e["wins"], "losses": e["losses"], "hot": e["hotStreak"], "veteran": e["veteran"] })
    })
}

// The key's name ("Alt+`", "Ctrl+Shift+F9") as RegisterHotKey wants it: modifiers and a virtual key.
pub fn parse_key(name: &str) -> Option<(u32, u32)> {
    let mut mods = 0;
    let mut vk = None;
    for part in name.split('+').map(str::trim) {
        match part.to_ascii_lowercase().as_str() {
            "alt" => mods |= 1,
            "ctrl" | "control" => mods |= 2,
            "shift" => mods |= 4,
            "win" => mods |= 8,
            "`" | "~" => vk = Some(0xC0),
            "tab" => vk = Some(0x09),
            "space" => vk = Some(0x20),
            k if k.len() == 1 && k.chars().all(|c| c.is_ascii_alphanumeric()) => vk = Some(k.to_ascii_uppercase().as_bytes()[0] as u32),
            k if k.starts_with('f') && k[1..].parse::<u32>().is_ok_and(|n| (1..=12).contains(&n)) => vk = Some(0x6F + k[1..].parse::<u32>().unwrap()),
            _ => return None,
        }
    }
    Some((mods, vk?))
}

// --- Looking the game up ----------------------------------------------------------------------

struct Player {
    puuid: String,
    name: String,
    team: u64,
    champion: u64,
    me: bool,
    rank: Value,
    mastery: Value,
    recent: Value,
    ids: Vec<String>,
}

fn player_view(p: &Player, names: &HashMap<u64, String>) -> Value {
    json!({
        "name": p.name, "me": p.me, "championId": p.champion,
        "champion": names.get(&p.champion).cloned().unwrap_or_default(),
        "rank": p.rank, "mastery": p.mastery, "recent": p.recent,
    })
}

fn publish(sh: &Shared, view: Value) {
    *VIEW.lock().unwrap_or_else(|e| e.into_inner()) = Some(view);
    push(sh);
}

fn status(sh: &Shared, state: &str) {
    publish(sh, json!({ "state": state, "key": key_name(sh) }));
}

fn key_name(sh: &Shared) -> String {
    sh.setting("scoutKey").as_str().filter(|k| parse_key(k).is_some()).unwrap_or(DEFAULT_KEY).to_string()
}

fn matches_dir(sh: &Shared) -> PathBuf {
    sh.dir.join("lol").join("matches")
}

// A game, kept on disk or read once from Riot and kept.
fn game(sh: &Shared, key: &str, regional: &str, id: &str) -> Option<Value> {
    let path = matches_dir(sh).join(format!("{}.json", id.replace(['/', '\\', '.'], "_")));
    if let Some(kept) = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).filter(|k| k["v"].as_u64() == Some(KEPT)) {
        return Some(kept);
    }
    let whole = riot(key, &format!("{regional}/lol/match/v5/matches/{id}")).ok()?;
    let kept = trim_match(&whole).unwrap_or_else(|| json!({ "v": KEPT, "id": id, "remake": true, "players": [] }));
    let _ = std::fs::create_dir_all(matches_dir(sh));
    let _ = std::fs::write(&path, kept.to_string());
    Some(kept)
}

// As a game loads: the ten looked up, a little at a time, the panel shown.
pub fn on_game(sh: &Arc<Shared>, pid: Option<u32>) {
    {
        let mut scouted = SCOUTED.lock().unwrap_or_else(|e| e.into_inner());
        // A fake game (tests) has no process: looked up once.
        let this = pid.unwrap_or(0);
        if *scouted == Some(this) || SCOUTING.load(Ordering::SeqCst) {
            return;
        }
        *scouted = Some(this);
    }
    SCOUTING.store(true, Ordering::SeqCst);
    let sh = sh.clone();
    crate::spawn_named("scout", move || {
        scout(&sh, true);
        SCOUTING.store(false, Ordering::SeqCst);
    });
}

// The game's ten (as a game loads: in_game), or, where there are none to
// find (a practice tool or custom game, or no game at all), her alone.
fn scout(sh: &Arc<Shared>, in_game: bool) {
    let key = sh.setting("riotApiKey").as_str().unwrap_or("").to_string();
    // The icons' names and the champions' in her language, once (Data Dragon).
    load_ddragon(&sh.lang());
    // Shown by itself only while the game loads: one already under way (she
    // started in the middle of it) is not covered over, the key brings it.
    // Told after the game's next look (lol.rs reads it each second), not before.
    if in_game {
        std::thread::sleep(Duration::from_millis(1500));
    }
    let loading = in_game && !crate::lol::in_game();
    if key.is_empty() {
        status(sh, "nokey");
        if loading {
            show(sh, true);
        }
        return;
    }
    status(sh, "looking");
    if loading {
        show(sh, true);
    }
    // Tests: someone's game on Taiwan's server looked up as if hers
    // (WAKUWAKU_SCOUT_AS=<their PUUID>), no client needed.
    let as_someone = std::env::var("WAKUWAKU_SCOUT_AS").ok().map(|puuid| (puuid, "tw2", HashMap::new(), String::new()));
    let Some((me, platform, names, my_name)) = as_someone.or_else(|| {
        let (riot_id, platform, names) = crate::lcu::me_platform_names()?;
        Some((puuid_of(&key, platform, &riot_id)?, platform, names, riot_id))
    }) else {
        sh.log("scout: the client is not there to say who she is, or Riot's API does not know her");
        status(sh, "noclient");
        return;
    };
    let (host, regional) = hosts(platform);
    // Spectator has the game a little after it starts loading.
    let mut live = Err(Miss::None);
    for _ in 0..if in_game { 12 } else { 0 } {
        live = riot(&key, &format!("{host}/lol/spectator/v5/active-games/by-summoner/{me}"));
        if live.is_ok() || live == Err(Miss::Key) {
            break;
        }
        std::thread::sleep(Duration::from_secs(5));
    }
    let (live, solo) = match live {
        Ok(v) => (v, ""),
        Err(Miss::Key) => return status(sh, "badkey"),
        // None to find: her own (profile).
        Err(_) => return profile(sh, &key, &host, &regional, &me, &my_name, &names),
    };
    let mut players: Vec<Player> = live["participants"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| {
            Some(Player {
                puuid: p["puuid"].as_str().filter(|s| !s.is_empty())?.to_string(),
                name: p["riotId"].as_str().unwrap_or("").to_string(),
                team: p["teamId"].as_u64().unwrap_or(100),
                champion: p["championId"].as_u64().unwrap_or(0),
                me: p["puuid"] == me.as_str(),
                rank: Value::Null,
                mastery: Value::Null,
                recent: Value::Null,
                ids: Vec::new(),
            })
        })
        .collect();
    let my_team = players.iter().find(|p| p.me).map_or(100, |p| p.team);
    let queue = live["gameQueueConfigId"].clone();
    sh.log(&format!("scout: {} players, queue {queue}", players.len()));
    let draw = |players: &[Player], done: usize, total: usize| {
        let team = |mine: bool| players.iter().filter(|p| (p.team == my_team) == mine).map(|p| player_view(p, &names)).collect::<Vec<_>>();
        // Her alone: the names of the champions of her games too.
        let played: HashMap<String, String> = if solo.is_empty() {
            HashMap::new()
        } else {
            players.iter().flat_map(|p| p.recent["list"].as_array().cloned().unwrap_or_default()).filter_map(|g| g["champion"].as_u64()).filter_map(|id| Some((id.to_string(), names.get(&id)?.clone()))).collect()
        };
        publish(sh, json!({ "state": "ready", "key": key_name(sh), "queue": queue, "us": team(true), "them": team(false), "done": done, "total": total, "solo": solo, "names": played }));
    };
    draw(&players, 0, 1);
    // The quick ones first: ranks and mastery.
    for i in 0..players.len() {
        let puuid = players[i].puuid.clone();
        if let Ok(entries) = riot(&key, &format!("{host}/lol/league/v4/entries/by-puuid/{puuid}")) {
            players[i].rank = crate::lcu::rank_of(&entries);
            if players[i].rank.is_null() {
                players[i].rank = json!({ "unranked": true });
            }
        }
        let champion = players[i].champion;
        if champion == 0 {
            // No game, no champion: no mastery to tell.
        } else if let Ok(m) = riot(&key, &format!("{host}/lol/champion-mastery/v4/champion-masteries/by-puuid/{puuid}/by-champion/{champion}")) {
            players[i].mastery = json!({ "level": m["championLevel"], "points": m["championPoints"] });
        } else {
            players[i].mastery = json!({ "level": 0, "points": 0 });
        }
        draw(&players, 0, 1);
    }
    // Their last games: the list, then each game, a round at a time so all
    // ten have a few before anyone has them all.
    for p in players.iter_mut() {
        if let Ok(Value::Array(ids)) = riot(&key, &format!("{regional}/lol/match/v5/matches/by-puuid/{}/ids?start=0&count={GAMES}", p.puuid)) {
            p.ids = ids.iter().filter_map(|i| i.as_str().map(str::to_string)).collect();
        }
    }
    let total: usize = players.iter().map(|p| p.ids.len()).sum();
    let mut kept: HashMap<String, Value> = HashMap::new();
    let mut done = 0;
    for round in 0..GAMES {
        for i in 0..players.len() {
            let Some(id) = players[i].ids.get(round).cloned() else { continue };
            if !kept.contains_key(&id) {
                if let Some(g) = game(sh, &key, &regional, &id) {
                    kept.insert(id.clone(), g);
                }
            }
            done += 1;
            let games: Vec<Value> = players[i].ids.iter().filter_map(|id| kept.get(id).cloned()).collect();
            players[i].recent = recent_of(&players[i].puuid.clone(), players[i].champion, &games);
        }
        draw(&players, done, total);
    }
    draw(&players, total, total);
    sh.log(&format!("scout: done, {} games read", kept.len()));
}

// Her own, where the game's ten cannot be found (a practice tool, a custom
// game) or out of a game, as Porofessor's profile: her icon and level, both
// ranked queues, her three most played champions by mastery, her last ten
// games whole, and where she plays in Solo/Duo (its last 100 games, read on
// in the background, kept on disk: the first look takes a couple of minutes,
// the next only the new ones). Each part told by one of the agents' pets
// (src/scout/).
fn profile(sh: &Arc<Shared>, key: &str, host: &str, regional: &str, me: &str, my_name: &str, names: &HashMap<u64, String>) {
    let why = if crate::lol::in_game() { "nogame" } else { "outside" };
    let summoner = riot(key, &format!("{host}/lol/summoner/v4/summoners/by-puuid/{me}")).unwrap_or(Value::Null);
    let entries = riot(key, &format!("{host}/lol/league/v4/entries/by-puuid/{me}")).unwrap_or(json!([]));
    let top: Vec<Value> = riot(key, &format!("{host}/lol/champion-mastery/v4/champion-masteries/by-puuid/{me}/top?count=3"))
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .map(|m| json!({ "champion": m["championId"], "level": m["championLevel"], "points": m["championPoints"] }))
        .collect();
    // In a game: the champion she plays (the game's own list, by name in the
    // client's language), and her mastery of it.
    let champion = crate::lol::my_champion().and_then(|name| names.iter().find(|(_, n)| **n == name).map(|(id, _)| *id));
    let now = champion.map(|c| {
        let m = riot(key, &format!("{host}/lol/champion-mastery/v4/champion-masteries/by-puuid/{me}/by-champion/{c}")).ok();
        json!({ "champion": c, "level": m.as_ref().map_or(json!(0), |m| m["championLevel"].clone()), "points": m.as_ref().map_or(json!(0), |m| m["championPoints"].clone()) })
    });
    let mut view = json!({
        "state": "ready", "solo": why, "key": key_name(sh), "names": names_json(names),
        "profile": {
            "name": my_name, "icon": summoner["profileIconId"], "level": summoner["summonerLevel"],
            "solo": entry_of(&entries, "RANKED_SOLO_5x5"), "flex": entry_of(&entries, "RANKED_FLEX_SR"),
            "top": top, "now": now, "recent": null, "roles": [], "rolesOf": 0, "rolesTotal": 0,
        },
    });
    publish(sh, view.clone());
    let ids = |query: &str| -> Vec<String> {
        match riot(key, &format!("{regional}/lol/match/v5/matches/by-puuid/{me}/ids?start=0{query}")) {
            Ok(Value::Array(ids)) => ids.iter().filter_map(|i| i.as_str().map(str::to_string)).collect(),
            _ => Vec::new(),
        }
    };
    let mut games = Vec::new();
    for id in ids(&format!("&count={GAMES}")) {
        if let Some(g) = game(sh, key, regional, &id) {
            games.push(g);
        }
        view["profile"]["recent"] = recent_of(me, champion.unwrap_or(0), &games);
        publish(sh, view.clone());
    }
    let solo_ids = ids("&queue=420&count=100");
    view["profile"]["rolesTotal"] = json!(solo_ids.len());
    let mut solo_games = Vec::new();
    for (i, id) in solo_ids.iter().enumerate() {
        // A game begun meanwhile: its ten first.
        if why == "outside" && crate::lol::in_game() {
            break;
        }
        if let Some(g) = game(sh, key, regional, id) {
            solo_games.push(g);
        }
        if i % 10 == 9 || i + 1 == solo_ids.len() {
            view["profile"]["roles"] = roles_of(me, &solo_games);
            view["profile"]["rolesOf"] = json!(i + 1);
            publish(sh, view.clone());
        }
    }
    sh.log(&format!("scout: her own, {} games and {} of Solo/Duo read", games.len(), solo_games.len()));
}

fn names_json(names: &HashMap<u64, String>) -> Value {
    Value::Object(names.iter().map(|(id, n)| (id.to_string(), json!(n))).collect())
}

// The icons the page draws with, by Riot's Data Dragon: its latest version,
// the summoner spells' names by their ids, the runes' and rune trees' icon
// paths by theirs, and the champions' names in her language: Taiwan's in
// Traditional Chinese, whatever the client's (a Japanese client named them
// in Japanese, 2026-10-21). Read once a run and language (load_ddragon, on
// the scouting thread); none until then, or offline: the page draws
// without them, the champions named as the client names them.
static DDRAGON: Mutex<Option<Value>> = Mutex::new(None);

fn ddragon_locale(lang: &str) -> &'static str {
    match lang {
        "zh-TW" => "zh_TW",
        "zh" => "zh_CN",
        _ => "en_US",
    }
}

fn load_ddragon(lang: &str) {
    let locale = ddragon_locale(lang);
    if DDRAGON.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|d| d["locale"] == locale) {
        return;
    }
    let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(10))).build().new_agent();
    let get = |url: &str| agent.get(url).call().ok()?.body_mut().read_json::<Value>().ok();
    let base = "https://ddragon.leagueoflegends.com";
    let Some(v) = get(&format!("{base}/api/versions.json")).and_then(|v| v[0].as_str().map(str::to_string)) else { return };
    let spells: serde_json::Map<String, Value> = get(&format!("{base}/cdn/{v}/data/en_US/summoner.json"))
        .and_then(|s| s["data"].as_object().cloned())
        .unwrap_or_default()
        .values()
        .filter_map(|s| Some((s["key"].as_str()?.to_string(), json!(s["id"].as_str()?))))
        .collect();
    let mut perks = serde_json::Map::new();
    for style in get(&format!("{base}/cdn/{v}/data/en_US/runesReforged.json")).and_then(|r| r.as_array().cloned()).unwrap_or_default() {
        perks.insert(style["id"].to_string(), style["icon"].clone());
        for rune in style["slots"].as_array().into_iter().flatten().flat_map(|s| s["runes"].as_array().cloned().unwrap_or_default()) {
            perks.insert(rune["id"].to_string(), rune["icon"].clone());
        }
    }
    let champions: serde_json::Map<String, Value> = get(&format!("{base}/cdn/{v}/data/{locale}/champion.json"))
        .and_then(|c| c["data"].as_object().cloned())
        .unwrap_or_default()
        .values()
        .filter_map(|c| Some((c["key"].as_str()?.to_string(), json!(c["name"].as_str()?))))
        .collect();
    *DDRAGON.lock().unwrap_or_else(|e| e.into_inner()) = Some(json!({ "v": v, "locale": locale, "spells": spells, "perks": perks, "champions": champions }));
}

// Each agent's pet, drawn beside the part it tells: its sheet and colour.
fn pets_of(sh: &Shared) -> Value {
    let settings = sh.settings.lock().unwrap().clone();
    let found = crate::data::pets(&sh.dir);
    Value::Object(
        crate::data::AGENTS
            .iter()
            .map(|&agent| {
                let pet = crate::data::pet_of(&settings, Some(agent), &found);
                let view = json!({
                    "sprite": pet.map(|p| crate::data::asset_url(&p.sheet)),
                    "version": pet.map_or(2, |p| p.version),
                    "color": pet.and_then(|p| p.color.clone()).unwrap_or_else(|| crate::data::agent_color(agent).to_string()),
                });
                (agent.to_string(), view)
            })
            .collect(),
    )
}

// --- Its window ---------------------------------------------------------------------------------

fn window(sh: &Shared) -> Option<WebviewWindow> {
    sh.app.get_webview_window(LABEL)
}

// The middle of the display the island is on.
fn rect(sh: &Shared) -> Option<((i32, i32), (i32, i32), f64)> {
    let a = crate::island::area(sh)?;
    let w = ((SIZE.0 * a.sf).round() as i32).min(a.w);
    let h = ((SIZE.1 * a.sf).round() as i32).min(a.h);
    Some(((a.x + (a.w - w) / 2, a.y + (a.h - h) / 2), (w, h), a.sf))
}

fn lay(win: &WebviewWindow, (x, y): (i32, i32), (w, h): (i32, i32)) {
    let _ = win.set_position(PhysicalPosition::new(x, y));
    let _ = win.set_size(PhysicalSize::new(w.max(1) as u32, h.max(1) as u32));
    let _ = win.set_position(PhysicalPosition::new(x, y));
}

fn create(sh: &Arc<Shared>) {
    if CREATED.swap(true, Ordering::SeqCst) {
        return;
    }
    let Some((pos, size, sf)) = rect(sh) else { return };
    let sh = sh.clone();
    std::thread::spawn(move || {
        let built = WebviewWindowBuilder::new(&sh.app, LABEL, WebviewUrl::App("scout/index.html".into()))
            .title("wakuwaku scout")
            .inner_size(size.0 as f64 / sf, size.1 as f64 / sf)
            .additional_browser_args(crate::browser_args(&sh))
            .data_directory(crate::webview_dir(&sh))
            .transparent(true)
            .decorations(false)
            .resizable(false)
            .maximizable(false)
            .minimizable(false)
            .skip_taskbar(true)
            .shadow(false)
            .always_on_top(true)
            .focusable(false)
            .focused(false)
            .visible(false)
            .background_throttling(BackgroundThrottlingPolicy::Disabled)
            .build();
        match built {
            Ok(win) => {
                lay(&win, pos, size);
                // A click on it puts it away (scout_close): it is up only
                // when asked for, and a key may be taken by something else.
                // Said outright, as the island's and the dock's are: left as
                // made, the clicks never reached its page (2026-10-21).
                let _ = win.set_ignore_cursor_events(false);
                sh.own_window(&win);
            }
            Err(err) => sh.log(&format!("scout: could not make its window: {err}")),
        }
    });
}

// Its page is up: drawn, and shown if it was asked for meanwhile.
pub fn ready(sh: &Arc<Shared>) {
    READY.store(true, Ordering::SeqCst);
    sh.log("scout: page ready");
    push(sh);
    apply(sh);
}

fn push(sh: &Shared) {
    if READY.load(Ordering::SeqCst) {
        let view = VIEW.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or(json!({ "state": "none" }));
        let mut view = view;
        view["lang"] = json!(sh.lang());
        view["key"] = json!(key_name(sh));
        view["dd"] = DDRAGON.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or(Value::Null);
        view["pets"] = pets_of(sh);
        let _ = sh.app.emit_to(LABEL, "scout:update", view);
    }
}

fn apply(sh: &Arc<Shared>) {
    let want = SHOWN.load(Ordering::SeqCst);
    if want && !CREATED.load(Ordering::SeqCst) {
        create(sh);
        return;
    }
    if !READY.load(Ordering::SeqCst) {
        return;
    }
    if let Some(win) = window(sh) {
        if want {
            if let Some((pos, size, _)) = rect(sh) {
                lay(&win, pos, size);
                // Onto a display of another scale, Windows sizes it again
                // for that scale a moment later (as the island, #37): it
                // came out 1100 x 620 for 1650 x 930 on a display at 150 %,
                // off to one side (2026-10-21). Laid again once it has.
                let again = win.clone();
                std::thread::spawn(move || {
                    for _ in 0..3 {
                        std::thread::sleep(Duration::from_millis(200));
                        let now = again.outer_size().map(|s| (s.width as i32, s.height as i32)).unwrap_or(size);
                        let at = again.outer_position().map(|p| (p.x, p.y)).unwrap_or(pos);
                        if (now, at) != (size, pos) {
                            lay(&again, pos, size);
                        }
                    }
                });
            }
            let _ = win.show();
            push(sh);
        } else {
            let _ = win.hide();
        }
    }
}

pub fn show(sh: &Arc<Shared>, on: bool) {
    SHOWN.store(on, Ordering::SeqCst);
    apply(sh);
}

// Its key pressed: up, or away.
pub fn toggle(sh: &Arc<Shared>) {
    if sh.setting("lol") == false {
        sh.log("scout: its key pressed, the League helper off");
        return;
    }
    let on = !SHOWN.load(Ordering::SeqCst);
    sh.log(&format!("scout: its key pressed, the panel {}", if on { "up" } else { "away" }));
    // Nothing of a game to show (none yet, or one that could not be looked
    // up): her own, looked up now.
    let has_ten = VIEW.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|v| v["state"] == "ready" && v["solo"] == "");
    if on && !has_ten && !SCOUTING.swap(true, Ordering::SeqCst) {
        let sh = sh.clone();
        crate::spawn_named("scout", move || {
            scout(&sh, false);
            SCOUTING.store(false, Ordering::SeqCst);
        });
    }
    show(sh, on);
}

// The game loaded and under way: the panel goes, until its key is pressed.
pub fn on_game_started(sh: &Arc<Shared>) {
    if SHOWN.load(Ordering::SeqCst) {
        show(sh, false);
    }
}

// --- Its key -----------------------------------------------------------------------------------

// The key (scoutKey) taken for her while she runs: pressed anywhere, in the
// game too, the panel comes or goes. Read as she starts.
#[cfg(windows)]
pub fn listen_key(sh: &Arc<Shared>) {
    let name = key_name(sh);
    let Some((mods, vk)) = parse_key(&name) else { return };
    let sh = sh.clone();
    crate::spawn_named("scout-key", move || {
        #[repr(C)]
        struct Msg {
            hwnd: isize,
            message: u32,
            wparam: usize,
            lparam: isize,
            time: u32,
            pt: [i32; 2],
            private: u32,
        }
        // backplate.rs declares GetMessageW with its own MSG: the same layout.
        #[allow(clashing_extern_declarations)]
        #[link(name = "user32")]
        extern "system" {
            fn RegisterHotKey(hwnd: isize, id: i32, mods: u32, vk: u32) -> i32;
            fn GetMessageW(msg: *mut Msg, hwnd: isize, min: u32, max: u32) -> i32;
        }
        const MOD_NOREPEAT: u32 = 0x4000;
        const WM_HOTKEY: u32 = 0x0312;
        // SAFETY: a hotkey of this thread's, and its messages into our own struct.
        unsafe {
            if RegisterHotKey(0, 1, mods | MOD_NOREPEAT, vk) == 0 {
                sh.log(&format!("scout: the key {name} is taken by another program"));
                return;
            }
            sh.log(&format!("scout: the key {name} is hers"));
            let mut msg: Msg = std::mem::zeroed();
            while GetMessageW(&mut msg, 0, 0, 0) > 0 {
                if msg.message == WM_HOTKEY {
                    toggle(&sh);
                }
            }
        }
    });
}

#[cfg(not(windows))]
pub fn listen_key(_sh: &Arc<Shared>) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_by_their_names() {
        assert_eq!(parse_key("Alt+`"), Some((1, 0xC0)));
        assert_eq!(parse_key(DEFAULT_KEY), Some((1, b'X' as u32)));
        assert_eq!(parse_key("Ctrl+Shift+F9"), Some((6, 0x78)));
        assert_eq!(parse_key("alt+z"), Some((1, b'Z' as u32)));
        assert_eq!(parse_key("Alt+Nope"), None);
        assert_eq!(parse_key("Alt"), None);
    }

    #[test]
    fn a_game_kept_and_how_the_last_ones_went() {
        let m = |id: &str, dur: f64, me_champ: u64, me_win: bool, k: u64| {
            json!({ "metadata": { "matchId": id }, "info": { "gameDuration": dur, "queueId": 420, "gameEndTimestamp": 1,
                "participants": [
                    { "puuid": "me", "championId": me_champ, "win": me_win, "kills": k, "deaths": 2, "assists": 4, "goldEarned": 9000 },
                    { "puuid": "foe", "championId": 1, "win": !me_win, "kills": 1, "deaths": 1, "assists": 1 } ] } })
        };
        // A remake is none.
        assert_eq!(trim_match(&m("TW2_0", 200.0, 103, true, 0)), None);
        let kept = trim_match(&m("TW2_1", 1800.0, 103, true, 6)).unwrap();
        let me = &kept["players"][0];
        assert_eq!((me["puuid"].as_str(), me["champion"].as_u64(), me["win"].as_bool(), me["k"].as_u64(), me["d"].as_u64(), me["a"].as_u64()), (Some("me"), Some(103), Some(true), Some(6), Some(2), Some(4)));
        assert_eq!(kept["v"], KEPT);
        // Latest first: two won, then a loss; twice on Ahri (103), both won.
        let games: Vec<Value> = [m("a", 1800.0, 103, true, 6), m("b", 1800.0, 103, true, 4), m("c", 1800.0, 64, false, 2)].iter().filter_map(trim_match).collect();
        let mut r = recent_of("me", 103, &games);
        assert_eq!((r["list"][2]["champion"].as_u64(), r["list"][2]["win"].as_bool(), r["list"][2]["queue"].as_u64()), (Some(64), Some(false), Some(420)));
        r.as_object_mut().unwrap().remove("list");
        assert_eq!(r, json!({ "games": 3, "wins": 2, "k": 4.0, "d": 2.0, "a": 4.0, "champGames": 2, "champWins": 2, "streak": 2 }));
        assert_eq!(recent_of("foe", 1, &games)["streak"], -2);
        assert_eq!(recent_of("nobody", 1, &games), Value::Null);
    }

    #[test]
    fn match_v5_s_region_for_each_platform() {
        assert_eq!(account_region_of("tw2"), "asia");
        assert_eq!(account_region_of("euw1"), "europe");
        assert_eq!(region_of("tw2"), "sea");
        assert_eq!(region_of("kr"), "asia");
        assert_eq!(region_of("euw1"), "europe");
        assert_eq!(region_of("na1"), "americas");
    }
}
