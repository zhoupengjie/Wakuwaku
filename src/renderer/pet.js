// What the pet shows, decided every tick in this order:
//   dragged   runs the way the window is dragged
//   reaction  a one-off (wave, jump, failed) over the mood
//   walking   an idle stroll along the screen
//   looking   idle, eyes on the cursor (or a glance around): rows 9-10
//   mood      the mood's own loop
const { CELL_W, CELL_H, CLIPS, MOOD_CLIP, REACTIONS, lookCell } = window.Sprite

const LABEL = {
  idle: '摸鱼中',
  working: '干活中…',
  waiting: '等你回复！',
  done: '搞定啦 ✓',
  review: '改好了，来看看 ✓',
  error: '呜…出错了',
}

const COLOR = {
  idle: '#9aa4b2',
  working: '#4d6bfe',
  waiting: '#f5a524',
  done: '#17c964',
  review: '#7c3aed',
  error: '#f31260',
}

// The cursor holds her eyes this long after it last moved, within this reach.
const LOOK_HOLD_MS = 1800
const LOOK_NEAR = 40
const LOOK_FAR = 900

const sprite = document.getElementById('pet')
const bubble = document.getElementById('bubble')

let mood = 'idle'
let detail = ''
let config = { scale: 0.55, bubble: true, walk: true, look: true }
let spriteUrl = null

let reaction = null // { clip, times, say, start }
let dragged = null // { dir, at }
let walking = null // { dir }
let looking = null // { row, frame, until }
let loop = { clip: '', start: 0 }

// --- Drawing ----------------------------------------------------------------

let drawn = ''

function drawCell(row, frame) {
  const key = `${row}:${frame}:${config.scale}`
  if (key === drawn) return
  drawn = key
  const s = config.scale
  sprite.style.backgroundPosition = `${-frame * CELL_W * s}px ${-row * CELL_H * s}px`
}

// Loop a clip, keeping its clock while it stays the same clip.
function drawLoop(name, now) {
  if (loop.clip !== name) loop = { clip: name, start: now }
  const clip = CLIPS[name]
  drawCell(clip.row, Math.floor((now - loop.start) / clip.ms) % clip.frames)
}

function tick() {
  const now = performance.now()

  if (dragged && now - dragged.at < 160) {
    return drawLoop(dragged.dir > 0 ? 'running-right' : 'running-left', now)
  }

  if (reaction) {
    const clip = CLIPS[reaction.clip]
    const frame = Math.floor((now - reaction.start) / clip.ms)
    if (frame < clip.frames * reaction.times) {
      loop = { clip: '', start: 0 }
      return drawCell(clip.row, frame % clip.frames)
    }
    reaction = null
    render()
  }

  if (walking) {
    return drawLoop(walking.dir > 0 ? 'running-right' : 'running-left', now)
  }

  if (looking && mood === 'idle' && now < looking.until) {
    loop = { clip: '', start: 0 }
    return drawCell(looking.row, looking.frame)
  }

  drawLoop(MOOD_CLIP[mood], now)
}

setInterval(tick, 40)

// --- Bubble ---------------------------------------------------------------

function render() {
  const root = document.documentElement.style
  root.setProperty('--scale', config.scale)
  root.setProperty('--accent', COLOR[mood])
  root.setProperty('--sprite', spriteUrl ? `url("${spriteUrl}")` : 'none')
  drawn = ''

  // No sprite yet: say how to get one, and keep a box to right-click.
  if (!spriteUrl) {
    bubble.textContent = '还没有宠物素材：npm run fetch-pet'
    bubble.classList.remove('hidden')
    return
  }

  const label = detail ? `${LABEL[mood]} · ${detail}` : LABEL[mood]
  const text = reaction?.say || label
  bubble.textContent = text
  bubble.classList.toggle('hidden', !config.bubble || (mood === 'idle' && !reaction?.say))
  sprite.title = label
}

// --- Idle life: walk a little, glance around --------------------------------

let idleTimer
let glanceTimer

function scheduleIdle() {
  clearTimeout(idleTimer)
  if (mood !== 'idle') return
  idleTimer = setTimeout(idleAct, 12000 + Math.random() * 18000)
}

function isFree() {
  return mood === 'idle' && !reaction && !walking && !dragged && !(looking && performance.now() < looking.until)
}

async function idleAct() {
  if (!isFree()) return scheduleIdle()

  if (config.walk && Math.random() < 0.4) {
    const dir = Math.random() < 0.5 ? -1 : 1
    const distance = Math.round((60 + Math.random() * 140) * (config.scale / 0.55))
    walking = { dir }
    await window.pet.walk(dir * distance, distance * 18)
    walking = null
  } else {
    glance(2 + Math.floor(Math.random() * 2))
  }

  scheduleIdle()
}

// Look at a few random directions in turn, then back to idle.
function glance(times) {
  if (times <= 0 || mood !== 'idle') return
  const index = Math.floor(Math.random() * 16)
  const { row, frame } = lookCell(Math.sin((index * Math.PI) / 8), -Math.cos((index * Math.PI) / 8))
  looking = { row, frame, until: performance.now() + 700 }
  glanceTimer = setTimeout(() => glance(times - 1), 700)
}

function stopIdle() {
  clearTimeout(idleTimer)
  clearTimeout(glanceTimer)
  looking = null
  if (walking) {
    window.pet.walkStop()
    walking = null
  }
}

// --- From main --------------------------------------------------------------

window.pet.onUpdate(data => {
  const wasMood = mood
  mood = data.mood
  detail = data.detail || ''
  config = data.config || config
  spriteUrl = data.sprite

  if (mood !== 'idle') {
    stopIdle()
  } else if (wasMood !== 'idle') {
    scheduleIdle()
  }
  render()
})

window.pet.onReact(({ react, say }) => {
  const r = REACTIONS[react]
  if (!r) return
  stopIdle()
  reaction = { ...r, say, start: performance.now() }
  render()
  scheduleIdle()
})

// Eyes on the cursor while it moves near enough.
window.pet.onCursor(({ dx, dy }) => {
  if (!config.look || mood !== 'idle' || reaction || walking || dragged) return
  const distance = Math.hypot(dx, dy)
  if (distance < LOOK_NEAR || distance > LOOK_FAR) return
  clearTimeout(glanceTimer)
  looking = { ...lookCell(dx, dy), until: performance.now() + LOOK_HOLD_MS }
})

window.pet.onDrag(dx => {
  if (dx !== 0) dragged = { dir: Math.sign(dx), at: performance.now() }
})

window.pet.onDragEnd(() => {
  dragged = null
  scheduleIdle()
})

// --- Pointer: hover turns click-through off, press drags --------------------

sprite.addEventListener('mouseenter', () => window.pet.hover(true))
sprite.addEventListener('mouseleave', () => window.pet.hover(false))
sprite.addEventListener('mousedown', e => {
  if (e.button === 0) {
    stopIdle()
    window.pet.dragStart()
  }
})
window.addEventListener('mouseup', e => {
  if (e.button === 0) window.pet.dragEnd()
})
sprite.addEventListener('contextmenu', e => {
  e.preventDefault()
  window.pet.menu()
})

// The first pet:update (on load) brings the sprite and the first render.
scheduleIdle()
