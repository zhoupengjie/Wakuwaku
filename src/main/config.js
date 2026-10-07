// The window's settings, kept in Electron's userData folder.
const fs = require('fs')
const path = require('path')
const { app } = require('electron')

const DEFAULTS = {
  pet: 'deepseek-chan',
  scale: 0.55,
  bubble: true,
  walk: true,
  look: true,
  x: undefined,
  y: undefined,
}

function file() {
  return path.join(app.getPath('userData'), 'config.json')
}

function load() {
  try {
    return { ...DEFAULTS, ...JSON.parse(fs.readFileSync(file(), 'utf8')) }
  } catch {
    return { ...DEFAULTS }
  }
}

function save(config) {
  try {
    fs.mkdirSync(path.dirname(file()), { recursive: true })
    fs.writeFileSync(file(), JSON.stringify(config, null, 2))
  } catch {
    // A settings file we cannot write only costs the remembered position.
  }
}

module.exports = { load, save }
