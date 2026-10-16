// A session's last reply in full, from its agent's own transcript (the path
// its hook events give: transcript_path, Grok's and Antigravity's
// transcriptPath), for the phone (GET /mobile/api/reply). Only read, never
// written; the end of the file alone, and so much of the reply at most.
//
//   claude  ~/.claude/projects/<project>/<session>.jsonl: an assistant line's
//           text blocks; one reply may be several lines, ended by a user line
//   codex   ~/.codex/sessions/…/rollout-*.jsonl: a response_item message of
//           the assistant's, its output_text; else task_complete's
//           last_agent_message
//   grok    ~/.grok/sessions/<cwd>/<id>/updates.jsonl: agent_message_chunk
//           texts of the last turn (from its user_message_chunk), those after
//           its last tool call if there are any
//   agy     …/brain/<id>/.system_generated/logs/transcript.jsonl: the last
//           PLANNER_RESPONSE's content (events_agy.rs reads it so too)
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use serde_json::Value;

// The most of a reply sent, in characters.
pub const MAX_CHARS: usize = 4000;
// So much of a transcript's end is read: a turn is there, and a long
// session's file (tens of MB) is not read whole every time.
const TAIL: u64 = 2 * 1024 * 1024;

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

// The file's last lines, whole (the first, cut, left out).
fn tail(path: &Path) -> Option<String> {
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let from = len.saturating_sub(TAIL);
    f.seek(SeekFrom::Start(from)).ok()?;
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes).ok()?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    Some(if from > 0 { text.split_once('\n').map_or(String::new(), |(_, rest)| rest.to_string()) } else { text })
}

fn lines(text: &str) -> impl DoubleEndedIterator<Item = Value> + '_ {
    text.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok())
}

fn texts_of(content: &Value, kind: &str) -> Vec<String> {
    match content {
        Value::String(t) => vec![t.clone()],
        Value::Array(blocks) => blocks.iter().filter(|b| s(b, "type") == kind).map(|b| s(b, "text").to_string()).filter(|t| !t.trim().is_empty()).collect(),
        _ => Vec::new(),
    }
}

fn claude(text: &str) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    for line in lines(text).rev() {
        match s(&line, "type") {
            "assistant" => {
                let found = texts_of(&line["message"]["content"], "text");
                if !found.is_empty() {
                    parts.splice(0..0, found);
                }
            }
            // Before the reply's first line: the tool results it read, or the person.
            "user" if !parts.is_empty() => break,
            _ => {}
        }
    }
    (!parts.is_empty()).then(|| parts.join("\n\n"))
}

fn codex(text: &str) -> Option<String> {
    let mut last_message = None;
    for line in lines(text).rev() {
        let p = &line["payload"];
        if s(&line, "type") == "response_item" && s(p, "type") == "message" && s(p, "role") == "assistant" {
            let found = texts_of(&p["content"], "output_text");
            if !found.is_empty() {
                return Some(found.join("\n\n"));
            }
        }
        if last_message.is_none() && s(&line, "type") == "event_msg" && s(p, "type") == "task_complete" && !s(p, "last_agent_message").is_empty() {
            last_message = Some(s(p, "last_agent_message").to_string());
        }
    }
    last_message
}

fn grok(text: &str) -> Option<String> {
    let (mut turn, mut since_tool) = (String::new(), String::new());
    for line in lines(text) {
        let u = &line["params"]["update"];
        match s(u, "sessionUpdate") {
            "user_message_chunk" => {
                turn.clear();
                since_tool.clear();
            }
            "tool_call" => since_tool.clear(),
            "agent_message_chunk" => {
                let t = s(&u["content"], "text");
                turn.push_str(t);
                since_tool.push_str(t);
            }
            _ => {}
        }
    }
    let reply = if since_tool.trim().is_empty() { turn } else { since_tool };
    (!reply.trim().is_empty()).then(|| reply.trim().to_string())
}

fn agy(text: &str) -> Option<String> {
    lines(text).rev().find(|step| s(step, "type") == "PLANNER_RESPONSE" && !s(step, "content").trim().is_empty()).map(|step| s(&step, "content").trim().to_string())
}

// The reply in a transcript's words, by its agent ("claude" for Claude
// Code's), or none.
pub fn from_text(agent: &str, text: &str) -> Option<String> {
    match agent {
        "codex" => codex(text),
        "grok" => grok(text),
        "agy" => agy(text),
        _ => claude(text),
    }
}

pub fn last_reply(agent: &str, path: &str) -> Option<String> {
    let path = Path::new(path);
    if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
        return None;
    }
    from_text(agent, &tail(path)?)
}

// At most MAX_CHARS characters, and whether it was cut.
pub fn clip(text: &str) -> (String, bool) {
    match text.char_indices().nth(MAX_CHARS) {
        Some((at, _)) => (text[..at].to_string(), true),
        None => (text.to_string(), false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn jsonl(lines: &[Value]) -> String {
        lines.iter().map(Value::to_string).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn claude_s_last_reply_however_many_lines_it_took() {
        let t = jsonl(&[
            json!({ "type": "user", "message": { "role": "user", "content": "do it" } }),
            json!({ "type": "assistant", "message": { "content": [{ "type": "text", "text": "Looking." }, { "type": "tool_use", "name": "Bash" }] } }),
            json!({ "type": "user", "message": { "content": [{ "type": "tool_result", "content": "ok" }] } }),
            json!({ "type": "assistant", "message": { "content": [{ "type": "thinking", "thinking": "hm" }] } }),
            json!({ "type": "assistant", "message": { "content": [{ "type": "text", "text": "## Done\n\n- one" }] } }),
            json!({ "type": "assistant", "message": { "content": [{ "type": "text", "text": "And **two**." }] } }),
            json!({ "type": "attachment", "attachment": { "type": "hook_success" } }),
        ]);
        assert_eq!(from_text("claude", &t).as_deref(), Some("## Done\n\n- one\n\nAnd **two**."));
        // At work still: the last words it said.
        let t2 = jsonl(&[json!({ "type": "assistant", "message": { "content": [{ "type": "text", "text": "Earlier." }] } }), json!({ "type": "user", "message": { "content": [{ "type": "tool_result" }] } })]);
        assert_eq!(from_text("claude", &t2).as_deref(), Some("Earlier."));
        assert_eq!(from_text("claude", ""), None);
    }

    #[test]
    fn codex_s_assistant_message_else_the_task_s_last() {
        let t = jsonl(&[
            json!({ "type": "response_item", "payload": { "type": "message", "role": "user", "content": [{ "type": "input_text", "text": "q" }] } }),
            json!({ "type": "response_item", "payload": { "type": "message", "role": "assistant", "content": [{ "type": "output_text", "text": "First" }, { "type": "output_text", "text": "Second" }] } }),
            json!({ "type": "event_msg", "payload": { "type": "task_complete", "last_agent_message": "Second" } }),
        ]);
        assert_eq!(from_text("codex", &t).as_deref(), Some("First\n\nSecond"));
        let t2 = jsonl(&[json!({ "type": "event_msg", "payload": { "type": "task_complete", "last_agent_message": "Only this" } })]);
        assert_eq!(from_text("codex", &t2).as_deref(), Some("Only this"));
    }

    #[test]
    fn grok_s_last_turn_after_its_last_tool() {
        let chunk = |k: &str, t: &str| json!({ "params": { "update": { "sessionUpdate": k, "content": { "type": "text", "text": t } } } });
        let t = jsonl(&[
            chunk("user_message_chunk", "old"),
            chunk("agent_message_chunk", "Old reply"),
            chunk("user_message_chunk", "new"),
            chunk("agent_message_chunk", "I'll look."),
            json!({ "params": { "update": { "sessionUpdate": "tool_call" } } }),
            chunk("agent_message_chunk", "Found "),
            chunk("agent_message_chunk", "it."),
            json!({ "params": { "update": { "sessionUpdate": "turn_completed" } } }),
        ]);
        assert_eq!(from_text("grok", &t).as_deref(), Some("Found it."));
        let t2 = jsonl(&[chunk("user_message_chunk", "q"), chunk("agent_message_chunk", "Before the tool."), json!({ "params": { "update": { "sessionUpdate": "tool_call" } } })]);
        assert_eq!(from_text("grok", &t2).as_deref(), Some("Before the tool."));
    }

    #[test]
    fn antigravity_s_last_planner_response() {
        let t = jsonl(&[
            json!({ "type": "PLANNER_RESPONSE", "content": "first" }),
            json!({ "type": "USER_INPUT", "content": "<USER_REQUEST>q</USER_REQUEST>" }),
            json!({ "type": "PLANNER_RESPONSE", "content": "  last  " }),
            json!({ "type": "PLANNER_RESPONSE", "content": "" }),
        ]);
        assert_eq!(from_text("agy", &t).as_deref(), Some("last"));
    }

    #[test]
    fn so_much_and_no_more() {
        let long = "字".repeat(MAX_CHARS + 5);
        let (cut, was) = clip(&long);
        assert_eq!((cut.chars().count(), was), (MAX_CHARS, true));
        assert_eq!(clip("short"), ("short".to_string(), false));
    }

    #[test]
    fn only_a_transcript_is_read() {
        assert_eq!(last_reply("claude", "C:\\Windows\\win.ini"), None);
        assert_eq!(last_reply("claude", "Z:\\no\\such.jsonl"), None);
    }
}
