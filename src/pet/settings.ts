// The settings, grown out of the island: one click on it opens them (and only
// then does the island take the keyboard); Esc, a click outside, ✕ or the head
// close them. Five pages:
//   now       General: the welcome (nothing connected yet) or a line on the
//             agents connected, the sessions (a click on one goes to its
//             window), four quick switches; the alerts (how long endings
//             stay, idle sessions, notifications, sound, prompts, quiet),
//             start at login and the language, her updates, crash reports, about
//   agents    the agents she follows (agents.ts): each found here a row (how
//             it stands, what she can do for it, connect, repair, remove),
//             opened in place to its details; the others to get
//   look      her home (island, taskbar), size, strolls, eyes; Windows' own
//             mode and accent colour; her and the pets (downloaded, the gallery)
//   widgets   plugins: the market (获取更多 ›: a list, each one's page, installed from there, updates), the built-in ones, those she runs for you (how each is and where it is put, no switch: placed is on; their settings folded under), other people's (the same, and each its own page with what it may do and 卸载), turns, nudges, updates by themselves, installing one from a file, waku, and how to write one
//   mail      mail accounts, and setting one up as Thunderbird does: an address and a password, the server found or typed
//
// What they show comes from main as a snapshot, again whenever it changes;
// what they change goes back as a patch, checked there. island.ts decides the
// island's shape (it is 'settings' while these are open) and places her.
//
// Everything that came from elsewhere (session names, pet names, errors) is
// escaped before it goes into the page.
import * as AddonUI from './addonui.ts'
import * as Agents from './agents.ts'
import * as Flow from './flow.ts'
import * as Status from './status.ts'
import * as Widgets from './widgets.ts'
import { t, list as nameList, render } from '../shared/i18n.ts'
import { CLIPS } from './sprite.ts'
import { pet } from './bridge.ts'
import { lookPart } from './settings-look.ts'
import { addonsPart } from './settings-addons.ts'
import { marketPart } from './settings-market.ts'
import { pluginsPart } from './settings-plugins.ts'
import { mailPart } from './settings-mail.ts'
import { letterPart } from './settings-letter.ts'
import { agentsPart } from './settings-agents.ts'
import { welcomePart } from './settings-welcome.ts'
import { settingsVars } from './settings-vars.ts'
import * as State from './settings-state.ts'
import { TABS, PART_OF } from './settings-state.ts'
import type { Addon, DraftBox, MailBox, MailForm, MailPick, Plugin, Settings, SettingsVars, Thread, WinMode } from './settings-vars.ts'
import type { Choices } from './settings-mail.ts'
import type { Widget } from './widgets.ts'
import type { SettingsEvent } from './settings-state.ts'
import type { IslandFace } from './island.ts'
import type { PickerFace } from './picker.ts'
import type { Way } from './settings-letter.ts'
import type { Config } from '../shared/config.ts'

// Where one is put (island-wmenu.ts placeNow): its place (null: nowhere),
// a board's name.
export interface Place {
  at: string | null
  name?: string
}

// A row of the Plugins page (widgetRows): its key, its place in the order,
// and what it is: a widget, or a plugin she runs or someone else's, with
// its widgets up now (mine).
export type WidgetRow = { key: string; i: number } & (
  | { widget: Widget; plugin?: undefined; addon?: undefined; mine?: undefined }
  | { plugin: Plugin; mine: Widget[]; widget?: undefined; addon?: undefined }
  | { addon: Addon; mine: Widget[]; widget?: undefined; plugin?: undefined }
)

// The Mail page's inbox (settingsPage inbox): its conversations
// (threads.rs) and what went wrong (mail.rs) are the Rust side's JSON.
export interface Inbox {
  id: string
  for: string
  filter: string
  threads: Thread[]
  total: number
  letters: number
  count: number
  busy: boolean
  error: any
  errors: any[]
  rev: number
  syncing: boolean
}

// A download's last word (market.rs, market:progress).
export interface MarketJob {
  id?: string
  stage: string
  got?: number
  total?: number
}

// One of the market's index (market.rs): the Rust side's JSON, its id
// read by name.
export interface MarketItem {
  id: string
  [field: string]: any
}

// The plugin market (settingsPage market): what went wrong reading its
// index is the Rust side's JSON.
export interface Market {
  items: MarketItem[] | null
  at: number
  error: any
  loading: boolean
  cat: string
  query: string
  jobs: Record<string, MarketJob>
  updating: boolean
}

// One of a plugin's parameters (plugins.rs view): text, number, flag or
// choice (its options), as set.
interface PluginParam {
  name: string
  kind?: string
  value?: unknown
  required?: boolean
  options?: string[]
}

// A clock drawn for a session (clockTag, tick): how it is, since when its
// turn runs, how long it took.
interface Clocked {
  mood: string
  since?: number | null
  took?: number | null
}

// The page's markup kept on what setHTML draws into, to change in place.
type Morphed = HTMLElement & { _html?: string }

// The element a click is on, or the nearest round it, by a selector (the
// click handler's at); a button's or another kind's by its type.
type At = <E extends Element = HTMLElement>(selector: string) => E | null

// Her three shortcuts' settings.
type ShortcutKey = 'shortcutApprove' | 'shortcutDeny' | 'shortcutIsland'

// A pet of codex-pets.net's gallery, as settings.rs lists them.
interface GalleryPet {
  id: string
  name: string
  author: string
  likes: number
  preview?: string
}

// The settings' parts as each is made (settingsPage, in its section's
// place): what each hands back, reached by a part made before it
// (pieces.<name>), or by one made before a later one, once the settings
// are up.
export interface SettingsParts {
  look: ReturnType<typeof lookPart>
  addons: ReturnType<typeof addonsPart>
  market: ReturnType<typeof marketPart>
  plugins: ReturnType<typeof pluginsPart>
  mail: ReturnType<typeof mailPart>
  letter: ReturnType<typeof letterPart>
  agents: ReturnType<typeof agentsPart>
  welcome: ReturnType<typeof welcomePart>
}

// What every part (settings-*.ts) is given as it is made (core, in
// settingsPage): what the settings remember (st), the parts made so far
// (pieces), and the settings' own constants and functions a part uses.
// Spelled out here, as settingsPage's own are out of reach of a type; tsc
// checks the core against it, and each part's use of it.
export interface SettingsCore {
  st: SettingsVars
  pieces: SettingsParts
  go: (e: SettingsEvent) => void
  AGENTS: Record<string, string>
  AGENT_NAMES: Record<string, string>
  AddonUI: typeof AddonUI
  Agents: typeof Agents
  COLOR: Record<string, string>
  Island: IslandFace | null
  MOST_FILES: number
  SIZES: [name: string, scale: number][]
  SPINS: number[]
  T: (key: string, vars?: Record<string, unknown>) => string
  Widgets: typeof Widgets
  body: HTMLElement
  drafts: Record<string, string>
  draw: (reset?: boolean) => void
  esc: (value: unknown) => string
  expanded: Set<string>
  freshRows: Set<string>
  inbox: Inbox
  keepDraft: () => void
  layer: HTMLElement
  loadGallery: (more: boolean) => Promise<void>
  market: Market
  nameList: typeof nameList
  onTaskbar: (s: Config) => boolean
  openHardware: Set<string>
  pageAlerts: () => string
  pageNow: () => string
  pagePets: () => string
  part: (id: string, html: string) => string
  patch: (change: Settings) => void
  placeHTML: (place: Place, key?: string) => string
  placeOf: (item: { plugin?: string; id?: string }, on: boolean | undefined) => Place
  pluginFields: (p: Plugin) => string
  pluginNote: (p: Plugin, mine: Widget[], placed: boolean) => { text: string; bad?: boolean }
  redrawMail: (focus?: string | null) => void
  relayout: () => number
  render: typeof render
  row: (title: string, note: string, control: string) => string
  sec: (text: string) => string
  seg: (key: string, options: Choices, current: unknown) => string
  setHTML: (el: Morphed, html: string) => boolean
  sheet: HTMLElement
  showPart: () => void
  sw: (key: string, on: unknown, extra?: string) => string
  thumb: (px: number, url: string, version: unknown, clip?: string) => string
  toMarketRow: (kind: string, title: string, note: string) => string
  toastEl: HTMLElement
  widgetRows: () => WidgetRow[]
}

export type SettingsFace = ReturnType<typeof settingsPage>

// The settings, on the island's page: made as it starts (main.ts), after the
// island and the picker (none unless the island is on); what the others see
// of them.
export function settingsPage(Island: IslandFace | null, Picker: PickerFace | null) {
  // What the settings remember, all of it (settings-vars.ts): st.<name>
  // here and in every part.
  const st = settingsVars(AddonUI.IDLE)
  // Where they are (st.isOpen, st.tab, st.sub...: settings-state.ts) moves
  // on here alone, by what happened.
  function go(e: SettingsEvent) {
    Object.assign(st, State.next(State.stateOf(st), e))
  }
  // The island's colours (on black), the same as island.ts.
  const COLOR: Record<string, string> = { idle: '#8e8e93', working: '#5e9bff', waiting: '#ffb340', done: '#34d27b', review: '#b18cff', error: '#ff5c6c' }
  // The five pages' names (TABS, PART_OF: settings-state.ts).
  const TAB_KEY: Record<string, string> = { now: 's.tabGeneral', agents: 's.tabAgents', look: 's.tabLook', widgets: 's.tabWidgets', mail: 's.tabMail' }
  const HEAD_KEY: Record<string, string> = { working: 's.headWorking', waiting: 's.headWaiting', done: 's.headDone', review: 's.headReview', error: 's.headError' }
  const SIZES: [string, number][] = [['small', 0.4], ['medium', 0.55], ['large', 0.75]]
  const HOMES = ['island', 'taskbar', 'split']
  const HOME_KEY: Record<string, string> = { island: 's.displayIsland', taskbar: 's.displayTaskbar', split: 's.displaySplit' }
  // Her taskbar the home: along an edge, or split into the top bar and the dock.
  const onTaskbar = (s: Config) => s.display === 'taskbar' || s.display === 'split'
  // How long what she tells stays (停留多久): the quick choices, until seen,
  // and Custom, whole seconds typed in a box beside them (flow.ts holdSecs,
  // zhoupengjie/Wakuwaku#59).
  const HOLDS: (number | 'seen')[] = [...Flow.HOLD_QUICK, 'seen']
  // Minutes a session at rest stays in sight; 0 always (state.rs is_shown).
  const IDLE_HIDES = [15, 30, 60, 120, 0]
  const WAITS = [30, 60, 120, 290]
  const SPINS = [0, 5, 8, 15]
  const ICON: Record<string, string> = {
    eye: '<path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>',
    moon: '<path d="M20 14A8 8 0 1 1 10 4a7 7 0 0 0 10 10z"/>',
    sound: '<path d="M4 9v6h4l5 4V5L8 9H4z"/><path d="M16 9a4 4 0 0 1 0 6"/>',
    pill: '<rect x="3" y="8" width="18" height="8" rx="4"/>',
  }

  const layer = document.getElementById('island-settings')!
  // On a Mac, a blur this soon after they opened (st.openedAt) is the
  // keyboard taken, not a click outside.
  const BLUR_GRACE_MS = 500
  // The gallery, loaded the first time the pets page is shown; or what a
  // search finds there, the words typed in the download box (a link there
  // is downloaded, any other words are looked for as the site's own search
  // box looks: a name, part of one, a word of its description). Each ask
  // numbered: a slower answer to an earlier search never lands over a later.
  const gallery = { sort: 'popular', query: '', items: [] as GalleryPet[], page: 0, totalPages: 1, total: 0, loading: false, error: '', getting: '', asked: 0 }
  const isLink = (text: string) => /:\/\/|codex-pets\.net/i.test(text)
  // The plugins page: what is typed in a plugin's settings folded out
  // (st.openPlugin) and not yet kept ("weather.City": "上海").
  const drafts: Record<string, string> = {}
  // Other people's plugins installed while the settings are open (their
  // rows edged in the accent until they close; a pack's by AddonUI.packKey).
  const freshRows = new Set<string>()
  // The plugin market (market.rs; addonui.ts its words and steps), a part
  // of the Plugins page (sub market, an item's page mkItem): its index as
  // last read (items null until one was), when, what went wrong reading it,
  // being read; the category and the words looked for; each download's last
  // word (market:progress) by id; 全部更新 under way. (What is asked over
  // the panel is st.mkAsk.)
  const market: Market = { items: null, at: 0, error: null, loading: false, cat: 'all', query: '', jobs: {}, updating: false }
  // Recording one: her global shortcuts let go meanwhile, so the keys
  // pressed reach this page (shortcuts.rs pause).
  const setRecording = (key: string | null) => {
    if (key === st.recordingShortcut) return
    st.recordingShortcut = key
    pet.shortcutsPause?.(key !== null)
  }
  pet.onShortcutStatus && pet.onShortcutStatus(({ failed }: { failed?: string[] } = {}) => {
    st.failedShortcuts = new Set(failed || [])
    if (st.snap) st.snap.shortcutsFailed = failed || []
    draw()
  })
  // The monitor's hardware (LibreHardwareMonitor's) folded out, by its id.
  const openHardware = new Set<string>()
  // The Mail page's inbox: whose (an account id), its newest letters and how
  // many there are, how many asked for, busy, what went wrong (the letter
  // open is st.reading).
  // threads: its conversations as kept (threads.rs), each with its letters;
  // total: how many there are; letters: how many letters the inbox holds;
  // rev: the store's revision they are of (a newer one, asked again);
  // syncing: a first sync still under way. The conversations folded out.
  const inbox: Inbox = { id: '', for: '', filter: 'all', threads: [], total: 0, letters: 0, count: 50, busy: false, error: null, errors: [], rev: 0, syncing: false }
  const expanded = new Set<string>()
  // The agents that read mail (agents.ts mail): their short names, and
  // their names on the Mail page's settings.
  const AGENTS = Object.fromEntries(Agents.TABLE.filter(a => a.mail).map(a => [a.id, a.mail!]))
  const AGENT_NAMES = Object.fromEntries(Agents.TABLE.filter(a => a.mail).map(a => [a.id, a.name]))
  // The most a letter takes with it (as send.rs has it).
  const MOST_FILES = 25 << 20

  // The letter left being written, as kept (none begun meanwhile).
  pet.mail.draft().then(d => {
    if (st.draft || !d || typeof d !== 'object' || !d.account) return
    st.draft = { ...d, files: [], busy: false, error: null, note: null }
    if (st.isOpen) draw()
  })

  function keepDraft() {
    clearTimeout(st.draftTimer)
    st.draftTimer = setTimeout(() => pet.mail.keepDraft(st.draft ? { ...st.draft, files: [], busy: false, error: null, note: null } : null), 300)
  }

  const T = (key: string, vars?: Record<string, unknown>) => t(st.lang, key, vars)
  const esc = (value: unknown) =>
    String(value ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' } as Record<string, string>)[c])
  const svg = (name: string) => `<svg class="ic" viewBox="0 0 24 24">${ICON[name]}</svg>`

  layer.innerHTML = `
    <div class="s-head" data-close></div>
    <div class="s-tabs"><span class="ind"></span>${TABS.map(k => `<button data-tab="${k}"><span></span><i class="n" hidden></i></button>`).join('')}</div>
    <div class="s-ask"></div>
    <div class="s-body"></div>
    <div class="s-foot"><span class="foot-text"></span><span class="grow"></span><span class="ver"></span></div>
    <div class="s-sheet" hidden></div>
    <div class="s-toast" role="status" hidden></div>`
  // (All just made above.)
  const head = layer.querySelector('.s-head')!
  const tabs = layer.querySelector('.s-tabs')!
  tabs.style.setProperty('--tabs', TABS.length as any)
  const askSlot = layer.querySelector('.s-ask')!
  const body = layer.querySelector('.s-body')!
  const foot = layer.querySelector('.s-foot')!
  // A question over the whole panel (installing a plugin, taking one away),
  // in place of a dialog of Windows'; a word for a few seconds at its foot.
  const sheet = layer.querySelector('.s-sheet')!
  const toastEl = layer.querySelector('.s-toast')!

  // The new markup, changed in place: only what differs is touched, so a
  // number that changed (a plugin's reading) does not redraw the page around
  // it, and a box being typed in keeps its focus and caret.
  function setHTML(el: Morphed, html: string) {
    if (el._html === html) return false
    el._html = html
    const next = document.createElement('template')
    next.innerHTML = html
    morphChildren(el, next.content)
    return true
  }

  // An element, or a text node or comment, which are read only for their
  // kind, name and text (the attributes and value: an element's).
  function morph(from: Element, to: Element) {
    if (from.nodeType !== to.nodeType || from.nodeName !== to.nodeName) return from.replaceWith(to.cloneNode(true))
    // A letter's frame with the same letter in it (settings-letter.ts
    // mailBodyHTML, its data-doc) is left as it is: its height and scrolling
    // are fitFrames', which the markup has not, and would be taken away at
    // every redraw of the page and set again. Another letter in it, its
    // page stays and fitFrames puts the letter in.
    if (from.nodeName === 'IFRAME' && from.getAttribute('data-doc') && from.getAttribute('data-doc') === to.getAttribute('data-doc')) return
    if (from.nodeType !== Node.ELEMENT_NODE) {
      if (from.nodeValue !== to.nodeValue) from.nodeValue = to.nodeValue
      return
    }
    for (const { name } of [...from.attributes]) if (!to.hasAttribute(name)) from.removeAttribute(name)
    for (const { name, value } of to.attributes) if (from.getAttribute(name) !== value) from.setAttribute(name, value)
    // A box's text, once typed in, is its own: set while it is not being typed in.
    if (from.nodeName === 'INPUT' && from !== document.activeElement && (from as HTMLInputElement).value !== (to.getAttribute('value') ?? '')) (from as HTMLInputElement).value = to.getAttribute('value') ?? ''
    morphChildren(from, to)
    if (from.nodeName === 'TEXTAREA' && from !== document.activeElement && (from as HTMLTextAreaElement).value !== to.textContent) (from as HTMLTextAreaElement).value = to.textContent
  }

  function morphChildren(from: Node, to: Node) {
    const have = [...from.childNodes]
    const want = [...to.childNodes]
    want.forEach((node, i) => (i < have.length ? morph(have[i] as Element, node as Element) : from.appendChild(node.cloneNode(true))))
    for (const extra of have.slice(want.length)) extra.remove()
  }

  function clock(ms: number) {
    const s = Math.max(0, Math.floor(ms / 1000))
    const h = Math.floor(s / 3600)
    const m = Math.floor((s % 3600) / 60)
    const sec = String(s % 60).padStart(2, '0')
    return h ? `${h}:${String(m).padStart(2, '0')}:${sec}` : `${m}:${sec}`
  }

  // How long a turn has run, or how long the finished one took.
  function timeOf(s: Clocked) {
    if ((s.mood === 'working' || s.mood === 'waiting') && s.since) return clock(Date.now() - s.since)
    if ((s.mood === 'done' || s.mood === 'review' || s.mood === 'error') && s.took) return clock(s.took)
    return ''
  }

  // A clock as markup that tick() keeps current, so a redraw is not needed for it.
  const clockTag = (s: Clocked, cls = 'clock') =>
    `<span class="${cls}" data-mood="${esc(s.mood)}" data-since="${s.since || ''}" data-took="${s.took || ''}" style="color:${COLOR[s.mood] || COLOR.idle}">${timeOf(s)}</span>`

  function tick() {
    for (const c of layer.querySelectorAll('[data-mood][data-since]')) {
      c.textContent = timeOf({ mood: c.dataset.mood!, since: Number(c.dataset.since) || null, took: Number(c.dataset.took) || null })
    }
  }

  // --- Small parts -------------------------------------------------------------------

  const sw = (key: string, on: unknown, extra = '') => `<button class="sw${on ? ' on' : ''}" data-sw="${key}" ${extra}></button>`
  // A few choices side by side; one marked off ([value, label, true]) is
  // there, greyed, and can't be picked.
  const seg = (key: string, options: Choices, current: unknown) =>
    `<span class="seg" data-seg="${key}">${options
      .map(([value, label, off]) => `<span data-value="${esc(value)}" class="${String(value) === String(current) ? 'on' : off ? 'off' : ''}"${off ? ' aria-disabled="true"' : ''}>${esc(label)}</span>`)
      .join('')}</span>`
  const row = (title: string, note: string, control: string) =>
    `<div class="r"><div class="grow"><div>${title}</div>${note ? `<div class="d">${note}</div>` : ''}</div>${control}</div>`
  const sec = (text: string) => `<div class="sec">${esc(text)}</div>`
  // Where a part of a page begins, to scroll to (PART_OF).
  const part = (id: string, html: string) => `<i class="part" data-part="${id}"></i>${html}`

  function thumb(px: number, url: string, version: unknown, clip = 'idle') {
    const s = px / 192
    const sheetH = version === 1 ? 1872 : 2288
    return `<div class="thumb" data-clip="${clip}" style="width:${px}px;height:${Math.round(208 * s)}px"><div class="sheet" style="background-image:url(&quot;${esc(url)}&quot;);background-size:1536px ${sheetH}px;transform:scale(${s})"></div></div>`
  }

  function startThumbs() {
    for (const timer of st.thumbTimers) clearTimeout(timer)
    st.thumbTimers = []
    for (const el of layer.querySelectorAll('.thumb')) {
      const { row: r, frames, ms } = CLIPS[el.dataset.clip as keyof typeof CLIPS] || CLIPS.idle
      const sheet = el.querySelector('.sheet')!
      let frame = 0
      const step = () => {
        sheet.style.backgroundPosition = `${-frame * 192}px ${-r * 208}px`
        frame = (frame + 1) % frames
        const timer = setTimeout(step, ms)
        st.thumbTimers.push(timer)
      }
      step()
    }
  }

  // --- The pages -------------------------------------------------------------------------

  function sessionDetails(det: NonNullable<ReturnType<typeof Status.detailsOf>>) {
    const kv = (key: string, val: string) => `<div class="kv"><span class="k">${esc(T(key))}</span><span class="v">${val}</span></div>`
    const rows: string[] = []
    if (det.files.length) {
      const list = det.files.map(f => `<span class="s-file mono path">${esc(f)}</span>`).join('')
      const more = det.moreFiles > 0 ? `<span class="d s-more">${esc(T('s.filesMore', { n: det.moreFiles, total: det.totalFiles }))}</span>` : ''
      rows.push(kv('s.changedFiles', `<div class="s-files">${list}${more}</div>`))
    }
    if (det.steps.length) {
      const list = det.steps
        .map(step => `<div class="s-step${step.ok ? '' : ' err'}"><span class="fi s-st-ic">${step.ok ? '\uE73E' : '\uE711'}</span><span class="ellip">${esc(step.text)}</span></div>`)
        .join('')
      rows.push(kv('s.recentSteps', `<div class="s-steps">${list}</div>`))
    }
    return `<div class="ag-det s-det">${rows.join('')}</div>`
  }

  function pageNow() {
    const s = st.snap.settings
    const sessions = st.snap.sessions || []
    const alive = new Set(sessions.map(x => x.id))
    for (const id of sessionOpen) if (!alive.has(id)) sessionOpen.delete(id)
    // Each session: its name, and where it is (or how its turn ended).
    const detailed = s.details !== false
    const list = sessions.length
      ? sessions
          .map(x => {
            const name = Status.nameOf(x, detailed)
            const what = [
              (x.agent && Agents.byId(x.agent)?.name) || '',
              Status.status(st.lang, x, { detailed, withClock: false }),
              detailed && x.name && x.project !== name ? x.project : '',
              detailed && Status.isEnding(x) ? x.reply : '',
            ].filter(Boolean)
            const det = Status.detailsOf(st.lang, x, { detailed })
            const open = !!det && sessionOpen.has(x.id)
            const exp = detailed
              ? det
                ? `<button type="button" class="s-exp" data-sess-open="${esc(x.id)}" aria-expanded="${open}" aria-label="${esc(T('s.changedFiles'))}"><span class="fi chev${open ? ' open' : ''}">\uE76C</span></button>`
                : '<span class="s-exp-space"></span>'
              : ''
            // One with a window to go to: a click on it goes there.
            const jump = x.jump ? ` data-jump="${esc(x.id)}"` : ''
            const tip = [...what, x.jump ? T('jump.hint') : ''].filter(Boolean).join('\n')
            return `<div class="r s-row${x.jump ? ' jumps' : ''}${open ? ' open' : ''}"${jump} title="${esc(tip)}"><span class="dot" style="background:${COLOR[x.mood] || COLOR.idle}"></span><div class="grow"><div class="ellip">${esc(name || '—')}</div><div class="d ellip">${esc(what.join(' · '))}</div></div>${clockTag(x)}${exp}</div>${open ? sessionDetails(det) : ''}`
          })
          .join('')
      : `<div class="note">${esc(T('home.noSessions', { list: toolNames(toolsHere()) }))}</div>`
    // Her out on the desktop; and her home, the next one at a press.
    const tiles: [key: string, icon: string, label: string, on: unknown][] = [
      ['show', 'eye', T('s.outTile'), !st.snap.hidden && s.out === true],
      ['dnd', 'moon', T('menu.dnd'), s.dnd],
      ['sound', 'sound', T('settings.sound'), s.sound],
      ['home', 'pill', T(HOME_KEY[s.display as string] || 's.displayIsland'), true],
    ]
    return `${sec(T('home.sessions'))}<div class="grp">${list}</div>
      ${sec(T('s.quick'))}<div class="tiles">${tiles
        .map(([key, icon, label, on]) => `<div class="tile${on ? ' on' : ''}" data-tile="${key}"><span class="c">${svg(icon)}</span>${esc(label)}</div>`)
        .join('')}</div>`
  }

  // A row to the market on its kind (spec §1: 获取更多宠物…, 获取更多壁纸…),
  // at the end of what is installed: widgets:market:pet or :wallpaper.
  const toMarketRow = (kind: string, title: string, note: string) =>
    `<div class="r jumps mk-more" data-look-market="${kind}" role="button" tabindex="0"><span class="mk-big sm">&#xE719;</span><div class="grow"><div>${esc(T(title))}</div><div class="d">${esc(T(note))}</div></div><span class="chev">&#xE76C;</span></div>`

  // Her pets: each with who made it and where it came from (a pack's:
  // 来自市场, or 未经审核 from a file: AddonUI.petNote), the one in use or
  // 使用; then 获取更多宠物… (the market on its pets); the gallery of
  // codex-pets.net and its download box as ever.
  function pagePets() {
    const current = st.snap.settings.pet
    // 删除 on those in her own folder (downloaded, or the market's), asked
    // first (petDeleteLayerOf); never Codex's, nor the only one (the user,
    // 2026-11-08: 宠物增加删除按钮).
    const canDelete = (p: { mine?: boolean }) => p.mine && st.snap.pets.length > 1
    const installed = st.snap.pets.length
      ? st.snap.pets
          .map(
            p => `<div class="r petrow">${thumb(52, p.url, p.version)}<div class="grow"><div>${esc(p.name)}</div><div class="d">${esc(AddonUI.petNote(p, T))}</div></div>
            ${p.id === current ? `<span class="d in-use">${esc(T('home.inUse'))}</span>` : `<button class="pbtn" data-use="${esc(p.id)}">${esc(T('home.use'))}</button>`}${canDelete(p) ? `<button class="pbtn no" data-pet-delete="${esc(p.id)}">${esc(T('pets.delete'))}</button>` : ''}</div>`,
          )
          .join('')
      : `<div class="note">${esc(T('settings.petsEmpty'))}</div>`
    const note = st.fetchNote.text ? `<div class="note${st.fetchNote.isError ? ' err' : ''}">${esc(st.fetchNote.text)}</div>` : `<div class="note">${esc(T('settings.fetchNote'))}</div>`
    const have = new Set(st.snap.pets.map(p => p.id))
    const cards = gallery.items
      .map(
        g => `<div class="card"><div class="pv">${g.preview ? `<img src="${esc(g.preview)}" alt="" loading="lazy">` : ''}</div>
          <div class="nm ellip" title="${esc(g.name)}">${esc(g.name)}</div><div class="d ellip">${esc(g.author)} · ♥ ${g.likes}</div>
          ${have.has(g.id) ? `<span class="d in-use">${esc(T('home.installed'))}</span>` : `<button class="pbtn sm" data-get="${esc(g.id)}" ${gallery.getting ? 'disabled' : ''}>${esc(gallery.getting === g.id ? T('home.getting') : T('home.get'))}</button>`}</div>`,
      )
      .join('')
    const more =
      gallery.loading ? `<div class="note">${esc(T('home.getting'))}</div>` : gallery.page < gallery.totalPages ? `<button class="pbtn wide" data-more>${esc(T('home.more'))}</button>` : ''
    // The button downloads a link, and looks for other words.
    const fetchLabel = st.fetching ? T('settings.fetching') : T(isLink(st.typedRef.trim()) ? 'settings.fetch' : 's.search')
    const q = gallery.query
    const listTitle = !q ? T('home.gallery') : gallery.loading && !gallery.items.length ? T('s.searching', { q }) : T('s.found', { q, n: gallery.total })
    const none = q && !gallery.loading && !gallery.error && !gallery.items.length ? `<div class="note">${esc(T('s.foundNone', { q }))}</div>` : ''
    return `${sec(T('home.installed'))}<div class="grp">${installed}${toMarketRow('pet', 'packs.morePets', 'packs.morePetsNote')}</div>
      ${sec(T('s.download'))}<div class="grp"><div class="r"><input class="in" id="s-ref" spellcheck="false" placeholder="${esc(T('s.fetchPlaceholder'))}"><button class="pbtn al" data-fetch ${st.fetching ? 'disabled' : ''}>${esc(fetchLabel)}</button></div>${note}</div>
      <div class="sec row-sec"><span class="ellip">${esc(listTitle)}</span><span class="grow"></span>${seg('gallery', [['popular', T('home.popular')], ['newest', T('home.newest')]], gallery.sort)}</div>
      ${gallery.error ? `<div class="note err">${esc(T('home.galleryError', { message: gallery.error }))}</div>` : ''}${none}
      <div class="cards">${cards}</div>${more}
      <div class="links"><span class="link" data-site="site">${esc(T('settings.browse'))} ›</span></div>`
  }

  // --- The parts' core ----------------------------------------------------------------
  // What every part (settings-*.ts) is given as it is made below, each in
  // its section's place: what the settings remember (st), the parts made so
  // far (pieces: each one's functions as it hands them back, a later
  // part's reached once the settings are up), and the settings' own
  // constants and functions a part uses.
  const pieces = {} as SettingsParts
  const core: SettingsCore = {
    st, pieces, go, AGENTS, AGENT_NAMES, AddonUI, Agents, COLOR, Island, MOST_FILES, SIZES, SPINS, T, Widgets,
    body, drafts, draw, esc, expanded, freshRows, inbox, keepDraft, layer, loadGallery, market, nameList,
    onTaskbar, openHardware, pageAlerts, pageNow, pagePets, part, patch, placeHTML, placeOf, pluginFields,
    pluginNote, redrawMail, relayout, render, row, sec, seg, setHTML, sheet, showPart, sw, thumb, toMarketRow,
    toastEl, widgetRows,
  }

  // --- Her island's look (island-look.ts drawIslandLook) ---------------------------------
  // In settings-look.ts:
  pieces.look = lookPart(core)
  const {
    ACCENT_PICKER, ilookDefault, ilookFor, ilookInput, ilookOf, ilookPresets, ilookSave, lhmAsks, pageLook,
    paperDo, winMode,
  } = pieces.look

  function pageAlerts() {
    const s = st.snap.settings
    // When she has something to tell that needs no answer (a turn's end, a
    // hello, a plugin's nudge): the island open, a ring round it (the flow
    // light, the default), or the dot alone (island-ring.ts cue); its three
    // choices under it, too long beside it.
    const cue = Flow.endCue(s)
    const cues = [
      ['open', T('s.endCueOpen')],
      ['ring', T('s.endCueRing')],
      ['none', T('s.endCueNone')],
    ]
    // How long it stays (停留多久, just under it: zhoupengjie/Wakuwaku#59,
    // #65): the island opened, or the ring lit and still after its turns
    // (its note says which, for the choice above); with the dot alone there
    // is nothing it holds (the dot waits until seen), so it is greyed then,
    // its choice kept. The choices under the words (five, too long beside
    // them), and with Custom on, the box for its seconds beside them, kept
    // as it is left or Enter is pressed (holdTyped).
    const holdOff = cue === 'none'
    const holds = HOLDS.map(h => [h, h === 'seen' ? T('s.holdSeen') : h >= 60 ? T('settings.minutes', { n: h / 60 }) : T('settings.seconds', { n: h })])
    holds.push(['custom', T('s.holdCustom')])
    const holdOn = Flow.holdChoice(s.hold, st.holdCustom)
    const range = esc(T('s.holdRange', { min: Flow.HOLD_MIN, max: Flow.HOLD_MAX }))
    const typed = 'hold:secs' in drafts ? drafts['hold:secs'] : Number.isInteger(s.hold) ? String(s.hold) : ''
    const secs =
      holdOn === 'custom'
        ? `<span class="hold-secs"><input class="in secs" data-hold="secs" value="${esc(typed)}" inputmode="numeric" maxlength="5" placeholder="${Flow.HOLD_MIN}–${Flow.HOLD_MAX}" title="${range}" aria-label="${range}" spellcheck="false" autocomplete="off"${holdOff ? ' disabled' : ''}><span class="d">${esc(T('s.holdUnit'))}</span></span>`
        : ''
    const holdPick = holdOff ? seg('hold', holds, holdOn).replace('class="seg"', 'class="seg off" aria-disabled="true"') : seg('hold', holds, holdOn)
    const waits = WAITS.map(w => [w, w >= 60 ? T('settings.minutes', { n: Math.round(w / 60) }) : T('settings.seconds', { n: w })])
    const idles = IDLE_HIDES.map(m => [m, !m ? T('s.idleAlways') : m >= 60 ? T('s.paperHours', { n: m / 60 }) : T('s.minutesShort', { n: m })])
    const notify = s.notify || {}
    return `${sec(T('s.holdTitle'))}<div class="grp"><div class="r col cue"><div><div>${esc(T('s.endCue'))}</div><div class="d">${esc(T('s.endCueNote'))}</div></div>${seg('endCue', cues, cue)}</div><div class="r col cue hold${holdOff ? ' off' : ''}"><div><div class="t">${esc(T('s.hold'))}</div><div class="d">${esc(T(`s.holdNote.${cue}`))}</div></div><div class="hold-pick">${holdPick}${secs}</div></div></div>
      ${sec(T('s.idleTitle'))}<div class="grp">${row(esc(T('s.idleHide')), esc(T('s.idleHideNote')), seg('idleHideMin', idles, s.idleHideMin))}</div>
      ${sec(T('settings.notify'))}<div class="grp">
        ${row(esc(T('settings.notifyWaiting')), '', sw('notify.waiting', notify.waiting))}
        ${row(esc(T('settings.notifyDone')), '', sw('notify.done', notify.done))}
        ${row(esc(T('settings.notifyError')), '', sw('notify.error', notify.error))}
        ${row(esc(T('settings.sound')), '', sw('sound', s.sound))}
      </div>
      ${sec(T('settings.prompts'))}<div class="grp">${row(esc(T('settings.promptWait')), esc(T('settings.promptWaitNote')), seg('promptWaitSec', waits, s.promptWaitSec))}</div>
      ${sec(T('s.quiet'))}<div class="grp">
        ${row(esc(T('menu.dnd')), esc(T('s.dndNote')), sw('dnd', s.dnd))}
        ${row(esc(T('settings.hideInFullscreen')), '', sw('hideInFullscreen', s.hideInFullscreen, st.snap.fullscreenAvailable ? '' : 'disabled'))}
      </div>`
  }

  // The plugins page as rows, in the order the person put them in: the
  // built-in ones, the plugins she runs (with what their widgets say),
  // other people's (addons.rs: plugins; the packs are a group of their
  // own, packsGroup), and the widgets of anyone else's scripts.
  function widgetRows(): WidgetRow[] {
    const rank = (key: string) => {
      const i = (st.snap.settings.widgetOrder || []).indexOf(key)
      return i < 0 ? Infinity : i
    }
    const widgets = st.snap.widgets || []
    const rows = [
      ...widgets.filter(w => w.builtIn).map(w => ({ key: w.id, widget: w })),
      ...(st.snap.plugins || []).map(p => ({ key: p.id, plugin: p, mine: widgets.filter(w => w.plugin === p.id) })),
      ...addonsNow().map(a => ({ key: a.id, addon: a, mine: widgets.filter(w => w.plugin === a.id) })),
      ...widgets.filter(w => !w.builtIn && !w.plugin).map(w => ({ key: w.id, widget: w })),
    ]
    return rows.map((r, i) => ({ ...r, i })).sort((x, y) => rank(x.key) - rank(y.key) || x.i - y.i)
  }

  // What a plugin she runs says under its name, as it is (placed is on, so
  // put nowhere is off): 没放; running, 运行中 and what it does; else why it
  // shows nothing (a parameter to fill in, 出错 with its last line). Put
  // somewhere and not on yet (picker.ts turns it on): starting.
  function pluginNote(p: Plugin, mine: Widget[], placed: boolean) {
    if (!placed) return { text: T('pl.unplaced') }
    if (p.state === 'off') return { text: T('pl.starting') }
    if (p.state === 'needs') return { text: T('pl.needs', { what: T(`pl.${p.id}.${p.missing}`) }), bad: true }
    // Ended with what went wrong (plugins.rs view: none of hers ends by
    // itself now that they run inside her; no PowerShell to start, no turn
    // to try again in so many seconds).
    if (p.state === 'stopped') return { text: T('pl.stopped', { line: p.said || '' }), bad: true }
    if (p.state === 'running' && !mine.length) return p.said ? { text: T('pl.trouble', { line: p.said }), bad: true } : { text: T('pl.starting') }
    return { text: `${T('pl.running')} · ${T(`w.ex.${p.id}.note`)}` }
  }

  // Where one is put (island-wmenu.ts placeNow): its place in a quiet word, or,
  // put nowhere, 添加小工具… (the picker, as her menu opens it, on its card:
  // key, picker.ts fromMenu focus).
  function placeOf(item: { plugin?: string; id?: string }, on: boolean | undefined): Place {
    return Island?.placeNow?.(item, on) || { at: on ? 'isle' : null }
  }
  function placeHTML(place: Place, key = '') {
    if (!place.at) return `<button class="pbtn sm" data-padd="${esc(key)}">${esc(T('pl.add'))}</button>`
    const where = esc(T(`pl.at.${place.at}`, { name: place.name || '' }))
    return `<span class="pl-at" title="${where}">${where}</span>`
  }

  // A plugin's settings, folded out under it: what is being typed stays.
  function pluginFields(p: Plugin) {
    const fields = p.params.map((f: PluginParam) => {
      const key = `${p.id}.${f.name}`
      const label = esc(T(`pl.${key}`))
      if (f.kind === 'flag') return `<div class="fr"><span class="fl">${label}</span><span class="grow"></span>${sw('pf:' + key, f.value === true)}</div>`
      // One of a few (a stock's up and down colours): the one in effect,
      // her language's until one is picked (plugins.rs choice).
      if (f.kind === 'choice') return `<div class="fr"><span class="fl">${label}</span><span class="grow"></span>${seg('pc:' + key, (f.options || []).map(o => [o, T(`pl.${key}.${o}`)]), f.value)}</div>`
      const value = key in drafts ? drafts[key] : f.value || ''
      return `<label class="fr"><span class="fl">${label}${f.required ? ' *' : ''}</span><input class="in" data-field="${esc(key)}" value="${esc(value)}" placeholder="${esc(T(`pl.${key}.hint`))}" spellcheck="false"${f.kind === 'number' ? ' inputmode="decimal"' : ''}></label>`
    })
    return `<div class="pf">${fields.join('')}</div>`
  }

  // --- Other people's plugins (addons.rs view; addonui.ts) ---------------------------------
  // In settings-addons.ts:
  pieces.addons = addonsPart(core)
  const {
    SUBS, addonById, addonField, addonRow, addonsNow, andList, backSub, canDoHTML, crumbHTML, openSub,
    packById, packTile, packsGroup, toLook,
  } = pieces.addons

  // --- The market (market.rs; addonui.ts) ------------------------------------------------
  // In settings-market.ts:
  pieces.market = marketPart(core)
  const {
    approveGo, cancelAsked, drawSheet, fillCards, installFromFile, installGo, loadMarket,
    marketInstall, marketItem, marketItemPage, marketPage, toast, uninstallGo, updateAll,
  } = pieces.market

  // --- The Plugins page, and the monitor's sensors -------------------------------------
  // In settings-plugins.ts:
  pieces.plugins = pluginsPart(core)
  const {
    pageWidgets, picksKey,
  } = pieces.plugins

  // --- Mail accounts and the inbox -----------------------------------------------------
  // In settings-mail.ts:
  pieces.mail = mailPart(core)
  const {
    APP_PASSWORD, AUTHS, PROXY_KINDS, PROXY_MODES, SECURITIES, WIDE_W, accountOf, agentChoices, agentsHTML,
    boxName, choicesOf, confOf, firstAgent, formProxy, handButtons, inboxHTML, inboxId, isLetter, isReading,
    labelOf, listed, loginWhy, mailDate, mailError, mailFix, mailFormHTML, mailList, mailNote, mailSaid,
    mailSize, outFields, people, providerOf, proxyFields, setConf, talkHTML, talkOf, twoPanes, where,
  } = pieces.mail

  // --- Writing a letter ----------------------------------------------------------------------
  // In settings-letter.ts:
  pieces.letter = letterPart(core)
  const {
    addFiles, closeMenu, fitFrames, hand, handMenu, loadInbox, menuEl, netNow, openChoices, openLetter,
    openMenu, openPicker, pageMail, pageNet, proxyAddress, proxySetting, replyIn, runNetTest, say, sendDraft,
    setPictures, setProxy, star, step, waySetting, write, writeAbout,
  } = pieces.letter

  // --- The Agent page (agents.ts) ---------------------------------------------------------
  // One row for each agent found on this computer: a dot for how it stands
  // in four of the island's colours (connected green, written and waiting
  // for its first turn blue, the person must act amber, not connected grey;
  // colour says how it stands, never which one it is), its name (and
  // version, where known), what she can do for it (five icons, lit, half or
  // dim, each with a tooltip saying which buttons or why not), and a button
  // only where the person has something to do (connect, repair, how to
  // trust Codex's hooks), else a faint ›. A row opens in place, one at a
  // time, to its details: what the person must do first (amber), how it
  // connects and the file written (the whole path, ~ for the home folder),
  // what works and what does not, what she can do in words, what to know,
  // reinstall and remove; Claude Code's two ways (the plugin,
  // settings.json). Connect all at the top, for those not connected yet;
  // those not found folded under "Other supported tools", each with where
  // to get it.
  // In settings-agents.ts:
  pieces.agents = agentsPart(core)
  const {
    aboutHTML, agentDo, connectAll, dshSwitch, failed, known, pageAgents, pageCrash, pageUpdate, sessionOpen,
  } = pieces.agents

  // --- The welcome: the agents connected, at the top of General -----------------------
  // While nothing is connected (and until one sends its first event, before
  // she is set up, onboarded): a card with a button for each one on this
  // computer, as equals (whichever one uses, or all), in a row: its name,
  // and under it how it stands; under them what each still needs, or why it
  // could not be written; what the buttons write; Claude Code's plugin the
  // other way. The same model as the Agent page (agents.ts: what is here,
  // how each stands, the same buttons). Once one is connected, General has
  // a line on them instead, which opens the Agent page (agentsLine). (The
  // appearance session's spec, 2026-10-13.)
  // "Not now" said (connectHintOff: the welcome's 以后再说, the island's
  // 不用提醒) and nothing connected: the welcome folded to one row at the
  // page's top; opened again by its 去连接, or by asking for connecting.
  // In settings-welcome.ts:
  pieces.welcome = welcomePart(core)
  const {
    PAGES, cardShows, displaysRow, failedHTML, screenRow, toolNames, toolsHere,
  } = pieces.welcome

  // --- Drawing --------------------------------------------------------------------------------

  function drawHead() {
    const now = st.snap.now
    const chosen = st.snap.pets.find(p => p.id === st.snap.settings.pet)
    const title = now.mood === 'idle' ? T('s.headIdle', { name: chosen ? chosen.name : 'Wakuwaku' }) : T(HEAD_KEY[now.mood])
    const asks = (st.snap.asks || []).length
    const sub = [Status.nameOf(now, st.snap.settings.details !== false), asks ? T('s.asksWaiting', { n: asks }) : ''].filter(Boolean).join(' · ')
    // By ✕, in Windows' own glyphs: locked, a click outside (or on the head)
    // closes them no more; on top, over every window as well (locked too:
    // on top and gone at the next click outside is no use). island.rs
    // keep_on_top.
    const locked = !!st.snap.settings.settingsPin
    const onTop = locked && !!st.snap.settings.settingsTop
    const lock = `<button class="x g${locked ? ' on' : ''}" data-lock title="${esc(T(locked ? 's.locked' : 's.lock'))}">${locked ? '&#xE72E;' : '&#xE785;'}</button>`
    const top = `<button class="x g${onTop ? ' on' : ''}" data-top title="${esc(T(onTop ? 's.onTop' : 's.top'))}">${onTop ? '&#xE840;' : '&#xE718;'}</button>`
    setHTML(
      head,
      `<div class="grow ellip"><span class="t">${esc(title)}</span>${sub ? ` <span class="s">· ${esc(sub)}</span>` : ''}</div>${clockTag(now)}${lock}${top}<button class="x" data-close title="${esc(T('s.close'))}">✕</button>`,
    )
  }

  function drawTabs() {
    const i = TABS.indexOf(st.tab)
    tabs.querySelector('.ind')!.style.transform = `translateX(${i * 100}%)`
    for (const b of tabs.querySelectorAll('button')) {
      b.classList.toggle('on', b.dataset.tab === st.tab)
      b.querySelector('span')!.textContent = T(TAB_KEY[b.dataset.tab!])
    }
    tabs.querySelector('[data-tab="now"] .n')!.hidden = !(st.snap.asks || []).length
    // An agent that wants the person (amber): the same dot on the Agent page's.
    tabs.querySelector('[data-tab="agents"] .n')!.hidden = !Agents.summary(st.snap, known()).wants.length
  }

  // The page, kept as it is when nothing changed; redrawn, the box being
  // typed in (the download box, a plugin's setting, the mail proxy's
  // address) gets the focus and the caret back.
  function drawBody(reset: boolean) {
    const box = body.contains(document.activeElement) && document.activeElement!.matches('input, textarea') ? (document.activeElement as HTMLInputElement | HTMLTextAreaElement) : null
    const which =
      box &&
      (box.id ? `#${box.id}` : box.dataset.field ? `[data-field="${box.dataset.field}"]` : box.dataset.mon ? `[data-mon="${box.dataset.mon}"]` : box.dataset.hold ? '[data-hold]' : box.dataset.mf ? `[data-mf="${box.dataset.mf}"]` : box.dataset.w ? `[data-w="${box.dataset.w}"]` : box.dataset.proxyAt !== undefined ? `[data-proxy-at="${box.dataset.proxyAt}"]` : null)
    const caret = which ? box.selectionStart : null
    const changed = setHTML(body, PAGES[st.tab]())
    if (reset) {
      body.scrollTop = 0
      // Two panes: what is on the right from its top; the list stays where it
      // was. What scrolls there is the part under the letter's bar
      // (.pane-r-body, zhoupengjie/Wakuwaku#114), the bar staying put.
      const right = body.querySelector('.pane-r-body') || body.querySelector('.pane-r')
      if (right) right.scrollTop = 0
    }
    if (!changed) return
    // Left as it is while it has the focus: set again, a word half-made in
    // an input method was lost (results landing while one typed).
    const again = (document.getElementById('s-ref') as HTMLInputElement)
    if (again && again.value !== st.typedRef) again.value = st.typedRef
    const back = which && body.querySelector<HTMLInputElement | HTMLTextAreaElement>(which)
    if (back && back !== document.activeElement) {
      back.focus()
      back.setSelectionRange(caret, caret)
    }
    startThumbs()
    fitFrames()
    fillCards()
  }

  function draw(reset = false) {
    if (!st.isOpen || !st.snap) return
    st.lang = st.snap.lang || st.lang
    // The Mail page in two panes is wider.
    const wide = twoPanes()
    layer.classList.toggle('wide', wide)
    layer.style.width = wide ? `${Math.min(WIDE_W, screen.availWidth - 40)}px` : ''
    drawHead()
    drawTabs()
    drawBody(reset)
    drawSheet()
    // Locked (the button by ✕), a click outside closes them no more.
    foot.querySelector('.foot-text')!.textContent = T(st.snap.settings.settingsPin ? 's.footLocked' : 's.foot')
    foot.querySelector('.ver')!.textContent = `v${st.snap.version}`
    tick()
    // The same height on every page, however much it holds: the head and
    // the tabs stay where they are from one page to the next (on the
    // taskbar it grows up from the strip, and a shorter page would take
    // them down from under the pointer). Never taller than the screen, nor
    // than the home keeps room for (the taskbar's): a page scrolls inside.
    const rest = layer.offsetHeight - body.offsetHeight
    const most = Island?.settingsMax?.() ?? screen.availHeight - 56
    body.style.height = `${Math.max(160, Math.min(460, most - rest))}px`
    Island?.changed()
  }

  // After the content changed height: measure once it is laid out.
  function relayout() {
    return requestAnimationFrame(() => Island?.changed())
  }

  // --- Open and close ----------------------------------------------------------------------------

  // A part of a page asked for by its old page's name (st.partToShow), scrolled to once drawn.
  function showPart() {
    let name = st.partToShow
    go({ type: 'part-shown' })
    if (!name) return
    const scroll = () => {
      const at = body.querySelector(`[data-part="${CSS.escape(name)}"]`)
      if (at) body.scrollTop += at.getBoundingClientRect().top - body.getBoundingClientRect().top
      return body.scrollTop
    }
    // There again as the page is drawn anew in a moment (the default pet put
    // in place, a snapshot coming), unless scrolled since: it was scrolled
    // once, before what came after moved it away (the test session, 2026-10-12).
    let set = scroll()
    for (const ms of [250, 800, 1600]) {
      setTimeout(() => {
        if (Math.abs(body.scrollTop - set) <= 1) set = scroll()
      }, ms)
    }
  }

  function open(next: string) {
    // At what was asked (settings-state.ts open): connecting (the welcome
    // open, folded or not), the market by category (widgets:market[:widget|
    // pet|wallpaper]: the picker's 获取更多…, the Look page's: on the
    // Plugins page, in its place), a part of a page, a widget's row, a page.
    const toMarket = AddonUI.marketLink(next)
    const wasOpen = st.isOpen
    go({ type: 'open', to: next, market: !!toMarket, now: Date.now() })
    if (toMarket) {
      market.cat = toMarket.cat
      market.query = ''
    }
    if (!wasOpen) {
      pet.keyboard(true)
      clearInterval(st.clockTimer)
      st.clockTimer = setInterval(tick, 1000)
    } else if (!document.hasFocus()) {
      // Locked and gone under another window: asked for again, to the front.
      pet.keyboard(true)
    }
    pet.settings.get().then(got => {
      st.snap = got
      // Connecting while the welcome card shows (nothing connected yet):
      // the card, at General's top; else the Agent page.
      if (st.partToShow === 'connect') go({ type: 'shown', cardShows: cardShows() })
      draw(true)
      showPart()
      if (st.tab === 'look') loadGallery(false)
      if (st.tab === 'widgets' && st.sub?.page === 'market') loadMarket(false)
      if (st.tab === 'mail' && st.letterToOpen) {
        const { account, uid, folder } = st.letterToOpen
        st.letterToOpen = null
        openLetter(account, uid, folder || 'inbox')
      }
      layer.focus({ preventScroll: true })
    })
    Island?.changed()
  }

  function close(why = 'asked') {
    if (!st.isOpen) return
    console.log('settings close:', why)
    // The welcome opened again only for this time: "not now" said since
    // (the island's 不用提醒), the next open shows it folded (the test session).
    // The Agent page opens again with its rows closed; the Plugins page as
    // itself (settings-state.ts close), nothing asked over it, no row edged as new.
    go({ type: 'close' })
    st.gone = null
    st.mkAsk = null
    st.approveFor = null
    setRecording(null)
    if (st.flow.step === 'ask' || st.flow.step === 'refused') st.flow = AddonUI.IDLE
    freshRows.clear()
    drawSheet()
    toastEl.hidden = true
    clearInterval(st.clockTimer)
    for (const timer of st.thumbTimers) clearTimeout(timer)
    st.thumbTimers = []
    // Seconds being typed are kept as the box is left (holdTyped); Custom is
    // on next time only for a custom number kept.
    if (layer.contains(document.activeElement)) (document.activeElement as HTMLElement).blur()
    st.holdCustom = false
    pet.settingsClosed()
    Island?.changed()
  }

  function setTab(next: string) {
    if (next === st.tab || !TABS.includes(next)) return
    go({ type: 'tab', to: next })
    draw(true)
    if (st.tab === 'look') loadGallery(false)
    relayout()
  }

  // A change: shown at once, then whatever main makes of it.
  function patch(change: Settings) {
    Object.assign(st.snap.settings, change)
    draw()
    pet.settings.set(change).then(got => {
      st.snap = got
      draw()
    })
  }

  async function loadGallery(more: boolean) {
    if (more ? gallery.loading : gallery.page > 0) return
    const asked = ++gallery.asked
    gallery.loading = true
    gallery.error = ''
    draw()
    const got = await pet.settings.gallery(more ? gallery.page + 1 : 1, gallery.sort, gallery.query)
    if (asked !== gallery.asked) return
    gallery.loading = false
    if (got.ok) {
      gallery.items = more ? [...gallery.items, ...got.items] : got.items
      gallery.page = got.page || 1
      gallery.totalPages = got.totalPages || 1
      gallery.total = got.total ?? gallery.items.length
    } else {
      gallery.error = got.error
    }
    draw()
    relayout()
  }

  // The gallery as a search finds it (no words: the gallery itself), from
  // its first page.
  function search(text: string) {
    clearTimeout(st.searchTimer)
    const query = text.trim()
    if (query === gallery.query) return
    gallery.query = query
    gallery.page = 0
    gallery.items = []
    gallery.total = 0
    loadGallery(false)
  }

  // Typed: looked for a moment after the last key (a link is not looked
  // for; the button downloads it), the button saying which it does.
  function searchSoon() {
    clearTimeout(st.searchTimer)
    const text = st.typedRef.trim()
    const button = layer.querySelector('[data-fetch]')
    if (button && !st.fetching) button.textContent = T(isLink(text) ? 'settings.fetch' : 's.search')
    if (!isLink(text)) st.searchTimer = setTimeout(() => search(text), 400)
  }

  async function fetchPet(reference: string, fromGallery: boolean) {
    if (st.fetching || gallery.getting) return
    if (fromGallery) gallery.getting = reference
    else st.fetching = reference
    st.fetchNote = { text: '', isError: false }
    draw()
    const got = await pet.settings.fetch(reference)
    st.fetching = null
    gallery.getting = ''
    if (got.snapshot) st.snap = got.snapshot
    if (got.ok) {
      st.fetchNote = { text: [T('settings.fetched', { name: got.pet.name, author: got.pet.author }), got.warning].filter(Boolean).join(' '), isError: false }
      if (!fromGallery) st.typedRef = ''
    } else {
      st.fetchNote = { text: got.error, isError: true }
    }
    draw()
    relayout()
  }

  function copy(text: string, button: HTMLElement) {
    const done = () => {
      button.textContent = T('home.copied')
      setTimeout(() => (button.textContent = button.dataset.label || T('home.copy')), 1800)
    }
    navigator.clipboard?.writeText(text).then(done, () => {
      const area = document.createElement('textarea')
      area.value = text
      document.body.append(area)
      area.select()
      document.execCommand('copy')
      area.remove()
      done()
    })
  }

  // --- Clicks and keys ----------------------------------------------------------------------------

  // Each element found below by a data- attribute has it: its dataset's
  // entry is there (!).
  layer.addEventListener('click', e => {
    const at: At = selector => (e.target as HTMLElement).closest(selector)
    // A click anywhere but on it closes the letter menu.
    if (!menuEl.hidden && !at('.mmenu')) closeMenu()
    // The question over the panel (installing a plugin, taking one away):
    // its buttons, its link to the developer docs; a press beside it is 取消.
    if (!sheet.hidden && sheet.contains(e.target as Node)) {
      const site = at('[data-site]')
      if (site) return pet.settings.openSite(site.dataset.site!)
      const b = at<HTMLButtonElement>('[data-sheet]')
      if (b?.disabled) return
      const what = b ? b.dataset.sheet : at('.sh') ? '' : 'cancel'
      if (what === 'go') return installGo()
      if (what === 'uninstall') return uninstallGo()
      if (what === 'approve') return approveGo()
      // The market's: installing one that asks for more, 全部更新 asked first.
      if (what === 'mkgo' || what === 'mkall') {
        const asked = st.mkAsk
        st.mkAsk = null
        drawSheet()
        // mkgo is the install's button: asked is { kind: 'install', id }.
        return what === 'mkgo' ? marketInstall(asked?.id!, true) : updateAll(true)
      }
      if (what === 'cancel') cancelAsked()
      return
    }
    if (at('[data-lock]')) return patch(st.snap.settings.settingsPin ? { settingsPin: false, settingsTop: false } : { settingsPin: true })
    if (at('[data-top]')) return patch(st.snap.settings.settingsPin && st.snap.settings.settingsTop ? { settingsTop: false } : { settingsPin: true, settingsTop: true })
    if (at('[data-close]')) {
      // Locked: ✕ closes them (and Esc), a press on the head no more.
      if (st.snap.settings.settingsPin && !at('.x[data-close]')) return
      return close('click')
    }
    const keyBind = at('[data-key-bind]')
    if (keyBind) {
      const key = keyBind.dataset.keyBind!
      setRecording(st.recordingShortcut === key ? null : key)
      return draw()
    }
    const keyClear = at('[data-key-clear]')
    if (keyClear) {
      const key = keyClear.dataset.keyClear!
      setRecording(null)
      return patch({ [key]: '' })
    }
    if (st.recordingShortcut && !at('.kbadge')) {
      setRecording(null)
      draw()
    }
    const tabButton = at('[data-tab]')
    if (tabButton) return setTab(tabButton.dataset.tab!)
    const sessToggle = at('[data-sess-open]')
    if (sessToggle) {
      const id = sessToggle.dataset.sessOpen!
      if (sessionOpen.has(id)) sessionOpen.delete(id)
      else sessionOpen.add(id)
      draw()
      return
    }
    // A session: to its window (the settings close as it comes to the front).
    const session = at('[data-jump]')
    if (session) {
      return pet.jump(session.dataset.jump).then(went => {
        if (!went) session.querySelector('.d')!.textContent = T('jump.notFound')
      })
    }
    const s = st.snap?.settings
    if (!s) return

    // Mail: an account set up, changed, kept, removed; its server looked
    // up, or typed in.
    if (handleMail(at)) return
    const toggle = at<HTMLButtonElement>('[data-sw]')
    if (toggle && !toggle.disabled) {
      const key = toggle.dataset.sw!
      if (key === 'login') return pet.settings.login(!st.snap.loginAtStart).then(got => ((st.snap = got), draw()))
      // Crash reports: sent by themselves, or never (not asked again).
      if (key === 'crashReports') return patch({ crashReports: st.snap.crash?.mode === 'auto' ? 'off' : 'auto' })
      if (key === 'paperShuffle') return paperDo('shuffle', !st.snap.winPaper?.shuffle)
      if (key === 'ilookBare') return ilookSave(ilookFor(), { ...ilookOf(ilookFor()), bare: !ilookOf(ilookFor()).bare })
      if (key.startsWith('notify.')) return patch({ notify: { ...s.notify, [key.slice(7)]: !s.notify?.[key.slice(7)] } })
      if (key === 'widgetNudge') return patch({ widgetNudge: s.widgetNudge === false })
      // Mail's codes on the island: on unless turned off (mail/codes.rs).
      if (key === 'mailCodes') return patch({ mailCodes: s.mailCodes === false })
      // On unless turned off (market.rs).
      if (key === 'pluginAutoUpdate') return patch({ pluginAutoUpdate: s.pluginAutoUpdate === false })
      // One of the monitor's parts on or off.
      if (key.startsWith('mon:')) return patch({ monitor: { ...(s.monitor || {}), [key.slice(4)]: (s.monitor || {})[key.slice(4)] === false } })
      // One of the hardware's sensors picked or not, LibreHardwareMonitor's
      // (lhms:) or the built-in ones (hws:), each source's own: from those
      // picked, or the ones shown by default (lhm.rs, hw.rs defaults) while
      // none were.
      if (key.startsWith('lhms:') || key.startsWith('hws:')) {
        const source = key.startsWith('lhms:') ? 'lhm' : 'builtin'
        const id = key.slice(key.indexOf(':') + 1)
        const name = picksKey(source)
        const view: { sensors?: { id: string; on?: boolean; toggle?: string }[] } | undefined = source === 'lhm' ? st.snap.lhm : st.snap.hw
        const picked: string[] = Array.isArray(s.monitor?.[name]) ? s.monitor[name] : (view?.sensors || []).filter(x => x.on && !x.toggle).map(x => x.id)
        return patch({ monitor: { ...(s.monitor || {}), [name]: picked.includes(id) ? picked.filter(x => x !== id) : [...picked, id].slice(-32) } })
      }
      // A plugin's yes-or-no setting; someone else's (its manifest's flag:
      // as set, else its default).
      if (key.startsWith('pf:')) {
        const [id, name] = key.slice(3).split('.')
        const p = (st.snap.plugins || []).find(x => x.id === id)
        return p && setPlugin(id, { [name]: !(p.params.find((f: PluginParam) => f.name === name) || {}).value })
      }
      if (key.startsWith('af:')) {
        const [id, name] = key.slice(3).split('.')
        const f = addonField(id, name)
        return f && setPlugin(id, { [name]: (f.value ?? f.default) !== true })
      }
      return patch({ [key]: !s[key as keyof Settings] })
    }
    // A hardware's sensors folded out, or in.
    const hardware = at('[data-lhw]')
    if (hardware) {
      const id = hardware.dataset.lhw!
      if (!openHardware.delete(id)) openHardware.add(id)
      return draw()
    }
    // LibreHardwareMonitor's password forgotten.
    if (at('[data-lhm-forget]')) return patch({ monitor: { ...(s.monitor || {}), lhmPass: '' } })
    // The monitor a second faster or slower.
    const pace = at<HTMLButtonElement>('[data-every]')
    if (pace && !pace.disabled) {
      const every = Math.min(10, Math.max(1, (s.monitor?.every || 2) + Number(pace.dataset.every)))
      return patch({ monitor: { ...(s.monitor || {}), every } })
    }
    // A plugin's settings folded out, or in.
    const fold = at('[data-pexp]')
    if (fold) {
      st.openPlugin = st.openPlugin === fold.dataset.pexp ? '' : fold.dataset.pexp!
      draw()
      return relayout()
    }
    // One put nowhere: 添加小工具…, the picker as her menu's opens it
    // (picker.ts fromMenu: on the taskbar, editing it with the picker open;
    // at the top, the picker alone), at its card; the settings make way.
    const add = at('[data-padd]')
    if (add) {
      close('picker')
      return Picker?.fromMenu?.({ open: 'picker', focus: add.dataset.padd || '' })
    }
    // Someone else's plugin: its own page (its name), back from it; turned
    // on again after it stopped responding; 卸载, asked first; one installed
    // from a .zip (Windows' dialog, then asked); a choice of many, its list.
    const addon = at('[data-addon]')
    if (addon) return openSub({ page: 'addon', id: addon.dataset.addon })
    // A pack: its own page (its name); 在「外观」里选, the Look page at its
    // kind's part; 卸载 there, asked first.
    const pack = at('[data-pack]')
    if (pack) {
      const [kind, id] = pack.dataset.pack!.split(':')
      return openSub({ page: 'pack', kind, id })
    }
    const look = at('[data-pack-look]')
    if (look) return toLook(look.dataset.packLook!)
    const packOut = at('[data-pack-uninstall]')
    if (packOut) {
      const [kind, id] = packOut.dataset.packUninstall!.split(':')
      st.gone = { id, kind, busy: false }
      return drawSheet()
    }
    // A pet's 删除 in the Look page's list: as a pack's 卸载, in its own words.
    const petOut = at('[data-pet-delete]')
    if (petOut) {
      st.gone = { id: petOut.dataset.petDelete!, kind: 'pet', busy: false, del: true }
      return drawSheet()
    }
    // The Look page's 获取更多宠物… and 获取更多壁纸…: the market on its kind.
    const lookMarket = at('[data-look-market]')
    if (lookMarket) {
      open(`widgets:market:${lookMarket.dataset.lookMarket}`)
      return relayout()
    }
    if (at('[data-sub-back]')) return backSub()
    // The market: opened (获取更多 ›); a row's button (安装, 更新; on its
    // page 添加小工具… or the Look page), then a row, to its page; read
    // again; to the Network proxy; 全部更新.
    if (at('[data-mk-open]')) {
      openSub({ page: 'market' })
      return loadMarket(false)
    }
    const mkDo = at<HTMLButtonElement>('[data-mk-do]')
    if (mkDo) {
      if (mkDo.disabled) return
      const id = mkDo.dataset.id!
      if (mkDo.dataset.mkDo === 'install' || mkDo.dataset.mkDo === 'update') return marketInstall(id)
      if (mkDo.dataset.mkDo === 'look') return toLook(marketItem(id)?.kind)
      if (mkDo.dataset.mkDo === 'add') {
        close('picker')
        return Picker?.fromMenu?.({ open: 'picker', focus: `plugin:${id}` })
      }
      return
    }
    const mkItem = at('[data-mk-item]')
    if (mkItem) return openSub({ page: 'mkItem', id: mkItem.dataset.mkItem })
    if (at('[data-mk-retry]')) return loadMarket(true)
    if (at('[data-mk-net]')) {
      go({ type: 'page', tab: 'now' })
      draw(true)
      relayout()
      go({ type: 'part', part: 'net' })
      return showPart()
    }
    const mkAll = at<HTMLButtonElement>('[data-mk-all]')
    if (mkAll && !mkAll.disabled) return updateAll()
    const revive = at<HTMLButtonElement>('[data-revive]')
    if (revive && !revive.disabled) {
      revive.disabled = true
      return pet.addons.revive(revive.dataset.revive).then(got => {
        if (got && typeof got === 'object' && got.settings) st.snap = got
        draw()
      })
    }
    const allow = at<HTMLButtonElement>('[data-approve]')
    if (allow && !allow.disabled) {
      st.approveFor = allow.dataset.approve!
      return drawSheet()
    }
    const out = at('[data-uninstall]')
    if (out) {
      st.gone = { id: out.dataset.uninstall!, busy: false }
      return drawSheet()
    }
    if (at('[data-addon-file]')) return installFromFile()
    const many = at('[data-achoice]')
    if (many) {
      const [id, name] = many.dataset.achoice!.split('.')
      const f = addonField(id, name)
      if (!f) return
      const choices = (f.options || []).map((o, i) => [o, f.optionLabels?.[i] || o])
      return openChoices(many, choices, f.value ?? f.default ?? f.options?.[0], v => setPlugin(id, { [name]: v }), true)
    }
    const option = at('[data-seg] > span')
    if (option) {
      if (option.classList.contains('off')) return
      const key = option.parentElement!.dataset.seg!
      const raw = option.dataset.value!
      // A plugin's choice (pc:<plugin>.<param>; someone else's ac:), kept:
      // the plugin started again with it (plugins.rs sync), or told it.
      if (key.startsWith('pc:') || key.startsWith('ac:')) {
        const [id, name] = key.slice(3).split('.')
        return setPlugin(id, { [name]: raw })
      }
      // The market's category.
      if (key === 'mk-cat') {
        market.cat = raw
        return draw()
      }
      if (key === 'taskbarLayout') {
        // Its parts as the preset has them; the widgets put there kept.
        const layout = Island?.tbWithPreset?.(raw)
        return layout && patch({ taskbarLayout: layout })
      }
      if (key === 'ilookMode') {
        st.ilookMode = raw === winMode() ? '' : raw as WinMode
        return draw()
      }
      if (key === 'ilookEdge') return ilookSave(ilookFor(), { ...ilookOf(ilookFor()), edge: raw })
      // How long endings stay: Custom shows the box for its seconds, the
      // caret in it (kept once typed: holdTyped); any other choice at once.
      if (key === 'hold') {
        st.holdCustom = raw === 'custom'
        if (st.holdCustom) {
          draw()
          const box = layer.querySelector<HTMLInputElement>('[data-hold]')
          box?.focus()
          box?.select()
          return
        }
      }
      if (key === 'paperPosition') return raw !== st.snap.winPaper?.position && paperDo('position', raw)
      if (key === 'paperEvery') return Number(raw) !== st.snap.winPaper?.every && paperDo('every', raw)
      if (key === 'winMode') {
        if ((raw === 'light') === !!st.snap.winLook?.light) return
        return pet.settings.winLook({ light: raw === 'light' }).then(got => ((st.snap = got), draw()))
      }
      if (key === 'dshVia') return dshSwitch(raw)
      if (key === 'gallery') {
        if (gallery.sort === raw) return
        gallery.sort = raw
        gallery.page = 0
        gallery.items = []
        return loadGallery(false)
      }
      // Where her island is (islandEdgeRow): one choice for two settings.
      if (key === 'islandWhere') return patch(raw === 'top' ? { islandEdge: 'top' } : { islandEdge: 'taskbar', islandEmbed: raw === 'taskbar' })
      const value = ['lang', 'monitorSource', 'islandPos', 'islandEdge', 'islandWidth', 'taskbarMaterial', 'taskbarClearWhen', 'taskbarButtons', 'taskbarAlign', 'taskbarEdge', 'taskbarDisplays', 'homeDisplay', 'splitDock', 'splitKeys', 'mailAgent', 'mailOffline', 'updateFrom', 'endCue'].includes(key) ? raw : raw === 'seen' ? 'seen' : Number(raw)
      return patch({ [key]: value })
    }
    const tile = at('[data-tile]')
    if (tile) {
      const key = tile.dataset.tile!
      if (key === 'show') {
        if (!st.snap.hidden) return patch({ out: s.out !== true })
        return pet.settings.showPet(true).then(got => ((st.snap = got), draw()))
      }
      if (key === 'home') return patch({ display: HOMES[(HOMES.indexOf(s.display as string) + 1) % HOMES.length] })
      return patch({ [key]: !s[key as keyof Settings] })
    }
    const mode = at('[data-mode]')
    if (mode) return mode.dataset.mode !== s.display && patch({ display: mode.dataset.mode })
    // The taskbar's layout dragged into shape on the strip: the settings make way.
    if (at('[data-tb-edit]')) {
      close('edit')
      return Island?.editTaskbar?.()
    }
    // Her island's look: folded out or in, a preset, a ground, back to Windows' grey.
    if (at('[data-ilook-fold]')) {
      st.ilookOpen = !st.ilookOpen
      draw()
      return relayout()
    }
    const ilookPick = at('[data-ilook-preset]')
    if (ilookPick) {
      const mode = ilookFor()
      // (Its card was drawn from the same presets.)
      return ilookSave(mode, { ...ilookPresets(mode).find(([id]) => id === ilookPick.dataset.ilookPreset)![1] })
    }
    const ilookBg = at('[data-ilook-bg]')
    if (ilookBg) return ilookSave(ilookFor(), { ...ilookOf(ilookFor()), bg: ilookBg.dataset.ilookBg! })
    if (at('[data-ilook-reset]')) return ilookSave(ilookFor(), ilookDefault(ilookFor()))
    // Windows' colour dialog takes the focus: not a click outside.
    if ((e.target as HTMLElement).matches('input[type="color"]')) st.picking = true
    // LibreHardwareMonitor: downloaded, started, its task, its folder (lhm_setup.rs).
    const lhmButton = at<HTMLButtonElement>('[data-lhm]')
    if (lhmButton && !lhmButton.disabled) {
      if (['get', 'start', 'startup', 'noStartup', 'remove'].includes(lhmButton.dataset.lhm!)) st.lhmAsking = Date.now()
      return pet.settings.lhm(lhmButton.dataset.lhm!).then(got => ((st.snap = got), draw()))
    }
    // Her updates: look now, the release's notes, Update.
    const updateButton = at<HTMLButtonElement>('[data-update]')
    if (updateButton && !updateButton.disabled) return pet.settings.update(updateButton.dataset.update!).then(got => ((st.snap = got), draw()))
    // The Network proxy: shown (from an update not reached), tried.
    if (at('[data-net-go]')) return (go({ type: 'part', part: 'net' }), showPart())
    const netButton = at<HTMLButtonElement>('[data-net-test]')
    if (netButton && !netButton.disabled) return runNetTest()
    if (at('[data-crash-show]')) return ((st.crashShown = !st.crashShown), draw())
    const crashButton = at<HTMLButtonElement>('[data-crash]')
    if (crashButton && !crashButton.disabled) return pet.settings.crash(crashButton.dataset.crash!).then(got => ((st.snap = got), draw()))
    const paperButton = at<HTMLButtonElement>('[data-paper]')
    if (paperButton && !paperButton.disabled) return paperDo(paperButton.dataset.paper!, paperButton.dataset.packId)
    const accent = at('[data-accent]')
    if (accent && ACCENT_PICKER) return pet.settings.winLook({ accent: accent.dataset.accent }).then(got => ((st.snap = got), draw()))
    const use = at('[data-use]')
    if (use) return patch({ pet: use.dataset.use })
    if (at('[data-fetch]')) {
      const text = st.typedRef.trim()
      if (!text) return document.getElementById('s-ref')?.focus()
      return isLink(text) ? fetchPet(text, false) : search(text)
    }
    const get = at('[data-get]')
    if (get) return fetchPet(get.dataset.get!, true)
    if (at('[data-more]')) return loadGallery(true)
    const copyButton = at('[data-copy]')
    if (copyButton) return copy(copyButton.dataset.copy!, copyButton)
    // An agent connected, repaired, removed (the welcome's buttons and the
    // Agent page's: data-agent="<id>:<install | remove>"); before a row's
    // own click, which opens it.
    const agentButton = at<HTMLButtonElement>('[data-agent]')
    if (agentButton) {
      if (agentButton.disabled) return
      const [id, action] = agentButton.dataset.agent!.split(':')
      return agentDo(id, action)
    }
    if (at('[data-ag-all]')) return connectAll()
    // DSH's hook bridge again, its packages from npmmirror (picked by the person).
    const mirror = at<HTMLButtonElement>('[data-dsh-mirror]')
    if (mirror) return mirror.disabled ? undefined : dshSwitch(failed.dsh?.via || 'bridge', 'npmmirror')
    // A row opened in place, or closed; one at a time. Its 怎么信任 (Codex's step) opens it.
    const agentRowAt = at('[data-ag-open]')
    if (agentRowAt) {
      const id = agentRowAt.dataset.agOpen!
      go({ type: 'agent', id, keep: agentRowAt.hasAttribute('data-ag-keep') })
      draw()
      relayout()
      return requestAnimationFrame(() => body.querySelector(`.ag-row[data-ag-open="${id}"] + .ag-det`)?.scrollIntoView({ block: 'nearest' }))
    }
    if (at('[data-ag-others]')) {
      go({ type: 'others' })
      draw()
      return relayout()
    }
    // The CPU temperature/power helper's switch (#18 part B): set up, or
    // remove, through an elevated child of hers (one UAC). The state updates
    // as the helper starts answering (the backend pushes while the settings
    // are open).
    const cpuSensors = at('[data-cpu-sensors]')
    if (cpuSensors) {
      return pet.settings.cpuSensors(cpuSensors.dataset.cpuSensors!).then(got => {
        st.snap = got.snapshot
        draw()
        relayout()
      })
    }
    // The welcome folded ("not now", the island's hint put away too), or
    // opened again from its row.
    if (at('[data-welcome-later]')) {
      go({ type: 'welcome', open: false })
      patch({ connectHintOff: true })
      return relayout()
    }
    if (at('[data-welcome-open]')) {
      go({ type: 'welcome', open: true })
      draw()
      return relayout()
    }
    // The welcome's plugin, folded out or in.
    if (at('[data-plugin-toggle]')) {
      st.pluginOpen = !st.pluginOpen
      draw()
      return relayout()
    }
    const site = at('[data-site]')
    // Google's pages for the address being added (data-for), when it says.
    if (site) return pet.settings.openSite(site.dataset.site!, site.dataset.for)
  })

  // A mail account's setup: what was clicked, if it was its. True when it was.
  function handleMail(at: At) {
    // How a letter goes to an agent: a model or effort picked, a picker
    // opened.
    const picked = at('[data-pick]')
    if (picked && st.menuPick) {
      const pick = st.menuPick
      closeMenu()
      pick(picked.dataset.pick!)
      return true
    }
    const picker = at('[data-picker]')
    if (picker) {
      openPicker(picker)
      return true
    }
    // Which agent letters go to: the dropdown; an open letter's ▾.
    const agentPick = at('[data-mail-agent]')
    if (agentPick) {
      openChoices(agentPick, agentChoices(), firstAgent(), v => v !== firstAgent() && patch({ mailAgent: v }), true)
      return true
    }
    const handMore = at('[data-hand-more]')
    if (handMore) {
      handMenu(handMore)
      return true
    }
    // The Mail page's proxy, the General page's Network proxy: its mode, a manual one's kind.
    const way = at('[data-seg^="mailProxy."] > span, [data-seg^="netProxy."] > span')
    if (way) {
      const [key, field] = way.parentElement!.dataset.seg!.split('.')
      setProxy(key as Way, { [field]: way.dataset.value })
      return true
    }
    // The inbox and a letter: handed to an agent (from a button or the
    // menu), starred, the menu, back, again, more, another inbox or filter,
    // one opened; more asked in a talk.
    // (A button with no account of its own is the open letter's.)
    const handing = at<HTMLButtonElement>('[data-hand]')
    if (handing) {
      const [agent, how] = handing.dataset.hand!.split(':')
      const ref = handing.dataset.account ? handing.dataset : { account: st.reading?.account, uid: st.reading?.uid, folder: st.reading?.folder }
      if (!handing.disabled) hand(ref.account!, Number(ref.uid), agent, how, ref.folder || 'inbox')
      return true
    }
    // An open letter's pictures from elsewhere: shown this once, for its
    // sender always, or not for its sender any more.
    const pictures = at('[data-pics]')
    if (pictures) {
      setPictures(pictures.dataset.pics!)
      return true
    }
    const starring = at('[data-star]')
    if (starring) {
      closeMenu()
      star(starring.dataset.star!, Number(starring.dataset.uid), starring.dataset.folder || 'inbox')
      return true
    }
    if (at('[data-talk-send]')) {
      const box = body.querySelector<HTMLInputElement>('[data-talk]')
      if (box && !box.disabled) say(box)
      return true
    }
    // A letter written: begun (new, a reply, passed on, with an agent's
    // words), put aside and taken up again, sent, thrown away; from whom,
    // cc and bcc shown, files added or taken off.
    if (handleWriting(at)) return true
    const more = at('[data-mail-menu]')
    if (more && st.reading) {
      const r = more.getBoundingClientRect()
      openMenu(st.reading.account, st.reading.uid, st.reading.folder, r.right - 220, r.bottom + 4)
      return true
    }
    const boxPick = at('[data-mail-box]')
    if (boxPick) {
      const all = st.snap.mail || []
      openChoices(boxPick, [['*', boxName('*')], ...all.map(a => [a.id, boxName(a.id)])], inboxId(), value => {
        inbox.id = value
        loadInbox(false)
      })
      return true
    }
    const filter = at('[data-seg="mail.filter"] > span')
    if (filter) {
      inbox.filter = filter.dataset.value!
      loadInbox(false)
      return true
    }
    // Two panes: the accounts and agents on the right again (a letter being
    // written put aside); one pane or two.
    if (at('[data-mail-setup]')) {
      st.reading = null
      st.writing = false
      draw(true)
      relayout()
      return true
    }
    const panes = at('[data-mail-panes]')
    if (panes) {
      patch({ mailPanes: panes.dataset.mailPanes })
      relayout()
      return true
    }
    if (at('[data-mail-back]')) {
      st.reading = null
      draw(true)
      relayout()
      return true
    }
    if (at('[data-mail-refresh]')) {
      // The server asked now (the list follows as the store changes).
      pet.mail.kick(inboxId())
      loadInbox(false)
      return true
    }
    if (at('[data-mail-more]')) {
      loadInbox(true)
      return true
    }
    // A conversation folded out or in (its ›).
    const twisty = at('[data-twisty]')
    if (twisty) {
      const key = twisty.dataset.twisty!
      if (!expanded.delete(key)) expanded.add(key)
      redrawMail()
      return true
    }
    const letter = at('.inbox [data-letter]')
    if (letter) {
      // Beside the list, a letter being written is put aside for it.
      if (twoPanes()) st.writing = false
      // A conversation folds out as its letter opens.
      if (letter.dataset.thread) expanded.add(letter.dataset.thread)
      openLetter(letter.dataset.account!, Number(letter.dataset.letter), letter.dataset.folder || 'inbox')
      return true
    }
    const turned = at('[data-sw^="mail:"]')
    if (turned) {
      const a = (st.snap.mail || []).find(x => x.id === turned.dataset.sw!.slice(5))
      if (a) pet.mail.switch(a.id, !a.on).then(got => ((st.snap = got), draw()))
      return true
    }
    if (at('[data-mail-add]')) {
      st.mailForm = { id: '', address: '', name: '', password: '', host: '', port: 993, security: 'ssl', auth: 'auto', username: '', ...outFields(null), ...proxyFields(null), step: 'start', advanced: false, probed: null, probedOut: null, busy: '', result: null }
      st.mailDelete = ''
      redrawMail('[data-mf="address"]')
      return true
    }
    const change = at('[data-mail-edit]')
    if (change) {
      const a = (st.snap.mail || []).find(x => x.id === change.dataset.mailEdit)
      if (a) {
        st.mailForm = { id: a.id, address: a.address, name: a.name || '', password: '', host: a.host, port: a.port, security: a.security, auth: a.auth || 'auto', username: a.username, ...outFields(a.smtp), ...proxyFields(a.proxy), step: 'found', source: '', advanced: true, probed: null, probedOut: null, busy: '', result: null }
        // One from before the pet sent mail: its outgoing server looked up.
        if (!a.smtp) findOutgoing(st.mailForm)
      }
      st.mailDelete = ''
      redrawMail('[data-mf="password"]')
      return true
    }
    if (at('[data-mail-cancel]')) {
      st.mailForm = null
      redrawMail()
      return true
    }
    // The fold opened or closed; a way or a method picked; Re-test.
    if (at('[data-mail-adv]') && st.mailForm) {
      st.mailForm.advanced = !st.mailForm.advanced
      redrawMail(st.mailForm.advanced ? '[data-mf="host"]' : null)
      return true
    }
    const mfpick = at('[data-mfpick]')
    if (mfpick && st.mailForm) {
      const f = st.mailForm
      const key = mfpick.dataset.mfpick as MailPick
      // The account's own way to its servers; manual for the first time,
      // the address the Mail page or Windows has.
      if (key === 'proxyMode' || key === 'proxyKind') {
        const mode = key === 'proxyMode'
        openChoices(mfpick, mode ? PROXY_MODES() : PROXY_KINDS, f[key] || (mode ? '' : 'http'), value => {
          f[key] = value
          if (value === 'manual' && !String(f.proxyAddress || '').trim()) {
            const known = proxySetting().address ? proxySetting() : st.snap.mailSystemProxy
            Object.assign(f, known ? { proxyKind: known.kind, proxyAddress: known.address } : { proxyAddress: '127.0.0.1:7890' })
          }
          f.probed = f.probedOut = null
          redrawMail(value === 'manual' ? '[data-mf="proxyAddress"]' : null)
        }, !mode)
        return true
      }
      const out = key.startsWith('smtp')
      const isWay = /security$/i.test(key)
      openChoices(mfpick, isWay ? SECURITIES() : AUTHS(), f[key] || 'auto', value => {
        f[key] = value
        // The usual port for the way, if the port was the other way's.
        const usual = out ? { ssl: 465, starttls: 587, plain: 587, auto: '' } : { ssl: 993, starttls: 143, plain: 143, auto: '' }
        const portKey = out ? 'smtpPort' : 'port'
        if (isWay && (out ? ['465', '587', '25', ''] : ['993', '143', '']).includes(String(f[portKey] ?? ''))) f[portKey] = usual[value as keyof typeof usual]
        if (out) f.probedOut = null
        else f.probed = null
        draw()
      })
      return true
    }
    if (at('[data-mail-probe]')) {
      retest()
      return true
    }
    if (at('[data-mail-find]')) {
      findServer()
      return true
    }
    if (at('[data-mail-save]')) {
      saveAccount()
      return true
    }
    const remove = at('[data-mail-remove]')
    if (remove) {
      const id = remove.dataset.mailRemove!
      if (st.mailDelete !== id) {
        st.mailDelete = id
        draw()
        return true
      }
      st.mailDelete = ''
      st.mailForm = null
      pet.mail.remove(id).then(got => ((st.snap = got), draw(), relayout()))
      return true
    }
    return false
  }

  function handleWriting(at: At) {
    if (at('[data-w-new]')) {
      write('new')
      return true
    }
    const re = at('[data-w-reply]')
    if (re) {
      closeMenu()
      const ref = re.dataset.account ? re.dataset : { account: st.reading?.account, uid: st.reading?.uid, folder: st.reading?.folder }
      // (A reply with no account of its own is the open letter's.)
      writeAbout(re.dataset.wReply!, ref.account!, Number(ref.uid), ref.folder || 'inbox')
      return true
    }
    const use = at('[data-w-use]')
    if (use && st.reading?.letter) {
      const talk = talkOf(st.reading.letter.key) || st.reading.talk
      const said = talk?.turns?.[Number(use.dataset.wUse)]?.text
      if (said) write('reply', st.reading.letter, replyIn(said))
      return true
    }
    if (at('[data-w-open]')) {
      st.writing = true
      draw(true)
      relayout()
      return true
    }
    if (!st.draft) return false
    if (at('[data-w-back]')) {
      st.writing = false
      st.dropSure = false
      st.draft.note = null
      draw(true)
      relayout()
      return true
    }
    if (at('[data-w-drop]')) {
      if (!st.dropSure) {
        st.dropSure = true
        draw()
        return true
      }
      st.draft = null
      st.writing = false
      st.dropSure = false
      keepDraft()
      draw(true)
      relayout()
      return true
    }
    if (at('[data-w-send]')) {
      sendDraft()
      return true
    }
    const from = at('[data-w-from]')
    if (from) {
      const d = st.draft
      openChoices(from, (st.snap.mail || []).map(a => [a.id, a.address]), d.account, v => {
        d.account = v
        keepDraft()
        draw()
      }, true)
      return true
    }
    if (at('[data-w-cc]')) {
      st.draft.showCc = true
      redrawMail('[data-w="cc"]')
      return true
    }
    if (at('[data-w-attach]')) {
      const input = body.querySelector('[data-w-file]')
      if (input) {
        st.picking = true
        input.click()
      }
      return true
    }
    const unfile = at('[data-w-unfile]')
    if (unfile) {
      const key = unfile.dataset.wUnfile!
      const list = key[0] === 'o' ? st.draft.forward?.files : st.draft.files
      list?.splice(Number(key.slice(1)), 1)
      st.draft.touched = true
      keepDraft()
      redrawMail()
      return true
    }
    return false
  }

  function redrawMail(focus?: string | null) {
    draw()
    relayout()
    if (focus) requestAnimationFrame(() => body.querySelector(focus)?.focus())
  }

  // The server for the address, looked up as Thunderbird does; filled in
  // when found, else the fields to fill.
  // (These are pressed in the form, or Enter in its boxes: there is one.)
  async function findServer() {
    const f = st.mailForm!
    if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(f.address.trim())) {
      f.result = { bad: true, text: T('mail.badAddress') }
      return redrawMail('[data-mf="address"]')
    }
    f.busy = 'find'
    f.result = null
    redrawMail()
    const got = await pet.mail.discover(f.address.trim(), formProxy(f))
    if (st.mailForm !== f) return
    f.busy = ''
    f.probed = null
    f.probedOut = null
    // The proxy on the way is not there, or not written right: said so.
    if (!got.ok && got.error?.kind) {
      f.result = { bad: true, text: mailError(got.error) }
      return redrawMail()
    }
    if (got.ok && got.found) {
      Object.assign(f, got.server, outFields(got.smtp), { step: 'found', source: got.source, notFound: false, oauth: false, advanced: false })
    } else {
      // Thunderbird's manual setup: the fold open, the way and port to be found.
      Object.assign(f, outFields(null), { step: 'manual', notFound: true, oauth: !!got.oauth, advanced: true, host: '', port: '', security: 'auto', auth: 'auto', username: f.username || f.address.trim() })
    }
    redrawMail(f.step === 'manual' ? '[data-mf="host"]' : null)
  }

  // The outgoing server of an account from before the pet sent mail,
  // looked up while its settings are open; filled in if none was typed.
  async function findOutgoing(f: MailForm) {
    f.findingOut = true
    const got = await pet.mail.discover(f.address, formProxy(f))
    f.findingOut = false
    if (st.mailForm !== f) return
    if (got.ok && got.smtp && !String(f.smtpHost || '').trim()) Object.assign(f, outFields(got.smtp), { smtpUser: f.username || got.smtp.username })
    redrawMail()
  }

  // Re-test: both servers, the outgoing one where there is one.
  async function retest() {
    const f = st.mailForm!
    await probeServer()
    if (st.mailForm === f && String(f.smtpHost || '').trim()) await probeOut()
  }

  // The outgoing server tried as the incoming one is below.
  async function probeOut() {
    const f = st.mailForm!
    f.busy = 'probe'
    f.probedOut = null
    redrawMail()
    const got = await pet.mail.probe(String(f.smtpHost).trim(), String(f.smtpPort ?? '').trim(), f.smtpSecurity || 'auto', 'smtp', formProxy(f))
    if (st.mailForm !== f) return false
    f.busy = ''
    if (got.ok && got.found) {
      Object.assign(f, { smtpSecurity: got.security, smtpPort: got.port })
      f.probedOut = { security: got.security, port: got.port, auths: got.auths || [] }
    } else {
      f.result = { bad: true, text: T('mail.w.outPrefix') + mailError(got.error, { host: String(f.smtpHost).trim(), port: String(f.smtpPort || '').trim() || '465 / 587', proxy: formProxy(f) }) }
    }
    redrawMail()
    return !!f.probedOut
  }

  // Re-test: the server tried without signing in; the way and port, when
  // they were to be detected, set to what answered. True when it did.
  async function probeServer() {
    const f = st.mailForm!
    if (!String(f.host || '').trim()) {
      f.result = { bad: true, text: T('mail.err.fields') }
      redrawMail('[data-mf="host"]')
      return false
    }
    f.busy = 'probe'
    f.result = null
    f.probed = null
    redrawMail()
    const got = await pet.mail.probe(String(f.host).trim(), String(f.port ?? '').trim(), f.security || 'auto', 'imap', formProxy(f))
    if (st.mailForm !== f) return false
    f.busy = ''
    if (got.ok && got.found) {
      Object.assign(f, { security: got.security, port: got.port })
      f.probed = { security: got.security, port: got.port, auths: got.auths || [] }
    } else {
      f.result = { bad: true, text: mailError(got.error, { host: String(f.host).trim(), port: String(f.port || '').trim() || '993 / 143', proxy: formProxy(f) }) }
    }
    redrawMail()
    return !!f.probed
  }

  // Signed in once, then kept; or what went wrong (and, for a Windows
  // domain's server, what the user name may be). A way or port still to be
  // detected is, first.
  async function saveAccount() {
    const f = st.mailForm!
    if (!f.id && !f.password) {
      f.result = { bad: true, text: T('mail.err.password') }
      return redrawMail('[data-mf="password"]')
    }
    // Gmail takes an app password alone: anything else (the usual Google
    // password, most likely) is not tried, which Google might hold against
    // the account.
    if (providerOf(f.host) === 'google' && f.password && !APP_PASSWORD.test(String(f.password).trim())) {
      f.result = { bad: true, text: T('mail.err.notAppPassword') }
      return redrawMail('[data-mf="password"]')
    }
    if ((f.security === 'auto' || !String(f.port ?? '').trim()) && !(await probeServer())) return
    if (st.mailForm !== f) return
    const outHost = String(f.smtpHost || '').trim()
    if (outHost && (f.smtpSecurity === 'auto' || !String(f.smtpPort ?? '').trim()) && !(await probeOut())) return
    if (st.mailForm !== f) return
    f.busy = 'save'
    f.result = null
    redrawMail()
    const username = String(f.username || f.address).trim()
    const smtp = outHost ? { host: outHost, port: Number(f.smtpPort), security: f.smtpSecurity, auth: f.smtpAuth || 'auto', username: String(f.smtpUser || username).trim() } : null
    const account = { id: f.id, address: f.address.trim(), name: String(f.name || '').trim(), host: String(f.host).trim(), port: Number(f.port), security: f.security, auth: f.auth || 'auto', username, smtp, proxy: formProxy(f) }
    // Google's app password as it shows it, in fours: the spaces left out.
    const typed = String(f.password || '')
    const password = providerOf(account.host) === 'google' && APP_PASSWORD.test(typed.trim()) ? typed.replace(/\s/g, '') : typed
    const got = await pet.mail.save(account, password)
    if (st.mailForm !== f) return
    f.busy = ''
    if (got.ok) {
      st.mailForm = null
      st.snap = got.snapshot
    } else {
      const domain = got.error?.kind === 'login' && (got.auths || []).some((a: string) => a === 'ntlm' || a === 'gssapi')
      const where = got.out ? { host: outHost, port: f.smtpPort, proxy: account.proxy } : account
      f.result = {
        bad: true,
        text: (got.out ? T('mail.w.outPrefix') : '') + mailError(got.error, where) + (domain ? ` ${T('mail.adv.domain')}` : ''),
        said: mailSaid(got.error),
        fix: mailFix(got.error, account.host),
      }
    }
    redrawMail()
  }

  layer.addEventListener('input', e => {
    if ((e.target as HTMLElement).dataset.mf && st.mailForm) st.mailForm[(e.target as HTMLElement).dataset.mf as MailBox] = (e.target as HTMLInputElement).value
    if ((e.target as HTMLElement).dataset.proxyAt !== undefined) st.proxyTyped = { key: (e.target as HTMLElement).dataset.proxyAt!, text: (e.target as HTMLInputElement).value, bad: false }
    if ((e.target as HTMLElement).dataset.w && st.draft) {
      st.draft[(e.target as HTMLElement).dataset.w as DraftBox] = (e.target as HTMLInputElement).value
      st.draft.touched = true
      st.dropSure = false
      keepDraft()
    }
    if ((e.target as HTMLElement).dataset.ilook) ilookInput(e.target as HTMLInputElement, false)
    if ((e.target as HTMLElement).id === 's-ref') {
      st.typedRef = (e.target as HTMLInputElement).value
      // Not mid-word in an input method: once the word is chosen.
      if (!e.isComposing) searchSoon()
    }
    // The market's words looked for: its list at once.
    if ((e.target as HTMLElement).id === 'mk-q') {
      market.query = (e.target as HTMLInputElement).value
      draw()
    }
    if ((e.target as HTMLElement).dataset.field) drafts[(e.target as HTMLElement).dataset.field!] = (e.target as HTMLInputElement).value
    if ((e.target as HTMLElement).dataset.mon) drafts[`mon:${(e.target as HTMLElement).dataset.mon}`] = (e.target as HTMLInputElement).value
    if ((e.target as HTMLElement).dataset.hold) drafts['hold:secs'] = (e.target as HTMLInputElement).value
  })

  // The word an input method has chosen: looked for as if typed.
  layer.addEventListener('compositionend', e => {
    if ((e.target as HTMLElement).id !== 's-ref') return
    st.typedRef = (e.target as HTMLInputElement).value
    searchSoon()
  })

  // A plugin's setting kept once typed (Enter, or leaving the box); a
  // number that is none goes back to what it was.
  layer.addEventListener('change', e => {
    if ((e.target as HTMLElement).matches('[data-w-file]')) return addFiles(e.target as HTMLInputElement)
    if ((e.target as HTMLElement).dataset.ilook) return ilookInput(e.target as HTMLInputElement, true)
    if ((e.target as HTMLElement).dataset.proxyAt !== undefined) return proxyAddress((e.target as HTMLElement).dataset.proxyAt as Way, (e.target as HTMLInputElement).value)
    if ((e.target as HTMLElement).dataset.mon) return monitorField((e.target as HTMLElement).dataset.mon!)
    if ((e.target as HTMLElement).dataset.hold) return holdTyped(e.target as HTMLInputElement)
    const key = (e.target as HTMLElement).dataset.field
    if (!key || !(key in drafts)) return
    const value = drafts[key].trim()
    delete drafts[key]
    const [id, name] = key.split('.')
    // Someone else's (its manifest's field): a number kept as one, none as
    // empty; one that is no number goes back to what it was.
    const own = addonField(id, name)
    if (own) {
      const n = Number(value)
      if (own.type === 'number' && value && !Number.isFinite(n)) return draw()
      return setPlugin(id, { [name]: own.type === 'number' && value ? n : value })
    }
    const field = ((st.snap.plugins || []).find(p => p.id === id)?.params || []).find((f: PluginParam) => f.name === name)
    if (!field || (field.kind === 'number' && value && !(Number(value) > 0))) return draw()
    setPlugin(id, { [name]: value })
  })

  // How the monitor reaches LibreHardwareMonitor, kept once typed: a port
  // that is none goes back to what it was; the password goes to Windows'
  // Credential Manager (lhm.rs keep_password), an empty one changes nothing.
  function monitorField(name: string) {
    const key = `mon:${name}`
    if (!(key in drafts)) return
    const typed = drafts[key]
    delete drafts[key]
    const m = st.snap.settings.monitor || {}
    if (name === 'lhmPort') {
      const port = typed.trim() ? Number(typed.trim()) : 8085
      if (!Number.isInteger(port) || port < 1 || port > 65535) return draw()
      return patch({ monitor: { ...m, lhmPort: port } })
    }
    if (name === 'lhmPass') return typed ? patch({ monitor: { ...m, lhmPass: typed } }) : draw()
    return patch({ monitor: { ...m, lhmUser: typed.trim().replace(/:/g, '').slice(0, 64) } })
  }

  // The seconds typed for how long endings stay, kept once Enter is pressed
  // or the box is left: rounded, within 3 to 600 (Flow.holdSecs), shown so
  // at once; no number goes back to what is kept.
  function holdTyped(box: HTMLInputElement) {
    delete drafts['hold:secs']
    const secs = Flow.holdSecs(box.value)
    const kept = st.snap.settings.hold
    box.value = String(secs ?? (Number.isInteger(kept) ? kept : ''))
    if (secs === null || secs === kept) return draw()
    patch({ hold: secs })
  }

  // Custom's box left empty, nothing custom kept: the choice kept shows again.
  layer.addEventListener('focusout', e => {
    if (!(e.target as HTMLElement).dataset?.hold) return
    delete drafts['hold:secs']
    if ((e.target as HTMLInputElement).value.trim() || Flow.holdChoice(st.snap?.settings?.hold) === 'custom') return
    st.holdCustom = false
    draw()
  })

  // A plugin's settings changed, the others kept.
  function setPlugin(id: string, change: Record<string, unknown>) {
    const all = st.snap.settings.plugins || {}
    return patch({ plugins: { ...all, [id]: { ...(all[id] || {}), ...change } } })
  }

  document.addEventListener('keydown', e => {
    if (!st.isOpen) return
    if (st.recordingShortcut) {
      e.preventDefault()
      e.stopPropagation()
      const key = st.recordingShortcut
      if (e.key === 'Escape') {
        setRecording(null)
        return draw()
      }
      if (e.key === 'Backspace' || e.key === 'Delete') {
        setRecording(null)
        return patch({ [key]: '' })
      }
      if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) return

      if (e.metaKey) {
        setRecording(null)
        toast(T('s.shortcutNoWin'))
        return draw()
      }

      if (!e.ctrlKey && !e.altKey) {
        setRecording(null)
        toast(T('s.shortcutNeedMod'))
        return draw()
      }

      const parts: string[] = []
      if (e.ctrlKey) parts.push('Ctrl')
      if (e.altKey) parts.push('Alt')
      if (e.shiftKey) parts.push('Shift')

      let mainKey = ''
      if (/^[a-zA-Z0-9]$/.test(e.key)) mainKey = e.key.toUpperCase()
      else if (/^Key([A-Z])$/.test(e.code)) mainKey = e.code.slice(3)
      else if (/^Digit([0-9])$/.test(e.code)) mainKey = e.code.slice(5)
      else if (/^F([1-9]|1[0-2])$/i.test(e.key)) mainKey = e.key.toUpperCase()
      else if (e.key === ' ' || e.code === 'Space') mainKey = 'Space'
      else if (e.key === 'Enter') mainKey = 'Enter'
      else if (e.key === 'Tab') mainKey = 'Tab'

      if (!mainKey) return
      parts.push(mainKey)
      const combo = parts.join('+')

      const s = st.snap.settings
      const defaults: Record<string, string> = { shortcutApprove: 'Alt+Y', shortcutDeny: 'Alt+N', shortcutIsland: 'Alt+W' }
      const others = ['shortcutApprove', 'shortcutDeny', 'shortcutIsland'].filter(k => k !== key)
      if (others.some(k => (s[k as ShortcutKey] ?? defaults[k]).toLowerCase() === combo.toLowerCase())) {
        setRecording(null)
        toast(T('s.shortcutDuplicate'))
        return draw()
      }

      setRecording(null)
      return patch({ [key]: combo })
    }
    if (e.key === 'Escape') {
      e.preventDefault()
      if (!menuEl.hidden) return closeMenu()
      // The question over the panel first: 取消, as its button (not while it
      // works). It left new permissions asked (approveFor) up, till 2026-11.
      if (!sheet.hidden) return cancelAsked()
      return close('Esc')
    }
    // A row that is a button (从文件安装…, 获取更多, one of the market's), by the keyboard as a press.
    if ((e.key === 'Enter' || e.key === ' ') && (e.target as HTMLElement).matches?.('[data-addon-file], [data-mk-open], [data-mk-item], [data-look-market]')) {
      e.preventDefault()
      return (e.target as HTMLElement).click()
    }
    // Asked over the panel: its buttons alone (the pages under it stay).
    if (!sheet.hidden) return
    if ((e.target as HTMLElement).matches?.('input, textarea')) {
      // Ctrl+Enter sends the letter being written.
      if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && (e.target as HTMLElement).dataset.w) {
        e.preventDefault()
        return sendDraft()
      }
      if (e.key === 'Enter' && (e.target as HTMLElement).id === 's-ref' && !e.isComposing) layer.querySelector('[data-fetch]')?.click()
      if (e.key === 'Enter' && ((e.target as HTMLElement).dataset.field || (e.target as HTMLElement).dataset.mon || (e.target as HTMLElement).dataset.hold || (e.target as HTMLElement).dataset.proxyAt !== undefined)) (e.target as HTMLElement).blur()
      if (e.key === 'Enter' && (e.target as HTMLElement).dataset.mf) layer.querySelector('[data-mail-find]:not([disabled]), [data-mail-save]:not([disabled])')?.click()
      if (e.key === 'Enter' && (e.target as HTMLElement).dataset.talk && !e.isComposing) say(e.target as HTMLInputElement)
      return
    }
    // Two panes: the letter above or below in the list.
    if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && twoPanes() && !(st.writing && st.draft) && menuEl.hidden) {
      e.preventDefault()
      return step(e.key === 'ArrowDown' ? 1 : -1)
    }
    const i = TABS.indexOf(st.tab)
    if (e.key === 'ArrowRight') {
      e.preventDefault()
      setTab(TABS[(i + 1) % TABS.length])
    }
    if (e.key === 'ArrowLeft') {
      e.preventDefault()
      setTab(TABS[(i + TABS.length - 1) % TABS.length])
    }
  })

  // (island/front.rs and stow.rs always send a tab: '' for none.)
  pet.onSettings(({ tab: next }: { tab?: string } = {}) => open(next!))
  pet.onSettingsChanged(got => {
    if (!st.isOpen) return
    st.snap = got
    draw()
    // What is kept changed (a sync): the list again, quietly.
    if (st.tab === 'mail' && inbox.for && (got.mailRev || 0) !== inbox.rev) loadInbox(false, true)
  })
  // The keyboard went elsewhere: a click outside the island; not a file
  // picker (a letter's files, the desktop's picture: its own window takes
  // the focus).
  // Locked (the button by ✕), they stay.
  pet.onBlur(() => !st.snap?.settings?.settingsPin && !st.picking && !st.pickingPaper && !st.pickingZip && !lhmAsks() && (!pet.mac || Date.now() - st.openedAt > BLUR_GRACE_MS) && close('blur'))
  layer.addEventListener('cancel', e => (e.target as HTMLElement).matches?.('[data-w-file]') && (st.picking = false), true)
  window.addEventListener('focus', () => setTimeout(() => (st.picking = false), 1000))

  return {
    isOpen: () => st.isOpen,
    layer,
    askSlot,
    // The language may change from the menu while they are open.
    redraw: () => draw(),
    close,
    // The letter to show once the Mail page opens ({ account, uid }).
    openLetter: (target: { account: string; uid: number; folder?: string }) => (st.letterToOpen = target),
    // The plugin whose settings are folded out once the plugins page opens.
    openPlugin: (id: unknown) => (st.openPlugin = String(id || '')),
  }
}
