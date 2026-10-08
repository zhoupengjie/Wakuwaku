// The pet app: puts the pieces together and owns what they share. Started by
// ./index.js.
//
//   pet-window.js     her on the desktop: size, place, mouse, walks, dragging
//   island-window.js  the island at the top of the screen, her home
//   home-window.js    the main window and what its page asks for
//   tray.js           the tray icon and the menu
//   connection.js     how Claude Code reaches her (plugin or settings.json hooks)
//   server.js         the port the hooks report to
//   state.js          per-session moods; asks.js the prompts waiting on you
//
// Where she is: display 'pet' is the pet alone; display 'island' is the
// island, with her in it, or out on the desktop (out) while it stays.
const path = require('path')
const { app, Notification } = require('electron')

const config = require('./config')
const pets = require('./pets')
const launch = require('./launch')
const { createFullscreenWatch } = require('./fullscreen')
const { createAsks } = require('./asks')
const { serve } = require('./server')
const { createPet } = require('./state')
const { createPetWindow } = require('./pet-window')
const { createIslandWindow } = require('./island-window')
const { createHomeWindow } = require('./home-window')
const { createTray } = require('./tray')
const connection = require('./connection')
const { toMessage } = require('../agents/claude-code/events')
const i18n = require('../shared/i18n')

const IS_DEBUG = process.env.WAKUWAKU_DEBUG === '1'
const ICON = path.join(__dirname, '..', 'assets', 'icon.png')
// The island's own animation of taking her in, before she counts as in.
const ABSORB_MS = 380

function lang() {
  const { settings } = ctx
  return settings.lang === 'zh' || settings.lang === 'en' ? settings.lang : i18n.detectLang(app.getLocale())
}

// Out of sight: hidden from the tray, or another app is full screen.
const hidden = { byUser: false, byFullscreen: false }

// What every part shares. The parts reach each other through it, so it is
// filled in before any of them runs.
const ctx = {
  port: 0,
  settings: null,
  fullscreen: null,
  lang,
  T: (key, vars) => i18n.t(lang(), key, vars),
  change,
  applyPatch,
  isVisible,
  setHidden,
  setFullscreen,
  applyVisibility,
  payload,
  send,
  greet,
  setDisplay,
  setOut,
  letOut,
  absorb,
  comeHome,
}

ctx.pet = createPet({
  onChange: () => {
    send()
    ctx.tray.refresh()
    ctx.home.push()
  },
  onReact: reaction => toPages('pet:react', reaction),
  onAlert: alert,
  hold: () => ctx.settings.hold,
})

setInterval(() => ctx.pet.checkStale(), 60 * 1000)

// Prompts waiting on the person; whichever page holds the panel shows the first.
ctx.asks = createAsks({
  onChange: list => {
    toPages('pet:asks', list)
    ctx.home.push()
  },
  timeoutMs: () => ctx.settings.promptWaitSec * 1000,
  lang,
})

ctx.petWindow = createPetWindow(ctx)
ctx.island = createIslandWindow(ctx)
ctx.home = createHomeWindow(ctx)
ctx.tray = createTray(ctx)

function toPages(channel, data) {
  for (const w of [ctx.petWindow.window(), ctx.island.window()]) w?.webContents.send(channel, data)
}

// Everything the pages draw from.
function payload() {
  const s = pets.sprite(ctx.settings.pet)
  return {
    ...ctx.pet.get(),
    config: ctx.settings,
    lang: lang(),
    sprite: s && s.url,
    spriteVersion: s ? s.version : 2,
    // The next session that wants something, for the island's second bubble.
    second: ctx.pet.list().filter(x => x.mood !== 'idle')[1]?.mood || null,
  }
}

function send() {
  ctx.petWindow.send()
  ctx.island.send()
}

// A hello when she first appears, or what needs fixing: once, from whichever
// window is the one up.
let hasGreeted = false
function greet() {
  if (hasGreeted) return
  hasGreeted = true
  const isStale = connection.hooksStatus(ctx.port) === 'stale'
  ctx.pet.apply({ react: 'wave', say: { key: isStale ? 'say.hooksStale' : 'say.arrived' } })
}

// --- Visibility -------------------------------------------------------------

function isVisible() {
  const s = ctx.settings
  return !hidden.byUser && !s.dnd && !(s.hideInFullscreen && hidden.byFullscreen)
}

function applyVisibility() {
  ctx.petWindow.applyVisibility()
  ctx.island.applyVisibility()
  // Out of sight, nobody can answer here: the terminal has them.
  if (!isVisible()) ctx.asks.dismissAll()
  ctx.tray.refresh()
}

function setHidden(isHidden) {
  hidden.byUser = isHidden
  applyVisibility()
}

function setFullscreen(isFull) {
  hidden.byFullscreen = isFull
  applyVisibility()
}

// --- Where she is ---------------------------------------------------------------

// The pet alone, or the island (she goes into it).
function setDisplay(display) {
  if (display === ctx.settings.display && !ctx.settings.out) return
  if (display === 'pet') ctx.petWindow.returnToSpot()
  change({ display, out: false })
}

// Out on the desktop where she was last left, or back in the island, from the menu.
function setOut(out) {
  if (ctx.settings.display !== 'island' || ctx.settings.out === out) return
  if (out) ctx.petWindow.returnToSpot()
  change({ out })
}

// Pulled out of the island: the pet window takes her, under the cursor.
function letOut(offset) {
  if (ctx.settings.display !== 'island' || ctx.settings.out) return
  ctx.petWindow.carry(offset)
}

// Brought back close to the island: it takes her in from where she is.
function absorb(point) {
  ctx.petWindow.hideNow()
  ctx.island.absorb(point)
  setTimeout(() => change({ out: false }), ABSORB_MS)
}

// Started again while running: find her. Back home on the desktop, or into the island.
function comeHome() {
  hidden.byUser = false
  if (ctx.settings.display === 'pet' || ctx.settings.out) ctx.petWindow.comeHome()
  applyVisibility()
}

// --- Alerts: a sound in the page, a system notification --------------------

function alert({ project, mood }) {
  const { settings, T } = ctx
  if (!settings || settings.dnd) return
  // One chime, from the window that is up.
  const front = settings.display === 'island' ? ctx.island.window() : ctx.petWindow.window()
  front?.webContents.send('pet:alert', { mood })

  const group = mood === 'waiting' ? 'waiting' : mood === 'error' ? 'error' : 'done'
  if (!settings.notify[group] || !Notification.isSupported()) return
  const note = new Notification({
    title: 'Wakuwaku',
    body: T(`notify.${mood}`, { project: project ? T('notify.project', { project }) : '' }),
    icon: ICON,
    silent: true,
  })
  note.on('click', () => {
    if (!isVisible()) setHidden(false)
  })
  note.show()
}

// --- Settings ---------------------------------------------------------------

function watchFullscreen() {
  if (ctx.settings.hideInFullscreen) ctx.fullscreen.start()
  else ctx.fullscreen.stop()
}

function change(patch) {
  Object.assign(ctx.settings, patch)
  config.save(ctx.settings)
  send()
  if ('dnd' in patch || 'hideInFullscreen' in patch) watchFullscreen()
  if (['dnd', 'hideInFullscreen', 'display', 'out'].some(key => key in patch)) applyVisibility()
  if ('dnd' in patch && patch.dnd) ctx.asks.dismissAll()
  ctx.tray.refresh()
  ctx.home.changed()
}

// What a settings patch may hold, checked: anything else is dropped.
const CHECKS = {
  lang: v => ['auto', 'zh', 'en'].includes(v),
  pet: v => typeof v === 'string' && pets.list().some(p => p.id === v),
  scale: v => Object.values(config.SCALES).includes(v),
  bubble: v => typeof v === 'boolean',
  walk: v => typeof v === 'boolean',
  look: v => typeof v === 'boolean',
  sound: v => typeof v === 'boolean',
  dnd: v => typeof v === 'boolean',
  hideInFullscreen: v => typeof v === 'boolean',
  onboarded: v => typeof v === 'boolean',
  display: v => v === 'pet' || v === 'island',
  out: v => typeof v === 'boolean',
  hold: v => v === 'seen' || [8, 30, 120].includes(v),
  promptWaitSec: v => [30, 60, 120, 290].includes(v),
  notify: v => v && typeof v === 'object' && ['waiting', 'done', 'error'].every(k => typeof v[k] === 'boolean'),
}

// A patch from the main window: checked, with the size, the display and her
// being out through the windows, so she keeps her spot.
function applyPatch(patch) {
  const ok = {}
  for (const [key, value] of Object.entries(patch || {})) {
    if (CHECKS[key]?.(value)) ok[key] = value
  }
  const { scale, display, out, ...rest } = ok
  if (scale !== undefined) ctx.petWindow.resize({ scale })
  if (display !== undefined) setDisplay(display)
  if (out !== undefined) setOut(out)
  if (Object.keys(rest).length) change(rest)
  return ctx.home.snapshot()
}

// --- Hooks ------------------------------------------------------------------

// An event from Claude Code. A prompt holds the answer until the person picks
// on the pet, the terminal settles it, or time runs out; out of sight, it is
// the terminal's at once.
function onHook(event, hungUp) {
  const message = toMessage(event)
  if (message) ctx.pet.apply(message)

  if (event?.hook_event_name === 'PermissionRequest') {
    if (!isVisible()) return {}
    return new Promise(resolve => {
      const id = ctx.asks.add(event, resolve)
      if (id === null) return resolve({})
      hungUp.addEventListener('abort', () => ctx.asks.cancel(id), { once: true })
    })
  }
  ctx.asks.seen(event)
  return undefined
}

// Debug only (WAKUWAKU_DEBUG=1): what the end-to-end test drives. Pages:
// 'pet' (her window), 'island', 'home' (the main window).
function page(name) {
  return name === 'home' ? ctx.home.window() : name === 'island' ? ctx.island.window() : ctx.petWindow.window()
}

function debugRoutes() {
  if (!IS_DEBUG) return {}
  return {
    // Pretend the cursor is at (dx, dy) from the pet's face.
    lookAt: at => ctx.petWindow.window()?.webContents.send('pet:cursor', at),
    // Take a walk now.
    walkBy: ({ dx, ms }) => ctx.petWindow.walk(dx, ms),
    // Click an element of the page that holds the prompt panel, as the person would.
    click: selector =>
      page(ctx.settings.display === 'island' ? 'island' : 'pet').webContents.executeJavaScript(
        `(() => { const el = document.querySelector(${JSON.stringify(selector)}); if (!el) return 'missing'; if (el.disabled) return 'disabled'; el.click(); return 'ok' })()`,
      ),
    // Run a script in a page.
    evaluate: ({ page: name, code }) => {
      const target = page(name)
      if (!target) return Promise.resolve('no such page')
      return target.webContents.executeJavaScript(String(code))
    },
    // Open the main window; bring her back into the island from where she
    // stands; or take a patch as the main window would.
    debugSettings: patch => {
      if (patch === 'open') return ctx.home.open(), ctx.home.snapshot()
      // As if she were let go just under the island.
      if (patch === 'absorb') {
        const b = ctx.island.window()?.getBounds()
        if (b) absorb({ x: Math.round(b.x + b.width / 2), y: b.y + 130 })
        return ctx.home.snapshot()
      }
      return applyPatch(patch)
    },
    // Pretend another app went full screen, or came back.
    debugFullscreen: isFull => {
      setFullscreen(isFull === true)
      return ctx.petWindow.where()
    },
  }
}

// --- App --------------------------------------------------------------------------

// Debug only: a line in <userData>/debug.log, for what happens before any window.
function debugLog(line) {
  if (!IS_DEBUG) return
  try {
    require('fs').appendFileSync(path.join(app.getPath('userData'), 'debug.log'), `${new Date().toISOString()} ${process.pid} ${line}
`)
  } catch {}
}

// One pet at a time. When another copy holds the lock: if it answers on the
// port it is the pet (and has heard from us, see second-instance); if not, it
// is one on its way out (just quit, still letting go), so wait a little.
async function takeLock() {
  if (app.requestSingleInstanceLock()) return true
  for (let i = 0; i < 24; i++) {
    if (await launch.isUp(ctx.port)) return false
    await new Promise(resolve => setTimeout(resolve, 250))
    if (app.requestSingleInstanceLock()) return true
  }
  return false
}

async function start(options) {
  ctx.port = options.port

  if (!(await takeLock())) {
    debugLog('another copy holds the lock; leaving')
    app.quit()
    return
  }
  app.setAppUserModelId('com.zhoupengjie.wakuwaku')

  // Started again (npm start, the Start menu) while running: the way to find
  // a pet that is out of sight.
  app.on('second-instance', () => {
    if (ctx.petWindow.window()) comeHome()
  })

  app.whenReady().then(() => {
    ctx.settings = config.load()
    if (process.platform === 'darwin') app.dock?.hide()

    ctx.fullscreen = createFullscreenWatch({
      ownHandles: () => [ctx.petWindow.handle(), ctx.island.handle(), ctx.home.handle()].filter(Boolean),
      onChange: isFull => setFullscreen(isFull),
    })

    serve({
      port: ctx.port,
      getState: () => ({
        ...ctx.pet.get(),
        ...(IS_DEBUG
          ? {
              window: ctx.petWindow.where(),
              island: ctx.island.where(),
              asks: ctx.asks.views(),
              settings: ctx.settings,
              hooks: connection.hooksStatus(ctx.port),
            }
          : {}),
      }),
      setState: msg => ctx.pet.apply(msg),
      onHook,
      snapshot: name => {
        const target = page(name) || ctx.petWindow.window()
        return target.webContents.capturePage().then(image => image.toPNG())
      },
      ...debugRoutes(),
      // Another copy already holds the port: let it be the pet.
      onTaken: () => app.quit(),
    })

    ctx.petWindow.create()
    ctx.island.applyVisibility()
    ctx.tray.create()
    watchFullscreen()

    // Nothing to show yet, or never set up: open the main window to begin.
    if (!pets.list().length || (!ctx.settings.onboarded && connection.connection(ctx.port) === 'none')) ctx.home.open()
  })

  app.on('window-all-closed', () => app.quit())
}

module.exports = { start }
