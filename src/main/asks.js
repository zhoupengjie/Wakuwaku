// Permission prompts and questions waiting on the person, answered from the pet.
//
// Each holds an open HTTP hook request: respond() sends the hook output and
// ends it. Claude Code shows its own dialog meanwhile and keeps the first
// answer, so an ask also goes away (answered {}, no decision) once the session
// shows the dialog is gone: the same tool call finishing, the turn ending, or a
// newer prompt from the same agent. And before Claude Code's own timeout.
const { viewOf, replyFor } = require('../shared/ask')

// The longest a prompt can wait: under the 300 s timeout the PermissionRequest
// hook is installed with. The settings can make it shorter.
const ASK_TIMEOUT_MS = 290 * 1000

// Events that mean the session has moved past any dialog it showed.
const MOVED_ON = new Set(['UserPromptSubmit', 'Stop', 'StopFailure', 'SessionEnd'])

function sameInput(a, b) {
  const strip = v => {
    if (!v || typeof v !== 'object') return v
    const { answers, ...rest } = v
    return rest
  }
  return JSON.stringify(strip(a)) === JSON.stringify(strip(b))
}

// timeoutMs: a number, or a function giving it as each prompt arrives.
// lang(): the language for the note a deny leaves for Claude.
function createAsks({ onChange, timeoutMs = ASK_TIMEOUT_MS, lang = () => 'en', timers = globalThis }) {
  const waitMs = () => Math.min(ASK_TIMEOUT_MS, typeof timeoutMs === 'function' ? timeoutMs() : timeoutMs)
  let nextId = 1
  let list = []

  function changed() {
    onChange(views())
  }

  function finish(ask, reply) {
    if (!list.includes(ask)) return
    list = list.filter(a => a !== ask)
    timers.clearTimeout(ask.timer)
    if (reply !== undefined) ask.respond(reply)
    changed()
  }

  // A new ask; respond(output) answers its hook request. Returns its id, or
  // null when the event is not one the pet can show.
  function add(event, respond) {
    const view = viewOf(event)
    if (!view) return null

    // Prompts come one at a time per agent: a newer one means the last closed.
    for (const old of list.filter(a => a.view.session === view.session && a.view.agent === view.agent)) {
      finish(old, {})
    }

    const ask = { id: nextId++, event, view, respond, timer: undefined }
    ask.timer = timers.setTimeout(() => finish(ask, {}), waitMs())
    list = [...list, ask]
    changed()
    return ask.id
  }

  // The person's choice; false when there is no such ask or the choice does not fit it.
  function answer(id, choice) {
    const ask = list.find(a => a.id === id)
    if (!ask) return false
    const reply = replyFor(ask.event, choice, lang())
    if (!reply) return false
    finish(ask, reply)
    return true
  }

  // Leave it to the terminal.
  function dismiss(id) {
    const ask = list.find(a => a.id === id)
    if (ask) finish(ask, {})
  }

  // The hook request went away (Claude Code gave up on it): forget, no answer.
  function cancel(id) {
    const ask = list.find(a => a.id === id)
    if (ask) finish(ask, undefined)
  }

  // Any other hook event: drop the asks it shows were settled in the terminal.
  function seen(e) {
    if (!e?.session_id) return
    for (const ask of list.filter(a => a.view.session === e.session_id)) {
      const isSameCall =
        (e.hook_event_name === 'PostToolUse' || e.hook_event_name === 'PostToolUseFailure') &&
        (e.agent_id || '') === ask.view.agent &&
        e.tool_name === ask.view.tool &&
        sameInput(e.tool_input, ask.event.tool_input)
      if (isSameCall || MOVED_ON.has(e.hook_event_name)) finish(ask, {})
    }
  }

  function views() {
    return list.map(a => ({ id: a.id, ...a.view }))
  }

  // Hand every prompt to the terminal (do not disturb).
  function dismissAll() {
    for (const ask of [...list]) finish(ask, {})
  }

  return { add, answer, dismiss, dismissAll, cancel, seen, views }
}

module.exports = { createAsks, ASK_TIMEOUT_MS }
