// A crash relay on 127.0.0.1 for trying the pet's crash reports without
// sending anything anywhere (src-tauri/src/crash.rs): relay/relay.js's own
// handle(), its issues kept in memory instead of on GitHub or Gitee.
//
//   node test/fake-relay.cjs [--port 47391]
//
// Start the pet with WAKUWAKU_CRASH_URL=http://127.0.0.1:47391/report (a
// build from the source tree sends then too; two relays, comma-separated).
// Each report is printed as the issue it makes, or the comment it adds.
//   GET /issues         the issues so far, with their comments (JSON)
//   GET /issues/<n>     one as it would show: its title, body and comments
//   GET /close/<n>      closed (as when fixed): the same crash from a newer
//                       version opens a new one
//   GET /mode?<name>=1  (0 to undo; /mode alone says them all), while it runs:
//     down     every report answered 503 (she keeps it, tries later)
//     busy     429 (the same)
//     refuse   400 (she puts it in crashes/refused)
//     slow     answered after 25 s (she gives up at 20 s)
// Every request is logged.

const http = require('http')
const path = require('path')
const { pathToFileURL } = require('url')

const arg = (name, fallback) => {
  const i = process.argv.indexOf(`--${name}`)
  return i > 0 ? process.argv[i + 1] : fallback
}
const port = Number(arg('port', 47391))
const base = `http://127.0.0.1:${port}`
const MODES = ['down', 'busy', 'refuse', 'slow']
const mode = {}

// Where the issues go: here, in memory.
const issues = []
const view = i => ({ number: i.number, url: i.url, state: i.state, body: i.body })
const forge = {
  name: 'fake',
  async find(tag) {
    const hits = issues.filter(i => i.title.includes(`[${tag}]`))
    const hit = hits.find(i => i.state === 'open') || hits[0]
    return hit ? view(hit) : null
  },
  async get(number) {
    const i = issues.find(x => x.number === number)
    if (!i) throw Object.assign(new Error('404'), { status: 404 })
    return view(i)
  },
  async create(title, body) {
    const i = { number: issues.length + 1, url: `${base}/issues/${issues.length + 1}`, state: 'open', title, body, comments: [] }
    issues.push(i)
    console.log(`\n=== new issue #${i.number}: ${title}\n${body}\n`)
    return view(i)
  },
  async comment(number, body) {
    issues.find(x => x.number === number).comments.push(body)
    console.log(`\n=== comment on #${number}\n${body}\n`)
  },
  async edit(number, body) {
    issues.find(x => x.number === number).body = body
    console.log(`(#${number} edited: ${(body.match(/\*\*Reports: \d+\*\*/) || [''])[0]})`)
  },
}

async function main() {
  const relay = await import(pathToFileURL(path.join(__dirname, '..', 'relay', 'relay.js')))
  const store = relay.memoryStore()
  const env = { PER_IP_DAY: arg('per-ip-day', '1000'), NEW_PER_DAY: arg('new-per-day', '1000'), COMMENTS: arg('comments', '10') }

  http
    .createServer((req, res) => {
      const chunks = []
      req.on('data', c => chunks.push(c))
      req.on('end', async () => {
        const url = new URL(req.url, base)
        const send = (status, body, type = 'application/json; charset=utf-8') => {
          console.log(`${new Date().toISOString().slice(11, 19)} ${req.method} ${req.url} → ${status}`)
          res.writeHead(status, { 'content-type': type })
          res.end(typeof body === 'string' ? body : JSON.stringify(body, null, 2))
        }
        if (url.pathname === '/mode') {
          for (const [k, v] of url.searchParams) if (MODES.includes(k)) mode[k] = v === '1'
          return send(200, Object.fromEntries(MODES.map(m => [m, !!mode[m]])))
        }
        if (url.pathname === '/issues') return send(200, issues)
        const one = url.pathname.match(/^\/(issues|close)\/(\d+)$/)
        if (one) {
          const i = issues.find(x => x.number === Number(one[2]))
          if (!i) return send(404, { error: 'no such issue' })
          if (one[1] === 'close') i.state = 'closed'
          return send(200, `${i.title}  (${i.state})\n\n${i.body}\n\n${i.comments.map(c => `--- comment\n${c}`).join('\n\n')}\n`, 'text/plain; charset=utf-8')
        }
        if (req.method === 'POST' && mode.slow) await new Promise(r => setTimeout(r, 25000))
        if (req.method === 'POST' && mode.down) return send(503, { ok: false, error: 'down (mode)' })
        if (req.method === 'POST' && mode.busy) return send(429, { ok: false, error: 'busy (mode)' })
        if (req.method === 'POST' && mode.refuse) return send(400, { ok: false, error: 'refused (mode)' })
        const out = await relay.handle(
          { method: req.method, path: url.pathname, ip: req.socket.remoteAddress, header: n => req.headers[n.toLowerCase()], body: Buffer.concat(chunks).toString('utf8') },
          env,
          store,
          forge,
        )
        if (out.status >= 400) console.log(`refused: ${out.json.error}`)
        send(out.status, out.json)
      })
    })
    .listen(port, '127.0.0.1', () => console.log(`fake crash relay on ${base}/report (issues: ${base}/issues)`))
}

main()
