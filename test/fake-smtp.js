// A small SMTP server on 127.0.0.1 for trying the pet's sending without a
// real account: no TLS (set the outgoing server up with security "plain"),
// one password, every letter it takes written to a folder.
//
//   node test/fake-smtp.js [--port 14325] [--dir <folder>] [--password test]
//                          [--auth plain,login | none] [--reject <address>,…]
//                          [--log]
//
// Each letter goes to <folder>/<n>.eml as it came (dots unstuffed), with
// <n>.json beside it: who from, to whom (the envelope, Bcc included).
// --auth says which AUTH mechanisms it offers (none: no sign-in, anyone may
// send); with some, MAIL is refused before signing in. --reject refuses
// those recipients (550), as a server does an address it has no mailbox for.
//
// Only what the pet says is understood: EHLO, HELO, AUTH PLAIN (inline or
// after 334), AUTH LOGIN, MAIL FROM, RCPT TO, DATA, RSET, NOOP, QUIT.
'use strict'
const fs = require('node:fs')
const net = require('node:net')
const path = require('node:path')

const args = process.argv.slice(2)
const opt = (name, value) => {
  const i = args.indexOf(`--${name}`)
  return i < 0 ? value : value === undefined ? true : args[i + 1]
}
const PORT = Number(opt('port', '14325'))
const DIR = path.resolve(opt('dir', path.join(require('node:os').tmpdir(), 'fake-smtp')))
const PASSWORD = opt('password', 'test')
const AUTHS = String(opt('auth', 'plain,login')).toUpperCase() === 'NONE' ? [] : String(opt('auth', 'plain,login')).toUpperCase().split(',')
const REJECT = new Set(String(opt('reject', '') || '').toLowerCase().split(',').filter(Boolean))
const LOG = !!opt('log')

fs.mkdirSync(DIR, { recursive: true })
let count = fs.readdirSync(DIR).filter(n => n.endsWith('.eml')).length

function serve(socket) {
  const c = { authed: AUTHS.length === 0, from: null, to: [], data: null, auth: null }
  let buf = Buffer.alloc(0)
  const say = text => {
    if (LOG) console.log('S:', text)
    if (!socket.destroyed) socket.write(`${text}\r\n`)
  }
  say('220 fake ESMTP ready')
  socket.on('error', () => {})
  socket.on('data', data => {
    buf = Buffer.concat([buf, data])
    for (;;) {
      const at = buf.indexOf('\r\n')
      if (at < 0) return
      const line = buf.subarray(0, at).toString('utf8')
      buf = buf.subarray(at + 2)
      if (c.data) {
        if (line === '.') {
          done()
          continue
        }
        // A line that starts with a dot was given one more.
        c.data.push(line.startsWith('.') ? line.slice(1) : line)
        continue
      }
      if (LOG) console.log('C:', line.startsWith('AUTH') || c.auth ? '(sign-in)' : line)
      command(line)
    }
  })

  function signIn(user, pass) {
    if (!user || pass !== PASSWORD) return say('535 5.7.8 Username and Password not accepted')
    c.authed = true
    say('235 2.7.0 Accepted')
  }

  const b64 = s => Buffer.from(s, 'base64').toString('utf8')

  function command(line) {
    // The steps of AUTH LOGIN, and of AUTH PLAIN without its answer inline.
    if (c.auth) {
      const step = c.auth
      c.auth = null
      if (line === '*') return say('501 5.7.0 cancelled')
      if (step === 'plain') {
        const [, user, pass] = b64(line).split('\0')
        return signIn(user, pass)
      }
      if (step === 'user') {
        c.auth = { user: b64(line) }
        return say('334 UGFzc3dvcmQ6')
      }
      return signIn(step.user, b64(line))
    }
    const [word, ...rest] = line.split(' ')
    const arg = rest.join(' ')
    switch (word.toUpperCase()) {
      case 'EHLO':
        say('250-fake.test hello')
        say('250-SIZE 31457280')
        say('250-8BITMIME')
        if (AUTHS.length) say(`250-AUTH ${AUTHS.join(' ')}`)
        return say('250 ENHANCEDSTATUSCODES')
      case 'HELO':
        return say('250 fake.test')
      case 'AUTH': {
        const [mech, initial] = arg.split(' ')
        if (!AUTHS.includes(mech.toUpperCase())) return say('504 5.5.4 mechanism not supported')
        if (mech.toUpperCase() === 'PLAIN') {
          if (initial) {
            const [, user, pass] = b64(initial).split('\0')
            return signIn(user, pass)
          }
          c.auth = 'plain'
          return say('334 ')
        }
        c.auth = 'user'
        return say('334 VXNlcm5hbWU6')
      }
      case 'MAIL': {
        if (!c.authed) return say('530 5.7.0 Authentication required')
        const m = /^FROM:<([^>]*)>/i.exec(arg)
        if (!m) return say('501 5.5.4 syntax: MAIL FROM:<address>')
        Object.assign(c, { from: m[1], to: [] })
        return say('250 2.1.0 OK')
      }
      case 'RCPT': {
        if (c.from === null) return say('503 5.5.1 MAIL first')
        const m = /^TO:<([^>]*)>/i.exec(arg)
        if (!m) return say('501 5.5.4 syntax: RCPT TO:<address>')
        if (REJECT.has(m[1].toLowerCase())) return say(`550 5.1.1 <${m[1]}>: no such user here`)
        c.to.push(m[1])
        return say('250 2.1.5 OK')
      }
      case 'DATA':
        if (!c.to.length) return say('503 5.5.1 RCPT first')
        c.data = []
        return say('354 Go ahead, end with <CRLF>.<CRLF>')
      case 'RSET':
        Object.assign(c, { from: null, to: [] })
        return say('250 2.0.0 OK')
      case 'NOOP':
        return say('250 2.0.0 OK')
      case 'QUIT':
        say('221 2.0.0 bye')
        return socket.end()
      default:
        return say('502 5.5.2 command not recognized')
    }
  }

  // A letter whole: kept, with its envelope.
  function done() {
    const raw = c.data.join('\r\n') + '\r\n'
    const n = ++count
    fs.writeFileSync(path.join(DIR, `${n}.eml`), raw)
    fs.writeFileSync(path.join(DIR, `${n}.json`), JSON.stringify({ from: c.from, to: c.to }, null, 2))
    console.log(`letter ${n}: from ${c.from} to ${c.to.join(', ')}, ${raw.length} bytes`)
    Object.assign(c, { from: null, to: [], data: null })
    say(`250 2.0.0 OK queued as ${n}`)
  }
}

net.createServer(serve).listen(PORT, '127.0.0.1', () => console.log(`fake SMTP on 127.0.0.1:${PORT}, letters to ${DIR}`))
