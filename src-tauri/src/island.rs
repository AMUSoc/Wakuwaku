// Her home's window (the page draws it, src/pet/island.js): the island, a
// black pill at the top centre. It stays up while she is out, and does the
// talking.
//
// Pulled out of her home, she becomes the pet window (carried under the
// cursor while the button is still held on the home); brought close to it
// again, it reaches for her and takes her back.
//
// Sizes here are logical (the page's) unless they say physical.
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;

use serde_json::{json, Value};
use tauri::utils::config::BackgroundThrottlingPolicy;
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

use crate::pointer::{ClickThrough, Mode};
use crate::screen::Area;
use crate::Shared;

// The window without extra room (island.js BASE): the island as
// wide as pulling her out needs, so it only ever grows down. A window whose
// top-left corner moves shows its old picture from there for a frame or two,
// so these never move it. The page asks for more (pet:panel) when a shape
// needs it.
const ISLAND: (f64, f64) = (760.0, 132.0);
// The island's top edge in the window, and its compact height.
const TOP: f64 = 8.0;
const COMPACT_H: f64 = 36.0;
// Her middle this close to her home: it reaches out for her; this close,
// letting go puts her back in.
const REACH_PX: f64 = 280.0;
const SNAP_PX: f64 = 140.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Reach {
    Far,
    Near,
    Snap,
}

#[derive(Default)]
pub struct Island {
    // Physical.
    pos: (i32, i32),
    size: (i32, i32),
    sf: f64,
    // Extra room the page asked for.
    room: Option<(f64, f64)>,
    created: bool,
    ready: bool,
    shown: bool,
    ct: ClickThrough,
    // The window, once made.
    hwnd: isize,
    // The button held on her (a pull, or carrying her out): the window keeps the pointer.
    holding: bool,
    reaching: bool,
    // Taking her in: the window stays up for it.
    pub absorbing: bool,
    // Risen only for the settings (she is out of sight): it goes again once
    // they close.
    pub temp: bool,
    pub settings_open: bool,
    // Settings asked for before its page was up: the page they open on.
    pending: Option<String>,
}

impl Island {
    fn sf(&self) -> f64 {
        if self.sf > 0.0 { self.sf } else { 1.0 }
    }

    // Physical: the island's, grown by the room the page asked for.
    fn size(&self) -> (i32, i32) {
        let sf = self.sf();
        let (w, h) = match self.room {
            Some((w, h)) => (w.max(ISLAND.0), h.max(ISLAND.1)),
            None => ISLAND,
        };
        ((w * sf).round() as i32, (h * sf).round() as i32)
    }

}

fn window(sh: &Shared) -> Option<WebviewWindow> {
    sh.app.get_webview_window("island")
}

// Up whenever she is in sight, home or out; or risen for the settings; or
// the settings open in it, whatever they just hid (she, do not disturb):
// they never go from under the person, only once closed.
pub fn is_shown(sh: &Shared) -> bool {
    let up = sh.is_visible();
    let isl = sh.island.lock().unwrap();
    up || isl.temp || isl.settings_open
}

// Its window's handle, once made (0 before).
pub fn hwnd(sh: &Shared) -> isize {
    sh.island.lock().unwrap().hwnd
}

// Where its window is and its size (physical), and its scale.
pub fn frame(sh: &Shared) -> ((i32, i32), (i32, i32), f64) {
    let isl = sh.island.lock().unwrap();
    (isl.pos, isl.size, isl.sf())
}

fn set_ignore(sh: &Shared, ignore: Option<bool>) {
    if let (Some(ignore), Some(win)) = (ignore, window(sh)) {
        sh.log(&format!("island: mouse {}", if ignore { "passes through" } else { "caught" }));
        let _ = win.set_ignore_cursor_events(ignore);
    }
}

// The display it is on: the one chosen for it (islandDisplay, the top-left
// corner of a display), while there is such a one; else where it is, else
// the primary one.
pub(crate) fn area(sh: &Shared) -> Option<Area> {
    let screens = sh.screens();
    if let Some(chosen) = chosen(sh).and_then(|at| screens.areas.iter().copied().find(|a| (a.mon.0, a.mon.1) == at)) {
        return Some(chosen);
    }
    let at = {
        let isl = sh.island.lock().unwrap();
        (isl.created && isl.size.0 > 0).then(|| (isl.pos.0 + isl.size.0 / 2, isl.pos.1 + isl.size.1 / 2))
    };
    match at {
        Some((x, y)) => screens.near(x, y),
        None => screens.primary,
    }
}

// The display chosen for it (the menu's Display): its top-left corner.
fn chosen(sh: &Shared) -> Option<(i32, i32)> {
    let v = sh.setting("islandDisplay");
    Some((v.get(0)?.as_i64()? as i32, v.get(1)?.as_i64()? as i32))
}

// Moved to the display now chosen for it, with the blur under it.
pub fn moved(sh: &Shared) {
    let shown = sh.island.lock().unwrap().shown;
    place(sh);
    if shown {
        lay_again(sh, "moved to another display");
        crate::backplate::again();
    }
    // The side island goes with it.
    crate::dock::keep_on_screen(sh);
}

// The display it is on (x, y, w, h), as area finds it.
pub fn display(sh: &Shared) -> Option<(i32, i32, i32, i32)> {
    area(sh).map(|a| a.mon)
}

// Where the window goes. Returns the physical rect if it moved.
fn fit(sh: &Shared) -> Option<((i32, i32), (i32, i32))> {
    let area = area(sh)?;
    let mut isl = sh.island.lock().unwrap();
    isl.sf = area.sf;
    let size = isl.size();
    // On a Mac the notch's place, over the menu bar (notch.rs); elsewhere
    // the top centre, below a taskbar that sits at the top.
    let pos = crate::notch::at_top(area.mon, size.0).unwrap_or((area.x + (area.w - size.0) / 2, area.y));
    if (pos, size) == (isl.pos, isl.size) {
        return None;
    }
    isl.pos = pos;
    isl.size = size;
    Some((pos, size))
}

// The window laid on its rect (physical): its place first, then its size,
// then its place again. Sized first and then moved to
// a display of another scale, Windows gave it the size for that one (the
// scales' ratio over it): an external display at 125% plugged back in and
// made the main one while it sat on the laptop's at 175%, her taskbar came
// out 1830 x 615 for 2560 x 860, the strip floating up the screen and short
// of its end, until she was restarted (zhoupengjie/Wakuwaku#37).
fn lay(win: &WebviewWindow, (x, y): (i32, i32), (w, h): (i32, i32)) {
    let _ = win.set_position(PhysicalPosition::new(x, y));
    let _ = win.set_size(PhysicalSize::new(w.max(1) as u32, h.max(1) as u32));
    let _ = win.set_position(PhysicalPosition::new(x, y));
}

// Laid again where it is meant to be (a change of scale under it).
fn lay_again(sh: &Shared, why: &str) {
    let (pos, size) = {
        let isl = sh.island.lock().unwrap();
        (isl.pos, isl.size)
    };
    // The display it is meant for: the one under the island's middle.
    let areas = sh.screens().areas;
    let (x, y) = (pos.0 + size.0 / 2, pos.1 + size.1 / 2);
    let Some(meant) = areas.iter().find(|a| x >= a.mon.0 && y >= a.mon.1 && x < a.mon.0 + a.mon.2 && y < a.mon.1 + a.mon.3) else { return };
    let Some(win) = window(sh) else { return };
    // Laid again only once it has that display's scale: of another (Windows
    // moving it off a display unplugged, the list of displays not yet
    // without it), its place is still to come, and laid now it went back
    // on the display going away for a moment.
    if win.scale_factor().is_ok_and(|sf| (sf - meant.sf).abs() > 0.01) {
        return;
    }
    let now = win.outer_size().map(|s| (s.width as i32, s.height as i32)).unwrap_or(size);
    if now != size {
        sh.log(&format!("island: sized by Windows to {}x{} ({why}); laid again at {pos:?} {size:?} on {:?} at {}", now.0, now.1, meant.mon, meant.sf));
        lay(&win, pos, size);
    }
}

fn place(sh: &Shared) {
    let Some((pos, size)) = fit(sh) else { return };
    if let Some(win) = window(sh) {
        lay(&win, pos, size);
    }
    // On another display, another notch (or none).
    crate::notch::settle(sh);
}

// Made the first time her home shows, then kept (hidden while she talks
// herself), so coming back is instant.
fn create(sh: &Arc<Shared>) {
    fit(sh);
    let (pos, size, sf) = {
        let mut isl = sh.island.lock().unwrap();
        if isl.created {
            return;
        }
        isl.created = true;
        isl.ct.force(Mode::Pass);
        (isl.pos, isl.size, isl.sf())
    };
    // Built off the main thread: a menu click (main thread) may be what asks
    // for it, and building there waits on itself.
    let sh = sh.clone();
    std::thread::spawn(move || {
        let built = WebviewWindowBuilder::new(&sh.app, "island", WebviewUrl::App("pet/index.html?role=island".into()))
            .title("wakuwaku island")
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
                crate::notch::settle(&sh);
                // A click anywhere else takes the focus from the settings: they close.
                let app = sh.app.clone();
                win.on_window_event(move |event| {
                    // Not when it is her own window being shown (Shared::show_window).
                    if let WindowEvent::Focused(false) = event {
                        if !crate::shared(&app).is_shuffling() {
                            let _ = app.emit_to("island", "island:blur", ());
                        }
                    }
                    // Its display's scale changed under it (a display plugged
                    // in or out, another made the main one): Windows sized it
                    // for the new scale; once it has, put where it belongs on
                    // the displays there are now, at its size (#37), not at
                    // the next look.
                    if let WindowEvent::ScaleFactorChanged { .. } = event {
                        let app = app.clone();
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            keep_on_screen(&crate::shared(&app));
                        });
                    }
                });
                #[cfg(windows)]
                if let Ok(hwnd) = win.hwnd() {
                    sh.island.lock().unwrap().hwnd = hwnd.0 as isize;
                    os::no_transitions(hwnd.0 as isize);
                }
                sh.own_window(&win);
                // Its renderer gone: told, and the page loaded again (crash.rs).
                crate::crash::watch(&sh, &win);
            }
            Err(err) => sh.log(&format!("island: could not make its window: {err}")),
        }
    });
}

// At the top of the screen, a see-through look (islandTopLook) that
// Windows' acrylic does not blur (none set under 90 %, or it cannot be:
// backplate.rs) deepens over a maximized window on her display (island.js
// setDeep, zhoupengjie/Wakuwaku#27): looked at with the full screen's look
// (every 1.5 s, or a window to the front), told the page as it changes
// (island:over-max). Windows' mode with it: the page has it in pet:update
// alone, so a change of it draws anew.
// Nothing for the default black look, nor while she is quiet.
static OVER_MAX: AtomicBool = AtomicBool::new(false);
static TOP_LIGHT: AtomicU8 = AtomicU8::new(2);

pub fn look_over_max(sh: &Arc<Shared>) {
    let looks = sh.setting("islandTopLook");
    if sh.is_quiet() || !looks.as_object().is_some_and(|m| m.values().any(Value::is_object)) {
        return;
    }
    let light = crate::wallpaper::look().0;
    if TOP_LIGHT.swap(light as u8, Ordering::SeqCst) != light as u8 {
        sh.redraw();
    }
    let look = &looks[if light { "light" } else { "dark" }];
    let deepens = look["bg"].is_string() && look["op"].as_u64().is_some_and(|op| op < 90) && (look["blur"].as_u64().unwrap_or(0) == 0 || !crate::backplate::available());
    let on = deepens && display(sh).is_some_and(os::maximized_on);
    if OVER_MAX.swap(on, Ordering::SeqCst) != on {
        let _ = sh.app.emit_to("island", "island:over-max", json!({ "on": on }));
    }
}

// Its page has its listeners up: draw, show, say hello.
pub fn ready(sh: &Arc<Shared>) {
    sh.log("island: page ready");
    // A new page knows of no maximized window yet (look_over_max).
    OVER_MAX.store(false, Ordering::SeqCst);
    let pending = {
        let mut isl = sh.island.lock().unwrap();
        isl.ready = true;
        isl.pending.take()
    };
    sh.redraw();
    // The band at a Mac's screen top, now that the page hears it.
    crate::notch::settle(sh);
    sh.greet();
    apply_visibility(sh);
    if let Some(tab) = pending {
        let _ = sh.app.emit_to("island", "island:settings", json!({ "tab": tab }));
    }
}

pub fn apply_visibility(sh: &Arc<Shared>) {
    let want = is_shown(sh);
    let (created, ready, shown) = {
        let isl = sh.island.lock().unwrap();
        (isl.created, isl.ready, isl.shown)
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
    {
        let mut isl = sh.island.lock().unwrap();
        isl.shown = want;
        if !want {
            isl.holding = false;
            isl.reaching = false;
        }
    }
    if let Some(win) = window(sh) {
        let _ = if want { win.show() } else { win.hide() };
    }
    // The blur under the island goes with it, and comes back with it (backplate.rs).
    if want {
        crate::backplate::again();
    } else {
        crate::backplate::hide();
    }
}

// Kept in its place (a display gone, the scale changed).
pub fn keep_on_screen(sh: &Shared) {
    let is_up = {
        let isl = sh.island.lock().unwrap();
        isl.shown && !isl.holding && !isl.reaching
    };
    if !is_up {
        return;
    }
    // Where it belongs now (a display come or gone moves it), then its size
    // as Windows left it: place compares only with what it asked for last,
    // the same, and would never set it again (#37).
    place(sh);
    lay_again(sh, "seen by the look");
}

// One look at the mouse. Returns ms until the next look.
pub fn poll(sh: &Shared, cursor: (i32, i32)) -> u64 {
    // She is being dragged past: the home is not being hovered.
    let passive = crate::pet::is_dragged(sh);
    let (ignore, pointer, wait) = {
        let mut isl = sh.island.lock().unwrap();
        if !isl.ready {
            return 500;
        }
        if !isl.shown {
            (isl.ct.force(Mode::Pass), None, 500)
        } else {
            let (pos, size, sf, held) = (isl.pos, isl.size, isl.sf(), isl.holding);
            let seen = isl.ct.look(cursor, pos, size, sf, held, passive && !held);
            if isl.ct.calmed() {
                sh.log(&format!("island: mouse mode bouncing, calmed: cursor {cursor:?}, window {pos:?} {size:?}, scale {sf}"));
            }
            let wait = if held || isl.ct.over { 50 } else if seen.forward { 30 } else if seen.near { 80 } else { 200 };
            (seen.ignore, seen.pointer, wait)
        }
    };
    set_ignore(sh, ignore);
    if let Some(at) = pointer {
        let _ = sh.app.emit_to("island", "pet:pointer", at);
    }
    wait
}

pub fn hover(sh: &Shared, is_over: bool) {
    let ignore = {
        let mut isl = sh.island.lock().unwrap();
        let held = isl.holding;
        isl.ct.set_over(is_over, held, false)
    };
    set_ignore(sh, ignore);
    // The home opens up to say what happened while hovered, so it counts
    // as seen once the pointer leaves.
    if !is_over && sh.pet.lock().unwrap().seen(crate::now_ms(), sh.hold()) {
        sh.redraw();
    }
}

// The room a shape needs, as the page measured it.
pub fn set_room(sh: &Shared, measured: &Value) {
    let room = match (measured["width"].as_f64(), measured["height"].as_f64()) {
        (Some(w), Some(h)) if w > 0.0 && h > 0.0 => Some((w.ceil(), h.ceil())),
        _ => None,
    };
    sh.island.lock().unwrap().room = room;
    place(sh);
}

// The button is held on her in her home (a pull, or carrying her out).
pub fn set_holding(sh: &Shared, is_holding: bool) {
    sh.log(&format!("island: holding {is_holding}"));
    let ignore = {
        let mut isl = sh.island.lock().unwrap();
        isl.holding = is_holding;
        if is_holding { isl.ct.force(Mode::Catch) } else { None }
    };
    set_ignore(sh, ignore);
}

// Where her home is on the screen, physical, whether it is up or not: what
// she is reached for from and taken back into: the middle of the island's
// lower edge (compact).
pub fn seat(sh: &Shared) -> Option<(i32, i32)> {
    let area = area(sh)?;
    let px = |v: f64| (v * area.sf).round() as i32;
    Some((area.x + area.w / 2, area.y + px(TOP + COMPACT_H)))
}

// Where she flies to before her home takes her in: just under the island,
// where the drop can take her.
pub fn landing(sh: &Shared) -> Option<(i32, i32)> {
    let seat = seat(sh)?;
    let sf = area(sh).map_or(1.0, |a| a.sf);
    Some((seat.0, seat.1 + (70.0 * sf) as i32))
}

// She is being moved near her home (point: her middle on the screen,
// physical), or no longer (None). Tells the page, which reaches out a drop
// for her, and says how close she is (Snap: let go, and she is back).
pub fn reach(sh: &Arc<Shared>, point: Option<(i32, i32)>, is_final: bool) -> Reach {
    let anchor = seat(sh);
    let (event, state, changed) = {
        let mut isl = sh.island.lock().unwrap();
        let sf = isl.sf();
        let was = isl.reaching;
        let state = match (point, anchor) {
            (Some(p), Some(a)) => {
                let distance = ((p.0 - a.0) as f64).hypot((p.1 - a.1) as f64) / sf;
                if distance < SNAP_PX { Reach::Snap } else if distance < REACH_PX { Reach::Near } else { Reach::Far }
            }
            _ => Reach::Far,
        };
        let event = if anchor.is_none() {
            None
        } else if state == Reach::Far {
            std::mem::take(&mut isl.reaching).then_some(Value::Null)
        } else if is_final && state == Reach::Snap {
            // Letting go close enough: the page takes over with absorb.
            None
        } else {
            isl.reaching = true;
            let p = point.unwrap();
            // Screen coordinates as the page has them (window.screenX): logical.
            Some(json!({ "x": p.0 as f64 / sf, "y": p.1 as f64 / sf, "snap": state == Reach::Snap }))
        };
        (event, state, was != isl.reaching)
    };
    if changed {
        apply_visibility(sh);
    }
    if let Some(at) = event {
        let _ = sh.app.emit_to("island", "island:reach", at);
    }
    state
}

// She has landed (or come home): whatever the home's page heard of the
// button, it is no longer held there.
pub fn end_hold(sh: &Shared) {
    let was = std::mem::take(&mut sh.island.lock().unwrap().holding);
    if was {
        let _ = sh.app.emit_to("island", "island:landed", ());
    }
}

// Take her in from where she is (her middle on the screen, physical); the
// window stays up for it until she is in (main.rs absorb).
pub fn absorb(sh: &Arc<Shared>, point: (i32, i32)) {
    let sf = {
        let mut isl = sh.island.lock().unwrap();
        isl.reaching = false;
        isl.absorbing = true;
        isl.sf()
    };
    apply_visibility(sh);
    let _ = sh.app.emit_to("island", "island:absorb", json!({ "x": point.0 as f64 / sf, "y": point.1 as f64 / sf }));
}

// --- The settings, grown out of her home ------------------------------------------------

// Open them on a page (or where they were). With no home up (she is out of
// sight), one rises for them and goes once they close.
pub fn open_settings(sh: &Arc<Shared>, tab: Option<&str>) {
    let is_up = sh.is_visible();
    let ready = {
        let mut isl = sh.island.lock().unwrap();
        isl.temp |= !is_up;
        isl.settings_open = true;
        if !isl.ready {
            isl.pending = Some(tab.unwrap_or("").to_string());
        }
        isl.ready
    };
    sh.log(&format!("island: settings open on {:?}{}", tab, if is_up { "" } else { " (risen for them)" }));
    apply_visibility(sh);
    keep_on_top(sh);
    sh.redraw();
    if ready {
        let _ = sh.app.emit_to("island", "island:settings", json!({ "tab": tab.unwrap_or("") }));
    }
}

// The page closed them (Esc, a click outside, ✕, the head). Once the home
// has shrunk back: a risen one goes, and so does one the settings kept up
// (she was hidden, or do not disturb turned on, while they were open).
pub fn settings_closed(sh: &Arc<Shared>) {
    sh.island.lock().unwrap().settings_open = false;
    sh.log("island: settings closed");
    keep_on_top(sh);
    // Set up once Claude Code or Codex is connected, not as the settings
    // close (a click outside closed them, and a new user, nothing
    // connected, was never asked again: the test session, 2026-10-12).
    if !sh.flag("onboarded") && crate::connection::seems_connected() {
        sh.change(json!({ "onboarded": true }));
    } else {
        sh.redraw();
    }
    let sh = sh.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(620));
        {
            let mut isl = sh.island.lock().unwrap();
            if isl.settings_open {
                return;
            }
            isl.temp = false;
        }
        apply_visibility(&sh);
        sh.redraw();
    });
}

// The settings' two buttons by ✕ (settings.js): locked ("settingsPin"), a
// click outside does not close them, and other windows come over them as
// over any window (hers kept on top only while they are not open); on top
// as well ("settingsTop"), they stay over every window, another program's
// own "on top" one too: looked at every 0.7 s, raised should one be over
// them. Unlocked, a click outside closes them: on top as ever.
static TOPMOST: AtomicBool = AtomicBool::new(true);
static WATCHING_TOP: AtomicBool = AtomicBool::new(false);

pub fn keep_on_top(sh: &Arc<Shared>) {
    let open = sh.island.lock().unwrap().settings_open;
    let (locked, top) = (sh.flag("settingsPin"), sh.flag("settingsTop"));
    let topmost = !(open && locked && !top);
    if TOPMOST.swap(topmost, Ordering::SeqCst) != topmost {
        if let Some(win) = window(sh) {
            let _ = win.set_always_on_top(topmost);
            // On top again (or not) takes a Mac's window off the menu bar's level.
            if topmost {
                crate::notch::settle(sh);
            }
        }
        sh.log(&format!("island: settings {}", if topmost { "on top of other windows" } else { "under the windows pressed" }));
    }
    if !(open && locked && top) || WATCHING_TOP.swap(true, Ordering::SeqCst) {
        return;
    }
    let sh = sh.clone();
    std::thread::spawn(move || loop {
        let watched = || sh.island.lock().unwrap().settings_open && sh.flag("settingsPin") && sh.flag("settingsTop");
        while watched() {
            let hwnd = hwnd(&sh);
            let own = sh.own.lock().unwrap().clone();
            if hwnd != 0 && os::covered(hwnd, &own) {
                os::raise_top(hwnd);
            }
            std::thread::sleep(std::time::Duration::from_millis(700));
        }
        WATCHING_TOP.store(false, Ordering::SeqCst);
        // Asked again in the moment between the last look and letting go.
        if !watched() || WATCHING_TOP.swap(true, Ordering::SeqCst) {
            return;
        }
    });
}

pub fn where_(sh: &Shared) -> Value {
    let actual = window(sh).and_then(|win| win.outer_position().ok()).map(|p| json!([p.x, p.y]));
    let isl = sh.island.lock().unwrap();
    json!({
        "actual": actual,
        "pos": [isl.pos.0, isl.pos.1],
        "size": [isl.size.0, isl.size.1],
        "room": isl.room.map(|(w, h)| json!([w, h])),
        "shown": isl.shown,
        "over": isl.ct.over,
        "holding": isl.holding,
        "reaching": isl.reaching,
        "temp": isl.temp,
        "settingsOpen": isl.settings_open,
    })
}

// --- Her windows among the others (Windows) --------------------------------------------------
// Kept on top over another program's own "on top" window (keep_on_top), a
// maximized window on her display (look_over_max), and shown without the
// system's fade (create).

#[cfg(windows)]
mod os {
    use std::ffi::c_void;

    type Hwnd = *mut c_void;

    #[repr(C)]
    #[derive(Default)]
    struct WinRect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn EnumWindows(each: extern "system" fn(Hwnd, isize) -> i32, param: isize) -> i32;
        fn IsWindowVisible(hwnd: Hwnd) -> i32;
        fn IsIconic(hwnd: Hwnd) -> i32;
        fn IsZoomed(hwnd: Hwnd) -> i32;
        fn GetWindow(hwnd: Hwnd, cmd: u32) -> Hwnd;
        fn GetWindowLongPtrW(hwnd: Hwnd, index: i32) -> isize;
        fn GetWindowTextW(hwnd: Hwnd, text: *mut u16, max: i32) -> i32;
        fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
        fn GetClassNameW(hwnd: Hwnd, name: *mut u16, max: i32) -> i32;
        fn GetWindowRect(hwnd: Hwnd, rect: *mut WinRect) -> i32;
        fn SetWindowPos(hwnd: Hwnd, after: isize, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
    }

    #[link(name = "dwmapi")]
    extern "system" {
        fn DwmGetWindowAttribute(hwnd: Hwnd, attr: u32, value: *mut c_void, size: u32) -> i32;
        fn DwmSetWindowAttribute(hwnd: Hwnd, attr: u32, value: *const c_void, size: u32) -> i32;
    }

    const GW_OWNER: u32 = 4;
    const GW_HWNDPREV: u32 = 3;
    const GWL_EXSTYLE: i32 = -20;
    const WS_EX_TRANSPARENT: isize = 0x20;
    const WS_EX_TOOLWINDOW: isize = 0x80;
    const WS_EX_APPWINDOW: isize = 0x0004_0000;
    const WS_EX_NOACTIVATE: isize = 0x0800_0000;
    const DWMWA_CLOAKED: u32 = 14;
    const HWND_TOPMOST: isize = -1;
    const SWP_NOSIZE: u32 = 0x1;
    const SWP_NOMOVE: u32 = 0x2;
    const SWP_NOACTIVATE: u32 = 0x10;
    // The desktop and Windows' own taskbars: never a program's window.
    const NOT_TASKS: [&str; 4] = ["Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd"];

    fn class_of(hwnd: Hwnd) -> String {
        let mut name = [0u16; 128];
        // SAFETY: our own buffer, its size passed.
        let n = unsafe { GetClassNameW(hwnd, name.as_mut_ptr(), 128) }.max(0) as usize;
        String::from_utf16_lossy(&name[..n])
    }

    fn has_title(hwnd: Hwnd) -> bool {
        let mut text = [0u16; 2];
        // SAFETY: as above.
        unsafe { GetWindowTextW(hwnd, text.as_mut_ptr(), 2) > 0 }
    }

    fn pid_of(hwnd: Hwnd) -> u32 {
        let mut pid = 0;
        // SAFETY: our own out-parameter.
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        pid
    }

    fn rect_of(hwnd: Hwnd) -> WinRect {
        let mut r = WinRect::default();
        // SAFETY: our own struct, of the size the call writes.
        unsafe { GetWindowRect(hwnd, &mut r) };
        r
    }

    // Cloaked: on another virtual desktop, or a store app's window not running.
    fn is_cloaked(hwnd: Hwnd) -> bool {
        let mut cloaked = 0u32;
        // SAFETY: our own u32, of the size passed.
        let got = unsafe { DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, &mut cloaked as *mut u32 as *mut c_void, 4) };
        got == 0 && cloaked != 0
    }

    // A program's window, as Windows' taskbar would give it a button: shown,
    // not on another desktop, not owned by another, not a tool window (or
    // marked to be on the taskbar), with a title; not one of hers.
    fn is_task(hwnd: Hwnd, me: u32) -> bool {
        // SAFETY: plain queries about a window being enumerated.
        unsafe {
            if IsWindowVisible(hwnd) == 0 || pid_of(hwnd) == me {
                return false;
            }
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let app = ex & WS_EX_APPWINDOW != 0;
            if !app && (!GetWindow(hwnd, GW_OWNER).is_null() || ex & (WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE) != 0) {
                return false;
            }
        }
        !is_cloaked(hwnd) && !NOT_TASKS.contains(&class_of(hwnd).as_str()) && has_title(hwnd)
    }

    extern "system" fn collect(hwnd: Hwnd, param: isize) -> i32 {
        // SAFETY: param is the Vec maximized_on() passed, alive for the enumeration.
        let found = unsafe { &mut *(param as *mut Vec<isize>) };
        found.push(hwnd as isize);
        1
    }

    // A program's window maximized (not minimized: one put away keeps its
    // maximized state) on this display (x, y, w, h): its middle there.
    pub fn maximized_on(mon: (i32, i32, i32, i32)) -> bool {
        let mut all: Vec<isize> = Vec::new();
        // SAFETY: collect() only pushes, while all lives.
        unsafe { EnumWindows(collect, &mut all as *mut Vec<isize> as isize) };
        let me = std::process::id();
        all.into_iter().any(|h| {
            let hwnd = h as Hwnd;
            // SAFETY: plain queries.
            let maximized = unsafe { IsZoomed(hwnd) != 0 && IsIconic(hwnd) == 0 };
            if !maximized || !is_task(hwnd, me) {
                return false;
            }
            let r = rect_of(hwnd);
            let (x, y) = ((r.left + r.right) / 2, (r.top + r.bottom) / 2);
            x >= mon.0 && x < mon.0 + mon.2 && y >= mon.1 && y < mon.1 + mon.3
        })
    }

    // Another window over this one where it is, in sight (not one of hers,
    // nor one the pointer passes through): another program's own "on top"
    // window that came over it, say.
    pub fn covered(hwnd: isize, own: &[isize]) -> bool {
        let me = rect_of(hwnd as Hwnd);
        let mut at = hwnd as Hwnd;
        for _ in 0..2000 {
            // SAFETY: plain queries about windows above this one.
            unsafe {
                at = GetWindow(at, GW_HWNDPREV);
                if at.is_null() {
                    return false;
                }
                if own.contains(&(at as isize)) || IsWindowVisible(at) == 0 || GetWindowLongPtrW(at, GWL_EXSTYLE) & WS_EX_TRANSPARENT != 0 || is_cloaked(at) {
                    continue;
                }
            }
            let r = rect_of(at);
            if r.right > r.left && r.bottom > r.top && r.left < me.right && me.left < r.right && r.top < me.bottom && me.top < r.bottom {
                return true;
            }
        }
        false
    }

    // To the top of the windows kept on top, the keyboard where it is.
    pub fn raise_top(hwnd: isize) {
        // SAFETY: a window handle and flags; nothing is pointed to.
        unsafe { SetWindowPos(hwnd as Hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
    }

    // Shown and hidden at once, without the system's own fade and growing
    // from the middle (DWMWA_TRANSITIONS_FORCEDISABLED).
    pub fn no_transitions(hwnd: isize) {
        const DWMWA_TRANSITIONS_FORCEDISABLED: u32 = 3;
        let on: i32 = 1;
        // SAFETY: our own BOOL, its size given.
        unsafe { DwmSetWindowAttribute(hwnd as Hwnd, DWMWA_TRANSITIONS_FORCEDISABLED, &on as *const i32 as *const c_void, 4) };
    }
}

#[cfg(not(windows))]
mod os {
    pub fn maximized_on(_mon: (i32, i32, i32, i32)) -> bool {
        false
    }
    pub fn covered(_hwnd: isize, _own: &[isize]) -> bool {
        false
    }
    pub fn raise_top(_hwnd: isize) {}
    pub fn no_transitions(_hwnd: isize) {}
}
