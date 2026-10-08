// The pet's window: transparent, frameless, always on top. Where it sits and
// how big it is (the pet, or the island at the top of the screen), when it
// shows, the mouse (click-through, her eyes, dragging) and her walks.
const path = require('path')
const { app, BrowserWindow, ipcMain, screen } = require('electron')

const config = require('./config')
const pets = require('./pets')
const { hooksStatus } = require('./connection')

// Window size at scale 1: room for the bubble above a 192x208 cell. The
// island's window is fixed: room for the island at its widest without a
// prompt (hovered, with a second session beside it), and for its spring.
const BASE_W = 300
const BASE_H = 320
const ISLAND = { width: 460, height: 132 }
const CELL_H = 208
// The cursor further than this from her face does not catch her eye.
const LOOK_FAR = 900

// A window still there to talk to: closed ones throw on any call.
function alive(w) {
  return w && !w.isDestroyed() ? w : null
}

function handleOf(w) {
  const b = w.getNativeWindowHandle()
  return b.length >= 8 ? b.readBigUInt64LE(0) : BigInt(b.readUInt32LE(0))
}

// ctx: what the app shares (settings, lang, pet, asks, change, tray, home).
function createPetWindow(ctx) {
  let win
  let drag
  let walking
  // The prompt panel's size while one is up, as the page measured it.
  let panel = null
  // How far the pet sits right of the window's centre: when the screen edge
  // stops the grown window from centring on her, she shifts so she stays put.
  let shift = 0
  // Where the window was before a panel grew it, to go back to exactly.
  let parked = null
  // Out of sight: hidden from the tray, or another app is full screen.
  let isUserHidden = false
  let isFullscreen = false

  const settings = () => ctx.settings
  const isIsland = () => settings().display === 'island'

  // --- Visibility -------------------------------------------------------------

  function isVisible() {
    const s = settings()
    return !isUserHidden && !s.dnd && !(s.hideInFullscreen && isFullscreen)
  }

  function applyVisibility() {
    if (!alive(win)) return
    if (isVisible()) {
      if (!win.isVisible()) win.showInactive()
    } else {
      if (win.isVisible()) win.hide()
      // Out of sight, nobody can answer here: the terminal has them.
      ctx.asks.dismissAll()
    }
    ctx.tray.refresh()
    scheduleCursor()
  }

  function setHidden(isHidden) {
    isUserHidden = isHidden
    applyVisibility()
  }

  function setFullscreen(isFull) {
    isFullscreen = isFull
    applyVisibility()
  }

  // --- Size and place -----------------------------------------------------------

  // The window without a prompt: the bubble and the pet, or the island.
  function baseSize() {
    if (isIsland()) return { ...ISLAND }
    const { scale } = settings()
    return { width: Math.round(BASE_W * scale), height: Math.round(BASE_H * scale) }
  }

  // The window now: taller, and wider if need be, while a prompt panel is up.
  function size() {
    const base = baseSize()
    if (!panel) return base
    // The island holds the prompt itself; the page says how much room it takes.
    if (isIsland()) return { width: Math.max(base.width, panel.width), height: Math.max(base.height, panel.height) }
    return { width: Math.max(base.width, panel.width), height: base.height + panel.height }
  }

  // The panel grows the window upwards and both ways, the pet staying put.
  function setPanel(next) {
    if (!alive(win)) return
    if (isIsland()) {
      panel = next
      const p = islandSpot()
      return moveTo(p.x, p.y)
    }
    const old = win.getBounds()
    const before = size()
    const petX = old.x + before.width / 2 + shift
    const petBottom = old.y + before.height
    if (!panel && next) parked = { x: old.x, y: old.y }
    panel = next

    if (!panel && parked) {
      moveTo(parked.x, parked.y)
      parked = null
      setShift(0)
      return
    }

    const after = size()
    const wantX = petX - after.width / 2
    const p = moveTo(wantX, petBottom - after.height)
    setShift(panel ? Math.round(wantX - p.x) : 0)
  }

  function setShift(px) {
    shift = px
    alive(win)?.webContents.send('pet:shift', shift)
  }

  // Keep the window on some display's work area.
  function clamp(x, y) {
    const { width, height } = size()
    const area = screen.getDisplayNearestPoint({ x: x + width / 2, y: y + height / 2 }).workArea
    return {
      x: Math.round(Math.min(Math.max(x, area.x), area.x + area.width - width)),
      y: Math.round(Math.min(Math.max(y, area.y), area.y + area.height - height)),
    }
  }

  // Bottom right of the primary display.
  function homeSpot() {
    const { width, height } = size()
    const area = screen.getPrimaryDisplay().workArea
    return { x: area.x + area.width - width - 24, y: area.y + area.height - height - 24 }
  }

  // The island hangs at the top centre of the display she is on (the primary
  // one to start with), below a taskbar that sits at the top.
  function islandSpot() {
    const { width } = size()
    const b = alive(win)?.getBounds()
    const display = b ? screen.getDisplayNearestPoint({ x: b.x + b.width / 2, y: b.y + b.height / 2 }) : screen.getPrimaryDisplay()
    const area = display.workArea
    return { x: Math.round(area.x + (area.width - width) / 2), y: area.y }
  }

  // Every move goes through here, size and all. On Windows at a fractional
  // display scale, setPosition on a transparent window rounds the width up by a
  // pixel each call; a walk makes hundreds of calls and the window, the pet at
  // its bottom centre, creeps off the screen. setBounds with our own size holds.
  function moveTo(x, y, { isFree = false } = {}) {
    const p = isFree ? { x: Math.round(x), y: Math.round(y) } : clamp(x, y)
    win.setBounds({ ...p, ...size() })
    return p
  }

  // Put the window back on screen at its own size if anything moved it off or
  // stretched it (a display unplugged, the scale changed, a drift).
  function keepOnScreen() {
    if (!alive(win) || drag || walking) return
    const b = win.getBounds()
    const want = size()
    const p = isIsland() ? islandSpot() : clamp(b.x, b.y)
    const isStretched = Math.abs(b.width - want.width) > 3 || Math.abs(b.height - want.height) > 3
    if (isStretched || p.x !== b.x || p.y !== b.y) {
      remember(moveTo(p.x, p.y))
    }
  }

  // Save where the window is, as where it would be without a prompt panel.
  function remember({ x, y }) {
    // The island has its own place; the pet's spot stays for when she is back.
    if (isIsland()) return
    const now = size()
    const base = baseSize()
    Object.assign(settings(), {
      x: Math.round(x + now.width / 2 + shift - base.width / 2),
      y: Math.round(y + now.height - base.height),
    })
    config.save(settings())
  }

  function comeHome() {
    stopWalking()
    isUserHidden = false
    applyVisibility()
    const p = isIsland() ? islandSpot() : homeSpot()
    remember(moveTo(p.x, p.y))
  }

  // A new size keeps the pet's spot: her bottom centre. Into the island, the
  // window goes to the top of the screen; out of it, back to where she was.
  function resize(patch) {
    const before = win.getBounds()
    const old = size()
    const wasIsland = isIsland()
    ctx.change(patch)
    const now = size()
    if (isIsland()) {
      const p = islandSpot()
      return moveTo(p.x, p.y)
    }
    if (wasIsland) {
      const home = homeSpot()
      return remember(moveTo(settings().x ?? home.x, settings().y ?? home.y))
    }
    remember(moveTo(before.x + (old.width - now.width) / 2, before.y + old.height - now.height))
  }

  // --- The window -----------------------------------------------------------------

  function create() {
    const { width, height } = size()
    const home = homeSpot()
    const start = clamp(settings().x ?? home.x, settings().y ?? home.y)
    if (isIsland()) {
      const area = screen.getPrimaryDisplay().workArea
      Object.assign(start, { x: Math.round(area.x + (area.width - width) / 2), y: area.y })
    }

    win = new BrowserWindow({
      ...start,
      width,
      height,
      show: false,
      transparent: true,
      frame: false,
      resizable: false,
      maximizable: false,
      fullscreenable: false,
      skipTaskbar: true,
      hasShadow: false,
      alwaysOnTop: true,
      focusable: false,
      backgroundColor: '#00000000',
      webPreferences: {
        preload: path.join(__dirname, '..', 'pet', 'preload.js'),
        contextIsolation: true,
        nodeIntegration: false,
        // An unfocused, half-transparent window still has to animate.
        backgroundThrottling: false,
      },
    })
    win.setAlwaysOnTop(true, 'floating')
    win.setVisibleOnAllWorkspaces(true)
    // Clicks pass through the transparent parts; see setMouseMode.
    setMouseMode('pass')
    win.webContents.on('console-message', e => console.log(`[page] ${e.message}`))
    win.webContents.setWindowOpenHandler(() => ({ action: 'deny' }))
    win.webContents.on('will-navigate', e => e.preventDefault())

    let isFirstLoad = true
    win.webContents.on('did-finish-load', () => {
      send()
      win.webContents.send('pet:asks', ctx.asks.views())
      if (isVisible()) win.showInactive()
      // A new window says hello, or what needs fixing.
      if (isFirstLoad) {
        const isStale = hooksStatus(ctx.port) === 'stale'
        ctx.pet.apply({ react: 'wave', say: { key: isStale ? 'say.hooksStale' : 'say.arrived' } })
      }
      isFirstLoad = false
    })
    // The pet window gone means the app is done, main window or not.
    win.on('closed', () => {
      win = null
      app.quit()
    })
    win.loadFile(path.join(__dirname, '..', 'pet', 'index.html'))

    setInterval(keepOnScreen, 2000)
    for (const event of ['display-added', 'display-removed', 'display-metrics-changed']) {
      screen.on(event, () => setTimeout(keepOnScreen, 500))
    }
    scheduleCursor()
  }

  // Everything the page draws from.
  function send() {
    const s = pets.sprite(settings().pet)
    alive(win)?.webContents.send('pet:update', {
      ...ctx.pet.get(),
      config: settings(),
      lang: ctx.lang(),
      sprite: s && s.url,
      spriteVersion: s ? s.version : 2,
      // The next session that wants something, for the island's second bubble.
      second: ctx.pet.list().filter(x => x.mood !== 'idle')[1]?.mood || null,
    })
  }

  // --- The mouse: click-through, and her eyes ----------------------------------
  //
  // The window lets clicks through its transparent parts. To still hear the
  // pointer arrive over her, Electron forwards mouse moves, which on Windows is
  // a system-wide mouse hook: every move anywhere passes through this process.
  // So forwarding is on only while the cursor is near the window; a poll of
  // the cursor (cheap) decides, and also feeds her eyes.
  const NEAR_PX = 24

  let isOverPet = false
  let mouseMode = ''
  let lastCursor = { x: NaN, y: NaN }
  let cursorTimer

  function setMouseMode(mode) {
    if (!alive(win) || mode === mouseMode) return
    mouseMode = mode
    if (mode === 'catch') win.setIgnoreMouseEvents(false)
    else if (mode === 'forward') win.setIgnoreMouseEvents(true, { forward: true })
    else win.setIgnoreMouseEvents(true)
  }

  function isNear(cursor) {
    const b = win.getBounds()
    return cursor.x >= b.x - NEAR_PX && cursor.x <= b.x + b.width + NEAR_PX && cursor.y >= b.y - NEAR_PX && cursor.y <= b.y + b.height + NEAR_PX
  }

  // Where the cursor is from the pet's face, for the 16 look directions.
  function cursorFromPet(cursor) {
    const { x, y } = win.getBounds()
    const { width, height } = size()
    const cellH = CELL_H * settings().scale
    return { dx: cursor.x - (x + width / 2 + shift), dy: cursor.y - (y + height - cellH * 0.62) }
  }

  function scheduleCursor(ms = 0) {
    clearTimeout(cursorTimer)
    cursorTimer = setTimeout(pollCursor, ms)
  }

  function pollCursor() {
    if (!alive(win)) return
    if (!isVisible()) {
      setMouseMode('pass')
      return scheduleCursor(500)
    }
    const cursor = screen.getCursorScreenPoint()
    const near = isNear(cursor)
    setMouseMode(drag || isOverPet ? 'catch' : near ? 'forward' : 'pass')

    // Her eyes follow only while idle, with no panel up.
    const canLook = settings().look && !isIsland() && !drag && !panel && ctx.pet.get().mood === 'idle'
    if (cursor.x === lastCursor.x && cursor.y === lastCursor.y) return scheduleCursor(near ? 80 : 200)
    lastCursor = cursor
    if (canLook) {
      const at = cursorFromPet(cursor)
      if (Math.hypot(at.dx, at.dy) <= LOOK_FAR) {
        win.webContents.send('pet:cursor', at)
        return scheduleCursor(80)
      }
    }
    scheduleCursor(near ? 80 : 200)
  }

  // --- Walks --------------------------------------------------------------------

  // A short walk: move the window by dx over ms, stopping at the screen edge.
  // Resolves true when it got there, false when stopped on the way.
  let stopWalk = () => {}

  function stopWalking() {
    stopWalk()
  }

  function walk(dx, ms) {
    stopWalking()
    const { x: x0, y: y0 } = win.getBounds()
    const target = clamp(x0 + dx, y0).x
    const started = Date.now()

    return new Promise(resolve => {
      let x = x0
      const finish = isThere => {
        clearInterval(walking)
        walking = undefined
        stopWalk = () => {}
        remember({ x, y: y0 })
        resolve(isThere)
      }
      stopWalk = () => finish(false)
      walking = setInterval(() => {
        if (drag) return finish(false)
        const t = Math.min(1, (Date.now() - started) / ms)
        x = moveTo(x0 + (target - x0) * t, y0).x
        if (t >= 1) finish(true)
      }, 16)
    })
  }

  // --- From the page: pointer, dragging, walking, the prompt panel ----------------

  ipcMain.on('pet:hover', (_, isOver) => {
    isOverPet = isOver === true
    if (!drag) setMouseMode(isOverPet ? 'catch' : 'forward')
    // The pointer on her: you have seen what she had to say. The island opens
    // up to say it while hovered, so it counts once the pointer leaves.
    if (isIsland() ? !isOver : isOver) ctx.pet.seen()
  })

  // Typing an answer needs the keyboard, which the window does not take otherwise.
  ipcMain.on('pet:keyboard', (_, needs) => {
    if (!alive(win)) return
    win.setFocusable(needs === true)
    if (needs === true) win.focus()
  })

  // Drag by following the cursor from main, so a fast flick that outruns the
  // page's own mouse events still carries the window. The page hears which way
  // it moves, to run that way.
  ipcMain.on('pet:drag-start', () => {
    stopWalking()
    // The island stays where it hangs: a press is only ever a click.
    if (isIsland()) {
      drag = { isIsland: true, moved: false }
      return
    }
    const { x, y } = win.getBounds()
    const cursor = screen.getCursorScreenPoint()
    drag = { dx: cursor.x - x, dy: cursor.y - y, x, y, moved: false, timer: undefined }
    drag.timer = setInterval(() => {
      const p = screen.getCursorScreenPoint()
      const nx = p.x - drag.dx
      const ny = p.y - drag.dy
      if (nx !== drag.x || ny !== drag.y) {
        drag.moved = drag.moved || Math.abs(nx - x) + Math.abs(ny - y) > 4
        // Free while held; put back on screen when let go.
        moveTo(nx, ny, { isFree: true })
        if (drag.moved) win.webContents.send('pet:drag', nx - drag.x)
        drag.x = nx
        drag.y = ny
      }
    }, 16)
  })

  ipcMain.on('pet:drag-end', () => {
    if (!drag) return
    if (drag.isIsland) {
      drag = undefined
      return ctx.home.open()
    }
    clearInterval(drag.timer)
    const wasClick = !drag.moved
    const { x, y } = drag
    drag = undefined
    parked = null
    remember(moveTo(x, y))
    win.webContents.send('pet:drag-end')
    if (wasClick) ctx.pet.apply({ react: 'jump' })
  })

  ipcMain.on('pet:menu', () => ctx.tray.popUp(win))
  ipcMain.on('pet:answer', (_, id, choice) => ctx.asks.answer(id, choice))
  ipcMain.on('pet:dismiss', (_, id) => ctx.asks.dismiss(id))
  ipcMain.on('pet:panel', (_, measured) => {
    const next =
      measured && measured.width > 0 && measured.height > 0
        ? { width: Math.ceil(measured.width), height: Math.ceil(measured.height) }
        : null
    if (JSON.stringify(next) !== JSON.stringify(panel)) setPanel(next)
  })
  ipcMain.on('pet:open-settings', () => ctx.home.open())
  ipcMain.handle('pet:walk', (_, dx, ms) => walk(dx, ms))
  ipcMain.on('pet:walk-stop', () => stopWalking())

  // The window's own spot, for checks: where it is, and the size it should be.
  function where() {
    const bounds = alive(win)?.getBounds()
    const area = bounds && screen.getDisplayMatching(bounds).workArea
    return { bounds, size: size(), workArea: area, shift, isVisible: isVisible() }
  }

  return {
    create,
    window: () => alive(win),
    handle: () => (alive(win) ? handleOf(win) : null),
    send,
    resize,
    comeHome,
    isVisible,
    setHidden,
    setFullscreen,
    applyVisibility,
    walk,
    where,
  }
}

module.exports = { createPetWindow, alive, handleOf }
