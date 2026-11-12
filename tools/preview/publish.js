// The playground as it is published (a page with its files beside it), served
// the same way here to try it, and served live from the repo while working:
//
//   node tools/preview/publish.js dev [port]                   live: the repo's files, reloaded as they change
//   node tools/preview/publish.js build <folder> [--label <text>]  the files, files.json, the size
//   node tools/preview/publish.js serve [port] [folder]        http://127.0.0.1:<port>/ (the build)
//
// build copies into <folder> what the playground needs, under the paths it
// is published at: playground.html (the page: its content alone, the host
// wraps it), preview/ (the stand-in and the playground's own scripts),
// pet/ and shared/ (her pages and every script and style they load, read
// from their own HTML and what each script imports; as scripts/build-pages.mjs
// writes them into dist/, which build runs first), pets/claude-chan/ (her sprites), assets/ (the
// wallpaper), addon/sdk.js and plugins/<id>/ (her SDK and the example
// plugins with a view of their own, for the cards that draw them). Her two
// pages get a viewport meta (published HTML starts with its own); nothing
// else is changed. The page's header says which code it
// is (gitinfo.js: 「分支 simulator · 3ec87d9」, or --label's words). files.json
// maps each published path but the page to its copy; the total size is printed.
//
// serve builds into a temporary folder (or serves <folder>) and answers only
// what files.json has (so a file left out shows here too), the page wrapped
// in a document as the host would.
//
// dev answers from the repo's own files under the same paths (her pages from
// dist/, built again as src/ changes), and tells the open page what changed (server-sent events, /__live): a
// stylesheet of her pages (swapped in place), a script or page of theirs
// (their frames reloaded), the playground's own (the page reloaded, its
// world carried over). A file it served that a build would leave out is
// said in the console.
const fs = require('node:fs')
const path = require('node:path')
const http = require('node:http')
const { execFileSync } = require('node:child_process')
const { stripTypeScriptTypes } = require('node:module')
const os = require('node:os')
const gitinfo = require('./gitinfo')

const HERE = __dirname
const REPO = path.join(HERE, '..', '..')
// Her pages' sources (their HTML read here), and as they run (what is served).
const PET = path.join(REPO, 'src', 'pet')
const DIST = path.join(REPO, 'dist')
const BUILD_PAGES = path.join(REPO, 'scripts', 'build-pages.mjs')
const buildPages = () => execFileSync(process.execPath, [BUILD_PAGES], { stdio: 'inherit' })
const PREVIEW = ['page.html', 'boot.js', 'fake-tauri.js', 'fixtures.js', 'scenes.js', 'playground.js']
const PAGES = ['index.html', 'desk.html']
// The role each page plays in the playground (its frame).
const ROLE_OF = { 'index.html': 'island', 'desk.html': 'desk' }
// What the frames load of the playground's own (a change reloads them), and
// what the page itself does (a change reloads it).
const FRAME_FILES = ['page.html', 'boot.js']
const PAGE_FILES = ['playground.html', 'playground.js', 'fake-tauri.js', 'fixtures.js', 'scenes.js']
// The folders the code it shows lives in (what a build's label looks at).
const SOURCES = ['src/pet', 'src/shared', 'src/addon', 'examples/plugins', 'pets/claude-chan', 'tools/preview']

// Her pages' scripts and styles, as their HTML loads them, and every script
// those import (import, import()), each by its published name (x.js) relative
// to src/pet; what a script imports is read from its source (x.ts), its
// types taken out first (an import type brings nothing at run time).
const IMPORTS = /(\bfrom\s*|\bimport\s*\(\s*|\bimport\s+)(['"])(\.{1,2}\/[^'"\n]+?)\2/g
// (Node says each time that its stripping is experimental: not here.)
function typesOut(code) {
  const said = process.emitWarning
  process.emitWarning = () => {}
  try {
    return stripTypeScriptTypes(code, { mode: 'strip' })
  } finally {
    process.emitWarning = said
  }
}
function pageFiles(page) {
  const html = fs.readFileSync(path.join(PET, page), 'utf8')
  const refs = [...html.matchAll(/<(?:script|link)\b[^>]*\b(?:src|href)="([^"]+)"/g)].map(m => m[1]).filter(p => !/^[a-z]+:/i.test(p))
  const seen = new Set(refs)
  const queue = refs.filter(r => r.endsWith('.js'))
  while (queue.length) {
    const ref = queue.shift()
    const source = path.resolve(PET, ref).replace(/\.js$/, '.ts')
    if (!fs.existsSync(source)) continue
    for (const m of typesOut(fs.readFileSync(source, 'utf8')).matchAll(IMPORTS)) {
      const next = path.posix.normalize(path.posix.join(path.posix.dirname(ref), m[3].replace(/\.ts$/, '.js')))
      if (seen.has(next)) continue
      seen.add(next)
      queue.push(next)
    }
  }
  return [...seen]
}

// Her page as published: with a viewport meta.
const pageText = page => fs.readFileSync(path.join(PET, page), 'utf8').replace(/(<meta charset="utf-8"\s*\/?>)/i, '$1\n    <meta name="viewport" content="width=device-width, initial-scale=1" />')

const esc = s => String(s).replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c])
// The playground's page with which code it is on its header.
const withLabel = (html, label, live = false) => html.replace('<!--wk-build-->', label ? ` <span class="wk-build${live ? ' wk-live' : ''}">${esc(label)}</span>` : '')

// The example plugins with a view of their own (examples/plugins), each
// file of their folders as published (plugins/<id>/...): the fake's
// addon_view writes their view.html, with her SDK (addon/sdk.js), into a
// card's frame.
const EXAMPLES = path.join(REPO, 'examples', 'plugins')
function viewFiles() {
  const out = []
  for (const id of fs.existsSync(EXAMPLES) ? fs.readdirSync(EXAMPLES) : []) {
    const dir = path.join(EXAMPLES, id)
    let manifest = null
    try {
      manifest = JSON.parse(fs.readFileSync(path.join(dir, 'manifest.json'), 'utf8'))
    } catch {}
    if (!manifest?.view) continue
    for (const f of fs.readdirSync(dir, { recursive: true })) if (fs.statSync(path.join(dir, f)).isFile()) out.push([`plugins/${id}/${String(f).split(path.sep).join('/')}`, path.join(dir, f)])
  }
  return out
}

// Published path -> where it comes from (a file, or text made here).
function plan() {
  const out = new Map()
  out.set('playground.html', { file: path.join(HERE, 'playground.html') })
  for (const f of PREVIEW) out.set(`preview/${f}`, { file: path.join(HERE, f) })
  out.set('assets/wall.jpg', { file: path.join(HERE, 'assets', 'wall.jpg') })
  out.set('pets/claude-chan/spritesheet.webp', { file: path.join(REPO, 'pets', 'claude-chan', 'spritesheet.webp') })
  out.set('addon/sdk.js', { file: path.join(DIST, 'addon', 'sdk.js') })
  for (const [pub, file] of viewFiles()) out.set(pub, { file })
  for (const page of PAGES) {
    out.set(`pet/${page}`, { text: pageText(page) })
    for (const ref of pageFiles(page)) {
      const abs = path.resolve(DIST, 'pet', ref)
      const rel = path.relative(DIST, abs).split(path.sep).join('/')
      out.set(rel, { file: abs })
    }
  }
  return out
}

function build(folder, { label } = {}) {
  const dir = path.resolve(folder)
  const said = label === undefined ? gitinfo.label(REPO, SOURCES) : label
  buildPages()
  fs.mkdirSync(dir, { recursive: true })
  const files = {}
  let total = 0
  for (const [pub, from] of plan()) {
    const to = path.join(dir, ...pub.split('/'))
    fs.mkdirSync(path.dirname(to), { recursive: true })
    if (pub === 'playground.html') fs.writeFileSync(to, withLabel(fs.readFileSync(from.file, 'utf8'), said))
    else if (from.file) fs.copyFileSync(from.file, to)
    else fs.writeFileSync(to, from.text)
    total += fs.statSync(to).size
    if (pub !== 'playground.html') files[pub] = to
  }
  fs.writeFileSync(path.join(dir, 'files.json'), JSON.stringify(files, null, 2))
  return { dir, files, total, label: said }
}

const TYPES = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.css': 'text/css; charset=utf-8', '.json': 'application/json', '.jpg': 'image/jpeg', '.png': 'image/png', '.webp': 'image/webp', '.svg': 'image/svg+xml', '.woff2': 'font/woff2' }

// The page as the host wraps it: its content in a document of its own
// (served live: told so first, window.WAKU_DEV).
const wrap = (content, dev = false) =>
  `<!doctype html>\n<html lang="zh-CN">\n<head>\n<meta charset="utf-8">\n<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">\n${dev ? '<script>window.WAKU_DEV = true</script>\n' : ''}</head>\n<body>\n${content}\n</body>\n</html>\n`

// For trying it at a width the browser's window cannot be (headless Edge's
// is 500 px at least): the page in a frame that wide (?w=&h=&q=).
function frameAt(url) {
  const w = Number(url.searchParams.get('w')) || 400
  const h = Number(url.searchParams.get('h')) || 860
  const q = url.searchParams.get('q') || ''
  return `<!doctype html><meta charset="utf-8"><style>html,body{margin:0;background:#777}iframe{display:block;border:0}</style><iframe width="${w}" height="${h}" src="/playground.html${q ? `?${q.replace(/"/g, '')}` : ''}"></iframe>`
}

const send = (res, status, type, body) => {
  res.writeHead(status, { 'content-type': type, 'cache-control': 'no-store' })
  res.end(body)
}
const sendFile = (res, file) => {
  res.writeHead(200, { 'content-type': TYPES[path.extname(file).toLowerCase()] || 'application/octet-stream', 'cache-control': 'no-store' })
  fs.createReadStream(file).pipe(res)
}
const pubOf = req => {
  const url = new URL(req.url, 'http://x')
  return { url, pub: decodeURIComponent(url.pathname).replace(/^\/+/, '') || 'playground.html' }
}
const listen = (server, port, what) => server.listen(port, '127.0.0.1', () => console.log(`${what}: http://127.0.0.1:${port}/`))

function serve(port, folder) {
  const { dir, files } = folder && fs.existsSync(path.join(folder, 'files.json')) ? { dir: path.resolve(folder), files: JSON.parse(fs.readFileSync(path.join(folder, 'files.json'), 'utf8')) } : build(folder || fs.mkdtempSync(path.join(os.tmpdir(), 'wk-playground-')))
  const server = http.createServer((req, res) => {
    const { url, pub } = pubOf(req)
    if (pub === '__frame') return send(res, 200, TYPES['.html'], frameAt(url))
    if (pub === 'playground.html') return send(res, 200, TYPES['.html'], wrap(fs.readFileSync(path.join(dir, 'playground.html'), 'utf8')))
    const file = files[pub]
    if (!file || !fs.existsSync(file)) return send(res, 404, 'text/plain', `not published: ${pub}`)
    sendFile(res, file)
  })
  listen(server, port, `playground (${dir})`)
}

// --- Live, from the repo ----------------------------------------------------------------
// The published paths and the repo's folders they are read from.
const ROOTS = [
  ['preview/', HERE],
  ['pet/', path.join(DIST, 'pet')],
  ['shared/', path.join(DIST, 'shared')],
  ['pets/', path.join(REPO, 'pets')],
  ['assets/', path.join(HERE, 'assets')],
  ['addon/', path.join(DIST, 'addon')],
  ['plugins/', EXAMPLES],
]
// A published path's file in the repo (none outside those folders).
function repoFile(pub) {
  for (const [prefix, dir] of ROOTS) {
    if (!pub.startsWith(prefix)) continue
    const file = path.resolve(dir, pub.slice(prefix.length))
    const rel = path.relative(dir, file)
    return rel && !rel.startsWith('..') && !path.isAbsolute(rel) ? file : null
  }
  return null
}
// A repo path (posix, from its root) as published, or null: a page script's
// source (x.ts) as what it is built into (x.js); a file of types alone, none.
function publishedOf(rel) {
  if (rel.endsWith('.d.ts')) return null
  if (rel.startsWith('src/') && rel.endsWith('.ts')) rel = rel.slice(0, -3) + '.js'
  const map = [
    ['tools/preview/assets/', 'assets/'],
    ['tools/preview/', 'preview/'],
    ['src/pet/', 'pet/'],
    ['src/shared/', 'shared/'],
    ['src/addon/', 'addon/'],
    ['examples/plugins/', 'plugins/'],
    ['pets/', 'pets/'],
  ]
  for (const [from, to] of map) if (rel.startsWith(from)) return to + rel.slice(from.length)
  return null
}

// What each frame's page loads, as published paths ({ island: Set, desk: Set }).
function framePages() {
  const out = {}
  for (const page of PAGES) out[ROLE_OF[page]] = new Set([`pet/${page}`, ...pageFiles(page).map(ref => path.posix.normalize(`pet/${ref}`))])
  return out
}

// What a changed file (repo path, posix) means for the open playground:
// { page: true } (it anew), { frames: [roles] } (those reloaded),
// { css: { role: [published paths] } } (those sheets swapped), or null.
function liveChange(rel, pages = framePages()) {
  const pub = publishedOf(rel)
  if (!pub) return null
  if (pub.startsWith('preview/')) {
    const f = pub.slice('preview/'.length)
    if (PAGE_FILES.includes(f)) return { page: true }
    if (FRAME_FILES.includes(f)) return { frames: Object.keys(pages) }
    return null
  }
  if (pub.startsWith('assets/')) return { page: true }
  // Her sprites, a plugin's view and her SDK (read into a card's frame by the fake): the frames again.
  if (pub.startsWith('pets/') || pub.startsWith('plugins/') || pub.startsWith('addon/')) return { frames: Object.keys(pages) }
  const roles = Object.keys(pages).filter(role => pages[role].has(pub))
  if (!roles.length) return null
  if (pub.endsWith('.css')) return { css: Object.fromEntries(roles.map(role => [role, [pub]])) }
  return { frames: roles }
}

// Several changes at once, as one message.
function mergeChanges(list) {
  const out = { page: false, frames: [], css: {}, files: [] }
  for (const { rel, change } of list) {
    if (!change) continue
    out.files.push(rel)
    if (change.page) out.page = true
    for (const role of change.frames || []) if (!out.frames.includes(role)) out.frames.push(role)
    for (const [role, files] of Object.entries(change.css || {})) out.css[role] = [...new Set([...(out.css[role] || []), ...files])]
  }
  return out.files.length ? out : null
}

function dev(port) {
  buildPages()
  const clients = new Set()
  const inBuild = () => new Set(plan().keys())
  const server = http.createServer((req, res) => {
    const { url, pub } = pubOf(req)
    if (pub === '__live') {
      res.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-store', connection: 'keep-alive' })
      res.write('retry: 1000\n\n')
      clients.add(res)
      req.on('close', () => clients.delete(res))
      return
    }
    if (pub === '__frame') return send(res, 200, TYPES['.html'], frameAt(url))
    if (pub === 'playground.html') {
      const label = gitinfo.label(REPO, SOURCES)
      return send(res, 200, TYPES['.html'], wrap(withLabel(fs.readFileSync(path.join(HERE, 'playground.html'), 'utf8'), `本地实时${label ? ` · ${label}` : ''}`, true), true))
    }
    const file = repoFile(pub)
    if (!file || !fs.existsSync(file) || !fs.statSync(file).isFile()) return send(res, 404, 'text/plain', `no such file: ${pub}`)
    if (!inBuild().has(pub)) console.log(`  served ${pub}, which a build leaves out (publish.js plan)`)
    if (pub.startsWith('pet/') && PAGES.includes(pub.slice(4))) return send(res, 200, TYPES['.html'], pageText(pub.slice(4)))
    sendFile(res, file)
  })

  // The repo's files, watched: a burst of saves told as one.
  let pending = new Map()
  let timer = null
  const told = rel => {
    pending.set(rel, true)
    clearTimeout(timer)
    timer = setTimeout(() => {
      const rels = [...pending.keys()]
      pending = new Map()
      // Her pages built again first: what is served is dist/.
      if (rels.some(rel => rel.startsWith('src/'))) {
        try {
          buildPages()
        } catch {
          return console.log('  her pages did not build: the open page left as it was')
        }
      }
      let pages
      try {
        pages = framePages()
      } catch {
        pages = { island: new Set(), desk: new Set() }
      }
      const msg = mergeChanges(rels.map(rel => ({ rel, change: liveChange(rel, pages) })))
      if (!msg) return
      console.log(`  changed ${msg.files.join(', ')}: ${msg.page ? 'the page' : [msg.frames.length ? `frames ${msg.frames.join(' ')}` : '', Object.keys(msg.css).length ? `styles in ${Object.keys(msg.css).join(' ')}` : ''].filter(Boolean).join(', ')}`)
      const data = `event: change\ndata: ${JSON.stringify(msg)}\n\n`
      for (const res of clients) res.write(data)
    }, 150)
  }
  const WATCHED = ['src/pet', 'src/shared', 'src/addon', 'examples/plugins', 'pets/claude-chan', 'tools/preview']
  for (const dir of WATCHED) {
    try {
      fs.watch(path.join(REPO, dir), { recursive: true }, (_, name) => {
        if (!name) return
        const rel = `${dir}/${String(name).split(path.sep).join('/')}`
        if (/\.(ts|js|css|html|webp|png|jpg|json)$/i.test(rel)) told(rel)
      })
    } catch (err) {
      console.log(`  cannot watch ${dir}: ${err.message}`)
    }
  }
  // A comment now and then keeps the stream open through idle proxies.
  setInterval(() => {
    for (const res of clients) res.write(': still here\n\n')
  }, 20e3).unref()
  listen(server, port, `playground, live from ${REPO}`)
  return server
}

module.exports = { plan, build, pageFiles, withLabel, publishedOf, framePages, liveChange, mergeChanges, repoFile, SOURCES }

if (require.main === module) {
  const args = process.argv.slice(2)
  const flag = name => {
    const i = args.indexOf(name)
    if (i < 0) return undefined
    const v = args[i + 1]
    args.splice(i, 2)
    return v ?? ''
  }
  const label = flag('--label')
  const [cmd, a, b] = args
  if (cmd === 'build' && a) {
    const { dir, files, total, label: said } = build(a, { label })
    console.log(`${Object.keys(files).length + 1} files into ${dir}: ${total} bytes (${(total / 1048576).toFixed(2)} MB)${said ? `, labelled 「${said}」` : ''}`)
  } else if (cmd === 'serve') {
    serve(Number(a) || 8765, b)
  } else if (cmd === 'dev') {
    dev(Number(a) || 8765)
  } else {
    console.log('node tools/preview/publish.js dev [port] | build <folder> [--label <text>] | serve [port] [folder]')
    process.exitCode = 1
  }
}
