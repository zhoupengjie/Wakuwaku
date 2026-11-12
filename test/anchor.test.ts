

// Her island at a top corner (zhoupengjie/Wakuwaku#59): where the page lays
// it out, how it grows, what hangs under it, where its window goes.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import * as A from '../src/pet/anchor.ts'

test('the side the island is laid out at: where its window was put, at the top of the screen only', () => {
  assert.equal(A.anchorOf({ home: 'island', at: 'left' }), 'left')
  assert.equal(A.anchorOf({ home: 'island', at: 'right' }), 'right')
  assert.equal(A.anchorOf({ home: 'island', at: 'center' }), 'center')
  // Nothing said (island.rs not placing it so yet), or anything else: the centre.
  for (const at of [undefined, null, '', 'centre', 'bottom-left', 1]) assert.equal(A.anchorOf({ home: 'island', at }), 'center', String(at))
  // On the taskbar, and round a Mac's notch: the centre (the notch's place).
  assert.equal(A.anchorOf({ home: 'taskbar', at: 'left' }), 'center')
  assert.equal(A.anchorOf({ home: 'island', at: 'right', notch: true }), 'center')
  assert.deepEqual(A.POSITIONS, ['left', 'center', 'right'])
  assert.equal(A.posOf('right'), 'right')
  assert.equal(A.posOf('top-right'), 'center')
})

test('opening, it grows toward the screen: from its middle at the centre, from its side at a corner', () => {
  const closed = { left: 230, top: 8, width: 300, height: 36 }
  assert.deepEqual(A.grown(closed, 420, 140, 'center'), { x: 170, y: 8, width: 420, height: 140 })
  // From the left corner, to the right: its left edge kept.
  const left = { left: 8, top: 8, width: 300, height: 36 }
  assert.deepEqual(A.grown(left, 420, 140, 'left'), { x: 8, y: 8, width: 420, height: 140 })
  // From the right corner, to the left: its right edge kept (8 px in from the window's 760).
  const right = { left: 452, top: 8, width: 300, height: 36 }
  assert.deepEqual(A.grown(right, 420, 140, 'right'), { x: 332, y: 8, width: 420, height: 140 })
  // Back to its closed size the same way.
  assert.deepEqual(A.grown({ left: 332, top: 8, width: 420, height: 140 }, 300, 36, 'right'), { x: 452, y: 8, width: 300, height: 36 })
})

test('hung under it (the capsule panel): under its middle, or lined up with its side; kept in the window', () => {
  const vw = 760
  assert.equal(A.under({ left: 230, width: 300 }, 340, 'center', vw), 210)
  assert.equal(A.under({ left: 8, width: 300 }, 340, 'left', vw), 8)
  assert.equal(A.under({ left: 452, width: 300 }, 340, 'right', vw), 412)
  // Never off the window's side.
  assert.equal(A.under({ left: 8, width: 120 }, 340, 'center', vw), 8)
  assert.equal(A.under({ left: 700, width: 52 }, 340, 'left', vw), vw - 340 - 8)
  assert.equal(A.SIDE, 8)
})

test("its window on the screen, as island.rs will put it: the work area's side, or its middle", () => {
  const area = [0, 0, 1600, 852]
  assert.equal(A.windowLeft('left', area, 760), 0)
  assert.equal(A.windowLeft('center', area, 760), 420)
  assert.equal(A.windowLeft('right', area, 760), 840)
  // A work area that does not start at the screen's left (Windows' own taskbar on the left).
  assert.equal(A.windowLeft('left', [48, 0, 1552, 900], 760), 48)
  assert.equal(A.windowLeft('right', [48, 0, 1552, 900], 760), 840)
})

// Along the foot of the screen (islandEdge bottom, zhoupengjie/Wakuwaku#105):
// over Windows' own taskbar, which stays as it is.
test('the edge the island is laid along: where its window was put, the top unless island.rs says the bottom', () => {
  assert.deepEqual(A.EDGES, ['top', 'bottom'])
  assert.equal(A.edgeAt({ home: 'island', edge: 'bottom' }), 'bottom')
  assert.equal(A.edgeAt({ home: 'island', edge: 'top' }), 'top')
  // Nothing said (an older island.rs), or anything else: the top.
  for (const edge of [undefined, null, '', 'foot', 'Bottom', 1]) assert.equal(A.edgeAt({ home: 'island', edge }), 'top', String(edge))
  // On the taskbar (its strip has its own edge), and round a Mac's notch: the top.
  assert.equal(A.edgeAt({ home: 'taskbar', edge: 'bottom' }), 'top')
  assert.equal(A.edgeAt({ home: 'island', edge: 'bottom', notch: true }), 'top')
  assert.equal(A.edgeOf('bottom'), 'bottom')
  assert.equal(A.edgeOf('left'), 'top')
})

test("at the foot: centred in Windows' taskbar's strip under it, or 8 px up from the work area's foot", () => {
  // Windows 11's 48 px strip: 6 px up; Windows 10's 40: 2; one thinner than the island: on the foot.
  assert.equal(A.foot(48), 6)
  assert.equal(A.foot(40), 2)
  assert.equal(A.foot(30), 0)
  // No taskbar along the foot: as far up as it hangs down from the top.
  assert.equal(A.foot(0), A.SIDE)
  assert.equal(A.CAPSULE, 36)
  // Opening, it grows upward from its foot, about its middle or from its side as at the top.
  const closed = { left: 230, top: 644, width: 300, height: 36 }
  assert.deepEqual(A.grown(closed, 420, 140, 'center', 'bottom'), { x: 170, y: 540, width: 420, height: 140 })
  assert.deepEqual(A.grown({ left: 8, top: 644, width: 300, height: 36 }, 420, 140, 'left', 'bottom'), { x: 8, y: 540, width: 420, height: 140 })
  // Back to its closed size the same way; the top kept at the top, as ever.
  assert.deepEqual(A.grown({ left: 170, top: 540, width: 420, height: 140 }, 300, 36, 'center', 'bottom'), { x: 230, y: 644, width: 300, height: 36 })
  assert.deepEqual(A.grown(closed, 420, 140, 'center', 'top'), { x: 170, y: 644, width: 420, height: 140 })
  assert.deepEqual(A.grown(closed, 420, 140, 'center'), { x: 170, y: 644, width: 420, height: 140 })
})

// Her capsule inside Windows' taskbar (embed.rs): its width within the room it has there.
test('the capsule as wide as set while its stretch of the taskbar has room, else narrow, else her alone', () => {
  const widths = { narrow: 240, normal: 300, wide: 380 }
  // No room said (not in the taskbar): as set.
  for (const room of [0, -1, undefined, null, NaN]) assert.equal(A.capsuleWidth(300, room, widths), 300, String(room))
  assert.equal(A.capsuleWidth(380, 400, widths), 380)
  assert.equal(A.capsuleWidth(380, 380, widths), 380)
  // Short of the set width: narrow; short of that: 0, her portrait alone.
  assert.equal(A.capsuleWidth(380, 379, widths), 240)
  assert.equal(A.capsuleWidth(300, 250, widths), 240)
  assert.equal(A.capsuleWidth(300, 239, widths), 0)
  assert.equal(A.capsuleWidth(240, 100, widths), 0)
  // Set narrow already: narrow or her alone.
  assert.equal(A.capsuleWidth(240, 240, widths), 240)
  // No widths told: the set width, or her alone.
  assert.equal(A.capsuleWidth(300, 250), 0)
})

test("its window at the foot, as island.rs will put it: its foot on the screen's bottom edge, or the work area's", () => {
  const area = [0, 0, 1600, 852]
  // Windows' taskbar 48 px along the foot: the window's foot on the screen's (900).
  assert.equal(A.windowTop('bottom', area, 688, 48), 212)
  // None there (along another edge, or hiding itself): on the work area's foot.
  assert.equal(A.windowTop('bottom', area, 688), 164)
  assert.equal(A.windowTop('bottom', [0, 48, 1600, 852], 688), 212)
  // At the top: the work area's top, whatever the strip.
  assert.equal(A.windowTop('top', area, 132, 48), 0)
  assert.equal(A.windowTop('top', [0, 48, 1600, 852], 132), 48)
})
