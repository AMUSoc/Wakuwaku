# CLAUDE.md

Wakuwaku 是一個 Windows 11 的桌面寵物兼任務列：用 Claude Code、Codex、Antigravity 的 hook 事件顯示每個 AI 會話在做什麼，並能在寵物身上回答權限確認。技術上是 **Tauri 2**：頁面是 `src/` 的 HTML / JS / CSS，由系統的 WebView2 顯示；其餘全在 `src-tauri/` 的 Rust 裡。（2026-10 之前是 Electron 版，在 git 歷史裡。）

更完整的開發說明（簡體中文）在 `docs/DEVELOPMENT.md`，使用者文件在 `README.md` / `README.en.md`。本檔只整理架構與常用指令；細節以那兩份和程式碼為準。

## 建置與測試

```bash
cd src-tauri
cargo run                 # 除錯版，有主控台視窗
cargo build --release     # target/release/wakuwaku.exe：單一檔案，沒有主控台
cargo test                # Rust 單元測試（hook 對應、多會話狀態機、確認的回覆與排隊、hooks 安裝、寵物網址……）
cd .. && node --test "test/*.test.js"   # 頁面的單元測試（i18n、注視方向、status、widgets…），不需要 npm install
```

- 頁面是打包進 exe 的（`Cargo.toml` 預設開啟 `custom-protocol`）：改了 `src/` 要重新編譯，cargo 會自己發現。**不需要 Node 和 Tauri CLI** 就能建置。
- 改了 `settings.js`、`i18n.js` 等頁面腳本時，可以用 `node --check <檔案>` 先檢查語法。
- 搬動專案資料夾後要 `cargo clean` 一次（`target/` 裡有絕對路徑）。
- 從原始碼執行時，設定、寵物、日誌都在專案的 `data/`（不進 git）；資料夾裡放一個 `debug.on` 就會寫 `debug.log`（含兩個頁面的 console.log）。

常用環境變數：`WAKUWAKU_PORT`（預設 47213）、`WAKUWAKU_USER_DATA`（換資料目錄，測試用）、`CLAUDE_CONFIG_DIR`、`CODEX_HOME`（測試時指向暫存目錄，不碰真實設定）、`WAKUWAKU_DEBUG=1`（開放 `/debug/eval`、`/debug/walk`）、`WAKUWAKU_IDLE_HIDE_MS`、`WAKUWAKU_QUIET_AFTER_MS`（測試時代替要等很久的設定）。

## 目錄

```
src/pet/           頁面：她的視窗（?role=pet）和島 / 任務列（?role=island）共用 index.html；bar.html 是其他螢幕上的任務列
src/shared/i18n.js 中英文字串（Rust 端少數字串在 src-tauri/src/i18n.rs）
src-tauri/src/     Rust 主程式（下表）
src-tauri/capabilities/  頁面可用的 Tauri 權限：新視窗一定要列進去
integrations/claude-code/  Claude Code 外掛 wakuwaku（只有 HTTP hooks）；.claude-plugin/ 是外掛市集入口
integrations/thunderbird/  Thunderbird 擴充
examples/widgets/  島上外掛的範例腳本
test/              頁面的 node --test 測試，以及假的 IMAP / SMTP / 中繼伺服器
relay/             當機報告中繼（Cloudflare Worker、阿里雲函式計算）
```

## Rust 模組的職責

**入口與共用狀態**
- `main.rs`：入口。依參數分流成 `--wakuwaku-ensure-running`（Claude Code 的 SessionStart）、`--wakuwaku-codex-hook`、`--wakuwaku-agy-hook <事件>`、任務列守護程序；否則以「誰佔著連接埠誰就是寵物」做單實例，建立 `Shared`（所有部分共用的狀態，**任何時候都不同時持有兩把鎖**），註冊頁面可呼叫的 Tauri 命令，並啟動 pointer、widgets、monitor、fullscreen、ticker 等背景執行緒。
- `server.rs`：127.0.0.1:47213 的 HTTP 介面（`/hook`、`/state`、`/widget`、`/health`、`/come-home`、`/quit`、`/debug/*`）。
- `state.rs`：狀態機。按會話記 mood、名字、執行中的工具、任務清單、改過的檔案、結果，挑出最需要你的那一個顯示；也負責閒置會話的隱藏（`is_shown`）與遺忘。
- `events.rs` / `events_codex.rs` / `events_agy.rs`：把 Claude Code / Codex / Antigravity 的 hook 事件換算成統一的「訊息」；會話名、專案名（git 倉庫名，含 worktree）。
- `asks.rs`：確認請求（PermissionRequest）：顯示什麼、每個選擇回給 agent 什麼、排隊與逾時。
- `data.rs`：資料目錄、設定讀寫、已下載的寵物和 Codex 的寵物、「使用者主動退出」標記。
- `settings.rs`：設定頁的快照、白名單校驗後的設定補丁、設定相關命令。
- `connection.rs`：外掛偵測、寫入 `~/.claude/settings.json` 的 hooks、Codex 的 `hooks.json`、Antigravity 的 `hooks.json`（安裝 / 移除 / 狀態），開機自啟。

**視窗與家**
- `pet.rs`：她的視窗：大小位置、眼睛、拖動與被攜帶、走動、頭頂的確認面板、飛回島裡。
- `island.rs`：她的家的視窗（頂端的靈動島，或取代 Windows 的任務列），依頁面要求擴大、把她吸回去、為設定臨時升起。
- `bars.rs`：其他螢幕上的任務列。`notch.rs`：macOS 瀏海裡的島。`backplate.rs`：島下面的壓克力模糊。
- `pointer.rs`（點擊穿透）、`screen.rs`（各螢幕工作區）、`focus.rs`（鍵盤焦點的取與還）、`fullscreen.rs`（別的程式是否全螢幕）。

**任務列模式（Windows 端）**
- `taskbar.rs`：自己的執行緒和訊息迴圈：藏起 Windows 的任務列、接管系統匣、視窗按鈕、縮圖、工作區自癒。
- `appbar.rs`（AppBar 佔位）、`shell.rs`（藏 / 還 Windows 任務列、守護程序）、`systray.rs`（接管系統匣）、`tasks.rs`（該有按鈕的視窗、程式圖示、啟動程式）、`pins.rs`（讀一次 Windows 的釘選）、`winkeys.rs`（Win+數字 / Win+T / Win+B 的低階鍵盤鉤子）、`inputs.rs`（輸入法）、`clock.rs`、`audio.rs`（音量）、`wallpaper.rs` / `paper.rs` / `theme.rs`（桌布、雲母、深淺色與強調色）、`icons.rs`（桌面圖示讓位）。

**其他**
- `jump.rs`：點會話跳到它的視窗（行程鏈、找視窗、叫到前面、Claude 桌面版的會話連結）。
- `widgets.rs`（島上的外掛：腳本送來的、內建的今天 / token / 監控）、`scripts.rs`（她代跑的外掛腳本）、`tokens.rs`（讀三種 agent 自己的紀錄算今天的 token）。
- `mail.rs`、`mail/`：郵件（IMAP / SMTP、交給 AI 處理）。`proxy.rs`：系統代理。
- `tray.rs`（系統匣圖示與選單）、`notify.rs`（toast 通知）、`updates.rs`（自動更新）、`crash.rs`、`crash/`（當機報告）、`power.rs`（省電、EcoQoS）、`gpu.rs`（WebView2 用哪張顯卡）、`detach.rs`（以檔案總管為父行程啟動自己，脫離 agent 的 job）、`fetch.rs`（codex-pets.net 下載寵物）、`i18n.rs`。

**頁面（`src/pet/`）**
- `bridge.js`：`window.pet`，頁面和 Rust 之間的呼叫與事件；所有監聽註冊完才送 `pet_ready`，並把 invoke 排隊依序送出。
- `pet.js`（動畫、走動、東張西望）、`island.js`（島與任務列的各種形狀）、`settings.js`（長在島裡的設定）、`panel.js`（確認面板）、`status.js`（把會話說成一句話）、`sprite.js`（圖集與 16 個注視方向）、`widgets.js`、`mica.js`、`presses.js`。

## Hook 事件的流向：hook → server.rs → state.rs → 前端

```
Claude Code ──HTTP hook（POST /hook?from=wakuwaku）────────────────┐
           └─SessionStart：wakuwaku.exe --wakuwaku-ensure-running ─┤  （沒在跑就以獨立行程啟動她，否則轉送事件）
Codex ──command hook：wakuwaku.exe --wakuwaku-codex-hook ──────────┤  POST /hook?from=wakuwaku&agent=codex
Antigravity ──command hook：wakuwaku.exe --wakuwaku-agy-hook <名> ─┤  POST /hook?from=wakuwaku&agent=agy
                                                                    ▼
                                            server.rs handle("/hook")
                                                                    │ events*.rs::to_message → 統一訊息
                                                                    ▼
                                            Shared::apply → state.rs Pet::apply → Outcome
                                                                    │ changed → Shared::redraw()
                                                                    ▼
                         emit "pet:update"（payload）、"pet:react"、"pet:asks" → bridge.js → pet.js / island.js / settings.js
```

### 1. 從 agent 到連接埠

- **Claude Code**：除 `SessionStart` 外都是 HTTP hook（`type: "http"`），不開行程。Claude Code **不對 `SessionStart` 執行 HTTP hook**，所以 settings.json 方式為它配一條背景命令（`async: true`）：`main.rs` 的 `ensure_running` 從 stdin 讀事件，連接埠上有 wakuwaku 回應就轉送，沒有就用 `detach::spawn` 啟動她（使用者主動退出過則不啟動，`data::may_start`）。外掛方式只有 HTTP hook，靠開機自啟。外掛的 `hooks.json` 要和 `connection.rs` 的 `EVENTS` 保持一致。
- **Codex**：只有 command hook。每個事件都執行一次 `main.rs` 的 `codex_hook`：補上行程鏈（`wakuwaku_chain`，供跳到視窗用），一般事件用 `post_and_go` 送出就走；只有 `PermissionRequest` 會等她的回答並把 `hookSpecificOutput` 印到 stdout（其他事件什麼都不印，Codex 會把輸出當成給模型的話）。永遠以 0 結束。
- **Antigravity**：`agy_hook` 先立刻印出回答（`Stop` 回 `{"decision":"allow"}`，其餘 `{}`），再把事件加上事件名送出。

### 2. server.rs 的 `/hook`

1. 讀 JSON，`connection::note_connected` 標記「已連接」；依 `?agent=` 在事件上加 `agent` 欄位，更新 `claude_seen` / `codex_seen` / `agy_seen`。
2. 依 agent 呼叫 `events::to_message`、`events_codex::to_message` 或 `events_agy::to_message`，得到統一格式的訊息（`/state` 也接受這個格式）：
   ```json
   { "session": "…", "project": "wakuwaku", "mood": "working", "detail": "Bash", "event": "tool-start", "toolId": "toolu_…", "step": { "key": "step.run", "vars": { … } } }
   ```
   主要欄位：`mood`（`idle` / `working` / `waiting` / `done` / `review` / `error`）、`event`（`turn-start` / `tool-start` / `tool-done` / `tool-failed` / `turn-end` / `session-end`）、`react` / `say`、`title` / `task`、`step` / `toolId`、`todo`、`file`、`reply` / `error`、`agent` / `turn`。完整表格見 `docs/DEVELOPMENT.md` 的「消息」一節。
3. 補上會話所在的行程鏈 `chain`（Claude Code 用請求的對端連接埠查 TCP 表；Codex / agy 用 hook 自己算好的），然後 `sh.apply(&msg)`。
4. 如果是 `PermissionRequest` 且她看得見（`asks_seen`）：把 tiny_http 的請求包成 `Responder` 交給 `asks.rs` 掛起，`push_asks()` 通知頁面；使用者回答、逾時（`promptWaitSec`，Codex 最多 60 秒）、或終端機已回答時才回覆。看不見（勿擾、隱藏、全螢幕）就直接回 `{}` 交給終端機。
5. 其他事件：`asks.seen(&event)` 收掉同一呼叫或同一會話已在終端機處理掉的確認，然後立刻回 `{}`。

### 3. state.rs

- `Pet::apply(msg, now, hold)` 依 `session` 更新各會話的狀態，回傳 `Outcome`：`changed`（要重畫）、`react`（播一次 wave / jump / failed）、`alerts`（提示音與系統通知）、`turns` / `worked_ms`（「今天」的統計）。
- 顯示哪個會話：等你確認 > 出錯 > 改好了 > 做完 > 工作中 > 閒置，同級取最新。做完 / 改好 / 出錯依設定 `hold` 停留（預設 8 秒，或直到滑鼠經過她）。
- Codex 的背景 hook 可能亂序到達：`state.rs` 記住最近結束的回合（`turn-end`），之後到達的同一回合事件不再改狀態。
- `ticker` 執行緒每秒呼叫 `tick`、`hide_quiet`（閒置會話隱藏，`idleHideMin`），每分鐘 `check_stale`（15 分鐘無事件的工作中 / 等待視為結束，2 小時遺忘）；有變化就 `redraw()`。

### 4. 到前端

- `Shared::redraw()` = `to_pages("pet:update", payload())` + `tray::refresh` + `push_settings()`。
- `payload()` 由 `pet.get(now)`（目前顯示的會話）、`second()`（下一個要你處理的會話）、`list()`（所有顯示中的會話）加上設定、語言、精靈圖網址、外掛等組成。她的視窗藏著時不收 `pet:update`，顯示時一次補上整份狀態。
- 其他事件：`pet:react`（一次性動作）、`pet:asks`（確認請求清單）、`pet:alert`、`pet:widgets`、`settings:changed`（設定開著時，每秒最多 4 次）。
- `bridge.js` 收到後交給 `pet.js`、`island.js`、`panel.js`、`settings.js` 等；`status.js` 把會話說成一句話。使用者在面板上回答時，頁面呼叫 `pet_answer` 命令 → `asks.answer` → `Responder` 回覆那個掛起的 HTTP 請求。

## 開發時要注意

- 持有鎖時絕不呼叫 Tauri 的視窗 getter（它們要等主執行緒，而選單回呼在主執行緒上拿鎖）；同時不拿兩把鎖；頁面的命令一律寫成 `async`。
- 新視窗要加進 `src-tauri/capabilities/` 的 `windows`，否則頁面的 `listen()` 會被拒。
- 她的視窗一律經 `Shared::show_window` 顯示（`show()` 會啟用視窗，要暫時忽略島的失焦）。
- hook 的回覆必須是物件；自動壓縮觸發的 `SessionStart`（`source: compact`）要忽略。
- 動到 hook 事件或安裝內容時，外掛的 `integrations/claude-code/` 與 `connection.rs` 要一起改。
- 文件與註解的風格：`docs/DEVELOPMENT.md`、README 和 commit 訊息會記下在 Hyper-V 測試 VM 裡實測過的行為；改了行為就更新 DEVELOPMENT.md 對應段落。更多踩過的坑見 DEVELOPMENT.md 的「踩过的坑」。
