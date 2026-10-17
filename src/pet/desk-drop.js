// A widget dragged from her home over this display's desktop (the picker of
// 2026-10-17): picker.js, in her home's window, says where the pointer is
// and when it is let go of; the host forwards it here, in this page's px
// (desk.rs in the app, phase 2; the playground's page in the simulator):
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
;(function () {
  const D = window.Desk?.inner
  if (!D) return
  const { invoke } = window.__TAURI__.core
  const { listen } = window.__TAURI__.event
  const G = window.DeskGrid
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

  function preview(p) {
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

  listen('desk:drag-preview', e => preview(e.payload), { target })
  listen('desk:drop', e => drop(e.payload), { target })
})()
