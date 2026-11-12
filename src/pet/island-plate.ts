// Behind the island and under it: Windows' blur put under the open island
// (backplate.rs), or a deeper ground of its own; and the island out of the
// way of the window under it (zhoupengjie/Wakuwaku#49), its size told.
// Made by island.ts in the section's place as the page starts
// (platePart(core), imported by island.ts). The island's core (IslandCore):
// its constants and functions as they are, what it remembers in core.st
// (island-vars.ts, read and written as st.<name>), and the parts made
// before this one in core.pieces (a later one's reached once the page is
// up). What it hands back is the island's again.
import * as Anchor from './anchor.ts'
import { pet } from './bridge.ts'
import type { IslandCore, Room } from './island.ts'

// Where the plate goes, in the page's px.
interface Box extends Room {
  x: number
  y: number
}

export function platePart(core: IslandCore) {
  const {
    st, pieces, Anchor, afterSpring, anchor, atBottom, body, home, inTopStrip, isCapsule, isEmbedded, isOn,
    isRinging, isSide, island, sizeOf, stillMotion, tbEdge,
  } = core

  // The page's own blur (--i-blur) blurs only what the page draws: open
  // above the strip, a see-through look had other programs' windows sharp
  // behind it. Under it, then, a window of Windows' acrylic (backplate.rs):
  // for a look both see-through and blurred, while the island is open (at
  // the top of the screen, while it is up at all) and someone is about;
  // faded in once its spring is over, out as it closes on the taskbar,
  // before it shrinks. While the island changes size it stays, under the
  // smaller of its two sizes, set in further, the ground as solid as
  // DEEP_OP meanwhile (movePlate, isl-firm), and is set where the island has
  // come to once it settles, the ground back as it does. Without it
  // (Windows 10, transparency effects off, the acrylic refused, a look not
  // blurred), over a maximized window the ground deepens (setDeep) so its
  // words read.
  const PLATE_IN_MS = 150
  const PLATE_OUT_MS = 100
  const DEEP_OP = 0.9
  const DEEP_MS = 150
  // The acrylic's corners are DWM's (8 px): set in from the island's edge
  // until they lie inside its rounder ones, (R - 8)(1 - 1/√2), a pixel at
  // least (4.1 under the taskbar's 22 px). The band left the page paints
  // (plated, style.css #island::before, its rim).
  const DWM_CORNER = 8
  const plateInset = (radius: number) => Math.max(1, (radius - DWM_CORNER) * (1 - Math.SQRT1_2))
  // While it springs to another size, this much further in, and as much
  // again as the spring may carry an edge past where it goes (style.css's
  // curve: some 7 % of the way it moves).
  const PLATE_SLACK = 3
  const SPRING_OVER = 0.07
  let plateWait: (() => void) | null = null
  let plateMove: (() => void) | null = null
  // A maximized window on her display, at the top of the screen (island.rs
  // look_over_max; on the taskbar, its windows' list says).
  let overMaxTop = false

  function plateWanted() {
    if (!isOn() || st.now.plate !== true || st.quiet || st.view === 'settings' || body.classList.contains('pulling') || body.classList.contains('isl-stowed')) return false
    const { op, blur } = pieces.look.groundOf(pieces.look.lookNow())
    if (!(op < 1 && blur > 0)) return false
    return home() !== 'taskbar' || st.view !== 'compact'
  }

  // After the island is laid out; resized: from its box and corners then
  // (from, fromRadius) to its new size.
  function placePlate(resized: boolean, from?: DOMRect | null, fromRadius?: number) {
    if (!plateWanted()) return hidePlate(PLATE_OUT_MS)
    if (!st.plateSent) {
      if (resized || !plateWait) {
        plateWait?.()
        // Opening from the capsule, the plate comes once the spring is
        // over: the ground as solid as DEEP_OP till then (isl-firm), as
        // while it moves, back as the plate fades in. It stood see-through
        // and clear over the windows behind for the whole spring, then went
        // white (zhoupengjie/Wakuwaku#43).
        if (resized) firmGround()
        plateWait = afterSpring(() => {
          plateWait = null
          showPlate(PLATE_IN_MS)
          softenGround(PLATE_IN_MS)
        })
      }
      return
    }
    if (resized && from) return movePlate(from, fromRadius)
    if (!plateMove) showPlate(0)
  }

  // Where the island will lie at its new size, from where it lies: at the
  // top of the screen from its top, about its middle (at a corner, from its
  // side there: anchor.ts; along the foot of the screen from its foot); on
  // the taskbar from the strip's edge it grows from, at her end's side
  // (isl-right, isl-low).
  function rectAt(r: DOMRect, width: number, height: number): Box {
    // About her capsule's middle in Windows' taskbar (isEmbedded, placeMid):
    // grown upward from a taskbar along the bottom, downward from one along the top.
    if (isEmbedded() && st.slotMiddle != null) return { x: pieces.layout.midLeft(width), y: atBottom() ? r.top + r.height - height : r.top, width, height }
    if (home() !== 'taskbar') return Anchor.grown(r, width, height, anchor(), atBottom() ? 'bottom' : 'top')
    const fromRight = tbEdge() === 'right' || (!isSide() && body.classList.contains('isl-right'))
    const fromFoot = tbEdge() === 'bottom' || (isSide() && body.classList.contains('isl-low'))
    const fromMid = !isSide() && st.slotMiddle != null && body.classList.contains('isl-mid')
    return { x: fromMid ? pieces.layout.midLeft(width) : fromRight ? r.right - width : r.left, y: fromFoot ? r.bottom - height : r.top, width, height }
  }

  // Changing size: under the smaller of its two boxes (where they meet),
  // each edge set in further by the slack and by as much as the spring may
  // carry that edge past where it goes (an edge it grows from stays put),
  // and the ground at once as solid as DEEP_OP (isl-firm): what the plate
  // no longer covers shows through no more than through the rim (at once,
  // the open island's words stood clear over the desktop as it closed;
  // the appearance session, 2026-10-14). Once it settles, the plate where
  // the island has come to and the ground back over DEEP_MS, together.
  function movePlate(from: DOMRect, fromRadius?: number) {
    const to = rectAt(from, parseFloat(island.style.width), parseFloat(island.style.height))
    const x = Math.max(from.left, to.x)
    const y = Math.max(from.top, to.y)
    const box = { x, y, width: Math.min(from.right, to.x + to.width) - x, height: Math.min(from.bottom, to.y + to.height) - y }
    const inset = plateInset(Math.max(fromRadius || 0, parseFloat(island.style.borderRadius) || 0))
    const edge = (moved: number) => inset + PLATE_SLACK + SPRING_OVER * Math.abs(moved)
    const sides = [edge(to.x - from.left), edge(to.y - from.top), edge(to.x + to.width - from.right), edge(to.y + to.height - from.bottom)]
    firmGround()
    sendPlate(box, sides, inset, 0)
    plateMove?.()
    plateMove = afterSpring(() => {
      plateMove = null
      softenGround(DEEP_MS)
      showPlate(0)
    })
  }

  // The ground as solid as DEEP_OP while the plate is not yet where the
  // island is (isl-firm), and back over ms.
  function firmGround() {
    for (const a of island.getAnimations()) if (a.id === 'firm') a.cancel()
    body.classList.add('isl-firm')
  }

  function softenGround(ms: number) {
    if (!body.classList.contains('isl-firm')) return
    const firm = getComputedStyle(island).backgroundColor
    body.classList.remove('isl-firm')
    if (!stillMotion()) island.animate([{ offset: 0, backgroundColor: firm }], { id: 'firm', duration: ms, easing: 'ease' })
  }

  // Under the island as it is now (moved: told again, no fade).
  function showPlate(fade: number) {
    if (!plateWanted()) return hidePlate(0)
    const r = island.getBoundingClientRect()
    const inset = plateInset(parseFloat(getComputedStyle(island).borderTopLeftRadius) || 0)
    sendPlate({ x: r.left, y: r.top, width: r.width, height: r.height }, [inset, inset, inset, inset], inset, fade)
  }

  // The plate in box, its edges set in by sides (left, top, right, bottom;
  // never to nothing: it stays), the rim over the corners' band (rim wide);
  // faded in when new.
  function sendPlate(box: Box, sides: number[], rim: number, fade: number) {
    const across = Math.min(1, (box.width - 2) / (sides[0] + sides[2]))
    const down = Math.min(1, (box.height - 2) / (sides[1] + sides[3]))
    const [left, top, right, bottom] = [sides[0] * across, sides[1] * down, sides[2] * across, sides[3] * down]
    const rect = { x: box.x + left, y: box.y + top, width: box.width - left - right, height: box.height - top - bottom, vw: innerWidth, vh: innerHeight, dark: pieces.look.modeNow() !== 'light' }
    island.style.setProperty('--rim', `${rim.toFixed(2)}px`)
    body.classList.add('plated')
    const sent = JSON.stringify(rect)
    if (sent === st.plateSent) return
    const first = !st.plateSent
    st.plateSent = sent
    pet.plate(rect, first && !stillMotion() ? fade : 0)
  }

  function hidePlate(fade: number) {
    plateWait?.()
    plateWait = null
    plateMove?.()
    plateMove = null
    body.classList.remove('plated', 'isl-firm')
    if (!st.plateSent) return
    st.plateSent = ''
    pet.plate(null, stillMotion() ? 0 : fade)
  }

  // Resized for its room, the window moves the island in the page.
  window.addEventListener('resize', () => st.plateSent && !plateMove && showPlate(0))

  // Over a maximized window (one on her display, taskbar/ "max"), a
  // see-through ground the plate does not blur goes to DEEP_OP while open,
  // over DEEP_MS (at once as it opens), and back as it closes or the window
  // is no longer maximized.
  const overMax = () => (home() === 'taskbar' ? st.taskWindows.some(a => (a.windows || []).some(w => w.max && !w.min)) : overMaxTop)
  pet.onOverMax(({ on }: { on?: boolean } = {}) => {
    overMaxTop = on === true
    if (isOn()) setDeep(true)
  })

  function deepWanted() {
    if (!isOn() || st.view === 'settings' || (home() === 'taskbar' && st.view === 'compact')) return false
    const { op, blur } = pieces.look.groundOf(pieces.look.lookNow())
    return op < DEEP_OP && !(st.now.plate === true && blur > 0) && overMax()
  }

  function setDeep(fades: boolean) {
    const on = deepWanted()
    if (body.classList.contains('isl-deep') === on) return
    const from = getComputedStyle(island).backgroundColor
    body.classList.toggle('isl-deep', on)
    if (fades && !stillMotion()) island.animate([{ offset: 0, backgroundColor: from }], { id: 'deep', duration: DEEP_MS, easing: 'ease' })
  }

  // --- Out of the way of the window under it (zhoupengjie/Wakuwaku#49) ----------------
  // At the top of the screen the closed island lay over the window under it
  // (a maximized browser's tab strip), and a press on a tab there was the
  // island's. While a window lies under it (island.rs look_under), unless
  // turned off (islandHideUnder), it slides up out of the way (isl-stowed,
  // style.css): a sliver of its foot left at the screen's edge, nothing of
  // it taking the pointer. Never while something wants the person: open
  // (hovered, a turn done, an error, a widget's nudge, with endCue open), a
  // ring round it (the flow light), a prompt, the settings, a session
  // waiting on you, her being taken home. Down again
  // once the window has gone, and while the pointer calls it (resting at
  // the edge over it, or on it, until it has been away a moment: island.rs
  // call_look). Never on the taskbar, nor at a Mac's screen top.
  let underTop = false
  let calledDown = false
  let stowSent = ''

  function stowWanted() {
    // Never over Windows' taskbar (#105): nothing's top lies under it there.
    if (home() !== 'island' || st.screenTop || atBottom() || inTopStrip() || st.config.islandHideUnder === false || !underTop || calledDown) return false
    if (st.view !== 'compact' || st.isHover || st.now.mood === 'waiting') return false
    // Down for a flow light's ring, as it came down when it opened by itself.
    return !st.pull && !st.isReaching && !st.isAbsorbing && !st.isComingHome && !isRinging()
  }

  function setStow() {
    if (!isOn()) return
    const on = stowWanted()
    const was = body.classList.contains('isl-stowed')
    body.classList.toggle('isl-stowed', on)
    // Main told whether it is up, and the closed island's size, where a
    // window under it is looked for (none on the taskbar).
    const { width, height } = home() === 'island' && !st.screenTop ? sizeOf('compact') : { width: 0, height: 0 }
    const sent = JSON.stringify([on, width, height])
    if (sent !== stowSent) {
      stowSent = sent
      pet.stow(on, width, height)
    }
    // The blur under it goes with it, and comes back once it is down, the
    // ground as solid as DEEP_OP till then, as when it opens.
    if (on !== was) placePlate(!on)
  }

  pet.onUnder(({ on }: { on?: boolean } = {}) => {
    underTop = on === true
    setStow()
  })
  pet.onCalled(({ on }: { on?: boolean } = {}) => {
    calledDown = on === true
    setStow()
  })

  function setSize({ width, height }: Room) {
    island.style.width = `${width}px`
    island.style.height = `${height}px`
    if ((home() === 'taskbar' && !isSide()) || isEmbedded()) pieces.layout.placeMid(width)
    // The taskbar's left end is square, and what grows up from it is rounded
    // above (on its Mica, her capsule and all it grows into round).
    // What grows from her end is flat where it meets the strip: the strip's
    // own end on the black one (its corners at the screen's edge square).
    const flat: Record<string, string> = { bottom: '22px 22px 0 0', top: '0 0 22px 22px', left: '0 22px 22px 0', right: '22px 0 0 22px' }
    island.style.borderRadius =
      home() === 'taskbar'
        ? st.view === 'compact'
          ? isCapsule()
            ? `${(isSide() ? width : height) / 2}px`
            : '0'
          : st.view === 'settings' || isCapsule()
            ? '22px'
            : flat[tbEdge()]
        : st.screenTop
          ? // Hung from a Mac's screen top: square where it meets the edge.
            `0 0 ${st.view === 'compact' ? 12 : 30}px ${st.view === 'compact' ? 12 : 30}px`
          : `${height > 60 ? 30 : height / 2}px`
  }

  return { DEEP_OP, PLATE_OUT_MS, hidePlate, placePlate, setDeep, setSize, setStow }
}
