// The pages' Config (src/shared/config.ts) and the keys the Rust side
// declares (src-tauri/src/config.rs): every key declared there is in it.
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { test } from 'node:test'

const read = (...p: string[]) => fs.readFileSync(path.join(import.meta.dirname, '..', ...p), 'utf8').replace(/\r\n/g, '\n')

test('every key config.rs declares is in the pages’ Config', () => {
  const rs = read('src-tauri', 'src', 'config.rs')
  const block = rs.slice(rs.indexOf('keys! {\n'), rs.indexOf('\n}\n', rs.indexOf('keys! {\n')))
  const declared = [...block.matchAll(/^\s*(?:\w+ \w+: \w+ = )?"(\w+)"/gm)].map(m => m[1])
  assert.ok(declared.length > 50, 'config.rs read')
  const ts = read('src', 'shared', 'config.ts')
  const iface = ts.slice(ts.indexOf('export interface Config {'))
  const ours = new Set([...iface.matchAll(/^ {2}(\w+)\?:/gm)].map(m => m[1]))
  assert.deepEqual(declared.filter(k => !ours.has(k)), [])
  // A switch there (Flag, On, YesNo) is boolean here.
  for (const [, key] of block.matchAll(/^\s*\w+ \w+: (?:Flag|On|YesNo) = "(\w+)"/gm)) assert.match(iface, new RegExp(`^ {2}${key}[?]: boolean$`, 'm'), key)
})
