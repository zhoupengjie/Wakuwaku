// The desktop's widgets on one display (the widget design of 2026-10-16, §3,
// §7, §8, §9, §10; desk.rs makes a window for each display that has some):
// each alone, several stacked, or several in a board, where it stands on
// the display's grid, its glass the wallpaper under it blurred (mica.js
// glassPaper, desk.rs reads which picture), the glass colour over it.
//
// Alone: dragged anywhere (it snaps to the grid, clear of the icons and the
// others; dropped on icons, the nearest free spot); two buttons on the
// pointer's (size, more); a double-click opens its settings. Stacked: one
// dragged onto another of its size and held there half a second; a drag
// moves the whole stack as one alone moves; the wheel (a touchpad's scroll)
// or a press on its dots turns it (0.45 s); one comes out by its menu.
// A board (工作台, §7 and its note of 2026-10-16): a larger pane, its glass
// blurred 40, its title bar always there (its name, how many, Add, More,
// Fold); its widgets flat tiles on its own grid of 8 columns, as many rows
// as they need, 2 to 4. One alone dragged onto it lands in the nearest
// empty cell (a row more if need be); a full board shakes and it goes back;
// a stack may not go in (it says so). A tile dragged off the board is one
// alone again; dragged within, it moves. Folded, the title bar alone with
// its widgets' glyphs. Its name: a double-click, Enter or Esc.
// Right-click: its menu, as the taskbar's and the capsule's.
//
// The window lets clicks through but where a widget is (desk.rs polls the
// pointer against the shapes this page sends, desk_shapes); while a press is
// held it keeps the mouse (desk_hold). Nothing moves by itself, nothing is
// drawn while nothing changes.
;(function () {
  const { invoke } = window.__TAURI__.core
  const { listen } = window.__TAURI__.event
  const { I18n, Widgets, WCards, WDesk, DeskGrid: G, Mica } = window
  const LABEL = window.__TAURI_INTERNALS__?.metadata?.currentWebview?.label || ''
  const target = { kind: 'WebviewWindow', label: LABEL }
  const desk = document.getElementById('desk')

  // What desk.rs says: this display (key, primary, keys of those there,
  // work area, size), her language, Windows' mode, the specifics setting,
  // her home and what its taskbar shows, her sheet; the layout (deskLayout,
  // every display's).
  let here = null
  let layout = []
  let all = []
  // The wallpaper blurred: for the widgets (30) and, while a board is here,
  // for the boards (40); { canvas, margin, scale } each.
  let paper = null
  let paper40 = null
  let paperTurn = 0
  let lastPaper = null
  let paper40For = null
  // The icons on this display (desk_icons), as rects.
  let icons = []
  const nodes = new Map()
  const tops = new Map()
  let spots = new Map()
  const drawn = new Set()
  const firstSeen = new Map()
  let menu = null
  let drag = null
  let turnedAt = 0
  let renaming = null

  const lang = () => here?.lang || 'zh'
  const t = (key, vars) => I18n.t(lang(), key, vars)
  const esc = WCards.esc
  const ctx = () => ({ lang: lang(), light: !!here?.light, detailed: here?.detailed !== false, now: Date.now(), all })
  const newId = () => `w${Date.now().toString(36).slice(-6)}${Math.random().toString(36).slice(2, 6)}`
  const reduced = () => matchMedia('(prefers-reduced-motion: reduce)').matches
  const grid = () => G.gridOf(here.work)
  const shown = () => (here ? G.shownHere(layout, here) : [])
  const visiting = e => !!e.d && e.d !== here.key
  const find = id => layout.find(e => e.id === id)
  const isBoard = n => !!n && G.isBoard(n.entry)

  // --- What is drawn ---------------------------------------------------------------------
  function draw() {
    if (!here) return
    const entries = shown()
    const g = grid()
    const fresh = new Set(entries.filter(e => !drawn.has(e.id)).map(e => e.id))
    // Something new here and the icons read a while ago: read again first.
    if (fresh.size && Date.now() - iconsAt > ICONS_FRESH_MS) {
      if (!iconsLoading)
        iconsLoading = loadIcons().then(() => {
          iconsLoading = null
          draw()
        })
      return
    }
    const { at, moved } = G.settle(entries, here, g, icons, fresh)
    spots = at
    for (const e of entries) {
      drawn.add(e.id)
      for (const m of G.membersOf(e)) if (!firstSeen.has(seenKey(e, m))) firstSeen.set(seenKey(e, m), Date.now())
    }
    if (moved.length) keep(entries.filter(e => moved.includes(e.id)).map(e => G.placed(e, here.key, at.get(e.id), visiting(e))))
    const ids = new Set(entries.map(e => e.id))
    for (const [id, n] of nodes)
      if (!ids.has(id)) {
        n.el.remove()
        nodes.delete(id)
        tops.delete(id)
        // Back here later (from another display): new here again, clear of the icons.
        drawn.delete(id)
      }
    for (const e of entries) drawEntry(e, at.get(e.id))
    if (menu && !nodes.has(menu.id)) closeMenu()
    if (lib && !nodes.has(lib.id)) closeLib()
    // A board here, and its glass not blurred yet: the picture again (once for each).
    if (entries.some(G.isBoard) && !paper40 && lastPaper && paper40For !== lastPaper) {
      paper40For = lastPaper
      setPaper(lastPaper)
    }
    shapes()
  }

  function drawEntry(e, cell) {
    let n = nodes.get(e.id)
    if (!n) {
      n = { el: document.createElement('div'), key: '', x: 0, y: 0 }
      n.el.dataset.id = e.id
      desk.append(n.el)
      nodes.set(e.id, n)
      wire(n)
    }
    n.entry = e
    n.rect = G.rectAt(grid(), cell, G.boxOf(e))
    const key = contentKey(e)
    if (key !== n.key) {
      n.key = key
      if (G.isBoard(e)) fillBoard(n)
      else fill(n)
    }
    if (!(drag && drag.n === n && drag.mode !== 'press' && drag.tile < 0)) place(n, n.rect.x, n.rect.y)
  }

  // What a node shows changes with: its entry, the one on top, the words of
  // its widgets, the mode, the language, the specifics, and a member just
  // put there (loading) or not; a board's name as shown too.
  function contentKey(e) {
    const ws = G.membersOf(e).map(m => Widgets.sourcesOf(m, all).map(w => [w.id, w.label, w.value, w.detail, w.color, w.private]))
    const loading = G.membersOf(e).map(m => isLoading(e, m))
    const board = G.isBoard(e) ? [boardLabel(e), e.folded, e.items.map(x => [x.size, x.at])] : null
    return JSON.stringify([e.type, e.size, G.membersOf(e), topOf(e), ws, here.light, lang(), here.detailed, here.sprite, loading, board])
  }
  const topOf = e => (G.isStack(e) ? Math.min(tops.get(e.id) || 0, e.items.length - 1) : 0)
  // Just put there, its plugin starting: a glint where its value goes, five
  // seconds at most (the design session), then greyed with "no data".
  const LOADING_MS = 5000
  const seenKey = (e, m) => `${e.id}/${G.keyOf(m)}`
  const isLoading = (e, m) => !Widgets.sourcesOf(m, all).length && Date.now() - (firstSeen.get(seenKey(e, m)) || 0) < LOADING_MS

  // A member's face at a size (the entry's, or a tile's): its widget's (a
  // plugin's first, which shows the others), or, with nothing to say, its
  // name greyed.
  function face(e, m, size = e.size) {
    const ws = Widgets.sourcesOf(m, all)
    const c = ctx()
    if (!ws.length) {
      const name = emptyName(e, m, c)
      return { html: WDesk.emptyHTML(m, name, isLoading(e, m) ? 'loading' : 'none', c), color: '', w: null }
    }
    return { html: WDesk.faceHTML(ws[0], size, c), color: WCards.colorOf(ws[0], c), w: ws[0] }
  }

  // A card: its own picture of the wallpaper under it (a canvas: desk.rs's
  // picture comes from another origin, which a page may draw but not read),
  // the glass colour and its glow over that, then its words.
  // A member with nothing to say: the name it was put there with, else its
  // plugin's (a built-in one's), numbered when several here would read the
  // same (wdesk.js memberName): never an id, never a bare "widget".
  function emptyName(e, m, c) {
    if (m.name) return m.name
    const empty = shown().flatMap(x => G.membersOf(x)).filter(x => !x.name && !Widgets.sourcesOf(x, all).length)
    const base = WDesk.memberName(m, c)
    const same = empty.filter(x => WDesk.memberName(x, c) === base)
    return same.length > 1 ? WDesk.memberName(m, c, same.findIndex(x => G.keyOf(x) === G.keyOf(m)) + 1) : base
  }

  const TOOLS = `<div class="tools"><button type="button" data-act="size" title="">&#xE740;</button><button type="button" data-act="more" title="">&#xE712;</button></div>`
  const CARD = size => `<div class="wd ${size}"><canvas class="wpaper"></canvas><div class="wtint"></div><div class="wbody"></div></div>`
  // Each member's place under the one on top: 0 on top, then the two
  // layers that show, then the rest unseen behind them.
  const LAYERS = ['top', 'b1', 'b2']
  const layerOf = (i, top, n) => LAYERS[(i - top + n) % n] || 'rest'
  const toolTitles = root => {
    for (const b of root.querySelectorAll('.tools button')) b.title = t(b.dataset.act === 'size' ? 'menu.size' : 'desk.more')
  }
  // A member's words into its card's body, as they changed.
  function fillBody(wd, e, m, size) {
    const body = wd?.querySelector('.wbody')
    if (!body) return
    const f = face(e, m, size)
    if (body._html !== f.html) {
      body.innerHTML = f.html
      body._html = f.html
      fillHer(body)
    }
    wd.classList.toggle('empty-w', !f.w)
    if (f.color) wd.style.setProperty('--c', f.color)
    else wd.style.removeProperty('--c')
  }
  // Loading (just put there, its plugin starting): drawn again once that is
  // over, no data then greyed; one timer, not a loop.
  function loadingTimer(n, e) {
    const left = G.membersOf(e).filter(m => isLoading(e, m)).map(m => LOADING_MS + 50 - (Date.now() - (firstSeen.get(seenKey(e, m)) || 0)))
    clearTimeout(n.loadingTimer)
    if (left.length) n.loadingTimer = setTimeout(draw, Math.max(0, Math.max(...left)))
  }

  // Built anew only when its shape changes (alone or stacked, its size, its
  // members); else only the words that changed, in place (the monitor's
  // every two seconds: its words alone, not its glass).
  function fill(n) {
    const e = n.entry
    const shape = JSON.stringify([e.type, e.size, G.membersOf(e).map(G.keyOf)])
    const top = topOf(e)
    n.el.className = `dw ${e.size}${G.isStack(e) ? ' stack' : ''}${n.el.classList.contains('hover') ? ' hover' : ''}${menu?.id === e.id ? ' menu' : ''}`
    n.el.style.width = n.el.style.height = ''
    if (shape !== n.shape) {
      n.shape = shape
      if (!G.isStack(e)) n.el.innerHTML = CARD(e.size)
      else {
        const layers = e.items.map((m, i) => `<div class="layer ${layerOf(i, top, e.items.length)}" data-i="${i}">${CARD(e.size)}</div>`).join('')
        const dots = e.items.map((_, i) => `<button type="button" data-p="${i}" aria-label="${i + 1}"></button>`).join('')
        n.el.innerHTML = `${layers}<div class="sdots">${dots}</div>`
      }
      const cards = n.el.querySelectorAll('.wd')
      cards[top].insertAdjacentHTML('beforeend', TOOLS)
      toolTitles(n.el)
      n.painted = ''
    }
    G.membersOf(e).forEach((m, i) => fillBody(n.el.querySelectorAll('.wd')[i], e, m, e.size))
    if (G.isStack(e)) {
      n.el.querySelectorAll('.layer').forEach((l, i) => {
        for (const k of ['top', 'b1', 'b2', 'rest', 'gone']) l.classList.toggle(k, k === layerOf(i, top, e.items.length))
      })
      for (const b of n.el.querySelectorAll('.sdots button')) b.classList.toggle('on', Number(b.dataset.p) === top)
      const tools = n.el.querySelector('.tools')
      const onTop = n.el.querySelectorAll('.wd')[top]
      if (tools && onTop && tools.parentElement !== onTop) onTop.append(tools)
    }
    paint(n)
    loadingTimer(n, e)
  }

  // --- A board ---------------------------------------------------------------------------------
  // Its name: its own, else 工作台; when several have none, the first is
  // still 工作台 and the others 工作台 2, 工作台 3 (the design session).
  function boardLabel(b) {
    if (b.name) return b.name
    const at = layout.filter(x => G.isBoard(x) && !x.name).findIndex(x => x.id === b.id)
    return at > 0 ? t('desk.boardN', { n: at + 1 }) : t('desk.board')
  }
  const BOARD_ICONS = 6
  // Its pane: its own picture of the wallpaper (blurred 40), its glass, its
  // title bar; its tiles flat on it, each where its cell is, no glass of their
  // own; an empty one a dashed box to drop into.
  function fillBoard(n) {
    const e = n.entry
    const box = G.boardBox(e)
    const shape = JSON.stringify(['board', !!e.folded, e.items.map(x => [G.keyOf(x), x.size, x.at])])
    n.el.className = `dw board${e.folded ? ' folded' : ''}${n.el.classList.contains('hover') ? ' hover' : ''}${menu?.id === e.id ? ' menu' : ''}`
    n.el.style.width = `${box.w}px`
    n.el.style.height = `${box.h}px`
    if (shape !== n.shape) {
      n.shape = shape
      const tiles = e.items
        .map((x, i) => {
          const r = G.tileRect(x.at, x.size)
          return `<div class="wd tile ${x.size}" data-i="${i}" style="left:${r.x}px;top:${r.y}px"><div class="wtint"></div><div class="wbody"></div>${TOOLS}</div>`
        })
        .join('')
      const empty = e.items.length ? '' : `<div class="bempty" style="left:${G.BOARD.pad}px;top:${G.BOARD.head}px;width:${G.BOARD.width - 2 * G.BOARD.pad}px;height:${G.sizePx('s').h}px"></div>`
      n.el.innerHTML = `<div class="db"><canvas class="wpaper"></canvas><div class="btint"></div><div class="bhead"><span class="btitle"></span><span class="bbtns"><button type="button" data-bact="add">&#xE710;</button><button type="button" data-bact="more">&#xE712;</button><button type="button" data-bact="fold"></button></span></div>${tiles}${empty}</div>`
      toolTitles(n.el)
      n.painted = ''
    }
    // The title bar: its name (not while it is being typed), how many, or
    // folded its widgets' glyphs (six, then +N); its buttons' words.
    if (renaming?.n !== n) {
      const glyphs = e.items.slice(0, BOARD_ICONS).map(x => {
        const w = Widgets.sourcesOf(G.memberOf(x), all)[0]
        const c = w ? WCards.colorOf(w, ctx()) : ''
        return `<span class="bgl"${c ? ` style="--c:${c}"` : ''}>${w ? WCards.glyphHTML(w) : WCards.memberGlyph(G.memberOf(x))}</span>`
      })
      const more = e.items.length > BOARD_ICONS ? `<span class="bmore">+${e.items.length - BOARD_ICONS}</span>` : ''
      // Empty: its name alone (no 「0 个小工具」).
      const after = e.folded ? `<span class="bicons">${glyphs.join('')}${more}</span>` : e.items.length ? `<span class="bcount">${esc(t('desk.boardCount', { n: e.items.length }))}</span>` : ''
      n.el.querySelector('.btitle').innerHTML = `<b class="bname">${esc(boardLabel(e))}</b>${after}`
      fillHer(n.el.querySelector('.btitle'))
    }
    const [add, more, fold] = n.el.querySelectorAll('.bbtns button')
    add.title = t('desk.add')
    more.title = t('desk.more')
    fold.title = t(e.folded ? 'desk.unfold' : 'desk.fold')
    fold.innerHTML = e.folded ? '&#xE70D;' : '&#xE70E;'
    const empty = n.el.querySelector('.bempty')
    if (empty) empty.textContent = t('desk.boardEmpty')
    e.items.forEach((x, i) => fillBody(n.el.querySelector(`.tile[data-i="${i}"]`), e, G.memberOf(x), x.size))
    paint(n)
    loadingTimer(n, e)
  }

  // Her face where today's glyph goes: the still first frame of her sheet.
  function fillHer(root) {
    for (const slot of root.querySelectorAll('.wher')) {
      if (!here.sprite) continue
      const px = slot.closest('.wmh, .bgl') ? 16 : 14
      const s = px * 0.00875
      slot.style.backgroundImage = `url("${here.sprite}")`
      slot.style.backgroundSize = `${1536 * s}px ${2288 * s}px`
      slot.style.backgroundPosition = `${-(192 * s - px) / 2}px -2px`
    }
  }

  // --- Its glass: the wallpaper under it, blurred ---------------------------------------------
  // A card at (x, y), w by h, drawn scaled by s from its middle and moved
  // down by dy (a stack's layers): its canvas given the part of the blurred
  // picture (src) that lies under it on the screen, so the two line up.
  const LAYER_FORM = { top: [1, 0], b1: [0.94, 13], b2: [0.88, 25], rest: [0.88, 25], gone: [0.96, -30] }
  function paperFor(canvas, x, y, w, h, s = 1, dy = 0, src = paper) {
    const dpr = devicePixelRatio || 1
    const cw = Math.round(w * dpr)
    const ch = Math.round(h * dpr)
    if (canvas.width !== cw) canvas.width = cw
    if (canvas.height !== ch) canvas.height = ch
    const c = canvas.getContext('2d')
    if (!src) return c.clearRect(0, 0, cw, ch)
    const k = src.scale
    const sx = (x + (w / 2) * (1 - s) + src.margin) * k
    const sy = (y + (h / 2) * (1 - s) + dy + src.margin) * k
    c.imageSmoothingEnabled = true
    c.imageSmoothingQuality = 'high'
    c.drawImage(src.canvas, sx, sy, w * s * k, h * s * k, 0, 0, cw, ch)
  }
  // Each of its cards painted for where it is now; not again while that is
  // so (the words changing leave its glass as it is). A board: its pane, at
  // blur 40 (that of 30 until it is there); its tiles have none.
  function paint(n, force = false) {
    if (isBoard(n)) {
      const box = G.boardBox(n.entry)
      const now = JSON.stringify([n.x, n.y, box.w, box.h, paperTurn, !!paper40, devicePixelRatio])
      if (!force && now === n.painted) return
      n.painted = now
      const canvas = n.el.querySelector('.db > .wpaper')
      if (canvas) paperFor(canvas, n.x, n.y, box.w, box.h, 1, 0, paper40 || paper)
      return
    }
    const { w, h } = G.sizePx(n.entry.size)
    const cards = [...n.el.querySelectorAll('.wd')]
    const forms = cards.map(wd => {
      const layer = wd.parentElement?.classList.contains('layer') ? wd.parentElement : null
      const kind = layer ? ['top', 'b1', 'b2', 'rest', 'gone'].find(k => layer.classList.contains(k)) || 'top' : 'top'
      return kind
    })
    const now = JSON.stringify([n.x, n.y, w, h, forms, paperTurn, devicePixelRatio])
    if (!force && now === n.painted) return
    n.painted = now
    cards.forEach((wd, i) => {
      const canvas = wd.querySelector('.wpaper')
      if (!canvas) return
      const kind = forms[i]
      const [s, dy] = LAYER_FORM[kind]
      paperFor(canvas, n.x, n.y, w, h, s, dy)
    })
  }
  function place(n, x, y) {
    n.x = Math.round(x)
    n.y = Math.round(y)
    n.el.style.transform = `translate(${n.x}px, ${n.y}px)`
    paint(n)
  }

  // A new picture (desk:paper): blurred once, for all of them; once more at
  // 40 while a board is here.
  async function setPaper(p) {
    const mine = ++paperTurn
    if (!p) {
      paper = paper40 = null
      for (const n of nodes.values()) paint(n, true)
      return
    }
    let img = null
    if (p.url) {
      img = new Image()
      img.src = p.url
      try {
        await img.decode()
      } catch {
        img = null
      }
    }
    if (mine !== paperTurn) return
    lastPaper = p
    const blurred = blur => {
      const { canvas, margin, scale } = Mica.glassPaper(p, img, { blur, saturate: 1.6 })
      return { canvas, margin, scale }
    }
    paper = blurred(30)
    paper40 = shown().some(G.isBoard) ? blurred(40) : null
    if (paper40) paper40For = p
    if (img) img.src = ''
    for (const n of nodes.values()) paint(n, true)
  }

  // --- Where the window takes the mouse -------------------------------------------------------
  // Each widget (a stack with its layers and its dots), each board, and an
  // open menu or library (a radius of 12: desk.rs takes the whole display
  // while one is open, a press elsewhere closes it).
  let sentShapes = ''
  function shapes() {
    const list = []
    for (const n of nodes.values()) {
      if (isBoard(n)) {
        const { w, h } = G.boardBox(n.entry)
        list.push([n.x, n.y, w, h, 28])
        continue
      }
      const { w, h } = G.sizePx(n.entry.size)
      if (G.isStack(n.entry)) list.push([n.x, n.y, w + 18, h + 28, 22])
      else list.push([n.x, n.y, w, h, 22])
    }
    for (const pop of [menuEl, libEl]) {
      const m = pop && !pop.hidden ? pop.getBoundingClientRect() : null
      if (m) list.push([m.left, m.top, m.width, m.height, 12])
    }
    const now = JSON.stringify(list.map(r => r.map(v => Math.round(v))))
    if (now === sentShapes) return
    sentShapes = now
    invoke('desk_shapes', { shapes: JSON.parse(now) }).catch(() => {})
  }
  const hold = on => invoke('desk_hold', { on }).catch(() => {})

  // --- Kept: what changed, merged with the others' (desk.rs desk_keep) ------------------------
  function keep(entries, gone = []) {
    if (!entries.length && !gone.length) return
    const given = new Map(entries.map(e => [e.id, e]))
    const out = []
    for (const e of layout) {
      if (gone.includes(e.id)) continue
      out.push(given.get(e.id) || e)
      given.delete(e.id)
    }
    layout = [...out, ...given.values()]
    invoke('desk_keep', { entries, gone }).catch(err => console.log('desk: not kept', err))
  }
  // A whole new layout kept as what changed in it.
  function keepLayout(next) {
    const was = new Map(layout.map(e => [e.id, JSON.stringify(e)]))
    const ids = new Set(next.map(e => e.id))
    keep(
      next.filter(e => was.get(e.id) !== JSON.stringify(e)),
      layout.filter(e => !ids.has(e.id)).map(e => e.id),
    )
    draw()
  }

  // --- The pointer: hover, press, drag, drop -----------------------------------------------------
  // A board's tile under the pointer shows its buttons, as one alone does.
  function hoverTile(n, tile) {
    if (!isBoard(n)) return
    for (const x of n.el.querySelectorAll('.tile')) x.classList.toggle('hover', x === tile)
  }
  function wire(n) {
    const el = n.el
    el.addEventListener('mouseenter', () => el.classList.add('hover'))
    el.addEventListener('mouseleave', () => {
      if (drag) return
      el.classList.remove('hover')
      hoverTile(n, null)
    })
    el.addEventListener('mouseover', ev => {
      if (!drag) hoverTile(n, ev.target.closest('.tile'))
    })
    el.addEventListener('pointerdown', ev => press(ev, n))
    el.addEventListener('pointermove', ev => move(ev))
    el.addEventListener('pointerup', ev => release(ev))
    el.addEventListener('pointercancel', () => cancel())
    el.addEventListener('dblclick', ev => {
      // What is under the pointer: the press took the pointer (press:
      // setPointerCapture), so the click and the double-click come to the
      // node itself, not to its name or tile (the VM, 2026-10-16).
      const under = document.elementFromPoint(ev.clientX, ev.clientY) || ev.target
      if (under.closest('.tools, .sdots, .bbtns, input')) return
      if (isBoard(n) && under.closest('.bname')) return startRename(n)
      const tile = tileOf(under)
      const w = isBoard(n) ? (tile >= 0 ? Widgets.sourcesOf(G.memberOf(n.entry.items[tile]), all)[0] : null) : shownWidget(n.entry)
      if (w) ask('settings', { widget: w.id })
    })
    el.addEventListener('contextmenu', ev => {
      ev.preventDefault()
      if (ev.target.closest('input')) return
      const tile = isBoard(n) ? tileOf(ev.target) : -1
      openMenu(n.entry.id, isBoard(n) && tile < 0 ? 'board' : 'full', { x: ev.clientX, y: ev.clientY }, tile)
    })
    el.addEventListener('click', ev => {
      const b = ev.target.closest('.tools button')
      if (b) {
        ev.stopPropagation()
        const r = b.getBoundingClientRect()
        const tile = isBoard(n) ? tileOf(b) : -1
        return menu?.id === n.entry.id && menu.kind === b.dataset.act && menu.tile === tile ? closeMenu() : openMenu(n.entry.id, b.dataset.act, { x: r.left, y: r.bottom + 6, button: r }, tile)
      }
      const ba = ev.target.closest('.bbtns button')
      if (ba) {
        ev.stopPropagation()
        const r = ba.getBoundingClientRect()
        const act = ba.dataset.bact
        if (act === 'fold') return fold(n)
        if (act === 'more') return menu?.id === n.entry.id && menu.kind === 'board' ? closeMenu() : openMenu(n.entry.id, 'board', { x: r.left, y: r.bottom + 6, button: r })
        if (act === 'add') return lib?.id === n.entry.id ? closeLib() : openLib(n, r)
      }
      const dot = ev.target.closest('.sdots button')
      if (dot && G.isStack(n.entry)) turnTo(n, Number(dot.dataset.p))
    })
    el.addEventListener(
      'wheel',
      ev => {
        if (!G.isStack(n.entry)) return
        ev.preventDefault()
        if (performance.now() - turnedAt < (reduced() ? 250 : 450)) return
        const delta = Math.abs(ev.deltaY) >= Math.abs(ev.deltaX) ? ev.deltaY : ev.deltaX
        if (Math.abs(delta) < 1) return
        turn(n, delta > 0 ? 1 : -1)
      },
      { passive: false },
    )
  }
  // The tile an element is in (its index on its board), or -1.
  const tileOf = el => {
    const tile = el?.closest?.('.tile')
    return tile ? Number(tile.dataset.i) : -1
  }

  // The widget a stack shows (its top), or the one alone.
  function shownWidget(e) {
    const m = G.isStack(e) ? e.items[topOf(e)] : G.memberOf(e)
    return Widgets.sourcesOf(m, all)[0] || null
  }

  // A press and a drag (the design session, 2026-10-16: no gestures told
  // apart by time or distance): a drag moves it, alone or the whole stack
  // or board; held over one of its size half a second, it goes onto that one
  // (a new stack, or into that stack); over a board, into its nearest empty
  // cell. A tile of a board dragged out of it is one alone; dragged in it,
  // it moves there. A stack turns by the wheel (a touchpad's scroll too) or
  // a press on its dots; one comes out of it by the menu.
  function press(ev, n) {
    if (ev.button !== 0 || ev.target.closest('.tools, .sdots, .bbtns, input')) return
    closeMenu()
    closeLib()
    const tile = isBoard(n) ? tileOf(ev.target) : -1
    const tileEl = tile >= 0 ? n.el.querySelector(`.tile[data-i="${tile}"]`) : null
    const tr = tile >= 0 ? G.tileRect(n.entry.items[tile].at, n.entry.items[tile].size) : { x: 0, y: 0 }
    drag = { n, tile, tileEl, mode: 'press', id: ev.pointerId, x0: ev.clientX, y0: ev.clientY, dx: ev.clientX - (n.x + tr.x), dy: ev.clientY - (n.y + tr.y), armed: null, over: null, overTimer: 0, into: null }
    try {
      n.el.setPointerCapture(ev.pointerId)
    } catch {}
    hold(true)
    // The icons as they are now, for the drop.
    loadIcons()
  }

  function move(ev) {
    const d = drag
    if (!d || ev.pointerId !== d.id) return
    const n = d.n
    if (d.mode === 'press') {
      if (Math.hypot(ev.clientX - d.x0, ev.clientY - d.y0) < 5) return
      d.mode = 'move'
      n.el.classList.add('dragging')
      d.tileEl?.classList.add('lifted')
    }
    // Its top left where it is being dragged to.
    const x = ev.clientX - d.dx
    const y = ev.clientY - d.dy
    if (d.tile >= 0) {
      const tr = G.tileRect(n.entry.items[d.tile].at, n.entry.items[d.tile].size)
      d.tileEl.style.transform = `translate(${Math.round(x - n.x - tr.x)}px, ${Math.round(y - n.y - tr.y)}px)`
    } else place(n, x, y)
    // A board takes what is not a board; over none, one alone may stack.
    if (d.tile >= 0 || !isBoard(n)) seekBoard(ev, x, y)
    if (d.tile < 0 && !d.into) seek(ev, n.entry)
    else seek(null)
  }

  // Over one it may stack onto, held half a second: that one ringed, armed.
  function seek(ev, moving) {
    const d = drag
    const over = ev ? [...nodes.values()].find(o => o !== d.n && !isBoard(o) && ev.clientX >= o.x && ev.clientX <= o.x + o.rect.w && ev.clientY >= o.y && ev.clientY <= o.y + o.rect.h) : null
    const ok = over && G.canStack(moving, over.entry) ? over : null
    if (ok === d.over) return
    clearTimeout(d.overTimer)
    d.over?.el.classList.remove('target')
    d.over = ok
    d.armed = null
    if (ok)
      d.overTimer = setTimeout(() => {
        if (drag === d && d.over === ok) {
          d.armed = ok.entry.id
          ok.el.classList.add('target')
        }
      }, 500)
  }

  // The board under the pointer (its title bar too), but the one dragged.
  function boardUnder(cx, cy, except) {
    for (const o of nodes.values()) {
      if (!isBoard(o) || o === except) continue
      const { w, h } = G.boardBox(o.entry)
      if (cx >= o.x && cx <= o.x + w && cy >= o.y && cy <= o.y + h) return o
    }
    return null
  }
  // Over a board: where it would land (a dashed place there), none (full:
  // it shakes as it is let go of), or a stack, which may not go in.
  function seekBoard(ev, x, y) {
    const d = drag
    const b = boardUnder(ev.clientX, ev.clientY, d.tile >= 0 ? null : d.n)
    const into = !b ? null : d.tile < 0 && G.isStack(d.n.entry) ? { b, stack: true } : { b, ...spotIn(b, x, y) }
    const was = d.into
    d.into = into
    if (was && (!into || was.b !== into.b || JSON.stringify(was.spot) !== JSON.stringify(into.spot))) was.b.el.querySelector('.bghost')?.remove()
    if (!into?.spot || into.b.entry.folded) return
    let ghost = into.b.el.querySelector('.bghost')
    if (!ghost) {
      ghost = document.createElement('div')
      ghost.className = 'bghost'
      into.b.el.querySelector('.db').append(ghost)
    }
    const r = G.tileRect(into.spot, into.size)
    Object.assign(ghost.style, { left: `${r.x}px`, top: `${r.y}px`, width: `${r.w}px`, height: `${r.h}px` })
  }
  // The cell one dragged with its top left at (x, y) takes in a board.
  function spotIn(b, x, y) {
    const d = drag
    const size = d.tile >= 0 ? d.n.entry.items[d.tile].size : d.n.entry.size
    const skip = d.tile >= 0 && b === d.n ? d.tile : -1
    const spot = b.entry.folded ? G.boardFirst(b.entry, size) : G.boardSpot(b.entry, G.tileAt(x - b.x, y - b.y, size), size, skip)
    return { spot, size }
  }
  function dropInto(d) {
    d?.into?.b.el.querySelector('.bghost')?.remove()
  }

  function release(ev) {
    const d = drag
    if (!d || ev.pointerId !== d.id) return
    drag = null
    clearTimeout(d.overTimer)
    hold(false)
    const n = d.n
    n.el.classList.remove('dragging')
    d.over?.el.classList.remove('target')
    dropInto(d)
    if (d.mode === 'press') return
    const x = ev.clientX - d.dx
    const y = ev.clientY - d.dy
    if (d.tile >= 0) return releaseTile(ev, d, x, y)
    const e = n.entry
    const back = () => place(n, n.rect.x, n.rect.y)
    if (d.armed) {
      // The one dropped on top.
      tops.set(d.armed, 0)
      return keepLayout(G.stackOnto(layout, e.id, d.armed, newId))
    }
    if (d.into) {
      // No stacks in a board (this time round): it says so, and goes back.
      if (d.into.stack) {
        toast(d.into.b, t('desk.noStackInBoard'))
        return back()
      }
      if (!d.into.spot) {
        shake(d.into.b)
        return back()
      }
      return keepLayout(G.intoBoard(layout, d.into.b.entry.id, G.memberOf(e), e.size, d.into.spot, newId))
    }
    const box = G.boxOf(e)
    const off = ev.clientX < 0 || ev.clientY < 0 || ev.clientX > innerWidth || ev.clientY > innerHeight
    if (off) return elsewhere(ev, x - ev.clientX, y - ev.clientY, box, cell => keepLayout(layout.map(o => (o.id === e.id ? G.placed(o, cell.key, cell.at) : o))), back)
    const cell = G.dropAt(grid(), x, y, box, others(e.id), icons)
    n.rect = G.rectAt(grid(), cell, box)
    place(n, n.rect.x, n.rect.y)
    keepLayout(layout.map(o => (o.id === e.id ? G.placed(o, here.key, cell) : o)))
  }
  // A tile let go of: in its board (moved there), in another board, or off
  // the board, one alone there (on another display too).
  function releaseTile(ev, d, x, y) {
    const b = d.n.entry
    const tile = b.items[d.tile]
    const back = () => {
      d.tileEl.style.transform = ''
      d.tileEl.classList.remove('lifted')
    }
    if (d.into) {
      if (!d.into.spot) {
        shake(d.into.b)
        return back()
      }
      if (d.into.b === d.n) {
        back()
        return keepLayout(G.moveInBoard(layout, b.id, d.tile, d.into.spot))
      }
      return keepLayout(G.intoBoard(layout, d.into.b.entry.id, G.memberOf(tile), tile.size, d.into.spot, newId))
    }
    const off = ev.clientX < 0 || ev.clientY < 0 || ev.clientX > innerWidth || ev.clientY > innerHeight
    if (off) return elsewhere(ev, x - ev.clientX, y - ev.clientY, tile.size, cell => keepLayout(G.outOfBoard(layout, b.id, d.tile, newId, cell.key, cell.at)), back)
    const cell = G.dropAt(grid(), x, y, tile.size, others(''), icons)
    keepLayout(G.outOfBoard(layout, b.id, d.tile, newId, here.key, cell))
  }
  function cancel() {
    const d = drag
    if (!d) return
    drag = null
    clearTimeout(d.overTimer)
    hold(false)
    d.n.el.classList.remove('dragging')
    d.over?.el.classList.remove('target')
    dropInto(d)
    if (d.tileEl) {
      d.tileEl.style.transform = ''
      d.tileEl.classList.remove('lifted')
    }
    place(d.n, d.n.rect.x, d.n.rect.y)
  }
  // The others' rects here (not this one; '' all of them).
  const others = id => [...nodes.values()].filter(o => o.entry.id !== id).map(o => o.rect)

  // Let go of on another display: its cell there (desk.rs desk_where says
  // which display, its work area and the point in its window's px); that
  // display's page clears it of icons and others as it comes (settle).
  async function elsewhere(ev, ox, oy, size, then, back) {
    let there = null
    try {
      there = await invoke('desk_where', { x: ev.clientX, y: ev.clientY })
    } catch {}
    if (!there || there.key === here.key) return back()
    const g = G.gridOf(there.work)
    then({ key: there.key, at: G.snap(g, there.x + ox, there.y + oy, size) })
  }

  // The icons as Explorer's list draws them (desk.rs icons: each picture
  // and label, both lines of a long one), read as a press starts, as the
  // page comes up, and before something new is placed here if they were
  // read a while ago (Explorer may have laid them anew meanwhile).
  const ICONS_FRESH_MS = 5000
  let iconsAt = 0
  let iconsLoading = null
  // Not read (null: Explorer starting again, busy), or none (a desktop
  // read before Explorer had laid its icons): as they were, asked again
  // every 3 s, five times at most; known only then, what was placed
  // meanwhile settled again, clear of them (the VM, 2026-10-16: widgets put
  // on the desktop as the page came up lay over icons it had not got).
  let iconsKnown = false
  let iconsMissed = false
  let iconsTries = 0
  let iconsTimer = 0
  async function loadIcons() {
    let list = null
    try {
      list = await invoke('desk_icons')
    } catch {}
    iconsAt = Date.now()
    clearTimeout(iconsTimer)
    if (Array.isArray(list) && list.length) {
      icons = list.map(([x, y, w, h]) => ({ x, y, w, h }))
      iconsKnown = true
      iconsTries = 0
      if (iconsMissed && !drag) {
        iconsMissed = false
        drawn.clear()
        draw()
      }
      return
    }
    if (Array.isArray(list) && !iconsKnown) icons = []
    if (!iconsKnown) iconsMissed = true
    if (iconsTries++ < 5) iconsTimer = setTimeout(loadIcons, 3000)
  }

  // A board refusing one (full): its frame shakes, 0.3 s (§7's note); with
  // motion reduced, a ring for as long.
  function shake(b) {
    const db = b?.el.querySelector('.db')
    if (!db) return
    db.classList.remove('shake')
    void db.offsetWidth
    db.classList.add('shake')
    clearTimeout(b.shakeTimer)
    b.shakeTimer = setTimeout(() => db.classList.remove('shake'), 320)
  }
  // A word on a board for two seconds (a stack may not go in).
  const toastEl = document.createElement('div')
  toastEl.className = 'wtoast'
  toastEl.hidden = true
  document.body.append(toastEl)
  let toastTimer = 0
  function toast(b, text) {
    const { w } = G.boardBox(b.entry)
    toastEl.textContent = text
    toastEl.hidden = false
    toastEl.style.left = `${Math.round(b.x + w / 2)}px`
    toastEl.style.top = `${Math.round(b.y + G.BOARD.head + 10)}px`
    clearTimeout(toastTimer)
    toastTimer = setTimeout(() => (toastEl.hidden = true), 2000)
  }

  // Folded to its title bar, or open again.
  function fold(n) {
    const b = n.entry
    const { folded, ...rest } = b
    keepLayout(layout.map(o => (o.id === b.id ? (folded ? rest : { ...b, folded: true }) : o)))
  }

  // Its name typed in its title bar (a double-click, or its menu): Enter
  // keeps it, Esc leaves it as it was, a press elsewhere keeps it. The
  // window takes the keyboard for it alone (desk.rs desk_type).
  function startRename(n) {
    if (renaming || !isBoard(n)) return
    const name = n.el.querySelector('.bname')
    if (!name) return
    closeMenu()
    const input = document.createElement('input')
    input.className = 'bname-in'
    input.maxLength = G.BOARD_NAME
    input.spellcheck = false
    input.value = boardLabel(n.entry)
    name.replaceWith(input)
    renaming = { n, input }
    input.addEventListener('keydown', ev => {
      if (ev.key === 'Enter') endRename(true)
      else if (ev.key === 'Escape') endRename(false)
      else return
      ev.preventDefault()
    })
    input.addEventListener('blur', () => endRename(true))
    invoke('desk_type', { on: true })
      .catch(() => {})
      .then(() => {
        if (renaming?.input !== input) return
        input.focus()
        input.select()
      })
  }
  function endRename(ok) {
    const r = renaming
    if (!r) return
    renaming = null
    invoke('desk_type', { on: false }).catch(() => {})
    const b = r.n.entry
    const name = r.input.value.trim()
    const named = ok && G.isBoardName(name) && name !== boardLabel(b)
    r.n.key = ''
    if (!named) return draw()
    const { name: _, ...rest } = b
    keepLayout(layout.map(o => (o.id === b.id ? (name ? { ...rest, name } : rest) : o)))
  }

  // --- A stack turned (§7, §9): the one on top up and away, the next rising --------------------
  function turn(n, by) {
    const s = n.entry
    turnTo(n, G.flip(topOf(s), s.items.length, by), by)
  }
  function turnTo(n, next, by = 0) {
    const s = n.entry
    const count = s.items.length
    const was = topOf(s)
    if (next === was || !G.isStack(s)) return
    by = by || (next > was ? 1 : -1)
    turnedAt = performance.now()
    tops.set(s.id, next)
    n.key = contentKey(s)
    const layers = [...n.el.querySelectorAll('.layer')]
    const setTo = (layer, cls) => {
      for (const k of ['top', 'b1', 'b2', 'rest', 'gone']) layer.classList.toggle(k, k === cls)
    }
    // The buttons go with the one on top.
    const tools = n.el.querySelector('.tools')
    if (tools) layers[next].querySelector('.wd').append(tools)
    const still = reduced()
    if (by > 0 && !still) {
      layers.forEach((l, i) => i !== was && setTo(l, layerOf(i, next, count)))
      setTo(layers[was], 'gone')
      setTimeout(() => {
        layers[was].classList.add('instant')
        setTo(layers[was], layerOf(was, next, count))
        void layers[was].offsetWidth
        layers[was].classList.remove('instant')
        paint(n)
      }, 450)
    } else if (by < 0 && !still) {
      layers[next].classList.add('instant')
      setTo(layers[next], 'gone')
      void layers[next].offsetWidth
      layers[next].classList.remove('instant')
      layers.forEach((l, i) => setTo(l, layerOf(i, next, count)))
    } else layers.forEach((l, i) => setTo(l, layerOf(i, next, count)))
    for (const b of n.el.querySelectorAll('.sdots button')) b.classList.toggle('on', Number(b.dataset.p) === next)
    paint(n)
    setTimeout(() => paint(n), 460)
  }

  // A letter come into one of a stack's (its unread grew): it comes to the
  // top, once (§7).
  function news(before) {
    for (const n of nodes.values()) {
      if (!G.isStack(n.entry)) continue
      const at = G.newsTop(n.entry, before, all)
      if (at >= 0 && at !== topOf(n.entry)) turnTo(n, at)
    }
  }

  // --- Its menu (§7, §10): size; where; stacked or apart; a board; settings; off -----------------
  const menuEl = document.createElement('div')
  menuEl.className = 'wmenu'
  menuEl.hidden = true
  document.body.append(menuEl)

  // Why a size is not there, by kind (the spec's §4, as island.js has it).
  function noSizeOf(w, size, stack = false) {
    if (stack) return t('desk.stackSize')
    const kind = w ? WCards.kindOf(w) : ''
    if (size === 'l' && kind === 'pomodoro') return t('menu.why.pomodoro')
    if (kind === 'countdown') return t(size === 'm' ? 'menu.why.countdownM' : 'menu.why.countdownL')
    return t(size === 'm' ? 'menu.noMedium' : 'menu.noLarge')
  }

  // The nearest of the same size it may stack with, here.
  function neighbour(e) {
    const n = nodes.get(e.id)
    if (!n) return null
    let best = null
    for (const o of nodes.values()) {
      if (o === n || !G.canStack(e, o.entry)) continue
      const d = Math.hypot(o.x - n.x, o.y - n.y)
      if (!best || d < best.d) best = { o, d }
    }
    return best?.o.entry || null
  }
  const nameOfMember = (e, m) => {
    const w = Widgets.sourcesOf(m, all)[0]
    return w ? WCards.titleOf(w, ctx()) : emptyName(e, m, ctx())
  }
  const nameOfEntry = e => nameOfMember(e, G.isStack(e) ? e.items[topOf(e)] : G.memberOf(e))
  // The boards here (three at most in a menu).
  const boardsHere = () => shown().filter(G.isBoard)

  const opt = (act, v, label, on, why = '') => `<button type="button" class="wmo${on ? ' on' : ''}${why ? ' off' : ''}" data-act="${act}" data-v="${v}"${why ? ` aria-disabled="true" data-why="${esc(why)}"` : ''}>${esc(label)}</button>`
  // An item; sub: a small grey line under it (what it does to the widgets).
  const item = (act, label, why = '', v = '', sub = '') => `<button type="button" class="wmi${why ? ' off' : ''}" data-act="${act}" data-v="${esc(v)}"${why ? ` aria-disabled="true" data-why="${esc(why)}"` : ''}>${esc(label)}${sub ? `<span class="wmsub">${esc(sub)}</span>` : ''}</button>`
  const line = (label, html) => `<div class="wmr"><span class="wml">${esc(label)}</span><span class="wms">${html}</span></div>`
  const head = (w, glyph, title) => `<div class="wmh" style="--c:${w ? WCards.colorOf(w, ctx()) : 'currentColor'}">${glyph}<b>${esc(title)}</b></div>`
  const whereLine = () => {
    const notBar = here.home === 'taskbar' ? '' : t('menu.notTaskbar')
    return line(t('menu.where'), opt('where', 'bar', t('menu.bar'), false, notBar) + opt('where', 'isle', t('menu.isle'), false) + opt('where', 'desk', t('menu.desk'), true))
  }

  function menuHTML(e, kind, tile = -1) {
    if (G.isBoard(e)) return tile >= 0 ? tileMenuHTML(e, kind, tile) : boardMenuHTML(e)
    const w = shownWidget(e)
    const stack = G.isStack(e)
    const glyph = w ? WCards.glyphHTML(w, 'wmg') : WCards.memberGlyph(stack ? e.items[0] : G.memberOf(e))
    const title = stack ? `${nameOfEntry(e)} · ${t('desk.inStack', { n: e.items.length })}` : nameOfEntry(e)
    const parts = [head(w, glyph, title)]
    if (kind === 'size' || kind === 'full') {
      const sizes = G.sizesOfEntry(e, all)
      parts.push(line(t('menu.size'), ['s', 'm', 'l'].map(z => opt('size', z, t(`menu.${z}`), e.size === z, sizes.includes(z) ? '' : noSizeOf(w, z, stack))).join('')))
    }
    if (kind === 'more' || kind === 'full') {
      parts.push(whereLine())
      parts.push('<div class="wmsep"></div>')
      if (stack) {
        parts.push(item('takeOut', t('menu.takeOut', { name: nameOfEntry(e) }), '', String(topOf(e))))
        parts.push(item('ungroup', t('menu.ungroup')))
      } else {
        const next = neighbour(e)
        const name = next ? nameOfEntry(next) : ''
        parts.push(next ? item('group', t('menu.stackWith', { name }), '', next.id) : item('group', t('menu.stackWithNone'), t('desk.noNeighbour')))
      }
      // Into a board here (its first empty cell), or a new one where it
      // stands; a stack may not go in (this time round).
      if (stack) parts.push(item('board', t('menu.board'), t('desk.noStackInBoard')))
      else {
        for (const b of boardsHere().slice(0, 3)) parts.push(item('board', t('desk.toBoard', { name: boardLabel(b) }), G.boardFirst(b, e.size) ? '' : t('desk.boardFull'), b.id))
        parts.push(item('board', t('desk.newBoard'), '', 'new'))
      }
      if (w) parts.push(item('settings', t('menu.settings')))
      parts.push(item('remove', t('desk.remove')))
    }
    parts.push('<div class="wmwhy" hidden></div>')
    return parts.join('')
  }

  // A tile's: its size (one with no room for it in the board greyed, why),
  // where, out of the board, its settings, off.
  function tileMenuHTML(b, kind, i) {
    const x = b.items[i]
    const m = G.memberOf(x)
    const w = Widgets.sourcesOf(m, all)[0]
    const parts = [head(w, w ? WCards.glyphHTML(w, 'wmg') : WCards.memberGlyph(m), nameOfMember(b, m))]
    if (kind === 'size' || kind === 'full') {
      const sizes = G.sizesOfEntry({ type: m.type, of: m.of }, all)
      const why = z => (!sizes.includes(z) ? noSizeOf(w, z) : z !== x.size && !G.boardSpot(b, x.at, z, i) ? t('desk.noRoom') : '')
      parts.push(line(t('menu.size'), ['s', 'm', 'l'].map(z => opt('size', z, t(`menu.${z}`), x.size === z, why(z))).join('')))
    }
    if (kind === 'more' || kind === 'full') {
      parts.push(whereLine())
      parts.push('<div class="wmsep"></div>')
      parts.push(item('outBoard', t('desk.outOfBoard')))
      if (w) parts.push(item('settings', t('menu.settings')))
      parts.push(item('remove', t('desk.remove')))
    }
    parts.push('<div class="wmwhy" hidden></div>')
    return parts.join('')
  }

  // A board's own (its More, or a right click on its pane): its name,
  // folded or open, apart (each alone on the desktop), gone (its widgets
  // back in the capsule).
  function boardMenuHTML(b) {
    return [
      `<div class="wmh"><b>${esc(boardLabel(b))}${b.items.length ? ` · ${esc(t('desk.boardCount', { n: b.items.length }))}` : ''}</b></div>`,
      item('rename', t('desk.rename')),
      item('fold', t(b.folded ? 'desk.unfold' : 'desk.fold')),
      '<div class="wmsep"></div>',
      item('apart', t('desk.boardApart'), b.items.length ? '' : t('desk.boardEmpty'), '', t('desk.boardApartSub')),
      item('removeBoard', t('desk.boardRemove'), '', '', t('desk.boardRemoveSub')),
      '<div class="wmwhy" hidden></div>',
    ].join('')
  }

  function openMenu(id, kind, at, tile = -1) {
    const e = find(id)
    if (!e || !here) return
    closeLib()
    for (const n of nodes.values()) {
      n.el.classList.toggle('menu', n.entry.id === id)
      for (const x of n.el.querySelectorAll('.tile')) x.classList.toggle('menu', n.entry.id === id && Number(x.dataset.i) === tile)
    }
    menu = { id, kind, tile }
    menuEl.innerHTML = menuHTML(e, kind, tile)
    fillHer(menuEl)
    menuEl.hidden = false
    popAt(menuEl, at)
    shapes()
  }
  // By the button (under it, its right edge at the button's) or at the
  // pointer; kept inside the display.
  function popAt(pop, at) {
    const w = pop.offsetWidth
    const h = pop.offsetHeight
    let x = at.button ? at.button.right - w : at.x
    let y = at.y
    if (y + h > innerHeight - 8) y = (at.button ? at.button.top - 6 : at.y) - h
    x = Math.max(8, Math.min(x, innerWidth - w - 8))
    y = Math.max(8, Math.min(y, innerHeight - h - 8))
    pop.style.left = `${Math.round(x)}px`
    pop.style.top = `${Math.round(y)}px`
  }
  function closeMenu() {
    if (!menu) return
    menu = null
    menuEl.hidden = true
    for (const n of nodes.values()) {
      n.el.classList.remove('menu')
      for (const x of n.el.querySelectorAll('.tile.menu')) x.classList.remove('menu')
    }
    shapes()
  }
  for (const pop of [menuEl]) {
    pop.addEventListener('mouseover', ev => {
      const why = pop.querySelector('.wmwhy')
      const off = ev.target.closest('[data-why]')
      if (!why) return
      why.hidden = !off
      if (off) why.textContent = off.dataset.why
      shapes()
    })
    pop.addEventListener('contextmenu', ev => ev.preventDefault())
  }
  // While a menu is open the window takes the whole display's desktop
  // (desk.rs poll): a press anywhere but on it closes it, as Windows' own
  // menus do; a press in another window closes it too (desk:outside).
  document.addEventListener(
    'pointerdown',
    ev => {
      if (menu && !menuEl.contains(ev.target) && !ev.target.closest('.tools button, .bbtns button')) closeMenu()
      if (lib && !libEl.contains(ev.target) && !ev.target.closest('.bbtns button')) closeLib()
    },
    true,
  )
  document.addEventListener('contextmenu', ev => {
    if (!ev.target.closest('.dw, .wmenu')) {
      ev.preventDefault()
      closeMenu()
      closeLib()
    }
  })
  menuEl.addEventListener('click', ev => {
    const b = ev.target.closest('[data-act]')
    if (!b || b.classList.contains('off') || !menu) return
    const e = find(menu.id)
    const tile = menu.tile
    closeMenu()
    if (!e) return
    if (G.isBoard(e)) return tile >= 0 ? actTile(e, tile, b.dataset.act, b.dataset.v) : actBoard(e, b.dataset.act)
    act(e, b.dataset.act, b.dataset.v)
  })

  // Each of its widgets or plugins, as the island's placeWidget takes them.
  const itemsOf = e => G.membersOf(e).map(G.itemOf)

  function act(e, what, v) {
    switch (what) {
      case 'size': {
        // The new size where it stands, or the nearest free spot from there.
        const cell = spots.get(e.id) || [0, 0]
        const blocks = others(e.id)
        const at = G.isFree(grid(), cell, v, blocks) ? cell : G.nearestFree(grid(), cell, v, [...blocks, ...icons]) || G.nearestFree(grid(), cell, v, blocks) || cell
        return keepLayout(layout.map(o => (o.id === e.id ? G.placed({ ...o, size: v }, here.key, at, visiting(o)) : o)))
      }
      case 'where':
        if (v === 'desk') return
        // A stack to the taskbar as a stack there; off the desktop by the island's page.
        return ask('place', { items: itemsOf(e), where: v, stack: G.isStack(e) })
      case 'group':
        tops.set(v, 0)
        return keepLayout(G.stackOnto(layout, e.id, v, newId))
      case 'ungroup': {
        const cell = spots.get(e.id)
        const apart = G.ungroup(layout, e.id, newId).map(o => (cell && !layout.some(x => x.id === o.id) ? G.placed(o, here.key, cell) : o))
        tops.delete(e.id)
        return keepLayout(apart)
      }
      case 'takeOut': {
        const cell = spots.get(e.id) || [0, 0]
        const free = G.nearestFree(grid(), cell, e.size, [...others(''), ...icons]) || cell
        tops.set(e.id, 0)
        return keepLayout(G.takeOut(layout, e.id, Number(v) || 0, newId, here.key, free))
      }
      case 'board': {
        if (v === 'new') return keepLayout(G.newBoard(layout, e.id, newId))
        const b = find(v)
        const spot = b && G.boardFirst(b, e.size)
        if (!spot) return shake(nodes.get(v))
        return keepLayout(G.intoBoard(layout, b.id, G.memberOf(e), e.size, spot, newId))
      }
      case 'settings': {
        const w = shownWidget(e)
        return w && ask('settings', { widget: w.id })
      }
      case 'remove':
        return ask('place', { items: itemsOf(e), where: 'isle' })
    }
  }

  // A tile's: another size where it stands (or the nearest room in its
  // board), elsewhere, out of the board beside it, its settings, off.
  function actTile(b, i, what, v) {
    const x = b.items[i]
    if (!x) return
    const m = G.memberOf(x)
    switch (what) {
      case 'size': {
        const spot = G.boardSpot(b, x.at, v, i)
        return spot ? keepLayout(G.moveInBoard(layout, b.id, i, spot, v)) : shake(nodes.get(b.id))
      }
      case 'where':
        if (v === 'desk') return
        return ask('place', { items: [G.itemOf(m)], where: v })
      case 'outBoard': {
        const from = spots.get(b.id) || [0, 0]
        const cell = G.nearestFree(grid(), from, x.size, [...others(''), ...icons]) || G.nearestFree(grid(), from, x.size, others('')) || from
        return keepLayout(G.outOfBoard(layout, b.id, i, newId, here.key, cell))
      }
      case 'settings': {
        const w = Widgets.sourcesOf(m, all)[0]
        return w && ask('settings', { widget: w.id })
      }
      case 'remove':
        return ask('place', { items: [G.itemOf(m)], where: 'isle' })
    }
  }

  function actBoard(b, what) {
    const n = nodes.get(b.id)
    switch (what) {
      case 'rename':
        return n && startRename(n)
      case 'fold':
        return n && fold(n)
      case 'apart':
        return keepLayout(G.boardApart(layout, b.id, newId))
      // Its widgets back in the capsule: on the desktop no more.
      case 'removeBoard':
        return keepLayout(layout.filter(o => o.id !== b.id))
    }
  }

  // --- Its Add: the widgets there are, each where it is now; a press puts it in -----------------
  const libEl = document.createElement('div')
  libEl.className = 'wmenu wlib'
  libEl.hidden = true
  document.body.append(libEl)
  let lib = null

  // The widgets there are, a plugin's once (its first): not those already
  // in this board; each saying where it is (in the capsule, on the taskbar,
  // on the desktop, in a board).
  function libItems(b) {
    const inBoard = new Set(b.items.map(G.keyOf))
    const bar = here.bar && typeof here.bar === 'object' ? Widgets.shownOn(Widgets.monitorAsPart({ left: [], center: [], right: [], ...here.bar }), all) : new Set()
    const monitorPart = here.home === 'taskbar' && (!here.bar || Object.values(here.bar).some(run => Array.isArray(run) && run.includes('monitor')))
    const seen = new Set()
    const out = []
    for (const w of all) {
      const m = w.plugin ? { type: 'plugin', of: w.plugin } : { type: 'widget', of: w.id }
      const key = G.keyOf(m)
      if (seen.has(key) || inBoard.has(key)) continue
      seen.add(key)
      const holder = layout.find(e => G.membersOf(e).some(x => G.keyOf(x) === key) || (w.plugin && G.membersOf(e).some(x => G.keyOf(x) === `widget:${w.id}`)))
      const where = holder ? (G.isBoard(holder) ? t('desk.atBoard', { name: boardLabel(holder) }) : t('desk.atDesk')) : bar.has(w.id) || (w.id === 'monitor' && monitorPart) ? t('desk.atBar') : t('desk.atIsle')
      out.push({ m, w, where, onBar: !holder && (bar.has(w.id) || (w.id === 'monitor' && monitorPart)) })
    }
    return out.slice(0, 40)
  }
  function libHTML(b) {
    const items = libItems(b)
    const rows = items.map((x, i) => {
      const c = WCards.colorOf(x.w, ctx())
      return `<button type="button" class="wli" data-k="${i}" style="--c:${c}">${WCards.glyphHTML(x.w, 'wmg')}<span class="wln">${esc(WCards.titleOf(x.w, ctx()))}</span><span class="wlw">${esc(x.where)}</span></button>`
    })
    return `<div class="wmh"><b>${esc(t('desk.library'))}</b></div>${rows.length ? rows.join('') : `<div class="wlnone">${esc(t('desk.libNone'))}</div>`}`
  }
  function openLib(n, button) {
    closeMenu()
    lib = { id: n.entry.id, items: libItems(n.entry) }
    libEl.innerHTML = libHTML(n.entry)
    fillHer(libEl)
    libEl.hidden = false
    popAt(libEl, { x: button.left, y: button.bottom + 6, button })
    shapes()
  }
  function closeLib() {
    if (!lib) return
    lib = null
    libEl.hidden = true
    shapes()
  }
  libEl.addEventListener('contextmenu', ev => ev.preventDefault())
  libEl.addEventListener('click', ev => {
    const row = ev.target.closest('[data-k]')
    if (!row || !lib) return
    const b = find(lib.id)
    const x = lib.items[Number(row.dataset.k)]
    closeLib()
    if (!b || !x) return
    // Small where it has it; the first empty cell (a row more if need be).
    const sizes = G.sizesOfEntry({ type: x.m.type, of: x.m.of }, all)
    const size = sizes.includes('s') ? 's' : sizes[0] || 's'
    const spot = G.boardFirst(b, size)
    if (!spot) return shake(nodes.get(b.id))
    keepLayout(G.intoBoard(layout, b.id, x.m, size, spot, newId))
    // On the taskbar: off it (one place), by the island's page.
    if (x.onBar) ask('unbar', { items: [G.itemOf(x.m)] })
  })

  // What her island's page does for the desktop (desk.rs desk_ask): a
  // widget's settings opened, a widget put elsewhere, off the taskbar.
  const ask = (what, args) => invoke('desk_ask', { what, args }).catch(() => {})

  // --- From desk.rs -------------------------------------------------------------------------------
  const on = (name, fn) => listen(name, e => fn(e.payload), { target })
  Promise.all([
    on('desk:state', async s => {
      // The icons first, before anything is drawn (the VM, 2026-10-16: the
      // widgets' words came while they were asked, and one come from the
      // other display was drawn over the icons, no longer new here).
      if (!here) await loadIcons()
      here = s
      layout = Array.isArray(s.layout) ? s.layout : []
      document.body.classList.toggle('lw', !!s.light)
      draw()
    }),
    on('desk:widgets', p => {
      const before = all
      all = Array.isArray(p?.widgets) ? p.widgets : []
      draw()
      news(before)
    }),
    on('desk:paper', p => setPaper(p)),
    // The window took the mouse (where the pointer is: the widget under it
    // shown hovered at once, before any move, a board's tile too), or lets
    // it through again.
    on('desk:pointer', at => {
      if (drag) return
      const el = at ? document.elementFromPoint(at.x, at.y) : null
      const under = el?.closest('.dw')
      for (const n of nodes.values()) {
        n.el.classList.toggle('hover', n.el === under)
        hoverTile(n, n.el === under ? el.closest('.tile') : null)
      }
    }),
    // A press elsewhere (on the desktop, another window): the menu goes.
    on('desk:outside', () => {
      closeMenu()
      closeLib()
    }),
  ])
    .then(() => invoke('desk_ready'))
    .catch(err => console.log('desk: listeners failed', err))

  // The page's console into debug.log (when it is on), and /debug/eval.
  const log = console.log.bind(console)
  console.log = (...args) => {
    log(...args)
    invoke('pet_log', { line: args.map(String).join(' ') }).catch(() => {})
  }
  window.__wkEval = (id, fn) => {
    Promise.resolve()
      .then(fn)
      .catch(err => 'error: ' + err.message)
      .then(value => invoke('debug_result', { id, value: value === undefined ? null : value }))
  }
  window.addEventListener('error', e => console.log('error:', e.message, e.filename, e.lineno))
  // For the tests' eyes: what is drawn where.
  window.Desk = {
    state: () => ({ here, layout, spots: Object.fromEntries(spots), icons, paper: !!paper, paper40: !!paper40, shapes: sentShapes }),
    draw,
    turn: (id, by) => nodes.get(id) && turn(nodes.get(id), by),
    openMenu,
    openLib: id => {
      const n = nodes.get(id)
      const b = n?.el.querySelector('[data-bact=add]')
      if (n && b) openLib(n, b.getBoundingClientRect())
    },
    rename: id => nodes.get(id) && startRename(nodes.get(id)),
    shake: id => shake(nodes.get(id)),
    toast: (id, text) => nodes.get(id) && toast(nodes.get(id), text),
    paperAgain: () => lastPaper && setPaper(lastPaper),
    // desk-drop.js: a widget from her home let go of over this display (the
    // picker of 2026-10-17): what is here, and how it is kept.
    inner: {
      get here() {
        return here
      },
      get layout() {
        return layout
      },
      get all() {
        return all
      },
      get icons() {
        return icons
      },
      nodes,
      grid,
      boardUnder,
      boardLabel,
      keepLayout,
      loadIcons,
      shake,
      toast,
      newId,
    },
  }
})()
