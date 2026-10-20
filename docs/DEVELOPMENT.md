# 开发说明

Wakuwaku 是一个 Tauri 2 程序：页面是 `src/` 里的 HTML / JS / CSS，由系统自带的 WebView2 显示；其余都在 `src-tauri/` 的 Rust 里。（2026-10 之前是 Electron 版，在 git 历史里。）

## 目录

```
src/                      页面，Tauri 直接把整个文件夹打进 exe（tauri.conf.json 的 frontendDist）
  pet/
    index.html            桌宠窗口（?role=pet&agent=<agent>）和灵动岛的窗口（?role=island）共用的页面
    bridge.js             window.pet：页面和 Rust 之间的调用和事件；替页面补发鼠标进出（见"点击穿透"）
    pet.js                她的动画、空闲时走动和东张西望
    island.js             灵动岛：几种形状、她在岛里、把她拉出来和放回去
    settings.js           长在岛里的设置（三个页签：通用、外观、插件）
    panel.js              确认面板：在岛里，或在设置的横幅里
    sprite.js             图集布局、16 个注视方向
    status.js             把一个会话说成话：名字、当前这一步、任务清单进度、结果（岛、设置、她的提示共用）
    widgets.js            岛上插件的图标和文字
    style.css, settings.css
  shared/i18n.js          中英文（页面用；Rust 那边的几句在 src-tauri/src/i18n.rs）
src-tauri/
  src/
    main.rs               入口：--wakuwaku-ensure-running、--wakuwaku-codex-hook、--wakuwaku-agy-hook、守护进程、单实例、各部分共享的 Shared、页面能调的命令
    pet.rs                桌宠窗口（每个 agent 一个，pet-<agent>）：大小和位置、眼睛、拖动和被"携带"、走动、头顶的面板、飞回岛里
    island.rs             灵动岛的窗口：放在哪、按页面要求扩大、伸手够她、把她吸回去、为设置临时升起、置顶
    backplate.rs          垫在岛的窗口正下面的亚克力：一个自己的原生窗口，DWM 模糊它后面的别的窗口（#20）
    updates.rs            软件更新：看 GitHub 和 Gitee 的最新发布、在岛上提示、下载校验替换并重启
    crash.rs, crash/      崩溃报告：panic 钩子、未处理异常过滤器、守护进程（她自己的 exe 带 --wakuwaku-guard，等她结束、看退出码）、WebView2 进程挂掉；常开的尾日志、脱敏（redact.rs）、问一次、发到中转
    proxy.rs              Windows 的系统代理（崩溃报告发到中转时用）
    wallpaper.rs          Windows 的浅色、深色模式和强调色（灵动岛的外观、设置用）
    settings.rs           设置看到的快照、校验后的设置补丁、设置的各个命令
    pointer.rs            两个窗口共用的点击穿透
    screen.rs             各个显示器的工作区
    asks.rs               确认请求：显示什么、每个选择回给 Claude Code 什么、排队和超时
    connection.rs         插件、settings.json 里的 hooks、Codex 的 hooks.json（安装 / 移除 / 状态）、开机自启
    fetch.rs              codex-pets.net：解析地址、下载宠物、图库
    fullscreen.rs         别的程序是否全屏（user32），每 1.5 秒看一次
    power.rs              省电：后台线程一直、整个进程和 WebView2 的进程在静默时交给效率核（EcoQoS），静默时页面内存设为低；多久没碰键盘鼠标
    gpu.rs                她那块屏由哪块显卡输出（D3DKMT），WebView2 就在那块上渲染
    focus.rs              前台窗口：设置拿走键盘前记下，收起时还回去
    jump.rs               点会话就到它的窗口：会话的进程链、找窗口、叫到前面、桌面版的会话链接
    widgets.rs            岛上的插件（第一层）：脚本发来的、内置的（今天、今天的 token、监控），什么时候冒头
    tokens.rs             今天的 token：读 Claude Code、Codex 和 Antigravity 自己的会话记录
    scripts.rs            她替你在后台跑的插件：示例脚本的开关、参数、启动、重启、停下
    notify.rs             系统通知（登记 AppUserModelId 后发 toast）
    tray.rs               托盘图标（随心情变脸）和菜单
    server.rs             127.0.0.1:47213 上的 HTTP 接口
    state.rs              状态机：按会话记 mood、名字、在跑的工具、任务清单、改过的文件、结果，挑最需要你的那个显示
    events.rs             Claude Code 的 hook 事件 → 消息；会话名（session_title 或会话记录）、项目名（git 仓库名）
    events_codex.rs       Codex 的 hook 事件 → 消息（步骤、任务、回复、项目名的说法用 events.rs 的）
    events_grok.rs        Grok Build 的 hook 事件 → 消息（它的 camelCase 字段、工具名，Notification 的等你确认和安静下来）；is_grok 认出经 Claude Code 的 hooks 来的 Grok 事件
    data.rs               数据目录、设置读写、已下载的宠物和 Codex 的宠物
    detach.rs             以资源管理器为父进程启动她（hook 拉起、更新、守护进程），不落在启动者的作业里；启动时自查
  icons/                  exe、窗口、托盘和通知的图标（npm 不需要：scripts/make-icons.js 用 ImageMagick 画）
  capabilities/           页面能用的 Tauri 权限：岛和桌宠窗口（pet-*）都要列进去
integrations/claude-code/ Claude Code 插件 wakuwaku（只有 HTTP hooks）
.claude-plugin/           插件市场入口 marketplace.json（位置是 Claude Code 规定的）
scripts/make-icons.js     画图标（node + ImageMagick），输出提交在 src-tauri/icons
test/                     页面的单元测试（node --test，不需要 npm install）；fake-relay.js 是假的崩溃报告中转，fake-release.js 是假的发布
src-tauri/fixtures/grok/  Grok Build 1.0.52 实际发来的 hook payload（tools/hook_dump.py 抓的），events_grok.rs 和 state.rs 的测试直接用
tools/hook_dump.py        抓 hook payload：127.0.0.1:47299 收到的每个 POST 原样写进 tools/dumps/（不进仓库），只用标准库
tools/grok-hooks/         指向 hook_dump.py 的 Grok hooks.json，抓 Grok 的 payload 用
tools/make_sheet.py       一帧一张的 PNG 排成宠物图集（docs/MAKING_A_PET.md），要 Pillow
relay/                    崩溃报告中转（Cloudflare Worker → GitHub issue；阿里云函数计算 → Gitee issue），部署步骤在 relay/README.md
examples/widgets/         岛上插件的示例脚本：waku、CI、番茄钟、截止日期、开发服务器、股票、天气……
docs/prototypes/          设计原型：island-settings.html 是设置长在岛里的手感原型
data/                     从源码运行时她的设置、宠物和日志（不进仓库）
pets/                     从源码运行时也会读这里的宠物（不进仓库）
art/                      美术和调查资料，只在本地（只有 art/README.md 进仓库）
```

## 构建和运行

```bash
cd src-tauri
cargo run                 # 调试版：有控制台窗口
cargo build --release     # target/release/wakuwaku.exe：单个文件，约 5 MB，没有控制台
cargo test                # Rust 单元测试
cd .. && node --test "test/*.test.js"   # 页面的单元测试
```

页面是打进 exe 的：改了 `src/` 要重新编译（cargo 会发现）。不需要 Node 和 Tauri CLI；`Cargo.toml` 默认开着 `custom-protocol`，所以 `cargo run` 也直接用打进去的页面，不找开发服务器。

**数据放在哪**（`data.rs` 的 `folder()`）：`WAKUWAKU_USER_DATA`（测试用）；从源码树里运行时是项目的 `data/`；否则是 exe 旁边的 `wakuwaku-data/`，启动时没有就新建一个。旧的 Electron 安装版的 `%APPDATA%\wakuwaku` 不再读取。数据目录里放一个 `debug.on` 文件，就会写 `debug.log`（两个页面的 console.log 也在里面）。

| 环境变量 | 作用 |
| --- | --- |
| `WAKUWAKU_PORT` | 端口（默认 47213） |
| `WAKUWAKU_USER_DATA` | 换一个数据目录 |
| `CLAUDE_CONFIG_DIR` | Claude Code 的配置目录（测试里指向临时目录，不碰真实设置） |
| `CODEX_HOME` | Codex 的配置目录（`hooks.json`、`pets/`），默认 `~/.codex` |
| `GROK_HOME` | Grok Build 的目录（`hooks/wakuwaku.json`），默认 `~/.grok` |
| `WAKUWAKU_DEBUG=1` | `/health` 带上两个窗口和设置，开放 `/debug/eval`、`/debug/walk` |
| `WAKUWAKU_IDLE_HIDE_MS` | 空闲会话多久不再显示（毫秒，`0` 一直显示），代替设置 `idleHideMin` |

## 插件

`integrations/claude-code/` 是 Claude Code 插件（名字 `wakuwaku`），仓库根目录的 `.claude-plugin/marketplace.json` 让整个仓库成为插件市场。插件只有 HTTP hook（插件不知道程序装在哪，没法启动她），所以靠开机自启。它的 `hooks.json` 和 `connection.rs` 里 `EVENTS` 写入 settings.json 的 HTTP 部分要保持一致（端口、`from=wakuwaku`、事件和 matcher、PermissionRequest 的 300 秒超时）。

设置里读 `enabledPlugins["wakuwaku@wakuwaku"]` 判断插件是否启用（只读），连接方式分 `plugin` / `hooks` / `both`（事件会重复）/ `none`。

## hook 怎么送到宠物

除 `SessionStart` 外的事件都是 HTTP hook（`type: "http"`，POST 到 `/hook?from=wakuwaku`），不开进程；`/hook` 一般立刻回 `{}`，只有 `PermissionRequest` 会挂起，等面板上的回答。**Claude Code 不对 `SessionStart` 运行 HTTP hook**，所以写入 settings.json 的方式给它配了一条后台命令（`async: true`）：程序本身加 `--wakuwaku-ensure-running`。它从 stdin 读事件，端口上有 wakuwaku 应答就把事件转过去（打招呼、登记会话），没有就以独立进程启动一份宠物，然后马上退出。**启动时以资源管理器为父进程**（`detach.rs` 的 `spawn`：`PROC_THREAD_ATTRIBUTE_PARENT_PROCESS` 设成桌面窗口所在的进程，`DETACHED_PROCESS`，不继承任何句柄），和在桌面上双击打开一样落在资源管理器的作业里。hook 是在 agent 的作业里运行的，Claude 桌面版这类应用的作业在应用退出时会结束里面所有进程，而且不让脱离：从桌面版的会话里拉起的她，连同守护进程，在桌面版更新时被一起结束（用户 2026-10-13）。资源管理器不在时退回原来的做法。她启动时也自查（`detach::should_leave`）：在某个作业里、父进程又不是资源管理器（旧版的 hook、终端、别的应用里启动），就照上面的办法重新启动自己，带上 `--wakuwaku-out-of-job` 防止反复，这一份退出；从源码运行时不做。更新后启动新版、崩溃后重启（`updates.rs` 的 `start_new`）、守护进程（`crash.rs` 的 `spawn_guard`）也都这样启动。**用户主动退出后不启动**：右键菜单（托盘、她或岛）的「退出」在数据文件夹写一个 `quit-by-choice` 标记（`data.rs` 的 `mark_quit`），`--wakuwaku-ensure-running` 和 Codex 的 `SessionStart` 看到它就不启动（`may_start`）；不然退出后马上又会被拉起来（`resume`、`/clear`、自动压缩也会触发 `SessionStart`）。她每次真正启动（手动打开、开机自启、更新后重启、崩溃后被拉起）都会清掉它（`main()` 拿到端口后 `clear_quit`）。崩溃不留标记，会话照旧把她拉起来。HTTP 的 `/quit` 不算主动退出、不留标记：它是给脚本和换新版本用的（换完会自己启动新的），有人 /quit 之后没再启动，会话把她拉起来也说得过去。

`connection.rs` 的 `status_of()` 判断我们的条目：`ok`、`httpOnly`（没有启动命令）、`missing`、`partial`（缺事件）、`stale`（端口不对、指向另一份程序，或者是旧版本）。识别"是我们的"靠 URL 里的 `from=wakuwaku`、参数里的 `--wakuwaku-ensure-running`，以及改名前的 `claude-pets` 两个记号和更早的 `claude-hook.js`。写之前把原文件备份到 `settings.json.wakuwaku.bak`（只备份一次）。

单实例：谁占着端口谁是宠物。再启动一份时端口被占，这份会 POST `/come-home`（让她重新出现、回到右下角）然后退出。

## Codex

Codex（CLI 0.114 起）的 hooks 和 Claude Code 很像，但**只有 command 类型，没有 HTTP**，所以每个事件都运行一次程序本身加 `--wakuwaku-codex-hook`（`main.rs` 的 `codex_hook()`）：从 stdin 读事件，POST 到 `/hook?from=wakuwaku&agent=codex`；只有 `PermissionRequest` 会把她的回答（`hookSpecificOutput`）打印到 stdout 给 Codex，别的事件什么都不打印（Codex 会把 hook 打印的纯文本当成给模型的话）。连不上她时，`SessionStart` 会以独立进程启动她。不管怎样都以 0 退出，Codex 不会显示 hook 失败。

`connection.rs` 的 Codex 部分写 `~/.codex/hooks.json`（支持 `CODEX_HOME`；不碰 `config.toml`），备份到 `hooks.json.wakuwaku.bak`。2026-10 在 Codex CLI 0.146.1 / Windows 11 上实测：

- **Codex 用会话的 shell 运行 hook 命令，Windows 上是 PowerShell 7**（源码里的默认 `cmd /C` 只在没配置 shell 时用）。`"带空格的路径" 参数` 在 PowerShell 里是语法错误，要写 `& '路径' 参数`；路径不需要引号时直接写裸路径（`codex_command()`）。
- Codex 跑 hook 用的是 `pwsh -NoProfile -Command`（`derive_exec_args`，不走 profile），实测启动约 0.28 秒；PowerShell 5.1 约 0.18 秒，cmd 约 0.02 秒，转发本身约 0.03 秒。hook 的 shell 跟着 Codex 会话的 shell 走，没法单独换。
- 所以 Codex 要等的（同步）hook 只装必要的几个（`connection.rs` 的 `CODEX_HOOKS`）：`SessionStart`、`UserPromptSubmit`、`Stop`、`Interrupt`、`SessionEnd`、`PermissionRequest`，`PostToolUse` 只对 `apply_patch|request_user_input`，`PreToolUse` 只对 `request_user_input`（matcher 是正则）。每一步的工具名靠两条不带 matcher 的 `async: true` 的 `PreToolUse` / `PostToolUse`。
- **async hook 从 Codex 0.148 开始才有**；0.146.1 实测会跳过它们（`skipping async hook …: async hooks are not supported yet`），而且是整条跳过，不会改成同步运行。所以安装时用 `codex --version` 判断（缓存一分钟），旧版本不装那两条；之后 Codex 升级了，状态会变成 `upgradable`（只缺后台的那两条；设置「通用」页的连接部分说"Codex 已升级"而不是"需要修复"），点「修复」补上，原来那些 hook 内容和位置都不变，Codex 里只需信任新加的两条（信任按 `hooks.json:<事件>:<组>:<序号>` 和内容的 hash 记在 `config.toml` 的 `[hooks.state]`）。缺别的才是 `partial`。问不到版本（只有桌面版）时当作新版本。
- 后台运行的 hook 可能乱序到达：每个事件都带 `turn_id`，`state.rs` 记住最近结束的几个回合（`Stop` / `Interrupt` 的消息带 `event: "turn-end"`），之后到达的同一回合的事件不再改状态，只有迟到的 `apply_patch` 会把「做完」改成「改好了」。新版本上 `apply_patch` 的 `PostToolUse` 会同步、后台各来一次，重复无害。
- 事件字段：`session_id`、`turn_id`、`cwd`、`hook_event_name`、`model`、`permission_mode`；shell 工具叫 `Bash`，改文件都是 `apply_patch`，两者的 `tool_input` 都只有 `command`（补丁全文）。失败的工具调用没有 `PostToolUse`；请求失败（比如模型不可用）连 `Stop` 都没有，所以 Codex 会话不会显示出错，干活状态靠 15 分钟的超时收尾。
- Codex 只运行**信任过**的 hook（按 hash 记在 `config.toml`），新装或改动后要在 Codex 里 `/hooks` 信任。我们不替用户信任。设置里能看出的只有"装了"和"收到过 Codex 的事件"（`Shared.codex_seen`），「通用」页的连接部分据此提示去信任。
- `PermissionRequest` 在 Codex 弹自己的确认框**之前**同步运行，hook 不回答 Codex 就不弹框。所以 Codex 的确认在面板上最多等 60 秒（`server.rs` 的 `CODEX_ASK_MS`），"去终端处理"或超时回 `{}` 后 Codex 才弹框。Codex 只接受 allow / deny，不接受 `updatedPermissions` / `updatedInput`，所以没有"以后都允许"（事件里也没有 `permission_suggestions`）。`hookSpecificOutput.decision` 的格式和 Claude Code 相同。
- `codex exec` 会把确认策略强制设成 never，测不到 `PermissionRequest`；测这一条要用交互式的 Codex。
- 测试运行 Codex 时用便宜的模型：`codex exec --ephemeral --skip-git-repo-check -m gpt-5.6-luna -c model_reasoning_effort='"low"' --dangerously-bypass-hook-trust -c 'hooks.Stop=[...]' "…" < /dev/null`（`-c hooks.*` 只对这一次生效，不改用户的配置；不重定向 stdin 会一直等输入）。

Codex 桌面版的宠物在 `~/.codex/pets/<id>/`（`pet.json` + `spritesheet.webp`，格式和 codex-pets.net 下载的一样），`data.rs` 把它排在自己的宠物之后一起读，只读不写，也不替它建文件夹。

## Grok Build

Grok Build（xAI 的命令行 coding agent，2026-07 以 Apache 2.0 开源：[xai-org/grok-build](https://github.com/xai-org/grok-build)）的 hooks 格式和 Claude Code 插件的 `hooks.json` 一样，**所有事件（包括 `SessionStart`）都能发 HTTP hook**。下面按 1.0.52 实测（2026-10-16，`tools/hook_dump.py` 抓的 payload 在 `src-tauri/fixtures/grok/`），细节都对过源码。

- **装在哪**：`connection.rs` 写 `~/.grok/hooks/wakuwaku.json`（`GROK_HOME`）。Grok 自己会读 `$GROK_HOME/hooks/*.json`，一直受信任，不用在 `/hooks` 里加路径（源码 `xai-grok-config/src/global_hook_sources.rs`；`/hooks` 加的路径记在 `~/.grok/hooks-paths`，一行一个绝对路径，不是 `config.toml`）。整个文件是我们的：移除就删掉，不用备份。事件：`SessionStart`、`SessionEnd`、`UserPromptSubmit`、`PreToolUse`、`PostToolUse`、`PostToolUseFailure`、`Stop`、`StopFailure`、`StopCancelled`，`Notification` 只要 `permission_prompt|idle_prompt`；全是 `type: "http"`，`/hook?from=wakuwaku&agent=grok`，`timeout` 2。状态：`absent`（没有 `~/.grok`）、`ok`、`stale`（别的端口、旧版本的事件）、`missing`、`unreadable`。已经开着的 Grok 会话要重开，或在 `/hooks` 里按 `r`。
- **字段**：Grok 自己的字段是 camelCase（`hookEventName: "pre_tool_use"`、`sessionId`、`toolName`、`toolInput`、`promptId`、`workspaceRoot`、`lastAssistantMessage`……），只给 10 个加了 snake_case 别名（`hook_event_name` 带 Claude 的 PascalCase 值，`session_id`、`transcript_path`、`permission_mode`、`tool_name`、`tool_input`、`tool_response`、`tool_use_id`、`duration_ms`、`is_interrupt`；源码 `xai-grok-hooks/src/event.rs` 的 `SNAKE_CASE_ALIASES`）。所以 `lastAssistantMessage`、`promptId`、`errorDetails`、`notificationType`、`subagentType` 只能按 camelCase 读。
- **工具**：`run_terminal_command`（`{command, description}`，Windows 上是 PowerShell 命令）、`write`（`{file_path, content}`）、`search_replace`（`{file_path, old_string, new_string}`）、`read_file`（`{target_file, offset?, limit?}`）、`grep`（`{pattern, path}`）、`list_dir`、`web_search`、`spawn_subagent`；MCP 工具是 `server__tool`（没有 `mcp__`）。`write`、`search_replace` 算改动（`state.rs` 的 `EDIT_TOOLS`）。`toolResult` 不读：读一整个文件时有几十 KB，shell 的输出还是字节数组。
- **没有 `PermissionRequest`，hook 也替不了你批准**：`PreToolUse` 回 `{"decision": "allow"}`（或 `hookSpecificOutput.permissionDecision`）只表示「不拦」，Grok 照样弹自己的确认（源码测试 `pre_tool_use_allow_does_not_auto_approve`）；能做的只有 `deny`、`ask`、`defer`。所以岛上不回答 Grok 的确认，只显示它在等你：Grok 的确认画面真的在等人时发 `Notification`（`notificationType: "permission_prompt"`，不带工具名，紧跟在那个工具的 `PreToolUse` 之后），`events_grok.rs` 发 `event: "permission"`，`state.rs` 用正在跑的那一步说「要批准：……」（没有就说「等你在终端里确认」）。批准了就接着来那个工具的 `PostToolUse`；拒绝了是 `StopCancelled`（`reason: "permission_rejected"`）。`/hook` 对 Grok 一律马上回 `{}`。
- **回合的结束**：`Stop`（`reason: "end_turn"`）、`StopFailure`（`error` 是 `rate_limit` 等，`errorDetails`）、`StopCancelled`（`reason` 是 `user_interrupt`、`permission_rejected`……）。会话结束时还会多来一个 `Stop`（`reason: "shutdown"`），**在 `SessionEnd` 之后到**，不算数，不然会话又被拉回「做完」。`StopCancelled` 不在 Grok 的主循环里发，可能比下一轮的 `UserPromptSubmit` 晚到：`state.rs` 记住最近几个开始过的回合，一个开始过、又不是现在这一轮的回合的结束只记下，不改状态；没看到开始的回合（它的 `UserPromptSubmit` 丢了）照常结束。
- **安静下来**：一轮结束、闲了大约一分钟后 Grok 发 `Notification`（`idle_prompt`）。它只收掉还停在干活或等你的会话（`event: "settle"`），不碰没看过的「做完」，也不为没见过的会话建一个。
- **子 agent**：子 agent 自己会话的事件带 `subagentType`，全部不算（它的 `SessionEnd` 会把主会话结束掉）。
- **Grok 也会读 Claude Code 的 hooks**：Claude Code 插件（`~/.claude/plugins/installed_plugins.json`）和 `~/.claude/settings.json`（源码 `xai-grok-agent/src/plugins/discovery.rs`、`xai-grok-hooks/src/discovery.rs`），所以同一个事件也会从 `/hook?from=wakuwaku`（没有 `agent`）来。Grok 只合并网址完全一样的 hook，网址不同就两边都发。`server.rs` 的 `sender_of` 用 body 里有没有 `hookEventName`（Claude Code 不发）认出它们：Grok 自己的文件装着（`connection::grok_hooked`，5 秒看一次）就丢掉，没装就当成 Grok 的。
- **找窗口**：Grok 的 hook 是 HTTP 的，和 Claude Code 一样用连接对端的端口查进程链（`server.rs` 的 `chain_for`）。
- **今天的统计**：Grok 的回合和 Antigravity 的一起算进 `agy_turns`（批准不经过她的回合），全是这种回合又没批准过时，批准显示「—」。今天的 token 不读 Grok 的记录。
- **抓 payload**：`python tools/hook_dump.py`（127.0.0.1:47299），把 `tools/grok-hooks/hooks.json` 放进 `~/.grok/hooks/`（或用 `/hooks` 加它所在的文件夹），重开 Grok 会话。
- **实测**（2026-10-16，Grok Build 1.0.52，Windows 11）：从「连接」页装好 `~/.grok/hooks/wakuwaku.json` 后，Grok 的会话正确显示在岛上，改了文件后这一轮结束显示「改好了」。

## 消息

hook 事件在 `events.rs`（Codex 的在 `events_codex.rs`）换算成消息，`/state` 也收这个格式：

```json
{ "session": "…", "project": "wakuwaku", "mood": "working", "detail": "Bash", "event": "tool-start", "toolId": "toolu_…", "step": { "key": "step.run", "vars": { "what": "Run the tests" } } }
```

| 字段 | 取值 |
| --- | --- |
| `session` / `project` | 会话 id、项目文件夹名；每个会话单独记状态 |
| `mood` | `idle` / `working` / `waiting` / `done` / `review` / `error` |
| `detail` | 工具名，或 `{ key, vars }`（按当前语言显示；vars 里可以再套一层 `{ key, vars }`，比如「要批准：$ git push」） |
| `event` | `turn-start` / `tool-start` / `tool-done` / `tool-failed` / `turn-end`（Codex、Grok 的回合结束）/ `session-end`；Grok 另有 `permission`（它自己的确认在等你）和 `settle`（安静下来，见下面的 Grok 一节） |
| `react` / `say` | `wave` / `jump` / `failed`，在当前 mood 上播一次，岛展开说 `say` |
| `title` / `task` | 会话名；你发的第一句话（只认 `source` 是用户的，少于 4 个字的「继续」「好的」不算）。有 title 就用 title |
| `step` / `toolId` | 工具在做什么（`step.*`，`events.rs` 的 `step_of`）；`toolId` 把开始和结束配成对，子 agent 的工具也能对上 |
| `todo` | 任务清单的变化：TodoWrite 给 `{ set }`，TaskCreate / TaskUpdate 给 `{ add }` / `{ update }` |
| `file` | 刚改过的文件（这一轮改了几个文件） |
| `reply` / `error` | Stop / StopFailure 的 `last_assistant_message` 摘前一两句；StopFailure 的 `error`（`error.*`）或 `error_details` |
| `agent` / `turn` | Codex 的消息带 `agent: "codex"` 和回合 id（见上面的 Codex 一节）；Antigravity 是 `agy`，Grok 是 `grok`，回合 id 是 `promptId` |

**会话名**：hook 自带的 `session_title`（只有 UserPromptSubmit、SessionStart 带，而且只是自定义标题：`/rename` 或桌面版起的名字）；没有时，在一轮开始和结束时读 `transcript_path` 末尾 256 KB 里最后一条 `custom-title`，没有再用 `ai-title`。**项目名**：从 `cwd` 往上找 `.git`，是目录就取它所在文件夹的名字；是文件（worktree）就按里面的 `gitdir: <仓库>/.git/worktrees/<名字>` 取仓库名；不在 git 里就用 `cwd` 的文件夹名。按 `cwd` 缓存。

页面拿到的每个会话（`state.rs` 的 `view`）还有 `name`、`step`（只在干活时有，没有在跑的工具就是「思考中」）、`stepSince`、`todo`（`{ done, total, active }`）、`files`、`reply`（只在结束时有）、`error`、`agent`。`payload` 里的 `list` 是所有显示着的会话（见下面「不再显示的会话」），展开的岛用它列出别的会话。设置里的「显示具体内容」（`details`）关掉时，页面只显示项目、状态和工具名，和以前一样。

显示哪个会话：等你确认 > 出错 > 改好了 > 做完 > 干活 > 空闲，同级取最新。做完、改好、出错默认停留 8 秒（设置 `hold`）；设置里可以改成 30 / 120 秒，或者一直保持到鼠标经过她（或离开灵动岛，`seen`）。干活和等你超过 15 分钟没有新事件就当作结束；做完的状态 2 小时没人看就回空闲，空闲会话 2 小时后被忘掉。

**不再显示的会话**（#39，`state.rs` 的 `is_shown`，`ticker` 每秒调一次 `hide_quiet`）：干活和等你确认的一直显示；做完、改好、出错在被看到之前一直显示（最多到上面的 2 小时）；空闲的会话从最后一个事件、或者它的结束被看到 / 停留时间到了收起（`seen_at`）算起，取较晚的那个，安静满设置 `idleHideMin` 分钟（15 / 30 / 60 / 120，默认 30，`0` 是一直显示）就不再显示。不显示的会话只是不进 `get()` 的 `others` / `sessions`、`list()` 和 `second()`，所以「+N」、展开的岛、设置里的会话列表和托盘提示都不算它。它还在 `sessions` 里，什么都不丢：再来一个事件就立刻回来（`apply` 里清掉 `hidden`），名字、项目、「今天」的统计都在；真正删掉还是按上面的 2 小时。测试时用环境变量 `WAKUWAKU_IDLE_HIDE_MS` 代替设置（毫秒，`0` 是一直显示）。

## HTTP 接口

| 请求 | 说明 |
| --- | --- |
| `GET /health` | `{ ok, app: "wakuwaku", runtime: "tauri", state }`；调试模式下还有 `window`、`island`、`settings` |
| `POST /hook` | Claude Code 的 hook 事件；一般立刻回 `{}`，`PermissionRequest` 等面板上的回答。正文上限 16 MB（Edit 的 PostToolUse 带着改之前的整个文件）。带 `?agent=codex` 的是 Codex 的事件（由 `--wakuwaku-codex-hook` 转来） |
| `POST /state` | 一条消息 |
| `POST /widget` | 岛上的一个插件（见"岛上的插件"）；回 `{ ok: true }`，或者 400 和 `{ error }` |
| `POST /come-home` | 再次启动时用：她重新出现 |
| `POST /quit` | 像菜单里的「退出」一样退出；换新版本时用，别直接杀进程 |
| `POST /debug/eval` | `{ page: "island" \| "pet-<agent>", code }`：在页面里运行一段表达式，返回结果（可以是 Promise）；`"pet"` 是 Claude 的桌宠 |
| `POST /debug/walk` | `{ dx, ms }`：立刻走一段 |
| `GET /mobile/…` | 手机页和它的数据（见"手机页"）；从别处来的请求只能到这里 |

## 在宠物上确认

交互模式下，终端确认框弹出的同时 PermissionRequest hook 在后台运行，先到先得。`server.rs` 把这个请求交给 `asks.rs` 挂起（`Responder` 里拿着 tiny_http 的请求），回答、超时、终端已答时才回复。后台子 agent 和 `claude -p` 会先等 hook 回复再决定，所以面板的等待时间（30 秒到 5 分钟）就是它们最多被拖住的时间。她不可见时（勿扰、隐藏、全屏），确认直接回 `{}`。

| 选择 | 回给 Claude Code 的 `hookSpecificOutput.decision` |
| --- | --- |
| 允许 | `{ behavior: "allow" }`（计划带回 `updatedInput`） |
| 以后都允许 | `{ behavior: "allow", updatedPermissions }`，取事件 `permission_suggestions` 里"允许"类的几条 |
| 拒绝 | `{ behavior: "deny", message }`（按当前语言） |
| 回答问题 | `{ behavior: "allow", updatedInput: { ...tool_input, answers } }`；选项、自己输入的文字、数字题都按题目文字作键 |
| 去终端处理 / 超时 / 终端已答 / 勿扰 / 全屏 | `{}` |

面板什么时候收起：同一个调用的 `PostToolUse` / `PostToolUseFailure` 到达（比较输入时忽略 `answers`）；同一个会话的 `UserPromptSubmit`、`Stop`、`StopFailure`、`SessionEnd`、`Interrupt`（Codex）；同一个 agent 又来了新请求；等待时间到。每个请求怎么结束的，日志里都有一行：`answer N`（在她身上答的）、`prompt N handed to the terminal: its time is up`（超时）、`… asked to on her`（去终端处理）、`prompt N settled in the terminal (事件名)`（终端已答）、`… out of sight`（看不见）。`asks.rs` 的 `expire` 和 `seen` 返回结束了的 id，供调用的地方写日志。以前超时、终端已答都不写，等满 290 秒退回终端的那次在日志里看不出来（2026-10-13 测试会话）。

面板在哪：哪个 agent 的确认，看那个 agent 的桌宠在不在桌面上（`main.rs` 的 `asks_for`，`asks::agent_of`）。在桌面上，就在那只桌宠头顶（下面「通知和确认在桌宠旁边」一节）；不在，就在岛里（岛的 `ask` 形状），设置开着时是设置里的横幅。

## 点会话就到它的窗口

在哪里点：展开的岛里的会话（`island.js` 的 `data-jump`）、设置「通用」页的会话行、确认面板的「去终端处理」（先交还给终端再跳）、单击她（等 500ms 确认不是双击；去的是指针刚移到她身上时她显示的那个会话，因为移上去就算看过，结束的状态会变回空闲）。都走 `session_jump` 命令 → `main.rs` 的 `jump_to` → `jump.rs` 的 `go`。

**会话在哪个进程里**（`state.rs` 的 `chain`，消息里是 `[[pid, 启动时间], …]`）：
- Claude Code 的 HTTP hook：`server.rs` 用请求的对端端口查 TCP 表（`GetExtendedTcpTable`），连接另一头就是 Claude Code 自己的进程，再往上找父进程。会话还没有进程链、或者一轮开始（UserPromptSubmit / SessionStart，可能换了进程续上）时才查。
- Codex：hook 命令（`--wakuwaku-codex-hook`）在自己里面算好父进程链，跳过它自己和 Codex 用来跑它的 shell，从 Codex 开始，放进事件的 `wakuwaku_chain` 字段。
- 每个进程都记启动时间，父进程不会比子进程晚启动：对不上的就是 pid 被复用了。走到 explorer、sihost、svchost 这类所有程序都在其下的进程就停。

**找哪个窗口**：会话自己的进程（链的第一个）还在才算（桌面版的会话除外，见下）。然后沿着链往上，第一个还活着（启动时间对得上）、有可见顶层窗口的进程：它自己的窗口，或者它下面 conhost / OpenConsole 的控制台窗口；有好几个（VS Code、Windows Terminal 都是一个进程好几个窗口）时取标题里有会话名、你的话或项目名的那个，没有就取最前面的。都没有时问控制台：`AttachConsole` → `GetConsoleWindow`，可见就是它，不可见就取它的根拥有者（Windows Terminal 托管从外面启动的控制台时，拥有那个隐藏的伪控制台窗口）。程序自己有控制台时（debug 版）不做这一步，否则要先放掉自己的。

**叫到前面**：最小化的先还原，再 `SetForegroundWindow`；不给（前台锁）就 `AttachThreadInput` 借一下前台窗口的输入再试。以 `SetForegroundWindow` 的返回值为准：真正切过去可能晚一点。

**Claude 桌面版**：它跑的每个会话在 `claude-code-sessions/**/local_<id>.json` 里有一条记录，开头有 `cliSessionId`（就是 hook 的 `session_id`）。安装版（MSIX）的在 `%LOCALAPPDATA%\Packages\Claude_*\LocalCache\Roaming\Claude\`，否则在 `%APPDATA%\Claude\`。找到了就打开 `claude://code/continue?session=local_<id>`，桌面版会切到那个会话；同时把它的窗口叫到前面，不管会话的进程还在不在。

**测试**：debug 版有控制台，测不到控制台那一步；用 `cargo rustc --bin wakuwaku -- -C link-args=/SUBSYSTEM:WINDOWS -C link-args=/ENTRY:mainCRTStartup` 编一个没有控制台的 debug 版（放到单独的 `CARGO_TARGET_DIR`，`RUSTFLAGS` 会连 proc-macro 一起改坏）。发事件的进程要一直开着（就像 Claude Code 自己），用 `pwsh -NoExit -Command Invoke-RestMethod …`，不能用发完就退出的 curl。

## 岛上的插件（第一层：只收数据）

插件就是岛上的一行字：图标、标题、数值、颜色。脚本往 `/widget` 发 JSON（字段见 [examples/widgets](../examples/widgets/README.md)），`widgets.rs` 校验后放进 `Shared.widgets`，宠物只显示文字，不运行任何东西。同一个 `id` 再发一次就是更新；`ttl` 秒没再收到就去掉（每 3 秒清一次），脚本停了它就自己消失。脚本发来的最多 32 个。

**会话永远优先**：只有最需要你的会话是空闲（`now.mood === 'idle'`）时，收起的岛才显示插件（`island.js` 的 `isQuiet`），展开时列出全部，当前那个高亮。轮换（`widgetSpin` 秒，0 不轮换）只在收起时走；滚轮切到下一个，点一个就固定显示它。宠物模式下不显示插件。

**冒头**（`nudge`）：插件可以请岛打开一次，`main.rs` 的 `nudge_widget` 只在看得见、设置允许（`widgetNudge`）、插件开着、而且没有会话在等你或有没看过的结束（`state.rs` 的 `wants_you`）时才发 `pet:nudge`。同一个插件一分钟最多一次。岛上展开 6.5 秒显示它的话；宠物模式下她挥手，在气泡里说（挥 6 次，约 4 秒）。

**私密**（`private: true`）：「显示具体内容」关掉时，页面不显示它的 `label` 和冒头的话，只显示 `value`（`widgets.js` 的 `words(lang, w, detailed)`）。

**内置的**（标题和数值是 `{ key, vars }`，按当前语言显示）：
- `today`：今天开始了几轮（state 的 `Outcome.turns`）、结束的几轮一共用了多久（`Outcome.worked_ms`，几个会话同时跑会叠加；被人停下的一轮（Codex 的 Interrupt、Antigravity 的取消，`idle` 加 `turn-end`）也算到停下时；一轮没说结束、下一轮就开始了，算到它最后一个事件）、在宠物上批准了几次（`pet_answer` 的 allow / always）。时长按 `h:mm` 显示，不满一分钟按秒（`widget.todaySeconds`），免得几轮短的一直是 0:00。Antigravity 和 Grok 的批准不经过她，今天的轮次全是它们的（`Today.agy_turns`）又没批准过时，批准显示「—」（`widget.todayValueAgy`）。存在数据目录的 `today.json`，按本地日期换天。
- `monitor`（监控，`widgets::Monitor`）：CPU（两次 `GetSystemTimes` 之差）、内存（`GlobalMemoryStatusEx`）、网速（两次 `GetIfTable` 之差，只算开着的以太网和 Wi-Fi，Windows 会把一块网卡经过各层过滤器列好几遍，按 MAC 地址只算一次；计数是 32 位的，过 4 GB 会从头再来，按回绕相减）、电池（`GetSystemPowerStatus`，没有电池这项就没有）。每项一个开关，设置 `monitor: { cpu, mem, net, battery, every }`，`every` 是几秒读一次，1 到 10，默认 2，「插件」页那一行的「设置」里 − / + 调。它的数值是 `{ parts: [{ icon, text }] }`：`cpu 12%`、`memory 46%`、`down 1.2M`、`up 80K`（每秒字节，`rate`）、`battery 85%`（充电时图标是 `bolt`）；岛上收起时只显示这一串读数（`widgets.js` 的 `partsHTML`），没有标题。每个读数是一个小格（`cellHTML`，`style.css` 的 `.wcell`）：Windows 自己图标字体里的图标（`Segoe Fluent Icons`，Windows 10 上是 `Segoe MDL2 Assets`；处理器 E950、内存 E964、下行 E74B、上行 E74A，电池按电量 E850–E859/E83F，充电 E85A–E863/E83E）和数字装在一起，底色从左往右填到读数那么多（`shareOf`：CPU、内存、电量就是百分比；网速没有 100%，按数量级，1 KB/s 空、100 MB/s 满，每大十倍多五分之一）；颜色跟文字走（深底白、浅底黑，`--cell-ink`），只有 CPU、内存到 90% 以上（或电池低于 20%）填成橙色；每格一样宽（按「100%」），数字变了列也不动。监控自己的图标是 Windows 的 E9D9（任务管理器「性能」那种，`widgets.js` 的 `GLYPHS`）。要两次读数的（CPU、网速）从第二次起才有。一个线程单独读它（`main.rs`），每 0.25 秒看一眼设置，打开或改了频率马上读。默认关闭。以前的 `sys`、`net`、`battery` 三个插件读设置时并进来（`data::adopt_monitor`）：哪个开着，监控就开着、开着那几项。
- `tokens`（`tokens.rs`）：今天的 token，每分钟读一次。Claude Code：`<配置目录>/projects/**/*.jsonl` 里每条回复的 `message.usage`（输入 + 缓存写 + 缓存读 + 输出），同一个 `message.id` 会按片段写好几行，只算一次。Codex：`~/.codex/sessions/**/*.jsonl` 的 `token_count` 事件是这个会话到那时的总数，今天的用量 = 今天最后一个 − 今天之前最后一个。Antigravity：`~/.gemini/<产品>/brain/<会话>/.system_generated/logs/transcript.jsonl`（agy 是 `antigravity-cli`，2.0 桌面版是 `antigravity`，文档里 IDE 是 `antigravity-ide`），一行一步，模型的每次回答（`PLANNER_RESPONSE`）带 `input_tokens`、`cache_read_tokens`、`output_tokens`（含思考）和 `created_at`（UTC，到秒），按 `step_index` 每步只算一次（文件被重写、从头再读也不重复）；合计 = 输入 + 缓存读 + 输出（按 Claude Code 的意思理解：输入不含缓存读；目前见到的缓存读都是 0）。旁边的 `transcript_full.jsonl`（hook 的 `transcriptPath` 指的是它）步骤一样、内容完整，更大，不读；`chunks/` 下是分块的副本，也不读。这个格式没公开文档，按 agy 1.3.4 实测（2026-10-14）。只读今天改过的文件，每个文件从上次读到的地方接着读，只读完整的行；换天从头算。默认打开。

**设置**：`widgetsOff`（关掉的 id，默认 `["monitor"]`）、`widgetOrder`（显示顺序，「插件」页的 ↑ 改它）、`widgetSpin`（0 / 5 / 8 / 15）、`widgetNudge`。

**「插件」页**：一行一个插件，每行一个开关。三个内置的一直列着（`widgets.rs` 的 `BUILT_IN`），关着的、或者开着还没读到的没有数值，岛上不显示它们（`main.rs` 的 `widgets_said` 发给岛的是有数值的，每个带着 `on`；岛上轮流的只有开着的）。内置的在岛上开着时才去算（`is_widget_wanted`）。刚打开的 token 马上读一次，不等下一轮。设置页的内容是原地改的（`settings.js` 的 `setHTML`：新的 HTML 和页面上的逐个节点比，只改不一样的属性和文字），所以监控每秒变的数字不会让整页重画、闪一下，正在输入的框也不会丢焦点。接着是她替你跑的插件（下一节），最后是别人的脚本发来的。↑ 改的是 `widgetOrder`，里面可以是 widget 的 id，也可以是插件的 id：插件发来的 widget 排在插件的位置（`view` 的 `owner`）。

## 她替你跑的插件（scripts.rs）

examples/widgets 里的天气、股票、番茄钟、倒计时、久坐提醒、CI、开发服务器，打开开关她就在后台替你跑，不用开终端。

- **设置**：`plugins: { <id>: { on, <参数>: 值 } }`，参数名就是脚本的参数名（`City`、`Repo`……），`scripts::is_ok` 只收认识的插件和参数：文字不带控制字符，数字是正数，开关是 true / false。页面上点「设置」展开参数；打开一个缺必填参数的插件时自动展开，那一行说「先填……」。正在输入的不会被每 3 秒的刷新冲掉（`settings.js` 的 `drafts`），回车或离开输入框才保存。
- **脚本从哪来**：编进 exe（`include_bytes!`），启动后写到 `<数据目录>/plugins/<id>.ps1`（内容不同才写），所以哪份宠物跑的都是自己那一版。
- **怎么跑**：有 PowerShell 7 就用它，否则用 Windows 自带的。每次启动都重新找（`scripts.rs` 的 `powershells`）：PATH 里真正的 pwsh、Program Files 里的、PATH 里商店版的应用执行别名（0 字节的重解析点）、Windows PowerShell，按这个顺序；上次成功的那个排最前，启动不了就换下一个，每次失败记一行 `plugins: <id> not started with <路径>: <错误>`，都启动不了时设置页写「PowerShell 启动不了」（`noShell`）。商店版 PowerShell 会自己在后台更新、挪走旧的程序包文件夹，之后同一个进程再用这个别名启动会报 os error 3（zhoupengjie/Wakuwaku#28），以前只找一次就一直失败到重启。`-NoProfile -NonInteractive -ExecutionPolicy Bypass -Command`，先把输出改成 UTF-8、去掉颜色，再 `& '<脚本>' -City '上海' -Port <端口>`。参数值放在单引号里，里面的单引号（包括 PowerShell 也认的弯引号）都写两遍，所以填什么都只是文字。没有窗口（`CREATE_NO_WINDOW`）。
- **一直开着**：主循环每 3 秒 `scripts::sync` 一次，改设置时也马上来一次：该跑没跑的启动，关掉的停下，参数变了的重启；自己结束的再启动，60 秒内就结束算失败，接连失败就等 10 秒、30 秒、1 分钟、5 分钟再试。停下（关掉或换参数）时它之前发的 widget 一起去掉，省得留着旧的（`owner_of` 按 id 认是谁发的：`weather`、`stock-*`、`ci-*` / `prs-*`……）。
- **说了什么**：脚本的输出全写进 `plugins/<id>.log`（每次启动重写），最后一行有内容的（去掉"警告:"前缀）显示在那一行下面：开着但岛上还没有它的东西时说「还没显示：……」，自己停了说「停了：…… · N 秒后再试」。
- **跟她一起走**：她启动时把开着的都启动，她开机启动（「连接」页的开关：注册表 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` 里的 `Wakuwaku`），插件也就开机就跑；插件自己不登记开机启动。所有插件进程都放进一个 job object（`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`），她退出时句柄关上，系统把它们全部结束，任务管理器里强行结束她也一样；正常退出时 `RunEvent::Exit` 还会先逐个结束。
- **waku** 不是开关：它包住你在终端里跑的命令。页面上给一条复制用的命令，指向写出来的 `plugins/waku.ps1`。

**以后的两层**：插件文件夹里的脚本由宠物定时运行（像 xbar），以及插件自己画的界面（要关进碰不到 IPC 的沙盒，否则能替你点"允许"）。

## 点击穿透

窗口要"透明处点击穿透，她身上接住点击"。Electron 可以穿透的同时照样收到 mousemove（`setIgnoreMouseEvents(true, { forward: true })`），Tauri 没有 forward，所以：

1. Rust 轮询光标（`pointer.rs`，由 `pet.rs`、`island.rs` 的 `poll` 调用；平时 200ms，靠近 80ms，在窗口里 30ms）。光标靠近窗口时，把它在页面里的位置发给页面（`pet:pointer`）。
2. `bridge.js` 用 `elementFromPoint` 算出光标下有哪些元素，替页面补发 `mouseenter` / `mouseleave`。
3. 页面照旧调用 `window.pet.hover(true)`，Rust 让窗口接住鼠标（`set_ignore_cursor_events(false)`）；离开时再变回穿透。

拖动由 Rust 跟随光标，并直接读鼠标按键（`GetAsyncKeyState`）：松手时页面没收到 mouseup 也能结束。鼠标在她身上时 Rust 也盯着按键，按下就开始拖（在岛里进出过之后，Windows 有时会吞掉她窗口的按下）。她被拖着经过灵动岛时，岛只做穿透，不会被"悬停"展开。

## 她的家：灵动岛

`display` 是她的家，只有 `island`；`desktop`（`{ claude, codex, agy, grok }`）是哪几个 agent 的桌宠在桌面上，`spots` 是各自放在哪（物理像素，`pet.rs` 写）。旧的 `out`、`x`、`y` 读进来变成 Claude 的（`data.rs` 的 `adopt_out`）。旧设置里的 `pet`、`capsule`、`corner`、`bar`、`taskbar` 读进来都换成 `island`（`pet` 另外让 Claude 的桌宠在桌面上）；已经没有的功能留下的设置（`corner`、`bubble`，任务栏的 `taskbar*`、`tray*`、`islandLook`，邮件的 `mail*` 等）读的时候丢掉（`data.rs` 的 `load`、`GONE`）。

**谁说话**：她的家。她出门在桌面上时只做动作：`pet.js` 不显示状态气泡（气泡只在没有宠物可显示时说一句怎么下载），确认面板只在岛的窗口里（`panel.js` 的 `holdsPanel`），提示音和通知也发给岛的窗口；状态变化、插件冒头时岛自己展开。

**有程序全屏时**：设置 `hideInFullscreen`（默认开）时，她所在的屏上有程序全屏，她和岛都藏起来，确认直接交给终端（`main.rs` 的 `look_fullscreen`，`fullscreen.rs`）。全屏检测在自己的线程里每 1.5 秒看一次；以前任务栏的窗口钩子会在前台窗口变了时马上叫它看，现在没有了，所以让开和回来最多晚 1.5 秒。

## 灵动岛

岛的窗口挂在所在屏幕工作区的顶部正中，平时 760×132（逻辑像素）：和拉她出来要的一样宽，所以窗口只往下长，左上角不动（左上角一动，Windows 会先把旧画面从新的左上角画出来一两帧，岛就往旁边跳一下）。窗口透明的地方点击会穿过去。岛有四种形状：收起（她的圆形小头像、项目、状态、用时）、展开（悬停 140ms 后；或者状态变化时自己展开 3.6 秒）、确认面板、设置。

**收起时的三个位置**（`island.js` 的 `fillCompact`）：左边是她（出门时是她的小头像），中间一行字（会话的状态和步骤，或插件的图标和标题），放不下就省略号，右边贴边是一个短数值（用时，或插件的数值，最多占 60%），别的会话在忙时数值前面一个「+N」。宽度不跟内容变，只看设置 `islandWidth`：窄 240、标准 300、宽 380（`WIDTHS`）；什么都没有时只有她那么宽（`MIN_W`）。这样插件轮换、步骤变化、计时走字都不会让岛伸缩；换成另一个插件或另一个会话时，内容从下面滑上来（`.turn`）。

**监控固定显示**（设置 `monitor.pin`，默认开，「插件」页监控的「设置」里）：开着时监控从轮换的插件里拿出来（`onUpdate`），灵动岛收起时岛往右加宽一段，最右边隔一条细线固定显示它的读数（`.pinned`），中间照常轮换。每个读数留够最长时的宽度（`min-width: 4.8ch`，「100%」「999K」；某次没读到时沿用上一次的读数，不让它消失），所以加宽的那段宽度也是固定的，数字跳动时岛不变宽窄。关掉时它和别的插件一样参与轮换，轮到它时中间那行显示它的读数。轮换时只有中间那段（`.turning`）滑进来，固定的读数不动；滑到一半岛重画（每秒的计时），用负的 `animation-delay` 接着滑，不从头来。

她在岛里只有一个元素 `#island-her`：外层管裁剪框（位置、大小、圆角），里层 `.sheet` 是原尺寸的一格图集，用 `transform: translate() scale()` 缩放。收起时是 0.21 倍、24px 的圆形头像；设置里是 0.28 倍、32px 的头像；展开和面板时是 0.5 倍的全身。都是同一组 CSS 属性，所以头像是"长成"全身的。形状变化全在 CSS 里（弹簧曲线 `cubic-bezier(0.3, 1.45, 0.5, 1)`，0.56 秒），不逐帧改窗口大小：需要更多空间时页面先通过 `pet_panel` 让窗口变大，40ms 后再让岛长大；收回时等动画做完再缩窗口。

**把她拉出来**：在 `#island-her` 上按下并移动超过 5px，岛的窗口扩到 760×440。`#goo` 层里岛的替身、一段脖子、一滴水三块黑色形状套 SVG 滤镜，就像液体一样连在一起。拉过 110px 断开：页面发 `island_release`（她的中心相对光标的偏移），Rust 把岛正在代表的那个 agent 的桌宠窗口（没有就现在建）放到光标下、让它在桌面上（`desktop`），然后跟随光标"携带"她（这时她的窗口完全穿透，按键还在岛的窗口上）。松手由岛的页面发 `island_drop`，或者由 Rust 读到按键已松开。没拉断就松手，她弹回座位。拉完那一下浏览器会在岛上补发一次 click，岛会忽略它（`pulledAt`），不当成"单击打开设置"。

**放回去**：她的窗口被拖动时，Rust 每帧算她的中心离岛下沿多远（280px 内靠近，140px 内够近），通过 `island:reach` 告诉岛的页面（坐标用逻辑像素，和页面的 `window.screenX` 一致），页面伸出一滴去够她。够近时松手：Rust 立刻藏起她的窗口，岛的页面播一段吸回去的动画（`island:absorb`），380ms 后让那只回家（`desktop` 里设为 false，窗口关掉）。在桌面上双击她，她会飞回岛里（`pet.rs` 的 `fly_home`）。

**它自己的外观**（#27）：设置 `islandTopLook`，`{ dark, light }`，各自 `{ bg, op, blur, edge }`；没设时纯黑、一圈 `rgba(255,255,255,0.07)` 的细线、白字，和以前一样，所以老用户什么都不变。`island.js` 的 `lookNow` 按 `pet:update` 的 `winLight`（Windows 的浅色、深色；`island.rs` 的 `look_over_max` 发现模式变了就重画一次）选哪一份；样式是 `style.css` 的 `home-island` 一节（`--i-bg`、`--i-edge`、`--i-line`，浅底上线和字都变深）。页面自己的 `backdrop-filter` 在这里没有东西可模糊，所以不设 `--i-blur`：半透明又开了毛玻璃时，`backplate.rs` 的亚克力底板跟着岛走，收起、展开、确认都垫着（设置不垫），大小变的时候不藏（见上面 #20 的 `movePlate`），静默时不垫。底板用不了时（Windows 10、关了透明效果），设置里不显示「云母毛玻璃」预设和毛玻璃滑块。半透明又没有模糊时，所在屏幕上有窗口最大化（`island.rs` 的 `os::maximized_on`：会有任务栏按钮的窗口、最大化、中心在这块屏上），底色加深到 90%：`look_over_max` 跟着全屏检测每 1.5 秒看一次（只在设了顶部外观、不透明度低于 90%、没有底板模糊、有人在用电脑时），变了才发 `island:over-max`。设置「外观」页的「灵动岛外观」放在灵动岛那一组里，预设是纯黑（默认）、Windows 深灰、云母毛玻璃、深灰 + 状态色光圈（深色），纯黑、Windows 浅灰、近白、云母毛玻璃、白 + 状态色光圈（浅色），上面一句说明这是顶部灵动岛的（`s.ilookTop`）；预览是桌面顶上的一颗岛，展开往下长。

## macOS（刘海里的岛，zhoupengjie/Wakuwaku#32）

从源码在 Mac 上编译、运行（jjme9307 的 PR，Apple Silicon + macOS 26.6 上试过）。家只有灵动岛，和 Windows 上一样。Accessory 应用（不进 Dock、不占菜单栏），`macos-private-api` 让窗口透明。

- **岛在刘海里**（`notch.rs`）：窗口在屏幕顶部正中、层级高过菜单栏；量出刘海的宽高（`safeAreaInsets`、`auxiliaryTopLeftArea` / `auxiliaryTopRightArea`）用 `island:top` 告诉页面，收起时和刘海一样大、顶边直角，鼠标穿过去（`aside`），刘海留给 Atoll 这类 App。左键点托盘图标（`island:peek`）或双击桌面上的她才展开；只有等你批准会自己弹出；展开的岛和从菜单打开的设置，鼠标进去再离开就收，一直没进来 6 秒后收，打字时不收。她的家在刘海里看不见，双击、拖向岛都不会把她收回去（`notch::in_notch`）。
- **多屏不同缩放**：Tauri 给的鼠标位置按主屏的缩放，每个窗口的位置按它自己那块屏的，`screen::cursor` 按窗口所在屏换算；拖她时用逻辑坐标摆窗口（`pet.rs` 的 `place`）。
- **岛在哪块屏**：默认在主屏；有两块以上屏时，托盘和岛的右键菜单多一个「岛在哪个屏幕」，选了记在设置 `islandDisplay`（那块屏左上角的物理坐标 `[x, y]`），`island.rs` 的 `area` 先找它，找不到（屏拔掉、排列改了）就照旧回主屏。
- **互不影响**：Mac 的东西只在 Mac 上跑，Windows 的只在 Windows 上跑。Rust 用 `cfg(target_os = "macos")` / `cfg(windows)`，Windows 专有的部分（`backplate.rs`、`jump.rs`、`fullscreen.rs`、`island.rs` 的 `os` 等）在别处有空实现；页面上 Mac 的行为只在收到 `island:top`、`island:peek`（只有 Mac 会发）或 `window.pet.mac` 为真时才有（`bridge.js`：`navigator.platform` 以 Mac 开头，Apple Silicon 上也是 `MacIntel`）。例如设置刚打开 0.5 秒内的失焦不算点外面、记录设置有没有被用过的监听，都只在 Mac 上；指针线程在 Windows 上每轮只读一次鼠标位置，和以前一样。只在 Windows 上成立的单元测试标 `cfg(windows)`。
- **Mac 上还没有的**：点会话跳到窗口、系统通知、监控插件、PowerShell 插件、开机自启、打包和自动更新、跟随系统强调色、WebView 崩溃监视和省电。

## 软件更新（updates.rs）

只对下载的 exe：从源码树的 `target` 下运行（`data::from_source`，和数据放在项目 `data/` 的判断是同一个）时不检查也不更新，设置里写着「从源码运行」。

- **两个地方**：GitHub 和 Gitee（[gitee.com/zhoupengjie/Wakuwaku](https://gitee.com/zhoupengjie/Wakuwaku)，给国内用户的副本；代码和 tag 同步过去，发布要在 Gitee 上另建，见下面「发布到 Gitee」）。
- **检查**：同时问两边，都带 User-Agent、不带 token：GitHub 是 `GET https://api.github.com/repos/zhoupengjie/Wakuwaku/releases/latest`，404（还没有发布）当作没有。Gitee **不用它的 API**：不登录时每个 IP 只有大约 60 次，用完后一个多小时都返回 403（Rate Limit Exceeded），国内很多人共用一个出口 IP；它的 `releases/latest` 还是"最后编辑的那个"，JSON 里也没有 `draft`、`html_url`、`size`、`digest`。所以改读仓库的 tag，和 `git clone` 一样（`GET https://gitee.com/zhoupengjie/Wakuwaku.git/info/refs?service=git-upload-pack`，pkt-line 格式），取版本最高的 tag（`v` 加数字开头；名字里有 `-` 的算预发布，跳过：只看 tag 分不出 Gitee 上的预发布，所以 Gitee 上的预发布 tag 必须带 `-`）。再按 Gitee 存放附件的固定地址取这个 tag 的 `…/releases/download/<tag>/<exe>.sha256`（`sha256sum` 的格式，也接受只有十六进制）：取得到就说明这一版在 Gitee 上已经发好（脚本先传 exe 再传 `.sha256`），exe 地址和发布页地址（`…/releases/tag/<tag>`）也按固定格式拼；404 说明这个 tag 在 Gitee 上还没有发布，Gitee 这边就当作没有。两边取版本高的；两边版本一样时，校验值优先用 GitHub 的 `digest`，两边都给了却不一样就不装（`mismatch`）。只有一边答不上来时只记日志；两边都答不上来才算检查失败。启动 20 秒后一次，之后每天一次（线程每 10 分钟看一眼是否到期；失败的话一小时后再试，只记日志）。草稿、预发布不算；`tag_name` 去掉 `v` 后和 `CARGO_PKG_VERSION` 按数字逐段比（0.10.0 > 0.9.0，带 `-` 后缀的预发布排在同号正式版前面）。设置 `autoUpdateCheck`（默认开）关掉就不再自己检查；「现在检查」随时可点。
- **先从哪边下载**：设置 `updateFrom`：`auto`（默认）在中国先 Gitee、其他地方先 GitHub；也可以设成 `gitee` / `github`。"在中国"是 Windows 的时区为 `China Standard Time`，或系统语言是 `zh-CN` / `zh-Hans…`（只算一次）。先下的那边失败（下载出错或校验不对）就换另一边，界面上写着正在从哪边下载。发布页（「查看说明」）也打开先下载那一边的。
- **提示**：每个新版本在岛上提示一次（`updateTold` 记着提示过的版本；她不在视线里或有会话在等你时，等下一次 10 分钟的检查再提示），点一下打开设置「通用」页的「软件更新」那一节。
- **更新**（只在用户点「更新」后）：下载附件 `wakuwaku-v<版本>-windows-x64.exe`（下载地址必须在 `https://github.com/zhoupengjie/Wakuwaku/` 或 `https://gitee.com/zhoupengjie/Wakuwaku/` 下；Gitee 的会跳转到 `foruda.gitee.com`）到数据文件夹的 `update-v<版本>.exe.part`，边下边算 SHA-256，和上面定下的校验值比；没有校验值就中止，对不上就删掉临时文件、换另一边，都不行就中止，界面上说明。有确认请求在等用户时先等它处理完。然后把正在运行的 exe 改名为 `<原名>.old.exe`（运行中的 exe 可以改名不能删），新 exe 放到原路径（保留用户的文件名），用 `--after-update <旧 pid>` 启动新 exe（以资源管理器为父进程，脱离她的进程树和作业，和守护进程一样：`detach.rs`），旧的走 `Shared::quit` 正常退出。新 exe 在绑定端口之前先等旧进程退出（最多一分钟），否则端口被占时它只会唤醒旧的然后自己退出。新版运行一分钟后删掉 `.old.exe`（旧版的守护进程可能还开着它，删不掉就隔 10 秒再试几次）。新版启动失败时，旧 exe 被换回原名。
- **手动换回旧版**：新版起不来、`.old.exe` 还在时：结束 wakuwaku 进程，删掉新的 exe，把 `<原名>.old.exe` 改回 `<原名>.exe`。
- **测试**：环境变量 `WAKUWAKU_UPDATE_URL` 当作 GitHub latest release 的 JSON 地址，`WAKUWAKU_UPDATE_GITEE_URL` 当作 Gitee 仓库的地址（读 `<地址>.git/info/refs` 和 `<地址>/releases/download/…`；给了其中一个时只问给了的那边，下载地址也不限于 GitHub 和 Gitee），`WAKUWAKU_UPDATE_FORCE=1` 让源码树里的调试版也检查、更新。日志都以 `update:` 开头。`node test/fake-release.js --exe <新版 exe> --version <版本>` 同时扮演这两边（端口 47390，`/mode?…=1` 在运行中切换各种故障，见文件开头）。
- **发布到 Gitee**：先在 GitHub 照常发布，再把 tag 推到 Gitee，然后运行 `scripts/gitee-release.ps1 -Version <版本>`（私人令牌放在环境变量 `GITEE_TOKEN`，需要 `projects` 权限；令牌放在请求头 `Authorization: token …` 里，放在 multipart 表单的 `access_token` 字段里时上传附件会被拒绝，返回 40001）。脚本从 GitHub 发布下载 exe，按 GitHub 的 `digest` 校验后，在 Gitee 建同名发布（说明照抄 GitHub 的），先传 exe、再传 `<exe>.sha256`，最后像更新器那样（不登录、不用 API）从 Gitee 下载回来再校验一遍。不带令牌运行时什么也不写，只做最后的校验。从国外往 Gitee 上传大约每秒 25 KB，7 MB 的 exe 要几分钟。Gitee 每个附件不超过 100 MB，每个仓库的附件总共不超过 1 GB，满了就删掉旧版本的附件。

## 崩溃报告（crash.rs、relay/）

她自己退出（崩溃）时记下发生了什么，用户同意后把去掉隐私的报告公开发成 Wakuwaku 的 issue。exe 里没有 token：报告先发到中转（`relay/`），token 只在中转里。

- **抓到什么**：
  - panic：release 版 `panic = "abort"`，任何线程 panic 整个进程都会结束。钩子在结束前写 `crashes/panic-<pid>.json`：消息、`文件:行:列`、线程名、调用栈（`模块+偏移`，`RtlCaptureStackBackTrace`）。
  - 原生崩溃（访问违例、栈溢出等）：`SetUnhandledExceptionFilter` 的过滤器写 `crashes/native-<pid>.txt`：异常码、出错的 `模块+偏移`、访问违例是读/写/执行以及是不是空指针附近、调用栈。过滤器不分配内存、不拿锁（堆可能已经坏了），路径在启动时就准备好。写完就以异常码结束进程（release 版的 panic 钩子同样以 `0xC0000409` 结束）：交给 Windows 的话，错误报告（WER）要收集转储，进程会多活 8 到 9 秒。代价是事件日志里没有 WER 的记录。
  - 都没写下来、但退出码是崩溃的（`0xC0000374` 堆损坏、fast fail 等）：守护进程（`crash.rs` 的 `guard`，她自己的 exe 带 `--wakuwaku-guard <pid> <数据目录>`，在她的进程外等她结束）拿到退出码。算崩溃的：`0xC…` 错误级 NTSTATUS（`0xC000013A` Ctrl+C 除外）、`0x80000003`、`0xE06D7363`、Rust 的 101；0、1（任务管理器、`taskkill /F`）和 0xFFFFFFFF（PowerShell 的 `Stop-Process`、.NET 的 `Process.Kill()`）不算。
  - WebView2 的进程挂了（`ProcessFailed`）：她页面的渲染进程退出或无响应，或浏览器进程退出。被杀掉的（`terminated`）只记日志不报告。渲染进程：重新加载页面；两个页面共用一个渲染进程，10 秒内的第二次不再报告。浏览器进程：运行超过一分钟时，她启动一个新的自己然后退出（和更新一样用 `--after-update <pid>` 等她结束；两个窗口都会收到这个事件，只重启一次）。报告的 code 是 `<种类>/<原因>/<退出码>`，比如 `render-exited/crashed/0xC0000005`。
- **守护进程**：只要报告发得出去（有中转地址）、崩溃报告也没关（`crashReports` 不是 `off`），她启动、拿到端口后就开一个（`main.rs`、`crash::spawn_guard`，每个进程只开一次）。发不出去时不开：panic 和原生崩溃的记录下次启动照样变成报告。守护进程不建窗口。它等她结束，生成报告（`crash::after_exit`）。参数里的数据文件夹就是她的数据文件夹。
- **常开的尾日志**：`Shared::log` 每一行都写 `recent.log`（不管 debug 开没开），超过 256 KB 改名成 `recent.old.log` 重新开始；一行里的换行写成 `⏎`，保证一条日志就是一行。每次启动先写一行 `start: pid <pid>, v<版本>`，报告的日志从这一行开始取，不带上一次运行的。debug.log 照旧只在 `debug.on` 时写。
- **报告**：`crashes/<时间>-<类型>-<pid>.json`，文件内容就是要发出去的全部内容：版本、类型、退出码、位置、消息、调用栈、Windows 版本（注册表的 CurrentBuild，≥ 22000 算 11）、WebView2 版本、她的家、运行了多久、时间（UTC，到分钟），以及崩溃前 5 分钟内的日志（最后 60 行，每行最多 300 字）。没有守护进程看到的记录（比如之前关着报告），下次启动时补成报告。等着发的最多留 20 份。
- **脱敏**（`crash/redact.rs`，单元测试列了每条规则的例子）：数据文件夹、exe 所在文件夹、`%USERPROFILE%` 换成占位符；Cargo registry 里依赖的路径缩成 `<crate>/src/…`；代码里用 `crash::private()` 包成 `‹…›` 的部分整段去掉（固定项的名字就是这样标的；本地的 debug.log 里照样有名字）；引号里的内容都换成 `"‹…›"`（只留 JSON 的键：16 个字符以内的纯小写单词、后面紧跟冒号）；别的程序的 exe 名换成 `<app>`（只留 `explorer.exe`、`msedgewebview2.exe` 和 `wakuwaku*.exe`）；邮箱、URL 的主机和路径（只留 GitHub、Gitee、codex-pets.net、本机）、其他路径（只留盘符、大家都有的文件夹和扩展名，如 `D:\…\Pictures\*.jpg`）、UUID、token、长十六进制、IP、主机名；用户名、计算机名、邮件账号、会话的项目名标题任务（只有进程内的 WebView2 报告知道这些）按整词替换；含非 ASCII 字母的词都换成 `‹…›`（日志本身是英文，非 ASCII 的几乎都是窗口标题、文件夹名这类用户内容）。中转收到后再按通用规则脱敏一次。
- **发不发**：设置 `crashReports`：`ask`（默认）、`auto`、`off`。`ask` 时，下次启动 20 秒后在岛上提示一次（`crashTold` 记着最新那份的名字；她不在视线里或有会话在等你时，10 分钟后再看），点一下打开设置「通用」页的「崩溃报告」：显示要发的内容，「以后自动发送」或「不发送」（清掉等着发的）。之后由开关控制。启动就崩、10 分钟内崩了 3 次还没回答过的，守护进程弹 Windows 自己的「是 / 否」对话框问同样的问题，答案直接写进 config.json。中转地址是空的、或从源码树运行（没设测试地址）时什么都不问也不发，只在本地记。
- **发送**：`auto` 时守护进程生成报告后马上发；她启动 20 秒后、之后每 10 分钟看一次，有等着发的就发（失败后 6 小时再试）。一次最多 5 份；每份先改名成 `.sending` 占住，守护进程和她不会重复发（进程半路没了，10 分钟后还原）。在中国（`updates::in_china`）先发 Gitee 的中转，否则先发 GitHub 的；一个发不出去换另一个。设了 Windows 系统代理（注册表 `ProxyEnable` / `ProxyServer`，Clash、v2rayN 都这么设；`proxy.rs`）且环境变量里没有代理时，先走代理再直连。200/201：存进 `crashes/sent/`（带 issue 地址，设置里可以点「查看 issue」）；400 等：存进 `crashes/refused/`，不再发；429、5xx、连不上：留着以后再发。
- **中转**（`relay/relay.js`，一份代码；Cloudflare Worker 直接用，Node 下由 `relay/server.js` 跑）：只收 `POST /report`，要带 `x-wakuwaku: 1`，不超过 48 KB，字段白名单逐个校验（多一个字段就 400）；同一 IP 每分钟 5 次（Cloudflare 的 rate limit 绑定）、每天 20 次（KV，IP 加盐哈希）；全部加起来每天最多新开 30 个 issue。同一个崩溃按签名去重（panic：文件和行，不带列；原生：异常码和 `模块+偏移`；其余：类型和退出码），签名的哈希前 10 位作为标题里的 `[crash:…]`。同一个签名：前 10 次各加一条评论，之后只更新正文里的计数，出现新版本时再评论；issue 已关闭时，旧版本的只计数，更新的版本新开一个 issue 并链接旧的。GitHub 推荐用 GitHub App（issue 由机器人开，你能收到通知），也支持 fine-grained token；Gitee 用私人令牌（建议用小号）。部署步骤见 `relay/README.md`。中转地址填在 `crash.rs` 的 `GITHUB_RELAY`、`GITEE_RELAY`。
- **测试**：`node test/fake-relay.js`（端口 47391）用 relay.js 自己的 `handle()`，issue 存在内存里并打印出来；`GET /issues` 看全部，`GET /issues/<n>` 看一个，`GET /close/<n>` 关掉一个（试回归），`GET /mode?down=1|busy=1|refuse=1|slow=1` 切换故障。宠物用 `WAKUWAKU_CRASH_URL=http://127.0.0.1:47391/report` 指过去（逗号分隔可以给两个；设了它从源码树运行也会发）。`WAKUWAKU_CRASH_TEST=<类型>[:<秒>]` 让她在启动后几秒（默认 10）崩一次：`panic`、`native`（往空指针写）、`exit`（以 `0xC0000374` 结束，什么都不写）、`webview`（WebView2 报 `terminated` 时也报告）。WebView2 这几项要手动触发：她的进程的子进程 `msedgewebview2.exe` 是浏览器进程，它的子进程里命令行带 `--type=renderer` 的是渲染进程；用 `Stop-Process -Force` 杀渲染进程，WebView2 报的是 `crashed`，两个页面都会重新加载；她运行超过一分钟后杀浏览器进程，她会重启自己。测试副本也有守护进程，它不建窗口。release 版的 panic 会结束进程；调试版的 panic 只结束那个线程，记录要等下次启动才变成报告。中转的单元测试在 `test/relay.test.js`，脱敏和记录解析的在 `cargo test crash`。

## 设置（长在岛里）

没有单独的主窗口。设计和手感见 `docs/prototypes/island-settings.html`。

- **默认宠物**（Claude小姐，`pets/claude-chan/`，仓库里跟踪，`.gitignore` 只放过它）：`fetch.rs` 用 `include_bytes!` / `include_str!` 把 `spritesheet.webp` 和 `pet.json` 打进 exe，首次运行一只宠物都没有时 `seed_default_pet` 直接写到 `wakuwaku-data/pets/claude-chan/`（和从宠物库下载的一样走 `put_pet`：先把 pet.json 和图写进 `pets/.claude-chan.new/`，再一次改名放到位；扫描跳过点开头的文件夹；写完调 `data::forget_pets` 让缓存作废。以前先图后 pet.json 直接写在原地：扫描恰好落在两步之间时只认到图，名字成了 id、作者为空，而 `pets()` 的缓存只看 pets 文件夹本身的修改时间——往子文件夹里补 pet.json 不改它，并且它按时钟节拍（约 15 ms）走，同一节拍里的两次改动看不出来——于是整次运行都错，2026-10-13 测试会话的新用户运行里碰到过），不再从 codex-pets.net 下载（国内访问慢）；写不进去才退回下载，再不行打开「宠物」页。exe 因此从约 7.4 MB 变成约 10.8 MB（2026-10-13：7,450,112 → 10,756,096 字节）。pet.json 里的出处说明原样保留（原始形象作者 B 站 UP 主 ZipZipPipe，非官方二创），README 的致谢里也写了。宠物库照样能下载别的宠物。
- **Antigravity（agy）**（2026-10-13，0.2.7）：`connection.rs` 在 `~/.gemini/config/hooks.json` 里加一个名为 `wakuwaku` 的块（这个文件按块名分工具；agy 命令行和 Antigravity 2.0 都读它，独立 IDE 不运行 hook）：SessionStart、PreInvocation（每次调模型；`invocationNum` 为 0 是一轮的开始）、PostToolUse（matcher `""`）、Stop。不挂 PreToolUse：它必须回一个 `decision`，回不出来就会拒掉工具。命令是 `<程序> --wakuwaku-agy-hook <事件名>`（payload 里没有事件名）。实测 agy 1.3.4：它用 `cmd /c "<命令>"` 跑，命令里的双引号被按 Go 的方式转义成 `\"`，cmd 认不出，所以路径不能加引号；有空格时用 8.3 短路径（`agy_program`），没有短路径就报错不装（`error_kind` 的 "path"）。hook 失败时 cmd 的中文报错不是 UTF-8，agy 这一轮会因「invalid UTF-8」直接失败，所以 `main.rs` 的 `agy_hook` 一启动就先把回答打印出来（Stop 回 `{"decision":"allow"}`，其余回 `{}`），之后不管发生什么都不再往外写；再把事件加上事件名发给 `/hook?agent=agy`。连接上限 100 ms：她没开时，Windows 连没人监听的本机端口要重试 2 秒，agy 每一步都会白等。找窗口的进程链只在会话开始和每轮第一次调模型时算。实测一个 hook：她开着约 0.1 秒，没开约 0.2 秒。`events_agy.rs` 把事件翻成和 Claude Code 一样的消息（`agent: "agy"`，会话 = `conversationId`，项目 = `workspacePaths[0]`）。工具那一步用它自带的描述（`toolCall.args.toolAction`，比如 "Creating hello.txt"）。用户的提问和最后的回复从对话记录（`transcriptPath` 末尾 256 KB：USER_INPUT 的 `<USER_REQUEST>`、最后一条带 content 的 PLANNER_RESPONSE）里读，格式没公开文档，读不到就不显示。放到后台的命令（`run_command` 的 `WaitMsBeforeAsync`；实测 agy 1.3.4，2026-10-14）：模型先回答，`Stop` 带 `fullyIdle: false`（她当作还在干活）；命令跑完才来它的 PostToolUse，记录里多一步 SYSTEM_MESSAGE，模型从 `invocationNum` 0 再调一次，最后 `Stop` 带 `fullyIdle: true`。这第二次开始还是同一轮：轮次按它回答的那条 USER_INPUT 的 `step_index` 认（`with_transcript` 的 `userStep`，消息里的 `turn`，和 Codex 的轮次一样走 `state.rs`），不然一轮算两轮，时长也只从第二次开始算。会话开始、每轮开始和 `Stop` 各在日志里记一行（`events_agy::log_line`：`fullyIdle`、`terminationReason`、会话 ID 前 8 位），用户报「时长没记上」时看数据目录的 recent.log（或放了 `debug.on` 后的 debug.log）就知道 `Stop` 来没来、是什么。write_to_file 等写文件的工具算作改动（`state.rs` 的 `EDIT_TOOLS`）。没有「等你批准」。欢迎卡片三个工具并列（外观会话的规格）：按钮两行（工具名，下面是状态），列数等于本机有的工具数；名字按 `I18n.list` 列举（「A、B 或 C」）；只有 Antigravity 时副标题不提批准；岛上提示和空会话提示列出本机有的工具（`main.rs` 在 `unconnected` 时带上 `tools`，即 `connection::tools_here`）。

- **新用户引导**（2026-10-13，外观会话的规格）：Claude Code 和 Codex 都没连接时（插件、settings.json 的 hooks、Codex 的 hooks 一样都没有），设置「通用」页最上面是欢迎卡片（`settings.js` 的 `welcomeBlock`），打开设置时滚到它（`showPart`：连接这段在欢迎卡片显示时换成它；之后页面又重画——默认宠物放好、快照回来——再对准一次，除非用户自己滚过）。卡片里 Claude Code 和 Codex 平权（用户：「不应该平权吗」）：电脑上两个都有、或者看不出来时，两个一样大的按钮并排「一键连接 Claude Code」「一键连接 Codex」；只有一个时只显示它，占满一行，标题也只说它（`toolsHere`：Codex 看它的文件夹，hooks 状态不是 `absent`；Claude Code 看 `~/.claude`（或 `CLAUDE_CONFIG_DIR`）在不在，快照里的 `claudeHere`）。按钮用 Windows 默认的强调色填充（深色时它的浅色调配黑字，浅色时深色调配白字）。每个按钮自己变成它的状态（`toolState`）：写入中转圈；Claude Code 写好了还没事件（`claudeSeen`：她这次启动以来有没有收到过，`server.rs` 在 /hook 上记，和 Codex 的 `codex_seen` 一样）是绿框「✓ Claude Code 装好了」，Codex 写好了等信任是黄框「Codex 还差一步」，都不能点但不变淡；写不进去是「重试连接 …」。按钮下面写每个还差什么：Claude Code 是绿底「新开一个会话…已经开着的要重开一次」，再一行写进了哪个文件（快照的 `settingsShown`，家目录写成 ~）、备份在哪（`settingsBackup`；原来没有这个文件就是「新建了 …（原来没有这个文件，不用备份）」），附「撤销」；Codex 是黄底「在 Codex 里输入 /hooks…信任 Wakuwaku」；写不进去是红框，开头说是哪个工具的哪个文件，然后完整路径、原因，再说怎么办（`connection.rs` 的 `error_kind` 从原因里看：被别的程序占着说「关掉它再试」，JSON 有错说「改好再试」，别的「再试一次」；Claude Code 的还加「或者改用下面的插件安装」并自动展开插件）。再下面三行说改了什么（第一行按显示的工具说 settings.json、hooks.json 或两个都说）；分隔线下是折起来的「Claude Code 也可以用插件安装」（只在 Claude Code 在、还没装时）。卡片一直是蓝底，颜色由各自的状态表达。连上之后（任何一个收到过事件，或用的是插件）第一次打开设置时只剩一行「✓ 已连接 Claude Code」（或 Codex，或「Claude Code 和 Codex」），下面淡色「N 个会话在线 · 管理 ›」，另一个在电脑上但还没装的，行尾一个「连接 …」小按钮；关掉设置时有连接才记 `onboarded`（`island.rs` 的 `settings_closed`；以前一关就记，点一下外面就再也不提醒），之后卡片就没了，连接的管理还在页面下面的「连接」那段。没连接时，启动时只自动打开一次设置（`welcomeOpened`；以前每次启动都开，只把她当桌宠的人每次开机都被打扰，还关不掉，2026-10-13 用户），说过「以后再说」/「不用提醒」（`connectHintOff`）就一次也不开。卡片最下面有「以后再说」（和岛上的「不用提醒」是同一个设置：岛上的提示也收起来）；说过以后、还没连接时，卡片收成通用页顶部的一行淡色「还没连接 Claude Code 或 Codex · 去连接 ›」，点「去连接」或从别处要求打开「连接」时重新展开。程序这边记着「有没有连接」（`connection.rs` 的 `look_connected` / `note_connected` / `seems_connected`：启动时、设置读快照时、连接或移除后看一次，任何 hook 事件到了就算连上），不在每次重画时去读设置文件；岛的数据里是 `unconnected`。岛上（`island.js` 的 `isHinting`：没连接、也没点过「不用提醒」`connectHintOff`）：她头像右上角一个强调色的点，收起的岛上文字换成「还没连接 ›」（强调色，不显示计时；悬停提示「还没连接 Claude Code 或 Codex，点一下去连接」）；展开的岛开头是「还没连接 Claude Code 或 Codex」和一行「去连接 ›」，底下照常，最下面一个小小的「不用提醒」；点岛（除了部件那几行）都是打开设置的欢迎卡片。
- **打开**：单击岛（不是拉她、不在确认面板上），或者托盘菜单「设置…」、勿扰时左键托盘、只有宠物时双击她、首次运行（没连接 Claude Code 时开在「连接」页；默认宠物放不到本地时开在「宠物」页）。都走 Rust 的 `island::open_settings`，它再发 `island:settings` 给岛的页面；页面还没起来时先记着，起来后再发。
- **焦点**：只有设置会拿键盘。页面打开设置时调 `pet_keyboard(true)`，Rust 先记下当前前台窗口（`focus.rs`），再让岛的窗口可获得焦点并聚焦。悬停展开、自动展开、来了确认请求都不碰焦点，所以终端里打的字不会被吃掉。
- **收起**：Esc、点 ✕、点面板头部，或窗口失去焦点（点岛外面：`WindowEvent::Focused(false)` → `island:blur`）。鼠标移开不收起。收起时页面发 `settings_closed`，Rust 让窗口变回不可获得焦点；如果前台还是岛（Esc、✕ 的情况），把焦点还给记下的窗口。
- **只有宠物时**：打开设置会临时升起一颗岛（`Island.temp`，`payload.islandTemp` 让页面把自己当成"岛开着"），收起后等岛缩回（620ms）再藏起来。设置里切成灵动岛模式，岛就留下来。
- **数据**：页面打开时取一次快照（`settings_get`），之后 Rust 在状态、确认请求、设置变化时推 `settings:changed`（每秒最多 4 次）。改动先在页面上显示，再发 `settings_set` 补丁，Rust 按白名单校验（`settings.rs` 的 `is_ok`），大小、显示方式、出门通过窗口去改，她的位置不跳。
- **高度**：面板宽 520px，正文高度不超过屏幕（`screen.availHeight` 减去头、页签、脚），超出的在正文里滚动；页签切换时岛的高度用弹簧过渡。

## 精灵图

Codex pet v2 图集为 1536×2288，8 列 × 11 行，每格 192×208。v1 是 1536×1872、9 行，没有注视动作。

| 行 | 网站上的名字 | 帧数 | 用于 |
| --- | --- | --- | --- |
| 0 | Idle | 6 | 空闲：播一轮再停 4 秒 |
| 1 / 2 | Run right / left | 8 | 空闲走动、拖动 |
| 3 | Waving | 4 | done、打招呼 |
| 4 | Jumping | 5 | TaskCompleted、单击 |
| 5 | Failed | 8 | error、工具失败 |
| 6 | Waiting | 6 | waiting |
| 7 | Running | 6 | working |
| 8 | Review | 6 | review |
| 9 / 10 | Look around | 8 + 8 | 16 个注视方向：从正上方顺时针，每 22.5° 一档 |

自己画一只：图集的完整规格（每行放什么、角色在格子里的位置、16 个方向、`pet.json`、放哪个文件夹）见 [MAKING_A_PET.md](MAKING_A_PET.md)，一帧一张的 PNG 用 `tools/make_sheet.py` 排成图集。

**每个 agent 一只**（设置 `agentPets`，如 `{ "codex": "owl" }`，键是 `data::AGENTS`：`claude`、`codex`、`agy`、`grok`）：`payload()` 的 `sprite` / `spriteVersion` 是目前显示的会话的 agent 的那只（`data::pet_for`；Claude Code 的会话没有 `agent`，算 `claude`），没设、设的那只不在了、或者没有会话，就是 `pet`。设置的「外观」页里每只宠物下面点 agent 的名字来设。换图集时页面先把新图载入并解码（`sprite.js` 的 `loadSheet`，`Image.decode()`，载过的留着），完成后才换上去，期间一直画旧的；后要的赢，载入失败也照样换上（`sheetSwitcher`）。第一张和「没有宠物」立刻换。

精灵图从数据目录读，经 Tauri 的 asset 协议给页面：`http://asset.localhost/<编码后的绝对路径>`（`data::asset_url`），宠物文件夹在启动时加进 asset 的允许范围。图库的预览用站点的 `posterUrl`（单帧 192×208）；`previewUrl` 是所有帧连成的一长条，缩进格子里只剩一条线。

## 实测（Windows 11，125% 缩放，空闲）

| | Electron 版（旧） | Tauri 版 |
| --- | --- | --- |
| 发布文件 | 88.5 MB（便携版 exe） | 约 5 MB（只有宠物模式时是 3.7 MB；HTTPS、注册表、通知加了 1.3 MB） |
| 进程数 | 4 | 7（1 + WebView2 的 6 个） |
| 私有内存合计 | 约 183 MB | 约 185 MB |
| 空闲 CPU（3 分钟平均） | 1.13% | 0.89% |

CPU 主要花在 GPU 进程和渲染进程上，两边几乎一样（页面是同一个）。Windows 上 WebView2 本身就是 Chromium，所以内存和 CPU 跟 Electron 持平，省下来的是安装包体积。

### 省下来的（#13、#14）

- **她回家后桌宠的页面也收起来**（`pet.rs` 的 `hide_window`）：只藏窗口时，WebView2 仍当页面可见（`document.hidden` 为假），`pet.js` 照样按节拍画她、闲逛时还请 Rust 挪一个看不见的窗口；桌宠窗口又关了后台节流。现在连 WebView 一起藏（`Webview::hide`，即 `SetIsVisible(false)`），窗口建好时页面也先藏起来；`pet.js` 在 `document.hidden` 时不画、不闲逛，显示时整份状态（含 `quiet`）随 `pet:update` 补上。
- **轮询改成等待**：监控线程改成条件变量，有事（静默开始或结束、设置改了）才叫醒；岛上她的头像那个 60 ms 的定时器只在有头像要播时才开。
- **静默时**：WebView2 的浏览器、GPU、渲染和工具进程也设 EcoQoS（只动执行速度那一位，Chromium 自己设的计时器精度保留），各页面的内存目标设为低（`SetMemoryUsageTargetLevel`），回来时设回。
- **线程和进程**：Tauri 的异步运行时只开 2 个工作线程（默认每个逻辑核一个）；网络服务并进浏览器进程（`NetworkServiceInProcess2`），少一个进程、约 40 MB；存储服务已经没有对应开关，仍是单独的进程。
- **显卡**：WebView2 默认在 Windows 给它的「省电」显卡（核显）上渲染，不管她的屏是哪块显卡输出；她那块屏在独显上时，每一帧都要从核显拷到独显，核显忙（比如 Chrome 在放视频）时她的动画就掉帧。现在启动时用 `D3DKMTOpenAdapterFromGdiDisplayName` 查出她那块屏（家所在的屏，启动时是主屏）的显卡，确认它能渲染（远程桌面的虚拟显示器、间接显示器不算），用 `--use-adapter-luid=<高位>,<低位>`（十进制）交给 WebView2；屏在核显上就用核显，不会为她叫醒独显。只在启动时定，换屏、插拔显示器后要重启她才跟着变。

在 Intel Core Ultra 9 386H + RTX 5090 Laptop 上实测（调试模式，有会话在跑，没人碰键盘鼠标）：

| | 改之前 | 改之后 |
| --- | --- | --- |
| 有人在、不碰她：整树 CPU | 60.1 ms/s | 48.4 ms/s |
| 有人在、不碰她：唤醒 | 849 次/秒 | 697 次/秒 |
| 隐藏的桌宠页面 5 秒画了几帧 | 11 | 0 |
| 主进程线程数 | 51 | 37 |
| 静默：唤醒 | 301 次/秒 | 159 次/秒 |
| 静默：工作集 | 549 MB | 171 MB |
| 渲染所在的显卡 | 核显（屏在独显上，#13 测的） | 独显（输出她那块屏的） |

## 每个 agent 一只桌宠（multi）

岛还是一个，行为不变：显示最需要你的会话，用那个 agent 的角色。桌面上可以同时有多只桌宠，每个 agent 最多一只，各自只反映自己 agent 的会话。

- **窗口**：`pet.rs` 每个 agent 一个窗口，标签 `pet-<agent>`（`capabilities` 写 `pet-*`），页面是 `index.html?role=pet&agent=<agent>`。状态在 `Shared.pets`（`pet::Pets`，每只一把锁；遍历时一只一只拿，不同时拿两把）。设置的 `desktop` 打开时才建窗口，关掉就关窗口（`pet::open_or_close`，`Shared::apply_visibility` 里每只看一次）：藏起来的 WebView 仍占着它的内存。没有窗口的 agent，`pointer` 线程、`ticker` 都直接跳过。
- **各自的会话**：`state.rs` 的 `get_for`、`list_for`、`mood_for`、`seen_for` 只看那个 agent 的会话（`state::agent_key`：Claude Code 的会话没有 `agent`，算 `claude`）。每只的 `pet:update` 是 `Shared::pet_payload`：岛的那份里共用的部分（设置、语言、静默、插件……）加上自己 agent 的会话和自己 agent 的角色（`data::pet_of`），带 `agent`；页面只收自己 agent 的。会话的反应（`pet:react`，`state.rs` 在里面写上会话的 agent）只发给岛和那个 agent 的桌宠；她自己的反应（没有会话）发给全部。点一下桌宠：只有它跳，跳到它自己 agent 的会话的窗口。指针移到它上面：只把它自己 agent 的结束算看过。眼睛跟光标看的是它自己 agent 的心情。
- **岛代表谁**：`Shared::island_agent` 是岛正在显示的会话的 agent（没有会话就是 `claude`）。岛上、托盘的「放她出来 / 叫她回家」，设置「通用」的「她在桌面上」快捷开关，从岛里把她拉出来，都是对这只；桌宠自己的右键菜单对它自己（`tray::Over::Her(agent)`，菜单项 `out:<agent>`、`corner:<agent>`）。岛上显示的那个 agent 的桌宠在桌面上时，岛里放她的头像，不放她本人（`herOut`）。
- **确认面板和通知**：桌宠在桌面上的 agent，确认在它头顶、做完的摘要在它旁边，岛不再自己打开（下一节）。
- **位置**：每只各自记在 `spots`；没有记过的，从右下角起，按 `data::AGENTS` 的顺序每只往左挪一个窗口宽，不会叠在一起。大小、闲逛、眼睛跟光标是全部桌宠共用的设置。
- **设置**：「外观」页「各个 agent 的角色」每一行有「放到桌面上」开关（`data-agent-out`，改 `desktop`）。
- **不叠在一起**（`pet.rs` 的「Room between them」）：同一块屏上两只的脚（窗口里她的中线，`Win::bottom`）左右至少隔她一个身宽加 20 逻辑像素（`room`，按那块屏的缩放）；不同屏互不相干。走路时（她自己闲逛、`/debug/walk`、被推开）目的地先截在别只前面（`clip_walk`：往别只走就停在 room 外，离开不拦），每一步再看一次，别只正在走就按它现在的位置，快撞上就停。放下时（拖完松手、从岛里拉出来松手、启动或打开时回到记住的位置，`make_room`）落在别只身上就走开：往左有空位、而且不用穿过别只就往左，不然往右（`free_spot`；身上压着的那只不算穿过），用跑步的样子过去（`pet:drag`）。几只同时放下（一起启动）时各自先按别只要去的地方（`heading`）规划，到了再看一次，最多三次。面板或泡泡开着的那只不会被推开，别只绕开她。实测（2026-10-16，测试副本）：三只启动时放在各差 30px 的位置，各自走开后间隔正好 126px；之后 3 分钟，闲逛加每 8 秒硬让一只朝另一只走 600px（共 22 次），每 0.5 秒量一次，最近的两只一直是 126px，没有更近过。
- **单击和开新对话**（`launch.rs`）：单击桌宠（等过双击的 500ms）跳到她显示的会话的窗口；她 agent 一个会话都没有时，开一个那个 agent 的新终端：Windows Terminal（`wt.exe`，在 `%LOCALAPPDATA%\Microsoft\WindowsApps` 或 PATH 里找）开新分页 `wt -w 0 new-tab -d <文件夹> --title <命令> powershell.exe -NoExit -Command <命令>`（命令 `claude` / `codex` / `grok` / `agy` 是 npm 的 `.cmd`，要经 shell 跑；跑完 PowerShell 留着），没有 Windows Terminal 就开一个 PowerShell 窗口。文件夹是设置 `agentDirs` 里那个 agent 的（「外观」页「各个 agent 的角色」每行的「项目文件夹」），没设、或那个文件夹不在了就用家目录。设置 `clickAction` 设成 `terminal` 时单击一律开新终端。桌宠的右键菜单有「开新对话」（同上）和「开网页版」（`rundll32 url.dll,FileProtocolHandler <网址>`，默认浏览器）。测试副本设 `WAKUWAKU_LAUNCH_DRY` 时只把要跑的写进日志，什么都不开。
- **测试副本**：`/health` 带 `pets`（每只的窗口：`made`、`shown`、位置），`/debug/walk` 可以带 `agent`。

实测（2026-10-16，Windows 11，release 版测试副本，三个 agent 各有一个干活中的会话；每种情况等 20 秒，3 秒一次取 3 次平均；她的进程加上它的 WebView2 进程）：

| | 进程（渲染进程） | 私有内存 | 工作集 | 6 秒内的 CPU 时间 |
| --- | --- | --- | --- | --- |
| 岛 + 0 只 | 6（1） | 187 MB | 92 MB | 0.09 s |
| 岛 + 1 只 | 7（2） | 220 MB | 257 MB | 1.17 s |
| 岛 + 2 只 | 8（3） | 280 MB | 417 MB | 1.23 s |
| 岛 + 3 只 | 9（4） | 313 MB | 497 MB | 1.44 s |
| 再全部关掉 | 6（1） | 208 MB | 265 MB | 0.16 s |

每只桌宠一个自己的渲染进程，私有内存多 30–60 MB；浏览器、GPU、工具进程共用。CPU 主要是第一只：她一动，GPU 进程就要一直合成透明窗口；第二、第三只加得不多。全部关掉后窗口和渲染进程都没了，私有内存多出的约 20 MB 留在浏览器进程里。工作集受静默时的内存目标影响（没人碰时被压低），看私有内存比较准。

## 通知和确认在桌宠旁边（beside）

某个 agent 的桌宠在桌面上（`desktop`）时，它的会话的事都在它旁边说，岛不替它说；桌宠不在桌面上的 agent 照旧走岛。

- **确认**（PermissionRequest：权限、AskUserQuestion、ExitPlanMode）：`asks::agent_of` 看确认是哪个 agent 的（`from` 为空是 Claude Code），`main.rs` 的 `asks_for` 给每个页面它自己那份：在桌面上的桌宠拿自己 agent 的，岛拿其余的；`push_asks` 每个页面各发一份，桌宠出去或回家（`desktop` 变了）、桌宠页面准备好时都重发一次，等着的确认就跟着搬过去。回答按 id，在哪个页面答都一样回到原来那个 hook；排队、逾时、终端先答了就收走，都和以前一样。两个 agent 同时问，各自在各自的桌宠头顶。页面收到和上次一样的清单不重画：别的页面答了一个，`push_asks` 会把所有页面都重发一遍，重画会让按钮又睡 600ms，点下去没反应（2026-10-16 测出来的）。
- **面板在她头顶**：`panel.js` 量好大小交给 `pet_panel` → `pet.rs` 的 `set_panel`，窗口长大，她在屏幕上的位置不动。她所在那块屏的工作区里，她上面放得下就放上面；放不下而下面放得下就放下面（窗口从她头顶开始，她在窗口最上面，页面加 `.below`，`#stage` 改成 `column-reverse`）；两边都放不下就放空间大的那边。左右挡在屏幕边上时，窗口推回屏幕里，她在窗口里往回挪（`pet:shift` 的 `x`）。她的脚在哪由 `Win::bottom` 算（面板在下面时是窗口顶加她自己的高度），记住的位置、岛伸手够她、眼睛跟光标都用它。面板开着时她不闲逛（`pet.rs` 的 `walk` 拒绝，正在走的停下；`pet.js` 的 `isFree`）。
- **打字**：和岛一样，只有点进输入框（AskUserQuestion 的「其他答案」）才拿键盘（`set_keyboard`：窗口变成可聚焦、拉到前面），答完或面板收起就把焦点还给原来的窗口（`prev_focus`，跳过她自己的窗口，所以在两只桌宠之间点来点去还是还给终端）。
- **做完的摘要**：她 agent 的会话做完、改好、出错（她自己那份 `get_for` 是 done / review / error）时，`pet.js` 的 `fillSummary` 在她旁边放一个泡泡，内容和岛展开时一样：会话名、怎么结束的（改了几个文件）和用了多久、回复（最多三行）；「显示具体内容」关掉时只有怎么结束的。它跟着状态走：「提醒停留」到了（`state.tick`）或者看过了（指针移到她或泡泡上：`pet.rs` 的 `hover` → `seen_for`）就回到闲着，泡泡就收起。泡泡固定宽 240px（和面板一样不随她的大小缩放），窗口为它长大的方法和面板一样（`panel.js` 的 `report` 量面板或泡泡里正在显示的那个）。确认来了，泡泡先藏起来（`.asking`）。
- **岛不自己打开**：岛显示的会话是这种 agent 的（`herOut`）时，心情变了也不展开（`island.js` 的 `nudgeFor` 那一行）；岛照样显示状态，点开也照样能看。系统通知和提示音不变。

实测（2026-10-16，测试副本，两块屏：主屏 100%、左边那块 150%）：Codex 的确认只出现在 Codex 的桌宠头顶，允许 / 拒绝 / 交给终端都回到对的 hook；Claude 和 Codex 同时问，各在各的；Codex 的桌宠收回去，它的确认搬到岛上。桌宠在主屏最上面：面板在下面，她的位置一个像素都没动；在 150% 那块屏的左上角：面板在下面、往右推回屏幕里，她也没动。做完的泡泡：会话名、「改好了 · 2 個檔案 · 花了 0:00」、回复；8 秒后收起，窗口回原大小；指针移上去立刻收起；「显示具体内容」关掉只剩怎么结束的。「把焦点还给原来的窗口」在测试环境里看不到前台窗口，没量到。

## 英雄联盟对局助手（lol.rs、lcu.rs）

只用 Riot 官方的接口：对局中读游戏自己开在本机的 Live Client Data API（`https://127.0.0.1:2999/liveclientdata/…`：`gamestats`、`eventdata`、`playerlist`、`activeplayer`），选英雄时读客户端的 LCU（`LeagueClientUx.exe` 旁边的 `lockfile` 给端口和密码，Basic auth `riot:<密码>`）和 Riot API（`league-v4 entries/by-puuid`，用户自己的个人金钥，设置 `riotApiKey`）。不读游戏内存、不注入、不替玩家操作、不截图识别。两个本机接口用 Riot 自己的证书，TLS 库对不上 127.0.0.1，不验证（流量不离开本机）；Riot API 照常验证。

- **Riot 的政策**（developer.riotgames.com/docs/lol 的 Game Integrity）：不能提供游戏里看不到、有优势的信息，不能替玩家做决定，不能找出游戏刻意隐藏的玩家。所以不做敌方召唤师技能、大招计时；说的都是中性的事实（「小龙 0:42 后刷新」，不说「去插眼」）；选英雄只查自己和组队的队友（Solo/Duo 里别的队友被隐藏，`lcu::to_look_up` 只认自己和 lobby 成员的 PUUID）。Overwolf 的 Riot 合规指南更严，明确禁止敌方召唤师技能计时和指挥玩家行动的通知。
- **分工**（`lol::view`，放进每只桌宠的 `pet:update` 的 `game`）：claude 最近一个要刷新的目标一行（一分钟内警示色，没有要刷新的就收起）；grok 小龙 / 男爵 / 先锋的刷新倒数（按剩余时间排，最多三行）、我方五人一人一行「英雄 闪／传」（Live Client 没有任何人的冷却，只有带了哪两个；技能名按 `rawDescription` 里游戏自己的名字对成固定的简称 `spell_short`，客户端是日文也一样；英雄名照客户端给的；`position` 是 `NONE` 或空的不显示），泡泡宽 300px；codex 对方新出的关键装备（`KEY_ITEMS`，名字用游戏给的、跟客户端语言）、出装或阵亡时自己的 CS / 每分钟、KDA、金钱，选英雄时自己和组队队友的段位；agy 每 5 分钟显示 15 秒双方的击杀（按死的是谁的英雄算）、小龙、塔（`Turret_T1_…` 是蓝方 ORDER 的塔）（`lol::tally`，从 eventdata 累计），阵亡时复活倒数（playerlist 的 `respawnTimer`）。
- **买装的判断**（`coach_moment`）：自己的装备数量变多或多了新的 ID，**而且** `currentGold` 同时减少，才算买了。维尔戈的被动附身会把整套装备换成对方的、金钱不变，变身换装的英雄也一样，都不算（第一场真实对局测出来的，2026-10-16）。Live Client 没有地图坐标，「回泉水」只能用「买了东西」代替。页面自己用 `gameTime` + 读到时的时间往下走钟，只有内容变了（或 30 秒）才重画。
- **目标的时间**（2026 赛季：小龙 5:00 首刷、之后每次被杀 5:00 后，远古龙 6:00 后；男爵 20:00 首刷、6:00 后；先锋 14:00 一次）是 `lol.rs` 的常量，没从游戏读到，换赛季要对一下。
- **侦测**：每 5 秒用进程快照找 `League of Legends.exe`（几毫秒）；对局中每秒读一次（玩家列表每 5 次读一次）。连不上三次或进程没了就当对局结束，全部恢复。没有对局时，看 `LeagueClientUx.exe` 的 gameflow phase，选英雄（`ChampSelect`）时才读 session、查段位（每个 PUUID 查一次）。
- **全屏例外**：对局中，`fullscreen.rs` 不把 `league of legends.exe` 当全屏，桌宠不躲；游戏要用无边框模式（README）。
- **设置**：「通用」页「英雄联盟对局助手」开关（`lol`）、「Riot API 金钥」（`riotApiKey`，只送 Riot API，不进日志和崩溃报告）。
- **测试**：`tools/lol_fake.py` 假装 Live Client、LCU 和 Riot API（`WAKUWAKU_LOL_URL`、`WAKUWAKU_LCU_URL`、`WAKUWAKU_RIOT_API_URL`），可以加事件、改时间、换装备、换 phase；`src-tauri/fixtures/lol/events.json` 是 Riot 官方的事件样本（时间都是 0，测试里另给时间）。2026-10-16 第一场真实对局（日文客户端）测出：维尔戈附身被当成买装、`position` 为 `NONE` 时显示出键名、技能名是客户端的日文，都已改；台服用语（预示者、巴龙、长老龙，在 zh-TW 里）。

## 本局十人（scout.rs、src/scout/）

像 Porofessor 那样：对局载入时，画面中间一块面板，左边我方、右边对方，每人一行：英雄图示和名字（客户端的英雄清单 `/lol-game-data/assets/v1/champion-summary.json`，跟客户端语言；图示走 CommunityDragon `raw.communitydragon.org`）、Riot ID、段位和这季胜败（league-v4，单双排没有就看弹性）、这只英雄的熟练度（champion-mastery-v4，没有就写「第一次玩这只」）、最近 10 场（match-v5）的胜场、平均 KDA、这只英雄几场几胜、3 连胜／连败以上。

- **谁在这局**：spectator-v5 `active-games/by-summoner/{自己的 PUUID}`（PUUID 和平台从 LCU 拿），游戏载入后才有（刚开始载入可能还没有：每 5 秒再问，最多 12 次）。选英雄阶段被隐藏的队友不查（Riot 政策），载入后十个人是游戏自己公开的。
- **四只宠物的战情室**（2026-10-21）：面板的每一块由一个 agent 的宠物「讲」，用它的颜色（`data::pet_of` 的宠物和颜色，`pets_of` 放进 `scout:update`），头像播 idle 那一行（图集四分之一大小），旁边一句它看到的重点。自己的战绩：Claude 牌位（单排徽章、分数、胜败长条，弹性积分在下面）、Grok 标签（百万富翁、某英雄爱好者、连胜连败、手感火热／冰冷、Riot 的 hotStreak 和 veteran）和专精前三、Antigravity 位置（单排最近 100 场按 `teamPosition` 统计，后台慢慢读，存硬盘）、Codex 最近 10 场（英雄、召唤师技能、主符文和副系、胜负、模式、K/D/A、补兵和每分钟、击杀参与、装备、十个人的头像、几天前和时长；放不下用滚轮）。十人：上面一排四只宠物各一句（Claude 双方平均段位、Grok 最值得注意的人：连胜、专精最高、第一次玩、Antigravity 组队几组、Codex 读取进度），每人一张卡片（头像、技能和符文、标签：第一次玩、百万富翁、本命＝最近 10 场 6 场以上玩这只、连胜连败、主玩位置、最近 10 场胜负小方块、专精、KDA、这只几场几胜、牌位徽章和胜率长条）。**组队**：同队两人最近 10 场里共有 2 场以上一样的对局就算一组，连着算（`groups_of`），头像右下角编号、左边同色。
- **图示**：英雄、头像、牌位徽章来自 CommunityDragon；装备、召唤师技能、符文来自 Data Dragon（`load_ddragon`：最新版本、`summoner.json` 的 id 对名字、`runesReforged.json` 的 id 对图示，以及**介面语言的英雄名**：zh-TW 用 `zh_TW`，日文客户端也显示台湾的英雄名）。对局快取第 2 版（`KEPT`）存整场需要的栏位，旧的会重读。zh-TW 照台服用语：专精（不是熟练度）、击杀参与、单排、弹性积分、随机单中。
- 实测：自己的战绩（钻石 I 27 分、维尔戈专精 242 万＝百万富翁、单排 100 场中路 64 场）；十人用台服一场正在打的弹性积分（两边五排，各被认成一组；双方平均翡翠 II／IV；某人 3 连胜）。
- **整季**（2026-10-21）：单排这季每一场。场数照 league-v4 的胜＋败（不用知道赛季从哪天开始），场次清单 `queue=420` 一页 100 个往前翻到够为止；每场细节一次请求，存硬盘（294 场第一次约 4 分钟，之后只补新的）。`season_of` 按英雄统计（场数、胜场、K/D/A、补兵、分钟、位置、这些场的 id），`roles_of` 按位置（再分英雄）。专精一次拿全部（`champion-mastery-v4 by-puuid`，不是 top）。**点进去的页面**：点 Claude 的牌位卡 → 这季胜率表（每只英雄：场数、胜败长条、KDA 比值和场均、每分钟补兵、专精）；点 Antigravity 的某个位置 → 那个位置用过的英雄；点 Grok 的卡 → 全部英雄专精（百万的金框，这季有打的标场数和胜率）；表里点一只英雄 → 它这季每一场（`scout_rows` 从硬盘读这些场、做成对战纪录的一列）→ 再点一场是计分板。实测：294 场、19 只英雄、维尔戈 210 场 105 胜 105 败、170 只英雄有专精共 1143 万分、中路 202 场。
- **时间轴**（2026-10-21）：打开一场的计分板时才问 match-v5 的 timeline（一次请求，精简后存 `lol/timelines/`：每分钟十个人的金钱、经验、补兵、等级，击杀〔谁杀谁、助攻、座标〕、买装、升技能〔EVOLVE 不算〕、大型野怪、建筑；`trim_timeline`），由 Antigravity 那张卡讲：队伍经济差曲线（以自己的队伍为准，领先绿、落后红，下面标小龙／巴龙／预示者被谁拿）、自己和对线对手（对面同 `teamPosition` 的人）的经济和补兵曲线、出装顺序（药水、眼不算）和时间、技能顺序、击杀地图（Data Dragon 的 `map11.png`，座标 0～15000，y 反过来）。图是页面自己画的 SVG。
- **四个 AI 的检讨会**（`review.rs`）：自己的对局计分板下面，四只宠物各一格，预设全勾，按「请勾选的几位检讨这场」才跑。Wakuwaku 先把这场整理成一页繁中的事实（`summary_of`：模式、时长、胜负、自己和对线对手的 K/D/A、补兵、伤害、金钱、视野、击杀参与和伤害占比、双方阵容、最后的装备、5／10／15…分的队伍经济差、10／15／20 分对线的金钱和补兵差、每次阵亡的时间、地点〔`area_of` 粗分上中下路、河道、双方野区和基地〕和被谁杀、大件的购买时间、大型野怪；装备和英雄都用介面语言的名字〔Data Dragon 的 item.json、champion.json〕），每只只问一次，各管一块（Claude 总评、Codex 数字、Antigravity 时间轴、Grok 对线和出装），提示里说不准用工具、300 字以内；四只同时跑，彼此不回应，结果存 `lol/reviews/`，再打开直接显示，完成的不会再跑，失败的可以再按。执行：各家非互动模式（`claude -p`、`codex exec --skip-git-repo-check -`〔不在 Git 仓库里要这个旗标〕、`grok --prompt-file … --permission-mode default`、`agy -p`），在 `lol/reviews/work/<agent>/`，字从 stdin 或档案进，读 stdout，6 分钟逾时，结束码不是 0 就算失败（Claude Code 没登入时把「Not logged in」印在 stdout）。只检讨自己的对局（别人的不跑，省 token）。实测（2026-10-21）：Claude、Codex、Antigravity 约 30 秒；Grok 只给装备 ID 时 4 分钟没回答（猜是去查 ID），换成中文装备名后正常；字数常超过 300（500～700 字）。测试版把 `CLAUDE_CONFIG_DIR`、`CODEX_HOME`、`GROK_HOME` 指到暂存目录时三家都没登入，要用真的环境测。
- **只看得到自己时**：spectator 查不到这局（练习工具、自订：2026-10-20 实测，练习工具的 queue 是 3140，spectator 回 404），或根本不在对局中按 `Alt+X`，面板改成「我的战绩」：自己的段位、这季胜败、最近 10 场的胜场和 KDA、每一场一格（英雄图示、胜负、英雄名、K/D/A，`recent_of` 的 `list`）；在对局中还有这只英雄的熟练度（英雄从 Live Client 自己那一行的 `championName`，用客户端的英雄清单对回 ID）。不在对局时每次按都重查一次（读过的对局在硬盘上，只要几次请求）。
- **客户端的 PUUID 不是 Riot API 的**：LCU 的 `current-summoner`、lobby、选英雄的 PUUID 直接拿去问 Riot API 会回 400「Exception decrypting」；要用 Riot ID（`gameName#tagLine`）问 account-v1（`asia` / `americas` / `europe`，亚太走 `asia`；`sea` 回 403）换成这把金钥看得懂的 PUUID（`scout::puuid_of`，跑着时记住）。spectator 里十个人的 PUUID 本来就是加密过的，直接用。台服客户端的 `region-locale` 是 `TW2`，不是 `TW`。
- **次数限制**：个人金钥每 2 分钟 100 次、每秒 20 次。所有 Riot API 请求（包括选英雄时 `lcu.rs` 查段位）都经过 `scout::riot` 排队（2 分钟 95 次、每秒 18 次），429 等 10 秒再试。顺序：spectator 1 次 → 十人段位和熟练度 20 次（几秒内出来）→ 十人的场次清单 10 次 → 每场细节，一轮一人一场，大家都先有几场。全部约 131 次，第 95 次之后要等到 2 分钟的窗口过去，所以最近战绩全部读完要两分多钟，面板标题旁显示「读最近战绩 n/总数」。读过的对局只留需要的栏位（谁、英雄、胜负、K/D/A），存在资料目录的 `lol/matches/`，同一场里的人共用，不再问第二次；不到 5 分钟的重开局不算。
- **显示**：视窗 `scout`（已列进 capabilities），放在岛所在那块屏的正中间，1100×620（不超过工作区），不拿焦点；右上角 ✕ 收起（`scout_close`：建好视窗时要明讲 `set_ignore_cursor_events(false)`，不讲的话点击到不了页面，2026-10-21）。**点进去**：点一场对战纪录开那场的计分板（`scout_game` 从硬盘读那场，十个人的 Riot ID、英雄和位置、KDA、击杀参与、对英雄伤害长条、金钱、补兵、装备，自己那一行金框；Codex 说伤害最高的是谁），点十人里的某人开他最近 10 场，再点一场一样是计分板；左上「← 返回」，详细页上也有 ✕。对局快取第 3 版加了 Riot ID、伤害、金钱、视野（旧的会重读，第一次会慢一点）。开着的时候挡着它底下的点击，但它只在要看时才开。测试时注意：PowerShell 的 `SetCursorPos` 在 150% 的屏上不是物理像素，点到面板外面的 LINE 视窗，看起来像点击没反应（用 `/debug/eval { page: "scout" }` 看页面收到哪些事件才查出来）。游戏行程一出现（载入画面）就开始查并显示；Live Client 读得到（载入完、进到游戏）就自动收起。之后按 `Alt+X`（设置 `scoutKey`，启动时读，`RegisterHotKey`，被别的程序占用会写进日志，每次按也会写）收起／叫出。原本是 `Alt+``：Microsoft 日文输入法拿它切半角／全角，输入法开着的视窗里按，热键收不到；`Alt+Z` 被 NVIDIA Overlay 占了（`Alt+F1` 也是）。面板换到缩放不同的屏上，Windows 过一下会再按那块屏的缩放改一次大小（和岛的 #37 一样），所以显示后 0.2 秒内看三次，不对就再摆一次，对局结束后还看得到上一局。没有金钥、金钥无效（401/403，开发金钥 24 小时过期）、查不到这局（自订、练习）、没开客户端，面板上都会直接说。
- **测试**：`tools/lol_fake.py` 加了 spectator、熟练度、场次清单、对局、英雄清单；`/fake/phase {"phase":"InProgress"}` 之后 spectator 才有这局。2026-10-20 用假的跑过：十人、段位、熟练度、第一次玩、战绩依次出来，快捷键叫出／收起正常。`WAKUWAKU_SCOUT_AS=<某人的 PUUID>` 把台服别人正在进行的对局当成自己的来查（不用开客户端，英雄名因此是空的），配合 `WAKUWAKU_LOL_URL` 指向假的 Live Client。
- **真金钥实测**（2026-10-20，开发金钥）：台服的 spectator、段位、熟练度、`sea` 的场次和对局都通；一场 8 人有 puuid 的对局读了 54 场，约 75 秒（另一个查询同时用着额度，碰到 429 等过）；面板上段位、胜率、熟练度、最近战绩、连胜连败都对。踩到的：新产生的金钥要 30 秒到两分钟才在所有服务器生效，期间同一个请求时而 401 `Unknown apikey`、时而 403、时而 200；Riot 挡 Python `urllib` 的默认 User-Agent（403），curl、ureq 不受影响，用 Python 测要自己设 User-Agent。

## 手机页（mobile.rs、src/mobile/）

`/mobile/` 是给手机看的单页：四个 agent 各一张卡（宠物的图集照 mood 播 `sprite.js` 的 `MOOD_CLIP` 那一行，半尺寸；`status.js` 说会话在做什么；做完／改好／出错时显示 `reply`），有确认在等时最上面是面板，按钮走和宠物一样的 `Shared::answer`（原来 `pet_answer` 的内容）。页面每 2 秒问 `api/state` 和 `api/asks`，页面藏着时不问。`i18n.js`、`status.js`、`sprite.js` 和页面自己的文件都用 `include_str!` 编进 exe，由 `mobile.rs` 直接回（不经 Tauri）。

| 请求 | 说明 |
| --- | --- |
| `GET /mobile/` | 页面；`?t=<token>` 时把 token 存成 cookie（`wk`，`Path=/mobile`、HttpOnly、SameSite=Lax，经 https 代理时加 Secure）再 302 回 `/mobile/`，网址里就不留 token |
| `GET /mobile/<文件>` | `app.js`、`style.css`、`i18n.js`、`status.js`、`sprite.js`、`manifest.webmanifest`、`sw.js`、两个图标：不用 token |
| `GET /mobile/api/state` | `{ lang, details, at, agents: [{ agent, now, list, sheet, version, pet, color }] }`；`details` 关着时 `reply`、`error` 清空 |
| `GET /mobile/api/asks` | `{ asks }`：所有确认（不分岛和桌宠） |
| `POST /mobile/api/answer` | `{ id, choice }`，回 `{ ok }`；要带 `x-wakuwaku` 头 |
| `GET /mobile/sheet/<agent>` | 那个 agent 的宠物的 `spritesheet.webp`（网址带宠物 id 和修改时间，换宠物就换网址） |
| `GET /mobile/api/reply` | `?session=<id>`：`{ session, agent, text, clipped, summaryOnly }`，它最后一则完整回复（`reply.rs`）；`details` 关着时 `{ hidden: true }` 不给内容 |
| `POST /mobile/api/run` | `{ agent, prompt, cwd? }`，回 `{ ok, started }`；`busy`（409）、`noFolder`、`badCwd`、`prompt`（400）；要带 `x-wakuwaku` 头 |
| `GET /mobile/api/conversation` | `?session=<id>`：`{ session, agent, mood, messages: [{ role, text, clipped }], truncated, summaryOnly, hidden, live, busy, canSay, how, why }`（`talk.rs`）；`how` 是 `resume` 或 `queue`，`why` 是 `live`、`busy`、`noCwd` |
| `POST /mobile/api/say` | `{ session, text }`，回 `{ ok, mode }`；`live`、`busy`（409）、`noCwd`、`prompt`（400）；要带 `x-wakuwaku` 头 |

`reply` 和 `run` 不分本机外来：都要「手机访问」开着（否则 403）、token 对（否则 401）。`api/state` 每个 agent 多了 `canRun`（设了项目文件夹）和 `running`（手机交的任务在跑：开始的时间）。

**完整回复（reply.rs）**：hook 事件带的对话记录路径（Claude Code、Codex 的 `transcript_path`，Grok、agy 的 `transcriptPath`）由 `server.rs` 放进消息的 `transcript`，`state.rs` 记在会话上。读文件尾 2 MB：Claude Code 取最后一段 assistant 的 text（一则回复可能分好几行，碰到 user 行就停）；Codex 取最后一个 assistant `response_item` 的 `output_text`，没有就用 `task_complete.last_agent_message`；Grok 取最后一轮（从 `user_message_chunk` 起）的 `agent_message_chunk`，有工具调用时只取最后一次调用之后的；agy 取最后一个有内容的 `PLANNER_RESPONSE`。最多 4000 字。读不到就给 `reply` 摘要并标 `summaryOnly`。2026-10-17 用这台电脑上四家真的对话记录试过。

**下新指令（run.rs）**：工作目录只能是设置 `agentDirs` 里那个 agent 的文件夹或它下面的（两边都 `canonicalize` 后比），没设就拒绝。任务的字从不放命令行：`claude`、`codex` 是 npm 的 `.cmd`，要经过 cmd.exe，里面的 `&`、`|` 会被当成命令。她把字写进 `data/runs/<agent>-<时间>.prompt.txt`，用 `detach::spawn_pid` 启动自己的 `--wakuwaku-run <job.json>`（脱离她的 job），由它跑 agent：`claude -p` 和 `codex exec -` 从 stdin 读那个文件，`grok --prompt-file <文件> --permission-mode default`，`agy -p <字>`（agy 只接受参数；它是 `.exe`，由 Rust 的 `Command` 正确转义，找到的不是 `.exe` 就不跑）。`CREATE_NO_WINDOW`，输出写进同名 `.log`；跑完删掉 prompt 文件，在 job 里写 `ended` 和 `exit`。同一 agent 只能一个：记着小帮手的 pid，`OpenProcess` + `GetExitCodeProcess` 看它还在不在、job 有没有 `ended`。`data/runs` 只留最近 20 次。日志只记前 80 字（`run::first_words`）。

各家非交互模式遇到要批准的工具（2026-10-17 实测，README 有表）：Claude Code `-p` 会触发 PermissionRequest hook，能在手机上回答，没人回答就当拒绝（`--permission-prompts` 默认 `host`，没有 host 时拒绝）；Codex `exec` 不问任何人（`-c approval_policy="on-request"` 也不问），默认只读沙盒；Grok 无头模式拒绝（但用户的 `config.toml` 若是 `always-approve` 就全部自动批准，所以手机交的一律加 `--permission-mode default`）；agy 无头模式拒绝。试法：暂存文件夹里请它执行 `echo … > probe.txt`，Claude Code 用 `--setting-sources project --settings` 把 hook 指到测试用的 47299，Codex 和 agy 的命令 hook 带着 `WAKUWAKU_PORT=47299`。

**接着电脑上的对话讲（talk.rs）**：先做了调查（2026-10-17，四家各在暂存文件夹开一个会话，用 ConPTY 开 TUI）：TUI 开着时从外面 `--resume -p`（Claude Code）、`-r -p`（Grok）、`--conversation -p`（agy）都会跑，但开着的那个看不到，对话就分岔（Claude 的 jsonl 长出两条枝，Grok 和 agy 的记录交错在一起）；Codex 的 `exec resume` 会被拒（「already has an active writer」），`codex queue` 能把话送进开着的那个。从 TUI 的画面读对话行不通：Claude 的输入框有灰色的「建议下一句」，Grok 的对话卷出了屏幕缓冲区。所以对话一律读对话记录，话一律走各家自己的命令，没有用 ConPTY。

- 对话：Claude 的 jsonl 是一棵树，从最新一行沿 `parentUuid`（压缩处用 `logicalParentUuid`）走回根，只取人打的（不要 tool_result、`<command-…>` 之类）和 text；Codex 取 `response_item` 的 user `input_text`（不要 `<environment_context>`、`# AGENTS.md`）和 assistant `output_text`；Grok 把 `user_message_chunk`、`agent_message_chunk` 按顺序接起来；agy 取 `USER_INPUT`（`<USER_REQUEST>` 里的）和 `PLANNER_RESPONSE`。读文件尾 32 MB，回最近 80 则，每则 4000 字；按「路径、长度、时间」缓存最近 8 份，页面每 3 秒问也不重读。
- 开着没：Claude Code 问 `claude agents --json`（3 秒缓存，约 0.45 秒一次），以它为准；Grok 看 `~/.grok/active_sessions.json` 有这个 session、pid 还在；agy 看 `presence/<id>.lock` 能不能独占打开（开着时它握着，关了文件还在）；再加上会话的进程链头还是原来那个（`jump::is_alive`）。只靠进程链会慢一步：重新打开的会话在第一轮之前不送 hook 事件。Codex 因此另有保险：`exec resume` 撞上 active writer，小帮手改用 `codex queue`。
- 讲：没开着用各家的 resume（`run.rs` 的 `Mode::Resume`），在会话自己的 `cwd`（hook 事件的 `cwd`，agy 的 `workspacePaths[0]`；`state.rs` 记着）跑；Codex 开着用 `Mode::Queue`：话只能放在参数里，所以直接跑 npm 包里的 `codex.exe`（带上 `CODEX_MANAGED_PACKAGE_ROOT`、`CODEX_MANAGED_BY_NPM`，跟 `codex.cmd` 的 node 脚本一样），不经过 cmd.exe。session id 只收字母、数字、`-`、`_`（要上命令行）。
- 关掉的会话：`session-end` 不再删掉，标 `closed`：结束状态照常停留，之后（或者不在结束状态时立刻）她这边看不到，手机的 `api/state` 每个 agent 多一个 `closed`（最近 5 个），一样能读能讲；跟其他闲置会话一样 2 小时后忘掉；再有事件（resume 了）就回来。
- 她一启动就清掉 Claude Code 会话的标记（`run::SESSION_MARKS`：`CLAUDECODE`、`CLAUDE_CODE_CHILD_SESSION`、`CLAUDE_CODE_SESSION_ID`……，不碰 `CLAUDE_CONFIG_DIR` 这类设置）：被 Claude Code 的 hook 叫起来时带着它们，她启动的 `claude -p` 就不存对话记录（实测，画面上写着 inherited CLAUDE_CODE_CHILD_SESSION marker）。

实测（2026-10-17，47299）：Claude 没开着时 resume 接上；用 ConPTY 把同一个会话打开，`claude agents` 认出开着，手机送 409；`/exit` 后又能接，上下文完整（APPLE, BANANA）。`claude --bg` 加 `--permission-mode manual` 时 PermissionRequest 到手机、回答有效；默认 auto mode 时 echo 直接放行没问。Codex 没开着 `exec resume` 接上；`codex resume` 打开后还没说话时她判断成没开着，`exec resume` 被拒，自动改 queue，开着的 TUI 处理了；`codex queue` 对没开着的会话说成功但不处理，下次打开才送进去。Grok（单独的家目录，没有 hook）没开着 `-r -p` 接上，打开 TUI 后 `active_sessions.json` 立刻认出开着。agy 没开着 `--conversation -p` 接上，打开后锁文件认出开着。Chrome 里点 agy 的会话、在对话页送出、对话页更新出 APPLE, BANANA。

**结束时关掉的会话**：非交互模式做完会马上送 `SessionEnd`（`claude -p`、`codex exec` 都是）。原来一收到就删，手机上连「做完了」都看不到；中间试过「留成闲置会话」，可电脑上关掉的会话就一直挂在岛上。现在是上面「关掉的会话」那样：标 `closed`，结束状态照常停留，之后她这边看不到，手机那边还在。

实测（2026-10-17，另开一份在 47299）：没 token、手机访问关着、没 `x-wakuwaku` 头、没设文件夹的 agent、`cwd` 是 `C:\Windows` 或 `..`、空任务都被挡；同一 agent 第二个任务 409；四家各从 `/run` 跑一次都成功（Claude Code 的任务里有 `"`、`&`、`|`、`%PATH%`，原样到了它手上）；Codex 的会话结束后留着，`/reply` 读到它带 Markdown 的完整回复；Chrome 里点会话打开完整回复、从输入框交任务给 Codex。

**谁算本机**（`mobile::is_local`）：对端是 loopback、没有 `X-Forwarded-For` / `Forwarded` / `Tailscale-*` 头、`Host` 是 `127.0.0.1` / `localhost` / `[::1]`（或者没有）。三条都满足才是本机；不是本机的请求只能到 `/mobile…`（其余一律 404，包括 `/hook`、`/health`、`/quit`），而且「手机访问」关着时连 `/mobile` 也 404；`api/` 和 `sheet/` 要 token（cookie、`?t=`、或 `x-wakuwaku-token` 头，定时比较）。这样 `tailscale serve`（从 127.0.0.1 转进来，但带 `Tailscale-*` 和 `X-Forwarded-For`，`Host` 是 ts.net 的名字）也算外面，DNS rebinding（`Host` 是别人的网域）也挡掉。本机的网页可以不带 token 打开 `/mobile/`，但别的网站的页面想替你按「允许」：`answer` 要 `x-wakuwaku` 头，跨源请求得先 preflight，这个端口从不回 CORS，所以送不出来。

**在哪听**：`mobileBind` 是 `tailscale`（默认）、`0.0.0.0` 或一个 IP。Tailscale 的地址用一个 UDP socket `connect` 到 `100.100.100.100:53`（Tailscale 自己的 DNS，不真的发包）看本机从哪个地址出去，在 100.64.0.0/10 里就是它。`mobile::refresh` 在启动、改设置（`mobile`、`mobileBind`）、以及每 15 秒时对一次：要的地址和正在听的不同就 `unblock` 旧的、在新地址上开第二个 `tiny_http::Server`（同一个端口；Windows 上 127.0.0.1 和 0.0.0.0 的同一个端口可以同时听，实测过），请求交给同一个 `server::handle`。放掉 0.0.0.0 时要从本机另一个地址敲一下那个端口（`mobile::knock`）：tiny_http 收掉监听时靠自己连自己叫醒卡在 accept 的线程，Windows 上连不到 0.0.0.0，端口就一直被占着（2026-10-17 实测，`netstat` 看到切回 Tailscale 后 0.0.0.0 还在 LISTENING）。设置页显示在哪听、没听成的原因（`noTailscale`、`badAddress`、`bind: …`）、手机上次来是多久前。

**token 和主题**：第一次启动时（`mobile::fill_secrets`）用系统的随机源（`getrandom`）做 `mobileToken`（16 字节 hex）和 ntfy 的 `topic`（`wakuwaku-` + 10 字节 hex），存进 config.json。设置页的「换一个 token」「换一个主题」是 `settings_mobile` 命令（`token` / `topic`）；token 不在 `settings.rs` 的白名单里，页面改不了。日志里从不写 token。

**QR code**：`src/shared/qr.js`，自己写的（byte 模式、纠错 M、版本 1–10，最多 213 字节），设置页画成 SVG。2026-10-17 对过：每种遮罩、版本 1 到 10 都和 segno 逐格一样（segno 在刚好对齐字节时多补一个 0x00 码字，对照时照它补），OpenCV 也都读得出来；`test/qr.test.js` 留了一个网址的指纹。

**推送**：`Shared::apply` 发提示音和通知的地方（勿扰时不发）同时 `mobile::push(agent, mood)`：`POST <server>/<topic>`，正文只有「<agent> 做完了」这类（Rust 端的 `push.*` 字串），`Title: Wakuwaku`、`Tags`、`Priority`（等你确认是 high）、`Click`（手机页的网址，不带 token）。「推一则试试」是 `settings_mobile("test")`，同步等结果，回 `{ pushed: { ok, error } }`。2026-10-17 用本机假 ntfy 实测：做完、等你、出错、试推四则的路径、标头、正文都对，项目名和回复都没送出去。

**PWA**：`manifest.webmanifest`（`start_url` / `scope` 都是 `/mobile/`，192 和 512 的图标从 `src-tauri/icons/icon.png` 放大）和 `sw.js`（页面和脚本网络优先、留一份；`api/`、`sheet/` 不进缓存）。浏览器只在 https 或 localhost 下注册 service worker，所以要装成 App 得经 `tailscale serve`；http 的 Tailscale 地址照样能用，只是没有离线外壳。

实测（2026-10-17，Windows 11，Tailscale 100.90.183.86，另开一份在 47299 端口）：外部无 token 时页面 200、`api/*` 401、`/hook` `/health` `/quit` `/state` 404；`?t=` 后 302 并设 cookie；带 `X-Forwarded-For`、`Tailscale-User-Login`、`Host: evil.example` 的本机请求都当外部；没有 `x-wakuwaku` 头的回答 404；Chrome 里从 Tailscale 地址打开、按「允许」，挂着的 hook 收到 `{"behavior":"allow"}`。0.0.0.0 → Tailscale、Tailscale → 0.0.0.0、0.0.0.0 → 关掉，`netstat` 上旧地址都马上放掉。

## 右边的竖岛（dock.rs、src/dock/）

托盘或岛的右键菜单「右边的竖岛」（设置 `dock`，默认关）：岛所在那块屏的右边缘、上下居中，一个 agent 一格（Claude、Codex、Antigravity、Grok），宠物在靠屏幕边那侧，左边是对话框写它在做什么（`status.js` 的 `brief`）；闲置又没有确认的只露出宠物。单击一格就在原地展开：那个 agent 的每个会话（名字、状态、用时、做完的回复）、「跳到窗口」按钮（有行程链才有），以及它的确认：权限和计划可以直接允许／拒绝（照 id 回到原本的 hook，和岛、桌宠上回答一样），问题类的写「到灵动岛上回答」。单击不再开终端机。顶端的岛照旧，两个同时在。

- 窗口固定 380×640（逻辑像素，高度不超过工作区），点击穿透和岛同一套（`pointer.rs` 的 `ClickThrough`），页面在每一格的 mouseenter／mouseleave 叫 `pet_hover`。每一格的节点一直留着，只换里面的内容，进出事件才不会跟着重画丢掉；鼠标在上面时不做每秒的重画（按钮按到一半不会被换掉）。
- 资料：`dock::payload`（每个 agent 的 `get_for`、`list_for`、宠物图集和颜色），`Shared::redraw` 时送 `dock:update`；确认清单全部送（`dock::push_asks`）。
- 跟着岛换屏（`islandDisplay`），屏幕变化时和岛一起 `keep_on_screen`；勿扰、隐藏、全屏时跟着她藏起来。
- 实测（2026-10-20，左边 150% 的屏）：四格位置正确，点 Codex 那格会展开；鼠标在格子上时窗口接住鼠标，移到窗口里的空白处、格子之间就恢复穿透。

## 重启后记得会话（keep.rs）

会话原本只在内存里：她一重启（换新版、崩溃、退出再开）就全忘了，之后没再送 hook 的会话一直不见，直到它的下一个事件。实测（2026-10-20）：Grok 一轮在等自己的后台任务（它的日志写 `waiting_task_output`），二十多分钟没送任何事件，这期间她重启过，右边的竖岛就一直说 Grok 没有会话。

- 每 10 秒（有变化才写）和退出时把会话写到资料目录的 `sessions.json`（`state.rs` 的 `snapshot`，`Session` 用 serde，缺的字段当新的），启动时读回（`restore`）：行程链第一个行程已经不在（`jump::is_alive`）、或超过 2 小时（`FORGET_MS`）的丢掉，其余照原样，之后照常由 `check_stale` 收。
- Grok 再补一层：读 `~/.grok/active_sessions.json`（`GROK_HOME`），行程还在、她又不认得的会话加回来，项目名照 `cwd`；日志 `logs/unified.jsonl` 的最后 4 MB 里，那个会话（同一个 pid）最后是 `prompt received` 而不是 `turn.complete`，就当成工作中。
- Grok 等自己的后台任务时可以很久不送事件：每分钟（`check_stale` 前）对工作中的 Grok 会话再看一次它的日志，那一轮还没结束就把 `at` 更新成现在（`keep_grok_busy`、`still_busy`），不会在 15 分钟时被当成结束。
- 实测：换新版重启后日志写 `sessions: 0 kept from before, 1 of Grok's open found`，竖岛的 Grok 那格显示「思考中」；10 秒后 `sessions.json` 写出来了。

## 踩过的坑

- **藏窗口不等于藏页面**：`WebviewWindow::hide` 只藏窗口，WebView2 照样当页面可见，定时器和动画都在跑；要连 WebView 一起藏（`win.as_ref().hide()`，即 `SetIsVisible(false)`），`document.hidden` 才是真。
- **线程的 EcoQoS 是第 3 类**：`SetThreadInformation` 的 `ThreadPowerThrottling` 是 3（0、1、2 是内存优先级、绝对优先级、动态代码策略）；写成 2 时调用因为大小不对失败（ERROR_BAD_LENGTH），返回值没人看，她的线程一直都不是 EcoQoS。
- **`--use-adapter-luid` 的格式**：Chromium（`ui/gl/gl_display.cc`）读的是 `<高位>,<低位>`，都按十进制；读不出来 GPU 进程就起不来。
- **右键菜单一闪就没**：muda 弹菜单前调用 `SetForegroundWindow`，不能获得焦点的窗口做不到，菜单立刻关掉。弹菜单前临时让窗口可获得焦点（`main.rs` 的 `pet_menu`）。
- **页面的 CSP**：精灵图走 `http://asset.localhost`，IPC 走 `http://ipc.localhost`，图库图片来自 `https://codex-pets.net`，都要在 `index.html` 的 CSP 里放行；否则 IPC 会退回 postMessage，图也加载不出来。
- **监听要先注册好**：`listen()` 本身也是一次 IPC，是异步的。bridge 等所有监听注册完再发 `pet_ready`，Rust 收到后才开始推状态。
- **新窗口要写进 capability**：`capabilities/default.json` 的 `windows` 里没有 `island` 时，岛的页面调 `listen()` 会被拒绝，页面永远等不到 `pet_ready`。桌宠窗口按 agent 命名，写成 `pet-*`。
- **PowerShell 的 `Measure`**：测试脚本里自己定义的函数叫 `Measure` 时，调用的是 `Measure-Object` 的别名，函数根本没跑、也没输出（量内存时踩到）。
- **调用要排队**：每次 `invoke` 都是独立请求，async 命令的执行顺序没有保证。拉她出来时页面依次发 holding、release、drop，顺序乱了就会出错，所以 bridge 把调用排成队一个个发（`walk` 除外，它要等走完才回复）。
- **锁和窗口 getter**：Tauri 的 getter（`outer_position`、`cursor_position` 等）要等主线程回复，菜单回调又在主线程上拿锁。所以拿着锁时绝不调 getter，同一时间也不拿两把锁；页面的命令一律写成 async，不在主线程上跑。
- **在主线程上建窗口会卡死**：菜单回调在主线程上，第一次建岛的窗口要放到另一个线程去建。
- **路径里不能有 `..`**：asset 协议的允许范围按路径匹配，带 `..` 的路径匹配不上（`data.rs` 的 `in_tree`）。
- **16 位 PNG**：Tauri 的 PNG 解码只要 8 位 RGBA，托盘和窗口图标都用 8 位（`make-icons.js` 加了 `-depth 8`）。
- **单击 ✕ 又打开了设置**：同一次 click 冒泡到岛上，岛那时已经不在设置里，就当成"单击打开"。岛的 click 忽略来自设置面板里的点击。
- **显示窗口会激活它**：tao 只在窗口第一次显示时不激活，之后 `show()` 用的是 `SW_SHOW`，被显示的窗口会成为本程序的活动窗口（本程序在前台时还会抢走前台），岛就会收到失焦事件，设置跟着收起。也不能绕开 tao 自己调 `ShowWindow`：tao 记的状态一旦是"不可见"，之后改任何窗口样式（比如切换点击穿透）时都会补一个 `SW_HIDE`。所以她的窗口都经 `Shared::show_window` 显示：之后 0.5 秒内岛的失焦不算数，设置开着时焦点马上还给岛。
- **系统通知**：未安装（没有开始菜单快捷方式）的程序要先在 `HKCU\Software\Classes\AppUserModelId\<id>` 登记名字和图标，toast 才会显示。
- **搬动 Rust 工程之后**：`target/` 里的构建产物带着绝对路径，搬家后要 `cargo clean` 一次。
- **全屏判断**：最大化窗口会比屏幕多出几像素边框，但底部止于任务栏；判断全屏还要看有没有标题栏。
- **hook 回包必须是对象**：服务端只会回对象，其他任何值都回 `{}`。
- **压缩时被打回空闲**：自动压缩会触发 `SessionStart`（`source: compact`），要忽略。
- **Linux 的中文 locale**：`zh_CN.UTF-8` 里下划线是单词字符，`/^zh\b/` 匹配不上（`i18n.js` 的 `detectLang`）。
- **PowerShell 里 `h` 是 `Get-History` 的别名**，写测试脚本时同名函数会被别名盖住。
- **从别的程序拖来的文件**：Tauri 的拖放处理换掉了 WebView2 自己的，页面收不到 `dragenter`，拖着时也收不到鼠标事件，只有 `tauri://drag-*`。窗口接着鼠标时（她的东西在光标下）拖动也由它接着，透明的地方也一样，放下的文件就落在她的窗口里；`bridge.js` 把拖动的位置当鼠标位置补发 mouseenter / mouseleave，窗口才会照常变回穿透（#36）。
