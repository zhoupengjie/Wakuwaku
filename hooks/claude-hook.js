#!/usr/bin/env node
// Claude Code command hook for claude-pets: SessionStart.
//
// Every other event goes straight to the window as an HTTP hook, with no
// process at all; this one also has to start the window when it is not up.
// Installed to run in the background (async), so it never holds up a session.
//
// It reads the event's JSON on stdin, prints nothing (SessionStart's stdout
// joins the conversation) and always exits 0. It takes any other event too,
// which is handy by hand:
//
//   echo '{"hook_event_name":"PreToolUse","tool_name":"Read"}' | node hooks/claude-hook.js
const { spawn } = require('child_process')
const path = require('path')

const { toMessage } = require('../src/shared/hook-events')

const ROOT = path.join(__dirname, '..')

function url() {
  return `http://127.0.0.1:${Number(process.env.CLAUDE_PETS_PORT || 47213)}`
}

function readStdin() {
  return new Promise(resolve => {
    let data = ''
    process.stdin.setEncoding('utf8')
    process.stdin.on('data', chunk => (data += chunk))
    process.stdin.on('end', () => resolve(data))
    process.stdin.on('error', () => resolve(data))
  })
}

async function isUp() {
  try {
    return (await fetch(`${url()}/health`, { signal: AbortSignal.timeout(500) })).ok
  } catch {
    return false
  }
}

// Start the window, detached so it outlives this hook and the session.
function launch() {
  const electron = require(path.join(ROOT, 'node_modules', 'electron'))
  spawn(electron, [ROOT], { detached: true, stdio: 'ignore' }).unref()
}

async function main() {
  const e = JSON.parse(await readStdin())
  const message = toMessage(e)

  // A new window greets on its own; it needs no message.
  if (e.hook_event_name === 'SessionStart' && message && process.env.CLAUDE_PETS_AUTOSTART !== '0' && !(await isUp())) {
    launch()
    return
  }

  if (!message) {
    return
  }

  await fetch(`${url()}/state`, {
    method: 'POST',
    headers: { 'content-type': 'application/json; charset=utf-8' },
    body: JSON.stringify(message),
    signal: AbortSignal.timeout(800),
  })
}

if (require.main === module) {
  main()
    .catch(() => {
      // The window is closed, or the payload was not what we expect.
    })
    .finally(() => process.exit(0))
}
