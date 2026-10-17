# 预览工具：样例页、模拟器、改前改后

用**真实的页面代码**（`src/pet` 的 index.html、desk.html、bar.html 和它们的脚本、样式）在浏览器里跑她的界面，Tauri 换成一个替身（`fake-tauri.js`）。不改产品代码。三样东西共用一份**场景**（`scenes.js`）：

- **模拟器**（试玩页）：一张假桌面，亲手点、拖、悬停；改代码时本地实时预览，验收时发成 Artifact。
- **样例页**：每个场景、每个状态截成 PNG，深浅色各一张，`docs/design/style.md` 的「样子」。
- **改前改后**：同一批场景，用两个版本的代码各拍一遍，左右拼在一起。

界面改动的循环：本地 `dev` 边改边看 → `compare.js` 出对比图 → 协调发一份分支版模拟器给用户试 → 合并。

## 场景（scenes.js）

一个**场景**是一整个世界：她的家、深浅色、条带、会话和询问、小工具和插件、任务栏和桌面的布局、设置；外加放起来时发生什么（`play`：一轮做完、打开设置、进入布局编辑）。场景只在 `SCENES` 里定义一次：

- 模拟器的「场景」一栏一键切过去，`#<id>` 打开时直接进这个场景（托管页只把 `#记号` 传进来，没有 `?参数`）；
- 样例页的一张图（`LIST` 里一项）= 一个场景 + 拍什么（哪个页面、屏幕上哪一块、拍前再做什么）。`page: 'screen'` 拍整个屏幕（桌面页在下，她的家在上，和模拟器一样）。还有一批只有样例页用的细节图，各带一个小世界（`world`）。

现在的场景：新用户第一次打开、平常、三个会话在忙、等你批准、做完了（流光）、出错了、放满小工具的任务栏、桌面、空桌面、灵动岛模式、浅色 + 透明任务栏、编辑任务栏布局中、添加小工具（编辑任务栏，右边开着小工具库）、编辑胶囊（胶囊上方的面板）。

加一个场景：在 `SCENES` 里加一项（`id`、`title`、`note`、`world`，可选 `home`、`mode`、`play`），在 `LIST` 里加一张它的图（`scene: '<id>'`、`section: 'scenes'`、`page`、`view`）。`test/preview.test.js` 会查世界是否完整、布局里的小工具是否都在、每个场景有没有图。新数据放进 `fixtures.js`，新的「Rust 会发的东西」放进 `fake-tauri.js`。

## 模拟器（playground）

```powershell
node tools/preview/publish.js dev [端口]                    # 本地实时：http://127.0.0.1:8765/，直接读仓库里的文件
node tools/preview/publish.js build <文件夹> [--label <文字>]  # 要发布的文件 + files.json，打出总大小
node tools/preview/publish.js serve [端口] [文件夹]          # 按发布的样子在本地试 build 的结果
```

- **dev**：不打包，按发布时的路径（`preview/`、`pet/`、`shared/`、`pets/`、`assets/`）直接读 `src/pet`、`src/shared`、`tools/preview`。改了文件，开着的页面自己跟上：她页面的样式就地换掉；她页面的脚本或 HTML 只重新载入用到它的那一块（世界不变，开着的设置、正在编辑的布局会再打开）；模拟器自己的文件（`playground.*`、`fake-tauri.js`、`fixtures.js`、`scenes.js`）整页重载，世界、场景、控制台的状态带过去。页头的绿点标签写着「本地实时 · 分支 … · 提交」。build 会漏掉的文件，dev 在控制台里说一声。
- **页头标签**：`build` 默认从 git 取「主线 · 3ec87d9」或「分支 simulator · 3ec87d9」，工作区有没提交的改动再加「+ 未提交的改动」；`--label` 换成自己的话（`--label ""` 不要标签）。
- **发布（协调来做）**：主线版发到固定的那个链接；分支要验收时，另 `build` 一份带分支标签的，单独发一个 Artifact。发布用 Artifact 工具：`file_path` 是文件夹里的 `playground.html`，`files` 是 `files.json` 那张表（发布路径 → 本地文件）。更新已有的链接时带上 `url`。
- `playground.html` 只有页面内容（title、meta、一个 style、标记和脚本），托管方会套上 html/head/body；`serve`、`dev` 也照这样套。仓库里这一份只能通过 `publish.js` 打开。
- `playground.js` 替 Rust 回答两个页面（`window.WakuHost`）：设置的保存和回送、询问的回答、桌面布局、程序按钮的前后台和固定、音量、任务栏布局编辑时的键盘。它还代画 Windows 画的东西：她的右键菜单（含「添加小工具…」「编辑胶囊」）、窗口的实时预览、岛下面的亚克力；从小工具库拖到桌面时，替 Rust 把指针位置和松手转给桌面页（`desk_drag_preview`、`desk_drop`，桌面页回 `desk_drag_answer`）。指针按她的窗口那样「穿过」：哪个页面的元素在指针下面，哪个 iframe 才接住指针。需要 Windows 的事（开始菜单、打开程序、连接 agent）不做，在左下角说一声。
- 本地还能用参数：`?home=island`、`?mode=light`、`?material=clear`、`?edge=left`；截图钩子 `?demo=open|card|done|ring|ask|answer|edit|menu|thumbs|settings|probe|check`，加 `&still=1` 让过渡停在终点。`window.WakuPlay` 给外面的检查用（当前场景、frame 载入次数、换样式次数）。无头 Edge 窗口最窄 500 px；看 400 px 宽用 `http://127.0.0.1:8765/__frame?w=400&h=860`。

## 样例页（gallery）

```powershell
pwsh tools/preview/render.ps1 -Out <输出文件夹>                    # 全部，深色和浅色
pwsh tools/preview/render.ps1 -Out <文件夹> -Only scene-*,flow-* -Modes dark
pwsh tools/preview/render.ps1 -Open                                 # 在 Edge 里打开活的 gallery.html
```

- 每张图出 `<id>-dark.png`、`<id>-light.png`（默认 2 倍像素），再拼成 `overview-dark.png`、`overview-light.png`。需要 node 和 Edge；6 个 Edge 并行，不在屏幕上画东西，也不碰正在跑的她。
- 截早了（页面还没冻住）的图左上角有个品红方块，页面出错的图底部有一条红色说明；脚本最后会列出来，退出码是 1。
- 只看一张：`shot.html?scene=<图的 id>&mode=dark`；看页面本身并打出调试信息：`page.html?role=island&scene=<id>&mode=dark&debug=1&probe=%23island`。用 file:// 打开时 Edge 要带 `--allow-file-access-from-files`（`-Open` 会带上）。
- 时间、动画、计时器都固定（时钟从 14:42 开始走），同一台机器上两次截图几乎一样，只有模糊和抗锯齿的个别像素差 1。要停在动画中间，在图的 `steps` 里 `p.freeze({ flow: 1100 })`。

## 改前改后（compare.js）

```powershell
node tools/preview/compare.js                                   # main 对 这个工作区现在的文件（含没提交的）
node tools/preview/compare.js --base main --head <分支> --only scene-*,capsule-* --modes dark --out <文件夹>
```

- 两边用**同一套**拍摄工具（工作区的，或 `--head` 的 `tools/preview`），只有她的代码（`src/pet`、`src/shared`、立绘）各用各的；`--base` 和 `--head` 的代码用 `git archive` 取到临时文件夹，不碰任何检出。
- 出来的：每张图每种模式一张 `<id>-<mode>.png`，左「改前」右「改后」，变了的地方用品红框框出来；`index.html` 按分区列出全部，默认只看有变化的；`before/`、`after/` 是两边原图。没路径时放在临时文件夹，最后打出来。
- 一个通道差超过 24、且超过 8 个像素，才算有变化（滤掉抗锯齿的噪声）。

## 文件

| 文件 | 做什么 |
|---|---|
| `fake-tauri.js` | Tauri 的替身：一份可改的世界（`Fake.world`），`Fake.set()` 改它，`Fake.push()` 按 Rust 的样子发给页面，页面的 `invoke` 从世界里回答。 |
| `fixtures.js` | 样例数据：会话、询问、小工具、插件、程序按钮、托盘、桌面和任务栏布局。node 也能读。 |
| `scenes.js` | 场景（`SCENES`）和样例页的图（`LIST`）。`node scenes.js --list`。 |
| `boot.js` + `page.html` | 读进真实页面（去掉 CSP），先装替身再跑它的脚本；拍图时放场景、跑步骤，然后冻住。 |
| `shot.html` + `shot.js`、`gallery.html`、`render.ps1` | 一张图、所有图活的样子、截图和总览。 |
| `playground.html` + `playground.js` | 模拟器。 |
| `publish.js`、`gitinfo.js` | dev / build / serve；从 git 取标签。 |
| `compare.js` + `pairs.ps1` | 改前改后：两边各拍一遍、比像素、拼图、写 index.html。 |
| `assets/wall.jpg` | 仓库 `wallpapers/gpt-cloud-terrace.png` 缩到 1920×1080。 |
