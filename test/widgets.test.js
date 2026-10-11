const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')
const { test } = require('node:test')

const Widgets = require('../src/pet/widgets')

test('every icon a widget may name has a drawing', () => {
  const text = fs.readFileSync(path.join(__dirname, '..', 'src-tauri', 'src', 'widgets.rs'), 'utf8')
  const names = [...text.match(/pub const ICONS: \[&str; \d+\] = \[([^\]]*)\]/)[1].matchAll(/"(\w+)"/g)].map(m => m[1])
  assert.ok(names.length > 10, `found only ${names.length} icons: the scan is broken`)
  for (const name of names) assert.ok(Widgets.ICONS[name], name)
  assert.match(Widgets.icon('nope'), /<circle/)
})

test('a widget says its words, a built-in one in her language', () => {
  const today = { label: { key: 'widget.today', vars: {} }, value: { key: 'widget.todayValue', vars: { turns: '12', time: '1:43', approvals: '5' } } }
  assert.deepEqual(Widgets.words('zh', today), { label: '今天', value: '12 轮 · 1:43 · 批准 5' })
  assert.deepEqual(Widgets.words('en', today), { label: 'Today', value: '12 turns · 1:43 · 5 approved' })
  assert.deepEqual(Widgets.words('zh', { label: '上海 · 多云', value: '22°' }), { label: '上海 · 多云', value: '22°' })
})

test("the monitor's readings in two rows: the machine's over the network's", () => {
  const p = (icon, text = '1') => ({ icon, text })
  const icons = rows => rows.map(row => row.map(x => x.icon))
  assert.deepEqual(icons(Widgets.rowsOf([p('cpu'), p('memory'), p('down'), p('up')])), [['cpu', 'memory'], ['down', 'up']])
  assert.deepEqual(icons(Widgets.rowsOf([p('cpu'), p('memory'), p('down'), p('up'), p('battery')])), [['cpu', 'memory', 'battery'], ['down', 'up']])
  // One kind only: half over half.
  assert.deepEqual(icons(Widgets.rowsOf([p('cpu'), p('memory')])), [['cpu'], ['memory']])
  assert.deepEqual(icons(Widgets.rowsOf([p('down'), p('up')])), [['down'], ['up']])
  assert.deepEqual(icons(Widgets.rowsOf([p('cpu'), p('memory'), p('battery')])), [['cpu', 'memory'], ['battery']])
  assert.deepEqual(icons(Widgets.rowsOf([p('cpu')])), [['cpu']])
  assert.deepEqual(Widgets.rowsOf([]), [])
})

test('a reading wants a look when the machine is nearly full or the battery low', () => {
  assert.equal(Widgets.isHigh({ icon: 'cpu', text: '90%' }), true)
  assert.equal(Widgets.isHigh({ icon: 'cpu', text: '89%' }), false)
  assert.equal(Widgets.isHigh({ icon: 'memory', text: '97%' }), true)
  assert.equal(Widgets.isHigh({ icon: 'battery', text: '20%' }), true)
  assert.equal(Widgets.isHigh({ icon: 'bolt', text: '5%' }), false)
  assert.equal(Widgets.isHigh({ icon: 'down', text: '999K' }), false)
  assert.match(Widgets.rowsHTML({ value: { parts: [{ icon: 'cpu', text: '95%' }, { icon: 'down', text: '1.2M' }] } }), /^<span class="wrow"><span class="wcell high" style="--p:95%">.*95%.*<\/span><span class="wrow"><span class="wcell" style="--p:62%">.*1\.2M/)
})

test("the taskbar's monitor leaves the battery to the tray", () => {
  const html = Widgets.rowsHTML({ value: { parts: [{ icon: 'cpu', text: '5%' }, { icon: 'battery', text: '72%' }, { icon: 'bolt', text: '40%' }, { icon: 'down', text: '1K' }] } })
  assert.doesNotMatch(html, /72%|40%/)
  assert.match(html, /5%.*1K/)
})

test('a reading as full as it goes: a share as it is, a rate by its order of magnitude', () => {
  const share = (icon, text) => Widgets.shareOf({ icon, text })
  assert.equal(share('cpu', '23%'), 23)
  assert.equal(share('memory', '100%'), 100)
  assert.equal(share('battery', '72%'), 72)
  // 1 KB/s empty, 100 MB/s full, each tenfold a fifth.
  assert.equal(share('down', '1.0K'), 0)
  assert.equal(share('down', '0B'), 0)
  assert.equal(share('up', '10K'), 20)
  assert.equal(share('down', '337K'), 51)
  assert.equal(share('down', '100M'), 100)
  assert.equal(share('down', '1.2G'), 100)
  assert.equal(share('cpu', '—'), 0)
})

test("a reading's glyph is Windows' own, the battery's by its level", () => {
  const glyph = (icon, text) => Widgets.glyphOf({ icon, text })
  assert.equal(glyph('cpu', '5%'), 'E950')
  assert.equal(glyph('memory', '5%'), 'E964')
  assert.equal(glyph('down', '5.1K'), 'EDDC')
  assert.equal(glyph('up', '337K'), 'EDDB')
  assert.match(Widgets.partsHTML({ value: { parts: [{ icon: 'down', text: '1K' }] } }), /class="wg tri"/)
  assert.equal(glyph('battery', '72%'), 'E857')
  assert.equal(glyph('battery', '100%'), 'E83F')
  assert.equal(glyph('bolt', '0%'), 'E85A')
})

test('a colour deepened until it reads on the light island, its hue kept', () => {
  const ratio = (a, b) => {
    const lum = hex => {
      const [r, g, b2] = [1, 3, 5].map(i => parseInt(hex.slice(i, i + 2), 16) / 255).map(v => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4))
      return 0.2126 * r + 0.7152 * g + 0.0722 * b2
    }
    return (lum(a) + 0.05) / (lum(b) + 0.05)
  }
  for (const c of ['#34d27b', '#ffb340', '#64d2ff', '#ffffff']) {
    const d = Widgets.deepen(c)
    assert.ok(ratio('#fbfbfb', d) >= 4.5, `${c} → ${d}`)
    assert.ok(ratio('#fbfbfb', d) < 6.5, `${c} → ${d} went too dark`)
  }
  // Already deep enough, or not a colour it knows: as it is.
  assert.equal(Widgets.deepen('#0f7b3f'), '#0f7b3f')
  assert.equal(Widgets.deepen('#123'), '#123')
  assert.equal(Widgets.deepen('tomato'), 'tomato')
  assert.equal(Widgets.deepen(undefined), undefined)
  // Its hue kept: green stays green.
  const [r, g, b] = [1, 3, 5].map(i => parseInt(Widgets.deepen('#34d27b').slice(i, i + 2), 16))
  assert.ok(g > r && g > b)
})

// --- On the taskbar ---------------------------------------------------------------------

const PARTS = ['her', 'start', 'apps', 'monitor', 'tray', 'keys', 'quick', 'clock', 'gear']
const WIN11 = { left: ['her'], center: ['start', 'apps'], right: ['monitor', 'tray', 'keys', 'quick', 'clock', 'gear'] }

test('a widget put on the taskbar is checked as settings.rs checks it', () => {
  assert.ok(Widgets.isEntry({ id: 'wa1', type: 'widget', of: 'today', size: 'M' }))
  assert.ok(Widgets.isEntry({ id: 'wa1', type: 'plugin', of: 'weather', icon: 'weather' }))
  assert.ok(Widgets.isEntry({ id: 'wa1', type: 'sep' }))
  assert.ok(!Widgets.isEntry({ id: 'start', type: 'sep' }))
  assert.ok(!Widgets.isEntry({ id: 'wa1', type: 'widget' }))
  assert.ok(!Widgets.isEntry({ id: 'wa1', type: 'gap', of: 'today' }))
  assert.ok(!Widgets.isEntry({ id: 'wa1', type: 'widget', of: 'today', size: 'L' }))
  assert.ok(!Widgets.isEntry({ id: 'wa1', type: 'widget', of: 'today', icon: 'nope' }))
  assert.ok(!Widgets.isEntry({ id: 'wa1', type: 'clock' }))
  assert.ok(!Widgets.isEntry('clock'))
})

test('a layout made whole: each part once, the missing ones where Windows has them, each widget once', () => {
  const w = { id: 'wa1', type: 'widget', of: 'today', size: 'S' }
  const got = Widgets.layoutOf({ left: ['start', 'her', 'start', 'nope', w], right: ['clock', { ...w }, { id: 'wb', type: 'flex' }] }, PARTS, WIN11)
  assert.deepEqual(got.left, ['start', 'her', w])
  assert.deepEqual(got.center, ['apps'])
  assert.deepEqual(got.right, ['clock', { id: 'wb', type: 'flex' }, 'monitor', 'tray', 'keys', 'quick', 'gear'])
  // A copy: changing it changes not what was kept.
  got.left[2].size = 'M'
  assert.equal(w.size, 'S')
  // Nothing kept: Windows' own.
  assert.deepEqual(Widgets.layoutOf(null, PARTS, WIN11), WIN11)
  // A part there is no longer (the sessions' tags, with the programs now): dropped.
  assert.deepEqual(Widgets.layoutOf({ center: ['start', 'apps', 'tags'] }, PARTS, WIN11).center, ['start', 'apps'])
  // 32 widgets at most.
  const many = Array.from({ length: 40 }, (_, i) => ({ id: `w${i}`, type: 'gap' }))
  assert.equal(Widgets.layoutOf({ right: many }, PARTS, WIN11).right.filter(p => typeof p !== 'string').length, 32)
})

test('a preset takes the parts, and the widgets stay in their runs', () => {
  const weather = { id: 'wa', type: 'plugin', of: 'weather' }
  const sep = { id: 'wb', type: 'sep' }
  const old = { left: ['her', weather], center: ['start', 'apps'], right: [sep, 'monitor', 'tray', 'keys', 'quick', 'clock', 'gear'] }
  const left = { left: ['her', 'start', 'apps'], center: [], right: WIN11.right }
  assert.deepEqual(Widgets.withParts(left, old), { left: ['her', 'start', 'apps', weather], center: [], right: [sep, ...WIN11.right] })
  // Which preset: by the parts alone.
  assert.deepEqual(Widgets.partsAlone(old), WIN11)
})

test("what a widget there shows: the one, or a plugin's every one; those shown take no turns in the island", () => {
  const all = [
    { id: 'today', builtIn: true, label: { key: 'widget.today' }, value: '3' },
    { id: 'stock-aapl', plugin: 'stock', label: 'AAPL', value: '231' },
    { id: 'stock-msft', plugin: 'stock', label: 'MSFT', value: '420' },
    { id: 'weather', plugin: 'weather', label: '上海', value: '22°' },
  ]
  const today = { id: 'wa', type: 'widget', of: 'today' }
  const stock = { id: 'wb', type: 'plugin', of: 'stock' }
  assert.deepEqual(Widgets.sourcesOf(today, all).map(w => w.id), ['today'])
  assert.deepEqual(Widgets.sourcesOf(stock, all).map(w => w.id), ['stock-aapl', 'stock-msft'])
  assert.deepEqual(Widgets.sourcesOf({ id: 'wc', type: 'flex' }, all), [])
  assert.deepEqual([...Widgets.shownOn({ left: ['her'], right: [today, 'clock', stock] }, all)].sort(), ['stock-aapl', 'stock-msft', 'today'])
})

test("a widget's name there: a plugin's, a built-in one's, a spacer's, a label as it was put there", () => {
  const all = [{ id: 'today', builtIn: true, label: { key: 'widget.today' }, value: '3' }, { id: 'inbox-a', label: 'me@example.com', value: '2', private: true }]
  assert.equal(Widgets.nameOf('zh', { type: 'plugin', of: 'weather' }), '天气')
  assert.equal(Widgets.nameOf('zh', { type: 'widget', of: 'today' }, all), '今天')
  assert.equal(Widgets.nameOf('en', { type: 'sep' }), 'Separator')
  // Private, with the specifics off: not its label.
  assert.equal(Widgets.nameOf('en', { type: 'widget', of: 'inbox-a' }, all, false), 'Widget')
  assert.equal(Widgets.nameOf('en', { type: 'widget', of: 'inbox-a' }, all, true), 'me@example.com')
  // Gone: as it was put there.
  assert.equal(Widgets.nameOf('en', { type: 'widget', of: 'gone', name: 'CI' }, all), 'CI')
})

test("a tool's settings: no control characters, C1's neither (as settings.rs has it)", () => {
  const e = cfg => Widgets.isEntry({ id: 'wa', type: 'tool', of: 'launch', cfg })
  assert.ok(e({ path: ['C:', 'Windows', 'notepad.exe'].join(String.fromCharCode(92)) }))
  assert.ok(!e({ path: 'a' + String.fromCharCode(9) + 'b' }))
  assert.ok(!e({ path: 'a' + String.fromCharCode(0x85) + 'b' }))
})
