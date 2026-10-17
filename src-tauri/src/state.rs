// What the pet is doing, from the messages the hooks bring (events.rs,
// events_codex.rs, events_agy.rs, events_grok.rs). A message is
//   { session?, project?, mood?, detail?, event?, react?, say?,
//     title?, task?, step?, toolId?, todo?, file?, reply?, error?, agent?, turn? }
//   mood     idle | working | waiting | done | review | error
//   detail   a tool name, or { key, vars } to translate
//   event    turn-start | tool-start | tool-done | tool-failed | turn-end | session-end,
//            and Grok's: permission (its own prompt waits on the person; what
//            for is the step under way) and settle (a minute after it went
//            quiet: one still at work or waiting is not, any other is left)
//   react    wave | jump | failed: played once over the mood, `say` in the bubble
//   agent    "codex", "agy", "grok", or none for Claude Code
//   turn     the turn it belongs to (Codex): some of its hooks run in the
//            background, so one can come after its turn ended (turn-end), and
//            changes nothing then. Antigravity's turn-start has one too (the
//            person's words it answers): it starts again, the same turn, once
//            a task it left running in the background is done. Grok's
//            (promptId): the end of a turn before the one under way changes
//            nothing (its cancel report may come after the next prompt)
//   title    the session's name; task   the person's words for the work
//   step     what a starting tool does, { key, vars }; toolId pairs it with its end
//   todo     { set: [{ text, active, status }] }, { add: { text, active } }
//            or { update: { id, status?, text?, active? } }
//   file     a file just edited
//   reply    how the turn ended, in Claude's words; error   why it failed
//   chain    the processes it runs in, [[pid, started], ...] (jump.rs)
//   transcript  its agent's transcript file, where its replies are whole (reply.rs)
//   cwd      the folder it works in (where the phone's words resume it, talk.rs)
//
// Each session keeps its own mood; the pet shows the one that most wants you.
// Instead of callbacks and timers, calls return what happened (Outcome) and
// tick() settles the endings that only stay a while.
use std::collections::HashMap;

use serde_json::{json, Map, Value};

use crate::jump;

const MOODS: [&str; 6] = ["idle", "working", "waiting", "done", "review", "error"];
const REACTS: [&str; 3] = ["wave", "jump", "failed"];
// Claude Code's, Codex's (apply_patch), Antigravity's (write_to_file, as
// agy 1.3.4 names it; its edits by the names it keeps from Windsurf), Grok
// Build's (write, search_replace).
const EDIT_TOOLS: [&str; 10] = ["Edit", "Write", "MultiEdit", "NotebookEdit", "apply_patch", "write_to_file", "replace_file_content", "multi_replace_file_content", "write", "search_replace"];
// The turns remembered as ended, per session.
const ENDED_TURNS: usize = 4;

// A session that died mid-turn never says idle: give up on it after this.
pub const STALE_MS: u64 = 15 * 60 * 1000;
// An ending nobody came to see, or a session gone quiet, after this.
pub const FORGET_MS: u64 = 2 * 60 * 60 * 1000;
// A session at rest, out of sight after this many minutes with nothing new
// from it, unless set otherwise (idleHideMin; 0 never).
pub const IDLE_HIDE_MIN: u64 = 30;
// Never more than this many of a list kept for a session.
const MAX_KEPT: usize = 100;
// How long a name goes into a notification.
const WHOSE_MAX: usize = 30;

fn priority(mood: &str) -> u8 {
    match mood {
        "waiting" => 5,
        "error" => 4,
        "review" => 3,
        "done" => 2,
        "working" => 1,
        _ => 0,
    }
}

// The agent a session is of, by the name the settings keep it under
// (data::AGENTS): Claude Code's sessions have none.
pub fn agent_key(agent: &str) -> &str {
    if agent.is_empty() {
        "claude"
    } else {
        agent
    }
}

fn is_ending(mood: &str) -> bool {
    matches!(mood, "done" | "review" | "error")
}

// Whether a session is in sight (Pet::hide_quiet). At work or waiting on
// you, always (STALE_MS gives up on one stuck); an ending, until it is seen
// (FORGET_MS at most: check_stale). At rest, until nothing has come from it
// for `hide_ms` (0: always), counted from its last event, or from when its
// ending was seen or its held time ran out, whichever came later. One out of
// sight is still kept, and its next event brings it back as it was.
pub fn is_shown(mood: &str, event_at: u64, seen_at: Option<u64>, now: u64, hide_ms: u64) -> bool {
    match mood {
        "idle" => {
            let quiet_since = seen_at.map_or(event_at, |seen| seen.max(event_at));
            hide_ms == 0 || now.saturating_sub(quiet_since) < hide_ms
        }
        _ => true,
    }
}

// How long done / review / error stay: until seen, or this many seconds.
#[derive(Clone, Copy)]
pub enum Hold {
    Seen,
    Secs(u64),
}

#[derive(Default)]
pub struct Outcome {
    // The mood shown may have changed: redraw.
    pub changed: bool,
    // { react, say } to play once.
    pub react: Option<Value>,
    // Moods just arrived that want a chime (and a notification): (mood,
    // whose: the session's name, or its project).
    pub alerts: Vec<(String, String)>,
    // For today's count: turns begun, and how long the ones that ended took
    // (stopped by the person too, and one that never said it ended, up to
    // its last event, when the next begins).
    pub turns: u64,
    pub worked_ms: u64,
}

// A tool running now, and what it does.
struct Running {
    id: String,
    step: Value,
    since: u64,
}

struct Todo {
    id: String,
    text: String,
    active: String,
    status: String,
}

struct Session {
    id: String,
    project: String,
    mood: String,
    detail: Value,
    at: u64,
    since: Option<u64>,
    took: Option<u64>,
    has_edited: bool,
    settle_at: Option<u64>,
    title: String,
    task: String,
    // The tools running, the latest last (subagents run theirs inside one).
    running: Vec<Running>,
    // When the last tool ended, or the turn began: thinking since then.
    thought_at: Option<u64>,
    todos: Vec<Todo>,
    // To-dos ever added one by one: Claude Code numbers them from 1.
    added: u32,
    // The files this turn edited.
    files: Vec<String>,
    reply: String,
    error: Value,
    agent: String,
    // The processes it runs in, for going to its window.
    chain: jump::Chain,
    transcript: String,
    cwd: String,
    // Its agent closed it (SessionEnd): out of sight on her once its ending
    // is over, kept for the phone (closed_for) to read and go on with, until
    // forgotten as any session at rest is.
    closed: bool,
    // The turn under way, the last few that ended (Codex), and the last few
    // seen to start (Grok: an end of one of those, not the one under way, is late).
    turn: String,
    ended_turns: Vec<String>,
    begun_turns: Vec<String>,
    // When its last event came (`at` moves too as an ending goes back to
    // rest), and when its ending was seen or its held time ran out: how
    // long it has been quiet. Out of sight as last looked at (is_shown).
    event_at: u64,
    seen_at: Option<u64>,
    hidden: bool,
}

impl Session {
    fn new(id: String, now: u64) -> Self {
        Session {
            id,
            project: String::new(),
            mood: "idle".into(),
            detail: Value::String(String::new()),
            at: now,
            since: None,
            took: None,
            has_edited: false,
            settle_at: None,
            title: String::new(),
            task: String::new(),
            running: Vec::new(),
            thought_at: None,
            todos: Vec::new(),
            added: 0,
            files: Vec::new(),
            reply: String::new(),
            error: Value::Null,
            agent: String::new(),
            chain: Vec::new(),
            transcript: String::new(),
            cwd: String::new(),
            closed: false,
            turn: String::new(),
            ended_turns: Vec::new(),
            begun_turns: Vec::new(),
            event_at: now,
            seen_at: None,
            hidden: false,
        }
    }

    // What it is called: Claude Code's name for it, or the person's words.
    fn name(&self) -> &str {
        if self.title.is_empty() { &self.task } else { &self.title }
    }

    fn whose(&self) -> String {
        clip(if self.name().is_empty() { &self.project } else { self.name() }, WHOSE_MAX)
    }
}

fn clip(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

// A text a message may carry: a string, or { key, vars } whose vars are
// strings, numbers, or (once) a text of the same kind.
fn text(value: Option<&Value>, max: usize) -> Value {
    text_in(value, max, true)
}

fn text_in(value: Option<&Value>, max: usize, may_nest: bool) -> Value {
    match value {
        Some(Value::String(s)) => Value::String(clip(s, max)),
        Some(Value::Object(o)) if o.get("key").is_some_and(Value::is_string) => {
            let mut vars = Map::new();
            if let Some(Value::Object(given)) = o.get("vars") {
                for (k, v) in given {
                    match v {
                        Value::String(s) => vars.insert(k.clone(), Value::String(clip(s, max))),
                        Value::Number(n) => vars.insert(k.clone(), Value::String(clip(&n.to_string(), max))),
                        Value::Object(_) if may_nest => vars.insert(k.clone(), text_in(Some(v), max, false)),
                        _ => None,
                    };
                }
            }
            json!({ "key": clip(o["key"].as_str().unwrap_or(""), 100), "vars": vars })
        }
        _ => Value::String(String::new()),
    }
}

fn str_field(msg: &Value, key: &str, max: usize) -> String {
    msg.get(key).and_then(Value::as_str).map(|s| clip(s, max)).unwrap_or_default()
}

// A change to the session's to-do list.
fn apply_todo(s: &mut Session, todo: &Value) {
    let item = |v: &Value, id: String| Todo {
        id,
        text: str_field(v, "text", 80),
        active: str_field(v, "active", 80),
        status: str_field(v, "status", 20),
    };
    if let Some(items) = todo.get("set").and_then(Value::as_array) {
        s.todos = items.iter().take(MAX_KEPT).enumerate().map(|(n, v)| item(v, (n + 1).to_string())).collect();
    } else if let Some(v) = todo.get("add") {
        s.added += 1;
        let mut t = item(v, s.added.to_string());
        if t.status.is_empty() {
            t.status = "pending".into();
        }
        if s.todos.len() < MAX_KEPT {
            s.todos.push(t);
        }
    } else if let Some(v) = todo.get("update") {
        let id = str_field(v, "id", 40);
        let status = str_field(v, "status", 20);
        if status == "deleted" {
            s.todos.retain(|t| t.id != id);
        } else if let Some(t) = s.todos.iter_mut().find(|t| t.id == id) {
            for (key, field) in [("status", &mut t.status), ("text", &mut t.text), ("active", &mut t.active)] {
                let value = str_field(v, key, 80);
                if !value.is_empty() {
                    *field = value;
                }
            }
        }
    }
}

// How far down its to-do list a session is, and what it is on.
fn todo_view(s: &Session) -> Value {
    if s.todos.is_empty() {
        return Value::Null;
    }
    let done = s.todos.iter().filter(|t| t.status == "completed").count();
    let active = s.todos.iter().find(|t| t.status == "in_progress").map_or("", |t| if t.active.is_empty() { &t.text } else { &t.active });
    json!({ "done": done, "total": s.todos.len(), "active": active })
}

// A session as the pages draw it.
fn view(s: &Session) -> Value {
    let is_working = s.mood == "working";
    let (step, step_since) = match s.running.last() {
        _ if !is_working => (Value::Null, None),
        Some(r) => (r.step.clone(), Some(r.since)),
        None => (json!({ "key": "step.thinking", "vars": {} }), s.thought_at),
    };
    json!({
        "id": s.id,
        "project": s.project,
        "agent": s.agent,
        // A window to go to when clicked.
        "jump": !s.chain.is_empty(),
        "mood": s.mood,
        "detail": s.detail,
        "at": s.at,
        "since": s.since,
        "took": s.took,
        "name": s.name(),
        "step": step,
        "stepSince": step_since,
        "todo": todo_view(s),
        "files": s.files.len(),
        "reply": if is_ending(&s.mood) { s.reply.as_str() } else { "" },
        "error": if s.mood == "error" { s.error.clone() } else { Value::Null },
    })
}

// A session as its conversation page sees it.
pub struct Talk {
    pub agent: String,
    pub transcript: String,
    pub cwd: String,
    pub chain: jump::Chain,
    pub reply: String,
    pub mood: String,
}

#[derive(Default)]
pub struct Pet {
    sessions: HashMap<String, Session>,
}

// Set a session's mood; Some(mood) when it wants a chime.
fn set(s: &mut Session, mood: &str, detail: Value, now: u64, hold: Hold) -> Option<String> {
    let was = std::mem::replace(&mut s.mood, mood.to_string());
    s.settle_at = None;
    s.detail = detail;
    s.at = now;

    if mood == "working" || mood == "waiting" {
        s.since.get_or_insert(now);
        s.took = None;
    } else {
        if let (Some(since), true) = (s.since, is_ending(mood)) {
            s.took = Some(now - since);
        }
        s.since = None;
        s.running.clear();
    }

    if let (true, Hold::Secs(secs)) = (is_ending(mood), hold) {
        s.settle_at = Some(now + secs * 1000);
    }
    (mood != was && (mood == "waiting" || is_ending(mood))).then(|| mood.to_string())
}

impl Pet {
    // Apply a message; None when it carries nothing we know.
    pub fn apply(&mut self, msg: &Value, now: u64, hold: Hold) -> Option<Outcome> {
        let mood = msg.get("mood").and_then(Value::as_str).filter(|m| MOODS.contains(m));
        let react = msg.get("react").and_then(Value::as_str).filter(|r| REACTS.contains(r));
        if mood.is_none() && react.is_none() {
            return None;
        }
        let mut out = Outcome::default();

        if let Some(mood) = mood {
            let id = str_field(msg, "session", 200);
            let project = str_field(msg, "project", 200);
            let detail = text(msg.get("detail"), 80);
            let event = msg.get("event").and_then(Value::as_str).unwrap_or("");
            let agent = str_field(msg, "agent", 20);
            let turn = str_field(msg, "turn", 100);

            // Grok settling: only a session still at work or waiting on the
            // person, which it is no longer; nothing else, none made for it.
            let is_settling = |s: &Session| matches!(s.mood.as_str(), "working" | "waiting");
            if event == "settle" && !self.sessions.get(&id).is_some_and(is_settling) {
                return Some(out);
            }
            if event == "session-end" {
                // Closed: an ending still to be seen (a non-interactive run
                // ends so: claude -p, codex exec); then, or at once, out of
                // her sight, kept for the phone to go on with.
                if let Some(s) = self.sessions.get_mut(&id) {
                    s.closed = true;
                    if !is_ending(&s.mood) {
                        set(s, "idle", Value::String(String::new()), now, hold);
                        s.hidden = true;
                        out.changed = true;
                    }
                }
            } else {
                let s = self.sessions.entry(id.clone()).or_insert_with(|| Session::new(id, now));
                // Going again (resumed): in sight again.
                if s.closed {
                    s.closed = false;
                    s.hidden = false;
                }
                if !project.is_empty() {
                    s.project = project;
                }
                let chain = jump::chain_from_json(msg.get("chain"));
                if !chain.is_empty() {
                    s.chain = chain;
                }
                if !agent.is_empty() {
                    s.agent = agent;
                }
                let transcript = str_field(msg, "transcript", 1000);
                if !transcript.is_empty() {
                    s.transcript = transcript;
                }
                let cwd = str_field(msg, "cwd", 1000);
                if !cwd.is_empty() {
                    s.cwd = cwd;
                }
                let is_edit = event == "tool-done" && detail.as_str().is_some_and(|d| EDIT_TOOLS.contains(&d));
                // Late, from a turn already over: only an edit counts, making its done a review.
                if !turn.is_empty() && s.ended_turns.contains(&turn) {
                    if is_edit && s.mood == "done" {
                        s.mood = "review".into();
                        out.changed = true;
                    }
                    return Some(out);
                }
                // Grok's end of a turn seen to start before the one under way
                // (a cancel is reported off its loop, after the next prompt at
                // times): kept as ended, and nothing else. A turn never seen
                // to start (its prompt's hook lost) ends as any would.
                if s.agent == "grok" && event == "turn-end" && turn != s.turn && s.begun_turns.contains(&turn) {
                    s.ended_turns.push(turn);
                    if s.ended_turns.len() > ENDED_TURNS {
                        s.ended_turns.remove(0);
                    }
                    return Some(out);
                }
                // Something new from it: back in sight at once, if it was out.
                s.event_at = now;
                s.hidden = false;
                for (key, field) in [("title", &mut s.title), ("task", &mut s.task)] {
                    let value = str_field(msg, key, 80);
                    if !value.is_empty() {
                        *field = value;
                    }
                }
                if !turn.is_empty() && event == "turn-end" {
                    s.ended_turns.push(turn.clone());
                    if s.ended_turns.len() > ENDED_TURNS {
                        s.ended_turns.remove(0);
                    }
                }
                // A turn starting anew (Codex may report a step of it first).
                let is_new_turn = turn.is_empty() || turn != s.turn;
                if !turn.is_empty() {
                    s.turn = turn;
                }
                if event == "turn-start" && is_new_turn {
                    out.turns += 1;
                    if !s.turn.is_empty() && !s.begun_turns.contains(&s.turn) {
                        s.begun_turns.push(s.turn.clone());
                        if s.begun_turns.len() > ENDED_TURNS {
                            s.begun_turns.remove(0);
                        }
                    }
                    // The turn before never said it ended (stopped without a
                    // word, or Antigravity's still waiting on a task of its
                    // own): it worked until its last event.
                    if let Some(since) = s.since.take() {
                        out.worked_ms += s.at.saturating_sub(since);
                    }
                    s.has_edited = false;
                    s.running.clear();
                    s.thought_at = Some(now);
                    s.files.clear();
                    s.reply.clear();
                    s.error = Value::Null;
                    // A list all done belongs to the work before.
                    if s.todos.iter().all(|t| t.status == "completed") {
                        s.todos.clear();
                    }
                }
                let tool_id = str_field(msg, "toolId", 200);
                match event {
                    "tool-start" => {
                        s.running.retain(|r| tool_id.is_empty() || r.id != tool_id);
                        if s.running.len() >= MAX_KEPT {
                            s.running.remove(0);
                        }
                        s.running.push(Running { id: tool_id, step: text(msg.get("step"), 80), since: now });
                    }
                    "tool-done" | "tool-failed" => {
                        // Its own start, or with no id to pair them by, the latest.
                        match s.running.iter().rposition(|r| r.id == tool_id) {
                            Some(at) => drop(s.running.remove(at)),
                            None if tool_id.is_empty() => drop(s.running.pop()),
                            None => {}
                        }
                        if s.running.is_empty() {
                            s.thought_at = Some(now);
                        }
                    }
                    _ => {}
                }
                if is_edit {
                    s.has_edited = true;
                }
                let file = str_field(msg, "file", 400);
                if !file.is_empty() && !s.files.contains(&file) && s.files.len() < MAX_KEPT {
                    s.files.push(file);
                }
                if let Some(todo) = msg.get("todo") {
                    apply_todo(s, todo);
                }
                let reply = str_field(msg, "reply", 160);
                if !reply.is_empty() {
                    s.reply = reply;
                }
                if msg.get("error").is_some() {
                    s.error = text(msg.get("error"), 80);
                }
                // Grok's prompt: what it waits on, the step under way.
                let detail = match s.running.last().filter(|_| event == "permission").map(|r| r.step.clone()) {
                    Some(step) if !step.is_null() && step != "" => json!({ "key": "detail.approve", "vars": { "what": step } }),
                    _ => detail,
                };
                let mut mood = mood;
                if mood == "done" && s.has_edited {
                    mood = "review";
                }
                if mood == "done" || mood == "review" {
                    s.has_edited = false;
                }
                let whose = s.whose();
                // How long the turn has gone on, should this end it.
                let busy_for = s.since.map(|since| now.saturating_sub(since));
                out.alerts.extend(set(s, mood, detail, now, hold).map(|m| (m, whose)));
                // Its end: done, or stopped by the person, having worked till then.
                if is_ending(mood) || (mood == "idle" && matches!(event, "turn-end" | "settle")) {
                    out.worked_ms += busy_for.unwrap_or(0);
                }
            }
            out.changed = true;
        }

        if let Some(react) = react {
            // A session's reaction is its agent's pet's (and the island's);
            // one of her own (no session) is every pet's.
            let has_session = !str_field(msg, "session", 200).is_empty();
            let agent = has_session.then(|| agent_key(&str_field(msg, "agent", 20)).to_string());
            out.react = Some(json!({ "react": react, "say": text(msg.get("say"), 80), "agent": agent }));
        }
        Some(out)
    }

    // You looked: every ending goes back to rest, quiet from now on. True
    // when anything did.
    pub fn seen(&mut self, now: u64, hold: Hold) -> bool {
        self.seen_where(now, hold, |_| true)
    }

    // One agent's pet looked at: its agent's endings only.
    pub fn seen_for(&mut self, agent: &str, now: u64, hold: Hold) -> bool {
        self.seen_where(now, hold, |s| agent_key(&s.agent) == agent)
    }

    fn seen_where(&mut self, now: u64, hold: Hold, of: impl Fn(&Session) -> bool) -> bool {
        let mut changed = false;
        for s in self.sessions.values_mut().filter(|s| is_ending(&s.mood) && of(s)) {
            set(s, "idle", Value::String(String::new()), now, hold);
            s.seen_at = Some(now);
            s.hidden |= s.closed;
            changed = true;
        }
        changed
    }

    // Endings held for a number of seconds go back to rest when it is up:
    // the island folds them away then, so they count as seen.
    pub fn tick(&mut self, now: u64, hold: Hold) -> bool {
        let mut changed = false;
        for s in self.sessions.values_mut() {
            if s.settle_at.is_some_and(|t| t <= now) {
                set(s, "idle", Value::String(String::new()), now, hold);
                s.seen_at = Some(now);
                s.hidden |= s.closed;
                changed = true;
            }
        }
        changed
    }

    // Sessions gone quiet for `hide_ms` (the idleHideMin setting) out of
    // sight, and those it lets back (a longer setting): left out of what the
    // pages are given, but kept. True when any changed.
    pub fn hide_quiet(&mut self, now: u64, hide_ms: u64) -> bool {
        let mut changed = false;
        for s in self.sessions.values_mut() {
            let hidden = (s.closed && !is_ending(&s.mood)) || !is_shown(&s.mood, s.event_at, s.seen_at, now, hide_ms);
            if s.hidden != hidden {
                s.hidden = hidden;
                changed = true;
            }
        }
        changed
    }

    pub fn check_stale(&mut self, now: u64, hold: Hold) -> bool {
        let mut changed = false;
        self.sessions.retain(|_, s| !(s.mood == "idle" && now.saturating_sub(s.at) > FORGET_MS));
        for s in self.sessions.values_mut() {
            let age = now.saturating_sub(s.at);
            let is_stuck = (s.mood == "working" || s.mood == "waiting") && age > STALE_MS;
            if is_stuck || (is_ending(&s.mood) && age > FORGET_MS) {
                set(s, "idle", Value::String(String::new()), now, hold);
                changed = true;
            }
        }
        changed
    }

    // The sessions in sight (hide_quiet).
    fn shown(&self) -> impl Iterator<Item = &Session> {
        self.sessions.values().filter(|s| !s.hidden)
    }

    // What the island shows: the session that most wants you, and the rest
    // in sight.
    pub fn get(&self, now: u64) -> Value {
        self.get_where(now, |_| true)
    }

    // What one agent's pet on the desktop shows: the same, of its agent's
    // sessions alone (none: at rest).
    pub fn get_for(&self, agent: &str, now: u64) -> Value {
        self.get_where(now, |s| agent_key(&s.agent) == agent)
    }

    fn get_where(&self, now: u64, of: impl Fn(&Session) -> bool) -> Value {
        let top = self
            .shown()
            .filter(|s| of(s))
            .fold(None::<&Session>, |best, s| match best {
                Some(b) if priority(&s.mood) < priority(&b.mood) => Some(b),
                Some(b) if priority(&s.mood) == priority(&b.mood) && s.at < b.at => Some(b),
                _ => Some(s),
            });
        let Some(top) = top else {
            let mut empty = view(&Session::new(String::new(), now));
            empty["others"] = json!(0);
            empty["sessions"] = json!(0);
            return empty;
        };
        let mut now = view(top);
        now["others"] = json!(self.shown().filter(|s| of(s) && s.id != top.id && s.mood != "idle").count());
        now["sessions"] = json!(self.shown().filter(|s| of(s)).count());
        now
    }

    // Every session in sight, the one that most wants you first: the island's
    // open lines, the settings' list.
    pub fn list(&self) -> Value {
        self.list_where(|_| true)
    }

    // An agent's sessions closed lately and out of her sight, the latest
    // first: the phone may still read them and go on with them.
    pub fn closed_for(&self, agent: &str, most: usize) -> Value {
        let mut all: Vec<&Session> = self.sessions.values().filter(|s| s.closed && s.hidden && agent_key(&s.agent) == agent).collect();
        all.sort_by(|a, b| b.at.cmp(&a.at));
        Value::Array(all.into_iter().take(most).map(view).collect())
    }

    // One agent's sessions so: its pet's bubble names only them.
    pub fn list_for(&self, agent: &str) -> Value {
        self.list_where(|s| agent_key(&s.agent) == agent)
    }

    fn list_where(&self, of: impl Fn(&Session) -> bool) -> Value {
        let mut all: Vec<&Session> = self.shown().filter(|s| of(s) && (!s.id.is_empty() || s.mood != "idle")).collect();
        all.sort_by(|a, b| priority(&b.mood).cmp(&priority(&a.mood)).then(b.at.cmp(&a.at)));
        Value::Array(all.into_iter().map(view).collect())
    }

    // The mood of the next session that wants something, after the one shown:
    // for the island's second bubble.
    pub fn second(&self) -> Option<String> {
        let mut busy: Vec<&Session> = self.shown().filter(|s| s.mood != "idle").collect();
        busy.sort_by(|a, b| priority(&b.mood).cmp(&priority(&a.mood)).then(b.at.cmp(&a.at)));
        busy.get(1).map(|s| s.mood.clone())
    }

    // Whether a session wants you: waiting on you, or an ending not yet seen.
    // Widgets keep to themselves meanwhile.
    pub fn wants_you(&self) -> bool {
        self.sessions.values().any(|s| s.mood == "waiting" || is_ending(&s.mood))
    }

    // What the sessions are called (projects, titles, tasks): kept out of a
    // crash report (crash.rs).
    pub fn names(&self) -> Vec<String> {
        self.sessions.values().flat_map(|s| [s.project.clone(), s.title.clone(), s.task.clone()]).filter(|n| !n.trim().is_empty()).collect()
    }

    // A session's agent (as the settings name it), transcript and the reply
    // it was last shown with: for its whole reply (reply.rs).
    pub fn transcript_of(&self, id: &str) -> Option<(String, String, String)> {
        let s = self.sessions.get(id)?;
        Some((agent_key(&s.agent).to_string(), s.transcript.clone(), s.reply.clone()))
    }

    // What the phone's conversation page needs of a session (talk.rs).
    pub fn talk_of(&self, id: &str) -> Option<Talk> {
        let s = self.sessions.get(id)?;
        Some(Talk { agent: agent_key(&s.agent).to_string(), transcript: s.transcript.clone(), cwd: s.cwd.clone(), chain: s.chain.clone(), reply: s.reply.clone(), mood: s.mood.clone() })
    }

    // Whether a session already knows where it runs.
    pub fn has_chain(&self, id: &str) -> bool {
        self.sessions.get(id).is_some_and(|s| !s.chain.is_empty())
    }

    // Where to go for a session: its chain, and the words its window's
    // title may hold (its name, its project).
    pub fn jump_target(&self, id: &str) -> Option<(jump::Chain, Vec<String>)> {
        let s = self.sessions.get(id).filter(|s| !s.chain.is_empty())?;
        Some((s.chain.clone(), vec![s.title.clone(), s.task.clone(), s.project.clone()]))
    }

    pub fn mood(&self, now: u64) -> String {
        self.get(now)["mood"].as_str().unwrap_or("idle").to_string()
    }

    pub fn mood_for(&self, agent: &str, now: u64) -> String {
        self.get_for(agent, now)["mood"].as_str().unwrap_or("idle").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(v: Value) -> Value {
        v
    }

    // A payload Grok Build sent (src-tauri/fixtures/grok), as events_grok.rs makes it a message.
    macro_rules! grok {
        ($name:literal) => {
            crate::events_grok::to_message(&serde_json::from_str::<Value>(include_str!(concat!("../fixtures/grok/", $name, ".json"))).unwrap()).unwrap()
        };
    }

    #[test]
    fn grok_waits_on_the_person_for_the_step_under_way() {
        let mut pet = Pet::default();
        pet.apply(&grok!("prompt"), 1, Hold::Seen);
        pet.apply(&grok!("pre-search-replace"), 2, Hold::Seen);
        let out = pet.apply(&grok!("notification-permission"), 3, Hold::Seen).unwrap();
        let now = pet.get(4);
        assert_eq!(now["mood"], "waiting");
        assert_eq!(now["detail"], json!({ "key": "detail.approve", "vars": { "what": { "key": "step.edit", "vars": { "file": "hello.txt" } } } }));
        assert_eq!(out.alerts.len(), 1);
        // Approved in Grok: the edit runs, and the turn ends with changes to see.
        pet.apply(&grok!("post-search-replace"), 5, Hold::Seen);
        assert_eq!(pet.get(6)["mood"], "working");
        pet.apply(&grok!("prompt-again"), 7, Hold::Seen);
        pet.apply(&grok!("pre-search-replace"), 8, Hold::Seen);
        pet.apply(&grok!("post-search-replace"), 9, Hold::Seen);
        pet.apply(&grok!("stop-end-turn"), 10, Hold::Seen);
        assert_eq!(pet.get(11)["mood"], "review");
        // With no step under way, it says only that Grok waits on the person.
        let mut pet = Pet::default();
        pet.apply(&grok!("notification-permission"), 1, Hold::Seen);
        assert_eq!(pet.get(2)["detail"], json!({ "key": "detail.approveThere", "vars": {} }));
    }

    #[test]
    fn grok_s_late_cancel_of_a_turn_before_changes_nothing() {
        let mut pet = Pet::default();
        pet.apply(&grok!("prompt"), 1, Hold::Seen);
        pet.apply(&grok!("prompt-again"), 2, Hold::Seen);
        // The first turn's cancel, reported after the second began.
        pet.apply(&grok!("cancelled-rejected"), 3, Hold::Seen);
        assert_eq!(pet.get(4)["mood"], "working");
        pet.apply(&grok!("stop-end-turn"), 5, Hold::Seen);
        assert_eq!(pet.get(6)["mood"], "done");
        // A turn whose prompt never came here ends as any would.
        let mut pet = Pet::default();
        pet.apply(&grok!("prompt"), 1, Hold::Seen);
        pet.apply(&grok!("stop-end-turn"), 2, Hold::Seen);
        assert_eq!(pet.get(3)["mood"], "done");
        pet.apply(&grok!("prompt-again"), 4, Hold::Seen);
        assert_eq!(pet.get(5)["mood"], "done", "its end came: a late copy of its prompt starts nothing");
        // Its own turn's cancel stops it.
        let mut pet = Pet::default();
        pet.apply(&grok!("prompt"), 1, Hold::Seen);
        pet.apply(&grok!("cancelled-rejected"), 2, Hold::Seen);
        assert_eq!(pet.get(3)["mood"], "idle");
    }

    #[test]
    fn grok_settling_ends_only_work_and_waiting() {
        // Still at work a minute after it went quiet: no longer.
        let mut pet = Pet::default();
        pet.apply(&grok!("prompt"), 1, Hold::Seen);
        let out = pet.apply(&grok!("notification-idle"), 60_000, Hold::Seen).unwrap();
        assert!(out.changed);
        assert_eq!(pet.get(60_001)["mood"], "idle");
        // An ending not yet seen stays.
        let mut pet = Pet::default();
        pet.apply(&grok!("prompt-again"), 1, Hold::Seen);
        pet.apply(&grok!("stop-end-turn"), 2, Hold::Seen);
        let out = pet.apply(&grok!("notification-idle"), 60_000, Hold::Seen).unwrap();
        assert!(!out.changed);
        assert_eq!(pet.get(60_001)["mood"], "done");
        // A session never seen is not made for it.
        let mut pet = Pet::default();
        pet.apply(&grok!("notification-idle"), 1, Hold::Seen);
        assert_eq!(pet.get(2)["sessions"], 0);
    }

    #[test]
    fn a_turn_that_edited_ends_in_review() {
        let mut pet = Pet::default();
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "turn-start" })), 1, Hold::Seen);
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "detail": "Edit", "event": "tool-done" })), 2, Hold::Seen);
        let out = pet.apply(&msg(json!({ "session": "a", "mood": "done" })), 5, Hold::Seen).unwrap();
        assert_eq!(pet.get(6)["mood"], "review");
        assert_eq!(pet.get(6)["took"], 4);
        assert_eq!(out.alerts, vec![("review".to_string(), String::new())]);
    }

    #[test]
    fn the_session_that_most_wants_you_shows() {
        let mut pet = Pet::default();
        pet.apply(&msg(json!({ "session": "a", "mood": "waiting", "project": "one" })), 1, Hold::Seen);
        pet.apply(&msg(json!({ "session": "b", "mood": "working", "project": "two" })), 2, Hold::Seen);
        let now = pet.get(3);
        assert_eq!(now["mood"], "waiting");
        assert_eq!(now["project"], "one");
        assert_eq!(now["others"], 1);
        assert_eq!(now["sessions"], 2);
    }

    #[test]
    fn endings_stay_until_seen_or_held_time() {
        let mut pet = Pet::default();
        pet.apply(&msg(json!({ "session": "a", "mood": "done" })), 1, Hold::Seen);
        assert!(!pet.tick(1_000_000, Hold::Seen));
        assert!(pet.seen(2, Hold::Seen));
        assert_eq!(pet.get(3)["mood"], "idle");

        pet.apply(&msg(json!({ "session": "a", "mood": "error" })), 10, Hold::Secs(8));
        assert!(!pet.tick(8_009, Hold::Secs(8)));
        assert!(pet.tick(8_010, Hold::Secs(8)));
        assert_eq!(pet.get(8_011)["mood"], "idle");
    }

    #[test]
    fn each_agent_s_pet_shows_its_own_sessions() {
        let mut pet = Pet::default();
        let hold = Hold::Secs(8);
        // Claude Code working (no agent), Codex waiting, Grok done.
        pet.apply(&msg(json!({ "session": "c", "project": "a", "mood": "working" })), 1, hold);
        pet.apply(&msg(json!({ "session": "x", "agent": "codex", "project": "b", "mood": "waiting" })), 2, hold);
        pet.apply(&msg(json!({ "session": "g", "agent": "grok", "project": "c", "mood": "done" })), 3, hold);
        // The island: the one that most wants you, of all.
        assert_eq!((pet.get(4)["id"].as_str(), pet.get(4)["sessions"].as_u64()), (Some("x"), Some(3)));
        // Each agent's pet: its own, counted among its own.
        let of = |agent: &str| {
            let v = pet.get_for(agent, 4);
            (v["id"].as_str().unwrap_or("").to_string(), v["mood"].as_str().unwrap_or("").to_string(), v["sessions"].as_u64(), v["others"].as_u64())
        };
        assert_eq!(of("claude"), ("c".into(), "working".into(), Some(1), Some(0)));
        assert_eq!(of("codex"), ("x".into(), "waiting".into(), Some(1), Some(0)));
        assert_eq!(of("grok"), ("g".into(), "done".into(), Some(1), Some(0)));
        // None of its own: at rest, as the island with none at all.
        assert_eq!(of("agy"), (String::new(), "idle".into(), Some(0), Some(0)));
        assert_eq!(pet.mood_for("codex", 4), "waiting");
        assert_eq!(pet.list_for("grok").as_array().map(Vec::len), Some(1));
        assert_eq!(pet.list().as_array().map(Vec::len), Some(3));

        // Grok's pet looked at: Grok's ending seen, Codex's waiting untouched.
        pet.apply(&msg(json!({ "session": "x2", "agent": "codex", "project": "d", "mood": "error" })), 5, hold);
        assert!(pet.seen_for("grok", 6, hold));
        assert_eq!(pet.mood_for("grok", 7), "idle");
        assert_eq!(pet.get_for("codex", 7)["id"], "x");
        assert!(pet.list_for("codex").as_array().unwrap().iter().any(|s| s["mood"] == "error"));
        // Nothing of Grok's left to see.
        assert!(!pet.seen_for("grok", 8, hold));
    }

    #[test]
    fn a_session_s_reaction_names_its_agent_and_one_of_her_own_none() {
        let mut pet = Pet::default();
        let out = pet.apply(&msg(json!({ "session": "x", "agent": "codex", "mood": "idle", "react": "wave" })), 1, Hold::Seen).unwrap();
        assert_eq!(out.react.unwrap()["agent"], "codex");
        let out = pet.apply(&msg(json!({ "session": "c", "mood": "working", "react": "failed" })), 2, Hold::Seen).unwrap();
        assert_eq!(out.react.unwrap()["agent"], "claude");
        let out = pet.apply(&msg(json!({ "react": "jump" })), 3, Hold::Seen).unwrap();
        assert!(out.react.unwrap()["agent"].is_null());
    }

    #[test]
    fn a_reaction_alone_changes_no_mood() {
        let mut pet = Pet::default();
        let out = pet.apply(&msg(json!({ "react": "jump", "say": { "key": "say.taskDone", "vars": { "task": "x", "n": 3, "bad": [] } } })), 1, Hold::Seen).unwrap();
        assert!(!out.changed);
        assert_eq!(out.react.unwrap()["say"], json!({ "key": "say.taskDone", "vars": { "task": "x", "n": "3" } }));
        assert!(pet.apply(&msg(json!({ "mood": "sleepy" })), 1, Hold::Seen).is_none());
    }

    #[test]
    fn stale_work_gives_up_and_quiet_sessions_are_forgotten() {
        let mut pet = Pet::default();
        pet.apply(&msg(json!({ "session": "a", "mood": "working" })), 0, Hold::Seen);
        pet.apply(&msg(json!({ "session": "b", "mood": "idle" })), 0, Hold::Seen);
        assert!(pet.check_stale(STALE_MS + 1, Hold::Seen));
        assert_eq!(pet.get(STALE_MS + 2)["mood"], "idle");
        pet.check_stale(FORGET_MS + 1, Hold::Seen);
        assert_eq!(pet.get(FORGET_MS + 2)["sessions"], 1);
        pet.apply(&msg(json!({ "session": "a", "mood": "idle", "event": "session-end" })), 1, Hold::Seen);
        assert_eq!(pet.get(FORGET_MS + 3)["sessions"], 0);
    }

    // A non-interactive run ends its session as it finishes (claude -p,
    // codex exec): its ending in sight, then out of sight but kept.
    #[test]
    fn a_session_closed_as_it_ended_is_kept() {
        let mut pet = Pet::default();
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "turn-start" })), 0, Hold::Secs(8));
        pet.apply(&msg(json!({ "session": "a", "mood": "done", "event": "turn-end", "reply": "PONG" })), 10, Hold::Secs(8));
        pet.apply(&msg(json!({ "session": "a", "mood": "idle", "event": "session-end" })), 11, Hold::Secs(8));
        assert_eq!((pet.get(12)["mood"].as_str(), pet.get(12)["reply"].as_str()), (Some("done"), Some("PONG")));
        assert!(pet.transcript_of("a").is_some());
        assert!(pet.tick(10 + 8000, Hold::Secs(8)));
        assert_eq!(pet.get(10 + 8001)["sessions"], 0);
        assert!(!pet.hide_quiet(10 + 9000, 0));
        // Its whole reply still there, and the phone still has it.
        assert!(pet.transcript_of("a").is_some_and(|(_, _, reply)| reply == "PONG"));
        assert_eq!(pet.closed_for("claude", 5)[0]["id"], "a");
        // Closed at work or at rest: out of her sight at once, the phone's still.
        pet.apply(&msg(json!({ "session": "c", "mood": "working", "agent": "codex" })), 30, Hold::Seen);
        pet.apply(&msg(json!({ "session": "c", "mood": "idle", "event": "session-end" })), 31, Hold::Seen);
        assert_eq!(pet.get_for("codex", 32)["sessions"], 0);
        assert!(!pet.hide_quiet(33, 0));
        assert_eq!(pet.get_for("codex", 34)["sessions"], 0);
        assert_eq!(pet.closed_for("codex", 5).as_array().map(Vec::len), Some(1));
        // Resumed (the phone's words, the computer's /resume): in sight again.
        pet.apply(&msg(json!({ "session": "c", "mood": "working", "agent": "codex", "event": "turn-start" })), 40, Hold::Seen);
        assert_eq!(pet.get_for("codex", 41)["sessions"], 1);
        assert_eq!(pet.closed_for("codex", 5).as_array().map(Vec::len), Some(0));
        // Forgotten after a while at rest, as any other.
        pet.apply(&msg(json!({ "session": "c", "mood": "idle", "event": "session-end" })), 50, Hold::Seen);
        pet.check_stale(50 + FORGET_MS + 1, Hold::Seen);
        assert!(pet.transcript_of("c").is_none());
    }

    #[test]
    fn a_transcript_is_kept_for_the_whole_reply() {
        let mut pet = Pet::default();
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "agent": "grok", "transcript": "C:\\t\\updates.jsonl" })), 0, Hold::Seen);
        pet.apply(&msg(json!({ "session": "a", "mood": "done", "reply": "short" })), 1, Hold::Seen);
        assert_eq!(pet.transcript_of("a"), Some(("grok".to_string(), "C:\\t\\updates.jsonl".to_string(), "short".to_string())));
        assert_eq!(pet.transcript_of("b"), None);
    }

    #[test]
    fn the_step_is_the_latest_tool_running_or_thinking() {
        let mut pet = Pet::default();
        let edit = json!({ "key": "step.edit", "vars": { "file": "a.rs" } });
        let agent = json!({ "key": "step.agent", "vars": { "what": "Explore" } });
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "turn-start" })), 10, Hold::Seen);
        assert_eq!(pet.get(11)["step"]["key"], "step.thinking");
        assert_eq!(pet.get(11)["stepSince"], 10);
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "tool-start", "toolId": "1", "step": agent })), 20, Hold::Seen);
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "tool-start", "toolId": "2", "step": edit })), 30, Hold::Seen);
        assert_eq!((pet.get(31)["step"].clone(), pet.get(31)["stepSince"].clone()), (edit, json!(30)));
        // The subagent's tool ends: back to the agent, not to thinking.
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "tool-done", "toolId": "2", "detail": "Edit", "file": "D:/p/a.rs" })), 40, Hold::Seen);
        assert_eq!(pet.get(41)["step"], agent);
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "tool-failed", "toolId": "1" })), 50, Hold::Seen);
        assert_eq!((pet.get(51)["step"]["key"].clone(), pet.get(51)["stepSince"].clone()), (json!("step.thinking"), json!(50)));
        assert_eq!(pet.get(51)["files"], 1);
        // Waiting or done, there is no step.
        pet.apply(&msg(json!({ "session": "a", "mood": "done", "reply": "改好了。" })), 60, Hold::Seen);
        let now = pet.get(61);
        assert_eq!((now["mood"].as_str(), now["step"].is_null(), now["reply"].as_str(), now["files"].as_u64()), (Some("review"), true, Some("改好了。"), Some(1)));
        // The next turn starts clean.
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "turn-start" })), 70, Hold::Seen);
        assert_eq!((pet.get(71)["files"].as_u64(), pet.get(71)["reply"].as_str()), (Some(0), Some("")));
    }

    #[test]
    fn a_session_is_named_by_its_title_or_else_its_task() {
        let mut pet = Pet::default();
        let out = pet.apply(&msg(json!({ "session": "a", "project": "pet", "mood": "waiting" })), 1, Hold::Seen).unwrap();
        assert_eq!(out.alerts, vec![("waiting".to_string(), "pet".to_string())]);
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "turn-start", "task": "修复岛" })), 2, Hold::Seen);
        assert_eq!(pet.get(3)["name"], "修复岛");
        pet.apply(&msg(json!({ "session": "a", "mood": "done", "title": "岛的设置" })), 4, Hold::Seen);
        assert_eq!(pet.get(5)["name"], "岛的设置");
        // A new task does not take over a title.
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "turn-start", "task": "下一件事" })), 6, Hold::Seen);
        assert_eq!(pet.list()[0]["name"], "岛的设置");
    }

    #[test]
    fn to_do_lists_count_what_is_done() {
        let mut pet = Pet::default();
        let todo = |pet: &mut Pet, todo: Value| {
            pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "tool-start", "todo": todo })), 1, Hold::Seen);
            pet.get(2)["todo"].clone()
        };
        let list = json!({ "set": [
            { "text": "A", "active": "Doing A", "status": "completed" },
            { "text": "B", "active": "Doing B", "status": "in_progress" },
            { "text": "C", "active": "", "status": "pending" },
        ] });
        assert_eq!(todo(&mut pet, list), json!({ "done": 1, "total": 3, "active": "Doing B" }));

        let mut pet = Pet::default();
        todo(&mut pet, json!({ "add": { "text": "A", "active": "" } }));
        todo(&mut pet, json!({ "add": { "text": "B", "active": "Doing B" } }));
        todo(&mut pet, json!({ "update": { "id": "1", "status": "completed" } }));
        assert_eq!(todo(&mut pet, json!({ "update": { "id": "2", "status": "in_progress" } })), json!({ "done": 1, "total": 2, "active": "Doing B" }));
        assert_eq!(todo(&mut pet, json!({ "update": { "id": "2", "status": "deleted" } })), json!({ "done": 1, "total": 1, "active": "" }));
        // All done, and a new turn: the list is gone.
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "turn-start" })), 3, Hold::Seen);
        assert!(pet.get(4)["todo"].is_null());
    }

    #[test]
    fn texts_may_hold_one_text_inside() {
        let nested = json!({ "key": "detail.approve", "vars": { "what": { "key": "step.command", "vars": { "what": "git push", "deeper": { "key": "x" } } } } });
        assert_eq!(text(Some(&nested), 80), json!({ "key": "detail.approve", "vars": { "what": { "key": "step.command", "vars": { "what": "git push" } } } }));
    }

    // A Codex message: session c, turn t.
    fn codex(turn: &str, more: Value) -> Value {
        let mut m = json!({ "session": "c", "agent": "codex", "turn": turn });
        for (k, v) in more.as_object().unwrap() {
            m[k] = v.clone();
        }
        m
    }

    #[test]
    fn a_codex_turn_with_a_patch_ends_in_review() {
        let mut pet = Pet::default();
        pet.apply(&codex("t1", json!({ "mood": "working", "event": "turn-start" })), 1, Hold::Seen);
        pet.apply(&codex("t1", json!({ "mood": "working", "detail": "apply_patch", "event": "tool-done" })), 2, Hold::Seen);
        pet.apply(&codex("t1", json!({ "mood": "done", "event": "turn-end" })), 3, Hold::Seen);
        assert_eq!(pet.get(4)["mood"], "review");
        assert_eq!(pet.get(4)["agent"], "codex");
        assert_eq!(pet.list()[0]["agent"], "codex");
    }

    #[test]
    fn late_codex_events_change_nothing_but_a_late_edit() {
        let mut pet = Pet::default();
        // The turn's own start comes after one of its steps: it still counts the edit.
        pet.apply(&codex("t1", json!({ "mood": "working", "detail": "apply_patch", "event": "tool-done" })), 1, Hold::Seen);
        pet.apply(&codex("t1", json!({ "mood": "working", "event": "turn-start" })), 2, Hold::Seen);
        pet.apply(&codex("t1", json!({ "mood": "done", "event": "turn-end" })), 3, Hold::Seen);
        assert_eq!(pet.get(4)["mood"], "review");

        // A step after its turn ended: she stays done, not back at work.
        pet.apply(&codex("t2", json!({ "mood": "working", "event": "turn-start" })), 10, Hold::Seen);
        pet.apply(&codex("t2", json!({ "mood": "done", "event": "turn-end" })), 11, Hold::Seen);
        pet.apply(&codex("t2", json!({ "mood": "working", "detail": "Bash", "event": "tool-start", "step": { "key": "step.command", "vars": { "what": "ls" } } })), 12, Hold::Seen);
        assert_eq!(pet.get(13)["mood"], "done");
        // A late edit makes that done a review.
        pet.apply(&codex("t2", json!({ "mood": "working", "detail": "apply_patch", "event": "tool-done" })), 14, Hold::Seen);
        assert_eq!(pet.get(15)["mood"], "review");

        // Interrupted: back to rest, and its stragglers too.
        pet.apply(&codex("t3", json!({ "mood": "working", "event": "turn-start" })), 20, Hold::Seen);
        pet.apply(&codex("t3", json!({ "mood": "idle", "event": "turn-end" })), 21, Hold::Seen);
        pet.apply(&codex("t3", json!({ "mood": "working", "detail": "Bash" })), 22, Hold::Seen);
        assert_eq!(pet.get(23)["mood"], "idle");
        // The next turn works as ever.
        pet.apply(&codex("t4", json!({ "mood": "working", "event": "turn-start" })), 30, Hold::Seen);
        assert_eq!(pet.get(31)["mood"], "working");
    }

    #[test]
    fn a_session_keeps_where_it_runs() {
        let mut pet = Pet::default();
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "project": "pet", "chain": [[10, 5], [8, 3]] })), 1, Hold::Seen);
        assert!(pet.has_chain("a"));
        assert_eq!(pet.get(2)["jump"], true);
        // A message without one keeps it.
        pet.apply(&msg(json!({ "session": "a", "mood": "done", "title": "岛" })), 3, Hold::Seen);
        assert_eq!(pet.jump_target("a"), Some((vec![(10, 5), (8, 3)], vec!["岛".into(), String::new(), "pet".into()])));
        pet.apply(&msg(json!({ "session": "b", "mood": "working" })), 4, Hold::Seen);
        assert!(!pet.has_chain("b") && pet.jump_target("b").is_none());
        assert_eq!(pet.list().as_array().unwrap().iter().find(|s| s["id"] == "b").unwrap()["jump"], false);
    }

    #[test]
    fn a_turn_counts_for_today_and_an_ending_wants_you() {
        let mut pet = Pet::default();
        let out = pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "turn-start" })), 1_000, Hold::Seen).unwrap();
        assert_eq!((out.turns, out.worked_ms), (1, 0));
        assert!(!pet.wants_you());
        let out = pet.apply(&msg(json!({ "session": "a", "mood": "done" })), 61_000, Hold::Seen).unwrap();
        assert_eq!((out.turns, out.worked_ms), (0, 60_000));
        assert!(pet.wants_you());
        pet.seen(62_000, Hold::Seen);
        assert!(!pet.wants_you());
    }

    #[test]
    fn a_stopped_turn_and_one_that_never_ended_count_too() {
        let mut pet = Pet::default();
        // Stopped by the person (Codex's Interrupt): it worked till then.
        pet.apply(&codex("t1", json!({ "mood": "working", "event": "turn-start" })), 1_000, Hold::Seen);
        let out = pet.apply(&codex("t1", json!({ "mood": "idle", "event": "turn-end" })), 21_000, Hold::Seen).unwrap();
        assert_eq!(out.worked_ms, 20_000);
        // A turn that never said it ended, then the next: up to its last event.
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "turn-start" })), 30_000, Hold::Seen);
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "detail": "Bash", "event": "tool-done" })), 40_000, Hold::Seen);
        let out = pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "turn-start" })), 100_000, Hold::Seen).unwrap();
        assert_eq!((out.turns, out.worked_ms), (1, 10_000));
        let out = pet.apply(&msg(json!({ "session": "a", "mood": "done" })), 105_000, Hold::Seen).unwrap();
        assert_eq!(out.worked_ms, 5_000);
        // Rest that ends no turn counts nothing.
        pet.apply(&msg(json!({ "session": "b", "mood": "working", "event": "turn-start" })), 200_000, Hold::Seen);
        assert_eq!(pet.apply(&msg(json!({ "session": "b", "mood": "idle" })), 210_000, Hold::Seen).unwrap().worked_ms, 0);
    }

    const HIDE: u64 = IDLE_HIDE_MIN * 60 * 1000;

    fn ids(pet: &Pet) -> Vec<String> {
        let mut ids: Vec<String> = pet.list().as_array().unwrap().iter().map(|s| s["id"].as_str().unwrap().to_string()).collect();
        ids.sort();
        ids
    }

    #[test]
    fn only_a_session_at_rest_goes_out_of_sight() {
        // At work, waiting on you, an ending not yet seen: in sight however long.
        for mood in ["working", "waiting", "done", "review", "error"] {
            assert!(is_shown(mood, 0, None, FORGET_MS * 10, HIDE), "{mood}");
        }
        // At rest: until quiet that long since its last event,
        assert!(is_shown("idle", 1_000, None, 1_000 + HIDE - 1, HIDE));
        assert!(!is_shown("idle", 1_000, None, 1_000 + HIDE, HIDE));
        // or since its ending was seen, whichever came later.
        assert!(is_shown("idle", 1_000, Some(5_000), 1_000 + HIDE, HIDE));
        assert!(!is_shown("idle", 1_000, Some(5_000), 5_000 + HIDE, HIDE));
        assert!(!is_shown("idle", 9_000, Some(5_000), 9_000 + HIDE, HIDE));
        // Always, when set so.
        assert!(is_shown("idle", 0, None, FORGET_MS * 10, 0));
    }

    #[test]
    fn a_seen_ending_goes_out_of_sight_and_comes_back_as_it_was() {
        let mut pet = Pet::default();
        pet.apply(&msg(json!({ "session": "a", "project": "pet", "mood": "working", "event": "turn-start", "task": "修复岛" })), 1_000, Hold::Seen);
        pet.apply(&msg(json!({ "session": "a", "mood": "done" })), 61_000, Hold::Seen);
        pet.apply(&msg(json!({ "session": "b", "mood": "working", "event": "turn-start" })), 61_000, Hold::Seen);
        // Not seen yet: kept past the time, as is the one at work.
        let seen = 61_000 + HIDE * 2;
        assert!(!pet.hide_quiet(seen, HIDE));
        assert_eq!(ids(&pet), ["a", "b"]);
        // Seen: quiet from then on, and out of sight once it has been that long.
        pet.seen(seen, Hold::Seen);
        assert!(!pet.hide_quiet(seen + HIDE - 1, HIDE));
        assert!(pet.hide_quiet(seen + HIDE, HIDE));
        assert_eq!(ids(&pet), ["b"]);
        let now = pet.get(seen + HIDE);
        assert_eq!((now["id"].as_str(), now["sessions"].as_u64(), now["others"].as_u64()), (Some("b"), Some(1), Some(0)));
        // Kept all the same: its names stay out of a crash report.
        assert!(pet.names().contains(&"修复岛".to_string()));
        // Its next event brings it back at once, as it was.
        let back = seen + HIDE + 10;
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "turn-start" })), back, Hold::Seen);
        assert!(!pet.hide_quiet(back, HIDE));
        let a = pet.list().as_array().unwrap().iter().find(|s| s["id"] == "a").cloned().unwrap();
        assert_eq!((a["name"].as_str(), a["project"].as_str(), a["mood"].as_str()), (Some("修复岛"), Some("pet"), Some("working")));
        assert_eq!(pet.get(back)["sessions"], 2);
    }

    #[test]
    fn an_ending_folded_away_counts_from_then() {
        let mut pet = Pet::default();
        pet.apply(&msg(json!({ "session": "a", "mood": "error" })), 0, Hold::Secs(8));
        assert!(pet.tick(8_000, Hold::Secs(8)));
        assert!(!pet.hide_quiet(8_000 + HIDE - 1, HIDE));
        assert!(pet.hide_quiet(8_000 + HIDE, HIDE));
        assert!(ids(&pet).is_empty());
        assert_eq!(pet.get(8_000 + HIDE)["sessions"], 0);
        // Always shown, set so: back in sight.
        assert!(pet.hide_quiet(8_000 + HIDE, 0));
        assert_eq!(ids(&pet), ["a"]);
    }

    #[test]
    fn work_given_up_and_endings_never_seen_count_from_their_last_event() {
        let mut pet = Pet::default();
        pet.apply(&msg(json!({ "session": "a", "mood": "working" })), 0, Hold::Seen);
        pet.apply(&msg(json!({ "session": "b", "mood": "done" })), 0, Hold::Seen);
        // Stuck work gone to rest stays in sight until quiet the time from its last event.
        pet.check_stale(STALE_MS + 1, Hold::Seen);
        assert!(!pet.hide_quiet(STALE_MS + 1, HIDE));
        assert!(pet.hide_quiet(HIDE, HIDE));
        assert_eq!(ids(&pet), ["b"]);
        // The ending nobody saw, given up after FORGET_MS: out of sight at once.
        pet.check_stale(FORGET_MS + 1, Hold::Seen);
        assert!(pet.hide_quiet(FORGET_MS + 1, HIDE));
        assert!(ids(&pet).is_empty());
        // Forgotten as before, out of sight or not.
        assert_eq!(pet.sessions.len(), 2);
        pet.check_stale(STALE_MS + 1 + FORGET_MS + 1, Hold::Seen);
        assert_eq!(pet.sessions.len(), 1);
    }
}
