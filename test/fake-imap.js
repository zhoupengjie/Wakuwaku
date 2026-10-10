// A small IMAP server on 127.0.0.1 for trying the pet's mail without a real
// account: no TLS (set the account up with security "plain"), one user, an
// inbox read from a folder of .eml files, and a Sent folder that is empty
// until a letter is put in it (APPEND).
//
//   node test/fake-imap.js [--port 14300] [--dir test/fixtures/mail]
//                          [--password test] [--no-idle] [--login-only]
//                          [--want-id] [--sent <folder>] [--unmarked-sent]
//                          [--log]
//
// The inbox is the folder's .eml files in name order (UID 1, 2, …); a name
// with "-seen" in it starts read, one with "-flagged" starred. A file added
// while it runs is new mail:
// a client in IDLE is told at once (* n EXISTS), others at their next
// command. --want-id refuses to open a folder before ID, as 163 does;
// --login-only offers LOGIN but no AUTHENTICATE PLAIN; --no-idle leaves
// IDLE out, so the pet polls. --sent writes each letter put into a folder
// there too (sent-1.eml…); --unmarked-sent lists Sent without \Sent, as
// servers without SPECIAL-USE do.
//
// Only what the pet asks for is understood: CAPABILITY, NOOP, LOGOUT, ID,
// LOGIN, AUTHENTICATE PLAIN, LIST, SELECT, EXAMINE, CLOSE, UNSELECT, APPEND,
// [UID] SEARCH (ALL, SEEN, UNSEEN, FLAGGED, UNFLAGGED, UID set, a sequence
// set), [UID] FETCH
// (UID, FLAGS, INTERNALDATE, RFC822.SIZE, BODY[…] and BODY.PEEK[…] whole,
// HEADER, HEADER.FIELDS, TEXT), [UID] STORE ±FLAGS, IDLE.
'use strict'
const fs = require('node:fs')
const net = require('node:net')
const path = require('node:path')

const args = process.argv.slice(2)
const opt = (name, value) => {
  const i = args.indexOf(`--${name}`)
  return i < 0 ? value : value === undefined ? true : args[i + 1]
}
const PORT = Number(opt('port', '14300'))
const DIR = path.resolve(opt('dir', path.join(__dirname, 'fixtures', 'mail')))
const PASSWORD = opt('password', 'test')
const IDLE = !opt('no-idle')
const LOGIN_ONLY = !!opt('login-only')
const WANT_ID = !!opt('want-id')
const LOG = !!opt('log')
const SENT_DIR = opt('sent', '') ? path.resolve(opt('sent', '')) : ''
const UNMARKED_SENT = !!opt('unmarked-sent')

const CAPS = ['IMAP4rev1', 'ID', 'UIDPLUS', ...(IDLE ? ['IDLE'] : []), ...(LOGIN_ONLY ? [] : ['AUTH=PLAIN', 'SASL-IR'])]
const UIDVALIDITY = 1700000000
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']

// --- The mailboxes ---------------------------------------------------------------------------------

const boxes = { INBOX: { letters: [], next: 1, files: new Set() }, Sent: { letters: [], next: 1, files: new Set() } }
const clients = new Set()

// IMAP's date-time: "09-Oct-2026 20:51:29 +0200".
function imapDate(d) {
  const p = n => String(n).padStart(2, '0')
  const off = -d.getTimezoneOffset()
  const zone = `${off < 0 ? '-' : '+'}${p(Math.floor(Math.abs(off) / 60))}${p(Math.abs(off) % 60)}`
  return `${p(d.getDate())}-${MONTHS[d.getMonth()]}-${d.getFullYear()} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())} ${zone}`
}

// The header block (to the blank line, kept) and the rest.
function split(raw) {
  const at = raw.indexOf('\r\n\r\n')
  return at < 0 ? [raw, Buffer.alloc(0)] : [raw.subarray(0, at + 4), raw.subarray(at + 4)]
}

function headerDate(raw) {
  const m = /^Date:\s*(.+)$/im.exec(split(raw)[0].toString('latin1'))
  const d = m && new Date(m[1].trim())
  return d && !isNaN(d) ? d : null
}

// The files not seen yet, as new letters; true when there were any.
function scan() {
  const box = boxes.INBOX
  let added = false
  for (const name of fs.readdirSync(DIR).filter(n => n.endsWith('.eml')).sort()) {
    if (box.files.has(name)) continue
    box.files.add(name)
    // Lines end in CRLF however the file was saved.
    const raw = Buffer.from(fs.readFileSync(path.join(DIR, name)).toString('latin1').replace(/\r?\n/g, '\r\n'), 'latin1')
    const flags = new Set([...(name.includes('-seen') ? ['\\Seen'] : []), ...(name.includes('-flagged') ? ['\\Flagged'] : [])])
    box.letters.push({ uid: box.next++, raw, flags, date: headerDate(raw) || fs.statSync(path.join(DIR, name)).mtime, name })
    added = true
  }
  return added
}

// --- One connection -----------------------------------------------------------------------------------

function serve(socket) {
  const c = { socket, authed: false, idOk: !WANT_ID, box: null, readOnly: true, told: 0, idle: null, auth: null }
  clients.add(c)
  let buf = Buffer.alloc(0)
  let parts = []
  let need = 0

  const out = chunks => {
    const data = Buffer.concat(chunks.map(x => (Buffer.isBuffer(x) ? x : Buffer.from(x, 'utf8'))))
    if (LOG) console.log('S:', data.toString('latin1').slice(0, 300).replace(/\r\n/g, '⏎'))
    if (!socket.destroyed) socket.write(data)
  }
  const line = text => out([`${text}\r\n`])
  c.line = line

  line(`* OK [CAPABILITY ${CAPS.join(' ')}] fake IMAP ready`)
  socket.on('error', () => {})
  socket.on('close', () => clients.delete(c))
  socket.on('data', data => {
    buf = Buffer.concat([buf, data])
    pump()
  })

  function pump() {
    for (;;) {
      if (need > 0) {
        if (buf.length < need) return
        parts.push({ lit: buf.subarray(0, need) })
        buf = buf.subarray(need)
        need = 0
        continue
      }
      const at = buf.indexOf('\r\n')
      if (at < 0) return
      const text = buf.subarray(0, at).toString('utf8')
      buf = buf.subarray(at + 2)
      if (LOG) console.log('C:', text.slice(0, 200))
      if (c.idle) {
        if (text.toUpperCase() === 'DONE') {
          line(`${c.idle} OK IDLE terminated`)
          c.idle = null
        } else line(`${c.idle} BAD expected DONE`)
        continue
      }
      if (c.auth) {
        const tag = c.auth
        c.auth = null
        plain(tag, text)
        continue
      }
      const lit = /\{(\d+)(\+?)\}$/.exec(text)
      if (lit) {
        parts.push(text.slice(0, -lit[0].length))
        need = Number(lit[1])
        if (!lit[2]) line('+ go ahead')
        if (need === 0) parts.push({ lit: Buffer.alloc(0) })
        continue
      }
      parts.push(text)
      const command = parts
      parts = []
      try {
        run(tokens(command))
      } catch (e) {
        line(`${(typeof command[0] === 'string' && command[0].split(' ')[0]) || '*'} BAD ${e.message}`)
      }
    }
  }

  // AUTHENTICATE PLAIN's answer: "\0user\0password" in base64.
  function plain(tag, b64) {
    const [, user, pass] = Buffer.from(b64.trim(), 'base64').toString('utf8').split('\0')
    signIn(tag, user, pass)
  }

  function signIn(tag, user, pass) {
    if (pass !== PASSWORD || !user) return line(`${tag} NO [AUTHENTICATIONFAILED] Invalid credentials`)
    c.authed = true
    line(`${tag} OK [CAPABILITY ${CAPS.join(' ')}] signed in`)
  }

  // Whether the client was told of letters it has not been (an EXISTS owed).
  function tellNew() {
    const n = c.box ? boxes[c.box].letters.length : 0
    if (c.box && n !== c.told) {
      line(`* ${n} EXISTS`)
      c.told = n
    }
  }
  c.tellNew = tellNew

  function run(t) {
    const [tag, word, ...rest] = t
    if (typeof tag !== 'string' || typeof word !== 'string') throw new Error('no command')
    let cmd = word.toUpperCase()
    let uid = false
    if (cmd === 'UID') {
      uid = true
      cmd = String(rest.shift()).toUpperCase()
    }
    const needAuth = !['CAPABILITY', 'NOOP', 'LOGOUT', 'LOGIN', 'AUTHENTICATE', 'ID', 'STARTTLS'].includes(cmd)
    if (needAuth && !c.authed) return line(`${tag} NO not signed in`)
    switch (cmd) {
      case 'CAPABILITY':
        line(`* CAPABILITY ${CAPS.join(' ')}`)
        return line(`${tag} OK CAPABILITY completed`)
      case 'NOOP':
        tellNew()
        return line(`${tag} OK NOOP completed`)
      case 'LOGOUT':
        line('* BYE see you')
        line(`${tag} OK LOGOUT completed`)
        return socket.end()
      case 'ID':
        c.idOk = true
        line('* ID ("name" "fake-imap" "vendor" "wakuwaku tests")')
        return line(`${tag} OK ID completed`)
      case 'LOGIN':
        return signIn(tag, str(rest[0]), str(rest[1]))
      case 'AUTHENTICATE':
        if (LOGIN_ONLY || String(rest[0]).toUpperCase() !== 'PLAIN') return line(`${tag} NO mechanism not supported`)
        if (rest[1] !== undefined) return plain(tag, str(rest[1]))
        c.auth = tag
        return line('+ ')
      case 'LIST':
      case 'LSUB':
        line('* LIST (\\HasNoChildren) "/" "INBOX"')
        line(`* LIST (\\HasNoChildren${UNMARKED_SENT ? '' : ' \\Sent'}) "/" "Sent"`)
        return line(`${tag} OK ${cmd} completed`)
      case 'APPEND': {
        // APPEND box [(flags)] [date] {n}: the letter is the literal, last.
        const name = Object.keys(boxes).find(k => k.toUpperCase() === str(rest[0]).toUpperCase())
        if (!name) return line(`${tag} NO [TRYCREATE] no such mailbox`)
        const box = boxes[name]
        const raw = Buffer.from(str(rest[rest.length - 1]), 'utf8')
        const flags = new Set(Array.isArray(rest[1]) ? rest[1].map(String) : [])
        const letter = { uid: box.next++, raw, flags, date: new Date(), name: `${name.toLowerCase()}-${box.next - 1}` }
        box.letters.push(letter)
        if (SENT_DIR) {
          fs.mkdirSync(SENT_DIR, { recursive: true })
          fs.writeFileSync(path.join(SENT_DIR, `${letter.name}.eml`), raw)
        }
        console.log(`APPEND ${name}: ${raw.length} bytes, flags ${[...flags].join(' ') || 'none'}`)
        return line(`${tag} OK [APPENDUID ${UIDVALIDITY} ${letter.uid}] APPEND completed`)
      }
      case 'SELECT':
      case 'EXAMINE': {
        const name = Object.keys(boxes).find(k => k.toUpperCase() === str(rest[0]).toUpperCase())
        if (!c.idOk) return line(`${tag} NO SELECT Unsafe Login. Please contact kefu@188.com for help`)
        if (!name) return line(`${tag} NO no such mailbox`)
        const box = boxes[name]
        c.box = name
        c.readOnly = cmd === 'EXAMINE'
        c.told = box.letters.length
        line('* FLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft)')
        line(`* OK [PERMANENTFLAGS (${c.readOnly ? '' : '\\Answered \\Flagged \\Deleted \\Seen \\Draft \\*'})] flags`)
        line(`* ${box.letters.length} EXISTS`)
        line('* 0 RECENT')
        line(`* OK [UIDVALIDITY ${UIDVALIDITY}] UIDs valid`)
        line(`* OK [UIDNEXT ${box.next}] next UID`)
        return line(`${tag} OK [${c.readOnly ? 'READ-ONLY' : 'READ-WRITE'}] ${cmd} completed`)
      }
      case 'CLOSE':
      case 'UNSELECT':
        c.box = null
        return line(`${tag} OK ${cmd} completed`)
      case 'SEARCH':
        return search(tag, uid, rest)
      case 'FETCH':
        return fetch(tag, uid, rest)
      case 'STORE':
        return store(tag, uid, rest)
      case 'IDLE':
        if (!IDLE) return line(`${tag} BAD IDLE not supported`)
        c.idle = tag
        line('+ idling')
        return tellNew()
      default:
        return line(`${tag} BAD unknown command ${cmd}`)
    }
  }

  // The letters a set names ("1:*", "3,5:7"), by sequence number or UID,
  // each with its sequence number.
  function pick(set, uid) {
    if (!c.box) throw new Error('no mailbox selected')
    const letters = boxes[c.box].letters.slice(0, c.told)
    const max = uid ? letters.reduce((m, l) => Math.max(m, l.uid), 0) : letters.length
    const ranges = String(set)
      .split(',')
      .map(r => r.split(':').map(x => (x === '*' ? max : Number(x))))
      .map(([a, b = a]) => [Math.min(a, b), Math.max(a, b)])
    const hit = n => ranges.some(([a, b]) => n >= a && n <= b)
    return letters.map((l, i) => ({ l, seq: i + 1 })).filter(({ l, seq }) => hit(uid ? l.uid : seq))
  }

  function search(tag, uid, keys) {
    if (!c.box) return line(`${tag} NO no mailbox selected`)
    let all = boxes[c.box].letters.slice(0, c.told).map((l, i) => ({ l, seq: i + 1 }))
    keys = keys.map(k => (typeof k === 'string' ? k : k))
    if (String(keys[0]).toUpperCase() === 'CHARSET') keys = keys.slice(2)
    for (let i = 0; i < keys.length; i++) {
      const k = String(keys[i]).toUpperCase()
      if (k === 'ALL') continue
      if (k === 'SEEN') all = all.filter(x => x.l.flags.has('\\Seen'))
      else if (k === 'UNSEEN') all = all.filter(x => !x.l.flags.has('\\Seen'))
      else if (k === 'FLAGGED') all = all.filter(x => x.l.flags.has('\\Flagged'))
      else if (k === 'UNFLAGGED') all = all.filter(x => !x.l.flags.has('\\Flagged'))
      else if (k === 'UID') {
        const set = new Set(pick(keys[++i], true).map(x => x.l.uid))
        all = all.filter(x => set.has(x.l.uid))
      } else if (/^[\d:*,]+$/.test(k)) {
        const set = new Set(pick(k, false).map(x => x.seq))
        all = all.filter(x => set.has(x.seq))
      } else return line(`${tag} BAD search key ${k} not supported`)
    }
    line(`* SEARCH${all.map(x => ` ${uid ? x.l.uid : x.seq}`).join('')}`)
    line(`${tag} OK SEARCH completed`)
  }

  // One FETCH item's answer for a letter.
  function item(name, letter, marks) {
    const n = name.toUpperCase()
    if (n === 'UID') return [`UID ${letter.uid}`]
    if (n === 'FLAGS') return [`FLAGS (${[...letter.flags].join(' ')})`]
    if (n === 'INTERNALDATE') return [`INTERNALDATE "${imapDate(letter.date)}"`]
    if (n === 'RFC822.SIZE') return [`RFC822.SIZE ${letter.raw.length}`]
    const body = /^BODY(\.PEEK)?\[([^\]]*)\](<\d+(\.\d+)?>)?$/i.exec(name)
    if (!body) throw new Error(`fetch item ${name} not supported`)
    if (!body[1] && !c.readOnly && !letter.flags.has('\\Seen')) {
      letter.flags.add('\\Seen')
      marks.add(letter)
    }
    const section = body[2]
    const [head, text] = split(letter.raw)
    let data
    const fields = /^HEADER\.FIELDS(\.NOT)? \((.*)\)$/i.exec(section)
    if (section === '') data = letter.raw
    else if (/^HEADER$/i.test(section)) data = head
    else if (/^TEXT$/i.test(section)) data = text
    else if (fields) data = pickFields(head, fields[2].split(/\s+/), !!fields[1])
    else throw new Error(`section ${section} not supported`)
    return [`BODY[${section}] {${data.length}}\r\n`, data]
  }

  function fetch(tag, uid, [set, what]) {
    if (!c.box) return line(`${tag} NO no mailbox selected`)
    let names = Array.isArray(what) ? what.map(String) : [String(what)]
    const macros = { FAST: ['FLAGS', 'INTERNALDATE', 'RFC822.SIZE'], ALL: ['FLAGS', 'INTERNALDATE', 'RFC822.SIZE'] }
    if (names.length === 1 && macros[names[0].toUpperCase()]) names = macros[names[0].toUpperCase()]
    if (uid && !names.some(n => n.toUpperCase() === 'UID')) names = ['UID', ...names]
    const marks = new Set()
    for (const { l, seq } of pick(set, uid)) {
      const chunks = [`* ${seq} FETCH (`]
      names.forEach((name, i) => chunks.push(...(i ? [' '] : []), ...item(name, l, marks)))
      if (marks.has(l) && !names.some(n => n.toUpperCase() === 'FLAGS')) chunks.push(` FLAGS (${[...l.flags].join(' ')})`)
      chunks.push(')\r\n')
      out(chunks)
    }
    line(`${tag} OK FETCH completed`)
  }

  function store(tag, uid, [set, how, flags]) {
    if (!c.box) return line(`${tag} NO no mailbox selected`)
    if (c.readOnly) return line(`${tag} NO mailbox is read-only`)
    const list = (Array.isArray(flags) ? flags : [flags]).map(String)
    const kind = String(how).toUpperCase()
    for (const { l, seq } of pick(set, uid)) {
      if (kind.startsWith('+')) list.forEach(f => l.flags.add(f))
      else if (kind.startsWith('-')) list.forEach(f => l.flags.delete(f))
      else l.flags = new Set(list)
      if (LOG || list.includes('\\Answered')) console.log(`STORE ${c.box} ${l.name}: ${[...l.flags].join(' ')}`)
      if (!kind.endsWith('.SILENT')) line(`* ${seq} FETCH (${uid ? `UID ${l.uid} ` : ''}FLAGS (${[...l.flags].join(' ')}))`)
    }
    line(`${tag} OK STORE completed`)
  }
}

// The named header fields (or all but them), each with its folded lines,
// and the blank line.
function pickFields(head, names, not) {
  const want = new Set(names.map(n => n.toUpperCase()))
  const lines = head.toString('latin1').split('\r\n')
  const kept = []
  let keep = false
  for (const l of lines) {
    if (l === '') break
    if (!/^[ \t]/.test(l)) keep = want.has(l.split(':')[0].trim().toUpperCase()) !== not
    if (keep) kept.push(l)
  }
  return Buffer.from(kept.map(l => `${l}\r\n`).join('') + '\r\n', 'latin1')
}

// A command's words: atoms (BODY[HEADER.FIELDS (A B)] stays one), quoted
// strings, literals ({lit}), and parenthesized lists as arrays.
function tokens(parts) {
  const out = []
  const stack = [out]
  for (const part of parts) {
    if (typeof part !== 'string') {
      stack[stack.length - 1].push(part.lit.toString('utf8'))
      continue
    }
    let i = 0
    while (i < part.length) {
      const ch = part[i]
      if (ch === ' ') i++
      else if (ch === '(') {
        const list = []
        stack[stack.length - 1].push(list)
        stack.push(list)
        i++
      } else if (ch === ')') {
        stack.pop()
        i++
      } else if (ch === '"') {
        let s = ''
        i++
        while (i < part.length && part[i] !== '"') {
          if (part[i] === '\\') i++
          s += part[i++]
        }
        i++
        stack[stack.length - 1].push({ quoted: s })
      } else {
        let s = ''
        let depth = 0
        while (i < part.length && (depth > 0 || !' ()'.includes(part[i]))) {
          if (part[i] === '[') depth++
          if (part[i] === ']') depth--
          s += part[i++]
        }
        stack[stack.length - 1].push(s)
      }
    }
  }
  return out.map(function flat(t) {
    return Array.isArray(t) ? t.map(flat) : typeof t === 'object' ? t.quoted : t
  })
}

const str = t => String(t ?? '')

scan()
setInterval(() => {
  if (!scan()) return
  for (const c of clients) if (c.idle) c.tellNew()
}, 500)

net.createServer(serve).listen(PORT, '127.0.0.1', () => console.log(`fake IMAP on 127.0.0.1:${PORT}, ${boxes.INBOX.letters.length} letters from ${DIR}`))
