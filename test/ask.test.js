const assert = require('node:assert/strict')
const { test } = require('node:test')

const { viewOf, replyFor, summarize } = require('../src/shared/ask')

const BASH = {
  hook_event_name: 'PermissionRequest',
  session_id: 's1',
  cwd: 'D:/work/claude-pets',
  tool_name: 'Bash',
  tool_input: { command: 'npm test', description: 'Run tests' },
  permission_suggestions: [
    { type: 'addRules', rules: [{ toolName: 'Bash', ruleContent: 'npm test:*' }], behavior: 'allow', destination: 'localSettings' },
    { type: 'addRules', rules: [{ toolName: 'Bash', ruleContent: 'rm:*' }], behavior: 'deny', destination: 'localSettings' },
  ],
}

const QUESTION = {
  hook_event_name: 'PermissionRequest',
  session_id: 's1',
  tool_name: 'AskUserQuestion',
  tool_input: {
    questions: [
      {
        header: '颜色',
        question: '用什么颜色？',
        multiSelect: false,
        options: [{ label: '蓝', description: '冷静' }, { label: '红' }],
      },
      { header: '功能', question: '要哪些功能？', multiSelect: true, options: [{ label: '走动' }, { label: '注视' }, { label: '气泡' }] },
    ],
  },
}

const PLAN = { hook_event_name: 'PermissionRequest', session_id: 's1', tool_name: 'ExitPlanMode', tool_input: { plan: '1. 写代码\n2. 测试' } }

const decision = reply => reply.hookSpecificOutput.decision

test('a permission prompt shows the command, the project and the always-allow rule', () => {
  const view = viewOf(BASH)
  assert.equal(view.kind, 'permission')
  assert.equal(view.summary, 'npm test')
  assert.equal(view.project, 'claude-pets')
  assert.deepEqual(view.always, { rules: 'Bash(npm test:*)', where: '本项目（仅自己）' })
})

test('allow, always and deny answer as Claude Code expects', () => {
  assert.deepEqual(replyFor(BASH, { action: 'allow' }), {
    hookSpecificOutput: { hookEventName: 'PermissionRequest', decision: { behavior: 'allow' } },
  })
  // Always adds only the allow suggestions, never the deny one.
  assert.deepEqual(decision(replyFor(BASH, { action: 'always' })), {
    behavior: 'allow',
    updatedPermissions: [BASH.permission_suggestions[0]],
  })
  assert.equal(decision(replyFor(BASH, { action: 'deny' })).behavior, 'deny')
})

test('no always-allow without a suggestion to add', () => {
  const bare = { ...BASH, permission_suggestions: undefined }
  assert.equal(viewOf(bare).always, null)
  assert.equal(replyFor(bare, { action: 'always' }), null)
})

test('a question is answered by label, multi-select joined with commas', () => {
  const view = viewOf(QUESTION)
  assert.equal(view.kind, 'question')
  assert.equal(view.canAnswer, true)
  assert.equal(view.questions[1].multiSelect, true)

  const d = decision(replyFor(QUESTION, { action: 'answer', answers: [['蓝'], ['走动', '气泡']] }))
  assert.equal(d.behavior, 'allow')
  assert.deepEqual(d.updatedInput.answers, { '用什么颜色？': '蓝', '要哪些功能？': '走动, 气泡' })
  // The questions go back with the answers.
  assert.deepEqual(d.updatedInput.questions, QUESTION.tool_input.questions)
})

test('answers that do not fit the question are refused', () => {
  const bad = [
    [['蓝']], // one question short
    [['绿'], ['走动']], // no such option
    [['蓝', '红'], ['走动']], // two picks on a single-select
    [[], ['走动']], // nothing picked
    'nope',
  ]
  for (const answers of bad) assert.equal(replyFor(QUESTION, { action: 'answer', answers }), null, JSON.stringify(answers))
  // A plain allow carries no answers, so Claude Code would ignore it.
  assert.equal(replyFor(QUESTION, { action: 'allow' }), null)
})

test('a free-text question is left to the terminal', () => {
  const text = { ...QUESTION, tool_input: { questions: [{ question: '叫什么名字？', type: 'text' }] } }
  assert.equal(viewOf(text).canAnswer, false)
  assert.equal(replyFor(text, { action: 'answer', answers: [['x']] }), null)
})

test('a plan is approved with its input carried back', () => {
  const view = viewOf(PLAN)
  assert.equal(view.kind, 'plan')
  assert.match(view.summary, /写代码/)
  assert.deepEqual(decision(replyFor(PLAN, { action: 'allow' })), { behavior: 'allow', updatedInput: PLAN.tool_input })
  assert.equal(replyFor(PLAN, { action: 'always' }), null)
})

test('other events and junk are not prompts', () => {
  for (const e of [null, {}, { hook_event_name: 'PreToolUse', tool_name: 'Bash' }, { hook_event_name: 'PermissionRequest' }]) {
    assert.equal(viewOf(e), null)
    assert.equal(replyFor(e, { action: 'allow' }), null)
  }
  assert.equal(replyFor(BASH, { action: 'explode' }), null)
})

test('summaries pick the telling field and stay short', () => {
  assert.equal(summarize('Edit', { file_path: 'a.js', old_string: 'x' }), 'a.js')
  assert.equal(summarize('WebFetch', { url: 'https://x.y' }), 'https://x.y')
  assert.equal(summarize('mcp__github__create_issue', { title: 't' }), '{"title":"t"}')
  assert.equal(summarize('Bash', { command: 'x'.repeat(1000) }).length, 401)
})
