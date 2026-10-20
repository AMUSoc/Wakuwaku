// The game's ten players at a glance, as Porofessor shows them: as a game
// loads, each one's rank, this season's wins, their mastery of the champion
// they play and how their last games went, in a panel in the middle of the
// screen (src/scout/), shown as the game loads and on a key (scoutKey,
// Alt+` unless set) after.
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
const SIZE: (f64, f64) = (1100.0, 620.0);
// Of each player's games, how many are read.
const GAMES: usize = 10;
// A personal key's limits, kept a little under.
const PER_2_MIN: usize = 95;
const PER_SECOND: usize = 18;
const DEFAULT_KEY: &str = "Alt+`";

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

// A game as kept: who played what, and how it went for each. Remakes (under
// five minutes) are none.
pub fn trim_match(m: &Value) -> Option<Value> {
    let info = &m["info"];
    if info["gameDuration"].as_f64().unwrap_or(0.0) < 300.0 {
        return None;
    }
    let players: Vec<Value> = info["participants"]
        .as_array()?
        .iter()
        .map(|p| json!({ "puuid": p["puuid"], "champion": p["championId"], "win": p["win"], "k": p["kills"], "d": p["deaths"], "a": p["assists"] }))
        .collect();
    Some(json!({ "id": m["metadata"]["matchId"], "queue": info["queueId"], "end": info["gameEndTimestamp"], "players": players }))
}

// How one player's last games went: how many, won, their kills, deaths and
// assists a game, on this champion how many and won, and the streak they are
// on (+3 three won in a row, -2 two lost). games: as kept, the latest first.
pub fn recent_of(puuid: &str, champion: u64, games: &[Value]) -> Value {
    let mine: Vec<&Value> = games.iter().filter_map(|g| g["players"].as_array()?.iter().find(|p| p["puuid"] == puuid)).collect();
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
    if let Some(kept) = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
        return Some(kept);
    }
    let whole = riot(key, &format!("{regional}/lol/match/v5/matches/{id}")).ok()?;
    let kept = trim_match(&whole).unwrap_or_else(|| json!({ "id": id, "remake": true, "players": [] }));
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
        scout(&sh);
        SCOUTING.store(false, Ordering::SeqCst);
    });
}

fn scout(sh: &Arc<Shared>) {
    let key = sh.setting("riotApiKey").as_str().unwrap_or("").to_string();
    if key.is_empty() {
        status(sh, "nokey");
        show(sh, true);
        return;
    }
    status(sh, "looking");
    show(sh, true);
    // Tests: someone's game on Taiwan's server looked up as if hers
    // (WAKUWAKU_SCOUT_AS=<their PUUID>), no client needed.
    let as_someone = std::env::var("WAKUWAKU_SCOUT_AS").ok().map(|puuid| (puuid, "tw2", HashMap::new()));
    let Some((me, platform, names)) = as_someone.or_else(crate::lcu::me_platform_names) else {
        sh.log("scout: the client is not there to say who she is");
        status(sh, "noclient");
        return;
    };
    let (host, regional) = hosts(platform);
    // Spectator has the game a little after it starts loading.
    let mut live = Err(Miss::None);
    for _ in 0..12 {
        live = riot(&key, &format!("{host}/lol/spectator/v5/active-games/by-summoner/{me}"));
        if live.is_ok() || live == Err(Miss::Key) {
            break;
        }
        std::thread::sleep(Duration::from_secs(5));
    }
    let live = match live {
        Ok(v) => v,
        Err(Miss::Key) => return status(sh, "badkey"),
        Err(_) => return status(sh, "nogame"),
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
        publish(sh, json!({ "state": "ready", "key": key_name(sh), "queue": queue, "us": team(true), "them": team(false), "done": done, "total": total }));
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
        if let Ok(m) = riot(&key, &format!("{host}/lol/champion-mastery/v4/champion-masteries/by-puuid/{puuid}/by-champion/{champion}")) {
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
                // Never in the way of the game: every click goes through.
                let _ = win.set_ignore_cursor_events(true);
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
        return;
    }
    let on = !SHOWN.load(Ordering::SeqCst);
    if on && VIEW.lock().unwrap_or_else(|e| e.into_inner()).is_none() {
        status(sh, "none");
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
        assert_eq!(kept["players"][0], json!({ "puuid": "me", "champion": 103, "win": true, "k": 6, "d": 2, "a": 4 }));
        // Latest first: two won, then a loss; twice on Ahri (103), both won.
        let games: Vec<Value> = [m("a", 1800.0, 103, true, 6), m("b", 1800.0, 103, true, 4), m("c", 1800.0, 64, false, 2)].iter().filter_map(trim_match).collect();
        let r = recent_of("me", 103, &games);
        assert_eq!(r, json!({ "games": 3, "wins": 2, "k": 4.0, "d": 2.0, "a": 4.0, "champGames": 2, "champWins": 2, "streak": 2 }));
        assert_eq!(recent_of("foe", 1, &games)["streak"], -2);
        assert_eq!(recent_of("nobody", 1, &games), Value::Null);
    }

    #[test]
    fn match_v5_s_region_for_each_platform() {
        assert_eq!(region_of("tw2"), "sea");
        assert_eq!(region_of("kr"), "asia");
        assert_eq!(region_of("euw1"), "europe");
        assert_eq!(region_of("na1"), "americas");
    }
}
