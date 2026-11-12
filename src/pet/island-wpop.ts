// A widget on the taskbar, opened: a press on one opens its card above it,
// as the clock's opens Windows' calendar; a stack opened (the spec's §5),
// its members side by side.
// Made by island.ts in the section's place as the page starts
// (wpopPart(core), imported by island.ts). The island's core (IslandCore):
// its constants and functions as they are, what it remembers in core.st
// (island-vars.ts, read and written as st.<name>), and the parts made
// before this one in core.pieces (a later one's reached once the page is
// up). What it hands back is the island's again.
import * as WCards from './wcards.ts'
import * as Widgets from './widgets.ts'
import { Picker, Settings } from './pages.ts'
import { pet } from './bridge.ts'
import type { IslandCore } from './island.ts'

export function wpopPart(core: IslandCore) {
  const {
    st, pieces, Widgets, el, home, isDetailed, isSide, newId, t, update,
  } = core
  const { barRest } = pieces.right
  const { entryNodes, fillHer, memberName, needsSettings, popCtx, tbLayout } = pieces.layout
  const { closePop, watchPresses } = pieces.glide

  // A press on one opens its card above it, as the clock's opens Windows'
  // calendar: what it says (its large size, wcards.ts cardHTML, when it has
  // one; each of a plugin's widgets else), where it comes from, and its
  // settings, its mail, or taking it off the taskbar. A carousel's: the one
  // shown; a stack's line pressed: that one's. A press anywhere else closes
  // it (watchPresses). A right press: its menu (openMenu).
  const wgPop = el('div', 'wgpop')
  wgPop.hidden = true
  barRest.append(wgPop)
  // The input methods to pick from (openImeList), as a menu of the windows' kind.
  const imePop = el('div', 'wpop menu imepop')
  imePop.hidden = true
  barRest.append(imePop)
  // A card's width there, and the stack's (the spec: 300).
  const STACK_W = 300
  const STACK_CARD_W = 276

  function openWidgetPop(id, pick = null) {
    if (st.popFor) closePop()
    if (st.imeOpen) pieces.ime.closeImeList()
    if (st.menuFor) pieces.wmenu.closeMenu()
    st.wgFor = id
    st.wgPick = pick
    drawWidgetPop()
    watchPresses()
  }

  function closeWidgetPop() {
    st.wgFor = null
    st.wgPick = null
    wgPop.hidden = true
    watchPresses()
  }

  function drawWidgetPop() {
    const node = st.wgFor && entryNodes.get(st.wgFor)
    const e = node?._entry
    if (!e || !node.isConnected || st.editLayout || home() !== 'taskbar') return closeWidgetPop()
    const detailed = isDetailed()
    const ctx = popCtx()
    // A group's: the one picked, or the one its carousel shows.
    const group = Widgets.isGroup(e)
    const slides = group ? Widgets.slidesOf(e, st.allWidgets) : []
    const shown = group ? (st.wgPick ? slides.find(s => s.w?.id === st.wgPick) : slides[node._at || 0]) : null
    const from = group ? (shown?.w ? [shown.w] : []) : Widgets.sourcesOf(e, st.allWidgets)
    const first = from[0]
    const head = el('div', 'wghead')
    head.innerHTML = first ? WCards.glyphHTML(first, 'wghg') : WCards.memberGlyph(group ? shown?.member || e.items[0] : e)
    head.firstElementChild.style.color = first ? WCards.colorOf(first, ctx) : ''
    const kind = e.type === 'plugin' || first?.plugin ? 'plugin' : first?.from || 'widget'
    const title = first ? WCards.titleOf(first, ctx) : group ? memberName(shown?.member || e.items[0]) : Widgets.nameOf(st.lang, e, st.allWidgets, detailed)
    head.append(el('b', '', title), el('span', 'wgfrom', t(st.lang, `taskbar.w.from.${kind}`)))
    fillHer(head, 18)
    let rows
    if (from.length === 1 && WCards.hasCard(first)) {
      const card = el('div', 'wgcard')
      card.innerHTML = WCards.cardHTML(first, ctx, st.allWidgets)
      fillHer(card, 22)
      rows = [card]
    } else {
      rows = from.map(w => {
        const said = Widgets.words(st.lang, w, detailed)
        const row = el('div', 'wgrow')
        const value = el('b', '', said.value)
        if (Widgets.partsOf(w)) value.innerHTML = Widgets.partsHTML(w)
        // A plugin's several: each by its own label.
        row.append(...(from.length > 1 && said.label ? [el('span', '', said.label)] : []), value)
        return row
      })
    }
    if (!rows.length) rows.push(el('div', 'wgnone', t(st.lang, !group && needsSettings(e) ? 'taskbar.lib.needs' : 'taskbar.w.none')))
    const acts = el('div', 'wgacts')
    const button = (what, label) => {
      const b = el('button', 'wgbtn', label)
      b.type = 'button'
      b.dataset.wg = what
      return b
    }
    if (kind === 'mail') acts.append(button('mail', t(st.lang, 'taskbar.w.mail')))
    if (kind === 'plugin' || kind === 'builtIn') acts.append(button('settings', t(st.lang, 'taskbar.w.settings')))
    acts.append(button('remove', t(st.lang, 'taskbar.w.remove')))
    // Drawn anew only when it changed (the monitor's every 2 s): a button
    // replaced under the pointer would not be pressed.
    const drawn = JSON.stringify([st.wgFor, st.wgPick, from.map(w => [w.id, w.label, w.value, w.detail, w.private]), detailed, st.lang, ctx.light])
    if (drawn !== wgPop._drawn || wgPop.hidden) {
      wgPop._drawn = drawn
      wgPop.replaceChildren(head, ...rows, acts)
    }
    wgPop.classList.toggle('carded', rows[0]?.classList.contains('wgcard'))
    wgPop.hidden = false
    // Over the stack it was picked from, else over it.
    const over = st.stackFor === st.wgFor && !stackPop.hidden ? stackPop : node
    const at = pieces.winlist.besideOrOver(over, wgPop, isSide() ? 8 : 4)
    wgPop.style.left = at.left === undefined ? '' : `${at.left}px`
    wgPop.style.top = at.top === undefined ? '' : `${at.top}px`
    wgPop.classList.toggle('above-stack', over === stackPop)
  }

  // The layout changed by hand (a menu, a stack's line dragged): kept, or
  // into the one being edited.
  function keepLayout(layout) {
    if (st.editLayout) st.editLayout = layout
    else {
      st.config.taskbarLayout = layout
      pet.settings.set({ taskbarLayout: layout })
    }
    st.arranged = ''
    update()
    // picker.ts: its cards' "已放" anew.
    Picker?.changed()
  }

  // Off the taskbar, and kept so.
  function removeEntry(id) {
    const layout = tbLayout()
    for (const k of Object.keys(layout)) layout[k] = layout[k].filter(p => Widgets.keyOf(p) !== id)
    if (st.wgFor === id) closeWidgetPop()
    if (st.stackFor === id) closeStackPop()
    keepLayout(layout)
  }

  // One of a group's off the taskbar: the rest stay (one left, alone).
  function removeMember(id, index) {
    closeWidgetPop()
    keepLayout(Widgets.dropMember(tbLayout(), id, index, newId))
  }

  wgPop.addEventListener('click', e => {
    const what = e.target.closest('[data-wg]')?.dataset.wg
    const entry = st.wgFor && entryNodes.get(st.wgFor)?._entry
    if (!what || !entry) return
    const group = Widgets.isGroup(entry)
    const w = group ? st.allWidgets.find(x => x.id === (st.wgPick || entryNodes.get(st.wgFor)._slides?.[entryNodes.get(st.wgFor)._at || 0]?.w?.id)) : null
    if (what === 'remove') {
      // picker.ts: taken off with a way back (undo), off once it is nowhere.
      const at = group && w ? entry.items.findIndex(m => Widgets.sourcesOf(m, st.allWidgets).includes(w)) : -1
      if (Picker?.remove({ entry: entry.id, member: at })) return closeWidgetPop()
      if (!group) return removeEntry(entry.id)
      const index = w ? entry.items.findIndex(m => Widgets.sourcesOf(m, st.allWidgets).includes(w)) : -1
      return index >= 0 ? removeMember(entry.id, index) : removeEntry(entry.id)
    }
    closeWidgetPop()
    if (what === 'mail') return pet.openSettings('mail')
    openWidgetSettings(w || (entry.type === 'plugin' ? { plugin: entry.of } : { id: entry.of }))
  })

  // A widget's place in the settings: the mail page for an inbox's unread,
  // else its row on the plugins page, its own settings folded out (the
  // monitor's, a plugin's she runs).
  function openWidgetSettings(w) {
    if (w.from === 'mail') return pet.openSettings('mail')
    const key = w.plugin || w.id
    if (w.plugin || key === 'monitor') Settings?.openPlugin?.(key)
    pet.openSettings(key ? `widgets:${key}` : 'widgets')
  }
  wgPop.addEventListener('contextmenu', e => {
    e.preventDefault()
    e.stopPropagation()
  })

  // --- A stack, opened (the spec's §5) ------------------------------------------------
  // A glass card rising from it (300 wide, 8 px over the strip, round it,
  // kept on the screen): a line each, its glyph in a block of its colour,
  // its name, a sentence, its number; rising in 0.32 s, the lines 40 ms
  // apart. Its place keeps its lit ground while it is open. A line dragged
  // up or down goes there; dragged out of it, out of the stack (just after
  // it); pressed, its card. A press on its place again, or anywhere else,
  // closes it.
  const stackPop = el('div', 'wstk')
  stackPop.hidden = true
  barRest.append(stackPop)
  let stackOpenedAt = 0
  // Dragged this far out of it, a line leaves the stack.
  const STACK_OUT_PX = 28

  function openStackPop(id) {
    if (st.popFor) closePop()
    if (st.imeOpen) pieces.ime.closeImeList()
    if (st.wgFor) closeWidgetPop()
    if (st.menuFor) pieces.wmenu.closeMenu()
    st.stackFor = id
    stackOpenedAt = Date.now()
    stackPop._drawn = ''
    drawStackPop()
    watchPresses()
    for (const node of entryNodes.values()) node.classList.toggle('open', node.dataset.entry === id && node._entry?.type === 'stack')
  }

  function closeStackPop() {
    st.stackFor = null
    stackDrag = null
    stackPop.hidden = true
    for (const node of entryNodes.values()) node.classList.remove('open')
    if (st.wgFor && st.wgPick) closeWidgetPop()
    watchPresses()
  }

  function drawStackPop() {
    const node = st.stackFor && entryNodes.get(st.stackFor)
    const e = node?._entry
    if (!e || e.type !== 'stack' || !node.isConnected || st.editLayout || home() !== 'taskbar') return closeStackPop()
    if (stackDrag?.moved) return
    const ctx = popCtx()
    const slides = Widgets.slidesOf(e, st.allWidgets)
    const drawn = JSON.stringify([st.stackFor, slides.map(s => (s.w ? [s.w.id, s.w.label, s.w.value, s.w.detail, s.w.private] : Widgets.memberKey(s.member))), ctx.detailed, ctx.light, st.lang])
    if (drawn !== stackPop._drawn || stackPop.hidden) {
      stackPop._drawn = drawn
      const rows = slides.map(s => {
        const index = e.items.indexOf(s.member)
        const body = s.w
          ? WCards.stackRowHTML(s.w, WCards.titleOf(s.w, ctx), ctx)
          : `<span class="sgl">${WCards.memberGlyph(s.member)}</span><b class="sname">${WCards.esc(memberName(s.member))}</b><span class="sval"></span><span class="ssay">${WCards.esc(t(st.lang, 'taskbar.w.none'))}</span>`
        return `<div class="srow${s.w ? '' : ' empty'}" data-member="${index}" data-widget="${s.w ? WCards.esc(s.w.id) : ''}">${body}</div>`
      })
      stackPop.innerHTML = `<div class="shead"><b>${WCards.esc(t(st.lang, 'taskbar.stack.title'))}</b><span>${WCards.esc(t(st.lang, 'taskbar.stack.count', { n: slides.length }))}</span></div>${rows.join('')}`
      fillHer(stackPop, 16)
    }
    stackPop.hidden = false
    // Rising as it opens, carried on (not begun again) when drawn anew.
    const into = Date.now() - stackOpenedAt
    stackPop.classList.toggle('rising', into < 700)
    if (into < 700) {
      stackPop.style.animationDelay = `-${into}ms`
      for (const [i, row] of [...stackPop.querySelectorAll('.srow')].entries()) row.style.animationDelay = `${i * 40 - into}ms`
    }
    const at = pieces.winlist.besideOrOver(node, stackPop, 8)
    stackPop.style.left = at.left === undefined ? '' : `${at.left}px`
    stackPop.style.top = at.top === undefined ? '' : `${at.top}px`
  }

  // A line held: { row, member, x, y, moved, out }.
  let stackDrag = null
  stackPop.addEventListener('pointerdown', e => {
    const row = e.target.closest('.srow')
    if (!row || e.button !== 0) return
    stackDrag = { row, member: Number(row.dataset.member), widget: row.dataset.widget, x: e.clientX, y: e.clientY, moved: false, out: false }
    try {
      stackPop.setPointerCapture(e.pointerId)
    } catch {}
  })
  stackPop.addEventListener('pointermove', e => {
    const d = stackDrag
    if (!d) return
    if (!d.moved && Math.hypot(e.clientX - d.x, e.clientY - d.y) < 5) return
    d.moved = true
    d.row.classList.add('dragging')
    d.row.style.transform = `translate(${e.clientX - d.x}px, ${e.clientY - d.y}px)`
    const r = stackPop.getBoundingClientRect()
    d.out = e.clientX < r.left - STACK_OUT_PX || e.clientX > r.right + STACK_OUT_PX || e.clientY < r.top - STACK_OUT_PX || e.clientY > r.bottom + STACK_OUT_PX
    d.row.classList.toggle('out', d.out)
    d.row.dataset.say = d.out ? t(st.lang, 'taskbar.stack.out') : ''
  })
  stackPop.addEventListener('pointerup', e => {
    const d = stackDrag
    stackDrag = null
    if (!d) return
    const id = st.stackFor
    if (!d.moved) return d.widget ? openWidgetPop(id, d.widget) : undefined
    d.row.classList.remove('dragging', 'out')
    d.row.style.transform = ''
    if (d.out) return keepLayout(Widgets.takeOut(tbLayout(), id, d.member, newId))
    // Before the line it is let go over (after it past its middle).
    const rows = [...stackPop.querySelectorAll('.srow')].filter(r => r !== d.row)
    const below = rows.find(r => {
      const b = r.getBoundingClientRect()
      return e.clientY < b.top + b.height / 2
    })
    const members = [...new Set(rows.map(r => Number(r.dataset.member)))]
    const to = below ? members.indexOf(Number(below.dataset.member)) : members.length
    const order = [...members.slice(0, to), d.member, ...members.slice(to)]
    const at = order.indexOf(d.member)
    if (at !== d.member) {
      stackPop._drawn = ''
      keepLayout(Widgets.reorder(tbLayout(), id, d.member, at))
    } else drawStackPop()
  })
  stackPop.addEventListener('pointercancel', () => {
    if (stackDrag) {
      stackDrag.row.classList.remove('dragging', 'out')
      stackDrag.row.style.transform = ''
    }
    stackDrag = null
  })
  stackPop.addEventListener('contextmenu', e => {
    e.preventDefault()
    e.stopPropagation()
    const row = e.target.closest('.srow')
    if (row && st.stackFor) pieces.wmenu.openMenu({ where: 'bar', entry: st.stackFor, member: Number(row.dataset.member), widget: row.dataset.widget || null }, row)
  })

  return { STACK_CARD_W, closeStackPop, closeWidgetPop, drawStackPop, drawWidgetPop, imePop, keepLayout, openStackPop, openWidgetPop, openWidgetSettings, removeEntry, removeMember, stackPop, wgPop }
}
