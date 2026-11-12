// The Plugins page: the built-in ones, those she runs and other people's,
// where each is placed; and the monitor's settings folded out under it (its
// sensors: Windows' own, the CPU helper, or LibreHardwareMonitor).
// Made by settings.ts in the section's place as the settings are made
// (pluginsPart(core), imported by settings.ts). The settings' core (SettingsCore):
// their constants and functions as they are, what they remember in core.st
// (settings-vars.ts, read and written as st.<name>), and the parts made
// before this one in core.pieces (a later one's reached once the settings
// are up). What it hands back is the settings' again.
import * as AddonUI from './addonui.ts'
import * as Widgets from './widgets.ts'
import type { SettingsCore } from './settings.ts'

export function pluginsPart(core: SettingsCore) {
  const {
    st, pieces, AddonUI, COLOR, SPINS, T, Widgets, drafts, esc, market, openHardware, placeHTML, placeOf,
    pluginFields, pluginNote, render, row, sec, seg, sw, widgetRows,
  } = core
  const { SUBS, addonRow, packsGroup } = pieces.addons

  // The monitor's settings, folded out under it: pinned or not, how often it
  // reads (1 to 10 seconds), where its sensors come from (monitorSource:
  // Windows' own, hw.rs, or LibreHardwareMonitor, lhm.rs; each keeps its own
  // picks; none chosen, the one in effect, snap.monitorSource), and those
  // sensors.
  function monitorFields() {
    const m = st.snap.settings.monitor || {}
    const source = st.snap.monitorSource === 'lhm' ? 'lhm' : 'builtin'
    const every = m.every || 2
    const pace = `<div class="fr"><span class="fl">${esc(T('mon.every'))}</span><span class="grow"></span><button class="pbtn sm" data-every="-1" ${every <= 1 ? 'disabled' : ''}>−</button><span class="every">${esc(T('settings.seconds', { n: every }))}</span><button class="pbtn sm" data-every="1" ${every >= 10 ? 'disabled' : ''}>+</button></div>`
    const pin = `<div class="fr"><span class="fl">${esc(T('mon.pin'))}</span><span class="d grow">${esc(T('mon.pinNote'))}</span>${sw('mon:pin', m.pin !== false)}</div>`
    // The one in effect; none chosen yet, it says how that one was found
    // (hw.rs Source::in_effect), and picking one keeps it.
    const auto = st.snap.settings.monitorSource == null ? `<div class="note lhm-note">${esc(T('mon.sourceAuto'))}</div>` : ''
    const from = `<div class="fr lhm-head"><span class="fl">${esc(T('mon.source'))}</span><span class="grow"></span>${seg('monitorSource', [['builtin', T('mon.sourceBuiltin')], ['lhm', 'LibreHardwareMonitor']], source)}</div>${auto}`
    if (source === 'builtin') return `<div class="pf">${pin}${pace}${from}${builtinFields()}</div>`
    const parts = ['cpu', 'mem', 'net', 'battery'].map(k => `<div class="fr"><span class="fl">${esc(T(`mon.${k}`))}</span><span class="grow"></span>${sw('mon:' + k, m[k] !== false)}</div>`)
    return `<div class="pf">${pin}${pace}${from}${parts.join('')}${sensorFields(m)}</div>`
  }

  // The setting in "monitor" with a source's picks (hw.rs Source::picks).
  const picksKey = source => (source === 'lhm' ? 'sensors' : 'builtinSensors')

  // Sensors under their hardware ({ id, name, about, list, more }), folded
  // (the ones on still shown): each hardware's name, what it is and how
  // many are picked; each sensor its name, reading and switch (key: what its
  // switch is: lhms:<id> or hws:<id>, picked; mon:<part>, one of the
  // monitor's own). One for the settings alone (info) has none; `more`,
  // a hardware's own lines (a drive's health), shows folded out.
  function sensorGroups(groups, keyOf) {
    return groups
      .map(g => {
        const open = openHardware.has(g.id)
        const shown = s => s.on && !s.info
        const picked = g.list.filter(shown).length
        const about = g.about ? ` <span class="d">${esc(g.about)}</span>` : ''
        const title = `<div class="fr lhw" data-lhw="${esc(g.id)}"><span class="fl">${open ? '▾' : '▸'} ${esc(g.name)}${about}</span><span class="d">${picked ? esc(T('mon.lhmTicked', { n: picked })) : ''}</span></div>`
        const rows = (open ? g.list : g.list.filter(shown)).map(s => `<div class="fr lsen${s.info ? ' info' : ''}"><span class="fl">${esc(s.name)}</span><span class="d">${esc(s.reading)}</span>${s.info ? '' : sw(esc(keyOf(s)), s.on)}</div>`)
        return title + rows.join('') + (open ? g.more || '' : '')
      })
      .join('')
  }

  // Windows' own readings (hw.rs, snap.hw): under the CPU, each card, the
  // memory, each drive, the network and the battery, as LibreHardwareMonitor's
  // are; the CPU's use, the memory's, the network's and the battery on the
  // monitor's own switches, the rest picked; an NVMe drive's health in its
  // own line, for the settings alone.
  function builtinFields() {
    const v = st.snap.hw
    if (!v || !Array.isArray(v.hardware)) return `<div class="note lhm-note">${esc(T('hw.unread'))}</div>`
    const hardwareName = h => h.name || T(`hw.${h.class}`)
    const groups = v.hardware.map(h => {
      const health = h.health ? `<div class="fr lsen info"><span class="fl">${esc(T('hw.health'))}</span><span class="d">${esc(T('hw.healthReading', { used: h.health.used, spare: h.health.spare, hours: h.health.hours }))}</span></div>` : ''
      const about = h.about && typeof h.about === 'object' ? render(st.lang, h.about) : h.about || ''
      const list = (v.sensors || []).filter(s => s.hardwareId === h.id).map(s => ({ ...s, name: T(s.label) }))
      return { id: h.id, name: hardwareName(h), about, list, more: health }
    })
    const note = `<div class="note lhm-note">${esc(T('mon.builtinNote'))}</div>`
    return note + cpuSensorsRow() + sensorGroups(groups, s => (s.toggle ? `mon:${s.toggle}` : `hws:${s.id}`))
  }

  // The CPU temperature/power helper (#18 part B, snap.cpuSensors): a switch
  // with a short line saying what it needs and how it is going. It needs the
  // PawnIO driver and a helper that runs as administrator (one UAC to set
  // up). Its state is not set up / setting up / running (with the CPU's
  // temperature) / PawnIO missing (offer to install it) / not answering
  // (the task is there but nothing comes — a VM may not expose the
  // registers). Turning it off removes the task and leaves PawnIO installed.
  function cpuSensorsRow() {
    const cs = st.snap.cpuSensors || {}
    const state = cs.state || 'off'
    const on = cs.on === true
    // needsUpdate: set up again to copy the newer helper. Off: set up (or,
    // with no PawnIO, download it first). On: remove.
    const action = state === 'needsUpdate' ? 'install' : on ? 'remove' : state === 'pawnioMissing' ? 'installPawnIO' : 'install'
    const bad = state === 'notAnswering' || state === 'pawnioMissing' || state === 'needsUpdate'
    const temp = typeof cs.temp === 'number' ? `${cs.temp.toFixed(1)} °C` : ''
    const note =
      state === 'running'
        ? T('sensors.running', { reading: temp || '—' })
        : state === 'settingUp'
          ? T('sensors.settingUp')
          : state === 'notAnswering'
            ? T('sensors.notAnswering')
            : state === 'needsUpdate'
              ? T('sensors.needsUpdate')
              : state === 'pawnioMissing'
                ? T('sensors.pawnioMissing')
                : T('sensors.note')
    const title = `<div class="fr"><span class="fl">${esc(T('sensors.title'))}</span><span class="grow"></span><button class="sw${on ? ' on' : ''}" data-cpu-sensors="${action}"></button></div>`
    const line = `<div class="note lhm-note${bad ? ' bad' : ''}">${esc(note)}</div>`
    // On: a dim line saying removing it keeps PawnIO.
    const removeHint = on ? `<div class="note lhm-note">${esc(T('sensors.removeNote'))}</div>` : ''
    return title + line + removeHint
  }

  // LibreHardwareMonitor's sensors, from one the person runs (lhm.rs,
  // snap.lhm): whether it was found, and if not, how to; its sensors to
  // pick, under their hardware (folded, the ones picked still shown); its
  // port, and a user and password for its authentication.
  function sensorFields(m) {
    const head = `<div class="note lhm-note">${esc(T('mon.lhmNote'))}</div>`
    const v = st.snap.lhm || {}
    const port = m.lhmPort || 8085
    const status =
      v.status === 'found'
        ? T(!(v.sensors || []).length ? 'mon.lhmNone' : v.version ? 'mon.lhmFoundVersion' : 'mon.lhmFound', { version: v.version || '' })
        : v.status === 'auth'
          ? T('mon.lhmAuth')
          : v.status === 'strange'
            ? T('mon.lhmStrange', { port })
            : v.status === 'missing'
              ? T('mon.lhmMissing', { port })
              : T('mon.lhmUnread')
    // Where to get it, while none answers.
    const get = v.status === 'found' || v.status === 'auth' || v.status === 'strange' ? '' : ` <span class="link" data-site="lhm">${esc(T('mon.lhmGet'))} ›</span>`
    // What she offers in its place (lhm_setup.rs): download it, start one
    // found, the steps; or, answering, where it is and its start at logon.
    const note = lhmSetup(port, `<div class="note lhm-note${v.status === 'found' ? '' : ' bad'}">${esc(status)}${get}</div>`)
    // Its sensors by hardware, in LHM's order.
    const groups = []
    for (const s of v.status === 'found' ? v.sensors || [] : []) {
      const last = groups[groups.length - 1]
      if (last && last.id === s.hardwareId) last.list.push(s)
      else groups.push({ id: s.hardwareId, name: s.hardware || s.hardwareId, list: [s] })
    }
    const sensors = sensorGroups(groups, s => 'lhms:' + s.id)
    const field = (key, label, attrs, value) => `<label class="fr"><span class="fl">${esc(label)}</span><input class="in" data-mon="${key}" value="${esc(`mon:${key}` in drafts ? drafts[`mon:${key}`] : value)}" spellcheck="false" ${attrs}></label>`
    // The password is never shown again: kept, it says so, and can be forgotten.
    const kept = m.lhmPass === true
    const forget = kept ? `<div class="fr"><span class="fl"></span><span class="d grow">${esc(T('mon.lhmPassKept'))}</span><button class="pbtn sm" data-lhm-forget>${esc(T('mon.lhmForget'))}</button></div>` : ''
    const reach =
      field('lhmPort', T('mon.lhmPort'), 'inputmode="numeric" placeholder="8085"', m.lhmPort || '') +
      field('lhmUser', T('mon.lhmUser'), `placeholder="${esc(T('mon.lhmOptional'))}" autocomplete="off"`, m.lhmUser || '') +
      field('lhmPass', T('mon.lhmPass'), `type="password" autocomplete="off" placeholder="${esc(kept ? '••••••' : T('mon.lhmOptional'))}"`, '') +
      forget
    return `${head}${note}${sensors}${reach}`
  }

  // LibreHardwareMonitor got, set up and started for her (lhm_setup.rs,
  // snap.lhmSetup), by its step: get (none found: what she would do, where
  // it goes, its start at logon, the download), start / turnOn (one found,
  // stopped), steps / otherPort (running, not answering), busy (a stage
  // under way); answering: LHM's own note (`note`), where it is, and for hers
  // (in Program Files, only administrators change it) its task and its
  // removal; someone's own is theirs, its start at logon LHM's menu's. What
  // went wrong last above it. Nothing off Windows, nor till it was first
  // looked for (looking).
  function lhmSetup(port, note) {
    const u = st.snap.lhmSetup
    if (!u || !u.step || u.step === 'off' || u.step === 'looking') return note
    const line = (text, bad?) => `<div class="note lhm-note${bad ? ' bad' : ''}">${text}</div>`
    const button = (what, label, accent?) => `<div class="fr"><span class="fl"></span><span class="grow"></span><button class="pbtn sm${accent ? ' al' : ''}" data-lhm="${what}">${esc(label)}</button></div>`
    const i = u.install
    const where = i ? i.dir || i.shown : ''
    const code = u.error?.code || ''
    const failed = code ? line(`${esc(T(`lhm.err${code[0].toUpperCase()}${code.slice(1)}`, { detail: u.error.detail || '', port, dir: u.dir || '' }))} <span class="link" data-lhm="dismiss">${esc(T('lhm.dismiss'))}</span>`, true) : ''
    // It asks for PawnIO itself as it starts, until it is there (or older than 2.0).
    const pawnio = !u.pawnio || /^[01]\./.test(u.pawnio) ? ` ${esc(T('lhm.pawnio'))}` : ''
    const startup = `<div class="fr"><span class="fl">${esc(T('lhm.startup'))}</span><span class="d grow">${esc(T('lhm.startupNote'))}</span>${sw('mon:lhmStartup', u.startupWanted !== false)}</div>`
    let body
    if (u.step === 'busy') {
      const j = u.job || {}
      const mb = n => (n / 1048576).toFixed(1)
      const name = j.stage || 'look'
      const stage = name === 'download' ? T('lhm.stageDownload', { asset: j.asset || '', version: j.version || '', pct: j.total ? Math.min(100, Math.floor((100 * j.got) / j.total)) : 0, got: mb(j.got || 0), total: mb(j.total || 0) }) : T(`lhm.stage${name[0].toUpperCase()}${name.slice(1)}`, { dir: u.dir || '' })
      body = line(esc(stage) + (name === 'wait' || name === 'install' ? pawnio : ''))
    } else if (u.step === 'get') {
      body =
        line(esc(T('lhm.notFound')), true) +
        line(`${esc(T('lhm.separate'))} ${esc(T('lhm.getNote', { dir: u.dir, port }))}`) +
        startup +
        button('get', T('lhm.get'), true) +
        line(`${pawnio.trim()} ${esc(T('lhm.removeLater'))}`.trim()) +
        line(`${esc(T('lhm.elsewhere'))} <span class="link" data-site="lhm">${esc(T('mon.lhmGet'))} ›</span>`)
    } else if (u.step === 'start' || u.step === 'turnOn') {
      // Hers turned on: the step as administrator, which starts it too. Hers
      // stopped is where it can be removed (not while it runs).
      const how = u.startup && !(u.step === 'turnOn' && i?.hers) ? T('lhm.byTask') : T('lhm.asks')
      const remove = i?.hers ? button('remove', T('lhm.uninstall')) : ''
      body = line(esc(T('lhm.foundStopped', { path: where })), true) + line(`${esc(T(u.step === 'turnOn' ? 'lhm.turnOnNote' : 'lhm.startNote', { port }))} ${esc(how)}${pawnio}`) + button('start', T(u.step === 'turnOn' ? 'lhm.turnOn' : 'lhm.start'), true) + remove
    } else if (u.step === 'otherPort') {
      body = line(esc(T('lhm.otherPort', { other: i?.port || '' })), true) + button('usePort', T('lhm.usePort', { other: i?.port || '' }), true)
    } else if (u.step === 'steps') {
      body = line(esc(T('lhm.runningOff', { path: where || 'LibreHardwareMonitor', port })), true)
    } else {
      // Answering: where it is; hers, its task and its removal.
      const about = i ? line(`${esc(T('lhm.where', { path: where }))} ${esc(T(i.hers ? 'lhm.hers' : 'lhm.theirs'))} <span class="link" data-lhm="folder">${esc(T('lhm.folder'))} ›</span>`) : ''
      const task =
        i && i.hers
          ? `<div class="fr"><span class="fl">${esc(T('lhm.startup'))}</span><span class="d grow">${esc(T(u.startup ? 'lhm.startupOn' : 'lhm.startupOff'))}</span><button class="pbtn sm" data-lhm="${u.startup ? 'noStartup' : 'startup'}">${esc(T(u.startup ? 'lhm.startupUnset' : 'lhm.startupSet'))}</button></div>`
          : ''
      const other = u.taskOther && i?.hers ? line(esc(T('lhm.taskOther', { path: u.taskOther }))) : ''
      const remove = i?.hers ? line(esc(T('lhm.remove', { dir: u.dir }))) + button('remove', T('lhm.uninstall')) : ''
      body = note + about + task + other + remove
    }
    return failed + body
  }
  // Plugins: built in, run by her, or anyone's script, none with a switch
  // (the user's call of 2026-10-17: a switch both started one and hid it
  // from her capsule, its desktop copy stopped with it; placed is on now,
  // picker.ts). Each says how it is and where it is put, or, put nowhere,
  // offers 添加小工具…; its settings folded under it. Then how they take
  // turns, whether they may open the island, whether the market's update
  // by themselves, installing one from a file, and how to write one. At the
  // top, 获取更多 › (the market), and updates waiting under it. Other
  // people's among hers (addonRow), a folder that did not install a quiet
  // line under them; the packs installed after them (资源包, packsGroup);
  // one's own page, a pack's, the market or one of its own in place of all
  // this while it is open (sub).
  function pageWidgets() {
    const own = st.sub && SUBS[st.sub.page]?.(st.sub)
    if (own) return own
    st.sub = null
    const s = st.snap.settings
    const left = ms => (ms >= 60000 ? T('settings.minutes', { n: Math.round(ms / 60000) }) : T('settings.seconds', { n: Math.max(1, Math.round(ms / 1000)) }))
    const rows = widgetRows()
      .map(r => {
        if (r.addon) return addonRow(r.addon, r.mine)
        if (r.plugin) {
          const p = r.plugin
          const place = placeOf({ plugin: p.id }, p.on)
          const says = r.mine.map(w => Widgets.words(st.lang, w, s.details !== false)).map(({ label, value }) => [label, value].filter(Boolean).join(' ')).join(' · ')
          const note = pluginNote(p, r.mine, !!place.at)
          const color = (r.mine[0] && r.mine[0].color) || (p.on ? '#5e9bff' : COLOR.idle)
          const isOpen = st.openPlugin === p.id
          const more = p.params.length ? `<button class="pbtn sm" data-pexp="${esc(p.id)}">${esc(T(isOpen ? 'pl.fold' : 'pl.settings'))}</button>` : ''
          return `<div class="r" data-part="widgets:${esc(r.key)}"><span class="wi" style="color:${esc(color)}">${Widgets.icon(p.icon)}</span><div class="grow"><div class="ellip">${esc(T(`w.ex.${p.id}`))}<span class="d"> ${esc(says)}</span></div><div class="d${note.bad ? ' bad' : ''}">${esc(note.text)}</div></div>${more}${placeHTML(place, `plugin:${p.id}`)}</div>${isOpen ? pluginFields(p) : ''}`
        }
        const w = r.widget
        const place = placeOf({ id: w.id }, w.on)
        const { label, value } = Widgets.words(st.lang, w, s.details !== false)
        // A built-in one says what it shows; on, with nothing read, says so.
        const builtIn = w.builtIn && [T('w.builtIn'), T(`w.about.${w.id}`), w.on && w.value == null ? T('w.unread') : ''].filter(Boolean).join(' · ')
        const from = builtIn || (w.from === 'mail' ? T('w.fromMail') : [T('w.script', { time: left(w.leftMs || 0) }), w.private ? T('w.private') : ''].filter(Boolean).join(' · '))
        const state = place.at ? `${T('pl.running')} · ${from}` : T('pl.unplaced')
        // The monitor: its readings as icon and number, its settings folded under it.
        const says = Widgets.partsOf(w) ? ` ${Widgets.partsHTML(w)}` : `<span class="d"> ${esc(value)}</span>`
        const isMonitor = w.id === 'monitor'
        const more = isMonitor ? `<button class="pbtn sm" data-pexp="monitor">${esc(T(st.openPlugin === 'monitor' ? 'pl.fold' : 'pl.settings'))}</button>` : ''
        return `<div class="r" data-part="widgets:${esc(r.key)}"><span class="wi" style="color:${esc(w.color || COLOR.idle)}">${Widgets.iconHTML(w, st.lang)}</span><div class="grow"><div class="ellip">${esc(label)}${says}</div><div class="d">${esc(state)}</div></div>${more}${placeHTML(place, `widget:${w.id}`)}</div>${isMonitor && st.openPlugin === 'monitor' ? monitorFields() : ''}`
      })
      .join('')
    const spins = SPINS.map(n => [n, n ? T('settings.seconds', { n }) : T('w.spinOff')])
    const example = `Invoke-RestMethod -Method Post http://127.0.0.1:${st.snap.port}/widget -ContentType application/json -Body '{"id":"hello","label":"Hello","value":"42","icon":"star"}'`
    const broken = (st.snap.addonsBroken || []).map(b => `<div class="note ad-broken ellip" title="${esc(b.reason || '')}">${esc(AddonUI.brokenLine(b, T))}</div>`).join('')
    const fromFile = `<div class="r jumps" data-addon-file role="button" tabindex="0"><div class="grow"><div>${esc(T('addons.fromFile'))}</div><div class="d">${esc(T('addons.fromFileNote'))}</div></div><span class="chev">&#xE76C;</span></div>`
    // At its top (spec §1, §3): 获取更多 ›, to the market; under it, when
    // updates wait for the person, 「N 个插件有更新 · 全部更新」.
    const more = `<div class="grp"><div class="r jumps mk-entry" data-mk-open role="button" tabindex="0"><span class="mk-big">&#xE719;</span><div class="grow"><div class="t">${esc(T('mk.title'))}</div><div class="d">${esc(T('mk.entryNote'))}</div></div><span class="chev">&#xE76C;</span></div></div>`
    const ups = AddonUI.updatesBanner(st.snap.marketUpdates, T)
    const banner = ups ? `<div class="mk-banner"><span class="grow">${esc(ups.text)}</span><button class="pbtn sm al" data-mk-all${market.updating ? ' disabled' : ''}>${esc(market.updating ? T('mk.updatingAll') : ups.button)}</button></div>` : ''
    return `${more}${banner}${sec(T('w.section'))}<div class="grp"><div class="note">${esc(T('w.note'))}</div>${rows}${broken}</div>
      ${packsGroup()}
      ${sec(T('w.show'))}<div class="grp">
        ${row(esc(T('w.spin')), '', seg('widgetSpin', spins, s.widgetSpin ?? 8))}
        ${row(esc(T('w.nudge')), esc(T('w.nudgeNote')), sw('widgetNudge', s.widgetNudge !== false))}
        ${row(esc(T('mk.auto')), esc(T('mk.autoNote')), sw('pluginAutoUpdate', s.pluginAutoUpdate !== false))}
        ${fromFile}
      </div>
      ${sec(T('w.write'))}<div class="grp"><div class="note">${esc(T('w.writeNote'))}</div>
        <div class="r"><span class="cmd mono">${esc(example)}</span><button class="pbtn" data-copy="${esc(example)}">${esc(T('home.copy'))}</button></div>
      </div>`
  }

  return { pageWidgets, picksKey }
}
