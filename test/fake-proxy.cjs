// A small proxy on 127.0.0.1 for trying the pet's mail through one
// (src-tauri/src/mail/proxy.rs) without Clash or v2rayN: one port that
// speaks HTTP CONNECT and SOCKS5 both, as Clash's mixed port does.
//
//   node test/fake-proxy.cjs [--port 17890] [--map <name>=<host>:<port>,…]
//                           [--http-only | --socks-only] [--only-443] [--log]
//
// --map sends a name to a server here (imap.example.test=127.0.0.1:14310 for
// test/fake-imap.cjs): the pet's account names a server no DNS knows, so only
// a proxy that looks the name up itself (CONNECT by name, SOCKS5's ATYP 3)
// gets there; a name not mapped is looked up here and connected to. Each
// way through is printed (--log: the bytes' count when it closes too).
// --http-only closes a SOCKS5 greeting and --socks-only an HTTP request, as
// a proxy of the other kind would; --only-443 refuses CONNECT to any other
// port (403), as Squid does by default. Stopped (Ctrl+C) and started again,
// it is a proxy restarted: the pet's IDLE breaks, and it connects again.
'use strict'
const net = require('node:net')

const args = process.argv.slice(2)
const opt = (name, value) => {
  const i = args.indexOf(`--${name}`)
  return i < 0 ? value : value === undefined ? true : args[i + 1]
}
const PORT = Number(opt('port', '17890'))
const MAP = new Map(
  String(opt('map', '') || '')
    .split(',')
    .filter(Boolean)
    .map(pair => {
      const [name, to] = pair.split('=')
      const at = to.lastIndexOf(':')
      return [name.toLowerCase(), { host: to.slice(0, at), port: Number(to.slice(at + 1)) }]
    }),
)
const HTTP = !opt('socks-only')
const SOCKS = !opt('http-only')
const ONLY_443 = !!opt('only-443')
const LOG = !!opt('log')

// Where a name and port go: the server it is mapped to, else itself.
const target = (host, port) => MAP.get(host.toLowerCase()) || { host, port }

// Bytes both ways, once the way is open; what is already read goes first.
function join(client, host, port, ready, rest) {
  const to = target(host, port)
  const up = net.connect(to.port, to.host)
  let bytes = 0
  let closed = false
  up.on('connect', () => {
    console.log(`${host}:${port} → ${to.host}:${to.port}`)
    ready(true)
    if (rest.length) up.write(rest)
    client.on('data', b => ((bytes += b.length), up.write(b)))
    up.on('data', b => ((bytes += b.length), client.write(b)))
    client.resume()
  })
  up.on('error', e => {
    console.log(`${host}:${port}: ${e.message}`)
    ready(false)
  })
  const end = () => {
    if (closed) return
    closed = true
    if (LOG) console.log(`${host}:${port} closed, ${bytes} bytes`)
    client.destroy()
    up.destroy()
  }
  up.on('close', end)
  client.on('close', end)
  client.on('error', end)
}

function http(client, buf) {
  const end = buf.indexOf('\r\n\r\n')
  if (end < 0) return false
  const line = buf.subarray(0, buf.indexOf('\r\n')).toString()
  const rest = buf.subarray(end + 4)
  const [method, to] = line.split(' ')
  if (method !== 'CONNECT') {
    client.end('HTTP/1.1 405 Method Not Allowed\r\n\r\n')
    return true
  }
  const at = to.lastIndexOf(':')
  const host = to.slice(0, at).replace(/^\[|\]$/g, '')
  const port = Number(to.slice(at + 1))
  if (ONLY_443 && port !== 443) {
    console.log(`${to}: refused (only 443)`)
    client.end('HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n')
    return true
  }
  join(client, host, port, ok => client.write(ok ? 'HTTP/1.1 200 Connection established\r\n\r\n' : 'HTTP/1.1 502 Bad Gateway\r\n\r\n'), rest)
  return true
}

// SOCKS5 (RFC 1928), no password: CONNECT by IPv4, name or IPv6.
function socks(client, buf) {
  if (buf.length < 2 || buf.length < 2 + buf[1]) return false
  const hello = 2 + buf[1]
  if (!buf.subarray(2, hello).includes(0)) {
    client.end(Buffer.from([5, 0xff]))
    return true
  }
  const req = buf.subarray(hello)
  if (req.length < 5) {
    if (!client.greeted) client.write(Buffer.from([5, 0]))
    client.greeted = true
    return false
  }
  if (!client.greeted) client.write(Buffer.from([5, 0]))
  const kind = req[3]
  const len = kind === 1 ? 4 : kind === 4 ? 16 : kind === 3 ? 1 + req[4] : -1
  if (len < 0) {
    client.end(Buffer.from([5, 8, 0, 1, 0, 0, 0, 0, 0, 0]))
    return true
  }
  if (req.length < 4 + len + 2) return false
  const addr = req.subarray(4, 4 + len)
  const host = kind === 1 ? [...addr].join('.') : kind === 3 ? addr.subarray(1).toString() : addr.toString('hex').match(/.{4}/g).join(':')
  const port = req.readUInt16BE(4 + len)
  join(client, host, port, ok => client.write(Buffer.from([5, ok ? 0 : 4, 0, 1, 127, 0, 0, 1, 0, 0])), req.subarray(4 + len + 2))
  return true
}

net
  .createServer(client => {
    let buf = Buffer.alloc(0)
    const read = b => {
      buf = Buffer.concat([buf, b])
      // The first byte says which: 5 is SOCKS5, a letter an HTTP request.
      const isSocks = buf[0] === 5
      if ((isSocks && !SOCKS) || (!isSocks && !HTTP)) return client.destroy()
      // The way asked for: nothing more read until it is open.
      if ((isSocks ? socks : http)(client, buf)) {
        client.off('data', read)
        client.pause()
      }
    }
    client.on('data', read)
    client.on('error', () => {})
  })
  .listen(PORT, '127.0.0.1', () => console.log(`fake proxy on 127.0.0.1:${PORT}${HTTP ? ' HTTP' : ''}${SOCKS ? ' SOCKS5' : ''}${ONLY_443 ? ' (CONNECT to 443 only)' : ''}`))
