const assert = require('node:assert/strict')
const { test, mock } = require('node:test')

const { createAsks, ASK_TIMEOUT_MS } = require('../src/main/asks')

const prompt = (over = {}) => ({
  hook_event_name: 'PermissionRequest',
  session_id: 's1',
  tool_name: 'Bash',
  tool_input: { command: 'npm test' },
  permission_suggestions: [
    { type: 'addRules', rules: [{ toolName: 'Bash', ruleContent: 'npm test:*' }], behavior: 'allow', destination: 'session' },
  ],
  ...over,
})

function setup() {
  mock.timers.enable({ apis: ['setTimeout'] })
  const shown = []
  const asks = createAsks({ onChange: list => shown.push(list.map(a => a.id)) })
  const add = event => {
    const replies = []
    const id = asks.add(event, r => replies.push(r))
    return { id, replies }
  }
  return { asks, shown, add }
}

test('answering on the pet sends the decision and clears the prompt', t => {
  t.after(() => mock.timers.reset())
  const { asks, add } = setup()
  const a = add(prompt())

  assert.equal(asks.views().length, 1)
  assert.equal(asks.answer(a.id, { action: 'always' }), true)
  assert.equal(a.replies.length, 1)
  assert.equal(a.replies[0].hookSpecificOutput.decision.updatedPermissions[0].destination, 'session')
  assert.deepEqual(asks.views(), [])
  // Once only.
  assert.equal(asks.answer(a.id, { action: 'allow' }), false)
  assert.equal(a.replies.length, 1)
})

test('a choice that does not fit leaves the prompt waiting', t => {
  t.after(() => mock.timers.reset())
  const { asks, add } = setup()
  const a = add(prompt({ permission_suggestions: [] }))

  assert.equal(asks.answer(a.id, { action: 'always' }), false)
  assert.deepEqual(a.replies, [])
  assert.equal(asks.views().length, 1)
})

test('the same call finishing means the terminal answered: no decision, prompt gone', t => {
  t.after(() => mock.timers.reset())
  const { asks, add } = setup()
  const a = add(prompt())

  // Another tool finishing does not settle it.
  asks.seen({ hook_event_name: 'PostToolUse', session_id: 's1', tool_name: 'Read', tool_input: { file_path: 'x' } })
  asks.seen({ hook_event_name: 'PostToolUse', session_id: 's1', tool_name: 'Bash', tool_input: { command: 'ls' } })
  asks.seen({ hook_event_name: 'PostToolUse', session_id: 's2', tool_name: 'Bash', tool_input: { command: 'npm test' } })
  assert.equal(asks.views().length, 1)

  asks.seen({ hook_event_name: 'PostToolUse', session_id: 's1', tool_name: 'Bash', tool_input: { command: 'npm test' } })
  assert.deepEqual(a.replies, [{}])
  assert.deepEqual(asks.views(), [])
})

test('a question answered in the terminal comes back with answers added, and still matches', t => {
  t.after(() => mock.timers.reset())
  const { asks, add } = setup()
  const questions = [{ question: 'Q?', options: [{ label: 'A' }, { label: 'B' }] }]
  const a = add(prompt({ tool_name: 'AskUserQuestion', tool_input: { questions } }))

  asks.seen({ hook_event_name: 'PostToolUse', session_id: 's1', tool_name: 'AskUserQuestion', tool_input: { questions, answers: { 'Q?': 'A' } } })
  assert.deepEqual(a.replies, [{}])
})

test('the turn moving on settles every prompt of that session', t => {
  t.after(() => mock.timers.reset())
  const { asks, add } = setup()
  for (const name of ['Stop', 'StopFailure', 'UserPromptSubmit', 'SessionEnd']) {
    const a = add(prompt())
    const other = add(prompt({ session_id: 's2' }))
    asks.seen({ hook_event_name: name, session_id: 's1' })
    assert.deepEqual(a.replies, [{}], name)
    assert.deepEqual(other.replies, [], name)
    asks.dismiss(other.id)
  }
})

test('a newer prompt from the same agent replaces the older; other agents queue', t => {
  t.after(() => mock.timers.reset())
  const { asks, add } = setup()
  const first = add(prompt())
  const sub = add(prompt({ agent_id: 'agent-1' }))
  const second = add(prompt({ tool_input: { command: 'npm run build' } }))

  assert.deepEqual(first.replies, [{}])
  assert.deepEqual(sub.replies, [])
  assert.deepEqual(
    asks.views().map(v => v.id),
    [sub.id, second.id],
  )
})

test('a prompt nobody answers gives up before Claude Code does', t => {
  t.after(() => mock.timers.reset())
  const { asks, add } = setup()
  const a = add(prompt())

  mock.timers.tick(ASK_TIMEOUT_MS - 1)
  assert.deepEqual(a.replies, [])
  mock.timers.tick(1)
  assert.deepEqual(a.replies, [{}])
  assert.deepEqual(asks.views(), [])
  assert.ok(ASK_TIMEOUT_MS < 300 * 1000)
})

test('dismiss leaves it to the terminal; cancel forgets without answering', t => {
  t.after(() => mock.timers.reset())
  const { asks, add } = setup()
  const a = add(prompt())
  const b = add(prompt({ session_id: 's2' }))

  asks.dismiss(a.id)
  assert.deepEqual(a.replies, [{}])
  asks.cancel(b.id)
  assert.deepEqual(b.replies, [])
  assert.deepEqual(asks.views(), [])
})

test('what is not a prompt is not queued', t => {
  t.after(() => mock.timers.reset())
  const { asks } = setup()
  assert.equal(asks.add({ hook_event_name: 'PreToolUse', tool_name: 'Bash' }, () => {}), null)
  assert.deepEqual(asks.views(), [])
})
