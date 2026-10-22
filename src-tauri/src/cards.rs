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

// The cards played from the hand on the other's turn to stop her ("手坑"),
// the ones met in Master Duel, by their English names: told from their
// text, a guess called 551 cards so (any "from the hand" with "the
// opponent"), most of them not.
const HAND_TRAPS: [&str; 25] = [
    "Ash Blossom & Joyous Spring",
    "Maxx \"C\"",
    "Effect Veiler",
    "Ghost Ogre & Snow Rabbit",
    "Ghost Belle & Haunted Mansion",
    "Ghost Mourner & Moonlit Chill",
    "Ghost Reaper & Winter Cherries",
    "Ghost Sister & Spooky Dogwood",
    "Nibiru, the Primal Being",
    "D.D. Crow",
    "PSY-Framegear Gamma",
    "Droll & Lock Bird",
    "Dimension Shifter",
    "Artifact Lancea",
    "Mulcharmy Fuwalos",
    "Mulcharmy Purulia",
    "Mulcharmy Meowls",
    "Retaliating \"C\"",
    "Skull Meister",
    "Gnomaterial",
    "Herald of Orange Light",
    "Infinite Impermanence",
    "Evenly Matched",
    "Dominus Impulse",
    "Dominus Purge",
];

// YGOPro's type bit of a monster.
const MONSTER: u32 = 0x1;

#[derive(Clone)]
pub struct Card {
    pub id: u64,
    pub name: String,
    pub jp: String,
    pub en: String,
    // Lowercased, for finding.
    key: String,
    // What the game sorts and filters by (YGOPro's numbers: `kind` its type
    // bits, `level` the level, rank or link rating with the pendulum scale
    // in the top byte, `race` and `attr` one bit each, `def` a link's
    // arrows), and its text, Traditional.
    pub kind: u32,
    pub atk: i32,
    pub def: i32,
    pub level: u32,
    pub race: u32,
    pub attr: u32,
    pub desc: String,
    // One of the hand traps (HAND_TRAPS).
    pub hand: bool,
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
            let n = |k: &str| v["data"][k].as_i64().unwrap_or(0);
            let kind = n("type") as u32;
            let text = &v["text"];
            let raw = [s(text, "pdesc"), s(text, "desc")].into_iter().filter(|t| !t.is_empty()).collect::<Vec<_>>().join("\n");
            let hand = HAND_TRAPS.iter().any(|h| h.eq_ignore_ascii_case(&en));
            let desc = traditional(&raw.replace("\r\n", "\n"), &map);
            Some(Card {
                id,
                name: if name.is_empty() { en.clone() } else { name },
                jp,
                en,
                key,
                kind,
                atk: n("atk") as i32,
                def: n("def") as i32,
                level: n("level") as u32,
                race: n("race") as u32,
                attr: n("attribute") as u32,
                desc,
                hand,
            })
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

// Every picture of a card (its alternate arts too: Master Duel has them,
// and its cards with art over the frame, the user's word, 2026-10-22), by
// its English name from YGOPRODeck, which lists them by name; asked once.
static ARTS: Mutex<Option<HashMap<u64, Vec<u64>>>> = Mutex::new(None);

pub fn arts(cards: &[Card], id: u64) -> Vec<String> {
    if let Some(hit) = ARTS.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(&id).cloned()) {
        return hit.into_iter().map(picture).collect();
    }
    let Some(en) = cards.iter().find(|c| c.id == id).map(|c| c.en.clone()).filter(|e| !e.is_empty()) else { return vec![picture(id)] };
    let url = format!("https://db.ygoprodeck.com/api/v7/cardinfo.php?name={}", en.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>());
    let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(10))).build().new_agent();
    let ids: Vec<u64> = agent
        .get(&url)
        .call()
        .ok()
        .and_then(|mut r| r.body_mut().read_json::<Value>().ok())
        .and_then(|v| v["data"][0]["card_images"].as_array().map(|a| a.iter().filter_map(|i| i["id"].as_u64()).collect()))
        .filter(|ids: &Vec<u64>| !ids.is_empty())
        .unwrap_or_else(|| vec![id]);
    ARTS.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(id, ids.clone());
    ids.into_iter().map(picture).collect()
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

// What the deck window shows of a card and sorts and filters it by: found
// by its passcode, or by its name when it was read without one.
pub fn info(cards: &[Card], id: Option<u64>, name: &str) -> Option<Value> {
    let name = name.trim().to_lowercase();
    let c = id
        .and_then(|id| cards.iter().find(|c| c.id == id))
        .or_else(|| (!name.is_empty()).then(|| cards.iter().find(|c| c.name.to_lowercase() == name || c.jp.to_lowercase() == name || c.en.to_lowercase() == name)).flatten())
        // Written a little otherwise ("I:P偽裝舞會蓮" for "I：P偽裝舞會蓮"): its spaces and dots aside.
        .or_else(|| {
            let bare = |t: &str| t.to_lowercase().chars().filter(|c| !c.is_whitespace() && !"・·.:：-－「」\"".contains(*c)).collect::<String>();
            let want = bare(&name);
            (want.chars().count() >= 3).then(|| cards.iter().find(|c| bare(&c.name) == want || bare(&c.jp) == want || bare(&c.en) == want)).flatten()
        })?;
    Some(info_of(c))
}

fn info_of(c: &Card) -> Value {
    json!({
        "id": c.id,
        "name": c.name,
        "jp": c.jp,
        "en": c.en,
        "kind": c.kind,
        "atk": c.atk,
        "def": c.def,
        "level": c.level & 0xff,
        "scale": (c.level >> 24) & 0xff,
        "race": c.race,
        "attr": c.attr,
        "desc": c.desc,
        "hand": c.hand,
        "img": picture(c.id),
    })
}

// The deck window's card list, as the game's: the cards whose name or text
// holds what is typed, through the filters (`f`: "type" monster | spell |
// trap | hand, and the bits "attr", "race", "kind", and "level"); nothing
// typed nor filtered, the cards of her deck, its theme and the hand traps.
// Ordered as the game lists them: monsters (main, then extra) by level high
// to low, spells, traps; then by name.
pub fn browse(cards: &[Card], q: &str, f: &Value, around: &[String], most: usize) -> Vec<Value> {
    let q = q.trim().to_lowercase();
    let n = |k: &str| f[k].as_u64().or_else(|| f[k].as_str()?.parse().ok()).unwrap_or(0) as u32;
    let (kind_of, attr, race, sub, level) = (f["type"].as_str().unwrap_or(""), n("attr"), n("race"), n("kind"), n("level"));
    let filtered = !kind_of.is_empty() || attr + race + sub + level > 0;
    let passes = |c: &Card| {
        let monster = c.kind & MONSTER != 0;
        (match kind_of {
            "monster" => monster,
            "spell" => c.kind & 0x2 != 0,
            "trap" => c.kind & 0x4 != 0,
            "hand" => c.hand,
            _ => true,
        }) && (attr == 0 || monster && c.attr == attr)
            && (race == 0 || monster && c.race == race)
            && (sub == 0 || c.kind & sub != 0)
            && (level == 0 || monster && c.level & 0xff == level)
    };
    let mut hits: Vec<&Card> = cards
        .iter()
        .filter(|c| {
            if !q.is_empty() {
                (c.key.contains(&q) || c.desc.to_lowercase().contains(&q)) && passes(c)
            } else if filtered {
                passes(c)
            } else {
                c.hand || around.iter().any(|a| *a == c.name)
            }
        })
        .collect();
    let rank = |c: &Card| -> (u8, i64) {
        let extra = c.kind & (0x40 | 0x2000 | 0x800000 | 0x4000000) != 0;
        let group = if c.kind & MONSTER != 0 { if extra { 1 } else { 0 } } else if c.kind & 0x2 != 0 { 2 } else { 3 };
        (group, -((c.level & 0xff) as i64))
    };
    // What is typed found in the name first, then in the text.
    hits.sort_by(|a, b| {
        let named = |c: &Card| !q.is_empty() && !c.key.contains(&q);
        named(a).cmp(&named(b)).then(rank(a).cmp(&rank(b))).then(a.name.cmp(&b.name))
    });
    hits.into_iter().take(most).map(info_of).collect()
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
                "1": { "id": 14558127, "md_name": "灰流丽", "jp_name": "灰流うらら", "en_name": "Ash Blossom & Joyous Spring",
                       "text": { "desc": "①：包含以下其中任意种效果的魔法·陷阱·怪兽的效果发动时，把这张卡从手卡丢弃才能发动。那个效果无效。" },
                       "data": { "type": 4129, "atk": 0, "def": 1800, "level": 3, "race": 16, "attribute": 4 } },
                "2": { "id": "8633261", "md_name": "冰燃・雷热涡炉", "jp_name": "アイス・ライゼオル", "en_name": "Ice Ryzeal" },
                "3": { "id": 35844557, "md_name": "剑极・雷热涡炉", "en_name": "Sword Ryzeal" },
                "4": { "id": 72238166, "md_name": "节极・雷热涡炉", "en_name": "Node Ryzeal" },
                "5": { "id": 19899073, "md_name": "天丛云之巳剑", "en_name": "Ame no Murakumo no Mitsurugi",
                       "text": { "types": "[怪兽|效果|仪式] 爬虫类/暗\n[★8] 3200/800", "desc": "「巳剑降临」降临\r\n①：这张卡特殊召唤的场合才能发动。" },
                       "data": { "type": 161, "atk": 3200, "def": 800, "level": 8, "race": 524288, "attribute": 32 } }
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
    fn what_the_game_sorts_a_card_by() {
        let cards = db();
        let ame = info(&cards, None, "天叢雲之巳劍").unwrap();
        assert_eq!((ame["atk"].as_i64(), ame["level"].as_u64(), ame["race"].as_u64(), ame["hand"].as_bool()), (Some(3200), Some(8), Some(0x80000), Some(false)));
        assert!(ame["desc"].as_str().unwrap().starts_with("「巳劍降臨」降臨\n①：這張卡"),"{}", ame["desc"]);
        let ash = info(&cards, Some(14558127), "").unwrap();
        assert_eq!((ash["kind"].as_u64(), ash["hand"].as_bool()), (Some(4129), Some(true)));
        assert!(info(&cards, None, "沒有這張").is_none());
        assert_eq!(info(&cards, None, "冰燃雷熱渦爐").unwrap()["id"], 8633261);
    }

    #[test]
    fn the_card_list_as_the_game_lists_it() {
        let cards = db();
        // Nothing typed: her deck's cards and the hand traps.
        let mine = browse(&cards, "", &json!({}), &["天叢雲之巳劍".to_string()], 50);
        let names: Vec<&str> = mine.iter().filter_map(|c| c["name"].as_str()).collect();
        assert_eq!(names, ["天叢雲之巳劍", "灰流麗"], "monsters by level high to low");
        // Typed: the name, then the text.
        assert_eq!(browse(&cards, "灰流", &json!({}), &[], 50)[0]["name"], "灰流麗");
        assert!(browse(&cards, "降臨", &json!({}), &[], 50).iter().any(|c| c["name"] == "天叢雲之巳劍"), "found in its text");
        // Filtered: level 3 monsters, hand traps.
        assert_eq!(browse(&cards, "", &json!({ "level": 3 }), &[], 50).len(), 1);
        assert_eq!(browse(&cards, "", &json!({ "type": "hand" }), &[], 50)[0]["name"], "灰流麗");
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
