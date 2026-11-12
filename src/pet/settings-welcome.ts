// The welcome at the top of General: the agents found on this computer, how
// each is connected, connecting them all at once; the displays' rows, the
// shortcuts that failed; and which page draws which tab (PAGES).
// Made by settings.ts in the section's place as the settings are made
// (welcomePart(core), imported by settings.ts). The settings' core (SettingsCore):
// their constants and functions as they are, what they remember in core.st
// (settings-vars.ts, read and written as st.<name>), and the parts made
// before this one in core.pieces (a later one's reached once the settings
// are up). What it hands back is the settings' again.
import * as Agents from './agents.ts'
import type { SettingsCore } from './settings.ts'

export function welcomePart(core: SettingsCore) {
  const {
    st, pieces, Agents, COLOR, Island, T, esc, nameList, pageAlerts, pageNow, part, row, sec, seg, sw, thumb,
  } = core
  const { pageLook, winMode } = pieces.look
  const { pageWidgets } = pieces.plugins
  const { pageMail, pageNet } = pieces.letter
  const { aboutHTML, failed, known, pageAgents, pageCrash, pageUpdate } = pieces.agents

  const anyIn = () => Agents.TABLE.some(a => Agents.isIn(st.snap, a.id))
  const welcomeFolded = () => st.snap.settings.connectHintOff === true && !st.welcomeOpen && !anyIn()
  const showWelcome = () => !anyIn() || !st.snap.settings.onboarded
  // The card itself, not General's line: shown, and none connected for real yet.
  const cardShows = () => showWelcome() && !toolsHere().some(tool => toolState(tool) === 'seen')

  // The ones on this computer, in the table's order (agents.ts here), but
  // one of a version she cannot connect to (Reasonix 0.x: its row on the
  // Agent page says why). None found: the first four (it cannot be told).
  function toolsHere() {
    const here = Agents.here(st.snap).filter(id => Agents.stateOf(st.snap, id) !== 'unsupported')
    return here.length ? here : ['claude', 'codex', 'agy', 'opencode']
  }

  // Their names as one says them: 「A、B 或 C」, "A, B and C". (The tools
  // named here are ids of agents.ts's table: byId and read find each.)
  const toolNames = (tools: string[], kind = 'or') => nameList(st.lang, tools.map(tool => Agents.byId(tool)!.name), kind)

  // Codex trusts her hooks as they are, by its own settings (zhoupengjie/Wakuwaku#66).
  const codexTrusted = () => Agents.read(st.snap, 'codex')?.trusted === true

  // How one stands on the card: busy, seen (a session's event came, or the
  // plugin in), set (written, no session yet: Codex's to be trusted first,
  // unless they are; ours there to repair counts as written), failed, none.
  function toolState(tool: string) {
    const state = Agents.stateOf(st.snap, tool, known())
    if (state === 'on') return 'seen'
    if (state === 'repair' && Agents.isIn(st.snap, tool)) return Agents.read(st.snap, tool)!.seen ? 'seen' : 'set'
    if (state === 'step') return 'set'
    return ['busy', 'set', 'failed'].includes(state) ? state : 'none'
  }

  // Windows' accent for the accent-filled buttons: its light shade on the
  // dark settings, its dark one on the light (the taskbar's look, else
  // Windows' defaults).
  function accentButton() {
    const pair: { light?: string; dark?: string } = Island?.accent?.() || {}
    return winMode() === 'light' ? `--acc:${pair.dark || '#0067c0'};--acc-ink:#fff` : `--acc:${pair.light || '#4cc2ff'};--acc-ink:#000`
  }

  // One's button: its name, and under it how it stands; set up and
  // connected in green, a step left (Codex) in amber, and those do nothing.
  function toolButton(tool: string) {
    const state = toolState(tool)
    const name = `<span class="wt-name">${esc(Agents.byId(tool)!.name)}</span>`
    const line = (text: string) => `<span class="wt-state">${text}</span>`
    if (state === 'busy') return `<button class="wc-tool" disabled>${name}${line(`<span class="spin"></span>${esc(T('wc.busy'))}`)}</button>`
    if (state === 'seen') return `<button class="wc-tool done" disabled>${name}${line(esc(T('wc.toolOn')))}</button>`
    if (state === 'set' && tool === 'codex' && !codexTrusted()) return `<button class="wc-tool warn" disabled>${name}${line(esc(T('wc.toolStep')))}</button>`
    if (state === 'set') return `<button class="wc-tool done" disabled>${name}${line(esc(T('wc.toolSet')))}</button>`
    return `<button class="wc-tool" data-agent="${tool}:install">${name}${line(esc(T(state === 'failed' ? 'wc.toolRetry' : 'wc.toolGo')))}</button>`
  }

  // Why one could not be written (the card's and the Agent page's): which
  // file, the whole path, the reason, what to do.
  function failedHTML(tool: string) {
    const { error, kind, folder } = failed[tool]
    const name = Agents.byId(tool)!.name
    // Her folder's path has a space, which agy can't run: not a file not
    // written, but where she is (the appearance session's words).
    if (kind === 'path') return `<div class="wc-err"><div><b>${esc(T('wc.errConnect', { name }))}</b></div><div>${esc(T('wc.way.path', { folder: folder || '' }))}</div></div>`
    // DSH's hook bridge, its packages not installed (its profiles as they
    // were): why, what pnpm said, and npmmirror to pick when the registry
    // could not be reached.
    if (tool === 'dsh' && kind.startsWith('bridge')) {
      const said = String(error || '').replace(/^.*?\]:\s*/, '')
      const mirror = kind === 'bridgeRegistry' ? `<div class="ag-btns"><button class="pbtn sm" data-dsh-mirror ${st.connecting ? 'disabled' : ''}>${esc(T('ag.dsh.mirror'))}</button></div>` : ''
      return `<div class="wc-err"><div><b>${esc(T('s.dshBridgeFailed'))}</b></div><div>${esc(T(`s.dshBK.${kind}`, { version: st.snap.agents?.dsh?.dshVersion || '' }))}</div>${said ? `<div class="mono">${esc(said)}</div>` : ''}${mirror}</div>`
    }
    const way = `${T(`wc.way.${kind}`)}${T(tool === 'claude' ? 'wc.wayPlugin' : 'wc.wayEnd')}`
    const file = Agents.read(st.snap, tool)!.shown
    return `<div class="wc-err"><div><b>${esc(T('wc.errLead', { name, file: file.split(/[\\/]/).pop() }))}</b></div>
        <div class="mono">${esc(file)}</div><div>${esc(error)}</div><div>${esc(way)}</div></div>`
  }

  // Under the buttons: what each still needs (where it was written, and
  // undone), or why it could not be written.
  function toolNote(tool: string) {
    const state = toolState(tool)
    const mono = (text: string) => `<span class="mono">${esc(text)}</span>`
    const { shown: file, backup } = Agents.read(st.snap, tool)!
    const undo = `<span class="link" data-agent="${tool}:remove">${esc(T('wc.undo'))}</span>`
    if (['claude', 'agy', 'zcode', 'dsh', 'reasonix'].includes(tool) && state === 'set') {
      const where = backup ? T('wc.written', { path: mono(file), backup: mono(`${file.split(/[\\/]/).pop()}.wakuwaku.bak`) }) : T('wc.created', { path: mono(file) })
      return `<div class="wc-tnote"><div>${T(`wc.${tool}Left`)}</div><div class="wc-where">${where}${undo}</div></div>`
    }
    if (tool === 'codex' && state === 'set' && !codexTrusted()) return `<div class="wc-tnote warn">${T('wc.codexLeft')}</div>`
    if (tool === 'opencode' && state === 'set') return `<div class="wc-tnote"><div>${T('wc.opencodeLeft')}</div><div class="wc-where">${T('wc.created', { path: mono(file) })}${undo}</div></div>`
    // omp: a file of our own in its agent folder, nothing of omp's changed.
    if (tool === 'omp' && state === 'set') return `<div class="wc-tnote"><div>${T('wc.ompLeft')}</div><div class="wc-where">${T('wc.ompWritten', { path: mono(file) })}${undo}</div></div>`
    return state === 'failed' ? failedHTML(tool) : ''
  }

  function welcomeBlock() {
    const tools = toolsHere()
    if (welcomeFolded()) {
      return `<div class="grp"><div class="r"><span class="dot" style="background:${COLOR.idle}"></span><div class="grow d">${esc(T('wc.notYet', { list: toolNames(tools) }))}</div><span class="link" data-welcome-open>${esc(T('hint.go'))} ›</span></div></div>`
    }
    const many = tools.length > 1
    const chosen = (st.snap.pets || []).find(p => p.id === st.snap.settings.pet)
    const face = chosen ? `<span class="wc-face">${thumb(40, chosen.url, chosen.version)}</span>` : ''
    // A nudge to approve only where one of them can ask (agents.ts approve).
    const sub = `${T('wc.sub1')}${tools.some(tool => Agents.byId(tool)!.caps.approve[0] !== 'no') ? T('wc.subAsk') : ''}${T('wc.sub2')}${many ? T('wc.subMany') : ''}`
    // OpenCode alone: a file of ours, nothing backed up, and she is not woken
    // by it; omp alone: a file of its own is written, nothing to back up.
    const safeKeys = many ? ['wc.safeMany', 'wc.safe2', 'wc.safe3'] : tools[0] === 'opencode' ? ['wc.safe.opencode', 'wc.safeOc2', 'wc.safeOc3'] : tools[0] === 'omp' ? ['wc.safe.omp', 'wc.safe3'] : [`wc.safe.${tools[0]}`, 'wc.safe2', 'wc.safe3']
    const safe = safeKeys.map(k => `<div><span class="fi">\uE73E</span><span>${T(k)}</span></div>`).join('')
    // Claude Code's plugin, while it is here and not written.
    const commands = st.snap.agents?.claude?.pluginCommands || []
    const plugBody = st.pluginOpen
      ? `<div class="wc-plugbody"><div class="d">${esc(T('wc.plugHow'))}</div>${commands.map((c: string) => `<div class="wc-cmd mono">${esc(c)}</div>`).join('')}
          <div class="wc-copy"><button class="pbtn" data-copy="${esc(commands.join('\n'))}" data-label="${esc(T('wc.copyBoth'))}">${esc(T('wc.copyBoth'))}</button><span class="d">${esc(T('wc.plugWake'))}</span></div></div>`
      : ''
    const plugin =
      tools.includes('claude') && ['none', 'failed', 'busy'].includes(toolState('claude'))
        ? `<div class="wc-div"></div>
        <button class="wc-plug${st.pluginOpen ? ' open' : ''}" data-plugin-toggle><span class="fi chev">\uE76C</span><span>${esc(T('wc.plug'))}</span><span class="d">${esc(T('wc.plugNote'))}</span></button>${plugBody}`
        : ''
    // Nothing connected: "not now" at its foot (as the island's 不用提醒);
    // the Agent page for more.
    const later = !anyIn() ? `<button class="wc-later" data-welcome-later>${esc(T('wc.later'))}</button>` : ''
    return `<div class="wcard" style="${accentButton()}">
        <div class="wc-head">${face}<div class="grow"><div class="wc-title">${esc(T('wc.title', { list: toolNames(tools) }))}</div><div class="wc-sub">${esc(sub)}</div></div></div>
        <div class="wc-lead">${esc(T(many ? 'wc.leadMany' : 'wc.leadOne'))}</div>
        <div class="wc-tools" style="grid-template-columns:repeat(${tools.length},minmax(0,1fr))">${tools.map(toolButton).join('')}</div>${tools.map(toolNote).join('')}
        <div class="wc-safe">${safe}</div>${plugin}<div class="wc-foot"><span class="link" data-tab="agents">${esc(T('wc.more'))} ›</span><span class="grow"></span>${later}</div>
      </div>`
  }

  // General's line on the agents once one is connected: how many, their
  // names, how many to repair or still to connect; a click opens the Agent
  // page. Its dot: amber while one wants a repair, else green (one seen) or
  // blue (written, waiting).
  function agentsLine() {
    // (summary fills connected in.)
    const s = Agents.summary(st.snap, known())
    const n = s.connected!.length
    const what = [n ? toolNames(s.connected!, 'and') : '', s.wants.length ? T('ag.lineFix', { n: s.wants.length }) : '', s.none.length ? T('ag.lineMore', { n: s.none.length }) : '']
    const mood = s.wants.length ? 'waiting' : s.on.length ? 'done' : 'working'
    const title = n ? T(n === 1 ? 'ag.line1' : 'ag.line', { n }) : T('ag.lineRepair')
    return `<div class="grp"><div class="r go" data-tab="agents"><span class="dot" style="background:${COLOR[mood]}"></span><div class="grow"><div>${esc(title)}</div><div class="d ellip">${esc(what.filter(Boolean).join(' · '))}</div></div><span class="link">${esc(T('wc.manage'))}</span></div></div>`
  }

  // Her taskbar on the main display alone (no taskbar on the others), or
  // on all of them (bars.rs); with one display, shown greyed (the appearance
  // session's spec, as Windows' own "show my taskbar on all displays").
  // The display her home is on (zhoupengjie/Wakuwaku#104, homeDisplay): a
  // segment a display, numbered as Windows numbers them, the one she is on
  // marked, its size and whether it is Windows' main display in the note;
  // with one display, greyed as the displays row is. Before 显示在 on the
  // taskbar and the top bar, first in the island's 位置 group. Picking one
  // keeps its key: she stays on that display whichever is made the main one.
  function screenRow() {
    const displays = st.snap.displays || []
    const one = displays.length < 2
    const hers = displays.find(d => d.hers) || displays.find(d => d.main) || displays[0]
    const note = one ? T('s.displaysOne') : hers ? T(hers.main ? 's.screenNoteMain' : 's.screenNote', { n: hers.n, w: hers.w, h: hers.h }) : ''
    const control = one
      ? `<span class="seg off">${displays.map(d => `<span class="${d === hers ? 'on' : ''}">${esc(String(d.n))}</span>`).join('')}</span>`
      : seg('homeDisplay', displays.map(d => [d.key, String(d.n)]), hers?.key)
    return row(esc(T('s.screen')), esc(note), control)
  }

  function displaysRow() {
    const value = st.snap.settings.taskbarDisplays === 'all' ? 'all' : 'main'
    const one = (st.snap.displays || []).length < 2
    const note = one ? T('s.displaysOne') : T(value === 'all' ? 's.displaysAllNote' : 's.displaysMainNote')
    const control = one
      ? `<span class="seg off">${['main', 'all'].map(v => `<span class="${v === value ? 'on' : ''}">${esc(T(`s.displays.${v}`))}</span>`).join('')}</span>`
      : seg('taskbarDisplays', ['main', 'all'].map(v => [v, T(`s.displays.${v}`)]), value)
    return row(esc(T('s.taskbarDisplays')), esc(note), control)
  }

  // General: the welcome (nothing connected yet) or the line on the agents
  // connected, the sessions and the quick switches, the alerts, startup and
  // the language, her updates, her crash reports and what this is.
  // Connecting them is the Agent page's.
  function shortcutRow(key: string, title: string, note: string, value: string | undefined) {
    const isRecording = st.recordingShortcut === key
    const displayVal = isRecording ? esc(T('s.shortcutRecording')) : (value ? esc(value) : `<span class="empty">${esc(T('s.shortcutNone'))}</span>`)
    const clearBtn = !isRecording && value ? `<button class="kclear" data-key-clear="${key}" title="${esc(T('s.shortcutNone'))}">✕</button>` : ''
    const isFailed = st.failedShortcuts ? st.failedShortcuts.has(key) : (st.snap.shortcutsFailed || []).includes(key)
    const errTip = isFailed ? `<div class="kerr">${esc(T('s.shortcutOccupied'))}</div>` : ''
    const control = `<div class="kctl"><button class="kbadge${isRecording ? ' rec' : ''}" data-key-bind="${key}">${displayVal}</button>${clearBtn}</div>`
    return row(esc(title), `${note ? esc(note) : ''}${errTip}`, control)
  }

  function pageShortcuts() {
    const s = st.snap.settings
    const on = s.shortcutsEnabled === true
    return `${sec(T('s.shortcuts'))}<div class="grp">
      ${row(esc(T('s.shortcutsEnable')), esc(T('s.shortcutsEnableNote')), sw('shortcutsEnabled', on))}
      ${on ? `
        ${shortcutRow('shortcutApprove', T('s.shortcutApprove'), T('s.shortcutApproveNote'), s.shortcutApprove ?? 'Alt+Y')}
        ${shortcutRow('shortcutDeny', T('s.shortcutDeny'), T('s.shortcutDenyNote'), s.shortcutDeny ?? 'Alt+N')}
        ${shortcutRow('shortcutIsland', T('s.shortcutIsland'), T('s.shortcutIslandNote'), s.shortcutIsland ?? 'Alt+W')}
      ` : ''}
    </div>`
  }

  function pageGeneral() {
    return `${cardShows() ? part('welcome', welcomeBlock()) : agentsLine()}${pageNow()}
      ${part('shortcuts', pageShortcuts())}
      ${part('alerts', pageAlerts())}
      ${sec(T('s.startLang'))}<div class="grp">
        ${row(esc(T('settings.startAtLogin')), esc(T('home.pluginStart')), sw('login', st.snap.loginAtStart))}
        ${row(esc(T('home.language')), '', seg('lang', [['auto', T('settings.languageAuto')], ['zh', '中文'], ['en', 'EN']], st.snap.settings.lang))}
      </div>
      ${part('update', pageUpdate())}
      ${part('net', pageNet())}
      ${part('crash', pageCrash())}
      ${aboutHTML()}`
  }

  const PAGES: Record<string, () => string> = { now: pageGeneral, agents: pageAgents, look: pageLook, widgets: pageWidgets, mail: pageMail }

  return { PAGES, cardShows, displaysRow, failedHTML, screenRow, toolNames, toolsHere }
}
