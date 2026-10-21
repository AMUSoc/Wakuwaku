// Yu-Gi-Oh! Master Duel's helper: on a key (duelKey, Alt+D unless set),
// while the game runs, she takes a picture of its window and asks Claude
// what the opponent plays and where to stop it: Claude reads the picture
// (the cards on the field, in the graveyard, the one opened on the left),
// names the deck, looks it up on the web when unsure, and says which
// effect to stop with which hand trap or card, and when. Its answer in a
// panel by the game's right edge (src/duel/); the key again, or a click on
// ✕, puts it away. Seen 2026-10-22: a field of three Snake-Eye cards named
// right in 26 seconds.
//
// Only the screen is read, as a person reads it: nothing of the game's own
// memory, nothing pressed for the person (Konami's terms forbid tools that
// play or read the game). A guess of the last 20 minutes goes with the next
// picture, which refines it; the person's own hand traps (duelHand) are
// what the advice is made of when set.
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::utils::config::BackgroundThrottlingPolicy;
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::Shared;

pub const LABEL: &str = "duel";
pub const DEFAULT_KEY: &str = "Alt+D";
const GAME: &str = "masterduel.exe";

// The game's process: Master Duel's, or for a test another's (WAKUWAKU_DUEL_EXE).
fn game_pid() -> Option<u32> {
    crate::lol::pid_of(&std::env::var("WAKUWAKU_DUEL_EXE").unwrap_or_else(|_| GAME.to_string()))
}
// The panel, in logical pixels, and its gap to the display's edge.
const SIZE: (f64, f64) = (440.0, 660.0);
const MARGIN: f64 = 24.0;
// The picture sent: so wide at most (a 2560 display halved).
const MAX_WIDTH: usize = 1600;
const TIMEOUT: Duration = Duration::from_secs(150);
// A guess this recent goes with the next picture.
const GUESS_MS: u64 = 20 * 60_000;

static VIEW: Mutex<Value> = Mutex::new(Value::Null);
static SHOWN: AtomicBool = AtomicBool::new(false);
static READY: AtomicBool = AtomicBool::new(false);
static CREATED: AtomicBool = AtomicBool::new(false);
static LOOKING: AtomicBool = AtomicBool::new(false);
// The last answer and when: the next picture is asked with it.
static LAST: Mutex<Option<(Value, u64)>> = Mutex::new(None);

pub fn key_name(sh: &Shared) -> String {
    sh.setting("duelKey").as_str().filter(|k| crate::scout::parse_key(k).is_some()).unwrap_or(DEFAULT_KEY).to_string()
}

fn dir(sh: &Shared) -> PathBuf {
    sh.dir.join("duel")
}

// Its key pressed: away if up; else, with the game running, a look.
pub fn press(sh: &Arc<Shared>) {
    if SHOWN.load(Ordering::SeqCst) {
        show(sh, false);
        return;
    }
    if sh.setting("duel") == false {
        return;
    }
    // Only with the game up: the key is someone else's elsewhere (Alt+D in a browser).
    let Some(pid) = game_pid() else { return };
    if LOOKING.load(Ordering::SeqCst) {
        show(sh, true);
        return;
    }
    look(sh, pid);
}

// Again, from its button: the panel out of the way of the picture first.
pub fn again(sh: &Arc<Shared>) {
    let Some(pid) = game_pid() else {
        publish(sh, json!({ "state": "error", "why": "noGame" }));
        return;
    };
    if LOOKING.load(Ordering::SeqCst) {
        return;
    }
    if let Some(win) = window(sh) {
        let _ = win.hide();
    }
    std::thread::sleep(Duration::from_millis(250));
    look(sh, pid);
}

fn look(sh: &Arc<Shared>, pid: u32) {
    let Some(rect) = os::window_rect(pid) else {
        sh.log("duel: the game's window not found");
        return;
    };
    LOOKING.store(true, Ordering::SeqCst);
    let picture = dir(sh).join("work").join("shot.png");
    let taken = std::fs::create_dir_all(dir(sh).join("work")).is_ok() && os::capture(rect).is_some_and(|(w, h, rgb)| save_png(&picture, w, h, &rgb).is_ok());
    if !taken {
        LOOKING.store(false, Ordering::SeqCst);
        sh.log("duel: the game's window could not be pictured");
        publish(sh, json!({ "state": "error", "why": "picture" }));
        show(sh, true);
        return;
    }
    let started = crate::now_ms();
    publish(sh, json!({ "state": "looking", "since": started, "last": last_guess() }));
    show(sh, true);
    let sh = sh.clone();
    crate::spawn_named("duel", move || {
        let prompt = prompt_of(&hand_of(&sh), last_guess().as_ref());
        let answer = run(&sh, &prompt).and_then(|text| crate::opgg::json_in(&text).ok_or_else(|| text.chars().take(120).collect()));
        let took = crate::now_ms().saturating_sub(started);
        match answer {
            Ok(found) => {
                sh.log(&format!("duel: {} in {} s", found["deck"].as_str().unwrap_or("?"), took / 1000));
                *LAST.lock().unwrap_or_else(|e| e.into_inner()) = Some((found.clone(), crate::now_ms()));
                publish(&sh, json!({ "state": "ready", "found": found, "took": took, "at": crate::now_ms() }));
            }
            Err(why) => {
                sh.log(&format!("duel: no answer ({why})"));
                publish(&sh, json!({ "state": "error", "why": why, "last": last_guess() }));
            }
        }
        LOOKING.store(false, Ordering::SeqCst);
    });
}

fn last_guess() -> Option<Value> {
    LAST.lock().unwrap_or_else(|e| e.into_inner()).as_ref().filter(|(_, at)| crate::now_ms().saturating_sub(*at) < GUESS_MS).map(|(v, _)| v.clone())
}

fn hand_of(sh: &Shared) -> String {
    sh.setting("duelHand").as_str().unwrap_or("").trim().chars().take(300).collect()
}

pub fn prompt_of(hand: &str, last: Option<&Value>) -> String {
    let mut p = String::from(
        "你是遊戲王 Master Duel 的對戰助手。先用 Read 讀這張遊戲截圖：shot.png（在目前資料夾）。\n\n\
         圖裡是對戰畫面：對手的卡在畫面上方（場上、墓地），左邊可能打開著某張卡的說明。請：\n\
         1. 認出看得到的對手的卡（卡名用台灣 Master Duel 的繁體中文官方譯名；不確定就寫日文或英文名）。\n\
         2. 判斷對手是什麼牌組（主題）。不確定時用 WebSearch 查目前的環境和這個牌組的打法（日文、英文的資料都可以）。\n\
         3. 告訴我要斷哪裡：這個牌組的關鍵展開點，用手坑或魔法陷阱該斷哪張卡的哪個效果、什麼時機。\n",
    );
    if !hand.is_empty() {
        p.push_str(&format!("\n我的牌組帶的手坑／斷點卡：{hand}。建議時優先用這些，沒有合適的才提別的。\n"));
    }
    if let Some(last) = last {
        p.push_str(&format!(
            "\n前一張截圖判斷對手可能是「{}」（看到：{}）。這張是同一場的新畫面：一起考慮，更新判斷。\n",
            last["deck"].as_str().unwrap_or("?"),
            last["seen"].as_array().map(|s| s.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("、")).unwrap_or_default()
        ));
    }
    p.push_str(
        "\n只回一個 JSON，不要其他文字：\n\
         {\"deck\":\"牌組名（繁中）\",\"deckAlt\":\"日文或英文名\",\"sure\":\"高|中|低\",\"seen\":[\"看到的卡名\"],\"plan\":\"它想做什麼，一句話\",\
         \"stop\":[{\"card\":\"要斷的卡\",\"effect\":\"哪個效果\",\"with\":\"用什麼斷\",\"when\":\"時機\",\"why\":\"為什麼，20字內\"}],\
         \"watch\":[\"要小心它的什麼，15字內\"],\"tip\":\"一句建議\"}\n\
         stop 最多 3 個，照重要程度排；watch 最多 3 個。看不出對手的卡（不是對戰畫面）時 deck 寫空字串、plan 說看到了什麼。全部繁體中文、台灣用語。\n",
    );
    p
}

fn run(sh: &Shared, prompt: &str) -> Result<String, String> {
    let folder = dir(sh).join("work");
    let prompt_file = folder.join("prompt.txt");
    std::fs::write(&prompt_file, prompt).map_err(|e| e.to_string())?;
    let exe = crate::run::find("claude").ok_or("claude not found")?;
    let mut cmd = std::process::Command::new(&exe);
    // The words on its input, never on a command line (claude is a .cmd shim).
    cmd.args(["-p", "--allowedTools", "Read", "WebSearch", "WebFetch"])
        .current_dir(&folder)
        .stdin(std::fs::File::open(&prompt_file).map_err(|e| e.to_string())?)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    for mark in crate::run::SESSION_MARKS {
        cmd.env_remove(mark);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let mut out = child.stdout.take().ok_or("no output")?;
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = out.read_to_string(&mut text);
        text
    });
    let started = Instant::now();
    let ok = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Ok(None) if started.elapsed() > TIMEOUT => {
                let _ = child.kill();
                return Err("timeout".into());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(300)),
            Err(e) => return Err(e.to_string()),
        }
    };
    let text = reader.join().unwrap_or_default().trim().to_string();
    if !ok || text.is_empty() {
        return Err(text.lines().next().unwrap_or("no answer").chars().take(120).collect());
    }
    Ok(text)
}

// --- The picture ---------------------------------------------------------------------

// Its pixels (RGB, rows top down) as a PNG, made smaller to MAX_WIDTH by
// averaging squares of pixels.
fn save_png(path: &std::path::Path, w: usize, h: usize, rgb: &[u8]) -> Result<(), String> {
    let (w2, h2, small) = shrink(w, h, rgb, MAX_WIDTH);
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w2 as u32, h2 as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&small).map_err(|e| e.to_string())
}

pub fn shrink(w: usize, h: usize, rgb: &[u8], most: usize) -> (usize, usize, Vec<u8>) {
    let step = w.div_ceil(most).max(1);
    if step == 1 {
        return (w, h, rgb.to_vec());
    }
    let (w2, h2) = (w / step, h / step);
    let mut out = vec![0u8; w2 * h2 * 3];
    for y in 0..h2 {
        for x in 0..w2 {
            let mut sum = [0u32; 3];
            for dy in 0..step {
                for dx in 0..step {
                    let at = ((y * step + dy) * w + x * step + dx) * 3;
                    for c in 0..3 {
                        sum[c] += rgb[at + c] as u32;
                    }
                }
            }
            for c in 0..3 {
                out[(y * w2 + x) * 3 + c] = (sum[c] / (step * step) as u32) as u8;
            }
        }
    }
    (w2, h2, out)
}

#[cfg(windows)]
mod os {
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    pub struct Rect {
        pub l: i32,
        pub t: i32,
        pub r: i32,
        pub b: i32,
    }

    #[repr(C)]
    struct BitmapInfoHeader {
        size: u32,
        width: i32,
        height: i32,
        planes: u16,
        bit_count: u16,
        compression: u32,
        size_image: u32,
        x_ppm: i32,
        y_ppm: i32,
        clr_used: u32,
        clr_important: u32,
    }

    type EnumProc = unsafe extern "system" fn(isize, isize) -> i32;
    // scout.rs, island.rs and others declare some of these with their own types: the same ABI.
    #[allow(clashing_extern_declarations)]
    #[link(name = "user32")]
    extern "system" {
        fn EnumWindows(f: EnumProc, l: isize) -> i32;
        fn GetWindowThreadProcessId(hwnd: isize, pid: *mut u32) -> u32;
        fn IsWindowVisible(hwnd: isize) -> i32;
        fn GetWindowRect(hwnd: isize, rect: *mut Rect) -> i32;
        fn GetDC(hwnd: isize) -> isize;
        fn ReleaseDC(hwnd: isize, dc: isize) -> i32;
    }
    #[allow(clashing_extern_declarations)]
    #[link(name = "gdi32")]
    extern "system" {
        fn CreateCompatibleDC(dc: isize) -> isize;
        fn CreateCompatibleBitmap(dc: isize, w: i32, h: i32) -> isize;
        fn SelectObject(dc: isize, obj: isize) -> isize;
        fn BitBlt(dst: isize, x: i32, y: i32, w: i32, h: i32, src: isize, sx: i32, sy: i32, rop: u32) -> i32;
        fn GetDIBits(dc: isize, bmp: isize, start: u32, lines: u32, bits: *mut u8, info: *mut BitmapInfoHeader, usage: u32) -> i32;
        fn DeleteObject(obj: isize) -> i32;
        fn DeleteDC(dc: isize) -> i32;
    }

    // The game's largest window shown: where it is on the screen (physical).
    pub fn window_rect(pid: u32) -> Option<Rect> {
        struct Search {
            pid: u32,
            best: Option<(i64, Rect)>,
        }
        unsafe extern "system" fn each(hwnd: isize, l: isize) -> i32 {
            // SAFETY: l is the Search below, alive for the enumeration.
            let search = unsafe { &mut *(l as *mut Search) };
            let mut pid = 0;
            let mut r = Rect::default();
            // SAFETY: reads of a window's process and place into our own values.
            unsafe {
                GetWindowThreadProcessId(hwnd, &mut pid);
                if pid == search.pid && IsWindowVisible(hwnd) != 0 && GetWindowRect(hwnd, &mut r) != 0 {
                    let size = (r.r - r.l) as i64 * (r.b - r.t) as i64;
                    if size > 0 && search.best.is_none_or(|(s, _)| size > s) {
                        search.best = Some((size, r));
                    }
                }
            }
            1
        }
        let mut search = Search { pid, best: None };
        // SAFETY: the callback is given our Search, which outlives the call.
        unsafe { EnumWindows(each, &mut search as *mut Search as isize) };
        search.best.map(|(_, r)| r)
    }

    // What the screen shows there (the game as the person sees it), RGB.
    pub fn capture(r: Rect) -> Option<(usize, usize, Vec<u8>)> {
        let (w, h) = (r.r - r.l, r.b - r.t);
        if w <= 0 || h <= 0 {
            return None;
        }
        const SRCCOPY: u32 = 0x00CC_0020;
        const CAPTUREBLT: u32 = 0x4000_0000;
        let mut bgra = vec![0u8; w as usize * h as usize * 4];
        // SAFETY: device contexts and a bitmap of ours, all let go below;
        // the pixels written into our own buffer of the size asked for.
        let ok = unsafe {
            let screen = GetDC(0);
            let mem = CreateCompatibleDC(screen);
            let bmp = CreateCompatibleBitmap(screen, w, h);
            let old = SelectObject(mem, bmp);
            let copied = BitBlt(mem, 0, 0, w, h, screen, r.l, r.t, SRCCOPY | CAPTUREBLT) != 0;
            let mut info = BitmapInfoHeader { size: std::mem::size_of::<BitmapInfoHeader>() as u32, width: w, height: -h, planes: 1, bit_count: 32, compression: 0, size_image: 0, x_ppm: 0, y_ppm: 0, clr_used: 0, clr_important: 0 };
            let lines = GetDIBits(mem, bmp, 0, h as u32, bgra.as_mut_ptr(), &mut info, 0);
            SelectObject(mem, old);
            DeleteObject(bmp);
            DeleteDC(mem);
            ReleaseDC(0, screen);
            copied && lines == h
        };
        if !ok {
            return None;
        }
        let rgb = bgra.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0]]).collect();
        Some((w as usize, h as usize, rgb))
    }
}

#[cfg(not(windows))]
mod os {
    #[derive(Default, Clone, Copy)]
    pub struct Rect {
        pub l: i32,
        pub t: i32,
        pub r: i32,
        pub b: i32,
    }
    pub fn window_rect(_pid: u32) -> Option<Rect> {
        None
    }
    pub fn capture(_r: Rect) -> Option<(usize, usize, Vec<u8>)> {
        None
    }
}

// --- The panel -------------------------------------------------------------------------

fn window(sh: &Shared) -> Option<WebviewWindow> {
    sh.app.get_webview_window(LABEL)
}

fn publish(sh: &Shared, view: Value) {
    *VIEW.lock().unwrap_or_else(|e| e.into_inner()) = view;
    push(sh);
}

fn push(sh: &Shared) {
    if READY.load(Ordering::SeqCst) {
        let mut view = VIEW.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if view.is_null() {
            view = json!({ "state": "none" });
        }
        view["lang"] = json!(sh.lang());
        view["key"] = json!(key_name(sh));
        view["hand"] = json!(hand_of(sh));
        let _ = sh.app.emit_to(LABEL, "duel:update", view);
    }
}

// By the right edge of the display the game is on, in the middle of its height.
fn rect(sh: &Shared) -> Option<((i32, i32), (i32, i32), f64)> {
    let game = game_pid().and_then(os::window_rect);
    let a = game.and_then(|r| sh.screens().near((r.l + r.r) / 2, (r.t + r.b) / 2)).or_else(|| crate::island::area(sh))?;
    let w = ((SIZE.0 * a.sf).round() as i32).min(a.w);
    let h = ((SIZE.1 * a.sf).round() as i32).min(a.h - (2.0 * MARGIN * a.sf) as i32);
    let margin = (MARGIN * a.sf).round() as i32;
    Some(((a.x + a.w - w - margin, a.y + (a.h - h) / 2), (w, h), a.sf))
}

fn lay(win: &WebviewWindow, (x, y): (i32, i32), (w, h): (i32, i32)) {
    let _ = win.set_position(PhysicalPosition::new(x, y));
    let _ = win.set_size(PhysicalSize::new(w.max(1) as u32, h.max(1) as u32));
    let _ = win.set_position(PhysicalPosition::new(x, y));
}

fn create(sh: &Arc<Shared>) {
    if CREATED.swap(true, Ordering::SeqCst) {
        return;
    }
    let Some((pos, size, sf)) = rect(sh) else {
        CREATED.store(false, Ordering::SeqCst);
        return;
    };
    let sh = sh.clone();
    std::thread::spawn(move || {
        let built = WebviewWindowBuilder::new(&sh.app, LABEL, WebviewUrl::App("duel/index.html".into()))
            .title("wakuwaku duel")
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
            .focusable(false)
            .focused(false)
            .visible(false)
            .background_throttling(BackgroundThrottlingPolicy::Disabled)
            .build();
        match built {
            Ok(win) => {
                lay(&win, pos, size);
                let _ = win.set_ignore_cursor_events(false);
                sh.own_window(&win);
            }
            Err(err) => sh.log(&format!("duel: could not make its window: {err}")),
        }
    });
}

// Its page is up: drawn, and shown if it was asked for meanwhile.
pub fn ready(sh: &Arc<Shared>) {
    READY.store(true, Ordering::SeqCst);
    push(sh);
    apply(sh);
}

fn apply(sh: &Arc<Shared>) {
    let want = SHOWN.load(Ordering::SeqCst);
    if want && !CREATED.load(Ordering::SeqCst) {
        create(sh);
        return;
    }
    if !READY.load(Ordering::SeqCst) {
        return;
    }
    if let Some(win) = window(sh) {
        if want {
            if let Some((pos, size, _)) = rect(sh) {
                lay(&win, pos, size);
                // Onto a display of another scale, Windows sizes it again a
                // moment later (as scout.rs found): laid again once it has.
                let again = win.clone();
                std::thread::spawn(move || {
                    for _ in 0..3 {
                        std::thread::sleep(Duration::from_millis(200));
                        let now = again.outer_size().map(|s| (s.width as i32, s.height as i32)).unwrap_or(size);
                        let at = again.outer_position().map(|p| (p.x, p.y)).unwrap_or(pos);
                        if (now, at) != (size, pos) {
                            lay(&again, pos, size);
                        }
                    }
                });
            }
            let _ = win.show();
            push(sh);
        } else {
            let _ = win.hide();
        }
    }
}

pub fn show(sh: &Arc<Shared>, on: bool) {
    SHOWN.store(on, Ordering::SeqCst);
    apply(sh);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_claude_is_asked() {
        let p = prompt_of("", None);
        assert!(p.contains("shot.png") && p.contains("\"stop\"") && !p.contains("我的牌組帶的"));
        let last = json!({ "deck": "蛇眼", "seen": ["蛇眼灰燼", "原罪寶-蛇眼"] });
        let p = prompt_of("灰流麗、增殖的G", Some(&last));
        assert!(p.contains("我的牌組帶的手坑／斷點卡：灰流麗、增殖的G"));
        assert!(p.contains("可能是「蛇眼」（看到：蛇眼灰燼、原罪寶-蛇眼）"));
    }

    #[test]
    fn a_picture_made_smaller_by_averaging() {
        // 4 x 2, two squares of 2 x 2: one dark, one light.
        let px = |v: u8| [v, v, v];
        let rgb: Vec<u8> = [px(0), px(0), px(200), px(100), px(0), px(0), px(200), px(100)].concat();
        let (w, h, small) = shrink(4, 2, &rgb, 2);
        assert_eq!((w, h), (2, 1));
        assert_eq!(small, vec![0, 0, 0, 150, 150, 150]);
        assert_eq!(shrink(4, 2, &rgb, 10).2, rgb);
    }
}
