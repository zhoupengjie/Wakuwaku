// Her extension for oh-my-pi (integrations/omp/wakuwaku.ts), run against a
// pretend omp and a pretend her: the handlers it registers, called as omp
// 18.11.1 calls them, and a port of our own that keeps what comes. What
// reaches her, in what order; nothing waited on, nothing thrown; the plan
// review answered from her.
const assert = require('node:assert/strict')
const fs = require('node:fs')
const http = require('node:http')
const os = require('node:os')
const path = require('node:path')
const { pathToFileURL } = require('node:url')
const { test } = require('node:test')

const SOURCE = fs.readFileSync(path.join(__dirname, '..', 'integrations', 'omp', 'wakuwaku.ts'), 'utf8')
let loads = 0

// The file as she writes it (connection.rs omp_extension), on this port.
function render(port, exe = '', data = '') {
  return SOURCE.replace('"__WAKUWAKU_PORT__"', `"${port}"`).replace('"__WAKUWAKU_EXE__"', JSON.stringify(exe)).replace('"__WAKUWAKU_DATA__"', JSON.stringify(data))
}

// A fresh copy of the module (its shared state gone) on a pretend omp.
async function load(port, exe, data) {
  delete globalThis.__wakuwakuOmp1
  const file = path.join(os.tmpdir(), `wakuwaku-omp-test-${process.pid}-${++loads}.mjs`)
  fs.writeFileSync(file, render(port, exe, data))
  try {
    const mod = await import(pathToFileURL(file).href)
    const handlers = {}
    mod.default({ on: (name, fn) => (handlers[name] = fn) })
    return handlers
  } finally {
    fs.rmSync(file, { force: true })
  }
}

// Her port: each event kept; answer(event) says what comes back (an object,
// or null to drop the connection).
async function her(answer = () => ({})) {
  const calls = []
  const server = http.createServer((req, res) => {
    let text = ''
    req.on('data', d => (text += d))
    req.on('end', async () => {
      const body = JSON.parse(text)
      calls.push({ url: req.url, body })
      const out = await answer(body)
      if (out === null) return req.socket.destroy()
      res.writeHead(200, { 'content-type': 'application/json' })
      res.end(JSON.stringify(out))
    })
  })
  await new Promise(done => server.listen(0, '127.0.0.1', done))
  return { port: server.address().port, calls, close: () => new Promise(done => server.close(done)) }
}

function ctx({ kind = 'main', id = 's1', review } = {}) {
  const resolved = []
  return {
    agent: { kind, id: kind === 'main' ? 'Main' : 'Sub', depth: kind === 'main' ? 0 : 1 },
    cwd: 'C:\\waku\\omp\\proj',
    sessionManager: { getSessionId: () => id, getCwd: () => 'C:\\waku\\omp\\proj', getSessionName: () => undefined },
    getPendingPlanReview: () => review && review(),
    resolvePlanReview: (...args) => resolved.push(args) > 0,
    resolved,
  }
}

const settle = async (n = 10) => {
  for (let i = 0; i < n; i++) await new Promise(done => setTimeout(done, 10))
}
const names = calls => calls.map(c => c.body.hook_event_name)

test('a turn in omp reaches her as Claude Code events, in order, waited on by nothing', async () => {
  const port = await her()
  const on = await load(port.port)
  const c = ctx()
  // Nothing returned to wait on, but the last word.
  assert.equal(on.session_start({ type: 'session_start' }, c), undefined)
  assert.equal(on.before_agent_start({ type: 'before_agent_start', prompt: 'Run echo hi' }, c), undefined)
  on.tool_execution_start({ toolCallId: 't1', toolName: 'edit', args: { path: 'a.txt', old_string: 'hi', new_string: 'bye' }, intent: 'Changing hi to bye' }, c)
  on.tool_execution_end({ toolCallId: 't1', toolName: 'edit', result: { content: [{ type: 'text', text: '1:bye' }], details: { path: 'C:\\waku\\omp\\proj\\a.txt' } }, isError: false }, c)
  on.tool_execution_start({ toolCallId: 't2', toolName: 'bash', args: { command: 'false' } }, c)
  on.tool_execution_end({ toolCallId: 't2', toolName: 'bash', result: { content: [{ type: 'text', text: 'exit code 1' }] }, isError: true }, c)
  // Going on (async work): no end yet.
  on.agent_end({ messages: [], willContinue: true }, c)
  on.agent_end({ messages: [{ role: 'user', content: 'x' }, { role: 'assistant', stopReason: 'stop', content: [{ type: 'text', text: 'Done.' }] }] }, c)
  await settle()
  const last = on.session_shutdown({ type: 'session_shutdown' }, c)
  assert.ok(last instanceof Promise, 'the last word is waited on while she is up')
  await last
  await port.close()
  const { calls } = port
  assert.deepEqual(names(calls), ['SessionStart', 'UserPromptSubmit', 'PreToolUse', 'PostToolUse', 'PreToolUse', 'PostToolUseFailure', 'Stop', 'SessionEnd'])
  assert.ok(calls.every(x => x.url === '/hook?from=wakuwaku&agent=omp'))
  assert.ok(calls.every(x => x.body.session_id === 's1' && x.body.cwd === 'C:\\waku\\omp\\proj'))
  assert.equal(calls[0].body.source, 'startup')
  assert.equal(calls[1].body.prompt, 'Run echo hi')
  assert.deepEqual(calls[2].body.tool_input, { path: 'a.txt', old_string: 'hi', new_string: 'bye' })
  assert.equal(calls[2].body.intent, 'Changing hi to bye')
  // The end brings the start's arguments and the file as omp resolved it.
  assert.deepEqual([calls[3].body.tool_input.path, calls[3].body.file, calls[3].body.tool_use_id], ['a.txt', 'C:\\waku\\omp\\proj\\a.txt', 't1'])
  assert.equal(calls[5].body.error, 'exit code 1')
  assert.equal(calls[6].body.last_assistant_message, 'Done.')
})

test('a subagent is not followed; errors and stops say so; long arguments are cut', async () => {
  const port = await her()
  const on = await load(port.port)
  const sub = ctx({ kind: 'sub', id: 'SayHi' })
  on.session_start({}, sub)
  on.before_agent_start({ prompt: 'Complete assignment' }, sub)
  on.agent_end({ messages: [{ role: 'assistant', content: [{ type: 'text', text: 'hi' }] }] }, sub)
  on.session_shutdown({}, sub)
  await settle()
  assert.deepEqual(port.calls, [])

  const c = ctx()
  on.tool_execution_start({ toolCallId: 'w', toolName: 'write', args: { path: 'big.txt', content: 'x'.repeat(10000) } }, c)
  on.agent_end({ messages: [{ role: 'assistant', stopReason: 'error', errorMessage: 'Connection error.', content: [] }] }, c)
  on.agent_end({ messages: [{ role: 'assistant', stopReason: 'aborted', content: [] }] }, c)
  await settle()
  await port.close()
  assert.deepEqual(names(port.calls), ['PreToolUse', 'StopFailure', 'Interrupt'])
  assert.equal(port.calls[0].body.tool_input.content.length, 2000)
  assert.equal(port.calls[0].body.tool_input_truncated, true)
  assert.equal(port.calls[1].body.error_details, 'Connection error.')
})

test('an edit reaches her with its text whole, up to 64 KB (zhoupengjie/Wakuwaku#73)', async () => {
  const port = await her()
  const on = await load(port.port)
  const c = ctx()
  const long = 'a'.repeat(5000)
  on.tool_execution_start({ toolCallId: 'e', toolName: 'edit', args: { path: 'notes.md', oldText: long, newText: `${long}b` } }, c)
  on.tool_execution_start({ toolCallId: 'm', toolName: 'edit', args: { path: 'notes.md', edits: [{ old_string: 'x'.repeat(70000), new_string: 'y' }] } }, c)
  await settle()
  await port.close()
  const [whole, cut] = port.calls.map(call => call.body)
  assert.deepEqual([whole.tool_input.oldText.length, whole.tool_input.newText.length, 'tool_input_truncated' in whole], [5000, 5001, false])
  assert.deepEqual([cut.tool_input.edits[0].old_string.length, cut.tool_input.edits[0].new_string, cut.tool_input_truncated], [64 * 1024, 'y', true])
})

test('a session switched ends the one before', async () => {
  const port = await her()
  const on = await load(port.port)
  on.session_start({}, ctx({ id: 'a' }))
  on.session_switch({}, ctx({ id: 'b' }))
  on.session_branch({}, ctx({ id: 'b' }))
  await settle()
  await port.close()
  assert.deepEqual(
    port.calls.map(x => [x.body.hook_event_name, x.body.session_id]),
    [['SessionStart', 'a'], ['SessionEnd', 'a'], ['SessionStart', 'b'], ['SessionStart', 'b']],
  )
})

test('her not up: nothing thrown or left unhandled, and for a while nothing tried', async () => {
  let unhandled = 0
  const count = () => unhandled++
  process.on('unhandledRejection', count)
  try {
    // A port that takes the connection and drops it, counting.
    let tried = 0
    const dropping = http.createServer()
    dropping.on('connection', socket => {
      tried++
      socket.destroy()
    })
    await new Promise(done => dropping.listen(0, '127.0.0.1', done))
    const on = await load(dropping.address().port)
    const c = ctx()
    on.session_start({}, c)
    on.before_agent_start({ prompt: 'hi' }, c)
    await settle()
    on.tool_execution_start({ toolCallId: 't', toolName: 'read', args: { path: 'a' } }, c)
    on.agent_end({ messages: [] }, c)
    // Not waited on as omp exits: she never answered.
    assert.equal(on.session_shutdown({}, c), undefined)
    await settle()
    // The first tried and failed; the one queued behind it given up; none after.
    assert.equal(tried, 1)
    await new Promise(done => dropping.close(done))

    // Nobody on the port at all, and an argument JSON cannot say: still nothing thrown.
    const looped = {}
    looped.self = looped
    const on2 = await load(dropping.address()?.port || 1)
    assert.equal(on2.tool_execution_start({ toolCallId: 'x', toolName: 'bash', args: looped }, c), undefined)
    assert.equal(on2.session_start({}, c), undefined)
    assert.equal(on2.plan_review_requested({ reviewId: 'r', planContent: 'p' }, c), undefined)
    await settle()
  } finally {
    process.off('unhandledRejection', count)
  }
  assert.equal(unhandled, 0)
})

// Her not up as a session starts: her exe run once to start her (it does
// not, should the person have quit her: hooks.rs ensure_running), on her port
// and with her data folder, told it is omp's and how the session started,
// for her log's "started by" (starts.rs ensure_who).
test('her not up as a session starts: her exe asked to start her, saying it is omp', async () => {
  const cp = require('node:child_process')
  const { syncBuiltinESMExports } = require('node:module')
  const real = cp.spawn
  const runs = []
  cp.spawn = (exe, args, options) => {
    runs.push({ exe, args, options })
    return { on() {}, unref() {} }
  }
  syncBuiltinESMExports()
  const dropping = http.createServer()
  dropping.on('connection', socket => socket.destroy())
  try {
    await new Promise(done => dropping.listen(0, '127.0.0.1', done))
    const port = dropping.address().port
    const on = await load(port, 'C:\\waku\\pet\\wakuwaku.exe', 'C:\\waku\\data')
    on.session_switch({}, ctx())
    await settle()
    assert.equal(runs.length, 1)
    assert.equal(runs[0].exe, 'C:\\waku\\pet\\wakuwaku.exe')
    assert.deepEqual(runs[0].args, ['--wakuwaku-ensure-running', 'omp', 'resume'])
    assert.equal(runs[0].options.env.WAKUWAKU_PORT, String(port))
    assert.equal(runs[0].options.env.WAKUWAKU_USER_DATA, 'C:\\waku\\data')
    assert.equal(runs[0].options.detached, true)
    // Once in an omp: another session starting runs it no more.
    on.session_start({}, ctx({ id: 's2' }))
    await settle()
    assert.equal(runs.length, 1)
  } finally {
    cp.spawn = real
    syncBuiltinESMExports()
    await new Promise(done => dropping.close(done))
  }
})

test('a plan review is answered on her, unless the terminal answered first', async () => {
  let decision = { behavior: 'allow' }
  const port = await her(body => (body.hook_event_name === 'PermissionRequest' ? { hookSpecificOutput: { hookEventName: 'PermissionRequest', decision } } : {}))
  const on = await load(port.port)
  let open = 'r1'
  const c = ctx({ review: () => (open ? { reviewId: open } : undefined) })
  const review = { type: 'plan_review_requested', reviewId: 'r1', sessionId: 's1', title: 'Hello', planFilePath: 'local://hello-plan.md', planContent: '1. Write hello.txt', options: [] }
  assert.equal(on.plan_review_requested(review, c), undefined)
  await settle()
  const { calls } = port
  assert.deepEqual(names(calls), ['PermissionRequest'])
  assert.deepEqual([calls[0].body.tool_name, calls[0].body.tool_input, calls[0].body.tool_use_id], ['ExitPlanMode', { plan: '1. Write hello.txt' }, 'r1'])
  assert.deepEqual(c.resolved, [['r1', 'execute']])
  // omp says it was resolved (by her): she hears it, as for one answered in the terminal.
  on.plan_review_resolved({ type: 'plan_review_resolved', reviewId: 'r1', choice: 'execute', by: 'extension' }, c)
  await settle()
  assert.deepEqual(calls[1].body, { session_id: 's1', cwd: 'C:\\waku\\omp\\proj', hook_event_name: 'PostToolUse', tool_name: 'ExitPlanMode', tool_input: { plan: '1. Write hello.txt' }, tool_use_id: 'r1', choice: 'execute', by: 'extension' })

  // Turned down on her: refined with her words.
  decision = { behavior: 'deny', message: 'The user denied this on the desktop pet.' }
  open = 'r2'
  on.plan_review_requested({ ...review, reviewId: 'r2' }, c)
  await settle()
  assert.deepEqual(c.resolved[1], ['r2', 'refine', { feedback: 'The user denied this on the desktop pet.' }])

  // Answered in the terminal before her answer came: hers is not used.
  open = undefined
  on.plan_review_requested({ ...review, reviewId: 'r3' }, c)
  await settle()
  assert.equal(c.resolved.length, 2)
  // Handed back to the terminal ({}): nothing resolved either.
  decision = undefined
  open = 'r4'
  on.plan_review_requested({ ...review, reviewId: 'r4' }, c)
  await settle()
  await port.close()
  assert.equal(c.resolved.length, 2)
})

test('the file as she writes it is still a module, whatever the exe path', async () => {
  const port = await her()
  const on = await load(port.port, 'C:\\Program Files\\Wakuwaku\\it\'s "her".exe', 'C:\\waku\\omp\\data')
  on.session_start({}, ctx())
  await settle()
  await port.close()
  assert.deepEqual(names(port.calls), ['SessionStart'])
})
