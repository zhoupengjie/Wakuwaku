// Before and after: the gallery's shots (scenes.js) of her pages at two
// points of the code, side by side, 改前 on the left and 改后 on the right, in
// Windows' dark and light mode; with an index page of them all.
//
//   node tools/preview/compare.js [--base main] [--head <ref>] [--only scene-*,capsule-*]
//                                 [--modes dark,light] [--out <folder>] [--scale 2] [--keep]
//
// --base   the code before (a branch, tag or commit; main by default)
// --head   the code after (a ref); left out: this checkout's files as they
//          are now, saved or not (the usual: a branch's work against main)
// --only   which shots, as render.ps1 -Only takes them (commas between)
// --out    where the pictures go (by default a folder in the temp folder, printed)
//
// Both sides are drawn by the same harness, this checkout's (or --head's)
// tools/preview: the same scenes, the same stand-in for Tauri; only her
// pages and their scripts and styles (src/pet, src/shared, her sprites; the
// plugins' SDK and the example plugins, whose views a card may draw) are
// each side's own. A ref's are taken with git archive into a temporary
// folder (removed at the end, unless --keep); no checkout is touched.
//
// Out:
//   <id>-<mode>.png   one per shot and mode: 改前 | 改后, what changed boxed
//   index.html        all the pairs by section, those that changed marked
//   before/, after/   each side's own shots (render.ps1)
// Two shots are the same when no pixel differs by more than a little (the
// blur and antialiasing noise of two runs, README).
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const zlib = require('node:zlib')
const { execFileSync, spawnSync } = require('node:child_process')
const gitinfo = require('./gitinfo')

const HERE = __dirname
const REPO = path.join(HERE, '..', '..')
// Her code, as a side has it; and the harness that draws it.
const PRODUCT = ['src/pet', 'src/shared', 'src/addon', 'examples/plugins', 'pets/claude-chan']
const HARNESS = 'tools/preview'
// What builds her pages (src/ into dist/, render.ps1 runs it), this checkout's
// for both sides: an older side's src/ is JavaScript, copied as it is.
const BUILD = 'scripts/build-pages.mjs'
// A channel differing by more than this is a change; fewer pixels than NOISE changed is none.
const TOLERANCE = 24
const NOISE = 8

// --- PNG, read (Edge's screenshots: 8 bits a channel, not interlaced) ---------------------
function readPng(buf) {
  if (buf.readUInt32BE(0) !== 0x89504e47) throw new Error('not a PNG')
  let pos = 8
  let width = 0
  let height = 0
  let depth = 0
  let type = 0
  let interlace = 0
  const idat = []
  while (pos + 8 <= buf.length) {
    const len = buf.readUInt32BE(pos)
    const kind = buf.toString('ascii', pos + 4, pos + 8)
    const data = buf.subarray(pos + 8, pos + 8 + len)
    if (kind === 'IHDR') {
      width = data.readUInt32BE(0)
      height = data.readUInt32BE(4)
      depth = data[8]
      type = data[9]
      interlace = data[12]
    } else if (kind === 'IDAT') idat.push(data)
    else if (kind === 'IEND') break
    pos += 12 + len
  }
  const channels = { 0: 1, 2: 3, 4: 2, 6: 4 }[type]
  if (depth !== 8 || interlace || !channels) throw new Error(`a PNG this reads not (depth ${depth}, type ${type}, interlace ${interlace})`)
  const raw = zlib.inflateSync(Buffer.concat(idat))
  const stride = width * channels
  const px = Buffer.alloc(stride * height)
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)]
    const src = y * (stride + 1) + 1
    const row = y * stride
    const up = row - stride
    for (let x = 0; x < stride; x++) {
      const a = x >= channels ? px[row + x - channels] : 0
      const b = y ? px[up + x] : 0
      const c = y && x >= channels ? px[up + x - channels] : 0
      let v = raw[src + x]
      if (filter === 1) v += a
      else if (filter === 2) v += b
      else if (filter === 3) v += (a + b) >> 1
      else if (filter === 4) {
        const p = a + b - c
        const pa = Math.abs(p - a)
        const pb = Math.abs(p - b)
        const pc = Math.abs(p - c)
        v += pa <= pb && pa <= pc ? a : pb <= pc ? b : c
      }
      px[row + x] = v & 255
    }
  }
  if (channels === 4) return { width, height, data: px }
  const data = Buffer.alloc(width * height * 4)
  for (let i = 0; i < width * height; i++) {
    const s = i * channels
    const [r, g, b2, al] = channels === 3 ? [px[s], px[s + 1], px[s + 2], 255] : channels === 2 ? [px[s], px[s], px[s], px[s + 1]] : [px[s], px[s], px[s], 255]
    data[i * 4] = r
    data[i * 4 + 1] = g
    data[i * 4 + 2] = b2
    data[i * 4 + 3] = al
  }
  return { width, height, data }
}

// How two pictures differ: the pixels that do (by more than tol in a
// channel), and the box around them ([x, y, w, h]), or that their sizes do.
function diffImages(a, b, tol = TOLERANCE) {
  if (a.width !== b.width || a.height !== b.height) return { size: true, changed: a.width * a.height, box: null }
  let changed = 0
  let x0 = Infinity
  let y0 = Infinity
  let x1 = -1
  let y1 = -1
  const w = a.width
  for (let i = 0, n = w * a.height; i < n; i++) {
    const o = i * 4
    if (Math.abs(a.data[o] - b.data[o]) > tol || Math.abs(a.data[o + 1] - b.data[o + 1]) > tol || Math.abs(a.data[o + 2] - b.data[o + 2]) > tol || Math.abs(a.data[o + 3] - b.data[o + 3]) > tol) {
      changed++
      const x = i % w
      const y = (i - x) / w
      if (x < x0) x0 = x
      if (x > x1) x1 = x
      if (y < y0) y0 = y
      if (y > y1) y1 = y
    }
  }
  return { size: false, changed, box: changed ? [x0, y0, x1 - x0 + 1, y1 - y0 + 1] : null }
}

// One pair's verdict, from its two files (either may be missing).
function judge(before, after) {
  if (!before && !after) return { status: 'missing', changed: 0, box: null }
  if (!before) return { status: 'new', changed: 0, box: null }
  if (!after) return { status: 'gone', changed: 0, box: null }
  const A = fs.readFileSync(before)
  const B = fs.readFileSync(after)
  if (A.equals(B)) return { status: 'same', changed: 0, box: null }
  const d = diffImages(readPng(A), readPng(B))
  if (d.size) return { status: 'size', changed: d.changed, box: null }
  return { status: d.changed > NOISE ? 'changed' : 'same', changed: d.changed, box: d.changed > NOISE ? d.box : null }
}

const STATUS = { same: '相同', changed: '有变化', new: '新增', gone: '没了', size: '大小变了', missing: '两边都没拍出来' }

// --- The two sides ------------------------------------------------------------------------
const git = (...args) => execFileSync('git', args, { cwd: REPO, maxBuffer: 1 << 30 })

// A ref's files at these paths, into a folder (git archive; tar reads it).
function archive(ref, paths, into) {
  fs.mkdirSync(into, { recursive: true })
  const there = git('ls-tree', '-d', '--name-only', ref, '--', ...paths).toString().split('\n').filter(Boolean)
  const want = paths.filter(p => there.includes(p) || there.some(t => p.startsWith(`${t}/`)))
  if (!want.length) throw new Error(`${ref} has none of ${paths.join(', ')}`)
  const tar = git('archive', '--format=tar', ref, '--', ...want)
  const r = spawnSync('tar', ['-xf', '-'], { cwd: into, input: tar, maxBuffer: 1 << 30 })
  if (r.status !== 0) throw new Error(`tar: ${r.stderr}`)
}

function copyFile(from, to) {
  fs.mkdirSync(path.dirname(to), { recursive: true })
  fs.copyFileSync(from, to)
}

function copyDir(from, to) {
  fs.mkdirSync(to, { recursive: true })
  for (const e of fs.readdirSync(from, { withFileTypes: true })) {
    if (e.name.startsWith('.')) continue
    const a = path.join(from, e.name)
    const b = path.join(to, e.name)
    if (e.isDirectory()) copyDir(a, b)
    else fs.copyFileSync(a, b)
  }
}

const q = s => `'${String(s).replace(/'/g, "''")}'`
function render(root, out, { only, modes, scale }) {
  const script = path.join(root, HARNESS, 'render.ps1')
  const cmd = `& ${q(script)} -Out ${q(out)} -Only ${only.map(q).join(',')} -Modes ${modes.map(q).join(',')} -Scale ${scale} -NoOverview`
  const r = spawnSync('pwsh', ['-NoProfile', '-NonInteractive', '-Command', cmd], { stdio: 'inherit' })
  if (r.error) throw r.error
  return r.status
}

// The shots --only picks, as render.ps1 -Only does (* and ? as -like has them).
function picks(globs) {
  const res = globs.map(g => new RegExp(`^${g.replace(/[.+^${}()|[\]\\]/g, '\\$&').replace(/\*/g, '.*').replace(/\?/g, '.')}$`, 'i'))
  return id => res.some(r => r.test(id))
}

// --- The pairs, drawn (pairs.ps1) and listed (index.html) ---------------------------------
const esc = s => String(s).replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c])

function indexPage({ list, pairs, modes, before, after }) {
  const bySection = list.sections
    .map(([key, name]) => [name, list.scenes.filter(s => s.section === key)])
    .filter(([, shots]) => shots.length)
  const changedIds = new Set(pairs.filter(p => p.status !== 'same').map(p => p.id))
  const card = s => {
    const mine = pairs.filter(p => p.id === s.id)
    const worst = mine.find(p => p.status !== 'same') || mine[0]
    return `<figure class="pair${changedIds.has(s.id) ? ' diff' : ''}">
  <figcaption><b>${esc(s.title)}</b> <code>${esc(s.id)}</code> <span class="st st-${worst?.status || 'missing'}">${esc(STATUS[worst?.status || 'missing'])}${worst?.status === 'changed' ? ` · ${worst.changed} 像素` : ''}</span></figcaption>
  ${modes.map(m => mine.find(p => p.mode === m)).filter(Boolean).map(p => `<a href="${esc(p.file)}"><img loading="lazy" src="${esc(p.file)}" alt="${esc(`${s.title} ${p.mode}`)}" style="max-width:min(100%, ${Math.round(p.width)}px)"></a>`).join('\n  ')}
</figure>`
  }
  const n = changedIds.size
  return `<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>改前改后对比</title>
<style>
  :root { --bg: #f3f3f3; --surface: #ffffff; --ink: #1c1c20; --soft: #64646c; --line: #d2d2d6; --hit: #c2185b; --ok: #2e7d32; color-scheme: light; font-family: 'Segoe UI Variable Text', 'Segoe UI', 'Microsoft YaHei UI', sans-serif; }
  @media (prefers-color-scheme: dark) { :root:not([data-theme='light']) { --bg: #1c1c1f; --surface: #26262a; --ink: #ebebf0; --soft: #96969e; --line: #3c3c42; --hit: #ff6fae; --ok: #7bd389; color-scheme: dark; } }
  :root[data-theme='dark'] { --bg: #1c1c1f; --surface: #26262a; --ink: #ebebf0; --soft: #96969e; --line: #3c3c42; --hit: #ff6fae; --ok: #7bd389; color-scheme: dark; }
  body { margin: 0; padding: 20px 16px 40px; background: var(--bg); color: var(--ink); }
  main { max-width: 1800px; margin: 0 auto; }
  h1 { margin: 0 0 6px; font-size: 22px; font-weight: 600; }
  .sub { margin: 0 0 14px; color: var(--soft); font-size: 14px; line-height: 1.6; }
  label { font-size: 14px; }
  h2 { margin: 28px 0 10px; font-size: 17px; font-weight: 600; }
  .pair { margin: 0 0 18px; padding: 12px; border-radius: 10px; background: var(--surface); box-shadow: 0 0 0 1px var(--line); }
  .pair figcaption { margin-bottom: 8px; font-size: 14px; }
  .pair code { color: var(--soft); font: 12px Consolas, monospace; }
  .pair img { display: block; height: auto; margin-top: 8px; border-radius: 4px; }
  .st { margin-left: 6px; padding: 1px 8px; border-radius: 999px; font-size: 12px; box-shadow: inset 0 0 0 1px currentColor; }
  .st-same { color: var(--soft); }
  .st-changed, .st-size, .st-gone, .st-new { color: var(--hit); }
  body.only-diff .pair:not(.diff) { display: none; }
  body.only-diff section:not(:has(.diff)) { display: none; }
</style>
</head>
<body>
<main>
<h1>改前改后对比</h1>
<p class="sub">左边改前：${esc(before)}<br>右边改后：${esc(after)}<br>${list.scenes.length} 个场景，${n ? `<b>${n} 个有变化</b>（框出来的是变了的地方）` : '都一样'}。点图看原大。</p>
<label><input type="checkbox" id="only"${n ? ' checked' : ''}> 只看有变化的</label>
${bySection.map(([name, shots]) => `<section><h2>${esc(name)}</h2>\n${shots.map(card).join('\n')}</section>`).join('\n')}
</main>
<script>
  const box = document.getElementById('only')
  const apply = () => document.body.classList.toggle('only-diff', box.checked)
  box.addEventListener('change', apply)
  apply()
</script>
</body>
</html>
`
}

// --- Running ------------------------------------------------------------------------------
function parseArgs(argv) {
  const o = { base: 'main', head: null, only: ['*'], modes: ['dark', 'light'], out: null, scale: 2, keep: false }
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]
    const v = () => argv[++i]
    if (a === '--base') o.base = v()
    else if (a === '--head') o.head = v()
    else if (a === '--only') o.only = v().split(',').map(s => s.trim()).filter(Boolean)
    else if (a === '--modes') o.modes = v().split(',').map(s => s.trim()).filter(m => m === 'dark' || m === 'light')
    else if (a === '--out') o.out = v()
    else if (a === '--scale') o.scale = Number(v()) || 2
    else if (a === '--keep') o.keep = true
    else throw new Error(`unknown: ${a}`)
  }
  return o
}

function main() {
  const o = parseArgs(process.argv.slice(2))
  const base = gitinfo.ref(REPO, o.base)
  const head = o.head ? gitinfo.ref(REPO, o.head) : gitinfo.here(REPO, [...PRODUCT, HARNESS])
  const beforeLabel = `改前 · ${gitinfo.labelOf(base)}`
  const afterLabel = `改后 · ${gitinfo.labelOf(head)}`
  const out = path.resolve(o.out || path.join(os.tmpdir(), 'wakuwaku-compare', `${base.sha}-${o.head ? head.sha : 'here'}`))
  fs.mkdirSync(out, { recursive: true })
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'wk-compare-'))
  console.log(`${beforeLabel}\n${afterLabel}\ninto ${out}`)

  try {
    // After: this checkout, or the ref's code with its own harness.
    let afterRoot = REPO
    if (o.head) {
      afterRoot = path.join(temp, 'after')
      archive(o.head, [...PRODUCT, HARNESS], afterRoot)
      copyFile(path.join(REPO, BUILD), path.join(afterRoot, BUILD))
    }
    // Before: the base's code, drawn by the after side's harness.
    const beforeRoot = path.join(temp, 'before')
    archive(o.base, PRODUCT, beforeRoot)
    copyDir(path.join(afterRoot, HARNESS), path.join(beforeRoot, HARNESS))
    copyFile(path.join(REPO, BUILD), path.join(beforeRoot, BUILD))

    const shots = { before: path.join(out, 'before'), after: path.join(out, 'after') }
    for (const side of ['before', 'after']) {
      fs.rmSync(shots[side], { recursive: true, force: true })
      console.log(`\n--- ${side === 'before' ? beforeLabel : afterLabel}`)
      render(side === 'before' ? beforeRoot : afterRoot, shots[side], o)
    }

    const list = JSON.parse(fs.readFileSync(path.join(shots.after, 'scenes.json'), 'utf8'))
    const picked = picks(o.only)
    list.scenes = list.scenes.filter(s => picked(s.id))
    const pairs = []
    for (const s of list.scenes) {
      for (const mode of o.modes) {
        const at = side => {
          const f = path.join(shots[side], `${s.id}-${mode}.png`)
          return fs.existsSync(f) ? f : null
        }
        const before = at('before')
        const after = at('after')
        const v = judge(before, after)
        // Each side at most 1600 px wide in the pair, else as shot.
        const k = Math.min(1, 1600 / (s.view[2] * o.scale))
        pairs.push({ id: s.id, mode, title: s.title, before, after, file: `${s.id}-${mode}.png`, out: path.join(out, `${s.id}-${mode}.png`), k, density: o.scale * k, ...v })
      }
    }
    const listFile = path.join(temp, 'pairs.json')
    fs.writeFileSync(listFile, JSON.stringify({ before: beforeLabel, after: afterLabel, status: STATUS, items: pairs }))
    const r = spawnSync('pwsh', ['-NoProfile', '-NonInteractive', '-File', path.join(HERE, 'pairs.ps1'), '-List', listFile], { stdio: 'inherit' })
    if (r.status !== 0) throw new Error('pairs.ps1 failed')

    // The index shows each pair at its size on the screen (they are shot at --scale).
    for (const p of pairs) p.width = fs.existsSync(p.out) ? Math.round(fs.readFileSync(p.out).readUInt32BE(16) / o.scale) : 0
    fs.writeFileSync(path.join(out, 'index.html'), indexPage({ list, pairs, modes: o.modes, before: beforeLabel.slice(5), after: afterLabel.slice(5) }))
    const changed = [...new Set(pairs.filter(p => p.status !== 'same').map(p => `${p.id} (${STATUS[p.status]})`))]
    console.log(`\n${list.scenes.length} shots x ${o.modes.length} modes; ${changed.length ? `changed: ${changed.join(', ')}` : 'none changed'}`)
    console.log(`index: ${path.join(out, 'index.html')}`)
  } finally {
    if (!o.keep) fs.rmSync(temp, { recursive: true, force: true })
    else console.log(`kept: ${temp}`)
  }
}

module.exports = { readPng, diffImages, judge, parseArgs, picks, indexPage, TOLERANCE, NOISE }

if (require.main === module) {
  try {
    main()
  } catch (err) {
    console.error(err.message)
    process.exitCode = 1
  }
}
