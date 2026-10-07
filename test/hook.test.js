const assert = require('node:assert/strict')
const { spawn } = require('node:child_process')
const http = require('node:http')
const path = require('node:path')
const { test } = require('node:test')

const { toMessage, EVENTS } = require('../hooks/claude-hook')

const HOOK = path.join(__dirname, '..', 'hooks', 'claude-hook.js')

test('each hook event becomes the right message', () => {
  const cases = [
    [{ hook_event_name: 'SessionStart', source: 'startup' }, { mood: 'idle', react: 'wave', say: '你好呀' }],
    [{ hook_event_name: 'SessionStart', source: 'resume' }, { mood: 'idle', react: 'wave', say: '你好呀' }],
    [{ hook_event_name: 'SessionStart', source: 'compact' }, null],
    [{ hook_event_name: 'SessionEnd', reason: 'exit' }, { mood: 'idle' }],
    [{ hook_event_name: 'UserPromptSubmit', prompt: 'hi' }, { mood: 'working', event: 'turn-start' }],
    [{ hook_event_name: 'PreToolUse', tool_name: 'Bash' }, { mood: 'working', detail: 'Bash' }],
    [{ hook_event_name: 'PreToolUse', tool_name: 'AskUserQuestion' }, { mood: 'waiting', detail: '有问题问你' }],
    [{ hook_event_name: 'PreToolUse', tool_name: 'ExitPlanMode' }, { mood: 'waiting', detail: '计划等你确认' }],
    [{ hook_event_name: 'PostToolUse', tool_name: 'Edit' }, { mood: 'working', detail: 'Edit', event: 'tool-done' }],
    [
      { hook_event_name: 'PostToolUseFailure', tool_name: 'Bash', error: 'exit 1' },
      { mood: 'working', detail: 'Bash', react: 'failed', say: 'Bash 失败了' },
    ],
    [{ hook_event_name: 'PostToolUseFailure', tool_name: 'Bash', is_interrupt: true }, null],
    [{ hook_event_name: 'PermissionRequest', tool_name: 'Bash' }, { mood: 'waiting', detail: 'Bash 需要你批准' }],
    [{ hook_event_name: 'Elicitation', mcp_server_name: 'github' }, { mood: 'waiting', detail: 'github 需要你填写' }],
    [{ hook_event_name: 'TaskCompleted', task_subject: '写测试' }, { react: 'jump', say: '完成：写测试' }],
    [{ hook_event_name: 'Stop' }, { mood: 'done' }],
    [{ hook_event_name: 'StopFailure', error: 'rate_limit' }, { mood: 'error' }],
    [{ hook_event_name: 'SubagentStart' }, null],
    [{}, null],
    [null, null],
  ]

  for (const [event, message] of cases) {
    assert.deepEqual(toMessage(event), message, JSON.stringify(event))
  }
})

test('every installed event maps to something', () => {
  for (const name of Object.keys(EVENTS)) {
    assert.notEqual(toMessage({ hook_event_name: name, tool_name: 'Bash', source: 'startup' }), null, name)
  }
  // The tool events, and only they, take a matcher.
  const tools = Object.entries(EVENTS)
    .filter(([, hasMatcher]) => hasMatcher)
    .map(([name]) => name)
  assert.deepEqual(tools.sort(), ['PermissionRequest', 'PostToolUse', 'PostToolUseFailure', 'PreToolUse'])
})

// Run the hook as Claude Code does: JSON on stdin, against a stand-in window.
function runHook(input, port) {
  return new Promise(resolve => {
    const child = spawn(process.execPath, [HOOK], {
      env: { ...process.env, CLAUDE_PETS_PORT: String(port), CLAUDE_PETS_AUTOSTART: '0' },
    })
    let stdout = ''
    child.stdout.on('data', chunk => (stdout += chunk))
    child.on('close', code => resolve({ code, stdout }))
    child.stdin.end(input)
  })
}

function standInWindow() {
  const posts = []
  const server = http.createServer((req, res) => {
    let body = ''
    req.setEncoding('utf8')
    req.on('data', chunk => (body += chunk))
    req.on('end', () => {
      if (req.url === '/state') posts.push(JSON.parse(body))
      res.writeHead(200, { 'content-type': 'application/json' })
      res.end(req.url === '/health' ? '{"ok":true}' : '{"ok":true}')
    })
  })
  return new Promise(resolve => server.listen(0, '127.0.0.1', () => resolve({ server, posts, port: server.address().port })))
}

test('the hook posts UTF-8 JSON, prints nothing and exits 0', async t => {
  const { server, posts, port } = await standInWindow()
  t.after(() => server.close())

  const run = await runHook(JSON.stringify({ hook_event_name: 'PermissionRequest', tool_name: 'Bash' }), port)
  assert.equal(run.code, 0)
  assert.equal(run.stdout, '')
  assert.deepEqual(posts, [{ mood: 'waiting', detail: 'Bash 需要你批准' }])
})

test('the hook stays quiet and exits 0 on junk, unknown events and a closed window', async t => {
  const { server, posts, port } = await standInWindow()
  t.after(() => server.close())

  for (const input of ['', 'not json', '{"hook_event_name":"SubagentStart"}']) {
    const run = await runHook(input, port)
    assert.equal(run.code, 0, input)
    assert.equal(run.stdout, '', input)
  }
  assert.deepEqual(posts, [])

  // Nothing listening on this port.
  const closed = await runHook('{"hook_event_name":"Stop"}', 1)
  assert.equal(closed.code, 0)
  assert.equal(closed.stdout, '')
})
