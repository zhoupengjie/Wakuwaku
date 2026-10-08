# Wakuwaku

[English](README.en.md) · 中文

> 名字来自《间谍过家家》里阿尼亚的口头禅"わくわく"（哇酷哇酷，好期待）：Claude 在干活，她在旁边满心期待地盯着。

一只浮在桌面上的小宠物，实时显示 Claude Code 在干什么：干活、等你批准、改完了等你看、做完了，还是出错了。需要你确认的时候，可以直接在她头顶的面板上点。

形象用的是 [Codex Pets](https://codex-pets.net) 社区的 v2 精灵图格式，网站上任意一只 v2 宠物都能换上。默认是 [大肥鱼/Deepseek Chan](https://codex-pets.net/#/pets/deepseek-chan)。

## 她会做什么

网站上每只 v2 宠物有 11 组动作，这里每一组都有用处：

支持网站上的 v2 宠物（11 组动作）和旧的 v1 宠物（9 组，没有注视动作，这时她空闲时只看前方）。

| 动作 | 什么时候 | 来自哪个 hook |
| --- | --- | --- |
| Idle | 空闲（眨一轮眼，停几秒） | 做完、出错后你看过她（鼠标经过）；`SessionEnd` |
| Running | Claude 在干活，气泡里显示这一轮的用时 | `UserPromptSubmit`、`PreToolUse`、`PostToolUse` |
| Waiting | 等你：授权、Claude 问你问题、计划等你确认、MCP 要你填表 | `PermissionRequest`；`PreToolUse`（AskUserQuestion / ExitPlanMode）；`Elicitation` |
| Review | 这一轮改过文件，结束了等你看 | `Stop`（这一轮里 Edit / Write 等工具执行过） |
| Waving | 这一轮结束、没有改文件；会话开始和她刚出现时打招呼 | `Stop`、`SessionStart` |
| Jumping | 任务清单里完成了一项；单击她 | `TaskCompleted` |
| Failed | 请求失败（会一直等你看到）；工具调用失败（闪一下） | `StopFailure`；`PostToolUseFailure` |
| Run right / left | 空闲时随机走几步；拖动时朝拖动方向跑 | — |
| Look around（16 个方向） | 空闲时眼睛跟着鼠标转；偶尔自己东张西望 | — |

- **同时开好几个 Claude Code 会话**：每个会话单独记状态。她显示最需要你注意的那个（等你确认 > 出错 > 改好了 > 做完 > 干活），气泡里写明是哪个项目，并告诉你另外还有几个在忙。
- **做完不会错过**：做完、改好、出错这几种状态会一直保持，直到鼠标经过她，也可以在设置里改成停留几秒。可选系统通知和提示音。
- **上下文自动压缩**时也会触发 `SessionStart`，这时她保持原样，不会被打回空闲。

## 在宠物上直接确认

| 场景 | 面板上能做的 |
| --- | --- |
| 授权（"允许 Bash 执行 npm test？"） | **允许** / **以后都允许** / **拒绝**，并显示要执行的命令和项目名 |
| Claude 问你问题（AskUserQuestion） | 点选项、在「其他答案」里打字、填文字题和数字题，支持多道题和多选 |
| 计划确认（ExitPlanMode） | **批准** / **拒绝** |

- 终端里的确认框照常弹出，**两边先答的算数**：在宠物上点了，终端的确认框会自动关掉；在终端答了，宠物的面板会收起。
- **以后都允许**用的是 Claude Code 自己给出的建议规则（和终端里的"以后不再询问"一样），面板上会写清楚要加什么规则、加在哪里。只会加"允许"类的规则。
- 面板刚弹出的 0.6 秒内按钮是灰的，防止误点。要打字时，点一下输入框她才会接管键盘，不会抢你正在打字的窗口。
- 面板最多等多久可以在设置里改（30 秒到 5 分钟），到时间就交给终端。勿扰模式下、或者有程序全屏时，确认直接交给终端，不弹面板。
- 局限：
  - 在终端答完后，要等这个会话的下一个事件，面板才会收起，所以可能多停留一会儿。这时再点面板，Claude 不会理会。
  - `claude -p` 这类非交互模式会先等 hook 回复再做决定，没人点的话最多会卡到面板等待时间结束。
  - MCP 弹出的表单（Elicitation）只做提醒。

## 安装

### 1. 装上宠物

- **安装程序（Windows）**：运行 `Wakuwaku Setup <版本>.exe`（目前需要自己打包，见下面"从源码运行"里的 `npm run dist`）。
- **从源码运行**：见下文。

第一次打开会弹出主窗口：在「宠物」页挑一只（图库直接来自 codex-pets.net，点「下载」即可），或者粘贴宠物页面的地址。

### 2. 连上 Claude Code（推荐用插件）

在 Claude Code 里依次输入（主窗口「Claude Code」页有复制按钮）：

```
/plugin marketplace add zhoupengjie/wakuwaku
/plugin install wakuwaku@wakuwaku
```

插件由 Claude Code 自己安装和卸载，**我们不改你的设置文件**（Claude Code 会在它自己的 `enabledPlugins` 里记一笔，卸载时去掉）。插件没法自动启动宠物，所以建议在主窗口里打开「开机自动启动」。

不想用插件的话，主窗口「Claude Code」页的「高级」里可以直接写入 `~/.claude/settings.json`（会先备份，卸载时只删我们加的条目）。这种方式的好处是新会话开始时能自动把宠物拉起来。两种都装了，主窗口会提示事件重复，并给一个「移除旧 hooks」的按钮。

### 从源码运行

需要 Node.js 18+。目前只在 Windows 11 上测试过；macOS / Linux 理论上也能跑（Linux 上透明窗口需要开启窗口合成器，全屏检测只支持 Windows）。

```bash
git clone https://github.com/zhoupengjie/wakuwaku.git
cd wakuwaku
npm install
npm run fetch-pet        # 从 codex-pets.net 下载默认宠物 deepseek-chan
npm run install-hooks    # 把 hooks 写进 ~/.claude/settings.json
```

之后新开的 Claude Code 会话会自动把她拉起来，也可以手动 `npm start`。打包成安装程序用 `npm run dist`，输出在 `dist/`。

如果 `npm install` 之后 `node_modules/electron/dist` 是空的，补跑一次 `node node_modules/electron/install.js`。

## 使用

- **拖动**：按住她拖到任意位置，她会朝拖动方向跑，位置会记住。
- **单击**：跳一下。
- **右键**（或右键任务栏托盘图标）：切换宠物、调大小、开关气泡、走动、眼睛跟着鼠标、灵动岛模式、回到右下角、勿扰、主窗口、退出。左键托盘图标可以显示或隐藏她。托盘图标会随状态变脸：空闲、干活、等你、做完、改好了、出错各一种。
- **灵动岛模式**：嫌整只宠物太大，可以换成屏幕顶部正中的一颗小黑岛，像 iPhone 的灵动岛：平时只显示她的脸、项目、状态和用时；鼠标停在上面，或者有事发生（等你、做完、出错）时，它会弹开几秒，露出她的头像和正在做什么；有确认请求时直接展开成面板，在岛里点。另一个会话也在等你时，旁边会分出一颗小圆。单击岛打开主窗口。
- **主窗口**：「现在」列出所有会话（项目、状态、用时）和等你确认的请求，可以直接点；「宠物」管理和下载宠物，带 codex-pets.net 图库；「外观」「提醒」是各种设置；「Claude Code」是连接方式；「关于」。你当前的宠物会从左下角探出头。
- 透明区域会点击穿透，不挡你操作下面的窗口。
- **找不到她了**：再运行一次（`npm start` 或开始菜单），她会回到右下角；窗口每 2 秒也会自查一次，跑出屏幕就自己回来。

### 换一只宠物

在主窗口的「宠物」页粘贴地址下载，或者用命令行：

```bash
npm run fetch-pet -- https://codex-pets.net/#/pets/deepseek-chan
```

一次可以给好几个，用空格隔开；只写 id（地址最后那一段）也行。只接受 codex-pets.net 上的地址，也只会从那里下载。地址里带 `&` 时要用引号括起来。

### 暂时关掉

| 想要 | 怎么做 |
| --- | --- |
| 现在先关掉 | 右键 → 退出。下次新开 Claude Code 会话她会自己回来 |
| 暂时别打扰我 | 右键 → 勿扰：她藏起来、不发通知、确认都交给终端 |
| 不要她自动出来，想用时手动开 | 设置里勾选「不让 Claude Code 自动启动宠物」后重新安装 hooks（命令行：`npm run install-hooks -- --http-only`）；想用时手动打开，或开启「开机自动启动」 |

她没开的时候，Claude Code 照常工作：发给她的事件会被立即拒绝连接（约 1ms），不会卡住你。

### 卸载

1. 如果打开过**开机自动启动**，先在主窗口里取消。
2. 用插件连接的：在 Claude Code 里 `/plugin uninstall wakuwaku@wakuwaku`，再 `/plugin marketplace remove wakuwaku`。写入 settings.json 的：在主窗口「Claude Code」页点「移除」（命令行：`npm run uninstall-hooks`），只会删掉本项目加的条目，你自己的 hooks 不受影响，第一次修改前的原文件备份在 `~/.claude/settings.json.wakuwaku.bak`。
3. 退出她，然后卸载程序或删除项目文件夹。安装版的卸载程序会自动做第 1、2 步。
4. （可选）删除她的设置和下载的宠物：Windows 上是 `%APPDATA%\wakuwaku`，macOS 是 `~/Library/Application Support/wakuwaku`，Linux 是 `~/.config/wakuwaku`。

> 从源码运行时，一定要先移除 hooks 再删文件夹：hooks 里记的是程序的路径。移动了文件夹的话，设置里会显示「指向了别的位置」，点「修复」就行。

## 工作原理

```
Claude Code ──HTTP hooks──▶ 宠物窗口 (127.0.0.1:47213/hook)
            └─SessionStart─▶ 程序本身 --wakuwaku-ensure-running（后台运行，她没开就启动）
```

- 所有事件都用 Claude Code 的 **HTTP hook** 直接发给她，调用工具时不启动任何进程（本机实测每次约 0.3ms）。她立刻回 `{}`，表示不做任何决定；只有确认请求会等你在面板上点了再回。
- Claude Code 不对 `SessionStart` 运行 HTTP hook。写入 settings.json 的方式给它配了一条后台命令：直接运行程序本身，她在就把事件转给她（打招呼），不在就启动她，不需要 Node。插件方式没有这条命令，靠开机自动启动。
- 空闲时约占单核 1% 的 CPU（Windows 11 实测），内存约 180MB（大部分是 Electron 本身）。

| 环境变量 | 作用 |
| --- | --- |
| `WAKUWAKU_PORT` | 换端口（默认 47213），她和 hooks 两边都要设 |
| `CLAUDE_CONFIG_DIR` | Claude Code 的配置目录不在 `~/.claude` 时 |

## 测试

```bash
npm test          # 单元测试：hook 映射、多会话状态机、确认面板的回复格式和排队、hooks 安装、多语言、16 方向注视……
npm run smoke     # 端到端：用独立端口和配置目录启动真实窗口，走一遍所有功能并截图到 out/smoke/
```

开发相关的说明见 [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)。

## 许可与致谢

- 代码以 [MIT 许可](LICENSE) 发布。
- 宠物精灵图来自 [codex-pets.net](https://codex-pets.net)，版权归各自作者所有，**不包含在本仓库和安装包里**，由用户在设置里或用 `npm run fetch-pet` 下载到本地。
- 默认宠物 [大肥鱼/Deepseek Chan](https://codex-pets.net/#/pets/deepseek-chan) 的作者是 Dullsaw。
- 16 个注视方向的映射规则（从正上方开始顺时针，每 22.5° 一档）与 codex-pets.net 保持一致。
