// Where her settings and pets live, the settings themselves, and the pets
// found.
//
// Her folder: WAKUWAKU_USER_DATA (tests), else data/ in the project when run
// from this source tree, else wakuwaku-data beside the .exe (portable).
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

const SOURCE_TREE: &str = env!("CARGO_MANIFEST_DIR");

// The sizes she comes in.
pub const SCALES: [(&str, f64); 3] = [("small", 0.4), ("medium", 0.55), ("large", 0.75)];

// The source tree's path this many levels up, then `rest`: no "..", which
// the asset protocol's scope would not match.
fn in_tree(up: usize, rest: &str) -> PathBuf {
    let tree = Path::new(SOURCE_TREE);
    tree.ancestors().nth(up).unwrap_or(tree).join(rest)
}

pub fn folder() -> PathBuf {
    if let Ok(dir) = std::env::var("WAKUWAKU_USER_DATA") {
        return dir.into();
    }
    if from_source() {
        return in_tree(1, "data");
    }
    let exe = std::env::current_exe().unwrap_or_default();
    exe.parent().map(|p| p.join("wakuwaku-data")).unwrap_or_else(|| "wakuwaku-data".into())
}

// Run from the source tree it was built in (its target folder), not a
// downloaded exe: its data stays in the tree, and it updates by a build.
// Quit by the person (her menu's Quit, from the tray or the taskbar): marked
// in her data folder, so that a session's hook (Claude Code's SessionStart,
// Codex's) does not start her again (it did, at once: a resume, /clear and
// a compaction start a session too) until she is started otherwise: opened
// by hand, at login, by an update, which every start of hers clears. A
// crash leaves no mark: the hooks start her again then, as ever. /quit
// (scripts, a new build put in her place, which starts her itself) leaves
// none either.
const QUIT_MARK: &str = "quit-by-choice";

pub fn mark_quit(dir: &Path) {
    let _ = fs::write(dir.join(QUIT_MARK), "quit from her menu: a session's hook does not start her until she is opened again\n");
}

pub fn clear_quit(dir: &Path) {
    let _ = fs::remove_file(dir.join(QUIT_MARK));
}

// Whether a session's hook may start her (none answering on her port).
pub fn may_start(dir: &Path) -> bool {
    !dir.join(QUIT_MARK).exists()
}

pub fn from_source() -> bool {
    std::env::current_exe().unwrap_or_default().starts_with(Path::new(SOURCE_TREE).join("target"))
}

// --- Settings ----------------------------------------------------------------------

fn defaults() -> Map<String, Value> {
    let Value::Object(map) = json!({
        // Traditional Chinese unless set otherwise (this fork's own; "auto"
        // goes by the system's language, i18n.rs).
        "lang": "zh-TW",
        "pet": "claude-chan",
        // A pet of its own for an agent's sessions ({ claude: id, ... });
        // one not set, or gone, is the pet above (pet_for).
        "agentPets": {},
        "scale": 0.55,
        // Her home: the island; out, she is on the desktop.
        "display": "island",
        "out": false,
        // The compact island's width: narrow, normal or wide (island.js WIDTHS).
        "islandWidth": "normal",
        // OLED's guard, on unless turned off: the island a screen pixel off
        // by turns (island.js oledShift).
        "oledShift": true,
        // Her updates looked for as she starts and once a day (updates.rs;
        // a downloaded exe only); downloaded from Gitee first in China and
        // GitHub first elsewhere, or as set.
        "autoUpdateCheck": true,
        "updateFrom": "auto",
        "details": true,
        // Widgets (plugins in the island): which are off, their order, how
        // often the island turns to the next, whether they may open it.
        "widgetsOff": ["monitor"],
        // The monitor's parts, and how often it reads (seconds, 1 to 10).
        "monitor": { "cpu": true, "mem": true, "net": true, "battery": true, "every": 2, "pin": true },
        "widgetOrder": [],
        "widgetSpin": 8,
        "widgetNudge": true,
        // The plugins she runs (scripts.rs): { id: { on, <Param>: value } }.
        "plugins": {},
        "walk": true,
        "look": true,
        // Done, changed or failed: shown 8 s, or until seen (8, 30, 120, "seen").
        "hold": 8,
        // A session at rest out of sight after this many minutes with nothing
        // new from it (15, 30, 60, 120; 0 never: state.rs is_shown).
        "idleHideMin": crate::state::IDLE_HIDE_MIN,
        "notify": { "waiting": false, "done": false, "error": false },
        "sound": false,
        "promptWaitSec": 290,
        "dnd": false,
        "hideInFullscreen": true,
        "onboarded": false,
    }) else {
        unreachable!()
    };
    map
}

pub fn load(dir: &Path) -> Map<String, Value> {
    let mut settings = defaults();
    if let Ok(Value::Object(saved)) = fs::read_to_string(dir.join("config.json")).map_err(|_| ()).and_then(|t| serde_json::from_str(&t).map_err(|_| ())) {
        settings.extend(saved);
    }
    // The homes of earlier versions are the island now: the capsule, the
    // corner, the top bar and the taskbar; the pet on her own too, with her
    // out of it.
    match settings.get("display").and_then(Value::as_str) {
        Some("pet") => {
            settings.insert("display".into(), json!("island"));
            settings.insert("out".into(), json!(true));
        }
        Some("capsule" | "corner" | "bar" | "taskbar") => {
            settings.insert("display".into(), json!("island"));
        }
        _ => {}
    }
    for gone in GONE {
        settings.remove(*gone);
    }
    adopt_monitor(&mut settings);
    settings
}

// Settings of what she no longer has: the bubble and the corner; the
// taskbar home, its look, layout, tray and pins, and her island's look on
// it; the mail; the Windows look and wallpaper pages kept nothing.
const GONE: &[&str] = &[
    "corner",
    "bubble",
    "taskbarMaterial",
    "taskbarDisplays",
    "taskbarClearWhen",
    "taskbarButtons",
    "taskbarAlign",
    "taskbarEdge",
    "taskbarLayout",
    "taskbarOrder",
    "taskbarGear",
    "taskbarPinsImported",
    "taskbarPinned",
    "trayPinned",
    "trayOrder",
    "islandLook",
    "lookDefaults",
    "mail",
    "mailAgent",
    "mailAgentConf",
    "mailPanes",
    "mailOffline",
    "mailProxy",
];

// CPU · memory, the network and the battery were widgets of their own: now
// they are the monitor's parts. It is on if any of them was, with those
// parts (all of them, if none was); it takes the first one's place in the order.
fn adopt_monitor(settings: &mut Map<String, Value>) {
    const OLD: [&str; 3] = ["sys", "net", "battery"];
    let list = |v: Option<&Value>| -> Vec<String> { v.and_then(Value::as_array).into_iter().flatten().filter_map(|s| s.as_str().map(String::from)).collect() };
    let off = list(settings.get("widgetsOff"));
    if !off.iter().any(|id| OLD.contains(&id.as_str())) {
        return;
    }
    let on = |id: &str| !off.iter().any(|o| o == id);
    let any = OLD.iter().any(|id| on(id));
    settings.insert("monitor".into(), json!({ "cpu": on("sys") || !any, "mem": on("sys") || !any, "net": on("net") || !any, "battery": on("battery") || !any, "every": 2 }));
    let mut off: Vec<String> = off.into_iter().filter(|id| !OLD.contains(&id.as_str())).collect();
    if !any {
        off.push("monitor".into());
    }
    settings.insert("widgetsOff".into(), json!(off));
    let mut order = Vec::new();
    for id in list(settings.get("widgetOrder")) {
        let id = if OLD.contains(&id.as_str()) { "monitor".to_string() } else { id };
        if !order.contains(&id) {
            order.push(id);
        }
    }
    settings.insert("widgetOrder".into(), json!(order));
}

pub fn save(dir: &Path, settings: &Map<String, Value>) {
    let _ = fs::create_dir_all(dir);
    // A settings file we cannot write only costs the remembered settings.
    let _ = fs::write(dir.join("config.json"), serde_json::to_string_pretty(settings).unwrap_or_default());
}

// --- Pets ----------------------------------------------------------------------------

#[derive(Clone)]
pub struct PetInfo {
    pub id: String,
    pub name: String,
    pub author: String,
    pub sheet: PathBuf,
    pub version: u8,
    // One of the pets Codex has (its desktop app keeps them in the same format).
    pub from_codex: bool,
}

// The pets Codex has: only read, never written.
pub fn codex_pets() -> PathBuf {
    crate::connection::codex_home().join("pets")
}

// Downloaded pets: <her folder>/pets, then the project's pets/ (from source),
// then Codex's.
pub fn pet_dirs(dir: &Path) -> Vec<PathBuf> {
    vec![dir.join("pets"), in_tree(1, "pets"), codex_pets()]
}

// The pets, as last found, until one of their folders changes (a pet added
// or taken away changes its folder's time), or she puts one in herself
// (forget_pets). Asked for on every redraw: the folders were read through
// each time.
type Stamps = Vec<(PathBuf, Option<std::time::SystemTime>)>;
static PETS: std::sync::Mutex<Option<(u64, Stamps, Vec<PetInfo>)>> = std::sync::Mutex::new(None);
static PETS_PUT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

// A pet put in by her (fetch.rs put_pet): what was found is looked for anew.
// The folder's time alone may not say so: it moves on by the clock's tick
// (some 15 ms), and two changes within one look the same. Counted, so a look
// under way as she puts it in is not kept either.
pub fn forget_pets() {
    PETS_PUT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

// What the pets found are kept by: each folder the pets are in, each pet's
// folder in them and its two files, by path and time. A pet's folder made
// and its sheet written a moment after (tools/slice_sheet.py, a copy by
// hand) changes only the pet's own folder, not the one it is in: by the
// folders alone, a look between the two kept it out until she restarted.
// By path too: two data folders (tests) of the same times are not one.
fn stamps_of(dir: &Path) -> Stamps {
    let time = |p: &Path| fs::metadata(p).and_then(|m| m.modified()).ok();
    let mut stamps = Vec::new();
    for root in pet_dirs(dir) {
        stamps.push((root.clone(), time(&root)));
        let Ok(entries) = fs::read_dir(&root) else { continue };
        let mut pets: Vec<PathBuf> = entries.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| e.path()).collect();
        pets.sort();
        for pet in pets {
            for p in [pet.join("spritesheet.webp"), pet.join("pet.json"), pet] {
                let t = time(&p);
                stamps.push((p, t));
            }
        }
    }
    stamps
}

pub fn pets(dir: &Path) -> Vec<PetInfo> {
    let put = PETS_PUT.load(std::sync::atomic::Ordering::SeqCst);
    let stamps = stamps_of(dir);
    let mut kept = PETS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((was, at, found)) = kept.as_ref() {
        if *was == put && *at == stamps {
            return found.clone();
        }
    }
    let found = scan_pets(dir);
    *kept = Some((put, stamps, found.clone()));
    found
}

fn scan_pets(dir: &Path) -> Vec<PetInfo> {
    let mut found: Vec<PetInfo> = Vec::new();
    let codex = codex_pets();
    for root in pet_dirs(dir) {
        let Ok(entries) = fs::read_dir(&root) else { continue };
        let mut ids: Vec<String> = entries
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .map(|e| e.file_name().to_string_lossy().into_owned())
            // Not one being put in place (fetch.rs put_pet: .<id>.new).
            .filter(|id| !id.starts_with('.'))
            .collect();
        ids.sort();
        for id in ids {
            let sheet = root.join(&id).join("spritesheet.webp");
            if found.iter().any(|p| p.id == id) || !sheet.exists() {
                continue;
            }
            let meta: Value = fs::read_to_string(root.join(&id).join("pet.json"))
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or(Value::Null);
            found.push(PetInfo {
                name: meta.get("displayName").and_then(Value::as_str).unwrap_or(&id).to_string(),
                author: meta.get("author").and_then(Value::as_str).unwrap_or("").to_string(),
                version: if meta.get("spriteVersionNumber").and_then(Value::as_u64) == Some(2) { 2 } else { 1 },
                id,
                sheet,
                from_codex: root == codex,
            });
        }
    }
    found
}

// The agents a pet may be set for, as sessions name them (Claude Code's
// have none: claude).
pub const AGENTS: [&str; 4] = ["claude", "codex", "agy", "grok"];

// The agent of a session as the pages draw it (state.rs view), by the
// name agentPets keeps it under: none for no session at all.
pub fn agent_key(session: &Value) -> Option<&str> {
    if session.get("id").and_then(Value::as_str).unwrap_or("").is_empty() {
        return None;
    }
    match session.get("agent").and_then(Value::as_str).unwrap_or("") {
        "" => Some("claude"),
        agent => Some(agent),
    }
}

// The pet a session is drawn as: its agent's own (agentPets), else the
// pet set; one not among those found, the next.
pub fn pet_for<'a>(settings: &Map<String, Value>, session: &Value, found: &'a [PetInfo]) -> Option<&'a PetInfo> {
    let find = |id: Option<&str>| id.and_then(|id| found.iter().find(|p| p.id == id));
    let own = agent_key(session).and_then(|agent| settings.get("agentPets")?.get(agent)?.as_str());
    find(own).or_else(|| find(settings.get("pet").and_then(Value::as_str)))
}

// The URL the page loads a sheet from: Tauri's asset protocol, as
// convertFileSrc makes it (encodeURIComponent of the path).
pub fn asset_url(path: &Path) -> String {
    let mut out = String::new();
    for byte in path.to_string_lossy().bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    if cfg!(any(windows, target_os = "android")) {
        format!("http://asset.localhost/{out}")
    } else {
        format!("asset://localhost/{out}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(v: Value) -> Map<String, Value> {
        let Value::Object(map) = v else { unreachable!() };
        map
    }

    fn pet(id: &str, version: u8) -> PetInfo {
        PetInfo { id: id.into(), name: id.into(), author: String::new(), sheet: PathBuf::from(format!("{id}/spritesheet.webp")), version, from_codex: false }
    }

    #[test]
    fn each_agent_s_sessions_are_drawn_as_its_own_pet() {
        let found = [pet("claude-chan", 2), pet("owl", 2), pet("cat", 1)];
        let s = settings(json!({ "pet": "claude-chan", "agentPets": { "codex": "owl", "grok": "cat", "agy": "gone" } }));
        let drawn = |session: Value| pet_for(&s, &session, &found).map(|p| p.id.clone());
        // Codex's and Grok's their own; Claude Code's (no agent) none set: the pet set.
        assert_eq!(drawn(json!({ "id": "a", "agent": "codex" })).as_deref(), Some("owl"));
        assert_eq!(drawn(json!({ "id": "b", "agent": "grok" })).as_deref(), Some("cat"));
        assert_eq!(drawn(json!({ "id": "c", "agent": "" })).as_deref(), Some("claude-chan"));
        // Its own taken away: the pet set.
        assert_eq!(drawn(json!({ "id": "d", "agent": "agy" })).as_deref(), Some("claude-chan"));
        // No session at all: the pet set, whatever agentPets says.
        assert_eq!(drawn(json!({ "id": "", "agent": "" })).as_deref(), Some("claude-chan"));
        // Claude's own set: Claude Code's sessions are drawn as it.
        let s = settings(json!({ "pet": "owl", "agentPets": { "claude": "cat" } }));
        assert_eq!(pet_for(&s, &json!({ "id": "c", "agent": "" }), &found).map(|p| p.id.as_str()), Some("cat"));
        // The pet set gone too: none (the page says how to get one).
        let s = settings(json!({ "pet": "gone" }));
        assert!(pet_for(&s, &json!({ "id": "c" }), &found).is_none());
        assert_eq!(defaults()["agentPets"], json!({}));
        assert_eq!(defaults()["lang"], "zh-TW");
    }

    #[test]
    fn a_pet_whose_sheet_comes_after_its_folder_is_found_once_it_does() {
        let dir = std::env::temp_dir().join(format!("wakuwaku-late-sheet-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let pet = dir.join("pets").join("late");
        fs::create_dir_all(&pet).unwrap();
        let has = |name: &str| pets(&dir).iter().any(|p| p.id == "late" && p.name == name && p.sheet.starts_with(&dir));
        // Looked at between the folder and its sheet: not a pet yet.
        assert!(!has("late"));
        // Its sheet in (the folder it is in unchanged): found.
        fs::write(pet.join("spritesheet.webp"), b"RIFF0000WEBP").unwrap();
        assert!(has("late"));
        // Its pet.json written after: its name taken up.
        fs::write(pet.join("pet.json"), r#"{ "displayName": "Late", "spriteVersionNumber": 2 }"#).unwrap();
        assert!(has("Late"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_three_old_widgets_become_the_monitor() {
        // CPU on, the network and the battery off: the monitor on with CPU and memory.
        let mut s = settings(json!({ "widgetsOff": ["net", "battery", "x"], "widgetOrder": ["today", "sys", "net"] }));
        adopt_monitor(&mut s);
        assert_eq!(s["monitor"], json!({ "cpu": true, "mem": true, "net": false, "battery": false, "every": 2 }));
        assert_eq!((s["widgetsOff"].clone(), s["widgetOrder"].clone()), (json!(["x"]), json!(["today", "monitor"])));
        // All three off: the monitor off, with every part ready for when it is on.
        let mut s = settings(json!({ "widgetsOff": ["sys", "net", "battery"] }));
        adopt_monitor(&mut s);
        assert_eq!((s["monitor"]["net"].clone(), s["widgetsOff"].clone()), (json!(true), json!(["monitor"])));
        // Already the monitor's: left as it is.
        let mut s = settings(json!({ "widgetsOff": ["monitor"] }));
        adopt_monitor(&mut s);
        assert!(!s.contains_key("monitor"));
    }

    #[test]
    fn the_taskbar_home_is_the_island_now() {
        let dir = std::env::temp_dir().join(format!("wakuwaku-home-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        fs::write(dir.join("config.json"), r#"{ "display": "taskbar", "out": false, "taskbarEdge": "left", "islandLook": {}, "mail": [], "walk": false }"#).unwrap();
        let s = load(&dir);
        assert_eq!(s["display"], "island");
        // What she no longer has is let go; the rest kept.
        assert!(!s.contains_key("taskbarEdge") && !s.contains_key("islandLook") && !s.contains_key("mail"));
        assert_eq!(s["walk"], false);
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(defaults()["display"], "island");
    }

    #[test]
    fn quit_by_choice_keeps_her_from_the_hooks_until_she_starts() {
        let dir = std::env::temp_dir().join(format!("wakuwaku-quit-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        // Crashed (nothing marked): a session's hook starts her.
        clear_quit(&dir);
        assert!(may_start(&dir));
        // Quit from her menu: it does not.
        mark_quit(&dir);
        assert!(!may_start(&dir));
        // Started otherwise (by hand, at login, an update): it may again.
        clear_quit(&dir);
        assert!(may_start(&dir));
        let _ = fs::remove_dir_all(&dir);
    }

}

