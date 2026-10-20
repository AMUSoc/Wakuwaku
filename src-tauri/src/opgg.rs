// A champion's build and matchup from OP.GG, asked through Claude Code:
// OP.GG's terms forbid scripts collecting from it (op.gg/lol/policies/
// agreement) and it has no public API, but it offers its data to AI agents
// through its own MCP server (github.com/opgginc/opgg-mcp). So Wakuwaku asks
// Claude, with that server lent for the one run (--mcp-config, the person's
// own Claude Code settings untouched; --strict-mcp-config, no other) and
// only its two lookups allowed (--allowedTools): no other tool, nothing
// skipped. Claude answers a line of JSON; she draws it (scout.js).
//
// On the person's word only (a button), kept a day for the same champion,
// position and opponent: one short Claude run, about twenty seconds.
use std::io::Read;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::Shared;

const MCP: &str = r#"{ "mcpServers": { "opgg": { "type": "http", "url": "https://mcp-api.op.gg/mcp" } } }"#;
const TOOLS: [&str; 2] = ["mcp__opgg__lol_get_champion_analysis", "mcp__opgg__lol_get_lane_matchup_guide"];
const TIMEOUT: Duration = Duration::from_secs(180);
const KEPT_MS: u64 = 24 * 60 * 60 * 1000;

// One at a time: two lookups would be two Claude runs.
static ASKING: Mutex<()> = Mutex::new(());

// OP.GG's names of the positions, from Riot's.
// None known (a mode without positions): "", OP.GG's usual one for it.
pub fn position_of(pos: &str) -> &'static str {
    match pos {
        "TOP" => "TOP",
        "JUNGLE" => "JUNGLE",
        "MIDDLE" => "MID",
        "BOTTOM" => "ADC",
        "UTILITY" => "SUPPORT",
        _ => "",
    }
}

pub fn prompt_of(champion: &str, position: &str, opponent: Option<&str>) -> String {
    let vs = opponent.map_or(String::new(), |o| format!("，對面同位置是 {o}"));
    let position = if position.is_empty() { "用 OP.GG 上它最常見的位置" } else { position };
    let vs_field = opponent.map_or("null".to_string(), |o| format!("{champion} 對 {o} 的勝率(0~1，查不到用 null)"));
    let tips = opponent.map_or("這隻英雄這個位置的重點".to_string(), |o| format!("對上 {o} 的重點"));
    format!(
        "用 opgg 的工具查：我玩 {champion}，位置 {position}{vs}。只回一行 JSON，不要其他文字：\
         {{\"patch\":\"版本\",\"position\":\"TOP/JUNGLE/MID/ADC/SUPPORT 之一\",\"winrate\":{champion} 這個位置的勝率(0~1),\"vs\":{vs_field},\
         \"core\":[核心裝備 item id 三件],\"boots\":鞋子 item id,\"keystone\":主符文 id,\"sub\":副系符文樹 id,\
         \"spells\":[召喚師技能 id],\"skills\":\"如 Q>E>W\",\"counters\":[最難打的 3 隻英雄英文名],\
         \"tips\":[\"{tips}，繁體中文台灣用語，一句 25 字內\",\"第二句\"]}}。查不到就回 {{\"error\":\"原因\"}}。"
    )
}

// The JSON in what Claude answered (it may say a word around it).
pub fn json_in(text: &str) -> Option<Value> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    serde_json::from_str(&text[start..=end]).ok()
}

fn dir(sh: &Shared) -> PathBuf {
    sh.dir.join("lol").join("opgg")
}

// The build and matchup for her champion in a position, against an
// opponent or none: kept (a day), or asked of Claude now.
pub fn ask(sh: &Shared, champion: u64, pos: &str, opponent: Option<u64>) -> Value {
    let en = crate::scout::champion_names_en();
    let Some(name) = en.get(&champion).cloned() else { return json!({ "error": "unknown champion" }) };
    let rival = opponent.and_then(|o| en.get(&o).cloned());
    let position = position_of(pos);
    let kept = dir(sh).join(format!("{champion}-{}-{}.json", if position.is_empty() { "ANY" } else { position }, opponent.unwrap_or(0)));
    let fresh = |v: &Value| v["at"].as_u64().is_some_and(|at| crate::now_ms().saturating_sub(at) < KEPT_MS);
    if let Some(v) = std::fs::read_to_string(&kept).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).filter(fresh) {
        return v;
    }
    let _one = ASKING.lock().unwrap_or_else(|e| e.into_inner());
    let prompt = prompt_of(&name, position, rival.as_deref());
    let answer = match run(sh, &prompt) {
        Ok(text) => json_in(&text).unwrap_or_else(|| json!({ "error": text.chars().take(120).collect::<String>() })),
        Err(why) => json!({ "error": why }),
    };
    sh.log(&format!("opgg: {name} {position}{}: {}", rival.as_ref().map_or(String::new(), |r| format!(" vs {r}")), if answer["error"].is_null() { "found" } else { "not found" }));
    let mut answer = answer;
    if answer["error"].is_null() {
        // The counters by id, as the page draws champions.
        let by_name = |n: &str| en.iter().find(|(_, v)| v.eq_ignore_ascii_case(n)).map(|(id, _)| *id);
        let ids: Vec<Value> = answer["counters"].as_array().into_iter().flatten().filter_map(|n| n.as_str().and_then(by_name)).map(|id| json!(id)).collect();
        answer["counterIds"] = json!(ids);
        answer["at"] = json!(crate::now_ms());
        let _ = std::fs::create_dir_all(dir(sh));
        let _ = std::fs::write(&kept, answer.to_string());
    }
    answer
}

fn run(sh: &Shared, prompt: &str) -> Result<String, String> {
    let folder = dir(sh).join("work");
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let mcp = folder.join("opgg-mcp.json");
    std::fs::write(&mcp, MCP).map_err(|e| e.to_string())?;
    let prompt_file = folder.join("prompt.txt");
    std::fs::write(&prompt_file, prompt).map_err(|e| e.to_string())?;
    let exe = crate::run::find("claude").ok_or("claude not found")?;
    let mut args: Vec<String> = vec!["-p".into(), "--mcp-config".into(), mcp.to_string_lossy().into_owned(), "--strict-mcp-config".into(), "--allowedTools".into()];
    args.extend(TOOLS.iter().map(|t| t.to_string()));
    let mut cmd = std::process::Command::new(&exe);
    // The words on its input, never on a command line (claude is a .cmd shim).
    cmd.args(&args)
        .current_dir(&folder)
        .stdin(std::fs::File::open(&prompt_file).map_err(|e| e.to_string())?)
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
            Ok(None) if started.elapsed() > TIMEOUT => {
                let _ = child.kill();
                return Err("timeout".into());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(300)),
            Err(e) => return Err(e.to_string()),
        }
    };
    let _ = std::fs::remove_file(&prompt_file);
    let text = reader.join().unwrap_or_default().trim().to_string();
    if !ok || text.is_empty() {
        return Err(text.lines().next().unwrap_or("no answer").chars().take(120).collect());
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_claude_is_asked_and_what_is_read_of_it() {
        let p = prompt_of("Viego", "JUNGLE", Some("Pantheon"));
        assert!(p.contains("對面同位置是 Pantheon") && p.contains("Viego 對 Pantheon 的勝率"));
        assert!(prompt_of("Viego", "JUNGLE", None).contains("\"vs\":null"));
        assert_eq!(position_of("UTILITY"), "SUPPORT");
        assert_eq!(position_of("MIDDLE"), "MID");
        assert_eq!(position_of(""), "");
        assert!(prompt_of("Xayah", "", None).contains("最常見的位置"));
        let said = "好的：\n{\"patch\":\"16.20\",\"winrate\":0.49,\"core\":[6672]}\n";
        assert_eq!(json_in(said).unwrap()["core"][0], 6672);
        assert_eq!(json_in("沒有"), None);
    }
}
