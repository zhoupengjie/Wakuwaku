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

test("the clear strip's words dark only where black reads better than white", () => {
  // The wallpaper under the strip in the report: a dark plum (#1c1318).
  const lin = v => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4)
  const y = (r, g, b) => 0.2126 * lin(r / 255) + 0.7152 * lin(g / 255) + 0.0722 * lin(b / 255)
  assert.equal(Mica.wantsDarkInk(y(0x1c, 0x13, 0x18)), false)
  assert.equal(Mica.wantsDarkInk(0), false)
  assert.equal(Mica.wantsDarkInk(1), true)
  // Windows' light grey taskbar, and a mid grey: dark; a deeper grey: light.
  assert.equal(Mica.wantsDarkInk(y(0xf3, 0xf3, 0xf3)), true)
  assert.equal(Mica.wantsDarkInk(y(0x80, 0x80, 0x80)), true)
  assert.equal(Mica.wantsDarkInk(y(0x60, 0x60, 0x60)), false)
  // The two read alike at about 0.18.
  assert.equal(Mica.wantsDarkInk(0.17), false)
  assert.equal(Mica.wantsDarkInk(0.19), true)
})

test("only the clear strip's own words go by the desktop under them", () => {
  const plum = Mica.wantsDarkInk(0.008)
  // Windows' light mode, a deep plum wallpaper under the clear strip: light
  // words on it; Windows' dark mode, a light one: dark words.
  assert.equal(Mica.stripInkLight({ light: true, clear: true, filled: false, backdrop: plum }), false)
  assert.equal(Mica.stripInkLight({ light: false, clear: true, filled: false, backdrop: true }), true)
  // A window maximized fills the strip in: as Windows' mode again.
  assert.equal(Mica.stripInkLight({ light: true, clear: true, filled: true, backdrop: false }), true)
  assert.equal(Mica.stripInkLight({ light: false, clear: true, filled: true, backdrop: true }), false)
  // The picture not read (yet): as Windows' mode.
  assert.equal(Mica.stripInkLight({ light: true, clear: true, filled: false, backdrop: null }), true)
  assert.equal(Mica.stripInkLight({ light: false, clear: true, filled: false, backdrop: null }), false)
  // The Mica or solid strip: as Windows' mode, whatever the picture.
  assert.equal(Mica.stripInkLight({ light: true, clear: false, filled: false, backdrop: false }), true)
  assert.equal(Mica.stripInkLight({ light: false, clear: false, filled: false, backdrop: true }), false)
})

test("the desktop's lightness along the strip, column by column", () => {
  // Three columns by two rows: black, white, mid grey (#808080, 0.216).
  const px = [0, 0, 0, 255, 255, 255, 128, 128, 128]
  const d = new Uint8ClampedArray([...px, ...px].flatMap((v, i) => (i % 3 === 2 ? [v, 255] : [v])))
  const along = Mica.profileOf(d, 3, 2)
  assert.equal(along.step, 10)
  assert.deepEqual(
    along.ys.map(y => +y.toFixed(3)),
    [0, 1, 0.216],
  )
  assert.equal(+along.mean.toFixed(3), 0.405)
  // A side bar's down it: its rows, each the same.
  const down = Mica.profileOf(d, 3, 2, true)
  assert.deepEqual(
    down.ys.map(y => +y.toFixed(3)),
    [0.405, 0.405],
  )
})

// The report's wallpaper (a deep blue, a light brown across the middle) as
// it lies under a 2560 px strip: dark at both ends, light in the middle, its
// mean (0.211) over the line, so the whole strip's words went dark and the
// clock and the tray on the deep blue at its right end were lost.
test("each part's words by the desktop under it alone", () => {
  const ys = [0.047, 0.21, 0.316, 0.231, 0.24, 0.45, 0.45, 0.074, 0.054, 0.035]
  const along = { step: 256, ys, mean: ys.reduce((a, y) => a + y, 0) / ys.length }
  assert.equal(Mica.wantsDarkInk(along.mean), true)
  // The clock and the tray at the right end: light words.
  assert.equal(Mica.inkOn(along, 2200, 2560), false)
  // The programs' buttons in the middle: dark ones.
  assert.equal(Mica.inkOn(along, 1100, 1500), true)
  // Across a dark column and a light one: the ink that reads better on
  // the one it reads worse on (white on 0.45 at 2.1:1, black on 0.074 at
  // 2.5:1).
  assert.equal(Mica.inkOn(along, 1700, 1900), true)
  // Order does not matter; nothing under it, or nothing known: none.
  assert.equal(Mica.inkOn(along, 2560, 2200), false)
  assert.equal(Mica.inkOn(along, 3000, 3100), null)
  assert.equal(Mica.inkOn(along, 500, 500), null)
  assert.equal(Mica.inkOn(null, 0, 100), null)
  // One column alone: as wantsDarkInk.
  for (const y of [0.1, 0.17, 0.19, 0.5]) assert.equal(Mica.inkOn({ step: 10, ys: [y] }, 0, 10), Mica.wantsDarkInk(y))
})

test("Windows' accent in the shade its taskbar takes for the words beside it", () => {
  const accent = { light: '#4cc2ff', dark: '#0067c0' }
  // Light words (the dark strip, or a deep plum wallpaper under the clear
  // one in Windows' light mode): the lighter shade; dark words: the darker.
  assert.equal(Mica.accentShade(accent, false), '#4cc2ff')
  assert.equal(Mica.accentShade(accent, true), '#0067c0')
  // Not known yet: none (style.css falls back to its own blue).
  assert.equal(Mica.accentShade(null, true), null)
  assert.equal(Mica.accentShade({ light: '#4cc2ff' }, true), null)
})

// What is drawn on the strip itself takes the accent by its words
// (--strip-accent); the settings and what opens from the strip by Windows'
// mode (--win-accent): a deep blue line under the window in front read dim
// on light words over a dark wallpaper before.
test("the strip's own marks take the accent by the strip's words", () => {
  const fs = require('node:fs')
  const path = require('node:path')
  const css = file => fs.readFileSync(path.join(__dirname, '..', 'src', 'pet', file), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '')
  const rules = text =>
    text
      .split('}')
      .map(block => block.split('{'))
      .filter(([, body]) => body)
      .map(([head, body]) => ({ head: head.trim(), body }))
  const style = rules(css('style.css'))
  const by = name => style.filter(r => r.body.includes(name)).map(r => r.head)
  for (const head of by('--strip-accent')) {
    assert.match(head, /#bar-rest/, head)
    assert.doesNotMatch(head, /\.wpop|\.tfly|#island/, head)
  }
  for (const mark of ['.win.front::after', '.win .count', '.tpriv .fi', '.drop-before', '.drop-after']) {
    assert.ok(by('--strip-accent').some(h => h.includes(mark)), mark)
    assert.ok(!by('--win-accent').some(h => h.includes(mark)), mark)
  }
  // The window list opened from the strip still as Windows' mode.
  assert.ok(by('--win-accent').some(h => h.includes('.wpop')))
})

// The settings, what opens from the strip, her island and the picker stay
// as Windows' mode (tb-light, isl-light): the strip's words (tb-ink-light)
// turned them dark in Windows' light mode over a dark wallpaper before.
test("the strip's words' class reaches nothing but the strip", () => {
  const fs = require('node:fs')
  const path = require('node:path')
  const css = file => fs.readFileSync(path.join(__dirname, '..', 'src', 'pet', file), 'utf8')
  const selectors = text =>
    text
      .replace(/\/\*[\s\S]*?\*\//g, '')
      .split('}')
      .map(block => block.split('{')[0])
      // At the commas between selectors, not those in :is( ) or :not( ).
      .flatMap(head => head.split(/,(?![^(]*\))/))
      .map(s => s.trim())
      .filter(Boolean)
  const inked = ['style.css', 'picker.css', 'settings.css'].flatMap(f => selectors(css(f)).filter(s => s.includes('ink-light')).map(s => [f, s]))
  assert.ok(inked.length > 20)
  for (const [file, s] of inked) {
    assert.match(s, /#bar-rest/, `${file}: ${s}`)
    assert.doesNotMatch(s, /#island\b|#island-|\.tbedit|\.tblib|\.wpop|\.tfly|\.wgpop|\.wstk|\.wmenu/, `${file}: ${s}`)
  }
  // The settings follow Windows' mode alone, in every home (set-light).
  assert.ok(selectors(css('settings.css')).some(s => s.startsWith('.set-light #island-settings')))
  assert.ok(!selectors(css('settings.css')).some(s => s.includes('.tb-light')))
  // The strip's own words and its clear halo go by each part's class.
  const style = selectors(css('style.css'))
  assert.ok(style.includes('#bar-rest .ink-light'))
  assert.ok(style.includes('.tb-clear:not(.tb-filled) #bar-rest .ink-light'))
  assert.ok(style.includes('#bar-rest .tclock:is(.ink-light, .ink-light *) i'))
  assert.ok(!style.includes('.tb-light #bar-rest .tclock i'))
  assert.ok(style.includes('.tb-light #bar-rest .wpop'))
})

// Each part dark or light by what is under it alone (island.js inkParts):
// nothing on the strip goes by the whole strip's ink any more but the
// placeholder of a part being dragged, which lies between them.
test("the strip's parts take their words from their own class", () => {
  const fs = require('node:fs')
  const path = require('node:path')
  const css = file => fs.readFileSync(path.join(__dirname, '..', 'src', 'pet', file), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '')
  assert.doesNotMatch(css('style.css'), /tb-ink-light/)
  assert.deepEqual(css('picker.css').match(/[^}]*tb-ink-light[^{]*/g).map(s => s.trim()), ['.tb-ink-light #bar-rest .pk-ph.pk-no'])
})
