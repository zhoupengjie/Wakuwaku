// A stand-in for Tauri (window.__TAURI__, __TAURI_INTERNALS__) that the real
// pages take as Tauri's: bridge.js (the island's page, her window), desk.js
// and bar.js. Loaded before the page's own scripts (boot.js), so the pages
// run in a plain browser as they run in her windows.
//
// What the Rust side would say comes from a world kept here (Fake.world):
// her home, the strip's edge and material, Windows' mode and accent, the
// sessions in their moods, the prompts waiting, the widgets (their sizes,
// where each is put), the monitor's readings, the desktop's layout, the
// settings. A scene changes it (Fake.set) and sends it to the page
// (Fake.push), or emits any event itself (Fake.emit). What the page asks
// (invoke) is answered from the world, as main would (settings kept and
// sent back, a prompt answered, the desktop's layout kept), or by a
// handler a scene sets (Fake.handle); every call is kept in Fake.calls.
//
// Three ways it runs:
//   alone         in page.html for the gallery (scenes.js), its own world;
//   a host page   the playground's own page (window.WAKU_HOST_PAGE): the
//                 world and what is built from it, nothing of Tauri's;
//   in its frame  a page in the playground: the playground's world, and
//                 what is asked goes to the playground first (WakuHost),
//                 which plays the Rust side for all its frames.
//
// The clock: alone, Date starts at a fixed moment (Fake.T0, 14:42 on a
// Saturday) and moves on as the page's timers fire (each at its due time),
// so the taskbar's time, the sessions' clocks and her frames are the same
// each run; ?clock=real, and in the playground, the real one.
//
// Where things are: in the repo (tools/preview beside src/ and pets/), or as
// the playground is published (preview/ beside pet/, shared/, pets/, assets/).
;(function () {
  const HERE = new URL('.', document.currentScript.src).href
  const IN_REPO = /\/tools\/preview\/$/.test(new URL(HERE).pathname)
  const ROOT = new URL(IN_REPO ? '../../' : '../', HERE).href
  const PAGES = new URL(IN_REPO ? 'src/pet/' : 'pet/', ROOT).href
  const ASSETS = IN_REPO ? new URL('assets/', HERE).href : new URL('assets/', ROOT).href
  // The example plugins' folders (their views) and her SDK, as published.
  const PLUGIN_FILES = new URL(IN_REPO ? 'examples/plugins/' : 'plugins/', ROOT).href
  const SDK_FILE = new URL(IN_REPO ? 'src/addon/sdk.js' : 'addon/sdk.js', ROOT).href
  const Q = new URLSearchParams(location.search)
  const HOST_PAGE = window.WAKU_HOST_PAGE === true
  const host = (() => {
    if (HOST_PAGE || window.parent === window) return null
    try {
      return window.parent.WakuHost && window.parent.Fake ? { api: window.parent.WakuHost, fake: window.parent.Fake } : null
    } catch {
      return null
    }
  })()
  const role = HOST_PAGE ? 'host' : Q.get('role') || 'island'
  // Each window's label, as main names them (bridge.js listens for its own).
  const LABELS = { island: 'island', pet: 'pet', desk: 'desk-1', bar: 'bar-1', barpop: 'barpop' }
  const label = LABELS[role] || role
  const clone = value => (value === undefined ? undefined : JSON.parse(JSON.stringify(value)))

  // --- The clock ------------------------------------------------------------------------
  const realClock = HOST_PAGE || !!host || Q.get('clock') === 'real'
  // Alone: 14:42 on 17 October 2026, in this computer's time zone.
  const T0 = realClock ? Date.now() : new Date(2026, 9, 17, 14, 42, 0).getTime()
  if (!realClock) {
    const Real = Date
    // The clock moves only as the page's timers fire, each at the time it
    // was due: the same in each run, however busy the machine is (with the
    // page's own clock, a turn's 1:36 was 1:37 in a run with more at once).
    let clock = 0
    const nowMs = () => T0 + clock
    const realTimeout = window.setTimeout.bind(window)
    const realInterval = window.setInterval.bind(window)
    const delay = ms => Math.max(0, Number(ms) || 0)
    window.setTimeout = (fn, ms, ...args) => {
      if (typeof fn !== 'function') return realTimeout(fn, ms, ...args)
      const due = clock + delay(ms)
      return realTimeout(() => {
        if (due > clock) clock = due
        fn(...args)
      }, ms)
    }
    window.setInterval = (fn, ms, ...args) => {
      if (typeof fn !== 'function') return realInterval(fn, ms, ...args)
      let due = clock + delay(ms)
      return realInterval(() => {
        if (due > clock) clock = due
        due = clock + delay(ms)
        fn(...args)
      }, ms)
    }
    class FakeDate extends Real {
      constructor(...args) {
        super(...(args.length ? args : [nowMs()]))
      }
      static now() {
        return nowMs()
      }
    }
    window.Date = FakeDate
  }

  // --- The IPC --------------------------------------------------------------------------
  const listeners = new Map()
  const handlers = new Map()
  const calls = []
  const errors = []
  const report = err => {
    errors.push(String(err?.stack || err))
    console.error(err)
  }

  function listen(channel, fn) {
    if (!listeners.has(channel)) listeners.set(channel, new Set())
    listeners.get(channel).add(fn)
    return Promise.resolve(() => listeners.get(channel)?.delete(fn))
  }

  // An event to the page, as Tauri delivers it ({ event, id, payload }): a
  // copy, made here (the page may change what it is given; the world stays).
  function emit(channel, payload) {
    const fns = [...(listeners.get(channel) || [])]
    if (!fns.length) return
    const data = clone(payload)
    for (const fn of fns) {
      try {
        fn({ event: channel, id: 0, payload: data })
      } catch (err) {
        report(err)
      }
    }
  }

  function invoke(cmd, args = {}) {
    calls.push({ cmd, args })
    try {
      // In the playground, its answer first ({ value }), or none (undefined): then ours.
      const got = host ? host.api.invoke(label, cmd, clone(args ?? {})) : undefined
      if (got !== undefined) return Promise.resolve(clone(got.value))
      const answer = handlers.get(cmd) || ANSWERS[cmd]
      return Promise.resolve(answer ? answer(args ?? {}) : null)
    } catch (err) {
      return Promise.reject(err)
    }
  }

  // Told to the page holding this one (shot.html), if any: Windows' acrylic
  // under the island (island_plate) is drawn there, behind the page.
  const tellOuter = message => {
    if (window.parent !== window) window.parent.postMessage({ wk: true, ...message }, '*')
  }

  if (!HOST_PAGE) {
    window.__TAURI_INTERNALS__ = { metadata: { currentWebview: { label }, currentWindow: { label } } }
    window.__TAURI__ = {
      core: { invoke },
      event: { listen, emit: () => Promise.resolve() },
      webviewWindow: { getCurrentWebviewWindow: () => ({ label }) },
    }
  }

  // --- The world ------------------------------------------------------------------------
  // The settings a new user has (data.rs defaults), but for what would make
  // two runs differ: no OLED pixel shift, the monitor on.
  const DEFAULTS = {
    lang: 'zh',
    pet: 'claude-chan',
    scale: 0.55,
    display: 'taskbar',
    out: false,
    islandWidth: 'normal',
    islandPos: 'center',
    islandHideUnder: true,
    taskbarMaterial: 'mica',
    taskbarDisplays: 'main',
    taskbarClearWhen: 'mica',
    taskbarButtons: 'icons',
    taskbarAlign: 'center',
    taskbarEdge: 'bottom',
    oledShift: false,
    autoUpdateCheck: true,
    updateFrom: 'auto',
    details: true,
    widgetsOff: [],
    monitor: { cpu: true, mem: true, net: true, battery: false, every: 2, pin: true },
    widgetOrder: [],
    widgetSpin: 8,
    widgetNudge: true,
    plugins: {},
    mail: [],
    walk: true,
    look: true,
    hold: 8,
    endCue: 'ring',
    idleHideMin: 30,
    notify: { waiting: false, done: false, error: false },
    sound: false,
    promptWaitSec: 290,
    dnd: false,
    hideInFullscreen: true,
    onboarded: true,
    lookDefaults: 2,
  }

  // The mail world (mail.rs, see mailWorld below): the fixtures' two
  // accounts, what they keep and the talks (Fixtures.MAIL), a copy of its
  // own for each world. fixtures.js loads after this file: the world made
  // as it loads has none yet, and takes it the first time mail is asked for
  // (in the playground the host page's world has it from its first scene on).
  const mailFrom = () => (window.Fixtures?.MAIL ? { ...clone(window.Fixtures.MAIL), draft: null, rev: 1 } : null)

  const makeWorld = () => ({
    lang: 'zh',
    // Windows' mode and accent colour (taskbar.rs look).
    mode: 'dark',
    accent: { base: '#0078d4', light: '#4cc2ff', dark: '#0067c0' },
    // Her home (taskbar, island), the strip's edge and material.
    home: 'taskbar',
    edge: 'bottom',
    material: 'mica',
    // The display, in px, and the strip's thickness.
    screen: [1600, 900],
    strip: 48,
    wallpaper: ASSETS + 'wall.jpg',
    sprite: ROOT + 'pets/claude-chan/spritesheet.webp',
    // The sessions as state.rs views them; the first is the one shown.
    sessions: [],
    // The prompts waiting (asks.rs views).
    asks: [],
    // Every widget with something to say (widgets.rs), and the plugins she runs (plugins.rs).
    widgets: [],
    plugins: [],
    // Other people's plugins installed (addons.rs view: Fixtures.ADDONS),
    // the folders that did not install, and the .zip Windows' dialog picks
    // next for 从文件安装… (a name of Fixtures.ADDON_FILES; null: closed;
    // unset: each in turn).
    addons: [],
    addonsBroken: [],
    addonPick: undefined,
    // The plugin market (market.rs, the fake's market command): its index
    // (unset: Fixtures.MARKET), how reading it goes (ok, closed: not open
    // yet, error: not reached), the share a download holds at (unset: it
    // goes on to the end), the updates waiting for the person.
    market: undefined,
    marketMode: 'ok',
    marketHold: undefined,
    marketUpdates: [],
    // Packs installed (packs.rs view: Fixtures.PACKS, each with its folder),
    // her pets that are packs (Fixtures.PACK_PETS), and the desktop's
    // picture as paper.rs reads it (a slideshow's folder: a pack in use).
    packs: [],
    packPets: [],
    paper: { file: 'gpt-cloud-terrace.png', slideshow: false, folder: null, position: 'fill', every: 0, shuffle: false },
    taskbarLayout: null,
    deskLayout: [],
    // The taskbar's programs, tray icons and keyboard (taskbar.rs).
    windows: [],
    tray: [],
    keys: {},
    // Any other setting, over the defaults.
    config: {},
    // The mail accounts, their letters, the talks about them (mailWorld).
    mail: mailFrom(),
    connected: true,
    quiet: false,
    // Windows' animations off (prefers-reduced-motion), as the pages read it; unset, the browser's.
    reducedMotion: undefined,
    version: '0.3.2',
    displayCount: 1,
  })
  const world = host ? host.fake.world : makeWorld()

  if (!HOST_PAGE) {
    // The screen is the world's (headless Edge's is its window, or 800 x 600):
    // what is laid out by it (the open island's most, the settings') as on hers.
    const SCREEN = {
      width: () => world.screen[0],
      height: () => world.screen[1],
      availWidth: () => world.screen[0],
      availHeight: () => world.screen[1] - (world.home === 'taskbar' ? 0 : world.strip),
      availLeft: () => 0,
      availTop: () => 0,
    }
    for (const [key, get] of Object.entries(SCREEN)) Object.defineProperty(window.screen, key, { get, configurable: true })

    // Windows' animations off or on, as the world says (the pages ask
    // matchMedia; their stylesheets' reduced-motion rules made to apply, or not).
    const realMatch = window.matchMedia.bind(window)
    const REDUCED = /prefers-reduced-motion:\s*reduce/
    window.matchMedia = query =>
      REDUCED.test(query) && typeof world.reducedMotion === 'boolean'
        ? { matches: world.reducedMotion, media: query, onchange: null, addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {}, dispatchEvent: () => false }
        : realMatch(query)
    window.addEventListener('load', () => world.reducedMotion === true && stillStyles())
  }

  function stillStyles() {
    for (const sheet of document.styleSheets) {
      let rules = []
      try {
        rules = [...sheet.cssRules]
      } catch {}
      for (const rule of rules) if (rule instanceof CSSMediaRule && /prefers-reduced-motion:\s*reduce/.test(rule.conditionText)) rule.media.mediaText = 'all'
    }
  }

  const config = () => ({
    ...DEFAULTS,
    lang: world.lang,
    display: world.home,
    taskbarEdge: world.edge,
    taskbarMaterial: world.material,
    ...(world.taskbarLayout ? { taskbarLayout: world.taskbarLayout } : {}),
    deskLayout: world.deskLayout,
    // The plugins running as the settings say it (plugins.rs: plugins[id].on;
    // other people's too, with their settings' values).
    plugins: {
      ...Object.fromEntries(world.plugins.filter(p => p.on).map(p => [p.id, { on: true }])),
      ...Object.fromEntries((world.addons || []).map(a => [a.id, { ...Object.fromEntries((a.settings || []).filter(f => f.value != null).map(f => [f.key, f.value])), on: !!a.on }])),
    },
    // The mail accounts as kept (mail.rs: changed only by its commands).
    mail: mailKept(),
    ...world.config,
  })

  // A plugin she runs on or off (plugins.rs: plugins[id].on, the world's
  // own until the settings say).
  function pluginOn(id) {
    const c = config().plugins?.[id]
    if (typeof c?.on === 'boolean') return c.on
    return !!(world.plugins.find(p => p.id === id) || (world.addons || []).find(a => a.id === id))?.on
  }
  // The widgets as main sends them (widgets_view): each on as widgetsOff
  // says; a plugin's gone once it is off (plugins.rs stops it), and one
  // turned on that had none says what the fixtures have for it
  // (Fixtures.SLEEPING: the picker's plugins, put somewhere); someone
  // else's, while it runs, what Fixtures.ADDON_WIDGETS has for it.
  function widgetsNow() {
    const off = Array.isArray(config().widgetsOff) ? config().widgetsOff : []
    const running = new Set(addonsNow().filter(a => a.state === 'running').map(a => a.id))
    const isAddon = id => (world.addons || []).some(a => a.id === id)
    const list = world.widgets.filter(w => !w.plugin || (isAddon(w.plugin) ? running.has(w.plugin) : pluginOn(w.plugin))).map(w => ({ ...w, on: !off.includes(w.id) }))
    const sleeping = window.Fixtures?.SLEEPING || {}
    for (const p of world.plugins) if (pluginOn(p.id) && !list.some(w => w.plugin === p.id) && sleeping[p.id]) list.push({ ...sleeping[p.id], on: !off.includes(sleeping[p.id].id) })
    const theirs = window.Fixtures?.ADDON_WIDGETS || {}
    for (const id of running) if (!list.some(w => w.plugin === id) && theirs[id]) list.push({ ...theirs[id], on: !off.includes(id) })
    return list
  }
  // The plugins as plugins.rs views them: on and running, or off.
  const pluginsNow = () => world.plugins.map(p => (pluginOn(p.id) ? { ...p, on: true, state: p.state === 'off' ? 'running' : p.state } : { ...p, on: false, state: 'off' }))
  // Other people's as addons.rs views them: on as placed is (the settings'
  // switch), each setting's value as kept, how it is (off; a required one
  // empty: needs; stopped as the world has it; running).
  function addonsNow() {
    const conf = config().plugins || {}
    return (world.addons || []).map(a => {
      const on = pluginOn(a.id)
      const settings = (a.settings || []).map(f => ({ ...f, value: conf[a.id]?.[f.key] ?? f.value ?? null }))
      const missing = settings.find(f => f.required && String(f.value ?? f.default ?? '').trim() === '')?.key || null
      const state = a.state === 'unsupported' ? 'unsupported' : !on ? 'off' : missing ? 'needs' : a.state === 'needsApproval' ? 'needsApproval' : a.state === 'stopped' ? 'stopped' : 'running'
      return { ...clone(a), on, state, missing, settings }
    })
  }

  const ISLAND_POS = ['left', 'center', 'right']
  const IDLE = { id: '', project: '', agent: 'claude', jump: false, mood: 'idle', detail: null, since: null, took: null, name: '', step: null, stepSince: null, todo: null, files: 0, reply: '', error: null }

  // pet:update, as main.rs payload makes it.
  function payload() {
    const list = world.sessions
    const top = list[0] || IDLE
    const busy = list.filter(s => s.id !== top.id && s.mood !== 'idle')
    const second = (busy.find(s => s.mood === 'waiting' || s.mood === 'error') || busy[0])?.mood || null
    return {
      ...IDLE,
      ...top,
      others: busy.length,
      sessions: list.length,
      list,
      config: config(),
      lang: world.lang,
      sprite: world.sprite,
      spriteVersion: 2,
      second,
      islandTemp: false,
      settingsOpen: false,
      quiet: world.quiet,
      fullScreen: false,
      plate: true,
      ...(world.home !== 'taskbar' ? { winLight: world.mode === 'light' } : {}),
      // At the top of the screen, the side her window is at (anchor.js): as
      // island.rs says it, having placed the window by islandPos
      // (zhoupengjie/Wakuwaku#59). The playground and the gallery put the
      // island's page there (scenes.js islandWindow).
      ...(world.home !== 'taskbar' ? { islandAt: ISLAND_POS.includes(config().islandPos) ? config().islandPos : 'center' } : {}),
      widgets: widgetsNow(),
      unconnected: !world.connected,
      ...(world.connected ? {} : { tools: ['claude', 'codex'] }),
    }
  }

  // The packs as packs.rs views them: in use, her pet; a wallpaper pack's,
  // Windows' slideshow on its folder (the world's paper).
  const packsNow = () =>
    (world.packs || []).map(p => ({ ...clone(p), inUse: p.kind === 'pet' ? config().pet === p.id : !!world.paper?.slideshow && world.paper.folder === p.folder }))

  // settings_get, as settings.rs snapshot makes it.
  function snapshot() {
    const on = world.connected
    return {
      settings: config(),
      lang: world.lang,
      pets: [{ id: 'claude-chan', name: 'Claude', author: 'Wakuwaku', url: world.sprite, version: 2, codex: false, pack: null }, ...clone(world.packPets || []).filter(p => (world.packs || []).some(k => k.kind === 'pet' && k.id === p.id))],
      now: payload(),
      sessions: world.sessions,
      asks: world.asks,
      isVisible: true,
      hidden: false,
      // Each agent's block, by its id (settings.rs AgentRow).
      agents: {
        claude: { hooks: on ? 'ok' : 'missing', connection: on ? 'hooks' : 'none', here: true, seen: on, file: 'C:\\Users\\you\\.claude\\settings.json', fileShown: '~\\.claude\\settings.json', backup: true, pluginCommands: ['/plugin marketplace add zhoupengjie/Wakuwaku', '/plugin install wakuwaku@wakuwaku'] },
        codex: { hooks: on ? 'ok' : 'missing', file: 'C:\\Users\\you\\.codex\\hooks.json', fileShown: '~\\.codex\\hooks.json', seen: on, version: '0.150.0', async: true },
        agy: { hooks: 'absent' },
        opencode: { hooks: 'absent' },
        omp: { hooks: 'absent' },
        zcode: { hooks: 'absent' },
        // DeepSeek Harness as a shot's world has it (settings.rs agent_row), else not here.
        dsh: world.dsh || { hooks: 'absent' },
        // Reasonix on this computer too (zhoupengjie/Wakuwaku#60): connected
        // with the others, unless the world says how it stands (world.reasonix).
        reasonix: { hooks: on ? 'ok' : 'missing', file: 'C:\\Users\\you\\AppData\\Roaming\\reasonix\\settings.json', fileShown: '~\\AppData\\Roaming\\reasonix\\settings.json', backup: true, seen: on, line: '2.x', ...(world.reasonix || {}) },
      },
      agentVersions: { claude: '2.1.0', reasonix: world.reasonix?.version || '2.43.0', ...(world.dsh ? { dsh: world.dsh.dshVersion || '0.2.1-rc.2' } : {}) },
      displayCount: world.displayCount,
      loginAtStart: true,
      winLook: { light: world.mode === 'light', accent: world.accent.base },
      winPaper: clone(world.paper),
      packs: packsNow(),
      version: world.version,
      update: { current: world.version, may: true, latest: { version: world.version, newer: false, canInstall: true }, checkedAt: T0 - 2 * 3600e3, ...(world.update || {}) },
      crash: { can: false },
      fullscreenAvailable: true,
      plate: true,
      widgets: widgetsNow(),
      lhm: { status: 'none' },
      hw: {},
      monitorSource: 'builtin',
      cpuSensors: { state: 'off' },
      lhmSetup: {},
      plugins: pluginsNow(),
      // Other people's plugins installed (addons.rs view), and the folders
      // there that did not install, each with why.
      addons: addonsNow(),
      addonsBroken: clone(world.addonsBroken || []),
      // The market's waiting for the person (market.rs updates_view).
      marketUpdates: clone(world.marketUpdates || []),
      // The mail accounts with how each is going, the agents found, their
      // models, the talks about letters, Windows' proxy, the store's revision.
      mail: mailView(),
      mailAgents: { ...MAIL_AGENTS },
      mailModels: clone(MAIL_MODELS),
      mailTalks: clone(mailWorld().talks),
      mailSystemProxy: clone(mailWorld().systemProxy) ?? null,
      // Windows' proxy as the web takes it (the General page's Network proxy).
      netSystemProxy: 'netSystemProxy' in world ? world.netSystemProxy : (clone(mailWorld().systemProxy) ?? null),
      mailRev: mailWorld().rev,
      port: 47213,
    }
  }

  // The desktop's picture as taskbar.rs sends it for the strip's Mica: the
  // display from the strip's top-left.
  function stripPaper() {
    const [w, h] = world.screen
    const s = world.strip
    const at = { bottom: [0, -(h - s)], top: [0, 0], split: [0, 0], left: [0, 0], right: [-(w - s), 0] }[world.edge] || [0, -(h - s)]
    return { url: world.wallpaper, position: 'fill', color: '#1b2533', mon: [...at, w, h], screen: [...at, w, h], sf: 1 }
  }

  // The desktop's window on the display (desk.rs desk:state): its work
  // area the display less the strip.
  function deskState() {
    const [w, h] = world.screen
    const s = world.home === 'taskbar' ? world.strip : 0
    // Split: the top bar's (32 unless 48), the dock keeping no room (splitDock hide).
    const top = world.edge === 'split' ? splitH() : s
    const work = { bottom: [0, 0, w, h - s], top: [0, s, w, h - s], split: [0, top, w, h - top], left: [s, 0, w - s, h], right: [0, 0, w - s, h] }[world.edge] || [0, 0, w, h - s]
    // accent: Windows' (as taskbar:look has it, and desk.rs sends it), for what is picked with Ctrl.
    return { key: 'd1', primary: true, keys: ['d1'], sf: 1, size: [w, h], work, lang: world.lang, light: world.mode === 'light', accent: world.accent, detailed: config().details !== false, home: world.home, sprite: world.sprite, layout: world.deskLayout }
  }

  const deskPaper = () => {
    const [w, h] = world.screen
    return { url: world.wallpaper, position: 'fill', color: '#1b2533', mon: [0, 0, w, h], screen: [0, 0, w, h], sf: 1 }
  }

  // Task View's button on, as Windows has it by default.
  const look = () => ({ mode: world.mode, accent: world.accent, endTask: false, flashes: 7, seconds: world.seconds === true, taskView: world.taskView !== false })

  // Her taskbar on another display (bars.rs bar:state): Start, the windows,
  // the clock; her capsule at its start while a prompt waits.
  // The desktop's picture for the dock at the foot, from its window's corner (bars.rs paper_from).
  function dockPaper() {
    const [w, h] = world.screen
    const at = [0, -(h - 48)]
    return { url: world.wallpaper, position: 'fill', color: '#1b2533', mon: [...at, w, h], screen: [...at, w, h], sf: 1 }
  }

  // Split, the top bar's thickness (island.rs strip_h).
  const splitH = () => (config().splitBar === 48 ? 48 : 32)

  function barState() {
    const c = config()
    // Split, the dock on her display (bars.rs refresh): along the foot, its
    // keys if set so, the desktop's picture from its window's corner.
    const dock = world.edge === 'split'
    return {
      dock,
      keys: dock && c.splitKeys === 'dock',
      keysNow: dock ? world.keys : null,
      stowed: false,
      windows: world.windows,
      look: look(),
      edge: dock ? 'bottom' : world.edge,
      align: c.taskbarAlign,
      material: world.material,
      clearWhen: c.taskbarClearWhen,
      filled: false,
      lang: world.lang,
      paper: dock ? dockPaper() : stripPaper(),
      sprite: world.sprite,
      spriteVersion: 2,
      islandLook: c.islandLook,
      asks: world.asks.length ? { n: world.asks.length, kind: world.asks[0].kind === 'question' ? 'answer' : 'approve' } : null,
    }
  }

  // The world sent to the page, as main sends it to that window.
  function push() {
    if (role === 'bar') return emit('bar:state', barState())
    if (role === 'desk') {
      emit('desk:state', deskState())
      emit('desk:widgets', { widgets: widgetsNow() })
      emit('desk:paper', deskPaper())
      // The plugins' views here: not while nobody is about (desk.rs views).
      emit('desk:views', { on: !world.quiet })
      return
    }
    if (role === 'island') {
      emit('taskbar:look', look())
      if (world.home === 'taskbar') {
        // Split: the top bar as thick as Windows gave it, in the screen's px
        // as island.rs tell_strip sends it (the shots are at twice the scale).
        const dpr = devicePixelRatio || 1
        if (world.edge === 'split') emit('taskbar:strip', { thick: Math.round(splitH() * dpr), win: [Math.round(innerWidth * dpr), Math.round(innerHeight * dpr)], sf: dpr })
        emit('taskbar:wallpaper', stripPaper())
        emit('taskbar:windows', world.windows)
        emit('taskbar:keys', world.keys)
        emit('taskbar:tray', world.tray)
      }
      emit('pet:update', payload())
      emit('pet:asks', world.asks)
    }
  }

  // Part of the world changed; objects merged one level down (config, keys).
  function set(patch = {}) {
    for (const [k, v] of Object.entries(patch)) {
      if (v && typeof v === 'object' && !Array.isArray(v) && world[k] && typeof world[k] === 'object' && !Array.isArray(world[k]) && k !== 'taskbarLayout') world[k] = { ...world[k], ...v }
      else world[k] = v
    }
    return world
  }

  // The desktop's layout as desk.rs keeps it: entries put there or changed, ids gone.
  function keepDesk(entries = [], gone = []) {
    const given = new Map(entries.map(e => [e.id, e]))
    const out = []
    for (const e of world.deskLayout) {
      if (gone.includes(e.id)) continue
      out.push(given.get(e.id) || e)
      given.delete(e.id)
    }
    world.deskLayout = [...out, ...given.values()]
  }

  // A prompt answered (pet_answer) or handed to the terminal (pet_dismiss):
  // gone, its session at work again.
  function answered(id, back = true) {
    const ask = world.asks.find(a => a.id === id)
    world.asks = world.asks.filter(a => a.id !== id)
    const s = ask && world.sessions.find(x => x.id === ask.session)
    if (s && back && !world.asks.some(a => a.session === s.id)) Object.assign(s, { mood: 'working', detail: null, step: { key: 'step.thinking', vars: {} }, stepSince: Date.now() })
    return !!ask
  }

  // --- Mail (mail.rs) -------------------------------------------------------------------
  // The mail world (world.mail): the accounts as the settings keep them,
  // each with how its sync is going (sync.rs: state, the unread, how much is
  // kept whole, the proxy it went through); what each keeps on this computer,
  // its inbox and Sent by UID (store.rs, with each letter's text and files);
  // the talks with an agent about a letter, by its key (agent.rs); the letter
  // being written (mail_draft); Windows' proxy as mail takes it; the store's
  // revision (mail.rs rev: the Mail page asks for its list again when it
  // changed). Nothing leaves the page: no server is asked, no agent runs.
  const mailEmpty = () => ({ accounts: [], letters: {}, talks: {}, draft: null, rev: 0, systemProxy: null })
  function mailWorld() {
    if (!world.mail) world.mail = mailFrom()
    return world.mail || mailEmpty()
  }
  const mailAccount = id => mailWorld().accounts.find(a => a.id === id) || null
  const boxOf = folder => (folder === 'sent' ? 'sent' : 'inbox')
  const letterOf = (id, folder, uid) => (mailWorld().letters[id]?.[boxOf(folder)] || []).find(l => l.uid === Number(uid)) || null
  const unreadIn = id => (mailWorld().letters[id]?.inbox || []).filter(l => !l.seen).length
  const mailFail = (kind, text = '') => ({ ok: false, error: { kind, text } })
  const GONE = () => mailFail('gone')
  const inZh = () => world.lang !== 'en'

  // The agents that read mail, as found on this computer (agent.rs
  // available: Antigravity's agy not), and the models each can be asked for
  // (agent.rs models: Claude's by alias, Codex's from its list, each with
  // the efforts it takes).
  const MAIL_AGENTS = { claude: true, codex: true, agy: false }
  const AGENT_NAMES = { claude: 'Claude Code', codex: 'Codex', agy: 'Antigravity' }
  const EFFORTS = ['low', 'medium', 'high', 'xhigh', 'max']
  const MAIL_MODELS = {
    claude: [
      ['fable', 'Fable'],
      ['opus', 'Opus'],
      ['sonnet', 'Sonnet'],
      ['haiku', 'Haiku'],
    ].map(([id, name]) => ({ id, name, efforts: EFFORTS })),
    codex: [
      { id: 'gpt-5.6', name: 'GPT-5.6', efforts: ['low', 'medium', 'high', 'xhigh'] },
      { id: 'gpt-5.6-luna', name: 'GPT-5.6 Luna', efforts: ['low', 'medium', 'high'] },
    ],
    claudeEfforts: EFFORTS,
    agy: [],
  }

  // Each account as mail.rs view sends it: as kept, with how its sync is
  // going (none while it is off), its unread counted from its inbox.
  function mailView() {
    return mailWorld().accounts.map(({ status, ...a }) => ({ ...clone(a), status: a.on && status ? { ...clone(status), ...(status.state === 'ok' ? { unread: unreadIn(a.id) } : {}) } : null }))
  }
  // As the settings keep them ("mail"): without how they are going.
  function mailKept() {
    return mailWorld().accounts.map(({ status, ...a }) => clone(a))
  }

  // The proxy an account's servers are reached through (mail/proxy.rs): its
  // own way, else the Mail page's (Windows' proxy unless set); its address.
  function viaOf(a) {
    const page = config().mailProxy?.mode && config().mailProxy.mode !== 'follow' ? config().mailProxy : config().netProxy
    const way = a.proxy || page || { mode: 'system' }
    if (way.mode === 'none') return null
    if (way.mode === 'manual') return way.address || null
    return mailWorld().systemProxy?.address || null
  }

  // A letter for the inbox's list (threads.rs letter_view): where it is,
  // how deep in its conversation, who (oneself said so), when, what about,
  // its flags, the start of its text.
  const attachedOf = l => (l.files || []).length > 0
  const snippetOf = text => [...String(text || '').split(/\s+/).filter(Boolean).join(' ')].slice(0, 140).join('')
  function letterRow(a, it, depth) {
    const l = it.l
    return {
      account: a.id,
      folder: it.folder,
      uid: l.uid,
      depth,
      from: l.from.name || l.from.address,
      address: l.from.address,
      me: it.folder === 'sent' || l.from.address.toLowerCase() === a.address.toLowerCase(),
      date: l.date,
      subject: l.subject,
      seen: l.seen,
      flagged: l.flagged,
      answered: l.answered,
      attached: attachedOf(l),
      snippet: snippetOf(l.text),
      messageId: l.id,
    }
  }

  // An account's conversations as threads.rs builds them: its inbox and its
  // Sent together (a letter in both once, the inbox's), a letter and every
  // id it names one conversation; those with a letter in the inbox that the
  // filter lets through (unseen: one unread in the inbox; flagged: one starred).
  function threadsOf(a, filter) {
    const kept = mailWorld().letters[a.id] || { inbox: [], sent: [] }
    const items = []
    const ids = new Set()
    for (const folder of ['inbox', 'sent']) {
      for (const l of kept[folder] || []) {
        if (l.id && ids.has(l.id)) continue
        if (l.id) ids.add(l.id)
        items.push({ folder, l, id: l.id || `${folder}:${l.uid}` })
      }
    }
    const up = new Map()
    const root = x => {
      if (!up.has(x)) up.set(x, x)
      while (up.get(x) !== x) x = up.get(x)
      return x
    }
    for (const it of items) for (const other of [...(it.l.refs || []), it.l.parent].filter(Boolean)) {
      const [ra, rb] = [root(it.id), root(other)]
      if (ra !== rb) up.set(rb, ra)
    }
    const groups = new Map()
    for (const it of items) {
      const r = root(it.id)
      if (!groups.has(r)) groups.set(r, [])
      groups.get(r).push(it)
    }
    const unseen = it => it.folder === 'inbox' && !it.l.seen
    const passes = members => (filter === 'unseen' ? members.some(unseen) : filter === 'flagged' ? members.some(it => it.l.flagged) : true)
    return [...groups.values()].filter(members => members.some(it => it.folder === 'inbox') && passes(members)).map(members => threadView(a, members))
  }

  // One conversation (threads.rs thread_view): each letter under the one it
  // answers (else the nearest before that it names, that is here; never in
  // a loop), each in the order sent; who wrote, each once; what a click on
  // it opens (the newest unread in the inbox, else the newest).
  function threadView(a, members) {
    const here = new Map(members.map(it => [it.id, it]))
    const when = it => it.l.date || 0
    const parent = new Map()
    for (const it of members) {
      const p = [it.l.parent, ...[...(it.l.refs || [])].reverse()].filter(Boolean).map(x => here.get(x)).find(x => x && x !== it)
      if (!p) continue
      let at = p
      let steps = 0
      while (at && at !== it && steps++ < members.length) at = parent.get(at)
      if (at !== it) parent.set(it, p)
    }
    const byTime = list => list.sort((x, y) => when(x) - when(y) || x.l.uid - y.l.uid)
    const kids = new Map()
    const roots = []
    for (const it of members) {
      const p = parent.get(it)
      if (!p) roots.push(it)
      else kids.set(p, [...(kids.get(p) || []), it])
    }
    const order = []
    const walk = (it, depth) => {
      order.push([it, depth])
      for (const k of byTime(kids.get(it) || [])) walk(k, depth + 1)
    }
    for (const r of byTime(roots)) walk(r, 0)
    const letters = order.map(([it, depth]) => letterRow(a, it, depth))
    const people = []
    const who = new Set()
    for (const l of letters) {
      const key = l.me ? 'me' : l.address.toLowerCase()
      if (who.has(key)) continue
      who.add(key)
      people.push({ name: l.from, me: l.me })
    }
    const newest = list => list.reduce((best, it) => (!best || when(it) >= when(best) ? it : best), null)
    const all = order.map(([it]) => it)
    const open = newest(all.filter(it => it.folder === 'inbox' && !it.l.seen)) || newest(all)
    const first = all[0]
    return {
      key: `${a.id}|${first.id}`,
      account: a.id,
      latest: Math.max(...all.map(when)),
      count: letters.length,
      unseen: members.filter(it => it.folder === 'inbox' && !it.l.seen).length,
      flagged: members.some(it => it.l.flagged),
      attached: members.some(it => attachedOf(it.l)),
      subject: first.l.subject,
      people,
      open: open ? { folder: open.folder, uid: open.l.uid } : null,
      letters,
    }
  }

  // A letter read whole, for the page (letters.rs view): who from, to whom,
  // what it answers, when, its text and files; where it is, its key (what
  // finds it again: its Message-ID, else the account and UID), its star.
  function letterWhole(id, folder, l) {
    return {
      from: [clone(l.from)],
      to: clone(l.to || []),
      cc: clone(l.cc || []),
      replyTo: clone(l.replyTo || []),
      references: clone(l.refs || []),
      subject: l.subject,
      date: l.date,
      messageId: l.id,
      text: l.text,
      cut: false,
      attachments: clone(l.files || []),
      uid: l.uid,
      key: l.id || `${id}:${l.uid}`,
      account: id,
      folder: boxOf(folder),
      flagged: l.flagged,
    }
  }

  // The settings and her island told what changed, a moment later, as main
  // does (push_settings; the island drawn anew for the widgets).
  function tellMail() {
    later(() => {
      emit('settings:changed', snapshot())
      if (role === 'island') emit('pet:update', payload())
    })
  }

  // An account's widget made anew from its inbox (sync.rs tell, mail.rs
  // show) where the world has it, or (add: on again, or new) put last; gone
  // while the account is off or removed (mail.rs sync). The list made anew
  // (the world's may be the fixtures' own).
  function inboxNow(id, add = false) {
    const wid = `inbox-${id}`
    const a = mailAccount(id)
    const at = world.widgets.findIndex(w => w.id === wid)
    if (!a || !a.on) {
      if (at >= 0) world.widgets = world.widgets.filter(w => w.id !== wid)
      return
    }
    const make = window.Fixtures?.inboxWidget
    if (!make || (at < 0 && !add)) return
    const made = make(a, mailWorld().letters[id]?.inbox || [])
    world.widgets = at >= 0 ? world.widgets.map((w, i) => (i === at ? { ...w, label: made.label, value: made.value, color: made.color, detail: made.detail } : w)) : [...world.widgets, made]
  }

  // What is kept changed (a letter read, starred, sent; a sync): the store's
  // revision on (the page asks for its list again), the account's widget
  // anew, and both told.
  function mailChanged(id, add = false) {
    mailWorld().rev += 1
    if (id) inboxNow(id, add)
    tellMail()
  }

  // An account's sync at work (mail.rs sync, sync.rs): connecting, and a
  // moment later its inbox looked at (its first sync done), how much is kept
  // and the proxy it went through said; its widget there.
  function syncSoon(id) {
    const a = mailAccount(id)
    if (!a) return
    a.status = { ...(a.status || {}), state: 'connecting' }
    setTimeout(() => {
      const now = mailAccount(id)
      if (!now?.on) return
      const kept = mailWorld().letters[id] || { inbox: [], sent: [] }
      kept.synced = true
      const n = kept.inbox.length + kept.sent.length
      now.status = { offline: { have: n, total: n, bytes: 0 }, ...now.status, state: 'ok', via: viaOf(now) }
      mailChanged(id, true)
    }, 900)
  }

  // The people in a field as typed (send.rs people_in): split at , ; and new
  // lines (not inside quotes or <>), each "name <address>" or an address;
  // one that is not, said.
  function peopleIn(text) {
    const out = []
    let part = ''
    let quoted = false
    let angle = false
    for (const c of `${text || ''},`) {
      if (c === '"') quoted = !quoted
      else if (c === '<' && !quoted) angle = true
      else if (c === '>' && !quoted) angle = false
      if (/[,;\n]/.test(c) && !quoted && !angle) {
        const p = part.trim()
        if (p) {
          const m = /^(.*?)<([^<>]+)>$/.exec(p)
          const address = (m ? m[2] : p).trim()
          const name = m ? m[1].trim().replace(/^"|"$/g, '') : ''
          out.push(/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(address) ? { name, address } : { bad: p })
        }
        part = ''
      } else part += c
    }
    return out
  }

  // The servers found for an address (discover.rs): the two example
  // domains as their providers say them, any other one guessed.
  const MAIL_SERVERS = {
    'example.com': { source: 'autoconfig', user: 'local', imap: ['mail.example.com', 993, 'ssl'], smtp: ['mail.example.com', 587, 'starttls'] },
    'example.org': { source: 'ispdb', user: 'address', imap: ['imap.example.org', 993, 'ssl'], smtp: ['smtp.example.org', 465, 'ssl'] },
    'gmail.com': { source: 'ispdb', user: 'address', imap: ['imap.gmail.com', 993, 'ssl'], smtp: ['smtp.gmail.com', 465, 'ssl'] },
  }

  // What an agent says about a letter here, where none runs (agent.rs: what
  // it says, what it asks of one, a reply), saying so; and to more asked.
  function agentSays(l, a) {
    const name = l.from.name || l.from.address
    const me = a?.name || a?.address || ''
    return inZh()
      ? `**讲了什么**：${name} 来信，说的是「${l.subject || '（无主题）'}」。\n\n**要你做什么**：（这是模拟器，没有 agent 真的读过这封信；在她身上，这里是 agent 的回答。）\n\n**怎么回**：\n\`\`\`\n${name}，你好：\n\n收到，谢谢！我看一下再回复你。\n\n${me}\n\`\`\``
      : `**What it says**: a letter from ${name} about "${l.subject || '(no subject)'}".\n\n**What it asks of you**: (this is the simulator: no agent read this letter; on her, an agent's answer is here.)\n\n**A reply**:\n\`\`\`\nHi ${name},\n\nthanks, got it! I will have a look and get back to you.\n\n${me}\n\`\`\``
  }
  const agentAgain = text => (inZh() ? `（模拟器里没有 agent。）你问的是「${text}」；在她身上，agent 在这里接着回答。` : `(No agent in the simulator.) You asked "${text}"; on her, the agent answers on here.`)

  // A turn of a talk under way (agent.rs go): the page told now, the letter
  // read (the first turn), then what it says, and done.
  function talkTurn(key, said, first) {
    later(() => emit('settings:changed', snapshot()))
    const step = (ms, fn) =>
      setTimeout(() => {
        const t = mailWorld().talks[key]
        if (!t || t.state !== 'running') return
        fn(t)
        emit('settings:changed', snapshot())
      }, ms)
    if (first) step(600, t => t.turns.push({ who: 'tool', text: 'Read · letter.md' }))
    step(1600, t => {
      t.turns.push({ who: 'agent', text: said })
      t.state = 'done'
    })
  }

  // --- Other people's plugins (addons.rs addon_manage) ---------------------------------
  // Windows' dialog picks the world's .zip (addonPick), else each of the
  // fixtures' in turn; a .zip looked at says what the fixtures have for it
  // (Fixtures.ADDON_FILES: a plugin, or why not), the version it replaces
  // as installed here; installed, it is in the world (from a file: not
  // reviewed, not placed); taken away, out of the world, its widgets and
  // its places; one stopped, running again.
  let picks = 0
  const DOWNLOADS = 'C:\\Users\\you\\Downloads\\'
  const zipOf = path => String(path || '').split('\\').pop()
  const isPack = x => x?.kind === 'pet' || x?.kind === 'wallpaper'
  // The one of that id here: a plugin, or a pack of its kind.
  const hereOf = got => (isPack(got) ? (world.packs || []).find(p => p.kind === got.kind && p.id === got.id) : (world.addons || []).find(a => a.id === got.id))
  function zipSummary(path) {
    const file = zipOf(path)
    const got = window.Fixtures?.ADDON_FILES?.[file]
    if (!got) return { ok: false, file, refused: { reason: 'zip', file: null, detail: 'not a zip' } }
    if (got.refused) return { ok: false, file, refused: clone(got.refused) }
    const before = hereOf(got)
    return { ok: true, file, id: got.id, name: got.name, version: got.version, author: got.author, kind: got.kind, perms: clone(got.perms), sensitive: !!got.sensitive, replaces: before ? before.version : null, api: got.api ?? 1, supported: true, refused: null }
  }
  // A pack put in (packs.rs put_in): in the world as the snapshot lists
  // it, its update in its place; a pet among her pets.
  function packIn(got, source) {
    const before = hereOf(got)
    const next = { id: got.id, kind: got.kind, name: got.name, description: got.description || null, author: got.author || '', version: got.version, source, reviewed: source === 'market', installedAt: Date.now(), size: got.size || 0, pictures: got.pictures || 0, preview: got.preview || null, inUse: false, folder: `C:\\Users\\you\\wakuwaku-data\\${got.kind === 'pet' ? 'pets' : 'wallpapers'}\\${got.id}` }
    world.packs = before ? world.packs.map(p => (p === before ? next : p)) : [...(world.packs || []), next]
    if (got.kind === 'pet' && !(world.packPets || []).some(p => p.id === got.id)) world.packPets = [...(world.packPets || []), { id: got.id, name: got.name, author: got.author || '', url: world.sprite, version: 2, codex: false, pack: { source, reviewed: source === 'market' } }]
    return before
  }
  function addonManage(what, arg) {
    if (what === 'pick') {
      const files = Object.keys(window.Fixtures?.ADDON_FILES || {})
      const name = world.addonPick === undefined ? files[picks++ % Math.max(1, files.length)] : world.addonPick
      return { path: name ? DOWNLOADS + name : null }
    }
    if (what === 'inspect') return zipSummary(arg)
    if (what === 'install') {
      const s = zipSummary(arg)
      if (!s.ok) return s
      const got = clone(window.Fixtures.ADDON_FILES[s.file])
      if (isPack(got)) {
        packIn(got, 'file')
        tellMail()
        return s
      }
      const before = (world.addons || []).find(a => a.id === got.id)
      const next = before ? { ...got, on: before.on, settings: before.settings, source: 'file', reviewed: false } : { ...got, source: 'file', reviewed: false, on: false, state: 'off', installedAt: Date.now() }
      world.addons = before ? world.addons.map(a => (a === before ? next : a)) : [...(world.addons || []), next]
      tellMail()
      return s
    }
    if (what === 'uninstall') {
      const id = String(arg || '')
      if (!(world.addons || []).some(a => a.id === id)) return { ok: false, error: `${id}: not installed` }
      const mine = m => !!m && m.type === 'plugin' && m.of === id
      world.addons = world.addons.filter(a => a.id !== id)
      world.widgets = world.widgets.filter(w => w.plugin !== id)
      world.deskLayout = world.deskLayout.filter(e => !mine(e)).map(e => (Array.isArray(e.items) ? { ...e, items: e.items.filter(m => !mine(m)) } : e))
      if (world.taskbarLayout) world.taskbarLayout = Object.fromEntries(Object.entries(world.taskbarLayout).map(([k, run]) => [k, Array.isArray(run) ? run.filter(e => !mine(e)) : run]))
      if (world.config.plugins) world.config = { ...world.config, plugins: Object.fromEntries(Object.entries(world.config.plugins).filter(([k]) => k !== id)) }
      tellMail()
      return { ok: true, snapshot: snapshot() }
    }
    if (what === 'revive') {
      world.addons = (world.addons || []).map(a => (a.id === arg && a.state === 'stopped' ? { ...a, state: 'running', said: null } : a))
      tellMail()
      return snapshot()
    }
    // The new permissions allowed (finding 1): it runs again, nothing more to ask.
    if (what === 'approve') {
      world.addons = (world.addons || []).map(a => (a.id === arg && a.state === 'needsApproval' ? { ...a, state: 'running', newPerms: [] } : a))
      tellMail()
      return snapshot()
    }
    // A pack taken away (packs.rs uninstall, "<kind>:<id>"): her pet, if it
    // was this one, the default one; the slideshow on its folder ended on
    // the picture it showed.
    if (what === 'uninstallPack') {
      const [kind, id] = String(arg || '').split(':')
      const p = (world.packs || []).find(x => x.kind === kind && x.id === id)
      if (!p) return { ok: false, error: `${id}: no ${kind} pack of that name here` }
      world.packs = world.packs.filter(x => x !== p)
      world.packPets = (world.packPets || []).filter(x => !(kind === 'pet' && x.id === id))
      if (kind === 'pet' && config().pet === id) world.config = { ...world.config, pet: 'claude-chan' }
      if (kind === 'wallpaper' && world.paper?.slideshow && world.paper.folder === p.folder) world.paper = { ...world.paper, slideshow: false, folder: null, file: `C:\\Users\\you\\wakuwaku-data\\wallpapers\\.kept\\${String(world.paper.file).split('\\').pop()}` }
      tellMail()
      return { ok: true, snapshot: snapshot() }
    }
    return { ok: false, error: `${what}: inspect, install, uninstall, uninstallPack, pick or revive` }
  }

  // --- The plugin market (market.rs market) ---------------------------------------------
  // Its index as the world has it, read in a moment (or not: closed, not
  // open yet; error, not reached, as her updates say it); one got from it,
  // its download told as it goes (market:progress), held at the world's
  // share if it says one, then in the world as the market's (reviewed,
  // not placed; a pack among the packs, a pet among her pets); every update
  // waiting, put in.
  const marketItems = () => clone(world.market ?? window.Fixtures?.MARKET ?? [])
  const wait = (ms, value) => new Promise(done => setTimeout(() => done(value), ms))
  function marketIndex() {
    if (world.marketMode === 'closed') return wait(250, { items: null, at: null, from: null, error: { code: 'closed', detail: 'GitHub' } })
    if (world.marketMode === 'error') return wait(250, { items: null, at: null, from: null, error: { code: 'failed', detail: 'GitHub: straight, github.com: timeout: connect; Gitee: straight, gitee.com: timeout: connect' } })
    return wait(250, { items: marketItems(), at: Date.now(), from: 'GitHub', error: null })
  }
  const marketWords = v => (v && typeof v === 'object' ? v[world.lang] || v.en : v || '')
  async function marketInstall(id) {
    const item = marketItems().find(i => i.id === id)
    if (!item) return { ok: false, error: { code: 'notFound', detail: id } }
    const tell = (got, stage) => emit('market:progress', { id, got, total: item.size, stage })
    for (const pct of [0, 15, 45, 80, 100]) {
      if (world.marketHold != null && pct > world.marketHold) {
        tell(Math.round((item.size * world.marketHold) / 100), 'download')
        return new Promise(() => {})
      }
      tell(Math.round((item.size * pct) / 100), 'download')
      await wait(180)
    }
    tell(item.size, 'verify')
    await wait(150)
    tell(item.size, 'install')
    await wait(150)
    if (isPack(item)) {
      const before = packIn({ id, kind: item.kind, name: marketWords(item.name), description: marketWords(item.description), author: item.author, version: item.version, size: item.size, pictures: item.kind === 'wallpaper' ? 6 : 0, preview: item.preview }, 'market')
      world.marketUpdates = (world.marketUpdates || []).filter(u => u.id !== id)
      tell(item.size, 'done')
      tellMail()
      return { ok: true, id, name: marketWords(item.name), version: item.version, kind: item.kind, perms: clone(item.perms), replaces: before ? before.version : null, snapshot: snapshot() }
    }
    const before = (world.addons || []).find(a => a.id === id)
    const next = {
      id, kind: 'plugin', name: marketWords(item.name), description: marketWords(item.description), author: item.author, version: item.version, homepage: item.homepage, api: item.api, category: item.category, sizes: item.sizes, color: item.color,
      icon: item.icon ? { url: item.icon } : item.glyph ? { name: item.glyph } : null, view: item.view, perms: clone(item.perms), sensitive: item.sensitive, source: 'market', reviewed: true, installedAt: Date.now(),
      on: before ? before.on : false, state: before ? before.state : 'off', missing: null, said: null, settings: before ? before.settings : [],
    }
    world.addons = before ? world.addons.map(a => (a === before ? next : a)) : [...(world.addons || []), next]
    world.marketUpdates = (world.marketUpdates || []).filter(u => u.id !== id)
    tell(item.size, 'done')
    tellMail()
    return { ok: true, id, name: next.name, version: item.version, kind: 'plugin', perms: clone(item.perms), replaces: before ? before.version : null, snapshot: snapshot() }
  }
  async function marketUpdateAll() {
    const ups = clone(world.marketUpdates || [])
    await wait(400)
    world.addons = (world.addons || []).map(a => {
      const u = ups.find(x => x.id === a.id)
      return u ? { ...a, version: u.to } : a
    })
    world.marketUpdates = []
    tellMail()
    return { ok: true, done: ups.map(u => u.id), failed: [], snapshot: snapshot() }
  }
  function marketCommand(what, arg) {
    if (what === 'index') return marketIndex()
    if (what === 'install') return marketInstall(String(arg || ''))
    if (what === 'updateAll') return marketUpdateAll()
    if (what === 'cancel') return { ok: true }
    return { ok: false, error: { code: 'what', detail: `${what}: index, install, updateAll or cancel` } }
  }

  // --- A plugin's own view (addons.rs addon_view, serve) ----------------------------------
  // Its view.html as her scheme serves it, here from the example's folder
  // (examples/plugins/<id>/, published as plugins/<id>/): her SDK the first
  // script in its head, its own scripts and styles written into it (the
  // frame, sandboxed, has no origin to fetch them from over file://), all
  // as one data: page. Its transitions held at their end for the gallery's
  // shots, as boot.js holds the page's. The widget's view the fixtures' (ADDON_VIEWS).
  const viewPages = new Map()
  const readText = url => {
    const xhr = new XMLHttpRequest()
    xhr.open('GET', url, false)
    xhr.send()
    return xhr.responseText
  }
  function viewPage(id, file) {
    const key = `${id}/${file}`
    if (viewPages.has(key)) return viewPages.get(key)
    let page = null
    try {
      const base = new URL(`${id}/`, PLUGIN_FILES)
      const src = new URL(file, base)
      const doc = new DOMParser().parseFromString(readText(src.href), 'text/html')
      for (const link of doc.querySelectorAll('link[rel="stylesheet"][href]')) {
        const style = doc.createElement('style')
        style.textContent = readText(new URL(link.getAttribute('href'), src).href)
        link.replaceWith(style)
      }
      for (const script of doc.querySelectorAll('script[src]')) {
        const inline = doc.createElement('script')
        inline.textContent = readText(new URL(script.getAttribute('src'), src).href)
        script.replaceWith(inline)
      }
      const sdk = doc.createElement('script')
      sdk.textContent = readText(SDK_FILE)
      doc.head.prepend(sdk)
      if (Q.get('scene') || Q.get('still') === '1') {
        const still = doc.createElement('style')
        still.textContent = '*, *::before, *::after { transition: none !important; animation: none !important; }'
        doc.head.append(still)
      }
      page = `data:text/html;charset=utf-8,${encodeURIComponent(`<!doctype html>\n${doc.documentElement.outerHTML}`)}`
    } catch (err) {
      report(err)
    }
    viewPages.set(key, page)
    return page
  }
  function addonView(what, widget) {
    const w = widgetsNow().find(x => x.id === widget && x.from === 'addon' && Number.isFinite(x.viewRev))
    const a = w && addonsNow().find(x => x.id === w.plugin && x.view && x.state === 'running')
    if (!a) return { error: `${widget}: no view running` }
    if (what === 'send') return { ok: true }
    // The examples' view is their view.html.
    const page = viewPage(a.id, 'view.html')
    if (!page) return { error: `${widget}: its view.html not read` }
    return { page, data: clone(world.addonViews?.[widget] ?? window.Fixtures?.ADDON_VIEWS?.[widget] ?? null), rev: w.viewRev }
  }

  // --- What the page asks ---------------------------------------------------------------
  const later = fn => setTimeout(fn, 0)
  const ANSWERS = {
    settings_get: () => snapshot(),
    // Kept, then sent back as main does: the settings anew, the island drawn anew.
    settings_set: ({ patch }) => {
      const p = { ...(patch || {}) }
      // The desktop's layout is the desktop's (desk.rs keeps it for its pages).
      if (Array.isArray(p.deskLayout)) world.deskLayout = p.deskLayout
      delete p.deskLayout
      Object.assign(world.config, p)
      later(() => {
        emit('settings:changed', snapshot())
        if (role === 'island') emit('pet:update', payload())
      })
      return snapshot()
    },
    settings_open: ({ tab }) => later(() => emit('island:settings', { tab: tab || null })),
    settings_gallery: () => ({ ok: true, items: [], page: 1, totalPages: 1, total: 0 }),
    // The desktop's picture (paper.rs): a wallpaper pack's folder (pack: its
    // id) is Windows' slideshow from then on; the rest, as it was.
    settings_wallpaper: ({ what, value }) => {
      const p = what === 'pack' && (world.packs || []).find(x => x.kind === 'wallpaper' && x.id === value)
      if (p) world.paper = { ...world.paper, slideshow: true, folder: p.folder, file: `${p.folder}\\1.jpg`, every: world.paper?.every || 1800000 }
      return new Promise(done => setTimeout(() => done(snapshot()), 300))
    },
    // The Network proxy's Test (proxy.rs test): GitHub reached through the proxy the page has, or Windows'.
    settings_net_test: ({ proxy }) => {
      const via = proxy?.mode === 'manual' ? proxy.address : proxy?.mode === 'none' ? null : (mailWorld().systemProxy?.address ?? null)
      return new Promise(done => setTimeout(() => done({ ok: true, via, ms: 412 }), 300))
    },
    session_jump: () => true,
    // Other people's plugins to run in the island's page (addons.js): none
    // here (no frame of theirs; what each says is the fixtures', widgetsNow).
    addons_running: () => [],
    // The Plugins page's doings with them (addons.rs addon_manage).
    addon_manage: ({ what, arg }) => addonManage(what, arg),
    // A card's plugin view: its page and data; what it sends (kept in calls).
    addon_view: ({ what, widget }) => addonView(what, widget),
    // The plugin market (market.rs market).
    market: ({ what, arg }) => marketCommand(what, arg),
    pet_walk: () => false,
    pet_answer: ({ id }) => {
      const ok = answered(id)
      later(push)
      return ok
    },
    pet_dismiss: ({ id }) => {
      answered(id, false)
      later(push)
    },
    taskbar_open: () => true,
    taskbar_volume: () => true,
    taskbar_clock_tip: () => new Date().toLocaleDateString('zh-CN', { year: 'numeric', month: 'long', day: 'numeric', weekday: 'long' }),
    taskbar_end_task: () => false,
    taskbar_inputs: () => [],
    taskbar_battery_font: () => null,
    // Windows' pictures are read from Windows itself: the glyph here.
    taskbar_task_view_icons: () => null,
    taskbar_held: () => ({ shift: false, ctrl: false }),
    taskbar_window: () => true,
    taskbar_windows: () => true,
    taskbar_window_menu: () => true,
    taskbar_app: () => true,
    desk_icons: () => [],
    desk_where: () => null,
    desk_keep: ({ entries, gone }) => {
      keepDesk(entries || [], gone || [])
      return true
    },
    // Mail, as mail.rs and its parts answer (bridge.js window.pet.mail),
    // from the mail world and kept there.
    // The inbox's conversations (letters.rs list): one account's or ("*")
    // every account's as one, newest first, the first `count`; how many
    // there are, how many letters the inboxes hold, the accounts that cannot
    // be reached, whether a first sync is still under way.
    mail_letters: ({ id, count, filter }) => {
      const m = mailWorld()
      const all = m.accounts.filter(a => id === '*' || a.id === id)
      if (!all.length && id !== '*') return GONE()
      const threads = all.flatMap(a => threadsOf(a, filter)).sort((x, y) => y.latest - x.latest)
      return {
        ok: true,
        total: threads.length,
        letters: all.reduce((n, a) => n + (m.letters[a.id]?.inbox.length || 0), 0),
        threads: threads.slice(0, Math.max(1, Math.min(2000, Number(count) || 50))),
        errors: all.filter(a => a.on && a.status?.state === 'error').map(a => ({ account: a.id, error: clone(a.status.error) })),
        syncing: all.some(a => a.on && m.letters[a.id]?.synced === false),
      }
    },
    // A letter opened: read whole, and marked read (the widget and the
    // unread anew); with its talk with an agent, if there is one.
    mail_letter: ({ id, uid, folder }) => {
      if (!mailAccount(id)) return GONE()
      const l = letterOf(id, folder, uid)
      if (!l) return mailFail('noLetter')
      if (!l.seen) {
        l.seen = true
        mailChanged(id)
      }
      const letter = letterWhole(id, folder, l)
      return { ok: true, letter, talk: clone(mailWorld().talks[letter.key]) || null }
    },
    // A star put on a letter or taken off.
    mail_flag: ({ id, uid, on, folder }) => {
      if (!mailAccount(id)) return GONE()
      const l = letterOf(id, folder, uid)
      if (!l) return mailFail('noLetter')
      l.flagged = on === true
      mailChanged(id)
      return { ok: true }
    },
    // Refresh: the accounts' syncs (one's, or every one's: "*") look at the
    // server now; nothing new on it here, the page told as a sync tells it.
    mail_kick: ({ id }) => {
      if (mailWorld().accounts.some(a => a.on && (id === '*' || a.id === id))) setTimeout(() => mailChanged(null), 600)
      return null
    },
    // An account on or off: off, its widget gone and its line 已关闭; on,
    // connecting and a moment later looked at again. The settings back.
    mail_switch: ({ id, on }) => {
      const a = mailAccount(id)
      if (a) {
        a.on = on === true
        if (a.on) syncSoon(id)
        else inboxNow(id)
      }
      tellMail()
      return snapshot()
    },
    // The servers for an address (discover.rs): the example domains' as
    // their providers say them, any other domain's guessed.
    mail_discover: ({ address }) => {
      const at = /^[^@\s]+@([^@\s]+\.[^@\s]+)$/.exec(String(address || '').trim())
      if (!at) return { ok: false, error: 'address' }
      const domain = at[1].toLowerCase()
      const known = MAIL_SERVERS[domain] || { source: 'guess', user: 'address', imap: [`imap.${domain}`, 993, 'ssl'], smtp: [`smtp.${domain}`, 465, 'ssl'] }
      const username = known.user === 'local' ? String(address).trim().split('@')[0] : String(address).trim()
      const server = ([host, port, security]) => ({ host, port, security, username, auth: 'auto' })
      return { ok: true, found: true, source: known.source, server: server(known.imap), smtp: server(known.smtp) }
    },
    // Re-test (discover.rs probe_server): a host as one is written answers
    // the way asked, else (auto) SSL/TLS, at the port given or its usual one.
    mail_probe: ({ host, port, security, kind }) => {
      const h = String(host || '').trim().toLowerCase()
      if (!/^[a-z0-9.-]+$/.test(h)) return mailFail('fields')
      const smtp = kind === 'smtp'
      const way = ['ssl', 'starttls', 'plain'].includes(security) ? security : 'ssl'
      const usual = smtp ? { ssl: 465, starttls: 587, plain: 25 } : { ssl: 993, starttls: 143, plain: 143 }
      const p = Number(port) > 0 && Number(port) < 65536 ? Number(port) : usual[way]
      return { ok: true, found: true, security: way, port: p, auths: ['plain', 'login'] }
    },
    // An account added or changed: "signed in" (a password of "wrong" is
    // turned down, to see how that reads), kept, and synced (a new one's
    // inbox empty, its first sync a moment). The settings back with it.
    mail_save: ({ account, password }) => {
      const m = mailWorld()
      const a = account || {}
      const address = String(a.address || '').trim()
      const host = String(a.host || '').trim().toLowerCase()
      if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(address) || !host || !(Number(a.port) > 0) || !String(a.username || '').trim()) return mailFail('fields')
      const known = mailAccount(a.id)
      if (!known && !password) return mailFail('password')
      // Gmail's words for an app password it does not take (imap.rs login_why: badPassword).
      if (password === 'wrong') return { ok: false, error: { kind: 'login', text: 'AUTHENTICATE failed: invalid credentials', why: 'badPassword' }, auths: ['plain', 'login'] }
      if (/gmail\.com$/.test(host) && password === 'wrongwrongwrongw') return { ok: false, error: { kind: 'login', text: 'Invalid credentials (Failure)', why: 'badPassword' } }
      const way = s => (['ssl', 'starttls', 'plain'].includes(s) ? s : 'ssl')
      const smtp = a.smtp && String(a.smtp.host || '').trim() ? { host: String(a.smtp.host).trim().toLowerCase(), port: Number(a.smtp.port), security: way(a.smtp.security), username: String(a.smtp.username || a.username).trim(), auth: a.smtp.auth || 'auto' } : null
      let id = known?.id
      if (!id) for (let n = 1; !id; n++) if (!mailAccount(`a${n}`)) id = `a${n}`
      const kept = { id, address, name: String(a.name || '').trim(), host, port: Number(a.port), security: way(a.security), username: String(a.username).trim(), auth: a.auth || 'auto', smtp, on: true, proxy: a.proxy || null }
      if (known) Object.assign(known, kept)
      else {
        m.accounts.push({ ...kept, status: { state: 'connecting' } })
        m.letters[id] = { inbox: [], sent: [], synced: false }
      }
      syncSoon(id)
      tellMail()
      return { ok: true, unread: unreadIn(id), snapshot: snapshot() }
    },
    // An account removed: what it kept, its talks and its widget gone too.
    mail_remove: ({ id }) => {
      const m = mailWorld()
      m.accounts = m.accounts.filter(a => a.id !== id)
      delete m.letters[id]
      for (const [key, t] of Object.entries(m.talks)) if (t.account === id) delete m.talks[key]
      inboxNow(id)
      m.rev += 1
      tellMail()
      return snapshot()
    },
    // A letter sent (send.rs): from an account with an outgoing server, to
    // the people typed; a copy kept in its Sent (in its conversation), the
    // letter it answers marked so.
    mail_send: ({ letter }) => {
      const d = letter || {}
      const a = mailAccount(d.account)
      if (!a) return GONE()
      if (!a.smtp) return mailFail('noSmtp')
      const [to, cc, bcc] = [d.to, d.cc, d.bcc].map(peopleIn)
      const bad = [...to, ...cc, ...bcc].find(p => p.bad)
      if (bad) return mailFail('address', bad.bad)
      const everyone = [...to, ...cc, ...bcc]
      if (!everyone.length) return mailFail('noOne')
      if (everyone.length > 100) return mailFail('tooMany')
      const m = mailWorld()
      const kept = (m.letters[a.id] ||= { inbox: [], sent: [] })
      const uid = Math.max(0, ...kept.sent.map(l => l.uid)) + 1
      const reply = d.reply
      const answered = reply && letterOf(reply.account, reply.folder, reply.uid)
      if (answered) answered.answered = true
      const passed = d.forward ? letterOf(d.forward.account, d.forward.folder, d.forward.uid) : null
      const files = [...(passed?.files || []).filter((_, i) => (d.forward.keep || []).includes(i)), ...(d.files || []).map(f => ({ name: f.name, size: Math.round((String(f.data || '').length * 3) / 4), type: f.type || '' }))]
      kept.sent.push({
        uid,
        date: Date.now(),
        from: { name: a.name, address: a.address },
        to,
        cc,
        subject: String(d.subject || ''),
        id: `${Date.now().toString(36)}.${uid}@${a.address.split('@')[1]}`,
        parent: reply?.messageId || '',
        refs: reply ? [...(reply.references || []), reply.messageId].filter(Boolean) : [],
        seen: true,
        flagged: false,
        answered: false,
        text: String(d.text || ''),
        files: clone(files),
      })
      mailChanged(a.id)
      return { ok: true, kept: 'kept' }
    },
    // The letter being written, kept (none: gone).
    mail_draft: () => clone(mailWorld().draft) ?? null,
    mail_keep_draft: ({ draft }) => {
      mailWorld().draft = draft && typeof draft === 'object' ? clone(draft) : null
      return null
    },
    // A letter handed to an agent (agent.rs mail_hand): a talk under it, an
    // answer made up here a moment later (no agent runs); a session in a
    // terminal, said not to be had here.
    mail_hand: ({ id, uid, agent, how, folder }) => {
      const a = mailAccount(id)
      if (!a) return GONE()
      if (!(agent in AGENT_NAMES)) return mailFail('agent')
      if (!MAIL_AGENTS[agent]) return mailFail('noAgent', AGENT_NAMES[agent])
      const l = letterOf(id, folder, uid)
      if (!l) return mailFail('noLetter')
      const key = l.id || `${id}:${l.uid}`
      if (how === 'open') return mailFail('start', inZh() ? '模拟器里开不了终端' : 'there is no terminal in the simulator')
      const talks = mailWorld().talks
      if (talks[key]?.state === 'running') return mailFail('busy')
      talks[key] = { key, agent, account: id, mailbox: boxOf(folder), uid: l.uid, state: 'running', error: '', turns: [{ who: 'start', text: '' }] }
      talkTurn(key, agentSays(l, a), true)
      return { ok: true, folder: `${id}-${boxOf(folder)}-${l.uid}`, key }
    },
    // More asked in a letter's talk: said, and answered a moment later.
    mail_say: ({ key, text }) => {
      const said = String(text || '').trim()
      if (!said) return mailFail('empty')
      const t = mailWorld().talks[key]
      if (!t) return mailFail('noTalk')
      if (t.state === 'running') return mailFail('busy')
      t.turns.push({ who: 'me', text: said })
      t.state = 'running'
      talkTurn(key, agentAgain(said), false)
      return { ok: true, key }
    },
    island_plate: ({ rect, fade }) => tellOuter({ plate: rect || null, fade: fade || 0 }),
    // Her own menu is Windows' (tray.rs menu), not the page's: said, not drawn.
    pet_menu: ({ part }) => tellOuter({ nativeMenu: part || 'her' }),
  }

  window.Fake = {
    T0,
    HERE,
    ROOT,
    PAGES,
    ASSETS,
    IN_REPO,
    role,
    label,
    world,
    makeWorld,
    DEFAULTS,
    calls,
    errors,
    listeners,
    emit,
    set,
    push,
    payload,
    snapshot,
    config,
    widgetsNow,
    pluginsNow,
    deskState,
    stripPaper,
    barState,
    keepDesk,
    answered,
    mailWorld,
    mailView,
    stillStyles,
    handle: (cmd, fn) => handlers.set(cmd, fn),
    heard: channel => (listeners.get(channel)?.size || 0) > 0,
    called: cmd => calls.filter(c => c.cmd === cmd),
  }
  if (host) host.api.attach(label, window)
})()
