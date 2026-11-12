// Her plugin for OpenCode 2 (integrations/opencode/wakuwaku.js): which
// events it tells her of and how, what it puts in, and a permission's way
// from OpenCode to her and her answer back. Against a stand-in for her port
// (never her own: the plugin's port is replaced, as she does installing it).
import assert from 'node:assert/strict'
import fs from 'node:fs'
import http from 'node:http'
import type { AddressInfo } from 'node:net'
import path from 'node:path'
import { test } from 'node:test'

const TEMPLATE = fs.readFileSync(path.join(import.meta.dirname, '..', 'integrations', 'opencode', 'wakuwaku.js'), 'utf8')

// The plugin as she writes it for a port.
const load = port => import('data:text/javascript,' + encodeURIComponent(TEMPLATE.replace('const PORT = 47213', `const PORT = ${port}`)))

// Her port: what came, and an answer for each (a function of the body).
async function fakeHer(answer: (event: any) => any = () => ({})) {
  const got = []
  const server = http.createServer((req, res) => {
    let body = ''
    req.on('data', chunk => (body += chunk))
    req.on('end', async () => {
      const event = JSON.parse(body)
      got.push({ url: req.url, event })
      const out = await answer(event)
      res.setHeader('content-type', 'application/json')
      res.end(JSON.stringify(out))
    })
  })
  await new Promise<void>(done => server.listen(0, '127.0.0.1', done))
  return { got, port: (server.address() as AddressInfo).port, close: () => new Promise(done => server.close(done)) }
}

// What OpenCode hands a plugin, as much of it as the plugin uses.
interface OpenCodeCtx {
  location: { directory: string }
  event: { subscribe: (options: { signal: AbortSignal }) => AsyncGenerator<unknown> }
  permission: { reply: (input: unknown) => Promise<unknown> }
}

// OpenCode's side: the folder this copy is for, and the replies it was given.
function fakeOpenCode(directory = 'C:\\work\\oc3'): { replies: unknown[], ctx: OpenCodeCtx } {
  const replies = []
  return {
    replies,
    ctx: {
      location: { directory },
      event: { subscribe: () => (async function* () {})() },
      permission: { reply: async input => replies.push(input) },
    },
  }
}

const here = { directory: 'C:\\work\\oc3' }
const elsewhere = { directory: 'C:\\work\\other' }
const ev = (type, data, location = here) => ({ id: `evt_${Math.random()}`, type, data, ...(location ? { location } : {}) })

test('a turn as OpenCode 2.0.29 gives it, told in order with what only the plugin knows', async () => {
  const her = await fakeHer()
  const { make } = await load(her.port)
  const { ctx } = fakeOpenCode()
  const plugin = make(ctx)
  const s = 'ses_1'
  for (const e of [
    ev('session.created', { sessionID: s, location: here, slug: 'calm-moon' }),
    ev('session.inbox.enqueued', { sessionID: s, inboxID: 'msg_1', item: { type: 'user', payload: { text: 'Run the shell command: echo hi' }, delivery: 'steer' } }),
    // These say not where: known by the session.
    ev('session.execution.started', { sessionID: s }, null),
    ev('session.renamed', { sessionID: s, title: 'Shell echo command' }),
    ev('session.tool.input.started', { sessionID: s, id: 'call_1', name: 'shell' }),
    ev('session.tool.called', { sessionID: s, id: 'call_1', input: { command: 'echo hi' }, executed: false }),
    ev('session.tool.success', { sessionID: s, id: 'call_1', content: [{ type: 'text', text: 'hi' }] }),
    ev('session.text.ended', { sessionID: s, text: 'Done — the command output `hi`.' }),
    ev('session.execution.succeeded', { sessionID: s }, null),
    // Another folder's, told by its own copy.
    ev('session.created', { sessionID: 'ses_9', location: elsewhere }, elsewhere),
    ev('session.execution.started', { sessionID: 'ses_9' }, null),
  ]) {
    plugin.hear(e)
  }
  await plugin.idle()
  assert.ok(her.got.every(g => g.url === '/hook?from=wakuwaku&agent=opencode'))
  const told = her.got.map(g => g.event)
  assert.deepEqual(
    told.map(e => e.type),
    ['session.created', 'session.execution.started', 'session.tool.called', 'session.tool.success', 'session.execution.succeeded'],
  )
  assert.ok(told.every(e => e.sessionID === s && e.directory === here.directory))
  assert.equal(told[1].prompt, 'Run the shell command: echo hi')
  assert.deepEqual([told[2].tool, told[2].input, told[2].title], ['shell', { command: 'echo hi' }, 'Shell echo command'])
  assert.deepEqual([told[3].tool, told[3].id], ['shell', 'call_1'])
  assert.equal(told[4].text, 'Done — the command output `hi`.')
  plugin.close()
  await her.close()
})

test("a permission asked: her answer is OpenCode's reply; none leaves it to OpenCode", async () => {
  const answers = { per_1: { decision: 'always' }, per_2: {} }
  const her = await fakeHer(e => (e.type === 'permission.asked' ? answers[e.id] : {}))
  const { make } = await load(her.port)
  const { ctx, replies } = fakeOpenCode()
  const plugin = make(ctx)
  plugin.hear(ev('session.created', { sessionID: 'ses_1', location: here }))
  // A sub-agent's session: its own events not told, its permission as its main session's.
  plugin.hear(ev('session.created', { sessionID: 'ses_child', parentID: 'ses_1', location: here }))
  plugin.hear(ev('session.tool.input.started', { sessionID: 'ses_child', id: 'call_c', name: 'shell' }))
  plugin.hear(ev('session.tool.called', { sessionID: 'ses_child', id: 'call_c', input: { command: 'echo child' } }))
  plugin.hear(ev('permission.asked', { id: 'per_1', sessionID: 'ses_child', action: 'shell', resources: ['echo child'], save: ['echo *'], source: { type: 'tool', id: 'call_c' } }))
  plugin.hear(ev('permission.asked', { id: 'per_2', sessionID: 'ses_1', action: 'external_directory', resources: ['C:\\elsewhere\\*'] }))
  // Turned down at once (as `opencode run` does): never put to her.
  plugin.hear(ev('permission.asked', { id: 'per_3', sessionID: 'ses_1', action: 'shell', resources: ['rm x'] }))
  plugin.hear(ev('permission.replied', { sessionID: 'ses_1', requestID: 'per_3', reply: 'reject' }))
  await plugin.idle()
  await new Promise(done => setTimeout(done, 100))
  const told = her.got.map(g => g.event)
  assert.deepEqual(told.map(e => e.type), ['session.created', 'permission.asked', 'permission.asked', 'permission.replied'])
  const child = told[1]
  assert.deepEqual([child.sessionID, child.agentID, child.tool, child.input], ['ses_1', 'ses_child', 'shell', { command: 'echo child' }])
  assert.deepEqual(child.save, ['echo *'])
  assert.deepEqual([told[2].id, told[2].agentID, told[3].requestID], ['per_2', undefined, 'per_3'])
  // Replied to OpenCode as the sub-agent's own; the one she left alone not at all.
  assert.deepEqual(replies, [{ sessionID: 'ses_child', requestID: 'per_1', decision: 'always' }])
  plugin.close()
  await her.close()
})

test('with her not there nothing waits, and OpenCode is never answered', async () => {
  // A port no one listens on.
  const her = await fakeHer()
  await her.close()
  const { make } = await load(her.port)
  const { ctx, replies } = fakeOpenCode()
  const plugin = make(ctx)
  const started = Date.now()
  plugin.hear(ev('session.created', { sessionID: 'ses_1', location: here }))
  plugin.hear(ev('permission.asked', { id: 'per_1', sessionID: 'ses_1', action: 'shell', resources: ['ls'] }))
  for (let i = 0; i < 20; i++) plugin.hear(ev('session.execution.started', { sessionID: 'ses_1' }, null))
  await plugin.idle()
  // One try, then the rest dropped for a while, not tried one by one.
  assert.ok(Date.now() - started < 4000, `${Date.now() - started} ms`)
  assert.deepEqual(replies, [])
  plugin.close()
})

test('OpenCode loads it: a default export with an id and a setup, which listens until let go', async () => {
  const her = await fakeHer()
  const plugin = (await load(her.port)).default
  assert.equal(plugin.id, 'wakuwaku')
  let subscribed = 0
  const { ctx } = fakeOpenCode()
  ctx.event.subscribe = ({ signal }) => {
    subscribed++
    return (async function* () {
      yield ev('session.created', { sessionID: 'ses_1', location: here })
      await new Promise(done => signal.addEventListener('abort', done))
    })()
  }
  const cleanup = await plugin.setup(ctx)
  assert.equal(typeof cleanup, 'function')
  await new Promise(done => setTimeout(done, 200))
  assert.deepEqual(her.got.map(g => g.event.type), ['session.created'])
  await cleanup()
  await new Promise(done => setTimeout(done, 50))
  assert.equal(subscribed, 1)
  await her.close()
})
