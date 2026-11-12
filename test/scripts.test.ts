
// The pages' scripts: no function declared twice in one scope. The later one
// quietly takes the name, and every call reaches it: the taskbar's
// placeHer() (her island over its slot) took the name of placeHer({ height })
// (her shape in the island), which was never called again, and she was an
// empty ring on the taskbar (2026-10-12). Neither node --check nor the
// browser says a word.
//
// By the lines, as the scripts are laid out: a declaration's scope is the
// run of lines indented at least as deep as it; a line indented less (the
// scope's closing brace, an else) ends it. Text in a template laid out
// further left only ends a run early (a twin missed, never one made up).
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { test } from 'node:test'

function twins(source) {
  const found = []
  const open = [] // { indent, names: Map(name -> line) }, innermost last
  source.split(/\r?\n/).forEach((line, i) => {
    if (!line.trim()) return
    const indent = line.length - line.trimStart().length
    while (open.length && open[open.length - 1].indent > indent) open.pop()
    const m = /^\s*(?:async\s+)?function\s*\*?\s*([\w$]+)\s*\(/.exec(line)
    if (!m) return
    let scope = open[open.length - 1]
    if (!scope || scope.indent !== indent) open.push((scope = { indent, names: new Map() }))
    const first = scope.names.get(m[1])
    if (first) found.push(`${m[1]}: lines ${first} and ${i + 1}`)
    else scope.names.set(m[1], i + 1)
  })
  return found
}

const scripts = ['src/pet', 'src/shared'].flatMap(dir =>
  fs
    .readdirSync(path.join(import.meta.dirname, '..', dir))
    .filter(f => f.endsWith('.ts') && !f.endsWith('.d.ts'))
    .map(f => path.join(dir, f)),
)

test('a function declared twice in one scope is found', () => {
  const two = ['(() => {', '  function a({ x }) {', '    return x', '  }', '  function a() {}', '})()'].join('\n')
  assert.deepEqual(twins(two), ['a: lines 2 and 5'])
  const apart = ['function f() {', '  function a() {}', '}', 'function g() {', '  function a() {}', '}'].join('\n')
  assert.deepEqual(twins(apart), [])
})

test('no page script declares a function twice in one scope', () => {
  assert.ok(scripts.length > 5, 'the page scripts found')
  for (const file of scripts) {
    assert.deepEqual(twins(fs.readFileSync(path.join(import.meta.dirname, '..', file), 'utf8')), [], file)
  }
})
