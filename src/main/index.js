// claude-pets: a transparent, frameless, always-on-top desktop pet that shows
// what Claude Code is doing. hooks/claude-hook.js reports to it over a local port.
const path = require('path')
const { app, BrowserWindow, ipcMain, Menu, screen } = require('electron')

// A separate profile (its own settings and single-instance lock), for tests.
if (process.env.CLAUDE_PETS_USER_DATA) {
  app.setPath('userData', process.env.CLAUDE_PETS_USER_DATA)
}

const config = require('./config')
const pets = require('./pets')
const { toMessage } = require('../shared/hook-events')
const { serve } = require('./server')
const { createPet } = require('./state')

const PORT = Number(process.env.CLAUDE_PETS_PORT || 47213)
const IS_DEBUG = process.env.CLAUDE_PETS_DEBUG === '1'

// Window size at scale 1: room for the bubble above a 192x208 cell.
const BASE_W = 300
const BASE_H = 320
const CELL_H = 208
const SCALES = { 小: 0.4, 中: 0.55, 大: 0.75 }

if (!app.requestSingleInstanceLock()) {
  app.quit()
}

let settings
let win
let drag
let walking

const pet = createPet({
  onChange: () => send(),
  onReact: reaction => win?.webContents.send('pet:react', reaction),
})

setInterval(() => pet.checkStale(), 60 * 1000)

// --- Window ---------------------------------------------------------------

function size() {
  return { width: Math.round(BASE_W * settings.scale), height: Math.round(BASE_H * settings.scale) }
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
  if (!win || drag || walking) return
  const b = win.getBounds()
  const want = size()
  const p = clamp(b.x, b.y)
  const isStretched = Math.abs(b.width - want.width) > 3 || Math.abs(b.height - want.height) > 3
  if (isStretched || p.x !== b.x || p.y !== b.y) {
    remember(moveTo(p.x, p.y))
  }
}

function remember({ x, y }) {
  Object.assign(settings, { x, y })
  config.save(settings)
}

setInterval(keepOnScreen, 2000)

function createWindow() {
  const { width, height } = size()
  const home = homeSpot()
  const start = clamp(settings.x ?? home.x, settings.y ?? home.y)

  win = new BrowserWindow({
    ...start,
    width,
    height,
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
      preload: path.join(__dirname, '..', 'preload', 'index.js'),
      contextIsolation: true,
      nodeIntegration: false,
      // An unfocused, half-transparent window still has to animate.
      backgroundThrottling: false,
    },
  })
  win.setAlwaysOnTop(true, 'floating')
  win.setVisibleOnAllWorkspaces(true)
  // Clicks pass through the transparent parts; the page turns this off while
  // the pointer is over the pet itself.
  win.setIgnoreMouseEvents(true, { forward: true })
  win.webContents.on('console-message', e => console.log(`[page] ${e.message}`))

  let isFirstLoad = true
  win.webContents.on('did-finish-load', () => {
    send()
    // A new window says hello.
    if (isFirstLoad) pet.apply({ react: 'wave', say: '来啦' })
    isFirstLoad = false
  })
  win.loadFile(path.join(__dirname, '..', 'renderer', 'index.html'))
}

function send() {
  win?.webContents.send('pet:update', { ...pet.get(), config: settings, sprite: pets.spriteUrl(settings.pet) })
}

// --- Looking at the cursor ------------------------------------------------

// Where the cursor is from the pet's face, for the 16 look directions.
function cursorFromPet(cursor) {
  const { x, y } = win.getBounds()
  const { width, height } = size()
  const cellH = CELL_H * settings.scale
  return { dx: cursor.x - (x + width / 2), dy: cursor.y - (y + height - cellH * 0.62) }
}

let lastCursor = { x: NaN, y: NaN }
setInterval(() => {
  if (!win || drag) return
  const cursor = screen.getCursorScreenPoint()
  if (cursor.x === lastCursor.x && cursor.y === lastCursor.y) return
  lastCursor = cursor
  win.webContents.send('pet:cursor', cursorFromPet(cursor))
}, 50)

// --- Menu -----------------------------------------------------------------

function change(patch) {
  Object.assign(settings, patch)
  config.save(settings)
  send()
}

function rescale(scale) {
  const { x, y } = win.getBounds()
  change({ scale })
  remember(moveTo(x, y))
}

// Start with the system: this same app, run from where it is now.
function loginItem() {
  return app.isPackaged ? { path: process.execPath } : { path: process.execPath, args: [app.getAppPath()] }
}

function isOpenAtLogin() {
  return app.getLoginItemSettings(loginItem()).openAtLogin
}

function menu() {
  const found = pets.list()

  return Menu.buildFromTemplate([
    {
      label: '宠物',
      submenu: found.length
        ? found.map(({ id, name }) => ({
            label: name,
            type: 'radio',
            checked: settings.pet === id,
            click: () => change({ pet: id }),
          }))
        : [{ label: '还没有，运行 npm run fetch-pet', enabled: false }],
    },
    {
      label: '大小',
      submenu: Object.entries(SCALES).map(([label, scale]) => ({
        label,
        type: 'radio',
        checked: settings.scale === scale,
        click: () => rescale(scale),
      })),
    },
    { label: '显示气泡', type: 'checkbox', checked: settings.bubble, click: item => change({ bubble: item.checked }) },
    { label: '空闲时走动', type: 'checkbox', checked: settings.walk, click: item => change({ walk: item.checked }) },
    { label: '眼睛跟着鼠标', type: 'checkbox', checked: settings.look, click: item => change({ look: item.checked }) },
    { label: '回到右下角', click: () => remember(moveTo(homeSpot().x, homeSpot().y)) },
    { type: 'separator' },
    {
      label: '开机自动启动',
      type: 'checkbox',
      checked: isOpenAtLogin(),
      click: item => app.setLoginItemSettings({ ...loginItem(), openAtLogin: item.checked }),
    },
    { type: 'separator' },
    { label: '退出', click: () => app.quit() },
  ])
}

// --- Pointer, dragging and walking -----------------------------------------

ipcMain.on('pet:hover', (_, isOver) => {
  if (!drag) win?.setIgnoreMouseEvents(!isOver, { forward: true })
})

// Drag by following the cursor from main, so a fast flick that outruns the
// page's own mouse events still carries the window. The page hears which way
// it moves, to run that way.
ipcMain.on('pet:drag-start', () => {
  stopWalking()
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
  clearInterval(drag.timer)
  const wasClick = !drag.moved
  const { x, y } = drag
  drag = undefined
  remember(moveTo(x, y))
  win.webContents.send('pet:drag-end')
  if (wasClick) pet.apply({ react: 'jump' })
})

ipcMain.on('pet:menu', () => menu().popup({ window: win }))

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

ipcMain.handle('pet:walk', (_, dx, ms) => walk(dx, ms))

ipcMain.on('pet:walk-stop', () => stopWalking())

// --- App ------------------------------------------------------------------

// Started again (npm start, a launcher) while running: the way to find a pet
// that is out of sight. Bring her home.
app.on('second-instance', () => {
  if (!win) return
  stopWalking()
  remember(moveTo(homeSpot().x, homeSpot().y))
  win.showInactive()
})

// The window's own spot, for checks: where it is, and the size it should be.
function where() {
  const bounds = win?.getBounds()
  const area = bounds && screen.getDisplayMatching(bounds).workArea
  return { bounds, size: settings && size(), workArea: area }
}

app.whenReady().then(() => {
  settings = config.load()
  for (const event of ['display-added', 'display-removed', 'display-metrics-changed']) {
    screen.on(event, () => setTimeout(keepOnScreen, 500))
  }
  serve({
    port: PORT,
    getState: () => ({ ...pet.get(), ...(IS_DEBUG ? { window: where() } : {}) }),
    setState: msg => pet.apply(msg),
    onHook: event => {
      const message = toMessage(event)
      if (message) pet.apply(message)
    },
    snapshot: () => win.webContents.capturePage().then(image => image.toPNG()),
    // Debug only: pretend the cursor is at (dx, dy) from the pet's face.
    lookAt: IS_DEBUG ? at => win.webContents.send('pet:cursor', at) : undefined,
    // Debug only: take a walk now.
    walkBy: IS_DEBUG ? ({ dx, ms }) => walk(dx, ms) : undefined,
    // Another copy already holds the port: let it be the pet.
    onTaken: () => app.quit(),
  })
  createWindow()
})

app.on('window-all-closed', () => app.quit())
