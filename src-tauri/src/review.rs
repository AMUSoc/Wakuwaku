// A game reviewed by the four agents (the game's ten panel, scout.rs): on
// the person's word only, once for each agent asked (a game reviewed is
// kept and shown again, never run again), each in its own part, three
// points in a hundred characters at most (cut to that if longer), and none
// of them answering another: no review goes on and on.
//
// She sums the game up herself (summary_of: the game kept, its timeline):
// the agents get a page of facts and no tools to fetch more with, so each
// costs one short answer. Each runs in its non-interactive mode (as run.rs
// finds and starts them) in a folder of her own, its words on its input or
// in a file (never on a command line through cmd.exe), its answer read from
// its output, given up on after six minutes.
//
//   claude   Claude: the game's turning points, as a whole
//   codex    Codex: the numbers: minions, gold, item timings
//   agy      Antigravity: the timeline: deaths, where and when, objectives
//   grok     Grok: the lane and the build
use std::collections::HashSet;
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::Shared;

// Short: three points, a line each, a hundred characters in all. Asked
// for 300, they wrote 500 to 700, a paper to read every game (2026-10-21).
const LIMIT: usize = 100;
const POINTS: usize = 3;
// A line kept no longer than this, whatever was written.
const LINE_CHARS: usize = 45;
// Grok takes longest (a minute for a short page, 2026-10-21).
const TIMEOUT: Duration = Duration::from_secs(360);

// The reviews under way: (game, agent).
static RUNNING: Mutex<Option<HashSet<(String, String)>>> = Mutex::new(None);

fn dir(sh: &Shared) -> PathBuf {
    sh.dir.join("lol").join("reviews")
}

fn path(sh: &Shared, id: &str) -> PathBuf {
    dir(sh).join(format!("{}.json", id.replace(['/', '\\', '.'], "_")))
}

// The reviews of a game as they are: { agent: { state: running | done | failed, text } }.
pub fn status(sh: &Shared, id: &str) -> Value {
    std::fs::read_to_string(path(sh, id)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(json!({}))
}

// As shown: one kept from before they were short, short too.
pub fn shown(sh: &Shared, id: &str) -> Value {
    let mut all = status(sh, id);
    if let Some(all) = all.as_object_mut() {
        for review in all.values_mut() {
            if let Some(text) = review["text"].as_str().map(short) {
                review["text"] = json!(text);
            }
        }
    }
    all
}

fn set(sh: &Shared, id: &str, agent: &str, state: Value) {
    static WRITING: Mutex<()> = Mutex::new(());
    let _held = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let mut all = status(sh, id);
    all[agent] = state;
    let _ = std::fs::create_dir_all(dir(sh));
    let _ = std::fs::write(path(sh, id), all.to_string());
}

// --- The game summed up (pure: tested) --------------------------------------------------------

// Where on the map, roughly, for the words: the lanes, the river, either
// side's jungle and base (blue is team 100's).
pub fn area_of(x: f64, y: f64, mine_blue: bool) -> &'static str {
    let ours_if = |blue: bool| blue == mine_blue;
    if x < 3500.0 && y < 3500.0 {
        return if ours_if(true) { "我方基地" } else { "敵方基地" };
    }
    if x > 11500.0 && y > 11500.0 {
        return if ours_if(false) { "我方基地" } else { "敵方基地" };
    }
    if (x - y).abs() < 1800.0 {
        return "中路";
    }
    if (x + y - 15000.0).abs() < 1600.0 {
        return "河道";
    }
    if x < 2500.0 || y > 12500.0 {
        return "上路";
    }
    if y < 2500.0 || x > 12500.0 {
        return "下路";
    }
    let blue_side = x + y < 15000.0;
    if ours_if(blue_side) { "我方野區" } else { "敵方野區" }
}

fn clock(secs: f64) -> String {
    format!("{}:{:02}", (secs / 60.0).floor() as u64, (secs % 60.0).round() as u64)
}

// A page of facts about her game, in Traditional Chinese: hers and her lane
// opponent's numbers, the team gold difference and hers with the opponent
// through the game, her deaths (when, where), the big monsters, her build.
pub fn summary_of(game: &Value, timeline: &Value, who: &str, names: &dyn Fn(u64) -> String, items: &dyn Fn(u64) -> String) -> Option<String> {
    let players = game["players"].as_array()?;
    let me = players.iter().find(|p| p["puuid"] == who)?;
    let my_team = me["team"].as_u64().unwrap_or(100);
    let opp = players.iter().find(|p| p["team"].as_u64() != Some(my_team) && p["pos"] == me["pos"] && !me["pos"].as_str().unwrap_or("").is_empty());
    let minutes = game["duration"].as_f64().unwrap_or(1.0) / 60.0;
    let n = |p: &Value, k: &str| p[k].as_f64().unwrap_or(0.0);
    let team_dmg: f64 = players.iter().filter(|p| p["team"].as_u64() == Some(my_team)).map(|p| n(p, "dmg")).sum();
    let line = |p: &Value| {
        format!(
            "{}（{}）K/D/A {}/{}/{}，補兵 {}（{:.1}/分），對英雄傷害 {}，金錢 {}，視野 {}",
            names(p["champion"].as_u64().unwrap_or(0)),
            p["pos"].as_str().unwrap_or(""),
            n(p, "k"), n(p, "d"), n(p, "a"),
            n(p, "cs"), n(p, "cs") / minutes,
            n(p, "dmg"), n(p, "gold"), n(p, "vision"),
        )
    };
    let mut out = Vec::new();
    out.push(format!("模式 {}，時長 {}，結果：{}。", game["queue"], clock(game["duration"].as_f64().unwrap_or(0.0)), if me["win"] == true { "勝利" } else { "失敗" }));
    out.push(format!("我：{}，擊殺參與 {}%，傷害佔全隊 {:.0}%。", line(me), n(me, "kp"), if team_dmg > 0.0 { n(me, "dmg") / team_dmg * 100.0 } else { 0.0 }));
    if let Some(o) = opp {
        out.push(format!("對線對手：{}。", line(o)));
    }
    let side = |team: u64| players.iter().filter(|p| p["team"].as_u64() == Some(team)).map(|p| names(p["champion"].as_u64().unwrap_or(0))).collect::<Vec<_>>().join("、");
    out.push(format!("我方：{}。敵方：{}。", side(my_team), side(if my_team == 100 { 200 } else { 100 })));
    let built = me["items"].as_array().into_iter().flatten().filter_map(|i| i.as_u64()).filter(|i| *i > 0).map(items).collect::<Vec<_>>().join("、");
    out.push(format!("最後的裝備：{built}。"));
    // The timeline, if there is one.
    if let Some(frames) = timeline["frames"].as_array() {
        let puuids: Vec<&str> = timeline["puuids"].as_array().into_iter().flatten().map(|p| p.as_str().unwrap_or("")).collect();
        let place = |puuid: &str| puuids.iter().position(|p| *p == puuid);
        let team_of = |i: usize| players.iter().find(|p| p["puuid"] == puuids.get(i).copied().unwrap_or("")).and_then(|p| p["team"].as_u64());
        let mine = place(who);
        let theirs = opp.and_then(|o| place(o["puuid"].as_str().unwrap_or("")));
        let at = |m: usize| frames.get(m).or(frames.last());
        let mut diffs = Vec::new();
        for m in [5, 10, 15, 20, 25, 30].into_iter().filter(|m| *m < frames.len()).chain(std::iter::once(frames.len().saturating_sub(1))) {
            let Some(f) = at(m) else { continue };
            let gold = f["gold"].as_array().cloned().unwrap_or_default();
            let diff: i64 = gold.iter().enumerate().map(|(i, g)| if team_of(i) == Some(my_team) { g.as_i64().unwrap_or(0) } else { -g.as_i64().unwrap_or(0) }).sum();
            diffs.push(format!("{m} 分 {diff:+}"));
        }
        out.push(format!("隊伍經濟差（我方減敵方）：{}。", diffs.join("、")));
        if let (Some(a), Some(b)) = (mine, theirs) {
            let mut lane = Vec::new();
            for m in [10, 15, 20].into_iter().filter(|m| *m < frames.len()) {
                let f = &frames[m];
                let g = |i: usize, k: &str| f[k][i].as_i64().unwrap_or(0);
                lane.push(format!("{m} 分 金錢 {:+}、補兵 {:+}", g(a, "gold") - g(b, "gold"), g(a, "cs") - g(b, "cs")));
            }
            out.push(format!("我對線上對手：{}。", lane.join("；")));
        }
        if let Some(a) = mine {
            let me1 = (a + 1) as u64;
            let mine_blue = my_team == 100;
            let deaths: Vec<String> = timeline["kills"].as_array().into_iter().flatten().filter(|k| k["victim"].as_u64() == Some(me1)).map(|k| {
                let killer = k["killer"].as_u64().unwrap_or(0) as usize;
                let by = players.iter().find(|p| p["puuid"] == puuids.get(killer.wrapping_sub(1)).copied().unwrap_or("")).map(|p| names(p["champion"].as_u64().unwrap_or(0))).unwrap_or_else(|| "防禦塔或野怪".into());
                format!("{} 在{}被{}擊殺（{} 人助攻）", clock(k["t"].as_f64().unwrap_or(0.0)), area_of(k["x"].as_f64().unwrap_or(0.0), k["y"].as_f64().unwrap_or(0.0), mine_blue), by, k["assists"].as_array().map_or(0, |a| a.len()))
            }).collect();
            out.push(format!("我的陣亡：{}。", if deaths.is_empty() { "沒有".to_string() } else { deaths.join("；") }));
            let builds: Vec<String> = timeline["items"].as_array().into_iter().flatten().filter(|e| e["p"].as_u64() == Some(me1) && e["item"].as_u64().is_some_and(|i| i >= 3000 && ![3340, 3363, 3364, 3330, 3513].contains(&i))).map(|e| format!("{} {}", clock(e["t"].as_f64().unwrap_or(0.0)), items(e["item"].as_u64().unwrap_or(0)))).collect();
            out.push(format!("我買成裝和大件的時間：{}。", builds.join("、")));
        }
        let monsters: Vec<String> = timeline["monsters"].as_array().into_iter().flatten().map(|m| {
            let kind = match m["type"].as_str().unwrap_or("") {
                "DRAGON" => "小龍",
                "BARON_NASHOR" => "巴龍",
                "RIFTHERALD" => "預示者",
                "HORDE" => "虛空蟲",
                _ => "大型野怪",
            };
            format!("{} {}{}", clock(m["t"].as_f64().unwrap_or(0.0)), if m["team"].as_u64() == Some(my_team) { "我方拿" } else { "敵方拿" }, kind)
        }).collect();
        out.push(format!("大型野怪：{}。", if monsters.is_empty() { "沒有".to_string() } else { monsters.join("、") }));
    }
    Some(out.join("\n"))
}

// What each is asked: its part, and the rules (no tools, the limit).
pub fn prompt_of(agent: &str, summary: &str) -> String {
    let part = match agent {
        "claude" => "你負責「總評」：這場輸贏的關鍵轉折。",
        "codex" => "你負責「數字」：最拖累或最幫助這場的數字，和一個具體目標。",
        "agy" => "你負責「時間軸」：最該避免的死亡和錯過的機會。",
        _ => "你負責「對線和出裝」：對線哪裡吃虧或佔便宜，出裝哪裡可以更好。",
    };
    format!(
        "你是英雄聯盟教練，正在和另外三位教練一起檢討玩家「我」的一場對局，每人負責一塊。{part}\n\
         規則：只根據下面的資料回答，不要使用任何工具、不要讀寫檔案、不要上網；用繁體中文和台灣的遊戲用語。\n\
         格式：最多 {POINTS} 行，每行一個重點、一句話，全部加起來 {LIMIT} 字以內。\
         不要開場白、不要小標題、不要粗體、不要重複資料、不要解釋理由。\n\n=== 對局資料 ===\n{summary}"
    )
}

// --- Running them -------------------------------------------------------------------------------

// What runs for each, and whether the words go on its input (else a file,
// or for agy, an argument to its .exe).
fn command_of(agent: &str, prompt_file: &str, prompt: &str) -> (&'static str, Vec<String>, bool) {
    let v = |a: &[&str]| a.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    match agent {
        // Out of a Git repository, codex exec wants it said.
        "codex" => ("codex", v(&["exec", "--skip-git-repo-check", "-"]), true),
        "grok" => ("grok", v(&["--prompt-file", prompt_file, "--permission-mode", "default"]), false),
        "agy" => ("agy", v(&["-p", prompt]), false),
        _ => ("claude", v(&["-p"]), true),
    }
}

// One of them asked something else (the duel's four, duel.rs): the same way, the same shortness.
// Its answer whole (its lines tidied, none cut short: she could not read
// them cut, 2026-10-23), at most six lines.
pub fn ask(agent: &str, folder: &std::path::Path, prompt: &str) -> Result<String, String> {
    run_whole(agent, folder, prompt).map(|text| {
        text.lines()
            .map(|l| l.trim().trim_start_matches(['-', '*', '•', '·', ' ']).replace("**", ""))
            .filter(|l| !l.is_empty())
            .take(6)
            .collect::<Vec<_>>()
            .join("
")
    })
}

fn run(agent: &str, folder: &std::path::Path, prompt: &str) -> Result<String, String> {
    run_whole(agent, folder, prompt).map(|text| short(&text))
}

fn run_whole(agent: &str, folder: &std::path::Path, prompt: &str) -> Result<String, String> {
    let prompt_file = folder.join(format!("{agent}.prompt.txt"));
    std::fs::write(&prompt_file, prompt).map_err(|e| e.to_string())?;
    let file = prompt_file.to_string_lossy().into_owned();
    let (name, mut args, on_input) = command_of(agent, &file, prompt);
    if name == "claude" {
        args.extend(crate::run::quiet(folder));
    }
    let exe = crate::run::find(name).ok_or(format!("{name} not found"))?;
    // Words as an argument never through cmd.exe.
    if agent == "agy" && !exe.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")) {
        return Err(format!("{name} is not an .exe here"));
    }
    let input = if on_input { std::process::Stdio::from(std::fs::File::open(&prompt_file).map_err(|e| e.to_string())?) } else { std::process::Stdio::null() };
    let mut cmd = std::process::Command::new(&exe);
    cmd.args(&args).current_dir(folder).stdin(input).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
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
    // Its answer read as it comes (a full pipe would stop it).
    let mut stdout = child.stdout.take().ok_or("no output")?;
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stdout.read_to_string(&mut text);
        text
    });
    let started = Instant::now();
    let ok = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Ok(None) if started.elapsed() > TIMEOUT => {
                let _ = child.kill();
                let _ = std::fs::remove_file(&prompt_file);
                return Err("timeout".into());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(300)),
            Err(e) => return Err(e.to_string()),
        }
    };
    let _ = std::fs::remove_file(&prompt_file);
    let text = reader.join().unwrap_or_default();
    let text = text.trim();
    if text.is_empty() {
        return Err("no answer".into());
    }
    // Ended badly: what it said is why (Claude Code not logged in said so
    // on its output, 2026-10-21), not a review.
    if !ok {
        return Err(text.lines().next().unwrap_or("").chars().take(120).collect());
    }
    Ok(text.to_string())
}

// What is shown, whatever was written: its first three lines with words,
// marks of lists and bold taken off, each cut to a line's length.
pub fn short(text: &str) -> String {
    text.lines()
        .map(|l| l.trim().trim_start_matches(['-', '*', '•', '·', ' ']).replace("**", ""))
        .map(|l| {
            // A list's number ("1. ") off as well.
            let digits = l.chars().take_while(|c| c.is_ascii_digit()).count();
            match l[digits..].chars().next() {
                Some('.' | '、' | ')') if digits > 0 => l[digits..].chars().skip(1).collect::<String>().trim().to_string(),
                _ => l,
            }
        })
        .filter(|l| !l.is_empty())
        .take(POINTS)
        .map(|l| if l.chars().count() > LINE_CHARS { format!("{}…", l.chars().take(LINE_CHARS).collect::<String>()) } else { l })
        .collect::<Vec<_>>()
        .join("\n")
}

// The game reviewed by these agents, those not yet asked (or that failed):
// the ones done stay as they are. Returns the reviews as they now are.
pub fn start(sh: &Arc<Shared>, id: &str, who: &str, agents: &[String]) -> Value {
    let game = crate::scout::kept_game(sh, id);
    let timeline = crate::scout::timeline(sh, id);
    let names = crate::scout::champion_names();
    let item_names = crate::scout::item_names();
    let Some(summary) = summary_of(&game, &timeline, who, &|c| names.get(&c).cloned().unwrap_or_else(|| format!("英雄{c}")), &|i| item_names.get(&i).cloned().unwrap_or_else(|| format!("裝備{i}"))) else {
        return json!({ "error": "nogame" });
    };
    let folder = dir(sh).join("work");
    let _ = std::fs::create_dir_all(&folder);
    let before = status(sh, id);
    for agent in agents.iter().filter(|a| crate::data::AGENTS.contains(&a.as_str())) {
        if before[agent.as_str()]["state"] == "done" {
            continue;
        }
        let key = (id.to_string(), agent.clone());
        {
            let mut running = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
            if !running.get_or_insert_with(HashSet::new).insert(key.clone()) {
                continue;
            }
        }
        set(sh, id, agent, json!({ "state": "running", "at": crate::now_ms() }));
        let (sh, id, agent, prompt, folder) = (sh.clone(), id.to_string(), agent.clone(), prompt_of(agent, &summary), folder.clone());
        crate::spawn_named("review", move || {
            // Each its own folder: two writing their words to one file would not do.
            let own = folder.join(&agent);
            let _ = std::fs::create_dir_all(&own);
            let said = run(&agent, &own, &prompt);
            sh.log(&format!("review of {id} by {agent}: {}", if said.is_ok() { "done".to_string() } else { format!("failed, {:?}", said.as_ref().err()) }));
            set(&sh, &id, &agent, match said {
                Ok(text) => json!({ "state": "done", "text": text, "at": crate::now_ms() }),
                Err(why) => json!({ "state": "failed", "why": why, "at": crate::now_ms() }),
            });
            RUNNING.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashSet::new).remove(&(id, agent));
        });
    }
    shown(sh, id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn where_on_the_map() {
        assert_eq!(area_of(1000.0, 1000.0, true), "我方基地");
        assert_eq!(area_of(14000.0, 14000.0, true), "敵方基地");
        assert_eq!(area_of(7400.0, 7500.0, true), "中路");
        assert_eq!(area_of(5000.0, 10000.0, true), "河道");
        assert_eq!(area_of(1500.0, 9000.0, true), "上路");
        assert_eq!(area_of(11000.0, 1200.0, true), "下路");
        assert_eq!(area_of(4000.0, 6500.0, true), "我方野區");
        assert_eq!(area_of(4000.0, 6500.0, false), "敵方野區");
    }

    #[test]
    fn a_game_summed_up_for_the_coaches() {
        let p = |puuid: &str, team: u64, champ: u64, win: bool| {
            json!({ "puuid": puuid, "team": team, "champion": champ, "pos": "MIDDLE", "win": win, "k": 5, "d": 3, "a": 7, "cs": 200, "dmg": 20000, "gold": 12000, "vision": 20, "kp": 55, "items": [3031, 0, 1055] })
        };
        let game = json!({ "queue": 420, "duration": 1800.0, "players": [p("me", 100, 234, true), p("them", 200, 103, false)] });
        let frame = |m: u64| json!({ "t": m, "gold": [500 * m + 100, 500 * m], "cs": [8 * m, 7 * m] });
        let timeline = json!({
            "puuids": ["me", "them"], "frames": (0..=30).map(frame).collect::<Vec<_>>(),
            "kills": [{ "t": 600.0, "killer": 2, "victim": 1, "assists": [], "x": 7400, "y": 7500 }],
            "items": [{ "t": 900.0, "p": 1, "item": 3031 }, { "t": 30.0, "p": 1, "item": 2003 }],
            "monsters": [{ "t": 700.0, "team": 100, "type": "DRAGON" }],
        });
        let names = |c: u64| if c == 234 { "維爾戈".to_string() } else { "阿璃".to_string() };
        let items = |i: u64| if i == 3031 { "無盡之刃".to_string() } else { format!("裝備{i}") };
        let s = summary_of(&game, &timeline, "me", &names, &items).unwrap();
        assert!(s.contains("結果：勝利"));
        assert!(s.contains("對線對手：阿璃"));
        assert!(s.contains("10 分 金錢 +100、補兵 +10"));
        assert!(s.contains("10:00 在中路被阿璃擊殺"));
        assert!(s.contains("15:00 無盡之刃"));
        assert!(s.contains("最後的裝備：無盡之刃、裝備1055"));
        assert!(!s.contains("2003"));
        assert!(s.contains("我方拿小龍"));
        assert!(prompt_of("codex", &s).contains("100 字以內"));
    }

    #[test]
    fn what_is_shown_is_short_whatever_was_written() {
        let long = "**轉折一：前期蟲子全丟**\n\n1. 8 分多敵方連拿三隻虛空蟲\n- 25:23 你單獨進敵方野區被抓\n3. 31:59 巴龍翻盤\n4. 第四點不要";
        assert_eq!(short(long), "轉折一：前期蟲子全丟\n8 分多敵方連拿三隻虛空蟲\n25:23 你單獨進敵方野區被抓");
        let one = "很".repeat(60);
        assert_eq!(short(&one).chars().count(), LINE_CHARS + 1);
    }
}
