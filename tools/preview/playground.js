// The playground (playground.html): a desktop with her real pages on it, to
// try how they feel by hand. Her home's page (index.html: the taskbar along
// the bottom, or the island at the top) and the desktop's (desk.html) run in
// frames (preview/page.html, play mode), each where its window would be on a
// 1600 x 900 screen over the wallpaper, scaled to fit. This page plays the
// Rust side for both (window.WakuHost): it holds the world (fake-tauri.js,
// as a host page), answers what they ask first, sends them what main would,
// and stands in for what Windows draws: her own right-click menu (a native
// menu), the windows' live pictures, the acrylic under the island.
//
// The pointer, as her windows let it through: each frame takes it only
// where something of hers is under it (as island.rs and desk.rs decide with
// the pointer and her shapes), else it goes through to what lies below (the
// desktop's widgets, the wallpaper). Presses anywhere are told back to the
// pages that wait for them (taskbar:press, desk:outside, island:blur).
//
// The panel at the side drives what Rust would send: a scene (scenes.js:
// a whole world at once, and what then happens in it; #<id> opens the
// page on one), sessions and their moods, a hello, a tool failing, a
// plugin's or a new version's nudge, Windows' mode, her home, the strip's
// material and edge, the setting for endings, nobody about, reduced
// motion; and starts the scene over.
//
// Served by publish.js dev (window.WAKU_DEV), it follows the repo's files:
// a page's stylesheet changed is swapped in its frame, a script or page
// changed reloads the frames it is in (the world kept, the settings or the
// layout editing open again), and the playground's own files reload it
// whole, the world, the scene and the panel carried over (sessionStorage).
//
// ?demo=open|done|edit|menu|thumbs|card|settings plays one of those once the
// pages are up (the headless shots); ?still=1 holds their transitions at
// their end, as headless Edge would leave them at their start.
;(function () {
  const Fake = window.Fake
  const F = window.Fixtures
  const Scenes = window.Scenes
  const world = Fake.world
  const Q = new URLSearchParams(location.search)
  const W = 1600
  const H = 900
  const STRIP = 48
  const ROOM = 640
  const SIDE_ROOM = 980
  const $ = sel => document.querySelector(sel)
  const clone = v => (v === undefined ? undefined : JSON.parse(JSON.stringify(v)))

  // --- The world to start from --------------------------------------------------------
  const NAMES = [
    ['任务栏设计', 'pet'],
    ['发布说明', 'pet'],
    ['邮件同步', 'mail'],
    ['修 #57', 'pet'],
    ['周报', 'docs'],
    ['desk.js 重构', 'pet'],
  ]
  const STEPS = [
    { key: 'step.edit', vars: { file: 'island.js' } },
    { key: 'step.command', vars: { what: 'cargo test' } },
    { key: 'step.read', vars: { file: 'style.md' } },
    { key: 'step.search', vars: { what: 'placeHer' } },
  ]
  const REPLY = { done: '改好了：胶囊在浅色下的描边。', review: '改了三个文件，来看看。', error: '' }
  let nextSession = 1
  let nextAsk = 1

  function newSession(mood) {
    const n = nextSession++
    const [name, project] = NAMES[(n - 1) % NAMES.length]
    const s = { id: `s${n}`, project, agent: n === 3 ? 'codex' : 'claude', jump: true, name, mood: 'idle', detail: null, at: Date.now(), since: null, took: null, step: null, stepSince: null, todo: null, files: 0, reply: '', error: null }
    moodOf(s, mood, true)
    return s
  }

  // --- The scenes (scenes.js): a whole world at once ------------------------------------
  // Windows' mode as the viewer picked it (the panel), or ?mode=, or their
  // system's: a scene about one mode (light-clear) has its own.
  let viewerMode = ['light', 'dark'].includes(Q.get('mode')) ? Q.get('mode') : matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark'
  let current = 'everyday'
  const DEFAULT_SCENE = 'everyday'

  // The fixtures' clock is this page's start (Fake.T0): a scene's times
  // moved to now, so a turn picked later has run as long as it says.
  function retime(w) {
    const by = Date.now() - F.T0
    for (const s of w.sessions || []) for (const k of ['since', 'stepSince', 'at']) if (typeof s[k] === 'number') s[k] += by
    for (const wd of w.widgets || []) for (const r of wd.detail?.rows || []) if (typeof r.at === 'number') r.at += by
    return w
  }

  // The next session and prompt numbers, after the ones there are.
  function countFrom(w) {
    const top = (list, of) => Math.max(0, ...list.map(of).filter(Number.isFinite))
    nextSession = top(w.sessions || [], s => parseInt(String(s.id).replace(/\D/g, ''), 10)) + 1
    nextAsk = top(w.asks || [], a => Number(a.id)) + 1
  }

  // The world made the scene's (over a fresh one: nothing of the last kept
  // but the viewer's mode and reduced motion), the pages loaded anew, then
  // its play. ?home=, ?material=, ?edge= over the first one (a link to a look).
  function applyScene(id, { play = true, overrides = false } = {}) {
    const scene = Scenes.sceneById(id) || Scenes.sceneById(DEFAULT_SCENE)
    current = scene.id
    const reduced = world.reducedMotion
    const fresh = { ...Fake.makeWorld(), ...retime(clone(Scenes.sceneWorld(scene, F))) }
    for (const k of Object.keys(world)) delete world[k]
    Object.assign(world, fresh, { mode: scene.mode || viewerMode, reducedMotion: reduced })
    if (overrides) {
      if (['island', 'taskbar'].includes(Q.get('home'))) world.home = Q.get('home')
      if (['mica', 'clear', 'black'].includes(Q.get('material'))) world.material = Q.get('material')
      if (['bottom', 'top', 'left', 'right'].includes(Q.get('edge'))) world.edge = Q.get('edge')
    }
    countFrom(world)
    setHash(scene.id)
    freshUi()
    loadAll(() => {
      if (!play || !scene.play) return
      scene.play(player).catch(err => note(`场景没放完：${err.message}`))
    })
    drawPanel()
  }

  // A clean slate for what this page draws over the frames.
  function freshUi() {
    document.documentElement.dataset.theme = world.mode
    editing = false
    watching = false
    drawThumbs([])
    drawPlate(null)
    hideMenu()
  }

  // #<id>: the scene to open on (the only part of the address the hosted page is given).
  const hashScene = () => {
    const id = decodeURIComponent(location.hash.slice(1))
    return Scenes.sceneById(id) ? id : null
  }
  function setHash(id) {
    try {
      if (hashScene() !== id) history.replaceState(null, '', `#${id}`)
    } catch {}
  }
  window.addEventListener('hashchange', () => {
    const id = hashScene()
    if (id && id !== current) applyScene(id)
  })

  // What a scene's play is given (scenes.js), here: the world, sent as main
  // would; events and the pointer to her home's page.
  const player = {
    get F() {
      return F
    },
    get world() {
      return world
    },
    wait: ms => new Promise(resolve => setTimeout(resolve, ms)),
    set: patch => Fake.set(patch),
    push: () => pushAll(),
    emit: (channel, payload) => {
      toIsland(channel, payload)
      if (channel === 'island:settings' || channel === 'taskbar:edit') focusFrame(frames.island)
    },
    hover: sel => pointerOn(sel, 'enter'),
    click: sel => pointerOn(sel, 'click'),
    context: sel => pointerOn(sel, 'context'),
    freeze: () => {},
  }
  function pointerOn(sel, what) {
    const win = frames.island.win
    const n = win?.document.querySelector(sel)
    if (!n) throw new Error(`no ${sel}`)
    const r = n.getBoundingClientRect()
    const at = { clientX: r.left + Math.min(20, r.width / 2), clientY: r.top + Math.min(10, r.height / 2), view: win }
    if (what === 'enter') {
      n.dispatchEvent(new win.PointerEvent('pointerenter', { ...at, bubbles: false }))
      n.dispatchEvent(new win.MouseEvent('mouseenter', { ...at, bubbles: false }))
    } else if (what === 'context') n.dispatchEvent(new win.MouseEvent('contextmenu', { ...at, bubbles: true, cancelable: true, button: 2 }))
    else n.click()
  }

  // A session into a mood, as state.rs keeps it: a turn's clock from its
  // start, an ending's from how long it took; a prompt with a wait.
  function moodOf(s, mood, fresh = false) {
    const now = Date.now()
    const busy = s.mood === 'working' || s.mood === 'waiting'
    world.asks = (world.asks || []).filter(a => a.session !== s.id)
    const base = { mood, detail: null, step: null, stepSince: null, todo: null, files: 0, reply: '', error: null, at: now }
    if (mood === 'working') {
      Object.assign(s, base, { since: busy ? s.since : now - (fresh ? 95e3 : 0), took: null, step: STEPS[(parseInt(s.id.slice(1), 10) - 1) % STEPS.length], stepSince: now - 4e3, todo: { done: 3, total: 7, active: '胶囊的流光' } })
    } else if (mood === 'waiting') {
      Object.assign(s, base, { since: busy ? s.since : now, took: null, detail: { key: 'detail.needsApproval', vars: { tool: 'Bash' } } })
      world.asks.push({ id: nextAsk++, kind: 'permission', tool: 'Bash', summary: 'npm test -- --watch=false', project: s.project, session: s.id, always: { rules: ['Bash(npm test:*)'], dirs: [], modes: [], where: ['localSettings'] } })
    } else if (mood === 'idle') {
      Object.assign(s, base, { since: null, took: null })
    } else {
      Object.assign(s, base, { since: null, took: s.since ? now - s.since : 151e3, reply: REPLY[mood] || '', files: mood === 'review' ? 3 : 0, error: mood === 'error' ? { key: 'error.rate_limit', vars: {} } : null })
    }
  }

  // --- The frames -------------------------------------------------------------------------
  const screenEl = $('#wk-screen')
  const viewport = $('#wk-viewport')
  const frames = {
    island: { role: 'island', el: $('#wk-home'), win: null, rect: [0, 0, 0, 0] },
    desk: { role: 'desk', el: $('#wk-desk'), win: null, rect: [0, 0, W, H] },
  }
  const byLabel = label => (label === 'island' ? frames.island : label.startsWith('desk') ? frames.desk : null)

  function homeRect() {
    if (world.home === 'island') return [(W - 760) / 2, 0, 760, 720]
    return { top: [0, 0, W, STRIP + ROOM], left: [0, 0, STRIP + SIDE_ROOM, H], right: [W - STRIP - SIDE_ROOM, 0, STRIP + SIDE_ROOM, H] }[world.edge] || [0, H - STRIP - ROOM, W, STRIP + ROOM]
  }

  // A frame's page (re)loaded; told once it is up (ready, below).
  let loads = 0
  function load(f) {
    f.win = null
    ready.delete(f.role)
    loads++
    if (f === frames.island) f.rect = homeRect()
    const [x, y, w, h] = f.rect
    Object.assign(f.el.style, { left: `${x}px`, top: `${y}px`, width: `${w}px`, height: `${h}px`, pointerEvents: 'none' })
    f.el.src = `preview/page.html?role=${f.role}&play=1${Q.get('still') ? '&still=1' : ''}&n=${loads}`
  }

  // Both pages up: what waits for them runs (a scene's play, the settings
  // open again), and once, a demo (?demo=).
  const ready = new Set()
  let waiting = []
  const bothUp = () => ready.has('island') && ready.has('desk')
  const whenReady = fn => (bothUp() ? fn() : waiting.push(fn))
  window.addEventListener('message', e => {
    if (!e.data?.wk || !e.data.ready) return
    ready.add(e.data.role)
    if (!bothUp()) return
    const run = waiting
    waiting = []
    for (const fn of run) {
      try {
        fn()
      } catch (err) {
        console.error(err)
      }
    }
    demo()
  })
  function loadAll(then) {
    load(frames.island)
    load(frames.desk)
    if (then) whenReady(then)
  }

  // --- Following the repo's files (publish.js dev) ----------------------------------------
  // What is open on her home's page that a reload would close: the settings
  // (on their tab), the layout being edited.
  function captureUi() {
    const ui = { settings: null, editing, picker: false, capsule: false }
    try {
      const win = frames.island.win
      if (win?.Settings?.isOpen()) ui.settings = win.document.querySelector('.s-tabs button.on')?.dataset.tab || 'now'
      ui.picker = !!win?.Picker?.isOpen()
      ui.capsule = !!win?.CapsulePanel?.isOpen()
    } catch {}
    return ui
  }
  function restoreUi(ui) {
    if (ui?.editing) toIsland('taskbar:edit', ui.picker ? { picker: true } : null)
    else if (ui?.picker) toIsland('island:picker', { open: 'picker' })
    if (ui?.capsule) toIsland('island:picker', { open: 'capsule' })
    if (ui?.settings) toIsland('island:settings', { tab: ui.settings })
    if (ui?.editing || ui?.settings || ui?.picker || ui?.capsule) focusFrame(frames.island)
  }

  // A script or page of theirs changed: these frames anew, the world kept.
  function reloadFrames(roles = ['island', 'desk']) {
    const ui = roles.includes('island') ? captureUi() : null
    if (roles.includes('island')) {
      freshUi()
      load(frames.island)
    }
    if (roles.includes('desk')) load(frames.desk)
    whenReady(() => restoreUi(ui))
  }

  // A stylesheet changed: swapped in the frames that have it, nothing else touched.
  let cssSwaps = 0
  function swapCss(byRole) {
    for (const [role, files] of Object.entries(byRole || {})) {
      const win = frames[role]?.win
      if (!win) continue
      for (const link of win.document.querySelectorAll('link[rel="stylesheet"]')) {
        const url = new URL(link.href)
        if (!files.includes(url.pathname.replace(/^\//, ''))) continue
        url.searchParams.set('v', String(Date.now()))
        // Windows' animations off: the new sheet's reduced-motion rules made to apply too.
        if (world.reducedMotion === true) link.addEventListener('load', () => win.Fake.stillStyles(), { once: true })
        link.href = url.href
        cssSwaps++
      }
    }
  }

  // The playground's own files changed: the page anew, all it holds carried over.
  const CARRY = 'wk-playground-carry'
  function carryAndReload() {
    try {
      sessionStorage.setItem(CARRY, JSON.stringify({ at: Date.now(), scene: current, viewerMode, world, ui: captureUi(), folded: panel.classList.contains('folded'), y: scrollY }))
    } catch {}
    location.reload()
  }
  function carried() {
    try {
      const c = JSON.parse(sessionStorage.getItem(CARRY) || 'null')
      sessionStorage.removeItem(CARRY)
      return c && Date.now() - c.at < 60e3 && c.world ? c : null
    } catch {
      return null
    }
  }
  function restore(c) {
    current = Scenes.sceneById(c.scene) ? c.scene : DEFAULT_SCENE
    viewerMode = c.viewerMode === 'light' ? 'light' : 'dark'
    for (const k of Object.keys(world)) delete world[k]
    Object.assign(world, Fake.makeWorld(), c.world)
    countFrom(world)
    freshUi()
    panel.classList.toggle('folded', !!c.folded)
    loadAll(() => restoreUi(c.ui))
    drawPanel()
    requestAnimationFrame(() => scrollTo(0, c.y || 0))
  }

  function follow() {
    const short = files => files.map(f => f.split('/').pop()).join('、')
    const es = new EventSource('__live')
    es.addEventListener('change', e => {
      let m
      try {
        m = JSON.parse(e.data)
      } catch {
        return
      }
      if (m.page) return carryAndReload()
      if (m.frames?.length) reloadFrames(m.frames)
      const css = Object.fromEntries(Object.entries(m.css || {}).filter(([role]) => !m.frames?.includes(role)))
      swapCss(css)
      note(`${m.frames?.length ? '重新载入了' : '换上了新样式：'}${short(m.files || [])}`)
    })
  }

  // A frame's own events, once its stand-in is up (WakuHost.attach): the
  // pointer routed, presses told back, where a right press was.
  function hook(f, win) {
    f.win = win
    const doc = win.document
    const toScreen = e => [f.rect[0] + e.clientX, f.rect[1] + e.clientY]
    doc.addEventListener('pointermove', e => route(...toScreen(e), e.buttons), true)
    doc.addEventListener('pointerdown', e => pressed(...toScreen(e), f), true)
    doc.addEventListener('contextmenu', e => (lastMenuAt = toScreen(e)), true)
  }

  // --- The pointer, as her windows let it through ----------------------------------------
  // Hers on her home's page: anything but the containers it lays itself out
  // in; on the taskbar, its strip whole (Windows' own is the same).
  const THROUGH = new Set(['island-bar', 'island-row', 'stage', 'goo', 'drop-her'])
  function catches(f, sx, sy) {
    if (!f.win) return false
    const [x, y, w, h] = f.rect
    const px = sx - x
    const py = sy - y
    if (px < 0 || py < 0 || px >= w || py >= h) return false
    if (f === frames.island && world.home === 'taskbar') {
      const strip = { top: py < STRIP, left: px < STRIP, right: px >= w - STRIP }[world.edge] ?? py >= h - STRIP
      if (strip) return true
    }
    let el = null
    try {
      el = f.win.document.elementFromPoint(px, py)
    } catch {
      return false
    }
    const doc = f.win.document
    if (!el || el === doc.documentElement || el === doc.body) return false
    return f === frames.desk ? el.id !== 'desk' : !THROUGH.has(el.id)
  }

  function route(sx, sy, buttons = 0) {
    // A press held (a drag): whoever has it keeps it.
    if (buttons) return
    const onHome = catches(frames.island, sx, sy)
    const onDesk = !onHome && catches(frames.desk, sx, sy)
    frames.island.el.style.pointerEvents = onHome ? 'auto' : 'none'
    frames.desk.el.style.pointerEvents = onDesk ? 'auto' : 'none'
  }

  let scale = 1
  const fromClient = (cx, cy) => {
    const r = screenEl.getBoundingClientRect()
    return [(cx - r.left) / scale, (cy - r.top) / scale]
  }
  const toClient = (sx, sy) => {
    const r = screenEl.getBoundingClientRect()
    return [r.left + sx * scale, r.top + sy * scale]
  }
  screenEl.addEventListener('pointermove', e => route(...fromClient(e.clientX, e.clientY), e.buttons))
  screenEl.addEventListener('pointerdown', e => {
    const [sx, sy] = fromClient(e.clientX, e.clientY)
    // A touch (no hover first): the frame under it takes the next one.
    route(sx, sy)
    pressed(sx, sy, null)
  })
  screenEl.addEventListener('contextmenu', e => e.preventDefault())

  // A press anywhere on the screen: told back to the taskbar's page while it
  // waits for one (its lists and pops close), the desktop's (its menus
  // close) and, off her home, her home's (the settings close: island:blur).
  let watching = false
  function pressed(sx, sy, inFrame) {
    hideMenu()
    const isl = frames.island
    if (watching && isl.win) isl.win.Fake.emit('taskbar:press', { x: sx - isl.rect[0], y: sy - isl.rect[1] })
    if (inFrame !== frames.desk && frames.desk.win) frames.desk.win.Fake.emit('desk:outside', null)
    if (inFrame !== isl && isl.win) isl.win.Fake.emit('island:blur', null)
  }

  // --- Sending, as main does -------------------------------------------------------------
  const pushIsland = () => frames.island.win?.Fake.push()
  const pushDesk = () => frames.desk.win?.Fake.push()
  const pushAll = () => {
    pushIsland()
    pushDesk()
    drawPanel()
  }
  const toIsland = (channel, payload) => frames.island.win?.Fake.emit(channel, payload)
  const focusFrame = f => {
    try {
      f.win?.focus()
    } catch {}
  }

  // --- What the pages ask, answered as Rust would ----------------------------------------
  let lastMenuAt = [W / 2, H - 24]
  let editing = false
  const PART_NAMES = { start: '开始菜单', search: '搜索', tasks: '任务视图', widgets: 'Windows 小组件', desktop: '显示桌面', quick: '快速设置', notifications: '通知中心', calendar: '日历', inputs: '输入法列表' }
  const ASK = {
    settings_set(from, { patch }) {
      const p = clone(patch) || {}
      if ('display' in p) setHome(p.display === 'island' ? 'island' : 'taskbar', false)
      if ('taskbarMaterial' in p) world.material = p.taskbarMaterial
      if ('taskbarEdge' in p && p.taskbarEdge !== world.edge) setEdge(p.taskbarEdge, false)
      for (const k of ['display', 'taskbarMaterial', 'taskbarEdge']) delete p[k]
      // The desktop's layout is the desktop's (desk.rs keeps it, its pages hear it).
      if (Array.isArray(p.deskLayout)) world.deskLayout = p.deskLayout
      delete p.deskLayout
      Object.assign(world.config, p)
      setTimeout(() => {
        frames.island.win?.Fake.emit('settings:changed', frames.island.win.Fake.snapshot())
        pushAll()
      })
      return { value: Fake.snapshot() }
    },
    settings_win_look(from, { light, accent }) {
      if (typeof light === 'boolean') viewerMode = world.mode = light ? 'light' : 'dark'
      if (accent) world.accent = { base: accent, light: accent, dark: accent }
      setTimeout(() => {
        pushAll()
        frames.island.win?.Fake.emit('settings:changed', frames.island.win.Fake.snapshot())
      })
      return { value: true }
    },
    settings_open(from, { tab }) {
      toIsland('island:settings', { tab: tab || null })
      focusFrame(frames.island)
      return { value: null }
    },
    pet_keyboard(from, { needs }) {
      if (needs) focusFrame(byLabel(from))
    },
    desk_type(from, { on }) {
      if (on) focusFrame(frames.desk)
    },
    pet_menu(from, { part }) {
      showMenu(part || 'her')
      return { value: null }
    },
    island_editing(from, { on }) {
      editing = !!on
      if (on) focusFrame(frames.island)
    },
    taskbar_watch_presses(from, { on }) {
      watching = !!on
    },
    taskbar_thumbs(from, { items }) {
      drawThumbs(items || [])
    },
    island_plate(from, { rect }) {
      drawPlate(rect)
    },
    taskbar_open(from, { what }) {
      note(`${PART_NAMES[what] || what}是 Windows 自己的，这里打不开`)
      return { value: true }
    },
    taskbar_app(from, { path, what }) {
      const app = world.windows.find(a => a.path === path)
      if (app && (what === 'pin' || what === 'unpin')) {
        app.pinned = what === 'pin'
        if (!app.pinned && !app.windows.length) world.windows = world.windows.filter(a => a !== app)
        setTimeout(pushIsland)
      } else note(`打开 ${app?.name || '程序'} 要在 Windows 上`)
      return { value: true }
    },
    taskbar_window(from, { id, what }) {
      const app = world.windows.find(a => a.windows.some(w => w.id === id))
      const win = app?.windows.find(w => w.id === id)
      if (!win) return { value: false }
      if (what === 'close' || what === 'end') {
        app.windows = app.windows.filter(w => w !== win)
        if (!app.windows.length && !app.pinned) world.windows = world.windows.filter(a => a !== app)
      } else {
        const toFront = what !== 'press' || !win.front
        for (const a of world.windows) for (const w of a.windows) w.front = false
        win.front = toFront
        win.min = !toFront
      }
      setTimeout(pushIsland)
      return { value: true }
    },
    taskbar_windows(from, { ids }) {
      const wins = world.windows.flatMap(a => a.windows).filter(w => ids.includes(w.id))
      const down = wins.some(w => w.front)
      for (const a of world.windows) for (const w of a.windows) w.front = false
      for (const w of wins) {
        w.min = down
        w.front = false
      }
      if (!down && wins.length) wins[0].front = true
      setTimeout(pushIsland)
      return { value: true }
    },
    taskbar_window_menu() {
      note('窗口自己的菜单是 Windows 的，这里没有')
      return { value: true }
    },
    taskbar_volume(from, { steps }) {
      const [level, muted] = world.keys.volume || [40, false]
      world.keys.volume = [Math.max(0, Math.min(100, level + 2 * steps)), muted]
      setTimeout(pushIsland)
      return { value: true }
    },
    taskbar_tray(from, { press }) {
      if (press === 'leftUp' || press === 'rightUp') note('托盘图标属于各自的程序，这里只是样子')
    },
    session_jump() {
      note('点会话会跳到它的终端窗口（要在 Windows 上）')
      return { value: true }
    },
    pet_answer(from, { id }) {
      const ok = Fake.answered(id)
      setTimeout(pushAll)
      return { value: ok }
    },
    pet_dismiss(from, { id }) {
      Fake.answered(id, false)
      note('交给终端了')
      setTimeout(pushAll)
      return { value: null }
    },
    desk_keep(from, { entries, gone }) {
      Fake.keepDesk(clone(entries) || [], clone(gone) || [])
      // The island's placement follows (a widget in one place); the
      // desktop hears what its home's page put there.
      setTimeout(() => {
        pushIsland()
        if (from === 'island') pushDesk()
      })
      return { value: true }
    },
    desk_ask(from, { what, args }) {
      toIsland('desk:ask', { what, args: clone(args) })
      return { value: null }
    },
    // A widget dragged from her home over the desktop (picker.js): where it
    // is, in the desktop's page's px, to that page (desk-drop.js), as desk.rs
    // will forward it (phase 2); and what that page said, back.
    desk_drag_preview(from, { at }) {
      frames.desk.win?.Fake.emit('desk:drag-preview', at ? toDesk(from, at) : null)
      return { value: null }
    },
    desk_drop(from, { at }) {
      if (at) frames.desk.win?.Fake.emit('desk:drop', toDesk(from, at))
      return { value: null }
    },
    desk_drag_answer(from, { said }) {
      toIsland('island:desk-answer', clone(said))
      return { value: null }
    },
    settings_hooks: () => noteValue('连接要改本机的 Claude Code 设置，试玩里不做'),
    settings_codex_hooks: () => noteValue('连接要改本机的 Codex 设置，试玩里不做'),
    settings_open_site: () => noteValue('打开网页在这里不做'),
    settings_update: () => noteValue('更新在这里不做'),
    settings_wallpaper: () => noteValue('换壁纸要在 Windows 上'),
    settings_login: () => noteValue('开机启动要在 Windows 上'),
  }
  // A point in her home's page, in the desktop's page.
  function toDesk(from, at) {
    const f = byLabel(from) || frames.island
    return { ...clone(at), x: f.rect[0] + at.x - frames.desk.rect[0], y: f.rect[1] + at.y - frames.desk.rect[1] }
  }
  const noteValue = text => {
    note(text)
    return { value: false }
  }

  window.WakuHost = {
    attach(label, win) {
      const f = byLabel(label)
      if (f) hook(f, win)
    },
    invoke(label, cmd, args) {
      const fn = ASK[cmd]
      if (!fn) return undefined
      try {
        return fn(label, clone(args) || {})
      } catch (err) {
        console.error(cmd, err)
        return undefined
      }
    },
  }

  // --- Her own menu (Windows': tray.rs menu), drawn here ----------------------------------
  const menuEl = $('#wk-menu')
  const MENUS = {
    taskbarHome: () => [
      ...(editing ? [['endEdit', '退出编辑'], '-'] : []),
      ['addWidgets', '添加小工具…'],
      ['dnd', '勿扰', !!world.config.dnd],
      ['settings', '设置…'],
      ['editTaskbar', '编辑任务栏布局'],
      '-',
      ['win', '任务管理器'],
      ['win', 'Windows 设置'],
      ['win', '控制面板'],
      ['win', '设备管理器'],
      '-',
      ['quit', '退出'],
    ],
    islandHome: () => [
      ['addWidgets', '添加小工具…'],
      ['editCapsule', '编辑胶囊'],
      '-',
      ['win', '宠物 ›'],
      ['win', '大小 ›'],
      ['walk', '空闲时走动', world.config.walk !== false],
      ['look', '眼睛跟着鼠标', world.config.look !== false],
      ['display:island', '她的家：灵动岛', world.home === 'island'],
      ['display:taskbar', '她的家：任务栏（代替 Windows 的）', world.home === 'taskbar'],
      ['out', '放她出来'],
      '-',
      ['dnd', '勿扰', !!world.config.dnd],
      ['settings', '设置…'],
      '-',
      ['quit', '退出'],
    ],
    volume: () => [['win', '声音问题疑难解答'], '-', ['win', '打开音量合成器'], ['win', '声音设置']],
    network: () => [['win', '诊断网络问题'], '-', ['win', '网络和 Internet 设置']],
    clock: () => [['win', '调整日期和时间'], ['win', '通知设置']],
  }

  function showMenu(part) {
    const items = (MENUS[part] || (world.home === 'taskbar' ? MENUS.taskbarHome : MENUS.islandHome))()
    menuEl.replaceChildren(
      ...items.map(it => {
        if (it === '-') return Object.assign(document.createElement('div'), { className: 'wk-sep' })
        const [act, text, checked] = it
        const b = document.createElement('button')
        b.type = 'button'
        b.className = 'wk-mi'
        b.dataset.act = act
        b.innerHTML = `<span class="wk-check">${checked ? '✓' : ''}</span><span></span>`
        b.lastChild.textContent = text
        return b
      }),
    )
    menuEl.dataset.mode = world.mode
    menuEl.hidden = false
    const [cx, cy] = toClient(...lastMenuAt)
    const r = menuEl.getBoundingClientRect()
    menuEl.style.left = `${Math.max(8, Math.min(cx, innerWidth - r.width - 8))}px`
    menuEl.style.top = `${Math.max(8, Math.min(cy - (world.home === 'taskbar' && world.edge === 'bottom' ? r.height : 0), innerHeight - r.height - 8))}px`
  }
  function hideMenu() {
    menuEl.hidden = true
  }
  menuEl.addEventListener('click', e => {
    const act = e.target.closest('[data-act]')?.dataset.act
    if (!act) return
    hideMenu()
    if (act === 'dnd' || act === 'walk' || act === 'look') {
      world.config[act] = act === 'dnd' ? !world.config.dnd : world.config[act] === false
      pushIsland()
    } else if (act === 'settings') {
      toIsland('island:settings', { tab: null })
      focusFrame(frames.island)
    } else if (act === 'editTaskbar') {
      toIsland('taskbar:edit', null)
      focusFrame(frames.island)
    } else if (act === 'addWidgets') {
      // On the taskbar: editing it, the picker open; at the top, the picker alone.
      if (world.home === 'taskbar') toIsland('taskbar:edit', { picker: true })
      else toIsland('island:picker', { open: 'picker' })
      focusFrame(frames.island)
    } else if (act === 'editCapsule') {
      toIsland('island:picker', { open: 'capsule' })
      focusFrame(frames.island)
    } else if (act === 'endEdit') toIsland('taskbar:edit', { end: true })
    else if (act.startsWith('display:')) setHome(act.slice(8))
    else if (act === 'quit') note('试玩里她不会退出')
    else note('这一项打开的是 Windows 自己的东西，试玩里没有')
  })
  document.addEventListener('pointerdown', e => !menuEl.contains(e.target) && !screenEl.contains(e.target) && hideMenu())
  document.addEventListener('keydown', e => e.key === 'Escape' && hideMenu())

  // --- What Windows draws: the windows' live pictures, the acrylic -----------------------
  const thumbsEl = $('#wk-thumbs')
  function drawThumbs(items) {
    const [fx, fy] = frames.island.rect
    thumbsEl.replaceChildren(
      ...items.map(([id, x, y, w, h]) => {
        const app = world.windows.find(a => a.windows.some(v => v.id === id))
        const win = app?.windows.find(v => v.id === id)
        const t = document.createElement('div')
        t.className = 'wk-thumb'
        Object.assign(t.style, { left: `${fx + x}px`, top: `${fy + y}px`, width: `${w}px`, height: `${h}px` })
        t.innerHTML = `<div class="wk-tbar"></div><div class="wk-tbody"><b></b><i></i><i></i><i></i></div>`
        t.querySelector('b').textContent = win?.title || app?.name || ''
        t.style.setProperty('--app', appColor(app))
        return t
      }),
    )
  }
  const APP_COLORS = { explorer: '#f2c23a', edge: '#1f9be0', terminal: '#3b3f4a', code: '#2f80ed', claude: '#d97757', notes: '#7a5af8' }
  const appColor = app => APP_COLORS[app?.app] || '#5e9bff'

  const plateEl = $('#wk-plate')
  function drawPlate(rect) {
    plateEl.hidden = !rect
    if (!rect) return
    const [fx, fy] = frames.island.rect
    Object.assign(plateEl.style, { left: `${fx + rect.x}px`, top: `${fy + rect.y}px`, width: `${rect.width}px`, height: `${rect.height}px` })
    plateEl.dataset.mode = rect.dark ? 'dark' : 'light'
  }

  // --- A word where Windows would act -----------------------------------------------------
  const toastEl = $('#wk-toast')
  let toastTimer
  function note(text) {
    toastEl.textContent = text
    toastEl.hidden = false
    clearTimeout(toastTimer)
    toastTimer = setTimeout(() => (toastEl.hidden = true), 2600)
  }

  // --- Home, look, motion ------------------------------------------------------------------
  function setHome(h, reload = true) {
    if (world.home === h) return
    world.home = h
    delete world.config.display
    editing = false
    drawThumbs([])
    drawPlate(null)
    if (reload) load(frames.island)
    pushDesk()
    drawPanel()
  }
  function setEdge(edge, reload = true) {
    world.edge = edge
    delete world.config.taskbarEdge
    if (reload && world.home === 'taskbar') load(frames.island)
    pushDesk()
    drawPanel()
  }
  function setMode(mode) {
    world.mode = mode
    viewerMode = mode
    document.documentElement.dataset.theme = mode
    pushAll()
  }

  // --- The monitor's readings, as widgets.rs sends them every 2 s ------------------------
  let cpu = 17
  setInterval(() => {
    if (world.quiet || document.hidden) return
    const mon = world.widgets.find(w => w.id === 'monitor')
    if (!mon) return
    cpu = Math.max(3, Math.min(96, Math.round(cpu + (Math.random() - 0.5) * 12)))
    const down = (2 + Math.random() * 40).toFixed(1)
    mon.value.parts = [
      { icon: 'cpu', text: `${cpu}%` },
      { icon: 'memory', text: '44%' },
      { icon: 'down', text: `${down}K` },
      { icon: 'up', text: `${Math.round(100 + Math.random() * 400)}K` },
    ]
    if (mon.detail?.series?.points) mon.detail.series.points = [...mon.detail.series.points.slice(1), cpu]
    if (mon.detail?.rows?.[0]) mon.detail.rows[0].value = `${cpu}%`
    frames.island.win?.Fake.emit('pet:widgets', { widgets: Fake.widgetsNow() })
    frames.desk.win?.Fake.emit('desk:widgets', { widgets: Fake.widgetsNow() })
  }, 2000)

  // --- The panel -----------------------------------------------------------------------------
  const MOODS = [
    ['working', '干活'],
    ['waiting', '等你'],
    ['done', '搞定'],
    ['review', '改好'],
    ['error', '出错'],
  ]
  const COLOR = { idle: '#8e8e93', working: '#5e9bff', waiting: '#ffb340', done: '#34d27b', review: '#b18cff', error: '#ff5c6c' }
  const panel = $('#wk-panel')
  const segs = {
    home: [
      ['taskbar', '任务栏'],
      ['island', '灵动岛'],
    ],
    mode: [
      ['light', '浅色'],
      ['dark', '深色'],
    ],
    material: [
      ['mica', 'Mica'],
      ['clear', '透明'],
      ['black', '纯色'],
    ],
    edge: [
      ['bottom', '下'],
      ['top', '上'],
      ['left', '左'],
      ['right', '右'],
    ],
    endCue: [
      ['open', '展开胶囊'],
      ['ring', '流光'],
      ['none', '只留小点'],
    ],
  }
  const segValue = {
    home: () => world.home,
    mode: () => world.mode,
    material: () => world.material,
    edge: () => world.edge,
    endCue: () => world.config.endCue || 'ring',
  }

  function drawPanel() {
    const scenesEl = panel.querySelector('#wk-scenes')
    if (!scenesEl.children.length) {
      for (const s of Scenes.SCENES) {
        const b = document.createElement('button')
        b.type = 'button'
        b.dataset.scene = s.id
        b.textContent = s.title
        scenesEl.append(b)
      }
    }
    for (const b of scenesEl.children) b.classList.toggle('on', b.dataset.scene === current)
    panel.querySelector('#wk-scene-note').textContent = Scenes.sceneById(current)?.note || ''
    for (const [key, options] of Object.entries(segs)) {
      const box = panel.querySelector(`[data-seg="${key}"]`)
      if (!box) continue
      if (!box.children.length) box.innerHTML = options.map(([v, label]) => `<button type="button" data-v="${v}">${label}</button>`).join('')
      for (const b of box.children) b.classList.toggle('on', b.dataset.v === segValue[key]())
    }
    panel.querySelector('[data-row="material"]').hidden = world.home !== 'taskbar'
    panel.querySelector('[data-row="edge"]').hidden = world.home !== 'taskbar'
    const list = panel.querySelector('#wk-sessions')
    list.replaceChildren(
      ...world.sessions.map(s => {
        const row = document.createElement('div')
        row.className = 'wk-srow'
        row.dataset.id = s.id
        row.innerHTML = `<div class="wk-sname"><i style="background:${COLOR[s.mood] || COLOR.idle}"></i><span></span><button type="button" class="wk-x" data-remove title="去掉这个会话">✕</button></div><div class="wk-moods">${MOODS.map(([m, label]) => `<button type="button" data-mood="${m}" class="${s.mood === m ? 'on' : ''}">${label}</button>`).join('')}</div>`
        row.querySelector('span').textContent = `${s.name} · ${s.project}${s.agent === 'codex' ? ' · Codex' : ''}`
        return row
      }),
    )
    panel.querySelector('[data-toggle="quiet"]').classList.toggle('on', !!world.quiet)
    panel.querySelector('[data-toggle="reduced"]').classList.toggle('on', world.reducedMotion === true)
  }

  panel.addEventListener('click', e => {
    const b = e.target.closest('button')
    if (!b) return
    if (b.dataset.scene) return applyScene(b.dataset.scene)
    const seg = b.closest('[data-seg]')?.dataset.seg
    if (seg) {
      const v = b.dataset.v
      if (seg === 'home') setHome(v)
      else if (seg === 'mode') setMode(v)
      else if (seg === 'material') {
        world.material = v
        delete world.config.taskbarMaterial
        pushAll()
      } else if (seg === 'edge') setEdge(v)
      else if (seg === 'endCue') {
        world.config.endCue = v
        pushAll()
      }
      return drawPanel()
    }
    const row = b.closest('.wk-srow')
    if (row) {
      const s = world.sessions.find(x => x.id === row.dataset.id)
      if (!s) return
      if (b.hasAttribute('data-remove')) {
        world.sessions = world.sessions.filter(x => x !== s)
        world.asks = world.asks.filter(a => a.session !== s.id)
      } else if (b.dataset.mood) {
        moodOf(s, b.dataset.mood)
        // The one something just happened to is the one shown.
        world.sessions = [s, ...world.sessions.filter(x => x !== s)]
      }
      return pushAll()
    }
    const act = b.dataset.act
    if (act === 'add') {
      if (world.sessions.length >= 6) return note('六个够多了')
      world.sessions = [newSession('working'), ...world.sessions]
      pushAll()
    } else if (act === 'hello') toIsland('pet:react', { react: 'wave', say: '你好呀' })
    else if (act === 'fail') toIsland('pet:react', { react: 'failed', say: 'Bash 失败了' })
    else if (act === 'nudge') toIsland('pet:nudge', { id: 'weather', words: '一小时后有雨，记得带伞', label: 'Dresden · 阴', value: '11°', private: false, open: null })
    else if (act === 'version') toIsland('pet:nudge', { id: 'update', icon: 'star', words: '有新版本 0.3.3', label: '点一下看看', value: '', private: false, open: { tab: 'update' } })
    else if (act === 'reset') applyScene(current)
    const toggle = b.dataset.toggle
    if (toggle === 'quiet') {
      world.quiet = !world.quiet
      toIsland('pet:quiet', { on: world.quiet })
      pushAll()
    } else if (toggle === 'reduced') {
      world.reducedMotion = world.reducedMotion === true ? false : true
      reloadFrames()
      drawPanel()
    }
  })

  $('#wk-fold')?.addEventListener('click', () => panel.classList.toggle('folded'))

  // --- Fitting the screen to the page --------------------------------------------------------
  function fit() {
    const width = viewport.clientWidth
    scale = Math.min(1, width / W)
    screenEl.style.transform = `scale(${scale})`
    viewport.style.height = `${Math.round(H * scale)}px`
  }
  new ResizeObserver(fit).observe(viewport)
  fit()

  // --- The shots' hooks (?demo=...), once both pages are up first ---------------------------
  let demoDone = false
  function demo() {
    const what = Q.get('demo')
    if (!what || demoDone) return
    demoDone = true
    const win = frames.island.win
    const doc = win?.document
    const after = (ms, fn) => setTimeout(fn, ms)
    const enter = sel => {
      const n = doc.querySelector(sel)
      if (!n) return
      const r = n.getBoundingClientRect()
      n.dispatchEvent(new win.MouseEvent('mouseenter', { clientX: r.left + 20, clientY: r.top + 10, bubbles: false }))
      return n
    }
    if (what === 'open') after(200, () => enter('#island'))
    if (what === 'card') {
      after(200, () => enter('#island'))
      after(700, () => doc.querySelector('#island-expanded [data-widget="weather"]')?.click())
    }
    if (what === 'done') {
      after(200, () => {
        const s = world.sessions[0]
        moodOf(s, 'done')
        pushAll()
      })
    }
    if (what === 'edit') {
      after(200, () => {
        lastMenuAt = [1000, H - 20]
        showMenu('her')
      })
      after(500, () => menuEl.querySelector('[data-act="editTaskbar"]')?.click())
      after(1100, () => doc.querySelector('[data-edit="add"]')?.click())
    }
    if (what === 'menu') {
      after(200, () => {
        lastMenuAt = [150, H - 30]
        showMenu('her')
      })
    }
    if (what === 'thumbs') after(200, () => doc.querySelector('#bar-rest [data-app="terminal"]')?.click())
    if (what === 'settings') after(200, () => toIsland('island:settings', { tab: 'now' }))
    // A turn done, its ring held 1.1 s in (headless Edge runs no animation on its own).
    if (what === 'ring') {
      after(200, () => {
        moodOf(world.sessions[0], 'done')
        pushAll()
      })
      after(350, () => frames.island.win.document.getElementById('flow')?.getAnimations({ subtree: true }).forEach(a => {
        a.pause()
        a.currentTime = 1100
      }))
    }
    // A prompt waiting, its card's buttons armed (0.6 s): ?demo=ask; then answered: ?demo=answer.
    if (what === 'ask' || what === 'answer') {
      after(200, () => {
        moodOf(world.sessions[0], 'waiting')
        pushAll()
      })
      if (what === 'answer') after(1200, () => doc.querySelector('#panel [data-action="allow"]')?.click())
    }
    // A run through what the pages ask of the host, then what went wrong, if anything.
    if (what === 'check') {
      const log = []
      const step = (ms, name, fn) =>
        after(ms, () => {
          try {
            fn()
            log.push(`${name}: ok`)
          } catch (err) {
            log.push(`${name}: ${err.message}`)
          }
        })
      step(200, 'edit the layout', () => toIsland('taskbar:edit', null))
      step(700, 'its library', () => frames.island.win.document.querySelector('[data-edit="add"]').click())
      step(1100, 'done', () => frames.island.win.document.querySelector('[data-edit="done"]').click())
      step(1500, 'a prompt', () => {
        moodOf(world.sessions[0], 'waiting')
        pushAll()
      })
      step(2400, 'allowed', () => frames.island.win.document.querySelector('#panel [data-action="allow"]').click())
      step(2800, 'a hello', () => toIsland('pet:react', { react: 'wave', say: '你好呀' }))
      step(3000, 'a plugin', () => toIsland('pet:nudge', { id: 'weather', words: '一小时后有雨', label: 'Dresden · 阴', value: '11°', private: false, open: null }))
      step(3200, 'settings', () => toIsland('island:settings', { tab: 'widgets' }))
      step(3600, 'light', () => setMode('light'))
      step(3800, 'island home', () => setHome('island'))
      step(5200, 'taskbar home', () => setHome('taskbar'))
      after(6600, () => {
        const errs = f => {
          try {
            return (f.win?.Fake.errors || []).join(' | ') || 'none'
          } catch (err) {
            return err.message
          }
        }
        const pre = document.createElement('pre')
        pre.style.cssText = 'position:fixed;left:0;top:0;right:0;z-index:99;margin:0;padding:6px;font:12px/1.4 Consolas,monospace;background:#000d;color:#fff;white-space:pre-wrap'
        const s = world.sessions[0]
        pre.textContent = [...log, `first session: ${s.name} ${s.mood}; asks ${world.asks.length}; layout kept: ${!!world.config.taskbarLayout}`, `errors: home ${errs(frames.island)}; desk ${errs(frames.desk)}; page ${pageErrors.join(' | ') || 'none'}`].join('\n')
        document.body.append(pre)
      })
    }
    // Which frame takes the pointer where (route), written over the page.
    if (what === 'probe')
      after(300, () => {
        const points = [
          ['capsule', 150, H - 24],
          ['strip, empty', 1200, H - 24],
          ['room above the strip', 800, 500],
          ['desktop stack', 120, 100],
          ['board', 200, 360],
          ['single widget', 1450, 90],
          ['wallpaper', 1300, 600],
        ]
        const lines = points.map(([name, x, y]) => {
          route(x, y)
          return `${name} (${x},${y}): home ${frames.island.el.style.pointerEvents}, desk ${frames.desk.el.style.pointerEvents}`
        })
        const pre = document.createElement('pre')
        pre.style.cssText = 'position:fixed;left:0;top:0;right:0;z-index:99;margin:0;padding:6px;font:12px/1.4 Consolas,monospace;background:#000d;color:#fff'
        pre.textContent = lines.join('\n')
        document.body.append(pre)
      })
  }

  // ?debug=1: how the frames stand, over the page after a moment.
  const pageErrors = []
  window.addEventListener('error', e => pageErrors.push(`${e.message} @ ${(e.filename || '').split('/').pop()}:${e.lineno}`))
  if (Q.get('debug'))
    setTimeout(() => {
      const pre = document.createElement('pre')
      pre.style.cssText = 'position:fixed;left:0;top:0;right:0;z-index:99;margin:0;padding:6px;font:11px/1.4 Consolas,monospace;background:#000c;color:#fff;white-space:pre-wrap'
      const about = f => {
        let doc = '?'
        try {
          doc = f.el.contentDocument ? `${f.el.contentDocument.readyState} ${f.el.contentDocument.body?.className || ''} errors: ${(f.el.contentWindow.Fake?.errors || []).join(' | ')}` : 'no document'
        } catch (err) {
          doc = `no access: ${err.message}`
        }
        return `${f.role}: src=${f.el.getAttribute('src')} attached=${!!f.win} ${doc}`
      }
      pre.textContent = [about(frames.island), about(frames.desk), `ready: ${[...ready].join(',')}`, `page errors: ${pageErrors.join(' | ')}`, `top: ${window.top === window}`].join('\n')
      document.body.append(pre)
    }, Number(Q.get('debug')) > 1 ? Number(Q.get('debug')) : 3000)

  // --- Go ----------------------------------------------------------------------------------
  // For checks from outside (a headless browser): what is shown, how often
  // the frames loaded and the stylesheets were swapped.
  window.WakuPlay = {
    world,
    scene: () => current,
    loads: () => loads,
    cssSwaps: () => cssSwaps,
    ready: () => bothUp(),
    apply: applyScene,
    reloadFrames,
  }
  const back = carried()
  if (back) restore(back)
  else applyScene(hashScene() || DEFAULT_SCENE, { overrides: true })
  if (window.WAKU_DEV) follow()
})()
