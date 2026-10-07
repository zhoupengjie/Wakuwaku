const assert = require('node:assert/strict')
const { execFileSync } = require('node:child_process')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const { test } = require('node:test')

const { EVENTS } = require('../hooks/claude-hook')

const SCRIPT = path.join(__dirname, '..', 'scripts', 'install-hooks.js')

function run(file, ...args) {
  execFileSync(process.execPath, [SCRIPT, '--settings', file, ...args], { stdio: 'pipe' })
  return JSON.parse(fs.readFileSync(file, 'utf8'))
}

function ours(settings) {
  return Object.entries(settings.hooks || {}).flatMap(([event, groups]) =>
    groups.flatMap(g => g.hooks.filter(h => h.command.includes('claude-hook.js')).map(() => [event, g.matcher])),
  )
}

test('install adds one entry per event, keeps the rest, and can run twice', t => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'claude-pets-'))
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }))
  const file = path.join(dir, 'settings.json')
  const mine = { type: 'command', command: 'echo mine' }
  fs.writeFileSync(file, JSON.stringify({ model: 'opus', hooks: { Stop: [{ hooks: [mine] }] } }))

  run(file)
  const settings = run(file)

  assert.equal(settings.model, 'opus')
  assert.deepEqual(settings.hooks.Stop[0].hooks, [mine])
  const entries = ours(settings)
  assert.equal(entries.length, Object.keys(EVENTS).length)
  for (const [event, hasMatcher] of Object.entries(EVENTS)) {
    const found = entries.filter(([e]) => e === event)
    assert.equal(found.length, 1, event)
    assert.equal(found[0][1], hasMatcher ? '*' : undefined, event)
  }
  assert.ok(fs.existsSync(`${file}.claude-pets.bak`))
})

test('uninstall takes back exactly what install added', t => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'claude-pets-'))
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }))
  const file = path.join(dir, 'settings.json')
  const before = { model: 'opus', hooks: { Stop: [{ hooks: [{ type: 'command', command: 'echo mine' }] }] } }
  fs.writeFileSync(file, JSON.stringify(before))

  run(file)
  assert.deepEqual(run(file, '--uninstall'), before)
})

test('install creates the settings file when there is none', t => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'claude-pets-'))
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }))
  const file = path.join(dir, 'sub', 'settings.json')

  assert.equal(ours(run(file)).length, Object.keys(EVENTS).length)
  assert.deepEqual(run(file, '--uninstall'), {})
})
