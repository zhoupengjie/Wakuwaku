// The scenes, and the gallery's shots of them.
//
// A scene is a whole world (fake-tauri.js): her home, Windows' mode, the
// strip, the sessions and prompts, the widgets and plugins, the taskbar's
// and the desktop's layouts, the settings. Each is defined once, here
// (SCENES), and used two ways:
//   played  the playground's 场景 group sets its world to it and plays it
//           (playground.js), and #<id> opens the playground on it;
//   shot    the gallery shoots it (LIST: a shot is a scene plus what to
//           shoot), with render.ps1, gallery.html and compare.js.
//
// A scene:
//   id      its name (the playground's #<id>)
//   title   as the playground's button says it
//   note    what there is to try in it (under the buttons)
//   home    her home: taskbar (the default) or island
//   islandPos  at the top of the screen, where the island sits: left,
//           center (the default), right (config.islandPos; its window put there)
//   islandEdge the edge of the screen it is along: top (the default), bottom
//           (config.islandEdge, zhoupengjie/Wakuwaku#105; its window at the foot)
//   mode    dark or light, for a scene about one (else the viewer's, or the shot's)
//   world   F => ({...}): the rest of its world (F: fixtures.js), whole:
//           every one of WORLD_KEYS (test/preview.test.js checks)
//   play    async p => {...}: what happens once her pages are up (a turn
//           ending, the settings opening), on her home's page
//   deskPlay  async p => {...}: the same on the desktop's page (p.win: its
//           window; widgets picked with Ctrl, their menu open)
//
// A shot (LIST), one picture of the gallery:
//   id        its name, and the PNG's (<id>-dark.png, <id>-light.png)
//   section   which part of the sheet (SECTIONS)
//   title     what it shows, as the sheet labels it
//   page      the page that draws it: island (index.html, her home's window),
//             desk (desk.html, the desktop's widgets), bar (bar.html, her
//             taskbar on another display), or screen (the desktop's page
//             under her home's, the whole screen as the playground has it)
//   scene     the scene it shoots; or world: F => ({...}), a world of its
//             own (the detail shots below: one state each, on a bare desktop)
//   home      her home, over the scene's
//   islandPos where the island sits at the top, over the scene's
//   islandEdge the edge it is along, over the scene's
//   view      [x, y, w, h]: the part of the 1600 x 900 screen shot
//   play      false: the scene's play left out
//   steps     async p => {...}: what it does after the play (p: boot.js),
//             on the page shot (screen: her home's)
//   budget    the page's time in ms (virtual: a timer of 3 s takes none of
//             the clock's) until it is shot; the page is frozen before it
//   settle    ms after the steps before the page is frozen (default 500)
//
// What p has, in the gallery (boot.js) and the playground (playground.js)
// alike: F, world, wait, set, push, emit, hover, click, context; the
// gallery's also freeze and specimen.
//
// Node reads all this (render.ps1, compare.js, the tests): node scenes.js --list.
;(function (root) {
  const SCREEN = [1600, 900]
  const STRIP = 48
  // Her home's window on the screen (island.rs): the taskbar's the screen
  // wide and the strip with its room above (640); the island's at the top
  // in the middle (760 wide, as tall as it grows: kept tall here).
  const WINDOWS = {
    taskbar: [0, SCREEN[1] - STRIP - 640, SCREEN[0], STRIP + 640],
    island: [(SCREEN[0] - 760) / 2, 0, 760, 720],
    desk: [0, 0, SCREEN[0], SCREEN[1]],
    // Her taskbar on another display (bars.rs): the strip alone.
    bar: [0, SCREEN[1] - STRIP, SCREEN[0], STRIP],
    // Split (taskbarEdge split, a shot's split): her strip the top bar, its
    // room under it; the dock's window the band along the foot, 48 high, its
    // glass in its middle 2 px off the foot (bars.rs dock_band, bar.css .b-dock).
    top: [0, 0, SCREEN[0], STRIP + 640],
    dock: [0, SCREEN[1] - 48, SCREEN[0], 48],
  }
  // The island's window at a top corner (islandPos: left, right), as
  // island.rs puts it (zhoupengjie/Wakuwaku#59; anchor.js
  // windowLeft): at the screen's side; the island in it 8 px from that side.
  // Windows' own taskbar at the bottom, not drawn: the work area's top and
  // sides are the screen's. Along the foot of the screen (islandEdge
  // bottom, zhoupengjie/Wakuwaku#105; anchor.js windowTop): its foot on the
  // screen's, over where Windows' taskbar would be, as tall as it is kept here.
  function islandWindow(pos, edge) {
    const [, y, w, h] = WINDOWS.island
    const x = pos === 'left' ? 0 : pos === 'right' ? SCREEN[0] - w : WINDOWS.island[0]
    return [x, edge === 'bottom' ? SCREEN[1] - h : y, w, h]
  }

  const SECTIONS = [
    ['scenes', '一键场景（试玩页的「场景」）'],
    ['taskbar', '任务栏条带'],
    ['capsule', '任务栏上的胶囊'],
    ['island', '屏幕顶部的灵动岛'],
    ['flow', '流光提醒'],
    ['menus', '菜单'],
    ['settings', '设置'],
    ['controls', '控件'],
    ['widgets', '小工具'],
    ['desktop', '桌面'],
    ['quiet', '没人在（不该有一直播放的动画）'],
    ['readme', 'README 的配图（readme.ps1）'],
  ]

  // The strip's foot is the screen's: a view's top this far above it.
  const above = h => SCREEN[1] - h

  // --- The scenes ------------------------------------------------------------------------
  // What a scene's world says, all of it (with home and mode, from the scene).
  const WORLD_KEYS = ['sessions', 'asks', 'widgets', 'plugins', 'windows', 'tray', 'keys', 'taskbarLayout', 'deskLayout', 'edge', 'material', 'config', 'connected', 'quiet']

  // The desktop each scene starts from: her widgets and plugins, the
  // programs, the tray and keyboard, Windows 11's taskbar, an everyday
  // desktop (a stack, a board, two alone), the strip in Mica along the
  // bottom, an agent connected; nobody at work.
  const desktop = F => ({
    sessions: [],
    asks: [],
    widgets: F.widgets('monitor', 'today', 'weather', 'tokens', 'inbox-a1', 'pomodoro', 'countdown', 'stock-aapl', 'deploy'),
    plugins: F.PLUGINS,
    windows: F.WINDOWS,
    tray: F.TRAY,
    keys: F.KEYS,
    taskbarLayout: F.TASKBAR.win11,
    deskLayout: F.DESK.everyday,
    edge: 'bottom',
    material: 'mica',
    // Two boards kept as combos (the picker's 组合): 写代码, 摸鱼.
    config: { boardSets: F.BOARD_SETS },
    connected: true,
    quiet: false,
  })
  // One at work, one just done.
  const workingAndDone = F => [F.session('working'), { ...F.others()[1], id: 's2' }]
  const codex = F => F.others()[0]

  // The first session's turn played to its end, as the world has it (done,
  // an error): at work a moment, then ended, so the ending shows as it
  // comes (the flow light).
  const turnEnds = async p => {
    const [last, ...rest] = p.world.sessions
    p.set({ sessions: [p.F.session('working', { id: last.id, name: last.name, project: last.project, agent: last.agent }), ...rest] })
    p.push()
    await p.wait(300)
    p.set({ sessions: [last, ...rest] })
    p.push()
  }

  const SCENES = [
    {
      id: 'first-run',
      title: '新用户第一次打开',
      note: '什么都还没连上：设置开在「通用」顶上的欢迎卡片。关掉设置，胶囊上有个强调色小点，提示去连接。',
      world: F => ({
        ...desktop(F),
        widgets: F.widgets('monitor', 'today', 'tokens'),
        plugins: F.PLUGINS.map(p => ({ ...p, on: false, state: 'off' })),
        deskLayout: [],
        // A new user's taskbar is clear (data.rs defaults); her capsule bare in dark.
        material: 'clear',
        connected: false,
        config: { onboarded: false },
      }),
      // As main does on the first start: the settings, where she is connected.
      play: async p => {
        await p.wait(200)
        p.emit('island:settings', { tab: 'connect' })
      },
    },
    {
      id: 'everyday',
      title: '平常',
      note: '一个会话在干活，一个刚做完。鼠标停在胶囊上会展开；桌面上的小工具可以拖。',
      world: F => ({ ...desktop(F), sessions: workingAndDone(F) }),
    },
    {
      id: 'three-busy',
      title: '三个会话在忙',
      note: 'Claude 两个、Codex 一个，都在干活。胶囊上显示最近变化的那个，展开能看到三个。',
      world: F => ({ ...desktop(F), sessions: [F.session('working'), codex(F), F.reading()] }),
    },
    {
      id: 'ask',
      title: '等你批准',
      note: '一个会话要跑一条命令，在等你点头：胶囊自己展开成询问卡片，按钮停一下才能点。',
      world: F => ({ ...desktop(F), sessions: [F.session('waiting'), codex(F)], asks: [F.ASK] }),
    },
    {
      id: 'done-flow',
      title: '做完了（流光）',
      note: '一轮刚做完：胶囊外的彩色光环一步一步地转两圈，静静亮一会儿，变成一个绿点。再点一次这个场景就重播。',
      world: F => ({ ...desktop(F), sessions: [F.session('done'), codex(F)], config: { endCue: 'ring' } }),
      play: turnEnds,
    },
    {
      id: 'error',
      title: '出错了',
      note: '一轮以出错收尾（触发了限流）：流光之后留下红点，展开能看到原因。',
      world: F => ({ ...desktop(F), sessions: [F.session('error'), codex(F)], config: { endCue: 'ring' } }),
      play: turnEnds,
    },
    // The hold setting (停留多久, zhoupengjie/Wakuwaku#65), for each way of telling.
    {
      id: 'hold-ring',
      title: '提醒停留',
      note: '「停留多久」是 30 秒：一轮刚做完，光环转两圈后静静亮着，到 30 秒才变成小点；鼠标停到胶囊上就算看过。右边「发生点什么」里的做完、改好、出错、打招呼能一次次再来；「她有事告诉你时」里能换停留时间（含自定义秒数）。',
      world: F => ({ ...desktop(F), sessions: [F.session('done'), codex(F)], config: { boardSets: F.BOARD_SETS, endCue: 'ring', hold: 30 } }),
      play: turnEnds,
    },
    {
      id: 'hold-open',
      title: '展开停留',
      note: '「她有事告诉你时」选了「展开胶囊」，停留 30 秒：一轮做完，胶囊自己展开，开 30 秒再收起；鼠标停上去再移开就马上收起，那个会话又开始干活也收起。换成「看到为止」，就一直开着，等鼠标经过。',
      world: F => ({ ...desktop(F), sessions: [F.session('done'), codex(F)], config: { boardSets: F.BOARD_SETS, endCue: 'open', hold: 30 } }),
      play: turnEnds,
    },
    {
      id: 'taskbar-full',
      title: '放满小工具的任务栏',
      note: '任务栏上两个中档、一个轮播、一个叠放，她旁边还有一个小的。点叠放会展开；右键能换大小、挪到别处。',
      world: F => ({
        ...desktop(F),
        sessions: [F.session('working')],
        widgets: F.widgets('monitor', 'weather', 'today', 'tokens', 'inbox-a1', 'stock-aapl', 'stock-nvda', 'pomodoro', 'countdown', 'deploy'),
        taskbarLayout: F.TASKBAR.full,
      }),
    },
    {
      id: 'desktop',
      title: '桌面',
      note: '桌面上有单个的、一个叠放、两个工作台（一个折叠着）。拖到同样大小的另一个上停半秒就叠起来；双击工作台的标题改名。',
      world: F => ({ ...desktop(F), sessions: [F.session('working')], widgets: F.WIDGETS, deskLayout: F.DESK.full }),
    },
    {
      id: 'calendar-media',
      title: '日历和正在播放',
      note: '她替你跑的两个插件：日历（日期、农历和节气、订阅的日历里接下来的安排、月历）和正在播放（电脑上在放的歌、进度，卡片上的上一首、暂停、下一首）。桌面上各三档；胶囊里各一行，点 › 展开成卡片。',
      world: F => ({
        ...desktop(F),
        sessions: [F.session('working')],
        widgets: [...F.widgets('monitor', 'today', 'tokens'), F.SLEEPING.calendar, F.SLEEPING.media],
        plugins: F.PLUGINS.map(p => (p.id === 'calendar' || p.id === 'media' ? { ...p, on: true, state: 'running' } : p)),
        deskLayout: F.DESK.calendarMedia,
      }),
    },
    {
      id: 'desktop-empty',
      title: '空桌面',
      note: '桌面上一个小工具都没有。右键胶囊里或任务栏上的小工具，选「桌面」就放上来。',
      world: F => ({ ...desktop(F), sessions: [F.session('working')], deskLayout: [] }),
    },
    {
      id: 'island-home',
      title: '灵动岛模式',
      home: 'island',
      note: '她的家在屏幕顶部的灵动岛：鼠标停上去展开。底下的任务栏是 Windows 自己的，这里不画。',
      world: F => ({ ...desktop(F), sessions: workingAndDone(F) }),
    },
    {
      id: 'island-right',
      title: '岛在右上',
      home: 'island',
      islandPos: 'right',
      note: '灵动岛放在屏幕右上角（设置「外观 → 位置」），这里是展开的样子：朝屏幕中间长。流光、小点、往上收起、右键的「添加小工具…」「编辑胶囊」都跟着它。右边「环境」里能换左上、居中、右上。',
      world: F => ({ ...desktop(F), sessions: workingAndDone(F) }),
      // Opened, as the pointer resting on it opens it.
      play: async p => {
        await p.wait(200)
        p.hover('#island')
      },
    },
    {
      id: 'island-bottom',
      title: '岛在底部',
      home: 'island',
      islandEdge: 'bottom',
      note: '灵动岛放进 Windows 自己的任务栏里（设置「外观 → 放在」，zhoupengjie/Wakuwaku#105；真机上胶囊是任务栏里她自己的小窗口，贴着托盘左边或任务栏左端；这里任务栏不画，岛的页面按盖在任务栏上的样子画：胶囊停在那一条的正中），鼠标停上去往上展开，拉她出来往上拉，设置立在岛上方。右边「环境」里能换屏幕顶部、任务栏里，也能换左、居中、右。',
      world: F => ({ ...desktop(F), sessions: workingAndDone(F) }),
      play: async p => {
        await p.wait(200)
        p.hover('#island')
      },
    },
    {
      id: 'light-clear',
      title: '浅色 + 透明任务栏',
      mode: 'light',
      note: 'Windows 浅色，任务栏透明：条带直接透出壁纸，字和图标换成深色。',
      world: F => ({ ...desktop(F), sessions: workingAndDone(F), material: 'clear' }),
    },
    {
      id: 'edit-layout',
      title: '编辑任务栏布局中',
      note: '正在编辑任务栏布局：各部分可以拖动，「+」打开小工具库；点完成、取消，或按 Esc 退出。',
      world: F => ({
        ...desktop(F),
        sessions: [F.session('working')],
        widgets: F.widgets('monitor', 'weather', 'today', 'tokens', 'inbox-a1', 'stock-aapl', 'stock-nvda', 'pomodoro', 'countdown', 'deploy'),
        taskbarLayout: F.TASKBAR.widgets,
      }),
      // As her menu's 编辑任务栏布局 does.
      play: async p => {
        await p.wait(200)
        p.emit('taskbar:edit', null)
      },
    },
    {
      id: 'add-widgets',
      title: '添加小工具',
      note: '右键任务栏选了「添加小工具…」：任务栏进入编辑，右边是小工具库。把卡片拖到任务栏（会让出位置）、她的胶囊、桌面或「工作」板上；单击加到托盘前。鼠标停在任务栏的小工具上能设置或移除，把它拖回库里、或拖到任务栏上方那条空地也是移除，都能撤销；拖到桌面就是搬过去。随改随存，Esc 先关库，再按一次就完成。',
      world: F => ({
        ...desktop(F),
        sessions: [F.session('working')],
        taskbarLayout: { left: ['her'], center: ['start', 'apps'], right: [{ id: 'wtw', type: 'plugin', of: 'weather', size: 'M', icon: 'weather' }, 'monitor', 'tray', 'keys', 'quick', 'clock'] },
      }),
      // As her menu's 添加小工具… does.
      play: async p => {
        await p.wait(200)
        p.emit('taskbar:edit', { picker: true })
      },
    },
    {
      id: 'edit-capsule',
      title: '编辑胶囊',
      note: '编辑任务栏时点了她的胶囊：上方是胶囊里的小工具。按住左边的点拖动排序（就是轮播和展开时的顺序）；每个选轮播、固定（最多五个，显示在胶囊右端）或展开时（只在展开的岛里）；× 移出胶囊（能撤销）；底下是轮播间隔。从小工具库把卡片拖到这块面板上也能加进来。',
      world: F => ({
        ...desktop(F),
        sessions: [],
        deskLayout: F.DESK.everyday.filter(e => e.id === 'wd1' || e.id === 'wd2'),
        config: {
          boardSets: F.BOARD_SETS,
          capsule: {
            members: [
              { key: 'plugin:weather', mode: 'pin' },
              { key: 'widget:today', mode: 'turn' },
              { key: 'plugin:pomodoro', mode: 'turn' },
              { key: 'widget:inbox-a1', mode: 'open' },
              { key: 'widget:tokens', mode: 'turn' },
              { key: 'plugin:countdown', mode: 'turn' },
            ],
          },
        },
      }),
      // Editing the taskbar, then a press on her capsule (as 编辑胶囊 does at the top).
      play: async p => {
        await p.wait(200)
        p.emit('taskbar:edit', null)
        await p.wait(300)
        p.emit('island:picker', { open: 'capsule' })
      },
    },
    {
      id: 'combos',
      title: '组合',
      note: '小工具库的「组合」：存好的工作台（写代码、摸鱼）。拖到桌面上就在那儿按原样搭起来，单击放到主屏的下一个空位；里面的小工具从原来的地方搬过来并打开。拖到任务栏或胶囊上不行。右键卡片（或卡片上的 ⋯）能改名、删除（能撤销）。存一个新的：工作台的 ⋯ →「存为组合…」。',
      world: F => ({ ...desktop(F), sessions: [F.session('working')] }),
      // As her menu's 添加小工具…, then its 组合.
      play: async p => {
        await p.wait(200)
        p.emit('taskbar:edit', { picker: true })
        await p.wait(400)
        p.click('.pk-chip[data-cat="sets"]')
      },
    },
    {
      id: 'multi-select',
      title: 'Ctrl 多选',
      note: '按住 Ctrl 在桌面空白处拖出一个框，或按住 Ctrl 点小工具（单个或叠放；工作台不行），选中几个；右键其中一个：「组成工作台」放在最左上那个的位置，接着直接给它起名；「从桌面移除」把它们都放回胶囊。不按 Ctrl 点一下、按 Esc 或点别处就取消选择。',
      world: F => ({ ...desktop(F), sessions: [F.session('working')], widgets: F.WIDGETS, deskLayout: F.DESK.full }),
      // Four picked (one alone, a stack, two at the far side) and their menu, as Ctrl and presses would.
      deskPlay: async p => {
        await p.wait(500)
        const D = p.win.Desk
        if (!D?.select) return
        D.select(['wa', 'wst', 'wfm', 'wfd'])
        await p.wait(100)
        const r = p.win.document.querySelector('[data-id="wfd"]').getBoundingClientRect()
        D.openMenu('wfd', 'sel', { x: r.left + 60, y: r.top + 40 })
      },
    },
  ]
  const sceneById = id => SCENES.find(s => s.id === id) || null

  // --- The shots -------------------------------------------------------------------------
  // What every detail shot on her home's page starts with: widgets, the
  // plugins, the taskbar's programs, tray and keyboard, Windows 11's layout.
  const home = F => ({
    widgets: F.widgets('monitor', 'weather', 'today', 'inbox-a1', 'stock-aapl', 'deploy'),
    plugins: F.PLUGINS,
    windows: F.WINDOWS,
    tray: F.TRAY,
    keys: F.KEYS,
    taskbarLayout: F.TASKBAR.win11,
  })
  // A session in a mood, the island left to say it as it would.
  const inMood = (mood, extra = {}) => F => ({ ...home(F), sessions: [F.session(mood)], asks: mood === 'waiting' ? [F.ASK] : [], ...extra })
  // An ending as a dot alone (endCue none): the closed island as it stays.
  const ended = mood => F => ({ ...inMood(mood)(F), config: { endCue: 'none' } })
  const busy = F => ({ ...home(F), sessions: [F.session('working'), ...F.others()] })

  const openIsland = async p => {
    p.hover('#island')
    await p.wait(400)
  }
  const openCard = async p => {
    await openIsland(p)
    p.click('#island-expanded [data-widget="weather"]')
    await p.wait(500)
  }
  const openIsleMenu = async p => {
    await openIsland(p)
    p.context('#island-expanded [data-widget="weather"]', 60, 12)
    await p.wait(300)
  }
  // Her capsule with five pinned (the most), the monitor first, two taking
  // turns; the strip without the monitor's part (so its readings are the
  // capsule's: #53).
  const PINS5 = {
    members: [
      { key: 'widget:monitor', mode: 'pin' },
      { key: 'plugin:weather', mode: 'pin' },
      { key: 'widget:today', mode: 'pin' },
      { key: 'plugin:stock', mode: 'pin' },
      { key: 'plugin:pomodoro', mode: 'pin' },
      { key: 'plugin:countdown', mode: 'turn' },
      { key: 'widget:tokens', mode: 'turn' },
    ],
  }
  const NO_MONITOR_PART = { left: ['her'], center: ['start', 'apps'], right: ['tray', 'keys', 'quick', 'clock'] }

  // Other people's three (fixtures.js ADDONS) among hers, as the Plugins
  // page and the picker show them: 下雨提醒 at work on the desktop (medium,
  // right of the everyday board), 数数 from a file and not placed, 天气雷达
  // not responding (on: in her capsule); a folder that did not install.
  const withAddons = F => ({
    ...busy(F),
    widgets: F.widgets('monitor', 'today', 'tokens', 'weather', 'stock-aapl', 'stock-nvda', 'pomodoro', 'countdown', 'inbox-a1', 'deploy'),
    deskLayout: [...F.DESK.everyday, { id: 'wra', type: 'plugin', of: 'rain-alert', size: 'm', name: '下雨提醒', icon: 'weather', d: 'd1', at: { d1: [10, 0] } }],
    addons: F.ADDONS,
    addonsBroken: F.ADDONS_BROKEN,
  })
  // All three at work (天气雷达 too: its letter for an icon), for where their widgets go.
  const addonsRunning = F => F.ADDONS.map(a => ({ ...a, on: true, state: 'running', said: null }))
  const openPluginsPage = async (p, tab = 'widgets') => {
    p.emit('island:settings', { tab })
    // A row asked for: past the settings' last scroll to it (1.6 s), as it stays.
    await p.wait(tab === 'widgets' ? 600 : 1800)
  }
  // 从文件安装…, Windows' dialog picking the world's .zip (fake-tauri.js addonManage).
  const installFromFile = async p => {
    await openPluginsPage(p)
    p.click('#island-settings [data-addon-file]')
    await p.wait(400)
  }
  // The market, as a link opens it (widgets:market[:kind]), its index read
  // (fake-tauri.js market: a quarter of a second); one of its items' page.
  const openMarket = async (p, link = 'widgets:market') => {
    p.emit('island:settings', { tab: link })
    await p.wait(700)
  }
  const openMarketItem = async (p, id) => {
    await openMarket(p)
    p.click(`#island-settings [data-mk-item="${id}"]`)
    // Her desktop's card templates loaded the first time (wdesk.js), and desk.css in their shadow.
    await p.wait(900)
  }
  const addonShot = (id, title, steps, more = {}, shot = {}) => ({ id, section: 'settings', title: `设置 · 插件 · ${title}`, page: 'island', home: 'taskbar', view: [0, above(660), 560, 660], world: F => ({ ...withAddons(F), ...more }), steps, ...shot })
  // The packs installed (fixtures.js PACKS), 像素猫 among her pets, Windows'
  // slideshow on 极光's folder.
  const withPacks = F => ({
    ...withAddons(F),
    packs: F.PACKS,
    packPets: F.PACK_PETS,
    paper: { file: `${F.PACKS[1].folder}\\3.jpg`, slideshow: true, folder: F.PACKS[1].folder, position: 'fill', every: 1800000, shuffle: false },
  })
  // Most of them scroll to a part of the page first (1.6 s): a longer budget.
  const packShot = (id, title, steps, more = {}) => ({ ...addonShot(id, title, steps), title: `设置 · ${title}`, world: F => ({ ...withPacks(F), ...more }), budget: 4000 })
  const WHOLE = [0, 0, SCREEN[0], SCREEN[1]]
  const STRIP_VIEW = [0, above(88), SCREEN[0], 88]
  const CLOSED = [0, above(80), 440, 80]

  // The capsule's and the island's states, each for one home.
  const states = (section, home, view) => [
    { id: `${section}-idle`, section, title: '收起 · 空闲（小工具轮流显示）', page: 'island', home, view: view.closed, world: F => ({ ...inMood('idle')(F), home }) },
    { id: `${section}-working`, section, title: '收起 · 干活中', page: 'island', home, view: view.closed, world: F => ({ ...inMood('working')(F), home }) },
    { id: `${section}-waiting`, section, title: '等你 · 询问卡片', page: 'island', home, view: view.ask, world: F => ({ ...inMood('waiting')(F), home }), settle: 900 },
    { id: `${section}-done`, section, title: '收起 · 搞定（留一个小点）', page: 'island', home, view: view.closed, world: F => ({ ...ended('done')(F), home }) },
    { id: `${section}-error`, section, title: '收起 · 出错（留一个小点）', page: 'island', home, view: view.closed, world: F => ({ ...ended('error')(F), home }) },
    { id: `${section}-open`, section, title: '展开 · 会话和小工具', page: 'island', home, view: view.open, world: F => ({ ...busy(F), home }), steps: openIsland },
    { id: `${section}-card`, section, title: '展开 · 天气的卡片打开', page: 'island', home, view: view.card, world: F => ({ ...busy(F), home }), steps: openCard },
  ]

  // The README's cards (readme.ps1 puts them in docs/readme/): a close view
  // of a scene each, 16:9 (the taskbar's a strip), in Chinese and in
  // English (lang). In English the world's own words are English too: the
  // fixtures' names and labels, as EN has them (inEnglish, in worldFor).
  const EN = {
    任务栏设计: 'Taskbar design',
    胶囊的流光: 'Capsule glow',
    邮件同步: 'Mail sync',
    周报: 'Weekly notes',
    发布说明: 'Release notes',
    '写好了。': 'Done.',
    'Dresden · 阴': 'Dresden · Cloudy',
    工作: 'Work',
    '周五见：我订好了那家餐厅': 'See you Friday, I booked the restaurant',
    'Anna：周五见：我订好了那家餐厅': 'Anna: See you Friday, I booked the restaurant',
    'v0.4 发布': 'v0.4 release',
    '还有 3 天': '3 days left',
    '10 月 20 日 周二': 'Tue, Oct 20',
    天: 'days',
    部署: 'Deploy',
    成功: 'Succeeded',
    苹果: 'Apple',
    专注: 'Focus',
  }
  // And the keyboard English (US), no input method: her taskbar shows ENG.
  const inEnglish = w => {
    const words = JSON.parse(JSON.stringify(w), (k, v) => (typeof v === 'string' && Object.hasOwn(EN, v) ? EN[v] : v))
    return { ...words, lang: 'en', ...(words.keys ? { keys: { ...words.keys, lang: 0x0409, ime: null } } : {}) }
  }
  const README_CARDS = [
    ['sessions', '会话', { scene: 'three-busy', page: 'island', view: [0, above(243), 432, 243], steps: openIsland }],
    ['ask', '批准', { scene: 'ask', page: 'island', view: [0, above(292), 520, 292], settle: 900 }],
    ['ring', '流光', { scene: 'hold-ring', page: 'island', view: [0, above(180), 320, 180], budget: 8000, steps: p => p.wait(4500) }],
    ['island', '灵动岛', { scene: 'island-home', page: 'island', view: [580, 0, 440, 248], steps: openIsland }],
    // A strip across both columns: Start to the clock, widgets on it (as
    // many as leave the clock its room).
    ['taskbar', '任务栏', { scene: 'taskbar-full', page: 'island', view: [620, above(64), 980, 64], world: F => ({ taskbarLayout: { ...F.TASKBAR.widgets, right: F.TASKBAR.widgets.right.filter(p => p.type !== 'stack') } }) }],
    ['desktop', '桌面小工具', { scene: 'desktop', page: 'screen', view: [436, 20, 704, 396], budget: 4000 }],
    // Top bar + dock (0.4.0): the whole screen, the top bar and the dock as
    // they come, a session at work in her capsule, the desktop clear.
    ['split', '顶栏 + Dock', { page: 'screen', home: 'taskbar', split: true, view: WHOLE, budget: 4000, world: F => ({ ...home(F), edge: 'split', sessions: [F.session('working'), F.others()[0]], deskLayout: [], config: { splitBar: 32, splitDock: 'hide', splitKeys: 'top' } }) }],
  ]
  const readmeShots = README_CARDS.flatMap(([id, title, shot]) =>
    ['zh', 'en'].map(lang => ({ id: `readme-${id}-${lang}`, section: 'readme', title: `${title}（${lang === 'zh' ? '中文' : 'English'}）`, lang, ...shot })),
  )

  const LIST = [
    // --- The scenes, each as it is best seen ------------------------------------------------
    { id: 'scene-first-run', section: 'scenes', scene: 'first-run', title: '新用户第一次打开 · 设置里的欢迎卡片', page: 'island', view: [0, above(660), 560, 660], settle: 700 },
    { id: 'scene-first-run-hint', section: 'scenes', scene: 'first-run', play: false, title: '新用户第一次打开 · 胶囊上提示去连接', page: 'island', view: CLOSED },
    { id: 'scene-everyday', section: 'scenes', scene: 'everyday', title: '平常 · 整个屏幕', page: 'screen', view: WHOLE, budget: 4000 },
    { id: 'scene-three-busy', section: 'scenes', scene: 'three-busy', title: '三个会话在忙 · 展开', page: 'island', view: [0, above(420), 520, 420], steps: openIsland },
    { id: 'scene-ask', section: 'scenes', scene: 'ask', title: '等你批准 · 询问卡片', page: 'island', view: [0, above(230), 520, 230], settle: 900 },
    {
      id: 'scene-done-flow',
      section: 'scenes',
      scene: 'done-flow',
      title: '做完了 · 流光转到一半',
      page: 'island',
      view: CLOSED,
      steps: async p => {
        await p.wait(50)
        p.freeze({ flow: 1100 })
      },
    },
    // Past its hold (8 s from the event, the default) and its fade.
    { id: 'scene-error', section: 'scenes', scene: 'error', title: '出错了 · 流光播完留下的红点', page: 'island', view: CLOSED, budget: 12000, steps: p => p.wait(9200) },
    // Past the turns (2 s), held lit and still (hold 30 s).
    { id: 'scene-hold-ring', section: 'scenes', scene: 'hold-ring', title: '提醒停留 · 流光转完，光环静止亮着（停 30 秒）', page: 'island', view: CLOSED, budget: 8000, steps: p => p.wait(4500) },
    // Past the few seconds it used to stay open (6.5 s with Claude's words), open still.
    { id: 'scene-hold-open', section: 'scenes', scene: 'hold-open', title: '展开停留 · 做完自己展开，7 秒时还开着（停 30 秒）', page: 'island', view: [0, above(420), 520, 420], budget: 10000, steps: p => p.wait(7000) },
    { id: 'scene-taskbar-full', section: 'scenes', scene: 'taskbar-full', title: '放满小工具的任务栏', page: 'island', view: STRIP_VIEW },
    { id: 'scene-desktop', section: 'scenes', scene: 'desktop', title: '桌面 · 单个、叠放、两个工作台', page: 'screen', view: WHOLE, budget: 4000 },
    { id: 'scene-desktop-empty', section: 'scenes', scene: 'desktop-empty', title: '空桌面', page: 'screen', view: WHOLE, budget: 4000 },
    { id: 'scene-calendar-media', section: 'scenes', scene: 'calendar-media', title: '日历和正在播放 · 桌面上各三档', page: 'screen', view: WHOLE, budget: 4000 },
    { id: 'scene-island-home', section: 'scenes', scene: 'island-home', title: '灵动岛模式 · 展开', page: 'island', view: [400, 0, 800, 420], steps: openIsland },
    { id: 'scene-island-right', section: 'scenes', scene: 'island-right', title: '岛在右上 · 展开，朝屏幕中间长', page: 'island', view: [800, 0, 800, 420], settle: 700 },
    { id: 'scene-island-bottom', section: 'scenes', scene: 'island-bottom', title: '岛在底部 · 展开，往上长', page: 'island', view: [400, above(420), 800, 420], settle: 700 },
    { id: 'scene-light-clear', section: 'scenes', scene: 'light-clear', title: '浅色 + 透明任务栏', page: 'island', view: STRIP_VIEW },
    { id: 'scene-edit-layout', section: 'scenes', scene: 'edit-layout', title: '编辑任务栏布局中', page: 'island', view: [0, above(200), 1600, 200], settle: 700 },
    { id: 'scene-add-widgets', section: 'scenes', scene: 'add-widgets', title: '添加小工具 · 编辑任务栏，右边是小工具库', page: 'screen', view: WHOLE, budget: 4000, settle: 700 },
    { id: 'scene-edit-capsule', section: 'scenes', scene: 'edit-capsule', title: '编辑胶囊 · 她的胶囊上方的面板', page: 'island', view: [0, above(440), 720, 440], settle: 700 },
    { id: 'scene-combos', section: 'scenes', scene: 'combos', title: '组合 · 小工具库里存好的工作台', page: 'screen', view: WHOLE, budget: 4000, settle: 700 },
    { id: 'scene-multi-select', section: 'scenes', scene: 'multi-select', title: 'Ctrl 多选 · 选中四个，右键的菜单', page: 'screen', view: WHOLE, budget: 4000 },

    // --- The README's cards, Chinese and English ---------------------------------------------
    ...readmeShots,

    // --- Nobody there: quiet, and nothing playing endlessly (boot.js fails the shot) --------
    { id: 'quiet-taskbar', section: 'quiet', scene: 'taskbar-full', play: false, title: '没人在 · 放满小工具的任务栏', page: 'island', view: STRIP_VIEW, world: () => ({ sessions: [], quiet: true }) },
    { id: 'quiet-island', section: 'quiet', scene: 'island-home', play: false, title: '没人在 · 灵动岛', page: 'island', view: [400, 0, 800, 120], world: () => ({ sessions: [], quiet: true }) },
    { id: 'quiet-desktop', section: 'quiet', scene: 'desktop', play: false, title: '没人在 · 桌面', page: 'screen', view: WHOLE, budget: 4000, world: () => ({ sessions: [], quiet: true }) },

    // --- The taskbar's strip ---------------------------------------------------------------
    ...[
      ['mica', 'Mica'],
      ['clear', '透明'],
      ['black', '纯色'],
    ].map(([material, name]) => ({
      id: `taskbar-${material}`,
      section: 'taskbar',
      title: `任务栏 · ${name}`,
      page: 'island',
      home: 'taskbar',
      view: STRIP_VIEW,
      world: F => ({ ...home(F), material, sessions: [F.session('working')] }),
    })),
    // Split (docs/design/specs/dock-topbar.md): the top bar and the dock at
    // the foot, 32 px and out of the way by default; 48 px with quick
    // settings and the clock in the dock. A screen shot: the desktop's page,
    // the top bar's and the dock's.
    ...[
      ['split', '顶栏 + dock · 默认（32 px）', {}],
      ['split-48-keys', '顶栏 + dock · 48 px，快速设置和时钟在 dock', { splitBar: 48, splitKeys: 'dock' }],
    ].map(([id, title, config]) => ({
      id: `taskbar-${id}`,
      section: 'taskbar',
      title,
      page: 'screen',
      home: 'taskbar',
      split: true,
      view: WHOLE,
      budget: 4000,
      world: F => ({ ...home(F), edge: 'split', sessions: [F.session('working'), F.others()[0]], deskLayout: [], config: { splitBar: 32, splitDock: 'hide', splitKeys: 'top', ...config } }),
    })),
    {
      id: 'taskbar-split-open',
      section: 'taskbar',
      title: '顶栏 + dock · 她在顶栏正中，往下展开',
      page: 'island',
      home: 'taskbar',
      split: true,
      view: [400, 0, 800, 300],
      steps: p => p.hover('#island'),
      world: F => ({ ...home(F), edge: 'split', sessions: [F.session('working'), F.others()[0]], config: { splitBar: 32, splitDock: 'hide', splitKeys: 'top' } }),
    },
    {
      id: 'taskbar-side-screen',
      section: 'taskbar',
      title: '副屏的任务栏 · 有询问在等时她在开头',
      page: 'bar',
      view: STRIP_VIEW,
      world: F => ({ ...home(F), material: 'mica', asks: [F.ASK] }),
    },

    // --- Her capsule on the taskbar, and her island at the top ------------------------------
    ...states('capsule', 'taskbar', {
      closed: CLOSED,
      ask: [0, above(230), 520, 230],
      open: [0, above(420), 520, 420],
      card: [0, above(660), 520, 660],
    }),
    ...states('island', 'island', {
      closed: [480, 0, 640, 60],
      ask: [500, 0, 600, 210],
      open: [520, 0, 560, 400],
      card: [520, 0, 560, 600],
    }),
    // DeepSeek Harness waiting on you: its approval (why it asks, under the
    // buttons; handled in DSH as the way out) and its question, its own
    // dialog open all the while.
    {
      id: 'capsule-ask-dsh',
      section: 'capsule',
      title: '等你 · DeepSeek Harness 要批准（下面是它说的原因）',
      page: 'island',
      home: 'taskbar',
      view: [0, above(330), 520, 330],
      world: F => ({ ...home(F), sessions: [F.dsh({ key: 'detail.approve', vars: { what: { key: 'step.write', vars: { file: 'note.txt' } } } })], asks: [F.DSH_ASK] }),
      settle: 900,
    },
    {
      id: 'capsule-question-dsh',
      section: 'capsule',
      title: '等你 · DeepSeek Harness 问你问题',
      page: 'island',
      home: 'taskbar',
      view: [0, above(360), 520, 360],
      world: F => ({ ...home(F), sessions: [F.dsh({ key: 'detail.asksQuestion' })], asks: [F.DSH_QUESTION] }),
      settle: 900,
    },
    // The calendar and what is playing in her capsule (plugins/calendar.rs,
    // media.rs): their rows (the date with the next event under it; the song,
    // a ring of how far along), and the song's card unfolded with its buttons.
    ...[
      ['capsule-calendar-media', '日历和正在播放 · 展开的胶囊里各一行', [0, above(330), 520, 330], openIsland],
      [
        'capsule-media-card',
        '正在播放 · 点 › 展开的卡片：进度、程序、上一首 / 暂停 / 下一首',
        [0, above(460), 520, 460],
        async p => {
          await openIsland(p)
          p.click('#island-expanded [data-widget="media"]')
          await p.wait(900)
        },
      ],
    ].map(([id, title, view, steps]) => ({
      id,
      section: 'capsule',
      title,
      page: 'island',
      home: 'taskbar',
      view,
      settle: 900,
      world: F => ({ ...home(F), widgets: [...F.widgets('today', 'tokens'), F.SLEEPING.calendar, F.SLEEPING.media], plugins: F.PLUGINS.map(p => (p.id === 'calendar' || p.id === 'media' ? { ...p, on: true, state: 'running' } : p)), sessions: [F.session('working')] }),
      steps,
    })),
    // At a top corner (islandPos, zhoupengjie/Wakuwaku#59): closed 8 px in
    // from the screen's side, and open, grown toward the middle.
    ...[
      ['left', '左上', [0, 0, 640, 60], [0, 0, 560, 400]],
      ['right', '右上', [960, 0, 640, 60], [1040, 0, 560, 400]],
    ].flatMap(([pos, name, closed, open]) => [
      { id: `island-${pos}`, section: 'island', title: `${name} · 收起 · 干活中`, page: 'island', home: 'island', islandPos: pos, view: closed, world: F => ({ ...inMood('working')(F), home: 'island' }) },
      { id: `island-${pos}-open`, section: 'island', title: `${name} · 展开，朝屏幕中间长`, page: 'island', home: 'island', islandPos: pos, view: open, world: F => ({ ...busy(F), home: 'island' }), steps: openIsland },
    ]),
    // Along the foot of the screen (islandEdge, zhoupengjie/Wakuwaku#105):
    // closed, centred in Windows' taskbar's strip (48 px, not drawn), and
    // open, grown upward from there.
    { id: 'island-bottom', section: 'island', title: '底部 · 收起 · 干活中，嵌在 Windows 任务栏的那一条上', page: 'island', home: 'island', islandEdge: 'bottom', view: [480, above(60), 640, 60], world: F => ({ ...inMood('working')(F), home: 'island' }) },
    { id: 'island-bottom-open', section: 'island', title: '底部 · 展开，往上长', page: 'island', home: 'island', islandEdge: 'bottom', view: [520, above(400), 560, 400], world: F => ({ ...busy(F), home: 'island' }), steps: openIsland },
    // Five pinned at the capsule's end (the most there may be), the monitor
    // among them, a session at work: in each home.
    ...[
      ['capsule', 'taskbar', [0, above(88), 1600, 88], '任务栏上的胶囊'],
      ['island', 'island', [240, 0, 1120, 60], '顶部的灵动岛'],
    ].map(([section, home, view, name]) => ({
      id: `${section}-pins5`,
      section,
      title: `收起 · 干活中，固定了五个（含系统监控）· ${name}`,
      page: 'island',
      home,
      view,
      world: F => ({ ...inMood('working')(F), home, widgets: F.widgets('monitor', 'weather', 'today', 'tokens', 'stock-aapl', 'stock-nvda', 'pomodoro', 'countdown'), taskbarLayout: NO_MONITOR_PART, config: { capsule: PINS5 } }),
    })),
    // The strip crowded (her capsule wide, two widgets after it): the
    // leftmost pins out, a +k before the rest.
    {
      id: 'capsule-pins5-tight',
      section: 'capsule',
      title: '收起 · 固定了五个，任务栏挤：左边的收成 +k',
      page: 'island',
      home: 'taskbar',
      view: [0, above(88), 1600, 88],
      world: F => ({
        ...inMood('working')(F),
        widgets: F.widgets('monitor', 'weather', 'today', 'tokens', 'stock-aapl', 'stock-nvda', 'pomodoro', 'countdown', 'deploy'),
        taskbarLayout: { ...NO_MONITOR_PART, left: ['her', { id: 'wtk', type: 'widget', of: 'deploy', size: 'M' }, { id: 'wtt', type: 'widget', of: 'tokens', size: 'S' }] },
        config: { capsule: PINS5, islandWidth: 'wide' },
      }),
    },

    // --- The flow light ----------------------------------------------------------------------
    // A turn done: the ring paused part of the way round, then the dot it leaves.
    {
      id: 'flow-ring',
      section: 'flow',
      title: '流光 · 光环转到一半',
      page: 'island',
      home: 'taskbar',
      view: CLOSED,
      world: F => ({ ...home(F), sessions: [F.session('working')] }),
      steps: async p => {
        await p.wait(300)
        p.set({ sessions: [p.F.session('done')] })
        p.push()
        await p.wait(50)
        p.freeze({ flow: 1100 })
      },
    },
    {
      id: 'flow-dot',
      section: 'flow',
      title: '流光 · 播完留下的小点',
      page: 'island',
      home: 'taskbar',
      view: CLOSED,
      budget: 12000,
      world: F => ({ ...home(F), sessions: [F.session('working')] }),
      // Past its hold (8 s from the event, the default) and its fade.
      steps: async p => {
        await p.wait(300)
        p.set({ sessions: [p.F.session('done')] })
        p.push()
        await p.wait(9200)
      },
    },
    {
      id: 'flow-ring-island',
      section: 'flow',
      title: '流光 · 顶部灵动岛上',
      page: 'island',
      home: 'island',
      view: [480, 0, 640, 60],
      world: F => ({ ...home(F), home: 'island', sessions: [F.session('working')] }),
      steps: async p => {
        await p.wait(300)
        p.set({ sessions: [p.F.session('done')] })
        p.push()
        await p.wait(50)
        p.freeze({ flow: 1100 })
      },
    },
    // Held (停留多久 30 s, zhoupengjie/Wakuwaku#65): its turns over, lit and
    // still, nothing of it animated; on the taskbar, at the top, at a corner.
    ...[
      ['flow-held', '任务栏上', 'taskbar', 'center', CLOSED],
      ['flow-held-island', '顶部灵动岛上', 'island', 'center', [480, 0, 640, 60]],
      ['flow-held-right', '右上角的灵动岛上', 'island', 'right', [960, 0, 640, 60]],
    ].map(([id, name, home_, islandPos, view]) => ({
      id,
      section: 'flow',
      title: `停留 · 转完后光环静止亮着 · ${name}`,
      page: 'island',
      home: home_,
      islandPos,
      view,
      budget: 8000,
      world: F => ({ ...home(F), home: home_, sessions: [F.session('working')], config: { hold: 30 } }),
      steps: async p => {
        await p.wait(300)
        p.set({ sessions: [p.F.session('done')] })
        p.push()
        await p.wait(4000)
      },
    })),

    // --- Menus -------------------------------------------------------------------------------
    // Her own right-click menu is Windows' (tray.rs): not a page's, not here.
    {
      id: 'menu-capsule',
      section: 'menus',
      title: '胶囊里小工具的右键菜单',
      page: 'island',
      home: 'taskbar',
      view: [0, above(460), 720, 460],
      world: busy,
      steps: openIsleMenu,
    },
    {
      id: 'menu-taskbar',
      section: 'menus',
      title: '任务栏上小工具的右键菜单',
      page: 'island',
      home: 'taskbar',
      view: [880, above(420), 720, 420],
      world: F => ({ ...home(F), taskbarLayout: F.TASKBAR.widgets, widgets: F.widgets('monitor', 'weather', 'today', 'inbox-a1', 'stock-aapl', 'stock-nvda', 'countdown'), sessions: [F.session('working')] }),
      steps: async p => {
        await p.wait(200)
        p.context('[data-entry="wtw"]', 12, 12)
        await p.wait(300)
      },
    },
    {
      id: 'menu-app',
      section: 'menus',
      title: '程序按钮的右键菜单',
      page: 'island',
      home: 'taskbar',
      view: [480, above(360), 640, 360],
      world: F => ({ ...home(F), sessions: [F.session('working')] }),
      steps: async p => {
        await p.wait(200)
        p.context('#bar-rest [data-app="code"]', 20, 20)
        await p.wait(400)
      },
    },

    // --- The settings ------------------------------------------------------------------------
    ...[
      ['now', 'general', '通用'],
      ['widgets', 'plugins', '插件'],
    ].map(([tab, id, name]) => ({
      id: `settings-${id}`,
      section: 'settings',
      title: `设置 · ${name}`,
      page: 'island',
      home: 'taskbar',
      view: [0, above(660), 560, 660],
      // The everyday desktop: the Plugins page says where each is (the
      // capsule, the taskbar, the desktop, the board 工作), or 没放.
      world: F => ({ ...busy(F), widgets: F.widgets('monitor', 'today', 'tokens', 'weather', 'stock-aapl', 'stock-nvda', 'pomodoro', 'countdown', 'inbox-a1', 'deploy'), deskLayout: F.DESK.everyday }),
      steps: async p => {
        p.emit('island:settings', { tab })
        await p.wait(600)
      },
    })),
    // Split (docs/design/specs/dock-topbar.md §1): the 外观 page under the
    // top bar, its position 顶栏 + dock and the three rows that come with it.
    {
      id: 'settings-look-split',
      section: 'settings',
      title: '设置 · 外观，任务栏位置「顶栏 + dock」（在她下面，以她为中）',
      page: 'island',
      home: 'taskbar',
      split: true,
      view: [400, 0, 800, 720],
      world: F => ({ ...busy(F), edge: 'split', config: { splitBar: 32, splitDock: 'hide', splitKeys: 'top' } }),
      steps: async p => {
        p.emit('island:settings', { tab: 'look' })
        await p.wait(600)
      },
    },
    {
      id: 'settings-general-open',
      section: 'settings',
      title: '设置 · 通用，会话展开（改过的文件、最近几步）',
      page: 'island',
      home: 'taskbar',
      view: [0, above(660), 560, 660],
      world: F => ({ ...busy(F), sessions: [F.session('review'), ...F.others()], widgets: F.widgets('monitor', 'today', 'tokens', 'weather', 'stock-aapl', 'stock-nvda', 'pomodoro', 'countdown', 'inbox-a1', 'deploy'), deskLayout: F.DESK.everyday }),
      steps: async p => {
        p.emit('island:settings', { tab: 'now' })
        await p.wait(600)
        p.click('#island-settings [data-sess-open="s1"]')
        p.click('#island-settings [data-sess-open="s2"]')
        await p.wait(300)
      },
    },
    {
      id: 'settings-general-nodetails',
      section: 'settings',
      title: '设置 · 通用，关掉具体内容（对齐计时）',
      page: 'island',
      home: 'taskbar',
      view: [0, above(660), 560, 660],
      world: F => ({
        ...busy(F),
        config: { details: false },
        sessions: [F.session('review'), ...F.others()],
        widgets: F.widgets('monitor', 'today', 'tokens', 'weather', 'stock-aapl', 'stock-nvda', 'pomodoro', 'countdown', 'inbox-a1', 'deploy'),
        deskLayout: F.DESK.everyday,
      }),
      steps: async p => {
        p.emit('island:settings', { tab: 'now' })
        await p.wait(600)
      },
    },
    // The Agent page: the agents found here (fake-tauri.js: Claude Code,
    // Codex and Reasonix, the others not on this computer), then Reasonix's
    // row opened to its details, and a Reasonix 0.x's, which she does not
    // connect to (zhoupengjie/Wakuwaku#60; a page with no such row is shot
    // as it is).
    ...[
      ['settings-agents', '设置 · Agent', null, null],
      ['settings-agents-reasonix', '设置 · Agent，Reasonix 那一行展开', 'reasonix', null],
      ['settings-agents-reasonix-0x', '设置 · Agent，Reasonix 0.x（不支持）展开', 'reasonix', { hooks: 'unsupported', line: '0.x', version: '0.52.0', seen: false, backup: false }],
    ].map(([id, title, open, reasonix]) => ({
      id,
      section: 'settings',
      title,
      page: 'island',
      home: 'taskbar',
      view: [0, above(660), 560, 660],
      world: F => ({ ...busy(F), ...(reasonix ? { reasonix } : {}) }),
      steps: async p => {
        p.emit('island:settings', { tab: 'agents' })
        await p.wait(600)
        const row = open && p.$(`#island-settings [data-ag-open="${open}"]`)
        if (row) {
          p.click(`#island-settings .ag-row[data-ag-open="${open}"]`)
          await p.wait(300)
        }
      },
    })),
    {
      id: 'settings-plugins-unplaced',
      section: 'settings',
      title: '设置 · 插件，往下：没放的（添加小工具…）',
      page: 'island',
      home: 'taskbar',
      view: [0, above(660), 560, 660],
      world: F => ({ ...busy(F), widgets: F.widgets('monitor', 'today', 'tokens', 'weather', 'stock-aapl', 'stock-nvda', 'pomodoro', 'countdown', 'inbox-a1', 'deploy'), deskLayout: F.DESK.everyday }),
      steps: async p => {
        p.emit('island:settings', { tab: 'widgets:countdown' })
        // Past the settings' last scroll to the part (1.6 s), as it stays.
        await p.wait(1800)
      },
    },
    // Other people's plugins on the Plugins page (docs/design/specs/plugins.md
    // §3): their rows among hers, 从文件安装… under 显示; a plugin's own page
    // (from the market; from a file, not reviewed); installing from a file
    // (asked, with what it will do; an update; a program inside; done: the
    // word, its row edged); 卸载 asked first.
    addonShot('settings-plugins-addons', '别人写的：运行中、未经审核没放、无响应已停用，没装上的文件夹', p => openPluginsPage(p, 'widgets:rain-alert')),
    addonShot('settings-plugins-addon-detail', '下雨提醒的详情（市场的）：它能做的、卸载', async p => {
      await openPluginsPage(p)
      p.click('#island-settings [data-addon="rain-alert"]')
      await p.wait(400)
    }),
    addonShot('settings-plugins-addon-detail-file', '数数的详情（从文件装的，未经审核）', async p => {
      await openPluginsPage(p)
      p.click('#island-settings [data-addon="tally"]')
      await p.wait(400)
    }),
    addonShot('settings-plugins-addon-approval', '要更多权限，先问（允许…）', async p => {
      await openPluginsPage(p)
      p.click('#island-settings [data-approve="note-pad"]')
      await p.wait(400)
    }),
    addonShot('settings-plugins-uninstall-confirm', '卸载先问一句', async p => {
      await openPluginsPage(p)
      p.click('#island-settings [data-addon="tally"]')
      await p.wait(300)
      p.click('#island-settings [data-uninstall]')
      await p.wait(400)
    }),
    addonShot('settings-plugins-install-confirm', '从文件安装：要权限的，先问', installFromFile, { addonPick: 'agent-board.zip' }),
    addonShot('settings-plugins-install-update', '从文件安装：同一个插件的新版，替换旧版', installFromFile, { addonPick: 'tally-1.1.0.zip' }),
    addonShot('settings-plugins-install-refused', '从文件安装：包里有程序，装不了', installFromFile, { addonPick: 'cool-tool.zip' }),
    // A plugin with a view of its own (Agent 排行): its page says 自绘界面;
    // too busy for the watchdog, stopped, its row says so (spec §3, §8).
    {
      ...addonShot('settings-plugins-addon-busy', 'Agent 排行（自绘界面）太忙，已停用', p => openPluginsPage(p, 'widgets:agent-board')),
      world: F => ({ ...withAddons(F), addons: [...F.ADDONS, { ...F.ADDON_BOARD, state: 'stopped', stopped: { why: 'busy', since: F.T0 - 20 * 60e3, at: F.T0 - 20 * 60e3, times: 0 } }] }),
    },
    {
      ...addonShot('settings-plugins-addon-detail-view', 'Agent 排行的详情：类别后面标「自绘界面」', async p => {
        await openPluginsPage(p)
        p.click('#island-settings [data-addon="agent-board"]')
        await p.wait(400)
      }),
      world: F => ({ ...withAddons(F), addons: [...F.ADDONS, F.ADDON_BOARD] }),
    },
    addonShot(
      'settings-plugins-installed',
      '从文件安装：装好了，提示和标出来的那一行',
      async p => {
        await installFromFile(p)
        p.click('#island-settings [data-sheet="go"]')
        // Past the scroll to its row (1.6 s).
        await p.wait(1900)
      },
      { addonPick: 'my-plugin.zip' },
      { budget: 5000 },
    ),
    // The plugin market (docs/design/specs/plugins.md §1, §2, §3, §5;
    // fixtures.js MARKET, fake-tauri.js market): its list, by category; one
    // one's page drawing its cards from what it says it shows (a tier-2
    // one's note); installing (one asking for more first asked; one asking
    // for the web alone at once, its download under way); not reached, not
    // open yet; updates waiting under 获取更多, and 全部更新 asked first.
    addonShot('settings-market', '获取更多：市场的列表（已装的写已安装）', p => openMarket(p)),
    addonShot('settings-market-pets', '获取更多：只看宠物（外观页的直达入口）', p => openMarket(p, 'widgets:market:pet')),
    addonShot('settings-market-detail', '市场里一个小工具的详情：用她的卡片画出小档和中档', p => openMarketItem(p, 'fx')),
    addonShot('settings-market-detail-view', '自绘界面的 Agent 排行：小、中、大档，和一句说明', p => openMarketItem(p, 'agent-board'), {}, { budget: 4000 }),
    addonShot('settings-market-detail-pack', '宠物包的详情：它的预览图，只有图片没有代码', p => openMarketItem(p, 'pet-shiba')),
    addonShot('settings-market-confirm', '市场里安装要权限的：先问一句', async p => {
      await openMarket(p)
      p.click('#island-settings [data-mk-do="install"][data-id="gh-notify"]')
      await p.wait(300)
    }),
    addonShot(
      'settings-market-installing',
      '市场里安装只联网的：直接下载，按钮成了进度',
      async p => {
        await openMarket(p)
        p.click('#island-settings [data-mk-do="install"][data-id="fx"]')
        await p.wait(400)
      },
      { marketHold: 45 },
    ),
    addonShot('settings-market-error', '市场连不上：哪条路、为什么，去网络代理', p => openMarket(p), { marketMode: 'error' }),
    addonShot('settings-market-closed', '市场还没开张（仓库没建，404）', p => openMarket(p), { marketMode: 'closed' }),
    {
      ...addonShot('settings-plugins-updates', '插件页最上面：获取更多，和「2 个插件有更新 · 全部更新」（关了自动更新）', p => openPluginsPage(p)),
      world: F => ({ ...withAddons(F), marketUpdates: F.MARKET_UPDATES, config: { pluginAutoUpdate: false } }),
    },
    {
      ...addonShot('settings-plugins-updates-confirm', '全部更新：新版要更多权限的，先列出来问', async p => {
        await openPluginsPage(p)
        p.click('#island-settings [data-mk-all]')
        await p.wait(400)
      }),
      world: F => ({ ...withAddons(F), marketUpdates: F.MARKET_UPDATES, config: { pluginAutoUpdate: false } }),
    },
    // Packs (docs/design/specs/plugins.md §1, §3; packs.rs; fixtures.js
    // PACKS): the Plugins page's 资源包 (像素猫 and 极光 from the market, 海边
    // from a file); her pets with 像素猫 among them and 获取更多宠物…; the
    // wallpaper's rows (极光 in use: Windows' slideshow on it), 获取更多壁纸…;
    // a pack's page, 卸载 asked first; a pet pack from a file, asked.
    packShot('settings-plugins-packs', '资源包：宠物和壁纸，从文件装的标未经审核，右边「在「外观」里选」', p => openPluginsPage(p, 'widgets:packs')),
    packShot('settings-look-pets-packs', '外观 · 她的宠物：资源包的像素猫写「来自市场」，下面「获取更多宠物…」', p => openPluginsPage(p, 'pets')),
    packShot('settings-look-wallpaper-packs', '外观 · 壁纸：两个壁纸包，在用的写「在用」，另一个「用这一包」，然后「获取更多壁纸…」', p => openPluginsPage(p, 'paper')),
    packShot('settings-pack-detail', '像素猫的详情：预览图、介绍、只有图片没有代码、卸载', async p => {
      await openPluginsPage(p, 'widgets:packs')
      p.click('#island-settings [data-pack="pet:pet-pixelcat"]')
      await p.wait(400)
    }),
    packShot('settings-pack-uninstall-confirm', '卸载资源包先问一句：正用着的宠物换回默认的', async p => {
      await openPluginsPage(p, 'widgets:packs')
      p.click('#island-settings [data-pack="pet:pet-pixelcat"]')
      await p.wait(300)
      p.click('#island-settings [data-pack-uninstall]')
      await p.wait(400)
    }),
    packShot('settings-pack-install-confirm', '从文件安装宠物包：未经审核，只有图片没有代码', installFromFile, { addonPick: 'mochi.zip' }),

    // General's 提醒: 她有事告诉你时, and just under it 停留多久, its note for
    // the cue chosen (zhoupengjie/Wakuwaku#59, #65): a ring, Custom's 45 s
    // in its box; the dot alone, greyed.
    ...[
      ['settings-alerts', '流光，自定义 45 秒', { endCue: 'ring', hold: 45 }],
      ['settings-alerts-none', '只留小点时变灰', { endCue: 'none', hold: 30 }],
    ].map(([id, name, config]) => ({
      id,
      section: 'settings',
      title: `设置 · 通用 · 提醒：${name}`,
      page: 'island',
      home: 'taskbar',
      view: [0, above(660), 560, 660],
      world: F => ({ ...busy(F), config }),
      steps: async p => {
        p.emit('island:settings', { tab: 'alerts' })
        // Past the settings' last scroll to the part (1.6 s), as it stays.
        await p.wait(1800)
      },
    })),
    // General's 网络代理 (zhoupengjie/Wakuwaku#68): Windows' proxy (Clash's,
    // 127.0.0.1:7890) and what the Test found; one typed in (SOCKS5); none
    // set in Windows and an update not downloaded, its line pointing there.
    ...[
      ['settings-net', '跟随系统，测试过', {}, null, true],
      ['settings-net-manual', '手动 SOCKS5', { netProxy: { mode: 'manual', kind: 'socks5', address: '127.0.0.1:10808' } }, null, false],
      [
        'settings-net-update',
        '更新下载失败，指到网络代理',
        {},
        {
          latest: { version: '0.3.4', newer: true, canInstall: true },
          error: { code: 'download', detail: 'GitHub: straight, github.com → release-assets.githubusercontent.com: timeout: connect' },
        },
        false,
      ],
    ].map(([id, name, config, update, test]) => ({
      id,
      section: 'settings',
      title: `设置 · 通用 · 网络代理：${name}`,
      page: 'island',
      home: 'taskbar',
      view: [0, above(660), 560, 660],
      world: F => ({ ...busy(F), config, ...(update ? { update, netSystemProxy: null } : {}) }),
      steps: async p => {
        p.emit('island:settings', { tab: update ? 'update' : 'net' })
        // Past the settings' last scroll to the part (1.6 s), as it stays.
        await p.wait(1800)
        if (test) {
          p.click('#island-settings [data-net-test]')
          await p.wait(600)
        }
      },
    })),
    // Look's 位置 (zhoupengjie/Wakuwaku#59): at the top of the screen, three
    // places (at the left here, as the island is); on the taskbar, a line
    // and the way to drag her capsule.
    {
      id: 'settings-look-island',
      section: 'settings',
      title: '设置 · 外观 · 灵动岛的位置（左上）',
      page: 'island',
      home: 'island',
      islandPos: 'left',
      view: [0, 0, 560, 720],
      world: F => ({ ...busy(F), home: 'island' }),
      steps: async p => {
        p.emit('island:settings', { tab: 'place' })
        await p.wait(1800)
      },
    },
    {
      id: 'settings-look-taskbar',
      section: 'settings',
      title: '设置 · 外观 · 任务栏模式下的「位置」：拖动她的胶囊',
      page: 'island',
      home: 'taskbar',
      view: [0, above(660), 560, 660],
      world: busy,
      steps: async p => {
        p.emit('island:settings', { tab: 'place' })
        await p.wait(1800)
      },
    },
    // Two displays (zhoupengjie/Wakuwaku#104): the 屏幕 row with a segment a
    // display, hers the second one (not Windows' main), and 显示在 below it.
    {
      id: 'settings-look-displays',
      section: 'settings',
      title: '设置 · 外观 · 两块屏：她的家在 2 号屏',
      page: 'island',
      home: 'taskbar',
      view: [0, above(660), 560, 660],
      world: F => ({
        ...busy(F),
        displays: [
          { key: 'd000000000001', n: 1, w: 2560, h: 1440, main: true, hers: false },
          { key: 'd000000000002', n: 2, w: 1920, h: 1080, main: false, hers: true },
        ],
        config: { homeDisplay: 'd000000000002' },
      }),
      steps: async p => {
        p.emit('island:settings', { tab: 'look' })
        await p.wait(600)
      },
    },
    // Mail, as a user with two accounts has it (fixtures.js MAIL): in two
    // panes (the screen has the room), every account's inbox as one on the
    // left, the accounts (unread, kept offline, through a proxy) and who
    // letters go to on the right.
    {
      id: 'settings-mail',
      section: 'settings',
      title: '设置 · 邮件：两个账号和收件箱',
      page: 'island',
      home: 'taskbar',
      view: [0, above(680), 1000, 680],
      world: busy,
      steps: async p => {
        p.emit('island:settings', { tab: 'mail' })
        // The inbox comes in once the page is drawn (mail_letters).
        await p.wait(1000)
      },
    },
    // The Agent page, DeepSeek Harness's row open: its two ways (her plugin,
    // loaded; DSH's own hook bridge, its packages not installed yet), the
    // way picker on top (zhoupengjie/Wakuwaku#61).
    ...[
      ['plugin', 'Wakuwaku 插件，已加载', F => ({ hooks: 'ok', way: 'plugin', via: null, why: '', seen: true, load: { state: 'loaded', at: F.T0 - 600e3, version: 1, profile: 'desktop', pid: 4321 }, listShown: '~\\.dsh\\wakuwaku\\wakuwaku.js' })],
      ['bridge', '官方钩子桥，包没装上', F => ({ hooks: 'stale', way: 'bridge', via: 'bridge', why: 'packages', seen: false, listShown: '~\\.dsh\\wakuwaku\\wakuwaku.js', hooksShown: '~\\AppData\\Roaming\\wakuwaku\\dsh-hooks.json' })],
    ].map(([way, name, dsh]) => ({
      id: `settings-agents-dsh-${way}`,
      section: 'settings',
      title: `设置 · Agent · DeepSeek Harness：${name}`,
      page: 'island',
      home: 'taskbar',
      view: [0, above(660), 560, 660],
      world: F => ({ ...busy(F), dsh: { file: 'C:\\Users\\you\\.dsh\\cordis.patch.yml', fileShown: '~\\.dsh\\cordis.patch.yml', backup: false, dshVersion: '0.2.1-rc.2', ...dsh(F) } }),
      steps: async p => {
        p.emit('island:settings', { tab: 'agents' })
        await p.wait(600)
        p.click('#island-settings [data-ag-open="dsh"] .ag-name')
        await p.wait(400)
        p.$('#island-settings .ag-row[data-ag-open="dsh"]')?.scrollIntoView({ block: 'start' })
        await p.wait(200)
      },
    })),

    // --- Controls, from the settings ---------------------------------------------------------
    {
      id: 'controls',
      section: 'controls',
      title: '开关、分段、按钮、输入框、下拉',
      page: 'island',
      home: 'taskbar',
      view: [0, above(660), 560, 660],
      world: busy,
      steps: async p => {
        p.emit('island:settings', { tab: 'now' })
        await p.wait(600)
        p.specimen()
        await p.wait(100)
        p.click('#island-settings .s-body [data-mail-agent]')
        await p.wait(200)
      },
    },

    // --- Widgets -----------------------------------------------------------------------------
    {
      id: 'widgets-sizes',
      section: 'widgets',
      title: '桌面上的小 / 中 / 大：天气、系统监控、Claude 今天、脚本小工具（字母标记）',
      page: 'desk',
      view: [0, 0, 1380, 740],
      budget: 4000,
      world: F => ({ widgets: F.WIDGETS, deskLayout: F.DESK.sizes }),
    },
    {
      id: 'widgets-taskbar',
      section: 'widgets',
      title: '任务栏上：一个中档、一个轮播、一个叠放',
      page: 'island',
      home: 'taskbar',
      view: [760, above(88), 840, 88],
      world: F => ({ ...home(F), taskbarLayout: F.TASKBAR.widgets, widgets: F.widgets('monitor', 'weather', 'today', 'inbox-a1', 'stock-aapl', 'stock-nvda', 'countdown'), sessions: [F.session('working')] }),
    },
    {
      id: 'widgets-stack-open',
      section: 'widgets',
      title: '任务栏上的叠放点开',
      page: 'island',
      home: 'taskbar',
      view: [880, above(400), 720, 400],
      world: F => ({ ...home(F), taskbarLayout: F.TASKBAR.widgets, widgets: F.widgets('monitor', 'weather', 'today', 'inbox-a1', 'stock-aapl', 'stock-nvda', 'countdown'), sessions: [F.session('working')] }),
      steps: async p => {
        await p.wait(200)
        p.click('.tw-stack')
        await p.wait(500)
      },
    },
    // Other people's plugins where widgets go, drawn by her cards from what
    // they say (spec §4): in the picker (数数 just installed: first, 新), on
    // the taskbar (medium and small, by her icon, their own picture and a
    // letter), in the open island, on the desktop.
    {
      id: 'add-widgets-addon',
      section: 'widgets',
      title: '小工具库：别人写的插件在各自的类别里，刚装的「数数」排最前、标「新」',
      page: 'screen',
      view: WHOLE,
      budget: 4000,
      settle: 700,
      world: F => ({ ...withAddons(F), sessions: [F.session('working')], taskbarLayout: { left: ['her'], center: ['start', 'apps'], right: [{ id: 'wtw', type: 'plugin', of: 'weather', size: 'M', icon: 'weather' }, 'monitor', 'tray', 'keys', 'quick', 'clock'] } }),
      steps: async p => {
        // The page's addonui (a module the page imports; on window before).
        const ui = window.AddonUI || (await import(new URL('addonui.js', window.Fake.PAGES).href))
        ui.news.add('tally')
        await p.wait(200)
        p.emit('taskbar:edit', { picker: true })
        await p.wait(600)
      },
    },
    {
      id: 'widgets-addons-taskbar',
      section: 'widgets',
      title: '任务栏上别人写的插件：下雨提醒（中，她的图标）、数数（小，它自己的图）、天气雷达（中，首字）',
      page: 'island',
      home: 'taskbar',
      view: [760, above(88), 840, 88],
      world: F => ({
        ...home(F),
        addons: addonsRunning(F),
        sessions: [F.session('working')],
        taskbarLayout: {
          left: ['her'],
          center: ['start', 'apps'],
          right: [
            { id: 'wra', type: 'plugin', of: 'rain-alert', size: 'M', name: '下雨提醒', icon: 'weather' },
            { id: 'wrh', type: 'plugin', of: 'tally', size: 'S', name: '数数' },
            { id: 'wrr', type: 'plugin', of: 'weather-radar', size: 'M', name: '天气雷达' },
            'tray',
            'keys',
            'quick',
            'clock',
          ],
        },
      }),
    },
    {
      id: 'widgets-addons-capsule',
      section: 'widgets',
      title: '展开的胶囊里别人写的插件，一行一个',
      page: 'island',
      home: 'taskbar',
      view: [0, above(420), 520, 420],
      world: F => ({ ...home(F), widgets: F.widgets('weather'), addons: addonsRunning(F), sessions: [F.session('working')] }),
      steps: openIsland,
    },
    {
      id: 'widgets-addons-desk',
      section: 'widgets',
      title: '桌面上别人写的插件：中和小，她的图标、它自己的图、首字',
      page: 'desk',
      view: [0, 0, 1100, 420],
      budget: 4000,
      world: F => ({
        widgets: [],
        addons: addonsRunning(F),
        deskLayout: [
          { id: 'wa1', type: 'plugin', of: 'rain-alert', size: 'm', name: '下雨提醒', icon: 'weather', d: 'd1', at: { d1: [0, 0] } },
          { id: 'wa2', type: 'plugin', of: 'tally', size: 'm', name: '数数', d: 'd1', at: { d1: [4, 0] } },
          { id: 'wa3', type: 'plugin', of: 'weather-radar', size: 'm', name: '天气雷达', d: 'd1', at: { d1: [8, 0] } },
          { id: 'wa4', type: 'plugin', of: 'rain-alert', size: 's', name: '下雨提醒', icon: 'weather', d: 'd1', at: { d1: [0, 2] } },
          { id: 'wa5', type: 'plugin', of: 'tally', size: 's', name: '数数', d: 'd1', at: { d1: [2, 2] } },
          { id: 'wa6', type: 'plugin', of: 'weather-radar', size: 's', name: '天气雷达', d: 'd1', at: { d1: [4, 2] } },
        ],
      }),
    },
    // A plugin's own view (tier 2, spec §4: examples/plugins/agent-board,
    // its view.html in her card, the card's glass, corners and buttons hers):
    // medium and large on the desktop; small, and another's medium beside,
    // drawn from their data.
    {
      id: 'widgets-addon-view-desk',
      section: 'widgets',
      title: '桌面上插件自绘的界面：Agent 排行的中档和大档；小档和下雨提醒用数据画',
      page: 'desk',
      view: [0, 0, 1100, 420],
      budget: 5000,
      settle: 900,
      world: F => ({
        widgets: [],
        addons: [F.ADDON_BOARD, ...addonsRunning(F)],
        deskLayout: [
          { id: 'wv1', type: 'widget', of: 'agent-board', size: 'm', d: 'd1', at: { d1: [0, 0] } },
          { id: 'wv2', type: 'plugin', of: 'agent-board', size: 'l', name: 'Agent 排行', icon: 'chart', d: 'd1', at: { d1: [5, 0] } },
          { id: 'wv3', type: 'widget', of: 'agent-board', size: 's', d: 'd1', at: { d1: [10, 0] } },
          { id: 'wv4', type: 'plugin', of: 'rain-alert', size: 'm', name: '下雨提醒', icon: 'weather', d: 'd1', at: { d1: [0, 2] } },
        ],
      }),
    },
    // In the open capsule: its row drawn from its data, its card unfolded the view.
    {
      id: 'widgets-addon-view-capsule',
      section: 'widgets',
      title: '展开的胶囊里，Agent 排行的卡片打开：插件自绘的界面',
      page: 'island',
      home: 'taskbar',
      view: [0, above(460), 520, 460],
      budget: 5000,
      settle: 900,
      world: F => ({ ...home(F), widgets: F.widgets('weather'), addons: [F.ADDON_BOARD], sessions: [F.session('working')] }),
      steps: async p => {
        await openIsland(p)
        p.click('#island-expanded [data-widget="agent-board"]')
        await p.wait(900)
      },
    },

    // --- The desktop -------------------------------------------------------------------------
    {
      id: 'desktop',
      section: 'desktop',
      title: '单个、叠放、工作台（展开和折叠），在壁纸上',
      page: 'desk',
      view: [0, 0, 1240, 620],
      budget: 4000,
      world: F => ({ widgets: F.WIDGETS, deskLayout: F.DESK.scene }),
    },
    // 移除工作台 from a board's menu: asked first, over the board (open, then folded).
    ...[
      ['wbw', '', '展开的'],
      ['wbf', '-folded', '折叠的'],
    ].map(([board, id, name]) => ({
      id: `desktop-board-remove${id}`,
      section: 'desktop',
      title: `${name}工作台：「移除工作台」先问一句`,
      page: 'desk',
      view: [0, 0, 1240, 620],
      budget: 4000,
      world: F => ({ widgets: F.WIDGETS, deskLayout: F.DESK.scene }),
      steps: async p => {
        await p.wait(400)
        window.Desk.openMenu(board, 'board', { x: 980, y: board === 'wbw' ? 60 : 580 })
        await p.wait(100)
        p.click('.wmenu [data-act="removeBoard"]')
        await p.wait(400)
      },
    })),
    {
      id: 'desktop-board-save',
      section: 'desktop',
      title: '工作台的「存为组合…」：组合的名字就在标题处输入',
      page: 'desk',
      view: [400, 0, 840, 420],
      budget: 4000,
      world: F => ({ widgets: F.WIDGETS, deskLayout: F.DESK.scene }),
      steps: async p => {
        await p.wait(400)
        window.Desk.saveSet?.('wbw')
        await p.wait(200)
      },
    },
    {
      id: 'desktop-ctrl-marquee',
      section: 'desktop',
      title: '按住 Ctrl 在桌面空白处拖出的框，碰到的小工具选中（工作台不选）',
      page: 'desk',
      view: [0, 0, 1240, 620],
      budget: 4000,
      world: F => ({ widgets: F.WIDGETS, deskLayout: F.DESK.scene }),
      steps: async p => {
        await p.wait(400)
        const at = (type, x, y) => new PointerEvent(type, { clientX: x, clientY: y, ctrlKey: true, button: 0, buttons: 1, pointerId: 1, isPrimary: true, bubbles: true, cancelable: true })
        const desk = document.getElementById('desk')
        desk.dispatchEvent(at('pointerdown', 10, 300))
        desk.dispatchEvent(at('pointermove', 300, 360))
        desk.dispatchEvent(at('pointermove', 600, 380))
        await p.wait(100)
      },
    },
  ]

  const DEFAULT_BUDGET = 3000

  const byId = id => LIST.find(s => s.id === id) || null
  // Her home in a shot: its own, else its scene's, else the taskbar.
  const homeOf = s => s.home || (s.scene && sceneById(s.scene)?.home) || 'taskbar'
  // Where the island sits at the top in a shot: its own, else its scene's, else the centre.
  const posOf = s => s.islandPos || (s.scene && sceneById(s.scene)?.islandPos) || 'center'
  // And the edge it is along: its own, else its scene's, else the top (#105).
  const edgeOf = s => s.islandEdge || (s.scene && sceneById(s.scene)?.islandEdge) || 'top'
  // Her home's window in a shot: the taskbar's, or the island's where it sits.
  const homeWindowOf = s => (homeOf(s) === 'island' ? islandWindow(posOf(s), edgeOf(s)) : s.split ? WINDOWS.top : WINDOWS.taskbar)
  // Where a shot's page lies on the screen (a screen shot: the desktop's; her home's frame is put by shot.js).
  const windowOf = s => (s.page === 'bar' && s.split ? WINDOWS.dock : s.page === 'desk' || s.page === 'bar' || s.page === 'screen' ? WINDOWS[s.page === 'screen' ? 'desk' : s.page] : homeWindowOf(s))

  // Where the island sits, and the edge it is along, into a world's settings
  // (as the settings keep them: along the bottom is "taskbar", in Windows'
  // taskbar, which the preview takes to lie along the bottom).
  const withPos = (w, pos, edge) => (pos || edge ? { ...w, config: { ...(w.config || {}), ...(pos ? { islandPos: pos } : {}), ...(edge ? { islandEdge: edge === 'bottom' ? 'taskbar' : edge } : {}) } } : w)

  // A scene's world, whole: its home, where the island sits and its mode with it.
  function sceneWorld(scene, F) {
    return withPos({ ...scene.world(F), home: scene.home || 'taskbar', ...(scene.mode ? { mode: scene.mode } : {}) }, scene.islandPos, scene.islandEdge)
  }
  // A shot's world: its scene's, or its own; its home and where the island sits over either.
  function worldFor(s, F) {
    const scene = s.scene ? sceneById(s.scene) : null
    if (s.scene && !scene) throw new Error(`shot ${s.id}: no scene ${s.scene}`)
    const base = scene ? sceneWorld(scene, F) : {}
    const own = s.world ? s.world(F) : {}
    // The shot's own settings over its scene's (where the island sits kept).
    const config = base.config || own.config ? { ...(base.config || {}), ...(own.config || {}) } : undefined
    const world = withPos({ ...base, ...own, ...(config ? { config } : {}), ...(s.home ? { home: s.home } : {}) }, s.islandPos, s.islandEdge)
    // A shot in English (the README's): her pages, and the world's own words.
    return s.lang === 'en' ? inEnglish(world) : world
  }
  // What plays before a shot's own steps: its scene's, unless left out.
  const playOf = s => (s.scene && s.play !== false ? sceneById(s.scene)?.play || null : null)
  // And on the desktop's page (deskPlay).
  const deskPlayOf = s => (s.scene && s.play !== false ? sceneById(s.scene)?.deskPlay || null : null)
  // The page a shot's steps run on (a screen shot's: her home's).
  const stepsOn = s => (s.page === 'screen' ? 'island' : s.page)

  // What render.ps1 needs of each, as plain data.
  const plain = s => ({
    id: s.id,
    section: s.section,
    title: s.title,
    page: s.page,
    scene: s.scene || null,
    home: homeOf(s),
    split: !!s.split,
    view: s.view,
    window: windowOf(s),
    budget: s.budget || DEFAULT_BUDGET,
    // Its language (the README's English cards): the browser's too, for the
    // dates and the weekday her clock writes in the system's own format.
    lang: s.lang || 'zh',
  })

  const api = {
    SCREEN,
    STRIP,
    SECTIONS,
    WINDOWS,
    WORLD_KEYS,
    SCENES,
    LIST,
    DEFAULT_BUDGET,
    sceneById,
    sceneWorld,
    byId,
    homeOf,
    posOf,
    edgeOf,
    islandWindow,
    homeWindowOf,
    windowOf,
    worldFor,
    playOf,
    deskPlayOf,
    stepsOn,
    plain,
    list: () => LIST.map(plain),
    scenes: () => SCENES.map(s => ({ id: s.id, title: s.title, note: s.note || '', home: s.home || 'taskbar', mode: s.mode || null, shots: LIST.filter(x => x.scene === s.id).map(x => x.id) })),
  }

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api
    if (require.main === module && process.argv.includes('--list')) {
      const out = JSON.stringify({ screen: SCREEN, sections: SECTIONS, scenes: api.list(), worlds: api.scenes() }, null, 1)
      const at = process.argv[process.argv.indexOf('--list') + 1]
      if (at && !at.startsWith('-')) require('node:fs').writeFileSync(at, out)
      else process.stdout.write(out)
    }
  } else {
    root.Scenes = api
  }
})(this)
