// Her home's window (the page draws it, src/pet/island.js), by display:
//   island  a black pill at the top centre
//   taskbar a strip along the bottom in place of Windows' own, which is put
//           away while it is up, its tray taken over (taskbar.rs); what
//           opens grows up from it, into room kept above it (appbar.rs)
// Both stay up while she is out, and do the talking.
//
// Pulled out of her home, she becomes the pet window (carried under the
// cursor while the button is still held on the home); brought close to it
// again, it reaches for her and takes her back.
//
// Sizes here are logical (the page's) unless they say physical.
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use tauri::utils::config::BackgroundThrottlingPolicy;
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

use crate::pointer::{ClickThrough, Mode};
use crate::screen::Area;
use crate::appbar::Edge;
use crate::{appbar, taskbar, Shared};

// The window without extra room, by home (island.js BASE): the island as
// wide as pulling her out needs, so it only ever grows down. A window whose
// top-left corner moves shows its old picture from there for a frame or two,
// so these never move it. The page asks for more (pet:panel) when a shape
// needs it.
const ISLAND: (f64, f64) = (760.0, 132.0);
// The taskbar's strip (as high as Windows' own, so the Start menu and the
// quick settings open just above it), and the room above it for what opens
// (a window growing up moves its top-left corner). The window is the two.
pub const TASKBAR_H: f64 = 48.0;
// As high as the settings ever are (they stand above the strip, island.js
// settingsMax): opening them never moves the window, which would flash.
const TASKBAR_ROOM: f64 = 640.0;
// Beside a side bar (taskbarEdge left or right): as wide as the settings
// ever are (the Mail page in two panes, settings.js WIDE_W), their gap off
// the strip and their shadow, for the same reason.
const SIDE_ROOM: f64 = 960.0 + 8.0 + 12.0;
// The middle of her portrait at the taskbar's start (its left end, or a
// side bar's top), along the strip.
const TASKBAR_SEAT: f64 = 24.0;
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
    // The window, once made (for the app bar).
    hwnd: isize,
    // The bar's or the taskbar's strip, as Windows granted it (physical x,
    // y, w, h), and what it was asked for (the display, the height, the
    // home): asked again only on a change.
    bar: Option<(i32, i32, i32, i32)>,
    bar_for: Option<((i32, i32, i32, i32), i32, &'static str, Edge)>,
    // The strip is a bar of ours Windows knows (appbar.rs), not one laid over
    // the room Windows' own taskbar keeps (taskbar.rs explorer_room).
    bar_own: bool,
    // The button held on her (a pull, or carrying her out): the window keeps the pointer.
    holding: bool,
    reaching: bool,
    // Taking her in: the window stays up for it.
    pub absorbing: bool,
    // Risen only for the settings (she is out of sight): it goes again once
    // they close.
    pub temp: bool,
    pub settings_open: bool,
    // Risen for the prompts waiting (asked from her taskbar on another
    // display while an app full screen hid it): it goes once they are answered.
    pub for_asks: bool,
    // The page told it is up over an app full screen for them alone (over_full).
    over_full: bool,
    // Settings asked for before its page was up: the page they open on.
    pending: Option<String>,
    // On the taskbar, her seat along the strip from its start (logical: from
    // its left, or a side bar's top), where the page has laid her island out
    // (taskbarLayout: it may be anywhere along the strip); none told yet: at
    // its start.
    seat_at: Option<f64>,
    // The strip, its edge and the window as the page was last told of them
    // (tell_strip).
    strip_told: Option<((i32, i32, i32, i32), Edge, (i32, i32), (i32, i32))>,
}

impl Island {
    fn sf(&self) -> f64 {
        if self.sf > 0.0 { self.sf } else { 1.0 }
    }

    // Physical. The taskbar is as long as its strip (its display, or what
    // Windows granted of it), with its room beside it: above a bottom bar,
    // below a top one, inside a side one's.
    fn size_for(&self, home: &str, area: &Area, edge: Edge) -> (i32, i32) {
        let sf = self.sf();
        let side = home == "taskbar" && edge.is_side();
        let base = match home {
            "taskbar" if side => (TASKBAR_H + SIDE_ROOM, self.bar.map_or(area.mon.3, |b| b.3) as f64 / sf),
            "taskbar" => (area.mon.2 as f64 / sf, TASKBAR_H + TASKBAR_ROOM),
            _ => ISLAND,
        };
        let (w, h) = match self.room {
            Some((w, h)) => (w.max(base.0), h.max(base.1)),
            None => base,
        };
        // The strip's length is the window's; only its room grows.
        let (w, h) = match home {
            "taskbar" if side => (w, base.1),
            "taskbar" => (base.0, h),
            _ => (w, h),
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
// The taskbar is up whatever hides her (out of sight, do not disturb): it is
// the person's taskbar; only an app full screen hides it, as Windows' own.
pub fn is_shown(sh: &Shared) -> bool {
    // Nor while another copy of her holds the taskbar (taskbar.rs waiting).
    let up = if sh.home() == "taskbar" { !sh.is_fullscreen() && !taskbar::waiting() } else { sh.is_visible() };
    let isl = sh.island.lock().unwrap();
    up || isl.temp || isl.settings_open || isl.for_asks
}

// Its window's handle, once made (0 before).
pub fn hwnd(sh: &Shared) -> isize {
    sh.island.lock().unwrap().hwnd
}

// Where its window is (physical) and its scale: the page's px to the screen's.
pub fn origin(sh: &Shared) -> ((i32, i32), f64) {
    let isl = sh.island.lock().unwrap();
    (isl.pos, isl.sf())
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

// The screen's edge her taskbar is along (taskbarEdge).
pub fn edge(sh: &Shared) -> Edge {
    Edge::of(sh.setting("taskbarEdge").as_str().unwrap_or("bottom"))
}

// The display it is on: where it is, else the primary one.
fn area(sh: &Shared) -> Option<Area> {
    let screens = sh.screens();
    let at = {
        let isl = sh.island.lock().unwrap();
        (isl.created && isl.size.0 > 0).then(|| (isl.pos.0 + isl.size.0 / 2, isl.pos.1 + isl.size.1 / 2))
    };
    match at {
        Some((x, y)) => screens.near(x, y),
        None => screens.primary,
    }
}

// The display it is on (x, y, w, h), as area finds it.
pub fn display(sh: &Shared) -> Option<(i32, i32, i32, i32)> {
    area(sh).map(|a| a.mon)
}

// Where the window goes, by home. Returns the physical rect if it moved.
fn fit(sh: &Shared) -> Option<((i32, i32), (i32, i32))> {
    let home = sh.home();
    let edge = edge(sh);
    let area = area(sh)?;
    let mut isl = sh.island.lock().unwrap();
    isl.sf = area.sf;
    let size = isl.size_for(home, &area, edge);
    let pos = match home {
        // The window's edge on the strip's outer one, the room inside it.
        "taskbar" => window_at(edge, isl.bar.unwrap_or_else(|| edge.strip(area.mon, (TASKBAR_H * area.sf).round() as i32)), size),
        // On a Mac the notch's place, over the menu bar (notch.rs); elsewhere
        // the top centre, below a taskbar that sits at the top.
        _ => crate::notch::at_top(area.mon, size.0).unwrap_or((area.x + (area.w - size.0) / 2, area.y)),
    };
    if (pos, size) == (isl.pos, isl.size) {
        return None;
    }
    isl.pos = pos;
    isl.size = size;
    Some((pos, size))
}

// The taskbar's window (physical) for its strip along an edge: its outer
// edge on the strip's, the room inside it.
fn window_at(edge: Edge, b: (i32, i32, i32, i32), size: (i32, i32)) -> (i32, i32) {
    match edge {
        Edge::Bottom => (b.0, b.1 + b.3 - size.1),
        Edge::Top | Edge::Left => (b.0, b.1),
        Edge::Right => (b.0 + b.2 - size.0, b.1),
    }
}

// The strip's thickness across it (physical), as it lies: what the page
// draws it as (its CSS px times the scale), so that its inner edge is the
// work area's, at any scale (175% left a hairline gap: zhoupengjie/Wakuwaku#2).
fn thickness(edge: Edge, b: (i32, i32, i32, i32)) -> i32 {
    if edge.is_side() { b.2 } else { b.3 }
}

// The page told the strip's thickness when the strip, its edge or the window
// change (or anew once it is ready), and a line for the log with all that
// sets where its inner edge lies: Windows' room, the strip, the work area,
// the window (on a change of edge too, the thickness the same: 2026-10-12).
pub fn tell_strip(sh: &Shared, again: bool) {
    if sh.home() != "taskbar" {
        return;
    }
    let edge = edge(sh);
    let (bar, pos, size, sf, told) = {
        let isl = sh.island.lock().unwrap();
        (isl.bar, isl.pos, isl.size, isl.sf(), isl.strip_told)
    };
    let Some(b) = bar else { return };
    let thick = thickness(edge, b);
    let now = Some((b, edge, pos, size));
    if now == told && !again {
        return;
    }
    sh.island.lock().unwrap().strip_told = now;
    let room = (edge == Edge::Bottom).then(|| area(sh).and_then(|a| crate::shell::room(a.mon))).flatten();
    sh.log(&format!(
        "taskbar: strip {b:?} along the {edge:?}, {thick} px ({} at {sf}); Windows' room {room:?}; work area {:?}; window at {pos:?} {size:?}",
        thick as f64 / sf,
        taskbar::work_area_rect()
    ));
    // With the window's size: the page may be a little bigger (island.js
    // fitPage, zhoupengjie/Wakuwaku#19).
    let _ = sh.app.emit_to("island", "taskbar:strip", json!({ "thick": thick, "win": [size.0, size.1] }));
}

// One at a time: two at once took the taskbar twice (two tray threads, one
// never stopped). Asked while one runs, it runs again after, with the last
// ask: 1 give back, 2 keep.
static SYNCING: AtomicBool = AtomicBool::new(false);
static SYNC_ASK: AtomicU8 = AtomicU8::new(0);

fn sync_bar(sh: &Shared, keep: bool) {
    SYNC_ASK.store(1 + keep as u8, Ordering::SeqCst);
    loop {
        if SYNCING.swap(true, Ordering::SeqCst) {
            return;
        }
        while let ask @ 1..=2 = SYNC_ASK.swap(0, Ordering::SeqCst) {
            sync_bar_now(sh, ask == 2);
        }
        SYNCING.store(false, Ordering::SeqCst);
        // An ask that came between the last look and letting go.
        if SYNC_ASK.load(Ordering::SeqCst) == 0 {
            return;
        }
    }
}

// The taskbar's strip along its edge (taskbarEdge): asked of Windows while
// the home is the taskbar and keep (up; also while an app full screen hides
// its window: giving the strip back would move every window), given back
// otherwise. Asked again only when the display, the height, the home or the
// edge change. It comes with Windows' own put away first and goes with it
// given back (taskbar.rs).
fn sync_bar_now(sh: &Shared, keep: bool) {
    let home = sh.home();
    let edge = match home {
        "taskbar" => Some((edge(sh), TASKBAR_H)),
        _ => None,
    };
    let want = match (keep, edge) {
        (true, Some((edge, h))) => area(sh).map(|a| (a, edge, (h * a.sf).round() as i32)),
        _ => None,
    };
    let (hwnd, held, registered) = {
        let isl = sh.island.lock().unwrap();
        (isl.hwnd, isl.bar_for, isl.bar_own)
    };
    if hwnd == 0 {
        return;
    }
    let ask = want.map(|(a, edge, h)| (a.mon, h, home, edge));
    if held == ask {
        return;
    }
    // Another home's strip (or none wanted): given back first. A bar of ours
    // taken away only if there is one: over Windows' room there is none, and
    // asking goes to the taskbar's window of the tray, on the taskbar's
    // thread, waited for (quitting, the main thread).
    if let Some((_, _, was, _)) = held.filter(|h| Some(h.2) != ask.map(|a| a.2)) {
        // The desktop's icons where they are, before the work area goes
        // back under them (icons.rs hold).
        if was == "taskbar" {
            for line in crate::icons::hold_within(&sh.dir, std::time::Duration::from_secs(2)) {
                sh.log(&line);
            }
        }
        if registered {
            appbar::remove(hwnd);
        }
        {
            let mut isl = sh.island.lock().unwrap();
            isl.bar = None;
            isl.bar_for = None;
            isl.bar_own = false;
        }
        if was == "taskbar" {
            taskbar::give_back(sh);
        }
        sh.log(&format!("island: {was} strip given back"));
    }
    let Some((area, edge, height)) = want else { return };
    if home == "taskbar" {
        let strip = edge.strip(area.mon, height);
        // The desktop's icons where they are, before anything of a strip along
        // another edge than the bottom moves them (icons.rs keep_before).
        if edge != Edge::Bottom {
            for line in crate::icons::keep_before_within(&sh.dir, std::time::Duration::from_millis(1500)) {
                sh.log(&line);
            }
        }
        // Windows' own keeps its room along the bottom for a bottom strip,
        // and none for another edge (taskbar.rs set_edge).
        taskbar::set_edge(edge);
        // Another copy of her holds it: nothing taken (it waits).
        if !taskbar::take(sh, strip) {
            return;
        }
        // Along the bottom, laid over the room Windows' own keeps, put away:
        // what it opens (the Start menu, the notifications) opens above it.
        // Its top and height are that room's (the work area stops there),
        // not ours worked out at this scale, when they differ by its
        // rounding (a pixel or two: one a pixel taller left a hairline gap
        // above the strip, zhoupengjie/Wakuwaku#2); a room of another size
        // (Windows 10's, 40 px) leaves ours as it is. Along another edge, a
        // bar of ours (Windows' opens at the bottom).
        if let Some(room) = (edge == Edge::Bottom).then(|| taskbar::explorer_room(area.mon)).flatten() {
            let strip = if (room.3 - height).abs() <= 2 { (area.mon.0, room.1, area.mon.2, room.3) } else { strip };
            let own = {
                let mut isl = sh.island.lock().unwrap();
                isl.bar = Some(strip);
                isl.bar_for = ask;
                std::mem::replace(&mut isl.bar_own, false)
            };
            if own {
                appbar::remove(hwnd);
            }
            taskbar::strip_moved(strip);
            sh.log(&format!("island: taskbar strip {strip:?} over Windows' room {room:?}; work area's bottom {}", taskbar::work_area_bottom()));
            return;
        }
    }
    let registered = sh.island.lock().unwrap().bar_own;
    if !registered && !appbar::register(hwnd) {
        sh.log("island: Windows would not take the bar");
        return;
    }
    let granted = appbar::place(hwnd, area.mon, edge, height);
    {
        let mut isl = sh.island.lock().unwrap();
        isl.bar = Some(granted);
        isl.bar_for = ask;
        isl.bar_own = true;
    }
    if home == "taskbar" {
        taskbar::strip_moved(granted);
    }
    sh.log(&format!("island: {home} strip {granted:?}; work area's bottom {}", taskbar::work_area_bottom()));
}

// Quitting, or the window going: the strip back to the other windows (and
// Windows' own taskbar back, for the taskbar's). Waited for, so it is back
// before she goes (should it take too long, the guard gives it back).
pub fn release_bar(sh: &Shared) {
    let t = std::time::Instant::now();
    while SYNCING.load(Ordering::SeqCst) && t.elapsed() < std::time::Duration::from_secs(5) {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    sync_bar(sh, false);
}

// Explorer started again: it forgot the strip, which is asked for anew.
pub fn take_strip_again(sh: &Shared) {
    let hwnd = {
        let mut isl = sh.island.lock().unwrap();
        if isl.bar_for.take().is_none() {
            return;
        }
        isl.bar_own = false;
        isl.hwnd
    };
    // Should it remember after all, a second registration would be refused.
    appbar::remove(hwnd);
    place(sh);
}

// The window laid on its rect (physical): its place first, then its size,
// then its place again, as bars.rs lay does. Sized first and then moved to
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
    let (pos, size, bar) = {
        let isl = sh.island.lock().unwrap();
        (isl.pos, isl.size, isl.bar)
    };
    // The display it is meant for: the one her strip lies on, or the
    // island's middle; none between strips (a display gone, the new one not
    // yet taken by the taskbar's thread).
    let areas = sh.screens().areas;
    let on = |r: (i32, i32, i32, i32)| areas.iter().find(|a| r.0 >= a.mon.0 && r.1 >= a.mon.1 && r.0 + r.2 <= a.mon.0 + a.mon.2 && r.1 + r.3 <= a.mon.1 + a.mon.3);
    let meant = match bar {
        _ if sh.home() != "taskbar" => on((pos.0 + size.0 / 2, pos.1 + size.1 / 2, 1, 1)),
        Some(b) => on(b),
        None => None,
    };
    let Some(meant) = meant else { return };
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
    sync_bar(sh, true);
    let fitted = fit(sh);
    tell_strip(sh, false);
    let Some((pos, size)) = fitted else { return };
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
                    crate::tasks::no_transitions(hwnd.0 as isize);
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
// alone there (no taskbar to hear it from), so a change of it draws anew.
// Nothing for the default black look, nor while she is quiet.
static OVER_MAX: AtomicBool = AtomicBool::new(false);
static TOP_LIGHT: AtomicU8 = AtomicU8::new(2);

pub fn look_over_max(sh: &Arc<Shared>) {
    let looks = sh.setting("islandTopLook");
    if sh.home() == "taskbar" || sh.is_quiet() || !looks.as_object().is_some_and(|m| m.values().any(Value::is_object)) {
        return;
    }
    let light = crate::wallpaper::look().0;
    if TOP_LIGHT.swap(light as u8, Ordering::SeqCst) != light as u8 {
        sh.redraw();
    }
    let look = &looks[if light { "light" } else { "dark" }];
    let deepens = look["bg"].is_string() && look["op"].as_u64().is_some_and(|op| op < 90) && (look["blur"].as_u64().unwrap_or(0) == 0 || !crate::backplate::available());
    let on = deepens && display(sh).is_some_and(crate::tasks::maximized_on);
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
    taskbar::resend(sh);
    tell_strip(sh, true);
    // The band at a Mac's screen top, now that the page hears it.
    crate::notch::settle(sh);
    sh.greet();
    // Taking the taskbar over: Windows' own laid over ours first, put away
    // under it, then turned to dust that settles into ours.
    let covered = handoff_cover(sh);
    apply_visibility(sh);
    if covered {
        handoff_ask(sh, json!({ "step": "go" }));
        sh.log("handoff: go");
    }
    if let Some(tab) = pending {
        let _ = sh.app.emit_to("island", "island:settings", json!({ "tab": tab }));
    }
}

pub fn apply_visibility(sh: &Arc<Shared>) {
    // Told before it shows, so the strip never shows over the full screen;
    // told it is over once it has gone (or, the full screen gone, stays up).
    let over = over_full(sh);
    let told = sh.island.lock().unwrap().over_full;
    if over && !told {
        tell_over_full(sh, true);
    }
    show_or_hide(sh);
    if !over && told {
        tell_over_full(sh, false);
    }
}

// Up over an app full screen for the prompts alone (show_asks), her taskbar
// the home: the page shows her island and what opens from it, not the strip
// under them (style.css over-full), and the pointer goes through there to
// what is full screen. The whole taskbar, the tray and the clock, came up
// over the video with them (the test session, 2026-10-13). Not with the
// settings open: they stand on the strip.
fn over_full(sh: &Shared) -> bool {
    sh.home() == "taskbar" && sh.is_fullscreen() && {
        let isl = sh.island.lock().unwrap();
        isl.for_asks && !isl.temp && !isl.settings_open
    }
}

fn tell_over_full(sh: &Shared, on: bool) {
    sh.island.lock().unwrap().over_full = on;
    let _ = sh.app.emit_to("island", "island:over-full", json!({ "on": on }));
    sh.log(&format!("island: {}", if on { "over the full screen for the prompts alone, the strip unseen" } else { "the strip seen again" }));
}

fn show_or_hide(sh: &Arc<Shared>) {
    // Another copy of her holding the taskbar: this one waits, its window
    // not shown over the other's.
    if sh.home() == "taskbar" {
        taskbar::look_for_other(sh);
    }
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
    } else {
        // The taskbar keeps the strip it holds while an app full screen
        // hides it; it takes none then: one taken with its window hidden
        // keeps no room (2026-10-10), so it waits for the window to show.
        // Given back, though, to another copy of her holding it too.
        let held = sh.island.lock().unwrap().bar_for.is_some();
        sync_bar(sh, sh.home() == "taskbar" && held && !taskbar::waiting());
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

// --- The handover with Windows' taskbar (handoff.js) ----------------------------------
// Windows' taskbar never gone for a moment. Taking over: a picture of it
// laid over ours, shown exactly over it, before it is put away; then its
// picture turns to dust that settles into ours. Quitting: ours turns to
// dust, Windows' shown again under it, ours fading over it before she
// goes. Each answer of the page's (covered, bare, done) waited for so long
// at most: neither side can hold the other. Not on the main thread: its
// events reach the page through the main thread.
static HANDOFF: Mutex<Vec<String>> = Mutex::new(Vec::new());
static HANDOFF_SAID: std::sync::Condvar = std::sync::Condvar::new();

// What the page answers (taskbar_handoff).
pub fn handoff_answer(what: &str) {
    HANDOFF.lock().unwrap().push(what.to_string());
    HANDOFF_SAID.notify_all();
}

// A step told the page; its answers from before forgotten.
fn handoff_ask(sh: &Shared, step: Value) {
    HANDOFF.lock().unwrap().clear();
    let _ = sh.app.emit_to("island", "taskbar:handoff", step);
}

// An answer, within ms at most: true once it came.
fn handoff_wait(what: &str, ms: u64) -> bool {
    let said = HANDOFF.lock().unwrap();
    let (said, _) = HANDOFF_SAID.wait_timeout_while(said, std::time::Duration::from_millis(ms), |s| !s.iter().any(|x| x == what)).unwrap();
    said.iter().any(|x| x == what)
}

// Her page ready, the taskbar to be taken (not yet up, nothing full screen,
// no other copy of hers holding it): Windows' taskbar pictured, her window
// shown exactly over it with the picture laid on, before it is put away.
// True once covered: then `go` follows the taking. A picture that cannot
// be had (auto-hidden, already put away) is none: the dust comes from above.
fn handoff_cover(sh: &Arc<Shared>) -> bool {
    if sh.home() != "taskbar" || taskbar::is_up() {
        return false;
    }
    taskbar::look_for_other(sh);
    if !is_shown(sh) {
        return false;
    }
    let Some(area) = area(sh) else { return false };
    // Windows' strip pictured only where hers takes its place: along the
    // bottom. Along another edge none (the dust gathers from above).
    let picture = (edge(sh) == Edge::Bottom).then(|| taskbar::strip_picture(area.mon)).flatten();
    let _ = fit(sh);
    let (pos, size, hwnd) = {
        let mut isl = sh.island.lock().unwrap();
        isl.shown = true;
        (isl.pos, isl.size, isl.hwnd)
    };
    let Some(win) = window(sh) else { return false };
    lay(&win, pos, size);
    let _ = win.show();
    let shown = std::time::Instant::now();
    // Over Windows' own, which is kept on top too.
    crate::tasks::raise_top(hwnd);
    let pictured = picture.is_some();
    // Its window's size (physical) too: the page lays the picture only once
    // its own size has come to that (a window made bigger reaches the page
    // some frames later; laid before, it sat too small, up and to the left).
    handoff_ask(sh, json!({ "step": "in", "picture": picture, "size": [size.0, size.1] }));
    let covered = handoff_wait("covered", 800);
    // Drawn by the page is not yet on the screen: a window just shown hands
    // WebView2's frames to DWM some 40 ms later, and Windows' strip put away
    // at once left the bare desktop under the strip that long (whatever the
    // material: the test session's films, 2026-10-13). So two of DWM's passes
    // waited for, and 150 ms since the window showed at least; the picture is
    // Windows' strip itself, so the wait cannot be seen.
    if pictured {
        crate::tasks::dwm_flush();
        crate::tasks::dwm_flush();
        let least = std::time::Duration::from_millis(150);
        if let Some(rest) = least.checked_sub(shown.elapsed()) {
            std::thread::sleep(rest);
        }
    }
    sh.log(&format!(
        "handoff: in ({}), {} ({} ms after the window showed)",
        if pictured { "pictured" } else { "no picture" },
        if covered { "covered" } else { "no answer in 800 ms" },
        shown.elapsed().as_millis()
    ));
    true
}

// Quitting with the taskbar up (and nothing full screen): hers turns to
// dust, Windows' shown under it once the page says so (bare), hers kept
// over it while it fades (done). Then she goes, and gives it back as ever.
pub fn handoff_out(sh: &Shared) {
    let shown = sh.island.lock().unwrap().shown;
    if sh.home() != "taskbar" || !taskbar::is_up() || !shown || sh.is_fullscreen() {
        return;
    }
    handoff_ask(sh, json!({ "step": "out" }));
    let bare = handoff_wait("bare", 500);
    taskbar::handing_back();
    crate::shell::show_bars();
    crate::tasks::raise_top(hwnd(sh));
    let done = bare && handoff_wait("done", 1500);
    sh.log(&format!("handoff: out, {}, {}", if bare { "bare" } else { "no answer in 500 ms" }, if done { "done" } else { "not waited for" }));
}

// Kept in its place (a display gone, the scale changed, another bar). And
// put back where it was put if Windows moved it: the taskbar's window, which
// reaches over its own strip, was pushed up into the work area by the strip's
// height (2026-10-10, as a game went full screen and back).
pub fn keep_on_screen(sh: &Shared) {
    let (is_up, pos, size) = {
        let isl = sh.island.lock().unwrap();
        (isl.shown && !isl.holding && !isl.reaching, isl.pos, isl.size)
    };
    if !is_up {
        return;
    }
    // Where it belongs now (a display come or gone moves it), then its size
    // as Windows left it: place compares only with what it asked for last,
    // the same, and would never set it again (#37).
    place(sh);
    lay_again(sh, "seen by the look");
    if sh.home() != "taskbar" {
        return;
    }
    let Some(win) = window(sh) else { return };
    if let Ok(at) = win.outer_position() {
        if (at.x, at.y) != pos && sh.island.lock().unwrap().pos == pos {
            sh.log(&format!("island: moved by Windows to ({}, {}); put back at {pos:?}", at.x, at.y));
            lay(&win, pos, size);
        }
    }
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

// Her seat on the taskbar as the page has laid her island out (logical px
// from the strip's left): what she is reached for from, and flies back to.
pub fn set_seat(sh: &Shared, at: f64) {
    if at.is_finite() && at >= 0.0 {
        sh.island.lock().unwrap().seat_at = Some(at);
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
// she is reached for from and taken back into. The island: the middle of
// its lower edge (compact); the taskbar: her portrait, on the strip's inner
// edge (above a bottom bar, below a top one, beside a side one).
pub fn seat(sh: &Shared) -> Option<(i32, i32)> {
    let home = sh.home();
    let edge = edge(sh);
    let area = area(sh)?;
    let (bar, seat_at) = {
        let isl = sh.island.lock().unwrap();
        (isl.bar, isl.seat_at)
    };
    let px = |v: f64| (v * area.sf).round() as i32;
    Some(match home {
        "taskbar" => {
            let (x, y, w, h) = bar.unwrap_or_else(|| edge.strip(area.mon, px(TASKBAR_H)));
            let at = px(seat_at.unwrap_or(TASKBAR_SEAT));
            match edge {
                Edge::Bottom => (x + at, y),
                Edge::Top => (x + at, y + h),
                Edge::Left => (x + w, y + at),
                Edge::Right => (x, y + at),
            }
        }
        _ => (area.x + area.w / 2, area.y + px(TOP + COMPACT_H)),
    })
}

// Where she flies to before her home takes her in: just under the island,
// just off the taskbar (inside it), where the drop can take her.
pub fn landing(sh: &Shared) -> Option<(i32, i32)> {
    let seat = seat(sh)?;
    let sf = area(sh).map_or(1.0, |a| a.sf);
    let off = (70.0 * sf) as i32;
    Some(match (sh.home(), edge(sh)) {
        ("taskbar", Edge::Bottom) => (seat.0, seat.1 - off),
        ("taskbar", Edge::Top) => (seat.0, seat.1 + off),
        ("taskbar", Edge::Left) => (seat.0 + off, seat.1),
        ("taskbar", Edge::Right) => (seat.0 - off, seat.1),
        _ => (seat.0, seat.1 + off),
    })
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

// The prompts answered: an island risen for them goes.
pub fn asks_answered(sh: &Arc<Shared>) {
    let was = std::mem::take(&mut sh.island.lock().unwrap().for_asks);
    if was {
        apply_visibility(sh);
    }
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
            if hwnd != 0 && crate::tasks::covered(hwnd, &own) {
                crate::tasks::raise_top(hwnd);
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
        "bar": isl.bar.map(|b| json!([b.0, b.1, b.2, b.3])),
    })
}

#[cfg(test)]
mod strip_tests {
    use super::{thickness, window_at, Edge, SIDE_ROOM, TASKBAR_H, TASKBAR_ROOM};

    // At every scale, a strip as Windows grants it (its room, a pixel more
    // or less than ours as it rounds, or our own bar's): the window's outer
    // edge on the strip's, and the page's strip (its CSS px times the scale)
    // exactly as thick, so its inner edge is the work area's.
    #[test]
    fn the_strip_and_the_window_meet_at_any_scale() {
        let mon = (0, 0, 3840, 2160);
        for sf in [1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 1.37] {
            let ours = (TASKBAR_H * sf).round() as i32;
            for edge in [Edge::Bottom, Edge::Top, Edge::Left, Edge::Right] {
                for t in [ours - 1, ours, ours + 1] {
                    let b = edge.strip(mon, t);
                    let size = if edge.is_side() { (((TASKBAR_H + SIDE_ROOM) * sf).round() as i32, mon.3) } else { (mon.2, ((TASKBAR_H + TASKBAR_ROOM) * sf).round() as i32) };
                    let (x, y) = window_at(edge, b, size);
                    // The window's outer edge on the strip's, the strip inside it.
                    match edge {
                        Edge::Bottom => assert_eq!(y + size.1, b.1 + b.3, "{sf} {edge:?} {t}"),
                        Edge::Top => assert_eq!(y, b.1, "{sf} {edge:?} {t}"),
                        Edge::Left => assert_eq!(x, b.0, "{sf} {edge:?} {t}"),
                        Edge::Right => assert_eq!(x + size.0, b.0 + b.2, "{sf} {edge:?} {t}"),
                    }
                    assert!(x <= b.0 && y <= b.1 && x + size.0 >= b.0 + b.2 && y + size.1 >= b.1 + b.3, "{sf} {edge:?} {t}");
                    // The page's strip: thick / scale CSS px, back to exactly thick.
                    let thick = thickness(edge, b);
                    assert_eq!(thick, t);
                    assert_eq!(((thick as f64 / sf) * sf).round() as i32, t, "{sf} {edge:?} {t}");
                }
            }
        }
    }
}
