<h1 align="center">Wakuwaku</h1>

<p align="center"><strong>A little island at the top of your screen that knows all your AI sessions. She also happens to be a pet.</strong></p>

<p align="center">
  <a href="https://github.com/zhoupengjie/Wakuwaku/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/zhoupengjie/Wakuwaku?style=flat-square&amp;label=release&amp;color=ff7eb3"></a>
  <img alt="Windows 11" src="https://img.shields.io/badge/Windows-11-0078d4?style=flat-square">
  <img alt="About 11 MB" src="https://img.shields.io/badge/one%20exe-about%2011%20MB-8b5cf6?style=flat-square">
  <a href="LICENSE"><img alt="MIT" src="https://img.shields.io/badge/license-MIT-34d27b?style=flat-square"></a>
</p>

<p align="center"><strong>English</strong> | <a href="README.md">简体中文</a></p>

<p align="center">
  <a href="#install">Install</a> ·
  <a href="#use">Use</a> ·
  <a href="#how-it-works">How it works</a> ·
  <a href="https://codex-pets.net">Another pet</a>
</p>

https://github.com/user-attachments/assets/7814d71a-77f0-43af-97a6-210d2cbb31dc

<p align="center"><sub>The video's captions are in Chinese.</sub></p>

> The name is Anya's "waku waku" from SPY×FAMILY: excited, can't wait. AI works, and she watches, all eager.

## What she can do

- **She knows all your AI sessions.** What Claude Code, Codex, Antigravity and Grok Build are doing, and for how long, right in the island at the top of the screen. Several at once, each kept apart.
- **Needs your OK? Click it on the island.** Allow, always allow, deny; answer Claude's questions; approve a plan. The terminal asks too, and whichever answers first counts.
- **Done, and she tells you.** How many files changed and what Claude said. Click a session to go straight to its window: a terminal, VS Code, or the Claude desktop app.
- **Plugins.** With no session at work, the island takes turns with today's usage, tokens, the weather, a pomodoro, a countdown; a system monitor stays put. One line of PowerShell makes a plugin of your own, and `waku <command>` tells you when a command is done.
- **Make it look your way.** A preset, or the island's own colour, opacity, blur and edge.
- **Any pet you like.** Search [codex-pets.net](https://codex-pets.net) and download one. Pull her down out of the island and she hops onto your desktop; double-click her and she flies home.

About 11 MB, one exe, nothing to install, free and open source. Next (planned): letting AI work your desktop, and raising your pet.

| Done, and she tells you | Plugins: on, off, set up, your own |
| --- | --- |
| [![A finished session open in the island: what changed, Claude's reply, and the two other sessions below](docs/screenshots/done.jpg)](docs/screenshots/done.jpg) | [![The Plugins page: monitor, today, tokens, weather, pomodoro, countdown](docs/screenshots/plugins.jpg)](docs/screenshots/plugins.jpg) |
| **The island, the way you like it** | **Her home at the top of the screen** |
| [![The Look page: the island's presets, colour and opacity](docs/screenshots/look.jpg)](docs/screenshots/look.jpg) | [![The island at the top of the screen, CPU, memory and network pinned at its right](docs/screenshots/island.jpg)](docs/screenshots/island.jpg) |

<sub>The projects and web pages in these pictures are made up for the demo. The pets come from codex-pets.net and belong to their authors: Claude小姐 (original character by ZipZipPipe, pixel fan art by zhoupengjie), 大肥鱼 (Dullsaw), GPT娘 (David).</sub>

## Her animations

Each pet has 11 animations, and every one of them has a job:

| Animation | When | From which hook |
| --- | --- | --- |
| Idle | Nothing going on (a blink, then a few still seconds) | After you've seen an ending (mouse over her); `SessionEnd` |
| Running | Claude is working; the island shows how long this turn has taken | `UserPromptSubmit`, `PreToolUse`, `PostToolUse` |
| Waiting | Waiting on you: a permission prompt, a question, a plan to approve, an MCP form | `PermissionRequest`; `PreToolUse` (AskUserQuestion / ExitPlanMode); `Elicitation` |
| Review | The turn edited files and is done, ready for you to look | `Stop` (after Edit / Write and friends ran this turn) |
| Waving | The turn is done without edits; hello when a session starts or she appears | `Stop`, `SessionStart` |
| Jumping | A task on the to-do list is done; you clicked her | `TaskCompleted` |
| Failed | A request failed (stays until you've seen it); a tool call failed (a quick flash) | `StopFailure`; `PostToolUseFailure` |
| Run right / left | A short stroll when idle; running the way you drag her | — |
| Look around (16 directions) | Her eyes follow the mouse when idle; now and then she glances around | — |

- **Several Claude Code sessions at once**: each keeps its own state. She shows the one that most needs you (waiting > error > review > done > working) and tells you how many others are busy; when another one is waiting on you, the bubble's second line names it. Sessions left idle too long stop piling up: one at rest (a finished one from when you saw it) with nothing new for 30 minutes leaves the "+N", the open island and the settings' list of sessions; its next event brings it straight back, state, name and today's counts intact. Change the time under Idle sessions on the General page: 15 or 30 minutes, 1 or 2 hours, or always show. Sessions working or waiting on you always show.
- **Where it is, at a glance**: a session goes by its name in Claude Code (from `/rename` or the desktop app's title; else the first thing you asked). While it works she says the step it is on: "Editing island.rs", "$ cargo test", "Agent: explore the code", "Thinking", with "for 1:12" once a step runs long, and "3/7" and a progress bar when it keeps a to-do list. Waiting, she says on what ("Needs your OK: $ git push"); done, how many files changed and a line of Claude's reply; failed, why ("Usage limit reached"). The project is the git repository's name, worktrees included. When sharing your screen, turn off Show the specifics on the Look page and only the project and the status show.
- **Endings don't slip by**: done, review and error stay until the mouse passes over her, or for a few seconds if you prefer. System notifications and a sound are optional.
- **Context compaction** also fires `SessionStart`; she ignores it and doesn't drop back to idle mid-work.

## Answer Claude on the pet

| What | What the panel lets you do |
| --- | --- |
| Permission ("Allow Bash to run npm test?") | **Allow** / **Always allow** / **Deny**, with the command and project shown |
| A question (AskUserQuestion) | Pick options, type your own answer, fill text and number questions; several questions and multi-select work |
| A plan (ExitPlanMode) | **Approve** / **Deny** |

- The terminal's own dialog still appears, and **whichever answer comes first wins**: answer on the pet and the terminal dialog closes; answer in the terminal and the panel goes away.
- **Always allow** adds the rules Claude Code itself suggests (the same as "don't ask again" in the terminal), and the panel spells out which rule goes where. It only ever adds allow rules.
- Buttons stay disabled for 0.6 s after a panel appears, so a click already on its way can't hit them. To type, click the box first; she takes the keyboard only then, never from the window you're typing in.
- How long the panel waits is up to you (30 s to 5 min); after that the terminal has it. In do-not-disturb, or while an app is full screen, prompts go straight to the terminal.
- Limits:
  - After you answer in the terminal, the panel closes on that session's next event, so it can linger for a moment. Clicking it then does nothing.
  - Non-interactive runs such as `claude -p` wait for the hook before deciding, so with nobody clicking they wait until the panel gives up.
  - MCP forms (Elicitation) are announced, not filled in.
- Codex's permission prompts come to the panel too, with **Allow** / **Deny** only (Codex takes no rules from a hook). Codex shows its own dialog only once the panel lets go, so the panel waits 1 minute at most; to answer in the terminal, click Handle in terminal.
- When Grok Build asks for your OK, the island says "Needs your OK: …", but **you answer in Grok**: a Grok hook's allow only means "not blocked" and can't approve for you, so Grok still shows its own prompt (as xai-org/grok-build's source and tests have it).

## Install

### 1. Get the pet

- **One exe (Windows)**: `wakuwaku.exe` is about 11 MB (3 of them the default pet), runs from any folder and keeps her settings and pets in a `wakuwaku-data` folder beside it; to remove her, delete the two. It uses the WebView2 that comes with Windows 11, nothing else to install.

Download `wakuwaku-<version>-windows-x64.exe` from [Releases](https://github.com/zhoupengjie/Wakuwaku/releases) (in China, the same file is on [Gitee](https://gitee.com/zhoupengjie/Wakuwaku/releases)); there is nothing to install. The exe is not code-signed, so on first run Windows SmartScreen may stop it: click More info → Run anyway. Or build it yourself, see below.
- **From source**: see below.

On first launch she downloads the default pet, Claude小姐 (online; offline, the settings open by themselves so you can pick one later). For another, click the island to open the settings and pick one on the Pets page (the gallery comes straight from codex-pets.net; click Download), or paste a pet page URL.

### 2. Connect Claude Code (the plugin is recommended)

In Claude Code, enter (the Connect page of the settings has copy buttons):

```
/plugin marketplace add zhoupengjie/wakuwaku
/plugin install wakuwaku@wakuwaku
```

Claude Code installs and removes the plugin itself, and **we don't touch your settings file** (Claude Code notes it in its own `enabledPlugins` and takes it out on uninstall). A plugin can't start the pet, so turn on Start at login on the Connect page.

If you'd rather not use a plugin, Advanced on the Connect page writes the hooks into `~/.claude/settings.json` (backed up first; removing takes out only our entries). In return, new sessions start the pet. With both on, the Connect page warns that events arrive twice.

### 3. Connect Codex (optional)

Click Install in the Codex part of the Connect page. It writes `~/.codex/hooks.json` (backed up first; removing takes out only our entries). Then **enter `/hooks` in Codex and trust Wakuwaku's hooks**: Codex runs only hooks you trusted, and the Connect page reminds you until the first event arrives. If the app moves, click Repair and trust them again.

How it differs from Claude Code:

| | Claude Code | Codex |
| --- | --- | --- |
| Working, waiting, done, review | ✓ | ✓ (a turn that ran `apply_patch` ends in review) |
| Errors | ✓ | No (Codex sends no event when a request fails; she goes back to idle after 15 minutes) |
| A jump when a to-do item is done | ✓ | No |
| Answer on the panel | Allow / Always allow / Deny, questions, plans | Allow / Deny |
| New sessions start the pet | With the settings.json way | Yes |

Codex hooks can only be commands: `wakuwaku.exe --wakuwaku-codex-hook` runs once and hands the event to her. On Windows Codex runs hooks in PowerShell, about 0.3 s to start each time, so Codex waits for it only when a turn starts and ends, on an edit (`apply_patch`), on a prompt, and when a session starts and ends; each step's tool is reported in the background. Background hooks need Codex 0.148 or later; an older one gets only the essential hooks, and the bubble doesn't name each tool.

The pets in Codex's desktop app (`~/.codex/pets`) use the same format, so they show up on the Pets page, marked From Codex.

### 4. Connect Antigravity (optional)

Google's Antigravity is supported: the `agy` CLI and the Antigravity 2.0 desktop app. Click Install in the Antigravity part of the Connect page. It adds a block named `wakuwaku` to `~/.gemini/config/hooks.json` (backed up first; removing takes out only that block, other tools' hooks stay). Then open a new agy session (or restart Antigravity 2.0). The standalone Antigravity IDE runs no hooks for now, so it can't be connected.

How it differs from Claude Code: Antigravity has no "needs your approval" event, so she follows its sessions starting, working, using tools, finishing and failing, but can't remind you to approve, and you can't approve on her.

Antigravity hooks can only be commands, run by `cmd` without reading quotes: if the app's path has a space, its 8.3 short path is written instead; with no short path either (some drives have short names turned off), installing fails, and moving the app to a folder with no spaces in its path fixes it. agy waits for the hook at each step, about 0.1–0.2 s.

### 5. Connect Grok Build (optional)

Click Install in the Grok Build part of the Connect page. It writes `~/.grok/hooks/wakuwaku.json` (Grok reads that folder by itself, with no path to add in `/hooks`; the whole file is ours, and removing deletes it). Restart a Grok session already open, or press `r` in `/hooks` to reload.

How it differs from Claude Code:

- Grok has no "needs your approval" event, and a hook can't approve for you. She learns from Grok's `Notification` (`permission_prompt`) that Grok is waiting on you and the island says "Needs your OK: …", but you answer in Grok.
- Grok also runs Claude Code's hooks (the plugin and `~/.claude/settings.json`), so each event comes twice. With Grok's own file in, she drops the Grok events that come through Claude Code's; without it, she takes them as Grok's.
- Today's approvals leave Grok out (as with Antigravity, its approvals never come to her).

### From source

Needs Rust (the MSVC toolchain on Windows). Only tested on Windows 11 so far; Tauri builds on macOS and Linux too, but full-screen detection, start at login and notifications are written for Windows only.

```bash
git clone https://github.com/zhoupengjie/wakuwaku.git
cd wakuwaku/src-tauri
cargo run                 # debug
cargo build --release     # target/release/wakuwaku.exe, one file
```

No Node and no Tauri CLI needed. The first run downloads the default pet; connect Claude Code from the Connect page of the settings. Run from source, her settings, downloaded pets and log live in the project's `data/` folder (not in git).

## Use

- **Drag** her anywhere; she runs the way you drag, and remembers the spot.
- **Click** her: she jumps, and the window of the session she shows comes to the front. **Double-click**: out on the desktop, she flies back home.
- **A session takes you to its window**: click a session in the open island, a row on the Now page of the settings, or Handle in terminal on a prompt, and the window it runs in comes to the front: Windows Terminal, a console window, VS Code, or the Claude desktop app (which opens that very session). When the session's process has ended, the island says it can't find its window. Windows Terminal comes to the front as a window, not at the tab.
- **Plugins in the island**: while no session needs you, plugins take turns in the island, like "Today 12 turns · 1:43 · 5 approved", or the weather, or a stock; sessions always come first. Hover to see them all, scroll to turn to the next, click one to show it and open its settings. Built in: Today (turns, how long the AI worked, in seconds under a minute, and approvals given on her; Antigravity's approvals never come to her, so a day of its sessions alone shows "—" for them) and Today's tokens (Claude Code's, Codex's and Antigravity's), on by default; and the Monitor, off: CPU, memory, network and battery, each with a switch, shown in the island as icons and numbers, updated every 1 to 10 seconds (2 by default). She can also run some for you in the background: weather, stocks/rates/coins, a pomodoro, a countdown, a stretch reminder, GitHub CI and dev servers; switch one on in the Plugins page and give it a city or a repository, no terminal needed, and they start with her. Any other is a script sending a few words to `127.0.0.1:47213/widget`. [examples/widgets](examples/widgets) has `waku <command>` (tells you when it is done), CI and pull requests, a pomodoro, deadlines, dev servers, stocks/rates/coins, weather and a stretch reminder. A private widget shows only its count while Show the specifics is off. A plugin may open the island once to tell you something (before a meeting, say), but never while a session waits on you. The Plugins page of the settings turns them on and off, orders them, and sets how often they take turns.
- **Right-click** her (or the tray icon): switch pets, size, walking, follow the mouse, let her out or call her back home, back to the corner, do not disturb, settings, quit. Left-click the tray icon to show or hide her (in do not disturb, the settings). The tray face changes with her mood: idle, working, waiting, done, review, error.
- **The island**: her home is a small black island at the top centre of the screen, like the iPhone's Dynamic Island. Black by default; under Look its ground, opacity and edge can change too, or a preset (Windows dark grey, Frosted Mica…); see-through, Windows 11 blurs the windows behind it, and where it can't, the ground deepens over a maximized window. Closed, it has her at the left, a line of words in the middle and the time or a widget's number at the right edge, at a width that stays (narrow, normal or wide, in the settings) whatever it shows. The Monitor is pinned by default: the island grows a little to the right and keeps CPU, memory and the network in sight there while the middle takes its turns; unpin it in its settings and it takes turns like any other widget. Most of the time it shows her round portrait, ringed in the colour of her mood, with the step the session is on (or, once it is done, whose it is) and the time. When something happens (a prompt, a turn done, changes to review, an error) the island opens by itself and the portrait grows into her whole self, standing inside and playing that mood, beside the session's name, where it is and Claude's reply, with a line below for each other session, then shrinks back after a few seconds; a prompt opens it into a panel with her beside it. Hovering opens it too. When other sessions are busy too, a small "+1" sits by the time.
- **Letting her out, and back in**: press her in the island and pull down. The island stretches like a drop of ink with her in it; pull far enough and the drop pinches off and she follows the cursor anywhere, landing where you let go. The island stays while she is out, her seat empty, still showing the status, the time and any prompt (on the desktop she only acts, she does not talk). To bring her home, drag her under the island: it reaches out a drop for her, and letting go close enough draws her back in; or double-click her and she flies back by herself. The right-click menu has Let her out and Call her back in too.
- **Settings grow out of the island**: there is no main window. Click the island and it opens into the settings; only then does it take the keyboard. Hovering, opening by itself or a prompt arriving never take it, so nothing you type in the terminal is lost. Esc, a click outside, ✕ or a click on the head close them, and the keyboard goes back where it was. Three pages: General (sessions, three quick switches, alerts, start at login and language, updates, crash reports, how Claude Code, Codex and Antigravity reach her), Look (the island, her size and ways, and the pets: yours, downloads, the codex-pets.net gallery) and Plugins; ← → switch pages. A prompt arriving while they are open shows as a banner below the pages, with a dot on General.
- Clicks pass through her transparent parts to whatever is underneath.
- **Lost her?** Start the app again and she comes back to the bottom right. She also checks every 2 s and walks back if she ends up off screen.

### Another pet

Paste its URL (like `https://codex-pets.net/#/pets/deepseek-chan`) on the Pets page of the settings, or just its id (the URL's last part); or click Download in the gallery below. Only codex-pets.net URLs are taken, and only that site is downloaded from.

### Wallpapers

The repository's [`wallpapers/`](wallpapers/) folder has five desktop wallpapers (the original PNGs, 3072×1728, 6.5–7.4 MB each). The app doesn't download or set them; to use one, download it and pick it in Windows' Settings → Personalization → Background.

### Off for a while

| You want | Do |
| --- | --- |
| Off for now | Right-click → Quit. Claude Code and Codex sessions no longer bring her back: open her yourself, or wait for the next start at login (only after a crash does a session start her again) |
| Some quiet | Right-click → Do not disturb: she hides, no notifications, prompts go to the terminal |
| No automatic start, only when I open her | Connect with the plugin (it never starts her) and turn off Start at login; open her yourself |

While she's closed, Claude Code works as usual: the events sent to her are refused at once (about 1 ms) and nothing waits.

### League of Legends game helper

While you play League of Legends, each pet out on the desktop (turn on "On the desktop" for its agent on the Look page) takes a part: Claude's shows the game clock and an objective about to spawn ("Dragon respawns in 0:42"); Grok's shows the Dragon, Baron and Herald respawn countdowns and the summoner spells your team's five took; Codex's shows the other team's key items as they get them ("Their Zed has Zhonya's Hourglass"), and your own CS (a minute), KDA and gold as you buy something or die. In champion select, Codex's shows your rank and your party's (put your own personal Riot API key on the General page; players not in your party are not looked up). The agents' notifications go on as ever. Once the game is over, all is as it was.

- **Riot's own ways in only**: in a game she reads only the Live Client Data API the game itself opens on this computer (`127.0.0.1:2999`). Never the game's memory, no injecting, nothing done for you, no reading the screen.
- **Neutral information only**: as Riot's developer policy asks (no information not in the game that gives an edge, no deciding for the player), no enemy summoner spell or ultimate timers, and no "ward now" or "group up".
- **Play in borderless mode** (game settings → Video → Window mode → Borderless). Full screen covers every other window, the pets with them.
- **The full-screen exception**: with "Hide while an app is full screen" on, League of Legends in a game does not count as full screen, so the pets stay; other full-screen apps hide them as before.
- Not wanted: turn off "League of Legends game helper" on the General page.

### Updates

The downloaded exe looks at the latest release on GitHub and on Gitee as it starts (GitHub's `https://api.github.com/repos/zhoupengjie/Wakuwaku/releases/latest`; on Gitee, the repo's tags and the `.sha256` file in a release; with no account of yours), then once a day. Drafts and prereleases don't count. A new version is told on the island once; click it for Updates on the General page of the settings, where you can read its notes. Nothing is downloaded until you click Update.

- What's downloaded is the release's `wakuwaku-v<version>-windows-x64.exe`: from Gitee first in China, from GitHub first elsewhere, and from the other if that fails (Download from, under Updates, picks one).
- It's checked against the SHA-256 GitHub gives for it, or, when GitHub gives none, the `.sha256` file in the Gitee release. If it doesn't match, none is given, or the two differ, it isn't installed and no file is touched.
- Once it checks out, your exe is renamed `<its name>.old.exe` and the new one put in its place (your file name kept); the new one starts and the old one quits, after any prompt waiting for you is answered. Once the new one has run a minute, `.old.exe` is deleted.
- If the new one won't start, `.old.exe` is still there: quit her, delete the new exe and rename `.old.exe` back to the name it had.
- To stop the checks: turn off Check for updates there, and click Check now when you want to.
- Run from source, she never checks or updates: building it again is the update.

### Crash reports

After she quits unexpectedly (a crash), the island asks you once, at her next start, whether to send crash reports automatically from now on. Click it to see everything that would be sent, then choose Send from now on or Don't send. You can switch it any time under Crash reports on the General page of the settings.

- A report is posted **publicly** as an issue: on Wakuwaku on Gitee in China, on GitHub elsewhere. One crash gets one issue; the same crash again only adds to its count.
- It holds her version, what crashed and where, Windows' and WebView2's versions, how long she had run, and the last few minutes of her log.
- Taken out before it's sent: user and computer names, paths (as `%USERPROFILE%`), other programs' names (only Explorer and her own are kept), window titles, e-mail addresses, session and project names, anything that looks like a token, and every word in Chinese or any other non-English script.
- There's no account or token in the exe: reports go to a relay (its code is in `relay/`), which opens the issue and limits how many one address can send.
- A report that can't be sent stays in `crashes/` in her data folder and is tried again later. Run from source, she only keeps them, and never asks or sends.

### Uninstall

1. If you turned on **Start at login**, turn it off on the Connect page.
2. With the plugin: in Claude Code, `/plugin uninstall wakuwaku@wakuwaku`, then `/plugin marketplace remove wakuwaku`. With hooks in settings.json: click Remove on the Connect page; only this project's entries go, your own hooks stay, and the file as it was before the first change is at `~/.claude/settings.json.wakuwaku.bak`. With Codex: click Remove in its part of the Connect page; the backup is `~/.codex/hooks.json.wakuwaku.bak`. With Grok: click Remove in the Grok Build part; it deletes `~/.grok/hooks/wakuwaku.json`.
3. Quit her, then delete the exe or the project folder. Notifications register her name in `HKCU\Software\Classes\AppUserModelId\com.zhoupengjie.wakuwaku`; delete that key too.
4. Delete her settings and downloaded pets: the `wakuwaku-data` folder beside the exe, or the project's `data/` when run from source.

> From source, remove the hooks before deleting the folder: they hold the program's path. If you moved the folder, settings will say the hooks point somewhere else; click Repair.

## How it works

```
Claude Code ──HTTP hooks──▶ the pet (127.0.0.1:47213/hook)
            └─SessionStart─▶ the app itself --wakuwaku-ensure-running (in the background; starts her if she's not up)
Codex ──command hooks──▶ the app itself --wakuwaku-codex-hook ──▶ the pet (/hook?agent=codex)
Grok Build ──HTTP hooks──▶ the pet (/hook?agent=grok)
```

- Every event is a Claude Code **HTTP hook** sent straight to her, so a tool call starts no process (about 0.3 ms here). She answers `{}` at once, meaning no decision; only a prompt waits until you answer on the panel.
- Claude Code runs no HTTP hook for `SessionStart`. The settings.json way gives it a background command instead: the app itself, which passes the event on if she's up (a hello) or starts her if not. No Node needed. The plugin has no such command; she starts at login.
- At rest she takes about 1% of one core (measured on Windows 11) and about 185 MB of private memory, mostly WebView2 (that is, Chromium) itself.

| Variable | Does |
| --- | --- |
| `WAKUWAKU_PORT` | Another port (default 47213); set it for both her and the hooks |
| `CLAUDE_CONFIG_DIR` | Claude Code's config folder, when it isn't `~/.claude` |
| `CODEX_HOME` | Codex's config folder, when it isn't `~/.codex` |
| `GROK_HOME` | Grok Build's folder, when it isn't `~/.grok` |

## Tests

```bash
cd src-tauri && cargo test     # Rust: hook mapping, the multi-session state machine, prompt replies and queueing, hooks install, pet URLs...
node --test "test/*.test.js"   # the page: i18n, 16 look directions (no npm install needed)
```

See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) (in Chinese) for development notes.

## License and credits

- The code is under the [MIT License](LICENSE).
- The pet sprites come from [codex-pets.net](https://codex-pets.net) and belong to their authors. **Except the default pet, they are not in this repository or the exe**; you download them in the settings.
- The default pet, [Claude小姐](https://codex-pets.net/#/pets/claude-chan), is pixel art by zhoupengjie: an unofficial fan work of the Claude小姐 character by Bilibili creator [ZipZipPipe](https://space.bilibili.com/4168597) (the character's original creator). It is in this repository (`pets/claude-chan/`) and carried in the exe: the first run puts her in place with nothing to download.
- The 16 look directions follow codex-pets.net's mapping (clockwise from straight up, one step per 22.5°).
