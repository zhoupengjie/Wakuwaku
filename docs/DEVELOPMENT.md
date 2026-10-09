# 开发说明

Wakuwaku 是一个 Tauri 2 程序：页面是 `src/` 里的 HTML / JS / CSS，由系统自带的 WebView2 显示；其余都在 `src-tauri/` 的 Rust 里。（2026-10 之前是 Electron 版，在 git 历史里。）

## 目录

```
src/                      页面，Tauri 直接把整个文件夹打进 exe（tauri.conf.json 的 frontendDist）
  pet/
    index.html            她的窗口（?role=pet）和灵动岛的窗口（?role=island）共用的页面
    bridge.js             window.pet：页面和 Rust 之间的调用和事件；替页面补发鼠标进出（见"点击穿透"）
    pet.js                她的动画、气泡、空闲时走动和东张西望
    island.js             灵动岛：几种形状、她在岛里、把她拉出来和放回去
    settings.js           长在岛里的设置（五个页签）
    panel.js              确认面板：在岛里、在设置的横幅里，或在她头顶
    sprite.js             图集布局、16 个注视方向
    status.js             把一个会话说成话：名字、当前这一步、任务清单进度、结果（气泡、岛、设置共用）
    widgets.js            岛上插件的图标和文字
    style.css, settings.css
  shared/i18n.js          中英文（页面用；Rust 那边的几句在 src-tauri/src/i18n.rs）
src-tauri/
  src/
    main.rs               入口：--wakuwaku-ensure-running、--wakuwaku-codex-hook、单实例、各部分共享的 Shared、页面能调的命令
    pet.rs                她的窗口：大小和位置、眼睛、拖动和被"携带"、走动、头顶的面板、飞回岛里
    island.rs             她的家的窗口：角落头像、灵动岛、顶栏各放在哪，按页面要求扩大、伸手够她、把她吸回去、为设置临时升起
    appbar.rs             顶栏占住的那条空间（Windows 的 AppBar）
    settings.rs           设置看到的快照、校验后的设置补丁、设置的各个命令
    pointer.rs            两个窗口共用的点击穿透
    screen.rs             各个显示器的工作区
    asks.rs               确认请求：显示什么、每个选择回给 Claude Code 什么、排队和超时
    connection.rs         插件、settings.json 里的 hooks、Codex 的 hooks.json（安装 / 移除 / 状态）、开机自启
    fetch.rs              codex-pets.net：解析地址、下载宠物、图库
    fullscreen.rs         别的程序是否全屏（user32）
    focus.rs              前台窗口：设置拿走键盘前记下，收起时还回去
    jump.rs               点会话就到它的窗口：会话的进程链、找窗口、叫到前面、桌面版的会话链接
    widgets.rs            岛上的插件（第一层）：脚本发来的、内置的（今天、CPU · 内存、网速、电池），什么时候冒头
    tokens.rs             今天的 token：读 Claude Code 和 Codex 自己的会话记录
    notify.rs             系统通知（登记 AppUserModelId 后发 toast）
    tray.rs               托盘图标（随心情变脸）和菜单
    server.rs             127.0.0.1:47213 上的 HTTP 接口
    state.rs              状态机：按会话记 mood、名字、在跑的工具、任务清单、改过的文件、结果，挑最需要你的那个显示
    events.rs             Claude Code 的 hook 事件 → 消息；会话名（session_title 或会话记录）、项目名（git 仓库名）
    events_codex.rs       Codex 的 hook 事件 → 消息（步骤、任务、回复、项目名的说法用 events.rs 的）
    data.rs               数据目录、设置读写、已下载的宠物和 Codex 的宠物
  icons/                  exe、窗口、托盘和通知的图标（npm 不需要：scripts/make-icons.js 用 ImageMagick 画）
  capabilities/           页面能用的 Tauri 权限：两个窗口都要列进去
integrations/claude-code/ Claude Code 插件 wakuwaku（只有 HTTP hooks）
.claude-plugin/           插件市场入口 marketplace.json（位置是 Claude Code 规定的）
scripts/make-icons.js     画图标（node + ImageMagick），输出提交在 src-tauri/icons
test/                     页面的单元测试（node --test，不需要 npm install）
examples/widgets/         岛上插件的示例脚本：waku、CI、番茄钟、截止日期、开发服务器、股票、天气、邮件……
integrations/thunderbird/ Thunderbird 扩展：未读邮件发到岛上（build.ps1 打包成 .xpi）
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

**数据放在哪**（`data.rs` 的 `folder()`）：`WAKUWAKU_USER_DATA`（测试用）；从源码树里运行时是项目的 `data/`；否则是 exe 旁边的 `wakuwaku-data/`。第一次运行时从 `%APPDATA%\wakuwaku`（旧的 Electron 安装版）复制设置和宠物，但不复制位置：旧版记的是逻辑像素，这一版记物理像素。数据目录里放一个 `debug.on` 文件，就会写 `debug.log`（两个页面的 console.log 也在里面）。

| 环境变量 | 作用 |
| --- | --- |
| `WAKUWAKU_PORT` | 端口（默认 47213） |
| `WAKUWAKU_USER_DATA` | 换一个数据目录 |
| `CLAUDE_CONFIG_DIR` | Claude Code 的配置目录（测试里指向临时目录，不碰真实设置） |
| `CODEX_HOME` | Codex 的配置目录（`hooks.json`、`pets/`），默认 `~/.codex` |
| `WAKUWAKU_DEBUG=1` | `/health` 带上两个窗口和设置，开放 `/debug/eval`、`/debug/walk` |

## 插件

`integrations/claude-code/` 是 Claude Code 插件（名字 `wakuwaku`），仓库根目录的 `.claude-plugin/marketplace.json` 让整个仓库成为插件市场。插件只有 HTTP hook（插件不知道程序装在哪，没法启动她），所以靠开机自启。它的 `hooks.json` 和 `connection.rs` 里 `EVENTS` 写入 settings.json 的 HTTP 部分要保持一致（端口、`from=wakuwaku`、事件和 matcher、PermissionRequest 的 300 秒超时）。

设置里读 `enabledPlugins["wakuwaku@wakuwaku"]` 判断插件是否启用（只读），连接方式分 `plugin` / `hooks` / `both`（事件会重复）/ `none`。

## hook 怎么送到宠物

除 `SessionStart` 外的事件都是 HTTP hook（`type: "http"`，POST 到 `/hook?from=wakuwaku`），不开进程；`/hook` 一般立刻回 `{}`，只有 `PermissionRequest` 会挂起，等面板上的回答。**Claude Code 不对 `SessionStart` 运行 HTTP hook**，所以写入 settings.json 的方式给它配了一条后台命令（`async: true`）：程序本身加 `--wakuwaku-ensure-running`。它从 stdin 读事件，端口上有 wakuwaku 应答就把事件转过去（打招呼、登记会话），没有就以独立进程（`DETACHED_PROCESS`）启动一份宠物，然后马上退出。

`connection.rs` 的 `status_of()` 判断我们的条目：`ok`、`httpOnly`（没有启动命令）、`missing`、`partial`（缺事件）、`stale`（端口不对、指向另一份程序，或者是旧版本）。识别"是我们的"靠 URL 里的 `from=wakuwaku`、参数里的 `--wakuwaku-ensure-running`，以及改名前的 `claude-pets` 两个记号和更早的 `claude-hook.js`。写之前把原文件备份到 `settings.json.wakuwaku.bak`（只备份一次）。

单实例：谁占着端口谁是宠物。再启动一份时端口被占，这份会 POST `/come-home`（让她重新出现、回到右下角）然后退出。

## Codex

Codex（CLI 0.114 起）的 hooks 和 Claude Code 很像，但**只有 command 类型，没有 HTTP**，所以每个事件都运行一次程序本身加 `--wakuwaku-codex-hook`（`main.rs` 的 `codex_hook()`）：从 stdin 读事件，POST 到 `/hook?from=wakuwaku&agent=codex`；只有 `PermissionRequest` 会把她的回答（`hookSpecificOutput`）打印到 stdout 给 Codex，别的事件什么都不打印（Codex 会把 hook 打印的纯文本当成给模型的话）。连不上她时，`SessionStart` 会以独立进程启动她。不管怎样都以 0 退出，Codex 不会显示 hook 失败。

`connection.rs` 的 Codex 部分写 `~/.codex/hooks.json`（支持 `CODEX_HOME`；不碰 `config.toml`），备份到 `hooks.json.wakuwaku.bak`。2026-10 在 Codex CLI 0.146.1 / Windows 11 上实测：

- **Codex 用会话的 shell 运行 hook 命令，Windows 上是 PowerShell 7**（源码里的默认 `cmd /C` 只在没配置 shell 时用）。`"带空格的路径" 参数` 在 PowerShell 里是语法错误，要写 `& '路径' 参数`；路径不需要引号时直接写裸路径（`codex_command()`）。
- Codex 跑 hook 用的是 `pwsh -NoProfile -Command`（`derive_exec_args`，不走 profile），实测启动约 0.28 秒；PowerShell 5.1 约 0.18 秒，cmd 约 0.02 秒，转发本身约 0.03 秒。hook 的 shell 跟着 Codex 会话的 shell 走，没法单独换。
- 所以 Codex 要等的（同步）hook 只装必要的几个（`connection.rs` 的 `CODEX_HOOKS`）：`SessionStart`、`UserPromptSubmit`、`Stop`、`Interrupt`、`SessionEnd`、`PermissionRequest`，`PostToolUse` 只对 `apply_patch|request_user_input`，`PreToolUse` 只对 `request_user_input`（matcher 是正则）。每一步的工具名靠两条不带 matcher 的 `async: true` 的 `PreToolUse` / `PostToolUse`。
- **async hook 从 Codex 0.148 开始才有**；0.146.1 实测会跳过它们（`skipping async hook …: async hooks are not supported yet`），而且是整条跳过，不会改成同步运行。所以安装时用 `codex --version` 判断（缓存一分钟），旧版本不装那两条；之后 Codex 升级了，状态会变成 `upgradable`（只缺后台的那两条；连接页说"Codex 已升级"而不是"需要修复"），点「修复」补上，原来那些 hook 内容和位置都不变，Codex 里只需信任新加的两条（信任按 `hooks.json:<事件>:<组>:<序号>` 和内容的 hash 记在 `config.toml` 的 `[hooks.state]`）。缺别的才是 `partial`。问不到版本（只有桌面版）时当作新版本。
- 后台运行的 hook 可能乱序到达：每个事件都带 `turn_id`，`state.rs` 记住最近结束的几个回合（`Stop` / `Interrupt` 的消息带 `event: "turn-end"`），之后到达的同一回合的事件不再改状态，只有迟到的 `apply_patch` 会把「做完」改成「改好了」。新版本上 `apply_patch` 的 `PostToolUse` 会同步、后台各来一次，重复无害。
- 事件字段：`session_id`、`turn_id`、`cwd`、`hook_event_name`、`model`、`permission_mode`；shell 工具叫 `Bash`，改文件都是 `apply_patch`，两者的 `tool_input` 都只有 `command`（补丁全文）。失败的工具调用没有 `PostToolUse`；请求失败（比如模型不可用）连 `Stop` 都没有，所以 Codex 会话不会显示出错，干活状态靠 15 分钟的超时收尾。
- Codex 只运行**信任过**的 hook（按 hash 记在 `config.toml`），新装或改动后要在 Codex 里 `/hooks` 信任。我们不替用户信任。设置里能看出的只有"装了"和"收到过 Codex 的事件"（`Shared.codex_seen`），连接页据此提示去信任。
- `PermissionRequest` 在 Codex 弹自己的确认框**之前**同步运行，hook 不回答 Codex 就不弹框。所以 Codex 的确认在面板上最多等 60 秒（`server.rs` 的 `CODEX_ASK_MS`），"去终端处理"或超时回 `{}` 后 Codex 才弹框。Codex 只接受 allow / deny，不接受 `updatedPermissions` / `updatedInput`，所以没有"以后都允许"（事件里也没有 `permission_suggestions`）。`hookSpecificOutput.decision` 的格式和 Claude Code 相同。
- `codex exec` 会把确认策略强制设成 never，测不到 `PermissionRequest`；测这一条要用交互式的 Codex。
- 测试运行 Codex 时用便宜的模型：`codex exec --ephemeral --skip-git-repo-check -m gpt-5.6-luna -c model_reasoning_effort='"low"' --dangerously-bypass-hook-trust -c 'hooks.Stop=[...]' "…" < /dev/null`（`-c hooks.*` 只对这一次生效，不改用户的配置；不重定向 stdin 会一直等输入）。

Codex 桌面版的宠物在 `~/.codex/pets/<id>/`（`pet.json` + `spritesheet.webp`，格式和 codex-pets.net 下载的一样），`data.rs` 把它排在自己的宠物之后一起读，只读不写，也不替它建文件夹。

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
| `event` | `turn-start` / `tool-start` / `tool-done` / `tool-failed` / `turn-end`（Codex 的回合结束）/ `session-end` |
| `react` / `say` | `wave` / `jump` / `failed`，在当前 mood 上播一次，期间气泡显示 `say` |
| `title` / `task` | 会话名；你发的第一句话（只认 `source` 是用户的，少于 4 个字的「继续」「好的」不算）。有 title 就用 title |
| `step` / `toolId` | 工具在做什么（`step.*`，`events.rs` 的 `step_of`）；`toolId` 把开始和结束配成对，子 agent 的工具也能对上 |
| `todo` | 任务清单的变化：TodoWrite 给 `{ set }`，TaskCreate / TaskUpdate 给 `{ add }` / `{ update }` |
| `file` | 刚改过的文件（这一轮改了几个文件） |
| `reply` / `error` | Stop / StopFailure 的 `last_assistant_message` 摘前一两句；StopFailure 的 `error`（`error.*`）或 `error_details` |
| `agent` / `turn` | Codex 的消息带 `agent: "codex"` 和回合 id（见上面的 Codex 一节） |

**会话名**：hook 自带的 `session_title`（只有 UserPromptSubmit、SessionStart 带，而且只是自定义标题：`/rename` 或桌面版起的名字）；没有时，在一轮开始和结束时读 `transcript_path` 末尾 256 KB 里最后一条 `custom-title`，没有再用 `ai-title`。**项目名**：从 `cwd` 往上找 `.git`，是目录就取它所在文件夹的名字；是文件（worktree）就按里面的 `gitdir: <仓库>/.git/worktrees/<名字>` 取仓库名；不在 git 里就用 `cwd` 的文件夹名。按 `cwd` 缓存。

页面拿到的每个会话（`state.rs` 的 `view`）还有 `name`、`step`（只在干活时有，没有在跑的工具就是「思考中」）、`stepSince`、`todo`（`{ done, total, active }`）、`files`、`reply`（只在结束时有）、`error`、`agent`。`payload` 里的 `list` 是所有会话，展开的岛用它列出别的会话。设置里的「显示具体内容」（`details`）关掉时，页面只显示项目、状态和工具名，和以前一样。

显示哪个会话：等你确认 > 出错 > 改好了 > 做完 > 干活 > 空闲，同级取最新。做完、改好、出错默认一直保持，直到鼠标经过她（或离开灵动岛）；设置里可以改成 8 / 30 / 120 秒。干活和等你超过 15 分钟没有新事件就当作结束；做完的状态 2 小时没人看就回空闲，空闲会话 2 小时后被忘掉。

## HTTP 接口

| 请求 | 说明 |
| --- | --- |
| `GET /health` | `{ ok, app: "wakuwaku", runtime: "tauri", state }`；调试模式下还有 `window`、`island`、`settings` |
| `POST /hook` | Claude Code 的 hook 事件；一般立刻回 `{}`，`PermissionRequest` 等面板上的回答。正文上限 16 MB（Edit 的 PostToolUse 带着改之前的整个文件）。带 `?agent=codex` 的是 Codex 的事件（由 `--wakuwaku-codex-hook` 转来） |
| `POST /state` | 一条消息 |
| `POST /widget` | 岛上的一个插件（见"岛上的插件"）；回 `{ ok: true }`，或者 400 和 `{ error }` |
| `POST /come-home` | 再次启动时用：她重新出现 |
| `POST /quit` | 像菜单里的「退出」一样退出（顶栏的空间还回去）；换新版本时用，别直接杀进程 |
| `POST /debug/eval` | `{ page: "pet" \| "island", code }`：在页面里运行一段表达式，返回结果（可以是 Promise） |
| `POST /debug/walk` | `{ dx, ms }`：立刻走一段 |

## 在宠物上确认

交互模式下，终端确认框弹出的同时 PermissionRequest hook 在后台运行，先到先得。`server.rs` 把这个请求交给 `asks.rs` 挂起（`Responder` 里拿着 tiny_http 的请求），回答、超时、终端已答时才回复。后台子 agent 和 `claude -p` 会先等 hook 回复再决定，所以面板的等待时间（30 秒到 5 分钟）就是它们最多被拖住的时间。她不可见时（勿扰、隐藏、全屏），确认直接回 `{}`。

| 选择 | 回给 Claude Code 的 `hookSpecificOutput.decision` |
| --- | --- |
| 允许 | `{ behavior: "allow" }`（计划带回 `updatedInput`） |
| 以后都允许 | `{ behavior: "allow", updatedPermissions }`，取事件 `permission_suggestions` 里"允许"类的几条 |
| 拒绝 | `{ behavior: "deny", message }`（按当前语言） |
| 回答问题 | `{ behavior: "allow", updatedInput: { ...tool_input, answers } }`；选项、自己输入的文字、数字题都按题目文字作键 |
| 去终端处理 / 超时 / 终端已答 / 勿扰 / 全屏 | `{}` |

面板什么时候收起：同一个调用的 `PostToolUse` / `PostToolUseFailure` 到达（比较输入时忽略 `answers`）；同一个会话的 `UserPromptSubmit`、`Stop`、`StopFailure`、`SessionEnd`、`Interrupt`（Codex）；同一个 agent 又来了新请求；等待时间到。

面板在哪：灵动岛模式在岛里（岛的 `ask` 形状）；设置开着时是设置里的横幅（`panel.js` 的 `holdsPanel()` 看 `settingsOpen`）；只有宠物时在她头顶，她的窗口向上、向两侧长大，她本人不动（`pet.rs` 的 `set_panel`，挡在屏幕边上时用 `pet:shift` 把她挪回原位）。

## 点会话就到它的窗口

在哪里点：展开的岛里的会话（`island.js` 的 `data-jump`）、设置「现在」页的会话行、确认面板的「去终端处理」（先交还给终端再跳）、单击她（等 500ms 确认不是双击；去的是指针刚移到她身上时她显示的那个会话，因为移上去就算看过，结束的状态会变回空闲）。都走 `session_jump` 命令 → `main.rs` 的 `jump_to` → `jump.rs` 的 `go`。

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

**私密**（`private: true`）：「显示具体内容」关掉时，页面不显示它的 `label` 和冒头的话，只显示 `value`（`widgets.js` 的 `words(lang, w, detailed)`）。邮件脚本都这样发：标题是"发件人：主题"，数值是"3 封未读"。

**内置的**（标题和数值是 `{ key, vars }`，按当前语言显示）：
- `today`：今天开始了几轮（state 的 `Outcome.turns`）、结束的几轮一共用了多久（`Outcome.worked_ms`，几个会话同时跑会叠加）、在宠物上批准了几次（`pet_answer` 的 allow / always）。存在数据目录的 `today.json`，按本地日期换天。
- `sys`：CPU（两次 `GetSystemTimes` 之差）和内存（`GlobalMemoryStatusEx`），3 秒一次，只在打开时算。默认关闭。
- `net`：网速，两次 `GetIfTable` 之差，3 秒一次。只算开着的以太网和 Wi-Fi；Windows 会把一块网卡经过各层过滤器列好几遍，按 MAC 地址只算一次。计数是 32 位的，过 4 GB 会从头再来，按回绕相减。默认关闭。
- `battery`：电量和充电中或还能用多久（`GetSystemPowerStatus`），30 秒一次；没有电池就不显示（「插件」页说「这台电脑没有电池」）。默认关闭。
- `tokens`（`tokens.rs`）：今天的 token，每分钟读一次。Claude Code：`<配置目录>/projects/**/*.jsonl` 里每条回复的 `message.usage`（输入 + 缓存写 + 缓存读 + 输出），同一个 `message.id` 会按片段写好几行，只算一次。Codex：`~/.codex/sessions/**/*.jsonl` 的 `token_count` 事件是这个会话到那时的总数，今天的用量 = 今天最后一个 − 今天之前最后一个。只读今天改过的文件，每个文件从上次读到的地方接着读，只读完整的行；换天从头算。默认打开。

**设置**：`widgetsOff`（关掉的 id，默认 `["sys", "net", "battery"]`）、`widgetOrder`（显示顺序，「插件」页的 ↑ 改它）、`widgetSpin`（0 / 5 / 8 / 15）、`widgetNudge`。

**「插件」页**：五个内置的一直列着（`widgets.rs` 的 `BUILT_IN`），关着的、或者开着还没读到的（比如台式机的电池）没有数值，岛上不显示它们（`main.rs` 发给岛的只有开着且有数值的）。刚打开的电池和 token 马上读一次，不等下一轮。下面「更多插件」列出 examples/widgets 的脚本，每个一个「复制」，复制的是一条 `powershell -ExecutionPolicy Bypass -File "…\weather.ps1" …` 命令（Windows PowerShell 哪台都有；端口不是 47213 时带上 `-Port`）。脚本目录（`data::examples`）：编译它的源码目录还在就用那里的，否则找 exe 旁边的 `examples/widgets`；都没有就只给 GitHub 地址。

**邮件**：IMAP（`mail-imap.ps1`）、Microsoft Graph（`mail-microsoft.ps1`）、Thunderbird 扩展，都只往 `/widget` 发文字，账号和密码不经过宠物。找 IMAP 服务器照 Thunderbird 的顺序：内置的几家 → ISPDB（`autoconfig.thunderbird.net/v1.1/<域名>`）→ MX 记录所属域名的 ISPDB → `imap.<域名>:993`。网易的服务器要先收到 ID 命令（RFC 2971）才肯打开收件箱，登录前后各发一次。授权码和 Graph 的 refresh token 用 DPAPI（`ConvertFrom-SecureString`）加密存在 `%LOCALAPPDATA%\wakuwaku\secrets`。测试时 `mail-imap.ps1 -NoTls -Server 127.0.0.1` 配一个本地的假 IMAP 服务器，`mail-microsoft.ps1 -LoginBase/-GraphBase` 指向本地的假登录和 Graph。**注意 PowerShell 变量名不分大小写**：脚本里的 `$server` 和参数 `-Server` 是同一个变量。

**以后的两层**：插件文件夹里的脚本由宠物定时运行（像 xbar），以及插件自己画的界面（要关进碰不到 IPC 的沙盒，否则能替你点"允许"）。

## 点击穿透

窗口要"透明处点击穿透，她身上接住点击"。Electron 可以穿透的同时照样收到 mousemove（`setIgnoreMouseEvents(true, { forward: true })`），Tauri 没有 forward，所以：

1. Rust 轮询光标（`pointer.rs`，由 `pet.rs`、`island.rs` 的 `poll` 调用；平时 200ms，靠近 80ms，在窗口里 30ms）。光标靠近窗口时，把它在页面里的位置发给页面（`pet:pointer`）。
2. `bridge.js` 用 `elementFromPoint` 算出光标下有哪些元素，替页面补发 `mouseenter` / `mouseleave`。
3. 页面照旧调用 `window.pet.hover(true)`，Rust 让窗口接住鼠标（`set_ignore_cursor_events(false)`）；离开时再变回穿透。

拖动由 Rust 跟随光标，并直接读鼠标按键（`GetAsyncKeyState`）：松手时页面没收到 mouseup 也能结束。鼠标在她身上时 Rust 也盯着按键，按下就开始拖（在岛里进出过之后，Windows 有时会吞掉她窗口的按下）。她被拖着经过灵动岛时，岛只做穿透，不会被"悬停"展开。

## 她的家：角落头像、灵动岛、顶栏

`display` 是她的家：`corner` | `island` | `bar`；`out` 是她在不在桌面上。三种用的是同一个窗口（`island`）、同一个页面元素 `#island`，同样的四种形状（收起、展开、确认、设置），只是放的位置和收起时的样子不同（`island.js` 的 `home()`，`body` 上的 `home-*`、`at-*` 类）：

| | 窗口放在 | 收起时 | 展开往哪长 | 她回家的落点（`seat`） |
| --- | --- | --- | --- | --- |
| `corner` | 工作区的一角（`corner`：br / bl / tr / tl），平时 120×120 | 56px 的圆，外圈是状态色，等你时脉动，别的会话在忙时有个小点 | 离开那个角 | 圆心 |
| `island` | 工作区顶部正中，平时 460×132 | 胶囊 | 往下 | 岛下沿中间 |
| `bar` | AppBar 给的那条（显示器顶部，30px 高），和屏幕一样宽 | 左端是她和最需要你的会话，右边是别的会话的标签（`#bar-rest`）、插件、设置 | 从左端往下垂 | 左端头像下面 |

**谁说话**：`Shared::she_talks()` = `corner` 且 `out`：圆圈收起，她自己用气泡和头顶的面板说话（就是以前的 `display: 'pet'`，旧设置读进来时换成 `corner` + `out`）。灵动岛和顶栏在她出门时照样留着、照样说话。`pet.js` 的气泡、`panel.js` 的 `holdsPanel`、提示音和通知发给哪个窗口，都看这个。

**把她拉出来、放回去**对三种都一样：脖子从家的边上离光标最近的点伸出来（`edgeNear`），所以在角落可以往上、往左拉。她在桌面上被拖近家时，角落的窗口会临时升起来伸手够她（`Island.reaching`），吸回去的动画期间也一直在（`absorbing`）。

**顶栏的空间**（`appbar.rs`）：顶栏显示时用 `SHAppBarMessage` 登记（`ABM_NEW`），按显示器顶部要一条 30 逻辑像素高的（`ABM_QUERYPOS` 后 `ABM_SETPOS`），系统把它从工作区里扣掉；窗口放到批下来的位置。只在显示器或高度变了时再要一次（`bar_for`），否则每次重排都会让所有窗口重新布局。换成别的方式、被隐藏（勿扰、全屏、隐藏）、退出（`RunEvent::Exit`）时 `ABM_REMOVE` 还回去。**进程被强行结束时还不回去**：要换新版本或调试时用 `POST /quit` 正常退出，别直接杀进程。

## 灵动岛

`display: "island"` 时岛的窗口挂在所在屏幕工作区的顶部正中，平时 460×132（逻辑像素）。岛有四种形状：收起（她的圆形小头像、项目、状态、用时）、展开（悬停 140ms 后；或者状态变化时自己展开 3.6 秒）、确认面板、设置。

她在岛里只有一个元素 `#island-her`：外层管裁剪框（位置、大小、圆角），里层 `.sheet` 是原尺寸的一格图集，用 `transform: translate() scale()` 缩放。收起时是 0.21 倍、24px 的圆形头像；设置里是 0.28 倍、32px 的头像；展开和面板时是 0.5 倍的全身。都是同一组 CSS 属性，所以头像是"长成"全身的。形状变化全在 CSS 里（弹簧曲线 `cubic-bezier(0.3, 1.45, 0.5, 1)`，0.56 秒），不逐帧改窗口大小：需要更多空间时页面先通过 `pet_panel` 让窗口变大，40ms 后再让岛长大；收回时等动画做完再缩窗口。

**把她拉出来**：在 `#island-her` 上按下并移动超过 5px，岛的窗口扩到 760×440。`#goo` 层里岛的替身、一段脖子、一滴水三块黑色形状套 SVG 滤镜，就像液体一样连在一起。拉过 110px 断开：页面发 `island_release`（她的中心相对光标的偏移），Rust 把她的窗口放到光标下、设 `out: true`，然后跟随光标"携带"她（这时她的窗口完全穿透，按键还在岛的窗口上）。松手由岛的页面发 `island_drop`，或者由 Rust 读到按键已松开。没拉断就松手，她弹回座位。拉完那一下浏览器会在岛上补发一次 click，岛会忽略它（`pulledAt`），不当成"单击打开设置"。

**放回去**：她的窗口被拖动时，Rust 每帧算她的中心离岛下沿多远（280px 内靠近，140px 内够近），通过 `island:reach` 告诉岛的页面（坐标用逻辑像素，和页面的 `window.screenX` 一致），页面伸出一滴去够她。够近时松手：Rust 立刻藏起她的窗口，岛的页面播一段吸回去的动画（`island:absorb`），380ms 后设 `out: false`。在桌面上双击她，她会飞回岛里（`pet.rs` 的 `fly_home`）。

## 设置（长在岛里）

没有单独的主窗口。设计和手感见 `docs/prototypes/island-settings.html`。

- **打开**：单击岛（不是拉她、不在确认面板上），或者托盘菜单「设置…」、勿扰时左键托盘、只有宠物时双击她、首次运行（没连接 Claude Code 时开在「连接」页；默认宠物下载失败时开在「宠物」页）。都走 Rust 的 `island::open_settings`，它再发 `island:settings` 给岛的页面；页面还没起来时先记着，起来后再发。
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

精灵图从数据目录读，经 Tauri 的 asset 协议给页面：`http://asset.localhost/<编码后的绝对路径>`（`data::asset_url`），宠物文件夹在启动时加进 asset 的允许范围。图库的预览用站点的 `posterUrl`（单帧 192×208）；`previewUrl` 是所有帧连成的一长条，缩进格子里只剩一条线。

## 实测（Windows 11，125% 缩放，空闲）

| | Electron 版（旧） | Tauri 版 |
| --- | --- | --- |
| 发布文件 | 88.5 MB（便携版 exe） | 约 5 MB（只有宠物模式时是 3.7 MB；HTTPS、注册表、通知加了 1.3 MB） |
| 进程数 | 4 | 7（1 + WebView2 的 6 个） |
| 私有内存合计 | 约 183 MB | 约 185 MB |
| 空闲 CPU（3 分钟平均） | 1.13% | 0.89% |

CPU 主要花在 GPU 进程和渲染进程上，两边几乎一样（页面是同一个）。Windows 上 WebView2 本身就是 Chromium，所以内存和 CPU 跟 Electron 持平，省下来的是安装包体积。

## 踩过的坑

- **右键菜单一闪就没**：muda 弹菜单前调用 `SetForegroundWindow`，不能获得焦点的窗口做不到，菜单立刻关掉。弹菜单前临时让窗口可获得焦点（`main.rs` 的 `pet_menu`）。
- **页面的 CSP**：精灵图走 `http://asset.localhost`，IPC 走 `http://ipc.localhost`，图库图片来自 `https://codex-pets.net`，都要在 `index.html` 的 CSP 里放行；否则 IPC 会退回 postMessage，图也加载不出来。
- **监听要先注册好**：`listen()` 本身也是一次 IPC，是异步的。bridge 等所有监听注册完再发 `pet_ready`，Rust 收到后才开始推状态。
- **新窗口要写进 capability**：`capabilities/default.json` 的 `windows` 里没有 `island` 时，岛的页面调 `listen()` 会被拒绝，页面永远等不到 `pet_ready`。
- **调用要排队**：每次 `invoke` 都是独立请求，async 命令的执行顺序没有保证。拉她出来时页面依次发 holding、release、drop，顺序乱了就会出错，所以 bridge 把调用排成队一个个发（`walk` 除外，它要等走完才回复）。
- **锁和窗口 getter**：Tauri 的 getter（`outer_position`、`cursor_position` 等）要等主线程回复，菜单回调又在主线程上拿锁。所以拿着锁时绝不调 getter，同一时间也不拿两把锁；页面的命令一律写成 async，不在主线程上跑。
- **在主线程上建窗口会卡死**：菜单回调在主线程上，第一次建岛的窗口要放到另一个线程去建。
- **路径里不能有 `..`**：asset 协议的允许范围按路径匹配，带 `..` 的路径匹配不上（`data.rs` 的 `in_tree`）。
- **16 位 PNG**：Tauri 的 PNG 解码只要 8 位 RGBA，托盘和窗口图标都用 8 位（`make-icons.js` 加了 `-depth 8`）。
- **单击 ✕ 又打开了设置**：同一次 click 冒泡到岛上，岛那时已经不在设置里，就当成"单击打开"。岛的 click 忽略来自设置面板里的点击。
- **显示窗口会激活它**：tao 只在窗口第一次显示时不激活，之后 `show()` 用的是 `SW_SHOW`，被显示的窗口会成为本程序的活动窗口（本程序在前台时还会抢走前台），岛就会收到失焦事件，设置跟着收起。也不能绕开 tao 自己调 `ShowWindow`：tao 记的状态一旦是"不可见"，之后改任何窗口样式（比如切换点击穿透）时都会补一个 `SW_HIDE`。所以她的窗口都经 `Shared::show_window` 显示：之后 0.5 秒内岛的失焦不算数，设置开着时焦点马上还给岛。
- **气泡不能挤她**：她的窗口里气泡和立绘竖着排在固定高度里，立绘要 `flex: none`，否则气泡一长，立绘的高度就被压缩、截掉。气泡里状态一行、其他会话一行，都用省略号截断，她说的话最多两行（`pet.js` 的 `fillBubble`）。
- **系统通知**：未安装（没有开始菜单快捷方式）的程序要先在 `HKCU\Software\Classes\AppUserModelId\<id>` 登记名字和图标，toast 才会显示。
- **搬动 Rust 工程之后**：`target/` 里的构建产物带着绝对路径，搬家后要 `cargo clean` 一次。
- **全屏判断**：最大化窗口会比屏幕多出几像素边框，但底部止于任务栏；判断全屏还要看有没有标题栏。
- **hook 回包必须是对象**：服务端只会回对象，其他任何值都回 `{}`。
- **压缩时被打回空闲**：自动压缩会触发 `SessionStart`（`source: compact`），要忽略。
- **Linux 的中文 locale**：`zh_CN.UTF-8` 里下划线是单词字符，`/^zh\b/` 匹配不上（`i18n.js` 的 `detectLang`）。
- **PowerShell 里 `h` 是 `Get-History` 的别名**，写测试脚本时同名函数会被别名盖住。
