# 岛上的插件：示例脚本

任何程序往 `http://127.0.0.1:47213/widget` 发一条 JSON，它就会出现在灵动岛上。没有会话需要你时，插件在岛上轮流显示；会话永远优先。隔一会儿再发一次，它就一直在；停了，过了 `ttl` 秒它就消失。宠物只显示这些文字，不运行任何东西。

**不用自己运行**：天气、股票、番茄钟、倒计时、久坐提醒、CI、开发服务器这几个，在设置的「插件」页打开开关，宠物就替你跑，参数在那一行的「设置」里填；她开机启动，它们也就开机就跑。她跑的是自己里面的同样功能（在她自己的进程里，不用 PowerShell，也不开别的进程），岛上显示的和这些脚本发的一模一样，参数名也一样。这里的同名脚本她不再运行，留作样例：写自己的插件时照着改，或者在终端里手动跑。

用 PowerShell 7（`pwsh`）或 Windows 自带的 PowerShell 都能跑；大多数加 `-Once` 只发一次。从 cmd 或 Git Bash 传好几个值时用逗号连起来、不加空格：`-Symbol USDCNY=X,BTC-USD`。

## 有哪些

| 脚本 | 岛上显示 | 说明 |
| --- | --- | --- |
| `waku <命令>` | cargo build · ⏳ 2:13，跑完叫开灵动岛：✓ 或 ✗ 退出码 | 命令照常在当前终端里跑。`waku.cmd`（cmd）和 `waku`（Git Bash）是同一个 |
| `ci.ps1 -Repo owner/repo [-Prs]` | main · CI ✓ 通过；PR · 3 个开着 #12✓ #15✗ | GitHub Actions；看着在跑的那次结束时叫开灵动岛。公开仓库不用 token，私有仓库设 `GITHUB_TOKEN` |
| `pomodoro.ps1` | 专注 · 第 1 轮 18:32 | 25 + 5 分钟，每 4 轮长休息；到点叫开灵动岛 |
| `countdown.ps1 -At '2026-10-10 18:00' -Title 交周报` | 交周报 还有 1 天 6 小时 | 也可以 `-File deadlines.txt`，一行一个；提前 1 小时和到点各提醒一次 |
| `devserver.ps1 -Url http://localhost:3000,http://localhost:5173` | 开发服务器 3000 ✓ 5173 ✗ | 有响应就算在跑；在跑的停了就叫开灵动岛 |
| `stock.ps1 -Symbol 600519.SS,USDCNY=X,BTC-USD -Name 茅台,美元,比特币` | 茅台 1,263 ▲0.58% | 股票、汇率、币价，Yahoo Finance 的图表数据（非官方接口）；默认红涨绿跌，`-GreenUp` 反过来（她替你跑时按插件页的「涨跌颜色」） |
| `weather.ps1 -City 上海` | Shanghai · 多云 22°，展开时加上今天的 14°~22° | Open-Meteo，不用注册；城市可以写中文 |
| `stretch.ps1 -Minutes 50` | 坐久了，起来走走 12 分钟后 | 到点叫开灵动岛 |
| `mail-imap.ps1 -Address you@qq.com` | 王总：周五的方案 3 封未读 | IMAP：QQ、163、126、Gmail、iCloud……见下面"邮件" |
| `mail-microsoft.ps1 -ClientId <id>` | 同上 | Outlook.com、Hotmail、Microsoft 365 |
| [Thunderbird 扩展](../../integrations/thunderbird) | 同上 | 用 Thunderbird 收信的，不用再登录 |

内置的（不用脚本，在设置的「插件」页开关）：今天（几轮、干了多久、批准几次）、今天的 token（Claude Code 和 Codex 的，读它们自己的记录）、监控（CPU、内存、网速、电池，各一个开关）。

## 邮件

**现在宠物自己就能收信**：设置里的「邮件」页，填邮箱地址和密码就行，不用这些脚本。下面的脚本留着作参考，Microsoft 365 / Outlook 的那个在宠物支持浏览器登录之前还用得上。

三条路，按你怎么收信选：

- **用 Thunderbird 收信**：装[扩展](../../integrations/thunderbird)，Thunderbird 已经登录好了，什么都不用再给。
- **QQ、163、126、Gmail、iCloud、Yahoo 等**：`mail-imap.ps1`。先运行一次 `-Setup`，它会告诉你去哪里生成授权码（应用专用密码，不是登录密码），验证能登录后加密存起来（用你的 Windows 账号加密，存在 `%LOCALAPPDATA%\wakuwaku\secrets`），以后不带 `-Setup` 运行。
- **Outlook.com、Hotmail、公司或学校的 Microsoft 365**：微软从 2024 年 9 月起不再让邮件程序用密码登录，只能 OAuth：`mail-microsoft.ps1`。要先在微软那边注册一个应用（免费，五分钟，步骤写在脚本开头），然后 `-Setup` 在浏览器里登录一次。

发件人和主题是私密的（`private`）：设置里的「显示具体内容」关掉时，岛上只显示「3 封未读」，新邮件冒头也只说数量。

**借鉴邮件客户端怎么对付一大堆不同的邮箱**：
- **找服务器**：Thunderbird 先问邮箱自己的配置文件，再查它维护的公共数据库 ISPDB，再看域名的 MX 记录指向谁、拿那家的配置，最后猜 `imap.域名`。`mail-imap.ps1` 也是这个顺序：先查自己内置的几家（顺带说清每家去哪里生成授权码），再查 ISPDB，再查 MX，最后猜。
- **登录**：各家规矩不同。国内邮箱和 Gmail、iCloud 用授权码；微软只认 OAuth。邮件客户端都是自己在微软、谷歌那里注册应用，用自己的应用 id 走 OAuth；我们是开源脚本，所以请你注册一个自己的（谷歌的邮件权限要过安全审查，所以 Gmail 走授权码）。
- **怪规矩**：网易（163、126、yeah.net）的服务器要客户端先报上名字（IMAP 的 ID 命令），否则打开收件箱时报 "Unsafe Login"。邮件客户端都会发，`mail-imap.ps1` 在登录前后各发一次。
- **密码放哪**：邮件客户端放进系统的钥匙串，不写在明文配置里；这里用 Windows 的 DPAPI 加密，只有你的 Windows 账号能解开。

## 发什么

```json
{ "id": "weather", "label": "上海 · 多云", "value": "22°", "detail": "14°~22°", "icon": "weather", "color": "#ffb340", "ttl": 720 }
```

| 字段 | 说明 |
| --- | --- |
| `id` | 小写字母、数字、`_`、`-`，最长 40；同一个 id 再发一次就是更新 |
| `label` | 最长 40 个字 |
| `value` | 可选，最长 24 个字，显示在右边，用 `color` 的颜色 |
| `detail` | 可选。一句话（最长 24 个字）：岛展开时显示在 `value` 后面（比如天气的当天温度范围）。或者一个对象，给它更大的样子用（见下面「大小和明细」） |
| `icon` | 可选：`cpu` `weather` `note` `calendar` `bell` `stock` `today` `clock` `mail` `music` `code` `star` `dot` `chart` `battery` `timer` `flag` `server` `check` `terminal` `coin` `gauge` `memory` `down` `up` `bolt` |
| `color` | 可选，`#rgb` 或 `#rrggbb` |
| `ttl` | 可选，多少秒没再收到就消失，默认 300，10 到 86400 |
| `nudge` | 可选，`true` 或一句话：叫开灵动岛一次（有会话在等你时不会；同一个插件一分钟最多一次） |
| `private` | 可选，`true`：「显示具体内容」关掉时，不显示 `label` 和 `nudge` 的话，只显示 `value` |
| `remove` | `true`：拿掉这个插件 |

回复 `{ "ok": true }`，或者 400 和 `{ "error": "…" }` 说明哪里不对。

### 大小和明细

每个插件至少有小的样子（图标和数值）。`detail` 是对象时，说它还有哪几档、给大的样子更多可放（胶囊里的两行、小曲线和点开的卡片，任务栏上的叠放）：

```json
{ "id": "weather", "label": "上海 · 多云", "value": "22°", "detail": {
  "sizes": ["s", "m", "l"],
  "extra": "14°~22°",
  "series": { "points": [18.5, 20.1, 22.0, 21.4], "unit": "°" },
  "rows": [{ "kind": "hour", "when": "14时", "icon": "cloud", "value": "22°" }]
} }
```

| 字段 | 说明 |
| --- | --- |
| `sizes` | 它有哪几档：`s`（小）、`m`（中）、`l`（大）。不写：小和中（中是数值在上、名字在下）。只写 `["s"]` 的（比如倒计时）在任务栏上也只有小档 |
| `extra` | 一句补充，最长 24 个字：中档的第二行，展开时接在数值后面。和以前的字符串 `detail` 一样 |
| `series` | 一条曲线或一排柱子：`points` 数字，最多 120 个（多了留最后 120 个）；可选 `unit`（16 个字）、`hi`、`lo`（曲线的上下界；只有一个点、带 `hi` 时画成圆环，比如番茄钟剩下的时间） |
| `rows` | 一个列表，最多 12 行，每行一个对象：名字类的 `from` `code` `name` `when` `price` `change` `value` `unit` `kind` `icon` 各 16 个字，主题类的 `subject` `text` 各 60 个字，`at`（毫秒时间戳），`points`（这一行自己的小曲线，最多 120 个数）。别的字段丢掉 |

超出长度的字截断、末尾加「…」，所以脚本撑不爆岛。天气的 `rows` 里 `kind: "hour"` 是逐小时（`icon`：`sun` `moon` `partly` `cloud` `fog` `rain` `snow` `storm`，画成 Windows 自己的图标），`kind: "day"` 是明后天；股票的一行是 `{ code, name, price, change, value }`（`value` 是成交量），`series` 是当天的价格。示例脚本 `weather.ps1`、`stock.ps1`、`pomodoro.ps1`、`countdown.ps1` 都这样发。

---

**Widgets in the island, from scripts.** POST that JSON to `http://127.0.0.1:47213/widget` and it shows in the island while no session needs you; send it again within `ttl` seconds to keep it there. The fields are in the table above. The scripts here: `waku` (tell me when a command is done), CI and pull requests, a pomodoro, deadlines, dev servers, stocks/rates/coins, weather, a stretch reminder, and mail over IMAP, Microsoft Graph, or Thunderbird. The weather, stocks, pomodoro, countdown, stretch, CI and dev-server ones she also runs for you when switched on in the Plugins page, but as part of herself (in her own process: no PowerShell, no process of their own), showing exactly what these scripts send, with the same parameters; she no longer runs these scripts, which stay as samples to start your own from, or to run by hand.

A widget is small at least (its icon and value). Its `detail`, as an object, says which sizes it has (`sizes`: `s`, `m`, `l`) and gives the larger ones more to show: `extra` (a few words, 24 characters: its second line), `series` (`points`, 120 at most, with `unit`, `hi`, `lo`) and `rows` (12 at most, each of `from code name when price change value unit kind icon`, 16 characters each, `subject text`, 60, `at` and `points`). Longer words are cut with "…"; anything else is dropped. A string `detail`, as before, is its extra.

## 让她替你说一句话

不想做成插件、只想让她马上告诉你一件事（比如 AI 跑完了要你看一眼），往 `http://127.0.0.1:47213/say` 发：

```json
{ "text": "测试跑完了，3 个失败", "from": "Claude Code", "secs": 15 }
```

| 字段 | 说明 |
| --- | --- |
| `text` | 要说的话，最长 200 个字，换行和多余的空格合成一个空格 |
| `from` | 可选，谁说的，最长 40 个字；不写就是「有话对你说」 |
| `secs` | 可选，显示几秒，2 到 120，默认 10 |
| `private` | 可选，`true`：「显示具体内容」关掉时只显示 `from` |

她在桌面上时，说在她头顶的气泡里（她还会挥挥手）；在家里（灵动岛、任务栏）时，岛弹开说，不管「她有事告诉你时」怎么设。勿扰、有程序全屏，或者有会话在等你确认时不说。回复 `{ "ok": true, "said": true }`；没说出来时 `said` 是 `false`；不对时 400 和 `{ "error": "…" }`。

PowerShell：

```powershell
Invoke-RestMethod -Method Post http://127.0.0.1:47213/say -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes((@{ text = '构建好了'; from = 'CI' } | ConvertTo-Json -Compress)))
```

curl（Git Bash、WSL 外的终端）：

```bash
curl -s -X POST http://127.0.0.1:47213/say -H 'Content-Type: application/json' -d '{"text":"Build is green","from":"CI"}'
```

**Have her say a few words to you.** POST `{ "text", "from"?, "secs"?, "private"? }` to `http://127.0.0.1:47213/say`: up to 200 characters (one line), who says it (40), for 2 to 120 seconds (10 by default). Out on the desktop she says it in a bubble over her head; at home her island opens saying it, whatever "When she has something to tell you" is set to. Not while do not disturb is on, something is full screen, or a session waits on you (`said: false`).
