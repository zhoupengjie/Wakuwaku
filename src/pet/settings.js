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
//   widgets   plugins: the market (获取更多 ›: a list, each one's page, installed from there, updates), the built-in ones, those she runs for you (how each is and where it is put, no switch: placed is on; their settings folded under), other people's (the same, and each its own page with what it may do and 卸载), turns, nudges, updates by themselves, installing one from a file, waku, and how to write one
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
  const AddonUI = window.AddonUI

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
  const PART_OF = { pets: 'look', place: 'look', paper: 'look', shortcuts: 'now', alerts: 'now', connect: 'agents', update: 'now', net: 'now', crash: 'now' }
  const HEAD_KEY = { working: 's.headWorking', waiting: 's.headWaiting', done: 's.headDone', review: 's.headReview', error: 's.headError' }
  const SIZES = [['small', 0.4], ['medium', 0.55], ['large', 0.75]]
  const HOMES = ['island', 'taskbar', 'split']
  const HOME_KEY = { island: 's.displayIsland', taskbar: 's.displayTaskbar', split: 's.displaySplit' }
  // Her taskbar the home: along an edge, or split into the top bar and the dock.
  const onTaskbar = s => s.display === 'taskbar' || s.display === 'split'
  // How long what she tells stays (停留多久): the quick choices, until seen,
  // and Custom, whole seconds typed in a box beside them (flow.js holdSecs,
  // zhoupengjie/Wakuwaku#59).
  const HOLDS = [...window.Flow.HOLD_QUICK, 'seen']
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
  // Other people's plugins (addons.rs; addonui.js the logic): a part of the
  // Plugins page opened in its place (sub: { page, id, back }: a plugin's
  // own page, a pack's ({ kind } too), the market, one of the market's),
  // the confirm layer over the panel (installing from a file, by
  // AddonUI.next's steps; taking one away: gone { id, busy }, a pack's with
  // its kind), Windows' dialog open for a .zip (its focus does not close
  // them), and those installed while they are open (their rows edged in
  // the accent until they close; a pack's by AddonUI.packKey).
  let sub = null
  let flow = AddonUI.IDLE
  let gone = null
  // A plugin whose newly-asked permissions are being allowed (finding 1).
  let approveFor = null
  let pickingZip = false
  const freshRows = new Set()
  // The plugin market (market.rs; addonui.js its words and steps), a part
  // of the Plugins page (sub market, an item's page mkItem): its index as
  // last read (items null until one was), when, what went wrong reading it,
  // being read; the category and the words looked for; each download's last
  // word (market:progress) by id; 全部更新 under way. Asked over the panel
  // (mkAsk): installing one that asks for more than the web ({ kind:
  // 'install', id }), or 全部更新 when one asks for more ({ kind: 'updates' }).
  const market = { items: null, at: 0, error: null, loading: false, cat: 'all', query: '', jobs: {}, updating: false }
  let mkAsk = null
  let recordingShortcut = null
  // Recording one: her global shortcuts let go meanwhile, so the keys
  // pressed reach this page (shortcuts.rs pause).
  const setRecording = key => {
    if (key === recordingShortcut) return
    recordingShortcut = key
    window.pet.shortcutsPause?.(key !== null)
  }
  let failedShortcuts = null
  window.pet.onShortcutStatus && window.pet.onShortcutStatus(({ failed } = {}) => {
    failedShortcuts = new Set(failed || [])
    if (snap) snap.shortcutsFailed = failed || []
    draw()
  })
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
  // A proxy address as typed and not yet kept (key: mailProxy, netProxy): being typed,
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
    <div class="s-foot"><span class="foot-text"></span><span class="grow"></span><span class="ver"></span></div>
    <div class="s-sheet" hidden></div>
    <div class="s-toast" role="status" hidden></div>`
  const head = layer.querySelector('.s-head')
  const tabs = layer.querySelector('.s-tabs')
  tabs.style.setProperty('--tabs', TABS.length)
  const askSlot = layer.querySelector('.s-ask')
  const body = layer.querySelector('.s-body')
  const foot = layer.querySelector('.s-foot')
  // A question over the whole panel (installing a plugin, taking one away),
  // in place of a dialog of Windows'; a word for a few seconds at its foot.
  const sheet = layer.querySelector('.s-sheet')
  const toastEl = layer.querySelector('.s-toast')

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

  function sessionDetails(det) {
    const kv = (key, val) => `<div class="kv"><span class="k">${esc(T(key))}</span><span class="v">${val}</span></div>`
    const rows = []
    if (det.files.length) {
      const list = det.files.map(f => `<span class="s-file mono path">${esc(f)}</span>`).join('')
      const more = det.moreFiles > 0 ? `<span class="d s-more">${esc(T('s.filesMore', { n: det.moreFiles, total: det.totalFiles }))}</span>` : ''
      rows.push(kv('s.changedFiles', `<div class="s-files">${list}${more}</div>`))
    }
    if (det.steps.length) {
      const list = det.steps
        .map(st => `<div class="s-step${st.ok ? '' : ' err'}"><span class="fi s-st-ic">${st.ok ? '\uE73E' : '\uE711'}</span><span class="ellip">${esc(st.text)}</span></div>`)
        .join('')
      rows.push(kv('s.recentSteps', `<div class="s-steps">${list}</div>`))
    }
    return `<div class="ag-det s-det">${rows.join('')}</div>`
  }

  function pageNow() {
    const s = snap.settings
    const sessions = snap.sessions || []
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
              Status.status(lang, x, { detailed, withClock: false }),
              detailed && x.name && x.project !== name ? x.project : '',
              detailed && Status.isEnding(x) ? x.reply : '',
            ].filter(Boolean)
            const det = Status.detailsOf(lang, x, { detailed })
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

  // A row to the market on its kind (spec §1: 获取更多宠物…, 获取更多壁纸…),
  // at the end of what is installed: widgets:market:pet or :wallpaper.
  const toMarketRow = (kind, title, note) =>
    `<div class="r jumps mk-more" data-look-market="${kind}" role="button" tabindex="0"><span class="mk-big sm">&#xE719;</span><div class="grow"><div>${esc(T(title))}</div><div class="d">${esc(T(note))}</div></div><span class="chev">&#xE76C;</span></div>`

  // Her pets: each with who made it and where it came from (a pack's:
  // 来自市场, or 未经审核 from a file: AddonUI.petNote), the one in use or
  // 使用; then 获取更多宠物… (the market on its pets); the gallery of
  // codex-pets.net and its download box as ever.
  function pagePets() {
    const current = snap.settings.pet
    // 删除 on those in her own folder (downloaded, or the market's), asked
    // first (petDeleteLayerOf); never Codex's, nor the only one (the user,
    // 2026-11-08: 宠物增加删除按钮).
    const canDelete = p => p.mine && snap.pets.length > 1
    const installed = snap.pets.length
      ? snap.pets
          .map(
            p => `<div class="r petrow">${thumb(52, p.url, p.version)}<div class="grow"><div>${esc(p.name)}</div><div class="d">${esc(AddonUI.petNote(p, T))}</div></div>
            ${p.id === current ? `<span class="d in-use">${esc(T('home.inUse'))}</span>` : `<button class="pbtn" data-use="${esc(p.id)}">${esc(T('home.use'))}</button>`}${canDelete(p) ? `<button class="pbtn no" data-pet-delete="${esc(p.id)}">${esc(T('pets.delete'))}</button>` : ''}</div>`,
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
    return `${sec(T('home.installed'))}<div class="grp">${installed}${toMarketRow('pet', 'packs.morePets', 'packs.morePetsNote')}</div>
      ${sec(T('s.download'))}<div class="grp"><div class="r"><input class="in" id="s-ref" spellcheck="false" placeholder="${esc(T('s.fetchPlaceholder'))}"><button class="pbtn al" data-fetch ${fetching ? 'disabled' : ''}>${esc(fetchLabel)}</button></div>${note}</div>
      <div class="sec row-sec"><span class="ellip">${esc(listTitle)}</span><span class="grow"></span>${seg('gallery', [['popular', T('home.popular')], ['newest', T('home.newest')]], gallery.sort)}</div>
      ${gallery.error ? `<div class="note err">${esc(T('home.galleryError', { message: gallery.error }))}</div>` : ''}${none}
      <div class="cards">${cards}</div>${more}
      <div class="links"><span class="link" data-site="site">${esc(T('settings.browse'))} ›</span></div>`
  }

  // --- Her island's look (island-look.js drawIslandLook) ---------------------------------
  let ilookOpen = false
  // The mode being set: '' follows Windows' own.
  let ilookMode = ''
  // Windows' dialog open for the desktop's picture: its own mark, held until
  // the dialog answers (picking, a letter's files', is let go a second after
  // the island has the focus again, which a press on it mid-dialog gives).
  let pickingPaper = false
  // Windows' question for LibreHardwareMonitor (run as administrator) takes
  // the focus too: from the press, and while its job is at that stage.
  let lhmAsking = 0
  // In settings-look.js:
  const {
    ACCENT_PICKER, ilookDefault, ilookFor, ilookInput, ilookOf, ilookPresets, ilookSave, lhmAsks, pageLook,
    paperDo, winMode,
  } = window.SettingsParts.look({
    AddonUI, COLOR, SIZES, T, draw, esc, onTaskbar, pagePets, part, patch, row, sec, seg, sw, toMarketRow,
    get displaysRow() { return displaysRow },
    get ilookMode() { return ilookMode },
    get ilookOpen() { return ilookOpen },
    get lhmAsking() { return lhmAsking },
    get packTile() { return packTile },
    get screenRow() { return screenRow },
    get pickingPaper() { return pickingPaper }, set pickingPaper(v) { pickingPaper = v },
    get snap() { return snap }, set snap(v) { snap = v },
  })

  function pageAlerts() {
    const s = snap.settings
    // When she has something to tell that needs no answer (a turn's end, a
    // hello, a plugin's nudge): the island open, a ring round it (the flow
    // light, the default), or the dot alone (island-ring.js cue); its three
    // choices under it, too long beside it.
    const cue = window.Flow.endCue(s)
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
    const holdOn = window.Flow.holdChoice(s.hold, holdCustom)
    const range = esc(T('s.holdRange', { min: window.Flow.HOLD_MIN, max: window.Flow.HOLD_MAX }))
    const typed = 'hold:secs' in drafts ? drafts['hold:secs'] : Number.isInteger(s.hold) ? String(s.hold) : ''
    const secs =
      holdOn === 'custom'
        ? `<span class="hold-secs"><input class="in secs" data-hold="secs" value="${esc(typed)}" inputmode="numeric" maxlength="5" placeholder="${window.Flow.HOLD_MIN}–${window.Flow.HOLD_MAX}" title="${range}" aria-label="${range}" spellcheck="false" autocomplete="off"${holdOff ? ' disabled' : ''}><span class="d">${esc(T('s.holdUnit'))}</span></span>`
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
        ${row(esc(T('settings.hideInFullscreen')), '', sw('hideInFullscreen', s.hideInFullscreen, snap.fullscreenAvailable ? '' : 'disabled'))}
      </div>`
  }

  // The plugins page as rows, in the order the person put them in: the
  // built-in ones, the plugins she runs (with what their widgets say),
  // other people's (addons.rs: plugins; the packs are a group of their
  // own, packsGroup), and the widgets of anyone else's scripts.
  function widgetRows() {
    const rank = key => {
      const i = (snap.settings.widgetOrder || []).indexOf(key)
      return i < 0 ? Infinity : i
    }
    const widgets = snap.widgets || []
    const rows = [
      ...widgets.filter(w => w.builtIn).map(w => ({ key: w.id, widget: w })),
      ...(snap.plugins || []).map(p => ({ key: p.id, plugin: p, mine: widgets.filter(w => w.plugin === p.id) })),
      ...addonsNow().map(a => ({ key: a.id, addon: a, mine: widgets.filter(w => w.plugin === a.id) })),
      ...widgets.filter(w => !w.builtIn && !w.plugin).map(w => ({ key: w.id, widget: w })),
    ]
    return rows.map((r, i) => ({ ...r, i })).sort((x, y) => rank(x.key) - rank(y.key) || x.i - y.i)
  }

  // What a plugin she runs says under its name, as it is (placed is on, so
  // put nowhere is off): 没放; running, 运行中 and what it does; else why it
  // shows nothing (a parameter to fill in, 出错 with its last line). Put
  // somewhere and not on yet (picker.js turns it on): starting.
  function pluginNote(p, mine, placed) {
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

  // Where one is put (island-wmenu.js placeNow): its place in a quiet word, or,
  // put nowhere, 添加小工具… (the picker, as her menu opens it, on its card:
  // key, picker.js fromMenu focus).
  const placeOf = (item, on) => window.Island?.placeNow?.(item, on) || { at: on ? 'isle' : null }
  function placeHTML(place, key = '') {
    if (!place.at) return `<button class="pbtn sm" data-padd="${esc(key)}">${esc(T('pl.add'))}</button>`
    const where = esc(T(`pl.at.${place.at}`, { name: place.name || '' }))
    return `<span class="pl-at" title="${where}">${where}</span>`
  }

  // A plugin's settings, folded out under it: what is being typed stays.
  function pluginFields(p) {
    const fields = p.params.map(f => {
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

  // --- Other people's plugins (addons.rs view; addonui.js) ---------------------------------
  // In settings-addons.js:
  const {
    SUBS, addonById, addonField, addonRow, addonsNow, andList, backSub, canDoHTML, crumbHTML, openSub,
    packById, packTile, packsGroup, toLook,
  } = window.SettingsParts.addons({
    AddonUI, COLOR, T, Widgets, drafts, draw, esc, freshRows, loadGallery, nameList, part, placeHTML, placeOf,
    sec, seg, showPart, sw,
    get lang() { return lang },
    get marketItem() { return marketItem },
    get marketItemPage() { return marketItemPage },
    get marketPage() { return marketPage },
    get openPlugin() { return openPlugin },
    get relayout() { return relayout },
    get snap() { return snap },
    get partToShow() { return partToShow }, set partToShow(v) { partToShow = v },
    get sub() { return sub }, set sub(v) { sub = v },
    get tab() { return tab }, set tab(v) { tab = v },
  })

  // --- The market (market.rs; addonui.js) ------------------------------------------------
  // In settings-market.js:
  const {
    approveGo, drawSheet, fillCards, flowStep, installFromFile, installGo, loadMarket, marketInstall,
    marketItem, marketItemPage, marketPage, toast, uninstallGo, updateAll,
  } = window.SettingsParts.market({
    AddonUI, COLOR, T, Widgets, addonById, andList, body, canDoHTML, crumbHTML, draw, esc, freshRows, market,
    packById, sec, seg, setHTML, sheet, showPart, toastEl,
    get isOpen() { return isOpen },
    get lang() { return lang },
    get netNow() { return netNow },
    get relayout() { return relayout },
    get tab() { return tab },
    get approveFor() { return approveFor }, set approveFor(v) { approveFor = v },
    get flow() { return flow }, set flow(v) { flow = v },
    get gone() { return gone }, set gone(v) { gone = v },
    get mkAsk() { return mkAsk }, set mkAsk(v) { mkAsk = v },
    get partToShow() { return partToShow }, set partToShow(v) { partToShow = v },
    get pickingZip() { return pickingZip }, set pickingZip(v) { pickingZip = v },
    get snap() { return snap }, set snap(v) { snap = v },
    get sub() { return sub }, set sub(v) { sub = v },
  })

  // --- The Plugins page, and the monitor's sensors -------------------------------------
  // In settings-plugins.js:
  const { pageWidgets, picksKey } = window.SettingsParts.plugins({
    AddonUI, COLOR, SPINS, SUBS, T, Widgets, addonRow, drafts, esc, market, openHardware, packsGroup,
    placeHTML, placeOf, pluginFields, pluginNote, render, row, sec, seg, sw, widgetRows,
    get lang() { return lang },
    get openPlugin() { return openPlugin },
    get snap() { return snap },
    get sub() { return sub }, set sub(v) { sub = v },
  })

  // --- Mail accounts and the inbox -----------------------------------------------------
  // In settings-mail.js:
  const {
    APP_PASSWORD, AUTHS, PROXY_KINDS, PROXY_MODES, SECURITIES, WIDE_W, accountOf, agentChoices, agentsHTML,
    boxName, choicesOf, confOf, firstAgent, formProxy, handButtons, inboxHTML, inboxId, isLetter, isReading,
    labelOf, listed, loginWhy, mailDate, mailError, mailFix, mailFormHTML, mailList, mailNote, mailSaid,
    mailSize, outFields, people, providerOf, proxyFields, setConf, talkHTML, talkOf, twoPanes, where,
  } = window.SettingsParts.mail({
    AGENTS, AGENT_NAMES, Agents, T, esc, expanded, inbox, patch, row, sec, seg,
    get draft() { return draft },
    get handNote() { return handNote },
    get lang() { return lang },
    get mailDelete() { return mailDelete },
    get mailForm() { return mailForm },
    get proxySetting() { return proxySetting },
    get reading() { return reading },
    get snap() { return snap },
    get tab() { return tab },
    get waySetting() { return waySetting },
    get writing() { return writing },
  })

  // --- Writing a letter ----------------------------------------------------------------------
  // The same menu as a picker: a model or an effort, the one set ticked.
  let menuPick = null
  // In settings-letter.js:
  const {
    addFiles, closeMenu, fitFrames, hand, handMenu, loadInbox, menuEl, netNow, openChoices, openLetter, openMenu,
    openPicker, pageMail, pageNet, proxyAddress, proxySetting, replyIn, runNetTest, say, sendDraft, setProxy,
    star, step, waySetting, write, writeAbout,
  } = window.SettingsParts.letter({
    AGENTS, COLOR, MOST_FILES, PROXY_KINDS, T, Widgets, accountOf, agentsHTML, body, choicesOf, confOf, draw,
    esc, expanded, firstAgent, handButtons, inbox, inboxHTML, inboxId, isLetter, isReading, keepDraft,
    labelOf, layer, listed, loginWhy, mailDate, mailError, mailFormHTML, mailList, mailNote, mailSize, patch,
    people, providerOf, redrawMail, row, sec, seg, setConf, sw, talkHTML, talkOf, twoPanes, where,
    get lang() { return lang },
    get mailForm() { return mailForm },
    get relayout() { return relayout },
    get snap() { return snap },
    get tab() { return tab },
    get draft() { return draft }, set draft(v) { draft = v },
    get dropSure() { return dropSure }, set dropSure(v) { dropSure = v },
    get handNote() { return handNote }, set handNote(v) { handNote = v },
    get handNoteTimer() { return handNoteTimer }, set handNoteTimer(v) { handNoteTimer = v },
    get menuPick() { return menuPick }, set menuPick(v) { menuPick = v },
    get picking() { return picking }, set picking(v) { picking = v },
    get proxyTyped() { return proxyTyped }, set proxyTyped(v) { proxyTyped = v },
    get reading() { return reading }, set reading(v) { reading = v },
    get writing() { return writing }, set writing(v) { writing = v },
  })

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
  let pluginOpen = false
  let agentOpen = ''
  let othersOpen = false
  // In settings-agents.js:
  const {
    aboutHTML, agentDo, connectAll, dshSwitch, failed, known, pageAgents, pageCrash, pageUpdate, sessionOpen,
  } = window.SettingsParts.agents({
    Agents, COLOR, T, draw, esc, nameList, netNow, patch, row, sec, seg, sw,
    get crashShown() { return crashShown },
    get failedHTML() { return failedHTML },
    get lang() { return lang },
    get othersOpen() { return othersOpen },
    get relayout() { return relayout },
    get tab() { return tab },
    get toolNames() { return toolNames },
    get agentOpen() { return agentOpen }, set agentOpen(v) { agentOpen = v },
    get connecting() { return connecting }, set connecting(v) { connecting = v },
    get pluginOpen() { return pluginOpen }, set pluginOpen(v) { pluginOpen = v },
    get snap() { return snap }, set snap(v) { snap = v },
  })

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
  // In settings-welcome.js:
  const {
    PAGES, cardShows, displaysRow, failedHTML, screenRow, toolNames, toolsHere,
  } = window.SettingsParts.welcome({
    Agents, COLOR, T, aboutHTML, esc, failed, known, nameList, pageAgents, pageAlerts, pageCrash, pageLook,
    pageMail, pageNet, pageNow, pageUpdate, pageWidgets, part, row, sec, seg, sw, thumb, winMode,
    get connecting() { return connecting },
    get failedShortcuts() { return failedShortcuts },
    get lang() { return lang },
    get pluginOpen() { return pluginOpen },
    get recordingShortcut() { return recordingShortcut },
    get snap() { return snap },
    get welcomeOpen() { return welcomeOpen },
  })

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
      (box.id ? `#${box.id}` : box.dataset.field ? `[data-field="${box.dataset.field}"]` : box.dataset.mon ? `[data-mon="${box.dataset.mon}"]` : box.dataset.hold ? '[data-hold]' : box.dataset.mf ? `[data-mf="${box.dataset.mf}"]` : box.dataset.w ? `[data-w="${box.dataset.w}"]` : box.dataset.proxyAt !== undefined ? `[data-proxy-at="${box.dataset.proxyAt}"]` : null)
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
    fitFrames()
    fillCards()
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
    drawSheet()
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
    // The market, by category (widgets:market[:widget|pet|wallpaper]: the
    // picker's 获取更多…, the Look page's): on the Plugins page, in its place.
    const toMarket = AddonUI.marketLink(next)
    if (toMarket) {
      tab = 'widgets'
      sub = { page: 'market', back: null }
      market.cat = toMarket.cat
      market.query = ''
    } else if (PART_OF[next]) {
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
      if (tab === 'widgets' && sub?.page === 'market') loadMarket(false)
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
    // The Agent page opens again with its rows closed; the Plugins page as
    // itself, nothing asked over it, no row edged as new.
    agentOpen = ''
    othersOpen = false
    sub = null
    gone = null
    mkAsk = null
    setRecording(null)
    if (flow.step === 'ask' || flow.step === 'refused') flow = AddonUI.IDLE
    freshRows.clear()
    drawSheet()
    toastEl.hidden = true
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
    sub = null
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
    // The question over the panel (installing a plugin, taking one away):
    // its buttons, its link to the developer docs; a press beside it is 取消.
    if (!sheet.hidden && sheet.contains(e.target)) {
      const site = at('[data-site]')
      if (site) return window.pet.settings.openSite(site.dataset.site)
      const b = at('[data-sheet]')
      if (b?.disabled) return
      const what = b ? b.dataset.sheet : at('.sh') ? '' : 'cancel'
      if (what === 'go') return installGo()
      if (what === 'uninstall') return uninstallGo()
      if (what === 'approve') return approveGo()
      // The market's: installing one that asks for more, 全部更新 asked first.
      if (what === 'mkgo' || what === 'mkall') {
        const asked = mkAsk
        mkAsk = null
        drawSheet()
        return what === 'mkgo' ? marketInstall(asked?.id, true) : updateAll(true)
      }
      if (what === 'cancel' && !(gone?.busy || flow.step === 'installing')) {
        gone = null
        mkAsk = null
        approveFor = null
        flowStep({ type: 'cancel' })
      }
      return
    }
    if (at('[data-lock]')) return patch(snap.settings.settingsPin ? { settingsPin: false, settingsTop: false } : { settingsPin: true })
    if (at('[data-top]')) return patch(snap.settings.settingsPin && snap.settings.settingsTop ? { settingsTop: false } : { settingsPin: true, settingsTop: true })
    if (at('[data-close]')) {
      // Locked: ✕ closes them (and Esc), a press on the head no more.
      if (snap.settings.settingsPin && !at('.x[data-close]')) return
      return close('click')
    }
    const keyBind = at('[data-key-bind]')
    if (keyBind) {
      const key = keyBind.dataset.keyBind
      setRecording(recordingShortcut === key ? null : key)
      return draw()
    }
    const keyClear = at('[data-key-clear]')
    if (keyClear) {
      const key = keyClear.dataset.keyClear
      setRecording(null)
      return patch({ [key]: '' })
    }
    if (recordingShortcut && !at('.kbadge')) {
      setRecording(null)
      draw()
    }
    const tabButton = at('[data-tab]')
    if (tabButton) return setTab(tabButton.dataset.tab)
    const sessToggle = at('[data-sess-open]')
    if (sessToggle) {
      const id = sessToggle.dataset.sessOpen
      if (sessionOpen.has(id)) sessionOpen.delete(id)
      else sessionOpen.add(id)
      draw()
      return
    }
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
        const view = source === 'lhm' ? snap.lhm : snap.hw
        const picked = Array.isArray(s.monitor?.[name]) ? s.monitor[name] : (view?.sensors || []).filter(x => x.on && !x.toggle).map(x => x.id)
        return patch({ monitor: { ...(s.monitor || {}), [name]: picked.includes(id) ? picked.filter(x => x !== id) : [...picked, id].slice(-32) } })
      }
      // A plugin's yes-or-no setting; someone else's (its manifest's flag:
      // as set, else its default).
      if (key.startsWith('pf:')) {
        const [id, name] = key.slice(3).split('.')
        const p = (snap.plugins || []).find(x => x.id === id)
        return p && setPlugin(id, { [name]: !(p.params.find(f => f.name === name) || {}).value })
      }
      if (key.startsWith('af:')) {
        const [id, name] = key.slice(3).split('.')
        const f = addonField(id, name)
        return f && setPlugin(id, { [name]: (f.value ?? f.default) !== true })
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
    // One put nowhere: 添加小工具…, the picker as her menu's opens it
    // (picker.js fromMenu: on the taskbar, editing it with the picker open;
    // at the top, the picker alone), at its card; the settings make way.
    const add = at('[data-padd]')
    if (add) {
      close('picker')
      return window.Picker?.fromMenu?.({ open: 'picker', focus: add.dataset.padd || '' })
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
      const [kind, id] = pack.dataset.pack.split(':')
      return openSub({ page: 'pack', kind, id })
    }
    const look = at('[data-pack-look]')
    if (look) return toLook(look.dataset.packLook)
    const packOut = at('[data-pack-uninstall]')
    if (packOut) {
      const [kind, id] = packOut.dataset.packUninstall.split(':')
      gone = { id, kind, busy: false }
      return drawSheet()
    }
    // A pet's 删除 in the Look page's list: as a pack's 卸载, in its own words.
    const petOut = at('[data-pet-delete]')
    if (petOut) {
      gone = { id: petOut.dataset.petDelete, kind: 'pet', busy: false, del: true }
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
    const mkDo = at('[data-mk-do]')
    if (mkDo) {
      if (mkDo.disabled) return
      const id = mkDo.dataset.id
      if (mkDo.dataset.mkDo === 'install' || mkDo.dataset.mkDo === 'update') return marketInstall(id)
      if (mkDo.dataset.mkDo === 'look') return toLook(marketItem(id)?.kind)
      if (mkDo.dataset.mkDo === 'add') {
        close('picker')
        return window.Picker?.fromMenu?.({ open: 'picker', focus: `plugin:${id}` })
      }
      return
    }
    const mkItem = at('[data-mk-item]')
    if (mkItem) return openSub({ page: 'mkItem', id: mkItem.dataset.mkItem })
    if (at('[data-mk-retry]')) return loadMarket(true)
    if (at('[data-mk-net]')) {
      tab = 'now'
      sub = null
      draw(true)
      relayout()
      partToShow = 'net'
      return showPart()
    }
    const mkAll = at('[data-mk-all]')
    if (mkAll && !mkAll.disabled) return updateAll()
    const revive = at('[data-revive]')
    if (revive && !revive.disabled) {
      revive.disabled = true
      return window.pet.addons.revive(revive.dataset.revive).then(got => {
        if (got && typeof got === 'object' && got.settings) snap = got
        draw()
      })
    }
    const allow = at('[data-approve]')
    if (allow && !allow.disabled) {
      approveFor = allow.dataset.approve
      return drawSheet()
    }
    const out = at('[data-uninstall]')
    if (out) {
      gone = { id: out.dataset.uninstall, busy: false }
      return drawSheet()
    }
    if (at('[data-addon-file]')) return installFromFile()
    const many = at('[data-achoice]')
    if (many) {
      const [id, name] = many.dataset.achoice.split('.')
      const f = addonField(id, name)
      if (!f) return
      const choices = (f.options || []).map((o, i) => [o, f.optionLabels?.[i] || o])
      return openChoices(many, choices, f.value ?? f.default ?? f.options?.[0], v => setPlugin(id, { [name]: v }), true)
    }
    const option = at('[data-seg] > span')
    if (option) {
      if (option.classList.contains('off')) return
      const key = option.parentElement.dataset.seg
      const raw = option.dataset.value
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
    // The Network proxy: shown (from an update not reached), tried.
    if (at('[data-net-go]')) return ((partToShow = 'net'), showPart())
    const netButton = at('[data-net-test]')
    if (netButton && !netButton.disabled) return runNetTest()
    if (at('[data-crash-show]')) return ((crashShown = !crashShown), draw())
    const crashButton = at('[data-crash]')
    if (crashButton && !crashButton.disabled) return window.pet.settings.crash(crashButton.dataset.crash).then(got => ((snap = got), draw()))
    const paperButton = at('[data-paper]')
    if (paperButton && !paperButton.disabled) return paperDo(paperButton.dataset.paper, paperButton.dataset.packId)
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
    // DSH's hook bridge again, its packages from npmmirror (picked by the person).
    const mirror = at('[data-dsh-mirror]')
    if (mirror) return mirror.disabled ? undefined : dshSwitch(failed.dsh?.via || 'bridge', 'npmmirror')
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
    // Google's pages for the address being added (data-for), when it says.
    if (site) return window.pet.settings.openSite(site.dataset.site, site.dataset.for)
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
    // The Mail page's proxy, the General page's Network proxy: its mode, a manual one's kind.
    const way = at('[data-seg^="mailProxy."] > span, [data-seg^="netProxy."] > span')
    if (way) {
      const [key, field] = way.parentElement.dataset.seg.split('.')
      setProxy(key, { [field]: way.dataset.value })
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
      f.result = { bad: true, text: T('mail.w.outPrefix') + mailError(got.error, { host: String(f.smtpHost).trim(), port: String(f.smtpPort || '').trim() || '465 / 587', proxy: formProxy(f) }) }
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
      f.result = { bad: true, text: mailError(got.error, { host: String(f.host).trim(), port: String(f.port || '').trim() || '993 / 143', proxy: formProxy(f) }) }
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
    // Gmail takes an app password alone: anything else (the usual Google
    // password, most likely) is not tried, which Google might hold against
    // the account.
    if (providerOf(f.host) === 'google' && f.password && !APP_PASSWORD.test(String(f.password).trim())) {
      f.result = { bad: true, text: T('mail.err.notAppPassword') }
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
    const password = providerOf(account.host) === 'google' && APP_PASSWORD.test(typed.trim()) ? typed.replace(/\s/g, '') : typed
    const got = await window.pet.mail.save(account, password)
    if (mailForm !== f) return
    f.busy = ''
    if (got.ok) {
      mailForm = null
      snap = got.snapshot
    } else {
      const domain = got.error?.kind === 'login' && (got.auths || []).some(a => a === 'ntlm' || a === 'gssapi')
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
    if (e.target.dataset.mf && mailForm) mailForm[e.target.dataset.mf] = e.target.value
    if (e.target.dataset.proxyAt !== undefined) proxyTyped = { key: e.target.dataset.proxyAt, text: e.target.value, bad: false }
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
    // The market's words looked for: its list at once.
    if (e.target.id === 'mk-q') {
      market.query = e.target.value
      draw()
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
    if (e.target.dataset.proxyAt !== undefined) return proxyAddress(e.target.dataset.proxyAt, e.target.value)
    if (e.target.dataset.mon) return monitorField(e.target.dataset.mon)
    if (e.target.dataset.hold) return holdTyped(e.target)
    const key = e.target.dataset.field
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
    if (recordingShortcut) {
      e.preventDefault()
      e.stopPropagation()
      const key = recordingShortcut
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

      const parts = []
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

      const s = snap.settings
      const defaults = { shortcutApprove: 'Alt+Y', shortcutDeny: 'Alt+N', shortcutIsland: 'Alt+W' }
      const others = ['shortcutApprove', 'shortcutDeny', 'shortcutIsland'].filter(k => k !== key)
      if (others.some(k => (s[k] ?? defaults[k]).toLowerCase() === combo.toLowerCase())) {
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
      // The question over the panel first: 取消 (not while it works).
      if (!sheet.hidden) {
        if (!(gone?.busy || flow.step === 'installing')) {
          gone = null
          mkAsk = null
          flowStep({ type: 'cancel' })
        }
        return
      }
      return close('Esc')
    }
    // A row that is a button (从文件安装…, 获取更多, one of the market's), by the keyboard as a press.
    if ((e.key === 'Enter' || e.key === ' ') && e.target.matches?.('[data-addon-file], [data-mk-open], [data-mk-item], [data-look-market]')) {
      e.preventDefault()
      return e.target.click()
    }
    // Asked over the panel: its buttons alone (the pages under it stay).
    if (!sheet.hidden) return
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
  window.pet.onBlur(() => !snap?.settings?.settingsPin && !picking && !pickingPaper && !pickingZip && !lhmAsks() && (!window.pet.mac || Date.now() - openedAt > BLUR_GRACE_MS) && close('blur'))
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
