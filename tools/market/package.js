// A Wakuwaku package (a plugin, or a pet or wallpaper pack) read and checked
// by docs/PLUGINS.md's rules, from its folder or its .zip: what check.js
// says, what build-index.js builds the market from, and the zip it builds.
//
//   const pkg = require('./package')
//   const r = pkg.read('packages/rain-alert')   // or 'rain-alert.zip'
//   r.manifest, r.kind, r.files ([{ path, size }]), r.errors, r.warnings,
//   r.packed (bytes zipped), r.unpacked
//   pkg.zipOf(r)                                // the same bytes every time
//
// Each problem is { code, file, zh, en }: file is the package's file it is
// about ('' for the package as a whole), zh and en say it for its author.
// No dependencies: the zip is read and written here, deflate by node's zlib.
const fs = require('node:fs')
const path = require('node:path')
const zlib = require('node:zlib')
const crypto = require('node:crypto')

const API = 1
const MB = 1024 * 1024
const LIMITS = {
  plugin: { packed: 5 * MB, unpacked: 20 * MB },
  pet: { packed: 50 * MB, unpacked: 200 * MB },
  wallpaper: { packed: 50 * MB, unpacked: 200 * MB },
}
const MAX_FILES = 500
// A zip with more entries than this is not looked into at all.
const MAX_ENTRIES = 10000
const ICON_MAX = 8 * 1024
// widget.sample, as JSON; a pack's preview picture.
const SAMPLE_MAX = 4 * 1024
const PREVIEW_MAX = 64 * 1024
// A pack's picture for the market (its top level), by the first there is.
const PREVIEWS = ['preview.webp', 'preview.png', 'preview.jpg']
const PREVIEW_TYPES = { '.webp': 'image/webp', '.png': 'image/png', '.jpg': 'image/jpeg' }

// File types by kind. A plugin is web files; a pack has no code: images, its
// json and a readme or licence.
const PLUGIN_TYPES = ['.js', '.mjs', '.json', '.html', '.htm', '.css', '.svg', '.png', '.jpg', '.jpeg', '.webp', '.gif', '.avif', '.woff', '.woff2', '.ttf', '.otf', '.mp3', '.ogg', '.wav', '.txt', '.md', '.wasm']
const PACK_TEXT = ['.json', '.md', '.txt']
const PET_IMAGES = ['.webp', '.png', '.jpg', '.jpeg']
const WALLPAPER_IMAGES = ['.jpg', '.jpeg', '.png', '.webp']
const TYPES = {
  plugin: PLUGIN_TYPES,
  pet: [...PET_IMAGES, ...PACK_TEXT],
  wallpaper: [...WALLPAPER_IMAGES, ...PACK_TEXT],
}
const KINDS = Object.keys(LIMITS)

// Her built-in plugins' and widgets' ids, and anything of mail's (mail-imap,
// mail-thunderbird…).
const RESERVED = ['weather', 'stock', 'pomodoro', 'countdown', 'stretch', 'ci', 'devserver', 'calendar', 'media', 'monitor', 'today', 'tokens', 'waku']
const isReserved = id => RESERVED.includes(id) || id.startsWith('mail')
// Her icons (src-tauri/src/widgets.rs ICONS). One not in the list is drawn as
// the name's first letter, so it is only a warning.
const ICONS = ['cpu', 'weather', 'note', 'calendar', 'bell', 'stock', 'today', 'clock', 'mail', 'music', 'code', 'star', 'dot', 'chart', 'battery', 'timer', 'flag', 'server',
  'check', 'terminal', 'coin', 'gauge', 'memory', 'down', 'up', 'bolt']
const CATEGORIES = ['system', 'productivity', 'info']
const SIZES = ['s', 'm', 'l']
const SETTING_TYPES = ['text', 'number', 'flag', 'choice']
const FLAGS = ['agents', 'hardware', 'notify']
const FIELDS = ['id', 'kind', 'name', 'description', 'version', 'author', 'homepage', 'api', 'main', 'view', 'category', 'widget', 'settings', 'permissions']
const ID = /^[a-z][a-z0-9-]{1,39}$/
const VERSION = /^(0|[1-9]\d{0,5})\.(0|[1-9]\d{0,5})\.(0|[1-9]\d{0,5})$/
const KEY = /^[A-Za-z][A-Za-z0-9]{0,31}$/
const COLOR = /^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/
// Names that only ever reach this computer or its network.
const LOCAL_SUFFIXES = ['localhost', 'local', 'localdomain', 'internal', 'lan', 'home.arpa', 'in-addr.arpa', 'ip6.arpa']

const problem = (code, file, zh, en) => ({ code, file, zh, en })
const sha256 = buf => crypto.createHash('sha256').update(buf).digest('hex')
const chars = s => [...s].length
const extOf = p => {
  const base = p.slice(p.lastIndexOf('/') + 1)
  const dot = base.lastIndexOf('.')
  return dot > 0 ? base.slice(dot).toLowerCase() : ''
}
const kb = n => n < MB ? `${(n / 1024).toFixed(1)} KB` : `${(n / MB).toFixed(1)} MB`

// CRC-32 as zip has it: node's own where it has one (22.2 and later).
const CRC_TABLE = (() => {
  const t = new Uint32Array(256)
  for (let n = 0; n < 256; n++) {
    let c = n
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1
    t[n] = c >>> 0
  }
  return t
})()
function crc32(buf) {
  if (zlib.crc32) return zlib.crc32(buf) >>> 0
  let c = 0xffffffff
  for (let i = 0; i < buf.length; i++) c = CRC_TABLE[(c ^ buf[i]) & 0xff] ^ (c >>> 8)
  return (c ^ 0xffffffff) >>> 0
}

// --- Paths ---------------------------------------------------------------------

// A name in a zip as a path inside the package, as her extractor takes one
// (src-tauri/src/lhm_setup.rs zip::safe): backslashes as slashes; no leading
// slash, no drive; no part empty, "." or ".."; none with a stream (:), a
// letter Windows refuses, a device's name (CON, NUL, COM1…), nor ending in a
// dot or a space. Its parts, or why not.
function partsOf(name) {
  const n = name.replace(/\\/g, '/')
  if (n.startsWith('/')) return { why: ['是绝对路径', 'an absolute path'] }
  if (/^[A-Za-z]:/.test(n)) return { why: ['带盘符', 'a drive letter'] }
  const parts = n.replace(/\/+$/, '').split('/')
  for (const p of parts) {
    if (p === '..') return { why: ['指向包外面（..）', 'points outside the package (..)'] }
    if (p === '' || p === '.') return { why: ['有空的一段或 "."', 'an empty part or "."'] }
    if (/[\u0000-\u001f<>:"|?*]/.test(p)) return { why: ['有 Windows 不允许的字符', 'a character Windows does not allow'] }
    if (/[. ]$/.test(p)) return { why: ['以点或空格结尾', 'ends in a dot or a space'] }
    const stem = p.split('.')[0].toUpperCase()
    if (['CON', 'PRN', 'AUX', 'NUL', 'CONIN$', 'CONOUT$'].includes(stem) || /^(COM|LPT)\d$/.test(stem)) return { why: ['是 Windows 的设备名', "a Windows device's name"] }
  }
  return { parts }
}

// --- Reading a zip -------------------------------------------------------------

// A zip's entries from its central directory: names, sizes, and a load()
// that inflates one and checks its size and CRC. No ZIP64 (her extractor
// takes none), no encryption, names in UTF-8 or plain ASCII. Throws an
// Error with .zh/.en when the zip itself cannot be read.
function unzip(b) {
  const fail = (zh, en) => Object.assign(new Error(en), { zh, en })
  const u16 = at => { if (at + 2 > b.length) throw fail('zip 被截断了', 'the zip is cut short'); return b.readUInt16LE(at) }
  const u32 = at => { if (at + 4 > b.length) throw fail('zip 被截断了', 'the zip is cut short'); return b.readUInt32LE(at) }
  let end = -1
  for (let i = b.length - 22; i >= Math.max(0, b.length - 22 - 65535); i--) {
    if (b.readUInt32LE(i) === 0x06054b50) { end = i; break }
  }
  if (end < 0) throw fail('不是 zip（找不到目录）', 'not a zip (no central directory)')
  const count = u16(end + 10)
  const at = u32(end + 16)
  if (count === 0xffff || at === 0xffffffff) throw fail('是 ZIP64，她不收；请用普通 zip', 'ZIP64, which she does not take; use a plain zip')
  if (count > MAX_ENTRIES) throw fail(`有 ${count} 项，太多了`, `${count} entries, far too many`)
  const out = []
  let p = at
  for (let i = 0; i < count; i++) {
    if (u32(p) !== 0x02014b50) throw fail('zip 的目录坏了', "the zip's directory is broken")
    const madeBy = u16(p + 4) >> 8
    const flags = u16(p + 8)
    const method = u16(p + 10)
    const crc = u32(p + 16)
    const packed = u32(p + 20)
    const size = u32(p + 24)
    const n = u16(p + 28), e = u16(p + 30), c = u16(p + 32)
    const attrs = u32(p + 38)
    const header = u32(p + 42)
    const raw = b.subarray(p + 46, p + 46 + n)
    if (raw.length < n) throw fail('zip 被截断了', 'the zip is cut short')
    if ([packed, size, header].includes(0xffffffff)) throw fail('是 ZIP64，她不收；请用普通 zip', 'ZIP64, which she does not take; use a plain zip')
    const ascii = raw.every(x => x < 0x80)
    let name
    if (flags & 0x800 || ascii) {
      try { name = new TextDecoder('utf-8', { fatal: true }).decode(raw) } catch { throw fail('有文件名不是 UTF-8', 'a file name is not UTF-8') }
    } else {
      throw fail('有文件名不是 ASCII 也没标 UTF-8（换一个打包工具）', 'a file name is neither ASCII nor marked UTF-8 (pack it with another tool)')
    }
    // Unix modes where a Unix zip gives them; a folder also by its DOS bit.
    const mode = madeBy === 3 ? attrs >>> 16 : 0
    const isDir = /[/\\]$/.test(name) || (mode & 0xf000) === 0x4000 || (madeBy !== 3 && (attrs & 0x10) !== 0 && size === 0)
    out.push({
      name,
      size,
      packed,
      isDir,
      isLink: (mode & 0xf000) === 0xa000,
      encrypted: (flags & 1) !== 0,
      method,
      load: () => entryBytes(b, { name, raw, method, crc, packed, size, header, flags }, fail),
    })
    p += 46 + n + e + c
  }
  return out
}

function entryBytes(b, e, fail) {
  const h = e.header
  if (h + 30 > b.length || b.readUInt32LE(h) !== 0x04034b50) throw fail(`${e.name}：找不到它的本地头`, `${e.name}: no local header`)
  const n = b.readUInt16LE(h + 26), x = b.readUInt16LE(h + 28)
  // The local header's name is the directory's, or an extractor that reads
  // the local headers would see another file than the one checked.
  if (!b.subarray(h + 30, h + 30 + n).equals(e.raw)) throw fail(`${e.name}：本地头里的文件名不一样`, `${e.name}: its local header names another file`)
  if (e.flags & 1) throw fail(`${e.name}：加密了`, `${e.name}: encrypted`)
  const from = h + 30 + n + x
  const packed = b.subarray(from, from + e.packed)
  if (packed.length < e.packed) throw fail(`${e.name}：被截断了`, `${e.name}: cut short`)
  let data
  if (e.method === 0) data = Buffer.from(packed)
  else if (e.method === 8) {
    try { data = zlib.inflateRawSync(packed, { maxOutputLength: Math.max(1, e.size) }) } catch {
      throw fail(`${e.name}：解压失败，或比它说的大`, `${e.name}: does not inflate, or is bigger than it says`)
    }
  } else throw fail(`${e.name}：压缩方法 ${e.method}，只收存储和 deflate`, `${e.name}: compression method ${e.method}; only stored and deflate`)
  if (data.length !== e.size) throw fail(`${e.name}：${data.length} 字节，不是它说的 ${e.size}`, `${e.name}: ${data.length} bytes, not the ${e.size} it says`)
  if (crc32(data) !== e.crc) throw fail(`${e.name}：CRC 不对，文件坏了`, `${e.name}: wrong CRC, the file is damaged`)
  return data
}

// --- Writing a zip -------------------------------------------------------------

// A zip of [{ path, data }], the same bytes for the same files every time:
// sorted by path (as UTF-8 bytes), every entry dated 1980-01-01 00:00, no
// folders, no attributes, names marked UTF-8; deflated (level 9) where that
// is smaller, stored where not. (The same node gives the same deflate; the
// market's workflows pin node's version.)
function writeZip(entries) {
  const sorted = entries.map(e => ({ ...e, nameBytes: Buffer.from(e.path, 'utf8') })).sort((a, b) => Buffer.compare(a.nameBytes, b.nameBytes))
  const locals = []
  const centrals = []
  let offset = 0
  for (const e of sorted) {
    const data = Buffer.isBuffer(e.data) ? e.data : Buffer.from(e.data)
    const deflated = zlib.deflateRawSync(data, { level: 9 })
    const store = e.store || deflated.length >= data.length
    const body = store ? data : deflated
    const crc = crc32(data)
    const head = Buffer.alloc(30)
    head.writeUInt32LE(0x04034b50, 0)
    head.writeUInt16LE(20, 4) // version needed: 2.0
    head.writeUInt16LE(0x800, 6) // UTF-8 names
    head.writeUInt16LE(store ? 0 : 8, 8)
    head.writeUInt16LE(0, 10) // 00:00
    head.writeUInt16LE(0x21, 12) // 1980-01-01
    head.writeUInt32LE(crc, 14)
    head.writeUInt32LE(body.length, 18)
    head.writeUInt32LE(data.length, 22)
    head.writeUInt16LE(e.nameBytes.length, 26)
    head.writeUInt16LE(0, 28)
    const central = Buffer.alloc(46)
    central.writeUInt32LE(0x02014b50, 0)
    central.writeUInt16LE(20, 4) // made by: MS-DOS, 2.0 (no Unix modes)
    central.writeUInt16LE(20, 6)
    central.writeUInt16LE(0x800, 8)
    central.writeUInt16LE(store ? 0 : 8, 10)
    central.writeUInt16LE(0, 12)
    central.writeUInt16LE(0x21, 14)
    central.writeUInt32LE(crc, 16)
    central.writeUInt32LE(body.length, 20)
    central.writeUInt32LE(data.length, 24)
    central.writeUInt16LE(e.nameBytes.length, 28)
    central.writeUInt32LE(offset, 42)
    locals.push(head, e.nameBytes, body)
    centrals.push(central, e.nameBytes)
    offset += 30 + e.nameBytes.length + body.length
  }
  const dir = Buffer.concat(centrals)
  const end = Buffer.alloc(22)
  end.writeUInt32LE(0x06054b50, 0)
  end.writeUInt16LE(sorted.length, 8)
  end.writeUInt16LE(sorted.length, 10)
  end.writeUInt32LE(dir.length, 12)
  end.writeUInt32LE(offset, 16)
  return Buffer.concat([...locals, dir, end])
}

// A checked package as its zip (read() keeps the files' bytes for this).
const zipOf = r => writeZip(r.files.map(f => ({ path: f.path, data: r.contents.get(f.path) })))

// --- Reading a folder ----------------------------------------------------------

// A folder's files as zip entries would be, names joined by "/". A link
// (symbolic or a junction) is not followed: it is a problem of its own.
function folderEntries(root) {
  const out = []
  const walk = rel => {
    for (const d of fs.readdirSync(path.join(root, rel), { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : 1))) {
      const name = rel ? `${rel}/${d.name}` : d.name
      const full = path.join(root, name)
      const st = fs.lstatSync(full)
      if (st.isSymbolicLink()) out.push({ name, size: 0, isDir: false, isLink: true, load: () => Buffer.alloc(0) })
      else if (st.isDirectory()) walk(name)
      else if (st.isFile()) out.push({ name, size: st.size, isDir: false, isLink: false, load: () => fs.readFileSync(full) })
      else out.push({ name, size: 0, isDir: false, isLink: true, load: () => Buffer.alloc(0) })
    }
  }
  walk('')
  return out
}

// --- What a file's first bytes say ---------------------------------------------

// A program whatever its name: Windows' MZ (PE), Linux's ELF, macOS's
// Mach-O (32, 64, and the universal "fat" one, whose CAFEBABE Java's class
// files share).
function programOf(b) {
  if (b.length >= 2 && b[0] === 0x4d && b[1] === 0x5a) return 'Windows (MZ/PE)'
  if (b.length < 4) return null
  const m = b.readUInt32BE(0)
  if (m === 0x7f454c46) return 'Linux (ELF)'
  if (m === 0xfeedface || m === 0xcefaedfe) return 'macOS (Mach-O 32-bit)'
  if (m === 0xfeedfacf || m === 0xcffaedfe) return 'macOS (Mach-O 64-bit)'
  if (m === 0xcafebabe || m === 0xbebafeca || m === 0xcafebabf || m === 0xbfbafeca) return 'macOS universal (Mach-O fat) / Java class'
  return null
}

// Whether a file's bytes look like its type; false only when they clearly do
// not (a warning for the reviewer: a picture that is no picture, an archive
// under another name).
const at = (b, i, s) => b.length >= i + s.length && b.subarray(i, i + s.length).equals(Buffer.from(s, 'latin1'))
const LOOKS = {
  '.png': b => at(b, 0, '\x89PNG\r\n\x1a\n'),
  '.jpg': b => at(b, 0, '\xff\xd8\xff'),
  '.jpeg': b => at(b, 0, '\xff\xd8\xff'),
  '.gif': b => at(b, 0, 'GIF87a') || at(b, 0, 'GIF89a'),
  '.webp': b => at(b, 0, 'RIFF') && at(b, 8, 'WEBP'),
  '.avif': b => at(b, 4, 'ftyp'),
  '.wasm': b => at(b, 0, '\0asm'),
  '.woff': b => at(b, 0, 'wOFF'),
  '.woff2': b => at(b, 0, 'wOF2'),
  '.ttf': b => at(b, 0, '\0\x01\0\0') || at(b, 0, 'true'),
  '.otf': b => at(b, 0, 'OTTO'),
  '.ogg': b => at(b, 0, 'OggS'),
  '.wav': b => at(b, 0, 'RIFF') && at(b, 8, 'WAVE'),
  '.mp3': b => at(b, 0, 'ID3') || (b.length >= 2 && b[0] === 0xff && (b[1] & 0xe0) === 0xe0),
}
const TEXT = ['.js', '.mjs', '.json', '.html', '.htm', '.css', '.svg', '.txt', '.md']
function looksRight(ext, b) {
  if (LOOKS[ext]) return LOOKS[ext](b)
  if (TEXT.includes(ext)) {
    if (b.includes(0)) return false
    try { new TextDecoder('utf-8', { fatal: true }).decode(b); return true } catch { return false }
  }
  return true
}

// An SVG drawn in her own page (the market's list, her widget's icon) must
// not run anything nor reach out.
function svgProblem(text) {
  if (/<script/i.test(text)) return ['有 <script>', 'has a <script>']
  if (/\son[a-z]+\s*=/i.test(text)) return ['有 on… 事件属性', 'has an on… event attribute']
  if (/<foreignObject/i.test(text)) return ['有 <foreignObject>', 'has a <foreignObject>']
  if (/<!ENTITY/i.test(text)) return ['有 <!ENTITY>', 'has an <!ENTITY>']
  if (/javascript:/i.test(text)) return ['有 javascript: 链接', 'has a javascript: link']
  if (/(?:href|src)\s*=\s*["']\s*(?!#|data:image\/)/i.test(text)) return ['引用了外面的东西（只能用 #id 或 data:image）', 'refers to something outside it (only #id or data:image)']
  if (/url\(\s*["']?\s*(?!#)/i.test(text)) return ['样式里引用了外面的东西', 'its style refers to something outside it']
  return null
}

// --- The manifest --------------------------------------------------------------

// A `net` host, as docs/PLUGINS.md takes one: an exact name or *.domain; no
// port, no IP literal, nothing that only reaches this computer or its
// network. Why not, or null.
function hostProblem(h) {
  if (typeof h !== 'string' || !h) return ['是空的', 'is empty']
  if (h.includes('://') || h.includes('/')) return ['是网址，只写主机名（api.example.com）', 'is a URL; write the host only (api.example.com)']
  if (h.includes('[') || (h.match(/:/g) || []).length > 1) return ['是 IP 地址（IPv6），只能写域名', 'is an IP address (IPv6); names only']
  if (h.includes(':')) return ['带端口；只能用 80 和 443，不写端口', 'has a port; only 80 and 443, with no port written']
  if (h !== h.toLowerCase()) return ['要小写', 'must be lower case']
  if (/[^\x00-\x7f]/.test(h)) return ['有非 ASCII 字符，用 punycode（xn--…）', 'is not ASCII; use punycode (xn--…)']
  let name = h
  const wild = h.startsWith('*.')
  if (wild) name = h.slice(2)
  if (name.includes('*')) return ['通配符只能写在最前面：*.example.com', 'a wildcard only at the start: *.example.com']
  const labels = name.split('.')
  const last = labels[labels.length - 1]
  // As a URL parser takes them: a last label of digits (127.1) or 0x… is an
  // IPv4 address.
  if (/^\d+$/.test(last) || /^0x[0-9a-f]*$/.test(last)) return ['是 IP 地址，只能写域名', 'is an IP address; names only']
  if (labels.length < 2) return [name === 'localhost' ? '是 localhost，只能访问外面的网站' : '不是完整的域名', name === 'localhost' ? 'is localhost; only sites out there' : 'is not a full domain name']
  if (LOCAL_SUFFIXES.some(s => name === s || name.endsWith(`.${s}`))) return ['只通到这台电脑或局域网', 'reaches only this computer or its network']
  if (name.length > 253 || !labels.every(l => /^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/.test(l))) return ['不是有效的域名', 'is not a valid domain name']
  return null
}

const LANGS = ['en', 'zh']
// A name, a description, a label: a string, or { en, zh } with en (the
// fallback). Its values, or a problem.
function textOf(v, what, file, errors, warnings, max) {
  const [zhWhat, enWhat] = what
  let values
  if (typeof v === 'string') values = { en: v }
  else if (v && typeof v === 'object' && !Array.isArray(v)) {
    values = v
    if (typeof v.en !== 'string') {
      errors.push(problem('text', file, `${zhWhat}要有英文（en），英文是默认的`, `${enWhat} needs English (en), the fallback`))
      return null
    }
    const other = Object.keys(v).filter(k => !LANGS.includes(k))
    if (other.length) warnings.push(problem('text-lang', file, `${zhWhat}里的 ${other.join(', ')} 用不上，只有 en 和 zh`, `${enWhat}: ${other.join(', ')} not used; only en and zh`))
  } else {
    errors.push(problem('text', file, `${zhWhat}要写成文字，或 { "en": …, "zh": … }`, `${enWhat} must be a string, or { "en": …, "zh": … }`))
    return null
  }
  let ok = true
  for (const lang of LANGS) {
    if (!(lang in values)) continue
    const s = values[lang]
    if (typeof s !== 'string' || !s.trim()) {
      errors.push(problem('text', file, `${zhWhat}（${lang}）是空的`, `${enWhat} (${lang}) is empty`))
      ok = false
    } else if (max && chars(s) > max.n) {
      const list = max.warn ? warnings : errors
      list.push(problem(max.code, file, `${zhWhat}（${lang}）有 ${chars(s)} 个字，${max.warn ? '最好' : '最多'} ${max.n} 个${max.warn ? '，长的会被截断' : ''}`, `${enWhat} (${lang}) is ${chars(s)} characters; ${max.warn ? 'keep it to' : 'at most'} ${max.n}${max.warn ? ', or it is cut' : ''}`))
      if (!max.warn) ok = false
    }
  }
  return ok ? values : null
}

// A path the manifest names (main, view, an icon): plain and inside.
function refProblem(v) {
  if (typeof v !== 'string' || !v) return ['要写文件名', 'must name a file']
  if (v.includes('\\')) return ['要用 / 分隔', 'must use / between folders']
  if (v.startsWith('./')) return [`写 ${v.slice(2)}，不要 ./`, `write ${v.slice(2)}, without ./`]
  const r = partsOf(v)
  return r.why || null
}

function checkManifest(m, files, kind, errors, warnings) {
  const F = 'manifest.json'
  const has = p => files.has(p)
  for (const k of Object.keys(m)) {
    if (!FIELDS.includes(k)) warnings.push(problem('field-unknown', F, `不认识的字段 "${k}"，会被忽略`, `unknown field "${k}", ignored`))
  }
  // id
  if (typeof m.id !== 'string' || !ID.test(m.id)) {
    errors.push(problem('id', F, 'id 要像 rain-alert：小写字母开头，只有小写字母、数字和 -，2 到 40 个字', 'id must look like rain-alert: a lower-case letter first, then lower-case letters, digits and -, 2 to 40 in all'))
  } else if (isReserved(m.id)) {
    errors.push(problem('id-reserved', F, `id "${m.id}" 是她内置的，换一个`, `id "${m.id}" is one of her own; pick another`))
  }
  if (!kind) errors.push(problem('kind', F, 'kind 要是 plugin、pet 或 wallpaper', 'kind must be plugin, pet or wallpaper'))
  if (m.name === undefined) errors.push(problem('name', F, '没有 name', 'no name'))
  else textOf(m.name, ['name', 'name'], F, errors, warnings, { n: 24, code: 'name' })
  if (m.description === undefined) errors.push(problem('description', F, '没有 description', 'no description'))
  else textOf(m.description, ['description', 'description'], F, errors, warnings, { n: 60, code: 'description-long', warn: true })
  if (typeof m.version !== 'string' || !VERSION.test(m.version)) errors.push(problem('version', F, 'version 要写成 x.y.z，比如 1.2.0', 'version must be x.y.z, like 1.2.0'))
  if (typeof m.author !== 'string' || !m.author.trim()) errors.push(problem('author', F, '没有 author', 'no author'))
  else if (chars(m.author) > 40) errors.push(problem('author', F, `author 有 ${chars(m.author)} 个字，最多 40 个`, `author is ${chars(m.author)} characters; at most 40`))
  if (m.homepage !== undefined) {
    let url = null
    try { url = typeof m.homepage === 'string' ? new URL(m.homepage) : null } catch {}
    if (!url || !['https:', 'http:'].includes(url.protocol)) errors.push(problem('homepage', F, 'homepage 要是 https:// 网址', 'homepage must be an https:// URL'))
    else if (url.protocol === 'http:') warnings.push(problem('homepage-http', F, 'homepage 最好用 https://', 'homepage had better be https://'))
  }
  if (m.api !== undefined || kind === 'plugin') {
    if (!Number.isInteger(m.api) || m.api < 1) errors.push(problem('api', F, 'api 要写 1', 'api must be 1'))
    else if (m.api > API) errors.push(problem('api', F, `api ${m.api} 比这个检查认识的（${API}）新`, `api ${m.api} is newer than this check knows (${API})`))
  }

  if (!kind) return
  if (kind !== 'plugin') {
    for (const k of ['main', 'view', 'settings', 'permissions']) {
      if (m[k] !== undefined) errors.push(problem('pack-field', F, `资源包没有代码，不能有 ${k}`, `a pack has no code, so no ${k}`))
    }
    for (const k of ['widget', 'category']) {
      if (m[k] !== undefined) warnings.push(problem('pack-field-ignored', F, `资源包用不上 ${k}，会被忽略`, `a pack does not use ${k}; ignored`))
    }
    return
  }

  // A plugin's code, view and widget.
  const ref = (k, ext, need) => {
    if (m[k] === undefined) {
      if (need) errors.push(problem(k, F, `插件要有 ${k}（一个 ${ext} 文件）`, `a plugin needs ${k} (a ${ext} file)`))
      return
    }
    const why = refProblem(m[k])
    if (why) errors.push(problem(k, F, `${k} ${why[0]}`, `${k} ${why[1]}`))
    else if (!m[k].endsWith(ext)) errors.push(problem(k, F, `${k} 要是 ${ext} 文件`, `${k} must be a ${ext} file`))
    else if (!has(m[k])) errors.push(problem(k, F, `${k} 写的 ${m[k]} 不在包里`, `${k} names ${m[k]}, which is not in the package`))
  }
  ref('main', '.js', true)
  ref('view', '.html', false)
  if (m.category === undefined) errors.push(problem('category', F, '没有 category（system、productivity 或 info）', 'no category (system, productivity or info)'))
  else if (!CATEGORIES.includes(m.category)) errors.push(problem('category', F, 'category 要是 system、productivity 或 info', 'category must be system, productivity or info'))

  const w = m.widget === undefined ? {} : m.widget
  if (!w || typeof w !== 'object' || Array.isArray(w)) errors.push(problem('widget', F, 'widget 要是一个对象', 'widget must be an object'))
  else {
    for (const k of Object.keys(w)) {
      if (!['sizes', 'color', 'icon', 'sample'].includes(k)) warnings.push(problem('field-unknown', F, `widget 里不认识的 "${k}"，会被忽略`, `unknown widget field "${k}", ignored`))
    }
    if (w.sample !== undefined) checkSample(w.sample, errors, warnings)
    let sizes = ['s', 'm']
    if (w.sizes !== undefined) {
      if (!Array.isArray(w.sizes) || !w.sizes.length || !w.sizes.every(s => SIZES.includes(s)) || new Set(w.sizes).size !== w.sizes.length) {
        errors.push(problem('sizes', F, 'widget.sizes 要是 s、m、l 里的几个，不重复', 'widget.sizes must be some of s, m, l, each once'))
      } else sizes = w.sizes
    }
    if (m.view !== undefined && !sizes.includes('m') && !sizes.includes('l')) {
      warnings.push(problem('view-sizes', F, '有 view，但 widget.sizes 里没有 m 或 l：自绘界面只画在中、大档卡片上', 'a view, but widget.sizes has no m or l: the view is drawn on M and L cards only'))
    }
    if (w.color !== undefined && (typeof w.color !== 'string' || !COLOR.test(w.color))) errors.push(problem('color', F, 'widget.color 要写成 #rgb 或 #rrggbb', 'widget.color must be #rgb or #rrggbb'))
    if (w.icon === undefined) {
      const svg = has('icon.svg') ? ['；包里有 icon.svg，要用它就写 "icon": "icon.svg"', '; to use the icon.svg there, write "icon": "icon.svg"'] : ['', '']
      warnings.push(problem('no-icon', F, `没有图标（widget.icon）：会画名字的第一个字${svg[0]}`, `no icon (widget.icon): the first letter of the name is drawn${svg[1]}`))
    }
    else if (typeof w.icon !== 'string' || !w.icon) errors.push(problem('icon', F, 'widget.icon 要写图标名或 .svg 文件', 'widget.icon must be an icon name or an .svg file'))
    else if (w.icon.toLowerCase().endsWith('.svg')) {
      const why = refProblem(w.icon)
      if (why) errors.push(problem('icon', F, `widget.icon ${why[0]}`, `widget.icon ${why[1]}`))
      else if (!has(w.icon)) errors.push(problem('icon', F, `widget.icon 写的 ${w.icon} 不在包里`, `widget.icon names ${w.icon}, which is not in the package`))
    } else if (!ICONS.includes(w.icon)) {
      warnings.push(problem('icon-name', F, `"${w.icon}" 不是她的图标，会画名字的第一个字（她的：${ICONS.join(' ')}）`, `"${w.icon}" is not one of her icons; the first letter of the name is drawn (hers: ${ICONS.join(' ')})`))
    }
  }

  if (m.settings !== undefined) checkSettings(m.settings, errors, warnings)
  if (m.permissions !== undefined) checkPermissions(m.permissions, errors, warnings)
}

function checkSettings(list, errors, warnings) {
  const F = 'manifest.json'
  if (!Array.isArray(list)) {
    errors.push(problem('settings', F, 'settings 要是一个列表', 'settings must be a list'))
    return
  }
  const seen = new Set()
  list.forEach((s, i) => {
    const at = `settings[${i}]`
    if (!s || typeof s !== 'object' || Array.isArray(s)) {
      errors.push(problem('settings', F, `${at} 要是一个对象`, `${at} must be an object`))
      return
    }
    const known = ['key', 'type', 'label', 'hint', 'required', 'default', 'options', 'optionLabels']
    for (const k of Object.keys(s)) {
      if (!known.includes(k)) warnings.push(problem('field-unknown', F, `${at} 里不认识的 "${k}"，会被忽略`, `${at}: unknown field "${k}", ignored`))
    }
    if (typeof s.key !== 'string' || !KEY.test(s.key)) errors.push(problem('settings', F, `${at}.key 要像 City：字母开头，只有字母和数字，最多 32 个`, `${at}.key must look like City: a letter first, letters and digits only, at most 32`))
    else if (seen.has(s.key.toLowerCase())) errors.push(problem('settings', F, `${at}.key "${s.key}" 重复了`, `${at}.key "${s.key}" is there twice`))
    else seen.add(s.key.toLowerCase())
    if (!SETTING_TYPES.includes(s.type)) {
      errors.push(problem('settings', F, `${at}.type 要是 text、number、flag 或 choice`, `${at}.type must be text, number, flag or choice`))
      return
    }
    if (s.label === undefined) errors.push(problem('settings', F, `${at} 没有 label`, `${at} has no label`))
    else textOf(s.label, [`${at}.label`, `${at}.label`], F, errors, warnings)
    if (s.hint !== undefined) textOf(s.hint, [`${at}.hint`, `${at}.hint`], F, errors, warnings)
    if (s.required !== undefined && typeof s.required !== 'boolean') errors.push(problem('settings', F, `${at}.required 要是 true 或 false`, `${at}.required must be true or false`))
    if (s.type === 'choice') {
      const ok = Array.isArray(s.options) && s.options.length > 0 && s.options.every(o => typeof o === 'string' && o) && new Set(s.options).size === s.options.length
      if (!ok) errors.push(problem('settings', F, `${at}.options 要是不重复的文字列表`, `${at}.options must be a list of different strings`))
      if (s.optionLabels !== undefined) {
        if (!s.optionLabels || typeof s.optionLabels !== 'object' || Array.isArray(s.optionLabels)) errors.push(problem('settings', F, `${at}.optionLabels 要是一个对象`, `${at}.optionLabels must be an object`))
        else {
          for (const [o, label] of Object.entries(s.optionLabels)) {
            if (ok && !s.options.includes(o)) errors.push(problem('settings', F, `${at}.optionLabels 里的 "${o}" 不在 options 里`, `${at}.optionLabels: "${o}" is not one of its options`))
            textOf(label, [`${at}.optionLabels.${o}`, `${at}.optionLabels.${o}`], F, errors, warnings)
          }
        }
      }
      if (s.default !== undefined && ok && !s.options.includes(s.default)) errors.push(problem('settings', F, `${at}.default 要是 options 里的一个`, `${at}.default must be one of its options`))
    } else {
      for (const k of ['options', 'optionLabels']) {
        if (s[k] !== undefined) warnings.push(problem('field-unknown', F, `${at}.${k} 只有 choice 用得上，会被忽略`, `${at}.${k} is for choice only; ignored`))
      }
      const want = { text: 'string', number: 'number', flag: 'boolean' }[s.type]
      if (s.default !== undefined && (typeof s.default !== want || (want === 'number' && !Number.isFinite(s.default)))) {
        errors.push(problem('settings', F, `${at}.default 要是 ${want}`, `${at}.default must be a ${want}`))
      }
    }
  })
}

function checkPermissions(p, errors, warnings) {
  const F = 'manifest.json'
  if (!p || typeof p !== 'object' || Array.isArray(p)) {
    errors.push(problem('permissions', F, 'permissions 要是一个对象', 'permissions must be an object'))
    return
  }
  for (const [k, v] of Object.entries(p)) {
    if (k === 'net') {
      if (!Array.isArray(v)) {
        errors.push(problem('permissions', F, 'permissions.net 要是主机名的列表', 'permissions.net must be a list of hosts'))
        continue
      }
      if (new Set(v).size !== v.length) warnings.push(problem('net-twice', F, 'permissions.net 里有重复的', 'permissions.net has a host twice'))
      for (const h of v) {
        const why = hostProblem(h)
        if (why) errors.push(problem('net-host', F, `permissions.net 的 "${h}" ${why[0]}`, `permissions.net "${h}" ${why[1]}`))
      }
    } else if (FLAGS.includes(k)) {
      if (typeof v !== 'boolean') errors.push(problem('permissions', F, `permissions.${k} 要是 true 或 false`, `permissions.${k} must be true or false`))
    } else {
      errors.push(problem('permissions', F, `不认识的权限 "${k}"（有 net、agents、hardware、notify）`, `unknown permission "${k}" (there are net, agents, hardware, notify)`))
    }
  }
}

// widget.sample: what its widget would say, in the POST /widget format
// (label, value, color, icon, private, detail), for the market's page to
// draw its cards with her own templates before it is installed. Checked as
// she takes a widget (src-tauri/src/widgets.rs made, detail_of): a field of
// the wrong kind is an error; what she would drop or cut is said; 4 KB of
// JSON at most.
const SAMPLE_FIELDS = ['label', 'value', 'color', 'icon', 'private', 'detail']
const DETAIL_FIELDS = ['sizes', 'series', 'rows', 'extra']
const ROW_NAMES = ['from', 'code', 'name', 'when', 'price', 'change', 'value', 'unit', 'kind', 'icon']
const ROW_SUBJECTS = ['subject', 'text']
function checkSample(s, errors, warnings) {
  const F = 'manifest.json'
  const err = (zh, en) => errors.push(problem('sample', F, `widget.sample${zh}`, `widget.sample${en}`))
  const cut = (zh, en) => warnings.push(problem('sample-cut', F, `widget.sample${zh}`, `widget.sample${en}`))
  if (!s || typeof s !== 'object' || Array.isArray(s)) return err(' 要是一个对象，和 POST /widget 的格式一样', ' must be an object, in the POST /widget format')
  const size = Buffer.byteLength(JSON.stringify(s))
  if (size > SAMPLE_MAX) err(` 有 ${kb(size)}，最多 4 KB`, ` is ${kb(size)}; at most 4 KB`)
  for (const k of Object.keys(s)) {
    if (!SAMPLE_FIELDS.includes(k)) cut(` 里的 "${k}" 用不上，会被去掉`, `: "${k}" is not used, and is dropped`)
  }
  const words = (v, at, max) => {
    if (v === undefined) return
    if (typeof v !== 'string') return err(`.${at} 要是文字`, `.${at} must be a string`)
    if (chars(v.trim()) > max) cut(`.${at} 超过 ${max} 个字，会被截断`, `.${at} is over ${max} characters, and is cut`)
  }
  words(s.label, 'label', 40)
  words(s.value, 'value', 24)
  if (s.color !== undefined && (typeof s.color !== 'string' || !COLOR.test(s.color))) err('.color 要写成 #rgb 或 #rrggbb', '.color must be #rgb or #rrggbb')
  if (s.icon !== undefined && !ICONS.includes(s.icon)) cut(`.icon "${s.icon}" 不是她的图标，会用 widget.icon`, `.icon "${s.icon}" is not one of her icons; widget.icon is used`)
  if (s.private !== undefined && typeof s.private !== 'boolean') err('.private 要是 true 或 false', '.private must be true or false')
  const d = s.detail
  if (d === undefined) return
  if (typeof d === 'string') return words(d, 'detail', 24)
  if (!d || typeof d !== 'object' || Array.isArray(d)) return err('.detail 要是文字，或 { series, rows, extra }', '.detail must be a string, or { series, rows, extra }')
  for (const k of Object.keys(d)) {
    if (!DETAIL_FIELDS.includes(k)) cut(`.detail 里的 "${k}" 用不上，会被去掉`, `.detail: "${k}" is not used, and is dropped`)
  }
  const points = (v, at) => {
    if (!Array.isArray(v) || !v.length || !v.every(n => typeof n === 'number' && Number.isFinite(n))) return err(`.${at} 要是数字的列表`, `.${at} must be a list of numbers`)
    if (v.length > 120) cut(`.${at} 有 ${v.length} 个，只留最后 120 个`, `.${at} has ${v.length}; the last 120 are kept`)
  }
  if (d.series !== undefined) {
    if (!d.series || typeof d.series !== 'object' || Array.isArray(d.series)) err('.detail.series 要是 { points, unit, hi, lo }', '.detail.series must be { points, unit, hi, lo }')
    else {
      points(d.series.points, 'detail.series.points')
      words(d.series.unit, 'detail.series.unit', 16)
      for (const k of ['hi', 'lo']) if (d.series[k] !== undefined && !Number.isFinite(d.series[k])) err(`.detail.series.${k} 要是数字`, `.detail.series.${k} must be a number`)
    }
  }
  if (d.rows !== undefined) {
    if (!Array.isArray(d.rows)) err('.detail.rows 要是一个列表', '.detail.rows must be a list')
    else {
      if (d.rows.length > 12) cut(`.detail.rows 有 ${d.rows.length} 行，只留前 12 行`, `.detail.rows has ${d.rows.length}; the first 12 are kept`)
      d.rows.forEach((r, i) => {
        const at = `detail.rows[${i}]`
        if (!r || typeof r !== 'object' || Array.isArray(r)) return err(`.${at} 要是一个对象`, `.${at} must be an object`)
        for (const [k, v] of Object.entries(r)) {
          if (ROW_NAMES.includes(k)) typeof v === 'number' || words(v, `${at}.${k}`, 16)
          else if (ROW_SUBJECTS.includes(k)) words(v, `${at}.${k}`, 60)
          else if (k === 'at') !(Number.isFinite(v) && v >= 0) && err(`.${at}.at 要是时间（毫秒）`, `.${at}.at must be a time (ms)`)
          else if (k === 'points') points(v, `${at}.points`)
          else cut(`.${at} 里的 "${k}" 用不上，会被去掉`, `.${at}: "${k}" is not used, and is dropped`)
        }
      })
    }
  }
  words(d.extra, 'detail.extra', 24)
}

// The permissions a plugin asked for, as the market lists them: what is
// not asked for left out.
function permissionsOf(m) {
  const p = m.permissions && typeof m.permissions === 'object' ? m.permissions : {}
  const out = {}
  if (Array.isArray(p.net) && p.net.length) out.net = [...new Set(p.net)]
  for (const k of FLAGS) if (p[k] === true) out[k] = true
  return out
}

// The icon the market shows, the one her widget draws: widget.icon when it
// is an .svg; as a data: URI when 8 KB or less.
function iconPath(r) {
  const icon = r.manifest && r.manifest.widget && r.manifest.widget.icon
  return typeof icon === 'string' && icon.toLowerCase().endsWith('.svg') && r.contents.has(icon) ? icon : null
}
function iconOf(r) {
  const p = iconPath(r)
  if (!p) return null
  const b = r.contents.get(p)
  return b.length <= ICON_MAX ? `data:image/svg+xml;base64,${b.toString('base64')}` : null
}
// One of her own icons by name, when widget.icon is one.
function glyphOf(r) {
  const icon = r.kind === 'plugin' && r.manifest && r.manifest.widget && r.manifest.widget.icon
  return typeof icon === 'string' && ICONS.includes(icon) ? icon : null
}
// widget.sample as the manifest has it (checked: checkSample).
function sampleOf(r) {
  const s = r.kind === 'plugin' && r.manifest && r.manifest.widget && r.manifest.widget.sample
  return s && typeof s === 'object' && !Array.isArray(s) ? s : null
}
// A pack's picture for the market: preview.webp, .png or .jpg at its top,
// 64 KB or less, as a data: URI. Neither a wallpaper nor a pet's.
function previewPath(r) {
  return r.kind === 'pet' || r.kind === 'wallpaper' ? PREVIEWS.find(p => r.contents.has(p)) || null : null
}
function previewOf(r) {
  const p = previewPath(r)
  if (!p) return null
  const b = r.contents.get(p)
  return b.length <= PREVIEW_MAX ? `data:${PREVIEW_TYPES[extOf(p)]};base64,${b.toString('base64')}` : null
}

// --- The whole package ---------------------------------------------------------

// Reads and checks a package: a folder, or a .zip. opts.market: the folder's
// name must be its id (packages/<id>/ in the market).
function read(source, opts = {}) {
  const errors = []
  const warnings = []
  const result = { source, manifest: null, kind: null, files: [], errors, warnings, packed: 0, unpacked: 0 }
  const contents = new Map()
  Object.defineProperty(result, 'contents', { value: contents, enumerable: false })
  let entries
  let zipped = null
  let st
  try { st = fs.statSync(source) } catch {
    errors.push(problem('unreadable', '', '找不到它', 'not found'))
    return result
  }
  if (st.isDirectory()) entries = folderEntries(source)
  else {
    // Bigger than any kind may be: not read at all.
    if (st.size > LIMITS.pet.packed) {
      errors.push(problem('too-large-packed', '', `zip 有 ${kb(st.size)}，最多 ${kb(LIMITS.pet.packed)}`, `the zip is ${kb(st.size)}; at most ${kb(LIMITS.pet.packed)}`))
      return result
    }
    zipped = fs.readFileSync(source)
    try { entries = unzip(zipped) } catch (e) {
      errors.push(problem('zip-bad', '', e.zh || '读不了这个 zip', e.en || e.message))
      return result
    }
  }

  // Names inside the package; one top folder holding everything taken away,
  // as her extractor does.
  const named = []
  for (const e of entries) {
    const { parts, why } = partsOf(e.name)
    if (why) {
      errors.push(problem('path-unsafe', e.name, `文件名不安全：${why[0]}`, `unsafe file name: ${why[1]}`))
      continue
    }
    named.push({ e, parts })
  }
  const top = named.length ? named[0].parts[0] : null
  const strip = top !== null && named.every(({ e, parts }) => parts[0] === top && (parts.length > 1 || e.isDir)) ? 1 : 0
  const byPath = new Map()
  const lower = new Map()
  for (const { e, parts } of named) {
    if (e.isDir) continue
    const p = parts.slice(strip).join('/')
    if (!p) continue
    if (e.isLink) {
      errors.push(problem('symlink', p, '是链接（symlink 或 junction），不收', 'a link (symlink or junction), not taken'))
      continue
    }
    if (e.encrypted) {
      errors.push(problem('zip-encrypted', p, '加密了', 'encrypted'))
      continue
    }
    const l = p.toLowerCase()
    if (lower.has(l)) {
      errors.push(problem('path-duplicate', p, `和 ${lower.get(l)} 重名（Windows 不分大小写）`, `the same name as ${lower.get(l)} (Windows ignores case)`))
      continue
    }
    lower.set(l, p)
    byPath.set(p, e)
  }
  const paths = [...byPath.keys()].sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)))
  // Each file's bytes read once (null when it cannot be).
  const tried = new Set()
  const load = p => {
    if (!tried.has(p)) {
      tried.add(p)
      try { contents.set(p, byPath.get(p).load()) } catch (err) {
        errors.push(problem('zip-bad', p, err.zh || `读不了：${err.message}`, err.en || `cannot be read: ${err.message}`))
      }
    }
    return contents.get(p) || null
  }

  // The manifest first: it says the kind, and the kind the rules.
  let manifest = null
  if (!byPath.has('manifest.json')) errors.push(problem('manifest-missing', '', '没有 manifest.json（要在最外层，或唯一的一个文件夹里）', 'no manifest.json (at the top, or in the one folder that holds everything)'))
  else if (byPath.get('manifest.json').size > 256 * 1024) errors.push(problem('manifest-json', 'manifest.json', '大得不像 manifest（超过 256 KB）', 'far too big for a manifest (over 256 KB)'))
  else {
    try {
      const b = load('manifest.json')
      if (b) {
        if (b[0] === 0xef && b[1] === 0xbb && b[2] === 0xbf) errors.push(problem('manifest-json', 'manifest.json', '开头有 BOM：存成不带 BOM 的 UTF-8', 'starts with a BOM: save it as UTF-8 without one'))
        else {
          const m = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(b))
          if (!m || typeof m !== 'object' || Array.isArray(m)) throw new Error('not an object')
          manifest = m
        }
      }
    } catch (e) {
      errors.push(problem('manifest-json', 'manifest.json', `不是有效的 JSON：${e.message}`, `not valid JSON: ${e.message}`))
    }
  }
  result.manifest = manifest
  const kind = manifest && KINDS.includes(manifest.kind) ? manifest.kind : null
  result.kind = kind
  const rules = kind || 'plugin'
  const limit = LIMITS[rules]
  const types = TYPES[rules]

  let unpacked = 0
  for (const p of paths) unpacked += byPath.get(p).size
  result.unpacked = unpacked
  result.files = paths.map(p => ({ path: p, size: byPath.get(p).size }))
  const what = { plugin: '插件', pet: '宠物包', wallpaper: '壁纸包' }[rules]
  if (paths.length > MAX_FILES) errors.push(problem('too-many-files', '', `有 ${paths.length} 个文件，最多 ${MAX_FILES} 个`, `${paths.length} files; at most ${MAX_FILES}`))
  const tooBig = unpacked > limit.unpacked
  if (tooBig) errors.push(problem('too-large-unpacked', '', `解开有 ${kb(unpacked)}，${what}最多 ${kb(limit.unpacked)}`, `${kb(unpacked)} unpacked; a ${rules === 'plugin' ? 'plugin' : 'pack'} may have ${kb(limit.unpacked)}`))
  if (zipped && zipped.length > limit.packed) errors.push(problem('too-large-packed', '', `zip 有 ${kb(zipped.length)}，${what}最多 ${kb(limit.packed)}`, `the zip is ${kb(zipped.length)}; a ${rules === 'plugin' ? 'plugin' : 'pack'} may have ${kb(limit.packed)}`))

  // Each file: its type by its name, then what it is by its first bytes
  // (not unpacked when the whole is too big already).
  let loaded = !tooBig
  for (const p of paths) {
    const ext = extOf(p)
    if (!types.includes(ext)) {
      const pack = rules !== 'plugin'
      errors.push(problem('file-type', p, `${ext ? `${ext} 文件` : '没有扩展名的文件'}不收${pack ? `（${what}只收 ${types.join(' ')}）` : ''}`, `${ext || 'a file with no extension'} is not taken${pack ? ` (a ${rules} pack takes ${types.join(' ')})` : ''}`))
    }
    if (tooBig) continue
    const b = load(p)
    if (!b) { loaded = false; continue }
    const prog = programOf(b)
    if (prog) errors.push(problem('program', p, `是可执行程序（${prog}），市场不收程序，从文件安装也一样`, `a program (${prog}); programs are never taken, from the market or from a file`))
    else if (types.includes(ext) && !looksRight(ext, b)) warnings.push(problem('content-mismatch', p, `内容看起来不是 ${ext} 文件`, `its bytes do not look like a ${ext} file`))
  }
  if (manifest) {
    checkManifest(manifest, byPath, kind, errors, warnings)
    if (opts.market && !zipped && path.basename(path.resolve(source)) !== manifest.id) {
      errors.push(problem('folder-id', '', `文件夹名要和 id 一样：packages/${manifest.id}/`, `the folder must be named after its id: packages/${manifest.id}/`))
    }
  }
  if (loaded && kind) checkKind(result, byPath, contents, errors, warnings)

  // A folder's zipped size is its zip's, as build-index.js makes it.
  if (zipped) result.packed = zipped.length
  else if (loaded) {
    result.packed = zipOf(result).length
    if (result.packed > limit.packed) errors.push(problem('too-large-packed', '', `打包后有 ${kb(result.packed)}，${what}最多 ${kb(limit.packed)}`, `${kb(result.packed)} zipped; a ${rules === 'plugin' ? 'plugin' : 'pack'} may have ${kb(limit.packed)}`))
  }
  return result
}

// What each kind needs beyond the manifest.
function checkKind(r, byPath, contents, errors, warnings) {
  if (r.kind === 'plugin') {
    const p = iconPath(r)
    if (p) {
      const b = contents.get(p)
      const why = svgProblem(b.toString('utf8'))
      if (why) errors.push(problem('icon-unsafe', p, `图标 ${why[0]}：它画在她自己的页面里`, `the icon ${why[1]}: it is drawn in her own page`))
      else if (b.length > ICON_MAX) warnings.push(problem('icon-large', p, `图标有 ${kb(b.length)}，超过 8 KB，市场列表里不显示`, `the icon is ${kb(b.length)}, over 8 KB: the market's list will not show it`))
    }
  } else if (r.kind === 'pet') {
    if (!byPath.has('pet.json')) errors.push(problem('pet-json', '', '宠物包要有 pet.json', 'a pet pack needs pet.json'))
    else {
      try {
        const pet = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(contents.get('pet.json')))
        if (!pet || typeof pet !== 'object' || Array.isArray(pet)) throw new Error('not an object')
        if (typeof pet.displayName !== 'string' || !pet.displayName) warnings.push(problem('pet-name', 'pet.json', '没有 displayName：她会显示 id', 'no displayName: she shows the id'))
        if (pet.spritesheetPath !== undefined && pet.spritesheetPath !== 'spritesheet.webp') warnings.push(problem('pet-sheet-path', 'pet.json', 'spritesheetPath 不是 spritesheet.webp：她只读 spritesheet.webp', 'spritesheetPath is not spritesheet.webp: she reads spritesheet.webp only'))
      } catch (e) {
        errors.push(problem('pet-json', 'pet.json', `不是有效的 JSON：${e.message}`, `not valid JSON: ${e.message}`))
      }
    }
    if (!byPath.has('spritesheet.webp')) errors.push(problem('pet-sheet', '', '宠物包要有 spritesheet.webp', 'a pet pack needs spritesheet.webp'))
  }
  // A pack's picture for the market: too big, not shown.
  const preview = previewPath(r)
  if (preview && contents.get(preview).length > PREVIEW_MAX) {
    warnings.push(problem('preview-large', preview, `预览图有 ${kb(contents.get(preview).length)}，超过 64 KB，市场里不显示`, `the preview is ${kb(contents.get(preview).length)}, over 64 KB: the market will not show it`))
  }
  if (r.kind === 'wallpaper') {
    const images = r.files.filter(f => WALLPAPER_IMAGES.includes(extOf(f.path)) && !PREVIEWS.includes(f.path))
    if (!images.length) errors.push(problem('wallpaper-empty', '', '壁纸包里没有图片（.jpg .jpeg .png .webp）', 'a wallpaper pack with no pictures (.jpg .jpeg .png .webp)'))
    const deep = images.filter(f => f.path.includes('/'))
    if (deep.length) warnings.push(problem('wallpaper-folder', deep[0].path, `${deep.length} 张图片在子文件夹里：她只用最外层的图片`, `${deep.length} pictures in sub-folders: she uses the top folder's only`))
  }
}

// --- Versions and sameness -----------------------------------------------------

const versionParts = v => String(v).split('.').map(Number)
function compareVersions(a, b) {
  const x = versionParts(a), y = versionParts(b)
  for (let i = 0; i < 3; i++) if ((x[i] || 0) !== (y[i] || 0)) return (x[i] || 0) < (y[i] || 0) ? -1 : 1
  return 0
}
// Whether two read packages hold the same files, byte for byte.
function sameContent(a, b) {
  if (a.files.length !== b.files.length) return false
  return a.files.every((f, i) => f.path === b.files[i].path && a.contents.get(f.path) && b.contents.get(f.path) && a.contents.get(f.path).equals(b.contents.get(f.path)))
}

module.exports = {
  read, unzip, writeZip, zipOf, iconOf, glyphOf, sampleOf, previewOf, permissionsOf, hostProblem, programOf, partsOf, compareVersions, sameContent, sha256, crc32,
  API, LIMITS, MAX_FILES, TYPES, RESERVED, ICONS, isReserved, kb,
}
