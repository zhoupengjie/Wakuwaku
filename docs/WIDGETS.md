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

---

**Your own widgets.** POST that JSON to `http://127.0.0.1:47213/widget` and it shows in the island while no session needs you; send it again within `ttl` seconds to keep it there. From a script or a program, or a browser extension (Thunderbird's); not from a web page, which is answered 403. The fields are in the table above. The weather, stocks, a pomodoro, a countdown, a stretch reminder, CI and dev servers are hers already: switch them on in the Plugins page.

A widget is small at least (its icon and value). Its `detail`, as an object, says which sizes it has (`sizes`: `s`, `m`, `l`) and gives the larger ones more to show: `extra` (a few words, 24 characters: its second line), `series` (`points`, 120 at most, with `unit`, `hi`, `lo`) and `rows` (12 at most, each of `from code name when price change value unit kind icon`, 16 characters each, `subject text`, 60, `at` and `points`). Longer words are cut with "…"; anything else is dropped. A string `detail`, as before, is its extra.
