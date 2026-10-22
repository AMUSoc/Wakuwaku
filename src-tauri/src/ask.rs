// Asking her a question, from the phone (mobile.rs /mobile/api/ask): the
// words go to Claude (with the web to look things up) or Antigravity
// (Google's search built in), the person's pick, as their non-interactive
// mode in her own folder (data/ask/, not shown as a session: server.rs
// is_hers). The last few questions and answers go with the next one, so it
// can be asked about further; 新問題 forgets them. (The computer had a box of
// its own on Alt+Q for a day: the user had no use for it there, 2026-10-22.)
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::Shared;

const TIMEOUT: Duration = Duration::from_secs(180);
// So many questions and answers go with the next one, each answer cut to so much.
const KEPT: usize = 3;
const ANSWER_KEPT: usize = 1500;
pub const AGENTS: [&str; 2] = ["claude", "agy"];

struct Turn {
    agent: String,
    question: String,
    answer: String,
}

static TURNS: Mutex<Vec<Turn>> = Mutex::new(Vec::new());
// What is under way: the question, its agent and when it was sent.
static PENDING: Mutex<Option<(String, String, u64)>> = Mutex::new(None);
static ERROR: Mutex<Option<String>> = Mutex::new(None);

fn dir(sh: &Shared) -> PathBuf {
    sh.dir.join("ask").join("work")
}

// The words sent: to the agent picked, with the last few turns.
pub fn send(sh: &Arc<Shared>, agent: &str, question: &str) -> bool {
    let question = question.trim().chars().take(4000).collect::<String>();
    if question.is_empty() || !AGENTS.contains(&agent) {
        return false;
    }
    {
        let mut pending = PENDING.lock().unwrap_or_else(|e| e.into_inner());
        if pending.is_some() {
            return false;
        }
        *pending = Some((agent.to_string(), question.clone(), crate::now_ms()));
    }
    *ERROR.lock().unwrap_or_else(|e| e.into_inner()) = None;
    sh.change(json!({ "askAgent": agent }));
    let sh = sh.clone();
    let agent = agent.to_string();
    crate::spawn_named("ask", move || {
        let prompt = prompt_of(&question, &TURNS.lock().unwrap_or_else(|e| e.into_inner()));
        let started = Instant::now();
        let answer = run(&sh, &agent, &prompt);
        sh.log(&format!("ask: {agent} answered in {} s ({})", started.elapsed().as_secs(), if answer.is_ok() { "ok" } else { "no answer" }));
        match answer {
            Ok(text) => {
                let mut turns = TURNS.lock().unwrap_or_else(|e| e.into_inner());
                turns.push(Turn { agent: agent.clone(), question, answer: text });
                let excess = turns.len().saturating_sub(KEPT);
                turns.drain(..excess);
            }
            Err(why) => *ERROR.lock().unwrap_or_else(|e| e.into_inner()) = Some(why),
        }
        *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = None;
    });
    true
}

// 新問題: what was asked forgotten.
pub fn clear() {
    if PENDING.lock().unwrap_or_else(|e| e.into_inner()).is_some() {
        return;
    }
    TURNS.lock().unwrap_or_else(|e| e.into_inner()).clear();
    *ERROR.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

fn prompt_of(question: &str, turns: &[Turn]) -> String {
    let mut p = String::from("你是使用者桌面上的小助手。用繁體中文、台灣用語回答，先講結論，簡短清楚（大約 300 字內，除非問題需要更長）。需要即時或最新的資訊（天氣、新聞、價格、店家、交通…）時先上網查，並附上來源。不要修改任何檔案，也不要執行指令。\n");
    if !turns.is_empty() {
        p.push_str("\n前面的對話（供參考）：\n");
        for t in turns {
            let answer: String = t.answer.chars().take(ANSWER_KEPT).collect();
            p.push_str(&format!("問：{}\n答：{}\n\n", t.question, answer));
        }
    }
    p.push_str(&format!("\n現在的問題：{question}\n"));
    p
}

fn run(sh: &Shared, agent: &str, prompt: &str) -> Result<String, String> {
    let folder = dir(sh);
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let prompt_file = folder.join("prompt.txt");
    std::fs::write(&prompt_file, prompt).map_err(|e| e.to_string())?;
    let (name, args, on_input): (&str, Vec<String>, bool) = match agent {
        // agy takes the words only as an argument; it is an .exe (run.rs).
        "agy" => ("agy", vec!["-p".into(), prompt.into()], false),
        _ => ("claude", ["-p", "--allowedTools", "WebSearch", "WebFetch"].iter().map(|s| s.to_string()).collect(), true),
    };
    let exe = crate::run::find(name).ok_or_else(|| format!("{name} not found"))?;
    if !on_input && !exe.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")) {
        return Err(format!("{name} is not an .exe here"));
    }
    let mut cmd = std::process::Command::new(&exe);
    cmd.args(&args).current_dir(&folder).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
    if on_input {
        cmd.stdin(std::fs::File::open(&prompt_file).map_err(|e| e.to_string())?);
    } else {
        cmd.stdin(std::process::Stdio::null());
    }
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
        let mut bytes = Vec::new();
        let _ = out.read_to_end(&mut bytes);
        String::from_utf8_lossy(&bytes).into_owned()
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
        return Err(text.lines().next().unwrap_or("no answer").chars().take(160).collect());
    }
    Ok(text)
}

// What has been asked and answered, what is under way, for the phone.
pub fn view(sh: &Shared) -> Value {
    let turns: Vec<Value> = TURNS.lock().unwrap_or_else(|e| e.into_inner()).iter().map(|t| json!({ "agent": t.agent, "q": t.question, "a": t.answer })).collect();
    let pending = PENDING.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|(a, q, at)| json!({ "agent": a, "q": q, "since": at }));
    json!({
        "lang": sh.lang(),
        "agent": sh.setting("askAgent").as_str().filter(|a| AGENTS.contains(a)).unwrap_or("claude"),
        "turns": turns,
        "pending": pending,
        "error": *ERROR.lock().unwrap_or_else(|e| e.into_inner()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_question_with_what_came_before() {
        let p = prompt_of("明天要帶傘嗎", &[]);
        assert!(p.contains("繁體中文") && p.contains("現在的問題：明天要帶傘嗎") && !p.contains("前面的對話"));
        let turns = vec![Turn { agent: "claude".into(), question: "台北天氣".into(), answer: "晴".repeat(2000) }];
        let p = prompt_of("那高雄呢", &turns);
        assert!(p.contains("問：台北天氣") && p.contains("現在的問題：那高雄呢"));
        assert!(p.chars().filter(|c| *c == '晴').count() == ANSWER_KEPT);
    }
}
