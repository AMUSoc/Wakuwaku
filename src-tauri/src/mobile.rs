// The phone's page (src/mobile/): the four agents' pets, their sessions, how
// their turns ended, and the prompts waiting, answered from the phone.
//
//   GET  /mobile/              the page (?t=<token>: kept as a cookie, then taken out of the address)
//   GET  /mobile/<file>        its scripts, style, manifest, service worker and icon
//   GET  /mobile/sheet/<agent> that agent's pet's sheet
//   GET  /mobile/api/state     each agent's session shown and its sessions (state.rs view)
//   GET  /mobile/api/asks      the prompts waiting (asks.rs views), every page's
//   POST /mobile/api/answer    { id, choice }: as the panel answers (Shared::answer)
//   GET  /mobile/api/reply     ?session=<id>: its last reply whole, from its transcript (reply.rs)
//   POST /mobile/api/run       { agent, prompt, cwd? }: a task for an agent (run.rs)
//   GET  /mobile/api/conversation  ?session=<id>: the whole conversation, and whether the phone may add to it (talk.rs)
//   POST /mobile/api/say       { session, text }: the phone's words for that session (talk.rs, run.rs)
//   GET  /mobile/api/job       ?agent=<a>&started=<ms>: how a run from the phone goes, its session (run.rs)
//   POST /mobile/api/notice    { pkg, app, title, text }: a notification on the phone, shown on the island (notice)
//   POST /mobile/api/clip      { text }: the phone's text onto the computer's clipboard
//   GET  /mobile/api/history   ?agent=&q=&offset=&limit=: every conversation the agents keep, the latest first (history.rs)
//   GET  /mobile/api/ask       asking her (ask.rs): the questions and answers, what is under way
//   POST /mobile/api/ask       { agent, text }: a question; POST /mobile/api/ask/clear: a new one
//   POST /mobile/api/drop      a file shared from the phone (its name in x-wakuwaku-name): into Downloads\Wakuwaku
//
// On this computer (127.0.0.1) the page needs nothing. With "phone access"
// on (the setting mobile), the same port is listened on at one more address
// as well: Tailscale's (100.64.0.0/10, found as she starts and every little
// while), 0.0.0.0, or an address set (mobileBind). A request from anywhere
// but this computer, through that address or a proxy in front of the port
// (tailscale serve: a forwarding header, or a host not this one), may ask for
// /mobile/ alone, and its data only with the token (mobileToken: a cookie,
// ?t=, or the x-wakuwaku-token header); the hooks and everything else are
// this computer's. The two that read a transcript or start a task ask for
// both, phone access on and the token, from this computer too.
//
// ntfy (the setting ntfy: { on, server, topic }): a done, changed, failed or
// waiting session told on the phone, by its agent and how it is alone, a tap
// opening the page.
use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use tiny_http::{Header, Request, Response, Server};

use crate::{data, i18n, now_ms, Shared};

const COOKIE: &str = "wk";
// How often the address listened on is looked at again: Tailscale up after
// her, or its address changed.
const LOOK_EVERY: Duration = Duration::from_secs(15);
pub const NTFY_SERVER: &str = "https://ntfy.sh";

const PAGE: &str = include_str!("../../src/mobile/index.html");
const APP: &str = include_str!("../../src/mobile/app.js");
const STYLE: &str = include_str!("../../src/mobile/style.css");
const MANIFEST: &str = include_str!("../../src/mobile/manifest.webmanifest");
const WORKER: &str = include_str!("../../src/mobile/sw.js");
const I18N: &str = include_str!("../../src/shared/i18n.js");
const STATUS: &str = include_str!("../../src/pet/status.js");
const SPRITE: &str = include_str!("../../src/pet/sprite.js");
const ICON_192: &[u8] = include_bytes!("../../src/mobile/icon-192.png");
const ICON_512: &[u8] = include_bytes!("../../src/mobile/icon-512.png");

// The extra address listened on, and why there is none when one is wanted.
struct Listening {
    on: Option<(IpAddr, Arc<Server>)>,
    problem: String,
}
static LISTENING: Mutex<Listening> = Mutex::new(Listening { on: None, problem: String::new() });
// When the phone last asked for anything of its own (ms), 0 never.
static SEEN: AtomicU64 = AtomicU64::new(0);
// The phone's battery as its app last said (percent, charging, when), and
// when a ring was asked for it from the tray (0 none).
static BATTERY: Mutex<Option<(u8, bool, u64)>> = Mutex::new(None);
static RING: AtomicU64 = AtomicU64::new(0);
// The phone's last notifications, the latest first, for the settings to
// list (and mute an app from): in memory alone, gone as she quits.
static NOTICES: Mutex<std::collections::VecDeque<Value>> = Mutex::new(std::collections::VecDeque::new());
const NOTICES_KEPT: usize = 20;
// The island opened for a notification at most once in so long; those
// between wait and are counted ("另外還有 3 則") with the next, so a busy
// group chat does not open and close it over and over (the user saw her
// "twitch", 2026-10-22). A one-time code opens it at once, always.
static POPPED: Mutex<(u64, u32)> = Mutex::new((0, 0));
const POP_EVERY_MS: u64 = 20_000;
// The clipboard's change count last given to the phone (or set from it): a
// change since is sent with its next ask; 0 until first looked at.
static CLIP_SEEN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
const CLIP_MOST: usize = 20_000;
// A file from the phone: so large at most.
const DROP_MOST: u64 = 1024 * 1024 * 1024;
// A ring not taken by the phone in so long is forgotten (it was away).
const RING_MS: u64 = 120_000;
// The phone's battery off the island once its app has not asked in so long.
const BATTERY_GONE_MS: u64 = 3 * 60_000;

// --- Who is asking --------------------------------------------------------------------

fn header<'a>(req: &'a Request, name: &str) -> Option<&'a str> {
    req.headers().iter().find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name)).map(|h| h.value.as_str())
}

// A host this computer is called by: 127.0.0.1, localhost, [::1], any port.
fn is_loopback_host(host: &str) -> bool {
    let name = if host.starts_with('[') { host.split(']').next().map_or(host, |h| &h[1..]) } else { host.rsplit_once(':').map_or(host, |(name, _)| name) };
    matches!(name.to_ascii_lowercase().as_str(), "127.0.0.1" | "localhost" | "::1")
}

// From this computer itself: not through the extra address, nor through a
// proxy in front of the port (tailscale serve passes the phone's on from
// 127.0.0.1, saying so in its headers and its host).
pub fn is_local(req: &Request) -> bool {
    let loopback = req.remote_addr().is_some_and(|a| a.ip().is_loopback());
    let forwarded = req.headers().iter().any(|h| {
        let field = h.field.as_str().as_str().to_ascii_lowercase();
        field == "x-forwarded-for" || field == "forwarded" || field.starts_with("tailscale-")
    });
    loopback && !forwarded && header(req, "host").map_or(true, is_loopback_host)
}

fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

// %XX and + in a query's value.
fn decode(v: &str) -> String {
    let b = v.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                let hex = |c: u8| (c as char).to_digit(16);
                match (hex(b[i + 1]), hex(b[i + 2])) {
                    (Some(h), Some(l)) => {
                        out.push((h * 16 + l) as u8);
                        i += 3;
                        continue;
                    }
                    _ => out.push(b'%'),
                }
            }
            b'+' => out.push(b' '),
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn query_of<'a>(url: &'a str, key: &str) -> Option<&'a str> {
    url.split_once('?')?.1.split('&').find_map(|p| p.strip_prefix(key)?.strip_prefix('='))
}

fn cookie_of<'a>(req: &'a Request, name: &str) -> Option<&'a str> {
    header(req, "cookie")?.split(';').find_map(|p| p.trim().strip_prefix(name)?.strip_prefix('='))
}

fn token(sh: &Shared) -> String {
    sh.setting("mobileToken").as_str().unwrap_or("").to_string()
}

// The token, in any of the three places it may come in.
fn has_token(sh: &Shared, req: &Request) -> bool {
    let want = token(sh);
    !want.is_empty() && [query_of(req.url(), "t"), cookie_of(req, COOKIE), header(req, "x-wakuwaku-token")].into_iter().flatten().any(|got| same(got, &want))
}

// --- Random words -----------------------------------------------------------------------

// n random bytes as hex, from the system's source.
pub fn random_hex(n: usize) -> String {
    let mut bytes = vec![0u8; n];
    getrandom::fill(&mut bytes).expect("the system's random source");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn new_token() -> String {
    random_hex(16)
}

pub fn new_topic() -> String {
    format!("wakuwaku-{}", random_hex(10))
}

// A token and a topic of her own, the first time she starts (or after one
// was cleared by hand); true when either was made.
pub fn fill_secrets(settings: &mut serde_json::Map<String, Value>) -> bool {
    let mut made = false;
    if settings.get("mobileToken").and_then(Value::as_str).unwrap_or("").is_empty() {
        settings.insert("mobileToken".into(), json!(new_token()));
        made = true;
    }
    let mut ntfy = settings.get("ntfy").and_then(Value::as_object).cloned().unwrap_or_default();
    if ntfy.get("topic").and_then(Value::as_str).unwrap_or("").is_empty() {
        ntfy.insert("topic".into(), json!(new_topic()));
        settings.insert("ntfy".into(), Value::Object(ntfy));
        made = true;
    }
    made
}

// --- Where she listens --------------------------------------------------------------------

pub fn is_tailscale(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    a == 100 && (b & 0xC0) == 64
}

// The address the way to `to` leaves from (a UDP socket aimed there: nothing
// is sent).
fn source_for(to: &str) -> Option<IpAddr> {
    let socket = UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    socket.connect(to).ok()?;
    Some(socket.local_addr().ok()?.ip())
}

// This computer's Tailscale address: the one the way to Tailscale's own
// (100.100.100.100, its DNS) leaves from, while Tailscale is up.
pub fn tailscale_ip() -> Option<Ipv4Addr> {
    match source_for("100.100.100.100:53")? {
        IpAddr::V4(ip) if is_tailscale(ip) => Some(ip),
        _ => None,
    }
}

// The address the setting asks for: none while phone access is off; Err
// while Tailscale's is wanted and there is none.
fn wanted(sh: &Shared) -> Result<Option<IpAddr>, String> {
    if !sh.flag("mobile") {
        return Ok(None);
    }
    match sh.setting("mobileBind").as_str().unwrap_or("tailscale") {
        "tailscale" | "" => tailscale_ip().map(|ip| Some(IpAddr::V4(ip))).ok_or_else(|| "noTailscale".to_string()),
        other => other.parse::<IpAddr>().map(Some).map_err(|_| "badAddress".to_string()),
    }
}

// Listen where the settings say now: the old address let go, the new one
// taken. Called as she starts, as the settings change, and every little while.
pub fn refresh(sh: &Arc<Shared>) {
    let want = wanted(sh);
    let target = want.clone().ok().flatten();
    let mut said = None;
    {
        let mut l = LISTENING.lock().unwrap();
        let current = l.on.as_ref().map(|(ip, _)| *ip);
        if current != target {
            if let Some((ip, server)) = l.on.take() {
                server.unblock();
                said = Some(format!("phone access: no longer on {ip}"));
            }
            if let Some(ip) = target {
                match Server::http((ip, sh.port)) {
                    Ok(server) => {
                        let server = Arc::new(server);
                        let (owner, s, port) = (sh.clone(), server.clone(), sh.port);
                        crate::spawn_named("mobile", move || {
                            for req in s.incoming_requests() {
                                crate::server::handle(&owner, req, false);
                            }
                            // Let go (the last hold on it): it wakes its own
                            // listener by calling itself, which cannot reach
                            // 0.0.0.0 on Windows, and the port stayed taken
                            // there. A knock from another of this
                            // computer's addresses does.
                            drop(s);
                            if ip.is_unspecified() {
                                knock(port);
                            }
                        });
                        l.on = Some((ip, server));
                        said = Some(format!("phone access: on {ip}:{}", sh.port));
                    }
                    // Said once, not every time it is tried again.
                    Err(err) => {
                        let problem = format!("bind: {err}");
                        if l.problem != problem {
                            said = Some(format!("phone access: {ip}:{} not taken ({err})", sh.port));
                            l.problem = problem;
                        }
                    }
                }
            }
        }
        if l.on.is_some() || target.is_none() {
            l.problem = want.err().unwrap_or_default();
        }
    }
    if let Some(line) = said {
        sh.log(&line);
        sh.push_settings();
    }
}

// A connection to the port at one of this computer's own addresses (not
// 127.0.0.1: that is the hooks' listener), opened and let go.
fn knock(port: u16) {
    let mine = [source_for("8.8.8.8:53"), tailscale_ip().map(IpAddr::V4)];
    if let Some(ip) = mine.into_iter().flatten().find(|ip| !ip.is_loopback()) {
        let _ = std::net::TcpStream::connect_timeout(&(ip, port).into(), Duration::from_secs(1));
    }
}

// Looked at again every little while, from the start.
pub fn start(sh: &Arc<Shared>) {
    refresh(sh);
    let sh = sh.clone();
    crate::spawn_named("mobile-watch", move || loop {
        std::thread::sleep(LOOK_EVERY);
        refresh(&sh);
    });
}

fn listening() -> Option<IpAddr> {
    LISTENING.lock().unwrap().on.as_ref().map(|(ip, _)| *ip)
}

// The page's address as the phone reaches it: the one set (a proxy's, as
// tailscale serve gives), else the address listened on (for 0.0.0.0,
// Tailscale's, else this computer's on its way out); with the token, for the
// QR code. None while there is none.
pub fn page_url(sh: &Shared, with_token: bool) -> Option<String> {
    let set = sh.setting("mobileUrl").as_str().unwrap_or("").trim().trim_end_matches('/').to_string();
    let base = if !set.is_empty() {
        set.trim_end_matches("/mobile").to_string()
    } else {
        let ip = match listening()? {
            IpAddr::V4(ip) if ip.is_unspecified() => tailscale_ip().map(IpAddr::V4).or_else(|| source_for("8.8.8.8:53"))?,
            ip => ip,
        };
        match ip {
            IpAddr::V6(ip) => format!("http://[{ip}]:{}", sh.port),
            ip => format!("http://{ip}:{}", sh.port),
        }
    };
    Some(if with_token { format!("{base}/mobile/?t={}", token(sh)) } else { format!("{base}/mobile/") })
}

// What the settings show of it.
pub fn view(sh: &Shared) -> Value {
    let (on, problem) = {
        let l = LISTENING.lock().unwrap();
        (l.on.as_ref().map(|(ip, _)| ip.to_string()), l.problem.clone())
    };
    let seen = SEEN.load(Ordering::SeqCst);
    let now = now_ms();
    // The specifics kept off screen: who and when alone.
    let details = sh.setting("details") != false;
    let notices: Vec<Value> = NOTICES
        .lock()
        .unwrap()
        .iter()
        .map(|n| {
            let mut n = n.clone();
            n["ago"] = json!(now.saturating_sub(n["at"].as_u64().unwrap_or(now)));
            if !details {
                n["title"] = json!("");
                n["text"] = json!("");
            }
            n
        })
        .collect();
    let battery = BATTERY.lock().unwrap().filter(|b| now.saturating_sub(b.2) <= BATTERY_GONE_MS).map(|b| json!({ "level": b.0, "charging": b.1 }));
    json!({
        "notices": notices,
        "battery": battery,
        "listening": on,
        "problem": problem,
        "url": page_url(sh, true),
        "tailscale": tailscale_ip().map(|ip| ip.to_string()),
        "seenAgo": (seen > 0).then(|| now_ms().saturating_sub(seen)),
    })
}

// --- The page and its data ----------------------------------------------------------------

fn respond(req: Request, code: u16, kind: &str, body: Vec<u8>, extra: &[(&str, String)]) {
    let mut response = Response::from_data(body).with_status_code(code).with_header(Header::from_bytes("content-type", kind).expect("a valid header"));
    response.add_header(Header::from_bytes("x-content-type-options", "nosniff").expect("a valid header"));
    response.add_header(Header::from_bytes("referrer-policy", "no-referrer").expect("a valid header"));
    for (name, value) in extra {
        if let Ok(h) = Header::from_bytes(name.as_bytes(), value.as_bytes()) {
            response.add_header(h);
        }
    }
    let _ = req.respond(response);
}

fn json_reply(req: Request, code: u16, body: Value) {
    respond(req, code, "application/json", body.to_string().into_bytes(), &[("cache-control", "no-store".into())])
}

fn redirect(req: Request, to: &str, cookie: Option<String>) {
    let mut extra = vec![("location", to.to_string()), ("cache-control", "no-store".to_string())];
    if let Some(c) = cookie {
        extra.push(("set-cookie", c));
    }
    respond(req, 302, "text/plain", Vec::new(), &extra);
}

fn sheet_of(sh: &Shared, agent: &str) -> Option<data::PetInfo> {
    let settings = sh.settings.lock().unwrap().clone();
    let found = data::pets(&sh.dir);
    data::pet_of(&settings, Some(agent), &found).cloned()
}

// Each agent: its pet (a sheet's address that changes as the pet does), the
// session it shows and its sessions.
fn state(sh: &Shared) -> Value {
    let details = sh.setting("details") != false;
    let now = now_ms();
    let agents: Vec<Value> = data::AGENTS
        .iter()
        .map(|agent| {
            let (mut shown, mut list, mut closed) = {
                let pet = sh.pet.lock().unwrap();
                (pet.get_own_for(agent, now), pet.list_own_for(agent), pet.closed_for(agent, 5))
            };
            // With the specifics kept off screen, no replies or errors either.
            if !details {
                for s in std::iter::once(&mut shown).chain(list.as_array_mut().into_iter().flatten()).chain(closed.as_array_mut().into_iter().flatten()) {
                    s["reply"] = json!("");
                    s["error"] = Value::Null;
                }
            }
            let pet = sheet_of(sh, agent);
            let stamp = pet.as_ref().and_then(|p| std::fs::metadata(&p.sheet).and_then(|m| m.modified()).ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
            json!({
                "agent": agent,
                "now": shown,
                "list": list,
                // Closed on the computer lately, out of her sight: still to be read and gone on with.
                "closed": closed,
                "sheet": pet.as_ref().map(|p| format!("sheet/{agent}?pet={}&m={stamp}", p.id)),
                "version": pet.as_ref().map_or(2, |p| p.version),
                "pet": pet.as_ref().map(|p| p.name.clone()),
                "color": pet.as_ref().and_then(|p| p.color.clone()).unwrap_or_else(|| data::agent_color(agent).to_string()),
                // A task may be given it (its project folder set), and the one from the phone running.
                "canRun": crate::run::folder(sh, agent).is_some(),
                "running": crate::run::running(agent),
            })
        })
        .collect();
    // Its name: the phone and her other computers show its sessions by it.
    json!({ "lang": sh.lang(), "details": details, "at": now, "machine": crate::peers::machine(), "agents": agents })
}

// A session's whole conversation, and how the phone may add to it: { session,
// agent, messages: [{ role, text, clipped }], truncated, live, canSay, how
// (resume | queue), why (live, busy, noCwd) }.
// A session as the phone's conversation needs it: one she has seen, else
// one of the agents' own records (history.rs).
fn talk_of(sh: &Shared, id: &str) -> Option<crate::state::Talk> {
    if let Some(t) = sh.pet.lock().unwrap().talk_of(id) {
        return Some(t);
    }
    let e = crate::history::find(id)?;
    Some(crate::state::Talk { agent: e.agent.to_string(), transcript: e.transcript, cwd: e.cwd, chain: Vec::new(), reply: String::new(), mood: "idle".into() })
}

fn conversation(sh: &Shared, id: &str) -> Option<Value> {
    let t = talk_of(sh, id)?;
    let hidden = sh.setting("details") == false;
    let (messages, truncated, summary_only) = match (!hidden && !t.transcript.is_empty()).then(|| crate::talk::conversation(&t.agent, &t.transcript)).flatten() {
        Some((m, cut)) if !m.is_empty() => (Value::Array(m), cut, false),
        // No transcript: what the island knows of it, said so.
        _ if !hidden && !t.reply.is_empty() => (json!([{ "role": "assistant", "text": t.reply, "clipped": false }]), false, true),
        _ => (json!([]), false, !hidden),
    };
    let live = crate::talk::is_live(&t.agent, id, &t.chain);
    let busy = crate::run::running(&t.agent).is_some();
    let has_cwd = !t.cwd.is_empty() && std::path::Path::new(&t.cwd).is_dir() && crate::run::is_id(id);
    let way = crate::talk::way(&t.agent, live, busy, has_cwd);
    Some(json!({
        "session": id,
        "agent": t.agent,
        "mood": t.mood,
        "messages": messages,
        "truncated": truncated,
        "summaryOnly": summary_only,
        "hidden": hidden,
        "live": live,
        "busy": busy,
        "canSay": way.is_ok() && !hidden,
        "how": way.ok(),
        "why": way.err(),
    }))
}

// The phone's words for a session: resumed, or queued into it while it is
// open (Codex); refused where they would fork it.
fn say(sh: &Shared, id: &str, text: &str) -> Result<Value, &'static str> {
    let t = talk_of(sh, id).ok_or("no such session")?;
    let live = crate::talk::is_live(&t.agent, id, &t.chain);
    let busy = crate::run::running(&t.agent).is_some();
    let dir = std::path::PathBuf::from(&t.cwd);
    let how = crate::talk::way(&t.agent, live, busy, !t.cwd.is_empty() && dir.is_dir())?;
    let mode = if how == "queue" { crate::run::Mode::Queue(id.to_string()) } else { crate::run::Mode::Resume(id.to_string()) };
    crate::run::say(sh, &t.agent, text, &dir, mode)
}

// A notification on the phone (its app's name and package, title, text):
// on the island for a moment as a widget's nudge is, the specifics kept off
// screen as hers are (details), a code in it copied by a click. Not while
// a session wants you, she is out of sight (hidden, full screen), the
// setting is off, or its app is muted. Nothing of it kept or logged.
fn notice(sh: &Shared, body: &Value) -> bool {
    let field = |k: &str, most: usize| body[k].as_str().map(|s| clip_chars(s.trim(), most)).unwrap_or_default();
    let (pkg, app, title, text) = (field("pkg", 200), field("app", 60), field("title", 120), field("text", 400));
    if title.is_empty() && text.is_empty() {
        return false;
    }
    {
        let mut kept = NOTICES.lock().unwrap();
        kept.push_front(json!({ "pkg": pkg, "app": app, "title": title, "text": text, "at": now_ms() }));
        kept.truncate(NOTICES_KEPT);
    }
    sh.push_settings();
    if sh.setting("phoneNotices") == false || sh.setting("phoneNoticeMute").as_array().is_some_and(|m| m.iter().any(|p| p.as_str() == Some(pkg.as_str()))) {
        return false;
    }
    if !sh.is_visible() || sh.pet.lock().unwrap().wants_you() {
        return false;
    }
    let code = code_of(&format!("{title} {text}"));
    let held = {
        let mut popped = POPPED.lock().unwrap();
        let now = now_ms();
        if !may_pop(now, popped.0, code.is_some()) {
            popped.1 += 1;
            return false;
        }
        let held = popped.1;
        *popped = (now, 0);
        held
    };
    let words = if text.is_empty() { title.clone() } else { text.clone() };
    let mut label = [app.as_str(), if text.is_empty() { "" } else { title.as_str() }].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" · ");
    if held > 0 {
        label.push_str(&crate::i18n::t(sh.lang(), "phone.more").replace("{n}", &held.to_string()));
    }
    let value = code.as_ref().map(|c| format!("{c} · {}", crate::i18n::t(sh.lang(), "phone.copy"))).unwrap_or_default();
    sh.to_pages("pet:nudge", json!({ "id": "phone", "icon": "phone", "words": words, "label": label, "value": value, "private": true, "open": code.map(|c| json!({ "copy": c })) }));
    true
}

// Whether a notification may open the island now (the last opened at `last`).
pub fn may_pop(now: u64, last: u64, is_code: bool) -> bool {
    is_code || last == 0 || now.saturating_sub(last) >= POP_EVERY_MS
}

fn clip_chars(s: &str, most: usize) -> String {
    match s.char_indices().nth(most) {
        Some((at, _)) => format!("{}…", &s[..at]),
        None => s.to_string(),
    }
}

// A one-time code in a message that says it is one: its first run of 4 to 8 digits.
pub fn code_of(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    const SAYS: [&str; 9] = ["驗證碼", "验证码", "認證碼", "认证码", "驗證", "code", "otp", "密碼", "passcode"];
    if !SAYS.iter().any(|w| lower.contains(w)) {
        return None;
    }
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            if (4..=8).contains(&(i - start)) {
                return Some(chars[start..i].iter().collect());
            }
        } else {
            i += 1;
        }
    }
    None
}

// The phone's battery ("80", "80+" while charging): the island's phone
// widget, and a word as it falls to 20% and to 10% unplugged.
fn phone_battery(sh: &Shared, said: &str) {
    let charging = said.ends_with('+');
    let Some(level) = said.trim_end_matches('+').trim().parse::<u8>().ok().filter(|l| *l <= 100) else { return };
    let now = now_ms();
    let before = BATTERY.lock().unwrap().replace((level, charging, now));
    let words = if charging { format!("{level}% ⚡") } else { format!("{level}%") };
    if sh.widgets.lock().unwrap().set_built_in("phone", json!(words), now) {
        sh.redraw_widgets();
    }
    if !charging {
        let was = before.filter(|b| !b.1).map_or(101, |b| b.0);
        if [20u8, 10].iter().any(|m| level <= *m && was > *m) {
            sh.nudge_widget("phone", &crate::i18n::t(sh.lang(), "phone.low").replace("{n}", &level.to_string()));
        }
    }
}

// Not heard from in a while: its battery off the island. True when it went.
pub fn phone_gone(sh: &Shared) -> bool {
    let stale = BATTERY.lock().unwrap().is_some_and(|b| now_ms().saturating_sub(b.2) > BATTERY_GONE_MS);
    if !stale {
        return false;
    }
    *BATTERY.lock().unwrap() = None;
    sh.widgets.lock().unwrap().remove_built_in("phone")
}

// The tray's "ring the phone": taken by its app's next ask.
pub fn ring() {
    RING.store(now_ms(), Ordering::SeqCst);
}

fn take_ring() -> bool {
    let at = RING.swap(0, Ordering::SeqCst);
    at != 0 && now_ms().saturating_sub(at) < RING_MS
}

// The computer's clipboard for the phone, if its text changed since it was
// last given (or set from it). Not with clipToPhone off; none the first
// time it is looked at (what was there before is not news).
fn clip_for_phone(sh: &Shared) -> Option<String> {
    if sh.setting("clipToPhone") == false {
        return None;
    }
    let seq = crate::clip::seq();
    let seen = CLIP_SEEN.swap(seq, Ordering::SeqCst);
    if seen == 0 || seen == seq {
        return None;
    }
    crate::clip::read(CLIP_MOST).1
}

// The phone's text onto the clipboard, not sent back to it; said on the island.
fn from_phone_clip(sh: &Shared, text: &str) -> bool {
    let Some(seq) = crate::clip::write(text) else { return false };
    CLIP_SEEN.store(seq, Ordering::SeqCst);
    let lang = sh.lang();
    sh.to_pages("pet:nudge", json!({ "id": "phone", "icon": "phone", "words": crate::i18n::t(lang, "phone.clip"), "label": clip_chars(text.trim(), 40), "value": "", "private": true, "open": null }));
    true
}

// Where the phone's files go: Downloads\Wakuwaku.
pub fn drops_dir() -> std::path::PathBuf {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(std::path::PathBuf::from).unwrap_or_default();
    home.join("Downloads").join("Wakuwaku")
}

// A file's name as Windows takes it: no path, none of \ / : * ? " < > |,
// no control characters, not ending in a dot or a space; none at all, the time.
pub fn safe_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("");
    let clean: String = base.chars().filter(|c| !c.is_control() && !"\\/:*?\"<>|".contains(*c)).take(120).collect();
    let clean = clean.trim().trim_end_matches(['.', ' ']).to_string();
    if clean.is_empty() || clean.chars().all(|c| c == '.') {
        format!("phone-{}", now_ms())
    } else {
        clean
    }
}

// Not over one there: "photo (2).jpg".
fn free_path(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let (stem, ext) = match name.rfind('.') {
        Some(at) if at > 0 => (&name[..at], &name[at..]),
        _ => (name, ""),
    };
    (2..1000).map(|n| dir.join(format!("{stem} ({n}){ext}"))).find(|p| !p.exists()).unwrap_or(first)
}

// A file from the phone, written as it comes; said on the island (a click: the folder).
fn drop_file(sh: &Shared, req: &mut Request, name: &str) -> Result<String, &'static str> {
    let dir = drops_dir();
    std::fs::create_dir_all(&dir).map_err(|_| "io")?;
    let path = free_path(&dir, &safe_name(name));
    let part = dir.join(format!("{}.part", path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
    let mut file = std::fs::File::create(&part).map_err(|_| "io")?;
    let copied = std::io::copy(&mut req.as_reader().take(DROP_MOST + 1), &mut file);
    drop(file);
    match copied {
        Ok(n) if n > 0 && n <= DROP_MOST => {}
        _ => {
            let _ = std::fs::remove_file(&part);
            return Err("size");
        }
    }
    std::fs::rename(&part, &path).map_err(|_| "io")?;
    let saved = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let lang = sh.lang();
    sh.to_pages("pet:nudge", json!({ "id": "phone", "icon": "down", "words": crate::i18n::t(lang, "phone.got").replace("{name}", &saved), "label": crate::i18n::t(lang, "phone.gotWhere"), "value": "", "private": false, "open": { "drops": true } }));
    sh.log("phone: a file into Downloads\\Wakuwaku");
    Ok(saved)
}

// Whether the phone's app has been heard from lately (the tray offers to ring it).
pub fn phone_near() -> bool {
    BATTERY.lock().unwrap().is_some_and(|b| now_ms().saturating_sub(b.2) <= BATTERY_GONE_MS)
}

fn read_json(req: &mut Request) -> Option<Value> {
    let mut bytes = Vec::new();
    req.as_reader().take(64 * 1024).read_to_end(&mut bytes).ok()?;
    serde_json::from_slice(&bytes).ok()
}

// A request for /mobile…, from this computer (`local`) or from elsewhere.
pub fn handle(sh: &Shared, mut req: Request, local: bool) {
    let path = req.url().split('?').next().unwrap_or("").to_string();
    let method = req.method().as_str().to_string();
    // From elsewhere: only while phone access is on.
    if !local && !sh.flag("mobile") {
        return json_reply(req, 404, json!({ "error": "not found" }));
    }
    let allowed = local || has_token(sh, &req);
    match (method.as_str(), path.as_str()) {
        ("GET", "/mobile") => {
            let to = match req.url().split_once('?') {
                Some((_, q)) => format!("/mobile/?{q}"),
                None => "/mobile/".to_string(),
            };
            redirect(req, &to, None)
        }
        ("GET", "/mobile/") => {
            // Come by the QR code: the token kept as a cookie, and taken out
            // of the address (the history, a screenshot, a bookmark).
            if query_of(req.url(), "t").is_some() {
                let cookie = has_token(sh, &req).then(|| {
                    let secure = header(&req, "x-forwarded-proto") == Some("https");
                    format!("{COOKIE}={}; Path=/mobile; Max-Age=31536000; HttpOnly; SameSite=Lax{}", token(sh), if secure { "; Secure" } else { "" })
                });
                return redirect(req, "/mobile/", cookie);
            }
            respond(req, 200, "text/html; charset=utf-8", PAGE.as_bytes().to_vec(), &[("cache-control", "no-cache".into())])
        }
        ("GET", "/mobile/app.js") => respond(req, 200, "text/javascript; charset=utf-8", APP.as_bytes().to_vec(), &[("cache-control", "no-cache".into())]),
        ("GET", "/mobile/i18n.js") => respond(req, 200, "text/javascript; charset=utf-8", I18N.as_bytes().to_vec(), &[("cache-control", "no-cache".into())]),
        ("GET", "/mobile/status.js") => respond(req, 200, "text/javascript; charset=utf-8", STATUS.as_bytes().to_vec(), &[("cache-control", "no-cache".into())]),
        ("GET", "/mobile/sprite.js") => respond(req, 200, "text/javascript; charset=utf-8", SPRITE.as_bytes().to_vec(), &[("cache-control", "no-cache".into())]),
        ("GET", "/mobile/style.css") => respond(req, 200, "text/css; charset=utf-8", STYLE.as_bytes().to_vec(), &[("cache-control", "no-cache".into())]),
        ("GET", "/mobile/manifest.webmanifest") => respond(req, 200, "application/manifest+json", MANIFEST.as_bytes().to_vec(), &[("cache-control", "no-cache".into())]),
        ("GET", "/mobile/sw.js") => respond(req, 200, "text/javascript; charset=utf-8", WORKER.as_bytes().to_vec(), &[("cache-control", "no-cache".into())]),
        ("GET", "/mobile/icon-192.png") => respond(req, 200, "image/png", ICON_192.to_vec(), &[("cache-control", "max-age=86400".into())]),
        ("GET", "/mobile/icon-512.png") => respond(req, 200, "image/png", ICON_512.to_vec(), &[("cache-control", "max-age=86400".into())]),
        // Phone access on and the token, wherever they come from.
        ("GET", "/mobile/api/reply" | "/mobile/api/conversation" | "/mobile/api/job" | "/mobile/api/ask" | "/mobile/api/history") | ("POST", "/mobile/api/run" | "/mobile/api/say" | "/mobile/api/notice" | "/mobile/api/clip" | "/mobile/api/drop" | "/mobile/api/ask" | "/mobile/api/ask/clear") if !sh.flag("mobile") => json_reply(req, 403, json!({ "error": "phone access off" })),
        ("GET", "/mobile/api/reply" | "/mobile/api/conversation" | "/mobile/api/job" | "/mobile/api/ask" | "/mobile/api/history") | ("POST", "/mobile/api/run" | "/mobile/api/say" | "/mobile/api/notice" | "/mobile/api/clip" | "/mobile/api/drop" | "/mobile/api/ask" | "/mobile/api/ask/clear") if !has_token(sh, &req) => json_reply(req, 401, json!({ "error": "token" })),
        ("GET", "/mobile/api/conversation") => {
            let id = query_of(req.url(), "session").map(decode).unwrap_or_default();
            match conversation(sh, &id) {
                Some(body) => json_reply(req, 200, body),
                None => json_reply(req, 404, json!({ "error": "no such session" })),
            }
        }
        ("POST", "/mobile/api/say") if header(&req, "x-wakuwaku").is_some() => {
            let Some(body) = read_json(&mut req) else { return json_reply(req, 400, json!({ "error": "bad json" })) };
            let id = body["session"].as_str().unwrap_or("").to_string();
            match say(sh, &id, body["text"].as_str().unwrap_or("")) {
                Ok(done) => json_reply(req, 200, done),
                Err(why) => json_reply(req, if matches!(why, "busy" | "live") { 409 } else if why == "no such session" { 404 } else { 400 }, json!({ "error": why })),
            }
        }
        ("POST", "/mobile/api/notice") if header(&req, "x-wakuwaku").is_some() => {
            let Some(body) = read_json(&mut req) else { return json_reply(req, 400, json!({ "error": "bad json" })) };
            let shown = notice(sh, &body);
            json_reply(req, 200, json!({ "ok": true, "shown": shown }))
        }
        ("POST", "/mobile/api/clip") if header(&req, "x-wakuwaku").is_some() => {
            let Some(body) = read_json(&mut req) else { return json_reply(req, 400, json!({ "error": "bad json" })) };
            let text = body["text"].as_str().unwrap_or("");
            if text.trim().is_empty() || text.chars().count() > CLIP_MOST {
                return json_reply(req, 400, json!({ "error": "text" }));
            }
            let ok = from_phone_clip(sh, text);
            json_reply(req, 200, json!({ "ok": ok }))
        }
        ("POST", "/mobile/api/drop") if header(&req, "x-wakuwaku").is_some() => {
            let name = header(&req, "x-wakuwaku-name").map(decode).unwrap_or_default();
            match drop_file(sh, &mut req, &name) {
                Ok(saved) => json_reply(req, 200, json!({ "ok": true, "name": saved })),
                Err(why) => json_reply(req, 400, json!({ "error": why })),
            }
        }
        ("GET", "/mobile/api/ask") => json_reply(req, 200, crate::ask::view(sh)),
        ("GET", "/mobile/api/history") => {
            let q = |k: &str| query_of(req.url(), k).map(decode).unwrap_or_default();
            let agent = q("agent");
            let agent = if crate::data::AGENTS.contains(&agent.as_str()) { agent } else { String::new() };
            let offset = q("offset").parse().unwrap_or(0);
            let limit = q("limit").parse::<usize>().unwrap_or(50).clamp(1, 200);
            let body = crate::history::list(sh, &agent, &q("q"), offset, limit);
            json_reply(req, 200, body)
        }
        ("POST", "/mobile/api/ask") if header(&req, "x-wakuwaku").is_some() => {
            let Some(body) = read_json(&mut req) else { return json_reply(req, 400, json!({ "error": "bad json" })) };
            let ok = crate::ask::send(&crate::shared(&sh.app), body["agent"].as_str().unwrap_or("claude"), body["text"].as_str().unwrap_or(""));
            json_reply(req, if ok { 200 } else { 409 }, json!({ "ok": ok }))
        }
        ("POST", "/mobile/api/ask/clear") if header(&req, "x-wakuwaku").is_some() => {
            crate::ask::clear();
            json_reply(req, 200, json!({ "ok": true }))
        }
        ("GET", "/mobile/api/job") => {
            let agent = query_of(req.url(), "agent").map(decode).unwrap_or_default();
            let started = query_of(req.url(), "started").and_then(|s| s.parse().ok()).unwrap_or(0);
            match crate::run::job(sh, &agent, started) {
                Some(body) => json_reply(req, 200, body),
                None => json_reply(req, 404, json!({ "error": "no such run" })),
            }
        }
        ("GET", "/mobile/api/reply") => {
            let id = query_of(req.url(), "session").map(decode).unwrap_or_default();
            let found = sh.pet.lock().unwrap().transcript_of(&id);
            let Some((agent, transcript, summary)) = found else { return json_reply(req, 404, json!({ "error": "no such session" })) };
            // The specifics kept off screen: none of it.
            if sh.setting("details") == false {
                return json_reply(req, 200, json!({ "session": id, "agent": agent, "text": "", "hidden": true }));
            }
            let whole = (!transcript.is_empty()).then(|| crate::reply::last_reply(&agent, &transcript)).flatten();
            let summary_only = whole.is_none();
            let (text, clipped) = crate::reply::clip(&whole.unwrap_or(summary));
            json_reply(req, 200, json!({ "session": id, "agent": agent, "text": text, "clipped": clipped, "summaryOnly": summary_only }))
        }
        // As an answer: only with the header no other site's page can send.
        ("POST", "/mobile/api/run") if header(&req, "x-wakuwaku").is_some() => {
            let Some(body) = read_json(&mut req) else { return json_reply(req, 400, json!({ "error": "bad json" })) };
            let agent = body["agent"].as_str().unwrap_or("");
            match crate::run::start(sh, agent, body["prompt"].as_str().unwrap_or(""), body["cwd"].as_str()) {
                Ok(started) => json_reply(req, 200, started),
                Err(why) => json_reply(req, if why == "busy" { 409 } else { 400 }, json!({ "error": why })),
            }
        }
        _ if !allowed => json_reply(req, 401, json!({ "error": "token" })),
        ("GET", "/mobile/api/state") => {
            SEEN.store(now_ms(), Ordering::SeqCst);
            let mut body = state(sh);
            // The phone's app (not the page): its battery, and a ring asked for it.
            if let Some(battery) = header(&req, "x-wakuwaku-battery") {
                // Where it is and its next event with a place (maps.rs), as it says.
                let event = header(&req, "x-wakuwaku-event").map(decode).and_then(|e| serde_json::from_str::<Value>(&e).ok());
                crate::maps::from_phone(header(&req, "x-wakuwaku-where").as_deref(), event.as_ref());
                body["maps"] = crate::maps::for_phone(sh);
                phone_battery(sh, &battery);
                body["ring"] = json!(take_ring());
                if let Some(text) = clip_for_phone(sh) {
                    body["clip"] = json!(text);
                }
            }
            json_reply(req, 200, body)
        }
        ("GET", "/mobile/api/asks") => {
            SEEN.store(now_ms(), Ordering::SeqCst);
            let asks = sh.asks.lock().unwrap().views();
            json_reply(req, 200, json!({ "asks": asks }))
        }
        // A header a page elsewhere cannot send without asking first (and
        // this port never says yes): no other site's page answers for you.
        ("POST", "/mobile/api/answer") if header(&req, "x-wakuwaku").is_some() => {
            let Some(body) = read_json(&mut req) else { return json_reply(req, 400, json!({ "error": "bad json" })) };
            let Some(id) = body["id"].as_u64() else { return json_reply(req, 400, json!({ "error": "no id" })) };
            let ok = sh.answer(id, &body["choice"]);
            sh.log(&format!("answer {id} from the phone page: {ok}"));
            json_reply(req, 200, json!({ "ok": ok }))
        }
        ("GET", p) if p.starts_with("/mobile/sheet/") => {
            let agent = &p["/mobile/sheet/".len()..];
            let bytes = data::AGENTS.contains(&agent).then(|| sheet_of(sh, agent)).flatten().and_then(|p| std::fs::read(&p.sheet).ok());
            match bytes {
                Some(bytes) => respond(req, 200, "image/webp", bytes, &[("cache-control", "private, max-age=604800".into())]),
                None => json_reply(req, 404, json!({ "error": "no pet" })),
            }
        }
        _ => json_reply(req, 404, json!({ "error": "not found" })),
    }
}

// --- ntfy ---------------------------------------------------------------------------------

pub fn agent_name(agent: &str) -> &'static str {
    match agent {
        "codex" => "Codex",
        "agy" => "Antigravity",
        "grok" => "Grok",
        _ => "Claude Code",
    }
}

// Where to post, from the setting: its server (https or http, ntfy.sh if
// none) and its topic.
fn ntfy_target(ntfy: &Value) -> Option<String> {
    let server = ntfy["server"].as_str().map(str::trim).filter(|s| !s.is_empty()).unwrap_or(NTFY_SERVER).trim_end_matches('/');
    let topic = ntfy["topic"].as_str().unwrap_or("");
    (is_server(server) && is_topic(topic)).then(|| format!("{server}/{topic}"))
}

pub fn is_server(s: &str) -> bool {
    (s.starts_with("https://") || s.starts_with("http://")) && s.len() <= 200 && !s.chars().any(|c| c.is_whitespace() || c.is_control())
}

pub fn is_topic(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

// The words of a push: its agent and how it is, nothing of what it worked on.
pub fn push_words(lang: &str, agent: &str, mood: &str) -> String {
    i18n::t(lang, &format!("push.{mood}")).replace("{agent}", agent_name(agent))
}

fn send(url: &str, words: &str, mood: &str, click: Option<String>) -> Result<(), String> {
    let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(15))).build().new_agent();
    let (tag, priority) = match mood {
        "waiting" => ("raising_hand", "high"),
        "error" => ("x", "default"),
        "test" => ("wave", "default"),
        _ => ("white_check_mark", "default"),
    };
    let mut post = agent.post(url).header("Title", "Wakuwaku").header("Tags", tag).header("Priority", priority);
    if let Some(click) = click {
        post = post.header("Click", click);
    }
    post.send(words).map(|_| ()).map_err(|e| e.to_string())
}

// A session done, changed, failed or waiting on you: told on the phone, off
// this thread.
pub fn push(sh: &Arc<Shared>, agent: &str, mood: &str) {
    let ntfy = sh.setting("ntfy");
    if ntfy["on"] != true {
        return;
    }
    let Some(url) = ntfy_target(&ntfy) else { return };
    let words = push_words(sh.lang(), agent, mood);
    let click = page_url(sh, false);
    let (sh, mood) = (sh.clone(), mood.to_string());
    std::thread::spawn(move || {
        if let Err(err) = send(&url, &words, &mood, click) {
            sh.log(&format!("ntfy failed: {err}"));
        }
    });
}

// One to try it, from the settings: whether it went.
pub fn push_test(sh: &Shared) -> Result<(), String> {
    let url = ntfy_target(&sh.setting("ntfy")).ok_or("no topic")?;
    send(&url, &i18n::t(sh.lang(), "push.test"), "test", page_url(sh, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_busy_chat_opens_the_island_once_in_a_while() {
        assert!(may_pop(1_000, 0, false));
        assert!(!may_pop(10_000, 1_000, false));
        assert!(may_pop(10_000, 1_000, true));
        assert!(may_pop(21_000, 1_000, false));
    }

    #[test]
    fn a_file_name_windows_takes() {
        assert_eq!(safe_name("IMG_0001.jpg"), "IMG_0001.jpg");
        assert_eq!(safe_name("../../evil:name?.txt"), "evilname.txt");
        assert_eq!(safe_name("C:\\x\\y.pdf"), "y.pdf");
        assert_eq!(safe_name("a<b>|c. "), "abc");
        assert!(safe_name("..").starts_with("phone-"));
        assert!(safe_name("").starts_with("phone-"));
    }

    #[test]
    fn a_one_time_code_where_the_message_says_so() {
        assert_eq!(code_of("您的驗證碼為 482913，5 分鐘內有效").as_deref(), Some("482913"));
        assert_eq!(code_of("Your Google code is G-123456").as_deref(), Some("123456"));
        assert_eq!(code_of("OTP: 12 34 9876").as_deref(), Some("9876"));
        assert_eq!(code_of("晚上 7 點 2024 號門口見"), None);
        assert_eq!(code_of("code 123456789"), None);
    }

    #[test]
    fn tailscale_addresses_are_the_cgnat_block() {
        assert!(is_tailscale(Ipv4Addr::new(100, 64, 0, 1)));
        assert!(is_tailscale(Ipv4Addr::new(100, 101, 102, 103)));
        assert!(is_tailscale(Ipv4Addr::new(100, 127, 255, 254)));
        assert!(!is_tailscale(Ipv4Addr::new(100, 63, 0, 1)));
        assert!(!is_tailscale(Ipv4Addr::new(100, 128, 0, 1)));
        assert!(!is_tailscale(Ipv4Addr::new(192, 168, 1, 2)));
    }

    #[test]
    fn hosts_of_this_computer() {
        assert!(is_loopback_host("127.0.0.1:47213"));
        assert!(is_loopback_host("localhost"));
        assert!(is_loopback_host("LOCALHOST:80"));
        assert!(is_loopback_host("[::1]:47213"));
        assert!(!is_loopback_host("100.101.102.103:47213"));
        assert!(!is_loopback_host("box.tail1234.ts.net"));
        assert!(!is_loopback_host("127.0.0.1.evil.example:47213"));
    }

    #[test]
    fn the_token_is_found_where_it_may_be() {
        assert_eq!(query_of("/mobile/?t=abc", "t"), Some("abc"));
        assert_eq!(query_of("/mobile/?x=1&t=abc", "t"), Some("abc"));
        assert_eq!(query_of("/mobile/?tt=abc", "t"), None);
        assert_eq!(query_of("/mobile/", "t"), None);
        assert!(same("abc", "abc"));
        assert!(!same("abc", "abd"));
        assert!(!same("abc", "abcd"));
    }

    #[test]
    fn a_query_value_decoded() {
        assert_eq!(decode("01a1-b2%3Ac"), "01a1-b2:c");
        assert_eq!(decode("a+b%E5%AD%97"), "a b字");
        assert_eq!(decode("50%"), "50%");
        assert_eq!(decode("%zz"), "%zz");
        assert_eq!(decode("%字"), "%字");
    }

    #[test]
    fn secrets_are_made_once() {
        let mut settings = serde_json::Map::new();
        assert!(fill_secrets(&mut settings));
        let token = settings["mobileToken"].as_str().unwrap().to_string();
        assert_eq!(token.len(), 32);
        assert!(is_topic(settings["ntfy"]["topic"].as_str().unwrap()));
        assert!(!fill_secrets(&mut settings));
        assert_eq!(settings["mobileToken"], json!(token));
        assert_ne!(new_token(), new_token());
    }

    #[test]
    fn ntfy_goes_to_its_server_and_topic() {
        assert_eq!(ntfy_target(&json!({ "topic": "wakuwaku-1" })).as_deref(), Some("https://ntfy.sh/wakuwaku-1"));
        assert_eq!(ntfy_target(&json!({ "server": "https://ntfy.example.com/", "topic": "t" })).as_deref(), Some("https://ntfy.example.com/t"));
        assert_eq!(ntfy_target(&json!({ "server": "ftp://x", "topic": "t" })), None);
        assert_eq!(ntfy_target(&json!({ "topic": "a/b" })), None);
        assert_eq!(ntfy_target(&json!({ "topic": "" })), None);
    }

    // Only the agent and how it is: no project, no command, no file.
    #[test]
    fn a_push_names_the_agent_and_the_mood_alone() {
        assert_eq!(push_words("zh-TW", "codex", "done"), "Codex 做完了");
        assert_eq!(push_words("en", "claude", "waiting"), "Claude Code is waiting for you");
        assert_eq!(push_words("zh", "grok", "error"), "Grok 出错了");
    }
}
