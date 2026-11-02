// Her taskbar on another display (bars.rs: taskbarDisplays all): Start, the
// windows on this display (as Windows' own setting for several displays
// says) and the clock, at the end the sliver that shows the desktop. Her
// capsule, the tray and the widgets are on the main display's alone (the
// appearance session's spec, 2026-10-13), but for a prompt waiting: then her
// capsule comes at its start, "waiting for your approval · N", a press
// showing the prompt in her island there. What it shows comes as bar:state
// (bars.rs refresh); a press asks what the main taskbar's would (taskbar.rs).
// Split (state.dock: taskbarEdge split, docs/design/specs/dock-topbar.md),
// the dock on her own display: Start and her strip's programs in a piece of
// glass in the middle of the foot, as wide as it asks its window to be
// (dock_size); quick settings and the clock after them if set so (state.keys);
// down out of the way of a window touching it (dock:stow), a line left.
;(function () {
  const { invoke } = window.__TAURI__.core
  const { listen } = window.__TAURI__.event
  const { t } = window.I18n
  const { CELL_W, CELL_H, CLIPS } = window.Sprite
  // For this window alone: listen() by itself hears what is sent to any (bridge.js).
  const LABEL = window.__TAURI_INTERNALS__?.metadata?.currentWebview?.label || window.__TAURI__.webviewWindow?.getCurrentWebviewWindow?.()?.label || ''
  const target = { kind: 'WebviewWindow', label: LABEL }

  const body = document.body
  const start = document.querySelector('.start')
  // Task View after Start, while Windows has its button on (as the main
  // taskbar's, island.js taskViewButton): Windows' own pictures, else its
  // icon font's glyph (bar.html).
  const taskView = document.querySelector('.tview')
  invoke('taskbar_task_view_icons')
    .then(icons => {
      if (!icons?.light || !icons?.dark) return
      const img = (svg, cls) => `<img class="${cls}" alt="" draggable="false" src="data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}">`
      taskView.innerHTML = img(icons.light, 'tv-light') + img(icons.dark, 'tv-dark')
    })
    .catch(() => {})
  const wins = document.getElementById('wins')
  const clock = document.querySelector('.clock')
  let state = { windows: [], look: {}, edge: 'bottom', align: 'center', material: 'clear', lang: 'zh' }

  const el = (tag, cls, text) => {
    const n = document.createElement(tag)
    if (cls) n.className = cls
    if (text != null) n.textContent = text
    return n
  }
  const isSide = () => state.edge === 'left' || state.edge === 'right'
  const isDock = () => state.dock === true
  const isLight = () => state.look?.mode === 'light'
  // How a clear strip fills in (taskbarClearWhen), as island.js reads it: as
  // Mica (the default) or solid, or never.
  const clearWhen = () => (['solid', 'always'].includes(state.clearWhen) ? state.clearWhen : 'mica')
  // Its ground: the material; clear, while a window is maximized or full
  // screen on this display (bars.rs fills) it fills in as Mica or solid,
  // unless it is to stay clear always, as hers does on hers (island.js isFilled).
  const ground = () => {
    // The dock is glass on the desktop's picture (Mica), clear or not; or solid.
    const material = isDock() ? (state.material === 'black' ? 'black' : 'mica') : state.material || 'clear'
    if (material !== 'clear' || !state.filled || clearWhen() === 'always') return material
    return clearWhen() === 'solid' ? 'black' : 'mica'
  }
  // Clear, filling in as Mica: its Mica drawn all along, unseen till then,
  // so that it fades in and out whole as hers does (style.css tb-clear-mica),
  // not its tint at once and the picture after.
  const clearMica = () => !isDock() && (state.material || 'clear') === 'clear' && clearWhen() === 'mica'

  // The Start mark, drawn on the screen's own pixels (as the main taskbar's, island.js drawStartMark).
  let startScale = 0
  function drawStart() {
    const dpr = devicePixelRatio || 1
    if (dpr === startScale) return
    startScale = dpr
    const box = Math.round(24 * dpr)
    const gap = Math.max(1, Math.round(0.8 * dpr))
    const pane = Math.floor((box - 1 - gap) / 2)
    const at = Math.floor((box - 2 * pane - gap) / 2)
    const far = at + pane + gap
    const end = far + pane
    const r = Math.max(0.5, 0.8 * dpr).toFixed(2)
    const rect = (x, y) => `<rect x="${x}" y="${y}" width="${pane}" height="${pane}" rx="${r}"/>`
    start.innerHTML = `<svg viewBox="0 0 ${box} ${box}" style="width:${box / dpr}px;height:${box / dpr}px"><defs><linearGradient id="start-blue" gradientUnits="userSpaceOnUse" x1="${at}" y1="${at}" x2="${end}" y2="${end}"><stop class="s0" offset="0"/><stop class="s1" offset="1"/></linearGradient></defs><g fill="url(#start-blue)">${rect(at, at)}${rect(far, at)}${rect(at, far)}${rect(far, far)}</g></svg>`
  }

  // As the main taskbar's (island.js tickClock): with the seconds while
  // Windows' own clock shows them, then anew at each second's start, else
  // every five seconds; called again when drawn, which sets the pace anew.
  let clockTick = 0
  function tickClock() {
    clearTimeout(clockTick)
    const seconds = state.look?.seconds === true
    clockTick = setTimeout(tickClock, seconds ? 1000 - (Date.now() % 1000) : 5000)
    const d = new Date()
    const zh = state.lang === 'zh' ? 'zh-CN' : undefined
    const time = d.toLocaleTimeString(zh, { hour: '2-digit', minute: '2-digit', second: seconds ? '2-digit' : undefined })
    const date = d.toLocaleDateString(zh, isSide() ? { month: 'numeric', day: 'numeric' } : { year: 'numeric', month: 'numeric', day: 'numeric' })
    if (clock.dataset.at !== time + date) {
      clock.dataset.at = time + date
      clock.replaceChildren(el('b', '', time), el('i', '', date))
    }
    const minute = `${date} ${d.getHours()}:${d.getMinutes()}`
    if (clock.dataset.tipAt !== minute) {
      clock.dataset.tipAt = minute
      tipClock()
    }
  }

  // The clock's tip and its right press as on the main taskbar
  // (zhoupengjie/Wakuwaku#36): the date in full and Windows' additional
  // clocks (clock.rs), read anew each minute and as the pointer comes; the
  // menu Windows' own taskbar has there (tray.rs part_menu).
  function tipClock() {
    invoke('taskbar_clock_tip')
      .then(tip => {
        if (typeof tip === 'string' && tip) clock.title = tip
      })
      .catch(() => {})
  }
  clock.addEventListener('pointerenter', tipClock)
  clock.addEventListener('contextmenu', () => invoke('pet_menu', { part: 'clock' }).catch(() => {}))

  // A button for each program, as the main taskbar's icons (island.js
  // drawWindows): its icon, a mark under it while running (longer in the
  // accent for the one in front), how many windows at its corner when more
  // than one. Its name in its tip only while it does not run: running, its
  // windows are listed when the pointer rests on it, and a tip stood over
  // the list's corner (the test session, 2026-10-13).
  function drawWindows() {
    const list = state.windows || []
    wins.replaceChildren(
      ...list.map(app => {
        const windows = app.windows || []
        const b = el('div', 'win')
        b.dataset.app = app.app
        b.classList.toggle('run', windows.length > 0)
        b.classList.toggle('front', windows.some(w => w.front))
        b.classList.toggle('flash', windows.some(w => w.flash))
        if (!windows.length) b.title = app.name || ''
        const img = el('img')
        img.src = app.png || ''
        img.alt = ''
        b.append(img)
        if (windows.length > 1) b.append(el('span', 'count', String(windows.length)))
        return b
      }),
    )
  }

  // Start and the programs centred on the whole strip: as much room before
  // them as the clock and the sliver take after (bar.css #lead).
  const end = document.getElementById('end')
  new ResizeObserver(() => {
    body.style.setProperty('--end-w', `${end.offsetWidth}px`)
    body.style.setProperty('--end-h', `${end.offsetHeight}px`)
  }).observe(end)

  // --- Her capsule, while a prompt waits ---------------------------------------
  // As her island on the main taskbar draws itself (island.js placeHer,
  // drawIslandLook): her round portrait playing her waiting clip, ringed in
  // the waiting colour, and the words in it; its ground her island's, as the
  // settings have it for Windows' mode (islandLook: bg, op, blur, edge,
  // bare), Windows' flyout grey by default; bare, none, its words haloed as
  // the clear strip's are; on the black strip, none either (it is black).
  const her = document.getElementById('her')
  const film = her.querySelector('.film')
  const sheet = her.querySelector('.sheet')
  const words = her.querySelector('.words')
  const WAITING = { dark: '#ffb340', light: '#b25e00' }
  const FACE = 30
  const PER_PX = 0.00875
  let frames = 0

  // A colour's relative lightness (WCAG), 0 to 1 (island.js lightness).
  function lightness(rgb) {
    const [r, g, b] = [(rgb >> 16) & 255, (rgb >> 8) & 255, rgb & 255].map(v => {
      v /= 255
      return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4
    })
    return 0.2126 * r + 0.7152 * g + 0.0722 * b
  }

  function drawHer() {
    const asks = state.asks
    // Not in the dock: her island on the top bar, over it, has the prompts.
    const on = !isDock() && !!asks && asks.n > 0
    if (her.hidden === on) her.hidden = !on
    body.classList.toggle('b-her', on)
    if (!on) {
      clearInterval(frames)
      frames = 0
      return
    }
    const mode = isLight() ? 'light' : 'dark'
    const capsule = isLight() || state.material !== 'black'
    const mine = (capsule && state.islandLook?.[mode]) || {}
    const bare = !capsule || (mine.bare ?? mode === 'dark') === true
    const bg = /^#[0-9a-f]{6}$/i.test(mine.bg || '') ? parseInt(mine.bg.slice(1), 16) : null
    const op = Number.isFinite(mine.op) ? Math.min(100, Math.max(20, mine.op)) / 100 : 1
    const blur = Number.isFinite(mine.blur) ? Math.min(60, Math.max(0, mine.blur)) : 0
    const inkLight = bare || bg === null ? isLight() : lightness(bg) > 0.35
    const ring = WAITING[inkLight ? 'light' : 'dark']
    her.classList.toggle('bare', bare)
    her.classList.toggle('ink-light', inkLight)
    her.style.setProperty('--ring', ring)
    her.style.setProperty('--h-bg', bg === null ? (isLight() ? '#f3f3f3' : '#2c2c2c') : `rgba(${(bg >> 16) & 255}, ${(bg >> 8) & 255}, ${bg & 255}, ${op})`)
    her.style.setProperty('--h-blur', blur ? `blur(${blur}px) saturate(1.25)` : 'none')
    const line = inkLight ? 'rgba(0, 0, 0, 0.08)' : 'rgba(255, 255, 255, 0.1)'
    const edge = { ring: `0 0 0 1.5px ${ring}, 0 0 12px color-mix(in srgb, ${ring} 35%, transparent)`, none: '0 0 0 0 transparent' }[mine.edge]
    her.style.setProperty('--h-edge', edge || `0 0 0 1px ${line}`)
    const said = t(state.lang, asks.kind === 'answer' ? 'bar.answer' : 'bar.approve', { n: asks.n })
    // Along a side, upright: her portrait and the number; the words in its tip.
    words.textContent = isSide() ? String(asks.n) : said
    her.title = said
    // Her sheet, scaled to the portrait and centred on her face.
    const scale = FACE * PER_PX
    film.style.backgroundImage = state.sprite ? `url("${state.sprite}")` : ''
    film.style.height = `${state.spriteVersion === 1 ? 1872 : 2288}px`
    sheet.style.transform = `translate(${-(CELL_W * scale - FACE) / 2}px, -2px) scale(${scale})`
    const clip = CLIPS.waiting
    const show = () => {
      if (!document.hidden) film.style.transform = `translate3d(${-(Math.floor(Date.now() / clip.ms) % clip.frames) * CELL_W}px, ${-clip.row * CELL_H}px, 0)`
    }
    show()
    if (!frames) frames = setInterval(show, clip.ms)
    // Room for it before Start and the buttons laid from the start.
    body.style.setProperty('--her-w', `${her.offsetWidth}px`)
    body.style.setProperty('--her-h', `${her.offsetHeight}px`)
  }

  function draw() {
    const edge = state.edge || 'bottom'
    for (const e of ['bottom', 'top', 'left', 'right']) body.classList.toggle(`b-${e}`, e === edge)
    body.classList.toggle('b-side', isSide())
    body.classList.toggle('b-light', isLight())
    for (const m of ['clear', 'black', 'mica']) body.classList.toggle(`b-${m}`, ground() === m)
    body.classList.toggle('b-clear-mica', clearMica())
    body.classList.toggle('b-start', state.align === 'left')
    body.classList.toggle('b-dock', isDock())
    body.classList.toggle('b-keys', isDock() && state.keys === true)
    quick.hidden = !(isDock() && state.keys === true)
    if (typeof state.stowed === 'boolean') body.classList.toggle('b-stowed', isDock() && state.stowed)
    if (state.keysNow) showKeys(state.keysNow)
    const accent = state.look?.accent
    if (accent) body.style.setProperty('--accent', isLight() ? accent.dark || accent.base : accent.light || accent.base)
    document.documentElement.lang = state.lang === 'zh' ? 'zh-CN' : 'en'
    drawStart()
    taskView.hidden = state.look?.taskView !== true
    taskView.title = t(state.lang, 'taskbar.taskView')
    drawWindows()
    tickClock()
    drawHer()
    drawMica()
  }

  // Mica: this display's own wallpaper under the strip, blurred and tinted
  // as the main strip's (mica.js; bars.rs read_papers sends where it lies);
  // its tint (style.css) until the picture comes.
  const mica = window.Mica.mount(document.getElementById('mica'))
  function drawMica() {
    const under = ground() === 'mica' || clearMica()
    if (under && state.paper) mica.set(state.paper, innerWidth, innerHeight, isLight() ? 'light' : 'dark')
    else if (!under) mica.clear()
  }

  // Presses: Start and the clock open what Windows' do, the sliver shows the
  // desktop; a program's button as on the main taskbar (island.js pressApp):
  // one not running starts, one window to the front or minimized, several
  // their list to pick from, pressed again it goes (zhoupengjie/Wakuwaku#34);
  // with the keys and the other buttons as on Windows' own
  // (zhoupengjie/Wakuwaku#36, presses.js pressOf).
  const appOf = key => (state.windows || []).find(a => a.app === key)
  // The window Ctrl and a press last brought (island.js cycled).
  let cycled = null
  // A window's own menu open: no list on the pointer resting (island.js menuUp).
  let menuUp = false
  function pressApp(node, app, what) {
    const windows = app.windows || []
    if (!what) return
    clearTimeout(popTimer)
    if (what === 'menu') return openPop(node, 'menu')
    if (what === 'launch' || what === 'admin') {
      closePop()
      return call('taskbar_app', { path: String(app.path), what, id: app.id ? String(app.id) : null })
    }
    if (what === 'windowMenu') {
      closePop()
      menuUp = true
      return Promise.resolve(call('taskbar_window_menu', { ids: windows.map(w => Number(w.id)) })).finally(() => (menuUp = false))
    }
    if (what === 'next') {
      closePop()
      const last = cycled?.app === app.app && Date.now() - cycled.at < window.Presses.CYCLED_MS ? cycled.id : null
      const next = window.Presses.nextWindow(windows, last)
      cycled = { app: app.app, id: next.id, at: Date.now() }
      return call('taskbar_window', { id: Number(next.id), what: 'front' })
    }
    if (windows.length > 1) {
      const listed = popFor?.kind === 'list' && popFor.app === app.app
      if (listed && popFor.pressed) return closePop()
      if (!listed) openPop(node, 'list')
      popFor.pressed = true
      return
    }
    closePop()
    if (!windows.length) return call('taskbar_app', { path: String(app.path), what: 'launch', id: app.id ? String(app.id) : null })
    call('taskbar_window', { id: Number(windows[0].id), what: 'press' })
  }
  // Shift and Ctrl as Windows has them at the press (island.js heldAtPress):
  // this window never takes the keyboard either.
  let heldAtPress = null
  document.addEventListener('pointerdown', e => {
    if (e.target.closest('[data-app]')) heldAtPress = invoke('taskbar_held').catch(() => null)
  })
  function pressAt(e, node) {
    const press = { type: e.type, button: e.button }
    const own = { shift: e.shiftKey, ctrl: e.ctrlKey }
    const held = heldAtPress
    heldAtPress = null
    Promise.resolve(held).then(keys => {
      const app = appOf(node.dataset.app)
      if (app) pressApp(node, app, window.Presses.pressOf(press, keys || own, (app.windows || []).length))
    })
  }
  document.addEventListener('click', e => {
    const node = e.target.closest('[data-app]')
    if (node) return pressAt(e, node)
    // Her capsule: the prompt shown in her island on the main display (bars.rs bar_asks).
    closePop()
    if (e.target.closest('#her')) return invoke('bar_asks').catch(() => {})
    const what = e.target.closest('[data-bar]')?.dataset.bar
    if (what) return invoke('taskbar_open', { what })
  })
  document.addEventListener('auxclick', e => {
    const node = e.target.closest('[data-app]')
    if (node && e.button === 1) pressAt(e, node)
  })
  // The middle button pressed: no scrolling by the pointer (Chromium's).
  document.addEventListener('mousedown', e => e.button === 1 && e.preventDefault())
  document.addEventListener('contextmenu', e => e.preventDefault())

  // Over a program's button, as on the main taskbar (island.js openPop): the
  // pointer resting on a running one, its windows; a right press, its menu.
  // In a window of their own over the strip (bars.rs bar_pop, barpop.js),
  // which goes once the pointer has left both a moment (told it here,
  // bar_pop_over) or at a press elsewhere. { app, kind } while open.
  let popFor = null
  let popTimer
  // Where the list's window lies (aimPop), in this page's px, while it shows.
  let popAt = null
  const call = (cmd, args) => invoke(cmd, args).catch(() => {})
  function openPop(node, kind) {
    const b = node.getBoundingClientRect()
    popFor = { app: node.dataset.app, kind }
    call('bar_pop', { kind, key: node.dataset.app, at: [b.left, b.top, b.width, b.height], dpr: devicePixelRatio || 1 })
  }
  function closePop() {
    clearTimeout(popTimer)
    popAt = null
    if (!popFor) return
    popFor = null
    call('barpop_hide')
  }
  // Another's list open: this button's only once the pointer has been on it
  // as long as Windows' own taskbar waits, and not while it is on its way
  // up into the list (aim.js, zhoupengjie/Wakuwaku#56), as on the main
  // taskbar (island.js aimPop). Where the list's window lies, in this
  // page's px, comes once it is laid (bars.rs barpop_place).
  const aim = window.Aim.create()
  listen('bar:pop-at', e => (popAt = e.payload || null), { target })
  const aimList = () => (popFor?.kind === 'list' && popAt ? { key: popFor.app, rect: popAt, edge: state.edge || 'bottom' } : null)
  function aimPop() {
    clearTimeout(popTimer)
    const key = aim.key()
    const node = key == null ? null : [...wins.querySelectorAll('[data-app]')].find(n => n.dataset.app === key)
    const app = node && appOf(key)
    if (!app || menuUp || popFor?.kind === 'menu' || !(app.windows || []).length) return
    const wait = aim.due(aimList(), performance.now())
    if (wait === null) return
    if (wait === 0) return openPop(node, 'list')
    popTimer = setTimeout(aimPop, wait)
  }
  wins.addEventListener('mousemove', e => aim.move(e.clientX, e.clientY, performance.now()))
  wins.addEventListener('mouseover', e => {
    const at = performance.now()
    aim.move(e.clientX, e.clientY, at)
    aim.over(e.target.closest('[data-app]')?.dataset.app ?? null, at)
    aimPop()
  })
  wins.addEventListener('mouseenter', () => call('bar_pop_over', { over: true }))
  wins.addEventListener('mouseleave', () => {
    clearTimeout(popTimer)
    aim.over(null, performance.now())
    call('bar_pop_over', { over: false })
  })
  // A right press: its menu; with Shift, its window's own (pressApp).
  wins.addEventListener('contextmenu', e => {
    const node = e.target.closest('[data-app]')
    if (!node) return
    e.preventDefault()
    clearTimeout(popTimer)
    pressAt(e, node)
  })
  // Gone by itself (the pointer away, a press elsewhere, its program gone).
  listen(
    'bar:pop-gone',
    () => {
      popFor = null
      popAt = null
    },
    { target },
  )

  // A file dragged from another program and held over a running program's
  // button half a second, as on the main taskbar (island.js fileHeld): its
  // window to the front, or with several their list open (barpop.js: a card
  // there held over as long brings that one). Tauri's drop handler has the
  // window's drags (tauri://drag-*, where it is in physical px), and the
  // page hears no mouse meanwhile: the list's window is told when the drag
  // is on the buttons or off them, as for the pointer (bar_pop_over).
  let fileHeld = null
  let fileTimer
  let fileOnWins = false
  for (const type of ['enter', 'over', 'leave', 'drop']) {
    listen(
      `tauri://drag-${type}`,
      ({ payload }) => {
        const p = type === 'enter' || type === 'over' ? payload?.position : null
        const node = p ? document.elementFromPoint(p.x / devicePixelRatio, p.y / devicePixelRatio) : null
        const onWins = !!node && wins.contains(node)
        if (onWins !== fileOnWins) {
          fileOnWins = onWins
          call('bar_pop_over', { over: onWins })
        }
        const button = onWins ? node.closest('[data-app]') : null
        if ((button?.dataset.app ?? null) === fileHeld) return
        fileHeld = button?.dataset.app ?? null
        clearTimeout(fileTimer)
        if (!button) return
        fileTimer = setTimeout(() => {
          const app = appOf(button.dataset.app)
          const windows = app?.windows || []
          if (windows.length === 1) {
            closePop()
            call('taskbar_window', { id: Number(windows[0].id), what: 'drag' })
          } else if (windows.length > 1 && !(popFor?.kind === 'list' && popFor.app === app.app)) {
            openPop(button, 'list')
          }
        }, window.Presses.DRAG_HOLD_MS)
      },
      { target },
    )
  }

  // --- The dock (split) ------------------------------------------------------------
  // As wide as Start and its programs (and quick settings and the clock),
  // asked of its window as they come and go (bars.rs dock_size).
  const strip = document.getElementById('bar')
  let askedW = 0
  new ResizeObserver(() => {
    if (!isDock()) return
    const w = Math.ceil(strip.offsetWidth)
    if (w === askedW) return
    askedW = w
    // Its Mica as wide as it (bar.css .b-dock #mica).
    body.style.setProperty('--dock-w', `${w}px`)
    call('dock_size', { w, dpr: devicePixelRatio || 1 })
  }).observe(strip)
  // Out of the way of a window touching it, and up again (bars.rs poll):
  // slid down off the screen, a line left at the foot (bar.css b-stowed).
  listen('dock:stow', e => body.classList.toggle('b-stowed', isDock() && e.payload?.stowed === true), { target })
  // Windows' Start menu open: Start lit, as the main taskbar's (island.js startButton).
  listen('dock:start', e => start.classList.toggle('open', e.payload?.open === true), { target })

  // Quick settings in the dock (state.keys): the network's way out and the
  // volume, as the main taskbar's button draws them (island.js showNet,
  // showVolume), from what Windows says (dock:keys, taskbar.rs keys_loop);
  // a press opens Windows' own (Win+A), the wheel on it the volume.
  const quick = document.querySelector('.quick')
  const [netBack, netFront] = quick.querySelectorAll('.net span')
  const [volBack, volFront] = quick.querySelectorAll('.vol span')
  const NET = { wired: '', wifi: '', other: '', none: '' }
  const WIFI = ['', '', '', '']
  function showKeys(k) {
    const kind = k?.net || 'other'
    const level = kind === 'wifi' && Number.isInteger(k?.bars) ? Math.min(Math.max(k.bars, 1), 4) : 0
    netFront.textContent = level ? WIFI[level - 1] : NET[kind] || NET.other
    netBack.textContent = level ? NET.wifi : ''
    netBack.hidden = !level
    const [vol, muted] = Array.isArray(k?.volume) ? k.volume : [null, false]
    volFront.textContent = muted ? '' : vol == null || vol >= 67 ? '' : vol >= 34 ? '' : vol > 0 ? '' : ''
    volBack.hidden = !!muted
    quick.title = t(state.lang, 'taskbar.quick')
  }
  listen('dock:keys', e => showKeys(e.payload), { target })
  let wheelGathered = 0
  quick.addEventListener(
    'wheel',
    e => {
      e.preventDefault()
      const dy = e.deltaMode === 1 ? e.deltaY * 33 : e.deltaY
      wheelGathered = Math.abs(dy) >= 50 ? Math.sign(dy) * 100 : wheelGathered + dy
      const steps = Math.trunc(wheelGathered / 100)
      if (!steps) return
      wheelGathered -= steps * 100
      call('taskbar_volume', { steps: -steps })
    },
    { passive: false },
  )

  tickClock()
  addEventListener('resize', () => {
    startScale = 0
    drawStart()
    drawMica()
  })
  listen('bar:state', e => {
    state = e.payload || state
    draw()
  }, { target }).then(() => invoke('bar_ready').catch(() => {}))
  draw()
})()
