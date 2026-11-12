// A widget on the taskbar, opened: a press on one opens its card above it,
// as the clock's opens Windows' calendar; a stack opened (the spec's §5),
// its members side by side.
// Made by island.js in the section's place as the page starts
// (wpopPart(core), imported by island.js): what it takes of the island's core
// comes in core, its functions and constants as they are, its lets read
// and written through core.<name> as they change; what it hands back is the
// island's again.
import * as WCards from './wcards.ts'
import * as Widgets from './widgets.ts'
import { Picker, Settings } from './pages.ts'
import { pet } from './bridge.ts'
import type { WpopCore } from './island.ts'

export function wpopPart(core: WpopCore) {
  const {
    Widgets, barRest, closePop, el, entryNodes, fillHer, home, isDetailed, isSide, memberName, needsSettings,
    popCtx, t, tbLayout, update, watchPresses,
  } = core

  // A press on one opens its card above it, as the clock's opens Windows'
  // calendar: what it says (its large size, wcards.js cardHTML, when it has
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
    if (core.popFor) closePop()
    if (core.imeOpen) core.closeImeList()
    if (core.menuFor) core.closeMenu()
    core.wgFor = id
    core.wgPick = pick
    drawWidgetPop()
    watchPresses()
  }

  function closeWidgetPop() {
    core.wgFor = null
    core.wgPick = null
    wgPop.hidden = true
    watchPresses()
  }

  function drawWidgetPop() {
    const node = core.wgFor && entryNodes.get(core.wgFor)
    const e = node?._entry
    if (!e || !node.isConnected || core.editLayout || home() !== 'taskbar') return closeWidgetPop()
    const detailed = isDetailed()
    const ctx = popCtx()
    // A group's: the one picked, or the one its carousel shows.
    const group = Widgets.isGroup(e)
    const slides = group ? Widgets.slidesOf(e, core.allWidgets) : []
    const shown = group ? (core.wgPick ? slides.find(s => s.w?.id === core.wgPick) : slides[node._at || 0]) : null
    const from = group ? (shown?.w ? [shown.w] : []) : Widgets.sourcesOf(e, core.allWidgets)
    const first = from[0]
    const head = el('div', 'wghead')
    head.innerHTML = first ? WCards.glyphHTML(first, 'wghg') : WCards.memberGlyph(group ? shown?.member || e.items[0] : e)
    head.firstElementChild.style.color = first ? WCards.colorOf(first, ctx) : ''
    const kind = e.type === 'plugin' || first?.plugin ? 'plugin' : first?.from || 'widget'
    const title = first ? WCards.titleOf(first, ctx) : group ? memberName(shown?.member || e.items[0]) : Widgets.nameOf(core.lang, e, core.allWidgets, detailed)
    head.append(el('b', '', title), el('span', 'wgfrom', t(core.lang, `taskbar.w.from.${kind}`)))
    fillHer(head, 18)
    let rows
    if (from.length === 1 && WCards.hasCard(first)) {
      const card = el('div', 'wgcard')
      card.innerHTML = WCards.cardHTML(first, ctx, core.allWidgets)
      fillHer(card, 22)
      rows = [card]
    } else {
      rows = from.map(w => {
        const said = Widgets.words(core.lang, w, detailed)
        const row = el('div', 'wgrow')
        const value = el('b', '', said.value)
        if (Widgets.partsOf(w)) value.innerHTML = Widgets.partsHTML(w)
        // A plugin's several: each by its own label.
        row.append(...(from.length > 1 && said.label ? [el('span', '', said.label)] : []), value)
        return row
      })
    }
    if (!rows.length) rows.push(el('div', 'wgnone', t(core.lang, !group && needsSettings(e) ? 'taskbar.lib.needs' : 'taskbar.w.none')))
    const acts = el('div', 'wgacts')
    const button = (what, label) => {
      const b = el('button', 'wgbtn', label)
      b.type = 'button'
      b.dataset.wg = what
      return b
    }
    if (kind === 'mail') acts.append(button('mail', t(core.lang, 'taskbar.w.mail')))
    if (kind === 'plugin' || kind === 'builtIn') acts.append(button('settings', t(core.lang, 'taskbar.w.settings')))
    acts.append(button('remove', t(core.lang, 'taskbar.w.remove')))
    // Drawn anew only when it changed (the monitor's every 2 s): a button
    // replaced under the pointer would not be pressed.
    const drawn = JSON.stringify([core.wgFor, core.wgPick, from.map(w => [w.id, w.label, w.value, w.detail, w.private]), detailed, core.lang, ctx.light])
    if (drawn !== wgPop._drawn || wgPop.hidden) {
      wgPop._drawn = drawn
      wgPop.replaceChildren(head, ...rows, acts)
    }
    wgPop.classList.toggle('carded', rows[0]?.classList.contains('wgcard'))
    wgPop.hidden = false
    // Over the stack it was picked from, else over it.
    const over = core.stackFor === core.wgFor && !stackPop.hidden ? stackPop : node
    const at = core.besideOrOver(over, wgPop, isSide() ? 8 : 4)
    wgPop.style.left = at.left === undefined ? '' : `${at.left}px`
    wgPop.style.top = at.top === undefined ? '' : `${at.top}px`
    wgPop.classList.toggle('above-stack', over === stackPop)
  }

  // The layout changed by hand (a menu, a stack's line dragged): kept, or
  // into the one being edited.
  function keepLayout(layout) {
    if (core.editLayout) core.editLayout = layout
    else {
      core.config.taskbarLayout = layout
      pet.settings.set({ taskbarLayout: layout })
    }
    core.arranged = ''
    update()
    // picker.js: its cards' "已放" anew.
    Picker?.changed()
  }

  // Off the taskbar, and kept so.
  function removeEntry(id) {
    const layout = tbLayout()
    for (const k of Object.keys(layout)) layout[k] = layout[k].filter(p => Widgets.keyOf(p) !== id)
    if (core.wgFor === id) closeWidgetPop()
    if (core.stackFor === id) closeStackPop()
    keepLayout(layout)
  }

  // One of a group's off the taskbar: the rest stay (one left, alone).
  function removeMember(id, index) {
    closeWidgetPop()
    keepLayout(Widgets.dropMember(tbLayout(), id, index, core.newId))
  }

  wgPop.addEventListener('click', e => {
    const what = e.target.closest('[data-wg]')?.dataset.wg
    const entry = core.wgFor && entryNodes.get(core.wgFor)?._entry
    if (!what || !entry) return
    const group = Widgets.isGroup(entry)
    const w = group ? core.allWidgets.find(x => x.id === (core.wgPick || entryNodes.get(core.wgFor)._slides?.[entryNodes.get(core.wgFor)._at || 0]?.w?.id)) : null
    if (what === 'remove') {
      // picker.js: taken off with a way back (undo), off once it is nowhere.
      const at = group && w ? entry.items.findIndex(m => Widgets.sourcesOf(m, core.allWidgets).includes(w)) : -1
      if (Picker?.remove({ entry: entry.id, member: at })) return closeWidgetPop()
      if (!group) return removeEntry(entry.id)
      const index = w ? entry.items.findIndex(m => Widgets.sourcesOf(m, core.allWidgets).includes(w)) : -1
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
    if (core.popFor) closePop()
    if (core.imeOpen) core.closeImeList()
    if (core.wgFor) closeWidgetPop()
    if (core.menuFor) core.closeMenu()
    core.stackFor = id
    stackOpenedAt = Date.now()
    stackPop._drawn = ''
    drawStackPop()
    watchPresses()
    for (const node of entryNodes.values()) node.classList.toggle('open', node.dataset.entry === id && node._entry?.type === 'stack')
  }

  function closeStackPop() {
    core.stackFor = null
    stackDrag = null
    stackPop.hidden = true
    for (const node of entryNodes.values()) node.classList.remove('open')
    if (core.wgFor && core.wgPick) closeWidgetPop()
    watchPresses()
  }

  function drawStackPop() {
    const node = core.stackFor && entryNodes.get(core.stackFor)
    const e = node?._entry
    if (!e || e.type !== 'stack' || !node.isConnected || core.editLayout || home() !== 'taskbar') return closeStackPop()
    if (stackDrag?.moved) return
    const ctx = popCtx()
    const slides = Widgets.slidesOf(e, core.allWidgets)
    const drawn = JSON.stringify([core.stackFor, slides.map(s => (s.w ? [s.w.id, s.w.label, s.w.value, s.w.detail, s.w.private] : Widgets.memberKey(s.member))), ctx.detailed, ctx.light, core.lang])
    if (drawn !== stackPop._drawn || stackPop.hidden) {
      stackPop._drawn = drawn
      const rows = slides.map(s => {
        const index = e.items.indexOf(s.member)
        const body = s.w
          ? WCards.stackRowHTML(s.w, WCards.titleOf(s.w, ctx), ctx)
          : `<span class="sgl">${WCards.memberGlyph(s.member)}</span><b class="sname">${WCards.esc(memberName(s.member))}</b><span class="sval"></span><span class="ssay">${WCards.esc(t(core.lang, 'taskbar.w.none'))}</span>`
        return `<div class="srow${s.w ? '' : ' empty'}" data-member="${index}" data-widget="${s.w ? WCards.esc(s.w.id) : ''}">${body}</div>`
      })
      stackPop.innerHTML = `<div class="shead"><b>${WCards.esc(t(core.lang, 'taskbar.stack.title'))}</b><span>${WCards.esc(t(core.lang, 'taskbar.stack.count', { n: slides.length }))}</span></div>${rows.join('')}`
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
    const at = core.besideOrOver(node, stackPop, 8)
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
    d.row.dataset.say = d.out ? t(core.lang, 'taskbar.stack.out') : ''
  })
  stackPop.addEventListener('pointerup', e => {
    const d = stackDrag
    stackDrag = null
    if (!d) return
    const id = core.stackFor
    if (!d.moved) return d.widget ? openWidgetPop(id, d.widget) : undefined
    d.row.classList.remove('dragging', 'out')
    d.row.style.transform = ''
    if (d.out) return keepLayout(Widgets.takeOut(tbLayout(), id, d.member, core.newId))
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
    if (row && core.stackFor) core.openMenu({ where: 'bar', entry: core.stackFor, member: Number(row.dataset.member), widget: row.dataset.widget || null }, row)
  })

  return { STACK_CARD_W, closeStackPop, closeWidgetPop, drawStackPop, drawWidgetPop, imePop, keepLayout, openStackPop, openWidgetPop, openWidgetSettings, removeEntry, removeMember, stackPop, wgPop }
}
