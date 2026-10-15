// The local HTTP port the Claude Code and Codex hooks report to.
//
//   GET  /health       { ok, app, runtime, state }; with WAKUWAKU_DEBUG=1, the windows and settings too
//   POST /hook         a Claude Code hook event; answers {}, or for a prompt the person's answer
//                      ?agent=codex: a Codex one, handed on by this program (main.rs codex_hook)
//   POST /state        a message (state.rs)
//   POST /widget       a widget for the island, from a script (widgets.rs)
//   POST /come-home    started again while running: back into sight
//   POST /debug/eval   { page, code }: run code in her page or the island's (WAKUWAKU_DEBUG=1 only)
//   POST /debug/walk   { dx, ms }: take a walk now (WAKUWAKU_DEBUG=1 only)
//   POST /quit         quit, as the menu does (a new build replacing her; killing her
//                      would leave the top bar's strip taken)
use std::io::Read;
use std::sync::Arc;

use serde_json::{json, Value};
use tauri::Manager;
use tiny_http::{Header, Request, Response, Server};

use crate::{events, events_agy, events_codex, events_grok, island, jump, now_ms, pet, Responder, Shared};

// Hook events carry whole files (an Edit's PostToolUse has the file before
// the edit), so this is generous.
const MAX_BODY: u64 = 16 * 1024 * 1024;
// Codex waits on a prompt's hook before it shows its own dialog: the panel
// holds one this long at most.
const CODEX_ASK_MS: u64 = 60 * 1000;

// Which agent's hook the request comes from (?agent=codex, ?agent=agy: handed
// on by this program, main.rs; ?agent=grok: Grok's own HTTP hooks); none for
// Claude Code's.
fn agent_of(url: &str) -> Option<&'static str> {
    let (_, query) = url.split_once('?')?;
    query.split('&').find_map(|p| match p {
        "agent=codex" => Some("codex"),
        "agent=agy" => Some("agy"),
        "agent=grok" => Some("grok"),
        _ => None,
    })
}

// Who an event is from: the URL says, else Claude Code's hooks brought it,
// Grok's among them (Grok loads them too, events_grok.rs is_grok). Grok's
// through Claude Code's hooks: dropped (None) while Grok's own bring the
// same here, else taken as Grok's.
fn sender_of(url: &str, event: &Value, port: u16) -> Option<Option<&'static str>> {
    match agent_of(url) {
        Some(agent) => Some(Some(agent)),
        None if events_grok::is_grok(event) => (!crate::connection::grok_hooked(port)).then_some(Some("grok")),
        None => Some(None),
    }
}

pub fn serve(sh: Arc<Shared>, server: Server) {
    let is_debug = std::env::var("WAKUWAKU_DEBUG").as_deref() == Ok("1");
    crate::spawn_named("server", move || {
        for req in server.incoming_requests() {
            handle(&sh, req, is_debug);
        }
    });
}

fn reply(req: Request, code: u16, body: Value) {
    let header = Header::from_bytes("content-type", "application/json").expect("a valid header");
    let _ = req.respond(Response::from_string(body.to_string()).with_status_code(code).with_header(header));
}

fn read_json(req: &mut Request) -> Option<Value> {
    let mut bytes = Vec::new();
    req.as_reader().take(MAX_BODY).read_to_end(&mut bytes).ok()?;
    serde_json::from_slice(&bytes).ok()
}

// The process chain of the session an event comes from, looked up when the
// session has none yet or a turn begins (it may have been resumed in another
// process): Codex's and Antigravity's hook command brings its own; for
// Claude Code and Grok (HTTP hooks), it is the process on the other end of
// this connection.
fn chain_for(sh: &Shared, req: &Request, event: &Value, brings_own: bool) -> Option<jump::Chain> {
    if brings_own {
        let chain = jump::chain_from_json(event.get(jump::CODEX_CHAIN));
        return (!chain.is_empty()).then_some(chain);
    }
    let session = event.get("session_id").and_then(Value::as_str).unwrap_or("");
    let is_turn = matches!(event["hook_event_name"].as_str(), Some("UserPromptSubmit" | "SessionStart"));
    if session.is_empty() || !(is_turn || !sh.pet.lock().unwrap().has_chain(session)) {
        return None;
    }
    let client = jump::client_of(req.remote_addr()?.port(), sh.port)?;
    let chain = jump::chain_of(client);
    (!chain.is_empty()).then_some(chain)
}

fn handle(sh: &Arc<Shared>, mut req: Request, is_debug: bool) {
    let path = req.url().split('?').next().unwrap_or("").to_string();
    let method = req.method().as_str().to_string();
    match (method.as_str(), path.as_str()) {
        ("POST", "/hook") => {
            let Some(mut event) = read_json(&mut req) else { return reply(req, 200, json!({})) };
            // Grok's, through Claude Code's hooks, while its own bring it too: once is enough.
            let Some(agent) = sender_of(req.url(), &event, sh.port) else { return reply(req, 200, json!({})) };
            // Heard from one: connected (the island stops asking).
            if crate::connection::note_connected(true) {
                sh.redraw();
            }
            // Codex's, Antigravity's and Grok's events, tagged for the prompts
            // (asks.rs) and read their own way.
            if let (Some(agent), Some(fields)) = (agent, event.as_object_mut()) {
                fields.insert("agent".into(), json!(agent));
            }
            let seen = match agent {
                Some("codex") => &sh.codex_seen,
                Some("agy") => &sh.agy_seen,
                Some("grok") => &sh.grok_seen,
                _ => &sh.claude_seen,
            };
            seen.store(now_ms(), std::sync::atomic::Ordering::SeqCst);
            let is_codex = agent == Some("codex");
            let msg = match agent {
                Some("codex") => events_codex::to_message(&event),
                Some("grok") => events_grok::to_message(&event),
                Some("agy") => {
                    events_agy::with_transcript(&mut event);
                    if let Some(line) = events_agy::log_line(&event) {
                        sh.log(&line);
                    }
                    events_agy::to_message(&event)
                }
                _ => events::to_message(&event),
            };
            if let Some(mut msg) = msg {
                // Where the session runs, for going to its window (jump.rs).
                if let Some(chain) = chain_for(sh, &req, &event, matches!(agent, Some("codex" | "agy"))) {
                    msg["chain"] = jump::chain_json(&chain);
                }
                sh.apply(&msg);
            }
            // A prompt holds the answer until the person picks on the pet, the
            // terminal settles it, or time runs out; out of sight (main.rs
            // asks_seen), it is the terminal's at once.
            let is_prompt = event["hook_event_name"] == "PermissionRequest";
            if is_prompt && !sh.asks_seen() {
                sh.log("a prompt handed to the terminal: out of sight");
            }
            if is_prompt && sh.asks_seen() {
                let respond: Responder = Box::new(move |out: Option<Value>| match out {
                    Some(out) => reply(req, 200, if out.is_object() { out } else { json!({}) }),
                    None => drop(req),
                });
                let mut wait = sh.setting("promptWaitSec").as_u64().unwrap_or(290) * 1000;
                if is_codex {
                    wait = wait.min(CODEX_ASK_MS);
                }
                let added = sh.asks.lock().unwrap().add(event, respond, now_ms(), wait);
                match added {
                    Ok(id) => {
                        sh.log(&format!("prompt {id} waits on the person"));
                        sh.push_asks();
                    }
                    Err(respond) => respond(Some(json!({}))),
                }
                return;
            }
            let settled = sh.asks.lock().unwrap().seen(&event);
            if !settled.is_empty() {
                for id in settled {
                    sh.log(&format!("prompt {id} settled in the terminal ({})", event["hook_event_name"].as_str().unwrap_or("?")));
                }
                sh.push_asks();
            }
            reply(req, 200, json!({}))
        }
        // A widget from a script (widgets.rs).
        ("POST", "/widget") => match read_json(&mut req) {
            Some(v) => match sh.put_widget(&v) {
                Ok(()) => reply(req, 200, json!({ "ok": true })),
                Err(why) => reply(req, 400, json!({ "error": why })),
            },
            None => reply(req, 400, json!({ "error": "bad json" })),
        },
        // Started again while she runs: she comes back into sight.
        ("POST", "/come-home") => {
            crate::come_home(sh);
            reply(req, 200, json!({ "ok": true }))
        }
        ("GET", "/health") => {
            let mut body = json!({ "ok": true, "app": "wakuwaku", "runtime": "tauri", "state": sh.pet.lock().unwrap().get(now_ms()) });
            if is_debug {
                body["window"] = pet::where_(sh);
                body["island"] = island::where_(sh);
                body["settings"] = Value::Object(sh.settings.lock().unwrap().clone());
            }
            reply(req, 200, body)
        }
        ("POST", "/state") => match read_json(&mut req) {
            Some(msg) if sh.apply(&msg) => reply(req, 200, json!({ "ok": true })),
            Some(_) => reply(req, 400, json!({ "error": "nothing to do" })),
            None => reply(req, 400, json!({ "error": "bad json" })),
        },
        ("POST", "/debug/eval") if is_debug => {
            let Some(body) = read_json(&mut req) else { return reply(req, 400, json!({ "error": "bad json" })) };
            let page = body["page"].as_str().unwrap_or("pet");
            let code = body["code"].as_str().unwrap_or("null");
            match evaluate(sh, page, code) {
                Some(value) => reply(req, 200, json!({ "result": value })),
                None => reply(req, 504, json!({ "error": "no answer" })),
            }
        }
        // Quit as the menu does (the bar's strip given back on the way): for
        // replacing her with a new build, which must not just kill her.
        ("POST", "/quit") | ("POST", "/debug/quit") => {
            reply(req, 200, json!({ "ok": true }));
            sh.quit();
        }
        ("POST", "/debug/walk") if is_debug => {
            let Some(body) = read_json(&mut req) else { return reply(req, 400, json!({ "error": "bad json" })) };
            let (dx, ms) = (body["dx"].as_f64().unwrap_or(0.0), body["ms"].as_f64().unwrap_or(500.0));
            // Walks block; this one answers when it is over.
            let sh = sh.clone();
            std::thread::spawn(move || {
                let arrived = pet::walk(&sh, dx, ms);
                reply(req, 200, json!({ "ok": true, "arrived": arrived }));
            });
        }
        _ => reply(req, 404, json!({ "error": "not found" })),
    }
}

// Run code in a page ("pet" or "island") and wait for what it gives back.
fn evaluate(sh: &Shared, page: &str, code: &str) -> Option<Value> {
    let win = sh.app.get_webview_window(page)?;
    let (tx, rx) = std::sync::mpsc::channel();
    let id = {
        let mut evals = sh.evals.lock().unwrap();
        evals.0 += 1;
        let id = evals.0;
        evals.1.insert(id, tx);
        id
    };
    win.eval(format!("window.__wkEval({id}, () => ({code}))")).ok()?;
    let answer = rx.recv_timeout(std::time::Duration::from_secs(3)).ok();
    sh.evals.lock().unwrap().1.remove(&id);
    answer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_hooks_say_where_they_come_from() {
        assert_eq!(agent_of("/hook?from=wakuwaku&agent=codex"), Some("codex"));
        assert_eq!(agent_of("/hook?agent=codex"), Some("codex"));
        assert_eq!(agent_of("/hook?from=wakuwaku&agent=agy"), Some("agy"));
        assert_eq!(agent_of("/hook?from=wakuwaku"), None);
        assert_eq!(agent_of("/hook?agent=codexx"), None);
        assert_eq!(agent_of("/hook"), None);
        assert_eq!(agent_of("/hook?from=wakuwaku&agent=grok"), Some("grok"));
    }

    // Grok's events through Claude Code's hooks (no agent in the URL), with
    // Grok's own hooks not in: taken as Grok's; Claude Code's stay its own.
    #[test]
    fn grok_events_without_an_agent_are_known_by_their_body() {
        let home = std::env::temp_dir().join(format!("wakuwaku-grok-home-{}", std::process::id()));
        std::env::set_var("GROK_HOME", &home);
        let grok = json!({ "hookEventName": "stop", "hook_event_name": "Stop", "session_id": "s" });
        let claude = json!({ "hook_event_name": "Stop", "session_id": "s" });
        assert_eq!(sender_of("/hook?from=wakuwaku", &grok, 47213), Some(Some("grok")));
        assert_eq!(sender_of("/hook?from=wakuwaku", &claude, 47213), Some(None));
        assert_eq!(sender_of("/hook?from=wakuwaku&agent=grok", &grok, 47213), Some(Some("grok")));
        assert_eq!(sender_of("/hook?from=wakuwaku&agent=codex", &claude, 47213), Some(Some("codex")));
    }
}
