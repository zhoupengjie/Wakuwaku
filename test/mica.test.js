const assert = require('node:assert/strict')
const { test } = require('node:test')

const Mica = require('../src/pet/mica')

// A 2560×1440 display at 125% (2048×1152 in the page's px), its strip of 48
// along the bottom: the display's top-left is 1104 above the strip's.
const paper = position => ({ position, mon: [0, -1104, 2048, 1152], screen: [0, -1104, 2048, 1152], sf: 1.25 })

test('a filled picture covers the display, cut evenly', () => {
  // As wide as the display's shape: scaled to it exactly.
  assert.deepEqual(Mica.place(paper('fill'), { w: 3840, h: 2160 }), { x: 0, y: -1104, w: 2048, h: 1152 })
  // Taller: as wide as the display, its top and foot cut.
  assert.deepEqual(Mica.place(paper('fill'), { w: 1000, h: 1000 }), { x: 0, y: -1104 - 448, w: 2048, h: 2048 })
  // Unknown ways of laying it are filled, as Windows' default.
  assert.deepEqual(Mica.place(paper('nope'), { w: 3840, h: 2160 }), Mica.place(paper('fill'), { w: 3840, h: 2160 }))
})

test('a fitted picture is all there, in the middle', () => {
  assert.deepEqual(Mica.place(paper('fit'), { w: 1000, h: 1000 }), { x: 448, y: -1104, w: 1152, h: 1152 })
})

test('stretched to the display, whatever its shape', () => {
  assert.deepEqual(Mica.place(paper('stretch'), { w: 1000, h: 1000 }), { x: 0, y: -1104, w: 2048, h: 1152 })
})

test('centred and tiled pictures keep their own size, in physical pixels', () => {
  assert.deepEqual(Mica.place(paper('center'), { w: 1000, h: 500 }), { x: 624, y: -1104 + 376, w: 800, h: 400 })
  assert.deepEqual(Mica.place(paper('tile'), { w: 100, h: 50 }), { x: 0, y: -1104, w: 80, h: 40, tile: true })
})

test('a spanned picture fills all the displays together', () => {
  // A second display of the same size to the right: the picture across both.
  const both = { ...paper('span'), screen: [0, -1104, 4096, 1152] }
  assert.deepEqual(Mica.place(both, { w: 7680, h: 2160 }), { x: 0, y: -1104, w: 4096, h: 1152 })
})
