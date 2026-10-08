const assert = require('node:assert/strict')
const { once } = require('node:events')
const { test } = require('node:test')

const { serve } = require('../src/main/server')
const { toMessage } = require('../src/shared/hook-events')

async function start(t) {
  const hooks = []
  const states = []
  const server = serve({
    port: 0,
    getState: () => ({ mood: 'idle' }),
    setState: msg => (['idle', 'working'].includes(msg?.mood) ? (states.push(msg), true) : false),
    onHook: event => hooks.push(event),
    snapshot: async () => Buffer.from('png'),
    onTaken: () => {},
  })
  await once(server, 'listening')
  t.after(() => server.close())
  return { url: `http://127.0.0.1:${server.address().port}`, hooks, states }
}

async function post(url, body) {
  const res = await fetch(url, { method: 'POST', headers: { 'content-type': 'application/json' }, body })
  return { status: res.status, body: await res.text() }
}

test('an HTTP hook gets {} back at once, and the event reaches the pet', async t => {
  const { url, hooks } = await start(t)
  const event = { hook_event_name: 'PermissionRequest', tool_name: 'Bash', tool_input: { command: 'ls' } }

  const res = await post(`${url}/hook?from=wakuwaku`, JSON.stringify(event))
  assert.deepEqual(res, { status: 200, body: '{}' })
  assert.deepEqual(hooks, [event])
  assert.deepEqual(toMessage(hooks[0]), { mood: 'waiting', detail: { key: 'detail.needsApproval', vars: { tool: 'Bash' } } })
})

test('a hook body we cannot read still gets {} and does nothing', async t => {
  const { url, hooks } = await start(t)

  for (const body of ['', 'not json', '{"a":']) {
    assert.deepEqual(await post(`${url}/hook`, body), { status: 200, body: '{}' }, body)
  }
  assert.deepEqual(hooks, [])
})

test('Chinese in a hook body survives', async t => {
  const { url, hooks } = await start(t)
  // Bytes split so a 3-byte character straddles two writes.
  const bytes = Buffer.from(JSON.stringify({ hook_event_name: 'TaskCompleted', task_subject: '写测试' }))

  const http = require('node:http')
  await new Promise((resolve, reject) => {
    const req = http.request(`${url}/hook`, { method: 'POST' }, res => {
      res.resume()
      res.on('end', resolve)
    })
    req.on('error', reject)
    const cut = bytes.indexOf(Buffer.from('测')) + 1
    req.write(bytes.subarray(0, cut))
    setTimeout(() => req.end(bytes.subarray(cut)), 20)
  })
  assert.equal(hooks[0].task_subject, '写测试')
})

test('/state, /health and unknown routes', async t => {
  const { url, states } = await start(t)

  assert.equal((await post(`${url}/state`, '{"mood":"working"}')).status, 200)
  assert.equal((await post(`${url}/state`, '{"mood":"nope"}')).status, 400)
  assert.equal((await post(`${url}/state`, 'junk')).status, 400)
  assert.deepEqual(states, [{ mood: 'working' }])

  const health = await (await fetch(`${url}/health`)).json()
  assert.equal(health.app, 'wakuwaku')
  assert.equal((await fetch(`${url}/nope`)).status, 404)
  // Debug routes are closed unless asked for.
  assert.equal((await post(`${url}/debug/look`, '{"dx":1,"dy":1}')).status, 404)
})

test('a prompt holds the hook request until it is answered', async t => {
  const hooks = []
  let answer
  const server = serve({
    port: 0,
    getState: () => ({}),
    setState: () => false,
    onHook: event => {
      hooks.push(event)
      return event.hook_event_name === 'PermissionRequest' ? new Promise(resolve => (answer = resolve)) : undefined
    },
    snapshot: async () => Buffer.alloc(0),
    onTaken: () => {},
  })
  await once(server, 'listening')
  t.after(() => server.close())
  const url = `http://127.0.0.1:${server.address().port}/hook`

  let settled = false
  const pending = post(url, JSON.stringify({ hook_event_name: 'PermissionRequest', tool_name: 'Bash' })).then(r => {
    settled = true
    return r
  })
  await new Promise(resolve => setTimeout(resolve, 100))
  assert.equal(settled, false)
  // Other events still answer at once meanwhile.
  assert.deepEqual(await post(url, '{"hook_event_name":"PreToolUse"}'), { status: 200, body: '{}' })

  const output = { hookSpecificOutput: { hookEventName: 'PermissionRequest', decision: { behavior: 'allow' } } }
  answer(output)
  assert.deepEqual(await pending, { status: 200, body: JSON.stringify(output) })
})

test('Claude Code hanging up tells onHook to forget the prompt', async t => {
  let hungUp
  const server = serve({
    port: 0,
    getState: () => ({}),
    setState: () => false,
    onHook: (event, signal) =>
      new Promise(() => {
        signal.addEventListener('abort', () => (hungUp = true))
      }),
    snapshot: async () => Buffer.alloc(0),
    onTaken: () => {},
  })
  await once(server, 'listening')
  t.after(() => server.close())

  const http = require('node:http')
  const req = http.request(`http://127.0.0.1:${server.address().port}/hook`, { method: 'POST' })
  req.on('error', () => {})
  req.end('{"hook_event_name":"PermissionRequest","tool_name":"Bash"}')
  await new Promise(resolve => setTimeout(resolve, 100))
  req.destroy()
  await new Promise(resolve => setTimeout(resolve, 100))
  assert.equal(hungUp, true)
})
