// The settings, grown out of the island (src/pet/settings.js): what they
// show (a snapshot, pushed again as things change) and what they change.
use std::sync::atomic::Ordering;
use std::sync::Arc;

use serde_json::{json, Map, Value};
use tauri::{AppHandle, WebviewWindow};

use crate::{connection, data, fetch, fullscreen, i18n, island, now_ms, pet, scripts, set_display, set_out, shared, Shared};

const REPO: &str = "https://github.com/zhoupengjie/wakuwaku";

// Everything the settings show.
pub fn snapshot(sh: &Shared) -> Value {
    let pets: Vec<Value> = data::pets(&sh.dir)
        .iter()
        .map(|p| json!({ "id": p.id, "name": p.name, "author": p.author, "url": data::asset_url(&p.sheet), "version": p.version, "codex": p.from_codex }))
        .collect();
    let (now, sessions) = {
        let pet = sh.pet.lock().unwrap();
        (pet.get(now_ms()), pet.list())
    };
    let (win_light, win_accent) = crate::wallpaper::look();
    // Whether either is connected, as the island tells it (connection.rs).
    let conn = connection::connection(sh.port);
    if connection::note_connected(conn != "none" || connection::is_codex_connected() || connection::is_agy_connected()) {
        sh.redraw();
    }
    json!({
        "settings": Value::Object(sh.settings.lock().unwrap().clone()),
        "lang": sh.lang(),
        "pets": pets,
        "now": now,
        "sessions": sessions,
        "asks": sh.asks.lock().unwrap().views(),
        "isVisible": sh.is_visible(),
        "hidden": sh.hidden.load(Ordering::SeqCst),
        "connection": conn,
        "pluginCommands": connection::PLUGIN_COMMANDS,
        "hooks": connection::hooks_status(sh.port),
        // Whether any Claude Code event came since she started: installed,
        // and a session seen (connected), or still waiting for one.
        "claudeSeen": sh.claude_seen.load(Ordering::SeqCst) > 0,
        "settingsFile": connection::claude_settings_file().to_string_lossy(),
        // The same as read (~ for the home folder), whether a copy of it was
        // kept (none: there was no file before), and whether Claude Code is
        // on this computer at all (its folder): the welcome offers the ones
        // that are.
        "settingsShown": connection::shown_path(&connection::claude_settings_file()),
        "settingsBackup": connection::backup_of(&connection::claude_settings_file()).exists(),
        "claudeHere": connection::claude_here(),
        // Codex's hooks.json, whether any Codex event came (they run once
        // trusted), and its version: one before 0.148 gets no background hooks.
        "codex": {
            "hooks": connection::codex_hooks_status(),
            "file": connection::codex_hooks_file().to_string_lossy(),
            "fileShown": connection::shown_path(&connection::codex_hooks_file()),
            "seen": sh.codex_seen.load(Ordering::SeqCst) > 0,
            "version": connection::codex_version().map(|(a, b, c)| format!("{a}.{b}.{c}")),
            "async": connection::codex_runs_async(),
        },
        // Antigravity's hooks.json (connection.rs), the same, and whether it
        // is on this computer at all (agy's folder, or its 2.0 app's).
        "agy": {
            "hooks": connection::agy_hooks_status(),
            "file": connection::agy_hooks_file().to_string_lossy(),
            "fileShown": connection::shown_path(&connection::agy_hooks_file()),
            "backup": connection::backup_of(&connection::agy_hooks_file()).exists(),
            "seen": sh.agy_seen.load(Ordering::SeqCst) > 0,
        },
        // How many displays there are: her taskbar on all of them is offered
        // only with more than one (Look, taskbarDisplays).
        "displayCount": sh.screens().areas.len(),
        "loginAtStart": connection::is_open_at_login(),
        // Windows' own mode and accent colour.
        "winLook": { "light": win_light, "accent": win_accent.map(|a| a.base) },
        "version": sh.app.package_info().version.to_string(),
        // Her updates: this version, the latest, an update under way (updates.rs).
        "update": crate::updates::view(&sh),
        // Her crash reports: as set, those waiting, the last sent (crash.rs).
        "crash": crate::crash::view(sh),
        "fullscreenAvailable": fullscreen::AVAILABLE,
        // Whether Windows' acrylic can go under the open island (backplate.rs).
        "plate": crate::backplate::available(),
        "widgets": sh.widgets_view(),
        // The plugins she runs, and where waku is for the terminal.
        "plugins": scripts::view(sh),
        "waku": scripts::waku_path(sh).to_string_lossy(),
        "port": sh.port,
    })
}

// One mode's island look: its ground (#rrggbb), how solid (20 to 100 %),
// how blurred what is behind (0 to 60 px), its edge, no capsule collapsed;
// each part may be left out (its default), nothing else.
fn is_island_look(v: &Value) -> bool {
    v.is_null()
        || v.as_object().is_some_and(|m| {
            m.iter().all(|(k, v)| match k.as_str() {
                "bg" => v.as_str().is_some_and(|s| s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())),
                "op" => v.as_u64().is_some_and(|n| (20..=100).contains(&n)),
                "blur" => v.as_u64().is_some_and(|n| n <= 60),
                "edge" => matches!(v.as_str(), Some("line" | "ring" | "none")),
                "bare" => v.is_boolean(),
                _ => false,
            })
        })
}

// The taskbar's layout: { left, center, right }, each a list of its parts
// (island.js TB_PARTS), none twice and nothing else; one left out goes where
// the default has it.
const TASKBAR_PARTS: [&str; 8] = ["her", "start", "apps", "monitor", "tray", "keys", "quick", "clock"];

// A widget put on the taskbar (widgets.js isEntry): { id, type, of, size,
// name, icon }, its id "w" and up to 15 of a-z 0-9; a widget's or a
// plugin's (of: its id), or a spacer (flex, gap, sep).
fn is_taskbar_widget(v: &Value) -> Option<&str> {
    let w = v.as_object()?;
    let id = w.get("id")?.as_str()?;
    let ok_id = id.len() >= 2 && id.len() <= 16 && id.starts_with('w') && id[1..].chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    let kind = w.get("type")?.as_str()?;
    let of = w.get("of").map(|of| of.as_str().filter(|of| crate::widgets::is_id(of)));
    let ok_of = match kind {
        "widget" => matches!(of, Some(Some(_))),
        "plugin" => matches!(of, Some(Some(of)) if crate::scripts::SCRIPTS.iter().any(|s| s.id == of)),
        "flex" | "gap" | "sep" => of.is_none(),
        _ => false,
    };
    let ok_rest = w.iter().all(|(key, v)| match key.as_str() {
        "id" | "type" | "of" => true,
        "size" => matches!(v.as_str(), Some("S" | "M")),
        "name" => v.as_str().is_some_and(|n| n.chars().count() <= 40 && !n.chars().any(char::is_control)),
        "icon" => v.as_str().is_some_and(|i| crate::widgets::ICONS.contains(&i)),
        _ => false,
    });
    (ok_id && ok_of && ok_rest).then_some(id)
}

// Whether a taskbar layout shows this widget: put there by itself, or its
// plugin's.
pub fn is_on_taskbar(layout: &Value, widget: &str) -> bool {
    let owner = crate::scripts::owner_of(widget);
    layout.as_object().into_iter().flat_map(|runs| runs.values()).filter_map(Value::as_array).flatten().any(|e| match (e["type"].as_str(), e["of"].as_str()) {
        (Some("widget"), Some(of)) => of == widget,
        (Some("plugin"), Some(of)) => Some(of) == owner,
        _ => false,
    })
}

// Its runs: the parts each once, and up to 32 widgets, each once.
fn is_taskbar_layout(v: &Value) -> bool {
    let Some(runs) = v.as_object() else { return false };
    let mut seen: Vec<&str> = Vec::new();
    let mut widgets = 0;
    runs.iter().all(|(run, parts)| {
        matches!(run.as_str(), "left" | "center" | "right")
            && parts.as_array().is_some_and(|parts| {
                parts.iter().all(|p| match (p.as_str(), is_taskbar_widget(p)) {
                    (Some(p), _) if TASKBAR_PARTS.contains(&p) && !seen.contains(&p) => {
                        seen.push(p);
                        true
                    }
                    (None, Some(id)) if !seen.contains(&id) && widgets < 32 => {
                        seen.push(id);
                        widgets += 1;
                        true
                    }
                    _ => false,
                })
            })
    })
}

// What a settings patch may hold, checked: anything else is dropped.
fn is_ok(sh: &Shared, key: &str, v: &Value) -> bool {
    let is_bool = v.is_boolean();
    match key {
        "lang" => matches!(v.as_str(), Some("auto" | "zh" | "en")),
        "pet" => v.as_str().is_some_and(|id| data::pets(&sh.dir).iter().any(|p| p.id == id)),
        "scale" => v.as_f64().is_some_and(|s| data::SCALES.iter().any(|(_, x)| (x - s).abs() < 1e-9)),
        "details" | "walk" | "look" | "sound" | "dnd" | "hideInFullscreen" | "onboarded" | "out" | "settingsPin" | "settingsTop" | "oledShift" | "autoUpdateCheck" => is_bool,
        // Where an update is downloaded from first (updates.rs gitee_first).
        "updateFrom" => matches!(v.as_str(), Some("auto" | "github" | "gitee")),
        // Crash reports sent by themselves, never, or asked about (crash.rs).
        "crashReports" => matches!(v.as_str(), Some("auto" | "off" | "ask")),
        // The taskbar home stands in for Windows' own: elsewhere, only the island.
        "display" => v.as_str() == Some("island"),
        "islandWidth" => matches!(v.as_str(), Some("narrow" | "normal" | "wide")),
        "taskbarMaterial" => matches!(v.as_str(), Some("mica" | "black" | "clear")),
        "taskbarClearWhen" => matches!(v.as_str(), Some("mica" | "solid" | "always")),
        "taskbarButtons" => matches!(v.as_str(), Some("icons" | "labels")),
        "taskbarAlign" => matches!(v.as_str(), Some("center" | "left")),
        "taskbarEdge" => matches!(v.as_str(), Some("bottom" | "top" | "left" | "right")),
        "taskbarDisplays" => matches!(v.as_str(), Some("main" | "all")),
        // The taskbar's parts in three runs (island.js tbLayout), each once.
        "taskbarLayout" => v.is_null() || is_taskbar_layout(v),
        // The programs' buttons in the order dragged: their keys (paths).
        "taskbarOrder" => v.as_array().is_some_and(|keys| keys.len() <= 200 && keys.iter().all(|k| k.as_str().is_some_and(|s| s.len() <= 1024))),
        "hold" => v.as_str() == Some("seen") || matches!(v.as_u64(), Some(8 | 30 | 120)),
        // Minutes a session at rest stays in sight, or 0: always (state.rs is_shown).
        "idleHideMin" => matches!(v.as_u64(), Some(0 | 15 | 30 | 60 | 120)),
        "promptWaitSec" => matches!(v.as_u64(), Some(30 | 60 | 120 | 290)),
        "notify" => ["waiting", "done", "error"].iter().all(|k| v.get(k).is_some_and(Value::is_boolean)),
        "widgetsOff" | "widgetOrder" => v.as_array().is_some_and(|ids| ids.len() <= 64 && ids.iter().all(|id| id.as_str().is_some_and(crate::widgets::is_id))),
        "widgetSpin" => matches!(v.as_u64(), Some(0 | 5 | 8 | 15)),
        "plugins" => scripts::is_ok(v),
        "monitor" => v.as_object().is_some_and(|m| {
            m.iter().all(|(k, v)| match k.as_str() {
                "cpu" | "mem" | "net" | "battery" | "pin" => v.is_boolean(),
                "every" => v.as_u64().is_some_and(|s| (1..=10).contains(&s)),
                _ => false,
            })
        }),
        "widgetNudge" => is_bool,
        // Not to be reminded that nothing is connected (island.js isHinting).
        "connectHintOff" => is_bool,
        // Her island's look on the taskbar, by Windows' mode (island.js
        // drawIslandLook): none, or a mode's none, is Windows' own grey.
        // And at the top of the screen (island.js lookNow), kept apart:
        // none is black, as it always was.
        "islandLook" | "islandTopLook" => v.is_null() || v.as_object().is_some_and(|m| m.iter().all(|(mode, look)| matches!(mode.as_str(), "dark" | "light") && is_island_look(look))),
        _ => false,
    }
}

// A patch from the settings: checked, with the size, the display and her
// being out through the windows, so she keeps her spot.
pub fn apply_patch(sh: &Arc<Shared>, patch: &Value) -> Value {
    let mut rest = Map::new();
    let (mut scale, mut display, mut out) = (None, None, None);
    for (key, value) in patch.as_object().into_iter().flatten() {
        if !is_ok(sh, key, value) {
            continue;
        }
        match key.as_str() {
            "scale" => scale = value.as_f64(),
            "display" => display = value.as_str().map(str::to_string),
            "out" => out = value.as_bool(),
            _ => {
                rest.insert(key.clone(), value.clone());
            }
        }
    }
    if let Some(scale) = scale {
        pet::resize(sh, scale);
    }
    if let Some(display) = display {
        set_display(sh, &display);
    }
    if let Some(out) = out {
        set_out(sh, out);
    }
    if rest.contains_key("dnd") && rest["dnd"] == true && sh.asks.lock().unwrap().dismiss_all() > 0 {
        sh.push_asks();
    }
    let plugins = rest.contains_key("plugins");
    let crash_auto = rest.get("crashReports").and_then(Value::as_str) == Some("auto");
    if !rest.is_empty() {
        sh.change(Value::Object(rest));
    }
    // Crash reports turned on: those waiting sent now.
    if crash_auto {
        crate::crash::send_now(sh);
    }
    if plugins && scripts::sync(sh) {
        sh.redraw();
    }
    snapshot(sh)
}

#[tauri::command]
pub async fn settings_get(app: AppHandle) -> Value {
    snapshot(&shared(&app))
}

#[tauri::command]
pub async fn settings_set(app: AppHandle, patch: Value) -> Value {
    apply_patch(&shared(&app), &patch)
}

// Open them from anywhere (the tray, her double-click): on a page, or where they were.
#[tauri::command]
pub async fn settings_open(app: AppHandle, tab: Option<String>) {
    island::open_settings(&shared(&app), tab.as_deref());
}

// The page closed them: the keyboard goes back, and a risen island goes.
#[tauri::command]
pub async fn settings_closed(app: AppHandle, window: WebviewWindow) {
    let sh = shared(&app);
    island::settings_closed(&sh);
    sh.set_keyboard(&window, false);
}

#[tauri::command]
pub async fn settings_show_pet(app: AppHandle, on: bool) -> Value {
    let sh = shared(&app);
    sh.set_hidden(!on);
    snapshot(&sh)
}

#[tauri::command]
pub async fn settings_login(app: AppHandle, on: bool) -> Value {
    let sh = shared(&app);
    if let Err(err) = connection::set_open_at_login(on) {
        sh.log(&format!("start at login: {err}"));
    }
    snapshot(&sh)
}

// install, install-http, remove: Claude Code's settings.json.
#[tauri::command]
pub async fn settings_hooks(app: AppHandle, action: String) -> Value {
    let sh = shared(&app);
    match connection::write_hooks(&action, sh.port) {
        Ok(()) => json!({ "ok": true, "snapshot": snapshot(&sh) }),
        Err(error) => json!({ "ok": false, "kind": connection::error_kind(&error), "error": error, "snapshot": snapshot(&sh) }),
    }
}

// install, remove: Codex's hooks.json.
#[tauri::command]
pub async fn settings_codex_hooks(app: AppHandle, action: String) -> Value {
    let sh = shared(&app);
    match connection::write_codex_hooks(&action, &i18n::t(sh.lang(), "codex.waiting")) {
        Ok(()) => json!({ "ok": true, "snapshot": snapshot(&sh) }),
        Err(error) => json!({ "ok": false, "kind": connection::error_kind(&error), "error": error, "snapshot": snapshot(&sh) }),
    }
}

// install, remove: Antigravity's hooks.json.
#[tauri::command]
pub async fn settings_agy_hooks(app: AppHandle, action: String) -> Value {
    let sh = shared(&app);
    match connection::write_agy_hooks(&action) {
        Ok(()) => json!({ "ok": true, "snapshot": snapshot(&sh) }),
        // The folder she is in, for one whose path agy can't run (kind "path").
        Err(error) => {
            let folder = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|p| p.to_string_lossy().into_owned())).unwrap_or_default();
            json!({ "ok": false, "kind": connection::error_kind(&error), "error": error, "folder": folder, "snapshot": snapshot(&sh) })
        }
    }
}

// Download a pet from codex-pets.net: the first one, or one replacing a
// missing sprite, becomes her.
#[tauri::command]
pub async fn settings_fetch(app: AppHandle, reference: String) -> Value {
    let sh = shared(&app);
    let dir = sh.dir.join("pets");
    let got = tauri::async_runtime::spawn_blocking(move || fetch::download_pet(&reference, &dir)).await;
    let lang = sh.lang();
    match got {
        Ok(Ok(pet)) => {
            let current = sh.setting("pet");
            let found = data::pets(&sh.dir);
            let has_current = found.iter().any(|p| Some(p.id.as_str()) == current.as_str());
            if !has_current || found.len() == 1 {
                sh.change(json!({ "pet": pet["id"] }));
            } else {
                sh.redraw();
            }
            let warning = pet["warning"]
                .as_object()
                .map(|w| fetch::describe(lang, &fetch::PetError { code: if w["code"] == "atlas" { "atlas" } else { "unknown" }, vars: w["vars"].clone() }));
            json!({ "ok": true, "pet": pet, "warning": warning, "snapshot": snapshot(&sh) })
        }
        Ok(Err(e)) => json!({ "ok": false, "error": fetch::describe(lang, &e), "snapshot": snapshot(&sh) }),
        Err(e) => json!({ "ok": false, "error": e.to_string(), "snapshot": snapshot(&sh) }),
    }
}

#[tauri::command]
pub async fn settings_gallery(app: AppHandle, page: Option<u64>, sort: Option<String>, query: Option<String>) -> Value {
    let sh = shared(&app);
    let sort = sort.unwrap_or_default();
    let query = query.unwrap_or_default();
    let got = tauri::async_runtime::spawn_blocking(move || fetch::gallery(page.unwrap_or(1), &sort, &query)).await;
    match got {
        Ok(Ok(page)) => json!({ "ok": true, "items": page["items"], "page": page["page"], "totalPages": page["totalPages"], "total": page["total"] }),
        Ok(Err(e)) => json!({ "ok": false, "error": fetch::describe(sh.lang(), &e) }),
        Err(e) => json!({ "ok": false, "error": e.to_string() }),
    }
}

// Her updates, from the General page: look now, the latest release's page,
// install it (updates.rs). The snapshot as it is then (a look or an
// install goes on on its own thread, pushed as it goes).
#[tauri::command]
pub async fn settings_update(app: AppHandle, what: String) -> Value {
    let sh = shared(&app);
    match what.as_str() {
        "check" => {
            let sh = sh.clone();
            std::thread::spawn(move || crate::updates::check_now(&sh));
            // Its "looking" shown at once.
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        "page" => crate::updates::open_page(&sh),
        "update" => crate::updates::update_now(&sh),
        _ => {}
    }
    snapshot(&sh)
}

// Her crash reports, from the General page: the answer to her asking
// (yes: sent from now on; no: never), sent now, or the last one's issue
// opened (crash.rs). The snapshot as it is then.
#[tauri::command]
pub async fn settings_crash(app: AppHandle, what: String) -> Value {
    let sh = shared(&app);
    match what.as_str() {
        "yes" => crate::crash::answer(&sh, true),
        "no" => crate::crash::answer(&sh, false),
        "send" => crate::crash::send_now(&sh),
        "open" => {
            if let Some(url) = crate::crash::open_last(&sh) {
                crate::jump::open_url(&url);
            }
        }
        _ => {}
    }
    snapshot(&sh)
}

// Only these places, whatever the page asks: the project or codex-pets.net.
#[tauri::command]
pub async fn settings_open_site(app: AppHandle, place: String) {
    let url = match place.as_str() {
        "repo" => REPO,
        _ => fetch::SITE,
    };
    // By ShellExecute, not `cmd /c start`: run from the installed pet (no
    // console), cmd got handles that were none and never started ("the
    // handle is invalid").
    if !crate::jump::open_url(url) {
        shared(&app).log(&format!("open {url}: ShellExecute failed"));
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn island_looks() {
        assert!(super::is_island_look(&json!(null)));
        assert!(super::is_island_look(&json!({})));
        assert!(super::is_island_look(&json!({ "bg": "#2C2c2c", "op": 62, "blur": 40, "edge": "ring", "bare": true })));
        assert!(!super::is_island_look(&json!({ "bg": "#2c2c2" })));
        assert!(!super::is_island_look(&json!({ "bg": "red" })));
        assert!(!super::is_island_look(&json!({ "op": 10 })));
        assert!(!super::is_island_look(&json!({ "blur": 61 })));
        assert!(!super::is_island_look(&json!({ "edge": "glow" })));
        assert!(!super::is_island_look(&json!({ "bare": 1 })));
        assert!(!super::is_island_look(&json!({ "ink": "light" })));
    }
}

#[cfg(test)]
mod layout_tests {
    use serde_json::json;

    #[test]
    fn taskbar_layouts() {
        assert!(super::is_taskbar_layout(&json!({ "left": ["her"], "center": ["start", "apps"], "right": ["monitor", "tray", "keys", "quick", "clock"] })));
        // The sessions' tags are no part any more (data.rs drops them as it reads).
        assert!(!super::is_taskbar_layout(&json!({ "center": ["start", "apps", "tags"] })));
        assert!(super::is_taskbar_layout(&json!({ "left": ["start", "apps"], "right": ["clock", "her"] })));
        assert!(!super::is_taskbar_layout(&json!({ "left": ["her", "her"] })));
        assert!(!super::is_taskbar_layout(&json!({ "left": ["her"], "right": ["her"] })));
        assert!(!super::is_taskbar_layout(&json!({ "top": ["her"] })));
        assert!(!super::is_taskbar_layout(&json!({ "left": ["desk"] })));
        assert!(!super::is_taskbar_layout(&json!({ "left": "her" })));
        assert!(!super::is_taskbar_layout(&json!(["her"])));
    }

    #[test]
    fn widgets_on_the_taskbar() {
        let ok = |w| super::is_taskbar_layout(&json!({ "right": [w, "clock"] }));
        assert!(ok(json!({ "id": "wa1", "type": "widget", "of": "today", "size": "M" })));
        assert!(ok(json!({ "id": "wa1", "type": "widget", "of": "inbox-x2", "name": "me@example.com", "icon": "mail" })));
        assert!(ok(json!({ "id": "wa1", "type": "plugin", "of": "weather", "size": "S", "icon": "weather" })));
        assert!(ok(json!({ "id": "wa1", "type": "flex" })));
        assert!(ok(json!({ "id": "wa1", "type": "sep" })));
        // Not a plugin she runs; no widget's id; a spacer of something.
        assert!(!ok(json!({ "id": "wa1", "type": "plugin", "of": "nope" })));
        assert!(!ok(json!({ "id": "wa1", "type": "widget" })));
        assert!(!ok(json!({ "id": "wa1", "type": "widget", "of": "Not An Id" })));
        assert!(!ok(json!({ "id": "wa1", "type": "gap", "of": "today" })));
        // Its id, size, icon, name; nothing else.
        assert!(!ok(json!({ "id": "start", "type": "flex" })));
        assert!(!ok(json!({ "id": "w", "type": "flex" })));
        assert!(!ok(json!({ "id": "wa1", "type": "widget", "of": "today", "size": "L" })));
        assert!(!ok(json!({ "id": "wa1", "type": "widget", "of": "today", "icon": "nope" })));
        assert!(!ok(json!({ "id": "wa1", "type": "widget", "of": "today", "name": "x".repeat(41) })));
        assert!(!ok(json!({ "id": "wa1", "type": "flex", "colour": "#fff" })));
        assert!(!ok(json!({ "id": "wa1", "type": "clock" })));
        // Each once, 32 at most.
        assert!(!super::is_taskbar_layout(&json!({ "left": [{ "id": "wa1", "type": "sep" }], "right": [{ "id": "wa1", "type": "sep" }] })));
        let many: Vec<_> = (0..33).map(|i| json!({ "id": format!("w{i}"), "type": "gap" })).collect();
        assert!(!super::is_taskbar_layout(&json!({ "right": many })));
        assert!(super::is_taskbar_layout(&json!({ "right": many[..32] })));
    }

    #[test]
    fn what_the_taskbar_shows() {
        let layout = json!({ "left": ["her"], "right": [{ "id": "wa", "type": "widget", "of": "tokens" }, { "id": "wb", "type": "plugin", "of": "stock" }, "clock"] });
        assert!(super::is_on_taskbar(&layout, "tokens"));
        assert!(super::is_on_taskbar(&layout, "stock-aapl"));
        assert!(!super::is_on_taskbar(&layout, "today"));
        assert!(!super::is_on_taskbar(&layout, "weather"));
        assert!(!super::is_on_taskbar(&json!(null), "tokens"));
    }
}
