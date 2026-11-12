// Her pages as WebView2 runs them: src/ written into dist/, each .ts with its
// types taken out (Node's own type stripping: the types become spaces, so
// every line and column stays where it was in the .ts, and an error's place
// in WebView2's console is the source's), its imports of ./x.ts made ./x.js;
// every other file (the pages, their styles, pictures) copied as it is.
// dist/ is what tauri.conf.json's frontendDist points at, what the exe
// carries and what the preview shows.
//
//   node scripts/build-pages.mjs [--src <folder>] [--out <folder>]
//
// src/ and dist/ beside this folder unless given. Run by src-tauri/build.rs
// before every build, so a cargo build always carries the pages as they are
// (Node 22.13 or later, for module.stripTypeScriptTypes; no npm packages).
// Types are not checked here: that is `npm run typecheck` (tsc), and the
// pages' tests. A file is written only when it differs from what is there,
// and what is in dist/ but no longer in src/ is taken away. Each relative
// import is checked to name a file that is there: one that does not stops
// the build, as WebView2 would stop the page.
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import * as nodeModule from 'node:module'

const ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), '..')
const args = process.argv.slice(2)
const arg = (name, fallback) => {
  const i = args.indexOf(name)
  return i >= 0 && args[i + 1] ? path.resolve(args[i + 1]) : fallback
}
const SRC = arg('--src', path.join(ROOT, 'src'))
const OUT = arg('--out', path.join(ROOT, 'dist'))
// Not for the pages: what tells Node and tsc these are modules, and the
// types alone (nothing to run).
const SKIP = new Set(['package.json', 'tsconfig.json'])
const skipped = name => SKIP.has(name) || name.endsWith('.d.ts')

const strip = nodeModule.stripTypeScriptTypes
if (typeof strip !== 'function') {
  console.error(`build-pages: Node ${process.versions.node} cannot take types out (module.stripTypeScriptTypes, Node 22.13 or later)`)
  process.exit(1)
}
// Node says each time that this is still experimental; once is enough here.
process.removeAllListeners('warning')

// Relative imports and exports: from './x.ts', import('./x.ts'), import './x.ts'.
const SPECIFIER = /(\bfrom\s*|\bimport\s*\(\s*|\bimport\s+)(['"])(\.{1,2}\/[^'"\n]+?)\2/g

function filesIn(dir, base = dir, out = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name)
    if (e.isDirectory()) filesIn(p, base, out)
    else out.push(path.relative(base, p))
  }
  return out
}

function writeIfChanged(file, data) {
  try {
    if (Buffer.compare(fs.readFileSync(file), Buffer.from(data)) === 0) return false
  } catch {}
  fs.mkdirSync(path.dirname(file), { recursive: true })
  // Whole or not at all: a build reading it meanwhile never sees half.
  const temp = `${file}.${process.pid}.tmp`
  fs.writeFileSync(temp, data)
  fs.renameSync(temp, file)
  return true
}

const problems = []
const written = new Set()
let changed = 0
for (const rel of filesIn(SRC)) {
  const name = path.basename(rel)
  if (skipped(name)) continue
  const from = path.join(SRC, rel)
  if (rel.endsWith('.ts')) {
    const outRel = rel.slice(0, -3) + '.js'
    let code
    try {
      code = strip(fs.readFileSync(from, 'utf8'), { mode: 'strip' })
    } catch (err) {
      problems.push(`${rel}: ${err.message}`)
      continue
    }
    code = code.replace(SPECIFIER, (all, lead, q, spec) => {
      const target = spec.endsWith('.ts') ? spec.slice(0, -3) + '.js' : spec
      const there = path.join(SRC, path.dirname(rel), spec)
      if (!fs.existsSync(there)) problems.push(`${rel}: imports ${spec}, which is not there`)
      else if (!spec.endsWith('.ts') && /\.js$/.test(spec)) problems.push(`${rel}: imports ${spec}: name the .ts`)
      return `${lead}${q}${target}${q}`
    })
    written.add(outRel)
    if (writeIfChanged(path.join(OUT, outRel), code)) changed++
  } else {
    written.add(rel)
    if (writeIfChanged(path.join(OUT, rel), fs.readFileSync(from))) changed++
  }
}

// What src/ no longer has.
let removed = 0
if (fs.existsSync(OUT)) {
  for (const rel of filesIn(OUT)) {
    if (written.has(rel)) continue
    fs.rmSync(path.join(OUT, rel))
    removed++
  }
}

if (problems.length) {
  console.error(`build-pages: ${problems.length} problem(s)\n  ${problems.join('\n  ')}`)
  process.exit(1)
}
console.log(`build-pages: ${written.size} files into ${path.relative(ROOT, OUT) || OUT} (${changed} written, ${removed} removed)`)
