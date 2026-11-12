// The taskbar's layout (taskbarLayout): its parts in three runs along the
// strip, her seat, the widgets on it, and several in one place (the spec's
// §5): stacks and carousels taking turns.
// Made by island.js in the section's place as the page starts
// (layoutPart(core), imported by island.js): what it takes of the island's core
// comes in core, its functions and constants as they are, its lets read
// and written through core.<name> as they change; what it hands back is the
// island's again.
import * as WCards from './wcards.js'
import * as Widgets from './widgets.js'
import { Picker } from './pages.js'
import { pet } from './bridge.js'

export function layoutPart(core) {
  const {
    CAPSULE_X, CELL_W, CLIPS, MOOD_CLIP, PER_PX, TASKBAR_GAP, Widgets, barRest, body, clockBox, deskEdge, el,
    face, facesBox, frontBox, growNode, home, isCapsule, isDetailed, isLight, isOn, isSide, isSlim, isSplit,
    isStripLight, island, keysBox, len, lo, onPixels, privBox, quickBox, sheetImage, showFrame, sizeOf,
    snapStart, startButton, startPart, t, takeWidgets, taskCompactH, taskViewButton, trayBox, widgetFaces,
    windowsBox,
  } = core

  // Its parts in three runs, as a KDE panel has them: from the left end, in
  // the middle of the screen (moved left when crowded, alignApps), and up to
  // the right end; show-the-desktop's sliver last of all, always (Windows'
  // corner). Her island is one of the parts ('her'): a slot in its run with
  // the island laid over it (placeSlot), growing from it the way there is
  // room (body.isl-right: from its right edge, leftwards). Edited by
  // dragging the parts about on the strip itself (editTaskbar).
  const TB_PARTS = ['her', 'start', 'apps', 'monitor', 'tray', 'keys', 'quick', 'clock', 'front']
  const TB_RIGHT = ['monitor', 'tray', 'keys', 'quick', 'clock']
  // Those that may be taken off the strip (the rest stay: the ghost says so).
  const TB_OPTIONAL = ['monitor', 'front']
  const TB_PRESETS = {
    win11: { left: ['her'], center: ['start', 'apps'], right: TB_RIGHT },
    left: { left: ['her', 'start', 'apps'], center: [], right: TB_RIGHT },
    herRight: { left: ['start', 'apps'], center: [], right: [...TB_RIGHT, 'her'] },
  }
  // Split, the top bar's: no Start, no programs (they are in the dock), her
  // in the middle (docs/design/specs/dock-topbar.md §2); quick settings and
  // the clock in the dock too if set so (splitKeys dock, bar.js).
  const inDock = () => ['start', 'apps', ...(core.config.splitKeys === 'dock' ? ['quick', 'clock'] : [])]
  const splitParts = () => TB_PARTS.filter(p => !inDock().includes(p))
  const splitPreset = () => ({ left: ['front'], center: ['her'], right: TB_RIGHT.filter(p => !inDock().includes(p)) })
  // On the top bar the program in front is always there (it may be moved).
  const SPLIT_OPTIONAL = TB_OPTIONAL.filter(p => p !== 'front')
  const herSlot = el('span', 'tslot')
  // The tray's icons and the marks of what uses the microphone, camera or
  // location: one part.
  const trayPart = el('span', 'tpair')
  trayPart.append(trayBox, privBox)
  const PART_NODES = { her: herSlot, start: startPart, apps: windowsBox, monitor: facesBox, tray: trayPart, keys: keysBox, quick: quickBox, clock: clockBox, front: frontBox }
  for (const [part, node] of Object.entries(PART_NODES)) node.dataset.part = part
  const zones = { left: el('span', 'tzone tz-left'), center: el('span', 'tzone tz-center'), right: el('span', 'tzone tz-right') }
  let seatSent = null

  // The layout: the one being edited, or as kept, or before any as
  // taskbarAlign had it; each part once, one it lacks (new since) where the
  // default has it; the widgets put there (widgets.js layoutOf). No gear
  // (zhoupengjie/Wakuwaku#38): the settings are a press on her capsule, or
  // her menus', away, as Windows 11's taskbar has none; one kept in an old
  // layout is left out, as a part no longer there.
  // The monitor's part may be taken off (#53): dragged off the strip it is
  // gone from the layout, not put back as a part new since; a monitor put
  // there as a widget is that part (widgets.js monitorAsPart).
  // The one kept is the strip's and the top bar's both: for the top bar
  // (split) Start and the programs left out and the program in front put
  // first (widgets.js splitOf); kept from there, with neither Start nor the
  // programs in it, for the strip along an edge the program in front taken
  // out again, the top bar's own, and her back at the left's start from the
  // middle (edgeOf). Else, on a return to the taskbar, the program in front
  // stayed at the strip's left end, before her, moved to the middle, with
  // Start and the programs after her (the user, 2026-11-07).
  function tbLayout(saved = core.editLayout || core.config.taskbarLayout) {
    const base = saved && typeof saved === 'object' ? saved : core.config.taskbarAlign === 'left' ? TB_PRESETS.left : TB_PRESETS.win11
    if (isSplit()) return Widgets.monitorAsPart(Widgets.layoutOf(Widgets.splitOf(base, inDock()), splitParts(), splitPreset(), SPLIT_OPTIONAL))
    return Widgets.monitorAsPart(Widgets.layoutOf(Widgets.edgeOf(base), TB_PARTS, TB_PRESETS.win11, TB_OPTIONAL))
  }
  // Whether the strip has the monitor's part.
  const hasMonitorPart = (layout = tbLayout()) => Object.values(layout).some(run => run.includes('monitor'))
  // Which preset its parts are as ('custom': none), whatever widgets are there.
  const presetOf = layout => {
    const parts = JSON.stringify(Widgets.partsAlone(tbLayout(layout)))
    return Object.keys(TB_PRESETS).find(k => JSON.stringify(tbLayout(TB_PRESETS[k])) === parts) || 'custom'
  }

  // The widgets put there, each its node, kept while it stays there (by its
  // id); dragged by its id as a part by its name while the layout is edited.
  const entryNodes = new Map()
  function entryNode(e) {
    let node = entryNodes.get(e.id)
    if (node && node._type !== e.type) {
      entryNodes.delete(e.id)
      node = null
    }
    if (!node) {
      node = el('span', `tw tw-${e.type}${Widgets.isGroup(e) ? ' tw-group' : ''}`)
      node.dataset.part = e.id
      node.dataset.entry = e.id
      node._type = e.type
      entryNodes.set(e.id, node)
    }
    node._entry = e
    // Several in one place are medium (a side bar's: small).
    // Along a side, and on the thin top bar (split), small.
    if (Widgets.isSource(e) || Widgets.isGroup(e)) node.dataset.size = isSide() || isSlim() || (e.size === 'S' && !Widgets.isGroup(e)) ? 'S' : 'M'
    return node
  }

  // The parts and widgets into their runs, as the layout has them.
  function arrangeTaskbar() {
    const layout = tbLayout()
    const key = JSON.stringify(layout)
    if (key === core.arranged) return
    core.arranged = key
    for (const k of Object.keys(zones)) zones[k].replaceChildren(...layout[k].map(p => (typeof p === 'string' ? PART_NODES[p] : entryNode(p))))
    const ids = new Set(Object.values(layout).flatMap(run => run.filter(p => typeof p !== 'string').map(p => p.id)))
    for (const id of [...entryNodes.keys()]) if (!ids.has(id)) entryNodes.delete(id)
    // First at the strip's left end: her island at its very edge, as ever.
    herSlot.classList.toggle('first', layout.left[0] === 'her')
    // A run with a flexible spacer takes the room there is (the middle one
    // not in the middle of the screen then: its spacer has the say).
    const stretch = k => layout[k].some(p => p.type === 'flex')
    for (const k of Object.keys(zones)) zones[k].classList.toggle('stretch', stretch(k))
    barRest.classList.toggle('tb-stretch', Object.keys(zones).some(stretch))
    // What the strip shows takes no turns in the island too.
    takeWidgets(core.allWidgets)
  }

  // Each widget put there, as big as it is set and it has (widgets.js
  // sizesOf: a countdown is small alone): S its glyph and value; M its value
  // over its name, as the clock has the date under the time (wcards.js
  // faceHTML: the glyph white or black as the tray's icons, only ups and
  // downs in colour). One with nothing to say greyed, with its name; just
  // put there with its plugin starting, a shimmer for a few seconds; its
  // plugin stopped, an amber glyph and why. A plugin with several (stocks,
  // CI runs) shows them in turn. Several in one place: a carousel (in turn,
  // drawGroup) or a stack (the first, its others opened over it).
  const ENTRY_TURN_MS = 5000
  // What the widgets on the strip are drawn with (wcards.js ctx), as its
  // words are; what opens from them (a card, a stack, a menu), as Windows'
  // mode, as their ground is.
  const barCtx = () => ({ lang: core.lang, light: isStripLight(), detailed: isDetailed(), width: core.STACK_CARD_W, hour: new Date().getHours(), now: Date.now() })
  const popCtx = () => ({ ...barCtx(), light: isLight() })
  // Placed this long ago at most, a widget with nothing yet is loading.
  const LOADING_MS = 20000
  const placedAt = new Map()
  // Why one shows nothing: its plugin wants its settings, stopped, is
  // starting (just put there), or nothing yet.
  function emptyState(m) {
    const p = m.type === 'plugin' ? pluginsSaid.list.find(x => x.id === m.of) : null
    if (p?.missing) return 'needs'
    if (p?.state === 'stopped') return 'error'
    const at = placedAt.get(Widgets.memberKey(m))
    if ((p?.state === 'running' || (m.of === 'monitor' && monitorWanted())) && at && Date.now() - at < LOADING_MS) return 'loading'
    return 'none'
  }
  const monitorWanted = () => core.allWidgets.some(w => w.id === 'monitor')
  // Its size as drawn: M where it has a medium size, else S.
  const sizeFor = (size, w) => (size === 'M' && (!w || Widgets.sizesOf(w).includes('m')) ? 'M' : 'S')

  function drawEntries() {
    const turn = Math.floor(Date.now() / ENTRY_TURN_MS)
    const detailed = isDetailed()
    const strip = barCtx()
    for (const node of entryNodes.values()) {
      // In its part's words (inkParts), dark or light by what is under it;
      // the strip's until it has been placed.
      const ctx = { ...strip, light: node._inked ? node.classList.contains('ink-light') : strip.light }
      const e = node._entry
      const name = Widgets.nameOf(core.lang, e, core.allWidgets, detailed)
      node.dataset.label = name
      if (Widgets.isGroup(e)) {
        drawGroup(node, e, ctx)
        continue
      }
      if (!Widgets.isSource(e)) continue
      const from = Widgets.sourcesOf(e, core.allWidgets)
      const w = from.length ? from[turn % from.length] : null
      // A plugin with nothing to say: maybe it wants its settings first.
      if (!w && e.type === 'plugin') askPlugins()
      const state = w ? '' : emptyState(e)
      const size = sizeFor(node.dataset.size, w)
      const drawn = JSON.stringify([size, name, w && [w.label, w.value, w.detail, w.color, w.private], state, ctx.light, detailed, core.lang])
      if (node._drawn === drawn) continue
      node._drawn = drawn
      node.classList.toggle('empty', !w)
      node.classList.toggle('S', size === 'S')
      const said = w ? Widgets.words(core.lang, w, detailed) : null
      node.title = w ? [said.label, said.value].filter(Boolean).join(' · ') : `${name} · ${t(core.lang, state === 'needs' ? 'taskbar.w.needs' : 'taskbar.w.none')}`
      node.innerHTML = w ? WCards.faceHTML(w, size, ctx) : WCards.emptyFaceHTML(e, name, size, state, ctx)
      fillHer(node, 16)
    }
    if (core.wgFor) core.drawWidgetPop()
    if (core.stackFor) core.drawStackPop()
    tickCarousels()
  }

  // Her face where a widget's glyph is hers (wcards.js .wher: Claude 今天),
  // still: one frame of her at rest, never played (a portrait that plays
  // keeps a timer going, playPortraits).
  function fillHer(root, px) {
    for (const slot of root.querySelectorAll('.wher:empty')) slot.append(herBadge(px))
  }
  function herBadge(px) {
    if (!core.spriteUrl) return face('idle', px)
    const scale = px * PER_PX
    const node = el('span', 'still-her badge')
    node.style.cssText = `width:${px}px;height:${px}px`
    const sheet = el('span', 'sheet')
    sheet.style.cssText = `transform:translate(${-(CELL_W * scale - px) / 2}px, -2px) scale(${scale})`
    sheetImage(sheet, core.spriteUrl)
    const clip = CLIPS[MOOD_CLIP.idle]
    if (clip) showFrame(sheet, clip.row, 0)
    node.append(sheet)
    return node
  }

  // --- Several in one place (the spec's §5) -------------------------------------------
  // A carousel: one place as wide as its widest member's medium size, its
  // members' widgets in turn, the next sliding up every 6 seconds (half a
  // second, the spec's curve); a column of dots at its right, the one shown
  // long. Not under the pointer (held), not while nobody is about (quiet),
  // never by itself with motion reduced (the wheel turns it then, by a
  // fade); news (a letter come) cuts in for a turn (widgets.js
  // carouselTurn). One timer for them all, only while one may turn.
  // A stack: the first shown (medium), two thin edges over it for the
  // others; a press opens them all above it (drawStackPop).
  const SLIDE_H = 40
  const carousels = new Map()
  let carouselTimer = 0
  const stillMotionNow = () => matchMedia('(prefers-reduced-motion: reduce)').matches

  function drawGroup(node, e, ctx) {
    const slides = Widgets.slidesOf(e, core.allWidgets)
    const size = node.dataset.size === 'S' ? 'S' : 'M'
    const face = s => (s.w ? WCards.faceHTML(s.w, sizeFor(size, s.w), ctx) : WCards.emptyFaceHTML(s.member, memberName(s.member), size, emptyState(s.member), ctx))
    const drawn = JSON.stringify([e.type, size, slides.map(s => (s.w ? [s.w.id, s.w.label, s.w.value, s.w.detail, s.w.color, s.w.private] : [Widgets.memberKey(s.member), emptyState(s.member)])), ctx.light, ctx.detailed, core.lang])
    // A plugin's member with nothing (its settings first?).
    if (slides.some(s => !s.w && s.member.type === 'plugin')) askPlugins()
    if (e.type === 'stack') {
      node.classList.toggle('open', core.stackFor === e.id)
      if (node._drawn === drawn) return
      node._drawn = drawn
      const top = slides[0]
      node.classList.toggle('empty', !top?.w)
      node.title = t(core.lang, 'taskbar.w.stackTip', { n: slides.length })
      node.innerHTML = top ? face(top) : ''
      fillHer(node, 16)
      return
    }
    // A carousel: its slides, then them again (it always turns upwards:
    // from the last to the first it goes on into the second run, then is put
    // back where the first run has it), and the dots.
    const state = carousels.get(e.id) || Widgets.carouselStart(Date.now())
    carousels.set(e.id, state)
    if (node._drawn !== drawn) {
      node._drawn = drawn
      // News: a mailbox in it whose unread grew since the last look.
      const was = node._words || new Map()
      const news = slides.findIndex(s => s.w && Widgets.newsIn(was.get(s.w.id), s.w))
      node._words = new Map(slides.filter(s => s.w).map(s => [s.w.id, s.w]))
      node._slides = slides
      const html = slides.map(s => `<span class="tws">${face(s)}</span>`).join('')
      node.innerHTML = `<span class="reel">${html}${slides.length > 1 ? html : ''}</span>${slides.length > 1 ? `<span class="dots">${slides.map(() => '<i></i>').join('')}</span>` : ''}`
      node.title = t(core.lang, 'taskbar.w.carouselTip', { n: slides.length })
      fillHer(node, 16)
      state.at = Math.min(state.at, Math.max(0, slides.length - 1))
      showSlide(node, state.at, false)
      if (news >= 0 && was.size) turnCarousel(e.id, news)
    }
  }

  // The slide shown: the reel moved (sliding, or at once), the dot lit.
  function showSlide(node, at, slide, by = 0) {
    const reel = node.querySelector('.reel')
    const n = node._slides?.length || 0
    if (!reel || !n) return
    for (const [i, dot] of [...node.querySelectorAll('.dots i')].entries()) dot.classList.toggle('on', i === at)
    const from = node._at ?? at
    node._at = at
    if (!slide || stillMotionNow()) {
      reel.style.transition = 'none'
      reel.style.transform = `translateY(${-at * SLIDE_H}px)`
      void reel.offsetHeight
      reel.style.transition = ''
      // By hand with motion reduced: a fade, not a slide (the spec's §9).
      if (slide) reel.animate([{ opacity: 0 }, { opacity: 1 }], { duration: 200, easing: 'ease-out' })
      return
    }
    // Upwards always: past the last into the second run, then back.
    const target = at > from || by > 0 ? (at > from ? at : at + n) : at
    reel.style.transform = `translateY(${-target * SLIDE_H}px)`
    if (target >= n) {
      clearTimeout(node._settle)
      node._settle = setTimeout(() => showSlide(node, at, false), CAROUSEL_SLIDE_MS + 40)
    }
  }
  const CAROUSEL_SLIDE_MS = 500

  // One turned now: by news (to it), or by hand (by), else its next.
  function turnCarousel(id, news = -1, by = 0) {
    const node = entryNodes.get(id)
    const state = carousels.get(id)
    const n = node?._slides?.length || 0
    if (!node || !state || n < 2) return
    const now = Date.now()
    const next = by ? Widgets.carouselStep(state, n, by, now) : Widgets.carouselTurn(news >= 0 ? state : { ...state, since: 0 }, n, now, news)
    carousels.set(id, next)
    if (next.at !== state.at) showSlide(node, next.at, true, by || 1)
    tickCarousels()
  }

  // The timer: to the next carousel due, while any may turn by itself.
  function tickCarousels() {
    clearTimeout(carouselTimer)
    carouselTimer = 0
    if (!isOn() || home() !== 'taskbar' || core.quiet || stillMotionNow() || core.editLayout || document.hidden) return
    const now = Date.now()
    let soonest = Infinity
    for (const [id, state] of carousels) {
      const node = entryNodes.get(id)
      if (!node?.isConnected || (node._slides?.length || 0) < 2) {
        if (!node?.isConnected) carousels.delete(id)
        continue
      }
      if (node._held) continue
      const due = Widgets.carouselDue(state, now)
      if (due <= 0) {
        const next = Widgets.carouselTurn(state, node._slides.length, now)
        carousels.set(id, next)
        if (next.at !== state.at) showSlide(node, next.at, true, 1)
        soonest = Math.min(soonest, Widgets.CAROUSEL_MS)
      } else soonest = Math.min(soonest, due)
    }
    if (soonest < Infinity) carouselTimer = setTimeout(tickCarousels, soonest + 20)
  }

  // Held under the pointer; let go, its turn starts over.
  barRest.addEventListener(
    'mouseenter',
    e => {
      const node = e.target.closest?.('.tw-carousel')
      if (node && e.target === node) node._held = true
    },
    true,
  )
  barRest.addEventListener(
    'mouseleave',
    e => {
      const node = e.target.closest?.('.tw-carousel')
      if (!node || e.target !== node) return
      node._held = false
      const state = carousels.get(node.dataset.entry)
      if (state) carousels.set(node.dataset.entry, { ...state, since: Date.now() })
      tickCarousels()
    },
    true,
  )
  // The wheel turns it by hand.
  barRest.addEventListener(
    'wheel',
    e => {
      const node = e.target.closest('.tw-carousel')
      if (!node || core.editLayout) return
      e.preventDefault()
      turnCarousel(node.dataset.entry, -1, e.deltaY > 0 ? 1 : -1)
    },
    { passive: false },
  )
  matchMedia('(prefers-reduced-motion: reduce)').addEventListener('change', () => tickCarousels())
  document.addEventListener('visibilitychange', () => tickCarousels())

  // A member's name, as put there (a plugin's, a widget's label as it was).
  const memberName = m => Widgets.nameOf(core.lang, { ...m, id: 'w0' }, core.allWidgets, isDetailed())
  // The name kept with someone else's plugin as it is put somewhere (her
  // own she knows by their ids): its own, as its widgets say it, for while
  // it has nothing to say.
  function ownName(id) {
    const name = t(core.lang, `w.ex.${id}`) === `w.ex.${id}` ? Widgets.pluginName(core.lang, id, core.allWidgets) : ''
    return name ? { name: [...name].slice(0, 40).join('') } : {}
  }

  // The plugins she runs as settings_get says, and other people's
  // (addons.rs), for which want their settings filled in first (missing: a
  // parameter they need) or were stopped: asked for while a plugin on the
  // strip has nothing to say, twice a minute at most, and once the layout
  // is kept.
  const pluginsSaid = { at: 0, list: [] }
  function askPlugins(now = false) {
    if (!now && Date.now() - pluginsSaid.at < 30000) return
    pluginsSaid.at = Date.now()
    pet.settings.get().then(got => tellPlugins(got && [...(got.plugins || []), ...(got.addons || [])]))
  }
  function tellPlugins(list) {
    if (!Array.isArray(list)) return
    pluginsSaid.list = list
    for (const node of entryNodes.values()) node._drawn = ''
    if (core.taskbarBuilt && home() === 'taskbar') drawEntries()
  }
  const needsSettings = e => e.type === 'plugin' && !!pluginsSaid.list.find(p => p.id === e.of)?.missing
  setInterval(() => {
    if (core.taskbarBuilt && home() === 'taskbar' && entryNodes.size) drawEntries()
  }, ENTRY_TURN_MS)

  // Her island over its slot, as wide as it is: from the strip's left, or,
  // its slot past the middle, from the right (what it grows into goes the
  // way there is room). Where it lies without OLED's shift, which moves
  // them both after. Her seat (where she is reached for, and flies back
  // to) told to main once it moves. (Named placeHer at first, it took the
  // name of the one that shapes her in the island, which was never called
  // again: she was an empty ring, 2026-10-12.)
  function placeSlot() {
    if (isSide()) return placeSideSlot()
    herSlot.style.height = ''
    herSlot.style.marginBottom = ''
    body.classList.remove('isl-low')
    const pad = isCapsule() ? CAPSULE_X : 0
    herSlot.style.width = `${onPixels(sizeOf('compact').width + 2 * pad)}px`
    herSlot.style.marginRight = `${pad ? 2 : 4}px`
    const shifted = (core.oledOn && parseFloat(document.documentElement.style.getPropertyValue('--oled-x'))) || 0
    const r = herSlot.getBoundingClientRect()
    const left = r.left - shifted + pad
    // In the middle run (the top bar's, split: her in the middle) what it
    // grows into grows from her capsule's middle (isl-mid, placeMid).
    const mid = zones.center.contains(herSlot)
    const atRight = !mid && (r.left + r.right) / 2 - shifted > innerWidth / 2
    body.classList.toggle('isl-right', atRight)
    body.classList.toggle('isl-mid', mid)
    core.slotMiddle = mid ? (r.left + r.right) / 2 - shifted : null
    placeMid()
    document.documentElement.style.setProperty('--isl-x', `${onPixels(atRight ? innerWidth - (r.right - shifted - pad) : left)}px`)
    const seat = Math.round(left + taskCompactH() / 2)
    if (seat !== seatSent) {
      seatSent = seat
      pet.seat(seat)
    }
  }
  const midLeft = width => Math.min(Math.max(8, core.slotMiddle - width / 2), innerWidth - width - 8)
  // Its left edge there for the width it has (or is going to: setSize), set
  // as the width is, on the same spring (style.css isl-mid), so it grows and
  // shrinks about its middle all the way.
  function placeMid(width = parseFloat(island.style.width)) {
    if (core.slotMiddle == null || !(width > 0)) return
    document.documentElement.style.setProperty('--isl-mx', `${onPixels(core.view === 'compact' ? core.slotMiddle - width / 2 : midLeft(width))}px`)
  }
  // On a side bar: the slot as tall as her upright capsule, from the bar's
  // top, or from its foot in its lower half (isl-low: what it grows into
  // goes up); her seat her portrait, along the bar from its top.
  function placeSideSlot() {
    const pad = isCapsule() ? CAPSULE_X : 0
    const cap = sizeOf('compact')
    herSlot.style.width = ''
    herSlot.style.marginRight = ''
    herSlot.style.height = `${onPixels(cap.height + 2 * pad)}px`
    herSlot.style.marginBottom = `${pad ? 2 : 4}px`
    const shifted = (core.oledOn && parseFloat(document.documentElement.style.getPropertyValue('--oled-y'))) || 0
    const r = herSlot.getBoundingClientRect()
    const top = r.top - shifted + pad
    const low = (r.top + r.bottom) / 2 - shifted > innerHeight / 2
    body.classList.remove('isl-right')
    body.classList.toggle('isl-low', low)
    document.documentElement.style.setProperty('--isl-y', `${onPixels(low ? innerHeight - (r.bottom - shifted - pad) : top)}px`)
    const seat = Math.round(top + cap.width / 2)
    if (seat !== seatSent) {
      seatSent = seat
      pet.seat(seat)
    }
  }
  // A run grown or shrunk moves her slot in it: the tray's icons come in a
  // few seconds after the strip is drawn, a program adds one, the monitor's
  // numbers get wider. Her island goes with it; it stayed where the slot had
  // been, over the monitor and the tray's chevron, until something drew the
  // strip anew (zhoupengjie/Wakuwaku#98).
  // A run grown or shrunk: the middle one centred again first (alignApps),
  // before the frame is drawn, then her slot placed. The top bar's program
  // in front changes its width with each window (drawFront), and the middle
  // run with her capsule in it went along with it, then slid back.
  const slotWatch = new ResizeObserver(() => {
    if (!core.taskbarBuilt || !isOn() || home() !== 'taskbar') return
    alignApps()
    placeSlot()
  })
  for (const zone of Object.values(zones)) slotWatch.observe(zone)

  // The settings beside a side bar, from her capsule's top (or foot, in the
  // bar's lower half), kept 8 px off the screen's ends.
  function placeSideSettings(height) {
    const at = parseFloat(document.documentElement.style.getPropertyValue('--isl-y')) || TASKBAR_GAP
    const y = Math.max(TASKBAR_GAP, Math.min(at, innerHeight - TASKBAR_GAP - height))
    document.documentElement.style.setProperty('--set-y', `${Math.round(y)}px`)
  }

  // When each window (by its handle) began flashing for attention: its
  // button is made anew with every change to the list, and the flashes
  // carry on from where they were (style.css .flash, --flash-delay)
  // instead of starting over.
  const flashSince = new Map()

  function fillTaskbar() {
    if (!core.taskbarBuilt) {
      // As wide as the strip: her island lies over its slot in it (placeSlot).
      barRest.style.left = '0px'
      barRest.replaceChildren(core.glide, zones.left, zones.center, growNode, zones.right, deskEdge, core.pop, core.wgPop, core.stackPop, core.imePop)
      core.taskbarBuilt = true
      core.trayDrawn = ''
      core.windowsDrawn = ''
    }
    arrangeTaskbar()
    drawEntries()
    // The programs' buttons as icons alone (taskbarButtons), or with their titles.
    barRest.classList.toggle('icons', isSide() || core.config.taskbarButtons !== 'labels')
    startButton.title = t(core.lang, 'taskbar.start')
    taskViewButton.title = t(core.lang, 'taskbar.taskView')
    taskViewButton.hidden = !core.look.taskView
    quickBox.title = t(core.lang, 'taskbar.quick')
    deskEdge.title = t(core.lang, 'taskbar.desktop')
    facesBox.replaceChildren(...widgetFaces())
    core.drawFront()
    core.drawWindows()
    core.drawTray()
    core.tickClock()
    alignApps()
    placeSlot()
    snapStart()
    core.inkParts()
    // Editing: picker.js's popover over its widget, where it is now.
    if (core.editLayout) Picker?.relaid()
  }

  // The middle run in the middle of the screen, as Windows 11 has Start and
  // the programs' buttons (the default layout): moved along by its margin,
  // never back past where it starts nor into the right run (the room before
  // it is all it may take). Moved, it slides; what hangs over a button (its
  // windows' list) is placed again once it is there, and her island too.
  // The room always kept between the middle run and the last (the empty
  // .grow): crowded, the programs give way before it does (style.css).
  const RUNS_APART = 12
  // The middle run's own length as last centred: the same, it is only what
  // lies before it that changed, and it stays where it is, set at once (it
  // slid away with the left run and back on its margin's transition, her
  // capsule in it going to and fro as the program in front changed: the
  // user, 2026-11-04); of another length, it slides to the middle as ever.
  let middleLength = null

  function alignApps() {
    if (!core.taskbarBuilt) return
    core.fadeWins()
    const middle = zones.center
    // Along the bar: its left margin, a side bar's top one (the other none).
    const margin = isSide() ? 'marginTop' : 'marginLeft'
    middle.style[isSide() ? 'marginLeft' : 'marginTop'] = ''
    // Measured by the margin as it is now (it may be sliding), set to the one wanted.
    const target = parseFloat(middle.style[margin]) || 0
    let want = 0
    // Empty, it is still a place to drop a part while the layout is edited;
    // with a flexible spacer anywhere, the spacer places it.
    let still = false
    if ((middle.childElementCount || core.editLayout) && !barRest.classList.contains('tb-stretch')) {
      const now = parseFloat(getComputedStyle(middle)[margin]) || 0
      const start = middle.getBoundingClientRect()
      const length = len(start)
      still = middleLength !== null && Math.abs(length - middleLength) < 0.5
      middleLength = length
      const room = Math.max(0, len(growNode.getBoundingClientRect()) - RUNS_APART) + now
      // Where it lies without OLED's shift (which moves the bar after): the
      // margin taken from where it is drawn undid the shift, and the middle
      // ones stayed put across.
      const shifted = (core.oledOn && parseFloat(document.documentElement.style.getPropertyValue(isSide() ? '--oled-y' : '--oled-x'))) || 0
      want = Math.round(Math.max(0, Math.min(((isSide() ? innerHeight : innerWidth) - length) / 2 - (lo(start) - shifted - now), room)))
      // A side bar's middle run gives way when crowded (its programs then
      // scroll), so its own height and the room left are worked out from
      // what everything wants, not from what it has been squeezed to.
      if (isSide()) {
        // Its programs' column scrolls: its whole height, not what it shows.
        const natural = [...middle.children].reduce((a, n) => {
          const cs = getComputedStyle(n)
          return a + (n === windowsBox ? n.scrollHeight : n.offsetHeight) + (parseFloat(cs.marginTop) || 0) + (parseFloat(cs.marginBottom) || 0)
        }, 0)
        const others = [...barRest.children].filter(n => n !== middle && n !== growNode && getComputedStyle(n).position !== 'absolute')
        const used = others.reduce((a, n) => {
          const cs = getComputedStyle(n)
          return a + n.offsetHeight + (parseFloat(cs.marginTop) || 0) + (parseFloat(cs.marginBottom) || 0)
        }, 0)
        const free = barRest.clientHeight - (parseFloat(getComputedStyle(barRest).paddingTop) || 0) - used - natural - RUNS_APART
        want = Math.round(Math.max(0, Math.min((innerHeight - natural) / 2 - (start.top - shifted - now), free)))
      }
    }
    if (want === Math.round(target)) return
    if (!still) {
      middle.style[margin] = `${want}px`
      return
    }
    middle.style.transition = 'none'
    middle.style[margin] = `${want}px`
    void middle.offsetWidth
    middle.style.transition = ''
  }
  zones.center.addEventListener('transitionend', e => {
    if (e.target !== zones.center || (e.propertyName !== 'margin-left' && e.propertyName !== 'margin-top')) return
    if (core.popFor) core.drawPop()
    if (core.glideOn?.isConnected) core.glideTo(core.glideOn, true)
    placeSlot()
    snapStart()
    core.inkParts()
    if (core.editLayout) Picker?.relaid()
    if (core.wgFor) core.drawWidgetPop()
    requestAnimationFrame(core.sendTrayRects)
  })

  return { PART_NODES, RUNS_APART, TB_OPTIONAL, TB_PRESETS, alignApps, askPlugins, carousels, drawEntries, entryNodes, fillHer, fillTaskbar, flashSince, hasMonitorPart, herSlot, memberName, midLeft, needsSettings, ownName, placeMid, placeSideSettings, placeSlot, placedAt, popCtx, presetOf, stillMotionNow, tbLayout, tickCarousels, turnCarousel, zones }
}
