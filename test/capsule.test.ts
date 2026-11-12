

// Her capsule as a container (the KDE-like picker of 2026-10-17): its
// members' order and modes (capsule.js), and what the island makes of them
// (deskgrid.js placement).
import assert from 'node:assert/strict'
import { test } from 'node:test'
import * as C from '../src/pet/capsule.ts'
import * as G from '../src/pet/deskgrid.ts'

const all = [
  { id: 'monitor', builtIn: true, on: true },
  { id: 'today', builtIn: true, on: true },
  { id: 'tokens', builtIn: true, on: true },
  { id: 'weather', plugin: 'weather', on: true },
  { id: 'stock-aapl', plugin: 'stock', on: true },
  { id: 'stock-nvda', plugin: 'stock', on: true },
  { id: 'off', on: false },
]
const ids = list => list.map(w => w.id)
const keys = list => list.map(m => m.key)
const modes = list => Object.fromEntries(list.map(m => [m.key, m.mode]))

test('without the setting: the widgets in their order, a plugin once, the monitor pinned by its pin in the island home only', () => {
  const island = C.derive(all, { home: 'island' })
  assert.deepEqual(keys(island), ['widget:monitor', 'widget:today', 'widget:tokens', 'plugin:weather', 'plugin:stock', 'widget:off'])
  assert.equal(modes(island)['widget:monitor'], 'pin')
  assert.equal(modes(C.derive(all, { home: 'island', pin: false }))['widget:monitor'], 'turn')
  // The taskbar her home: its readings are the strip's part, or it takes turns (#53).
  assert.equal(modes(C.derive(all, { home: 'taskbar' }))['widget:monitor'], 'turn')
  assert.ok(island.filter(m => m.key !== 'widget:monitor').every(m => m.mode === 'turn'))
})

test('the members as they are: the list order, those on and not listed at the end, those off or elsewhere left out', () => {
  const capsule = { members: [{ key: 'plugin:stock', mode: 'turn' }, { key: 'widget:gone', mode: 'turn' }, { key: 'widget:today', mode: 'open' }, { key: 'widget:monitor', mode: 'pin' }] }
  const inCapsule = w => w.on !== false && w.id !== 'tokens'
  const m = C.effective({ capsule, all, home: 'island', inCapsule })
  assert.deepEqual(keys(m), ['plugin:stock', 'widget:today', 'widget:monitor', 'plugin:weather'])
  assert.deepEqual(ids(m[0].widgets), ['stock-aapl', 'stock-nvda'])
  assert.deepEqual(modes(m), { 'plugin:stock': 'turn', 'widget:today': 'open', 'widget:monitor': 'pin', 'plugin:weather': 'turn' })
  // The island home's monitor: its pin says pinned or not; 'open' is the capsule's own.
  assert.equal(modes(C.effective({ capsule, all, home: 'island', pin: false, inCapsule }))['widget:monitor'], 'turn')
  const open = { members: [{ key: 'widget:monitor', mode: 'open' }] }
  assert.equal(modes(C.effective({ capsule: open, all, home: 'island' }))['widget:monitor'], 'open')
  // A list written by hand with six pinned: the sixth takes turns.
  const six = { members: ['today', 'tokens', 'off', 'monitor', 'weather', 'stock-aapl'].map(id => ({ key: id === 'weather' ? 'plugin:weather' : id === 'stock-aapl' ? 'plugin:stock' : `widget:${id}`, mode: 'pin' })) }
  const m6 = C.effective({ capsule: six, all: all.map(w => ({ ...w, on: true })), home: 'taskbar' })
  assert.deepEqual(m6.filter(x => x.mode === 'pin').map(x => x.key), ['widget:today', 'widget:tokens', 'widget:off', 'widget:monitor', 'plugin:weather'])
  assert.equal(modes(m6)['plugin:stock'], 'turn')
  // Junk kept out: unknown modes, keys, twice the same.
  assert.deepEqual(C.storedOf({ members: [{ key: 'widget:a', mode: 'x' }, { key: 'bad', mode: 'turn' }, { key: 'widget:b', mode: 'turn' }, { key: 'widget:b', mode: 'pin' }] }), [{ key: 'widget:b', mode: 'turn' }])
  assert.equal(C.storedOf(undefined), null)
})

test("five pinned at most (the user's call of 2026-10-17; it was two): a sixth refused; turns and open always", () => {
  assert.equal(C.MAX_PINS, 5)
  let list = ['a', 'b', 'c', 'd', 'e', 'f'].map(k => ({ key: `widget:${k}`, mode: 'turn' }))
  for (const k of ['a', 'b', 'c', 'd', 'e']) list = C.setMode(list, `widget:${k}`, 'pin')
  assert.deepEqual(list.filter(m => m.mode === 'pin').length, 5)
  assert.equal(C.canPin(list, 'widget:f'), false)
  assert.equal(C.canPin(list, 'widget:a'), true)
  assert.equal(modes(C.setMode(list, 'widget:f', 'pin'))['widget:f'], 'turn')
  assert.equal(modes(C.setMode(list, 'widget:f', 'open'))['widget:f'], 'open')
  // One of the five let go: the sixth may.
  assert.equal(C.canPin(C.setMode(list, 'widget:a', 'turn'), 'widget:f'), true)
  // Added pinned with five there: takes turns.
  assert.equal(modes(C.add(list, 'widget:g', -1, 'pin'))['widget:g'], 'turn')
  // Nothing for a key not there, or a mode not known.
  assert.deepEqual(C.setMode(list, 'widget:z', 'pin'), list)
  assert.deepEqual(C.setMode(list, 'widget:c', 'big'), list)
})

test('the order by hand: moved, added at its end or a place, taken out and put back where it was (undo)', () => {
  const list = ['a', 'b', 'c', 'd'].map(k => ({ key: `widget:${k}`, mode: 'turn' }))
  assert.deepEqual(keys(C.move(list, 0, 2)), ['widget:b', 'widget:c', 'widget:a', 'widget:d'])
  assert.deepEqual(keys(C.move(list, 3, 0)), ['widget:d', 'widget:a', 'widget:b', 'widget:c'])
  assert.deepEqual(keys(C.move(list, 1, 9)), keys(list))
  assert.deepEqual(keys(C.add(list, 'plugin:weather')), [...keys(list), 'plugin:weather'])
  assert.deepEqual(keys(C.add(list, 'plugin:weather', 1)), ['widget:a', 'plugin:weather', 'widget:b', 'widget:c', 'widget:d'])
  assert.deepEqual(keys(C.add(list, 'widget:b')), keys(list), 'there already: as it was')
  const pinned = C.setMode(list, 'widget:c', 'pin')
  const { list: out, record } = C.remove(pinned, 'widget:c')
  assert.deepEqual(keys(out), ['widget:a', 'widget:b', 'widget:d'])
  assert.deepEqual(record, { key: 'widget:c', mode: 'pin', index: 2 })
  const back = C.restore(out, record)
  assert.deepEqual(keys(back), keys(list))
  assert.equal(modes(back)['widget:c'], 'pin')
  // Put back into a shorter list: at its end.
  assert.deepEqual(keys(C.restore([{ key: 'widget:a', mode: 'turn' }], record)), ['widget:a', 'widget:c'])
  assert.equal(C.remove(list, 'widget:z').record, null)
})

test('written as the capsule and, for the Rust side of now, widgetOrder and the monitor pin', () => {
  const list = [{ key: 'widget:monitor', mode: 'turn' }, { key: 'plugin:weather', mode: 'pin' }, { key: 'widget:today', mode: 'open' }]
  const p = C.patchFor({ widgetSpin: 5 }, list, 'island')
  assert.deepEqual(p.capsule, { members: list, spin: 5 })
  assert.deepEqual(p.widgetOrder, ['monitor', 'weather', 'today'])
  assert.deepEqual(p.monitor, { pin: false })
  // Pinned and its pin on already: nothing more to say; the taskbar home: never.
  assert.equal(C.patchFor({ monitor: { pin: true, cpu: true } }, C.setMode(list, 'widget:monitor', 'pin'), 'island').monitor, undefined)
  assert.equal(C.patchFor({}, list, 'taskbar').monitor, undefined)
  // The turns' length: widgetSpin, the panel setting both.
  assert.equal(C.spinOf({}), 8)
  assert.equal(C.spinOf({ widgetSpin: 15, capsule: { spin: 5 } }), 15)
  assert.deepEqual(C.spinPatch({ capsule: { members: list } }, 5), { widgetSpin: 5, capsule: { members: list, spin: 5 } })
  assert.equal(C.spinPatch({}, 7), null)
})

test('in the island: members in order (turns, pinned at its end, the open ones listed only); those elsewhere not listed', () => {
  const capsule = {
    members: [
      { key: 'plugin:weather', mode: 'pin' },
      { key: 'widget:tokens', mode: 'turn' },
      { key: 'widget:today', mode: 'open' },
      { key: 'plugin:stock', mode: 'turn' },
    ],
  }
  const desk = [{ id: 'wa', type: 'widget', of: 'monitor', size: 'm' }]
  const p = G.placement({ all, home: 'island', desk, capsule })
  assert.deepEqual(ids(p.turning), ['tokens', 'stock-aapl', 'stock-nvda'])
  assert.deepEqual(ids(p.pins), ['weather'])
  assert.equal(p.pinned, false)
  // The members' widgets alone (one widget, one place: the monitor is on the desktop).
  assert.deepEqual(ids(p.listed), ['weather', 'tokens', 'today', 'stock-aapl', 'stock-nvda'])
  assert.deepEqual(ids(p.listed), p.members.flatMap(m => ids(m.widgets)))
  assert.equal(p.where, undefined)
  // The taskbar her home: one on the strip is not listed; its monitor part shows its readings.
  const bar = { left: ['her'], center: ['start', 'apps'], right: [{ id: 'w1', type: 'plugin', of: 'stock', size: 'M' }, 'monitor', 'tray', 'keys', 'quick', 'clock'] }
  const q = G.placement({ all, home: 'taskbar', taskbar: bar, capsule })
  assert.equal(q.pinned, true)
  assert.deepEqual(ids(q.turning), ['tokens'])
  assert.deepEqual(ids(q.listed), ['weather', 'tokens', 'today'])
  assert.deepEqual(keys(q.members), ['plugin:weather', 'widget:tokens', 'widget:today'])
})

test("an old config: a plugin on and put nowhere is her capsule's member, listed in the open island; no migration", () => {
  // The Plugins page's switch of before turned it on, nothing put it anywhere.
  const old = [...all.filter(w => w.id !== 'off'), { id: 'ci-main', plugin: 'ci', on: true }]
  for (const capsule of [undefined, { members: [{ key: 'widget:today', mode: 'turn' }] }]) {
    const p = G.placement({ all: old, home: 'island', desk: [], capsule })
    assert.ok(keys(p.members).includes('plugin:ci'), JSON.stringify(capsule))
    assert.ok(ids(p.listed).includes('ci-main'))
    assert.ok(ids(p.turning).includes('ci-main'))
  }
  // On the strip of her taskbar home, it is the strip's alone.
  const bar = { left: ['her'], center: ['start', 'apps'], right: [{ id: 'w1', type: 'plugin', of: 'ci', size: 'M' }, 'tray'] }
  const q = G.placement({ all: old, home: 'taskbar', taskbar: bar })
  assert.equal(keys(q.members).includes('plugin:ci'), false)
  assert.equal(ids(q.listed).includes('ci-main'), false)
})

test("a new script's widget joins her capsule taking turns (轮播), at its end, the setting or none", () => {
  const script = { id: 'deploy', from: 'script', on: true }
  for (const capsule of [undefined, { members: [{ key: 'widget:today', mode: 'pin' }] }]) {
    const m = C.effective({ capsule, all: [...all, script], home: 'taskbar' })
    assert.equal(m.at(-1).key, 'widget:deploy')
    assert.equal(m.at(-1).mode, 'turn')
  }
})

test('five pinned in the room there is: as they are while they fit, then values off from the left (the monitor last, the others back that fit), then icons off from the left with a +k', () => {
  // The monitor's readings (200 px, its icon 16), four others (80 px, their icons 16); 8 between, the +k 24.
  const pins = [{ full: 200, icon: 16, monitor: true }, ...Array.from({ length: 4 }, () => ({ full: 80, icon: 16 }))]
  const fit = room => C.fitPins(pins, room, { gap: 8, plus: 24 })
  // All of it: 200 + 4 * 80 + 4 * 8 = 552.
  assert.deepEqual(fit(552), { modes: ['full', 'full', 'full', 'full', 'full'], more: 0 })
  // A little less: the leftmost other's value goes (the monitor's kept).
  assert.deepEqual(fit(551).modes, ['full', 'icon', 'full', 'full', 'full'])
  assert.deepEqual(fit(552 - 64 * 3).modes, ['full', 'icon', 'icon', 'icon', 'full'])
  // The others all icons (200 + 4 * 16 + 32 = 296), then the monitor's readings.
  assert.deepEqual(fit(296).modes, ['full', 'icon', 'icon', 'icon', 'icon'])
  // Its readings as its icon, the values back that fit again from the right (16 + 2 * 16 + 2 * 80 + 32 = 240).
  assert.deepEqual(fit(295).modes, ['icon', 'icon', 'icon', 'full', 'full'])
  assert.deepEqual(fit(239).modes, ['icon', 'icon', 'icon', 'icon', 'full'])
  // Icons alone (5 * 16 + 32 = 112), then the leftmost out with a +k before the rest.
  assert.deepEqual(fit(112), { modes: ['icon', 'icon', 'icon', 'icon', 'icon'], more: 0 })
  // (one out would be 4 * 16 + 24 + 4 * 8 = 120: the +k as wide as an icon and its gap, two go)
  assert.deepEqual(fit(111), { modes: ['hidden', 'hidden', 'icon', 'icon', 'icon'], more: 2 })
  assert.deepEqual(fit(64), { modes: ['hidden', 'hidden', 'hidden', 'hidden', 'icon'], more: 4 })
  assert.deepEqual(fit(24), { modes: ['hidden', 'hidden', 'hidden', 'hidden', 'hidden'], more: 5 })
  // No room even for that: none shown.
  assert.deepEqual(fit(10), { modes: ['hidden', 'hidden', 'hidden', 'hidden', 'hidden'], more: 0 })
  assert.deepEqual(C.fitPins([], 0), { modes: [], more: 0 })
})
