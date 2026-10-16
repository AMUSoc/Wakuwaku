// A new conversation with an agent, from its pet: a new tab of Windows
// Terminal (wt.exe; none, a PowerShell window) running the agent's own
// command in its project folder (settings agentDirs; none, or gone, the
// home folder); or its website in the default browser.
//
// WAKUWAKU_LAUNCH_DRY (tests): what would run is logged, nothing started.
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::Shared;

// The command each agent's terminal tool is started by.
pub fn command_for(agent: &str) -> &'static str {
    match agent {
        "codex" => "codex",
        "grok" => "grok",
        "agy" => "agy",
        _ => "claude",
    }
}

// Each agent's own website.
pub fn web_for(agent: &str) -> &'static str {
    match agent {
        "codex" => "https://chatgpt.com/codex",
        "grok" => "https://grok.com",
        "agy" => "https://gemini.google.com",
        _ => "https://claude.ai",
    }
}

// Where the agent starts: its project folder as set, if it is there; else home.
pub fn dir_for(dirs: &Value, agent: &str, home: &Path) -> PathBuf {
    dirs.get(agent).and_then(Value::as_str).map(str::trim).filter(|d| !d.is_empty()).map(PathBuf::from).filter(|d| d.is_dir()).unwrap_or_else(|| home.to_path_buf())
}

// Windows Terminal's arguments: a new tab in its window there is (-w 0), in
// that folder, titled for the agent, a PowerShell that runs its command and
// stays (the command a shim, claude.cmd: not run by itself).
pub fn terminal_args(dir: &Path, agent: &str) -> Vec<String> {
    ["-w", "0", "new-tab", "-d"]
        .iter()
        .map(|s| s.to_string())
        .chain([dir.to_string_lossy().into_owned(), "--title".into(), command_for(agent).into(), "powershell.exe".into(), "-NoExit".into(), "-Command".into(), command_for(agent).into()])
        .collect()
}

fn home() -> PathBuf {
    std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\"))
}

// Windows Terminal, where its alias is (the WindowsApps folder) or on the PATH.
fn wt() -> Option<PathBuf> {
    let apps = std::env::var_os("LOCALAPPDATA").map(|l| PathBuf::from(l).join("Microsoft").join("WindowsApps"));
    let path = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect::<Vec<_>>()).unwrap_or_default();
    apps.into_iter().chain(path).map(|d| d.join("wt.exe")).find(|p| p.is_file())
}

fn dry(sh: &Shared, what: &str) -> bool {
    let is_dry = std::env::var_os("WAKUWAKU_LAUNCH_DRY").is_some();
    if is_dry {
        sh.log(&format!("launch (dry): {what}"));
    }
    is_dry
}

// A new terminal of the agent's, in its folder. True when it was started.
pub fn terminal(sh: &Shared, agent: &str) -> bool {
    let dir = dir_for(&sh.setting("agentDirs"), agent, &home());
    let cmd = command_for(agent);
    if let Some(wt) = wt() {
        let args = terminal_args(&dir, agent);
        if dry(sh, &format!("{} {}", wt.display(), args.join(" "))) {
            return true;
        }
        let ok = std::process::Command::new(&wt).args(&args).spawn().is_ok();
        sh.log(&format!("launch: {agent} in a new tab of Windows Terminal, in {}: {}", dir.display(), if ok { "started" } else { "failed" }));
        if ok {
            return true;
        }
    }
    if dry(sh, &format!("powershell.exe -NoExit -Command {cmd} (in {})", dir.display())) {
        return true;
    }
    let mut command = std::process::Command::new("powershell.exe");
    command.args(["-NoExit", "-Command", cmd]).current_dir(&dir);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_CONSOLE: u32 = 0x10;
        command.creation_flags(CREATE_NEW_CONSOLE);
    }
    let ok = command.spawn().is_ok();
    sh.log(&format!("launch: {agent} in a PowerShell window, in {}: {}", dir.display(), if ok { "started" } else { "failed" }));
    ok
}

// The agent's website, in the default browser.
pub fn web(sh: &Shared, agent: &str) -> bool {
    let url = web_for(agent);
    if dry(sh, &format!("open {url}")) {
        return true;
    }
    // The shell's own way to open a link in the default browser, no window of its own.
    let ok = std::process::Command::new("rundll32.exe").args(["url.dll,FileProtocolHandler", url]).spawn().is_ok();
    sh.log(&format!("launch: {url} in the browser: {}", if ok { "opened" } else { "failed" }));
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn each_agent_s_command_website_and_folder() {
        assert_eq!([command_for("claude"), command_for("codex"), command_for("grok"), command_for("agy")], ["claude", "codex", "grok", "agy"]);
        assert_eq!(web_for("codex"), "https://chatgpt.com/codex");
        assert_eq!(web_for("agy"), "https://gemini.google.com");
        let home = std::env::temp_dir();
        let here = std::env::current_dir().unwrap();
        let dirs = json!({ "codex": here.to_string_lossy(), "grok": "Z:\\no\\such\\folder", "agy": "  " });
        // Set and there: it; set and gone, blank or not set: home.
        assert_eq!(dir_for(&dirs, "codex", &home), here);
        assert_eq!(dir_for(&dirs, "grok", &home), home);
        assert_eq!(dir_for(&dirs, "agy", &home), home);
        assert_eq!(dir_for(&dirs, "claude", &home), home);
        // Windows Terminal: a new tab in that folder, a PowerShell running the command.
        let args = terminal_args(Path::new("D:\\work\\my app"), "codex");
        assert_eq!(args, ["-w", "0", "new-tab", "-d", "D:\\work\\my app", "--title", "codex", "powershell.exe", "-NoExit", "-Command", "codex"]);
    }
}
