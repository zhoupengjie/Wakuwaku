const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')
const { test } = require('node:test')

const { STRINGS, t, render, detectLang } = require('../src/shared/i18n')

test('Chinese and English have the same keys, and the same {placeholders}', () => {
  const zh = Object.keys(STRINGS.zh).sort()
  const en = Object.keys(STRINGS.en).sort()
  assert.deepEqual(zh, en)
  const holes = s => (s.match(/\{\w+\}/g) || []).sort().join(',')
  for (const key of zh) assert.equal(holes(STRINGS.zh[key]), holes(STRINGS.en[key]), key)
})

test('every key the code uses is there', () => {
  // Keys named in the source as 'xxx.yyy' after t( / T( / { key: , or built
  // from a mood / menu / where name.
  const root = path.join(__dirname, '..', 'src')
  const files = []
  const walk = dir => {
    for (const d of fs.readdirSync(dir, { withFileTypes: true })) {
      const p = path.join(dir, d.name)
      if (d.isDirectory()) walk(p)
      else if (p.endsWith('.js') && !p.endsWith('i18n.js')) files.push(p)
    }
  }
  walk(root)
  const used = new Set()
  for (const file of files) {
    const text = fs.readFileSync(file, 'utf8')
    for (const m of text.matchAll(/(?:\bt\(\s*\w+,\s*|\bT\(\s*|key:\s*)'([a-z]+\.[A-Za-z]+)'/g)) used.add(m[1])
  }
  for (const mood of ['idle', 'working', 'waiting', 'done', 'review', 'error']) used.add(`mood.${mood}`)
  for (const mood of ['waiting', 'done', 'review', 'error']) used.add(`notify.${mood}`)
  for (const size of ['small', 'medium', 'large']) used.add(`menu.${size}`)
  for (const where of ['session', 'localSettings', 'projectSettings', 'userSettings']) used.add(`where.${where}`)

  assert.ok(used.size > 60, `found only ${used.size} keys: the scan is broken`)
  for (const key of used) assert.ok(key in STRINGS.zh, `missing: ${key}`)
})

test('t fills placeholders and falls back to English, then the key', () => {
  assert.equal(t('zh', 'panel.needsApproval', { tool: 'Bash' }), 'Bash 需要你批准')
  assert.equal(t('en', 'panel.needsApproval', { tool: 'Bash' }), 'Bash needs your approval')
  assert.equal(t('fr', 'panel.allow'), 'Allow')
  assert.equal(t('zh', 'no.such.key'), 'no.such.key')
  assert.equal(t('en', 'panel.more'), '{n} more')
})

test('render takes plain text or a key', () => {
  assert.equal(render('zh', 'Bash'), 'Bash')
  assert.equal(render('en', { key: 'say.taskDone', vars: { task: 'tests' } }), 'Done: tests')
  assert.equal(render('en', ''), '')
  assert.equal(render('en', null), '')
})

test('any Chinese locale is Chinese; anything else English', () => {
  for (const l of ['zh', 'zh-CN', 'zh-TW', 'zh_CN.UTF-8', 'zh-Hans-CN']) assert.equal(detectLang(l), 'zh', l)
  for (const l of ['en', 'en-US', 'ja-JP', 'C.UTF-8', '', undefined]) assert.equal(detectLang(l), 'en', String(l))
})
