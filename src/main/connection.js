// How Claude Code reaches her: the plugin, or hooks written into its
// settings.json. Read on demand (the person may change either at any time),
// and written only when they ask, from the main window.
const fs = require('fs')
const path = require('path')
const { app } = require('electron')

const launch = require('./launch')
const claudeHooks = require('../agents/claude-code/hooks')

const PLUGIN_COMMANDS = ['/plugin marketplace add zhoupengjie/wakuwaku', `/plugin install ${claudeHooks.PLUGIN_ID}`]

function readClaudeSettings() {
  try {
    return JSON.parse(fs.readFileSync(launch.claudeSettingsFile(), 'utf8'))
  } catch (err) {
    if (err.code === 'ENOENT') return {}
    throw err
  }
}

// Our entries in settings.json: 'ok', 'missing', 'stale' and so on (see
// hooks.status), or 'unreadable'.
function hooksStatus(port) {
  try {
    return claudeHooks.status(readClaudeSettings(), { port, launch: launch.launchSpec() })
  } catch {
    return 'unreadable'
  }
}

function isPluginEnabled() {
  try {
    return claudeHooks.isPluginEnabled(readClaudeSettings())
  } catch {
    return false
  }
}

// 'plugin', 'hooks' (written into settings.json), 'both' (every event
// twice), or 'none'.
function connection(port) {
  const plugin = isPluginEnabled()
  const hooks = hooksStatus(port)
  const isHooks = hooks !== 'missing' && hooks !== 'unreadable'
  return plugin && isHooks ? 'both' : plugin ? 'plugin' : isHooks ? 'hooks' : 'none'
}

// Change Claude Code's settings.json, backed up once beside itself.
function writeHooks(action, port) {
  const file = launch.claudeSettingsFile()
  const before = readClaudeSettings()
  const after =
    action === 'remove'
      ? claudeHooks.uninstall(before)
      : claudeHooks.install(before, { port, launch: launch.launchSpec(), httpOnly: action === 'install-http' })
  const backup = `${file}.wakuwaku.bak`
  if (fs.existsSync(file) && !fs.existsSync(backup)) fs.copyFileSync(file, backup)
  fs.mkdirSync(path.dirname(file), { recursive: true })
  fs.writeFileSync(file, `${JSON.stringify(after, null, 2)}\n`)
}

// Start with the system: this same app, run from where it is now.
function isOpenAtLogin() {
  try {
    return app.getLoginItemSettings(launch.launchSpecForLogin()).openAtLogin
  } catch {
    return false
  }
}

function setOpenAtLogin(on) {
  app.setLoginItemSettings({ ...launch.launchSpecForLogin(), openAtLogin: on === true })
}

module.exports = { PLUGIN_COMMANDS, hooksStatus, connection, writeHooks, isOpenAtLogin, setOpenAtLogin }
