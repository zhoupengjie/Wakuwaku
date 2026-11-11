// Her taskbar on another display (bars.rs: taskbarDisplays all): Start, the
// windows on this display (as Windows' own setting for several displays
// says) and the clock, at the end the sliver that shows the desktop. Her
// capsule, the tray and the widgets are on the main display's alone (the
// appearance session's spec, 2026-10-13), but for a prompt waiting: then her
// capsule comes at its start, "waiting for your approval · N", a press
// showing the prompt in her island there. What it shows comes as bar:state
// (bars.rs refresh); a press asks what the main taskbar's would (taskbar/).
// Split (state.dock: taskbarEdge split, docs/design/specs/dock-topbar.md),
// the dock on her own display: Start and her strip's programs in a piece of
// glass in the middle of the foot, its window a band along the foot that
// never moves, the glass growing and shrinking in it (dock_glass), Windows'
// acrylic on a plate under it (state.acrylic); after the programs the
// windows minimized into it and the Recycle Bin, as macOS has them; quick
// settings and the clock after them if set so (state.keys). Out of the way
// of a window touching it, the glass slides down in it (dock:stow).
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
  // taskbar's, island-right.js taskViewButton): Windows' own pictures, else its
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
    // The dock is glass: Windows' acrylic, or the desktop's picture (Mica), clear or not; or solid.
    const material = isDock() ? (state.material === 'black' ? 'black' : 'mica') : state.material || 'clear'
    if (material !== 'clear' || !state.filled || clearWhen() === 'always') return material
    return clearWhen() === 'solid' ? 'black' : 'mica'
  }
  // Clear, filling in as Mica: its Mica drawn all along, unseen till then,
  // so that it fades in and out whole as hers does (style.css tb-clear-mica),
  // not its tint at once and the picture after.
  const clearMica = () => !isDock() && (state.material || 'clear') === 'clear' && clearWhen() === 'mica'

  // The Start mark, drawn on the screen's own pixels (as the main taskbar's, island-right.js drawStartMark).
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

  // As the main taskbar's (island-look.js tickClock): with the seconds while
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
  // Each program's button kept from one drawing to the next (by its key),
  // what changed changed in it, as on the main taskbar (island.js
  // drawWindows): its line springs between a dash and the pill of the one
  // in front, a new one's grows in (fresh), a press plays out though the
  // windows change under it (presses.js pressIn). In the dock a program
  // started grows into its place and one ended shrinks away, the glass
  // with them, as its minimized windows do (moveMin): it jumped narrower
  // as the window brought back from it was closed (the user, 2026-11-04).
  let buttonsDrawn = false
  function drawWindows() {
    const list = state.windows || []
    const old = new Map([...wins.children].filter(b => b.dataset.app != null).map(b => [b.dataset.app, b]))
    const fresh = []
    const animate = isDock() && buttonsDrawn
    const buttons = list.map(app => {
      const windows = app.windows || []
      let b = old.get(app.app)
      old.delete(app.app)
      if (!b) {
        b = el('div', 'win')
        b.dataset.app = app.app
        const img = el('img')
        img.alt = ''
        img.draggable = false
        b.append(img)
        if (buttonsDrawn) {
          b.classList.add('fresh')
          fresh.push(b)
        }
      }
      b.classList.toggle('run', windows.length > 0)
      b.classList.toggle('front', windows.some(w => w.front))
      b.classList.toggle('flash', windows.some(w => w.flash))
      if (!windows.length) b.title = app.name || ''
      else b.removeAttribute('title')
      const img = b.querySelector(':scope > img')
      if (img.getAttribute('src') !== (app.png || '')) img.src = app.png || ''
      let count = b.querySelector(':scope > .count')
      if (windows.length > 1) {
        if (!count) {
          count = el('span', 'count')
          b.append(count)
        }
        if (count.textContent !== String(windows.length)) count.textContent = String(windows.length)
      } else count?.remove()
      return b
    })
    for (const b of old.values()) {
      if (!animate) {
        b.remove()
        continue
      }
      // Gone: no longer pressed or pointed at as it goes.
      delete b.dataset.app
      b.classList.add('leaving')
      if (glideOn === b) glideOff()
      moveMin(b, false).then(() => b.remove())
    }
    // In their order, each moved only where it is not already; those
    // going stay where they are till gone.
    let at = wins.firstElementChild
    for (const b of buttons) {
      while (at?.classList.contains('leaving')) at = at.nextElementSibling
      if (at === b) at = at.nextElementSibling
      else wins.insertBefore(b, at)
    }
    if (fresh.length) requestAnimationFrame(() => requestAnimationFrame(() => fresh.forEach(b => b.classList.remove('fresh'))))
    if (animate) for (const b of fresh) moveMin(b, true)
    buttonsDrawn = true
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

  // A colour's relative lightness (WCAG), 0 to 1 (island-look.js lightness).
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
    const acrylicWas = body.classList.contains('b-acrylic')
    body.classList.toggle('b-acrylic', isDock() && state.acrylic === true)
    body.classList.toggle('b-down', isDock() && down)
    if (state.keysNow) showKeys(state.keysNow)
    const accent = state.look?.accent
    if (accent) body.style.setProperty('--accent', isLight() ? accent.dark || accent.base : accent.light || accent.base)
    document.documentElement.lang = state.lang === 'zh' ? 'zh-CN' : 'en'
    drawStart()
    taskView.hidden = state.look?.taskView !== true
    taskView.title = t(state.lang, 'taskbar.taskView')
    drawWindows()
    drawMins()
    drawBin(state.bin)
    // Laid out anew: where its glass goes (the acrylic come or gone, or the
    // mode changed, its plate set again once it is still).
    if (isDock()) {
      if (acrylicWas !== body.classList.contains('b-acrylic') || isLight() !== wasLight) {
        settled = null
        plateUp = false
      }
      wasLight = isLight()
      glassMoves()
    }
    tickClock()
    drawHer()
    drawMica()
  }

  // Mica: this display's own wallpaper under the strip, blurred and tinted
  // as the main strip's (mica.js; bars.rs read_papers sends where it lies);
  // its tint (style.css) until the picture comes.
  const mica = window.Mica.mount(document.getElementById('mica'))
  function drawMica() {
    // Windows' acrylic under the dock: no picture of its own.
    const under = (ground() === 'mica' || clearMica()) && !(isDock() && state.acrylic === true)
    if (under && state.paper) mica.set(state.paper, innerWidth, innerHeight, isLight() ? 'light' : 'dark')
    else if (!under) mica.clear()
  }

  // Presses: Start and the clock open what Windows' do, the sliver shows the
  // desktop; a program's button as on the main taskbar (island-winlist.js pressApp):
  // one not running starts, one window to the front or minimized, several
  // their list to pick from, pressed again it goes (zhoupengjie/Wakuwaku#34);
  // with the keys and the other buttons as on Windows' own
  // (zhoupengjie/Wakuwaku#36, presses.js pressOf).
  const appOf = key => (state.windows || []).find(a => a.app === key)
  // The window Ctrl and a press last brought (island-winlist.js cycled).
  let cycled = null
  // A window's own menu open: no list on the pointer resting (island-winlist.js menuUp).
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
  // Shift and Ctrl as Windows has them at the press (island-winlist.js heldAtPress):
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
    // A window minimized into the dock: back, to the front (drawMins).
    const mini = e.target.closest('[data-min]')
    if (mini) return call('taskbar_window', { id: Number(mini.dataset.min), what: 'front' })
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
  // A right press: never the browser's menu. On the dock, off its programs,
  // its minimized windows, the clock, the Recycle Bin and her, the menu her
  // taskbar has (tray.rs Over::Dock), with 占着位置 checked while the dock
  // keeps its room (splitDock room; the user, 2026-11-08: 对着dock右键 …
  // 跟任务栏保持一致，加上「占着位置」).
  document.addEventListener('contextmenu', e => {
    e.preventDefault()
    if (!isDock() || e.target.closest('[data-app], #mins, .bin, #her') || clock.contains(e.target)) return
    invoke('pet_menu', { part: null }).catch(() => {})
  })

  // Over a program's button, as on the main taskbar (island-glide.js openPop): the
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
  // taskbar (island-winlist.js aimPop). Where the list's window lies, in this
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
  // button half a second, as on the main taskbar (island-winlist.js fileHeld): its
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
        // Let go of on it: programs and shortcuts to them pinned (taskbar/mod.rs taskbar_pin_files).
        if (type === 'drop' && Array.isArray(payload?.paths) && payload.paths.length && payload.position) {
          const on = document.elementFromPoint(payload.position.x / devicePixelRatio, payload.position.y / devicePixelRatio)
          if (on && strip.contains(on)) call('taskbar_pin_files', { paths: payload.paths.map(String) })
        }
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
  // Its window a band along the whole foot that stays as it is while it is
  // up (bars.rs dock_band), as her island's does: in its middle the glass
  // (#bar: Start and its programs, quick settings and the clock), as wide as
  // they are, growing and shrinking here alone, with what is in it, in the
  // same frame. The window takes the pointer only where the glass is, letting
  // it through elsewhere (dock_glass: hit; bars.rs poll), and Windows'
  // acrylic lies on a plate under it, where the glass settles (plate). While it changes size the plate stays under
  // the smaller of where it was and where it goes (a frame or two behind: so
  // the glass, #glass, does not shrink inside it before it has gone, LAG_MS),
  // the glass beyond it as solid as the plate is not there (b-firm, the hole
  // the plate: --hole-l, --hole-r); once still a moment (SETTLE_MS), the plate
  // where the glass is and the ground back, over FIRM_MS (bar.css). Without
  // the acrylic, the desktop's picture (#mica) is cut to the same. Out of the
  // way of a window touching it (dock:stow), the glass slides down in the
  // band but for its top (b-down), solid, its plate off meanwhile. Moved and
  // sized a frame at a time instead, its page came a frame behind each time
  // and shook (the user, 2026-11-04). Ready once its programs are in: its
  // window shows then, all at once (bars.rs reveal), not with Start alone.
  const strip = document.getElementById('bar')
  const glassEl = document.getElementById('glass')
  const micaEl = document.getElementById('mica')
  const SETTLE_MS = 80
  const LAG_MS = 60
  const SLIDE_MS = 180
  const PLATE_IN_MS = 150
  // DWM's corners on the plate (8 px) inside the glass's (8): a pixel in
  // (backplate.rs; island-plate.js plateInset).
  const PLATE_INSET = 1
  // Where the glass is as it last settled (its px), where the plate goes
  // meanwhile and where it may still be (its last but one, LAG_MS), whether
  // the glass is down (till bars.rs says: it comes up from there).
  let settled = null
  let plateAt = null
  let plateWas = null
  let lagTimer = null
  let settleTimer = null
  let sliding = null
  let down = true
  // Windows' acrylic wholly in under the glass (bars.rs plate_in), and the
  // wait for it as the glass settles; with none to come (no acrylic), as if in.
  let plateUp = false
  let plateWait = null
  const plateReady = () => state.acrylic !== true || plateUp
  let wasLight = false
  let glassSent = ''
  const boxOf = () => {
    const r = strip.getBoundingClientRect()
    return { x: r.left, y: strip.offsetTop, w: r.width, h: r.height }
  }
  const meet = (a, b) => {
    const x = Math.max(a.x, b.x)
    const y = Math.max(a.y, b.y)
    return { x, y, w: Math.max(0, Math.min(a.x + a.w, b.x + b.w) - x), h: Math.max(0, Math.min(a.y + a.h, b.y + b.h) - y) }
  }
  const join = (a, b) => {
    const x = Math.min(a.x, b.x)
    const y = Math.min(a.y, b.y)
    return { x, y, w: Math.max(a.x + a.w, b.x + b.w) - x, h: Math.max(a.y + a.h, b.y + b.h) - y }
  }
  const within = (a, b) => a.x >= b.x - 0.5 && a.x + a.w <= b.x + b.w + 0.5
  const same = (a, b) => (a && b ? Math.abs(a.x - b.x) < 0.5 && Math.abs(a.w - b.w) < 0.5 && Math.abs(a.y - b.y) < 0.5 && Math.abs(a.h - b.h) < 0.5 : a === b)
  const arr = b => [b.x, b.y, b.w, b.h].map(v => Math.round(v * 100) / 100)

  // The glass drawn round what is in it (now) and over where the plate may
  // still be; the firm ground's hole and the picture's cut where the plate goes.
  function drawGlass(now) {
    const g = plateWas ? join(now, plateWas) : now
    glassEl.style.left = `${g.x - now.x}px`
    glassEl.style.width = `${g.w}px`
    // The hole only over a plate that is there: opened before, the glass
    // stood clear while the plate came (plateCame).
    const hole = plateAt && !down && plateReady() ? plateAt : null
    // From the firm layer's own edge, a pixel in from the glass's (bar.css).
    body.style.setProperty('--hole-l', `${hole ? hole.x - g.x - 1 : 0}px`)
    body.style.setProperty('--hole-r', `${hole ? hole.x + hole.w - g.x - 1 : 0}px`)
    micaEl.style.clipPath = hole ? `inset(${hole.y}px ${innerWidth - hole.x - hole.w}px ${innerHeight - hole.y - hole.h}px ${hole.x}px round 8px)` : 'inset(50%)'
    return g
  }

  function sendGlass(g, plate, fade) {
    const p = plate && state.acrylic === true && plate.w > 2 * PLATE_INSET && plate.h > 2 * PLATE_INSET ? { x: plate.x + PLATE_INSET, y: plate.y + PLATE_INSET, w: plate.w - 2 * PLATE_INSET, h: plate.h - 2 * PLATE_INSET } : null
    // Pressed and drawn down to the window's foot: the gap under the glass
    // is its own, and so is its top, all that is left of it out of the way.
    const hit = { x: g.x, y: g.y, w: g.w, h: Math.max(g.h, innerHeight - g.y) }
    const msg = { glass: arr(g), hit: arr(hit), plate: p && arr(p), fade, dark: !isLight(), dpr: devicePixelRatio || 1, ready: (state.windows || []).length > 0 }
    const sent = JSON.stringify({ ...msg, fade: 0 })
    if (sent === glassSent) return
    glassSent = sent
    call('dock_glass', msg)
  }

  // Its glass changed size, or is about to (to: where it goes, should it be
  // narrower than now).
  function glassMoves(to) {
    if (!isDock()) return
    const now = boxOf()
    clearTimeout(plateWait)
    plateWait = null
    if (down || sliding || !settled) {
      drawGlass(now)
      sendGlass(now, null, 0)
      return settleSoon()
    }
    const goes = to ? meet(to, now) : now
    const next = meet(plateAt || settled, goes)
    if (!same(next, plateAt)) {
      // Shrinking, the glass kept over where the plate is until it has gone.
      if (plateAt && !within(plateAt, next)) {
        plateWas = plateWas ? join(plateWas, plateAt) : plateAt
        clearTimeout(lagTimer)
        lagTimer = setTimeout(() => {
          plateWas = null
          if (isDock()) sendGlass(drawGlass(boxOf()), plateAt, 0)
        }, LAG_MS)
      }
      plateAt = next
    }
    body.classList.add('b-firm')
    sendGlass(drawGlass(now), plateAt, 0)
    settleSoon()
  }

  function settleSoon() {
    clearTimeout(settleTimer)
    settleTimer = setTimeout(settle, SETTLE_MS)
  }

  // Still: the plate where the glass is, the ground back (b-firm off), its
  // minimized windows' pictures where their rooms are.
  function settle() {
    if (!isDock() || down || sliding || minMoving > 0) return
    const now = boxOf()
    settled = plateAt = now
    plateWas = null
    clearTimeout(lagTimer)
    sendGlass(drawGlass(now), now, PLATE_IN_MS)
    // Solid till the plate is in under it (dock:plate), then the ground
    // fades out over it; should it never say, after a second all the same.
    clearTimeout(plateWait)
    plateWait = null
    if (plateReady()) body.classList.remove('b-firm')
    else plateWait = setTimeout(plateCame, 1000)
    sendMins()
  }

  // The plate wholly in under the glass (or waited for long enough): the
  // ground fades out over it. The hole is cut once that has faded (it only
  // shows with b-firm, as the glass next moves).
  function plateCame() {
    if (down) return
    plateUp = true
    if (!plateWait) return
    clearTimeout(plateWait)
    plateWait = null
    if (!isDock() || sliding || !settled) return
    body.classList.remove('b-firm')
  }
  listen('dock:plate', () => plateCame(), { target })

  new ResizeObserver(() => {
    glassMoves()
    // The highlight kept on its button as the glass changes size.
    if (glideOn?.isConnected) glideTo(glideOn, true)
  }).observe(strip)

  // --- The pointer's glide (as the main taskbar's: island-glide.js glideTo) ---------------
  // One highlight under the pointer for all its buttons, behind them,
  // sliding from one to the next, its leading edge first and the trailing
  // one after, so it stretches and gathers, a little squashed on the way
  // (bar.css .hdrop; the user, 2026-11-04: "as on her taskbar"). It comes
  // and goes in place, jumps rather than slides a long way, and gives a
  // little under a press. The buttons' own hover washes give way to it.
  // Not on a minimized window's: its picture is Windows' own, over the
  // page, and the wash showed round it as a lighter block (the user,
  // 2026-11-08: 窗口第一次最小化到dock后，背景会有一个高亮亮块).
  const GLIDE_ON = '.start, .tview, .win:not(.mini), .quick, .clock'
  const GLIDE_FAR = 360
  const glide = el('span', 'hdrop')
  strip.insertBefore(glide, glassEl.nextSibling)
  let glideOn = null
  let glideTimer

  function glideTo(node, still = false) {
    clearTimeout(glideTimer)
    const side = isSide()
    const bar = strip.getBoundingClientRect()
    const r = node.getBoundingClientRect()
    // Along the strip its two ends slide (left and right, or a side
    // strip's top and bottom); across it, as the button is.
    const [from, to, crossFrom, crossSize] = side ? ['top', 'bottom', 'left', 'width'] : ['left', 'right', 'top', 'height']
    const lo = b => (side ? b.top : b.left)
    const hi = b => (side ? b.bottom : b.right)
    const left = lo(r) - lo(bar)
    const right = hi(bar) - hi(r)
    const was = glideOn && glide.classList.contains('on') && glide.dataset.axis === from ? parseFloat(glide.style[from]) : null
    if (glide.dataset.axis !== from) {
      for (const p of ['left', 'right', 'top', 'bottom', 'width', 'height']) glide.style[p] = ''
      glide.dataset.axis = from
    }
    glide.style[crossFrom] = `${side ? r.left - bar.left : r.top - bar.top}px`
    glide.style[crossSize] = `${side ? r.width : r.height}px`
    if (still || was === null || Math.abs(left - was) > GLIDE_FAR) {
      glide.style.transition = 'none'
      glide.style[from] = `${left}px`
      glide.style[to] = `${right}px`
      void glide.offsetWidth
      glide.style.transition = ''
    } else if (left !== was) {
      const lead = left > was ? to : from
      const trail = lead === to ? from : to
      glide.style.transition = `${lead} 0.22s cubic-bezier(0.3, 1.3, 0.5, 1), ${trail} 0.42s cubic-bezier(0.3, 1.45, 0.5, 1), opacity 0.16s ease, transform 0.3s cubic-bezier(0.3, 1.45, 0.5, 1)`
      glide.style[from] = `${left}px`
      glide.style[to] = `${right}px`
      glide.classList.remove('squash')
      void glide.offsetWidth
      glide.classList.add('squash')
    }
    glide.classList.add('on')
    glideOn = node
  }

  function glideOff() {
    clearTimeout(glideTimer)
    glideTimer = setTimeout(() => {
      glide.classList.remove('on', 'down')
      glideOn = null
    }, 90)
  }

  const isGlideOn = node => node?.matches?.(GLIDE_ON) && !node.hidden
  strip.addEventListener('mouseenter', e => isGlideOn(e.target) && glideTo(e.target), true)
  strip.addEventListener('mouseleave', e => (e.target === strip || isGlideOn(e.target)) && glideOff(), true)
  strip.addEventListener('pointerdown', e => {
    if (e.button === 0 && glideOn && e.target.closest(GLIDE_ON) === glideOn) glide.classList.add('down')
  })
  addEventListener('pointerup', () => glide.classList.remove('down'))
  // Its buttons pressed in and springing back (presses.js pressIn; not a
  // minimized window's, whose picture is Windows' own over it).
  window.Presses.pressIn(strip, '.start, .tview, .win:not(.mini)')
  addEventListener('resize', () => {
    settled = null
    glassMoves()
  })

  // Out of the way, or up (bars.rs tell_stow): it slides down or up, solid
  // and its plate off meanwhile; up, its plate under it once it is still.
  listen(
    'dock:stow',
    e => {
      const stowed = e.payload?.stowed === true
      down = stowed
      plateAt = plateWas = settled = null
      // Off as it goes down, and none yet as it comes up.
      plateUp = false
      clearTimeout(plateWait)
      plateWait = null
      clearTimeout(lagTimer)
      clearTimeout(sliding)
      body.classList.add('b-firm')
      body.classList.toggle('b-down', stowed)
      sending.mins = ''
      call('dock_thumbs', { items: [], dpr: devicePixelRatio || 1 })
      // Up and still: its plate under it at once.
      sliding = setTimeout(() => {
        sliding = null
        if (down) glassMoves()
        else settle()
      }, SLIDE_MS + 20)
      glassMoves()
    },
    { target },
  )
  // Windows' Start menu open: Start lit, as the main taskbar's (island-right.js startButton).
  listen('dock:start', e => start.classList.toggle('open', e.payload?.open === true), { target })

  // Quick settings in the dock (state.keys): the network's way out and the
  // volume, as the main taskbar's button draws them (island-right.js showNet,
  // showVolume), from what Windows says (dock:keys, taskbar/win/keys.rs keys_loop);
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

  // The windows minimized into the dock (split), as macOS has them: each a
  // room for its live picture (Windows draws it there: bars.rs dock_thumbs),
  // its program's icon at its corner, its title in its tip; a press brings
  // it back. One minimized grows into its place, the glass widening with it
  // (glassMoves, each frame); one brought back shrinks away (the user,
  // 2026-11-02: they came and went at once), the plate told at once where
  // the glass goes. The live pictures off meanwhile, back where the rooms
  // are once it is still (settle).
  const mins = document.getElementById('mins')
  const minSep = document.getElementById('minsep')
  const MIN_W = 50
  const MIN_MS = 220
  let minMoving = 0
  function drawMins() {
    const list = isDock() ? (state.windows || []).flatMap(app => (app.windows || []).filter(w => w.min).map(w => ({ app, w }))) : []
    const drawn = JSON.stringify(list.map(({ app, w }) => [w.id, w.title, app.png]))
    if (drawn === mins.dataset.drawn) return
    const first = mins.dataset.drawn === undefined
    mins.dataset.drawn = drawn
    const want = new Set(list.map(({ w }) => String(w.id)))
    for (const b of [...mins.querySelectorAll('[data-min]')]) {
      if (want.has(b.dataset.min)) continue
      // Brought back: no longer pressed as it goes; its picture shrinks with it.
      b.dataset.going = b.dataset.min
      delete b.dataset.min
      moveMin(b, false).then(() => {
        b.remove()
        minSep.hidden = !mins.querySelector('[data-min]')
      })
    }
    let after = null
    for (const { app, w } of list) {
      let b = mins.querySelector(`[data-min="${CSS.escape(String(w.id))}"]`)
      if (!b) {
        b = el('div', 'win mini')
        b.dataset.min = w.id
        b.append(el('span', 'room'), Object.assign(el('img', 'badge'), { alt: '', draggable: false }))
        if (after) after.after(b)
        else mins.prepend(b)
        if (!first) moveMin(b, true)
      }
      b.title = w.title || app.name || ''
      const badge = b.querySelector('.badge')
      if (badge.getAttribute('src') !== (app.png || '')) badge.src = app.png || ''
      after = b
    }
    if (list.length) minSep.hidden = false
    else if (!mins.querySelector('.mini')) minSep.hidden = true
    sendMins()
  }
  // Grown in from nothing, or shrunk away (a minimized window, or a
  // program's button); the live pictures following their rooms meanwhile
  // (follow). Its own width (a program's 40, a minimized window's 50), and
  // the gap beside it going with it, so nothing jumps as it is taken out at
  // the end. Going, the glass as it will be without it, first (glassMoves).
  function moveMin(b, coming) {
    minMoving++
    follow()
    if (!coming) {
      const was = b.style.display
      b.style.display = 'none'
      const to = boxOf()
      b.style.display = was
      glassMoves(to)
    }
    const gap = parseFloat(getComputedStyle(b.parentElement).columnGap) || 0
    const w = b.getBoundingClientRect().width || MIN_W
    const out = { width: '0px', marginRight: `${-gap}px`, opacity: 0, transform: 'scale(0.5)' }
    const shown = { width: `${w}px`, marginRight: '0px', opacity: 1, transform: 'none' }
    const quiet = matchMedia('(prefers-reduced-motion: reduce)').matches
    const a = b.animate(coming ? [out, shown] : [shown, out], { duration: quiet ? 1 : MIN_MS, easing: 'cubic-bezier(0.2, 0.8, 0.2, 1)', fill: 'forwards' })
    return a.finished
      .catch(() => {})
      .then(() => {
        if (coming) a.cancel()
        if (--minMoving === 0) settleSoon()
      })
  }
  // Where their rooms are, once it is up and still (settle), as they change;
  // and where each one's program's icon is, drawn over its picture there
  // (bars.rs dock_badges: the picture is Windows', over the page).
  const sending = { mins: '' }
  function roomsNow() {
    return [...mins.querySelectorAll('[data-min], [data-going]')].map(b => {
      const r = b.querySelector('.room').getBoundingClientRect()
      const i = b.querySelector('.badge').getBoundingClientRect()
      return [Number(b.dataset.min ?? b.dataset.going), r.left, r.top, r.width, r.height, i.width > 0 ? [i.left, i.top, i.width, i.height] : null]
    })
  }
  function sendRooms(items) {
    const sent = JSON.stringify(items)
    if (sent === sending.mins) return
    sending.mins = sent
    call('dock_thumbs', { items, dpr: devicePixelRatio || 1 })
  }
  function sendMins() {
    if (!isDock() || minMoving > 0 || down || sliding || !settled) return
    requestAnimationFrame(() => sendRooms(roomsNow()))
  }
  // While one grows in or shrinks away, the pictures follow their rooms at
  // every frame. They were taken off for it and put back once the glass was
  // still: each went grey and came back, every time a window went into the
  // dock or came out (the user, 2026-11-08: 每次最小化到dock和点开后，dock的
  // 略缩图都会闪灰黑一下然后又出现略缩图).
  let following = false
  function follow() {
    if (following) return
    following = true
    const step = () => {
      if (minMoving === 0 || !isDock()) {
        following = false
        return
      }
      if (!down && !sliding) sendRooms(roomsNow())
      // The wash under the pointer kept on its button as the buttons move
      // (it stayed where the button had been, a block beside it).
      if (glideOn?.isConnected && glide.classList.contains('on')) glideTo(glideOn, true)
      requestAnimationFrame(step)
    }
    requestAnimationFrame(step)
  }

  // The Recycle Bin, last in the dock (split): Windows' own picture of it,
  // empty or full (bars.rs look_at_bin); a press opens it.
  const bin = document.querySelector('.bin')
  const binSep = document.getElementById('binsep')
  function drawBin(b) {
    bin.hidden = binSep.hidden = !isDock()
    if (!isDock()) return
    const img = bin.querySelector('img')
    if (b?.png && img.getAttribute('src') !== b.png) img.src = b.png
    img.hidden = !b?.png
    bin.classList.toggle('full', b?.full === true)
    bin.title = t(state.lang, b?.full ? 'dock.binFull' : 'dock.bin')
  }
  // A right press on it: its menu, as Windows' own (open it, empty it; tray.rs part_menu).
  bin.addEventListener('contextmenu', e => {
    e.preventDefault()
    call('pet_menu', { part: 'recycleBin' })
  })
  listen(
    'dock:bin',
    e => {
      state.bin = e.payload
      drawBin(state.bin)
    },
    { target },
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
