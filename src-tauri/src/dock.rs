// The side island (the dock setting): a column at the right edge of the
// island's display, one row for each agent, its pet's head and a bubble of
// what it is doing; a click on a row opens it there (src/dock/). Beside the
// island at the top, not instead of it.
//
// Its window is a fixed size and lets clicks through but on what the page
// draws (pointer.rs), as the island's does.
use std::sync::Arc;

use serde_json::{json, Value};
use tauri::utils::config::BackgroundThrottlingPolicy;
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::pointer::{ClickThrough, Mode};
use crate::{data, now_ms, Shared};

pub const LABEL: &str = "dock";

// The window, logical: as wide as a bubble and its head, as tall as four
// rows with one of them open. Kept inside the display's work area.
const SIZE: (f64, f64) = (380.0, 640.0);

#[derive(Default)]
pub struct Dock {
    // Physical.
    pos: (i32, i32),
    size: (i32, i32),
    sf: f64,
    created: bool,
    ready: bool,
    shown: bool,
    ct: ClickThrough,
}

fn window(sh: &Shared) -> Option<WebviewWindow> {
    sh.app.get_webview_window(LABEL)
}

// Wanted: the setting on, and she in sight (not hidden, do not disturb, full screen).
fn is_wanted(sh: &Shared) -> bool {
    sh.flag("dock") && sh.is_visible()
}

// Where it goes on the island's display: the right edge, the middle. Returns
// the physical rect if it moved.
fn fit(sh: &Shared) -> Option<((i32, i32), (i32, i32), f64)> {
    let area = crate::island::area(sh)?;
    let sf = area.sf;
    let w = (SIZE.0 * sf).round() as i32;
    let h = ((SIZE.1 * sf).round() as i32).min(area.h);
    let pos = (area.x + area.w - w, area.y + (area.h - h) / 2);
    let mut dock = sh.dock.lock().unwrap();
    if (pos, (w, h), sf) == (dock.pos, dock.size, dock.sf) {
        return None;
    }
    dock.pos = pos;
    dock.size = (w, h);
    dock.sf = sf;
    Some((pos, (w, h), sf))
}

fn lay(win: &WebviewWindow, (x, y): (i32, i32), (w, h): (i32, i32)) {
    let _ = win.set_position(PhysicalPosition::new(x, y));
    let _ = win.set_size(PhysicalSize::new(w.max(1) as u32, h.max(1) as u32));
    let _ = win.set_position(PhysicalPosition::new(x, y));
}

fn place(sh: &Shared) {
    if let (Some((pos, size, _)), Some(win)) = (fit(sh), window(sh)) {
        lay(&win, pos, size);
    }
}

fn create(sh: &Arc<Shared>) {
    fit(sh);
    let (pos, size, sf) = {
        let mut dock = sh.dock.lock().unwrap();
        if dock.created {
            return;
        }
        dock.created = true;
        dock.ct.force(Mode::Pass);
        (dock.pos, dock.size, if dock.sf > 0.0 { dock.sf } else { 1.0 })
    };
    // Off the main thread, as the island's (a menu click may ask for it).
    let sh = sh.clone();
    std::thread::spawn(move || {
        let built = WebviewWindowBuilder::new(&sh.app, LABEL, WebviewUrl::App("dock/index.html".into()))
            .title("wakuwaku dock")
            .inner_size(size.0 as f64 / sf, size.1 as f64 / sf)
            .additional_browser_args(crate::browser_args(&sh))
            .data_directory(crate::webview_dir(&sh))
            .transparent(true)
            .decorations(false)
            .resizable(false)
            .maximizable(false)
            .minimizable(false)
            .skip_taskbar(true)
            .shadow(false)
            .always_on_top(true)
            .visible_on_all_workspaces(true)
            .focusable(false)
            .focused(false)
            .visible(false)
            .background_throttling(BackgroundThrottlingPolicy::Disabled)
            .build();
        match built {
            Ok(win) => {
                lay(&win, pos, size);
                let _ = win.set_ignore_cursor_events(true);
                sh.own_window(&win);
                crate::crash::watch(&sh, &win);
            }
            Err(err) => sh.log(&format!("dock: could not make its window: {err}")),
        }
    });
}

// Its page has its listeners up.
pub fn ready(sh: &Arc<Shared>) {
    sh.log("dock: page ready");
    sh.dock.lock().unwrap().ready = true;
    push(sh);
    apply_visibility(sh);
}

// Up or away, as the setting and her being in sight say. Made the first
// time it is wanted, then kept hidden while not.
pub fn apply_visibility(sh: &Arc<Shared>) {
    let want = is_wanted(sh);
    let (created, ready, shown) = {
        let dock = sh.dock.lock().unwrap();
        (dock.created, dock.ready, dock.shown)
    };
    if !created {
        if want {
            create(sh);
        }
        return;
    }
    if !ready {
        return;
    }
    if want {
        place(sh);
    }
    if want == shown {
        return;
    }
    let ignore = {
        let mut dock = sh.dock.lock().unwrap();
        dock.shown = want;
        dock.ct.reset(!want)
    };
    if let Some(win) = window(sh) {
        if let Some(ignore) = ignore {
            let _ = win.set_ignore_cursor_events(ignore);
        }
        let _ = if want { win.show() } else { win.hide() };
    }
    if want {
        push(sh);
    }
}

// Kept on the island's display (a display come or gone, the island moved).
pub fn keep_on_screen(sh: &Shared) {
    if sh.dock.lock().unwrap().shown {
        place(sh);
    }
}

fn is_up(sh: &Shared) -> bool {
    let dock = sh.dock.lock().unwrap();
    dock.ready && dock.shown
}

// What its page draws: every agent's row, with what its pet is drawn from.
pub fn payload(sh: &Shared) -> Value {
    let settings = sh.settings.lock().unwrap().clone();
    let found = data::pets(&sh.dir);
    let rows: Vec<Value> = {
        let pet = sh.pet.lock().unwrap();
        data::AGENTS
            .iter()
            .map(|&agent| {
                let sprite = data::pet_of(&settings, Some(agent), &found);
                json!({
                    "agent": agent,
                    "now": pet.get_for(agent, now_ms()),
                    "list": pet.list_for(agent),
                    "sprite": sprite.map(|p| data::asset_url(&p.sheet)),
                    "spriteVersion": sprite.map_or(2, |p| p.version),
                    "color": sprite.and_then(|p| p.color.clone()).unwrap_or_else(|| data::agent_color(agent).to_string()),
                })
            })
            .collect()
    };
    json!({
        "rows": rows,
        "lang": sh.lang(),
        "details": settings.get("details").and_then(Value::as_bool).unwrap_or(true),
        "quiet": sh.is_quiet(),
    })
}

// The rows drawn anew (Shared::redraw), while it is up.
pub fn push(sh: &Shared) {
    if is_up(sh) {
        let _ = sh.app.emit_to(LABEL, "dock:update", payload(sh));
    }
}

// The prompts waiting: every one, whichever page answers it (by its id).
pub fn push_asks(sh: &Shared) {
    if sh.dock.lock().unwrap().ready {
        let views = sh.asks.lock().unwrap().views_where(|_| true);
        let _ = sh.app.emit_to(LABEL, "pet:asks", views);
    }
}

// One look at the mouse. Returns ms until the next look.
pub fn poll(sh: &Shared, cursor: (i32, i32)) -> u64 {
    let (ignore, pointer, wait) = {
        let mut dock = sh.dock.lock().unwrap();
        if !dock.ready || !dock.shown {
            return 1000;
        }
        let (pos, size, sf) = (dock.pos, dock.size, if dock.sf > 0.0 { dock.sf } else { 1.0 });
        let seen = dock.ct.look(cursor, pos, size, sf, false, false);
        let wait = if dock.ct.over { 50 } else if seen.forward { 30 } else if seen.near { 80 } else { 200 };
        (seen.ignore, seen.pointer, wait)
    };
    if let (Some(ignore), Some(win)) = (ignore, window(sh)) {
        let _ = win.set_ignore_cursor_events(ignore);
    }
    if let Some(at) = pointer {
        let _ = sh.app.emit_to(LABEL, "pet:pointer", at);
    }
    wait
}

pub fn hover(sh: &Shared, is_over: bool) {
    let ignore = sh.dock.lock().unwrap().ct.set_over(is_over, false, false);
    if let (Some(ignore), Some(win)) = (ignore, window(sh)) {
        let _ = win.set_ignore_cursor_events(ignore);
    }
}
