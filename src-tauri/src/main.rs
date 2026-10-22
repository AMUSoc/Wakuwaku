// Wakuwaku: a transparent, frameless, always-on-top desktop pet that shows
// what Claude Code, Codex and Antigravity are doing; their hooks report to a
// local port. The pages are src/pet (her window and the island's), talking
// to this side through src/pet/bridge.js.
//
// Where she is: her home is the island at the top centre; she is in it, or
// out on the desktop (out), and the island stays up and talks for her. The
// settings grow out of the island; with none up, one rises for them.
//
//   pet.rs         her window: size, place, eyes, drags, being carried, walks
//   island.rs      the island's window, and the settings
//   backplate.rs   the acrylic blur under the island
//   settings.rs    what the settings show and change
//   pointer.rs     click-through, for both windows; screen.rs the work areas
//   asks.rs        prompts answered on her or the island
//   connection.rs  the plugin, hooks in settings.json and Codex's hooks.json, start at login
//   fetch.rs       pets from codex-pets.net
//   fullscreen.rs, focus.rs, notify.rs   bits of Windows
//   jump.rs        to a session's window, when you click the session
//   tray.rs        the tray icon and the menu
//   server.rs      the port the hooks report to
//   state.rs       per-session moods; events.rs (Claude Code),
//                  events_codex.rs (Codex) and events_agy.rs (Antigravity)
//                  hook events into messages
//   data.rs        her folder, settings and pets
//   widgets.rs     plugins in the island: widgets from scripts, and the built-in ones;
//                  scripts.rs the plugins she runs for you
//   tokens.rs      today's tokens, from Claude Code's, Codex's and Antigravity's own records
//   crash.rs       crash reports, and her guard; updates.rs her updates
// The defaults (data.rs) are one json! long enough to need more than 128.
#![recursion_limit = "256"]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod asks;
mod backplate;
mod connection;
mod crash;
mod data;
mod detach;
mod dock;
mod events;
mod events_agy;
mod events_codex;
mod events_grok;
mod fetch;
mod focus;
mod fullscreen;
mod gpu;
mod i18n;
mod island;
mod jump;
mod keep;
mod launch;
mod lcu;
mod lol;
mod mobile;
mod notch;
mod notify;
mod opgg;
mod pet;
mod pointer;
mod power;
mod proxy;
mod reply;
mod review;
mod run;
mod screen;
mod scout;
mod duel;
mod deckscan;
mod cards;
mod clip;
mod history;
mod maps;
mod ask;
mod scripts;
mod server;
mod settings;
mod state;
mod talk;
mod tokens;
mod tray;
mod updates;
mod wallpaper;
mod widgets;

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Map, Value};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

// The island's own animation of taking her in, before she counts as in.
const ABSORB_MS: u64 = 380;

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

// Answers a held hook request: Some(output), or None to drop it.
pub type Responder = Box<dyn FnOnce(Option<Value>) + Send>;

// What every part shares. No two of its locks are ever held at once.
pub struct Shared {
    pub app: AppHandle,
    pub dir: PathBuf,
    pub port: u16,
    pub settings: Mutex<Map<String, Value>>,
    pub pet: Mutex<state::Pet>,
    pub pets: pet::Pets,
    pub island: Mutex<island::Island>,
    pub dock: Mutex<dock::Dock>,
    pub asks: Mutex<asks::Asks<Responder>>,
    // Plugins in the island (widgets.rs), and what today has seen.
    pub widgets: Mutex<widgets::Widgets>,
    pub today: Mutex<widgets::Today>,
    // The plugins she runs for you (scripts.rs).
    pub scripts: Mutex<scripts::Runner>,
    screens: Mutex<screen::Screens>,
    // Out of sight: hidden from the tray, or another app is full screen.
    pub hidden: AtomicBool,
    by_fullscreen: AtomicBool,
    // Nobody at the keyboard or mouse for a while (check_quiet).
    quiet: AtomicBool,
    greeted: AtomicBool,
    // When the last Codex event came (ms), 0 never: Codex runs its hooks
    // only once the person trusts them.
    pub codex_seen: std::sync::atomic::AtomicU64,
    // The same for Antigravity's, and Grok's.
    pub agy_seen: std::sync::atomic::AtomicU64,
    pub grok_seen: std::sync::atomic::AtomicU64,
    // When the last Claude Code event came (ms), 0 never: the settings tell
    // connected from installed, waiting for a session (settings.js).
    pub claude_seen: std::sync::atomic::AtomicU64,
    is_logging: bool,
    // Our windows' handles: never counted as another app full screen.
    own: Mutex<Vec<isize>>,
    // The window that had the keyboard before one of hers took it.
    prev_focus: Mutex<isize>,
    push_pending: AtomicBool,
    // Until when (ms) a loss of focus is our own doing: showing a window of
    // hers makes it the active one, and that is no click elsewhere.
    pub shuffle_until: std::sync::atomic::AtomicU64,
    // Debug only: answers to /debug/eval, by id.
    pub evals: Mutex<(u64, std::collections::HashMap<u64, std::sync::mpsc::Sender<Value>>)>,
}

impl Shared {
    pub fn setting(&self, key: &str) -> Value {
        self.settings.lock().unwrap().get(key).cloned().unwrap_or(Value::Null)
    }

    pub fn flag(&self, key: &str) -> bool {
        self.setting(key).as_bool().unwrap_or(false)
    }

    // Whether an agent's pet is out on the desktop (desktop).
    pub fn is_out(&self, agent: &str) -> bool {
        self.setting("desktop").get(agent).and_then(Value::as_bool).unwrap_or(false)
    }

    // The agents whose pets are out.
    pub fn out_agents(&self) -> Vec<&'static str> {
        data::AGENTS.into_iter().filter(|agent| self.is_out(agent)).collect()
    }

    // desktop with one agent's pet out or not.
    fn desktop_with(&self, agent: &str, out: bool) -> Value {
        let mut desktop = self.setting("desktop");
        if let Some(m) = desktop.as_object_mut() {
            m.insert(agent.into(), json!(out));
        }
        desktop
    }

    // The agent whose pet the island stands for: the session it shows (none:
    // Claude's). Its pet is let out and called back from the island, the
    // tray and the settings.
    pub fn island_agent(&self) -> &'static str {
        let now = self.pet.lock().unwrap().get(now_ms());
        data::agent_key(&now).and_then(|agent| data::AGENTS.into_iter().find(|a| *a == agent)).unwrap_or("claude")
    }

    pub fn lang(&self) -> &'static str {
        i18n::lang(self.setting("lang").as_str().unwrap_or("auto"))
    }

    pub fn hold(&self) -> state::Hold {
        match self.setting("hold").as_u64() {
            Some(secs) => state::Hold::Secs(secs),
            None => state::Hold::Seen,
        }
    }

    // How long a session at rest stays in sight with nothing new from it (ms,
    // 0 always): the idleHideMin setting, or WAKUWAKU_IDLE_HIDE_MS, for a
    // test that cannot wait (state.rs is_shown).
    pub fn idle_hide_ms(&self) -> u64 {
        static TEST: std::sync::OnceLock<Option<u64>> = std::sync::OnceLock::new();
        let test = *TEST.get_or_init(|| std::env::var("WAKUWAKU_IDLE_HIDE_MS").ok().and_then(|s| s.parse().ok()));
        test.unwrap_or_else(|| self.setting("idleHideMin").as_u64().unwrap_or(state::IDLE_HIDE_MIN) * 60 * 1000)
    }

    pub fn screens(&self) -> screen::Screens {
        self.screens.lock().unwrap().clone()
    }

    // Where a prompt can be answered: her up. Else nobody can answer here,
    // and the terminal has it at once (server.rs, apply_visibility).
    pub fn asks_seen(&self) -> bool {
        self.is_visible()
    }

    pub fn is_visible(&self) -> bool {
        !self.hidden.load(Ordering::SeqCst)
            && !self.flag("dnd")
            && !(self.flag("hideInFullscreen") && self.by_fullscreen.load(Ordering::SeqCst))
    }

    // Nobody at the keyboard or mouse for a while: she holds still, the
    // loops look less often, and Windows may run her on its efficient cores
    // at lower clocks (power.rs). Over the moment anything is touched.
    pub fn is_quiet(&self) -> bool {
        self.quiet.load(Ordering::SeqCst)
    }

    // Looked at by the clock (every second) and by the mouse's own loop.
    pub fn check_quiet(&self) -> bool {
        let after = quiet_after_ms();
        let quiet = after > 0 && power::idle_ms() >= after;
        if self.quiet.swap(quiet, Ordering::SeqCst) != quiet {
            let eco = power::process_eco(quiet);
            // WebView2's processes, where her pages' work is, with her; and
            // their memory (#14).
            let webview_eco = power::webview_eco(quiet);
            let windows = self.app.webview_windows();
            for window in windows.values() {
                power::webview_memory(window, quiet);
            }
            self.log(&format!("quiet: {quiet} (efficiency mode set: {eco}; for {webview_eco} of WebView2's processes; memory {} for {} pages)", if quiet { "low" } else { "normal" }, windows.len()));
            self.to_pages("pet:quiet", json!({ "on": quiet }));
            wake_monitor();
        }
        quiet
    }

    // Another app is full screen (watched while the setting asks).
    // The displays she is on (x, y, w, h): her island's, and
    // hers while she is out. An app full screen on another is none of hers.
    pub fn displays(&self) -> Vec<(i32, i32, i32, i32)> {
        let mut on: Vec<(i32, i32, i32, i32)> = island::display(self).into_iter().collect();
        for agent in data::AGENTS {
            if let Some(mon) = pet::is_shown(self, agent).then(|| pet::display(self, agent)).flatten() {
                if !on.contains(&mon) {
                    on.push(mon);
                }
            }
        }
        on
    }

    pub fn is_fullscreen(&self) -> bool {
        self.by_fullscreen.load(Ordering::SeqCst)
    }

    // Her home: the island (island.rs).
    pub fn home(&self) -> &'static str {
        "island"
    }

    pub fn own_window(&self, _win: &WebviewWindow) {
        #[cfg(windows)]
        if let Ok(hwnd) = _win.hwnd() {
            self.own.lock().unwrap().push(hwnd.0 as isize);
        }
    }

    // A window of hers closed (a pet called home): no longer hers.
    pub fn disown_window(&self, _win: &WebviewWindow) {
        #[cfg(windows)]
        if let Ok(hwnd) = _win.hwnd() {
            self.own.lock().unwrap().retain(|h| *h != hwnd.0 as isize);
        }
    }

    // A line in recent.log always (what a crash report tells of what came
    // before, crash.rs), and in debug.log while debugging.
    pub fn log(&self, line: &str) {
        let whole = format!("{} [tauri] {line}\n", now_ms());
        crash::tail(&self.dir, &whole);
        if !self.is_logging {
            return;
        }
        // A whole line in one write, one at a time: written in its parts
        // (writeln!), two threads' lines in the same moment came out woven
        // into each other (2026-10-11, the quit's steps). The file is kept
        // open (it was opened and closed for every line); opened again
        // should a write fail (the file taken away).
        static LOG: Mutex<Option<std::fs::File>> = Mutex::new(None);
        let mut log = LOG.lock().unwrap_or_else(|e| e.into_inner());
        let written = log.as_mut().is_some_and(|f| f.write_all(whole.as_bytes()).is_ok());
        if !written {
            *log = OpenOptions::new().create(true).append(true).open(self.dir.join("debug.log")).ok();
            if let Some(f) = log.as_mut() {
                let _ = f.write_all(whole.as_bytes());
            }
        }
    }

    // Everything the pages draw from.
    pub fn payload(&self) -> Value {
        let (mut now, second, list) = {
            let pet = self.pet.lock().unwrap();
            (pet.get(now_ms()), pet.second(), pet.list())
        };
        let settings = self.settings.lock().unwrap().clone();
        // The session shown drawn as its agent's pet, if it has one.
        let found = data::pets(&self.dir);
        let sprite = data::pet_for(&settings, &now, &found);
        let (temp, settings_open) = {
            let isl = self.island.lock().unwrap();
            (isl.temp, isl.settings_open)
        };
        now["config"] = Value::Object(settings);
        now["lang"] = json!(self.lang());
        now["sprite"] = sprite.map_or(Value::Null, |p| json!(data::asset_url(&p.sheet)));
        now["spriteVersion"] = json!(sprite.map_or(2, |p| p.version));
        // The pet of the agent the island stands for out on the desktop: the
        // island shows its portrait, not her (island.js isHome).
        now["herOut"] = json!(self.is_out(self.island_agent()));
        // The next session that wants something, for the island's second bubble.
        now["second"] = json!(second);
        // Every session, the one shown first: the open island lists the others.
        now["list"] = list;
        // The island risen only for the settings, and whether they are open:
        // then the island holds any prompt.
        now["islandTemp"] = json!(temp);
        now["settingsOpen"] = json!(settings_open);
        now["quiet"] = json!(self.is_quiet());
        // Whether a blur of what is behind the island can be put under it
        // (backplate.rs): Windows 11, transparency effects on.
        now["plate"] = json!(backplate::available());
        // Windows' mode, for her island's look at the top of the screen.
        now["winLight"] = json!(wallpaper::look().0);
        // The widgets that have something to say: those on take their turns
        // in the island while nothing needs you.
        now["widgets"] = self.widgets_said();
        // Nothing connected (Claude Code, Codex, Antigravity): the island
        // says so, naming those on this computer, and a press on it opens
        // the settings where they are (island.js).
        let unconnected = !connection::seems_connected();
        now["unconnected"] = json!(unconnected);
        if unconnected {
            now["tools"] = json!(connection::tools_here());
        }
        now
    }

    // Everything an agent's pet on the desktop draws from: the island's, but
    // of its agent's sessions alone (state.rs get_for), as its agent's pet.
    pub fn pet_payload(&self, agent: &str, island: &Value) -> Value {
        let (mut now, list) = {
            let pet = self.pet.lock().unwrap();
            (pet.get_for(agent, now_ms()), pet.list_for(agent))
        };
        for key in ["config", "lang", "quiet", "plate", "winLight", "widgets", "unconnected", "tools", "islandTemp", "settingsOpen"] {
            if let Some(v) = island.get(key) {
                now[key] = v.clone();
            }
        }
        let found = data::pets(&self.dir);
        let sprite = island["config"].as_object().and_then(|settings| data::pet_of(settings, Some(agent), &found));
        now["sprite"] = sprite.map_or(Value::Null, |p| json!(data::asset_url(&p.sheet)));
        now["spriteVersion"] = json!(sprite.map_or(2, |p| p.version));
        // Its bubble's and prompt panel's edge: the pet's own colour, or its agent's.
        now["petColor"] = json!(sprite.and_then(|p| p.color.clone()).unwrap_or_else(|| data::agent_color(agent).to_string()));
        now["list"] = list;
        now["agent"] = json!(agent);
        // A League of Legends game on: its part for this agent's pet (lol.rs).
        now["game"] = lol::view(agent);
        now
    }

    // Everything the pages draw from, for an agent's pet window as it shows.
    pub fn send_to_pet(&self, agent: &str) {
        let _ = self.app.emit_to(pet::label(agent), "pet:update", self.pet_payload(agent, &self.payload()));
    }

    // To the island and every pet's window there is.
    fn to_pages(&self, event: &str, payload: Value) {
        let _ = self.app.emit_to("island", event, payload.clone());
        for agent in data::AGENTS {
            let _ = self.app.emit_to(pet::label(agent), event, payload.clone());
        }
    }

    // The mood or the settings changed: the pages, the tray, the settings.
    pub fn redraw(&self) {
        let island = self.payload();
        // A pet's window hidden (it is home) hears no updates: it gets the
        // whole state as it shows (pet.rs apply_visibility).
        for agent in data::AGENTS {
            if pet::is_up(self, agent) {
                let _ = self.app.emit_to(pet::label(agent), "pet:update", self.pet_payload(agent, &island));
            }
        }
        let _ = self.app.emit_to("island", "pet:update", island);
        dock::push(self);
        tray::refresh(self);
        self.push_settings();
    }

    // Only a widget's words changed (the monitor's readings every couple of
    // seconds): the island's widgets alone, not the whole state to both
    // pages, the tray and the settings.
    pub fn redraw_widgets(&self) {
        let _ = self.app.emit_to("island", "pet:widgets", json!({ "widgets": self.widgets_said() }));
        self.push_settings();
    }

    // The prompts waiting changed.
    // The prompts each page holds: an agent's pet out on the desktop, its
    // own agent's (asks::agent_of); the island, the rest.
    pub fn asks_for(&self, pet: Option<&str>) -> Value {
        let out = self.out_agents();
        let asks = self.asks.lock().unwrap();
        match pet {
            Some(agent) => asks.views_where(|a| a == agent && out.contains(&a)),
            None => asks.views_where(|a| !out.contains(&a)),
        }
    }

    pub fn push_asks(&self) {
        let _ = self.app.emit_to("island", "pet:asks", self.asks_for(None));
        for agent in data::AGENTS {
            if pet::is_made(self, agent) {
                let _ = self.app.emit_to(pet::label(agent), "pet:asks", self.asks_for(Some(agent)));
            }
        }
        dock::push_asks(self);
        self.push_settings();
    }

    // The open settings follow what happens, a few times a second at most.
    pub fn push_settings(&self) {
        if !self.island.lock().unwrap().settings_open || self.push_pending.swap(true, Ordering::SeqCst) {
            return;
        }
        let sh = shared(&self.app);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(250));
            sh.push_pending.store(false, Ordering::SeqCst);
            let _ = sh.app.emit_to("island", "settings:changed", settings::snapshot(&sh));
        });
    }

    // A prompt answered on her or on the phone (mobile.rs): the hook held
    // for it answered.
    pub fn answer(&self, id: u64, choice: &Value) -> bool {
        let lang = self.lang();
        let ok = self.asks.lock().unwrap().answer(id, choice, lang);
        self.log(&format!("answer {id}: {choice} -> {ok}"));
        if ok {
            self.push_asks();
            // An approval given on her, for today's count.
            if matches!(choice["action"].as_str(), Some("allow" | "always")) {
                self.count_today(|t| t.approvals += 1);
            }
        }
        ok
    }

    // A message from the hooks (or a reaction of our own).
    pub fn apply(&self, msg: &Value) -> bool {
        let out = self.pet.lock().unwrap().apply(msg, now_ms(), self.hold());
        let Some(out) = out else { return false };
        if out.turns > 0 || out.worked_ms > 0 {
            // Antigravity's and Grok's: their approvals never come to her (widgets.rs agy_turns).
            let is_agy = msg["agent"] == "agy" || msg["agent"] == "grok";
            self.count_today(|t| {
                t.turns += out.turns;
                if is_agy {
                    t.agy_turns += out.turns;
                }
                t.worked_ms += out.worked_ms;
            });
        }
        if out.changed {
            self.redraw();
        }
        if let Some(react) = out.react {
            match react["agent"].as_str() {
                Some(agent) => {
                    let _ = self.app.emit_to("island", "pet:react", react.clone());
                    let _ = self.app.emit_to(pet::label(agent), "pet:react", react);
                }
                None => self.to_pages("pet:react", react),
            }
        }
        if !self.flag("dnd") {
            // On the phone as well (ntfy), by its agent and how it is alone.
            let agent = state::agent_key(msg["agent"].as_str().unwrap_or(""));
            for (mood, project) in out.alerts {
                self.alert(&mood, &project);
                mobile::push(&shared(&self.app), agent, &mood);
            }
        }
        true
    }

    // --- Widgets ----------------------------------------------------------------------

    fn list_setting(&self, key: &str) -> Vec<String> {
        self.setting(key).as_array().into_iter().flatten().filter_map(|v| v.as_str().map(str::to_string)).collect()
    }

    pub fn is_widget_on(&self, id: &str) -> bool {
        !self.list_setting("widgetsOff").iter().any(|o| o == id)
    }

    // On in the island: a built-in one is worked out then.
    pub fn is_widget_wanted(&self, id: &str) -> bool {
        self.is_widget_on(id)
    }

    // The widgets with something to say, each saying whether it is on.
    fn widgets_said(&self) -> Value {
        Value::Array(self.widgets_view().as_array().into_iter().flatten().filter(|w| !w["value"].is_null()).cloned().collect())
    }

    // Every widget, in the person's order, on or off.
    pub fn widgets_view(&self) -> Value {
        let (off, order) = (self.list_setting("widgetsOff"), self.list_setting("widgetOrder"));
        self.widgets.lock().unwrap().view(now_ms(), &off, &order, &scripts::owner_of)
    }

    // A widget from a script: there it is, and it may open the island for itself.
    pub fn put_widget(&self, v: &Value) -> Result<(), &'static str> {
        let put = self.widgets.lock().unwrap().put(v, now_ms())?;
        self.redraw();
        if let Some(words) = put.nudge {
            self.nudge_widget(&put.id, &words);
        }
        Ok(())
    }

    // The island opens for a widget, once: only in sight, when nudges are
    // allowed and the widget is on, and while no session needs you.
    pub fn nudge_widget(&self, id: &str, words: &str) {
        self.nudge_widget_to(id, words, Value::Null);
    }

    // The same, and a click on the island while it says so opens what
    // `open` names ({ tab, … }: a letter on the Mail page) instead of the
    // settings as they were.
    pub fn nudge_widget_to(&self, id: &str, words: &str, open: Value) {
        if !self.is_visible() || self.setting("widgetNudge") == false || !self.is_widget_on(id) || self.pet.lock().unwrap().wants_you() {
            return;
        }
        let Some((label, value, private)) = self.widgets.lock().unwrap().label_of(id) else { return };
        self.to_pages("pet:nudge", json!({ "id": id, "words": words, "label": label, "value": value, "private": private, "open": open }));
    }

    // Today's counts changed: kept, and the widget says so.
    pub fn count_today(&self, change: impl FnOnce(&mut widgets::Today)) {
        let words = {
            let mut today = self.today.lock().unwrap();
            today.roll(&widgets::local_date());
            change(&mut today);
            today.save(&self.dir);
            today.words()
        };
        if self.widgets.lock().unwrap().set_built_in("today", words, now_ms()) {
            self.redraw();
        }
    }

    // A chime from her home, and a system notification if asked for.
    fn alert(&self, mood: &str, project: &str) {
        let _ = self.app.emit_to("island", "pet:alert", json!({ "mood": mood }));
        let group = match mood {
            "waiting" => "waiting",
            "error" => "error",
            _ => "done",
        };
        if self.setting("notify").get(group).and_then(Value::as_bool) != Some(true) {
            return;
        }
        let lang = self.lang();
        let whose = if project.is_empty() { String::new() } else { i18n::t(lang, "notify.project").replace("{project}", project) };
        let body = i18n::t(lang, &format!("notify.{mood}")).replace("{project}", &whose);
        let sh = shared(&self.app);
        // Off this thread: a toast can take a moment.
        std::thread::spawn(move || {
            if let Err(err) = notify::show(&body) {
                sh.log(&format!("notification failed: {err}"));
            }
        });
    }

    // A hello when she first appears: once, from whichever window is the one up.
    pub fn greet(&self) {
        if self.greeted.swap(true, Ordering::SeqCst) {
            return;
        }
        let is_stale = connection::hooks_status(self.port) == "stale" || connection::codex_hooks_status() == "stale";
        self.apply(&json!({ "react": "wave", "say": { "key": if is_stale { "say.hooksStale" } else { "say.arrived" } } }));
    }

    pub fn change(self: &Arc<Self>, patch: Value) {
        let Value::Object(patch) = patch else { return };
        let touches_visibility = ["dnd", "display", "desktop", "hideInFullscreen", "dock"].iter().any(|k| patch.contains_key(*k));
        let touches_top = ["settingsPin", "settingsTop"].iter().any(|k| patch.contains_key(*k));
        let touches_monitor = ["monitor", "widgetsOff"].iter().any(|k| patch.contains_key(*k));
        let touches_desktop = patch.contains_key("desktop");
        let touches_mobile = ["mobile", "mobileBind"].iter().any(|k| patch.contains_key(*k));
        let touches_island_display = patch.contains_key("islandDisplay");
        {
            let mut settings = self.settings.lock().unwrap();
            settings.extend(patch);
            data::save(&self.dir, &settings);
        }
        if touches_monitor {
            wake_monitor();
        }
        self.redraw();
        tray::refresh_menu(self);
        if touches_visibility {
            self.apply_visibility();
        }
        // A pet out or home: its agent's prompts go to it, or back to the island.
        if touches_desktop {
            self.push_asks();
        }
        if touches_top {
            island::keep_on_top(self);
        }
        if touches_island_display {
            island::moved(self);
        }
        // Phone access on, off, or at another address: listened for there.
        if touches_mobile {
            mobile::refresh(self);
        }
    }

    // Another app full screen or no longer: she steps aside or comes back.
    // Looked at on a thread of its own (setup), every 1.5 s. Every look
    // logged for 10 s after a change: what it saw.
    pub fn look_fullscreen(self: &Arc<Self>) {
        static CHANGED: Mutex<Option<Instant>> = Mutex::new(None);
        let watching = fullscreen::AVAILABLE && self.flag("hideInFullscreen");
        let own = self.own.lock().unwrap().clone();
        let cover = if watching { fullscreen::covering(&own) } else { Some(None) };
        let on = self.displays();
        let is_full = cover.map(|c| c.is_some_and(|mon| on.is_empty() || on.contains(&mon)));
        let changed = is_full.is_some_and(|f| self.by_fullscreen.swap(f, Ordering::SeqCst) != f);
        if !changed {
            if CHANGED.lock().unwrap().is_some_and(|at| at.elapsed() < Duration::from_secs(10)) {
                let saw = is_full.map_or("one of hers in front".to_string(), |f| f.to_string());
                self.log(&format!("full screen looked at: {saw}"));
            }
            return;
        }
        *CHANGED.lock().unwrap() = Some(Instant::now());
        let is_full = is_full == Some(true);
        let which = if is_full { format!(", {}", fullscreen::front()) } else { String::new() };
        let which = format!("{which}; hers on {:?}", self.displays());
        self.log(&format!("another app full screen: {is_full}{which}"));
        let t = Instant::now();
        self.apply_visibility();
        self.log(&format!("full screen: {} in {} ms", if is_full { "stepped aside" } else { "back" }, t.elapsed().as_millis()));
    }

    pub fn apply_visibility(self: &Arc<Self>) {
        for agent in data::AGENTS {
            // Out: its window made; home: closed (not kept hidden: a WebView
            // keeps its memory).
            pet::open_or_close(self, agent);
            pet::apply_visibility(self, agent);
        }
        island::apply_visibility(self);
        dock::apply_visibility(self);
        // Out of sight, nobody can answer here: the terminal has them.
        if !self.asks_seen() {
            let n = self.asks.lock().unwrap().dismiss_all();
            if n > 0 {
                self.log(&format!("{n} prompt(s) handed to the terminal: out of sight"));
                self.push_asks();
            }
        }
        tray::refresh(self);
    }

    pub fn set_hidden(self: &Arc<Self>, is_hidden: bool) {
        self.hidden.store(is_hidden, Ordering::SeqCst);
        self.apply_visibility();
        tray::refresh_menu(self);
        self.push_settings();
    }

    // Show a window of hers. Once created, a Tauri window shows activated
    // (SW_SHOW): the active one of this app, and the foreground one if this
    // app had it (the settings open). Neither is the person clicking
    // elsewhere, so the island ignores the loss of focus for a moment; and
    // open settings get the keyboard straight back.
    pub fn show_window(&self, win: &WebviewWindow) {
        self.shuffle_until.store(now_ms() + 500, Ordering::SeqCst);
        let _ = win.show();
        if self.island.lock().unwrap().settings_open && win.label() != "island" {
            if let Some(island) = self.app.get_webview_window("island") {
                let _ = island.set_focus();
            }
        }
    }

    // Quit (the menu's Quit, POST /quit), never taking longer than 8 s: a
    // quit once hung (80 s, 2026-10-11). Ended then, whatever is stuck.
    pub fn quit(self: &Arc<Self>) {
        if QUITTING.swap(true, Ordering::SeqCst) {
            return;
        }
        self.log("quitting");
        // Her sessions kept for when she is up again (keep.rs).
        keep::save(self);
        let watched = self.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(8));
            watched.log("quitting: not done in 8 s: ended");
            end_now();
        });
        // Off the main thread: what the exit waits on reaches the page through it.
        let going = self.clone();
        std::thread::spawn(move || going.app.exit(0));
    }

    pub fn is_shuffling(&self) -> bool {
        now_ms() < self.shuffle_until.load(Ordering::SeqCst)
    }

    // A window of hers takes the keyboard (the settings, a box to type in),
    // or lets go of it: then the window that had it before gets it back, if
    // hers still has it (a click elsewhere already moved it on).
    pub fn set_keyboard(&self, window: &WebviewWindow, on: bool) {
        #[cfg(windows)]
        let mine = window.hwnd().map(|h| h.0 as isize).unwrap_or(0);
        #[cfg(not(windows))]
        let mine = 0isize;
        if on {
            let before = focus::foreground();
            if before != mine && !self.own.lock().unwrap().contains(&before) {
                *self.prev_focus.lock().unwrap() = before;
            }
            let _ = window.set_focusable(true);
            // To the front by borrowing the input of the window there
            // (jump::bring_window): when the foreground is not hers to take,
            // tao's way (an Alt pressed for it) went to the window in front,
            // its menu bar opened, the keyboard left there (with Narrator on,
            // 2026-10-14).
            #[cfg(windows)]
            crate::jump::bring_window(mine);
            let _ = window.set_focus();
        } else {
            let _ = window.set_focusable(false);
            let prev = *self.prev_focus.lock().unwrap();
            if focus::foreground() == mine {
                focus::set_foreground(prev);
            }
        }
        self.log(&format!("{}: keyboard {}", window.label(), if on { "taken" } else { "let go" }));
    }
}

// --- Where she is -----------------------------------------------------------------

// Her home: the island (she goes into it, back from the desktop).
pub fn set_display(sh: &Arc<Shared>, display: &str) {
    let out = sh.out_agents();
    if sh.setting("display").as_str() == Some(display) && out.is_empty() {
        return;
    }
    for agent in &out {
        pet::hide_now(sh, agent);
    }
    let home: Map<String, Value> = data::AGENTS.iter().map(|agent| (agent.to_string(), json!(false))).collect();
    sh.change(json!({ "display": display, "desktop": home }));
}

// An agent's pet out on the desktop where it was last left, or back home,
// from the menu or the settings.
pub fn set_out(sh: &Arc<Shared>, agent: &str, out: bool) {
    if sh.is_out(agent) == out {
        return;
    }
    // A window kept (being carried home): to where it was left; none, it is
    // made there (apply_visibility).
    if out && pet::is_made(sh, agent) {
        pet::return_to_spot(sh, agent);
    }
    sh.change(json!({ "desktop": sh.desktop_with(agent, out) }));
}

// Carried out of the island: out where it is (its spot not gone back to).
pub fn set_out_now(sh: &Arc<Shared>, agent: &str) {
    sh.change(json!({ "desktop": sh.desktop_with(agent, true) }));
}

// Brought back close to the island: it takes the pet in from where it is.
pub fn absorb(sh: &Arc<Shared>, agent: &str, point: (i32, i32)) {
    sh.log(&format!("absorbed {agent} from {point:?}"));
    pet::hide_now(sh, agent);
    island::absorb(sh, point);
    let sh = sh.clone();
    let agent = agent.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(ABSORB_MS));
        sh.change(json!({ "desktop": sh.desktop_with(&agent, false) }));
        sh.island.lock().unwrap().absorbing = false;
        sh.apply_visibility();
    });
}

// Started again while running, or "back to the corner": find her.
pub fn come_home(sh: &Arc<Shared>) {
    sh.hidden.store(false, Ordering::SeqCst);
    for agent in sh.out_agents() {
        pet::come_home(sh, agent);
    }
    sh.apply_visibility();
    tray::refresh_menu(sh);
}

pub(crate) fn shared(app: &AppHandle) -> Arc<Shared> {
    app.state::<Arc<Shared>>().inner().clone()
}

// --- From the pages -----------------------------------------------------------------
// Async, so none of them runs on the main thread while it waits on a lock.
// `window` says which page: "pet" or "island".

fn is_island(window: &WebviewWindow) -> bool {
    window.label() == "island"
}

// The side island's page (dock.rs).
fn is_dock(window: &WebviewWindow) -> bool {
    window.label() == dock::LABEL
}

// The agent of a pet's window (pet-codex: codex); the island's, Claude's.
fn agent_of(window: &WebviewWindow) -> String {
    pet::agent_of(window.label()).unwrap_or("claude").to_string()
}

#[tauri::command]
async fn pet_ready(app: AppHandle, window: WebviewWindow) {
    let sh = shared(&app);
    if window.label() == scout::LABEL {
        scout::ready(&sh);
        return;
    }
    if window.label() == duel::LABEL {
        duel::ready(&sh);
        return;
    }
    if window.label() == duel::FRAMES {
        duel::frames_ready(&sh);
        return;
    }
    if window.label() == duel::DECK {
        duel::deck_ready(&sh);
        return;
    }
    if is_dock(&window) {
        dock::ready(&sh);
        dock::push_asks(&sh);
        return;
    }
    if is_island(&window) { island::ready(&sh) } else { pet::ready(&sh, &agent_of(&window)) }
    let views = sh.asks_for(pet::agent_of(window.label()));
    let _ = app.emit_to(window.label(), "pet:asks", views);
}

#[tauri::command]
async fn pet_hover(app: AppHandle, window: WebviewWindow, is_over: bool) {
    let sh = shared(&app);
    if is_dock(&window) {
        dock::hover(&sh, is_over)
    } else if is_island(&window) {
        island::hover(&sh, is_over)
    } else {
        pet::hover(&sh, &agent_of(&window), is_over)
    }
}

#[tauri::command]
async fn pet_drag_start(app: AppHandle, window: WebviewWindow) {
    if !is_island(&window) {
        pet::drag_start(&shared(&app), &agent_of(&window));
    }
}

// The button went up over her: that ends a carry as well (the island may
// have lost the button on the way).
#[tauri::command]
async fn pet_drag_end(app: AppHandle, window: WebviewWindow) {
    if !is_island(&window) {
        pet::let_go(&shared(&app), &agent_of(&window));
    }
}

// Her menu.
#[tauri::command]
async fn pet_menu(app: AppHandle, window: WebviewWindow) {
    let sh = shared(&app);
    let over = match pet::agent_of(window.label()).and_then(|a| data::AGENTS.into_iter().find(|x| *x == a)) {
        Some(agent) => tray::Over::Her(agent),
        None => tray::Over::Home,
    };
    let menu = tray::menu(&sh, over);
    if let Ok(menu) = menu {
        // A Windows menu closes at once unless its window can come to the
        // front, which these cannot (they never take focus): for the menu, it may.
        let was_open = sh.island.lock().unwrap().settings_open && is_island(&window);
        let _ = window.set_focusable(true);
        let _ = window.popup_menu(&menu);
        if !was_open {
            let _ = window.set_focusable(false);
        }
    }
}

// On her: out of the island, she flies back into it; as the pet on her own,
// the settings. (The island has no double-click: one click opens the settings.)
#[tauri::command]
async fn pet_double_click(app: AppHandle, window: WebviewWindow) {
    let sh = shared(&app);
    if !is_island(&window) {
        pet::double_click(&sh, &agent_of(&window));
    }
}

// The room a shape of the island needs; above her, the prompt panel's size.
#[tauri::command]
async fn pet_panel(app: AppHandle, window: WebviewWindow, measured: Value) {
    let sh = shared(&app);
    if is_island(&window) {
        island::set_room(&sh, &measured);
    } else {
        let size = match (measured["width"].as_f64(), measured["height"].as_f64()) {
            (Some(w), Some(h)) if w > 0.0 && h > 0.0 => Some((w.ceil(), h.ceil())),
            _ => None,
        };
        pet::set_panel(&sh, &agent_of(&window), size);
    }
}

// A game of a row in the game's ten, whole, for its scoreboard (scout.rs).
#[tauri::command]
async fn scout_game(app: AppHandle, id: String) -> Value {
    scout::kept_game(&shared(&app), &id)
}

// Rows of these games kept, as hers (who) in them, for a champion's or a
// position's games opened (scout.rs).
#[tauri::command]
async fn scout_rows(app: AppHandle, ids: Vec<String>, who: String) -> Value {
    scout::rows_of(&shared(&app), &ids, &who)
}

// A game's timeline (each minute's gold of the ten, kills, items, skills),
// for its scoreboard's curves: one request the first time (scout.rs).
#[tauri::command]
async fn scout_timeline(app: AppHandle, id: String) -> Value {
    let sh = shared(&app);
    tauri::async_runtime::spawn_blocking(move || scout::timeline(&sh, &id)).await.unwrap_or(Value::Null)
}

// A game reviewed by the agents asked (review.rs): those not yet done
// started, each once; the reviews as they are.
#[tauri::command]
async fn scout_review_start(app: AppHandle, id: String, who: String, agents: Vec<String>) -> Value {
    let sh = shared(&app);
    tauri::async_runtime::spawn_blocking(move || review::start(&sh, &id, &who, &agents)).await.unwrap_or(Value::Null)
}

// A game's reviews as they are (review.rs).
#[tauri::command]
async fn scout_review(app: AppHandle, id: String) -> Value {
    review::shown(&shared(&app), &id)
}

// A champion's build and matchup from OP.GG, through Claude (opgg.rs):
// kept a day, or asked now (about twenty seconds).
#[tauri::command]
async fn scout_opgg(app: AppHandle, champion: u64, position: String, opponent: Option<u64>) -> Value {
    let sh = shared(&app);
    tauri::async_runtime::spawn_blocking(move || opgg::ask(&sh, champion, &position, opponent.filter(|o| *o > 0))).await.unwrap_or(Value::Null)
}

// A position's strongest champions from OP.GG, through Claude, for
// champion select (opgg.rs): kept a day.
#[tauri::command]
async fn scout_opgg_meta(app: AppHandle, position: String) -> Value {
    let sh = shared(&app);
    tauri::async_runtime::spawn_blocking(move || opgg::meta(&sh, &position)).await.unwrap_or(Value::Null)
}

// Her own champions against an opponent in champion select, by OP.GG's
// lane matchups through Claude (opgg.rs): kept a day.
#[tauri::command]
async fn scout_opgg_picks(app: AppHandle, opponent: u64, position: String, candidates: Vec<u64>) -> Value {
    let sh = shared(&app);
    tauri::async_runtime::spawn_blocking(move || opgg::picks(&sh, opponent, &position, &candidates)).await.unwrap_or(Value::Null)
}

// The game's ten put away by its close button (scout.rs).
#[tauri::command]
async fn scout_close(app: AppHandle) {
    let sh = shared(&app);
    sh.log("scout: clicked away");
    scout::show(&sh, false);
}

// The folder the phone's files go to (mobile.rs drop), from the island's word of one.
#[tauri::command]
async fn open_drops() {
    let dir = mobile::drops_dir();
    let _ = std::fs::create_dir_all(&dir);
    jump::open_url(&dir.to_string_lossy());
}

// Master Duel's helper (duel.rs): put away by its ✕, or asked again.
#[tauri::command]
async fn duel_close(app: AppHandle) {
    duel::show(&shared(&app), false);
}

// Her deck to look over and put right (duel.rs).
#[tauri::command]
async fn duel_deck_open(app: AppHandle) {
    duel::open_deck(&shared(&app));
}

#[tauri::command]
async fn duel_deck_save(app: AppHandle, deck: Value) {
    duel::save_deck(&shared(&app), &deck);
}

// The deck window's drop-down: cards by what is typed.
#[tauri::command]
async fn duel_cards(app: AppHandle, q: String) -> Value {
    let sh = shared(&app);
    tauri::async_runtime::spawn_blocking(move || duel::search_cards(&sh, &q)).await.unwrap_or(Value::Null)
}

#[tauri::command]
async fn duel_crew_ask(app: AppHandle, agent: String, q: String) {
    let sh = shared(&app);
    tauri::async_runtime::spawn_blocking(move || duel::ask(&sh, &agent, &q)).await.ok();
}

#[tauri::command]
async fn duel_crew_typing(app: AppHandle, on: bool) {
    let sh = shared(&app);
    tauri::async_runtime::spawn_blocking(move || duel::typing(&sh, on)).await.ok();
}

#[tauri::command]
async fn duel_card_info(app: AppHandle, cards: Vec<(Option<u64>, String)>) -> Value {
    let sh = shared(&app);
    tauri::async_runtime::spawn_blocking(move || duel::card_info(&sh, &cards)).await.unwrap_or(Value::Null)
}

#[tauri::command]
async fn duel_card_arts(app: AppHandle, id: u64) -> Value {
    let sh = shared(&app);
    tauri::async_runtime::spawn_blocking(move || duel::card_arts(&sh, id)).await.unwrap_or(Value::Null)
}

// Read again with a theme from her, the cards she marked right kept.
#[tauri::command]
async fn duel_deck_rescan(app: AppHandle, theme: String, kept: Vec<(String, String)>) {
    duel::scan_deck(&shared(&app), &theme, kept, false);
}

// The bar's card out or in; its 掃描牌組; her saved decks.
#[tauri::command]
async fn duel_fly(app: AppHandle, open: bool) {
    let sh = shared(&app);
    tauri::async_runtime::spawn_blocking(move || duel::fly(&sh, open)).await.ok();
}

#[tauri::command]
async fn duel_watch(app: AppHandle, on: bool) {
    let sh = shared(&app);
    tauri::async_runtime::spawn_blocking(move || duel::watch(&sh, on)).await.ok();
}

#[tauri::command]
async fn duel_scan(app: AppHandle) {
    let sh = shared(&app);
    tauri::async_runtime::spawn_blocking(move || duel::scan(&sh)).await.ok();
}

#[tauri::command]
async fn duel_deck_use(app: AppHandle, id: String) {
    let sh = shared(&app);
    tauri::async_runtime::spawn_blocking(move || duel::use_deck(&sh, &id)).await.ok();
}

#[tauri::command]
async fn duel_deck_drop(app: AppHandle, id: String) {
    let sh = shared(&app);
    tauri::async_runtime::spawn_blocking(move || duel::drop_deck(&sh, &id)).await.ok();
}

#[tauri::command]
async fn duel_deck_new(app: AppHandle) {
    let sh = shared(&app);
    tauri::async_runtime::spawn_blocking(move || duel::new_deck(&sh)).await.ok();
}

#[tauri::command]
async fn duel_deck_close(app: AppHandle) {
    duel::close_deck(&shared(&app));
}

#[tauri::command]
async fn duel_again(app: AppHandle) {
    let sh = shared(&app);
    tauri::async_runtime::spawn_blocking(move || duel::again(&sh)).await.ok();
}

#[tauri::command]
async fn pet_answer(app: AppHandle, id: u64, choice: Value) -> bool {
    shared(&app).answer(id, &choice)
}

#[tauri::command]
async fn pet_dismiss(app: AppHandle, id: u64) {
    let sh = shared(&app);
    if sh.asks.lock().unwrap().dismiss(id) {
        sh.log(&format!("prompt {id} handed to the terminal: asked to on her"));
        sh.push_asks();
    }
}

// Typing needs the keyboard, which these windows do not take otherwise.
#[tauri::command]
async fn pet_keyboard(app: AppHandle, window: WebviewWindow, needs: bool) {
    let sh = shared(&app);
    // The open settings keep it whatever a prompt's box does.
    if !needs && is_island(&window) && sh.island.lock().unwrap().settings_open {
        return;
    }
    sh.set_keyboard(&window, needs);
}

#[tauri::command]
async fn pet_walk(app: AppHandle, window: WebviewWindow, dx: f64, ms: f64) -> bool {
    if is_island(&window) {
        return false;
    }
    let sh = shared(&app);
    let agent = agent_of(&window);
    tauri::async_runtime::spawn_blocking(move || pet::walk(&sh, &agent, dx, ms)).await.unwrap_or(false)
}

#[tauri::command]
async fn pet_walk_stop(app: AppHandle, window: WebviewWindow) {
    pet::stop_walking(&shared(&app), &agent_of(&window));
}

#[tauri::command]
async fn island_holding(app: AppHandle, is_holding: bool) {
    island::set_holding(&shared(&app), is_holding);
}

// Under her island, a blur of what is behind it (backplate.rs): where the
// page puts it ({ x, y, width, height } in its px, set in from the island's
// edge; the size of the page it measured in; Windows' mode), faded in over
// fade ms; none, faded out. Measured in a window of another size (resized
// meanwhile), it is not shown: the page sends it again once resized.
#[tauri::command]
async fn island_plate(app: AppHandle, rect: Option<Value>, fade: u32) {
    let sh = shared(&app);
    let Some(r) = rect else { return backplate::set(&sh, None, fade) };
    let ((ox, oy), (ww, wh), sf) = island::frame(&sh);
    let num = |key: &str| r[key].as_f64().unwrap_or(0.0);
    if (num("vw") * sf - ww as f64).abs() > 2.0 || (num("vh") * sf - wh as f64).abs() > 2.0 {
        sh.log(&format!("plate: measured in a {}x{} page, the window {ww}x{wh} at {sf}: not shown", num("vw"), num("vh")));
        return;
    }
    let (x, y, w, h) = backplate::on_screen((ox, oy), sf, (num("x"), num("y"), num("width"), num("height")));
    let plate = (w > 0 && h > 0).then_some(backplate::Plate { x, y, w, h, dark: r["dark"].as_bool() != Some(false) });
    backplate::set(&sh, plate, fade);
}

// The drop pinched off: she is out, under the cursor, her middle this far
// from it. The button is still held on the island; island_drop says when it
// is let go.
#[tauri::command]
async fn island_release(app: AppHandle, offset: Value) {
    let sh = shared(&app);
    sh.log(&format!("island: release {offset}"));
    let agent = sh.island_agent();
    if !sh.is_out(agent) {
        pet::carry(&sh, agent, &offset);
    }
}

#[tauri::command]
async fn island_drop(app: AppHandle) {
    let sh = shared(&app);
    sh.log("island: drop");
    // The pet carried out (the island may show another's by now).
    for agent in data::AGENTS {
        if pet::is_carried(&sh, agent) {
            pet::let_go(&sh, agent);
        }
    }
}

// A page's answer to /debug/eval.
#[tauri::command]
async fn debug_result(app: AppHandle, id: u64, value: Value) {
    if let Some(tx) = shared(&app).evals.lock().unwrap().1.remove(&id) {
        let _ = tx.send(value);
    }
}

// A session clicked (in the island, the settings, the panel): to its window.
// False when there is nowhere to go.
#[tauri::command]
async fn session_jump(app: AppHandle, id: String) -> bool {
    let sh = shared(&app);
    tauri::async_runtime::spawn_blocking(move || jump_to(&sh, &id)).await.unwrap_or(false)
}

// To a session's window (jump.rs), by what the pet knows of it. Blocking: it
// looks through the processes and windows.
pub fn jump_to(sh: &Shared, id: &str) -> bool {
    let target = sh.pet.lock().unwrap().jump_target(id);
    let went = target.is_some_and(|(chain, hints)| {
        let hints: Vec<&str> = hints.iter().map(String::as_str).collect();
        jump::go(id, &chain, &hints)
    });
    sh.log(&format!("jump to {id}: {}", if went { "there" } else { "nowhere to go" }));
    went
}

// The monitor's thread woken from its wait (setup): its settings changed
// (Shared::change), the quiet began or ended (Shared::check_quiet).
static MONITOR_WAKE: (Mutex<bool>, std::sync::Condvar) = (Mutex::new(false), std::sync::Condvar::new());

fn wake_monitor() {
    let (woken, cv) = &MONITOR_WAKE;
    *woken.lock().unwrap() = true;
    cv.notify_one();
}

// Nobody at the keyboard or mouse this long: quiet (Shared::check_quiet).
// WAKUWAKU_QUIET_AFTER_MS, for a test that cannot wait; 0 never.
pub const QUIET_AFTER_MS: u64 = 90_000;

fn quiet_after_ms() -> u64 {
    static AFTER: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *AFTER.get_or_init(|| std::env::var("WAKUWAKU_QUIET_AFTER_MS").ok().and_then(|s| s.parse().ok()).unwrap_or(QUIET_AFTER_MS))
}

// What WebView2 is started with: wry's own (its Office and PDF parts and
// SmartScreen off); her two pages in one renderer process, not one each
// (same origin, same show; the second was 90 MB and its own idle); the
// network service in its browser process, not a process of its own (#14;
// the storage service's process has no such switch any more: it stays);
// and rendering on the graphics card that drives her display
// (gpu.rs, #13). WebView2 takes them once, for all her windows (another
// set refused): worked out as the first is made, her home's display then
// the primary one. WAKUWAKU_BROWSER_ARGS, more of them (to try one).
pub fn browser_args(sh: &Shared) -> &'static str {
    static ARGS: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ARGS.get_or_init(|| {
        let mut args = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --enable-features=NetworkServiceInProcess2 --renderer-process-limit=1".to_string();
        match island::display(sh).and_then(gpu::switch_for) {
            Some((switch, display)) => {
                sh.log(&format!("webview: rendering on the card driving {display} ({switch})"));
                args = format!("{args} {switch}");
            }
            None => sh.log("webview: its card left to Chromium (her display's card not told, or it renders nothing)"),
        }
        if let Ok(more) = std::env::var("WAKUWAKU_BROWSER_ARGS") {
            args = format!("{args} {more}");
        }
        args
    })
}

// Where WebView2 keeps its own files for her pages: in her data folder
// (portable, as the rest), not Windows' shared place for the app id. Two
// copies of her sharing that place with different browser arguments cannot
// both start (WebView2 refuses the second, 0x8007139F): an older build
// beside this one, as in a seamless swap, or a test copy.
pub fn webview_dir(sh: &Shared) -> PathBuf {
    sh.dir.join("webview")
}

// A thread with a name, for whoever looks at her threads' CPU.
pub fn spawn_named<F: FnOnce() + Send + 'static>(name: &str, f: F) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new().name(name.to_string()).spawn(f).expect("a thread")
}

#[tauri::command]
async fn pet_log(app: AppHandle, window: WebviewWindow, line: String) {
    shared(&app).log(&format!("{} page: {line}", window.label()));
}

// --- Started by the SessionStart hook ----------------------------------------------------

// The event a hook gives on stdin, if it comes soon.
fn read_event() -> Option<Value> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut text = String::new();
        let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut text);
        let _ = tx.send(text);
    });
    rx.recv_timeout(Duration::from_millis(1000)).ok().and_then(|text| serde_json::from_str(&text).ok())
}

// Start her, detached so she outlives the hook and the session (she greets on
// her own), out of its agent's job and inheriting no handle (detach.rs).
fn start_detached() {
    let Ok(exe) = std::env::current_exe() else { return };
    detach::spawn(&exe, &[]);
}

// If no pet answers on the port, start one (unless the person quit her:
// data.rs mark_quit); if one does, hand her the event. Then leave, quick and
// quiet, whatever happens.
fn ensure_running(port: u16) {
    let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_millis(800))).build().new_agent();
    let up = agent
        .get(&format!("http://127.0.0.1:{port}/health"))
        .call()
        .ok()
        .and_then(|mut res| res.body_mut().read_to_string().ok())
        .is_some_and(|body| body.contains("wakuwaku"));
    let event = read_event();
    if up {
        if let Some(event) = event {
            let _ = agent.post(&format!("http://127.0.0.1:{port}/hook?from=wakuwaku")).send_json(&event);
        }
    } else if data::may_start(&data::folder()) {
        start_detached();
    }
}

// --- Run by Codex's hooks -----------------------------------------------------------------

// Codex runs its hooks as commands: this program with CODEX_FLAG, for every
// event. Hand her the event, and print her answer to a prompt, the decision
// Codex reads; nothing else, since Codex takes what a hook prints as words
// for the model. SessionStart starts her when she is not up (unless the
// person quit her: data.rs mark_quit). Whatever
// happens, leave with 0, so Codex never shows a failed hook.
// An event handed to her port with no answer waited for (none is needed):
// the request written whole and the connection closed at once. False when
// she is not up: no one listens there (asked of Windows' table in a moment:
// a connection tried would take it 2 s to give up on, and the agent waits
// on its hook meanwhile). Up but not yet answering (her first second or two:
// her port is held at once, answered once she is set up), it waits in her
// port's queue till she is, and nothing is lost.
fn post_and_go(port: u16, agent: &str, event: &Value) -> bool {
    if !jump::is_listening(port) {
        return false;
    }
    let to = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let Ok(mut stream) = std::net::TcpStream::connect_timeout(&to, Duration::from_millis(100)) else { return false };
    let body = event.to_string();
    let head = format!(
        "POST /hook?from=wakuwaku&agent={agent} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let sent = stream.write_all(head.as_bytes()).and_then(|()| stream.write_all(body.as_bytes())).is_ok();
    let _ = stream.shutdown(std::net::Shutdown::Write);
    sent
}

// Codex's hook (connection::CODEX_FLAG): the event handed to her port, with
// where the Codex session runs (the processes above this one, for going to
// its window from the pet, jump.rs). A prompt waits for her answer, which is
// printed for Codex; any other event is handed on and let go (post_and_go).
// Its session starting with her not up starts her, unless the person quit
// her (data.rs mark_quit).
fn codex_hook(port: u16) {
    let Some(mut event) = read_event() else { return };
    if let Some(fields) = event.as_object_mut() {
        fields.insert(jump::CODEX_CHAIN.into(), jump::chain_json(&jump::hook_chain()));
    }
    let name = event["hook_event_name"].as_str().unwrap_or("").to_string();
    if name != "PermissionRequest" {
        if !post_and_go(port, "codex", &event) && name == "SessionStart" && data::may_start(&data::folder()) {
            start_detached();
        }
        return;
    }
    // A prompt waits for the person, just under the hook's own timeout.
    let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(connection::CODEX_PROMPT_TIMEOUT - 5))).build().new_agent();
    if let Ok(mut res) = agent.post(&format!("http://127.0.0.1:{port}/hook?from=wakuwaku&agent=codex")).send_json(&event) {
        let body = res.body_mut().read_to_string().unwrap_or_default();
        if serde_json::from_str::<Value>(&body).is_ok_and(|v| v.get("hookSpecificOutput").is_some()) {
            let mut out = std::io::stdout();
            let _ = out.write_all(body.as_bytes());
            let _ = out.flush();
        }
    }
}

// Antigravity's hook (connection::AGY_FLAG and the event's name, which its
// payload does not carry): its answer printed first, whatever comes after
// (a hook that fails or answers nothing breaks agy's turn: connection.rs),
// then the event handed to her port with its name and let go (post_and_go:
// agy waits on each hook, and her answer is nothing to it). Quit by the
// person, she is not up: nothing tried. Its session starting with her not
// up starts her, as Codex's does. Where the session runs only as it starts
// or a turn begins (kept till then; some 10 ms: most of a hook is its
// process starting, some 50 to 80 ms).
fn agy_hook(port: u16, name: &str) {
    let answer = if name == "Stop" { r#"{"decision":"allow"}"# } else { "{}" };
    let mut out = std::io::stdout();
    let _ = out.write_all(answer.as_bytes());
    let _ = out.flush();
    let Some(mut event) = read_event() else { return };
    if !data::may_start(&data::folder()) {
        return;
    }
    let turn = name == "SessionStart" || (name == "PreInvocation" && event.get("invocationNum").and_then(Value::as_u64) == Some(0));
    if let Some(fields) = event.as_object_mut() {
        fields.insert("hook_event_name".into(), json!(name));
        if turn {
            fields.insert(jump::CODEX_CHAIN.into(), jump::chain_json(&jump::hook_chain()));
        }
    }
    if !post_and_go(port, "agy", &event) && name == "SessionStart" {
        start_detached();
    }
}

fn main() {
    // Started from a Claude Code session's hook: not one of its children to
    // what she starts (run.rs strip_session_marks).
    run::strip_session_marks();
    // Her exe again, as her guard (crash.rs): it waits for her to end, and
    // sees whether she crashed.
    let args: Vec<String> = std::env::args().collect();
    if let Some((pid, data)) = crash::guard_args(&args) {
        crash::guard(pid, move |code, started| crash::after_exit(&data, pid, code, started));
        return;
    }
    let port: u16 = std::env::var("WAKUWAKU_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(47213);
    if std::env::args().any(|a| a == connection::ENSURE_FLAG || a == connection::LEGACY_ENSURE_FLAG) {
        return ensure_running(port);
    }
    if std::env::args().any(|a| a == connection::CODEX_FLAG) {
        return codex_hook(port);
    }
    if let Some(at) = args.iter().position(|a| a == connection::AGY_FLAG) {
        return agy_hook(port, args.get(at + 1).map_or("", String::as_str));
    }
    // A task from the phone, run out of her job (run.rs).
    if let Some(at) = args.iter().position(|a| a == run::FLAG) {
        return run::helper(args.get(at + 1).map_or("", String::as_str));
    }

    // In the job of whoever started her (a hook of an older her, a terminal,
    // an app's shell), which may end her as it closes: started again as if
    // opened from Explorer, once, and this copy goes (detach.rs). Not run
    // from the source tree (cargo run waits on this copy).
    if !args.iter().any(|a| a == detach::FLAG) && !data::from_source() && detach::should_leave() {
        let rest: Vec<&str> = args.iter().skip(1).map(String::as_str).chain([detach::FLAG]).collect();
        if let Ok(exe) = std::env::current_exe() {
            if detach::spawn(&exe, &rest) {
                return;
            }
        }
    }

    // Started by an update: the one it replaces waited for, so this one
    // gets the port, not just wakes that one.
    updates::wait_after_update(&args);

    let dir = data::folder();
    let _ = std::fs::create_dir_all(&dir);

    // One pet at a time: whoever holds the port is the pet. Started again
    // while she runs: she comes back into sight, and this copy leaves.
    let listener = match tiny_http::Server::http(("127.0.0.1", port)) {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("wakuwaku: port {port} is taken ({err}); another pet is up");
            let _ = ureq::post(&format!("http://127.0.0.1:{port}/come-home")).send_empty();
            return;
        }
    };

    // She is up: quit from her menu before, she is wanted again now (by
    // hand, at login, an update), and the sessions' hooks may start her
    // from here on.
    data::clear_quit(&dir);

    // Her crashes caught from here on, and a guard to see her end while
    // her reports can go and are not off: crash.rs.
    crash::install(&dir);
    if crash::wants_guard(&dir) {
        crash::spawn_guard(&dir);
    }

    // Tauri's async commands on two threads: its own runtime starts one for
    // each logical processor, nearly all of them idle all day (31 of 32,
    // #14). Kept for as long as she runs.
    let runtime = Box::leak(Box::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().expect("a runtime")));
    tauri::async_runtime::set(runtime.handle().clone());

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            pet_ready,
            pet_hover,
            pet_drag_start,
            pet_drag_end,
            pet_menu,
            pet_double_click,
            pet_panel,
            pet_answer,
            scout_close,
            duel_close,
            duel_deck_open,
            duel_deck_save,
            duel_deck_rescan,
            duel_fly,
            duel_scan,
            duel_watch,
            duel_deck_use,
            duel_deck_drop,
            duel_deck_new,
            duel_cards,
            duel_card_arts,
            duel_card_info,
            duel_crew_ask,
            duel_crew_typing,
            duel_deck_close,
            open_drops,
            duel_again,
            scout_game,
            scout_rows,
            scout_timeline,
            scout_review_start,
            scout_review,
            scout_opgg,
            scout_opgg_meta,
            scout_opgg_picks,
            pet_dismiss,
            pet_keyboard,
            pet_walk,
            pet_walk_stop,
            island_holding,
            island_plate,
            island_release,
            island_drop,
            debug_result,
            pet_log,
            session_jump,
            settings::settings_get,
            settings::settings_set,
            settings::settings_open,
            settings::settings_closed,
            settings::settings_show_pet,
            settings::settings_login,
            settings::settings_hooks,
            settings::settings_codex_hooks,
            settings::settings_agy_hooks,
            settings::settings_grok_hooks,
            settings::settings_fetch,
            settings::settings_gallery,
            settings::settings_open_site,
            settings::settings_update,
            settings::settings_crash,
            settings::settings_mobile,
        ])
        .on_menu_event(|app, event| tray::on_menu(&shared(app), event.id().as_ref()))
        .setup(move |app| {
            // On macOS she is an accessory, as a menu-bar app is: no Dock icon,
            // and the menu bar is never hers, even while her settings are open.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let mut settings = data::load(&dir);
            // The phone page's token and the ntfy topic: her own, made once.
            if mobile::fill_secrets(&mut settings) {
                data::save(&dir, &settings);
            }
            // Her pet gone: the first one there is.
            let found = data::pets(&dir);
            let id = settings.get("pet").and_then(Value::as_str).unwrap_or("").to_string();
            if !found.is_empty() && !found.iter().any(|p| p.id == id) {
                settings.insert("pet".into(), json!(found[0].id));
            }
            for folder in data::pet_dirs(&dir) {
                // Codex's pets are only read, never made a folder for.
                if folder != data::codex_pets() {
                    let _ = std::fs::create_dir_all(&folder);
                }
                let _ = app.asset_protocol_scope().allow_directory(&folder, true);
            }
            notify::register(&dir);

            let today = widgets::Today::load(&dir, &widgets::local_date());
            let sh = Arc::new(Shared {
                app: app.handle().clone(),
                is_logging: dir.join("debug.on").exists(),
                dir,
                port,
                settings: Mutex::new(settings),
                pet: Mutex::new(state::Pet::default()),
                pets: pet::Pets::default(),
                island: Mutex::new(island::Island::default()),
                dock: Mutex::new(dock::Dock::default()),
                asks: Mutex::new(asks::Asks::default()),
                widgets: Mutex::new(widgets::Widgets::default()),
                scripts: Mutex::new(scripts::Runner::default()),
                today: Mutex::new(today),
                screens: Mutex::new(screen::read(app.handle())),
                hidden: AtomicBool::new(false),
                by_fullscreen: AtomicBool::new(false),
                quiet: AtomicBool::new(false),
                greeted: AtomicBool::new(false),
                codex_seen: std::sync::atomic::AtomicU64::new(0),
                agy_seen: std::sync::atomic::AtomicU64::new(0),
                grok_seen: std::sync::atomic::AtomicU64::new(0),
                claude_seen: std::sync::atomic::AtomicU64::new(0),
                own: Mutex::new(Vec::new()),
                prev_focus: Mutex::new(0),
                push_pending: AtomicBool::new(false),
                shuffle_until: std::sync::atomic::AtomicU64::new(0),
                evals: Mutex::new((0, std::collections::HashMap::new())),
            });
            app.manage(sh.clone());
            sh.log(&format!("up on port {}, data in {}", sh.port, sh.dir.display()));
            sh.log(&format!("displays: {}", sh.screens().describe()));
            // The sessions from before she restarted, and Grok's open ones (keep.rs).
            keep::restore(&sh);

            // The pets out on the desktop: their windows (the others' are
            // made as they are let out).
            for agent in sh.out_agents() {
                pet::create(&sh, agent)?;
            }
            // An app full screen as she starts, known before her home first shows.
            if fullscreen::AVAILABLE && sh.flag("hideInFullscreen") {
                sh.by_fullscreen.store(fullscreen::check(&[], &sh.displays()) == Some(true), Ordering::SeqCst);
            }
            island::apply_visibility(&sh);
            dock::apply_visibility(&sh);
            tray::create(&sh)?;
            server::serve(sh.clone(), listener);
            // Phone access: the page listened for on Tailscale's address too (mobile.rs).
            mobile::start(&sh);
            // Her updates looked for (a downloaded exe only).
            updates::start(&sh);
            // Crashes since: their reports made, sent or asked about.
            crash::start(&sh);
            // A League of Legends game: what the pets show of it (lol.rs).
            lol::start(&sh);
            // The game's ten, on a key (scout.rs).
            scout::listen_key(&sh);

            // The mouse: click-through, her eyes, drags; for both windows.
            // Quiet (nobody touching it), looked at once a second.
            let pointer = sh.clone();
            spawn_named("pointer", move || loop {
                // Each window's pointer in its own display's scale on a Mac
                // (screen::cursor); elsewhere the one pointer, read once.
                #[cfg(target_os = "macos")]
                let at = (screen::cursor(&pointer, pet::sf(&pointer, "claude")), screen::cursor(&pointer, island::origin(&pointer).1));
                #[cfg(not(target_os = "macos"))]
                let at = {
                    let c = screen::cursor(&pointer, 1.0);
                    (c, c)
                };
                let wait = match at {
                    (Some(her), Some(home)) => data::AGENTS.iter().map(|agent| pet::poll(&pointer, agent, her)).fold(island::poll(&pointer, home).min(dock::poll(&pointer, home)), u64::min),
                    _ => 200,
                };
                let wait = if pointer.check_quiet() { wait.max(1000) } else { wait };
                std::thread::sleep(Duration::from_millis(wait));
            });

            // The widgets: today's from the start; the tokens' each minute
            // while they are on; a script's gone once it stops sending.
            sh.count_today(|_| {});
            let watcher = sh.clone();
            spawn_named("widgets", move || {
                power::thread_eco();
                let mut tokens = tokens::Tokens::default();
                let (claude_dir, codex_dir, agy_dirs) = (tokens::claude_dir(), tokens::codex_dir(), tokens::agy_dirs());
                let mut tokens_at: Option<std::time::Instant> = None;
                loop {
                    let now = now_ms();
                    let mut changed = watcher.widgets.lock().unwrap().expire(now);
                    // A built-in widget: its words while it is on, gone when off.
                    let mut built_in = |id: &str, words: Option<Value>| {
                        let mut widgets = watcher.widgets.lock().unwrap();
                        changed |= match words {
                            Some(value) => widgets.set_built_in(id, value, now),
                            None => widgets.remove_built_in(id),
                        };
                    };
                    // Just turned on: read now, not at its next turn.
                    let unread = |id: &str| watcher.is_widget_wanted(id) && watcher.widgets.lock().unwrap().label_of(id).is_none();
                    if tokens_at.is_none_or(|t| t.elapsed() >= tokens::EVERY) || unread("tokens") {
                        tokens_at = Some(std::time::Instant::now());
                        let count = watcher.is_widget_wanted("tokens").then(|| {
                            tokens.update(&claude_dir, &codex_dir, &agy_dirs, widgets::local_midnight());
                            tokens::words(tokens.count())
                        });
                        built_in("tokens", count);
                    }
                    // A new day: today's counts start again.
                    if watcher.today.lock().unwrap().date != widgets::local_date() {
                        watcher.count_today(|_| {});
                    }
                    // The plugins she runs: started, stopped, started again.
                    changed |= scripts::sync(&watcher);
                    if changed {
                        watcher.redraw_widgets();
                    }
                    // Quiet (nobody at the keyboard): every ten seconds.
                    std::thread::sleep(Duration::from_secs(if watcher.is_quiet() { 10 } else { 3 }));
                }
            });

            // The monitor: the parts that are on, every so many seconds (its
            // setting, 1 to 10), while it is on; read again at once when it
            // is turned on, its pace changes or the quiet does (woken by
            // Shared::change and check_quiet: it looked four times a second
            // whether they had, #14).
            let monitored = sh.clone();
            spawn_named("monitor", move || {
                power::thread_eco();
                let mut monitor = widgets::Monitor::new();
                let mut next = std::time::Instant::now();
                let mut was = (false, 0, false);
                loop {
                    let watching = widgets::Watching::from(&monitored.setting("monitor"));
                    let (on, quiet) = (monitored.is_widget_on("monitor"), monitored.is_quiet());
                    if std::time::Instant::now() >= next || (on, watching.every, quiet) != was {
                        was = (on, watching.every, quiet);
                        // Quiet (nobody looking), every ten seconds at least.
                        let every = if quiet { watching.every.max(10) } else { watching.every };
                        next = std::time::Instant::now() + Duration::from_secs(every);
                        let words = if on { monitor.read(&watching) } else { None };
                        let changed = {
                            let mut widgets = monitored.widgets.lock().unwrap();
                            match words {
                                Some(value) => widgets.set_built_in("monitor", value, now_ms()),
                                None => widgets.remove_built_in("monitor"),
                            }
                        };
                        if changed {
                            monitored.redraw_widgets();
                        }
                    }
                    // Off, till it is turned on.
                    let wait = if on { next.saturating_duration_since(std::time::Instant::now()) } else { Duration::from_secs(3600) };
                    let (woken, cv) = &MONITOR_WAKE;
                    let guard = woken.lock().unwrap();
                    let (mut guard, _) = cv.wait_timeout_while(guard, wait, |w| !*w).unwrap();
                    *guard = false;
                }
            });

            // Another app full screen, looked at on this thread alone
            // (Shared::look_fullscreen).
            let watcher = sh.clone();
            spawn_named("fullscreen", move || loop {
                std::thread::sleep(Duration::from_millis(1500));
                watcher.look_fullscreen();
                island::look_over_max(&watcher);
            });

            // Endings held for a while; prompts out of time; sessions gone
            // quiet, out of sight and forgotten; the windows on screen.
            let ticker = sh.clone();
            spawn_named("ticker", move || {
                for n in 1u64.. {
                    std::thread::sleep(Duration::from_millis(500));
                    let now = now_ms();
                    let hold = ticker.hold();
                    if n % 2 == 0 {
                        ticker.check_quiet();
                        let hide = ticker.idle_hide_ms();
                        let mut changed = ticker.pet.lock().unwrap().tick(now, hold);
                        if n % 120 == 0 {
                            // A Grok turn still under way by its own log is not stale (keep.rs).
                            keep::keep_grok_busy(&ticker);
                            changed |= ticker.pet.lock().unwrap().check_stale(now, hold);
                            if mobile::phone_gone(&ticker) {
                                ticker.redraw_widgets();
                            }
                        }
                        changed |= ticker.pet.lock().unwrap().hide_quiet(now, hide);
                        if changed {
                            ticker.redraw();
                        }
                        // Each one handed over on its own line, as an answer
                        // is: a prompt let wait its whole time said nothing
                        // (the test session, 2026-10-13).
                        let gone = ticker.asks.lock().unwrap().expire(now);
                        if !gone.is_empty() {
                            for id in gone {
                                ticker.log(&format!("prompt {id} handed to the terminal: its time is up"));
                            }
                            ticker.push_asks();
                        }
                    }
                    // Her sessions to disk every 10 s, when they changed (keep.rs).
                    if n % 20 == 0 {
                        keep::save(&ticker);
                    }
                    if n % 4 == 0 {
                        let screens = screen::read(&ticker.app);
                        *ticker.screens.lock().unwrap() = screens;
                        for agent in data::AGENTS {
                            pet::keep_on_screen(&ticker, agent);
                        }
                        island::keep_on_screen(&ticker);
                        dock::keep_on_screen(&ticker);
                    }
                }
            });

            // No pet at all yet: the default one (Claude小姐), from the exe
            // (fetch.rs seed_default_pet); written nowhere (the disk full,
            // the folder not to be written), downloaded; that failing too,
            // the settings, to pick one later.
            if found.is_empty() {
                let first = sh.clone();
                std::thread::spawn(move || {
                    let pets = first.dir.join("pets");
                    let got = match fetch::seed_default_pet(&pets) {
                        Ok(got) => Ok(got),
                        Err(e) => {
                            first.log(&format!("default pet not written ({e}): downloading it"));
                            fetch::download_pet(fetch::DEFAULT_PET, &pets).map_err(|e| fetch::describe("en", &e))
                        }
                    };
                    match got {
                        Ok(got) => {
                            first.log(&format!("default pet in place: {}", got["id"]));
                            first.change(json!({ "pet": got["id"] }));
                        }
                        Err(e) => {
                            first.log(&format!("default pet: {e}"));
                            island::open_settings(&first, Some("pets"));
                        }
                    }
                });
            }
            // Never set up: the settings, where Claude Code or Codex gets
            // connected, once (welcomeOpened). Every start until one was
            // connected was too much for one who keeps her as a pet only,
            // with no way to stop it (the user, 2026-10-13); nor once "not
            // now" was said (connectHintOff, the welcome's or the island's).
            if !connection::look_connected(port) && !sh.flag("onboarded") && !sh.flag("welcomeOpened") && !sh.flag("connectHintOff") {
                sh.change(json!({ "welcomeOpened": true }));
                island::open_settings(&sh, Some("connect"));
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while running wakuwaku")
        // Quitting: the plugins she ran stopped.
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                let sh = shared(app);
                sh.log("quitting: the plugins stopped");
                scripts::stop_all(&sh);
                sh.log("quitting: done");
            }
        });
}

// This process ended at once, whatever its threads are doing.
// Quitting once: a second quit (the tray, an update) while the first hands over does nothing.
static QUITTING: AtomicBool = AtomicBool::new(false);

fn end_now() -> ! {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetCurrentProcess() -> isize;
            fn TerminateProcess(process: isize, code: u32) -> i32;
        }
        // SAFETY: this process's own pseudo-handle.
        unsafe { TerminateProcess(GetCurrentProcess(), 0) };
    }
    std::process::exit(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Handed on and let go while her port is held but not yet answered (her
    // first second or two): the event waits in its queue, whole, and is read
    // once she answers; then as she answers at once.
    #[test]
    fn an_event_let_go_is_read_whole_later() {
        let server = tiny_http::Server::http(("127.0.0.1", 0)).unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let event = json!({ "hook_event_name": "Stop", "conversationId": "c", "text": "x".repeat(20_000) });
        let t = std::time::Instant::now();
        assert!(post_and_go(port, "agy", &event));
        assert!(t.elapsed() < Duration::from_millis(200), "let go at once");
        std::thread::sleep(Duration::from_millis(300));
        let mut req = server.recv_timeout(Duration::from_secs(2)).unwrap().expect("a request");
        assert_eq!(req.url(), "/hook?from=wakuwaku&agent=agy");
        let mut body = String::new();
        std::io::Read::read_to_string(req.as_reader(), &mut body).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&body).unwrap(), event);
        // Her answer goes nowhere, harmlessly.
        let _ = req.respond(tiny_http::Response::from_string("{}"));
        // No one there: nothing tried.
        drop(server);
        assert!(!post_and_go(port, "agy", &event));
    }
}
