const assert = require('node:assert/strict')
const { readFileSync } = require('node:fs')
const { test } = require('node:test')
const vm = require('node:vm')

test('a DPR-only resize resends unchanged settings hit regions, once per scale', () => {
  const source = readFileSync(require.resolve('../src/pet/island.js'), 'utf8').replace(/\r\n/g, '\n')
  const solid = source.slice(source.indexOf("  let solidSent = ''"), source.indexOf('  const solidWatch ='))
  const resize = source.slice(source.indexOf('  let stripThick =')).match(/  window\.addEventListener\('resize', \(\) => \{[\s\S]*?\n  \}\)/)[0]
  const sent = []
  const listeners = {}
  let redraws = 0
  // Keep the CSS bounds fixed: a scale-only change must still reach Rust.
  const settings = { isConnected: true, getBoundingClientRect: () => ({ left: 40, top: 500, width: 520, height: 100 }) }
  const page = vm.createContext({
    devicePixelRatio: 1,
    stripThick: 48,
    editLayout: null,
    isOn: () => true,
    fitPage: () => {},
    document: { documentElement: { style: { setProperty: () => {} } } },
    window: {
      Settings: { isOpen: () => true, layer: settings, redraw: () => redraws++ },
      pet: { editing: (on, rects) => sent.push({ on, rects: JSON.parse(JSON.stringify(rects)), dpr: page.devicePixelRatio }) },
      addEventListener: (type, listener) => { listeners[type] = listener },
    },
  })
  vm.runInContext(solid + '\n' + resize + '\nsendSolid()', page)
  assert.equal(sent.length, 1)
  listeners.resize()
  assert.equal(sent.length, 1)
  page.devicePixelRatio = 1.25
  listeners.resize()
  assert.equal(sent.length, 2)
  assert.deepEqual(sent[1].rects, sent[0].rects)
  assert.equal(sent[1].dpr, 1.25)
  assert.equal(redraws, 2)
  listeners.resize()
  assert.equal(sent.length, 2)
})
