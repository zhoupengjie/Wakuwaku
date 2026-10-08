// The pet app: puts the pieces together and owns what they share. Started by
// ./index.js.
//
//   pet-window.js   her window: size, place, visibility, mouse, walks
//   home-window.js  the main window and what its page asks for
//   tray.js         the tray icon and the menu
//   connection.js   how Claude Code reaches her (plugin or settings.json hooks)
//   server.js       the port the hooks report to
//   state.js        per-session moods; asks.js the prompts waiting on you
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
const { createHomeWindow } = require('./home-window')
const { createTray } = require('./tray')
const connection = require('./connection')
const { toMessage } = require('../agents/claude-code/events')
const i18n = require('../shared/i18n')

const IS_DEBUG = process.env.WAKUWAKU_DEBUG === '1'
const ICON = path.join(__dirname, '..', 'assets', 'icon.png')

function lang() {
  const { settings } = ctx
  return settings.lang === 'zh' || settings.lang === 'en' ? settings.lang : i18n.detectLang(app.getLocale())
}

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
}

ctx.pet = createPet({
  onChange: () => {
    ctx.petWindow.send()
    ctx.tray.refresh()
    ctx.home.push()
  },
  onReact: reaction => ctx.petWindow.window()?.webContents.send('pet:react', reaction),
  onAlert: alert,
  hold: () => ctx.settings.hold,
})

setInterval(() => ctx.pet.checkStale(), 60 * 1000)

// Prompts waiting on the person; the page shows the first.
ctx.asks = createAsks({
  onChange: list => {
    ctx.petWindow.window()?.webContents.send('pet:asks', list)
    ctx.home.push()
  },
  timeoutMs: () => ctx.settings.promptWaitSec * 1000,
  lang,
})

ctx.petWindow = createPetWindow(ctx)
ctx.home = createHomeWindow(ctx)
ctx.tray = createTray(ctx)

// --- Alerts: a sound in the page, a system notification --------------------

function alert({ project, mood }) {
  const { settings, T } = ctx
  if (!settings || settings.dnd) return
  ctx.petWindow.window()?.webContents.send('pet:alert', { mood })

  const group = mood === 'waiting' ? 'waiting' : mood === 'error' ? 'error' : 'done'
  if (!settings.notify[group] || !Notification.isSupported()) return
  const note = new Notification({
    title: 'Wakuwaku',
    body: T(`notify.${mood}`, { project: project ? T('notify.project', { project }) : '' }),
    icon: ICON,
    silent: true,
  })
  note.on('click', () => {
    if (!ctx.petWindow.isVisible()) ctx.petWindow.setHidden(false)
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
  ctx.petWindow.send()
  if ('dnd' in patch || 'hideInFullscreen' in patch) {
    watchFullscreen()
    ctx.petWindow.applyVisibility()
  }
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
  hold: v => v === 'seen' || [8, 30, 120].includes(v),
  promptWaitSec: v => [30, 60, 120, 290].includes(v),
  notify: v => v && typeof v === 'object' && ['waiting', 'done', 'error'].every(k => typeof v[k] === 'boolean'),
}

// A patch from the main window: checked, and the size or display mode
// through the window so she keeps her spot.
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
  if (Object.keys(sizing).length) ctx.petWindow.resize(sizing)
  if (Object.keys(ok).length) change(ok)
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
    if (!ctx.petWindow.isVisible()) return {}
    return new Promise(resolve => {
      const id = ctx.asks.add(event, resolve)
      if (id === null) return resolve({})
      hungUp.addEventListener('abort', () => ctx.asks.cancel(id), { once: true })
    })
  }
  ctx.asks.seen(event)
  return undefined
}

// Debug only (WAKUWAKU_DEBUG=1): what the end-to-end test drives.
function debugRoutes() {
  if (!IS_DEBUG) return {}
  const page = name => (name === 'home' ? ctx.home.window() : ctx.petWindow.window())
  return {
    // Pretend the cursor is at (dx, dy) from the pet's face.
    lookAt: at => ctx.petWindow.window()?.webContents.send('pet:cursor', at),
    // Take a walk now.
    walkBy: ({ dx, ms }) => ctx.petWindow.walk(dx, ms),
    // Click an element of the pet page, as the person would.
    click: selector =>
      ctx.petWindow
        .window()
        .webContents.executeJavaScript(
          `(() => { const el = document.querySelector(${JSON.stringify(selector)}); if (!el) return 'missing'; if (el.disabled) return 'disabled'; el.click(); return 'ok' })()`,
        ),
    // Run a script in the pet page or the main window.
    evaluate: ({ page: name, code }) => {
      const target = page(name)
      if (!target) return Promise.resolve('no such page')
      return target.webContents.executeJavaScript(String(code))
    },
    // Open the main window, or take a patch as it would.
    debugSettings: patch => (patch === 'open' ? (ctx.home.open(), ctx.home.snapshot()) : applyPatch(patch)),
    // Pretend another app went full screen, or came back.
    debugFullscreen: isFull => {
      ctx.petWindow.setFullscreen(isFull === true)
      return ctx.petWindow.where()
    },
  }
}

// --- App --------------------------------------------------------------------------

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
    app.quit()
    return
  }
  app.setAppUserModelId('com.zhoupengjie.wakuwaku')

  // Started again (npm start, the Start menu) while running: the way to find
  // a pet that is out of sight. Bring her home.
  app.on('second-instance', () => {
    if (ctx.petWindow.window()) ctx.petWindow.comeHome()
  })

  app.whenReady().then(() => {
    ctx.settings = config.load()
    if (process.platform === 'darwin') app.dock?.hide()

    ctx.fullscreen = createFullscreenWatch({
      ownHandles: () => [ctx.petWindow.handle(), ctx.home.handle()].filter(Boolean),
      onChange: isFull => ctx.petWindow.setFullscreen(isFull),
    })

    serve({
      port: ctx.port,
      getState: () => ({
        ...ctx.pet.get(),
        ...(IS_DEBUG
          ? { window: ctx.petWindow.where(), asks: ctx.asks.views(), settings: ctx.settings, hooks: connection.hooksStatus(ctx.port) }
          : {}),
      }),
      setState: msg => ctx.pet.apply(msg),
      onHook,
      snapshot: name => {
        const target = (name === 'home' && ctx.home.window()) || ctx.petWindow.window()
        return target.webContents.capturePage().then(image => image.toPNG())
      },
      ...debugRoutes(),
      // Another copy already holds the port: let it be the pet.
      onTaken: () => app.quit(),
    })

    ctx.petWindow.create()
    ctx.tray.create()
    watchFullscreen()

    // Nothing to show yet, or never set up: open the main window to begin.
    if (!pets.list().length || (!ctx.settings.onboarded && connection.connection(ctx.port) === 'none')) ctx.home.open()
  })

  app.on('window-all-closed', () => app.quit())
}

module.exports = { start }
