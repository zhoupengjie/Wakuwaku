

// Elements as names, their boxes by name.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { stale, EDGE_PX } from '../src/pet/hovers.ts'

const boxes = {
  body: { left: 0, top: 0, right: 2048, bottom: 688 },
  bar: { left: 0, top: 0, right: 2048, bottom: 688 },
  island: { left: 6, top: 520, right: 306, bottom: 683 },
  her: { left: 18, top: 560, right: 114, bottom: 664 },
  rest: { left: 306, top: 640, right: 2048, bottom: 688 },
}
const boxOf = el => boxes[el]

test('entered by a played mouseenter, the island is left once the pointer is off it', () => {
  // Caught over the room above the strip: the island and her are left,
  // innermost first; what the pointer is still in stays.
  assert.deepEqual(stale(['body', 'bar', 'island', 'her'], ['body', 'bar'], { x: 1000, y: 200, caught: true }, boxOf), ['her', 'island'])
  // Still on the island (over her no more): her alone.
  assert.deepEqual(stale(['body', 'bar', 'island', 'her'], ['body', 'bar', 'island'], { x: 200, y: 600, caught: true }, boxOf), ['her'])
})

test('caught, nothing is left at its edge, where WebView2 may still have the pointer on it', () => {
  // A pixel past the island's right edge (on the strip's rest now): kept.
  const near = { x: 306 + EDGE_PX - 1, y: 660, caught: true }
  assert.deepEqual(stale(['body', 'bar', 'island'], ['body', 'bar', 'rest'], near, boxOf), [])
  // Clear of it: left.
  const clear = { x: 306 + EDGE_PX + 1, y: 660, caught: true }
  assert.deepEqual(stale(['body', 'bar', 'island'], ['body', 'bar', 'rest'], clear, boxOf), ['island'])
})

test('letting clicks through (forwarded), what the pointer is not on is left at once', () => {
  assert.deepEqual(stale(['body', 'bar', 'island'], ['body', 'bar', 'rest'], { x: 307, y: 660 }, boxOf), ['island'])
})

test('gone: all of it is left', () => {
  assert.deepEqual(stale(['body', 'bar', 'island'], [], null, boxOf), ['island', 'bar', 'body'])
})

test('a popup drawn outside its box is still in it while under the pointer', () => {
  // The strip's window list above the strip: in the rest by the DOM, not by its box.
  assert.deepEqual(stale(['body', 'bar', 'rest'], ['body', 'bar', 'rest'], { x: 600, y: 300, caught: true }, boxOf), [])
})
