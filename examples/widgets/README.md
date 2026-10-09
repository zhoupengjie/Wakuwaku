# 岛上的插件：示例脚本

任何程序往 `http://127.0.0.1:47213/widget` 发一条 JSON，它就会出现在灵动岛上。没有会话需要你时，插件在岛上轮流显示；会话永远优先。隔一会儿再发一次，它就一直在；停了，过了 `ttl` 秒它就消失。

| 脚本 | 显示 | 来源 |
| --- | --- | --- |
| `weather.ps1 -City Shanghai` | 上海 · 多云　22° | wttr.in，不用注册 |
| `stock.ps1 -Symbol 600519.SS -Name 茅台` | 茅台　1,263.00 ▲0.58% | Yahoo Finance 的图表数据（非官方接口） |
| `stretch.ps1 -Minutes 50` | 坐久了，起来走走　12 分钟后；到点时叫开灵动岛提醒你 | 本机计时 |

用 PowerShell 7（`pwsh`）或 Windows 自带的 PowerShell 都能跑，`-Once` 只发一次。想开机就跑，把它放进「任务计划程序」。

## 发什么

```json
{ "id": "weather", "label": "上海 · 多云", "value": "22°", "icon": "weather", "color": "#ffb340", "ttl": 720 }
```

| 字段 | 说明 |
| --- | --- |
| `id` | 小写字母、数字、`_`、`-`，最长 40；同一个 id 再发一次就是更新 |
| `label` | 最长 40 个字 |
| `value` | 可选，最长 24 个字，显示在右边，用 `color` 的颜色 |
| `icon` | 可选：`cpu` `weather` `note` `calendar` `bell` `stock` `today` `clock` `mail` `music` `code` `star` `dot` |
| `color` | 可选，`#rgb` 或 `#rrggbb` |
| `ttl` | 可选，多少秒没再收到就消失，默认 300，10 到 86400 |
| `nudge` | 可选，`true` 或一句话：叫开灵动岛一次（有会话在等你时不会；同一个插件一分钟最多一次） |
| `remove` | `true`：拿掉这个插件 |

回复 `{ "ok": true }`，或者 400 和 `{ "error": "…" }` 说明哪里不对。宠物只显示这些文字，不运行任何东西。

---

**Widgets in the island, from scripts.** POST that JSON to `http://127.0.0.1:47213/widget` and it shows in the island while no session needs you; send it again within `ttl` seconds to keep it there. The fields are in the table above.
