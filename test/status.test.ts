import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { test } from 'node:test'
import { STRINGS, render } from '../src/shared/i18n.ts'
import * as Status from '../src/pet/status.ts'

const NOW = 1_000_000
const session = fields => ({ id: 'a', project: 'pet', mood: 'working', detail: '', since: NOW - 75_000, took: null, name: '', step: null, stepSince: null, todo: null, files: 0, reply: '', error: null, ...fields })

test('every text the Rust side makes has words in both languages', () => {
  const dir = path.join(import.meta.dirname, '..', 'src-tauri', 'src')
  const used = new Set()
  for (const file of ['events.rs', 'state.rs', 'widgets.rs', 'shared.rs']) {
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

test('detailsOf returns capped file list and mapped recent steps, or null when off or empty', () => {
  const files = Array.from({ length: 18 }, (_, i) => 'src/file' + (i + 1) + '.js')
  const recentSteps = [
    { step: { key: 'step.read', vars: { file: 'a.js' } }, ok: true },
    { step: { key: 'step.edit', vars: { file: 'b.js' } }, ok: false },
  ]
  const s = session({ fileList: files, files: 18, recentSteps })
  const det = Status.detailsOf('zh', s)
  assert.equal(det.files.length, 15)
  assert.equal(det.files[0], 'src/file1.js')
  assert.equal(det.totalFiles, 18)
  assert.equal(det.moreFiles, 3)
  assert.equal(det.steps.length, 2)
  assert.equal(det.steps[0].text, '读 a.js')
  assert.equal(det.steps[0].ok, true)
  assert.equal(det.steps[1].text, '改 b.js')
  assert.equal(det.steps[1].ok, false)

  // With specifics off: returns null
  assert.equal(Status.detailsOf('zh', s, { detailed: false }), null)

  // Empty files and empty steps: returns null
  assert.equal(Status.detailsOf('zh', session({ fileList: [], files: 0, recentSteps: [] })), null)
})

test('out on the desktop she says what the closed island says: nothing at rest', () => {
  assert.equal(Status.talkOf('zh', session({ mood: 'idle' })), null)
  assert.equal(Status.talkOf('zh', null), null)
  // At work: whose, what, its clock, the others busy.
  const busy = session({ name: '结算页', step: { key: 'step.edit', vars: { file: 'checkout.tsx' } }, stepSince: NOW - 41_000, others: 2 })
  assert.deepEqual(Status.talkOf('zh', busy, { now: NOW }), { mood: 'working', name: '结算页', what: '改 checkout.tsx', clock: '1:15', reply: '', others: 2 })
  // The specifics off: the project, and the tool's name alone.
  assert.equal(Status.talkOf('zh', busy, { now: NOW, detailed: false }).name, 'pet')
  // Done: how it ended, how long it took, Claude's last words.
  const done = session({ mood: 'done', took: 245_000, reply: '结算页改成了三步表单。' })
  assert.deepEqual(Status.talkOf('zh', done, { now: NOW }), { mood: 'done', name: 'pet', what: '搞定', clock: '用时 4:05', reply: '结算页改成了三步表单。', others: 0 })
  assert.equal(Status.talkOf('zh', done, { now: NOW, detailed: false }).reply, '')
  // An error says why.
  const failed = Status.talkOf('en', session({ mood: 'error', took: 5_000, error: { key: 'error.rate_limit' } }), { now: NOW })
  assert.match(failed.what, /^Error: /)
  assert.equal(failed.clock, 'took 0:05')
})

test('her list says each session in a line, a resting one as resting', () => {
  assert.deepEqual(Status.rowOf('zh', session({ id: 'b', mood: 'idle', since: null }), { now: NOW }), { id: 'b', mood: 'idle', name: 'pet', what: '摸鱼中', clock: '' })
  assert.deepEqual(Status.rowOf('zh', session({ mood: 'review', took: 65_000, files: 3 }), { now: NOW }), { id: 'a', mood: 'review', name: 'pet', what: '改好了', clock: '1:05' })
  assert.equal(Status.rowOf('en', session({ step: { key: 'step.edit', vars: { file: 'a.rs' } } }), { now: NOW }).what, 'Editing a.rs')
  // No project nor name: the mood's words in its place.
  assert.equal(Status.rowOf('zh', session({ project: '', mood: 'idle' }), { now: NOW }).name, '摸鱼中')
})
