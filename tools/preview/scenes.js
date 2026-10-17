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
//   mode    dark or light, for a scene about one (else the viewer's, or the shot's)
//   world   F => ({...}): the rest of its world (F: fixtures.js), whole:
//           every one of WORLD_KEYS (test/preview.test.js checks)
//   play    async p => {...}: what happens once her pages are up (a turn
//           ending, the settings opening), on her home's page
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
    config: {},
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
      note: '一轮刚做完：胶囊转三圈彩色流光，留下一个绿点。再点一次这个场景就重播。',
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
      note: '编辑任务栏时点了她的胶囊：上方是胶囊里的小工具。按住左边的点拖动排序（就是轮播和展开时的顺序）；每个选轮播、固定（最多两个，显示在胶囊右端）或展开时（只在展开的岛里）；× 移出胶囊（能撤销）；底下是轮播间隔。从小工具库把卡片拖到这块面板上也能加进来。',
      world: F => ({
        ...desktop(F),
        sessions: [],
        deskLayout: F.DESK.everyday.filter(e => e.id === 'wd1' || e.id === 'wd2'),
        config: {
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
    { id: 'scene-error', section: 'scenes', scene: 'error', title: '出错了 · 流光播完留下的红点', page: 'island', view: CLOSED, budget: 6000, steps: p => p.wait(3400) },
    { id: 'scene-taskbar-full', section: 'scenes', scene: 'taskbar-full', title: '放满小工具的任务栏', page: 'island', view: STRIP_VIEW },
    { id: 'scene-desktop', section: 'scenes', scene: 'desktop', title: '桌面 · 单个、叠放、两个工作台', page: 'screen', view: WHOLE, budget: 4000 },
    { id: 'scene-desktop-empty', section: 'scenes', scene: 'desktop-empty', title: '空桌面', page: 'screen', view: WHOLE, budget: 4000 },
    { id: 'scene-island-home', section: 'scenes', scene: 'island-home', title: '灵动岛模式 · 展开', page: 'island', view: [400, 0, 800, 420], steps: openIsland },
    { id: 'scene-light-clear', section: 'scenes', scene: 'light-clear', title: '浅色 + 透明任务栏', page: 'island', view: STRIP_VIEW },
    { id: 'scene-edit-layout', section: 'scenes', scene: 'edit-layout', title: '编辑任务栏布局中', page: 'island', view: [0, above(200), 1600, 200], settle: 700 },
    { id: 'scene-add-widgets', section: 'scenes', scene: 'add-widgets', title: '添加小工具 · 编辑任务栏，右边是小工具库', page: 'screen', view: WHOLE, budget: 4000, settle: 700 },
    { id: 'scene-edit-capsule', section: 'scenes', scene: 'edit-capsule', title: '编辑胶囊 · 她的胶囊上方的面板', page: 'island', view: [0, above(440), 720, 440], settle: 700 },

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
      budget: 6000,
      world: F => ({ ...home(F), sessions: [F.session('working')] }),
      steps: async p => {
        await p.wait(300)
        p.set({ sessions: [p.F.session('done')] })
        p.push()
        await p.wait(3400)
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
      world: F => ({ ...busy(F), widgets: F.widgets('monitor', 'today', 'weather', 'stock-aapl', 'stock-nvda', 'pomodoro', 'countdown', 'inbox-a1', 'deploy') }),
      steps: async p => {
        p.emit('island:settings', { tab })
        await p.wait(600)
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
  ]

  const DEFAULT_BUDGET = 3000

  const byId = id => LIST.find(s => s.id === id) || null
  // Her home in a shot: its own, else its scene's, else the taskbar.
  const homeOf = s => s.home || (s.scene && sceneById(s.scene)?.home) || 'taskbar'
  // Where a shot's page lies on the screen (a screen shot: the desktop's; her home's frame is put by shot.js).
  const windowOf = s => (s.page === 'desk' || s.page === 'bar' || s.page === 'screen' ? WINDOWS[s.page === 'screen' ? 'desk' : s.page] : WINDOWS[homeOf(s)])

  // A scene's world, whole: its home and its mode with it.
  function sceneWorld(scene, F) {
    return { ...scene.world(F), home: scene.home || 'taskbar', ...(scene.mode ? { mode: scene.mode } : {}) }
  }
  // A shot's world: its scene's, or its own; its home over either.
  function worldFor(s, F) {
    const scene = s.scene ? sceneById(s.scene) : null
    if (s.scene && !scene) throw new Error(`shot ${s.id}: no scene ${s.scene}`)
    return { ...(scene ? sceneWorld(scene, F) : {}), ...(s.world ? s.world(F) : {}), ...(s.home ? { home: s.home } : {}) }
  }
  // What plays before a shot's own steps: its scene's, unless left out.
  const playOf = s => (s.scene && s.play !== false ? sceneById(s.scene)?.play || null : null)
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
    view: s.view,
    window: windowOf(s),
    budget: s.budget || DEFAULT_BUDGET,
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
    windowOf,
    worldFor,
    playOf,
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
