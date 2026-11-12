import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { test } from 'node:test'
import { STRINGS } from '../src/shared/i18n.ts'
import * as Agents from '../src/pet/agents.ts'

const root = path.join(import.meta.dirname, '..')
const read = file => fs.readFileSync(path.join(root, file), 'utf8')

// A settings snapshot as settings.rs makes one: every agent on this
// computer and not connected, unless changed.
function snapshot(change = {}) {
  const block = (status, extra = {}) => ({ hooks: status, file: 'C:\\Users\\me\\x', fileShown: '~\\x', backup: false, seen: false, ...extra })
  // Claude Code's given as before, by itself (connection, hooks,
  // claudeSeen, claudeHere), the others' by their ids: all into agents.
  const { connection = 'none', hooks = 'missing', claudeSeen = false, claudeHere = true, ...rest } = change
  const agents = {
    claude: block(hooks, { connection, here: claudeHere, seen: claudeSeen, file: 'C:\\Users\\me\\.claude\\settings.json', fileShown: '~\\.claude\\settings.json', pluginCommands: [] }),
    codex: block('missing', { version: '0.150.0', async: true }),
    agy: block('missing'),
    opencode: { hooks: 'missing', file: 'C:\\Users\\me\\.config\\opencode\\plugins\\wakuwaku.js', fileShown: '~\\.config\\opencode\\plugins\\wakuwaku.js', seen: false },
    omp: block('missing', { more: 0 }),
    zcode: block('missing'),
    dsh: block('missing', { why: '', listShown: '~\\.dsh\\wakuwaku\\wakuwaku.js' }),
    reasonix: block('missing', { line: '2.x' }),
  }
  for (const id of Object.keys(agents)) {
    if (id in rest) {
      agents[id] = rest[id]
      delete rest[id]
    }
  }
  return {
    agents,
    mailAgents: { claude: true, codex: true, agy: false },
    agentVersions: { claude: '2.1.296', zcode: '3.15.3', dsh: '0.2.0-rc.2', reasonix: '2.3.0', opencode: null, omp: null },
    sessions: [],
    ...rest,
  }
}
const absent = { codex: { hooks: 'absent' }, agy: { hooks: 'absent' }, opencode: { hooks: 'absent' }, omp: { hooks: 'absent' }, zcode: { hooks: 'absent' }, dsh: { hooks: 'absent' }, reasonix: { hooks: 'absent' } }

test('each agent in the table is whole, and its words are there in both languages', () => {
  const bridge = read('src/pet/bridge.ts')
  const sites = read('src-tauri/src/settings.rs')
  const keys = new Set()
  // Each written or removed through the bridge's one call, by its id.
  assert.match(bridge, /\n\s+agent: \(id, action\) => invoke\('settings_agent', \{ agent: id, action \}\)/)
  assert.equal(Agents.TABLE.length, 8)
  for (const a of Agents.TABLE) {
    assert.match(a.id, /^[a-z]+$/)
    assert.ok(a.name, a.id)
    assert.ok(['hooks', 'plugin', 'extension', 'bridge'].includes(a.via), `${a.id}: via ${a.via}`)
    // Its site one settings.rs may open (its id one settings_agent knows:
    // connection/mod.rs's test connected_as_the_page_counts_it).
    assert.equal(a.site, `agent:${a.id}`)
    assert.match(sites, new RegExp(`\\("${a.id}", "https://`), `${a.id}: its site in AGENT_SITES`)
    assert.deepEqual(Object.keys(a.caps), Agents.CAPS, a.id)
    for (const [cap, [level, key]] of Object.entries(a.caps)) {
      assert.ok(['yes', 'part', 'no'].includes(level), `${a.id}.${cap}: ${level}`)
      keys.add(key)
    }
    if (a.caps.mail[0] !== 'no') keys.add(`ag.cap.mail.missing.${a.id}`)
    for (const k of ['what', 'note', 'how', 'st.set']) keys.add(`ag.${k}.${a.id}`)
    keys.add(`ag.via.${a.via}`)
    for (const k of Object.values(a.words || {})) keys.add(k)
    if (a.step) keys.add(a.step)
    if (a.stepNote) keys.add(a.stepNote)
    assert.ok(a.good.every(s => a.in.includes(s)), `${a.id}: good is part of in`)
  }
  for (const cap of Agents.CAPS) keys.add(`ag.cap.${cap}`)
  for (const id of ['claude', 'codex', 'agy']) keys.add(`mail.agentMissing.${id}`)
  for (const k of keys) {
    assert.ok(k in STRINGS.zh, `missing in zh: ${k}`)
    assert.ok(k in STRINGS.en, `missing in en: ${k}`)
  }
})

test('the table has every agent the server hears, and no other', () => {
  // connection/mod.rs ALL: the agents she connects and hears (server.rs
  // keeps when each was last heard by its place there), agent-core's names
  // for them, which are their ids in lower case.
  const all = /pub const ALL: \[Agent; \d+\] = \[([^\]]*)\]/.exec(read('src-tauri/src/connection/mod.rs'))[1]
  const heard = [...all.matchAll(/Agent::(\w+)/g)].map(m => m[1].toLowerCase())
  assert.ok(heard.length >= 8, 'the scan is broken')
  assert.deepEqual(heard.sort(), Agents.TABLE.map(a => a.id).sort())
})

test('what she can do is what the code does', () => {
  const lit = cap => Agents.TABLE.filter(a => a.caps[cap][0] !== 'no').map(a => a.id).sort()
  // Approvals on the island: Claude Code's, and those panel.js names as
  // the other agents a prompt may come from.
  const panel = read('src/pet/panel.ts')
  const from = Object.keys(eval(`(${/const FROM = (\{[^}]*\})/.exec(panel)[1]})`))
  assert.deepEqual(lit('approve'), ['claude', ...from].sort())
  // omp's are its plan reviews alone.
  assert.equal(Agents.byId('omp').caps.approve[0], 'part')
  // A question answered on the island: Claude Code's AskUserQuestion, and
  // DeepSeek Harness's, whose answers agent-core's asks.rs gives back by
  // their ids (its own tests say so, in its repository).
  assert.deepEqual(lit('choose'), ['claude', 'dsh'])
  // Mail: those mail/agent.rs runs.
  const mail = /const NAMES: \[&str; \d\] = \[([^\]]*)\]/.exec(read('src-tauri/src/mail/agent.rs'))[1]
  assert.deepEqual(lit('mail'), [...mail.matchAll(/"(\w+)"/g)].map(m => m[1]).sort())
  // Today counts every agent; tokens only those tokens.rs reads.
  assert.deepEqual(lit('stats'), Agents.TABLE.map(a => a.id).sort())
  // (tokens.rs keeps a count for each agent it reads, by its id.)
  const fields = /pub struct Tokens \{([\s\S]*?)\n\}/.exec(read('src-tauri/src/tokens.rs'))[1]
  const counted = new Set([...fields.matchAll(/\n\s+(\w+): /g)].map(m => m[1]))
  const withTokens = Agents.TABLE.filter(a => ['ag.cap.stats.tokens', 'ag.cap.stats.agy', 'ag.cap.stats.reasonix'].includes(a.caps.stats[1])).map(a => a.id)
  assert.deepEqual(withTokens, Agents.TABLE.filter(a => counted.has(a.id)).map(a => a.id))
  assert.deepEqual(withTokens, ['claude', 'codex', 'agy', 'omp', 'dsh', 'reasonix'])
})

test('found here or not', () => {
  const snap = snapshot()
  assert.deepEqual(Agents.here(snap), ['claude', 'codex', 'agy', 'opencode', 'omp', 'zcode', 'dsh', 'reasonix'])
  assert.deepEqual(Agents.elsewhere(snap), [])
  const none = snapshot({ claudeHere: false, ...absent })
  assert.deepEqual(Agents.here(none), [])
  assert.deepEqual(Agents.elsewhere(none), ['claude', 'codex', 'agy', 'opencode', 'omp', 'zcode', 'dsh', 'reasonix'])
  // Claude Code's folder gone, its plugin still on: here.
  assert.deepEqual(Agents.here(snapshot({ claudeHere: false, connection: 'plugin', ...absent })), ['claude'])
  // Only OpenCode and ZCode.
  const two = snapshot({ claudeHere: false, ...absent, opencode: { hooks: 'missing' }, zcode: { hooks: 'ok', seen: true } })
  assert.deepEqual(Agents.here(two), ['opencode', 'zcode'])
})

test('how each stands, and its one button', () => {
  const state = (change, id, known) => Agents.stateOf(snapshot(change), id, known)
  const action = (change, id, known) => Agents.actionOf(snapshot(change), id, known)
  // Not connected: Connect.
  assert.equal(state({}, 'codex'), 'none')
  assert.deepEqual(action({}, 'codex'), { act: 'install', label: 'ag.do.install' })
  // Written, waiting for its first turn: nothing to do, no button; an event
  // seen: connected, no button either (Reinstall and Remove are in its details).
  for (const id of ['agy', 'omp', 'zcode', 'dsh', 'reasonix']) {
    assert.equal(state({ [id]: { hooks: 'ok', seen: false } }, id), 'set', id)
    assert.equal(action({ [id]: { hooks: 'ok', seen: false } }, id), null, id)
    assert.equal(state({ [id]: { hooks: 'ok', seen: true } }, id), 'on', id)
    assert.equal(action({ [id]: { hooks: 'ok', seen: true } }, id), null, id)
  }
  assert.equal(state({ opencode: { hooks: 'ok', seen: false } }, 'opencode'), 'set')
  // Codex written and not trusted yet: a step of the person's, its button opens the row.
  assert.equal(state({ codex: { hooks: 'ok', seen: false } }, 'codex'), 'step')
  assert.deepEqual(action({ codex: { hooks: 'ok', seen: false } }, 'codex'), { act: 'open', label: 'ag.do.trust' })
  assert.equal(state({ codex: { hooks: 'ok', seen: true } }, 'codex'), 'on')
  assert.equal(action({ codex: { hooks: 'ok', seen: true } }, 'codex'), null)
  // Trusted as they are by Codex's own settings (zhoupengjie/Wakuwaku#66):
  // no event since she started is no step; not trusted, or not known, is.
  assert.equal(state({ codex: { hooks: 'ok', seen: false, trusted: true } }, 'codex'), 'set')
  assert.equal(action({ codex: { hooks: 'ok', seen: false, trusted: true } }, 'codex'), null)
  assert.equal(Agents.setLineOf(snapshot({ codex: { hooks: 'ok', trusted: true } }), 'codex'), 'ag.st.set.codexTrusted')
  for (const trusted of [false, null]) {
    assert.equal(state({ codex: { hooks: 'ok', seen: false, trusted } }, 'codex'), 'step', String(trusted))
    assert.equal(Agents.setLineOf(snapshot({ codex: { hooks: 'ok', trusted } }), 'codex'), 'ag.st.set.codex')
  }
  // Ours to repair is a repair, trusted or not.
  assert.equal(state({ codex: { hooks: 'upgradable', seen: false, trusted: true } }, 'codex'), 'repair')
  assert.deepEqual(Agents.TABLE.filter(a => a.step).map(a => a.id), ['codex'])
  // Its standing note: while the trust shows in amber above it, only its
  // second half (the trust not said twice); once trusted, all of it.
  assert.equal(Agents.noteOf('codex', 'step'), 'ag.note.codexWake')
  for (const state of ['on', 'set', 'repair', 'none']) assert.equal(Agents.noteOf('codex', state), 'ag.note.codex', state)
  assert.equal(Agents.noteOf('dsh', 'set'), 'ag.note.dsh')
  for (const lang of ['zh', 'en']) {
    assert.ok(STRINGS[lang]['ag.note.codex'].endsWith(STRINGS[lang]['ag.note.codexWake']), lang)
    assert.ok(STRINGS[lang]['ag.note.codexWake'].length < STRINGS[lang]['ag.note.codex'].length / 2, lang)
  }
  // Ours there but to repair (moved, older, part missing, turned off): Repair.
  for (const [id, status] of [['codex', 'stale'], ['codex', 'upgradable'], ['codex', 'partial'], ['agy', 'stale'], ['agy', 'off'], ['omp', 'partial'], ['zcode', 'off'], ['zcode', 'partial'], ['dsh', 'stale'], ['reasonix', 'stale'], ['reasonix', 'partial']]) {
    assert.equal(state({ [id]: { hooks: status } }, id), 'repair', `${id} ${status}`)
    assert.deepEqual(action({ [id]: { hooks: status } }, id), { act: 'install', label: 'settings.repair' }, `${id} ${status}`)
  }
  assert.equal(state({ opencode: { hooks: 'stale' } }, 'opencode'), 'repair')
  // Its file unreadable: no button; written and failed: Retry; being written: none.
  assert.equal(state({ zcode: { hooks: 'unreadable' } }, 'zcode'), 'error')
  assert.equal(action({ zcode: { hooks: 'unreadable' } }, 'zcode'), null)
  const failed = { failed: { dsh: { error: 'locked', kind: 'locked' } } }
  assert.equal(state({}, 'dsh', failed), 'failed')
  assert.deepEqual(action({}, 'dsh', failed), { act: 'install', label: 'wc.toolRetry' })
  assert.equal(state({}, 'omp', { busy: 'omp' }), 'busy')
  assert.equal(action({}, 'omp', { busy: 'omp' }), null)
  // Connected since, a failure before does not count.
  assert.equal(state({ dsh: { hooks: 'ok', seen: true } }, 'dsh', failed), 'on')
})

test('DeepSeek Harness: written is not loaded', () => {
  // connection.rs dsh_view: hooks, why, load.
  const dsh = (hooks, more = {}) => snapshot({ dsh: { hooks, why: '', seen: false, listShown: '~\\.dsh\\wakuwaku\\wakuwaku.js', ...more } })
  const state = snap => Agents.stateOf(snap, 'dsh')
  // Written, DSH not running or only starting; or running since before: blue, its line says which.
  assert.equal(state(dsh('ok', { load: { state: 'waiting' } })), 'set')
  assert.equal(Agents.setLineOf(dsh('ok', { load: { state: 'waiting' } }), 'dsh'), 'ag.st.set.dshWaiting')
  assert.equal(Agents.setLineOf(dsh('ok', { load: { state: 'restart' } }), 'dsh'), 'ag.st.set.dsh')
  assert.equal(Agents.setLineOf(dsh('ok', { load: { state: 'restart' } }), 'zcode'), 'ag.st.set.zcode')
  // DSH running without it (turned off and back, written while it ran): not
  // connected, whatever came before; DSH closed since an event: connected.
  assert.equal(state(dsh('ok', { seen: true, load: { state: 'restart' } })), 'set')
  assert.equal(state(dsh('ok', { seen: true, load: { state: 'waiting' } })), 'on')
  // Loaded (an older version too): connected, before any session.
  const loaded = dsh('ok', { load: { state: 'loaded', at: 1792200000000, version: 1, profile: 'desktop', pid: 4321 } })
  assert.equal(state(loaded), 'on')
  assert.equal(Agents.read(loaded, 'dsh').load.profile, 'desktop')
  assert.equal(state(dsh('ok', { load: { state: 'older' } })), 'on')
  // Written but not loaded while DSH runs: amber, Repair, "DSH 插件没生效".
  const notLoaded = dsh('notLoaded', { load: { state: 'notLoaded' } })
  assert.equal(state(notLoaded), 'repair')
  assert.deepEqual(Agents.actionOf(notLoaded, 'dsh'), { act: 'install', label: 'settings.repair' })
  assert.equal(Agents.wordOf('dsh', Agents.read(notLoaded, 'dsh').status), 's.dshNotLoaded')
  assert.equal(STRINGS.zh['s.dshNotLoaded'], 'DSH 插件没生效')
  assert.equal(Agents.read(dsh('notLoaded', { why: 'off' }), 'dsh').why, 'off')
  // The old bridge's rows: a status of their own, Repair.
  const bridge = dsh('stale', { why: 'bridge' })
  assert.equal(Agents.read(bridge, 'dsh').status, 'bridge')
  assert.equal(state(bridge), 'repair')
  assert.equal(Agents.wordOf('dsh', 'bridge'), 's.dshBridge')
  assert.equal(Agents.wordOf('dsh', 'stale'), 's.dshStale')
  // Connected (for Connect all and the summary) in all of these.
  for (const snap of [loaded, notLoaded, bridge]) assert.ok(Agents.isIn(snap, 'dsh'))
  assert.ok(!Agents.isIn(dsh('missing'), 'dsh'))
  assert.deepEqual(Agents.summary(notLoaded).wants.filter(id => id === 'dsh'), ['dsh'])
  for (const key of ['ag.st.set.dshWaiting', 's.dshWaiting', 's.dshRestart', 's.dshLoaded', 's.dshOlder', 's.dshNotLoadedWhy', 's.dshNotLoadedOff', 's.dshBridgeWhy', 'ag.pluginFile']) {
    assert.ok(key in STRINGS.zh && key in STRINGS.en, key)
  }
})

test("DeepSeek Harness's other way: its own hook bridge", () => {
  const dsh = (hooks, more = {}) => snapshot({ dsh: { hooks, why: '', seen: false, way: 'bridge', via: 'bridge', listShown: '~\\.dsh\\wakuwaku\\wakuwaku.js', ...more } })
  const state = snap => Agents.stateOf(snap, 'dsh')
  // Its packages missing (DSH 0.2.1 on), or of another DSH; its hook list
  // another exe's; the plugin's rows there: each its own words, amber, Repair.
  for (const [why, status, word] of [['packages', 'bridgePackages', 's.dshBPackages'], ['version', 'bridgeVersion', 's.dshBVersion'], ['hooks', 'bridgeHooks', 's.dshBHooks'], ['way', 'bridgeWay', 's.dshBWay']]) {
    const snap = dsh('stale', { why })
    assert.equal(Agents.read(snap, 'dsh').status, status, why)
    assert.equal(state(snap), 'repair', why)
    assert.deepEqual(Agents.actionOf(snap, 'dsh'), { act: 'install', label: 'settings.repair' })
    assert.equal(Agents.wordOf('dsh', status), word)
    assert.ok(word in STRINGS.zh && word in STRINGS.en && `s.dshBWhy.${status}` in STRINGS.zh && `s.dshBWhy.${status}` in STRINGS.en, word)
  }
  // Written: an event since DSH started is connected; the first still to
  // come, not, whatever came before; DSH not running, as before.
  assert.equal(state(dsh('ok', { load: { state: 'effective' } })), 'on')
  const first = dsh('ok', { seen: true, load: { state: 'firstEvent' } })
  assert.equal(state(first), 'set')
  assert.equal(Agents.setLineOf(first, 'dsh'), 'ag.st.set.dshFirstEvent')
  assert.equal(state(dsh('ok', { seen: true, load: { state: 'waiting' } })), 'on')
  // Its hook list shown, not the plugin's file.
  assert.equal(Agents.read(dsh('ok'), 'dsh').list, '')
  assert.deepEqual([Agents.read(dsh('ok', { hooksShown: '~\\x\\dsh-hooks.json' }), 'dsh').list, Agents.read(dsh('ok'), 'dsh').listKey], ['~\\x\\dsh-hooks.json', 'ag.list'])
  // Its own words: no step's tool, its note; the plugin's otherwise.
  assert.equal(Agents.whatOf(dsh('ok'), 'dsh'), 'ag.what.dshBridge')
  assert.equal(Agents.noteOf('dsh', 'on', dsh('ok')), 'ag.note.dshBridge')
  assert.deepEqual(Agents.capsOf(dsh('ok'), 'dsh')[0], ['status', 'part', 'ag.cap.status.dshBridge'])
  // No approval nor question on the island that way (no PermissionRequest), nor in Today.
  assert.deepEqual(Agents.capsOf(dsh('ok'), 'dsh').map(([cap, level, key]) => [cap, level, key]).slice(1, 4), [['approve', 'no', 'ag.cap.approve.dshBridge'], ['choose', 'no', 'ag.cap.choose.dshBridge'], ['stats', 'yes', 'ag.cap.stats.dsh']])
  assert.deepEqual(Agents.capsOf(snapshot({ dsh: { hooks: 'ok', way: 'plugin' } }), 'dsh').slice(1, 4).map(([cap, level]) => level), ['yes', 'yes', 'yes'])
  const pluginWay = snapshot({ dsh: { hooks: 'ok', way: 'plugin' } })
  assert.equal(Agents.whatOf(pluginWay, 'dsh'), 'ag.what.dsh')
  assert.equal(Agents.noteOf('dsh', 'on', pluginWay), 'ag.note.dsh')
  assert.deepEqual(Agents.capsOf(pluginWay, 'dsh')[0], ['status', 'yes', 'ag.cap.status.dsh'])
  for (const key of ['ag.what.dshBridge', 'ag.note.dshBridge', 'ag.cap.status.dshBridge', 'ag.cap.approve.dshBridge', 'ag.cap.choose.dshBridge', 'ag.cap.stats.dsh']) assert.ok(key in STRINGS.zh && key in STRINGS.en, key)
  assert.equal(Agents.read(dsh('ok'), 'dsh').way, 'bridge')
  // The plugin's way unless the snapshot says the bridge.
  assert.equal(Agents.read(snapshot({ dsh: { hooks: 'missing' } }), 'dsh').way, 'plugin')
  // The words of the way picker, its notes, a failure's, both languages.
  const keys = ['ag.dsh.via', 'ag.dsh.viaPlugin', 'ag.dsh.viaBridge', 'ag.dsh.viaNote.plugin', 'ag.dsh.viaNote.bridge', 'ag.dsh.switching.plugin', 'ag.dsh.switching.bridge', 'ag.dsh.mirror', 's.dshBridgeOn', 's.dshBridgeFirst', 's.dshBridgeWaiting', 's.dshBridgeFailed']
  for (const kind of ['Registry', 'Version', 'NotFound', 'ReleaseAge', 'Timeout', 'Runtime', 'Busy', 'Disk', 'Permission', 'Other']) keys.push(`s.dshBK.bridge${kind}`)
  for (const key of keys) assert.ok(key in STRINGS.zh && key in STRINGS.en, key)
  assert.equal(STRINGS.zh['ag.dsh.viaNote.plugin'], '不用联网，事件即时，能在岛上批准和回答提问')
  assert.equal(STRINGS.zh['ag.dsh.viaNote.bridge'], 'DSH 官方的钩子桥，要从 npm 下载两个包，每个事件约 1 秒，不能审批')
  // Every kind connection/mod.rs error_kind gives the bridge's failures has its words.
  const rust = read('src-tauri/src/connection/mod.rs')
  for (const [, kind] of rust.matchAll(/=> "(bridge[A-Z]\w+)",/g)) assert.ok(`s.dshBK.${kind}` in STRINGS.zh, kind)
})

test("Claude Code's two ways", () => {
  const state = change => Agents.stateOf(snapshot(change), 'claude')
  assert.equal(state({}), 'none')
  // The plugin: connected (no event is needed to know).
  assert.equal(state({ connection: 'plugin' }), 'on')
  // settings.json's hooks: written, then seen; nothing that starts her is fine too.
  assert.equal(state({ connection: 'hooks', hooks: 'ok' }), 'set')
  assert.equal(Agents.actionOf(snapshot({ connection: 'plugin' }), 'claude'), null)
  assert.equal(state({ connection: 'hooks', hooks: 'ok', claudeSeen: true }), 'on')
  assert.equal(state({ connection: 'hooks', hooks: 'httpOnly', claudeSeen: true }), 'on')
  assert.equal(state({ connection: 'hooks', hooks: 'stale' }), 'repair')
  assert.equal(state({ hooks: 'unreadable' }), 'error')
  // Both at once: Repair takes settings.json's out, the plugin stays.
  assert.equal(state({ connection: 'both', hooks: 'ok' }), 'repair')
  assert.deepEqual(Agents.actionOf(snapshot({ connection: 'both', hooks: 'ok' }), 'claude'), { act: 'remove', label: 'settings.repair' })
  assert.equal(Agents.read(snapshot(), 'claude').shown, '~\\.claude\\settings.json')
  assert.equal(Agents.read(snapshot(), 'claude').version, '2.1.296')
})

test('the files, as each reads them', () => {
  const snap = snapshot({ omp: { hooks: 'ok', fileShown: '~\\.omp\\agent\\extensions\\wakuwaku.ts', more: 2 } })
  assert.equal(Agents.read(snap, 'omp').more, 2)
  assert.equal(Agents.read(snap, 'omp').shown, '~\\.omp\\agent\\extensions\\wakuwaku.ts')
  assert.equal(Agents.read(snap, 'dsh').list, '~\\.dsh\\wakuwaku\\wakuwaku.js')
  assert.equal(Agents.read(snap, 'dsh').listKey, 'ag.pluginFile')
  assert.equal(Agents.read(snap, 'opencode').shown, '~\\.config\\opencode\\plugins\\wakuwaku.js')
  assert.equal(Agents.read(snap, 'codex').version, '0.150.0')
  assert.equal(Agents.read(snap, 'zcode').version, '3.15.3')
  assert.equal(Agents.read(snap, 'agy').version, '')
  // Its own words for a status that wants something, else the shared ones.
  assert.equal(Agents.wordOf('omp', 'stale'), 's.ompStale')
  assert.equal(Agents.wordOf('codex', 'stale'), 'settings.hooksStale')
  assert.equal(Agents.wordOf('zcode', 'off'), 's.zcodeOff')
  assert.equal(Agents.wordOf('claude', 'both'), 'home.connBoth')
})

test('Connect all, and General’s line', () => {
  const snap = snapshot({ connection: 'plugin', codex: { hooks: 'ok', seen: false }, agy: { hooks: 'stale' }, ...{ omp: { hooks: 'absent' } } })
  // Here and not connected: OpenCode, ZCode, DeepSeek Harness, Reasonix (agy is to repair, omp is not here).
  assert.deepEqual(Agents.toConnect(snap), ['opencode', 'zcode', 'dsh', 'reasonix'])
  // One being written is not offered again; one that failed is.
  assert.deepEqual(Agents.toConnect(snap, { busy: 'zcode', failed: { dsh: { kind: 'locked' } } }), ['opencode', 'dsh', 'reasonix'])
  // Codex written, to be trusted: connected, and wanting the person (as agy's repair).
  const s = Agents.summary(snap)
  assert.deepEqual(s.on, ['claude'])
  assert.deepEqual(s.set, [])
  assert.deepEqual(s.connected, ['claude', 'codex'])
  assert.deepEqual(s.wants, ['codex', 'agy'])
  assert.deepEqual(s.none, ['opencode', 'zcode', 'dsh', 'reasonix'])
  // A file unreadable and a write that failed want the person too.
  const more = Agents.summary(snapshot({ zcode: { hooks: 'unreadable' }, omp: { hooks: 'ok' } }), { failed: { dsh: { kind: 'locked' } } })
  assert.deepEqual(more.wants, ['zcode', 'dsh'])
  assert.deepEqual(more.set, ['omp'])
  // Nothing wants the person: no tab dot.
  assert.deepEqual(Agents.summary(snapshot({ connection: 'plugin', codex: { hooks: 'ok', seen: true }, zcode: { hooks: 'ok' } })).wants, [])
})

test('a Reasonix 0.x: on the page, never offered nor written', () => {
  const old = snapshot({ reasonix: { hooks: 'unsupported', line: '0.x', fileShown: '~\\AppData\\Roaming\\reasonix\\settings.json' } })
  // Here, its row there, grey, no button; not one to connect, nor a repair.
  assert.ok(Agents.here(old).includes('reasonix'))
  assert.equal(Agents.stateOf(old, 'reasonix'), 'unsupported')
  assert.equal(Agents.actionOf(old, 'reasonix'), null)
  assert.ok(!Agents.toConnect(old).includes('reasonix'))
  const s = Agents.summary(old)
  for (const part of ['on', 'set', 'wants', 'none', 'connected']) assert.ok(!s[part].includes('reasonix'), part)
  assert.equal(Agents.read(old, 'reasonix').line, '0.x')
  assert.equal(Agents.wordOf('reasonix', 'unsupported'), 's.reasonixOld')
  // A 0.x with ours written all the same (by hand, or before): ours is there, to repair or remove.
  assert.equal(Agents.stateOf(snapshot({ reasonix: { hooks: 'stale', line: '0.x' } }), 'reasonix'), 'repair')
  // 2.x, connected: approvals and questions stay in Reasonix (phase 1).
  const caps = Object.fromEntries(Agents.capsOf(snapshot(), 'reasonix').map(([cap, level]) => [cap, level]))
  assert.deepEqual(caps, { status: 'yes', approve: 'no', choose: 'no', stats: 'yes', mail: 'no' })
})

test('the dots: four colours, by what the person has to do', () => {
  // Green connected, blue written and waiting, amber the person must act,
  // grey not connected (or a version she cannot connect to).
  assert.deepEqual(Agents.MOOD, { on: 'done', set: 'working', busy: 'working', step: 'waiting', repair: 'waiting', error: 'waiting', failed: 'waiting', unsupported: 'idle', none: 'idle' })
  assert.deepEqual([...new Set(Object.values(Agents.MOOD))].sort(), ['done', 'idle', 'waiting', 'working'])
  // Every state that wants the person is amber, and only those.
  for (const [state, mood] of Object.entries(Agents.MOOD)) assert.equal(mood === 'waiting', Agents.WANTS.includes(state), state)
})

test('mail: lit where its command is found, and the Mail page’s list', () => {
  const caps = (snap, id) => Object.fromEntries(Agents.capsOf(snap, id).map(([cap, level, key]) => [cap, [level, key]]))
  assert.deepEqual(caps(snapshot(), 'claude').mail, ['yes', 'ag.cap.mail.claude'])
  assert.deepEqual(caps(snapshot(), 'agy').mail, ['no', 'ag.cap.mail.missing.agy'])
  assert.deepEqual(caps(snapshot({ mailAgents: { agy: true } }), 'agy').mail, ['yes', 'ag.cap.mail.agy'])
  assert.deepEqual(caps(snapshot(), 'dsh').mail, ['no', 'ag.cap.mail.no'])
  assert.deepEqual(caps(snapshot(), 'dsh').approve, ['yes', 'ag.cap.approve.dsh'])
  // Claude Code and Codex always; Antigravity where it is (its app alone:
  // greyed), found, or picked already.
  const ids = (snap, current) => Agents.mailAgents(snap, current).map(a => `${a.id}${a.found ? '' : '?'}`)
  assert.deepEqual(ids(snapshot(), 'claude'), ['claude', 'codex', 'agy?'])
  assert.deepEqual(ids(snapshot({ agy: { hooks: 'absent' } }), 'claude'), ['claude', 'codex'])
  assert.deepEqual(ids(snapshot({ agy: { hooks: 'absent' } }), 'agy'), ['claude', 'codex', 'agy?'])
  assert.deepEqual(ids(snapshot({ agy: { hooks: 'absent' }, mailAgents: { agy: true } }), 'claude'), ['claude?', 'codex?', 'agy'])
  assert.deepEqual(Agents.mailAgents(snapshot(), 'claude')[0], { id: 'claude', name: 'Claude Code', short: 'Claude', found: true })
})

test('sessions in sight, by agent', () => {
  const snap = snapshot({ sessions: [{ id: 'a' }, { id: 'b', agent: 'codex' }, { id: 'c', agent: 'codex' }, { id: 'd', agent: 'dsh' }] })
  assert.equal(Agents.sessionsOf(snap, 'claude'), 1)
  assert.equal(Agents.sessionsOf(snap, 'codex'), 2)
  assert.equal(Agents.sessionsOf(snap, 'omp'), 0)
})
