// A card of her deck told by its picture (duel.rs's deck reading): each
// slot of the deck editor beside the official pictures (YGOPRODeck's,
// their alternate arts too), by the same look deckscan.rs keeps of a card
// (its art in 12x12 grey, alike as a correlation). Measured on her 55
// cards (2026-10-23): every slot whose best was 0.55 or more, and 0.15
// ahead of the next card, was that card (46 of them); the rest (Master
// Duel's own alternate arts, its shine across the art) were all under
// 0.47 or close behind another, and go to Claude. Claude's own answers
// are checked so too. The pictures are kept in data/cards/art/, each card's
// list of arts in data/cards/arts.json.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde_json::Value;

use crate::deckscan::{alike, print, Image};

pub const SURE: f32 = 0.55;
pub const AHEAD: f32 = 0.15;
// Fetched at once.
const AT_ONCE: usize = 6;

fn dir(data: &Path) -> PathBuf {
    data.join("cards").join("art")
}

fn ids_path(data: &Path) -> PathBuf {
    data.join("cards").join("arts.json")
}

// Each card's arts (its passcode → the pictures' ids), and each picture's look.
static IDS: Mutex<Option<HashMap<u64, Vec<u64>>>> = Mutex::new(None);
static LOOKS: Mutex<Option<HashMap<u64, Vec<f32>>>> = Mutex::new(None);

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(12))).build().new_agent()
}

fn known_ids(data: &Path) -> HashMap<u64, Vec<u64>> {
    let mut ids = IDS.lock().unwrap_or_else(|e| e.into_inner());
    ids.get_or_insert_with(|| {
        std::fs::read_to_string(ids_path(data))
            .ok()
            .and_then(|t| serde_json::from_str::<HashMap<String, Vec<u64>>>(&t).ok())
            .map(|m| m.into_iter().filter_map(|(k, v)| Some((k.parse().ok()?, v))).collect())
            .unwrap_or_default()
    })
    .clone()
}

// A card's pictures, by its English name from YGOPRODeck (which lists its
// arts by name), asked once and kept.
pub fn ids(data: &Path, id: u64, en: &str) -> Vec<u64> {
    if let Some(hit) = known_ids(data).get(&id) {
        return hit.clone();
    }
    if en.is_empty() {
        return vec![id];
    }
    let url = format!("https://db.ygoprodeck.com/api/v7/cardinfo.php?name={}", en.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>());
    let Some(found) = agent()
        .get(&url)
        .call()
        .ok()
        .and_then(|mut r| r.body_mut().read_json::<Value>().ok())
        .and_then(|v| v["data"][0]["card_images"].as_array().map(|a| a.iter().filter_map(|i| i["id"].as_u64()).collect::<Vec<_>>()))
        .filter(|ids| !ids.is_empty())
    else {
        // Not kept: asked again another time.
        return vec![id];
    };
    let all = {
        let mut ids = IDS.lock().unwrap_or_else(|e| e.into_inner());
        let ids = ids.get_or_insert_with(HashMap::new);
        ids.insert(id, found.clone());
        ids.iter().map(|(k, v)| (k.to_string(), v.clone())).collect::<HashMap<_, _>>()
    };
    let _ = std::fs::create_dir_all(data.join("cards"));
    let _ = std::fs::write(ids_path(data), serde_json::to_string(&all).unwrap_or_default());
    found
}

// One picture's look: kept, or read from its file, or fetched.
fn look(data: &Path, picture: u64) -> Option<Vec<f32>> {
    if let Some(hit) = LOOKS.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(&picture).cloned()) {
        return Some(hit);
    }
    let file = dir(data).join(format!("{picture}.jpg"));
    let bytes = match std::fs::read(&file) {
        Ok(b) if b.len() > 1000 => b,
        _ => {
            let mut res = agent().get(&crate::cards::picture(picture)).call().ok()?;
            let b = res.body_mut().read_to_vec().ok()?;
            let _ = std::fs::create_dir_all(dir(data));
            let _ = std::fs::write(&file, &b);
            b
        }
    };
    let rgb = image::load_from_memory_with_format(&bytes, image::ImageFormat::Jpeg).ok()?.to_rgb8();
    let (w, h) = (rgb.width() as usize, rgb.height() as usize);
    let l = print(&Image { w, h, rgb: rgb.into_raw() });
    LOOKS.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(picture, l.clone());
    Some(l)
}

// The looks of these cards (passcode, English name), every art of each,
// fetched a few at once; a card none of whose pictures came is left out.
pub fn looks(data: &Path, cards: &[(u64, String)]) -> HashMap<u64, Vec<Vec<f32>>> {
    let mut want: Vec<(u64, String)> = cards.to_vec();
    want.sort();
    want.dedup_by_key(|c| c.0);
    let chunk = want.len().div_ceil(AT_ONCE).max(1);
    std::thread::scope(|s| {
        let jobs: Vec<_> = want
            .chunks(chunk)
            .map(|part| s.spawn(move || part.iter().map(|(id, en)| (*id, ids(data, *id, en).into_iter().filter_map(|p| look(data, p)).collect::<Vec<_>>())).collect::<Vec<_>>()))
            .collect();
        jobs.into_iter().flat_map(|j| j.join().unwrap_or_default()).filter(|(_, l)| !l.is_empty()).collect()
    })
}

// How like a card a slot is: its closest art.
pub fn score(slot: &[f32], arts: &[Vec<f32>]) -> f32 {
    arts.iter().map(|a| alike(slot, a)).fold(f32::MIN, f32::max)
}

// The card a slot is, when its picture says so plainly: the best of the
// pool, SURE alike and AHEAD of the next card.
pub fn which(slot: &[f32], pool: &HashMap<u64, Vec<Vec<f32>>>) -> Option<(u64, f32)> {
    let mut scores: Vec<(f32, u64)> = pool.iter().map(|(id, arts)| (score(slot, arts), *id)).collect();
    scores.sort_by(|a, b| b.0.total_cmp(&a.0));
    let (best, id) = *scores.first()?;
    let next = scores.get(1).map_or(f32::MIN, |s| s.0);
    (best >= SURE && best - next >= AHEAD).then_some((id, best))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slot_told_only_when_plain() {
        let a = vec![1.0, -1.0, 1.0, -1.0];
        let b = vec![1.0, 1.0, -1.0, -1.0];
        let c = vec![1.0, -1.0, -1.0, 1.0];
        let pool: HashMap<u64, Vec<Vec<f32>>> = [(1, vec![a.clone()]), (2, vec![b.clone(), a.clone()]), (3, vec![c.clone()])].into_iter().collect();
        // Like 1 and like one of 2's arts alike: not plain.
        assert_eq!(which(&a, &pool), None);
        let pool: HashMap<u64, Vec<Vec<f32>>> = [(1, vec![a.clone()]), (2, vec![b.clone()]), (3, vec![c.clone()])].into_iter().collect();
        assert_eq!(which(&a, &pool), Some((1, 1.0)));
        // An alternate art counts as the card.
        let pool: HashMap<u64, Vec<Vec<f32>>> = [(2, vec![b.clone(), c.clone()]), (1, vec![a.clone()])].into_iter().collect();
        assert_eq!(which(&c, &pool).map(|w| w.0), Some(2));
        assert_eq!(which(&a, &HashMap::new()), None);
        // Half alike to everything: no.
        let half = vec![0.5, -0.5, 0.5, -0.5];
        let pool: HashMap<u64, Vec<Vec<f32>>> = [(1, vec![a.clone()])].into_iter().collect();
        assert_eq!(which(&half, &pool), None);
    }
}
