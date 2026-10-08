// What the pet shows, decided whenever a frame is due, in this order:
//   dragged   runs the way the window is dragged
//   reaction  a one-off (wave, jump, failed) over the mood
//   walking   an idle stroll along the screen
//   looking   idle, eyes on the cursor (or a glance around): rows 9-10
//   mood      the mood's own loop
//
// Frames are drawn when they change, not on a fixed tick, so a pet at rest
// costs next to nothing.
const { CELL_W, CELL_H, CLIPS, MOOD_CLIP, REACTIONS, lookCell } = window.Sprite
const { t, render: say } = window.I18n

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

let lang = 'en'
let now = { mood: 'idle', detail: '', project: '', since: null, took: null, others: 0, sessions: 0 }
let config = { scale: 0.55, bubble: true, walk: true, look: true, sound: false, dnd: false }
let spriteUrl = null
// 2: the 11-row sheet; 1: the older 9-row one, without the look-around rows.
let spriteVersion = 2

// The same page draws her window (role=pet) and the island's (role=island,
// island.js); in the island's, she is not drawn here.
const ROLE = new URLSearchParams(location.search).get('role') === 'island' ? 'island' : 'pet'
const isIsland = () => ROLE === 'island'
// Out of the island on the desktop, the island does the talking.
const isOutOfIsland = () => ROLE === 'pet' && config.display === 'island'

let reaction = null // { clip, times, say, start }
let dragged = null // { dir, at }
let walking = null // { dir }
let looking = null // { row, frame, until }
let loop = { clip: '', start: 0 }

// --- Drawing ------------------------------------------------------------------

let drawn = ''
let frameTimer

function drawCell(row, frame) {
  const key = `${row}:${frame}:${config.scale}`
  if (key === drawn) return
  drawn = key
  const s = config.scale
  sprite.style.backgroundPosition = `${-frame * CELL_W * s}px ${-row * CELL_H * s}px`
}

// At rest she blinks through the idle row once, then holds still this long.
// Every frame of a transparent window costs a full repaint (most of her CPU),
// and a pet at rest is most of the day.
const IDLE_HOLD_MS = 4000

// Loop a clip, keeping its clock while it stays the same clip. Returns the
// time until its next frame.
function drawLoop(name, at) {
  if (loop.clip !== name) loop = { clip: name, start: at }
  const clip = CLIPS[name]
  const t = at - loop.start

  if (name === 'idle') {
    const playMs = clip.frames * clip.ms
    const inCycle = t % (playMs + IDLE_HOLD_MS)
    if (inCycle >= playMs) {
      drawCell(clip.row, 0)
      return playMs + IDLE_HOLD_MS - inCycle
    }
    drawCell(clip.row, Math.floor(inCycle / clip.ms))
    return clip.ms - (inCycle % clip.ms)
  }

  drawCell(clip.row, Math.floor(t / clip.ms) % clip.frames)
  return clip.ms - (t % clip.ms)
}

// Draw what is due now; returns ms until something next changes.
function frame() {
  const at = performance.now()

  // In the island she is not drawn at all.
  if (isIsland()) return 60 * 60 * 1000

  if (dragged && at - dragged.at < 160) {
    drawLoop(dragged.dir > 0 ? 'running-right' : 'running-left', at)
    return 40
  }

  if (reaction) {
    const clip = CLIPS[reaction.clip]
    const step = Math.floor((at - reaction.start) / clip.ms)
    if (step < clip.frames * reaction.times) {
      loop = { clip: '', start: 0 }
      drawCell(clip.row, step % clip.frames)
      return clip.ms - ((at - reaction.start) % clip.ms)
    }
    reaction = null
    render()
  }

  if (walking) return drawLoop(walking.dir > 0 ? 'running-right' : 'running-left', at)

  if (looking && spriteVersion === 2 && now.mood === 'idle' && at < looking.until) {
    loop = { clip: '', start: 0 }
    drawCell(looking.row, looking.frame)
    return looking.until - at
  }

  return drawLoop(MOOD_CLIP[now.mood], at)
}

// Draw now and plan the next frame; called again on anything that changes her.
function kick() {
  clearTimeout(frameTimer)
  const wait = frame()
  frameTimer = setTimeout(kick, Math.max(16, Math.ceil(wait)))
}

// --- Bubble -------------------------------------------------------------------

function clock(ms) {
  const s = Math.max(0, Math.floor(ms / 1000))
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  const sec = String(s % 60).padStart(2, '0')
  return h ? `${h}:${String(m).padStart(2, '0')}:${sec}` : `${m}:${sec}`
}

// The bubble's words: whose, what, for how long, and who else is busy.
function words() {
  const parts = [t(lang, `mood.${now.mood}`)]
  const detail = say(lang, now.detail)
  if (detail) parts.push(detail)
  if ((now.mood === 'working' || now.mood === 'waiting') && now.since) parts.push(clock(Date.now() - now.since))
  if ((now.mood === 'done' || now.mood === 'review' || now.mood === 'error') && now.took) parts.push(t(lang, 'detail.took', { time: clock(now.took) }))
  let text = parts.join(' · ')
  // Several sessions: say whose this is, and how many others are busy.
  if (now.project && now.sessions > 1) text = `${now.project}：${text}`
  if (now.others > 0) text += `\n${t(lang, 'detail.moreSessions', { n: now.others })}`
  return text
}

let clockTimer

function render() {
  const root = document.documentElement.style
  root.setProperty('--scale', config.scale)
  root.setProperty('--accent', COLOR[now.mood])
  root.setProperty('--sprite', spriteUrl ? `url("${spriteUrl}")` : 'none')
  root.setProperty('--sheet-h', `${spriteVersion === 2 ? 2288 : 1872}px`)
  drawn = ''

  clearInterval(clockTimer)
  if (isIsland()) return

  // No sprite yet: say how to get one, and keep a box to right-click.
  if (!spriteUrl) {
    bubble.textContent = t(lang, 'say.noSprite')
    bubble.classList.remove('hidden')
    sprite.classList.add('empty')
    return
  }
  sprite.classList.remove('empty')

  const text = reaction?.say ? say(lang, reaction.say) : words()
  bubble.textContent = text
  bubble.classList.toggle('hidden', !config.bubble || isOutOfIsland() || (now.mood === 'idle' && !reaction?.say))
  sprite.title = words()

  // The running clock in the bubble.
  if (!reaction?.say && now.since && (now.mood === 'working' || now.mood === 'waiting')) {
    clockTimer = setInterval(() => {
      bubble.textContent = words()
    }, 1000)
  }
}

// --- Idle life: walk a little, glance around ---------------------------------------

let idleTimer
let glanceTimer

function scheduleIdle() {
  clearTimeout(idleTimer)
  if (now.mood !== 'idle' || isIsland()) return
  idleTimer = setTimeout(idleAct, 12000 + Math.random() * 18000)
}

function isFree() {
  return now.mood === 'idle' && !reaction && !walking && !dragged && !(looking && performance.now() < looking.until)
}

async function idleAct() {
  if (!isFree()) return scheduleIdle()

  if (config.walk && Math.random() < 0.4) {
    const dir = Math.random() < 0.5 ? -1 : 1
    const distance = Math.round((60 + Math.random() * 140) * (config.scale / 0.55))
    walking = { dir }
    kick()
    await window.pet.walk(dir * distance, distance * 18)
    walking = null
    kick()
  } else {
    glance(2 + Math.floor(Math.random() * 2))
  }

  scheduleIdle()
}

// Look at a few random directions in turn, then back to idle.
function glance(times) {
  if (times <= 0 || now.mood !== 'idle' || spriteVersion !== 2) return
  const index = Math.floor(Math.random() * 16)
  const { row, frame: cell } = lookCell(Math.sin((index * Math.PI) / 8), -Math.cos((index * Math.PI) / 8))
  looking = { row, frame: cell, until: performance.now() + 700 }
  kick()
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

// --- Sound: two soft notes, made here (no audio files) ---------------------------------

let audio

function chime(mood) {
  if (!config.sound || config.dnd) return
  try {
    audio = audio || new AudioContext()
    const notes = mood === 'error' ? [440, 330] : mood === 'waiting' ? [660, 880] : [523, 784]
    notes.forEach((hz, i) => {
      const osc = audio.createOscillator()
      const gain = audio.createGain()
      const at = audio.currentTime + i * 0.16
      osc.frequency.value = hz
      gain.gain.setValueAtTime(0.0001, at)
      gain.gain.exponentialRampToValueAtTime(0.12, at + 0.02)
      gain.gain.exponentialRampToValueAtTime(0.0001, at + 0.3)
      osc.connect(gain).connect(audio.destination)
      osc.start(at)
      osc.stop(at + 0.32)
    })
  } catch {
    // No sound device: nothing to play on.
  }
}

// --- From main --------------------------------------------------------------------------

window.pet.onUpdate(data => {
  const wasMood = now.mood
  now = data
  lang = data.lang || lang
  config = data.config || config
  spriteUrl = data.sprite
  spriteVersion = data.spriteVersion === 1 ? 1 : 2

  if (now.mood !== 'idle') {
    stopIdle()
  } else if (wasMood !== 'idle') {
    scheduleIdle()
  }
  render()
  kick()
})

window.pet.onReact(({ react, say: text }) => {
  const r = REACTIONS[react]
  if (!r) return
  stopIdle()
  reaction = { ...r, say: text, start: performance.now() }
  render()
  kick()
  scheduleIdle()
})

window.pet.onAlert(({ mood }) => chime(mood))

// Eyes on the cursor while it moves near enough.
window.pet.onCursor(({ dx, dy }) => {
  if (!config.look || isIsland() || spriteVersion !== 2 || now.mood !== 'idle' || reaction || walking || dragged) return
  const distance = Math.hypot(dx, dy)
  if (distance < LOOK_NEAR || distance > LOOK_FAR) return
  clearTimeout(glanceTimer)
  looking = { ...lookCell(dx, dy), until: performance.now() + LOOK_HOLD_MS }
  kick()
})

window.pet.onDrag(dx => {
  if (dx === 0) return
  const isNew = !dragged
  dragged = { dir: Math.sign(dx), at: performance.now() }
  if (isNew) kick()
})

window.pet.onDragEnd(() => {
  dragged = null
  kick()
  scheduleIdle()
})

// --- Pointer: hover turns click-through off, press drags --------------------------------

sprite.addEventListener('mouseenter', () => window.pet.hover(true))
sprite.addEventListener('mouseleave', () => window.pet.hover(false))
sprite.addEventListener('mousedown', e => {
  // Into the debug log (when on): which window a press reaches.
  console.log('mousedown on her, button', e.button)
  if (e.button === 0) {
    stopIdle()
    window.pet.dragStart()
  }
})
sprite.addEventListener('contextmenu', e => {
  e.preventDefault()
  window.pet.menu()
})

window.addEventListener('mouseup', e => {
  console.log('mouseup, button', e.button)
  if (e.button === 0) window.pet.dragEnd()
})
// A double-click: back into the island if she is out of it, else the main
// window (main decides). A single one makes her jump.
sprite.addEventListener('dblclick', () => window.pet.doubleClick())

// The first pet:update (on load) brings the sprite and the first render.
kick()
scheduleIdle()
