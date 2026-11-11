// The taskbar's look: its Mica (the desktop's picture under it, mica.js),
// light or dark and Windows' accent, the ink of its words on what is under
// them; the clock, the tray drawn and opened, the peek at the desktop.
// Made by island.js in the section's place as the page starts
// (window.IslandParts.look(core)): what it takes of the island's core
// comes in core, its functions and constants as they are, its lets read
// and written through core.<name> as they change; what it hands back is the
// island's again.
window.IslandParts = window.IslandParts || {}
window.IslandParts.look = function (core) {
  const {
    DEEP_OP, PEEK_MS, TASKBAR_H, alignApps, barRest, body, clearWhen, clockBox, deskEdge, drawEntries,
    drawWindows, el, endNudge, frontBox, frontIcon, frontName, glide, home, isCapsule, isClear, isFilled,
    isLight, isMica, isOn, isSide, isSlim, isStripLight, island, islandViews, placeAnchor, sendSolid, setDeep,
    snapStart, stripH, t, trayBox, update, watchPresses,
  } = core

  // Behind the strip's parts: the desktop's picture under it, blurred and
  // tinted (mica.js) for Windows' mode, as taskbar/ sent it last; drawn
  // again only when the picture, the mode or the strip's width changes. None
  // on a solid strip. On a clear one (tb-clear) it is drawn too, unseen, if
  // the strip is to fill in as Mica (tb-clear-mica): it fades in at once
  // while a window is maximized or full screen on its display (tb-filled,
  // drawn again as the windows change); filled in solid, tb-solid. The
  // strip light in Windows' light mode (tb-light), its own words dark on
  // it or on a light desktop under the clear strip (tb-ink-light), her end
  // a capsule wherever the strip is not black (tb-capsule).
  // The strip as thick as it lies on the screen (island.rs tell_strip): its
  // physical px over the scale, a fraction of a CSS px if need be, so its
  // inner edge is the work area's, where maximized windows stop (a hairline
  // gap at 175%: zhoupengjie/Wakuwaku#2). 48 until told.
  let stripThick = 0
  function setStrip() {
    if (!stripThick) return
    document.documentElement.style.setProperty('--strip-h', `${stripThick / (devicePixelRatio || 1)}px`)
    fitPage()
    // A monitor's scale or Windows text scaling may have changed while the
    // settings are open; recompute their body height with the new CSS room.
    if (window.Settings?.isOpen?.()) window.Settings.redraw()
    // Once laid out for it (its edge may have just changed too).
    setTimeout(() => {
      const r = island.parentElement.getBoundingClientRect()
      const dpr = devicePixelRatio || 1
      console.log(`taskbar: strip in the page ${[r.left, r.top, r.right, r.bottom].map(v => +(v * dpr).toFixed(2)).join(', ')} (x ${dpr}, told ${stripThick} px)`)
    }, 400)
  }
  // Her tray icon clicked on a Mac: the island in the notch drops open, or
  // goes back. Open, it goes back once the pointer has been in it and left,
  // or after a while if the pointer never comes.
  function setPeek(on) {
    if (on !== core.peek) console.log(`island: peek ${on ? 'open' : 'closed'}`)
    if (!on) endNudge()
    core.peek = on
    clearTimeout(core.peekTimer)
    if (core.peek) core.peekTimer = setTimeout(() => !core.isHover && setPeek(false), PEEK_MS)
    update()
  }
  // Her tray icon toggles it; a double-click on her only opens it ({ open }).
  window.pet.onPeek(p => setPeek(p?.open === true ? true : !core.peek))
  window.pet.onTop(({ height, notch } = {}) => {
    const next = height > 0 ? { height, notch: notch > 0 ? notch : 0 } : null
    if (JSON.stringify(next) === JSON.stringify(core.screenTop)) return
    core.screenTop = next
    body.classList.toggle('at-top', !!core.screenTop)
    body.classList.toggle('notched', !!core.screenTop?.notch)
    document.documentElement.style.setProperty('--top-h', `${core.screenTop ? core.screenTop.height : 36}px`)
    // Round a notch, only ever in the middle.
    placeAnchor()
    update()
  })
  // The page as big as its window (told with the strip, its physical size).
  // At 175% Chromium makes the page a little bigger than its window (the
  // window's 1204 px over 1.75 come to 688.00006 in its sums, so 689 CSS
  // px): what lies along the page's foot or right end lay past the window's,
  // the strip 2 px low and a gap above it (zhoupengjie/Wakuwaku#19). The
  // overhang cut off (style.css body.home-taskbar); a few px at most, else
  // it is the window being resized under a size told before.
  let winPx = null
  function fitPage() {
    const root = document.documentElement
    const r = root.getBoundingClientRect()
    const dpr = devicePixelRatio || 1
    const over = (have, want) => (have - want / dpr > 0.01 && have - want / dpr < 4 ? have - want / dpr : 0)
    root.style.setProperty('--page-cut-x', `${winPx ? over(r.width, winPx[0]) : 0}px`)
    root.style.setProperty('--page-cut-y', `${winPx ? over(r.height, winPx[1]) : 0}px`)
  }
  window.pet.onStrip(({ thick, win, sf } = {}) => {
    if (!(thick > 0)) return
    stripThick = thick
    if (Number.isFinite(sf) && sf > 0) core.stripScale = sf
    if (Array.isArray(win) && win[0] > 0 && win[1] > 0) winPx = win
    setStrip()
  })
  window.addEventListener('resize', () => {
    if (!stripThick) return
    document.documentElement.style.setProperty('--strip-h', `${stripThick / (devicePixelRatio || 1)}px`)
    fitPage()
    if (window.Settings?.isOpen?.()) window.Settings.redraw()
    sendSolid()
  })

  const micaBox = el('div')
  micaBox.id = 'mica'
  island.parentElement.prepend(micaBox)
  const mica = Mica.mount(micaBox)
  let paper = null

  // The settings as Windows' own Settings in every home, her island's too:
  // its light panel in Windows' light mode (settings.css set-light), its
  // dark greys in the dark. They were the island's own (its ground, its
  // greys) at the top of the screen and over a black strip, Windows' on a
  // strip of mica, clear or light: two looks for the one page (the user,
  // 2026-11-08: 灵动岛设置不同模式都应该统一). The island's own look stays
  // its capsule's and what opens in it.
  const isWinLight = () => isOn() && core.look.mode === 'light'
  function setSettingsLook() {
    body.classList.toggle('set-light', isWinLight())
    setInk()
  }

  function drawMica() {
    const clearMica = isClear() && clearWhen() === 'mica'
    body.classList.toggle('tb-mica', isMica())
    body.classList.toggle('tb-light', isLight())
    body.classList.toggle('set-light', isWinLight())
    body.classList.toggle('tb-ink-light', isStripLight())
    setAccent('--strip-accent', Mica.accentShade(core.look.accent, isStripLight()))
    body.classList.toggle('tb-capsule', isCapsule())
    body.classList.toggle('tb-clear', isClear())
    body.classList.toggle('tb-clear-mica', clearMica)
    body.classList.toggle('tb-filled', isFilled())
    body.classList.toggle('tb-solid', isFilled() && clearWhen() === 'solid')
    if (isMica() || clearMica) mica.set(paper, isSide() ? TASKBAR_H : innerWidth, isSide() ? innerHeight : stripH(), core.look.mode)
    else mica.clear()
    drawIslandLook()
    measureBackdrop()
    inkParts()
    // The strip's words turned light or dark (a window maximized over the
    // clear strip fills it in, back to Windows' mode): what is drawn with
    // their colour (stripHue, the widgets' cards) drawn again.
    const light = isStripLight()
    if (light !== stripWasLight) {
      stripWasLight = light
      requestAnimationFrame(() => update())
    }
  }
  let stripWasLight = null

  // Each of the strip's parts its own words (style.css ink-light), and
  // Windows' accent in their shade (--strip-accent), by the desktop right
  // under it on the clear strip (mica.js inkOn): the clock over a dark
  // corner of a picture light in the middle keeps light words, the
  // programs' buttons over the light middle dark ones. Elsewhere, or while
  // it is not known, all as the strip's (isStripLight). Done again once
  // they are laid out (fillTaskbar), the middle run has slid (alignApps)
  // or the picture is known; a widget's face is drawn in its part's
  // (drawEntries), so it is drawn again when that turns. The glide over a
  // part as it is.
  const stripParts = () => [...barRest.querySelectorAll(':scope > .tzone > [data-part]'), deskEdge]
  function partLight(node) {
    if (!core.backdrop || !isClear() || isFilled()) return isStripLight()
    const strip = island.parentElement.getBoundingClientRect()
    const r = node.getBoundingClientRect()
    const [from, to] = isSide() ? [r.top - strip.top, r.bottom - strip.top] : [r.left - strip.left, r.right - strip.left]
    return Mica.stripInkLight({ light: isLight(), clear: true, filled: false, backdrop: Mica.inkOn(core.backdrop, from, to) })
  }
  const inkOf = node => (node.closest('[data-part]') || node).classList.contains('ink-light')
  function inkParts() {
    if (!core.taskbarBuilt) return
    let turned = false
    for (const node of stripParts()) {
      const light = partLight(node)
      if (!node._inked || node.classList.contains('ink-light') !== light) {
        node.classList.toggle('ink-light', light)
        node._inked = true
        turned = true
      }
      setAccent('--strip-accent', Mica.accentShade(core.look.accent, light), node)
    }
    if (core.glideOn) glide.classList.toggle('ink-light', inkOf(core.glideOn))
    if (turned) drawEntries()
  }

  // The desktop under the clear strip looked at again when the picture or
  // the strip's size changes; drawn anew once it is known.
  let backdropKey = ''
  function measureBackdrop() {
    if (!isClear()) return
    const width = isSide() ? TASKBAR_H : innerWidth
    const height = isSide() ? innerHeight : stripH()
    const key = JSON.stringify([paper, width, height])
    if (!paper || key === backdropKey) return
    backdropKey = key
    Mica.lightness(paper, width, height, isSide()).then(profile => {
      if (key !== backdropKey) return
      if (profile) {
        const ys = profile.ys
        console.log(`taskbar: the desktop under the clear strip ${profile.mean.toFixed(3)} light (${Math.min(...ys).toFixed(3)} to ${Math.max(...ys).toFixed(3)} along it), its words ${Mica.wantsDarkInk(profile.mean) ? 'dark' : 'light'} as a whole`)
      } else console.log('taskbar: the desktop under the clear strip not read')
      core.backdrop = profile
      drawMica()
    })
  }

  // Windows' mode: as taskbar/ tells the taskbar (taskbar:look), and in
  // pet:update at the top of the screen (winLight), where there is no
  // taskbar to hear it from.
  const modeNow = () => (home() === 'taskbar' ? core.look.mode : core.now.winLight === true ? 'light' : 'dark')
  // The look as the settings give it now (none: the defaults): on the
  // taskbar where her end is a capsule (islandLook), at the top of the
  // screen its own (islandTopLook, zhoupengjie/Wakuwaku#27); and its
  // ground's colour (null: none given), how solid (0.2 to 1; solid when no
  // colour is given) and blurred.
  const lookNow = () => (home() === 'taskbar' ? (isCapsule() && core.config.islandLook?.[core.look.mode]) || {} : core.config.islandTopLook?.[modeNow()] || {})
  function groundOf(mine) {
    const bg = /^#[0-9a-f]{6}$/i.test(mine.bg || '') ? parseInt(mine.bg.slice(1), 16) : null
    const op = bg !== null && Number.isFinite(mine.op) ? Math.min(100, Math.max(20, mine.op)) / 100 : 1
    const blur = Number.isFinite(mine.blur) ? Math.min(60, Math.max(0, mine.blur)) : 0
    return { bg, op, blur }
  }

  // Her island's look on the taskbar, as the settings may set it for each of
  // Windows' modes (islandLook: { dark, light }, each { bg: '#rrggbb', op: 20
  // to 100, blur: 0 to 60, edge: 'line' | 'ring' | 'none', bare }), over
  // Windows' own flyout grey (style.css --i-bg, --i-blur, --i-edge,
  // island-bare); a part left out keeps its default. The ring is the
  // session's colour (--ring, as the island draws). At the top of the
  // screen the same from its own (islandTopLook), over black; there the
  // page has nothing behind the island to blur: only Windows' acrylic
  // blurs it (placePlate).
  function drawIslandLook() {
    const mine = lookNow()
    const { bg, op, blur } = groundOf(mine)
    const rgba = a => `rgba(${(bg >> 16) & 255}, ${(bg >> 8) & 255}, ${bg & 255}, ${a})`
    if (bg === null) body.style.removeProperty('--i-bg')
    else body.style.setProperty('--i-bg', rgba(op))
    // Deeper over a maximized window, with nothing to blur it (setDeep).
    if (bg === null || op >= DEEP_OP) body.style.removeProperty('--i-bg-deep')
    else body.style.setProperty('--i-bg-deep', rgba(DEEP_OP))
    // The rim over the acrylic's edge (sendPlate): three quarters of the
    // way from the ground's opacity to solid, half that as it fades in.
    const rim = op + (1 - op) * 0.75
    if (bg === null || op >= 1) {
      body.style.removeProperty('--i-rim')
      body.style.removeProperty('--i-rim-half')
    } else {
      body.style.setProperty('--i-rim', rgba(rim.toFixed(3)))
      body.style.setProperty('--i-rim-half', rgba((rim / 2).toFixed(3)))
    }
    if (blur && home() === 'taskbar') island.style.setProperty('--i-blur', `blur(${blur}px) saturate(1.25)`)
    else island.style.removeProperty('--i-blur')
    const edge = {
      ring: '0 0 0 1.5px var(--ring), 0 0 12px color-mix(in srgb, var(--ring) 35%, transparent)',
      none: '0 0 0 0 transparent',
    }[mine.edge]
    if (edge) island.style.setProperty('--i-edge', edge)
    else island.style.removeProperty('--i-edge')
    // Bare unless said otherwise in the dark mode (the default since
    // 2026-10-13, settings.js ILOOK_DEFAULT), not in the light.
    body.classList.toggle('island-bare', isCapsule() && (mine.bare ?? core.look.mode === 'dark') === true)
    core.groundLight = bg === null ? null : lightness(bg) > 0.35
    setInk()
  }

  // A colour's relative lightness (WCAG), 0 to 1.
  function lightness(rgb) {
    const [r, g, b] = [(rgb >> 16) & 255, (rgb >> 8) & 255, rgb & 255].map(v => {
      v /= 255
      return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4
    })
    return 0.2126 * r + 0.7152 * g + 0.0722 * b
  }

  // Her island's words dark (style.css isl-light, hue, ink) on a light
  // ground: Windows' light mode's, or a light one of the settings' own
  // (groundLight; null: none given); in the settings, theirs, as Windows'
  // mode in every home (setSettingsLook).
  function setInk() {
    const own = core.view === 'settings' ? null : core.groundLight
    const was = core.inkLight
    core.inkLight = own === null ? (core.view === 'settings' ? isWinLight() : isLight()) : own
    body.classList.toggle('isl-light', core.inkLight)
    // A card's plugin view told her theme, and its card's colours.
    if (was !== core.inkLight) islandViews?.relook(core.allWidgets)
  }

  window.pet.onWallpaper(p => {
    paper = p
    drawMica()
  })

  // Windows' mode and accent colour (taskbar/). The accent as its own
  // taskbar draws with it: a lighter one on the dark strip, a darker one on
  // the light (Mica.accentShade). --win-accent by Windows' mode, for the
  // settings and what opens from the strip; --strip-accent by the strip's
  // own words (drawMica), for what is drawn on it: the line under the window
  // in front, a program's count, the privacy marks, where a dragged button
  // lands; on each part, by its own (inkParts). Their own blue until it
  // comes. Not --accent: that is her mood's colour (pet.js).
  function setAccent(name, colour, on = document.documentElement) {
    if (colour) on.style.setProperty(name, colour)
    else on.style.removeProperty(name)
  }

  window.pet.onLook(l => {
    const seconds = core.look.seconds
    core.look = { mode: l?.mode === 'light' ? 'light' : 'dark', accent: l?.accent || null, endTask: l?.endTask === true, seconds: l?.seconds === true, taskView: l?.taskView === true }
    if (core.look.seconds !== seconds) clockLoop()
    // The settings by Windows' mode in every home, now (setSettingsLook).
    setSettingsLook()
    // How many times a button flashes for attention (style.css .flash).
    if (l?.flashes !== undefined) barRest.style.setProperty('--flashes', l.flashes === 0 ? 'infinite' : String(l.flashes))
    setAccent('--win-accent', Mica.accentShade(core.look.accent, core.look.mode === 'light'))
    setAccent('--strip-accent', Mica.accentShade(core.look.accent, isStripLight()))
    if (isOn() && home() === 'taskbar') {
      drawMica()
      update()
    }
  })
  window.addEventListener('resize', drawMica)

  // The program in front (frontBox): the one whose window is in front; none
  // (her own, the desktop, a window with no button), the last one while it
  // is still there, else the desktop.
  function drawFront() {
    const app = core.taskWindows.find(a => (a.windows || []).some(w => w.front)) || core.taskWindows.find(a => a.app === core.frontWas)
    core.frontWas = app?.app ?? null
    const win = app && ((app.windows || []).find(w => w.front) || (app.windows || [])[0])
    const name = app ? app.name || '' : t(core.lang, 'taskbar.frontDesktop')
    if (frontName.textContent !== name) frontName.textContent = name
    frontIcon.hidden = !app?.png
    if (app?.png && frontIcon.getAttribute('src') !== app.png) frontIcon.src = app.png
    frontBox.title = win?.title || name
  }

  window.pet.onWindows(list => {
    core.taskWindows = Array.isArray(list) ? list : []
    if (isOn() && home() === 'taskbar') {
      drawFront()
      drawWindows()
      alignApps()
      snapStart()
      drawMica()
      setDeep(true)
    }
  })

  // The time and the date, as Windows' taskbar has them: with the seconds
  // while Windows' own clock shows them (taskbar/win/look.rs seconds_on), the time
  // anew at each second's start then, else every five seconds. Its digits
  // as wide as each other (style.css .tclock), so the tray beside it keeps
  // still as they change.
  let clockTick = 0
  function tickClock() {
    const d = new Date()
    const time = d.toLocaleTimeString(core.lang === 'zh' ? 'zh-CN' : undefined, { hour: '2-digit', minute: '2-digit', second: core.look.seconds ? '2-digit' : undefined })
    // On the slim top bar one line, as a menu bar's: the date with its weekday, then the time.
    const slim = isSlim()
    const date = d.toLocaleDateString(core.lang === 'zh' ? 'zh-CN' : undefined, slim ? { month: 'long', day: 'numeric', weekday: 'short' } : isSide() ? { month: 'numeric', day: 'numeric' } : { year: 'numeric', month: 'numeric', day: 'numeric' })
    if (clockBox.dataset.at !== time + date) {
      clockBox.dataset.at = time + date
      clockBox.replaceChildren(...(slim ? [el('i', '', date), el('b', '', time)] : [el('b', '', time), el('i', '', date)]))
    }
    clockBox.classList.toggle('secs', core.look.seconds === true)
    // The tip each minute, not each second.
    const minute = `${date} ${d.getHours()}:${d.getMinutes()}`
    if (clockBox.dataset.tipAt !== minute) {
      clockBox.dataset.tipAt = minute
      tipClock()
    }
  }
  function clockLoop() {
    clearTimeout(clockTick)
    if (isOn() && home() === 'taskbar') tickClock()
    clockTick = setTimeout(clockLoop, core.look.seconds ? 1000 - (Date.now() % 1000) : 5000)
  }
  clockLoop()
  // Its tip as Windows' own (zhoupengjie/Wakuwaku#36, clock.rs): the date in
  // full with its weekday, in the person's own format, and Windows'
  // additional clocks; read anew each minute and as the pointer comes.
  function tipClock() {
    window.pet.taskbar.clockTip().then(tip => {
      if (typeof tip === 'string' && tip) clockBox.title = tip
    })
  }
  clockBox.addEventListener('pointerenter', tipClock)

  // The tray, as Windows' has it: the icons kept out on the taskbar, and
  // the rest folded away behind ^, open above it on a press. Where each is
  // kept: the person's choice (dragged from one to the other, trayPinned),
  // else Windows' own (windowsOut). In what order: as dragged (trayOrder,
  // names, those not running kept too), the rest after, as they came.
  const isOut = i => core.config.trayPinned?.[i.name] ?? i.windowsOut === true
  const trayOrder = () => (Array.isArray(core.config.trayOrder) ? core.config.trayOrder.map(String) : [])
  function orderedTray() {
    const order = trayOrder()
    const rank = i => (order.includes(i.name) ? order.indexOf(i.name) : Infinity)
    return core.trayIcons
      .map((i, n) => [i, n])
      .sort((a, b) => rank(a[0]) - rank(b[0]) || a[1] - b[1])
      .map(([i]) => i)
  }
  const chevron = el('span', 'tchev')
  chevron.append(el('span', 'fi', ''))
  chevron.setAttribute('role', 'button')
  const outBox = el('span', 'tout')
  const flyout = el('span', 'tfly')

  // One a press does not reach (reach false: its program handed its icon to
  // Windows' own taskbar only, before hers was up, and never named hers the
  // message to send it; systray.rs) says so in its tip.
  function trayIcon(i) {
    const slot = el('span', 'ticon')
    slot.dataset.tray = i.key
    slot.dataset.name = i.name
    const unreached = i.reach === false ? `\n${t(core.lang, 'taskbar.trayUnreached')}` : ''
    slot.title = (i.tip || i.exe) + unreached
    slot.setAttribute('role', 'button')
    slot.setAttribute('aria-label', (i.tip || i.name || i.exe || '') + unreached)
    if (i.png) {
      const img = el('img')
      img.src = i.png
      img.draggable = false
      slot.append(img)
      markMono(slot, img)
    }
    return slot
  }

  // A tray icon in one colour only, white or black, as some programs and
  // Windows' own (safely remove hardware, the microphone in use) draw them
  // for one kind of taskbar whatever Windows' mode: marked (data-mono), so
  // the other kind turns it over to read (style.css). Each picture looked at
  // once.
  const monoOf = new Map()
  function markMono(slot, img) {
    const mark = mono => {
      if (mono) slot.dataset.mono = mono
      else delete slot.dataset.mono
    }
    if (monoOf.has(img.src)) return mark(monoOf.get(img.src))
    const look = () => {
      const c = document.createElement('canvas')
      c.width = img.naturalWidth
      c.height = img.naturalHeight
      const g = c.getContext('2d')
      g.drawImage(img, 0, 0)
      const d = g.getImageData(0, 0, c.width, c.height).data
      let seen = 0
      let grey = 0
      let lum = 0
      for (let p = 0; p < d.length; p += 4) {
        if (d[p + 3] < 96) continue
        seen++
        lum += 0.299 * d[p] + 0.587 * d[p + 1] + 0.114 * d[p + 2]
        if (Math.max(d[p], d[p + 1], d[p + 2]) - Math.min(d[p], d[p + 1], d[p + 2]) < 24) grey++
      }
      const mono = seen && grey / seen > 0.9 ? (lum / seen > 200 ? 'light' : lum / seen < 60 ? 'dark' : '') : ''
      if (monoOf.size > 300) monoOf.clear()
      monoOf.set(img.src, mono)
      mark(mono)
    }
    if (img.complete && img.naturalWidth) look()
    else img.addEventListener('load', look, { once: true })
  }

  // Starting, the tray kept as wide as it was last time until as many icons
  // are back (the programs add theirs one by one once the tray is ours,
  // seconds after the rest is drawn), or a few seconds: what is left of it
  // never jumps leftwards as the last ones come. Kept here, as it is, while
  // on the taskbar.
  const TRAY_KEPT = 'trayWidth'
  let trayHold = null
  try {
    const kept = JSON.parse(localStorage.getItem(TRAY_KEPT) || 'null')
    if (kept?.w > 0 && kept.w < 2000 && kept.n > 0) trayHold = kept
  } catch {}
  let trayKept = ''
  function holdTray() {
    if (trayHold && (core.trayIcons.length >= trayHold.n || performance.now() > 8000)) trayHold = null
    trayBox.style.minWidth = trayHold ? `${trayHold.w}px` : ''
    trayBox.style.justifyContent = trayHold ? 'flex-end' : ''
    if (trayHold) return
    requestAnimationFrame(() => {
      if (home() !== 'taskbar' || !core.trayIcons.length) return
      const w = Math.round(trayBox.getBoundingClientRect().width)
      const kept = JSON.stringify({ w, n: core.trayIcons.length })
      if (w <= 0 || kept === trayKept) return
      trayKept = kept
      try {
        localStorage.setItem(TRAY_KEPT, kept)
      } catch {}
    })
  }
  setTimeout(holdTray, 8100)

  function drawTray() {
    const icons = orderedTray()
    const folded = icons.filter(i => !isOut(i))
    if (!folded.length) core.trayOpen = false
    const drawn = JSON.stringify([icons.map(i => [i.key, i.png.length, i.png.slice(-24), i.tip, isOut(i), i.reach]), core.trayOpen])
    if (drawn !== core.trayDrawn) {
      core.trayDrawn = drawn
      chevron.hidden = !folded.length
      chevron.classList.toggle('open', core.trayOpen)
      chevron.setAttribute('aria-label', t(core.lang, 'taskbar.showHidden'))
      chevron.setAttribute('aria-expanded', String(core.trayOpen))
      outBox.replaceChildren(...icons.filter(isOut).map(trayIcon))
      flyout.replaceChildren(...folded.map(trayIcon))
      flyout.hidden = !core.trayOpen
      if (trayBox.firstChild !== flyout) trayBox.replaceChildren(flyout, chevron, outBox)
    }
    holdTray()
    requestAnimationFrame(sendTrayRects)
  }

  function openTray(open) {
    if (core.trayOpen === open) return
    core.trayOpen = open
    trayBox.classList.toggle('upper', isSide() && trayBox.getBoundingClientRect().top < innerHeight / 2)
    drawTray()
    watchPresses()
  }
  chevron.addEventListener('click', () => openTray(!core.trayOpen))
  // Folded away again once the pointer has been gone from it a moment.
  let trayCloseTimer
  trayBox.addEventListener('mouseleave', () => {
    clearTimeout(trayCloseTimer)
    trayCloseTimer = setTimeout(() => openTray(false), 1200)
  })
  trayBox.addEventListener('mouseenter', () => clearTimeout(trayCloseTimer))

  // Where each icon is, for the programs that ask (taskbar/): a folded
  // one, while folded, is where ^ is.
  function sendTrayRects() {
    if (home() !== 'taskbar') return
    const at = node => {
      const r = node.getBoundingClientRect()
      return [r.left, r.top, r.width, r.height]
    }
    const rects = core.trayIcons.map(i => {
      const shown = trayBox.querySelector(`[data-tray="${i.key}"]`)
      const visible = shown && (isOut(i) || core.trayOpen)
      return [Number(i.key), ...at(visible ? shown : chevron)]
    })
    const sent = JSON.stringify(rects)
    if (sent === core.trayRectsSent) return
    core.trayRectsSent = sent
    window.pet.taskbar.trayRects(rects)
  }
  window.addEventListener('resize', () => {
    core.trayRectsSent = ''
    requestAnimationFrame(sendTrayRects)
  })

  // The icons' pictures kept here: taskbar/ (send_icons) sends each once,
  // and again only when it is another; a list without one's png means the
  // one kept.
  const trayPngs = new Map()
  window.pet.onTray(list => {
    core.trayIcons = (Array.isArray(list) ? list : []).map(i => {
      if (i.png === undefined) i.png = trayPngs.get(i.key) ?? ''
      trayPngs.set(i.key, i.png)
      return i
    })
    for (const key of [...trayPngs.keys()]) if (!core.trayIcons.some(i => i.key === key)) trayPngs.delete(key)
    if (isOn() && home() === 'taskbar') drawTray()
  })

  return { chevron, drawFront, drawMica, drawTray, flyout, groundOf, inkOf, inkParts, isOut, lookNow, modeNow, openTray, orderedTray, outBox, sendTrayRects, setInk, setPeek, tickClock, trayOrder }
}
