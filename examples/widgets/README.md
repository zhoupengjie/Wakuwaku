# 岛上的插件：示例脚本

任何程序往 `http://127.0.0.1:47213/widget` 发一条 JSON，它就会出现在灵动岛上。没有会话需要你时，插件在岛上轮流显示；会话永远优先。隔一会儿再发一次，它就一直在；停了，过了 `ttl` 秒它就消失。宠物只显示这些文字，不运行任何东西。

**不用自己运行**：天气、股票、番茄钟、倒计时、久坐提醒、CI、开发服务器这几个，在设置的「插件」页打开开关，宠物就在后台替你跑，参数在那一行的「设置」里填；她开机启动，它们也就开机就跑。下面是自己在终端里跑的用法，写新插件时可以参考。

用 PowerShell 7（`pwsh`）或 Windows 自带的 PowerShell 都能跑；大多数加 `-Once` 只发一次。从 cmd 或 Git Bash 传好几个值时用逗号连起来、不加空格：`-Symbol USDCNY=X,BTC-USD`。

## 有哪些

| 脚本 | 岛上显示 | 说明 |
| --- | --- | --- |
| `waku <命令>` | cargo build · ⏳ 2:13，跑完叫开灵动岛：✓ 或 ✗ 退出码 | 命令照常在当前终端里跑。`waku.cmd`（cmd）和 `waku`（Git Bash）是同一个 |
| `ci.ps1 -Repo owner/repo [-Prs]` | main · CI ✓ 通过；PR · 3 个开着 #12✓ #15✗ | GitHub Actions；看着在跑的那次结束时叫开灵动岛。公开仓库不用 token，私有仓库设 `GITHUB_TOKEN` |
| `pomodoro.ps1` | 专注 · 第 1 轮 18:32 | 25 + 5 分钟，每 4 轮长休息；到点叫开灵动岛 |
| `countdown.ps1 -At '2026-10-10 18:00' -Title 交周报` | 交周报 还有 1 天 6 小时 | 也可以 `-File deadlines.txt`，一行一个；提前 1 小时和到点各提醒一次 |
| `devserver.ps1 -Url http://localhost:3000,http://localhost:5173` | 开发服务器 3000 ✓ 5173 ✗ | 有响应就算在跑；在跑的停了就叫开灵动岛 |
| `stock.ps1 -Symbol 600519.SS,USDCNY=X,BTC-USD -Name 茅台,美元,比特币` | 茅台 1,263 ▲0.58% | 股票、汇率、币价，Yahoo Finance 的图表数据（非官方接口） |
| `weather.ps1 -City 上海` | Shanghai · 多云 22° | Open-Meteo，不用注册；城市可以写中文 |
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
{ "id": "weather", "label": "上海 · 多云", "value": "22°", "icon": "weather", "color": "#ffb340", "ttl": 720 }
```

| 字段 | 说明 |
| --- | --- |
| `id` | 小写字母、数字、`_`、`-`，最长 40；同一个 id 再发一次就是更新 |
| `label` | 最长 40 个字 |
| `value` | 可选，最长 24 个字，显示在右边，用 `color` 的颜色 |
| `icon` | 可选：`cpu` `weather` `note` `calendar` `bell` `stock` `today` `clock` `mail` `music` `code` `star` `dot` `chart` `battery` `timer` `flag` `server` `check` `terminal` `coin` `gauge` `memory` `down` `up` `bolt` |
| `color` | 可选，`#rgb` 或 `#rrggbb` |
| `ttl` | 可选，多少秒没再收到就消失，默认 300，10 到 86400 |
| `nudge` | 可选，`true` 或一句话：叫开灵动岛一次（有会话在等你时不会；同一个插件一分钟最多一次） |
| `private` | 可选，`true`：「显示具体内容」关掉时，不显示 `label` 和 `nudge` 的话，只显示 `value` |
| `remove` | `true`：拿掉这个插件 |

回复 `{ "ok": true }`，或者 400 和 `{ "error": "…" }` 说明哪里不对。

---

**Widgets in the island, from scripts.** POST that JSON to `http://127.0.0.1:47213/widget` and it shows in the island while no session needs you; send it again within `ttl` seconds to keep it there. The fields are in the table above. The scripts here: `waku` (tell me when a command is done), CI and pull requests, a pomodoro, deadlines, dev servers, stocks/rates/coins, weather, a stretch reminder, and mail over IMAP, Microsoft Graph, or Thunderbird.
