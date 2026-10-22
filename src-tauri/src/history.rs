// Every conversation the four agents keep on this computer, for the phone's
// list (GET /mobile/api/history), as ChatGPT's or Claude's apps list theirs:
// its title, agent, project and when, the latest first, found by words. The
// user could not find an earlier conversation again from the phone: only the
// few she had seen were there (2026-10-22). Read from the agents' own files:
//
//   Claude Code  <config>/projects/<project>/<id>.jsonl: its ai-title (or
//                custom-title), else the first thing asked; cwd on its rows
//   Codex        <home>/sessions/**/rollout-*.jsonl: session_meta (id, cwd),
//                the first thing asked
//   Grok         ~/.grok/sessions/<cwd>/<id>/summary.json (id, cwd, its
//                session_summary); the conversation in updates.jsonl
//   Antigravity  <gemini>/<product>/brain/<id>/.system_generated/logs/
//                transcript.jsonl: the first USER_REQUEST
//
// A file read once is kept until it changes (its time); the folders looked
// through again at most every 15 seconds. Her own runs (data/lol, duel, ask)
// are left out, as on the island.
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use serde_json::{json, Value};

use crate::Shared;

#[derive(Clone, Debug)]
pub struct Entry {
    pub id: String,
    pub agent: &'static str,
    // The conversation as talk.rs reads it.
    pub transcript: String,
    pub cwd: String,
    pub title: String,
    pub at: u64,
    // What it is found by: its title and what was asked, lowercased.
    search: String,
}

struct Index {
    files: HashMap<PathBuf, (SystemTime, Option<Entry>)>,
    looked: Option<Instant>,
}

static INDEX: Mutex<Option<Index>> = Mutex::new(None);
const LOOK_EVERY: Duration = Duration::from_secs(15);
// So much of a file's start read for what was asked; of what was asked, so much kept to search.
const HEAD: u64 = 256 * 1024;
const SEARCH_KEPT: usize = 4000;

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

fn head(path: &Path) -> String {
    let Ok(mut f) = std::fs::File::open(path) else { return String::new() };
    let mut bytes = Vec::new();
    let _ = (&mut f).take(HEAD).read_to_end(&mut bytes);
    // A cut line at the end is not a row.
    let text = String::from_utf8_lossy(&bytes).into_owned();
    match text.rfind('\n') {
        Some(at) if (bytes.len() as u64) >= HEAD => text[..at].to_string(),
        _ => text,
    }
}

// What a person typed, not what a tool or the harness put in their place.
fn typed(text: &str) -> Option<String> {
    let t = text.trim();
    let harness = t.starts_with('<') || t.starts_with("Caveat:") || t.starts_with("[Request interrupted") || t.starts_with("# AGENTS.md");
    (!t.is_empty() && !harness).then(|| t.to_string())
}

fn texts(content: &Value, kinds: &[&str]) -> String {
    match content {
        Value::String(t) => t.clone(),
        Value::Array(blocks) => blocks.iter().filter(|b| kinds.contains(&s(b, "type"))).map(|b| s(b, "text")).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}

// The first line of what was asked, as a title.
fn first_line(text: &str) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    crate::events::clip(line, 80)
}

// How her own runs begin (review.rs, duel.rs, ask.rs): Antigravity's
// records say no folder, so they are known by their words.
const HER_PROMPTS: [&str; 3] = ["你是英雄聯盟教練", "你是遊戲王 Master Duel 的對戰助手", "你是使用者桌面上的小助手"];

fn finish(id: String, agent: &'static str, transcript: &Path, cwd: String, title: Option<String>, asked: Vec<String>, at: u64) -> Option<Entry> {
    if asked.first().is_some_and(|a| HER_PROMPTS.iter().any(|p| a.starts_with(p))) {
        return None;
    }
    // A title given empty (Grok's, before its first summary) is none.
    let title = title.filter(|t| !t.trim().is_empty());
    if id.is_empty() || (title.is_none() && asked.is_empty()) {
        return None;
    }
    let title = title.unwrap_or_else(|| first_line(&asked[0]));
    let mut search = title.to_lowercase();
    for a in &asked {
        if search.len() > SEARCH_KEPT {
            break;
        }
        search.push('\n');
        search.push_str(&a.to_lowercase());
    }
    Some(Entry { id, agent, transcript: transcript.to_string_lossy().into_owned(), cwd, title, at, search })
}

pub fn claude_entry(path: &Path, at: u64) -> Option<Entry> {
    let id = path.file_stem()?.to_string_lossy().into_owned();
    let mut cwd = String::new();
    let mut asked = Vec::new();
    for line in head(path).lines() {
        let Ok(row) = serde_json::from_str::<Value>(line) else { continue };
        if cwd.is_empty() {
            cwd = s(&row, "cwd").to_string();
        }
        if s(&row, "type") == "user" && row.get("isMeta") != Some(&Value::Bool(true)) && row.get("isSidechain") != Some(&Value::Bool(true)) {
            if let Some(t) = typed(&texts(&row["message"]["content"], &["text"])) {
                asked.push(t);
            }
        }
    }
    let title = crate::events::transcript_title(&path.to_string_lossy());
    finish(id, "claude", path, cwd, title, asked, at)
}

pub fn codex_entry(path: &Path, at: u64) -> Option<Entry> {
    let (mut id, mut cwd, mut asked) = (String::new(), String::new(), Vec::new());
    for line in head(path).lines() {
        let Ok(row) = serde_json::from_str::<Value>(line) else { continue };
        let p = &row["payload"];
        match s(&row, "type") {
            "session_meta" if id.is_empty() => {
                id = s(p, "id").to_string();
                cwd = s(p, "cwd").to_string();
            }
            "response_item" if s(p, "type") == "message" && s(p, "role") == "user" => {
                if let Some(t) = typed(&texts(&p["content"], &["input_text"])) {
                    asked.push(t);
                }
            }
            _ => {}
        }
    }
    finish(id, "codex", path, cwd, None, asked, at)
}

// Grok's summary.json: its conversation is updates.jsonl beside it.
pub fn grok_entry(summary: &Path, at: u64) -> Option<Entry> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(summary).ok()?).ok()?;
    let updates = summary.with_file_name("updates.jsonl");
    let mut asked = Vec::new();
    for line in head(&updates).lines() {
        let Ok(row) = serde_json::from_str::<Value>(line) else { continue };
        let u = &row["params"]["update"];
        if s(u, "sessionUpdate") == "user_message_chunk" {
            if let Some(t) = typed(s(&u["content"], "text")) {
                asked.push(t);
            }
        }
    }
    let title = Some(s(&v, "session_summary").to_string());
    finish(s(&v["info"], "id").to_string(), "grok", &updates, s(&v["info"], "cwd").to_string(), title, asked, at)
}

pub fn agy_entry(transcript: &Path, at: u64) -> Option<Entry> {
    // brain/<id>/.system_generated/logs/transcript.jsonl
    let id = transcript.parent()?.parent()?.parent()?.file_name()?.to_string_lossy().into_owned();
    let mut asked = Vec::new();
    for line in head(transcript).lines() {
        let Ok(step) = serde_json::from_str::<Value>(line) else { continue };
        if s(&step, "type") == "USER_INPUT" {
            let c = s(&step, "content");
            let said = c.split_once("<USER_REQUEST>").and_then(|(_, r)| r.split_once("</USER_REQUEST>")).map_or(c, |(said, _)| said);
            if let Some(t) = typed(said) {
                asked.push(t);
            }
        }
    }
    finish(id, "agy", transcript, String::new(), None, asked, at)
}

// The files to read, by agent: (file, how it is read).
type Reader = fn(&Path, u64) -> Option<Entry>;

fn files() -> Vec<(PathBuf, Reader)> {
    let mut out: Vec<(PathBuf, Reader)> = Vec::new();
    let list = |dir: &Path| std::fs::read_dir(dir).map(|r| r.flatten().map(|e| e.path()).collect::<Vec<_>>()).unwrap_or_default();
    for project in list(&crate::tokens::claude_dir()) {
        for f in list(&project) {
            if f.extension().is_some_and(|e| e == "jsonl") {
                out.push((f, claude_entry));
            }
        }
    }
    // Codex: sessions/<year>/<month>/<day>/rollout-*.jsonl
    for year in list(&crate::tokens::codex_dir()) {
        for month in list(&year) {
            for day in list(&month) {
                for f in list(&day) {
                    if f.extension().is_some_and(|e| e == "jsonl") {
                        out.push((f, codex_entry));
                    }
                }
            }
        }
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_default();
    let grok = std::env::var_os("GROK_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".grok")).join("sessions");
    for folder in list(&grok) {
        for session in list(&folder) {
            let summary = session.join("summary.json");
            if summary.is_file() {
                out.push((summary, grok_entry));
            }
        }
    }
    for brain in crate::tokens::agy_dirs() {
        for conv in list(&brain) {
            let t = conv.join(".system_generated").join("logs").join("transcript.jsonl");
            if t.is_file() {
                out.push((t, agy_entry));
            }
        }
    }
    out
}

fn ms(t: SystemTime) -> u64 {
    t.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

// The index, brought up to date if it was not lately.
fn with_index<T>(f: impl FnOnce(&Index) -> T) -> T {
    let mut guard = INDEX.lock().unwrap_or_else(|e| e.into_inner());
    let index = guard.get_or_insert_with(|| Index { files: HashMap::new(), looked: None });
    if index.looked.is_none_or(|t| t.elapsed() >= LOOK_EVERY) {
        let found = files();
        let mut kept = HashMap::with_capacity(found.len());
        for (path, read) in found {
            let Ok(time) = std::fs::metadata(&path).and_then(|m| m.modified()) else { continue };
            let entry = match index.files.remove(&path) {
                Some((was, entry)) if was == time => entry,
                _ => read(&path, ms(time)),
            };
            kept.insert(path, (time, entry));
        }
        index.files = kept;
        index.looked = Some(Instant::now());
    }
    f(index)
}

fn is_hers(sh: &Shared, cwd: &str) -> bool {
    ["lol", "duel", "ask"].iter().any(|d| crate::server::is_under(cwd, &sh.dir.join(d).to_string_lossy()))
}

// The list: the latest first, of one agent or all, found by words, a page of it.
pub fn list(sh: &Shared, agent: &str, query: &str, offset: usize, limit: usize) -> Value {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let mut all: Vec<Entry> = with_index(|i| {
        i.files
            .values()
            .filter_map(|(_, e)| e.as_ref())
            .filter(|e| agent.is_empty() || e.agent == agent)
            .filter(|e| words.iter().all(|w| e.search.contains(w.as_str())))
            .cloned()
            .collect()
    });
    all.retain(|e| !is_hers(sh, &e.cwd));
    // A session in two files (Grok's summary, Claude's resumed): the latest.
    all.sort_by(|a, b| b.at.cmp(&a.at));
    let mut seen = std::collections::HashSet::new();
    all.retain(|e| seen.insert((e.agent, e.id.clone())));
    let total = all.len();
    let details = sh.setting("details") != false;
    let items: Vec<Value> = all
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|e| {
            let project = if e.cwd.is_empty() { String::new() } else { crate::events::project_of(&e.cwd) };
            json!({ "id": e.id, "agent": e.agent, "title": if details { e.title } else { project.clone() }, "project": project, "at": e.at })
        })
        .collect();
    json!({ "items": items, "total": total, "at": crate::now_ms() })
}

// One conversation of the list: what talk.rs needs of it.
pub fn find(id: &str) -> Option<Entry> {
    with_index(|i| i.files.values().filter_map(|(_, e)| e.as_ref()).filter(|e| e.id == id).max_by_key(|e| e.at).cloned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, rows: &[Value]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wakuwaku-history-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, rows.iter().map(Value::to_string).collect::<Vec<_>>().join("\n")).unwrap();
        path
    }

    #[test]
    fn each_agent_s_conversation_as_a_line_of_the_list() {
        let c = file(
            "abc-123.jsonl",
            &[
                json!({ "type": "user", "cwd": "C:\\w\\pet", "message": { "content": "<command-name>/clear</command-name>" } }),
                json!({ "type": "user", "cwd": "C:\\w\\pet", "message": { "content": [{ "type": "text", "text": "幫我修登入頁\n細節…" }] } }),
                json!({ "type": "ai-title", "aiTitle": "Login page fix" }),
            ],
        );
        let e = claude_entry(&c, 5).unwrap();
        assert_eq!((e.id.as_str(), e.agent, e.cwd.as_str(), e.title.as_str(), e.at), ("abc-123", "claude", "C:\\w\\pet", "Login page fix", 5));
        assert!(e.search.contains("幫我修登入頁"));
        let x = file(
            "rollout-1.jsonl",
            &[
                json!({ "type": "session_meta", "payload": { "id": "t-1", "cwd": "C:\\w\\x" } }),
                json!({ "type": "response_item", "payload": { "type": "message", "role": "user", "content": [{ "type": "input_text", "text": "# AGENTS.md\n…" }] } }),
                json!({ "type": "response_item", "payload": { "type": "message", "role": "user", "content": [{ "type": "input_text", "text": "say hi" }] } }),
            ],
        );
        let e = codex_entry(&x, 1).unwrap();
        assert_eq!((e.id.as_str(), e.title.as_str(), e.cwd.as_str()), ("t-1", "say hi", "C:\\w\\x"));
        // Nothing asked, no title: not listed.
        assert!(codex_entry(&file("rollout-2.jsonl", &[json!({ "type": "session_meta", "payload": { "id": "t-2" } })]), 1).is_none());
        // Her own runs: not listed.
        assert!(finish("a".into(), "agy", Path::new("t.jsonl"), String::new(), None, vec!["你是使用者桌面上的小助手。…".into()], 1).is_none());
        // An empty title and nothing asked (Grok's at first): not listed, nothing broken.
        assert!(finish("g".into(), "grok", Path::new("u.jsonl"), String::new(), Some(String::new()), Vec::new(), 1).is_none());
    }
}
