# claude-pets

一只浮在桌面上的小宠物，实时显示 Claude Code 在干什么：干活、等你批准、改完了等你看、做完了，还是出错了。

形象用的是 [Codex Pets](https://codex-pets.net) 社区的 v2 精灵图格式，网站上任意一只 v2 宠物都能换上。默认是 [大肥鱼/Deepseek Chan](https://codex-pets.net/#/pets/deepseek-chan)。

## 状态

网站上每只 v2 宠物有 11 组动作，这里每一组都有对应的用途：

| 动作 | 什么时候 | 来自哪个 hook |
| --- | --- | --- |
| Idle | 空闲 | `SessionEnd`；或者做完、出错 8 秒后、Review 20 秒后自动回到这里 |
| Running | Claude 在干活 | `UserPromptSubmit`、`PreToolUse`、`PostToolUse` |
| Waiting | 等你：授权框、Claude 问你问题、计划等你确认、MCP 要你填表 | `PermissionRequest`；`PreToolUse`（AskUserQuestion / ExitPlanMode）；`Elicitation` |
| Review | 这一轮改过文件，结束了等你看 | `Stop`（这一轮里 Edit / Write 等工具执行过） |
| Waving | 这一轮结束、没有改文件；会话开始和窗口刚打开时也会打招呼 | `Stop`、`SessionStart` |
| Jumping | 任务清单里完成了一项；单击宠物 | `TaskCompleted` |
| Failed | 请求失败（会持续一会儿）；工具调用失败（闪一下） | `StopFailure`；`PostToolUseFailure` |
| Run right / left | 空闲时随机走几步；拖动窗口时朝拖动方向跑 | — |
| Look around（16 个方向） | 空闲时眼睛跟着鼠标转；偶尔自己东张西望 | — |

上下文自动压缩时也会触发 `SessionStart`（`source: compact`），这时候宠物保持原样，不会被打回空闲。

## 安装

需要 Node.js 18+。目前只在 Windows 11 上测试过；macOS / Linux 理论上也能跑（Linux 上透明窗口需要开启窗口合成器）。

```bash
git clone https://github.com/zhoupengjie/claude-pets.git
cd claude-pets
npm install
npm run fetch-pet        # 从 codex-pets.net 下载默认宠物 deepseek-chan
npm run install-hooks    # 把 hooks 写进 ~/.claude/settings.json
```

之后新开的 Claude Code 会话会自动把宠物拉起来，也可以手动 `npm start`。

如果 `npm install` 之后 `node_modules/electron/dist` 是空的，补跑一次 `node node_modules/electron/install.js`。

## 使用

- **拖动**：按住宠物拖到任意位置，她会朝拖动的方向跑，位置会记住。
- **单击**：跳一下。
- **右键**：切换宠物、调大小、开关气泡、开关空闲走动、开关眼睛跟着鼠标、回到右下角、开机自动启动、退出。
- 透明区域会点击穿透，不挡你操作下面的窗口。
- **找不到她了**（比如拔掉了外接显示器）：再运行一次 `npm start`，已经在运行的那只会回到右下角。窗口每 2 秒也会自查一次，跑出屏幕就自己回来。

### 换一只宠物

在 codex-pets.net 上找到喜欢的宠物，页面地址是 `https://codex-pets.net/#/pets/<id>`：

```bash
npm run fetch-pet -- <id>
```

然后右键 → 宠物 → 选它。

### 卸载

```bash
npm run uninstall-hooks
```

只会删掉本项目加的条目，你自己的 hooks 不受影响；第一次修改前的原文件备份在 `~/.claude/settings.json.claude-pets.bak`。

## 工作原理

```
Claude Code ──HTTP hooks──▶ 悬浮窗 (Electron, 127.0.0.1:47213/hook)
            └─SessionStart─▶ hooks/claude-hook.js（后台运行，窗口没开就启动它）
```

- 除 `SessionStart` 以外的事件都用 Claude Code 的 **HTTP hook**：Claude Code 直接把事件 POST 给窗口，窗口立刻回 `{}`（表示不做任何决定）。调用工具时**不会启动任何进程**，本机实测每次约 0.3ms。
- `SessionStart` 每个会话只触发一次，要负责在窗口没开时把它启动起来，所以走 node 脚本，并且设成 `async` 在后台运行，不耽误会话启动。窗口是独立进程，关掉会话它也还在。
- HTTP hook 只能把事件发给一个已经在监听的地址，没法启动程序，所以 `SessionStart` 保留成命令。如果你连这一次后台进程也不想要，可以全部改成 HTTP，再在宠物右键菜单里打开「开机自动启动」：

  ```bash
  npm run install-hooks -- --http-only
  ```

  代价是你手动退出宠物之后，要等下次开机，或者手动 `npm start`，她才会回来。
- 窗口没开时，HTTP hook 连接会被直接拒绝（约 1ms），Claude 照常工作。
- 窗口 15 分钟没收到新状态时自动回到空闲，防止会话崩溃后一直卡在"干活中"。

> 早期版本每个事件都启动一次 node（通过 bash），每次约 120ms；一次工具调用有前后两个事件，加起来约 0.24 秒。重新运行 `npm run install-hooks` 会把旧条目换成 HTTP 版。

| 环境变量 | 作用 |
| --- | --- |
| `CLAUDE_PETS_PORT` | 换端口（默认 47213），窗口和 hook 两边都要设 |
| `CLAUDE_PETS_AUTOSTART=0` | 会话开始时不自动启动窗口 |

## 测试

```bash
npm test          # 单元测试：hook 事件映射、HTTP 接口、状态机、16 方向注视、install-hooks
npm run smoke     # 端到端：用独立端口和配置目录启动真实窗口，通过 hook 走一遍所有状态，截图存到 out/smoke/
```

开发相关的说明见 [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)。

## 素材与致谢

- 宠物精灵图来自 [codex-pets.net](https://codex-pets.net)，版权归各自作者所有，**不包含在本仓库里**，由 `npm run fetch-pet` 下载到本地的 `pets/`。
- 默认宠物 [大肥鱼/Deepseek Chan](https://codex-pets.net/#/pets/deepseek-chan) 的作者是 Dullsaw。
- 16 个注视方向的映射规则（从正上方开始顺时针，每 22.5° 一档）与 codex-pets.net 保持一致。
