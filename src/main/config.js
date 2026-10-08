// The window's settings, kept in Electron's userData folder.
const fs = require('fs')
const path = require('path')
const { app } = require('electron')

// The sizes she comes in.
const SCALES = { small: 0.4, medium: 0.55, large: 0.75 }

const DEFAULTS = {
  // 'auto' follows the system; or 'zh' / 'en'.
  lang: 'auto',
  // Claude小姐, downloaded from codex-pets.net on first run (app.js).
  pet: 'claude-chan',
  scale: 0.55,
  // 'pet' (the whole pet) or 'island' (a black pill at the top of the screen,
  // her home); with the island, whether she is out on the desktop.
  display: 'pet',
  out: false,
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
    // The capsule of earlier versions is the island now.
    if (saved.display === 'capsule') saved.display = 'island'
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

module.exports = { DEFAULTS, SCALES, load, save }
