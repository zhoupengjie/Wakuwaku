// The agents she follows, in one table: each one's name, how she connects to
// it, where that is written, what she can do for it (its capabilities) and
// what to know about it; and from the settings' snapshot, how each stands.
// The Agent page draws from it, the welcome on General, General's status
// line and the Mail page's picker too, so a new agent is a row here (and its
// words in i18n.js) besides its connector (connection/<id>.rs, a place in
// connection/mod.rs's ALL and its matches, its events_*.rs, its snapshot
// block in settings.rs). Loaded by
// the page before settings.js, and required by the tests.
//
// What she can do, five things, each yes, part or no, with the words that
// say so (a tooltip; the row's details list them), as the code has it:
//   status   follows its sessions (events.rs and each events_*.rs)
//   approve  answers its approvals on the island (asks.rs, panel.js): which
//            buttons; a PermissionRequest held for her
//   choose   answers a question with choices on the island (asks.rs
//            AskUserQuestion: Claude Code's, and DeepSeek Harness's through
//            her plugin; the others' questions only make her wait,
//            "detail.asksQuestion")
//   stats    counted in Today (main.rs count_today: every agent's turns and
//            time), and its tokens (tokens.rs: Claude Code, Codex, Antigravity,
//            omp, DeepSeek Harness, Reasonix)
//   mail     can read a letter for the Mail page (mail/agent.rs: claude,
//            codex, agy), when its command is found (snapshot mailAgents)
;(function (root) {
  const CAPS = ['status', 'approve', 'choose', 'stats', 'mail']
  // Windows' own glyphs for them (Segoe Fluent Icons): View, Accept,
  // BulletedList, AreaChart, Mail.
  const CAP_GLYPH = { status: '\uE890', approve: '\uE8FB', choose: '\uE8FD', stats: '\uE9D2', mail: '\uE715' }

  // Each one's own block of the snapshot (settings.rs): what of ours is
  // there (its status: absent when it is not on this computer), its file
  // and the same as read (~ for the home folder), whether a copy was kept,
  // whether an event came since she started; its version (Codex's in its
  // block, the others' where a file says it: agentVersions, connection/versions.rs
  // versions); whether it trusts ours as they are (Codex's, from its own
  // settings: true, false, null when not known). Each under the snapshot's
  // agents, by its id (settings.rs AgentRow).
  const own = (snap, key) => (snap.agents || {})[key] || {}
  const block = key => snap => {
    const b = own(snap, key)
    return { status: b.hooks, file: b.file || '', shown: b.fileShown || b.file || '', backup: !!b.backup, seen: !!b.seen, version: b.version || (snap.agentVersions || {})[key] || '', trusted: b.trusted ?? null }
  }

  // The table, in the order they are shown.
  //   name    as the island and the pages name it
  //   via     how she connects: hooks, plugin, extension, bridge
  //   site    where to get it (settings.rs settings_open_site, "agent:<id>")
  //   read    its part of the snapshot
  //   in      the statuses that mean ours is there (connected, or to repair)
  //   good    the ones of those that need nothing
  //   words   its own words for a status that wants something (wordOf)
  //   step    written, a step of the person's left before its first event
  //           (Codex: trusting the hooks in its /hooks; none once its own
  //           settings say they are trusted as they are): its button's words
  //   setLine written, no event yet: its line's words, from its read, where
  //           they depend on more than its id (ag.st.set.<id>)
  //   caps    [level, the words' key] for each of CAPS
  //   mail    its name on the Mail page (agent.rs Agent::name), if it reads mail
  const TABLE = [
    {
      id: 'claude',
      name: 'Claude Code',
      via: 'hooks',
      site: 'agent:claude',
      read: snap => {
        // Through the plugin, settings.json's hooks, both, or neither.
        const b = block('claude')(snap)
        const c = own(snap, 'claude').connection
        return {
          ...b,
          status: c === 'plugin' || c === 'both' ? c : b.status,
          seen: b.seen || c === 'plugin',
          here: own(snap, 'claude').here !== false || (!!c && c !== 'none'),
        }
      },
      in: ['plugin', 'both', 'ok', 'httpOnly', 'stale', 'partial'],
      good: ['plugin', 'ok', 'httpOnly'],
      words: { both: 'home.connBoth', unreadable: 's.hooksUnreadable' },
      caps: {
        status: ['yes', 'ag.cap.status.claude'],
        approve: ['yes', 'ag.cap.approve.claude'],
        choose: ['yes', 'ag.cap.choose.claude'],
        stats: ['yes', 'ag.cap.stats.tokens'],
        mail: ['yes', 'ag.cap.mail.claude'],
      },
      mail: 'Claude',
    },
    {
      id: 'codex',
      name: 'Codex',
      via: 'hooks',
      site: 'agent:codex',
      read: block('codex'),
      in: ['ok', 'stale', 'upgradable', 'partial'],
      good: ['ok'],
      words: { upgradable: 's.codexUpgradable', unreadable: 's.codexUnreadable' },
      step: 'ag.do.trust',
      // Its standing note while the step's own words show above it: the
      // rest of it, not the trust twice.
      stepNote: 'ag.note.codexWake',
      // Trusted already (zhoupengjie/Wakuwaku#66): only its next turn to wait for.
      setLine: r => (r.trusted === true ? 'ag.st.set.codexTrusted' : 'ag.st.set.codex'),
      caps: {
        status: ['yes', 'ag.cap.status.codex'],
        approve: ['yes', 'ag.cap.approve.codex'],
        choose: ['no', 'ag.cap.choose.codex'],
        stats: ['yes', 'ag.cap.stats.tokens'],
        mail: ['yes', 'ag.cap.mail.codex'],
      },
      mail: 'Codex',
    },
    {
      id: 'agy',
      name: 'Antigravity',
      via: 'hooks',
      site: 'agent:agy',
      read: block('agy'),
      in: ['ok', 'stale'],
      good: ['ok'],
      words: { off: 's.agyOff', unreadable: 's.agyUnreadable' },
      caps: {
        status: ['yes', 'ag.cap.status.agy'],
        approve: ['yes', 'ag.cap.approve.agy'],
        choose: ['no', 'ag.cap.choose.agy'],
        stats: ['yes', 'ag.cap.stats.agy'],
        mail: ['yes', 'ag.cap.mail.agy'],
      },
      mail: 'Antigravity',
    },
    {
      id: 'opencode',
      name: 'OpenCode',
      via: 'plugin',
      site: 'agent:opencode',
      read: block('opencode'),
      in: ['ok', 'stale'],
      good: ['ok'],
      words: { stale: 's.opencodeStale', unreadable: 's.opencodeUnreadable' },
      caps: {
        status: ['yes', 'ag.cap.status.opencode'],
        approve: ['yes', 'ag.cap.approve.opencode'],
        choose: ['no', 'ag.cap.choose.opencode'],
        stats: ['yes', 'ag.cap.stats.turns'],
        mail: ['no', 'ag.cap.mail.no'],
      },
    },
    {
      id: 'omp',
      name: 'omp',
      // Its project's name, beside its command's.
      aka: 'oh-my-pi',
      via: 'extension',
      site: 'agent:omp',
      // Its extension in each profile's agent folder too: the first one's
      // path, and how many more.
      read: snap => ({ ...block('omp')(snap), more: own(snap, 'omp').more || 0 }),
      in: ['ok', 'stale', 'partial'],
      good: ['ok'],
      words: { stale: 's.ompStale', partial: 's.ompPartial', unreadable: 's.ompUnreadable' },
      caps: {
        status: ['yes', 'ag.cap.status.omp'],
        approve: ['part', 'ag.cap.approve.omp'],
        choose: ['no', 'ag.cap.choose.omp'],
        stats: ['yes', 'ag.cap.stats.tokens'],
        mail: ['no', 'ag.cap.mail.no'],
      },
    },
    {
      id: 'zcode',
      name: 'ZCode',
      via: 'hooks',
      site: 'agent:zcode',
      read: block('zcode'),
      in: ['ok', 'stale', 'partial'],
      good: ['ok'],
      words: { off: 's.zcodeOff', unreadable: 's.zcodeUnreadable' },
      caps: {
        status: ['yes', 'ag.cap.status.zcode'],
        approve: ['yes', 'ag.cap.approve.zcode'],
        choose: ['no', 'ag.cap.choose.zcode'],
        stats: ['yes', 'ag.cap.stats.turns'],
        mail: ['no', 'ag.cap.mail.no'],
      },
    },
    {
      id: 'dsh',
      name: 'DeepSeek Harness',
      via: 'plugin',
      site: 'agent:dsh',
      // Her plugin's file in DSH's folder, besides the patch file; whether
      // DSH loaded it (connection/dsh.rs dsh_view: load), which counts as seen;
      // the old bridge's rows as a status of their own (bridge), and one
      // written but not loaded while DSH runs (notLoaded, why: off when DSH's
      // own settings turned it off).
      // Two ways (way, connection/dsh.rs dsh_way): her plugin, or DSH's own hook
      // bridge, whose stale rows say why as statuses of their own (way: the
      // plugin's rows there; hooks: its hook list for another exe or
      // folder; packages: not in DSH's profiles; version: another DSH's).
      read: snap => {
        const b = block('dsh')(snap)
        const d = own(snap, 'dsh')
        const load = d.load || {}
        const way = d.way === 'bridge' ? 'bridge' : 'plugin'
        const bridgeWhy = { way: 'bridgeWay', hooks: 'bridgeHooks', packages: 'bridgePackages', version: 'bridgeVersion' }
        const status = b.status !== 'stale' ? b.status : way === 'bridge' ? bridgeWhy[d.why] || 'stale' : d.why === 'bridge' ? 'bridge' : 'stale'
        // Connected: DSH runs her plugin, or the bridge's events came since
        // DSH started; or they came since she started and DSH is not
        // running now. DSH running without it (to be restarted, or the
        // bridge's first event still to come) is not, whatever came before.
        const seen = ['loaded', 'older', 'effective'].includes(load.state) || (b.seen && !['restart', 'firstEvent'].includes(load.state))
        // Its file besides the patch file: the plugin's, or the bridge's hook list.
        const list = way === 'bridge' ? d.hooksShown || '' : d.listShown || ''
        return { ...b, status, way, via: d.via || null, why: d.why || '', load, seen, list, listKey: way === 'bridge' ? 'ag.list' : 'ag.pluginFile' }
      },
      // The bridge's way says what it does in words of its own: no step's
      // tool (a hook costs a second, so few are hooked), no approval nor
      // question on the island (it has no PermissionRequest), its note.
      wording: r =>
        r.way === 'bridge'
          ? {
              what: 'ag.what.dshBridge',
              note: 'ag.note.dshBridge',
              caps: {
                status: ['part', 'ag.cap.status.dshBridge'],
                approve: ['no', 'ag.cap.approve.dshBridge'],
                choose: ['no', 'ag.cap.choose.dshBridge'],
                stats: ['yes', 'ag.cap.stats.dsh'],
              },
            }
          : null,
      in: ['ok', 'stale', 'bridge', 'notLoaded', 'bridgeWay', 'bridgeHooks', 'bridgePackages', 'bridgeVersion'],
      good: ['ok'],
      words: {
        bridge: 's.dshBridge',
        stale: 's.dshStale',
        notLoaded: 's.dshNotLoaded',
        unreadable: 's.dshUnreadable',
        bridgeWay: 's.dshBWay',
        bridgeHooks: 's.dshBHooks',
        bridgePackages: 's.dshBPackages',
        bridgeVersion: 's.dshBVersion',
      },
      // Written, not working yet: DSH to start, or to be restarted; the
      // bridge's first event still to come.
      setLine: r => (r.load?.state === 'restart' ? 'ag.st.set.dsh' : r.load?.state === 'firstEvent' ? 'ag.st.set.dshFirstEvent' : 'ag.st.set.dshWaiting'),
      // Its approvals and questions answered on the island through her
      // plugin (asks.rs), DSH's own window asking all the while.
      caps: {
        status: ['yes', 'ag.cap.status.dsh'],
        approve: ['yes', 'ag.cap.approve.dsh'],
        choose: ['yes', 'ag.cap.choose.dsh'],
        stats: ['yes', 'ag.cap.stats.tokens'],
        mail: ['no', 'ag.cap.mail.no'],
      },
    },
    {
      id: 'reasonix',
      name: 'Reasonix',
      via: 'hooks',
      site: 'agent:reasonix',
      // Its line too: 2.x, 1.x (the same hooks), or the Node 0.x, which she
      // does not write to (unsupported).
      read: snap => ({ ...block('reasonix')(snap), line: own(snap, 'reasonix').line || '' }),
      in: ['ok', 'stale', 'partial'],
      good: ['ok'],
      words: { unsupported: 's.reasonixOld', unreadable: 's.reasonixUnreadable' },
      // Phase 1 of zhoupengjie/Wakuwaku#60: its sessions followed, its
      // approvals still answered in Reasonix.
      caps: {
        status: ['yes', 'ag.cap.status.reasonix'],
        approve: ['no', 'ag.cap.approve.reasonix'],
        choose: ['no', 'ag.cap.choose.reasonix'],
        stats: ['yes', 'ag.cap.stats.reasonix'],
        mail: ['no', 'ag.cap.mail.no'],
      },
    },
  ]

  const byId = id => TABLE.find(a => a.id === id) || null

  // The words for a status that wants something (to repair, unreadable),
  // its own where it has them (words), else these.
  const WORDS = { stale: 'settings.hooksStale', partial: 'settings.hooksPartial', unreadable: 'ag.st.unreadable', off: 'ag.st.off' }
  const wordOf = (id, status) => byId(id)?.words?.[status] || WORDS[status] || 'settings.hooksStale'

  // One's part of the snapshot, as each reads it; here unless its status
  // says it is absent (no folder of its own on this computer).
  function read(snap, id) {
    const a = byId(id)
    if (!a || !snap) return null
    const r = a.read(snap)
    return { ...r, here: r.here ?? (r.status !== undefined && r.status !== 'absent') }
  }

  // Ours is there: connected, or there to repair.
  const isIn = (snap, id) => {
    const r = read(snap, id)
    return !!r && byId(id).in.includes(r.status)
  }

  // How one stands, for its dot and its button:
  //   busy     being written now
  //   on       connected, an event seen (a session came)
  //   set      written, no event yet: nothing for the person to do but use it
  //   step     written, no event yet, and a step of the person's first
  //            (Codex: trusting the hooks; the table's step), not known
  //            to be taken already (Codex's trust read from its settings)
  //   repair   ours there but not as this copy would write it (moved, an
  //            older version, a part missing, turned off, both of Claude
  //            Code's ways at once)
  //   error    its file can't be read
  //   failed   the last write failed (why: the page keeps it)
  //   unsupported  here, but a version she cannot connect to (Reasonix 0.x,
  //            whose hooks are not 2.x's): nothing to press, its row says why
  //   none     not connected
  // The page passes what it knows: { busy: id being written, failed: { id: why } }.
  function stateOf(snap, id, { busy = '', failed = {} } = {}) {
    if (busy === id) return 'busy'
    const a = byId(id)
    const r = read(snap, id)
    if (!a || !r) return 'none'
    if (a.in.includes(r.status)) return !a.good.includes(r.status) ? 'repair' : r.seen ? 'on' : a.step && r.trusted !== true ? 'step' : 'set'
    if (r.status === 'off') return 'repair'
    if (r.status === 'unsupported') return 'unsupported'
    if (failed[id]) return 'failed'
    if (r.status === 'unreadable') return 'error'
    return 'none'
  }

  // The states that want the person (a repair, a step, a file to put right,
  // a write that failed): amber, and the Agent tab's dot.
  const WANTS = ['step', 'repair', 'error', 'failed']

  // The mood whose colour a state's dot takes (the island's, settings.js
  // COLOR), four of them: green connected, blue written and waiting for
  // its first turn, amber the person must act, grey not connected (or not
  // to be, this version). Colour says how it stands, never which one it is.
  const MOOD = { on: 'done', set: 'working', busy: 'working', step: 'waiting', repair: 'waiting', error: 'waiting', failed: 'waiting', unsupported: 'idle', none: 'idle' }

  // Its row's button, only where the person has something to do:
  //   install   connect (or try again), or repair
  //   remove    Claude Code with both ways at once: settings.json's hooks
  //             taken out, the plugin kept (its Repair)
  //   open      a step to take (Codex's trust): the row opens to say how
  //   null      none: connected, written and waiting, being written, or its
  //             file unreadable (to be put right by hand: its details say so)
  // Reinstall and Remove are in the open row's details.
  function actionOf(snap, id, known) {
    const state = stateOf(snap, id, known)
    if (state === 'none' || state === 'failed') return { act: 'install', label: state === 'failed' ? 'wc.toolRetry' : 'ag.do.install' }
    if (state === 'repair') return id === 'claude' && read(snap, id).status === 'both' ? { act: 'remove', label: 'settings.repair' } : { act: 'install', label: 'settings.repair' }
    if (state === 'step') return { act: 'open', label: byId(id).step }
    return null
  }

  // The ones on this computer, and the others, in the table's order.
  const here = snap => TABLE.filter(a => read(snap, a.id)?.here).map(a => a.id)
  const elsewhere = snap => TABLE.filter(a => !read(snap, a.id)?.here).map(a => a.id)

  // Those on this computer and not connected, for Connect all.
  const toConnect = (snap, known) => here(snap).filter(id => ['none', 'failed'].includes(stateOf(snap, id, known)))

  // What she can do for one, each of CAPS: [cap, level, the words' key].
  // Mail where its command is found (agent.rs available): else no, and why.
  function capsOf(snap, id) {
    const a = byId(id)
    const own = (snap && wordingOf(snap, id)?.caps) || {}
    return CAPS.map(cap => {
      const [level, key] = own[cap] || a.caps[cap]
      if (cap === 'mail' && level !== 'no' && snap && !(snap.mailAgents || {})[id]) return [cap, 'no', `ag.cap.mail.missing.${id}`]
      return [cap, level, key]
    })
  }

  // General's status line and the Agent tab's dot: the ones on this
  // computer by how they stand. connected: written (Codex's still to be
  // trusted too); wants: those the person must act on (amber); none: those
  // still to connect (one of a version she cannot connect to is not).
  function summary(snap, known) {
    const out = { here: here(snap), on: [], set: [], wants: [], none: [] }
    for (const id of out.here) {
      const state = stateOf(snap, id, known)
      if (state === 'on' || state === 'set') out[state].push(id)
      else if (WANTS.includes(state)) out.wants.push(id)
      else if (state !== 'unsupported') out.none.push(id)
    }
    out.connected = out.here.filter(id => out.on.includes(id) || out.set.includes(id) || stateOf(snap, id, known) === 'step')
    return out
  }

  // The Mail page's picker: those that read mail, each found (its command
  // here) or not, and why not. Antigravity only where it is, or picked
  // already (few have it; zhoupengjie/Wakuwaku#42): its app alone brings no
  // agy, so it is listed then, greyed.
  function mailAgents(snap, current) {
    const found = snap.mailAgents || {}
    return TABLE.filter(a => a.mail)
      .filter(a => a.id !== 'agy' || found.agy || current === 'agy' || read(snap, 'agy').here)
      .map(a => ({ id: a.id, name: a.name, short: a.mail, found: !!found[a.id] }))
  }

  // How many sessions of one are in sight now (state.rs: agent, none for Claude Code's).
  const sessionsOf = (snap, id) => (snap.sessions || []).filter(s => (s.agent || 'claude') === id).length

  // Words of its own for how it stands now (wording: DSH's bridge), or none.
  const wordingOf = (snap, id) => byId(id)?.wording?.(read(snap, id) || {}) || null

  // The standing note at the foot of one's details (ag.note.<id>); while
  // its step shows in amber at their top, the part that does not say it
  // again; its own words for how it stands, given the snapshot.
  const noteOf = (id, state, snap) => (state === 'step' && byId(id)?.stepNote) || (snap && wordingOf(snap, id)?.note) || `ag.note.${id}`

  // What it can do and not, in a sentence (ag.what.<id>, or its own words for how it stands).
  const whatOf = (snap, id) => wordingOf(snap, id)?.what || `ag.what.${id}`

  // Its line's words while written and waiting for its first event (set, step).
  const setLineOf = (snap, id) => byId(id)?.setLine?.(read(snap, id) || {}) || `ag.st.set.${id}`

  const api = { CAPS, CAP_GLYPH, TABLE, MOOD, WANTS, byId, noteOf, whatOf, setLineOf, wordOf, read, isIn, stateOf, actionOf, here, elsewhere, toConnect, capsOf, summary, mailAgents, sessionsOf }

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api
  } else {
    root.Agents = api
  }
})(this ?? window)
