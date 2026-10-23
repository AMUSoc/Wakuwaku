// Her other computers (2026-10-24: the user's desktop at home and this
// laptop, each to show the other's agents too): each set by the address its
// phone page gives with its token (setting `peers`, the QR code's address
// pasted). Every 2 seconds each is asked for its own sessions and prompts
// (/mobile/api/state and /asks: what it tells the phone, its own alone, so
// two computers never pass each other's back); its sessions are shown here
// with its name (state.rs remote), its prompts among hers, an answer given
// here sent back to it. Gone three times running, its sessions go too.
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use crate::Shared;

const EVERY: Duration = Duration::from_secs(2);
const GONE_AFTER: u32 = 3;
// The ids this computer gives another's prompts: far above its own.
pub const FIRST: u64 = 1 << 40;
static NEXT: AtomicU64 = AtomicU64::new(FIRST);

// This computer's name, as the others and the phone show it.
pub fn machine() -> String {
    std::env::var("COMPUTERNAME").ok().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "?".into())
}

// Where a computer's phone page is, and its token: the address the QR code
// holds ("http://100.1.2.3:47213/mobile/?t=…"), or the page's address with
// "#token" after it.
pub fn parse(s: &str) -> Option<(String, String)> {
    let s = s.trim();
    if !crate::mobile::is_server(s) {
        return None;
    }
    let (address, token) = match s.split_once("?t=").or_else(|| s.split_once('#')) {
        Some((a, t)) => (a, t.split('&').next().unwrap_or("")),
        None => return None,
    };
    let root = address.trim_end_matches('/').trim_end_matches("/mobile");
    (!token.is_empty()).then(|| (format!("{root}/mobile/"), token.to_string()))
}

#[derive(Default)]
struct Peer {
    machine: String,
    misses: u32,
    asks: Vec<Value>,
}

// Each by its page's address.
static PEERS: Mutex<Option<HashMap<String, Peer>>> = Mutex::new(None);
// The id given here to another's prompt: its page, its token, its own id.
static IDS: Mutex<Option<HashMap<u64, (String, String, u64)>>> = Mutex::new(None);

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(4))).http_status_as_error(false).build().new_agent()
}

fn get(root: &str, token: &str, path: &str) -> Result<Value, String> {
    let mut res = agent().get(&format!("{root}{path}")).header("x-wakuwaku-token", token).call().map_err(|e| e.to_string())?;
    if res.status() != 200 {
        return Err(format!("{}", res.status()));
    }
    res.body_mut().read_json::<Value>().map_err(|e| e.to_string())
}

pub fn start(sh: &Arc<Shared>) {
    let sh = sh.clone();
    crate::spawn_named("peers", move || loop {
        std::thread::sleep(EVERY);
        let wanted: Vec<(String, String)> = sh.setting("peers").as_array().into_iter().flatten().filter_map(|p| parse(p.as_str()?)).collect();
        // Those no longer set: their sessions and prompts gone.
        let dropped: Vec<String> = {
            let mut peers = PEERS.lock().unwrap();
            let peers = peers.get_or_insert_with(HashMap::new);
            let gone: Vec<String> = peers.keys().filter(|k| !wanted.iter().any(|(r, _)| r == *k)).cloned().collect();
            gone.iter().filter_map(|k| peers.remove(k)).map(|p| p.machine).collect()
        };
        let mut changed = false;
        let mut asks_changed = false;
        for m in dropped.iter().filter(|m| !m.is_empty()) {
            changed |= sh.pet.lock().unwrap().set_remote(m, Vec::new()).0;
            asks_changed = true;
        }
        for (root, token) in &wanted {
            let (c, a) = look(&sh, root, token);
            changed |= c;
            asks_changed |= a;
        }
        if changed {
            sh.redraw();
        }
        if asks_changed {
            sh.push_asks();
        }
    });
}

// One computer asked: whether its sessions, its prompts changed.
fn look(sh: &Shared, root: &str, token: &str) -> (bool, bool) {
    let got = get(root, token, "api/state").and_then(|state| Ok((state, get(root, token, "api/asks")?)));
    let mine = machine();
    let (machine, sessions, asks) = match got {
        // Not itself (its own address set by mistake).
        Ok((state, _)) if state["machine"].as_str() == Some(mine.as_str()) => return (false, false),
        Ok((state, asks)) => {
            let machine = state["machine"].as_str().filter(|m| !m.is_empty()).unwrap_or(root).to_string();
            (machine.clone(), sessions_of(&state, &machine), asks["asks"].as_array().cloned().unwrap_or_default())
        }
        Err(_) => {
            let mut peers = PEERS.lock().unwrap();
            let Some(p) = peers.get_or_insert_with(HashMap::new).get_mut(root) else { return (false, false) };
            p.misses += 1;
            if p.misses < GONE_AFTER || p.machine.is_empty() {
                return (false, false);
            }
            let had = !p.asks.is_empty();
            p.asks.clear();
            let machine = p.machine.clone();
            drop(peers);
            return (sh.pet.lock().unwrap().set_remote(&machine, Vec::new()).0, had);
        }
    };
    let asks: Vec<Value> = asks.into_iter().map(|a| ask_of(a, root, token, &machine)).collect();
    let asks_changed = {
        let mut peers = PEERS.lock().unwrap();
        let p = peers.get_or_insert_with(HashMap::new).entry(root.to_string()).or_default();
        p.misses = 0;
        p.machine = machine.clone();
        let changed = p.asks != asks;
        p.asks = asks;
        changed
    };
    let (changed, alerts) = sh.pet.lock().unwrap().set_remote(&machine, sessions);
    // Chimed here as this computer's own are.
    for (mood, project) in alerts {
        sh.alert(&mood, &format!("{machine}・{project}"));
    }
    (changed, asks_changed)
}

// Its sessions in sight as its pages draw them, named by it and marked
// as not here (no window to go to).
fn sessions_of(state: &Value, machine: &str) -> Vec<Value> {
    state["agents"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|a| a["list"].as_array().cloned().unwrap_or_default())
        .map(|mut s| {
            s["id"] = json!(format!("{machine}:{}", s["id"].as_str().unwrap_or("")));
            s["machine"] = json!(machine);
            s["jump"] = json!(false);
            s
        })
        .collect()
}

// Another's prompt with an id of this computer's (the same one each time it is seen).
fn ask_of(mut a: Value, root: &str, token: &str, machine: &str) -> Value {
    let theirs = a["id"].as_u64().unwrap_or(0);
    let id = {
        let mut ids = IDS.lock().unwrap();
        let ids = ids.get_or_insert_with(HashMap::new);
        match ids.iter().find(|(_, (r, _, t))| r == root && *t == theirs) {
            Some((id, _)) => *id,
            None => {
                let id = NEXT.fetch_add(1, Ordering::SeqCst);
                ids.insert(id, (root.to_string(), token.to_string(), theirs));
                // Kept for the last 200.
                if ids.len() > 200 {
                    let oldest = *ids.keys().min().unwrap_or(&id);
                    ids.remove(&oldest);
                }
                id
            }
        }
    };
    a["id"] = json!(id);
    a["machine"] = json!(machine);
    if let Some(s) = a["session"].as_str() {
        a["session"] = json!(format!("{machine}:{s}"));
    }
    a
}

// The other computers' prompts, for one agent's pet page or for the island.
pub fn asks_where(keep: impl Fn(&str) -> bool) -> Vec<Value> {
    PEERS.lock().unwrap().as_ref().into_iter().flat_map(|p| p.values()).flat_map(|p| p.asks.iter()).filter(|a| keep(crate::asks::agent_of(a))).cloned().collect()
}

// What the settings show: this computer's name, and each other one's and whether it answers.
pub fn view() -> Value {
    let list: Vec<Value> = PEERS.lock().unwrap().as_ref().into_iter().flat_map(|p| p.values()).map(|p| json!({ "machine": p.machine, "ok": !p.machine.is_empty() && p.misses < GONE_AFTER })).collect();
    json!({ "machine": machine(), "list": list })
}

pub fn is_theirs(id: u64) -> bool {
    id >= FIRST
}

// An answer to another computer's prompt, sent to it.
pub fn answer(id: u64, choice: &Value) -> bool {
    let Some((root, token, theirs)) = IDS.lock().unwrap().as_ref().and_then(|ids| ids.get(&id).cloned()) else { return false };
    let sent = agent()
        .post(&format!("{root}api/answer"))
        .header("x-wakuwaku-token", &token)
        .header("x-wakuwaku", "1")
        .send_json(json!({ "id": theirs, "choice": choice }));
    let ok = sent.ok().filter(|r| r.status() == 200).and_then(|mut r| r.body_mut().read_json::<Value>().ok()).is_some_and(|v| v["ok"] == true);
    if ok {
        // Gone from here at once, not at the next look.
        if let Some(peers) = PEERS.lock().unwrap().as_mut() {
            for p in peers.values_mut() {
                p.asks.retain(|a| a["id"].as_u64() != Some(id));
            }
        }
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_computer_by_its_phone_address() {
        assert_eq!(parse("http://100.1.2.3:47213/mobile/?t=abc"), Some(("http://100.1.2.3:47213/mobile/".into(), "abc".into())));
        assert_eq!(parse(" https://home.tail1.ts.net/mobile?t=x&y=1 "), Some(("https://home.tail1.ts.net/mobile/".into(), "x".into())));
        assert_eq!(parse("http://100.1.2.3:47213#tok"), Some(("http://100.1.2.3:47213/mobile/".into(), "tok".into())));
        assert_eq!(parse("http://100.1.2.3:47213/mobile/"), None, "no token");
        assert_eq!(parse("100.1.2.3:47213/mobile/?t=abc"), None);
    }

    #[test]
    fn its_sessions_named_by_it_and_not_to_be_gone_to() {
        let state = json!({ "agents": [
            { "agent": "claude", "list": [{ "id": "a1", "agent": "", "mood": "working", "jump": true }] },
            { "agent": "codex", "list": [] }
        ] });
        let s = sessions_of(&state, "DESKTOP-HOME");
        assert_eq!(s.len(), 1);
        assert_eq!((s[0]["id"].as_str(), s[0]["machine"].as_str(), s[0]["jump"].as_bool()), (Some("DESKTOP-HOME:a1"), Some("DESKTOP-HOME"), Some(false)));
    }

    #[test]
    fn its_prompts_keep_the_id_given_here() {
        let a = ask_of(json!({ "id": 7, "session": "s", "from": "codex" }), "http://x/mobile/", "t", "PC");
        let again = ask_of(json!({ "id": 7, "session": "s", "from": "codex" }), "http://x/mobile/", "t", "PC");
        let other = ask_of(json!({ "id": 7 }), "http://y/mobile/", "t", "PC2");
        assert!(is_theirs(a["id"].as_u64().unwrap()));
        assert_eq!(a["id"], again["id"]);
        assert_ne!(a["id"], other["id"]);
        assert_eq!((a["session"].as_str(), a["machine"].as_str()), (Some("PC:s"), Some("PC")));
    }
}
