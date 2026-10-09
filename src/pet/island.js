// Her home: by display, a round portrait in a corner of the screen (corner),
// a black pill at the top centre after the iPhone's Dynamic Island (island),
// or a strip along the top of the screen (bar), its left end hers and the
// other sessions as tags along the rest. The same element in each, which
// springs between three shapes (in a corner, away from the corner; in the
// bar, hanging below it):
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
  // from the top of it. Wide enough for a pull (PULL_ROOM) and for the
  // monitor's capsule beside the widest island, so the window
  // only ever grows down: a window whose top-left corner moves shows its old
  // picture from the new corner for a frame or two, and the island jumped
  // sideways as she was pulled out or taken back.
  const BASE = { width: 960, height: 132 }
  const TOP = 8
  // Her two sizes: the round portrait in the compact island, and her whole
  // self (one sheet cell, halved) standing in the open one.
  const HEAD = 24
  const HEAD_SCALE = 0.21
  // Her portrait in the settings' head.
  const SET_HEAD = 32
  const SET_HEAD_SCALE = 0.28
  // The corner's circle and its margin, and the window without extra room
  // there: big enough for the card the circle opens into on a hover and for
  // a pull (PULL_ROOM), so none of those resize the window (one resized from
  // a corner moves its top-left corner and shows its old picture from there
  // for a frame or two: the circle jumped). It is see-through and lets
  // clicks through but over the island.
  // The bar's height and her portrait in it. (island.rs has these too.)
  const CIRCLE = 56
  const CORNER_M = 14
  const CORNER_BASE = { width: 760, height: 440 }
  const BAR_H = 30
  const BAR_HEAD = 22
  // A portrait px across is the sheet at this scale.
  const PER_PX = 0.00875
  // The settings' width.
  const SETTINGS_W = 520
  const BODY_SCALE = 0.5
  const BODY_W = Math.round(CELL_W * BODY_SCALE)
  const BODY_H = Math.round(CELL_H * BODY_SCALE)
  const OPEN_H = BODY_H + 12
  // The open island's widest, with her and without; the other sessions it lists.
  const OPEN_MAX_W = 480
  const OPEN_BARE_MAX_W = 440
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
  // The monitor stands apart: always in sight (its capsule, the bar's right
  // end), never one of the turns.
  let monitor = null
  // What the compact island showed last: something else slides in.
  let shownKey = ''
  let spinTimer = null

  const ROLE = new URLSearchParams(location.search).get('role') === 'island' ? 'island' : 'pet'
  // Risen only for the settings (she is the pet on her own): it goes once they close.
  // This page draws her home in the home's window; her own window draws her.
  const isOn = () => ROLE === 'island'
  // Her home, and for the corner, which.
  const home = () => (config.display === 'corner' || config.display === 'bar' ? config.display : 'island')
  const corner = () => (['br', 'bl', 'tr', 'tl'].includes(config.corner) ? config.corner : 'br')
  // In her seat: there is a pet to show, and she is not out on the desktop.
  const isHome = () => !!spriteUrl && (config.out !== true || isComingHome)
  const isAsking = () => isOn() && !panel.hidden
  // Out from the corner she does the talking (her bubble, her panel): the
  // corner keeps her portrait, and opens only when hovered.
  const sheTalks = () => home() === 'corner' && config.out === true
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
    node.style.cssText = `width:${px}px;height:${px}px;background:${COLOR[mood] || COLOR.idle}`
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
    node.style.cssText = `width:${px}px;height:${px}px;box-shadow:0 0 0 1.5px ${COLOR[now.mood] || COLOR.idle}`
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
    // The corner's circle: her alone, ringed in the mood's colour, and a
    // dot in the next one's when other sessions are busy too; her portrait
    // while she is out.
    cornerBadge.classList.toggle('on', now.others > 0)
    cornerBadge.style.background = COLOR[second] || COLOR.working
    if (home() === 'corner' && !isHome()) {
      compact.replaceChildren(stillHer(CIRCLE - 8))
      compact.classList.add('bare')
      return
    }
    // Three places: her at the left (her portrait while she is out), a line
    // of words cut short in the middle, a short value at the right edge (the
    // clock, a widget's number), with "+N" for the other sessions before it.
    const widget = shownWidget()
    let words, value, key
    if (widget) {
      const said = Widgets.words(lang, widget, isDetailed())
      words = el('span', 'label wlabel')
      words.insertAdjacentHTML('afterbegin', Widgets.icon(widget.icon))
      words.firstChild.style.color = widget.color || COLOR.idle
      words.append(said.label)
      value = el('span', 'clock wvalue', said.value)
      value.style.color = widget.color || COLOR.idle
      key = `w:${widget.id}`
    } else {
      words = el('span', 'label', Status.brief(lang, now, { detailed: isDetailed() }))
      value = el('span', 'clock', time())
      value.style.color = COLOR[now.mood]
      key = `s:${now.id || ''}:${now.mood}`
    }
    // In the bar the other sessions have their own tags.
    const more = now.others > 0 && home() !== 'bar' ? el('span', 'more', `+${now.others}`) : null
    if (more) more.style.color = COLOR[second] || COLOR.working
    compact.replaceChildren(...(isHome() ? [] : [stillHer(24)]), words, ...(more ? [more] : []), value)
    const isBare = !words.textContent && !value.textContent && !more
    compact.classList.toggle('bare', isBare)
    compact.style.width = isBare ? '' : `${slotWidth()}px`
    // Something else now (another widget's turn, another session): it slides in.
    if (key !== shownKey) {
      const isFirst = !shownKey
      shownKey = key
      if (!isFirst && view === 'compact') {
        compact.classList.remove('turn')
        void compact.offsetWidth
        compact.classList.add('turn')
      }
    }
  }

  // The compact island's width as set.
  const slotWidth = () => WIDTHS[config.islandWidth] || WIDTHS.normal

  // A widget's line in the open island: the one shown is lit.
  function widgetRow(widget, isShown) {
    const { label, value } = Widgets.words(lang, widget, isDetailed())
    const row = el('span', isShown ? 'wrow on' : 'wrow')
    row.dataset.widget = widget.id
    row.insertAdjacentHTML('afterbegin', Widgets.icon(widget.icon))
    row.firstChild.style.color = widget.color || COLOR.idle
    const valueText = el('span', 'wv', value)
    valueText.style.color = widget.color || COLOR.idle
    if (Widgets.partsOf(widget)) valueText.innerHTML = Widgets.partsHTML(widget)
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
    clockText.style.color = COLOR[now.mood]
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
      fill.style.cssText = `width:${Math.round((100 * now.todo.done) / now.todo.total)}%;background:${COLOR[now.mood]}`
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
        dot.style.background = COLOR[x.mood] || COLOR.idle
        const what = detailed ? Status.status(lang, x, { withClock: false }) : t(lang, `island.${x.mood}`)
        const when = el('span', 'when', Status.time(x))
        when.style.color = COLOR[x.mood] || COLOR.idle
        row.append(dot, el('span', 'who', Status.nameOf(x, detailed)), el('span', 'what', what), when)
        list.append(row)
      }
      if (others.length > OTHERS_SHOWN) list.append(el('span', 'other rest', t(lang, 'status.moreSessions', { n: others.length - OTHERS_SHOWN })))
      lines.append(list)
    }
    // Nothing from the sessions: the monitor, and the widgets, the one shown lit.
    if (isQuiet() && (widgets.length || monitor) && !(isNudging() && nudge.widget)) {
      const list = el('span', 'wlist')
      const shown = shownWidget()
      if (monitor) list.append(widgetRow(monitor, false))
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
      top.firstChild.style.color = w.color || COLOR.idle
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
      s.setProperty('--ring', COLOR[now.mood])
      herSheet.style.transform = `translate(${-(CELL_W * scale - px) / 2}px, -2px) scale(${scale})`
    }
    if (view === 'compact') {
      if (home() === 'corner') portrait(CIRCLE - 8, (CIRCLE - 8) * PER_PX, 4, 4)
      else if (home() === 'bar') portrait(BAR_HEAD, BAR_HEAD * PER_PX, 4, 4)
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

  // The window with no extra room, by home: the bar is as wide as the screen.
  const baseRoom = () => (home() === 'corner' ? CORNER_BASE : home() === 'bar' ? { width: innerWidth, height: BAR_H } : BASE)

  function roomFor(want) {
    if (home() === 'corner') {
      const width = want.width + 2 * CORNER_M + 8
      const height = want.height + 2 * CORNER_M + 8
      return width <= CORNER_BASE.width && height <= CORNER_BASE.height ? null : { width, height }
    }
    // The bar: whatever hangs below it (main keeps the width the screen's).
    if (home() === 'bar') {
      if (want.height <= BAR_H) return null
      return { width: innerWidth, height: want.height + 12 }
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
    // The bar's left end is square, and what hangs from it is rounded below;
    // the corner's is a circle until it opens.
    island.style.borderRadius =
      home() === 'bar' ? (view === 'compact' ? '0' : '0 0 22px 22px') : home() === 'corner' && view === 'compact' ? `${CIRCLE / 2}px` : `${height > 60 ? 30 : height / 2}px`
  }

  // --- Deciding ---------------------------------------------------------------

  function sizeOf(name) {
    const withHer = isHome()
    if (name === 'settings') return { width: SETTINGS_W, height: window.Settings.layer.offsetHeight }
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
    if (home() === 'corner') return { width: CIRCLE, height: CIRCLE }
    const width = compact.classList.contains('bare') ? Math.max(MIN_W, compact.offsetWidth) : slotWidth()
    return { width, height: home() === 'bar' ? BAR_H : 36 }
  }

  // --- The bar's right part ---------------------------------------------------------

  // The other sessions as tags (a press goes to one's window), a widget, and
  // the settings. Its left edge stays where the compact island ends, so what
  // opens from the island hangs over it.
  const barRest = document.createElement('div')
  barRest.id = 'bar-rest'
  island.after(barRest)
  // The corner's dot for other busy sessions.
  const cornerBadge = el('span')
  cornerBadge.id = 'corner-badge'
  island.append(cornerBadge)
  // The monitor's capsule, beside the compact island (the island keeps the
  // middle): its readings always in sight, whatever the island says.
  const gauge = el('div')
  gauge.id = 'island-gauge'
  island.after(gauge)
  compact.addEventListener('animationend', () => compact.classList.remove('turn'))

  function fillGauge() {
    const isUp = isOn() && home() === 'island' && view === 'compact' && !!monitor
    gauge.classList.toggle('on', isUp)
    if (isUp) gauge.innerHTML = Widgets.partsHTML(monitor)
  }

  function fillBar() {
    if (home() !== 'bar') return
    barRest.style.left = `${sizeOf('compact').width}px`
    const detailed = isDetailed()
    const tags = Status.othersOf(now.list, now).map(x => {
      const tag = el('span', 'tag')
      if (x.jump && x.id) {
        tag.dataset.jump = x.id
        tag.title = t(lang, 'jump.hint')
      }
      const dot = el('i', 'dot')
      dot.style.background = COLOR[x.mood] || COLOR.idle
      const when = el('span', 'when', Status.time(x))
      when.style.color = COLOR[x.mood] || COLOR.idle
      tag.append(dot, el('span', 'who', Status.nameOf(x, detailed)), el('span', 'what', Status.brief(lang, x, { detailed })), when)
      return tag
    })
    const parts = [el('span', 'tags'), el('span', 'grow')]
    parts[0].append(...tags)
    const w = widgets.length ? widgets[widgetAt % widgets.length] : null
    if (w) {
      const { label, value } = Widgets.words(lang, w, detailed)
      const face = el('span', 'face')
      face.insertAdjacentHTML('afterbegin', Widgets.icon(w.icon))
      face.firstChild.style.color = w.color || COLOR.idle
      const v = el('span', 'v', value)
      v.style.color = w.color || COLOR.idle
      if (Widgets.partsOf(w)) v.innerHTML = Widgets.partsHTML(w)
      face.append(Widgets.partsOf(w) ? '' : label, v)
      parts.push(face)
    }
    // The monitor, always there by the settings.
    if (monitor) {
      const readings = el('span', 'face mon')
      readings.innerHTML = Widgets.partsHTML(monitor)
      parts.push(readings)
    }
    const gear = el('span', 'gear')
    gear.dataset.bar = 'settings'
    gear.insertAdjacentHTML('afterbegin', '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="3"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M4.9 19.1L7 17M17 7l2.1-2.1"/></svg>')
    parts.push(gear)
    barRest.replaceChildren(...parts)
  }

  // A press on a tag goes to its session; on the gear, the settings.
  barRest.addEventListener('pointerdown', e => {
    if (e.button !== 0) return
    const tag = e.target.closest('[data-jump]')
    if (tag) return jumpTo(tag.dataset.jump)
    if (e.target.closest('[data-bar="settings"]')) window.pet.openSettings()
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
    island.style.setProperty('--ring', COLOR[now.mood])
    island.classList.toggle('wants', now.mood === 'waiting')
    seatPanel()
    body.classList.toggle('has-her', isHome())
    body.classList.toggle('compact-her', view === 'compact' || view === 'settings')
    if (spriteUrl) herSheet.style.backgroundImage = `url("${spriteUrl}")`
    fillCompact()
    fillExpanded()
    fillBar()
    fillGauge()

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
  // drop below the island hangs from its lower edge, and one up and to the
  // left of a corner's circle from that side.
  function edgeNear(r, x, y) {
    return [Math.min(Math.max(x, r.left + 24), r.right - 24), Math.min(Math.max(y, r.top + 14), r.bottom - 14)]
  }

  // Her seat in her home, in the page: where she springs back to.
  function seatIn(r) {
    if (home() === 'corner') return [r.left + CIRCLE / 2, r.top + CIRCLE / 2]
    if (home() === 'bar') return [r.left + 15, r.top + 15]
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
    // From the top of the screen she can only come down; from a corner, any way.
    const cy = home() === 'corner' ? y + 30 : Math.max(y + 30, r.bottom - 10)
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
    body.classList.remove('home-corner', 'home-island', 'home-bar', 'at-br', 'at-bl', 'at-tr', 'at-tl')
    if (isOn()) body.classList.add(`home-${home()}`, `at-${corner()}`)
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
    widgets = all.filter(w => w.id !== 'monitor')
    const still = widgets.findIndex(w => w.id === was)
    if (still >= 0) widgetAt = still
    place()
    // Something new that wants you, or is finished: the island opens for a
    // moment, a longer one with Claude's words to read.
    const isReading = isDetailed() && Status.isEnding(now) && !!now.reply
    if (isOn() && !sheTalks() && before !== now.mood && NUDGE_CLIP[now.mood]) nudgeFor(NUDGE_CLIP[now.mood], null, isReading ? NUDGE_READ_MS : NUDGE_MS)
    else update()
  })

  // A hello, or something to fix: the island opens to say it, and she waves.
  window.pet.onReact(({ say: text }) => {
    if (isOn() && !sheTalks() && text) nudgeFor('waving', say(lang, text))
  })

  // A widget asks to open the island (main checked that nothing wants you).
  window.pet.onNudge(n => {
    if (!isOn() || sheTalks()) return
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
      if (e.target.matches('input')) return
      e.preventDefault()
      window.pet.menu()
    })
  }

  blink()

  // panel.js tells the island when its prompt changes.
  window.Island = { isOn, changed: update, reach, absorb }
})()
