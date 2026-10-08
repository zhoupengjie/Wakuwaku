// The local HTTP port the Claude Code hook reports to.
//
//   GET  /health       { ok, app, state }
//   POST /hook         a Claude Code hook event, as an HTTP hook sends it; answers {},
//                      or for a prompt, the person's answer on the pet
//   POST /state        a message (src/main/state.js): { mood, detail, event, react, say }
//   GET  /snapshot     the window as a PNG (debugging); ?page=settings for that one
//   POST /debug/look   { dx, dy }: look as if the cursor were there (WAKUWAKU_DEBUG=1 only)
//   POST /debug/walk   { dx, ms }: take a walk now (WAKUWAKU_DEBUG=1 only)
//   POST /debug/click  { selector }: click that element in the page (WAKUWAKU_DEBUG=1 only)
//   POST /debug/eval   { page, code }: run code in the pet or settings page (WAKUWAKU_DEBUG=1 only)
//   POST /debug/settings  "open", or a settings patch (WAKUWAKU_DEBUG=1 only)
//   POST /debug/fullscreen  true / false: as if another app went full screen (WAKUWAKU_DEBUG=1 only)
const http = require('http')

const MAX_BODY = 4096

// The request's JSON body: bytes first, decoded once, so a character split
// across chunks stays whole.
function readJson(req) {
  return new Promise((resolve, reject) => {
    const chunks = []
    let size = 0
    req.on('data', chunk => {
      chunks.push(chunk)
      size += chunk.length
      if (size > MAX_BODY) {
        req.destroy()
        reject(new Error('too large'))
      }
    })
    req.on('end', () => {
      try {
        resolve(JSON.parse(Buffer.concat(chunks).toString('utf8')))
      } catch (err) {
        reject(err)
      }
    })
    req.on('error', reject)
  })
}

function serve({ port, getState, setState, onHook, snapshot, lookAt, walkBy, click, evaluate, debugSettings, debugFullscreen, onTaken }) {
  const server = http.createServer(async (req, res) => {
    const reply = (code, body) => {
      res.writeHead(code, { 'content-type': 'application/json' })
      res.end(JSON.stringify(body))
    }
    const route = `${req.method} ${req.url.split('?')[0]}`

    // Claude Code reads the answer as the hook's output. onHook gives the
    // output, or a promise of it for a prompt the person answers on the pet;
    // anything else, or anything unreadable, is {} at once (no decision).
    // If Claude Code hangs up first, the signal tells onHook to forget it.
    if (route === 'POST /hook') {
      let event
      try {
        event = await readJson(req)
      } catch {
        return reply(200, {})
      }
      const hangUp = new AbortController()
      res.on('close', () => {
        if (!res.writableFinished) hangUp.abort()
      })
      let out
      try {
        out = await onHook(event, hangUp.signal)
      } catch {
        out = undefined
      }
      if (hangUp.signal.aborted) return
      // Only an object is a hook output; anything else would be a bad answer.
      return reply(200, out && typeof out === 'object' && !Array.isArray(out) ? out : {})
    }

    if (req.method === 'GET' && req.url === '/health') {
      return reply(200, { ok: true, app: 'wakuwaku', state: getState() })
    }

    if (route === 'GET /snapshot') {
      try {
        const png = await snapshot(new URL(req.url, 'http://x').searchParams.get('page') || 'pet')
        res.writeHead(200, { 'content-type': 'image/png' })
        return res.end(png)
      } catch {
        return reply(500, { error: 'capture failed' })
      }
    }

    if (req.method === 'POST' && req.url === '/state') {
      try {
        return setState(await readJson(req)) ? reply(200, { ok: true }) : reply(400, { error: 'nothing to do' })
      } catch {
        return reply(400, { error: 'bad json' })
      }
    }

    const debug = { '/debug/eval': evaluate, '/debug/settings': debugSettings, '/debug/fullscreen': debugFullscreen }[req.url]
    if (req.method === 'POST' && debug) {
      try {
        return reply(200, { result: await debug(await readJson(req)) })
      } catch (err) {
        return reply(400, { error: String(err?.message || err) })
      }
    }

    if (req.method === 'POST' && req.url === '/debug/click' && click) {
      try {
        const { selector } = await readJson(req)
        return reply(200, { result: await click(String(selector)) })
      } catch {
        return reply(400, { error: 'bad json' })
      }
    }

    if (req.method === 'POST' && req.url === '/debug/walk' && walkBy) {
      try {
        const { dx, ms } = await readJson(req)
        return reply(200, { ok: true, arrived: await walkBy({ dx: Number(dx), ms: Number(ms) }) })
      } catch {
        return reply(400, { error: 'bad json' })
      }
    }

    if (req.method === 'POST' && req.url === '/debug/look' && lookAt) {
      try {
        const { dx, dy } = await readJson(req)
        lookAt({ dx: Number(dx), dy: Number(dy) })
        return reply(200, { ok: true })
      } catch {
        return reply(400, { error: 'bad json' })
      }
    }

    reply(404, { error: 'not found' })
  })

  server.on('error', err => {
    if (err.code === 'EADDRINUSE') onTaken()
  })
  server.listen(port, '127.0.0.1')

  return server
}

module.exports = { serve }
