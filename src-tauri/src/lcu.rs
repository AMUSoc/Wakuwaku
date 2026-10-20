// The League client in champion select: her own rank and her party's, on
// Codex's pet out on the desktop. Read from the client's own local API (LCU:
// its lockfile beside LeagueClientUx.exe gives the port and the password)
// and Riot's API with the person's own key (settings: riotApiKey).
//
// Only she and those in her party: in ranked Solo/Duo the others on her team
// are hidden by the game, and Riot's policy forbids finding them out ("cannot
// identify or analyze players who are deliberately hidden by the game"). The
// key goes nowhere but Riot's API: never logged, never in a crash report.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use serde_json::{json, Value};

use crate::Shared;

const CLIENT_EXE: &str = "leagueclientux.exe";

// What the pages show: { players: [{ name, me, position, rank }], keyless }.
static SELECT: Mutex<Option<Value>> = Mutex::new(None);

pub fn view() -> Option<Value> {
    SELECT.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

// --- What the client says (pure: tested) ----------------------------------------------------

// The lockfile: name:pid:port:password:protocol.
pub fn parse_lockfile(text: &str) -> Option<(u16, String)> {
    let parts: Vec<&str> = text.trim().split(':').collect();
    if parts.len() < 5 {
        return None;
    }
    Some((parts[2].parse().ok()?, parts[3].to_string()))
}

// Riot's API's platform for the client's region (/riotclient/region-locale).
// A client in Taiwan says "TW2" (2026-10-20): the platform's own name is
// taken as well.
pub fn platform_of(region: &str) -> Option<&'static str> {
    const PLATFORMS: [&str; 17] = ["na1", "euw1", "eun1", "kr", "jp1", "br1", "la1", "la2", "oc1", "tr1", "ru", "tw2", "vn2", "sg2", "ph2", "th2", "me1"];
    if let Some(p) = PLATFORMS.iter().find(|p| p.eq_ignore_ascii_case(region)) {
        return Some(p);
    }
    Some(match region.to_uppercase().as_str() {
        "NA" => "na1",
        "EUW" => "euw1",
        "EUNE" => "eun1",
        "KR" => "kr",
        "JP" => "jp1",
        "BR" => "br1",
        "LAN" => "la1",
        "LAS" => "la2",
        "OCE" => "oc1",
        "TR" => "tr1",
        "RU" => "ru",
        "TW" => "tw2",
        "VN" => "vn2",
        "SG" => "sg2",
        "PH" => "ph2",
        "TH" => "th2",
        "ME" => "me1",
        _ => return None,
    })
}

// Her rank from league-v4's entries: Solo/Duo's, else Flex's; none, unranked.
pub fn rank_of(entries: &Value) -> Value {
    let list = entries.as_array().cloned().unwrap_or_default();
    let pick = ["RANKED_SOLO_5x5", "RANKED_FLEX_SR"].iter().find_map(|q| list.iter().find(|e| e["queueType"] == *q));
    match pick {
        Some(e) => json!({
            "queue": if e["queueType"] == "RANKED_SOLO_5x5" { "solo" } else { "flex" },
            "tier": e["tier"], "rank": e["rank"], "lp": e["leaguePoints"], "wins": e["wins"], "losses": e["losses"],
        }),
        None => Value::Null,
    }
}

// Those on her team in champion select to look up: she and her party (by
// PUUID), nobody else. party: PUUID to name (the lobby's members).
pub fn to_look_up(session: &Value, me: &str, party: &HashMap<String, String>) -> Vec<(String, String)> {
    session["myTeam"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| {
            let puuid = p["puuid"].as_str().unwrap_or("");
            (!puuid.is_empty() && (puuid == me || party.contains_key(puuid))).then(|| (puuid.to_string(), p["assignedPosition"].as_str().unwrap_or("").to_string()))
        })
        .collect()
}

// "riot:password", as Basic auth wants it.
fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ABC[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

// --- Reading the client ----------------------------------------------------------------------

// Where the client is: the folder of LeagueClientUx.exe (its lockfile there).
#[cfg(windows)]
fn client_dir() -> Option<PathBuf> {
    let pid = crate::lol::pid_of(CLIENT_EXE)?;
    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
        fn CloseHandle(handle: isize) -> i32;
        fn QueryFullProcessImageNameW(process: isize, flags: u32, name: *mut u16, size: *mut u32) -> i32;
    }
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    // SAFETY: our own buffer and size; the handle closed.
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process == 0 {
            return None;
        }
        let mut path = [0u16; 512];
        let mut size = 512u32;
        let ok = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut size) != 0;
        CloseHandle(process);
        ok.then(|| PathBuf::from(String::from_utf16_lossy(&path[..size as usize])).parent().map(PathBuf::from)).flatten()
    }
}

#[cfg(not(windows))]
fn client_dir() -> Option<PathBuf> {
    None
}

// The client's API: its URL and Basic auth. WAKUWAKU_LCU_URL (tests:
// tools/lol_fake.py) stands for it, no client looked for.
fn client() -> Option<(String, String)> {
    if let Ok(url) = std::env::var("WAKUWAKU_LCU_URL") {
        return Some((url, format!("Basic {}", base64(b"riot:test"))));
    }
    let (port, password) = parse_lockfile(&std::fs::read_to_string(client_dir()?.join("lockfile")).ok()?)?;
    Some((format!("https://127.0.0.1:{port}"), format!("Basic {}", base64(format!("riot:{password}").as_bytes()))))
}

// The client's API is local, under Riot's own certificate (lol.rs agent):
// not checked. Riot's API is on the internet: checked, as ever.
fn local() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(3)))
        .tls_config(ureq::tls::TlsConfig::builder().disable_verification(true).build())
        .build()
        .new_agent()
}

fn lcu(agent: &ureq::Agent, (base, auth): &(String, String), path: &str) -> Option<Value> {
    agent.get(&format!("{base}{path}")).header("Authorization", auth.as_str()).call().ok()?.body_mut().read_json::<Value>().ok()
}

// Who she is (her Riot ID, name#tag: the client's PUUID is not the one
// Riot's API knows her by, scout::puuid_of), her platform on Riot's API,
// and the champions' names in the client's language (its game data: id to
// name): for the game's ten (scout.rs). None without the client.
pub fn me_platform_names() -> Option<(String, &'static str, HashMap<u64, String>)> {
    let c = client()?;
    let agent = local();
    let summoner = lcu(&agent, &c, "/lol-summoner/v1/current-summoner")?;
    let me = format!("{}#{}", summoner["gameName"].as_str()?, summoner["tagLine"].as_str()?);
    let platform = lcu(&agent, &c, "/riotclient/region-locale").and_then(|r| r["region"].as_str().and_then(platform_of))?;
    let names = lcu(&agent, &c, "/lol-game-data/assets/v1/champion-summary.json")
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|c| Some((c["id"].as_u64()?, c["name"].as_str()?.to_string())))
        .collect();
    Some((me, platform, names))
}

// What is kept from one look to the next: her party, and the ranks found.
#[derive(Default)]
pub struct Looking {
    party: HashMap<String, String>,
    me: (String, String),
    ranks: HashMap<String, Value>,
    platform: Option<&'static str>,
}

// One look at the client (every 5 s while it runs and no game is on).
pub fn look(sh: &Shared, l: &mut Looking) {
    let shown = match client() {
        Some(c) => look_at(sh, l, &c),
        None => {
            *l = Looking::default();
            None
        }
    };
    let changed = {
        let mut kept = SELECT.lock().unwrap_or_else(|e| e.into_inner());
        let changed = *kept != shown;
        *kept = shown;
        changed
    };
    if changed {
        sh.redraw();
    }
}

fn look_at(sh: &Shared, l: &mut Looking, c: &(String, String)) -> Option<Value> {
    let agent = local();
    let phase = lcu(&agent, c, "/lol-gameflow/v1/gameflow-phase")?;
    let phase = phase.as_str().unwrap_or("");
    if l.me.0.is_empty() {
        if let Some(me) = lcu(&agent, c, "/lol-summoner/v1/current-summoner") {
            let name = [me["gameName"].as_str().unwrap_or(""), me["tagLine"].as_str().unwrap_or("")].join("#");
            l.me = (me["puuid"].as_str().unwrap_or("").to_string(), name.trim_matches('#').to_string());
        }
        l.platform = lcu(&agent, c, "/riotclient/region-locale").and_then(|r| r["region"].as_str().and_then(platform_of));
    }
    // Her party, while there is a lobby (kept into champion select).
    if matches!(phase, "Lobby" | "Matchmaking" | "ReadyCheck" | "ChampSelect") {
        if let Some(lobby) = lcu(&agent, c, "/lol-lobby/v2/lobby") {
            l.party = lobby["members"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|m| {
                    let puuid = m["puuid"].as_str()?.to_string();
                    let name = [m["gameName"].as_str().unwrap_or(""), m["gameTag"].as_str().unwrap_or("")].join("#");
                    Some((puuid, name.trim_matches('#').to_string()))
                })
                .collect();
        }
    } else {
        l.party.clear();
    }
    if phase != "ChampSelect" {
        return None;
    }
    let session = lcu(&agent, c, "/lol-champ-select/v1/session")?;
    let key = sh.setting("riotApiKey").as_str().unwrap_or("").to_string();
    let mut players = Vec::new();
    for (puuid, position) in to_look_up(&session, &l.me.0, &l.party) {
        let me = puuid == l.me.0;
        let name = if me { l.me.1.clone() } else { l.party.get(&puuid).cloned().unwrap_or_default() };
        // Riot's API knows them by another PUUID than the client's: found by their Riot ID.
        if !key.is_empty() && !l.ranks.contains_key(&puuid) {
            let known = l.platform.and_then(|p| crate::scout::puuid_of(&key, p, &name));
            if let Some(entries) = known.and_then(|known| ranks(&key, l.platform, &known)) {
                l.ranks.insert(puuid.clone(), rank_of(&entries));
            }
        }
        players.push(json!({ "name": name, "me": me, "position": position, "rank": l.ranks.get(&puuid).cloned().unwrap_or(Value::Null) }));
    }
    Some(json!({ "players": players, "keyless": key.is_empty() }))
}

// league-v4's entries for one PUUID, with her own key. WAKUWAKU_RIOT_API_URL
// (tests) stands for Riot's API.
fn ranks(key: &str, platform: Option<&str>, puuid: &str) -> Option<Value> {
    let base = match std::env::var("WAKUWAKU_RIOT_API_URL") {
        Ok(url) => url,
        Err(_) => format!("https://{}.api.riotgames.com", platform?),
    };
    // In turn with the game's ten (scout.rs): the same key's limits.
    crate::scout::riot(key, &format!("{base}/lol/league/v4/entries/by-puuid/{puuid}")).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lockfile_and_the_region() {
        assert_eq!(parse_lockfile("LeagueClient:12345:54321:s3cr3t:https\n"), Some((54321, "s3cr3t".into())));
        assert_eq!(parse_lockfile("nonsense"), None);
        assert_eq!((platform_of("TW"), platform_of("euw"), platform_of("XX")), (Some("tw2"), Some("euw1"), None));
        // As a client in Taiwan says it.
        assert_eq!((platform_of("TW2"), platform_of("KR")), (Some("tw2"), Some("kr")));
        assert_eq!(base64(b"riot:s3cr3t"), "cmlvdDpzM2NyM3Q=");
        assert_eq!(base64(b"riot:ab"), "cmlvdDphYg==");
    }

    #[test]
    fn only_she_and_her_party_are_looked_up() {
        // Solo/Duo with a duo: two of her team hidden (no PUUID), one not in her party.
        let session = json!({ "myTeam": [
            { "puuid": "me", "assignedPosition": "middle" },
            { "puuid": "duo", "assignedPosition": "utility" },
            { "puuid": "", "assignedPosition": "top" },
            { "puuid": "", "assignedPosition": "jungle" },
            { "puuid": "stranger", "assignedPosition": "bottom" },
        ] });
        let party = HashMap::from([("duo".to_string(), "Duo#TW1".to_string())]);
        assert_eq!(to_look_up(&session, "me", &party), vec![("me".into(), "middle".into()), ("duo".into(), "utility".into())]);
    }

    #[test]
    fn her_solo_rank_else_her_flex() {
        let solo = json!({ "queueType": "RANKED_SOLO_5x5", "tier": "GOLD", "rank": "II", "leaguePoints": 45, "wins": 12, "losses": 10 });
        let flex = json!({ "queueType": "RANKED_FLEX_SR", "tier": "SILVER", "rank": "I", "leaguePoints": 3, "wins": 2, "losses": 1 });
        assert_eq!(rank_of(&json!([flex.clone(), solo])), json!({ "queue": "solo", "tier": "GOLD", "rank": "II", "lp": 45, "wins": 12, "losses": 10 }));
        assert_eq!(rank_of(&json!([flex]))["queue"], "flex");
        assert_eq!(rank_of(&json!([])), Value::Null);
    }
}
