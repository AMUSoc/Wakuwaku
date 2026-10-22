// Google Maps for her small words about getting about (the user's asks,
// 2026-10-22): how long home or to work takes now, by the person's way
// (mapsMode: drive, two_wheeler, transit), against the usual; the weather
// where the phone is (Open-Meteo); and when to leave for the next event in
// the phone's calendar that has a place. The phone tells where it is and
// its next event with its state asks (mobile.rs); what she works out goes
// back with them, and onto the island (a widget, a nudge to leave).
//
// Google's Routes API (computeRoutes, traffic aware) and Geocoding API,
// with the person's own key (mapsKey: kept in the settings alone, never
// logged). Asked at most every 10 minutes for a way, an address once.
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use serde_json::{json, Value};

use crate::{now_ms, Shared};

const ROUTE_EVERY_MS: u64 = 10 * 60_000;
const WEATHER_EVERY_MS: u64 = 30 * 60_000;
// The phone's place this old is not where it is.
const PLACE_FRESH_MS: u64 = 20 * 60_000;
// Near enough to be there (home, work): no way to go.
const THERE_M: f64 = 600.0;
// Spare minutes before an event: the word to leave comes this early.
const SPARE_MIN: u64 = 10;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct At {
    pub lat: f64,
    pub lng: f64,
}

#[derive(Default)]
struct Kept {
    // Where the phone is, and when it said so.
    phone: Option<(At, u64)>,
    // Its next event with a place: title, place, when it begins (ms).
    event: Option<(String, String, u64)>,
    // Addresses found once.
    found: HashMap<String, Option<At>>,
    // Ways asked lately: (from, to, mode) rounded, its minutes now and usually, when.
    ways: HashMap<String, (u64, u64, u64)>,
    weather: Option<(At, String, u64, u64)>,
    // Events already told to leave for (their begin and title).
    left_for: Vec<String>,
}

static KEPT: Mutex<Option<Kept>> = Mutex::new(None);

fn with<T>(f: impl FnOnce(&mut Kept) -> T) -> T {
    let mut k = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    f(k.get_or_insert_with(Kept::default))
}

fn key(sh: &Shared) -> Option<String> {
    sh.setting("mapsKey").as_str().map(str::trim).filter(|k| k.starts_with("AIza") && k.len() < 80).map(str::to_string)
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(10))).build().new_agent()
}

// Metres between two places (haversine).
pub fn metres(a: At, b: At) -> f64 {
    let r = 6_371_000.0_f64;
    let (p1, p2) = (a.lat.to_radians(), b.lat.to_radians());
    let dp = (b.lat - a.lat).to_radians();
    let dl = (b.lng - a.lng).to_radians();
    let h = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * r * h.sqrt().asin()
}

// An address as a place (Geocoding API), found once.
fn place(sh: &Shared, address: &str) -> Option<At> {
    let address = address.trim();
    if address.is_empty() {
        return None;
    }
    if let Some(hit) = with(|k| k.found.get(address).copied()) {
        return hit;
    }
    let key = key(sh)?;
    let url = format!("https://maps.googleapis.com/maps/api/geocode/json?address={}&language=zh-TW&region=tw&key={key}", encode(address));
    let got = agent().get(&url).call().ok().and_then(|mut r| r.body_mut().read_json::<Value>().ok()).and_then(|v| {
        let l = &v["results"][0]["geometry"]["location"];
        Some(At { lat: l["lat"].as_f64()?, lng: l["lng"].as_f64()? })
    });
    with(|k| k.found.insert(address.to_string(), got));
    got
}

fn encode(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

pub fn mode_of(sh: &Shared) -> &'static str {
    match sh.setting("mapsMode").as_str() {
        Some("two_wheeler") => "TWO_WHEELER",
        Some("transit") => "TRANSIT",
        _ => "DRIVE",
    }
}

// Minutes from one place to another now, and as it usually is (Routes API).
fn way(sh: &Shared, from: At, to: At, mode: &str) -> Option<(u64, u64)> {
    let id = format!("{:.3},{:.3}>{:.3},{:.3}:{mode}", from.lat, from.lng, to.lat, to.lng);
    let now = now_ms();
    if let Some((m, usual, at)) = with(|k| k.ways.get(&id).copied()) {
        if now.saturating_sub(at) < ROUTE_EVERY_MS {
            return Some((m, usual));
        }
    }
    let key = key(sh)?;
    let mut body = json!({
        "origin": { "location": { "latLng": { "latitude": from.lat, "longitude": from.lng } } },
        "destination": { "location": { "latLng": { "latitude": to.lat, "longitude": to.lng } } },
        "travelMode": mode,
    });
    if mode != "TRANSIT" {
        body["routingPreference"] = json!("TRAFFIC_AWARE");
    }
    let got: Value = agent()
        .post("https://routes.googleapis.com/directions/v2:computeRoutes")
        .header("X-Goog-Api-Key", &key)
        .header("X-Goog-FieldMask", "routes.duration,routes.staticDuration")
        .send_json(&body)
        .ok()?
        .body_mut()
        .read_json()
        .ok()?;
    let secs = |s: &Value| s.as_str().and_then(|s| s.trim_end_matches('s').parse::<u64>().ok());
    let r = &got["routes"][0];
    let m = secs(&r["duration"])?.div_ceil(60);
    let usual = secs(&r["staticDuration"]).map_or(m, |s| s.div_ceil(60));
    with(|k| k.ways.insert(id, (m, usual, now)));
    Some((m, usual))
}

// The phone's word of where it is ("25.03,121.43") and of its next event
// with a place ({ title, place, begin }), with its state ask.
pub fn from_phone(at: Option<&str>, event: Option<&Value>) {
    let now = now_ms();
    if let Some((lat, lng)) = at.and_then(|s| s.split_once(',')).and_then(|(a, b)| Some((a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?))) {
        if (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lng) {
            with(|k| k.phone = Some((At { lat, lng }, now)));
        }
    }
    if let Some(e) = event {
        let begin = e["begin"].as_u64().unwrap_or(0);
        let place = e["place"].as_str().unwrap_or("").trim().to_string();
        with(|k| k.event = (begin > now && !place.is_empty()).then(|| (e["title"].as_str().unwrap_or("").chars().take(60).collect(), place.chars().take(200).collect(), begin)));
    }
}

fn phone_at() -> Option<At> {
    with(|k| k.phone.filter(|(_, at)| now_ms().saturating_sub(*at) < PLACE_FRESH_MS).map(|(p, _)| p))
}

// "回家 32 分（平常 22 分）": home from where the phone is, or to work from
// home in a weekday morning there; none when it is there already.
pub fn commute(sh: &Shared) -> Option<String> {
    let home = place(sh, sh.setting("mapsHome").as_str().unwrap_or(""));
    let work = place(sh, sh.setting("mapsWork").as_str().unwrap_or(""));
    let here = phone_at().or(home)?;
    let mode = mode_of(sh);
    let (label, to) = match (home, work) {
        (Some(h), Some(w)) if metres(here, h) < THERE_M => ("上班", w),
        (Some(h), _) if metres(here, h) >= THERE_M => ("回家", h),
        (None, Some(w)) if metres(here, w) >= THERE_M => ("上班", w),
        _ => return None,
    };
    if metres(here, to) < THERE_M {
        return None;
    }
    let (m, usual) = way(sh, here, to, mode)?;
    Some(words(label, m, usual))
}

pub fn words(label: &str, m: u64, usual: u64) -> String {
    if m > usual + 4 {
        format!("{label} {m} 分（比平常多 {} 分）", m - usual)
    } else {
        format!("{label} {m} 分")
    }
}

// The weather where the phone is (Open-Meteo, no key): "新莊 23° 小雨 · 降雨 70%".
pub fn weather(_sh: &Shared) -> Option<(String, u64)> {
    let here = phone_at()?;
    let now = now_ms();
    if let Some((at, words, rain, when)) = with(|k| k.weather.clone()) {
        if metres(at, here) < 3000.0 && now.saturating_sub(when) < WEATHER_EVERY_MS {
            return Some((words, rain));
        }
    }
    let url = format!("https://api.open-meteo.com/v1/forecast?latitude={:.3}&longitude={:.3}&current=temperature_2m,weather_code&hourly=precipitation_probability&forecast_hours=6&timezone=auto", here.lat, here.lng);
    let v: Value = agent().get(&url).call().ok()?.body_mut().read_json().ok()?;
    let temp = v["current"]["temperature_2m"].as_f64()?;
    let sky = sky(v["current"]["weather_code"].as_u64().unwrap_or(999));
    let rain = v["hourly"]["precipitation_probability"].as_array().map(|a| a.iter().filter_map(Value::as_u64).max().unwrap_or(0)).unwrap_or(0);
    let words = format!("{}° {sky} · 6 小時內降雨 {rain}%", temp.round() as i64);
    with(|k| k.weather = Some((here, words.clone(), rain, now)));
    Some((words, rain))
}

fn sky(code: u64) -> &'static str {
    match code {
        0 => "晴",
        1 => "大致晴朗",
        2 => "多雲",
        3 => "陰",
        45 | 48 => "霧",
        51..=57 => "毛毛雨",
        61 | 80 => "小雨",
        63 | 81 => "雨",
        65 | 82 => "大雨",
        95..=99 => "雷雨",
        _ => "",
    }
}

// The next event to leave for: when (minutes from now) the way there takes
// all the time left but SPARE_MIN; told once (its id), and then until it begins.
pub fn leave(sh: &Shared) -> Option<Value> {
    let (title, at_place, begin) = with(|k| k.event.clone())?;
    let now = now_ms();
    if begin <= now {
        return None;
    }
    let here = phone_at().or_else(|| place(sh, sh.setting("mapsHome").as_str().unwrap_or("")))?;
    let to = place(sh, &at_place)?;
    if metres(here, to) < THERE_M {
        return None;
    }
    let (m, _) = way(sh, here, to, mode_of(sh))?;
    let left_min = (begin - now) / 60_000;
    let go_in = left_min.saturating_sub(m + SPARE_MIN);
    let id = format!("{begin}:{title}");
    let due = go_in == 0;
    let first = due && with(|k| {
        if k.left_for.contains(&id) {
            false
        } else {
            k.left_for.push(id.clone());
            if k.left_for.len() > 20 {
                k.left_for.remove(0);
            }
            true
        }
    });
    if first {
        sh.to_pages("pet:nudge", json!({ "id": "maps", "icon": "flag", "words": format!("該出發了：{title}"), "label": format!("{at_place} · 路上 {m} 分"), "value": "", "private": true, "open": null }));
    }
    Some(json!({ "id": id, "title": title, "place": at_place, "minutes": m, "goIn": go_in, "due": due }))
}

// What was worked out last for the phone, and when; one working it out now.
static LAST: Mutex<(Value, u64)> = Mutex::new((Value::Null, 0));
static WORKING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
const FRESH_MS: u64 = 60_000;

// What goes back to the phone with its state: the last worked out, at once
// (the web is asked for it on a thread of its own, not while the phone waits).
pub fn for_phone(sh: &Shared) -> Value {
    if key(sh).is_none() && phone_at().is_none() {
        return Value::Null;
    }
    let (last, at) = LAST.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if now_ms().saturating_sub(at) >= FRESH_MS && !WORKING.swap(true, std::sync::atomic::Ordering::SeqCst) {
        let sh = crate::shared(&sh.app);
        crate::spawn_named("maps", move || {
            let (weather, rain) = weather(&sh).map_or((None, 0), |(w, r)| (Some(w), r));
            let has_key = key(&sh).is_some();
            let fresh = json!({ "commute": has_key.then(|| commute(&sh)).flatten(), "weather": weather, "rain": rain, "leave": has_key.then(|| leave(&sh)).flatten() });
            // On the island too, while it is known (a widget that goes by itself).
            if let Some(words) = fresh["commute"].as_str() {
                let put = sh.widgets.lock().unwrap().put(&json!({ "id": "commute", "label": words, "icon": "flag", "color": "#34d27b", "ttl": 900 }), now_ms()).is_ok();
                if put {
                    sh.redraw_widgets();
                }
            }
            *LAST.lock().unwrap_or_else(|e| e.into_inner()) = (fresh, now_ms());
            WORKING.store(false, std::sync::atomic::Ordering::SeqCst);
        });
    }
    last
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances_and_words() {
        let xinzhuang = At { lat: 25.036, lng: 121.432 };
        let taipei101 = At { lat: 25.0332, lng: 121.5649 };
        let d = metres(xinzhuang, taipei101);
        assert!((13_000.0..14_000.0).contains(&d), "{d}");
        assert_eq!(words("回家", 32, 22), "回家 32 分（比平常多 10 分）");
        assert_eq!(words("上班", 25, 24), "上班 25 分");
        assert_eq!(encode("台北 101"), "%E5%8F%B0%E5%8C%97%20101");
    }
}
