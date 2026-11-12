import assert from 'node:assert/strict'
import { test } from 'node:test'
import vm from 'node:vm'
import { pageCode } from './stripped.ts'

test('a DPR-only resize resends unchanged settings hit regions, once per scale', () => {
  const source = pageCode('pet', 'island.ts')
  // The strip's thickness and the resize that tells it are the taskbar's look's (island-look.ts).
  const look = pageCode('pet', 'island-look.ts')
  const solid = source.slice(source.indexOf('\n  function sendSolid()'), source.indexOf('\n  const solidWatch ='))
  const resize = look.slice(look.indexOf('  let stripThick =')).match(/  window\.addEventListener\('resize', \(\) => \{[\s\S]*?\n  \}\)/)[0]
  const sent = []
  const listeners: Record<string, () => void> = {}
  let redraws = 0
  // Keep the CSS bounds fixed: a scale-only change must still reach Rust.
  const settings = { isConnected: true, getBoundingClientRect: () => ({ left: 40, top: 500, width: 520, height: 100 }) }
  const page = vm.createContext({
    devicePixelRatio: 1,
    stripThick: 48,
    // What the page remembers (island-vars.ts): the layout being edited, the rects last sent.
    st: { editLayout: null, solidSent: '' },
    isOn: () => true,
    fitPage: () => {},
    document: { documentElement: { style: { setProperty: () => {} } } },
    // The picker's and the settings' faces, given to the island once made
    // (island.ts later), and the bridge's pet (imported).
    later: { Picker: null, Settings: { isOpen: () => true, layer: settings, redraw: () => redraws++ } },
    pet: { editing: (on, rects) => sent.push({ on, rects: JSON.parse(JSON.stringify(rects)), dpr: page.devicePixelRatio }) },
    window: {
      addEventListener: (type, listener) => {
        listeners[type] = listener
      },
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
