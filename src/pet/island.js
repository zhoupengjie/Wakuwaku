// Her home: by display, a black pill at the top centre after the iPhone's
// Dynamic Island (island), or a strip along the bottom of the screen in
// place of Windows' taskbar (taskbar): its left end hers, the other sessions
// as tags along the rest, with the Start button, the tray's icons
// (taskbar.rs) and the clock besides. The same element in each, which
// springs between three shapes (below the island; in the taskbar, growing
// up from it):
//   compact   her round portrait, where the session is (or whose ending it
//             is) and the clock
//   expanded  hovered, or for a few seconds when something happens (a turn
//             done, an error, she needs you): her whole self standing in it,
//             playing that mood, beside the session's name, where it is, how
//             its turn ended, and a line for each other session; a click on
//             a session goes to its window (jump.rs)
//   ask       a prompt from Claude, answered right in the island, her beside it
// Other sessions busy too: a small "+N" by the clock, in the colour of the
// next one that wants something. The words come from status.js.
//
// While no session needs anything, plugins (widgets.rs, widgets.js) take
// turns in the compact island, and the open one lists them all; the wheel
// turns to the next, a click on one keeps it. One may ask to open the island
// for itself (a nudge): never while a session wants you.
//
// She is one element throughout: the portrait grows into her whole self and
// shrinks back, so she is never in two places at once.
//
// Press her and pull down, and the island stretches like a drop of ink with
// her in it; pull far enough and the drop pinches off and she is out: her own
// window takes her, under the cursor, and she lands where you let go. Let go
// before that and she springs back in. The island stays while she is out, her
// seat empty; bring her close and it reaches out a drop for her, let go there
// and it takes her back in.
//
// This page runs in the island's own window (role=island); her window loads
// the same page as role=pet.
//
// The window is just big enough for all this and clicks pass through it; only
// the island takes the pointer. Shapes change in CSS, never by resizing the
// window frame by frame, so the spring stays smooth. When a shape needs more
// room, the window grows first and the island after; when it needs less, the
// island shrinks first and the window after.
;(function () {
  const { t, render: say } = window.I18n
  const { CELL_W, CELL_H, CLIPS, MOOD_CLIP } = window.Sprite
  const Status = window.Status
  const Widgets = window.Widgets

  // Brighter than the pet's colours: these sit on black. On the light
  // taskbar (Windows' light mode) the same, deep enough to read on white.
  const COLOR = {
    idle: '#8e8e93',
    working: '#5e9bff',
    waiting: '#ffb340',
    done: '#34d27b',
    review: '#b18cff',
    error: '#ff5c6c',
  }
  const COLOR_LIGHT = {
    idle: '#6e6e73',
    working: '#0067c0',
    waiting: '#b25e00',
    done: '#0f7b3f',
    review: '#7346c9',
    error: '#c42b3c',
  }
  const hue = () => (isLight() ? COLOR_LIGHT : COLOR)
  // A widget's own colour (a script's, or the monitor's), on the light
  // island as deep as the moods' there: one of the moods' colours as its
  // deeper one, any other darkened until it reads on white (widgets.js).
  function ink(colour) {
    if (!colour || !isLight()) return colour
    const mood = Object.keys(COLOR).find(k => COLOR[k] === String(colour).toLowerCase())
    return mood ? COLOR_LIGHT[mood] : Widgets.deepen(colour)
  }

  // What she plays when the island opens by itself for each mood.
  const NUDGE_CLIP = { waiting: 'waiting', done: 'waving', review: 'review', error: 'failed' }

  // How long the island stays open when something happens (longer with
  // Claude's words to read), and how long the pointer has to rest on it
  // before it opens (so passing by does not).
  const NUDGE_MS = 3600
  const NUDGE_READ_MS = 6500
  const HOVER_OPEN_MS = 140
  const HOVER_CLOSE_MS = 260
  // Matches the springs in style.css: the window waits this long to shrink.
  const SPRING_MS = 560
  const MIN_W = 112
  // The compact island's width, as set (narrow, normal, wide): the same
  // whatever it holds, so it keeps its size while the words change and the
  // widgets take turns. Just her (MIN_W) when there is nothing to say.
  const WIDTHS = { narrow: 240, normal: 300, wide: 380 }
  // The window without extra room (main's ISLAND), and the island's distance
  // from the top of it. As wide as a pull needs (PULL_ROOM), so the window
  // only ever grows down: a window whose top-left corner moves shows its old
  // picture from the new corner for a frame or two, and the island jumped
  // sideways as she was pulled out or taken back.
  const BASE = { width: 760, height: 132 }
  const TOP = 8
  // Her two sizes: the round portrait in the compact island, and her whole
  // self (one sheet cell, halved) standing in the open one.
  const HEAD = 24
  const HEAD_SCALE = 0.21
  // Her portrait in the settings' head.
  const SET_HEAD = 32
  const SET_HEAD_SCALE = 0.28
  // The taskbar's strip, the room kept above it (both island.rs's), and her
  // portrait in it.
  const TASKBAR_H = 48
  const TASKBAR_ROOM = 640
  // The settings above the taskbar: clear of it, and room for their shadow.
  const TASKBAR_GAP = 8
  const TASKBAR_SHADE = 12
  const TASK_HEAD = 30
  // On the taskbar's Mica her end is an island of its own in the strip: a
  // black capsule this far in from the strip's left end and its foot.
  const CAPSULE_X = 6
  const CAPSULE_Y = 5
  // A portrait px across is the sheet at this scale.
  const PER_PX = 0.00875
  // The settings' width (the Mail page in two panes is wider: settings.js).
  const SETTINGS_W = 520
  const BODY_SCALE = 0.5
  const BODY_W = Math.round(CELL_W * BODY_SCALE)
  const BODY_H = Math.round(CELL_H * BODY_SCALE)
  const OPEN_H = BODY_H + 12
  // The open island's widest, with her and without: a little over what its
  // usual lines take (the monitor's row, the session's), so a long one (a
  // letter's subject) ends in "…" rather than widening it. The other
  // sessions it lists.
  const OPEN_MAX_W = 420
  const OPEN_BARE_MAX_W = 360
  const OTHERS_SHOWN = 3
  // Pulling her out: the room the window takes for it, how far the drop
  // stretches before it pinches off, and the drop's size.
  const PULL_ROOM = { width: 760, height: 440 }
  const BREAK_PX = 110
  const DROP_R = 56
  // Reaching for her: how far the drop goes out when she is near (part of
  // the way), and its size at the start.
  const REACH_SHARE = 0.55
  const REACH_R = 22

  const body = document.body
  const island = document.getElementById('island')
  const compact = document.getElementById('island-compact')
  const expanded = document.getElementById('island-expanded')
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
  let now = { mood: 'idle', detail: '', project: '', since: null, took: null, others: 0, sessions: 0, list: [] }
  let config = {}
  // Windows' look as taskbar.rs sent it last: its mode (dark, light) and accent colour.
  let look = { mode: 'dark' }
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
  // Her being pulled out: { x0, y0, started, broken, at, carrying }. Once
  // the drop pinches off she is carrying: her window has her, this page only
  // waits for the button to be let go.
  let pull = null
  // When the last pull ended: the click the browser sends after it (the press
  // on her, the release on the island) is not a click on the island.
  let pulledAt = 0
  let dropTimer
  // Reaching for her (her middle on the screen, close), or taking her in.
  let isReaching = false
  let isAbsorbing = false
  // Taken back in: she is home from then on for this page, before main says
  // so, so she lands in her live portrait and not in the still one.
  let isComingHome = false
  let comingHomeTimer
  // Her live portrait was showing at the last draw: one showing again is put
  // in its place at once, not sprung there from where she last stood (her
  // whole self, if she was pulled out of the open island).
  let herShown = false
  let retractTimer
  // The session a press in the open island landed on: the clocks redraw it
  // every second, so the release may come down on its redrawn self.
  let pressed = { id: '', at: 0 }
  // The widgets that are on, the one shown, and the turn to the next.
  let widgets = []
  let widgetAt = 0
  // The monitor, pinned (its setting, on by default): always in sight at the
  // island's right end (the bar's), never one of the turns. Unpinned it is
  // one of the widgets taking turns.
  let monitor = null
  // What the compact island showed last, and when something else came: it
  // slides in, and carries on sliding in when the island is drawn again.
  let shownKey = ''
  let turnedAt = 0
  // The compact island's width, as drawn last.
  let compactWidth = MIN_W
  let spinTimer = null

  const ROLE = new URLSearchParams(location.search).get('role') === 'island' ? 'island' : 'pet'
  // Risen only for the settings (she is the pet on her own): it goes once they close.
  // This page draws her home in the home's window; her own window draws her.
  const isOn = () => ROLE === 'island'
  // Her home: the island, or the taskbar.
  const home = () => (config.display === 'taskbar' ? 'taskbar' : 'island')
  // A strip across the screen (the taskbar): the other sessions have tags in it.
  const isStrip = () => home() === 'taskbar'
  // The taskbar's strip the desktop's picture through (mica.js), neither
  // solid nor clear.
  const isMica = () => isOn() && home() === 'taskbar' && !['black', 'clear'].includes(config.taskbarMaterial)
  // The taskbar's strip clear, the desktop itself through it; while a window
  // is maximized or full screen on its display (taskbar.rs "max") it fills
  // in, as Mica (its default) or solid, unless it is to stay clear always
  // (taskbarClearWhen).
  const isClear = () => isOn() && home() === 'taskbar' && config.taskbarMaterial === 'clear'
  const clearWhen = () => (['solid', 'always'].includes(config.taskbarClearWhen) ? config.taskbarClearWhen : 'mica')
  const isFilled = () => isClear() && clearWhen() !== 'always' && taskWindows.some(a => (a.windows || []).some(w => w.max && !w.min))
  // The taskbar light, as Windows' own is in its light mode (taskbar.rs).
  const isLight = () => isOn() && home() === 'taskbar' && look.mode === 'light'
  // Her end a capsule of its own in the strip (black, or light in Windows'
  // light mode), wherever the strip is not black itself: on the Mica, clear,
  // or light.
  const isCapsule = () => isMica() || isClear() || isLight()
  // The compact taskbar island's height: the strip's, or the capsule's.
  const taskCompactH = () => (isCapsule() ? TASKBAR_H - 2 * CAPSULE_Y : TASKBAR_H)
  // In her seat: there is a pet to show, and she is not out on the desktop.
  const isHome = () => !!spriteUrl && (config.out !== true || isComingHome)
  const isAsking = () => isOn() && !panel.hidden
  // The settings open in the island (settings.js).
  const isSetting = () => isOn() && !!window.Settings?.isOpen()
  const isNudging = () => !!nudge && Date.now() < nudge.until
  // The specifics on screen (session names, commands, replies), unless turned off.
  const isDetailed = () => config.details !== false
  // Nothing from the sessions to show: the widgets' turn.
  const isQuiet = () => now.mood === 'idle'
  const shownWidget = () => (isQuiet() && widgets.length ? widgets[widgetAt % widgets.length] : null)

  // The clock: how long this turn has run, or how long the finished one took.
  const time = () => Status.time(now)

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
    node.style.cssText = `width:${px}px;height:${px}px;background:${hue()[mood] || hue().idle}`
    node.append(el('i'), el('i'))
    return node
  }

  // Her in the island's line while she is out on the desktop (her own
  // element went with her): a round portrait, her mood's first frame, ringed
  // in its colour. The face stands in only when there is no pet at all.
  function stillHer(px) {
    if (!spriteUrl) return face(now.mood, px)
    const scale = px * 0.00875
    const node = el('span', 'still-her')
    node.style.cssText = `width:${px}px;height:${px}px;box-shadow:0 0 0 1.5px ${hue()[now.mood] || hue().idle}`
    const sheet = el('span', 'sheet')
    const clip = MOOD_CLIP[now.mood]
    sheet.dataset.clip = clip
    sheet.style.cssText = `background-image:url("${spriteUrl}");transform:translate(${-(CELL_W * scale - px) / 2}px, -2px) scale(${scale})`
    showFrame(sheet, CLIPS[clip].row, frameNow(CLIPS[clip]))
    node.append(sheet)
    return node
  }

  // The frame a clip is on now, by the clock: a portrait drawn anew (the
  // island redraws every second) carries on where the last one was.
  const frameNow = clip => Math.floor(Date.now() / clip.ms) % clip.frames

  // Her portraits while she is out play her mood's clip.
  setInterval(() => {
    if (!isOn() || document.hidden) return
    for (const sheet of document.querySelectorAll('.still-her .sheet[data-clip]')) {
      const clip = CLIPS[sheet.dataset.clip]
      if (clip) showFrame(sheet, clip.row, frameNow(clip))
    }
  }, 60)

  function showFrame(sheet, row, frame) {
    sheet.style.backgroundPosition = `${-frame * CELL_W}px ${-row * CELL_H}px`
  }

  // --- The shapes ---------------------------------------------------------------

  function fillCompact() {
    // Three places: her at the left (her portrait while she is out), a line
    // of words cut short in the middle, a short value at its right (the
    // clock, a widget's number), with "+N" for the other sessions before it;
    // then, pinned, the monitor's readings at the island's right end.
    const widget = shownWidget()
    let words, value, key
    if (widget && Widgets.partsOf(widget)) {
      words = el('span', 'label wparts')
      words.innerHTML = Widgets.partsHTML(widget, ink)
      value = el('span', 'clock')
      key = `w:${widget.id}`
    } else if (widget) {
      const said = Widgets.words(lang, widget, isDetailed())
      words = el('span', 'label wlabel')
      words.insertAdjacentHTML('afterbegin', Widgets.icon(widget.icon))
      words.firstChild.style.color = ink(widget.color) || hue().idle
      words.append(said.label)
      value = el('span', 'clock wvalue', said.value)
      value.style.color = ink(widget.color) || hue().idle
      key = `w:${widget.id}`
    } else {
      words = el('span', 'label', Status.brief(lang, now, { detailed: isDetailed() }))
      value = el('span', 'clock', time())
      value.style.color = hue()[now.mood]
      key = `s:${now.id || ''}:${now.mood}`
    }
    // In the bar and the taskbar the other sessions have their own tags.
    const more = now.others > 0 && !isStrip() ? el('span', 'more', `+${now.others}`) : null
    if (more) more.style.color = hue()[second] || hue().working
    // What takes turns, together: it slides in when it is something else.
    const turning = el('span', 'turning')
    turning.append(words, ...(more ? [more] : []), value)
    const isBare = !words.textContent && !value.textContent && !more
    const pinned = isPinned() && home() === 'island' ? el('span', 'pinned') : null
    if (pinned) pinned.innerHTML = Widgets.partsHTML(monitor, ink)
    compact.replaceChildren(...(isHome() ? [] : [stillHer(24)]), ...(isBare && pinned ? [] : [turning]), ...(pinned ? [pinned] : []))
    compact.classList.toggle('bare', isBare)
    // As wide as set, and the readings' width more when pinned (their own,
    // fixed); just her (and the readings) with nothing to say.
    compact.style.width = isBare ? '' : `${slotWidth() + (pinned ? pinned.offsetWidth + 12 : 0)}px`
    compactWidth = isBare ? Math.max(MIN_W, compact.offsetWidth) : parseFloat(compact.style.width)
    if (key !== shownKey) {
      if (shownKey && view === 'compact') turnedAt = Date.now()
      shownKey = key
    }
    const into = Date.now() - turnedAt
    if (into < TURN_MS) {
      turning.classList.add('turn')
      turning.style.animationDelay = `-${into}ms`
    }
  }

  // How long something new takes to slide into the compact island.
  const TURN_MS = 320
  const isPinned = () => !!monitor && config.monitor?.pin !== false

  // The compact island's width as set.
  const slotWidth = () => WIDTHS[config.islandWidth] || WIDTHS.normal

  // A widget's line in the open island: the one shown is lit.
  function widgetRow(widget, isShown) {
    const { label, value } = Widgets.words(lang, widget, isDetailed())
    const row = el('span', isShown ? 'wrow on' : 'wrow')
    row.dataset.widget = widget.id
    row.insertAdjacentHTML('afterbegin', Widgets.icon(widget.icon))
    row.firstChild.style.color = ink(widget.color) || hue().idle
    const valueText = el('span', 'wv', value)
    valueText.style.color = ink(widget.color) || hue().idle
    if (Widgets.partsOf(widget)) valueText.innerHTML = Widgets.partsHTML(widget, ink)
    row.append(el('span', 'wl', label), valueText)
    return row
  }

  // Whose it is (the session's name) with the clock, where it is, how the
  // turn ended in Claude's words, the to-do list's progress, and a line for
  // each other session. A hello (or something to fix) takes the name's place.
  function fillExpanded() {
    const detailed = isDetailed()
    const name = Status.nameOf(now, detailed)
    const title = (isNudging() && nudge.text) || (detailed && name) || t(lang, `mood.${now.mood}`)
    const sub = detailed
      ? [Status.status(lang, now), name !== now.project ? now.project : '']
      : [now.project, say(lang, now.detail)]
    const others = Status.othersOf(now.list, now)

    const head = el('span', 'head')
    const clockText = el('span', 'clock', time())
    clockText.style.color = hue()[now.mood]
    head.append(el('span', 'title', title), clockText)
    // The session shown, all of it one place to click.
    const me = el('span', 'me')
    canJump(me, now)
    me.append(head)
    // Not the title again (resting, with no name: both would say so).
    const subText = sub.filter(p => p && p !== title).join(' · ')
    if (subText) me.append(el('span', 'sub', subText))
    if (detailed && Status.isEnding(now) && now.reply) me.append(el('span', 'reply', now.reply))
    // How far down the to-do list, and the item it is on.
    if (detailed && now.todo?.total) {
      const progress = el('span', 'progress')
      const bar = el('span', 'bar')
      const fill = el('i')
      fill.style.cssText = `width:${Math.round((100 * now.todo.done) / now.todo.total)}%;background:${hue()[now.mood]}`
      bar.append(fill)
      progress.append(bar)
      if (now.todo.active) progress.append(el('span', 'item', now.todo.active))
      me.append(progress)
    }
    const lines = el('span', 'lines')
    lines.append(me)
    if (others.length) {
      const list = el('span', 'others')
      for (const x of others.slice(0, OTHERS_SHOWN)) {
        const row = el('span', 'other')
        canJump(row, x)
        const dot = el('i', 'dot')
        dot.style.background = hue()[x.mood] || hue().idle
        const what = detailed ? Status.status(lang, x, { withClock: false }) : t(lang, `island.${x.mood}`)
        const when = el('span', 'when', Status.time(x))
        when.style.color = hue()[x.mood] || hue().idle
        row.append(dot, el('span', 'who', Status.nameOf(x, detailed)), el('span', 'what', what), when)
        list.append(row)
      }
      if (others.length > OTHERS_SHOWN) list.append(el('span', 'other rest', t(lang, 'status.moreSessions', { n: others.length - OTHERS_SHOWN })))
      lines.append(list)
    }
    // The monitor, and the widgets, the one shown lit: under the sessions too,
    // a session at work not all the island says (2026-10-10).
    if ((widgets.length || monitor) && !(isNudging() && nudge.widget)) {
      const list = el('span', 'wlist')
      const shown = shownWidget()
      if (isPinned()) list.append(widgetRow(monitor, false))
      for (const w of widgets) list.append(widgetRow(w, w === shown))
      lines.append(list)
    }
    // A widget that asked to open the island: what it says, over all else.
    if (isNudging() && nudge.widget) {
      const w = nudge.widget
      const { label, value } = Widgets.words(lang, w, isDetailed())
      // A private one's words stay off screen too, while the specifics do.
      const words = w.private && !isDetailed() ? '' : w.words
      const card = el('span', 'me')
      const top = el('span', 'head')
      top.insertAdjacentHTML('afterbegin', Widgets.icon(w.icon))
      top.firstChild.style.color = ink(w.color) || hue().idle
      top.append(el('span', 'title', words || label || value))
      card.append(top, el('span', 'sub', words ? [label, value].filter(Boolean).join(' · ') : label ? value : ''))
      lines.replaceChildren(card)
    }
    expanded.replaceChildren(...(isHome() ? [] : [stillHer(48)]), lines)
  }

  // On to the next widget (or back), and its turn starts over.
  function turnWidget(step) {
    if (widgets.length < 2) return
    widgetAt = (widgetAt + step + widgets.length) % widgets.length
    clearTimeout(spinTimer)
    spinTimer = null
    update()
  }

  // Widgets take turns while the island is quiet and compact.
  function spin() {
    const sec = Number(config.widgetSpin ?? 8)
    if (!(isQuiet() && widgets.length > 1 && sec > 0 && view === 'compact')) {
      clearTimeout(spinTimer)
      spinTimer = null
      return
    }
    if (!spinTimer) {
      spinTimer = setTimeout(() => {
        spinTimer = null
        widgetAt = (widgetAt + 1) % widgets.length
        update()
      }, sec * 1000)
    }
  }

  // A session with a window to go to: clicking it there goes there.
  function canJump(node, session) {
    if (!session.jump || !session.id) return
    node.dataset.jump = session.id
    node.title = t(lang, 'jump.hint')
  }

  // To a session's window; when it has none any more, the island says so.
  function jumpTo(id) {
    window.pet.jump(id).then(went => {
      if (!went) nudgeFor(MOOD_CLIP[now.mood], t(lang, 'jump.notFound'), 2200)
    })
  }

  // --- Her, in the island -----------------------------------------------------------

  // Her shape for the island's: a round portrait ringed in the mood's colour
  // (small in the compact island, a little bigger in the settings' head), or
  // her whole self standing at the left. All the same sheet, cropped and
  // scaled, so one springs into another.
  function placeHer({ height }) {
    if (!isHome()) return
    const s = her.style
    const portrait = (px, scale, left, top) => {
      Object.assign(s, { left: `${left}px`, top: `${top}px`, width: `${px}px`, height: `${px}px`, borderRadius: `${px / 2}px` })
      s.setProperty('--ring', hue()[now.mood])
      herSheet.style.transform = `translate(${-(CELL_W * scale - px) / 2}px, -2px) scale(${scale})`
    }
    if (view === 'compact') {
      if (home() === 'taskbar') portrait(TASK_HEAD, TASK_HEAD * PER_PX, (taskCompactH() - TASK_HEAD) / 2, (taskCompactH() - TASK_HEAD) / 2)
      else portrait(HEAD, HEAD_SCALE, 6, 6)
    } else if (view === 'settings') {
      portrait(SET_HEAD, SET_HEAD_SCALE, 16, 13)
    } else {
      const top = height - BODY_H - (view === 'ask' ? 10 : 6)
      Object.assign(s, { left: '12px', top: `${top}px`, width: `${BODY_W}px`, height: `${BODY_H}px`, borderRadius: '0px' })
      herSheet.style.transform = `translate(0px, 0px) scale(${BODY_SCALE})`
    }
  }

  // Her mood's clip, in the portrait as standing; waiting by a prompt, a
  // reaction's while the island says something.
  function animateHer() {
    if (!isHome()) return
    const name = view === 'ask' ? 'waiting' : isNudging() ? nudge.clip : MOOD_CLIP[now.mood]
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

  // The window with no extra room, by home: the taskbar is as wide as the
  // screen, with its room above.
  const baseRoom = () => (home() === 'taskbar' ? { width: innerWidth, height: TASKBAR_H + TASKBAR_ROOM } : BASE)

  function roomFor(want) {
    // The taskbar: what grows up from it, or the settings standing above it,
    // past its room (the window then grows up too, which flashes: its room
    // is kept as high as the settings ever are, settingsMax).
    if (home() === 'taskbar') {
      const height = want.height + TASKBAR_SHADE + (view === 'settings' ? TASKBAR_H + TASKBAR_GAP : isCapsule() ? CAPSULE_Y : 0)
      if (height <= TASKBAR_H + TASKBAR_ROOM) return null
      return { width: innerWidth, height }
    }
    const width = want.width + 48
    const height = TOP + want.height + 24
    return width <= BASE.width && height <= BASE.height ? null : { width, height }
  }

  // Returns true when the window has to grow first.
  function setRoom(next) {
    const size = r => r || baseRoom()
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
    // The taskbar's left end is square, and what grows up from it is rounded
    // above (on its Mica, her capsule and all it grows into round).
    island.style.borderRadius =
      home() === 'taskbar'
        ? view === 'compact'
          ? isCapsule()
            ? `${height / 2}px`
            : '0'
          : view === 'settings' || isCapsule()
            ? '22px'
            : '22px 22px 0 0'
        : `${height > 60 ? 30 : height / 2}px`
  }

  // --- Deciding ---------------------------------------------------------------

  function sizeOf(name) {
    const withHer = isHome()
    if (name === 'settings') return { width: window.Settings.layer.offsetWidth || SETTINGS_W, height: window.Settings.layer.offsetHeight }
    if (name === 'ask') {
      return {
        width: panel.offsetWidth + 28 + (withHer ? 108 : 0),
        height: Math.max(panel.offsetHeight + 26, withHer ? BODY_H + 20 : 0),
      }
    }
    if (name === 'expanded') {
      // As tall as what it says, never shorter than her.
      return withHer
        ? { width: Math.min(OPEN_MAX_W, Math.max(320, expanded.offsetWidth)), height: Math.max(OPEN_H, expanded.offsetHeight) }
        : { width: Math.min(OPEN_BARE_MAX_W, Math.max(300, expanded.offsetWidth)), height: Math.max(84, expanded.offsetHeight) }
    }
    return { width: compactWidth, height: home() === 'taskbar' ? taskCompactH() : 36 }
  }

  // --- The taskbar's right part ---------------------------------------------------

  // The other sessions as tags (a press goes to one's window), a widget, and
  // the settings, with the Start button before them, the tray's icons and
  // the clock after. Its left edge stays where the compact island ends, so
  // what opens from the island grows up over it.
  const barRest = document.createElement('div')
  barRest.id = 'bar-rest'
  island.after(barRest)

  function sessionTags() {
    const detailed = isDetailed()
    // In the taskbar, a session whose window has a button needs no tag: the
    // button goes to it, and the island says how it is.
    const onButtons = new Set(home() === 'taskbar' ? taskWindows.flatMap(a => (a.windows || []).flatMap(w => (w.sessions || []).map(s => s.id))) : [])
    return Status.othersOf(now.list, now)
      .filter(x => !onButtons.has(x.id))
      .map(x => {
        const tag = el('span', 'tag')
        if (x.jump && x.id) {
          tag.dataset.jump = x.id
          tag.title = t(lang, 'jump.hint')
        }
        const dot = el('i', 'dot')
        dot.style.background = hue()[x.mood] || hue().idle
        const when = el('span', 'when', Status.time(x))
        when.style.color = hue()[x.mood] || hue().idle
        tag.append(dot, el('span', 'who', Status.nameOf(x, detailed)), el('span', 'what', Status.brief(lang, x, { detailed })), when)
        return tag
      })
  }

  // The widget shown, and the monitor, pinned: always there by the settings.
  // In the taskbar only the monitor (before the tray), in two rows as the
  // clock has its two, coloured as Windows' own taskbar would (style.css):
  // the widgets take their turns at its left end, in her place (fillCompact).
  function widgetFaces() {
    const faces = []
    const w = widgets.length && home() !== 'taskbar' ? widgets[widgetAt % widgets.length] : null
    if (w) {
      const { label, value } = Widgets.words(lang, w, isDetailed())
      const face = el('span', 'face')
      face.insertAdjacentHTML('afterbegin', Widgets.icon(w.icon))
      face.firstChild.style.color = ink(w.color) || hue().idle
      const v = el('span', 'v', value)
      v.style.color = ink(w.color) || hue().idle
      if (Widgets.partsOf(w)) v.innerHTML = Widgets.partsHTML(w, ink)
      face.append(Widgets.partsOf(w) ? '' : label, v)
      faces.push(face)
    }
    if (isPinned()) {
      const inRows = home() === 'taskbar'
      const readings = el('span', inRows ? 'face mon rows' : 'face mon')
      readings.innerHTML = inRows ? Widgets.rowsHTML(monitor) : Widgets.partsHTML(monitor, ink)
      faces.push(readings)
    }
    return faces
  }

  // The settings: Windows' own settings cog, from its icon font (style.css .fi).
  function gearNode() {
    const gear = el('span', 'gear')
    gear.dataset.bar = 'settings'
    gear.append(el('span', 'fi', ''))
    return gear
  }

  function fillBar() {
    if (home() === 'taskbar') fillTaskbar()
  }

  // The taskbar's parts that stay put: rebuilt every second with the
  // clocks, the tray's icons would lose the pointer and blink.
  const startButton = el('span', 'start')
  startButton.dataset.bar = 'start'
  // Windows 11's mark, drawn as Windows draws it (its own is drawn by its
  // taskbar's code: no file or icon font holds it), as measured on its own
  // taskbar: as big as the programs' icons (29 of the 30 screen pixels of
  // 24px at 125%), four panes in one blue running from light at the top left
  // to deep at the bottom right (style.css .s0, .s1), a one-pixel gap. On
  // the screen's own pixels, as Windows': the panes in whole device pixels
  // for the screen's scale, and the mark moved by the part of a pixel it
  // lands off one (snapStart), or the gap blurs.
  let startScale = 0
  function drawStartMark() {
    const dpr = devicePixelRatio || 1
    if (dpr === startScale) return
    startScale = dpr
    const box = Math.round(24 * dpr)
    const gap = Math.max(1, Math.round(0.8 * dpr))
    const pane = Math.floor((box - 1 - gap) / 2)
    const at = Math.floor((box - 2 * pane - gap) / 2)
    const far = at + pane + gap
    const end = far + pane
    const r = Math.max(0.5, 0.8 * dpr).toFixed(2)
    const rect = (x, y) => `<rect x="${x}" y="${y}" width="${pane}" height="${pane}" rx="${r}"/>`
    startButton.innerHTML = `<svg viewBox="0 0 ${box} ${box}" style="width:${box / dpr}px;height:${box / dpr}px"><defs><linearGradient id="start-blue" gradientUnits="userSpaceOnUse" x1="${at}" y1="${at}" x2="${end}" y2="${end}"><stop class="s0" offset="0"/><stop class="s1" offset="1"/></linearGradient></defs><g fill="url(#start-blue)">${rect(at, at)}${rect(far, at)}${rect(at, far)}${rect(far, far)}</g></svg>`
  }
  // Moved by its layout (position: relative), not by a transform: a moved
  // layer is drawn again off the grid, blurring it (2026-10-10, on the
  // live taskbar). The part of the taskbar it is in starts on the grid too
  // (onPixels in fillTaskbar).
  function snapStart() {
    const mark = startButton.firstElementChild
    if (!mark || !mark.isConnected || startButton.classList.contains('down')) return
    const dpr = devicePixelRatio || 1
    mark.style.left = '0px'
    mark.style.top = '0px'
    const r = mark.getBoundingClientRect()
    const off = v => (Math.round(v * dpr) - v * dpr) / dpr
    mark.style.left = `${off(r.left).toFixed(3)}px`
    mark.style.top = `${off(r.top).toFixed(3)}px`
  }
  // A length that starts and ends on the screen's own pixels.
  const onPixels = v => Math.round(v * (devicePixelRatio || 1)) / (devicePixelRatio || 1)
  drawStartMark()
  window.addEventListener('resize', () => {
    drawStartMark()
    snapStart()
  })
  // As Windows 11's: its mark pressed in under the pointer and springing
  // back; a bounce when Windows' Start menu opens by the Windows key, and
  // lit while it is open (taskbar.rs taskbar:start). Pressed just now, the
  // press was its bounce.
  let startPressedAt = 0
  startButton.addEventListener('pointerdown', e => {
    if (e.button !== 0) return
    startButton.classList.add('down')
    startPressedAt = Date.now()
  })
  for (const type of ['pointerup', 'pointerleave', 'pointercancel']) startButton.addEventListener(type, () => startButton.classList.remove('down'))
  startButton.addEventListener('animationend', () => startButton.classList.remove('bounce'))
  window.pet.onStart(({ open } = {}) => {
    startButton.classList.toggle('open', !!open)
    if (!open || Date.now() - startPressedAt < 800) return
    startButton.classList.remove('bounce')
    void startButton.offsetWidth
    startButton.classList.add('bounce')
  })
  const windowsBox = el('span', 'wins')
  const tagsBox = el('span', 'tags')
  const facesBox = el('span', 'faces')
  const trayBox = el('span', 'tray')
  // The keyboard: Caps Lock while it is on, and the input method's script
  // (a press switches it; a right press, the input methods to pick from).
  const keysBox = el('span', 'kbd')
  const capsMark = el('span', 'caps', 'A')
  const imeMark = el('span', 'ime')
  imeMark.dataset.bar = 'ime'
  keysBox.append(capsMark, imeMark)
  // Windows' quick settings (Wi-Fi, Bluetooth, volume…), as its own taskbar
  // has them: the network's way out (by cable, Wi-Fi, a VPN's, or none) and
  // the volume, in its own icon font (.fi: Segoe Fluent Icons).
  const quickBox = el('span', 'tquick')
  quickBox.dataset.bar = 'quick'
  const NET = { wired: '', wifi: '', other: '', none: '' }
  const netMark = el('span', 'fi net')
  // The volume as Windows draws it: the speaker's three waves faint, those up
  // to the volume solid over them; struck through, muted.
  const volMark = el('span', 'fi vol')
  const volBack = el('span', 'back', '')
  const volFront = el('span', 'front', '')
  volMark.append(volBack, volFront)
  quickBox.append(netMark, volMark)
  function showNet(kind) {
    netMark.textContent = NET[kind] || NET.other
  }
  showNet('other')
  function showVolume(v) {
    const [level, muted] = Array.isArray(v) ? v : [null, false]
    volFront.textContent = muted ? '' : level == null || level >= 67 ? '' : level >= 34 ? '' : level > 0 ? '' : ''
    volBack.hidden = !!muted
    volMark.title = level == null ? '' : muted ? t(lang, 'taskbar.muted') : t(lang, 'taskbar.volume', { n: level })
  }
  // What is using the microphone, the camera, the location, as Windows'
  // taskbar shows it before the input method; a press, its privacy settings.
  const PRIVACY = { mic: '', cam: '', loc: '' }
  const privBox = el('span', 'tpriv')
  let privDrawn = ''
  function showPrivacy(use) {
    const drawn = JSON.stringify(use || {})
    if (drawn === privDrawn) return
    privDrawn = drawn
    privBox.replaceChildren(
      ...Object.keys(PRIVACY)
        .filter(k => use?.[k]?.length)
        .map(k => {
          const mark = el('span', 'fi', PRIVACY[k])
          mark.dataset.bar = `privacy-${k}`
          mark.title = t(lang, `taskbar.inUse.${k}`, { apps: use[k].join(lang === 'zh' ? '、' : ', ') })
          return mark
        }),
    )
  }
  const clockBox = el('span', 'tclock')
  clockBox.dataset.bar = 'notifications'
  const taskGear = gearNode()
  // Show the desktop: a sliver at the screen's right edge, as Windows' own (Win+D).
  const deskEdge = el('span', 'tdesk')
  deskEdge.dataset.bar = 'desktop'
  // The room between the programs' buttons and what comes after them: what
  // centring them moves them into (alignApps).
  const growNode = el('span', 'grow')
  let taskbarBuilt = false
  // The tray's icons as last sent ({ key, png, tip, exe }), and as drawn.
  let trayIcons = []
  let trayDrawn = ''
  let trayRectsSent = ''
  // The windows' buttons as last sent (taskbar.rs), and as drawn.
  let taskWindows = []
  let windowsDrawn = ''

  function fillTaskbar() {
    barRest.style.left = `${onPixels(sizeOf('compact').width + (isCapsule() ? CAPSULE_X + 4 : 0))}px`
    if (!taskbarBuilt) {
      barRest.replaceChildren(glide, startButton, windowsBox, tagsBox, growNode, facesBox, trayBox, privBox, keysBox, quickBox, clockBox, taskGear, deskEdge, pop)
      taskbarBuilt = true
      trayDrawn = ''
      windowsDrawn = ''
    }
    // The programs' buttons as icons alone (taskbarButtons), or with their titles.
    barRest.classList.toggle('icons', config.taskbarButtons !== 'labels')
    startButton.title = t(lang, 'taskbar.start')
    quickBox.title = t(lang, 'taskbar.quick')
    clockBox.title = t(lang, 'taskbar.clock')
    deskEdge.title = t(lang, 'taskbar.desktop')
    tagsBox.replaceChildren(...sessionTags())
    facesBox.replaceChildren(...widgetFaces())
    drawWindows()
    drawTray()
    tickClock()
    alignApps()
    snapStart()
  }

  // Start and the programs' buttons in the middle of the screen, as Windows
  // 11 has them (taskbarAlign: center; left, after her end): moved along by
  // the Start button's margin, never back past where they start nor into
  // what comes after them (the room before the monitor and the tray is all
  // they may take). Moved, they slide; what hangs over a button (its
  // windows' list) is placed again once they are there.
  function alignApps() {
    if (!taskbarBuilt) return
    // Measured by the margin as it is now (it may be sliding), set to the one wanted.
    const target = parseFloat(startButton.style.marginLeft) || 0
    let want = 0
    if (config.taskbarAlign !== 'left') {
      const now = parseFloat(getComputedStyle(startButton).marginLeft) || 0
      const start = startButton.getBoundingClientRect()
      const width = windowsBox.getBoundingClientRect().right - start.left
      const room = growNode.getBoundingClientRect().width + now
      want = Math.round(Math.max(0, Math.min((innerWidth - width) / 2 - (start.left - now), room)))
    }
    if (want !== Math.round(target)) startButton.style.marginLeft = `${want}px`
  }
  startButton.addEventListener('transitionend', e => {
    if (e.propertyName !== 'margin-left') return
    if (popFor) drawPop()
    if (glideOn?.isConnected) glideTo(glideOn, true)
    snapStart()
    requestAnimationFrame(sendTrayRects)
  })

  // --- The pointer's glide --------------------------------------------------------

  // Under the pointer, one highlight for all the taskbar's buttons that
  // slides from one to the next as her island's drop moves: its leading
  // edge first, the trailing one after, so it stretches and gathers, a
  // little squashed on the way (style.css .hdrop). It comes and goes in
  // place, jumps rather than slides a long way, and gives a little under a
  // press. Not over what opens above the buttons, nor while a tray icon or
  // a program's button is being dragged.
  const GLIDE_ON = '.start, .win, .tout .ticon, .tchev, .tpriv, .kbd .ime, .tquick, .tclock, .gear, .tag[data-jump]'
  const GLIDE_FAR = 360
  const glide = el('span', 'hdrop')
  let glideOn = null
  let glideTimer

  function glideTo(node, still = false) {
    clearTimeout(glideTimer)
    const bar = barRest.getBoundingClientRect()
    const r = node.getBoundingClientRect()
    const left = r.left - bar.left
    const right = bar.right - r.right
    const was = glideOn && glide.classList.contains('on') ? parseFloat(glide.style.left) : null
    glide.style.top = `${r.top - bar.top}px`
    glide.style.height = `${r.height}px`
    if (still || was === null || Math.abs(left - was) > GLIDE_FAR) {
      glide.style.transition = 'none'
      glide.style.left = `${left}px`
      glide.style.right = `${right}px`
      void glide.offsetWidth
      glide.style.transition = ''
    } else if (left !== was) {
      const lead = left > was ? 'right' : 'left'
      const trail = lead === 'right' ? 'left' : 'right'
      glide.style.transition = `${lead} 0.22s cubic-bezier(0.3, 1.3, 0.5, 1), ${trail} 0.42s cubic-bezier(0.3, 1.45, 0.5, 1), opacity 0.16s ease, transform 0.3s cubic-bezier(0.3, 1.45, 0.5, 1)`
      glide.style.left = `${left}px`
      glide.style.right = `${right}px`
      glide.classList.remove('squash')
      void glide.offsetWidth
      glide.classList.add('squash')
    }
    glide.classList.add('on')
    glideOn = node
  }

  function glideOff() {
    clearTimeout(glideTimer)
    glideTimer = setTimeout(() => {
      glide.classList.remove('on', 'down')
      glideOn = null
    }, 90)
  }

  const isGlideOn = node => node?.matches?.(GLIDE_ON) && !node.closest('.wpop, .tfly')
  barRest.addEventListener(
    'mouseenter',
    e => {
      if (!isOn() || home() !== 'taskbar' || !isGlideOn(e.target) || body.classList.contains('tray-dragging') || body.classList.contains('win-dragging')) return
      glideTo(e.target)
    },
    true,
  )
  barRest.addEventListener(
    'mouseleave',
    e => {
      if (e.target === barRest || isGlideOn(e.target)) glideOff()
    },
    true,
  )
  barRest.addEventListener('pointerdown', e => {
    if (e.button === 0 && glideOn && e.target.closest(GLIDE_ON) === glideOn) glide.classList.add('down')
  })
  window.addEventListener('pointerup', () => glide.classList.remove('down'))
  window.addEventListener('resize', () => isOn() && home() === 'taskbar' && alignApps())

  // A button for each program (taskbar.rs): those kept on the taskbar
  // first, then the others as their windows came. Its icon; the title of its
  // one window, or its name and how many; none while it does not run (its
  // name then under the pointer). Icons alone (style.css #bar-rest.icons):
  // no title, no count, its windows listed when the pointer rests on it. A
  // line under it, longer for the one in front; lit while a window of it
  // flashes for attention. No marks for the sessions in its windows: the
  // island says how they are.
  // The programs' buttons in the order dragged (taskbarOrder, their keys,
  // those not running kept), the rest after, as they came.
  const appOrder = () => (Array.isArray(config.taskbarOrder) ? config.taskbarOrder.map(String) : [])
  function orderedApps() {
    const order = appOrder()
    const rank = a => (order.includes(a.app) ? order.indexOf(a.app) : Infinity)
    return taskWindows
      .map((a, n) => [a, n])
      .sort((x, y) => rank(x[0]) - rank(y[0]) || x[1] - y[1])
      .map(([a]) => a)
  }

  function drawWindows() {
    const apps = orderedApps()
    const drawn = JSON.stringify([apps, lang])
    if (drawn === windowsDrawn) return
    windowsDrawn = drawn
    windowsBox.replaceChildren(
      ...apps.map(app => {
        const windows = app.windows || []
        const button = el('span', 'win')
        button.dataset.app = app.app
        button.classList.toggle('front', windows.some(w => w.front))
        button.classList.toggle('min', windows.length > 0 && windows.every(w => w.min))
        button.classList.toggle('flash', windows.some(w => w.flash))
        button.classList.toggle('idle', !windows.length)
        if (!windows.length) button.title = app.name || ''
        if (app.png) {
          const img = el('img')
          img.src = app.png
          img.draggable = false
          button.append(img)
        } else {
          button.append(el('span', 'letter', (app.name || '?').slice(0, 1).toUpperCase()))
        }
        if (windows.length) button.append(el('span', 'wt', windows.length === 1 ? windows[0].title : app.name))
        if (windows.length > 1) button.append(el('span', 'count', String(windows.length)))
        return button
      }),
    )
    if (popFor) drawPop()
  }

  // Above a program's button: its windows to pick from (on a press with
  // several, or the pointer resting on it), or what can be done with it (a
  // right press): open another, keep it on the taskbar or no longer, close.
  const pop = el('div', 'wpop')
  pop.hidden = true
  barRest.append(pop)
  // { app, kind: 'list' | 'menu' } while open, and what it showed last.
  let popFor = null
  let popDrawn = ''
  let popTimer
  const appOf = key => taskWindows.find(a => a.app === key)

  function openPop(key, kind) {
    clearTimeout(popTimer)
    popFor = { app: key, kind }
    drawPop()
    watchPresses()
  }

  function closePop() {
    clearTimeout(popMoveTimer)
    popMoving = false
    popShown = null
    pop.getAnimations().forEach(a => a.id === 'slide' && a.cancel())
    pop.classList.remove('sliding', 'rising')
    pop.style.width = ''
    pop.style.height = ''
    clearTimeout(popTimer)
    popFor = null
    pop.hidden = true
    sendThumbs()
    watchPresses()
  }

  // While the windows' list (or menu) or the folded tray icons are open, a
  // press anywhere else closes them, as Windows' do: told by the host
  // (taskbar.rs press hook), for ours never take the focus whose loss would.
  let watching = false
  function watchPresses() {
    const want = !!popFor || trayOpen
    if (want === watching) return
    watching = want
    window.pet.taskbar.watch(want)
  }
  window.pet.onPress(at => {
    if (!isOn() || home() !== 'taskbar' || !at) return
    const over = document.elementFromPoint(at.x, at.y)
    if (popFor && !(over && (pop.contains(over) || windowsBox.contains(over)))) closePop()
    if (trayOpen && !(over && trayBox.contains(over))) openTray(false)
  })

  function drawPop() {
    const app = popFor && appOf(popFor.app)
    const button = app && windowsBox.querySelector(`[data-app="${CSS.escape(app.app)}"]`)
    if (!app || !button) return closePop()
    const windows = app.windows || []
    const rows = []
    if (popFor.kind === 'list') {
      // A card for each: its icon, title and ✕, and room under them for
      // its live picture (taskbar.rs thumbs).
      for (const w of windows) {
        const card = el('div', w.front ? 'wcard front' : 'wcard')
        card.dataset.win = w.id
        const top = el('div', 'wtop')
        if (w.png || app.png) {
          const img = el('img')
          img.src = w.png || app.png
          top.append(img)
        }
        top.append(el('span', 'wt', w.title))
        const x = el('span', 'wx', '✕')
        x.dataset.close = w.id
        top.append(x)
        const room = el('div', 'wthumb')
        room.dataset.thumb = w.id
        card.append(top, room)
        rows.push(card)
      }
    } else {
      const item = (text, act) => {
        const row = el('div', 'witem', text)
        row.dataset.act = act
        return row
      }
      const head = el('div', 'whead', app.name)
      rows.push(head, item(t(lang, windows.length ? 'taskbar.newWindow' : 'taskbar.open'), 'launch'), item(t(lang, app.pinned ? 'taskbar.unpin' : 'taskbar.pin'), app.pinned ? 'unpin' : 'pin'))
      if (windows.length) rows.push(item(t(lang, windows.length > 1 ? 'taskbar.closeAll' : 'taskbar.close'), 'close'))
    }
    if (!rows.length) return closePop()
    // Another program's windows taking the place of the last's (the pointer
    // gone along the buttons): where the list was and how big, to slide from.
    const isList = popFor.kind === 'list'
    const from = isList && !pop.hidden && pop.classList.contains('list') && popShown && popShown !== app.app ? { left: pop.offsetLeft, width: pop.offsetWidth, height: pop.offsetHeight } : null
    const opening = pop.hidden
    // Drawn again only when what it shows changed: a card replaced under a
    // press is never clicked (the windows' list comes again often).
    const drawn = JSON.stringify([popFor, app.name, app.pinned, windows.map(w => [w.id, w.title, w.front, (w.png || '').length]), lang])
    if (drawn !== popDrawn || pop.hidden) {
      popDrawn = drawn
      pop.replaceChildren(...rows)
    }
    pop.classList.toggle('menu', popFor.kind === 'menu')
    pop.classList.toggle('list', isList)
    pop.hidden = false
    popShown = app.app
    // Mid-slide, the list keeps going where it was going.
    if (popMoving && !from) return
    // Over its button, kept within the screen, at its own size.
    pop.classList.remove('sliding', 'rising')
    pop.style.width = ''
    pop.style.height = ''
    const left = button.offsetLeft + button.offsetWidth / 2 - pop.offsetWidth / 2
    const to = { left: Math.max(4, Math.min(left, barRest.offsetWidth - pop.offsetWidth - 4)), width: pop.offsetWidth, height: pop.offsetHeight }
    if (from) return slidePop(from, to)
    pop.style.left = `${to.left}px`
    if (opening && isList) return risePop()
    requestAnimationFrame(sendThumbs)
  }

  // The list moving from one program's button to the next as Windows' own
  // does: sliding over and taking its new size, the new cards coming in from
  // the side it moves to. Windows draws the live pictures, which cannot
  // slide with it: they go for the slide and come back once it is there.
  // Opening, it rises a little into place, the pictures after it.
  const POP_SLIDE_MS = 300
  let popShown = null
  let popMoving = false
  let popMoveTimer

  function holdThumbs() {
    popMoving = true
    clearTimeout(popMoveTimer)
    thumbsSent = '[]'
    window.pet.taskbar.thumbs([])
  }

  function settlePop(ms) {
    popMoveTimer = setTimeout(() => {
      popMoving = false
      // A slide not quite done by now would hold the list at its old size.
      pop.getAnimations().forEach(a => a.id === 'slide' && a.cancel())
      pop.classList.remove('sliding', 'rising')
      pop.style.width = ''
      pop.style.height = ''
      sendThumbs()
    }, ms)
  }

  function slidePop(from, to) {
    holdThumbs()
    pop.style.setProperty('--come', `${to.left > from.left ? 16 : -16}px`)
    pop.classList.add('sliding')
    const box = b => ({ left: `${b.left}px`, width: `${b.width}px`, height: `${b.height}px` })
    Object.assign(pop.style, box(to))
    pop.getAnimations().forEach(a => a.id === 'slide' && a.cancel())
    const slide = pop.animate([box(from), box(to)], { duration: POP_SLIDE_MS, easing: 'cubic-bezier(0.3, 1.2, 0.5, 1)' })
    slide.id = 'slide'
    settlePop(POP_SLIDE_MS)
  }

  function risePop() {
    holdThumbs()
    pop.classList.add('rising')
    settlePop(180)
  }

  // The live pictures where the cards left room for them; none once closed.
  let thumbsSent = ''
  function sendThumbs() {
    // Not while the list slides or rises: they come once it is in place.
    if (popMoving) return
    const items = popFor?.kind === 'list' ? [...pop.querySelectorAll('[data-thumb]')].map(r => {
      const b = r.getBoundingClientRect()
      return [Number(r.dataset.thumb), b.left, b.top, b.width, b.height]
    }) : []
    const sent = JSON.stringify(items)
    if (sent === thumbsSent) return
    thumbsSent = sent
    window.pet.taskbar.thumbs(items)
  }
  window.addEventListener('resize', () => {
    thumbsSent = ''
    requestAnimationFrame(sendThumbs)
  })

  // A press on a program's button: one not running starts; one window goes
  // to the front, or is minimized when in front; several to pick from.
  windowsBox.addEventListener('click', e => {
    const app = appOf(e.target.closest('[data-app]')?.dataset.app)
    if (!app) return
    e.stopPropagation()
    // The end of a drag, not a press.
    if (winDragged) {
      winDragged = false
      return
    }
    const windows = app.windows || []
    if (!windows.length) return window.pet.taskbar.app(app.path, 'launch', app.id)
    closePop()
    if (windows.length === 1) return window.pet.taskbar.window(windows[0].id, 'press')
    // Several: all to the front, or all minimized when one is in front (the
    // list of them comes with the pointer resting on the button).
    window.pet.taskbar.windows(windows.map(w => w.id))
  })
  windowsBox.addEventListener('contextmenu', e => {
    const app = appOf(e.target.closest('[data-app]')?.dataset.app)
    if (!app) return
    e.preventDefault()
    e.stopPropagation()
    openPop(app.app, 'menu')
  })
  // A program's button dragged along the taskbar: put where it is let go
  // among the others, before the one under the pointer (after it past its
  // middle), the order kept (taskbarOrder). { app, x, y, ghost } while held.
  let winPress = null
  let winDragged = false
  windowsBox.addEventListener('pointerdown', e => {
    const key = e.target.closest('[data-app]')?.dataset.app
    if (e.button !== 0 || !key) return
    winPress = { app: key, x: e.clientX, y: e.clientY, ghost: null }
  })
  windowsBox.addEventListener('pointermove', e => {
    if (!winPress) return
    // Let go of off the buttons before a drag began: nothing held now.
    if (!(e.buttons & 1)) return cancelWinDrag()
    if (!winPress.ghost && Math.hypot(e.clientX - winPress.x, e.clientY - winPress.y) > 6) {
      // Held from here on, wherever the pointer goes; only now, for a
      // captured press clicks on the box, not on its button.
      try {
        windowsBox.setPointerCapture(e.pointerId)
      } catch {}
      const from = windowsBox.querySelector(`[data-app="${CSS.escape(winPress.app)}"] img`)
      winPress.ghost = el('img', 'tghost')
      if (from) winPress.ghost.src = from.src
      body.append(winPress.ghost)
      body.classList.add('win-dragging')
      clearTimeout(popTimer)
      closePop()
    }
    if (winPress.ghost) {
      Object.assign(winPress.ghost.style, { left: `${e.clientX - 10}px`, top: `${e.clientY - 10}px` })
      markWinDrop(winDropAt(e.clientX, e.clientY, winPress.app))
    }
  })
  windowsBox.addEventListener('pointerup', e => {
    const press = winPress
    winPress = null
    if (!press?.ghost) return
    press.ghost.remove()
    body.classList.remove('win-dragging')
    markWinDrop(null)
    // The click that follows the release is the drag's.
    winDragged = true
    setTimeout(() => (winDragged = false), 0)
    const drop = winDropAt(e.clientX, e.clientY, press.app)
    if (!drop) return
    const order = appOrder()
    for (const a of orderedApps()) if (!order.includes(a.app)) order.push(a.app)
    order.splice(order.indexOf(press.app), 1)
    order.splice(order.indexOf(drop.target) + (drop.after ? 1 : 0), 0, press.app)
    window.pet.settings.set({ taskbarOrder: order.slice(0, 200) })
  })
  // Cancelled, or the pointer let go of before the button came up (the
  // window letting the mouse through once it left the taskbar): no drop.
  const cancelWinDrag = () => {
    winPress?.ghost?.remove()
    winPress = null
    body.classList.remove('win-dragging')
    markWinDrop(null)
  }
  windowsBox.addEventListener('pointercancel', cancelWinDrag)
  windowsBox.addEventListener('lostpointercapture', () => winPress && cancelWinDrag())

  // Where a dragged button would land at (x, y): beside the button whose
  // span holds x (past the last, after it); null when let go off the
  // taskbar, or on itself.
  function winDropAt(x, y, key) {
    const strip = windowsBox.getBoundingClientRect()
    if (y < strip.top - 24 || y > strip.bottom + 24) return null
    const buttons = [...windowsBox.querySelectorAll('[data-app]')]
    const self = buttons.find(b => b.dataset.app === key)
    const r0 = self?.getBoundingClientRect()
    if (r0 && x >= r0.left && x <= r0.right) return null
    const others = buttons.filter(b => b !== self)
    if (!others.length) return null
    for (const b of others) {
      const r = b.getBoundingClientRect()
      if (x < r.right) return { target: b.dataset.app, after: x > r.left + r.width / 2, slot: b }
    }
    const last = others[others.length - 1]
    return { target: last.dataset.app, after: true, slot: last }
  }

  let winMarked = null
  function markWinDrop(drop) {
    if (winMarked) winMarked.classList.remove('drop-before', 'drop-after')
    winMarked = drop?.slot || null
    if (winMarked) winMarked.classList.add(drop.after ? 'drop-after' : 'drop-before')
  }

  // The pointer resting on a running program's button: its windows.
  windowsBox.addEventListener('mouseover', e => {
    const app = appOf(e.target.closest('[data-app]')?.dataset.app)
    clearTimeout(popTimer)
    if (winPress?.ghost) return
    if (!app || popFor?.kind === 'menu') return
    if (popFor?.app === app.app) return
    popTimer = setTimeout(() => (app.windows || []).length && openPop(app.app, 'list'), popFor ? 0 : 500)
  })
  // Gone from the button and the list a moment: it closes.
  for (const node of [windowsBox, pop]) {
    node.addEventListener('mouseleave', () => {
      clearTimeout(popTimer)
      popTimer = setTimeout(closePop, 400)
    })
    node.addEventListener('mouseenter', () => popFor && clearTimeout(popTimer))
  }
  // On the press, not the click: what was pressed may be drawn anew before
  // the button comes up.
  pop.addEventListener('click', e => e.stopPropagation())
  pop.addEventListener('pointerdown', e => {
    if (e.button !== 0) return
    e.stopPropagation()
    const app = popFor && appOf(popFor.app)
    if (!app) return
    const close = e.target.closest('[data-close]')
    if (close) return window.pet.taskbar.window(close.dataset.close, 'close')
    const row = e.target.closest('[data-win]')
    if (row) {
      closePop()
      return window.pet.taskbar.window(row.dataset.win, 'press')
    }
    const act = e.target.closest('[data-act]')?.dataset.act
    if (!act) return
    closePop()
    if (act === 'close') for (const w of app.windows || []) window.pet.taskbar.window(w.id, 'close')
    else window.pet.taskbar.app(app.path, act, app.id)
  })
  pop.addEventListener('contextmenu', e => {
    e.preventDefault()
    e.stopPropagation()
  })

  // The script's mark by the layout's language: its own and plain letters.
  const SCRIPTS = { 0x04: ['中', '英'], 0x11: ['あ', 'A'], 0x12: ['한', 'A'] }
  const LANGS = { 0x09: 'EN', 0x07: 'DE', 0x0c: 'FR', 0x0a: 'ES', 0x10: 'IT', 0x16: 'PT', 0x19: 'RU', 0x13: 'NL', 0x1f: 'TR' }
  window.pet.onKeys(keys => {
    const primary = (keys?.lang || 0) & 0x3ff
    const script = SCRIPTS[primary]
    imeMark.textContent = script ? script[keys.native === false ? 1 : 0] : LANGS[primary] || primary.toString(16).toUpperCase()
    imeMark.classList.toggle('unknown', !!script && keys.native == null)
    capsMark.classList.toggle('on', !!keys?.caps)
    capsMark.title = keys?.caps ? 'Caps Lock' : ''
    showNet(keys?.net)
    showPrivacy(keys?.use)
    showVolume(keys?.volume)
  })

  // --- The taskbar's look: Mica, light or dark, Windows' accent -------------------------

  // Behind the strip's parts: the desktop's picture under it, blurred and
  // tinted (mica.js) for Windows' mode, as taskbar.rs sent it last; drawn
  // again only when the picture, the mode or the strip's width changes. None
  // on a solid strip. On a clear one (tb-clear) it is drawn too, unseen, if
  // the strip is to fill in as Mica (tb-clear-mica): it fades in at once
  // while a window is maximized or full screen on its display (tb-filled,
  // drawn again as the windows change); filled in solid, tb-solid. The
  // strip light in Windows' light mode (tb-light), her end a capsule
  // wherever the strip is not black (tb-capsule).
  const micaBox = el('div')
  micaBox.id = 'mica'
  island.parentElement.prepend(micaBox)
  const mica = Mica.mount(micaBox)
  let paper = null

  function drawMica() {
    const clearMica = isClear() && clearWhen() === 'mica'
    body.classList.toggle('tb-mica', isMica())
    body.classList.toggle('tb-light', isLight())
    body.classList.toggle('tb-capsule', isCapsule())
    body.classList.toggle('tb-clear', isClear())
    body.classList.toggle('tb-clear-mica', clearMica)
    body.classList.toggle('tb-filled', isFilled())
    body.classList.toggle('tb-solid', isFilled() && clearWhen() === 'solid')
    if (isMica() || clearMica) mica.set(paper, innerWidth, TASKBAR_H, look.mode)
    else mica.clear()
  }

  window.pet.onWallpaper(p => {
    paper = p
    drawMica()
  })

  // Windows' mode and accent colour (taskbar.rs). The accent as its own
  // taskbar draws with it: a lighter one on the dark strip, a darker one on
  // the light (style.css --win-accent, for the monitor's icons and the line
  // under the window in front; its own blue until it comes). Not --accent:
  // that is her mood's colour (pet.js).
  window.pet.onLook(l => {
    look = { mode: l?.mode === 'light' ? 'light' : 'dark', accent: l?.accent || null }
    const colour = look.accent && (look.mode === 'light' ? look.accent.dark : look.accent.light)
    if (colour) document.documentElement.style.setProperty('--win-accent', colour)
    else document.documentElement.style.removeProperty('--win-accent')
    if (isOn() && home() === 'taskbar') {
      drawMica()
      update()
    }
  })
  window.addEventListener('resize', drawMica)

  window.pet.onWindows(list => {
    taskWindows = Array.isArray(list) ? list : []
    if (isOn() && home() === 'taskbar') {
      drawWindows()
      tagsBox.replaceChildren(...sessionTags())
      alignApps()
      snapStart()
      drawMica()
    }
  })

  // The time and the date, as Windows' taskbar has them.
  function tickClock() {
    const d = new Date()
    const time = d.toLocaleTimeString(lang === 'zh' ? 'zh-CN' : undefined, { hour: '2-digit', minute: '2-digit' })
    const date = d.toLocaleDateString(lang === 'zh' ? 'zh-CN' : undefined, { year: 'numeric', month: 'numeric', day: 'numeric' })
    if (clockBox.dataset.at !== time + date) {
      clockBox.dataset.at = time + date
      clockBox.replaceChildren(el('b', '', time), el('i', '', date))
    }
  }
  setInterval(() => isOn() && home() === 'taskbar' && tickClock(), 5000)

  // The tray, as Windows' has it: the icons kept out on the taskbar, and
  // the rest folded away behind ^, open above it on a press. Where each is
  // kept: the person's choice (dragged from one to the other, trayPinned),
  // else Windows' own (windowsOut). In what order: as dragged (trayOrder,
  // names, those not running kept too), the rest after, as they came.
  const isOut = i => config.trayPinned?.[i.name] ?? i.windowsOut === true
  const trayOrder = () => (Array.isArray(config.trayOrder) ? config.trayOrder.map(String) : [])
  function orderedTray() {
    const order = trayOrder()
    const rank = i => (order.includes(i.name) ? order.indexOf(i.name) : Infinity)
    return trayIcons
      .map((i, n) => [i, n])
      .sort((a, b) => rank(a[0]) - rank(b[0]) || a[1] - b[1])
      .map(([i]) => i)
  }
  const chevron = el('span', 'tchev')
  chevron.append(el('span', 'fi', ''))
  const outBox = el('span', 'tout')
  const flyout = el('span', 'tfly')
  let trayOpen = false

  function trayIcon(i) {
    const slot = el('span', 'ticon')
    slot.dataset.tray = i.key
    slot.dataset.name = i.name
    slot.title = i.tip || i.exe
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

  function drawTray() {
    const icons = orderedTray()
    const folded = icons.filter(i => !isOut(i))
    if (!folded.length) trayOpen = false
    const drawn = JSON.stringify([icons.map(i => [i.key, i.png.length, i.png.slice(-24), i.tip, isOut(i)]), trayOpen])
    if (drawn !== trayDrawn) {
      trayDrawn = drawn
      chevron.hidden = !folded.length
      chevron.classList.toggle('open', trayOpen)
      outBox.replaceChildren(...icons.filter(isOut).map(trayIcon))
      flyout.replaceChildren(...folded.map(trayIcon))
      flyout.hidden = !trayOpen
      if (trayBox.firstChild !== flyout) trayBox.replaceChildren(flyout, chevron, outBox)
    }
    requestAnimationFrame(sendTrayRects)
  }

  function openTray(open) {
    if (trayOpen === open) return
    trayOpen = open
    drawTray()
    watchPresses()
  }
  chevron.addEventListener('click', () => openTray(!trayOpen))
  // Folded away again once the pointer has been gone from it a moment.
  let trayCloseTimer
  trayBox.addEventListener('mouseleave', () => {
    clearTimeout(trayCloseTimer)
    trayCloseTimer = setTimeout(() => openTray(false), 1200)
  })
  trayBox.addEventListener('mouseenter', () => clearTimeout(trayCloseTimer))

  // Where each icon is, for the programs that ask (taskbar.rs): a folded
  // one, while folded, is where ^ is.
  function sendTrayRects() {
    if (home() !== 'taskbar') return
    const at = node => {
      const r = node.getBoundingClientRect()
      return [r.left, r.top, r.width, r.height]
    }
    const rects = trayIcons.map(i => {
      const shown = trayBox.querySelector(`[data-tray="${i.key}"]`)
      const visible = shown && (isOut(i) || trayOpen)
      return [Number(i.key), ...at(visible ? shown : chevron)]
    })
    const sent = JSON.stringify(rects)
    if (sent === trayRectsSent) return
    trayRectsSent = sent
    window.pet.taskbar.trayRects(rects)
  }
  window.addEventListener('resize', () => {
    trayRectsSent = ''
    requestAnimationFrame(sendTrayRects)
  })

  window.pet.onTray(list => {
    trayIcons = Array.isArray(list) ? list : []
    if (isOn() && home() === 'taskbar') drawTray()
  })

  // A press on a tray icon goes to its program, as Windows' taskbar tells it:
  // the second press of a double one as a double press; the pointer coming,
  // moving (at most every 100 ms) and going. A left press is told once it
  // is let go, for it may be a drag instead: an icon dragged onto ^ or into
  // the folded ones is folded away, one dragged onto the taskbar kept out,
  // either put where it is let go among the others there.
  const trayKey = e => e.target.closest('[data-tray]')?.dataset.tray
  let trayOver = null
  let trayMovedAt = 0
  // A left press on an icon not yet let go: { key, name, x, y, ghost }.
  let trayPress = null
  trayBox.addEventListener('pointerdown', e => {
    if (e.button === 0 && trayKey(e)) {
      try {
        trayBox.setPointerCapture(e.pointerId)
      } catch {}
    }
  })
  trayBox.addEventListener('mousedown', e => {
    const key = trayKey(e)
    if (!key) return
    e.stopPropagation()
    if (e.button === 2) return window.pet.taskbar.tray(key, 'rightDown')
    if (e.button !== 0) return
    if (e.detail === 2) {
      trayPress = null
      return window.pet.taskbar.tray(key, 'double')
    }
    trayPress = { key, name: e.target.closest('[data-tray]').dataset.name, x: e.clientX, y: e.clientY, ghost: null }
  })
  trayBox.addEventListener('mouseup', e => {
    const key = trayKey(e)
    if (e.button === 2 && key) {
      e.stopPropagation()
      return window.pet.taskbar.tray(key, 'rightUp')
    }
    if (e.button !== 0 || !trayPress) return
    e.stopPropagation()
    const press = trayPress
    trayPress = null
    if (!press.ghost) {
      window.pet.taskbar.tray(press.key, 'leftDown')
      window.pet.taskbar.tray(press.key, 'leftUp')
      if (flyout.contains(e.target)) openTray(false)
      return
    }
    press.ghost.remove()
    body.classList.remove('tray-dragging')
    markDrop(null)
    const icon = trayIcons.find(i => String(i.key) === press.key)
    const drop = dropAt(e.clientX, e.clientY, press.name)
    if (!icon || !drop) return
    // Kept out or folded, and where among the others: before the icon
    // under the pointer (after it past its middle), else last there.
    const order = trayOrder()
    for (const i of orderedTray()) if (!order.includes(i.name)) order.push(i.name)
    order.splice(order.indexOf(press.name), 1)
    let at
    if (drop.target) {
      at = order.indexOf(drop.target) + (drop.after ? 1 : 0)
    } else {
      const there = orderedTray().filter(i => i.name !== press.name && isOut(i) === drop.out)
      at = there.length ? order.indexOf(there[there.length - 1].name) + 1 : order.length
    }
    order.splice(at, 0, press.name)
    window.pet.taskbar.trayPin(press.name, isOut(icon) === drop.out ? null : drop.out, order.slice(0, 200))
  })

  // Where a dragged icon would land at (x, y): kept out (anywhere on the
  // taskbar) or folded (the folded ones, ^), and the icon there it goes
  // beside (none: last); null, nowhere.
  function dropAt(x, y, name) {
    const over = document.elementFromPoint(x, y)
    if (!over) return null
    const out = !(flyout.contains(over) || chevron.contains(over))
    if (out && !barRest.contains(over)) return null
    const slot = over.closest('[data-tray]')
    // Let go on itself: left where it was.
    if (slot?.dataset.name === name) return null
    const mine = slot && (out ? outBox : flyout).contains(slot) ? slot : null
    if (!mine) return { out, target: null, after: false, slot: null }
    const r = mine.getBoundingClientRect()
    return { out, target: mine.dataset.name, after: x > r.left + r.width / 2, slot: mine }
  }

  // While dragging: a bar on the side of the icon it would land beside.
  let dropMarked = null
  function markDrop(drop) {
    if (dropMarked) dropMarked.classList.remove('drop-before', 'drop-after')
    dropMarked = drop?.slot || null
    if (dropMarked) dropMarked.classList.add(drop.after ? 'drop-after' : 'drop-before')
  }
  window.addEventListener('mousemove', e => {
    if (!trayPress) return
    if (!trayPress.ghost && Math.hypot(e.clientX - trayPress.x, e.clientY - trayPress.y) > 6) {
      const from = trayBox.querySelector(`[data-tray="${trayPress.key}"] img`)
      trayPress.ghost = el('img', 'tghost')
      if (from) trayPress.ghost.src = from.src
      body.append(trayPress.ghost)
      body.classList.add('tray-dragging')
      // Dragged from the taskbar: the folded ones open to take it.
      openTray(true)
    }
    if (trayPress.ghost) {
      Object.assign(trayPress.ghost.style, { left: `${e.clientX - 10}px`, top: `${e.clientY - 10}px` })
      markDrop(dropAt(e.clientX, e.clientY, trayPress.name))
    }
  })
  trayBox.addEventListener('mousemove', e => {
    const key = trayKey(e) || null
    if (key !== trayOver) {
      if (trayOver) window.pet.taskbar.tray(trayOver, 'out')
      if (key) window.pet.taskbar.tray(key, 'in')
      trayOver = key
    }
    if (key && Date.now() - trayMovedAt > 100) {
      trayMovedAt = Date.now()
      window.pet.taskbar.tray(key, 'move')
    }
  })
  trayBox.addEventListener('mouseleave', () => {
    if (trayOver) window.pet.taskbar.tray(trayOver, 'out')
    trayOver = null
  })

  // A press on a tag goes to its session; on the gear, the settings.
  barRest.addEventListener('pointerdown', e => {
    if (e.button !== 0) return
    const tag = e.target.closest('[data-jump]')
    if (tag) return jumpTo(tag.dataset.jump)
    if (e.target.closest('[data-bar="settings"]')) window.pet.openSettings()
  })
  // The taskbar's own buttons open what Windows' do: Start, and from the
  // clock the notifications and the calendar (once the button is up).
  barRest.addEventListener('click', e => {
    const what = e.target.closest('[data-bar]')?.dataset.bar
    if (what && what !== 'settings') window.pet.taskbar.open(what)
  })
  // A right press: a tray icon's program has it; on the input method, the
  // input methods to pick from; elsewhere, her menu.
  barRest.addEventListener('contextmenu', e => {
    e.preventDefault()
    if (trayKey(e)) return
    if (e.target.closest('[data-bar="ime"]')) return window.pet.taskbar.open('inputs')
    window.pet.menu()
  })
  // It takes the pointer while it is under it (the window lets clicks through elsewhere).
  barRest.addEventListener('mouseenter', () => window.pet.hover(true))
  barRest.addEventListener('mouseleave', () => window.pet.hover(false))

  function update() {
    clearInterval(clockTimer)
    // While she is being pulled out, the pull has the island.
    if (!isOn() || (pull?.started && !pull.carrying) || isAbsorbing) return
    // A prompt answered: the island closes rather than lingering open.
    if (view === 'ask' && !isAsking()) nudge = null
    view = isSetting() ? 'settings' : isAsking() ? 'ask' : isHover || isNudging() ? 'expanded' : 'compact'
    island.dataset.view = view
    island.style.setProperty('--ring', hue()[now.mood])
    island.classList.toggle('wants', now.mood === 'waiting')
    seatPanel()
    body.classList.toggle('has-her', isHome())
    body.classList.toggle('compact-her', view === 'compact' || view === 'settings')
    if (spriteUrl) herSheet.style.backgroundImage = `url("${spriteUrl}")`
    fillCompact()
    fillExpanded()
    fillBar()

    const want = sizeOf(view)
    // Reaching for her keeps the room to reach in.
    const grows = setRoom(isReaching ? PULL_ROOM : roomFor(want))
    // The window has its room before the island (and she) grow into it.
    const grow = () => {
      setSize(want)
      const snap = isHome() && !herShown
      if (snap) her.classList.add('snap')
      placeHer(want)
      if (snap) {
        void her.offsetWidth
        her.classList.remove('snap')
      }
      herShown = isHome()
      animateHer()
    }
    if (grows) setTimeout(grow, 40)
    else grow()

    // The clocks run: the turn's, the step's, the other sessions'. Their words
    // may grow (a step timed once it has run a while), so the shape follows.
    const isTimed = time() || now.stepSince || Status.othersOf(now.list, now).some(x => Status.time(x))
    if (isTimed && (view === 'compact' || view === 'expanded')) clockTimer = setInterval(update, 1000)
    spin()
  }

  function nudgeFor(clip, text, ms = NUDGE_MS, widget = null) {
    nudge = { until: Date.now() + ms, clip, text, widget }
    clearTimeout(nudgeTimer)
    nudgeTimer = setTimeout(() => {
      nudge = null
      update()
    }, ms)
    update()
  }

  // Now and then a blink, as a class for a moment (no running animation).
  function blink() {
    clearTimeout(blinkTimer)
    blinkTimer = setTimeout(() => {
      if (isOn() && !document.hidden) {
        island.classList.add('blink')
        setTimeout(() => island.classList.remove('blink'), 140)
      }
      blink()
    }, 3500 + Math.random() * 4000)
  }

  // --- Pulling her out: the drop --------------------------------------------------

  function pullDown(e) {
    console.log('pointerdown on her in the island, button', e.button)
    if (e.button !== 0 || !isHome() || view === 'ask' || isSetting() || isAbsorbing) return
    // No mouse events follow, so the island does not take this for a click.
    e.preventDefault()
    pull = { x0: e.clientX, y0: e.clientY, started: false, broken: false, at: null, carrying: false }
    window.pet.holding(true)
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

  function pullMove(e) {
    if (pull?.started) pulledAt = Date.now()
    if (!pull || pull.carrying) return
    if (!pull.started) {
      if (Math.hypot(e.clientX - pull.x0, e.clientY - pull.y0) < 5) return
      startPull()
    }
    stretch(e.clientX, e.clientY)
    // Pinched off: from here her own window carries her, anywhere on the screen.
    if (pull.broken) handOff(e)
  }

  function startPull() {
    pull.started = true
    nudge = null
    clearTimeout(nudgeTimer)
    clearInterval(clockTimer)
    // Room to stretch in, then the drop: she hangs in it kicking her legs.
    setRoom(PULL_ROOM)
    startDropHer()
    body.classList.add('pulling')
  }

  function startDropHer() {
    clearTimeout(dropTimer)
    dropSheet.style.backgroundImage = `url("${spriteUrl}")`
    const { row, frames, ms } = CLIPS.jumping
    let frame = 0
    const step = () => {
      showFrame(dropSheet, row, frame)
      frame = (frame + 1) % frames
      dropTimer = setTimeout(step, ms)
    }
    step()
  }

  // Where a neck leaves her home: the point of its edge nearest (x, y), so a
  // drop below the island hangs from its lower edge, and one above the
  // taskbar stands on its upper edge.
  function edgeNear(r, x, y) {
    return [Math.min(Math.max(x, r.left + 24), r.right - 24), Math.min(Math.max(y, r.top + 14), r.bottom - 14)]
  }

  // Her seat in her home, in the page: where she springs back to.
  function seatIn(r) {
    if (home() === 'taskbar') return [r.left + r.height / 2, r.top + r.height / 2]
    return [r.left + 18, r.top + 18]
  }

  // The home's double, a neck from it to (cx, cy), and a drop there; the
  // filter melts them into one shape. A neck of 0 leaves the drop on its own.
  function drawGoo(cx, cy, { neck, radius = DROP_R }) {
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
  function stretch(x, y) {
    const r = island.getBoundingClientRect()
    const cx = x
    // From the top of the screen she can only come down, from the taskbar
    // only up.
    const cy = home() === 'taskbar' ? Math.min(y + 30, r.top + 10) : Math.max(y + 30, r.bottom - 10)
    const [ax, ay] = edgeNear(r, cx, cy)
    const dist = Math.hypot(cx - ax, cy - ay)
    pull.broken = dist > BREAK_PX
    drawGoo(cx, cy, { neck: pull.broken ? 0 : Math.max(8, 34 - dist * 0.22) })
    Object.assign(dropHer.style, { left: `${cx - BODY_W / 2}px`, top: `${cy - BODY_H / 2}px` })
    pull.at = { cx, cy }
  }

  // The drop let go of her: her window takes her where she is, and the island
  // wobbles back with her seat empty.
  function handOff(e) {
    console.log('hand-off')
    pull.carrying = true
    clearTimeout(dropTimer)
    window.pet.releaseHer({ dx: pull.at.cx - e.clientX, dy: pull.at.cy - e.clientY })
    body.classList.remove('pulling')
    wobble()
    herClip = null
    update()
  }

  function wobble() {
    body.classList.remove('wobble')
    void island.offsetWidth
    body.classList.add('wobble')
    setTimeout(() => body.classList.remove('wobble'), 520)
  }

  function pullUp(e) {
    console.log('pull up:', e?.type, pull ? (pull.carrying ? 'carrying' : pull.started ? 'pulling' : 'pressed') : 'no pull')
    window.removeEventListener('pointermove', pullMove)
    window.removeEventListener('pointerup', pullUp)
    window.removeEventListener('pointercancel', pullUp)
    window.pet.holding(false)
    if (!pull) return
    const done = pull
    pull = null
    if (!done.started) {
      // A click on her, not a pull: the settings, as anywhere on the island.
      if (view !== 'ask') window.pet.openSettings()
      return
    }
    pulledAt = Date.now()
    if (done.carrying) {
      // Let go: she lands there, or comes home if she was let go by the island.
      window.pet.dropHer()
      window.pet.hover(false)
      return
    }
    pull = done
    settleBack(done.at)
  }

  // Into her seat from where the drop has her: the drop and she spring back.
  function settleBack(at) {
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
      clearTimeout(dropTimer)
      dropHer.style.transform = ''
      pull = null
      isAbsorbing = false
      herClip = null
      window.pet.hover(island.matches(':hover'))
      update()
    }, 340)
  }

  // --- Reaching for her, and taking her back ------------------------------------------

  // Run fn once the window has its room to reach in: drawn before, the
  // island would move under the drawing as the window grows around it.
  function whenRoomy(fn) {
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
  function reach(at) {
    if (!isOn() || isAbsorbing) return
    clearTimeout(retractTimer)
    if (!at) {
      if (!isReaching) return
      isReaching = false
      // The drop draws back into the island.
      body.classList.add('settling')
      const r = island.getBoundingClientRect()
      const [sx, sy] = seatIn(r)
      drawGoo(sx, sy, { neck: 0, radius: 8 })
      retractTimer = setTimeout(() => {
        body.classList.remove('reaching', 'settling')
        update()
      }, 320)
      return
    }
    if (!isReaching) {
      isReaching = true
      // The next call draws, once the window has grown.
      return whenRoomy(() => isReaching && reach(at))
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
  function absorb(at) {
    if (!isOn() || !at) return
    clearTimeout(retractTimer)
    isReaching = false
    isAbsorbing = true
    isComingHome = true
    clearTimeout(comingHomeTimer)
    // Main says she is in a moment after; should it not, she is not.
    comingHomeTimer = setTimeout(() => {
      isComingHome = false
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

  window.pet.onReach(reach)
  window.pet.onAbsorb(absorb)
  // She landed but this page never heard the button go up: forget the pull.
  window.pet.onLanded(() => {
    if (!pull?.carrying) return
    console.log('landed without a pointerup here')
    window.removeEventListener('pointermove', pullMove)
    window.removeEventListener('pointerup', pullUp)
    window.removeEventListener('pointercancel', pullUp)
    pull = null
    window.pet.hover(false)
    update()
  })

  her.addEventListener('pointerdown', pullDown)
  island.addEventListener('lostpointercapture', () => console.log('lost pointer capture', pull ? (pull.carrying ? 'while carrying' : 'while pulling') : ''))

  // --- On and off ---------------------------------------------------------------

  // The prompt panel lives in the island while it is on (a banner in the
  // settings while they are open), above the pet otherwise.
  function seatPanel() {
    if (!isOn()) return
    const seat = isSetting() ? window.Settings.askSlot : island
    if (panel.parentElement !== seat) seat.append(panel)
  }

  function place() {
    body.classList.toggle('island', isOn())
    body.classList.remove('home-island', 'home-taskbar')
    if (isOn()) body.classList.add(`home-${home()}`)
    drawMica()
    seatPanel()
    if (!isOn() && panel.parentElement !== stage) stage.prepend(panel)
    if (!isOn()) {
      clearInterval(clockTimer)
      clearTimeout(herTimer)
      clearTimeout(dropTimer)
      clearTimeout(roomTimer)
      body.classList.remove('pulling', 'settling', 'reaching', 'has-her', 'compact-her')
      pull = null
      isReaching = false
      isAbsorbing = false
      herClip = null
      room = null
      view = ''
      herShown = false
    }
  }

  window.pet.onUpdate(data => {
    const before = now.mood
    now = data
    lang = data.lang || lang
    config = data.config || {}
    if (config.out !== true) isComingHome = false
    spriteUrl = data.sprite
    second = data.second || null
    // The same widget stays shown as the list changes around it.
    const was = widgets.length ? widgets[widgetAt % widgets.length].id : null
    const all = Array.isArray(data.widgets) ? data.widgets : []
    monitor = all.find(w => w.id === 'monitor') || null
    widgets = all.filter(w => !(w === monitor && isPinned()))
    const still = widgets.findIndex(w => w.id === was)
    if (still >= 0) widgetAt = still
    place()
    // Something new that wants you, or is finished: the island opens for a
    // moment, a longer one with Claude's words to read.
    const isReading = isDetailed() && Status.isEnding(now) && !!now.reply
    if (isOn() && before !== now.mood && NUDGE_CLIP[now.mood]) nudgeFor(NUDGE_CLIP[now.mood], null, isReading ? NUDGE_READ_MS : NUDGE_MS)
    else update()
  })

  // A hello, or something to fix: the island opens to say it, and she waves.
  window.pet.onReact(({ say: text }) => {
    if (isOn() && text) nudgeFor('waving', say(lang, text))
  })

  // A widget asks to open the island (main checked that nothing wants you).
  window.pet.onNudge(n => {
    if (!isOn()) return
    const at = widgets.findIndex(w => w.id === n.id)
    if (at >= 0) widgetAt = at
    nudgeFor('waving', null, NUDGE_READ_MS, { ...n, icon: widgets[at]?.icon, color: widgets[at]?.color })
  })

  // --- The pointer ----------------------------------------------------------------

  for (const target of [island]) {
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
    // One click opens the settings (no double-click to wait for), except on a
    // prompt and its buttons, in the settings themselves, or on her (her
    // press may be a pull; pullUp decides). In the open island, a click on a
    // session goes to its window instead.
    target.addEventListener('click', e => {
      // A click in the settings (✕, the head) may just have closed them: not a click to open.
      if (isSetting() || window.Settings?.layer?.contains(e.target) || Date.now() - pulledAt < 500) return
      if (view === 'ask' || panel.contains(e.target) || her.contains(e.target)) return
      // The press, if the clocks redrew the session between it and the release.
      const id = view === 'expanded' && (e.target.closest('[data-jump]')?.dataset.jump || (Date.now() - pressed.at < 1000 && pressed.id))
      pressed = { id: '', at: 0 }
      if (id) return jumpTo(id)
      // A widget in the open island: the one shown from now on.
      const row = view === 'expanded' && e.target.closest('[data-widget]')
      if (row) {
        const at = widgets.findIndex(w => w.id === row.dataset.widget)
        if (at >= 0) turnWidget(at - (widgetAt % widgets.length))
        return
      }
      // A word that points somewhere (an agent answered about a letter): there.
      const open = isNudging() && nudge.widget?.open
      if (open?.tab === 'mail') {
        window.Settings?.openLetter?.({ account: open.account, uid: open.uid })
        return window.pet.openSettings('mail')
      }
      window.pet.openSettings()
    })
    // The wheel turns to the next widget, while the widgets have the island.
    target.addEventListener(
      'wheel',
      e => {
        if (!isQuiet() || widgets.length < 2 || isSetting() || view === 'ask') return
        e.preventDefault()
        turnWidget(e.deltaY > 0 ? 1 : -1)
      },
      { passive: false },
    )
    target.addEventListener('pointerdown', e => {
      const session = e.button === 0 && view === 'expanded' && e.target.closest('[data-jump]')
      pressed = { id: session ? session.dataset.jump : '', at: Date.now() }
    })
    target.addEventListener('contextmenu', e => {
      if (e.target.matches('input, textarea')) return
      e.preventDefault()
      window.pet.menu()
    })
  }

  blink()

  // panel.js tells the island when its prompt changes.
  // The most the settings may be high: above the taskbar, its room (the
  // window never moves for them); elsewhere, none said (the screen's).
  const settingsMax = () => (home() === 'taskbar' ? TASKBAR_ROOM - TASKBAR_GAP - TASKBAR_SHADE : null)

  window.Island = { isOn, changed: update, reach, absorb, settingsMax }
})()
