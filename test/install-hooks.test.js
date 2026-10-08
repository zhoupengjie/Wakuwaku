const assert = require('node:assert/strict')
const { execFileSync } = require('node:child_process')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const { test } = require('node:test')

const { EVENTS } = require('../src/shared/hook-events')
const { HTTP_EVENTS } = require('../src/shared/hooks-config')
const hooksConfig = require('../src/shared/hooks-config')

const SCRIPT = path.join(__dirname, '..', 'scripts', 'install-hooks.js')
const ROOT = path.join(__dirname, '..')

function tempSettings(t, content) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'claude-pets-'))
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }))
  const file = path.join(dir, 'settings.json')
  if (content) fs.writeFileSync(file, JSON.stringify(content))
  return file
}

function run(file, ...args) {
  execFileSync(process.execPath, [SCRIPT, '--settings', file, ...args], { stdio: 'pipe' })
  return JSON.parse(fs.readFileSync(file, 'utf8'))
}

// Our entries, as [event, matcher, hook].
function ours(settings) {
  return Object.entries(settings.hooks || {}).flatMap(([event, groups]) =>
    groups.flatMap(g => g.hooks.filter(hooksConfig.isOurs).map(h => [event, g.matcher, h])),
  )
}

const launch = { command: require('electron'), args: [ROOT] }

test('the script installs an HTTP hook per event, plus the starter, and can run twice', t => {
  const mine = { type: 'command', command: 'echo mine' }
  const file = tempSettings(t, { model: 'opus', hooks: { Stop: [{ hooks: [mine] }] } })

  run(file)
  const settings = run(file)

  assert.equal(settings.model, 'opus')
  assert.deepEqual(settings.hooks.Stop[0].hooks, [mine])
  const entries = ours(settings)
  assert.equal(entries.length, HTTP_EVENTS.length + 1)

  // Claude Code runs no HTTP hook for SessionStart: none is installed for it.
  assert.equal(entries.filter(([e, , h]) => e === 'SessionStart' && h.type === 'http').length, 0)
  for (const event of HTTP_EVENTS) {
    const { matcher, timeout } = EVENTS[event]
    const http = entries.filter(([e, , h]) => e === event && h.type === 'http')
    assert.equal(http.length, 1, event)
    assert.equal(http[0][1], matcher ? '*' : undefined, event)
    assert.equal(http[0][2].url, 'http://127.0.0.1:47213/hook?from=claude-pets', event)
    assert.equal(http[0][2].timeout, timeout ?? 2, event)
  }

  // SessionStart also starts the pet: this copy's Electron, as an argument list.
  const [starter] = entries.filter(([, , h]) => h.type === 'command')
  assert.equal(starter[0], 'SessionStart')
  assert.equal(starter[2].async, true)
  assert.equal(starter[2].command, launch.command)
  assert.deepEqual(starter[2].args, [ROOT, hooksConfig.ENSURE_FLAG])
  assert.equal(hooksConfig.status(settings, { port: 47213, launch }), 'ok')
  assert.ok(fs.existsSync(`${file}.claude-pets.bak`))
})

test('--http-only leaves out the starter', t => {
  const file = tempSettings(t)
  const settings = run(file, '--http-only')
  assert.equal(ours(settings).length, HTTP_EVENTS.length)
  assert.ok(ours(settings).every(([, , h]) => h.type === 'http'))
  assert.equal(hooksConfig.status(settings, { port: 47213, launch }), 'httpOnly')
})

test('install replaces what older versions added', t => {
  const old = { type: 'command', command: 'node "D:/somewhere/hooks/claude-hook.js"', timeout: 5 }
  const oldHttp = { type: 'http', url: 'http://127.0.0.1:47213/hook?from=claude-pets', timeout: 2 }
  const file = tempSettings(t, { hooks: { SessionStart: [{ hooks: [old] }], Stop: [{ hooks: [oldHttp] }] } })

  assert.equal(hooksConfig.status(JSON.parse(fs.readFileSync(file, 'utf8')), { port: 47213, launch }), 'stale')
  const settings = run(file)
  assert.equal(ours(settings).length, HTTP_EVENTS.length + 1)
  assert.equal(settings.hooks.Stop.length, 1)
  assert.equal(hooksConfig.status(settings, { port: 47213, launch }), 'ok')
})

test('uninstall takes back exactly what install added', t => {
  const before = { model: 'opus', hooks: { Stop: [{ hooks: [{ type: 'command', command: 'echo mine' }] }] } }
  const file = tempSettings(t, before)
  run(file)
  assert.deepEqual(run(file, '--uninstall'), before)
})

test('install creates the settings file when there is none', t => {
  const file = path.join(path.dirname(tempSettings(t)), 'sub', 'settings.json')
  assert.equal(ours(run(file)).length, HTTP_EVENTS.length + 1)
  assert.deepEqual(run(file, '--uninstall'), {})
})
