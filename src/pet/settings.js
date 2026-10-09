// The settings, grown out of the island: one click on it opens them (and only
// then does the island take the keyboard); Esc, a click outside, ✕ or the head
// close them. Five pages:
//   now       the sessions (a click on one goes to its window), and four quick switches
//   pets      the pets downloaded, a download by link or id, the gallery
//   look      pet or island, size, bubble, strolls, eyes, language
//   alerts    how long endings stay, notifications, sound, prompts, quiet
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

  // The island's colours (on black), the same as island.js.
  const COLOR = { idle: '#8e8e93', working: '#5e9bff', waiting: '#ffb340', done: '#34d27b', review: '#b18cff', error: '#ff5c6c' }
  const TABS = ['now', 'pets', 'look', 'alerts', 'connect']
  const TAB_KEY = { now: 's.tabNow', pets: 's.tabPets', look: 's.tabLook', alerts: 's.tabAlerts', connect: 's.tabConnect' }
  const HEAD_KEY = { working: 's.headWorking', waiting: 's.headWaiting', done: 's.headDone', review: 's.headReview', error: 's.headError' }
  const SIZES = [['small', 0.4], ['medium', 0.55], ['large', 0.75]]
  const HOLDS = ['seen', 8, 30, 120]
  const WAITS = [30, 60, 120, 290]
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

  function setHTML(el, html) {
    if (el._html === html) return false
    el._html = html
    el.innerHTML = html
    return true
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
    // Show the pet: in the island, whether she is out on the desktop (she is
    // always in sight there); as the pet on her own, shown or hidden.
    const isShown = s.display === 'island' ? !snap.hidden && s.out === true : !snap.hidden
    const tiles = [
      ['show', 'eye', T('home.showPet'), isShown],
      ['dnd', 'moon', T('menu.dnd'), s.dnd],
      ['sound', 'sound', T('settings.sound'), s.sound],
      ['island', 'pill', T('s.island'), s.display === 'island'],
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
        <div class="mode${s.display === 'pet' ? ' on' : ''}" data-mode="pet"><div class="pv">${current ? thumb(42, current.url, current.version) : ''}</div><div class="l"><span class="rd"></span>${esc(T('home.displayPet'))}</div></div>
        <div class="mode${s.display === 'island' ? ' on' : ''}" data-mode="island"><div class="pv"><i></i></div><div class="l"><span class="rd"></span>${esc(T('s.displayIsland'))}</div></div>
      </div>
      ${sec(T('s.her'))}<div class="grp">
        ${row(esc(T('menu.size')), '', seg('scale', SIZES.map(([name, scale]) => [scale, T(`menu.${name}`)]), s.scale))}
        ${row(esc(T('s.bubble')), esc(T('s.bubbleNote')), sw('bubble', s.bubble))}
        ${row(esc(T('s.details')), esc(T('s.detailsNote')), sw('details', s.details !== false))}
        ${row(esc(T('s.walk')), esc(T('s.walkNote')), sw('walk', s.walk))}
        ${row(esc(T('s.look')), '', sw('look', s.look))}
      </div>
      ${sec(T('s.general'))}<div class="grp">${row(esc(T('home.language')), '', seg('lang', [['auto', T('settings.languageAuto')], ['zh', '中文'], ['en', 'EN']], s.lang))}</div>`
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

  const PAGES = { now: pageNow, pets: pagePets, look: pageLook, alerts: pageAlerts, connect: pageConnect }

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

  // The page, kept as it is when nothing changed (a redraw would take the
  // focus and the caret from the download box).
  function drawBody(reset) {
    const box = document.getElementById('s-ref')
    const hadFocus = document.activeElement === box
    const caret = hadFocus ? box.selectionStart : null
    const changed = setHTML(body, PAGES[tab]())
    if (reset) body.scrollTop = 0
    if (!changed) return
    const again = document.getElementById('s-ref')
    if (again) {
      again.value = typedRef
      if (hadFocus) {
        again.focus()
        again.setSelectionRange(caret, caret)
      }
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
    // Never taller than the screen: the page scrolls inside instead.
    const rest = layer.offsetHeight - body.offsetHeight
    body.style.maxHeight = `${Math.max(160, Math.min(460, screen.availHeight - 56 - rest))}px`
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

    const toggle = at('[data-sw]')
    if (toggle && !toggle.disabled) {
      const key = toggle.dataset.sw
      if (key === 'login') return window.pet.settings.login(!snap.loginAtStart).then(got => ((snap = got), draw()))
      if (key.startsWith('notify.')) return patch({ notify: { ...s.notify, [key.slice(7)]: !s.notify?.[key.slice(7)] } })
      return patch({ [key]: !s[key] })
    }
    const option = at('[data-seg] > span')
    if (option) {
      const key = option.parentElement.dataset.seg
      const raw = option.dataset.value
      if (key === 'gallery') {
        if (gallery.sort === raw) return
        gallery.sort = raw
        gallery.page = 0
        gallery.items = []
        return loadGallery(false)
      }
      const value = key === 'lang' ? raw : raw === 'seen' ? 'seen' : Number(raw)
      return patch({ [key]: value })
    }
    const tile = at('[data-tile]')
    if (tile) {
      const key = tile.dataset.tile
      if (key === 'show') {
        if (s.display === 'island' && !snap.hidden) return patch({ out: s.out !== true })
        return window.pet.settings.showPet(snap.hidden).then(got => ((snap = got), draw()))
      }
      if (key === 'island') return patch({ display: s.display === 'island' ? 'pet' : 'island' })
      return patch({ [key]: !s[key] })
    }
    const mode = at('[data-mode]')
    if (mode) return mode.dataset.mode !== s.display && patch({ display: mode.dataset.mode })
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

  layer.addEventListener('input', e => {
    if (e.target.id === 's-ref') typedRef = e.target.value
  })

  document.addEventListener('keydown', e => {
    if (!isOpen) return
    if (e.key === 'Escape') {
      e.preventDefault()
      return close('Esc')
    }
    if (e.target.matches?.('input, textarea')) {
      if (e.key === 'Enter' && e.target.id === 's-ref') layer.querySelector('[data-fetch]')?.click()
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
  }
})()
