// The widget picker (the KDE-like design of 2026-10-17, its prototype the
// user tried): a sidebar at the screen's side, the full height of the room
// above her taskbar's strip, with every widget there is, to drag where it
// should go or to press to add. Loaded after island.js, in her home's page
// (role=island; in her own window it does nothing), and working with
// island.js through window.Isle (its hooks there are marked "picker.js").
//
// Its way in: her taskbar's menu, 添加小工具… (editing the layout, the picker
// open), or the edit bar's 添加小工具; in the island's home her menu's
// 添加小工具… (the picker alone; island:picker).
//
// What it has: a search, the kinds (全部 系统 效率 信息 邮件 脚本), a card
// for each widget (its icon, name and what it is, and "已放 N" where it is
// put), 获取更多… (to come) and 管理… (Settings, 插件). One instance of each
// for now: a card put again is the same widget moved.
//
// Where one goes, by where it is let go of (picker.js hit):
//   the strip      a gap opens where it would go, the others make room
//                  (front half of one: before it; back half: after); a
//                  full strip has its medium ones made small first, past
//                  that it shakes and says no;
//   her capsule    the middle half of her slot on the strip (or the island
//                  at the top, or the capsule's panel): one of its members;
//   the desktop    anywhere else: the desktop's page shows where it lands
//                  and puts it there (desk-drop.js, through the host:
//                  desk_drag_preview, desk_drop);
//   the picker     taken off (a widget dragged from the strip), or nothing
//                  (a card);
//   the bare band  the 90 px above the strip: taken off (from the strip).
// A press on a card: just before the tray (the island's home: into the
// capsule). Placed is on: put somewhere, its plugin runs (plugins[id].on) or
// its widget is on (widgetsOff); taken from its last place, off.
//
// Editing the strip, a widget under the pointer has a popover (its size,
// 设置, 移除); a press on her capsule opens its panel (capsule-panel.js).
// Every widget taken off says so in a toast with 撤销 for 10 s, and is put
// back where it was. Esc: what is open first, then (editing) 完成.
;(function () {
  const Isle = window.Isle
  if (!Isle || !Isle.isOn()) {
    window.Picker = null
    return
  }
  const { t } = window.I18n
  const Widgets = window.Widgets
  const WCards = window.WCards
  const G = window.DeskGrid
  const C = window.Capsule
  const P = window.Placing
  const el = Isle.el
  const T = (key, vars) => t(Isle.lang, key, vars)
  const esc = WCards.esc
  const reduced = () => matchMedia('(prefers-reduced-motion: reduce)').matches
  const clone = v => JSON.parse(JSON.stringify(v))
  const FI = code => `<span class="pk-fi" aria-hidden="true">&#x${code};</span>`
  const inRect = (x, y, r) => !!r && x >= r.left && x <= r.right && y >= r.top && y <= r.bottom
  const rectOf = node => (node && node.isConnected && !node.hidden ? node.getBoundingClientRect() : null)

  // Its kinds, and which a plugin she runs is (scripts.rs PLUGINS).
  const CATS = ['all', 'system', 'work', 'info', 'mail', 'script']
  const PLUGIN_CAT = { weather: 'info', stock: 'info', pomodoro: 'work', countdown: 'work', stretch: 'work', ci: 'script', devserver: 'script' }
  const BUILT_IN_CAT = { monitor: 'system', today: 'work', tokens: 'work' }
  // The spacers' marks (the layout's own parts, on the strip alone).
  const SPACER_MARK = {
    flex: '<path d="M4 12h16M7 9l-3 3 3 3M17 9l3 3-3 3"/>',
    gap: '<path d="M5 7v10M19 7v10M8 12h8"/>',
    sep: '<path d="M12 4v16"/>',
  }
  // The gap a widget opens on the strip as it comes (its width, px).
  const GAP_W = { M: 112, S: 64, part: 96, flex: 40, gap: 20, sep: 14, side: 44 }
  // Dragged this far above the strip (across it), a widget from it goes.
  const BAND_PX = 90
  const UNDO_MS = 10000
  const NOTE_MS = 3200

  // --- What it shows ---------------------------------------------------------------------
  const pk = el('aside', 'pk')
  pk.setAttribute('aria-label', '')
  pk.hidden = true
  pk.innerHTML = `
    <div class="pk-head"><h2></h2><button type="button" class="pk-ib" data-pk="menu">${FI('E712')}</button><button type="button" class="pk-ib" data-pk="close">${FI('E8BB')}</button></div>
    <div class="pk-menu" hidden><button type="button" data-pk="reset"></button></div>
    <label class="pk-search">${FI('E721')}<input type="search" autocomplete="off" spellcheck="false"></label>
    <div class="pk-chips"></div>
    <p class="pk-hint"></p>
    <div class="pk-grid"></div>
    <p class="pk-note" hidden></p>
    <div class="pk-foot"><button type="button" data-pk="more"></button><button type="button" data-pk="manage"></button></div>`
  const $ = sel => pk.querySelector(sel)
  const search = $('input')
  const grid = $('.pk-grid')
  const chips = $('.pk-chips')
  const menu = $('.pk-menu')
  const note = $('.pk-note')

  // The popover over a widget on the strip, the toasts, the ghost under the
  // pointer, the band above the strip, the gap opened on the strip.
  const pop = el('div', 'pk-pop')
  pop.hidden = true
  const toasts = el('div', 'pk-toasts')
  toasts.setAttribute('aria-live', 'polite')
  const ghost = el('div', 'pk-ghost')
  ghost.hidden = true
  ghost.innerHTML = '<span class="pk-gt"></span><span class="pk-gl"></span>'
  const band = el('div', 'pk-band')
  band.hidden = true
  const gapNode = el('span', 'pk-ph')
  for (const node of [pk, pop, toasts]) {
    node.addEventListener('mouseenter', () => window.pet.hover(true))
    node.addEventListener('mouseleave', () => window.pet.hover(false))
    node.addEventListener('contextmenu', e => e.preventDefault())
  }
  document.body.append(pk, pop, toasts, ghost, band)

  let open = false
  let cat = 'all'
  let query = ''
  // settings_get: every widget (on or off) and the plugins she runs.
  let snap = null
  let cards = []
  // A press not yet a drag; the drag; the desktop's last word on it.
  let press = null
  let drag = null
  let deskSaid = null
  let pending = null
  let tickets = 0
  let lastPointer = 'mouse'

  const home = () => Isle.home()
  const editing = () => !!Isle.editLayout
  const light = () => Isle.winLight
  const theme = () => {
    for (const node of [pk, pop, toasts, ghost, band]) node.classList.toggle('pk-l', light())
    const side = Isle.tbEdge()
    pk.classList.toggle('pk-left', home() === 'taskbar' && side === 'right')
    pk.dataset.home = home()
    pk.dataset.edge = home() === 'taskbar' ? side : 'top'
    toasts.dataset.home = home()
    toasts.dataset.edge = home() === 'taskbar' ? side : 'top'
    toasts.classList.toggle('pk-editing', editing())
  }

  // --- The cards -------------------------------------------------------------------------
  const allNow = () => (snap?.widgets?.length ? snap.widgets : Isle.allWidgets)
  const clip = (s, n = 40) => [...String(s || '')].slice(0, n).join('')
  function cardsOf() {
    const all = allNow()
    const detailed = Isle.isDetailed()
    const out = []
    const builtIn = { monitor: 'card.monitor', today: 'card.today', tokens: 'widget.tokens' }
    for (const w of all.filter(x => x.builtIn)) out.push({ key: `widget:${w.id}`, kind: 'builtIn', cat: BUILT_IN_CAT[w.id] || 'system', w, name: T(builtIn[w.id] || 'taskbar.w.widget'), note: T(`picker.d.${w.id in BUILT_IN_CAT ? w.id : 'builtIn'}`) })
    for (const p of snap?.plugins || []) out.push({ key: `plugin:${p.id}`, kind: 'plugin', cat: PLUGIN_CAT[p.id] || 'script', w: all.find(x => x.plugin === p.id) || null, plugin: p, name: T(`w.ex.${p.id}`), note: T(`w.ex.${p.id}.note`) })
    for (const w of all.filter(x => x.from === 'mail')) {
      const label = Widgets.words(Isle.lang, w, detailed).label
      out.push({ key: `widget:${w.id}`, kind: 'mail', cat: 'mail', w, name: label ? `${T('card.mail')} · ${label}` : T('card.mail'), note: T('picker.d.mail') })
    }
    for (const w of all.filter(x => x.from === 'script' && !x.plugin)) out.push({ key: `widget:${w.id}`, kind: 'script', cat: 'script', w, name: Widgets.words(Isle.lang, w, detailed).label || T('taskbar.w.widget'), note: T('picker.d.script') })
    // The layout's spacers: the strip alone has them.
    if (home() === 'taskbar') for (const type of ['flex', 'gap', 'sep']) out.push({ key: `spacer:${type}`, kind: 'layout', cat: 'system', spacer: type, name: T(`taskbar.w.${type}`), note: T(`taskbar.lib.note.${type}`) })
    return out
  }

  // The icon in its colour's tint (a spacer's: the strip's grey).
  function markOf(card) {
    if (card.spacer) return `<svg class="wicon" viewBox="0 0 24 24" aria-hidden="true">${SPACER_MARK[card.spacer]}</svg>`
    const w = card.w || { id: card.key.split(':')[1], icon: card.plugin?.icon, label: card.name }
    return Widgets.iconHTML(w, Isle.lang)
  }
  const colorOf = card => {
    const c = card.w?.color
    if (!c) return light() ? '#5d5d5d' : '#a1a1a6'
    return light() ? Widgets.deepen(c) : c
  }

  // Where each is put now: the strip, the desktop, her capsule (by key).
  function placesOf() {
    const strip = home() === 'taskbar' ? Isle.tbLayout() : null
    const bar = P.placedKeys({ taskbar: strip, home: home() })
    const desk = P.placedKeys({ desk: Isle.config.deskLayout, home: 'island' })
    const cap = new Set(Isle.members.map(m => m.key))
    const spacers = {}
    for (const run of P.RUNS) for (const p of strip?.[run] || []) if (p && ['flex', 'gap', 'sep'].includes(p.type)) spacers[p.type] = (spacers[p.type] || 0) + 1
    return { bar, desk, cap, spacers }
  }
  const countOf = (card, at) => (card.spacer ? at.spacers[card.spacer] || 0 : (at.bar.has(card.key) ? 1 : 0) + (at.desk.has(card.key) ? 1 : 0) + (at.cap.has(card.key) ? 1 : 0))

  function drawChips() {
    chips.replaceChildren(
      ...CATS.map(c => {
        const b = el('button', 'pk-chip', T(`picker.cat.${c}`))
        b.type = 'button'
        b.dataset.cat = c
        b.setAttribute('aria-pressed', String(c === cat))
        return b
      }),
    )
  }

  function drawGrid() {
    cards = cardsOf()
    const q = query.trim().toLowerCase()
    const shown = cards.filter(c => (cat === 'all' || c.cat === cat) && (!q || `${c.name} ${c.note} ${c.key} ${t('en', c.plugin ? `w.ex.${c.plugin.id}` : 'none')}`.toLowerCase().includes(q)))
    if (!shown.length) {
      grid.replaceChildren(el('p', 'pk-empty', q ? T('picker.none', { q: query.trim() }) : T('picker.noneHere')))
      return
    }
    const at = placesOf()
    grid.replaceChildren(
      ...shown.map(c => {
        const b = el('button', 'pk-card')
        b.type = 'button'
        b.dataset.key = c.key
        b.style.setProperty('--c', colorOf(c))
        b.innerHTML = `<span class="pk-wi">${markOf(c)}</span><span class="pk-n">${esc(c.name)}</span><span class="pk-d">${esc(c.note)}</span><span class="pk-bd" hidden></span>`
        b.title = c.note
        return b
      }),
    )
    drawBadges(at)
  }
  function drawBadges(at = placesOf()) {
    for (const b of grid.querySelectorAll('.pk-card')) {
      const c = cards.find(x => x.key === b.dataset.key)
      const n = c ? countOf(c, at) : 0
      const bd = b.querySelector('.pk-bd')
      bd.textContent = n ? T('picker.placed', { n }) : ''
      bd.hidden = !n
    }
  }

  function drawPicker() {
    theme()
    pk.setAttribute('aria-label', T('picker.title'))
    $('h2').textContent = T('picker.title')
    $('[data-pk="menu"]').title = T('picker.moreMenu')
    $('[data-pk="menu"]').hidden = !(home() === 'taskbar' && editing())
    $('[data-pk="close"]').title = T('picker.close')
    $('[data-pk="reset"]').textContent = T('picker.reset')
    $('[data-pk="more"]').textContent = T('picker.getMore')
    $('[data-pk="manage"]').textContent = T('picker.manage')
    search.placeholder = T('picker.search')
    search.setAttribute('aria-label', T('picker.search'))
    $('.pk-hint').textContent = T(home() === 'taskbar' ? 'picker.hint' : 'picker.hintIsland')
    drawChips()
    drawGrid()
  }

  // --- Open, closed ----------------------------------------------------------------------
  function show() {
    if (open) return
    open = true
    pk.hidden = false
    drawPicker()
    // In from the side: laid out first, then slid (style picker.css).
    void pk.offsetWidth
    pk.classList.add('pk-open')
    window.pet.settings.get().then(got => {
      snap = got
      if (open) drawGrid()
    })
    // The keyboard for its search (editing has it already).
    if (home() === 'island') window.pet.keyboard(true)
    if (lastPointer !== 'touch' && lastPointer !== 'pen') setTimeout(() => open && search.focus({ preventScroll: true }), 40)
    after()
  }
  function hide() {
    if (!open) return
    open = false
    pk.classList.remove('pk-open')
    menu.hidden = true
    note.hidden = true
    query = ''
    search.value = ''
    search.blur()
    // Out by the side, then gone (or at once, motion reduced).
    setTimeout(() => !open && (pk.hidden = true), reduced() ? 0 : 220)
    if (home() === 'island' && !window.CapsulePanel?.isOpen()) window.pet.keyboard(false)
    after()
  }
  // What it changed: the edit bar's 添加小工具, the window's room and where
  // it takes the mouse.
  function after() {
    if (editing()) Isle.drawEditBar()
    if (home() === 'island') Isle.update()
    else Isle.sendSolid()
    requestAnimationFrame(() => Isle.sendSolid())
  }

  pk.addEventListener('click', e => {
    const b = e.target.closest('[data-pk], .pk-chip')
    if (!b) return
    if (b.classList.contains('pk-chip')) {
      cat = b.dataset.cat
      drawChips()
      return drawGrid()
    }
    const what = b.dataset.pk
    if (what !== 'menu') menu.hidden = true
    if (what === 'close') return hide()
    if (what === 'menu') {
      menu.hidden = !menu.hidden
      return Isle.sendSolid()
    }
    if (what === 'reset') return resetParts()
    if (what === 'more') return say(T('picker.moreNote'))
    if (what === 'manage') {
      // The settings over her island: editing lets the pointer through it, so done first.
      if (editing()) Isle.endEdit(true)
      hide()
      return window.pet.openSettings('widgets')
    }
  })
  const say = text => {
    const again = note.textContent === text && !note.hidden
    note.textContent = text
    note.hidden = again
  }
  search.addEventListener('input', () => {
    query = search.value
    drawGrid()
  })
  // A card by the keyboard (Enter, Space): added as a press adds it.
  grid.addEventListener('click', e => {
    const b = e.target.closest('.pk-card')
    if (!b || e.detail !== 0) return
    const c = cards.find(x => x.key === b.dataset.key)
    if (c) pressAdd(c, b)
  })

  // --- Placing ---------------------------------------------------------------------------
  // The settings changed, here first (a second change before the Rust side
  // answers builds on it), then there.
  function setConfig(patch) {
    if (!patch) return
    Object.assign(Isle.config, clone(patch))
    window.pet.settings.set(patch)
  }
  // On or off: the setting (placing.js switchPatch), and the widgets as
  // this page has them, at once (the Rust side's word on them comes after:
  // her capsule is those on, and would show one just taken off meanwhile).
  function setSwitch(keys, on) {
    const list = keys.filter(k => !k.startsWith('spacer:'))
    if (!list.length) return
    for (const w of [...Isle.allWidgets, ...(snap?.widgets || [])]) if (list.includes(C.keyOfWidget(w)) || list.includes(`widget:${w.id}`)) w.on = on
    setConfig(P.switchesPatch(Isle.config, list, on))
  }
  const switchOn = keys => setSwitch(keys, true)
  // Off, those now nowhere: not on the strip nor on the desktop (her
  // capsule is those on that are nowhere else, so taken from there, or from
  // their last place elsewhere, they are off).
  function offIfNowhere(keys) {
    const strip = home() === 'taskbar' ? Isle.tbLayout() : null
    const placed = P.placedKeys({ taskbar: strip, desk: Isle.config.deskLayout, home: home() })
    setSwitch(keys.filter(k => !placed.has(k)), false)
  }

  // The member a key is (what the desktop's layout keeps).
  function memberOf(key, card = null) {
    const item = P.itemOf(key)
    const w = card?.w || (item.plugin ? allNow().find(x => x.plugin === item.plugin) : allNow().find(x => x.id === item.id))
    const icon = (w?.icon || card?.plugin?.icon) in Widgets.ICONS && (w?.icon || card?.plugin?.icon) !== 'dot' ? { icon: w?.icon || card.plugin.icon } : {}
    const name = !item.plugin && w && !w.private ? { name: clip(Widgets.words(Isle.lang, w).label) } : {}
    return { type: item.plugin ? 'plugin' : 'widget', of: item.plugin || item.id, ...icon, ...name }
  }
  const widgetOfKey = key => {
    const item = P.itemOf(key)
    return item.plugin ? allNow().find(x => x.plugin === item.plugin) : allNow().find(x => x.id === item.id)
  }
  // The entry a card puts on the strip: medium where it has it; the monitor
  // its part (#53); a spacer itself.
  function entryFor(card) {
    if (card.spacer) return { id: Isle.newId(), type: card.spacer }
    if (card.key === 'widget:monitor') return 'monitor'
    const m = memberOf(card.key, card)
    const w = card.w || widgetOfKey(card.key)
    return { id: Isle.newId(), ...m, size: !w || Widgets.sizesOf(w).includes('m') ? 'M' : 'S' }
  }
  // A widget's or plugin's name, as its card says it (a plugin's its own,
  // whatever its widgets are called now).
  const nameOfKey = key => {
    if (key.startsWith('plugin:')) return T(`w.ex.${key.slice(7)}`)
    const c = cards.find(x => x.key === key) || cardsOf().find(x => x.key === key)
    if (c) return c.name
    const w = widgetOfKey(key)
    return w ? Widgets.words(Isle.lang, w, Isle.isDetailed()).label || T('taskbar.w.widget') : T('taskbar.w.widget')
  }

  // A widget or plugin (by key) off the strip: alone, or out of its group
  // (one left: alone); the monitor's part too.
  function offStrip(layout, key) {
    let out = clone(layout)
    for (const run of P.RUNS)
      for (const p of [...out[run]]) {
        if (p === 'monitor' && key === 'widget:monitor') out[run] = out[run].filter(x => x !== p)
        if (!p || typeof p !== 'object') continue
        if (Widgets.isGroup(p)) {
          const at = p.items.findIndex(m => Widgets.memberKey(m) === key)
          if (at >= 0) out = Widgets.dropMember(out, p.id, at, Isle.newId)
        } else if (Widgets.isSource(p) && Widgets.memberKey(p) === key) out[run] = out[run].filter(x => x !== p)
      }
    return out
  }
  // Its standalone entry on the strip (a widget alone, or the monitor's part).
  function stripEntryOf(key) {
    if (home() !== 'taskbar') return null
    const layout = Isle.tbLayout()
    for (const run of P.RUNS)
      for (const p of layout[run]) {
        if (p === 'monitor' && key === 'widget:monitor') return 'monitor'
        if (p && typeof p === 'object' && Widgets.isSource(p) && Widgets.memberKey(p) === key) return p.id
      }
    return null
  }
  const onDesk = key => G.has(Isle.config.deskLayout, P.itemOf(key))

  // Her capsule's members as they are now, and those turned on and kept in
  // its setting that have nothing to say yet (a plugin starting): where they stood.
  const isOnKey = key => {
    const it = P.itemOf(key)
    return it.plugin ? Isle.config.plugins?.[it.plugin]?.on === true : !(Isle.config.widgetsOff || []).includes(it.id)
  }
  function listNow() {
    let out = Isle.members.map(m => ({ key: m.key, mode: m.mode }))
    const have = new Set(out.map(m => m.key))
    const strip = home() === 'taskbar' ? Isle.tbLayout() : null
    const placed = P.placedKeys({ taskbar: strip, desk: Isle.config.deskLayout, home: home() })
    ;(C.storedOf(Isle.config.capsule) || []).forEach((m, i) => {
      if (!have.has(m.key) && isOnKey(m.key) && !placed.has(m.key)) out = C.add(out, m.key, Math.min(i, out.length), m.mode)
    })
    return out
  }
  function writeCapsule(list) {
    setConfig(C.patchFor(Isle.config, list, home()))
    refresh()
  }
  // Everything that shows where widgets are, anew.
  function refresh() {
    Isle.takeWidgets(Isle.allWidgets)
    Isle.update()
    if (open) drawBadges()
    window.CapsulePanel?.refresh()
  }

  // Into the strip at a place (moved there from elsewhere on it, or new);
  // its medium ones made small first when it was full.
  function putOnStrip(key, at, shrink = [], fromStrip = null, card = null) {
    let layout = Isle.tbLayout()
    let entry
    if (fromStrip) {
      entry = P.removalOf(layout, fromStrip)?.entry
      layout = P.without(layout, fromStrip)
    } else {
      entry = entryFor(card || { key })
      if (!card?.spacer) layout = offStrip(layout, key)
    }
    if (!entry) return
    if (shrink.length) layout = P.withSizes(layout, shrink, 'S')
    if (shrink.includes('__new') && typeof entry === 'object') entry.size = 'S'
    layout = P.insertAt(layout, at.run, at.index, entry)
    Isle.keepLayout(layout)
    if (!fromStrip && !card?.spacer) {
      if (onDesk(key)) Isle.deskOff([P.itemOf(key)])
      Isle.placedAt.set(key, Date.now())
      switchOn([key])
      Isle.askPlugins(true)
    }
    if (shrink.length) note1(T('picker.shrunk'))
    flash(typeof entry === 'string' ? Isle.PART_NODES[entry] : Isle.entryNodes.get(entry.id))
    refresh()
  }

  // Into her capsule (at its end, or a place): off the strip and the desktop first (one place).
  function putInCapsule(keys, at = -1) {
    let layout = home() === 'taskbar' ? Isle.tbLayout() : null
    for (const key of keys) if (layout) layout = offStrip(layout, key)
    if (layout && JSON.stringify(layout) !== JSON.stringify(Isle.tbLayout())) Isle.keepLayout(layout)
    const desk = keys.filter(onDesk)
    if (desk.length) Isle.deskOff(desk.map(P.itemOf))
    let list = listNow()
    for (const [i, key] of keys.entries()) list = C.add(list.filter(m => m.key !== key), key, at >= 0 ? at + i : -1)
    for (const key of keys) Isle.placedAt.set(key, Date.now())
    switchOn(keys)
    writeCapsule(list)
    Isle.askPlugins(true)
    flash(home() === 'taskbar' ? Isle.herSlot : Isle.island)
  }

  // A press on a card: just before the tray (the island's home: into the
  // capsule). One on the strip already: shown where it is.
  function pressAdd(card, node) {
    if (home() !== 'taskbar') {
      if (card.spacer) return refuse(node, T('picker.onlyStrip'))
      if (Isle.members.some(m => m.key === card.key)) {
        flash(Isle.island)
        return note1(T('picker.inCapsule'))
      }
      return putInCapsule([card.key])
    }
    const there = !card.spacer && stripEntryOf(card.key)
    if (there) {
      flash(there === 'monitor' ? Isle.PART_NODES.monitor : Isle.entryNodes.get(there))
      return note1(T('picker.already'))
    }
    const at = P.beforeTray(Isle.tbLayout())
    const fit = fitFor(card.spacer ? GAP_W[card.spacer] : Isle.isSide() ? GAP_W.side : GAP_W[sizeOfCard(card)])
    if (!fit.ok) return refuse(node, T('picker.full'))
    putOnStrip(card.key, at, fit.shrink, null, card)
  }
  const sizeOfCard = card => (card.key === 'widget:monitor' ? 'part' : card.w && !Widgets.sizesOf(card.w).includes('m') ? 'S' : 'M')

  // Room on the strip for one this wide more (placing.js fitStrip): what
  // its runs take now, against its length less the show-desktop sliver.
  function fitFor(width, except = null) {
    const L = r => Isle.len(r)
    const strip = Isle.barRest.getBoundingClientRect()
    const edge = Isle.barRest.querySelector('.tdesk')
    // The middle run stays where it is centred (island.js alignApps keeps
    // it there, never moving it back): what lies before it is used too.
    const mid = getComputedStyle(Isle.zones.center)
    // A side bar's: its programs scroll when crowded (two buttons' room kept), its middle run moves up.
    const side = Isle.isSide()
    let used = 12 + (edge ? L(edge.getBoundingClientRect()) : 0) + (side ? 0 : parseFloat(mid.marginLeft) || 0)
    const shrinkable = []
    for (const z of Object.values(Isle.zones)) {
      used += 8
      for (const n of z.children) {
        if (n === gapNode || n === except || n.classList.contains('pk-lifted')) continue
        used += side && n === Isle.PART_NODES.apps ? Math.min(L(n.getBoundingClientRect()), 88) : L(n.getBoundingClientRect())
        const e = n._entry
        if (e && Widgets.isSource(e) && n.dataset.size === 'M' && !n.classList.contains('S')) shrinkable.push({ id: e.id, width: L(n.getBoundingClientRect()) })
      }
    }
    if (width > GAP_W.S) shrinkable.push({ id: '__new', width })
    return P.fitStrip({ room: L(strip), used, adding: width, shrinkable, small: GAP_W.S })
  }

  // --- Taken off, put back ---------------------------------------------------------------
  // A widget taken off: where it was, how it comes back, in a toast.
  // what: { entry, member } (the strip: an entry by its id, one of a
  // group's by its index), { widget } (one listed in the open island:
  // wherever it is), { key } (her capsule's). True when it was done here.
  function remove(what) {
    if (what?.entry) return removeFromStrip(what.entry, what.member ?? -1)
    if (what?.key) return removeFromCapsule(what.key)
    const w = what?.widget
    if (!w) return false
    const key = C.keyOfWidget(w)
    if (Isle.members.some(m => m.key === key)) return removeFromCapsule(key)
    const where = Isle.where.get(w.id)
    if (where === 'bar') {
      const id = stripEntryOf(key)
      if (id) return removeFromStrip(id)
      const layout = Isle.tbLayout()
      for (const run of P.RUNS)
        for (const p of layout[run])
          if (p && Widgets.isGroup(p)) {
            const at = p.items.findIndex(m => Widgets.memberKey(m) === key)
            if (at >= 0) return removeFromStrip(p.id, at)
          }
      return false
    }
    if (where === 'desk') return removeFromDesk(key)
    return false
  }

  function removeFromStrip(id, member = -1) {
    const before = Isle.tbLayout()
    const rec = P.removalOf(before, id)
    if (!rec) return false
    const e = rec.entry
    if (typeof e === 'string' && !Isle.TB_OPTIONAL.includes(e)) return false
    const group = typeof e === 'object' && Widgets.isGroup(e)
    const gone = typeof e === 'string' ? ['widget:monitor'] : group && member >= 0 ? [Widgets.memberKey(e.items[member])] : Widgets.membersOf(e).map(Widgets.memberKey)
    const after = group && member >= 0 ? Widgets.dropMember(before, id, member, Isle.newId) : P.without(before, id)
    closePop()
    Isle.keepLayout(after)
    offIfNowhere(gone)
    refresh()
    const name = group && member < 0 ? Widgets.nameOf(Isle.lang, e, Isle.allWidgets, Isle.isDetailed()) : nameOfKey(gone[0])
    undoToast(name, () => {
      const now = Isle.tbLayout()
      // Nothing else changed since: as it was; else back where it stood.
      let back = JSON.stringify(now) === JSON.stringify(after) ? before : P.restoreEntry(now, group && member >= 0 ? { entry: { id: Isle.newId(), ...e.items[member], size: 'M' }, run: rec.run, index: rec.index + 1 } : rec)
      if (back === now) return
      for (const key of gone) if (onDesk(key)) Isle.deskOff([P.itemOf(key)])
      Isle.keepLayout(back)
      switchOn(gone)
      refresh()
      flash(typeof e === 'string' ? Isle.PART_NODES[e] : Isle.entryNodes.get(e.id))
    })
    return true
  }

  function removeFromCapsule(key) {
    const { list, record } = C.remove(listNow(), key)
    if (!record) return false
    offIfNowhere([key])
    writeCapsule(list)
    undoToast(nameOfKey(key), () => {
      if (stripEntryOf(key) || onDesk(key)) return
      switchOn([key])
      writeCapsule(C.restore(listNow(), record))
      flash(home() === 'taskbar' ? Isle.herSlot : Isle.island)
    })
    return true
  }

  function removeFromDesk(key) {
    const before = Array.isArray(Isle.config.deskLayout) ? clone(Isle.config.deskLayout) : []
    const holder = before.find(e => G.membersOf(e).some(m => `${m.type}:${m.of}` === key))
    if (!holder) return false
    Isle.deskOff([P.itemOf(key)])
    offIfNowhere([key])
    refresh()
    undoToast(nameOfKey(key), () => {
      if (stripEntryOf(key) || listNow().some(m => m.key === key)) return
      const keys = new Set(G.membersOf(holder).map(m => `${m.type}:${m.of}`))
      const now = (Array.isArray(Isle.config.deskLayout) ? Isle.config.deskLayout : []).filter(e => e.id !== holder.id && !G.membersOf(e).some(m => keys.has(`${m.type}:${m.of}`)))
      Isle.keepDeskLayout([...now, holder])
      switchOn([key])
      refresh()
    })
    return true
  }

  // --- Toasts ----------------------------------------------------------------------------
  function toast(text, undo = null) {
    theme()
    const node = el('div', undo ? 'pk-toast' : 'pk-toast pk-info')
    node.append(el('span', '', text))
    let timer = 0
    const gone = () => {
      clearTimeout(timer)
      node.remove()
      toasts.hidden = !toasts.children.length
      after()
    }
    if (undo) {
      node.append(el('span', 'pk-dot', '·'))
      const b = el('button', 'pk-undo', T('picker.undo'))
      b.type = 'button'
      b.addEventListener('click', () => {
        gone()
        undo()
      })
      node.append(b)
    }
    // The same word again: the old one goes.
    if (!undo) for (const x of [...toasts.children]) if (x.classList.contains('pk-info') && x.firstChild.textContent === text) x.remove()
    toasts.append(node)
    while (toasts.children.length > 3) toasts.firstChild.remove()
    toasts.hidden = false
    timer = setTimeout(gone, undo ? UNDO_MS : NOTE_MS)
    after()
  }
  const undoToast = (name, undo) => toast(T('picker.removed', { name }), undo)
  const note1 = text => toast(text)

  function flash(node) {
    if (!node?.isConnected) return
    node.classList.remove('pk-flash')
    void node.offsetWidth
    node.classList.add('pk-flash')
    setTimeout(() => node.classList.remove('pk-flash'), 1300)
  }
  function shake(node) {
    if (!node?.isConnected) return
    node.classList.remove('pk-shake')
    void node.offsetWidth
    node.classList.add('pk-shake')
    setTimeout(() => node.classList.remove('pk-shake'), 420)
  }
  function refuse(node, text) {
    if (home() === 'taskbar') shake(Isle.barRest)
    shake(node)
    note1(text)
  }

  // The layout's parts as Windows 11 has them, the widgets kept (⋯ 恢复默认), with a way back.
  function resetParts() {
    menu.hidden = true
    if (!editing()) return
    const before = Isle.tbLayout()
    Isle.resetParts()
    refresh()
    toast(T('picker.resetDone'), () => editing() && Isle.keepLayout(before))
  }

  // --- The popover over a widget on the strip (editing) ------------------------------------
  let popFor = null
  let popSticky = false
  let popTimer = 0
  let popHide = 0
  const popEntryOf = node => node?._entry || (node?.dataset.part === 'monitor' ? 'monitor' : null)
  function openPop(node, sticky) {
    const e = popEntryOf(node)
    if (!e || !editing()) return
    clearTimeout(popTimer)
    clearTimeout(popHide)
    popFor = node
    popSticky = sticky
    theme()
    const part = e === 'monitor'
    const group = !part && Widgets.isGroup(e)
    const w = part ? allNow().find(x => x.id === 'monitor') : !group && Widgets.sourcesOf(e, Isle.allWidgets)[0]
    const name = part ? T('card.monitor') : node.dataset.label || Widgets.nameOf(Isle.lang, e, Isle.allWidgets, Isle.isDetailed())
    const seg = (act, v, label, on, why = '') => `<button type="button" class="pk-seg${on ? ' on' : ''}" data-pop="${act}" data-v="${v}"${why ? ` aria-disabled="true" title="${esc(why)}"` : ''}>${esc(label)}</button>`
    const rows = []
    if (!part && Widgets.isSource(e)) {
      const noM = w && !Widgets.sizesOf(w).includes('m')
      const now = node.dataset.size === 'S' || noM ? 'S' : 'M'
      rows.push(`<div class="pk-row"><span>${esc(T('menu.size'))}</span><span class="pk-segs">${seg('size', 'S', T('menu.s'), now === 'S')}${seg('size', 'M', T('menu.m'), now === 'M', noM ? T('menu.noMedium') : '')}</span></div>`)
    }
    if (group) rows.push(`<div class="pk-row"><span>${esc(T('menu.group'))}</span><span class="pk-segs">${seg('group', 'carousel', T('menu.carousel'), e.type === 'carousel')}${seg('group', 'stack', T('menu.stack'), e.type === 'stack')}${seg('group', 'apart', T('menu.ungroup'), false)}</span></div>`)
    const settable = part || (!group && (e.type === 'plugin' || w?.builtIn || w?.from === 'mail'))
    pop.innerHTML = `<div class="pk-pt"><span class="pk-wi" style="--c:${w ? colorOf({ w }) : 'currentColor'}">${w ? Widgets.iconHTML(w, Isle.lang) : group ? FI('E8FD') : ''}</span><b>${esc(name)}</b></div>${rows.join('')}<div class="pk-pb">${settable ? `<button type="button" data-pop="settings">${FI('E713')}${esc(T('taskbar.w.settings'))}</button>` : ''}<button type="button" data-pop="remove">${FI('E74D')}${esc(T('taskbar.w.remove'))}</button></div>`
    pop.hidden = false
    placePop()
    Isle.watchPresses()
    after()
  }
  // Over its widget (beside a side bar), kept on the screen.
  function placePop() {
    if (!popFor?.isConnected) return closePop()
    const r = popFor.getBoundingClientRect()
    const w = pop.offsetWidth
    const h = pop.offsetHeight
    const edge = Isle.tbEdge()
    let x = r.left + r.width / 2 - w / 2
    let y = r.top - h - 10
    if (edge === 'top') y = r.bottom + 10
    if (edge === 'left') [x, y] = [r.right + 10, r.top + r.height / 2 - h / 2]
    if (edge === 'right') [x, y] = [r.left - w - 10, r.top + r.height / 2 - h / 2]
    pop.style.left = `${Math.round(Math.max(8, Math.min(x, innerWidth - w - 8)))}px`
    pop.style.top = `${Math.round(Math.max(8, Math.min(y, innerHeight - h - 8)))}px`
  }
  function closePop() {
    clearTimeout(popTimer)
    clearTimeout(popHide)
    if (!popFor && pop.hidden) return
    popFor = null
    popSticky = false
    pop.hidden = true
    Isle.watchPresses()
    after()
  }
  const isWidgetNode = n => !!n && (!!n._entry && (Widgets.isSource(n._entry) || Widgets.isGroup(n._entry)) || n.dataset.part === 'monitor')
  Isle.barRest.addEventListener('pointerover', e => {
    if (!editing() || drag || press || e.pointerType !== 'mouse') return
    const node = e.target.closest('[data-part]')
    if (node && node === popFor) return clearTimeout(popHide)
    clearTimeout(popTimer)
    if (isWidgetNode(node)) popTimer = setTimeout(() => !drag && openPop(node, false), 260)
    if (popFor && !popSticky) {
      clearTimeout(popHide)
      popHide = setTimeout(closePop, 260)
    }
  })
  Isle.barRest.addEventListener('pointerleave', () => {
    clearTimeout(popTimer)
    if (popFor && !popSticky) popHide = setTimeout(closePop, 260)
  })
  pop.addEventListener('pointerenter', () => clearTimeout(popHide))
  pop.addEventListener('pointerleave', () => {
    if (popFor && !popSticky) popHide = setTimeout(closePop, 260)
  })
  pop.addEventListener('click', e => {
    const b = e.target.closest('[data-pop]')
    const node = popFor
    const entry = popEntryOf(node)
    if (!b || !entry || b.getAttribute('aria-disabled') === 'true') return
    const what = b.dataset.pop
    const v = b.dataset.v
    if (what === 'remove') return removeFromStrip(entry === 'monitor' ? 'monitor' : entry.id)
    if (what === 'settings') {
      const w = entry === 'monitor' ? allNow().find(x => x.id === 'monitor') : Widgets.sourcesOf(entry, Isle.allWidgets)[0]
      closePop()
      Isle.endEdit(true)
      return Isle.openWidgetSettings(w || (entry.type === 'plugin' ? { plugin: entry.of } : { id: entry.of }))
    }
    let layout = Isle.tbLayout()
    if (what === 'size') {
      const at = Widgets.findEntry(layout, entry.id)
      if (at) layout[at.run][at.at] = { ...at.entry, size: v }
    } else if (what === 'group') layout = v === 'apart' ? Widgets.ungroup(layout, entry.id, Isle.newId) : Widgets.regroup(layout, entry.id, v)
    Isle.keepLayout(layout)
    if (what === 'group' && v === 'apart') return closePop()
    const fresh = Isle.entryNodes.get(entry.id)
    if (fresh) openPop(fresh, true)
  })

  // --- Dragging ------------------------------------------------------------------------------
  // A press on a card, or (editing) on a part or a widget of the strip
  // (island.js gives it here): a drag once it has moved 5 px; let go where
  // it was, a press (a card added, a widget's popover, her capsule's panel).
  grid.addEventListener('pointerdown', e => {
    lastPointer = e.pointerType
    const b = e.target.closest('.pk-card')
    if (!b || e.button !== 0 || drag) return
    e.preventDefault()
    menu.hidden = true
    press = { src: 'card', key: b.dataset.key, node: b, x: e.clientX, y: e.clientY, id: e.pointerId }
    try {
      grid.setPointerCapture(e.pointerId)
    } catch {}
  })
  function pressStrip(e, node) {
    lastPointer = e.pointerType
    if (drag) return
    if (!node) return closePop()
    press = { src: 'strip', key: node.dataset.part, node, x: e.clientX, y: e.clientY, id: e.pointerId }
    try {
      Isle.barRest.setPointerCapture(e.pointerId)
    } catch {}
  }

  // What is dragged: its key (a widget's or plugin's; a part's name), its
  // entry on the strip (by id, or a part), the member the desktop keeps,
  // what it shows and says, how wide a gap it opens.
  function begin(p) {
    const d = { src: p.src, pid: p.id, at: null, tgt: { kind: 'none' }, sig: '', hot: null, sent: null }
    if (p.src === 'card') {
      const card = cards.find(c => c.key === p.key)
      if (!card) return
      d.card = card
      d.key = card.key
      d.name = card.name
      d.mark = markOf(card)
      d.color = colorOf(card)
      // On the strip already, alone: it is that one moved.
      const there = !card.spacer && stripEntryOf(card.key)
      if (there) {
        d.strip = there
        d.node = there === 'monitor' ? Isle.PART_NODES.monitor : Isle.entryNodes.get(there)
      }
      d.width = card.spacer ? GAP_W[card.spacer] : d.node ? Isle.len(d.node.getBoundingClientRect()) : Isle.isSide() ? GAP_W.side : GAP_W[sizeOfCard(card)]
    } else {
      const layout = Isle.tbLayout()
      const rec = P.removalOf(layout, p.key)
      if (!rec) return
      const e = rec.entry
      d.strip = p.key
      d.node = p.node
      d.entry = e
      d.part = typeof e === 'string' ? e : null
      d.key = d.part ? (d.part === 'monitor' ? 'widget:monitor' : null) : Widgets.isSource(e) ? Widgets.memberKey(e) : null
      d.keys = d.part === 'monitor' ? ['widget:monitor'] : typeof e === 'object' ? Widgets.membersOf(e).map(Widgets.memberKey) : []
      d.name = p.node.dataset.label || (d.part ? T(`taskbar.part.${d.part}`) : Widgets.nameOf(Isle.lang, e, Isle.allWidgets, Isle.isDetailed()))
      const w = d.key && widgetOfKey(d.key)
      d.mark = w ? Widgets.iconHTML(w, Isle.lang) : FI(d.part ? 'E700' : 'E8FD')
      d.color = w ? colorOf({ w }) : 'currentColor'
      d.width = Isle.len(p.node.getBoundingClientRect())
    }
    // What may go where: a part that stays (Start, the buttons, the tray,
    // the clock, her), on the strip alone; a spacer too; the rest anywhere.
    d.stays = !!d.part && !Isle.TB_OPTIONAL.includes(d.part)
    d.anywhere = !d.stays && !d.card?.spacer && !(d.entry && typeof d.entry === 'object' && !Widgets.isSource(d.entry) && !Widgets.isGroup(d.entry))
    d.desk = d.anywhere && !!d.key
    d.capsuleKeys = d.anywhere ? (d.keys?.length ? d.keys : d.key ? [d.key] : []) : []
    if (d.desk) {
      const w = widgetOfKey(d.key)
      d.item = memberOf(d.key, d.card)
      d.size = !w || Widgets.sizesOf(w).includes('m') ? 'm' : 's'
    }
    drag = d
    closePop()
    menu.hidden = true
    theme()
    if (d.node) d.node.classList.add('pk-lifted')
    gapNode.style.cssText = ''
    ghost.querySelector('.pk-gt').innerHTML = `<span class="pk-wi" style="--c:${d.color}">${d.mark}</span>`
    ghost.hidden = false
    ghost.style.transition = 'none'
    ghost.style.opacity = ''
    document.body.classList.add('pk-dragging')
    pk.classList.add('pk-thru')
    if (d.src === 'strip' && !d.stays && home() === 'taskbar') {
      band.textContent = T('picker.band')
      band.hidden = false
      placeBand()
    }
    after()
  }

  // Where the pointer is, as a place to let go.
  function hit(x, y) {
    const d = drag
    const pr = open ? rectOf(pk) : null
    if (pr && inRect(x, y, pr)) return { kind: 'picker' }
    const panel = window.CapsulePanel?.isOpen() ? rectOf(window.CapsulePanel.el) : null
    if (panel && inRect(x, y, panel)) return { kind: 'capsule', at: window.CapsulePanel.indexAt(y) }
    if (home() !== 'taskbar') {
      const r = Isle.island.getBoundingClientRect()
      if (inRect(x, y, { left: r.left - 14, right: r.right + 14, top: r.top - 14, bottom: r.bottom + 14 })) return { kind: 'capsule' }
      return { kind: 'desk' }
    }
    const s = Isle.barRest.getBoundingClientRect()
    const edge = Isle.tbEdge()
    const off = { bottom: s.top - y, top: y - s.bottom, left: x - s.right, right: s.left - x }[edge]
    if (off <= 2) return stripHit(x, y)
    if (d.src === 'strip' && !d.stays && off <= BAND_PX) return { kind: 'bare' }
    const bar = rectOf(document.querySelector('.tbedit'))
    if (bar && inRect(x, y, bar)) return { kind: 'none' }
    for (const node of [toasts, pop]) if (!node.hidden && inRect(x, y, rectOf(node))) return { kind: 'none' }
    return { kind: 'desk' }
  }

  function stripHit(x, y) {
    const d = drag
    const a = Isle.along(x, y)
    // Her capsule: the middle half of her slot.
    const her = Isle.herSlot.getBoundingClientRect()
    if (d.capsuleKeys.length && a > Isle.lo(her) + Isle.len(her) * 0.25 && a < Isle.lo(her) + Isle.len(her) * 0.75) return { kind: 'capsule' }
    const runs = Object.entries(Isle.zones).map(([run, z]) => {
      const r = z.getBoundingClientRect()
      const items = [...z.children].filter(n => n !== gapNode && n !== d.node && !n.classList.contains('pk-lifted')).map(n => n.getBoundingClientRect())
      return { run, lo: Isle.lo(r), hi: Isle.hi(r), items: items.map(r2 => ({ lo: Isle.lo(r2), hi: Isle.hi(r2) })) }
    })
    const at = P.slotAt(runs, a)
    // New to the strip: room for it, or its medium ones made small, or none.
    d.fit ||= d.strip ? { ok: true, shrink: [] } : fitFor(d.width)
    return { kind: 'strip', at, fit: d.fit }
  }

  // Ghost's words, and whether it is a no (amber) or a taking-off.
  function labelOf(tg) {
    const d = drag
    const moving = d.src === 'strip' || !!d.strip || (d.key && (onDesk(d.key) || Isle.members.some(m => m.key === d.key)))
    switch (tg.kind) {
      case 'strip':
        if (!tg.fit.ok) return [T('picker.full'), 'no']
        return [d.src === 'strip' ? T('picker.here') : tg.fit.shrink.length ? T('picker.toStripSmall') : T(moving ? 'picker.moveStrip' : 'picker.toStrip'), '']
      case 'capsule':
        return d.capsuleKeys.length ? [T(moving ? 'picker.moveCapsule' : 'picker.toCapsule'), ''] : [T('picker.onlyStrip'), 'no']
      case 'picker':
        return d.src === 'strip' ? (d.stays ? [T('taskbar.edit.fixed'), 'no'] : [T('picker.dropRemove'), 'rm']) : [T('picker.backIn'), 'no']
      case 'bare':
        return [T('picker.dropRemove'), 'rm']
      case 'desk': {
        if (!d.desk) return [T(noDeskWhy(d)), 'no']
        const said = deskSaid
        if (said && !said.ok) return [T(said.why === 'boardFull' ? 'desk.boardFull' : 'picker.deskFull'), 'no']
        if (said?.where === 'board') return [T(moving ? 'picker.moveBoard' : 'picker.toBoard', { name: said.name || T('desk.board') }), '']
        return [T(moving ? 'picker.moveDesk' : 'picker.toDesk'), '']
      }
      default:
        return [T('picker.nowhere'), 'no']
    }
  }

  function moveTo(x, y) {
    const d = drag
    const tg = hit(x, y)
    d.tgt = tg
    const sig = tg.kind === 'strip' ? `strip:${tg.at.run}:${tg.at.index}:${tg.fit.ok}` : tg.kind
    if (sig !== d.sig) {
      d.sig = sig
      gapAt(tg.kind === 'strip' ? tg : null)
    }
    // Her capsule lit, the picker, the band.
    const capsuleHot = tg.kind === 'capsule'
    Isle.island.classList.toggle('pk-hot', capsuleHot)
    window.CapsulePanel?.hot(capsuleHot)
    pk.classList.toggle('pk-hot', tg.kind === 'picker')
    pk.dataset.drop = tg.kind === 'picker' ? labelOf(tg)[0] : ''
    band.classList.toggle('pk-hot', tg.kind === 'bare')
    // Over the desktop: its page shows where it lands (through the host).
    if (tg.kind === 'desk' && d.desk) {
      const last = d.sent
      if (!last || Math.hypot(x - last.x, y - last.y) > 6 || Date.now() - last.at > 120) {
        d.sent = { x, y, at: Date.now() }
        window.pet.deskPreview({ x: Math.round(x), y: Math.round(y), item: d.item, size: d.size })
      }
    } else if (d.sent) {
      d.sent = null
      deskSaid = null
      window.pet.deskPreview(null)
    }
    const [words, tone] = labelOf(tg)
    ghost.querySelector('.pk-gl').textContent = `${d.name} · ${words}`
    ghost.className = `pk-ghost${tone ? ` pk-${tone}` : ''}${light() ? ' pk-l' : ''}${nearStrip(x, y) ? ' pk-up' : ''}`
    ghost.style.left = `${x}px`
    ghost.style.top = `${y}px`
  }
  // Why not on the desktop: a part that stays, a spacer, several in one place.
  const noDeskWhy = d => (d.stays ? 'taskbar.edit.fixed' : d.entry && typeof d.entry === 'object' && Widgets.isGroup(d.entry) ? 'picker.noGroupDesk' : 'picker.onlyStrip')
  const nearStrip = (x, y) => home() === 'taskbar' && Isle.tbEdge() === 'bottom' && y > Isle.barRest.getBoundingClientRect().top - 70

  // The gap where it would go, the others moved aside (FLIP: from where they
  // were, 160 ms); none, the gap closed.
  function gapAt(tg) {
    const d = drag
    const nodes = Object.values(Isle.zones).flatMap(z => [...z.children])
    const was = new Map(nodes.map(n => [n, n.getBoundingClientRect()]))
    if (!tg) gapNode.remove()
    else {
      const z = Isle.zones[tg.at.run]
      const list = [...z.children].filter(n => n !== gapNode && n !== d.node && !n.classList.contains('pk-lifted'))
      const ref = list[tg.at.index] || null
      gapNode.classList.toggle('pk-no', !tg.fit.ok)
      gapNode.style[Isle.isSide() ? 'height' : 'width'] = `${Math.round(d.width)}px`
      if (ref) z.insertBefore(gapNode, ref)
      else z.append(gapNode)
    }
    Isle.alignApps()
    Isle.placeSlot()
    if (reduced()) return
    for (const n of nodes) {
      const a = was.get(n)
      if (!a || !n.isConnected || n === gapNode || n.classList.contains('pk-lifted')) continue
      const b = n.getBoundingClientRect()
      const dx = a.left - b.left
      const dy = a.top - b.top
      if (Math.abs(dx) < 0.5 && Math.abs(dy) < 0.5) continue
      n.animate([{ transform: `translate(${dx}px, ${dy}px)` }, { transform: 'none' }], { duration: 160, easing: 'cubic-bezier(.2,.8,.2,1)' })
    }
  }

  function placeBand() {
    const s = Isle.barRest.getBoundingClientRect()
    const edge = Isle.tbEdge()
    const r = { bottom: [s.left, s.top - BAND_PX, s.width, BAND_PX], top: [s.left, s.bottom, s.width, BAND_PX], left: [s.right, s.top, BAND_PX, s.height], right: [s.left - BAND_PX, s.top, BAND_PX, s.height] }[edge]
    Object.assign(band.style, { left: `${r[0]}px`, top: `${r[1]}px`, width: `${r[2]}px`, height: `${r[3]}px` })
    band.dataset.edge = edge
  }

  // Let go: where it goes (or back where it was).
  function end(ok, x, y) {
    const d = drag
    if (!d) return
    drag = null
    const tg = ok ? d.tgt : { kind: 'none' }
    const hadGap = gapNode.isConnected
    gapNode.remove()
    if (d.node) d.node.classList.remove('pk-lifted')
    if (hadGap || d.node) {
      Isle.alignApps()
      Isle.placeSlot()
    }
    document.body.classList.remove('pk-dragging')
    pk.classList.remove('pk-thru', 'pk-hot')
    band.hidden = true
    band.classList.remove('pk-hot')
    Isle.island.classList.remove('pk-hot')
    window.CapsulePanel?.hot(false)
    if (d.sent && tg.kind !== 'desk') window.pet.deskPreview(null)
    let landed = false
    switch (tg.kind) {
      case 'strip':
        if (!tg.fit.ok) {
          refuse(d.src === 'card' ? d.card && grid.querySelector(`[data-key="${CSS.escape(d.key)}"]`) : null, T('picker.full'))
          break
        }
        putOnStrip(d.key, tg.at, tg.fit.shrink, d.strip || null, d.card || null)
        landed = true
        break
      case 'capsule':
        if (!d.capsuleKeys.length) {
          note1(T('picker.onlyStrip'))
          break
        }
        putInCapsule(d.capsuleKeys, tg.at ?? -1)
        landed = true
        break
      case 'picker':
      case 'bare':
        if (d.src === 'strip' && !d.stays) {
          removeFromStrip(d.strip)
          landed = true
        } else if (d.stays && d.src === 'strip') note1(T('taskbar.edit.fixed'))
        break
      case 'desk':
        if (!d.desk) {
          note1(T(noDeskWhy(d)))
          break
        }
        dropOnDesk(d, x, y)
        landed = true
        break
    }
    deskSaid = null
    if (landed) ghost.hidden = true
    else flyBack(d)
    after()
  }

  // Back where it came from: to its card, or its place on the strip.
  function flyBack(d) {
    const to = d.src === 'card' ? grid.querySelector(`[data-key="${CSS.escape(d.key)}"]`) : d.node
    if (!to?.isConnected || reduced() || (d.src === 'card' && !open)) return (ghost.hidden = true)
    const r = to.getBoundingClientRect()
    ghost.style.transition = 'left .22s cubic-bezier(.2,.8,.2,1), top .22s cubic-bezier(.2,.8,.2,1), opacity .22s'
    ghost.style.left = `${r.left + r.width / 2}px`
    ghost.style.top = `${r.top + r.height / 2}px`
    ghost.style.opacity = '0'
    setTimeout(() => {
      if (!drag) ghost.hidden = true
      ghost.style.transition = 'none'
      ghost.style.opacity = ''
    }, 240)
  }

  // Over the desktop: its page puts it there (desk-drop.js), and says so;
  // from the strip, it is off it then (a move). No word back (a Rust side
  // that does not forward it yet): put on the desktop the island's own way,
  // where the desktop's page finds room (top right).
  function dropOnDesk(d, x, y) {
    const ticket = `t${++tickets}`
    const was = { key: d.key, strip: d.src === 'strip' ? d.strip : d.strip || null, item: d.item, keys: d.capsuleKeys }
    clearTimeout(pending?.timer)
    pending = { ticket, was, timer: setTimeout(() => pending?.ticket === ticket && deskFallback(), 700) }
    window.pet.deskDrop({ x: Math.round(x), y: Math.round(y), item: d.item, size: d.size, ticket })
  }
  function deskFallback() {
    const { was } = pending
    pending = null
    if (was.strip) Isle.keepLayout(P.without(Isle.tbLayout(), was.strip))
    Isle.placeWidget(P.itemOf(was.key), 'desk')
    switchOn([was.key])
    refresh()
  }
  window.pet.onDeskAnswer(a => {
    if (!a || typeof a !== 'object') return
    if (a.ticket == null) {
      // About the preview: the ghost's words.
      if (!drag || drag.tgt.kind !== 'desk') return
      deskSaid = a
      const [words, tone] = labelOf(drag.tgt)
      ghost.querySelector('.pk-gl').textContent = `${drag.name} · ${words}`
      ghost.classList.toggle('pk-no', tone === 'no')
      return
    }
    if (!pending || a.ticket !== pending.ticket) return
    clearTimeout(pending.timer)
    const { was } = pending
    pending = null
    if (!a.ok) return note1(T(a.why === 'boardFull' ? 'desk.boardFull' : 'picker.deskFull'))
    // There now (the desktop's page kept it): off the strip and the capsule (one place), on.
    if (was.strip) Isle.keepLayout(P.without(Isle.tbLayout(), was.strip))
    else if (home() === 'taskbar' && stripEntryOf(was.key) == null) {
      const off = offStrip(Isle.tbLayout(), was.key)
      if (JSON.stringify(off) !== JSON.stringify(Isle.tbLayout())) Isle.keepLayout(off)
    }
    Isle.placedAt.set(was.key, Date.now())
    switchOn([was.key])
    refresh()
  })

  window.addEventListener(
    'pointermove',
    e => {
      if (drag) {
        if (e.pointerId === drag.pid) moveTo(e.clientX, e.clientY)
        return
      }
      if (!press || e.pointerId !== press.id || Math.hypot(e.clientX - press.x, e.clientY - press.y) < 5) return
      const p = press
      press = null
      begin(p)
      if (drag) moveTo(e.clientX, e.clientY)
    },
    true,
  )
  window.addEventListener(
    'pointerup',
    e => {
      if (drag) {
        if (e.pointerId === drag.pid) end(true, e.clientX, e.clientY)
        return
      }
      const p = press
      press = null
      if (!p || e.pointerId !== p.id || e.button !== 0) return
      if (p.src === 'card') {
        const c = cards.find(x => x.key === p.key)
        if (c) pressAdd(c, p.node)
        return
      }
      // A press on her capsule: its panel; on a widget: its popover, kept.
      if (p.key === 'her') {
        closePop()
        return window.CapsulePanel?.toggle()
      }
      if (isWidgetNode(p.node)) return popFor === p.node && popSticky ? closePop() : openPop(p.node, true)
      closePop()
    },
    true,
  )
  window.addEventListener(
    'pointercancel',
    e => {
      press = null
      if (drag && e.pointerId === drag.pid) end(false)
    },
    true,
  )
  window.addEventListener('blur', () => {
    if (drag) end(false)
  })

  // --- Esc, presses elsewhere, her menu ------------------------------------------------------
  // Esc: a drag, the popover, the ⋯ menu, the search's words, the capsule's
  // panel, the picker, in that order; true when one was.
  function escape() {
    if (drag) {
      end(false)
      return true
    }
    if (!pop.hidden) {
      closePop()
      return true
    }
    if (!menu.hidden) {
      menu.hidden = true
      return true
    }
    if (open && search.value) {
      search.value = ''
      query = ''
      drawGrid()
      return true
    }
    if (window.CapsulePanel?.isOpen()) {
      window.CapsulePanel.close()
      return true
    }
    if (open) {
      hide()
      return true
    }
    return false
  }
  document.addEventListener('keydown', e => {
    if (e.key === 'Escape' && !editing() && escape()) e.preventDefault()
  })
  // A press elsewhere (the Rust side's word, or in the page): the popover
  // and the capsule's panel go.
  window.pet.onPress(at => {
    if (!at) return
    const over = document.elementFromPoint(at.x, at.y)
    // Its own (the picker, a toast, the edit bar, the strip being edited) are not elsewhere.
    const ours = over?.closest?.('.pk, .pk-toasts, .pk-pop, .cp, .tbedit, #bar-rest, #island')
    if (popFor && !(over && (pop.contains(over) || popFor.contains(over))) && !over?.closest?.('.pk-toasts')) closePop()
    if (window.CapsulePanel?.isOpen() && !ours) window.CapsulePanel.close()
  })
  document.addEventListener(
    'pointerdown',
    e => {
      if (!menu.hidden && !menu.contains(e.target) && !e.target.closest('[data-pk="menu"]')) menu.hidden = true
      if (popFor && !pop.contains(e.target) && !popFor.contains(e.target) && !Isle.barRest.contains(e.target)) closePop()
    },
    true,
  )
  // Her island's window lost the keyboard (a press on another window): in
  // the island's home, what it has open goes (editing the strip, it stays).
  window.pet.onBlur(() => {
    if (home() !== 'island' || drag) return
    hide()
    window.CapsulePanel?.close()
  })
  // Her menu in the island's home: 添加小工具… (the picker alone), 编辑胶囊;
  // on the taskbar, the picker comes with editing its layout.
  window.pet.onPicker(p => {
    const what = p?.open === 'capsule' ? 'capsule' : 'picker'
    if (what === 'capsule') {
      if (home() === 'island') hide()
      return window.CapsulePanel?.open()
    }
    if (home() === 'taskbar') return Isle.editTaskbar({ picker: true })
    window.CapsulePanel?.close()
    show()
  })
  // What it shows follows the settings and the widgets.
  window.pet.onUpdate(() => {
    if (open) {
      theme()
      drawBadges()
    }
  })
  window.pet.onSettingsChanged(got => {
    if (got && typeof got === 'object') snap = got
    if (open) drawGrid()
  })

  // --- For island.js -------------------------------------------------------------------------
  window.Picker = {
    open: show,
    close: hide,
    toggle: () => (open ? hide() : show()),
    isOpen: () => open,
    pressStrip,
    escape,
    remove,
    setMode(w, mode) {
      const key = C.keyOfWidget(w)
      const list = listNow()
      if (!list.some(m => m.key === key)) return
      if (mode === 'pin' && !C.canPin(list, key)) return note1(T('capsule.pinFull'))
      writeCapsule(C.setMode(list, key, mode))
    },
    // The open island's menu for one listed there: its way of showing in
    // her capsule and 移出胶囊 (a member), or off where it is.
    isleMenu(w, { opt, line }) {
      const key = C.keyOfWidget(w)
      const list = listNow()
      const m = list.find(x => x.key === key)
      if (!m) {
        const where = Isle.where.get(w.id)
        return where ? { lines: [], remove: T(where === 'desk' ? 'desk.remove' : 'menu.remove') } : null
      }
      const why = C.canPin(list, key) ? '' : T('capsule.pinFull')
      return {
        lines: [line(T('capsule.show'), opt('cmode', 'turn', T('capsule.turn'), m.mode === 'turn') + opt('cmode', 'pin', T('capsule.pin'), m.mode === 'pin', why) + opt('cmode', 'open', T('capsule.openOnlyLong'), m.mode === 'open'))],
        remove: T('capsule.out'),
      }
    },
    // Kept as it goes, editing: the layout being edited is the one kept.
    persist() {
      const layout = Isle.editLayout
      if (!layout) return
      const kept = Isle.tbLayout(Isle.config.taskbarLayout || null)
      if (JSON.stringify(layout) === JSON.stringify(kept)) return
      setConfig({ taskbarLayout: clone(layout) })
    },
    // Editing over: what it had open goes (the picker on the taskbar).
    ended() {
      if (drag) end(false)
      press = null
      closePop()
      if (home() === 'taskbar') hide()
      window.CapsulePanel?.close()
    },
    closeAll() {
      hide()
      closePop()
      window.CapsulePanel?.close()
    },
    // The strip laid out anew: the popover over its widget again.
    relaid() {
      if (popFor) placePop()
      if (drag && !band.hidden) placeBand()
    },
    changed() {
      if (open) drawBadges()
      window.CapsulePanel?.refresh()
    },
    // Where the window takes the mouse (island.js sendSolid).
    solids: () => [...(open ? [pk] : []), ...(pop.hidden ? [] : [pop]), ...(toasts.children.length ? [toasts] : []), ...(window.CapsulePanel?.isOpen() ? [window.CapsulePanel.el] : [])],
    watches: () => (!!popFor && popSticky) || !!window.CapsulePanel?.isOpen(),
    // The island's home: room in the window for the picker, the panel, a toast under her island.
    roomOver(room, base) {
      if (home() !== 'island') return room
      const want = open ? 640 : window.CapsulePanel?.isOpen() ? window.CapsulePanel.height() + 72 : toasts.children.length ? 120 : 0
      if (!want) return room
      const r = room || base
      return { width: Math.max(r.width, base.width), height: Math.max(r.height, want) }
    },
    // For capsule-panel.js: her capsule's list, written; taken out with a way back.
    listNow,
    setConfig,
    writeCapsule,
    removeFromCapsule,
    putInCapsule,
    toast,
    theme: light,
    nameOfKey,
    colorOf: w => colorOf({ w }),
  }
})()
