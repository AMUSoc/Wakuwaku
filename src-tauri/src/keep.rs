// The sessions kept across her restarts. Held only in memory, a restart (a
// new version, a crash, her quit and opened again) forgot every one, and a
// session that sent nothing since stayed gone until its next event: a Grok
// turn waiting on a task of its own sends none for as long as it waits
// (2026-10-20). So they are written to sessions.json now and then and as
// she quits, and read back as she starts (state.rs snapshot, restore).
//
// Grok's open sessions are looked for too, in its own files: one opened
// while she was not running, or before she first kept any, comes back with
// its folder and whether a turn is under way (its log's last word on it).
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::{json, Value};

use crate::{jump, now_ms, Shared};

const FILE: &str = "sessions.json";
// Of Grok's log, only its end is read: the last few MB hold every open
// session's latest turn.
const GROK_LOG_TAIL: u64 = 4 * 1024 * 1024;

// What was written last, so the same is not written again every few seconds.
static WRITTEN: Mutex<String> = Mutex::new(String::new());

// Written as they are now, if they changed since.
pub fn save(sh: &Shared) {
    let text = sh.pet.lock().unwrap().snapshot().to_string();
    let mut written = WRITTEN.lock().unwrap();
    if *written == text {
        return;
    }
    let path = sh.dir.join(FILE);
    let part = sh.dir.join(format!("{FILE}.part"));
    if fs::write(&part, &text).is_ok() && fs::rename(&part, &path).is_ok() {
        *written = text;
    }
}

// Read back as she starts: those kept, then Grok's open ones she did not keep.
pub fn restore(sh: &Shared) {
    let now = now_ms();
    let kept = fs::read_to_string(sh.dir.join(FILE)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).unwrap_or(Value::Null);
    let back = sh.pet.lock().unwrap().restore(&kept, now, jump::is_alive);
    // Her own runs (reviews, OP.GG) were kept too before they were left out.
    let hers = sh.dir.join("lol").to_string_lossy().into_owned();
    let dropped = sh.pet.lock().unwrap().forget_where(|cwd| crate::server::is_under(cwd, &hers));
    let back = back - dropped.min(back);
    let mut grok = 0;
    for msg in grok_open() {
        let id = msg["session"].as_str().unwrap_or("");
        let hold = sh.hold();
        let mut pet = sh.pet.lock().unwrap();
        if !pet.knows(id) && pet.apply(&msg, now, hold).is_some() {
            grok += 1;
        }
    }
    sh.log(&format!("sessions: {back} kept from before, {grok} of Grok's open found"));
    // Kept as read, not written again for nothing.
    let text = sh.pet.lock().unwrap().snapshot().to_string();
    *WRITTEN.lock().unwrap() = text;
}

// A Grok turn that sends nothing for a long while (waiting on a task of its
// own) is not given up on at the 15 minutes (state.rs check_stale) while
// Grok's log says it is still under way. Looked at each minute, only with
// one at work.
pub fn keep_grok_busy(sh: &Shared) {
    let busy = sh.pet.lock().unwrap().grok_busy();
    if busy.is_empty() {
        return;
    }
    let open: Vec<String> = grok_open().into_iter().filter(|m| m["mood"] == "working").filter_map(|m| m["session"].as_str().map(str::to_string)).collect();
    let still: Vec<String> = busy.into_iter().filter(|id| open.contains(id)).collect();
    if !still.is_empty() {
        sh.pet.lock().unwrap().still_busy(&still, now_ms());
    }
}

fn grok_home() -> PathBuf {
    std::env::var_os("GROK_HOME").map(PathBuf::from).unwrap_or_else(|| {
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_default();
        home.join(".grok")
    })
}

// Grok's sessions open now (~/.grok/active_sessions.json, its process still
// running), as messages: its folder, its process, at work if its log says a
// turn began and has not ended.
fn grok_open() -> Vec<Value> {
    let dir = grok_home();
    let Ok(text) = fs::read_to_string(dir.join("active_sessions.json")) else { return Vec::new() };
    let Ok(Value::Array(open)) = serde_json::from_str::<Value>(&text) else { return Vec::new() };
    let log = log_tail(&dir.join("logs").join("unified.jsonl"));
    open.iter()
        .filter_map(|o| {
            let id = o.get("session_id")?.as_str()?;
            let pid = u32::try_from(o.get("pid")?.as_u64()?).ok()?;
            if !jump::is_running(pid) {
                return None;
            }
            let cwd = o.get("cwd").and_then(Value::as_str).unwrap_or("");
            let working = is_turn_open(&log, id, pid);
            Some(json!({
                "session": id,
                "agent": "grok",
                "project": crate::events::project_of(cwd),
                "cwd": cwd,
                "mood": if working { "working" } else { "idle" },
                "event": if working { "turn-start" } else { "" },
                "chain": jump::chain_json(&jump::chain_of(pid)),
            }))
        })
        .collect()
}

fn log_tail(path: &PathBuf) -> String {
    let Ok(mut f) = fs::File::open(path) else { return String::new() };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let _ = f.seek(SeekFrom::Start(len.saturating_sub(GROK_LOG_TAIL)));
    let mut bytes = Vec::new();
    let _ = f.read_to_end(&mut bytes);
    String::from_utf8_lossy(&bytes).into_owned()
}

// A turn of this session (of this process) began and has not yet ended, as
// Grok's own log last says: "prompt received" starts one, "turn.complete"
// ends it.
fn is_turn_open(log: &str, id: &str, pid: u32) -> bool {
    let mut open = false;
    for line in log.lines() {
        if !line.contains(id) {
            continue;
        }
        let Ok(e) = serde_json::from_str::<Value>(line) else { continue };
        if e["sid"] != id || e["pid"].as_u64() != Some(pid as u64) {
            continue;
        }
        match e["msg"].as_str().unwrap_or("") {
            "prompt received" => open = true,
            "turn.complete" => open = false,
            _ => {}
        }
    }
    open
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_turn_is_open_from_its_prompt_to_its_end() {
        let log = [
            r#"{"pid":7,"sid":"a","msg":"prompt received"}"#,
            r#"{"pid":7,"sid":"a","msg":"turn.complete"}"#,
            r#"{"pid":7,"sid":"a","msg":"prompt received"}"#,
            r#"{"pid":7,"sid":"a","msg":"turn.phase_transition"}"#,
            r#"{"pid":8,"sid":"b","msg":"prompt received"}"#,
            r#"{"pid":8,"sid":"b","msg":"turn.complete"}"#,
        ]
        .join("\n");
        assert!(super::is_turn_open(&log, "a", 7));
        assert!(!super::is_turn_open(&log, "b", 8));
        // Another process of the same session (resumed elsewhere): not this one's.
        assert!(!super::is_turn_open(&log, "a", 9));
    }
}
