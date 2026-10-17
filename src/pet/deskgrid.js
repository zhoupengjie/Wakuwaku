// The desktop's widgets as a layout (the widget design of 2026-10-16, §3,
// §7, §10): where each one stands on a display's grid, which display shows
// it, several stacked into one, and boards holding several as tiles. Plain
// data in, plain data out, for desk.js to draw and the tests to try; desk.rs
// keeps the layout (deskLayout) and settings.rs checks it the same way
// (isEntry).
//
// The grid: cells of 72 px with 12 px gutters (a pitch of 84), from 24 px
// inside the display's work area; small 2 x 2 cells (156 x 156), medium
// 4 x 2 (324 x 156), large 4 x 4 (324 x 324), all in the display's CSS px.
//
// An entry: { id, type: 'widget' | 'plugin', of, size: 's' | 'm' | 'l',
// name?, icon?, d?, at? } alone, or { id, type: 'stack', items, size, d?,
// at? } several in one, 2 to 8, each { type, of, name?, icon? } as the
// taskbar's groups have them (widgets.js isMember), or { id, type: 'board',
// name?, items, folded?, d?, at? } a board (工作台), 0 to 16 tiles, each a
// member with its { size, at: [col, row] } on the board's own grid of 8
// columns and 2 to 4 rows. d: the display it is on (desk.rs key_of); at:
// where it stood on each display it has been on, { key: [col, row] }, so
// each keeps its own (the coordinator's "positions remembered per display").
;(function (root) {
  const Widgets = root.Widgets || require('./widgets')

  const CELL = 72
  const GAP = 12
  const PITCH = CELL + GAP
  const EDGE = 24
  // Cells across and down, by size.
  const SIZES = { s: [2, 2], m: [4, 2], l: [4, 4] }
  const MAX_ENTRIES = 32
  const MAX_MEMBERS = 8
  const MAX_AT = 8

  const cellsOf = size => SIZES[size] || SIZES.s
  const pxOf = n => n * PITCH - GAP
  // A size ('s', 'm', 'l') or a box ({ w, h } in px, a board's): the cells
  // it spans on the display's grid, and its px.
  const isBox = size => !!size && typeof size === 'object'
  const spanOf = size => (isBox(size) ? [Math.ceil((size.w + GAP) / PITCH), Math.ceil((size.h + GAP) / PITCH)] : cellsOf(size))
  const sizePx = size => (isBox(size) ? { w: size.w, h: size.h } : { w: pxOf(cellsOf(size)[0]), h: pxOf(cellsOf(size)[1]) })

  // --- The grid ------------------------------------------------------------------------
  // A display's grid from its work area [x, y, w, h] (the window's CSS px):
  // its first cell 24 px in, as many whole cells as fit with 24 px to spare
  // at the far side too.
  function gridOf(work) {
    const [x, y, w, h] = Array.isArray(work) && work.length === 4 ? work : [0, 0, 0, 0]
    const fit = n => Math.max(0, Math.floor((n - 2 * EDGE + GAP) / PITCH))
    return { x0: x + EDGE, y0: y + EDGE, cols: fit(w), rows: fit(h) }
  }

  // Where a cell is, and a widget of a size (or a board's box) standing on it.
  const rectAt = (grid, [c, r], size) => ({ x: grid.x0 + c * PITCH, y: grid.y0 + r * PITCH, ...sizePx(size) })

  // Whether a widget of a size on this cell is on the grid, whole.
  function fits(grid, [c, r], size) {
    const [cw, ch] = spanOf(size)
    return Number.isInteger(c) && Number.isInteger(r) && c >= 0 && r >= 0 && c + cw <= grid.cols && r + ch <= grid.rows
  }

  // The cell nearest a top-left corner (px), kept on the grid.
  function snap(grid, x, y, size) {
    const [cw, ch] = spanOf(size)
    const c = Math.round((x - grid.x0) / PITCH)
    const r = Math.round((y - grid.y0) / PITCH)
    return [Math.max(0, Math.min(Math.max(0, grid.cols - cw), c)), Math.max(0, Math.min(Math.max(0, grid.rows - ch), r))]
  }

  // Two rects meet (sharing an edge is not meeting).
  const overlaps = (a, b) => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h

  // Free: on the grid, over none of what is there (other widgets, icons).
  const isFree = (grid, cell, size, blocks) => fits(grid, cell, size) && !blocks.some(b => overlaps(rectAt(grid, cell, size), b))

  // The free cell nearest one (by the distance between the two places'
  // corners; the same distance, the higher, then the more to the left);
  // null when there is none.
  function nearestFree(grid, [c0, r0], size, blocks) {
    const [cw, ch] = spanOf(size)
    let best = null
    for (let r = 0; r + ch <= grid.rows; r++)
      for (let c = 0; c + cw <= grid.cols; c++) {
        const d = (c - c0) ** 2 + (r - r0) ** 2
        if (best && (d > best.d || (d === best.d && (r > best.r || (r === best.r && c >= best.c))))) continue
        if (isFree(grid, [c, r], size, blocks)) best = { c, r, d }
      }
    return best ? [best.c, best.r] : null
  }

  // Where a new one goes first: the top right, as the desktop's icons start
  // at the top left.
  const firstSpot = (grid, size) => [Math.max(0, grid.cols - spanOf(size)[0]), 0]

  // Let go of with its top-left here (px): the cell it snaps to, or, that
  // over another or over icons, the nearest free one; with nothing free,
  // where it snapped (over icons sooner than over another widget).
  function dropAt(grid, x, y, size, widgets, icons) {
    const want = snap(grid, x, y, size)
    return nearestFree(grid, want, size, [...widgets, ...icons]) || nearestFree(grid, want, size, widgets) || want
  }

  // --- Which display shows what ---------------------------------------------------------
  // Those this display shows: its own, and on the main display those whose
  // display is not there (gone, or none said yet).
  function shownHere(layout, here) {
    const keys = here.keys || []
    return (Array.isArray(layout) ? layout : []).filter(e => e.d === here.key || (here.primary && (!e.d || !keys.includes(e.d))))
  }

  // Where each of them stands here: where it stood last on this display,
  // unless another took it first or it is off the grid now (the work area
  // changed), or, new on the display (fresh: those not drawn here before,
  // the page just up or one come from another display), it lies over icons;
  // else, and for one never here, the nearest free cell from there (or the
  // top right), clear of the icons. The boards first: one grown by a tile
  // keeps its place, the widgets alone make way. Gives { at: Map(id ->
  // [c, r]), moved: [ids] }: those moved, to keep.
  function settle(entries, here, grid, icons = [], fresh = null) {
    const at = new Map()
    const moved = []
    const taken = []
    for (const e of [...entries.filter(isBoard), ...entries.filter(x => !isBoard(x))]) {
      const size = boxOf(e)
      const was = Array.isArray(e.at?.[here.key]) ? e.at[here.key] : null
      const clear = fresh && fresh.has(e.id) ? [...taken, ...icons] : taken
      let cell = was && isFree(grid, was, size, clear) ? was : null
      if (!cell) {
        const from = was ? snap(grid, rectAt(grid, was, size).x, rectAt(grid, was, size).y, size) : firstSpot(grid, size)
        cell = nearestFree(grid, from, size, [...taken, ...icons]) || nearestFree(grid, from, size, taken) || from
        moved.push(e.id)
      }
      at.set(e.id, cell)
      taken.push(rectAt(grid, cell, size))
    }
    return { at, moved }
  }

  // An entry standing on a cell of a display (and on it now). visiting:
  // shown on the main display while its own is not there, it stays its
  // own display's, to go back there once it is (its place here kept too).
  const placed = (e, key, cell, visiting = false) => ({ ...e, ...(visiting && e.d ? {} : { d: key }), at: keepAt({ ...(e.at || {}), [key]: cell }, key) })
  // Eight displays remembered at most: the one it is on kept, the oldest gone.
  function keepAt(at, key) {
    const keys = Object.keys(at)
    if (keys.length <= MAX_AT) return at
    const drop = keys.filter(k => k !== key).slice(0, keys.length - MAX_AT)
    return Object.fromEntries(keys.filter(k => !drop.includes(k)).map(k => [k, at[k]]))
  }

  // --- Several in one: a stack (§7) ------------------------------------------------------
  const isStack = e => e?.type === 'stack'
  const isBoard = e => e?.type === 'board'
  const memberOf = e => ({ type: e.type, of: e.of, ...(e.name ? { name: e.name } : {}), ...(e.icon ? { icon: e.icon } : {}) })
  const membersOf = e => (isStack(e) ? e.items : isBoard(e) ? e.items.map(memberOf) : e && (e.type === 'widget' || e.type === 'plugin') ? [memberOf(e)] : [])
  const keyOf = m => `${m.type}:${m.of}`
  // A member as the island's placeWidget takes it: a widget ({ id }) or a plugin ({ plugin }).
  const itemOf = m => (m.type === 'plugin' ? { plugin: m.of } : { id: m.of })

  // The sizes a member has: its widgets' (a plugin's, the first's); one
  // with nothing to say yet, any.
  function sizesOfMember(m, all) {
    const ws = Widgets.sourcesOf(m, all)
    return ws.length ? Widgets.sizesOf(ws[0]) : ['s', 'm', 'l']
  }
  // An entry's: those all its members have.
  const sizesOfEntry = (e, all) => membersOf(e).reduce((acc, m) => acc.filter(z => sizesOfMember(m, all).includes(z)), ['s', 'm', 'l'])

  // Whether one may be stacked onto another: the same size, not itself,
  // eight at most in all once none is there twice; never a board, nor onto one.
  function canStack(from, onto) {
    if (!from || !onto || from.id === onto.id || isBoard(from) || isBoard(onto) || from.size !== onto.size) return false
    const keys = new Set(membersOf(onto).map(keyOf))
    const more = membersOf(from).filter(m => !keys.has(keyOf(m)))
    return more.length > 0 && keys.size + more.length <= MAX_MEMBERS
  }

  // One dropped onto another (held there half a second): a stack where the
  // other stood, the one dropped on top (first); one stack onto another,
  // all of its members. The one dropped gone from the layout. Unchanged
  // when they may not stack.
  function stackOnto(layout, fromId, ontoId, newId) {
    const from = layout.find(e => e.id === fromId)
    const onto = layout.find(e => e.id === ontoId)
    if (!canStack(from, onto)) return layout
    const keys = new Set(membersOf(onto).map(keyOf))
    const items = [...membersOf(from).filter(m => !keys.has(keyOf(m))), ...membersOf(onto)]
    const stack = { id: isStack(onto) ? onto.id : newId(), type: 'stack', items, size: onto.size, ...(onto.d ? { d: onto.d } : {}), ...(onto.at ? { at: onto.at } : {}) }
    return layout.filter(e => e.id !== fromId).map(e => (e.id === ontoId ? stack : e))
  }

  // One member of a stack taken out (dragged out of it): alone, where it
  // was let go of; the stack without it (one left, that one alone, where
  // the stack stood).
  function takeOut(layout, stackId, index, newId, key, cell) {
    const s = layout.find(e => e.id === stackId)
    if (!isStack(s) || !s.items[index]) return layout
    const items = s.items.filter((_, i) => i !== index)
    const where = { ...(s.d ? { d: s.d } : {}), ...(s.at ? { at: s.at } : {}) }
    const rest = items.length === 1 ? { id: newId(), ...items[0], size: s.size, ...where } : { ...s, items }
    const out = placed({ id: newId(), ...s.items[index], size: s.size, ...where }, key, cell)
    return layout.flatMap(e => (e.id === stackId ? [rest, out] : [e]))
  }

  // A stack taken apart: each alone, the first where it stood, the others
  // where the display finds room (settle).
  function ungroup(layout, stackId, newId) {
    const s = layout.find(e => e.id === stackId)
    if (!isStack(s)) return layout
    const where = { ...(s.d ? { d: s.d } : {}) }
    const alone = s.items.map((m, i) => ({ id: newId(), ...m, size: s.size, ...where, ...(i === 0 && s.at ? { at: s.at } : {}) }))
    return layout.flatMap(e => (e.id === stackId ? alone : [e]))
  }

  // Turned by the wheel, a drag or a dot: the next, the one before, or one.
  const flip = (top, n, by) => (n ? (((top + by) % n) + n) % n : 0)

  // A member with news comes to the top, once (§7): a letter came (its
  // unread grew, widgets.js newsIn). Its index, or -1.
  function newsTop(stack, before, after) {
    if (!isStack(stack)) return -1
    const was = new Map((before || []).map(w => [w.id, w]))
    return stack.items.findIndex(m => Widgets.sourcesOf(m, after || []).some(w => Widgets.newsIn(was.get(w.id), w)))
  }

  // --- A board (§7 工作台, and its note of 2026-10-16) --------------------------------------------
  // A larger pane, its title bar 46 px, 14 px round its grid of 8 columns of
  // the desktop's cells (688 px across), as many rows as its tiles need, 2
  // (one row of small or medium ones) to 4; folded, its title bar alone.
  // Its widgets are flat tiles on that grid at their own sizes; no stacks.
  const BOARD = { cols: 8, minRows: 2, maxRows: 4, pad: 14, head: 46, width: 8 * PITCH - GAP + 2 * 14 }
  const MAX_TILES = 16
  const BOARD_NAME = 16

  // Its rows: as many as its tiles reach, 2 at least.
  function boardRows(b) {
    const used = Math.max(0, ...(b?.items || []).map(t => t.at[1] + cellsOf(t.size)[1]))
    return Math.min(BOARD.maxRows, Math.max(BOARD.minRows, used))
  }
  // Its box on the desktop (px): the title bar alone while folded.
  const boardBox = b => ({ w: BOARD.width, h: b?.folded ? BOARD.head : BOARD.head + pxOf(boardRows(b)) + BOARD.pad })
  // What an entry takes on the desktop's grid: its size, or a board's box.
  const boxOf = e => (isBoard(e) ? boardBox(e) : e.size)
  // A tile's place in its board (px from the board's top left).
  const tileRect = ([c, r], size) => ({ x: BOARD.pad + c * PITCH, y: BOARD.head + r * PITCH, ...sizePx(size) })

  // On the board's grid, whole, within so many rows.
  function tileFits([c, r], size, rows) {
    const [cw, ch] = cellsOf(size)
    return Number.isInteger(c) && Number.isInteger(r) && c >= 0 && r >= 0 && c + cw <= BOARD.cols && r + ch <= rows
  }
  // Two tiles meet.
  function tilesMeet(a, as, b, bs) {
    const [aw, ah] = cellsOf(as)
    const [bw, bh] = cellsOf(bs)
    return a[0] < b[0] + bw && b[0] < a[0] + aw && a[1] < b[1] + bh && b[1] < a[1] + ah
  }
  // Free: on the grid within the rows, over none of the others (but skip, the one moving).
  const tileFree = (b, cell, size, rows, skip = -1) => tileFits(cell, size, rows) && !b.items.some((t, i) => i !== skip && tilesMeet(cell, size, t.at, t.size))

  // The empty cell nearest one, within so many rows (the same distance, the
  // higher, then the more to the left); null when there is none.
  function tileNearest(b, [c0, r0], size, rows, skip = -1) {
    const [cw, ch] = cellsOf(size)
    let best = null
    for (let r = 0; r + ch <= rows; r++)
      for (let c = 0; c + cw <= BOARD.cols; c++) {
        const d = (c - c0) ** 2 + (r - r0) ** 2
        if (best && (d > best.d || (d === best.d && (r > best.r || (r === best.r && c >= best.c))))) continue
        if (tileFree(b, [c, r], size, rows, skip)) best = { c, r, d }
      }
    return best ? [best.c, best.r] : null
  }
  // Rows it has without the one moving.
  const rowsWithout = (b, skip) => boardRows({ ...b, items: b.items.filter((_, i) => i !== skip) })

  // Where one let go of over a board goes, from the cell under it: the
  // nearest empty cell in the rows it has; none there, a row more and so
  // on, up to four; null when it is full (the board shakes, the widget goes
  // back). skip: a tile of its own moving in it.
  function boardSpot(b, want, size, skip = -1) {
    if (!isBoard(b) || (skip < 0 && b.items.length >= MAX_TILES)) return null
    const rows = rowsWithout(b, skip)
    return tileNearest(b, want, size, rows, skip) || tileNearest(b, want, size, BOARD.maxRows, skip)
  }
  // The first empty cell (its Add's pick): the first in reading order, in
  // the rows it has, else a row more; null when full.
  function boardFirst(b, size) {
    if (!isBoard(b) || b.items.length >= MAX_TILES) return null
    const [cw, ch] = cellsOf(size)
    for (const rows of [boardRows(b), BOARD.maxRows])
      for (let r = 0; r + ch <= rows; r++) for (let c = 0; c + cw <= BOARD.cols; c++) if (tileFree(b, [c, r], size, rows)) return [c, r]
    return null
  }
  // The board's cell a tile's top left (px from the board's) snaps to.
  function tileAt(x, y, size) {
    const [cw, ch] = cellsOf(size)
    const c = Math.round((x - BOARD.pad) / PITCH)
    const r = Math.round((y - BOARD.head) / PITCH)
    return [Math.max(0, Math.min(BOARD.cols - cw, c)), Math.max(0, Math.min(BOARD.maxRows - ch, r))]
  }

  // A widget or plugin put in a board on a cell, at a size: out of wherever
  // else it was on the desktop (its entry gone, or it out of its stack or
  // its other board) first.
  function intoBoard(layout, boardId, m, size, cell, newId) {
    const tile = { ...memberOf(m), size, at: cell }
    return without(layout, itemOf(m), newId).map(e => (e.id === boardId && isBoard(e) ? { ...e, items: [...e.items, tile] } : e))
  }
  // A tile moved in its board, or given another size there.
  function moveInBoard(layout, boardId, index, cell, size) {
    return layout.map(e => (e.id === boardId && isBoard(e) && e.items[index] ? { ...e, items: e.items.map((t, i) => (i === index ? { ...t, at: cell, ...(size ? { size } : {}) } : t)) } : e))
  }
  // A tile taken out (dragged off the board, or by its menu): alone on the
  // desktop, on that cell of a display.
  function outOfBoard(layout, boardId, index, newId, key, cell) {
    const b = layout.find(e => e.id === boardId)
    if (!isBoard(b) || !b.items[index]) return layout
    const t = b.items[index]
    const alone = placed({ id: newId(), ...memberOf(t), size: t.size, ...(b.d ? { d: b.d } : {}) }, key, cell)
    return layout.flatMap(e => (e.id === boardId ? [{ ...e, items: e.items.filter((_, i) => i !== index) }, alone] : [e]))
  }
  // A widget alone made the first tile of a new board, where it stood (a
  // stack may not: no stacks in a board).
  function newBoard(layout, entryId, newId) {
    const e = layout.find(x => x.id === entryId)
    if (!e || isStack(e) || isBoard(e)) return layout
    const board = { id: newId(), type: 'board', items: [{ ...memberOf(e), size: e.size, at: [0, 0] }], ...(e.d ? { d: e.d } : {}), ...(e.at ? { at: e.at } : {}) }
    return layout.map(x => (x.id === entryId ? board : x))
  }
  // A board taken apart: each tile alone on the desktop, the first where
  // the board stood, the others where the display finds room (settle).
  function boardApart(layout, boardId, newId) {
    const b = layout.find(e => e.id === boardId)
    if (!isBoard(b)) return layout
    const alone = b.items.map((t, i) => ({ id: newId(), ...memberOf(t), size: t.size, ...(b.d ? { d: b.d } : {}), ...(i === 0 && b.at ? { at: b.at } : {}) }))
    return layout.flatMap(e => (e.id === boardId ? alone : [e]))
  }

  // --- Kept as settings.rs checks it ------------------------------------------------------
  const isKey = k => typeof k === 'string' && /^[a-z0-9]{1,40}$/.test(k)
  const isCell = v => Array.isArray(v) && v.length === 2 && v.every(n => Number.isInteger(n) && n >= 0 && n <= 255)
  const isAt = at => at && typeof at === 'object' && !Array.isArray(at) && Object.keys(at).length <= MAX_AT && Object.entries(at).every(([k, v]) => isKey(k) && isCell(v))
  const isBoardName = n => typeof n === 'string' && [...n].length <= BOARD_NAME && !/[\u0000-\u001f\u007f]/.test(n)
  const isTile = t =>
    !!t && typeof t === 'object' && !Array.isArray(t) && Object.keys(t).every(k => ['type', 'of', 'name', 'icon', 'size', 'at'].includes(k)) && ['s', 'm', 'l'].includes(t.size) && isCell(t.at) && tileFits(t.at, t.size, BOARD.maxRows) && Widgets.isMember(memberOf(t))
  function isBoardEntry(e) {
    const known = ['id', 'type', 'name', 'items', 'folded', 'd', 'at']
    if (!Object.keys(e).every(k => known.includes(k))) return false
    if (e.name !== undefined && !isBoardName(e.name)) return false
    if (e.folded !== undefined && typeof e.folded !== 'boolean') return false
    if (!Array.isArray(e.items) || e.items.length > MAX_TILES || !e.items.every(isTile)) return false
    if (new Set(e.items.map(keyOf)).size !== e.items.length) return false
    return e.items.every((t, i) => e.items.every((u, j) => j <= i || !tilesMeet(t.at, t.size, u.at, u.size)))
  }
  function isEntry(e) {
    if (!e || typeof e !== 'object' || Array.isArray(e)) return false
    if (typeof e.id !== 'string' || !/^w[a-z0-9]{1,15}$/.test(e.id)) return false
    if (e.d !== undefined && !isKey(e.d)) return false
    if (e.at !== undefined && !isAt(e.at)) return false
    if (isBoard(e)) return isBoardEntry(e)
    if (!['s', 'm', 'l'].includes(e.size)) return false
    const known = ['id', 'type', 'of', 'size', 'name', 'icon', 'd', 'at', 'items']
    if (!Object.keys(e).every(k => known.includes(k))) return false
    if (isStack(e)) {
      return e.of === undefined && e.name === undefined && e.icon === undefined && Array.isArray(e.items) && e.items.length >= 2 && e.items.length <= MAX_MEMBERS && e.items.every(Widgets.isMember) && new Set(e.items.map(keyOf)).size === e.items.length
    }
    return (e.type === 'widget' || e.type === 'plugin') && e.items === undefined && Widgets.isMember(memberOf(e))
  }
  // A layout: 32 at most, each id once, each widget or plugin once.
  function isLayout(layout) {
    if (!Array.isArray(layout) || layout.length > MAX_ENTRIES || !layout.every(isEntry)) return false
    const ids = layout.map(e => e.id)
    const keys = layout.flatMap(membersOf).map(keyOf)
    return new Set(ids).size === ids.length && new Set(keys).size === keys.length
  }

  // Whether the desktop shows a widget ({ id }) or a plugin ({ plugin }).
  function has(layout, item) {
    const key = item.plugin ? `plugin:${item.plugin}` : `widget:${item.id}`
    return (Array.isArray(layout) ? layout : []).some(e => membersOf(e).some(m => keyOf(m) === key))
  }
  // The widgets it shows (not to take their turns in the island too).
  function shownOn(layout, all) {
    const ids = new Set()
    for (const e of Array.isArray(layout) ? layout : []) for (const m of membersOf(e)) for (const w of Widgets.sourcesOf(m, all)) ids.add(w.id)
    return ids
  }
  // A widget or plugin off the desktop: its entry gone, or it out of its
  // stack or its board (the board stays, empty or not).
  function without(layout, item, newId) {
    const key = item.plugin ? `plugin:${item.plugin}` : `widget:${item.id}`
    return (Array.isArray(layout) ? layout : []).flatMap(e => {
      if (isBoard(e)) return e.items.some(t => keyOf(t) === key) ? [{ ...e, items: e.items.filter(t => keyOf(t) !== key) }] : [e]
      if (!isStack(e)) return membersOf(e).some(m => keyOf(m) === key) ? [] : [e]
      const items = e.items.filter(m => keyOf(m) !== key)
      if (items.length === e.items.length) return [e]
      if (items.length === 1) return [{ id: newId(), ...items[0], size: e.size, ...(e.d ? { d: e.d } : {}), ...(e.at ? { at: e.at } : {}) }]
      return [{ ...e, items }]
    })
  }

  // --- A widget in one place (§7's note; island.js takeWidgets) -------------------------------
  // Put on the desktop (alone, stacked, in a board), or on the taskbar while
  // that is her home: out of the closed island, neither taking turns there
  // nor pinned at its end. The rest that are on are her capsule's members
  // (capsule.js: the capsule setting's order and modes, or, without it,
  // the widgets' order and the monitor's pin): those taking turns, those
  // pinned at its end (two at most), those only in the open island. The
  // taskbar her home, the monitor's readings are the strip's monitor part
  // while it has one and its pin is on (#53: taken off it, it is a member
  // as any other). The open island lists the members in their order, then
  // those put elsewhere (where: 'bar' or 'desk', by id). all: the widgets
  // with something to say (on or off); taskbar: her taskbar's layout as
  // island.js tbLayout makes it whole; capsule: the capsule setting.
  function placement({ all = [], home = 'island', taskbar = null, desk = [], pin, capsule } = {}) {
    const C = root.Capsule || (typeof require === 'function' ? require('./capsule') : null)
    const onBar = home === 'taskbar' && taskbar ? Widgets.shownOn(taskbar, all) : new Set()
    const onDesk = shownOn(desk, all)
    const out = new Set([...onBar, ...onDesk])
    const monitor = all.find(w => w.id === 'monitor' && w.on !== false) || null
    const part = !taskbar || Object.values(taskbar).some(run => Array.isArray(run) && run.includes('monitor'))
    const inPart = !!monitor && home === 'taskbar' && part && pin !== false && !out.has('monitor')
    const inCapsule = w => w.on !== false && !out.has(w.id) && !(inPart && w === monitor)
    const members = C.effective({ capsule, all, home, pin, inCapsule })
    const turning = members.filter(m => m.mode === 'turn').flatMap(m => m.widgets)
    const pins = members.filter(m => m.mode === 'pin').map(m => m.widgets[0])
    const pinned = inPart || (home !== 'taskbar' && !!monitor && pins.includes(monitor))
    const elsewhere = all.filter(w => w.on !== false && (out.has(w.id) || (inPart && w === monitor)))
    const listed = [...members.flatMap(m => m.widgets), ...elsewhere]
    const where = new Map(elsewhere.map(w => [w.id, onDesk.has(w.id) ? 'desk' : 'bar']))
    return { out, monitor, pinned, turning, listed, members, pins, where }
  }

  // --- Let go of here from her home (picker.js, desk-drop.js) ---------------------------------
  // From the cell wanted, row by row to the next free one, round to the top
  // again (as Plasma finds a place: not the nearest, the next); null when
  // there is none.
  function nextFree(grid, [c0, r0], size, blocks = []) {
    const [cw, ch] = spanOf(size)
    const cols = grid.cols - cw + 1
    const rows = grid.rows - ch + 1
    if (cols <= 0 || rows <= 0) return null
    const n = cols * rows
    const start = Math.max(0, Math.min(rows - 1, r0)) * cols + Math.max(0, Math.min(cols - 1, c0))
    for (let i = 0; i < n; i++) {
      const j = (start + i) % n
      const cell = [j % cols, Math.floor(j / cols)]
      if (isFree(grid, cell, size, blocks)) return cell
    }
    return null
  }

  // Where one of a size let go of at (x, y) lands, the pointer at its
  // middle: over a board, its first free cell (at tileSize); else the cell
  // under it, or the next free one, clear of the icons if it can be, never
  // over another widget. { ok, where: 'desk' | 'board', cell | spot, want, why }.
  function landing({ grid, x, y, size, blocks = [], icons = [], board = null, tileSize = 's' }) {
    if (board) {
      const spot = boardFirst(board, tileSize)
      return spot ? { ok: true, where: 'board', spot } : { ok: false, where: 'board', why: 'boardFull' }
    }
    const { w, h } = sizePx(size)
    const want = snap(grid, x - w / 2, y - h / 2, size)
    const cell = nextFree(grid, want, size, [...blocks, ...icons]) || nextFree(grid, want, size, blocks)
    return cell ? { ok: true, where: 'desk', cell, want } : { ok: false, where: 'desk', want, why: 'full' }
  }

  // The layout with it there (a member: { type, of, name?, icon? }): out of
  // wherever else it was on the desktop first (one place), then into the
  // board, or alone on the cell of this display (key).
  function dropped(layout, land, { m, size, tileSize = 's', key, boardId, newId }) {
    if (!land?.ok) return layout
    if (land.where === 'board') return intoBoard(layout, boardId, m, tileSize, land.spot, newId)
    return [...without(layout, itemOf(m), newId), placed({ id: newId(), ...memberOf(m), size }, key, land.cell)]
  }

  const api = {
    CELL,
    GAP,
    PITCH,
    EDGE,
    SIZES,
    BOARD,
    MAX_TILES,
    BOARD_NAME,
    sizePx,
    spanOf,
    gridOf,
    rectAt,
    fits,
    snap,
    overlaps,
    isFree,
    nearestFree,
    firstSpot,
    dropAt,
    shownHere,
    settle,
    placed,
    isStack,
    isBoard,
    membersOf,
    memberOf,
    keyOf,
    itemOf,
    sizesOfEntry,
    canStack,
    stackOnto,
    takeOut,
    ungroup,
    flip,
    newsTop,
    boardRows,
    boardBox,
    boxOf,
    tileRect,
    tileFree,
    boardSpot,
    boardFirst,
    tileAt,
    intoBoard,
    moveInBoard,
    outOfBoard,
    newBoard,
    boardApart,
    isBoardName,
    isEntry,
    isLayout,
    has,
    shownOn,
    without,
    placement,
    nextFree,
    landing,
    dropped,
  }

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api
  } else {
    root.DeskGrid = api
  }
})(this)
