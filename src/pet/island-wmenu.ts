// A widget's menu (the spec's §10): a right press on a widget, on the strip
// or in the open island; its sizes, where it is shown (the island, the
// taskbar, the desktop), its settings, taken off.
// Made by island.ts in the section's place as the page starts
// (wmenuPart(core), imported by island.ts): what it takes of the island's core
// comes in core, its functions and constants as they are, its lets read
// and written through core.<name> as they change; what it hands back is the
// island's again.
import * as DeskGrid from './deskgrid.ts'
import * as Placing from './placing.ts'
import * as WCards from './wcards.ts'
import * as Widgets from './widgets.ts'
import { Picker } from './pages.ts'
import { pet } from './bridge.ts'
import type { WmenuCore } from './island.ts'

export function wmenuPart(core: WmenuCore) {
  const {
    Widgets, barRest, closePop, closeStackPop, closeWidgetPop, el, entryNodes, fillHer, hasMonitorPart, home,
    isDetailed, island, isleCtx, keepLayout, memberName, openWidgetSettings, ownName, placedAt, popCtx,
    removeEntry, removeMember, rowSize, setRowSize, showCard, stackPop, t, tbLayout, update, watchPresses,
  } = core

  // A right press on a widget, on the strip or in the open island: its size
  // (small, medium, large; one it has not greyed, why under the pointer),
  // where it goes (the taskbar, the island; the desktop to come), with the
  // one beside it in one place (a stack, a carousel) or apart again, its
  // settings, off. menuFor: { where: 'bar' | 'isle', entry, member, widget }.
  const wmenu = el('div', 'wmenu')
  wmenu.hidden = true

  // What a menu is for: its widget (the one picked, else the one shown),
  // the entry on the strip it is part of, and which of a group's it is.
  // On a group's place itself: the group (member -1), the widget it shows
  // and that one's member (shown) to take out.
  function menuTarget(m) {
    const node = m.entry ? entryNodes.get(m.entry) : null
    const entry = node?._entry || null
    const group = !!entry && Widgets.isGroup(entry)
    let w = m.widget ? core.allWidgets.find(x => x.id === m.widget) || null : null
    if (!w && entry) w = group ? node._slides?.[node._at || 0]?.w || null : Widgets.sourcesOf(entry, core.allWidgets)[0] || null
    const memberOf = x => (x ? entry.items.findIndex(mm => Widgets.sourcesOf(mm, core.allWidgets).includes(x)) : -1)
    const member = group ? (m.member ?? (m.widget ? memberOf(w) : -1)) : -1
    const slide = group && member < 0 ? node._slides?.[node._at || 0] : null
    const shown = slide ? entry.items.findIndex(x => Widgets.memberKey(x) === Widgets.memberKey(slide.member)) : -1
    return { node, entry, w, member, shown }
  }

  // The widget or group next to one in its run (after it, else before it).
  function neighbourOf(id) {
    const layout = tbLayout()
    const at = Widgets.findEntry(layout, id)
    if (!at) return null
    const run = layout[at.run]
    const fits = p => p && typeof p !== 'string' && (Widgets.isSource(p) || Widgets.isGroup(p))
    return [run[at.at + 1], run[at.at - 1]].find(fits) || null
  }

  // Why a size is not there, by kind (the spec's §4).
  function noSize(w, size) {
    const kind = WCards.kindOf(w)
    if (size === 'L' && kind === 'pomodoro') return t(core.lang, 'menu.why.pomodoro')
    if (kind === 'countdown') return t(core.lang, size === 'M' ? 'menu.why.countdownM' : 'menu.why.countdownL')
    return t(core.lang, size === 'M' ? 'menu.noMedium' : 'menu.noLarge')
  }

  function menuHTML(m) {
    const { node, entry, w: shownW, member, shown } = menuTarget(m)
    const esc = WCards.esc
    const group = !!entry && Widgets.isGroup(entry)
    // A group's own menu speaks for the group, not the one it shows.
    const w = group && member < 0 ? null : shownW
    const bar = m.where === 'bar'
    const ctx = bar ? popCtx() : isleCtx()
    const opt = (act, v, label, on, why = '') => `<button type="button" class="wmo${on ? ' on' : ''}${why ? ' off' : ''}" data-act="${act}" data-v="${v}"${why ? ` aria-disabled="true" title="${esc(why)}" data-why="${esc(why)}"` : ''}>${esc(label)}</button>`
    const item = (act, label, why = '', v = '') => `<button type="button" class="wmi${why ? ' off' : ''}" data-act="${act}" data-v="${esc(v)}"${why ? ` aria-disabled="true" title="${esc(why)}" data-why="${esc(why)}"` : ''}>${esc(label)}</button>`
    const line = (label, html) => `<div class="wmr"><span class="wml">${esc(label)}</span><span class="wms">${html}</span></div>`
    const name = w ? WCards.titleOf(w, ctx) : group && member < 0 ? Widgets.nameOf(core.lang, entry, core.allWidgets, isDetailed()) : entry ? memberName(group ? entry.items[member] : entry) : ''
    const glyph = w ? WCards.glyphHTML(w, 'wmg') : entry ? WCards.memberGlyph(group ? entry.items[Math.max(0, member)] : entry) : ''
    const parts = [`<div class="wmh" style="--c:${w ? WCards.colorOf(w, ctx) : 'currentColor'}">${glyph}<b>${esc(name)}</b></div>`]
    const sizes = w ? Widgets.sizesOf(w) : ['s', 'm']
    // Its size: on the strip S or M (never L: its card opens by a press);
    // in the island its row S or M, and L its card unfolded.
    if (!(group && member < 0)) {
      if (bar && !group) {
        const now = node?.dataset.size === 'S' || node?.classList.contains('S') ? 'S' : 'M'
        parts.push(line(t(core.lang, 'menu.size'), opt('size', 'S', t(core.lang, 'menu.s'), now === 'S') + opt('size', 'M', t(core.lang, 'menu.m'), now === 'M', sizes.includes('m') || !w ? '' : noSize(w, 'M')) + opt('size', 'L', t(core.lang, 'menu.l'), false, t(core.lang, 'menu.noLargeBar'))))
      } else if (!bar && w) {
        const now = rowSize(w)
        const open = core.cardFor === w.id
        parts.push(line(t(core.lang, 'menu.size'), opt('size', 'S', t(core.lang, 'menu.s'), now === 'S' && !open) + opt('size', 'M', t(core.lang, 'menu.m'), now === 'M' && !open, sizes.includes('m') ? '' : noSize(w, 'M')) + opt('size', 'L', t(core.lang, 'menu.l'), open, WCards.hasCard(w) ? '' : noSize(w, 'L'))))
      }
    }
    // Where it goes: the taskbar (only where she lives there), the island, the desktop.
    // Where it is now (one place, §7): the strip's own on the strip, her
    // capsule's in the open island (it lists those alone).
    if (w || entry) {
      const place = entry ? 'bar' : placeOf(w)
      const notBar = home() === 'taskbar' ? '' : t(core.lang, 'menu.notTaskbar')
      parts.push(line(t(core.lang, 'menu.where'), opt('where', 'bar', t(core.lang, 'menu.bar'), place === 'bar', place === 'bar' ? '' : notBar) + opt('where', 'isle', t(core.lang, 'menu.isle'), place === 'isle') + opt('where', 'desk', t(core.lang, 'menu.desk'), place === 'desk', pet.mac ? t(core.lang, 'menu.noDesk') : '')))
    }
    parts.push('<div class="wmsep"></div>')
    // With others in one place, or apart again.
    if (bar && entry) {
      if (group && member < 0) {
        parts.push(line(t(core.lang, 'menu.group'), opt('regroup', 'carousel', t(core.lang, 'menu.carousel'), entry.type === 'carousel') + opt('regroup', 'stack', t(core.lang, 'menu.stack'), entry.type === 'stack')))
        // The one it shows, out of it.
        if (shown >= 0) parts.push(item('takeOut', t(core.lang, 'menu.takeOut', { name: shownW ? WCards.titleOf(shownW, ctx) : memberName(entry.items[shown]) }), '', String(shown)))
        parts.push(item('ungroup', t(core.lang, 'menu.ungroup')))
      } else if (group) {
        parts.push(item('takeOut', t(core.lang, entry.type === 'stack' ? 'menu.outOfStack' : 'menu.outOfCarousel'), '', String(member)))
      } else {
        // The one beside it: a group it may join (its kind kept), or both
        // made one of either kind; a single one by its name (天气).
        const next = neighbourOf(entry.id)
        if (!next) {
          const why = t(core.lang, 'menu.noNeighbour')
          parts.push(item('group', t(core.lang, 'menu.stackWithNone'), why, 'stack'), item('group', t(core.lang, 'menu.carouselWithNone'), why, 'carousel'))
        } else if (Widgets.isGroup(next)) {
          const n = next.items.length
          const other = next.type === 'stack' ? 'carousel' : 'stack'
          const full = n >= 8 ? t(core.lang, 'menu.groupFull') : ''
          parts.push(item('join', t(core.lang, `menu.join.${next.type}`, { n }), full), item('group', t(core.lang, `menu.${other}WithGroup.${next.type}`), '', other))
        } else {
          const w = Widgets.sourcesOf(next, core.allWidgets)[0]
          const name = w ? WCards.titleOf(w, ctx) : memberName(next)
          parts.push(item('group', t(core.lang, 'menu.carouselWith', { name }), '', 'carousel'), item('group', t(core.lang, 'menu.stackWith', { name }), '', 'stack'))
        }
      }
    }
    // Into a board on the desktop (§7): each there is (three at most), or a
    // new one; never a group, never on a Mac.
    if (pet.mac) parts.push(item('board', t(core.lang, 'menu.board'), t(core.lang, 'menu.noDesk')))
    else if (group && member < 0) parts.push(item('board', t(core.lang, 'menu.board'), t(core.lang, 'desk.noStackInBoard')))
    else if (w || entry) {
      const it = menuItemOf(w, entry, member)
      const inBoard = b => !!it && b.items.some(x => DeskGrid.keyOf(x) === keyFor(it))
      for (const b of core.deskBoards().slice(0, 3)) {
        if (inBoard(b)) continue
        parts.push(item('board', t(core.lang, 'desk.toBoard', { name: core.boardName(b) }), it && DeskGrid.boardFirst(b, core.boardSize(it)) ? '' : t(core.lang, 'desk.boardFull'), b.id))
      }
      parts.push(item('board', t(core.lang, 'desk.newBoard'), '', 'new'))
    }
    if (w) parts.push(item('settings', t(core.lang, 'menu.settings')))
    // picker.ts: in the open island, a member's way of showing in her
    // capsule (under its name), and out of it (移出胶囊).
    const isle = !bar && w ? Picker?.isleMenu?.(w, { opt, line }) : null
    if (isle) parts.splice(1, 0, ...isle.lines)
    parts.push(item('remove', isle?.remove || t(core.lang, bar && entry ? 'menu.remove' : 'capsule.out')))
    parts.push('<div class="wmwhy" hidden></div>')
    return parts.join('')
  }

  function openMenu(m, near, at = null) {
    if (core.popFor) closePop()
    if (core.imeOpen) core.closeImeList()
    if (core.wgFor && m.where === 'bar') closeWidgetPop()
    core.menuFor = m
    wmenu.innerHTML = menuHTML(m)
    fillHer(wmenu, 16)
    wmenu.hidden = false
    wmenu.classList.toggle('isle', m.where === 'isle')
    if (m.where === 'bar') {
      if (wmenu.parentElement !== barRest) barRest.append(wmenu)
      const over = core.stackFor && !stackPop.hidden && stackPop.contains(near) ? stackPop : near
      const pos = core.besideOrOver(over, wmenu, 8)
      wmenu.style.left = pos.left === undefined ? '' : `${pos.left}px`
      wmenu.style.top = pos.top === undefined ? '' : `${pos.top}px`
      wmenu.style.bottom = ''
      wmenu.classList.toggle('above-stack', over === stackPop)
    } else {
      // In the open island, at the pointer, kept inside it; the island as
      // tall as it needs to hold it (sizeOf follows #island-expanded).
      if (wmenu.parentElement !== island) island.append(wmenu)
      const box = island.getBoundingClientRect()
      const x = Math.max(8, Math.min((at?.x ?? box.left + 20) - box.left, box.width - wmenu.offsetWidth - 8))
      const y = Math.max(8, (at?.y ?? box.top + 20) - box.top)
      wmenu.style.left = `${Math.round(x)}px`
      wmenu.style.top = `${Math.round(y)}px`
      wmenu.style.bottom = ''
      core.menuRoom = Math.ceil(y + wmenu.offsetHeight + 10)
      update()
    }
    watchPresses()
  }

  function closeMenu() {
    if (!core.menuFor) return
    const wasIsle = core.menuFor.where === 'isle'
    core.menuFor = null
    wmenu.hidden = true
    wmenu.querySelector('.wmwhy')?.setAttribute('hidden', '')
    watchPresses()
    if (wasIsle) {
      core.menuRoom = 0
      update()
    }
  }

  // A greyed one under the pointer: why, at the menu's foot.
  wmenu.addEventListener('mouseover', e => {
    const why = wmenu.querySelector('.wmwhy')
    const off = e.target.closest('[data-why]')
    if (!why) return
    why.hidden = !off
    if (off) why.textContent = off.dataset.why
  })
  wmenu.addEventListener('contextmenu', e => {
    e.preventDefault()
    e.stopPropagation()
  })
  wmenu.addEventListener('click', e => {
    e.stopPropagation()
    const b = e.target.closest('[data-act]')
    if (!b || b.classList.contains('off') || !core.menuFor) return
    const m = core.menuFor
    const { entry, w, member } = menuTarget(m)
    const v = b.dataset.v
    closeMenu()
    switch (b.dataset.act) {
      case 'size':
        if (m.where === 'bar' && entry) return keepLayout(withEntry(entry.id, p => ({ ...p, size: v })))
        if (!w) return
        if (v === 'L') return showCard(core.cardFor === w.id ? null : w.id)
        if (core.cardFor === w.id) showCard(null)
        return setRowSize(w.id, v)
      case 'where':
        if (v === 'bar' && w && !entry) return placeWidget(menuItemOf(w), 'bar')
        if (v === 'isle' && entry) return core.backToIsland(w, entry, member)
        if (v === 'isle' && w) return placeWidget(menuItemOf(w), 'isle')
        if (v === 'desk') return core.toDesk(w, entry, member)
        return
      case 'board': {
        const it = menuItemOf(w, entry, member)
        // Off the strip (its group, or its place) and into the board.
        return it && core.toBoard(it, v)
      }
      case 'group': {
        const next = entry && neighbourOf(entry.id)
        return next && keepLayout(Widgets.groupWith(tbLayout(), next.id, entry.id, v, core.newId))
      }
      // Into the group beside it, as its last; its kind kept.
      case 'join': {
        const next = entry && neighbourOf(entry.id)
        return next && Widgets.isGroup(next) && keepLayout(Widgets.groupWith(tbLayout(), entry.id, next.id, next.type, core.newId))
      }
      case 'regroup':
        return entry && keepLayout(Widgets.regroup(tbLayout(), entry.id, v))
      case 'ungroup':
        if (core.stackFor === entry?.id) closeStackPop()
        return entry && keepLayout(Widgets.ungroup(tbLayout(), entry.id, core.newId))
      case 'takeOut':
        return entry && keepLayout(Widgets.takeOut(tbLayout(), entry.id, Number(v), core.newId))
      case 'settings':
        return w && openWidgetSettings(w)
      // picker.ts: its way of showing in her capsule.
      case 'cmode':
        return w && Picker?.setMode(w, v)
      case 'remove':
        // picker.ts: taken off, with a way back (undo), and off once it is nowhere.
        if (Picker?.remove(m.where === 'bar' && entry ? { entry: entry.id, member } : { widget: w })) return
        if (m.where === 'bar' && entry) return Widgets.isGroup(entry) && member >= 0 ? removeMember(entry.id, member) : removeEntry(entry.id)
        // Out of her capsule with no picker to do it: off (put nowhere now).
        if (w) return pet.settings.set({ widgetsOff: [...new Set([...(core.config.widgetsOff || []), w.id])] })
    }
  })

  // The layout with one entry changed.
  function withEntry(id, change) {
    const layout = tbLayout()
    const at = Widgets.findEntry(layout, id)
    if (at) layout[at.run][at.at] = change(at.entry)
    return layout
  }

  // Put on the taskbar: at the right run's start (as the library's press
  // puts one), medium; a plugin's widget as its plugin (all of them, in turn).
  function putOnBar(w) {
    const source = w.plugin ? { type: 'plugin', of: w.plugin } : { type: 'widget', of: w.id }
    if (Widgets.shownOn(tbLayout(), core.allWidgets).has(w.id)) return
    const named = w.private ? {} : w.plugin ? ownName(w.plugin) : { name: [...String(Widgets.words(core.lang, w).label)].slice(0, 40).join('') }
    const entry = { id: core.newId(), ...source, size: Widgets.sizesOf(w).includes('m') ? 'M' : 'S', ...(w.icon in Widgets.ICONS ? { icon: w.icon } : {}), ...named }
    const layout = tbLayout()
    layout.right.unshift(entry)
    placedAt.set(Widgets.memberKey(source), Date.now())
    keepLayout(layout)
  }

  // Where a widget ({ id }) or a plugin she runs ({ plugin }) is, the
  // taskbar, the island or the desktop, and put there (a menu's 放到, the
  // desktop's asks). On the taskbar alone or in a group; put there, at the
  // right run's start; back in the island, out of wherever it was.
  const keyFor = item => (item.plugin ? `plugin:${item.plugin}` : `widget:${item.id}`)
  // The monitor on the strip is its part (#53): there while her home is the taskbar.
  const isMonitorItem = item => !item.plugin && item.id === 'monitor'
  // A widget as placeWidget takes it: as the desktop has it (its plugin's
  // entry, or its own), else its plugin's, else its own; a member of the
  // strip's (a group's by its index) as it is there.
  function menuItemOf(w, entry = null, member = -1) {
    const m = entry ? (Widgets.isGroup(entry) ? entry.items[member] : entry) : null
    if (m) return m.type === 'plugin' ? { plugin: m.of } : { id: m.of }
    if (!w) return null
    if (w.plugin && !DeskGrid.has(core.config.deskLayout, { id: w.id })) return { plugin: w.plugin }
    return { id: w.id }
  }
  // Where a widget is now: on the desktop (alone, stacked, in a board), on
  // the strip (the taskbar her home), else in the island.
  function placeOf(w) {
    if (!w) return 'isle'
    if (DeskGrid.has(core.config.deskLayout, { id: w.id }) || (w.plugin && DeskGrid.has(core.config.deskLayout, { plugin: w.plugin }))) return 'desk'
    if (core.placedOut.has(w.id) || (w.id === 'monitor' && home() === 'taskbar' && hasMonitorPart())) return 'bar'
    return 'isle'
  }
  function whereOf(item) {
    if (DeskGrid.has(core.config.deskLayout, item)) return 'desk'
    if (isMonitorItem(item) && home() === 'taskbar' && hasMonitorPart()) return 'bar'
    const keys = new Set(Object.values(tbLayout()).flatMap(run => run.filter(p => typeof p !== 'string').flatMap(Widgets.membersOf).map(Widgets.memberKey)))
    return keys.has(keyFor(item)) ? 'bar' : 'isle'
  }
  // For the Plugins page (settings.ts): where one is put now, as placing.ts
  // whereIs says (the strip, the desktop, a board by its name, her capsule
  // while it is on, or nowhere: null), a plugin's widgets standing on the
  // desktop by themselves counted as it.
  function placeNow(item, on) {
    const keys = [keyFor(item), ...(item.plugin ? core.allWidgets.filter(w => w.plugin === item.plugin).map(w => `widget:${w.id}`) : [])]
    const p = Placing.whereIs({ keys, on, taskbar: home() === 'taskbar' ? tbLayout() : null, desk: core.config.deskLayout, home: home() })
    return p.board ? { at: 'board', name: core.boardName(p.board) } : { at: p.at }
  }
  function placeWidget(item, where) {
    const key = keyFor(item)
    // Off the desktop as it goes elsewhere (the desktop's pages keep the places).
    if (where !== 'desk' && DeskGrid.has(core.config.deskLayout, item)) core.deskOff([item])
    if (where === 'bar') {
      if (whereOf(item) === 'bar') return
      const layout = tbLayout()
      // The monitor: its part, at the right run's start.
      if (isMonitorItem(item)) {
        for (const k of Object.keys(layout)) layout[k] = layout[k].filter(p => p !== 'monitor')
        layout.right.unshift('monitor')
        return keepLayout(layout)
      }
      const w = item.plugin ? null : core.allWidgets.find(x => x.id === item.id)
      const source = item.plugin ? { type: 'plugin', of: item.plugin } : { type: 'widget', of: item.id }
      const named = w && !w.private ? { name: [...String(Widgets.words(core.lang, w).label)].slice(0, 40).join('') } : item.plugin ? ownName(item.plugin) : {}
      const icon = (w?.icon || item.icon) in Widgets.ICONS ? { icon: w?.icon || item.icon } : {}
      layout.right.unshift({ id: core.newId(), ...source, size: !w || Widgets.sizesOf(w).includes('m') ? 'M' : 'S', ...icon, ...named })
      placedAt.set(key, Date.now())
      return keepLayout(layout)
    }
    if (where !== 'isle' && where !== 'desk') return
    let layout = tbLayout()
    let off = false
    // The monitor's part off the strip: always for the desktop (one place,
    // whatever her home); for the island while the taskbar is her home (in
    // the island's home the strip is not there, its part left for it).
    if (isMonitorItem(item) && (where === 'desk' || home() === 'taskbar') && hasMonitorPart(layout)) {
      for (const k of Object.keys(layout)) layout[k] = layout[k].filter(p => p !== 'monitor')
      off = true
    }
    const dropped = dropFromBar(layout, key)
    layout = dropped.layout
    off ||= dropped.off
    // The taskbar's layout kept only when it was on it (not written as a
    // preset's for one put on the desktop from the capsule).
    if (off || where === 'isle') keepLayout(layout)
    if (where === 'desk') return core.putOnDesk(item)
    if (item.id && (core.config.widgetsOff || []).includes(item.id)) pet.settings.set({ widgetsOff: core.config.widgetsOff.filter(x => x !== item.id) })
  }

  // A widget or plugin (by its key) off a taskbar layout: alone, or out of
  // its group (one left: alone).
  function dropFromBar(layout, key) {
    let off = false
    for (const k of Object.keys(layout))
      for (const p of [...layout[k]]) {
        if (typeof p === 'string') continue
        if (Widgets.isGroup(p)) {
          const at = p.items.findIndex(m => Widgets.memberKey(m) === key)
          if (at >= 0) {
            layout = Widgets.dropMember(layout, p.id, at, core.newId)
            off = true
          }
        } else if (Widgets.isSource(p) && Widgets.memberKey(p) === key) {
          layout[k] = layout[k].filter(x => x !== p)
          off = true
        }
      }
    return { layout, off }
  }
  // Off the taskbar, these (a desktop's board took them, desk.ts): the
  // monitor its part. Kept only if any was there.
  function offBar(items) {
    let layout = tbLayout()
    let off = false
    for (const item of items) {
      if (isMonitorItem(item) && hasMonitorPart(layout)) {
        for (const k of Object.keys(layout)) layout[k] = layout[k].filter(p => p !== 'monitor')
        off = true
      }
      const dropped = dropFromBar(layout, keyFor(item))
      layout = dropped.layout
      off ||= dropped.off
    }
    if (off) keepLayout(layout)
  }

  return { closeMenu, keyFor, offBar, openMenu, placeNow, placeWidget, whereOf, wmenu }
}
