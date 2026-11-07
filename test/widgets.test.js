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
  assert.deepEqual(Widgets.words('zh', today), { label: '今天', value: '12 轮 · 1:43 · 批准 5', detail: '' })
  assert.deepEqual(Widgets.words('en', today), { label: 'Today', value: '12 turns · 1:43 · 5 approved', detail: '' })
  assert.deepEqual(Widgets.words('zh', { label: '上海 · 多云', value: '22°' }), { label: '上海 · 多云', value: '22°', detail: '' })
  // A script's detail (the weather's range), for the open island.
  assert.deepEqual(Widgets.words('zh', { label: '上海 · 多云', value: '22°', detail: '14°~22°' }), { label: '上海 · 多云', value: '22°', detail: '14°~22°' })
  // Under a minute in seconds; Antigravity's turns alone, no approvals to count (widgets.rs Today::words).
  const short = { ...today, value: { key: 'widget.todayValueAgy', vars: { turns: '2', time: { key: 'widget.todaySeconds', vars: { n: '45' } }, approvals: '0' } } }
  assert.equal(Widgets.words('zh', short).value, '2 轮 · 45 秒 · 批准 —')
  assert.equal(Widgets.words('en', short).value, '2 turns · 45 s · approvals —')
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
  // A side bar's one column: a reading a row, the battery left out as ever.
  const monitor = { value: { parts: [p('cpu'), p('memory'), p('down'), p('up'), p('battery')] } }
  assert.equal((Widgets.rowsHTML(monitor, true).match(/class="wrow"/g) || []).length, 4)
  assert.equal((Widgets.rowsHTML(monitor).match(/class="wrow"/g) || []).length, 2)
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

// The hardware's sensors from LibreHardwareMonitor (lhm.rs): { icon, kind, text }.
test("a sensor's reading: its hardware's glyph, full and orange by its kind, after the machine's in the rows", () => {
  const s = (icon, kind, text) => ({ icon, kind, text })
  // A temperature from 30 °C empty to 100 °C full, orange from 90 °C; a
  // load as the CPU's; watts, a fan and a clock never full nor orange.
  assert.equal(Widgets.shareOf(s('cpu', 'temp', '62°')), 46)
  assert.equal(Widgets.shareOf(s('gpu', 'temp', '25°')), 0)
  assert.equal(Widgets.shareOf(s('gpu', 'temp', '104°')), 100)
  assert.equal(Widgets.shareOf(s('gpu', 'load', '37%')), 37)
  assert.equal(Widgets.shareOf(s('gpu', 'power', '122W')), 0)
  assert.equal(Widgets.shareOf(s('fan', 'fan', '1180')), 0)
  assert.equal(Widgets.isHigh(s('cpu', 'temp', '90°')), true)
  assert.equal(Widgets.isHigh(s('cpu', 'temp', '89°')), false)
  assert.equal(Widgets.isHigh(s('gpu', 'load', '95%')), true)
  assert.equal(Widgets.isHigh(s('gpu', 'power', '300W')), false)
  assert.equal(Widgets.isHigh(s('fan', 'fan', '2400')), false)
  const glyph = icon => Widgets.glyphOf({ icon, kind: 'temp', text: '50°' })
  assert.deepEqual(['cpu', 'gpu', 'board', 'disk', 'fan', 'memory', 'sensor', 'what'].map(glyph), ['E950', 'F211', 'F20C', 'EDA2', 'E72C', 'E964', 'E9D9', 'E950'])
  assert.match(Widgets.partsHTML({ value: { parts: [s('cpu', 'temp', '95°')] } }), /^<span class="wcell high" style="--p:93%">.*95°/)
  // In the taskbar's two rows, as many over as under, the machine's first.
  const icons = rows => rows.map(row => row.map(x => x.icon + (x.kind ? `:${x.kind}` : '')))
  const p = icon => ({ icon, text: '1' })
  assert.deepEqual(icons(Widgets.rowsOf([p('cpu'), p('memory'), p('down'), p('up'), s('cpu', 'temp', '62°'), s('gpu', 'temp', '58°'), s('gpu', 'load', '37%')])), [
    ['cpu', 'memory', 'cpu:temp', 'gpu:temp'],
    ['down', 'up', 'gpu:load'],
  ])
  assert.deepEqual(icons(Widgets.rowsOf([p('cpu'), p('memory'), p('down'), p('up'), s('cpu', 'temp', '62°')])), [['cpu', 'memory', 'cpu:temp'], ['down', 'up']])
  // Sensors alone, or with one kind: half over half.
  assert.deepEqual(icons(Widgets.rowsOf([s('cpu', 'temp', '62°'), s('gpu', 'temp', '58°'), s('gpu', 'load', '37%')])), [['cpu:temp', 'gpu:temp'], ['gpu:load']])
  assert.deepEqual(icons(Widgets.rowsOf([p('down'), p('up'), s('cpu', 'temp', '62°')])), [['down', 'up'], ['cpu:temp']])
})

// The built-in readings (hw.rs): a drive's temperature orange from its own
// warning; a drive read or written filled as the network's, but full at
// 1 GB/s, its glyph marked down or up; a card's clock and memory never full.
test("a built-in reading: a drive's own warning, its rates filled and marked", () => {
  const s = (icon, kind, text, warn) => ({ icon, kind, text, ...(warn === undefined ? {} : { warn }) })
  assert.equal(Widgets.isHigh(s('disk', 'temp', '84°', 84)), true)
  assert.equal(Widgets.isHigh(s('disk', 'temp', '83°', 84)), false)
  assert.equal(Widgets.isHigh(s('disk', 'temp', '91°', 94)), false)
  assert.equal(Widgets.isHigh(s('disk', 'temp', '90°')), true)
  assert.equal(Widgets.shareOf(s('disk', 'temp', '56°', 84)), 37)
  assert.deepEqual(['512B', '8.4K', '1.2M', '35M', '1.1G', '7.0G'].map(text => Widgets.shareOf(s('disk', 'read', text))), [0, 15, 51, 76, 100, 100])
  assert.equal(Widgets.shareOf(s('disk', 'write', '1.0M')), 50)
  // The network's own scale is as it was: 100 MB/s full.
  assert.equal(Widgets.shareOf({ icon: 'down', text: '1.0M' }), 60)
  assert.equal(Widgets.shareOf(s('gpu', 'clock', '2.3G')), 0)
  assert.equal(Widgets.shareOf(s('gpu', 'data', '1.2G')), 0)
  const html = Widgets.partsHTML({ value: { parts: [s('disk', 'read', '1.2M'), s('disk', 'write', '80K'), s('disk', 'load', '3%')] } })
  assert.match(html, /<span class="wg rd" aria-hidden="true">&#xEDA2;<\/span><b>1.2M<\/b>/)
  assert.match(html, /<span class="wg wr" aria-hidden="true">&#xEDA2;<\/span><b>80K<\/b>/)
  assert.match(html, /<span class="wg" aria-hidden="true">&#xEDA2;<\/span><b>3%<\/b>/)
  assert.match(Widgets.partsHTML({ value: { parts: [s('disk', 'temp', '85°', 84)] } }), /^<span class="wcell high"/)
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

const PARTS = ['her', 'start', 'apps', 'monitor', 'tray', 'keys', 'quick', 'clock']
const WIN11 = { left: ['her'], center: ['start', 'apps'], right: ['monitor', 'tray', 'keys', 'quick', 'clock'] }

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
  assert.deepEqual(got.right, ['clock', { id: 'wb', type: 'flex' }, 'monitor', 'tray', 'keys', 'quick'])
  // A copy: changing it changes not what was kept.
  got.left[2].size = 'M'
  assert.equal(w.size, 'S')
  // Nothing kept: Windows' own.
  assert.deepEqual(Widgets.layoutOf(null, PARTS, WIN11), WIN11)
  // A part there is no longer (the sessions' tags, the settings' gear): dropped.
  assert.deepEqual(Widgets.layoutOf({ center: ['start', 'apps', 'tags'] }, PARTS, WIN11).center, ['start', 'apps'])
  assert.deepEqual(Widgets.layoutOf({ right: ['clock', 'gear'] }, PARTS, WIN11).right, ['clock', 'monitor', 'tray', 'keys', 'quick'])
  // 32 widgets at most.
  const many = Array.from({ length: 40 }, (_, i) => ({ id: `w${i}`, type: 'gap' }))
  assert.equal(Widgets.layoutOf({ right: many }, PARTS, WIN11).right.filter(p => typeof p !== 'string').length, 32)
})

test("the monitor's part may be taken off, and stays off; a monitor put there as a widget is that part (#53)", () => {
  // Taken off and kept so: not put back as a part new since.
  const off = { left: ['her'], center: ['start', 'apps'], right: ['tray', 'keys', 'quick', 'clock'] }
  assert.deepEqual(Widgets.layoutOf(off, PARTS, WIN11, ['monitor']).right, ['tray', 'keys', 'quick', 'clock'])
  // The others are, as before (a layout kept from before a part was there).
  assert.deepEqual(Widgets.layoutOf({ left: ['her'], right: ['clock'] }, PARTS, WIN11, ['monitor']).right, ['clock', 'tray', 'keys', 'quick'])
  // Nothing kept: Windows' own, the monitor's part in it.
  assert.deepEqual(Widgets.layoutOf(null, PARTS, WIN11, ['monitor']), WIN11)
  // A monitor put there alone (a layout of before): the part, where it stood.
  const mon = { id: 'wm', type: 'widget', of: 'monitor', size: 'M' }
  const today = { id: 'wt', type: 'widget', of: 'today', size: 'S' }
  assert.deepEqual(Widgets.monitorAsPart({ left: ['her', mon], center: ['start', 'apps'], right: ['monitor', 'tray', 'clock'] }), { left: ['her', 'monitor'], center: ['start', 'apps'], right: ['tray', 'clock'] })
  // Twice: once, the first.
  assert.deepEqual(Widgets.monitorAsPart({ left: [mon], center: [], right: [{ ...mon, id: 'wn' }, today] }), { left: ['monitor'], center: [], right: [today] })
  // In a group: it stays there, the part goes.
  const group = { id: 'wg', type: 'carousel', items: [{ type: 'widget', of: 'monitor' }, { type: 'widget', of: 'today' }] }
  assert.deepEqual(Widgets.monitorAsPart({ left: ['her'], center: [group], right: ['monitor', 'clock'] }), { left: ['her'], center: [group], right: ['clock'] })
  // Neither: as it was (the same layout).
  const plain = { left: ['her'], center: [today], right: ['monitor'] }
  assert.equal(Widgets.monitorAsPart(plain), plain)
})

test('the top bar (split) and the strip along an edge share the layout kept: each made from the other, and back', () => {
  const DOCK = ['start', 'apps']
  const RIGHT = ['monitor', 'tray', 'keys', 'quick', 'clock']
  const SPLIT_PARTS = ['her', 'monitor', 'tray', 'keys', 'quick', 'clock', 'front']
  const SPLIT = { left: ['front'], center: ['her'], right: RIGHT }
  const today = { id: 'wt', type: 'widget', of: 'today', size: 'S' }
  const top = layout => Widgets.layoutOf(Widgets.splitOf(layout, DOCK), SPLIT_PARTS, SPLIT, [])
  const edge = layout => Widgets.layoutOf(Widgets.edgeOf(layout), [...PARTS, 'front'], WIN11, ['monitor', 'front'])
  // Windows 11's (the default) for the top bar: her in the middle, the program in front first at the left.
  assert.deepEqual(top(WIN11), SPLIT)
  assert.deepEqual(top(null), SPLIT)
  // Kept there (no Start, no programs: they are in the dock), for the strip
  // along an edge: the program in front is the top bar's, taken out; her
  // at the left's start again; Start and the programs back where Windows
  // has them. Back and forth, the same.
  assert.deepEqual(edge(SPLIT), WIN11)
  assert.deepEqual(top(edge(SPLIT)), SPLIT)
  // The program in front moved on the top bar, a widget put there: as it
  // was left there; for the strip the program in front out all the same,
  // the widget where it was.
  const edited = { left: [today], center: ['her'], right: ['front', ...RIGHT] }
  assert.deepEqual(top(edited), edited)
  assert.deepEqual(edge(edited), { left: ['her', today], center: ['start', 'apps'], right: RIGHT })
  // Her moved on the top bar to the right, with the program in front: she stays.
  assert.deepEqual(edge({ left: [], center: [], right: ['front', 'her', ...RIGHT] }), { left: [], center: ['start', 'apps'], right: ['her', ...RIGHT] })
  // Quick settings and the clock in the dock too: left out for the top bar
  // (the parts in the top bar's list all the same: layoutOf puts them back,
  // island.js leaves them out of it), back for the strip.
  const tight = ['start', 'apps', 'quick', 'clock']
  assert.deepEqual(Widgets.splitOf({ ...WIN11, right: ['monitor', 'tray', 'keys', today, 'quick', 'clock'] }, tight), { left: ['front'], center: ['her'], right: ['monitor', 'tray', 'keys', today] })
  assert.deepEqual(edge({ left: ['front'], center: ['her'], right: ['monitor', 'tray', 'keys', today] }), { ...WIN11, right: ['monitor', 'tray', 'keys', today, 'quick', 'clock'] })
  // One kept for the strip itself (Start in it): as it was, the program in
  // front where it was put there, if it was (an optional part elsewhere).
  const own = { left: ['her', 'front'], center: ['start', 'apps'], right: RIGHT }
  assert.equal(Widgets.edgeOf(own), own)
  assert.deepEqual(edge(own), own)
  // For the top bar: Start and the programs out, her to the middle (at the
  // left's start, nothing else in the middle), the program in front where
  // it was. Kept from there, the strip has her first and no program in
  // front: the top bar's layout says nothing of what the strip had.
  assert.deepEqual(top(own), { left: ['front'], center: ['her'], right: RIGHT })
  assert.deepEqual(edge(top(own)), WIN11)
  // Her first at the left with others after her, nothing in the middle: she
  // moves to the middle for the top bar; back, alone there, to the left's
  // start, before them.
  assert.deepEqual(top({ left: ['her', today], center: ['start', 'apps'], right: RIGHT }), { left: ['front', today], center: ['her'], right: RIGHT })
  assert.deepEqual(edge({ left: ['front', today], center: ['her'], right: RIGHT }), { left: ['her', today], center: ['start', 'apps'], right: RIGHT })
  // Nothing kept, or none of the runs: as it was (the defaults, layoutOf).
  assert.equal(Widgets.edgeOf(null), null)
  const bare = {}
  assert.equal(Widgets.edgeOf(bare), bare)
  assert.deepEqual(edge(bare), WIN11)
})

test("one without an icon: its name's first character on its colour, or grey, wherever its icon would be", () => {
  // Its colour (a tint under it, the letter in it; one step deeper on a light ground).
  assert.equal(Widgets.markHTML('部署', '#36cfc9'), `<span class="wmark" style="--mark:#36cfc9;--mark-deep:${Widgets.deepen('#36cfc9')}" aria-hidden="true">部</span>`)
  assert.notEqual(Widgets.deepen('#36cfc9'), '#36cfc9')
  // None: grey (.none, no colour of its own).
  assert.equal(Widgets.markHTML('build', null, 'gl'), '<span class="wmark none gl" aria-hidden="true">B</span>')
  // Not a colour: as none; nothing to go by: a middle dot; markup kept out.
  assert.match(Widgets.markHTML('x', 'red; background:url(x)'), /class="wmark none"/)
  assert.match(Widgets.markHTML('', ''), />·</)
  assert.match(Widgets.markHTML('<b>', ''), />&lt;</)
  // A script's widget that sent none ('dot'): its mark; one with an icon: the icon.
  assert.match(Widgets.iconHTML({ id: 'deploy', icon: 'dot', label: 'deploy', color: '#ff7a45' }), /wmark.*--mark:#ff7a45;--mark-deep:#[0-9a-f]{6}.*>D</)
  assert.match(Widgets.iconHTML({ id: 'mail', icon: 'mail' }), /wicon/)
  // A label of words to render: in her language.
  assert.match(Widgets.iconHTML({ id: 'x', icon: 'dot', label: { key: 'widget.today' } }, 'zh'), /wmark/)
  // The cards' glyph the same (the desktop, the strip, the capsule's rows, the menus).
  const WCards = require('../src/pet/wcards')
  assert.match(WCards.glyphHTML({ id: 'deploy', from: 'script', icon: 'dot', label: '部署', color: '#36cfc9' }, 'gl'), /wmark gl.*>部</)
  assert.match(WCards.memberGlyph({ type: 'widget', of: 'build' }), />B</)
  assert.match(WCards.memberGlyph({ type: 'widget', of: 'x', name: '部署' }), />部</)
})

test('a preset takes the parts, and the widgets stay in their runs', () => {
  const weather = { id: 'wa', type: 'plugin', of: 'weather' }
  const sep = { id: 'wb', type: 'sep' }
  const old = { left: ['her', weather], center: ['start', 'apps'], right: [sep, 'monitor', 'tray', 'keys', 'quick', 'clock'] }
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

// --- Sizes and details (widgets.rs detail_of; the widget design's §1) ------------------

test("a widget's detail: a string is its extra, an object as it came; its sizes as it says, else by what it is", () => {
  assert.deepEqual(Widgets.detailOf({ detail: '14°~22°' }), { extra: '14°~22°' })
  assert.deepEqual(Widgets.detailOf({ detail: '' }), {})
  assert.deepEqual(Widgets.detailOf({ detail: null }), {})
  assert.deepEqual(Widgets.detailOf({}), {})
  assert.deepEqual(Widgets.detailOf({ detail: ['s'] }), {})
  const weather = { id: 'weather', detail: { sizes: ['s', 'm', 'l'], extra: '8°~16°', series: { points: [8, 11] } } }
  assert.equal(Widgets.detailOf(weather).series.points.length, 2)
  assert.equal(Widgets.words('zh', { label: 'Dresden · 阴', value: '11°', ...weather }).detail, '8°~16°')
  assert.deepEqual(Widgets.sizesOf(weather), ['s', 'm', 'l'])
  // In order, whatever order it came in.
  assert.deepEqual(Widgets.sizesOf({ detail: { sizes: ['l', 's'] } }), ['s', 'l'])
  // A countdown: small alone (its medium greyed on the taskbar too).
  assert.deepEqual(Widgets.sizesOf({ id: 'countdown', detail: { sizes: ['s'] } }), ['s'])
  // Said nothing: a script's small and medium (its value over its label);
  // the built-in ones' own.
  assert.deepEqual(Widgets.sizesOf({ id: 'ci-main', label: 'main', value: '✓' }), ['s', 'm'])
  assert.deepEqual(Widgets.sizesOf({ id: 'monitor' }), ['s', 'm', 'l'])
  assert.deepEqual(Widgets.sizesOf({ id: 'today' }), ['s', 'm', 'l'])
  assert.deepEqual(Widgets.sizesOf({ id: 'tokens' }), ['s', 'm'])
})

// --- Several in one place (the design's §5) ----------------------------------------------

const today = { type: 'widget', of: 'today', icon: 'today' }
const stock = { type: 'plugin', of: 'stock' }
const mail = { type: 'widget', of: 'inbox-a1', name: 'me@example.com', icon: 'mail' }

test('several in one place are checked as settings.rs checks them', () => {
  assert.ok(Widgets.isEntry({ id: 'wc1', type: 'carousel', items: [today, stock] }))
  assert.ok(Widgets.isEntry({ id: 'wc1', type: 'stack', items: [today, stock, mail] }))
  assert.ok(!Widgets.isEntry({ id: 'wc1', type: 'stack', items: [today] }), 'one alone')
  assert.ok(!Widgets.isEntry({ id: 'wc1', type: 'stack', items: Array.from({ length: 9 }, (_, i) => ({ type: 'widget', of: `w${i}` })) }), 'nine')
  assert.ok(Widgets.isEntry({ id: 'wc1', type: 'stack', items: Array.from({ length: 8 }, (_, i) => ({ type: 'widget', of: `w${i}` })) }), 'eight')
  assert.ok(!Widgets.isEntry({ id: 'wc1', type: 'stack', items: [today, { ...today }] }), 'the same twice')
  assert.ok(!Widgets.isEntry({ id: 'wc1', type: 'stack', items: [today, { type: 'sep' }] }), 'a spacer in it')
  assert.ok(!Widgets.isEntry({ id: 'wc1', type: 'stack', items: [today, { type: 'widget', of: 'tokens', size: 'M' }] }), 'a size of its own')
  assert.ok(!Widgets.isEntry({ id: 'wc1', type: 'stack', items: [today, { id: 'wd', type: 'widget', of: 'tokens' }] }), 'an id of its own')
  assert.ok(!Widgets.isEntry({ id: 'wc1', type: 'stack' }), 'no items')
  assert.ok(!Widgets.isEntry({ id: 'wc1', type: 'carousel', size: 'M', items: [today, stock] }), 'a size')
  assert.ok(!Widgets.isEntry({ id: 'wc1', type: 'stack', of: 'today', items: [today, stock] }), 'an of')
  assert.ok(!Widgets.isEntry({ id: 'wa1', type: 'widget', of: 'today', items: [today, stock] }), 'items on one alone')
  // Kept as a copy, its members too.
  const group = { id: 'wc1', type: 'stack', items: [today, stock] }
  const got = Widgets.layoutOf({ right: [group, 'clock'] }, PARTS, WIN11)
  got.right[0].items[0].of = 'tokens'
  assert.equal(group.items[0].of, 'today')
})

test('two put in one place, made the other kind, taken apart, one taken out, one moved', () => {
  let n = 0
  const newId = () => `wn${++n}`
  const a = { id: 'wa', type: 'widget', of: 'today', size: 'M', icon: 'today' }
  const b = { id: 'wb', type: 'plugin', of: 'stock', size: 'S' }
  const c = { id: 'wcc', type: 'widget', of: 'inbox-a1', name: 'me@example.com' }
  const layout = { left: ['her', c], center: ['start', 'apps'], right: [a, 'tray', b, 'clock'] }
  // b dropped on a: a stack where a was, a first; b gone from its place.
  const one = Widgets.groupWith(layout, 'wb', 'wa', 'stack', newId)
  assert.deepEqual(one.right, [{ id: 'wn1', type: 'stack', items: [{ type: 'widget', of: 'today', icon: 'today' }, { type: 'plugin', of: 'stock' }] }, 'tray', 'clock'])
  assert.deepEqual(layout.right, [a, 'tray', b, 'clock'], 'the one given left as it was')
  assert.ok(Widgets.isEntry(one.right[0]))
  // c into the stack: appended, its kind kept.
  const two = Widgets.groupWith(one, 'wcc', 'wn1', 'carousel', newId)
  assert.equal(two.right[0].type, 'stack')
  assert.deepEqual(two.right[0].items.map(m => m.of), ['today', 'stock', 'inbox-a1'])
  assert.deepEqual(two.left, ['her'])
  // Itself, or a spacer: nothing done.
  assert.equal(Widgets.groupWith(two, 'wn1', 'wn1', 'stack', newId), two)
  assert.equal(Widgets.groupWith({ right: [a, { id: 'wf', type: 'flex' }] }, 'wf', 'wa', 'stack', newId).right.length, 2)
  // The same one is not put in twice.
  const twice = Widgets.groupWith({ right: [{ id: 'wg', type: 'stack', items: [today, stock] }, { id: 'wt', type: 'widget', of: 'today' }] }, 'wt', 'wg', 'stack', newId)
  assert.deepEqual(twice.right[0].items.map(m => m.of), ['today', 'stock'])
  // The other kind.
  assert.equal(Widgets.regroup(two, 'wn1', 'carousel').right[0].type, 'carousel')
  assert.equal(Widgets.regroup(two, 'wn1', 'nope'), two)
  // Moved: the first last.
  assert.deepEqual(Widgets.reorder(two, 'wn1', 0, 2).right[0].items.map(m => m.of), ['stock', 'inbox-a1', 'today'])
  assert.equal(Widgets.reorder(two, 'wn1', 0, 9), two)
  // One taken out: just after it, alone and medium.
  const out = Widgets.takeOut(two, 'wn1', 1, newId)
  assert.deepEqual(out.right.slice(0, 2).map(p => p.type), ['stack', 'plugin'])
  assert.deepEqual(out.right[1], { id: out.right[1].id, type: 'plugin', of: 'stock', size: 'M' })
  // Two in it, one taken out: both alone, in their order.
  const pair = Widgets.takeOut(one, 'wn1', 0, newId)
  assert.deepEqual(pair.right.slice(0, 2).map(p => p.of), ['stock', 'today'])
  assert.ok(pair.right.slice(0, 2).every(p => Widgets.isEntry(p)))
  // One gone: the rest stay; one left, alone.
  assert.deepEqual(Widgets.dropMember(two, 'wn1', 0, newId).right[0].items.map(m => m.of), ['stock', 'inbox-a1'])
  assert.deepEqual(Widgets.dropMember(one, 'wn1', 0, () => 'wz').right[0], { id: 'wz', type: 'plugin', of: 'stock', size: 'M' })
  // Apart: each alone where it was, in its order, the names and icons kept.
  const apart = Widgets.ungroup(two, 'wn1', newId)
  assert.deepEqual(
    apart.right.slice(0, 3).map(p => [p.type, p.of, p.size, p.name || p.icon || '']),
    [
      ['widget', 'today', 'M', 'today'],
      ['plugin', 'stock', 'M', ''],
      ['widget', 'inbox-a1', 'M', 'me@example.com'],
    ],
  )
  assert.ok(apart.right.every(p => typeof p === 'string' || Widgets.isEntry(p)))
})

test("what several in one place show: each member's widgets as slides; one with nothing a slide still", () => {
  const all = [
    { id: 'today', builtIn: true, label: { key: 'widget.today' }, value: '3' },
    { id: 'stock-aapl', plugin: 'stock', label: 'AAPL', value: '231' },
    { id: 'stock-msft', plugin: 'stock', label: 'MSFT', value: '420' },
  ]
  const group = { id: 'wc', type: 'carousel', items: [stock, today, mail] }
  assert.deepEqual(Widgets.sourcesOf(group, all).map(w => w.id), ['stock-aapl', 'stock-msft', 'today'])
  assert.deepEqual(
    Widgets.slidesOf(group, all).map(s => [s.member.of, s.w?.id || null]),
    [
      ['stock', 'stock-aapl'],
      ['stock', 'stock-msft'],
      ['today', 'today'],
      ['inbox-a1', null],
    ],
  )
  assert.deepEqual([...Widgets.shownOn({ right: [group] }, all)].sort(), ['stock-aapl', 'stock-msft', 'today'])
  // Its name: its kind and how many.
  assert.equal(Widgets.nameOf('zh', group, all), '轮播 · 3 个')
  assert.equal(Widgets.nameOf('en', { ...group, type: 'stack' }, all), 'Stack · 3')
  assert.deepEqual(Widgets.membersOf({ id: 'wa', type: 'widget', of: 'today', size: 'S', icon: 'today' }), [{ type: 'widget', of: 'today', icon: 'today' }])
  assert.deepEqual(Widgets.membersOf({ id: 'wf', type: 'flex' }), [])
})

// --- A carousel's turns (the design's §5, §9) ---------------------------------------------

test('a carousel turns every 6 s, a slide with news cuts in for a turn, the wheel turns it by hand', () => {
  const T = Widgets.CAROUSEL_MS
  assert.equal(T, 6000)
  let s = Widgets.carouselStart(1000)
  assert.deepEqual(s, { at: 0, since: 1000, back: -1 })
  // Not yet; then the next; round again past the last.
  assert.equal(Widgets.carouselTurn(s, 3, 1000 + T - 1).at, 0)
  assert.equal(Widgets.carouselDue(s, 1000 + T - 1), 1)
  s = Widgets.carouselTurn(s, 3, 1000 + T)
  assert.deepEqual(s, { at: 1, since: 1000 + T, back: -1 })
  assert.equal(Widgets.carouselDue(s, 1000 + T), T)
  s = Widgets.carouselTurn(Widgets.carouselTurn(s, 3, 1000 + 2 * T), 3, 1000 + 3 * T)
  assert.equal(s.at, 0)
  // News on the third while the first shows: at once, for a whole turn,
  // then back to the one that would have come next (the second).
  s = Widgets.carouselTurn(s, 3, 1000 + 3 * T + 500, 2)
  assert.deepEqual(s, { at: 2, since: 1000 + 3 * T + 500, back: 1 })
  assert.equal(Widgets.carouselTurn(s, 3, 1000 + 4 * T).at, 2)
  s = Widgets.carouselTurn(s, 3, 1000 + 4 * T + 500)
  assert.deepEqual(s, { at: 1, since: 1000 + 4 * T + 500, back: -1 })
  // News on the one shown: nothing to cut in.
  assert.equal(Widgets.carouselTurn(s, 3, 1000 + 4 * T + 600, 1).since, s.since)
  // By hand: either way, round; its turn anew; any news forgotten.
  assert.deepEqual(Widgets.carouselStep({ at: 0, since: 0, back: 2 }, 3, -1, 50), { at: 2, since: 50, back: -1 })
  assert.deepEqual(Widgets.carouselStep({ at: 2, since: 0, back: -1 }, 3, 1, 50), { at: 0, since: 50, back: -1 })
  // One slide (or a member gone): it stays.
  assert.equal(Widgets.carouselTurn({ at: 2, since: 0, back: -1 }, 1, 99999).at, 0)
  assert.equal(Widgets.carouselTurn({ at: 4, since: 0, back: -1 }, 3, 1).at, 2)
})

test('news on a slide: a mailbox whose unread grew; a reading or a clock changing is none', () => {
  const box = n => ({ id: 'inbox-a', from: 'mail', value: { key: 'mail.unread', vars: { n: String(n) } } })
  assert.equal(Widgets.newsIn(box(2), box(3)), true)
  assert.equal(Widgets.newsIn(box(3), box(3)), false)
  assert.equal(Widgets.newsIn(box(3), box(1)), false)
  assert.equal(Widgets.newsIn(undefined, box(3)), false)
  assert.equal(Widgets.newsIn({ id: 'pomodoro', value: '18:42' }, { id: 'pomodoro', value: '18:32' }), false)
  assert.equal(Widgets.newsIn({ id: 'monitor', value: { parts: [] } }, { id: 'monitor', value: { parts: [{ icon: 'cpu', text: '9%' }] } }), false)
})
