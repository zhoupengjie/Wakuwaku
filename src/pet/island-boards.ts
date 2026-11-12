// Boards on the desktop (§7 工作台): widgets and stacks sent from the island
// or the taskbar to the desktop (deskLayout), gathered on a board, and back.
// Made by island.ts in the section's place as the page starts
// (boardsPart(core), imported by island.ts). The island's core (IslandCore):
// its constants and functions as they are, what it remembers in core.st
// (island-vars.ts, read and written as st.<name>), and the parts made
// before this one in core.pieces (a later one's reached once the page is
// up). What it hands back is the island's again.
import * as DeskGrid from './deskgrid.ts'
import * as Widgets from './widgets.ts'
import { pet } from './bridge.ts'
import type { IslandCore } from './island.ts'
import type { WidgetItem } from './island-vars.ts'
import type { Board, Entry as DeskEntry } from './deskgrid.ts'
import type { Entry, Widget } from './widgets.ts'

export function boardsPart(core: IslandCore) {
  const {
    st, pieces, Widgets, home, newId, t, takeWidgets, update,
  } = core
  const { tbLayout } = pieces.layout
  const { closeStackPop, keepLayout, removeEntry, removeMember } = pieces.wpop
  const { offBar, placeWidget } = pieces.wmenu

  // Their names as the desktop's pages give them: their own, else 工作台,
  // numbered when several have none.
  const deskBoards = () => (Array.isArray(st.config.deskLayout) ? st.config.deskLayout : []).filter(DeskGrid.isBoard)
  function boardName(b: Board) {
    if (b.name) return b.name
    const unnamed = deskBoards().filter(x => !x.name)
    return unnamed.length > 1 ? t(st.lang, 'desk.boardN', { n: unnamed.findIndex(x => x.id === b.id) + 1 }) : t(st.lang, 'desk.board')
  }
  // The size a widget takes in a board: small where it has it.
  function boardSize(item: WidgetItem) {
    const m = item.plugin ? { type: 'plugin', of: item.plugin } : { type: 'widget', of: item.id }
    const sizes = DeskGrid.sizesOfEntry(m, st.allWidgets)
    return sizes.includes('s') ? 's' : sizes[0] || 's'
  }
  // Into a board (its first empty cell), or a new board with it: off the
  // taskbar and wherever else it was on the desktop first (one place).
  function toBoard(item: WidgetItem, boardId: string) {
    const old = Array.isArray(st.config.deskLayout) ? st.config.deskLayout : []
    // An item is its plugin's, else its own by its id.
    const m = item.plugin ? { type: 'plugin', of: item.plugin } : { type: 'widget', of: item.id! }
    const size = boardSize(item)
    const board = old.find(e => e.id === boardId && DeskGrid.isBoard(e))
    let next: DeskEntry[]
    if (board) {
      const spot = DeskGrid.boardFirst(board, size)
      if (!spot) return
      next = DeskGrid.intoBoard(old, board.id, m, size, spot, newId)
    } else {
      next = [...DeskGrid.without(old, item, newId), { id: newId(), type: 'board', items: [{ ...m, size, at: [0, 0] }] }]
    }
    offBar([item])
    keepDeskLayout(next)
  }
  // A whole desktop layout kept as what changed in it (desk.rs desk_keep
  // merges it with the pages'; a new board goes to the pointer's display).
  function keepDeskLayout(next: DeskEntry[]) {
    const old = Array.isArray(st.config.deskLayout) ? st.config.deskLayout : []
    const was = new Map(old.map(e => [e.id, JSON.stringify(e)]))
    const ids = new Set(next.map(e => e.id))
    st.config.deskLayout = next
    pet.desk.keep(
      next.filter(e => was.get(e.id) !== JSON.stringify(e)),
      old.filter(e => !ids.has(e.id)).map(e => e.id),
    )
    takeWidgets(st.allWidgets)
    update()
  }

  // On the desktop (desk.rs, deskgrid.ts): medium where it has it; where it
  // stands the desktop's page says (the display the pointer is on, the top
  // right, clear of the icons).
  function putOnDesk(item: WidgetItem) {
    if (DeskGrid.has(st.config.deskLayout, item)) return
    const w = item.plugin ? st.allWidgets.find(x => x.plugin === item.plugin) : st.allWidgets.find(x => x.id === item.id)
    const sizes = w ? Widgets.sizesOf(w) : ['s', 'm']
    const source = item.plugin ? { type: 'plugin', of: item.plugin } : { type: 'widget', of: item.id }
    const label = w && !w.private && !item.plugin ? [...String(Widgets.words(st.lang, w).label)].slice(0, 40).join('') : ''
    const icon = (w?.icon || item.icon) in Widgets.ICONS ? { icon: w?.icon || item.icon } : {}
    keepDesk([{ id: newId(), ...source, size: sizes.includes('m') ? 'm' : 's', ...icon, ...(label ? { name: label } : {}) }])
  }
  function keepDesk(entries: DeskEntry[]) {
    st.config.deskLayout = [...(Array.isArray(st.config.deskLayout) ? st.config.deskLayout : []), ...entries]
    pet.desk.keep(entries, [])
    takeWidgets(st.allWidgets)
    update()
  }
  function deskOff(items: WidgetItem[]) {
    let next = Array.isArray(st.config.deskLayout) ? st.config.deskLayout : []
    for (const it of items) next = DeskGrid.without(next, it, newId)
    st.config.deskLayout = next
    pet.settings.set({ deskLayout: next })
  }
  // From the strip or the capsule to the desktop: a group as a stack there
  // (medium where all of it has that), one alone as itself.
  function toDesk(w: Widget | null, entry: Entry | null, member: number) {
    if (entry && Widgets.isGroup(entry) && member < 0) {
      const items = entry.items.map(m => ({ type: m.type, of: m.of, ...(m.name ? { name: m.name } : {}), ...(m.icon ? { icon: m.icon } : {}) }))
      const sizes = DeskGrid.sizesOfEntry({ type: 'stack', items }, st.allWidgets)
      if (st.stackFor === entry.id) closeStackPop()
      removeEntry(entry.id)
      return keepDesk([{ id: newId(), type: 'stack', items, size: sizes.includes('m') ? 'm' : sizes[0] || 's' }])
    }
    const m = entry ? (Widgets.isGroup(entry) ? entry.items[member] : entry) : null
    const item = m ? (m.type === 'plugin' ? { plugin: m.of } : { id: m.of }) : w?.plugin ? { plugin: w.plugin } : w ? { id: w.id } : null
    if (item) placeWidget(item, 'desk')
  }
  // A desktop page's asking (desk.rs desk_ask): a widget's settings, or
  // widgets put elsewhere; a stack onto the taskbar as a stack there.
  function deskPlace({ items = [], where, stack }: { items?: { id?: string; plugin?: string }[]; where?: string; stack?: boolean } = {}) {
    // None is in neither.
    if (!['bar', 'isle'].includes(where as string) || !items.length) return
    if (where === 'bar' && home() !== 'taskbar') return
    if (where === 'bar' && stack && items.length >= 2) {
      deskOff(items)
      const members = items.slice(0, 8).map(it => (it.plugin ? { type: 'plugin', of: it.plugin } : { type: 'widget', of: it.id }))
      const layout = tbLayout()
      layout.right.unshift({ id: newId(), type: 'stack', items: members })
      return keepLayout(layout)
    }
    for (const it of items) placeWidget(it, where!)
  }

  // Back in the island: off the strip (out of its group), and on there.
  function backToIsland(w: Widget | null, entry: Entry, member: number) {
    if (Widgets.isGroup(entry) && member >= 0) removeMember(entry.id, member)
    else removeEntry(entry.id)
    const id = w?.id || (entry.type === 'widget' ? entry.of : null)
    if (id && (st.config.widgetsOff || []).includes(id)) pet.settings.set({ widgetsOff: st.config.widgetsOff!.filter(x => x !== id) })
  }

  return { backToIsland, boardName, boardSize, deskBoards, deskOff, deskPlace, keepDesk, keepDeskLayout, putOnDesk, toBoard, toDesk }
}
