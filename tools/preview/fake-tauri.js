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
    // Every widget with something to say (widgets.rs), and the plugins she runs (scripts.rs).
    widgets: [],
    plugins: [],
    taskbarLayout: null,
    deskLayout: [],
    // The taskbar's programs, tray icons and keyboard (taskbar.rs).
    windows: [],
    tray: [],
    keys: {},
    // Any other setting, over the defaults.
    config: {},
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
    // The plugins running as the settings say it (scripts.rs: plugins[id].on).
    plugins: Object.fromEntries(world.plugins.filter(p => p.on).map(p => [p.id, { on: true }])),
    ...world.config,
  })

  // A plugin she runs on or off (scripts.rs: plugins[id].on, the world's
  // own until the settings say).
  function pluginOn(id) {
    const c = config().plugins?.[id]
    if (typeof c?.on === 'boolean') return c.on
    return !!world.plugins.find(p => p.id === id)?.on
  }
  // The widgets as main sends them (widgets_view): each on as widgetsOff
  // says; a plugin's gone once it is off (scripts.rs stops it), and one
  // turned on that had none says what the fixtures have for it
  // (Fixtures.SLEEPING: the picker's plugins, put somewhere).
  function widgetsNow() {
    const off = Array.isArray(config().widgetsOff) ? config().widgetsOff : []
    const list = world.widgets.filter(w => !w.plugin || pluginOn(w.plugin)).map(w => ({ ...w, on: !off.includes(w.id) }))
    const sleeping = window.Fixtures?.SLEEPING || {}
    for (const p of world.plugins) if (pluginOn(p.id) && !list.some(w => w.plugin === p.id) && sleeping[p.id]) list.push({ ...sleeping[p.id], on: !off.includes(sleeping[p.id].id) })
    return list
  }
  // The plugins as scripts.rs views them: on and running, or off.
  const pluginsNow = () => world.plugins.map(p => (pluginOn(p.id) ? { ...p, on: true, state: p.state === 'off' ? 'running' : p.state } : { ...p, on: false, state: 'off' }))

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
      widgets: widgetsNow(),
      unconnected: !world.connected,
      ...(world.connected ? {} : { tools: ['claude', 'codex'] }),
    }
  }

  // settings_get, as settings.rs snapshot makes it.
  function snapshot() {
    const on = world.connected
    return {
      settings: config(),
      lang: world.lang,
      pets: [{ id: 'claude-chan', name: 'Claude', author: 'Wakuwaku', url: world.sprite, version: 2, codex: false }],
      now: payload(),
      sessions: world.sessions,
      asks: world.asks,
      isVisible: true,
      hidden: false,
      connection: on ? 'hooks' : 'none',
      pluginCommands: ['/plugin marketplace add zhoupengjie/Wakuwaku', '/plugin install wakuwaku@wakuwaku'],
      hooks: on ? 'ok' : 'missing',
      claudeSeen: on,
      settingsFile: 'C:\\Users\\you\\.claude\\settings.json',
      settingsShown: '~\\.claude\\settings.json',
      settingsBackup: true,
      claudeHere: true,
      codex: { hooks: on ? 'ok' : 'missing', file: 'C:\\Users\\you\\.codex\\hooks.json', fileShown: '~\\.codex\\hooks.json', seen: on, version: '0.150.0', async: true },
      agy: { hooks: 'absent' },
      opencode: { plugin: 'absent' },
      omp: { hooks: 'absent' },
      zcode: { hooks: 'absent' },
      dsh: { hooks: 'absent' },
      agentVersions: { claude: '2.1.0' },
      displayCount: world.displayCount,
      loginAtStart: true,
      winLook: { light: world.mode === 'light', accent: world.accent.base },
      winPaper: { file: 'gpt-cloud-terrace.png', position: 'fill', every: 0, shuffle: false },
      version: world.version,
      update: { current: world.version, may: true, latest: { version: world.version, newer: false, canInstall: true }, checkedAt: T0 - 2 * 3600e3 },
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
      mail: [],
      mailAgents: { claude: true, codex: true },
      mailModels: {},
      mailTalks: [],
      mailSystemProxy: null,
      mailRev: 0,
      waku: 'C:\\Users\\you\\AppData\\Local\\wakuwaku\\waku.ps1',
      port: 47213,
    }
  }

  // The desktop's picture as taskbar.rs sends it for the strip's Mica: the
  // display from the strip's top-left.
  function stripPaper() {
    const [w, h] = world.screen
    const s = world.strip
    const at = { bottom: [0, -(h - s)], top: [0, 0], left: [0, 0], right: [-(w - s), 0] }[world.edge] || [0, -(h - s)]
    return { url: world.wallpaper, position: 'fill', color: '#1b2533', mon: [...at, w, h], screen: [...at, w, h], sf: 1 }
  }

  // The desktop's window on the display (desk.rs desk:state): its work
  // area the display less the strip.
  function deskState() {
    const [w, h] = world.screen
    const s = world.home === 'taskbar' ? world.strip : 0
    const work = { bottom: [0, 0, w, h - s], top: [0, s, w, h - s], left: [s, 0, w - s, h], right: [0, 0, w - s, h] }[world.edge] || [0, 0, w, h - s]
    return { key: 'd1', primary: true, keys: ['d1'], sf: 1, size: [w, h], work, lang: world.lang, light: world.mode === 'light', detailed: config().details !== false, home: world.home, sprite: world.sprite, layout: world.deskLayout }
  }

  const deskPaper = () => {
    const [w, h] = world.screen
    return { url: world.wallpaper, position: 'fill', color: '#1b2533', mon: [0, 0, w, h], screen: [0, 0, w, h], sf: 1 }
  }

  const look = () => ({ mode: world.mode, accent: world.accent, endTask: false, flashes: 7 })

  // Her taskbar on another display (bars.rs bar:state): Start, the windows,
  // the clock; her capsule at its start while a prompt waits.
  function barState() {
    const c = config()
    return {
      windows: world.windows,
      look: look(),
      edge: world.edge,
      align: c.taskbarAlign,
      material: world.material,
      clearWhen: c.taskbarClearWhen,
      filled: false,
      lang: world.lang,
      paper: stripPaper(),
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
      return
    }
    if (role === 'island') {
      emit('taskbar:look', look())
      if (world.home === 'taskbar') {
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
    session_jump: () => true,
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
    mail_draft: () => null,
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
    stillStyles,
    handle: (cmd, fn) => handlers.set(cmd, fn),
    heard: channel => (listeners.get(channel)?.size || 0) > 0,
    called: cmd => calls.filter(c => c.cmd === cmd),
  }
  if (host) host.api.attach(label, window)
})()
