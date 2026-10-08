# 开发说明

## 目录

```
src/
  main/index.js         入口：按参数决定这次是当宠物、做启动检查（--wakuwaku-ensure-running）还是卸载清理（--wakuwaku-cleanup）
  main/app.js           宠物本体：窗口、托盘、设置窗口、通知、勿扰与全屏、鼠标、拖动、走动、本地端口
  main/state.js         状态机：按会话记 mood，挑最需要你的那个显示；Review 还是挥手、做完后等你看到、用时、提醒
  main/asks.js          等你确认的请求：排队、超时、发现终端已经答过就收起
  main/server.js        127.0.0.1:47213 上的 HTTP 接口
  main/launch.js        这份程序怎么启动（给 hook 和开机启动用）、启动检查、卸载清理
  main/fullscreen.js    别的程序是否全屏（Windows：通过 koffi 调 user32）
  main/config.js        设置读写（userData 下的 config.json）
  main/pets.js          已下载的宠物：<userData>/pets 和源码里的 pets/
  preload/index.js      给宠物页面的 window.pet
  renderer/             宠物页面：pet.js（动画、气泡、空闲行为）、island.js（灵动岛）、panel.js（确认面板）、sprite.js（图集布局、16 方向）
  settings/             主窗口（现在 / 宠物 / 外观 / 提醒 / Claude Code / 关于）：settings.js、preload.js、style.css
  assets/               图标：icon.png、tray/<心情>.png（托盘随心情变脸）、faces/<心情>.png；由 npm run icons 生成
  shared/hook-events.js hook 事件 → 消息；每个事件怎么安装
  shared/hooks-config.js  settings.json 里我们那几条的添加、移除和状态检查
  shared/ask.js         PermissionRequest → 面板内容；你的选择 → 回给 Claude Code 的 JSON
  shared/i18n.js        中英文文字
  shared/pet-fetch.js   从 codex-pets.net 下载宠物
scripts/                fetch-pet.js、install-hooks.js（从源码运行时用）、build-plugin.js、make-icons.js、smoke.js（端到端测试）
plugin/                 Claude Code 插件 wakuwaku（由 build-plugin 生成）
test/                   node:test 单元测试
build/                  打包资源：图标、installer.nsh（卸载时运行清理）
```

## 插件

`plugin/` 是 Claude Code 插件（名字 `wakuwaku`），仓库根目录的 `.claude-plugin/marketplace.json` 让整个仓库成为插件市场。两者都由 `npm run build-plugin` 从 `hooks-config.pluginHooks()` 生成，单元测试会检查仓库里的文件和代码一致。插件只有 HTTP hook（插件不知道程序装在哪，也没法不依赖 Node 跑命令），所以靠开机自动启动。实测（临时的 `CLAUDE_CONFIG_DIR`）：安装后 HTTP hook 正常送达；卸载并移除市场后，`settings.json` 只剩空的 `enabledPlugins` 和 `extraKnownMarketplaces`。

主窗口读 `enabledPlugins["wakuwaku@wakuwaku"]` 判断插件是否启用（只读），连接方式分 `plugin` / `hooks` / `both`（事件会重复）/ `none`。

## hook 怎么送到宠物

除 `SessionStart` 外的事件都是 HTTP hook（`type: "http"`，POST 到 `/hook?from=wakuwaku`），不开进程；`/hook` 一般立刻回 `{}`，只有 `PermissionRequest` 会挂起，等面板上的回答。**Claude Code 不对 `SessionStart` 运行 HTTP hook**（源码里会跳过：`HTTP hooks are not supported for SessionStart`），所以它只有一条后台命令（`async: true`）：用参数列表直接运行程序本身加 `--wakuwaku-ensure-running`，它从 stdin 读到事件，端口有人应答就把事件转过去（打招呼、登记会话），没人应答就以独立进程启动一份宠物，然后马上退出。注意：Electron 主进程在 Windows 上 `process.stdin` 读不到管道数据，要用 `fs.createReadStream(null, { fd: 0 })`。参数列表写法不经过 shell，路径里有空格也不用加引号，也不需要 Node。

`hooks-config.status()` 判断我们的条目处于什么状态：`ok`、`httpOnly`（没有启动命令）、`missing`、`partial`（缺事件）、`stale`（端口不对、指向另一份程序，或者是早期的 node 版本）。设置窗口据此显示「安装 / 重新安装 / 修复 / 移除」。识别"是我们的"靠 URL 里的 `from=wakuwaku`、参数里的 `--wakuwaku-ensure-running`，以及早期版本的 `claude-hook.js`。

改名前（claude-pets）写进 settings.json 的条目（`from=claude-pets`、`--claude-pets-ensure-running`）仍算"我们的"：状态显示为 `stale`，可以修复或移除；旧的启动参数照样能用。旧的设置目录 `%APPDATA%\claude-pets` 在第一次启动时整个搬到 `wakuwaku`，搬不动（旧程序还开着）就只复制 `config.json` 和 `pets/`。

## 消息

hook 事件在宠物里换算成消息（`src/shared/hook-events.js`），`/state` 也收这个格式：

```json
{ "session": "…", "project": "wakuwaku", "mood": "working", "detail": "Bash", "event": "tool-done", "react": "failed", "say": { "key": "say.toolFailed", "vars": { "tool": "Bash" } } }
```

| 字段 | 取值 |
| --- | --- |
| `session` / `project` | 会话 id、项目文件夹名；每个会话单独记状态 |
| `mood` | `idle` / `working` / `waiting` / `done` / `review` / `error` |
| `detail` | 工具名，或 `{ key, vars }`（按当前语言显示） |
| `event` | `turn-start` / `tool-done` / `session-end` |
| `react` / `say` | `wave` / `jump` / `failed`，在当前 mood 上播一次，期间气泡显示 `say` |

显示哪个会话：等你确认 > 出错 > 改好了 > 做完 > 干活 > 空闲，同级取最新。做完、改好、出错默认一直保持，直到 `seen()`（鼠标经过宠物）；设置里 `hold` 可以改成 8 / 30 / 120 秒。干活和等你超过 15 分钟没有新事件就当作结束；做完的状态 2 小时没人看就回空闲，空闲会话 2 小时后被忘掉。

## HTTP 接口

| 请求 | 说明 |
| --- | --- |
| `GET /health` | `{ ok, app: "wakuwaku", state }` |
| `POST /hook` | Claude Code 的 hook 事件；一般立刻回 `{}`，`PermissionRequest` 等面板上的回答 |
| `POST /state` | 一条消息 |
| `GET /snapshot` | 宠物窗口截图；`?page=settings` 截设置窗口 |
| `POST /debug/look`、`/debug/walk`、`/debug/click`、`/debug/eval`、`/debug/settings`、`/debug/fullscreen` | 测试用，只在 `WAKUWAKU_DEBUG=1` 时开放；这时 `/health` 还会带上窗口位置、等待中的请求和设置 |

| 环境变量 | 作用 |
| --- | --- |
| `WAKUWAKU_PORT` | 端口 |
| `WAKUWAKU_USER_DATA` | 换一个配置目录（也就换了单实例锁），测试用它和你正在用的宠物互不干扰 |
| `CLAUDE_CONFIG_DIR` | Claude Code 的配置目录（测试里指向临时目录，不碰你的真实设置） |
| `WAKUWAKU_DEBUG=1` | 开放调试接口 |

## 在宠物上确认

依据（本机 2.1.293 源码）：交互模式下，终端确认框弹出的同时，PermissionRequest hook 在后台运行，先到先得（`claim()`）。但后台子 agent（交互会话里）和 `claude -p` 这类没有确认框的场景，会先等 hook 回复再决定，所以面板的等待时间（设置里 30 秒到 5 分钟）就是它们最多被拖住的时间。

| 选择 | 回给 Claude Code 的 `hookSpecificOutput.decision` |
| --- | --- |
| 允许 | `{ behavior: "allow" }` |
| 以后都允许 | `{ behavior: "allow", updatedPermissions }`，取事件 `permission_suggestions` 里"允许"类的几条 |
| 拒绝 | `{ behavior: "deny", message }`（按当前语言） |
| 回答问题 | `{ behavior: "allow", updatedInput: { ...tool_input, answers } }`；选项、自己输入的文字、数字题都按题目文字作键 |
| 批准计划 | `{ behavior: "allow", updatedInput: tool_input }` |
| 去终端处理 / 超时 / 终端已答 / 勿扰 / 全屏 | `{}` |

AskUserQuestion 的题目类型字段叫 `kind`（`choice` / `text` / `number`，数字题有 `min`、`max`、`step`、`defaultValue`、`unit`）；选择题的"其他"由 Claude Code 自动提供，所以面板上总有一个输入框。

面板什么时候收起（`src/main/asks.js`）：同一个调用的 `PostToolUse` / `PostToolUseFailure` 到达（比较输入时忽略 `answers`）；同一个会话的 `UserPromptSubmit`、`Stop`、`StopFailure`、`SessionEnd`；同一个 agent 又来了新请求；等待时间到；Claude Code 断开了请求。

## 精灵图

Codex pet v2 图集为 1536×2288，8 列 × 11 行，每格 192×208。

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
| 9 / 10 | Look around | 8 + 8 | 16 个注视方向：从正上方顺时针，每 22.5° 一档，`row = 9 + floor(i / 8)`、`frame = i % 8` |

## 测试

- `npm test`：单元测试。
- `npm run smoke`：端到端。启动真实窗口（端口 47299、临时配置目录、临时 Claude Code 配置目录），走一遍首次运行、在设置里装 hooks、所有心情、多会话、等你看到、中英文切换、勿扰与全屏、走动不出屏、确认面板（含打字和数字题）、在设置里下载宠物、注视、会话开始时的启动检查，并截图到 `out/smoke/`。真实的鼠标跟随和拖动需要人工检查。

## 性能

空闲时 CPU 主要花在两处，都已处理：

- **每帧重绘**：Windows 上透明窗口每画一帧都要整块拷贝一次画面（主进程 + GPU 进程合计约 3%）。所以待机动画播一轮就停 4 秒，帧也只在变化时才画（`kick()` 按下一帧到来的时间排定，不用固定 40ms 的定时器）。
- **鼠标转发**：为了让透明窗口"点击穿透、又能感知鼠标经过"，Electron 在 Windows 上会装全局低层鼠标钩子，屏幕上任何鼠标移动都要经过主进程。现在只有鼠标靠近窗口时才开启转发（`setMouseMode`），平时只轮询一次鼠标坐标。

结果：空闲约占单核 1.1%（之前 3%–4.8%），藏起来时约 0.2%–0.5%。

## 踩过的坑

- **preload 全局重名**：preload 暴露了 `window.pet`，页面里再声明 `const pet` 会导致整个脚本不执行。
- **动画停住**：不抢焦点的半透明窗口会被 Chromium 节流，必须 `backgroundThrottling: false`。
- **中文乱码**：POST body 要先把 Buffer 拼完整再解码 UTF-8。
- **宠物走出屏幕**：Windows 在 125% 这类非整数缩放下，对透明无边框窗口调用 `setPosition`，每次宽度多 1px。所有移动都走 `moveTo()`，用 `setBounds` 显式传宽高；`keepOnScreen()` 定时把窗口拉回屏幕。
- **窗口关了定时器还在用它**：宠物窗口关闭（而设置窗口还开着）后，定时器访问已销毁的窗口会报 `Object has been destroyed`。所有访问都经过 `alive(win)`，宠物窗口关闭即退出程序。
- **hook 回包必须是对象**：服务端只会回对象，其他任何值都回 `{}`。
- **压缩时被打回空闲**：自动压缩会触发 `SessionStart`（`source: compact`），要忽略。
- **Linux 的中文 locale**：`zh_CN.UTF-8` 里下划线是单词字符，`/^zh\b/` 匹配不上。
- **最大化不是全屏**：最大化窗口会比屏幕多出 9px 边框，但底部止于任务栏；判断全屏还要看有没有标题栏。
- **PowerShell 里 `Measure` 是 `Measure-Object` 的别名**，同名函数会被别名盖住（写测量脚本时踩到）。
- **Electron 二进制没下载**：补跑 `node node_modules/electron/install.js`。
- **Node 24 的 `node --test`** 不接受目录参数，要用 `"test/*.test.js"`。
- **主窗口的 CSP 不允许 style 属性**：`setAttribute('style', …)` 会被拦，要用 `element.style.cssText`（CSSOM）。
- **单实例锁的交接**：刚退出的旧进程可能还占着锁，这时启动的新进程拿不到锁就会退出。现在拿不到锁时先看端口：有宠物应答就退出，没有就每 250ms 重试，最多 6 秒。
- **被挡住的窗口不重绘**：截图（`capturePage`）拿到的可能是旧画面，冒烟测试截主窗口前先把它调到前面。
- **v1 宠物**：1536×1872、9 行，没有第 9、10 行的注视动作；页面按 `spriteVersion` 跳过注视，背景图高度也跟着变。

## 灵动岛

`display: "island"` 时窗口固定 460×132，挂在所在屏幕工作区的顶部正中，宠物和气泡不画。岛在三种形状之间弹：收起（脸、项目、状态、用时）、展开（悬停 140ms 后，或者变成等你 / 做完 / 改好 / 出错、打招呼时自动弹开 3.2 秒）、确认面板。形状变化全在 CSS 里（width / height 过渡，带回弹的贝塞尔曲线），不改窗口大小，所以不卡；只有确认面板例外：先让窗口变大再让岛长大，收回时等岛缩完（560ms）再缩窗口。面板（#panel）在岛模式下被挪进岛里，由 island.js 量尺寸、通知主进程。悬停展开时会显示做完的内容，所以岛模式下鼠标**离开**才算「看过了」。旧配置里的 `display: "capsule"` 读进来就是 island。
