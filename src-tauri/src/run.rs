// A new task from the phone (POST /mobile/api/run): an agent's
// non-interactive mode in its project folder (settings agentDirs; none set,
// none run), its hooks reporting to her as any session's do, its prompts
// answered on the phone where its agent sends them through a hook.
//
// The words never go on a command line (claude and codex are .cmd shims,
// run by cmd.exe, where & or | in them would be commands): she writes them
// to a file and starts a helper of hers (--wakuwaku-run <job>) out of her
// job (detach.rs), which runs the agent with the file as its input:
//
//   claude  claude -p                                 (the words on stdin)
//   codex   codex exec -                              (the words on stdin)
//   grok    grok --prompt-file <file> --permission-mode default
//   agy     agy -p <words>                            (agy.exe takes them only so;
//                                                      an .exe: no cmd.exe between)
//
// No flag that skips a permission. Grok's --permission-mode default is the
// other way: the person's config may approve everything (always-approve),
// and a task from the phone must not inherit that. What each does with a
// tool that would ask (tried 2026-10-17; README): Claude Code asks through
// its PermissionRequest hook (answered on the phone; unanswered, denied);
// Codex's exec asks nobody, in its read-only sandbox; Grok's and
// Antigravity's headless modes deny it.
//
// One task from the phone at a time for each agent: her job file says which
// helper runs it (its pid), until the helper writes how it ended.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::{json, Value};

use crate::{now_ms, Shared};

pub const FLAG: &str = "--wakuwaku-run";
// The longest prompt taken, in characters.
const MAX_PROMPT: usize = 8000;
// So many of a log line's words, at most (the person's words are theirs).
const LOGGED: usize = 80;
// The runs kept in her folder (their job files and output).
const KEEP: usize = 20;

struct Run {
    pid: u32,
    started: u64,
    job: PathBuf,
}

static RUNS: Mutex<Option<HashMap<String, Run>>> = Mutex::new(None);

fn runs_dir(sh: &Shared) -> PathBuf {
    sh.dir.join("runs")
}

// The agent's project folder, as set and there.
pub fn folder(sh: &Shared, agent: &str) -> Option<PathBuf> {
    let dir = sh.setting("agentDirs").get(agent)?.as_str()?.trim().to_string();
    let path = PathBuf::from(dir);
    path.is_dir().then_some(path)
}

// Where it runs: the folder, or one under it (cwd), whole paths compared.
pub fn workdir(folder: &Path, cwd: Option<&str>) -> Option<PathBuf> {
    let root = std::fs::canonicalize(folder).ok()?;
    let Some(cwd) = cwd.map(str::trim).filter(|c| !c.is_empty()) else { return Some(folder.to_path_buf()) };
    let asked = Path::new(cwd);
    let asked = if asked.is_absolute() { asked.to_path_buf() } else { folder.join(asked) };
    let real = std::fs::canonicalize(&asked).ok()?;
    (real.starts_with(&root) && real.is_dir()).then_some(asked)
}

pub fn first_words(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    match flat.char_indices().nth(LOGGED) {
        Some((at, _)) => format!("{}…", &flat[..at]),
        None => flat,
    }
}

// The task from the phone running for an agent: when it began, if one is.
pub fn running(agent: &str) -> Option<u64> {
    let mut runs = RUNS.lock().unwrap();
    let map = runs.get_or_insert_with(HashMap::new);
    let alive = map.get(agent).is_some_and(|r| !ended(&r.job) && alive(r.pid));
    if !alive {
        map.remove(agent);
    }
    map.get(agent).map(|r| r.started)
}

fn ended(job: &Path) -> bool {
    std::fs::read_to_string(job).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).is_some_and(|v| v.get("ended").is_some())
}

pub fn start(sh: &Shared, agent: &str, prompt: &str, cwd: Option<&str>) -> Result<Value, &'static str> {
    if !crate::data::AGENTS.contains(&agent) {
        return Err("agent");
    }
    let prompt = prompt.trim();
    if prompt.is_empty() || prompt.chars().count() > MAX_PROMPT {
        return Err("prompt");
    }
    let folder = folder(sh, agent).ok_or("noFolder")?;
    let dir = workdir(&folder, cwd).ok_or("badCwd")?;
    if running(agent).is_some() {
        return Err("busy");
    }
    let runs = runs_dir(sh);
    std::fs::create_dir_all(&runs).map_err(|_| "io")?;
    prune(&runs);
    let started = now_ms();
    let job = runs.join(format!("{agent}-{started}.json"));
    let words = runs.join(format!("{agent}-{started}.prompt.txt"));
    std::fs::write(&words, prompt).map_err(|_| "io")?;
    let spec = json!({ "agent": agent, "dir": dir.to_string_lossy(), "prompt": words.to_string_lossy(), "log": runs.join(format!("{agent}-{started}.log")).to_string_lossy(), "started": started });
    std::fs::write(&job, spec.to_string()).map_err(|_| "io")?;
    let exe = std::env::current_exe().map_err(|_| "io")?;
    let job_arg = job.to_string_lossy().into_owned();
    let Some(pid) = crate::detach::spawn_pid(&exe, &[FLAG, &job_arg]) else {
        let _ = std::fs::remove_file(&words);
        return Err("spawn");
    };
    RUNS.lock().unwrap().get_or_insert_with(HashMap::new).insert(agent.to_string(), Run { pid, started, job });
    sh.log(&format!("run from the phone: {agent} in {} (helper {pid}): {}", dir.display(), first_words(prompt)));
    Ok(json!({ "ok": true, "agent": agent, "started": started }))
}

// Only the last few runs' files kept.
fn prune(runs: &Path) {
    let Ok(entries) = std::fs::read_dir(runs) else { return };
    let mut jobs: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "json")).collect();
    jobs.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    let excess = jobs.len().saturating_sub(KEEP);
    for job in &jobs[..excess] {
        let stem = job.with_extension("");
        for path in [job.clone(), stem.with_extension("log"), stem.with_extension("prompt.txt")] {
            let _ = std::fs::remove_file(path);
        }
    }
}

// --- The helper (her exe again, out of her job) ---------------------------------------

// An agent's command, as found: on the PATH, or where each installs itself.
fn find(name: &str) -> Option<PathBuf> {
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    dirs.extend([env("APPDATA").map(|a| a.join("npm")), env("USERPROFILE").map(|h| h.join(".grok").join("bin")), env("LOCALAPPDATA").map(|l| l.join("agy").join("bin"))].into_iter().flatten());
    let names: Vec<String> = if cfg!(windows) { ["exe", "cmd"].iter().map(|e| format!("{name}.{e}")).collect() } else { vec![name.to_string()] };
    dirs.iter().flat_map(|d| names.iter().map(move |n| d.join(n))).find(|p| p.is_file())
}

// What runs, and whether the words go on its input.
pub fn command(agent: &str, prompt_file: &str, prompt: &str) -> (&'static str, Vec<String>, bool) {
    match agent {
        "codex" => ("codex", vec!["exec".into(), "-".into()], true),
        "grok" => ("grok", vec!["--prompt-file".into(), prompt_file.into(), "--permission-mode".into(), "default".into()], false),
        "agy" => ("agy", vec!["-p".into(), prompt.into()], false),
        _ => ("claude", vec!["-p".into()], true),
    }
}

pub fn helper(job_path: &str) {
    let job_path = PathBuf::from(job_path);
    let Some(mut job) = std::fs::read_to_string(&job_path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else { return };
    let get = |k: &str| job[k].as_str().unwrap_or("").to_string();
    let (agent, dir, prompt_file, log) = (get("agent"), get("dir"), get("prompt"), get("log"));
    let prompt = std::fs::read_to_string(&prompt_file).unwrap_or_default();
    let (name, args, on_input) = command(&agent, &prompt_file, &prompt);
    let outcome = match find(name) {
        // agy's words are an argument: never through cmd.exe.
        Some(exe) if agent == "agy" && !exe.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")) => Err("agy is not an .exe here".to_string()),
        Some(exe) => run_it(&exe, &args, &dir, &prompt_file, on_input, &log),
        None => Err(format!("{name} not found")),
    };
    // The words gone with the run; how it ended kept.
    let _ = std::fs::remove_file(&prompt_file);
    job["ended"] = json!(now_ms());
    match outcome {
        Ok(code) => job["exit"] = json!(code),
        Err(why) => job["error"] = json!(why),
    }
    let _ = std::fs::write(&job_path, job.to_string());
}

fn run_it(exe: &Path, args: &[String], dir: &str, prompt_file: &str, on_input: bool, log: &str) -> Result<Option<i32>, String> {
    let out = std::fs::File::create(log).map_err(|e| e.to_string())?;
    let err = out.try_clone().map_err(|e| e.to_string())?;
    let input = if on_input { std::process::Stdio::from(std::fs::File::open(prompt_file).map_err(|e| e.to_string())?) } else { std::process::Stdio::null() };
    let mut cmd = std::process::Command::new(exe);
    cmd.args(args).current_dir(dir).stdin(input).stdout(out).stderr(err);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // No console window of its own.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.status().map(|s| s.code()).map_err(|e| e.to_string())
}

// --- Whether a helper still runs --------------------------------------------------------

#[cfg(windows)]
fn alive(pid: u32) -> bool {
    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
        fn GetExitCodeProcess(process: isize, code: *mut u32) -> i32;
        fn CloseHandle(handle: isize) -> i32;
    }
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;
    // SAFETY: a handle we open and close; the code written into our own u32.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h == 0 {
            return false;
        }
        let mut code = 0u32;
        let ok = GetExitCodeProcess(h, &mut code) != 0 && code == STILL_ACTIVE;
        CloseHandle(h);
        ok
    }
}

#[cfg(not(windows))]
fn alive(pid: u32) -> bool {
    // SAFETY: signal 0 only asks whether the process is there.
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_agent_s_non_interactive_command_and_no_skipping() {
        assert_eq!(command("claude", "p.txt", "hi"), ("claude", vec!["-p".to_string()], true));
        assert_eq!(command("codex", "p.txt", "hi"), ("codex", vec!["exec".to_string(), "-".to_string()], true));
        assert_eq!(command("grok", "p.txt", "hi").1, ["--prompt-file", "p.txt", "--permission-mode", "default"]);
        assert_eq!(command("agy", "p.txt", "a \"quoted\" & piped | thing"), ("agy", vec!["-p".to_string(), "a \"quoted\" & piped | thing".to_string()], false));
        for agent in crate::data::AGENTS {
            let (_, args, _) = command(agent, "p.txt", "hi");
            assert!(!args.iter().any(|a| a.contains("dangerously") || a.contains("always-approve") || a.contains("bypass") || a.contains("yolo")), "{agent}");
        }
    }

    #[test]
    fn only_under_the_agent_s_folder() {
        let root = std::env::temp_dir().join(format!("wakuwaku-run-{}", std::process::id()));
        let sub = root.join("app").join("src");
        std::fs::create_dir_all(&sub).unwrap();
        assert_eq!(workdir(&root, None), Some(root.clone()));
        assert_eq!(workdir(&root, Some("  ")), Some(root.clone()));
        assert!(workdir(&root, Some("app/src")).is_some());
        assert!(workdir(&root, Some(&sub.to_string_lossy())).is_some());
        assert_eq!(workdir(&root, Some("..")), None);
        assert_eq!(workdir(&root, Some("app/../..")), None);
        assert_eq!(workdir(&root, Some(&std::env::temp_dir().to_string_lossy())), None);
        assert_eq!(workdir(&root, Some("no-such")), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_log_line_has_the_first_80_characters_alone() {
        let long = "字".repeat(200);
        assert_eq!(first_words(&long).chars().count(), 81);
        assert_eq!(first_words("fix   the\nbug"), "fix the bug");
    }
}
