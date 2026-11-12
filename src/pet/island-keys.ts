// The keyboard on the taskbar (Win+T, Win+B), as on Windows' own: the
// programs' buttons and the tray's icons gone through with the arrow keys,
// pressed with Enter or Space, their menus with Shift+F10.
// Made by island.js in the section's place as the page starts
// (keysPart(core), imported by island.js): what it takes of the island's core
// comes in core, its functions and constants as they are, its lets read
// and written through core.<name> as they change; what it hands back is the
// island's again.
import { Picker, Settings } from './pages.ts'
import { pet } from './bridge.ts'

export function keysPart(core) {
  const {
    along, appOf, barRest, batMark, body, chevron, clockBox, closeImeList, closePop, closeStackPop,
    closeWidgetPop, el, endEdit, flyout, hi, home, isOn, isOut, isSetting, isSide, lo, netMark, openImeList,
    openMenu, openStackPop, openTray, openWidgetPop, orderedTray, outBox, pop, pressApp, quickBox, t, trayBox,
    trayOrder, volMark, wgPop, windowsBox, wmenu,
  } = core

  // As on Windows' own taskbar: Win+T puts the keyboard on the programs'
  // buttons (the first; Win+Shift+T the last; again, the next one), Win+B
  // on the tray (^ first, then the icons kept out). The arrows along the bar
  // go from one to the next (up and down on a side bar; Home, End), Tab
  // goes between the two, Enter or Space is a press, Shift+F10 or the menu
  // key a right press, Esc gives the keyboard back to the window that had
  // it. What a press opens over a button (its windows to pick from, its
  // menu, the folded tray icons) is gone through the same way, any arrow
  // along it, Esc back to the button. A ring round the one it is on, as
  // Windows draws it (style.css .kfocus). The keys come from the hook that
  // takes them from Windows' put-away taskbar (winkeys.rs), which has given
  // her window the keyboard already; a press that brings another window to
  // the front takes it away again (island:blur), and that ends it too.
  // Where it is: the part ('apps', 'tray'; 'pop', 'fly' for what opened
  // over them), the one it is on (by its key, and its place), the part and
  // the one it came from into what opened, and when the settings were
  // closed for it.
  let keyPart = null
  let keyAt = null
  let keyFrom = null
  let keyedAt = 0
  // Win pressed on the strip: the keyboard given back while it is held
  // (winkeys.rs pause), the ring kept for a Win+T or Win+B after it.
  let keyPaused = false
  const keyId = node => node.dataset.app ?? node.dataset.tray ?? node.dataset.win ?? node.dataset.act ?? (node === chevron ? '^' : '')
  function keyNodes(part) {
    if (part === 'apps') return [...windowsBox.querySelectorAll('[data-app]')]
    if (part === 'tray') return [...(chevron.hidden ? [] : [chevron]), ...outBox.querySelectorAll('[data-tray]')]
    if (part === 'pop') return pop.hidden ? [] : [...pop.querySelectorAll('[data-win], [data-act]')]
    if (part === 'fly') return core.trayOpen ? [...flyout.querySelectorAll('[data-tray]')] : []
    return []
  }

  // The ring on the one it is at, found again after the bar is drawn anew
  // (by its key, else at its place); what opened over a button gone, back
  // on that button.
  function keyRing() {
    if ((keyPart === 'pop' || keyPart === 'fly') && !keyNodes(keyPart).length) {
      keyPart = keyFrom?.part || 'apps'
      keyAt = keyFrom?.at || null
    }
    const nodes = keyNodes(keyPart)
    const node = nodes.find(n => keyId(n) === keyAt?.id) || nodes[Math.min(keyAt?.i ?? 0, nodes.length - 1)] || null
    for (const n of barRest.querySelectorAll('.kfocus')) if (n !== node) n.classList.remove('kfocus')
    if (!node) return
    keyAt = { id: keyId(node), i: nodes.indexOf(node) }
    node.classList.add('kfocus')
    node.tabIndex = -1
    if (!node.hasAttribute('role')) node.setAttribute('role', 'button')
    // A window's card by its title (its ✕ is no part of its name).
    if (node.dataset.win) node.setAttribute('aria-label', node.querySelector('.wt')?.textContent || '')
    if (document.activeElement !== node) node.focus({ preventScroll: true })
    // In sight in a side bar's programs' column, should it scroll.
    if (isSide() && windowsBox.contains(node)) {
      const box = windowsBox.getBoundingClientRect()
      const r = node.getBoundingClientRect()
      windowsBox.scrollTop += r.top < box.top ? r.top - box.top : r.bottom > box.bottom ? r.bottom - box.bottom : 0
    }
  }
  // Drawn anew (the windows changed, a tray icon came or went): the ring found again.
  new MutationObserver(() => keyPart && keyRing()).observe(barRest, { childList: true, subtree: true })

  function keyStep(by) {
    const nodes = keyNodes(keyPart)
    if (!nodes.length) return
    const at = Math.max(0, nodes.findIndex(n => keyId(n) === keyAt?.id))
    keyAt = { id: keyId(nodes[(at + by + nodes.length) % nodes.length]) }
    keyRing()
  }

  // Into what a press opened over a button, its first.
  function keyInto(part) {
    keyFrom = { part: keyPart, at: keyAt }
    keyPart = part
    keyAt = null
    keyRing()
  }

  function keysOff() {
    keyPaused = false
    if (!keyPart) return
    keyPart = keyAt = keyFrom = null
    keyRing()
    if (barRest.contains(document.activeElement)) document.activeElement.blur()
    pet.keyboard(false)
  }

  // Enter, Space: as a press with the pointer, by the same handlers (a
  // program started, its window to the front, its windows listed; a window
  // or an item picked from them; a tray icon's program told; ^ opening the
  // folded ones). A program's button pressed at once (pressApp), not by a
  // click: the pointer's waits for the keys held (taskbar_held), and its
  // list would not be open yet to go into.
  function keyPress(node) {
    if (keyPart === 'apps') {
      const app = appOf(node.dataset.app)
      if (app) pressApp(app, 'press')
      if (core.popFor?.app === node.dataset.app && !pop.hidden) keyInto('pop')
      return
    }
    if (keyPart === 'pop') {
      node.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, cancelable: true, button: 0 }))
      return keyRing()
    }
    if (node === chevron) {
      openTray(!core.trayOpen)
      if (core.trayOpen) keyInto('fly')
      return
    }
    pet.taskbar.tray(node.dataset.tray, 'leftDown')
    pet.taskbar.tray(node.dataset.tray, 'leftUp')
    if (keyPart === 'fly') {
      openTray(false)
      keyRing()
    }
  }

  // Shift+F10, the menu key: as a right press (a program's menu, the keyboard
  // in it; a tray icon's program's own).
  function keyMenu(node) {
    if (keyPart === 'apps') {
      const app = appOf(node.dataset.app)
      if (app) pressApp(app, 'menu')
      if (core.popFor?.kind === 'menu' && !pop.hidden) keyInto('pop')
    } else if (node.dataset.tray) {
      pet.taskbar.tray(node.dataset.tray, 'rightDown')
      pet.taskbar.tray(node.dataset.tray, 'rightUp')
    }
  }

  // Esc: out of what opened, back on its button; else the keyboard given back.
  function keyBack() {
    if (keyPart === 'pop') closePop()
    else if (keyPart === 'fly') openTray(false)
    else return keysOff()
    keyRing()
  }

  pet.onFocusBar(({ part, back } = {}) => {
    // Win let go after something else than Win+T or Win+B (winkeys.rs win_up).
    if (part === 'end') return keysOff()
    keyPaused = false
    barRest.setAttribute('aria-label', t(core.lang, 'taskbar.aria'))
    if (!isOn() || home() !== 'taskbar' || core.editLayout) return pet.keyboard(false)
    // The settings open: closed (theirs went back to the window before as
    // they closed, which blurs this one); the keyboard comes once Win is let
    // go (winkeys.rs).
    keyedAt = 0
    if (isSetting()) {
      keyedAt = Date.now()
      Settings?.close?.('keys')
    }
    const want = part === 'tray' ? 'tray' : 'apps'
    // Win+T again with the keyboard on the buttons: the next one (Shift: the one before), as Windows' does.
    const again = keyPart === 'apps' && want === 'apps'
    if (core.popFor) closePop()
    if (core.trayOpen) openTray(false)
    keyPart = want
    keyFrom = null
    const nodes = keyNodes(want)
    if (!nodes.length) return keysOff()
    if (again) return keyStep(back ? -1 : 1)
    keyAt = { id: keyId(nodes[back ? nodes.length - 1 : 0]) }
    keyRing()
  })
  // Gone to another window: over. Not for the settings closing as it began
  // (keyedAt: then).
  pet.onBlur(() => keyPart && !keyPaused && Date.now() - keyedAt > 600 && keysOff())

  document.addEventListener(
    'keydown',
    e => {
      // Win pressed with the keyboard here (on the strip, or in the
      // settings): given back while Win is held, for what comes with it
      // reaches the hook only so (winkeys.rs); the settings close with it.
      if (e.key === 'Meta' && !e.repeat && home() === 'taskbar' && (keyPart || isSetting())) {
        keyPaused = !!keyPart
        return pet.taskbar.keysPause()
      }
      // Editing the taskbar's layout: Esc closes what the picker has open
      // (picker.js: a drag, its popover, the search's words, the picker),
      // then ends it, as 完成: kept as it goes, there is no 取消 (#53).
      if (core.editLayout && e.key === 'Escape') {
        e.preventDefault()
        if (Picker?.escape()) return
        return endEdit(true)
      }
      if (!keyPart) return
      // The settings opened meanwhile (a press on her capsule): the keys are theirs.
      if (isSetting()) return keysOff()
      const k = e.key
      const node = keyNodes(keyPart).find(n => keyId(n) === keyAt?.id)
      // Along the bar, or in what opened over it any way.
      const [prev, next] = keyPart === 'pop' || keyPart === 'fly' ? [null, null] : isSide() ? ['ArrowUp', 'ArrowDown'] : ['ArrowLeft', 'ArrowRight']
      if (next ? k === next : k === 'ArrowRight' || k === 'ArrowDown') keyStep(1)
      else if (prev ? k === prev : k === 'ArrowLeft' || k === 'ArrowUp') keyStep(-1)
      else if (k === 'Home' || k === 'End') {
        const nodes = keyNodes(keyPart)
        if (nodes.length) keyAt = { id: keyId(nodes[k === 'Home' ? 0 : nodes.length - 1]) }
        keyRing()
      } else if (k === 'Tab' && (keyPart === 'apps' || keyPart === 'tray')) {
        const other = keyPart === 'apps' ? 'tray' : 'apps'
        if (keyNodes(other).length) {
          keyPart = other
          keyAt = null
          keyRing()
        }
      } else if ((k === 'Enter' || k === ' ') && node) {
        if (!e.repeat) keyPress(node)
      } else if ((k === 'ContextMenu' || (k === 'F10' && e.shiftKey)) && node) keyMenu(node)
      else if (k === 'Escape') keyBack()
      else if (k !== 'Tab') return
      e.preventDefault()
      e.stopPropagation()
    },
    true,
  )

  // A press on a tray icon goes to its program, as Windows' taskbar tells it:
  // the second press of a double one as a double press; the pointer coming,
  // moving (at most every 100 ms) and going. A left press is told once it
  // is let go, for it may be a drag instead: an icon dragged onto ^ or into
  // the folded ones is folded away, one dragged onto the taskbar kept out,
  // either put where it is let go among the others there.
  const trayKey = e => e.target.closest('[data-tray]')?.dataset.tray
  let trayOver = null
  let trayMovedAt = 0
  // A left press on an icon not yet let go: { key, name, x, y, ghost }.
  let trayPress = null
  trayBox.addEventListener('pointerdown', e => {
    if (e.button === 0 && trayKey(e)) {
      try {
        trayBox.setPointerCapture(e.pointerId)
      } catch {}
    }
  })
  trayBox.addEventListener('mousedown', e => {
    const key = trayKey(e)
    if (!key) return
    e.stopPropagation()
    if (e.button === 2) return pet.taskbar.tray(key, 'rightDown')
    if (e.button !== 0) return
    if (e.detail === 2) {
      trayPress = null
      return pet.taskbar.tray(key, 'double')
    }
    trayPress = { key, name: e.target.closest('[data-tray]').dataset.name, x: e.clientX, y: e.clientY, ghost: null }
  })
  trayBox.addEventListener('mouseup', e => {
    const key = trayKey(e)
    if (e.button === 2 && key) {
      e.stopPropagation()
      return pet.taskbar.tray(key, 'rightUp')
    }
    if (e.button !== 0 || !trayPress) return
    e.stopPropagation()
    const press = trayPress
    trayPress = null
    if (!press.ghost) {
      pet.taskbar.tray(press.key, 'leftDown')
      pet.taskbar.tray(press.key, 'leftUp')
      if (flyout.contains(e.target)) openTray(false)
      return
    }
    press.ghost.remove()
    body.classList.remove('tray-dragging')
    markDrop(null)
    const icon = core.trayIcons.find(i => String(i.key) === press.key)
    const drop = dropAt(e.clientX, e.clientY, press.name)
    if (!icon || !drop) return
    // Kept out or folded, and where among the others: before the icon
    // under the pointer (after it past its middle), else last there.
    const order = trayOrder()
    for (const i of orderedTray()) if (!order.includes(i.name)) order.push(i.name)
    order.splice(order.indexOf(press.name), 1)
    let at
    if (drop.target) {
      at = order.indexOf(drop.target) + (drop.after ? 1 : 0)
    } else {
      const there = orderedTray().filter(i => i.name !== press.name && isOut(i) === drop.out)
      at = there.length ? order.indexOf(there[there.length - 1].name) + 1 : order.length
    }
    order.splice(at, 0, press.name)
    pet.taskbar.trayPin(press.name, isOut(icon) === drop.out ? null : drop.out, order.slice(0, 200))
  })

  // Where a dragged icon would land at (x, y): kept out (anywhere on the
  // taskbar) or folded (the folded ones, ^), and the icon there it goes
  // beside (none: last); null, nowhere.
  function dropAt(x, y, name) {
    const over = document.elementFromPoint(x, y)
    if (!over) return null
    const out = !(flyout.contains(over) || chevron.contains(over))
    if (out && !barRest.contains(over)) return null
    const slot = over.closest('[data-tray]')
    // Let go on itself: left where it was.
    if (slot?.dataset.name === name) return null
    const mine = slot && (out ? outBox : flyout).contains(slot) ? slot : null
    if (!mine) return { out, target: null, after: false, slot: null }
    const r = mine.getBoundingClientRect()
    // The folded ones in their grid; those kept out along the bar.
    const a = out ? along(x, y) : x
    const [start, size] = out && isSide() ? [r.top, r.height] : [r.left, r.width]
    return { out, target: mine.dataset.name, after: a > start + size / 2, slot: mine }
  }

  // While dragging: a bar on the side of the icon it would land beside.
  let dropMarked = null
  function markDrop(drop) {
    if (dropMarked) dropMarked.classList.remove('drop-before', 'drop-after')
    dropMarked = drop?.slot || null
    if (dropMarked) dropMarked.classList.add(drop.after ? 'drop-after' : 'drop-before')
  }
  window.addEventListener('mousemove', e => {
    if (!trayPress) return
    if (!trayPress.ghost && Math.hypot(e.clientX - trayPress.x, e.clientY - trayPress.y) > 6) {
      const from = trayBox.querySelector(`[data-tray="${trayPress.key}"] img`)
      trayPress.ghost = el('img', 'tghost')
      if (from) trayPress.ghost.src = from.src
      body.append(trayPress.ghost)
      body.classList.add('tray-dragging')
      // Dragged from the taskbar: the folded ones open to take it.
      openTray(true)
    }
    if (trayPress.ghost) {
      Object.assign(trayPress.ghost.style, { left: `${e.clientX - 10}px`, top: `${e.clientY - 10}px` })
      markDrop(dropAt(e.clientX, e.clientY, trayPress.name))
    }
  })
  trayBox.addEventListener('mousemove', e => {
    const key = trayKey(e) || null
    if (key !== trayOver) {
      if (trayOver) pet.taskbar.tray(trayOver, 'out')
      if (key) pet.taskbar.tray(key, 'in')
      trayOver = key
    }
    if (key && Date.now() - trayMovedAt > 100) {
      trayMovedAt = Date.now()
      pet.taskbar.tray(key, 'move')
    }
  })
  trayBox.addEventListener('mouseleave', () => {
    if (trayOver) pet.taskbar.tray(trayOver, 'out')
    trayOver = null
  })

  // The taskbar's own buttons open what Windows' do: Start, and from the
  // clock the notifications and the calendar (once the button is up).
  barRest.addEventListener('click', e => {
    const what = e.target.closest('[data-bar]')?.dataset.bar
    if (what && what !== 'settings') pet.taskbar.open(what)
    if (what === 'ime' && core.imeOpen) closeImeList()
    // A widget put there: its card, or closed again; a carousel's, the one
    // it shows; a stack: its others opened over it, or closed again.
    const widget = e.target.closest('.tw-widget, .tw-plugin, .tw-carousel')
    if (widget) core.wgFor === widget.dataset.entry ? closeWidgetPop() : openWidgetPop(widget.dataset.entry)
    const stack = e.target.closest('.tw-stack')
    if (stack) core.stackFor === stack.dataset.entry ? closeStackPop() : openStackPop(stack.dataset.entry)
  })
  // A right press: a tray icon's program has it; on the input method, the
  // input methods to pick from; on a widget put there, its card; on the
  // volume, the network or the clock, the menu Windows' own taskbar has
  // there (zhoupengjie/Wakuwaku#36: tray.rs part_menu); elsewhere, her menu.
  barRest.addEventListener('contextmenu', e => {
    e.preventDefault()
    if (trayKey(e)) return
    if (wgPop.contains(e.target) || wmenu.contains(e.target)) return
    // A widget put there (one, or several in one place): its menu.
    const widget = e.target.closest('.tw-widget, .tw-plugin, .tw-group')
    if (widget) return openMenu({ where: 'bar', entry: widget.dataset.entry }, widget)
    if (e.target.closest('[data-bar="ime"]')) return core.imeOpen ? closeImeList() : openImeList()
    const mark = quickBox.contains(e.target) ? quickMarkAt(e.clientX, e.clientY) : null
    pet.menu(mark === volMark ? 'volume' : mark === netMark ? 'network' : clockBox.contains(e.target) ? 'clock' : null)
  })
  // Which of the quick settings' marks a press on its button is for: the
  // nearest along the strip, each having the button's height (its width
  // along a side), as Windows' own do. The battery's menu stays hers.
  function quickMarkAt(x, y) {
    const a = along(x, y)
    let nearest = null
    let gap = Infinity
    for (const mark of [netMark, volMark, batMark]) {
      if (mark.hidden) continue
      const r = mark.getBoundingClientRect()
      const d = a < lo(r) ? lo(r) - a : a > hi(r) ? a - hi(r) : 0
      if (d < gap) [nearest, gap] = [mark, d]
    }
    return nearest
  }
  // It takes the pointer while it is under it (the window lets clicks through elsewhere).
  barRest.addEventListener('mouseenter', () => pet.hover(true))
  barRest.addEventListener('mouseleave', () => pet.hover(false))
}
