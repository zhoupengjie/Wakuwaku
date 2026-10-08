// How this copy of the app starts, for the SessionStart hook and start at
// login, and the job it can be started for instead of being the pet.
const { spawn } = require('child_process')
const fs = require('fs')
const http = require('http')
const os = require('os')
const path = require('path')
const { app } = require('electron')

const hooksConfig = require('../agents/claude-code/hooks')

// This copy, as a command and arguments: the built app on its own, or
// Electron running the source folder. The portable build runs from a
// temporary copy that changes every time; PORTABLE_EXECUTABLE_FILE is the
// .exe itself, which stays.
function launchSpec() {
  if (!app.isPackaged) return { command: process.execPath, args: [app.getAppPath()] }
  return { command: process.env.PORTABLE_EXECUTABLE_FILE || process.execPath, args: [] }
}

function claudeSettingsFile() {
  return path.join(process.env.CLAUDE_CONFIG_DIR || path.join(os.homedir(), '.claude'), 'settings.json')
}

function isUp(port) {
  return new Promise(resolve => {
    const req = http.get({ host: '127.0.0.1', port, path: '/health', timeout: 600 }, res => {
      res.resume()
      resolve(res.statusCode === 200)
    })
    req.on('timeout', () => req.destroy())
    req.on('error', () => resolve(false))
  })
}

// The hook's event on stdin, or null (none within ms, or not JSON). Read from
// fd 0 directly: in Electron's main process on Windows, process.stdin ends at
// once without the data.
function readEvent(ms) {
  return new Promise(resolve => {
    let data = ''
    let isDone = false
    const done = () => {
      if (isDone) return
      isDone = true
      clearTimeout(timer)
      try {
        resolve(JSON.parse(data))
      } catch {
        resolve(null)
      }
    }
    const timer = setTimeout(done, ms)
    try {
      const input = fs.createReadStream(null, { fd: 0, encoding: 'utf8' })
      input.on('data', chunk => {
        data += chunk
        if (data.length > 1e6) done()
      })
      input.on('end', done)
      input.on('error', done)
    } catch {
      done()
    }
  })
}

// Pass the event on, as an HTTP hook would (Claude Code runs none for SessionStart).
function forward(port, event) {
  return new Promise(resolve => {
    const body = Buffer.from(JSON.stringify(event))
    const req = http.request(
      { host: '127.0.0.1', port, path: '/hook?from=wakuwaku', method: 'POST', timeout: 800, headers: { 'content-type': 'application/json', 'content-length': body.length } },
      res => {
        res.resume()
        res.on('end', resolve)
      },
    )
    req.on('timeout', () => req.destroy())
    req.on('error', resolve)
    req.end(body)
  })
}

// Started by the SessionStart hook: if no pet answers on the port, start a
// normal copy, detached so it outlives the hook and the session (she greets
// on her own); if one does, hand her the event. Then leave. Quick and quiet,
// whatever happens.
async function ensureRunning(port) {
  try {
    const [event, up] = await Promise.all([readEvent(1000), isUp(port)])
    if (up) {
      if (event) await forward(port, event)
    } else {
      const { command, args } = launchSpec()
      spawn(command, args, { detached: true, stdio: 'ignore', env: process.env }).unref()
    }
  } catch {
    // Nothing to tell anyone: the session goes on without the pet.
  }
  app.exit(0)
}

function launchSpecForLogin() {
  const { command, args } = launchSpec()
  return { path: command, args }
}

module.exports = { ENSURE_FLAG: hooksConfig.ENSURE_FLAG, LEGACY_ENSURE_FLAG: hooksConfig.LEGACY_ENSURE_FLAG, launchSpec, launchSpecForLogin, claudeSettingsFile, isUp, ensureRunning }
