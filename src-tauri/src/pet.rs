// Her windows: transparent, frameless, always on top, her whole self on the
// desktop, one for each agent's pet (pet-claude, pet-codex, ...: data.rs
// AGENTS). Where each sits and how big it is, the mouse (click-through, her
// eyes, dragging and being carried) and her walks. Up when she is out of
// her home (out); at home, her home's window has her (island.rs).
//
// Everything here is in physical pixels; the page gets logical (CSS) ones.
//
// Locks: a window getter waits on the main thread, and menu clicks run there
// and take these locks; so no getter is ever called with a lock held, and no
// two locks are held at once.
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::utils::config::BackgroundThrottlingPolicy;
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::pointer::{ClickThrough, Mode};
use crate::{island, now_ms, Shared};

// Window size at scale 1: room for the bubble above a 192x208 cell, and
// wide enough that the bubble says something before it has to cut it short.
const BASE_W: f64 = 360.0;
const BASE_H: f64 = 320.0;
const CELL_H: f64 = 208.0;
const CELL_W: f64 = 192.0;
// Between two pets' feet on one screen: her width and this much more (logical px).
const ROOM_GAP: f64 = 20.0;
// A pet pushed aside walks there this fast (ms a logical px; her own walks, 18).
const PUSH_MS_PER_PX: f64 = 6.0;
// The cursor further than this (logical px) from her face does not catch her eye.
const LOOK_FAR: f64 = 900.0;
// Two clicks on her this close together are a double-click.
const DOUBLE_MS: u64 = 500;
// How long a click on her still goes to the session she showed as the pointer came.
const CLICK_GOES_MS: u64 = 60_000;
// Flying back into the island, from wherever she is.
const FLY_MS: f64 = 380.0;

struct Drag {
    // The cursor's offset in the window.
    dx: i32,
    dy: i32,
    x: i32,
    y: i32,
    x0: i32,
    y0: i32,
    moved: bool,
    // Pulled out of the island: the button is the island's, she takes no mouse.
    carried: bool,
}

#[derive(Default)]
pub struct Win {
    pos: (i32, i32),
    size: (i32, i32),
    sf: f64,
    // Its window made (an agent's pet not let out has none).
    made: bool,
    ready: bool,
    shown: bool,
    ct: ClickThrough,
    drag: Option<Drag>,
    walk_gen: u64,
    walking: bool,
    // Where her feet are headed on a walk (x, physical px): the others make
    // room for her there, not where she is on the way.
    heading: Option<i32>,
    flying: bool,
    last_click: Option<Instant>,
    // The session she was showing when the pointer came over her (seeing it
    // sends it to rest), and when: a click on her goes to its window.
    click_goes: Option<(String, Instant)>,
    // Watching the button while the pointer is on her (see poll).
    watching: bool,
    was_down: bool,
    // The prompt panel's size (logical) while one is up above her, as the page measured it.
    panel: Option<(f64, f64)>,
    // How far she sits right of the window's centre: when the screen edge
    // stops the grown window from centring on her, she shifts so she stays put.
    shift: i32,
    // The panel below her, not above: too little room above her on her
    // screen. The window then starts at her top, she at its top.
    below: bool,
    // Where the window was before a panel grew it, to go back to exactly.
    parked: Option<(i32, i32)>,
}

// Each agent's pet window's state, one lock each: never two held at once.
pub struct Pets([std::sync::Mutex<Win>; 4]);

impl Default for Pets {
    fn default() -> Self {
        Pets(Default::default())
    }
}

impl Pets {
    // The window's state of an agent's pet (one not known: Claude's).
    pub fn of(&self, agent: &str) -> &std::sync::Mutex<Win> {
        &self.0[crate::data::AGENTS.iter().position(|a| *a == agent).unwrap_or(0)]
    }
}

// An agent's pet window's label, as the capabilities know it (pet-*).
pub fn label(agent: &str) -> String {
    format!("pet-{agent}")
}

// The agent of a pet window, by its label.
pub fn agent_of(label: &str) -> Option<&str> {
    label.strip_prefix("pet-").filter(|a| crate::data::AGENTS.contains(a))
}

// Her window's display's scale.
pub fn sf(sh: &Shared, agent: &str) -> f64 {
    sh.pets.of(agent).lock().unwrap().sf()
}

impl Win {
    fn sf(&self) -> f64 {
        if self.sf > 0.0 { self.sf } else { 1.0 }
    }

    fn base_size(&self, scale: f64) -> (i32, i32) {
        ((BASE_W * scale * self.sf()).round() as i32, (BASE_H * scale * self.sf()).round() as i32)
    }

    // The window now: taller, and wider if need be, while a prompt panel is up.
    fn size_for(&self, scale: f64) -> (i32, i32) {
        let base = self.base_size(scale);
        match self.panel {
            Some((w, h)) => (base.0.max((w * self.sf()).ceil() as i32), base.1 + (h * self.sf()).ceil() as i32),
            None => base,
        }
    }

    // Her own height on the screen (one cell).
    fn her_height(&self, scale: f64) -> i32 {
        (CELL_H * scale * self.sf()).round() as i32
    }

    // Her feet on the screen: the window's bottom, or with a panel below
    // her, her own height down from its top.
    fn bottom(&self, scale: f64) -> i32 {
        if self.below {
            self.pos.1 + self.her_height(scale)
        } else {
            self.pos.1 + self.size.1
        }
    }

    // Her middle on the screen, for the island to reach for.
    fn her_point(&self, scale: f64) -> (i32, i32) {
        (self.pos.0 + self.size.0 / 2 + self.shift, self.bottom(scale) - self.her_height(scale) / 2)
    }
}

fn window(sh: &Shared, agent: &str) -> Option<WebviewWindow> {
    sh.app.get_webview_window(&label(agent))
}

fn scale(sh: &Shared) -> f64 {
    sh.setting("scale").as_f64().unwrap_or(0.55)
}

// Up: she is out of her home.
pub fn is_shown(sh: &Shared, agent: &str) -> bool {
    sh.is_visible() && sh.is_out(agent)
}

// The display her window is on (x, y, w, h).
pub fn display(sh: &Shared, agent: &str) -> Option<(i32, i32, i32, i32)> {
    let (pos, size) = {
        let win = sh.pets.of(agent).lock().unwrap();
        (win.pos, win.size)
    };
    sh.screens().near(pos.0 + size.0 / 2, pos.1 + size.1 / 2).map(|a| a.mon)
}

fn set_ignore(sh: &Shared, agent: &str, ignore: Option<bool>) {
    if let (Some(ignore), Some(win)) = (ignore, window(sh, agent)) {
        sh.log(&format!("pet: mouse {}", if ignore { "passes through" } else { "caught" }));
        let _ = win.set_ignore_cursor_events(ignore);
    }
}

fn place(sh: &Shared, agent: &str, (x, y): (i32, i32)) {
    if let Some(win) = window(sh, agent) {
        // On a Mac a window's physical place is in its own display's scale,
        // which changes as she crosses to a display of another: placed in
        // points, from the scale her place was reckoned in (Win::sf), she
        // stays under the pointer across (dragged over, she drifted away
        // from it, 2026-10-14). Elsewhere the physical place is one space.
        #[cfg(target_os = "macos")]
        {
            let sf = sh.pets.of(agent).lock().unwrap().sf();
            let _ = win.set_position(tauri::LogicalPosition::new(x as f64 / sf, y as f64 / sf));
        }
        #[cfg(not(target_os = "macos"))]
        let _ = win.set_position(PhysicalPosition::new(x, y));
    }
}

// Move the window, kept on screen unless `free`.
fn move_to(sh: &Shared, agent: &str, x: i32, y: i32, free: bool) -> (i32, i32) {
    let screens = sh.screens();
    let p = {
        let mut w = sh.pets.of(agent).lock().unwrap();
        let p = if free { (x, y) } else { screens.clamp((x, y), w.size) };
        w.pos = p;
        p
    };
    place(sh, agent, p);
    p
}

// Save where the window is, as where it would be without a prompt panel.
fn remember(sh: &Shared, agent: &str) {
    let scale = scale(sh);
    let (x, y) = {
        let w = sh.pets.of(agent).lock().unwrap();
        let base = w.base_size(scale);
        (w.pos.0 + w.size.0 / 2 + w.shift - base.0 / 2, w.bottom(scale) - base.1)
    };
    let mut settings = sh.settings.lock().unwrap();
    let spots = settings.entry("spots").or_insert_with(|| json!({}));
    if let Some(spots) = spots.as_object_mut() {
        spots.insert(agent.into(), json!([x, y]));
    }
    crate::data::save(&sh.dir, &settings);
}

// Bottom right of the primary display: each agent's pet a window's width
// further left than the one before it (data.rs AGENTS), so they do not stand
// on each other.
fn home_spot(sh: &Shared, agent: &str) -> (i32, i32) {
    let primary = sh.screens().primary;
    let nth = crate::data::AGENTS.iter().position(|a| *a == agent).unwrap_or(0) as i32;
    let w = sh.pets.of(agent).lock().unwrap();
    let margin = (24.0 * w.sf()).round() as i32;
    match primary {
        Some(a) => (a.x + a.w - w.size.0 - margin - nth * w.size.0, a.y + a.h - w.size.1 - margin),
        None => (0, 0),
    }
}

// Where she was last left, or home.
fn saved_spot(sh: &Shared, agent: &str) -> (i32, i32) {
    let saved = sh.setting("spots").get(agent).cloned().unwrap_or(Value::Null);
    let spot = match (saved.get(0).and_then(Value::as_i64), saved.get(1).and_then(Value::as_i64)) {
        (Some(x), Some(y)) => (x as i32, y as i32),
        _ => home_spot(sh, agent),
    };
    let size = sh.pets.of(agent).lock().unwrap().size;
    sh.screens().clamp(spot, size)
}

// --- Room between them ----------------------------------------------------------------

// Where her feet are on the screen (their middle, physical px), the screen
// they are on, and whether she may be moved (not with a panel or a bubble up).
struct Feet {
    x: i32,
    mon: (i32, i32, i32, i32),
    sf: f64,
    fixed: bool,
}

// Where she is now, or (planning: headed) where she will be.
fn feet(sh: &Shared, agent: &str, screens: &crate::screen::Screens, headed: bool) -> Option<Feet> {
    let scale = scale(sh);
    let (x, y, fixed) = {
        let w = sh.pets.of(agent).lock().unwrap();
        if !w.shown {
            return None;
        }
        let (x, y) = (w.pos.0 + w.size.0 / 2 + w.shift, w.bottom(scale));
        let x = if headed { w.heading.unwrap_or(x) } else { x };
        (x, y, w.panel.is_some())
    };
    let a = screens.near(x, y - 1)?;
    Some(Feet { x, mon: a.mon, sf: a.sf, fixed })
}

// The other pets' feet on the same screen as hers (each taken on its own:
// no two locks held).
fn others_by(sh: &Shared, agent: &str, me: &Feet, screens: &crate::screen::Screens, headed: bool) -> Vec<(&'static str, i32)> {
    crate::data::AGENTS
        .into_iter()
        .filter(|a| *a != agent)
        .filter_map(|a| feet(sh, a, screens, headed).filter(|f| f.mon == me.mon).map(|f| (a, f.x)))
        .collect()
}

fn xs(others: &[(&str, i32)]) -> Vec<i32> {
    others.iter().map(|(_, x)| *x).collect()
}

// How far apart two pets' feet stay: her width and a gap, in her screen's px.
fn room(scale: f64, sf: f64) -> i32 {
    ((CELL_W * scale + ROOM_GAP) * sf).round() as i32
}

// Walking from one spot to another (her feet's x): the farthest she gets
// without coming nearer than room to another in her way. Walking away from
// one already too near is free; walking into it, she stays.
pub fn clip_walk(from: i32, to: i32, others: &[i32], room: i32) -> i32 {
    let mut to = to;
    for &o in others {
        if to > from && o > from {
            to = to.min((o - room).max(from));
        } else if to < from && o < from {
            to = to.max((o + room).min(from));
        }
    }
    to
}

// Set down too near another: the nearest spot room away from every other,
// within lo..=hi (her feet's x on her screen), reached without passing one
// clear of her (one she is on she gets off of either way). To the left if
// there is room there, else to the right; None: nowhere.
pub fn free_spot(x: i32, others: &[i32], room: i32, (lo, hi): (i32, i32)) -> Option<i32> {
    let fits = |at: i32| (lo..=hi).contains(&at) && others.iter().all(|&o| (at - o).abs() >= room);
    if fits(x) {
        return Some(x);
    }
    let passes = |at: i32| others.iter().any(|&o| (o - x).abs() >= room && o > at.min(x) && o < at.max(x));
    let spots = || others.iter().flat_map(|&o| [o - room, o + room]).filter(|&at| fits(at) && !passes(at));
    let left = spots().filter(|&at| at < x).max();
    let right = spots().filter(|&at| at > x).min();
    left.or(right)
}

// Set down (let go, carried out, back as she starts): too near another on
// her screen, she walks aside to a free spot, leftwards if she can. Not
// with a panel or a bubble up: then the others keep away from her.
pub fn make_room(sh: &Arc<Shared>, agent: &str) {
    make_room_again(sh, agent, 3);
}

// Set down at the same moment as another (as they start), each may have
// planned before the other went: once there, looked at again, so often.
fn make_room_again(sh: &Arc<Shared>, agent: &str, tries: u32) {
    let screens = sh.screens();
    let Some(me) = feet(sh, agent, &screens, false) else { return };
    if me.fixed || tries == 0 {
        return;
    }
    // Room among the others where they are headed.
    let others = xs(&others_by(sh, agent, &me, &screens, true));
    let scale = scale(sh);
    let room = room(scale, me.sf);
    let Some(a) = screens.near(me.x, sh.pets.of(agent).lock().unwrap().bottom(scale) - 1) else { return };
    let half = (CELL_W * scale * me.sf / 2.0).round() as i32;
    let Some(spot) = free_spot(me.x, &others, room, (a.x + half, a.x + a.w - half)) else { return };
    if spot == me.x {
        return;
    }
    sh.log(&format!("pet: {agent} set down too near another, walks {} px aside", spot - me.x));
    let dx = (spot - me.x) as f64 / me.sf;
    // Headed there from now: the others plan around it.
    sh.pets.of(agent).lock().unwrap().heading = Some(spot);
    let sh = sh.clone();
    let agent = agent.to_string();
    std::thread::spawn(move || {
        walk_as(&sh, &agent, dx, dx.abs() * PUSH_MS_PER_PX, true);
        // Headed nowhere now, whether she got there or never set off.
        sh.pets.of(&agent).lock().unwrap().heading = None;
        make_room_again(&sh, &agent, tries - 1);
    });
}

// --- The window ---------------------------------------------------------------------

pub fn create(sh: &Shared, agent: &str) -> tauri::Result<()> {
    let primary = sh.screens().primary;
    {
        let mut w = sh.pets.of(agent).lock().unwrap();
        w.sf = primary.map_or(1.0, |a| a.sf);
        w.size = w.base_size(scale(sh));
        w.ct.force(Mode::Pass);
        w.made = true;
    }
    let pos = saved_spot(sh, agent);
    let (size, sf) = {
        let mut w = sh.pets.of(agent).lock().unwrap();
        w.pos = pos;
        (w.size, w.sf())
    };

    let win = WebviewWindowBuilder::new(&sh.app, label(agent), WebviewUrl::App(format!("pet/index.html?role=pet&agent={agent}").into()))
        .title("wakuwaku")
        .inner_size(size.0 as f64 / sf, size.1 as f64 / sf)
        .additional_browser_args(crate::browser_args(sh))
        .data_directory(crate::webview_dir(sh))
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
        // An unfocused, half-transparent window still has to animate.
        .background_throttling(BackgroundThrottlingPolicy::Disabled)
        .build()?;
    win.set_size(PhysicalSize::new(size.0 as u32, size.1 as u32))?;
    win.set_position(PhysicalPosition::new(pos.0, pos.1))?;
    // Clicks pass through until the pointer is on her.
    win.set_ignore_cursor_events(true)?;
    sh.own_window(&win);
    // Its renderer gone: told, and the page loaded again (crash.rs).
    crate::crash::watch(&crate::shared(&sh.app), &win);
    Ok(())
}

// Whether an agent's pet has its window.
pub fn is_made(sh: &Shared, agent: &str) -> bool {
    sh.pets.of(agent).lock().unwrap().made
}

// Carried out of the island right now.
pub fn is_carried(sh: &Shared, agent: &str) -> bool {
    sh.pets.of(agent).lock().unwrap().drag.as_ref().is_some_and(|d| d.carried)
}

// Out on the desktop: its window made; home: closed, its state let go (its
// spot is kept in the settings). Not while it is carried or flying home.
pub fn open_or_close(sh: &Arc<Shared>, agent: &str) {
    let out = sh.is_out(agent);
    let (made, busy) = {
        let w = sh.pets.of(agent).lock().unwrap();
        (w.made, w.drag.is_some() || w.flying)
    };
    if out && !made {
        sh.log(&format!("pet: {agent}'s window made"));
        if let Err(e) = create(sh, agent) {
            sh.log(&format!("pet: {agent}'s window not made: {e}"));
        }
    } else if !out && made && !busy {
        sh.log(&format!("pet: {agent}'s window closed"));
        if let Some(win) = window(sh, agent) {
            sh.disown_window(&win);
            let _ = win.destroy();
        }
        *sh.pets.of(agent).lock().unwrap() = Win::default();
    }
}

// Her page has its listeners up: draw and show (her home says hello).
pub fn ready(sh: &Arc<Shared>, agent: &str) {
    sh.log("pet: page ready");
    sh.pets.of(agent).lock().unwrap().ready = true;
    sh.redraw();
    // Her window made hidden, its page was not: put away with it while she
    // is home (as she starts, mostly; apply_visibility has nothing to change).
    if !is_shown(sh, agent) {
        if let Some(win) = window(sh, agent) {
            hide_window(&win);
        }
    }
    apply_visibility(sh, agent);
    // Back where she was left: not on another.
    make_room(sh, agent);
}

pub fn apply_visibility(sh: &Shared, agent: &str) {
    let want = is_shown(sh, agent);
    {
        let mut w = sh.pets.of(agent).lock().unwrap();
        if !w.ready || w.shown == want {
            return;
        }
        w.shown = want;
        if !want {
            w.drag = None;
            w.walk_gen += 1;
            w.walking = false;
        }
    }
    if let Some(win) = window(sh, agent) {
        if want {
            let _ = win.as_ref().show();
            sh.show_window(&win);
            // Hidden, her page heard no updates (main.rs to_pages): the
            // whole state now, whether quiet with it.
            sh.send_to_pet(agent);
        } else {
            hide_window(&win);
        }
    }
}

// Her window put away, and its page with it (WebView2's IsVisible): with
// the window alone hidden, the page went on playing her clips and walking
// her about unseen, its timers never throttled (#14: half her renderer's and
// most of her GPU process's time while she was home).
fn hide_window(win: &WebviewWindow) {
    let _ = win.hide();
    let _ = win.as_ref().hide();
}

// Her window on the screen now. Asked from anywhere (main.rs to_pages),
// perhaps while the window's state is held: then taken as up.
pub fn is_up(sh: &Shared, agent: &str) -> bool {
    sh.pets.of(agent).try_lock().map(|w| w.shown).unwrap_or(true)
}

// A new size, for every pet: each keeps its spot, its bottom centre.
pub fn resize(sh: &Arc<Shared>, scale: f64) {
    sh.change(json!({ "scale": scale }));
    for agent in crate::data::AGENTS {
        if window(sh, agent).is_some() {
            resize_one(sh, agent, scale);
        }
    }
}

fn resize_one(sh: &Shared, agent: &str, scale: f64) {
    let screens = sh.screens();
    let (p, size) = {
        let mut w = sh.pets.of(agent).lock().unwrap();
        let old = w.size;
        w.size = w.size_for(scale);
        let (x, y) = w.pos;
        let p = screens.clamp((x + (old.0 - w.size.0) / 2, y + old.1 - w.size.1), w.size);
        w.pos = p;
        (p, w.size)
    };
    if let Some(win) = window(sh, agent) {
        let _ = win.set_size(PhysicalSize::new(size.0 as u32, size.1 as u32));
    }
    place(sh, agent, p);
    remember(sh, agent);
}

pub fn come_home(sh: &Shared, agent: &str) {
    stop_walking(sh, agent);
    let home = home_spot(sh, agent);
    move_to(sh, agent, home.0, home.1, false);
    remember(sh, agent);
}

// Back to where she was left (out of the island, or the pet again).
pub fn return_to_spot(sh: &Shared, agent: &str) {
    let scale = scale(sh);
    let size = {
        let mut w = sh.pets.of(agent).lock().unwrap();
        w.panel = None;
        w.parked = None;
        w.shift = 0;
        w.below = false;
        w.size = w.size_for(scale);
        w.size
    };
    let _ = sh.app.emit_to(label(&agent), "pet:shift", json!({ "x": 0, "below": false }));
    if let Some(win) = window(sh, agent) {
        let _ = win.set_size(PhysicalSize::new(size.0 as u32, size.1 as u32));
    }
    let p = saved_spot(sh, agent);
    move_to(sh, agent, p.0, p.1, false);
}

// The prompt panel's size (logical) as the page measured it, or None once it
// has gone. It grows the window upwards and both ways, her staying put.
pub fn set_panel(sh: &Shared, agent: &str, measured: Option<(f64, f64)>) {
    let scale = scale(sh);
    let screens = sh.screens();
    let (pos, size, shift, sf, below) = {
        let mut w = sh.pets.of(agent).lock().unwrap();
        if w.panel == measured {
            return;
        }
        let (old, before) = (w.pos, w.size);
        let pet_x = old.0 + before.0 / 2 + w.shift;
        let pet_bottom = w.bottom(scale);
        if w.panel.is_none() && measured.is_some() {
            w.parked = Some(old);
            // Not walking off with it.
            w.walk_gen += 1;
            w.walking = false;
        }
        w.panel = measured;
        w.size = w.size_for(scale);
        match (measured, w.parked.take()) {
            (None, Some(parked)) => {
                w.pos = parked;
                w.shift = 0;
                w.below = false;
            }
            (panel, parked) => {
                w.parked = parked;
                let want_x = pet_x - w.size.0 / 2;
                // Above her if it fits on her screen's work area; else below
                // if that does; else whichever side has more room.
                let grown = w.size.1 - w.base_size(scale).1;
                let her_h = w.her_height(scale);
                w.below = match (panel, screens.near(pet_x, pet_bottom - her_h / 2)) {
                    (Some(_), Some(a)) => {
                        let above = pet_bottom - w.base_size(scale).1 - a.y;
                        let below = a.y + a.h - pet_bottom;
                        above < grown && (below >= grown || below > above)
                    }
                    _ => false,
                };
                let want_y = if w.below { pet_bottom - her_h } else { pet_bottom - w.size.1 };
                let p = screens.clamp((want_x, want_y), w.size);
                w.pos = p;
                w.shift = if panel.is_some() { want_x - p.0 } else { 0 };
            }
        }
        (w.pos, w.size, w.shift, w.sf(), w.below)
    };
    if let Some(win) = window(sh, agent) {
        let _ = win.set_size(PhysicalSize::new(size.0 as u32, size.1 as u32));
    }
    place(sh, agent, pos);
    let _ = sh.app.emit_to(label(&agent), "pet:shift", json!({ "x": (shift as f64 / sf).round(), "below": below }));
}

// Put the window back on screen at its own size if anything moved it off or
// stretched it (a display unplugged, the scale changed).
pub fn keep_on_screen(sh: &Shared, agent: &str) {
    let Some(win) = window(sh, agent) else { return };
    {
        let w = sh.pets.of(agent).lock().unwrap();
        if !w.ready || !w.shown || w.drag.is_some() || w.walking || w.flying {
            return;
        }
    }
    let (Ok(pos), Ok(size)) = (win.outer_position(), win.outer_size()) else { return };
    let screens = sh.screens();
    let scale = scale(sh);
    let fix = {
        let mut w = sh.pets.of(agent).lock().unwrap();
        if w.drag.is_some() || w.walking || w.flying {
            return;
        }
        let centre = (pos.x + size.width as i32 / 2, pos.y + size.height as i32 / 2);
        if let Some(a) = screens.near(centre.0, centre.1) {
            w.sf = a.sf;
        }
        w.size = w.size_for(scale);
        let is_stretched = (size.width as i32 - w.size.0).abs() > 3 || (size.height as i32 - w.size.1).abs() > 3;
        let p = screens.clamp((pos.x, pos.y), w.size);
        w.pos = p;
        (is_stretched || p != (pos.x, pos.y)).then_some((p, w.size, is_stretched))
    };
    if let Some((p, size, is_stretched)) = fix {
        sh.log(&format!("pet: back on screen at {p:?}{}", if is_stretched { ", resized" } else { "" }));
        let _ = win.set_size(PhysicalSize::new(size.0 as u32, size.1 as u32));
        place(sh, agent, p);
        remember(sh, agent);
    }
}

// --- The pointer ----------------------------------------------------------------------

// Whether the primary button is held right now, wherever the pointer is
// (None: cannot tell). With the buttons swapped, the primary is the right.
#[cfg(windows)]
pub fn primary_down() -> Option<bool> {
    #[link(name = "user32")]
    extern "system" {
        fn GetAsyncKeyState(key: i32) -> i16;
        fn GetSystemMetrics(index: i32) -> i32;
    }
    // SAFETY: plain user32 calls with no pointers.
    unsafe {
        let key = if GetSystemMetrics(23) != 0 { 0x02 } else { 0x01 };
        Some(GetAsyncKeyState(key) as u16 & 0x8000 != 0)
    }
}

#[cfg(not(windows))]
pub fn primary_down() -> Option<bool> {
    None
}

// One look at the mouse (cursor in physical px): the window's mode, the
// page's pointer, her eyes, a drag. Returns ms until the next look.
pub fn poll(sh: &Arc<Shared>, agent: &str, cursor: (i32, i32)) -> u64 {
    // No window: nothing to look at, nor to wait for.
    if !sh.pets.of(agent).lock().unwrap().made {
        return u64::MAX;
    }
    let is_idle = sh.pet.lock().unwrap().mood_for(agent, now_ms()) == "idle";
    let look = sh.flag("look");
    let scale = scale(sh);
    let down = primary_down();

    let mut events: Vec<(&str, Value)> = Vec::new();
    let mut moved_to = None;
    let mut ignore = None;
    let mut reach = None;
    let mut let_go_now = false;
    let mut press_now = false;
    let wait = {
        let mut w = sh.pets.of(agent).lock().unwrap();
        let sf = w.sf();
        if !w.ready {
            200
        } else if !w.shown {
            ignore = w.ct.force(Mode::Pass);
            w.watching = false;
            500
        } else if w.flying {
            16
        } else if let Some(carried) = w.drag.as_ref().map(|d| d.carried) {
            // Carried, the button is the island's: she takes no mouse at all,
            // so she never sees half of a press.
            ignore = w.ct.force(if carried { Mode::Pass } else { Mode::Catch });
            // Follow the cursor from here, so a flick that outruns the page's
            // own mouse events still carries her; the button up anywhere ends it.
            if down == Some(false) {
                let_go_now = true;
            } else {
                let d = w.drag.as_mut().unwrap();
                let (nx, ny) = (cursor.0 - d.dx, cursor.1 - d.dy);
                if (nx, ny) != (d.x, d.y) {
                    d.moved = d.moved || (nx - d.x0).abs() + (ny - d.y0).abs() > (4.0 * sf) as i32;
                    let moved = d.moved;
                    if moved {
                        events.push(("pet:drag", json!(nx - d.x)));
                    }
                    d.x = nx;
                    d.y = ny;
                    w.pos = (nx, ny);
                    moved_to = Some((nx, ny));
                    if moved {
                        reach = Some(w.her_point(scale));
                    }
                }
            }
            16
        } else {
            let (pos, size) = (w.pos, w.size);
            let seen = w.ct.look(cursor, pos, size, sf, false, false);
            if w.ct.calmed() {
                sh.log(&format!("pet: mouse mode bouncing, calmed: cursor {cursor:?}, window {pos:?} {size:?}, scale {sf}"));
            }
            ignore = seen.ignore;
            if let Some(at) = seen.pointer {
                events.push(("pet:pointer", at));
            }
            // After she has been in the island and out again, Windows can eat
            // the button-down on her window (the page sees only the button-up).
            // So while the pointer is on her, the button itself is watched too,
            // and a press starts the drag, whichever hears it first.
            let is_down = down == Some(true);
            // Not with a prompt panel up: a press there is on its buttons.
            if w.ct.over && w.panel.is_none() {
                if w.watching && is_down && !w.was_down {
                    press_now = true;
                }
                w.watching = true;
            } else {
                w.watching = false;
            }
            w.was_down = is_down;
            // Her eyes follow only while idle.
            let mut is_looking = false;
            if seen.moved && look && is_idle && w.panel.is_none() {
                let face = (pos.0 as f64 + size.0 as f64 / 2.0 + w.shift as f64, w.bottom(scale) as f64 - CELL_H * scale * sf * 0.62);
                let (dx, dy) = ((cursor.0 as f64 - face.0) / sf, (cursor.1 as f64 - face.1) / sf);
                if dx.hypot(dy) <= LOOK_FAR {
                    events.push(("pet:cursor", json!({ "dx": dx, "dy": dy })));
                    is_looking = true;
                }
            }
            if w.ct.over { 16 } else if seen.forward { 30 } else if seen.near || is_looking { 80 } else { 200 }
        }
    };

    set_ignore(sh, agent, ignore);
    if let Some(p) = moved_to {
        place(sh, agent, p);
    }
    for (name, payload) in events {
        let _ = sh.app.emit_to(label(&agent), name, payload);
    }
    if let Some(p) = reach {
        island::reach(sh, Some(p), false);
    }
    if press_now {
        sh.log("pet: press seen from the button");
        drag_start(sh, agent);
    }
    if let_go_now {
        let_go(sh, agent);
    }
    wait
}

// Being dragged from her own window (not carried out of the island).
// Any of them.
pub fn is_dragged(sh: &Shared) -> bool {
    crate::data::AGENTS.iter().any(|agent| sh.pets.of(agent).lock().unwrap().drag.as_ref().is_some_and(|d| !d.carried))
}

pub fn hover(sh: &Shared, agent: &str, is_over: bool) {
    let ignore = {
        let mut w = sh.pets.of(agent).lock().unwrap();
        let (held, passive) = match &w.drag {
            Some(d) => (!d.carried, d.carried),
            None => (false, false),
        };
        w.ct.set_over(is_over, held, passive)
    };
    set_ignore(sh, agent, ignore);
    if is_over {
        let shown = {
            let now = sh.pet.lock().unwrap().get_for(agent, now_ms());
            let id = now["id"].as_str().unwrap_or("");
            (now["mood"] != "idle" && now["jump"] == true && !id.is_empty()).then(|| id.to_string())
        };
        if let Some(id) = shown {
            sh.pets.of(agent).lock().unwrap().click_goes = Some((id, Instant::now()));
        }
    }
    // The pointer on her: you have seen what she had to say.
    if is_over && sh.pet.lock().unwrap().seen_for(agent, now_ms(), sh.hold()) {
        sh.redraw();
    }
}

pub fn drag_start(sh: &Shared, agent: &str) {
    let Some((cx, cy)) = crate::screen::cursor(sh, sf(sh, agent)) else { return };
    let ignore = {
        let mut w = sh.pets.of(agent).lock().unwrap();
        // Already started from the button: the same press.
        if w.drag.is_some() || !w.shown || w.flying {
            return;
        }
        w.walk_gen += 1;
        w.walking = false;
        let (x, y) = w.pos;
        w.drag = Some(Drag { dx: cx - x, dy: cy - y, x, y, x0: x, y0: y, moved: false, carried: false });
        w.ct.force(Mode::Catch)
    };
    sh.log("pet: drag start");
    set_ignore(sh, agent, ignore);
}

// Let go: back into the island if she is close enough to it, else she lands
// where she is, kept on screen. A click makes her jump, and goes to the
// window of the session she shows; two, see double_click.
pub fn let_go(sh: &Arc<Shared>, agent: &str) {
    let scale = scale(sh);
    let (drag, is_double, her) = {
        let mut w = sh.pets.of(agent).lock().unwrap();
        let Some(drag) = w.drag.take() else { return };
        let is_double = !drag.moved && !drag.carried && w.last_click.is_some_and(|t| t.elapsed() < Duration::from_millis(DOUBLE_MS));
        if !drag.moved && !drag.carried {
            w.last_click = if is_double { None } else { Some(Instant::now()) };
        }
        w.was_down = false;
        (drag, is_double, w.her_point(scale))
    };
    sh.log(&format!("pet: let go, {}{}", if drag.moved { "moved" } else { "a click" }, if drag.carried { " (carried)" } else { "" }));
    let _ = sh.app.emit_to(label(&agent), "pet:drag-end", ());
    if drag.carried {
        island::end_hold(sh);
    }
    if drag.moved {
        // Not into a notch's out-of-sight home by a drag (notch::in_notch).
        if !crate::notch::in_notch() && island::reach(sh, Some(her), true) == island::Reach::Snap {
            return crate::absorb(sh, agent, her);
        }
        island::reach(sh, None, false);
    }
    move_to(sh, agent, drag.x, drag.y, false);
    remember(sh, agent);
    if is_double {
        return double_click(sh, agent);
    }
    // Set down on another: aside.
    if drag.moved || drag.carried {
        make_room(sh, agent);
    }
    // A click: a jump. Carried out of the island: she lands with one.
    if !drag.moved || drag.carried {
        let _ = sh.app.emit_to(label(agent), "pet:react", json!({ "react": "jump" }));
    }
    // A click, and no second one after it: to the window of the session she
    // was showing (jump.rs).
    if !drag.moved && !drag.carried {
        let sh = sh.clone();
        let agent = agent.to_string();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(DOUBLE_MS + 20));
            let goes = {
                let mut w = sh.pets.of(&agent).lock().unwrap();
                let is_single = w.last_click.is_some_and(|t| t.elapsed() >= Duration::from_millis(DOUBLE_MS));
                match w.click_goes.take() {
                    Some((id, at)) if is_single && at.elapsed() < Duration::from_millis(CLICK_GOES_MS) => Some(id),
                    other => {
                        w.click_goes = other;
                        None
                    }
                }
            };
            if let Some(id) = goes {
                crate::jump_to(&sh, &id);
            }
        });
    }
}

// A double-click: she flies back into her home (else the settings, grown
// out of a home risen for them).
pub fn double_click(sh: &Arc<Shared>, agent: &str) {
    if sh.is_out(agent) && crate::notch::in_notch() {
        // Her home in a Mac's notch is out of sight: she stays, and the
        // island drops open from it to show what is going on (island.js peek):
        // open, not toggled, so a second double-click does not shut it again.
        sh.log("pet: double-click; the island drops open from the notch");
        let _ = sh.app.emit_to("island", "island:peek", serde_json::json!({ "open": true }));
    } else if sh.is_out(agent) {
        fly_home(sh, agent);
    } else {
        island::open_settings(sh, None);
    }
}

// Back to her home from wherever she is: drawn in faster and faster,
// running that way, her home reaching out for her; then it takes her in.
// Her spot on the desktop stays where it was, for when she is let out again.
fn fly_home(sh: &Arc<Shared>, agent: &str) {
    let Some(end) = island::landing(sh) else { return };
    let scale = scale(sh);
    let (start, offset, end) = {
        let mut w = sh.pets.of(agent).lock().unwrap();
        if w.flying || !w.shown {
            return;
        }
        w.flying = true;
        w.drag = None;
        w.walk_gen += 1;
        w.walking = false;
        let start = w.her_point(scale);
        (start, (start.0 - w.pos.0, start.1 - w.pos.1), end)
    };
    let dir = if end.0 == start.0 { 1 } else { end.0 - start.0 };
    let sh = sh.clone();
    let agent = agent.to_string();
    std::thread::spawn(move || {
        let started = Instant::now();
        let mut p;
        loop {
            let t = (started.elapsed().as_secs_f64() * 1000.0 / FLY_MS).min(1.0);
            let k = t * t;
            p = (start.0 + ((end.0 - start.0) as f64 * k) as i32, start.1 + ((end.1 - start.1) as f64 * k) as i32);
            move_to(&sh, &agent, p.0 - offset.0, p.1 - offset.1, true);
            let _ = sh.app.emit_to(label(&agent), "pet:drag", dir);
            island::reach(&sh, Some(p), false);
            if t >= 1.0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(16));
        }
        sh.pets.of(&agent).lock().unwrap().flying = false;
        let _ = sh.app.emit_to(label(&agent), "pet:drag-end", ());
        crate::absorb(&sh, &agent, p);
    });
}

// Pulled out of the island, the button still held on the island: up she
// comes under the cursor, her middle where the drop had it (offset, logical
// px from the cursor), and follows it.
pub fn carry(sh: &Arc<Shared>, agent: &str, offset: &Value) {
    // Its window made now if it has none: shown under the cursor once its
    // page is up (ready), carried meanwhile.
    if !is_made(sh, agent) {
        if let Err(e) = create(sh, agent) {
            sh.log(&format!("pet: {agent}'s window not made: {e}"));
            return;
        }
    }
    let Some(cursor) = crate::screen::cursor(sh, sf(sh, agent)) else { return };
    let scale = scale(sh);
    let p = {
        let mut w = sh.pets.of(agent).lock().unwrap();
        let sf = w.sf();
        let dx = offset["dx"].as_f64().unwrap_or(0.0) * sf;
        let dy = offset["dy"].as_f64().unwrap_or(0.0) * sf;
        let x = cursor.0 + dx as i32 - w.size.0 / 2;
        let y = cursor.1 + dy as i32 - (w.size.1 - (CELL_H * scale * sf / 2.0) as i32);
        w.pos = (x, y);
        w.walk_gen += 1;
        w.walking = false;
        w.drag = Some(Drag { dx: cursor.0 - x, dy: cursor.1 - y, x, y, x0: x, y0: y, moved: true, carried: true });
        w.ct.reset(true);
        (x, y)
    };
    sh.log(&format!("pet: carried out of the island at {p:?}"));
    place(sh, agent, p);
    set_ignore(sh, agent, Some(true));
    crate::set_out_now(sh, agent);
}

// Gone into the island at once, before the island's animation starts.
pub fn hide_now(sh: &Shared, agent: &str) {
    {
        let mut w = sh.pets.of(agent).lock().unwrap();
        w.drag = None;
        w.walk_gen += 1;
        w.walking = false;
        w.shown = false;
        w.ct.reset(true);
    }
    set_ignore(sh, agent, Some(true));
    if let Some(win) = window(sh, agent) {
        hide_window(&win);
    }
}

// --- Walks ------------------------------------------------------------------------------

// A short walk: move the window by dx (logical) over ms, stopping at the
// screen edge. True when it got there, false when stopped on the way.
pub fn walk(sh: &Shared, agent: &str, dx: f64, ms: f64) -> bool {
    walk_as(sh, agent, dx, ms, false)
}

// A walk; pushed aside (make_room), her page shown her running (pet:drag,
// as she flies home), her own walks being her page's.
fn walk_as(sh: &Shared, agent: &str, dx: f64, ms: f64, shown_running: bool) -> bool {
    if !is_shown(sh, agent) {
        return false;
    }
    let screens = sh.screens();
    let scale = scale(sh);
    let Some(me) = feet(sh, agent, &screens, false) else { return false };
    let room = room(scale, me.sf);
    // Where she would get to: on the screen, and short of the others in her
    // way. Pushed aside, not those she is on: she is getting off of them.
    let on: Vec<&str> = if shown_running { others_by(sh, agent, &me, &screens, false).into_iter().filter(|(_, x)| (x - me.x).abs() < room).map(|(a, _)| a).collect() } else { Vec::new() };
    let in_way = |sh: &Shared| xs(&others_by(sh, agent, &me, &screens, false).into_iter().filter(|(a, _)| !on.contains(a)).collect::<Vec<_>>());
    let others = in_way(sh);
    let (gen, x0, y0, target) = {
        let mut w = sh.pets.of(agent).lock().unwrap();
        if !w.shown || w.drag.is_some() || w.flying || w.panel.is_some() {
            return false;
        }
        let (x0, y0) = w.pos;
        let target = screens.clamp((x0 + (dx * w.sf()).round() as i32, y0), w.size).0;
        let target = x0 + clip_walk(me.x, me.x + (target - x0), &others, room) - me.x;
        if target == x0 {
            w.heading = None;
            return false;
        }
        w.walk_gen += 1;
        w.walking = true;
        w.heading = Some(me.x + (target - x0));
        (w.walk_gen, x0, y0, target)
    };
    let dir = (target - x0).signum();
    let started = Instant::now();
    let ms = ms.max(1.0);
    let arrived = loop {
        std::thread::sleep(Duration::from_millis(16));
        let t = (started.elapsed().as_secs_f64() * 1000.0 / ms).min(1.0);
        // The others as they are now (they walk too): one in her way, she stops.
        let others = in_way(sh);
        let p = {
            let mut w = sh.pets.of(agent).lock().unwrap();
            if w.walk_gen != gen || w.drag.is_some() || w.flying {
                break false;
            }
            let p = screens.clamp((x0 + ((target - x0) as f64 * t).round() as i32, y0), w.size);
            let (from, to) = (w.pos.0 + w.size.0 / 2 + w.shift, p.0 + w.size.0 / 2 + w.shift);
            if clip_walk(from, to, &others, room) != to {
                break false;
            }
            w.pos = p;
            p
        };
        place(sh, agent, p);
        if shown_running {
            let _ = sh.app.emit_to(label(agent), "pet:drag", dir);
        }
        if t >= 1.0 {
            break true;
        }
    };
    {
        let mut w = sh.pets.of(agent).lock().unwrap();
        if w.walk_gen == gen {
            w.walking = false;
            w.heading = None;
        }
    }
    if shown_running {
        let _ = sh.app.emit_to(label(agent), "pet:drag-end", ());
    }
    remember(sh, agent);
    arrived
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_walk_stops_short_of_another_in_her_way() {
        // Room 126 (her width at 0.55, 20 apart). Another at 1000.
        assert_eq!(room(0.55, 1.0), 126);
        // Walking right towards it: as far as 874.
        assert_eq!(clip_walk(700, 950, &[1000], 126), 874);
        // Not that far: all the way.
        assert_eq!(clip_walk(700, 800, &[1000], 126), 800);
        // Leftwards, one behind her (to the right) is no matter.
        assert_eq!(clip_walk(700, 500, &[1000], 126), 500);
        // Already too near: walking into it, she stays; away, free.
        assert_eq!(clip_walk(950, 990, &[1000], 126), 950);
        assert_eq!(clip_walk(950, 800, &[1000], 126), 800);
        // Two in the way: the nearer stops her.
        assert_eq!(clip_walk(500, 1500, &[1300, 900], 126), 774);
        assert_eq!(clip_walk(1500, 0, &[1300, 900], 126), 1426);
    }

    #[test]
    fn set_down_on_another_she_goes_aside_leftwards_first() {
        let bounds = (60, 2500);
        // Clear: where she is.
        assert_eq!(free_spot(500, &[1000], 126, bounds), Some(500));
        // On one, either side of it: leftwards, room away from it.
        assert_eq!(free_spot(950, &[1000], 126, bounds), Some(874));
        assert_eq!(free_spot(1050, &[1000], 126, bounds), Some(874));
        // No room left of it (the screen's edge): to the right.
        assert_eq!(free_spot(100, &[120], 126, bounds), Some(246));
        // On two at once: left of both.
        assert_eq!(free_spot(1050, &[1100, 980], 126, bounds), Some(854));
        assert_eq!(free_spot(1020, &[1100, 1060], 126, bounds), Some(934));
        // On one, and another clear of her to the left, too near it for her
        // between them: not past that one, so to the right.
        assert_eq!(free_spot(500, &[550, 300], 126, bounds), Some(676));
        // Nowhere on the screen.
        assert_eq!(free_spot(100, &[80, 200], 126, (60, 210)), None);
    }
}

pub fn stop_walking(sh: &Shared, agent: &str) {
    let mut w = sh.pets.of(agent).lock().unwrap();
    w.walk_gen += 1;
    w.walking = false;
}

// Where the window is, for checks.
pub fn where_(sh: &Shared, agent: &str) -> Value {
    let actual = window(sh, agent).and_then(|win| win.outer_position().ok()).map(|p| json!([p.x, p.y]));
    let w = sh.pets.of(agent).lock().unwrap();
    json!({
        "actual": actual,
        "pos": [w.pos.0, w.pos.1],
        "size": [w.size.0, w.size.1],
        "scaleFactor": w.sf(),
        "made": w.made,
        "below": w.below,
        "panel": w.panel.map(|(pw, ph)| json!([pw, ph])),
        "shown": w.shown,
        "over": w.ct.over,
        "walking": w.walking,
        "flying": w.flying,
        "dragging": w.drag.as_ref().map(|d| if d.carried { "carried" } else { "dragged" }),
    })
}
