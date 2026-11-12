// A LibreHardwareMonitor Remote Web Server on 127.0.0.1, for trying the
// monitor's hardware sensors (src-tauri/src/lhm.rs) without LHM installed:
// GET /data.json answers test/fixtures/lhm/data.json (an AMD desktop with an
// NVIDIA card), its values moving a little each time, as a running LHM's do,
// in the shape LHM 0.9.6 sends it (HttpServer.cs at its tag): no Version, its
// Raw* values words like its Value ("62.4 °C").
//
//   node test/fake-lhm.cjs [--port 8085] [--file <data.json>] [--user u --pass p] [--nightly | --old]
//
//   --nightly        as LHM built from its source since March 2026: the
//                    Version at the root ("0.9.6"), Raw* values numbers
//   --old            as LHM 0.9.4 and before, in German: no HardwareId, no
//                    Raw* values, only the words, a comma for the point ("62,4 °C")
//   --user, --pass   Basic auth, as LHM's "Authentication" (401 without it,
//                    or with a wrong one)
//   GET /set?id=<SensorId>&value=<n>   that sensor held at n (the CPU at 95 °C
//                    for the orange); /set?id=<SensorId> lets it go again
//   GET /mode?down=1 (0 to undo; /mode alone says them all), while it runs:
//     down    every request answered 503
//     slow    answered after 600 ms (she gives up at 300 ms)
//     strange answered with a page that is not LHM's
// Every request is logged.

const http = require('http')
const fs = require('fs')
const path = require('path')

const arg = (name, fallback) => {
  const i = process.argv.indexOf(`--${name}`)
  return i > 0 ? process.argv[i + 1] : fallback
}
const has = name => process.argv.includes(`--${name}`)
const port = Number(arg('port', 8085))
const file = arg('file', path.join(__dirname, 'fixtures', 'lhm', 'data.json'))
const user = arg('user', '')
const pass = arg('pass', '')
const shape = has('nightly') ? 'nightly' : has('old') ? 'old' : 'release'
const tree = JSON.parse(fs.readFileSync(file, 'utf8'))
const MODES = ['down', 'slow', 'strange']
const mode = {}
const held = new Map()

// LHM's words for a value, by its type (SensorNode.cs); a comma in German.
const UNITS = { Voltage: [3, ' V'], Clock: [1, ' MHz'], Load: [1, ' %'], Temperature: [1, ' °C'], Fan: [0, ' RPM'], Control: [1, ' %'], Power: [1, ' W'], SmallData: [1, ' MB'] }
function words(type, v) {
  if (typeof v !== 'number') return '-'
  const [digits, unit] = UNITS[type] || [1, '']
  const text = v.toFixed(digits) + unit
  return shape === 'old' ? text.replace('.', ',') : text
}

// The tree now: each sensor's value moved by up to 4% (loads kept within
// 0 to 100), or held; in the shape asked for.
function now(node) {
  const out = { ...node }
  if (node.SensorId) {
    let v = node.RawValue
    if (held.has(node.SensorId)) v = held.get(node.SensorId)
    else if (typeof v === 'number') {
      v = v * (1 + (Math.random() - 0.5) * 0.08)
      if (node.Type === 'Load' || node.Type === 'Control') v = Math.min(100, Math.max(0, v))
    }
    out.Value = words(node.Type, v)
    out.RawValue = shape === 'nightly' ? v : out.Value
    if (shape === 'release') for (const k of ['RawMin', 'RawMax']) out[k] = words(node.Type, node[k])
    if (shape === 'old') {
      for (const k of ['RawMin', 'RawValue', 'RawMax']) delete out[k]
      for (const k of ['Min', 'Max']) out[k] = String(out[k]).replace('.', ',')
    }
  }
  if (shape === 'old') delete out.HardwareId
  out.Children = (node.Children || []).map(now)
  return out
}

const server = http.createServer((req, res) => {
  const url = new URL(req.url, `http://127.0.0.1:${port}`)
  console.log(new Date().toISOString().slice(11, 23), req.method, req.url, req.headers.authorization ? '(auth)' : '')
  if (url.pathname === '/mode') {
    for (const m of MODES) if (url.searchParams.has(m)) mode[m] = url.searchParams.get(m) === '1'
    res.writeHead(200, { 'content-type': 'application/json' })
    return res.end(JSON.stringify(mode))
  }
  if (url.pathname === '/set') {
    const id = url.searchParams.get('id')
    const value = url.searchParams.get('value')
    if (value === null) held.delete(id)
    else held.set(id, Number(value))
    res.writeHead(200, { 'content-type': 'application/json' })
    return res.end(JSON.stringify(Object.fromEntries(held)))
  }
  if (user || pass) {
    const given = Buffer.from(String(req.headers.authorization || '').replace(/^Basic /, ''), 'base64').toString()
    if (given !== `${user}:${pass}`) {
      res.writeHead(401, { 'www-authenticate': 'Basic realm=""' })
      return res.end()
    }
  }
  if (mode.down) {
    res.writeHead(503)
    return res.end()
  }
  const answer = () => {
    if (url.pathname !== '/data.json') {
      res.writeHead(404)
      return res.end()
    }
    if (mode.strange) {
      res.writeHead(200, { 'content-type': 'text/html' })
      return res.end('<!doctype html><title>Not LHM</title>')
    }
    const body = now(tree)
    if (shape !== 'nightly') delete body.Version
    res.writeHead(200, { 'content-type': 'application/json', 'cache-control': 'no-cache' })
    res.end(JSON.stringify(body))
  }
  if (mode.slow) setTimeout(answer, 600)
  else answer()
})

server.listen(port, '127.0.0.1', () => console.log(`fake LHM (${shape}) on http://127.0.0.1:${port}/data.json${user || pass ? `, Basic auth as ${user}` : ''}`))
