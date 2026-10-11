// A container of widgets on the desktop (desk.rs: its own window, as big
// as it is, under the programs and over the icons). Its widgets in
// columns, or one at a time (the wheel turns them); a press does what a
// tool's does, or opens its card under them (‹ back). Edited with the
// taskbar: a head to move it by (the system's own move, snapped by
// desk.rs), grid or stack, its columns, glass or clear, off; a widget
// chosen by a press (small, medium, large, off) or dragged onto another's
// place.
;(function () {
  const id = new URLSearchParams(location.search).get('box') || ''
  const root = document.getElementById('desk')
  const { t } = window.I18n
  let st = { lang: 'en', details: true, boxes: [], widgets: [], tools: {}, notes: {}, picked: [], editing: false }
  // Each tool's own (a timer, the calculator, a question asked); each
  // stack's shown one; the widget whose card shows; the one chosen while
  // editing; whether the window took the keyboard; launchers' icons.
  const local = new Map()
  const stackCur = new Map()
  let open = null
  let chosen = null
  let keyboard = false
  const icons = new Map()
  let sent = ''
  let press = null

  const esc = v => String(v ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c])
  const T = (key, vars) => t(st.lang, key, vars)
  const me = () => (Array.isArray(st.boxes) ? st.boxes : []).find(b => b.id === id)
  const lightNow = matchMedia('(prefers-color-scheme: light)')
  const shownOf = it => (it?.type === 'stack' ? it.items[(((stackCur.get(it.id) ?? 0) % it.items.length) + it.items.length) % it.items.length] : it)

  function iconOf(path) {
    if (!icons.has(path)) {
      icons.set(path, null)
      window.pet.tools.icon(path).then(png => {
        icons.set(path, png || '')
        if (png) draw()
      })
    }
    return icons.get(path) || null
  }

  const toolX = e => ({ lang: st.lang, entry: e, cfg: e.cfg || {}, state: st.tools || {}, local: local.get(e.id) || {}, now: Date.now(), apps: [], notes: st.notes || {}, picked: st.picked || [], iconOf })

  // The container changed (its widgets, mode, columns, ground), kept by desk.rs.
  function change(patch) {
    const b = me()
    if (!b) return
    Object.assign(b, patch)
    window.pet.desk.change(id, patch)
    draw()
  }
  // One widget in it changed (in a stack too).
  function changeItem(itemId, fn) {
    const b = me()
    if (!b) return
    const items = b.items.map(it =>
      it.id === itemId ? fn(it) : it.type === 'stack' ? { ...it, items: it.items.map(one => (one.id === itemId ? fn(one) : one)) } : it,
    )
    change({ items })
  }

  let notesTimer
  const toolApi = e => ({
    call: (what, arg) => window.pet.tools.do(e.of, what, arg),
    setCfg: patch => changeItem(e.id, it => ({ ...it, cfg: { ...(it.cfg || {}), ...patch } })),
    setLocal: patch => {
      local.set(e.id, { ...(local.get(e.id) || {}), ...patch })
      draw()
    },
    setNotes: text => {
      st.notes = { ...(st.notes || {}), [e.id]: [...String(text)].slice(0, 4000).join('') }
      clearTimeout(notesTimer)
      notesTimer = setTimeout(() => window.pet.settings.set({ notes: st.notes }), 600)
    },
    taskbar: what => window.pet.taskbar.open(what),
    window: w => window.pet.taskbar.window(Number(w), 'press'),
  })

  // A widget's tile (a plugin's or a script's).
  function dataTile(e, size) {
    const from = Widgets.sourcesOf(e, st.widgets || [])
    const w = from.length ? from[Math.floor(Date.now() / 5000) % from.length] : null
    const name = Widgets.nameOf(st.lang, e, st.widgets || [], st.details)
    const icon = `<span class="gico" style="color:${esc((w && Widgets.deepen(w.color, lightNow.matches ? '#fbfbfb' : '#000000', lightNow.matches ? 4.5 : 1)) || '#8e8e93')}">${Widgets.icon(w?.icon || e.icon || 'dot')}</span>`
    const said = w ? Widgets.words(st.lang, w, st.details) : null
    const none = T('taskbar.w.none')
    if (size === 'S') return `<div class="gs">${icon}<b>${esc(said?.value || '—')}</b></div>`
    const top = `<div class="gtop">${icon}<span>${esc(name)}</span></div>`
    if (size === 'M') return `${top}<b class="gval">${esc(said?.value || none)}</b>`
    if (from.length > 1) return top + from.map(x => Widgets.words(st.lang, x, st.details)).map(x => `<div class="grow2"><span>${esc(x.label)}</span><b>${esc(x.value)}</b></div>`).join('')
    return `${top}<b class="gbig">${esc(said?.value || none)}</b>`
  }

  function tileHTML(it) {
    const e = shownOf(it)
    const size = it.size || 'M'
    const html = e.type === 'tool' ? Tools.tile(e.of, size, toolX(e)) : dataTile(e, size)
    const dots = it.type === 'stack' ? `<span class="tdots">${it.items.map((_, i) => `<i class="${it.items[i] === e ? 'on' : ''}"></i>`).join('')}</span>` : ''
    return html + dots
  }

  // The card of the one opened: a tool's own, or a widget's lines.
  function cardHTML(it) {
    const e = shownOf(it)
    const name = Widgets.nameOf(st.lang, e, st.widgets || [], st.details)
    let body
    if (e.type === 'tool') body = Tools.card(e.of, toolX(e))
    else
      body =
        Widgets.sourcesOf(e, st.widgets || [])
          .map(x => Widgets.words(st.lang, x, st.details))
          .map(x => `<div class="wgrow"><span>${esc(x.label)}</span><b>${esc(x.value)}</b></div>`)
          .join('') || `<div class="tnone">${esc(T('taskbar.w.none'))}</div>`
    const mark = e.type === 'tool' ? Tools.icon(e.of) : Widgets.icon(e.icon || 'dot')
    return `<div class="icard"><div class="ichead"><button type="button" class="icback" data-d="back" title="${esc(T('taskbar.grid.back'))}">‹</button>${mark}<b>${esc(name)}</b></div><div class="tcard">${body}</div></div>`
  }

  // Its head while editing: moved by the grip; grid or stack, its columns,
  // its ground, off.
  function headHTML(b) {
    const button = (what, label, on = false) => `<button type="button" class="dbtn${on ? ' on' : ''}" data-d="${what}">${label}</button>`
    const cols = b.mode === 'grid' ? `${button('cols-', '−')}<span class="dn">${esc(T('desk.cols', { n: b.cols || 1 }))}</span>${button('cols+', '+')}` : ''
    return `<div class="dhead"><span class="dgrip" data-d="move" title="${esc(T('desk.move'))}">⠿</span>${button('grid', esc(T('desk.grid')), b.mode !== 'stack')}${button('stack', esc(T('desk.stack')), b.mode === 'stack')}${cols}${button('style', esc(T(b.style === 'clear' ? 'desk.clear' : 'desk.glass')))}${button('remove', '✕')}</div>`
  }

  // The one chosen while editing: its size, or off.
  function toolsHTML(b) {
    const it = b.items.find(x => x.id === chosen)
    if (!it) return ''
    const button = (what, label, on = false) => `<button type="button" class="dbtn${on ? ' on' : ''}" data-d="${what}">${label}</button>`
    return `<div class="dtools">${['S', 'M', 'L'].map(z => button(`size${z}`, esc(T(`taskbar.w.size.${z}`)), (it.size || 'M') === z)).join('')}${button('take', esc(T('taskbar.w.remove')))}</div>`
  }

  // Drawn anew when what it shows changed (never under a field being typed
  // in); its window as big as it is.
  function draw() {
    const b = me()
    if (!b) return
    const typing = root.contains(document.activeElement) && document.activeElement.matches('input, textarea')
    document.documentElement.classList.toggle('light', lightNow.matches)
    const editing = !!st.editing
    if (!editing) chosen = null
    const stack = b.mode === 'stack'
    const items = Array.isArray(b.items) ? b.items : []
    const at = ((stackCur.get(id) ?? 0) % items.length + items.length) % (items.length || 1)
    const shown = stack ? items.slice(at, at + 1) : items
    const cols = stack ? ((shown[0]?.size || 'M') === 'S' ? 1 : 2) : Math.max(1, Math.min(6, b.cols || 2))
    const tiles = shown.map(it => `<div class="itile${it.type === 'stack' ? ' stack' : ''}${chosen === it.id ? ' tb-chosen' : ''}" data-size="${esc(it.size || 'M')}" data-item="${esc(it.id)}">${tileHTML(it)}</div>`).join('')
    const dots = stack && items.length > 1 ? `<span class="tdots dboxdots">${items.map((_, i) => `<i class="${i === at ? 'on' : ''}"></i>`).join('')}</span>` : ''
    const openIt = open && items.find(it => it.id === open)
    const html = `<div class="dbox ${b.style === 'clear' ? 'clear' : 'glass'}${editing ? ' editing' : ''}">${editing ? headHTML(b) : ''}<div class="igrid" style="--cols:${cols}">${tiles}</div>${dots}${editing ? toolsHTML(b) : ''}${!editing && openIt ? cardHTML(openIt) : ''}</div>`
    if (html !== root._html && !typing) {
      root._html = html
      root.innerHTML = html
    }
    const box = root.firstElementChild
    if (!box) return
    const r = box.getBoundingClientRect()
    const size = `${Math.ceil(r.width)}x${Math.ceil(r.height)}`
    if (size !== sent) {
      sent = size
      window.pet.desk.size(id, Math.ceil(r.width), Math.ceil(r.height))
    }
  }

  // --- The pointer ------------------------------------------------------------------
  root.addEventListener('click', e => {
    const b = me()
    if (!b) return
    const what = e.target.closest('[data-d]')?.dataset.d
    if (what) return act(b, what)
    const tile = e.target.closest('.itile')
    const card = e.target.closest('.icard')
    const it = tile ? b.items.find(x => x.id === tile.dataset.item) : card ? b.items.find(x => x.id === open) : null
    if (!it) return
    if (st.editing) {
      chosen = chosen === it.id ? null : it.id
      return draw()
    }
    const one = shownOf(it)
    const button = e.target.closest('[data-t]')
    if (button && one.type === 'tool') {
      const w = button.dataset.t
      let arg = button.dataset.arg ?? ''
      if (w === 'setPath') arg = [...String(root.querySelector('[data-field="path"]')?.value || '').replace(/[\u0000-\u001f\u007f-\u009f]/g, '').trim()].slice(0, 300).join('')
      Tools.act(one.of, w, arg, { ...toolX(one), data: { ...button.dataset } }, toolApi(one))
      if (card && ['window', 'search', 'start', 'open', 'do'].includes(w) && one.of !== 'timer') close()
      return draw()
    }
    if (tile) {
      if (one.type === 'tool' && one.of === 'timer' && local.get(one.id)?.done) local.set(one.id, { ...local.get(one.id), done: false })
      if (one.type === 'tool' && Tools.press(one.of, toolX(one), toolApi(one))) return draw()
      open = open === it.id ? null : it.id
      // A question a card asked goes with it.
      for (const [k, l] of local) if (l.confirm) local.set(k, { ...l, confirm: null, askedAt: 0 })
      draw()
    }
  })

  function close() {
    open = null
    if (keyboard) {
      keyboard = false
      window.pet.keyboard(false)
    }
  }

  // The head's buttons, the chosen one's, the card's ‹ back.
  function act(b, what) {
    if (what === 'back') {
      close()
      return draw()
    }
    if (what === 'grid' || what === 'stack') return change({ mode: what })
    if (what === 'cols-' || what === 'cols+') {
      const least = b.items.some(it => (it.size || 'M') !== 'S') ? 2 : 1
      return change({ cols: Math.max(least, Math.min(6, (b.cols || 2) + (what === 'cols+' ? 1 : -1))) })
    }
    if (what === 'style') return change({ style: b.style === 'clear' ? 'glass' : 'clear' })
    if (what === 'remove') return window.pet.desk.remove(id)
    if (what.startsWith('size') && chosen) {
      const size = what.slice(4)
      const items = b.items.map(it => (it.id === chosen ? { ...it, size } : it))
      const cols = size === 'S' ? b.cols : Math.max(2, b.cols || 2)
      return change({ items, cols })
    }
    if (what === 'take' && chosen) {
      const items = b.items.filter(it => it.id !== chosen)
      chosen = null
      // The last one taken: the container goes (desk.rs).
      return items.length ? change({ items }) : window.pet.desk.remove(id)
    }
  }

  // Moved by its grip (the system's own move); a widget dragged onto
  // another's place goes there (before it, or after past its middle).
  root.addEventListener('pointerdown', e => {
    if (!st.editing || e.button !== 0) return
    if (e.target.closest('[data-d="move"]')) {
      e.preventDefault()
      return window.pet.desk.move(id)
    }
    const tile = e.target.closest('.itile')
    if (tile) press = { item: tile.dataset.item, x: e.clientX, y: e.clientY }
  })
  root.addEventListener('pointerup', e => {
    const p = press
    press = null
    const b = me()
    if (!p || !b || Math.hypot(e.clientX - p.x, e.clientY - p.y) < 6) return
    const over = document.elementFromPoint(e.clientX, e.clientY)?.closest('.itile')
    if (!over || over.dataset.item === p.item) return
    const r = over.getBoundingClientRect()
    const items = b.items.filter(it => it.id !== p.item)
    const moving = b.items.find(it => it.id === p.item)
    let at = items.findIndex(it => it.id === over.dataset.item)
    if (e.clientX > r.left + r.width / 2) at++
    items.splice(at, 0, moving)
    change({ items })
  })

  root.addEventListener(
    'wheel',
    e => {
      const b = me()
      if (!b) return
      // A stack of widgets in it, or the container one at a time.
      const tile = e.target.closest('.itile.stack')
      const it = tile && b.items.find(x => x.id === tile.dataset.item)
      const step = e.deltaY > 0 ? 1 : -1
      if (it) stackCur.set(it.id, (stackCur.get(it.id) ?? 0) + step + it.items.length)
      else if (b.mode === 'stack' && b.items.length > 1) stackCur.set(id, (stackCur.get(id) ?? 0) + step + b.items.length)
      else return
      e.preventDefault()
      draw()
    },
    { passive: false },
  )

  // A field on a card: the keyboard once clicked; a note kept as typed;
  // Enter keeps a launcher's path.
  root.addEventListener('mousedown', e => {
    const field = e.target.closest('.icard input, .icard textarea')
    if (!field || keyboard) return
    keyboard = true
    window.pet.keyboard(true)
    setTimeout(() => field.focus(), 30)
  })
  root.addEventListener('input', e => {
    const b = me()
    const it = b && shownOf(b.items.find(x => x.id === open))
    if (it && e.target.matches('[data-field="note"]')) toolApi(it).setNotes(e.target.value)
  })
  root.addEventListener('keydown', e => {
    if (e.key === 'Enter' && e.target.matches('[data-field="path"]')) root.querySelector('[data-t="setPath"]')?.click()
    if (e.key === 'Escape') {
      close()
      draw()
    }
  })

  // --- What it hears ---------------------------------------------------------------
  const take = s => {
    if (!s || typeof s !== 'object') return
    st = { ...st, ...s }
    if (!me()) return
    draw()
  }
  window.pet.onDesk(take)
  window.pet.onTools(state => {
    st.tools = state || {}
    draw()
  })
  window.pet.desk.state().then(take)
  lightNow.addEventListener('change', () => draw())
  // Each second: clocks and timers go on; a timer run out rings.
  setInterval(() => {
    const b = me()
    if (!b) return
    for (const top of b.items)
      for (const e of Widgets.itemsOf(top)) {
        if (e.type !== 'tool' || e.of !== 'timer') continue
        const x = toolX(e)
        if (Tools.rings(x)) {
          local.set(e.id, { ...x.local, endAt: null, left: 0, done: true })
          window.pet.tools.do('timer', 'done', t(st.lang, 'tool.timer.rang', { name: Widgets.nameOf(st.lang, e) }))
        }
      }
    draw()
  }, 1000)
})()
