// Her pulled out of the island and taken back: pressed and pulled down, the
// island stretches like a drop of ink with her in it and pinches off, her
// own window taking her under the cursor; brought close again, it reaches
// out a drop for her and takes her back in (reach, absorb).
// Made by island.ts in the section's place as the page starts
// (dropPart(core), imported by island.ts). The island's core (IslandCore):
// its constants and functions as they are, what it remembers in core.st
// (island-vars.ts, read and written as st.<name>), and the parts made
// before this one in core.pieces (a later one's reached once the page is
// up). What it hands back is the island's again.
import * as State from './island-state.ts'
import { pet } from './bridge.ts'
import type { IslandCore } from './island.ts'
import type { Pull } from './island-state.ts'

// Her middle on the screen, from her own window (main: island:reach,
// island:absorb); snap: let go now and she is back.
interface At {
  x: number
  y: number
  snap?: boolean
}

export function dropPart(core: IslandCore) {
  const {
    st, pieces, go, BODY_H, BODY_W, BREAK_PX, CLIPS, DROP_R, PULL_ROOM, REACH_R, REACH_SHARE, atBottom, body,
    dropHer, dropSheet, endNudge, gooDrop, gooIsland, gooNeck, her, home, isHome, isNudging, isOn, isSetting,
    isSide, island, pressNudge, setRoom, sheetImage, showFrame, tbEdge, update,
  } = core
  const { hidePlate, setStow } = pieces.plate

  function pullDown(e: PointerEvent) {
    console.log('pointerdown on her in the island, button', e.button)
    if (e.button !== 0 || !isHome() || st.view === 'ask' || isSetting() || st.isAbsorbing) return
    // No mouse events follow, so the island does not take this for a click.
    e.preventDefault()
    go({ type: 'press', x: e.clientX, y: e.clientY })
    pet.holding(true)
    window.addEventListener('pointermove', pullMove)
    window.addEventListener('pointerup', pullUp)
    window.addEventListener('pointercancel', pullUp)
    // The button is held on this window wherever the cursor goes, so the
    // pointer events keep coming here even once her own window has her. On the
    // island, not on her: her element is hidden once she is out, and a hidden
    // element loses the pointer.
    try {
      island.setPointerCapture(e.pointerId)
    } catch {}
  }

  function pullMove(e: PointerEvent) {
    if (st.pull?.started) st.pulledAt = Date.now()
    if (!st.pull || st.pull.carrying) return
    if (!st.pull.started) {
      if (Math.hypot(e.clientX - st.pull.x0, e.clientY - st.pull.y0) < 5) return
      startPull()
    }
    stretch(e.clientX, e.clientY)
    // Pinched off: from here her own window carries her, anywhere on the screen.
    if (st.pull.broken) handOff(e)
  }

  function startPull() {
    go({ type: 'pull' })
    clearTimeout(st.nudgeTimer)
    clearInterval(st.clockTimer)
    // Room to stretch in, then the drop: she hangs in it kicking her legs.
    setRoom(PULL_ROOM)
    startDropHer()
    body.classList.add('pulling')
    hidePlate(0)
  }

  function startDropHer() {
    clearTimeout(st.dropTimer)
    sheetImage(dropSheet, st.spriteUrl)
    const { row, frames, ms } = CLIPS.jumping
    let frame = 0
    const step = () => {
      showFrame(dropSheet, row, frame)
      frame = (frame + 1) % frames
      st.dropTimer = setTimeout(step, ms)
    }
    step()
  }

  // Where a neck leaves her home: the point of its edge nearest (x, y), so a
  // drop below the island hangs from its lower edge, and one above the
  // taskbar stands on its upper edge.
  function edgeNear(r: DOMRect, x: number, y: number) {
    const [ix, iy] = isSide() ? [14, 24] : [24, 14]
    return [Math.min(Math.max(x, r.left + ix), r.right - ix), Math.min(Math.max(y, r.top + iy), r.bottom - iy)]
  }

  // Her seat in her home, in the page: where she springs back to.
  function seatIn(r: DOMRect) {
    if (home() === 'taskbar' && isSide()) return [r.left + r.width / 2, r.top + r.width / 2]
    if (home() === 'taskbar') return [r.left + r.height / 2, r.top + r.height / 2]
    return [r.left + 18, r.top + 18]
  }

  // The home's double, a neck from it to (cx, cy), and a drop there; the
  // filter melts them into one shape. A neck of 0 leaves the drop on its own.
  function drawGoo(cx: number, cy: number, { neck, radius = DROP_R }: { neck: number; radius?: number }) {
    const r = island.getBoundingClientRect()
    const [ax, ay] = edgeNear(r, cx, cy)
    Object.assign(gooIsland.style, {
      left: `${r.left}px`,
      top: `${r.top}px`,
      width: `${r.width}px`,
      height: `${r.height}px`,
      borderRadius: getComputedStyle(island).borderRadius,
    })
    Object.assign(gooDrop.style, { left: `${cx - radius}px`, top: `${cy - radius}px`, width: `${radius * 2}px`, height: `${radius * 2}px` })
    const length = Math.hypot(cx - ax, cy - ay)
    const angle = (Math.atan2(cy - ay, cx - ax) * 180) / Math.PI - 90
    Object.assign(gooNeck.style, {
      left: `${ax - neck / 2}px`,
      top: `${ay}px`,
      width: `${neck}px`,
      height: `${length}px`,
      transform: `rotate(${angle}deg)`,
    })
    return length
  }

  // You hold her by the head: her middle hangs below the cursor. The neck
  // thins as it stretches, and pinches off past BREAK_PX.
  function stretch(x: number, y: number) {
    const r = island.getBoundingClientRect()
    // From the top of the screen (her island, a top bar) she can only come
    // down, from a bottom bar (or her island along the foot, #105) only up,
    // from a side bar only into the screen.
    const edge = home() === 'taskbar' ? tbEdge() : atBottom() ? 'bottom' : 'top'
    const cx = edge === 'left' ? Math.max(x, r.right - 10) : edge === 'right' ? Math.min(x, r.left + 10) : x
    const cy = edge === 'bottom' ? Math.min(y + 30, r.top + 10) : edge === 'top' ? Math.max(y + 30, r.bottom - 10) : y
    const [ax, ay] = edgeNear(r, cx, cy)
    const dist = Math.hypot(cx - ax, cy - ay)
    go({ type: 'stretch', broken: dist > BREAK_PX, at: { cx, cy } })
    // Stretched, so pulled (pullMove).
    drawGoo(cx, cy, { neck: st.pull!.broken ? 0 : Math.max(8, 34 - dist * 0.22) })
    Object.assign(dropHer.style, { left: `${cx - BODY_W / 2}px`, top: `${cy - BODY_H / 2}px` })
  }

  // The drop let go of her: her window takes her where she is, and the island
  // wobbles back with her seat empty.
  // Pinched off: stretched to where the drop has her (st.pull.at).
  function handOff(e: PointerEvent) {
    console.log('hand-off')
    go({ type: 'carry' })
    clearTimeout(st.dropTimer)
    pet.releaseHer({ dx: st.pull!.at!.cx - e.clientX, dy: st.pull!.at!.cy - e.clientY })
    body.classList.remove('pulling')
    wobble()
    st.herClip = null
    update()
  }

  function wobble() {
    body.classList.remove('wobble')
    void island.offsetWidth
    body.classList.add('wobble')
    setTimeout(() => body.classList.remove('wobble'), 520)
  }

  function pullUp(e?: PointerEvent) {
    console.log('pull up:', e?.type, st.pull ? (st.pull.carrying ? 'carrying' : st.pull.started ? 'pulling' : 'pressed') : 'no pull')
    window.removeEventListener('pointermove', pullMove)
    window.removeEventListener('pointerup', pullUp)
    window.removeEventListener('pointercancel', pullUp)
    pet.holding(false)
    if (!st.pull) return
    const done = st.pull
    // A press that never moved, or her carried off, is over here; one let
    // go in the drop stays until she is back in her seat (settleBack).
    go({ type: 'release' })
    if (!done.started) {
      // A click on her, not a pull: the settings, as anywhere on the island;
      // open by itself, it closes instead, as anywhere on it (pressNudge).
      // The click the browser sends after, on the island that held the
      // pointer, is this press too: not one to open the settings.
      if (st.view === 'ask') return
      if (!isNudging()) return pet.openSettings()
      // What was kept and shown as she was looked at: not open by itself
      // (island-state.ts pressOn), so the settings, as for any other press.
      if (State.pressOn(st.nudge) === 'press') {
        endNudge()
        return pet.openSettings()
      }
      st.pulledAt = Date.now()
      return pressNudge()
    }
    st.pulledAt = Date.now()
    if (done.carrying) {
      // Let go: she lands there, or comes home if she was let go by the island.
      pet.dropHer()
      pet.hover(false)
      return
    }
    settleBack(done.at)
  }

  // Into her seat from where the drop has her: the drop and she spring back.
  function settleBack(at: Pull['at']) {
    const r = island.getBoundingClientRect()
    const [tx, ty] = seatIn(r)
    if (at) drawGoo(at.cx, at.cy, { neck: 26 })
    body.classList.add('pulling')
    requestAnimationFrame(() => {
      body.classList.add('settling')
      drawGoo(tx, ty, { neck: 0, radius: 10 })
      Object.assign(dropHer.style, { left: `${tx - BODY_W / 2}px`, top: `${ty - BODY_H / 2}px`, transform: 'scale(0.25)' })
    })
    setTimeout(() => {
      body.classList.remove('pulling', 'settling', 'reaching')
      clearTimeout(st.dropTimer)
      dropHer.style.transform = ''
      go({ type: 'settled' })
      st.herClip = null
      pet.hover(island.matches(':hover'))
      update()
    }, 340)
  }

  // --- Reaching for her, and taking her back ------------------------------------------

  // Run fn once the window has its room to reach in: drawn before, the
  // island would move under the drawing as the window grows around it.
  function whenRoomy(fn: () => void) {
    setRoom(PULL_ROOM)
    if (innerWidth >= PULL_ROOM.width - 2 && innerHeight >= PULL_ROOM.height - 2) return fn()
    const go = () => {
      window.removeEventListener('resize', go)
      clearTimeout(timer)
      fn()
    }
    const timer = setTimeout(go, 200)
    window.addEventListener('resize', go)
  }

  // Her middle on the screen as she is moved near (snap: let go now and she
  // is back), or null when she has gone off again.
  function reach(at: At | null) {
    if (!isOn() || st.isAbsorbing) return
    clearTimeout(st.retractTimer)
    if (!at) {
      if (!st.isReaching) return
      go({ type: 'reach', on: false })
      // The drop draws back into the island.
      body.classList.add('settling')
      const r = island.getBoundingClientRect()
      const [sx, sy] = seatIn(r)
      drawGoo(sx, sy, { neck: 0, radius: 8 })
      st.retractTimer = setTimeout(() => {
        body.classList.remove('reaching', 'settling')
        update()
      }, 320)
      return
    }
    if (!st.isReaching) {
      go({ type: 'reach', on: true })
      // Down from out of the way, to reach for her.
      setStow()
      // The next call draws, once the window has grown.
      return whenRoomy(() => st.isReaching && reach(at))
    }
    body.classList.remove('settling')
    body.classList.add('reaching')
    const x = at.x - window.screenX
    const y = at.y - window.screenY
    const r = island.getBoundingClientRect()
    const [ax, ay] = edgeNear(r, x, y)
    // Close: the drop reaches all the way and holds her; nearer: part way.
    const k = at.snap ? 1 : REACH_SHARE * Math.min(1, Math.max(0, 1 - (Math.hypot(x - ax, y - ay) - 140) / 140))
    const tx = ax + (x - ax) * k
    const ty = ay + (y - ay) * k
    drawGoo(tx, ty, { neck: at.snap ? 26 : 20, radius: at.snap ? DROP_R : REACH_R + 14 * k })
  }

  // She was let go close enough: from her window's spot into the drop, and
  // the drop back into the island.
  function absorb(at: At | null) {
    if (!isOn() || !at) return
    clearTimeout(st.retractTimer)
    go({ type: 'absorb' })
    setStow()
    clearTimeout(st.comingHomeTimer)
    // Main says she is in a moment after; should it not, she is not.
    st.comingHomeTimer = setTimeout(() => {
      go({ type: 'homecoming-over' })
      update()
    }, 3000)
    body.classList.remove('reaching', 'settling')
    startDropHer()
    whenRoomy(() => {
      const cx = at.x - window.screenX
      const cy = at.y - window.screenY
      Object.assign(dropHer.style, { left: `${cx - BODY_W / 2}px`, top: `${cy - BODY_H / 2}px`, transform: '' })
      settleBack({ cx, cy })
    })
  }

  pet.onReach(reach)
  pet.onAbsorb(absorb)
  // She landed but this page never heard the button go up: forget the pull.
  pet.onLanded(() => {
    if (!st.pull?.carrying) return
    console.log('landed without a pointerup here')
    window.removeEventListener('pointermove', pullMove)
    window.removeEventListener('pointerup', pullUp)
    window.removeEventListener('pointercancel', pullUp)
    go({ type: 'landed' })
    pet.hover(false)
    update()
  })

  her.addEventListener('pointerdown', pullDown)
  island.addEventListener('lostpointercapture', () => console.log('lost pointer capture', st.pull ? (st.pull.carrying ? 'while carrying' : 'while pulling') : ''))

  return { absorb, reach }
}
