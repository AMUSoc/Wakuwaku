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
// The frames laid on the game itself (src/duel/frames.html).
pub const FRAMES: &str = "duelframes";
pub const DEFAULT_KEY: &str = "Alt+D";
const GAME: &str = "masterduel.exe";

// The game's process: Master Duel's, or for a test another's (WAKUWAKU_DUEL_EXE).
fn game_pid() -> Option<u32> {
    crate::lol::pid_of(&std::env::var("WAKUWAKU_DUEL_EXE").unwrap_or_else(|_| GAME.to_string()))
}
// The panel, in logical pixels, and its gap to the display's edge.
const SIZE: (f64, f64) = (380.0, 620.0);
// The bar of her tools down the game's right (Alt+D brings it up, 2026-10-22:
// "a small toolbar, not a chat window"); a card of what was asked for slides
// out to its left (FLY), the window as large as both then.
const BAR: (f64, f64) = (64.0, 318.0);
const BAR_GAP: f64 = 8.0;
static FLY: AtomicBool = AtomicBool::new(false);
const MARGIN: f64 = 12.0;
// The picture sent: so wide at most (a 2560 display halved).
const MAX_WIDTH: usize = 1600;
const TIMEOUT: Duration = Duration::from_secs(150);
// Reading a deck card by card, a theme looked up first: longer (it took
// 106 to 363 s, 2026-10-22; once or twice a deck, so waited for).
const DECK_TIMEOUT: Duration = Duration::from_secs(600);
// A guess this recent goes with the next picture.
const GUESS_MS: u64 = 20 * 60_000;

static VIEW: Mutex<Value> = Mutex::new(Value::Null);
static SHOWN: AtomicBool = AtomicBool::new(false);
static READY: AtomicBool = AtomicBool::new(false);
static CREATED: AtomicBool = AtomicBool::new(false);
static LOOKING: AtomicBool = AtomicBool::new(false);
// The last picture of the game, whole (the deck editor's cards are cut out of it).
static FULL: Mutex<Option<crate::deckscan::Image>> = Mutex::new(None);
static SCANNING: Mutex<Option<u64>> = Mutex::new(None);
static FRAMES_MADE: AtomicBool = AtomicBool::new(false);
static FRAMES_READY: AtomicBool = AtomicBool::new(false);
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
    if game_pid().is_none() {
        return;
    }
    // The bar alone; a look is one of its buttons. Still looking: its card out.
    FLY.store(LOOKING.load(Ordering::SeqCst), Ordering::SeqCst);
    show(sh, true);
}

// The card beside the bar out or in (its page says which card).
pub fn fly(sh: &Arc<Shared>, open: bool) {
    FLY.store(open, Ordering::SeqCst);
    if SHOWN.load(Ordering::SeqCst) {
        apply(sh);
    }
}

// The bar's 掃描牌組: the deck editor pictured and read card by card, a new deck.
pub fn scan(sh: &Arc<Shared>) {
    // Not done: the deck window says why.
    let fail = |why: &str| {
        open_deck(sh);
        let mut view = deck_view(sh);
        view["error"] = json!(why);
        let _ = sh.app.emit_to(DECK, "duel:deck", view);
    };
    let Some(pid) = game_pid() else { return fail("noGame") };
    let Some((hwnd, rect)) = os::game_window(pid) else { return fail("noGame") };
    let Some((w, h, rgb)) = os::capture_game(hwnd, rect) else {
        sh.log("duel: the game's window could not be pictured");
        return fail("picture");
    };
    let _ = std::fs::create_dir_all(dir(sh));
    let _ = crate::deckscan::Image { w, h, rgb }.save(&dir(sh).join("full.png"));
    scan_deck(sh, "", Vec::new(), true);
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
    let Some((hwnd, rect)) = os::game_window(pid) else {
        sh.log("duel: the game's window not found");
        return;
    };
    LOOKING.store(true, Ordering::SeqCst);
    let picture = dir(sh).join("work").join("shot.png");
    let taken = std::fs::create_dir_all(dir(sh).join("work")).is_ok()
        && os::capture_game(hwnd, rect).is_some_and(|(w, h, rgb)| {
            let saved = save_png(&picture, w, h, &rgb).is_ok();
            *FULL.lock().unwrap_or_else(|e| e.into_inner()) = Some(crate::deckscan::Image { w, h, rgb });
            saved
        });
    if !taken {
        LOOKING.store(false, Ordering::SeqCst);
        sh.log("duel: the game's window could not be pictured");
        publish(sh, json!({ "state": "error", "why": "picture" }));
        show(sh, true);
        return;
    }
    let started = crate::now_ms();
    keep_shot(sh, &picture, started);
    *ASKS.lock().unwrap_or_else(|e| e.into_inner()) = None;
    publish(sh, json!({ "state": "looking", "since": started, "last": last_guess() }));
    FLY.store(true, Ordering::SeqCst);
    show(sh, true);
    let sh = sh.clone();
    crate::spawn_named("duel", move || {
        let prompt = prompt_of(&hand_of(&sh), &deck_of(&sh), last_guess().as_ref());
        let answer = run(&sh, &prompt, TIMEOUT).and_then(|text| answer_in(&text).ok_or_else(|| text.chars().take(120).collect()));
        let took = crate::now_ms().saturating_sub(started);
        match answer {
            Ok(found) if found["screen"] == "deck" => {
                // The deck editor: its cards cut out and read one by one, in the deck window.
                let full = FULL.lock().unwrap_or_else(|e| e.into_inner()).take();
                if let Some(full) = full {
                    let _ = std::fs::create_dir_all(dir(&sh));
                    let _ = full.save(&dir(&sh).join("full.png"));
                }
                // Not left "looking" for the next time it is opened.
                publish(&sh, json!({ "state": "none" }));
                show(&sh, false);
                LOOKING.store(false, Ordering::SeqCst);
                let theme = crate::deckscan::read_deck(&dir(&sh))["theme"].as_str().unwrap_or("").to_string();
                scan_deck(&sh, &theme, Vec::new(), true);
                return;
            }
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

// Each picture kept too (duel/shots/, the last 60): what the game looks
// like, for laying the helper's frames on its zones and seeing how well
// cards are told apart (2026-10-22).
fn keep_shot(sh: &Shared, picture: &std::path::Path, at: u64) {
    let shots = dir(sh).join("shots");
    if std::fs::create_dir_all(&shots).is_err() || std::fs::copy(picture, shots.join(format!("{at}.png"))).is_err() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(&shots) else { return };
    let mut all: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    all.sort();
    let excess = all.len().saturating_sub(60);
    for old in &all[..excess] {
        let _ = std::fs::remove_file(old);
    }
}

// The answer in what Claude wrote: the first JSON object in it that says
// which screen it saw (or a deck's cards), whatever is written around it (a word before, a note
// after with braces of its own: the first try failed so, 2026-10-22).
pub fn answer_in(text: &str) -> Option<Value> {
    text.match_indices('{').find_map(|(at, _)| {
        let v = serde_json::Deserializer::from_str(&text[at..]).into_iter::<Value>().next()?.ok()?;
        (v.get("screen").is_some() || v.get("deck").is_some() || v.get("cards").is_some()).then_some(v)
    })
}

fn last_guess() -> Option<Value> {
    LAST.lock().unwrap_or_else(|e| e.into_inner()).as_ref().filter(|(_, at)| crate::now_ms().saturating_sub(*at) < GUESS_MS).map(|(v, _)| v.clone())
}

fn deck_of(sh: &Shared) -> String {
    sh.setting("duelDeck").as_str().unwrap_or("").trim().chars().take(1500).collect()
}

fn hand_of(sh: &Shared) -> String {
    sh.setting("duelHand").as_str().unwrap_or("").trim().chars().take(300).collect()
}

pub fn prompt_of(hand: &str, deck: &str, last: Option<&Value>) -> String {
    let mut p = String::from(
        "你是遊戲王 Master Duel 的對戰助手。先用 Read 讀這張遊戲截圖：shot.png（在目前資料夾，就是遊戲畫面本身）。\n\n\
         【如果這是牌組編輯畫面】（上面主要牌組、下面額外牌組的卡片格子）：只回 {\"screen\":\"deck\"}，不用認卡（之後會一張一張切出來再認）。\n\n\
         【如果這是對戰畫面】畫面上方是對手的場地，下方是我的；左邊可能打開著某張卡的說明，中間可能正跳出「是否連鎖」。\n\
         對手的格子這樣叫（照畫面上的左右）：上面那排魔法陷阱區 S1～S5（左到右），下一排怪獸區 M1～M5（左到右），\
         對手怪獸區右邊那格場地區 F，中間兩格額外怪獸區 EL（左）、ER（右）。在手牌、墓地、除外或連鎖中的寫 hand、gy、ban、chain。\n\
         請：\n\
         1. 認出看得到的對手的卡（卡名用台灣 Master Duel 的繁體中文官方譯名；左邊或上方有卡片說明時以說明為準；不確定就寫日文或英文名）。\n\
         2. 判斷對手是什麼牌組。不確定時用 WebSearch 查目前的環境和這個牌組的打法（日文、英文的資料都可以）。\n\
         3. 告訴我要斷哪裡：關鍵展開點，用手坑或魔法陷阱該斷哪張卡的哪個效果、什麼時機，那張卡在哪一格。\n\
         4. 正在問「是否連鎖」時：說要不要連鎖、用哪張（看得到我能連鎖的卡）。\n\
         5. 我這邊：對上這個牌組先攻、後攻各該怎麼打；看得到我的手牌時，這回合具體怎麼做（幾個步驟）。\n",
    );
    if !deck.is_empty() {
        p.push_str(&format!("\n我的牌組（我自己記的，可能不完整）：\n{deck}\n建議都照我真的有的卡來說。\n"));
    }
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
        "\n對戰畫面只回一個 JSON，不要其他文字：\n\
         {\"screen\":\"duel\",\"deck\":\"對手牌組名（繁中）\",\"deckAlt\":\"日文或英文名\",\"sure\":\"高|中|低\",\"seen\":[\"看到的對手卡名\"],\"plan\":\"它想做什麼，一句話\",\
         \"chain\":{\"card\":\"對手正在發動的卡\",\"answer\":\"要不要連鎖、用哪張，一句話\"},\
         \"stop\":[{\"card\":\"要斷的卡\",\"zone\":\"M3|S1|F|EL|hand|gy|chain…\",\"effect\":\"哪個效果\",\"with\":\"用什麼斷\",\"when\":\"時機\",\"why\":\"為什麼，20字內\"}],\
         \"me\":{\"first\":\"先攻怎麼打，一句\",\"second\":\"後攻怎麼打，一句\",\"now\":[\"這回合第一步\",\"第二步\"]},\
         \"watch\":[\"要小心它的什麼，15字內\"],\"tip\":\"一句建議\",\
         \"crew\":{\"claude\":\"總指揮：現在最該做的一件事\",\"codex\":\"算數字：生命值、打點、還差多少傷害或資源張數\",\"grok\":\"看對手：它可能還藏著什麼（蓋牌、手坑、墓地效果）\",\"agy\":\"查環境：這牌組在目前環境的地位、常帶的卡\"}}\n\
         crew 是四位軍師各說一句、各 25 字內、口氣像隊友在旁邊講話；看不出來的那位就說她要你注意什麼。\
         沒在問連鎖時 chain 寫 null；看不到我的手牌時 now 寫 []。stop 最多 3 個，照重要程度排；now 最多 4 步；watch 最多 3 個。\
         看不出對手的卡時 deck 寫空字串、plan 說看到了什麼。全部繁體中文、台灣用語。\n",
    );
    p
}

fn run(sh: &Shared, prompt: &str, timeout: Duration) -> Result<String, String> {
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
            Ok(None) if started.elapsed() > timeout => {
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
    #[derive(Default, Clone, Copy, Debug)]
    pub struct Rect {
        pub l: i32,
        pub t: i32,
        pub r: i32,
        pub b: i32,
    }

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
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
        fn IsIconic(hwnd: isize) -> i32;
        fn GetClientRect(hwnd: isize, rect: *mut Rect) -> i32;
        fn ClientToScreen(hwnd: isize, p: *mut Point) -> i32;
        fn PrintWindow(hwnd: isize, dc: isize, flags: u32) -> i32;
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

    // The game's largest window shown, and where its inside (its client
    // area: no title bar, no borders) is on the screen (physical).
    pub fn game_window(pid: u32) -> Option<(isize, Rect)> {
        struct Search {
            pid: u32,
            best: Option<(i64, isize, Rect)>,
        }
        unsafe extern "system" fn each(hwnd: isize, l: isize) -> i32 {
            // SAFETY: l is the Search below, alive for the enumeration.
            let search = unsafe { &mut *(l as *mut Search) };
            let mut pid = 0;
            let mut r = Rect::default();
            // SAFETY: reads of a window's process and size into our own values.
            unsafe {
                GetWindowThreadProcessId(hwnd, &mut pid);
                if pid == search.pid && IsWindowVisible(hwnd) != 0 && IsIconic(hwnd) == 0 && GetClientRect(hwnd, &mut r) != 0 {
                    let size = r.r as i64 * r.b as i64;
                    if size > 0 && search.best.is_none_or(|(s, _, _)| size > s) {
                        let mut at = Point { x: 0, y: 0 };
                        ClientToScreen(hwnd, &mut at);
                        search.best = Some((size, hwnd, Rect { l: at.x, t: at.y, r: at.x + r.r, b: at.y + r.b }));
                    }
                }
            }
            1
        }
        let mut search = Search { pid, best: None };
        // SAFETY: the callback is given our Search, which outlives the call.
        unsafe { EnumWindows(each, &mut search as *mut Search as isize) };
        search.best.map(|(_, h, r)| (h, r))
    }

    pub fn window_rect(pid: u32) -> Option<Rect> {
        game_window(pid).map(|(_, r)| r)
    }

    // The game's inside as it draws it (PrintWindow, its full content): the
    // whole of it even under other windows (tried 2026-10-22, the terminal
    // over half of it); else what the screen shows there. RGB.
    pub fn capture_game(hwnd: isize, r: Rect) -> Option<(usize, usize, Vec<u8>)> {
        let (w, h) = (r.r - r.l, r.b - r.t);
        if w <= 0 || h <= 0 {
            return None;
        }
        const SRCCOPY: u32 = 0x00CC_0020;
        const CAPTUREBLT: u32 = 0x4000_0000;
        // PW_CLIENTONLY | PW_RENDERFULLCONTENT
        const PRINT: u32 = 1 | 2;
        let mut bgra = vec![0u8; w as usize * h as usize * 4];
        // SAFETY: device contexts and a bitmap of ours, all let go below; the
        // pixels written into our own buffer of the size asked for.
        let ok = unsafe {
            let screen = GetDC(0);
            let mem = CreateCompatibleDC(screen);
            let bmp = CreateCompatibleBitmap(screen, w, h);
            let old = SelectObject(mem, bmp);
            let printed = PrintWindow(hwnd, mem, PRINT) != 0;
            let copied = printed || BitBlt(mem, 0, 0, w, h, screen, r.l, r.t, SRCCOPY | CAPTUREBLT) != 0;
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
        // All black: what PrintWindow gives for a window it cannot draw.
        if bgra.chunks_exact(4).all(|p| p[0] == 0 && p[1] == 0 && p[2] == 0) {
            return None;
        }
        let rgb = bgra.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0]]).collect();
        Some((w as usize, h as usize, rgb))
    }
}

#[cfg(not(windows))]
mod os {
    #[derive(Default, Clone, Copy, Debug)]
    pub struct Rect {
        pub l: i32,
        pub t: i32,
        pub r: i32,
        pub b: i32,
    }
    pub fn game_window(_pid: u32) -> Option<(isize, Rect)> {
        None
    }
    pub fn window_rect(_pid: u32) -> Option<Rect> {
        None
    }
    pub fn capture_game(_hwnd: isize, _r: Rect) -> Option<(usize, usize, Vec<u8>)> {
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
    let sh = crate::shared(&sh.app);
    apply_frames(&sh, SHOWN.load(Ordering::SeqCst) && FLY.load(Ordering::SeqCst));
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
        view["crew"] = crew(sh);
        view["fly"] = json!(FLY.load(Ordering::SeqCst));
        view["asks"] = Value::Object(ASKS.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_default());
        let _ = sh.app.emit_to(LABEL, "duel:update", view);
    }
}

// --- The four, each with a part ("軍師團", the user's word, 2026-10-22) ----------------------

// Each one's part, as her line in the panel says and as she is asked.
const PARTS: [(&str, &str); 4] = [
    ("claude", "總指揮：綜合大家的看法，告訴我現在最該做什麼"),
    ("codex", "算數字：生命值、攻擊力、打點、還差多少傷害、雙方資源張數"),
    ("grok", "看對手：對手可能還藏著什麼（蓋牌、手坑、墓地效果），它下一步想幹嘛"),
    ("agy", "查環境：這個牌組在目前環境的地位、常見構築和常帶的卡、怎麼對付"),
];

// Each asked by her (agent → { q, state: asking | done | failed, a, since, stood_in }).
static ASKS: Mutex<Option<serde_json::Map<String, Value>>> = Mutex::new(None);
// The window that had the keyboard before she typed in the panel.
static TYPED_FROM: Mutex<isize> = Mutex::new(0);

// The four, with what each pet is drawn from.
fn crew(sh: &Shared) -> Value {
    let settings = sh.settings.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let found = crate::data::pets(&sh.dir);
    json!(PARTS
        .iter()
        .map(|&(agent, _)| {
            let sprite = crate::data::pet_of(&settings, Some(agent), &found);
            json!({
                "agent": agent,
                "sprite": sprite.map(|p| crate::data::asset_url(&p.sheet)),
                "color": sprite.and_then(|p| p.color.clone()).unwrap_or_else(|| crate::data::agent_color(agent).to_string()),
            })
        })
        .collect::<Vec<_>>())
}

// What one of them is asked: her part, the duel as Claude read it off the
// last picture, her deck, the question; no tools, short.
pub fn ask_prompt(agent: &str, q: &str, found: &Value, deck: &str) -> String {
    let part = PARTS.iter().find(|p| p.0 == agent).map_or(PARTS[0].1, |p| p.1);
    let mut p = format!(
        "你是遊戲王 Master Duel 軍師團的一員，四位軍師各管一塊，你負責「{part}」。玩家在對戰中問你：「{q}」
         規則：只根據下面的資料和你自己對遊戲王的了解回答，不要使用任何工具、不要讀寫檔案、不要上網；         繁體中文、台灣 Master Duel 的卡名和用語；從你負責的角度回答，最多 3 行、100 字內，直接講結論，不要開場白。
"
    );
    if found.is_object() {
        p.push_str(&format!("
=== 目前對局（另一位軍師剛看遊戲截圖整理的）===
{}
", serde_json::to_string_pretty(found).unwrap_or_default()));
    } else {
        p.push_str("
（還沒看過對局畫面。）
");
    }
    if !deck.is_empty() {
        p.push_str(&format!("
=== 我的牌組 ===
{deck}
"));
    }
    p
}

// She asks one of them: that agent itself answers (Claude for one not
// installed here, said so), the panel told as it goes.
pub fn ask(sh: &Arc<Shared>, agent: &str, q: &str) {
    let q: String = q.trim().chars().take(200).collect();
    let Some(&(agent, _)) = PARTS.iter().find(|p| p.0 == agent) else { return };
    if q.is_empty() {
        return;
    }
    let since = crate::now_ms();
    ASKS.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(Default::default).insert(agent.into(), json!({ "q": q, "state": "asking", "since": since }));
    push(sh);
    let found = VIEW.lock().unwrap_or_else(|e| e.into_inner()).get("found").cloned().unwrap_or(Value::Null);
    let prompt = ask_prompt(agent, &q, &found, &deck_of(sh));
    let sh = sh.clone();
    crate::spawn_named("duel-ask", move || {
        let folder = dir(&sh).join("work");
        let _ = std::fs::create_dir_all(&folder);
        let installed = crate::run::find(match agent {
            "codex" => "codex",
            "grok" => "grok",
            "agy" => "agy",
            _ => "claude",
        })
        .is_some();
        let by = if installed { agent } else { "claude" };
        let got = crate::review::ask(by, &folder, &prompt);
        let entry = match got {
            Ok(a) => json!({ "q": q, "state": "done", "a": a, "since": since, "stoodIn": !installed }),
            Err(why) => json!({ "q": q, "state": "failed", "a": why, "since": since }),
        };
        {
            let mut asks = ASKS.lock().unwrap_or_else(|e| e.into_inner());
            let asks = asks.get_or_insert_with(Default::default);
            // Asked again meanwhile (or a new picture): not this answer.
            if asks.get(agent).and_then(|a| a["since"].as_u64()) != Some(since) {
                return;
            }
            asks.insert(agent.into(), entry);
        }
        push(&sh);
    });
}

// The panel takes the keyboard while she types a question, and gives it
// back to the game after.
pub fn typing(sh: &Shared, on: bool) {
    let Some(win) = window(sh) else { return };
    if on {
        *TYPED_FROM.lock().unwrap_or_else(|e| e.into_inner()) = crate::focus::foreground();
        let _ = win.set_focusable(true);
        let _ = win.set_focus();
    } else {
        let _ = win.set_focusable(false);
        let from = std::mem::take(&mut *TYPED_FROM.lock().unwrap_or_else(|e| e.into_inner()));
        if from != 0 {
            crate::focus::set_foreground(from);
        }
    }
}

// Inside the game, by its right edge, as a map helper sits in a game's
// corner: as tall as the game but a margin, at most SIZE; by the right edge
// of the island's display when no game is found.
fn rect(sh: &Shared) -> Option<((i32, i32), (i32, i32), f64)> {
    let game = game_pid().and_then(os::window_rect);
    let a = game.and_then(|r| sh.screens().near((r.l + r.r) / 2, (r.t + r.b) / 2)).or_else(|| crate::island::area(sh))?;
    let (gx, gy, gw, gh) = game.map_or((a.x, a.y, a.w, a.h), |r| (r.l, r.t, r.r - r.l, r.b - r.t));
    let margin = (MARGIN * a.sf).round() as i32;
    let bar = (BAR.0 * a.sf).round() as i32;
    let (w, h) = if FLY.load(Ordering::SeqCst) {
        let card = ((SIZE.0 * a.sf).round() as i32).min(gw / 3).max((260.0 * a.sf) as i32);
        (card + bar + (BAR_GAP * a.sf).round() as i32, ((SIZE.1 * a.sf).round() as i32).min(gh - 2 * margin))
    } else {
        (bar, ((BAR.1 * a.sf).round() as i32).min(gh - 2 * margin))
    };
    // Below the opponent's LP and the turn's mark, which sit at the game's top right.
    let top = gy + (gh as f64 * 0.16) as i32;
    let h = h.min(gy + gh - margin - top);
    Some(((gx + gw - w - margin, top), (w, h), a.sf))
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

// --- Her deck, read slot by slot (deckscan.rs) ---------------------------------------------

// The deck editor's picture (duel/full.png) cut into its cards and read,
// with her theme and the cards she has marked right (slot, name) kept; the
// deck window told as it goes. One at a time.
pub fn scan_deck(sh: &Arc<Shared>, theme: &str, kept: Vec<(String, String)>, fresh: bool) {
    {
        let mut scanning = SCANNING.lock().unwrap_or_else(|e| e.into_inner());
        if scanning.is_some() {
            return;
        }
        *scanning = Some(crate::now_ms());
    }
    open_deck(sh);
    let sh = sh.clone();
    let theme = theme.trim().to_string();
    crate::spawn_named("duel-deck", move || {
        let started = crate::now_ms();
        let done = (|| -> Result<(), String> {
            let d = dir(&sh);
            let full = crate::deckscan::Image::load(&d.join("full.png")).ok_or("no picture of the deck editor")?;
            let cards = crate::deckscan::slots(&full);
            if cards.is_empty() {
                return Err("no cards".into());
            }
            let slots_dir = d.join("slots");
            let _ = std::fs::remove_dir_all(&slots_dir);
            std::fs::create_dir_all(&slots_dir).map_err(|e| e.to_string())?;
            for (slot, img) in &cards {
                let _ = img.save(&slots_dir.join(format!("{slot}.png")));
            }
            let work = d.join("work");
            std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
            crate::deckscan::sheet(&cards).ok_or("no sheet")?.save(&work.join("sheet.png"))?;
            let main = cards.iter().filter(|(s, _)| s.starts_with('m')).count();
            // The cards she has said are what before, known by their look (yellow
            // still, for her to look at: her word, 2026-10-22).
            let memory = crate::deckscan::known(&d);
            let recalled: Vec<(String, String, Option<u64>)> = cards
                .iter()
                .filter(|(slot, _)| !kept.iter().any(|(s, _)| s == slot))
                .filter_map(|(slot, img)| crate::deckscan::recall(&memory, &crate::deckscan::print(img)).map(|k| (slot.clone(), k.name.clone(), k.id)))
                .collect();
            let known_names: Vec<(String, String)> = kept.iter().cloned().chain(recalled.iter().map(|(s, n, _)| (s.clone(), n.clone()))).collect();
            let numbered: Vec<(usize, String)> = known_names.iter().filter_map(|(slot, name)| cards.iter().position(|(s, _)| s == slot).map(|i| (i + 1, name.clone()))).collect();
            // The theme's cards from the card database, when it has any: picked among, no web search.
            let theme_cards = if theme.is_empty() { Vec::new() } else { crate::cards::with_cards(&sh.dir, |all| crate::cards::theme_cards(all, &theme)).unwrap_or_default() };
            let prompt = crate::deckscan::prompt_of(main, cards.len() - main, &theme, &theme_cards, &numbered);
            // All known: no one asked.
            let found = if numbered.len() == cards.len() {
                json!({ "cards": [] })
            } else {
                let text = run(&sh, &prompt, DECK_TIMEOUT)?;
                answer_in(&text).ok_or_else(|| text.chars().take(120).collect::<String>())?
            };
            let read: Vec<Value> = cards
                .iter()
                .enumerate()
                .map(|(i, (slot, _))| {
                    if let Some((_, name)) = kept.iter().find(|(s, _)| s == slot) {
                        return json!({ "slot": slot, "name": name, "sure": true });
                    }
                    if let Some((_, name, id)) = recalled.iter().find(|(s, _, _)| s == slot) {
                        return json!({ "slot": slot, "name": name, "id": id, "sure": false, "remembered": true });
                    }
                    let c = found["cards"].as_array().and_then(|a| a.iter().find(|c| c["n"].as_u64() == Some(i as u64 + 1)));
                    let said = c.and_then(|c| c["name"].as_str()).unwrap_or("");
                    // As the card database names it, when it names it so (English, Japanese, a "?").
                    let name = crate::cards::with_cards(&sh.dir, |all| crate::cards::best_match(all, said)).flatten().unwrap_or_else(|| said.to_string());
                    json!({ "slot": slot, "name": name, "sure": c.is_some_and(|c| c["sure"] != false) })
                })
                .collect();
            let theme = if theme.is_empty() { found["theme"].as_str().unwrap_or("").to_string() } else { theme.clone() };
            // Read again from its window: still the deck it was (its name, its place among the saved).
            let was = crate::deckscan::read_deck(&d);
            let (name, lib) = if fresh { (json!(""), Value::Null) } else { (was["name"].clone(), was["lib"].clone()) };
            let deck = json!({ "theme": theme, "cards": read, "at": crate::now_ms(), "name": name, "lib": lib });
            std::fs::write(crate::deckscan::deck_path(&d), deck.to_string()).map_err(|e| e.to_string())?;
            sh.change(json!({ "duelDeck": crate::deckscan::text_of(&deck) }));
            Ok(())
        })();
        let took = crate::now_ms().saturating_sub(started) / 1000;
        match &done {
            Ok(()) => sh.log(&format!("duel: her deck read card by card in {took} s ({} known by their look)", crate::deckscan::read_deck(&dir(&sh))["cards"].as_array().map_or(0, |a| a.iter().filter(|c| c["remembered"] == true).count()))),
            Err(why) => sh.log(&format!("duel: her deck not read ({why})")),
        }
        *SCANNING.lock().unwrap_or_else(|e| e.into_inner()) = None;
        let mut view = deck_view(&sh);
        view["error"] = json!(done.err());
        let _ = sh.app.emit_to(DECK, "duel:deck", view);
    });
}

// --- Her deck, to look over ---------------------------------------------------------------

pub const DECK: &str = "dueldeck";
static DECK_MADE: AtomicBool = AtomicBool::new(false);
static DECK_READY: AtomicBool = AtomicBool::new(false);
static DECK_WANTED: AtomicBool = AtomicBool::new(false);
// Where she put the deck window (dragged by its top, sized by its edges:
// she wanted the game's own deck beside it, 2026-10-22), kept for the next
// time in duel/deckwin.json.
static DECK_SPOT: Mutex<Option<(i32, i32, i32, i32)>> = Mutex::new(None);

fn deck_spot(sh: &Shared) -> Option<(i32, i32, i32, i32)> {
    let mut spot = DECK_SPOT.lock().unwrap_or_else(|e| e.into_inner());
    if spot.is_none() {
        *spot = std::fs::read_to_string(dir(sh).join("deckwin.json")).ok().and_then(|t| serde_json::from_str(&t).ok());
    }
    *spot
}

fn deck_view(sh: &Shared) -> Value {
    let d = dir(sh);
    let deck = crate::deckscan::read_deck(&d);
    let slots_dir = d.join("slots");
    let _ = sh.app.asset_protocol_scope().allow_directory(&slots_dir, false);
    let at = deck["at"].as_u64().unwrap_or(0);
    // Each slot's picture, its address new with each reading.
    let pictures: serde_json::Map<String, Value> = deck["cards"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| {
            let slot = c["slot"].as_str()?;
            let file = slots_dir.join(format!("{slot}.png"));
            file.is_file().then(|| (slot.to_string(), json!(format!("{}?v={at}", crate::data::asset_url(&file)))))
        })
        .collect();
    let scanning = *SCANNING.lock().unwrap_or_else(|e| e.into_inner());
    // Which slots look alike (copies of a card): one renamed renames the others.
    let looks: Vec<(String, Vec<f32>)> = pictures
        .keys()
        .filter_map(|slot| Some((slot.clone(), crate::deckscan::print(&crate::deckscan::Image::load(&slots_dir.join(format!("{slot}.png")))?))))
        .collect();
    let alike: serde_json::Map<String, Value> = looks
        .iter()
        .map(|(slot, look)| (slot.clone(), json!(looks.iter().filter(|(s, l)| s != slot && crate::deckscan::alike(look, l) >= crate::deckscan::ALIKE).map(|(s, _)| s).collect::<Vec<_>>())))
        .collect();
    json!({ "lang": sh.lang(), "deck": deck, "decks": decks(sh), "pictures": pictures, "alike": alike, "scanning": scanning, "hasPicture": d.join("full.png").is_file(), "text": deck_of(sh) })
}

// Up, in the middle of the display the game is on (it takes the keyboard: names are typed in it).
pub fn open_deck(sh: &Arc<Shared>) {
    DECK_WANTED.store(true, Ordering::SeqCst);
    // The card database ready by the time a name is typed (fetched the first time).
    let _ = crate::cards::with_cards(&sh.dir, |_| ());
    let a = game_pid().and_then(os::window_rect).and_then(|r| sh.screens().near((r.l + r.r) / 2, (r.t + r.b) / 2)).or_else(|| crate::island::area(sh));
    let Some(a) = a else { return };
    let (mut w, mut h) = (((1080.0 * a.sf) as i32).min(a.w - 40), ((760.0 * a.sf) as i32).min(a.h - 40));
    let mut pos = (a.x + (a.w - w) / 2, a.y + (a.h - h) / 2);
    // Where she left it, while its top is still on a display.
    if let Some((x, y, sw, sh_)) = deck_spot(sh).filter(|&(x, y, sw, _)| sh.screens().near(x + sw / 2, y + 20).is_some()) {
        (pos, w, h) = ((x, y), sw.max(400), sh_.max(300));
    }
    if !DECK_MADE.swap(true, Ordering::SeqCst) {
        let sh = sh.clone();
        let sf = a.sf;
        std::thread::spawn(move || {
            let built = WebviewWindowBuilder::new(&sh.app, DECK, WebviewUrl::App("duel/deck.html".into()))
                .title("wakuwaku duel deck")
                .inner_size(w as f64 / sf, h as f64 / sf)
                .additional_browser_args(crate::browser_args(&sh))
                .data_directory(crate::webview_dir(&sh))
                .transparent(true)
                .decorations(false)
                .resizable(true)
                .min_inner_size(560.0, 420.0)
                .skip_taskbar(true)
                .shadow(false)
                .always_on_top(true)
                .visible(false)
                .build();
            match built {
                Ok(win) => {
                    lay(&win, pos, (w, h));
                    let _ = win.set_ignore_cursor_events(false);
                    sh.own_window(&win);
                }
                Err(err) => sh.log(&format!("duel: could not make its deck window: {err}")),
            }
        });
        return;
    }
    if let Some(win) = sh.app.get_webview_window(DECK) {
        lay(&win, pos, (w, h));
        if DECK_READY.load(Ordering::SeqCst) {
            let _ = sh.app.emit_to(DECK, "duel:deck", deck_view(sh));
            let _ = win.show();
            let _ = win.set_focus();
        }
    }
}

// The deck window's drop-down: cards by what is typed (none while the
// database is being fetched the first time: "loading").
pub fn search_cards(sh: &Shared, q: &str) -> Value {
    match crate::cards::with_cards(&sh.dir, |all| crate::cards::search(all, q, 12)) {
        Some(found) => json!({ "cards": found }),
        None => json!({ "cards": [], "loading": true }),
    }
}

// Every picture of a card, its alternate arts too.
pub fn card_arts(sh: &Shared, id: u64) -> Value {
    json!(crate::cards::with_cards(&sh.dir, |all| crate::cards::arts(all, id)).unwrap_or_default())
}

pub fn deck_ready(sh: &Arc<Shared>) {
    DECK_READY.store(true, Ordering::SeqCst);
    if DECK_WANTED.load(Ordering::SeqCst) {
        open_deck(sh);
    }
}

pub fn close_deck(sh: &Shared) {
    DECK_WANTED.store(false, Ordering::SeqCst);
    if let Some(win) = sh.app.get_webview_window(DECK) {
        if let (Ok(p), Ok(s)) = (win.outer_position(), win.outer_size()) {
            let spot = (p.x, p.y, s.width as i32, s.height as i32);
            *DECK_SPOT.lock().unwrap_or_else(|e| e.into_inner()) = Some(spot);
            let _ = std::fs::write(dir(sh).join("deckwin.json"), json!([spot.0, spot.1, spot.2, spot.3]).to_string());
        }
        let _ = win.hide();
    }
}

// What the deck window sorts and filters by and shows of each card: its
// ATK, DEF, level, attribute, type, kind and text, from the database.
pub fn card_info(sh: &Shared, cards: &[(Option<u64>, String)]) -> Value {
    match crate::cards::with_cards(&sh.dir, |all| cards.iter().map(|(id, name)| crate::cards::info(all, *id, name).unwrap_or(Value::Null)).collect::<Vec<_>>()) {
        Some(found) => json!({ "cards": found }),
        None => json!({ "cards": [], "loading": true }),
    }
}

// Her saved decks (duel/decks/<id>/: deck.json and its cards' pictures),
// one of them the one in use (duel/decks/active): the deck window opens on
// it and the helper's advice goes by it (she asked, 2026-10-22).
fn decks_dir(sh: &Shared) -> PathBuf {
    dir(sh).join("decks")
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    let _ = std::fs::remove_dir_all(to);
    let _ = std::fs::create_dir_all(to);
    for e in std::fs::read_dir(from).into_iter().flatten().flatten() {
        let _ = std::fs::copy(e.path(), to.join(e.file_name()));
    }
}

pub fn decks(sh: &Shared) -> Value {
    let mut list: Vec<Value> = std::fs::read_dir(decks_dir(sh))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let deck = crate::deckscan::read_deck(&e.path());
            let id = e.file_name().to_string_lossy().into_owned();
            let main = deck["cards"].as_array().map_or(0, |a| a.iter().filter(|c| c["slot"].as_str().is_some_and(|s| s.starts_with('m'))).count());
            Some(json!({ "id": id, "name": deck["name"].as_str().filter(|n| !n.is_empty()).unwrap_or(&id), "main": main, "saved": deck["saved"] }))
        })
        .collect();
    list.sort_by(|a, b| b["saved"].as_u64().cmp(&a["saved"].as_u64()));
    json!(list)
}

// One saved deck taken up: in the window, in use.
pub fn use_deck(sh: &Arc<Shared>, id: &str) {
    let from = decks_dir(sh).join(id);
    if id.contains(['/', '\\', '.']) || !from.join("deck.json").is_file() {
        return;
    }
    let d = dir(sh);
    let mut deck = crate::deckscan::read_deck(&from);
    deck["lib"] = json!(id);
    deck["at"] = json!(crate::now_ms());
    let _ = std::fs::write(crate::deckscan::deck_path(&d), deck.to_string());
    copy_dir(&from.join("slots"), &d.join("slots"));
    let _ = std::fs::write(decks_dir(sh).join("active"), id);
    sh.change(json!({ "duelDeck": crate::deckscan::text_of(&deck) }));
    let _ = sh.app.emit_to(DECK, "duel:deck", deck_view(sh));
}

// A saved deck thrown away (the one in the window stays, saved nowhere).
pub fn drop_deck(sh: &Arc<Shared>, id: &str) {
    if id.is_empty() || id.contains(['/', '\\', '.']) {
        return;
    }
    let _ = std::fs::remove_dir_all(decks_dir(sh).join(id));
    let d = dir(sh);
    let mut deck = crate::deckscan::read_deck(&d);
    if deck["lib"] == id {
        deck["lib"] = Value::Null;
        let _ = std::fs::write(crate::deckscan::deck_path(&d), deck.to_string());
    }
    let _ = sh.app.emit_to(DECK, "duel:deck", deck_view(sh));
}

// A new deck: the window empty, to be read from the game.
pub fn new_deck(sh: &Arc<Shared>) {
    let d = dir(sh);
    let _ = std::fs::write(crate::deckscan::deck_path(&d), json!({ "theme": "", "cards": [], "name": "", "lib": null, "at": crate::now_ms() }).to_string());
    let _ = std::fs::remove_dir_all(d.join("slots"));
    let _ = std::fs::remove_file(d.join("full.png"));
    let _ = sh.app.emit_to(DECK, "duel:deck", deck_view(sh));
}

// Put right by hand: { theme, name, cards: [{ slot, name, sure }] }; kept
// among her saved decks by its name (the one it came from, or one so named).
pub fn save_deck(sh: &Arc<Shared>, deck: &Value) {
    let cards: Vec<Value> = deck["cards"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| {
            let slot = c["slot"].as_str().filter(|s| s.len() <= 8 && (s.starts_with('m') || s.starts_with('e')))?;
            let name: String = c["name"].as_str().unwrap_or("").trim().chars().take(60).collect();
            Some(json!({ "slot": slot, "name": name, "sure": c["sure"] != false, "id": c["id"].as_u64(), "remembered": c["sure"] == false && c["remembered"] == true }))
        })
        .take(80)
        .collect();
    let theme: String = deck["theme"].as_str().unwrap_or("").trim().chars().take(60).collect();
    let name: String = deck["name"].as_str().unwrap_or("").trim().chars().take(40).collect();
    let name = if name.is_empty() { if theme.is_empty() { "我的牌組".to_string() } else { theme.clone() } } else { name };
    let was = crate::deckscan::read_deck(&dir(sh));
    // Where among the saved: the one it came from, else one of that name, else new.
    let lib = was["lib"]
        .as_str()
        .filter(|id| decks_dir(sh).join(id).is_dir())
        .map(str::to_string)
        .or_else(|| decks(sh).as_array().into_iter().flatten().find(|d| d["name"] == name.as_str()).and_then(|d| d["id"].as_str().map(str::to_string)))
        .unwrap_or_else(|| crate::now_ms().to_string());
    let deck = json!({ "theme": theme, "name": name, "lib": lib, "cards": cards, "at": was["at"], "saved": crate::now_ms() });
    let _ = std::fs::create_dir_all(dir(sh));
    let _ = std::fs::write(crate::deckscan::deck_path(&dir(sh)), deck.to_string());
    let keep = decks_dir(sh).join(&lib);
    let _ = std::fs::create_dir_all(&keep);
    let _ = std::fs::write(crate::deckscan::deck_path(&keep), deck.to_string());
    copy_dir(&dir(sh).join("slots"), &keep.join("slots"));
    let _ = std::fs::write(decks_dir(sh).join("active"), &lib);
    sh.change(json!({ "duelDeck": crate::deckscan::text_of(&deck) }));
    // Each card she marked right, its look learnt: known the next time it is read.
    let slots = dir(sh).join("slots");
    let learnt: Vec<crate::deckscan::Known> = deck["cards"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["sure"] == true && !c["name"].as_str().unwrap_or("").is_empty())
        .filter_map(|c| {
            let img = crate::deckscan::Image::load(&slots.join(format!("{}.png", c["slot"].as_str()?)))?;
            Some(crate::deckscan::Known { look: crate::deckscan::print(&img), name: c["name"].as_str()?.to_string(), id: c["id"].as_u64() })
        })
        .collect();
    let n = learnt.len();
    crate::deckscan::remember(&dir(sh), learnt);
    sh.log(&format!("duel: her deck put right by hand, {n} cards' looks learnt"));
    let mut view = deck_view(sh);
    view["saved"] = deck["name"].clone();
    let _ = sh.app.emit_to(DECK, "duel:deck", view);
}

// --- The frames on the game ------------------------------------------------------------

// The zones to frame from an answer: each card to stop that is on the field,
// numbered as the panel numbers them.
pub fn zones_of(found: &Value) -> Value {
    let stops = found["stop"].as_array().cloned().unwrap_or_default();
    Value::Array(
        stops
            .iter()
            .take(3)
            .enumerate()
            .filter(|(_, s)| matches!(s["zone"].as_str(), Some("S1" | "S2" | "S3" | "S4" | "S5" | "M1" | "M2" | "M3" | "M4" | "M5" | "F" | "EL" | "ER")))
            .map(|(i, s)| json!({ "zone": s["zone"], "n": i + 1, "with": s["with"] }))
            .collect(),
    )
}

fn frames_window(sh: &Shared) -> Option<WebviewWindow> {
    sh.app.get_webview_window(FRAMES)
}

fn make_frames(sh: &Arc<Shared>) {
    if FRAMES_MADE.swap(true, Ordering::SeqCst) {
        return;
    }
    let Some(r) = game_pid().and_then(os::window_rect) else {
        FRAMES_MADE.store(false, Ordering::SeqCst);
        return;
    };
    let sh = sh.clone();
    std::thread::spawn(move || {
        let built = WebviewWindowBuilder::new(&sh.app, FRAMES, WebviewUrl::App("duel/frames.html".into()))
            .title("wakuwaku duel frames")
            .inner_size(400.0, 300.0)
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
            .build();
        match built {
            Ok(win) => {
                lay(&win, (r.l, r.t), (r.r - r.l, r.b - r.t));
                // Every click goes on to the game under it.
                let _ = win.set_ignore_cursor_events(true);
                sh.own_window(&win);
            }
            Err(err) => sh.log(&format!("duel: could not make its frames: {err}")),
        }
    });
}

pub fn frames_ready(sh: &Arc<Shared>) {
    FRAMES_READY.store(true, Ordering::SeqCst);
    apply(sh);
}

// The frames: on the game's inside while the panel is up with an answer.
fn apply_frames(sh: &Arc<Shared>, want: bool) {
    let zones = VIEW.lock().unwrap_or_else(|e| e.into_inner()).get("found").map(zones_of).unwrap_or(json!([]));
    let any = zones.as_array().is_some_and(|z| !z.is_empty());
    if want && any && !FRAMES_MADE.load(Ordering::SeqCst) {
        make_frames(sh);
        return;
    }
    if !FRAMES_READY.load(Ordering::SeqCst) {
        return;
    }
    let Some(win) = frames_window(sh) else { return };
    if want && any {
        if let Some(r) = game_pid().and_then(os::window_rect) {
            lay(&win, (r.l, r.t), (r.r - r.l, r.b - r.t));
        }
        let _ = sh.app.emit_to(FRAMES, "duel:frames", json!({ "zones": zones }));
        let _ = win.show();
    } else {
        let _ = win.hide();
    }
}

// Its page is up: drawn, and shown if it was asked for meanwhile.
pub fn ready(sh: &Arc<Shared>) {
    READY.store(true, Ordering::SeqCst);
    push(sh);
    apply(sh);
}

fn apply(sh: &Arc<Shared>) {
    let want = SHOWN.load(Ordering::SeqCst);
    // The frames on the game go with the look's card.
    apply_frames(sh, want && FLY.load(Ordering::SeqCst));
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
        let p = prompt_of("", "", None);
        assert!(p.contains("shot.png") && p.contains("\"stop\"") && !p.contains("我的牌組帶的"));
        let last = json!({ "deck": "蛇眼", "seen": ["蛇眼灰燼", "原罪寶-蛇眼"] });
        let p = prompt_of("灰流麗、增殖的G", "巳劍：天叢雲之巳劍×2", Some(&last));
        assert!(p.contains("我的牌組（我自己記的") && p.contains("天叢雲之巳劍×2") && p.contains("\"zone\""));
        assert!(p.contains("我的牌組帶的手坑／斷點卡：灰流麗、增殖的G"));
        assert!(p.contains("可能是「蛇眼」（看到：蛇眼灰燼、原罪寶-蛇眼）"));
    }

    #[test]
    fn the_answer_whatever_is_written_around_it() {
        let said = "好的，這是分析：
```json
{\"screen\":\"duel\",\"deck\":\"蛇眼\",\"stop\":[]}
```
補充：{其他}";
        assert_eq!(answer_in(said).unwrap()["deck"], "蛇眼");
        assert!(answer_in("沒有 JSON").is_none());
        assert_eq!(answer_in("看完了。```json
{\"theme\":\"雷熱\",\"cards\":[{\"n\":1}]}
```").unwrap()["theme"], "雷熱");
        assert_eq!(answer_in("{\"x\":1} {\"screen\":\"deck\",\"myDeck\":\"a\"}").unwrap()["screen"], "deck");
    }

    #[test]
    fn frames_for_the_cards_on_the_field_numbered_as_listed() {
        let found = json!({ "stop": [ { "zone": "hand" }, { "zone": "M3", "with": "灰流麗" }, { "zone": "F" }, { "zone": "S2" } ] });
        assert_eq!(zones_of(&found), json!([{ "zone": "M3", "n": 2, "with": "灰流麗" }, { "zone": "F", "n": 3, "with": null }]));
        assert_eq!(zones_of(&json!({})), json!([]));
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
