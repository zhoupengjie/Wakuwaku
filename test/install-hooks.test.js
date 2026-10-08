const assert = require('node:assert/strict')
const { execFileSync } = require('node:child_process')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const { test } = require('node:test')

const { EVENTS } = require('../src/shared/hook-events')

const SCRIPT = path.join(__dirname, '..', 'scripts', 'install-hooks.js')

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
    groups.flatMap(g =>
      g.hooks
        .filter(h => String(h.url || '').includes('from=claude-pets') || String(h.command || '').includes('claude-hook.js'))
        .map(h => [event, g.matcher, h]),
    ),
  )
}

test('install adds one entry per event, HTTP except SessionStart, and can run twice', t => {
  const mine = { type: 'command', command: 'echo mine' }
  const file = tempSettings(t, { model: 'opus', hooks: { Stop: [{ hooks: [mine] }] } })

  run(file)
  const settings = run(file)

  assert.equal(settings.model, 'opus')
  assert.deepEqual(settings.hooks.Stop[0].hooks, [mine])
  const entries = ours(settings)
  assert.equal(entries.length, Object.keys(EVENTS).length)

  for (const [event, { matcher, via }] of Object.entries(EVENTS)) {
    const found = entries.filter(([e]) => e === event)
    assert.equal(found.length, 1, event)
    const [, gotMatcher, hook] = found[0]
    assert.equal(gotMatcher, matcher ? '*' : undefined, event)
    if (via === 'http') {
      assert.equal(hook.type, 'http', event)
      assert.equal(hook.timeout, event === 'PermissionRequest' ? 300 : 2, event)
      assert.match(hook.url, /^http:\/\/127\.0\.0\.1:\d+\/hook\?from=claude-pets$/, event)
    } else {
      assert.equal(hook.type, 'command', event)
      assert.equal(hook.async, true, event)
      assert.match(hook.command, /claude-hook\.js"$/, event)
    }
  }
  assert.ok(fs.existsSync(`${file}.claude-pets.bak`))
})

test('--http-only makes SessionStart an HTTP hook too, and switching back works', t => {
  const file = tempSettings(t)

  const all = ours(run(file, '--http-only'))
  assert.equal(all.length, Object.keys(EVENTS).length)
  assert.ok(all.every(([, , hook]) => hook.type === 'http'))

  const mixed = ours(run(file))
  assert.equal(mixed.length, Object.keys(EVENTS).length)
  assert.equal(mixed.find(([e]) => e === 'SessionStart')[2].type, 'command')
})

test('install replaces the command hooks an older version added', t => {
  const old = { type: 'command', command: 'node "D:/somewhere/hooks/claude-hook.js"', timeout: 5 }
  const file = tempSettings(t, {
    hooks: {
      PreToolUse: [{ matcher: '*', hooks: [old] }],
      Stop: [{ hooks: [old] }],
    },
  })

  const settings = run(file)
  assert.equal(ours(settings).length, Object.keys(EVENTS).length)
  assert.equal(settings.hooks.PreToolUse.length, 1)
  assert.equal(settings.hooks.PreToolUse[0].hooks[0].type, 'http')
})

test('uninstall takes back exactly what install added', t => {
  const before = { model: 'opus', hooks: { Stop: [{ hooks: [{ type: 'command', command: 'echo mine' }] }] } }
  const file = tempSettings(t, before)

  run(file)
  assert.deepEqual(run(file, '--uninstall'), before)
})

test('install creates the settings file when there is none', t => {
  const file = path.join(path.dirname(tempSettings(t)), 'sub', 'settings.json')

  assert.equal(ours(run(file)).length, Object.keys(EVENTS).length)
  assert.deepEqual(run(file, '--uninstall'), {})
})
