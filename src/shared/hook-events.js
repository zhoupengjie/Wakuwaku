// Claude Code hook events → pet messages. Shared by the window, which takes
// most events straight over HTTP, and hooks/claude-hook.js, which takes
// SessionStart (it may have to start the window first).

// How each event reaches the pet, for install-hooks:
//   matcher  the tool events take one
//   via      'http': Claude Code POSTs the event to the window, no process;
//            'command': runs hooks/claude-hook.js, in the background
const EVENTS = {
  SessionStart: { matcher: false, via: 'command' },
  SessionEnd: { matcher: false, via: 'http' },
  UserPromptSubmit: { matcher: false, via: 'http' },
  PreToolUse: { matcher: true, via: 'http' },
  PostToolUse: { matcher: true, via: 'http' },
  PostToolUseFailure: { matcher: true, via: 'http' },
  PermissionRequest: { matcher: true, via: 'http' },
  Elicitation: { matcher: false, via: 'http' },
  TaskCompleted: { matcher: false, via: 'http' },
  Stop: { matcher: false, via: 'http' },
  StopFailure: { matcher: false, via: 'http' },
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

module.exports = { EVENTS, toMessage }
