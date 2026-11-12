// The market's tools (tools/market): a package checked by docs/PLUGINS.md's
// rules from its folder or its zip, programs refused whatever their name,
// names that would land outside it, its limits; the same zip for the same
// files; the index built from the packages; the fake market the app's tests
// read; the mirror to Gitee.
import assert from 'node:assert/strict'
import crypto from 'node:crypto'
import fs from 'node:fs'
import http from 'node:http'
import os from 'node:os'
import path from 'node:path'
import { execFileSync, spawnSync } from 'node:child_process'
import { test, after } from 'node:test'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)

const MARKET = path.join(import.meta.dirname, '..', 'tools', 'market')
const pkg = require(path.join(MARKET, 'package'))
const { build } = require(path.join(MARKET, 'build-index'))
const { check } = require(path.join(MARKET, 'check'))
const fake = require(path.join(MARKET, 'fake-market'))
const { mirror } = require(path.join(MARKET, 'gitee-mirror'))

const made = []
const tmp = () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'wakuwaku-market-test-'))
  made.push(dir)
  return dir
}
after(() => made.forEach(dir => fs.rmSync(dir, { recursive: true, force: true })))
const sha = b => crypto.createHash('sha256').update(b).digest('hex')
const codes = list => list.map(p => p.code)

const MANIFEST = {
  id: 'rain-alert',
  kind: 'plugin',
  name: { en: 'Rain alert', zh: '下雨提醒' },
  description: { en: 'Will it rain in the next hours, and when', zh: '接下来几小时会不会下雨，几点开始' },
  version: '1.2.0',
  author: 'lin',
  homepage: 'https://github.com/lin/rain-alert',
  api: 1,
  main: 'main.js',
  view: 'view.html',
  category: 'info',
  widget: { sizes: ['s', 'm'], color: '#8cc8ff', icon: 'icon.svg' },
  settings: [
    { key: 'City', type: 'text', required: true, label: { en: 'City', zh: '城市' }, hint: { en: 'e.g. Dresden', zh: '比如 Dresden' } },
    { key: 'Hours', type: 'choice', options: ['3', '6', '12'], default: '6', label: { en: 'Look ahead', zh: '往后看' } },
    { key: 'Alert', type: 'flag', default: true, label: { en: 'Tell me before it rains', zh: '下雨前提醒我' } },
  ],
  permissions: { net: ['api.open-meteo.com'], notify: true },
}
const ICON = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M2 8h12"/></svg>'
const PNG = Buffer.concat([Buffer.from('89504e470d0a1a0a0000000d49484452', 'hex'), Buffer.alloc(32)])
const WEBP = Buffer.concat([Buffer.from('RIFF'), Buffer.alloc(4), Buffer.from('WEBPVP8 '), Buffer.alloc(32)])
const PE = (() => {
  const b = Buffer.alloc(256)
  b.write('MZ', 0, 'latin1')
  b.writeUInt32LE(0x80, 0x3c)
  b.write('PE\0\0', 0x80, 'latin1')
  return b
})()
const ELF = Buffer.concat([Buffer.from('7f454c46020101', 'hex'), Buffer.alloc(57)])

// A good plugin's files, with some changed: a value of null leaves one out.
function files(change = {}, manifest = {}) {
  const all = {
    'manifest.json': JSON.stringify({ ...MANIFEST, ...manifest }, null, 2),
    'main.js': "waku.widget.set({ value: '70%' })\n",
    'view.html': '<p>rain</p>\n',
    'icon.svg': ICON,
    ...change,
  }
  return Object.entries(all).filter(([, v]) => v !== null).map(([p, data]) => ({ path: p, data: Buffer.isBuffer(data) ? data : Buffer.from(data) }))
}
function folderOf(list, dir = path.join(tmp(), 'rain-alert')) {
  for (const f of list) {
    fs.mkdirSync(path.dirname(path.join(dir, f.path)), { recursive: true })
    fs.writeFileSync(path.join(dir, f.path), f.data)
  }
  return dir
}
function zipOf(list) {
  const file = path.join(tmp(), 'package.zip')
  fs.writeFileSync(file, pkg.writeZip(list))
  return file
}
const readZip = list => pkg.read(zipOf(list))

test('a good plugin passes, from its folder and from its zip, with its files listed', () => {
  for (const r of [pkg.read(folderOf(files()), { market: true }), readZip(files())]) {
    assert.deepEqual(r.errors, [])
    assert.deepEqual(r.warnings, [])
    assert.equal(r.kind, 'plugin')
    assert.equal(r.manifest.id, 'rain-alert')
    assert.deepEqual(r.files.map(f => f.path), ['icon.svg', 'main.js', 'manifest.json', 'view.html'])
    assert.equal(r.files.find(f => f.path === 'main.js').size, 34)
    assert.ok(r.packed > 0 && r.unpacked > 0)
  }
})

test('an .exe is refused by its type and as a program', () => {
  const r = readZip(files({ 'helper.exe': PE }))
  assert.deepEqual(codes(r.errors).sort(), ['file-type', 'program'])
  assert.ok(r.errors.every(e => e.file === 'helper.exe'))
  assert.match(r.errors.find(e => e.code === 'program').zh, /可执行程序/)
})

test('a program under an allowed name is refused by its first bytes: PE as .png, ELF, Mach-O', () => {
  const pe = readZip(files({ 'art/logo.png': PE }))
  assert.deepEqual(codes(pe.errors), ['program'])
  assert.equal(pe.errors[0].file, 'art/logo.png')
  assert.match(pe.errors[0].en, /MZ\/PE/)
  const elf = readZip(files({ 'sound.mp3': ELF }))
  assert.deepEqual(codes(elf.errors), ['program'])
  assert.match(elf.errors[0].en, /ELF/)
  for (const magic of ['feedface', 'cefaedfe', 'feedfacf', 'cffaedfe', 'cafebabe']) {
    const r = readZip(files({ 'lib.wasm': Buffer.concat([Buffer.from(magic, 'hex'), Buffer.alloc(28)]) }))
    assert.deepEqual(codes(r.errors), ['program'], magic)
  }
  // From a folder the same.
  assert.deepEqual(codes(pkg.read(folderOf(files({ 'logo.png': PE }))).errors), ['program'])
})

test('names that would land outside the package are refused: ../, absolute, a drive, a device', () => {
  for (const name of ['../evil.js', 'a/../../evil.js', '/evil.js', '\\evil.js', 'C:/evil.js', 'c:evil.js', '..\\evil.js', 'nul.js', 'a:b.js', 'x./main.js']) {
    const r = readZip(files({ [name]: 'alert(1)' }))
    assert.deepEqual(codes(r.errors), ['path-unsafe'], name)
    assert.equal(r.errors[0].file, name)
    assert.ok(!r.files.some(f => f.path.includes('evil')), name)
  }
  // Two names Windows takes as one: the second left out.
  const twice = readZip(files({ 'Main.js': 'x' }))
  assert.equal(codes(twice.errors).filter(c => c === 'path-duplicate').length, 1)
  assert.equal(twice.files.filter(f => f.path.toLowerCase() === 'main.js').length, 1)
})

test('limits: 500 files, 20 MB unpacked and 5 MB zipped for a plugin, 200 and 50 for a pack', () => {
  const many = {}
  // The good plugin's four files and 496 more.
  for (let i = 0; i < 496; i++) many[`f/${i}.txt`] = 'x'
  assert.deepEqual(codes(readZip(files(many)).errors), [])
  many['f/496.txt'] = 'x'
  assert.deepEqual(codes(readZip(files(many)).errors), ['too-many-files'])

  // Zeros: a tiny zip, too big unpacked (and not unpacked to find out).
  const big = readZip(files({ 'data.json': Buffer.alloc(21 * 1024 * 1024, 0x20) }))
  assert.deepEqual(codes(big.errors), ['too-large-unpacked'])
  // Noise does not shrink: too big zipped, from a zip and from a folder.
  const noise = crypto.randomBytes(5.2 * 1024 * 1024)
  assert.deepEqual(codes(readZip(files({ 'noise.png': noise })).errors), ['too-large-packed'])
  const folder = pkg.read(folderOf(files({ 'noise.png': noise })))
  assert.deepEqual(codes(folder.errors), ['too-large-packed'])
  assert.deepEqual(codes(folder.warnings), ['content-mismatch'])
  // A pack may have it.
  const pack = readZip([
    { path: 'manifest.json', data: Buffer.from(JSON.stringify({ id: 'beach', kind: 'wallpaper', name: 'Beach', description: 'Sea', version: '1.0.0', author: 'lin' })) },
    { path: 'sea.png', data: Buffer.concat([PNG, noise]) },
  ])
  assert.deepEqual(codes(pack.errors), [])
})

test('ids: their shape, and her own refused', () => {
  for (const id of ['Rain', 'rain_alert', '1rain', 'r', 'rain alert', 'a'.repeat(41), 12]) {
    assert.deepEqual(codes(readZip(files({}, { id })).errors), ['id'], String(id))
  }
  for (const id of ['weather', 'stock', 'pomodoro', 'countdown', 'stretch', 'ci', 'devserver', 'calendar', 'media', 'monitor', 'today', 'tokens', 'waku', 'mail', 'mail-imap', 'mail-gmail']) {
    assert.deepEqual(codes(readZip(files({}, { id })).errors), ['id-reserved'], id)
  }
  assert.deepEqual(codes(readZip(files({}, { id: 'my-weather' })).errors), [])
  // In the market a folder is named after its id.
  const dir = folderOf(files(), path.join(tmp(), 'rain'))
  assert.deepEqual(codes(pkg.read(dir, { market: true }).errors), ['folder-id'])
  assert.deepEqual(codes(pkg.read(dir).errors), [])
})

test('net hosts: exact names or *.domain; no port, no IP, nothing local', () => {
  const r = readZip(files({}, { permissions: { net: ['localhost', '127.0.0.1', 'example.com:8080', '*.example.com', 'api.example.com'] } }))
  assert.deepEqual(codes(r.errors), ['net-host', 'net-host', 'net-host'])
  assert.match(r.errors[0].en, /"localhost"/)
  assert.match(r.errors[1].en, /IP address/)
  assert.match(r.errors[2].en, /port/)
  for (const h of ['[::1]', '::1', '10.0.0.1', '127.1', '0x7f000001', 'printer.local', 'nas.lan', 'app.localhost', 'intranet', '*.com', 'a.*.com', 'https://api.example.com', 'API.example.com', 'bücher.de', 'example.com.', '-x.com', '']) {
    assert.ok(pkg.hostProblem(h), h)
  }
  for (const h of ['api.open-meteo.com', '*.github.io', 'xn--bcher-kva.de', 'a1.b-2.example']) assert.equal(pkg.hostProblem(h), null, h)
  // Only the four permissions there are, flags as true or false.
  assert.deepEqual(codes(readZip(files({}, { permissions: { files: true } })).errors), ['permissions'])
  assert.deepEqual(codes(readZip(files({}, { permissions: { agents: 'yes' } })).errors), ['permissions'])
})

test('a plugin needs main, a .js in the package; view, an .html in it', () => {
  assert.deepEqual(codes(readZip(files({}, { main: undefined })).errors), ['main'])
  assert.deepEqual(codes(readZip(files({ 'main.js': null })).errors), ['main'])
  assert.deepEqual(codes(readZip(files({}, { main: 'view.html' })).errors), ['main'])
  assert.deepEqual(codes(readZip(files({}, { main: './main.js' })).errors), ['main'])
  assert.deepEqual(codes(readZip(files({}, { main: '../main.js' })).errors), ['main'])
  assert.deepEqual(codes(readZip(files({ 'view.html': null })).errors), ['view'])
  assert.deepEqual(codes(readZip(files({ 'src/run.js': 'x' }, { main: 'src/run.js' })).errors), [])
})

test("the manifest's other rules, and the warnings an author gets", () => {
  const e = m => codes(readZip(files({}, m)).errors)
  const w = (m, change = {}) => codes(readZip(files(change, m)).warnings)
  assert.deepEqual(e({ kind: 'theme' }), ['kind'])
  assert.deepEqual(e({ version: '1.2' }), ['version'])
  assert.deepEqual(e({ name: 'A name far longer than twenty-four' }), ['name'])
  assert.deepEqual(e({ name: { zh: '只有中文' } }), ['text'])
  assert.deepEqual(e({ author: '' }), ['author'])
  assert.deepEqual(e({ api: 2 }), ['api'])
  assert.deepEqual(e({ category: 'games' }), ['category'])
  assert.deepEqual(e({ widget: { sizes: ['xl'] } }), ['sizes'])
  assert.deepEqual(e({ widget: { color: 'blue' } }), ['color'])
  assert.deepEqual(e({ homepage: 'ftp://x' }), ['homepage'])
  assert.deepEqual(e({ settings: [{ key: 'city name', type: 'text', label: 'City' }] }), ['settings'])
  assert.deepEqual(e({ settings: [{ key: 'Mode', type: 'choice', label: 'Mode' }] }), ['settings'])
  assert.deepEqual(e({ settings: [{ key: 'N', type: 'number', label: 'N', default: '3' }] }), ['settings'])
  assert.deepEqual(e({ settings: [{ key: 'A', type: 'flag', label: 'A' }, { key: 'a', type: 'flag', label: 'a' }] }), ['settings'])
  assert.deepEqual(e({ settings: [{ key: 'M', type: 'choice', options: ['x'], optionLabels: { y: 'Y' }, label: 'M' }] }), ['settings'])
  // Warnings: no icon, a long description, a view nowhere to be drawn, an
  // icon not hers, a field not known, a picture that is none.
  assert.deepEqual(w({ widget: { sizes: ['s', 'm'] } }), ['no-icon'])
  assert.deepEqual(w({ description: 'A description that goes on and on, well past what the market shows on a line' }), ['description-long'])
  assert.deepEqual(w({ widget: { sizes: ['s'], icon: 'icon.svg' } }), ['view-sizes'])
  assert.deepEqual(w({ widget: { icon: 'cloud' } }), ['icon-name'])
  assert.deepEqual(w({ widget: { icon: 'bell' } }), [])
  assert.deepEqual(w({ colour: 'red' }), ['field-unknown'])
  assert.deepEqual(w({}, { 'img/a.png': 'not a png' }), ['content-mismatch'])
  // Her page draws the icon: nothing in it that runs or reaches out.
  assert.deepEqual(codes(readZip(files({ 'icon.svg': '<svg onload="x()"/>' })).errors), ['icon-unsafe'])
  assert.deepEqual(codes(readZip(files({ 'icon.svg': '<svg><image href="https://x.test/a.png"/></svg>' })).errors), ['icon-unsafe'])
  assert.deepEqual(codes(readZip(files({ 'icon.svg': `<svg>${' '.repeat(9000)}</svg>` })).warnings), ['icon-large'])
  // A manifest saved with a BOM, or not JSON.
  assert.deepEqual(codes(readZip(files({ 'manifest.json': Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), Buffer.from(JSON.stringify(MANIFEST))]) })).errors), ['manifest-json'])
  assert.deepEqual(codes(readZip(files({ 'manifest.json': '{ id: 1 }' })).errors), ['manifest-json'])
  assert.deepEqual(codes(readZip(files({ 'manifest.json': null })).errors), ['manifest-missing'])
})

const PACK = { name: { en: 'Beach', zh: '海边' }, description: 'Sea and sand', version: '1.0.0', author: 'lin' }

test('packs: a wallpaper pack takes pictures, json and md/txt only; a pet pack its pet.json and sheet', () => {
  const wall = list => readZip([{ path: 'manifest.json', data: Buffer.from(JSON.stringify({ id: 'beach', kind: 'wallpaper', ...PACK })) }, ...list])
  assert.deepEqual(codes(wall([{ path: 'sea.png', data: PNG }, { path: 'README.md', data: Buffer.from('# Beach') }]).errors), [])
  const js = wall([{ path: 'sea.png', data: PNG }, { path: 'slideshow.js', data: Buffer.from('x') }])
  assert.deepEqual(codes(js.errors), ['file-type'])
  assert.equal(js.errors[0].file, 'slideshow.js')
  assert.deepEqual(codes(wall([{ path: 'sea.gif', data: Buffer.from('GIF89a') }]).errors), ['file-type', 'wallpaper-empty'])
  assert.deepEqual(codes(wall([{ path: 'more/sea.png', data: PNG }, { path: 'top.png', data: PNG }]).warnings), ['wallpaper-folder'])

  const pet = (list, m = {}) => readZip([{ path: 'manifest.json', data: Buffer.from(JSON.stringify({ id: 'cat', kind: 'pet', ...PACK, ...m })) }, ...list])
  const petJson = { path: 'pet.json', data: Buffer.from(JSON.stringify({ id: 'cat', displayName: 'Cat', spritesheetPath: 'spritesheet.webp' })) }
  const sheet = { path: 'spritesheet.webp', data: WEBP }
  assert.deepEqual(codes(pet([petJson, sheet]).errors), [])
  assert.deepEqual(codes(pet([sheet]).errors), ['pet-json'])
  assert.deepEqual(codes(pet([petJson]).errors), ['pet-sheet'])
  assert.deepEqual(codes(pet([petJson, sheet, { path: 'walk.js', data: Buffer.from('x') }]).errors), ['file-type'])
  // No code, so no main, view, settings or permissions.
  assert.deepEqual(codes(pet([petJson, sheet], { main: 'main.js', permissions: {} }).errors), ['pack-field', 'pack-field'])
  assert.deepEqual(codes(pet([petJson, sheet], { category: 'info' }).warnings), ['pack-field-ignored'])
})

test('one top folder holding everything is taken away; files beside it keep it', () => {
  const list = files().map(f => ({ ...f, path: `rain-alert-main/${f.path}` }))
  const r = readZip([{ path: 'rain-alert-main/', data: Buffer.alloc(0) }, ...list])
  assert.deepEqual(r.errors, [])
  assert.deepEqual(r.files.map(f => f.path), ['icon.svg', 'main.js', 'manifest.json', 'view.html'])
  // Its zip has the files at its top.
  const again = pkg.unzip(pkg.zipOf(r)).map(e => e.name)
  assert.deepEqual(again, ['icon.svg', 'main.js', 'manifest.json', 'view.html'])
  // Two top folders: nothing taken away, so no manifest at the top.
  assert.deepEqual(codes(readZip([...list, { path: '__MACOSX/._x', data: Buffer.from('x') }]).errors).sort(), ['file-type', 'manifest-missing'])
  // As PowerShell's Compress-Archive writes them: backslashes.
  const ps = readZip(files().map(f => ({ ...f, path: `rain-alert\\${f.path}` })))
  assert.deepEqual(ps.errors, [])
  assert.deepEqual(ps.files.map(f => f.path), ['icon.svg', 'main.js', 'manifest.json', 'view.html'])
})

test('a damaged or odd zip is refused, not half read', () => {
  const good = pkg.writeZip(files())
  const file = path.join(tmp(), 'bad.zip')
  // A byte of main.js's data (stored: it is short), so its CRC fails.
  const bad = Buffer.from(good)
  const at = bad.indexOf('waku.widget')
  assert.ok(at > 0)
  bad[at] ^= 1
  fs.writeFileSync(file, bad)
  const crc = pkg.read(file)
  assert.deepEqual(codes(crc.errors), ['zip-bad'])
  assert.equal(crc.errors[0].file, 'main.js')
  assert.match(crc.errors[0].en, /CRC/)
  // A local header naming another file than the directory does.
  const other = Buffer.from(good)
  other.write('wain.js', other.indexOf('main.js'), 'latin1')
  fs.writeFileSync(file, other)
  assert.match(pkg.read(file).errors.map(e => e.en).join(), /local header names another file/)
  // A file that inflates to more than the directory says (a zip bomb's way).
  const lying = pkg.writeZip([...files(), { path: 'x.txt', data: Buffer.alloc(4000, 0x41) }])
  const cd = lying.lastIndexOf(Buffer.from('x.txt')) - 46
  lying.writeUInt32LE(10, cd + 24)
  fs.writeFileSync(file, lying)
  assert.match(pkg.read(file).errors.map(e => e.en).join(), /bigger than it says/)
  fs.writeFileSync(file, 'not a zip at all')
  assert.deepEqual(codes(pkg.read(file).errors), ['zip-bad'])
  fs.writeFileSync(file, good.subarray(0, good.length - 30))
  assert.deepEqual(codes(pkg.read(file).errors), ['zip-bad'])
})

test('the same folder gives the same zip, byte for byte, whatever the order, place or times', () => {
  const a = folderOf(files({ 'img/b.png': PNG, 'img/a.png': PNG, 'zz.txt': 'z' }))
  const b = folderOf([...files({ 'img/b.png': PNG, 'img/a.png': PNG, 'zz.txt': 'z' })].reverse())
  fs.utimesSync(path.join(b, 'main.js'), new Date('2001-02-03'), new Date('2001-02-03'))
  const za = pkg.zipOf(pkg.read(a))
  const zb = pkg.zipOf(pkg.read(b))
  assert.equal(sha(za), sha(zb))
  assert.equal(sha(za), sha(pkg.zipOf(pkg.read(a))))
  assert.equal(sha(pkg.writeZip(files())), sha(pkg.writeZip(files().reverse())))
  // Read back: every entry dated 1980-01-01, sorted, its bytes as they were.
  const entries = pkg.unzip(za)
  assert.deepEqual(entries.map(e => e.name), ['icon.svg', 'img/a.png', 'img/b.png', 'main.js', 'manifest.json', 'view.html', 'zz.txt'])
  assert.equal(entries.find(e => e.name === 'main.js').load().toString(), "waku.widget.set({ value: '70%' })\n")
  // A changed file, another zip.
  fs.writeFileSync(path.join(b, 'zz.txt'), 'y')
  assert.notEqual(sha(za), sha(pkg.zipOf(pkg.read(b))))
})

function market(list) {
  const root = tmp()
  const packages = path.join(root, 'packages')
  for (const [id, fl] of Object.entries(list)) folderOf(fl, path.join(packages, id))
  return { root, packages, out: path.join(root, 'out') }
}
const wallpaper = (id, version = '1.0.0') => [
  { path: 'manifest.json', data: Buffer.from(JSON.stringify({ id, kind: 'wallpaper', ...PACK, version })) },
  { path: 'sea.png', data: PNG },
]
const plain = (id, m = {}) => files({ 'view.html': null, 'icon.svg': null }, { id, view: undefined, widget: { icon: 'bell' }, permissions: undefined, ...m })

test('build-index: the zips and index.json, items by kind then id, as the app reads them', () => {
  const m = market({ 'rain-alert': files(), 'beach': wallpaper('beach'), 'a-clock': plain('a-clock', { category: 'productivity' }) })
  const r = build({ packages: m.packages, out: m.out, now: '2026-10-21T08:00:00Z', changed: path.join(m.root, 'changed.txt') })
  assert.equal(r.ok, true)
  const index = JSON.parse(fs.readFileSync(path.join(m.out, 'index.json'), 'utf8'))
  assert.equal(index.format, 1)
  assert.equal(index.generated, '2026-10-21T08:00:00.000Z')
  assert.deepEqual(index.items.map(i => i.id), ['a-clock', 'rain-alert', 'beach'])
  const rain = index.items[1]
  assert.deepEqual(Object.keys(rain), ['id', 'kind', 'name', 'description', 'author', 'homepage', 'version', 'api', 'category', 'sizes', 'color', 'view', 'permissions', 'file', 'size', 'sha256', 'icon', 'glyph', 'sample', 'preview', 'updated'])
  assert.deepEqual(rain.name, MANIFEST.name)
  assert.equal(rain.file, 'rain-alert-1.2.0.zip')
  assert.equal(rain.view, true)
  assert.deepEqual(rain.sizes, ['s', 'm'])
  assert.equal(rain.color, '#8cc8ff')
  assert.equal(rain.category, 'info')
  assert.deepEqual(rain.permissions, { net: ['api.open-meteo.com'], notify: true })
  assert.equal(rain.icon, `data:image/svg+xml;base64,${Buffer.from(ICON).toString('base64')}`)
  assert.equal(rain.updated, '2026-10-21')
  const zip = fs.readFileSync(path.join(m.out, rain.file))
  assert.equal(rain.size, zip.length)
  assert.equal(rain.sha256, sha(zip))
  assert.deepEqual(codes(pkg.read(path.join(m.out, rain.file)).errors), [])
  // Its own .svg: no glyph of hers; no sample given, none.
  assert.deepEqual([rain.glyph, rain.sample, rain.preview], [null, null, null])
  const clock = index.items[0]
  assert.equal(clock.view, false)
  assert.equal(clock.icon, null)
  assert.equal(clock.glyph, 'bell')
  assert.equal(clock.homepage, 'https://github.com/lin/rain-alert')
  assert.deepEqual(clock.permissions, {})
  const beach = index.items[2]
  assert.deepEqual([beach.kind, beach.api, beach.category, beach.sizes, beach.color, beach.view, beach.icon, beach.glyph, beach.sample, beach.preview, beach.homepage], ['wallpaper', null, null, null, null, false, null, null, null, null, null])
  assert.deepEqual(fs.readFileSync(path.join(m.root, 'changed.txt'), 'utf8').split('\n').filter(Boolean).sort(), ['a-clock-1.2.0.zip', 'beach-1.0.0.zip', 'rain-alert-1.2.0.zip'])
})

const SAMPLE = { label: 'Dresden', value: '70%', detail: { series: { points: [10, 30, 70], unit: '%', hi: 100, lo: 0 }, rows: [{ kind: 'hour', when: '17:00', icon: 'rain', value: '70%' }], extra: '17:00 ☂' } }

test('widget.sample: checked as she takes a widget; what she would cut or drop said; 4 KB at most', () => {
  const sample = s => readZip(files({}, { widget: { ...MANIFEST.widget, sample: s } }))
  const good = sample(SAMPLE)
  assert.deepEqual(good.errors, [])
  assert.deepEqual(good.warnings, [])
  // The wrong kind of thing: errors.
  assert.deepEqual(codes(sample('70%').errors), ['sample'])
  assert.deepEqual(codes(sample({ value: 70 }).errors), ['sample'])
  assert.deepEqual(codes(sample({ color: 'blue' }).errors), ['sample'])
  assert.deepEqual(codes(sample({ private: 'yes' }).errors), ['sample'])
  assert.deepEqual(codes(sample({ detail: { series: { points: ['a'] } } }).errors), ['sample'])
  assert.deepEqual(codes(sample({ detail: { rows: {} } }).errors), ['sample'])
  assert.deepEqual(codes(sample({ detail: { rows: [{ at: -1 }] } }).errors), ['sample'])
  const big = sample({ value: '1', detail: { rows: Array.from({ length: 24 }, () => ({ subject: '雨'.repeat(60), name: 'y'.repeat(16) })), extra: 'z' } })
  assert.ok(codes(big.errors).includes('sample'), 'over 4 KB')
  assert.match(big.errors.find(e => e.code === 'sample').en, /at most 4 KB/)
  // What she would cut or drop: warnings, each said.
  const w = s => sample(s).warnings.map(p => p.en)
  assert.match(w({ value: 'a value far longer than twenty-four' }).join(), /value is over 24 characters/)
  assert.match(w({ value: '1', ttl: 60 }).join(), /"ttl" is not used/)
  assert.match(w({ value: '1', icon: 'cloud' }).join(), /not one of her icons/)
  assert.match(w({ value: '1', detail: { rows: Array.from({ length: 13 }, (_, i) => ({ name: String(i) })) } }).join(), /first 12 are kept/)
  assert.match(w({ value: '1', detail: { rows: [{ name: 'a', colour: 'red' }] } }).join(), /"colour" is not used/)
  // As the index carries it, with her glyph for an icon of hers.
  const m = market({ 'a-clock': plain('a-clock', { widget: { icon: 'bell', sample: SAMPLE } }) })
  assert.equal(build({ packages: m.packages, out: m.out }).ok, true)
  const item = JSON.parse(fs.readFileSync(path.join(m.out, 'index.json'), 'utf8')).items[0]
  assert.deepEqual([item.glyph, item.sample], ['bell', SAMPLE])
  // check.js says when there is none: the market draws no cards for it.
  const run = dir => spawnSync(process.execPath, [path.join(MARKET, 'check.js'), dir], { encoding: 'utf8', env: { ...process.env, GITHUB_ACTIONS: '' } }).stdout
  assert.match(run(folderOf(files())), /no widget\.sample/)
  assert.doesNotMatch(run(path.join(m.packages, 'a-clock')), /no widget\.sample/)
})

test("a pack's preview: preview.webp|png|jpg, 64 KB or less, inlined; neither a wallpaper nor counted as one", () => {
  const wall = (list, id = 'beach') => [{ path: 'manifest.json', data: Buffer.from(JSON.stringify({ id, kind: 'wallpaper', ...PACK })) }, ...list]
  const m = market({ beach: wall([{ path: 'sea.png', data: PNG }, { path: 'preview.webp', data: WEBP }]) })
  assert.equal(build({ packages: m.packages, out: m.out }).ok, true)
  const item = JSON.parse(fs.readFileSync(path.join(m.out, 'index.json'), 'utf8')).items[0]
  assert.equal(item.preview, `data:image/webp;base64,${WEBP.toString('base64')}`)
  assert.equal(item.sample, null)
  // Too big: not shown, said.
  const huge = Buffer.concat([PNG, crypto.randomBytes(70 * 1024)])
  const r = readZip(wall([{ path: 'sea.png', data: PNG }, { path: 'preview.png', data: huge }]))
  assert.deepEqual(codes(r.errors), [])
  assert.deepEqual(codes(r.warnings), ['preview-large'])
  assert.equal(pkg.previewOf(r), null)
  // A wallpaper pack with its preview alone has no wallpaper.
  assert.deepEqual(codes(readZip(wall([{ path: 'preview.jpg', data: Buffer.from('ffd8ffe0', 'hex') }])).errors), ['wallpaper-empty'])
  // A plugin's preview.png is only a picture of its own.
  assert.equal(pkg.previewOf(readZip(files({ 'preview.png': PNG }))), null)
})

test("build-index: a version keeps the day it was first built; a new one gets today's", () => {
  const m = market({ 'rain-alert': files(), 'beach': wallpaper('beach') })
  assert.equal(build({ packages: m.packages, out: m.out, now: '2026-10-21T08:00:00Z' }).ok, true)
  const first = JSON.parse(fs.readFileSync(path.join(m.out, 'index.json'), 'utf8'))
  // A day later, nothing changed: the same days, the same bytes, nothing to upload.
  const changed = path.join(m.root, 'changed.txt')
  const again = build({ packages: m.packages, out: m.out, now: '2026-10-22T08:00:00Z', changed })
  assert.deepEqual(again.index.items.map(i => i.updated), ['2026-10-21', '2026-10-21'])
  assert.deepEqual(again.index.items.map(i => i.sha256), first.items.map(i => i.sha256))
  assert.equal(fs.readFileSync(changed, 'utf8'), '')
  // The wallpapers' version raised: its day is the new one, and only it goes up.
  folderOf(wallpaper('beach', '1.1.0'), path.join(m.packages, 'beach'))
  const prev = path.join(m.root, 'published.json')
  fs.copyFileSync(path.join(m.out, 'index.json'), prev)
  fs.rmSync(path.join(m.out, 'index.json'))
  const third = build({ packages: m.packages, out: m.out, previous: prev, now: '2026-10-23T08:00:00Z', changed })
  assert.deepEqual(third.index.items.map(i => [i.id, i.version, i.updated]), [['rain-alert', '1.2.0', '2026-10-21'], ['beach', '1.1.0', '2026-10-23']])
  assert.equal(fs.readFileSync(changed, 'utf8'), 'beach-1.1.0.zip\n')
})

test('build-index writes nothing when any package fails, and says why', () => {
  const m = market({ 'rain-alert': files(), 'bad': files({ 'run.exe': PE }, { id: 'bad' }) })
  const r = build({ packages: m.packages, out: m.out })
  assert.equal(r.ok, false)
  assert.deepEqual(r.failed.map(f => path.basename(f.dir)), ['bad'])
  assert.equal(fs.existsSync(m.out), false)
  const cli = spawnSync(process.execPath, [path.join(MARKET, 'build-index.js'), '--packages', m.packages, '--out', m.out], { encoding: 'utf8' })
  assert.equal(cli.status, 1)
  assert.match(cli.stdout, /run\.exe/)
  assert.match(cli.stdout, /not built/)
})

test('check.js: a report in both languages, exit 1 on an error, 0 with warnings only', () => {
  const good = folderOf(files())
  const warned = folderOf(files({}, { widget: { sizes: ['s', 'm'] } }))
  const bad = zipOf(files({ '../evil.js': 'x' }))
  const run = (...args) => spawnSync(process.execPath, [path.join(MARKET, 'check.js'), ...args], { encoding: 'utf8', env: { ...process.env, GITHUB_ACTIONS: '' } })
  const ok = run(good, warned)
  assert.equal(ok.status, 0)
  assert.match(ok.stdout, /通过/)
  assert.match(ok.stdout, /passed/)
  assert.match(ok.stdout, /没有图标/)
  assert.match(ok.stdout, /no icon/)
  const no = run(good, bad)
  assert.equal(no.status, 1)
  assert.match(no.stdout, /错误 {2}\.\.\/evil\.js: 文件名不安全/)
  assert.match(no.stdout, /error \.\.\/evil\.js: unsafe file name/)
  assert.match(no.stdout, /2 packages: 1 passed, 1 failed/)
  const json = JSON.parse(run('--json', bad).stdout)
  assert.deepEqual(json[0].errors.map(e => e.code), ['path-unsafe'])
  assert.equal(run().status, 2)
})

test('check.js --against: files changed in a package need a higher version', () => {
  const base = market({ 'rain-alert': files() })
  const same = market({ 'rain-alert': files() })
  const edited = market({ 'rain-alert': files({ 'main.js': 'changed()' }) })
  const raised = market({ 'rain-alert': files({ 'main.js': 'changed()' }, { version: '1.2.1' }) })
  const lower = market({ 'rain-alert': files({}, { version: '1.1.9' }) })
  const one = m => check([path.join(m.packages, 'rain-alert')], { market: true, against: base.packages })[0]
  assert.deepEqual(codes(one(same).errors), [])
  assert.deepEqual(codes(one(edited).errors), ['version-not-bumped'])
  assert.deepEqual(codes(one(raised).errors), [])
  assert.deepEqual(codes(one(lower).errors), ['version-lower'])
  // A new package has nothing to be compared with.
  const fresh = market({ 'a-clock': plain('a-clock') })
  assert.deepEqual(codes(check([path.join(fresh.packages, 'a-clock')], { market: true, against: base.packages })[0].errors), [])
})

function get(url, opts = {}) {
  return new Promise((resolve, reject) => {
    const req = http.get(url, res => {
      const parts = []
      res.on('data', d => parts.push(d))
      res.on('end', () => resolve({ status: res.statusCode, headers: res.headers, body: Buffer.concat(parts) }))
    })
    req.on('error', reject)
    if (opts.timeout) req.setTimeout(opts.timeout, () => { req.destroy(); resolve({ status: 'timeout' }) })
  })
}

test('the fake market serves the index and zips where GitHub and Gitee have them, and goes wrong on request', async () => {
  const m = market({ 'rain-alert': files(), 'beach': wallpaper('beach') })
  const served = await fake.start(m.packages, { port: 0, log: false, slowWait: 50, slowRate: 1024 * 1024 })
  try {
    assert.equal(served.built, true)
    assert.match(served.base, /^http:\/\/127\.0\.0\.1:\d+\/releases\/download\/market\/$/)
    const index = JSON.parse((await get(`${served.base}index.json`)).body)
    const item = index.items.find(i => i.id === 'rain-alert')
    const zip = await get(served.base + item.file)
    assert.equal(zip.status, 200)
    assert.equal(zip.headers['content-type'], 'application/zip')
    assert.equal(sha(zip.body), item.sha256)
    const origin = served.base.replace(/\/releases\/download\/market\/$/, '')
    // Also at the top, and under an owner and repository as on GitHub.
    assert.equal(sha((await get(`${origin}/${item.file}`)).body), item.sha256)
    assert.equal(sha((await get(`${origin}/lin/wakuwaku-plugins/releases/download/market/${item.file}`)).body), item.sha256)
    assert.equal((await get(`${served.base}nothing.zip`)).status, 404)
    assert.equal((await get(`${served.base}..%2Fpackage.json`)).status, 404)

    // A query on one request.
    assert.equal((await get(`${served.base}index.json?missing`)).status, 404)
    assert.equal((await get(`${served.base}index.json?fail`)).status, 500)
    const bad = await get(`${served.base}${item.file}?badsha`)
    assert.equal(bad.body.length, item.size)
    assert.notEqual(sha(bad.body), item.sha256)
    const moved = await get(`${served.base}${item.file}?redirect`)
    assert.equal(moved.status, 302)
    assert.match(moved.headers.location, new RegExp(`^/storage/${item.file}`))
    assert.equal(sha((await get(origin + moved.headers.location)).body), item.sha256)
    const t = Date.now()
    assert.equal(sha((await get(`${served.base}${item.file}?slow`)).body), item.sha256)
    assert.ok(Date.now() - t >= 45)
    assert.equal((await get(`${served.base}index.json?stall`, { timeout: 200 })).status, 'timeout')

    // A base that carries modes, for one client; one file only.
    const only = `${origin}/@missing=${item.file},badsha=*.zip/releases/download/market/`
    assert.equal((await get(`${only}index.json`)).status, 200)
    assert.equal((await get(only + item.file)).status, 404)
    const beach = index.items.find(i => i.id === 'beach')
    assert.notEqual(sha((await get(only + beach.file)).body), beach.sha256)
    // Switched while it runs, for everyone.
    assert.deepEqual(JSON.parse((await get(`${origin}/mode?badsha=1`)).body), { badsha: '1' })
    assert.equal((await get(`${served.base}index.json`)).status, 200)
    assert.notEqual(sha((await get(served.base + item.file)).body), item.sha256)
    assert.deepEqual(JSON.parse((await get(`${origin}/mode?badsha=0`)).body), {})
    assert.equal(sha((await get(served.base + item.file)).body), item.sha256)
  } finally {
    await served.close()
  }
  assert.equal(fs.existsSync(served.root), false)
  assert.deepEqual(fake.parseModes('slow, badsha=a.zip,nonsense'), { slow: '1', badsha: 'a.zip' })
})

test('the Gitee mirror makes the release, puts up what it lacks and index.json last', async () => {
  const m = market({ 'rain-alert': files(), 'beach': wallpaper('beach') })
  assert.equal(build({ packages: m.packages, out: m.out }).ok, true)
  // A Gitee as its API answers: no release yet, then one; its files.
  const calls = []
  let release = null
  let files_ = []
  let nextId = 1
  const gitee = http.createServer((req, res) => {
    const parts = []
    req.on('data', d => parts.push(d))
    req.on('end', () => {
      const body = Buffer.concat(parts)
      const url = new URL(req.url, 'http://x')
      calls.push(`${req.method} ${url.pathname}`)
      const send = (status, json) => { res.writeHead(status, { 'Content-Type': 'application/json' }); res.end(json === undefined ? '' : JSON.stringify(json)) }
      assert.equal(req.headers.authorization, 'token secret')
      if (url.pathname === '/api/v5/repos/lin/market') return send(200, { default_branch: 'master' })
      if (url.pathname === '/api/v5/repos/lin/market/releases/tags/market') return release ? send(200, release) : send(404, { message: 'Not Found' })
      if (url.pathname === '/api/v5/repos/lin/market/releases' && req.method === 'POST') {
        const f = new URLSearchParams(body.toString())
        assert.equal(f.get('tag_name'), 'market')
        assert.equal(f.get('target_commitish'), 'master')
        release = { id: 7, tag_name: 'market' }
        return send(201, release)
      }
      if (url.pathname === '/api/v5/repos/lin/market/releases/7/attach_files' && req.method === 'GET') return send(200, files_.map(f => ({ id: f.id, name: f.name })))
      if (url.pathname === '/api/v5/repos/lin/market/releases/7/attach_files' && req.method === 'POST') {
        const name = /filename="([^"]+)"/.exec(body.toString('latin1'))[1]
        files_.push({ id: nextId++, name, body })
        return send(201, { id: nextId - 1, name })
      }
      const del = url.pathname.match(/^\/api\/v5\/repos\/lin\/market\/releases\/7\/attach_files\/(\d+)$/)
      if (del && req.method === 'DELETE') {
        files_ = files_.filter(f => f.id !== Number(del[1]))
        return send(204)
      }
      send(404, { message: 'Not Found' })
    })
  })
  await new Promise(r => gitee.listen(0, '127.0.0.1', r))
  const api = `http://127.0.0.1:${gitee.address().port}/api/v5`
  try {
    const opts = { repo: 'lin/market', dir: m.out, token: 'secret', api, verify: false, log: () => {}, retryWait: 1 }
    const first = await mirror(opts)
    assert.deepEqual(first.put, ['beach-1.0.0.zip', 'rain-alert-1.2.0.zip', 'index.json'])
    assert.ok(calls.includes('POST /api/v5/repos/lin/market/releases'))
    // Again: only index.json, the old one taken away first; a changed zip too.
    calls.length = 0
    const again = await mirror({ ...opts, changed: ['beach-1.0.0.zip'] })
    assert.deepEqual(again.put, ['beach-1.0.0.zip', 'index.json'])
    assert.ok(!calls.includes('POST /api/v5/repos/lin/market/releases'))
    assert.equal(calls.filter(c => c.startsWith('DELETE')).length, 2)
    assert.deepEqual(files_.map(f => f.name).sort(), ['beach-1.0.0.zip', 'index.json', 'rain-alert-1.2.0.zip'])
    assert.ok(files_.find(f => f.name === 'index.json').body.includes(fs.readFileSync(path.join(m.out, 'index.json'))))
  } finally {
    await new Promise(r => gitee.close(r))
  }
  // Without a token it does nothing.
  const out = execFileSync(process.execPath, [path.join(MARKET, 'gitee-mirror.js'), '--repo', 'lin/market', '--dir', m.out], { encoding: 'utf8', env: { ...process.env, GITEE_TOKEN: '' } })
  assert.match(out, /no GITEE_TOKEN/)
})
