// Claude Code hook events → pet messages (see src/main/state.js), and how
// each event is installed (src/agents/claude-code/hooks.js).
//
// Texts are { key, vars } for src/shared/i18n.js, so the pet says them in the
// language she is set to. Each message names its session and project, so
// several Claude Code sessions each have their own mood.
const path = require('path')

// How each event is installed:
//   matcher  the tool events take one
//   timeout  seconds Claude Code waits for the answer (default 2); a prompt
//            waits for the person to answer it on the pet
const EVENTS = {
  SessionStart: { matcher: false },
  SessionEnd: { matcher: false },
  UserPromptSubmit: { matcher: false },
  PreToolUse: { matcher: true },
  PostToolUse: { matcher: true },
  PostToolUseFailure: { matcher: true },
  PermissionRequest: { matcher: true, timeout: 300 },
  Elicitation: { matcher: false },
  TaskCompleted: { matcher: false },
  Stop: { matcher: false },
  StopFailure: { matcher: false },
}

// What a tool that stops for the person is waiting on.
const ASKS_PERSON = {
  AskUserQuestion: { key: 'detail.asksQuestion' },
  ExitPlanMode: { key: 'detail.planToConfirm' },
}

// The mood part of the message for an event, or null for one that changes nothing.
function moodOf(e) {
  switch (e.hook_event_name) {
    case 'SessionStart':
      // A compaction starts a new context mid-work: nothing changed for the pet.
      return e.source === 'compact' ? null : { mood: 'idle', react: 'wave', say: { key: 'say.hello' } }
    case 'SessionEnd':
      return { mood: 'idle', event: 'session-end' }
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
      return e.is_interrupt
        ? null
        : { mood: 'working', detail: e.tool_name, react: 'failed', say: { key: 'say.toolFailed', vars: { tool: e.tool_name } } }
    case 'PermissionRequest':
      return {
        mood: 'waiting',
        detail: ASKS_PERSON[e.tool_name] || { key: 'detail.needsApproval', vars: { tool: e.tool_name } },
      }
    case 'Elicitation':
      return { mood: 'waiting', detail: { key: 'detail.needsInput', vars: { server: e.mcp_server_name } } }
    case 'TaskCompleted':
      return {
        react: 'jump',
        say: e.task_subject ? { key: 'say.taskDone', vars: { task: e.task_subject } } : { key: 'say.taskDoneGeneric' },
      }
    case 'Stop':
      return { mood: 'done' }
    case 'StopFailure':
      return { mood: 'error' }
    default:
      return null
  }
}

// The message for an event, or null for one that changes nothing.
function toMessage(e) {
  if (!e || typeof e !== 'object') return null
  const message = moodOf(e)
  if (!message) return null
  if (typeof e.session_id === 'string' && e.session_id) message.session = e.session_id
  if (typeof e.cwd === 'string' && e.cwd) message.project = path.basename(e.cwd)
  return message
}

module.exports = { EVENTS, toMessage }
