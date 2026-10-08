// The claude-pets entries in Claude Code's settings.json: add, remove, check.
// Pure functions over the parsed settings, for scripts/install-hooks.js and
// the settings window.
//
// Every event is an HTTP hook: Claude Code POSTs it to the window, no process
// at all. SessionStart also gets a command, run in the background (async), that
// starts the window when it is not up, which no HTTP hook can do. That command
// is the app itself with ENSURE_FLAG, given as an argument list (no shell), so
// no Node is needed and no path is ever quoted.
const { EVENTS } = require('./hook-events')

const ENSURE_FLAG = '--claude-pets-ensure-running'
const URL_MARK = 'from=claude-pets'

// A path as Windows compares them: one slash, any case.
function samePath(p) {
  const text = String(p ?? '')
  return process.platform === 'win32' ? text.replace(/\\/g, '/').toLowerCase() : text
}

function hookUrl(port) {
  return `http://127.0.0.1:${port}/hook?${URL_MARK}`
}

// Ours: what this version installs, and what older ones did (a command
// running hooks/claude-hook.js).
function isOurs(hook) {
  const text = [hook?.url, hook?.command, ...(Array.isArray(hook?.args) ? hook.args : [])].map(String).join(' ')
  return text.includes(URL_MARK) || text.includes(ENSURE_FLAG) || text.includes('claude-hook.js')
}

// Drop our entries from every event, and events left empty. Returns a copy.
function strip(hooks) {
  const out = {}
  for (const [event, groups] of Object.entries(hooks || {})) {
    if (!Array.isArray(groups)) {
      out[event] = groups
      continue
    }
    const kept = groups
      .map(group => ({ ...group, hooks: (group.hooks || []).filter(h => !isOurs(h)) }))
      .filter(group => group.hooks.length > 0)
    if (kept.length) out[event] = kept
  }
  return out
}

// The events that get an HTTP hook: all but SessionStart, which Claude Code
// runs no HTTP hook for (it skips them). SessionStart gets the starter
// command instead, which also passes the event on to the pet.
const HTTP_EVENTS = Object.keys(EVENTS).filter(name => name !== 'SessionStart')

function httpGroups(port) {
  const out = {}
  for (const event of HTTP_EVENTS) {
    const { matcher, timeout } = EVENTS[event]
    const hooks = [{ type: 'http', url: hookUrl(port), timeout: timeout ?? 2 }]
    out[event] = [matcher ? { matcher: '*', hooks } : { hooks }]
  }
  return out
}

// What we add to settings.json, event by event: [{ matcher?, hooks: [...] }].
//   port     the window's port
//   launch   { command, args }: starts the app; omitted with httpOnly
//   httpOnly no command at all: nothing starts the window for you
function entries({ port, launch, httpOnly = false }) {
  const out = httpGroups(port)
  if (!httpOnly) {
    out.SessionStart = [{ hooks: [{ type: 'command', command: launch.command, args: [...launch.args, ENSURE_FLAG], async: true, timeout: 15 }] }]
  }
  return out
}

// The hooks the Claude Code plugin carries: HTTP only (a plugin cannot know
// where the app is, so it cannot start her).
function pluginHooks({ port }) {
  return httpGroups(port)
}

// settings with our entries (re)placed. Returns a new object.
function install(settings, options) {
  const hooks = strip(settings?.hooks)
  for (const [event, groups] of Object.entries(entries(options))) {
    hooks[event] = [...(hooks[event] || []), ...groups]
  }
  return { ...(settings || {}), hooks }
}

// settings without our entries. Returns a new object.
function uninstall(settings) {
  const out = { ...(settings || {}) }
  const hooks = strip(settings?.hooks)
  if (Object.keys(hooks).length) out.hooks = hooks
  else delete out.hooks
  return out
}

// How our entries in settings.json stand against what this copy would install:
//   'missing'   none
//   'ok'        all there, starting this copy
//   'httpOnly'  all there, nothing starts the window
//   'stale'     there, but for another port or another copy (moved, or an old version)
//   'partial'   some events lack them
function status(settings, { port, launch }) {
  const found = {}
  for (const [event, groups] of Object.entries(settings?.hooks || {})) {
    if (!Array.isArray(groups)) continue
    const ours = groups.flatMap(g => (g.hooks || []).filter(isOurs))
    if (ours.length) found[event] = ours
  }

  const names = Object.keys(EVENTS)
  const present = names.filter(name => found[name])
  if (!present.length) return 'missing'

  const wantUrl = hookUrl(port)
  const isOld =
    names.some(name => (found[name] || []).some(h => h.type !== 'http' && !(Array.isArray(h.args) && h.args.includes(ENSURE_FLAG)))) ||
    (found.SessionStart || []).some(h => h.type === 'http')
  const isOtherPort = present.some(name => found[name].some(h => h.type === 'http' && h.url !== wantUrl))
  const starters = (found.SessionStart || []).filter(h => h.type === 'command')
  const want = [launch.command, ...launch.args, ENSURE_FLAG].map(samePath)
  const isOtherCopy = starters.some(h => JSON.stringify([h.command, ...(h.args || [])].map(samePath)) !== JSON.stringify(want))
  if (isOld || isOtherPort || isOtherCopy) return 'stale'
  if (HTTP_EVENTS.some(name => !found[name])) return 'partial'
  return starters.length ? 'ok' : 'httpOnly'
}

// Whether the desk-pet plugin is turned on in these settings.
const PLUGIN_ID = 'desk-pet@desk-pet'

function isPluginEnabled(settings) {
  return settings?.enabledPlugins?.[PLUGIN_ID] === true
}

module.exports = { ENSURE_FLAG, PLUGIN_ID, HTTP_EVENTS, hookUrl, isOurs, install, uninstall, status, pluginHooks, isPluginEnabled }
