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
