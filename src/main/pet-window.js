// The pet's window: transparent, frameless, always on top, her whole self on
// the desktop. Where it sits and how big it is, the mouse (click-through, her
// eyes, dragging) and her walks. Up when she is the pet (display 'pet'), or
// when she is out of the island (display 'island' with out); in the island,
// the island has her (island-window.js).
const path = require('path')
const { app, BrowserWindow, ipcMain, screen } = require('electron')

const config = require('./config')
const { createClickThrough } = require('./click-through')
const { debugLog } = require('./debug-log')
const { isPrimaryDown } = require('./mouse-button')

// Window size at scale 1: room for the bubble above a 192x208 cell.
const BASE_W = 300
const BASE_H = 320
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

// ctx: what the app shares (settings, isVisible, pet, asks, change, payload,
// greet, tray, home, island, absorb).
function createPetWindow(ctx) {
  let win
  // Being moved by the pointer: dragged from her own page, or carried after
  // she was pulled out of the island (the island's page holds the button).
  let drag
  let walking
  // The prompt panel's size while one is up, as the page measured it.
  let panel = null
  // How far the pet sits right of the window's centre: when the screen edge
  // stops the grown window from centring on her, she shifts so she stays put.
  let shift = 0
  // Where the window was before a panel grew it, to go back to exactly.
  let parked = null

  const settings = () => ctx.settings
  const isIslandMode = () => settings().display === 'island'
  const isShown = () => ctx.isVisible() && (!isIslandMode() || settings().out === true)
  const mine = e => alive(win) && e.sender === win.webContents

  const pointer = createClickThrough({
    name: 'pet',
    getWin: () => alive(win),
    isShown,
    isHeld: () => !!drag,
    onPoll: lookAt,
  })

  // --- Size and place -----------------------------------------------------------

  function baseSize() {
    const { scale } = settings()
    return { width: Math.round(BASE_W * scale), height: Math.round(BASE_H * scale) }
  }

  // The window now: taller, and wider if need be, while a prompt panel is up.
  function size() {
    const base = baseSize()
    if (!panel) return base
    return { width: Math.max(base.width, panel.width), height: base.height + panel.height }
  }

  // The panel grows the window upwards and both ways, the pet staying put.
  function setPanel(next) {
    if (!alive(win)) return
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

  // Where she was last left, or home.
  function savedSpot() {
    const home = homeSpot()
    return clamp(settings().x ?? home.x, settings().y ?? home.y)
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
    const p = clamp(b.x, b.y)
    const isStretched = Math.abs(b.width - want.width) > 3 || Math.abs(b.height - want.height) > 3
    if (isStretched || p.x !== b.x || p.y !== b.y) remember(moveTo(p.x, p.y))
  }

  // Save where the window is, as where it would be without a prompt panel.
  function remember({ x, y }) {
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
    const p = homeSpot()
    remember(moveTo(p.x, p.y))
  }

  // Back to where she was left (out of the island, or the pet again).
  function returnToSpot() {
    if (!alive(win)) return
    panel = null
    parked = null
    setShift(0)
    const p = savedSpot()
    moveTo(p.x, p.y)
  }

  // A new size keeps the pet's spot: her bottom centre.
  function resize(patch) {
    const before = win.getBounds()
    const old = size()
    ctx.change(patch)
    const now = size()
    remember(moveTo(before.x + (old.width - now.width) / 2, before.y + old.height - now.height))
  }

  // Her middle on the screen, for the island to reach for.
  function herPoint() {
    const b = win.getBounds()
    const { width, height } = size()
    return { x: Math.round(b.x + width / 2 + shift), y: Math.round(b.y + height - (CELL_H * settings().scale) / 2) }
  }

  // --- The window -----------------------------------------------------------------

  function create() {
    const { width, height } = size()
    win = new BrowserWindow({
      ...savedSpot(),
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
    // Clicks pass through the transparent parts; see click-through.js.
    pointer.setMode('pass')
    win.webContents.on('console-message', e => debugLog('pet page:', e.message))
    win.webContents.setWindowOpenHandler(() => ({ action: 'deny' }))
    win.webContents.on('will-navigate', e => e.preventDefault())

    win.webContents.on('did-finish-load', () => {
      send()
      win.webContents.send('pet:asks', ctx.asks.views())
      applyVisibility()
      // A new window says hello, or what needs fixing; the island says it when it is the one up.
      if (!isIslandMode()) ctx.greet()
    })
    // The pet window gone means the app is done, main window or not.
    win.on('closed', () => {
      win = null
      app.quit()
    })
    win.loadFile(path.join(__dirname, '..', 'pet', 'index.html'), { query: { role: 'pet' } })

    setInterval(keepOnScreen, 2000)
    for (const event of ['display-added', 'display-removed', 'display-metrics-changed']) {
      screen.on(event, () => setTimeout(keepOnScreen, 500))
    }
    pointer.schedule()
  }

  function applyVisibility() {
    if (!alive(win)) return
    if (isShown()) {
      if (!win.isVisible()) win.showInactive()
    } else if (win.isVisible()) {
      if (drag) letGo()
      stopWalking()
      win.hide()
    }
    pointer.schedule()
  }

  // Everything the page draws from.
  function send() {
    alive(win)?.webContents.send('pet:update', ctx.payload())
  }

  // --- Her eyes on the cursor -----------------------------------------------------

  let lastCursor = { x: NaN, y: NaN }

  // Where the cursor is from the pet's face, for the 16 look directions.
  function cursorFromPet(cursor) {
    const { x, y } = win.getBounds()
    const { width, height } = size()
    const cellH = CELL_H * settings().scale
    return { dx: cursor.x - (x + width / 2 + shift), dy: cursor.y - (y + height - cellH * 0.62) }
  }

  // Her eyes follow only while idle, with no panel up.
  function lookAt(cursor) {
    if (cursor.x === lastCursor.x && cursor.y === lastCursor.y) return undefined
    lastCursor = cursor
    if (!settings().look || drag || panel || ctx.pet.get().mood !== 'idle') return undefined
    const at = cursorFromPet(cursor)
    if (Math.hypot(at.dx, at.dy) > LOOK_FAR) return undefined
    win.webContents.send('pet:cursor', at)
    return 80
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

  // --- Dragging and carrying ------------------------------------------------------

  // Follow the cursor from main, so a fast flick that outruns the page's own
  // mouse events still carries the window. The page hears which way it moves,
  // to run that way; the island hears where she is, to reach for her.
  function follow(grab, { carried }) {
    debugLog('pet', 'follow', carried ? 'carried' : 'dragged')
    stopWalking()
    // Only ever one drag: a new one replaces whatever was still held.
    if (drag) clearInterval(drag.timer)
    const { x, y } = win.getBounds()
    drag = { ...grab, x, y, moved: carried, carried, timer: undefined }
    drag.timer = setInterval(() => {
      // Carried, the button is held on the island's window and its release
      // may reach neither page: the button's own state says when to let go.
      if (drag.carried && isPrimaryDown() === false) return letGo()
      const p = screen.getCursorScreenPoint()
      const nx = p.x - drag.dx
      const ny = p.y - drag.dy
      if (nx === drag.x && ny === drag.y) return
      drag.moved = drag.moved || Math.abs(nx - x) + Math.abs(ny - y) > 4
      // Free while held; put back on screen when let go.
      moveTo(nx, ny, { isFree: true })
      if (drag.moved) win.webContents.send('pet:drag', nx - drag.x)
      drag.x = nx
      drag.y = ny
      if (isIslandMode() && drag.moved) ctx.island.reach(herPoint())
    }, 16)
  }

  // Let go: back into the island if she is close enough to it, else she lands.
  function letGo() {
    debugLog('pet', 'let go', drag ? (drag.carried ? 'carried' : 'dragged') : 'nothing held')
    if (!drag) return
    clearInterval(drag.timer)
    const { x, y, moved, carried } = drag
    drag = undefined
    parked = null
    win.webContents.send('pet:drag-end')
    if (carried) ctx.island.endHold()
    if (isIslandMode() && moved) {
      if (ctx.island.reach(herPoint(), { final: true }) === 'snap') return ctx.absorb(herPoint())
      ctx.island.reach(null)
    }
    remember(moveTo(x, y))
    // A click: a jump. Carried out of the island: she lands with one.
    if (!moved || carried) ctx.pet.apply({ react: 'jump' })
  }

  // Pulled out of the island, the button still held on the island: up she
  // comes under the cursor, her middle where the drop had it, and follows it.
  function carry(offset) {
    if (!alive(win)) return
    const cursor = screen.getCursorScreenPoint()
    panel = null
    setShift(0)
    const { width, height } = size()
    const x = cursor.x + (Number(offset?.dx) || 0) - width / 2
    const y = cursor.y + (Number(offset?.dy) || 0) - (height - (CELL_H * settings().scale) / 2)
    moveTo(x, y, { isFree: true })
    ctx.change({ out: true })
    win.moveTop()
    pointer.reset()
    follow({ dx: cursor.x - x, dy: cursor.y - y }, { carried: true })
  }

  // Gone into the island at once, before the island's animation starts.
  function hideNow() {
    if (drag) {
      clearInterval(drag.timer)
      drag = undefined
    }
    stopWalking()
    alive(win)?.hide()
    pointer.reset()
  }

  // --- From her page ---------------------------------------------------------------

  ipcMain.on('pet:hover', (e, isOver) => {
    if (!mine(e)) return
    pointer.setOver(isOver)
    // The pointer on her: you have seen what she had to say.
    if (isOver) ctx.pet.seen()
  })

  // Typing an answer needs the keyboard, which the window does not take otherwise.
  ipcMain.on('pet:keyboard', (e, needs) => {
    if (!mine(e)) return
    win.setFocusable(needs === true)
    if (needs === true) win.focus()
  })

  ipcMain.on('pet:drag-start', e => {
    debugLog('pet', 'drag-start from', mine(e) ? 'pet page' : 'another page')
    if (!mine(e)) return
    const { x, y } = win.getBounds()
    const cursor = screen.getCursorScreenPoint()
    follow({ dx: cursor.x - x, dy: cursor.y - y }, { carried: false })
  })

  // The button went up over her: that ends a carry as well (the island may
  // have lost the button on the way).
  ipcMain.on('pet:drag-end', e => {
    if (!mine(e)) return
    debugLog('pet', 'drag-end from pet page', drag?.carried ? '(carried)' : '')
    letGo()
  })

  ipcMain.on('pet:menu', e => mine(e) && ctx.tray.popUp(win))
  ipcMain.on('pet:answer', (e, id, choice) => mine(e) && ctx.asks.answer(id, choice))
  ipcMain.on('pet:dismiss', (e, id) => mine(e) && ctx.asks.dismiss(id))
  ipcMain.on('pet:panel', (e, measured) => {
    if (!mine(e)) return
    const next =
      measured && measured.width > 0 && measured.height > 0
        ? { width: Math.ceil(measured.width), height: Math.ceil(measured.height) }
        : null
    if (JSON.stringify(next) !== JSON.stringify(panel)) setPanel(next)
  })
  ipcMain.on('pet:open-settings', e => mine(e) && ctx.home.open())
  ipcMain.handle('pet:walk', (e, dx, ms) => (mine(e) && isShown() ? walk(dx, ms) : false))
  ipcMain.on('pet:walk-stop', e => mine(e) && stopWalking())

  // The window's own spot, for checks: where it is, and the size it should be.
  function where() {
    const bounds = alive(win)?.getBounds()
    const area = bounds && screen.getDisplayMatching(bounds).workArea
    return { bounds, size: size(), workArea: area, shift, isVisible: !!alive(win)?.isVisible() }
  }

  return {
    create,
    window: () => alive(win),
    handle: () => (alive(win) ? handleOf(win) : null),
    send,
    resize,
    comeHome,
    returnToSpot,
    applyVisibility,
    carry,
    drop: letGo,
    hideNow,
    walk,
    where,
  }
}

module.exports = { createPetWindow, alive, handleOf }
