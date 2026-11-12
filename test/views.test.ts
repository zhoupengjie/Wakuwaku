// Plugins' own views in her cards (src/pet/views.js): where a card shows
// one and where it falls back to its widget's data, what a view's frame is
// given and told, its messages sent in the name of its card's widget alone,
// its pings, and what fails it for the session.


// A host with fakes: frames are plain objects (a card's children kept in
// card.kids); what it asks and sends, posts and logs is recorded; timers
// run when told; her answer for a widget's view as `views` says.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import * as V from '../src/pet/views.ts'

function setup({ views = {}, canShow = true } = {}) {
  let t = 0
  const timers = []
  const asked = []
  const sent = []
  const posted = []
  const logs = []
  const made = []
  let changes = 0
  const look = { theme: 'dark', lang: 'zh' }
  const host = V.createViews({
    ask: widget => {
      asked.push(widget)
      return Promise.resolve(views[widget] ?? { error: 'no view running' })
    },
    send: (widget, message) => sent.push([widget, message]),
    make: (name, page) => {
      const el: any = {
        name,
        page,
        parentElement: null,
        classes: new Set(),
        classList: { add: c => el.classes.add(c) },
        remove: () => {
          if (el.parentElement) el.parentElement.kids = el.parentElement.kids.filter(k => k !== el)
          el.parentElement = null
          el.removed = true
        },
      }
      const win = { name: `win-${made.length}` }
      made.push({ el, win })
      return { el, win }
    },
    move: (card, el) => {
      el.remove()
      el.removed = false
      el.parentElement = card
      card.kids.push(el)
    },
    // A card is in the page as it says; a frame, as its card is.
    live: x => x.live ?? !!x.parentElement?.live,
    post: (win, message) => posted.push({ to: win.name, ...message }),
    styleOf: w => ({ fg: '#f5f5f7', muted: 'rgba(235,235,245,.64)', color: w.color || '#a1a1a6', font: 'Segoe UI', fontDisplay: 'Segoe UI Variable Display' }),
    look: () => ({ ...look }),
    changed: () => changes++,
    log: line => logs.push(line),
    now: () => t,
    setTimer: (fn, ms) => {
      timers.push({ fn, at: t + ms, live: true })
      return timers.length
    },
    clearTimer: n => {
      if (n && timers[n - 1]) timers[n - 1].live = false
    },
    canShow,
  })
  const advance = ms => {
    t += ms
    for (const timer of timers) if (timer.live && timer.at <= t) {
      timer.live = false
      timer.fn()
    }
  }
  const flush = () => new Promise(r => setImmediate(r))
  const card = () => ({ live: true, kids: [] })
  return { host, asked, sent, posted, logs, made, advance, flush, card, look, changes: () => changes }
}

const board = (more = {}) => ({ id: 'agent-board', plugin: 'agent-board', from: 'addon', color: '#f5a35c', viewRev: 3, label: '最忙：pet', value: '42 分钟', ...more })
const theView = { page: 'http://wakuplugin.localhost/agent-board/view.html', data: { rows: [{ project: 'pet', minutes: 42 }] }, rev: 3 }

test('a view only where a plugin with one gave a view, at medium and large', () => {
  for (const [w, size] of [
    [board(), 's'],
    [board({ viewRev: undefined }), 'm'],
    [board({ from: 'script' }), 'm'],
    [{ id: 'weather', from: 'builtIn', viewRev: 3 }, 'l'],
    [null, 'm'],
  ])
    assert.equal(V.viewable(w, size), false, JSON.stringify([w, size]))
  assert.ok(V.viewable(board(), 'm') && V.viewable(board(), 'l'))
})

test('a card shows its data until its view’s page and data are had, then its frame, given all it needs in its name', async () => {
  const s = setup({ views: { 'agent-board': theView } })
  const card = s.card()
  assert.equal(s.host.place(card, 'w1:0', board(), 'm'), false, 'its data first')
  assert.equal(s.host.place(card, 'w1:0', board(), 'm'), false)
  await s.flush()
  assert.deepEqual(s.asked, ['agent-board'], 'asked once')
  assert.equal(s.changes(), 1, 'the page drawn again')
  assert.equal(s.host.place(card, 'w1:0', board(), 'm'), true)
  const [{ el, win }] = s.made
  assert.equal(el.page, theView.page)
  assert.deepEqual(card.kids, [el])
  assert.deepEqual(JSON.parse(el.name), { waku: 1, id: 'agent-board', size: 'm', data: theView.data, theme: 'dark', lang: 'zh', style: { fg: '#f5f5f7', muted: 'rgba(235,235,245,.64)', color: '#f5a35c', font: 'Segoe UI', fontDisplay: 'Segoe UI Variable Display' } })
  // Seen once it says it is ready.
  assert.equal(el.classes.has('ready'), false)
  s.host.receive(win, { waku: 1, t: 'ready', view: true })
  assert.equal(el.classes.has('ready'), true)
  // Placed again in the same card: the same frame, not another, nothing asked again.
  assert.equal(s.host.place(card, 'w1:0', board(), 'm'), true)
  await s.flush()
  assert.deepEqual([s.made.length, s.asked.length], [1, 1])
  // At another size, or another widget there now: a frame of its own.
  s.host.place(card, 'w1:0', board(), 'l')
  assert.equal(el.removed, true)
  // Her view not had (not running, none given): its data, nothing made.
  const t = setup()
  t.host.place(t.card(), 'w2:0', board(), 'm')
  await t.flush()
  assert.equal(t.made.length, 0)
})

test('its card drawn anew without it: a frame of its own in the new card; the page’s sweep takes those whose card went', async () => {
  const s = setup({ views: { 'agent-board': theView } })
  const old = s.card()
  s.host.place(old, 'w1:0', board(), 'm')
  await s.flush()
  s.host.place(old, 'w1:0', board(), 'm')
  old.live = false
  s.host.sweep()
  assert.equal(s.host.frames.size, 0)
  assert.equal(s.host.watching, false, 'no frame, no timer')
})

test('the fallback: nobody about or the desktop covered, Windows’ animations off, the watchdog’s while, a page that cannot keep it', async () => {
  for (const patch of [{ on: false }, { reduced: true }, { paused: true }]) {
    const s = setup({ views: { 'agent-board': theView } })
    const card = s.card()
    s.host.place(card, 'w1:0', board(), 'l')
    await s.flush()
    s.host.place(card, 'w1:0', board(), 'l')
    assert.equal(s.host.frames.size, 1)
    const before = s.changes()
    s.host.set(patch)
    assert.equal(s.host.frames.size, 0, `${JSON.stringify(patch)}: its frame gone`)
    assert.equal(s.made[0].el.removed, true)
    assert.ok(s.changes() > before, 'the page drawn again, from the data')
    assert.equal(s.host.place(card, 'w1:0', board(), 'l'), false)
    await s.flush()
    assert.equal(s.made.length, 1, 'none made meanwhile')
    // Back: shown again.
    s.host.set(Object.fromEntries(Object.entries(patch).map(([k, v]) => [k, !v])))
    s.host.place(card, 'w1:0', board(), 'l')
    await s.flush()
    assert.equal(s.host.place(card, 'w1:0', board(), 'l'), true)
  }
  const none = setup({ views: { 'agent-board': theView }, canShow: false })
  none.host.place(none.card(), 'w1:0', board(), 'm')
  await none.flush()
  assert.equal(none.made.length, 0)
})

test('a view that does not load, or does not answer twice in a row, is gone for the session: its data drawn instead', async () => {
  const s = setup({ views: { 'agent-board': theView } })
  const card = s.card()
  s.host.place(card, 'w1:0', board(), 'm')
  await s.flush()
  s.host.place(card, 'w1:0', board(), 'm')
  s.advance(V.LOAD_MS)
  assert.deepEqual(s.host.failed, new Set(['agent-board']))
  assert.match(s.logs.at(-1), /agent-board: its view did not load/)
  assert.equal(s.host.place(card, 'w1:0', board(), 'm'), false)
  await s.flush()
  assert.equal(s.made.length, 1, 'not tried again')
  // Pings: one missed is nothing, two in a row is.
  const p = setup({ views: { 'agent-board': theView } })
  const c = p.card()
  p.host.place(c, 'w1:0', board(), 'm')
  await p.flush()
  p.host.place(c, 'w1:0', board(), 'm')
  const { win } = p.made[0]
  p.host.receive(win, { waku: 1, t: 'ready' })
  const pongs = () => p.posted.filter(m => m.t === 'ping').at(-1).n
  p.advance(5000)
  p.advance(3000)
  assert.equal(p.host.failed.size, 0, 'one miss')
  p.advance(2000)
  p.host.receive(win, { waku: 1, t: 'pong', n: pongs() })
  p.advance(3000)
  p.advance(2000)
  p.advance(3000)
  assert.equal(p.host.failed.size, 0, 'answered between: counted afresh')
  p.advance(2000)
  p.advance(3000)
  assert.deepEqual(p.host.failed, new Set(['agent-board']), 'two in a row')
  assert.equal(p.host.frames.size, 0)
  assert.equal(p.host.watching, false)
})

test('a new revision of its view asked for and handed to it; her theme, language and its colour told as they change', async () => {
  const views = { 'agent-board': theView }
  const s = setup({ views })
  const card = s.card()
  s.host.place(card, 'w1:0', board(), 'm')
  await s.flush()
  s.host.place(card, 'w1:0', board(), 'm')
  const { win } = s.made[0]
  s.host.widgets([board()])
  await s.flush()
  assert.deepEqual(s.asked, ['agent-board'], 'the same revision: nothing asked')
  views['agent-board'] = { ...theView, data: { rows: [] }, rev: 4 }
  s.host.widgets([board({ viewRev: 4 })])
  await s.flush()
  assert.deepEqual(s.posted.at(-1), { to: win.name, waku: 1, t: 'event', what: 'data', value: { rows: [] } })
  s.host.widgets([board({ viewRev: 4 })])
  await s.flush()
  assert.equal(s.asked.length, 2)
  s.look.theme = 'light'
  s.look.lang = 'en'
  s.host.relook([board({ viewRev: 4, color: '#a46a00' })])
  const told = s.posted.slice(-2)
  assert.deepEqual(
    told.map(m => [m.what, m.value, m.style?.color]),
    [
      ['theme', 'light', '#a46a00'],
      ['lang', 'en', undefined],
    ],
  )
  s.host.relook([board({ viewRev: 4, color: '#a46a00' })])
  assert.equal(s.posted.length, told.length + s.posted.indexOf(told[0]), 'nothing new: nothing told')
})

test('what a view sends goes in the name of its card’s widget, known by its frame; ten a second, 64 KB at most', async () => {
  const s = setup({ views: { 'agent-board': theView } })
  const card = s.card()
  s.host.place(card, 'w1:0', board(), 'm')
  await s.flush()
  s.host.place(card, 'w1:0', board(), 'm')
  const { win } = s.made[0]
  // From nobody's frame: nothing.
  assert.equal(s.host.receive({ name: 'stranger' }, { waku: 1, t: 'send', message: 1 }), false)
  // Saying it is another widget's or plugin's changes nothing.
  s.host.receive(win, { waku: 1, t: 'send', message: { shown: 'm' }, widget: 'rain-alert', id: 'rain-alert' })
  await s.flush()
  assert.deepEqual(s.sent, [['agent-board', { shown: 'm' }]])
  for (let i = 0; i < 12; i++) s.host.receive(win, { waku: 1, t: 'send', message: i })
  await s.flush()
  assert.equal(s.sent.length, 10, 'ten in its second')
  s.advance(1000)
  s.host.receive(win, { waku: 1, t: 'send', message: 'x'.repeat(70_000) })
  s.host.receive(win, { waku: 1, t: 'send', message: 'ok' })
  await s.flush()
  assert.deepEqual(s.sent.at(-1), ['agent-board', 'ok'])
  assert.equal(s.sent.length, 11)
  // Its log lines, with its plugin's name; not a message of hers at all: nothing.
  s.host.receive(win, { waku: 1, t: 'error', line: 'TypeError: x (view.js:3)' })
  assert.equal(s.logs.at(-1), 'plugin agent-board view: error: TypeError: x (view.js:3)')
  assert.equal(s.host.receive(win, { t: 'send' }), false)
})
