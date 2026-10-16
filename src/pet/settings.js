// The settings, grown out of the island: one click on it opens them (and only
// then does the island take the keyboard); Esc, a click outside, ✕ or the head
// close them. Five pages:
//   now       General: the welcome (nothing connected yet) or a line on the
//             agents connected, the sessions (a click on one goes to its
//             window), four quick switches; the alerts (how long endings
//             stay, idle sessions, notifications, sound, prompts, quiet),
//             start at login and the language, her updates, crash reports, about
//   agents    the agents she follows (agents.js): each found here a row (how
//             it stands, what she can do for it, connect, repair, remove),
//             opened in place to its details; the others to get
//   look      her home (island, taskbar), size, strolls, eyes; Windows' own
//             mode and accent colour; her and the pets (downloaded, the gallery)
//   widgets   plugins: the built-in ones, those she runs for you (a switch, their settings folded under), their order, turns, nudges, waku, and how to write one
//   mail      mail accounts, and setting one up as Thunderbird does: an address and a password, the server found or typed
//
// What they show comes from main as a snapshot, again whenever it changes;
// what they change goes back as a patch, checked there. island.js decides the
// island's shape (it is 'settings' while these are open) and places her.
//
// Everything that came from elsewhere (session names, pet names, errors) is
// escaped before it goes into the page.
;(function () {
  const ROLE = new URLSearchParams(location.search).get('role') === 'island' ? 'island' : 'pet'
  if (ROLE !== 'island') {
    window.Settings = { isOpen: () => false }
    return
  }

  const { t, list: nameList, render } = window.I18n
  const { CLIPS } = window.Sprite
  const Status = window.Status
  const Widgets = window.Widgets
  const Agents = window.Agents

  // The island's colours (on black), the same as island.js.
  const COLOR = { idle: '#8e8e93', working: '#5e9bff', waiting: '#ffb340', done: '#34d27b', review: '#b18cff', error: '#ff5c6c' }
  // Five pages: General (the sessions, the alerts, startup and the
  // language), Agent (connecting them, right after General), Look (her
  // home, Windows, her and the pets), Plugins and Mail. The pets, the alerts
  // and connecting were pages of their own: asked for by those names
  // (island.rs asks for the pets, or connecting), the page they are part of
  // opens, scrolled to them (PART_OF, showPart); connecting while the
  // welcome shows is the welcome, at General's top.
  const TABS = ['now', 'agents', 'look', 'widgets', 'mail']
  const TAB_KEY = { now: 's.tabGeneral', agents: 's.tabAgents', look: 's.tabLook', widgets: 's.tabWidgets', mail: 's.tabMail' }
  const PART_OF = { pets: 'look', alerts: 'now', connect: 'agents', update: 'now', crash: 'now' }
  const HEAD_KEY = { working: 's.headWorking', waiting: 's.headWaiting', done: 's.headDone', review: 's.headReview', error: 's.headError' }
  const SIZES = [['small', 0.4], ['medium', 0.55], ['large', 0.75]]
  const HOMES = ['island', 'taskbar']
  const HOME_KEY = { island: 's.displayIsland', taskbar: 's.displayTaskbar' }
  // How long endings stay: until seen, the quick choices, and Custom, whole
  // seconds typed in a box beside them (flow.js holdSecs, #59).
  const HOLDS = ['seen', ...window.Flow.HOLD_QUICK]
  // Minutes a session at rest stays in sight; 0 always (state.rs is_shown).
  const IDLE_HIDES = [15, 30, 60, 120, 0]
  const WAITS = [30, 60, 120, 290]
  const SPINS = [0, 5, 8, 15]
  const ICON = {
    eye: '<path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>',
    moon: '<path d="M20 14A8 8 0 1 1 10 4a7 7 0 0 0 10 10z"/>',
    sound: '<path d="M4 9v6h4l5 4V5L8 9H4z"/><path d="M16 9a4 4 0 0 1 0 6"/>',
    pill: '<rect x="3" y="8" width="18" height="8" rx="4"/>',
  }

  const layer = document.getElementById('island-settings')
  let isOpen = false
  // When they last opened: on a Mac the loss of focus that comes with taking
  // the keyboard is not a click outside (opened from her menu, the menu going
  // away gave a blur at once, and they shut as they opened, 2026-10-14). On
  // Windows a blur closes them at once, as ever.
  let openedAt = 0
  const BLUR_GRACE_MS = 500
  let tab = 'now'
  let snap = null
  let lang = 'en'
  // The crash report shown, as it would be sent (pageCrash).
  let crashShown = false
  // The download box: what is typed, and how the last download went.
  let typedRef = ''
  let fetching = null
  let fetchNote = { text: '', isError: false }
  // The gallery, loaded the first time the pets page is shown; or what a
  // search finds there, the words typed in the download box (a link there
  // is downloaded, any other words are looked for as the site's own search
  // box looks: a name, part of one, a word of its description). Each ask
  // numbered: a slower answer to an earlier search never lands over a later.
  const gallery = { sort: 'popular', query: '', items: [], page: 0, totalPages: 1, total: 0, loading: false, error: '', getting: '', asked: 0 }
  let searchTimer
  const isLink = text => /:\/\/|codex-pets\.net/i.test(text)
  let thumbTimers = []
  let clockTimer
  // The plugins page: the plugin whose settings are folded out, and what is
  // typed in them and not yet kept ("weather.City": "上海").
  let openPlugin = ''
  const drafts = {}
  // Custom picked for how long endings stay, its box shown before anything
  // is typed in it (until the settings close or another choice is picked).
  let holdCustom = false
  // The monitor's hardware (LibreHardwareMonitor's) folded out, by its id.
  const openHardware = new Set()
  // The mail account being set up or changed, as Thunderbird's dialog has
  // it: the address and password; then (step found) the server looked up,
  // or (step manual) the server's fields; busy while looking or signing in;
  // what came of the last try. Null while none is.
  let mailForm = null
  // An account's delete asked once: asked again, it goes.
  let mailDelete = ''
  // The Mail page's proxy address as typed and not yet kept: being typed,
  // or (bad) not read as address:port.
  let proxyTyped = null
  // The Mail page's inbox: whose (an account id), its newest letters and how
  // many there are, how many asked for, busy, what went wrong. The letter
  // open ({ uid, busy | letter, summary | error }), null while the list
  // shows. What came of handing one to an agent, for a few seconds.
  // threads: its conversations as kept (threads.rs), each with its letters;
  // total: how many there are; letters: how many letters the inbox holds;
  // rev: the store's revision they are of (a newer one, asked again);
  // syncing: a first sync still under way. The conversations folded out.
  const inbox = { id: '', for: '', filter: 'all', threads: [], total: 0, letters: 0, count: 50, busy: false, error: null, errors: [], rev: 0, syncing: false }
  const expanded = new Set()
  // A letter to open once the Mail page shows (a click on the island's word that an agent answered).
  let letterToOpen = null
  let reading = null
  let handNote = null
  let handNoteTimer
  // The agents that read mail (agents.js mail): their short names, and
  // their names on the Mail page's settings.
  const AGENTS = Object.fromEntries(Agents.TABLE.filter(a => a.mail).map(a => [a.id, a.mail]))
  const AGENT_NAMES = Object.fromEntries(Agents.TABLE.filter(a => a.mail).map(a => [a.id, a.name]))
  // The letter being written, one at a time: from which account, to whom,
  // the subject and text, the letter it answers or passes on, files added;
  // touched once typed in; sending, what went wrong. Kept until sent or
  // thrown away, across a restart too (in the data folder, mail.rs
  // mail_draft: each copy of the pet its own, which the page's storage is
  // not; its files left out). writing: it shows; else the inbox does, with
  // a line to go back to it. A discard asked once: asked again, it goes.
  let draft = null
  let writing = false
  let dropSure = false
  let draftTimer
  // The file picker is open for a letter's files: the focus it takes does
  // not close the settings (the desktop's picture has its own, pickingPaper).
  let picking = false
  // The most a letter takes with it (as send.rs has it).
  const MOST_FILES = 25 << 20

  // The letter left being written, as kept (none begun meanwhile).
  window.pet.mail.draft().then(d => {
    if (draft || !d || typeof d !== 'object' || !d.account) return
    draft = { ...d, files: [], busy: false, error: null, note: null }
    if (isOpen) draw()
  })

  function keepDraft() {
    clearTimeout(draftTimer)
    draftTimer = setTimeout(() => window.pet.mail.keepDraft(draft ? { ...draft, files: [], busy: false, error: null, note: null } : null), 300)
  }

  const T = (key, vars) => t(lang, key, vars)
  const esc = value =>
    String(value ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c])
  const svg = name => `<svg class="ic" viewBox="0 0 24 24">${ICON[name]}</svg>`

  layer.innerHTML = `
    <div class="s-head" data-close></div>
    <div class="s-tabs"><span class="ind"></span>${TABS.map(k => `<button data-tab="${k}"><span></span><i class="n" hidden></i></button>`).join('')}</div>
    <div class="s-ask"></div>
    <div class="s-body"></div>
    <div class="s-foot"><span class="foot-text"></span><span class="grow"></span><span class="ver"></span></div>`
  const head = layer.querySelector('.s-head')
  const tabs = layer.querySelector('.s-tabs')
  tabs.style.setProperty('--tabs', TABS.length)
  const askSlot = layer.querySelector('.s-ask')
  const body = layer.querySelector('.s-body')
  const foot = layer.querySelector('.s-foot')

  // The new markup, changed in place: only what differs is touched, so a
  // number that changed (a plugin's reading) does not redraw the page around
  // it, and a box being typed in keeps its focus and caret.
  function setHTML(el, html) {
    if (el._html === html) return false
    el._html = html
    const next = document.createElement('template')
    next.innerHTML = html
    morphChildren(el, next.content)
    return true
  }

  function morph(from, to) {
    if (from.nodeType !== to.nodeType || from.nodeName !== to.nodeName) return from.replaceWith(to.cloneNode(true))
    if (from.nodeType !== Node.ELEMENT_NODE) {
      if (from.nodeValue !== to.nodeValue) from.nodeValue = to.nodeValue
      return
    }
    for (const { name } of [...from.attributes]) if (!to.hasAttribute(name)) from.removeAttribute(name)
    for (const { name, value } of to.attributes) if (from.getAttribute(name) !== value) from.setAttribute(name, value)
    // A box's text, once typed in, is its own: set while it is not being typed in.
    if (from.nodeName === 'INPUT' && from !== document.activeElement && from.value !== (to.getAttribute('value') ?? '')) from.value = to.getAttribute('value') ?? ''
    morphChildren(from, to)
    if (from.nodeName === 'TEXTAREA' && from !== document.activeElement && from.value !== to.textContent) from.value = to.textContent
  }

  function morphChildren(from, to) {
    const have = [...from.childNodes]
    const want = [...to.childNodes]
    want.forEach((node, i) => (i < have.length ? morph(have[i], node) : from.appendChild(node.cloneNode(true))))
    for (const extra of have.slice(want.length)) extra.remove()
  }

  function clock(ms) {
    const s = Math.max(0, Math.floor(ms / 1000))
    const h = Math.floor(s / 3600)
    const m = Math.floor((s % 3600) / 60)
    const sec = String(s % 60).padStart(2, '0')
    return h ? `${h}:${String(m).padStart(2, '0')}:${sec}` : `${m}:${sec}`
  }

  // How long a turn has run, or how long the finished one took.
  function timeOf(s) {
    if ((s.mood === 'working' || s.mood === 'waiting') && s.since) return clock(Date.now() - s.since)
    if ((s.mood === 'done' || s.mood === 'review' || s.mood === 'error') && s.took) return clock(s.took)
    return ''
  }

  // A clock as markup that tick() keeps current, so a redraw is not needed for it.
  const clockTag = (s, cls = 'clock') =>
    `<span class="${cls}" data-mood="${esc(s.mood)}" data-since="${s.since || ''}" data-took="${s.took || ''}" style="color:${COLOR[s.mood] || COLOR.idle}">${timeOf(s)}</span>`

  function tick() {
    for (const c of layer.querySelectorAll('[data-mood][data-since]')) {
      c.textContent = timeOf({ mood: c.dataset.mood, since: Number(c.dataset.since) || null, took: Number(c.dataset.took) || null })
    }
  }

  // --- Small parts -------------------------------------------------------------------

  const sw = (key, on, extra = '') => `<button class="sw${on ? ' on' : ''}" data-sw="${key}" ${extra}></button>`
  // A few choices side by side; one marked off ([value, label, true]) is
  // there, greyed, and can't be picked.
  const seg = (key, options, current) =>
    `<span class="seg" data-seg="${key}">${options
      .map(([value, label, off]) => `<span data-value="${esc(value)}" class="${String(value) === String(current) ? 'on' : off ? 'off' : ''}"${off ? ' aria-disabled="true"' : ''}>${esc(label)}</span>`)
      .join('')}</span>`
  const row = (title, note, control) =>
    `<div class="r"><div class="grow"><div>${title}</div>${note ? `<div class="d">${note}</div>` : ''}</div>${control}</div>`
  const sec = text => `<div class="sec">${esc(text)}</div>`
  // Where a part of a page begins, to scroll to (PART_OF).
  const part = (id, html) => `<i class="part" data-part="${id}"></i>${html}`

  function thumb(px, url, version, clip = 'idle') {
    const s = px / 192
    const sheetH = version === 1 ? 1872 : 2288
    return `<div class="thumb" data-clip="${clip}" style="width:${px}px;height:${Math.round(208 * s)}px"><div class="sheet" style="background-image:url(&quot;${esc(url)}&quot;);background-size:1536px ${sheetH}px;transform:scale(${s})"></div></div>`
  }

  function startThumbs() {
    for (const timer of thumbTimers) clearTimeout(timer)
    thumbTimers = []
    for (const el of layer.querySelectorAll('.thumb')) {
      const { row: r, frames, ms } = CLIPS[el.dataset.clip] || CLIPS.idle
      const sheet = el.querySelector('.sheet')
      let frame = 0
      const step = () => {
        sheet.style.backgroundPosition = `${-frame * 192}px ${-r * 208}px`
        frame = (frame + 1) % frames
        const timer = setTimeout(step, ms)
        thumbTimers.push(timer)
      }
      step()
    }
  }

  // --- The pages -------------------------------------------------------------------------

  function pageNow() {
    const s = snap.settings
    const sessions = snap.sessions || []
    // Each session: its name, and where it is (or how its turn ended).
    const detailed = s.details !== false
    const list = sessions.length
      ? sessions
          .map(x => {
            const name = Status.nameOf(x, detailed)
            const what = [
              (x.agent && Agents.byId(x.agent)?.name) || '',
              Status.status(lang, x, { detailed, withClock: false }),
              detailed && x.name && x.project !== name ? x.project : '',
              detailed && Status.isEnding(x) ? x.reply : '',
            ].filter(Boolean)
            // One with a window to go to: a click on it goes there.
            const jump = x.jump ? ` data-jump="${esc(x.id)}"` : ''
            const tip = [...what, x.jump ? T('jump.hint') : ''].filter(Boolean).join('\n')
            return `<div class="r${x.jump ? ' jumps' : ''}"${jump} title="${esc(tip)}"><span class="dot" style="background:${COLOR[x.mood] || COLOR.idle}"></span><div class="grow"><div class="ellip">${esc(name || '—')}</div><div class="d ellip">${esc(what.join(' · '))}</div></div>${clockTag(x)}</div>`
          })
          .join('')
      : `<div class="note">${esc(T('home.noSessions', { list: toolNames(toolsHere()) }))}</div>`
    // Her out on the desktop; and her home, the next one at a press.
    const tiles = [
      ['show', 'eye', T('s.outTile'), !snap.hidden && s.out === true],
      ['dnd', 'moon', T('menu.dnd'), s.dnd],
      ['sound', 'sound', T('settings.sound'), s.sound],
      ['home', 'pill', T(HOME_KEY[s.display] || 's.displayIsland'), true],
    ]
    return `${sec(T('home.sessions'))}<div class="grp">${list}</div>
      ${sec(T('s.quick'))}<div class="tiles">${tiles
        .map(([key, icon, label, on]) => `<div class="tile${on ? ' on' : ''}" data-tile="${key}"><span class="c">${svg(icon)}</span>${esc(label)}</div>`)
        .join('')}</div>`
  }

  function pagePets() {
    const current = snap.settings.pet
    const installed = snap.pets.length
      ? snap.pets
          .map(
            p => `<div class="r petrow">${thumb(52, p.url, p.version)}<div class="grow"><div>${esc(p.name)}</div><div class="d">${esc([p.author, p.codex ? T('s.fromCodex') : '', p.version === 1 ? T('home.v1') : ''].filter(Boolean).join(' · '))}</div></div>
            ${p.id === current ? `<span class="d in-use">${esc(T('home.inUse'))}</span>` : `<button class="pbtn" data-use="${esc(p.id)}">${esc(T('home.use'))}</button>`}</div>`,
          )
          .join('')
      : `<div class="note">${esc(T('settings.petsEmpty'))}</div>`
    const note = fetchNote.text ? `<div class="note${fetchNote.isError ? ' err' : ''}">${esc(fetchNote.text)}</div>` : `<div class="note">${esc(T('settings.fetchNote'))}</div>`
    const have = new Set(snap.pets.map(p => p.id))
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
    const fetchLabel = fetching ? T('settings.fetching') : T(isLink(typedRef.trim()) ? 'settings.fetch' : 's.search')
    const q = gallery.query
    const listTitle = !q ? T('home.gallery') : gallery.loading && !gallery.items.length ? T('s.searching', { q }) : T('s.found', { q, n: gallery.total })
    const none = q && !gallery.loading && !gallery.error && !gallery.items.length ? `<div class="note">${esc(T('s.foundNone', { q }))}</div>` : ''
    return `${sec(T('home.installed'))}<div class="grp">${installed}</div>
      ${sec(T('s.download'))}<div class="grp"><div class="r"><input class="in" id="s-ref" spellcheck="false" placeholder="${esc(T('s.fetchPlaceholder'))}"><button class="pbtn al" data-fetch ${fetching ? 'disabled' : ''}>${esc(fetchLabel)}</button></div>${note}</div>
      <div class="sec row-sec"><span class="ellip">${esc(listTitle)}</span><span class="grow"></span>${seg('gallery', [['popular', T('home.popular')], ['newest', T('home.newest')]], gallery.sort)}</div>
      ${gallery.error ? `<div class="note err">${esc(T('home.galleryError', { message: gallery.error }))}</div>` : ''}${none}
      <div class="cards">${cards}</div>${more}
      <div class="links"><span class="link" data-site="site">${esc(T('settings.browse'))} ›</span></div>`
  }

  // --- Her island's look (island.js drawIslandLook) ---------------------------------
  // By Windows' mode: a preset, or its parts one by one, over Windows' own
  // flyout grey (none set: that grey). On the taskbar only where her end is
  // a capsule (the Mica, clear or light strip); on the black one she is part
  // of it. Folded out under the taskbar's options, or the island's at the
  // top of the screen (its own look, ITOP_*). Open, the settings are the
  // island itself, so a small one here shows the look (hovered, it opens).
  const ILOOK_DEFAULT = {
    // Bare in the dark mode, a new user's since 2026-10-13 (the person's own
    // look; island.js drawIslandLook takes it as the default too).
    dark: { bg: '#2c2c2c', op: 100, blur: 0, edge: 'line', bare: true },
    light: { bg: '#f3f3f3', op: 100, blur: 0, edge: 'line', bare: false },
  }
  // Each preset its own look: the dark grey was the default's, the bare look
  // once that became the default, and a press on the bare one lit the grey
  // (zhoupengjie/Wakuwaku#17).
  const ILOOK_PRESETS = {
    dark: [
      ['black', { bg: '#000000', op: 100, blur: 0, edge: 'line', bare: false }],
      ['gray', { bg: '#2c2c2c', op: 100, blur: 0, edge: 'line', bare: false }],
      ['mica', { bg: '#242424', op: 62, blur: 40, edge: 'line', bare: false }],
      ['ring', { bg: '#2c2c2c', op: 100, blur: 0, edge: 'ring', bare: false }],
      ['bare', { bg: '#2c2c2c', op: 100, blur: 0, edge: 'line', bare: true }],
    ],
    light: [
      ['white', { bg: '#fbfbfb', op: 100, blur: 0, edge: 'line', bare: false }],
      ['gray', { bg: '#f3f3f3', op: 100, blur: 0, edge: 'line', bare: false }],
      ['mica', { bg: '#f6f6f6', op: 62, blur: 40, edge: 'line', bare: false }],
      ['ring', { bg: '#fbfbfb', op: 100, blur: 0, edge: 'ring', bare: false }],
      ['bare', { bg: '#f3f3f3', op: 100, blur: 0, edge: 'line', bare: true }],
      ['black', { bg: '#000000', op: 100, blur: 0, edge: 'none', bare: false }],
    ],
  }
  // At the top of the screen its own look, kept apart (islandTopLook,
  // zhoupengjie/Wakuwaku#27): black by default in either mode, as it always
  // was (the iPhone's, its faint line, white words), so nobody's changes;
  // no bare capsule (no strip to be part of up there), and no 「云母毛玻璃」
  // where Windows' acrylic cannot go under it (nothing else would blur).
  const ITOP_DEFAULT = {
    dark: { bg: '#000000', op: 100, blur: 0, edge: 'line', bare: false },
    light: { bg: '#000000', op: 100, blur: 0, edge: 'line', bare: false },
  }
  const ITOP_PRESETS = {
    dark: [
      ['black', { bg: '#000000', op: 100, blur: 0, edge: 'line', bare: false }],
      ['gray', { bg: '#2c2c2c', op: 100, blur: 0, edge: 'line', bare: false }],
      ['mica', { bg: '#242424', op: 62, blur: 40, edge: 'line', bare: false }],
      ['ring', { bg: '#2c2c2c', op: 100, blur: 0, edge: 'ring', bare: false }],
    ],
    light: [
      ['black', { bg: '#000000', op: 100, blur: 0, edge: 'line', bare: false }],
      ['gray', { bg: '#f3f3f3', op: 100, blur: 0, edge: 'line', bare: false }],
      ['white', { bg: '#fbfbfb', op: 100, blur: 0, edge: 'line', bare: false }],
      ['mica', { bg: '#f6f6f6', op: 62, blur: 40, edge: 'line', bare: false }],
      ['ring', { bg: '#fbfbfb', op: 100, blur: 0, edge: 'ring', bare: false }],
    ],
  }
  // The moods' colours on a light ground, as island.js has them (COLOR_LIGHT).
  const COLOR_DEEP = { idle: '#6e6e73', working: '#0067c0', waiting: '#b25e00', done: '#0f7b3f', review: '#7346c9', error: '#c42b3c' }
  // Grounds to pick for each mode: as dark (or light) as its words need.
  const ILOOK_SWATCHES = {
    dark: ['#000000', '#1c1c1c', '#202020', '#2c2c2c', '#3a3a3a', '#1f2733', '#2a2433', '#1f2d27', '#33271f'],
    light: ['#ffffff', '#fbfbfb', '#f3f3f3', '#e8e8e8', '#dddddd', '#eef3fb', '#f5effa', '#eef7f1', '#fbf3ec'],
  }
  let ilookOpen = false
  // The mode being set: '' follows Windows' own.
  let ilookMode = ''
  const winMode = () => (snap.winLook?.light ? 'light' : 'dark')
  const ilookFor = () => ilookMode || winMode()
  // Her home's: the taskbar's, or the top of the screen's.
  const isTop = () => snap.settings.display !== 'taskbar'
  const ilookKey = () => (isTop() ? 'islandTopLook' : 'islandLook')
  const ilookDefault = mode => (isTop() ? ITOP_DEFAULT : ILOOK_DEFAULT)[mode]
  const ilookPresets = mode => (isTop() ? ITOP_PRESETS[mode].filter(([id]) => id !== 'mica' || snap.plate === true) : ILOOK_PRESETS[mode])
  const ilookOf = mode => ({ ...ilookDefault(mode), ...(snap.settings[ilookKey()]?.[mode] || {}) })
  const sameLook = (a, b) => a.bg.toLowerCase() === b.bg.toLowerCase() && a.op === b.op && a.blur === b.blur && a.edge === b.edge && !!a.bare === !!b.bare
  const ilookPreset = mode => (ilookPresets(mode).find(([, p]) => sameLook(p, ilookOf(mode))) || ['custom'])[0]
  const ilookName = (mode, id) => T(id === 'custom' ? 's.ilookCustom' : isTop() && id === 'black' ? 's.ilookP.top.black' : `s.ilookP.${mode}.${id}`)

  // A mode's look kept (its default kept as none); while a slider moves,
  // only shown here, kept once it is let go.
  function ilookSave(mode, look, keep = true) {
    const key = ilookKey()
    const all = { ...(snap.settings[key] || {}) }
    all[mode] = sameLook(look, ilookDefault(mode)) ? null : look
    if (keep) return patch({ [key]: all })
    snap.settings[key] = all
    draw()
  }

  // A colour's relative luminance (WCAG), as island.js judges a ground:
  // above 0.35, the words on it go dark.
  function luminance(hex) {
    const [r, g, b] = [1, 3, 5].map(i => {
      const v = parseInt(hex.slice(i, i + 2), 16) / 255
      return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4
    })
    return 0.2126 * r + 0.7152 * g + 0.0722 * b
  }
  const inkDarkOn = hex => luminance(hex) > 0.35

  function toHsl(hex) {
    const [r, g, b] = [1, 3, 5].map(i => parseInt(hex.slice(i, i + 2), 16) / 255)
    const max = Math.max(r, g, b)
    const min = Math.min(r, g, b)
    const l = (max + min) / 2
    const d = max - min
    const s = d === 0 ? 0 : d / (1 - Math.abs(2 * l - 1))
    const h = d === 0 ? 0 : max === r ? ((g - b) / d + 6) % 6 : max === g ? (b - r) / d + 2 : (r - g) / d + 4
    return [h, s, l]
  }

  function fromHsl(h, s, l) {
    const c = (1 - Math.abs(2 * l - 1)) * s
    const x = c * (1 - Math.abs((h % 2) - 1))
    const [r, g, b] = [[c, x, 0], [x, c, 0], [0, c, x], [0, x, c], [x, 0, c], [c, 0, x]][Math.floor(h) % 6]
    const m = l - c / 2
    return `#${[r, g, b].map(v => Math.round(Math.min(1, Math.max(0, v + m)) * 255).toString(16).padStart(2, '0')).join('')}`
  }

  // A ground picked, any colour: the words on it go light or dark as it is
  // (island.js), so only a mid one reads poorly either way (between 0.18
  // and 0.36, under 4.5 to 1 with white words, and dark ones not yet):
  // moved out to the nearer side, its hue kept.
  function ilookSteer(hex) {
    hex = hex.toLowerCase()
    const y = luminance(hex)
    if (y <= 0.18 || y >= 0.36) return hex
    const [h, s, l] = toHsl(hex)
    const darker = y - 0.18 < 0.36 - y
    let [lo, hi] = darker ? [0, l] : [l, 1]
    // The lightness where it crosses out of the middle, a little past it.
    for (let i = 0; i < 24; i++) {
      const mid = (lo + hi) / 2
      const y2 = luminance(fromHsl(h, s, mid))
      if (darker) {
        if (y2 <= 0.175) lo = mid
        else hi = mid
      } else if (y2 >= 0.365) hi = mid
      else lo = mid
    }
    return fromHsl(h, s, darker ? lo : hi)
  }

  // The island as the look makes it, on a strip over a desktop; big, it
  // opens under the pointer (open, above the strip, the page has nothing
  // behind it to blur: its ground alone, as on the taskbar; blurred where
  // Windows' acrylic goes under it, island.js placePlate). At the top of
  // the screen, a pill at the top of a desktop, opening down; blurred only
  // by that acrylic, open or not.
  function ilookPreview(mode, look, big = false) {
    const [r, g, b] = [1, 3, 5].map(i => parseInt(look.bg.slice(i, i + 2), 16))
    // Its words, and the moods' colours, as light or dark as its ground is.
    const inkDark = inkDarkOn(look.bg)
    const moods = inkDark ? COLOR_DEEP : COLOR
    const ring = moods[snap.now?.mood] || moods.working
    const top = isTop()
    const line = top ? (inkDark ? 'rgba(0,0,0,0.08)' : 'rgba(255,255,255,0.07)') : mode === 'dark' ? 'rgba(255,255,255,0.1)' : 'rgba(0,0,0,0.08)'
    const edge = look.edge === 'ring' ? `0 0 0 1.5px ${ring}, 0 0 12px ${ring}59` : look.edge === 'none' ? '0 0 0 0 transparent' : `0 0 0 1px ${line}`
    const plate = snap.plate === true && look.op < 100 && look.blur > 0
    const blur = look.blur && (plate || !top) ? `blur(${look.blur}px) saturate(1.25)` : 'none'
    const style = `--pv-bg:rgba(${r},${g},${b},${look.op / 100});--pv-blur:${blur};--pv-edge:${edge};--pv-ring:${ring}`
    const more = big ? `<span class="more"><span>${esc(T('s.ilookSample2'))}</span><span>${esc(T('s.ilookSample3'))}</span></span>` : ''
    return `<div class="ilook-pv ${mode}${inkDark ? ' ink-dark' : ''}${look.bare ? ' bare' : ''}${big ? ' big' : ''}${plate ? ' plate' : ''}${top ? ' top' : ''}" style="${style}"><span class="cap"><span class="top"><i class="face"></i><span class="w">${esc(T('s.ilookSample'))}</span><span class="t">1:42</span></span>${more}</span></div>`
  }

  function ilookPanel() {
    const s = snap.settings
    const mode = ilookFor()
    const look = ilookOf(mode)
    const active = ilookPreset(mode)
    const top = isTop()
    // On the black strip in the dark, she is part of it: none of this
    // shows. At the top of the screen, its own look.
    const isCapsule = (s.taskbarMaterial || 'clear') !== 'black' || winMode() === 'light'
    const note = top ? T('s.ilookTop') : isCapsule ? '' : T('s.ilookBlack')
    const cards = ilookPresets(mode)
      .map(([id, p]) => `<button class="ilook-card${id === active ? ' on' : ''}" data-ilook-preset="${id}">${ilookPreview(mode, p)}<span>${esc(ilookName(mode, id))}</span></button>`)
      .join('')
    const swatches = ILOOK_SWATCHES[mode]
      .map(c => `<button class="swc${c === look.bg.toLowerCase() ? ' on' : ''}" data-ilook-bg="${c}" style="background:${c}" title="${c}"></button>`)
      .join('')
    const picker = `<span class="ilook-sw">${swatches}<input type="color" class="swc-in" data-ilook="bg" value="${esc(look.bg)}" title="${esc(T('s.ilookPick'))}"></span>`
    const range = (key, min, max, unit) => `<span class="ilook-rng"><input type="range" class="rng" data-ilook="${key}" min="${min}" max="${max}" step="1" value="${look[key]}"><span class="rv">${look[key]}${unit}</span></span>`
    // At the top no bare capsule, and a blur only where Windows' acrylic
    // can go under the island.
    const blurRow = top && snap.plate !== true ? '' : row(esc(T('s.ilookBlur')), esc(T(top ? 's.ilookBlurTopNote' : 's.ilookBlurNote')), range('blur', 0, 60, 'px'))
    return `<div class="ilook">
        ${note ? `<div class="note">${esc(note)}</div>` : ''}
        <div class="ilook-top">${seg('ilookMode', [['dark', T('s.ilookDark')], ['light', T('s.ilookLight')]], mode)}<span class="grow"></span><button class="pbtn sm" data-ilook-reset ${sameLook(look, ilookDefault(mode)) ? 'disabled' : ''}>${esc(T('s.ilookReset'))}</button></div>
        ${mode === winMode() ? '' : `<div class="note">${esc(T('s.ilookOther'))}</div>`}
        ${ilookPreview(mode, look, true)}
        <div class="ilook-cards">${cards}</div>
        ${row(esc(T('s.ilookBg')), esc(T('s.ilookBgNote')), picker)}
        ${row(esc(T('s.ilookOp')), '', range('op', 20, 100, '%'))}
        ${blurRow}
        ${row(esc(T('s.ilookEdge')), '', seg('ilookEdge', ['none', 'line', 'ring'].map(e => [e, T(`s.ilookEdge.${e}`)]), look.edge))}
        ${top ? '' : row(esc(T('s.ilookBare')), esc(T('s.ilookBareNote')), sw('ilookBare', look.bare))}
      </div>`
  }

  // A slider moved (shown here) or let go (kept); a colour picked, moved
  // out of the greys no words read on.
  function ilookInput(target, keep) {
    const mode = ilookFor()
    const key = target.dataset.ilook
    const value = key === 'bg' ? ilookSteer(target.value) : Number(target.value)
    if (key === 'bg' && value !== target.value.toLowerCase()) target.value = value
    ilookSave(mode, { ...ilookOf(mode), [key]: value }, keep)
  }

  // The taskbar's layout (island.js tbLayout): a preset, or one dragged
  // into shape on the strip itself (the button: the settings close for it).
  function layoutRow() {
    const preset = window.Island?.tbPreset?.() || 'win11'
    const presets = ['win11', 'left', 'herRight'].map(p => [p, T(`s.layout.${p}`)])
    const note = T(preset === 'custom' ? 's.layoutCustom' : 's.layoutNote')
    return row(esc(T('s.taskbarLayout')), esc(note), `<span class="btns">${seg('taskbarLayout', presets, preset)}<button class="pbtn sm" data-tb-edit>${esc(T('s.layoutEdit'))}</button></span>`)
  }

  // At the top of the screen, up out of the way while a window lies under
  // it (island.js setStow, zhoupengjie/Wakuwaku#49): on unless turned off.
  // Not on a Mac, where it hangs over the menu bar.
  function hideUnderRow() {
    return row(esc(T('s.islandHideUnder')), esc(T('s.islandHideUnderNote')), sw('islandHideUnder', snap.settings.islandHideUnder !== false))
  }

  // The fold's own row: how the island looks in Windows' mode now.
  function ilookRow() {
    const mode = winMode()
    const now = T('s.ilookNow', { mode: T(mode === 'dark' ? 's.ilookDark' : 's.ilookLight'), name: ilookName(mode, ilookPreset(mode)) })
    return `${row(esc(T('s.ilook')), esc(now), `<button class="pbtn sm" data-ilook-fold>${esc(T(ilookOpen ? 's.ilookFold' : 's.ilookAdjust'))}</button>`)}${ilookOpen ? ilookPanel() : ''}`
  }

  function pageLook() {
    const s = snap.settings
    return `${sec(T('home.display'))}<div class="modes">
        <div class="mode${s.display === 'taskbar' ? '' : ' on'}" data-mode="island"><div class="pv"><i></i></div><div class="l"><span class="rd"></span>${esc(T('s.displayIsland'))}</div></div>
        <div class="mode${s.display === 'taskbar' ? ' on' : ''}" data-mode="taskbar"><div class="pv"><s></s></div><div class="l"><span class="rd"></span>${esc(T('s.displayTaskbar'))}</div></div>
      </div>
      ${s.display === 'taskbar' ? `<div class="grp"><div class="note">${esc(T('s.taskbarNote'))}</div></div><div class="grp">${row(esc(T('s.taskbarEdge')), esc(T('s.taskbarEdgeNote')), seg('taskbarEdge', ['bottom', 'top', 'left', 'right'].map(e => [e, T(`s.edge.${e}`)]), s.taskbarEdge || 'bottom'))}${displaysRow()}${row(esc(T('s.taskbarMaterial')), esc(T('s.taskbarMaterialNote')), seg('taskbarMaterial', ['mica', 'black', 'clear'].map(m => [m, T(`s.material.${m}`)]), s.taskbarMaterial || 'mica'))}${s.taskbarMaterial === 'clear' ? row(esc(T('s.clearWhen')), esc(T('s.clearWhenNote')), seg('taskbarClearWhen', ['mica', 'solid', 'always'].map(w => [w, T(`s.clearWhen.${w}`)]), ['solid', 'always'].includes(s.taskbarClearWhen) ? s.taskbarClearWhen : 'mica')) : ''}${['left', 'right'].includes(s.taskbarEdge) ? row(esc(T('s.taskbarButtons')), esc(T('s.taskbarButtonsSide')), `<span class="seg off">${['icons', 'labels'].map(b => `<span class="${b === 'icons' ? 'on' : ''}">${esc(T(`s.buttons.${b}`))}</span>`).join('')}</span>`) : row(esc(T('s.taskbarButtons')), esc(T('s.taskbarButtonsNote')), seg('taskbarButtons', ['icons', 'labels'].map(b => [b, T(`s.buttons.${b}`)]), s.taskbarButtons || 'icons'))}${layoutRow()}${ilookRow()}</div>` : ''}
      <div class="grp">${row(esc(T('s.islandWidth')), esc(T('s.islandWidthNote')), seg('islandWidth', ['narrow', 'normal', 'wide'].map(w => [w, T(`s.width.${w}`)]), s.islandWidth || 'normal'))}${s.display === 'taskbar' || window.pet.mac ? '' : hideUnderRow()}${row(esc(T('s.oledShift')), esc(T('s.oledShiftNote')), sw('oledShift', s.oledShift))}${s.display === 'taskbar' ? '' : ilookRow()}</div>
      ${pageWindows()}
      ${sec(T('s.her'))}<div class="grp">
        ${row(esc(T('menu.size')), '', seg('scale', SIZES.map(([name, scale]) => [scale, T(`menu.${name}`)]), s.scale))}
        ${row(esc(T('s.details')), esc(T('s.detailsNote')), sw('details', s.details !== false))}
        ${row(esc(T('s.walk')), esc(T('s.walkNote')), sw('walk', s.walk))}
        ${row(esc(T('s.look')), '', sw('look', s.look))}
      </div>
      ${part('pets', pagePets())}`
  }

  // Windows' own mode and accent colour, set as Settings → Personalization →
  // Colors sets them (theme.rs): its 48 colours to pick from.
  const ACCENTS = [
    '#ffb900', '#ff8c00', '#f7630c', '#ca5010', '#da3b01', '#ef6950', '#d13438', '#ff4343',
    '#e74856', '#e81123', '#ea005e', '#c30052', '#e3008c', '#bf0077', '#c239b3', '#9a0089',
    '#0078d7', '#0063b1', '#8e8cd8', '#6b69d6', '#8764b8', '#744da9', '#b146c2', '#881798',
    '#0099bc', '#2d7d9a', '#00b7c3', '#038387', '#00b294', '#018574', '#00cc6a', '#10893e',
    '#7a7574', '#5d5a58', '#68768a', '#515c6b', '#567c73', '#486860', '#498205', '#107c10',
    '#767676', '#4c4a48', '#69797e', '#4a5459', '#647c64', '#525e54', '#847545', '#7e735f',
  ]
  // The accent colours held back for now: DWM's frames' colour does not yet
  // follow them for sure (theme.rs set_accent), 2026-10-10.
  const ACCENT_PICKER = false
  function pageWindows() {
    const look = snap.winLook || {}
    const accent = String(look.accent || '').toLowerCase()
    // One picked outside these (its own colour in Windows' settings): last, ringed.
    const colours = /^#[0-9a-f]{6}$/.test(accent) && !ACCENTS.includes(accent) ? [...ACCENTS, accent] : ACCENTS
    const swatches = colours.map(c => `<span class="acc${c === accent ? ' on' : ''}" data-accent="${c}" style="background:${c}" title="${c}"></span>`).join('')
    return `${sec(T('s.windows'))}<div class="grp">
        ${row(esc(T('s.winMode')), esc(T('s.winModeNote')), seg('winMode', [['light', T('s.winMode.light')], ['dark', T('s.winMode.dark')]], look.light ? 'light' : 'dark'))}
        ${ACCENT_PICKER ? `<div class="r col"><div>${esc(T('s.winAccent'))}</div><div class="accents">${swatches}</div></div>` : ''}
      </div>
      ${pagePaper()}`
  }

  // Windows' dialog open for the desktop's picture: its own mark, held until
  // the dialog answers (picking, a letter's files', is let go a second after
  // the island has the focus again, which a press on it mid-dialog gives).
  let pickingPaper = false
  // Windows' question for LibreHardwareMonitor (run as administrator) takes
  // the focus too: from the press, and while its job is at that stage.
  let lhmAsking = 0
  const lhmAsks = () => Date.now() - lhmAsking < 5000 || ['install', 'elevate', 'start'].includes(snap?.lhmSetup?.job?.stage)
  // One ask at a time: its buttons grey until Windows has done it (a step
  // waits up to a few seconds to see the picture change; presses meanwhile
  // went on stepping, 2026-10-10).
  let paperBusy = false

  // Asked of Windows for the desktop's picture; the snapshot as it is after.
  function paperDo(what, value) {
    if (paperBusy) return
    paperBusy = true
    pickingPaper = what === 'picture' || what === 'folder'
    draw()
    return window.pet.settings
      .wallpaper(what, value)
      .then(got => {
        snap = got
      })
      .finally(() => {
        paperBusy = false
        pickingPaper = false
        draw()
      })
  }

  // The desktop's picture (paper.rs): one picture or a folder's in turn,
  // picked in Windows' file dialog; how it is laid; the turns.
  const PAPER_POSITIONS = ['fill', 'fit', 'stretch', 'tile', 'center', 'span']
  const PAPER_EVERY = [60000, 600000, 1800000, 3600000, 21600000, 86400000]
  const lastPart = path => String(path || '').split(/[\\/]/).filter(Boolean).pop() || ''
  function pagePaper() {
    const paper = snap.winPaper
    if (!paper) return ''
    const now = paper.slideshow
      ? T('s.paperNowFolder', { name: lastPart(paper.folder) || T('s.paperUnknown') })
      : T('s.paperNowPicture', { name: lastPart(paper.file) || T('s.paperUnknown') })
    const every = PAPER_EVERY.map(ms => [ms, ms >= 86400000 ? T('s.paperDay') : ms >= 3600000 ? T('s.paperHours', { n: ms / 3600000 }) : T('settings.minutes', { n: ms / 60000 })])
    const button = (what, label) => `<button class="pbtn sm" data-paper="${what}" ${paperBusy ? 'disabled' : ''}>${esc(T(label))}</button>`
    // Back to the picture before the last change made here, while there is one.
    const pick = `<span class="btns">${button('picture', 's.paperPicture')}${button('folder', 's.paperFolder')}${paper.canUndo ? button('undo', 's.paperUndo') : ''}</span>`
    return `${sec(T('s.wallpaper'))}<div class="grp">
        ${row(esc(T('s.paperNow')), esc(now), pick)}
        ${row(esc(T('s.paperFit')), '', seg('paperPosition', PAPER_POSITIONS.map(p => [p, T(`s.paperPos.${p}`)]), paper.position))}
        ${
          paper.slideshow
            ? `${row(esc(T('s.paperEvery')), '', seg('paperEvery', every, paper.every))}
        ${row(esc(T('s.paperShuffle')), '', sw('paperShuffle', paper.shuffle))}
        ${row(esc(T('s.paperNext')), '', `<span class="btns">${button('previous', 's.paperPrevButton')}${button('next', 's.paperNextButton')}</span>`)}`
            : ''
        }
      </div>`
  }

  function pageAlerts() {
    const s = snap.settings
    // How long endings stay: the choices under the words (five, too long
    // beside them), and with Custom on, the box for its seconds beside them,
    // kept as it is left or Enter is pressed (holdTyped).
    const holds = HOLDS.map(h => [h, h === 'seen' ? T('s.holdSeen') : h >= 60 ? T('settings.minutes', { n: h / 60 }) : T('settings.seconds', { n: h })])
    holds.push(['custom', T('s.holdCustom')])
    const holdOn = window.Flow.holdChoice(s.hold, holdCustom)
    const range = esc(T('s.holdRange', { min: window.Flow.HOLD_MIN, max: window.Flow.HOLD_MAX }))
    const typed = 'hold:secs' in drafts ? drafts['hold:secs'] : Number.isInteger(s.hold) ? String(s.hold) : ''
    const secs =
      holdOn === 'custom'
        ? `<span class="hold-secs"><input class="in secs" data-hold="secs" value="${esc(typed)}" inputmode="numeric" maxlength="5" placeholder="${window.Flow.HOLD_MIN}–${window.Flow.HOLD_MAX}" title="${range}" aria-label="${range}" spellcheck="false" autocomplete="off"><span class="d">${esc(T('s.holdUnit'))}</span></span>`
        : ''
    const waits = WAITS.map(w => [w, w >= 60 ? T('settings.minutes', { n: Math.round(w / 60) }) : T('settings.seconds', { n: w })])
    const idles = IDLE_HIDES.map(m => [m, !m ? T('s.idleAlways') : m >= 60 ? T('s.paperHours', { n: m / 60 }) : T('s.minutesShort', { n: m })])
    const notify = s.notify || {}
    // When she has something to tell that needs no answer (a turn's end, a
    // hello, a plugin's nudge): the island open, a ring round it (the flow
    // light, the default), or the dot alone (island.js cue); its three
    // choices under it, too long beside it.
    const cue = window.Flow.endCue(s)
    const cues = [
      ['open', T('s.endCueOpen')],
      ['ring', T('s.endCueRing')],
      ['none', T('s.endCueNone')],
    ]
    return `${sec(T('s.holdTitle'))}<div class="grp"><div class="r col cue"><div>${esc(T('s.hold'))}</div><div class="hold-pick">${seg('hold', holds, holdOn)}${secs}</div></div><div class="r col cue"><div><div>${esc(T('s.endCue'))}</div><div class="d">${esc(T('s.endCueNote'))}</div></div>${seg('endCue', cues, cue)}</div></div>
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
        ${row(esc(T('settings.hideInFullscreen')), '', sw('hideInFullscreen', s.hideInFullscreen, snap.fullscreenAvailable ? '' : 'disabled'))}
      </div>`
  }

  // The plugins page as rows, in the order the person put them in: the
  // built-in ones, the plugins she runs (with what their widgets say), and
  // the widgets of anyone else's scripts.
  function widgetRows() {
    const rank = key => {
      const i = (snap.settings.widgetOrder || []).indexOf(key)
      return i < 0 ? Infinity : i
    }
    const widgets = snap.widgets || []
    const rows = [
      ...widgets.filter(w => w.builtIn).map(w => ({ key: w.id, widget: w })),
      ...(snap.plugins || []).map(p => ({ key: p.id, plugin: p, mine: widgets.filter(w => w.plugin === p.id) })),
      ...widgets.filter(w => !w.builtIn && !w.plugin).map(w => ({ key: w.id, widget: w })),
    ]
    return rows.map((r, i) => ({ ...r, i })).sort((x, y) => rank(x.key) - rank(y.key) || x.i - y.i)
  }

  // What a plugin she runs says under its name: what it does, or why it
  // shows nothing.
  function pluginNote(p, mine) {
    if (p.state === 'needs') return { text: T('pl.needs', { what: T(`pl.${p.id}.${p.missing}`) }), bad: true }
    if (p.state === 'stopped') {
      // No PowerShell would start it: not the plugin's fault (#28).
      const why = p.noShell ? T('pl.noShell', { line: p.said || '' }) : p.said ? T('pl.stopped', { line: p.said }) : T('pl.stoppedQuiet')
      return { text: p.retryIn ? `${why} · ${T('pl.retry', { n: p.retryIn })}` : why, bad: true }
    }
    if (p.state === 'running' && !mine.length) return p.said ? { text: T('pl.trouble', { line: p.said }), bad: true } : { text: T('pl.starting') }
    return { text: T(`w.ex.${p.id}.note`) }
  }

  // A plugin's settings, folded out under it: what is being typed stays.
  function pluginFields(p) {
    const fields = p.params.map(f => {
      const key = `${p.id}.${f.name}`
      const label = esc(T(`pl.${key}`))
      if (f.kind === 'flag') return `<div class="fr"><span class="fl">${label}</span><span class="grow"></span>${sw('pf:' + key, f.value === true)}</div>`
      // One of a few (a stock's up and down colours): the one in effect,
      // her language's until one is picked (scripts.rs choice).
      if (f.kind === 'choice') return `<div class="fr"><span class="fl">${label}</span><span class="grow"></span>${seg('pc:' + key, (f.options || []).map(o => [o, T(`pl.${key}.${o}`)]), f.value)}</div>`
      const value = key in drafts ? drafts[key] : f.value || ''
      return `<label class="fr"><span class="fl">${label}${f.required ? ' *' : ''}</span><input class="in" data-field="${esc(key)}" value="${esc(value)}" placeholder="${esc(T(`pl.${key}.hint`))}" spellcheck="false"${f.kind === 'number' ? ' inputmode="decimal"' : ''}></label>`
    })
    return `<div class="pf">${fields.join('')}</div>`
  }

  // The monitor's settings, folded out under it: pinned or not, how often it
  // reads (1 to 10 seconds), where its sensors come from (monitorSource:
  // Windows' own, hw.rs, or LibreHardwareMonitor, lhm.rs; each keeps its own
  // picks; none chosen, the one in effect, snap.monitorSource), and those
  // sensors.
  function monitorFields() {
    const m = snap.settings.monitor || {}
    const source = snap.monitorSource === 'lhm' ? 'lhm' : 'builtin'
    const every = m.every || 2
    const pace = `<div class="fr"><span class="fl">${esc(T('mon.every'))}</span><span class="grow"></span><button class="pbtn sm" data-every="-1" ${every <= 1 ? 'disabled' : ''}>−</button><span class="every">${esc(T('settings.seconds', { n: every }))}</span><button class="pbtn sm" data-every="1" ${every >= 10 ? 'disabled' : ''}>+</button></div>`
    const pin = `<div class="fr"><span class="fl">${esc(T('mon.pin'))}</span><span class="d grow">${esc(T('mon.pinNote'))}</span>${sw('mon:pin', m.pin !== false)}</div>`
    // The one in effect; none chosen yet, it says how that one was found
    // (hw.rs Source::in_effect), and picking one keeps it.
    const auto = snap.settings.monitorSource == null ? `<div class="note lhm-note">${esc(T('mon.sourceAuto'))}</div>` : ''
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
    const v = snap.hw
    if (!v || !Array.isArray(v.hardware)) return `<div class="note lhm-note">${esc(T('hw.unread'))}</div>`
    const hardwareName = h => h.name || T(`hw.${h.class}`)
    const groups = v.hardware.map(h => {
      const health = h.health ? `<div class="fr lsen info"><span class="fl">${esc(T('hw.health'))}</span><span class="d">${esc(T('hw.healthReading', { used: h.health.used, spare: h.health.spare, hours: h.health.hours }))}</span></div>` : ''
      const about = h.about && typeof h.about === 'object' ? render(lang, h.about) : h.about || ''
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
    const cs = snap.cpuSensors || {}
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
    const v = snap.lhm || {}
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
    const u = snap.lhmSetup
    if (!u || !u.step || u.step === 'off' || u.step === 'looking') return note
    const line = (text, bad) => `<div class="note lhm-note${bad ? ' bad' : ''}">${text}</div>`
    const button = (what, label, accent) => `<div class="fr"><span class="fl"></span><span class="grow"></span><button class="pbtn sm${accent ? ' al' : ''}" data-lhm="${what}">${esc(label)}</button></div>`
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
  // Plugins: built in, run by her, or anyone's script, each with its switch
  // (and a step up); how they take turns, whether they may open the island,
  // waku for the terminal, and how to write one.
  function pageWidgets() {
    const s = snap.settings
    const left = ms => (ms >= 60000 ? T('settings.minutes', { n: Math.round(ms / 60000) }) : T('settings.seconds', { n: Math.max(1, Math.round(ms / 1000)) }))
    // Where each goes (the widget design's §10): a row in the capsule, on
    // the taskbar (where she lives there), or on the desktop (Windows'; desk.rs),
    // island.js placeWidget keeping each. Its switch stays what turns it off.
    const places = [['isle', T('menu.isle')], ...(s.display === 'taskbar' ? [['bar', T('menu.bar')]] : []), ...(window.pet?.mac ? [] : [['desk', T('menu.desk')]])]
    const onBarHome = !!window.Island?.whereOf && places.length > 1
    const whereSeg = item => (onBarHome ? seg(`where:${item.plugin ? `p:${item.plugin}` : `w:${item.id}`}`, places, window.Island.whereOf(item)) : '')
    const rows = widgetRows()
      .map((r, i) => {
        const up = (i > 0 ? `<button class="pbtn sm" data-wup="${esc(r.key)}" title="${esc(T('w.up'))}">↑</button>` : '') + (r.plugin ? whereSeg({ plugin: r.plugin.id }) : whereSeg(r.widget.plugin ? { plugin: r.widget.plugin } : { id: r.widget.id }))
        if (r.plugin) {
          const p = r.plugin
          const says = r.mine.map(w => Widgets.words(lang, w, s.details !== false)).map(({ label, value }) => [label, value].filter(Boolean).join(' ')).join(' · ')
          const note = pluginNote(p, r.mine)
          const color = (r.mine[0] && r.mine[0].color) || (p.on ? '#5e9bff' : COLOR.idle)
          const isOpen = openPlugin === p.id
          const more = p.params.length ? `<button class="pbtn sm" data-pexp="${esc(p.id)}">${esc(T(isOpen ? 'pl.fold' : 'pl.settings'))}</button>` : ''
          return `<div class="r" data-part="widgets:${esc(r.key)}"><span class="wi" style="color:${esc(color)}">${Widgets.icon(p.icon)}</span><div class="grow"><div class="ellip">${esc(T(`w.ex.${p.id}`))}<span class="d"> ${esc(says)}</span></div><div class="d${note.bad ? ' bad' : ''}">${esc(note.text)}</div></div>${more}${up}${sw('pl:' + p.id, p.on)}</div>${isOpen ? pluginFields(p) : ''}`
        }
        const w = r.widget
        const { label, value } = Widgets.words(lang, w, s.details !== false)
        // A built-in one says what it shows; on, with nothing read, says so.
        const builtIn = w.builtIn && [T('w.builtIn'), T(`w.about.${w.id}`), w.on && w.value == null ? T('w.unread') : ''].filter(Boolean).join(' · ')
        const from = builtIn || (w.from === 'mail' ? T('w.fromMail') : [T('w.script', { time: left(w.leftMs || 0) }), w.private ? T('w.private') : ''].filter(Boolean).join(' · '))
        // The monitor: its readings as icon and number, its settings folded under it.
        const says = Widgets.partsOf(w) ? ` ${Widgets.partsHTML(w)}` : `<span class="d"> ${esc(value)}</span>`
        const isMonitor = w.id === 'monitor'
        const more = isMonitor ? `<button class="pbtn sm" data-pexp="monitor">${esc(T(openPlugin === 'monitor' ? 'pl.fold' : 'pl.settings'))}</button>` : ''
        return `<div class="r" data-part="widgets:${esc(r.key)}"><span class="wi" style="color:${esc(w.color || COLOR.idle)}">${Widgets.iconHTML(w, lang)}</span><div class="grow"><div class="ellip">${esc(label)}${says}</div><div class="d">${esc(from)}</div></div>${more}${up}${sw('w:' + w.id, w.on)}</div>${isMonitor && openPlugin === 'monitor' ? monitorFields() : ''}`
      })
      .join('')
    const spins = SPINS.map(n => [n, n ? T('settings.seconds', { n }) : T('w.spinOff')])
    const waku = `powershell -ExecutionPolicy Bypass -File "${snap.waku}" npm run build`
    const example = `Invoke-RestMethod -Method Post http://127.0.0.1:${snap.port}/widget -ContentType application/json -Body '{"id":"hello","label":"Hello","value":"42","icon":"star"}'`
    return `${sec(T('w.section'))}<div class="grp"><div class="note">${esc(T('w.note'))}${onBarHome ? ` ${esc(T('w.whereNote'))}` : ''}</div>${rows}</div>
      ${sec(T('w.show'))}<div class="grp">
        ${row(esc(T('w.spin')), '', seg('widgetSpin', spins, s.widgetSpin ?? 8))}
        ${row(esc(T('w.nudge')), esc(T('w.nudgeNote')), sw('widgetNudge', s.widgetNudge !== false))}
      </div>
      ${sec(T('w.terminal'))}<div class="grp"><div class="note">${esc(T('w.ex.waku.note'))}</div>
        <div class="r"><span class="cmd mono">${esc(waku)}</span><button class="pbtn" data-copy="${esc(waku)}">${esc(T('home.copy'))}</button></div>
      </div>
      ${sec(T('w.write'))}<div class="grp"><div class="note">${esc(T('w.writeNote'))}</div>
        <div class="r"><span class="cmd mono">${esc(example)}</span><button class="pbtn" data-copy="${esc(example)}">${esc(T('home.copy'))}</button></div>
      </div>`
  }

  // What went wrong with a mail account, in words: through a proxy, which
  // hop (mail/proxy.rs), and what to try then.
  function mailError(err, server) {
    const kind = err?.kind || 'other'
    const where = server ? `${server.host}:${server.port}` : ''
    // The server's own words (or the proxy's), where they say why.
    const detail = err?.text && ['login', 'refused', 'other', 'recipient', 'notProxy', 'proxyRefused', 'proxyServer'].includes(kind) ? (lang === 'zh' ? '：' : ': ') + err.text : ''
    const hint = err?.hint ? (lang === 'zh' ? '。' : '. ') + T(`mail.err.${kind === 'notProxy' ? 'kind' : 'try'}.${err.hint}`) : ''
    return T(`mail.err.${kind}`, { where, proxy: err?.proxy || '' }) + detail + hint
  }

  // An account's line: how its inbox is, or why it is not.
  function mailNote(a) {
    if (!a.on) return { text: T('mail.off') }
    const st = a.status || {}
    // How much is kept whole on this computer, while it is still coming down.
    const off = st.offline || {}
    const down = off.total && off.have < off.total ? ` · ${T('mail.offline', { have: off.have, total: off.total })}` : ''
    // Through a proxy: which.
    const via = st.via ? ` · ${T('mail.via', { proxy: st.via })}` : ''
    if (st.state === 'ok') return { text: (st.unread ? T('mail.unread', { n: st.unread }) : T('mail.none')) + down + via }
    if (st.state === 'error') return { text: mailError(st.error, a), bad: true }
    return { text: T('mail.connecting') }
  }

  // Setting an account up: the address and the password, then the server
  // found (a line, and a way to change it) or its fields to fill in.
  // Thunderbird's choices for the incoming server: how the connection is
  // secured, and how to sign in (a normal password, either way; the others
  // it offers are listed, not to be picked).
  const SECURITIES = () => [['auto', T('mail.auto')], ['plain', T('mail.plain')], ['starttls', 'STARTTLS'], ['ssl', 'SSL/TLS']]
  const AUTHS = () => [
    ['auto', T('mail.auto')],
    ['plain', T('mail.auth.plain')],
    ['login', T('mail.auth.login')],
    ['cram', T('mail.auth.cram'), true],
    ['gssapi', 'Kerberos / GSSAPI', true],
    ['ntlm', 'NTLM', true],
    ['oauth2', 'OAuth2', true],
  ]
  const labelOf = (list, value) => list.find(x => x[0] === value)?.[1] || value

  // Where a provider takes an app password (or a code) instead of the one
  // its users sign in with, by its IMAP host; and where it makes one.
  function providerOf(host) {
    const h = String(host || '').toLowerCase()
    if (/(^|\.)(gmail|googlemail)\.com$/.test(h)) return 'google'
    if (/(^|\.)mail\.me\.com$/.test(h)) return 'apple'
    if (/(^|\.)yahoo\.com$/.test(h)) return 'yahoo'
    if (/(^|\.)qq\.com$/.test(h)) return 'qq'
    if (/(^|\.)(163|126)\.com$|(^|\.)yeah\.net$/.test(h)) return 'netease'
    return ''
  }
  const HINT_SITE = { google: 'googleAppPasswords', apple: 'appleAppPasswords', yahoo: 'yahooAppPasswords' }

  function hintHTML(host) {
    const p = providerOf(host)
    if (!p) return ''
    const site = HINT_SITE[p]
    return `<div class="note">${esc(T(`mail.hint.${p}`))}${site ? ` <span class="link" data-site="${site}">${esc(T('mail.hint.open'))}</span>` : ''}</div>`
  }

  // The outgoing server's fields, from what was found (or none).
  const outFields = o =>
    o
      ? { smtpHost: o.host, smtpPort: o.port, smtpSecurity: o.security, smtpAuth: o.auth || 'auto', smtpUser: o.username }
      : { smtpHost: '', smtpPort: '', smtpSecurity: 'auto', smtpAuth: 'auto', smtpUser: '' }

  // An account's own way to its servers (its "proxy", mail/proxy.rs) as the
  // form has it: a mode ('' for the Mail page's), and a manual one's kind
  // and address; and back, as sent (null: the Mail page's).
  const proxyFields = p => ({ proxyMode: p?.mode || '', proxyKind: p?.kind === 'socks5' ? 'socks5' : 'http', proxyAddress: p?.address || '' })
  const formProxy = f =>
    !f.proxyMode ? null : f.proxyMode === 'manual' ? { mode: 'manual', kind: f.proxyKind || 'http', address: String(f.proxyAddress || '').trim() } : { mode: f.proxyMode }
  const PROXY_KINDS = [
    ['http', 'HTTP'],
    ['socks5', 'SOCKS5'],
  ]
  const PROXY_MODES = () => [
    ['', T('mail.proxy.page', { what: T(`mail.proxy.${proxySetting().mode}Long`) })],
    ['system', T('mail.proxy.systemLong')],
    ['manual', T('mail.proxy.manualLong')],
    ['none', T('mail.proxy.noneLong')],
  ]

  // Setting an account up: the address, a name to send with, the password;
  // then the servers found (a line each) or not, and what the provider
  // wants for a password; and, folded as Thunderbird's manual setup, the
  // incoming and the outgoing server's every setting, with Re-test.
  function mailFormHTML() {
    const f = mailForm
    const field = (key, label, attrs = '') =>
      `<label class="fr"><span class="fl">${esc(label)}</span><input class="in" data-mf="${key}" value="${esc(f[key] ?? '')}" spellcheck="false" ${attrs}></label>`
    const head = [
      field('address', T('mail.address'), `placeholder="you@example.com" autocomplete="off"${f.id ? ' disabled' : ''}`),
      field('name', T('mail.name'), `placeholder="${esc(T('mail.namePlaceholder'))}" autocomplete="off"`),
      field('password', T('mail.password'), `type="password" autocomplete="off" placeholder="${esc(f.id ? T('mail.passwordKeep') : '')}"`),
      `<div class="note">${esc(T('mail.passwordNote'))}</div>`,
      f.step === 'start' ? '' : hintHTML(f.host),
    ].join('')
    const line = (proto, host, port, security, auth) => `${proto} · ${host}:${port} · ${labelOf(SECURITIES(), security)}${auth && auth !== 'auto' ? ` · ${labelOf(AUTHS(), auth)}` : ''}`
    const out = f.smtpHost ? `<div class="ellip">${esc(line('SMTP', f.smtpHost, f.smtpPort, f.smtpSecurity, f.smtpAuth))}</div>` : ''
    const noOut = f.smtpHost || f.findingOut ? '' : `<div class="note">${esc(T('mail.found.noOut'))}</div>`
    const found =
      f.step === 'found'
        ? `<div class="r"><div class="grow"><div class="ellip">${esc(line('IMAP', f.host, f.port, f.security, f.auth))}</div>${out}${f.source ? `<div class="d">${esc(T(`mail.source.${f.source}`))}</div>` : ''}</div></div>${noOut}`
        : ''
    const notFound = f.step === 'manual' && f.notFound ? `<div class="note">${esc(T(f.oauth ? 'mail.oauth' : 'mail.notFound'))}</div>` : ''
    const adv = f.step === 'start' ? '' : advancedHTML(field)
    const busy = f.busy ? `<div class="note">${esc(T({ find: 'mail.finding', probe: 'mail.adv.testing' }[f.busy] || 'mail.signingIn'))}</div>` : ''
    const result = f.result ? `<div class="note${f.result.bad ? ' err' : ''}">${esc(f.result.text)}</div>` : ''
    const off = f.busy ? 'disabled' : ''
    const go = f.step === 'start' ? `<button class="pbtn al" data-mail-find ${off}>${esc(T('mail.continue'))}</button>` : `<button class="pbtn al" data-mail-save ${off}>${esc(T('mail.done'))}</button>`
    const again = f.step === 'manual' && !f.id ? `<button class="pbtn" data-mail-find ${off}>${esc(T('mail.findAgain'))}</button>` : ''
    const retest = f.step !== 'start' && f.advanced ? `<button class="pbtn" data-mail-probe ${off}>${esc(T('mail.adv.retest'))}</button>` : ''
    const remove = f.id ? `<button class="pbtn" data-mail-remove="${esc(f.id)}">${esc(T(mailDelete === f.id ? 'mail.removeSure' : 'mail.remove'))}</button>` : ''
    return `${sec(T(f.id ? 'mail.change' : 'mail.add'))}<div class="grp mail-form">${head}${found}${notFound}${adv}${busy}${result}
      <div class="r">${remove}<span class="grow"></span><button class="pbtn" data-mail-cancel>${esc(T('mail.cancel'))}</button>${again}${retest}${go}</div></div>`
  }

  // The fold: Thunderbird's manual setup for the incoming server and the
  // outgoing one (protocol, host name, port, connection security,
  // authentication method, user name), what Re-test found of each.
  function advancedHTML(field) {
    const f = mailForm
    const toggle = `<button class="adv-h${f.advanced ? ' on' : ''}" data-mail-adv><span class="chev">›</span>${esc(T('mail.adv'))}</button>`
    if (!f.advanced) return `<div class="adv">${toggle}</div>`
    const pick = (key, list) => `<button class="pick" data-mfpick="${key}">${esc(labelOf(list, f[key] || 'auto'))} ▾</button>`
    const rows = [
      `<div class="adv-sec">${esc(T('mail.adv.in'))}</div>`,
      `<div class="fr"><span class="fl">${esc(T('mail.adv.protocol'))}</span><button class="pick" disabled>IMAP</button></div>`,
      field('host', T('mail.adv.host'), 'placeholder="imap.example.com"'),
      field('port', T('mail.port'), `inputmode="numeric" placeholder="${esc(T('mail.auto'))}"`),
      `<div class="fr"><span class="fl">${esc(T('mail.adv.security'))}</span>${pick('security', SECURITIES())}</div>`,
      `<div class="fr"><span class="fl">${esc(T('mail.adv.auth'))}</span>${pick('auth', AUTHS())}</div>`,
      field('username', T('mail.username'), `placeholder="${esc(f.address || 'you@example.com')}"`),
      f.security === 'plain' ? `<div class="note err">${esc(T('mail.adv.plainWarn'))}</div>` : '',
      probedHTML(f.probed),
      `<div class="adv-sec">${esc(T('mail.adv.out'))}</div>`,
      `<div class="fr"><span class="fl">${esc(T('mail.adv.protocol'))}</span><button class="pick" disabled>SMTP</button></div>`,
      field('smtpHost', T('mail.adv.host'), 'placeholder="smtp.example.com"'),
      field('smtpPort', T('mail.port'), `inputmode="numeric" placeholder="${esc(T('mail.auto'))}"`),
      `<div class="fr"><span class="fl">${esc(T('mail.adv.security'))}</span>${pick('smtpSecurity', SECURITIES())}</div>`,
      `<div class="fr"><span class="fl">${esc(T('mail.adv.auth'))}</span>${pick('smtpAuth', AUTHS())}</div>`,
      field('smtpUser', T('mail.username'), `placeholder="${esc(f.username || f.address || 'you@example.com')}"`),
      f.smtpHost && f.smtpSecurity === 'plain' ? `<div class="note err">${esc(T('mail.adv.plainWarn'))}</div>` : '',
      probedHTML(f.probedOut),
      `<div class="note">${esc(T('mail.adv.outNote'))}</div>`,
      `<div class="adv-sec">${esc(T('mail.proxy'))}</div>`,
      `<div class="fr"><span class="fl">${esc(T('mail.adv.proxyWay'))}</span><button class="pick" data-mfpick="proxyMode">${esc(labelOf(PROXY_MODES(), f.proxyMode || ''))} ▾</button></div>`,
      f.proxyMode === 'manual' ? `<div class="fr"><span class="fl">${esc(T('mail.proxy.kind'))}</span><button class="pick" data-mfpick="proxyKind">${esc(labelOf(PROXY_KINDS, f.proxyKind || 'http'))} ▾</button></div>` : '',
      f.proxyMode === 'manual' ? field('proxyAddress', T('mail.proxy.address'), 'placeholder="127.0.0.1:7890"') : '',
      `<div class="note">${esc(T('mail.adv.proxyNote'))}</div>`,
    ]
    return `<div class="adv on">${toggle}<div class="adv-b">${rows.join('')}</div></div>`
  }

  // What Re-test found: how it is reached, how it lets one sign in, and,
  // for a Windows domain's server (Exchange), what the user name may be.
  function probedHTML(p) {
    if (!p) return ''
    const names = p.auths.map(a => {
      const [, label, off] = AUTHS().find(x => x[0] === a) || [a, a, true]
      return off ? `${label}${T('mail.unsupported')}` : label
    })
    const how = T('mail.adv.found', { how: labelOf(SECURITIES(), p.security), port: p.port })
    const auths = p.auths.some(a => a === 'plain' || a === 'login') ? T('mail.adv.auths', { list: names.join(lang === 'zh' ? '、' : ', ') }) : T('mail.adv.noAuth')
    const domain = p.auths.some(a => a === 'ntlm' || a === 'gssapi') ? `<div class="note">${esc(T('mail.adv.domain'))}</div>` : ''
    return `<div class="note ok">✓ ${esc(how)} ${esc(auths)}</div>${domain}`
  }

  // Who a letter goes to first (the setting), and the others that read mail
  // (agents.js mailAgents): Antigravity only where it is (or set first), as
  // few have it: its agy found, or Antigravity on this computer as the Agent
  // page has it (agy's folder, or the 2.0 app's: connection.rs agy_here).
  // The app alone brings no agy, which mail runs: Antigravity is listed
  // then, greyed, with a line on why (zhoupengjie/Wakuwaku#42). agent.rs
  // looks for agy again while the settings are open, and once installed it
  // can be picked.
  const firstAgent = () => (AGENTS[snap.settings.mailAgent] ? snap.settings.mailAgent : 'claude')
  const mailList = () => Agents.mailAgents(snap, firstAgent())
  const agyWanted = () => mailList().some(a => a.id === 'agy' && !a.found)
  // A letter's talk going on now, by its key (agent.rs).
  const talkOf = key => (key && (snap.mailTalks || {})[key]) || null
  const letterKey = l => l.messageId || `${l.account}:${l.uid}`
  const accountOf = id => (snap.mail || []).find(a => a.id === id) || null

  // The inbox shown: "*" (every account's as one) where there are several,
  // else the one; one picked before stays while it is there.
  function inboxId() {
    const all = snap.mail || []
    if (inbox.id === '*' && all.length > 1) return '*'
    if (all.some(a => a.id === inbox.id)) return inbox.id
    return all.length > 1 ? '*' : all[0]?.id || ''
  }

  const unreadOf = a => (a.on && a.status?.state === 'ok' ? a.status.unread || 0 : 0)

  function mailDate(ms, full) {
    if (!ms) return ''
    const d = new Date(ms)
    const now = new Date()
    const locale = lang === 'zh' ? 'zh-CN' : 'en-US'
    const time = d.toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit', hour12: false })
    if (full) return `${d.toLocaleDateString(locale, { year: 'numeric', month: 'short', day: 'numeric' })} ${time}`
    if (d.toDateString() === now.toDateString()) return time
    if (d.getFullYear() === now.getFullYear()) return d.toLocaleDateString(locale, { month: 'short', day: 'numeric' })
    return d.toLocaleDateString(locale, { year: 'numeric', month: 'numeric', day: 'numeric' })
  }

  const mailSize = n =>
    n >= 1 << 30 ? `${(n / (1 << 30)).toFixed(1)} GB` : n >= 1 << 20 ? `${(n / (1 << 20)).toFixed(1)} MB` : n >= 1024 ? `${Math.round(n / 1024)} KB` : `${n} B`
  const people = list => (list || []).map(p => (p.name ? `${p.name} <${p.address}>` : p.address)).join(', ')
  const starHTML = (account, uid, on, folder = 'inbox') =>
    `<button class="star${on ? ' on' : ''}" data-star="${esc(account)}" data-folder="${folder}" data-uid="${uid}" title="${esc(T(on ? 'mail.unstar' : 'mail.star'))}">${on ? '★' : '☆'}</button>`

  // A letter by where it is: its account, its folder (inbox, sent), its UID
  // there (the inbox's and Sent's UIDs are their own).
  const isLetter = (l, account, folder, uid) => !!l && l.account === account && (l.folder || 'inbox') === (folder || 'inbox') && l.uid === uid
  const isReading = (account, folder, uid) => isLetter(reading, account, folder, uid)
  const listed = (account, folder, uid) => inbox.threads.flatMap(t => t.letters).find(l => isLetter(l, account, folder, uid)) || null
  const where = l => `data-letter="${l.uid}" data-folder="${l.folder || 'inbox'}" data-account="${esc(l.account)}"`
  const whoOf = l => (l.me ? T('mail.me') : l.from || l.address || '?')
  const chipOf = l => {
    const talk = talkOf(letterKey(l))
    return talk ? `<span class="chip ${talk.state}">${esc(T(`mail.chip.${talk.state}`, { agent: AGENTS[talk.agent] || 'Agent' }))}</span>` : ''
  }
  const marks = l => `${l.answered ? `<span class="answered" title="${esc(T('mail.answered'))}">↩</span> ` : ''}${l.attached ? '📎 ' : ''}`

  // A letter's card in the inbox: unread or not, who, when, what about, its
  // star, whose (in every account's), and whether an agent is on it.
  function letterRow(l, every) {
    const whose = every ? `<span class="acct ellip">${esc(accountOf(l.account)?.address || '')}</span>` : ''
    const open = isReading(l.account, l.folder, l.uid) ? ' open' : ''
    return `<div class="r letter${l.seen ? '' : ' unread'}${open}" ${where(l)}><span class="udot"></span>
      <div class="grow"><div class="lt"><span class="who ellip">${esc(whoOf(l))}</span><span class="when">${esc(mailDate(l.date))}</span></div>
      <div class="lt"><span class="d ellip grow">${marks(l)}${esc(l.subject || T('mail.noSubject'))}</span>${whose}</div></div>${chipOf(l)}${starHTML(l.account, l.uid, l.flagged, l.folder)}</div>`
  }

  // A conversation, as Thunderbird's cards show one: who wrote (oneself as
  // me), how many letters, when the newest came, what about; folded out, its
  // letters under it in the order they answer one another, each indented by
  // how deep it is, with the start of its text. A click on it opens its
  // newest unread letter (else its newest) and folds it out; › folds it.
  function threadHTML(t, every) {
    const out = expanded.has(t.key)
    const target = t.letters.find(l => isLetter(l, t.account, t.open?.folder, t.open?.uid)) || t.letters.at(-1)
    const open = !out && t.letters.some(l => isReading(l.account, l.folder, l.uid)) ? ' open' : ''
    const who = t.people.map(p => (p.me ? T('mail.me') : p.name)).join(', ')
    const whose = every ? `<span class="acct ellip">${esc(accountOf(t.account)?.address || '')}</span>` : ''
    const any = { answered: t.letters.some(l => l.answered), attached: t.attached }
    const head = `<div class="r letter thread${t.unseen ? ' unread' : ''}${open}" data-thread="${esc(t.key)}" ${where(target)}>
      <span class="twisty${out ? ' out' : ''}" data-twisty="${esc(t.key)}" title="${esc(T(out ? 'mail.fold' : 'mail.unfold'))}">›</span>
      <div class="grow"><div class="lt"><span class="who ellip">${esc(who)}</span><span class="cnt">${t.count}</span><span class="when">${esc(mailDate(t.latest))}</span></div>
      <div class="lt"><span class="d ellip grow">${marks(any)}${esc(t.subject || T('mail.noSubject'))}</span>${whose}</div></div>${starHTML(target.account, target.uid, t.flagged, target.folder)}</div>`
    if (!out) return head
    const cards = t.letters
      .map(l => {
        const here = isReading(l.account, l.folder, l.uid) ? ' open' : ''
        return `<div class="r letter child${l.seen ? '' : ' unread'}${here}" style="--depth:${Math.min(l.depth, 4)}" ${where(l)}><span class="udot"></span>
        <div class="grow"><div class="lt"><span class="who ellip">${esc(whoOf(l))}</span><span class="when">${esc(mailDate(l.date))}</span></div>
        <div class="lt"><span class="d ellip grow">${marks(l)}${esc(l.snippet || l.subject || T('mail.noSubject'))}</span></div></div>${chipOf(l)}${starHTML(l.account, l.uid, l.flagged, l.folder)}</div>`
      })
      .join('')
    return `${head}<div class="thread-cards">${cards}</div>`
  }

  // Which inbox, as the picker names it: every account's (with all their
  // unread), or one (with its).
  function boxName(id) {
    const all = snap.mail || []
    if (id === '*') return T('mail.box.all', { n: all.reduce((n, a) => n + unreadOf(a), 0) })
    const a = accountOf(id)
    return a ? T('mail.box.one', { address: a.address, n: unreadOf(a) }) : ''
  }

  // The Mail page in two panes, list and letter side by side, as Thunderbird
  // has it (the setting mailPanes, two unless one), where the screen has the
  // room; the settings as wide as that then (island.js measures them).
  const WIDE_W = 960
  const roomForTwo = () => screen.availWidth >= 820
  const twoPanes = () => tab === 'mail' && snap?.settings?.mailPanes !== 'one' && roomForTwo()

  // The inbox: which (every account's, or one), all, unread or starred, its
  // newest letters, more on asking; one pane or two.
  function inboxHTML(id) {
    const all = snap.mail || []
    const every = id === '*'
    const two = twoPanes()
    const pick = all.length > 1 ? `<button class="pick" data-mail-box>${esc(boxName(id))} ▾</button>` : ''
    const filters = seg('mail.filter', [['all', T('mail.filter.all')], ['unseen', T('mail.filter.unseen')], ['flagged', T('mail.filter.flagged')]], inbox.filter)
    // Two panes: the accounts and agents shown on the right again (the letter
    // open put away); one pane or two.
    const setup = two ? `<span class="link" data-mail-setup title="${esc(T('mail.setupTip'))}">${esc(T('mail.setup'))}</span>` : ''
    const panes = two ? `<span class="link" data-mail-panes="one">${esc(T('mail.panes.one'))}</span>` : roomForTwo() ? `<span class="link" data-mail-panes="two">${esc(T('mail.panes.two'))}</span>` : ''
    const head = `<div class="sec row-sec"><span class="ellip">${esc(T('mail.inbox'))}${inbox.letters ? ` · ${inbox.letters}` : ''}</span><span class="grow"></span><span class="link" data-w-new>${esc(T('mail.write'))}</span><span class="link" data-mail-refresh>${esc(T(inbox.busy ? 'mail.loading' : 'mail.refresh'))}</span>${setup}${panes}</div>`
    const tools = `<div class="r mail-tools">${pick}<span class="grow"></span>${filters}</div>`
    // The letter put aside, to go on with (not while it is written beside the list).
    const kept = draft && !(two && writing)
      ? `<div class="r draft-row" data-w-open><span class="chip">${esc(T('mail.w.draft'))}</span><div class="grow ellip">${esc(draft.subject || T('mail.noSubject'))}${draft.to ? ` · ${esc(draft.to)}` : ''}</div><span class="link">${esc(T('mail.w.goOn'))}</span></div>`
      : ''
    const note = handNote && !two ? `<div class="note${handNote.bad ? ' err' : ''}">${esc(handNote.text)}</div>` : ''
    // Accounts that could not be asked, in every account's.
    const missed = (inbox.errors || []).map(e => `<div class="note err">${esc(`${accountOf(e.account)?.address || ''}: ${mailError(e.error, accountOf(e.account))}`)}</div>`).join('')
    let list
    if (inbox.error) list = `<div class="note err">${esc(mailError(inbox.error, accountOf(id)))}</div>`
    else if (!inbox.threads.length) list = `<div class="note">${esc(T(inbox.syncing ? 'mail.syncing' : inbox.busy && !inbox.rev ? 'mail.loading' : `mail.empty.${inbox.filter}`))}</div>`
    else list = inbox.threads.map(t => (t.count > 1 ? threadHTML(t, every) : letterRow(t.letters[0], every))).join('')
    const left = inbox.total - inbox.threads.length
    const more = !inbox.error && left > 0 ? `<div class="r"><button class="pbtn wide" data-mail-more ${inbox.busy ? 'disabled' : ''}>${esc(T('mail.moreThreads', { n: Math.min(50, left) }))}</button></div>` : ''
    return `${head}<div class="grp inbox">${tools}${kept}${note}${missed}${list}${more}</div>`
  }

  // Mail's AI, one for every letter of every mailbox (the user, 2026-10-14:
  // which agent, its model, its effort, and no more): its model and effort
  // (mailAgentConf, agent.rs; empty is the agent's own setting), what a talk
  // may do the least it has, said under them (Antigravity's models are each
  // one effort already). Not found, it says so there. Which agent: a
  // dropdown of those that read mail, the ones whose command is not found
  // greyed with why (agentChoices).
  function agentsHTML() {
    const ag = firstAgent()
    const found = snap.mailAgents || {}
    const agent = row(esc(T('mail.agent')), esc(T('mail.agentNote')), `<button class="pick" data-mail-agent>${esc(AGENT_NAMES[ag])} ▾</button>`)
    const why = agyWanted() ? `<div class="note">${esc(T('mail.agyCli'))} <span class="link" data-site="agyCli">${esc(T('mail.agyCliHow'))}</span></div>` : ''
    const note = found[ag] ? T(`mail.conf.note.${ag}`) : T('mail.err.noAgent', { agent: AGENT_NAMES[ag] })
    const pickers = ag === 'agy' ? pickerHTML(ag, 'model') : pickerHTML(ag, 'model') + pickerHTML(ag, 'effort')
    const model = row(esc(T(ag === 'agy' ? 'mail.conf.model' : 'mail.conf.session')), esc(note), `<span class="btns">${pickers}</span>`)
    return `<div class="grp agent-pick">${agent}${why}${model}</div>`
  }

  // How a letter goes to an agent (mailAgentConf, agent.rs): its model and
  // effort; empty is the agent's own setting.
  const confOf = ag => ((snap.settings.mailAgentConf || {})[ag] || {})
  const modelsOf = ag => (snap.mailModels || {})[ag] || []

  // The efforts a model takes (all of them while it is the agent's own).
  function effortsOf(ag, model) {
    const all = modelsOf(ag)
    const m = all.find(x => x.id === model)
    return m ? m.efforts : [...new Set(all.flatMap(x => x.efforts))]
  }

  // A picker's choices: the agent's own first, then each model or effort.
  function choicesOf(ag, key) {
    const c = confOf(ag)
    const list = key === 'model' ? modelsOf(ag).map(m => [m.id, m.name]) : effortsOf(ag, c.model).map(e => [e, effortName(e)])
    // One set before and no longer listed stays a choice.
    if (c[key] && !list.some(([v]) => v === c[key])) list.push([c[key], c[key]])
    return [['', T('mail.conf.own')], ...list]
  }

  const effortName = e => (T(`mail.effort.${e}`) === `mail.effort.${e}` ? e : T(`mail.effort.${e}`))

  function pickerHTML(ag, key) {
    const value = confOf(ag)[key] || ''
    const label = choicesOf(ag, key).find(([v]) => v === value)?.[1] || value
    return `<button class="pick" data-picker="${ag}.${key}">${esc(label)} ▾</button>`
  }

  // One of them changed; an effort the new model does not take goes back
  // to the agent's own.
  function setConf(ag, key, value) {
    const all = snap.settings.mailAgentConf || {}
    const next = { ...(all[ag] || {}), [key]: value }
    if (key === 'model' && next.effort && !effortsOf(ag, value).includes(next.effort)) next.effort = ''
    patch({ mailAgentConf: { ...all, [ag]: next } })
  }

  // A little markdown, as agents write: lines, **bold**, *italic*, `code`,
  // headings, lists, quotes (indented under a list item too, or within
  // bold: **> …**), ``` blocks (a reply it wrote, often). The text is
  // escaped first.
  const inline = s =>
    s
      .replace(/\*\*(.+?)\*\*/g, '<b>$1</b>')
      .replace(/(^|[^*\w])\*(?!\s)([^*]+?)\*(?![*\w])/g, '$1<i>$2</i>')
      .replace(/`([^`]+)`/g, '<code>$1</code>')

  function md(text) {
    let html = ''
    let block = null
    const pre = lines => `<pre class="md-pre">${lines.join('\n')}</pre>`
    for (const raw of esc(text).split('\n')) {
      if (/^\s*```/.test(raw)) {
        if (block) html += pre(block)
        block = block ? null : []
        continue
      }
      if (block) {
        block.push(raw)
        continue
      }
      let m
      if ((m = /^\s*(\*\*)?&gt;\s?(.*)$/.exec(raw))) {
        // GitHub's [!NOTE] and the like: the quote says it already.
        if (!/^\[!(note|tip|important|warning|caution)\]$/i.test(m[2].trim())) html += `<div class="md-q">${inline((m[1] || '') + m[2]) || '&nbsp;'}</div>`
        continue
      }
      const line = inline(raw)
      if ((m = /^\s*([-*•]|\d+[.)])\s+(.*)$/.exec(line))) html += `<div class="md-li"><span>${m[1] === '*' ? '•' : m[1]}</span><span>${m[2]}</span></div>`
      else if ((m = /^#{1,4}\s+(.*)$/.exec(line))) html += `<div class="md-h">${m[1]}</div>`
      else html += line.trim() ? `<div>${line}</div>` : '<div class="md-gap"></div>'
    }
    return block ? html + pre(block) : html
  }

  // The talk under a letter: who with, how it is going, what was said (the
  // agent's words as they come, what it did in small), and a box to ask more.
  function talkHTML(talk, letter) {
    const agent = AGENTS[talk.agent] || 'Agent'
    const going = talk.state === 'running'
    const state = going ? T('mail.talk.going', { agent }) : talk.state === 'failed' ? T('mail.talk.failed', { agent, why: talk.error || '' }) : T('mail.talk.done', { agent })
    const turns = talk.turns
      .map((t, i) => {
        if (t.who === 'start') return `<div class="t-start">${esc(T('mail.talk.start', { agent }))}</div>`
        if (t.who === 'me') return `<div class="t-me">${esc(t.text)}</div>`
        if (t.who === 'tool') return `<div class="t-tool">· ${esc(t.text)}</div>`
        // What it said, once said, to reply with.
        const writing = going && i === talk.turns.length - 1
        return `<div class="t-agent">${md(t.text)}</div>${writing ? '' : `<div class="t-use"><span class="link" data-w-use="${i}">${esc(T('mail.w.useReply'))}</span></div>`}`
      })
      .join('')
    const thinking = going && talk.turns.at(-1)?.who !== 'agent' ? `<div class="t-wait">${esc(T('mail.talk.thinking'))}</div>` : ''
    const ask = `<div class="t-ask"><input class="in" data-talk="${esc(letter.key)}" placeholder="${esc(T('mail.talk.ask', { agent }))}" ${going ? 'disabled' : ''} spellcheck="false"><button class="pbtn sm al" data-talk-send ${going ? 'disabled' : ''}>${esc(T('mail.talk.send'))}</button></div>`
    return `<div class="talk ${talk.state}"><div class="talk-h"><span class="grow">${esc(state)}</span>${going ? '' : `<span class="link" data-hand="${talk.agent}:talk">${esc(T('mail.talk.again'))}</span>`}</div>${turns}${thinking}${ask}</div>`
  }

  // The Mail page's agents as the dropdown has them: [id, name, why not].
  const agentChoices = () => mailList().map(a => [a.id, a.name, a.found ? '' : T(`mail.agentMissing.${a.id}`)])

  // The buttons above an open letter: its star, reply, hand it to the first
  // agent (when no talk is there yet), and ⋯ for the rest. Handing it is a
  // split button: its left part to the first agent, ▾ to another one found
  // for this letter only, or that one made the first (handMenu).
  function handButtons(l, talk) {
    const ag = firstAgent()
    const off = !(snap.mailAgents || {})[ag] ? 'disabled' : ''
    const others = mailList().filter(a => a.found && a.id !== ag)
    const more = others.length ? `<button class="pbtn sm al more" data-hand-more title="${esc(T('mail.hand.others'))}">▾</button>` : ''
    const give = talk ? '' : `<span class="split${more ? ' two' : ''}"><button class="pbtn sm al" data-hand="${ag}:talk" ${off}>${esc(T('mail.hand.talk', { agent: AGENTS[ag] }))}</button>${more}</span>`
    const reply = `<button class="pbtn sm" data-w-reply="reply">${esc(T('mail.reply'))}</button>`
    return `${starHTML(l.account, l.uid, l.flagged)}${reply}${give}<button class="pbtn sm" data-mail-menu title="${esc(T('mail.hand.more'))}">⋯</button>`
  }

  // --- Writing a letter ----------------------------------------------------------------------

  const personLine = p => (p.name ? `${/[,;"<>]/.test(p.name) ? `"${p.name.replace(/"/g, '')}"` : p.name} <${p.address}>` : p.address)
  const peopleLine = list => list.map(personLine).join(', ')
  const samePerson = (a, b) => a.address.toLowerCase() === b.address.toLowerCase()
  const eachOnce = list => list.filter((p, i) => p.address && list.findIndex(q => samePerson(p, q)) === i)
  const quoted = text => String(text || '').split('\n').map(line => (line ? `> ${line}` : '>')).join('\n')

  // The letter passed on, as mail programs write it under the new one.
  function forwarded(l) {
    const head = [
      `---------- ${T('mail.w.fwdHead')} ----------`,
      `${T('mail.w.from')}: ${people(l.from)}`,
      `${T('mail.w.date')}: ${mailDate(l.date, true)}`,
      `${T('mail.w.subject')}: ${l.subject || ''}`,
      `${T('mail.to')}: ${people(l.to)}`,
      l.cc.length ? `${T('mail.cc')}: ${people(l.cc)}` : '',
    ]
    return `${head.filter(Boolean).join('\n')}\n\n${l.text || ''}`
  }

  // The account a new letter goes from: the one whose inbox shows, if it is
  // on; else the first that is on and has an outgoing server, the first on,
  // the first.
  function fromAccount() {
    const all = snap.mail || []
    const shown = accountOf(inboxId())
    if (shown?.on) return shown.id
    return (all.find(a => a.on && a.smtp) || all.find(a => a.on) || all[0])?.id
  }

  // A letter begun: new (from the account fromAccount picks), a reply to
  // the sender or to all (from the account it came to; the letter quoted
  // under it, words an agent wrote on top, if given), or one passed on
  // (with its attachments). One half written is not thrown away for it:
  // it shows.
  function write(kind, l, words) {
    closeMenu()
    if (draft && (draft.touched || draft.files.length)) {
      draft.note = T('mail.w.unsent')
      writing = true
      return redrawMail()
    }
    const account = accountOf(l?.account)?.id || fromAccount()
    if (!account) return
    const me = (accountOf(account)?.address || '').toLowerCase()
    const notMe = list => (list || []).filter(p => p.address && p.address.toLowerCase() !== me)
    const d = { kind, account, to: '', cc: '', bcc: '', showCc: false, subject: '', text: '', reply: null, forward: null, files: [], touched: !!words, busy: false, error: null, note: null }
    if (l && (kind === 'reply' || kind === 'all')) {
      // To whom it asks replies to go, else who wrote it; one's own letter
      // (in Sent), to whom it went.
      const mine = (l.from || []).some(p => (p.address || '').toLowerCase() === me)
      const back = l.replyTo?.length ? l.replyTo : mine && l.to?.length ? l.to : l.from
      const everyone = eachOnce([...notMe(back), ...notMe(l.to)])
      const to = kind === 'all' && everyone.length ? everyone : back
      d.to = peopleLine(to)
      if (kind === 'all') d.cc = peopleLine(eachOnce(notMe(l.cc)).filter(p => !to.some(t => samePerson(t, p))))
      d.showCc = !!d.cc
      d.subject = /^\s*(re|aw|sv|回复|答复)\s*[:：]/i.test(l.subject || '') ? l.subject : `Re: ${l.subject || ''}`
      d.text = `${words ? `${words}\n\n` : '\n\n'}${T('mail.w.wrote', { date: mailDate(l.date, true), who: people(l.from) })}\n${quoted(l.text)}`
      d.reply = { account: l.account, folder: l.folder || 'inbox', uid: l.uid, messageId: l.messageId, references: l.references || [] }
    }
    if (l && kind === 'forward') {
      d.subject = /^\s*(fwd?|wg|转发)\s*[:：]/i.test(l.subject || '') ? l.subject : `Fwd: ${l.subject || ''}`
      d.text = `\n\n${forwarded(l)}`
      d.forward = { account: l.account, folder: l.folder || 'inbox', uid: l.uid, files: (l.attachments || []).map((f, i) => ({ index: i, name: f.name, size: f.size })) }
    }
    draft = d
    writing = true
    dropSure = false
    keepDraft()
    draw(true)
    relayout()
    // The caret where one writes first: the people, or above the quote.
    requestAnimationFrame(() => {
      const box = body.querySelector(kind === 'reply' || kind === 'all' ? '[data-w="text"]' : '[data-w="to"]')
      box?.focus()
      if (box?.matches('textarea')) {
        box.setSelectionRange(0, 0)
        box.scrollTop = 0
      }
    })
  }

  // The same about a letter in the list: opened first (for its text).
  async function writeAbout(kind, account, uid, folder, words) {
    if (!(reading?.letter && isReading(account, folder, uid))) await openLetter(account, uid, folder)
    if (reading?.letter && isReading(account, folder, uid)) write(kind, reading.letter, words)
  }

  // The reply in an agent's words: the longest ``` block, if it wrote one; else all of it.
  function replyIn(text) {
    const blocks = [...String(text || '').matchAll(/```[^\n]*\n([\s\S]*?)```/g)].map(m => m[1].trim())
    return (blocks.sort((a, b) => b.length - a.length)[0] || String(text || '')).trim()
  }

  // The letter being written: back (it stays), discard, send; from
  // (where there are several accounts), to, cc and bcc (when asked for),
  // the subject, the text; the files, each with ×, and more.
  function writeHTML() {
    const d = draft
    const all = snap.mail || []
    const off = d.busy ? 'disabled' : ''
    // Back: to the letter answered, or the inbox; beside the list (two
    // panes), put aside.
    const back = twoPanes() ? 'mail.w.aside' : d.reply || d.forward ? (reading?.letter ? 'mail.w.backLetter' : 'mail.inbox') : 'mail.inbox'
    const bar = `<div class="mail-bar"><span class="link" data-w-back>‹ ${esc(T(back))}</span><span class="grow"></span>
      <button class="pbtn sm" data-w-drop ${off}>${esc(T(dropSure ? 'mail.w.dropSure' : 'mail.w.drop'))}</button>
      <button class="pbtn sm al" data-w-send title="${esc(T('mail.w.ctrlEnter'))}" ${off}>${esc(T(d.busy ? 'mail.w.sending' : 'mail.w.send'))}</button></div>`
    const box = (key, label, extra = '') =>
      `<div class="fr"><span class="fl">${esc(label)}</span><input class="in" data-w="${key}" value="${esc(d[key] ?? '')}" spellcheck="false" autocomplete="off" ${off}>${extra}</div>`
    const address = accountOf(d.account)?.address || ''
    const from =
      all.length > 1
        ? `<div class="fr"><span class="fl">${esc(T('mail.w.from'))}</span><button class="pick" data-w-from ${off}>${esc(address)} ▾</button></div>`
        : `<div class="fr"><span class="fl">${esc(T('mail.w.from'))}</span><span class="d ellip">${esc(address)}</span></div>`
    const ccLink = d.showCc ? '' : `<span class="link" data-w-cc>${esc(T('mail.w.ccBcc'))}</span>`
    const files = [
      ...(d.forward?.files || []).map((f, i) => ({ ...f, key: `o${i}` })),
      ...d.files.map((f, i) => ({ ...f, key: `f${i}` })),
    ]
      .map(f => `<span class="chip">📎 ${esc(f.name || '?')} · ${mailSize(f.size || 0)}<button class="unfile" data-w-unfile="${f.key}" title="${esc(T('mail.w.unfile'))}" ${off}>×</button></span>`)
      .join('')
    const notes = [d.note ? `<div class="note">${esc(d.note)}</div>` : '', d.error ? `<div class="note err">${esc(d.error)}</div>` : ''].join('')
    return `${bar}${sec(T(`mail.w.kind.${d.kind || 'new'}`))}<div class="grp mail-write">${notes}
      ${from}
      ${box('to', T('mail.to'), ccLink)}
      ${d.showCc ? box('cc', T('mail.cc')) + box('bcc', T('mail.w.bcc')) : ''}
      ${box('subject', T('mail.w.subject'))}
      <textarea class="in ta" data-w="text" rows="10" spellcheck="false" ${off}>${esc(d.text)}</textarea>
      <div class="w-files">${files}<button class="pbtn sm" data-w-attach ${off}>📎 ${esc(T('mail.w.attach'))}</button><input type="file" multiple hidden data-w-file></div>
    </div>`
  }

  // Files picked, read for the letter; no more than it may take.
  const readBase64 = file =>
    new Promise((ok, no) => {
      const r = new FileReader()
      r.onload = () => ok(String(r.result).split(',')[1] || '')
      r.onerror = () => no(r.error)
      r.readAsDataURL(file)
    })

  async function addFiles(input) {
    picking = false
    const d = draft
    const list = [...(input.files || [])]
    input.value = ''
    if (!d || !list.length) return
    let total = [...d.files, ...(d.forward?.files || [])].reduce((n, f) => n + (f.size || 0), 0)
    d.error = null
    for (const file of list) {
      if (total + file.size > MOST_FILES) {
        d.error = T('mail.err.tooBig')
        break
      }
      try {
        d.files.push({ name: file.name, type: file.type || 'application/octet-stream', size: file.size, data: await readBase64(file) })
        total += file.size
      } catch {
        d.error = T('mail.err.file', { what: file.name })
      }
    }
    d.touched = true
    redrawMail()
  }

  // Why a letter did not go, in words: the page's own reasons, or the
  // outgoing server's.
  function sendError(got) {
    const err = got.error || {}
    const own = ['address', 'noOne', 'tooMany', 'tooBig', 'tooLong', 'file', 'noSmtp', 'gone', 'noLetter']
    if (own.includes(err.kind)) return T(`mail.err.${err.kind}`, { what: err.text || '' })
    const a = accountOf(draft?.account)
    if (!got.out) return mailError(err, a)
    const hint = err.kind === 'login' && providerOf(a?.host) ? ` ${T(`mail.hint.${providerOf(a.host)}`)}` : ''
    return T('mail.w.outPrefix') + mailError(err, a?.smtp || { host: '?', port: '' }) + hint
  }

  // Sent: the letter gone from here, back where one was, and what came of
  // the copy in Sent.
  async function sendDraft() {
    const d = draft
    if (!d || d.busy) return
    if (!`${d.to}${d.cc}${d.bcc}`.trim()) {
      d.error = T('mail.err.noOne')
      return redrawMail('[data-w="to"]')
    }
    Object.assign(d, { busy: true, error: null, note: null })
    dropSure = false
    redrawMail()
    const letter = {
      account: d.account,
      to: d.to,
      cc: d.cc,
      bcc: d.bcc,
      subject: d.subject,
      text: d.text,
      reply: d.reply,
      forward: d.forward ? { account: d.forward.account, folder: d.forward.folder || 'inbox', uid: d.forward.uid, keep: d.forward.files.map(f => f.index) } : null,
      files: d.files.map(f => ({ name: f.name, type: f.type, data: f.data })),
    }
    const got = await window.pet.mail.send(letter)
    if (draft !== d) return
    d.busy = false
    if (!got.ok) {
      d.error = sendError(got)
      return redrawMail()
    }
    // The letter answered shows it (the list comes again with the reply in it).
    const answered = d.reply && listed(d.reply.account, d.reply.folder, d.reply.uid)
    if (answered) answered.answered = true
    draft = null
    writing = false
    keepDraft()
    draw(true)
    relayout()
    noteFor({ text: T(`mail.w.sent.${got.kept || 'server'}`), bad: got.kept === 'failed' })
  }

  // A letter open: back to the inbox, its star and the hand-off, who, when,
  // its attachments, the talk about it, the text.
  function readingHTML() {
    const r = reading
    const l = r.letter
    const talk = l ? talkOf(l.key) || r.talk : null
    // Beside the list (two panes) there is no inbox to go back to.
    const back = twoPanes() ? '' : `<span class="link" data-mail-back>‹ ${esc(T('mail.inbox'))}</span>`
    const bar = `<div class="mail-bar">${back}<span class="grow"></span>${l ? handButtons(l, talk) : ''}</div>`
    const note = handNote && !twoPanes() ? `<div class="note${handNote.bad ? ' err' : ''}">${esc(handNote.text)}</div>` : ''
    if (r.busy) return `${bar}<div class="grp"><div class="note">${esc(T('mail.opening'))}</div></div>`
    if (r.error) return `${bar}<div class="grp"><div class="note err">${esc(handError(r.error))}</div></div>`
    const files = l.attachments.length ? `<div class="m-files">${l.attachments.map(f => `<span class="chip">📎 ${esc(f.name || '?')} · ${mailSize(f.size)}</span>`).join('')}</div>` : ''
    // Whose (several accounts), and one's own, from Sent.
    const address = accountOf(l.account)?.address || ''
    const inSent = l.folder === 'sent' ? T('mail.inSent', { address }) : ''
    const whose = inSent || (snap.mail || []).length > 1 ? `<div class="d">${esc(inSent || T('mail.in', { address }))}</div>` : ''
    return `${bar}${note}<div class="grp mail-read" ${where(l)}>
      <div class="m-subj">${esc(l.subject || T('mail.noSubject'))}</div>
      <div class="d">${esc(people(l.from))} · ${esc(mailDate(l.date, true))}</div>
      ${l.to.length ? `<div class="d">${esc(T('mail.to'))}: ${esc(people(l.to))}</div>` : ''}
      ${l.cc.length ? `<div class="d">${esc(T('mail.cc'))}: ${esc(people(l.cc))}</div>` : ''}
      ${whose}${files}${talk ? talkHTML(talk, l) : ''}
      <div class="m-text">${esc(l.text)}</div>${l.cut ? `<div class="d">${esc(T('mail.cut'))}</div>` : ''}
    </div>`
  }

  // Mail: each account with its switch and setting one up; the inbox
  // (every account's, or one), and a letter open or being written. In two
  // panes the inbox is on the left, and on the right the letter being
  // written, the one open, or (neither) the accounts and the agents; in one,
  // each takes the page.
  function pageMail() {
    const id = inboxId()
    if (twoPanes()) {
      if (id && inbox.for !== `${id}|${inbox.filter}` && !inbox.busy) setTimeout(() => loadInbox(false))
      const left = id ? inboxHTML(id) : `<div class="note">${esc(T('mail.noBox'))}</div>`
      const note = handNote ? `<div class="note${handNote.bad ? ' err' : ''}">${esc(handNote.text)}</div>` : ''
      const right = writing && draft ? writeHTML() : reading ? readingHTML() : `${accountsHTML()}${id ? agentsHTML() : ''}`
      return `<div class="panes"><div class="pane-l">${left}</div><div class="pane-r">${note}${right}</div></div>`
    }
    if (writing && draft) return writeHTML()
    if (reading) return readingHTML()
    // The inbox comes in the first time it is shown (and for another one or filter).
    if (id && inbox.for !== `${id}|${inbox.filter}` && !inbox.busy) setTimeout(() => loadInbox(false))
    return `${accountsHTML()}${id ? inboxHTML(id) + agentsHTML() : ''}`
  }

  // The accounts: each with its switch and its settings; the form for one.
  function accountsHTML() {
    const accounts = snap.mail || []
    const rows = accounts
      .map(a => {
        const note = mailNote(a)
        return `<div class="r"><span class="wi" style="color:${a.on ? '#5e9bff' : COLOR.idle}">${Widgets.icon('mail')}</span><div class="grow"><div class="ellip">${esc(a.address)}</div><div class="d${note.bad ? ' bad' : ''}">${esc(note.text)}</div></div><button class="pbtn sm" data-mail-edit="${esc(a.id)}">${esc(T('pl.settings'))}</button>${sw('mail:' + a.id, a.on)}</div>`
      })
      .join('')
    const add = mailForm ? '' : `<div class="r"><button class="pbtn wide" data-mail-add>${esc(T('mail.add'))}</button></div>`
    return `${sec(T('mail.section'))}<div class="grp"><div class="note">${esc(T('mail.note'))}</div>${rows}${add}</div>
      ${accounts.length ? offlineHTML() : ''}${proxyHTML()}${mailForm ? mailFormHTML() : ''}`
  }

  // How much is kept whole on this computer (the setting mailOffline: the
  // last year unless set), and the room every account's takes now.
  function offlineHTML() {
    const value = ['90d', 'year', 'all', 'opened'].includes(snap.settings.mailOffline) ? snap.settings.mailOffline : 'year'
    const used = (snap.mail || []).reduce((n, a) => n + (a.status?.offline?.bytes || 0), 0)
    const note = T(`mail.keep.${value}Note`) + (used ? ` ${T('mail.keep.used', { size: mailSize(used) })}` : '')
    const choices = ['90d', 'year', 'all', 'opened'].map(v => [v, T(`mail.keep.${v}`)])
    return `<div class="grp keep">${row(esc(T('mail.keep')), esc(note), seg('mailOffline', choices, value))}</div>`
  }

  // How mail reaches its servers (the setting mailProxy, mail/proxy.rs):
  // through Windows' proxy (the default; straight while none is set),
  // through one typed in (HTTP or SOCKS5), or straight. A manual one's kind
  // and address are kept while another is on.
  function proxySetting() {
    const p = snap.settings.mailProxy || {}
    return { mode: ['system', 'manual', 'none'].includes(p.mode) ? p.mode : 'system', kind: p.kind === 'socks5' ? 'socks5' : 'http', address: p.address || '' }
  }

  function proxyHTML() {
    const p = proxySetting()
    const sys = snap.mailSystemProxy
    const now = p.mode === 'system' ? (sys ? T('mail.proxy.systemNow', { proxy: sys.address, kind: labelOf(PROXY_KINDS, sys.kind) }) : T('mail.proxy.systemNone')) : T(`mail.proxy.${p.mode}Note`)
    const modes = ['system', 'manual', 'none'].map(m => [m, T(`mail.proxy.${m}`)])
    const typed = proxyTyped ? proxyTyped.text : p.address
    const manual =
      p.mode === 'manual'
        ? `<div class="r">${seg('mailProxy.kind', PROXY_KINDS, p.kind)}<input class="in grow" data-proxy-at value="${esc(typed)}" placeholder="127.0.0.1:7890" spellcheck="false" autocomplete="off"></div>${proxyTyped?.bad ? `<div class="note err">${esc(T('mail.proxy.bad'))}</div>` : ''}`
        : ''
    return `<div class="grp proxy">${row(esc(T('mail.proxy')), esc(`${now} ${T('mail.proxy.perAccount')}`), seg('mailProxy.mode', modes, p.mode))}${manual}</div>`
  }

  // The Mail page's proxy changed, kept whole; manual for the first time,
  // Windows' proxy's address, else Clash's usual port, to change.
  function setProxy(change) {
    const next = { ...proxySetting(), ...change }
    if (next.mode === 'manual' && !next.address) {
      const sys = snap.mailSystemProxy
      Object.assign(next, sys ? { kind: sys.kind, address: sys.address } : { address: '127.0.0.1:7890' })
    }
    proxyTyped = null
    patch({ mailProxy: next })
  }

  // Its address as typed (Enter, or leaving the box): a scheme before it
  // (socks5://, http://) says the kind; kept once it reads as address:port.
  function proxyAddress(text) {
    let address = String(text || '').trim()
    let kind = proxySetting().kind
    const scheme = /^([a-z0-9]+):\/\//i.exec(address)
    if (scheme) {
      kind = /^socks/i.test(scheme[1]) ? 'socks5' : 'http'
      address = address.slice(scheme[0].length).replace(/\/+$/, '')
    }
    const port = Number(address.split(':').pop())
    if (!/^(\[[0-9a-f:.]+\]|[a-z0-9._-]+):\d{1,5}$/i.test(address) || !(port > 0 && port < 65536)) {
      proxyTyped = { text, bad: true }
      return draw()
    }
    if (address === proxySetting().address && kind === proxySetting().kind) {
      proxyTyped = null
      return draw()
    }
    setProxy({ kind, address })
  }

  // The letters as the list shows them: a conversation folded in is the
  // letter a click on it opens; folded out, each of its letters.
  function shown() {
    return inbox.threads.flatMap(t => {
      if (t.count > 1 && expanded.has(t.key)) return t.letters.map(l => ({ l, t }))
      const l = t.letters.find(x => isLetter(x, t.account, t.open?.folder, t.open?.uid)) || t.letters.at(-1)
      return [{ l, t }]
    })
  }

  // The letter beside this one in the list (two panes, ↑ ↓), opened.
  function step(by) {
    const all = shown()
    const at = all.findIndex(({ l, t }) => isReading(l.account, l.folder, l.uid) || (!expanded.has(t.key) && t.letters.some(x => isReading(x.account, x.folder, x.uid))))
    const next = all[at < 0 ? (by > 0 ? 0 : all.length - 1) : at + by]?.l
    if (!next) return
    openLetter(next.account, next.uid, next.folder)
    requestAnimationFrame(() => body.querySelector(`.inbox [data-letter="${next.uid}"][data-folder="${next.folder}"][data-account="${CSS.escape(next.account)}"]`)?.scrollIntoView({ block: 'nearest' }))
  }

  // The inbox's conversations as kept, as filtered; with more, 50 more.
  // Asked again, quietly, whenever what is kept changed (snap.mailRev).
  async function loadInbox(more, quiet = false) {
    const id = inboxId()
    if (!id || inbox.busy) return
    const which = `${id}|${inbox.filter}`
    if (inbox.for !== which) Object.assign(inbox, { threads: [], total: 0, letters: 0, count: 50, error: null, errors: [], rev: 0 })
    if (more) inbox.count += 50
    const rev = snap?.mailRev || 0
    Object.assign(inbox, { id, for: which, busy: true, error: null })
    if (!quiet) redrawMail()
    const got = await window.pet.mail.letters(id, inbox.count, inbox.filter)
    inbox.busy = false
    if (inbox.for !== which) return redrawMail()
    if (got.ok) Object.assign(inbox, { threads: got.threads, total: got.total, letters: got.letters, errors: got.errors || [], syncing: got.syncing, rev })
    else inbox.error = got.error
    redrawMail()
    // It changed again meanwhile.
    if (got.ok && (snap?.mailRev || 0) !== rev) loadInbox(false, true)
  }

  // A letter opened: read whole, from what is kept (or the server), and
  // marked read.
  async function openLetter(account, uid, folder = 'inbox') {
    if (!accountOf(account)) return
    reading = { account, folder, uid, busy: true }
    draw(true)
    relayout()
    const got = await window.pet.mail.letter(account, uid, folder)
    if (!isReading(account, folder, uid)) return
    reading = got.ok ? { account, folder, uid, letter: got.letter, talk: got.talk } : { account, folder, uid, error: got.error }
    const l = listed(account, folder, uid)
    if (l && got.ok) Object.assign(l, { seen: true, flagged: got.letter.flagged })
    draw()
    relayout()
  }

  // Why a letter could not be had or handed on, in words.
  function handError(err, agent) {
    const kind = err?.kind || 'other'
    const own = ['noAgent', 'start', 'save', 'noLetter', 'gone', 'busy', 'noTalk', 'noSession', 'empty']
    if (!own.includes(kind)) return mailError(err, accountOf(reading?.account))
    return T(`mail.err.${kind}`, { agent: AGENTS[agent] || err.text || '' }) + (kind === 'start' || kind === 'save' ? `${lang === 'zh' ? '：' : ': '}${err.text}` : '')
  }

  function noteFor(note) {
    clearTimeout(handNoteTimer)
    handNote = note
    redrawMail()
    if (note) handNoteTimer = setTimeout(() => noteFor(null), 10000)
  }

  // A letter handed to an agent: a talk about it under the letter (opened,
  // if it was not), or a session in a terminal.
  async function hand(account, uid, agent, how, folder = 'inbox') {
    closeMenu()
    if (how === 'talk' && !isReading(account, folder, uid)) await openLetter(account, uid, folder)
    noteFor({ text: T('mail.hand.busy', { agent: AGENTS[agent] }) })
    const got = await window.pet.mail.hand(account, uid, agent, how, folder)
    if (got.ok && how === 'talk' && reading?.letter && isReading(account, folder, uid)) reading.talk = null
    noteFor(got.ok ? (how === 'open' ? { text: T('mail.hand.opened', { agent: AGENTS[agent] }) } : null) : { text: handError(got.error, agent), bad: true })
  }

  // More asked in a letter's talk.
  async function say(box) {
    const text = box.value.trim()
    if (!text) return
    box.value = ''
    const got = await window.pet.mail.say(box.dataset.talk, text)
    if (!got.ok) {
      box.value = text
      noteFor({ text: handError(got.error), bad: true })
    }
  }

  // A star put on a letter or taken off: at once on the page, then on the
  // server (back as it was if that failed).
  async function star(account, uid, folder = 'inbox') {
    const inList = listed(account, folder, uid)
    const open = reading?.letter && isReading(account, folder, uid) ? reading.letter : null
    const on = !(inList || open)?.flagged
    for (const l of [inList, open]) if (l) l.flagged = on
    redrawMail()
    const got = await window.pet.mail.flag(account, uid, on, folder)
    if (!got.ok) {
      for (const l of [inList, open]) if (l) l.flagged = !on
      noteFor({ text: mailError(got.error, accountOf(account)), bad: true })
    }
  }

  // The menu a right-click on a letter opens: mail's AI (a talk, or a
  // terminal), greyed out when not found; its star.
  const menuEl = document.createElement('div')
  menuEl.className = 'mmenu'
  menuEl.hidden = true
  layer.append(menuEl)

  function openMenu(account, uid, folder, x, y) {
    const found = snap.mailAgents || {}
    const at = `data-account="${esc(account)}" data-folder="${folder || 'inbox'}" data-uid="${uid}"`
    const item = (ag, how, label) =>
      `<button class="mi" data-hand="${ag}:${how}" ${at} ${found[ag] ? '' : 'disabled'}>${esc(label)}${found[ag] ? '' : ` <span class="d">${esc(T('mail.hand.missing'))}</span>`}</button>`
    const ag = firstAgent()
    const talks = item(ag, 'talk', T('mail.hand.talk', { agent: AGENTS[ag] }))
    const terminals = item(ag, 'open', T('mail.hand.open', { agent: AGENTS[ag] }))
    const l = listed(account, folder, uid) || (isReading(account, folder, uid) ? reading.letter : null)
    const starItem = `<button class="mi" data-star="${esc(account)}" data-folder="${folder || 'inbox'}" data-uid="${uid}">${esc(T(l?.flagged ? 'mail.unstar' : 'mail.star'))}</button>`
    const replies = [
      ['reply', 'mail.reply'],
      ['all', 'mail.replyAll'],
      ['forward', 'mail.forward'],
    ]
      .map(([kind, key]) => `<button class="mi" data-w-reply="${kind}" ${at}>${esc(T(key))}</button>`)
      .join('')
    menuEl.innerHTML = `${replies}<div class="mi-sep"></div>${talks}<div class="mi-sep"></div>${terminals}<div class="mi-sep"></div>${starItem}`
    menuPick = null
    placeMenu(x, y)
  }

  // The same menu as a picker: a model or an effort, the one set ticked.
  let menuPick = null

  function placeMenu(x, y) {
    menuEl.hidden = false
    const box = layer.getBoundingClientRect()
    menuEl.style.left = `${Math.max(4, Math.min(x - box.left, box.width - menuEl.offsetWidth - 4))}px`
    menuEl.style.top = `${Math.max(4, Math.min(y - box.top, box.height - menuEl.offsetHeight - 4))}px`
  }

  // A menu of [value, label, off] under a button, the first (the agent's or
  // server's own, autodetect, every account) apart unless `together`; one
  // that is off is there, greyed, with why (off's words, else "not
  // supported").
  function openChoices(button, choices, value, onPick, together = false) {
    menuEl.innerHTML = choices
      .map(
        ([v, label, off], i) =>
          `${i === 1 && !together ? '<div class="mi-sep"></div>' : ''}<button class="mi${v === value ? ' on' : ''}" data-pick="${esc(v)}" ${off ? 'disabled' : ''}>${esc(label)}${off ? ` <span class="d">${esc(typeof off === 'string' ? off : T('mail.unsupported'))}</span>` : ''}</button>`,
      )
      .join('')
    menuPick = onPick
    const r = button.getBoundingClientRect()
    placeMenu(r.left, r.bottom + 4)
  }

  // The split button's ▾ on an open letter: the other agents found, each
  // to hand this letter to (the first agent stays as it is), and under
  // them each made the first instead.
  function handMenu(button) {
    const ag = firstAgent()
    const others = mailList().filter(a => a.found && a.id !== ag)
    menuEl.innerHTML = `${others.map(a => `<button class="mi" data-pick="hand:${a.id}">${esc(T('mail.hand.talk', { agent: a.short }))}</button>`).join('')}<div class="mi-sep"></div>${others
      .map(a => `<button class="mi" data-pick="first:${a.id}">${esc(T('mail.hand.makeFirst', { agent: a.short }))}</button>`)
      .join('')}`
    menuPick = v => {
      const [what, id] = v.split(':')
      if (what === 'first') return patch({ mailAgent: id })
      if (reading) hand(reading.account, Number(reading.uid), id, 'talk', reading.folder || 'inbox')
    }
    const r = button.getBoundingClientRect()
    placeMenu(r.right - 220, r.bottom + 4)
  }

  function openPicker(button) {
    const [ag, key] = button.dataset.picker.split('.')
    openChoices(button, choicesOf(ag, key), confOf(ag)[key] || '', v => setConf(ag, key, v))
  }

  function closeMenu() {
    menuEl.hidden = true
    menuPick = null
  }

  layer.addEventListener('contextmenu', e => {
    const letter = tab === 'mail' && e.target.closest('[data-letter]')
    if (!letter || e.target.closest('.talk')) return
    // Ours, not the island's menu.
    e.preventDefault()
    e.stopPropagation()
    openMenu(letter.dataset.account, Number(letter.dataset.letter), letter.dataset.folder || 'inbox', e.clientX, e.clientY)
  })
  body.addEventListener('scroll', closeMenu)

  // --- The Agent page (agents.js) ---------------------------------------------------------
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
  let connecting = ''
  // Why each could not be written: { error, kind, folder } (locked, invalid, path, other).
  const failed = {}
  let pluginOpen = false
  let agentOpen = ''
  let othersOpen = false
  let connectingAll = false
  const known = () => ({ busy: connecting, failed })

  // One written or removed (its call in bridge.js, agents.js call): what
  // came back shown; not written, why (the card names the file), its row
  // open to say so, and Claude Code's plugin the other way.
  async function agentDo(id, action) {
    const a = Agents.byId(id)
    if (!a || connecting) return
    if (action === 'install') {
      connecting = id
      delete failed[id]
      draw()
    }
    const got = await window.pet.settings[a.call](action)
    connecting = ''
    snap = got.snapshot
    if (action === 'install' && !got.ok) {
      failed[id] = { error: got.error || '', kind: got.kind || 'other', folder: got.folder || '' }
      if (id === 'claude') pluginOpen = true
      if (tab === 'agents') agentOpen = id
    }
    draw()
    relayout()
    return got.ok
  }

  // Connect all: each found and not connected, one after another.
  async function connectAll() {
    if (connectingAll) return
    connectingAll = true
    for (const id of Agents.toConnect(snap, known())) await agentDo(id, 'install')
    connectingAll = false
    draw()
  }

  const capTip = (cap, key) => T('ag.capTip', { cap: T(`ag.cap.${cap}`), what: T(key) })
  const capIcon = (cap, level, key) => `<span class="cap ${level}" title="${esc(capTip(cap, key))}">${Agents.CAP_GLYPH[cap]}</span>`

  // How one stands, in its row's second line.
  function agentLine(id, state) {
    const r = Agents.read(snap, id)
    if (state === 'on') {
      const n = Agents.sessionsOf(snap, id)
      return n ? T(n === 1 ? 'ag.st.on1' : 'ag.st.onN', { n }) : T('ag.st.on')
    }
    if (state === 'set' || state === 'step') return T(`ag.st.set.${id}`)
    if (state === 'busy') return T('wc.busy')
    if (state === 'failed') return T('ag.st.failed')
    if (state === 'repair' || state === 'error') return T(Agents.wordOf(id, r.status))
    return T('ag.st.none')
  }

  function agentRow(id) {
    const a = Agents.byId(id)
    const r = Agents.read(snap, id)
    const state = Agents.stateOf(snap, id, known())
    const action = Agents.actionOf(snap, id, known())
    const open = agentOpen === id
    const small = [a.aka, r.version].filter(Boolean).map(text => `<span class="ver">${esc(text)}</span>`).join('')
    const line = agentLine(id, state)
    // A button only where the person has something to do (agents.js
    // actionOf); else a faint › saying the row opens.
    let end = `<span class="fi chev${open ? ' open' : ''}">\uE76C</span>`
    if (state === 'busy') end = `<button class="pbtn sm" disabled><span class="spin"></span>${esc(T('wc.busy'))}</button>`
    else if (action?.act === 'open') end = `<button class="pbtn sm" data-ag-open="${id}" data-ag-keep>${esc(T(action.label))}</button>`
    else if (action) end = `<button class="pbtn sm" data-agent="${id}:${action.act}" ${connecting ? 'disabled' : ''}>${esc(T(action.label))}</button>`
    const caps = `<span class="caps">${Agents.capsOf(snap, id).map(([cap, level, key]) => capIcon(cap, level, key)).join('')}</span>`
    return `<div class="r ag-row${open ? ' open' : ''}" data-ag-open="${id}"><span class="dot" style="background:${COLOR[Agents.MOOD[state]]}"></span><div class="grow"><div class="ag-name ellip">${esc(a.name)}${small}</div><div class="d ellip" title="${esc(line)}">${esc(line)}</div></div>${caps}<span class="ag-end">${end}</span></div>${open ? agentDetails(id, state) : ''}`
  }

  // What the person must do now, in amber at the top of the details (as
  // its dot): Codex's trust, its background hooks, an old Codex, ZCode's
  // hooks turned off, Claude Code's two ways at once, a file unreadable.
  function agentSteps(id, state) {
    const r = Agents.read(snap, id)
    const warn = []
    if (id === 'codex' && state === 'step') warn.push(T('ag.trust.codex'))
    if (id === 'codex' && r.status === 'upgradable') warn.push(T('s.codexUpgradeNote'))
    if (id === 'codex' && r.here && snap.codex?.async === false) warn.push(T('s.codexOld', { version: r.version }))
    if (id === 'zcode' && r.status === 'off') warn.push(T('s.zcodeOffNote'))
    if (id === 'claude' && r.status === 'both') warn.push(T('ag.claude.both'))
    if (state === 'error') warn.push(T('ag.note.unreadable'))
    return warn.map(text => `<div class="ag-note warn">${esc(text)}</div>`).join('')
  }

  // What to know about one: how it is, and what comes next while it waits
  // for its first turn (a new omp session, DSH restarted).
  function agentNotes(id, state) {
    const next = state === 'set' && id === 'omp' ? T('s.ompNew') : state === 'set' && id === 'dsh' ? T('s.dshRestart') : ''
    return `${next ? `<div class="ag-note">${esc(next)}</div>` : ''}<div class="ag-note">${esc(T(Agents.noteOf(id, state)))}</div>`
  }

  // A field of the details: its name, what it is.
  const field = (key, value) => `<div class="kv"><span class="k">${esc(T(key))}</span><span class="v">${value}</span></div>`
  const pathOf = text => `<span class="mono path">${esc(text)}</span>`

  // Where one is written: the file (omp's in each profile too; DSH's hook
  // list in her data folder), its backup.
  function agentFiles(id) {
    const r = Agents.read(snap, id)
    return [
      field('ag.file', `${pathOf(r.shown)}${r.more ? esc(T('s.ompMore', { n: r.more })) : ''}`),
      r.list ? field('ag.list', pathOf(r.list)) : '',
      r.backup ? field('ag.backup', pathOf(`${r.shown}.wakuwaku.bak`)) : '',
    ].join('')
  }

  // Claude Code's two ways, each its own: the plugin (recommended: Claude
  // Code installs and removes it, nothing of ours in its settings) and the
  // hooks written straight into settings.json (new sessions start her).
  function claudeWays() {
    const commands = snap.pluginCommands || []
    const plugged = snap.connection === 'plugin' || snap.connection === 'both'
    const plugin = `<div class="ag-way"><div class="ag-way-h"><b>${esc(T('home.plugin'))}</b><span class="grow"></span><span class="d">${esc(T(plugged ? 'home.pluginOn' : 'ag.claude.pluginOff'))}</span></div>
      <div class="d">${esc(T(plugged ? 'ag.claude.pluginRemove' : 'home.pluginWhy'))}</div>
      ${plugged ? '' : `${commands.map(c => `<div class="wc-cmd mono">${esc(c)}</div>`).join('')}<div class="ag-btns"><button class="pbtn sm" data-copy="${esc(commands.join('\n'))}" data-label="${esc(T('wc.copyBoth'))}">${esc(T('wc.copyBoth'))}</button><span class="d">${esc(T('home.pluginStart'))}</span></div>`}</div>`
    const status = snap.hooks
    const word = { ok: 'settings.hooksOk', missing: 'settings.hooksMissing', stale: 'settings.hooksStale', partial: 'settings.hooksPartial', httpOnly: 'settings.hooksHttpOnly', unreadable: 's.hooksUnreadable' }[status] || 'settings.hooksMissing'
    const busy = connecting === 'claude'
    // Not written while the plugin is on: every event would come twice.
    const buttons =
      status === 'unreadable' || (plugged && status === 'missing')
        ? ''
        : status === 'missing'
          ? `<button class="pbtn sm" data-agent="claude:install" ${connecting ? 'disabled' : ''}>${busy ? `<span class="spin"></span>${esc(T('wc.busy'))}` : esc(T('settings.install'))}</button>`
          : `<button class="pbtn sm" data-agent="claude:install" ${connecting ? 'disabled' : ''}>${esc(T(status === 'ok' || status === 'httpOnly' ? 'settings.reinstall' : 'settings.repair'))}</button><button class="pbtn sm no" data-agent="claude:remove" ${connecting ? 'disabled' : ''}>${esc(T('settings.remove'))}</button>`
    const direct = `<div class="ag-way"><div class="ag-way-h"><b>${esc(T('ag.claude.direct'))}</b><span class="grow"></span><span class="d">${esc(T(plugged && status === 'missing' ? 'ag.claude.notNeeded' : word))}</span></div>
      ${agentFiles('claude')}<div class="d">${esc(T('home.advancedWhy'))}</div>${buttons ? `<div class="ag-btns">${buttons}</div>` : ''}</div>`
    return plugin + direct
  }

  // How one connects, in one word for all (the plugin or the hooks for Claude Code, as connected).
  function viaOf(id) {
    if (id !== 'claude') return T(`ag.via.${Agents.byId(id).via}`)
    const c = snap.connection
    return c === 'plugin' ? T('ag.via.plugin') : c === 'hooks' ? T('ag.via.hooks') : c === 'both' ? `${T('ag.via.plugin')} + ${T('ag.via.hooks')}` : T('ag.via.claudeNone')
  }

  function agentDetails(id, state) {
    const caps = Agents.capsOf(snap, id)
      .map(([cap, level, key]) => `<div class="ag-cap ${level}">${capIcon(cap, level, key)}<span class="ag-cap-n">${esc(T(`ag.cap.${cap}`))}</span><span class="d">${esc(T(key))}</span></div>`)
      .join('')
    const ours = Agents.isIn(snap, id) || state === 'repair'
    const busy = connecting === id
    // Connect, repair or install again; remove, with the file it is taken out of.
    const buttons =
      state === 'error'
        ? ''
        : ours
          ? `<button class="pbtn sm" data-agent="${id}:install" ${connecting ? 'disabled' : ''}>${busy ? `<span class="spin"></span>${esc(T('wc.busy'))}` : esc(T(state === 'repair' ? 'settings.repair' : 'settings.reinstall'))}</button><button class="pbtn sm no" data-agent="${id}:remove" ${connecting ? 'disabled' : ''}>${esc(T('settings.remove'))}</button>`
          : `<button class="pbtn sm al" data-agent="${id}:install" ${connecting ? 'disabled' : ''}>${busy ? `<span class="spin"></span>${esc(T('wc.busy'))}` : esc(T(state === 'failed' ? 'wc.toolRetry' : 'ag.do.install'))}</button>`
    const top = id === 'claude' ? claudeWays() : `${field('ag.via', esc(viaOf(id)))}${agentFiles(id)}`
    return `<div class="ag-det">${agentSteps(id, state)}${failed[id] ? failedHTML(id) : ''}${top}
        <div class="ag-what">${esc(T(`ag.what.${id}`))}</div>
        <div class="ag-caps">${caps}</div>
        ${agentNotes(id, state)}
        ${id === 'claude' ? '' : `<div class="ag-btns">${buttons}</div>`}
      </div>`
  }

  function pageAgents() {
    const here = Agents.here(snap)
    const away = Agents.elsewhere(snap)
    const left = Agents.toConnect(snap, known())
    // Hidden with none left; its tooltip names whom it connects.
    const all = left.length
      ? `<button class="pbtn sm al" data-ag-all title="${esc(T('ag.connectAllTip', { list: toolNames(left, 'and') }))}" ${connecting || connectingAll ? 'disabled' : ''}>${connectingAll ? `<span class="spin"></span>${esc(T('wc.busy'))}` : esc(T('ag.connectAll', { n: left.length }))}</button>`
      : ''
    const rows = here.length ? here.map(agentRow).join('') : `<div class="note">${esc(T('ag.noneHere'))}</div>`
    // What the icons are, once.
    const legend = Agents.CAPS.map(cap => `<span class="ag-key"><span class="cap yes">${Agents.CAP_GLYPH[cap]}</span>${esc(T(`ag.cap.${cap}`))}</span>`).join('')
    const others = away.length
      ? `${sec(T('ag.others'))}<div class="grp">
          <div class="r go" data-ag-others><span class="fi chev${othersOpen || !here.length ? ' open' : ''}">\uE76C</span><div class="grow ellip">${esc(nameList(lang, away.map(id => Agents.byId(id).name), 'and'))}</div><span class="d">${esc(T('ag.othersNote'))}</span></div>
          ${othersOpen || !here.length ? away.map(id => `<div class="r"><div class="grow"><div>${esc(Agents.byId(id).name)}${Agents.byId(id).aka ? `<span class="ver">${esc(Agents.byId(id).aka)}</span>` : ''}</div><div class="d">${esc(T(`ag.how.${id}`))}</div></div><span class="link" data-site="${Agents.byId(id).site}">${esc(T('ag.get'))} ›</span></div>`).join('') : ''}
        </div>`
      : ''
    return `<div class="sec row-sec"><span class="ellip">${esc(T('ag.found', { n: here.length }))}</span><span class="grow"></span>${all}</div>
      <div class="grp ag-list">${rows}</div>
      ${here.length ? `<div class="ag-legend">${legend}</div><div class="note ag-legend-note">${esc(T('ag.legend'))}</div>` : ''}
      ${others}`
  }

  // About: this version, the project's page, the credits (General's foot).
  const aboutHTML = () => `${sec(T('settings.about'))}<div class="grp">
        <div class="r"><div class="grow">${esc(T('s.version', { version: snap.version }))}</div><span class="link" data-site="repo">${esc(T('home.repo'))} ›</span></div>
        <div class="note">${esc(T('home.credits'))}</div>
      </div>`

  // Her updates (updates.rs): this version and the latest, its notes and
  // Update, or a look now; how an update goes (downloaded, checked, waiting
  // for a prompt's answer, restarting) or went wrong; looked for by itself
  // or not; downloaded from GitHub or Gitee first. A build run from the
  // source tree is never looked for.
  function pageUpdate() {
    const u = snap.update || {}
    const version = T('s.updateVersion', { version: u.current || snap.version })
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
    const buttons = newer
      ? `<span class="btns"><button class="pbtn sm" data-update="page">${esc(T('s.updateNotes'))}</button><button class="pbtn sm al" data-update="update" ${busy || !latest.canInstall ? 'disabled' : ''}>${esc(T('s.updateInstall'))}</button></span>`
      : `<button class="pbtn sm" data-update="check" ${busy ? 'disabled' : ''}>${esc(T('s.updateCheck'))}</button>`
    const title = newer ? `${version}${T('s.updateLatestIs', { version: latest.version })}` : version
    const when = u.checkedAt
      ? T('s.updateCheckedAt', { time: new Date(u.checkedAt).toLocaleString(lang === 'zh' ? 'zh-CN' : 'en-US', { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' }) })
      : ''
    return `${sec(T('s.update'))}<div class="grp">
        ${row(esc(title), esc(state), buttons)}
        ${row(esc(T('s.updateAuto')), esc([T('s.updateAutoNote'), when].filter(Boolean).join(' · ')), sw('autoUpdateCheck', snap.settings.autoUpdateCheck !== false))}
        ${row(esc(T('s.updateFrom')), esc(T('s.updateFromNote')), seg('updateFrom', [['auto', T('s.updateFromAuto')], ['gitee', 'Gitee'], ['github', 'GitHub']], snap.settings.updateFrom || 'auto'))}
      </div>`
  }

  // Her crash reports (crash.rs): asked about once (yes from now on, or no),
  // the report shown as it would be sent; sent by themselves or not; those
  // waiting, sending, what went wrong, the last one's issue. Nothing while
  // there is nowhere to send to (a build from the source tree, no relay yet).
  function pageCrash() {
    const c = snap.crash || {}
    if (!c.can) return ''
    const when = ms => new Date(ms).toLocaleString(lang === 'zh' ? 'zh-CN' : 'en-US', { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' })
    const ask =
      c.mode === 'ask' && c.newest
        ? `<div class="grp">
            <div class="note">${esc(T('s.crashAsk', { when: when(c.newest.at) }))}</div>
            ${crashShown ? `<pre class="crash-text mono">${esc(c.newest.text)}</pre>` : ''}
            <div class="r"><button class="pbtn sm" data-crash-show>${esc(T(crashShown ? 's.crashHide' : 's.crashShow'))}</button><div class="grow"></div><span class="btns"><button class="pbtn sm" data-crash="no">${esc(T('s.crashNo'))}</button><button class="pbtn sm al" data-crash="yes">${esc(T('s.crashYes'))}</button></span></div>
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

  // --- The welcome: the agents connected, at the top of General -----------------------
  // While nothing is connected (and until one sends its first event, before
  // she is set up, onboarded): a card with a button for each one on this
  // computer, as equals (whichever one uses, or all), in a row: its name,
  // and under it how it stands; under them what each still needs, or why it
  // could not be written; what the buttons write; Claude Code's plugin the
  // other way. The same model as the Agent page (agents.js: what is here,
  // how each stands, the same buttons). Once one is connected, General has
  // a line on them instead, which opens the Agent page (agentsLine). (The
  // appearance session's spec, 2026-10-13.)
  // "Not now" said (connectHintOff: the welcome's 以后再说, the island's
  // 不用提醒) and nothing connected: the welcome folded to one row at the
  // page's top; opened again by its 去连接, or by asking for connecting.
  let welcomeOpen = false
  const anyIn = () => Agents.TABLE.some(a => Agents.isIn(snap, a.id))
  const welcomeFolded = () => snap.settings.connectHintOff === true && !welcomeOpen && !anyIn()
  const showWelcome = () => !anyIn() || !snap.settings.onboarded
  // The card itself, not General's line: shown, and none connected for real yet.
  const cardShows = () => showWelcome() && !toolsHere().some(tool => toolState(tool) === 'seen')

  // The ones on this computer, in the table's order (agents.js here). None
  // found: the first four (it cannot be told).
  function toolsHere() {
    const here = Agents.here(snap)
    return here.length ? here : ['claude', 'codex', 'agy', 'opencode']
  }

  // Their names as one says them: 「A、B 或 C」, "A, B and C".
  const toolNames = (tools, kind = 'or') => nameList(lang, tools.map(tool => Agents.byId(tool).name), kind)

  // How one stands on the card: busy, seen (a session's event came, or the
  // plugin in), set (written, no session yet: Codex's to be trusted first;
  // ours there to repair counts as written), failed, none.
  function toolState(tool) {
    const state = Agents.stateOf(snap, tool, known())
    if (state === 'on') return 'seen'
    if (state === 'repair' && Agents.isIn(snap, tool)) return Agents.read(snap, tool).seen ? 'seen' : 'set'
    if (state === 'step') return 'set'
    return ['busy', 'set', 'failed'].includes(state) ? state : 'none'
  }

  // Windows' accent for the accent-filled buttons: its light shade on the
  // dark settings, its dark one on the light (the taskbar's look, else
  // Windows' defaults).
  function accentButton() {
    const pair = window.Island?.accent?.() || {}
    return winMode() === 'light' ? `--acc:${pair.dark || '#0067c0'};--acc-ink:#fff` : `--acc:${pair.light || '#4cc2ff'};--acc-ink:#000`
  }

  // One's button: its name, and under it how it stands; set up and
  // connected in green, a step left (Codex) in amber, and those do nothing.
  function toolButton(tool) {
    const state = toolState(tool)
    const name = `<span class="wt-name">${esc(Agents.byId(tool).name)}</span>`
    const line = text => `<span class="wt-state">${text}</span>`
    if (state === 'busy') return `<button class="wc-tool" disabled>${name}${line(`<span class="spin"></span>${esc(T('wc.busy'))}`)}</button>`
    if (state === 'seen') return `<button class="wc-tool done" disabled>${name}${line(esc(T('wc.toolOn')))}</button>`
    if (state === 'set' && tool === 'codex') return `<button class="wc-tool warn" disabled>${name}${line(esc(T('wc.toolStep')))}</button>`
    if (state === 'set') return `<button class="wc-tool done" disabled>${name}${line(esc(T('wc.toolSet')))}</button>`
    return `<button class="wc-tool" data-agent="${tool}:install">${name}${line(esc(T(state === 'failed' ? 'wc.toolRetry' : 'wc.toolGo')))}</button>`
  }

  // Why one could not be written (the card's and the Agent page's): which
  // file, the whole path, the reason, what to do.
  function failedHTML(tool) {
    const { error, kind, folder } = failed[tool]
    const name = Agents.byId(tool).name
    // Her folder's path has a space, which agy can't run: not a file not
    // written, but where she is (the appearance session's words).
    if (kind === 'path') return `<div class="wc-err"><div><b>${esc(T('wc.errConnect', { name }))}</b></div><div>${esc(T('wc.way.path', { folder: folder || '' }))}</div></div>`
    const way = `${T(`wc.way.${kind}`)}${T(tool === 'claude' ? 'wc.wayPlugin' : 'wc.wayEnd')}`
    const file = Agents.read(snap, tool).shown
    return `<div class="wc-err"><div><b>${esc(T('wc.errLead', { name, file: file.split(/[\\/]/).pop() }))}</b></div>
        <div class="mono">${esc(file)}</div><div>${esc(error)}</div><div>${esc(way)}</div></div>`
  }

  // Under the buttons: what each still needs (where it was written, and
  // undone), or why it could not be written.
  function toolNote(tool) {
    const state = toolState(tool)
    const mono = text => `<span class="mono">${esc(text)}</span>`
    const { shown: file, backup } = Agents.read(snap, tool)
    const undo = `<span class="link" data-agent="${tool}:remove">${esc(T('wc.undo'))}</span>`
    if (['claude', 'agy', 'zcode', 'dsh'].includes(tool) && state === 'set') {
      const where = backup ? T('wc.written', { path: mono(file), backup: mono(`${file.split(/[\\/]/).pop()}.wakuwaku.bak`) }) : T('wc.created', { path: mono(file) })
      return `<div class="wc-tnote"><div>${T(`wc.${tool}Left`)}</div><div class="wc-where">${where}${undo}</div></div>`
    }
    if (tool === 'codex' && state === 'set') return `<div class="wc-tnote warn">${T('wc.codexLeft')}</div>`
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
    const pet = (snap.pets || []).find(p => p.id === snap.settings.pet)
    const face = pet ? `<span class="wc-face">${thumb(40, pet.url, pet.version)}</span>` : ''
    // A nudge to approve only where one of them can ask (agents.js approve).
    const sub = `${T('wc.sub1')}${tools.some(tool => Agents.byId(tool).caps.approve[0] !== 'no') ? T('wc.subAsk') : ''}${T('wc.sub2')}${many ? T('wc.subMany') : ''}`
    // OpenCode alone: a file of ours, nothing backed up, and she is not woken
    // by it; omp alone: a file of its own is written, nothing to back up.
    const safeKeys = many ? ['wc.safeMany', 'wc.safe2', 'wc.safe3'] : tools[0] === 'opencode' ? ['wc.safe.opencode', 'wc.safeOc2', 'wc.safeOc3'] : tools[0] === 'omp' ? ['wc.safe.omp', 'wc.safe3'] : [`wc.safe.${tools[0]}`, 'wc.safe2', 'wc.safe3']
    const safe = safeKeys.map(k => `<div><span class="fi">\uE73E</span><span>${T(k)}</span></div>`).join('')
    // Claude Code's plugin, while it is here and not written.
    const commands = snap.pluginCommands || []
    const plugBody = pluginOpen
      ? `<div class="wc-plugbody"><div class="d">${esc(T('wc.plugHow'))}</div>${commands.map(c => `<div class="wc-cmd mono">${esc(c)}</div>`).join('')}
          <div class="wc-copy"><button class="pbtn" data-copy="${esc(commands.join('\n'))}" data-label="${esc(T('wc.copyBoth'))}">${esc(T('wc.copyBoth'))}</button><span class="d">${esc(T('wc.plugWake'))}</span></div></div>`
      : ''
    const plugin =
      tools.includes('claude') && ['none', 'failed', 'busy'].includes(toolState('claude'))
        ? `<div class="wc-div"></div>
        <button class="wc-plug${pluginOpen ? ' open' : ''}" data-plugin-toggle><span class="fi chev">\uE76C</span><span>${esc(T('wc.plug'))}</span><span class="d">${esc(T('wc.plugNote'))}</span></button>${plugBody}`
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
    const s = Agents.summary(snap, known())
    const n = s.connected.length
    const what = [n ? toolNames(s.connected, 'and') : '', s.wants.length ? T('ag.lineFix', { n: s.wants.length }) : '', s.none.length ? T('ag.lineMore', { n: s.none.length }) : '']
    const mood = s.wants.length ? 'waiting' : s.on.length ? 'done' : 'working'
    const title = n ? T(n === 1 ? 'ag.line1' : 'ag.line', { n }) : T('ag.lineRepair')
    return `<div class="grp"><div class="r go" data-tab="agents"><span class="dot" style="background:${COLOR[mood]}"></span><div class="grow"><div>${esc(title)}</div><div class="d ellip">${esc(what.filter(Boolean).join(' · '))}</div></div><span class="link">${esc(T('wc.manage'))}</span></div></div>`
  }

  // Her taskbar on the main display alone (no taskbar on the others), or
  // on all of them (bars.rs); with one display, shown greyed (the appearance
  // session's spec, as Windows' own "show my taskbar on all displays").
  function displaysRow() {
    const value = snap.settings.taskbarDisplays === 'all' ? 'all' : 'main'
    const one = (snap.displayCount || 1) < 2
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
  function pageGeneral() {
    return `${cardShows() ? part('welcome', welcomeBlock()) : agentsLine()}${pageNow()}
      ${part('alerts', pageAlerts())}
      ${sec(T('s.startLang'))}<div class="grp">
        ${row(esc(T('settings.startAtLogin')), esc(T('home.pluginStart')), sw('login', snap.loginAtStart))}
        ${row(esc(T('home.language')), '', seg('lang', [['auto', T('settings.languageAuto')], ['zh', '中文'], ['en', 'EN']], snap.settings.lang))}
      </div>
      ${part('update', pageUpdate())}
      ${part('crash', pageCrash())}
      ${aboutHTML()}`
  }

  const PAGES = { now: pageGeneral, agents: pageAgents, look: pageLook, widgets: pageWidgets, mail: pageMail }

  // --- Drawing --------------------------------------------------------------------------------

  function drawHead() {
    const now = snap.now
    const pet = snap.pets.find(p => p.id === snap.settings.pet)
    const title = now.mood === 'idle' ? T('s.headIdle', { name: pet ? pet.name : 'Wakuwaku' }) : T(HEAD_KEY[now.mood])
    const asks = (snap.asks || []).length
    const sub = [Status.nameOf(now, snap.settings.details !== false), asks ? T('s.asksWaiting', { n: asks }) : ''].filter(Boolean).join(' · ')
    // By ✕, in Windows' own glyphs: locked, a click outside (or on the head)
    // closes them no more; on top, over every window as well (locked too:
    // on top and gone at the next click outside is no use). island.rs
    // keep_on_top.
    const locked = !!snap.settings.settingsPin
    const onTop = locked && !!snap.settings.settingsTop
    const lock = `<button class="x g${locked ? ' on' : ''}" data-lock title="${esc(T(locked ? 's.locked' : 's.lock'))}">${locked ? '&#xE72E;' : '&#xE785;'}</button>`
    const top = `<button class="x g${onTop ? ' on' : ''}" data-top title="${esc(T(onTop ? 's.onTop' : 's.top'))}">${onTop ? '&#xE840;' : '&#xE718;'}</button>`
    setHTML(
      head,
      `<div class="grow ellip"><span class="t">${esc(title)}</span>${sub ? ` <span class="s">· ${esc(sub)}</span>` : ''}</div>${clockTag(now)}${lock}${top}<button class="x" data-close title="${esc(T('s.close'))}">✕</button>`,
    )
  }

  function drawTabs() {
    const i = TABS.indexOf(tab)
    tabs.querySelector('.ind').style.transform = `translateX(${i * 100}%)`
    for (const b of tabs.querySelectorAll('button')) {
      b.classList.toggle('on', b.dataset.tab === tab)
      b.querySelector('span').textContent = T(TAB_KEY[b.dataset.tab])
    }
    tabs.querySelector('[data-tab="now"] .n').hidden = !(snap.asks || []).length
    // An agent that wants the person (amber): the same dot on the Agent page's.
    tabs.querySelector('[data-tab="agents"] .n').hidden = !Agents.summary(snap, known()).wants.length
  }

  // The page, kept as it is when nothing changed; redrawn, the box being
  // typed in (the download box, a plugin's setting, the mail proxy's
  // address) gets the focus and the caret back.
  function drawBody(reset) {
    const box = body.contains(document.activeElement) && document.activeElement.matches('input, textarea') ? document.activeElement : null
    const which =
      box &&
      (box.id ? `#${box.id}` : box.dataset.field ? `[data-field="${box.dataset.field}"]` : box.dataset.mon ? `[data-mon="${box.dataset.mon}"]` : box.dataset.hold ? '[data-hold]' : box.dataset.mf ? `[data-mf="${box.dataset.mf}"]` : box.dataset.w ? `[data-w="${box.dataset.w}"]` : box.dataset.proxyAt !== undefined ? '[data-proxy-at]' : null)
    const caret = which ? box.selectionStart : null
    const changed = setHTML(body, PAGES[tab]())
    if (reset) {
      body.scrollTop = 0
      // Two panes: what is on the right from its top; the list stays where it was.
      const right = body.querySelector('.pane-r')
      if (right) right.scrollTop = 0
    }
    if (!changed) return
    // Left as it is while it has the focus: set again, a word half-made in
    // an input method was lost (results landing while one typed).
    const again = document.getElementById('s-ref')
    if (again && again.value !== typedRef) again.value = typedRef
    const back = which && body.querySelector(which)
    if (back && back !== document.activeElement) {
      back.focus()
      back.setSelectionRange(caret, caret)
    }
    startThumbs()
  }

  function draw(reset = false) {
    if (!isOpen || !snap) return
    lang = snap.lang || lang
    // The Mail page in two panes is wider.
    const wide = twoPanes()
    layer.classList.toggle('wide', wide)
    layer.style.width = wide ? `${Math.min(WIDE_W, screen.availWidth - 40)}px` : ''
    drawHead()
    drawTabs()
    drawBody(reset)
    // Locked (the button by ✕), a click outside closes them no more.
    foot.querySelector('.foot-text').textContent = T(snap.settings.settingsPin ? 's.footLocked' : 's.foot')
    foot.querySelector('.ver').textContent = `v${snap.version}`
    tick()
    // The same height on every page, however much it holds: the head and
    // the tabs stay where they are from one page to the next (on the
    // taskbar it grows up from the strip, and a shorter page would take
    // them down from under the pointer). Never taller than the screen, nor
    // than the home keeps room for (the taskbar's): a page scrolls inside.
    const rest = layer.offsetHeight - body.offsetHeight
    const most = window.Island?.settingsMax?.() ?? screen.availHeight - 56
    body.style.height = `${Math.max(160, Math.min(460, most - rest))}px`
    window.Island?.changed()
  }

  // After the content changed height: measure once it is laid out.
  const relayout = () => requestAnimationFrame(() => window.Island?.changed())

  // --- Open and close ----------------------------------------------------------------------------

  // A part of a page asked for by its old page's name, to scroll to once drawn.
  let partToShow = ''

  function showPart() {
    let name = partToShow
    partToShow = ''
    if (!name) return
    const go = () => {
      const at = body.querySelector(`[data-part="${CSS.escape(name)}"]`)
      if (at) body.scrollTop += at.getBoundingClientRect().top - body.getBoundingClientRect().top
      return body.scrollTop
    }
    // There again as the page is drawn anew in a moment (the default pet put
    // in place, a snapshot coming), unless scrolled since: it was scrolled
    // once, before what came after moved it away (the test session, 2026-10-12).
    let set = go()
    for (const ms of [250, 800, 1600]) {
      setTimeout(() => {
        if (Math.abs(body.scrollTop - set) <= 1) set = go()
      }, ms)
    }
  }

  function open(next) {
    // Asked for connecting: the welcome open, folded or not.
    if (next === 'connect') welcomeOpen = true
    if (PART_OF[next]) {
      tab = PART_OF[next]
      partToShow = next
    } else if (String(next).startsWith('widgets:')) {
      // A widget's row on the plugins page (a press on it in the open island).
      tab = 'widgets'
      partToShow = next
    } else if (TABS.includes(next)) {
      tab = next
    }
    const wasOpen = isOpen
    isOpen = true
    if (!wasOpen) openedAt = Date.now()
    if (!wasOpen) {
      window.pet.keyboard(true)
      clearInterval(clockTimer)
      clockTimer = setInterval(tick, 1000)
    } else if (!document.hasFocus()) {
      // Locked and gone under another window: asked for again, to the front.
      window.pet.keyboard(true)
    }
    window.pet.settings.get().then(got => {
      snap = got
      // Connecting while the welcome card shows (nothing connected yet):
      // the card, at General's top; else the Agent page.
      if (partToShow === 'connect' && cardShows()) {
        tab = 'now'
        partToShow = 'welcome'
      }
      draw(true)
      showPart()
      if (tab === 'look') loadGallery(false)
      if (tab === 'mail' && letterToOpen) {
        const { account, uid, folder } = letterToOpen
        letterToOpen = null
        openLetter(account, uid, folder || 'inbox')
      }
      layer.focus({ preventScroll: true })
    })
    window.Island?.changed()
  }

  function close(why = 'asked') {
    if (!isOpen) return
    console.log('settings close:', why)
    isOpen = false
    // The welcome opened again only for this time: "not now" said since
    // (the island's 不用提醒), the next open shows it folded (the test session).
    welcomeOpen = false
    // The Agent page opens again with its rows closed.
    agentOpen = ''
    othersOpen = false
    clearInterval(clockTimer)
    for (const timer of thumbTimers) clearTimeout(timer)
    thumbTimers = []
    // Seconds being typed are kept as the box is left (holdTyped); Custom is
    // on next time only for a custom number kept.
    if (layer.contains(document.activeElement)) document.activeElement.blur()
    holdCustom = false
    window.pet.settingsClosed()
    window.Island?.changed()
  }

  function setTab(next) {
    if (next === tab || !TABS.includes(next)) return
    tab = next
    draw(true)
    if (tab === 'look') loadGallery(false)
    relayout()
  }

  // A change: shown at once, then whatever main makes of it.
  function patch(change) {
    Object.assign(snap.settings, change)
    draw()
    window.pet.settings.set(change).then(got => {
      snap = got
      draw()
    })
  }

  async function loadGallery(more) {
    if (more ? gallery.loading : gallery.page > 0) return
    const asked = ++gallery.asked
    gallery.loading = true
    gallery.error = ''
    draw()
    const got = await window.pet.settings.gallery(more ? gallery.page + 1 : 1, gallery.sort, gallery.query)
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
  function search(text) {
    clearTimeout(searchTimer)
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
    clearTimeout(searchTimer)
    const text = typedRef.trim()
    const button = layer.querySelector('[data-fetch]')
    if (button && !fetching) button.textContent = T(isLink(text) ? 'settings.fetch' : 's.search')
    if (!isLink(text)) searchTimer = setTimeout(() => search(text), 400)
  }

  async function fetchPet(reference, fromGallery) {
    if (fetching || gallery.getting) return
    if (fromGallery) gallery.getting = reference
    else fetching = reference
    fetchNote = { text: '', isError: false }
    draw()
    const got = await window.pet.settings.fetch(reference)
    fetching = null
    gallery.getting = ''
    if (got.snapshot) snap = got.snapshot
    if (got.ok) {
      fetchNote = { text: [T('settings.fetched', { name: got.pet.name, author: got.pet.author }), got.warning].filter(Boolean).join(' '), isError: false }
      if (!fromGallery) typedRef = ''
    } else {
      fetchNote = { text: got.error, isError: true }
    }
    draw()
    relayout()
  }

  function copy(text, button) {
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

  layer.addEventListener('click', e => {
    const at = selector => e.target.closest(selector)
    // A click anywhere but on it closes the letter menu.
    if (!menuEl.hidden && !at('.mmenu')) closeMenu()
    if (at('[data-lock]')) return patch(snap.settings.settingsPin ? { settingsPin: false, settingsTop: false } : { settingsPin: true })
    if (at('[data-top]')) return patch(snap.settings.settingsPin && snap.settings.settingsTop ? { settingsTop: false } : { settingsPin: true, settingsTop: true })
    if (at('[data-close]')) {
      // Locked: ✕ closes them (and Esc), a press on the head no more.
      if (snap.settings.settingsPin && !at('.x[data-close]')) return
      return close('click')
    }
    const tabButton = at('[data-tab]')
    if (tabButton) return setTab(tabButton.dataset.tab)
    // A session: to its window (the settings close as it comes to the front).
    const session = at('[data-jump]')
    if (session) {
      return window.pet.jump(session.dataset.jump).then(went => {
        if (!went) session.querySelector('.d').textContent = T('jump.notFound')
      })
    }
    const s = snap?.settings
    if (!s) return

    // Mail: an account set up, changed, kept, removed; its server looked
    // up, or typed in.
    if (handleMail(at)) return
    const toggle = at('[data-sw]')
    if (toggle && !toggle.disabled) {
      const key = toggle.dataset.sw
      if (key === 'login') return window.pet.settings.login(!snap.loginAtStart).then(got => ((snap = got), draw()))
      // Crash reports: sent by themselves, or never (not asked again).
      if (key === 'crashReports') return patch({ crashReports: snap.crash?.mode === 'auto' ? 'off' : 'auto' })
      if (key === 'paperShuffle') return paperDo('shuffle', !snap.winPaper?.shuffle)
      if (key === 'ilookBare') return ilookSave(ilookFor(), { ...ilookOf(ilookFor()), bare: !ilookOf(ilookFor()).bare })
      if (key.startsWith('notify.')) return patch({ notify: { ...s.notify, [key.slice(7)]: !s.notify?.[key.slice(7)] } })
      // A widget on or off.
      if (key.startsWith('w:')) {
        const id = key.slice(2)
        const off = (s.widgetsOff || []).filter(o => o !== id)
        return patch({ widgetsOff: off.length === (s.widgetsOff || []).length ? [...off, id] : off })
      }
      if (key === 'widgetNudge') return patch({ widgetNudge: s.widgetNudge === false })
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
        const view = source === 'lhm' ? snap.lhm : snap.hw
        const picked = Array.isArray(s.monitor?.[name]) ? s.monitor[name] : (view?.sensors || []).filter(x => x.on && !x.toggle).map(x => x.id)
        return patch({ monitor: { ...(s.monitor || {}), [name]: picked.includes(id) ? picked.filter(x => x !== id) : [...picked, id].slice(-32) } })
      }
      // A plugin she runs on or off; on and missing a setting, its settings open.
      if (key.startsWith('pl:')) {
        const p = (snap.plugins || []).find(x => x.id === key.slice(3))
        if (!p) return
        if (!p.on && p.params.some(f => f.required && !String(f.value || '').trim())) openPlugin = p.id
        return setPlugin(p.id, { on: !p.on })
      }
      // A plugin's yes-or-no setting.
      if (key.startsWith('pf:')) {
        const [id, name] = key.slice(3).split('.')
        const p = (snap.plugins || []).find(x => x.id === id)
        return p && setPlugin(id, { [name]: !(p.params.find(f => f.name === name) || {}).value })
      }
      return patch({ [key]: !s[key] })
    }
    // A hardware's sensors folded out, or in.
    const hardware = at('[data-lhw]')
    if (hardware) {
      const id = hardware.dataset.lhw
      if (!openHardware.delete(id)) openHardware.add(id)
      return draw()
    }
    // LibreHardwareMonitor's password forgotten.
    if (at('[data-lhm-forget]')) return patch({ monitor: { ...(s.monitor || {}), lhmPass: '' } })
    // The monitor a second faster or slower.
    const pace = at('[data-every]')
    if (pace && !pace.disabled) {
      const every = Math.min(10, Math.max(1, (s.monitor?.every || 2) + Number(pace.dataset.every)))
      return patch({ monitor: { ...(s.monitor || {}), every } })
    }
    // A plugin's settings folded out, or in.
    const fold = at('[data-pexp]')
    if (fold) {
      openPlugin = openPlugin === fold.dataset.pexp ? '' : fold.dataset.pexp
      draw()
      return relayout()
    }
    // A row a step up: the order as shown, with it and the one before swapped.
    const up = at('[data-wup]')
    if (up) {
      const ids = widgetRows().map(r => r.key)
      const i = ids.indexOf(up.dataset.wup)
      if (i > 0) [ids[i - 1], ids[i]] = [ids[i], ids[i - 1]]
      return patch({ widgetOrder: ids })
    }
    const option = at('[data-seg] > span')
    if (option) {
      if (option.classList.contains('off')) return
      const key = option.parentElement.dataset.seg
      const raw = option.dataset.value
      // A plugin's choice (pc:<plugin>.<param>), kept: its script started
      // again with it (scripts.rs sync).
      if (key.startsWith('pc:')) {
        const [id, name] = key.slice(3).split('.')
        return setPlugin(id, { [name]: raw })
      }
      // A widget (w:) or a plugin she runs (p:) put on the taskbar or the
      // desktop, or back in the capsule (island.js placeWidget keeps the layouts).
      if (key.startsWith('where:')) {
        const [kind, ...rest] = key.slice(6).split(':')
        const item = kind === 'p' ? { plugin: rest.join(':') } : { id: rest.join(':') }
        window.Island?.placeWidget?.(item, raw)
        return setTimeout(draw, 50)
      }
      if (key === 'taskbarLayout') {
        // Its parts as the preset has them; the widgets put there kept.
        const layout = window.Island?.tbWithPreset?.(raw)
        return layout && patch({ taskbarLayout: layout })
      }
      if (key === 'ilookMode') {
        ilookMode = raw === winMode() ? '' : raw
        return draw()
      }
      if (key === 'ilookEdge') return ilookSave(ilookFor(), { ...ilookOf(ilookFor()), edge: raw })
      // How long endings stay: Custom shows the box for its seconds, the
      // caret in it (kept once typed: holdTyped); any other choice at once.
      if (key === 'hold') {
        holdCustom = raw === 'custom'
        if (holdCustom) {
          draw()
          const box = layer.querySelector('[data-hold]')
          box?.focus()
          box?.select()
          return
        }
      }
      if (key === 'paperPosition') return raw !== snap.winPaper?.position && paperDo('position', raw)
      if (key === 'paperEvery') return Number(raw) !== snap.winPaper?.every && paperDo('every', raw)
      if (key === 'winMode') {
        if ((raw === 'light') === !!snap.winLook?.light) return
        return window.pet.settings.winLook({ light: raw === 'light' }).then(got => ((snap = got), draw()))
      }
      if (key === 'gallery') {
        if (gallery.sort === raw) return
        gallery.sort = raw
        gallery.page = 0
        gallery.items = []
        return loadGallery(false)
      }
      const value = ['lang', 'monitorSource', 'islandWidth', 'taskbarMaterial', 'taskbarClearWhen', 'taskbarButtons', 'taskbarAlign', 'taskbarEdge', 'taskbarDisplays', 'mailAgent', 'mailOffline', 'updateFrom', 'endCue'].includes(key) ? raw : raw === 'seen' ? 'seen' : Number(raw)
      return patch({ [key]: value })
    }
    const tile = at('[data-tile]')
    if (tile) {
      const key = tile.dataset.tile
      if (key === 'show') {
        if (!snap.hidden) return patch({ out: s.out !== true })
        return window.pet.settings.showPet(true).then(got => ((snap = got), draw()))
      }
      if (key === 'home') return patch({ display: HOMES[(HOMES.indexOf(s.display) + 1) % HOMES.length] })
      return patch({ [key]: !s[key] })
    }
    const mode = at('[data-mode]')
    if (mode) return mode.dataset.mode !== s.display && patch({ display: mode.dataset.mode })
    // The taskbar's layout dragged into shape on the strip: the settings make way.
    if (at('[data-tb-edit]')) {
      close('edit')
      return window.Island?.editTaskbar?.()
    }
    // Her island's look: folded out or in, a preset, a ground, back to Windows' grey.
    if (at('[data-ilook-fold]')) {
      ilookOpen = !ilookOpen
      draw()
      return relayout()
    }
    const ilookPick = at('[data-ilook-preset]')
    if (ilookPick) {
      const mode = ilookFor()
      return ilookSave(mode, { ...ilookPresets(mode).find(([id]) => id === ilookPick.dataset.ilookPreset)[1] })
    }
    const ilookBg = at('[data-ilook-bg]')
    if (ilookBg) return ilookSave(ilookFor(), { ...ilookOf(ilookFor()), bg: ilookBg.dataset.ilookBg })
    if (at('[data-ilook-reset]')) return ilookSave(ilookFor(), ilookDefault(ilookFor()))
    // Windows' colour dialog takes the focus: not a click outside.
    if (e.target.matches('input[type="color"]')) picking = true
    // LibreHardwareMonitor: downloaded, started, its task, its folder (lhm_setup.rs).
    const lhmButton = at('[data-lhm]')
    if (lhmButton && !lhmButton.disabled) {
      if (['get', 'start', 'startup', 'noStartup', 'remove'].includes(lhmButton.dataset.lhm)) lhmAsking = Date.now()
      return window.pet.settings.lhm(lhmButton.dataset.lhm).then(got => ((snap = got), draw()))
    }
    // Her updates: look now, the release's notes, Update.
    const updateButton = at('[data-update]')
    if (updateButton && !updateButton.disabled) return window.pet.settings.update(updateButton.dataset.update).then(got => ((snap = got), draw()))
    if (at('[data-crash-show]')) return ((crashShown = !crashShown), draw())
    const crashButton = at('[data-crash]')
    if (crashButton && !crashButton.disabled) return window.pet.settings.crash(crashButton.dataset.crash).then(got => ((snap = got), draw()))
    const paperButton = at('[data-paper]')
    if (paperButton && !paperButton.disabled) return paperDo(paperButton.dataset.paper)
    const accent = at('[data-accent]')
    if (accent && ACCENT_PICKER) return window.pet.settings.winLook({ accent: accent.dataset.accent }).then(got => ((snap = got), draw()))
    const use = at('[data-use]')
    if (use) return patch({ pet: use.dataset.use })
    if (at('[data-fetch]')) {
      const text = typedRef.trim()
      if (!text) return document.getElementById('s-ref')?.focus()
      return isLink(text) ? fetchPet(text, false) : search(text)
    }
    const get = at('[data-get]')
    if (get) return fetchPet(get.dataset.get, true)
    if (at('[data-more]')) return loadGallery(true)
    const copyButton = at('[data-copy]')
    if (copyButton) return copy(copyButton.dataset.copy, copyButton)
    // An agent connected, repaired, removed (the welcome's buttons and the
    // Agent page's: data-agent="<id>:<install | remove>"); before a row's
    // own click, which opens it.
    const agentButton = at('[data-agent]')
    if (agentButton) {
      if (agentButton.disabled) return
      const [id, action] = agentButton.dataset.agent.split(':')
      return agentDo(id, action)
    }
    if (at('[data-ag-all]')) return connectAll()
    // A row opened in place, or closed; one at a time. Its 怎么信任 (Codex's step) opens it.
    const agentRowAt = at('[data-ag-open]')
    if (agentRowAt) {
      const id = agentRowAt.dataset.agOpen
      agentOpen = agentOpen === id && !agentRowAt.hasAttribute('data-ag-keep') ? '' : id
      draw()
      relayout()
      return requestAnimationFrame(() => body.querySelector(`.ag-row[data-ag-open="${id}"] + .ag-det`)?.scrollIntoView({ block: 'nearest' }))
    }
    if (at('[data-ag-others]')) {
      othersOpen = !othersOpen
      draw()
      return relayout()
    }
    // The CPU temperature/power helper's switch (#18 part B): set up, or
    // remove, through an elevated child of hers (one UAC). The state updates
    // as the helper starts answering (the backend pushes while the settings
    // are open).
    const cpuSensors = at('[data-cpu-sensors]')
    if (cpuSensors) {
      return window.pet.settings.cpuSensors(cpuSensors.dataset.cpuSensors).then(got => {
        snap = got.snapshot
        draw()
        relayout()
      })
    }
    // The welcome folded ("not now", the island's hint put away too), or
    // opened again from its row.
    if (at('[data-welcome-later]')) {
      welcomeOpen = false
      patch({ connectHintOff: true })
      return relayout()
    }
    if (at('[data-welcome-open]')) {
      welcomeOpen = true
      draw()
      return relayout()
    }
    // The welcome's plugin, folded out or in.
    if (at('[data-plugin-toggle]')) {
      pluginOpen = !pluginOpen
      draw()
      return relayout()
    }
    const site = at('[data-site]')
    if (site) return window.pet.settings.openSite(site.dataset.site)
  })

  // A mail account's setup: what was clicked, if it was its. True when it was.
  function handleMail(at) {
    // How a letter goes to an agent: a model or effort picked, a picker
    // opened.
    const picked = at('[data-pick]')
    if (picked && menuPick) {
      const pick = menuPick
      closeMenu()
      pick(picked.dataset.pick)
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
    // The Mail page's proxy: its mode, a manual one's kind.
    const way = at('[data-seg^="mailProxy."] > span')
    if (way) {
      setProxy({ [way.parentElement.dataset.seg.slice(10)]: way.dataset.value })
      return true
    }
    // The inbox and a letter: handed to an agent (from a button or the
    // menu), starred, the menu, back, again, more, another inbox or filter,
    // one opened; more asked in a talk.
    const handing = at('[data-hand]')
    if (handing) {
      const [agent, how] = handing.dataset.hand.split(':')
      const ref = handing.dataset.account ? handing.dataset : { account: reading?.account, uid: reading?.uid, folder: reading?.folder }
      if (!handing.disabled) hand(ref.account, Number(ref.uid), agent, how, ref.folder || 'inbox')
      return true
    }
    const starring = at('[data-star]')
    if (starring) {
      closeMenu()
      star(starring.dataset.star, Number(starring.dataset.uid), starring.dataset.folder || 'inbox')
      return true
    }
    if (at('[data-talk-send]')) {
      const box = body.querySelector('[data-talk]')
      if (box && !box.disabled) say(box)
      return true
    }
    // A letter written: begun (new, a reply, passed on, with an agent's
    // words), put aside and taken up again, sent, thrown away; from whom,
    // cc and bcc shown, files added or taken off.
    if (handleWriting(at)) return true
    const more = at('[data-mail-menu]')
    if (more && reading) {
      const r = more.getBoundingClientRect()
      openMenu(reading.account, reading.uid, reading.folder, r.right - 220, r.bottom + 4)
      return true
    }
    const boxPick = at('[data-mail-box]')
    if (boxPick) {
      const all = snap.mail || []
      openChoices(boxPick, [['*', boxName('*')], ...all.map(a => [a.id, boxName(a.id)])], inboxId(), value => {
        inbox.id = value
        loadInbox(false)
      })
      return true
    }
    const filter = at('[data-seg="mail.filter"] > span')
    if (filter) {
      inbox.filter = filter.dataset.value
      loadInbox(false)
      return true
    }
    // Two panes: the accounts and agents on the right again (a letter being
    // written put aside); one pane or two.
    if (at('[data-mail-setup]')) {
      reading = null
      writing = false
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
      reading = null
      draw(true)
      relayout()
      return true
    }
    if (at('[data-mail-refresh]')) {
      // The server asked now (the list follows as the store changes).
      window.pet.mail.kick(inboxId())
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
      const key = twisty.dataset.twisty
      if (!expanded.delete(key)) expanded.add(key)
      redrawMail()
      return true
    }
    const letter = at('.inbox [data-letter]')
    if (letter) {
      // Beside the list, a letter being written is put aside for it.
      if (twoPanes()) writing = false
      // A conversation folds out as its letter opens.
      if (letter.dataset.thread) expanded.add(letter.dataset.thread)
      openLetter(letter.dataset.account, Number(letter.dataset.letter), letter.dataset.folder || 'inbox')
      return true
    }
    const turned = at('[data-sw^="mail:"]')
    if (turned) {
      const a = (snap.mail || []).find(x => x.id === turned.dataset.sw.slice(5))
      if (a) window.pet.mail.switch(a.id, !a.on).then(got => ((snap = got), draw()))
      return true
    }
    if (at('[data-mail-add]')) {
      mailForm = { id: '', address: '', name: '', password: '', host: '', port: 993, security: 'ssl', auth: 'auto', username: '', ...outFields(null), ...proxyFields(null), step: 'start', advanced: false, probed: null, probedOut: null, busy: '', result: null }
      mailDelete = ''
      redrawMail('[data-mf="address"]')
      return true
    }
    const change = at('[data-mail-edit]')
    if (change) {
      const a = (snap.mail || []).find(x => x.id === change.dataset.mailEdit)
      if (a) {
        mailForm = { id: a.id, address: a.address, name: a.name || '', password: '', host: a.host, port: a.port, security: a.security, auth: a.auth || 'auto', username: a.username, ...outFields(a.smtp), ...proxyFields(a.proxy), step: 'found', source: '', advanced: true, probed: null, probedOut: null, busy: '', result: null }
        // One from before the pet sent mail: its outgoing server looked up.
        if (!a.smtp) findOutgoing(mailForm)
      }
      mailDelete = ''
      redrawMail('[data-mf="password"]')
      return true
    }
    if (at('[data-mail-cancel]')) {
      mailForm = null
      redrawMail()
      return true
    }
    // The fold opened or closed; a way or a method picked; Re-test.
    if (at('[data-mail-adv]') && mailForm) {
      mailForm.advanced = !mailForm.advanced
      redrawMail(mailForm.advanced ? '[data-mf="host"]' : null)
      return true
    }
    const mfpick = at('[data-mfpick]')
    if (mfpick && mailForm) {
      const f = mailForm
      const key = mfpick.dataset.mfpick
      // The account's own way to its servers; manual for the first time,
      // the address the Mail page or Windows has.
      if (key === 'proxyMode' || key === 'proxyKind') {
        const mode = key === 'proxyMode'
        openChoices(mfpick, mode ? PROXY_MODES() : PROXY_KINDS, f[key] || (mode ? '' : 'http'), value => {
          f[key] = value
          if (value === 'manual' && !String(f.proxyAddress || '').trim()) {
            const known = proxySetting().address ? proxySetting() : snap.mailSystemProxy
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
        if (isWay && (out ? ['465', '587', '25', ''] : ['993', '143', '']).includes(String(f[portKey] ?? ''))) f[portKey] = usual[value]
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
      const id = remove.dataset.mailRemove
      if (mailDelete !== id) {
        mailDelete = id
        draw()
        return true
      }
      mailDelete = ''
      mailForm = null
      window.pet.mail.remove(id).then(got => ((snap = got), draw(), relayout()))
      return true
    }
    return false
  }

  function handleWriting(at) {
    if (at('[data-w-new]')) {
      write('new')
      return true
    }
    const re = at('[data-w-reply]')
    if (re) {
      closeMenu()
      const ref = re.dataset.account ? re.dataset : { account: reading?.account, uid: reading?.uid, folder: reading?.folder }
      writeAbout(re.dataset.wReply, ref.account, Number(ref.uid), ref.folder || 'inbox')
      return true
    }
    const use = at('[data-w-use]')
    if (use && reading?.letter) {
      const talk = talkOf(reading.letter.key) || reading.talk
      const said = talk?.turns?.[Number(use.dataset.wUse)]?.text
      if (said) write('reply', reading.letter, replyIn(said))
      return true
    }
    if (at('[data-w-open]')) {
      writing = true
      draw(true)
      relayout()
      return true
    }
    if (!draft) return false
    if (at('[data-w-back]')) {
      writing = false
      dropSure = false
      draft.note = null
      draw(true)
      relayout()
      return true
    }
    if (at('[data-w-drop]')) {
      if (!dropSure) {
        dropSure = true
        draw()
        return true
      }
      draft = null
      writing = false
      dropSure = false
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
      const d = draft
      openChoices(from, (snap.mail || []).map(a => [a.id, a.address]), d.account, v => {
        d.account = v
        keepDraft()
        draw()
      }, true)
      return true
    }
    if (at('[data-w-cc]')) {
      draft.showCc = true
      redrawMail('[data-w="cc"]')
      return true
    }
    if (at('[data-w-attach]')) {
      const input = body.querySelector('[data-w-file]')
      if (input) {
        picking = true
        input.click()
      }
      return true
    }
    const unfile = at('[data-w-unfile]')
    if (unfile) {
      const key = unfile.dataset.wUnfile
      const list = key[0] === 'o' ? draft.forward?.files : draft.files
      list?.splice(Number(key.slice(1)), 1)
      draft.touched = true
      keepDraft()
      redrawMail()
      return true
    }
    return false
  }

  function redrawMail(focus) {
    draw()
    relayout()
    if (focus) requestAnimationFrame(() => body.querySelector(focus)?.focus())
  }

  // The server for the address, looked up as Thunderbird does; filled in
  // when found, else the fields to fill.
  async function findServer() {
    const f = mailForm
    if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(f.address.trim())) {
      f.result = { bad: true, text: T('mail.badAddress') }
      return redrawMail('[data-mf="address"]')
    }
    f.busy = 'find'
    f.result = null
    redrawMail()
    const got = await window.pet.mail.discover(f.address.trim(), formProxy(f))
    if (mailForm !== f) return
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
  async function findOutgoing(f) {
    f.findingOut = true
    const got = await window.pet.mail.discover(f.address, formProxy(f))
    f.findingOut = false
    if (mailForm !== f) return
    if (got.ok && got.smtp && !String(f.smtpHost || '').trim()) Object.assign(f, outFields(got.smtp), { smtpUser: f.username || got.smtp.username })
    redrawMail()
  }

  // Re-test: both servers, the outgoing one where there is one.
  async function retest() {
    const f = mailForm
    await probeServer()
    if (mailForm === f && String(f.smtpHost || '').trim()) await probeOut()
  }

  // The outgoing server tried as the incoming one is below.
  async function probeOut() {
    const f = mailForm
    f.busy = 'probe'
    f.probedOut = null
    redrawMail()
    const got = await window.pet.mail.probe(String(f.smtpHost).trim(), String(f.smtpPort ?? '').trim(), f.smtpSecurity || 'auto', 'smtp', formProxy(f))
    if (mailForm !== f) return false
    f.busy = ''
    if (got.ok && got.found) {
      Object.assign(f, { smtpSecurity: got.security, smtpPort: got.port })
      f.probedOut = { security: got.security, port: got.port, auths: got.auths || [] }
    } else {
      f.result = { bad: true, text: T('mail.w.outPrefix') + mailError(got.error, { host: String(f.smtpHost).trim(), port: String(f.smtpPort || '').trim() || '465 / 587' }) }
    }
    redrawMail()
    return !!f.probedOut
  }

  // Re-test: the server tried without signing in; the way and port, when
  // they were to be detected, set to what answered. True when it did.
  async function probeServer() {
    const f = mailForm
    if (!String(f.host || '').trim()) {
      f.result = { bad: true, text: T('mail.err.fields') }
      redrawMail('[data-mf="host"]')
      return false
    }
    f.busy = 'probe'
    f.result = null
    f.probed = null
    redrawMail()
    const got = await window.pet.mail.probe(String(f.host).trim(), String(f.port ?? '').trim(), f.security || 'auto', 'imap', formProxy(f))
    if (mailForm !== f) return false
    f.busy = ''
    if (got.ok && got.found) {
      Object.assign(f, { security: got.security, port: got.port })
      f.probed = { security: got.security, port: got.port, auths: got.auths || [] }
    } else {
      f.result = { bad: true, text: mailError(got.error, { host: String(f.host).trim(), port: String(f.port || '').trim() || '993 / 143' }) }
    }
    redrawMail()
    return !!f.probed
  }

  // Signed in once, then kept; or what went wrong (and, for a Windows
  // domain's server, what the user name may be). A way or port still to be
  // detected is, first.
  async function saveAccount() {
    const f = mailForm
    if (!f.id && !f.password) {
      f.result = { bad: true, text: T('mail.err.password') }
      return redrawMail('[data-mf="password"]')
    }
    if ((f.security === 'auto' || !String(f.port ?? '').trim()) && !(await probeServer())) return
    if (mailForm !== f) return
    const outHost = String(f.smtpHost || '').trim()
    if (outHost && (f.smtpSecurity === 'auto' || !String(f.smtpPort ?? '').trim()) && !(await probeOut())) return
    if (mailForm !== f) return
    f.busy = 'save'
    f.result = null
    redrawMail()
    const username = String(f.username || f.address).trim()
    const smtp = outHost ? { host: outHost, port: Number(f.smtpPort), security: f.smtpSecurity, auth: f.smtpAuth || 'auto', username: String(f.smtpUser || username).trim() } : null
    const account = { id: f.id, address: f.address.trim(), name: String(f.name || '').trim(), host: String(f.host).trim(), port: Number(f.port), security: f.security, auth: f.auth || 'auto', username, smtp, proxy: formProxy(f) }
    // Google's app password as it shows it, in fours: the spaces left out.
    const typed = String(f.password || '')
    const password = providerOf(account.host) === 'google' && /^[a-z]{4}(\s?[a-z]{4}){3}$/i.test(typed.trim()) ? typed.replace(/\s/g, '') : typed
    const got = await window.pet.mail.save(account, password)
    if (mailForm !== f) return
    f.busy = ''
    if (got.ok) {
      mailForm = null
      snap = got.snapshot
    } else {
      const domain = got.error?.kind === 'login' && (got.auths || []).some(a => a === 'ntlm' || a === 'gssapi')
      const where = got.out ? { host: outHost, port: f.smtpPort } : account
      f.result = { bad: true, text: (got.out ? T('mail.w.outPrefix') : '') + mailError(got.error, where) + (domain ? ` ${T('mail.adv.domain')}` : '') }
    }
    redrawMail()
  }

  layer.addEventListener('input', e => {
    if (e.target.dataset.mf && mailForm) mailForm[e.target.dataset.mf] = e.target.value
    if (e.target.dataset.proxyAt !== undefined) proxyTyped = { text: e.target.value, bad: false }
    if (e.target.dataset.w && draft) {
      draft[e.target.dataset.w] = e.target.value
      draft.touched = true
      dropSure = false
      keepDraft()
    }
    if (e.target.dataset.ilook) ilookInput(e.target, false)
    if (e.target.id === 's-ref') {
      typedRef = e.target.value
      // Not mid-word in an input method: once the word is chosen.
      if (!e.isComposing) searchSoon()
    }
    if (e.target.dataset.field) drafts[e.target.dataset.field] = e.target.value
    if (e.target.dataset.mon) drafts[`mon:${e.target.dataset.mon}`] = e.target.value
    if (e.target.dataset.hold) drafts['hold:secs'] = e.target.value
  })

  // The word an input method has chosen: looked for as if typed.
  layer.addEventListener('compositionend', e => {
    if (e.target.id !== 's-ref') return
    typedRef = e.target.value
    searchSoon()
  })

  // A plugin's setting kept once typed (Enter, or leaving the box); a
  // number that is none goes back to what it was.
  layer.addEventListener('change', e => {
    if (e.target.matches('[data-w-file]')) return addFiles(e.target)
    if (e.target.dataset.ilook) return ilookInput(e.target, true)
    if (e.target.dataset.proxyAt !== undefined) return proxyAddress(e.target.value)
    if (e.target.dataset.mon) return monitorField(e.target.dataset.mon)
    if (e.target.dataset.hold) return holdTyped(e.target)
    const key = e.target.dataset.field
    if (!key || !(key in drafts)) return
    const value = drafts[key].trim()
    delete drafts[key]
    const [id, name] = key.split('.')
    const field = ((snap.plugins || []).find(p => p.id === id)?.params || []).find(f => f.name === name)
    if (!field || (field.kind === 'number' && value && !(Number(value) > 0))) return draw()
    setPlugin(id, { [name]: value })
  })

  // How the monitor reaches LibreHardwareMonitor, kept once typed: a port
  // that is none goes back to what it was; the password goes to Windows'
  // Credential Manager (lhm.rs keep_password), an empty one changes nothing.
  function monitorField(name) {
    const key = `mon:${name}`
    if (!(key in drafts)) return
    const typed = drafts[key]
    delete drafts[key]
    const m = snap.settings.monitor || {}
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
  function holdTyped(box) {
    delete drafts['hold:secs']
    const secs = window.Flow.holdSecs(box.value)
    const kept = snap.settings.hold
    box.value = String(secs ?? (Number.isInteger(kept) ? kept : ''))
    if (secs === null || secs === kept) return draw()
    patch({ hold: secs })
  }

  // Custom's box left empty, nothing custom kept: the choice kept shows again.
  layer.addEventListener('focusout', e => {
    if (!e.target.dataset?.hold) return
    delete drafts['hold:secs']
    if (e.target.value.trim() || window.Flow.holdChoice(snap?.settings?.hold) === 'custom') return
    holdCustom = false
    draw()
  })

  // A plugin's settings changed, the others kept.
  function setPlugin(id, change) {
    const all = snap.settings.plugins || {}
    return patch({ plugins: { ...all, [id]: { ...(all[id] || {}), ...change } } })
  }

  document.addEventListener('keydown', e => {
    if (!isOpen) return
    if (e.key === 'Escape') {
      e.preventDefault()
      if (!menuEl.hidden) return closeMenu()
      return close('Esc')
    }
    if (e.target.matches?.('input, textarea')) {
      // Ctrl+Enter sends the letter being written.
      if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && e.target.dataset.w) {
        e.preventDefault()
        return sendDraft()
      }
      if (e.key === 'Enter' && e.target.id === 's-ref' && !e.isComposing) layer.querySelector('[data-fetch]')?.click()
      if (e.key === 'Enter' && (e.target.dataset.field || e.target.dataset.mon || e.target.dataset.hold || e.target.dataset.proxyAt !== undefined)) e.target.blur()
      if (e.key === 'Enter' && e.target.dataset.mf) layer.querySelector('[data-mail-find]:not([disabled]), [data-mail-save]:not([disabled])')?.click()
      if (e.key === 'Enter' && e.target.dataset.talk && !e.isComposing) say(e.target)
      return
    }
    // Two panes: the letter above or below in the list.
    if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && twoPanes() && !(writing && draft) && menuEl.hidden) {
      e.preventDefault()
      return step(e.key === 'ArrowDown' ? 1 : -1)
    }
    const i = TABS.indexOf(tab)
    if (e.key === 'ArrowRight') {
      e.preventDefault()
      setTab(TABS[(i + 1) % TABS.length])
    }
    if (e.key === 'ArrowLeft') {
      e.preventDefault()
      setTab(TABS[(i + TABS.length - 1) % TABS.length])
    }
  })

  window.pet.onSettings(({ tab: next } = {}) => open(next))
  window.pet.onSettingsChanged(got => {
    if (!isOpen) return
    snap = got
    draw()
    // What is kept changed (a sync): the list again, quietly.
    if (tab === 'mail' && inbox.for && (got.mailRev || 0) !== inbox.rev) loadInbox(false, true)
  })
  // The keyboard went elsewhere: a click outside the island; not a file
  // picker (a letter's files, the desktop's picture: its own window takes
  // the focus).
  // Locked (the button by ✕), they stay.
  window.pet.onBlur(() => !snap?.settings?.settingsPin && !picking && !pickingPaper && !lhmAsks() && (!window.pet.mac || Date.now() - openedAt > BLUR_GRACE_MS) && close('blur'))
  layer.addEventListener('cancel', e => e.target.matches?.('[data-w-file]') && (picking = false), true)
  window.addEventListener('focus', () => setTimeout(() => (picking = false), 1000))

  window.Settings = {
    isOpen: () => isOpen,
    layer,
    askSlot,
    // The language may change from the menu while they are open.
    redraw: () => draw(),
    close,
    // The letter to show once the Mail page opens ({ account, uid }).
    openLetter: target => (letterToOpen = target),
    // The plugin whose settings are folded out once the plugins page opens.
    openPlugin: id => (openPlugin = String(id || '')),
  }
})()
