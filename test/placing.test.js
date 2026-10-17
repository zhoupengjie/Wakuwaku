const assert = require('node:assert/strict')
const { test } = require('node:test')

const P = require('../src/pet/placing')
const G = require('../src/pet/deskgrid')

// Putting widgets in places from the picker (placing.js; picker.js uses it):
// where on the strip, room on it, undo, and placed-is-on.
const WIN11 = { left: ['her'], center: ['start', 'apps'], right: ['monitor', 'tray', 'keys', 'quick', 'clock'] }

test('where one let go of on the strip goes: front half before, back half after, a gap after the one before', () => {
  const runs = [
    { run: 'left', lo: 0, hi: 300, items: [{ lo: 0, hi: 300 }] },
    { run: 'center', lo: 700, hi: 900, items: [{ lo: 700, hi: 744 }, { lo: 748, hi: 900 }] },
    { run: 'right', lo: 1200, hi: 1600, items: [{ lo: 1200, hi: 1300 }, { lo: 1310, hi: 1400 }, { lo: 1404, hi: 1600 }] },
  ]
  assert.deepEqual(P.slotAt(runs, 1210), { run: 'right', index: 0 })
  assert.deepEqual(P.slotAt(runs, 1260), { run: 'right', index: 1 })
  // Between two: after the one before (before the next).
  assert.deepEqual(P.slotAt(runs, 1305), { run: 'right', index: 1 })
  assert.deepEqual(P.slotAt(runs, 1590), { run: 'right', index: 3 })
  assert.deepEqual(P.slotAt(runs, 720), { run: 'center', index: 0 })
  assert.deepEqual(P.slotAt(runs, 850), { run: 'center', index: 2 })
  // Off every run: the nearest.
  assert.deepEqual(P.slotAt(runs, 1000), { run: 'center', index: 2 })
  assert.deepEqual(P.slotAt(runs, 1150), { run: 'right', index: 0 })
  // An empty run is a place too.
  assert.deepEqual(P.slotAt([{ run: 'center', lo: 760, hi: 840, items: [] }], 800), { run: 'center', index: 0 })
})

test('a press on a card: just before the tray, else at the right run’s end; put in at a place', () => {
  assert.deepEqual(P.beforeTray(WIN11), { run: 'right', index: 1 })
  assert.deepEqual(P.beforeTray({ left: ['her', 'tray'], center: [], right: ['clock'] }), { run: 'left', index: 1 })
  assert.deepEqual(P.beforeTray({ left: ['her'], center: [], right: ['clock'] }), { run: 'right', index: 1 })
  const e = { id: 'wx', type: 'plugin', of: 'ci', size: 'M' }
  const out = P.insertAt(WIN11, 'right', 1, e)
  assert.deepEqual(out.right, ['monitor', e, 'tray', 'keys', 'quick', 'clock'])
  assert.deepEqual(WIN11.right, ['monitor', 'tray', 'keys', 'quick', 'clock'], 'the one given left as it was')
  assert.deepEqual(P.insertAt(WIN11, 'center', 99, e).center, ['start', 'apps', e])
})

test('a full strip: medium ones made small first (the widest first), else no room', () => {
  assert.deepEqual(P.fitStrip({ room: 1600, used: 1400, adding: 120 }), { ok: true, shrink: [] })
  const shrinkable = [
    { id: 'wa', width: 110 },
    { id: 'wb', width: 150 },
    { id: 'wc', width: 64 },
  ]
  // 60 over: the widest gives 86.
  assert.deepEqual(P.fitStrip({ room: 1600, used: 1540, adding: 120, shrinkable }), { ok: true, shrink: ['wb'] })
  // 100 over: both medium ones.
  assert.deepEqual(P.fitStrip({ room: 1600, used: 1580, adding: 120, shrinkable }), { ok: true, shrink: ['wb', 'wa'] })
  // More than they can give: none.
  assert.deepEqual(P.fitStrip({ room: 1600, used: 1700, adding: 120, shrinkable }), { ok: false, shrink: [] })
  const layout = { left: ['her'], center: [], right: [{ id: 'wa', type: 'widget', of: 'today', size: 'M' }, { id: 'wb', type: 'plugin', of: 'weather', size: 'M' }] }
  assert.deepEqual(P.withSizes(layout, ['wb'], 'S').right.map(p => p.size), ['M', 'S'])
})

test('undo: an entry taken off goes back where it was, or the run’s end; not twice', () => {
  const e = { id: 'wa', type: 'plugin', of: 'weather', size: 'M' }
  const layout = { left: ['her'], center: ['start', 'apps'], right: ['monitor', e, 'tray', 'clock'] }
  const record = P.removalOf(layout, 'wa')
  assert.deepEqual(record, { entry: e, run: 'right', index: 1 })
  const off = P.without(layout, 'wa')
  assert.deepEqual(off.right, ['monitor', 'tray', 'clock'])
  assert.deepEqual(P.restoreEntry(off, record), layout)
  // Things moved meanwhile: as near as it can.
  const shorter = { left: ['her'], center: ['start', 'apps'], right: ['clock'] }
  assert.deepEqual(P.restoreEntry(shorter, record).right, ['clock', e])
  // There again (put back by hand, or its plugin on the strip again): unchanged.
  assert.equal(P.restoreEntry(layout, record), layout)
  const again = P.insertAt(off, 'left', 1, { id: 'wz', type: 'plugin', of: 'weather', size: 'S' })
  assert.equal(P.restoreEntry(again, record), again)
  // A part (the monitor's) the same way.
  const part = P.removalOf(layout, 'monitor')
  assert.deepEqual(P.restoreEntry(P.without(layout, 'monitor'), part), layout)
  assert.equal(P.removalOf(layout, 'nope'), null)
})

test('placed is on: put on the strip or the desktop, its plugin on or its widget out of widgetsOff; taken from its last place, off', () => {
  const taskbar = { ...WIN11, right: [{ id: 'w1', type: 'carousel', items: [{ type: 'plugin', of: 'stock' }, { type: 'widget', of: 'today' }] }, ...WIN11.right] }
  const desk = [{ id: 'wb', type: 'board', items: [{ type: 'plugin', of: 'weather', size: 's', at: [0, 0] }] }, { id: 'wd', type: 'widget', of: 'deploy', size: 's' }]
  assert.deepEqual([...P.placedKeys({ taskbar, desk, home: 'taskbar' })].sort(), ['plugin:stock', 'plugin:weather', 'widget:deploy', 'widget:monitor', 'widget:today'])
  // Her home the island: the strip is not there.
  assert.deepEqual([...P.placedKeys({ taskbar, desk, home: 'island' })].sort(), ['plugin:weather', 'widget:deploy'])
  const config = { plugins: { weather: { on: false, City: '上海' } }, widgetsOff: ['monitor'] }
  assert.deepEqual(P.switchPatch(config, 'plugin:weather', true), { plugins: { weather: { on: true, City: '上海' } } })
  assert.deepEqual(P.switchPatch(config, 'plugin:ci', true), { plugins: { weather: { on: false, City: '上海' }, ci: { on: true } } })
  assert.equal(P.switchPatch(config, 'plugin:weather', false), null)
  assert.deepEqual(P.switchPatch(config, 'widget:monitor', true), { widgetsOff: [] })
  assert.deepEqual(P.switchPatch(config, 'widget:today', false), { widgetsOff: ['monitor', 'today'] })
  assert.equal(P.switchPatch(config, 'widget:today', true), null)
  // Several in one patch.
  assert.deepEqual(P.switchesPatch(config, ['plugin:weather', 'plugin:ci', 'widget:monitor'], true), { plugins: { weather: { on: true, City: '上海' }, ci: { on: true } }, widgetsOff: [] })
  assert.equal(P.switchesPatch(config, ['plugin:weather'], false), null)
})

test('let go of over the desktop: the next free cell from the one under the pointer, row by row, never over another', () => {
  const g = G.gridOf([0, 0, 1600, 852])
  // 18 columns, 9 rows of cells.
  assert.equal(g.cols, 18)
  assert.equal(g.rows, 9)
  const blocks = [G.rectAt(g, [4, 2], 'm'), G.rectAt(g, [8, 2], 's')]
  // Over [4, 2] (a medium there): the next in the row with room for a small one.
  assert.deepEqual(G.nextFree(g, [4, 2], 's', blocks), [10, 2])
  // Past the row's end: the next row's start.
  assert.deepEqual(G.nextFree(g, [15, 2], 's', [G.rectAt(g, [14, 2], 'm')]), [0, 3])
  // Wanted off the grid: kept on it.
  assert.deepEqual(G.nextFree(g, [16, 8], 'm', []), [14, 7])
  // The last cell taken: round to the top.
  const full = [G.rectAt(g, [16, 7], 's')]
  assert.deepEqual(G.nextFree(g, [16, 7], 's', full), [0, 0])
  // The pointer at its middle: its top left half a widget up and left.
  const at = (c, r) => ({ x: 24 + 84 * c + 78, y: 24 + 84 * r + 78 })
  assert.deepEqual(G.landing({ grid: g, ...at(6, 4), size: 's', blocks }), { ok: true, where: 'desk', cell: [6, 4], want: [6, 4] })
  const on = G.landing({ grid: g, ...at(4, 2), size: 's', blocks })
  assert.deepEqual([on.want, on.cell], [
    [4, 2],
    [10, 2],
  ])
  // Nowhere free: no.
  const everywhere = [{ x: 0, y: 0, w: 1600, h: 900 }]
  assert.equal(G.landing({ grid: g, x: 300, y: 300, size: 's', blocks: everywhere }).ok, false)
  // Over icons: the next clear of them; none clear of them, over them sooner than over a widget.
  const icons = [G.rectAt(g, [6, 4], 's')]
  assert.deepEqual(G.landing({ grid: g, ...at(6, 4), size: 's', icons }).cell, [8, 4])
  assert.deepEqual(G.landing({ grid: g, ...at(6, 4), size: 's', icons: everywhere }).cell, [6, 4])
})

test('let go of over a board: its first free cell; full, no; the layout with it there, out of anywhere else first', () => {
  const g = G.gridOf([0, 0, 1600, 852])
  const board = { id: 'wb', type: 'board', items: [{ type: 'widget', of: 'today', size: 'm', at: [0, 0] }] }
  const land = G.landing({ grid: g, x: 0, y: 0, size: 'm', board, tileSize: 's' })
  assert.deepEqual(land, { ok: true, where: 'board', spot: [4, 0] })
  const layout = [board, { id: 'wd', type: 'plugin', of: 'weather', size: 'm', d: 'd1', at: { d1: [0, 3] } }]
  let n = 0
  const newId = () => `wn${++n}`
  const m = { type: 'plugin', of: 'weather' }
  const into = G.dropped(layout, land, { m, size: 'm', tileSize: 's', key: 'd1', boardId: 'wb', newId })
  assert.deepEqual(into, [{ ...board, items: [...board.items, { type: 'plugin', of: 'weather', size: 's', at: [4, 0] }] }])
  // On the grid: alone on that cell of the display, the old one gone (a move).
  const desk = G.dropped(layout, { ok: true, where: 'desk', cell: [6, 3] }, { m, size: 'm', key: 'd1', newId })
  assert.deepEqual(desk, [board, { id: 'wn1', type: 'plugin', of: 'weather', size: 'm', d: 'd1', at: { d1: [6, 3] } }])
  // Full: no.
  const full = { ...board, items: Array.from({ length: 16 }, (_, i) => ({ type: 'widget', of: `w${i}`, size: 's', at: [(i % 4) * 2, Math.floor(i / 4)] })) }
  assert.deepEqual(G.landing({ grid: g, x: 0, y: 0, size: 's', board: full }), { ok: false, where: 'board', why: 'boardFull' })
  assert.equal(G.dropped(layout, { ok: false }, { m, size: 'm', key: 'd1', newId }), layout)
})
