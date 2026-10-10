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
  assert.match(Widgets.rowsHTML({ value: { parts: [{ icon: 'cpu', text: '95%' }, { icon: 'down', text: '1.2M' }] } }), /^<span class="wrow"><span class="wpart high">.*95%.*<\/span><span class="wrow"><span class="wpart">.*1\.2M/)
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
