// Her plugin for DeepSeek Harness (integrations/dsh/wakuwaku.js): which of
// DSH's points it listens on and how (first in line, back at once), what it
// tells her and in which words (Claude Code's hooks', as DSH's own bridge
// said them), what only it sees (the last words, the name, whose a session
// is, a turn that ended unfinished), its word on loading, and that nothing of
// hers ever reaches DSH: no wait, no error. DSH's approvals and questions:
// told with the call's input, the first answer winning (hers withdraws DSH's
// own dialog through the request's signal), DSH as it was when she is not
// there. Against a stand-in for her port and a stand-in for DSH's context
// (never DSH itself).
import assert from 'node:assert/strict'
import fs from 'node:fs'
import http from 'node:http'
import type { AddressInfo } from 'node:net'
import os from 'node:os'
import path from 'node:path'
import { test } from 'node:test'
import urlModule from 'node:url'
import cryptoModule from 'node:crypto'

const SOURCE = fs.readFileSync(path.join(import.meta.dirname, '..', 'integrations', 'dsh', 'wakuwaku.js'), 'utf8')
const load = () => import('data:text/javascript,' + encodeURIComponent(SOURCE))

// Her port: what came, in order. answer(event): what she gives back for it
// ({} by default), or a promise of it (held till then: a prompt); dropped,
// whether a held one's connection was let go before she answered.
async function fakeHer(answer: (event: any) => any = () => ({})) {
  const got = []
  const dropped = []
  const server = http.createServer((req, res) => {
    let body = ''
    req.on('data', chunk => (body += chunk))
    req.on('end', async () => {
      const event = JSON.parse(body)
      got.push({ url: req.url, event })
      res.on('close', () => res.writableEnded || dropped.push(event.hook_event_name))
      const out = await answer(event)
      if (res.destroyed) return
      res.setHeader('content-type', 'application/json')
      res.end(JSON.stringify(out))
    })
  })
  await new Promise<void>(done => server.listen(0, '127.0.0.1', done))
  const close = () => {
    server.closeAllConnections()
    return new Promise(done => server.close(done))
  }
  return { got, dropped, port: (server.address() as AddressInfo).port, close }
}

// DSH's side: the listeners a plugin registers (and whether first in line),
// its effects, its services.
function fakeDsh(services = {}) {
  const on = {}
  const effects = []
  return {
    on,
    effects,
    ctx: {
      on: (name, fn, prepend) => (on[name] = { fn, prepend: !!prepend }),
      effect: (fn, label) => effects.push({ dispose: fn(), label }),
      get: key => services[key],
    },
  }
}

// A marker the plugin writes in the background, once whole (it may be there
// and still empty for a moment): its word, or what failed after 5 s.
async function written(marker) {
  for (let i = 0; ; i++) {
    try {
      return JSON.parse(fs.readFileSync(marker, 'utf8'))
    } catch (e) {
      if (i >= 500) throw e
      await new Promise(done => setTimeout(done, 10))
    }
  }
}

const WORKSPACE = 'C:\\Users\\waku\\Documents\\deepseek-harness\\default-workspace'
const session = (id, more = {}) => ({ id, header: { type: 'session', version: 4, id, cwd: WORKSPACE, createdAt: 1, isSeeded: false, delegationDepth: 0, ...more } })
const agentOf = s => ({ session: s })
const ev = (type, data) => ({ type, seq: 0, time: 0, data })
// What a waterfall's next() gives back, to see it handed back as it is.
const NEXT = Object.freeze({ kind: 'enter', marker: true })

test('a turn as DSH runs it, told in the words of its Claude Code bridge', async () => {
  const her = await fakeHer()
  const { make } = await load()
  const dsh = fakeDsh()
  const plugin = make(dsh.ctx, { port: her.port }, { dir: '' })
  plugin.listen()
  // Every point, the waterfalls first in line; a waterfall's next() back as it is, at once.
  assert.deepEqual(Object.keys(dsh.on).sort(), ['agent/created', 'agent/pre-step', 'agent/turn-stopping', 'approval/request', 'session/event', 'tools/post-execute', 'tools/pre-execute', 'user-questions/request'])
  for (const name of ['agent/pre-step', 'tools/pre-execute', 'tools/post-execute', 'approval/request', 'user-questions/request']) assert.equal(dsh.on[name].prepend, true, name)
  const s = session('session-6aa5a96d-33fc')
  const agent = agentOf(s)
  const log = e => dsh.on['session/event'].fn(s, e)

  assert.equal(dsh.on['agent/created'].fn({ agent, source: 'startup' }), undefined)
  log(ev('turn/start', { turn: 1 }))
  const user = { role: 'user', content: [{ type: 'text', text: 'Read note.txt, then create copy.txt' }], source: { kind: 'user' } }
  assert.equal(dsh.on['agent/pre-step'].fn({ agent, messages: [user], turn: 1 }, () => NEXT), NEXT)
  // A step that only goes on: nothing told.
  assert.equal(dsh.on['agent/pre-step'].fn({ agent, messages: [], turn: 1 }, () => NEXT), NEXT)
  log(ev('session/title', { title: 'Use the ask_user_question tool to', source: { kind: 'fallback' } }))
  log(ev('session/title', { title: 'Read and copy a note', source: { kind: 'provider' } }))
  const write = { name: 'write', arguments: { file_path: 'C:\\w\\copy.txt', content: 'x'.repeat(100000) }, callId: 'call_00_CKTB', agent }
  assert.equal(dsh.on['tools/pre-execute'].fn(write, () => NEXT), NEXT)
  assert.equal(dsh.on['tools/post-execute'].fn(write, { content: [{ type: 'text', text: '<path>C:\\w\\copy.txt</path>' }], isError: false }, () => NEXT), NEXT)
  log(ev('assistant/message', { turn: 1, step: 1, message: { role: 'assistant', content: [{ type: 'text', text: "I'll read note.txt first." }, { type: 'tool-call', id: 'c' }] } }))
  log(ev('assistant/message', { turn: 1, step: 2, message: { role: 'assistant', content: [{ type: 'text', text: 'Done — copy.txt' }, { type: 'text', text: 'has `hi`.' }] } }))
  assert.equal(dsh.on['agent/turn-stopping'].fn({ agent, turn: 1 }), undefined)
  // Its turn/end after a Stop told: nothing more.
  log(ev('turn/end', { turn: 1, reason: { kind: 'completed' } }))
  await plugin.idle()
  await her.close()

  assert.ok(her.got.every(g => g.url === '/hook?from=wakuwaku&agent=dsh'))
  const told = her.got.map(g => g.event)
  assert.deepEqual(told.map(e => e.hook_event_name), ['SessionStart', 'UserPromptSubmit', 'PreToolUse', 'PostToolUse', 'Stop'])
  const base = { session_id: 'session-6aa5a96d-33fc', transcript_path: '', cwd: WORKSPACE, origin: '', parent_session: '' }
  assert.deepEqual(told[0], { ...base, hook_event_name: 'SessionStart', source: 'startup' })
  assert.deepEqual(told[1], { ...base, hook_event_name: 'UserPromptSubmit', prompt: 'Read note.txt, then create copy.txt', source: 'user' })
  // The name as DSH gave it last; a file written whole cut short.
  assert.equal(told[2].session_title, 'Read and copy a note')
  assert.deepEqual([told[2].tool_name, told[2].tool_use_id, told[2].tool_input.file_path, told[2].tool_input.content.length], ['write', 'call_00_CKTB', 'C:\\w\\copy.txt', 2000])
  // Cut short: said so, and it ends so (zhoupengjie/Wakuwaku#73).
  assert.deepEqual([told[2].tool_input_truncated, told[2].tool_input.content.endsWith('…')], [true, true])
  assert.deepEqual([told[3].tool_response, told[3].is_error], ['<path>C:\\w\\copy.txt</path>', false])
  // The turn's last words, its text parts joined.
  assert.deepEqual([told[4].stop_hook_active, told[4].last_assistant_message], [false, 'Done — copy.txt\nhas `hi`.'])
})

test('a turn that ends unfinished, and a subagent', async () => {
  const her = await fakeHer()
  const { make } = await load()
  const parent = session('session-1')
  const mid = session('a1-mid', { origin: 'subagent', parentSession: 'session-1', delegationDepth: 1 })
  const child = session('b2-child', { origin: 'subagent', parentSession: 'a1-mid', delegationDepth: 2 })
  const live = new Map([parent, mid, child].map(s => [s.id, s]))
  const dsh = fakeDsh({ sessions: { get: id => live.get(id) } })
  const plugin = make(dsh.ctx, { port: her.port }, { dir: '' })
  plugin.listen()
  const log = (s, e) => dsh.on['session/event'].fn(s, e)
  // Stopped by the person (no turn-stopping): Stop, its reason, no words.
  log(parent, ev('turn/start', { turn: 1 }))
  log(parent, ev('assistant/message', { message: { content: [{ type: 'text', text: 'Half way' }] } }))
  log(parent, ev('turn/end', { turn: 1, reason: { kind: 'aborted', reason: { kind: 'user' } } }))
  // A turn that failed: StopFailure, why.
  log(parent, ev('turn/start', { turn: 2 }))
  log(parent, ev('turn/end', { turn: 2, reason: { kind: 'error', error: { message: '401 Unauthorized', code: 'AUTH' } } }))
  // A subagent's tool: whose, up its lineage as far as live sessions go; its
  // own name (made of its task) not sent.
  log(child, ev('session/title', { title: 'List all files in the' }))
  const exec = { name: 'grep', arguments: { pattern: 'TODO' }, callId: 'c9', agent: agentOf(child) }
  dsh.on['tools/pre-execute'].fn(exec, () => NEXT)
  // Its parent not live: the parent itself.
  live.delete('a1-mid')
  dsh.on['tools/pre-execute'].fn(exec, () => NEXT)
  await plugin.idle()
  await her.close()
  const told = her.got.map(g => g.event)
  assert.deepEqual(
    told.map(e => [e.hook_event_name, e.stop_reason, e.last_assistant_message, e.error_details]),
    [
      ['Stop', 'aborted', '', undefined],
      ['StopFailure', undefined, undefined, '401 Unauthorized'],
      ['PreToolUse', undefined, undefined, undefined],
      ['PreToolUse', undefined, undefined, undefined],
    ],
  )
  assert.deepEqual([told[2].origin, told[2].parent_session, told[2].top_session], ['subagent', 'a1-mid', 'session-1'])
  assert.equal('session_title' in told[2], false)
  assert.equal(told[3].top_session, 'a1-mid')
  assert.equal(told[0].origin, '')
  assert.equal('top_session' in told[0], false)
})

test('its word as DSH loads it, and as it is let go', async () => {
  const her = await fakeHer()
  const { make, apply, VERSION } = await load()
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'wakuwaku-dsh-'))
  const dsh = fakeDsh({ profileContext: { name: 'desktop' } })
  const plugin = make(dsh.ctx, { port: her.port }, { dir, sha: '0123456789abcdef' })
  const letGo = plugin.hello()
  await plugin.idle()
  const marker = path.join(dir, 'loaded-desktop.json')
  const word = await written(marker)
  assert.deepEqual([word.pid, word.version, word.sha, word.profile, word.port], [process.pid, VERSION, '0123456789abcdef', 'desktop', her.port])
  assert.ok(Math.abs(Date.now() - word.at) < 5000)
  assert.deepEqual(her.got.map(g => g.event), [{ session_id: '', hook_event_name: 'WakuwakuLoaded', version: VERSION, pid: process.pid, profile: 'desktop' }])
  letGo()
  assert.equal(fs.existsSync(marker), false)
  // Let go: nothing more told.
  plugin.sessionStart({ agent: agentOf(session('s')), source: 'startup' })
  await plugin.idle()
  assert.equal(her.got.length, 1)
  // apply, as DSH calls it: its listeners and its letting go, as an effect.
  const real = fakeDsh({ profileContext: { name: 'web' } })
  apply(real.ctx, { port: her.port })
  assert.equal(Object.keys(real.on).length, 8)
  assert.equal(real.effects.length, 1)
  real.effects[0].dispose()
  await her.close()
  fs.rmSync(dir, { recursive: true, force: true })
})

test('an edit reaches her with its text whole, up to 64 KB (zhoupengjie/Wakuwaku#73)', async () => {
  const her = await fakeHer()
  const { make } = await load()
  const dsh = fakeDsh()
  const plugin = make(dsh.ctx, { port: her.port }, { dir: '' })
  plugin.listen()
  const agent = agentOf(session('session-edit'))
  const long = 'a'.repeat(5000)
  const edit = { name: 'edit', arguments: { file_path: 'C:\\w\\notes.md', old_string: long, new_string: `${long}b`, replace_all: false }, callId: 'call_e', agent }
  dsh.on['tools/pre-execute'].fn(edit, () => NEXT)
  const huge = { name: 'str_replace_editor', arguments: { command: 'str_replace', path: 'C:\\w\\notes.md', old_str: 'x'.repeat(70000), new_str: 'y' }, callId: 'call_h', agent }
  dsh.on['tools/pre-execute'].fn(huge, () => NEXT)
  await plugin.idle()
  await her.close()
  const [whole, cut] = her.got.map(g => g.event)
  assert.deepEqual([whole.tool_input.old_string.length, whole.tool_input.new_string.length, whole.tool_input.replace_all, 'tool_input_truncated' in whole], [5000, 5001, false, false])
  assert.deepEqual([cut.tool_input.old_str.length, cut.tool_input.old_str.endsWith('…'), cut.tool_input.new_str, cut.tool_input_truncated], [64 * 1024, true, 'y', true])
})

test('its own digest, the same with its lines ending LF or CRLF', async () => {
  const { pathToFileURL } = urlModule
  const { createHash } = cryptoModule
  const lf = SOURCE.replace(/\r\n/g, '\n')
  const want = createHash('sha256').update(lf).digest('hex').slice(0, 16)
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'wakuwaku-dsh-'))
  const shas = []
  for (const [name, text] of [['lf', lf], ['crlf', lf.replace(/\n/g, '\r\n')]]) {
    fs.mkdirSync(path.join(dir, name))
    const file = path.join(dir, name, 'wakuwaku.js')
    fs.writeFileSync(file, text)
    const { make } = await import(pathToFileURL(file).href)
    const plugin = make(fakeDsh({ profileContext: { name: 'desktop' } }).ctx, { port: 9 })
    const letGo = plugin.hello()
    const marker = path.join(dir, name, 'loaded-desktop.json')
    shas.push((await written(marker)).sha)
    letGo()
  }
  assert.deepEqual(shas, [want, want])
  fs.rmSync(dir, { recursive: true, force: true })
})

test('nothing of hers reaches DSH: she not there, odd input, errors', async () => {
  const { make, apply } = await load()
  // A port no one listens on.
  const closed = await fakeHer()
  await closed.close()
  const dsh = fakeDsh()
  const plugin = make(dsh.ctx, { port: closed.port }, { dir: '' })
  plugin.listen()
  const started = Date.now()
  for (let i = 0; i < 20; i++) dsh.on['agent/created'].fn({ agent: agentOf(session(`s${i}`)), source: 'startup' })
  // Odd input: no agent, no session, no content; a session whose header throws.
  assert.equal(dsh.on['tools/pre-execute'].fn(undefined, () => NEXT), NEXT)
  assert.equal(dsh.on['tools/post-execute'].fn({ name: 'x' }, null, () => NEXT), NEXT)
  assert.equal(dsh.on['agent/pre-step'].fn({ messages: [{}] }, () => NEXT), NEXT)
  const bad = { get header() { throw new Error('gone') } }
  assert.doesNotThrow(() => dsh.on['session/event'].fn(bad, ev('turn/end', {})))
  assert.doesNotThrow(() => dsh.on['session/event'].fn(null, null))
  assert.doesNotThrow(() => dsh.on['agent/turn-stopping'].fn({}))
  await plugin.idle()
  // Away after the first refusal: the rest not even tried.
  assert.ok(Date.now() - started < 1500, `${Date.now() - started} ms`)
  // A context that throws on everything: apply still returns.
  assert.doesNotThrow(() => apply({ on() { throw new Error('no') }, effect() { throw new Error('no') }, get() { throw new Error('no') } }, {}))
  // Its config's port, else her own default.
  const other = fakeDsh()
  assert.doesNotThrow(() => make(other.ctx, { port: 'x' }, { dir: '' }).listen())
})

// DSH's side of an approval or a question: next() as its gateway does it,
// pending till DSH's own dialog is answered (answer()), withdrawn (rejected)
// as the signal the request carries then aborts.
function dialog(req) {
  let answer
  const shown = new Promise((resolve, reject) => {
    answer = resolve
    const signal = req.signal
    signal?.addEventListener('abort', () => reject(new Error('cancelled by the Host')), { once: true })
  })
  return { next: () => shown, answer: value => answer(value) }
}

// An approval as DSH's tools pipeline asks one (dsh-tools serviceAsk, a
// sandbox escalation): the call it is about, no input of its own.
function approvalOf(agent, callId, signal = new AbortController().signal) {
  return {
    agent,
    toolName: 'pwsh',
    callId,
    reason: 'escalate sandbox to danger-full-access: the user asked to read a file outside the workspace',
    displayReason: { en: 'Allow this operation with danger-full-access permissions: …', zh: '允许本次操作使用 danger-full-access 权限：…' },
    signal,
  }
}

const settle = () => new Promise(done => setTimeout(done, 50))

test('an approval answered on her island first: DSH takes hers, its own dialog withdrawn', async () => {
  let decide
  const her = await fakeHer(e => (e.hook_event_name === 'PermissionRequest' ? new Promise(done => (decide = done)) : {}))
  const { make } = await load()
  const dsh = fakeDsh()
  const plugin = make(dsh.ctx, { port: her.port }, { dir: '' })
  plugin.listen()
  const s = session('session-ed6ad37c')
  const agent = agentOf(s)
  dsh.on['tools/pre-execute'].fn({ name: 'pwsh', arguments: { command: 'Get-Content -Path C:\\Windows\\win.ini', sandbox_permissions: 'danger-full-access', justification: 'asked' }, callId: 'call_01', agent }, () => NEXT)
  const turn = new AbortController()
  const req = approvalOf(agent, 'call_01', turn.signal)
  const own = dialog(req)
  const outcome = dsh.on['approval/request'].fn(req, own.next)
  // The request's signal is ours too now, the turn's still in it.
  assert.notEqual(req.signal, turn.signal)
  assert.equal(req.signal.aborted, false)
  while (!decide) await settle()
  decide({ decision: 'allow' })
  assert.equal(await outcome, 'allowed-once')
  // DSH's dialog withdrawn; the turn itself untouched.
  assert.equal(req.signal.aborted, true)
  assert.equal(turn.signal.aborted, false)
  await plugin.idle()
  await settle()
  await her.close()
  const told = her.got.map(g => g.event)
  assert.deepEqual(told.map(e => e.hook_event_name), ['PreToolUse', 'PermissionRequest', 'PermissionReplied'])
  const asked = told[1]
  assert.deepEqual([asked.tool_name, asked.tool_input.command, asked.tool_use_id, asked.answerable, asked.session_id], ['pwsh', 'Get-Content -Path C:\\Windows\\win.ini', 'call_01', true, 'session-ed6ad37c'])
  assert.ok(asked.reason.startsWith('escalate sandbox to danger-full-access'))
  assert.equal(asked.display_reason.zh, '允许本次操作使用 danger-full-access 权限：…')
  assert.deepEqual([told[2].permission_id, told[2].tool_use_id, told[2].outcome, told[2].by], [asked.permission_id, 'call_01', 'allowed-once', 'her'])

  // Denied on her island: DSH's outcome rejected.
  const her2 = await fakeHer(e => (e.hook_event_name === 'PermissionRequest' ? { decision: 'deny' } : {}))
  const plugin2 = make(dsh.ctx, { port: her2.port }, { dir: '' })
  const req2 = approvalOf(agent, 'call_02')
  assert.equal(await plugin2.approval(req2, dialog(req2).next), 'rejected')
  await plugin2.idle()
  await her2.close()
})

test('an approval answered in DSH first, or left to DSH: hers let go, she told', async () => {
  // She holds it (the person has not answered on her island yet).
  const her = await fakeHer(e => (e.hook_event_name === 'PermissionRequest' ? new Promise(() => {}) : {}))
  const { make } = await load()
  const dsh = fakeDsh()
  const plugin = make(dsh.ctx, { port: her.port }, { dir: '' })
  plugin.listen()
  const agent = agentOf(session('session-1'))
  const req = approvalOf(agent, 'call_03')
  const own = dialog(req)
  const outcome = dsh.on['approval/request'].fn(req, own.next)
  while (her.got.length < 1) await settle()
  own.answer('rejected')
  assert.equal(await outcome, 'rejected')
  assert.equal(req.signal.aborted, false)
  await plugin.idle()
  await settle()
  assert.deepEqual(her.dropped, ['PermissionRequest'])
  const told = her.got.map(g => g.event)
  assert.deepEqual(told.map(e => [e.hook_event_name, e.outcome, e.by]), [['PermissionRequest', undefined, undefined], ['PermissionReplied', 'rejected', 'dsh']])
  // Unknown to tools/pre-execute: its tool by DSH's name, no input.
  assert.deepEqual([told[0].tool_name, told[0].tool_input], ['pwsh', {}])
  await her.close()

  // She gives none ({}: out of sight, its time up, sent to DSH): DSH's dialog answers.
  const her2 = await fakeHer()
  const plugin2 = make(dsh.ctx, { port: her2.port }, { dir: '' })
  const req2 = approvalOf(agent, 'call_04')
  const own2 = dialog(req2)
  const outcome2 = plugin2.approval(req2, own2.next)
  while (her2.got.length < 1) await settle()
  await settle()
  own2.answer('allowed-once')
  assert.equal(await outcome2, 'allowed-once')
  // The turn stopped meanwhile: DSH's own outcome; its error passed on as it
  // is. Stopped before it reached her, nothing of it is told.
  const turn = new AbortController()
  const req3 = approvalOf(agent, 'call_05', turn.signal)
  const outcome3 = plugin2.approval(req3, dialog(req3).next)
  turn.abort()
  await assert.rejects(outcome3, /cancelled by the Host/)
  // Stopped once she has it: cancelled.
  const turn4 = new AbortController()
  const req4 = approvalOf(agent, 'call_09', turn4.signal)
  const outcome4 = plugin2.approval(req4, dialog(req4).next)
  while (her2.got.length < 3) await settle()
  turn4.abort()
  await assert.rejects(outcome4, /cancelled by the Host/)
  await plugin2.idle()
  await settle()
  await her2.close()
  const told2 = her2.got.map(g => [g.event.hook_event_name, g.event.tool_use_id, g.event.outcome])
  assert.deepEqual(told2, [
    ['PermissionRequest', 'call_04', undefined],
    ['PermissionReplied', 'call_04', 'allowed-once'],
    ['PermissionRequest', 'call_09', undefined],
    ['PermissionReplied', 'call_09', 'cancelled'],
  ])
})

test('she not there, or a request that cannot be taken over: DSH as it was', async () => {
  const { make } = await load()
  const closed = await fakeHer()
  await closed.close()
  const dsh = fakeDsh()
  const plugin = make(dsh.ctx, { port: closed.port }, { dir: '' })
  const agent = agentOf(session('session-1'))
  // Not known away yet: asked, refused, DSH's dialog the one.
  const req = approvalOf(agent, 'call_06')
  const own = dialog(req)
  const outcome = plugin.approval(req, own.next)
  await plugin.idle()
  own.answer('allowed-once')
  assert.equal(await outcome, 'allowed-once')
  // Known away: nothing taken over, next() handed back as it is.
  const signal = new AbortController().signal
  const req2 = approvalOf(agent, 'call_07', signal)
  assert.equal(plugin.approval(req2, () => NEXT), NEXT)
  assert.equal(req2.signal, signal)
  const q = { questions: [{ id: 'a', question: '?', options: [{ label: 'x' }] }], agent, signal }
  assert.equal(plugin.question(q, () => NEXT), NEXT)
  assert.equal(q.signal, signal)

  // Frozen: she is told, but her answer is not taken (DSH's dialog would stay).
  const her = await fakeHer(e => (e.hook_event_name === 'PermissionRequest' ? { decision: 'allow' } : {}))
  const plugin2 = make(dsh.ctx, { port: her.port }, { dir: '' })
  const req3 = Object.freeze(approvalOf(agent, 'call_08'))
  const own3 = dialog(req3)
  const outcome3 = plugin2.approval(req3, own3.next)
  while (her.got.length < 1) await settle()
  await settle()
  assert.equal(her.got[0].event.answerable, false)
  own3.answer('rejected')
  assert.equal(await outcome3, 'rejected')
  await plugin2.idle()
  await her.close()
})

test('a question answered on her island: its answers back to DSH, by id', async () => {
  let reply = { answers: [{ id: 'name', selected: ['test2'] }] }
  const her = await fakeHer(e => (e.hook_event_name === 'PermissionRequest' ? reply : {}))
  const { make } = await load()
  const dsh = fakeDsh()
  const plugin = make(dsh.ctx, { port: her.port }, { dir: '' })
  plugin.listen()
  const agent = agentOf(session('session-q'))
  const questions = [{ id: 'name', header: 'File Name', question: 'test1 or test2?', options: [{ label: 'test1', description: 'one' }, { label: 'test2' }] }]
  dsh.on['tools/pre-execute'].fn({ name: 'ask_user_question', arguments: { questions }, callId: 'call_q1', agent }, () => NEXT)
  const req = { questions, agent, signal: new AbortController().signal }
  const answered = await dsh.on['user-questions/request'].fn(req, dialog(req).next)
  assert.deepEqual(answered, { answers: [{ id: 'name', selected: ['test2'] }] })
  assert.equal(req.signal.aborted, true)
  await plugin.idle()
  const asked = her.got.map(g => g.event).find(e => e.hook_event_name === 'PermissionRequest')
  assert.deepEqual([asked.tool_name, asked.tool_use_id, asked.tool_input.questions[0].options[1].label], ['ask_user_question', 'call_q1', 'test2'])
  // Answers that do not fit (another id): none taken; DSH's dialog answers.
  reply = { answers: [{ id: 'other', selected: ['x'] }] }
  const req2 = { questions, agent, signal: new AbortController().signal }
  const own2 = dialog(req2)
  const outcome2 = plugin.question(req2, own2.next)
  await plugin.idle()
  await settle()
  own2.answer({ answers: [{ id: 'name', selected: [], custom: 'mine' }] })
  assert.deepEqual(await outcome2, { answers: [{ id: 'name', selected: [], custom: 'mine' }] })
  await plugin.idle()
  await her.close()
  assert.deepEqual(her.got.filter(g => g.event.hook_event_name === 'PermissionReplied').map(g => [g.event.outcome, g.event.by]), [['answered', 'her'], ['answered', 'dsh']])
})
