// The Agent page (agents.ts says what each agent is and can do): each one's
// row and what opens under it, connecting, repairing, removing; and the
// General page's update, crash reports and about.
// Made by settings.ts in the section's place as the page starts
// (agentsPart(core), imported by settings.ts): what it takes of the settings' core
// comes in core, its functions and constants as they are, its lets read
// and written through core.<name> as they change; what it hands back is the
// settings' again.
import * as Agents from './agents.ts'
import { pet } from './bridge.ts'

// What this part takes of the settings' core: the names it reads (and,
// of its lets, writes); settings.ts's call is checked against them.
export interface AgentsCore {
  Agents: any
  COLOR: any
  T: any
  agentOpen: any
  connecting: any
  crashShown: any
  draw: any
  esc: any
  failedHTML: any
  lang: any
  nameList: any
  netNow: any
  othersOpen: any
  patch: any
  pluginOpen: any
  relayout: any
  row: any
  sec: any
  seg: any
  snap: any
  sw: any
  tab: any
  toolNames: any
}

// Why one could not be written: the error, its kind, and the folder it was
// for, or DeepSeek Harness's way it was switching to.
interface Failure {
  error: string
  kind: string
  folder?: string
  via?: string
}

export function agentsPart(core: AgentsCore) {
  const {
    Agents, COLOR, T, draw, esc, nameList, netNow, patch, row, sec, seg, sw,
  } = core

  // Why each could not be written: { error, kind, folder } (locked, invalid, path, other).
  const failed: Record<string, Failure> = {}
  const sessionOpen = new Set()
  let connectingAll = false
  const known = () => ({ busy: core.connecting, failed })

  // One written or removed (bridge.ts settings.agent): what
  // came back shown; not written, why (the card names the file), its row
  // open to say so, and Claude Code's plugin the other way.
  async function agentDo(id, action) {
    const a = Agents.byId(id)
    if (!a || core.connecting) return
    if (action === 'install') {
      core.connecting = id
      // DSH's way said meanwhile (the bridge's packages take a while).
      if (id === 'dsh') dshSwitching = Agents.read(core.snap, 'dsh')?.way || ''
      delete failed[id]
      draw()
    }
    const got = await pet.settings.agent(id, action)
    core.connecting = ''
    dshSwitching = ''
    core.snap = got.snapshot
    if (action === 'install' && !got.ok) {
      failed[id] = { error: got.error || '', kind: got.kind || 'other', folder: got.folder || '' }
      if (id === 'claude') core.pluginOpen = true
      if (core.tab === 'agents') core.agentOpen = id
    }
    draw()
    core.relayout()
    return got.ok
  }

  // DeepSeek Harness's way switched (her plugin, or DSH's own hook bridge:
  // agents.ts, connection/dsh.rs dsh_way): the chosen one put in, the other
  // taken away, in its row meanwhile (the bridge's packages come from npm:
  // a minute or two); not connected yet, the choice is kept for Connect.
  // registry: 'npmmirror', picked after the registry pnpm names could not
  // be reached.
  let dshSwitching = ''
  async function dshSwitch(via, registry?) {
    if (core.connecting) return
    const r = Agents.read(core.snap, 'dsh') || {}
    if (!registry && !Agents.isIn(core.snap, 'dsh')) return (r.via || 'plugin') === via ? undefined : patch({ dshVia: via })
    if (!registry && r.way === via && r.status === 'ok') return
    core.connecting = 'dsh'
    dshSwitching = via
    delete failed.dsh
    draw()
    const got = await pet.settings.dshVia(via, registry)
    core.connecting = ''
    dshSwitching = ''
    core.snap = got.snapshot
    if (!got.ok) {
      failed.dsh = { error: got.error || '', kind: got.kind || 'other', via }
      if (core.tab === 'agents') core.agentOpen = 'dsh'
    }
    draw()
    core.relayout()
  }

  // Connect all: each found and not connected, one after another.
  async function connectAll() {
    if (connectingAll) return
    connectingAll = true
    for (const id of Agents.toConnect(core.snap, known())) await agentDo(id, 'install')
    connectingAll = false
    draw()
  }

  const capTip = (cap, key) => T('ag.capTip', { cap: T(`ag.cap.${cap}`), what: T(key) })
  const capIcon = (cap, level, key) => `<span class="cap ${level}" title="${esc(capTip(cap, key))}">${Agents.CAP_GLYPH[cap]}</span>`

  // How one stands, in its row's second line.
  function agentLine(id, state) {
    const r = Agents.read(core.snap, id)
    if (state === 'on') {
      const n = Agents.sessionsOf(core.snap, id)
      return n ? T(n === 1 ? 'ag.st.on1' : 'ag.st.onN', { n }) : T('ag.st.on')
    }
    if (state === 'set' || state === 'step') return T(Agents.setLineOf(core.snap, id))
    if (state === 'busy' && id === 'dsh' && dshSwitching) return T(`ag.dsh.switching.${dshSwitching}`)
    if (state === 'busy') return T('wc.busy')
    if (state === 'failed') return T('ag.st.failed')
    if (state === 'repair' || state === 'error') return T(Agents.wordOf(id, r.status))
    // A version she cannot connect to (Reasonix 0.x): that it is not (which,
    // beside its name).
    if (state === 'unsupported') return T(Agents.wordOf(id, r.status))
    return T('ag.st.none')
  }

  function agentRow(id) {
    const a = Agents.byId(id)
    const r = Agents.read(core.snap, id)
    const state = Agents.stateOf(core.snap, id, known())
    const action = Agents.actionOf(core.snap, id, known())
    const open = core.agentOpen === id
    const small = [a.aka, r.version].filter(Boolean).map(text => `<span class="ver">${esc(text)}</span>`).join('')
    const line = agentLine(id, state)
    // A button only where the person has something to do (agents.ts
    // actionOf); else a faint › saying the row opens.
    let end = `<span class="fi chev${open ? ' open' : ''}">\uE76C</span>`
    if (state === 'busy') end = `<button class="pbtn sm" disabled><span class="spin"></span>${esc(T('wc.busy'))}</button>`
    else if (action?.act === 'open') end = `<button class="pbtn sm" data-ag-open="${id}" data-ag-keep>${esc(T(action.label))}</button>`
    else if (action) end = `<button class="pbtn sm" data-agent="${id}:${action.act}" ${core.connecting ? 'disabled' : ''}>${esc(T(action.label))}</button>`
    const caps = `<span class="caps">${Agents.capsOf(core.snap, id).map(([cap, level, key]) => capIcon(cap, level, key)).join('')}</span>`
    return `<div class="r ag-row${open ? ' open' : ''}" data-ag-open="${id}"><span class="dot" style="background:${COLOR[Agents.MOOD[state]]}"></span><div class="grow"><div class="ag-name ellip">${esc(a.name)}${small}</div><div class="d ellip" title="${esc(line)}">${esc(line)}</div></div>${caps}<span class="ag-end">${end}</span></div>${open ? agentDetails(id, state) : ''}`
  }

  // What the person must do now, in amber at the top of the details (as
  // its dot): Codex's trust, its background hooks, an old Codex, ZCode's
  // hooks turned off, Claude Code's two ways at once, a file unreadable.
  function agentSteps(id, state) {
    const r = Agents.read(core.snap, id)
    const warn = []
    if (id === 'codex' && state === 'step') warn.push(T('ag.trust.codex'))
    if (id === 'codex' && r.status === 'upgradable') warn.push(T('s.codexUpgradeNote'))
    if (id === 'codex' && r.here && core.snap.agents?.codex?.async === false) warn.push(T('s.codexOld', { version: r.version }))
    if (id === 'zcode' && r.status === 'off') warn.push(T('s.zcodeOffNote'))
    if (id === 'claude' && r.status === 'both') warn.push(T('ag.claude.both'))
    // DSH: the old bridge's rows; the plugin written but not loaded, and why.
    if (id === 'dsh' && r.status === 'bridge') warn.push(T('s.dshBridgeWhy'))
    if (id === 'dsh' && r.status === 'notLoaded') warn.push(T(r.why === 'off' ? 's.dshNotLoadedOff' : 's.dshNotLoadedWhy'))
    // The hook bridge's: its packages missing or of another DSH (Repair
    // puts them in from npm), its rows or hook list another's.
    if (id === 'dsh' && ['bridgePackages', 'bridgeVersion', 'bridgeHooks', 'bridgeWay'].includes(r.status)) warn.push(T(`s.dshBWhy.${r.status}`, { version: core.snap.agents?.dsh?.dshVersion || '' }))
    if (state === 'error') warn.push(T('ag.note.unreadable'))
    return warn.map(text => `<div class="ag-note warn">${esc(text)}</div>`).join('')
  }

  // What to know about one: how it is, and what comes next while it waits
  // for its first turn (a new omp session, DSH started or restarted); for
  // DSH, whether and when it loaded her plugin; or why a version of it
  // cannot be connected (Reasonix 0.x).
  function agentNotes(id, state) {
    const r = Agents.read(core.snap, id)
    const next =
      state === 'set' && id === 'omp'
        ? T('s.ompNew')
        : id === 'dsh'
          ? dshNote(state)
          : state === 'unsupported' && id === 'reasonix'
            ? T('s.reasonixOldNote', { version: r.version || r.line })
            : ''
    return `${next ? `<div class="ag-note">${esc(next)}</div>` : ''}<div class="ag-note">${esc(T(Agents.noteOf(id, state, core.snap)))}</div>`
  }

  // DSH and her plugin (connection/dsh.rs dsh_view): loaded, when and in which
  // profile; an older one loaded; DSH to be restarted; DSH to start.
  function dshNote(state) {
    const r = Agents.read(core.snap, 'dsh') || {}
    const load = r.load || {}
    // The hook bridge: an event of it since DSH started, the first still to
    // come, or DSH not running.
    if (r.way === 'bridge') {
      if (load.state === 'effective') return T('s.dshBridgeOn')
      if (load.state === 'firstEvent') return T('s.dshBridgeFirst')
      return state === 'set' ? T('s.dshBridgeWaiting') : ''
    }
    if (load.state === 'loaded' || load.state === 'older') {
      const time = load.at ? new Date(load.at).toLocaleString(core.lang === 'zh' ? 'zh-CN' : 'en-US', { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' }) : ''
      const loaded = T('s.dshLoaded', { time, profile: load.profile || '—' })
      return load.state === 'older' ? `${loaded} ${T('s.dshOlder')}` : loaded
    }
    if (state !== 'set') return ''
    return T(load.state === 'restart' ? 's.dshRestart' : 's.dshWaiting')
  }

  // A field of the details: its name, what it is.
  const field = (key, value) => `<div class="kv"><span class="k">${esc(T(key))}</span><span class="v">${value}</span></div>`
  const pathOf = text => `<span class="mono path">${esc(text)}</span>`

  // Where one is written: the file (omp's in each profile too; DSH's patch
  // file, and her plugin beside it), its backup.
  function agentFiles(id) {
    const r = Agents.read(core.snap, id)
    return [
      field('ag.file', `${pathOf(r.shown)}${r.more ? esc(T('s.ompMore', { n: r.more })) : ''}`),
      r.list ? field(r.listKey || 'ag.list', pathOf(r.list)) : '',
      r.backup ? field('ag.backup', pathOf(`${r.shown}.wakuwaku.bak`)) : '',
    ].join('')
  }

  // Claude Code's two ways, each its own: the plugin (recommended: Claude
  // Code installs and removes it, nothing of ours in its settings) and the
  // hooks written straight into settings.json (new sessions start her).
  function claudeWays() {
    const commands = core.snap.agents?.claude?.pluginCommands || []
    const plugged = ['plugin', 'both'].includes(core.snap.agents?.claude?.connection)
    const plugin = `<div class="ag-way"><div class="ag-way-h"><b>${esc(T('home.plugin'))}</b><span class="grow"></span><span class="d">${esc(T(plugged ? 'home.pluginOn' : 'ag.claude.pluginOff'))}</span></div>
      <div class="d">${esc(T(plugged ? 'ag.claude.pluginRemove' : 'home.pluginWhy'))}</div>
      ${plugged ? '' : `${commands.map(c => `<div class="wc-cmd mono">${esc(c)}</div>`).join('')}<div class="ag-btns"><button class="pbtn sm" data-copy="${esc(commands.join('\n'))}" data-label="${esc(T('wc.copyBoth'))}">${esc(T('wc.copyBoth'))}</button><span class="d">${esc(T('home.pluginStart'))}</span></div>`}</div>`
    const status = core.snap.agents?.claude?.hooks
    const word = { ok: 'settings.hooksOk', missing: 'settings.hooksMissing', stale: 'settings.hooksStale', partial: 'settings.hooksPartial', httpOnly: 'settings.hooksHttpOnly', unreadable: 's.hooksUnreadable' }[status] || 'settings.hooksMissing'
    const busy = core.connecting === 'claude'
    // Not written while the plugin is on: every event would come twice.
    const buttons =
      status === 'unreadable' || (plugged && status === 'missing')
        ? ''
        : status === 'missing'
          ? `<button class="pbtn sm" data-agent="claude:install" ${core.connecting ? 'disabled' : ''}>${busy ? `<span class="spin"></span>${esc(T('wc.busy'))}` : esc(T('settings.install'))}</button>`
          : `<button class="pbtn sm" data-agent="claude:install" ${core.connecting ? 'disabled' : ''}>${esc(T(status === 'ok' || status === 'httpOnly' ? 'settings.reinstall' : 'settings.repair'))}</button><button class="pbtn sm no" data-agent="claude:remove" ${core.connecting ? 'disabled' : ''}>${esc(T('settings.remove'))}</button>`
    const direct = `<div class="ag-way"><div class="ag-way-h"><b>${esc(T('ag.claude.direct'))}</b><span class="grow"></span><span class="d">${esc(T(plugged && status === 'missing' ? 'ag.claude.notNeeded' : word))}</span></div>
      ${agentFiles('claude')}<div class="d">${esc(T('home.advancedWhy'))}</div>${buttons ? `<div class="ag-btns">${buttons}</div>` : ''}</div>`
    return plugin + direct
  }

  // DeepSeek Harness's way, to pick: her plugin (recommended) or DSH's own
  // hook bridge, the one in use lit (not connected yet: the one chosen for
  // Connect; being switched to: that one), and in grey what it means
  // (dshSwitch switches).
  function dshWay() {
    const r = Agents.read(core.snap, 'dsh') || {}
    const way = dshSwitching || (Agents.isIn(core.snap, 'dsh') ? r.way : r.via || 'plugin')
    const options = [
      ['plugin', T('ag.dsh.viaPlugin')],
      ['bridge', T('ag.dsh.viaBridge')],
    ]
    return `${field('ag.dsh.via', seg('dshVia', options, way))}<div class="d ag-way-note">${esc(T(`ag.dsh.viaNote.${way}`))}</div>`
  }

  // How one connects, in one word for all (the plugin or the hooks for Claude Code, as connected).
  function viaOf(id) {
    if (id !== 'claude') return T(`ag.via.${Agents.byId(id).via}`)
    const c = core.snap.agents?.claude?.connection
    return c === 'plugin' ? T('ag.via.plugin') : c === 'hooks' ? T('ag.via.hooks') : c === 'both' ? `${T('ag.via.plugin')} + ${T('ag.via.hooks')}` : T('ag.via.claudeNone')
  }

  function agentDetails(id, state) {
    const caps = Agents.capsOf(core.snap, id)
      .map(([cap, level, key]) => `<div class="ag-cap ${level}">${capIcon(cap, level, key)}<span class="ag-cap-n">${esc(T(`ag.cap.${cap}`))}</span><span class="d">${esc(T(key))}</span></div>`)
      .join('')
    const ours = Agents.isIn(core.snap, id) || state === 'repair'
    const busy = core.connecting === id
    // Connect, repair or install again; remove, with the file it is taken out of.
    const buttons =
      state === 'error' || state === 'unsupported'
        ? ''
        : ours
          ? `<button class="pbtn sm" data-agent="${id}:install" ${core.connecting ? 'disabled' : ''}>${busy ? `<span class="spin"></span>${esc(T('wc.busy'))}` : esc(T(state === 'repair' ? 'settings.repair' : 'settings.reinstall'))}</button><button class="pbtn sm no" data-agent="${id}:remove" ${core.connecting ? 'disabled' : ''}>${esc(T('settings.remove'))}</button>`
          : `<button class="pbtn sm al" data-agent="${id}:install" ${core.connecting ? 'disabled' : ''}>${busy ? `<span class="spin"></span>${esc(T('wc.busy'))}` : esc(T(state === 'failed' ? 'wc.toolRetry' : 'ag.do.install'))}</button>`
    // A version she does not connect to: no file of hers to show.
    const top = id === 'claude' ? claudeWays() : id === 'dsh' ? `${dshWay()}${agentFiles(id)}` : `${field('ag.via', esc(viaOf(id)))}${state === 'unsupported' ? '' : agentFiles(id)}`
    return `<div class="ag-det">${agentSteps(id, state)}${failed[id] ? core.failedHTML(id) : ''}${top}
        <div class="ag-what">${esc(T(Agents.whatOf(core.snap, id)))}</div>
        <div class="ag-caps">${caps}</div>
        ${agentNotes(id, state)}
        ${id === 'claude' ? '' : `<div class="ag-btns">${buttons}</div>`}
      </div>`
  }

  function pageAgents() {
    const here = Agents.here(core.snap)
    const away = Agents.elsewhere(core.snap)
    const left = Agents.toConnect(core.snap, known())
    // Hidden with none left; its tooltip names whom it connects.
    const all = left.length
      ? `<button class="pbtn sm al" data-ag-all title="${esc(T('ag.connectAllTip', { list: core.toolNames(left, 'and') }))}" ${core.connecting || connectingAll ? 'disabled' : ''}>${connectingAll ? `<span class="spin"></span>${esc(T('wc.busy'))}` : esc(T('ag.connectAll', { n: left.length }))}</button>`
      : ''
    const rows = here.length ? here.map(agentRow).join('') : `<div class="note">${esc(T('ag.noneHere'))}</div>`
    // What the icons are, once.
    const legend = Agents.CAPS.map(cap => `<span class="ag-key"><span class="cap yes">${Agents.CAP_GLYPH[cap]}</span>${esc(T(`ag.cap.${cap}`))}</span>`).join('')
    const others = away.length
      ? `${sec(T('ag.others'))}<div class="grp">
          <div class="r go" data-ag-others><span class="fi chev${core.othersOpen || !here.length ? ' open' : ''}">\uE76C</span><div class="grow ellip">${esc(nameList(core.lang, away.map(id => Agents.byId(id).name), 'and'))}</div><span class="d">${esc(T('ag.othersNote'))}</span></div>
          ${core.othersOpen || !here.length ? away.map(id => `<div class="r"><div class="grow"><div>${esc(Agents.byId(id).name)}${Agents.byId(id).aka ? `<span class="ver">${esc(Agents.byId(id).aka)}</span>` : ''}</div><div class="d">${esc(T(`ag.how.${id}`))}</div></div><span class="link" data-site="${Agents.byId(id).site}">${esc(T('ag.get'))} ›</span></div>`).join('') : ''}
        </div>`
      : ''
    return `<div class="sec row-sec"><span class="ellip">${esc(T('ag.found', { n: here.length }))}</span><span class="grow"></span>${all}</div>
      <div class="grp ag-list">${rows}</div>
      ${here.length ? `<div class="ag-legend">${legend}</div><div class="note ag-legend-note">${esc(T('ag.legend'))}</div>` : ''}
      ${others}`
  }

  // About: this version, the project's page, the credits (General's foot).
  const aboutHTML = () => `${sec(T('settings.about'))}<div class="grp">
        <div class="r"><div class="grow">${esc(T('s.version', { version: core.snap.version }))}</div><span class="link" data-site="repo">${esc(T('home.repo'))} ›</span></div>
        <div class="note">${esc(T('home.credits'))}</div>
      </div>`

  // Her updates (updates.rs): this version and the latest, its notes and
  // Update, or a look now; how an update goes (downloaded, checked, waiting
  // for a prompt's answer, restarting) or went wrong; looked for by itself
  // or not; downloaded from GitHub or Gitee first. A build run from the
  // source tree is never looked for.
  function pageUpdate() {
    const u = core.snap.update || {}
    const version = T('s.updateVersion', { version: u.current || core.snap.version })
    if (!u.may) return `${sec(T('s.update'))}<div class="grp">${row(esc(version), esc(T('s.updateSource')), '')}</div>`
    const latest = u.latest
    const newer = !!latest?.newer
    const mb = n => (n / 1048576).toFixed(1)
    let state = ''
    if (u.stage === 'download') state = T('s.updateDownloading', { from: u.from || '', pct: u.total ? Math.min(100, Math.floor((100 * u.got) / u.total)) : 0, got: mb(u.got || 0), total: mb(u.total || 0) })
    else if (u.stage) state = T(`s.updateStage.${u.stage}`)
    else if (u.checking) state = T('s.updateChecking')
    else if (u.error) state = T(`s.updateError.${u.error.code}`, { detail: u.error.detail || '' })
    else if (newer && !latest.canInstall) state = T('s.updateError.noDigest')
    else if (newer) state = T('s.updateNewer', { version: latest.version })
    else if (latest) state = T('s.updateLatest')
    else if (u.checkedAt) state = T('s.updateNone')
    const busy = !!u.stage || !!u.checking
    // Not reached (a look or a download): the way it went, and to the Network proxy below.
    const unreached = !busy && ['check', 'download'].includes(u.error?.code)
    if (unreached) state += (core.lang === 'zh' ? '。' : '. ') + T('s.updateProxyHint')
    const way = unreached ? `<div class="r"><div class="grow d">${esc(T('s.net.now', { what: netNow() }))}</div><span class="link" data-net-go>${esc(T('s.netProxy'))} ›</span></div>` : ''
    const buttons = newer
      ? `<span class="btns"><button class="pbtn sm" data-update="page">${esc(T('s.updateNotes'))}</button><button class="pbtn sm al" data-update="update" ${busy || !latest.canInstall ? 'disabled' : ''}>${esc(T('s.updateInstall'))}</button></span>`
      : `<button class="pbtn sm" data-update="check" ${busy ? 'disabled' : ''}>${esc(T('s.updateCheck'))}</button>`
    const title = newer ? `${version}${T('s.updateLatestIs', { version: latest.version })}` : version
    const when = u.checkedAt
      ? T('s.updateCheckedAt', { time: new Date(u.checkedAt).toLocaleString(core.lang === 'zh' ? 'zh-CN' : 'en-US', { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' }) })
      : ''
    return `${sec(T('s.update'))}<div class="grp">
        ${row(esc(title), esc(state), buttons)}${way}
        ${row(esc(T('s.updateAuto')), esc([T('s.updateAutoNote'), when].filter(Boolean).join(' · ')), sw('autoUpdateCheck', core.snap.settings.autoUpdateCheck !== false))}
        ${row(esc(T('s.updateFrom')), esc(T('s.updateFromNote')), seg('updateFrom', [['auto', T('s.updateFromAuto')], ['gitee', 'Gitee'], ['github', 'GitHub']], core.snap.settings.updateFrom || 'auto'))}
      </div>`
  }

  // Her crash reports (crash.rs): asked about once (yes from now on, or no),
  // the report shown as it would be sent; sent by themselves or not; those
  // waiting, sending, what went wrong, the last one's issue. Nothing while
  // there is nowhere to send to (a build from the source tree, no relay yet).
  function pageCrash() {
    const c = core.snap.crash || {}
    if (!c.can) return ''
    const when = ms => new Date(ms).toLocaleString(core.lang === 'zh' ? 'zh-CN' : 'en-US', { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' })
    const ask =
      c.mode === 'ask' && c.newest
        ? `<div class="grp">
            <div class="note">${esc(T('s.crashAsk', { when: when(c.newest.at) }))}</div>
            ${core.crashShown ? `<pre class="crash-text mono">${esc(c.newest.text)}</pre>` : ''}
            <div class="r"><button class="pbtn sm" data-crash-show>${esc(T(core.crashShown ? 's.crashHide' : 's.crashShow'))}</button><div class="grow"></div><span class="btns"><button class="pbtn sm" data-crash="no">${esc(T('s.crashNo'))}</button><button class="pbtn sm al" data-crash="yes">${esc(T('s.crashYes'))}</button></span></div>
          </div>`
        : ''
    let state = ''
    if (c.sending) state = T('s.crashSending')
    else if (c.mode === 'auto' && c.waiting && c.error) state = T('s.crashError', { n: c.waiting, detail: c.error })
    else if (c.mode === 'auto' && c.waiting) state = T('s.crashWaiting', { n: c.waiting })
    else if (c.last?.at) state = T('s.crashSent', { when: when(c.last.at), n: c.last.number ? `#${c.last.number}` : '' })
    const retry = c.mode === 'auto' && c.waiting && !c.sending ? `<button class="pbtn sm" data-crash="send">${esc(T('s.crashSend'))}</button>` : ''
    const open = c.last?.url ? `<button class="pbtn sm" data-crash="open">${esc(T('s.crashOpen'))}</button>` : ''
    return `${sec(T('s.crash'))}${ask}<div class="grp">
        ${row(esc(T('s.crashAuto')), esc([T('s.crashAutoNote'), state].filter(Boolean).join(' · ')), `${retry}${open}${sw('crashReports', c.mode === 'auto')}`)}
      </div>`
  }

  return { aboutHTML, agentDo, connectAll, dshSwitch, failed, known, pageAgents, pageCrash, pageUpdate, sessionOpen }
}
