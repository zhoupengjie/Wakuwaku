const assert = require('node:assert/strict')
const { test } = require('node:test')

const hooksConfig = require('../src/shared/hooks-config')
const { EVENTS } = require('../src/shared/hook-events')

const here = { command: 'C:\\Apps\\Claude Pets\\Claude Pets.exe', args: [] }
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
  const moved = hooksConfig.install({}, { port, launch: { command: 'D:\\old\\Claude Pets.exe', args: [] } })
  assert.equal(hooksConfig.status(moved, { port, launch: here }), 'stale')
  // Another port.
  assert.equal(hooksConfig.status(hooksConfig.install({}, { port: 1234, launch: here }), { port, launch: here }), 'stale')
})

test('the same copy is recognised whatever the slashes and case', () => {
  if (process.platform !== 'win32') return
  const settings = hooksConfig.install({}, { port, launch: { command: 'c:/apps/claude pets/claude pets.exe', args: [] } })
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
  assert.deepEqual(hooksConfig.uninstall(installed), mine)
  // Twice is the same as once.
  assert.deepEqual(hooksConfig.install(installed, { port, launch: here }), installed)
})

test('only our entries are ours', () => {
  assert.equal(hooksConfig.isOurs({ type: 'command', command: 'echo mine' }), false)
  assert.equal(hooksConfig.isOurs({ type: 'http', url: 'http://127.0.0.1:47213/hook?from=claude-pets' }), true)
  assert.equal(hooksConfig.isOurs({ type: 'command', command: 'x.exe', args: [hooksConfig.ENSURE_FLAG] }), true)
  assert.equal(hooksConfig.isOurs({ type: 'command', command: 'node "D:/x/hooks/claude-hook.js"' }), true)
})
