// Grok Build hook events → pet messages, the same messages as events.rs makes
// (state.rs has the format), plus agent: "grok" and the turn they belong to.
// The words for a step, a task, a reply, an error and the project are
// events.rs's.
//
// Grok posts each event as an HTTP hook to /hook?agent=grok (connection.rs
// writes ~/.grok/hooks/wakuwaku.json), and the Claude Code hooks it also
// loads (the plugin, ~/.claude/settings.json) bring it to /hook without an
// agent: server.rs knows those by hookEventName. What differs from Claude
// Code (Grok Build 1.0.52, xai-org/grok-build):
//   - its own keys are camelCase; only some have snake_case copies
//     (hook_event_name, session_id, tool_name, tool_input, tool_use_id…), so
//     promptId, lastAssistantMessage and errorDetails are read as they are
//   - its tools: run_terminal_command, write, search_replace, read_file
//     (target_file), grep, list_dir, web_search, spawn_subagent; an MCP
//     tool is server__tool
//   - no PermissionRequest: a hook's allow does not approve a call, so the
//     person answers in Grok, and Notification permission_prompt says Grok
//     waits on them; idle_prompt comes a minute after a turn settles
//   - a turn ends in Stop (reason end_turn), StopFailure or StopCancelled;
//     the session's end brings an extra Stop (reason shutdown) after
//     SessionEnd, which ends nothing
//   - a subagent's events carry subagentType: not the session's own
// The tool's result (toolResult, tool_response: a whole file read may be
// tens of KB) is never read.
use serde_json::{json, Value};

use crate::events::{base, error_of, project_of, reply_of, step, step_of, task_of};

fn s<'a>(e: &'a Value, key: &str) -> &'a str {
    e.get(key).and_then(Value::as_str).unwrap_or("")
}

// Grok's tools that edit a file, by its own names (state.rs EDIT_TOOLS too).
const EDITS: [&str; 2] = ["write", "search_replace"];

// What a tool is doing, in plain words: Grok's own tools by what they take,
// the rest as Claude Code's of the same kind.
fn step_of_grok(tool: &str, input: &Value) -> Value {
    let i = |key: &str| s(input, key);
    match tool {
        "run_terminal_command" => step_of("Bash", input),
        "write" => step("step.write", json!({ "file": base(i("file_path")) })),
        "search_replace" => step("step.edit", json!({ "file": base(i("file_path")) })),
        "read_file" => step("step.read", json!({ "file": base(i("target_file")) })),
        "grep" => step_of("Grep", input),
        "web_search" => step_of("WebSearch", input),
        "spawn_subagent" => step_of("Agent", input),
        // An MCP tool: Grok names it server__tool, Claude Code mcp__server__tool.
        _ => match tool.split_once("__").filter(|(server, name)| !server.is_empty() && !name.is_empty()) {
            Some((server, name)) => step("step.mcp", json!({ "server": server, "tool": name })),
            None => step("step.tool", json!({ "tool": tool })),
        },
    }
}

// The mood part of the message for an event, or None for one that changes nothing.
fn mood_of(e: &Value) -> Option<Value> {
    // A subagent's own session: not the one the person is in.
    if !s(e, "subagentType").is_empty() {
        return None;
    }
    let tool = s(e, "tool_name");
    let input = e.get("tool_input").unwrap_or(&Value::Null);
    let event = s(e, "hook_event_name");
    let mut m = match event {
        "SessionStart" => json!({ "mood": "idle", "react": "wave", "say": { "key": "say.hello" } }),
        "SessionEnd" => json!({ "mood": "idle", "event": "session-end" }),
        "UserPromptSubmit" => {
            let mut m = json!({ "mood": "working", "event": "turn-start" });
            if let Some(task) = task_of(s(e, "prompt")) {
                m["task"] = json!(task);
            }
            m
        }
        "PreToolUse" => json!({ "mood": "working", "detail": tool, "event": "tool-start", "step": step_of_grok(tool, input) }),
        "PostToolUse" => {
            let mut m = json!({ "mood": "working", "detail": tool, "event": "tool-done" });
            if EDITS.contains(&tool) && !s(input, "file_path").is_empty() {
                m["file"] = json!(s(input, "file_path"));
            }
            m
        }
        "PostToolUseFailure" => json!({
            "mood": "working", "detail": tool, "event": "tool-failed", "react": "failed",
            "say": { "key": "say.toolFailed", "vars": { "tool": tool } },
        }),
        "Notification" => match s(e, "notificationType") {
            // Grok's own prompt waits on the person: what for, state.rs says
            // from the step under way (the notification does not name it).
            "permission_prompt" => json!({ "mood": "waiting", "event": "permission", "detail": { "key": "detail.approveThere" } }),
            // A minute after the session settled: one still at work or
            // waiting is not (state.rs: it changes nothing else).
            "idle_prompt" => json!({ "mood": "idle", "event": "settle" }),
            _ => return None,
        },
        // Only a turn's own end: the one at the session's end ends nothing.
        "Stop" if !matches!(s(e, "reason"), "" | "end_turn") => return None,
        "Stop" => json!({ "mood": "done", "event": "turn-end" }),
        "StopFailure" => {
            let as_claude = json!({ "error": s(e, "error"), "error_details": s(e, "errorDetails") });
            json!({ "mood": "error", "event": "turn-end", "error": error_of(&as_claude) })
        }
        // Stopped by the person, or a call they rejected: nothing to come and see.
        "StopCancelled" => json!({ "mood": "idle", "event": "turn-end" }),
        _ => return None,
    };
    if matches!(event, "PreToolUse" | "PostToolUse" | "PostToolUseFailure") && !s(e, "tool_use_id").is_empty() {
        m["toolId"] = json!(s(e, "tool_use_id"));
    }
    if event == "Stop" {
        if let Some(reply) = reply_of(s(e, "lastAssistantMessage")) {
            m["reply"] = json!(reply);
        }
    }
    Some(m)
}

// The message for an event, or None for one that changes nothing.
pub fn to_message(e: &Value) -> Option<Value> {
    if !e.is_object() {
        return None;
    }
    let mut message = mood_of(e)?;
    message["agent"] = json!("grok");
    for (from, to) in [("session_id", "session"), ("promptId", "turn")] {
        if !s(e, from).is_empty() {
            message[to] = json!(s(e, from));
        }
    }
    if !s(e, "cwd").is_empty() {
        message["project"] = json!(project_of(s(e, "cwd")));
    }
    Some(message)
}

// Whether an event is Grok's: its camelCase event name, which Claude Code
// never sends (server.rs, for the Claude Code hooks Grok loads too).
pub fn is_grok(e: &Value) -> bool {
    e.get("hookEventName").and_then(Value::as_str).is_some_and(|n| !n.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    // As Grok Build 1.0.52 sent them (tools/hook_dump.py, 2026-10-16), whole.
    macro_rules! fixture {
        ($name:literal) => {
            serde_json::from_str::<Value>(include_str!(concat!("../fixtures/grok/", $name, ".json"))).unwrap()
        };
    }

    // The step a fixture's PreToolUse says.
    macro_rules! step_in {
        ($name:literal) => {
            to_message(&fixture!($name)).unwrap()["step"].clone()
        };
    }

    #[test]
    fn grok_events_are_known_by_their_camel_case_name() {
        for e in [fixture!("session-start"), fixture!("pre-write"), fixture!("notification-idle"), fixture!("stop-shutdown")] {
            assert!(is_grok(&e));
        }
        assert!(!is_grok(&json!({ "hook_event_name": "Stop", "session_id": "s", "cwd": "C:\\x" })));
        assert!(!is_grok(&json!("nope")));
    }

    #[test]
    fn a_session_starts_and_ends() {
        let start = fixture!("session-start");
        let m = to_message(&start).unwrap();
        assert_eq!((m["mood"].as_str(), m["react"].as_str(), m["agent"].as_str()), (Some("idle"), Some("wave"), Some("grok")));
        assert_eq!(m["session"].as_str(), Some(s(&start, "session_id")));
        assert_eq!(m["project"], "grok-test");
        let m = to_message(&fixture!("session-end")).unwrap();
        assert_eq!(m["event"], "session-end");
        // The Stop that follows SessionEnd as the session closes ends nothing.
        assert!(to_message(&fixture!("stop-shutdown")).is_none());
    }

    #[test]
    fn a_turn_starts_with_the_persons_words_and_ends_with_grok_s() {
        let prompt = fixture!("prompt");
        let m = to_message(&prompt).unwrap();
        assert_eq!((m["mood"].as_str(), m["event"].as_str()), (Some("working"), Some("turn-start")));
        assert_eq!(m["turn"].as_str(), prompt.get("promptId").and_then(Value::as_str));
        assert!(m["task"].as_str().is_some_and(|t| t.starts_with("讀 hello.txt 的內容")));

        let stop = fixture!("stop-end-turn");
        let m = to_message(&stop).unwrap();
        assert_eq!((m["mood"].as_str(), m["event"].as_str()), (Some("done"), Some("turn-end")));
        assert_eq!(m["turn"].as_str(), stop.get("promptId").and_then(Value::as_str));
        assert!(m["reply"].as_str().is_some_and(|r| r.contains("hello.txt")), "{}", m["reply"]);
    }

    #[test]
    fn each_tool_says_what_it_does() {
        assert_eq!(step_in!("pre-write"), json!({ "key": "step.write", "vars": { "file": "hello.txt" } }));
        assert_eq!(step_in!("pre-read-file"), json!({ "key": "step.read", "vars": { "file": "hello.txt" } }));
        assert_eq!(step_in!("pre-grep"), json!({ "key": "step.search", "vars": { "what": "hello" } }));
        assert_eq!(step_in!("pre-search-replace"), json!({ "key": "step.edit", "vars": { "file": "hello.txt" } }));
        assert_eq!(step_in!("pre-terminal"), json!({ "key": "step.run", "vars": { "what": "Print hello.txt file contents" } }));
        let m = to_message(&fixture!("pre-terminal")).unwrap();
        assert_eq!((m["mood"].as_str(), m["event"].as_str(), m["detail"].as_str()), (Some("working"), Some("tool-start"), Some("run_terminal_command")));
        assert!(m["toolId"].as_str().is_some_and(|id| id.starts_with("call-")));
        assert_eq!(step_of_grok("linear__save_issue", &json!({})), json!({ "key": "step.mcp", "vars": { "server": "linear", "tool": "save_issue" } }));
        assert_eq!(step_of_grok("list_dir", &json!({})), json!({ "key": "step.tool", "vars": { "tool": "list_dir" } }));
    }

    #[test]
    fn a_tool_s_end_names_the_file_it_edited_and_leaves_its_result() {
        let m = to_message(&fixture!("post-search-replace")).unwrap();
        assert_eq!((m["event"].as_str(), m["detail"].as_str()), (Some("tool-done"), Some("search_replace")));
        assert_eq!(m["file"], "C:\\Users\\iaedo\\grok-test\\hello.txt");
        assert_eq!(to_message(&fixture!("post-write")).unwrap()["file"], "C:\\Users\\iaedo\\grok-test\\hello.txt");
        assert!(to_message(&fixture!("post-terminal")).unwrap().get("file").is_none());
        // A whole file read (84 KB): its result is not carried on.
        let big = fixture!("post-read-file-large");
        assert!(big["toolResult"].to_string().len() > 40_000);
        let m = to_message(&big).unwrap();
        assert!(m.to_string().len() < 1_000, "{m}");
        assert_eq!(m["event"], "tool-done");
    }

    #[test]
    fn grok_waiting_on_the_person_and_settling() {
        let m = to_message(&fixture!("notification-permission")).unwrap();
        assert_eq!((m["mood"].as_str(), m["event"].as_str()), (Some("waiting"), Some("permission")));
        assert_eq!(m["detail"], json!({ "key": "detail.approveThere" }));
        let m = to_message(&fixture!("notification-idle")).unwrap();
        assert_eq!((m["mood"].as_str(), m["event"].as_str()), (Some("idle"), Some("settle")));
        assert!(to_message(&json!({ "hook_event_name": "Notification", "hookEventName": "notification", "notificationType": "task_complete" })).is_none());
    }

    #[test]
    fn a_turn_cancelled_or_failed() {
        // A rejected permission, and Ctrl+C.
        for (name, e) in [("rejected", fixture!("cancelled-rejected")), ("interrupt", fixture!("cancelled-interrupt"))] {
            let m = to_message(&e).unwrap();
            assert_eq!((m["mood"].as_str(), m["event"].as_str()), (Some("idle"), Some("turn-end")), "{name}");
            assert_eq!(m["turn"].as_str(), e.get("promptId").and_then(Value::as_str), "{name}");
            assert!(m.get("reply").is_none(), "{name}");
        }
        // StopFailure: not caught from Grok; its fields as Grok's docs and source give them.
        let m = to_message(&json!({ "hook_event_name": "StopFailure", "hookEventName": "stop_failure", "session_id": "s", "promptId": "p", "error": "rate_limit", "errorDetails": "429" })).unwrap();
        assert_eq!((m["mood"].as_str(), m["turn"].as_str()), (Some("error"), Some("p")));
        assert_eq!(m["error"], json!({ "key": "error.rate_limit", "vars": {} }));
        let m = to_message(&json!({ "hook_event_name": "StopFailure", "hookEventName": "stop_failure", "error": "unknown", "errorDetails": "socket closed\nmore" })).unwrap();
        assert_eq!(m["error"], "socket closed");
    }

    #[test]
    fn a_subagent_s_events_are_not_the_session_s() {
        let mut end = fixture!("session-end");
        end["subagentType"] = json!("explore");
        assert!(to_message(&end).is_none());
        assert!(to_message(&json!("nope")).is_none());
    }
}
