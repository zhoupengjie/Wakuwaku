#!/usr/bin/env node
// Claude Code command hook for claude-pets.
//
// Claude Code runs this on each hooked event with the event's JSON on stdin;
// it turns the event into a message and posts it to the pet window. It prints
// nothing (some events add stdout to the conversation) and always exits 0, so
// a closed window or a bad payload never gets in Claude's way.
//
// Installed by `npm run install-hooks`; see scripts/install-hooks.js.
const { spawn } = require('child_process')
const path = require('path')

const ROOT = path.join(__dirname, '..')

// The events this hook takes, for install-hooks: true where they take a matcher.
const EVENTS = {
  SessionStart: false,
  SessionEnd: false,
  UserPromptSubmit: false,
  PreToolUse: true,
  PostToolUse: true,
  PostToolUseFailure: true,
  PermissionRequest: true,
  Elicitation: false,
  TaskCompleted: false,
  Stop: false,
  StopFailure: false,
}

// What a tool that stops for the person is waiting on.
const ASKS_PERSON = {
  AskUserQuestion: '有问题问你',
  ExitPlanMode: '计划等你确认',
}

// The message for an event, or null for one that changes nothing.
// See src/main/state.js for what a message holds.
function toMessage(e) {
  switch (e?.hook_event_name) {
    case 'SessionStart':
      // A compaction starts a new context mid-work: nothing changed for the pet.
      return e.source === 'compact' ? null : { mood: 'idle', react: 'wave', say: '你好呀' }
    case 'SessionEnd':
      return { mood: 'idle' }
    case 'UserPromptSubmit':
      return { mood: 'working', event: 'turn-start' }
    case 'PreToolUse':
      return e.tool_name in ASKS_PERSON
        ? { mood: 'waiting', detail: ASKS_PERSON[e.tool_name] }
        : { mood: 'working', detail: e.tool_name }
    case 'PostToolUse':
      return { mood: 'working', detail: e.tool_name, event: 'tool-done' }
    case 'PostToolUseFailure':
      // An interrupt is the person stopping it, not the tool failing.
      return e.is_interrupt ? null : { mood: 'working', detail: e.tool_name, react: 'failed', say: `${e.tool_name} 失败了` }
    case 'PermissionRequest':
      return { mood: 'waiting', detail: `${e.tool_name} 需要你批准` }
    case 'Elicitation':
      return { mood: 'waiting', detail: `${e.mcp_server_name} 需要你填写` }
    case 'TaskCompleted':
      return { react: 'jump', say: e.task_subject ? `完成：${e.task_subject}` : '完成一项' }
    case 'Stop':
      return { mood: 'done' }
    case 'StopFailure':
      return { mood: 'error' }
    default:
      return null
  }
}

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

module.exports = { toMessage, EVENTS }
