

// Boards (工作台, the widget design's §7 and its note of 2026-10-16): a pane
// 688 px across, its title bar 46 px, 14 px round a grid of 8 columns of the
// desktop's 72 px cells, 2 to 4 rows; its widgets flat tiles on that grid.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import * as G from '../src/pet/deskgrid.ts'

const WORK = [0, 0, 2048, 1104]
const here = { key: 'd1', primary: true, keys: ['d1'] }
const w = (id, size, extra = {}) => ({ id, type: 'widget', of: id.slice(1), size, ...extra })
const tile = (of, size, at, extra = {}) => ({ type: 'widget', of, size, at, ...extra })
const board = (id, items = [], extra = {}) => ({ id, type: 'board', items, ...extra })
let n = 0
const newId = () => `wn${++n}`

test('a board: 688 px across, its rows as its tiles need, 2 to 4; folded, its title bar alone', () => {
  assert.equal(G.BOARD.width, 688)
  assert.deepEqual(G.boardBox(board('wb')), { w: 688, h: 46 + 156 + 14 })
  assert.equal(G.boardRows(board('wb', [tile('a', 's', [0, 0])])), 2)
  assert.equal(G.boardRows(board('wb', [tile('a', 'l', [4, 0])])), 4)
  assert.equal(G.boardRows(board('wb', [tile('a', 's', [0, 1])])), 3)
  assert.deepEqual(G.boardBox(board('wb', [tile('a', 'l', [0, 0])])), { w: 688, h: 46 + 324 + 14 })
  assert.deepEqual(G.boardBox(board('wb', [tile('a', 'l', [0, 0])], { folded: true })), { w: 688, h: 46 })
  // On the desktop's grid it spans 9 columns (688 > 8 cells' 660) and 3 rows (216 px).
  assert.deepEqual(G.spanOf(G.boardBox(board('wb'))), [9, 3])
  assert.deepEqual(G.spanOf({ w: 688, h: 46 }), [9, 1])
  // Its tiles where the board's grid has them: 14 px in, under its title bar.
  assert.deepEqual(G.tileRect([2, 2], 'm'), { x: 14 + 168, y: 46 + 168, w: 324, h: 156 })
})

test('on the desktop a board takes its box; grown, it keeps its place and the others make way', () => {
  const g = G.gridOf(WORK)
  const b = board('wb', [tile('a', 'm', [0, 0])], { d: 'd1', at: { d1: [2, 1] } })
  const single = w('wx', 's', { d: 'd1', at: { d1: [2, 4] } })
  // Two rows: the board reaches down to 24 + 84 + 216; the one below it is clear.
  let s = G.settle([single, b], here, g)
  assert.deepEqual(s.at.get('wb'), [2, 1])
  assert.deepEqual(s.at.get('wx'), [2, 4])
  assert.deepEqual(s.moved, [])
  // Grown to four rows (a large tile): the board stays, the one under it moves.
  const grown = { ...b, items: [...b.items, tile('c', 'l', [4, 0])] }
  s = G.settle([single, grown], here, g)
  assert.deepEqual(s.at.get('wb'), [2, 1])
  assert.notDeepEqual(s.at.get('wx'), [2, 4])
  assert.deepEqual(s.moved, ['wx'])
  // Folded, it is its title bar alone: what stood under it may.
  assert.deepEqual(G.settle([single, { ...grown, folded: true }], here, g).moved, [])
  // A new board goes top right, whole on the grid.
  const fresh = G.settle([board('wn')], here, g).at.get('wn')
  assert.deepEqual(fresh, [g.cols - 9, 0])
  assert.ok(G.fits(g, fresh, G.boardBox(board('wn'))))
})

test('let go of over a board: the nearest empty cell; none, a row more, up to four; full, none', () => {
  // Empty: where it was let go of.
  assert.deepEqual(G.boardSpot(board('wb'), [3, 0], 's'), [3, 0])
  assert.deepEqual(G.boardSpot(board('wb'), [6, 1], 'm'), [4, 0])
  // Its two rows full of mediums: a small goes into a third and fourth row.
  const two = board('wb', [tile('a', 'm', [0, 0]), tile('b', 'm', [4, 0])])
  assert.deepEqual(G.boardSpot(two, [5, 0], 's'), [5, 2])
  // Room in the rows it has: there, not a row more.
  const gap = board('wb', [tile('a', 'm', [0, 0]), tile('b', 's', [4, 0])])
  assert.deepEqual(G.boardSpot(gap, [0, 3], 's'), [6, 0])
  // Four rows full: none (it shakes, the widget goes back).
  const full = board('wb', [tile('a', 'm', [0, 0]), tile('b', 'm', [4, 0]), tile('c', 'm', [0, 2]), tile('d', 'm', [4, 2])])
  assert.equal(G.boardSpot(full, [0, 0], 's'), null)
  // A large needs four rows of its four columns.
  assert.deepEqual(G.boardSpot(two, [0, 0], 'l'), null)
  assert.deepEqual(G.boardSpot(board('wb', [tile('a', 'm', [0, 0])]), [0, 0], 'l'), [4, 0])
  // Moving one of its own: its own cells are free to it.
  assert.deepEqual(G.boardSpot(two, [4, 0], 'm', 1), [4, 0])
  assert.deepEqual(G.boardSpot(two, [2, 1], 'm', 1), [4, 0])
  // Sixteen tiles at most.
  assert.equal(G.boardSpot(board('wb', Array.from({ length: 16 }, (_, i) => tile(`t${i}`, 's', [0, 0]))), [0, 0], 's'), null)
})

test('its Add puts one in the first empty cell, in reading order', () => {
  assert.deepEqual(G.boardFirst(board('wb'), 'm'), [0, 0])
  assert.deepEqual(G.boardFirst(board('wb', [tile('a', 's', [0, 0])]), 'm'), [2, 0])
  assert.deepEqual(G.boardFirst(board('wb', [tile('a', 'm', [2, 0])]), 's'), [0, 0])
  assert.deepEqual(G.boardFirst(board('wb', [tile('a', 'm', [0, 0]), tile('b', 'm', [4, 0])]), 's'), [0, 2])
  assert.equal(G.boardFirst(board('wb', [tile('a', 'l', [0, 0]), tile('b', 'l', [4, 0])]), 's'), null)
})

test('a tile let go of snaps to the board cell under its corner', () => {
  assert.deepEqual(G.tileAt(14, 46, 's'), [0, 0])
  assert.deepEqual(G.tileAt(14 + 84 * 3 + 40, 46 + 84 - 30, 's'), [3, 1])
  assert.deepEqual(G.tileAt(9999, 9999, 'm'), [4, 2])
  assert.deepEqual(G.tileAt(-99, -99, 'l'), [0, 0])
})

test('into a board and out again: off wherever it was; alone where it was let go of', () => {
  const layout = [
    w('wa', 's', { d: 'd1', at: { d1: [0, 0] } }),
    { id: 'ws', type: 'stack', size: 'm', d: 'd1', items: [{ type: 'widget', of: 'b' }, { type: 'plugin', of: 'stock' }] },
    board('wb', [tile('c', 's', [0, 0])], { d: 'd1', at: { d1: [5, 0] } }),
  ]
  // The one alone, dragged in: its entry gone, a tile on the cell.
  let next = G.intoBoard(layout, 'wb', G.memberOf(layout[0]), 's', [2, 0], newId)
  assert.deepEqual(
    next.map(e => e.id),
    ['ws', 'wb'],
  )
  assert.deepEqual(next[1].items[1], { type: 'widget', of: 'a', size: 's', at: [2, 0] })
  // One of a stack, from the library: out of its stack (one left: alone).
  next = G.intoBoard(next, 'wb', { type: 'plugin', of: 'stock' }, 'm', [4, 0], newId)
  assert.deepEqual(next[0], { id: 'wn1', type: 'widget', of: 'b', size: 'm', d: 'd1' })
  assert.deepEqual(next[1].items.map(G.keyOf), ['widget:c', 'widget:a', 'plugin:stock'])
  // Moved in it, then another size there.
  next = G.moveInBoard(next, 'wb', 1, [0, 2])
  assert.deepEqual(next[1].items[1].at, [0, 2])
  next = G.moveInBoard(next, 'wb', 1, [0, 2], 'm')
  assert.equal(next[1].items[1].size, 'm')
  // Taken out: alone on its cell of the display, the board without it.
  next = G.outOfBoard(next, 'wb', 0, newId, 'd1', [3, 6])
  assert.deepEqual(next[1].items.map(G.keyOf), ['widget:a', 'plugin:stock'])
  assert.deepEqual(next[2], { id: 'wn2', type: 'widget', of: 'c', size: 's', d: 'd1', at: { d1: [3, 6] } })
  assert.ok(G.isLayout(next), JSON.stringify(next))
})

test('a new board from one alone, where it stood; never from a stack; apart again', () => {
  const layout = [w('wa', 'm', { d: 'd1', at: { d1: [4, 2] } }), { id: 'ws', type: 'stack', size: 's', items: [{ type: 'widget', of: 'b' }, { type: 'widget', of: 'c' }] }]
  const made = G.newBoard(layout, 'wa', newId)
  assert.deepEqual(made[0], { id: made[0].id, type: 'board', items: [{ type: 'widget', of: 'a', size: 'm', at: [0, 0] }], d: 'd1', at: { d1: [4, 2] } })
  assert.equal(G.newBoard(layout, 'ws', newId), layout)
  const b = { ...made[0], items: [...made[0].items, tile('x', 's', [4, 0])] }
  const apart = G.boardApart([b, layout[1]], b.id, newId)
  assert.deepEqual(
    apart.map(e => [e.type, e.of, e.size, e.at]),
    [
      ['widget', 'a', 'm', { d1: [4, 2] }],
      ['widget', 'x', 's', undefined],
      ['stack', undefined, 's', undefined],
    ],
  )
})

test('boards in a layout: as settings.rs checks them; what they show; off the desktop', () => {
  const b = board('wb', [tile('a', 'm', [0, 0]), tile('stock', 's', [4, 0], { type: 'plugin' })], { name: '行情', folded: false, d: 'd1', at: { d1: [1, 1] } })
  assert.ok(G.isLayout([b, w('wz', 's')]))
  assert.ok(G.isLayout([board('wb')]))
  const bad = [
    [board('wb', [tile('a', 's', [0, 0]), tile('b', 's', [1, 0])])], // they meet
    [board('wb', [tile('a', 'm', [6, 0])])], // past its eighth column
    [board('wb', [tile('a', 's', [0, 3])])], // past its fourth row
    [board('wb', [tile('a', 'xl', [0, 0])])],
    [board('wb', [{ type: 'widget', of: 'a', size: 's' }])],
    [board('wb', [], { name: '一二三四五六七八九十一二三四五六七' })],
    [board('wb', [], { name: 'a\nb' })],
    [board('wb', [], { folded: 1 })],
    [board('wb', [], { size: 's' })],
    [board('wb', [tile('a', 's', [0, 0]), tile('a', 's', [2, 0])])],
    [board('wb', [tile('a', 's', [0, 0])]), w('wa', 's')], // in a board and alone
    [board('wb', [{ id: 'ws', type: 'stack', size: 's', items: [] }])],
    [board('wb', Array.from({ length: 17 }, (_, i) => tile(`t${i}`, 's', [(i % 4) * 2, 0])))],
  ]
  for (const x of bad) assert.ok(!G.isLayout(x), JSON.stringify(x))
  // What it shows, and off it: the board stays, without that one.
  assert.ok(G.has([b], { id: 'a' }))
  assert.ok(G.has([b], { plugin: 'stock' }))
  const all = [{ id: 'a' }, { id: 'stock-aapl', plugin: 'stock' }, { id: 'q' }]
  assert.deepEqual([...G.shownOn([b], all)].sort(), ['a', 'stock-aapl'])
  const left = G.without([b], { plugin: 'stock' }, newId)
  assert.deepEqual(left[0].items.map(G.keyOf), ['widget:a'])
  assert.deepEqual(G.without(left, { id: 'a' }, newId)[0].items, [])
  // Never stacked, nor stacked onto.
  assert.ok(!G.canStack(b, w('wq', 's')))
  assert.ok(!G.canStack(w('wq', 's'), b))
  assert.ok(G.isBoardName('工作'))
  assert.ok(!G.isBoardName('x'.repeat(17)))
})

// Several Ctrl-selected on the desktop made one board (desk.js, the user's call of 2026-10-17).
test('selected: ones alone and stacks, never a board; a rectangle meets those it overlaps', () => {
  const g = G.gridOf(WORK)
  assert.equal(G.isSelectable(w('wa', 's')), true)
  assert.equal(G.isSelectable({ id: 'ws', type: 'stack', size: 's', items: [{ type: 'widget', of: 'a' }, { type: 'widget', of: 'b' }] }), true)
  assert.equal(G.isSelectable(board('wb')), false)
  const list = [
    { id: 'wa', entry: w('wa', 's'), rect: G.rectAt(g, [0, 0], 's') },
    { id: 'wc', entry: w('wc', 'm'), rect: G.rectAt(g, [6, 0], 'm') },
    { id: 'wb', entry: board('wb'), rect: G.rectAt(g, [2, 0], G.boardBox(board('wb'))) },
  ]
  // From the first cell's middle to the third column: the first, and the board left out.
  assert.deepEqual(G.selectIn({ x: 60, y: 60, w: 160, h: 40 }, list), ['wa'])
  assert.deepEqual(G.selectIn({ x: 0, y: 0, w: 2000, h: 200 }, list), ['wa', 'wc'])
})

test("made one board: tiles in reading order (a stack's members each one), sizes kept where it has room, else the later ones small", () => {
  const at = new Map([['wb', [6, 0]], ['wa', [0, 3]], ['ws', [0, 0]]])
  const stack = { id: 'ws', type: 'stack', size: 's', items: [{ type: 'widget', of: 'x' }, { type: 'plugin', of: 'p', icon: 'stock' }] }
  const r = G.tilesOf([w('wa', 'm'), w('wb', 'l'), stack], at)
  // Reading order: the stack (row 0, col 0), wb (row 0, col 6), wa (row 3).
  assert.deepEqual(
    r.items.map(t => [t.of, t.size, t.at]),
    [
      ['x', 's', [0, 0]],
      ['p', 's', [2, 0]],
      ['b', 'l', [4, 0]],
      ['a', 'm', [0, 2]],
    ],
  )
  assert.equal(r.items[1].icon, 'stock')
  // Three large ones: a board holds two; with the third small there is no
  // room still, so the second is made small too.
  const big = G.tilesOf([w('wa', 'l'), w('wb', 'l'), w('wc', 'l')], new Map([['wa', [0, 0]], ['wb', [4, 0]], ['wc', [8, 0]]]))
  assert.deepEqual(big.items.map(t => t.size), ['l', 's', 's'])
  assert.ok(G.isEntry(board('wq', big.items)))
  // Seventeen: more than a board holds.
  const many = Array.from({ length: 17 }, (_, i) => w(`w${i}`, 's'))
  assert.deepEqual(G.tilesOf(many, new Map(many.map((e, i) => [e.id, [i, 0]]))), { why: 'tooMany' })
  assert.deepEqual(G.tilesOf([board('wb')], new Map()), { why: 'none' })
})

test('made one board: where the top-left-most stood, else the nearest place its box fits, over none of the others', () => {
  const g = G.gridOf(WORK)
  const layout = [w('wa', 's', { d: 'd1', at: { d1: [2, 0] } }), w('wb', 's', { d: 'd1', at: { d1: [0, 3] } }), w('wc', 'm', { d: 'd1', at: { d1: [12, 0] } })]
  const at = new Map([['wa', [2, 0]], ['wb', [0, 3]], ['wc', [12, 0]]])
  const r = G.boardOf(layout, ['wa', 'wb'], { at, grid: g, key: 'd1', newId: () => 'wnb', name: '工作台 2' })
  assert.deepEqual(r.board.items.map(t => [t.of, t.at]), [['a', [0, 0]], ['b', [2, 0]]])
  assert.equal(r.board.name, '工作台 2')
  // At wa's cell (2, 0): its box (9 x 3 cells) clear of wc at column 12.
  assert.deepEqual(r.board.at, { d1: [2, 0] })
  assert.deepEqual(r.layout.map(e => e.id), ['wc', 'wnb'])
  assert.ok(G.isLayout(r.layout))
  // wc in the way there: the nearest place clear of it.
  const r2 = G.boardOf(layout, ['wa', 'wb'], { at: new Map([...at, ['wc', [6, 0]]]), grid: g, key: 'd1', newId: () => 'wnb' })
  const box = G.rectAt(g, r2.board.at.d1, G.boardBox(r2.board))
  assert.equal(G.overlaps(box, G.rectAt(g, [6, 0], 'm')), false)
  // A board among those chosen is left out: nothing to make one of.
  assert.deepEqual(G.boardOf([board('wq')], ['wq'], { at: new Map(), grid: g, key: 'd1', newId }), { why: 'none' })
})

// Combos: boards kept to put up again (settings "boardSets").
const set = (id, name, items, saved = 1) => ({ id, name, items, saved })
const T1 = [
  { type: 'widget', of: 'today', size: 'm', at: [0, 0] },
  { type: 'plugin', of: 'weather', size: 's', at: [4, 0] },
]

test('combos are checked as settings.rs will check them: twenty at most, ids and names once, their tiles as a board has them', () => {
  assert.equal(G.isBoardSets([set('s1', '写代码', T1), set('s2', '摸鱼', T1.slice(1))]), true)
  assert.equal(G.isBoardSets([]), true)
  const bad = [
    [set('s1', '写代码', T1), set('s1', '摸鱼', T1)],
    [set('s1', '写代码', T1), set('s2', '写代码', T1)],
    [set('x1', '写代码', T1)],
    [set('s1', '', T1)],
    [set('s1', ' 前后有空格 ', T1)],
    [set('s1', '一二三四五六七八九十一二三四五六七', T1)],
    [set('s1', '写代码', [])],
    [set('s1', '写代码', [{ ...T1[0], name: '今天' }])],
    [set('s1', '写代码', [T1[0], { ...T1[1], at: [2, 0] }])],
    [{ ...set('s1', '写代码', T1), extra: 1 }],
    [set('s1', '写代码', T1, -1)],
    Array.from({ length: 21 }, (_, i) => set(`s${i}`, `组合 ${i}`, T1)),
  ]
  for (const list of bad) assert.equal(G.isBoardSets(list), false, JSON.stringify(list).slice(0, 90))
  // Kept from a board: its tiles' widgets, sizes and places, no names or icons.
  const b = board('wb', [{ ...T1[0], name: '今天' }, { ...T1[1], icon: 'weather' }], { name: '写代码' })
  assert.deepEqual(G.setOf(b, { id: 's9', name: '写代码', saved: 5 }), set('s9', '写代码', T1, 5))
})

test('a combo saved: one of its name replaced, else added; past twenty the oldest goes; renamed, removed, put back', () => {
  const two = [set('s1', '写代码', T1, 10), set('s2', '摸鱼', T1, 20)]
  const again = G.saveSet(two, set('s3', '写代码', T1.slice(1), 30))
  assert.equal(again.replaced, true)
  assert.deepEqual(
    again.list.map(s => [s.id, s.name, s.items.length, s.saved]),
    [
      ['s1', '写代码', 1, 30],
      ['s2', '摸鱼', 2, 20],
    ],
  )
  const added = G.saveSet(two, set('s3', '看行情', T1, 30))
  assert.deepEqual([added.replaced, added.dropped, added.list.length], [false, null, 3])
  const full = Array.from({ length: 20 }, (_, i) => set(`s${i}`, `组合 ${i}`, T1, 100 - i))
  const over = G.saveSet(full, set('sn', '新的', T1, 200))
  assert.equal(over.list.length, 20)
  assert.equal(over.dropped.name, '组合 19')
  assert.ok(G.isBoardSets(over.list))
  // Renamed: not to another's name, nor to nothing.
  assert.deepEqual(G.renameSet(two, 's2', ' 摸会儿鱼 ').map(s => s.name), ['写代码', '摸会儿鱼'])
  assert.equal(G.renameSet(two, 's2', '写代码'), null)
  assert.equal(G.renameSet(two, 's2', '  '), null)
  assert.equal(G.renameSet(two, 'sx', '新'), null)
  // Removed, and put back where it stood (undo); there again already: as it is.
  const { list, record } = G.removeSet(two, 's1')
  assert.deepEqual(list.map(s => s.id), ['s2'])
  assert.deepEqual(G.restoreSet(list, record).map(s => s.id), ['s1', 's2'])
  assert.deepEqual(G.restoreSet(two, record).map(s => s.id), ['s1', 's2'])
})

test('a combo put up again: its board, those no longer there left out; its widgets out of anywhere else on the desktop first', () => {
  const s = set('s1', '写代码', [...T1, { type: 'plugin', of: 'gone', size: 's', at: [6, 0] }])
  const { board: b, skipped } = G.boardFromSet(s, { known: m => m.of !== 'gone', newId: () => 'wnb' })
  assert.equal(skipped, 1)
  assert.deepEqual(b, { id: 'wnb', type: 'board', name: '写代码', items: T1 })
  assert.deepEqual(G.boardFromSet(s, { known: () => false, newId }), { board: null, skipped: 3 })
  const layout = [
    { id: 'wst', type: 'stack', size: 's', items: [{ type: 'widget', of: 'today' }, { type: 'widget', of: 'deploy' }] },
    { id: 'wbo', type: 'board', items: [{ type: 'plugin', of: 'weather', size: 's', at: [0, 0] }] },
  ]
  let k = 0
  const out = G.withBoard(layout, b, { key: 'd1', cell: [3, 2], newId: () => `wk${++k}` })
  assert.deepEqual(
    out.map(e => [e.id, e.type]),
    [
      ['wk1', 'widget'],
      ['wbo', 'board'],
      ['wnb', 'board'],
    ],
  )
  assert.deepEqual(out[1].items, [])
  assert.deepEqual(out[2].at, { d1: [3, 2] })
  assert.ok(G.isLayout(out))
  // No cell: where the display finds room (settle).
  assert.equal(G.withBoard([], b, { newId }).at(-1).at, undefined)
})
