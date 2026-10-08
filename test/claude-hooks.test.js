const assert = require('node:assert/strict')
const { test } = require('node:test')

const hooksConfig = require('../src/agents/claude-code/hooks')
const { EVENTS } = require('../src/agents/claude-code/events')

const here = { command: 'C:\\Apps\\Wakuwaku\\Wakuwaku.exe', args: [] }
const port = 47213

test('status tells installed, missing, http-only, partial and stale apart', () => {
  const ok = hooksConfig.install({}, { port, launch: here })
  assert.equal(hooksConfig.status(ok, { port, launch: here }), 'ok')
  assert.equal(hooksConfig.status({}, { port, launch: here }), 'missing')
  assert.equal(hooksConfig.status({ hooks: { Stop: [{ hooks: [{ type: 'command', command: 'echo mine' }] }] } }, { port, launch: here }), 'missing')

  const httpOnly = hooksConfig.install({}, { port, launch: here, httpOnly: true })
  assert.equal(hooksConfig.status(httpOnly, { port, launch: here }), 'httpOnly')

  const partial = structuredClone(ok)
  delete partial.hooks.Stop
  assert.equal(hooksConfig.status(partial, { port, launch: here }), 'partial')

  // Moved: the starter runs another copy.
  const moved = hooksConfig.install({}, { port, launch: { command: 'D:\\old\\Wakuwaku.exe', args: [] } })
  assert.equal(hooksConfig.status(moved, { port, launch: here }), 'stale')
  // Another port.
  assert.equal(hooksConfig.status(hooksConfig.install({}, { port: 1234, launch: here }), { port, launch: here }), 'stale')
})

test('the same copy is recognised whatever the slashes and case', () => {
  if (process.platform !== 'win32') return
  const settings = hooksConfig.install({}, { port, launch: { command: 'c:/apps/wakuwaku/wakuwaku.exe', args: [] } })
  assert.equal(hooksConfig.status(settings, { port, launch: here }), 'ok')
})

test('the starter is an argument list, so a path with spaces needs no quoting', () => {
  const settings = hooksConfig.install({}, { port, launch: here })
  const starter = settings.hooks.SessionStart[0].hooks.find(h => h.type === 'command')
  assert.deepEqual(starter, { type: 'command', command: here.command, args: [hooksConfig.ENSURE_FLAG], async: true, timeout: 15 })
})

test('install keeps other hooks, even ones in the same event; uninstall restores', () => {
  const mine = { hooks: { PreToolUse: [{ matcher: 'Bash', hooks: [{ type: 'command', command: 'echo mine' }] }] }, model: 'opus' }
  const installed = hooksConfig.install(mine, { port, launch: here })
  assert.equal(installed.hooks.PreToolUse.length, 2)
  assert.equal(Object.keys(installed.hooks).length, Object.keys(EVENTS).length)
  assert.ok(installed.hooks.SessionStart[0].hooks.every(x => x.type === 'command'))
  assert.deepEqual(hooksConfig.uninstall(installed), mine)
  // Twice is the same as once.
  assert.deepEqual(hooksConfig.install(installed, { port, launch: here }), installed)
})

test('only our entries are ours', () => {
  assert.equal(hooksConfig.isOurs({ type: 'command', command: 'echo mine' }), false)
  assert.equal(hooksConfig.isOurs({ type: 'http', url: 'http://127.0.0.1:47213/hook?from=wakuwaku' }), true)
  assert.equal(hooksConfig.isOurs({ type: 'command', command: 'x.exe', args: [hooksConfig.ENSURE_FLAG] }), true)
  assert.equal(hooksConfig.isOurs({ type: 'command', command: 'node "D:/x/hooks/claude-hook.js"' }), true)
})

test('hooks from before the rename (claude-pets) are ours, need a repair, and go on uninstall', () => {
  const old = {
    hooks: {
      Stop: [{ hooks: [{ type: 'http', url: 'http://127.0.0.1:47213/hook?from=claude-pets', timeout: 2 }] }],
      SessionStart: [{ hooks: [{ type: 'command', command: here.command, args: ['--claude-pets-ensure-running'], async: true }] }],
    },
  }
  assert.equal(hooksConfig.status(old, { port, launch: here }), 'stale')
  assert.deepEqual(hooksConfig.uninstall(old), {})
  const fixed = hooksConfig.install(old, { port, launch: here })
  assert.equal(hooksConfig.status(fixed, { port, launch: here }), 'ok')
  assert.ok(!JSON.stringify(fixed).includes('claude-pets'))
})

test('an HTTP hook on SessionStart (which Claude Code skips) counts as an old install', () => {
  const settings = hooksConfig.install({}, { port, launch: here })
  settings.hooks.SessionStart[0].hooks.unshift({ type: 'http', url: hooksConfig.hookUrl(port), timeout: 2 })
  assert.equal(hooksConfig.status(settings, { port, launch: here }), 'stale')
})

test('the plugin carries an HTTP hook for every event but SessionStart', () => {
  const hooks = hooksConfig.pluginHooks({ port })
  assert.deepEqual(Object.keys(hooks).sort(), [...hooksConfig.HTTP_EVENTS].sort())
  assert.ok(!('SessionStart' in hooks))
  for (const groups of Object.values(hooks)) assert.ok(groups.every(g => g.hooks.every(x => x.type === 'http' && x.url === hooksConfig.hookUrl(port))))
})

test('the plugin in the repo is what the code would build', () => {
  const fs = require('node:fs')
  const path = require('node:path')
  const root = path.join(__dirname, '..')
  const { plugin, marketplace, PLUGIN_NAME, MARKETPLACE_NAME } = require('../scripts/build-plugin')
  const read = p => JSON.parse(fs.readFileSync(path.join(root, p), 'utf8'))
  assert.deepEqual(read('integrations/claude-code/hooks/hooks.json'), { hooks: hooksConfig.pluginHooks({ port: 47213 }) }, 'run npm run build-plugin')
  assert.deepEqual(read('integrations/claude-code/.claude-plugin/plugin.json'), plugin(), 'run npm run build-plugin')
  assert.deepEqual(read('.claude-plugin/marketplace.json'), marketplace(), 'run npm run build-plugin')
  // Third-party plugin names may not start with "claude-".
  assert.ok(!/^claude-/.test(PLUGIN_NAME) && !/^claude-/.test(MARKETPLACE_NAME))
  assert.equal(hooksConfig.PLUGIN_ID, `${PLUGIN_NAME}@${MARKETPLACE_NAME}`)
})

test('the plugin counts as on only when enabled', () => {
  assert.equal(hooksConfig.isPluginEnabled({ enabledPlugins: { 'wakuwaku@wakuwaku': true } }), true)
  assert.equal(hooksConfig.isPluginEnabled({ enabledPlugins: { 'wakuwaku@wakuwaku': false } }), false)
  assert.equal(hooksConfig.isPluginEnabled({}), false)
  assert.equal(hooksConfig.isPluginEnabled(null), false)
})
