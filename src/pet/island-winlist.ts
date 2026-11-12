// A program's windows, listed over its button: drawn, slid along as the
// pointer goes from button to button, their live pictures placed (Windows
// draws them: taskbar_thumbs), pressed, dragged, and files dragged onto a
// button bringing its window up.
// Made by island.ts in the section's place as the page starts
// (winlistPart(core), imported by island.ts). The island's core (IslandCore):
// its constants and functions as they are, what it remembers in core.st
// (island-vars.ts, read and written as st.<name>), and the parts made
// before this one in core.pieces (a later one's reached once the page is
// up). What it hands back is the island's again.
import * as Aim from './aim.ts'
import * as Presses from './presses.ts'
import { pet } from './bridge.ts'
import type { IslandCore } from './island.ts'

export function winlistPart(core: IslandCore) {
  const {
    st, pieces, across, along, body, crossHi, crossLo, el, hi, home, isOn, isSide, island, len, lo, t, tbEdge,
  } = core
  const { barRest, windowsBox } = pieces.right
  const {
    appOf, appOrder, closePop, isExplorer, openPop, orderedApps, pop, popReach,
  } = pieces.glide

  function drawPop() {
    const app = st.popFor && appOf(st.popFor.app)
    const button = app && windowsBox.querySelector(`[data-app="${CSS.escape(app.app)}"]`)
    if (!app || !button) return closePop()
    const windows = app.windows || []
    const rows = []
    if (st.popFor.kind === 'list') {
      // A card for each: its icon, title and ✕, and room under them for
      // its live picture (taskbar/mod.rs thumbs).
      for (const w of windows) {
        const card = el('div', w.front ? 'wcard front' : 'wcard')
        card.dataset.win = w.id
        const top = el('div', 'wtop')
        if (w.png || app.png) {
          const img = el('img')
          img.src = w.png || app.png
          top.append(img)
        }
        // A window with no title (tasks.rs is_task): its program's name.
        top.append(el('span', 'wt', w.title || app.name))
        const x = el('span', 'wx', '✕')
        x.dataset.close = w.id
        top.append(x)
        const room = el('div', 'wthumb')
        room.dataset.thumb = w.id
        card.append(top, room)
        rows.push(card)
      }
    } else {
      const item = (text, act) => {
        const row = el('div', 'witem', text)
        row.dataset.act = act
        return row
      }
      const head = el('div', 'whead', app.name)
      rows.push(head, item(t(st.lang, windows.length ? 'taskbar.newWindow' : 'taskbar.open'), 'launch'), item(t(st.lang, app.pinned ? 'taskbar.unpin' : 'taskbar.pin'), app.pinned ? 'unpin' : 'pin'))
      // Windows' End task while its setting is on (zhoupengjie/Wakuwaku#36),
      // over the close as on Windows' menu: the program's windows' processes
      // ended (tasks.rs end_task). None for File Explorer, as Windows offers
      // none: its windows are the shell's own (explorer.exe).
      if (windows.length && st.look.endTask && !isExplorer(app.path)) rows.push(item(t(st.lang, windows.length > 1 ? 'taskbar.endAll' : 'taskbar.end'), 'end'))
      if (windows.length) rows.push(item(t(st.lang, windows.length > 1 ? 'taskbar.closeAll' : 'taskbar.close'), 'close'))
    }
    if (!rows.length) return closePop()
    // Another program's windows taking the place of the last's (the pointer
    // gone along the buttons): where the list was and how big, to slide from.
    const isList = st.popFor.kind === 'list'
    const from = isList && !pop.hidden && pop.classList.contains('list') && st.popShown && st.popShown !== app.app ? { [isSide() ? 'top' : 'left']: isSide() ? pop.offsetTop : pop.offsetLeft, width: pop.offsetWidth, height: pop.offsetHeight } : null
    const opening = pop.hidden
    // Drawn again only when what it shows changed: a card replaced under a
    // press is never clicked (the windows' list comes again often).
    const drawn = JSON.stringify([st.popFor.app, st.popFor.kind, app.name, app.pinned, windows.map(w => [w.id, w.title, w.front, (w.png || '').length]), st.lang, st.look.endTask])
    if (drawn !== st.popDrawn || pop.hidden) {
      st.popDrawn = drawn
      pop.replaceChildren(...rows, popReach)
    }
    pop.classList.toggle('menu', st.popFor.kind === 'menu')
    pop.classList.toggle('list', isList)
    pop.hidden = false
    st.popShown = app.app
    // Mid-slide, the list keeps going where it was going.
    if (st.popMoving && !from) return
    // Over its button (beside it on a side bar, 6 px off the bar), kept
    // within the screen, at its own size.
    pop.classList.remove('sliding', 'rising')
    pop.style.width = ''
    pop.style.height = ''
    const to = { ...besideOrOver(button, pop, isSide() ? 8 : 4), width: pop.offsetWidth, height: pop.offsetHeight }
    if (from) return slidePop(from, to)
    pop.style.left = to.left === undefined ? '' : `${to.left}px`
    pop.style.top = to.top === undefined ? '' : `${to.top}px`
    if (opening && isList) return risePop()
    requestAnimationFrame(sendThumbs)
  }

  // The list moving from one program's button to the next as Windows' own
  // does: sliding over and taking its new size, the new cards coming in from
  // the side it moves to. Windows draws the live pictures, which cannot
  // slide with it: they go for the slide and come back once it is there.
  // Opening, it rises a little into place, the pictures after it.
  const POP_SLIDE_MS = 300

  function holdThumbs() {
    st.popMoving = true
    clearTimeout(st.popMoveTimer)
    thumbsSent = '[]'
    pet.taskbar.thumbs([])
  }

  function settlePop(ms) {
    st.popMoveTimer = setTimeout(() => {
      st.popMoving = false
      // A slide not quite done by now would hold the list at its old size.
      pop.getAnimations().forEach(a => a.id === 'slide' && a.cancel())
      pop.classList.remove('sliding', 'rising')
      pop.style.width = ''
      pop.style.height = ''
      sendThumbs()
    }, ms)
  }

  // Where a pop goes in the bar (its px): centred over a node along the bar,
  // or beside it on a side bar (its place there by style.css), kept this
  // far from the bar's ends.
  function besideOrOver(node, popNode, keep) {
    const r = node.getBoundingClientRect()
    const base = barRest.getBoundingClientRect()
    if (isSide()) return { top: Math.round(Math.max(keep, Math.min(r.top - base.top + r.height / 2 - popNode.offsetHeight / 2, barRest.offsetHeight - popNode.offsetHeight - keep))) }
    return { left: Math.round(Math.max(keep, Math.min(r.left - base.left + r.width / 2 - popNode.offsetWidth / 2, barRest.offsetWidth - popNode.offsetWidth - keep))) }
  }

  function slidePop(from, to) {
    holdThumbs()
    // Along the bar: across a bottom or top one, up and down a side one.
    const key = 'top' in to ? 'top' : 'left'
    pop.style.setProperty('--come', `${to[key] > from[key] ? 16 : -16}px`)
    pop.classList.add('sliding')
    const box = b => ({ [key]: `${b[key]}px`, width: `${b.width}px`, height: `${b.height}px` })
    Object.assign(pop.style, box(to))
    pop.getAnimations().forEach(a => a.id === 'slide' && a.cancel())
    const slide = pop.animate([box(from), box(to)], { duration: POP_SLIDE_MS, easing: 'cubic-bezier(0.3, 1.2, 0.5, 1)' })
    slide.id = 'slide'
    settlePop(POP_SLIDE_MS)
  }

  function risePop() {
    holdThumbs()
    pop.classList.add('rising')
    settlePop(180)
  }

  // The live pictures where the cards left room for them; none once closed.
  let thumbsSent = ''
  function sendThumbs() {
    // Not while the list slides or rises: they come once it is in place.
    if (st.popMoving) return
    const items = st.popFor?.kind === 'list' ? [...pop.querySelectorAll('[data-thumb]')].map(r => {
      const b = r.getBoundingClientRect()
      return [Number(r.dataset.thumb), b.left, b.top, b.width, b.height]
    }) : []
    const sent = JSON.stringify(items)
    if (sent === thumbsSent) return
    thumbsSent = sent
    pet.taskbar.thumbs(items)
  }
  window.addEventListener('resize', () => {
    thumbsSent = ''
    requestAnimationFrame(sendThumbs)
  })

  // A press on a program's button: one not running starts; one window goes
  // to the front, or is minimized when in front; several, their list at
  // once to pick one from, as Windows 11's own (zhoupengjie/Wakuwaku#34:
  // they all came to the front together, or all went down), and pressed
  // again it goes. One the pointer rested on stays open for the press.
  // With the keys and the other buttons as on Windows' own
  // (zhoupengjie/Wakuwaku#36, presses.ts pressOf): the middle button or
  // Shift, another of it; Ctrl and Shift, another as administrator; Ctrl on
  // one with several windows, the next of them; a right press, its menu,
  // with Shift its window's own menu (one for all its windows when it has
  // several).
  function pressApp(app, what) {
    const windows = app.windows || []
    if (!what) return
    if (what === 'menu') return openPop(app.app, 'menu')
    if (what !== 'press') closePop()
    if (what === 'launch' || what === 'admin') return pet.taskbar.app(app.path, what, app.id)
    if (what === 'windowMenu') {
      menuUp = true
      return pet.taskbar.windowMenu(windows.map(w => w.id)).finally(() => (menuUp = false))
    }
    if (what === 'next') {
      const last = cycled?.app === app.app && Date.now() - cycled.at < Presses.CYCLED_MS ? cycled.id : null
      const next = Presses.nextWindow(windows, last)
      cycled = { app: app.app, id: next.id, at: Date.now() }
      return pet.taskbar.window(next.id, 'front')
    }
    if (!windows.length) return pet.taskbar.app(app.path, 'launch', app.id)
    if (windows.length > 1) {
      const listed = st.popFor?.kind === 'list' && st.popFor.app === app.app && !pop.hidden
      if (listed && st.popFor.pressed) return closePop()
      if (!listed) openPop(app.app, 'list')
      st.popFor.pressed = true
      return
    }
    closePop()
    pet.taskbar.window(windows[0].id, 'press')
  }
  // The window Ctrl and a press last brought, for the next to go on from
  // it while its mark in front has not come yet ({ app, id, at }).
  let cycled = null
  // A window's own menu open (Shift and a right press): no list of windows
  // on the pointer resting on the button, which came up over the menu.
  let menuUp = false
  // Shift and Ctrl as Windows has them at the press (taskbar_held), there
  // by the time the button comes up: her window never takes the keyboard,
  // and what the page's own events say of them may be behind. Theirs only
  // when it could not say.
  let heldAtPress = null
  windowsBox.addEventListener('pointerdown', e => {
    if (e.target.closest('[data-app]')) heldAtPress = pet.taskbar.held()
  })
  function pressAt(e) {
    const key = e.target.closest('[data-app]')?.dataset.app
    const press = { type: e.type, button: e.button }
    const own = { shift: e.shiftKey, ctrl: e.ctrlKey }
    const held = heldAtPress
    heldAtPress = null
    Promise.resolve(held).then(keys => {
      const app = appOf(key)
      if (app) pressApp(app, Presses.pressOf(press, keys || own, (app.windows || []).length))
    })
  }
  windowsBox.addEventListener('click', e => {
    if (!appOf(e.target.closest('[data-app]')?.dataset.app)) return
    e.stopPropagation()
    // The end of a drag, not a press.
    if (winDragged) {
      winDragged = false
      return
    }
    pressAt(e)
  })
  windowsBox.addEventListener('auxclick', e => {
    if (e.button !== 1 || !appOf(e.target.closest('[data-app]')?.dataset.app)) return
    e.stopPropagation()
    pressAt(e)
  })
  // The middle button pressed: no scrolling by the pointer (Chromium's), on
  // a side bar's column of buttons.
  windowsBox.addEventListener('mousedown', e => e.button === 1 && e.preventDefault())
  windowsBox.addEventListener('contextmenu', e => {
    if (!appOf(e.target.closest('[data-app]')?.dataset.app)) return
    e.preventDefault()
    e.stopPropagation()
    pressAt(e)
  })
  // A program's button dragged along the taskbar: put where it is let go
  // among the others, before the one under the pointer (after it past its
  // middle), the order kept (taskbarOrder). { app, x, y, ghost } while held.
  let winPress = null
  let winDragged = false
  windowsBox.addEventListener('pointerdown', e => {
    const key = e.target.closest('[data-app]')?.dataset.app
    if (e.button !== 0 || !key) return
    winPress = { app: key, x: e.clientX, y: e.clientY, ghost: null }
  })
  windowsBox.addEventListener('pointermove', e => {
    if (!winPress) return
    // Let go of off the buttons before a drag began: nothing held now.
    if (!(e.buttons & 1)) return cancelWinDrag()
    if (!winPress.ghost && Math.hypot(e.clientX - winPress.x, e.clientY - winPress.y) > 6) {
      // Held from here on, wherever the pointer goes; only now, for a
      // captured press clicks on the box, not on its button.
      try {
        windowsBox.setPointerCapture(e.pointerId)
      } catch {}
      const from = windowsBox.querySelector(`[data-app="${CSS.escape(winPress.app)}"] img`)
      winPress.ghost = el('img', 'tghost')
      if (from) winPress.ghost.src = from.src
      body.append(winPress.ghost)
      body.classList.add('win-dragging')
      clearTimeout(st.popTimer)
      closePop()
    }
    if (winPress.ghost) {
      Object.assign(winPress.ghost.style, { left: `${e.clientX - 10}px`, top: `${e.clientY - 10}px` })
      markWinDrop(winDropAt(e.clientX, e.clientY, winPress.app))
    }
  })
  windowsBox.addEventListener('pointerup', e => {
    const press = winPress
    winPress = null
    if (!press?.ghost) return
    press.ghost.remove()
    body.classList.remove('win-dragging')
    markWinDrop(null)
    // The click that follows the release is the drag's.
    winDragged = true
    setTimeout(() => (winDragged = false), 0)
    const drop = winDropAt(e.clientX, e.clientY, press.app)
    if (!drop) return
    const order = appOrder()
    for (const a of orderedApps()) if (!order.includes(a.app)) order.push(a.app)
    order.splice(order.indexOf(press.app), 1)
    order.splice(order.indexOf(drop.target) + (drop.after ? 1 : 0), 0, press.app)
    pet.settings.set({ taskbarOrder: order.slice(0, 200) })
  })
  // Cancelled, or the pointer let go of before the button came up (the
  // window letting the mouse through once it left the taskbar): no drop.
  const cancelWinDrag = () => {
    winPress?.ghost?.remove()
    winPress = null
    body.classList.remove('win-dragging')
    markWinDrop(null)
  }
  windowsBox.addEventListener('pointercancel', cancelWinDrag)
  windowsBox.addEventListener('lostpointercapture', () => winPress && cancelWinDrag())

  // Where a dragged button would land at (x, y): beside the button whose
  // span holds x (past the last, after it); null when let go off the
  // taskbar, or on itself.
  function winDropAt(x, y, key) {
    const strip = windowsBox.getBoundingClientRect()
    const [a, c] = [along(x, y), across(x, y)]
    if (c < crossLo(strip) - 24 || c > crossHi(strip) + 24) return null
    const buttons = [...windowsBox.querySelectorAll('[data-app]')]
    const self = buttons.find(b => b.dataset.app === key)
    const r0 = self?.getBoundingClientRect()
    if (r0 && a >= lo(r0) && a <= hi(r0)) return null
    const others = buttons.filter(b => b !== self)
    if (!others.length) return null
    for (const b of others) {
      const r = b.getBoundingClientRect()
      if (a < hi(r)) return { target: b.dataset.app, after: a > lo(r) + len(r) / 2, slot: b }
    }
    const last = others[others.length - 1]
    return { target: last.dataset.app, after: true, slot: last }
  }

  let winMarked = null
  function markWinDrop(drop) {
    if (winMarked) winMarked.classList.remove('drop-before', 'drop-after')
    winMarked = drop?.slot || null
    if (winMarked) winMarked.classList.add(drop.after ? 'drop-after' : 'drop-before')
  }

  // The pointer resting on a running program's button: its windows. With
  // another's list open, only once it has been on this button as long as
  // Windows' own taskbar waits, and not while it is on its way up into
  // the list (aim.ts, zhoupengjie/Wakuwaku#56): it changed at once to any
  // button crossed on the way to a card.
  const aim = Aim.create()
  const aimList = () => (st.popFor?.kind === 'list' && !pop.hidden ? { key: st.popFor.app, rect: pop.getBoundingClientRect(), edge: tbEdge() } : null)
  function aimPop() {
    clearTimeout(st.popTimer)
    const app = appOf(aim.key())
    if (!app || winPress?.ghost || menuUp || st.popFor?.kind === 'menu' || !(app.windows || []).length) return
    const wait = aim.due(aimList(), performance.now())
    if (wait === null) return
    if (wait === 0) return openPop(app.app, 'list')
    st.popTimer = setTimeout(aimPop, wait)
  }
  windowsBox.addEventListener('mousemove', e => aim.move(e.clientX, e.clientY, performance.now()))
  windowsBox.addEventListener('mouseover', e => {
    const at = performance.now()
    aim.move(e.clientX, e.clientY, at)
    aim.over(e.target.closest('[data-app]')?.dataset.app ?? null, at)
    aimPop()
  })
  // Gone from the button and the list a moment: it closes. Between them it
  // is on the list (style.css .wpop::after: its room reaches the strip, as
  // Windows' own does).
  for (const node of [windowsBox, pop]) {
    node.addEventListener('mouseleave', () => {
      if (node === windowsBox) aim.over(null, performance.now())
      clearTimeout(st.popTimer)
      st.popTimer = setTimeout(closePop, Aim.CLOSE_MS)
    })
    node.addEventListener('mouseenter', () => st.popFor && clearTimeout(st.popTimer))
  }

  // A file dragged from another program and held over a running program's
  // button half a second: its window to the front, or with several their
  // list open, a card in it held over as long bringing that one, for the
  // file to be let go of in the window, as on Windows' own
  // (zhoupengjie/Wakuwaku#36). The drag's place comes from Tauri
  // (bridge.ts onFileDrag, which also plays it as the pointer's: the list
  // goes once the drag has left it and the buttons a moment, as for the
  // pointer). What it is held over: 'win:<id>', 'app:<key>', or null.
  let fileHeld = null
  let fileTimer
  pet.onFileDrag((type, at, paths) => {
    // Let go of over the strip: programs and shortcuts to them pinned, as
    // Windows' own taskbar pins one dropped on it (taskbar/mod.rs taskbar_pin_files).
    if (type === 'drop' && at && paths?.length && isOn() && home() === 'taskbar' && !st.editLayout) {
      const on = document.elementFromPoint(at.x, at.y)
      if (on && (barRest.contains(on) || island.contains(on))) pet.taskbar.pinFiles(paths)
    }
    const live = at && (type === 'enter' || type === 'over') && isOn() && home() === 'taskbar' && !st.editLayout
    const node = live ? document.elementFromPoint(at.x, at.y) : null
    const card = node && pop.contains(node) ? node.closest('[data-win]') : null
    const button = !card && node && windowsBox.contains(node) ? node.closest('[data-app]') : null
    const held = card ? `win:${card.dataset.win}` : button ? `app:${button.dataset.app}` : null
    if (held === fileHeld) return
    fileHeld = held
    clearTimeout(fileTimer)
    if (!held) return
    fileTimer = setTimeout(() => {
      if (card) return pet.taskbar.window(card.dataset.win, 'drag')
      const app = appOf(button.dataset.app)
      const windows = app?.windows || []
      if (windows.length === 1) {
        closePop()
        pet.taskbar.window(windows[0].id, 'drag')
      } else if (windows.length > 1 && !(st.popFor?.kind === 'list' && st.popFor.app === app.app && !pop.hidden)) {
        openPop(app.app, 'list')
      }
    }, Presses.DRAG_HOLD_MS)
  })

  // On the press, not the click: what was pressed may be drawn anew before
  // the button comes up.
  pop.addEventListener('click', e => e.stopPropagation())
  pop.addEventListener('pointerdown', e => {
    if (e.button !== 0) return
    e.stopPropagation()
    const app = st.popFor && appOf(st.popFor.app)
    if (!app) return
    const close = e.target.closest('[data-close]')
    if (close) return pet.taskbar.window(close.dataset.close, 'close')
    const row = e.target.closest('[data-win]')
    // A window picked: to the front, never minimized (taskbar/ 'front').
    if (row) {
      closePop()
      return pet.taskbar.window(row.dataset.win, 'front')
    }
    const act = e.target.closest('[data-act]')?.dataset.act
    if (!act) return
    closePop()
    if (act === 'close' || act === 'end') for (const w of app.windows || []) pet.taskbar.window(w.id, act)
    else pet.taskbar.app(app.path, act, app.id)
  })
  pop.addEventListener('contextmenu', e => {
    e.preventDefault()
    e.stopPropagation()
  })

  return { besideOrOver, drawPop, pressApp, sendThumbs }
}
