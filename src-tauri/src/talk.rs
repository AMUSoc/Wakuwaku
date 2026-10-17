// A session's whole conversation for the phone (GET /mobile/api/conversation),
// and the phone's words added to it (POST /mobile/api/say).
//
// The conversation is read from the agent's own transcript (the path its hook
// events give), never from a terminal's screen:
//
//   claude  the jsonl is a tree (parentUuid): from its latest line back to the
//           root, past a compaction by logicalParentUuid; the person's words
//           (not tool results, not commands' own lines) and the text blocks
//   codex   response_item messages, the person's input_text (not the
//           <environment_context> and the like it sends first) and output_text
//   grok    user_message_chunk and agent_message_chunk, in order
//   agy     USER_INPUT (inside <USER_REQUEST>) and PLANNER_RESPONSE content
//
// Words go on only where they cannot fork the session (tried 2026-10-17,
// README): with the session open on the computer, Claude Code, Grok and
// Antigravity each run a second agent on the same id that the open one never
// sees, so then the phone only watches; Codex has a queue into a live
// session (`codex queue`). Not open: each agent's resume in its
// non-interactive mode (run.rs), in the folder the session works in.
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

// The most of a transcript read (its end), and the messages sent (each cut
// as a reply is, reply.rs MAX_CHARS).
const MAX_FILE: u64 = 32 * 1024 * 1024;
const MAX_MESSAGES: usize = 80;
// Conversations read lately, by file, its length and time: a page asking
// every few seconds does not read a long session's tens of MB each time.
type Kept = (String, u64, Option<std::time::SystemTime>, Vec<Value>, bool);
static READ: Mutex<Vec<Kept>> = Mutex::new(Vec::new());

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

#[derive(Debug, PartialEq)]
pub struct Message {
    pub role: &'static str,
    pub text: String,
}

// The file, whole, or its last MAX_FILE (then: true).
fn read(path: &Path) -> Option<(String, bool)> {
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let from = len.saturating_sub(MAX_FILE);
    f.seek(SeekFrom::Start(from)).ok()?;
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes).ok()?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    Some(if from > 0 { (text.split_once('\n').map_or(String::new(), |(_, r)| r.to_string()), true) } else { (text, false) })
}

fn rows(text: &str) -> Vec<Value> {
    text.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).collect()
}

fn block_texts(content: &Value, kinds: &[&str]) -> String {
    match content {
        Value::String(t) => t.clone(),
        Value::Array(blocks) => blocks.iter().filter(|b| kinds.contains(&s(b, "type"))).map(|b| s(b, "text").trim().to_string()).filter(|t| !t.is_empty()).collect::<Vec<_>>().join("\n\n"),
        _ => String::new(),
    }
}

// What the harness puts in the person's place: a command and its output, a
// reminder, Codex's context, an interruption.
const NOT_TYPED: &[&str] = &[
    "<command-",
    "<local-command",
    "<system-reminder",
    "<bash-",
    "<user-prompt-submit-hook",
    "<environment_context",
    "<user_instructions",
    "<turn_aborted",
    "Caveat:",
    "[Request interrupted",
];

// What the person typed, not what a tool, a command or the harness put there.
fn is_persons(text: &str) -> bool {
    let t = text.trim_start();
    !t.is_empty() && !NOT_TYPED.iter().any(|p| t.starts_with(p))
}

fn push(out: &mut Vec<Message>, role: &'static str, text: String) {
    let text = text.trim().to_string();
    if text.is_empty() {
        return;
    }
    match out.last_mut() {
        // One turn's text blocks come on several lines: one message.
        Some(last) if last.role == role => {
            last.text.push_str("\n\n");
            last.text.push_str(&text);
        }
        _ => out.push(Message { role, text }),
    }
}

fn claude(text: &str) -> Vec<Message> {
    let all = rows(text);
    let by_id: HashMap<&str, &Value> = all.iter().filter_map(|r| Some((r.get("uuid")?.as_str()?, r))).collect();
    let is_talk = |r: &Value| matches!(s(r, "type"), "user" | "assistant") && r.get("isSidechain") != Some(&Value::Bool(true));
    // The latest line of the conversation: where it is now.
    let Some(leaf) = all.iter().rev().find(|r| is_talk(r)) else { return Vec::new() };
    let mut chain = Vec::new();
    let mut cur = Some(leaf);
    let mut seen = 0;
    while let Some(r) = cur {
        seen += 1;
        if seen > 200_000 {
            break;
        }
        if is_talk(r) {
            chain.push(r);
        }
        let parent = r.get("parentUuid").and_then(Value::as_str).or_else(|| r.get("logicalParentUuid").and_then(Value::as_str));
        cur = parent.and_then(|p| by_id.get(p).copied());
    }
    let mut out = Vec::new();
    for r in chain.into_iter().rev() {
        let content = &r["message"]["content"];
        if s(r, "type") == "user" {
            if r.get("isMeta") == Some(&Value::Bool(true)) {
                continue;
            }
            let said = block_texts(content, &["text"]);
            if is_persons(&said) {
                push(&mut out, "user", said);
            }
        } else {
            push(&mut out, "assistant", block_texts(content, &["text"]));
        }
    }
    out
}

fn codex(text: &str) -> Vec<Message> {
    let mut out = Vec::new();
    for r in rows(text) {
        let p = &r["payload"];
        if s(&r, "type") != "response_item" || s(p, "type") != "message" {
            continue;
        }
        match s(p, "role") {
            "user" => {
                let said = block_texts(&p["content"], &["input_text"]);
                if is_persons(&said) && !said.starts_with("# AGENTS.md") {
                    push(&mut out, "user", said);
                }
            }
            "assistant" => push(&mut out, "assistant", block_texts(&p["content"], &["output_text"])),
            _ => {}
        }
    }
    out
}

fn grok(text: &str) -> Vec<Message> {
    let mut out: Vec<Message> = Vec::new();
    let mut last_role = "";
    for r in rows(text) {
        let u = &r["params"]["update"];
        let role = match s(u, "sessionUpdate") {
            "user_message_chunk" => "user",
            "agent_message_chunk" => "assistant",
            "tool_call" => {
                last_role = "";
                continue;
            }
            _ => continue,
        };
        let piece = s(&u["content"], "text");
        // Chunks of one message: joined as they came, not as paragraphs.
        match out.last_mut() {
            Some(m) if m.role == role && last_role == role => m.text.push_str(piece),
            Some(m) if m.role == role => {
                m.text.push_str("\n\n");
                m.text.push_str(piece);
            }
            _ => out.push(Message { role, text: piece.to_string() }),
        }
        last_role = role;
    }
    out.retain(|m| !m.text.trim().is_empty());
    for m in &mut out {
        m.text = m.text.trim().to_string();
    }
    out
}

fn agy(text: &str) -> Vec<Message> {
    let mut out = Vec::new();
    for step in rows(text) {
        match s(&step, "type") {
            "USER_INPUT" => {
                let c = s(&step, "content");
                let said = c.split_once("<USER_REQUEST>").and_then(|(_, r)| r.split_once("</USER_REQUEST>")).map_or(c, |(said, _)| said);
                push(&mut out, "user", said.to_string());
            }
            "PLANNER_RESPONSE" => push(&mut out, "assistant", s(&step, "content").to_string()),
            _ => {}
        }
    }
    out
}

pub fn from_text(agent: &str, text: &str) -> Vec<Message> {
    match agent {
        "codex" => codex(text),
        "grok" => grok(text),
        "agy" => agy(text),
        _ => claude(text),
    }
}

// The conversation, its last MAX_MESSAGES each cut at MAX_CHARS, and whether
// anything was left out; none when there is no transcript to read.
pub fn conversation(agent: &str, path: &str) -> Option<(Vec<Value>, bool)> {
    let file = Path::new(path);
    if file.extension().and_then(|e| e.to_str()) != Some("jsonl") {
        return None;
    }
    let meta = std::fs::metadata(file).ok()?;
    let (len, at) = (meta.len(), meta.modified().ok());
    if let Some(hit) = READ.lock().unwrap().iter().find(|r| r.0 == path && r.1 == len && r.2 == at) {
        return Some((hit.3.clone(), hit.4));
    }
    let got = read_conversation(agent, file)?;
    let mut kept = READ.lock().unwrap();
    kept.retain(|r| r.0 != path);
    kept.push((path.to_string(), len, at, got.0.clone(), got.1));
    if kept.len() > 8 {
        kept.remove(0);
    }
    Some(got)
}

fn read_conversation(agent: &str, path: &Path) -> Option<(Vec<Value>, bool)> {
    let (text, cut) = read(path)?;
    let all = from_text(agent, &text);
    let left_out = all.len().saturating_sub(MAX_MESSAGES);
    let shown = all
        .into_iter()
        .skip(left_out)
        .map(|m| {
            let (text, clipped) = crate::reply::clip(&m.text);
            json!({ "role": m.role, "text": text, "clipped": clipped })
        })
        .collect();
    Some((shown, cut || left_out > 0))
}

// --- Whether it is open on the computer ---------------------------------------------

// Claude Code's own list of its sessions (`claude agents --json`): ids of
// those running, interactive or in the background. Asked at most every 3 s.
static CLAUDE_LIVE: Mutex<Option<(Instant, Option<Vec<String>>)>> = Mutex::new(None);

fn claude_live() -> Option<Vec<String>> {
    let mut kept = CLAUDE_LIVE.lock().unwrap();
    if let Some((at, ids)) = kept.as_ref() {
        if at.elapsed() < Duration::from_secs(3) {
            return ids.clone();
        }
    }
    let ids = crate::run::find("claude").and_then(|exe| {
        let mut cmd = std::process::Command::new(exe);
        cmd.args(["agents", "--json"]).stdin(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        let out = cmd.output().ok()?;
        let list: Value = serde_json::from_slice(&out.stdout).ok()?;
        Some(list.as_array()?.iter().filter_map(|a| a.get("sessionId")?.as_str().map(str::to_string)).collect())
    });
    *kept = Some((Instant::now(), ids.clone()));
    ids
}

fn home() -> std::path::PathBuf {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(std::path::PathBuf::from).unwrap_or_default()
}

// Grok's own list of the sessions open (~/.grok/active_sessions.json:
// session_id, pid, cwd, opened_at), its process still running.
fn grok_live(session: &str) -> bool {
    let dir = std::env::var_os("GROK_HOME").map(std::path::PathBuf::from).unwrap_or_else(|| home().join(".grok"));
    let Ok(text) = std::fs::read_to_string(dir.join("active_sessions.json")) else { return false };
    let Ok(Value::Array(open)) = serde_json::from_str::<Value>(&text) else { return false };
    open.iter().any(|o| s(o, "session_id") == session && o.get("pid").and_then(Value::as_u64).and_then(|p| u32::try_from(p).ok()).is_some_and(crate::jump::is_running))
}

// Antigravity holds presence/<conversation>.lock while the conversation is
// open (the file stays after): one it will not let us have to ourselves.
fn agy_live(session: &str) -> bool {
    let lock = home().join(".gemini").join("antigravity-cli").join("presence").join(format!("{session}.lock"));
    if !lock.is_file() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        std::fs::OpenOptions::new().read(true).write(true).share_mode(0).open(&lock).is_err()
    }
    #[cfg(not(windows))]
    false
}

// Open on the computer: by each agent's own word where it gives one
// (Claude Code's list, Grok's open sessions, Antigravity's lock), or the
// process its hooks came from still running. A session opened again sends
// no hook event until its first turn, so the last alone lags (Codex's
// guards itself: run.rs).
pub fn is_live(agent: &str, session: &str, chain: &crate::jump::Chain) -> bool {
    let own = match agent {
        "claude" => claude_live().map(|ids| ids.iter().any(|i| i == session)),
        "grok" => Some(grok_live(session)),
        "agy" => Some(agy_live(session)),
        _ => None,
    };
    if agent == "claude" {
        if let Some(open) = own {
            return open;
        }
    }
    own == Some(true) || chain.first().is_some_and(|&(pid, at)| crate::jump::is_alive(pid, at))
}

// Whether the phone may add to it now, and how: Some("resume") or
// Some("queue"), or None with why.
pub fn way(agent: &str, live: bool, busy: bool, has_cwd: bool) -> Result<&'static str, &'static str> {
    if busy {
        return Err("busy");
    }
    if live {
        return if agent == "codex" { Ok("queue") } else { Err("live") };
    }
    if !has_cwd {
        return Err("noCwd");
    }
    Ok("resume")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jsonl(lines: &[Value]) -> String {
        lines.iter().map(Value::to_string).collect::<Vec<_>>().join("\n")
    }

    fn talk(v: &[Message]) -> Vec<(&str, &str)> {
        v.iter().map(|m| (m.role, m.text.as_str())).collect()
    }

    // Two lines from one point (a resume while it was open): the latest line's way.
    #[test]
    fn claude_s_conversation_follows_the_latest_branch() {
        let t = jsonl(&[
            json!({ "type": "user", "uuid": "u1", "parentUuid": null, "message": { "content": "Remember APPLE" } }),
            json!({ "type": "assistant", "uuid": "a1", "parentUuid": "u1", "message": { "content": [{ "type": "text", "text": "OK" }] } }),
            json!({ "type": "attachment", "uuid": "x1", "parentUuid": "a1" }),
            json!({ "type": "user", "uuid": "u2", "parentUuid": "x1", "message": { "content": "Branch one" } }),
            json!({ "type": "assistant", "uuid": "a2", "parentUuid": "u2", "message": { "content": [{ "type": "text", "text": "one" }] } }),
            json!({ "type": "user", "uuid": "u3", "parentUuid": "a1", "message": { "content": "Branch two" } }),
            json!({ "type": "assistant", "uuid": "a3", "parentUuid": "u3", "message": { "content": [{ "type": "tool_use", "name": "Bash" }] } }),
            json!({ "type": "user", "uuid": "u4", "parentUuid": "a3", "message": { "content": [{ "type": "tool_result", "content": "ok" }] } }),
            json!({ "type": "assistant", "uuid": "a4", "parentUuid": "u4", "message": { "content": [{ "type": "text", "text": "## Two\n\n- done" }] } }),
            json!({ "type": "assistant", "uuid": "a5", "parentUuid": "a4", "message": { "content": [{ "type": "text", "text": "PONG" }] } }),
            json!({ "type": "user", "uuid": "m1", "parentUuid": "a5", "isMeta": true, "message": { "content": "<command-name>/exit</command-name>" } }),
        ]);
        assert_eq!(talk(&from_text("claude", &t)), vec![("user", "Remember APPLE"), ("assistant", "OK"), ("user", "Branch two"), ("assistant", "## Two\n\n- done\n\nPONG")]);
    }

    #[test]
    fn claude_s_conversation_goes_on_past_a_compaction() {
        let t = jsonl(&[
            json!({ "type": "user", "uuid": "u1", "parentUuid": null, "message": { "content": "before" } }),
            json!({ "type": "system", "uuid": "c1", "parentUuid": null, "logicalParentUuid": "u1", "subtype": "compact_boundary" }),
            json!({ "type": "user", "uuid": "u2", "parentUuid": "c1", "message": { "content": "after" } }),
        ]);
        assert_eq!(talk(&from_text("claude", &t)), vec![("user", "before\n\nafter")]);
    }

    #[test]
    fn codex_s_messages_without_what_it_sends_first() {
        let item = |role: &str, kind: &str, text: &str| json!({ "type": "response_item", "payload": { "type": "message", "role": role, "content": [{ "type": kind, "text": text }] } });
        let t = jsonl(&[
            item("user", "input_text", "<environment_context>cwd</environment_context>"),
            item("user", "input_text", "# AGENTS.md instructions"),
            item("user", "input_text", "Remember APPLE"),
            item("assistant", "output_text", "OK"),
            json!({ "type": "event_msg", "payload": { "type": "user_message", "message": "Remember APPLE" } }),
            item("user", "input_text", "Again"),
            item("assistant", "output_text", "APPLE"),
        ]);
        assert_eq!(talk(&from_text("codex", &t)), vec![("user", "Remember APPLE"), ("assistant", "OK"), ("user", "Again"), ("assistant", "APPLE")]);
    }

    #[test]
    fn grok_s_chunks_joined_into_messages() {
        let c = |k: &str, t: &str| json!({ "params": { "update": { "sessionUpdate": k, "content": { "type": "text", "text": t } } } });
        let t = jsonl(&[
            c("user_message_chunk", "Remember APPLE"),
            c("agent_message_chunk", "O"),
            c("agent_message_chunk", "K"),
            c("user_message_chunk", "Look"),
            c("agent_message_chunk", "Looking."),
            json!({ "params": { "update": { "sessionUpdate": "tool_call" } } }),
            c("agent_message_chunk", "Found it."),
        ]);
        assert_eq!(talk(&from_text("grok", &t)), vec![("user", "Remember APPLE"), ("assistant", "OK"), ("user", "Look"), ("assistant", "Looking.\n\nFound it.")]);
    }

    #[test]
    fn antigravity_s_steps() {
        let t = jsonl(&[
            json!({ "type": "USER_INPUT", "content": "<USER_REQUEST>Remember APPLE</USER_REQUEST><ADDITIONAL_METADATA>x</ADDITIONAL_METADATA>" }),
            json!({ "type": "PLANNER_RESPONSE", "content": "" }),
            json!({ "type": "PLANNER_RESPONSE", "content": "OK" }),
        ]);
        assert_eq!(talk(&from_text("agy", &t)), vec![("user", "Remember APPLE"), ("assistant", "OK")]);
    }

    #[test]
    fn the_phone_adds_only_where_it_cannot_fork() {
        assert_eq!(way("claude", false, false, true), Ok("resume"));
        assert_eq!(way("claude", true, false, true), Err("live"));
        assert_eq!(way("grok", true, false, true), Err("live"));
        assert_eq!(way("agy", true, false, true), Err("live"));
        assert_eq!(way("codex", true, false, true), Ok("queue"));
        assert_eq!(way("codex", false, false, true), Ok("resume"));
        assert_eq!(way("codex", true, true, true), Err("busy"));
        assert_eq!(way("claude", false, false, false), Err("noCwd"));
    }
}
