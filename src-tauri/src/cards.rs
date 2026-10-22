// Every Yu-Gi-Oh! card, for her deck window's drop-down and the deck
// reading (duel.rs, deckscan.rs): ygocdb.com's database (cards.zip, some
// 14 000 cards: Master Duel's own Chinese names, the Japanese, the English,
// each card's passcode), fetched once and again weekly into data/cards/.
// Its Chinese is Simplified: made Traditional in Taiwan's way character by
// character (s2t.txt, OpenCC's s2twp for every character the names use),
// and whole for the 707 names where words read differently (cards_tw.txt,
// by passcode; both made 2026-10-22). Pictures by passcode from YGOPRODeck.
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde_json::{json, Value};

const URL: &str = "https://ygocdb.com/api/v0/cards.zip";
const REFRESH: Duration = Duration::from_secs(7 * 24 * 3600);
const S2T: &str = include_str!("s2t.txt");
const WHOLE: &str = include_str!("cards_tw.txt");

#[derive(Clone)]
pub struct Card {
    pub id: u64,
    pub name: String,
    pub jp: String,
    pub en: String,
    // Lowercased, for finding.
    key: String,
}

static CARDS: Mutex<Option<Vec<Card>>> = Mutex::new(None);
static FETCHING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn picture(id: u64) -> String {
    format!("https://images.ygoprodeck.com/images/cards_small/{id}.jpg")
}

fn traditional(simplified: &str, map: &HashMap<char, char>) -> String {
    simplified.chars().map(|c| *map.get(&c).unwrap_or(&c)).collect()
}

fn s2t() -> HashMap<char, char> {
    S2T.lines().filter_map(|l| {
        let mut c = l.chars();
        Some((c.next()?, c.next()?))
    }).collect()
}

// The database's cards.json, read into cards.
pub fn parse(json_text: &str) -> Vec<Card> {
    let Ok(Value::Object(all)) = serde_json::from_str::<Value>(json_text) else { return Vec::new() };
    let map = s2t();
    let whole: HashMap<u64, &str> = WHOLE.lines().filter_map(|l| l.split_once('\t')).filter_map(|(id, n)| Some((id.parse().ok()?, n))).collect();
    let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").trim().to_string();
    let mut out: Vec<Card> = all
        .values()
        .filter_map(|v| {
            let id = v.get("id").and_then(|i| i.as_u64().or_else(|| i.as_str()?.parse().ok()))?;
            let sc = [s(v, "md_name"), s(v, "cn_name"), s(v, "sc_name")].into_iter().find(|n| !n.is_empty()).unwrap_or_default();
            let name = whole.get(&id).map(|n| n.to_string()).unwrap_or_else(|| traditional(&sc, &map));
            let (jp, en) = (s(v, "jp_name"), s(v, "en_name"));
            if name.is_empty() && en.is_empty() {
                return None;
            }
            let key = format!("{}\n{}\n{}\n{}", name, sc, jp, en).to_lowercase();
            Some(Card { id, name: if name.is_empty() { en.clone() } else { name }, jp, en, key })
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

// The one file in a zip, deflated (cards.zip holds cards.json alone).
pub fn unzip_first(zip: &[u8]) -> Option<Vec<u8>> {
    if zip.len() < 30 || zip[..4] != [0x50, 0x4b, 0x03, 0x04] {
        return None;
    }
    let u16_at = |i: usize| u16::from_le_bytes([zip[i], zip[i + 1]]) as usize;
    let method = u16_at(8);
    let start = 30 + u16_at(26) + u16_at(28);
    let size = u32::from_le_bytes([zip[18], zip[19], zip[20], zip[21]]) as usize;
    // Its size given (no data descriptor after): that much alone.
    let data = if size > 0 { zip.get(start..start + size)? } else { zip.get(start..)? };
    match method {
        0 => Some(data.to_vec()),
        8 => miniz_oxide::inflate::decompress_to_vec(data).ok(),
        _ => None,
    }
}

fn dir(data: &Path) -> PathBuf {
    data.join("cards")
}

// The cards, read from her copy; fetched (on a thread of its own) when
// there is none or it is a week old. None until there is a copy.
pub fn with_cards<T>(data: &Path, f: impl FnOnce(&[Card]) -> T) -> Option<T> {
    let file = dir(data).join("cards.json");
    let old = std::fs::metadata(&file).and_then(|m| m.modified()).ok().is_none_or(|t| t.elapsed().unwrap_or_default() > REFRESH);
    if old && !FETCHING.swap(true, std::sync::atomic::Ordering::SeqCst) {
        let data = data.to_path_buf();
        std::thread::spawn(move || {
            let got = fetch(&data);
            if got.is_ok() {
                *CARDS.lock().unwrap_or_else(|e| e.into_inner()) = None;
            }
            FETCHING.store(false, std::sync::atomic::Ordering::SeqCst);
        });
    }
    let mut cards = CARDS.lock().unwrap_or_else(|e| e.into_inner());
    if cards.is_none() {
        let text = std::fs::read_to_string(&file).ok()?;
        let read = parse(&text);
        if read.is_empty() {
            return None;
        }
        *cards = Some(read);
    }
    cards.as_deref().map(f)
}

fn fetch(data: &Path) -> Result<(), String> {
    let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(120))).build().new_agent();
    let mut zip = Vec::new();
    agent.get(URL).header("user-agent", "wakuwaku").call().map_err(|e| e.to_string())?.body_mut().as_reader().read_to_end(&mut zip).map_err(|e| e.to_string())?;
    let json = unzip_first(&zip).ok_or("not a zip of cards")?;
    if parse(&String::from_utf8_lossy(&json)).len() < 1000 {
        return Err("too few cards".into());
    }
    std::fs::create_dir_all(dir(data)).map_err(|e| e.to_string())?;
    std::fs::write(dir(data).join("cards.json"), json).map_err(|e| e.to_string())
}

// Cards by what is typed: the name first, as it starts, then as it holds
// it; the Japanese or English too. The shortest names first among equals.
pub fn search(cards: &[Card], q: &str, most: usize) -> Vec<Value> {
    let q = q.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<(u8, usize, &Card)> = cards
        .iter()
        .filter_map(|c| {
            let name = c.name.to_lowercase();
            let rank = if name == q {
                0
            } else if name.starts_with(&q) {
                1
            } else if name.contains(&q) {
                2
            } else if c.key.contains(&q) {
                3
            } else {
                return None;
            };
            Some((rank, c.name.chars().count(), c))
        })
        .collect();
    hits.sort_by_key(|h| (h.0, h.1));
    hits.into_iter().take(most).map(|(_, _, c)| json!({ "id": c.id, "name": c.name, "jp": c.jp, "en": c.en, "img": picture(c.id) })).collect()
}

// A theme's cards: those whose names hold a word of it ("雷熱祀劍" holds
// 雷熱, which 12 cards' names hold; a word of two characters at least that
// three cards share, when the whole is no card's).
pub fn theme_cards(cards: &[Card], theme: &str) -> Vec<String> {
    let pieces: Vec<String> = theme.split(|c: char| c.is_whitespace() || "、,，+＋/／".contains(c)).map(|p| p.trim().to_lowercase()).filter(|p| p.chars().count() >= 2).collect();
    let count = |w: &str| cards.iter().filter(|c| c.key.contains(w)).count();
    let mut words = Vec::new();
    for p in pieces {
        if count(&p) >= 2 {
            words.push(p);
            continue;
        }
        let chars: Vec<char> = p.chars().collect();
        for len in (2..chars.len()).rev() {
            for at in 0..=chars.len() - len {
                let w: String = chars[at..at + len].iter().collect();
                if !words.iter().any(|x: &String| x.contains(&w)) && count(&w) >= 3 {
                    words.push(w);
                }
            }
        }
    }
    let mut out: Vec<String> = cards.iter().filter(|c| words.iter().any(|w| c.key.contains(w.as_str()))).map(|c| c.name.clone()).collect();
    out.sort();
    out.dedup();
    out.truncate(120);
    out
}

// A name as Claude wrote it (any language, a "?" after) as the database
// names the card: the same name, or the one card whose name holds it.
pub fn best_match(cards: &[Card], said: &str) -> Option<String> {
    let q = said.trim().trim_end_matches(['?', '？']).trim().to_lowercase();
    if q.chars().count() < 2 {
        return None;
    }
    if let Some(c) = cards.iter().find(|c| c.name.to_lowercase() == q || c.jp.to_lowercase() == q || c.en.to_lowercase() == q) {
        return Some(c.name.clone());
    }
    let holding: Vec<&Card> = cards.iter().filter(|c| c.name.to_lowercase().contains(&q) || c.en.to_lowercase().contains(&q)).collect();
    (holding.len() == 1).then(|| holding[0].name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Vec<Card> {
        parse(
            &json!({
                "1": { "id": 14558127, "md_name": "灰流丽", "jp_name": "灰流うらら", "en_name": "Ash Blossom & Joyous Spring" },
                "2": { "id": "8633261", "md_name": "冰燃・雷热涡炉", "jp_name": "アイス・ライゼオル", "en_name": "Ice Ryzeal" },
                "3": { "id": 35844557, "md_name": "剑极・雷热涡炉", "en_name": "Sword Ryzeal" },
                "4": { "id": 72238166, "md_name": "节极・雷热涡炉", "en_name": "Node Ryzeal" },
                "5": { "id": 19899073, "md_name": "天丛云之巳剑", "en_name": "Ame no Murakumo no Mitsurugi" }
            })
            .to_string(),
        )
    }

    #[test]
    fn traditional_names_found_by_what_is_typed() {
        let cards = db();
        assert!(cards.iter().any(|c| c.name == "灰流麗"));
        assert!(cards.iter().any(|c| c.name == "天叢雲之巳劍"));
        let found = search(&cards, "灰流", 5);
        assert_eq!(found[0]["name"], "灰流麗");
        assert_eq!(found[0]["img"], "https://images.ygoprodeck.com/images/cards_small/14558127.jpg");
        assert_eq!(search(&cards, "ryzeal", 5).len(), 3);
        assert!(search(&cards, "  ", 5).is_empty());
    }

    #[test]
    fn a_theme_s_cards_and_a_name_put_right() {
        let cards = db();
        let ryzeal = theme_cards(&cards, "雷熱祀劍");
        assert_eq!(ryzeal.len(), 3, "{ryzeal:?}");
        assert!(ryzeal.contains(&"冰燃・雷熱渦爐".to_string()));
        assert_eq!(best_match(&cards, "Ice Ryzeal"), Some("冰燃・雷熱渦爐".into()));
        assert_eq!(best_match(&cards, "灰流麗？"), Some("灰流麗".into()));
        assert_eq!(best_match(&cards, "雷熱渦爐"), None);
    }

    #[test]
    fn the_real_database_when_it_is_here() {
        let Ok(zip) = std::fs::read(std::env::var("WAKUWAKU_CARDS_ZIP").unwrap_or_default()) else { return };
        let cards = parse(&String::from_utf8_lossy(&unzip_first(&zip).unwrap()));
        assert!(cards.len() > 13_000);
        assert!(theme_cards(&cards, "雷熱祀劍").iter().any(|n| n.contains("雷熱渦爐")));
        assert_eq!(best_match(&cards, "Ash Blossom & Joyous Spring").as_deref(), Some("灰流麗"));
    }

    #[test]
    fn the_one_file_of_a_zip() {
        // A stored (not deflated) zip of "x.json" holding "{}".
        let mut z = vec![0x50, 0x4b, 0x03, 0x04, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 2, 0, 0, 0, 6, 0, 0, 0];
        z.extend_from_slice(b"x.json{}");
        assert_eq!(unzip_first(&z), Some(b"{}".to_vec()));
    }
}
