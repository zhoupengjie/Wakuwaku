// The pointer's glide along the strip's buttons, and the list a program's
// button opens above it (its windows, or a widget's card): opened, closed,
// kept up while the pointer is on its way to it.
// Made by island.ts in the section's place as the page starts
// (glidePart(core), imported by island.ts): what it takes of the island's core
// comes in core, its functions and constants as they are, its lets read
// and written through core.<name> as they change; what it hands back is the
// island's again.
import * as Presses from './presses.ts'
import { Picker } from './pages.ts'
import { pet } from './bridge.ts'
import type { GlideCore } from './island.ts'

export function glidePart(core: GlideCore) {
  const {
    alignApps, barRest, body, crossLo, el, entryNodes, flashSince, hi, home, imeMark, isOn, isSide, lo,
    openedUp, placeSlot, t, trayBox, windowsBox,
  } = core

  // Under the pointer, one highlight for all the taskbar's buttons that
  // slides from one to the next as her island's drop moves: its leading
  // edge first, the trailing one after, so it stretches and gathers, a
  // little squashed on the way (style.css .hdrop). It comes and goes in
  // place, jumps rather than slides a long way, and gives a little under a
  // press. Not over what opens above the buttons, nor while a tray icon or
  // a program's button is being dragged.
  const GLIDE_ON = '.start, .tview, .win, .tout .ticon, .tchev, .tpriv, .kbd .ime, .tquick, .face.mon, .tw-widget, .tw-plugin, .tw-group, .tclock'
  const GLIDE_FAR = 360
  const glide = el('span', 'hdrop')
  let glideTimer

  function glideTo(node, still = false) {
    clearTimeout(glideTimer)
    const bar = barRest.getBoundingClientRect()
    const r = node.getBoundingClientRect()
    // Along the bar its two ends slide (left and right, or a side bar's top
    // and bottom); across it, as the button is.
    const [from, to, crossFrom, crossSize] = isSide() ? ['top', 'bottom', 'left', 'width'] : ['left', 'right', 'top', 'height']
    const left = lo(r) - lo(bar)
    const right = hi(bar) - hi(r)
    const was = core.glideOn && glide.classList.contains('on') && glide.dataset.axis === from ? parseFloat(glide.style[from]) : null
    if (glide.dataset.axis !== from) {
      for (const p of ['left', 'right', 'top', 'bottom', 'width', 'height']) glide.style[p] = ''
      glide.dataset.axis = from
    }
    glide.style[crossFrom] = `${crossLo(r) - crossLo(bar)}px`
    glide.style[crossSize] = `${isSide() ? r.width : r.height}px`
    if (still || was === null || Math.abs(left - was) > GLIDE_FAR) {
      glide.style.transition = 'none'
      glide.style[from] = `${left}px`
      glide.style[to] = `${right}px`
      void glide.offsetWidth
      glide.style.transition = ''
    } else if (left !== was) {
      const lead = left > was ? to : from
      const trail = lead === to ? from : to
      glide.style.transition = `${lead} 0.22s cubic-bezier(0.3, 1.3, 0.5, 1), ${trail} 0.42s cubic-bezier(0.3, 1.45, 0.5, 1), opacity 0.16s ease, transform 0.3s cubic-bezier(0.3, 1.45, 0.5, 1)`
      glide.style[from] = `${left}px`
      glide.style[to] = `${right}px`
      glide.classList.remove('squash')
      void glide.offsetWidth
      glide.classList.add('squash')
    }
    glide.classList.add('on')
    glide.classList.toggle('ink-light', core.inkOf(node))
    core.glideOn = node
  }

  function glideOff() {
    clearTimeout(glideTimer)
    glideTimer = setTimeout(() => {
      glide.classList.remove('on', 'down')
      core.glideOn = null
    }, 90)
  }

  const isGlideOn = node => node?.matches?.(GLIDE_ON) && !node.closest('.wpop, .wgpop, .tfly, .wstk, .wmenu')
  // The programs' buttons pressed in and springing back (presses.ts pressIn;
  // Start and Task View have their own, style.css .start.down).
  Presses.pressIn(windowsBox, '.win')
  barRest.addEventListener(
    'mouseenter',
    e => {
      if (!isOn() || home() !== 'taskbar' || !isGlideOn(e.target) || body.classList.contains('tray-dragging') || body.classList.contains('win-dragging')) return
      glideTo(e.target)
    },
    true,
  )
  barRest.addEventListener(
    'mouseleave',
    e => {
      if (e.target === barRest || isGlideOn(e.target)) glideOff()
    },
    true,
  )
  barRest.addEventListener('pointerdown', e => {
    if (e.button === 0 && core.glideOn && (e.target as HTMLElement).closest(GLIDE_ON) === core.glideOn) glide.classList.add('down')
  })
  window.addEventListener('pointerup', () => glide.classList.remove('down'))
  window.addEventListener('resize', () => {
    if (!isOn() || home() !== 'taskbar') return
    alignApps()
    placeSlot()
  })

  // A button for each program (taskbar/): those kept on the taskbar
  // first, then the others as their windows came. Its icon; the title of its
  // one window, or its name and how many; none while it does not run (its
  // name then under the pointer). Icons alone (style.css #bar-rest.icons):
  // no title, no count, its windows listed when the pointer rests on it. A
  // line under it, longer for the one in front; lit while a window of it
  // flashes for attention. No marks for the sessions in its windows: the
  // island says how they are.
  // The programs' buttons in the order dragged (taskbarOrder, their keys,
  // those not running kept), the rest after, as they came.
  const appOrder = () => (Array.isArray(core.config.taskbarOrder) ? core.config.taskbarOrder.map(String) : [])
  function orderedApps() {
    const order = appOrder()
    const rank = a => (order.includes(a.app) ? order.indexOf(a.app) : Infinity)
    return core.taskWindows
      .map((a, n) => [a, n])
      .sort((x, y) => rank(x[0]) - rank(y[0]) || x[1] - y[1])
      .map(([a]) => a)
  }

  // A side bar's programs crowded into their column: faded where more of
  // them lies past an end (12 px), as it scrolls.
  function fadeWins() {
    const side = isSide()
    const more = windowsBox.scrollHeight - windowsBox.clientHeight
    windowsBox.classList.toggle('fade-start', side && more > 0 && windowsBox.scrollTop > 0)
    windowsBox.classList.toggle('fade-end', side && more > 0 && windowsBox.scrollTop < more - 1)
  }
  windowsBox.addEventListener('scroll', fadeWins, { passive: true })

  function drawWindows() {
    const apps = orderedApps()
    const drawn = JSON.stringify([apps, core.lang])
    if (drawn === core.windowsDrawn) return
    core.windowsDrawn = drawn
    const now = Date.now()
    const flashing = new Set(apps.flatMap(a => (a.windows || []).filter(w => w.flash).map(w => w.id)))
    for (const id of [...flashSince.keys()]) if (!flashing.has(id)) flashSince.delete(id)
    for (const id of flashing) if (!flashSince.has(id)) flashSince.set(id, now)
    // Each program's button kept from one drawing to the next (by its key),
    // what changed changed in it: its line stretches as its window comes to
    // the front (style.css #bar-rest .win::after), a press on it plays out
    // (presses.ts pressIn) though the windows change under it; a new one's
    // line grows in (fresh). Drawn anew each time, none of it could move.
    const old = new Map([...windowsBox.children].filter(b => b.dataset.app != null).map(b => [b.dataset.app, b]))
    const fresh = []
    const buttons = apps.map(app => {
      const windows = app.windows || []
      let button = old.get(app.app)
      old.delete(app.app)
      if (!button) {
        button = el('span', 'win')
        button.dataset.app = app.app
        // As Windows' own buttons say themselves to a screen reader.
        button.setAttribute('role', 'button')
        if (core.buttonsDrawn) {
          button.classList.add('fresh')
          fresh.push(button)
        }
      }
      button.setAttribute('aria-label', t(core.lang, !windows.length ? 'taskbar.ariaPinned' : windows.length > 1 ? 'taskbar.ariaMany' : 'taskbar.ariaOne', { name: app.name || '', n: windows.length }))
      button.classList.toggle('front', windows.some(w => w.front))
      button.classList.toggle('min', windows.length > 0 && windows.every(w => w.min))
      const flashes = windows.filter(w => w.flash)
      button.classList.toggle('flash', flashes.length > 0)
      // From the longest-flashing window's start: a negative delay puts
      // the animation that far along (past its end: the last frame).
      if (flashes.length) button.style.setProperty('--flash-delay', `${-Math.max(...flashes.map(w => now - flashSince.get(w.id)))}ms`)
      else button.style.removeProperty('--flash-delay')
      button.classList.toggle('idle', !windows.length)
      if (!windows.length) button.title = app.name || ''
      else button.removeAttribute('title')
      fillButton(button, app, windows)
      return button
    })
    for (const b of old.values()) b.remove()
    // In their order, each moved only where it is not already (a move
    // starts it anew: its line, its press).
    buttons.forEach((b, i) => {
      if (windowsBox.children[i] !== b) windowsBox.insertBefore(b, windowsBox.children[i] || null)
    })
    if (fresh.length) requestAnimationFrame(() => requestAnimationFrame(() => fresh.forEach(b => b.classList.remove('fresh'))))
    core.buttonsDrawn = true
    if (core.popFor) core.drawPop()
  }

  // A button's insides as its program is now: its icon (or its first
  // letter), the title of its one window or its name, how many; each
  // changed only as it changes, the icon kept through a press.
  function fillButton(button, app, windows) {
    let icon = button.firstElementChild
    if (app.png) {
      if (icon?.tagName !== 'IMG') {
        const img = el('img')
        img.draggable = false
        if (icon) icon.replaceWith(img)
        else button.prepend(img)
        icon = img
      }
      if (icon.getAttribute('src') !== app.png) icon.src = app.png
    } else {
      const letter = (app.name || '?').slice(0, 1).toUpperCase()
      if (!icon?.classList.contains('letter')) {
        const span = el('span', 'letter')
        if (icon) icon.replaceWith(span)
        else button.prepend(span)
        icon = span
      }
      if (icon.textContent !== letter) icon.textContent = letter
    }
    let title = button.querySelector(':scope > .wt')
    const words = windows.length ? (windows.length === 1 ? windows[0].title || app.name : app.name) || '' : null
    if (words === null) title?.remove()
    else {
      if (!title) {
        title = el('span', 'wt')
        icon.after(title)
      }
      if (title.textContent !== words) title.textContent = words
    }
    let count = button.querySelector(':scope > .count')
    if (windows.length > 1) {
      if (!count) {
        count = el('span', 'count')
        button.append(count)
      }
      if (count.textContent !== String(windows.length)) count.textContent = String(windows.length)
    } else count?.remove()
  }

  // Above a program's button: its windows to pick from (on a press with
  // several, or the pointer resting on it), or what can be done with it (a
  // right press): open another, keep it on the taskbar or no longer, close.
  const pop = el('div', 'wpop')
  pop.hidden = true
  barRest.append(pop)
  // Its room for the pointer down to the buttons (style.css .wreach, #56).
  const popReach = el('span', 'wreach')
  const appOf = key => core.taskWindows.find(a => a.app === key)
  // File Explorer's button, by its program: explorer.exe, the shell itself.
  const isExplorer = path => /(^|[\\/])explorer\.exe$/i.test(path || '')

  function openPop(key, kind) {
    if (core.wgFor) core.closeWidgetPop()
    if (core.imeOpen) core.closeImeList()
    clearTimeout(core.popTimer)
    core.popFor = { app: key, kind }
    core.drawPop()
    watchPresses()
    // Windows' End task as its setting is now (the look tells it only every
    // half minute): the menu drawn again should it have changed.
    if (kind === 'menu')
      pet.taskbar.endTask().then(on => {
        if (on === core.look.endTask) return
        core.look.endTask = on === true
        if (core.popFor?.kind === 'menu') core.drawPop()
      })
  }

  function closePop() {
    clearTimeout(core.popMoveTimer)
    core.popMoving = false
    core.popShown = null
    pop.getAnimations().forEach(a => a.id === 'slide' && a.cancel())
    pop.classList.remove('sliding', 'rising')
    pop.style.width = ''
    pop.style.height = ''
    clearTimeout(core.popTimer)
    core.popFor = null
    pop.hidden = true
    core.sendThumbs()
    watchPresses()
  }

  // While the windows' list (or menu) or the folded tray icons are open, a
  // press anywhere else closes them, as Windows' do: told by the host
  // (taskbar/win/presses.rs press hook), for ours never take the focus whose loss would.
  let watching = false
  function watchPresses() {
    // picker.ts: its capsule panel, its popover, closed by a press elsewhere too.
    const want = !!core.popFor || core.trayOpen || !!core.wgFor || !!core.stackFor || core.menuFor?.where === 'bar' || core.imeOpen || !!Picker?.watches()
    if (want === watching) return
    watching = want
    pet.taskbar.watch(want)
    openedUp('pops', want)
  }
  pet.onPress(at => {
    if (!isOn() || home() !== 'taskbar' || !at) return
    const over = document.elementFromPoint(at.x, at.y)
    if (core.popFor && !(over && (pop.contains(over) || windowsBox.contains(over)))) closePop()
    if (core.trayOpen && !(over && trayBox.contains(over))) core.openTray(false)
    if (core.wgFor && !(over && (core.wgPop.contains(over) || entryNodes.get(core.wgFor)?.contains(over) || core.stackPop.contains(over)))) core.closeWidgetPop()
    if (core.stackFor && !(over && (core.stackPop.contains(over) || entryNodes.get(core.stackFor)?.contains(over) || core.wgPop.contains(over) || core.wmenu.contains(over)))) core.closeStackPop()
    if (core.menuFor?.where === 'bar' && !(over && core.wmenu.contains(over))) core.closeMenu()
    if (core.imeOpen && !(over && (core.imePop.contains(over) || imeMark.contains(over)))) core.closeImeList()
  })

  return { appOf, appOrder, closePop, drawWindows, fadeWins, glide, glideTo, isExplorer, openPop, orderedApps, pop, popReach, watchPresses }
}
