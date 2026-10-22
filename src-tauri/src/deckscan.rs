// Her own deck read off Master Duel's deck editor, card by card (duel.rs):
// each slot of the editor's grid cut out of the game's picture (main deck
// ten a row, up to five rows; the extra deck two), the empty ones left out,
// each kept as a little picture for her deck window (duel/slots/), and all
// laid larger, numbered, on one sheet Claude reads. The whole screen at
// once, each card some 60 px wide, was mostly guessed (more than half
// unsure, 254 s, 2026-10-22). With a theme from her ("雷熱祀劍"), Claude
// first looks up which cards it has, and picks among those; the cards she
// has marked right are kept as they are.
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

// The editor's grid, as parts of the game's inside (16:9), measured on a
// 2048 x 1152 picture of it: a card's left, top, width and height, the step
// to the next one across and down.
const X0: f64 = 548.0 / 2048.0;
const STEP_X: f64 = 81.3 / 2048.0;
const W: f64 = 68.0 / 2048.0;
const H: f64 = 99.0 / 1152.0;
const STEP_Y: f64 = 112.0 / 1152.0;
const MAIN_Y0: f64 = 237.0 / 1152.0;
const EXTRA_Y0: f64 = 846.0 / 1152.0;
const MAIN_ROWS: usize = 5;
const EXTRA_ROWS: usize = 2;
// On the sheet: each card this many times as large, ten a row.
const SCALE: f64 = 2.2;
const SHEET_COLS: usize = 10;
const LABEL_H: usize = 26;

pub struct Image {
    pub w: usize,
    pub h: usize,
    pub rgb: Vec<u8>,
}

impl Image {
    fn crop(&self, x: usize, y: usize, w: usize, h: usize) -> Image {
        let mut rgb = Vec::with_capacity(w * h * 3);
        for row in y..(y + h).min(self.h) {
            let from = (row * self.w + x) * 3;
            rgb.extend_from_slice(&self.rgb[from..from + w.min(self.w - x) * 3]);
        }
        Image { w: w.min(self.w - x), h: h.min(self.h - y), rgb }
    }

    // How bright it is, and how much it varies: an empty slot is dark and
    // nearly flat (its outline alone), a card bright and busy (measured: empty
    // at most 46 bright, 27 varied; cards at least 100 and 50).
    fn light(&self) -> (f64, f64) {
        let n = self.rgb.len().max(1) as f64;
        let mean = self.rgb.iter().map(|&v| v as f64).sum::<f64>() / n;
        (mean, (self.rgb.iter().map(|&v| (v as f64 - mean).powi(2)).sum::<f64>() / n).sqrt())
    }

    fn scaled(&self, k: f64) -> Image {
        let (w, h) = (((self.w as f64) * k) as usize, ((self.h as f64) * k) as usize);
        let mut rgb = vec![0u8; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let (sx, sy) = (((x as f64) / k) as usize, ((y as f64) / k) as usize);
                let from = (sy.min(self.h - 1) * self.w + sx.min(self.w - 1)) * 3;
                rgb[(y * w + x) * 3..(y * w + x) * 3 + 3].copy_from_slice(&self.rgb[from..from + 3]);
            }
        }
        Image { w, h, rgb }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), self.w as u32, self.h as u32);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().map_err(|e| e.to_string())?.write_image_data(&self.rgb).map_err(|e| e.to_string())
    }

    pub fn load(path: &Path) -> Option<Image> {
        let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).ok()?));
        let mut reader = decoder.read_info().ok()?;
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).ok()?;
        let (w, h) = (info.width as usize, info.height as usize);
        let rgb = match info.color_type {
            png::ColorType::Rgb => buf[..w * h * 3].to_vec(),
            png::ColorType::Rgba => buf[..w * h * 4].chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect(),
            _ => return None,
        };
        Some(Image { w, h, rgb })
    }
}

// The cards on the editor: (slot id "m0".. / "e0".., its picture).
pub fn slots(screen: &Image) -> Vec<(String, Image)> {
    let (sw, sh) = (screen.w as f64, screen.h as f64);
    let mut out = Vec::new();
    for (part, y0, rows) in [("m", MAIN_Y0, MAIN_ROWS), ("e", EXTRA_Y0, EXTRA_ROWS)] {
        let mut n = 0;
        for r in 0..rows {
            for c in 0..10 {
                let x = ((X0 + c as f64 * STEP_X) * sw).round() as usize;
                let y = ((y0 + r as f64 * STEP_Y) * sh).round() as usize;
                let (w, h) = ((W * sw).round() as usize, (H * sh).round() as usize);
                if x + w > screen.w || y + h > screen.h {
                    continue;
                }
                let card = screen.crop(x, y, w, h);
                let (mean, spread) = card.light();
                if mean < 70.0 || spread < 38.0 {
                    continue;
                }
                out.push((format!("{part}{n}"), card));
                n += 1;
            }
        }
    }
    out
}

// 3 x 5 digits, a row of bits each.
const DIGITS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111],
    [0b010, 0b110, 0b010, 0b010, 0b111],
    [0b111, 0b001, 0b111, 0b100, 0b111],
    [0b111, 0b001, 0b111, 0b001, 0b111],
    [0b101, 0b101, 0b111, 0b001, 0b001],
    [0b111, 0b100, 0b111, 0b001, 0b111],
    [0b111, 0b100, 0b111, 0b101, 0b111],
    [0b111, 0b001, 0b010, 0b010, 0b010],
    [0b111, 0b101, 0b111, 0b101, 0b111],
    [0b111, 0b101, 0b111, 0b001, 0b111],
];

fn put_number(sheet: &mut Image, x: usize, y: usize, n: usize) {
    let px = 4;
    for (i, ch) in n.to_string().chars().enumerate() {
        let d = DIGITS[ch.to_digit(10).unwrap_or(0) as usize];
        for (row, bits) in d.iter().enumerate() {
            for col in 0..3 {
                if bits >> (2 - col) & 1 == 1 {
                    for dy in 0..px {
                        for dx in 0..px {
                            let (xx, yy) = (x + i * 4 * px + col * px + dx, y + row * px + dy);
                            if xx < sheet.w && yy < sheet.h {
                                let at = (yy * sheet.w + xx) * 3;
                                sheet.rgb[at..at + 3].copy_from_slice(&[255, 220, 80]);
                            }
                        }
                    }
                }
            }
        }
    }
}

// All the cards larger, ten a row, each numbered from 1 above it (the
// numbers Claude answers by).
pub fn sheet(cards: &[(String, Image)]) -> Option<Image> {
    let first = &cards.first()?.1;
    let cell = first.scaled(SCALE);
    let (cw, ch) = (cell.w + 8, cell.h + LABEL_H);
    let rows = cards.len().div_ceil(SHEET_COLS);
    let (w, h) = (cw * SHEET_COLS.min(cards.len()), ch * rows);
    let mut out = Image { w, h, rgb: vec![18; w * h * 3] };
    for (i, (_, card)) in cards.iter().enumerate() {
        let big = card.scaled(SCALE);
        let (ox, oy) = ((i % SHEET_COLS) * cw, (i / SHEET_COLS) * ch);
        put_number(&mut out, ox + 4, oy + 3, i + 1);
        for y in 0..big.h.min(cell.h) {
            let from = y * big.w * 3;
            let to = ((oy + LABEL_H + y) * w + ox) * 3;
            let n = big.w.min(cell.w) * 3;
            out.rgb[to..to + n].copy_from_slice(&big.rgb[from..from + n]);
        }
    }
    Some(out)
}

pub fn prompt_of(main: usize, extra: usize, theme: &str, theme_cards: &[String], kept: &[(usize, String)]) -> String {
    let mut p = format!(
        "用 Read 讀 sheet.png：這是遊戲王 Master Duel 牌組編輯畫面裡一張一張切出來放大的卡，每張上面有黃色編號。\
         1～{main} 是主要牌組，{}～{} 是額外牌組（有的話）。同一張卡放了幾張就會出現幾次。\n",
        main + 1,
        main + extra
    );
    if !theme_cards.is_empty() {
        p.push_str(&format!(
            "\n我說這副牌是「{}」。卡片資料庫裡這個主題的卡有：{}。主題卡從這份清單挑，名字照清單寫；不用上網查。其他的卡（手坑、泛用卡）照你認得的寫。\n",
            theme.trim(),
            theme_cards.join("、")
        ));
    } else if !theme.trim().is_empty() {
        p.push_str(&format!(
            "\n我說這副牌是「{}」。先用 WebSearch 查這個主題的卡表（最多查 2 次，不要打開網頁細讀），再拿每張的卡圖去比對：主題卡優先從那份清單裡挑。卡名一律寫台灣繁體中文譯名。\n",
            theme.trim()
        ));
    }
    if !kept.is_empty() {
        p.push_str("\n這幾張我已經確認過，不用再判斷：");
        p.push_str(&kept.iter().map(|(n, name)| format!("{n}={name}")).collect::<Vec<_>>().join("、"));
        p.push('\n');
    }
    p.push_str(
        "\nMaster Duel 有異圖（同一張卡不同的卡圖）、超框（卡圖蓋過邊框）和閃卡特效：卡圖可能跟一般版本不一樣，別只看整張圖，要看卡圖的主角、卡片種類的顏色（怪獸的橘、效果、儀式藍、融合紫、同步白、超量黑、連結藍，魔法綠、陷阱紅）和稀有度標記。\n\
         卡片最上面有一條很小的卡名，看得清楚時以它為準。卡名用台灣 Master Duel 繁體中文官方譯名（不確定就寫日文或英文名）。\n\
         只回一個 JSON：{\"theme\":\"牌組主題\",\"cards\":[{\"n\":編號,\"name\":\"卡名\",\"sure\":true}]}\n\
         每個編號都要有一筆；沒把握的 sure 寫 false，名字寫最可能的猜測。\n",
    );
    p
}

// --- A card's look, to tell copies apart and know it again ---------------------------------

// Its art (the card's middle, past its name and rarity at the top and
// above its text) in 12x12 grey, each the average of its square, less
// their mean and over their spread: Master Duel's shine makes copies of a
// card brighter or darker, not otherwise. Measured on her 55 cards
// (2026-10-22): copies alike 0.955 and more, other cards 0.57 at most (one
// copy with its shine moving, 0.74 to the others, is left to her).
pub const ALIKE: f32 = 0.85;
const PRINT: usize = 12;

pub fn print(img: &Image) -> Vec<f32> {
    let (x0, x1) = ((img.w as f32 * 0.12) as usize, (img.w as f32 * 0.88) as usize);
    let (y0, y1) = ((img.h as f32 * 0.18) as usize, (img.h as f32 * 0.66) as usize);
    let mut cells = vec![0f32; PRINT * PRINT];
    let mut counts = vec![0f32; PRINT * PRINT];
    for y in y0..y1.min(img.h) {
        for x in x0..x1.min(img.w) {
            let cell = ((y - y0) * PRINT / (y1 - y0).max(1)).min(PRINT - 1) * PRINT + ((x - x0) * PRINT / (x1 - x0).max(1)).min(PRINT - 1);
            let i = (y * img.w + x) * 3;
            cells[cell] += 0.299 * img.rgb[i] as f32 + 0.587 * img.rgb[i + 1] as f32 + 0.114 * img.rgb[i + 2] as f32;
            counts[cell] += 1.0;
        }
    }
    for (v, n) in cells.iter_mut().zip(&counts) {
        *v /= n.max(1.0);
    }
    let mean = cells.iter().sum::<f32>() / cells.len() as f32;
    let spread = (cells.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / cells.len() as f32).sqrt().max(1.0);
    cells.iter().map(|v| (v - mean) / spread).collect()
}

// How alike two are: 1 the same, 0 nothing alike.
pub fn alike(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>() / a.len() as f32
}

// The cards she has said are what (duel/known.json): a look each, its name
// and passcode; a deck read again is filled from it (she asked, 2026-10-22).
fn known_path(dir: &Path) -> PathBuf {
    dir.join("known.json")
}

pub struct Known {
    pub look: Vec<f32>,
    pub name: String,
    pub id: Option<u64>,
}

pub fn known(dir: &Path) -> Vec<Known> {
    let list: Vec<Value> = std::fs::read_to_string(known_path(dir)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    list.iter()
        .filter_map(|k| {
            Some(Known {
                look: k["p"].as_array()?.iter().map(|v| v.as_f64().unwrap_or(0.0) as f32 / 100.0).collect(),
                name: k["name"].as_str()?.to_string(),
                id: k["id"].as_u64(),
            })
        })
        .collect()
}

// What a look is, when she has said so for one like it.
pub fn recall<'a>(known: &'a [Known], look: &[f32]) -> Option<&'a Known> {
    known.iter().map(|k| (alike(&k.look, look), k)).filter(|(s, _)| *s >= ALIKE).max_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, k)| k)
}

// These learnt, each with whether she put that card right herself: only
// then does it change what a known look is (a save of names she had not
// touched once taught it 25 wrong ones, 2026-10-23); otherwise only looks
// not known yet are added. The last 3000 kept.
pub fn remember(dir: &Path, learnt: Vec<(Known, bool)>) {
    let mut all = known(dir);
    for (k, taught) in learnt {
        match all.iter_mut().find(|o| alike(&o.look, &k.look) >= ALIKE) {
            Some(o) if taught => *o = k,
            Some(_) => {}
            None => all.push(k),
        }
    }
    let skip = all.len().saturating_sub(3000);
    let list: Vec<Value> = all[skip..].iter().map(|k| json!({ "p": k.look.iter().map(|v| (v * 100.0).round() as i32).collect::<Vec<_>>(), "name": k.name, "id": k.id })).collect();
    let _ = std::fs::write(known_path(dir), Value::Array(list).to_string());
}

// The slots and what each was read as, the deck window's (duel/deck.json).
pub fn deck_path(dir: &Path) -> PathBuf {
    dir.join("deck.json")
}

pub fn read_deck(dir: &Path) -> Value {
    std::fs::read_to_string(deck_path(dir)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(json!({ "theme": "", "cards": [] }))
}

// The deck as the helper's prompt reads it: 主題, then each card "name ×n"
// in the order first seen, "？" after one any copy of which is unsure.
pub fn text_of(deck: &Value) -> String {
    let mut lines = Vec::new();
    if let Some(t) = deck["theme"].as_str().filter(|t| !t.trim().is_empty()) {
        lines.push(format!("主題：{}", t.trim()));
    }
    for (part, head) in [("m", "【主要牌組】"), ("e", "【額外牌組】")] {
        lines.push(head.to_string());
        let mut seen: Vec<(String, u64, bool)> = Vec::new();
        for c in deck["cards"].as_array().into_iter().flatten().filter(|c| c["slot"].as_str().is_some_and(|s| s.starts_with(part))) {
            let name = c["name"].as_str().unwrap_or("").trim().to_string();
            if name.is_empty() {
                continue;
            }
            let sure = c["sure"] != false;
            match seen.iter_mut().find(|s| s.0 == name) {
                Some(s) => {
                    s.1 += 1;
                    s.2 &= sure;
                }
                None => seen.push((name, 1, sure)),
            }
        }
        lines.extend(seen.into_iter().map(|(name, n, sure)| format!("{name} ×{n}{}", if sure { "" } else { " ？" })));
    }
    lines.join("\n").chars().take(1500).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_of_a_card_alike_through_its_shine_and_known_again() {
        // A card's art, and the same brighter (its shine); another card.
        let art = |f: &dyn Fn(usize, usize) -> u8| {
            let (w, h) = (68, 99);
            Image { w, h, rgb: (0..w * h).flat_map(|i| { let v = f(i % w, i / w); [v, v, v] }).collect() }
        };
        let card = art(&|x, y| ((x * 7 + y * 3) % 200) as u8);
        let shiny = art(&|x, y| (((x * 7 + y * 3) % 200) as f32 * 0.8 + 40.0) as u8);
        let other = art(&|x, y| ((x * 13 + (99 - y) * 11) % 230) as u8);
        assert!(alike(&print(&card), &print(&shiny)) > 0.99);
        assert!(alike(&print(&card), &print(&other)) < ALIKE);
        let dir = std::env::temp_dir().join(format!("wakuwaku-known-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        remember(&dir, vec![(Known { look: print(&card), name: "灰流麗".into(), id: Some(14558127) }, false)]);
        // Said again: put right, not added; a new look added.
        remember(&dir, vec![(Known { look: print(&shiny), name: "灰流麗".into(), id: Some(14558127) }, true), (Known { look: print(&other), name: "增殖的Z".into(), id: None }, false)]);
        let k = known(&dir);
        assert_eq!(k.len(), 2);
        assert_eq!(recall(&k, &print(&card)).map(|k| k.id), Some(Some(14558127)));
        // Saved under another name without her touching it: what is known stays.
        remember(&dir, vec![(Known { look: print(&card), name: "巳劍之尊 佐士".into(), id: None }, false)]);
        assert_eq!(recall(&known(&dir), &print(&card)).map(|k| k.name.clone()).as_deref(), Some("灰流麗"));
        // Put right by her: it changes.
        remember(&dir, vec![(Known { look: print(&card), name: "無限泡影".into(), id: None }, true)]);
        assert_eq!(recall(&known(&dir), &print(&card)).map(|k| k.name.clone()).as_deref(), Some("無限泡影"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_deck_as_lines_with_its_counts() {
        let deck = json!({ "theme": "雷熱祀劍", "cards": [
            { "slot": "m0", "name": "灰流麗", "sure": true },
            { "slot": "m1", "name": "灰流麗", "sure": false },
            { "slot": "m2", "name": "天叢雲之巳劍", "sure": true },
            { "slot": "e0", "name": "S:P 小夜騎士", "sure": true },
            { "slot": "e1", "name": "", "sure": true } ] });
        assert_eq!(text_of(&deck), "主題：雷熱祀劍\n【主要牌組】\n灰流麗 ×2 ？\n天叢雲之巳劍 ×1\n【額外牌組】\nS:P 小夜騎士 ×1");
    }

    #[test]
    fn a_sheet_numbers_its_cards() {
        let card = Image { w: 10, h: 14, rgb: (0..10 * 14 * 3).map(|i| (i % 251) as u8).collect() };
        let cards: Vec<(String, Image)> = (0..12).map(|i| (format!("m{i}"), Image { w: card.w, h: card.h, rgb: card.rgb.clone() })).collect();
        let s = sheet(&cards).unwrap();
        assert_eq!(s.w, (22 + 8) * 10);
        assert_eq!(s.h, (30 + LABEL_H) * 2);
        let p = prompt_of(10, 2, "雷熱祀劍", &[], &[(3, "灰流麗".into())]);
        assert!(p.contains("1～10 是主要牌組，11～12 是額外牌組") && p.contains("「雷熱祀劍」") && p.contains("3=灰流麗") && p.contains("WebSearch"));
        let p = prompt_of(10, 2, "雷熱", &["冰燃・雷熱渦爐".into(), "劍極・雷熱渦爐".into()], &[]);
        assert!(p.contains("冰燃・雷熱渦爐、劍極・雷熱渦爐") && p.contains("不用上網查") && !p.contains("WebSearch 查"));
    }
}

