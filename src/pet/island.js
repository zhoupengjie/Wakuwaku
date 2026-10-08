// The island: a black pill at the top of the screen, after the iPhone's
// Dynamic Island, and her home. It springs between three shapes:
//   compact   her round portrait, a few words and the clock
//   expanded  hovered, or for a few seconds when something happens (a turn
//             done, an error, she needs you): her whole self standing in it,
//             playing that mood, beside what is going on
//   ask       a prompt from Claude, answered right in the island, her beside it
// and a second, round bubble beside it when another session wants you too.
//
// She is one element throughout: the portrait grows into her whole self and
// shrinks back, so she is never in two places at once.
//
// Press her and pull down, and the island stretches like a drop of ink with
// her in it; pull far enough and the drop pinches off and she lands on the
// desktop as the free pet. Let go before that and she springs back in.
//
// The window is just big enough for all this and clicks pass through it; only
// the island takes the pointer. Shapes change in CSS, never by resizing the
// window frame by frame, so the spring stays smooth. When a shape needs more
// room, the window grows first and the island after; when it needs less, the
// island shrinks first and the window after.
;(function () {
  const { t, render: say } = window.I18n
  const { CELL_W, CELL_H, CLIPS, MOOD_CLIP } = window.Sprite

  // Brighter than the pet's colours: these sit on black.
  const COLOR = {
    idle: '#8e8e93',
    working: '#5e9bff',
    waiting: '#ffb340',
    done: '#34d27b',
    review: '#b18cff',
    error: '#ff5c6c',
  }

  // What she plays when the island opens by itself for each mood.
  const NUDGE_CLIP = { waiting: 'waiting', done: 'waving', review: 'review', error: 'failed' }

  // How long the island stays open when something happens, and how long the
  // pointer has to rest on it before it opens (so passing by does not).
  const NUDGE_MS = 3600
  const HOVER_OPEN_MS = 140
  const HOVER_CLOSE_MS = 260
  // Matches the springs in style.css: the window waits this long to shrink.
  const SPRING_MS = 560
  const MIN_W = 112
  // The window without extra room (main's ISLAND), and the island's distance
  // from the top of it.
  const BASE = { width: 460, height: 132 }
  const TOP = 8
  // Her two sizes: the round portrait in the compact island, and her whole
  // self (one sheet cell, halved) standing in the open one.
  const HEAD = 24
  const HEAD_SCALE = 0.21
  const BODY_SCALE = 0.5
  const BODY_W = Math.round(CELL_W * BODY_SCALE)
  const BODY_H = Math.round(CELL_H * BODY_SCALE)
  const OPEN_H = BODY_H + 12
  // Pulling her out: the room the window takes for it, how far the drop
  // stretches before it pinches off, and the drop's size.
  const PULL_ROOM = { width: 760, height: 440 }
  const BREAK_PX = 110
  const DROP_R = 56

  const body = document.body
  const island = document.getElementById('island')
  const compact = document.getElementById('island-compact')
  const expanded = document.getElementById('island-expanded')
  const side = document.getElementById('island-side')
  const her = document.getElementById('island-her')
  const herSheet = her.querySelector('.sheet')
  const gooIsland = document.querySelector('#goo .goo-island')
  const gooNeck = document.querySelector('#goo .goo-neck')
  const gooDrop = document.querySelector('#goo .goo-drop')
  const dropHer = document.getElementById('drop-her')
  const dropSheet = dropHer.querySelector('.sheet')
  const panel = document.getElementById('panel')
  const stage = document.getElementById('stage')

  let lang = 'en'
  let now = { mood: 'idle', detail: '', project: '', since: null, took: null, others: 0, sessions: 0 }
  let config = {}
  let spriteUrl = null
  let second = null
  let isHover = false
  let hoverTimer
  // Something just happened: { until, clip, text }. The island opens for a
  // moment, she plays clip, and text (a hello) replaces the mood's line.
  let nudge = null
  let nudgeTimer
  let view = ''
  let clockTimer
  let blinkTimer
  let herClip = null
  let herTimer
  let room = null
  let roomTimer
  // Her being pulled out: { x0, y0, started, broken, at }.
  let pull = null
  let dropTimer

  const isOn = () => config.display === 'island'
  const isAsking = () => isOn() && !panel.hidden
  const isNudging = () => !!nudge && Date.now() < nudge.until

  function clock(ms) {
    const s = Math.max(0, Math.floor(ms / 1000))
    const h = Math.floor(s / 3600)
    const m = Math.floor((s % 3600) / 60)
    const sec = String(s % 60).padStart(2, '0')
    return h ? `${h}:${String(m).padStart(2, '0')}:${sec}` : `${m}:${sec}`
  }

  // The clock: how long this turn has run, or how long the finished one took.
  function time() {
    if ((now.mood === 'working' || now.mood === 'waiting') && now.since) return clock(Date.now() - now.since)
    if ((now.mood === 'done' || now.mood === 'review' || now.mood === 'error') && now.took) return clock(now.took)
    return ''
  }

  function el(tag, cls, text) {
    const node = document.createElement(tag)
    if (cls) node.className = cls
    if (text !== undefined) node.textContent = text
    return node
  }

  // A round face with two capsule eyes, in a mood's colour: for a second
  // session, and in her place when there is no pet yet.
  function face(mood, px) {
    const node = el('span', 'face')
    node.style.cssText = `width:${px}px;height:${px}px;background:${COLOR[mood] || COLOR.idle}`
    node.append(el('i'), el('i'))
    return node
  }

  function showFrame(sheet, row, frame) {
    sheet.style.backgroundPosition = `${-frame * CELL_W}px ${-row * CELL_H}px`
  }

  // --- The shapes ---------------------------------------------------------------

  function fillCompact() {
    const label = now.mood === 'idle' ? '' : [now.project, t(lang, `island.${now.mood}`)].filter(Boolean).join(' · ')
    const clockText = el('span', 'clock', time())
    clockText.style.color = COLOR[now.mood]
    compact.replaceChildren(...(spriteUrl ? [] : [face(now.mood, 24)]), el('span', 'label', label), clockText)
    compact.classList.toggle('bare', !label && !clockText.textContent)
  }

  function fillExpanded() {
    const title = (isNudging() && nudge.text) || t(lang, `mood.${now.mood}`)
    const sub = [now.project, say(lang, now.detail)].filter(Boolean).join(' · ')
    const more = now.others > 0 ? t(lang, 'detail.moreSessions', { n: now.others }) : ''
    const lines = el('span', 'lines')
    lines.append(el('span', 'title', title))
    if (sub || more) lines.append(el('span', 'sub', [sub, more].filter(Boolean).join(' · ')))
    const clockText = el('span', 'clock', time())
    clockText.style.color = COLOR[now.mood]
    expanded.replaceChildren(...(spriteUrl ? [] : [face(now.mood, 48)]), lines, clockText)
  }

  function fillSide() {
    side.replaceChildren(second ? face(second, 20) : '')
    side.classList.toggle('shown', !!second && view !== 'ask')
  }

  // --- Her, in the island -----------------------------------------------------------

  // Her shape for the island's: a round portrait ringed in the mood's colour,
  // or her whole self standing at the left. Both are the same sheet, cropped
  // and scaled, so one springs into the other.
  function placeHer({ height }) {
    if (!spriteUrl) return
    const s = her.style
    if (view === 'compact') {
      Object.assign(s, { left: '6px', top: '6px', width: `${HEAD}px`, height: `${HEAD}px`, borderRadius: `${HEAD / 2}px` })
      s.boxShadow = `0 0 0 1.5px ${COLOR[now.mood]}`
      herSheet.style.transform = `translate(${-(CELL_W * HEAD_SCALE - HEAD) / 2}px, -2px) scale(${HEAD_SCALE})`
    } else {
      const top = height - BODY_H - (view === 'ask' ? 10 : 6)
      Object.assign(s, { left: '12px', top: `${top}px`, width: `${BODY_W}px`, height: `${BODY_H}px`, borderRadius: '0px' })
      s.boxShadow = '0 0 0 0 transparent'
      herSheet.style.transform = `translate(0px, 0px) scale(${BODY_SCALE})`
    }
  }

  // Still in the portrait (her mood's first frame); playing once she stands.
  function animateHer() {
    if (!spriteUrl) return
    const name = view === 'compact' ? null : view === 'ask' ? 'waiting' : isNudging() ? nudge.clip : MOOD_CLIP[now.mood]
    if (name && name === herClip) return
    clearTimeout(herTimer)
    herClip = name
    if (!name) return showFrame(herSheet, CLIPS[MOOD_CLIP[now.mood]].row, 0)
    const { row, frames, ms } = CLIPS[name]
    let frame = 0
    const step = () => {
      showFrame(herSheet, row, frame)
      frame = (frame + 1) % frames
      herTimer = setTimeout(step, ms)
    }
    step()
  }

  // --- Room: the window grows before the island does, and shrinks after ----------------

  function roomFor(want) {
    const sideW = second && view !== 'ask' ? 42 : 0
    const width = want.width + sideW + 48
    const height = TOP + want.height + 24
    return width <= BASE.width && height <= BASE.height ? null : { width, height }
  }

  // Returns true when the window has to grow first.
  function setRoom(next) {
    const size = r => r || BASE
    clearTimeout(roomTimer)
    if (JSON.stringify(next) === JSON.stringify(room)) return false
    const grows = size(next).width > size(room).width || size(next).height > size(room).height
    const apply = () => {
      room = next
      window.pet.panel(next)
    }
    if (grows) apply()
    else roomTimer = setTimeout(apply, SPRING_MS)
    return grows
  }

  function setSize({ width, height }) {
    island.style.width = `${width}px`
    island.style.height = `${height}px`
    island.style.borderRadius = `${height > 60 ? 30 : height / 2}px`
  }

  // --- Deciding ---------------------------------------------------------------

  function sizeOf(name) {
    const withHer = !!spriteUrl
    if (name === 'ask') {
      return {
        width: panel.offsetWidth + 28 + (withHer ? 108 : 0),
        height: Math.max(panel.offsetHeight + 26, withHer ? BODY_H + 20 : 0),
      }
    }
    if (name === 'expanded') {
      return withHer
        ? { width: Math.min(440, Math.max(320, expanded.offsetWidth)), height: OPEN_H }
        : { width: Math.min(400, Math.max(300, expanded.offsetWidth)), height: 84 }
    }
    return { width: Math.max(MIN_W, compact.offsetWidth), height: 36 }
  }

  function update() {
    clearInterval(clockTimer)
    // While she is being pulled out, the pull has the island.
    if (!isOn() || pull?.started) return
    // A prompt answered: the island closes rather than lingering open.
    if (view === 'ask' && !isAsking()) nudge = null
    view = isAsking() ? 'ask' : isHover || isNudging() ? 'expanded' : 'compact'
    island.dataset.view = view
    body.classList.toggle('has-her', !!spriteUrl)
    if (spriteUrl) herSheet.style.backgroundImage = `url("${spriteUrl}")`
    fillCompact()
    fillExpanded()
    fillSide()

    const want = sizeOf(view)
    const grows = setRoom(roomFor(want))
    // The window has its room before the island (and she) grow into it.
    const grow = () => {
      setSize(want)
      placeHer(want)
      animateHer()
    }
    if (grows) setTimeout(grow, 40)
    else grow()

    if (time()) {
      clockTimer = setInterval(() => {
        for (const c of island.querySelectorAll('.clock')) c.textContent = time()
      }, 1000)
    }
  }

  function nudgeFor(clip, text) {
    nudge = { until: Date.now() + NUDGE_MS, clip, text }
    clearTimeout(nudgeTimer)
    nudgeTimer = setTimeout(() => {
      nudge = null
      update()
    }, NUDGE_MS)
    update()
  }

  // Now and then a blink, as a class for a moment (no running animation).
  function blink() {
    clearTimeout(blinkTimer)
    blinkTimer = setTimeout(() => {
      if (isOn() && !document.hidden) {
        island.classList.add('blink')
        side.classList.add('blink')
        setTimeout(() => {
          island.classList.remove('blink')
          side.classList.remove('blink')
        }, 140)
      }
      blink()
    }, 3500 + Math.random() * 4000)
  }

  // --- Pulling her out: the drop --------------------------------------------------

  function pullDown(e) {
    if (e.button !== 0 || !spriteUrl || view === 'ask') return
    // No mouse events follow, so the island does not take this for a click.
    e.preventDefault()
    pull = { x0: e.clientX, y0: e.clientY, started: false, broken: false, at: null }
    window.addEventListener('pointermove', pullMove)
    window.addEventListener('pointerup', pullUp)
    try {
      her.setPointerCapture(e.pointerId)
    } catch {}
  }

  function pullMove(e) {
    if (!pull) return
    if (!pull.started) {
      if (Math.hypot(e.clientX - pull.x0, e.clientY - pull.y0) < 5) return
      startPull()
    }
    stretch(e.clientX, e.clientY)
  }

  function startPull() {
    pull.started = true
    nudge = null
    clearTimeout(nudgeTimer)
    clearInterval(clockTimer)
    // Room to stretch in, then the drop: she hangs in it kicking her legs.
    setRoom(PULL_ROOM)
    dropSheet.style.backgroundImage = `url("${spriteUrl}")`
    body.classList.add('pulling')
    const { row, frames, ms } = CLIPS.jumping
    let frame = 0
    const step = () => {
      showFrame(dropSheet, row, frame)
      frame = (frame + 1) % frames
      dropTimer = setTimeout(step, ms)
    }
    step()
  }

  // The island's double, a neck that thins as it stretches, and the drop with
  // her in it; the filter melts them into one shape until the neck goes.
  function stretch(x, y) {
    const r = island.getBoundingClientRect()
    // You hold her by the head: her middle hangs below the cursor.
    const cx = x
    const cy = Math.max(y + 30, r.bottom - 10)
    const ax = Math.min(Math.max(cx, r.left + 24), r.right - 24)
    const ay = r.bottom - 14
    const dist = Math.hypot(cx - ax, cy - ay)
    const wasBroken = pull.broken
    pull.broken = dist > BREAK_PX || (wasBroken && dist > BREAK_PX * 0.7)
    if (pull.broken && !wasBroken) wobble()

    Object.assign(gooIsland.style, {
      left: `${r.left}px`,
      top: `${r.top}px`,
      width: `${r.width}px`,
      height: `${r.height}px`,
      borderRadius: getComputedStyle(island).borderRadius,
    })
    Object.assign(gooDrop.style, { left: `${cx - DROP_R}px`, top: `${cy - DROP_R}px`, width: `${DROP_R * 2}px`, height: `${DROP_R * 2}px` })
    const neck = pull.broken ? 0 : Math.max(8, 34 - dist * 0.22)
    const angle = (Math.atan2(cy - ay, cx - ax) * 180) / Math.PI - 90
    Object.assign(gooNeck.style, {
      left: `${ax - neck / 2}px`,
      top: `${ay}px`,
      width: `${neck}px`,
      height: `${dist}px`,
      transform: `rotate(${angle}deg)`,
    })
    Object.assign(dropHer.style, { left: `${cx - BODY_W / 2}px`, top: `${cy - BODY_H / 2}px` })
    pull.at = { cx, cy }
  }

  function wobble() {
    body.classList.remove('wobble')
    void island.offsetWidth
    body.classList.add('wobble')
    setTimeout(() => body.classList.remove('wobble'), 520)
  }

  function pullUp(e) {
    window.removeEventListener('pointermove', pullMove)
    window.removeEventListener('pointerup', pullUp)
    if (!pull) return
    const done = pull
    if (!done.started) {
      // A click on her: the main window, as a click on the island.
      pull = null
      return window.pet.openSettings()
    }
    clearTimeout(dropTimer)
    if (done.broken && done.at) {
      // Free: she lands where the drop let go of her, feet first.
      window.pet.dropOut({ dx: done.at.cx - e.clientX, dy: done.at.cy + BODY_H / 2 - e.clientY })
      return
    }
    settleBack()
  }

  // Let go too soon: the drop and she spring back into her seat.
  function settleBack() {
    const r = island.getBoundingClientRect()
    const tx = r.left + 18
    const ty = r.top + 18
    body.classList.add('settling')
    Object.assign(gooDrop.style, { left: `${tx - 10}px`, top: `${ty - 10}px`, width: '20px', height: '20px' })
    Object.assign(gooNeck.style, { width: '0px', height: '0px' })
    Object.assign(dropHer.style, { left: `${tx - BODY_W / 2}px`, top: `${ty - BODY_H / 2}px`, transform: 'scale(0.25)' })
    setTimeout(() => {
      body.classList.remove('pulling', 'settling')
      dropHer.style.transform = ''
      pull = null
      herClip = null
      window.pet.hover(island.matches(':hover') || side.matches(':hover'))
      update()
    }, 320)
  }

  her.addEventListener('pointerdown', pullDown)

  // --- On and off ---------------------------------------------------------------

  // The prompt panel lives in the island while it is on, above the pet otherwise.
  function place() {
    body.classList.toggle('island', isOn())
    if (isOn() && panel.parentElement !== island) island.append(panel)
    if (!isOn() && panel.parentElement !== stage) stage.prepend(panel)
    if (!isOn()) {
      clearInterval(clockTimer)
      clearTimeout(herTimer)
      clearTimeout(dropTimer)
      clearTimeout(roomTimer)
      body.classList.remove('pulling', 'settling', 'has-her')
      pull = null
      herClip = null
      room = null
      view = ''
    }
  }

  window.pet.onUpdate(data => {
    const before = now.mood
    now = data
    lang = data.lang || lang
    config = data.config || {}
    spriteUrl = data.sprite
    second = data.second || null
    place()
    // Something new that wants you, or is finished: the island opens for a moment.
    if (isOn() && before !== now.mood && NUDGE_CLIP[now.mood]) nudgeFor(NUDGE_CLIP[now.mood], null)
    else update()
  })

  // A hello, or something to fix: the island opens to say it, and she waves.
  window.pet.onReact(({ say: text }) => {
    if (isOn() && text) nudgeFor('waving', say(lang, text))
  })

  // --- The pointer ----------------------------------------------------------------

  for (const target of [island, side]) {
    target.addEventListener('mouseenter', () => {
      window.pet.hover(true)
      clearTimeout(hoverTimer)
      hoverTimer = setTimeout(() => {
        isHover = true
        update()
      }, HOVER_OPEN_MS)
    })
    target.addEventListener('mouseleave', () => {
      // Pulling her out keeps the window's pointer, wherever it goes.
      if (!pull?.started) window.pet.hover(false)
      clearTimeout(hoverTimer)
      hoverTimer = setTimeout(() => {
        isHover = false
        update()
      }, HOVER_CLOSE_MS)
    })
    // A click opens the main window; the prompt's own buttons and she are not clicks on the island.
    target.addEventListener('mousedown', e => {
      if (e.button === 0 && !panel.contains(e.target) && !her.contains(e.target)) window.pet.dragStart()
    })
    target.addEventListener('contextmenu', e => {
      if (panel.contains(e.target) && e.target.matches('input')) return
      e.preventDefault()
      window.pet.menu()
    })
  }

  blink()

  // panel.js tells the island when its prompt changes.
  window.Island = { isOn, changed: update }
})()
