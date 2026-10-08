// The main window: sessions and prompts now, pets and the codex-pets.net
// gallery, the look, alerts, how Claude Code is connected, about. One at a
// time; it follows what happens through snapshots.
const path = require('path')
const { app, BrowserWindow, clipboard, ipcMain, shell } = require('electron')

const launch = require('./launch')
const pets = require('./pets')
const connection = require('./connection')
const { alive, handleOf } = require('./pet-window')
const petFetch = require('../shared/pet-fetch')

const ICON = path.join(__dirname, '..', 'assets', 'icon.png')

// ctx: what the app shares (settings, lang, pet, asks, change, applyPatch,
// petWindow, fullscreen).
function createHomeWindow(ctx) {
  let win = null
  let pushTimer

  // Everything the page shows.
  function snapshot() {
    return {
      settings: ctx.settings,
      lang: ctx.lang(),
      pets: pets.list(),
      now: ctx.pet.get(),
      sessions: ctx.pet.list(),
      asks: ctx.asks.views(),
      isVisible: ctx.petWindow.isVisible(),
      connection: connection.connection(ctx.port),
      pluginCommands: connection.PLUGIN_COMMANDS,
      hooks: connection.hooksStatus(ctx.port),
      settingsFile: launch.claudeSettingsFile(),
      loginAtStart: connection.isOpenAtLogin(),
      version: app.getVersion(),
      fullscreenAvailable: ctx.fullscreen?.isAvailable ?? false,
      platform: process.platform,
      isPackaged: app.isPackaged,
    }
  }

  // Tell the page now (after a change it asked for).
  function changed() {
    alive(win)?.webContents.send('home:changed', snapshot())
  }

  // Tell the page soon: it follows what happens, a few times a second at most.
  function push() {
    if (!alive(win) || pushTimer) return
    pushTimer = setTimeout(() => {
      pushTimer = undefined
      changed()
    }, 250)
  }

  function open() {
    if (alive(win)) {
      win.show()
      win.focus()
      return
    }
    win = new BrowserWindow({
      width: 860,
      height: 640,
      minWidth: 640,
      minHeight: 480,
      show: false,
      title: 'Wakuwaku',
      icon: ICON,
      autoHideMenuBar: true,
      webPreferences: {
        preload: path.join(__dirname, '..', 'home', 'preload.js'),
        contextIsolation: true,
        nodeIntegration: false,
      },
    })
    win.removeMenu()
    win.webContents.setWindowOpenHandler(() => ({ action: 'deny' }))
    win.webContents.on('will-navigate', e => e.preventDefault())
    win.loadFile(path.join(__dirname, '..', 'home', 'index.html'))
    win.once('ready-to-show', () => win.show())
    win.on('closed', () => {
      win = null
    })
  }

  ipcMain.handle('home:get', () => snapshot())
  ipcMain.handle('home:set', (_, patch) => ctx.applyPatch(patch))
  ipcMain.handle('home:login', (_, on) => {
    connection.setOpenAtLogin(on)
    return snapshot()
  })
  ipcMain.handle('home:hooks', (_, action) => {
    if (!['install', 'install-http', 'remove'].includes(action)) return { ok: false, snapshot: snapshot() }
    try {
      connection.writeHooks(action, ctx.port)
      return { ok: true, snapshot: snapshot() }
    } catch (err) {
      return { ok: false, error: err.message, snapshot: snapshot() }
    }
  })
  ipcMain.handle('home:fetch', async (_, ref) => {
    try {
      const got = await petFetch.downloadPet(String(ref ?? ''), pets.userDir())
      // The first pet, or one replacing a missing sprite, becomes the pet.
      if (!pets.spriteUrl(ctx.settings.pet) || pets.list().length === 1) ctx.change({ pet: got.id })
      else ctx.petWindow.send()
      return { ok: true, pet: got, warning: got.warning ? petFetch.describe(ctx.lang(), got.warning) : undefined, snapshot: snapshot() }
    } catch (err) {
      return { ok: false, error: petFetch.describe(ctx.lang(), err), snapshot: snapshot() }
    }
  })
  // Only these two places, whatever the page asks.
  ipcMain.handle('home:open-site', (_, where) => shell.openExternal(where === 'repo' ? 'https://github.com/zhoupengjie/wakuwaku' : petFetch.SITE))
  ipcMain.handle('home:copy', (_, text) => {
    clipboard.writeText(String(text ?? ''))
    return true
  })
  ipcMain.handle('home:answer', (_, id, choice) => ctx.asks.answer(id, choice))
  ipcMain.handle('home:dismiss', (_, id) => ctx.asks.dismiss(id))
  ipcMain.handle('home:show-pet', (_, on) => {
    ctx.petWindow.setHidden(on !== true)
    return snapshot()
  })

  // A page of codex-pets.net's gallery, fetched here. Only what the gallery
  // shows is passed on, and only pictures from the site itself.
  ipcMain.handle('home:gallery', async (_, { page = 1, sort = 'popular' } = {}) => {
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

  return {
    open,
    window: () => alive(win),
    handle: () => (alive(win) ? handleOf(win) : null),
    snapshot,
    changed,
    push,
  }
}

module.exports = { createHomeWindow }
