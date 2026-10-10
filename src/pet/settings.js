// The settings, grown out of the island: one click on it opens them (and only
// then does the island take the keyboard); Esc, a click outside, ✕ or the head
// close them. Five pages:
//   now       the sessions (a click on one goes to its window), and four quick switches
//   pets      the pets downloaded, a download by link or id, the gallery
//   look      her home (island, taskbar), size, strolls, eyes, language; Windows'
//             own mode and accent colour
//   alerts    how long endings stay, notifications, sound, prompts, quiet
//   widgets   plugins: the built-in ones, those she runs for you (a switch, their settings folded under), their order, turns, nudges, waku, and how to write one
//   mail      mail accounts, and setting one up as Thunderbird does: an address and a password, the server found or typed
//   connect   how Claude Code and Codex reach her, start at login, the hooks, about
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

  const { t } = window.I18n
  const { CLIPS } = window.Sprite
  const Status = window.Status
  const Widgets = window.Widgets

  // The island's colours (on black), the same as island.js.
  const COLOR = { idle: '#8e8e93', working: '#5e9bff', waiting: '#ffb340', done: '#34d27b', review: '#b18cff', error: '#ff5c6c' }
  // Four pages: General (the sessions, the alerts, connecting, startup and
  // the language), Look (her home, Windows, her and the pets), Plugins and
  // Mail. The pets, the alerts and connecting were pages of their own: asked
  // for by those names (island.rs asks for the pets, or connecting), the
  // page they are part of opens, scrolled to them (PART_OF, showPart).
  const TABS = ['now', 'look', 'widgets', 'mail']
  const TAB_KEY = { now: 's.tabGeneral', look: 's.tabLook', widgets: 's.tabWidgets', mail: 's.tabMail' }
  const PART_OF = { pets: 'look', alerts: 'now', connect: 'now' }
  const HEAD_KEY = { working: 's.headWorking', waiting: 's.headWaiting', done: 's.headDone', review: 's.headReview', error: 's.headError' }
  const SIZES = [['small', 0.4], ['medium', 0.55], ['large', 0.75]]
  const HOMES = ['island', 'taskbar']
  const HOME_KEY = { island: 's.displayIsland', taskbar: 's.displayTaskbar' }
  const HOLDS = ['seen', 8, 30, 120]
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
  let tab = 'now'
  let snap = null
  let lang = 'en'
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
  // The mail account being set up or changed, as Thunderbird's dialog has
  // it: the address and password; then (step found) the server looked up,
  // or (step manual) the server's fields; busy while looking or signing in;
  // what came of the last try. Null while none is.
  let mailForm = null
  // An account's delete asked once: asked again, it goes.
  let mailDelete = ''
  // The Mail page's inbox: whose (an account id), its newest letters and how
  // many there are, how many asked for, busy, what went wrong. The letter
  // open ({ uid, busy | letter, summary | error }), null while the list
  // shows. What came of handing one to an agent, for a few seconds.
  const inbox = { id: '', for: '', filter: 'all', letters: [], total: 0, count: 50, busy: false, error: null, errors: [] }
  // A letter to open once the Mail page shows (a click on the island's word that an agent answered).
  let letterToOpen = null
  let reading = null
  let handNote = null
  let handNoteTimer
  const AGENTS = { claude: 'Claude', codex: 'Codex' }
  // The letter being written, one at a time: from which account, to whom,
  // the subject and text, the letter it answers or passes on, files added;
  // touched once typed in; sending, what went wrong. Kept until sent or
  // thrown away, across a restart too (in the page's storage, its files
  // left out). writing: it shows; else the inbox does, with a line to go
  // back to it. A discard asked once: asked again, it goes.
  const DRAFT_KEY = 'wakuwaku.mailDraft'
  let draft = loadDraft()
  let writing = false
  let dropSure = false
  let draftTimer
  // The file picker is open for a letter's files: the focus it takes does
  // not close the settings (the desktop's picture has its own, pickingPaper).
  let picking = false
  // The most a letter takes with it (as send.rs has it).
  const MOST_FILES = 25 << 20

  function loadDraft() {
    try {
      const d = JSON.parse(localStorage.getItem(DRAFT_KEY) || 'null')
      return d && typeof d === 'object' && d.account ? { ...d, files: [], busy: false, error: null, note: null } : null
    } catch {
      return null
    }
  }

  function keepDraft() {
    clearTimeout(draftTimer)
    draftTimer = setTimeout(() => {
      try {
        if (draft) localStorage.setItem(DRAFT_KEY, JSON.stringify({ ...draft, files: [], busy: false, error: null, note: null }))
        else localStorage.removeItem(DRAFT_KEY)
      } catch {}
    }, 300)
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
  const seg = (key, options, current) =>
    `<span class="seg" data-seg="${key}">${options.map(([value, label]) => `<span data-value="${esc(value)}" class="${String(value) === String(current) ? 'on' : ''}">${esc(label)}</span>`).join('')}</span>`
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
              x.agent === 'codex' ? 'Codex' : '',
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
      : `<div class="note">${esc(T('home.noSessions'))}</div>`
    // Her out on the desktop; and her home, the next one at a press.
    const tiles = [
      ['show', 'eye', T('s.outTile'), !snap.hidden && s.out === true],
      ['dnd', 'moon', T('menu.dnd'), s.dnd],
      ['sound', 'sound', T('settings.sound'), s.sound],
      ['home', 'pill', T(HOME_KEY[s.display] || 's.displayIsland'), true],
    ]
    const welcome = !s.onboarded && snap.connection === 'none' ? `<div class="welcome">${esc(T('s.welcome'))}</div>` : ''
    return `${welcome}${sec(T('home.sessions'))}<div class="grp">${list}</div>
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

  function pageLook() {
    const s = snap.settings
    return `${sec(T('home.display'))}<div class="modes">
        <div class="mode${s.display === 'taskbar' ? '' : ' on'}" data-mode="island"><div class="pv"><i></i></div><div class="l"><span class="rd"></span>${esc(T('s.displayIsland'))}</div></div>
        <div class="mode${s.display === 'taskbar' ? ' on' : ''}" data-mode="taskbar"><div class="pv"><s></s></div><div class="l"><span class="rd"></span>${esc(T('s.displayTaskbar'))}</div></div>
      </div>
      ${s.display === 'taskbar' ? `<div class="grp"><div class="note">${esc(T('s.taskbarNote'))}</div></div><div class="grp">${row(esc(T('s.taskbarMaterial')), esc(T('s.taskbarMaterialNote')), seg('taskbarMaterial', ['mica', 'black', 'clear'].map(m => [m, T(`s.material.${m}`)]), s.taskbarMaterial || 'mica'))}${s.taskbarMaterial === 'clear' ? row(esc(T('s.clearWhen')), esc(T('s.clearWhenNote')), seg('taskbarClearWhen', ['mica', 'solid', 'always'].map(w => [w, T(`s.clearWhen.${w}`)]), ['solid', 'always'].includes(s.taskbarClearWhen) ? s.taskbarClearWhen : 'mica')) : ''}${row(esc(T('s.taskbarButtons')), esc(T('s.taskbarButtonsNote')), seg('taskbarButtons', ['icons', 'labels'].map(b => [b, T(`s.buttons.${b}`)]), s.taskbarButtons || 'icons'))}${row(esc(T('s.taskbarAlign')), '', seg('taskbarAlign', ['center', 'left'].map(a => [a, T(`s.align.${a}`)]), s.taskbarAlign || 'center'))}</div>` : ''}
      <div class="grp">${row(esc(T('s.islandWidth')), esc(T('s.islandWidthNote')), seg('islandWidth', ['narrow', 'normal', 'wide'].map(w => [w, T(`s.width.${w}`)]), s.islandWidth || 'normal'))}</div>
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
    const holds = HOLDS.map(h => [h, h === 'seen' ? T('s.holdSeen') : h >= 60 ? T('settings.minutes', { n: h / 60 }) : T('settings.seconds', { n: h })])
    const waits = WAITS.map(w => [w, w >= 60 ? T('settings.minutes', { n: Math.round(w / 60) }) : T('settings.seconds', { n: w })])
    const notify = s.notify || {}
    return `${sec(T('s.holdTitle'))}<div class="grp">${row(esc(T('s.hold')), '', seg('hold', holds, s.hold))}</div>
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
      const why = p.said ? T('pl.stopped', { line: p.said }) : T('pl.stoppedQuiet')
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
      const value = key in drafts ? drafts[key] : f.value || ''
      return `<label class="fr"><span class="fl">${label}${f.required ? ' *' : ''}</span><input class="in" data-field="${esc(key)}" value="${esc(value)}" placeholder="${esc(T(`pl.${key}.hint`))}" spellcheck="false"${f.kind === 'number' ? ' inputmode="decimal"' : ''}></label>`
    })
    return `<div class="pf">${fields.join('')}</div>`
  }

  // The monitor's settings, folded out under it: its parts on or off, and
  // how often it reads (1 to 10 seconds).
  function monitorFields() {
    const m = snap.settings.monitor || {}
    const parts = ['cpu', 'mem', 'net', 'battery'].map(k => `<div class="fr"><span class="fl">${esc(T(`mon.${k}`))}</span><span class="grow"></span>${sw('mon:' + k, m[k] !== false)}</div>`)
    const every = m.every || 2
    const pace = `<div class="fr"><span class="fl">${esc(T('mon.every'))}</span><span class="grow"></span><button class="pbtn sm" data-every="-1" ${every <= 1 ? 'disabled' : ''}>−</button><span class="every">${esc(T('settings.seconds', { n: every }))}</span><button class="pbtn sm" data-every="1" ${every >= 10 ? 'disabled' : ''}>+</button></div>`
    const pin = `<div class="fr"><span class="fl">${esc(T('mon.pin'))}</span><span class="d grow">${esc(T('mon.pinNote'))}</span>${sw('mon:pin', m.pin !== false)}</div>`
    return `<div class="pf">${pin}${parts.join('')}${pace}</div>`
  }

  // Plugins: built in, run by her, or anyone's script, each with its switch
  // (and a step up); how they take turns, whether they may open the island,
  // waku for the terminal, and how to write one.
  function pageWidgets() {
    const s = snap.settings
    const left = ms => (ms >= 60000 ? T('settings.minutes', { n: Math.round(ms / 60000) }) : T('settings.seconds', { n: Math.max(1, Math.round(ms / 1000)) }))
    const rows = widgetRows()
      .map((r, i) => {
        const up = i > 0 ? `<button class="pbtn sm" data-wup="${esc(r.key)}" title="${esc(T('w.up'))}">↑</button>` : ''
        if (r.plugin) {
          const p = r.plugin
          const says = r.mine.map(w => Widgets.words(lang, w, s.details !== false)).map(({ label, value }) => [label, value].filter(Boolean).join(' ')).join(' · ')
          const note = pluginNote(p, r.mine)
          const color = (r.mine[0] && r.mine[0].color) || (p.on ? '#5e9bff' : COLOR.idle)
          const isOpen = openPlugin === p.id
          const more = p.params.length ? `<button class="pbtn sm" data-pexp="${esc(p.id)}">${esc(T(isOpen ? 'pl.fold' : 'pl.settings'))}</button>` : ''
          return `<div class="r"><span class="wi" style="color:${esc(color)}">${Widgets.icon(p.icon)}</span><div class="grow"><div class="ellip">${esc(T(`w.ex.${p.id}`))}<span class="d"> ${esc(says)}</span></div><div class="d${note.bad ? ' bad' : ''}">${esc(note.text)}</div></div>${more}${up}${sw('pl:' + p.id, p.on)}</div>${isOpen ? pluginFields(p) : ''}`
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
        return `<div class="r"><span class="wi" style="color:${esc(w.color || COLOR.idle)}">${Widgets.icon(w.icon)}</span><div class="grow"><div class="ellip">${esc(label)}${says}</div><div class="d">${esc(from)}</div></div>${more}${up}${sw('w:' + w.id, w.on)}</div>${isMonitor && openPlugin === 'monitor' ? monitorFields() : ''}`
      })
      .join('')
    const spins = SPINS.map(n => [n, n ? T('settings.seconds', { n }) : T('w.spinOff')])
    const waku = `powershell -ExecutionPolicy Bypass -File "${snap.waku}" npm run build`
    const example = `Invoke-RestMethod -Method Post http://127.0.0.1:${snap.port}/widget -ContentType application/json -Body '{"id":"hello","label":"Hello","value":"42","icon":"star"}'`
    return `${sec(T('w.section'))}<div class="grp"><div class="note">${esc(T('w.note'))}</div>${rows}</div>
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

  // What went wrong with a mail account, in words.
  function mailError(err, server) {
    const kind = err?.kind || 'other'
    const where = server ? `${server.host}:${server.port}` : ''
    // The server's own words, where they say why.
    const detail = err?.text && ['login', 'refused', 'other', 'recipient'].includes(kind) ? (lang === 'zh' ? '：' : ': ') + err.text : ''
    return T(`mail.err.${kind}`, { where }) + detail
  }

  // An account's line: how its inbox is, or why it is not.
  function mailNote(a) {
    if (!a.on) return { text: T('mail.off') }
    const st = a.status || {}
    if (st.state === 'ok') return { text: st.unread ? T('mail.unread', { n: st.unread }) : T('mail.none') }
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

  // Who a letter goes to first (the setting), and both in that order.
  const firstAgent = () => (snap.settings.mailAgent === 'codex' ? 'codex' : 'claude')
  const agentOrder = () => (firstAgent() === 'codex' ? ['codex', 'claude'] : ['claude', 'codex'])
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

  const mailSize = n => (n >= 1 << 20 ? `${(n / (1 << 20)).toFixed(1)} MB` : n >= 1024 ? `${Math.round(n / 1024)} KB` : `${n} B`)
  const people = list => (list || []).map(p => (p.name ? `${p.name} <${p.address}>` : p.address)).join(', ')
  const starHTML = (account, uid, on) => `<button class="star${on ? ' on' : ''}" data-star="${esc(account)}" data-uid="${uid}" title="${esc(T(on ? 'mail.unstar' : 'mail.star'))}">${on ? '★' : '☆'}</button>`

  // A letter's line in the inbox: unread or not, who, when, what about, its
  // star, whose (in every account's), and whether an agent is on it.
  function letterRow(l, every) {
    const talk = talkOf(letterKey(l))
    const chip = talk ? `<span class="chip ${talk.state}">${esc(T(`mail.chip.${talk.state}`, { agent: AGENTS[talk.agent] || 'Agent' }))}</span>` : ''
    const whose = every ? `<span class="acct ellip">${esc(accountOf(l.account)?.address || '')}</span>` : ''
    // The one open beside the list (two panes).
    const open = reading && reading.account === l.account && reading.uid === l.uid ? ' open' : ''
    return `<div class="r letter${l.seen ? '' : ' unread'}${open}" data-letter="${l.uid}" data-account="${esc(l.account)}"><span class="udot"></span>
      <div class="grow"><div class="lt"><span class="who ellip">${esc(l.from || l.address || '?')}</span><span class="when">${esc(mailDate(l.date))}</span></div>
      <div class="lt"><span class="d ellip grow">${l.answered ? `<span class="answered" title="${esc(T('mail.answered'))}">↩</span> ` : ''}${l.attached ? '📎 ' : ''}${esc(l.subject || T('mail.noSubject'))}</span>${whose}</div></div>${chip}${starHTML(l.account, l.uid, l.flagged)}</div>`
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
    const head = `<div class="sec row-sec"><span class="ellip">${esc(T('mail.inbox'))}${inbox.total ? ` · ${inbox.total}` : ''}</span><span class="grow"></span><span class="link" data-w-new>${esc(T('mail.write'))}</span><span class="link" data-mail-refresh>${esc(T(inbox.busy ? 'mail.loading' : 'mail.refresh'))}</span>${setup}${panes}</div>`
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
    else if (!inbox.letters.length) list = `<div class="note">${esc(T(inbox.busy ? 'mail.loading' : `mail.empty.${inbox.filter}`))}</div>`
    else list = inbox.letters.map(l => letterRow(l, every)).join('')
    const left = inbox.total - inbox.letters.length
    const more = !inbox.error && left > 0 ? `<div class="r"><button class="pbtn wide" data-mail-more ${inbox.busy ? 'disabled' : ''}>${esc(T('mail.more', { n: Math.min(50, left) }))}</button></div>` : ''
    return `${head}<div class="grp inbox">${tools}${kept}${note}${missed}${list}${more}</div>`
  }

  // Who a letter goes to first, and how each agent takes it.
  function agentsHTML() {
    const agent = row(esc(T('mail.agent')), esc(T('mail.agentNote')), seg('mailAgent', [['claude', 'Claude Code'], ['codex', 'Codex']], firstAgent()))
    return `<div class="grp agent-pick">${agent}</div>${agentOrder().map(agentConfHTML).join('')}`
  }

  // How a letter goes to each agent (mailAgentConf, agent.rs): how much its
  // talk may do, and its model and effort; empty is the agent's own setting.
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

  function agentConfHTML(ag) {
    const c = confOf(ag)
    const access = ['read', 'ask', 'mine'].includes(c.access) ? c.access : 'read'
    const missing = (snap.mailAgents || {})[ag] ? '' : ` · ${T('mail.hand.missing')}`
    return `<div class="sec">${esc(ag === 'claude' ? 'Claude Code' : 'Codex')}${esc(missing)}</div><div class="grp agent-conf">
      ${row(esc(T('mail.conf.access')), esc(T(`mail.conf.${access}Note.${ag}`)), seg(`mconf.${ag}`, ['read', 'ask', 'mine'].map(a => [a, T(`mail.conf.${a}`)]), access))}
      <div class="r"><div class="grow">${esc(T('mail.conf.session'))}</div>${pickerHTML(ag, 'model')}${pickerHTML(ag, 'effort')}</div>
    </div>`
  }

  // One of them changed; an effort the new model does not take goes back
  // to the agent's own.
  function setConf(ag, key, value) {
    const all = snap.settings.mailAgentConf || {}
    const next = { ...(all[ag] || {}), [key]: value }
    if (key === 'model' && next.effort && !effortsOf(ag, value).includes(next.effort)) next.effort = ''
    patch({ mailAgentConf: { ...all, [ag]: next } })
  }

  // A little markdown, as agents write: lines, **bold**, `code`, headings,
  // lists, quotes, ``` blocks (a reply it wrote, often). The text is escaped
  // first.
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
      const line = raw.replace(/\*\*(.+?)\*\*/g, '<b>$1</b>').replace(/`([^`]+)`/g, '<code>$1</code>')
      let m
      if ((m = /^\s*([-*•]|\d+[.)])\s+(.*)$/.exec(line))) html += `<div class="md-li"><span>${m[1] === '*' ? '•' : m[1]}</span><span>${m[2]}</span></div>`
      else if ((m = /^#{1,4}\s+(.*)$/.exec(line))) html += `<div class="md-h">${m[1]}</div>`
      else if ((m = /^&gt;\s?(.*)$/.exec(line))) html += `<div class="md-q">${m[1] || '&nbsp;'}</div>`
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

  // The buttons above an open letter: its star, reply, hand it to the first
  // agent (when no talk is there yet), and ⋯ for the rest.
  function handButtons(l, talk) {
    const ag = firstAgent()
    const off = !(snap.mailAgents || {})[ag] ? 'disabled' : ''
    const give = talk ? '' : `<button class="pbtn sm al" data-hand="${ag}:talk" ${off}>${esc(T('mail.hand.talk', { agent: AGENTS[ag] }))}</button>`
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
      const back = l.replyTo?.length ? l.replyTo : l.from
      const everyone = eachOnce([...notMe(back), ...notMe(l.to)])
      const to = kind === 'all' && everyone.length ? everyone : back
      d.to = peopleLine(to)
      if (kind === 'all') d.cc = peopleLine(eachOnce(notMe(l.cc)).filter(p => !to.some(t => samePerson(t, p))))
      d.showCc = !!d.cc
      d.subject = /^\s*(re|aw|sv|回复|答复)\s*[:：]/i.test(l.subject || '') ? l.subject : `Re: ${l.subject || ''}`
      d.text = `${words ? `${words}\n\n` : '\n\n'}${T('mail.w.wrote', { date: mailDate(l.date, true), who: people(l.from) })}\n${quoted(l.text)}`
      d.reply = { account: l.account, uid: l.uid, messageId: l.messageId, references: l.references || [] }
    }
    if (l && kind === 'forward') {
      d.subject = /^\s*(fwd?|wg|转发)\s*[:：]/i.test(l.subject || '') ? l.subject : `Fwd: ${l.subject || ''}`
      d.text = `\n\n${forwarded(l)}`
      d.forward = { account: l.account, uid: l.uid, files: (l.attachments || []).map((f, i) => ({ index: i, name: f.name, size: f.size })) }
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
  async function writeAbout(kind, account, uid, words) {
    if (!(reading?.letter && reading.account === account && reading.uid === uid)) await openLetter(account, uid)
    if (reading?.letter && reading.account === account && reading.uid === uid) write(kind, reading.letter, words)
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
      forward: d.forward ? { account: d.forward.account, uid: d.forward.uid, keep: d.forward.files.map(f => f.index) } : null,
      files: d.files.map(f => ({ name: f.name, type: f.type, data: f.data })),
    }
    const got = await window.pet.mail.send(letter)
    if (draft !== d) return
    d.busy = false
    if (!got.ok) {
      d.error = sendError(got)
      return redrawMail()
    }
    // The letter answered shows it.
    const answered = d.reply && inbox.letters.find(l => l.account === d.reply.account && l.uid === d.reply.uid)
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
    const whose = (snap.mail || []).length > 1 ? `<div class="d">${esc(T('mail.in', { address: accountOf(l.account)?.address || '' }))}</div>` : ''
    return `${bar}${note}<div class="grp mail-read" data-letter="${l.uid}" data-account="${esc(l.account)}">
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
      ${mailForm ? mailFormHTML() : ''}`
  }

  // The letter beside this one in the list (two panes, ↑ ↓), opened.
  function step(by) {
    const at = inbox.letters.findIndex(l => reading && l.account === reading.account && l.uid === reading.uid)
    const next = inbox.letters[at < 0 ? (by > 0 ? 0 : inbox.letters.length - 1) : at + by]
    if (!next) return
    openLetter(next.account, next.uid)
    requestAnimationFrame(() => body.querySelector(`.inbox [data-letter="${next.uid}"][data-account="${CSS.escape(next.account)}"]`)?.scrollIntoView({ block: 'nearest' }))
  }

  // The newest letters of the inbox shown, as filtered; with more, 50 more.
  async function loadInbox(more) {
    const id = inboxId()
    if (!id || inbox.busy) return
    const which = `${id}|${inbox.filter}`
    if (inbox.for !== which) Object.assign(inbox, { letters: [], total: 0, count: 50, error: null, errors: [] })
    if (more) inbox.count += 50
    Object.assign(inbox, { id, for: which, busy: true, error: null })
    redrawMail()
    const got = await window.pet.mail.letters(id, inbox.count, inbox.filter)
    inbox.busy = false
    if (inbox.for !== which) return redrawMail()
    if (got.ok) Object.assign(inbox, { letters: got.letters, total: got.total, errors: got.errors || [] })
    else inbox.error = got.error
    redrawMail()
  }

  // A letter opened: read whole (and marked read) while the page waits.
  async function openLetter(account, uid) {
    if (!accountOf(account)) return
    reading = { account, uid, busy: true }
    draw(true)
    relayout()
    const got = await window.pet.mail.letter(account, uid)
    if (reading?.uid !== uid || reading?.account !== account) return
    reading = got.ok ? { account, uid, letter: got.letter, talk: got.talk } : { account, uid, error: got.error }
    const listed = inbox.letters.find(l => l.uid === uid && l.account === account)
    if (listed && got.ok) Object.assign(listed, { seen: true, flagged: got.letter.flagged })
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
  async function hand(account, uid, agent, how) {
    closeMenu()
    if (how === 'talk' && !(reading?.account === account && reading?.uid === uid)) await openLetter(account, uid)
    noteFor({ text: T('mail.hand.busy', { agent: AGENTS[agent] }) })
    const got = await window.pet.mail.hand(account, uid, agent, how)
    if (got.ok && how === 'talk' && reading?.letter && reading.uid === uid) reading.talk = null
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
  async function star(account, uid) {
    const listed = inbox.letters.find(l => l.uid === uid && l.account === account)
    const open = reading?.letter && reading.account === account && reading.uid === uid ? reading.letter : null
    const on = !(listed || open)?.flagged
    for (const l of [listed, open]) if (l) l.flagged = on
    redrawMail()
    const got = await window.pet.mail.flag(account, uid, on)
    if (!got.ok) {
      for (const l of [listed, open]) if (l) l.flagged = !on
      noteFor({ text: mailError(got.error, accountOf(account)), bad: true })
    }
  }

  // The menu a right-click on a letter opens: each agent (a talk, or a
  // terminal), the first agent first, one not found greyed out; its star.
  const menuEl = document.createElement('div')
  menuEl.className = 'mmenu'
  menuEl.hidden = true
  layer.append(menuEl)

  function openMenu(account, uid, x, y) {
    const found = snap.mailAgents || {}
    const item = (ag, how, label) =>
      `<button class="mi" data-hand="${ag}:${how}" data-account="${esc(account)}" data-uid="${uid}" ${found[ag] ? '' : 'disabled'}>${esc(label)}${found[ag] ? '' : ` <span class="d">${esc(T('mail.hand.missing'))}</span>`}</button>`
    const talks = agentOrder().map(ag => item(ag, 'talk', T('mail.hand.talk', { agent: AGENTS[ag] }))).join('')
    const terminals = agentOrder().map(ag => item(ag, 'open', T('mail.hand.open', { agent: AGENTS[ag] }))).join('')
    const l = inbox.letters.find(x => x.uid === uid && x.account === account) || (reading?.uid === uid ? reading.letter : null)
    const starItem = `<button class="mi" data-star="${esc(account)}" data-uid="${uid}">${esc(T(l?.flagged ? 'mail.unstar' : 'mail.star'))}</button>`
    const replies = [
      ['reply', 'mail.reply'],
      ['all', 'mail.replyAll'],
      ['forward', 'mail.forward'],
    ]
      .map(([kind, key]) => `<button class="mi" data-w-reply="${kind}" data-account="${esc(account)}" data-uid="${uid}">${esc(T(key))}</button>`)
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
  // that is off is there, greyed.
  function openChoices(button, choices, value, onPick, together = false) {
    menuEl.innerHTML = choices
      .map(
        ([v, label, off], i) =>
          `${i === 1 && !together ? '<div class="mi-sep"></div>' : ''}<button class="mi${v === value ? ' on' : ''}" data-pick="${esc(v)}" ${off ? 'disabled' : ''}>${esc(label)}${off ? ` <span class="d">${esc(T('mail.unsupported'))}</span>` : ''}</button>`,
      )
      .join('')
    menuPick = onPick
    const r = button.getBoundingClientRect()
    placeMenu(r.left, r.bottom + 4)
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
    openMenu(letter.dataset.account, Number(letter.dataset.letter), e.clientX, e.clientY)
  })
  body.addEventListener('scroll', closeMenu)

  function pageConnect() {
    const conn = { plugin: 'home.connPlugin', hooks: 'home.connHooks', both: 'home.connBoth', none: 'home.connNone' }[snap.connection] || 'home.connNone'
    const isOk = snap.connection === 'plugin' || snap.connection === 'hooks'
    const sessions = (snap.sessions || []).length
    const hooksWord = {
      ok: 'settings.hooksOk',
      missing: 'settings.hooksMissing',
      stale: 'settings.hooksStale',
      partial: 'settings.hooksPartial',
      httpOnly: 'settings.hooksHttpOnly',
      unreadable: 's.hooksUnreadable',
    }[snap.hooks]
    const hooksButtons =
      snap.hooks === 'missing'
        ? `<button class="pbtn" data-hooks="install">${esc(T('settings.install'))}</button>`
        : snap.hooks === 'unreadable'
          ? ''
          : `<button class="pbtn" data-hooks="install">${esc(T(snap.hooks === 'ok' || snap.hooks === 'httpOnly' ? 'settings.reinstall' : 'settings.repair'))}</button><button class="pbtn no" data-hooks="remove">${esc(T('settings.remove'))}</button>`
    // Codex: hooks in its hooks.json, which it runs only once trusted (/hooks):
    // until an event came, they may be waiting for that.
    const codex = snap.codex || {}
    // upgradable: all works; Codex was updated since and runs background hooks now.
    const codexIn = ['ok', 'stale', 'upgradable', 'partial'].includes(codex.hooks)
    const codexConn =
      codex.hooks === 'ok' && codex.seen
        ? 's.codexConnOk'
        : codex.hooks === 'ok'
          ? 's.codexConnTrust'
          : codex.hooks === 'upgradable'
            ? 's.codexConnUpgrade'
            : codexIn
              ? 's.codexConnRepair'
              : 's.codexConnNone'
    const codexColor = codexConn === 's.codexConnOk' ? COLOR.done : codex.hooks === 'upgradable' ? COLOR.working : codexIn ? COLOR.waiting : COLOR.idle
    const codexWord = {
      ok: 'settings.hooksOk',
      missing: 'settings.hooksMissing',
      stale: 'settings.hooksStale',
      upgradable: 's.codexUpgradable',
      partial: 'settings.hooksPartial',
      unreadable: 's.codexUnreadable',
      absent: 's.codexAbsent',
    }[codex.hooks]
    const codexButtons =
      codex.hooks === 'missing'
        ? `<button class="pbtn" data-codex="install">${esc(T('settings.install'))}</button>`
        : codexIn
          ? `<button class="pbtn" data-codex="install">${esc(T(codex.hooks === 'ok' ? 'settings.reinstall' : 'settings.repair'))}</button><button class="pbtn no" data-codex="remove">${esc(T('settings.remove'))}</button>`
          : ''
    return `${sec(T('s.status'))}<div class="grp"><div class="r"><span class="dot" style="background:${isOk ? COLOR.done : snap.connection === 'both' ? COLOR.waiting : COLOR.idle}"></span><div class="grow">${esc(T(conn))}</div><span class="d">${esc(T('s.sessionsOnline', { n: sessions }))}</span></div>
        <div class="r"><span class="dot" style="background:${codexColor}"></span><div class="grow">${esc(T(codexConn))}</div></div></div>
      ${sec(T('home.plugin'))}<div class="grp"><div class="note">${esc(T('home.pluginWhy'))}</div>
        ${snap.pluginCommands.map(c => `<div class="r"><span class="cmd mono">${esc(c)}</span><button class="pbtn" data-copy="${esc(c)}">${esc(T('home.copy'))}</button></div>`).join('')}
      </div>
      ${sec(T('home.advanced'))}<div class="grp">
        <div class="note">${esc(T('home.advancedWhy'))}</div>
        <div class="r"><div class="grow"><div>${esc(T('settings.hooks'))}: ${esc(T(hooksWord))}</div><div class="d mono ellip">${esc(snap.settingsFile)}</div></div>${hooksButtons}</div>
      </div>
      ${sec(T('s.codex'))}<div class="grp">
        <div class="note">${esc(T('s.codexWhy'))}</div>
        <div class="r"><div class="grow"><div>${esc(T('settings.hooks'))}: ${esc(T(codexWord))}</div><div class="d mono ellip">${esc(codex.file)}</div></div>${codexButtons}</div>
        ${codex.hooks === 'upgradable' ? `<div class="note">${esc(T('s.codexUpgradeNote'))}</div>` : ''}
        ${codexIn && codex.hooks !== 'upgradable' && !codex.seen ? `<div class="note">${esc(T('s.codexTrust'))}</div>` : ''}
        ${codex.hooks !== 'absent' && !codex.async ? `<div class="note">${esc(T('s.codexOld', { version: codex.version }))}</div>` : ''}
      </div>
      ${sec(T('settings.about'))}<div class="grp">
        <div class="r"><div class="grow">${esc(T('s.version', { version: snap.version }))}</div><span class="link" data-site="repo">${esc(T('home.repo'))} ›</span></div>
        <div class="note">${esc(T('home.credits'))}</div>
      </div>`
  }

  // General: the sessions and the quick switches, the alerts, startup and
  // the language, connecting (the plugin, the hooks) and what this is.
  function pageGeneral() {
    return `${pageNow()}
      ${part('alerts', pageAlerts())}
      ${sec(T('s.startLang'))}<div class="grp">
        ${row(esc(T('settings.startAtLogin')), esc(T('home.pluginStart')), sw('login', snap.loginAtStart))}
        ${row(esc(T('home.language')), '', seg('lang', [['auto', T('settings.languageAuto')], ['zh', '中文'], ['en', 'EN']], snap.settings.lang))}
      </div>
      ${part('connect', pageConnect())}`
  }

  const PAGES = { now: pageGeneral, look: pageLook, widgets: pageWidgets, mail: pageMail }

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
  }

  // The page, kept as it is when nothing changed; redrawn, the box being
  // typed in (the download box, a plugin's setting) gets the focus and the
  // caret back.
  function drawBody(reset) {
    const box = body.contains(document.activeElement) && document.activeElement.matches('input, textarea') ? document.activeElement : null
    const which =
      box &&
      (box.id ? `#${box.id}` : box.dataset.field ? `[data-field="${box.dataset.field}"]` : box.dataset.mf ? `[data-mf="${box.dataset.mf}"]` : box.dataset.w ? `[data-w="${box.dataset.w}"]` : null)
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
    const at = partToShow && body.querySelector(`[data-part="${partToShow}"]`)
    partToShow = ''
    if (at) body.scrollTop += at.getBoundingClientRect().top - body.getBoundingClientRect().top
  }

  function open(next) {
    if (PART_OF[next]) {
      tab = PART_OF[next]
      partToShow = next
    } else if (TABS.includes(next)) {
      tab = next
    }
    const wasOpen = isOpen
    isOpen = true
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
      draw(true)
      showPart()
      if (tab === 'look') loadGallery(false)
      if (tab === 'mail' && letterToOpen) {
        const { account, uid } = letterToOpen
        letterToOpen = null
        openLetter(account, uid)
      }
      layer.focus({ preventScroll: true })
    })
    window.Island?.changed()
  }

  function close(why = 'asked') {
    if (!isOpen) return
    console.log('settings close:', why)
    isOpen = false
    clearInterval(clockTimer)
    for (const timer of thumbTimers) clearTimeout(timer)
    thumbTimers = []
    if (layer.contains(document.activeElement)) document.activeElement.blur()
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
      setTimeout(() => (button.textContent = T('home.copy')), 1500)
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
      if (key === 'paperShuffle') return paperDo('shuffle', !snap.winPaper?.shuffle)
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
      const key = option.parentElement.dataset.seg
      const raw = option.dataset.value
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
      const value = ['lang', 'islandWidth', 'taskbarMaterial', 'taskbarClearWhen', 'taskbarButtons', 'taskbarAlign', 'mailAgent'].includes(key) ? raw : raw === 'seen' ? 'seen' : Number(raw)
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
    const hooks = at('[data-hooks]')
    if (hooks) {
      return window.pet.settings.hooks(hooks.dataset.hooks).then(got => {
        snap = got.snapshot
        draw()
        relayout()
      })
    }
    const codexHooks = at('[data-codex]')
    if (codexHooks) {
      return window.pet.settings.codexHooks(codexHooks.dataset.codex).then(got => {
        snap = got.snapshot
        draw()
        relayout()
      })
    }
    const site = at('[data-site]')
    if (site) return window.pet.settings.openSite(site.dataset.site)
  })

  // A mail account's setup: what was clicked, if it was its. True when it was.
  function handleMail(at) {
    // How a letter goes to an agent: a model or effort picked, a picker
    // opened, how much a session may do.
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
    const access = at('[data-seg^="mconf."] > span')
    if (access) {
      setConf(access.parentElement.dataset.seg.slice(6), 'access', access.dataset.value)
      return true
    }
    // The inbox and a letter: handed to an agent (from a button or the
    // menu), starred, the menu, back, again, more, another inbox or filter,
    // one opened; more asked in a talk.
    const handing = at('[data-hand]')
    if (handing) {
      const [agent, how] = handing.dataset.hand.split(':')
      if (!handing.disabled) hand(handing.dataset.account || reading?.account, Number(handing.dataset.uid || reading?.uid), agent, how)
      return true
    }
    const starring = at('[data-star]')
    if (starring) {
      closeMenu()
      star(starring.dataset.star, Number(starring.dataset.uid))
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
      openMenu(reading.account, reading.uid, r.right - 220, r.bottom + 4)
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
      loadInbox(false)
      return true
    }
    if (at('[data-mail-more]')) {
      loadInbox(true)
      return true
    }
    const letter = at('.inbox [data-letter]')
    if (letter) {
      // Beside the list, a letter being written is put aside for it.
      if (twoPanes()) writing = false
      openLetter(letter.dataset.account, Number(letter.dataset.letter))
      return true
    }
    const turned = at('[data-sw^="mail:"]')
    if (turned) {
      const a = (snap.mail || []).find(x => x.id === turned.dataset.sw.slice(5))
      if (a) window.pet.mail.switch(a.id, !a.on).then(got => ((snap = got), draw()))
      return true
    }
    if (at('[data-mail-add]')) {
      mailForm = { id: '', address: '', name: '', password: '', host: '', port: 993, security: 'ssl', auth: 'auto', username: '', ...outFields(null), step: 'start', advanced: false, probed: null, probedOut: null, busy: '', result: null }
      mailDelete = ''
      redrawMail('[data-mf="address"]')
      return true
    }
    const change = at('[data-mail-edit]')
    if (change) {
      const a = (snap.mail || []).find(x => x.id === change.dataset.mailEdit)
      if (a) {
        mailForm = { id: a.id, address: a.address, name: a.name || '', password: '', host: a.host, port: a.port, security: a.security, auth: a.auth || 'auto', username: a.username, ...outFields(a.smtp), step: 'found', source: '', advanced: true, probed: null, probedOut: null, busy: '', result: null }
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
      writeAbout(re.dataset.wReply, re.dataset.account || reading?.account, Number(re.dataset.uid || reading?.uid))
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
    const got = await window.pet.mail.discover(f.address.trim())
    if (mailForm !== f) return
    f.busy = ''
    f.probed = null
    f.probedOut = null
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
    const got = await window.pet.mail.discover(f.address)
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
    const got = await window.pet.mail.probe(String(f.smtpHost).trim(), String(f.smtpPort ?? '').trim(), f.smtpSecurity || 'auto', 'smtp')
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
    const got = await window.pet.mail.probe(String(f.host).trim(), String(f.port ?? '').trim(), f.security || 'auto')
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
    const account = { id: f.id, address: f.address.trim(), name: String(f.name || '').trim(), host: String(f.host).trim(), port: Number(f.port), security: f.security, auth: f.auth || 'auto', username, smtp }
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
    if (e.target.dataset.w && draft) {
      draft[e.target.dataset.w] = e.target.value
      draft.touched = true
      dropSure = false
      keepDraft()
    }
    if (e.target.id === 's-ref') {
      typedRef = e.target.value
      // Not mid-word in an input method: once the word is chosen.
      if (!e.isComposing) searchSoon()
    }
    if (e.target.dataset.field) drafts[e.target.dataset.field] = e.target.value
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
    const key = e.target.dataset.field
    if (!key || !(key in drafts)) return
    const value = drafts[key].trim()
    delete drafts[key]
    const [id, name] = key.split('.')
    const field = ((snap.plugins || []).find(p => p.id === id)?.params || []).find(f => f.name === name)
    if (!field || (field.kind === 'number' && value && !(Number(value) > 0))) return draw()
    setPlugin(id, { [name]: value })
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
      if (e.key === 'Enter' && e.target.dataset.field) e.target.blur()
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
  })
  // The keyboard went elsewhere: a click outside the island; not a file
  // picker (a letter's files, the desktop's picture: its own window takes
  // the focus).
  // Locked (the button by ✕), they stay.
  window.pet.onBlur(() => !snap?.settings?.settingsPin && !picking && !pickingPaper && close('blur'))
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
  }
})()
