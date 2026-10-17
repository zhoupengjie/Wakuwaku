const assert = require('node:assert/strict')
const { test } = require('node:test')

const G = require('../src/pet/deskgrid')

// A widget in one place (the widget design's §7, its note of 2026-10-16;
// zhoupengjie/Wakuwaku#53): what the closed island takes turns with, whether
// the monitor's readings are pinned at its end (or in the strip's part), and
// what the open island lists (island.js takeWidgets, deskgrid.js placement).
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
const WIN11 = { left: ['her'], center: ['start', 'apps'], right: ['monitor', 'tray', 'keys', 'quick', 'clock'] }
const noPart = { left: ['her'], center: ['start', 'apps'], right: ['tray', 'keys', 'quick', 'clock'] }

test('nothing put elsewhere: all on take turns, the monitor pinned by its setting', () => {
  const p = G.placement({ all, home: 'island', desk: [] })
  assert.equal(p.pinned, true)
  assert.deepEqual(ids(p.turning), ['today', 'tokens', 'weather', 'stock-aapl', 'stock-nvda'])
  assert.deepEqual(ids(p.listed), ['monitor', 'today', 'tokens', 'weather', 'stock-aapl', 'stock-nvda'])
  // Unpinned: one of the turns, listed in its own place.
  const q = G.placement({ all, home: 'island', desk: [], pin: false })
  assert.equal(q.pinned, false)
  assert.deepEqual(ids(q.turning), ['monitor', 'today', 'tokens', 'weather', 'stock-aapl', 'stock-nvda'])
  assert.deepEqual(ids(q.listed), ids(q.turning))
  // The monitor off: none, nothing pinned.
  const r = G.placement({ all: all.map(w => (w.id === 'monitor' ? { ...w, on: false } : w)), home: 'island' })
  assert.equal(r.pinned, false)
  assert.equal(r.monitor, null)
})

test('on the desktop (alone, stacked or in a board): out of the turns and not pinned, still listed', () => {
  const desk = [
    { id: 'wa', type: 'widget', of: 'monitor', size: 'm' },
    { id: 'ws', type: 'stack', size: 's', items: [{ type: 'widget', of: 'today' }, { type: 'plugin', of: 'weather' }] },
    { id: 'wb', type: 'board', items: [{ type: 'plugin', of: 'stock', size: 's', at: [0, 0] }] },
  ]
  const p = G.placement({ all, home: 'island', desk })
  assert.equal(p.pinned, false)
  assert.deepEqual([...p.out].sort(), ['monitor', 'stock-aapl', 'stock-nvda', 'today', 'weather'])
  assert.deepEqual(ids(p.turning), ['tokens'])
  // The overview: every one on, the capsule's first (the picker of
  // 2026-10-17), then those elsewhere, each saying where.
  assert.deepEqual(ids(p.listed), ['tokens', 'monitor', 'today', 'weather', 'stock-aapl', 'stock-nvda'])
  assert.deepEqual([...p.where.values()], ['desk', 'desk', 'desk', 'desk', 'desk'])
})

test('on the taskbar: out of the island while the taskbar is her home only', () => {
  const bar = { ...WIN11, right: [{ id: 'w1', type: 'widget', of: 'today', size: 'M' }, { id: 'w2', type: 'carousel', items: [{ type: 'plugin', of: 'stock' }, { type: 'widget', of: 'tokens' }] }, ...WIN11.right] }
  const p = G.placement({ all, home: 'taskbar', taskbar: bar })
  assert.deepEqual(ids(p.turning), ['weather'])
  assert.equal(p.pinned, true)
  // Her home the island: the taskbar's layout is not there to show them.
  assert.deepEqual(ids(G.placement({ all, home: 'island', taskbar: bar }).turning), ['today', 'tokens', 'weather', 'stock-aapl', 'stock-nvda'])
})

test("the taskbar her home: the monitor pinned while the strip has its part; taken off (#53), it takes its turns", () => {
  assert.equal(G.placement({ all, home: 'taskbar', taskbar: WIN11 }).pinned, true)
  const p = G.placement({ all, home: 'taskbar', taskbar: noPart })
  assert.equal(p.pinned, false)
  assert.deepEqual(ids(p.turning), ['monitor', 'today', 'tokens', 'weather', 'stock-aapl', 'stock-nvda'])
  // Its setting off: not pinned, part or none.
  assert.equal(G.placement({ all, home: 'taskbar', taskbar: WIN11, pin: false }).pinned, false)
  // On the desktop too: the part shows nothing (no readings twice).
  assert.equal(G.placement({ all, home: 'taskbar', taskbar: WIN11, desk: [{ id: 'wa', type: 'widget', of: 'monitor', size: 's' }] }).pinned, false)
})
