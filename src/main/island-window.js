// The island's window: a black pill at the top centre of the screen (the page
// draws it, island.js), up whenever display is 'island', whether she is in it
// or out on the desktop. It is her home: pulled out of it, she becomes the
// pet window (carried under the cursor while the button is still held on the
// island); brought close to it again, it reaches for her and takes her back.
const path = require('path')
const { BrowserWindow, ipcMain, screen } = require('electron')

const { createClickThrough } = require('./click-through')
const { debugLog } = require('./debug-log')
const { alive, handleOf } = require('./pet-window')

// The window without extra room: the island at its widest without a prompt
// (hovered, with a second session beside it), and its spring. The page asks
// for more (pet:panel) when a shape needs it.
const ISLAND = { width: 460, height: 132 }
// The island's top edge in the window, and its compact height.
const TOP = 8
const COMPACT_H = 36
// Her middle this close to the island: it reaches out for her; this close,
// letting go puts her back in.
const REACH_PX = 280
const SNAP_PX = 140

// ctx: what the app shares (settings, isVisible, pet, asks, payload, greet,
// tray, home, letOut, petWindow).
function createIslandWindow(ctx) {
  let win = null
  // Extra room the page asked for, or null.
  let room = null
  let isReaching = false
  // The button held on her (a pull, or carrying her out): the window keeps the pointer.
  let isHolding = false

  const settings = () => ctx.settings
  const isShown = () => ctx.isVisible() && settings().display === 'island'
  const mine = e => alive(win) && e.sender === win.webContents

  const pointer = createClickThrough({ name: 'island', getWin: () => alive(win), isShown, isHeld: () => isHolding })

  function size() {
    return room ? { width: Math.max(ISLAND.width, room.width), height: Math.max(ISLAND.height, room.height) } : { ...ISLAND }
  }

  // The top centre of the display it is on (the primary one to start with),
  // below a taskbar that sits at the top.
  function spot() {
    const { width } = size()
    const b = alive(win)?.getBounds()
    const display = b ? screen.getDisplayNearestPoint({ x: b.x + b.width / 2, y: b.y + b.height / 2 }) : screen.getPrimaryDisplay()
    const area = display.workArea
    return { x: Math.round(area.x + (area.width - width) / 2), y: area.y }
  }

  function place() {
    if (!alive(win)) return
    const want = { ...spot(), ...size() }
    const b = win.getBounds()
    if (b.x !== want.x || b.y !== want.y || b.width !== want.width || b.height !== want.height) win.setBounds(want)
  }

  function create() {
    win = new BrowserWindow({
      ...spot(),
      ...size(),
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
        backgroundThrottling: false,
      },
    })
    win.setAlwaysOnTop(true, 'floating')
    win.setVisibleOnAllWorkspaces(true)
    pointer.setMode('pass')
    win.webContents.on('console-message', e => debugLog('island page:', e.message))
    win.webContents.setWindowOpenHandler(() => ({ action: 'deny' }))
    win.webContents.on('will-navigate', e => e.preventDefault())
    win.webContents.on('did-finish-load', () => {
      send()
      win.webContents.send('pet:asks', ctx.asks.views())
      applyVisibility()
      ctx.greet()
    })
    win.on('closed', () => {
      win = null
    })
    win.loadFile(path.join(__dirname, '..', 'pet', 'index.html'), { query: { role: 'island' } })

    setInterval(() => isShown() && place(), 2000)
    for (const event of ['display-added', 'display-removed', 'display-metrics-changed']) {
      screen.on(event, () => setTimeout(place, 500))
    }
    pointer.schedule()
  }

  // Made the first time she goes into the island, then kept (hidden when the
  // pet is on her own), so coming back is instant.
  function applyVisibility() {
    if (!alive(win)) {
      if (settings().display === 'island') create()
      return
    }
    if (isShown()) {
      place()
      if (!win.isVisible()) win.showInactive()
    } else if (win.isVisible()) {
      win.hide()
    }
    pointer.schedule()
  }

  function send() {
    alive(win)?.webContents.send('pet:update', ctx.payload())
  }

  // Where the island hangs: the middle of its lower edge (compact), on the
  // screen; null while it is not up.
  function anchor() {
    if (!alive(win) || !isShown()) return null
    const b = win.getBounds()
    return { x: b.x + b.width / 2, y: b.y + TOP + COMPACT_H }
  }

  // She is being moved near the island (point: her middle on the screen), or
  // no longer (null). Tells the page, which reaches out a drop for her, and
  // says how close she is: 'far', 'near' or 'snap' (let go, and she is back).
  function reach(point, { final = false } = {}) {
    if (!alive(win) || !isShown()) return 'far'
    const stop = () => {
      if (isReaching) win.webContents.send('island:reach', null)
      isReaching = false
      return 'far'
    }
    if (!point) return stop()
    const a = anchor()
    if (!a) return 'far'
    const distance = Math.hypot(point.x - a.x, point.y - a.y)
    const state = distance < SNAP_PX ? 'snap' : distance < REACH_PX ? 'near' : 'far'
    if (state === 'far') return stop()
    // Letting go close enough: the page takes over with absorb.
    if (final && state === 'snap') return state
    isReaching = true
    win.webContents.send('island:reach', { x: point.x, y: point.y, snap: state === 'snap' })
    return state
  }

  // She has landed (or come home): whatever the island's page heard of the
  // button, it is no longer held there.
  function endHold() {
    if (!isHolding) return
    isHolding = false
    pointer.schedule()
    alive(win)?.webContents.send('island:landed')
  }

  // Take her in from where she is (her middle on the screen).
  function absorb(point) {
    isReaching = false
    alive(win)?.webContents.send('island:absorb', point)
  }

  // --- From the island's page ------------------------------------------------------

  ipcMain.on('pet:hover', (e, isOver) => {
    if (!mine(e)) return
    pointer.setOver(isOver)
    // The island opens up to say what happened while hovered, so it counts
    // as seen once the pointer leaves.
    if (!isOver) ctx.pet.seen()
  })
  ipcMain.on('pet:keyboard', (e, needs) => {
    if (!mine(e)) return
    win.setFocusable(needs === true)
    if (needs === true) win.focus()
  })
  ipcMain.on('pet:menu', e => mine(e) && ctx.tray.popUp(win))
  ipcMain.on('pet:answer', (e, id, choice) => mine(e) && ctx.asks.answer(id, choice))
  ipcMain.on('pet:dismiss', (e, id) => mine(e) && ctx.asks.dismiss(id))
  ipcMain.on('pet:open-settings', e => mine(e) && ctx.home.open())
  // The room a shape needs, as the page measured it.
  ipcMain.on('pet:panel', (e, measured) => {
    if (!mine(e)) return
    room =
      measured && measured.width > 0 && measured.height > 0
        ? { width: Math.ceil(measured.width), height: Math.ceil(measured.height) }
        : null
    place()
  })
  // The drop pinched off: she is out, under the cursor, her middle this far
  // from it. The button is still held here; island:drop says when it is let go.
  ipcMain.on('island:holding', (e, is) => {
    if (!mine(e)) return
    isHolding = is === true
    debugLog('island', 'holding', isHolding)
    pointer.schedule()
  })
  ipcMain.on('island:release', (e, offset) => {
    debugLog('island', 'release', JSON.stringify(offset))
    if (mine(e)) ctx.letOut(offset)
  })
  ipcMain.on('island:drop', e => {
    debugLog('island', 'drop')
    if (mine(e)) ctx.petWindow.drop()
  })

  function where() {
    const bounds = alive(win)?.getBounds()
    const area = bounds && screen.getDisplayMatching(bounds).workArea
    return { bounds, size: size(), workArea: area, isVisible: !!alive(win)?.isVisible() }
  }

  return {
    applyVisibility,
    window: () => alive(win),
    handle: () => (alive(win) ? handleOf(win) : null),
    send,
    reach,
    absorb,
    endHold,
    seat: anchor,
    where,
  }
}

module.exports = { createIslandWindow }
