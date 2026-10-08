// The pet: its window, tray icon, settings window, and the port the hooks
// report to. Started by ./index.js.
const fs = require('fs')
const path = require('path')
const { app, BrowserWindow, clipboard, ipcMain, Menu, Notification, Tray, nativeImage, screen, shell } = require('electron')

const config = require('./config')
const pets = require('./pets')
const launch = require('./launch')
const { createFullscreenWatch } = require('./fullscreen')
const { createAsks } = require('./asks')
const { serve } = require('./server')
const { createPet } = require('./state')
const { toMessage } = require('../shared/hook-events')
const hooksConfig = require('../shared/hooks-config')
const i18n = require('../shared/i18n')
const petFetch = require('../shared/pet-fetch')

const IS_DEBUG = process.env.WAKUWAKU_DEBUG === '1'
const ICON = path.join(__dirname, '..', 'assets', 'icon.png')
const TRAY_ICON = mood => path.join(__dirname, '..', 'assets', 'tray', `${mood}.png`)

// Window size at scale 1: room for the bubble above a 192x208 cell. The
// island's window is fixed: room for the island at its widest without a
// prompt (hovered, with a second session beside it), and for its spring.
const BASE_W = 300
const BASE_H = 320
const ISLAND = { width: 460, height: 132 }
const CELL_H = 208
const SCALES = { small: 0.4, medium: 0.55, large: 0.75 }
// The cursor further than this from her face does not catch her eye.
const LOOK_FAR = 900

let port
let settings
let win
let settingsWin = null
let tray = null
let fullscreen
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

// A window still there to talk to: closed ones throw on any call.
function alive(w) {
  return w && !w.isDestroyed() ? w : null
}

const isIsland = () => settings.display === 'island'

function lang() {
  return settings.lang === 'zh' || settings.lang === 'en' ? settings.lang : i18n.detectLang(app.getLocale())
}

function T(key, vars) {
  return i18n.t(lang(), key, vars)
}

const pet = createPet({
  onChange: () => {
    send()
    refreshTray()
    pushHome()
  },
  onReact: reaction => alive(win)?.webContents.send('pet:react', reaction),
  onAlert: alert,
  hold: () => settings.hold,
})

setInterval(() => pet.checkStale(), 60 * 1000)

// Prompts waiting on the person; the page shows the first.
const asks = createAsks({
  onChange: list => {
    alive(win)?.webContents.send('pet:asks', list)
    pushHome()
  },
  timeoutMs: () => settings.promptWaitSec * 1000,
  lang,
})

// --- Visibility -------------------------------------------------------------

function isVisible() {
  return !isUserHidden && !settings.dnd && !(settings.hideInFullscreen && isFullscreen)
}

function applyVisibility() {
  if (!alive(win)) return
  if (isVisible()) {
    if (!win.isVisible()) win.showInactive()
  } else {
    if (win.isVisible()) win.hide()
    // Out of sight, nobody can answer here: the terminal has them.
    asks.dismissAll()
  }
  refreshTray()
  scheduleCursor()
}

function handleOf(w) {
  const b = w.getNativeWindowHandle()
  return b.length >= 8 ? b.readBigUInt64LE(0) : BigInt(b.readUInt32LE(0))
}

function watchFullscreen() {
  if (settings.hideInFullscreen) fullscreen.start()
  else fullscreen.stop()
}

// --- Window -----------------------------------------------------------------

// The window without a prompt: the bubble and the pet, or the island.
function baseSize() {
  if (isIsland()) return { ...ISLAND }
  return { width: Math.round(BASE_W * settings.scale), height: Math.round(BASE_H * settings.scale) }
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
  Object.assign(settings, {
    x: Math.round(x + now.width / 2 + shift - base.width / 2),
    y: Math.round(y + now.height - base.height),
  })
  config.save(settings)
}

function comeHome() {
  stopWalking()
  isUserHidden = false
  applyVisibility()
  const p = isIsland() ? islandSpot() : homeSpot()
  remember(moveTo(p.x, p.y))
}

setInterval(keepOnScreen, 2000)

function createWindow() {
  const { width, height } = size()
  const home = homeSpot()
  const start = clamp(settings.x ?? home.x, settings.y ?? home.y)
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
      preload: path.join(__dirname, '..', 'preload', 'index.js'),
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
    win.webContents.send('pet:asks', asks.views())
    if (isVisible()) win.showInactive()
    // A new window says hello, or what needs fixing.
    if (isFirstLoad) {
      const hooks = hooksStatus()
      pet.apply({ react: 'wave', say: { key: hooks === 'stale' ? 'say.hooksStale' : 'say.arrived' } })
    }
    isFirstLoad = false
  })
  // The pet window gone means the app is done, settings window or not.
  win.on('closed', () => {
    win = null
    app.quit()
  })
  win.loadFile(path.join(__dirname, '..', 'renderer', 'index.html'))
}

function send() {
  const s = pets.sprite(settings.pet)
  alive(win)?.webContents.send('pet:update', {
    ...pet.get(),
    config: settings,
    lang: lang(),
    sprite: s && s.url,
    spriteVersion: s ? s.version : 2,
    // The next session that wants something, for the island's second bubble.
    second: pet.list().filter(x => x.mood !== 'idle')[1]?.mood || null,
  })
}

// --- Alerts: a sound in the page, a system notification --------------------

function alert({ project, mood }) {
  if (!settings || settings.dnd) return
  alive(win)?.webContents.send('pet:alert', { mood })

  const group = mood === 'waiting' ? 'waiting' : mood === 'error' ? 'error' : 'done'
  if (!settings.notify[group] || !Notification.isSupported()) return
  const note = new Notification({
    title: 'Wakuwaku',
    body: T(`notify.${mood}`, { project: project ? T('notify.project', { project }) : '' }),
    icon: ICON,
    silent: true,
  })
  note.on('click', () => comeHomeIfHidden())
  note.show()
}

function comeHomeIfHidden() {
  if (!isVisible()) {
    isUserHidden = false
    applyVisibility()
  }
}

// --- Looking at the cursor ----------------------------------------------------

// Where the cursor is from the pet's face, for the 16 look directions.
function cursorFromPet(cursor) {
  const { x, y } = win.getBounds()
  const { width, height } = size()
  const cellH = CELL_H * settings.scale
  return { dx: cursor.x - (x + width / 2 + shift), dy: cursor.y - (y + height - cellH * 0.62) }
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
  const canLook = settings.look && !isIsland() && !drag && !panel && pet.get().mood === 'idle'
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

// --- Settings ---------------------------------------------------------------

function change(patch) {
  Object.assign(settings, patch)
  config.save(settings)
  send()
  if ('dnd' in patch || 'hideInFullscreen' in patch) {
    watchFullscreen()
    applyVisibility()
  }
  if ('dnd' in patch && patch.dnd) asks.dismissAll()
  refreshTray()
  alive(settingsWin)?.webContents.send('settings:changed', snapshot())
}

// A new size keeps the pet's spot: her bottom centre. Into the island, the
// window goes to the top of the screen; out of it, back to where she was.
function resize(patch) {
  const before = win.getBounds()
  const old = size()
  const wasIsland = isIsland()
  change(patch)
  const now = size()
  if (isIsland()) {
    const p = islandSpot()
    return moveTo(p.x, p.y)
  }
  if (wasIsland) {
    const home = homeSpot()
    return remember(moveTo(settings.x ?? home.x, settings.y ?? home.y))
  }
  remember(moveTo(before.x + (old.width - now.width) / 2, before.y + old.height - now.height))
}

function rescale(scale) {
  resize({ scale })
}

// What a settings patch may hold, checked: anything else is dropped.
const CHECKS = {
  lang: v => ['auto', 'zh', 'en'].includes(v),
  pet: v => typeof v === 'string' && pets.list().some(p => p.id === v),
  scale: v => Object.values(SCALES).includes(v),
  bubble: v => typeof v === 'boolean',
  walk: v => typeof v === 'boolean',
  look: v => typeof v === 'boolean',
  sound: v => typeof v === 'boolean',
  dnd: v => typeof v === 'boolean',
  hideInFullscreen: v => typeof v === 'boolean',
  onboarded: v => typeof v === 'boolean',
  display: v => v === 'pet' || v === 'island',
  hold: v => v === 'seen' || [8, 30, 120].includes(v),
  promptWaitSec: v => [30, 60, 120, 290].includes(v),
  notify: v => v && typeof v === 'object' && ['waiting', 'done', 'error'].every(k => typeof v[k] === 'boolean'),
}

function applyPatch(patch) {
  const ok = {}
  for (const [key, value] of Object.entries(patch || {})) {
    if (CHECKS[key]?.(value)) ok[key] = value
  }
  const sizing = {}
  for (const key of ['scale', 'display']) {
    if (key in ok) {
      sizing[key] = ok[key]
      delete ok[key]
    }
  }
  if (Object.keys(sizing).length) resize(sizing)
  if (Object.keys(ok).length) change(ok)
  return snapshot()
}

// Start with the system: this same app, run from where it is now.
function isOpenAtLogin() {
  try {
    return app.getLoginItemSettings(launch.launchSpecForLogin()).openAtLogin
  } catch {
    return false
  }
}

function readClaudeSettings() {
  try {
    return JSON.parse(fs.readFileSync(launch.claudeSettingsFile(), 'utf8'))
  } catch (err) {
    if (err.code === 'ENOENT') return {}
    throw err
  }
}

function hooksStatus() {
  try {
    return hooksConfig.status(readClaudeSettings(), { port, launch: launch.launchSpec() })
  } catch {
    return 'unreadable'
  }
}

function isPluginEnabled() {
  try {
    return hooksConfig.isPluginEnabled(readClaudeSettings())
  } catch {
    return false
  }
}

// How Claude Code reaches her: 'plugin', 'hooks' (written into settings.json),
// 'both' (every event twice), or 'none'.
function connection() {
  const plugin = isPluginEnabled()
  const hooks = hooksStatus()
  const isHooks = hooks !== 'missing' && hooks !== 'unreadable'
  return plugin && isHooks ? 'both' : plugin ? 'plugin' : isHooks ? 'hooks' : 'none'
}

// Change Claude Code's settings.json, backed up once beside itself.
function writeHooks(action) {
  const file = launch.claudeSettingsFile()
  const before = readClaudeSettings()
  const after =
    action === 'remove'
      ? hooksConfig.uninstall(before)
      : hooksConfig.install(before, { port, launch: launch.launchSpec(), httpOnly: action === 'install-http' })
  const backup = `${file}.wakuwaku.bak`
  if (fs.existsSync(file) && !fs.existsSync(backup)) fs.copyFileSync(file, backup)
  fs.mkdirSync(path.dirname(file), { recursive: true })
  fs.writeFileSync(file, `${JSON.stringify(after, null, 2)}\n`)
}

const PLUGIN_COMMANDS = ['/plugin marketplace add zhoupengjie/wakuwaku', `/plugin install ${hooksConfig.PLUGIN_ID}`]

function snapshot() {
  return {
    settings,
    lang: lang(),
    pets: pets.list(),
    now: pet.get(),
    sessions: pet.list(),
    asks: asks.views(),
    isVisible: isVisible(),
    connection: connection(),
    pluginCommands: PLUGIN_COMMANDS,
    hooks: hooksStatus(),
    settingsFile: launch.claudeSettingsFile(),
    loginAtStart: isOpenAtLogin(),
    version: app.getVersion(),
    fullscreenAvailable: fullscreen?.isAvailable ?? false,
    platform: process.platform,
    isPackaged: app.isPackaged,
  }
}

// The main window follows what happens, a few times a second at most.
let homeTimer
function pushHome() {
  if (!alive(settingsWin) || homeTimer) return
  homeTimer = setTimeout(() => {
    homeTimer = undefined
    alive(settingsWin)?.webContents.send('settings:changed', snapshot())
  }, 250)
}

function openSettings() {
  if (settingsWin && !settingsWin.isDestroyed()) {
    settingsWin.show()
    settingsWin.focus()
    return
  }
  settingsWin = new BrowserWindow({
    width: 860,
    height: 640,
    minWidth: 640,
    minHeight: 480,
    show: false,
    title: 'Wakuwaku',
    icon: ICON,
    autoHideMenuBar: true,
    webPreferences: {
      preload: path.join(__dirname, '..', 'settings', 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  })
  settingsWin.removeMenu()
  settingsWin.webContents.setWindowOpenHandler(() => ({ action: 'deny' }))
  settingsWin.webContents.on('will-navigate', e => e.preventDefault())
  settingsWin.loadFile(path.join(__dirname, '..', 'settings', 'index.html'))
  settingsWin.once('ready-to-show', () => settingsWin.show())
  settingsWin.on('closed', () => {
    settingsWin = null
  })
}

ipcMain.handle('settings:get', () => snapshot())
ipcMain.handle('settings:set', (_, patch) => applyPatch(patch))
ipcMain.handle('settings:login', (_, on) => {
  app.setLoginItemSettings({ ...launch.launchSpecForLogin(), openAtLogin: on === true })
  return snapshot()
})
ipcMain.handle('settings:hooks', (_, action) => {
  if (!['install', 'install-http', 'remove'].includes(action)) return { ok: false, snapshot: snapshot() }
  try {
    writeHooks(action)
    return { ok: true, snapshot: snapshot() }
  } catch (err) {
    return { ok: false, error: err.message, snapshot: snapshot() }
  }
})
ipcMain.handle('settings:fetch', async (_, ref) => {
  try {
    const got = await petFetch.downloadPet(String(ref ?? ''), pets.userDir())
    // The first pet, or one replacing a missing sprite, becomes the pet.
    if (!pets.spriteUrl(settings.pet) || pets.list().length === 1) change({ pet: got.id })
    else send()
    return { ok: true, pet: got, warning: got.warning ? petFetch.describe(lang(), got.warning) : undefined, snapshot: snapshot() }
  } catch (err) {
    return { ok: false, error: petFetch.describe(lang(), err), snapshot: snapshot() }
  }
})
// Only these two places, whatever the page asks.
ipcMain.handle('settings:open-site', (_, where) => shell.openExternal(where === 'repo' ? 'https://github.com/zhoupengjie/wakuwaku' : petFetch.SITE))
ipcMain.handle('settings:copy', (_, text) => {
  clipboard.writeText(String(text ?? ''))
  return true
})
ipcMain.handle('settings:answer', (_, id, choice) => asks.answer(id, choice))
ipcMain.handle('settings:dismiss', (_, id) => asks.dismiss(id))
ipcMain.handle('settings:show-pet', (_, on) => {
  isUserHidden = on !== true
  applyVisibility()
  return snapshot()
})

// A page of codex-pets.net's gallery, fetched here. Only what the gallery
// shows is passed on, and only pictures from the site itself.
ipcMain.handle('settings:gallery', async (_, { page = 1, sort = 'popular' } = {}) => {
  try {
    const url = `${petFetch.SITE}/api/pets?page=${Math.max(1, Number(page) || 1)}&pageSize=12&sort=${sort === 'newest' ? 'newest' : 'popular'}`
    const res = await fetch(url, { headers: { 'user-agent': 'wakuwaku' } })
    if (!res.ok) throw new Error(String(res.status))
    const body = await res.json()
    const items = (body.pets || [])
      .filter(p => typeof p.id === 'string' && /^[a-z0-9][a-z0-9-]*$/i.test(p.id))
      .map(p => ({
        id: p.id,
        name: String(p.displayName || p.id),
        author: String(p.ownerName || ''),
        likes: Number(p.likeCount) || 0,
        version: p.spriteVersionNumber === 2 ? 2 : 1,
        preview: typeof p.previewUrl === 'string' && p.previewUrl.startsWith(`${petFetch.SITE}/`) ? p.previewUrl : '',
      }))
    return { ok: true, items, page: body.page, totalPages: body.totalPages }
  } catch (err) {
    return { ok: false, error: String(err?.message || err) }
  }
})

// --- Menus and tray ------------------------------------------------------------

function menuItems({ isTray }) {
  const found = pets.list()
  return [
    isTray
      ? {
          label: isVisible() ? T('menu.hide') : T('menu.show'),
          enabled: !settings.dnd,
          click: () => {
            isUserHidden = isVisible()
            applyVisibility()
          },
        }
      : null,
    {
      label: T('menu.pet'),
      submenu: found.length
        ? found.map(({ id, name }) => ({ label: name, type: 'radio', checked: settings.pet === id, click: () => change({ pet: id }) }))
        : [{ label: T('menu.noPets'), click: openSettings }],
    },
    {
      label: T('menu.size'),
      submenu: Object.entries(SCALES).map(([name, scale]) => ({
        label: T(`menu.${name}`),
        type: 'radio',
        checked: settings.scale === scale,
        click: () => rescale(scale),
      })),
    },
    { label: T('menu.bubble'), type: 'checkbox', checked: settings.bubble, click: item => change({ bubble: item.checked }) },
    { label: T('menu.walk'), type: 'checkbox', checked: settings.walk, click: item => change({ walk: item.checked }) },
    { label: T('menu.look'), type: 'checkbox', checked: settings.look, click: item => change({ look: item.checked }) },
    { label: T('menu.island'), type: 'checkbox', checked: isIsland(), click: item => resize({ display: item.checked ? 'island' : 'pet' }) },
    { label: T('menu.home'), click: comeHome },
    { type: 'separator' },
    { label: T('menu.dnd'), type: 'checkbox', checked: settings.dnd, click: item => change({ dnd: item.checked }) },
    { label: T('menu.settings'), click: openSettings },
    { type: 'separator' },
    { label: T('menu.quit'), click: () => app.quit() },
  ].filter(Boolean)
}

function statusText() {
  const now = pet.get()
  return `${T(`mood.${now.mood}`)}${now.project ? ` · ${now.project}` : ''}`
}

function createTray() {
  try {
    tray = new Tray(nativeImage.createFromPath(TRAY_ICON('idle')))
    trayMood = 'idle'
  } catch {
    tray = null
    return
  }
  const popUp = () => tray.popUpContextMenu(Menu.buildFromTemplate(menuItems({ isTray: true })))
  if (process.platform === 'darwin') {
    tray.on('click', popUp)
  } else {
    // Click: show or hide her; right-click: the menu.
    tray.on('click', () => {
      if (settings.dnd) return openSettings()
      isUserHidden = isVisible()
      applyVisibility()
    })
    tray.on('right-click', popUp)
  }
  refreshTray()
}

let trayMood = ''
function refreshTray() {
  if (!tray || !settings) return
  tray.setToolTip(T('tray.tooltip', { status: settings.dnd ? T('menu.dnd') : statusText() }))
  // The tray face shows her mood.
  const mood = pet.get().mood
  if (mood !== trayMood) {
    trayMood = mood
    try {
      tray.setImage(nativeImage.createFromPath(TRAY_ICON(mood)))
    } catch {}
  }
}

// --- Pointer, dragging, walking, the prompt panel ---------------------------------

ipcMain.on('pet:hover', (_, isOver) => {
  isOverPet = isOver === true
  if (!drag) setMouseMode(isOverPet ? 'catch' : 'forward')
  // The pointer on her: you have seen what she had to say. The island opens
  // up to say it while hovered, so it counts once the pointer leaves.
  if (isIsland() ? !isOver : isOver) pet.seen()
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
    return openSettings()
  }
  clearInterval(drag.timer)
  const wasClick = !drag.moved
  const { x, y } = drag
  drag = undefined
  parked = null
  remember(moveTo(x, y))
  win.webContents.send('pet:drag-end')
  if (wasClick) pet.apply({ react: 'jump' })
})

ipcMain.on('pet:menu', () => Menu.buildFromTemplate(menuItems({ isTray: false })).popup({ window: win }))

ipcMain.on('pet:answer', (_, id, choice) => asks.answer(id, choice))
ipcMain.on('pet:dismiss', (_, id) => asks.dismiss(id))
ipcMain.on('pet:panel', (_, measured) => {
  const next =
    measured && measured.width > 0 && measured.height > 0
      ? { width: Math.ceil(measured.width), height: Math.ceil(measured.height) }
      : null
  if (JSON.stringify(next) !== JSON.stringify(panel)) setPanel(next)
})
ipcMain.on('pet:open-settings', () => openSettings())

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

// --- App --------------------------------------------------------------------------

// The window's own spot, for checks: where it is, and the size it should be.
function where() {
  const bounds = alive(win)?.getBounds()
  const area = bounds && screen.getDisplayMatching(bounds).workArea
  return { bounds, size: settings && size(), workArea: area, shift, isVisible: isVisible() }
}

// One pet at a time. When another copy holds the lock: if it answers on the
// port it is the pet (and has heard from us, see second-instance); if not, it
// is one on its way out (just quit, still letting go), so wait a little.
async function takeLock() {
  if (app.requestSingleInstanceLock()) return true
  for (let i = 0; i < 24; i++) {
    if (await launch.isUp(port)) return false
    await new Promise(resolve => setTimeout(resolve, 250))
    if (app.requestSingleInstanceLock()) return true
  }
  return false
}

async function start(options) {
  port = options.port

  if (!(await takeLock())) {
    app.quit()
    return
  }
  app.setAppUserModelId('com.zhoupengjie.wakuwaku')

  // Started again (npm start, the Start menu) while running: the way to find
  // a pet that is out of sight. Bring her home.
  app.on('second-instance', () => {
    if (win) comeHome()
  })

  app.whenReady().then(() => {
    settings = config.load()
    if (process.platform === 'darwin') app.dock?.hide()

    fullscreen = createFullscreenWatch({
      ownHandles: () => [win, settingsWin].filter(w => w && !w.isDestroyed()).map(handleOf),
      onChange: isFull => {
        isFullscreen = isFull
        applyVisibility()
      },
    })

    for (const event of ['display-added', 'display-removed', 'display-metrics-changed']) {
      screen.on(event, () => setTimeout(keepOnScreen, 500))
    }

    serve({
      port,
      getState: () => ({ ...pet.get(), ...(IS_DEBUG ? { window: where(), asks: asks.views(), settings, hooks: hooksStatus() } : {}) }),
      setState: msg => pet.apply(msg),
      onHook: (event, hungUp) => {
        const message = toMessage(event)
        if (message) pet.apply(message)

        // A prompt: hold the answer until the person picks on the pet, the
        // terminal settles it, or time runs out. Out of sight, it is the
        // terminal's at once.
        if (event?.hook_event_name === 'PermissionRequest') {
          if (!isVisible()) return {}
          return new Promise(resolve => {
            const id = asks.add(event, resolve)
            if (id === null) return resolve({})
            hungUp.addEventListener('abort', () => asks.cancel(id), { once: true })
          })
        }
        asks.seen(event)
        return undefined
      },
      snapshot: page => {
        const target = page === 'settings' && settingsWin && !settingsWin.isDestroyed() ? settingsWin : win
        return target.webContents.capturePage().then(image => image.toPNG())
      },
      // Debug only: pretend the cursor is at (dx, dy) from the pet's face.
      lookAt: IS_DEBUG ? at => win.webContents.send('pet:cursor', at) : undefined,
      // Debug only: take a walk now.
      walkBy: IS_DEBUG ? ({ dx, ms }) => walk(dx, ms) : undefined,
      // Debug only: click an element of the page, as the person would.
      click: IS_DEBUG
        ? selector =>
            win.webContents.executeJavaScript(
              `(() => { const el = document.querySelector(${JSON.stringify(selector)}); if (!el) return 'missing'; if (el.disabled) return 'disabled'; el.click(); return 'ok' })()`,
            )
        : undefined,
      // Debug only: run a script in the pet page or the settings page.
      evaluate: IS_DEBUG
        ? ({ page, code }) => {
            const target = page === 'settings' ? settingsWin : win
            if (!target || target.isDestroyed()) return Promise.resolve('no such page')
            return target.webContents.executeJavaScript(String(code))
          }
        : undefined,
      // Debug only: open the settings window, or take a patch as it would.
      debugSettings: IS_DEBUG ? patch => (patch === 'open' ? (openSettings(), snapshot()) : applyPatch(patch)) : undefined,
      // Debug only: pretend another app went full screen, or came back.
      debugFullscreen: IS_DEBUG
        ? isFull => {
            isFullscreen = isFull === true
            applyVisibility()
            return where()
          }
        : undefined,
      // Another copy already holds the port: let it be the pet.
      onTaken: () => app.quit(),
    })

    createWindow()
    createTray()
    watchFullscreen()
    scheduleCursor()

    // Nothing to show yet, or never set up: open the main window to begin.
    if (!pets.list().length || (!settings.onboarded && connection() === 'none')) openSettings()
  })

  app.on('window-all-closed', () => app.quit())
}

module.exports = { start }
