// The tray icon, whose face shows her mood, and the menu it shares with a
// right-click on her.
const path = require('path')
const { app, Menu, Tray, nativeImage } = require('electron')

const pets = require('./pets')
const { SCALES } = require('./config')

const TRAY_ICON = mood => path.join(__dirname, '..', 'assets', 'tray', `${mood}.png`)

// ctx: what the app shares (settings, T, pet, change, isVisible, setHidden,
// setDisplay, setOut, comeHome, petWindow, home).
function createTray(ctx) {
  let tray = null
  let trayMood = ''

  function menuItems({ isTray }) {
    const s = ctx.settings
    const T = ctx.T
    const found = pets.list()
    return [
      isTray
        ? {
            label: ctx.isVisible() ? T('menu.hide') : T('menu.show'),
            enabled: !s.dnd,
            click: () => ctx.setHidden(ctx.isVisible()),
          }
        : null,
      {
        label: T('menu.pet'),
        submenu: found.length
          ? found.map(({ id, name }) => ({ label: name, type: 'radio', checked: s.pet === id, click: () => ctx.change({ pet: id }) }))
          : [{ label: T('menu.noPets'), click: () => ctx.home.open() }],
      },
      {
        label: T('menu.size'),
        submenu: Object.entries(SCALES).map(([name, scale]) => ({
          label: T(`menu.${name}`),
          type: 'radio',
          checked: s.scale === scale,
          click: () => ctx.petWindow.resize({ scale }),
        })),
      },
      { label: T('menu.bubble'), type: 'checkbox', checked: s.bubble, click: item => ctx.change({ bubble: item.checked }) },
      { label: T('menu.walk'), type: 'checkbox', checked: s.walk, click: item => ctx.change({ walk: item.checked }) },
      { label: T('menu.look'), type: 'checkbox', checked: s.look, click: item => ctx.change({ look: item.checked }) },
      {
        label: T('menu.island'),
        type: 'checkbox',
        checked: s.display === 'island',
        click: item => ctx.setDisplay(item.checked ? 'island' : 'pet'),
      },
      // In island mode: let her out onto the desktop, or call her back in.
      s.display === 'island'
        ? { label: s.out ? T('menu.callBack') : T('menu.letOut'), click: () => ctx.setOut(!s.out) }
        : null,
      { label: T('menu.home'), click: () => ctx.comeHome() },
      { type: 'separator' },
      { label: T('menu.dnd'), type: 'checkbox', checked: s.dnd, click: item => ctx.change({ dnd: item.checked }) },
      { label: T('menu.settings'), click: () => ctx.home.open() },
      { type: 'separator' },
      { label: T('menu.quit'), click: () => app.quit() },
    ].filter(Boolean)
  }

  // The menu, over her or the tray.
  function popUp(win) {
    const menu = Menu.buildFromTemplate(menuItems({ isTray: !win }))
    if (win) menu.popup({ window: win })
    else tray?.popUpContextMenu(menu)
  }

  function create() {
    try {
      tray = new Tray(nativeImage.createFromPath(TRAY_ICON('idle')))
      trayMood = 'idle'
    } catch {
      tray = null
      return
    }
    if (process.platform === 'darwin') {
      tray.on('click', () => popUp())
    } else {
      // Click: show or hide her; right-click: the menu.
      tray.on('click', () => {
        if (ctx.settings.dnd) return ctx.home.open()
        ctx.setHidden(ctx.isVisible())
      })
      tray.on('right-click', () => popUp())
    }
    refresh()
  }

  function statusText() {
    const now = ctx.pet.get()
    return `${ctx.T(`mood.${now.mood}`)}${now.project ? ` · ${now.project}` : ''}`
  }

  function refresh() {
    if (!tray || !ctx.settings) return
    const T = ctx.T
    tray.setToolTip(T('tray.tooltip', { status: ctx.settings.dnd ? T('menu.dnd') : statusText() }))
    // The tray face shows her mood.
    const mood = ctx.pet.get().mood
    if (mood !== trayMood) {
      trayMood = mood
      try {
        tray.setImage(nativeImage.createFromPath(TRAY_ICON(mood)))
      } catch {}
    }
  }

  return { create, refresh, popUp }
}

module.exports = { createTray }
