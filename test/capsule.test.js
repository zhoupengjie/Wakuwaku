const assert = require('node:assert/strict')
const { test } = require('node:test')

const C = require('../src/pet/capsule')
const G = require('../src/pet/deskgrid')

// Her capsule as a container (the KDE-like picker of 2026-10-17): its
// members' order and modes (capsule.js), and what the island makes of them
// (deskgrid.js placement).
const all = [
  { id: 'monitor', builtIn: true, on: true },
  { id: 'today', builtIn: true, on: true },
  { id: 'tokens', builtIn: true, on: true },
  { id: 'weather', plugin: 'weather', on: true },
  { id: 'stock-aapl', plugin: 'stock', on: true },
  { id: 'stock-nvda', plugin: 'stock', on: true },
  { id: 'off', on: false },
]
const ids = list => list.map(w => w.id)
const keys = list => list.map(m => m.key)
const modes = list => Object.fromEntries(list.map(m => [m.key, m.mode]))

test('without the setting: the widgets in their order, a plugin once, the monitor pinned by its pin in the island home only', () => {
  const island = C.derive(all, { home: 'island' })
  assert.deepEqual(keys(island), ['widget:monitor', 'widget:today', 'widget:tokens', 'plugin:weather', 'plugin:stock', 'widget:off'])
  assert.equal(modes(island)['widget:monitor'], 'pin')
  assert.equal(modes(C.derive(all, { home: 'island', pin: false }))['widget:monitor'], 'turn')
  // The taskbar her home: its readings are the strip's part, or it takes turns (#53).
  assert.equal(modes(C.derive(all, { home: 'taskbar' }))['widget:monitor'], 'turn')
  assert.ok(island.filter(m => m.key !== 'widget:monitor').every(m => m.mode === 'turn'))
})

test('the members as they are: the list order, those on and not listed at the end, those off or elsewhere left out', () => {
  const capsule = { members: [{ key: 'plugin:stock', mode: 'turn' }, { key: 'widget:gone', mode: 'turn' }, { key: 'widget:today', mode: 'open' }, { key: 'widget:monitor', mode: 'pin' }] }
  const inCapsule = w => w.on !== false && w.id !== 'tokens'
  const m = C.effective({ capsule, all, home: 'island', inCapsule })
  assert.deepEqual(keys(m), ['plugin:stock', 'widget:today', 'widget:monitor', 'plugin:weather'])
  assert.deepEqual(ids(m[0].widgets), ['stock-aapl', 'stock-nvda'])
  assert.deepEqual(modes(m), { 'plugin:stock': 'turn', 'widget:today': 'open', 'widget:monitor': 'pin', 'plugin:weather': 'turn' })
  // The island home's monitor: its pin says pinned or not; 'open' is the capsule's own.
  assert.equal(modes(C.effective({ capsule, all, home: 'island', pin: false, inCapsule }))['widget:monitor'], 'turn')
  const open = { members: [{ key: 'widget:monitor', mode: 'open' }] }
  assert.equal(modes(C.effective({ capsule: open, all, home: 'island' }))['widget:monitor'], 'open')
  // A list written by hand with three pinned: the third takes turns.
  const three = { members: ['today', 'tokens', 'off'].map(id => ({ key: `widget:${id}`, mode: 'pin' })) }
  const m3 = C.effective({ capsule: three, all: all.map(w => ({ ...w, on: true })), home: 'taskbar' })
  assert.deepEqual(m3.filter(x => x.mode === 'pin').map(x => x.key), ['widget:today', 'widget:tokens'])
  // Junk kept out: unknown modes, keys, twice the same.
  assert.deepEqual(C.storedOf({ members: [{ key: 'widget:a', mode: 'x' }, { key: 'bad', mode: 'turn' }, { key: 'widget:b', mode: 'turn' }, { key: 'widget:b', mode: 'pin' }] }), [{ key: 'widget:b', mode: 'turn' }])
  assert.equal(C.storedOf(undefined), null)
})

test('two pinned at most: a third refused; turns and open always', () => {
  let list = [{ key: 'widget:a', mode: 'turn' }, { key: 'widget:b', mode: 'turn' }, { key: 'widget:c', mode: 'turn' }]
  list = C.setMode(list, 'widget:a', 'pin')
  list = C.setMode(list, 'widget:b', 'pin')
  assert.equal(C.canPin(list, 'widget:c'), false)
  assert.equal(C.canPin(list, 'widget:a'), true)
  assert.deepEqual(modes(C.setMode(list, 'widget:c', 'pin')), { 'widget:a': 'pin', 'widget:b': 'pin', 'widget:c': 'turn' })
  assert.deepEqual(modes(C.setMode(list, 'widget:c', 'open')), { 'widget:a': 'pin', 'widget:b': 'pin', 'widget:c': 'open' })
  // One of the two let go: the third may.
  assert.equal(C.canPin(C.setMode(list, 'widget:a', 'turn'), 'widget:c'), true)
  // Added pinned with two there: takes turns.
  assert.equal(modes(C.add(list, 'widget:d', -1, 'pin'))['widget:d'], 'turn')
  // Nothing for a key not there, or a mode not known.
  assert.deepEqual(C.setMode(list, 'widget:z', 'pin'), list)
  assert.deepEqual(C.setMode(list, 'widget:c', 'big'), list)
})

test('the order by hand: moved, added at its end or a place, taken out and put back where it was (undo)', () => {
  const list = ['a', 'b', 'c', 'd'].map(k => ({ key: `widget:${k}`, mode: 'turn' }))
  assert.deepEqual(keys(C.move(list, 0, 2)), ['widget:b', 'widget:c', 'widget:a', 'widget:d'])
  assert.deepEqual(keys(C.move(list, 3, 0)), ['widget:d', 'widget:a', 'widget:b', 'widget:c'])
  assert.deepEqual(keys(C.move(list, 1, 9)), keys(list))
  assert.deepEqual(keys(C.add(list, 'plugin:weather')), [...keys(list), 'plugin:weather'])
  assert.deepEqual(keys(C.add(list, 'plugin:weather', 1)), ['widget:a', 'plugin:weather', 'widget:b', 'widget:c', 'widget:d'])
  assert.deepEqual(keys(C.add(list, 'widget:b')), keys(list), 'there already: as it was')
  const pinned = C.setMode(list, 'widget:c', 'pin')
  const { list: out, record } = C.remove(pinned, 'widget:c')
  assert.deepEqual(keys(out), ['widget:a', 'widget:b', 'widget:d'])
  assert.deepEqual(record, { key: 'widget:c', mode: 'pin', index: 2 })
  const back = C.restore(out, record)
  assert.deepEqual(keys(back), keys(list))
  assert.equal(modes(back)['widget:c'], 'pin')
  // Put back into a shorter list: at its end.
  assert.deepEqual(keys(C.restore([{ key: 'widget:a', mode: 'turn' }], record)), ['widget:a', 'widget:c'])
  assert.equal(C.remove(list, 'widget:z').record, null)
})

test('written as the capsule and, for the Rust side of now, widgetOrder and the monitor pin', () => {
  const list = [{ key: 'widget:monitor', mode: 'turn' }, { key: 'plugin:weather', mode: 'pin' }, { key: 'widget:today', mode: 'open' }]
  const p = C.patchFor({ widgetSpin: 5 }, list, 'island')
  assert.deepEqual(p.capsule, { members: list, spin: 5 })
  assert.deepEqual(p.widgetOrder, ['monitor', 'weather', 'today'])
  assert.deepEqual(p.monitor, { pin: false })
  // Pinned and its pin on already: nothing more to say; the taskbar home: never.
  assert.equal(C.patchFor({ monitor: { pin: true, cpu: true } }, C.setMode(list, 'widget:monitor', 'pin'), 'island').monitor, undefined)
  assert.equal(C.patchFor({}, list, 'taskbar').monitor, undefined)
  // The turns' length: widgetSpin, the panel setting both.
  assert.equal(C.spinOf({}), 8)
  assert.equal(C.spinOf({ widgetSpin: 15, capsule: { spin: 5 } }), 15)
  assert.deepEqual(C.spinPatch({ capsule: { members: list } }, 5), { widgetSpin: 5, capsule: { members: list, spin: 5 } })
  assert.equal(C.spinPatch({}, 7), null)
})

test('in the island: members in order (turns, pinned at its end, the open ones listed only), then those elsewhere with where they are', () => {
  const capsule = {
    members: [
      { key: 'plugin:weather', mode: 'pin' },
      { key: 'widget:tokens', mode: 'turn' },
      { key: 'widget:today', mode: 'open' },
      { key: 'plugin:stock', mode: 'turn' },
    ],
  }
  const desk = [{ id: 'wa', type: 'widget', of: 'monitor', size: 'm' }]
  const p = G.placement({ all, home: 'island', desk, capsule })
  assert.deepEqual(ids(p.turning), ['tokens', 'stock-aapl', 'stock-nvda'])
  assert.deepEqual(ids(p.pins), ['weather'])
  assert.equal(p.pinned, false)
  assert.deepEqual(ids(p.listed), ['weather', 'tokens', 'today', 'stock-aapl', 'stock-nvda', 'monitor'])
  assert.equal(p.where.get('monitor'), 'desk')
  assert.equal(p.where.has('weather'), false)
  // The taskbar her home: one on the strip is listed as there; its monitor part shows its readings.
  const bar = { left: ['her'], center: ['start', 'apps'], right: [{ id: 'w1', type: 'plugin', of: 'stock', size: 'M' }, 'monitor', 'tray', 'keys', 'quick', 'clock'] }
  const q = G.placement({ all, home: 'taskbar', taskbar: bar, capsule })
  assert.equal(q.pinned, true)
  assert.deepEqual(ids(q.turning), ['tokens'])
  assert.deepEqual(ids(q.listed), ['weather', 'tokens', 'today', 'monitor', 'stock-aapl', 'stock-nvda'])
  assert.deepEqual([...q.where.entries()], [['monitor', 'bar'], ['stock-aapl', 'bar'], ['stock-nvda', 'bar']])
  assert.deepEqual(keys(q.members), ['plugin:weather', 'widget:tokens', 'widget:today'])
})
