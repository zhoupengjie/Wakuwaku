// The local HTTP port the Claude Code hook reports to.
//
//   GET  /health       { ok, app, state }
//   POST /hook         a Claude Code hook event, as an HTTP hook sends it; answers {}
//   POST /state        a message (src/main/state.js): { mood, detail, event, react, say }
//   GET  /snapshot     the window as a PNG (debugging)
//   POST /debug/look   { dx, dy }: look as if the cursor were there (CLAUDE_PETS_DEBUG=1 only)
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

function serve({ port, getState, setState, onHook, snapshot, lookAt, onTaken }) {
  const server = http.createServer(async (req, res) => {
    const reply = (code, body) => {
      res.writeHead(code, { 'content-type': 'application/json' })
      res.end(JSON.stringify(body))
    }
    const route = `${req.method} ${req.url.split('?')[0]}`

    // Claude Code waits on this answer and reads it as the hook's output:
    // always at once, always {} (no decision), whatever came in.
    if (route === 'POST /hook') {
      try {
        onHook(await readJson(req))
      } catch {
        // Not an event we can read: nothing to show.
      }
      return reply(200, {})
    }

    if (req.method === 'GET' && req.url === '/health') {
      return reply(200, { ok: true, app: 'claude-pets', state: getState() })
    }

    if (req.method === 'GET' && req.url === '/snapshot') {
      try {
        const png = await snapshot()
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
