import assert from 'node:assert/strict'
import { test } from 'node:test'
import * as Aim from '../src/pet/aim.ts'

const { create, heading, inTriangle, nearSide, OPEN_MS, SWITCH_MS, REST_MS } = Aim

// A bottom bar as the page has it at 125% (CSS px): the strip 640..688,
// its buttons 40 px tall (644..684), B, A, C side by side; A's list of
// three cards standing 6 px over the strip, centred on A, as wide as three.
const B = { x: 662, y: 664 }
const A = { x: 706, y: 664 }
const C = { x: 750, y: 664 }
const LIST = { left: 376, top: 434, right: 1036, bottom: 634 }
const shown = key => ({ key, rect: LIST, edge: 'bottom' })

test('a point in a triangle, on its edge, or out of it', () => {
  const a = { x: 0, y: 0 }
  const b = { x: 10, y: 0 }
  const c = { x: 0, y: 10 }
  assert.equal(inTriangle({ x: 2, y: 2 }, a, b, c), true)
  assert.equal(inTriangle({ x: 5, y: 0 }, a, b, c), true, 'on an edge')
  assert.equal(inTriangle({ x: 6, y: 6 }, a, b, c), false)
  assert.equal(inTriangle({ x: -1, y: 2 }, a, b, c), false)
  // Either way round.
  assert.equal(inTriangle({ x: 2, y: 2 }, a, c, b), true)
})

test("the list's side towards the strip, by the strip's edge, a little longer", () => {
  const r = { left: 100, top: 200, right: 300, bottom: 400 }
  assert.deepEqual(nearSide(r, 'bottom', 6), [{ x: 94, y: 400 }, { x: 306, y: 400 }])
  assert.deepEqual(nearSide(r, 'top', 6), [{ x: 94, y: 200 }, { x: 306, y: 200 }])
  assert.deepEqual(nearSide(r, 'left', 6), [{ x: 100, y: 194 }, { x: 100, y: 406 }])
  assert.deepEqual(nearSide(r, 'right', 6), [{ x: 300, y: 194 }, { x: 300, y: 406 }])
})

test('up and to the side towards the list is heading into it; along the strip, down or at rest is not', () => {
  // Up to the left, at a slant flatter than a button's height over the
  // list's half width: still into it.
  assert.equal(heading(A, { x: 690, y: 660 }, LIST, 'bottom'), true)
  assert.equal(heading(A, { x: 726, y: 659 }, LIST, 'bottom'), true)
  // Straight up.
  assert.equal(heading(A, { x: 706, y: 650 }, LIST, 'bottom'), true)
  // Along the strip, flat: towards the neighbour, not the list.
  assert.equal(heading(A, { x: 690, y: 664 }, LIST, 'bottom'), false)
  // Down, or down and along.
  assert.equal(heading(A, { x: 700, y: 670 }, LIST, 'bottom'), false)
  // Not moved.
  assert.equal(heading(A, { ...A }, LIST, 'bottom'), false)
  // Up and out past the list's end: not into it.
  const narrow = { left: 650, top: 434, right: 760, bottom: 634 }
  assert.equal(heading({ x: 900, y: 664 }, { x: 912, y: 660 }, narrow, 'bottom'), false)
  assert.equal(heading({ x: 780, y: 664 }, { x: 772, y: 650 }, narrow, 'bottom'), true)
})

test('heading into the list beside or below a bar the other edges have', () => {
  // A top bar: the list below the strip (y 54..254).
  const below = { left: 376, top: 54, right: 1036, bottom: 254 }
  assert.equal(heading({ x: 706, y: 24 }, { x: 690, y: 28 }, below, 'top'), true)
  assert.equal(heading({ x: 706, y: 24 }, { x: 690, y: 20 }, below, 'top'), false)
  // A left bar: the list to its right (x 54..274), along it y 300..700.
  const right = { left: 54, top: 300, right: 274, bottom: 700 }
  assert.equal(heading({ x: 24, y: 500 }, { x: 28, y: 484 }, right, 'left'), true)
  assert.equal(heading({ x: 24, y: 500 }, { x: 24, y: 484 }, right, 'left'), false)
  // A right bar (its strip at x 1872..1920): the list to its left.
  const left = { left: 1600, top: 300, right: 1818, bottom: 700 }
  assert.equal(heading({ x: 1896, y: 500 }, { x: 1888, y: 490 }, left, 'right'), true)
  // Mostly along the bar: aimed past the list's top.
  assert.equal(heading({ x: 1896, y: 500 }, { x: 1892, y: 484 }, left, 'right'), false)
  assert.equal(heading({ x: 1896, y: 500 }, { x: 1900, y: 484 }, left, 'right'), false)
})

test('none open: a list opens once the pointer has been on the button OPEN_MS', () => {
  const aim = create()
  assert.equal(aim.due(null, 0), null, 'on no button: nothing')
  aim.move(A.x, A.y, 1000)
  aim.over('A', 1000)
  assert.equal(aim.due(null, 1000), OPEN_MS)
  // Moving within the button (its icon, its count): still counted from when it came.
  aim.over('A', 1100)
  assert.equal(aim.due(null, 1200), OPEN_MS - 200)
  assert.equal(aim.due(null, 1000 + OPEN_MS), 0)
  // Off the buttons, and back: counted anew.
  aim.over(null, 1300)
  assert.equal(aim.due(null, 1300), null)
  aim.over('A', 1400)
  assert.equal(aim.due(null, 1450), OPEN_MS - 50)
})

test("one open: another's list once the pointer has rested on its button SWITCH_MS, as on Windows' own", () => {
  const aim = create()
  aim.move(A.x, A.y, 0)
  aim.over('A', 0)
  assert.equal(aim.due(shown('A'), 10), null, 'its own list: nothing to do')
  // Onto B, still.
  aim.move(B.x, B.y, 1000)
  aim.over('B', 1000)
  assert.equal(aim.due(shown('A'), 1000 + REST_MS + 1), SWITCH_MS - REST_MS - 1)
  assert.equal(aim.due(shown('A'), 1000 + SWITCH_MS), 0)
  // Back on A before that: A's stays (nothing to do), and B counts anew next time.
  aim.over('A', 1100)
  assert.equal(aim.due(shown('A'), 1100), null)
  aim.move(B.x, B.y, 1150)
  aim.over('B', 1150)
  assert.equal(aim.due(shown('A'), 1250), SWITCH_MS - 100)
})

test('swept along the strip over B and C quickly: neither takes the list', () => {
  const aim = create()
  aim.move(A.x, A.y, 0)
  aim.over('A', 0)
  // Along, flat: 20 px every 16 ms.
  let t = 1000
  for (let x = A.x; x <= C.x + 60; x += 20, t += 16) {
    aim.move(x, A.y, t)
    const key = x < 684 ? 'B' : x < 728 ? 'A' : x < 772 ? 'C' : null
    aim.over(key, t)
    const due = aim.due(shown('A'), t)
    assert.ok(due === null || due > 0, `at x ${x}: ${due}`)
  }
})

test('on its way up into the list across B, B never takes it; stopped on B, it does SWITCH_MS after the last move', () => {
  const aim = create()
  aim.move(A.x, A.y, 0)
  aim.over('A', 0)
  // Slowly up and to the left across B, 2 px left and half a px up every
  // 16 ms, never off the strip: 0.35 s on B, longer than Windows waits.
  let t = 1000
  let x = A.x
  let y = A.y
  for (; x > 640; x -= 2, y -= 0.5, t += 16) {
    aim.move(x, y, t)
    aim.over(x < 684 ? 'B' : 'A', t)
    assert.notEqual(aim.due(shown('A'), t), 0, `switched at ${x},${y}`)
  }
  // Stopped on B (it never got up off the strip): REST_MS later it heads
  // nowhere, and the list is B's SWITCH_MS after its last move.
  const last = t - 16
  aim.move(x + 2, y + 0.5, last)
  assert.equal(aim.due(shown('A'), last + REST_MS + 1), SWITCH_MS - REST_MS - 1)
  assert.equal(aim.due(shown('A'), last + SWITCH_MS), 0)
})

test("on B moving along it or down, the time counts as resting: as on Windows' own", () => {
  const aim = create()
  aim.move(A.x, A.y, 0)
  aim.over('A', 0)
  // Across B slowly, flat.
  let t = 1000
  for (let x = 682; x >= 644; x -= 1, t += 8) {
    aim.move(x, B.y, t)
    aim.over('B', t)
  }
  assert.equal(aim.due(shown('A'), 1000 + SWITCH_MS), 0)
  // Down from the list onto B: B's after SWITCH_MS on it.
  const down = create()
  down.move(A.x, 600, 0)
  down.over(null, 0)
  t = 2000
  for (let y = 600; y <= 664; y += 8, t += 16) {
    down.move(B.x, y, t)
    down.over(y >= 644 ? 'B' : null, t)
  }
  const came = t - 16 - 16 * Math.floor((664 - 644) / 8)
  assert.equal(down.due(shown('A'), came + SWITCH_MS), 0)
})

test('heading wants a move within REST_MS, and two points to go by', () => {
  const aim = create()
  assert.equal(aim.heading(shown('A'), 0), false)
  aim.move(A.x, A.y, 0)
  assert.equal(aim.heading(shown('A'), 0), false, 'one point')
  aim.move(A.x - 4, A.y - 4, 16)
  assert.equal(aim.heading(shown('A'), 16), true)
  assert.equal(aim.heading(shown('A'), 16 + REST_MS + 1), false, 'at rest since')
  assert.equal(aim.heading(null, 16), false, 'no list')
})

test("the trail keeps about TRAIL_MS: the way is judged over it, not by one step's pixel", () => {
  const aim = create()
  // Up-left at a slant, but the last step alone flat (a pixel left).
  aim.move(706, 664, 0)
  aim.move(700, 662, 40)
  aim.move(694, 660, 80)
  aim.move(693, 660, 96)
  assert.equal(aim.heading(shown('A'), 96), true)
  // Long flat since: judged from where it was TRAIL_MS ago, flat.
  for (let k = 1; k <= 10; k++) aim.move(693 - k, 660, 96 + 16 * k)
  assert.equal(aim.heading(shown('A'), 256), false)
})
