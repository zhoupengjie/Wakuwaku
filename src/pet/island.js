// The island: a black pill at the top of the screen, after the iPhone's
// Dynamic Island. It springs between three shapes:
//   compact   her face, a few words and the clock (just the face when idle)
//   expanded  hovered, or for a few seconds when something happens: her
//             animated head, what is going on, the clock
//   ask       a prompt from Claude, answered right in the island
// and a second, round bubble beside it when another session wants you too.
//
// The window around it is fixed and clicks pass through it; only the island
// takes the pointer. Shapes change in CSS, never by resizing the window, so
// the spring stays smooth. A prompt is the exception: the window grows first,
// then the island; it shrinks back once the island has.
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

  // How long the island opens by itself when something happens, and how long
  // the pointer has to rest on it before it opens (so passing by does not).
  const PEEK_MS = 3200
  const HOVER_OPEN_MS = 140
  const HOVER_CLOSE_MS = 260
  // Matches the spring in style.css: the window waits this long to shrink.
  const SPRING_MS = 560
  const MIN_W = 112
  const HEAD = 48
  const HEAD_SCALE = 0.42

  const island = document.getElementById('island')
  const compact = document.getElementById('island-compact')
  const expanded = document.getElementById('island-expanded')
  const side = document.getElementById('island-side')
  const panel = document.getElementById('panel')
  const stage = document.getElementById('stage')

  let lang = 'en'
  let now = { mood: 'idle', detail: '', project: '', since: null, took: null, others: 0, sessions: 0 }
  let config = {}
  let spriteUrl = null
  let second = null
  let isHover = false
  let hoverTimer
  let peek = null // { until, text }
  let peekTimer
  let view = ''
  let clockTimer
  let shrinkTimer
  let headTimer
  let blinkTimer

  const isOn = () => config.display === 'island'
  const isAsking = () => isOn() && !panel.hidden

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

  // A round face with two capsule eyes, in a mood's colour.
  function face(mood, px) {
    const node = el('span', 'face')
    node.style.cssText = `width:${px}px;height:${px}px;background:${COLOR[mood] || COLOR.idle}`
    node.append(el('i'), el('i'))
    return node
  }

  // --- The shapes ---------------------------------------------------------------

  function fillCompact() {
    const label = now.mood === 'idle' ? '' : [now.project, t(lang, `island.${now.mood}`)].filter(Boolean).join(' · ')
    const clockText = el('span', 'clock', time())
    clockText.style.color = COLOR[now.mood]
    compact.replaceChildren(face(now.mood, 24), el('span', 'label', label), clockText)
    compact.classList.toggle('bare', !label && !clockText.textContent)
  }

  function fillExpanded() {
    const head = el('span', 'portrait')
    if (spriteUrl) {
      head.style.cssText = `width:${HEAD}px;height:${HEAD}px;background-image:url("${spriteUrl}");background-size:${1536 * HEAD_SCALE}px ${(config.sheetH || 2288) * HEAD_SCALE}px`
    } else {
      head.append(face(now.mood, HEAD))
    }
    const title = peek?.text || t(lang, `mood.${now.mood}`)
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
      const x = -(frame * CELL_W * HEAD_SCALE) - (CELL_W * HEAD_SCALE - HEAD) / 2
      const y = -(clip.row * CELL_H * HEAD_SCALE) - 4
      head.style.backgroundPosition = `${x}px ${y}px`
      frame = (frame + 1) % clip.frames
      if (view === 'expanded') headTimer = setTimeout(step, clip.ms)
    }
    step()
  }

  function fillSide() {
    side.replaceChildren(second ? face(second, 20) : '')
    side.classList.toggle('shown', !!second && view !== 'ask')
  }

  // The size a shape wants, measured off the page.
  function sizeOf(name) {
    if (name === 'ask') return { width: panel.offsetWidth + 28, height: panel.offsetHeight + 26 }
    if (name === 'expanded') return { width: Math.min(400, Math.max(300, expanded.offsetWidth)), height: 84 }
    return { width: Math.max(MIN_W, compact.offsetWidth), height: 36 }
  }

  function setSize({ width, height }) {
    island.style.width = `${width}px`
    island.style.height = `${height}px`
    island.style.borderRadius = `${height > 60 ? 30 : height / 2}px`
  }

  // --- Deciding ---------------------------------------------------------------

  function update() {
    clearInterval(clockTimer)
    if (!isOn()) return
    const was = view
    view = isAsking() ? 'ask' : isHover || (peek && Date.now() < peek.until) ? 'expanded' : 'compact'
    island.dataset.view = view
    fillCompact()
    fillExpanded()
    fillSide()

    const want = sizeOf(view)
    clearTimeout(shrinkTimer)
    if (view === 'ask') {
      // Room first, then the island grows into it.
      window.pet.panel({ width: want.width + 48, height: want.height + 40 })
      setTimeout(() => setSize(sizeOf('ask')), was === 'ask' ? 0 : 40)
    } else {
      setSize(want)
      if (was === 'ask') shrinkTimer = setTimeout(() => window.pet.panel(null), SPRING_MS)
    }

    if (time()) {
      clockTimer = setInterval(() => {
        for (const c of island.querySelectorAll('.clock')) c.textContent = time()
      }, 1000)
    }
  }

  function openFor(text) {
    peek = { until: Date.now() + PEEK_MS, text }
    clearTimeout(peekTimer)
    peekTimer = setTimeout(() => {
      peek = null
      update()
    }, PEEK_MS)
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
    // Something new that wants you, or is finished: open for a moment.
    if (isOn() && before !== now.mood && ['waiting', 'done', 'review', 'error'].includes(now.mood)) openFor(null)
    else update()
  })

  window.pet.onReact(({ say: text }) => {
    if (isOn() && text) openFor(say(lang, text))
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
