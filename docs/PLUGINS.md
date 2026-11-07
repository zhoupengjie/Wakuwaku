# Writing a Wakuwaku plugin

> Status: the contract for plugin API version 1 and for packs, implemented on the `plugins` branch (phases 1–4). The product decisions (where things go, how they look) are in `docs/design/specs/plugins.md`.

A plugin is a folder of web files: a `manifest.json`, a `main.js`, and optionally a `view.html` plus images. It contains no programs. Wakuwaku runs it in a sandboxed iframe. Her own page talks to it with messages, and the plugin reaches everything else (the web, its settings, its storage, her) through a small `waku` object.

There are three ways to put something on her widgets:

| Tier | What you write | Where it shows | In the market |
|---|---|---|---|
| 1. Data | `main.js` calls `waku.widget.set({...})` | Everywhere widgets go (taskbar, capsule, desktop), drawn by her built-in cards | Yes |
| 2. Own view | Tier 1, plus a `view.html` | Desktop M/L cards and the capsule's unfolded card draw your view; everywhere else uses your tier-1 data | Yes |
| 3. External program | Any program that POSTs to `http://127.0.0.1:47213/widget` (`examples/widgets/README.md`) | Like tier 1 | **Never.** You run it yourself |

## Folder and manifest

```
rain-alert/
  manifest.json
  main.js        runs in the background while the widget is placed
  view.html      optional (tier 2)
  icon.svg       optional
```

```json
{
  "id": "rain-alert",
  "kind": "plugin",
  "name": { "en": "Rain alert", "zh": "下雨提醒" },
  "description": { "en": "Will it rain in the next hours, and when", "zh": "接下来几小时会不会下雨，几点开始" },
  "version": "1.2.0",
  "author": "lin",
  "homepage": "https://github.com/lin/rain-alert",
  "api": 1,
  "main": "main.js",
  "view": "view.html",
  "category": "info",
  "widget": {
    "sizes": ["s", "m"], "color": "#8cc8ff", "icon": "weather",
    "sample": { "label": "Dresden", "value": "70%", "detail": { "extra": "17:00", "series": { "points": [10, 30, 55, 70, 62], "unit": "%" } } }
  },
  "settings": [
    { "key": "City", "type": "text", "required": true,
      "label": { "en": "City", "zh": "城市" }, "hint": { "en": "e.g. Dresden", "zh": "比如 Dresden" } },
    { "key": "Hours", "type": "choice", "options": ["3", "6", "12"], "default": "6",
      "label": { "en": "Look ahead", "zh": "往后看" } },
    { "key": "Alert", "type": "flag", "default": true, "label": { "en": "Tell me before it rains", "zh": "下雨前提醒我" } }
  ],
  "permissions": { "net": ["api.open-meteo.com"], "notify": true }
}
```

| Field | Rule |
|---|---|
| `id` | `^[a-z][a-z0-9-]{1,39}$`, unique in the market. Built-in ids (`weather`, `stock`, `pomodoro`, `countdown`, `stretch`, `ci`, `devserver`, `calendar`, `media`, `monitor`, `today`, `tokens`, `waku`) are refused, and so is any id beginning with `mail` or as her own widgets' do (`stock-`, `ci-`, `prs-`, `inbox-`). |
| `kind` | `plugin`. Packs use `pet` or `wallpaper` (see Packs). |
| `name`, `description` | A string, or `{ "en": …, "zh": … }`. English is the fallback. Name ≤ 24 characters. A description of ≤ 60 characters is shown in full; a longer one is cut with "…" where it is shown (the market's check warns about it), and one over 500 is refused. |
| `version` | `x.y.z`. |
| `author` | ≤ 40 characters. |
| `api` | `1`. A plugin asking for a newer API waits for a newer Wakuwaku and says so on the Plugins page. |
| `main` | Required for `plugin`. A `.js` file in the folder. |
| `view` | Optional. An `.html` file in the folder. |
| `category` | `system`, `productivity` or `info`: the widget library's 系统 / 效率 / 信息. |
| `widget.sizes` | A subset of `s`, `m`, `l`. Default `["s", "m"]`. |
| `widget.color` | `#rgb` or `#rrggbb`. Used only on the icon, progress and corner glow, as for built-in widgets. |
| `widget.icon` | One of her built-in icon names (`examples/widgets/README.md`), or an `.svg` in the folder: drawn in one colour as a mask (its shape alone; the text colour on the taskbar, the widget's colour where her widgets use theirs), as a picture, so nothing in it runs. The market's check refuses an SVG icon with scripts, `on…` attributes, `<foreignObject>`, entities (`<!ENTITY>`) or links to anything outside it. Without an icon, the first letter of the name in a tinted square. |
| `widget.sample` | Optional. What your widget would show, in the `POST /widget` format (`label`, `value`, `color`, `icon`, `private`, `detail`), at most 4 KB of JSON. The market's page draws your cards from it with her own templates (S, M, and L if `sizes` has it) before anyone installs your plugin; without it, that page shows no cards. The market's check takes it as she takes a widget: a field of the wrong kind is an error, and what she would drop or cut (an unknown field, words too long, more than 12 rows) is a warning. Use example data that reads in either language; nothing in it is shown once the plugin is installed. |
| `settings` | Fields drawn on the Plugins page, values stored in her settings. `type` is `text`, `number`, `flag` or `choice` (with `options`, and optional `optionLabels: { option: {en, zh} }`). `key` matches `^[A-Za-z][A-Za-z0-9]{0,31}$` and is not `on` (her switch). A `required` field left empty keeps the plugin from running ("needs settings"). |
| `permissions` | See below. Anything not asked for is not available. |

## Permissions

| Permission | Gives | Asked at install |
|---|---|---|
| (none) | Its own widget(s), its settings, its storage, theme, language, quiet | — |
| `net: [hosts]` | `waku.fetch` to those hosts only: exact names, or `*.example.com` (the names under it, not `example.com` itself). Ports 80 and 443 only. Never `localhost`, IP literals or private addresses. | No. Listed on the detail page |
| `agents: true` | `waku.agents`: each agent session's state | Yes |
| `hardware: true` | `waku.hardware`: CPU, memory, GPU, network readings | Yes |
| `notify: true` | `waku.notify`: a nudge on her widget | Yes |

A plugin runs with what was granted when it went in (a market install's reviewed index permissions, a file install's the manifest's, as the confirm layer showed). If a later version — or an edited folder — asks for **more** than was granted, the plugin stops and its Plugins-page row says 要更多权限 (Needs more permissions) with 允许… (Allow…), which lists the new permissions; until the user agrees it does not run. At run time a plugin is given only what was both granted and still asked for. A plugin that asks for a newer `api` installs with its permissions unread, so the user is asked once a newer Wakuwaku can run it.

## The `waku` object

`main.js` runs in a hidden sandboxed frame while the plugin's widget is placed somewhere. Placing it turns the plugin on, and taking it from its last place turns it off, as for built-in plugins. The frame has no storage of its own, and no network except `waku.fetch`.

```js
// main.js
async function look () {
  const { City, Hours } = waku.settings
  const r = await waku.fetch(`https://api.open-meteo.com/v1/forecast?...&city=${encodeURIComponent(City)}`)
  const data = await r.json()
  waku.widget.set({
    value: '70%',
    detail: { extra: '17:00 起有雨', series: { points: data.hourly.slice(0, Number(Hours)), unit: '%' } },
    view: { hours: data.hourly }
  })
}
waku.on('settings', look)
waku.every(600, look)
look()
```

| API | What it does |
|---|---|
| `waku.id`, `waku.version` | This plugin's id and version. |
| `waku.lang` | `'zh'` or `'en'`. Changes fire `waku.on('lang')`. |
| `waku.theme` | `'dark'` or `'light'`. Changes fire `waku.on('theme')`. |
| `waku.quiet` | `true` while nobody is at the computer. Changes fire `waku.on('quiet')`. |
| `waku.settings` | The current values, keyed by `settings[].key`. Changes fire `waku.on('settings', values)`. |
| `waku.widget.set(data)`, `waku.widget.set(sub, data)` | Show data on the plugin's widget, or on a second widget `sub` (`^[a-z0-9]{1,16}$`, its id `<id>_<sub>` 40 characters at most; at most 8 per plugin). `data` follows the `POST /widget` format: `label` (default: the plugin's name), `value`, `color`, `icon`, `private`, and `detail` `{ sizes, series, rows, extra }`. On top of that, `view`: any JSON up to 32 KB, handed to `view.html`. Widgets stay while the plugin runs, so no `ttl` is needed. |
| `waku.widget.remove(sub?)` | Take a widget away. |
| `waku.every(seconds, fn)` | Call `fn` every `seconds` (at least 1). While `waku.quiet`, the interval is three times as long. Returns a function that stops it. |
| `waku.fetch(url, { method, headers, body })` | Resolves to `{ status, ok, headers, text(), json() }`. Allowed only by `net`, through the user's proxy setting (General › Network proxy). Bodies are text, up to 1 MB each way. Times out after 15 s. |
| `waku.storage.get(key)`, `.set(key, value)`, `.remove(key)`, `.all()` | Promises. JSON values, up to 1 MB per plugin in total, kept across updates and deleted with the plugin. |
| `waku.agents.list()` | (`agents`) A promise of `[{ id, agent, project, mood, since, detail }]`, where `mood` is `idle`, `working`, `waiting`, `done`, `review` or `error`. `waku.on('agents', list)` fires on change, at most once a second. |
| `waku.hardware.read()` | (`hardware`) A promise of `{ cpu, memory, gpu, netDown, netUp }`. Percentages are 0–100 and rates are bytes per second; a field is `null` when not known. `waku.on('hardware', r)` fires every few seconds while someone listens. |
| `waku.notify(text)` | (`notify`) Nudges her widget with `text` (≤ 60 characters). At most once a minute, never while she is asking you something, and only if "plugins may open the island" is on. |
| `waku.on('view', fn)` | Messages from `view.html` (`waku.send`): `fn(message, { sub })`, `sub` the widget whose card sent it (`''` for the plugin's own). |
| `waku.log(...)` | One line in her log, with the plugin's id. An uncaught error is logged the same way, and its message shows under the plugin on the Plugins page. |

### view.html (tier 2)

`view.html` is drawn inside her card on the desktop (M and L) and in the capsule's unfolded card (shown for a widget with an `l` size). Her card supplies the glass, corners, hover buttons and menu; you draw what goes inside. A widget is drawn by your view once it gives a `view` field (`waku.widget.set({ ..., view: {...} })`); one that gives none keeps her drawing of its data. The `waku` object is there too, before your own scripts run:

| API | What it does |
|---|---|
| `waku.size` | `'m'` or `'l'`: the desktop's medium card (292 × 128 px inside) or large one (292 × 296). The capsule's card is `'m'`, as wide as the capsule's lines (266 to 312) and 128 high. |
| `waku.data` | The `view` field of the widget's last `waku.widget.set`, there from the start. `waku.on('data', fn)` fires when it changes. |
| `waku.send(message)` | Sends any JSON to `main.js` (`waku.on('view')`): up to 64 KB, ten a second. |
| `waku.theme`, `waku.lang`, `waku.on('theme' / 'lang')` | As in `main.js`. |
| `waku.log(...)` | One line in her log, with the plugin's id. |
| CSS variables | `--waku-fg`, `--waku-muted`, `--waku-color` (the widget's colour), `--waku-font`, `--waku-font-display`, on `:root`, changed with her theme. The body is transparent, with no margin and nothing to scroll; leave `color-scheme` as it is, or the frame gets an opaque ground. |

It is for showing: the pointer passes through it to her card, so dragging, double-clicking and the card's menu stay hers, and the view gets no clicks. Her two buttons show at the card's top right while the pointer is on it: keep that corner (about 60 × 30 px) free of anything that matters.

While nobody is about, while the desktop is covered, or while Windows has animations turned off, the view is taken away and the card falls back to your tier-1 data, as it does until your view has loaded. A view that does not load within a few seconds, or does not answer her pings, is not shown again until she restarts. Keep animations short and let them stop (`docs/design/style.md`: no endless motion). `examples/plugins/agent-board` is a plugin with a view.

## The sandbox

- The frame is `sandbox="allow-scripts"`, so it has no origin of its own: no cookies, `localStorage` or IndexedDB, no forms, no popups, and no reaching her page or other plugins.
- Its files come from `http://wakuplugin.localhost/<id>/`. A Content-Security-Policy allows scripts, styles, images and fonts from your own folder and `data:` / `blob:`, and blocks all network access (`connect-src 'none'`). Use `waku.fetch`. That includes your own folder: `import` your modules, and put data your code needs in a `.js` module rather than fetching a `.json`.
- No network but `waku.fetch`: `connect-src 'none'` stops `fetch`, `XMLHttpRequest`, `WebSocket` and `EventSource`, and WebView2 is started with WebRTC's non-proxied UDP turned off so a plugin cannot reach the network by WebRTC either. The SDK also blanks the WebRTC constructors in the frame and in its child frames, as a second layer.
- No workers: `worker-src 'none'`. A worker would run off her page's main thread, out of the watchdog's reach, so there are none in v1.
- `main.js` is loaded as a module (`import`, top-level `await`).
- It cannot reach her local port (`127.0.0.1:47213`), the file system, other programs or Windows.
- Watchdog: she pings every plugin every 5 s. One that does not answer within 3 s is reloaded. One that hangs again within ten minutes is turned off and marked "not responding" on the Plugins page, until the user turns it back on. So is one whose own long tasks (the main thread held 50 ms or more at a time) add up to over 2 s in a minute ("too busy"). All plugins share one process: when none answers, she restarts them one at a time to find the one that hangs. Do heavy work in small pieces.

## Packaging and installing

- A plugin is a `.zip` of the folder: its files at the root, or in one top folder. A plain zip: entries stored or deflated; ZIP64 archives and encrypted entries are refused.
- Allowed file types: `.js .mjs .json .html .htm .css .svg .png .jpg .jpeg .webp .gif .avif .woff .woff2 .ttf .otf .mp3 .ogg .wav .txt .md .wasm`. Anything else is refused, a file with no extension too (name it `LICENSE.txt`). What a Mac's or Windows' zipping adds (`__MACOSX/`, `.DS_Store`, `Thumbs.db`, `desktop.ini`) is left out.
- Files are also checked by their first bytes, so a renamed program (`MZ`, ELF, Mach-O) is refused whatever its name.
- Limits: plugins 5 MB zipped and 20 MB unpacked; packs 50 MB and 200 MB; 500 files at most. A MB here is a MiB (1024 × 1024 bytes), as are the 1 MB of storage and of a fetch's body; the manifest's 64 KB is 64 × 1024 bytes.
- From a file: Plugins page › 显示 › 从文件安装… (Install from file…), Windows' file dialog for the `.zip`. She asks before installing: the plugin is marked "not reviewed", and what it will do is listed. One with a program in it, or anything else she refuses, is not installed, and she says why. A plugin with the same `id` as one installed is its update: its settings and storage stay. Installed is not placed: it shows on the Plugins page as 没放 (not placed) and first in the widget library, marked 新 (new), until it is put somewhere.
- From the market: Plugins page › 获取更多 (Get more). Market entries are reviewed.

## Packs

A pack has no code: nothing in it runs, and installing one asks for nothing. Its `manifest.json` has `kind` `pet` or `wallpaper`, with `id`, `name`, `description`, `version` and `author` as above, and no `main`, `view`, `settings` or `permissions` (`api` may be left out). It holds pictures and a few words alone: a file of any other type (a `.js`, a `.css`, a `.html`) keeps it from installing, from the market or from a file.

```
pixel-cat/                  three-skies/
  manifest.json               manifest.json
  pet.json                    dusk.png
  spritesheet.webp            lake.png
  preview.png    optional     aurora.jpg
  LICENSE.txt    optional     preview.png    optional
```

- `pet`: the files of a pet as she reads them already: both `pet.json` (a JSON object: `displayName`, `author`, `spriteVersionNumber` 2 for the 8 × 11 sheet) and `spritesheet.webp` are required, at the pack's top level beside its `manifest.json`; other pictures (`.webp .png .jpg .jpeg`) and words (`.json .md .txt`) may come along. Installed into `wakuwaku-data\pets\<id>\`, it is listed with her other pets on the Look page (来自市场, From the market, or 未经审核, Not reviewed, under its name) and chosen there with 使用 (Use).
- `wallpaper`: pictures (`.jpg .jpeg .png .webp`), at the pack's top level, one at least; words (`.json .md .txt`) may come along. Pictures in sub-folders are not installed. Installed into `wakuwaku-data\wallpapers\<id>\`, it gets a row under the Look page's Desktop, 「壁纸包 · N 张」 (Wallpaper pack · N pictures), and 用这一包 (Use this pack) makes that folder Windows' slideshow, as 选文件夹… (Folder…) with it would.
- Either may have a `preview.webp`, `preview.png` or `preview.jpg` at its top level, 64 KB or less: the market's page shows it, and once installed her pages draw it as the pack's tile. It is neither a wallpaper nor part of the pet: it is kept beside the packs' folders, not in its own.
- An `id` is unique among packs of its kind. A pet pack whose `id` is the name of a pet already there that is no pack's (the default `claude-chan`, one downloaded from codex-pets.net, one made by hand, one of Codex's) is refused, and that pet is left as it is; so is a wallpaper pack whose `id` is a folder in her wallpapers that is no pack's. She marks each pack's folder with a `pack.json` of her own (a pack's own `pack.json` is replaced by hers).
- The same `id` again is its update (from the market's daily look, or a newer `.zip` from a file): the files are replaced, and what was chosen stays chosen: the pet in use is still her, the slideshow on its folder goes on. Uninstalled from its page on the Plugins page (资源包, Packs), the pet in use gives way to the default pet; a slideshow on its folder ends on the picture it showed, so the desktop stays as it was.
- From a file, as a plugin: Plugins page › 显示 › 从文件安装… (Install from file…). She asks first, marks it not reviewed, and says it is pictures only.
- The repository keeps no example pack. There is none for a pet either: the default pet's art is not free to hand on as a pack of its own.

## The market

The market is a public repository of packages and an `index.json` built from them, served from GitHub and from Gitee for China. Authors open a pull request. A check runs first (`tools/market/check.js`: format, file types, programs, size), then the maintainer reviews and merges. `tools/market/README.md` says how to run the checks yourself before you submit.

In Wakuwaku it is Plugins › 获取更多 (Get more), and the widget library's 获取更多… (Get more…), which opens it on its widgets:

- Its index is read when the page opens and is an hour old, through the Network proxy setting, from GitHub, or Gitee first in China (as her own updates go), the other when one fails. While the repository does not exist, the page says the market is not open yet.
- Each entry's page shows your cards drawn from `widget.sample`, or a pack's preview, then the description and what it can do, as a plugin's page does once installed.
- Installing: one that asks for `agents`, `hardware` or `notify` is asked about first; one that asks for the web alone installs at once (its hosts are listed on its page), and so does a pack. The zip is installed only when its size and SHA-256 are the index's and its manifest's `id`, `version` and `kind` are the entry's; then it is checked as any zip is. It is marked reviewed, and the permissions it asks for are recorded as granted. The Look page's 获取更多宠物… (Get more pets…) and 获取更多壁纸… (Get more wallpapers…) open the market on its pets and its wallpapers.
- Updates: she looks once a day (only when something from the market is installed). A newer version asking for nothing beyond what was granted (a pack's always) installs by itself and its frame loads again, unless 插件自动更新 (Update plugins automatically) is off. One asking for more waits: the Plugins page says 「N 个插件有更新」 with 全部更新 (Update all), which first lists what each newly asks for. Raise `version` for every change, or nobody gets it.
