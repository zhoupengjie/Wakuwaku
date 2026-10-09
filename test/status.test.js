const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')
const { test } = require('node:test')

const { STRINGS, render } = require('../src/shared/i18n')
const Status = require('../src/pet/status')

const NOW = 1_000_000
const session = fields => ({ id: 'a', project: 'pet', mood: 'working', detail: '', since: NOW - 75_000, took: null, name: '', step: null, stepSince: null, todo: null, files: 0, reply: '', error: null, ...fields })

test('every text the Rust side makes has words in both languages', () => {
  const dir = path.join(__dirname, '..', 'src-tauri', 'src')
  const used = new Set()
  for (const file of ['events.rs', 'state.rs', 'widgets.rs', 'main.rs']) {
    const text = fs.readFileSync(path.join(dir, file), 'utf8')
    for (const m of text.matchAll(/"((?:step|detail|say|error|widget)\.[A-Za-z_]+)"/g)) used.add(m[1])
    // error.{kind}, for each kind events.rs knows
    const kinds = text.match(/const KINDS: \[&str; \d+\] = \[([^\]]*)\]/)
    if (kinds) for (const k of kinds[1].matchAll(/"(\w+)"/g)) used.add(`error.${k[1]}`)
  }
  assert.ok(used.size > 25, `found only ${used.size} keys: the scan is broken`)
  for (const key of used) {
    assert.ok(key in STRINGS.zh, `missing in zh: ${key}`)
    assert.ok(key in STRINGS.en, `missing in en: ${key}`)
  }
})

test('a text may hold another one', () => {
  const approve = { key: 'detail.approve', vars: { what: { key: 'step.command', vars: { what: 'git push' } } } }
  assert.equal(render('zh', approve), '要批准：$ git push')
  assert.equal(render('en', approve), 'Needs your OK: $ git push')
})

test('a busy session says the step it is on, its to-do count, and for how long', () => {
  const s = session({ step: { key: 'step.edit', vars: { file: 'island.rs' } }, stepSince: NOW - 3_000 })
  assert.equal(Status.status('zh', s, { now: NOW }), '改 island.rs')
  assert.equal(Status.status('zh', { ...s, stepSince: NOW - 41_000 }, { now: NOW }), '改 island.rs · 已跑 0:41')
  assert.equal(Status.status('zh', { ...s, stepSince: NOW - 41_000 }, { now: NOW, withClock: false }), '改 island.rs')
  assert.equal(Status.status('en', { ...s, todo: { done: 3, total: 7, active: 'Running tests' } }, { now: NOW }), '3/7 · Editing island.rs')
  const thinking = session({ step: { key: 'step.thinking', vars: {} }, stepSince: NOW - 12_000 })
  assert.equal(Status.status('zh', thinking, { now: NOW }), '思考了 0:12')
  assert.equal(Status.status('zh', { ...thinking, stepSince: NOW - 2_000 }, { now: NOW }), '思考中')
  assert.equal(Status.time(s, NOW), '1:15')
})

test('waiting says what for; an ending how it ended', () => {
  const approve = { key: 'detail.approve', vars: { what: { key: 'step.command', vars: { what: 'git push' } } } }
  assert.equal(Status.status('zh', session({ mood: 'waiting', detail: approve })), '要批准：$ git push')
  assert.equal(Status.status('zh', session({ mood: 'review', files: 3 })), '改好了 · 3 个文件')
  assert.equal(Status.status('en', session({ mood: 'review', files: 1 })), 'Changed 1 file')
  assert.equal(Status.status('zh', session({ mood: 'review' })), STRINGS.zh['mood.review'])
  assert.equal(Status.status('zh', session({ mood: 'error', error: { key: 'error.rate_limit', vars: {} } })), '出错了：用量到上限了')
  assert.equal(Status.status('zh', session({ mood: 'error', error: '' })), STRINGS.zh['mood.error'])
})

test('the compact island: where a busy one is, whose an ending is', () => {
  const s = session({ name: '修复岛的设置', step: { key: 'step.read', vars: { file: 'a.rs' } }, stepSince: NOW - 60_000 })
  assert.equal(Status.brief('zh', s, { now: NOW }), '读 a.rs')
  assert.equal(Status.brief('zh', { ...s, mood: 'done' }), '搞定 · 修复岛的设置')
  assert.equal(Status.brief('zh', { ...s, mood: 'done', name: '' }), '搞定 · pet')
  assert.equal(Status.brief('zh', { ...s, mood: 'idle' }), '')
})

test('with the specifics off, only the mood, the project and the tool show', () => {
  const approve = { key: 'detail.approve', vars: { what: { key: 'step.command', vars: { what: 'rm -rf build' } } } }
  const off = { detailed: false }
  assert.equal(Status.brief('zh', session({ name: '秘密项目', step: { key: 'step.edit', vars: { file: 'secret.rs' } }, detail: 'Edit' }), off), 'pet · 干活')
  assert.equal(Status.status('zh', session({ step: { key: 'step.edit', vars: { file: 'secret.rs' } }, detail: 'Edit' }), off), `${STRINGS.zh['mood.working']} · Edit`)
  assert.equal(Status.status('zh', session({ mood: 'waiting', detail: approve }), off), STRINGS.zh['mood.waiting'])
  assert.equal(Status.status('zh', session({ mood: 'waiting', detail: { key: 'detail.asksQuestion' } }), off), STRINGS.zh['detail.asksQuestion'])
  assert.equal(Status.nameOf(session({ name: '秘密项目' }), false), 'pet')
})

test('the others: every busy session but the one shown, the most urgent found', () => {
  const list = [session({ id: 'a' }), session({ id: 'b', mood: 'idle' }), session({ id: 'c', mood: 'done' }), session({ id: 'd', mood: 'waiting' })]
  assert.deepEqual(Status.othersOf(list, list[0]).map(x => x.id), ['c', 'd'])
  assert.equal(Status.urgentOf(list, list[0]).id, 'd')
  assert.equal(Status.urgentOf(list, list[3]), undefined)
  assert.deepEqual(Status.othersOf(undefined, list[0]), [])
})
