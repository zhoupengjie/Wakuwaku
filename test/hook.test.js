const assert = require('node:assert/strict')
const { test } = require('node:test')

const { toMessage, EVENTS } = require('../src/shared/hook-events')

const base = { session_id: 's1', cwd: 'D:/work/claude-pets' }

test('each hook event becomes the right message, with its session and project', () => {
  const cases = [
    [{ hook_event_name: 'SessionStart', source: 'startup' }, { mood: 'idle', react: 'wave', say: { key: 'say.hello' } }],
    [{ hook_event_name: 'SessionStart', source: 'resume' }, { mood: 'idle', react: 'wave', say: { key: 'say.hello' } }],
    [{ hook_event_name: 'SessionEnd', reason: 'exit' }, { mood: 'idle', event: 'session-end' }],
    [{ hook_event_name: 'UserPromptSubmit', prompt: 'hi' }, { mood: 'working', event: 'turn-start' }],
    [{ hook_event_name: 'PreToolUse', tool_name: 'Bash' }, { mood: 'working', detail: 'Bash' }],
    [{ hook_event_name: 'PreToolUse', tool_name: 'AskUserQuestion' }, { mood: 'waiting', detail: { key: 'detail.asksQuestion' } }],
    [{ hook_event_name: 'PreToolUse', tool_name: 'ExitPlanMode' }, { mood: 'waiting', detail: { key: 'detail.planToConfirm' } }],
    [{ hook_event_name: 'PostToolUse', tool_name: 'Edit' }, { mood: 'working', detail: 'Edit', event: 'tool-done' }],
    [
      { hook_event_name: 'PostToolUseFailure', tool_name: 'Bash', error: 'exit 1' },
      { mood: 'working', detail: 'Bash', react: 'failed', say: { key: 'say.toolFailed', vars: { tool: 'Bash' } } },
    ],
    [{ hook_event_name: 'PermissionRequest', tool_name: 'Bash' }, { mood: 'waiting', detail: { key: 'detail.needsApproval', vars: { tool: 'Bash' } } }],
    [{ hook_event_name: 'PermissionRequest', tool_name: 'AskUserQuestion' }, { mood: 'waiting', detail: { key: 'detail.asksQuestion' } }],
    [{ hook_event_name: 'Elicitation', mcp_server_name: 'github' }, { mood: 'waiting', detail: { key: 'detail.needsInput', vars: { server: 'github' } } }],
    [{ hook_event_name: 'TaskCompleted', task_subject: '写测试' }, { react: 'jump', say: { key: 'say.taskDone', vars: { task: '写测试' } } }],
    [{ hook_event_name: 'TaskCompleted' }, { react: 'jump', say: { key: 'say.taskDoneGeneric' } }],
    [{ hook_event_name: 'Stop' }, { mood: 'done' }],
    [{ hook_event_name: 'StopFailure', error: 'rate_limit' }, { mood: 'error' }],
  ]

  for (const [event, message] of cases) {
    assert.deepEqual(toMessage({ ...base, ...event }), { ...message, session: 's1', project: 'claude-pets' }, JSON.stringify(event))
  }
})

test('events that change nothing, and junk, give no message', () => {
  for (const event of [
    { hook_event_name: 'SessionStart', source: 'compact' },
    { hook_event_name: 'PostToolUseFailure', tool_name: 'Bash', is_interrupt: true },
    { hook_event_name: 'SubagentStart' },
    {},
    null,
    'Stop',
  ]) {
    assert.equal(toMessage(event), null, JSON.stringify(event))
  }
})

test('a message without a session or folder simply has neither', () => {
  assert.deepEqual(toMessage({ hook_event_name: 'Stop' }), { mood: 'done' })
})

test('every installed event maps to something; only the tool events take a matcher', () => {
  for (const name of Object.keys(EVENTS)) {
    assert.notEqual(toMessage({ hook_event_name: name, tool_name: 'Bash', source: 'startup' }), null, name)
  }
  const tools = Object.entries(EVENTS)
    .filter(([, e]) => e.matcher)
    .map(([name]) => name)
  assert.deepEqual(tools.sort(), ['PermissionRequest', 'PostToolUse', 'PostToolUseFailure', 'PreToolUse'])
  assert.equal(EVENTS.PermissionRequest.timeout, 300)
})
