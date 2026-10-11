const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')
const { test } = require('node:test')

const Tools = require('../src/pet/tools')
const { STRINGS } = require('../src/shared/i18n')

test('the calculator works sums out as arithmetic has it, never by eval', () => {
  assert.equal(Tools.calculate('1+2×3'), 7)
  assert.equal(Tools.calculate('(1+2)×3'), 9)
  assert.equal(Tools.calculate('10÷4'), 2.5)
  assert.equal(Tools.calculate('−3+5'), 2)
  assert.equal(Tools.calculate('2×−3'), -6)
  assert.equal(Tools.shown(Tools.calculate('0.1+0.2')), '0.3')
  assert.equal(Tools.calculate('1÷0'), null)
  assert.equal(Tools.calculate('1+'), null)
  assert.equal(Tools.calculate('alert(1)'), null)
  assert.equal(Tools.calculate('2**3'), null)
})

test('keys pressed on the calculator', () => {
  const press = keys => keys.reduce((s, k) => Tools.typed(s, k), '')
  assert.equal(press(['1', '2', '+', '3', '=']), '15')
  assert.equal(press(['1', '+', '×', '2']), '1×2')
  assert.equal(press(['+']), '')
  assert.equal(press(['−', '4']), '−4')
  assert.equal(press(['1', '.', '.', '5']), '1.5')
  assert.equal(press(['.', '5']), '0.5')
  assert.equal(press(['5', '0', '%']), '0.5')
  assert.equal(press(['1', '2', '⌫']), '1')
  assert.equal(press(['1', '2', 'C']), '')
})

test('sizes and times as they are shown', () => {
  assert.equal(Tools.size(0), '0 B')
  assert.equal(Tools.size(980), '980 B')
  assert.equal(Tools.size(1536), '1.5 KB')
  assert.equal(Tools.size(512 * 1024 ** 3), '512 GB')
  assert.equal(Tools.clock(299001), '5:00')
  assert.equal(Tools.clock(59000), '0:59')
  assert.equal(Tools.clock(3723000), '1:02:03')
  assert.equal(Tools.clock(-5), '0:00')
  const tokyo = Tools.timeIn('Asia/Tokyo', Date.UTC(2026, 9, 11, 12, 34, 56))
  assert.deepEqual(tokyo, { h: 21, m: 34, s: 56 })
})

test('each kind has a face and a card, with or without what tools.rs read', () => {
  const now = Date.UTC(2026, 9, 11, 12, 0, 0)
  const state = {
    media: { title: 'Song', artist: 'Singer', app: 'Spotify', playing: true, prev: true, next: true, toggle: true },
    trash: { items: 3, bytes: 1234567 },
    disk: [{ drive: 'C:', label: 'System', free: 100e9, total: 500e9 }],
    awake: { on: true, until: now + 60000 },
  }
  const apps = [{ app: 'a', name: 'Notepad', path: 'C:\\Windows\\notepad.exe', png: '', windows: [{ id: 1, title: 'note.txt' }] }]
  for (const kind of Tools.KINDS) {
    for (const x of [
      { lang: 'zh', entry: { id: 'wa', type: 'tool', of: kind }, cfg: {}, state: {}, local: {}, now },
      { lang: 'en', entry: { id: 'wa', type: 'tool', of: kind }, cfg: { path: 'C:\\x.exe', tz: 'Asia/Tokyo', style: 'analog', drive: 'C:' }, state, local: { expr: '1+2' }, now, apps, notes: { wa: 'hello' }, picked: ['#0080ff'], iconOf: () => null },
    ]) {
      const f = Tools.face(kind, x)
      assert.equal(typeof f.value, 'string', `${kind} value`)
      assert.ok(f.icon.includes('<svg'), `${kind} icon`)
      assert.equal(typeof Tools.card(kind, x), 'string', `${kind} card`)
    }
  }
})

test('what each says is there in both languages', () => {
  const keys = new Set()
  for (const kind of Tools.KINDS) keys.add(`tool.${kind}`).add(`tool.${kind}.note`)
  for (const w of ['lock', 'sleep', 'signout', 'restart', 'shutdown']) keys.add(`tool.power.${w}`)
  for (const [, key] of Tools.CITIES) keys.add(key)
  for (const file of ['src/pet/tools.js', 'src/pet/island.js']) {
    const text = fs.readFileSync(path.join(__dirname, '..', file), 'utf8')
    for (const m of text.matchAll(/'(tool\.[a-zA-Z.]+)'/g)) keys.add(m[1])
    for (const m of text.matchAll(/'(taskbar\.(?:w|lib)\.[a-zA-Z.]+)'/g)) keys.add(m[1])
  }
  assert.ok(keys.size > 80, `found only ${keys.size} keys: the scan is broken`)
  for (const key of keys) {
    assert.ok(key in STRINGS.zh, `zh missing: ${key}`)
    assert.ok(key in STRINGS.en, `en missing: ${key}`)
  }
})

test('a press: search, the picker, a launcher with a program, awake are done at once; the rest open their card', () => {
  const done = []
  const api = { call: (what, arg) => done.push([what, arg]), taskbar: what => done.push(['taskbar', what]) }
  const x = (kind, cfg = {}, state = {}) => ({ lang: 'zh', entry: { id: 'wa', type: 'tool', of: kind }, cfg, state, local: {}, now: 0 })
  assert.equal(Tools.press('search', x('search'), api), true)
  assert.equal(Tools.press('picker', x('picker'), api), true)
  assert.equal(Tools.press('launch', x('launch', { path: 'C:\\a.exe' }), api), true)
  assert.equal(Tools.press('launch', x('launch'), api), false)
  assert.equal(Tools.press('awake', x('awake', {}, { awake: { on: true } }), api), true)
  assert.equal(Tools.press('media', x('media'), api), false)
  assert.deepEqual(done, [['taskbar', 'search'], ['start', undefined], ['open', 'C:\\a.exe'], ['off', undefined]])
})

test('every tool tools.rs takes, tools.js has, and no other', () => {
  const text = fs.readFileSync(path.join(__dirname, '..', 'src-tauri', 'src', 'tools.rs'), 'utf8')
  const kinds = [...text.match(/pub const KINDS: \[&str; \d+\] = \[([^\]]*)\]/)[1].matchAll(/"(\w+)"/g)].map(m => m[1])
  assert.deepEqual([...kinds].sort(), [...Tools.KINDS].sort())
})

test('emptying the bin, signing out, restarting, shutting down: only once asked, and not by the click that asked', () => {
  const calls = []
  let local = {}
  const api = { call: (what, arg) => calls.push(arg ? `${what} ${arg}` : what), setLocal: patch => (local = { ...local, ...patch }) }
  const x = (kind, now) => ({ lang: 'zh', entry: { id: 'wa', type: 'tool', of: kind }, cfg: {}, state: {}, local, now })
  // Not asked: nothing.
  Tools.act('trash', 'empty', '', x('trash', 1000), api)
  Tools.act('power', 'do', 'restart', x('power', 1000), api)
  assert.deepEqual(calls, [])
  // Asked, and the second press of a double click: nothing.
  Tools.act('trash', 'ask', 'empty', x('trash', 1000), api)
  Tools.act('trash', 'empty', '', x('trash', 1200), api)
  assert.deepEqual(calls, [])
  // Answered.
  Tools.act('trash', 'empty', '', x('trash', 1500), api)
  assert.deepEqual(calls, ['empty'])
  // Asked for one thing, answered with another: nothing.
  local = {}
  Tools.act('power', 'ask', 'signout', x('power', 2000), api)
  Tools.act('power', 'do', 'shutdown', x('power', 3000), api)
  assert.deepEqual(calls, ['empty'])
  Tools.act('power', 'do', 'signout', x('power', 3000), api)
  // Locked at once.
  Tools.act('power', 'do', 'lock', x('power', 3000), api)
  assert.deepEqual(calls, ['empty', 'signout', 'lock'])
  // The question's Cancel comes first, not where the button that asked was.
  local = { confirm: 'restart', askedAt: 0 }
  const card = Tools.card('power', x('power', 0))
  assert.ok(card.indexOf('data-t="cancel"') < card.indexOf('data-t="do"'))
})
