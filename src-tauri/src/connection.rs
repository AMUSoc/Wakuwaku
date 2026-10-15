// How Claude Code and Codex reach her: the plugin, or hooks written into
// Claude Code's settings.json; hooks written into Codex's hooks.json. Read on
// demand (the person may change any of them at any time), and written only
// when they ask, from the settings.
//
// Every Claude Code event is an HTTP hook: Claude Code POSTs it to her port,
// no process at all. SessionStart also gets a command, run in the background
// (async), that starts her when she is not up, which no HTTP hook can do.
// That command is this program with ENSURE_FLAG, given as an argument list
// (no shell). Codex runs only commands: see "Codex" below.
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};

pub const ENSURE_FLAG: &str = "--wakuwaku-ensure-running";
pub const CODEX_FLAG: &str = "--wakuwaku-codex-hook";
pub const AGY_FLAG: &str = "--wakuwaku-agy-hook";
const URL_MARK: &str = "from=wakuwaku";
// What the app wrote before it was called Wakuwaku (claude-pets).
pub const LEGACY_ENSURE_FLAG: &str = "--claude-pets-ensure-running";
const LEGACY_URL_MARK: &str = "from=claude-pets";
pub const PLUGIN_ID: &str = "wakuwaku@wakuwaku";
pub const PLUGIN_COMMANDS: [&str; 2] = ["/plugin marketplace add zhoupengjie/wakuwaku", "/plugin install wakuwaku@wakuwaku"];

// Each event Claude Code reports, and whether it takes a matcher; the prompt
// waits for the person (up to 300 s) instead of the default 2.
const EVENTS: [(&str, bool, u64); 11] = [
    ("SessionStart", false, 2),
    ("SessionEnd", false, 2),
    ("UserPromptSubmit", false, 2),
    ("PreToolUse", true, 2),
    ("PostToolUse", true, 2),
    ("PostToolUseFailure", true, 2),
    ("PermissionRequest", true, 300),
    ("Elicitation", false, 2),
    ("TaskCompleted", false, 2),
    ("Stop", false, 2),
    ("StopFailure", false, 2),
];

pub fn claude_settings_file() -> PathBuf {
    let dir = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).unwrap_or_default()).join(".claude"));
    dir.join("settings.json")
}

// Whether Claude Code is on this computer, as far as can be told: its
// folder there (~/.claude, or CLAUDE_CONFIG_DIR).
pub fn claude_here() -> bool {
    claude_settings_file().parent().is_some_and(Path::is_dir)
}

// The copy write_json keeps of a settings file (made once, beside it).
pub fn backup_of(file: &Path) -> PathBuf {
    PathBuf::from(format!("{}.wakuwaku.bak", file.display()))
}

// A path as the person reads it: ~ for the home folder when it is under it.
pub fn shown_path(file: &Path) -> String {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from);
    match home.as_deref().filter(|h| !h.as_os_str().is_empty()).and_then(|h| file.strip_prefix(h).ok()) {
        Some(rest) => format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display()),
        None => file.display().to_string(),
    }
}

// What kept a settings file from being written, for what to do about it:
// locked (another program has it open), invalid (not JSON), other. From
// the error's text: io's ends "(os error 32)" (or 33) whatever the
// language, serde_json's says where, "at line 3 column 5".
pub fn error_kind(error: &str) -> &'static str {
    if error.contains(AGY_NO_NAME) {
        "path"
    } else if error.contains("(os error 32)") || error.contains("(os error 33)") {
        "locked"
    } else if error.contains(" at line ") && error.contains(" column ") {
        "invalid"
    } else {
        "other"
    }
}

// A JSON settings file: {} when there is none, Err when unreadable.
// A file as last read, kept while its size and time stay (a look at the
// file's metadata, not its contents): the settings' snapshot asks for
// Claude Code's settings.json three times and Codex's hooks.json twice,
// up to four times a second while the settings are open (2026-10-13).
type Stamp = Option<(u64, std::time::SystemTime)>;
static READ: Mutex<Option<std::collections::HashMap<PathBuf, (Stamp, Result<Value, String>)>>> = Mutex::new(None);

fn read_json(file: &Path) -> Result<Value, String> {
    let stamp: Stamp = fs::metadata(file).ok().map(|m| (m.len(), m.modified().unwrap_or(std::time::UNIX_EPOCH)));
    let mut read = READ.lock().unwrap_or_else(|e| e.into_inner());
    let read = read.get_or_insert_with(std::collections::HashMap::new);
    if let Some((at, value)) = read.get(file) {
        if *at == stamp {
            return value.clone();
        }
    }
    let value = read_json_now(file);
    read.insert(file.to_path_buf(), (stamp, value.clone()));
    value
}

fn read_json_now(file: &Path) -> Result<Value, String> {
    match fs::read_to_string(file) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(e.to_string()),
    }
}

// Claude Code's settings.
fn read_settings() -> Result<Value, String> {
    read_json(&claude_settings_file())
}

// Write a settings file, backed up once beside itself.
fn write_json(file: &Path, value: &Value) -> Result<(), String> {
    let backup = backup_of(file);
    if file.exists() && !backup.exists() {
        fs::copy(file, &backup).map_err(|e| e.to_string())?;
    }
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    let written = fs::write(file, format!("{text}\n")).map_err(|e| e.to_string());
    // As last read no longer: read again next time, whatever its stamp.
    if let Some(read) = READ.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        read.remove(file);
    }
    written
}

// This program, as the hook command starts it.
pub fn launch_command() -> String {
    std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
}

fn hook_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/hook?{URL_MARK}")
}

// A path as Windows compares them: one slash, any case.
fn same_path(p: &str) -> String {
    if cfg!(windows) { p.replace('\\', "/").to_lowercase() } else { p.to_string() }
}

// Ours: what this version installs, and what older ones did.
fn is_ours(hook: &Value) -> bool {
    let mut text = [hook.get("url"), hook.get("command")].iter().flatten().map(|v| v.to_string()).collect::<Vec<_>>().join(" ");
    for arg in hook.get("args").and_then(Value::as_array).into_iter().flatten() {
        text.push(' ');
        text.push_str(&arg.to_string());
    }
    [URL_MARK, ENSURE_FLAG, CODEX_FLAG, LEGACY_URL_MARK, LEGACY_ENSURE_FLAG, "claude-hook.js"].iter().any(|m| text.contains(m))
}

// Our hooks, event by event.
fn ours_by_event(settings: &Value) -> Map<String, Value> {
    let mut found = Map::new();
    for (event, groups) in settings.get("hooks").and_then(Value::as_object).into_iter().flatten() {
        let ours: Vec<Value> = groups
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|g| g.get("hooks").and_then(Value::as_array).cloned().unwrap_or_default())
            .filter(is_ours)
            .collect();
        if !ours.is_empty() {
            found.insert(event.clone(), Value::Array(ours));
        }
    }
    found
}

// Our entries dropped from every event, and events left empty.
fn strip(hooks: Option<&Value>) -> Map<String, Value> {
    let mut out = Map::new();
    let Some(Value::Object(hooks)) = hooks else { return out };
    for (event, groups) in hooks {
        let Value::Array(groups) = groups else {
            out.insert(event.clone(), groups.clone());
            continue;
        };
        let kept: Vec<Value> = groups
            .iter()
            .filter_map(|group| {
                let mut group = group.clone();
                let hooks: Vec<Value> = group.get("hooks").and_then(Value::as_array).into_iter().flatten().filter(|h| !is_ours(h)).cloned().collect();
                if hooks.is_empty() {
                    return None;
                }
                group["hooks"] = Value::Array(hooks);
                Some(group)
            })
            .collect();
        if !kept.is_empty() {
            out.insert(event.clone(), Value::Array(kept));
        }
    }
    out
}

// What we add, event by event. http_only: no command, nothing starts her.
fn entries(port: u16, http_only: bool) -> Map<String, Value> {
    let mut out = Map::new();
    for (event, matcher, timeout) in EVENTS {
        // Claude Code runs no HTTP hook for SessionStart: the starter command
        // passes that one on.
        if event == "SessionStart" {
            continue;
        }
        let hooks = json!([{ "type": "http", "url": hook_url(port), "timeout": timeout }]);
        let group = if matcher { json!({ "matcher": "*", "hooks": hooks }) } else { json!({ "hooks": hooks }) };
        out.insert(event.into(), json!([group]));
    }
    if !http_only {
        out.insert(
            "SessionStart".into(),
            json!([{ "hooks": [{ "type": "command", "command": launch_command(), "args": [ENSURE_FLAG], "async": true, "timeout": 15 }] }]),
        );
    }
    out
}

pub fn install(settings: &Value, port: u16, http_only: bool) -> Value {
    let mut hooks = strip(settings.get("hooks"));
    for (event, groups) in entries(port, http_only) {
        let mut all = hooks.get(&event).and_then(Value::as_array).cloned().unwrap_or_default();
        all.extend(groups.as_array().cloned().unwrap_or_default());
        hooks.insert(event, Value::Array(all));
    }
    let mut out = settings.as_object().cloned().unwrap_or_default();
    out.insert("hooks".into(), Value::Object(hooks));
    Value::Object(out)
}

pub fn uninstall(settings: &Value) -> Value {
    let mut out = settings.as_object().cloned().unwrap_or_default();
    let hooks = strip(settings.get("hooks"));
    if hooks.is_empty() {
        out.remove("hooks");
    } else {
        out.insert("hooks".into(), Value::Object(hooks));
    }
    Value::Object(out)
}

// How our entries stand against what this copy would install:
//   missing / ok / httpOnly (nothing starts her) / stale (another port or
//   copy, or an old version) / partial (some events lack them).
pub fn status_of(settings: &Value, port: u16) -> &'static str {
    let found = ours_by_event(settings);
    if !EVENTS.iter().any(|(name, ..)| found.contains_key(*name)) {
        return "missing";
    }
    let list = |name: &str| found.get(name).and_then(Value::as_array).cloned().unwrap_or_default();
    let has_flag = |h: &Value| h.get("args").and_then(Value::as_array).is_some_and(|a| a.iter().any(|x| x == ENSURE_FLAG));
    let is_old = EVENTS.iter().any(|(name, ..)| list(name).iter().any(|h| h["type"] != "http" && !has_flag(h)))
        || list("SessionStart").iter().any(|h| h["type"] == "http");
    let want_url = hook_url(port);
    let is_other_port = EVENTS.iter().any(|(name, ..)| list(name).iter().any(|h| h["type"] == "http" && h["url"] != want_url.as_str()));
    let starters: Vec<Value> = list("SessionStart").into_iter().filter(|h| h["type"] == "command").collect();
    let want = vec![same_path(&launch_command()), same_path(ENSURE_FLAG)];
    let is_other_copy = starters.iter().any(|h| {
        let mut got = vec![same_path(h["command"].as_str().unwrap_or(""))];
        got.extend(h.get("args").and_then(Value::as_array).into_iter().flatten().map(|a| same_path(a.as_str().unwrap_or(""))));
        got != want
    });
    if is_old || is_other_port || is_other_copy {
        return "stale";
    }
    if EVENTS.iter().any(|(name, ..)| *name != "SessionStart" && !found.contains_key(*name)) {
        return "partial";
    }
    if starters.is_empty() { "httpOnly" } else { "ok" }
}

pub fn hooks_status(port: u16) -> &'static str {
    match read_settings() {
        Ok(settings) => status_of(&settings, port),
        Err(_) => "unreadable",
    }
}

pub fn is_plugin_enabled() -> bool {
    read_settings().is_ok_and(|s| s.get("enabledPlugins").and_then(|p| p.get(PLUGIN_ID)) == Some(&Value::Bool(true)))
}

// 'plugin', 'hooks' (written into settings.json), 'both' (every event twice), or 'none'.
pub fn connection(port: u16) -> &'static str {
    let plugin = is_plugin_enabled();
    let hooks = !matches!(hooks_status(port), "missing" | "unreadable");
    match (plugin, hooks) {
        (true, true) => "both",
        (true, false) => "plugin",
        (false, true) => "hooks",
        (false, false) => "none",
    }
}

// Change Claude Code's settings.json, backed up once beside itself.
// action: install, install-http, remove.
pub fn write_hooks(action: &str, port: u16) -> Result<(), String> {
    // Read from the disk, not as last read: nothing of someone else's is written back.
    let before = read_json_now(&claude_settings_file())?;
    let after = match action {
        "remove" => uninstall(&before),
        "install" => install(&before, port, false),
        "install-http" => install(&before, port, true),
        _ => return Err(format!("unknown action {action}")),
    };
    write_json(&claude_settings_file(), &after)
}


// --- Codex: hooks in ~/.codex/hooks.json ---------------------------------------------
//
// Codex runs hooks only as commands, through the session's shell (on Windows
// `pwsh -NoProfile -Command`, about 0.3 s to start): this program with
// CODEX_FLAG, which hands the event to her port and prints her answer to a
// prompt (main.rs). So few hooks make Codex wait: the turn starting and
// ending, a prompt, an edit (apply_patch) or a question, the session. Each
// tool call's step comes from hooks run in the background (async), which
// Codex has from 0.148 on; older ones skip async hooks with a warning, so
// those are left out for them.
//
// Codex runs a hook only once the person trusts it (/hooks in Codex), and
// again after it changes, such as this program moving: we never trust them
// on their behalf.

pub const CODEX_PROMPT_TIMEOUT: u64 = 300;
// A version: major, minor, patch.
pub type Version = (u64, u64, u64);
// The first Codex that runs async hooks.
const CODEX_ASYNC_SINCE: Version = (0, 148, 0);

// One hook we add: its event, the tools it is for (a regex, None for all),
// how long it may run (s), and whether in the background.
struct CodexHook {
    event: &'static str,
    matcher: Option<&'static str>,
    timeout: u64,
    is_async: bool,
}

// A prompt waits for the person; Codex gives Interrupt and SessionEnd 3 s at most.
const CODEX_HOOKS: [CodexHook; 10] = [
    CodexHook { event: "SessionStart", matcher: None, timeout: 10, is_async: false },
    CodexHook { event: "UserPromptSubmit", matcher: None, timeout: 10, is_async: false },
    CodexHook { event: "PreToolUse", matcher: Some("^request_user_input$"), timeout: 10, is_async: false },
    CodexHook { event: "PostToolUse", matcher: Some("^(apply_patch|request_user_input)$"), timeout: 10, is_async: false },
    CodexHook { event: "PermissionRequest", matcher: None, timeout: CODEX_PROMPT_TIMEOUT, is_async: false },
    CodexHook { event: "Stop", matcher: None, timeout: 10, is_async: false },
    CodexHook { event: "Interrupt", matcher: None, timeout: 3, is_async: false },
    CodexHook { event: "SessionEnd", matcher: None, timeout: 3, is_async: false },
    // Every step, in the background.
    CodexHook { event: "PreToolUse", matcher: None, timeout: 10, is_async: true },
    CodexHook { event: "PostToolUse", matcher: None, timeout: 10, is_async: true },
];

fn codex_hooks_for(with_async: bool) -> impl Iterator<Item = &'static CodexHook> {
    CODEX_HOOKS.iter().filter(move |h| with_async || !h.is_async)
}

pub fn codex_home() -> PathBuf {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).unwrap_or_default()).join(".codex"))
}

pub fn codex_hooks_file() -> PathBuf {
    codex_home().join("hooks.json")
}

// "codex-cli 0.146.1" (or 0.149.0-alpha.1) → (0, 146, 1).
fn parse_version(text: &str) -> Option<Version> {
    text.split_whitespace().find_map(|word| {
        let mut parts = word.split('.').map(|p| p.chars().take_while(char::is_ascii_digit).collect::<String>().parse::<u64>().ok());
        Some((parts.next()??, parts.next()??, parts.next()??))
    })
}

fn ask_codex_version() -> Option<Version> {
    let mut command = std::process::Command::new("codex");
    command.arg("--version").stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW
        command.creation_flags(0x0800_0000);
    }
    parse_version(&String::from_utf8_lossy(&command.output().ok()?.stdout))
}

// Codex's version, None when there is no `codex` to ask (the desktop app
// alone). Asked at most once a minute.
pub fn codex_version() -> Option<Version> {
    static KNOWN: Mutex<Option<(Instant, Option<Version>)>> = Mutex::new(None);
    let mut known = KNOWN.lock().unwrap();
    match *known {
        Some((at, version)) if at.elapsed() < Duration::from_secs(60) => version,
        _ => {
            let version = ask_codex_version();
            *known = Some((Instant::now(), version));
            version
        }
    }
}

// Whether Codex runs async hooks; one we cannot ask is taken as a current one.
pub fn codex_runs_async() -> bool {
    codex_version().is_none_or(|v| v >= CODEX_ASYNC_SINCE)
}

// The command line Codex's shell runs: the bare path when it needs no
// quoting (any shell takes it), else quoted for PowerShell (& '...') on
// Windows or for a POSIX shell.
pub fn codex_command(exe: &str) -> String {
    let is_plain = !exe.is_empty() && exe.chars().all(|c| c.is_ascii_alphanumeric() || "\\/:._-".contains(c));
    if is_plain {
        return format!("{exe} {CODEX_FLAG}");
    }
    if cfg!(windows) {
        format!("& '{}' {CODEX_FLAG}", exe.replace('\'', "''"))
    } else {
        format!("'{}' {CODEX_FLAG}", exe.replace('\'', r"'\''"))
    }
}

// What we add, event by event; `waiting` is what Codex shows while a prompt waits on her.
fn codex_entries(command: &str, waiting: &str, with_async: bool) -> Map<String, Value> {
    let mut out = Map::new();
    for want in codex_hooks_for(with_async) {
        let mut hook = json!({ "type": "command", "command": command, "timeout": want.timeout });
        if want.is_async {
            hook["async"] = json!(true);
        }
        if want.event == "PermissionRequest" {
            hook["statusMessage"] = json!(waiting);
        }
        let mut group = json!({ "hooks": [hook] });
        if let Some(matcher) = want.matcher {
            group["matcher"] = json!(matcher);
        }
        let groups = out.entry(want.event).or_insert_with(|| json!([]));
        if let Value::Array(groups) = groups {
            groups.push(group);
        }
    }
    out
}

pub fn codex_install(file: &Value, command: &str, waiting: &str, with_async: bool) -> Value {
    let mut hooks = strip(file.get("hooks"));
    for (event, groups) in codex_entries(command, waiting, with_async) {
        let mut all = hooks.get(&event).and_then(Value::as_array).cloned().unwrap_or_default();
        all.extend(groups.as_array().cloned().unwrap_or_default());
        hooks.insert(event, Value::Array(all));
    }
    let mut out = file.as_object().cloned().unwrap_or_default();
    out.insert("hooks".into(), Value::Object(hooks));
    Value::Object(out)
}

// Our hooks in a hooks.json: (event, matcher, hook).
fn codex_ours(file: &Value) -> Vec<(String, Option<String>, Value)> {
    let mut ours = Vec::new();
    for (event, groups) in file.get("hooks").and_then(Value::as_object).into_iter().flatten() {
        for group in groups.as_array().into_iter().flatten() {
            let matcher = group.get("matcher").and_then(Value::as_str).map(str::to_string);
            for hook in group.get("hooks").and_then(Value::as_array).into_iter().flatten().filter(|h| is_ours(h)) {
                ours.push((event.clone(), matcher.clone(), hook.clone()));
            }
        }
    }
    ours
}

fn is_codex_hook(found: &(String, Option<String>, Value), want: &CodexHook, command: &str) -> bool {
    let (event, matcher, hook) = found;
    event == want.event
        && matcher.as_deref() == want.matcher
        && hook["type"] == "command"
        && hook["command"] == command
        && hook["timeout"].as_u64() == Some(want.timeout)
        && hook.get("async").and_then(Value::as_bool).unwrap_or(false) == want.is_async
}

// How our entries stand against what this copy would install: missing / ok /
// stale (another copy of the program, or another version's hooks) /
// upgradable (only the background ones lack: Codex was updated since) /
// partial (others lack).
pub fn codex_status_of(file: &Value, command: &str, with_async: bool) -> &'static str {
    let ours = codex_ours(file);
    if ours.is_empty() {
        return "missing";
    }
    if !ours.iter().all(|found| CODEX_HOOKS.iter().any(|want| is_codex_hook(found, want, command))) {
        return "stale";
    }
    let lacking: Vec<&CodexHook> = codex_hooks_for(with_async).filter(|want| !ours.iter().any(|found| is_codex_hook(found, want, command))).collect();
    if lacking.is_empty() {
        "ok"
    } else if lacking.iter().all(|want| want.is_async) {
        "upgradable"
    } else {
        "partial"
    }
}

// absent (no Codex here), unreadable, or how our entries stand.
pub fn codex_hooks_status() -> &'static str {
    if !codex_home().is_dir() {
        return "absent";
    }
    match read_json(&codex_hooks_file()) {
        Ok(file) => codex_status_of(&file, &codex_command(&launch_command()), codex_runs_async()),
        Err(_) => "unreadable",
    }
}

pub fn is_codex_connected() -> bool {
    matches!(codex_hooks_status(), "ok" | "stale" | "upgradable" | "partial")
}

// Whether Claude Code or Codex is connected, as last looked (at start, as the
// settings read it, after connecting) or heard (any event from them): what
// the island asks for while it is not (island.js), without reading their
// settings files on every redraw. Connected until looked, so a first draw
// never asks wrongly.
static CONNECTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn note_connected(connected: bool) -> bool {
    CONNECTED.swap(connected, std::sync::atomic::Ordering::SeqCst) != connected
}

pub fn look_connected(port: u16) -> bool {
    let connected = connection(port) != "none" || is_codex_connected() || is_agy_connected() || is_grok_connected(port);
    note_connected(connected);
    connected
}

pub fn seems_connected() -> bool {
    CONNECTED.load(std::sync::atomic::Ordering::SeqCst)
}

// Change Codex's hooks.json, backed up once beside itself. action: install, remove.
pub fn write_codex_hooks(action: &str, waiting: &str) -> Result<(), String> {
    let file = codex_hooks_file();
    // Read from the disk, not as last read: nothing of someone else's is written back.
    let before = read_json_now(&file)?;
    let after = match action {
        "remove" if !file.exists() => return Ok(()),
        "remove" => uninstall(&before),
        "install" => codex_install(&before, &codex_command(&launch_command()), waiting, codex_runs_async()),
        _ => return Err(format!("unknown action {action}")),
    };
    write_json(&file, &after)
}

// --- Antigravity: hooks in ~/.gemini/config/hooks.json --------------------------------
//
// Antigravity (Google's agy, and its 2.0 app, which reads the same file) runs
// hooks only as commands, each tool's in a block of its own by name: ours is
// "wakuwaku". Its events: SessionStart; PreInvocation, each call of the model
// (the first of a turn, invocationNum 0); PostToolUse; Stop. None for a prompt
// waiting on the person: she follows its work and its end, and asks nothing
// (events_agy.rs). Its standalone IDE runs none, as reported.
//
// Tried with agy 1.3.4 (2026-10-13): it runs a hook as `cmd /c "<command>"`,
// the command's own double quotes escaped as Go escapes them (\"), which cmd
// does not read, so a path with a space can not be quoted: its 8.3 name is
// used for one, and with none it is not installed. And a hook that fails is
// told back in cmd's code page, not UTF-8: agy's turn failed on it (invalid
// UTF-8). Ours answers at once whatever comes (main.rs agy_hook).

const AGY_BLOCK: &str = "wakuwaku";
const AGY_TIMEOUT: u64 = 10;

pub fn gemini_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).unwrap_or_default()).join(".gemini")
}

pub fn agy_hooks_file() -> PathBuf {
    gemini_dir().join("config").join("hooks.json")
}

// Whether Antigravity is on this computer: agy's folder, or the 2.0 app's
// (the standalone IDE's alone does not count: it runs no hooks).
pub fn agy_here() -> bool {
    let gemini = gemini_dir();
    gemini.join("antigravity-cli").is_dir() || gemini.join("antigravity").is_dir()
}

// A path cmd reads as one word with no quotes around it.
fn is_bare(path: &str) -> bool {
    !path.is_empty() && !path.chars().any(|c| c.is_whitespace() || "&|<>^()%!\"',;=`".contains(c))
}

#[cfg(windows)]
fn short_path(path: &str) -> Option<String> {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetShortPathNameW(long: *const u16, short: *mut u16, size: u32) -> u32;
    }
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut out = vec![0u16; 1024];
    // SAFETY: both buffers ours, the second as long as said.
    let n = unsafe { GetShortPathNameW(wide.as_ptr(), out.as_mut_ptr(), out.len() as u32) } as usize;
    (n > 0 && n < out.len()).then(|| String::from_utf16_lossy(&out[..n]))
}

#[cfg(not(windows))]
fn short_path(_path: &str) -> Option<String> {
    None
}

// This program as agy's cmd reads it: as it is, or by its 8.3 name; None
// when neither will do (a space in it, and short names turned off there).
pub fn agy_program(exe: &str) -> Option<String> {
    if is_bare(exe) {
        return Some(exe.to_string());
    }
    short_path(exe).filter(|s| is_bare(s))
}

// Our block: the event's name after the flag (the payload has none).
fn agy_block(program: &str) -> Value {
    let hook = |event: &str| json!({ "type": "command", "command": format!("{program} {AGY_FLAG} {event}"), "timeout": AGY_TIMEOUT });
    json!({
        "SessionStart": [hook("SessionStart")],
        "PreInvocation": [hook("PreInvocation")],
        "PostToolUse": [{ "matcher": "", "hooks": [hook("PostToolUse")] }],
        "Stop": [hook("Stop")],
    })
}

pub fn agy_install(file: &Value, program: &str) -> Value {
    let mut out = file.as_object().cloned().unwrap_or_default();
    out.insert(AGY_BLOCK.into(), agy_block(program));
    Value::Object(out)
}

pub fn agy_uninstall(file: &Value) -> Value {
    let mut out = file.as_object().cloned().unwrap_or_default();
    out.remove(AGY_BLOCK);
    Value::Object(out)
}

// How our block stands against what this copy would write: missing / ok /
// stale (another copy of the program, or another version's) / off (turned
// off in the file: enabled false).
pub fn agy_status_of(file: &Value, program: Option<&str>) -> &'static str {
    let Some(ours) = file.get(AGY_BLOCK) else { return "missing" };
    if ours.get("enabled").and_then(Value::as_bool) == Some(false) {
        return "off";
    }
    let mut ours = ours.clone();
    if let Some(m) = ours.as_object_mut() {
        m.remove("enabled");
    }
    match program {
        Some(p) if ours == agy_block(p) => "ok",
        _ => "stale",
    }
}

// absent (no Antigravity here), unreadable, or how our block stands.
pub fn agy_hooks_status() -> &'static str {
    if !agy_here() {
        return "absent";
    }
    match read_json(&agy_hooks_file()) {
        Ok(file) => agy_status_of(&file, agy_program(&launch_command()).as_deref()),
        Err(_) => "unreadable",
    }
}

// The tools on this computer by name, in their order (the island names
// them while none is connected): Claude Code by its folder, Codex,
// Antigravity and Grok by theirs; none found, all (it can not be told).
pub fn tools_here() -> Vec<&'static str> {
    let here: Vec<&str> = [("Claude Code", claude_here()), ("Codex", codex_home().is_dir()), ("Antigravity", agy_here()), ("Grok", grok_here())]
        .into_iter()
        .filter(|(_, is)| *is)
        .map(|(name, _)| name)
        .collect();
    if here.is_empty() {
        vec!["Claude Code", "Codex", "Antigravity", "Grok"]
    } else {
        here
    }
}

pub fn is_agy_connected() -> bool {
    matches!(agy_hooks_status(), "ok" | "stale")
}

// What keeps this program from agy's hooks, as write_agy_hooks says it
// (error_kind: "path").
const AGY_NO_NAME: &str = "no name for this program without quotes";

// Change Antigravity's hooks.json, backed up once beside itself. action: install, remove.
pub fn write_agy_hooks(action: &str) -> Result<(), String> {
    let file = agy_hooks_file();
    // Read from the disk, not as last read: nothing of someone else's is written back.
    let before = read_json_now(&file)?;
    let after = match action {
        "remove" if !file.exists() => return Ok(()),
        "remove" => agy_uninstall(&before),
        "install" => {
            let exe = launch_command();
            let Some(program) = agy_program(&exe) else { return Err(format!("{exe}: {AGY_NO_NAME} (a space in its path, and no short name for it)")) };
            agy_install(&before, &program)
        }
        _ => return Err(format!("unknown action {action}")),
    };
    write_json(&file, &after)
}

// --- Grok Build: hooks in ~/.grok/hooks/wakuwaku.json -----------------------------------
//
// Grok Build loads every *.json in $GROK_HOME/hooks (~/.grok/hooks) as hooks
// of its own, always trusted, with no /hooks add (xai-org/grok-build,
// global_hook_sources.rs): the file is all ours. Its hooks are HTTP ones, as
// Claude Code's, to /hook?…&agent=grok (events_grok.rs). It loads Claude
// Code's too (the plugin, ~/.claude/settings.json), which bring the same
// events without an agent: server.rs drops those while ours are in.

pub fn grok_home() -> PathBuf {
    match std::env::var_os("GROK_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => PathBuf::from(std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).unwrap_or_default()).join(".grok"),
    }
}

pub fn grok_hooks_file() -> PathBuf {
    grok_home().join("hooks").join("wakuwaku.json")
}

pub fn grok_hook_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/hook?{URL_MARK}&agent=grok")
}

// The events Grok's hooks bring, and a matcher for one: the run of a turn,
// its three ends (Stop, StopFailure, StopCancelled), and of its
// notifications Grok's own prompt waiting on the person and its settling.
// No PermissionRequest: Grok has none (events_grok.rs).
const GROK_EVENTS: [(&str, Option<&str>); 10] = [
    ("SessionStart", None),
    ("SessionEnd", None),
    ("UserPromptSubmit", None),
    ("PreToolUse", None),
    ("PostToolUse", None),
    ("PostToolUseFailure", None),
    ("Notification", Some("permission_prompt|idle_prompt")),
    ("Stop", None),
    ("StopFailure", None),
    ("StopCancelled", None),
];
// /hook answers at once ({}: no decision, Grok's own prompt asks), and a
// refused connection (she is not up) fails open at once.
const GROK_TIMEOUT: u64 = 2;

// Whether Grok Build is on this computer: its folder.
pub fn grok_here() -> bool {
    grok_home().is_dir()
}

// The whole file, as this copy writes it for its port.
pub fn grok_file(port: u16) -> Value {
    let mut hooks = Map::new();
    for (event, matcher) in GROK_EVENTS {
        let mut group = json!({ "hooks": [{ "type": "http", "url": grok_hook_url(port), "timeout": GROK_TIMEOUT }] });
        if let Some(m) = matcher {
            group["matcher"] = json!(m);
        }
        hooks.insert(event.into(), json!([group]));
    }
    json!({ "hooks": hooks })
}

// Whether any hook in a file is one of ours, to whatever port.
fn grok_has_ours(file: &Value) -> bool {
    let groups = file.get("hooks").and_then(Value::as_object).into_iter().flat_map(|m| m.values()).filter_map(Value::as_array).flatten();
    groups
        .filter_map(|g| g.get("hooks").and_then(Value::as_array))
        .flatten()
        .any(|h| h.get("url").and_then(Value::as_str).is_some_and(|u| u.contains(URL_MARK) && u.contains("agent=grok")))
}

// Our file against what this copy would write: ok, stale (another port,
// another version's events), or missing.
pub fn grok_status_of(file: &Value, port: u16) -> &'static str {
    if *file == grok_file(port) {
        "ok"
    } else if grok_has_ours(file) {
        "stale"
    } else {
        "missing"
    }
}

// absent (no Grok here), unreadable, or how our file stands.
pub fn grok_hooks_status(port: u16) -> &'static str {
    if !grok_here() {
        return "absent";
    }
    match read_json(&grok_hooks_file()) {
        Ok(file) => grok_status_of(&file, port),
        Err(_) => "unreadable",
    }
}

pub fn is_grok_connected(port: u16) -> bool {
    matches!(grok_hooks_status(port), "ok" | "stale")
}

// Whether Grok's own hooks bring its events to this port: our file there,
// naming our URL. Asked for each Grok event the Claude Code hooks bring
// (server.rs), so looked at once in a few seconds at most, and anew once
// the file is written.
static GROK_LOOKED: Mutex<Option<(Instant, bool)>> = Mutex::new(None);

pub fn grok_hooked(port: u16) -> bool {
    let mut looked = GROK_LOOKED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, hooked)) = *looked {
        if at.elapsed() < std::time::Duration::from_secs(5) {
            return hooked;
        }
    }
    let hooked = std::fs::read_to_string(grok_hooks_file()).is_ok_and(|text| text.contains(&grok_hook_url(port)));
    *looked = Some((Instant::now(), hooked));
    hooked
}

// Write our file in Grok's hooks folder, or take it away. It is all ours:
// nothing to back up or keep. action: install, remove. An open Grok
// session takes it up once restarted (or /hooks, r).
pub fn write_grok_hooks(action: &str, port: u16) -> Result<(), String> {
    let file = grok_hooks_file();
    let done = match action {
        "remove" if !file.exists() => Ok(()),
        "remove" => fs::remove_file(&file).map_err(|e| e.to_string()),
        "install" => {
            let text = serde_json::to_string_pretty(&grok_file(port)).map_err(|e| e.to_string())?;
            file.parent().map_or(Ok(()), fs::create_dir_all).map_err(|e| e.to_string())?;
            fs::write(&file, format!("{text}\n")).map_err(|e| e.to_string())
        }
        _ => return Err(format!("unknown action {action}")),
    };
    // As last read no longer.
    if let Some(read) = READ.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        read.remove(&file);
    }
    *GROK_LOOKED.lock().unwrap_or_else(|e| e.into_inner()) = None;
    done
}

// --- Start at login: this program, from where it is now -------------------------------

#[cfg(windows)]
mod login {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const NAME: &str = "Wakuwaku";

    fn command() -> String {
        format!("\"{}\"", super::launch_command())
    }

    pub fn is_on() -> bool {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(RUN, KEY_READ)
            .and_then(|k| k.get_value::<String, _>(NAME))
            .is_ok_and(|v| v.eq_ignore_ascii_case(&command()))
    }

    pub fn set(on: bool) -> Result<(), String> {
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey_with_flags(RUN, KEY_READ | KEY_WRITE).map_err(|e| e.to_string())?;
        if on {
            key.set_value(NAME, &command()).map_err(|e| e.to_string())
        } else {
            match key.delete_value(NAME) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
                _ => Ok(()),
            }
        }
    }
}

#[cfg(not(windows))]
mod login {
    pub fn is_on() -> bool {
        false
    }

    pub fn set(_on: bool) -> Result<(), String> {
        Err("not on this system".into())
    }
}

pub use login::{is_on as is_open_at_login, set as set_open_at_login};

#[cfg(test)]
mod tests {
    #[test]
    fn grok_hooks_bring_each_event_here_and_say_how_they_stand() {
        use serde_json::json;
        let file = super::grok_file(47213);
        let hooks = file["hooks"].as_object().unwrap();
        let events: Vec<&str> = hooks.keys().map(String::as_str).collect();
        for event in ["SessionStart", "SessionEnd", "UserPromptSubmit", "PreToolUse", "PostToolUse", "PostToolUseFailure", "Notification", "Stop", "StopFailure", "StopCancelled"] {
            assert!(events.contains(&event), "{event}");
        }
        assert!(!events.contains(&"PermissionRequest"));
        assert_eq!(hooks["Stop"], json!([{ "hooks": [{ "type": "http", "url": "http://127.0.0.1:47213/hook?from=wakuwaku&agent=grok", "timeout": 2 }] }]));
        assert_eq!(hooks["Notification"][0]["matcher"], "permission_prompt|idle_prompt");
        assert!(hooks["PreToolUse"][0].get("matcher").is_none());

        assert_eq!(super::grok_status_of(&file, 47213), "ok");
        // Another port's, or one an older copy wrote with fewer events: ours, not as this one writes it.
        assert_eq!(super::grok_status_of(&super::grok_file(47299), 47213), "stale");
        let mut fewer = file.clone();
        fewer["hooks"].as_object_mut().unwrap().remove("StopCancelled");
        assert_eq!(super::grok_status_of(&fewer, 47213), "stale");
        // Someone else's, or nothing.
        assert_eq!(super::grok_status_of(&json!({ "hooks": { "Stop": [{ "hooks": [{ "type": "http", "url": "http://127.0.0.1:47299/hook?from=dump" }] }] } }), 47213), "missing");
        assert_eq!(super::grok_status_of(&json!({}), 47213), "missing");
    }

    #[test]
    fn agy_hooks_install_status_and_uninstall() {
        let program = r"D:\pet\wakuwaku.exe";
        let theirs = json!({ "my-linter-hook": { "PostToolUse": [{ "matcher": "run_command", "hooks": [{ "command": "./lint.sh" }] }] } });
        assert_eq!(agy_status_of(&theirs, Some(program)), "missing");
        let with = agy_install(&theirs, program);
        assert_eq!(agy_status_of(&with, Some(program)), "ok");
        assert_eq!(with["wakuwaku"]["Stop"][0]["command"], r"D:\pet\wakuwaku.exe --wakuwaku-agy-hook Stop");
        assert_eq!(with["wakuwaku"]["PostToolUse"][0]["matcher"], "");
        // Theirs kept, ours alone taken out.
        assert_eq!(with["my-linter-hook"], theirs["my-linter-hook"]);
        assert_eq!(agy_uninstall(&with), theirs);
        assert_eq!(agy_status_of(&with, Some(r"E:\elsewhere\wakuwaku.exe")), "stale");
        assert_eq!(agy_status_of(&with, None), "stale");
        let mut off = with.clone();
        off["wakuwaku"]["enabled"] = json!(false);
        assert_eq!(agy_status_of(&off, Some(program)), "off");
        off["wakuwaku"]["enabled"] = json!(true);
        assert_eq!(agy_status_of(&off, Some(program)), "ok");
    }

    #[test]
    fn a_path_cmd_reads_without_quotes() {
        assert!(is_bare(r"D:\pet\wakuwaku.exe"));
        assert!(is_bare(r"C:\Users\张三\wakuwaku.exe"));
        assert!(!is_bare(r"C:\Program Files\Wakuwaku\wakuwaku.exe"));
        assert!(!is_bare(r"C:\Program Files (x86)\w.exe"));
        assert!(!is_bare(r"C:\a&b\w.exe"));
        assert_eq!(agy_program(r"D:\pet\wakuwaku.exe").as_deref(), Some(r"D:\pet\wakuwaku.exe"));
        assert_eq!(super::error_kind(&format!("x: {AGY_NO_NAME} (a space)")), "path");
    }

    #[test]
    fn a_write_that_failed_says_why() {
        assert_eq!(super::error_kind("另一个程序正在使用此文件，进程无法访问。 (os error 32)"), "locked");
        assert_eq!(super::error_kind("expected `,` or `}` at line 3 column 5"), "invalid");
        assert_eq!(super::error_kind("拒绝访问。 (os error 5)"), "other");
    }

    use super::*;

    #[test]
    fn install_status_and_uninstall() {
        let mine = json!({ "hooks": { "Stop": [{ "hooks": [{ "type": "command", "command": "echo hi" }] }] }, "model": "x" });
        assert_eq!(status_of(&mine, 47213), "missing");

        let with = install(&mine, 47213, false);
        assert_eq!(status_of(&with, 47213), "ok");
        assert_eq!(status_of(&with, 47299), "stale");
        assert_eq!(with["hooks"]["Stop"].as_array().unwrap().len(), 2);
        assert_eq!(with["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"], 300);
        assert_eq!(with["hooks"]["SessionStart"][0]["hooks"][0]["args"], json!([ENSURE_FLAG]));
        assert_eq!(status_of(&install(&mine, 47213, true), 47213), "httpOnly");

        // Installing twice replaces, never doubles.
        let again = install(&with, 47213, false);
        assert_eq!(again["hooks"]["Stop"].as_array().unwrap().len(), 2);

        let without = uninstall(&with);
        assert_eq!(without, mine);
        assert_eq!(uninstall(&install(&json!({}), 47213, false)), json!({}));
    }

    #[test]
    fn some_events_missing_is_partial() {
        let mut with = install(&json!({}), 47213, false);
        with["hooks"].as_object_mut().unwrap().remove("Elicitation");
        assert_eq!(status_of(&with, 47213), "partial");
    }

    #[test]
    fn codex_hooks_install_status_and_uninstall() {
        let cmd = codex_command(r"D:\pet\wakuwaku.exe");
        assert_eq!(cmd, r"D:\pet\wakuwaku.exe --wakuwaku-codex-hook");
        let mine = json!({ "hooks": { "Stop": [{ "hooks": [{ "type": "command", "command": "notify-me" }] }] }, "other": 1 });
        assert_eq!(codex_status_of(&mine, &cmd, true), "missing");

        let with = codex_install(&mine, &cmd, "waiting", true);
        assert_eq!(codex_status_of(&with, &cmd, true), "ok");
        assert_eq!(with["hooks"]["Stop"].as_array().unwrap().len(), 2);
        let ask = &with["hooks"]["PermissionRequest"][0]["hooks"][0];
        assert_eq!((ask["timeout"].as_u64(), ask.get("async"), ask["statusMessage"].as_str()), (Some(300), None, Some("waiting")));
        // A question waits in the foreground; every step in the background.
        let pre = with["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!((pre[0]["matcher"].as_str(), pre[0]["hooks"][0].get("async")), (Some("^request_user_input$"), None));
        assert_eq!((pre[1].get("matcher"), &pre[1]["hooks"][0]["async"]), (None, &json!(true)));
        assert_eq!(with["hooks"]["PostToolUse"][0]["matcher"], "^(apply_patch|request_user_input)$");
        // Another copy of the program: a repair.
        assert_eq!(codex_status_of(&with, &codex_command(r"E:\elsewhere\wakuwaku.exe"), true), "stale");

        let again = codex_install(&with, &cmd, "waiting", true);
        assert_eq!(again["hooks"]["Stop"].as_array().unwrap().len(), 2);
        assert_eq!(again["hooks"]["PreToolUse"].as_array().unwrap().len(), 2);
        let mut part = again.clone();
        part["hooks"].as_object_mut().unwrap().remove("Interrupt");
        assert_eq!(codex_status_of(&part, &cmd, true), "partial");

        assert_eq!(uninstall(&with), mine);
        assert_eq!(uninstall(&codex_install(&json!({}), &cmd, "w", true)), json!({}));
    }

    #[test]
    fn an_older_codex_gets_no_background_hooks() {
        let cmd = codex_command(r"D:\pet\wakuwaku.exe");
        let old = codex_install(&json!({}), &cmd, "w", false);
        let all: Vec<&Value> = old["hooks"].as_object().unwrap().values().flat_map(|g| g.as_array().unwrap()).collect();
        assert_eq!(all.len(), 8);
        assert!(all.iter().all(|g| g["hooks"][0].get("async").is_none()));
        assert_eq!(codex_status_of(&old, &cmd, false), "ok");
        // Codex updated since: only the background ones lack.
        assert_eq!(codex_status_of(&old, &cmd, true), "upgradable");
        let mut part = old.clone();
        part["hooks"].as_object_mut().unwrap().remove("Stop");
        assert_eq!(codex_status_of(&part, &cmd, true), "partial");
        // Installed for a newer Codex, read for an older one: still ours, nothing stale.
        assert_eq!(codex_status_of(&codex_install(&json!({}), &cmd, "w", true), &cmd, false), "ok");
    }

    #[test]
    fn codex_versions() {
        assert_eq!(parse_version("codex-cli 0.146.1\n"), Some((0, 146, 1)));
        assert_eq!(parse_version("codex-cli 0.149.0-alpha.1"), Some((0, 149, 0)));
        assert_eq!(parse_version("nothing here"), None);
        assert!((0, 146, 1) < CODEX_ASYNC_SINCE && (0, 148, 0) >= CODEX_ASYNC_SINCE && (1, 0, 0) >= CODEX_ASYNC_SINCE);
    }

    #[test]
    fn codex_commands_quote_what_needs_it() {
        if cfg!(windows) {
            assert_eq!(codex_command(r"C:\Program Files\Waku\wakuwaku.exe"), r"& 'C:\Program Files\Waku\wakuwaku.exe' --wakuwaku-codex-hook");
            assert_eq!(codex_command(r"D:\宠物\it's.exe"), r"& 'D:\宠物\it''s.exe' --wakuwaku-codex-hook");
        } else {
            assert_eq!(codex_command("/opt/my pets/wakuwaku"), "'/opt/my pets/wakuwaku' --wakuwaku-codex-hook");
        }
    }
}
