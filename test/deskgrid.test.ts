

// This computer's desktop as icons.rs read it (examples/icons.rs list,
// 2026-10-16): 2560 x 1440 at 125%, the work area 2560 x 1380 (the
// taskbar's 60 px), three icons down the left, each its grid's cell of
// 93 x 122 px; in the desk window's CSS px (/ 1.25).
import assert from 'node:assert/strict'
import { test } from 'node:test'
import * as G from '../src/pet/deskgrid.ts'

const SF = 1.25
const WORK = [0, 0, 2560 / SF, 1380 / SF]
const ICONS = [
  [0, 5],
  [0, 127],
  [0, 249],
].map(([x, y]) => ({ x: x / SF, y: y / SF, w: 93 / SF, h: 122 / SF }))

test('the grid: 72 px cells, 12 px gutters, 24 px in from the work area', () => {
  assert.deepEqual(G.sizePx('s'), { w: 156, h: 156 })
  assert.deepEqual(G.sizePx('m'), { w: 324, h: 156 })
  assert.deepEqual(G.sizePx('l'), { w: 324, h: 324 })
  const g = G.gridOf(WORK)
  assert.equal(g.x0, 24)
  assert.equal(g.y0, 24)
  // 2048 wide: (2048 - 48 + 12) / 84 = 23.9 cells; 1104 high: 12.7.
  assert.equal(g.cols, 23)
  assert.equal(g.rows, 12)
  // The last column's right edge 24 px or more from the work area's.
  const far = G.rectAt(g, [g.cols - 2, g.rows - 2], 's')
  assert.ok(far.x + far.w <= WORK[2] - 24, JSON.stringify(far))
  assert.ok(far.y + far.h <= WORK[3] - 24, JSON.stringify(far))
  // A work area not at the display's corner (a taskbar on the left).
  assert.deepEqual(G.rectAt(G.gridOf([48, 0, 1000, 700]), [1, 1], 'm'), { x: 72 + 84, y: 24 + 84, w: 324, h: 156 })
})

test('a drop snaps to the nearest cell, kept on the grid', () => {
  const g = G.gridOf(WORK)
  assert.deepEqual(G.snap(g, 24, 24, 's'), [0, 0])
  assert.deepEqual(G.snap(g, 24 + 84 * 3 + 40, 24 + 84 * 2 - 41, 's'), [3, 2])
  assert.deepEqual(G.snap(g, 24 + 84 * 3 + 43, 24, 's'), [4, 0])
  // Past the edges: the last cells a widget of its size fits on.
  assert.deepEqual(G.snap(g, -500, -500, 'l'), [0, 0])
  assert.deepEqual(G.snap(g, 99999, 99999, 'l'), [g.cols - 4, g.rows - 4])
  assert.ok(G.fits(g, [g.cols - 4, g.rows - 4], 'l'))
  assert.ok(!G.fits(g, [g.cols - 3, 0], 'l'))
})

test('dropped onto the icons, a widget moves to the nearest free spot', () => {
  const g = G.gridOf(WORK)
  // Over the first icon (its cell reaches 74 px in): the nearest free cell
  // is the next column, as near as can be.
  const at = G.dropAt(g, 24, 24, 's', [], ICONS)
  assert.deepEqual(at, [1, 0])
  assert.ok(!ICONS.some(i => G.overlaps(G.rectAt(g, at, 's'), i)))
  // Medium over the second icon: beside the icons, its row kept.
  assert.deepEqual(G.dropAt(g, 30, 24 + 84 * 1, 'm', [], ICONS), [1, 1])
  // Clear of the icons, it stays where it was let go of.
  assert.deepEqual(G.dropAt(g, 24 + 84 * 10, 24 + 84 * 4, 'l', [], ICONS), [10, 4])
  // Onto another widget: the nearest spot beside it, over neither.
  const other = G.rectAt(g, [10, 4], 'l')
  const next = G.dropAt(g, 24 + 84 * 11, 24 + 84 * 4, 'm', [other], ICONS)
  assert.ok(!G.overlaps(G.rectAt(g, next, 'm'), other))
  assert.ok(Math.abs(next[0] - 11) + Math.abs(next[1] - 4) <= 4, JSON.stringify(next))
})

test("an icon is all its list draws: a widget let go of over its label's second line moves too", () => {
  // The test VM's left display at 175 % (CSS px): DeepSeek Harness's icon as
  // Explorer's list draws it (desk.rs icons: LVM_GETITEMRECT, LVIR_BOUNDS),
  // and the place of the grid's spacing it was taken for before, its label's
  // second line and the picture's sides outside that.
  const drawn = { x: 1560 / 1.75, y: 530 / 1.75, w: 130 / 1.75, h: 147 / 1.75 }
  const place = { x: 1583 / 1.75, y: 527 / 1.75, w: 93 / 1.75, h: 125 / 1.75 }
  // A work area whose row falls just under the place, on the label.
  const g = G.gridOf([0, 350, 1920 / 1.75, 617])
  const cell = [10, 0]
  const r = G.rectAt(g, cell, 's')
  assert.ok(!G.overlaps(r, place) && G.overlaps(r, drawn), JSON.stringify([r, place, drawn]))
  // Taken for its place, it stayed over the label; taken as drawn, it moves.
  assert.deepEqual(G.dropAt(g, r.x, r.y, 's', [], [place]), cell)
  const moved = G.dropAt(g, r.x, r.y, 's', [], [drawn])
  assert.notDeepEqual(moved, cell)
  assert.ok(!G.overlaps(G.rectAt(g, moved, 's'), drawn))
})

test('a display with no room left: over the icons sooner than over a widget', () => {
  const g = G.gridOf([0, 0, 24 * 2 + 156, 24 * 2 + 156])
  assert.deepEqual([g.cols, g.rows], [2, 2])
  const icon = { x: 30, y: 30, w: 40, h: 40 }
  assert.deepEqual(G.dropAt(g, 24, 24, 's', [], [icon]), [0, 0])
  assert.equal(G.nearestFree(g, [0, 0], 's', [G.rectAt(g, [0, 0], 's')]), null)
})

const here = { key: 'd1', primary: true, keys: ['d1', 'd2'] }
const w = (id, size, extra = {}) => ({ id, type: 'widget', of: id.slice(1), size, ...extra })

test('each display shows its own, the main one those whose display is gone', () => {
  const layout = [w('wa', 's', { d: 'd1' }), w('wb', 's', { d: 'd2' }), w('wc', 's', { d: 'd9' }), w('wd', 's')]
  assert.deepEqual(G.shownHere(layout, here).map(e => e.id), ['wa', 'wc', 'wd'])
  assert.deepEqual(G.shownHere(layout, { key: 'd2', primary: false, keys: ['d1', 'd2'] }).map(e => e.id), ['wb'])
})

test('settled: each where it stood on this display; new ones top right, clear of icons and each other', () => {
  const g = G.gridOf(WORK)
  const entries = [w('wa', 'm', { d: 'd1', at: { d1: [3, 1], d2: [0, 0] } }), w('wb', 's', { d: 'd1' }), w('wc', 'l', { d: 'd1', at: { d2: [5, 5] } })]
  const { at, moved } = G.settle(entries, here, g, ICONS)
  assert.deepEqual(at.get('wa'), [3, 1])
  assert.deepEqual(at.get('wb'), [g.cols - 2, 0])
  // Never on this display: from the top right, beside the one there.
  assert.deepEqual(at.get('wc'), [g.cols - 6, 0])
  assert.deepEqual(moved, ['wb', 'wc'])
  // Two remembered on the same cells (the work area changed): the first keeps it.
  const clash = [w('wa', 's', { at: { d1: [2, 2] } }), w('wb', 's', { at: { d1: [3, 2] } })]
  const second = G.settle(clash, here, g, ICONS)
  assert.deepEqual(second.at.get('wa'), [2, 2])
  assert.notDeepEqual(second.at.get('wb'), [3, 2])
  assert.deepEqual(second.moved, ['wb'])
  // Remembered over the icons: kept where it stood, unless new here.
  const over = [w('wa', 's', { at: { d1: [0, 0] } })]
  assert.deepEqual(G.settle(over, here, g, ICONS).at.get('wa'), [0, 0])
  assert.deepEqual(G.settle(over, here, g, ICONS, new Set(['wa'])).at.get('wa'), [1, 0])
})

test('placed: on a display and where, each display remembered; a visitor stays its own display', () => {
  const e = w('wa', 's', { d: 'd2', at: { d2: [1, 1] } })
  assert.deepEqual(G.placed(e, 'd1', [4, 0]), { ...e, d: 'd1', at: { d2: [1, 1], d1: [4, 0] } })
  assert.deepEqual(G.placed(e, 'd1', [4, 0], true), { ...e, d: 'd2', at: { d2: [1, 1], d1: [4, 0] } })
  // Eight displays at most, the one it is on kept.
  const many = w('wb', 's', { at: Object.fromEntries(Array.from({ length: 8 }, (_, i) => [`k${i}`, [i, 0]])) })
  const kept = G.placed(many, 'k9', [9, 0])
  assert.equal(Object.keys(kept.at).length, 8)
  assert.deepEqual(kept.at.k9, [9, 0])
  assert.ok(!('k0' in kept.at))
})

test('stacks: onto one of the same size, the dropped on top; out and apart again', () => {
  let n = 0
  const newId = () => `wn${++n}`
  const layout = [w('wa', 'm', { d: 'd1', at: { d1: [0, 0] } }), w('wb', 'm', { d: 'd1', at: { d1: [5, 0] } }), w('wc', 's'), { id: 'wp', type: 'plugin', of: 'stock', size: 'm' }]
  // Not the same size: no stack.
  assert.equal(G.stackOnto(layout, 'wc', 'wa', newId), layout)
  assert.ok(!G.canStack(layout[0], layout[0]))
  const one = G.stackOnto(layout, 'wb', 'wa', newId)
  assert.equal(one.length, 3)
  const s = one[0]
  assert.equal(s.type, 'stack')
  assert.deepEqual(s.items, [{ type: 'widget', of: 'b' }, { type: 'widget', of: 'a' }])
  assert.deepEqual([s.size, s.d, s.at], ['m', 'd1', { d1: [0, 0] }])
  assert.ok(G.isLayout(one), JSON.stringify(one))
  // A plugin onto the stack: in it, on top.
  const two = G.stackOnto(one, 'wp', s.id, newId)
  assert.deepEqual(two.find(e => e.id === s.id).items.map(G.keyOf), ['plugin:stock', 'widget:b', 'widget:a'])
  // Taken out by a drag: alone where let go of, the rest stays.
  const out = G.takeOut(two, s.id, 0, newId, 'd1', [8, 3])
  const alone = out.find(e => e.of === 'stock')
  assert.deepEqual([alone.type, alone.size, alone.d, alone.at.d1], ['plugin', 'm', 'd1', [8, 3]])
  assert.deepEqual(out.find(e => e.id === s.id).items.map(G.keyOf), ['widget:b', 'widget:a'])
  // The last but one out: the stack is the other alone, where it stood.
  const last = G.takeOut(out, s.id, 0, newId, 'd1', [8, 6])
  const rest = last.find(e => e.of === 'a')
  assert.deepEqual([rest.type, rest.at], ['widget', { d1: [0, 0] }])
  assert.ok(!last.some(G.isStack))
  assert.ok(G.isLayout(last), JSON.stringify(last))
  // Taken apart: each alone, the first where the stack stood.
  const apart = G.ungroup(two, s.id, newId)
  assert.deepEqual(apart.map(e => e.type), ['plugin', 'widget', 'widget', 'widget'])
  assert.deepEqual(apart[0].at, { d1: [0, 0] })
  assert.equal(apart[1].at, undefined)
})

test('a stack turns by one, around; news brings a member to the top', () => {
  assert.equal(G.flip(0, 3, 1), 1)
  assert.equal(G.flip(2, 3, 1), 0)
  assert.equal(G.flip(0, 3, -1), 2)
  assert.equal(G.flip(5, 0, 1), 0)
  const s = { id: 'ws', type: 'stack', size: 's', items: [{ type: 'widget', of: 'weather' }, { type: 'widget', of: 'inbox-a' }] }
  const mail = n => ({ id: 'inbox-a', from: 'mail', label: 'A', value: { key: 'x', vars: { n } } })
  const weather = { id: 'weather', label: 'Dresden', value: '11°' }
  assert.equal(G.newsTop(s, [weather, mail(2)], [weather, mail(3)]), 1)
  assert.equal(G.newsTop(s, [weather, mail(3)], [weather, mail(3)]), -1)
  assert.equal(G.newsTop(s, [weather, mail(3)], [weather, mail(1)]), -1)
})

test('the sizes a stack may take: those all of its members have', () => {
  const all = [
    { id: 'weather', detail: { sizes: ['s', 'm', 'l'] } },
    { id: 'pomodoro', detail: { sizes: ['s', 'm'] } },
    { id: 'countdown', detail: { sizes: ['s'] } },
  ]
  const s = { id: 'ws', type: 'stack', size: 's', items: [{ type: 'widget', of: 'weather' }, { type: 'widget', of: 'pomodoro' }] }
  assert.deepEqual(G.sizesOfEntry(s, all), ['s', 'm'])
  assert.deepEqual(G.sizesOfEntry({ ...s, items: [...s.items, { type: 'widget', of: 'countdown' }] }, all), ['s'])
  // One with nothing to say yet: any.
  assert.deepEqual(G.sizesOfEntry(w('wx', 's'), all), ['s', 'm', 'l'])
})

test('a layout kept: as settings.rs checks it', () => {
  assert.ok(G.isLayout([]))
  assert.ok(G.isLayout([w('wa', 'l', { d: 'd1', at: { d1: [3, 4] }, name: '天气' })]))
  const bad = [
    [w('wa', 'xl')],
    [w('Wa', 's')],
    [w('wa', 's', { d: 'D1' })],
    [w('wa', 's', { at: { d1: [1, -1] } })],
    [w('wa', 's', { at: { d1: [1] } })],
    [w('wa', 's', { extra: 1 })],
    [w('wa', 's'), w('wa', 's')],
    [w('wa', 's'), { ...w('wb', 's'), of: 'a' }],
    [{ id: 'ws', type: 'stack', size: 's', items: [{ type: 'widget', of: 'a' }] }],
    [{ id: 'ws', type: 'stack', size: 's', of: 'a', items: [{ type: 'widget', of: 'a' }, { type: 'widget', of: 'b' }] }],
    [{ id: 'ws', type: 'stack', size: 's', items: [{ type: 'widget', of: 'a' }, { type: 'widget', of: 'a' }] }],
    Array.from({ length: 33 }, (_, i) => w(`w${i}`, 's')),
  ]
  for (const b of bad) assert.ok(!G.isLayout(b), JSON.stringify(b))
})

test('off the desktop: its entry gone, or out of its stack', () => {
  let n = 0
  const newId = () => `wn${++n}`
  const layout = [w('wa', 's'), { id: 'ws', type: 'stack', size: 'm', d: 'd1', at: { d1: [1, 1] }, items: [{ type: 'widget', of: 'b' }, { type: 'plugin', of: 'stock' }] }]
  assert.ok(G.has(layout, { id: 'a' }))
  assert.ok(G.has(layout, { plugin: 'stock' }))
  assert.ok(!G.has(layout, { id: 'stock-x' }))
  assert.deepEqual(G.without(layout, { id: 'a' }, newId).map(e => e.id), ['ws'])
  const left = G.without(layout, { plugin: 'stock' }, newId)
  assert.deepEqual(left[1], { id: 'wn1', type: 'widget', of: 'b', size: 'm', d: 'd1', at: { d1: [1, 1] } })
  const all = [{ id: 'a' }, { id: 'b' }, { id: 'stock-aapl', plugin: 'stock' }, { id: 'c' }]
  assert.deepEqual([...G.shownOn(layout, all)].sort(), ['a', 'b', 'stock-aapl'])
})
