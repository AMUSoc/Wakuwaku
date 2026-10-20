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

// What a kept game holds; one kept with less (an older version) is read
// again. 3: each player's Riot ID, damage to champions, gold and vision,
// for a game's whole scoreboard (scout_game).
const KEPT: u64 = 3;

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
                "name": format!("{}#{}", p["riotIdGameName"].as_str().unwrap_or(""), p["riotIdTagline"].as_str().unwrap_or("")).trim_matches('#'),
                "dmg": n("totalDamageDealtToChampions"), "gold": n("goldEarned"), "vision": n("visionScore"),
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
            for k in ["id", "queue", "end", "duration"] {
                row[k] = g[k].clone();
            }
            // Whose row: the scoreboard (scout_game) marks them in it.
            row["who"] = json!(puuid);
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
    let mut by: Vec<(String, Vec<&Value>)> = Vec::new();
    for g in games.iter().filter(|g| g["queue"] == 420) {
        let Some(p) = g["players"].as_array().and_then(|ps| ps.iter().find(|p| p["puuid"] == puuid)) else { continue };
        let pos = p["pos"].as_str().filter(|s| !s.is_empty()).unwrap_or("NONE").to_string();
        match by.iter_mut().find(|(r, _)| *r == pos) {
            Some(row) => row.1.push(g),
            None => by.push((pos, vec![g])),
        }
    }
    by.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    Value::Array(
        by.into_iter()
            .map(|(pos, games)| {
                let owned: Vec<Value> = games.into_iter().cloned().collect();
                let season = season_of(puuid, &owned);
                let wins = season["champs"].as_array().into_iter().flatten().map(|c| c["wins"].as_u64().unwrap_or(0)).sum::<u64>();
                json!({ "pos": pos, "games": owned.len(), "wins": wins, "champs": season["champs"] })
            })
            .collect(),
    )
}

// Her games by champion, the most played first: how many and won, her
// kills, deaths and assists, minions and minutes all told, the positions
// played it in, and the games' ids (the latest first: games is so).
pub fn season_of(puuid: &str, games: &[Value]) -> Value {
    let mut by: Vec<(u64, serde_json::Map<String, Value>)> = Vec::new();
    for g in games {
        let Some(p) = g["players"].as_array().and_then(|ps| ps.iter().find(|p| p["puuid"] == puuid)) else { continue };
        let champion = p["champion"].as_u64().unwrap_or(0);
        let i = match by.iter().position(|(c, _)| *c == champion) {
            Some(i) => i,
            None => {
                let row = json!({ "champion": champion, "games": 0, "wins": 0, "k": 0, "d": 0, "a": 0, "cs": 0, "minutes": 0.0, "roles": {}, "ids": [] });
                by.push((champion, row.as_object().cloned().unwrap_or_default()));
                by.len() - 1
            }
        };
        let row = &mut by[i].1;
        let add = |row: &mut serde_json::Map<String, Value>, k: &str, n: u64| {
            let was = row[k].as_u64().unwrap_or(0);
            row.insert(k.to_string(), json!(was + n));
        };
        add(row, "games", 1);
        add(row, "wins", p["win"].as_bool().unwrap_or(false) as u64);
        for k in ["k", "d", "a", "cs"] {
            add(row, k, p[k].as_u64().unwrap_or(0));
        }
        let minutes = row["minutes"].as_f64().unwrap_or(0.0) + g["duration"].as_f64().unwrap_or(0.0) / 60.0;
        row.insert("minutes".into(), json!(minutes));
        let pos = p["pos"].as_str().filter(|s| !s.is_empty()).unwrap_or("NONE").to_string();
        let roles = row.get_mut("roles").and_then(Value::as_object_mut).expect("roles");
        let n = roles.get(&pos).and_then(Value::as_u64).unwrap_or(0);
        roles.insert(pos, json!(n + 1));
        row.get_mut("ids").and_then(Value::as_array_mut).expect("ids").push(g["id"].clone());
    }
    by.sort_by(|a, b| b.1["games"].as_u64().cmp(&a.1["games"].as_u64()));
    json!({ "champs": by.into_iter().map(|(_, row)| Value::Object(row)).collect::<Vec<_>>() })
}

// Her rows (as recent_of lists them) of these games kept, for a
// champion's or a position's games opened in the page (scout_rows).
pub fn rows_of(sh: &Shared, ids: &[String], who: &str) -> Value {
    let games: Vec<Value> = ids.iter().map(|id| kept_game(sh, id)).filter(|g| g.is_object()).collect();
    recent_of(who, 0, &games)["list"].clone()
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
    // Spectator's: their two summoner spells, keystone and second tree.
    spells: Value,
    keystone: Value,
    sub: Value,
    rank: Value,
    mastery: Value,
    recent: Value,
    ids: Vec<String>,
}

fn player_view(p: &Player, names: &HashMap<u64, String>, group: u64) -> Value {
    json!({
        "name": p.name, "me": p.me, "championId": p.champion,
        "champion": names.get(&p.champion).cloned().unwrap_or_default(),
        "rank": p.rank, "mastery": p.mastery, "recent": p.recent,
        "spells": p.spells, "keystone": p.keystone, "sub": p.sub, "group": group,
    })
}

// Those on a team who play together: two who share two or more of their
// last games are one group (and so on, a group joined through any one of
// them). Groups numbered from 1 by their first player; 0, none.
// ids: each player's last games; team: each one's team.
pub fn groups_of(ids: &[Vec<String>], team: &[u64]) -> Vec<u64> {
    let n = ids.len();
    let mut root: Vec<usize> = (0..n).collect();
    fn find(root: &mut Vec<usize>, i: usize) -> usize {
        let mut i = i;
        while root[i] != i {
            root[i] = root[root[i]];
            i = root[i];
        }
        i
    }
    for a in 0..n {
        for b in a + 1..n {
            if team[a] != team[b] {
                continue;
            }
            let shared = ids[a].iter().filter(|id| ids[b].contains(id)).count();
            if shared >= 2 {
                let (ra, rb) = (find(&mut root, a), find(&mut root, b));
                root[rb] = ra;
            }
        }
    }
    let roots: Vec<usize> = (0..n).map(|i| find(&mut root, i)).collect();
    let mut numbers: Vec<(usize, u64)> = Vec::new();
    (0..n)
        .map(|i| {
            let alone = roots.iter().filter(|r| **r == roots[i]).count() < 2;
            if alone {
                return 0;
            }
            match numbers.iter().find(|(r, _)| *r == roots[i]) {
                Some((_, g)) => *g,
                None => {
                    let g = numbers.len() as u64 + 1;
                    numbers.push((roots[i], g));
                    g
                }
            }
        })
        .collect()
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

// The key and match-v5's host the last look used: a game's timeline is
// asked for later, as its scoreboard opens (scout_timeline).
static LAST: Mutex<Option<(String, String)>> = Mutex::new(None);

fn remember_hosts(key: &str, regional: &str) {
    *LAST.lock().unwrap_or_else(|e| e.into_inner()) = Some((key.to_string(), regional.to_string()));
}

// A game's timeline as kept: each minute's gold, experience, minions and
// level of the ten (by their place, 1 to 10, as the game's players are
// listed), and what happened: kills (who, whom, who helped, where), items
// bought, skills levelled, Dragons, Heralds and Barons, buildings lost.
pub fn trim_timeline(t: &Value) -> Option<Value> {
    let info = &t["info"];
    let puuids: Vec<Value> = (1..=10).map(|i| info["participants"].as_array().and_then(|ps| ps.iter().find(|p| p["participantId"] == i)).map_or(Value::Null, |p| p["puuid"].clone())).collect();
    let mut frames = Vec::new();
    let (mut kills, mut items, mut skills, mut monsters, mut buildings) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for f in info["frames"].as_array()? {
        let of = |k: &dyn Fn(&Value) -> u64| (1..=10).map(|i| k(&f["participantFrames"][i.to_string()])).collect::<Vec<_>>();
        frames.push(json!({
            "t": (f["timestamp"].as_f64().unwrap_or(0.0) / 60000.0).round(),
            "gold": of(&|p| p["totalGold"].as_u64().unwrap_or(0)),
            "xp": of(&|p| p["xp"].as_u64().unwrap_or(0)),
            "cs": of(&|p| p["minionsKilled"].as_u64().unwrap_or(0) + p["jungleMinionsKilled"].as_u64().unwrap_or(0)),
            "level": of(&|p| p["level"].as_u64().unwrap_or(0)),
        }));
        for e in f["events"].as_array().into_iter().flatten() {
            let t = (e["timestamp"].as_f64().unwrap_or(0.0) / 1000.0).round();
            match e["type"].as_str().unwrap_or("") {
                "CHAMPION_KILL" => kills.push(json!({ "t": t, "killer": e["killerId"], "victim": e["victimId"], "assists": e["assistingParticipantIds"].as_array().cloned().unwrap_or_default(), "x": e["position"]["x"], "y": e["position"]["y"] })),
                "ITEM_PURCHASED" => items.push(json!({ "t": t, "p": e["participantId"], "item": e["itemId"] })),
                "SKILL_LEVEL_UP" if e["levelUpType"] == "NORMAL" => skills.push(json!({ "t": t, "p": e["participantId"], "slot": e["skillSlot"] })),
                "ELITE_MONSTER_KILL" => monsters.push(json!({ "t": t, "team": e["killerTeamId"], "type": e["monsterType"], "sub": e["monsterSubType"] })),
                "BUILDING_KILL" => buildings.push(json!({ "t": t, "lost": e["teamId"], "type": e["buildingType"], "lane": e["laneType"] })),
                _ => {}
            }
        }
    }
    Some(json!({ "v": 1, "puuids": puuids, "frames": frames, "kills": kills, "items": items, "skills": skills, "monsters": monsters, "buildings": buildings }))
}

// A game's timeline, kept on disk or read once from Riot (one request) and
// kept; none without a look having been made yet (no key, no host).
pub fn timeline(sh: &Shared, id: &str) -> Value {
    let path = sh.dir.join("lol").join("timelines").join(format!("{}.json", id.replace(['/', '\\', '.'], "_")));
    if let Some(kept) = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
        return kept;
    }
    let Some((key, regional)) = LAST.lock().unwrap_or_else(|e| e.into_inner()).clone() else { return Value::Null };
    let Some(kept) = riot(&key, &format!("{regional}/lol/match/v5/matches/{id}/timeline")).ok().as_ref().and_then(trim_timeline) else { return Value::Null };
    let _ = std::fs::create_dir_all(path.parent().unwrap_or(&path));
    let _ = std::fs::write(&path, kept.to_string());
    kept
}

fn game_path(sh: &Shared, id: &str) -> PathBuf {
    matches_dir(sh).join(format!("{}.json", id.replace(['/', '\\', '.'], "_")))
}

// A game as kept (read before, for a row of her games or a player's): its
// whole scoreboard, for the page (scout_game). None if not kept.
pub fn kept_game(sh: &Shared, id: &str) -> Value {
    std::fs::read_to_string(game_path(sh, id)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).unwrap_or(Value::Null)
}

// A game, kept on disk or read once from Riot and kept.
fn game(sh: &Shared, key: &str, regional: &str, id: &str) -> Option<Value> {
    let path = game_path(sh, id);
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
    remember_hosts(&key, &regional);
    // Spectator has the game a little after it starts loading; a custom
    // game (the practice tool) never: her own at once, not a minute later.
    let custom = in_game && crate::lcu::is_custom_game() == Some(true);
    // Its own page wants the champion she plays: once the game has loaded
    // (the game's own data up, lol.rs), at most a minute.
    if custom {
        for _ in 0..30 {
            if crate::lol::in_game() {
                break;
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    }
    let mut live = Err(Miss::None);
    for _ in 0..if in_game && !custom { 12 } else { 0 } {
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
                spells: json!([p["spell1Id"], p["spell2Id"]]),
                keystone: p["perks"]["perkIds"][0].clone(),
                sub: p["perks"]["perkSubStyle"].clone(),
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
        let ids: Vec<Vec<String>> = players.iter().map(|p| p.ids.clone()).collect();
        let teams: Vec<u64> = players.iter().map(|p| p.team).collect();
        let groups = groups_of(&ids, &teams);
        let team = |mine: bool| players.iter().zip(&groups).filter(|(p, _)| (p.team == my_team) == mine).map(|(p, g)| player_view(p, &names, *g)).collect::<Vec<_>>();
        publish(sh, json!({ "state": "ready", "key": key_name(sh), "queue": queue, "us": team(true), "them": team(false), "done": done, "total": total, "solo": solo, "names": names_json(&names) }));
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
    remember_hosts(key, regional);
    let why = if crate::lol::in_game() { "nogame" } else { "outside" };
    let summoner = riot(key, &format!("{host}/lol/summoner/v4/summoners/by-puuid/{me}")).unwrap_or(Value::Null);
    let entries = riot(key, &format!("{host}/lol/league/v4/entries/by-puuid/{me}")).unwrap_or(json!([]));
    // Every champion's mastery, in one call (the most first); the top three of it.
    let masteries: Vec<Value> = riot(key, &format!("{host}/lol/champion-mastery/v4/champion-masteries/by-puuid/{me}"))
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .map(|m| json!({ "champion": m["championId"], "level": m["championLevel"], "points": m["championPoints"], "last": m["lastPlayTime"] }))
        .collect();
    let top: Vec<Value> = masteries.iter().take(3).cloned().collect();
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
            "top": top, "masteries": masteries, "now": now, "recent": null, "roles": [], "rolesOf": 0, "rolesTotal": 0, "season": null, "me": me,
            "pos": LAST_POS.lock().unwrap_or_else(|e| e.into_inner()).clone(),
        },
    });
    publish(sh, view.clone());
    let ids = |query: &str| -> Vec<String> {
        let query = if query.contains("start=") { query.to_string() } else { format!("&start=0{query}") };
        match riot(key, &format!("{regional}/lol/match/v5/matches/by-puuid/{me}/ids?{}", query.trim_start_matches('&'))) {
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
    // This season's Solo/Duo games, every one: as many as league-v4 counts
    // her won and lost (no season's start date needed), a hundred ids a
    // page; unranked, the last hundred.
    let season = view["profile"]["solo"].as_object().map_or(100, |r| (r["wins"].as_u64().unwrap_or(0) + r["losses"].as_u64().unwrap_or(0)) as usize).clamp(1, 1000);
    let mut solo_ids: Vec<String> = Vec::new();
    while solo_ids.len() < season {
        let page = ids(&format!("&queue=420&start={}&count=100", solo_ids.len()));
        let got = page.len();
        solo_ids.extend(page);
        if got < 100 {
            break;
        }
    }
    solo_ids.truncate(season);
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
            view["profile"]["season"] = season_of(me, &solo_games);
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
    // In English too: OP.GG is asked by them (opgg.rs).
    let champions_en: serde_json::Map<String, Value> = get(&format!("{base}/cdn/{v}/data/en_US/champion.json"))
        .and_then(|c| c["data"].as_object().cloned())
        .unwrap_or_default()
        .values()
        .filter_map(|c| Some((c["key"].as_str()?.to_string(), json!(c["name"].as_str()?))))
        .collect();
    // The stat shards' icons (Data Dragon's runes have none): CommunityDragon's.
    let cdragon = "https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default";
    let shards: serde_json::Map<String, Value> = get(&format!("{cdragon}/v1/perks.json"))
        .and_then(|p| p.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter(|p| p["id"].as_u64().is_some_and(|id| (5000..6000).contains(&id)))
        .filter_map(|p| {
            let path = p["iconPath"].as_str()?.to_lowercase().replace("/lol-game-data/assets/", "/");
            Some((p["id"].to_string(), json!(format!("{cdragon}{path}"))))
        })
        .collect();
    let champions: serde_json::Map<String, Value> = get(&format!("{base}/cdn/{v}/data/{locale}/champion.json"))
        .and_then(|c| c["data"].as_object().cloned())
        .unwrap_or_default()
        .values()
        .filter_map(|c| Some((c["key"].as_str()?.to_string(), json!(c["name"].as_str()?))))
        .collect();
    // The items' names too, for a review's summary (review.rs): an agent
    // given only ids went looking them up (Grok, past four minutes, 2026-10-21).
    let items: serde_json::Map<String, Value> = get(&format!("{base}/cdn/{v}/data/{locale}/item.json"))
        .and_then(|c| c["data"].as_object().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(id, item)| Some((id, json!(item["name"].as_str()?))))
        .collect();
    *DDRAGON.lock().unwrap_or_else(|e| e.into_inner()) = Some(json!({ "v": v, "locale": locale, "spells": spells, "perks": perks, "champions": champions, "championsEn": champions_en, "items": items, "shards": shards }));
}

// The champions' names in her language, by id (Data Dragon's, once read):
// for a review's summary (review.rs). None read yet, none.
pub fn champion_names() -> HashMap<u64, String> {
    let dd = DDRAGON.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or(Value::Null);
    dd["champions"].as_object().into_iter().flatten().filter_map(|(id, n)| Some((id.parse().ok()?, n.as_str()?.to_string()))).collect()
}

// The champions' names in English, by id: OP.GG is asked by them (opgg.rs).
pub fn champion_names_en() -> HashMap<u64, String> {
    let dd = DDRAGON.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or(Value::Null);
    dd["championsEn"].as_object().into_iter().flatten().filter_map(|(id, n)| Some((id.parse().ok()?, n.as_str()?.to_string()))).collect()
}

// The items' names in her language, by id (Data Dragon's, once read).
pub fn item_names() -> HashMap<u64, String> {
    let dd = DDRAGON.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or(Value::Null);
    dd["items"].as_object().into_iter().flatten().filter_map(|(id, n)| Some((id.parse().ok()?, n.as_str()?.to_string()))).collect()
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

// --- Champion select ----------------------------------------------------------------------------

// In champion select now (the panel shown once as it begins).
static SELECTING: AtomicBool = AtomicBool::new(false);
// Her position in the last champion select: the game's page looks her
// champion's build up for it.
static LAST_POS: Mutex<String> = Mutex::new(String::new());

// Champion select as the client says it, every five seconds (lcu.rs): the
// panel's select page, shown as select begins.
pub fn on_select(sh: &Arc<Shared>, select: &Value) {
    let first = !SELECTING.swap(true, Ordering::SeqCst);
    *LAST_POS.lock().unwrap_or_else(|e| e.into_inner()) = select["pos"].as_str().unwrap_or("").to_string();
    let unchanged = VIEW.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|v| v["state"] == "select" && v["select"] == *select);
    if unchanged {
        return;
    }
    // The icons' names (Data Dragon) and her champions' mastery, on a
    // thread: never in the way of the client's look.
    if first {
        *POOL.lock().unwrap_or_else(|e| e.into_inner()) = Value::Null;
        let sh = sh.clone();
        std::thread::spawn(move || {
            load_ddragon(sh.lang());
            let pool = pool_of(&sh);
            *POOL.lock().unwrap_or_else(|e| e.into_inner()) = pool;
            let select = VIEW.lock().unwrap_or_else(|e| e.into_inner()).as_ref().filter(|v| v["state"] == "select").map(|v| v["select"].clone());
            if let Some(select) = select {
                publish(&sh, select_view(&sh, &select));
            }
        });
    }
    publish(sh, select_view(sh, select));
    if first && sh.setting("riotApiKey").as_str().is_some() {
        show(sh, true);
    }
}

// Out of champion select: its page goes (the panel with it, if it showed it).
pub fn on_select_end(sh: &Arc<Shared>) {
    if !SELECTING.swap(false, Ordering::SeqCst) {
        return;
    }
    let was_select = VIEW.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|v| v["state"] == "select");
    if was_select {
        *VIEW.lock().unwrap_or_else(|e| e.into_inner()) = None;
        if !crate::lol::in_game() {
            show(sh, false);
        }
    }
}

// Her champions by mastery (every one, the most first), read as champion
// select begins: the ones she plays well, for the picks against the other
// team's (scout.js). Null until read.
static POOL: Mutex<Value> = Mutex::new(Value::Null);

fn pool_of(sh: &Shared) -> Value {
    let key = sh.setting("riotApiKey").as_str().unwrap_or("").to_string();
    let Some((riot_id, platform, _)) = crate::lcu::me_platform_names() else { return Value::Null };
    let Some(me) = puuid_of(&key, platform, &riot_id) else { return Value::Null };
    let (host, regional) = hosts(platform);
    remember_hosts(&key, &regional);
    riot(&key, &format!("{host}/lol/champion-mastery/v4/champion-masteries/by-puuid/{me}"))
        .ok()
        .and_then(|v| v.as_array().cloned())
        .map(|all| Value::Array(all.iter().map(|m| json!({ "champion": m["championId"], "level": m["championLevel"], "points": m["championPoints"] })).collect()))
        .unwrap_or(Value::Null)
}

fn select_view(sh: &Shared, select: &Value) -> Value {
    let pool = POOL.lock().unwrap_or_else(|e| e.into_inner()).clone();
    json!({ "state": "select", "key": key_name(sh), "select": select, "pool": pool })
}

// --- Its window ---------------------------------------------------------------------------------

fn window(sh: &Shared) -> Option<WebviewWindow> {
    sh.app.get_webview_window(LABEL)
}

// The middle of the display the island is on.
// The middle of the display the game is on (its window's middle), else of
// the island's: on another display it was never seen (2026-10-21).
fn rect(sh: &Shared) -> Option<((i32, i32), (i32, i32), f64)> {
    let a = game_window().and_then(|(x, y)| sh.screens().near(x, y)).or_else(|| crate::island::area(sh))?;
    let w = ((SIZE.0 * a.sf).round() as i32).min(a.w);
    let h = ((SIZE.1 * a.sf).round() as i32).min(a.h);
    Some(((a.x + (a.w - w) / 2, a.y + (a.h - h) / 2), (w, h), a.sf))
}

// The middle of the game's own window (its largest one shown), physical; none without a game.
#[cfg(windows)]
fn game_window() -> Option<(i32, i32)> {
    let pid = crate::lol::pid_of("league of legends.exe")?;
    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        l: i32,
        t: i32,
        r: i32,
        b: i32,
    }
    type EnumProc = unsafe extern "system" fn(isize, isize) -> i32;
    // island.rs and detach.rs declare these with their own types: the same ABI.
    #[allow(clashing_extern_declarations)]
    #[link(name = "user32")]
    extern "system" {
        fn EnumWindows(f: EnumProc, l: isize) -> i32;
        fn GetWindowThreadProcessId(hwnd: isize, pid: *mut u32) -> u32;
        fn IsWindowVisible(hwnd: isize) -> i32;
        fn GetWindowRect(hwnd: isize, rect: *mut Rect) -> i32;
    }
    struct Search {
        pid: u32,
        best: Option<(i64, (i32, i32))>,
    }
    unsafe extern "system" fn each(hwnd: isize, l: isize) -> i32 {
        // SAFETY: l is the Search below, alive for the enumeration.
        let search = unsafe { &mut *(l as *mut Search) };
        let mut pid = 0;
        let mut r = Rect::default();
        // SAFETY: reads of a window's process and place into our own values.
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == search.pid && IsWindowVisible(hwnd) != 0 && GetWindowRect(hwnd, &mut r) != 0 {
                let size = (r.r - r.l) as i64 * (r.b - r.t) as i64;
                if search.best.is_none_or(|(s, _)| size > s) {
                    search.best = Some((size, ((r.l + r.r) / 2, (r.t + r.b) / 2)));
                }
            }
        }
        1
    }
    let mut search = Search { pid, best: None };
    // SAFETY: the callback is given our Search, which outlives the call.
    unsafe { EnumWindows(each, &mut search as *mut Search as isize) };
    search.best.map(|(_, at)| at)
}

#[cfg(not(windows))]
fn game_window() -> Option<(i32, i32)> {
    None
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
        if let Some(dd) = view["dd"].as_object_mut() {
            dd.remove("items");
            dd.remove("championsEn");
        }
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
    let has_ten = VIEW.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|v| (v["state"] == "ready" && v["solo"] == "") || v["state"] == "select");
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

// The key (scoutKey), heard wherever it is pressed, the game in front too:
// the panel comes or goes. Read as she starts.
//
// Not a hotkey of Windows' (RegisterHotKey): pressed in the game, none ever
// came (2026-10-21), the game keeping the keyboard's hotkeys from Windows
// (raw input's RIDEV_NOHOTKEYS). A low-level keyboard hook sees every key
// before any program does, as a push-to-talk key does; it only looks, every
// key goes on where it was going.
#[cfg(windows)]
mod key {
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::{Arc, OnceLock};

    use crate::Shared;

    pub static SHARED: OnceLock<Arc<Shared>> = OnceLock::new();
    pub static MODS: AtomicU32 = AtomicU32::new(0);
    pub static VK: AtomicU32 = AtomicU32::new(0);
    // Held: its repeats are not presses.
    static DOWN: AtomicBool = AtomicBool::new(false);

    #[repr(C)]
    struct KeyEvent {
        vk: u32,
        scan: u32,
        flags: u32,
        time: u32,
        extra: usize,
    }

    #[link(name = "user32")]
    extern "system" {
        fn CallNextHookEx(hook: isize, code: i32, w: usize, l: isize) -> isize;
        fn GetAsyncKeyState(vk: i32) -> i16;
    }

    fn held(vk: i32) -> bool {
        // SAFETY: a read of the keyboard's state.
        unsafe { GetAsyncKeyState(vk) < 0 }
    }

    // The modifiers held now, as parse_key names them (Alt 1, Ctrl 2, Shift 4, Win 8).
    fn mods_now() -> u32 {
        (held(0x12) as u32) | (held(0x11) as u32) << 1 | (held(0x10) as u32) << 2 | ((held(0x5B) || held(0x5C)) as u32) << 3
    }

    pub unsafe extern "system" fn on_key(code: i32, w: usize, l: isize) -> isize {
        const WM_KEYDOWN: usize = 0x100;
        const WM_KEYUP: usize = 0x101;
        const WM_SYSKEYDOWN: usize = 0x104;
        const WM_SYSKEYUP: usize = 0x105;
        if code >= 0 && l != 0 {
            // SAFETY: Windows hands a KBDLLHOOKSTRUCT for a low-level keyboard hook.
            let event = unsafe { &*(l as *const KeyEvent) };
            if event.vk == VK.load(Ordering::Relaxed) {
                match w {
                    WM_KEYDOWN | WM_SYSKEYDOWN if mods_now() == MODS.load(Ordering::Relaxed) && !DOWN.swap(true, Ordering::SeqCst) => {
                        // Off the hook's own thread: Windows gives it a moment only.
                        if let Some(sh) = SHARED.get().cloned() {
                            std::thread::spawn(move || super::toggle(&sh));
                        }
                    }
                    WM_KEYUP | WM_SYSKEYUP => DOWN.store(false, Ordering::SeqCst),
                    _ => {}
                }
            }
        }
        // SAFETY: the key goes on, as it was going.
        unsafe { CallNextHookEx(0, code, w, l) }
    }
}

#[cfg(windows)]
pub fn listen_key(sh: &Arc<Shared>) {
    let name = key_name(sh);
    let Some((mods, vk)) = parse_key(&name) else { return };
    key::MODS.store(mods, Ordering::SeqCst);
    key::VK.store(vk, Ordering::SeqCst);
    let _ = key::SHARED.set(sh.clone());
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
        type HookProc = unsafe extern "system" fn(i32, usize, isize) -> isize;
        // backplate.rs declares GetMessageW with its own MSG: the same layout.
        #[allow(clashing_extern_declarations)]
        #[link(name = "user32")]
        extern "system" {
            fn SetWindowsHookExW(id: i32, hook: HookProc, module: isize, thread: u32) -> isize;
            fn GetMessageW(msg: *mut Msg, hwnd: isize, min: u32, max: u32) -> i32;
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GetModuleHandleW(name: *const u16) -> isize;
        }
        const WH_KEYBOARD_LL: i32 = 13;
        // SAFETY: a hook of this thread's, its messages pumped here (a
        // low-level hook is called on the thread that set it, through its loop).
        unsafe {
            if SetWindowsHookExW(WH_KEYBOARD_LL, key::on_key, GetModuleHandleW(std::ptr::null()), 0) == 0 {
                sh.log(&format!("scout: the key {name} could not be listened for"));
                return;
            }
            sh.log(&format!("scout: the key {name} is heard (low-level hook)"));
            let mut msg: Msg = std::mem::zeroed();
            while GetMessageW(&mut msg, 0, 0, 0) > 0 {}
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
    fn those_who_play_together_are_found_by_their_games() {
        let ids = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let all = vec![ids(&["a", "b", "c"]), ids(&["b", "c", "x"]), ids(&["y"]), ids(&["c", "x", "z"]), ids(&["a", "b"])];
        // 0 and 1 share b and c; 1 and 3 share c and x: one group of three.
        // 4 shares a and b with 0, but plays on the other team: none.
        assert_eq!(groups_of(&all, &[100, 100, 100, 100, 200]), vec![1, 1, 0, 1, 0]);
        assert_eq!(groups_of(&[ids(&["a"]), ids(&["a"])], &[100, 100]), vec![0, 0]);
    }

    #[test]
    fn her_season_by_champion_and_position() {
        let g = |id: &str, champ: u64, win: bool, pos: &str| {
            json!({ "id": id, "queue": 420, "duration": 1800.0, "players": [{ "puuid": "me", "champion": champ, "win": win, "k": 5, "d": 2, "a": 7, "cs": 180, "pos": pos }] })
        };
        let games = vec![g("a", 234, true, "MIDDLE"), g("b", 234, false, "JUNGLE"), g("c", 103, true, "MIDDLE")];
        let s = season_of("me", &games);
        let viego = &s["champs"][0];
        assert_eq!((viego["champion"].as_u64(), viego["games"].as_u64(), viego["wins"].as_u64(), viego["cs"].as_u64()), (Some(234), Some(2), Some(1), Some(360)));
        assert_eq!(viego["ids"], json!(["a", "b"]));
        assert_eq!(viego["roles"], json!({ "MIDDLE": 1, "JUNGLE": 1 }));
        assert_eq!(viego["minutes"].as_f64(), Some(60.0));
        let roles = roles_of("me", &games);
        assert_eq!((roles[0]["pos"].as_str(), roles[0]["games"].as_u64(), roles[0]["wins"].as_u64()), (Some("MIDDLE"), Some(2), Some(2)));
        assert_eq!(roles[0]["champs"].as_array().map(|c| c.len()), Some(2));
    }

    #[test]
    fn a_timeline_kept() {
        let pf = |gold: u64| json!({ "totalGold": gold, "xp": 100, "minionsKilled": 10, "jungleMinionsKilled": 2, "level": 2 });
        let mut frame = json!({ "timestamp": 60000, "participantFrames": {}, "events": [
            { "type": "CHAMPION_KILL", "timestamp": 61000, "killerId": 1, "victimId": 6, "assistingParticipantIds": [2], "position": { "x": 100, "y": 200 } },
            { "type": "ITEM_PURCHASED", "timestamp": 62000, "participantId": 1, "itemId": 1055 },
            { "type": "SKILL_LEVEL_UP", "timestamp": 63000, "participantId": 1, "skillSlot": 1, "levelUpType": "NORMAL" },
            { "type": "SKILL_LEVEL_UP", "timestamp": 63000, "participantId": 1, "skillSlot": 4, "levelUpType": "EVOLVE" },
            { "type": "WARD_PLACED", "timestamp": 64000 } ] });
        for i in 1..=10 {
            frame["participantFrames"][i.to_string()] = pf(500 + i);
        }
        let t = json!({ "info": { "participants": [{ "participantId": 1, "puuid": "me" }], "frames": [frame] } });
        let kept = trim_timeline(&t).unwrap();
        assert_eq!(kept["puuids"][0], "me");
        assert_eq!(kept["frames"][0]["t"], 1.0);
        assert_eq!(kept["frames"][0]["gold"][0], 501);
        assert_eq!(kept["frames"][0]["cs"][9], 12);
        assert_eq!(kept["kills"][0], json!({ "t": 61.0, "killer": 1, "victim": 6, "assists": [2], "x": 100, "y": 200 }));
        assert_eq!(kept["items"].as_array().map(|a| a.len()), Some(1));
        // An evolution is no level of a skill.
        assert_eq!(kept["skills"].as_array().map(|a| a.len()), Some(1));
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
