# 开发说明

## 目录

```
src/
  main/index.js       应用生命周期、窗口、拖动、走动、鼠标位置、右键菜单
  main/state.js       状态机：消息 → mood；Review 还是挥手、自动回 idle、过期兜底、反应动作
  main/asks.js        等你确认的请求：排队、超时、发现终端已经答过就收起
  main/server.js      127.0.0.1:47213 上的 HTTP 接口
  main/config.js      设置读写（Electron userData 下的 config.json：宠物、大小、气泡、走动、注视、位置）
  main/pets.js        列出 pets/ 下已下载的宠物
  preload/index.js    contextBridge 暴露给页面的 window.pet
  renderer/sprite.js  图集布局、mood → 动作、16 方向注视换算（页面和测试共用）
  renderer/pet.js     每 40ms 决定画哪一格：拖动 > 反应 > 走动 > 注视 > mood
  renderer/           index.html、style.css
  shared/hook-events.js  hook 事件 → 消息的映射，以及每个事件怎么安装（http / command、超时）
  shared/ask.js       PermissionRequest → 面板内容；你的选择 → 回给 Claude Code 的 JSON
  renderer/panel.js   确认面板（只用 textContent，不拼 HTML：内容来自工具调用）
hooks/claude-hook.js  SessionStart 用的 command hook：窗口没开就启动，否则发消息
scripts/
  fetch-pet.js        从 codex-pets.net 下载 spritesheet.webp + pet.json；接受 id 或宠物页面地址（parsePetRef）
  install-hooks.js    写入 / 移除 settings.json 里的 hooks
  smoke.js            端到端冒烟测试
test/                 node:test 单元测试
pets/<id>/            下载的宠物（gitignore）
out/                  冒烟测试截图（gitignore）
```

## hook 怎么送到窗口

| 方式 | 事件 | 开销 |
| --- | --- | --- |
| HTTP hook（`type: "http"`，POST 到 `/hook?from=claude-pets`） | 除 SessionStart 外全部 | 不开进程，本机约 0.3ms；窗口没开时连接被拒约 1ms |
| command hook（`async: true`） | SessionStart | 每个会话一次 node，后台运行，不阻塞 |

本机实测，早期"每个事件起一次 node"的方式：通过 bash 约 120ms/次，直接起 node 约 84ms/次；一次工具调用有 Pre 和 Post 两次。

`/hook` 收到什么都立刻回 `200 {}`：Claude Code 会把回包当成 hook 的输出来读，`{}` 表示不做任何决定（比如不会替你批准或拒绝授权）。事件换算在窗口里做（`src/shared/hook-events.js`）。

`install-hooks` 靠 URL 里的 `from=claude-pets`，或者命令里的 `claude-hook.js`，认出哪些条目是自己加的，所以重新安装会把早期的 command 版换成 HTTP 版。http / async hook 在本机 2.1.257、2.1.289、2.1.293 上都确认支持。

## 消息

窗口内部的消息格式（`/state` 也收这个；详见 `src/main/state.js`）：

```json
{ "mood": "working", "detail": "Bash", "event": "tool-done", "react": "failed", "say": "Bash 失败了" }
```

| 字段 | 取值 |
| --- | --- |
| `mood` | `idle` / `working` / `waiting` / `done` / `review` / `error` |
| `detail` | 气泡里跟在状态后面的说明，最多 80 字 |
| `event` | `turn-start`（发了新消息）/ `tool-done`（工具执行完）。窗口靠它判断这一轮有没有改过文件 |
| `react` | `wave` / `jump` / `failed`，在当前 mood 上播一次，不改变 mood |
| `say` | 反应期间气泡里显示的话 |

`mood` 和 `react` 至少要有一个。`done` 时，如果这一轮里 Edit / Write / MultiEdit / NotebookEdit 执行过，就变成 `review`。

## HTTP 接口

| 请求 | 说明 |
| --- | --- |
| `GET /health` | `{ ok, app: "claude-pets", state }` |
| `POST /hook` | Claude Code 的原始 hook 事件（HTTP hook 发来的）。一般立刻回 `{}`；`PermissionRequest` 会挂起，等你在面板上点了再回 |
| `POST /state` | 一条消息 |
| `GET /snapshot` | 当前窗口截图（PNG） |
| `POST /debug/look` | `{ dx, dy }`：假装鼠标在宠物脸旁边这个位置。只在 `CLAUDE_PETS_DEBUG=1` 时开放 |
| `POST /debug/walk` | `{ dx, ms }`：马上走一段。只在 `CLAUDE_PETS_DEBUG=1` 时开放；这个模式下 `/health` 还会带上窗口的位置、尺寸和所在显示器的工作区 |
| `POST /debug/click` | `{ selector }`：像人一样点页面里的元素，返回 `ok` / `disabled` / `missing`。只在 `CLAUDE_PETS_DEBUG=1` 时开放 |

手动测试：

```bash
curl -X POST http://127.0.0.1:47213/hook -d '{"hook_event_name":"PreToolUse","tool_name":"Read"}'
echo '{"hook_event_name":"SessionStart","source":"startup"}' | node hooks/claude-hook.js
curl http://127.0.0.1:47213/snapshot -o snap.png
```

在 Windows 的 Git Bash 里，命令行参数中的中文不是按 UTF-8 发送的，会变成乱码；要测中文，就把 JSON 写进 UTF-8 文件，再用 `--data-binary @file` 发，或者直接通过 hook 脚本发。

| 环境变量 | 作用 |
| --- | --- |
| `CLAUDE_PETS_PORT` | 端口 |
| `CLAUDE_PETS_AUTOSTART=0` | hook 不自动启动窗口 |
| `CLAUDE_PETS_USER_DATA` | 换一个配置目录（也就换了单实例锁），冒烟测试用它和你正在用的窗口互不干扰 |
| `CLAUDE_PETS_DEBUG=1` | 开放 `/debug/look`、`/debug/walk`、`/debug/click`；`/health` 带上窗口位置和等待中的请求 |

## 在宠物上确认（PermissionRequest）

依据（本机 2.1.293 源码）：交互模式下，终端确认框弹出的同时，PermissionRequest hook 在后台运行，代码里用 `claim()` 实现先到先得：hook 先给出决定，确认框就关掉；人先答了，hook 的结果就被丢掉。所以面板挂着请求不会挡住终端。

| 你的选择 | 回给 Claude Code 的 `hookSpecificOutput.decision` |
| --- | --- |
| 允许 | `{ behavior: "allow" }` |
| 以后都允许 | `{ behavior: "allow", updatedPermissions: [...] }`，用的是事件里 `permission_suggestions` 中"允许"类的那几条（addRules + allow、addDirectories、setMode=acceptEdits） |
| 拒绝 | `{ behavior: "deny", message: "用户在桌宠上拒绝了。" }` |
| 选择题作答 | `{ behavior: "allow", updatedInput: { ...tool_input, answers: { "题目文字": "选项" } } }`，多选用 `", "` 连接 |
| 批准计划 | `{ behavior: "allow", updatedInput: tool_input }`（需要人参与的工具，不带 updatedInput 的 allow 会被忽略） |
| 去终端处理 / 超时 / 终端已答 | `{}`（不做决定） |

面板什么时候收起（`src/main/asks.js`）：
- 同一个会话里，同一个工具、同样输入的 `PostToolUse` 或 `PostToolUseFailure` 到达（说明在终端批准了；比较输入时忽略 `answers`）；
- 同一个会话的 `UserPromptSubmit`、`Stop`、`StopFailure`、`SessionEnd`；
- 同一个会话、同一个 agent 又来了一个新的请求（确认框是一个一个弹的）；
- 290 秒没人点（hook 超时设的是 300 秒，赶在 Claude Code 放弃前先回 `{}`）；
- Claude Code 自己断开了请求。

PermissionRequest 的输入里没有 `tool_use_id`，所以只能这样推断。在终端拒绝后，面板会等到这一轮结束或者下一个请求到来时才收起。

窗口：面板出现时，窗口向上、并向两边变大，宠物在屏幕上保持不动。如果贴着屏幕边缘、窗口没法以宠物为中心，就把宠物在窗口里反向挪（`--shift`）；面板收起后精确回到原位（`parked`），中途拖动过就以新位置为准。

## 精灵图

Codex pet v2 图集为 1536×2288，8 列 × 11 行，每格 192×208。

| 行 | 网站上的名字 | 帧数 | 用于 |
| --- | --- | --- | --- |
| 0 | Idle | 6（v2 规范允许第 7 帧放中性注视姿势，deepseek-chan 没画） | idle |
| 1 / 2 | Run right / left | 8 | 空闲走动、拖动 |
| 3 | Waving | 4 | done、打招呼 |
| 4 | Jumping | 5 | TaskCompleted、单击 |
| 5 | Failed | 8 | error、工具失败 |
| 6 | Waiting | 6 | waiting |
| 7 | Running | 6 | working |
| 8 | Review | 6 | review |
| 9 / 10 | Look around | 8 + 8 | 16 个注视方向 |

注视方向的索引 `i` 从正上方开始顺时针算，每 22.5° 一档；对应格子是 `row = 9 + floor(i / 8)`、`frame = i % 8`，与 codex-pets.net 的 "Looking with you" 模式一致。

## 测试

- `npm test`：单元测试，包括 hook 映射（含真实进程：不输出、总返回 0、UTF-8）、状态机（用模拟计时器）、注视换算、install-hooks（用临时 settings 文件）。
- `npm run smoke`：启动真实窗口（端口 47299 + 临时配置目录，不影响你正在用的那只），通过 hook 脚本走一遍所有事件，检查状态并截图到 `out/smoke/`。拖动和真实鼠标跟随需要人工检查。

## 踩过的坑

- **preload 全局重名**：preload 通过 `contextBridge` 暴露了 `window.pet`，页面里再声明 `const pet` 会报重复声明错误，导致整个脚本不执行。
- **动画停住**：窗口不抢焦点、又是半透明，Chromium 会节流它，动画会停。必须设 `backgroundThrottling: false`。
- **中文乱码**：POST body 要先把 Buffer 拼完整再解码 UTF-8，否则跨数据块的多字节字符会被切坏。
- **宠物走出屏幕**：Windows 在 125% 这类非整数缩放下，对透明无边框窗口调用 `setPosition`，每调用一次宽度会多 1px（本机实测 300 次后从 168 变成 468）。闲置走动每 16ms 调一次，窗口越来越宽，而宠物画在窗口底部正中，看起来就像她自己走出了屏幕；位置限制又是按代码以为的尺寸算的，所以也拖不回来。现在所有移动都经过 `moveTo()`，用 `setBounds` 并显式传入宽高；另外 `keepOnScreen()` 每 2 秒、以及显示器变化时检查一次，尺寸不对或者出了屏幕就拉回来。`npm run smoke` 里有这条回归测试（来回走 8 趟、走到左右两边尽头，检查尺寸和位置；把 `moveTo` 换回 `setPosition`，这个测试就会失败）。
- **hook 回包必须是对象**：Claude Code 把 HTTP hook 的回包当作 JSON 输出来解析。onHook 不小心返回了数字之类的值，服务端也只会回 `{}`（单元测试里真遇到过：桩函数返回了 `push()` 的结果 1）。
- **拖动不跟手**：拖动是在主进程里轮询鼠标位置来移动窗口，不依赖页面的 mousemove，所以快速甩动也不会丢。
- **点击穿透**：透明区域的穿透靠 `setIgnoreMouseEvents(true, { forward: true })`；鼠标进入宠物时由页面通知主进程关掉穿透。
- **压缩时被打回空闲**：自动压缩上下文会触发 `SessionStart`（`source: compact`），必须忽略，否则干活到一半会变回空闲。
- **hook 往对话里塞内容**：`UserPromptSubmit` / `SessionStart` 的 stdout 会被加进对话，所以 hook 绝对不能输出任何东西。
- **Electron 二进制没下载**：npm 装 electron 时 postinstall 可能不下载二进制，补跑 `node node_modules/electron/install.js` 即可。
- **Node 24 的 `node --test`**：不接受目录参数，要用 `"test/*.test.js"` 这种 glob。

## 历史

这个项目最初是一个 Claude Code 函数式插件（status-pet），把宠物画在输入框上方，但会占掉浏览空间，所以改成了现在的独立悬浮窗加标准 hooks。
