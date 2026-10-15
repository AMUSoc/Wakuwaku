// Her updates, for the exe downloaded from a release (a build run from the
// source tree updates by a build, and is never looked for): Wakuwaku's
// latest release looked at as she starts, then once a day (settings:
// autoUpdateCheck, on unless turned off; "check now" any time), on GitHub
// and on Gitee, its copy for China (the same release there: the exe, and
// <exe>.sha256 beside it), both at once; the newer goes. Gitee's API is
// not used: without an account it allows an address some sixty calls, and
// then refuses it for over an hour (many in China share one address); its
// tags are read instead as git reads them, and the newest one's
// <exe>.sha256 fetched from where its release keeps it. A newer one is
// told on the island once (updateTold). On the person's word (Update, on
// the General page): its exe downloaded into her data folder, from Gitee
// first in China and GitHub first elsewhere (updateFrom: auto, or one of
// them), from the other should that fail; checked against the SHA-256
// GitHub gives for it, or Gitee's <exe>.sha256 when GitHub gives none (none
// at all, the two differing, or the download another: not installed), put
// where hers is, hers kept beside it as <name>.old.exe, and started with
// --after-update <her pid>: it waits for her to be gone (the port), and
// she quits as the menu does. A prompt waiting for an answer
// is answered first. The old exe goes once the new one has run a minute;
// should the new one not start, the old one is still there to be renamed
// back (DEVELOPMENT.md).
//
// For tests: WAKUWAKU_UPDATE_URL, GitHub's release JSON from elsewhere, and
// WAKUWAKU_UPDATE_GITEE_URL, a Gitee repo elsewhere (its <url>.git/info/refs
// and <url>releases/download/…; only those given are asked then, their exes
// taken from anywhere); WAKUWAKU_UPDATE_FORCE=1, a build from the source
// tree looks and updates too.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::Shared;

const GITHUB_LATEST: &str = "https://api.github.com/repos/zhoupengjie/Wakuwaku/releases/latest";
const GITHUB_REPO: &str = "https://github.com/zhoupengjie/Wakuwaku/";
const GITEE_REPO: &str = "https://gitee.com/zhoupengjie/Wakuwaku/";
const GITHUB_TEST: &str = "WAKUWAKU_UPDATE_URL";
const GITEE_TEST: &str = "WAKUWAKU_UPDATE_GITEE_URL";
pub const AFTER_UPDATE: &str = "--after-update";
const DAY_MS: u64 = 86_400_000;
const HOUR_MS: u64 = 3_600_000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Source {
    GitHub,
    Gitee,
}

impl Source {
    pub fn name(self) -> &'static str {
        match self {
            Source::GitHub => "GitHub",
            Source::Gitee => "Gitee",
        }
    }
}

// What one place says of its latest release: the version, its page, its
// exe by name and the SHA-256 given for it.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub source: Source,
    pub version: String,
    pub page: String,
    pub exe: Option<String>,
    pub digest: Option<String>,
    pub size: u64,
}

// The release to go by: the newest version either place has, what each
// that has it says of it (GitHub first), and the SHA-256 to check by: the
// one given; none if none is, or if the two differ (`conflict`).
#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: String,
    pub from: Vec<Found>,
    pub digest: Option<String>,
    pub conflict: bool,
}

impl Release {
    // Its places, the one preferred first.
    fn ordered(&self, gitee_first: bool) -> Vec<&Found> {
        let mut all: Vec<&Found> = self.from.iter().collect();
        all.sort_by_key(|f| (f.source == Source::Gitee) != gitee_first);
        all
    }

    fn page(&self, gitee_first: bool) -> String {
        self.ordered(gitee_first).iter().map(|f| f.page.clone()).find(|p| !p.is_empty()).unwrap_or_default()
    }

    fn can_install(&self) -> bool {
        self.digest.is_some() && self.from.iter().any(|f| f.exe.is_some())
    }
}

struct State {
    // When last looked (ms), and when to look again.
    checked_at: u64,
    next_at: u64,
    checking: bool,
    latest: Option<Release>,
    // What went wrong last: { code, detail }.
    error: Option<Value>,
    // An update under way: download, verify, waiting (a prompt open), restart;
    // where it is downloaded from.
    stage: Option<&'static str>,
    from: Option<Source>,
    got: u64,
    total: u64,
}

static STATE: Mutex<State> = Mutex::new(State { checked_at: 0, next_at: 0, checking: false, latest: None, error: None, stage: None, from: None, got: 0, total: 0 });
// Started by an update, after this pid: said in the log once she is up.
static AFTER: Mutex<Option<u32>> = Mutex::new(None);

pub fn current() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

// A downloaded exe (or one told to, for tests): only those update.
pub fn may_update() -> bool {
    std::env::var("WAKUWAKU_UPDATE_FORCE").as_deref() == Ok("1") || !crate::data::from_source()
}

fn test_url(var: &str) -> Option<String> {
    std::env::var(var).ok().filter(|u| !u.trim().is_empty())
}

fn testing() -> bool {
    test_url(GITHUB_TEST).is_some() || test_url(GITEE_TEST).is_some()
}

// In China, as far as Windows says: its time zone China's, or its language
// Chinese as written there. Asked once.
pub(crate) fn in_china() -> bool {
    static CHINA: OnceLock<bool> = OnceLock::new();
    *CHINA.get_or_init(|| {
        let locale = sys_locale::get_locale().unwrap_or_default().to_ascii_lowercase();
        if locale == "zh" || locale.starts_with("zh-cn") || locale.starts_with("zh-hans") {
            return true;
        }
        #[cfg(windows)]
        {
            use winreg::enums::HKEY_LOCAL_MACHINE;
            use winreg::RegKey;
            let zone = RegKey::predef(HKEY_LOCAL_MACHINE)
                .open_subkey(r"SYSTEM\CurrentControlSet\Control\TimeZoneInformation")
                .and_then(|k| k.get_value::<String, _>("TimeZoneKeyName"))
                .unwrap_or_default();
            if zone.split('\0').next() == Some("China Standard Time") {
                return true;
            }
        }
        false
    })
}

// Gitee first: as set (updateFrom), or by itself in China.
fn gitee_first(sh: &Shared) -> bool {
    match sh.setting("updateFrom").as_str() {
        Some("gitee") => true,
        Some("github") => false,
        _ => in_china(),
    }
}

// Whether one version is newer than another: their numbers one by one (so
// 0.10.0 is after 0.9.0), a missing one 0; one with a part after "-" (a
// prerelease) before the same without.
pub fn newer(theirs: &str, ours: &str) -> bool {
    fn parts(v: &str) -> (Vec<u64>, bool) {
        let v = v.trim().trim_start_matches(['v', 'V']);
        let v = v.split('+').next().unwrap_or(v);
        let (core, pre) = match v.split_once('-') {
            Some((core, _)) => (core, true),
            None => (v, false),
        };
        (core.split('.').map(|n| n.trim().parse().unwrap_or(0)).collect(), pre)
    }
    let ((a, a_pre), (b, b_pre)) = (parts(theirs), parts(ours));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    !a_pre && b_pre
}

fn same_version(a: &str, b: &str) -> bool {
    !newer(a, b) && !newer(b, a)
}

fn exe_name(version: &str) -> String {
    format!("wakuwaku-v{version}-windows-x64.exe")
}

// A release's version by its tag, if it counts (no draft, no prerelease).
fn version_of(v: &Value) -> Option<String> {
    if v["draft"] == true || v["prerelease"] == true {
        return None;
    }
    let version = v["tag_name"].as_str()?.trim().trim_start_matches(['v', 'V']).to_string();
    (!version.is_empty()).then_some(version)
}

// A SHA-256 in hex, lowercase.
fn sha256_hex(h: &str) -> Option<String> {
    let h = h.trim();
    (h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit())).then(|| h.to_ascii_lowercase())
}

// An <exe>.sha256 as sha256sum writes it ("<hex>  <name>"), or the hex alone.
pub fn sums_of(text: &str) -> Option<String> {
    sha256_hex(text.trim_start_matches('\u{feff}').split_whitespace().next()?)
}

// GitHub's release JSON: its exe by name with the SHA-256 given for it.
pub fn github_release(v: &Value) -> Option<Found> {
    let version = version_of(v)?;
    let name = exe_name(&version);
    let asset = v["assets"].as_array().and_then(|all| all.iter().find(|a| a["name"].as_str() == Some(name.as_str())));
    Some(Found {
        source: Source::GitHub,
        page: v["html_url"].as_str().unwrap_or("").to_string(),
        exe: asset.and_then(|a| a["browser_download_url"].as_str()).map(str::to_string),
        digest: asset.and_then(|a| a["digest"].as_str()).and_then(|d| d.strip_prefix("sha256:")).and_then(sha256_hex),
        size: asset.and_then(|a| a["size"].as_u64()).unwrap_or(0),
        version,
    })
}

// The newest of a repo's tags as git lists them (info/refs: pkt-lines of
// "<sha> refs/tags/<tag>", a peeled one again with ^{}, the first with
// "\0<capabilities>"): of those that are versions (v and a number), none
// with a "-" in it: a prerelease, which tags alone cannot tell otherwise.
pub fn newest_tag(refs: &str) -> Option<String> {
    refs.lines()
        .filter_map(|line| line.split_once(" refs/tags/").and_then(|(_, tag)| tag.split(['\0', ' ']).next()))
        .map(str::trim)
        .filter(|tag| !tag.ends_with("^{}") && !tag.contains('-') && tag.trim_start_matches(['v', 'V']).starts_with(|c: char| c.is_ascii_digit()))
        .map(str::to_string)
        .reduce(|a, b| if newer(&b, &a) { b } else { a })
}

// Gitee: its newest tag, and that tag's release by where Gitee keeps its
// files (<repo>releases/download/<tag>/<file>): its <exe>.sha256 there,
// put up after the exe, so the release is whole; or none of it yet (404).
fn gitee_found(repo: &str) -> Result<Option<Found>, String> {
    let repo = format!("{}/", repo.trim_end_matches('/'));
    let refs = get_text(&format!("{}.git/info/refs?service=git-upload-pack", repo.trim_end_matches('/')), 1 << 20)?.unwrap_or_default();
    let Some(tag) = newest_tag(&refs) else { return Ok(None) };
    let version = tag.trim_start_matches(['v', 'V']).to_string();
    let exe = format!("{repo}releases/download/{tag}/{}", exe_name(&version));
    let Some(sums) = get_text(&format!("{exe}.sha256"), 4096)? else { return Ok(None) };
    Ok(Some(Found { source: Source::Gitee, page: format!("{repo}releases/tag/{tag}"), exe: Some(exe), digest: sums_of(&sums), size: 0, version }))
}

// The newest either place has: the places that have it, and its SHA-256.
pub fn merge(found: Vec<Found>) -> Option<Release> {
    let version = found.iter().map(|f| f.version.clone()).reduce(|a, b| if newer(&b, &a) { b } else { a })?;
    let mut from: Vec<Found> = found.into_iter().filter(|f| same_version(&f.version, &version)).collect();
    from.sort_by_key(|f| f.source == Source::Gitee);
    let mut digests: Vec<&String> = from.iter().filter_map(|f| f.digest.as_ref()).collect();
    digests.dedup();
    let conflict = digests.len() > 1;
    let digest = if conflict { None } else { digests.first().map(|d| d.to_string()) };
    Some(Release { version, from, digest, conflict })
}

// From GitHub or Gitee itself (their release pages, and the downloads they
// redirect to), unless a test gave the release from elsewhere.
fn allowed(url: &str) -> bool {
    testing() || [GITHUB_REPO, GITEE_REPO].iter().any(|repo| url.get(..repo.len()).is_some_and(|p| p.eq_ignore_ascii_case(repo)))
}

fn agent(seconds: u64) -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(seconds))).build().new_agent()
}

// A JSON from there; none if it is not there (404: no release yet).
fn get_json(url: &str, accept: &str) -> Result<Option<Value>, String> {
    let res = agent(20).get(url).header("user-agent", &format!("wakuwaku/{}", current())).header("accept", accept).call();
    match res {
        Err(ureq::Error::StatusCode(404)) => Ok(None),
        Err(e) => Err(e.to_string()),
        Ok(mut res) => res.body_mut().read_json().map(Some).map_err(|e| e.to_string()),
    }
}

// A text from there (at most `limit` bytes); none if it is not there (404).
fn get_text(url: &str, limit: u64) -> Result<Option<String>, String> {
    match agent(20).get(url).header("user-agent", &format!("wakuwaku/{}", current())).call() {
        Err(ureq::Error::StatusCode(404)) => Ok(None),
        Err(e) => Err(e.to_string()),
        Ok(mut res) => res.body_mut().with_config().limit(limit).read_to_string().map(Some).map_err(|e| e.to_string()),
    }
}

// Where each place's release is looked at (GitHub's API, Gitee's repo):
// both, or those a test gives.
fn endpoints() -> Vec<(Source, String)> {
    if !testing() {
        return vec![(Source::GitHub, GITHUB_LATEST.to_string()), (Source::Gitee, GITEE_REPO.to_string())];
    }
    [(Source::GitHub, test_url(GITHUB_TEST)), (Source::Gitee, test_url(GITEE_TEST))].into_iter().filter_map(|(source, url)| Some((source, url?))).collect()
}

// What one place says.
fn fetch(source: Source, url: &str) -> Result<Option<Found>, String> {
    match source {
        Source::GitHub => Ok(get_json(url, "application/vnd.github+json")?.as_ref().and_then(github_release)),
        Source::Gitee => gitee_found(url),
    }
}

// Both places looked at together, and the newest of what they say. Neither
// answering is an error; one not, only said in the log.
fn fetch_latest(sh: &Shared) -> Result<Option<Release>, String> {
    let asked = endpoints();
    let got: Vec<(Source, Result<Option<Found>, String>)> = std::thread::scope(|s| {
        let looks: Vec<_> = asked.iter().map(|(source, url)| (*source, s.spawn(move || fetch(*source, url)))).collect();
        looks.into_iter().map(|(source, look)| (source, look.join().unwrap_or_else(|_| Err("stopped".into())))).collect()
    });
    let mut found = Vec::new();
    let mut failed = Vec::new();
    for (source, got) in got {
        match got {
            Ok(f) => found.extend(f),
            Err(e) => failed.push(format!("{}: {e}", source.name())),
        }
    }
    if !failed.is_empty() && failed.len() == asked.len() {
        return Err(failed.join("; "));
    }
    for e in &failed {
        sh.log(&format!("update: could not look on {e}"));
    }
    Ok(merge(found))
}

fn error(code: &str, detail: impl Into<String>) -> Value {
    json!({ "code": code, "detail": detail.into() })
}

fn places(r: &Release) -> String {
    r.from.iter().map(|f| f.source.name()).collect::<Vec<_>>().join(", ")
}

// Looked at now (one look at a time); told on the island if newer.
pub fn check_now(sh: &Arc<Shared>) {
    {
        let mut s = STATE.lock().unwrap();
        if s.checking {
            return;
        }
        s.checking = true;
    }
    sh.push_settings();
    let got = fetch_latest(sh);
    let now = crate::now_ms();
    {
        let mut s = STATE.lock().unwrap();
        s.checking = false;
        s.checked_at = now;
        match &got {
            Ok(release) => {
                s.latest = release.clone();
                s.next_at = now + DAY_MS;
                if s.stage.is_none() {
                    // GitHub and Gitee not agreeing on a newer one's SHA-256.
                    s.error = release.as_ref().filter(|r| r.conflict && newer(&r.version, current())).map(|r| error("mismatch", r.version.clone()));
                }
            }
            // Quietly again in an hour.
            Err(e) => {
                s.next_at = now + HOUR_MS;
                s.error = Some(error("check", e.clone()));
            }
        }
    }
    match &got {
        Ok(Some(r)) if newer(&r.version, current()) => sh.log(&format!("update: v{} out ({}), newer than v{}", r.version, places(r), current())),
        Ok(Some(r)) => sh.log(&format!("update: v{} is the latest ({}); this is v{}", r.version, places(r), current())),
        Ok(None) => sh.log("update: no release to go by (none, a draft or a prerelease)"),
        Err(e) => sh.log(&format!("update: could not look ({e}); again in an hour")),
    }
    if let Ok(Some(r)) = &got {
        if r.conflict {
            sh.log(&format!("update: v{}: GitHub and Gitee give different SHA-256s; not to be installed", r.version));
        }
    }
    tell(sh);
    sh.push_settings();
}

// A newer one told on the island, once for each: only while she is in
// sight and no session needs you (else at a later look).
fn tell(sh: &Arc<Shared>) {
    let Some(r) = STATE.lock().unwrap().latest.clone().filter(|r| newer(&r.version, current())) else { return };
    if sh.setting("updateTold").as_str() == Some(r.version.as_str()) || !sh.is_visible() || sh.pet.lock().unwrap().wants_you() {
        return;
    }
    let lang = sh.lang();
    let words = crate::i18n::t(lang, "update.out").replace("{version}", &r.version);
    sh.to_pages("pet:nudge", json!({ "id": "update", "icon": "star", "words": words, "label": crate::i18n::t(lang, "update.tap"), "value": "", "private": false, "open": { "tab": "update" } }));
    sh.change(json!({ "updateTold": r.version }));
    sh.log(&format!("update: v{} told on the island", r.version));
}

// As she starts: a download cut off cleared away, the old exe gone after a
// minute's run, and the looks
// (twenty seconds in, then as due; every ten minutes it is seen whether a
// look is due, or a newer one waits to be told).
pub fn start(sh: &Arc<Shared>) {
    if let Some(pid) = *AFTER.lock().unwrap() {
        sh.log(&format!("update: v{} up, after pid {pid} ended", current()));
    }
    // A download cut off (she quit in the middle of it): its .part gone.
    for entry in std::fs::read_dir(&sh.dir).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("update-v") && name.ends_with(".exe.part") && std::fs::remove_file(entry.path()).is_ok() {
            sh.log(&format!("update: {name}, left by a download cut off, removed"));
        }
    }
    let tidy = sh.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(60));
        remove_old(&tidy);
    });
    if !may_update() {
        sh.log("update: run from the source tree: not looked for");
        return;
    }
    let sh = sh.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(20));
        loop {
            let due = crate::now_ms() >= STATE.lock().unwrap().next_at;
            if due && sh.setting("autoUpdateCheck") != false {
                check_now(&sh);
            } else {
                tell(&sh);
            }
            std::thread::sleep(Duration::from_secs(600));
        }
    });
}

// For the settings: this version, the latest (its page the preferred
// place's), and how an update goes.
pub fn view(sh: &Shared) -> Value {
    let first = gitee_first(sh);
    let s = STATE.lock().unwrap();
    let latest = s.latest.as_ref().map(|r| {
        json!({
            "version": r.version,
            "page": r.page(first),
            "newer": newer(&r.version, current()),
            "canInstall": r.can_install(),
            "places": r.from.iter().map(|f| f.source.name()).collect::<Vec<_>>(),
        })
    });
    json!({
        "current": current(),
        "may": may_update(),
        "source": crate::data::from_source(),
        "checking": s.checking,
        "checkedAt": s.checked_at,
        "latest": latest,
        "error": s.error,
        "stage": s.stage,
        "from": s.from.map(Source::name),
        "got": s.got,
        "total": s.total,
    })
}

// The latest release's page, opened: the preferred place's (only GitHub's
// or Gitee's, unless a test's).
pub fn open_page(sh: &Shared) {
    let first = gitee_first(sh);
    let page = STATE.lock().unwrap().latest.as_ref().map(|r| r.page(first)).unwrap_or_default();
    if page.is_empty() || !allowed(&page) {
        return;
    }
    if !crate::jump::open_url(&page) {
        sh.log(&format!("update: could not open {page}"));
    }
}

// The person said Update: off on its own thread.
pub fn update_now(sh: &Arc<Shared>) {
    let release = {
        let mut s = STATE.lock().unwrap();
        if s.stage.is_some() {
            return;
        }
        let Some(r) = s.latest.clone().filter(|r| newer(&r.version, current())) else { return };
        s.stage = Some("download");
        s.from = None;
        s.got = 0;
        s.total = 0;
        s.error = None;
        r
    };
    sh.push_settings();
    let sh = sh.clone();
    std::thread::spawn(move || {
        if let Err(e) = install(&sh, &release) {
            sh.log(&format!("update: v{} not installed: {e}", release.version));
            let mut s = STATE.lock().unwrap();
            s.stage = None;
            s.from = None;
            s.error = Some(e);
            drop(s);
            sh.push_settings();
        }
    });
}

fn stage(sh: &Shared, stage: &'static str) {
    STATE.lock().unwrap().stage = Some(stage);
    sh.push_settings();
}

fn install(sh: &Arc<Shared>, r: &Release) -> Result<(), Value> {
    if !may_update() {
        return Err(error("source", ""));
    }
    if r.conflict {
        return Err(error("mismatch", r.version.clone()));
    }
    let digest = r.digest.as_deref().ok_or_else(|| error("noDigest", ""))?;
    let exes: Vec<(Source, &str, u64)> = r.ordered(gitee_first(sh)).into_iter().filter_map(|f| Some((f.source, f.exe.as_deref()?, f.size))).collect();
    let part = sh.dir.join(format!("update-v{}.exe.part", r.version));
    // From the place preferred; the other should that fail.
    let mut failed = error("noExe", "");
    let mut done = false;
    for (source, url, size) in exes {
        if !allowed(url) {
            failed = error("elsewhere", url);
            continue;
        }
        {
            let mut s = STATE.lock().unwrap();
            s.stage = Some("download");
            s.from = Some(source);
            s.got = 0;
            s.total = size;
        }
        sh.push_settings();
        let got = match download(sh, url, &part) {
            Ok(got) => got,
            Err(e) => {
                let _ = std::fs::remove_file(&part);
                sh.log(&format!("update: v{} not downloaded from {}: {e}", r.version, source.name()));
                failed = error("download", format!("{}: {e}", source.name()));
                continue;
            }
        };
        stage(sh, "verify");
        if got == digest {
            sh.log(&format!("update: v{} downloaded from {}, its SHA-256 as given", r.version, source.name()));
            done = true;
            break;
        }
        let _ = std::fs::remove_file(&part);
        sh.log(&format!("update: v{} from {}: SHA-256 {got}, not {digest}", r.version, source.name()));
        failed = error("digest", format!("{}: {got} ≠ {digest}", source.name()));
    }
    if !done {
        return Err(failed);
    }
    // A prompt waiting for an answer: answered first (the new one would
    // not have it).
    if !sh.asks.lock().unwrap().is_empty() {
        stage(sh, "waiting");
        sh.log("update: a prompt is open: waiting for its answer");
        while !sh.asks.lock().unwrap().is_empty() {
            std::thread::sleep(Duration::from_secs(1));
        }
    }
    stage(sh, "restart");
    let exe = replace(&part).map_err(|e| {
        let _ = std::fs::remove_file(&part);
        error("replace", e)
    })?;
    if let Err(e) = start_new(&exe) {
        // Hers back where it was.
        let _ = std::fs::remove_file(&exe);
        let _ = std::fs::rename(old_path(&exe), &exe);
        return Err(error("start", e));
    }
    sh.log(&format!("update: v{} in place of v{} ({}), started; she quits for it", r.version, current(), exe.display()));
    sh.quit();
    Ok(())
}

// Downloaded to `to`, its SHA-256 as it comes (lowercase hex); the progress
// for the settings.
fn download(sh: &Shared, url: &str, to: &Path) -> Result<String, String> {
    let mut res = agent(600).get(url).header("user-agent", &format!("wakuwaku/{}", current())).call().map_err(|e| e.to_string())?;
    let total = res.body().content_length().unwrap_or(0);
    if total > 0 {
        STATE.lock().unwrap().total = total;
    }
    let mut reader = res.body_mut().with_config().limit(512 * 1024 * 1024).reader();
    let mut file = std::fs::File::create(to).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut got = 0u64;
    loop {
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        hash.update(&buf[..n]);
        got += n as u64;
        STATE.lock().unwrap().got = got;
        sh.push_settings();
    }
    file.flush().map_err(|e| e.to_string())?;
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

// Beside hers, its name kept (she may have been renamed): <name>.old.exe.
fn old_path(exe: &Path) -> PathBuf {
    let stem = exe.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "wakuwaku".into());
    exe.with_file_name(format!("{stem}.old.exe"))
}

// The new exe where hers is (a running exe can be renamed, not removed),
// hers kept as <name>.old.exe. Its path.
fn replace(part: &Path) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let old = old_path(&exe);
    let _ = std::fs::remove_file(&old);
    std::fs::rename(&exe, &old).map_err(|e| format!("{} → {}: {e}", exe.display(), old.display()))?;
    let put = std::fs::rename(part, &exe).or_else(|_| std::fs::copy(part, &exe).map(|_| ()));
    if let Err(e) = put {
        let _ = std::fs::rename(&old, &exe);
        return Err(format!("{}: {e}", exe.display()));
    }
    let _ = std::fs::remove_file(part);
    Ok(exe)
}

// Started on its own (out of her process tree and any job, as if opened
// from Explorer: detach.rs), to wait for her.
fn start_new(exe: &Path) -> Result<(), String> {
    let pid = std::process::id().to_string();
    if crate::detach::spawn(exe, &[AFTER_UPDATE, &pid]) {
        Ok(())
    } else {
        Err(format!("{} would not start ({})", exe.display(), std::io::Error::last_os_error()))
    }
}

// Her own exe started again, to wait for her to end (crash.rs: WebView2's
// browser process gone, her pages with it).
pub fn start_again() -> Result<(), String> {
    start_new(&std::env::current_exe().map_err(|e| e.to_string())?)
}

// After a minute's run, the exe an update left: gone (tried a few times:
// the old one's guard may still have it open a moment).
fn remove_old(sh: &Shared) {
    let Ok(exe) = std::env::current_exe() else { return };
    let old = old_path(&exe);
    for _ in 0..6 {
        if !old.exists() {
            return;
        }
        if std::fs::remove_file(&old).is_ok() {
            sh.log(&format!("update: {} removed, the new one having run a minute", old.display()));
            return;
        }
        std::thread::sleep(Duration::from_secs(10));
    }
    sh.log(&format!("update: could not remove {}", old.display()));
}

// Started by an update (--after-update <pid>): that one waited for, at most
// a minute, before she takes the port.
pub fn wait_after_update(args: &[String]) {
    let Some(pid) = args.iter().position(|a| a == AFTER_UPDATE).and_then(|i| args.get(i + 1)).and_then(|p| p.parse::<u32>().ok()) else { return };
    *AFTER.lock().unwrap() = Some(pid);
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
            fn WaitForSingleObject(handle: isize, ms: u32) -> u32;
            fn CloseHandle(handle: isize) -> i32;
        }
        const SYNCHRONIZE: u32 = 0x0010_0000;
        // SAFETY: a handle we close.
        unsafe {
            let process = OpenProcess(SYNCHRONIZE, 0, pid);
            if process != 0 {
                WaitForSingleObject(process, 60_000);
                CloseHandle(process);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{allowed, github_release, merge, newer, newest_tag, sums_of, Found, Source};

    // Her version said the same everywhere it is said: what updates go by
    // (Cargo.toml), the exe's own and the settings' (tauri.conf.json), and
    // the Claude Code plugin's; a test build once had only one of them changed.
    #[test]
    fn one_version_everywhere() {
        let ours = env!("CARGO_PKG_VERSION");
        for (file, text) in [("tauri.conf.json", include_str!("../tauri.conf.json")), ("plugin.json", include_str!("../../integrations/claude-code/.claude-plugin/plugin.json"))] {
            let v: serde_json::Value = serde_json::from_str(text).unwrap();
            assert_eq!(v["version"].as_str(), Some(ours), "{file} says another version than Cargo.toml");
        }
    }

    #[test]
    fn versions_compared() {
        assert!(newer("v0.2.1", "0.2.0"));
        assert!(newer("0.10.0", "0.9.0"));
        assert!(newer("1.0", "0.99.9"));
        assert!(!newer("0.2.0", "0.2.0"));
        assert!(!newer("v0.2.0", "0.2.0"));
        assert!(!newer("0.1.9", "0.2.0"));
        assert!(!newer("0.9.0", "0.10.0"));
        assert!(newer("0.2.0", "0.2.0-beta"));
        assert!(!newer("0.2.0-beta", "0.2.0"));
    }

    #[test]
    fn github_read() {
        let asset = |name: &str, digest: &str| json!({ "name": name, "size": 7, "digest": digest, "browser_download_url": format!("https://github.com/zhoupengjie/Wakuwaku/releases/download/x/{name}") });
        let good = json!({
            "tag_name": "v0.2.1", "draft": false, "prerelease": false,
            "html_url": "https://github.com/zhoupengjie/Wakuwaku/releases/tag/v0.2.1",
            "assets": [asset("notes.txt", ""), asset("wakuwaku-v0.2.1-windows-x64.exe", &format!("sha256:{}", "AB".repeat(32)))],
        });
        let r = github_release(&good).unwrap();
        assert_eq!(r.source, Source::GitHub);
        assert_eq!(r.version, "0.2.1");
        assert_eq!(r.digest.as_deref(), Some("ab".repeat(32).as_str()));
        assert!(r.exe.unwrap().ends_with("wakuwaku-v0.2.1-windows-x64.exe"));
        let mut draft = good.clone();
        draft["draft"] = json!(true);
        assert!(github_release(&draft).is_none());
        let mut pre = good.clone();
        pre["prerelease"] = json!(true);
        assert!(github_release(&pre).is_none());
        // No SHA-256 given, or not one: none to check by.
        let mut bare = good.clone();
        bare["assets"] = json!([asset("wakuwaku-v0.2.1-windows-x64.exe", "md5:xyz")]);
        assert_eq!(github_release(&bare).unwrap().digest, None);
        // The exe by another name: none to install.
        let mut other = good;
        other["assets"] = json!([asset("wakuwaku.exe", "")]);
        assert_eq!(github_release(&other).unwrap().exe, None);
    }

    // A repo's tags as git's info/refs gives them (as Gitee's does): the
    // highest version, by number; not a prerelease ("-"), a peeled line
    // (^{}) or a tag that is no version.
    #[test]
    fn tags_read() {
        let sha = "a".repeat(40);
        let pkt = |line: String| format!("{:04x}{line}", line.len() + 4);
        let mut refs = pkt("# service=git-upload-pack\n".into()) + "0000";
        refs += &pkt(format!("{sha} HEAD\0multi_ack thin-pack side-band symref=HEAD:refs/heads/main agent=git/2\n"));
        refs += &pkt(format!("{sha} refs/heads/main\n"));
        for tag in ["v0.2.1", "v0.2.1^{}", "v0.9.0", "v0.10.0", "v0.10.0^{}", "v0.11.0-beta", "nightly"] {
            refs += &pkt(format!("{sha} refs/tags/{tag}\n"));
        }
        refs += "0000";
        assert_eq!(newest_tag(&refs).as_deref(), Some("v0.10.0"));
        // The first line's capabilities after \0, when it is a tag.
        assert_eq!(newest_tag(&pkt(format!("{sha} refs/tags/v0.2.1\0multi_ack\n"))).as_deref(), Some("v0.2.1"));
        assert_eq!(newest_tag(&(pkt("# service=git-upload-pack\n".into()) + "0000" + &pkt(format!("{sha} refs/heads/main\n")) + "0000")), None);
        assert_eq!(newest_tag(""), None);
    }

    #[test]
    fn sums_read() {
        let hex = "AB".repeat(32);
        assert_eq!(sums_of(&format!("{hex}  wakuwaku-v0.2.2-windows-x64.exe\n")).as_deref(), Some("ab".repeat(32).as_str()));
        assert_eq!(sums_of(&format!("\u{feff}{hex} *wakuwaku.exe\r\n")).as_deref(), Some("ab".repeat(32).as_str()));
        assert_eq!(sums_of(&hex).as_deref(), Some("ab".repeat(32).as_str()));
        assert_eq!(sums_of("not a sum"), None);
        assert_eq!(sums_of(""), None);
    }

    fn found(source: Source, version: &str, digest: Option<&str>) -> Found {
        Found { source, version: version.into(), page: format!("{}/{version}", source.name()), exe: Some(format!("{}/{version}.exe", source.name())), digest: digest.map(str::to_string), size: 0 }
    }

    #[test]
    fn places_merged() {
        let (a, b) = ("a".repeat(64), "b".repeat(64));
        // The same release in both: GitHub's SHA-256, both places to download from.
        let r = merge(vec![found(Source::Gitee, "0.2.2", Some(&a)), found(Source::GitHub, "0.2.2", Some(&a))]).unwrap();
        assert_eq!(r.version, "0.2.2");
        assert_eq!(r.from.iter().map(|f| f.source).collect::<Vec<_>>(), [Source::GitHub, Source::Gitee]);
        assert_eq!(r.digest.as_deref(), Some(a.as_str()));
        assert!(!r.conflict && r.can_install());
        // The preferred place first.
        assert_eq!(r.ordered(true)[0].source, Source::Gitee);
        assert_eq!(r.ordered(false)[0].source, Source::GitHub);
        assert_eq!(r.page(true), "Gitee/0.2.2");
        // Gitee without a .sha256 read: GitHub's still checks Gitee's exe.
        let r = merge(vec![found(Source::GitHub, "0.2.2", Some(&a)), found(Source::Gitee, "0.2.2", None)]).unwrap();
        assert_eq!(r.digest.as_deref(), Some(a.as_str()));
        // One newer: only it, by its own SHA-256.
        let r = merge(vec![found(Source::GitHub, "0.2.1", Some(&a)), found(Source::Gitee, "0.2.2", Some(&b))]).unwrap();
        assert_eq!(r.version, "0.2.2");
        assert_eq!(r.from.len(), 1);
        assert_eq!(r.digest.as_deref(), Some(b.as_str()));
        // Two that differ: none trusted.
        let r = merge(vec![found(Source::GitHub, "0.2.2", Some(&a)), found(Source::Gitee, "v0.2.2", Some(&b))]).unwrap();
        assert!(r.conflict);
        assert_eq!(r.digest, None);
        assert!(!r.can_install());
        // None given: nothing to install.
        assert!(!merge(vec![found(Source::Gitee, "0.2.2", None)]).unwrap().can_install());
        assert_eq!(merge(vec![]), None);
    }

    // Both places as they answer now (network; `cargo test live -- --ignored`).
    #[test]
    #[ignore]
    fn live_places() {
        let github = super::fetch(Source::GitHub, super::GITHUB_LATEST).unwrap();
        let gitee = super::fetch(Source::Gitee, super::GITEE_REPO).unwrap();
        println!("GitHub: {github:?}\nGitee: {gitee:?}");
        assert!(github.as_ref().is_some_and(|f| f.digest.is_some() && f.exe.is_some()));
        assert!(gitee.as_ref().is_some_and(|f| f.digest.is_some() && f.exe.is_some()));
        let r = merge(github.into_iter().chain(gitee).collect()).unwrap();
        println!("{r:?}");
        assert!(!r.conflict);
        // Another repo on Gitee: its newest tag read, no .sha256 of hers there (404), nothing.
        assert_eq!(super::fetch(Source::Gitee, "https://gitee.com/cxasm/notepad--").unwrap(), None);
    }

    // Against test/fake-release.js (started with --version 0.2.2), the test
    // URLs set: `cargo test fake_places -- --ignored --test-threads 1`.
    #[test]
    #[ignore]
    fn fake_places() {
        let set = |m: &str| {
            ureq::get(&format!("http://127.0.0.1:47390/mode?{m}")).call().unwrap();
        };
        let look = || merge(super::endpoints().into_iter().filter_map(|(s, u)| super::fetch(s, &u).unwrap()).collect()).unwrap();
        set("githubOld=0&giteeOld=0&noDigest=0&noSums=0&badSums=0&giteeEmpty=0");
        let r = look();
        assert_eq!((r.version.as_str(), r.from.len(), r.conflict), ("0.2.2", 2, false));
        assert_eq!(r.from[1].digest, r.digest, "Gitee's .sha256, through its redirect, agrees with GitHub");
        set("badSums=1");
        assert!(look().conflict);
        set("badSums=0&noDigest=1");
        assert!(look().digest.is_some(), "Gitee's .sha256 when GitHub gives none");
        // No .sha256 on Gitee: its release not there (yet); GitHub's alone, without a digest.
        set("noSums=1");
        let r = look();
        assert_eq!((r.from.len(), r.can_install()), (1, false));
        set("noDigest=0&noSums=0&githubOld=1");
        let r = look();
        assert_eq!((r.version.as_str(), r.from.len(), r.from[0].source), ("0.2.2", 1, Source::Gitee));
        // Gitee without tags, or its newest tag without a release: GitHub's alone.
        set("githubOld=0&giteeEmpty=1");
        assert_eq!(look().from.len(), 1);
        set("giteeEmpty=0&giteeOld=1");
        assert_eq!(look().from.len(), 1);
        set("giteeOld=0");
    }

    #[test]
    fn only_her_repos() {
        assert!(allowed("https://github.com/zhoupengjie/Wakuwaku/releases/download/v0.2.2/wakuwaku-v0.2.2-windows-x64.exe"));
        assert!(allowed("https://gitee.com/zhoupengjie/Wakuwaku/releases/download/v0.2.2/wakuwaku-v0.2.2-windows-x64.exe"));
        assert!(allowed("https://gitee.com/zhoupengjie/wakuwaku/releases/tag/v0.2.2"));
        assert!(!allowed("https://gitee.com/someone/Wakuwaku/releases/download/v0.2.2/x.exe"));
        assert!(!allowed("https://gitee.com.evil.example/zhoupengjie/Wakuwaku/"));
        assert!(!allowed("http://gitee.com/zhoupengjie/Wakuwaku/releases/download/v0.2.2/x.exe"));
        assert!(!allowed("https://gitee.com/zhoupengjie/Wakuwaku"));
    }
}
