# 写自己的插件

任何程序往 `http://127.0.0.1:47213/widget` 发一条 JSON，它就会出现在灵动岛上。没有会话需要你时，插件在岛上轮流显示；会话永远优先。隔一会儿再发一次，它就一直在；停了，过了 `ttl` 秒它就消失。宠物只显示这些文字，不运行任何东西。

天气、股票、番茄钟、倒计时、久坐提醒、CI、开发服务器这些，她自己就有，在设置的「插件」页打开就行，不用写。这里说的是你自己的。

PowerShell 里一行就能发：

```powershell
Invoke-RestMethod -Method Post http://127.0.0.1:47213/widget -ContentType application/json -Body '{"id":"hello","label":"Hello","value":"42","icon":"star"}'
```

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

回复 `{ "ok": true }`，或者 400 和 `{ "error": "…" }` 说明哪里不对。从网页里发（浏览器带上了网站的 `Origin`）一律回 403：脚本和程序照常，浏览器扩展（比如 Thunderbird 的）也照常。

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

超出长度的字截断、末尾加「…」，所以发什么都撑不爆岛。天气的 `rows` 里 `kind: "hour"` 是逐小时（`icon`：`sun` `moon` `partly` `cloud` `fog` `rain` `snow` `storm`，画成 Windows 自己的图标），`kind: "day"` 是明后天；股票的一行是 `{ code, name, price, change, value }`（`value` 是成交量），`series` 是当天的价格。她自己的天气、股票、番茄钟、倒计时都这样发。

## 请用户批准（POST /ask）

插件只显示，不回传。要用户**批准或拒绝**一件事、再把结果拿回去（比如把聊天软件里带按钮、过一会儿就超时的审批转到桌面上），发到 `/ask`。它和 agent 的确认请求出现在同一个面板上：岛升起来、响提示音，批准和拒绝的快捷键也能用；面板上写明是哪个「外部程序」在问，不会被当成 Claude Code 的。

这个请求**一直挂着，等用户点了才回复**，一个请求拿一个结果：不用注册、订阅，也不用开端口等回调。她退出时连接会断开。

```powershell
$body = @{ id = 'req-42'; app = 'Discord 桥'; title = 'Claude 想运行 npm run deploy'; text = '在 build-server 上'; expires = 120 } | ConvertTo-Json
$r = Invoke-RestMethod -Method Post http://127.0.0.1:47213/ask -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($body)) -TimeoutSec 0
$r.decision   # allow、deny，或者空：没人在这里回答，看 $r.reason
```

```js
const res = await fetch('http://127.0.0.1:47213/ask', {
  method: 'POST',
  headers: { 'content-type': 'application/json' },
  body: JSON.stringify({ id: 'req-42', app: 'Discord 桥', title: 'Claude 想运行 npm run deploy', expires: 120 }),
})
const { decision, reason } = await res.json()
```

| 字段 | 说明 |
| --- | --- |
| `id` | 你自己给这个请求起的名字，1 到 128 个字 |
| `app` | 谁在问，面板上这样称呼你，最长 40 个字 |
| `title` | 要批准的是什么，一行，最长 120 个字 |
| `text` | 可选，更多内容，显示在面板的框里，可以换行，最长 2000 个字 |
| `expires` | 可选，最多等多少秒，默认 300，10 到 3600 |

回复：

| 回复 | 什么时候 |
| --- | --- |
| `{ id, decision: "allow" }` 或 `"deny"` | 用户点了批准或拒绝（或按了快捷键） |
| `{ id, decision: null, reason }` | 这里没人回答，你自己另想办法（比如还是发到聊天软件里）。`reason`：`expired` 时间到了；`dismissed` 用户点了「交回」；`cancelled` 你自己撤回了；`replaced` 同一个 `id` 又发了一次，新的那个接着等；`away` 她现在看不见（隐藏了、勿扰、全屏时），和 agent 的请求交回终端一样；`off` 用户在设置里关掉了「其他程序的审批」 |
| `{ id, decision, again: true }` | 这个 `id` 十分钟内已经有过结果：原样再告诉你一次，不再问用户。连接正好在她回答时断了，重发一次就能拿到 |
| 400 和 `{ error }` | 哪里不对 |
| 429 和 `{ error }` | 已经有 5 个在等了 |

在别处先有了结果（比如聊天软件里有人点了），发 `{ "id": "req-42", "cancel": true }` 把面板上的撤掉，回 `{ ok }`；原来挂着的那个请求会收到 `reason: "cancelled"`。

她只显示你给的字、把用户的选择交回给你，不替你做任何事。和 `/widget` 一样只收本机程序发的，网页发来的一律 403。要等得久的，注意你那边 HTTP 客户端自己的超时：PowerShell 的 `Invoke-RestMethod` 默认 100 秒（上面用 `-TimeoutSec 0` 不限），Node 的 `fetch` 等回复最多 300 秒，`expires` 超过 290 时改用 `http` 模块。

---

**Your own widgets.** POST that JSON to `http://127.0.0.1:47213/widget` and it shows in the island while no session needs you; send it again within `ttl` seconds to keep it there. From a script or a program, or a browser extension (Thunderbird's); not from a web page, which is answered 403. The fields are in the table above. The weather, stocks, a pomodoro, a countdown, a stretch reminder, CI and dev servers are hers already: switch them on in the Plugins page.

A widget is small at least (its icon and value). Its `detail`, as an object, says which sizes it has (`sizes`: `s`, `m`, `l`) and gives the larger ones more to show: `extra` (a few words, 24 characters: its second line), `series` (`points`, 120 at most, with `unit`, `hi`, `lo`) and `rows` (12 at most, each of `from code name when price change value unit kind icon`, 16 characters each, `subject text`, 60, `at` and `points`). Longer words are cut with "…"; anything else is dropped. A string `detail`, as before, is its extra.

**Asking the person to approve something.** POST `{ id, app, title, text?, expires? }` to `http://127.0.0.1:47213/ask` and the request is held until the person picks Allow or Deny on her prompt panel, beside the agents' own and marked as another program's: the answer is `{ id, decision: "allow" | "deny" }`. When nobody answers here it is `{ id, decision: null, reason }` (`expired`, `dismissed`, `cancelled`, `replaced`, `away` while she is out of sight, `off` when the person turned it off), and you ask your own way. The same `id` asked again within ten minutes of a decision gets that decision again (`again: true`). `{ id, cancel: true }` takes one back. `expires` is 10 to 3600 seconds (300 by default); five wait at most (429); not from a web page (403). Mind your HTTP client's own timeout.
