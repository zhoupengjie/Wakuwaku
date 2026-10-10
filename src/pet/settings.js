// The settings, grown out of the island: one click on it opens them (and only
// then does the island take the keyboard); Esc, a click outside, ✕ or the head
// close them. Five pages:
//   now       the sessions (a click on one goes to its window), and four quick switches
//   pets      the pets downloaded, a download by link or id, the gallery
//   look      her home (corner, island, bar, taskbar), size, bubble, strolls, eyes,
//             language; Windows' own mode and accent colour
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
  const TABS = ['now', 'pets', 'look', 'alerts', 'widgets', 'mail', 'connect']
  const TAB_KEY = { now: 's.tabNow', pets: 's.tabPets', look: 's.tabLook', alerts: 's.tabAlerts', widgets: 's.tabWidgets', mail: 's.tabMail', connect: 's.tabConnect' }
  const HEAD_KEY = { working: 's.headWorking', waiting: 's.headWaiting', done: 's.headDone', review: 's.headReview', error: 's.headError' }
  const SIZES = [['small', 0.4], ['medium', 0.55], ['large', 0.75]]
  const HOMES = ['corner', 'island', 'bar', 'taskbar']
  const HOME_KEY = { corner: 's.displayCorner', island: 's.displayIsland', bar: 's.displayBar', taskbar: 's.displayTaskbar' }
  const CORNERS = ['br', 'bl', 'tr', 'tl']
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
  // The gallery, loaded the first time the pets page is shown.
  const gallery = { sort: 'popular', items: [], page: 0, totalPages: 1, loading: false, error: '', getting: '' }
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
    return `${sec(T('home.installed'))}<div class="grp">${installed}</div>
      ${sec(T('s.download'))}<div class="grp"><div class="r"><input class="in" id="s-ref" spellcheck="false" placeholder="${esc(T('s.fetchPlaceholder'))}"><button class="pbtn al" data-fetch ${fetching ? 'disabled' : ''}>${esc(fetching ? T('settings.fetching') : T('settings.fetch'))}</button></div>${note}</div>
      <div class="sec row-sec"><span>${esc(T('home.gallery'))}</span><span class="grow"></span>${seg('gallery', [['popular', T('home.popular')], ['newest', T('home.newest')]], gallery.sort)}</div>
      ${gallery.error ? `<div class="note err">${esc(T('home.galleryError', { message: gallery.error }))}</div>` : ''}
      <div class="cards">${cards}</div>${more}
      <div class="links"><span class="link" data-site="site">${esc(T('settings.browse'))} ›</span></div>`
  }

  function pageLook() {
    const s = snap.settings
    const current = snap.pets.find(p => p.id === s.pet)
    return `${sec(T('home.display'))}<div class="modes">
        <div class="mode${s.display === 'corner' ? ' on' : ''}" data-mode="corner"><div class="pv"><b class="circle">${current ? thumb(24, current.url, current.version) : ''}</b></div><div class="l"><span class="rd"></span>${esc(T('s.displayCorner'))}</div></div>
        <div class="mode${s.display === 'island' ? ' on' : ''}" data-mode="island"><div class="pv"><i></i></div><div class="l"><span class="rd"></span>${esc(T('s.displayIsland'))}</div></div>
        <div class="mode${s.display === 'bar' ? ' on' : ''}" data-mode="bar"><div class="pv"><u></u></div><div class="l"><span class="rd"></span>${esc(T('s.displayBar'))}</div></div>
        <div class="mode${s.display === 'taskbar' ? ' on' : ''}" data-mode="taskbar"><div class="pv"><s></s></div><div class="l"><span class="rd"></span>${esc(T('s.displayTaskbar'))}</div></div>
      </div>
      ${s.display === 'taskbar' ? `<div class="grp"><div class="note">${esc(T('s.taskbarNote'))}</div></div><div class="grp">${row(esc(T('s.taskbarMaterial')), esc(T('s.taskbarMaterialNote')), seg('taskbarMaterial', ['mica', 'black'].map(m => [m, T(`s.material.${m}`)]), s.taskbarMaterial || 'mica'))}${row(esc(T('s.taskbarButtons')), esc(T('s.taskbarButtonsNote')), seg('taskbarButtons', ['icons', 'labels'].map(b => [b, T(`s.buttons.${b}`)]), s.taskbarButtons || 'icons'))}${row(esc(T('s.taskbarAlign')), '', seg('taskbarAlign', ['center', 'left'].map(a => [a, T(`s.align.${a}`)]), s.taskbarAlign || 'center'))}</div>` : ''}
      ${s.display === 'corner' ? `<div class="grp">${row(esc(T('s.corner')), '', seg('corner', CORNERS.map(c => [c, T(`s.corner.${c}`)]), s.corner || 'br'))}</div>` : ''}
      ${s.display !== 'corner' ? `<div class="grp">${row(esc(T('s.islandWidth')), esc(T('s.islandWidthNote')), seg('islandWidth', ['narrow', 'normal', 'wide'].map(w => [w, T(`s.width.${w}`)]), s.islandWidth || 'normal'))}</div>` : ''}
      ${sec(T('s.her'))}<div class="grp">
        ${row(esc(T('menu.size')), '', seg('scale', SIZES.map(([name, scale]) => [scale, T(`menu.${name}`)]), s.scale))}
        ${row(esc(T('s.bubble')), esc(T('s.bubbleNote')), sw('bubble', s.bubble))}
        ${row(esc(T('s.details')), esc(T('s.detailsNote')), sw('details', s.details !== false))}
        ${row(esc(T('s.walk')), esc(T('s.walkNote')), sw('walk', s.walk))}
        ${row(esc(T('s.look')), '', sw('look', s.look))}
      </div>
      ${sec(T('s.general'))}<div class="grp">${row(esc(T('home.language')), '', seg('lang', [['auto', T('settings.languageAuto')], ['zh', '中文'], ['en', 'EN']], s.lang))}</div>
      ${pageWindows()}`
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
  function pageWindows() {
    const look = snap.winLook || {}
    const accent = String(look.accent || '').toLowerCase()
    const swatches = ACCENTS.map(c => `<span class="acc${c === accent ? ' on' : ''}" data-accent="${c}" style="background:${c}" title="${c}"></span>`).join('')
    return `${sec(T('s.windows'))}<div class="grp">
        ${row(esc(T('s.winMode')), esc(T('s.winModeNote')), seg('winMode', [['light', T('s.winMode.light')], ['dark', T('s.winMode.dark')]], look.light ? 'light' : 'dark'))}
        <div class="r col"><div>${esc(T('s.winAccent'))}</div><div class="accents">${swatches}</div></div>
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
    const detail = err?.text && ['login', 'refused', 'other'].includes(kind) ? (lang === 'zh' ? '：' : ': ') + err.text : ''
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

  // Setting an account up: the address and the password; then the server
  // found (a line) or not; and, folded as Thunderbird's manual setup, the
  // incoming server's every setting, with Re-test.
  function mailFormHTML() {
    const f = mailForm
    const field = (key, label, attrs = '') =>
      `<label class="fr"><span class="fl">${esc(label)}</span><input class="in" data-mf="${key}" value="${esc(f[key] ?? '')}" spellcheck="false" ${attrs}></label>`
    const head = [
      field('address', T('mail.address'), `placeholder="you@example.com" autocomplete="off"${f.id ? ' disabled' : ''}`),
      field('password', T('mail.password'), `type="password" autocomplete="off" placeholder="${esc(f.id ? T('mail.passwordKeep') : '')}"`),
      `<div class="note">${esc(T('mail.passwordNote'))}</div>`,
    ].join('')
    const found =
      f.step === 'found'
        ? `<div class="r"><div class="grow"><div class="ellip">IMAP · ${esc(f.host)}:${esc(f.port)} · ${esc(labelOf(SECURITIES(), f.security))}${f.auth && f.auth !== 'auto' ? ` · ${esc(labelOf(AUTHS(), f.auth))}` : ''}</div>${f.source ? `<div class="d">${esc(T(`mail.source.${f.source}`))}</div>` : ''}</div></div>`
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

  // The fold: Thunderbird's manual setup for the incoming server (protocol,
  // host name, port, connection security, authentication method, user name),
  // what Re-test found, and the outgoing server, not here yet.
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
      probedHTML(),
      `<div class="adv-sec">${esc(T('mail.adv.out'))}</div><div class="note">${esc(T('mail.adv.outNote'))}</div>`,
    ]
    return `<div class="adv on">${toggle}<div class="adv-b">${rows.join('')}</div></div>`
  }

  // What Re-test found: how it is reached, how it lets one sign in, and,
  // for a Windows domain's server (Exchange), what the user name may be.
  function probedHTML() {
    const p = mailForm.probed
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
    return `<div class="r letter${l.seen ? '' : ' unread'}" data-letter="${l.uid}" data-account="${esc(l.account)}"><span class="udot"></span>
      <div class="grow"><div class="lt"><span class="who ellip">${esc(l.from || l.address || '?')}</span><span class="when">${esc(mailDate(l.date))}</span></div>
      <div class="lt"><span class="d ellip grow">${l.attached ? '📎 ' : ''}${esc(l.subject || T('mail.noSubject'))}</span>${whose}</div></div>${chip}${starHTML(l.account, l.uid, l.flagged)}</div>`
  }

  // Which inbox, as the picker names it: every account's (with all their
  // unread), or one (with its).
  function boxName(id) {
    const all = snap.mail || []
    if (id === '*') return T('mail.box.all', { n: all.reduce((n, a) => n + unreadOf(a), 0) })
    const a = accountOf(id)
    return a ? T('mail.box.one', { address: a.address, n: unreadOf(a) }) : ''
  }

  // The inbox: which (every account's, or one), all, unread or starred, its
  // newest letters, more on asking, and who a letter goes to.
  function inboxHTML(id) {
    const all = snap.mail || []
    const every = id === '*'
    const pick = all.length > 1 ? `<button class="pick" data-mail-box>${esc(boxName(id))} ▾</button>` : ''
    const filters = seg('mail.filter', [['all', T('mail.filter.all')], ['unseen', T('mail.filter.unseen')], ['flagged', T('mail.filter.flagged')]], inbox.filter)
    const head = `<div class="sec row-sec"><span>${esc(T('mail.inbox'))}${inbox.total ? ` · ${inbox.total}` : ''}</span><span class="grow"></span><span class="link" data-mail-refresh>${esc(T(inbox.busy ? 'mail.loading' : 'mail.refresh'))}</span></div>`
    const tools = `<div class="r mail-tools">${pick}<span class="grow"></span>${filters}</div>`
    const note = handNote ? `<div class="note${handNote.bad ? ' err' : ''}">${esc(handNote.text)}</div>` : ''
    // Accounts that could not be asked, in every account's.
    const missed = (inbox.errors || []).map(e => `<div class="note err">${esc(`${accountOf(e.account)?.address || ''}: ${mailError(e.error, accountOf(e.account))}`)}</div>`).join('')
    let list
    if (inbox.error) list = `<div class="note err">${esc(mailError(inbox.error, accountOf(id)))}</div>`
    else if (!inbox.letters.length) list = `<div class="note">${esc(T(inbox.busy ? 'mail.loading' : `mail.empty.${inbox.filter}`))}</div>`
    else list = inbox.letters.map(l => letterRow(l, every)).join('')
    const left = inbox.total - inbox.letters.length
    const more = !inbox.error && left > 0 ? `<div class="r"><button class="pbtn wide" data-mail-more ${inbox.busy ? 'disabled' : ''}>${esc(T('mail.more', { n: Math.min(50, left) }))}</button></div>` : ''
    const agent = row(esc(T('mail.agent')), esc(T('mail.agentNote')), seg('mailAgent', [['claude', 'Claude Code'], ['codex', 'Codex']], firstAgent()))
    return `${head}<div class="grp inbox">${tools}${note}${missed}${list}${more}</div><div class="grp agent-pick">${agent}</div>${agentOrder().map(agentConfHTML).join('')}`
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
  // lists, quotes. The text is escaped first.
  function md(text) {
    let html = ''
    for (const raw of esc(text).split('\n')) {
      const line = raw.replace(/\*\*(.+?)\*\*/g, '<b>$1</b>').replace(/`([^`]+)`/g, '<code>$1</code>')
      let m
      if ((m = /^\s*([-*•]|\d+[.)])\s+(.*)$/.exec(line))) html += `<div class="md-li"><span>${m[1] === '*' ? '•' : m[1]}</span><span>${m[2]}</span></div>`
      else if ((m = /^#{1,4}\s+(.*)$/.exec(line))) html += `<div class="md-h">${m[1]}</div>`
      else if ((m = /^&gt;\s?(.*)$/.exec(line))) html += `<div class="md-q">${m[1] || '&nbsp;'}</div>`
      else html += line.trim() ? `<div>${line}</div>` : '<div class="md-gap"></div>'
    }
    return html
  }

  // The talk under a letter: who with, how it is going, what was said (the
  // agent's words as they come, what it did in small), and a box to ask more.
  function talkHTML(talk, letter) {
    const agent = AGENTS[talk.agent] || 'Agent'
    const going = talk.state === 'running'
    const state = going ? T('mail.talk.going', { agent }) : talk.state === 'failed' ? T('mail.talk.failed', { agent, why: talk.error || '' }) : T('mail.talk.done', { agent })
    const turns = talk.turns
      .map(t => {
        if (t.who === 'start') return `<div class="t-start">${esc(T('mail.talk.start', { agent }))}</div>`
        if (t.who === 'me') return `<div class="t-me">${esc(t.text)}</div>`
        if (t.who === 'tool') return `<div class="t-tool">· ${esc(t.text)}</div>`
        return `<div class="t-agent">${md(t.text)}</div>`
      })
      .join('')
    const thinking = going && talk.turns.at(-1)?.who !== 'agent' ? `<div class="t-wait">${esc(T('mail.talk.thinking'))}</div>` : ''
    const ask = `<div class="t-ask"><input class="in" data-talk="${esc(letter.key)}" placeholder="${esc(T('mail.talk.ask', { agent }))}" ${going ? 'disabled' : ''} spellcheck="false"><button class="pbtn sm al" data-talk-send ${going ? 'disabled' : ''}>${esc(T('mail.talk.send'))}</button></div>`
    return `<div class="talk ${talk.state}"><div class="talk-h"><span class="grow">${esc(state)}</span>${going ? '' : `<span class="link" data-hand="${talk.agent}:talk">${esc(T('mail.talk.again'))}</span>`}</div>${turns}${thinking}${ask}</div>`
  }

  // The buttons above an open letter: its star, hand it to the first agent
  // (when no talk is there yet), and ⋯ for the rest.
  function handButtons(l, talk) {
    const ag = firstAgent()
    const off = !(snap.mailAgents || {})[ag] ? 'disabled' : ''
    const give = talk ? '' : `<button class="pbtn sm al" data-hand="${ag}:talk" ${off}>${esc(T('mail.hand.talk', { agent: AGENTS[ag] }))}</button>`
    return `${starHTML(l.account, l.uid, l.flagged)}${give}<button class="pbtn sm" data-mail-menu title="${esc(T('mail.hand.more'))}">⋯</button>`
  }

  // A letter open: back to the inbox, its star and the hand-off, who, when,
  // its attachments, the talk about it, the text.
  function readingHTML() {
    const r = reading
    const l = r.letter
    const talk = l ? talkOf(l.key) || r.talk : null
    const bar = `<div class="mail-bar"><span class="link" data-mail-back>‹ ${esc(T('mail.inbox'))}</span><span class="grow"></span>${l ? handButtons(l, talk) : ''}</div>`
    const note = handNote ? `<div class="note${handNote.bad ? ' err' : ''}">${esc(handNote.text)}</div>` : ''
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
  // (every account's, or one), and a letter open.
  function pageMail() {
    if (reading) return readingHTML()
    const accounts = snap.mail || []
    const rows = accounts
      .map(a => {
        const note = mailNote(a)
        return `<div class="r"><span class="wi" style="color:${a.on ? '#5e9bff' : COLOR.idle}">${Widgets.icon('mail')}</span><div class="grow"><div class="ellip">${esc(a.address)}</div><div class="d${note.bad ? ' bad' : ''}">${esc(note.text)}</div></div><button class="pbtn sm" data-mail-edit="${esc(a.id)}">${esc(T('pl.settings'))}</button>${sw('mail:' + a.id, a.on)}</div>`
      })
      .join('')
    const add = mailForm ? '' : `<div class="r"><button class="pbtn wide" data-mail-add>${esc(T('mail.add'))}</button></div>`
    const id = inboxId()
    // The inbox comes in the first time it is shown (and for another one or filter).
    if (id && inbox.for !== `${id}|${inbox.filter}` && !inbox.busy) setTimeout(() => loadInbox(false))
    return `${sec(T('mail.section'))}<div class="grp"><div class="note">${esc(T('mail.note'))}</div>${rows}${add}</div>
      ${mailForm ? mailFormHTML() : ''}
      ${id ? inboxHTML(id) : ''}`
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
    menuEl.innerHTML = `${talks}<div class="mi-sep"></div>${terminals}<div class="mi-sep"></div>${starItem}`
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
  // server's own, autodetect, every account) apart; one that is off is there, greyed.
  function openChoices(button, choices, value, onPick) {
    menuEl.innerHTML = choices
      .map(
        ([v, label, off], i) =>
          `${i === 1 ? '<div class="mi-sep"></div>' : ''}<button class="mi${v === value ? ' on' : ''}" data-pick="${esc(v)}" ${off ? 'disabled' : ''}>${esc(label)}${off ? ` <span class="d">${esc(T('mail.unsupported'))}</span>` : ''}</button>`,
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
      ${sec(T('s.startup'))}<div class="grp">
        ${row(esc(T('settings.startAtLogin')), esc(T('home.pluginStart')), sw('login', snap.loginAtStart))}
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

  const PAGES = { now: pageNow, pets: pagePets, look: pageLook, alerts: pageAlerts, widgets: pageWidgets, mail: pageMail, connect: pageConnect }

  // --- Drawing --------------------------------------------------------------------------------

  function drawHead() {
    const now = snap.now
    const pet = snap.pets.find(p => p.id === snap.settings.pet)
    const title = now.mood === 'idle' ? T('s.headIdle', { name: pet ? pet.name : 'Wakuwaku' }) : T(HEAD_KEY[now.mood])
    const asks = (snap.asks || []).length
    const sub = [Status.nameOf(now, snap.settings.details !== false), asks ? T('s.asksWaiting', { n: asks }) : ''].filter(Boolean).join(' · ')
    setHTML(
      head,
      `<div class="grow ellip"><span class="t">${esc(title)}</span>${sub ? ` <span class="s">· ${esc(sub)}</span>` : ''}</div>${clockTag(now)}<button class="x" data-close title="${esc(T('s.close'))}">✕</button>`,
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
    const box = body.contains(document.activeElement) && document.activeElement.matches('input') ? document.activeElement : null
    const which = box && (box.id ? `#${box.id}` : box.dataset.field ? `[data-field="${box.dataset.field}"]` : box.dataset.mf ? `[data-mf="${box.dataset.mf}"]` : null)
    const caret = which ? box.selectionStart : null
    const changed = setHTML(body, PAGES[tab]())
    if (reset) body.scrollTop = 0
    if (!changed) return
    const again = document.getElementById('s-ref')
    if (again) again.value = typedRef
    const back = which && body.querySelector(which)
    if (back) {
      back.focus()
      back.setSelectionRange(caret, caret)
    }
    startThumbs()
  }

  function draw(reset = false) {
    if (!isOpen || !snap) return
    lang = snap.lang || lang
    drawHead()
    drawTabs()
    drawBody(reset)
    foot.querySelector('.foot-text').textContent = T('s.foot')
    foot.querySelector('.ver').textContent = `v${snap.version}`
    tick()
    // Never taller than the screen, nor than the home keeps room for (the
    // taskbar's): the page scrolls inside instead.
    const rest = layer.offsetHeight - body.offsetHeight
    const most = window.Island?.settingsMax?.() ?? screen.availHeight - 56
    body.style.maxHeight = `${Math.max(160, Math.min(460, most - rest))}px`
    window.Island?.changed()
  }

  // After the content changed height: measure once it is laid out.
  const relayout = () => requestAnimationFrame(() => window.Island?.changed())

  // --- Open and close ----------------------------------------------------------------------------

  function open(next) {
    if (TABS.includes(next)) tab = next
    const wasOpen = isOpen
    isOpen = true
    if (!wasOpen) {
      window.pet.keyboard(true)
      clearInterval(clockTimer)
      clockTimer = setInterval(tick, 1000)
    }
    window.pet.settings.get().then(got => {
      snap = got
      draw(true)
      if (tab === 'pets') loadGallery(false)
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
    if (tab === 'pets') loadGallery(false)
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
    if (gallery.loading || (!more && gallery.page > 0)) return
    gallery.loading = true
    gallery.error = ''
    draw()
    const got = await window.pet.settings.gallery(more ? gallery.page + 1 : 1, gallery.sort)
    gallery.loading = false
    if (got.ok) {
      gallery.items = more ? [...gallery.items, ...got.items] : got.items
      gallery.page = got.page || 1
      gallery.totalPages = got.totalPages || 1
    } else {
      gallery.error = got.error
    }
    draw()
    relayout()
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
    if (at('[data-close]')) return close('click')
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
      const value = ['lang', 'corner', 'islandWidth', 'taskbarMaterial', 'taskbarButtons', 'taskbarAlign', 'mailAgent'].includes(key) ? raw : raw === 'seen' ? 'seen' : Number(raw)
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
    const accent = at('[data-accent]')
    if (accent) return window.pet.settings.winLook({ accent: accent.dataset.accent }).then(got => ((snap = got), draw()))
    const use = at('[data-use]')
    if (use) return patch({ pet: use.dataset.use })
    if (at('[data-fetch]')) {
      if (!typedRef.trim()) return document.getElementById('s-ref')?.focus()
      return fetchPet(typedRef.trim(), false)
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
      mailForm = { id: '', address: '', password: '', host: '', port: 993, security: 'ssl', auth: 'auto', username: '', step: 'start', advanced: false, probed: null, busy: '', result: null }
      mailDelete = ''
      redrawMail('[data-mf="address"]')
      return true
    }
    const change = at('[data-mail-edit]')
    if (change) {
      const a = (snap.mail || []).find(x => x.id === change.dataset.mailEdit)
      if (a) mailForm = { id: a.id, address: a.address, password: '', host: a.host, port: a.port, security: a.security, auth: a.auth || 'auto', username: a.username, step: 'found', source: '', advanced: true, probed: null, busy: '', result: null }
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
      openChoices(mfpick, key === 'security' ? SECURITIES() : AUTHS(), f[key] || 'auto', value => {
        f[key] = value
        // The usual port for the way, if the port was the other way's.
        const usual = { ssl: 993, starttls: 143, plain: 143, auto: '' }
        if (key === 'security' && ['993', '143', ''].includes(String(f.port))) f.port = usual[value]
        f.probed = null
        draw()
      })
      return true
    }
    if (at('[data-mail-probe]')) {
      probeServer()
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
    if (got.ok && got.found) {
      Object.assign(f, got.server, { step: 'found', source: got.source, notFound: false, oauth: false, advanced: false })
    } else {
      // Thunderbird's manual setup: the fold open, the way and port to be found.
      Object.assign(f, { step: 'manual', notFound: true, oauth: !!got.oauth, advanced: true, host: '', port: '', security: 'auto', auth: 'auto', username: f.username || f.address.trim() })
    }
    redrawMail(f.step === 'manual' ? '[data-mf="host"]' : null)
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
    f.busy = 'save'
    f.result = null
    redrawMail()
    const account = { id: f.id, address: f.address.trim(), host: String(f.host).trim(), port: Number(f.port), security: f.security, auth: f.auth || 'auto', username: String(f.username || f.address).trim() }
    const got = await window.pet.mail.save(account, f.password)
    if (mailForm !== f) return
    f.busy = ''
    if (got.ok) {
      mailForm = null
      snap = got.snapshot
    } else {
      const domain = got.error?.kind === 'login' && (got.auths || []).some(a => a === 'ntlm' || a === 'gssapi')
      f.result = { bad: true, text: mailError(got.error, account) + (domain ? ` ${T('mail.adv.domain')}` : '') }
    }
    redrawMail()
  }

  layer.addEventListener('input', e => {
    if (e.target.dataset.mf && mailForm) mailForm[e.target.dataset.mf] = e.target.value
    if (e.target.id === 's-ref') typedRef = e.target.value
    if (e.target.dataset.field) drafts[e.target.dataset.field] = e.target.value
  })

  // A plugin's setting kept once typed (Enter, or leaving the box); a
  // number that is none goes back to what it was.
  layer.addEventListener('change', e => {
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
      if (e.key === 'Enter' && e.target.id === 's-ref') layer.querySelector('[data-fetch]')?.click()
      if (e.key === 'Enter' && e.target.dataset.field) e.target.blur()
      if (e.key === 'Enter' && e.target.dataset.mf) layer.querySelector('[data-mail-find]:not([disabled]), [data-mail-save]:not([disabled])')?.click()
      if (e.key === 'Enter' && e.target.dataset.talk && !e.isComposing) say(e.target)
      return
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
  // The keyboard went elsewhere: a click outside the island.
  window.pet.onBlur(() => close('blur'))

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
