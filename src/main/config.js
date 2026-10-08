// The window's settings, kept in Electron's userData folder.
const fs = require('fs')
const path = require('path')
const { app } = require('electron')

const DEFAULTS = {
  // 'auto' follows the system; or 'zh' / 'en'.
  lang: 'auto',
  pet: 'deepseek-chan',
  scale: 0.55,
  bubble: true,
  walk: true,
  look: true,
  // How long done / review / error stay: 'seen' (until the pointer is on
  // her) or a number of seconds.
  hold: 'seen',
  notify: { waiting: false, done: false, error: false },
  sound: false,
  // How long the prompt panel waits before leaving it to the terminal.
  promptWaitSec: 290,
  dnd: false,
  hideInFullscreen: true,
  // Shown the welcome once.
  onboarded: false,
  x: undefined,
  y: undefined,
}

function file() {
  return path.join(app.getPath('userData'), 'config.json')
}

function load() {
  try {
    const saved = JSON.parse(fs.readFileSync(file(), 'utf8'))
    return { ...DEFAULTS, ...saved, notify: { ...DEFAULTS.notify, ...(saved.notify || {}) } }
  } catch {
    return { ...DEFAULTS, notify: { ...DEFAULTS.notify } }
  }
}

function save(config) {
  try {
    fs.mkdirSync(path.dirname(file()), { recursive: true })
    fs.writeFileSync(file(), JSON.stringify(config, null, 2))
  } catch {
    // A settings file we cannot write only costs the remembered settings.
  }
}

module.exports = { DEFAULTS, load, save }
