// Third-party plugins in the island's page (src/pet/addons.ts): a message is
// a plugin's only when it comes from that plugin's frame, whatever it says;
// what each may ask; what each is told, and when; frames started, started
// over, stopped. And the SDK in a frame (src/addon/sdk.ts): the waku object
// over messages to the page.


// A host with fakes: its frames are plain objects; what it posts and asks is
// recorded; timers run when told; invoke answers as `answers` says.
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import vm from 'node:vm'
import { test } from 'node:test'
import * as A from '../src/pet/addons.ts'
import { pageCode } from './stripped.ts'

function setup(answers = {}) {
  const posted = []
  const asked = []
  const logs = []
  const dropped = []
  let t = 0
  const timers = []
  const made = []
  const host = A.createHost({
    invoke: (cmd, args) => {
      asked.push({ cmd, args })
      const a = answers[cmd]
      return Promise.resolve(typeof a === 'function' ? a(args) : (a ?? { ok: true }))
    },
    make: (id, src) => {
      const win = { name: `win-${id}-${made.length}` }
      const el = { id, src }
      made.push({ id, src, win, el })
      return { el, win }
    },
    drop: rec => dropped.push(rec.id),
    post: (win, message) => posted.push({ to: win.name, ...message }),
    log: line => logs.push(line),
    now: () => t,
    setTimer: (fn, ms) => {
      timers.push({ fn, at: t + ms, live: true })
      return timers.length
    },
    clearTimer: n => {
      if (n && timers[n - 1]) timers[n - 1].live = false
    },
  })
  const advance = ms => {
    t += ms
    for (const timer of timers) if (timer.live && timer.at <= t) {
      timer.live = false
      timer.fn()
    }
  }
  const flush = () => new Promise(r => setImmediate(r))
  return { host, posted, asked, logs, dropped, made, advance, flush, win: id => made.filter(m => m.id === id).at(-1)?.win }
}

const item = (id, extra = {}) => ({ id, version: '1.0.0', rev: `1.0.0:${id}`, main: 'main.js', perms: { net: [], agents: false, hardware: false, notify: false }, settings: {}, ...extra })

test('frames start once her language is known, one per plugin, at its own first page', () => {
  const s = setup()
  s.host.apply([item('hello'), item('Bad Id'), { nope: 1 }])
  assert.equal(s.made.length, 0, 'not before her language is known')
  s.host.setEnv({ lang: 'en', theme: 'light', quiet: false })
  assert.deepEqual(s.made.map(m => m.src), ['http://wakuplugin.localhost/hello/__waku/main.html'])
  // Ready: told who it is and how things are.
  s.host.receive(s.win('hello'), { waku: 1, t: 'ready' })
  assert.deepEqual(s.posted.at(-1), { to: s.win('hello').name, waku: 1, t: 'init', id: 'hello', version: '1.0.0', lang: 'en', theme: 'light', quiet: false, settings: {}, main: 'main.js' })
})

test('a message is the plugin’s whose frame sent it, whatever it says it is', async () => {
  const s = setup()
  s.host.setEnv({ lang: 'zh' })
  s.host.apply([item('hello'), item('rain-alert', { perms: { net: ['api.open-meteo.com'], notify: true } })])
  // From nobody's frame: nothing.
  assert.equal(s.host.receive({ name: 'stranger' }, { waku: 1, t: 'notify', n: 1, text: 'x', id: 'rain-alert' }), false)
  // From hello's, claiming to be rain-alert, with rain-alert's permission: hello's, refused.
  s.host.receive(s.win('hello'), { waku: 1, t: 'notify', n: 2, text: 'x', id: 'rain-alert' })
  s.host.receive(s.win('hello'), { waku: 1, t: 'storage', n: 3, op: 'set', key: 'k', value: 1, id: 'rain-alert' })
  await s.flush()
  assert.deepEqual(s.posted.filter(p => p.t === 'reply').map(p => [p.n, p.ok, p.error]), [
    [2, false, 'no notify permission'],
    [3, true, undefined],
  ])
  assert.deepEqual(s.asked.map(a => [a.cmd, a.args.id]), [['storage', 'hello']])
  // Not a message of hers at all.
  assert.equal(s.host.receive(s.win('hello'), { t: 'notify' }), false)
  assert.equal(s.host.receive(s.win('hello'), 'notify'), false)
})

test('what it asks goes to her in its name, its answers back to its frame', async () => {
  const s = setup({
    fetch: { status: 200, ok: true, headers: {}, text: '{}' },
    storage: { value: 7 },
    remove: { error: 'not running' },
  })
  s.host.setEnv({ lang: 'zh' })
  s.host.apply([item('hello')])
  const w = s.win('hello')
  s.host.receive(w, { waku: 1, t: 'fetch', n: 1, url: 'https://example.com/', method: 'GET', headers: { a: 1 } })
  s.host.receive(w, { waku: 1, t: 'storage', n: 2, op: 'get', key: 'k' })
  s.host.receive(w, { waku: 1, t: 'storage', n: 3, op: 'drop', key: 'k' })
  s.host.receive(w, { waku: 1, t: 'remove', n: 4, sub: 'two' })
  s.host.receive(w, { waku: 1, t: 'agents', n: 5 })
  s.host.receive(w, { waku: 1, t: 'hardware', n: 6 })
  await s.flush()
  assert.deepEqual(s.asked[0], { cmd: 'fetch', args: { id: 'hello', url: 'https://example.com/', method: 'GET', headers: { a: '1' }, body: null } })
  assert.deepEqual(s.asked[2].args.op, 'get', 'an op it has not: get')
  const replies = Object.fromEntries(s.posted.filter(p => p.t === 'reply').map(p => [p.n, p.ok ? p.value : `! ${p.error}`]))
  assert.deepEqual(replies, { 1: { status: 200, ok: true, headers: {}, text: '{}' }, 2: 7, 3: 7, 4: '! not running', 5: '! no agents permission', 6: '! no hardware permission' })
})

test('four requests at once at most; a widget’s words once a second, the last of them', async () => {
  let release
  const s = setup({ fetch: () => new Promise(r => (release = r)) })
  s.host.setEnv({ lang: 'zh' })
  s.host.apply([item('hello')])
  const w = s.win('hello')
  for (let n = 1; n <= 5; n++) s.host.receive(w, { waku: 1, t: 'fetch', n, url: 'https://example.com/' })
  await s.flush()
  assert.deepEqual(s.posted.filter(p => p.t === 'reply').map(p => [p.n, p.error]), [[5, '4 requests at once at most']])
  release({ status: 200, ok: true, headers: {}, text: '' })
  await s.flush()
  // Widgets: the first at once, those within its second as one at its end.
  s.host.receive(w, { waku: 1, t: 'widget', n: 10, sub: '', data: { value: '1' } })
  s.host.receive(w, { waku: 1, t: 'widget', n: 11, sub: '', data: { value: '2' } })
  s.host.receive(w, { waku: 1, t: 'widget', n: 12, sub: '', data: { value: '3' } })
  s.host.receive(w, { waku: 1, t: 'widget', n: 13, sub: 'two', data: { value: 'b' } })
  await s.flush()
  const sent = () => s.asked.filter(a => a.cmd === 'widget').map(a => [a.args.sub, a.args.data.value])
  assert.deepEqual(sent(), [
    [null, '1'],
    ['two', 'b'],
  ])
  s.advance(999)
  await s.flush()
  assert.equal(sent().length, 2)
  s.advance(1)
  await s.flush()
  assert.deepEqual(sent().at(-1), [null, '3'])
  const answered = s.posted.filter(p => p.t === 'reply' && p.n >= 10).map(p => p.n).sort()
  assert.deepEqual(answered, [10, 11, 12, 13])
  // Not JSON, or too big: refused here.
  s.host.receive(w, { waku: 1, t: 'widget', n: 20, data: { big: 'x'.repeat(70_000) } })
  assert.equal(s.posted.at(-1).error, 'a widget is JSON, up to 64 KB')
})

test('settings, language, theme and quiet told as they change; a changed plugin started over', () => {
  const s = setup()
  s.host.setEnv({ lang: 'zh', theme: 'dark', quiet: false })
  s.host.apply([item('hello', { settings: { Step: 1 } })])
  s.host.receive(s.win('hello'), { waku: 1, t: 'ready' })
  s.posted.length = 0
  s.host.apply([item('hello', { settings: { Step: 2 } })])
  s.host.setEnv({ lang: 'zh', theme: 'light', quiet: true })
  s.host.setEnv({ lang: 'zh', theme: 'light', quiet: true })
  assert.deepEqual(s.posted.map(p => [p.what, p.value]), [
    ['settings', { Step: 2 }],
    ['theme', 'light'],
    ['quiet', true],
  ])
  // Another version (or its manifest edited): a frame of its own again.
  s.host.apply([item('hello', { rev: '1.1.0:x', settings: { Step: 2 } })])
  assert.deepEqual(s.dropped, ['hello'])
  assert.equal(s.made.length, 2)
  // Gone from the list: stopped.
  s.host.apply([])
  assert.deepEqual(s.dropped, ['hello', 'hello'])
  assert.equal(s.host.recs.size, 0)
})

test('the agents to those that may and listen, once a second at most, and only what a plugin needs', () => {
  const s = setup()
  s.host.setEnv({ lang: 'zh' })
  s.host.apply([item('watcher', { perms: { agents: true } }), item('hello')])
  for (const id of ['watcher', 'hello']) {
    s.host.receive(s.win(id), { waku: 1, t: 'ready' })
    s.host.receive(s.win(id), { waku: 1, t: 'listen', what: 'agents' })
  }
  const told = () => s.posted.filter(p => p.what === 'agents')
  assert.deepEqual(told().map(p => p.to), [s.win('watcher').name], 'only one that may, at once as it listens')
  const session = { id: 's1', agent: 'codex', project: 'pet', mood: 'working', since: 5, detail: 'Editing a.rs', name: 'secret task', reply: 'secret words', jump: true }
  s.host.setAgents([session])
  assert.deepEqual(told().at(-1).value, [{ id: 's1', agent: 'codex', project: 'pet', mood: 'working', since: 5, detail: 'Editing a.rs' }])
  s.host.setAgents([{ ...session, mood: 'done' }])
  assert.equal(told().length, 2, 'the next within its second waits')
  s.advance(1000)
  assert.equal(told().at(-1).value[0].mood, 'done')
  s.host.setAgents([{ ...session, mood: 'done' }])
  s.advance(5000)
  assert.equal(told().length, 3, 'the same again: nothing told')
  assert.deepEqual(A.agentsOf([{ id: 'x', mood: 'dancing', detail: { key: 'k' } }, null, { id: '' }]), [{ id: 'x', agent: 'claude', project: '', mood: 'idle', since: null, detail: '' }])
})

test('the hardware read while one that may listens, slower while nobody is about', async () => {
  const s = setup({ hardware: { cpu: 12, memory: 40, gpu: null, netDown: 1, netUp: 2 } })
  s.host.setEnv({ lang: 'zh' })
  s.host.apply([item('meter', { perms: { hardware: true } })])
  s.host.receive(s.win('meter'), { waku: 1, t: 'ready' })
  s.advance(60_000)
  assert.equal(s.asked.length, 0, 'nobody listens: never read')
  s.host.receive(s.win('meter'), { waku: 1, t: 'listen', what: 'hardware' })
  s.advance(5000)
  await s.flush()
  assert.deepEqual(s.posted.filter(p => p.what === 'hardware').map(p => p.value.cpu), [12])
  s.host.setEnv({ quiet: true })
  s.advance(5000)
  await s.flush()
  assert.equal(s.asked.length, 1, 'quiet: three times as long')
  s.advance(10_000)
  await s.flush()
  assert.equal(s.asked.length, 2)
  s.host.apply([])
  s.advance(60_000)
  await s.flush()
  assert.equal(s.asked.length, 2, 'stopped: no more')
})

test('a frame loading its page again starts over; one that keeps on is stopped (crashed), and held until it changes', () => {
  const s = setup()
  s.host.setEnv({ lang: 'zh' })
  s.host.apply([item('hopper')])
  const load = () => s.host.loaded('hopper', s.host.recs.get('hopper').el)
  load()
  assert.equal(s.made.length, 1, 'its first load is its own')
  for (let i = 0; i < 3; i++) {
    load()
    load()
  }
  assert.equal(s.made.length, 4, 'started over each time')
  load()
  assert.equal(s.host.recs.size, 0, 'the fourth time in a minute: stopped')
  assert.ok(s.logs.at(-1).includes('stopped'))
  assert.deepEqual(s.asked.filter(a => a.cmd === 'stop').map(a => [a.args.id, a.args.why]), [['hopper', 'crashed']])
  s.host.apply([item('hopper')])
  assert.equal(s.host.recs.size, 0, 'held while it is the same')
  s.host.apply([item('hopper', { rev: 'edited' })])
  assert.equal(s.host.recs.size, 1, 'changed: tried again')
  // A load of a frame since dropped means nothing.
  s.host.loaded('hopper', { stale: true })
  assert.equal(s.made.length, 5)
})

test('its log lines and errors: thirty a minute at most, errors said on the Plugins page', async () => {
  const s = setup()
  s.host.setEnv({ lang: 'zh' })
  s.host.apply([item('chatty')])
  const w = s.win('chatty')
  for (let i = 0; i < 40; i++) s.host.receive(w, { waku: 1, t: 'log', line: `line ${i}` })
  assert.equal(s.logs.length, 30)
  assert.equal(s.logs[0], 'plugin chatty: line 0')
  s.advance(60_000)
  s.host.receive(w, { waku: 1, t: 'error', line: 'TypeError: x is undefined (main.js:3)' })
  await s.flush()
  assert.deepEqual(s.asked.at(-1), { cmd: 'said', args: { id: 'chatty', line: 'TypeError: x is undefined (main.js:3)' } })
  assert.equal(s.host.ping('chatty'), true)
  assert.equal(s.host.ping('nobody'), false)
})

// --- The watchdog -----------------------------------------------------------------------------
// Frames up and ready; each answers the pings it is sent when `answers` says.
type Watched = ReturnType<typeof setup> & {
  answer: (busy?: Record<string, number>) => void
  pings: () => number
  stops: () => any[]
  views: () => any[]
}
function watched(ids, answers: (id: string, n: string) => boolean = () => true) {
  const s = setup() as Watched
  s.host.setEnv({ lang: 'zh' })
  s.host.apply(ids.map(id => item(id)))
  for (const id of ids) s.host.receive(s.win(id), { waku: 1, t: 'ready' })
  // Each ping answered at once by those that do (as the SDK does), with their long tasks.
  s.answer = (busy = {}) => {
    for (const p of s.posted.filter(x => x.t === 'ping' && !x.seen)) {
      p.seen = true
      const m = s.made.find(x => x.win.name === p.to)
      if (m && answers(m.id, p.n)) s.host.receive(m.win, { waku: 1, t: 'pong', n: p.n, busy: busy[m.id] ?? 0 })
    }
  }
  s.pings = () => s.posted.filter(x => x.t === 'ping').length
  s.stops = () => s.asked.filter(a => a.cmd === 'stop').map(a => a.args)
  s.views = () => s.asked.filter(a => a.cmd === 'views').map(a => a.args.on)
  return s
}

test('the watchdog: a ping every 5 s while any plugin runs, none before a frame had time to load, no timer with none', () => {
  const s = watched(['ann', 'bob'])
  assert.equal(s.host.watching, true)
  s.advance(4999)
  assert.equal(s.pings(), 0)
  s.advance(1)
  // A fresh nonce per ping, so a plugin cannot forge an answer (not a plain
  // round counter): the recipients are what matter here.
  const first = s.posted.filter(x => x.t === 'ping')
  assert.deepEqual(first.map(x => x.to), [s.win('ann').name, s.win('bob').name])
  assert.ok(first.every(x => typeof x.n === 'string' && x.n) && first[0].n !== first[1].n)
  s.answer()
  s.advance(3000)
  s.advance(2000)
  assert.equal(s.pings(), 4, 'the next, 5 s on')
  s.answer()
  s.advance(3000)
  assert.deepEqual([s.dropped, s.stops()], [[], []], 'both answered: left alone')
  // Nobody about: three times as long.
  s.host.setEnv({ quiet: true })
  s.advance(14_999)
  assert.equal(s.pings(), 4)
  s.advance(1)
  assert.equal(s.pings(), 6)
  s.answer()
  s.host.apply([])
  assert.equal(s.host.watching, false, 'none running: no timer')
})

test('the watchdog: no answer within 3 s, loaded again (the views away a moment); again within ten minutes, stopped as hung', () => {
  let aAnswers = false
  const s = watched(['ann', 'bob'], id => id !== 'ann' || aAnswers)
  s.advance(5000)
  s.answer()
  s.advance(2999)
  assert.equal(s.dropped.length, 0, 'not before 3 s')
  s.advance(1)
  assert.deepEqual(s.dropped, ['ann'], 'a, and a alone')
  assert.deepEqual(s.views(), [false])
  s.advance(A.RELOAD_MS)
  assert.equal(s.made.filter(m => m.id === 'ann').length, 2, 'started again a moment later')
  assert.deepEqual(s.views(), [false, true])
  s.host.receive(s.win('ann'), { waku: 1, t: 'ready' })
  // It answers a while, then hangs again (b still answers): stopped.
  aAnswers = true
  for (let i = 0; i < 4; i++) {
    s.advance(1000)
    s.answer()
    s.advance(3000)
    s.advance(1000)
    s.answer()
  }
  assert.deepEqual(s.stops(), [])
  aAnswers = false
  s.advance(5000)
  s.answer()
  s.advance(3000)
  const [stop] = s.stops()
  assert.deepEqual([stop.id, stop.why, stop.times], ['ann', 'hung', 2])
  assert.equal(stop.since, 8000, 'since its first miss')
  assert.ok(!s.host.recs.has('ann') && s.host.recs.has('bob'))
  // Held, until she sends it changed (it is stopped in her: she will not send it).
  s.host.apply([item('ann'), item('bob')])
  assert.ok(!s.host.recs.has('ann'))
})

test('the watchdog: two misses more than ten minutes apart are not twice; a late check counts for nothing', () => {
  let answers = false
  const s = watched(['ann', 'bob'], id => id !== 'ann' || answers)
  s.advance(5000)
  s.answer()
  s.advance(3000)
  s.advance(A.RELOAD_MS)
  s.host.receive(s.win('ann'), { waku: 1, t: 'ready' })
  answers = true
  // Eleven minutes of answers.
  for (let i = 0; i < 132; i++) {
    s.advance(5000)
    s.answer()
  }
  answers = false
  s.advance(5000)
  s.answer()
  s.advance(3000)
  assert.deepEqual(s.stops(), [], 'loaded again, not stopped')
  assert.equal(s.made.filter(m => m.id === 'ann').length, 2)
  s.advance(A.RELOAD_MS)
  s.host.receive(s.win('ann'), { waku: 1, t: 'ready' })
  // A ping whose check comes 10 s late (the page held, the computer asleep): nothing.
  s.advance(4000)
  const before = s.dropped.length
  s.advance(10_000)
  assert.equal(s.dropped.length, before)
  assert.deepEqual(s.stops(), [])
})

test('the watchdog: all silent at once, no one blamed; all go, then start one at a time: the one after which they fall silent stopped', () => {
  // c holds the process they share: once it runs, nobody answers.
  let cRuns = true
  let hang = true
  const s = watched(['ann', 'bob', 'cat'], () => !hang)
  s.advance(5000)
  s.answer()
  s.advance(3000)
  assert.deepEqual(s.dropped.sort(), ['ann', 'bob', 'cat'], 'all taken away, the process with them')
  assert.deepEqual(s.views(), [false], 'the views too')
  assert.ok(s.host.recovering)
  assert.deepEqual(s.stops(), [], 'no one blamed for it')
  hang = false
  const step = id => {
    assert.equal(s.made.at(-1).id, id)
    s.host.receive(s.win(id), { waku: 1, t: 'ready' })
    if (id === 'cat' && cRuns) hang = true
    s.advance(2000)
    s.answer()
    s.advance(3000)
  }
  s.advance(A.RELOAD_MS)
  step('ann')
  step('bob')
  assert.ok(s.host.recs.has('ann') && s.host.recs.has('bob') && !s.stops().length, 'ann and bob answered, one by one; cat next, once bob had')
  step('cat')
  const [stop] = s.stops()
  assert.deepEqual([stop.id, stop.why, stop.times, stop.since], ['cat', 'hung', 2, 8000])
  assert.equal(s.host.recs.size, 0, 'the others taken away again (held with it)')
  hang = false
  cRuns = false
  s.advance(A.RELOAD_MS)
  step('ann')
  step('bob')
  assert.equal(s.host.recovering, null, 'done')
  assert.deepEqual([...s.host.recs.keys()].sort(), ['ann', 'bob'])
  assert.deepEqual(s.views(), [false, true], 'the views back')
  assert.equal(s.stops().length, 1, 'only c')
  // Watched as before.
  const n = s.pings()
  s.advance(5000)
  assert.equal(s.pings(), n + 2)
})

test('the watchdog: one that hangs a while after it starts is found the second time, each given as long as that took', () => {
  // cat holds the process they share 20 s after each start: then nobody answers.
  let clock = 0
  const s = watched(['ann', 'bob', 'cat'], () => !hung())
  const born = new Map()
  const hung = () => s.made.some(m => m.id === 'cat' && s.host.recs.get('cat')?.win === m.win && clock - born.get(m.win) >= 20_000)
  const tick = ms => {
    for (let i = 0; i < ms / 500; i++) {
      clock += 500
      s.advance(500)
      for (const m of s.made) {
        if (born.has(m.win)) continue
        born.set(m.win, clock)
        s.host.receive(m.win, { waku: 1, t: 'ready' })
      }
      s.answer()
    }
  }
  for (const m of s.made) born.set(m.win, 0)
  const hunts = () => s.logs.filter(l => /none answered/.test(l))
  tick(45_000)
  assert.equal(hunts().length, 1, 'all silent once cat hung')
  assert.match(hunts()[0], /2 s each/)
  assert.deepEqual(s.stops(), [], 'cat got past its 2 s: no one found')
  tick(30_000)
  assert.equal(hunts().length, 2, 'silent again once it hung again')
  assert.match(hunts()[1], /, 2[0-9] s each/, 'each given as long as that took')
  tick(120_000)
  assert.deepEqual(s.stops().map(x => [x.id, x.why]), [['cat', 'hung']], 'the second time: cat')
  assert.equal(s.host.recovering, null)
  assert.deepEqual([...s.host.recs.keys()].sort(), ['ann', 'bob'])
  assert.deepEqual(s.views().at(-1), true, 'the views back')
})

test('the watchdog: the first one started again not answering is not blamed at once: their old process may be there still', () => {
  let hang = true
  const s = watched(['ann', 'bob'], () => !hang)
  s.advance(5000)
  s.answer()
  s.advance(3000)
  assert.ok(s.host.recovering)
  // ann started again a while on, into the old process, held as it was: no answer.
  s.advance(A.RELOAD_MS)
  s.advance(2000)
  s.answer()
  s.advance(3000)
  assert.deepEqual(s.stops(), [])
  assert.match(s.logs.at(-1), /old process may be there still/)
  // That process gone by now: ann, then bob, each answering.
  hang = false
  s.advance(2 * A.RELOAD_MS)
  for (let i = 0; i < 2; i++) {
    s.advance(2000)
    s.answer()
    s.advance(3000)
  }
  assert.equal(s.host.recovering, null)
  assert.deepEqual(s.stops(), [], 'no one blamed')
  assert.deepEqual([...s.host.recs.keys()].sort(), ['ann', 'bob'])
})

test('the watchdog: one alone silent is loaded again, not the subject of a hunt', () => {
  const s = watched(['solo'], () => false)
  s.advance(5000)
  s.answer()
  s.advance(3000)
  assert.equal(s.host.recovering, null)
  assert.deepEqual(s.dropped, ['solo'])
})

test('the watchdog: over 2 s of its own long tasks in a minute, stopped as busy; older ones fall away', () => {
  const s = watched(['busy', 'calm'])
  const ping = (host, busy) => {
    host.advance(5000)
    host.answer(busy)
  }
  ping(s, { busy: 900, calm: 600 })
  ping(s, { busy: 900, calm: 0 })
  assert.deepEqual(s.stops(), [])
  ping(s, { busy: 300, calm: 0 })
  const [stop] = s.stops()
  assert.deepEqual([stop.id, stop.why, stop.times], ['busy', 'busy', 0])
  assert.ok(s.host.recs.has('calm') && !s.host.recs.has('busy'))
  // 1.9 s, and a minute on 0.6 s more: the first gone by then, not over.
  const t = watched(['calm'])
  ping(t, { calm: 1900 })
  for (let i = 0; i < 11; i++) ping(t, { calm: 0 })
  ping(t, { calm: 600 })
  assert.deepEqual(t.stops(), [])
  ping(t, { calm: 1500 })
  assert.deepEqual(t.stops().map(x => x.why), ['busy'], 'within its minute: over')
})

test('the watchdog: a forged pong does not count, so a hung plugin is still stopped', () => {
  const s = setup()
  s.host.setEnv({ lang: 'zh' })
  s.host.apply([item('good'), item('evil')])
  for (const id of ['good', 'evil']) s.host.receive(s.win(id), { waku: 1, t: 'ready' })
  const stops = () => s.asked.filter(a => a.cmd === 'stop').map(a => a.args)
  // good answers with the nonce she sent it; evil forges a huge fixed n, as
  // the review's evil did — it never knows the real one, so it never counts.
  const answer = () => {
    for (const p of s.posted.filter(x => x.t === 'ping' && !x.seen)) {
      p.seen = true
      const m = s.made.find(x => x.win.name === p.to)
      if (m) s.host.receive(m.win, { waku: 1, t: 'pong', n: m.id === 'good' ? p.n : 999999999 })
    }
  }
  s.advance(5000)
  answer()
  s.advance(3000)
  assert.deepEqual(s.dropped, ['evil'], 'evil alone missed; good counted, evil’s forgery did not')
  s.advance(A.RELOAD_MS)
  s.host.receive(s.win('evil'), { waku: 1, t: 'ready' })
  // Its new frame needs a moment to be pinged again; then it misses a second
  // time within ten minutes and is stopped (good, which answers, is left be).
  for (let i = 0; i < 2; i++) {
    s.advance(5000)
    answer()
    s.advance(3000)
  }
  const [stop] = stops()
  assert.equal(stop && stop.id, 'evil')
  assert.equal(stop && stop.why, 'hung')
  assert.ok(s.host.recs.has('good') && !s.host.recs.has('evil'))
})

test('a flood of messages from one frame is dropped before its data is read, one line said', async () => {
  const s = setup({ storage: { value: 1 } })
  s.host.setEnv({ lang: 'zh' })
  s.host.apply([item('loud')])
  s.host.receive(s.win('loud'), { waku: 1, t: 'ready' })
  const win = s.win('loud')
  s.advance(1000) // past the window the 'ready' was in
  for (let i = 1; i <= 70; i++) s.host.receive(win, { waku: 1, t: 'storage', op: 'get', key: 'k', n: i })
  await s.flush()
  const replies = s.posted.filter(p => p.t === 'reply')
  assert.equal(replies.length, 60, '60 a second read, the other 10 dropped')
  assert.ok(s.logs.some(l => /too many messages/.test(l)))
})

test('requests of every kind at once are capped per plugin, not only fetch', async () => {
  const s = setup({ storage: { value: 1 } })
  s.host.setEnv({ lang: 'zh' })
  s.host.apply([item('heavy')])
  s.host.receive(s.win('heavy'), { waku: 1, t: 'ready' })
  const win = s.win('heavy')
  for (let i = 1; i <= 12; i++) s.host.receive(win, { waku: 1, t: 'storage', op: 'get', key: 'k', n: i })
  // Four refused at once, before her side is asked (that reply is synchronous).
  const refused = s.posted.filter(p => p.t === 'reply' && !p.ok && /requests at once/.test(String(p.error)))
  assert.equal(refused.length, 4)
  await s.flush()
  // Only eight ever reached her side.
  assert.equal(s.asked.filter(a => a.cmd === 'storage').length, 8)
})

test('a fetch refused for the eight requests already out gives its place back', async () => {
  let release
  const pending = new Promise(r => (release = r))
  const s = setup({ storage: () => pending, fetch: { status: 200, ok: true, headers: {}, text: '' } })
  s.host.setEnv({ lang: 'zh' })
  s.host.apply([item('busy')])
  s.host.receive(s.win('busy'), { waku: 1, t: 'ready' })
  const win = s.win('busy')
  for (let i = 1; i <= 8; i++) s.host.receive(win, { waku: 1, t: 'storage', op: 'get', key: 'k', n: i })
  // Five fetches, one after another, each refused for the eight out (ask
  // answered at once, with no promise: its .finally threw, and each kept
  // one of the four places for fetches for good).
  for (let n = 11; n <= 15; n++) {
    s.host.receive(win, { waku: 1, t: 'fetch', n, url: 'https://example.com/' })
    await s.flush()
  }
  assert.deepEqual(
    s.posted.filter(p => p.t === 'reply' && p.n > 10).map(p => p.error),
    Array(5).fill('8 requests at once at most'),
  )
  release({ value: 1 })
  await s.flush()
  // The eight answered: a fetch goes through again.
  s.host.receive(win, { waku: 1, t: 'fetch', n: 20, url: 'https://example.com/' })
  await s.flush()
  assert.equal(s.posted.find(p => p.t === 'reply' && p.n === 20)?.ok, true)
})

test('a plugin cannot keep more widget entries than her side holds', () => {
  const s = setup()
  s.host.setEnv({ lang: 'zh' })
  s.host.apply([item('many')])
  s.host.receive(s.win('many'), { waku: 1, t: 'ready' })
  const win = s.win('many')
  let refused = 0
  for (let i = 0; i < 20; i++) {
    s.host.receive(win, { waku: 1, t: 'widget', sub: `s${i}`, data: { value: String(i) }, n: 100 + i })
    const r = s.posted.find(p => p.t === 'reply' && p.n === 100 + i)
    if (r && !r.ok && /widgets at most/.test(String(r.error))) refused++
  }
  // Eight subs kept, the rest turned away without an entry (no timer) made.
  assert.equal(refused, 12)
})

test('a message of its view goes to its main.js, with which card it came from', () => {
  const s = setup()
  s.host.setEnv({ lang: 'zh' })
  s.host.apply([item('board')])
  assert.equal(s.host.fromView('board', '', { shown: 'm' }), false, 'not before it is ready')
  s.host.receive(s.win('board'), { waku: 1, t: 'ready' })
  assert.equal(s.host.fromView('board', 'two', { shown: 'l' }), true)
  assert.deepEqual(s.posted.at(-1), { to: s.win('board').name, waku: 1, t: 'event', what: 'view', value: { message: { shown: 'l' }, sub: 'two' } })
  assert.equal(s.host.fromView('nobody', '', {}), false)
})

// --- The SDK, in a frame of its own ---------------------------------------------------------

// A frame for the SDK: its main's page (or a view's: page, and the name the
// page holding it gave it), a document to set its colours on; a browser
// that tells long tasks, or none (observer).
function frame({ page = '/hello/__waku/main.html', name = '', observer = null } = {}) {
  const sent = []
  const handlers = { message: [], error: [], unhandledrejection: [] }
  const parent = { postMessage: m => sent.push(JSON.parse(JSON.stringify(m))) }
  const timers = []
  const vars = {}
  const sheets = []
  // The SDK puts its waku on it.
  const window: Record<string, any> = {
    parent,
    name,
    location: { href: `http://wakuplugin.localhost${page}`, pathname: page },
    addEventListener: (type, fn) => handlers[type]?.push(fn),
    RTCPeerConnection: function () {},
  }
  const document = {
    documentElement: { style: { setProperty: (k, v) => (vars[k] = v) } },
    head: { appendChild: el => sheets.push(el.textContent) },
    createElement: () => ({ textContent: '' }),
  }
  const context = {
    window,
    document,
    location: window.location,
    URL,
    JSON,
    Promise,
    Error,
    TypeError,
    Object,
    Number,
    String,
    Array,
    Map,
    Set,
    Math,
    setTimeout: (fn, ms) => timers.push({ fn, ms }) && timers.length,
    clearTimeout: n => n && timers[n - 1] && (timers[n - 1].fn = null),
    ...(observer ? { PerformanceObserver: observer } : {}),
  }
  vm.createContext(context)
  // Its types taken out as build-pages.mjs does (lines and columns kept).
  const sdk = pageCode('addon', 'sdk.ts')
  vm.runInContext(sdk, context, {
    filename: 'sdk.ts',
    // Its main, as the frame would load it: here, nothing.
    importModuleDynamically: () => Promise.resolve(new vm.SyntheticModule([], () => {}, { context })),
  })
  const deliver = data => handlers.message.forEach(fn => fn({ source: parent, data }))
  return { sent, window, deliver, timers, handlers, parent, vars, sheets }
}

test('the SDK says it is ready, and is given its waku at init', async () => {
  const f = frame()
  assert.deepEqual(f.sent, [{ waku: 1, t: 'ready' }])
  assert.equal(f.window.RTCPeerConnection, undefined, 'WebRTC taken away')
  assert.equal(f.window.waku, undefined)
  // From anyone but the page holding it: nothing.
  f.handlers.message.forEach(fn => fn({ source: {}, data: { waku: 1, t: 'init', id: 'x' } }))
  assert.equal(f.window.waku, undefined)
  f.deliver({ waku: 1, t: 'init', id: 'hello', version: '1.0.0', lang: 'en', theme: 'light', quiet: false, settings: { Step: 2 }, main: 'main.js' })
  const waku = f.window.waku
  assert.equal(waku.id, 'hello')
  assert.equal(waku.lang, 'en')
  assert.equal(waku.settings.Step, 2)
  assert.throws(() => {
    'use strict'
    waku.settings.Step = 3
  })
  // A second init changes nothing.
  f.deliver({ waku: 1, t: 'init', id: 'other', version: '9.9.9', lang: 'zh', settings: {}, main: 'x.js' })
  assert.equal(f.window.waku.id, 'hello')
})

test('the SDK asks by message and gives back what the page answers', async () => {
  const f = frame()
  f.deliver({ waku: 1, t: 'init', id: 'hello', version: '1.0.0', lang: 'zh', theme: 'dark', quiet: false, settings: {}, main: 'main.js' })
  const waku = f.window.waku
  const got = waku.storage.get('count')
  const set = waku.widget.set('two', { value: 3, fn: () => 1 })
  const fetched = waku.fetch('https://api.open-meteo.com/v1/x', { headers: { accept: 'application/json' } })
  const asks = f.sent.filter(m => m.n)
  assert.deepEqual(asks.map(m => m.t), ['storage', 'widget', 'fetch'])
  assert.deepEqual(asks[1].data, { value: 3 }, 'as JSON has it')
  assert.deepEqual(asks[2].headers, { accept: 'application/json' })
  f.deliver({ waku: 1, t: 'reply', n: asks[0].n, ok: true, value: 41 })
  f.deliver({ waku: 1, t: 'reply', n: asks[1].n, ok: false, error: 'label: some words' })
  f.deliver({ waku: 1, t: 'reply', n: asks[2].n, ok: true, value: { status: 200, ok: true, headers: {}, text: '{"a":1}' } })
  assert.equal(await got, 41)
  await assert.rejects(set, /label: some words/)
  const r = await fetched
  assert.equal(r.status, 200)
  assert.deepEqual(await r.json(), { a: 1 })
  assert.throws(() => waku.on('nope', () => {}), /waku.on/)
})

test('the SDK’s every: at least a second, three times as long while nobody is about', () => {
  const f = frame()
  f.deliver({ waku: 1, t: 'init', id: 'hello', version: '1.0.0', lang: 'zh', theme: 'dark', quiet: false, settings: {}, main: 'main.js' })
  let calls = 0
  const stop = f.window.waku.every(0.1, () => calls++)
  assert.equal(f.timers.at(-1).ms, 1000)
  f.deliver({ waku: 1, t: 'event', what: 'quiet', value: true })
  assert.equal(f.timers.at(-1).ms, 3000)
  f.timers.at(-1).fn()
  assert.equal(calls, 1)
  stop()
  const last = f.timers.at(-1)
  assert.equal(last.fn, null, 'stopped')
  let seen = null
  f.window.waku.on('settings', v => (seen = v))
  f.deliver({ waku: 1, t: 'event', what: 'settings', value: { City: 'Dresden' } })
  assert.deepEqual({ ...seen }, { City: 'Dresden' })
  assert.equal(f.window.waku.settings.City, 'Dresden')
  // Its listening for the agents is told the page.
  f.window.waku.on('agents', () => {})
  assert.deepEqual(f.sent.at(-1), { waku: 1, t: 'listen', what: 'agents' })
})

test('the SDK answers the watchdog with its own long tasks since the last ping, none where the browser tells none', () => {
  let seen = null
  class Observer {
    static supportedEntryTypes = ['longtask', 'mark']
    constructor(fn) {
      seen = fn
    }
    observe() {}
  }
  const f = frame({ observer: Observer })
  f.deliver({ waku: 1, t: 'ping', n: 1 })
  assert.deepEqual(f.sent.at(-1), { waku: 1, t: 'pong', n: 1, busy: 0 }, 'before init too')
  // Its own (self) counted; one of another frame's in the same process not.
  seen({ getEntries: () => [{ name: 'self', duration: 120.4 }, { name: 'cross-origin-descendant', duration: 900 }, { name: 'self', duration: 60 }] })
  f.deliver({ waku: 1, t: 'ping', n: 2 })
  assert.deepEqual(f.sent.at(-1), { waku: 1, t: 'pong', n: 2, busy: 180 })
  f.deliver({ waku: 1, t: 'ping', n: 3 })
  assert.equal(f.sent.at(-1).busy, 0, 'counted from the last ping')
  const none = frame()
  none.deliver({ waku: 1, t: 'ping', n: 1 })
  assert.equal(none.sent.at(-1).busy, null)
})

test('the SDK hands main.js its view’s messages with the card they came from', () => {
  const f = frame()
  f.deliver({ waku: 1, t: 'init', id: 'board', version: '1.0.0', lang: 'zh', theme: 'dark', quiet: false, settings: {}, main: 'main.js' })
  const got = []
  f.window.waku.on('view', (message, from) => got.push([message, { ...from }]))
  f.deliver({ waku: 1, t: 'event', what: 'view', value: { message: { shown: 'm' }, sub: '' } })
  f.deliver({ waku: 1, t: 'event', what: 'view', value: { message: 7, sub: 'two' } })
  assert.deepEqual(got, [
    [{ shown: 'm' }, { sub: '' }],
    [7, { sub: 'two' }],
  ])
})

test('the SDK in a view: its waku there at once from its frame’s name, its colours on :root, told anew by messages', () => {
  const name = JSON.stringify({ waku: 1, id: 'board', size: 'l', data: { rows: [{ project: 'pet', minutes: 42 }] }, theme: 'light', lang: 'en', style: { fg: '#1b1b1f', muted: 'rgba(30,30,36,.62)', color: '#f5a35c', font: 'Segoe UI', fontDisplay: 'Segoe UI Variable Display', nope: 'x' } })
  const f = frame({ page: '/board/view.html', name })
  const waku = f.window.waku
  assert.ok(waku, 'no init needed')
  assert.deepEqual([waku.id, waku.size, waku.theme, waku.lang, waku.data.rows[0].minutes], ['board', 'l', 'light', 'en', 42])
  assert.deepEqual(f.vars, { '--waku-fg': '#1b1b1f', '--waku-muted': 'rgba(30,30,36,.62)', '--waku-color': '#f5a35c', '--waku-font': 'Segoe UI', '--waku-font-display': 'Segoe UI Variable Display' })
  assert.match(f.sheets[0], /background:transparent/)
  assert.deepEqual(f.sent, [{ waku: 1, t: 'ready', view: true }])
  assert.equal(waku.storage, undefined, 'a view has no storage, no web, no widget of its own')
  assert.equal(waku.fetch, undefined)
  // Pings answered; data, theme and language told as they change.
  f.deliver({ waku: 1, t: 'ping', n: 4 })
  assert.deepEqual(f.sent.at(-1), { waku: 1, t: 'pong', n: 4 })
  const heard = []
  waku.on('data', d => heard.push(['data', d]))
  waku.on('theme', d => heard.push(['theme', d]))
  f.deliver({ waku: 1, t: 'event', what: 'data', value: { rows: [] } })
  f.deliver({ waku: 1, t: 'event', what: 'theme', value: 'dark', style: { fg: '#f5f5f7' } })
  assert.deepEqual(heard, [
    ['data', { rows: [] }],
    ['theme', 'dark'],
  ])
  assert.equal(f.vars['--waku-fg'], '#f5f5f7')
  assert.deepEqual(waku.data, { rows: [] })
  // From anyone but the page holding it: nothing.
  f.handlers.message.forEach(fn => fn({ source: {}, data: { waku: 1, t: 'event', what: 'data', value: 'fake' } }))
  assert.deepEqual(waku.data, { rows: [] })
  // What it sends its main.js, as JSON has it.
  waku.send({ shown: 'l', fn: () => 1 })
  assert.deepEqual(f.sent.at(-1), { waku: 1, t: 'send', message: { shown: 'l' } })
  assert.throws(() => waku.on('settings', () => {}), /waku.on/)
  // No name given (or not hers): as a medium one, dark, Chinese, no data.
  const bare = frame({ page: '/board/view.html', name: 'something else' })
  assert.deepEqual([bare.window.waku.size, bare.window.waku.theme, bare.window.waku.data], ['m', 'dark', null])
})
