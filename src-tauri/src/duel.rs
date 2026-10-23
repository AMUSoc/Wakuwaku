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
const BAR: (f64, f64) = (64.0, 222.0);
const BAR_GAP: f64 = 8.0;
static FLY: AtomicBool = AtomicBool::new(false);
// How tall the card wants to be (logical px, its page measures it): no
// taller, so no empty glass over the game catching its clicks (2026-10-23).
static FIT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
const MARGIN: f64 = 12.0;
// The picture sent: so wide at most (a 2560 display halved).
// The longest side Claude sees whole (larger is made smaller on its side).
// It was 1600 by halving: a 2048-wide game went to Claude at 1024, its
// cards too small to read, for every look until 2026-10-23.
const MAX_WIDTH: usize = 1568;
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

// The card's page: how tall it wants to be.
pub fn fit(sh: &Arc<Shared>, height: u32) {
    let height = height.clamp(1, 2000);
    if FIT.swap(height, Ordering::SeqCst) != height && SHOWN.load(Ordering::SeqCst) && FLY.load(Ordering::SeqCst) {
        if let (Some(win), Some((pos, size, _))) = (window(sh), rect(sh)) {
            lay(&win, pos, size);
        }
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
    look_with(sh, pid, false)
}

// A look; `quiet` (the watch): the card keeps its last answer, marked updating,
// and the deck editor seen is left alone.
fn look_with(sh: &Arc<Shared>, pid: u32, quiet: bool) {
    let Some((hwnd, rect)) = os::game_window(pid) else {
        sh.log("duel: the game's window not found");
        return;
    };
    LOOKING.store(true, Ordering::SeqCst);
    let picture = dir(sh).join("work").join("shot.png");
    let taken = std::fs::create_dir_all(dir(sh).join("work")).is_ok()
        && os::capture_game(hwnd, rect).is_some_and(|(w, h, rgb)| {
            let saved = save_png(&picture, w, h, &rgb).is_ok();
            // Closer: the opponent's zones one by one, and my hand along the bottom.
            let work = dir(sh).join("work");
            let low = h * 80 / 100;
            let _ = save_png(&work.join("hand.png"), w, h - low, &rows(w, &rgb, low, h));
            let full = crate::deckscan::Image { w, h, rgb };
            let zones = zone_sheet(&full);
            let _ = save_png(&work.join("zones.png"), zones.w, zones.h, &zones.rgb);
            *FULL.lock().unwrap_or_else(|e| e.into_inner()) = Some(full);
            saved
        });
    if !taken {
        LOOKING.store(false, Ordering::SeqCst);
        if quiet {
            return;
        }
        sh.log("duel: the game's window could not be pictured");
        publish(sh, json!({ "state": "error", "why": "picture" }));
        show(sh, true);
        return;
    }
    let started = crate::now_ms();
    keep_shot(sh, &picture, started);
    let has_answer = VIEW.lock().unwrap_or_else(|e| e.into_inner())["state"] == "ready";
    if quiet && has_answer {
        VIEW.lock().unwrap_or_else(|e| e.into_inner())["updating"] = json!(started);
        push(sh);
    } else {
        publish(sh, json!({ "state": "looking", "since": started, "last": last_guess() }));
        FLY.store(true, Ordering::SeqCst);
        show(sh, true);
    }
    let sh = sh.clone();
    crate::spawn_named("duel", move || {
        tidy_match();
        let guess = match_guess().or_else(last_guess);
        let mut prompt = prompt_of(&hand_of(&sh), &deck_of(&sh), guess.as_ref(), &match_known());
        prompt.push_str(&card_notes(&sh));
        let work = dir(&sh).join("work");
        let pictures = [work.join("shot.png"), work.join("zones.png"), work.join("hand.png")];
        let answer = see(&sh, &prompt, &pictures, TIMEOUT).and_then(|text| answer_in(&text).ok_or_else(|| text.chars().take(120).collect()));
        let took = crate::now_ms().saturating_sub(started);
        match answer {
            // The watch on the deck editor: nothing to do there.
            Ok(found) if quiet && found["screen"] == "deck" => {
                VIEW.lock().unwrap_or_else(|e| e.into_inner()).as_object_mut().map(|v| v.remove("updating"));
                push(&sh);
            }
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
            Ok(mut found) => {
                let guessed = MATCH.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|m| m.deck.clone()).unwrap_or_default();
                crate::cards::with_cards(&sh.dir, |all| settle(all, &mut found, &guessed));
                sh.log(&format!("duel: {} in {} s", found["deck"].as_str().unwrap_or("?"), took / 1000));
                *LAST.lock().unwrap_or_else(|e| e.into_inner()) = Some((found.clone(), crate::now_ms()));
                take_in(&sh, &found);
                let at = crate::now_ms();
                publish(&sh, json!({ "state": "ready", "found": found, "took": took, "at": at, "quiet": quiet, "more": true }));
                LOOKING.store(false, Ordering::SeqCst);
                more(&sh, found, at);
                return;
            }
            // The watch not answered: the last answer stays.
            Err(why) if quiet && has_answer => {
                sh.log(&format!("duel: no answer ({why})"));
                VIEW.lock().unwrap_or_else(|e| e.into_inner()).as_object_mut().map(|v| v.remove("updating"));
                push(&sh);
            }
            Err(why) => {
                sh.log(&format!("duel: no answer ({why})"));
                publish(&sh, json!({ "state": "error", "why": why, "last": last_guess() }));
            }
        }
        LOOKING.store(false, Ordering::SeqCst);
    });
}

// The rest of a look, from its first answer: how to play on, what to
// watch, a tip; laid into the answer shown while it is still that one.
const REST: [&str; 3] = ["me", "watch", "tip"];

fn more(sh: &Arc<Shared>, found: Value, at: u64) {
    let mut core = found;
    if let Some(o) = core.as_object_mut() {
        for k in ["screen", "deckAlt", "fresh"] {
            o.remove(k);
        }
    }
    let prompt = prompt_more(&core, &deck_of(sh), &card_notes(sh));
    let got = see(sh, &prompt, &[], TIMEOUT).ok().and_then(|t| answer_of(&t));
    let took = crate::now_ms().saturating_sub(at);
    {
        let mut view = VIEW.lock().unwrap_or_else(|e| e.into_inner());
        if view["at"].as_u64() != Some(at) {
            return;
        }
        view.as_object_mut().map(|v| v.remove("more"));
        if let Some(got) = &got {
            for k in REST {
                if !got[k].is_null() {
                    view["found"][k] = got[k].clone();
                }
            }
        }
    }
    if let Some(got) = &got {
        if let Some((v, _)) = LAST.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            for k in REST {
                v[k] = got[k].clone();
            }
        }
    }
    sh.log(&format!("duel: the rest {} in {} s more", if got.is_some() { "came" } else { "did not come" }, took / 1000));
    push(sh);
}

// Names as the card database writes them; a monster named wrong, or not
// named, put right by what the picture shows of it (ATK, DEF, level) when
// one card of those numbers fits (among the deck guessed's cards, this
// look's or the duel's before, else of all). Such an entry says so ("by": "stats").
pub fn settle(all: &[crate::cards::Card], found: &mut Value, guessed: &str) {
    let deck = found["deck"].as_str().filter(|d| !d.is_empty()).unwrap_or(guessed).to_string();
    let theme: Vec<String> = if deck.is_empty() { Vec::new() } else { crate::cards::theme_cards(all, &deck) };
    if let Some(field) = found["field"].as_array_mut() {
        for f in field.iter_mut() {
            let said = f["name"].as_str().unwrap_or("").trim().to_string();
            if said == "蓋牌" {
                continue;
            }
            let known = (!said.is_empty())
                .then(|| crate::cards::info(all, None, &said).or_else(|| crate::cards::best_match(all, &said).and_then(|n| crate::cards::info(all, None, &n))))
                .flatten();
            let fits = |i: &Value| ["atk", "def", "level"].iter().all(|k| f[*k].as_i64().is_none_or(|v| i[*k].as_i64() == Some(v)));
            let by_numbers = by_stats(all, f, &theme);
            match (known, by_numbers) {
                (Some(i), _) if fits(&i) => f["name"] = i["name"].clone(),
                // Its numbers say another card (they may be changed by an effect: one card of those numbers, else as said).
                (_, Some(n)) => {
                    f["name"] = json!(n);
                    f["by"] = json!("stats");
                }
                (Some(i), None) => f["name"] = i["name"].clone(),
                (None, None) => {}
            }
        }
    }
    // Every card named as the database names it; the field's among those seen.
    let fix = |v: &mut Value| {
        if let Some(n) = v.as_str().and_then(|s| crate::cards::best_match(all, s)) {
            *v = json!(n);
        }
    };
    for k in ["seen", "myHand", "mySet", "used"] {
        if let Some(a) = found[k].as_array_mut() {
            a.iter_mut().for_each(fix);
        }
    }
    let named: Vec<Value> = found["field"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| f["name"].as_str())
        .filter(|n| !n.is_empty() && *n != "蓋牌" && !n.starts_with(['?', '？']))
        .map(|n| json!(n))
        .collect();
    if !found["seen"].is_array() {
        found["seen"] = json!([]);
    }
    if let Some(seen) = found["seen"].as_array_mut() {
        for n in named {
            if !seen.contains(&n) {
                seen.push(n);
            }
        }
    }
    // A card to stop that is on the field: framed where the field was read
    // to hold it (Claude once wrote M3 for the card it had read on M4, 2026-10-23).
    let on_field: Vec<(String, String)> = found["field"].as_array().into_iter().flatten().filter_map(|f| Some((f["name"].as_str()?.to_string(), f["zone"].as_str()?.to_string()))).collect();
    if let Some(stops) = found["stop"].as_array_mut() {
        for s in stops.iter_mut() {
            fix(&mut s["card"]);
            let card = s["card"].as_str().unwrap_or("");
            let zones: Vec<&String> = on_field.iter().filter(|(n, _)| n == card).map(|(_, z)| z).collect();
            let elsewhere = matches!(s["zone"].as_str(), Some("hand" | "gy" | "ban"));
            if zones.len() == 1 && !elsewhere {
                s["zone"] = json!(zones[0]);
            }
        }
    }
}

// The one card with these numbers: among the deck guessed's when any are, else of all.
fn by_stats(all: &[crate::cards::Card], f: &Value, theme: &[String]) -> Option<String> {
    let (atk, def) = (f["atk"].as_i64()?, f["def"].as_i64()?);
    let fit = crate::cards::by_stats(all, atk as i32, def as i32, f["level"].as_u64().map(|l| l as u32));
    let ours: Vec<&&crate::cards::Card> = fit.iter().filter(|c| theme.contains(&c.name)).collect();
    match (ours.len(), fit.len()) {
        (1, _) => Some(ours[0].name.clone()),
        (0, 1) => Some(fit[0].name.clone()),
        _ => None,
    }
}

// --- Watching the duel (盯場: she asked for it to keep looking through the
// opponent's turn, 2026-10-23) ---------------------------------------------------------
//
// On from the bar's 👁: every 5 s the game is pictured small; changed since
// the last look (or 20 s on) and no look running, a look, quietly: the card
// keeps the last answer and says it is updating. Her own turn ("turn":
// "mine") it rests, looking once in 30 s to see the turn come back. What the
// duel has shown is kept (every opponent card seen, the deck guessed) and
// asked with; the deck guessed is looked up on the web once (its plan, where
// to stop it), and that too goes with every look. A fresh duel starts over.
static WATCH: AtomicBool = AtomicBool::new(false);
static WATCHING: AtomicBool = AtomicBool::new(false);
static MATCH: Mutex<Option<Match>> = Mutex::new(None);
const WATCH_EVERY: Duration = Duration::from_secs(5);
const WATCH_ANYWAY_MS: u64 = 20_000;
const WATCH_RESTING_MS: u64 = 30_000;

#[derive(Default)]
struct Match {
    seen: Vec<String>,
    deck: String,
    mine: bool,
    looked: u64,
    frame: Vec<u8>,
    // The deck looked up: its name, and what was found (null while looking).
    research: Option<(String, Value)>,
    // The turn's number, the cards she has used this turn, and her hand as last read.
    turn: i64,
    used: Vec<String>,
    hand: Vec<String>,
    // When an answer last came in: 20 minutes on, another duel.
    at: u64,
}

// An old duel forgotten (no look for 20 minutes).
fn tidy_match() {
    let mut m = MATCH.lock().unwrap_or_else(|e| e.into_inner());
    if m.as_ref().is_some_and(|m| m.at > 0 && crate::now_ms().saturating_sub(m.at) > GUESS_MS) {
        *m = Some(Match::default());
    }
}

pub fn watch(sh: &Arc<Shared>, on: bool) {
    WATCH.store(on, Ordering::SeqCst);
    if on {
        *MATCH.lock().unwrap_or_else(|e| e.into_inner()) = Some(Match::default());
        FLY.store(true, Ordering::SeqCst);
        show(sh, true);
        if !WATCHING.swap(true, Ordering::SeqCst) {
            let sh = sh.clone();
            crate::spawn_named("duel-watch", move || {
                while WATCH.load(Ordering::SeqCst) {
                    std::thread::sleep(WATCH_EVERY);
                    watch_once(&sh);
                }
                WATCHING.store(false, Ordering::SeqCst);
            });
        }
    }
    push(sh);
}

// The game pictured small (32x18 grey) and how much it changed since.
fn small(w: usize, h: usize, rgb: &[u8]) -> Vec<u8> {
    let (sw, sh_) = (32, 18);
    (0..sw * sh_)
        .map(|i| {
            let (x, y) = ((i % sw) * w / sw + w / sw / 2, (i / sw) * h / sh_ + h / sh_ / 2);
            let p = (y.min(h - 1) * w + x.min(w - 1)) * 3;
            ((rgb[p] as u32 * 3 + rgb[p + 1] as u32 * 6 + rgb[p + 2] as u32) / 10) as u8
        })
        .collect()
}

fn changed(a: &[u8], b: &[u8]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 255.0;
    }
    a.iter().zip(b).map(|(x, y)| (*x as f32 - *y as f32).abs()).sum::<f32>() / a.len() as f32
}

fn watch_once(sh: &Arc<Shared>) {
    if !SHOWN.load(Ordering::SeqCst) || LOOKING.load(Ordering::SeqCst) {
        return;
    }
    let Some(pid) = game_pid() else { return };
    let Some((hwnd, rect)) = os::game_window(pid) else { return };
    let Some((w, h, rgb)) = os::capture_game(hwnd, rect) else { return };
    let now = crate::now_ms();
    let frame = small(w, h, &rgb);
    let go = {
        let mut m = MATCH.lock().unwrap_or_else(|e| e.into_inner());
        let Some(m) = m.as_mut() else { return };
        let since = now.saturating_sub(m.looked);
        let go = if m.mine { since >= WATCH_RESTING_MS } else { changed(&frame, &m.frame) > 3.0 || since >= WATCH_ANYWAY_MS };
        if go {
            m.frame = frame;
            m.looked = now;
        }
        go
    };
    if go {
        look_with(sh, pid, true);
    }
}

// What the duel has shown, for the prompt: the deck guessed and every card seen.
fn match_guess() -> Option<Value> {
    let m = MATCH.lock().unwrap_or_else(|e| e.into_inner());
    let m = m.as_ref().filter(|m| !m.seen.is_empty() || !m.deck.is_empty())?;
    Some(json!({ "deck": if m.deck.is_empty() { "?" } else { m.deck.as_str() }, "seen": m.seen }))
}

// What the cards say, for the look (2026-10-23: told to throw a second
// copy of a card used once by name that turn, and stops guessed from art):
// the cards she has used this turn; her deck's cards that are once a turn
// by name; the texts of the opponent's cards seen and of her hand.
fn card_notes(sh: &Shared) -> String {
    let (turn, used, seen, hand) = {
        let m = MATCH.lock().unwrap_or_else(|e| e.into_inner());
        m.as_ref().map(|m| (m.turn, m.used.clone(), m.seen.clone(), m.hand.clone())).unwrap_or_default()
    };
    let mine: Vec<String> = {
        let deck = crate::deckscan::read_deck(&dir(sh));
        let mut names: Vec<String> = deck["cards"].as_array().into_iter().flatten().filter_map(|c| c["name"].as_str()).map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).collect();
        names.sort();
        names.dedup();
        names
    };
    crate::cards::with_cards(&sh.dir, |all| {
        let mut out = String::new();
        if !used.is_empty() {
            out.push_str(&format!("\n這回合（第 {turn} 回合）我已經發動過效果的卡：{}。\n", used.join("、")));
        }
        let once: Vec<String> = mine
            .iter()
            .filter_map(|n| crate::cards::info(all, None, n).and_then(|i| crate::cards::once_by_name(i["desc"].as_str()?)).map(|rule| format!("{n}（{rule}）")))
            .collect();
        if !once.is_empty() {
            out.push_str(&format!(
                "\n我牌組裡「這個卡名一回合只能用一次」的卡（同名的另一張這回合也不能再發動同一個效果）：\n{}\n",
                once.join("\n")
            ));
        }
        let texts = |names: &[String], most: usize| names.iter().take(most).filter_map(|n| crate::cards::text_of(all, n, 260).map(|t| format!("・{n}：{t}"))).collect::<Vec<_>>().join("\n");
        let theirs = texts(&seen, 10);
        if !theirs.is_empty() {
            out.push_str(&format!("\n對手這場出現過的卡的效果原文（斷點照這個判斷）：\n{theirs}\n"));
        }
        let held = texts(&hand, 7);
        if !held.is_empty() {
            out.push_str(&format!("\n我手上的牌（上一張畫面認的）的效果原文：\n{held}\n"));
        }
        out
    })
    .unwrap_or_default()
}

fn match_known() -> String {
    let m = MATCH.lock().unwrap_or_else(|e| e.into_inner());
    m.as_ref().and_then(|m| m.research.as_ref()).filter(|(_, v)| v.is_object()).map(|(_, v)| v.to_string()).unwrap_or_default()
}

// An answer taken in: the cards seen kept, whose turn, a fresh duel; the deck
// guessed looked up when it is new.
fn take_in(sh: &Arc<Shared>, found: &Value) {
    let deck = found["deck"].as_str().unwrap_or("").trim().to_string();
    let look_up = {
        let mut m = MATCH.lock().unwrap_or_else(|e| e.into_inner());
        let m = m.get_or_insert_with(Match::default);
        m.at = crate::now_ms();
        if found["fresh"] == true && !m.seen.is_empty() {
            let frame = std::mem::take(&mut m.frame);
            *m = Match { frame, looked: m.looked, ..Match::default() };
            SAID.lock().unwrap_or_else(|e| e.into_inner()).clear();
        }
        for c in found["seen"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            let c = c.trim();
            if !c.is_empty() && !c.starts_with('?') && !m.seen.iter().any(|s| s == c) {
                m.seen.push(c.to_string());
            }
        }
        m.mine = found["turn"] == "mine";
        // A new turn: nothing used in it yet.
        if let Some(n) = found["turnNo"].as_i64().filter(|n| *n > 0) {
            if n != m.turn {
                m.turn = n;
                m.used.clear();
            }
        }
        for c in found["used"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            let c = c.trim();
            if !c.is_empty() && !c.starts_with('?') && !m.used.iter().any(|u| u == c) {
                m.used.push(c.to_string());
            }
        }
        let hand: Vec<String> = found["myHand"].as_array().into_iter().flatten().filter_map(Value::as_str).filter(|c| !c.starts_with('?')).map(str::to_string).collect();
        if !hand.is_empty() {
            m.hand = hand;
        }
        let sure = found["sure"] != "低";
        if !deck.is_empty() && sure {
            m.deck = deck.clone();
        }
        let new = !m.deck.is_empty() && m.research.as_ref().is_none_or(|(d, _)| *d != m.deck);
        if new {
            m.research = Some((m.deck.clone(), Value::Null));
        }
        new.then(|| (m.deck.clone(), m.seen.clone()))
    };
    if let Some((deck, seen)) = look_up {
        research(sh, deck, seen);
    }
}

// The opponent's deck looked up on the web (kept a week in duel/research/).
fn research(sh: &Arc<Shared>, deck: String, seen: Vec<String>) {
    let sh = sh.clone();
    crate::spawn_named("duel-research", move || {
        let name: String = deck.chars().filter(|c| !"\\/:*?\"<>|".contains(*c)).collect();
        let file = dir(&sh).join("research").join(format!("{name}.json"));
        let fresh = std::fs::metadata(&file).and_then(|m| m.modified()).ok().is_some_and(|t| t.elapsed().unwrap_or_default() < Duration::from_secs(7 * 24 * 3600));
        let found = if fresh {
            std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok())
        } else {
            let prompt = format!(
                "遊戲王 Master Duel：我的對手用的應該是「{deck}」（這場看過的卡：{}）。用 WebSearch 查這副牌目前在 Master Duel 的構築和打法（最多查 3 次，日文、英文的資料都可以），\
                 只回一個 JSON，不要其他文字：{{\"deck\":\"牌組名（台灣繁中）\",\"plan\":\"它怎麼展開，一兩句\",\"keys\":[\"展開的關鍵卡\"],\
                 \"chokes\":[{{\"card\":\"最該斷的卡\",\"effect\":\"哪個效果\",\"why\":\"斷了會怎樣，20字內\"}}],\"traps\":[\"它常帶的手坑、泡影類\"]}}\n\
                 chokes 最多 4 個、照重要程度排；卡名用台灣 Master Duel 繁體中文官方譯名。",
                seen.join("、")
            );
            let got = run_in(&sh, "research", &prompt, TIMEOUT, true).ok().and_then(|t| answer_of(&t));
            if let Some(v) = &got {
                let _ = std::fs::create_dir_all(file.parent().unwrap_or(&file));
                let _ = std::fs::write(&file, v.to_string());
            }
            got
        };
        sh.log(&format!("duel: {deck} looked up ({})", if found.is_some() { "found" } else { "nothing" }));
        {
            let mut m = MATCH.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(m) = m.as_mut().filter(|m| m.research.as_ref().is_some_and(|(d, _)| *d == deck)) {
                m.research = Some((deck, found.unwrap_or(json!({ "failed": true }))));
            }
        }
        push(&sh);
    });
}

// The first JSON object in what was written, whatever it holds.
fn answer_of(text: &str) -> Option<Value> {
    let start = text.find('{')?;
    let mut depth = 0;
    for (i, c) in text[start..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return serde_json::from_str(&text[start..start + i + 1]).ok();
                }
            }
            _ => {}
        }
    }
    None
}

fn watch_view() -> Value {
    let on = WATCH.load(Ordering::SeqCst);
    let m = MATCH.lock().unwrap_or_else(|e| e.into_inner());
    let Some(m) = m.as_ref().filter(|_| on) else { return json!({ "on": false }) };
    json!({
        "on": true,
        "resting": m.mine,
        "seen": m.seen,
        "deck": m.deck,
        "research": m.research.as_ref().map(|(d, v)| json!({ "deck": d, "found": v })),
    })
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

// What the look asks first, all that a duel cannot wait for: whose turn,
// my hand, the opponent's cards zone by zone, its deck, the chain asked
// about, what to stop (2026-10-23: one look took 21 to 47 s, most of it
// Claude thinking and writing the four's lines and the plans for later;
// those come after, from `prompt_more`, the panel showing this meanwhile).
// The pictures are in the question, in this order: the game whole, the
// opponent's zones cut out larger, my hand.
pub fn prompt_of(hand: &str, deck: &str, last: Option<&Value>, known: &str) -> String {
    let mut p = String::from(
        "你是遊戲王 Master Duel 的對戰助手。附了三張圖：\n\
         圖 1：遊戲畫面整張。\n\
         圖 2：對手那半場一格一格切出來放大的（看卡、看數字以這張為準）：上面一排 6 直行，前 5 行上面有黃色編號 1～5，\
         每一行的上半是對手的魔法陷阱區（S1～S5），下半是它的怪獸區（M1～M5），怪獸上方的白色數字是「攻擊力/守備力」、星星旁的數字是星數；\
         第 6 行（沒有編號）是場地區 F。下面一排左邊是中間左邊的額外怪獸區 EL，右邊是 ER。\n\
         圖 3：畫面最下面，我的手牌。\n\n\
         【如果這是牌組編輯畫面】（上面主要牌組、下面額外牌組的卡片格子）：只回 {\"screen\":\"deck\"}，不用認卡（之後會一張一張切出來再認）。\n\n\
         【如果這是對戰畫面】先搞清楚誰是誰（曾把我自己的卡全當成對手的，整場判斷都錯）：\n\
         - 我：左下角的頭像、名字和 LP；對手：右上角的頭像、名字和 LP。畫面上半是對手的場地，下半是我的。\n\
         - 誰的回合：右邊中間的六角形寫著「Turn 數字」和階段（Draw、Standby、Main1、Battle、Main2、End）。藍色、金邊的是我的回合，紅色的是對手的回合。\n\
         - 畫面下方跳出的選擇視窗（「選擇…」「釋放…」「請選擇要發動的…」「從牌組加入手牌」）是我正在操作：裡面的卡都是我的。\n\
         - 左邊的卡片說明只是游標指著的那張，可能是我的也可能是對手的：看那張卡在哪一半場；在我的牌組清單裡、又不在對手那半場的，當成我的。\n\
         - 右邊打開著的決鬥紀錄（一條一條的卡名和「發動」「特殊召喚」「加入」）寫著卡名：認卡以它為準；每條旁邊的頭像是誰做的。\n\
         - 「「某卡」已發動，是否連鎖」是在問我要不要回應，那張卡可能是任何一方發動的：我的回合、又是我牌組裡的卡，多半是我自己發動的。\n\
         - 畫面中間的大卡片特寫和 CHAIN 數字是正在發動的卡，誰發動的要照上面的方式判斷。\n\
         - 只有確定是對手的卡（在上半場、在對手的墓地或除外、或確定是對手發動的）才能寫進 seen、field，拿來判斷對手的牌組。看不出是誰的就不要算。\n\
         對手的格子：魔法陷阱區 S1～S5、怪獸區 M1～M5（左到右，和圖 2 的編號一樣）、場地區 F、額外怪獸區 EL、ER；在手牌、墓地、除外或連鎖中的寫 hand、gy、ban、chain。\n\
         請：\n\
         0. 認出我現在手上的每一張牌（myHand，看圖 3），和我場上蓋著、能用的卡。認不出的寫「?」，不要用牌組清單去猜。\n\
         1. 一格一格認對手場上表側的卡（field，看圖 2）：卡名用台灣 Master Duel 的繁體中文官方譯名（不確定就寫日文或英文名），\
         怪獸照抄白色的攻擊力、守備力和星數（看不清楚就不寫）。蓋著的卡寫 \"蓋牌\"。\n\
         2. 判斷對手是什麼牌組，憑你自己對 Master Duel 的了解（不能上網，要快：玩家正在等）。\n\
         3. 告訴我要斷哪裡：關鍵展開點、哪張卡的哪個效果、什麼時機、在哪一格。\
         「用什麼斷」（with）只能寫我現在手上或場上真的有的卡（第 0 步認出來的）；\
         我手上沒有能斷它的，with 寫「手上沒有」，why 說我這回合怎麼撐過去。絕對不要叫我用手上沒有的卡。\n\
         4. 正在問「是否連鎖」時：說要不要連鎖、用哪張——同樣只能用我手上或場上有的。\n",
    );
    if !deck.is_empty() {
        p.push_str(&format!("\n我的牌組（認手牌時用來對卡名；牌組裡有不代表現在手上有）：\n{deck}\n"));
    }
    if !hand.is_empty() {
        p.push_str(&format!("\n我的牌組帶的手坑／斷點卡：{hand}。只有我現在手上（或場上）真的有的才能叫我用。\n"));
    }
    if !known.is_empty() {
        p.push_str(&format!("\n對手這副牌的資料（剛上網查的，判斷要斷哪裡時參考）：\n{known}\n"));
    }
    if let Some(last) = last {
        p.push_str(&format!(
            "\n這一場之前判斷對手可能是「{}」（到目前看過的對手卡：{}）。這張是同一場的新畫面：一起考慮，更新判斷；之前看過的卡也算對手有的。\n",
            last["deck"].as_str().unwrap_or("?"),
            last["seen"].as_array().map(|s| s.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("、")).unwrap_or_default()
        ));
    }
    p.push_str(
        "\n對戰畫面只回一個 JSON，不要其他文字，每個字串都短：\n\
         {\"screen\":\"duel\",\"turn\":\"theirs|mine|?\",\"turnNo\":3,\"used\":[\"這回合我已經發動過效果的卡\"],\"fresh\":false,\"myHand\":[\"我手上的牌，認不出寫?\"],\"mySet\":[\"我場上蓋著能用的卡\"],\
         \"field\":[{\"zone\":\"M3\",\"name\":\"卡名\",\"atk\":3200,\"def\":800,\"level\":8}],\
         \"deck\":\"對手牌組名（繁中）\",\"deckAlt\":\"日文或英文名\",\"sure\":\"高|中|低\",\"seen\":[\"看到的對手卡名\"],\"plan\":\"它想做什麼，一句話\",\
         \"chain\":{\"card\":\"對手正在發動的卡\",\"answer\":\"要不要連鎖、用哪張，一句話\"},\
         \"stop\":[{\"card\":\"要斷的卡\",\"zone\":\"M3|S1|F|EL|hand|gy|chain…\",\"effect\":\"哪個效果\",\"with\":\"用什麼斷\",\"when\":\"時機\",\"why\":\"為什麼，20字內\"}]}\n\
         turnNo 是六角形上的回合數字。used：這回合我已經發動過效果的卡（看連鎖、我的墓地、場上、決鬥紀錄和下面附的紀錄）。\
         一回合一次的規則：卡名寫著「這個卡名…1回合只能使用1次」的，這回合我已經用過一張，手上同名的另一張這回合就不能再發動同一個效果，絕對不要建議；\
         只寫「1回合1次」沒寫卡名的，另一張可以。\
         斷點：照下面附的對手卡效果原文，斷它真正的展開效果（檢索、從牌組特召、送墓的那個），說清楚是第幾個效果、在哪個時機發動。\
         turn 是現在輪到誰：對手的回合 theirs、我的回合 mine、看不出來 ?。\
         fresh：這張看起來是一場剛開始的對決（第 1 回合、雙方 8000 分、場上幾乎沒卡）寫 true。\
         field 只寫對手場上有卡的格子；沒在問連鎖時 chain 寫 null；stop 最多 3 個，照重要程度排。\
         看不出對手的卡時 deck 寫空字串、plan 說看到了什麼。全部繁體中文、台灣用語。\n",
    );
    p
}

// What comes after the first answer, from it alone (no pictures, so quick):
// how she plays on (going first, second, this turn's steps), what to watch,
// a tip.
pub fn prompt_more(found: &Value, deck: &str, notes: &str) -> String {
    let mut p = String::from(
        "你是遊戲王 Master Duel 的對戰助手。另一位助手剛看了遊戲截圖，整理成下面的 JSON（myHand 是我手上的牌、field 和 seen 是對手的卡、stop 是要斷的地方）。\
         不要使用任何工具，照這些資料和你對 Master Duel 的了解補上：\n\
         1. 我這邊：對上這個牌組先攻、後攻各該怎麼打；看得到我的手牌時，照我手上這幾張，這回合具體怎麼做（幾個步驟，只能用我手上或場上有的卡）。\n\
         2. 要小心它的什麼、一句建議。\n",
    );
    p.push_str(&format!("\n=== 剛看到的 ===\n{found}\n"));
    if !deck.is_empty() {
        p.push_str(&format!("\n=== 我的牌組 ===\n{deck}\n"));
    }
    p.push_str(notes);
    p.push_str(
        "\n只回一個 JSON，不要其他文字：\n\
         {\"me\":{\"first\":\"先攻怎麼打，一句\",\"second\":\"後攻怎麼打，一句\",\"now\":[\"這回合第一步\",\"第二步\"]},\
         \"watch\":[\"要小心它的什麼，15字內\"],\"tip\":\"一句建議\"}\n\
         一回合一次的規則：卡名寫著「這個卡名…1回合只能使用1次」的，這回合已經用過一張的，同名的另一張不能再發動同一個效果，步驟裡絕對不要用。\
         看不到我的手牌時 now 寫 []。now 最多 4 步；watch 最多 3 個。全部繁體中文、台灣用語。\n",
    );
    p
}

// A deck read: Claude reads the sheet itself and may search the web (a
// theme's card list).
fn run(sh: &Shared, prompt: &str, timeout: Duration, web: bool) -> Result<String, String> {
    run_in(sh, "work", prompt, timeout, web)
}

fn run_in(sh: &Shared, sub: &str, prompt: &str, timeout: Duration, web: bool) -> Result<String, String> {
    let folder = dir(sh).join(sub);
    let _ = std::fs::create_dir_all(&folder);
    let prompt_file = folder.join("prompt.txt");
    std::fs::write(&prompt_file, prompt).map_err(|e| e.to_string())?;
    let tools: &[&str] = if web { &["-p", "--allowedTools", "Read", "WebSearch", "WebFetch"] } else { &["-p", "--allowedTools", "Read"] };
    let text = claude(&folder, tools, &prompt_file, timeout)?;
    if text.is_empty() {
        return Err("no answer".into());
    }
    Ok(text)
}

// A look: the pictures in the question itself (stream-json, base64), no
// tools, little thinking. Read one by one with Read it was a turn each,
// and Claude thought at length: 34 s for one shot, 20 s so (and one turn),
// measured 2026-10-23 on a shot of her duel.
fn see(sh: &Shared, prompt: &str, images: &[PathBuf], timeout: Duration) -> Result<String, String> {
    let folder = dir(sh).join("work");
    let _ = std::fs::create_dir_all(&folder);
    let _ = std::fs::write(folder.join(if images.is_empty() { "more.txt" } else { "prompt.txt" }), prompt);
    let mut content: Vec<Value> = images
        .iter()
        .filter_map(|p| std::fs::read(p).ok())
        .map(|png| json!({ "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": base64(&png) } }))
        .collect();
    content.push(json!({ "type": "text", "text": prompt }));
    let input = folder.join(if images.is_empty() { "more.jsonl" } else { "in.jsonl" });
    let line = json!({ "type": "user", "message": { "role": "user", "content": content } });
    std::fs::write(&input, format!("{line}\n")).map_err(|e| e.to_string())?;
    let out = claude(&folder, &["-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose", "--tools", "", "--effort", "low"], &input, timeout)?;
    // The last line says how it went and holds the answer.
    let result = out.lines().rev().filter_map(|l| serde_json::from_str::<Value>(l).ok()).find(|v| v["type"] == "result").ok_or("no answer")?;
    let text = result["result"].as_str().unwrap_or("").trim().to_string();
    if result["is_error"] == true || text.is_empty() {
        return Err(text.lines().next().unwrap_or("no answer").chars().take(120).collect());
    }
    Ok(text)
}

// Claude asked, its words on its input from a file, never on a command line
// (claude is a .cmd shim); what it wrote, whole.
fn claude(folder: &std::path::Path, args: &[&str], input: &std::path::Path, timeout: Duration) -> Result<String, String> {
    let exe = crate::run::find("claude").ok_or("claude not found")?;
    let mut cmd = std::process::Command::new(&exe);
    cmd.args(args)
        .args(crate::run::quiet(folder))
        .current_dir(folder)
        .stdin(std::fs::File::open(input).map_err(|e| e.to_string())?)
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
            Ok(None) => std::thread::sleep(Duration::from_millis(200)),
            Err(e) => return Err(e.to_string()),
        }
    };
    let text = reader.join().unwrap_or_default().trim().to_string();
    if !ok {
        return Err(text.lines().next().unwrap_or("no answer").chars().take(120).collect());
    }
    Ok(text)
}

fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            out.push(if i <= c.len() { A[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    out
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
    if w <= most || w == 0 || h == 0 {
        return (w, h, rgb.to_vec());
    }
    // To exactly `most` wide, each pixel the average of the pixels it covers.
    let (w2, h2) = (most, (h * most / w).max(1));
    let mut out = vec![0u8; w2 * h2 * 3];
    for y in 0..h2 {
        let (y0, y1) = (y * h / h2, ((y + 1) * h / h2).max(y * h / h2 + 1).min(h));
        for x in 0..w2 {
            let (x0, x1) = (x * w / w2, ((x + 1) * w / w2).max(x * w / w2 + 1).min(w));
            let mut sum = [0u32; 3];
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let at = (sy * w + sx) * 3;
                    for c in 0..3 {
                        sum[c] += rgb[at + c] as u32;
                    }
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u32;
            for c in 0..3 {
                out[(y * w2 + x) * 3 + c] = (sum[c] / n) as u8;
            }
        }
    }
    (w2, h2, out)
}

// Rows y0..y1 of a picture, whole width.
fn rows(w: usize, rgb: &[u8], y0: usize, y1: usize) -> Vec<u8> {
    rgb[y0 * w * 3..y1 * w * 3].to_vec()
}

// The opponent's side cut out zone by zone, larger (zones.png): its five
// columns (the back row over the monster zones, the ATK/DEF and stars over
// each monster), numbered 1 to 5 above, then the field zone; under them
// the two extra monster zones. As parts of the game's inside, from the
// frames' zones (src/duel/frames.js). A card on the field is some 60 px
// wide in the picture of the whole game Claude sees: its name, its numbers
// too small to read (2026-10-23).
const COLUMNS: [(f64, f64); 6] = [(0.284, 0.373), (0.370, 0.458), (0.456, 0.544), (0.542, 0.630), (0.628, 0.715), (0.713, 0.785)];
const COLUMN_Y: (f64, f64) = (0.10, 0.378);
const EXTRA: [(f64, f64, f64, f64); 2] = [(0.365, 0.38, 0.458, 0.54), (0.542, 0.38, 0.635, 0.54)];

pub fn zone_sheet(game: &crate::deckscan::Image) -> crate::deckscan::Image {
    use crate::deckscan::Image;
    let (gw, gh) = (game.w as f64, game.h as f64);
    let cut = |l: f64, t: f64, r: f64, b: f64| game.crop((l * gw) as usize, (t * gh) as usize, ((r - l) * gw) as usize, ((b - t) * gh) as usize);
    // A column some 240 px wide.
    let k = (240.0 / ((COLUMNS[0].1 - COLUMNS[0].0) * gw)).clamp(1.0, 3.0);
    let (gap, label) = (10, 26);
    let columns: Vec<Image> = COLUMNS.iter().map(|&(l, r)| cut(l, COLUMN_Y.0, r, COLUMN_Y.1).scaled(k)).collect();
    let extras: Vec<Image> = EXTRA.iter().map(|&(l, t, r, b)| cut(l, t, r, b).scaled(k)).collect();
    let w = columns.iter().map(|c| c.w + gap).sum::<usize>();
    let top = columns.iter().map(|c| c.h).max().unwrap_or(0);
    let low = extras.iter().map(|c| c.h).max().unwrap_or(0);
    let mut out = Image { w, h: label + top + gap + label + low, rgb: vec![18; w * (label + top + gap + label + low) * 3] };
    let mut x = 0;
    for (i, c) in columns.iter().enumerate() {
        if i < 5 {
            crate::deckscan::put_number(&mut out, x + c.w / 2 - 6, 3, i + 1);
        }
        out.paste(c, x, label);
        x += c.w + gap;
    }
    // EL under the second column, ER under the fourth, as on the field.
    let under = |n: usize| columns[..n].iter().map(|c| c.w + gap).sum::<usize>();
    for (e, n) in extras.iter().zip([1, 3]) {
        out.paste(e, under(n), label + top + gap + label);
    }
    out
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
        fn SetWindowPos(hwnd: isize, after: isize, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
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

    // A window of hers to the top of the windows not on top of all, not
    // activated, not waited for.
    pub fn raise(hwnd: isize) {
        const HWND_TOP: isize = 0;
        // SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS
        const FLAGS: u32 = 0x1 | 0x2 | 0x10 | 0x4000;
        // SAFETY: a window of ours, its place in the order alone.
        unsafe { SetWindowPos(hwnd, HWND_TOP, 0, 0, 0, 0, FLAGS) };
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
    pub fn raise(_hwnd: isize) {}
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
        view["fly"] = json!(FLY.load(Ordering::SeqCst));
        view["watch"] = watch_view();
        view["said"] = json!(*SAID.lock().unwrap_or_else(|e| e.into_inner()));
        let _ = sh.app.emit_to(LABEL, "duel:update", view);
    }
}

// --- Telling her what is happening ------------------------------------------------------
//
// Under the card she types what the duel is doing ("對手發動灰流麗，要連鎖嗎"),
// and is answered from it and from the last look, in words only (no
// picture to read: a chain does not wait for one). Her idea, 2026-10-23,
// after the four ("軍師團") turned out of no use to her and were taken out.

// What she said and was answered, this duel's last few: { q, state: asking | done | failed, a, since, took }.
static SAID: Mutex<Vec<Value>> = Mutex::new(Vec::new());
const SAID_KEPT: usize = 6;

pub fn say_prompt(q: &str, found: &Value, age_s: u64, said: &[Value], notes: &str, named: &str) -> String {
    let mut p = String::from(
        "你是遊戲王 Master Duel 的對戰助手。玩家正在對戰，連鎖沒時間等：他打字告訴你現在的狀況或問你，\
         用最多 3 行、80 字內回答，第一句就是結論（連不連鎖、用哪張、斷哪一個效果、現在該按什麼）。\
         只能叫他用他手上或場上真的有的卡（下面 myHand、mySet）；這個卡名一回合只能用一次、這回合已經用過的，同名的另一張不能再用。\
         不要使用任何工具，不要開場白。繁體中文、台灣 Master Duel 的卡名。\n",
    );
    if found.is_object() {
        p.push_str(&format!("\n=== 剛剛看到的場面（{age_s} 秒前的截圖，之後可能又變了，以玩家說的為準）===\n{found}\n"));
    } else {
        p.push_str("\n（還沒看過場面，只照玩家說的回答。）\n");
    }
    p.push_str(notes);
    if !named.is_empty() {
        p.push_str(&format!("\n玩家提到的卡的效果原文：\n{named}\n"));
    }
    let before: Vec<String> = said.iter().filter(|s| s["state"] == "done").map(|s| format!("玩家：{}\n你：{}", s["q"].as_str().unwrap_or(""), s["a"].as_str().unwrap_or(""))).collect();
    if !before.is_empty() {
        p.push_str(&format!("\n=== 剛才說過的 ===\n{}\n", before.join("\n")));
    }
    p.push_str(&format!("\n玩家現在說：「{q}」\n"));
    p
}

pub fn say(sh: &Arc<Shared>, q: &str) {
    let q: String = q.trim().chars().take(200).collect();
    if q.is_empty() {
        return;
    }
    let since = crate::now_ms();
    let before = {
        let mut said = SAID.lock().unwrap_or_else(|e| e.into_inner());
        let before = said.clone();
        said.push(json!({ "q": q, "state": "asking", "since": since }));
        let skip = said.len().saturating_sub(SAID_KEPT);
        said.drain(..skip);
        before
    };
    push(sh);
    let sh = sh.clone();
    crate::spawn_named("duel-say", move || {
        let (mut found, at) = {
            let view = VIEW.lock().unwrap_or_else(|e| e.into_inner());
            (view.get("found").cloned().unwrap_or(Value::Null), view["at"].as_u64().unwrap_or(0))
        };
        if let Some(o) = found.as_object_mut() {
            for k in ["screen", "deckAlt", "fresh", "me", "watch", "tip"] {
                o.remove(k);
            }
        }
        let named = crate::cards::with_cards(&sh.dir, |all| {
            crate::cards::named_in(all, &q, 4).iter().filter_map(|n| crate::cards::text_of(all, n, 260).map(|t| format!("・{n}：{t}"))).collect::<Vec<_>>().join("\n")
        })
        .unwrap_or_default();
        let age = crate::now_ms().saturating_sub(at) / 1000;
        let prompt = say_prompt(&q, &found, age, &before, &card_notes(&sh), &named);
        let got = see(&sh, &prompt, &[], TIMEOUT);
        let took = crate::now_ms().saturating_sub(since);
        sh.log(&format!("duel: told something, answered {} in {} s", if got.is_ok() { "so" } else { "not" }, took / 1000));
        {
            let mut said = SAID.lock().unwrap_or_else(|e| e.into_inner());
            let Some(entry) = said.iter_mut().find(|s| s["since"].as_u64() == Some(since)) else { return };
            *entry = match got {
                // Its words plain (it writes **bold** at times).
                Ok(a) => json!({ "q": q, "state": "done", "a": a.replace("**", ""), "since": since, "took": took }),
                Err(why) => json!({ "q": q, "state": "failed", "a": why, "since": since }),
            };
        }
        push(&sh);
    });
}

// The panel takes the keyboard while she types, and gives it back to the
// game after (the window that had it).
static TYPED_FROM: Mutex<isize> = Mutex::new(0);

pub fn typing(sh: &Shared, on: bool) {
    let Some(win) = window(sh) else { return };
    if on {
        *TYPED_FROM.lock().unwrap_or_else(|e| e.into_inner()) = crate::focus::foreground();
        let _ = win.set_focusable(true);
        let _ = win.set_focus();
    } else {
        // Given back only when it still has it (not when she clicked another window away).
        let has_it = win.hwnd().is_ok_and(|h| h.0 as isize == crate::focus::foreground());
        let _ = win.set_focusable(false);
        let from = std::mem::take(&mut *TYPED_FROM.lock().unwrap_or_else(|e| e.into_inner()));
        if from != 0 && has_it {
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
        let most = ((SIZE.1 * a.sf).round() as i32).min(gh - 2 * margin);
        let fit = FIT.load(Ordering::SeqCst) as f64;
        let h = if fit > 0.0 { ((fit * a.sf).round() as i32).clamp((BAR.1 * a.sf).round() as i32, most.max(1)) } else { most };
        (card + bar + (BAR_GAP * a.sf).round() as i32, h)
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
            .always_on_top(false)
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
            // Not the deck editor (a duel, a menu): said so, not a deck of nonsense.
            if cards.len() < 20 {
                return Err("notDeck".into());
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
            // The rest told by the official pictures when plainly one of the cards
            // likely here (art.rs): her decks', the theme's, the hand traps.
            let looks: Vec<Vec<f32>> = cards.iter().map(|(_, img)| crate::deckscan::print(img)).collect();
            let pool = crate::art::looks(&sh.dir, &likely_cards(&sh, &theme));
            let by_art: Vec<(String, String, u64)> = cards
                .iter()
                .enumerate()
                .filter(|(_, (slot, _))| !kept.iter().any(|(s, _)| s == slot) && !recalled.iter().any(|(s, _, _)| s == slot))
                .filter_map(|(i, (slot, _))| crate::art::which(&looks[i], &pool).map(|(id, _)| (slot.clone(), id)))
                .filter_map(|(slot, id)| Some((slot, name_of(&sh, id)?, id)))
                .collect();
            sh.log(&format!("duel: {} of {} cards told by their pictures ({} pictured)", by_art.len(), cards.len(), pool.len()));
            let known_names: Vec<(String, String)> = kept
                .iter()
                .cloned()
                .chain(recalled.iter().map(|(s, n, _)| (s.clone(), n.clone())))
                .chain(by_art.iter().map(|(s, n, _)| (s.clone(), n.clone())))
                .collect();
            let numbered: Vec<(usize, String)> = known_names.iter().filter_map(|(slot, name)| cards.iter().position(|(s, _)| s == slot).map(|i| (i + 1, name.clone()))).collect();
            // The theme's cards from the card database, when it has any: picked among, no web search.
            let theme_cards = if theme.is_empty() { Vec::new() } else { crate::cards::with_cards(&sh.dir, |all| crate::cards::theme_cards(all, &theme)).unwrap_or_default() };
            let prompt = crate::deckscan::prompt_of(main, cards.len() - main, &theme, &theme_cards, &numbered);
            // All known: no one asked.
            let found = if numbered.len() == cards.len() {
                json!({ "cards": [] })
            } else {
                let text = run(&sh, &prompt, DECK_TIMEOUT, true)?;
                answer_in(&text).ok_or_else(|| text.chars().take(120).collect::<String>())?
            };
            // Claude's names as the database writes them, each checked against its
            // pictures: alike, sure; plainly another of the theme it named, that one;
            // else unsure (it was sure of names whose pictures were nothing alike).
            let said: Vec<(usize, String, bool)> = found["cards"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| {
                    let n = c["n"].as_u64()? as usize;
                    let name = c["name"].as_str().unwrap_or("");
                    let name = crate::cards::with_cards(&sh.dir, |all| crate::cards::best_match(all, name)).flatten().unwrap_or_else(|| name.to_string());
                    Some((n, name, c["sure"] != false))
                })
                .collect();
            let named: Vec<(u64, String)> = crate::cards::with_cards(&sh.dir, |all| {
                said.iter().filter_map(|(_, name, _)| crate::cards::info(all, None, name)).filter_map(|i| Some((i["id"].as_u64()?, i["en"].as_str().unwrap_or("").to_string()))).collect()
            })
            .unwrap_or_default();
            let named_looks = crate::art::looks(&sh.dir, &named);
            let found_theme = found["theme"].as_str().unwrap_or("").to_string();
            let theme_pool = if found_theme.is_empty() || found_theme == theme {
                std::collections::HashMap::new()
            } else {
                crate::art::looks(&sh.dir, &likely_cards(&sh, &found_theme))
            };
            let checked: Vec<(usize, String, bool)> = said
                .into_iter()
                .map(|(n, name, sure)| {
                    let Some(look) = looks.get(n.wrapping_sub(1)) else { return (n, name, sure) };
                    let id = crate::cards::with_cards(&sh.dir, |all| crate::cards::info(all, None, &name).and_then(|i| i["id"].as_u64())).flatten();
                    match id.and_then(|id| named_looks.get(&id)).map(|arts| crate::art::score(look, arts)) {
                        Some(score) if score >= crate::art::SURE => (n, name, true),
                        alike => match crate::art::which(look, &theme_pool).and_then(|(id, _)| name_of(&sh, id)) {
                            Some(other) => (n, other, true),
                            // Its pictures not had: as Claude said.
                            None => (n, name, sure && alike.is_none()),
                        },
                    }
                })
                .collect();
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
                    if let Some((_, name, id)) = by_art.iter().find(|(s, _, _)| s == slot) {
                        return json!({ "slot": slot, "name": name, "id": id, "sure": true, "art": true });
                    }
                    match checked.iter().find(|(n, _, _)| *n == i + 1) {
                        Some((_, name, sure)) => json!({ "slot": slot, "name": name, "sure": sure }),
                        None => json!({ "slot": slot, "name": "", "sure": false }),
                    }
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

// The cards likely in her deck, to tell its slots by their pictures:
// those of her saved decks, the theme's, the hand traps (passcode, English name).
fn likely_cards(sh: &Shared, theme: &str) -> Vec<(u64, String)> {
    let d = dir(sh);
    let mut names: Vec<String> = Vec::new();
    let decks = std::fs::read_dir(decks_dir(sh)).into_iter().flatten().flatten().map(|e| e.path().join("deck.json")).chain([crate::deckscan::deck_path(&d)]);
    for file in decks {
        let deck: Value = std::fs::read_to_string(file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        names.extend(deck["cards"].as_array().into_iter().flatten().filter_map(|c| c["name"].as_str()).map(str::to_string));
    }
    crate::cards::with_cards(&sh.dir, |all| {
        if !theme.is_empty() {
            names.extend(crate::cards::theme_cards(all, theme));
        }
        let mut out: Vec<(u64, String)> = names.iter().filter_map(|n| crate::cards::info(all, None, n)).filter_map(|i| Some((i["id"].as_u64()?, i["en"].as_str().unwrap_or("").to_string()))).collect();
        out.extend(all.iter().filter(|c| c.hand).map(|c| (c.id, c.en.clone())));
        out.sort();
        out.dedup();
        out
    })
    .unwrap_or_default()
}

fn name_of(sh: &Shared, id: u64) -> Option<String> {
    crate::cards::with_cards(&sh.dir, |all| all.iter().find(|c| c.id == id).map(|c| c.name.clone())).flatten()
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
    // Three panels as the game's deck editor: about 1280 wide.
    let (mut w, mut h) = (((1280.0 * a.sf) as i32).min(a.w - 40), ((800.0 * a.sf) as i32).min(a.h - 40));
    let mut pos = (a.x + (a.w - w) / 2, a.y + (a.h - h) / 2);
    // Where she left it, while its top is still on a display.
    if let Some((x, y, sw, sh_)) = deck_spot(sh).filter(|&(x, y, sw, _)| sh.screens().near(x + sw / 2, y + 20).is_some()) {
        (pos, w, h) = ((x, y), sw.max((1000.0 * a.sf) as i32).min(a.w), sh_.max((600.0 * a.sf) as i32).min(a.h));
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
                .min_inner_size(1000.0, 600.0)
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
// The deck window's card list (cards.rs browse): what is typed, the filters,
// and, nothing asked, her deck's cards, its theme's and the hand traps.
pub fn browse_cards(sh: &Shared, q: &str, filter: &Value, names: &[String], theme: &str) -> Value {
    let got = crate::cards::with_cards(&sh.dir, |all| {
        let mut around: Vec<String> = names.to_vec();
        if !theme.trim().is_empty() {
            around.extend(crate::cards::theme_cards(all, theme));
        }
        crate::cards::browse(all, q, filter, &around, 150)
    });
    match got {
        Some(found) => json!({ "cards": found }),
        None => json!({ "cards": [], "loading": true }),
    }
}

pub fn search_cards(sh: &Shared, q: &str) -> Value {
    match crate::cards::with_cards(&sh.dir, |all| crate::cards::search(all, q, 12)) {
        Some(found) => json!({ "cards": found }),
        None => json!({ "cards": [], "loading": true }),
    }
}

// Every picture of a card, its alternate arts too.
// Every picture of a card (its alternate arts too: Master Duel has them,
// and its cards with art over the frame, the user's word, 2026-10-22).
pub fn card_arts(sh: &Shared, id: u64) -> Value {
    let en = crate::cards::with_cards(&sh.dir, |all| all.iter().find(|c| c.id == id).map(|c| c.en.clone())).flatten().unwrap_or_default();
    json!(crate::art::ids(&sh.dir, id, &en).into_iter().map(crate::cards::picture).collect::<Vec<_>>())
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
    let deck_in = deck;
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
    // Each card she marked right, its look learnt: known the next time it is
    // read. A look already known takes another name only from a card she put
    // right herself in the window this time (`taught`, its slots).
    let taught: Vec<&str> = deck_in["taught"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    let slots = dir(sh).join("slots");
    let learnt: Vec<(crate::deckscan::Known, bool)> = deck["cards"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["sure"] == true && !c["name"].as_str().unwrap_or("").is_empty())
        .filter_map(|c| {
            let img = crate::deckscan::Image::load(&slots.join(format!("{}.png", c["slot"].as_str()?)))?;
            let slot = c["slot"].as_str()?;
            Some((crate::deckscan::Known { look: crate::deckscan::print(&img), name: c["name"].as_str()?.to_string(), id: c["id"].as_u64() }, taught.contains(&slot)))
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
            .always_on_top(false)
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

// How long the frames stay on the game after a look: what they point at is
// gone a turn or two later (they stayed after the duel was over, 2026-10-23).
const FRAMES_FOR: u64 = 120_000;
static FRAMES_ON: AtomicBool = AtomicBool::new(false);
static FRAMES_WATCH: AtomicBool = AtomicBool::new(false);

// Every 150 ms while she has the bar up: the game come to the front, the
// bar and the frames raised over it. They are not on top of everything:
// over her other windows they were in the way, and put away whenever the
// game was not in front they vanished from over a game still in sight and
// came back as if opened anew (her words, 2026-10-23). So they sit just
// over the game, and another window over the game is over them too. Each
// second: the frames off once stale, the bar gone with the game.
const TICK: Duration = Duration::from_millis(150);

fn watch_frames(sh: &Arc<Shared>) {
    if FRAMES_WATCH.swap(true, Ordering::SeqCst) {
        return;
    }
    let sh = sh.clone();
    crate::spawn_named("duel-frames", move || {
        let (mut game, mut was_front, mut ticks) = (0isize, false, 0u32);
        loop {
            std::thread::sleep(TICK);
            if !SHOWN.load(Ordering::SeqCst) {
                was_front = false;
                continue;
            }
            ticks += 1;
            if ticks % 7 == 1 {
                let Some(pid) = game_pid() else {
                    show(&sh, false);
                    continue;
                };
                game = os::game_window(pid).map_or(0, |(h, _)| h);
                let want = FLY.load(Ordering::SeqCst);
                if FRAMES_ON.load(Ordering::SeqCst) != (want && frames_fresh()) {
                    apply_frames(&sh, want);
                }
            }
            let front = game != 0 && crate::focus::foreground() == game;
            if front && !was_front {
                for win in [frames_window(&sh), window(&sh)].into_iter().flatten() {
                    if let Ok(h) = win.hwnd() {
                        os::raise(h.0 as isize);
                    }
                }
            }
            was_front = front;
        }
    });
}

fn frames_fresh() -> bool {
    let at = VIEW.lock().unwrap_or_else(|e| e.into_inner())["at"].as_u64().unwrap_or(0);
    crate::now_ms().saturating_sub(at) < FRAMES_FOR
}

// The frames: on the game's inside while the panel is up with an answer.
fn apply_frames(sh: &Arc<Shared>, want: bool) {
    watch_frames(sh);
    let want = want && frames_fresh();
    let zones = VIEW.lock().unwrap_or_else(|e| e.into_inner()).get("found").map(zones_of).unwrap_or(json!([]));
    let any = zones.as_array().is_some_and(|z| !z.is_empty());
    FRAMES_ON.store(want, Ordering::SeqCst);
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
                let sh = sh.clone();
                std::thread::spawn(move || {
                    for _ in 0..3 {
                        std::thread::sleep(Duration::from_millis(200));
                        // Where it should be now (the card may have told its height meanwhile).
                        let Some((pos, size, _)) = rect(&sh) else { return };
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
    if !on {
        WATCH.store(false, Ordering::SeqCst);
    }
    apply(sh);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_made_smaller_to_what_claude_sees_whole() {
        // 2048 wide: to 1568, not halved to 1024.
        let (w, h) = (2048, 1152);
        let rgb: Vec<u8> = (0..w * h).flat_map(|i| { let v = (i % w * 255 / w) as u8; [v, v, v] }).collect();
        let (w2, h2, small) = shrink(w, h, &rgb, MAX_WIDTH);
        assert_eq!((w2, h2), (1568, 882));
        assert_eq!(small.len(), w2 * h2 * 3);
        // Left dark, right light, as it was.
        assert!(small[0] < 5 && small[(w2 - 1) * 3] > 250);
        assert_eq!(shrink(800, 450, &vec![7; 800 * 450 * 3], MAX_WIDTH).0, 800);
        assert_eq!(rows(4, &(0..48).collect::<Vec<u8>>(), 1, 3).len(), 24);
    }

    #[test]
    fn what_claude_is_asked() {
        let p = prompt_of("", "", None, "");
        assert!(p.contains("圖 2") && p.contains("圖 3") && p.contains("\"stop\"") && p.contains("\"field\"") && !p.contains("我的牌組帶的"));
        assert!(!p.contains("\"me\""), "that comes after");
        let last = json!({ "deck": "蛇眼", "seen": ["蛇眼灰燼", "原罪寶-蛇眼"] });
        let p = prompt_of("灰流麗、增殖的G", "巳劍：天叢雲之巳劍×2", Some(&last), "{\"deck\":\"蛇眼\"}");
        assert!(p.contains("剛上網查的") && p.contains("\"turn\"") && p.contains("fresh"));
        assert!(p.contains("\"turnNo\"") && p.contains("\"used\"") && p.contains("同名的另一張這回合就不能再發動"));
        assert!(p.contains("牌組裡有不代表現在手上有") && p.contains("天叢雲之巳劍×2") && p.contains("\"zone\""));
        assert!(p.contains("我的牌組帶的手坑／斷點卡：灰流麗、增殖的G。只有我現在手上"));
        assert!(p.contains("\"myHand\"") && p.contains("手上沒有"));
        assert!(p.contains("可能是「蛇眼」（到目前看過的對手卡：蛇眼灰燼、原罪寶-蛇眼）"));
        let m = prompt_more(&json!({ "myHand": ["灰流麗"] }), "巳劍", "\n這回合用過：x\n");
        assert!(m.contains("\"now\"") && m.contains("灰流麗") && m.contains("這回合用過：x") && m.contains("不要使用任何工具") && !m.contains("crew"));
    }

    #[test]
    fn what_she_tells_it_asked_with_the_last_look() {
        let said = [json!({ "q": "要連鎖嗎", "a": "不用", "state": "done" }), json!({ "q": "還在想", "state": "asking" })];
        let p = say_prompt("對手發動灰流麗", &json!({ "myHand": ["無限泡影"] }), 12, &said, "\n這回合用過：x\n", "・灰流麗：①：…");
        assert!(p.contains("12 秒前") && p.contains("無限泡影") && p.contains("這回合用過：x") && p.contains("・灰流麗：①"));
        assert!(p.contains("玩家：要連鎖嗎\n你：不用") && !p.contains("還在想"), "only what was answered");
        assert!(p.ends_with("玩家現在說：「對手發動灰流麗」\n"));
        let p = say_prompt("x", &Value::Null, 0, &[], "", "");
        assert!(p.contains("還沒看過場面") && !p.contains("提到的卡"));
    }

    #[test]
    fn pictures_in_the_question() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64(&[0xff, 0xfe]), "//4=");
    }

    #[test]
    fn the_opponent_s_zones_cut_out_larger() {
        let (w, h) = (2048, 1152);
        let game = crate::deckscan::Image { w, h, rgb: vec![90; w * h * 3] };
        let z = zone_sheet(&game);
        // Five columns and the field zone some 240 px each, two rows.
        assert!(z.w > 1300 && z.w < 1700, "{}", z.w);
        assert!(z.h > 500 && z.h < 900, "{}", z.h);
        assert_eq!(z.rgb.len(), z.w * z.h * 3);
        // A small game: no smaller than it is.
        let small = crate::deckscan::Image { w: 1280, h: 720, rgb: vec![90; 1280 * 720 * 3] };
        assert!(zone_sheet(&small).w >= 1300);
    }

    #[test]
    fn names_put_right_by_the_database() {
        let all = crate::cards::parse(
            &json!({
                "1": { "id": 14558127, "md_name": "灰流丽", "en_name": "Ash Blossom & Joyous Spring", "data": { "type": 4129, "atk": 0, "def": 1800, "level": 3 } },
                "2": { "id": 19899073, "md_name": "天丛云之巳剑", "en_name": "Ame no Murakumo no Mitsurugi", "data": { "type": 161, "atk": 3200, "def": 800, "level": 8 } },
                "3": { "id": 1, "md_name": "别的巳剑", "en_name": "Other Mitsurugi", "data": { "type": 33, "atk": 1600, "def": 400, "level": 4 } },
                "4": { "id": 2, "md_name": "路人", "en_name": "Someone", "data": { "type": 33, "atk": 1600, "def": 400, "level": 4 } }
            })
            .to_string(),
        );
        let mut found = json!({
            "deck": "巳劍",
            "seen": ["Ash Blossom & Joyous Spring"],
            "field": [
                { "zone": "M3", "name": "天雲之劍", "atk": 3200, "def": 800, "level": 8 },
                { "zone": "M4", "name": "?", "atk": 1600, "def": 400, "level": 4 },
                { "zone": "S2", "name": "蓋牌" },
                { "zone": "M5", "name": "灰流麗", "atk": 3000, "def": 0 }
            ],
            "stop": [{ "card": "Ame no Murakumo no Mitsurugi", "zone": "M2" }, { "card": "別的巳劍", "zone": "gy" }]
        });
        settle(&all, &mut found, "");

        let f = &found["field"];
        assert_eq!((f[0]["name"].as_str(), f[0]["by"].as_str()), (Some("天叢雲之巳劍"), Some("stats")), "only one card is 3200/800 ★8");
        assert_eq!(f[1]["name"], "別的巳劍", "two cards are 1600/400 ★4: the deck's one");
        assert_eq!(f[2]["name"], "蓋牌");
        assert_eq!(f[3]["name"], "灰流麗", "numbers no card has (changed by an effect): as said");
        assert_eq!(found["stop"][0]["card"], "天叢雲之巳劍");
        assert_eq!(found["stop"][0]["zone"], "M3", "where the field was read to hold it");
        assert_eq!(found["stop"][1]["zone"], "gy", "another copy, in the graveyard");
        assert_eq!(found["seen"], json!(["灰流麗", "天叢雲之巳劍", "別的巳劍"]));
        // No deck this look: the duel's guess before.
        let mut found = json!({ "deck": "", "field": [{ "zone": "M1", "name": "?", "atk": 1600, "def": 400, "level": 4 }] });
        settle(&all, &mut found, "巳劍");
        assert_eq!(found["field"][0]["name"], "別的巳劍");
        let mut found = json!({ "deck": "", "field": [{ "zone": "M1", "name": "?", "atk": 1600, "def": 400, "level": 4 }] });
        settle(&all, &mut found, "");
        assert_eq!(found["field"][0]["name"], "?", "two cards fit, nothing to choose by");
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

