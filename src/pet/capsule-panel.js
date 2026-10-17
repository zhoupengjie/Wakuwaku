// Her capsule's panel (the picker of 2026-10-17): a popover over her
// capsule on the taskbar (under the island at the top), opened by a press on
// her capsule while the taskbar's layout is edited, or by her menu's 编辑胶囊.
// Loaded after picker.js, which it works with (window.Picker) as island.js
// does (window.Isle).
//
// It lists the capsule's members (capsule.js), in their order: each with a
// grip to drag it up or down (that order is their turns' and the open
// island's), its icon, name and what it says now, how it shows (轮播 its
// turn in the closed capsule, 固定 always at its right end, two at most,
// 展开时 in the open island only), and × (移出胶囊: off, with a way back).
// At its foot how long a turn lasts (widgetSpin; the Plugins page has it
// too for now). A widget dragged from the picker may be let go of over it.
// Sessions come first: the capsule shows a session that needs you before
// any of its widgets.
;(function () {
  const Isle = window.Isle
  const Picker = window.Picker
  if (!Isle || !Picker) {
    window.CapsulePanel = null
    return
  }
  const { t } = window.I18n
  const Widgets = window.Widgets
  const WCards = window.WCards
  const C = window.Capsule
  const el = Isle.el
  const T = (key, vars) => t(Isle.lang, key, vars)
  const esc = WCards.esc
  const reduced = () => matchMedia('(prefers-reduced-motion: reduce)').matches
  const GRIP = '<svg viewBox="0 0 10 16" aria-hidden="true"><circle cx="3" cy="3" r="1.3"/><circle cx="7" cy="3" r="1.3"/><circle cx="3" cy="8" r="1.3"/><circle cx="7" cy="8" r="1.3"/><circle cx="3" cy="13" r="1.3"/><circle cx="7" cy="13" r="1.3"/></svg>'

  const panel = el('div', 'cp')
  panel.hidden = true
  panel.setAttribute('role', 'dialog')
  for (const type of ['mouseenter', 'mouseleave']) panel.addEventListener(type, () => window.pet.hover(type === 'mouseenter'))
  panel.addEventListener('contextmenu', e => e.preventDefault())
  document.body.append(panel)
  let open = false
  let drawn = ''
  let rowDrag = null

  const home = () => Isle.home()

  // Its members as they are now (picker.js listNow), each with its widgets.
  function members() {
    const list = Picker.listNow()
    const all = Isle.allWidgets
    return list.map(m => {
      const id = m.key.slice(m.key.indexOf(':') + 1)
      const ws = m.key.startsWith('plugin:') ? all.filter(w => w.plugin === id) : all.filter(w => w.id === id)
      return { ...m, w: ws[0] || null }
    })
  }

  function html() {
    const ms = members()
    const list = ms.map(({ key, mode }) => ({ key, mode }))
    const detailed = Isle.isDetailed()
    const count = C.MODES.map(mode => [mode, ms.filter(m => m.mode === mode).length]).filter(([, n]) => n)
    const head = `<div class="cp-head"><b>${esc(T('capsule.title'))}</b><span class="cp-count">${esc(count.map(([mode, n]) => T(`capsule.count.${mode}`, { n })).join(' · '))}</span><button type="button" class="pk-ib" data-cp="close" title="${esc(T('picker.close'))}"><span class="pk-fi" aria-hidden="true">&#xE8BB;</span></button></div>`
    const seg = (key, mode, on, why = '') => `<button type="button" class="pk-seg${on ? ' on' : ''}" data-cp="mode" data-key="${esc(key)}" data-v="${mode}"${why ? ` aria-disabled="true" title="${esc(why)}"` : ''}>${esc(T(`capsule.${mode === 'open' ? 'openOnly' : mode}`))}</button>`
    const rows = ms.map(m => {
      const w = m.w
      const name = Picker.nameOfKey(m.key)
      const said = w ? WCards.briefOf(w, { lang: Isle.lang, light: Isle.winLight, detailed }).value : T('taskbar.w.none')
      const icon = w ? Widgets.iconHTML(w, Isle.lang) : Widgets.icon('dot')
      const why = m.mode !== 'pin' && !C.canPin(list, m.key) ? T('capsule.pinFull') : ''
      return `<div class="cp-row" data-key="${esc(m.key)}"><span class="cp-grip" title="${esc(T('capsule.drag'))}">${GRIP}</span><span class="pk-wi" style="--c:${w ? Picker.colorOf(w) : 'currentColor'}">${icon}</span><span class="cp-name">${esc(name)}</span><span class="cp-val">${esc(said)}</span><span class="pk-segs">${seg(m.key, 'turn', m.mode === 'turn')}${seg(m.key, 'pin', m.mode === 'pin', why)}${seg(m.key, 'open', m.mode === 'open')}</span><button type="button" class="cp-out" data-cp="out" data-key="${esc(m.key)}" title="${esc(T('capsule.out'))}"><span class="pk-fi" aria-hidden="true">&#xE711;</span></button></div>`
    })
    const spin = C.spinOf(Isle.config)
    const spins = C.SPINS.map(s => `<button type="button" class="pk-seg${s === spin ? ' on' : ''}" data-cp="spin" data-v="${s}">${esc(s ? T('capsule.sec', { n: s }) : T('capsule.noSpin'))}</button>`).join('')
    return `${head}<div class="cp-list">${rows.join('') || `<p class="cp-empty">${esc(T('capsule.empty'))}</p>`}</div><div class="cp-foot"><span>${esc(T('capsule.spin'))}</span><span class="pk-segs">${spins}</span></div><p class="cp-note">${esc(T('capsule.note'))}</p>`
  }

  // Drawn anew when what it says changed (the monitor's readings every 2 s),
  // not while a row is being dragged.
  function draw(force = false) {
    if (!open || rowDrag) return
    panel.classList.toggle('pk-l', Isle.winLight)
    panel.dataset.home = home()
    const now = html()
    if (now === drawn && !force) return place()
    drawn = now
    panel.innerHTML = now
    place()
  }

  // Over her capsule on the strip (beside a side bar's), under the island
  // at the top; kept on the screen.
  function place() {
    const w = panel.offsetWidth
    const h = panel.offsetHeight
    let x
    let y
    if (home() === 'taskbar') {
      const r = Isle.herSlot.getBoundingClientRect()
      const edge = Isle.tbEdge()
      x = r.left
      y = r.top - h - 10
      if (edge === 'top') y = r.bottom + 10
      if (edge === 'left') [x, y] = [r.right + 10, r.top]
      if (edge === 'right') [x, y] = [r.left - w - 10, r.top]
    } else {
      const r = Isle.island.getBoundingClientRect()
      x = r.left + r.width / 2 - w / 2
      y = Math.max(r.bottom, 44) + 12
    }
    panel.style.left = `${Math.round(Math.max(8, Math.min(x, innerWidth - w - 8)))}px`
    panel.style.top = `${Math.round(Math.max(8, Math.min(y, innerHeight - h - 8)))}px`
  }

  function show() {
    if (open) return draw(true)
    open = true
    panel.hidden = false
    drawn = ''
    draw(true)
    panel.classList.remove('cp-in')
    void panel.offsetWidth
    panel.classList.add('cp-in')
    if (home() === 'island') window.pet.keyboard(true)
    changed()
  }
  function hide() {
    if (!open) return
    open = false
    rowDrag = null
    panel.hidden = true
    if (home() === 'island' && !Picker.isOpen()) window.pet.keyboard(false)
    changed()
  }
  // The window's room and where it takes the mouse; presses elsewhere watched.
  function changed() {
    Isle.watchPresses()
    if (home() === 'island') Isle.update()
    else Isle.sendSolid()
    requestAnimationFrame(() => Isle.sendSolid())
  }

  panel.addEventListener('click', e => {
    const b = e.target.closest('[data-cp]')
    if (!b || b.getAttribute('aria-disabled') === 'true') return
    const what = b.dataset.cp
    if (what === 'close') return hide()
    if (what === 'out') return Picker.removeFromCapsule(b.dataset.key)
    if (what === 'mode') {
      const list = Picker.listNow()
      return Picker.writeCapsule(C.setMode(list, b.dataset.key, b.dataset.v))
    }
    if (what === 'spin') {
      Picker.setConfig(C.spinPatch(Isle.config, Number(b.dataset.v)))
      Isle.update()
      return draw(true)
    }
  })

  // A row dragged by its grip: moved up or down among the others (they make
  // room), let go: that order kept.
  panel.addEventListener('pointerdown', e => {
    const grip = e.target.closest('.cp-grip')
    if (!grip || e.button !== 0) return
    e.preventDefault()
    const row = grip.closest('.cp-row')
    const rows = [...panel.querySelectorAll('.cp-row')]
    rowDrag = { row, from: rows.indexOf(row), to: rows.indexOf(row), y0: e.clientY, id: e.pointerId, h: row.offsetHeight + 2 }
    row.classList.add('cp-lift')
    try {
      panel.setPointerCapture(e.pointerId)
    } catch {}
  })
  panel.addEventListener('pointermove', e => {
    const d = rowDrag
    if (!d || e.pointerId !== d.id) return
    const rows = [...panel.querySelectorAll('.cp-row')]
    const dy = e.clientY - d.y0
    d.row.style.transform = `translateY(${dy}px)`
    const to = Math.max(0, Math.min(rows.length - 1, d.from + Math.round(dy / d.h)))
    if (to === d.to) return
    d.to = to
    rows.forEach((r, i) => {
      if (r === d.row) return
      const shift = d.from < to && i > d.from && i <= to ? -d.h : d.from > to && i >= to && i < d.from ? d.h : 0
      r.style.transition = reduced() ? 'none' : 'transform .16s cubic-bezier(.2,.8,.2,1)'
      r.style.transform = shift ? `translateY(${shift}px)` : ''
    })
  })
  const dropRow = e => {
    const d = rowDrag
    if (!d || e.pointerId !== d.id) return
    rowDrag = null
    if (d.to !== d.from && e.type === 'pointerup') Picker.writeCapsule(C.move(Picker.listNow(), d.from, d.to))
    draw(true)
  }
  panel.addEventListener('pointerup', dropRow)
  panel.addEventListener('pointercancel', dropRow)

  // A press elsewhere in the page (not her capsule, the strip being edited,
  // its bar, the picker or a toast): closed.
  document.addEventListener(
    'pointerdown',
    e => {
      if (!open || panel.contains(e.target) || Isle.island.contains(e.target) || e.target.closest('.pk, .pk-toasts, .pk-pop, .tbedit, #bar-rest')) return
      hide()
    },
    true,
  )
  // Its words and values follow the widgets and the settings.
  window.pet.onWidgets(() => draw())
  window.pet.onUpdate(() => draw())

  window.CapsulePanel = {
    el: panel,
    isOpen: () => open,
    open: show,
    close: hide,
    toggle: () => (open ? hide() : show()),
    refresh: () => draw(true),
    height: () => (open ? panel.offsetHeight : 0),
    // A widget from the picker over it: lit; let go of at y, the place in its list.
    hot: on => panel.classList.toggle('cp-hot', !!on && open),
    indexAt(y) {
      const rows = [...panel.querySelectorAll('.cp-row')]
      const at = rows.findIndex(r => {
        const b = r.getBoundingClientRect()
        return y < b.top + b.height / 2
      })
      return at < 0 ? rows.length : at
    },
  }
})()
