// A widget dragged from her home over this display's desktop (the picker of
// 2026-10-17): picker.js, in her home's window, says where the pointer is
// and when it is let go of; the host forwards it here, in this page's px
// (desk.rs in the app, to the display under the pointer alone; the
// playground's page in the simulator):
//   desk:drag-preview  { x, y, item, size } | null   where it would land
//   desk:drop          { x, y, item, size, ticket }  let go of
// item: the member to put here ({ type, of, name?, icon? }); size: its size
// alone on the grid ('s', 'm'). This page does the grid's sums (deskgrid.js
// landing): over a board, its first free cell; else the cell under the
// pointer (the widget's middle there), or the next free one row by row,
// clear of the icons where it can be, never over another widget; and keeps
// it as its own drops are kept (desk.js keepLayout: desk_keep). What it
// found goes back to her home's page (desk_drag_answer → island:desk-answer:
// { ticket, ok, where, why, name }; ticket null for a preview's), for the
// ghost's words there and, once let go, for the strip to let it go (a move).
//
// A combo (the picker's 组合, settings boardSets) comes as { x, y, set:
// { id, name, items }, ticket? } in place of item and size: the board it
// was, put up again with its middle under the pointer (or the next free
// place for its box), never into another board; its widgets out of
// wherever else they stand here first (one place each), those alone here
// not in its way. The answer's name is the combo's. desk.rs forwards the
// payload as it is (set as well as item).
//
// A display with nothing on it has its window made as the drag reaches it
// (desk.rs): what comes before this page knows its display (desk:state,
// its icons read first) waits for it, the last preview and the drop.
import * as G from './deskgrid.js'
import { Desk } from './pages.js'

// Once desk.js has made the desktop (its inner parts, D).
const D = Desk?.inner
if (D) deskDrop()

function deskDrop() {
  const { invoke } = window.__TAURI__.core
  const { listen } = window.__TAURI__.event
  const LABEL = window.__TAURI_INTERNALS__?.metadata?.currentWebview?.label || ''
  const target = { kind: 'WebviewWindow', label: LABEL }

  // Where it would land (dashed, in Windows' accent; amber where it may
  // not), and the grid's cells faintly under it.
  const cells = document.createElement('div')
  cells.className = 'dcells'
  cells.hidden = true
  const land = document.createElement('div')
  land.className = 'dland'
  land.hidden = true
  document.body.append(cells, land)
  let saidLast = ''
  let lit = null
  let iconsAsked = false

  const keyOf = m => `${m.type}:${m.of}`
  // The size it takes in a board: small where it has it.
  const tileSizeOf = m => {
    const sizes = G.sizesOfEntry({ type: m.type, of: m.of }, D.all)
    return sizes.includes('s') ? 's' : sizes[0] || 's'
  }

  // A combo's board: its box landed as one alone is, over none of those here
  // but the ones that go into it (alone here, every member of theirs in it).
  function setLanding(p) {
    const grid = D.grid()
    const { board } = G.boardFromSet(p.set, { newId: D.newId })
    if (!board) return null
    const keys = new Set(board.items.map(keyOf))
    const goes = e => !G.isBoard(e) && G.membersOf(e).every(m => keys.has(keyOf(m)))
    const blocks = [...D.nodes.values()].filter(n => !goes(n.entry)).map(n => n.rect)
    const box = G.boardBox(board)
    return { grid, board, box, land: G.landing({ grid, x: p.x, y: p.y, size: box, blocks, icons: D.icons }) }
  }

  function landingAt(p) {
    if (!D.here || !p?.item) return null
    const grid = D.grid()
    const key = keyOf(p.item)
    const board = D.boardUnder(p.x, p.y, null)
    // Those here but the one it is (alone here already: it moves).
    const blocks = [...D.nodes.values()].filter(n => G.isBoard(n.entry) || G.membersOf(n.entry).length !== 1 || keyOf(G.membersOf(n.entry)[0]) !== key).map(n => n.rect)
    const tileSize = tileSizeOf(p.item)
    const size = ['s', 'm', 'l'].includes(p.size) ? p.size : 'm'
    return { grid, board, tileSize, size, land: G.landing({ grid, x: p.x, y: p.y, size, blocks, icons: D.icons, board: board?.entry || null, tileSize }) }
  }

  function clear() {
    cells.hidden = true
    land.hidden = true
    if (lit) {
      lit.el.classList.remove('dhot', 'dno')
      lit.el.querySelector('.bghost')?.remove()
      lit = null
    }
  }

  function answer(said) {
    invoke('desk_drag_answer', { said }).catch(() => {})
  }

  // Where a combo's board would stand: dashed as one alone's place.
  function previewSet(p) {
    const r = D.here ? setLanding(p) : null
    if (lit) clear()
    if (!r) {
      clear()
      return
    }
    const { grid, box, land: l } = r
    const at = G.rectAt(grid, l.cell || l.want, box)
    Object.assign(land.style, { left: `${at.x}px`, top: `${at.y}px`, width: `${at.w}px`, height: `${at.h}px` })
    land.classList.toggle('no', !l.ok)
    land.hidden = false
    Object.assign(cells.style, { left: `${grid.x0}px`, top: `${grid.y0}px`, width: `${Math.max(0, grid.cols * G.PITCH - G.GAP)}px`, height: `${Math.max(0, grid.rows * G.PITCH - G.GAP)}px` })
    cells.hidden = false
    const said = { ticket: null, ok: l.ok, where: 'desk', why: l.why || '', name: p.set.name || '' }
    const was = saidLast
    saidLast = JSON.stringify(said)
    if (saidLast !== was) answer(said)
  }
  function dropSet(p) {
    const r = D.here ? setLanding(p) : null
    clear()
    saidLast = ''
    iconsAsked = false
    const ticket = p?.ticket ?? ''
    if (!r) return answer({ ticket, ok: false, where: 'desk', why: 'none' })
    if (!r.land.ok) return answer({ ticket, ok: false, where: 'desk', why: r.land.why || '' })
    D.keepLayout(G.withBoard(D.layout, r.board, { key: D.here.key, cell: r.land.cell, newId: D.newId }))
    answer({ ticket, ok: true, where: 'desk', why: '', name: p.set.name || '', board: r.board.id })
  }

  function preview(p) {
    if (p?.set) {
      if (!iconsAsked) {
        iconsAsked = true
        D.loadIcons()
      }
      return previewSet(p)
    }
    const r = p ? landingAt(p) : null
    if (!r) {
      clear()
      saidLast = ''
      return
    }
    if (!iconsAsked) {
      iconsAsked = true
      D.loadIcons()
    }
    const { grid, board, land: l } = r
    if (lit && lit !== board) clear()
    if (l.where === 'board') {
      land.hidden = true
      cells.hidden = true
      lit = board
      board.el.classList.toggle('dhot', l.ok)
      board.el.classList.toggle('dno', !l.ok)
      if (l.ok && !board.entry.folded) {
        let ghost = board.el.querySelector('.bghost')
        if (!ghost) {
          ghost = document.createElement('div')
          ghost.className = 'bghost'
          board.el.querySelector('.db')?.append(ghost)
        }
        const t = G.tileRect(l.spot, r.tileSize)
        Object.assign(ghost.style, { left: `${t.x}px`, top: `${t.y}px`, width: `${t.w}px`, height: `${t.h}px` })
      }
    } else {
      const at = G.rectAt(grid, l.cell || l.want, r.size)
      Object.assign(land.style, { left: `${at.x}px`, top: `${at.y}px`, width: `${at.w}px`, height: `${at.h}px` })
      land.classList.toggle('no', !l.ok)
      land.hidden = false
      Object.assign(cells.style, { left: `${grid.x0}px`, top: `${grid.y0}px`, width: `${Math.max(0, grid.cols * G.PITCH - G.GAP)}px`, height: `${Math.max(0, grid.rows * G.PITCH - G.GAP)}px` })
      cells.hidden = false
    }
    const said = { ticket: null, ok: l.ok, where: l.where, why: l.why || '', name: board ? D.boardLabel(board.entry) : '' }
    const was = saidLast
    saidLast = JSON.stringify(said)
    if (saidLast !== was) answer(said)
  }

  function drop(p) {
    if (p?.set) return dropSet(p)
    const r = landingAt(p)
    clear()
    saidLast = ''
    iconsAsked = false
    const ticket = p?.ticket ?? ''
    if (!r) return answer({ ticket, ok: false, where: 'desk', why: 'none' })
    const { land: l, board } = r
    if (!l.ok) {
      if (board) D.shake(board)
      return answer({ ticket, ok: false, where: l.where, why: l.why || '' })
    }
    D.keepLayout(G.dropped(D.layout, l, { m: p.item, size: r.size, tileSize: r.tileSize, key: D.here.key, boardId: board?.entry.id, newId: D.newId }))
    answer({ ticket, ok: true, where: l.where, why: '', name: board ? D.boardLabel(board.entry) : '' })
  }

  // Before this page knows its display: the last preview and a drop kept,
  // looked at again every 50 ms for 3 s (a drop then says none).
  let waiting = null
  function whenHere(kind, p) {
    if (D.here) return kind === 'drop' ? drop(p) : preview(p)
    if (!waiting) waiting = { preview: null, drop: null, tries: 0 }
    waiting[kind] = p
    if (waiting.timer) return
    const look = () => {
      const w = waiting
      if (D.here || ++w.tries > 60) {
        waiting = null
        if (w.drop) return drop(w.drop)
        return D.here && w.preview && preview(w.preview)
      }
      w.timer = setTimeout(look, 50)
    }
    waiting.timer = setTimeout(look, 50)
  }

  listen('desk:drag-preview', e => whenHere('preview', e.payload), { target })
  listen('desk:drop', e => whenHere('drop', e.payload), { target })
}
