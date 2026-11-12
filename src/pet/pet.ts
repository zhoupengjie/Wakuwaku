// What the pet shows, decided whenever a frame is due, in this order:
//   dragged   runs the way the window is dragged
//   reaction  a one-off (wave, jump, failed) over the mood
//   walking   an idle stroll along the screen
//   looking   idle, eyes on the cursor (or a glance around): rows 9-10
//   mood      the mood's own loop
//
// Frames are drawn when they change, not on a fixed tick, so a pet at rest
// costs next to nothing.
import * as Status from './status.ts'
import { CELL_W, CELL_H, CLIPS, MOOD_CLIP, REACTIONS, lookCell } from './sprite.ts'
import { t } from '../shared/i18n.ts'
import { pet } from './bridge.ts'
import type { Config } from '../shared/config.ts'

// Her own window's page (index.html, role=pet; in the island's, role=island,
// she is not drawn here): made by main.ts. What it hands back: what the
// prompts' panel asks of her window (HerTalk: room).
export function herPage() {
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
  // The whole sheet behind her cell-sized box (style.css #pet-film); made
  // here should the page lack it (a harness with an older index.html).
  const film = document.getElementById('pet-film') || sprite.appendChild(Object.assign(document.createElement('div'), { id: 'pet-film' }))
  // Her words over her head out on the desktop (and, with no pet at all, how
  // to get one); the list she opens under the pointer, made here as the film is.
  const bubble = document.getElementById('bubble')
  const herList = document.getElementById('her-list') || sprite.parentElement.insertBefore(Object.assign(document.createElement('div'), { id: 'her-list', hidden: true }), sprite)
  const panelBox = document.getElementById('panel')
  // Under her feet while she is carried out of her home: a double-click sends her back.
  const hint = document.getElementById('her-hint') || sprite.parentElement.appendChild(Object.assign(document.createElement('div'), { id: 'her-hint', hidden: true }))

  let lang = 'en'
  // The session shown as main tells it (pet:update), with every session in
  // list; its id once it is one of theirs.
  interface Now {
    id?: string
    mood: string
    detail: any
    project: string
    since: number
    took: number
    others: number
    sessions: number
    list: any[]
  }
  let now: Now = { mood: 'idle', detail: '', project: '', since: null, took: null, others: 0, sessions: 0, list: [] }
  let config: Config = { scale: 0.55, details: true, walk: true, look: true, sound: false, dnd: false }
  let spriteUrl = null
  // 2: the 11-row sheet; 1: the older 9-row one, without the look-around rows.
  let spriteVersion = 2

  // The same page draws her window (role=pet) and the island's (role=island,
  // island.ts); in the island's, she is not drawn here.
  const ROLE = new URLSearchParams(location.search).get('role') === 'island' ? 'island' : 'pet'
  const isIsland = () => ROLE === 'island'
  // Out on the desktop she says it herself (the user's call, 2026-10-28: it
  // was all her home's, and the island at the top folds away under a
  // maximized window, so often nothing said anything): see "Her words" below.

  // Nobody at the keyboard or mouse for a while (pet:quiet): she stands still.
  let quiet = false

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
    // The sheet moved behind the cell: a transform on its own layer, no repaint.
    film.style.transform = `translate3d(${-frame * CELL_W * s}px, ${-row * CELL_H * s}px, 0)`
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

    // Nobody at the keyboard or mouse for a while: she stands still on her
    // mood's first frame (a reaction still plays; a drag still carries her).
    if (quiet && !reaction && !dragged && !walking) {
      loop = { clip: '', start: 0 }
      drawCell(CLIPS[MOOD_CLIP[now.mood]].row, 0)
      return 60 * 60 * 1000
    }

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
  // Not while her page is put away with her window (she is home: pet.rs):
  // drawn again as it shows.
  function kick() {
    clearTimeout(frameTimer)
    if (document.hidden) return
    const wait = frame()
    frameTimer = setTimeout(kick, Math.max(16, Math.ceil(wait)))
  }

  document.addEventListener('visibilitychange', () => {
    if (document.hidden) {
      stopIdle()
      closeList()
      hint.hidden = true
      clearTimeout(tickTimer)
      // Main may set her window's size anew meanwhile (pet.rs return_to_spot):
      // the room over her head told again as she shows.
      roomSent = undefined
      return clearTimeout(frameTimer)
    }
    kick()
    scheduleIdle()
    talk()
    room(true)
  })

  // --- Her words, out on the desktop -------------------------------------------------
  // Over her head, in the island's dark capsule, what the closed island says:
  // the session shown, what it is doing and for how long; or how its turn
  // ended, how long it took and Claude's last words, for as long as main holds
  // the ending (the hold setting) or until the pointer comes over her (seen).
  // Nothing at rest. A press on it goes to that session's window. A prompt
  // (panel.ts) takes its place, and so does the list she opens when the
  // pointer rests on her: every session, a press on one to its window.

  // The moods' colours on her dark capsule: the island's on black.
  const ON_DARK = { idle: '#8e8e93', working: '#5e9bff', waiting: '#ffb340', done: '#34d27b', review: '#b18cff', error: '#ff5c6c' }
  // The pointer rests on her this long before her list opens; off her and the
  // list this long before it closes (the way up from her to it crosses a gap).
  const LIST_MS = 400
  const LIST_LEAVE_MS = 300
  const LIST_ROWS = 6
  // Less room over her head is given back this long after: a turn ending and
  // the next starting do not move her window twice.
  const ROOM_KEEP_MS = 1500
  // What is over her head goes back into it this fast (style.css her-unpop).
  const POP_OUT_MS = 160

  const isOut = () => !isIsland() && config.out === true && !!spriteUrl
  const isAsking = () => document.body.classList.contains('asking')
  const detailed = () => config.details !== false

  function span(cls, text?) {
    const node = document.createElement('span')
    node.className = cls
    if (text) node.textContent = text
    return node
  }

  // A dot in the mood's colour, whose, what, "+N", the clock: her capsule's
  // line, and each line of her list.
  function line({ mood, name, what, clock, others }: { mood: string; name?: string; what?: string; clock?: string; others?: number }) {
    const row = span('row')
    const dot = span('dot')
    dot.style.background = ON_DARK[mood] || ON_DARK.idle
    row.append(dot)
    if (name) row.append(span('who', name))
    if (what) row.append(span('what', what))
    if (others) row.append(span('more', `+${others}`))
    if (clock) {
      const c = span('clock', clock)
      c.style.color = ON_DARK[mood] || ON_DARK.idle
      row.append(c)
    }
    return row
  }

  // Out of her head with the island's spring as it comes, back into it as it
  // goes, and once in place nothing moves (the design's motion: quick, light,
  // played once). Her capsule hides by a class, the rest by their hidden.
  const isUp = node => (node === bubble ? !node.classList.contains('hidden') : !node.hidden) && !node.classList.contains('out')
  const goTimers = new Map()

  function replayIn(node) {
    node.classList.remove('in')
    void node.offsetWidth
    node.classList.add('in')
  }

  function popIn(node) {
    const was = isUp(node)
    clearTimeout(goTimers.get(node))
    node.classList.remove('out')
    if (node === bubble) node.classList.remove('hidden')
    else node.hidden = false
    if (!was) replayIn(node)
  }

  function popOut(node, then) {
    clearTimeout(goTimers.get(node))
    if (!isUp(node)) return then?.()
    node.classList.remove('in')
    node.classList.add('out')
    goTimers.set(
      node,
      setTimeout(() => {
        node.classList.remove('out')
        if (node === bubble) node.classList.add('hidden')
        else node.hidden = true
        then?.()
        room()
      }, POP_OUT_MS),
    )
  }

  let saidKey = ''
  let tickTimer

  // Her capsule as the session shown is now; drawn again only when what it
  // says changes, its clock kept up each second in place (tick). Something
  // else said while it is up slides in, as the closed island's words do.
  function talk() {
    document.body.classList.toggle('her-out', isOut())
    const said = isOut() ? Status.talkOf(lang, now, { detailed: detailed() }) : null
    const key = said ? JSON.stringify({ ...said, clock: Status.isEnding(now) ? said.clock : '' }) : ''
    if (key !== saidKey) {
      saidKey = key
      if (said) {
        const lines = [line(said), ...(said.reply ? [span('reply', said.reply)] : [])]
        if (isUp(bubble) && bubble.childElementCount) for (const n of lines) n.classList.add('turn')
        bubble.replaceChildren(...lines)
        bubble.title = [[said.name, said.what].filter(Boolean).join(' · '), said.reply].filter(Boolean).join('\n')
        popIn(bubble)
      } else {
        popOut(bubble, () => {
          bubble.replaceChildren()
          bubble.title = ''
        })
      }
    }
    if (listOpen) fillList()
    tick()
    room()
  }

  // A busy session's clock (and an open list's) each second, while seen.
  function tick() {
    clearTimeout(tickTimer)
    if (!isOut() || quiet || document.hidden) return
    const busy = s => s.mood === 'working' || s.mood === 'waiting'
    if (!(saidKey && busy(now)) && !(listOpen && (now.list || []).some(busy))) return
    tickTimer = setTimeout(() => {
      const c = bubble.querySelector('.clock')
      if (c && busy(now)) c.textContent = Status.time(now)
      if (listOpen) fillList()
      tick()
      room()
    }, 1000 - (Date.now() % 1000) + 20)
  }

  bubble.addEventListener('click', () => {
    if (isOut() && now.id) pet.jump(now.id)
  })
  bubble.addEventListener('mouseenter', () => isOut() && pet.hover(true, false))
  bubble.addEventListener('mouseleave', () => isOut() && pet.hover(false, false))

  // --- Her list, under the pointer ----------------------------------------------------

  let listOpen = false
  let listTimer
  let leaveTimer
  let listKey = ''

  // Drawn again only when its lines change; their clocks kept up in place.
  function fillList() {
    const all = now.list && now.list.length ? now.list : now.id ? [now] : []
    const sessions = all.slice(0, LIST_ROWS)
    const said = sessions.map(s => Status.rowOf(lang, s, { detailed: detailed() }))
    const key = JSON.stringify([lang, all.length, said.map(r => ({ ...r, clock: '' }))])
    if (key === listKey) {
      herList.querySelectorAll('.row').forEach((row, i) => {
        const c = row.querySelector('.clock')
        if (c && said[i]) c.textContent = said[i].clock
      })
      return
    }
    listKey = key
    const rows = said.map(r => {
      const row = line(r)
      if (r.id) {
        row.classList.add('go')
        row.addEventListener('click', () => {
          closeList()
          pet.jump(r.id)
        })
      }
      return row
    })
    const more = all.length - sessions.length
    const head = span('head', all.length ? t(lang, all.length === 1 ? 'her.session' : 'her.sessions', { n: all.length }) : t(lang, 'her.noSessions'))
    const foot = span('foot')
    const button = (text, act) => {
      const b = span('btn', text)
      b.addEventListener('click', () => {
        closeList()
        act()
      })
      return b
    }
    foot.append(button(t(lang, 'her.settings'), () => pet.openSettings()), button(t(lang, 'her.home'), () => pet.doubleClick()))
    herList.replaceChildren(head, ...rows, ...(more > 0 ? [span('rest', t(lang, 'status.moreSessions', { n: more }))] : []), foot)
  }

  function openList() {
    clearTimeout(listTimer)
    if (!isOut() || isAsking() || dragged || listOpen) return
    listOpen = true
    fillList()
    popIn(herList)
    document.body.classList.add('listing')
    tick()
    room()
  }

  // Back into her head; her capsule, under it meanwhile, comes out again.
  function closeList() {
    clearTimeout(listTimer)
    clearTimeout(leaveTimer)
    if (!listOpen) return
    listOpen = false
    listKey = ''
    popOut(herList, () => {
      herList.replaceChildren()
      document.body.classList.remove('listing')
      if (isUp(bubble) && !isAsking()) replayIn(bubble)
    })
  }

  // Off her and the list: it closes in a moment, unless the pointer comes back.
  function leaveSoon() {
    clearTimeout(listTimer)
    clearTimeout(leaveTimer)
    leaveTimer = setTimeout(closeList, LIST_LEAVE_MS)
  }

  herList.addEventListener('mouseenter', () => {
    clearTimeout(leaveTimer)
    pet.hover(true, false)
  })
  herList.addEventListener('mouseleave', () => {
    pet.hover(false, false)
    leaveSoon()
  })

  // --- The room over her head ---------------------------------------------------------

  // What is over her head (a prompt, her list, her words) measured for main:
  // her window grows up and out for more than it has above her, she staying
  // put (pet.rs set_panel); less is given back a moment later.
  let roomSent
  // Less room on its way (its timer, and how much), not put off again by the
  // clock's tick asking the same.
  let roomTimer = null
  let roomLess

  // A prompt comes out of her head; once it is answered, her capsule again.
  let wasAsking = false

  function room(atOnce = false) {
    if (isIsland()) return
    requestAnimationFrame(() => {
      const asking = isAsking()
      if (asking !== wasAsking) {
        wasAsking = asking
        if (asking) replayIn(panelBox)
        else if (isUp(bubble)) replayIn(bubble)
      }
      if (asking && listOpen) return closeList()
      const shown = [panelBox, herList, bubble].find(n => n && !n.hidden && !n.classList.contains('hidden') && n.offsetHeight > 0)
      const want = shown ? { width: Math.ceil(shown.offsetWidth) + 16, height: Math.ceil(shown.offsetHeight) + 12 } : null
      const same = (a, b) => JSON.stringify(a) === JSON.stringify(b)
      const send = () => {
        clearTimeout(roomTimer)
        roomTimer = null
        roomSent = want
        pet.panel(want)
      }
      if (same(want, roomSent)) {
        clearTimeout(roomTimer)
        roomTimer = null
        return
      }
      const isLess = !want || (roomSent && want.width <= roomSent.width && want.height <= roomSent.height)
      if (!isLess || atOnce) return send()
      if (roomTimer && same(want, roomLess)) return
      clearTimeout(roomTimer)
      roomLess = want
      roomTimer = setTimeout(send, ROOM_KEEP_MS)
    })
  }

  const herTalk = { room }

  // The bubble never pushes her out of her window: one line for the status
  // and one for the others, cut short with an ellipsis; something she says
  // (a task's name can be long) two lines at most. All of it in the tooltip.
  function fillBubble(lines, isSaying = false) {
    const line = (cls, text) => {
      const node = document.createElement('span')
      node.className = cls
      node.textContent = text
      return node
    }
    bubble.replaceChildren(
      ...(isSaying ? [line('say', lines.main)] : [line('line', lines.main), ...(lines.more ? [line('line more', lines.more)] : [])]),
    )
    bubble.title = [lines.main, lines.more].filter(Boolean).join('\n')
  }

  function render() {
    const root = document.documentElement.style
    root.setProperty('--scale', config.scale as any)
    root.setProperty('--accent', COLOR[now.mood])
    root.setProperty('--sprite', spriteUrl ? `url("${spriteUrl}")` : 'none')
    root.setProperty('--sheet-h', `${spriteVersion === 2 ? 2288 : 1872}px`)
    drawn = ''

    if (isIsland()) return

    // No sprite yet: say how to get one, and keep a box to right-click.
    if (!spriteUrl) {
      document.body.classList.remove('her-out')
      closeList()
      saidKey = 'noSprite'
      fillBubble({ main: t(lang, 'say.noSprite') }, true)
      bubble.classList.remove('hidden')
      sprite.classList.add('empty')
      return
    }
    sprite.classList.remove('empty')
    if (saidKey === 'noSprite') saidKey = '-'
    // Her language heard only as her window shows, after the hint came.
    if (!hint.hidden) fillHint()
    talk()
  }

  // --- Idle life: walk a little, glance around ---------------------------------------

  let idleTimer
  let glanceTimer

  function scheduleIdle() {
    clearTimeout(idleTimer)
    if (now.mood !== 'idle' || isIsland() || document.hidden) return
    idleTimer = setTimeout(idleAct, 12000 + Math.random() * 18000)
  }

  // Free for a walk or a glance: at rest, someone about, and she out to be seen.
  function isFree() {
    return now.mood === 'idle' && !quiet && !document.hidden && !reaction && !walking && !dragged && !(looking && performance.now() < looking.until)
  }

  async function idleAct() {
    if (!isFree()) return scheduleIdle()

    if (config.walk && Math.random() < 0.4) {
      const dir = Math.random() < 0.5 ? -1 : 1
      const distance = Math.round((60 + Math.random() * 140) * (config.scale / 0.55))
      walking = { dir }
      kick()
      await pet.walk(dir * distance, distance * 18)
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
      pet.walkStop()
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

  pet.onUpdate(data => {
    const wasMood = now.mood
    now = data
    lang = data.lang || lang
    config = data.config || config
    spriteUrl = data.sprite
    spriteVersion = data.spriteVersion === 1 ? 1 : 2
    quiet = data.quiet === true

    if (now.mood !== 'idle') {
      stopIdle()
    } else if (wasMood !== 'idle') {
      scheduleIdle()
    }
    render()
    kick()
  })

  pet.onQuiet(({ on }) => {
    quiet = on === true
    kick()
    tick()
  })

  function react({ react: name, say: text, times }) {
    const r = REACTIONS[name]
    if (!r) return
    stopIdle()
    reaction = { ...r, ...(times ? { times } : {}), say: text, start: performance.now() }
    render()
    kick()
    scheduleIdle()
  }

  pet.onReact(react)

  pet.onAlert(({ mood }) => chime(mood))

  // Eyes on the cursor while it moves near enough.
  pet.onCursor(({ dx, dy }) => {
    if (!config.look || isIsland() || spriteVersion !== 2 || now.mood !== 'idle' || reaction || walking || dragged) return
    const distance = Math.hypot(dx, dy)
    if (distance < LOOK_NEAR || distance > LOOK_FAR) return
    clearTimeout(glanceTimer)
    looking = { ...lookCell(dx, dy), until: performance.now() + LOOK_HOLD_MS }
    kick()
  })

  pet.onDrag(dx => {
    if (dx === 0) return
    const isNew = !dragged
    dragged = { dir: Math.sign(dx), at: performance.now() }
    if (isNew) {
      closeList()
      kick()
    }
  })

  pet.onDragEnd(() => {
    dragged = null
    kick()
    scheduleIdle()
  })

  // Carried out of her home, the button still held: under her feet, that a
  // double-click sends her back (main makes the room for it, pet.rs carry).
  function fillHint() {
    hint.replaceChildren(span('', t(lang, 'her.dblHome')))
  }

  pet.onCarried?.(({ on }) => {
    if (on) fillHint()
    hint.hidden = on !== true
  })

  // --- Pointer: hover turns click-through off, press drags --------------------------------

  // Out on the desktop, resting on her opens her list (a press does not).
  sprite.addEventListener('mouseenter', () => {
    pet.hover(true)
    clearTimeout(leaveTimer)
    clearTimeout(listTimer)
    if (isOut() && !listOpen) listTimer = setTimeout(openList, LIST_MS)
  })
  sprite.addEventListener('mouseleave', () => {
    pet.hover(false)
    if (listOpen) leaveSoon()
    else clearTimeout(listTimer)
  })
  sprite.addEventListener('mousedown', e => {
    // Into the debug log (when on): which window a press reaches.
    console.log('mousedown on her, button', e.button)
    closeList()
    if (e.button === 0) {
      stopIdle()
      pet.dragStart()
    }
  })
  sprite.addEventListener('contextmenu', e => {
    e.preventDefault()
    closeList()
    pet.menu()
  })

  window.addEventListener('mouseup', e => {
    console.log('mouseup, button', e.button)
    if (e.button === 0) pet.dragEnd()
  })
  // A double-click: back into the island if she is out of it, else the main
  // window (main decides). A single one makes her jump.
  sprite.addEventListener('dblclick', () => pet.doubleClick())

  // The first pet:update (on load) brings the sprite and the first render.
  kick()
  scheduleIdle()
  return herTalk
}
export type HerTalkFace = ReturnType<typeof herPage>
