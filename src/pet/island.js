// The island: a black pill at the top of the screen, after the iPhone's
// Dynamic Island, and her home. It springs between three shapes:
//   compact   her portrait, a few words and the clock (just her when idle)
//   expanded  hovered, or when she has something to say: her animated head,
//             what is going on, the clock
//   ask       a prompt from Claude, answered right in the island
// and a second, round bubble beside it when another session wants you too.
//
// When something wants you (a prompt, a turn done, an error), she peeks out
// from under the island, playing that mood, then slips back in. She never
// takes a click: whatever is under her still gets it.
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

  // What she plays when she peeks out for each mood.
  const PEEK_CLIP = { waiting: 'waiting', done: 'waving', review: 'review', error: 'failed' }

  // How long she stays out (or the island stays open) when something
  // happens, and how long the pointer has to rest on the island before it
  // opens (so passing by does not).
  const NUDGE_MS = 3600
  const HOVER_OPEN_MS = 140
  const HOVER_CLOSE_MS = 260
  // Matches the springs in style.css: the window waits this long to shrink.
  const SPRING_MS = 560
  const MIN_W = 112
  const HEAD = 48
  const HEAD_SCALE = 0.42
  // The window without extra room (main's ISLAND), the island's distance from
  // the top, and her size peeking out: how far she tucks under the island.
  const BASE = { width: 460, height: 132 }
  const TOP = 8
  const PEEK_SCALE = 0.5
  const PEEK_W = Math.round(CELL_W * PEEK_SCALE)
  const PEEK_H = Math.round(CELL_H * PEEK_SCALE)
  const TUCK = 24

  const bar = document.getElementById('island-bar')
  const island = document.getElementById('island')
  const compact = document.getElementById('island-compact')
  const expanded = document.getElementById('island-expanded')
  const side = document.getElementById('island-side')
  const peekSprite = document.getElementById('peek-sprite')
  const panel = document.getElementById('panel')
  const stage = document.getElementById('stage')

  let lang = 'en'
  let now = { mood: 'idle', detail: '', project: '', since: null, took: null, others: 0, sessions: 0 }
  let config = {}
  let spriteUrl = null
  let second = null
  let isHover = false
  let hoverTimer
  // Something just happened: { until, clip, text }. With text, the island
  // opens to say it; either way she peeks out playing clip.
  let nudge = null
  let nudgeTimer
  let view = ''
  let clockTimer
  let headTimer
  let blinkTimer
  let peekClip = null
  let peekTimer
  let room = null
  let roomTimer

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
  // session, and for her when there is no pet yet.
  function face(mood, px) {
    const node = el('span', 'face')
    node.style.cssText = `width:${px}px;height:${px}px;background:${COLOR[mood] || COLOR.idle}`
    node.append(el('i'), el('i'))
    return node
  }

  // --- Her, from the sheet ----------------------------------------------------------

  function sheet(scale) {
    return `background-image:url("${spriteUrl}");background-size:${1536 * scale}px ${(config.sheetH || 2288) * scale}px`
  }

  // Her head, px across: a frame of a row, cropped round.
  function showHead(node, px, row, frame) {
    const k = (HEAD_SCALE * px) / HEAD
    const x = -(frame * CELL_W * k) - (CELL_W * k - px) / 2
    const y = -(row * CELL_H * k) - (4 * px) / HEAD
    node.style.backgroundPosition = `${x}px ${y}px`
  }

  function portrait(px, mood) {
    if (!spriteUrl) return face(mood, px)
    const node = el('span', 'portrait')
    node.style.cssText = `width:${px}px;height:${px}px;${sheet((HEAD_SCALE * px) / HEAD)};box-shadow:0 0 0 1.5px ${COLOR[mood]}`
    showHead(node, px, CLIPS[MOOD_CLIP[mood]].row, 0)
    return node
  }

  // --- The shapes ---------------------------------------------------------------

  function fillCompact() {
    const label = now.mood === 'idle' ? '' : [now.project, t(lang, `island.${now.mood}`)].filter(Boolean).join(' · ')
    const clockText = el('span', 'clock', time())
    clockText.style.color = COLOR[now.mood]
    compact.replaceChildren(portrait(24, now.mood), el('span', 'label', label), clockText)
    compact.classList.toggle('bare', !label && !clockText.textContent)
  }

  function fillExpanded() {
    const head = portrait(HEAD, now.mood)
    const title = (isNudging() && nudge.text) || t(lang, `mood.${now.mood}`)
    const sub = [now.project, say(lang, now.detail)].filter(Boolean).join(' · ')
    const more = now.others > 0 ? t(lang, 'detail.moreSessions', { n: now.others }) : ''
    const lines = el('span', 'lines')
    lines.append(el('span', 'title', title))
    if (sub || more) lines.append(el('span', 'sub', [sub, more].filter(Boolean).join(' · ')))
    const clockText = el('span', 'clock', time())
    clockText.style.color = COLOR[now.mood]
    expanded.replaceChildren(head, lines, clockText)
    animateHead(head)
  }

  // Her head plays her mood's row while the island is open (and only then:
  // every frame repaints the window).
  function animateHead(head) {
    clearTimeout(headTimer)
    if (!spriteUrl || view !== 'expanded') return
    const clip = CLIPS[MOOD_CLIP[now.mood]]
    let frame = 0
    const step = () => {
      showHead(head, HEAD, clip.row, frame)
      frame = (frame + 1) % clip.frames
      if (view === 'expanded') headTimer = setTimeout(step, clip.ms)
    }
    step()
  }

  function fillSide() {
    side.replaceChildren(second ? face(second, 20) : '')
    side.classList.toggle('shown', !!second && view !== 'ask')
  }

  // --- Peeking out ------------------------------------------------------------------

  // Out with a clip, or back in (null). She slides from behind the island.
  function setPeek(clip) {
    if (!spriteUrl) clip = null
    if (clip === peekClip) return
    peekClip = clip
    clearTimeout(peekTimer)
    if (!clip) {
      bar.classList.remove('peeking')
      return
    }
    peekSprite.style.cssText = `width:${PEEK_W}px;height:${PEEK_H}px;${sheet(PEEK_SCALE)}`
    const { row, frames, ms } = CLIPS[clip]
    let frame = 0
    const step = () => {
      peekSprite.style.backgroundPosition = `${-frame * CELL_W * PEEK_SCALE}px ${-row * CELL_H * PEEK_SCALE}px`
      frame = (frame + 1) % frames
      if (peekClip === clip) peekTimer = setTimeout(step, ms)
    }
    step()
    // A frame to lay her out hidden, then out she comes.
    requestAnimationFrame(() => peekClip === clip && bar.classList.add('peeking'))
  }

  // --- Room: the window grows before the island does, and shrinks after ----------------

  function roomFor(want, isPeeking) {
    const sideW = second && view !== 'ask' ? 42 : 0
    const width = Math.max(want.width + sideW + 48, isPeeking ? PEEK_W + 48 : 0)
    const height = TOP + want.height + (isPeeking ? PEEK_H - TUCK + 6 : 0) + 24
    return width <= BASE.width && height <= BASE.height ? null : { width, height }
  }

  // Returns true when the window has to grow first.
  function setRoom(next) {
    const size = r => r || BASE
    if (JSON.stringify(next) === JSON.stringify(room)) {
      clearTimeout(roomTimer)
      return false
    }
    clearTimeout(roomTimer)
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
    if (name === 'ask') return { width: panel.offsetWidth + 28, height: panel.offsetHeight + 26 }
    if (name === 'expanded') return { width: Math.min(400, Math.max(300, expanded.offsetWidth)), height: 84 }
    return { width: Math.max(MIN_W, compact.offsetWidth), height: 36 }
  }

  function update() {
    clearInterval(clockTimer)
    if (!isOn()) return
    // A prompt answered: she has done her job, back in she goes.
    if (view === 'ask' && !isAsking()) nudge = null
    view = isAsking() ? 'ask' : isHover || (isNudging() && nudge.text) ? 'expanded' : 'compact'
    island.dataset.view = view
    fillCompact()
    fillExpanded()
    fillSide()

    // Out from under the island while a prompt waits, or for a moment when
    // something happens.
    const clip = view === 'ask' ? 'waiting' : isNudging() ? nudge.clip : null
    const want = sizeOf(view)
    const grows = setRoom(roomFor(want, !!clip && !!spriteUrl))
    // The window has its room before the island (and she) grow into it.
    const grow = () => {
      setSize(view === 'ask' ? sizeOf('ask') : want)
      setPeek(clip)
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

  // --- On and off ---------------------------------------------------------------

  // The prompt panel lives in the island while it is on, above the pet otherwise.
  function place() {
    document.body.classList.toggle('island', isOn())
    if (isOn() && panel.parentElement !== island) island.append(panel)
    if (!isOn() && panel.parentElement !== stage) stage.prepend(panel)
    if (!isOn()) {
      clearInterval(clockTimer)
      clearTimeout(headTimer)
      setPeek(null)
      room = null
      view = ''
    }
  }

  window.pet.onUpdate(data => {
    const before = now.mood
    now = data
    lang = data.lang || lang
    config = { ...(data.config || {}), sheetH: data.spriteVersion === 1 ? 1872 : 2288 }
    spriteUrl = data.sprite
    second = data.second || null
    place()
    // Something new that wants you, or is finished: she peeks out for a moment.
    if (isOn() && before !== now.mood && PEEK_CLIP[now.mood]) nudgeFor(PEEK_CLIP[now.mood], null)
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
      window.pet.hover(false)
      clearTimeout(hoverTimer)
      hoverTimer = setTimeout(() => {
        isHover = false
        update()
      }, HOVER_CLOSE_MS)
    })
    // A click opens the main window; the prompt's own buttons are not clicks on the island.
    target.addEventListener('mousedown', e => {
      if (e.button === 0 && !panel.contains(e.target)) window.pet.dragStart()
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
