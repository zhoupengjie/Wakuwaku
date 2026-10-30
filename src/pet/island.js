// Her home: by display, a black pill at the top centre after the iPhone's
// Dynamic Island (island; or at a top corner, anchor.js), or a strip along
// the bottom of the screen in place of Windows' taskbar (taskbar): its left
// end hers, the other sessions as tags along the rest, with the Start
// button, the tray's icons (taskbar.rs) and the clock besides. The same element in each, which
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
// turns in the compact island, and the open one lists her capsule's; the wheel
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
  const { t, render: say, list: nameList } = window.I18n
  const { CELL_W, CELL_H, CLIPS, MOOD_CLIP, REACTIONS } = window.Sprite
  const Status = window.Status
  const Widgets = window.Widgets
  const Flow = window.Flow
  const Anchor = window.Anchor

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
  // The moods' colours in her island, as its words are (setInk): deeper on
  // a light ground; on the taskbar's strip (stripHue), as its words are.
  let inkLight = false
  let groundLight = null
  const hue = () => (inkLight ? COLOR_LIGHT : COLOR)
  const stripHue = () => (isStripLight() ? COLOR_LIGHT : COLOR)
  // A widget's own colour (a script's, or the monitor's), on a light ground
  // as deep as the moods' there: one of the moods' colours as its deeper
  // one, any other darkened until it reads on white (widgets.js).
  function deeper(colour, light) {
    if (!colour || !light) return colour
    const mood = Object.keys(COLOR).find(k => COLOR[k] === String(colour).toLowerCase())
    return mood ? COLOR_LIGHT[mood] : Widgets.deepen(colour)
  }
  const ink = colour => deeper(colour, inkLight)
  const stripInk = colour => deeper(colour, isStripLight())

  // What she plays when the island opens by itself for each mood.
  const NUDGE_CLIP = { waiting: 'waiting', done: 'waving', review: 'review', error: 'failed' }

  // How long the island stays open when something happens (longer with
  // Claude's words to read): a prompt, a new version; what the 「她有事告诉
  // 你时」 setting governs (a turn's end, a hello, a plugin's nudge) as long
  // as the hold says (Flow.openFor), these its time once seen. And how long
  // the pointer has to rest on it before it opens (so passing by does not).
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
  // On a Mac the island hangs from the screen's top edge over the menu bar
  // (notch.rs, island:top): { height, notch }, the band's height and the
  // notch's width (0 without one); null elsewhere. With a notch the compact
  // island wraps round it: her in the ear at its left, the clock in the ear at
  // its right, the words (which the notch would hide) left out.
  let screenTop = null
  const EAR = 64
  // At the top of the screen, the side island.rs placed her window at
  // (pet:update islandAt: left, center, right; zhoupengjie/Wakuwaku#59): the
  // island laid out at that side of it (isl-at-left, isl-at-right), and all
  // that follows it (anchor.js). None said (an older island.rs), the
  // centre, where it is.
  let islandAt = 'center'
  const anchor = () => Anchor.anchorOf({ home: home(), at: islandAt, notch: !!screenTop?.notch })
  // Dropped open from the notch by her tray icon (setPeek), for PEEK_MS at
  // most if the pointer never comes.
  let peek = false
  let peekTimer = 0
  const PEEK_MS = 6000
  let settingsIdleTimer = 0
  let settingsUsed = false
  // Where the pointer was first seen in the settings: a move counts once it
  // goes a few px from there (WebKit may send one as they open under it).
  let settingsFrom = null
  // Listened for on a Mac only: the notch is there alone.
  for (const type of window.pet.mac ? ['pointermove', 'pointerdown', 'wheel', 'keydown'] : []) {
    document.addEventListener(
      type,
      e => {
        if (view !== 'settings') return
        if (type !== 'pointermove') settingsUsed = true
        else if (!settingsFrom) settingsFrom = { x: e.screenX, y: e.screenY }
        else if (Math.hypot(e.screenX - settingsFrom.x, e.screenY - settingsFrom.y) > 8) settingsUsed = true
      },
      true,
    )
  }
  const slotWidth = () => (screenTop?.notch ? screenTop.notch + 2 * EAR : WIDTHS[config.islandWidth] || WIDTHS.normal)
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
  // Its padding above and below its lines (style.css #island-expanded).
  const EXPANDED_PAD_Y = 24
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
  // Not connected yet: a dot in Windows' accent at her portrait's top-right
  // (placeHer puts it there; style.css shows it in the compact island).
  const hintDot = document.createElement('span')
  hintDot.className = 'hint-dot'
  her.after(hintDot)
  // Something happened while the island stayed closed (the flow light,
  // below): a dot of the same kind at the same place, in the newest event's
  // colour, until seen. Never with the hint's: not connected, nothing happens.
  const newsDot = document.createElement('span')
  newsDot.className = 'news-dot'
  hintDot.after(newsDot)
  newsDot.addEventListener('animationend', () => newsDot.classList.remove('pop', 'hop'))
  // The dot's event: its kind and (a plugin's) colour, and whether it shows
  // yet (it comes as the ring fades).
  let news = null
  // What the island would have shown by itself: { clip, text, ms, widget,
  // until }, shown as it next opens (takePending), until then.
  let pending = null
  // An app full screen on her display (pet:full-screen, main.rs look_fullscreen).
  let fullScreen = false
  // A reaction inside a turn her portrait plays: { clip, until }.
  let reacting = null
  let reactTimer
  // The flow light's ring, by the island in its row (the island clips what
  // is in it): see "The flow light" below.
  const flow = document.createElement('div')
  flow.id = 'flow'
  flow.innerHTML = '<div class="flow-glow"><div class="flow-ring"><i class="flow-spin"><b></b><b></b></i></div></div><div class="flow-ring"><i class="flow-spin"><b></b><b></b></i></div>'
  island.after(flow)
  const spins = [...flow.querySelectorAll('.flow-spin')]
  const ANCHORED = !!window.CSS?.supports?.('anchor-name: --a')
  const isRinging = () => flow.classList.contains('on')
  // Its state (flow.js), its animations running, its colours' blends, its
  // dot's and its end's timers.
  let flowState = Flow.idle()
  let flowAnims = []
  let blends = []
  let flowTimers = []
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
  // OLED's guard against burn-in (the settings' oledShift): what stays put
  // all day, the taskbar's icons, words and clock and the island, a screen
  // pixel off every 2.5 minutes, round a 5 x 5 square row by row and back
  // (two hours), one pixel a step: too little to see. Whole screen pixels
  // (a part of one blurs the words); by the clock, so a restart goes on
  // where it was. The strip's own ground stays put (style.css --oled-x).
  // Up here: what lays the taskbar out (alignApps) reads it.
  const OLED_STEP_MS = 150000
  const OLED_PATH = (() => {
    const square = []
    for (let y = -2; y <= 2; y++) {
      const xs = [-2, -1, 0, 1, 2]
      square.push(...(y % 2 ? xs.reverse() : xs).map(x => [x, y]))
    }
    return [...square, ...square.slice(1, -1).reverse()]
  })()
  let oledOn = false
  let oledTimer
  function oledShift() {
    clearTimeout(oledTimer)
    const root = document.documentElement
    if (oledOn) {
      const at = Date.now()
      const [x, y] = OLED_PATH[Math.floor(at / OLED_STEP_MS) % OLED_PATH.length]
      const dpr = devicePixelRatio || 1
      root.style.setProperty('--oled-x', `${x / dpr}px`)
      root.style.setProperty('--oled-y', `${y / dpr}px`)
      root.classList.add('oled-shift')
      oledTimer = setTimeout(oledShift, OLED_STEP_MS - (at % OLED_STEP_MS) + 50)
    } else {
      root.classList.remove('oled-shift')
      root.style.removeProperty('--oled-x')
      root.style.removeProperty('--oled-y')
    }
    // The tray's icons where they are drawn now, for the menus they open.
    requestAnimationFrame(sendTrayRects)
  }
  // Windows' look as taskbar.rs sent it last: its mode (dark, light) and
  // accent colour; and whether its End task is on (a program's menu offers it).
  let look = { mode: 'dark' }
  let spriteUrl = null
  let second = null
  let isHover = false
  let hoverTimer
  // Something just happened: { until, clip, text }. The island opens for a
  // moment, she plays clip, and text (a hello) replaces the mood's line.
  let nudge = null
  let nudgeTimer
  // Closed by a press while it was open by itself (dismiss): the pointer
  // still on it opens it no more, until it has left and come back.
  let dismissed = false
  let view = ''
  let clockTimer
  let blinkTimer
  let herClip = null
  // Nobody at the keyboard or mouse for a while (pet:quiet): she stands still.
  let quiet = false
  let herTimer
  let room = null
  let roomTimer
  // On the taskbar, what stands open up from the strip, as last told:
  // the island (open in any way), and the lists and flyouts over the strip
  // (watchPresses). Opening, her window is raised over any window come over
  // it (openedUp, island.rs raise, zhoupengjie/Wakuwaku#55).
  const openUp = { island: false, pops: false }
  // The open island's lines scrolled this far (fillExpanded), and a card
  // just unfolded in them, to be brought into sight.
  let linesTop = 0
  let cardFresh = false
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
  // Her window up on the desktop (main.rs payload herUp): out there she says
  // it herself, the prompts over her head too (pet.js, panel.js), and the
  // island does not open for them.
  let herUp = false
  const herTalks = () => config.out === true && herUp && !isComingHome
  // Her live portrait was showing at the last draw: one showing again is put
  // in its place at once, not sprung there from where she last stood (her
  // whole self, if she was pulled out of the open island).
  let herShown = false
  let retractTimer
  // The session a press in the open island landed on: the clocks redraw it
  // every second, so the release may come down on its redrawn self.
  let pressed = { id: '', at: 0 }
  // The widgets that are on, the one shown, and the turn to the next; all
  // with something to say, on or off (any may be on the taskbar).
  let widgets = []
  let widgetAt = 0
  let allWidgets = []
  // The monitor, pinned (its setting, on by default): always in sight at the
  // island's right end (the bar's), never one of the turns. Unpinned it is
  // one of the widgets taking turns.
  let monitor = null

  // Those put elsewhere: on the desktop (desk.rs), or on the taskbar while
  // it is her home. A widget is in one place (§7): neither taking turns in
  // the closed island nor pinned at its end, nor listed in the open island
  // (that lists her capsule's own, the picker of 2026-10-17).
  let placedOut = new Set()
  let pinnedNow = false
  let listedNow = []
  // picker.js: her capsule's members (capsule.js: the capsule setting's
  // order and modes), and those pinned at its right end.
  let membersNow = []
  let pinsNow = []

  // The widgets as sent: those that take turns are those on, but for the
  // monitor pinned and what is put elsewhere (deskgrid.js placement, the
  // rule tested there); the same one stays shown as the list changes
  // around it.
  function takeWidgets(all) {
    const was = widgets.length ? widgets[widgetAt % widgets.length].id : null
    allWidgets = Array.isArray(all) ? all : []
    const p = DeskGrid.placement({ all: allWidgets, home: home(), taskbar: home() === 'taskbar' ? tbLayout() : null, desk: config.deskLayout, pin: config.monitor?.pin, capsule: config.capsule })
    ;({ monitor, pinned: pinnedNow, listed: listedNow, out: placedOut, turning: widgets, members: membersNow, pins: pinsNow } = p)
    const still = widgets.findIndex(w => w.id === was)
    if (still >= 0) widgetAt = still
  }
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
  // How light the desktop under the clear strip is, along it (mica.js
  // lightness): what each part's words go by (inkParts); null, not known
  // (yet). backdropLight: the strip's as a whole, by its mean, true light
  // enough for dark words.
  let backdrop = null
  const backdropLight = () => (backdrop ? Mica.wantsDarkInk(backdrop.mean) : null)
  // The taskbar light, as Windows' own is in its light mode (taskbar.rs):
  // the strip's ground, what opens from it, her island, the settings.
  const isLight = () => isOn() && home() === 'taskbar' && look.mode === 'light'
  // The strip's own words dark (tb-ink-light), what is drawn on it alone:
  // clear, with the desktop showing through it, as the desktop under them
  // wants (backdropLight), as text over iOS's glass goes by what is behind
  // it, not by the system's mode (dark words on a dark wallpaper, haloed
  // white, did not read); else, or while that is not known, as Windows'.
  // The strip as a whole: what lies on it outside its parts; each part by
  // what is under it alone (partLight, ink-light).
  const isStripLight = () => Mica.stripInkLight({ light: isLight(), clear: isClear(), filled: isFilled(), backdrop: backdropLight() })
  // Her end a capsule of its own in the strip (black, or light in Windows'
  // light mode), wherever the strip is not black itself: on the Mica, clear,
  // or light.
  const isCapsule = () => isMica() || isClear() || isLight()
  // The screen's edge the taskbar is along (taskbarEdge): bottom, as
  // Windows' own, or top, left, right. Its look turns with it (style.css
  // tb-top, tb-left, tb-right).
  const tbEdge = () => (home() === 'taskbar' && ['top', 'left', 'right'].includes(config.taskbarEdge) ? config.taskbarEdge : 'bottom')
  // Along the left or right of the screen: upright, its runs top, middle,
  // bottom, its buttons one above another.
  const isSide = () => tbEdge() === 'left' || tbEdge() === 'right'
  // Along the strip and across it, in the page: x and left/right on a
  // bottom or top bar, y and top/bottom on a side one.
  const along = (x, y) => (isSide() ? y : x)
  const across = (x, y) => (isSide() ? x : y)
  const lo = r => (isSide() ? r.top : r.left)
  const hi = r => (isSide() ? r.bottom : r.right)
  const len = r => (isSide() ? r.height : r.width)
  const crossLo = r => (isSide() ? r.left : r.top)
  const crossHi = r => (isSide() ? r.right : r.bottom)
  // A side bar's capsule (A): upright, as wide as the strip less 5 a side
  // (on the black strip, its end as wide as the strip), her portrait in its
  // round end, her session's clock under it, no words (the tooltip and the
  // open island have them): a circle with no clock.
  const SIDE_CLOCK = 4 + 13 + 7
  function sideCapsule(timed) {
    const width = isCapsule() ? TASKBAR_H - 2 * CAPSULE_Y : TASKBAR_H
    const inset = (width - TASK_HEAD) / 2
    return { width, height: timed ? inset + TASK_HEAD + SIDE_CLOCK : width }
  }
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
  // Neither Claude Code nor Codex connected (main.rs payload, unconnected),
  // and no "don't remind me" (connectHintOff): the island asks for it, and a
  // press on it opens the settings' welcome, where they are connected.
  const isHinting = () => now.unconnected === true && config.connectHintOff !== true
  // The tools on this computer by name, as one says them (main.rs: with
  // unconnected): 「Claude Code、Codex 或 Antigravity」.
  const toolNames = () => nameList(lang, now.tools || ['Claude Code', 'Codex', 'Antigravity', 'OpenCode'], 'or')
  // Windows' accent as it reads on the island's ground: its light shade on a
  // dark one, its dark shade on a light one.
  const hintAccent = () => (isLight() ? look.accent?.dark || '#0067c0' : look.accent?.light || '#4cc2ff')
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

  // The portraits' clock (playPortraits).
  let portraitTimer = null

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
    sheet.style.cssText = `transform:translate(${-(CELL_W * scale - px) / 2}px, -2px) scale(${scale})`
    sheetImage(sheet, spriteUrl)
    showFrame(sheet, CLIPS[clip].row, frameNow(CLIPS[clip]))
    node.append(sheet)
    playPortraits()
    return node
  }

  // The frame a clip is on now, by the clock: a portrait drawn anew (the
  // island redraws every second) carries on where the last one was.
  const frameNow = clip => Math.floor(Date.now() / clip.ms) % clip.frames

  // Her portraits while she is out play her mood's clip (not while quiet).
  // Its timer runs only while there are any to play, started as one is
  // drawn or the quiet ends: it woke 17 times a second all day for none
  // (#14).
  function playPortraits() {
    if (portraitTimer) return
    portraitTimer = setInterval(() => {
      const sheets = document.querySelectorAll('.still-her .sheet[data-clip]')
      if (!sheets.length || !isOn() || document.hidden || quiet) {
        clearInterval(portraitTimer)
        portraitTimer = null
        return
      }
      for (const sheet of sheets) {
        const clip = CLIPS[sheet.dataset.clip]
        if (clip) showFrame(sheet, clip.row, frameNow(clip))
      }
    }, 60)
  }

  // The sheet (the film) behind a cell-sized box, moved to the frame's cell
  // by a transform on its own layer (style.css .film): a frame costs no
  // repaint (it was the box's background-position, painted anew each frame).
  function filmOf(sheet) {
    let film = sheet.querySelector('.film')
    if (!film) {
      film = el('span', 'film')
      sheet.append(film)
    }
    return film
  }

  function showFrame(sheet, row, frame) {
    filmOf(sheet).style.transform = `translate3d(${-frame * CELL_W}px, ${-row * CELL_H}px, 0)`
  }

  function sheetImage(sheet, url) {
    filmOf(sheet).style.backgroundImage = url ? `url("${url}")` : ''
  }

  // --- The shapes ---------------------------------------------------------------

  function fillCompact() {
    // A side bar's upright capsule: her portrait (as the island draws it, or
    // a still one while she is out) and her session's clock under it; its
    // words in its tooltip.
    if (isSide()) {
      const at = time()
      const hint = isHinting()
      // Not connected: a link's glyph in the clock's place, in the accent.
      const clock = hint ? el('span', 'fi chint', '\uE71B') : el('span', 'clock', at)
      if (!hint) {
        clock.dataset.live = 'time'
        clock.style.color = hue()[now.mood]
      }
      compact.replaceChildren(...(isHome() ? [] : [stillHer(TASK_HEAD)]), ...(hint || at ? [clock] : []))
      compact.classList.toggle('bare', !(hint || at))
      compact.style.width = ''
      compact.title = hint ? t(lang, 'hint.tip', { list: toolNames() }) : Status.brief(lang, now, { detailed: isDetailed() })
      compactWidth = sideCapsule(hint || !!at).width
      return
    }
    compact.title = isHinting() ? t(lang, 'hint.tip', { list: toolNames() }) : ''
    // Three places: her at the left (her portrait while she is out), a line
    // of words cut short in the middle, a short value at its right (the
    // clock, a widget's number), with "+N" for the other sessions before it;
    // then, pinned, the monitor's readings at the island's right end.
    const widget = shownWidget()
    let words, value, key
    if (isHinting()) {
      // Not connected: "Connect Claude Code ›" in the accent, no clock.
      words = el('span', 'label hint', t(lang, 'hint.connect'))
      words.append(el('span', 'fi hchev', '\uE76C'))
      value = el('span', 'clock', '')
      key = 'hint'
    } else if (widget && Widgets.partsOf(widget)) {
      words = el('span', 'label wparts')
      words.innerHTML = Widgets.partsHTML(widget)
      value = el('span', 'clock')
      key = `w:${widget.id}`
    } else if (widget) {
      const said = Widgets.words(lang, widget, isDetailed())
      words = el('span', 'label wlabel')
      words.insertAdjacentHTML('afterbegin', Widgets.iconHTML(widget, lang))
      words.firstChild.style.color = ink(widget.color) || hue().idle
      words.append(said.label)
      value = el('span', 'clock wvalue', said.value)
      value.style.color = ink(widget.color) || hue().idle
      key = `w:${widget.id}`
    } else {
      words = el('span', 'label', Status.brief(lang, now, { detailed: isDetailed() }))
      value = el('span', 'clock', time())
      // Kept up each second in place (tickTimes).
      value.dataset.live = 'time'
      value.style.color = hue()[now.mood]
      key = `s:${now.id || ''}:${now.mood}`
    }
    // The other sessions, as "+N" (the island's open lines list them).
    const more = now.others > 0 ? el('span', 'more', `+${now.others}`) : null
    if (more) more.style.color = hue()[second] || hue().working
    // What takes turns, together: it slides in when it is something else.
    const turning = el('span', 'turning')
    turning.append(words, ...(more ? [more] : []), value)
    const isBare = !words.textContent && !value.textContent && !more
    // picker.js: the members pinned at its end (five at most, capsule.js): the
    // monitor's readings as they were, another its glyph and value.
    const pinned = pinsNow.length ? el('span', 'pinned') : null
    if (pinned) pinned.innerHTML = pinsNow.map(pinHTML).join('')
    compact.replaceChildren(...(isHome() ? [] : [stillHer(24)]), ...(isBare && pinned ? [] : [turning]), ...(pinned ? [pinned] : []))
    compact.classList.toggle('bare', isBare)
    // As many as there is room for (fitPins), what it says in the middle as wide as set.
    if (pinned && !screenTop?.notch) {
      if (isBare) compact.style.width = ''
      fitPinned(pinned, pinRoom(isBare ? compact.offsetWidth - pinned.offsetWidth : slotWidth() + 12))
    }
    // As wide as set, and the readings' width more when pinned (their own,
    // fixed); just her (and the readings) with nothing to say.
    // Round a notch: always tucked into it, just as wide and empty, whatever
    // goes on, so the menu bar's icons by it stay in sight and the notch is
    // left to the app that lives there (Atoll and the like: the user's call,
    // 2026-10-14). Her tray icon drops it open (peek); a prompt opens it.
    const tucked = !!screenTop?.notch
    body.classList.toggle('tucked', tucked)
    compact.style.width = tucked
      ? `${screenTop.notch}px`
      : isBare && !screenTop?.notch
        ? ''
        : `${slotWidth() + (pinned?.offsetWidth ? pinned.offsetWidth + 12 : 0)}px`
    compactWidth = compact.style.width ? parseFloat(compact.style.width) : Math.max(MIN_W, compact.offsetWidth)
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

  // A pin at the capsule's end: its glyph and value; the monitor its
  // readings, or (short of room) its glyph alone.
  function pinHTML(w) {
    const color = ink(w.color) || hue().idle
    if (Widgets.partsOf(w)) return `<span class="wpin wmon"><span class="wmfull">${Widgets.partsHTML(w)}</span><span class="wmico" style="color:${color}">${Widgets.iconHTML(w, lang)}</span></span>`
    return `<span class="wpin" style="color:${color}">${Widgets.iconHTML(w, lang)}<b>${WCards.esc(WCards.briefOf(w, { lang, light: inkLight, detailed: isDetailed() }).value)}</b></span>`
  }
  // Five pinned (the user's call of 2026-10-17) may not fit: each measured
  // with its value and as its glyph alone, capsule.js fitPins says which
  // shows how (the values going from the left first, the monitor's
  // readings last, then the others' that fit back; then the leftmost out,
  // a "+k" before the rest, their names in its tip).
  function fitPinned(pinned, room) {
    const nodes = [...pinned.children]
    const sizes = nodes.map(n => {
      const full = n.offsetWidth
      n.classList.add('short')
      const icon = n.offsetWidth
      n.classList.remove('short')
      return { full, icon, monitor: n.classList.contains('wmon') }
    })
    const more = el('span', 'wpmore', `+${nodes.length}`)
    pinned.prepend(more)
    const cs = getComputedStyle(pinned)
    const own = (parseFloat(cs.paddingLeft) || 0) + (parseFloat(cs.borderLeftWidth) || 0)
    const fit = window.Capsule.fitPins(sizes, room - own, { gap: parseFloat(cs.columnGap) || 8, plus: more.offsetWidth })
    nodes.forEach((n, i) => {
      n.classList.toggle('short', fit.modes[i] === 'icon')
      n.classList.toggle('gone', fit.modes[i] === 'hidden')
    })
    more.textContent = `+${fit.more}`
    more.hidden = !fit.more
    more.title = pinsNow.filter((_, i) => fit.modes[i] === 'hidden').map(w => WCards.titleOf(w, isleCtx())).join('、')
    pinned.classList.toggle('gone', fit.modes.every(m => m === 'hidden') && !fit.more)
  }
  // The most the pins may take (the line before them too): at the top, what
  // keeps the island in its window as it is (BASE less the 48 px roomFor
  // adds, so the window never grows for them); on the strip, the room up to
  // what stands beside her capsule, so nothing there moves for them: the
  // middle run where it is centred (her capsule before it), else the free
  // room between the runs (growNode, and the middle's margin where nothing
  // is centred). head: what the capsule takes besides its pins.
  function pinRoom(head) {
    if (home() !== 'taskbar') return BASE.width - 48 - head
    if (isSide() || !taskbarBuilt) return Infinity
    const pad = isCapsule() ? CAPSULE_X : 0
    const now = len(herSlot.getBoundingClientRect()) - 2 * pad
    const middle = zones.center
    const at = middle.getBoundingClientRect()
    const margin = parseFloat(getComputedStyle(middle).marginLeft) || 0
    const grow = len(growNode.getBoundingClientRect())
    const zone = Object.keys(zones).find(k => zones[k].contains(herSlot))
    const centred = middle.childElementCount > 0 && !barRest.classList.contains('tb-stretch')
    const free = zone === 'left' && centred ? (innerWidth - len(at)) / 2 - (lo(at) - margin) : zone === 'center' ? 2 * Math.min(margin, grow) : grow + (centred ? 0 : margin)
    return now + free - RUNS_APART - head
  }

  // How long something new takes to slide into the compact island.
  const TURN_MS = 320
  // Its readings at the closed island's end (or the taskbar's monitor part):
  // its setting, unless it is put on the desktop or on the taskbar (§7); the
  // taskbar her home, while the strip has that part (taken off it, #53, it
  // takes its turns in her capsule as any other).
  const isPinned = () => pinnedNow

  // A widget's line in the open island (the spec's §6, wcards.js rowHTML),
  // the one shown lit: small, its glyph, name and value in one line;
  // medium, its name over a second line (the weather's range, the monitor's
  // memory and network), a small curve where it has a series. One with a
  // large size has a ›: a press unfolds its card in place under it (one at
  // a time; again, folded), the › turned; one without opens its settings,
  // as before. A right press: its menu (openMenu). Each row's size is the
  // person's (widgetSize, from the menu); unset, medium where it has more
  // to say than its value, small else.
  const isleCtx = () => ({ lang, light: inkLight, detailed: isDetailed(), width: cardWidth(), hour: new Date().getHours(), now: Date.now(), all: allWidgets })
  // The card as wide as the open island's lines (the spec: about 270):
  // the island at its widest, less her room and its padding, the card's.
  const cardWidth = () => (isHome() ? OPEN_MAX_W - 124 - 22 : OPEN_BARE_MAX_W - 18 - 22) + 16 - 24
  function rowSize(w) {
    const set = config.widgetSize?.[w.id]
    if (set === 'S' || set === 'M') return set === 'M' && !Widgets.sizesOf(w).includes('m') ? 'S' : set
    const d = Widgets.detailOf(w)
    const more = d.extra || d.series || ['monitor', 'today', 'tokens', 'weather'].includes(WCards.kindOf(w))
    return more && Widgets.sizesOf(w).includes('m') ? 'M' : 'S'
  }
  function setRowSize(id, size) {
    window.pet.settings.set({ widgetSize: { ...(config.widgetSize || {}), [id]: size } })
  }
  // The widget whose card is unfolded, and since when (its unfolding carries
  // on, not begun again, as the island is drawn anew every second).
  let cardFor = null
  let cardSince = 0
  const CARD_MS = 340
  function showCard(id) {
    cardFor = id
    cardSince = Date.now()
    cardFresh = !!id
    update()
  }

  // A plugin's own view in its unfolded card (views.js; the spec's §4):
  // its view.html over the card, a medium one there (as wide as the card,
  // 128 px high), where its widget gives one; the card drawn from its data
  // until the view is ready, and whenever views may not show (nobody about,
  // animations off, the watchdog's while; and where this page cannot keep a
  // frame loaded as it draws its lines anew each second: moveBefore).
  const islandStyle = name => getComputedStyle(island).getPropertyValue(name).trim()
  const islandViews =
    window.Views && ROLE === 'island'
      ? (window.Views.island = window.Views.boot({
          ask: widget => window.pet.addons.view('data', widget),
          send: (widget, message) => window.pet.addons.view('send', widget, message),
          styleOf: w => ({ fg: islandStyle('--wg-fg'), muted: islandStyle('--wg-2'), color: WCards.colorOf(w, isleCtx()), font: getComputedStyle(expanded).fontFamily, fontDisplay: islandStyle('--wg-num') }),
          look: () => ({ theme: inkLight ? 'light' : 'dark', lang }),
          // Drawn again once what told it is done (pet:update, pet:quiet).
          changed: () => queueMicrotask(() => isOn() && update()),
          log: line => console.log(line),
          redrawn: true,
        }))
      : null

  function widgetRow(widget, isShown) {
    const ctx = isleCtx()
    const card = WCards.hasCard(widget)
    const open = card && cardFor === widget.id
    const size = rowSize(widget)
    const row = el('span', `wrow wr-${size}${isShown ? ' on' : ''}${open ? ' open' : ''}${card ? ' carded' : ''}`)
    row.dataset.widget = widget.id
    row.innerHTML = WCards.rowHTML(widget, size, ctx, { open, card })
    fillHer(row, 14)
    if (!open) return row
    const unfolded = el('span', 'icard')
    unfolded.dataset.card = widget.id
    unfolded.style.setProperty('--c', WCards.colorOf(widget, ctx))
    unfolded.innerHTML = WCards.cardHTML(widget, ctx, allWidgets)
    unfolded.classList.toggle('has-view', !!islandViews?.place(unfolded, `isle:${widget.id}`, widget, 'm'))
    fillHer(unfolded, 22)
    const into = Date.now() - cardSince
    if (into < CARD_MS) unfolded.style.animationDelay = `-${into}ms`
    else unfolded.classList.add('still')
    const pair = document.createDocumentFragment()
    pair.append(row, unfolded)
    return pair
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
    if (isHinting() && !isNudging()) {
      // Not connected: what it is for, and a row to go and connect. The
      // tools by name only in its tip and the welcome card: three of them
      // cut the line short in the open island, its meaning with them.
      const card = el('span', 'me')
      const top = el('span', 'head')
      top.append(el('span', 'title', t(lang, 'hint.title')))
      card.append(top, el('span', 'sub', t(lang, 'hint.sub')))
      const go = el('span', 'hintgo')
      go.dataset.connect = '1'
      go.append(el('span', '', t(lang, 'hint.go')), el('span', '', '›'))
      lines.append(card, go)
    } else lines.append(me)
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
    // The widgets, the one shown lit: under the sessions too, a session at
    // work not all the island says (2026-10-10). Her capsule's members in
    // their order (deskgrid.js placement); those on the taskbar or the
    // desktop are there, not here again (one widget, one place).
    if (listedNow.length && !(isNudging() && nudge.widget)) {
      const list = el('span', 'wlist')
      const shown = shownWidget()
      for (const w of listedNow) list.append(widgetRow(w, w === shown))
      lines.append(list)
    }
    // A widget that asked to open the island: what it says, over all else.
    if (isNudging() && nudge.widget) {
      const w = nudge.widget
      const { label, value: bare, detail } = Widgets.words(lang, w, isDetailed())
      // Its detail after the value, as in its line (the weather's range).
      const value = [bare, detail].filter(Boolean).join(' · ')
      // A private one's words stay off screen too, while the specifics do.
      const words = w.private && !isDetailed() ? '' : w.words
      const card = el('span', 'me')
      const top = el('span', 'head')
      top.insertAdjacentHTML('afterbegin', Widgets.iconHTML(w, lang))
      top.firstChild.style.color = ink(w.color) || hue().idle
      top.append(el('span', 'title', words || label || value))
      card.append(top, el('span', 'sub', words ? [label, value].filter(Boolean).join(' · ') : label ? value : ''))
      lines.replaceChildren(card)
    }
    // Not connected: at its foot, "don't remind me".
    if (isHinting() && !isNudging()) {
      const off = el('span', 'hintoff', t(lang, 'hint.off'))
      off.dataset.hintOff = '1'
      lines.append(off)
    }
    // On the taskbar, no taller than there is room for (openMaxH, #55): its
    // lines scroll (style.css), kept where they were scrolled to as it is
    // drawn anew (every second while a clock runs).
    const most = openMaxH()
    const old = expanded.querySelector(':scope > .lines')
    if (old && view === 'expanded') linesTop = old.scrollTop
    lines.style.maxHeight = Number.isFinite(most) ? `${Math.max(60, Math.floor(most - EXPANDED_PAD_Y))}px` : ''
    // A card's plugin view kept loaded as its card is drawn anew (views.js park, mount).
    islandViews?.park()
    expanded.replaceChildren(...(isHome() ? [] : [stillHer(48)]), lines)
    islandViews?.mount()
    // A card unfolded: the island at its widest, the card as wide as its
    // lines; a widget's menu open in it: as tall as the menu needs.
    expanded.classList.toggle('has-card', !!expanded.querySelector('.icard'))
    expanded.style.minHeight = menuRoom ? `${Math.min(menuRoom, most)}px` : ''
    expanded.classList.toggle('scrolls', lines.scrollHeight > lines.clientHeight + 1)
    if (linesTop) lines.scrollTop = linesTop
    // A card just unfolded below what is in sight: scrolled up into it, once
    // laid out (its height untransformed: it unfolds from its top, scaled).
    if (cardFresh) {
      cardFresh = false
      requestAnimationFrame(() => {
        const now = expanded.querySelector(':scope > .lines')
        const card = now?.querySelector('.icard')
        if (!card || now.scrollHeight <= now.clientHeight + 1) return
        const below = card.getBoundingClientRect().top + card.offsetHeight + 4 - now.getBoundingClientRect().bottom
        if (below > 1) now.scrollTop += below
        linesTop = now.scrollTop
      })
    }
  }

  // On to the next widget (or back), and its turn starts over.
  function turnWidget(step) {
    if (widgets.length < 2) return
    widgetAt = (widgetAt + step + widgets.length) % widgets.length
    clearTimeout(spinTimer)
    spinTimer = null
    update()
  }

  // Widgets take turns while the island is quiet and compact; not while
  // nobody is about (quiet), as the strip's carousels: each turn redraws
  // the closed island and its slide, which kept the renderer and the GPU
  // at about 80 wakes a second between them, against 18 held still.
  function spin() {
    const sec = Number(config.widgetSpin ?? 8)
    if (!(isQuiet() && !quiet && widgets.length > 1 && sec > 0 && view === 'compact')) {
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
  function placeHer({ width, height }) {
    if (!isHome()) return
    const s = her.style
    const portrait = (px, scale, left, top) => {
      Object.assign(s, { left: `${left}px`, top: `${top}px`, width: `${px}px`, height: `${px}px`, borderRadius: `${px / 2}px` })
      s.setProperty('--ring', hue()[now.mood])
      herSheet.style.transform = `translate(${-(CELL_W * scale - px) / 2}px, -2px) scale(${scale})`
    }
    if (view === 'compact') {
      // At a Mac's screen top, in the middle of the band's height.
      const [px, at] = home() === 'taskbar' ? [TASK_HEAD, (taskCompactH() - TASK_HEAD) / 2] : [HEAD, screenTop ? (screenTop.height - HEAD) / 2 : 6]
      const left = home() === 'taskbar' ? at : screenTop ? 10 : 6
      if (home() === 'taskbar') portrait(TASK_HEAD, TASK_HEAD * PER_PX, at, at)
      else portrait(HEAD, HEAD_SCALE, left, at)
      // The dot over her portrait's top-right, 2 px out; 1 px in on an
      // upright capsule, whose round top would cut it (the island clips).
      const [dx, dy] = isSide() ? [px - 10, 1] : [px - 7, -2]
      Object.assign(hintDot.style, { left: `${left + dx}px`, top: `${at + dy}px` })
    } else if (view === 'settings') {
      portrait(SET_HEAD, SET_HEAD_SCALE, 16, 13)
    } else {
      // Under a top bar, or from a side bar's capsule in its upper half, she
      // hangs from the island's top; else she stands on its foot. Beside a
      // right bar, at the island's right (it is mirrored).
      const hangs = tbEdge() === 'top' || (isSide() && !body.classList.contains('isl-low'))
      const top = hangs ? 12 : height - BODY_H - (view === 'ask' ? 10 : 6)
      const left = tbEdge() === 'right' ? width - BODY_W - 12 : 12
      Object.assign(s, { left: `${left}px`, top: `${top}px`, width: `${BODY_W}px`, height: `${BODY_H}px`, borderRadius: '0px' })
      herSheet.style.transform = `translate(0px, 0px) scale(${BODY_SCALE})`
    }
  }

  // Her mood's clip, in the portrait as standing; waiting by a prompt, a
  // reaction's while the island says something.
  // At rest she plays the idle row once, then holds its first frame this
  // long (as she does out on the desktop, pet.js): each frame is a frame of
  // the whole strip.
  const IDLE_HOLD_MS = 4000

  function animateHer() {
    if (!isHome()) return
    // Quiet (nobody at the keyboard or mouse): still, on her mood's first frame.
    // A reaction inside a turn (playReaction) over her mood while it plays.
    const reacts = reacting && Date.now() < reacting.until
    const name = quiet ? '' : view === 'ask' ? 'waiting' : isNudging() ? nudge.clip : reacts ? reacting.clip : MOOD_CLIP[now.mood]
    if (name && name === herClip) return
    clearTimeout(herTimer)
    herClip = name
    if (!name) return showFrame(herSheet, CLIPS[MOOD_CLIP[now.mood]].row, 0)
    const { row, frames, ms } = CLIPS[name]
    let frame = 0
    const step = () => {
      showFrame(herSheet, row, frame)
      frame = (frame + 1) % frames
      if (name === 'idle' && frame === 0) {
        // The row played through: back to its first frame, held.
        herTimer = setTimeout(() => {
          showFrame(herSheet, row, 0)
          herTimer = setTimeout(step, IDLE_HOLD_MS)
        }, ms)
      } else {
        herTimer = setTimeout(step, ms)
      }
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
    if (home() === 'taskbar' && isSide()) return null
    if (home() === 'taskbar') {
      const height = want.height + TASKBAR_SHADE + (view === 'settings' ? TASKBAR_H + TASKBAR_GAP : isCapsule() ? CAPSULE_Y : 0)
      if (height <= TASKBAR_H + TASKBAR_ROOM) return null
      return { width: innerWidth, height }
    }
    const width = want.width + 48
    const height = TOP + want.height + 24
    return width <= BASE.width && height <= BASE.height ? null : { width, height }
  }

  // The most the open island may be on the taskbar (zhoupengjie/Wakuwaku#55):
  // within the room kept above the strip, so the window never grows up for
  // it (a window growing up moves its top-left corner, which flashes; the
  // widgets' rows and the cards unfolded in them can take more than the
  // room), and within its screen should that be shorter (1080 px at 175%
  // are 617 for the 688 of strip and room); beside a side bar, from where
  // it stands to the window's far end, as the settings are. Its lines
  // scroll inside it past that (fillExpanded).
  function openMaxH() {
    if (home() !== 'taskbar') return Infinity
    if (isSide()) {
      const at = parseFloat(document.documentElement.style.getPropertyValue('--isl-y')) || 0
      return innerHeight - at - TASKBAR_GAP - TASKBAR_SHADE
    }
    const tall = Math.min(TASKBAR_H + TASKBAR_ROOM, (screen.height || Infinity) - TASKBAR_GAP)
    return tall - TASKBAR_SHADE - (isCapsule() ? CAPSULE_Y : 0)
  }

  // Opened up from the strip, or closed (part: island, pops): opening when
  // nothing was, her window over any other that lies over it.
  function openedUp(part, on) {
    const was = openUp.island || openUp.pops
    openUp[part] = on
    if (on && !was && isOn() && home() === 'taskbar') window.pet.raise()
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

  // fn once the island's size spring is over: its width's or height's
  // transitionend, or a little after the spring's length should none come
  // (the size unchanged, a spring cut short). Returns what calls it off.
  function afterSpring(fn, late = 0) {
    let timer
    const stop = () => {
      clearTimeout(timer)
      island.removeEventListener('transitionend', done)
    }
    const done = e => {
      if (e && (e.target !== island || (e.propertyName !== 'width' && e.propertyName !== 'height'))) return
      stop()
      fn()
    }
    island.addEventListener('transitionend', done)
    timer = setTimeout(done, SPRING_MS + 50 + late)
    return stop
  }

  // Collapsing on the taskbar, the open island's ground, edge and shadow
  // stay (style.css shrinking) until it has shrunk, then fade to the
  // capsule's over GROUND_FADE_MS: gone at once, a bare capsule's ground
  // left the open island's words over the desktop while it was still big
  // (zhoupengjie/Wakuwaku#16). Open again meanwhile, it has them at once.
  const GROUND_FADE_MS = 180
  let groundWait = null
  const stillMotion = () => matchMedia('(prefers-reduced-motion: reduce)').matches

  function holdGround(was, late) {
    if (home() === 'taskbar' && view === 'compact' && was && was !== 'compact') {
      groundWait?.()
      island.classList.add('shrinking')
      groundWait = afterSpring(fadeGround, late)
      return
    }
    if (view === 'compact' && home() === 'taskbar') return
    groundWait?.()
    groundWait = null
    island.classList.remove('shrinking')
    for (const a of island.getAnimations()) if (a.id === 'ground') a.cancel()
  }

  function fadeGround() {
    groundWait = null
    const held = getComputedStyle(island)
    const from = { offset: 0, backgroundColor: held.backgroundColor, boxShadow: held.boxShadow, backdropFilter: held.backdropFilter }
    island.classList.remove('shrinking')
    if (!stillMotion()) island.animate([from], { id: 'ground', duration: GROUND_FADE_MS, easing: 'ease-out' })
  }

  // --- Behind the island: a blur, or a deeper ground (zhoupengjie/Wakuwaku#20) ------
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
  const plateInset = radius => Math.max(1, (radius - DWM_CORNER) * (1 - Math.SQRT1_2))
  // While it springs to another size, this much further in, and as much
  // again as the spring may carry an edge past where it goes (style.css's
  // curve: some 7 % of the way it moves).
  const PLATE_SLACK = 3
  const SPRING_OVER = 0.07
  // What the plate was last told (where, as JSON; '': none), the wait for
  // the spring before it fades in, and the wait while it moves with it.
  let plateSent = ''
  let plateWait = null
  let plateMove = null
  // A maximized window on her display, at the top of the screen (island.rs
  // look_over_max; on the taskbar, its windows' list says).
  let overMaxTop = false

  function plateWanted() {
    if (!isOn() || now.plate !== true || quiet || view === 'settings' || body.classList.contains('pulling') || body.classList.contains('isl-stowed')) return false
    const { op, blur } = groundOf(lookNow())
    if (!(op < 1 && blur > 0)) return false
    return home() !== 'taskbar' || view !== 'compact'
  }

  // After the island is laid out; resized: from its box and corners then
  // (from, fromRadius) to its new size.
  function placePlate(resized, from, fromRadius) {
    if (!plateWanted()) return hidePlate(PLATE_OUT_MS)
    if (!plateSent) {
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
  // side there: anchor.js); on the taskbar from the strip's edge it grows
  // from, at her end's side (isl-right, isl-low).
  function rectAt(r, width, height) {
    if (home() !== 'taskbar') return Anchor.grown(r, width, height, anchor())
    const fromRight = tbEdge() === 'right' || (!isSide() && body.classList.contains('isl-right'))
    const fromFoot = tbEdge() === 'bottom' || (isSide() && body.classList.contains('isl-low'))
    return { x: fromRight ? r.right - width : r.left, y: fromFoot ? r.bottom - height : r.top, width, height }
  }

  // Changing size: under the smaller of its two boxes (where they meet),
  // each edge set in further by the slack and by as much as the spring may
  // carry that edge past where it goes (an edge it grows from stays put),
  // and the ground at once as solid as DEEP_OP (isl-firm): what the plate
  // no longer covers shows through no more than through the rim (at once,
  // the open island's words stood clear over the desktop as it closed;
  // the appearance session, 2026-10-14). Once it settles, the plate where
  // the island has come to and the ground back over DEEP_MS, together.
  function movePlate(from, fromRadius) {
    const to = rectAt(from, parseFloat(island.style.width), parseFloat(island.style.height))
    const x = Math.max(from.left, to.x)
    const y = Math.max(from.top, to.y)
    const box = { x, y, width: Math.min(from.right, to.x + to.width) - x, height: Math.min(from.bottom, to.y + to.height) - y }
    const inset = plateInset(Math.max(fromRadius || 0, parseFloat(island.style.borderRadius) || 0))
    const edge = moved => inset + PLATE_SLACK + SPRING_OVER * Math.abs(moved)
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

  function softenGround(ms) {
    if (!body.classList.contains('isl-firm')) return
    const firm = getComputedStyle(island).backgroundColor
    body.classList.remove('isl-firm')
    if (!stillMotion()) island.animate([{ offset: 0, backgroundColor: firm }], { id: 'firm', duration: ms, easing: 'ease' })
  }

  // Under the island as it is now (moved: told again, no fade).
  function showPlate(fade) {
    if (!plateWanted()) return hidePlate(0)
    const r = island.getBoundingClientRect()
    const inset = plateInset(parseFloat(getComputedStyle(island).borderTopLeftRadius) || 0)
    sendPlate({ x: r.left, y: r.top, width: r.width, height: r.height }, [inset, inset, inset, inset], inset, fade)
  }

  // The plate in box, its edges set in by sides (left, top, right, bottom;
  // never to nothing: it stays), the rim over the corners' band (rim wide);
  // faded in when new.
  function sendPlate(box, sides, rim, fade) {
    const across = Math.min(1, (box.width - 2) / (sides[0] + sides[2]))
    const down = Math.min(1, (box.height - 2) / (sides[1] + sides[3]))
    const [left, top, right, bottom] = [sides[0] * across, sides[1] * down, sides[2] * across, sides[3] * down]
    const rect = { x: box.x + left, y: box.y + top, width: box.width - left - right, height: box.height - top - bottom, vw: innerWidth, vh: innerHeight, dark: modeNow() !== 'light' }
    island.style.setProperty('--rim', `${rim.toFixed(2)}px`)
    body.classList.add('plated')
    const sent = JSON.stringify(rect)
    if (sent === plateSent) return
    const first = !plateSent
    plateSent = sent
    window.pet.plate(rect, first && !stillMotion() ? fade : 0)
  }

  function hidePlate(fade) {
    plateWait?.()
    plateWait = null
    plateMove?.()
    plateMove = null
    body.classList.remove('plated', 'isl-firm')
    if (!plateSent) return
    plateSent = ''
    window.pet.plate(null, stillMotion() ? 0 : fade)
  }

  // Resized for its room, the window moves the island in the page.
  window.addEventListener('resize', () => plateSent && !plateMove && showPlate(0))

  // Over a maximized window (one on her display, taskbar.rs "max"), a
  // see-through ground the plate does not blur goes to DEEP_OP while open,
  // over DEEP_MS (at once as it opens), and back as it closes or the window
  // is no longer maximized.
  const overMax = () => (home() === 'taskbar' ? taskWindows.some(a => (a.windows || []).some(w => w.max && !w.min)) : overMaxTop)
  window.pet.onOverMax(({ on } = {}) => {
    overMaxTop = on === true
    if (isOn()) setDeep(true)
  })

  function deepWanted() {
    if (!isOn() || view === 'settings' || (home() === 'taskbar' && view === 'compact')) return false
    const { op, blur } = groundOf(lookNow())
    return op < DEEP_OP && !(now.plate === true && blur > 0) && overMax()
  }

  function setDeep(fades) {
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
    if (home() !== 'island' || screenTop || config.islandHideUnder === false || !underTop || calledDown) return false
    if (view !== 'compact' || isHover || now.mood === 'waiting') return false
    // Down for a flow light's ring, as it came down when it opened by itself.
    return !pull && !isReaching && !isAbsorbing && !isComingHome && !isRinging()
  }

  function setStow() {
    if (!isOn()) return
    const on = stowWanted()
    const was = body.classList.contains('isl-stowed')
    body.classList.toggle('isl-stowed', on)
    // Main told whether it is up, and the closed island's size, where a
    // window under it is looked for (none on the taskbar).
    const { width, height } = home() === 'island' && !screenTop ? sizeOf('compact') : { width: 0, height: 0 }
    const sent = JSON.stringify([on, width, height])
    if (sent !== stowSent) {
      stowSent = sent
      window.pet.stow(on, width, height)
    }
    // The blur under it goes with it, and comes back once it is down, the
    // ground as solid as DEEP_OP till then, as when it opens.
    if (on !== was) placePlate(!on)
  }

  window.pet.onUnder(({ on } = {}) => {
    underTop = on === true
    setStow()
  })
  window.pet.onCalled(({ on } = {}) => {
    calledDown = on === true
    setStow()
  })

  function setSize({ width, height }) {
    island.style.width = `${width}px`
    island.style.height = `${height}px`
    // The taskbar's left end is square, and what grows up from it is rounded
    // above (on its Mica, her capsule and all it grows into round).
    // What grows from her end is flat where it meets the strip: the strip's
    // own end on the black one (its corners at the screen's edge square).
    const flat = { bottom: '22px 22px 0 0', top: '0 0 22px 22px', left: '0 22px 22px 0', right: '22px 0 0 22px' }
    island.style.borderRadius =
      home() === 'taskbar'
        ? view === 'compact'
          ? isCapsule()
            ? `${(isSide() ? width : height) / 2}px`
            : '0'
          : view === 'settings' || isCapsule()
            ? '22px'
            : flat[tbEdge()]
        : screenTop
          ? // Hung from a Mac's screen top: square where it meets the edge.
            `0 0 ${view === 'compact' ? 12 : 30}px ${view === 'compact' ? 12 : 30}px`
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
    if (isSide()) return sideCapsule(isHinting() || !!time())
    return { width: compactWidth, height: home() === 'taskbar' ? taskCompactH() : screenTop ? screenTop.height : 36 }
  }

  // --- The taskbar's right part ---------------------------------------------------

  // Start and the programs, the monitor, the tray's icons, the clock and the
  // settings. Its left edge stays where the compact island ends, so what
  // opens from the island grows up over it. The other sessions have no tags
  // on it: their windows' buttons go to them, and the island says how they
  // are (the user's call, 2026-10-12).
  const barRest = document.createElement('div')
  barRest.id = 'bar-rest'
  // Keys as Windows' own taskbar takes them: a screen reader in a page
  // reads it by its arrows (Narrator's scan mode) and the arrows never came
  // to her buttons; as an application they come (tried, 2026-10-14). Named
  // as the keyboard comes to it (keys, below).
  barRest.setAttribute('role', 'application')
  island.after(barRest)

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
      face.insertAdjacentHTML('afterbegin', Widgets.iconHTML(w, lang))
      face.firstChild.style.color = stripInk(w.color) || stripHue().idle
      const v = el('span', 'v', value)
      v.style.color = stripInk(w.color) || stripHue().idle
      if (Widgets.partsOf(w)) v.innerHTML = Widgets.partsHTML(w)
      face.append(Widgets.partsOf(w) ? '' : label, v)
      faces.push(face)
    }
    if (isPinned()) {
      const inRows = home() === 'taskbar'
      const readings = el('span', inRows ? 'face mon rows' : 'face mon')
      readings.innerHTML = inRows ? Widgets.rowsHTML(monitor, isSide()) : Widgets.partsHTML(monitor)
      faces.push(readings)
    }
    return faces
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
  // Task View, after Start as on Windows' own taskbar, while Windows has its
  // button on (taskbar:look taskView): a press opens it (Win+Tab). Windows'
  // own pictures (taskbar_task_view_icons: Settings' two, for a light strip
  // and a dark one, style.css .tv-light, .tv-dark); where Windows has none
  // (Windows 10), the Task View glyph of its icon font. One part with Start
  // in the layout (PART_NODES.start), as on Windows' they go together.
  const taskViewButton = el('span', 'tview')
  taskViewButton.dataset.bar = 'tasks'
  taskViewButton.hidden = true
  taskViewButton.innerHTML = '<i class="tv-glyph"></i>'
  Promise.resolve(window.pet.taskbar.taskViewIcons?.()).then(icons => {
    if (!icons?.light || !icons?.dark) return
    const img = (svg, cls) => `<img class="${cls}" alt="" draggable="false" src="data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}">`
    taskViewButton.innerHTML = img(icons.light, 'tv-light') + img(icons.dark, 'tv-dark')
  })
  const startPart = el('span', 'tstart')
  startPart.append(startButton, taskViewButton)
  const windowsBox = el('span', 'wins')
  const facesBox = el('span', 'faces')
  const trayBox = el('span', 'tray')
  // The keyboard: Caps Lock while it is on, and the input method's script
  // or picture (a press switches its script; a right press, the input
  // methods to pick from).
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
  // The Wi-Fi as Windows 11 draws it, as the volume is: all its arcs faint,
  // those its signal reaches solid over them (keys.bars, tasks.rs
  // wifi_bars: Windows' own 0 to 5 bars, to its four: the dot alone, one
  // arc, two, all); all of them solid while not known. It was always all.
  const WIFI = ['', '', '', '']
  const netBack = el('span', 'back')
  const netFront = el('span', 'front')
  netMark.append(netBack, netFront)
  // The volume as Windows draws it: the speaker's three waves faint, those up
  // to the volume solid over them; struck through, muted.
  const volMark = el('span', 'fi vol')
  const volBack = el('span', 'back', '')
  const volFront = el('span', 'front', '')
  volMark.append(volBack, volFront)
  // The battery after them, as Windows' own taskbar has it on a laptop (none
  // on a desktop), in Windows' own battery font (SysBatt Fluent Icons, read
  // from Windows: taskbar_battery_font) as its taskbar draws it: an outline,
  // a bolt in it while charging, a plug while plugged in and not charging (a
  // battery its laptop keeps at 80% too), and over it the level in tenths, in
  // the fills cut for the bolt or the plug; green while charging, yellow with
  // the energy saver on, red when critically low. Its percentage beside it as
  // Windows' "Battery percentage" has it. Without the font (before Windows
  // 11 has it), Segoe's battery glyphs, one for each tenth.
  const batMark = el('span', 'bat')
  batMark.hidden = true
  const batBase = el('span', 'fi base')
  const batFill = el('span', 'fi fill')
  const batGlyph = el('span', 'glyph')
  batGlyph.append(batBase, batFill)
  const batText = el('span', 'pct')
  batMark.append(batGlyph, batText)
  // Windows': each outline; the plain fills for tenths 1 to 10 after the
  // plain outline, the cut ones for 3 to 10 after the bolt's and the plug's
  // (1 and 2 do not reach the bolt or the plug, and are the plain ones).
  const SYSBATT = { plain: 0xf8d0, bolt: 0xf8db, plug: 0xf8e4 }
  const BATTERY = {
    level: ['', '', '', '', '', '', '', '', '', '', ''],
    charging: ['', '', '', '', '', '', '', '', '', '', ''],
    saver: ['', '', '', '', '', '', '', '', '', '', ''],
  }
  let sysBatt = false
  let batShown = null
  window.pet.taskbar
    .batteryFont()
    .then(bytes => {
      // Bytes as they are, or as numbers where the IPC had to send them so.
      const font = Array.isArray(bytes) ? new Uint8Array(bytes).buffer : bytes
      if (!(font instanceof ArrayBuffer) || !font.byteLength) return
      const face = new FontFace('SysBatt Fluent Icons', font)
      return face.load().then(() => {
        document.fonts.add(face)
        sysBatt = true
        showBattery(batShown)
      })
    })
    .catch(() => {})
  quickBox.append(netMark, volMark, batMark)
  // The wheel turned on it: the volume, as on Windows' own taskbar
  // (zhoupengjie/Wakuwaku#35): a notch a step up or down (2%, by Windows'
  // volume keys, its own flyout shown); a touchpad's or a fine wheel's
  // small turns gathered to a notch's worth. Not while the layout is edited.
  let wheelGathered = 0
  quickBox.addEventListener(
    'wheel',
    e => {
      if (editLayout) return
      e.preventDefault()
      const dy = e.deltaMode === 1 ? e.deltaY * 33 : e.deltaY
      wheelGathered = Math.abs(dy) >= 50 ? Math.sign(dy) * 100 : wheelGathered + dy
      const steps = Math.trunc(wheelGathered / 100)
      if (!steps) return
      wheelGathered -= steps * 100
      window.pet.taskbar.volume(-steps)
    },
    { passive: false },
  )
  function showNet(kind, bars) {
    const level = kind === 'wifi' && Number.isInteger(bars) ? Math.min(Math.max(bars, 1), 4) : 0
    netFront.textContent = level ? WIFI[level - 1] : NET[kind] || NET.other
    netBack.textContent = level ? NET.wifi : ''
    netBack.hidden = !level
  }
  showNet('other')
  function showVolume(v) {
    const [level, muted] = Array.isArray(v) ? v : [null, false]
    volFront.textContent = muted ? '' : level == null || level >= 67 ? '' : level >= 34 ? '' : level > 0 ? '' : ''
    // Muted, the waves' room is the struck-through speaker's, unseen.
    volBack.textContent = muted ? volFront.textContent : ''
    volBack.hidden = !!muted
    volMark.title = level == null ? '' : muted ? t(lang, 'taskbar.muted') : t(lang, 'taskbar.volume', { n: level })
  }
  // { percent, plugged, charging, saver, critical, left (s), shown } (widgets.rs battery_state), or none.
  function showBattery(b) {
    batShown = b
    batMark.hidden = !b
    if (!b) return
    const n = b.percent
    // In tenths as Windows has them: 96% full, 86% nine, and some left a
    // tenth (red when critically low); empty at none.
    const tenth = Math.max(0, Math.min(10, Math.ceil(n / 10)))
    const plugged = b.plugged || b.charging
    batMark.classList.toggle('sysbatt', sysBatt)
    if (sysBatt) {
      const outline = SYSBATT[b.charging ? 'bolt' : plugged ? 'plug' : 'plain']
      batBase.textContent = String.fromCharCode(outline)
      batFill.textContent = !tenth ? '' : String.fromCharCode(outline === SYSBATT.plain || tenth < 3 ? SYSBATT.plain + tenth : outline + tenth - 2)
    } else {
      batBase.textContent = (plugged ? BATTERY.charging : b.saver ? BATTERY.saver : BATTERY.level)[tenth]
      batFill.textContent = ''
    }
    // The fill's color, on battery alone but for charging's.
    batMark.dataset.tint = b.charging ? 'green' : plugged ? '' : b.critical ? 'red' : b.saver ? 'yellow' : ''
    batText.textContent = `${n}%`
    batText.hidden = !b.shown
    const left = Number.isFinite(b.left) ? Math.round(b.left / 60) : null
    const time = left == null ? '' : left >= 60 ? t(lang, 'taskbar.hm', { h: Math.floor(left / 60), m: left % 60 }) : t(lang, 'taskbar.m', { m: left })
    const said = plugged ? t(lang, 'taskbar.batteryPlugged', { n }) : time ? t(lang, 'taskbar.batteryLeft', { n, time }) : t(lang, 'taskbar.battery', { n })
    batMark.title = b.saver ? `${said}\n${t(lang, 'taskbar.batterySaver')}` : said
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
  // Show the desktop: a sliver at the screen's right edge, as Windows' own (Win+D).
  const deskEdge = el('span', 'tdesk')
  deskEdge.dataset.bar = 'desktop'
  // The room between the middle run and the right one: what centring the
  // middle moves it into (alignApps).
  const growNode = el('span', 'grow')
  let taskbarBuilt = false

  // --- The taskbar's layout (taskbarLayout) -------------------------------------------
  // Its parts in three runs, as a KDE panel has them: from the left end, in
  // the middle of the screen (moved left when crowded, alignApps), and up to
  // the right end; show-the-desktop's sliver last of all, always (Windows'
  // corner). Her island is one of the parts ('her'): a slot in its run with
  // the island laid over it (placeSlot), growing from it the way there is
  // room (body.isl-right: from its right edge, leftwards). Edited by
  // dragging the parts about on the strip itself (editTaskbar).
  const TB_PARTS = ['her', 'start', 'apps', 'monitor', 'tray', 'keys', 'quick', 'clock']
  const TB_RIGHT = ['monitor', 'tray', 'keys', 'quick', 'clock']
  // Those that may be taken off the strip (the rest stay: the ghost says so).
  const TB_OPTIONAL = ['monitor']
  const TB_PRESETS = {
    win11: { left: ['her'], center: ['start', 'apps'], right: TB_RIGHT },
    left: { left: ['her', 'start', 'apps'], center: [], right: TB_RIGHT },
    herRight: { left: ['start', 'apps'], center: [], right: [...TB_RIGHT, 'her'] },
  }
  const herSlot = el('span', 'tslot')
  // The tray's icons and the marks of what uses the microphone, camera or
  // location: one part.
  const trayPart = el('span', 'tpair')
  trayPart.append(trayBox, privBox)
  const PART_NODES = { her: herSlot, start: startPart, apps: windowsBox, monitor: facesBox, tray: trayPart, keys: keysBox, quick: quickBox, clock: clockBox }
  for (const [part, node] of Object.entries(PART_NODES)) node.dataset.part = part
  const zones = { left: el('span', 'tzone tz-left'), center: el('span', 'tzone tz-center'), right: el('span', 'tzone tz-right') }
  // While the layout is being edited: the one being made, not yet kept.
  let editLayout = null
  let arranged = ''
  let seatSent = null

  // The layout: the one being edited, or as kept, or before any as
  // taskbarAlign had it; each part once, one it lacks (new since) where the
  // default has it; the widgets put there (widgets.js layoutOf). No gear
  // (zhoupengjie/Wakuwaku#38): the settings are a press on her capsule, or
  // her menus', away, as Windows 11's taskbar has none; one kept in an old
  // layout is left out, as a part no longer there.
  // The monitor's part may be taken off (#53): dragged off the strip it is
  // gone from the layout, not put back as a part new since; a monitor put
  // there as a widget is that part (widgets.js monitorAsPart).
  function tbLayout(saved = editLayout || config.taskbarLayout) {
    const base = saved && typeof saved === 'object' ? saved : config.taskbarAlign === 'left' ? TB_PRESETS.left : TB_PRESETS.win11
    return Widgets.monitorAsPart(Widgets.layoutOf(base, TB_PARTS, TB_PRESETS.win11, TB_OPTIONAL))
  }
  // Whether the strip has the monitor's part.
  const hasMonitorPart = (layout = tbLayout()) => Object.values(layout).some(run => run.includes('monitor'))
  // Which preset its parts are as ('custom': none), whatever widgets are there.
  const presetOf = layout => {
    const parts = JSON.stringify(Widgets.partsAlone(tbLayout(layout)))
    return Object.keys(TB_PRESETS).find(k => JSON.stringify(tbLayout(TB_PRESETS[k])) === parts) || 'custom'
  }

  // The widgets put there, each its node, kept while it stays there (by its
  // id); dragged by its id as a part by its name while the layout is edited.
  const entryNodes = new Map()
  function entryNode(e) {
    let node = entryNodes.get(e.id)
    if (node && node._type !== e.type) {
      entryNodes.delete(e.id)
      node = null
    }
    if (!node) {
      node = el('span', `tw tw-${e.type}${Widgets.isGroup(e) ? ' tw-group' : ''}`)
      node.dataset.part = e.id
      node.dataset.entry = e.id
      node._type = e.type
      entryNodes.set(e.id, node)
    }
    node._entry = e
    // Several in one place are medium (a side bar's: small).
    if (Widgets.isSource(e) || Widgets.isGroup(e)) node.dataset.size = isSide() || (e.size === 'S' && !Widgets.isGroup(e)) ? 'S' : 'M'
    return node
  }

  // The parts and widgets into their runs, as the layout has them.
  function arrangeTaskbar() {
    const layout = tbLayout()
    const key = JSON.stringify(layout)
    if (key === arranged) return
    arranged = key
    for (const k of Object.keys(zones)) zones[k].replaceChildren(...layout[k].map(p => (typeof p === 'string' ? PART_NODES[p] : entryNode(p))))
    const ids = new Set(Object.values(layout).flatMap(run => run.filter(p => typeof p !== 'string').map(p => p.id)))
    for (const id of [...entryNodes.keys()]) if (!ids.has(id)) entryNodes.delete(id)
    // First at the strip's left end: her island at its very edge, as ever.
    herSlot.classList.toggle('first', layout.left[0] === 'her')
    // A run with a flexible spacer takes the room there is (the middle one
    // not in the middle of the screen then: its spacer has the say).
    const stretch = k => layout[k].some(p => p.type === 'flex')
    for (const k of Object.keys(zones)) zones[k].classList.toggle('stretch', stretch(k))
    barRest.classList.toggle('tb-stretch', Object.keys(zones).some(stretch))
    // What the strip shows takes no turns in the island too.
    takeWidgets(allWidgets)
  }

  // Each widget put there, as big as it is set and it has (widgets.js
  // sizesOf: a countdown is small alone): S its glyph and value; M its value
  // over its name, as the clock has the date under the time (wcards.js
  // faceHTML: the glyph white or black as the tray's icons, only ups and
  // downs in colour). One with nothing to say greyed, with its name; just
  // put there with its plugin starting, a shimmer for a few seconds; its
  // plugin stopped, an amber glyph and why. A plugin with several (stocks,
  // CI runs) shows them in turn. Several in one place: a carousel (in turn,
  // drawGroup) or a stack (the first, its others opened over it).
  const ENTRY_TURN_MS = 5000
  // What the widgets on the strip are drawn with (wcards.js ctx), as its
  // words are; what opens from them (a card, a stack, a menu), as Windows'
  // mode, as their ground is.
  const barCtx = () => ({ lang, light: isStripLight(), detailed: isDetailed(), width: STACK_CARD_W, hour: new Date().getHours(), now: Date.now() })
  const popCtx = () => ({ ...barCtx(), light: isLight() })
  // Placed this long ago at most, a widget with nothing yet is loading.
  const LOADING_MS = 20000
  const placedAt = new Map()
  // Why one shows nothing: its plugin wants its settings, stopped, is
  // starting (just put there), or nothing yet.
  function emptyState(m) {
    const p = m.type === 'plugin' ? pluginsSaid.list.find(x => x.id === m.of) : null
    if (p?.missing) return 'needs'
    if (p?.state === 'stopped') return 'error'
    const at = placedAt.get(Widgets.memberKey(m))
    if ((p?.state === 'running' || (m.of === 'monitor' && monitorWanted())) && at && Date.now() - at < LOADING_MS) return 'loading'
    return 'none'
  }
  const monitorWanted = () => allWidgets.some(w => w.id === 'monitor')
  // Its size as drawn: M where it has a medium size, else S.
  const sizeFor = (size, w) => (size === 'M' && (!w || Widgets.sizesOf(w).includes('m')) ? 'M' : 'S')

  function drawEntries() {
    const turn = Math.floor(Date.now() / ENTRY_TURN_MS)
    const detailed = isDetailed()
    const strip = barCtx()
    for (const node of entryNodes.values()) {
      // In its part's words (inkParts), dark or light by what is under it;
      // the strip's until it has been placed.
      const ctx = { ...strip, light: node._inked ? node.classList.contains('ink-light') : strip.light }
      const e = node._entry
      const name = Widgets.nameOf(lang, e, allWidgets, detailed)
      node.dataset.label = name
      if (Widgets.isGroup(e)) {
        drawGroup(node, e, ctx)
        continue
      }
      if (!Widgets.isSource(e)) continue
      const from = Widgets.sourcesOf(e, allWidgets)
      const w = from.length ? from[turn % from.length] : null
      // A plugin with nothing to say: maybe it wants its settings first.
      if (!w && e.type === 'plugin') askPlugins()
      const state = w ? '' : emptyState(e)
      const size = sizeFor(node.dataset.size, w)
      const drawn = JSON.stringify([size, name, w && [w.label, w.value, w.detail, w.color, w.private], state, ctx.light, detailed, lang])
      if (node._drawn === drawn) continue
      node._drawn = drawn
      node.classList.toggle('empty', !w)
      node.classList.toggle('S', size === 'S')
      const said = w ? Widgets.words(lang, w, detailed) : null
      node.title = w ? [said.label, said.value].filter(Boolean).join(' · ') : `${name} · ${t(lang, state === 'needs' ? 'taskbar.w.needs' : 'taskbar.w.none')}`
      node.innerHTML = w ? WCards.faceHTML(w, size, ctx) : WCards.emptyFaceHTML(e, name, size, state, ctx)
      fillHer(node, 16)
    }
    if (wgFor) drawWidgetPop()
    if (stackFor) drawStackPop()
    tickCarousels()
  }

  // Her face where a widget's glyph is hers (wcards.js .wher: Claude 今天),
  // still: one frame of her at rest, never played (a portrait that plays
  // keeps a timer going, playPortraits).
  function fillHer(root, px) {
    for (const slot of root.querySelectorAll('.wher:empty')) slot.append(herBadge(px))
  }
  function herBadge(px) {
    if (!spriteUrl) return face('idle', px)
    const scale = px * PER_PX
    const node = el('span', 'still-her badge')
    node.style.cssText = `width:${px}px;height:${px}px`
    const sheet = el('span', 'sheet')
    sheet.style.cssText = `transform:translate(${-(CELL_W * scale - px) / 2}px, -2px) scale(${scale})`
    sheetImage(sheet, spriteUrl)
    const clip = CLIPS[MOOD_CLIP.idle]
    if (clip) showFrame(sheet, clip.row, 0)
    node.append(sheet)
    return node
  }

  // --- Several in one place (the spec's §5) -------------------------------------------
  // A carousel: one place as wide as its widest member's medium size, its
  // members' widgets in turn, the next sliding up every 6 seconds (half a
  // second, the spec's curve); a column of dots at its right, the one shown
  // long. Not under the pointer (held), not while nobody is about (quiet),
  // never by itself with motion reduced (the wheel turns it then, by a
  // fade); news (a letter come) cuts in for a turn (widgets.js
  // carouselTurn). One timer for them all, only while one may turn.
  // A stack: the first shown (medium), two thin edges over it for the
  // others; a press opens them all above it (drawStackPop).
  const SLIDE_H = 40
  const carousels = new Map()
  let carouselTimer = 0
  const stillMotionNow = () => matchMedia('(prefers-reduced-motion: reduce)').matches

  function drawGroup(node, e, ctx) {
    const slides = Widgets.slidesOf(e, allWidgets)
    const size = node.dataset.size === 'S' ? 'S' : 'M'
    const face = s => (s.w ? WCards.faceHTML(s.w, sizeFor(size, s.w), ctx) : WCards.emptyFaceHTML(s.member, memberName(s.member), size, emptyState(s.member), ctx))
    const drawn = JSON.stringify([e.type, size, slides.map(s => (s.w ? [s.w.id, s.w.label, s.w.value, s.w.detail, s.w.color, s.w.private] : [Widgets.memberKey(s.member), emptyState(s.member)])), ctx.light, ctx.detailed, lang])
    // A plugin's member with nothing (its settings first?).
    if (slides.some(s => !s.w && s.member.type === 'plugin')) askPlugins()
    if (e.type === 'stack') {
      node.classList.toggle('open', stackFor === e.id)
      if (node._drawn === drawn) return
      node._drawn = drawn
      const top = slides[0]
      node.classList.toggle('empty', !top?.w)
      node.title = t(lang, 'taskbar.w.stackTip', { n: slides.length })
      node.innerHTML = top ? face(top) : ''
      fillHer(node, 16)
      return
    }
    // A carousel: its slides, then them again (it always turns upwards:
    // from the last to the first it goes on into the second run, then is put
    // back where the first run has it), and the dots.
    const state = carousels.get(e.id) || Widgets.carouselStart(Date.now())
    carousels.set(e.id, state)
    if (node._drawn !== drawn) {
      node._drawn = drawn
      // News: a mailbox in it whose unread grew since the last look.
      const was = node._words || new Map()
      const news = slides.findIndex(s => s.w && Widgets.newsIn(was.get(s.w.id), s.w))
      node._words = new Map(slides.filter(s => s.w).map(s => [s.w.id, s.w]))
      node._slides = slides
      const html = slides.map(s => `<span class="tws">${face(s)}</span>`).join('')
      node.innerHTML = `<span class="reel">${html}${slides.length > 1 ? html : ''}</span>${slides.length > 1 ? `<span class="dots">${slides.map(() => '<i></i>').join('')}</span>` : ''}`
      node.title = t(lang, 'taskbar.w.carouselTip', { n: slides.length })
      fillHer(node, 16)
      state.at = Math.min(state.at, Math.max(0, slides.length - 1))
      showSlide(node, state.at, false)
      if (news >= 0 && was.size) turnCarousel(e.id, news)
    }
  }

  // The slide shown: the reel moved (sliding, or at once), the dot lit.
  function showSlide(node, at, slide, by = 0) {
    const reel = node.querySelector('.reel')
    const n = node._slides?.length || 0
    if (!reel || !n) return
    for (const [i, dot] of [...node.querySelectorAll('.dots i')].entries()) dot.classList.toggle('on', i === at)
    const from = node._at ?? at
    node._at = at
    if (!slide || stillMotionNow()) {
      reel.style.transition = 'none'
      reel.style.transform = `translateY(${-at * SLIDE_H}px)`
      void reel.offsetHeight
      reel.style.transition = ''
      // By hand with motion reduced: a fade, not a slide (the spec's §9).
      if (slide) reel.animate([{ opacity: 0 }, { opacity: 1 }], { duration: 200, easing: 'ease-out' })
      return
    }
    // Upwards always: past the last into the second run, then back.
    const target = at > from || by > 0 ? (at > from ? at : at + n) : at
    reel.style.transform = `translateY(${-target * SLIDE_H}px)`
    if (target >= n) {
      clearTimeout(node._settle)
      node._settle = setTimeout(() => showSlide(node, at, false), CAROUSEL_SLIDE_MS + 40)
    }
  }
  const CAROUSEL_SLIDE_MS = 500

  // One turned now: by news (to it), or by hand (by), else its next.
  function turnCarousel(id, news = -1, by = 0) {
    const node = entryNodes.get(id)
    const state = carousels.get(id)
    const n = node?._slides?.length || 0
    if (!node || !state || n < 2) return
    const now = Date.now()
    const next = by ? Widgets.carouselStep(state, n, by, now) : Widgets.carouselTurn(news >= 0 ? state : { ...state, since: 0 }, n, now, news)
    carousels.set(id, next)
    if (next.at !== state.at) showSlide(node, next.at, true, by || 1)
    tickCarousels()
  }

  // The timer: to the next carousel due, while any may turn by itself.
  function tickCarousels() {
    clearTimeout(carouselTimer)
    carouselTimer = 0
    if (!isOn() || home() !== 'taskbar' || quiet || stillMotionNow() || editLayout || document.hidden) return
    const now = Date.now()
    let soonest = Infinity
    for (const [id, state] of carousels) {
      const node = entryNodes.get(id)
      if (!node?.isConnected || (node._slides?.length || 0) < 2) {
        if (!node?.isConnected) carousels.delete(id)
        continue
      }
      if (node._held) continue
      const due = Widgets.carouselDue(state, now)
      if (due <= 0) {
        const next = Widgets.carouselTurn(state, node._slides.length, now)
        carousels.set(id, next)
        if (next.at !== state.at) showSlide(node, next.at, true, 1)
        soonest = Math.min(soonest, Widgets.CAROUSEL_MS)
      } else soonest = Math.min(soonest, due)
    }
    if (soonest < Infinity) carouselTimer = setTimeout(tickCarousels, soonest + 20)
  }

  // Held under the pointer; let go, its turn starts over.
  barRest.addEventListener(
    'mouseenter',
    e => {
      const node = e.target.closest?.('.tw-carousel')
      if (node && e.target === node) node._held = true
    },
    true,
  )
  barRest.addEventListener(
    'mouseleave',
    e => {
      const node = e.target.closest?.('.tw-carousel')
      if (!node || e.target !== node) return
      node._held = false
      const state = carousels.get(node.dataset.entry)
      if (state) carousels.set(node.dataset.entry, { ...state, since: Date.now() })
      tickCarousels()
    },
    true,
  )
  // The wheel turns it by hand.
  barRest.addEventListener(
    'wheel',
    e => {
      const node = e.target.closest('.tw-carousel')
      if (!node || editLayout) return
      e.preventDefault()
      turnCarousel(node.dataset.entry, -1, e.deltaY > 0 ? 1 : -1)
    },
    { passive: false },
  )
  matchMedia('(prefers-reduced-motion: reduce)').addEventListener('change', () => tickCarousels())
  document.addEventListener('visibilitychange', () => tickCarousels())

  // A member's name, as put there (a plugin's, a widget's label as it was).
  const memberName = m => Widgets.nameOf(lang, { ...m, id: 'w0' }, allWidgets, isDetailed())
  // The name kept with someone else's plugin as it is put somewhere (her
  // own she knows by their ids): its own, as its widgets say it, for while
  // it has nothing to say.
  function ownName(id) {
    const name = t(lang, `w.ex.${id}`) === `w.ex.${id}` ? Widgets.pluginName(lang, id, allWidgets) : ''
    return name ? { name: [...name].slice(0, 40).join('') } : {}
  }

  // The plugins she runs as settings_get says, and other people's
  // (addons.rs), for which want their settings filled in first (missing: a
  // parameter they need) or were stopped: asked for while a plugin on the
  // strip has nothing to say, twice a minute at most, and once the layout
  // is kept.
  const pluginsSaid = { at: 0, list: [] }
  function askPlugins(now = false) {
    if (!now && Date.now() - pluginsSaid.at < 30000) return
    pluginsSaid.at = Date.now()
    window.pet.settings.get().then(got => tellPlugins(got && [...(got.plugins || []), ...(got.addons || [])]))
  }
  function tellPlugins(list) {
    if (!Array.isArray(list)) return
    pluginsSaid.list = list
    for (const node of entryNodes.values()) node._drawn = ''
    if (taskbarBuilt && home() === 'taskbar') drawEntries()
  }
  const needsSettings = e => e.type === 'plugin' && !!pluginsSaid.list.find(p => p.id === e.of)?.missing
  setInterval(() => {
    if (taskbarBuilt && home() === 'taskbar' && entryNodes.size) drawEntries()
  }, ENTRY_TURN_MS)

  // Her island over its slot, as wide as it is: from the strip's left, or,
  // its slot past the middle, from the right (what it grows into goes the
  // way there is room). Where it lies without OLED's shift, which moves
  // them both after. Her seat (where she is reached for, and flies back
  // to) told to main once it moves. (Named placeHer at first, it took the
  // name of the one that shapes her in the island, which was never called
  // again: she was an empty ring, 2026-10-12.)
  function placeSlot() {
    if (isSide()) return placeSideSlot()
    herSlot.style.height = ''
    herSlot.style.marginBottom = ''
    body.classList.remove('isl-low')
    const pad = isCapsule() ? CAPSULE_X : 0
    herSlot.style.width = `${onPixels(sizeOf('compact').width + 2 * pad)}px`
    herSlot.style.marginRight = `${pad ? 2 : 4}px`
    const shifted = (oledOn && parseFloat(document.documentElement.style.getPropertyValue('--oled-x'))) || 0
    const r = herSlot.getBoundingClientRect()
    const left = r.left - shifted + pad
    const atRight = (r.left + r.right) / 2 - shifted > innerWidth / 2
    body.classList.toggle('isl-right', atRight)
    document.documentElement.style.setProperty('--isl-x', `${onPixels(atRight ? innerWidth - (r.right - shifted - pad) : left)}px`)
    const seat = Math.round(left + taskCompactH() / 2)
    if (seat !== seatSent) {
      seatSent = seat
      window.pet.seat(seat)
    }
  }
  // On a side bar: the slot as tall as her upright capsule, from the bar's
  // top, or from its foot in its lower half (isl-low: what it grows into
  // goes up); her seat her portrait, along the bar from its top.
  function placeSideSlot() {
    const pad = isCapsule() ? CAPSULE_X : 0
    const cap = sizeOf('compact')
    herSlot.style.width = ''
    herSlot.style.marginRight = ''
    herSlot.style.height = `${onPixels(cap.height + 2 * pad)}px`
    herSlot.style.marginBottom = `${pad ? 2 : 4}px`
    const shifted = (oledOn && parseFloat(document.documentElement.style.getPropertyValue('--oled-y'))) || 0
    const r = herSlot.getBoundingClientRect()
    const top = r.top - shifted + pad
    const low = (r.top + r.bottom) / 2 - shifted > innerHeight / 2
    body.classList.remove('isl-right')
    body.classList.toggle('isl-low', low)
    document.documentElement.style.setProperty('--isl-y', `${onPixels(low ? innerHeight - (r.bottom - shifted - pad) : top)}px`)
    const seat = Math.round(top + cap.width / 2)
    if (seat !== seatSent) {
      seatSent = seat
      window.pet.seat(seat)
    }
  }
  // A run grown or shrunk moves her slot in it: the tray's icons come in a
  // few seconds after the strip is drawn, a program adds one, the monitor's
  // numbers get wider. Her island goes with it; it stayed where the slot had
  // been, over the monitor and the tray's chevron, until something drew the
  // strip anew (zhoupengjie/Wakuwaku#98).
  const slotWatch = new ResizeObserver(() => {
    if (taskbarBuilt && isOn() && home() === 'taskbar') placeSlot()
  })
  for (const zone of Object.values(zones)) slotWatch.observe(zone)

  // The settings beside a side bar, from her capsule's top (or foot, in the
  // bar's lower half), kept 8 px off the screen's ends.
  function placeSideSettings(height) {
    const at = parseFloat(document.documentElement.style.getPropertyValue('--isl-y')) || TASKBAR_GAP
    const y = Math.max(TASKBAR_GAP, Math.min(at, innerHeight - TASKBAR_GAP - height))
    document.documentElement.style.setProperty('--set-y', `${Math.round(y)}px`)
  }

  // The tray's icons as last sent ({ key, png, tip, exe }), and as drawn.
  let trayIcons = []
  let trayDrawn = ''
  let trayRectsSent = ''
  // The windows' buttons as last sent (taskbar.rs), and as drawn.
  let taskWindows = []
  let windowsDrawn = ''
  // When each window (by its handle) began flashing for attention: its
  // button is made anew with every change to the list, and the flashes
  // carry on from where they were (style.css .flash, --flash-delay)
  // instead of starting over.
  const flashSince = new Map()

  function fillTaskbar() {
    if (!taskbarBuilt) {
      // As wide as the strip: her island lies over its slot in it (placeSlot).
      barRest.style.left = '0px'
      barRest.replaceChildren(glide, zones.left, zones.center, growNode, zones.right, deskEdge, pop, wgPop, stackPop, imePop)
      taskbarBuilt = true
      trayDrawn = ''
      windowsDrawn = ''
    }
    arrangeTaskbar()
    drawEntries()
    // The programs' buttons as icons alone (taskbarButtons), or with their titles.
    barRest.classList.toggle('icons', isSide() || config.taskbarButtons !== 'labels')
    startButton.title = t(lang, 'taskbar.start')
    taskViewButton.title = t(lang, 'taskbar.taskView')
    taskViewButton.hidden = !look.taskView
    quickBox.title = t(lang, 'taskbar.quick')
    deskEdge.title = t(lang, 'taskbar.desktop')
    facesBox.replaceChildren(...widgetFaces())
    drawWindows()
    drawTray()
    tickClock()
    alignApps()
    placeSlot()
    snapStart()
    inkParts()
    // Editing: picker.js's popover over its widget, where it is now.
    if (editLayout) window.Picker?.relaid()
  }

  // The middle run in the middle of the screen, as Windows 11 has Start and
  // the programs' buttons (the default layout): moved along by its margin,
  // never back past where it starts nor into the right run (the room before
  // it is all it may take). Moved, it slides; what hangs over a button (its
  // windows' list) is placed again once it is there, and her island too.
  // The room always kept between the middle run and the last (the empty
  // .grow): crowded, the programs give way before it does (style.css).
  const RUNS_APART = 12

  function alignApps() {
    if (!taskbarBuilt) return
    fadeWins()
    const middle = zones.center
    // Along the bar: its left margin, a side bar's top one (the other none).
    const margin = isSide() ? 'marginTop' : 'marginLeft'
    middle.style[isSide() ? 'marginLeft' : 'marginTop'] = ''
    // Measured by the margin as it is now (it may be sliding), set to the one wanted.
    const target = parseFloat(middle.style[margin]) || 0
    let want = 0
    // Empty, it is still a place to drop a part while the layout is edited;
    // with a flexible spacer anywhere, the spacer places it.
    if ((middle.childElementCount || editLayout) && !barRest.classList.contains('tb-stretch')) {
      const now = parseFloat(getComputedStyle(middle)[margin]) || 0
      const start = middle.getBoundingClientRect()
      const length = len(start)
      const room = Math.max(0, len(growNode.getBoundingClientRect()) - RUNS_APART) + now
      // Where it lies without OLED's shift (which moves the bar after): the
      // margin taken from where it is drawn undid the shift, and the middle
      // ones stayed put across.
      const shifted = (oledOn && parseFloat(document.documentElement.style.getPropertyValue(isSide() ? '--oled-y' : '--oled-x'))) || 0
      want = Math.round(Math.max(0, Math.min(((isSide() ? innerHeight : innerWidth) - length) / 2 - (lo(start) - shifted - now), room)))
      // A side bar's middle run gives way when crowded (its programs then
      // scroll), so its own height and the room left are worked out from
      // what everything wants, not from what it has been squeezed to.
      if (isSide()) {
        // Its programs' column scrolls: its whole height, not what it shows.
        const natural = [...middle.children].reduce((a, n) => {
          const cs = getComputedStyle(n)
          return a + (n === windowsBox ? n.scrollHeight : n.offsetHeight) + (parseFloat(cs.marginTop) || 0) + (parseFloat(cs.marginBottom) || 0)
        }, 0)
        const others = [...barRest.children].filter(n => n !== middle && n !== growNode && getComputedStyle(n).position !== 'absolute')
        const used = others.reduce((a, n) => {
          const cs = getComputedStyle(n)
          return a + n.offsetHeight + (parseFloat(cs.marginTop) || 0) + (parseFloat(cs.marginBottom) || 0)
        }, 0)
        const free = barRest.clientHeight - (parseFloat(getComputedStyle(barRest).paddingTop) || 0) - used - natural - RUNS_APART
        want = Math.round(Math.max(0, Math.min((innerHeight - natural) / 2 - (start.top - shifted - now), free)))
      }
    }
    if (want !== Math.round(target)) middle.style[margin] = `${want}px`
  }
  zones.center.addEventListener('transitionend', e => {
    if (e.target !== zones.center || (e.propertyName !== 'margin-left' && e.propertyName !== 'margin-top')) return
    if (popFor) drawPop()
    if (glideOn?.isConnected) glideTo(glideOn, true)
    placeSlot()
    snapStart()
    inkParts()
    if (editLayout) window.Picker?.relaid()
    if (wgFor) drawWidgetPop()
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
  const GLIDE_ON = '.start, .tview, .win, .tout .ticon, .tchev, .tpriv, .kbd .ime, .tquick, .face.mon, .tw-widget, .tw-plugin, .tw-group, .tclock'
  const GLIDE_FAR = 360
  const glide = el('span', 'hdrop')
  let glideOn = null
  let glideTimer

  function glideTo(node, still = false) {
    clearTimeout(glideTimer)
    const bar = barRest.getBoundingClientRect()
    const r = node.getBoundingClientRect()
    // Along the bar its two ends slide (left and right, or a side bar's top
    // and bottom); across it, as the button is.
    const [from, to, crossFrom, crossSize] = isSide() ? ['top', 'bottom', 'left', 'width'] : ['left', 'right', 'top', 'height']
    const left = lo(r) - lo(bar)
    const right = hi(bar) - hi(r)
    const was = glideOn && glide.classList.contains('on') && glide.dataset.axis === from ? parseFloat(glide.style[from]) : null
    if (glide.dataset.axis !== from) {
      for (const p of ['left', 'right', 'top', 'bottom', 'width', 'height']) glide.style[p] = ''
      glide.dataset.axis = from
    }
    glide.style[crossFrom] = `${crossLo(r) - crossLo(bar)}px`
    glide.style[crossSize] = `${isSide() ? r.width : r.height}px`
    if (still || was === null || Math.abs(left - was) > GLIDE_FAR) {
      glide.style.transition = 'none'
      glide.style[from] = `${left}px`
      glide.style[to] = `${right}px`
      void glide.offsetWidth
      glide.style.transition = ''
    } else if (left !== was) {
      const lead = left > was ? to : from
      const trail = lead === to ? from : to
      glide.style.transition = `${lead} 0.22s cubic-bezier(0.3, 1.3, 0.5, 1), ${trail} 0.42s cubic-bezier(0.3, 1.45, 0.5, 1), opacity 0.16s ease, transform 0.3s cubic-bezier(0.3, 1.45, 0.5, 1)`
      glide.style[from] = `${left}px`
      glide.style[to] = `${right}px`
      glide.classList.remove('squash')
      void glide.offsetWidth
      glide.classList.add('squash')
    }
    glide.classList.add('on')
    glide.classList.toggle('ink-light', inkOf(node))
    glideOn = node
  }

  function glideOff() {
    clearTimeout(glideTimer)
    glideTimer = setTimeout(() => {
      glide.classList.remove('on', 'down')
      glideOn = null
    }, 90)
  }

  const isGlideOn = node => node?.matches?.(GLIDE_ON) && !node.closest('.wpop, .wgpop, .tfly, .wstk, .wmenu')
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
  window.addEventListener('resize', () => {
    if (!isOn() || home() !== 'taskbar') return
    alignApps()
    placeSlot()
  })

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

  // A side bar's programs crowded into their column: faded where more of
  // them lies past an end (12 px), as it scrolls.
  function fadeWins() {
    const side = isSide()
    const more = windowsBox.scrollHeight - windowsBox.clientHeight
    windowsBox.classList.toggle('fade-start', side && more > 0 && windowsBox.scrollTop > 0)
    windowsBox.classList.toggle('fade-end', side && more > 0 && windowsBox.scrollTop < more - 1)
  }
  windowsBox.addEventListener('scroll', fadeWins, { passive: true })

  function drawWindows() {
    const apps = orderedApps()
    const drawn = JSON.stringify([apps, lang])
    if (drawn === windowsDrawn) return
    windowsDrawn = drawn
    const now = Date.now()
    const flashing = new Set(apps.flatMap(a => (a.windows || []).filter(w => w.flash).map(w => w.id)))
    for (const id of [...flashSince.keys()]) if (!flashing.has(id)) flashSince.delete(id)
    for (const id of flashing) if (!flashSince.has(id)) flashSince.set(id, now)
    windowsBox.replaceChildren(
      ...apps.map(app => {
        const windows = app.windows || []
        const button = el('span', 'win')
        button.dataset.app = app.app
        // As Windows' own buttons say themselves to a screen reader.
        button.setAttribute('role', 'button')
        button.setAttribute('aria-label', t(lang, !windows.length ? 'taskbar.ariaPinned' : windows.length > 1 ? 'taskbar.ariaMany' : 'taskbar.ariaOne', { name: app.name || '', n: windows.length }))
        button.classList.toggle('front', windows.some(w => w.front))
        button.classList.toggle('min', windows.length > 0 && windows.every(w => w.min))
        const flashes = windows.filter(w => w.flash)
        button.classList.toggle('flash', flashes.length > 0)
        // From the longest-flashing window's start: a negative delay puts
        // the animation that far along (past its end: the last frame).
        if (flashes.length) button.style.setProperty('--flash-delay', `${-Math.max(...flashes.map(w => now - flashSince.get(w.id)))}ms`)
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
        if (windows.length) button.append(el('span', 'wt', windows.length === 1 ? windows[0].title || app.name : app.name))
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
  // Its room for the pointer down to the buttons (style.css .wreach, #56).
  const popReach = el('span', 'wreach')
  // { app, kind: 'list' | 'menu' } while open, and what it showed last.
  let popFor = null
  let popDrawn = ''
  let popTimer
  const appOf = key => taskWindows.find(a => a.app === key)
  // File Explorer's button, by its program: explorer.exe, the shell itself.
  const isExplorer = path => /(^|[\\/])explorer\.exe$/i.test(path || '')

  function openPop(key, kind) {
    if (wgFor) closeWidgetPop()
    if (imeOpen) closeImeList()
    clearTimeout(popTimer)
    popFor = { app: key, kind }
    drawPop()
    watchPresses()
    // Windows' End task as its setting is now (the look tells it only every
    // half minute): the menu drawn again should it have changed.
    if (kind === 'menu')
      window.pet.taskbar.endTask().then(on => {
        if (on === look.endTask) return
        look.endTask = on === true
        if (popFor?.kind === 'menu') drawPop()
      })
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
  // The widget on the taskbar whose card is open (openWidgetPop: its entry,
  // and in a group the widget picked); the stack opened (openStackPop); a
  // widget's menu on the strip (openMenu); the input methods' list open
  // (openImeList).
  let wgFor = null
  let wgPick = null
  let stackFor = null
  let imeOpen = false
  function watchPresses() {
    // picker.js: its capsule panel, its popover, closed by a press elsewhere too.
    const want = !!popFor || trayOpen || !!wgFor || !!stackFor || menuFor?.where === 'bar' || imeOpen || !!window.Picker?.watches()
    if (want === watching) return
    watching = want
    window.pet.taskbar.watch(want)
    openedUp('pops', want)
  }
  window.pet.onPress(at => {
    if (!isOn() || home() !== 'taskbar' || !at) return
    const over = document.elementFromPoint(at.x, at.y)
    if (popFor && !(over && (pop.contains(over) || windowsBox.contains(over)))) closePop()
    if (trayOpen && !(over && trayBox.contains(over))) openTray(false)
    if (wgFor && !(over && (wgPop.contains(over) || entryNodes.get(wgFor)?.contains(over) || stackPop.contains(over)))) closeWidgetPop()
    if (stackFor && !(over && (stackPop.contains(over) || entryNodes.get(stackFor)?.contains(over) || wgPop.contains(over) || wmenu.contains(over)))) closeStackPop()
    if (menuFor?.where === 'bar' && !(over && wmenu.contains(over))) closeMenu()
    if (imeOpen && !(over && (imePop.contains(over) || imeMark.contains(over)))) closeImeList()
  })

  // --- A widget on the taskbar, opened ------------------------------------------------
  // A press on one opens its card above it, as the clock's opens Windows'
  // calendar: what it says (its large size, wcards.js cardHTML, when it has
  // one; each of a plugin's widgets else), where it comes from, and its
  // settings, its mail, or taking it off the taskbar. A carousel's: the one
  // shown; a stack's line pressed: that one's. A press anywhere else closes
  // it (watchPresses). A right press: its menu (openMenu).
  const wgPop = el('div', 'wgpop')
  wgPop.hidden = true
  barRest.append(wgPop)
  // The input methods to pick from (openImeList), as a menu of the windows' kind.
  const imePop = el('div', 'wpop menu imepop')
  imePop.hidden = true
  barRest.append(imePop)
  // A card's width there, and the stack's (the spec: 300).
  const STACK_W = 300
  const STACK_CARD_W = 276

  function openWidgetPop(id, pick = null) {
    if (popFor) closePop()
    if (imeOpen) closeImeList()
    if (menuFor) closeMenu()
    wgFor = id
    wgPick = pick
    drawWidgetPop()
    watchPresses()
  }

  function closeWidgetPop() {
    wgFor = null
    wgPick = null
    wgPop.hidden = true
    watchPresses()
  }

  function drawWidgetPop() {
    const node = wgFor && entryNodes.get(wgFor)
    const e = node?._entry
    if (!e || !node.isConnected || editLayout || home() !== 'taskbar') return closeWidgetPop()
    const detailed = isDetailed()
    const ctx = popCtx()
    // A group's: the one picked, or the one its carousel shows.
    const group = Widgets.isGroup(e)
    const slides = group ? Widgets.slidesOf(e, allWidgets) : []
    const shown = group ? (wgPick ? slides.find(s => s.w?.id === wgPick) : slides[node._at || 0]) : null
    const from = group ? (shown?.w ? [shown.w] : []) : Widgets.sourcesOf(e, allWidgets)
    const first = from[0]
    const head = el('div', 'wghead')
    head.innerHTML = first ? WCards.glyphHTML(first, 'wghg') : WCards.memberGlyph(group ? shown?.member || e.items[0] : e)
    head.firstElementChild.style.color = first ? WCards.colorOf(first, ctx) : ''
    const kind = e.type === 'plugin' || first?.plugin ? 'plugin' : first?.from || 'widget'
    const title = first ? WCards.titleOf(first, ctx) : group ? memberName(shown?.member || e.items[0]) : Widgets.nameOf(lang, e, allWidgets, detailed)
    head.append(el('b', '', title), el('span', 'wgfrom', t(lang, `taskbar.w.from.${kind}`)))
    fillHer(head, 18)
    let rows
    if (from.length === 1 && WCards.hasCard(first)) {
      const card = el('div', 'wgcard')
      card.innerHTML = WCards.cardHTML(first, ctx, allWidgets)
      fillHer(card, 22)
      rows = [card]
    } else {
      rows = from.map(w => {
        const said = Widgets.words(lang, w, detailed)
        const row = el('div', 'wgrow')
        const value = el('b', '', said.value)
        if (Widgets.partsOf(w)) value.innerHTML = Widgets.partsHTML(w)
        // A plugin's several: each by its own label.
        row.append(...(from.length > 1 && said.label ? [el('span', '', said.label)] : []), value)
        return row
      })
    }
    if (!rows.length) rows.push(el('div', 'wgnone', t(lang, !group && needsSettings(e) ? 'taskbar.lib.needs' : 'taskbar.w.none')))
    const acts = el('div', 'wgacts')
    const button = (what, label) => {
      const b = el('button', 'wgbtn', label)
      b.type = 'button'
      b.dataset.wg = what
      return b
    }
    if (kind === 'mail') acts.append(button('mail', t(lang, 'taskbar.w.mail')))
    if (kind === 'plugin' || kind === 'builtIn') acts.append(button('settings', t(lang, 'taskbar.w.settings')))
    acts.append(button('remove', t(lang, 'taskbar.w.remove')))
    // Drawn anew only when it changed (the monitor's every 2 s): a button
    // replaced under the pointer would not be pressed.
    const drawn = JSON.stringify([wgFor, wgPick, from.map(w => [w.id, w.label, w.value, w.detail, w.private]), detailed, lang, ctx.light])
    if (drawn !== wgPop._drawn || wgPop.hidden) {
      wgPop._drawn = drawn
      wgPop.replaceChildren(head, ...rows, acts)
    }
    wgPop.classList.toggle('carded', rows[0]?.classList.contains('wgcard'))
    wgPop.hidden = false
    // Over the stack it was picked from, else over it.
    const over = stackFor === wgFor && !stackPop.hidden ? stackPop : node
    const at = besideOrOver(over, wgPop, isSide() ? 8 : 4)
    wgPop.style.left = at.left === undefined ? '' : `${at.left}px`
    wgPop.style.top = at.top === undefined ? '' : `${at.top}px`
    wgPop.classList.toggle('above-stack', over === stackPop)
  }

  // The layout changed by hand (a menu, a stack's line dragged): kept, or
  // into the one being edited.
  function keepLayout(layout) {
    if (editLayout) editLayout = layout
    else {
      config.taskbarLayout = layout
      window.pet.settings.set({ taskbarLayout: layout })
    }
    arranged = ''
    update()
    // picker.js: its cards' "已放" anew.
    window.Picker?.changed()
  }

  // Off the taskbar, and kept so.
  function removeEntry(id) {
    const layout = tbLayout()
    for (const k of Object.keys(layout)) layout[k] = layout[k].filter(p => Widgets.keyOf(p) !== id)
    if (wgFor === id) closeWidgetPop()
    if (stackFor === id) closeStackPop()
    keepLayout(layout)
  }

  // One of a group's off the taskbar: the rest stay (one left, alone).
  function removeMember(id, index) {
    closeWidgetPop()
    keepLayout(Widgets.dropMember(tbLayout(), id, index, newId))
  }

  wgPop.addEventListener('click', e => {
    const what = e.target.closest('[data-wg]')?.dataset.wg
    const entry = wgFor && entryNodes.get(wgFor)?._entry
    if (!what || !entry) return
    const group = Widgets.isGroup(entry)
    const w = group ? allWidgets.find(x => x.id === (wgPick || entryNodes.get(wgFor)._slides?.[entryNodes.get(wgFor)._at || 0]?.w?.id)) : null
    if (what === 'remove') {
      // picker.js: taken off with a way back (undo), off once it is nowhere.
      const at = group && w ? entry.items.findIndex(m => Widgets.sourcesOf(m, allWidgets).includes(w)) : -1
      if (window.Picker?.remove({ entry: entry.id, member: at })) return closeWidgetPop()
      if (!group) return removeEntry(entry.id)
      const index = w ? entry.items.findIndex(m => Widgets.sourcesOf(m, allWidgets).includes(w)) : -1
      return index >= 0 ? removeMember(entry.id, index) : removeEntry(entry.id)
    }
    closeWidgetPop()
    if (what === 'mail') return window.pet.openSettings('mail')
    openWidgetSettings(w || (entry.type === 'plugin' ? { plugin: entry.of } : { id: entry.of }))
  })

  // A widget's place in the settings: the mail page for an inbox's unread,
  // else its row on the plugins page, its own settings folded out (the
  // monitor's, a plugin's she runs).
  function openWidgetSettings(w) {
    if (w.from === 'mail') return window.pet.openSettings('mail')
    const key = w.plugin || w.id
    if (w.plugin || key === 'monitor') window.Settings?.openPlugin?.(key)
    window.pet.openSettings(key ? `widgets:${key}` : 'widgets')
  }
  wgPop.addEventListener('contextmenu', e => {
    e.preventDefault()
    e.stopPropagation()
  })

  // --- A stack, opened (the spec's §5) ------------------------------------------------
  // A glass card rising from it (300 wide, 8 px over the strip, round it,
  // kept on the screen): a line each, its glyph in a block of its colour,
  // its name, a sentence, its number; rising in 0.32 s, the lines 40 ms
  // apart. Its place keeps its lit ground while it is open. A line dragged
  // up or down goes there; dragged out of it, out of the stack (just after
  // it); pressed, its card. A press on its place again, or anywhere else,
  // closes it.
  const stackPop = el('div', 'wstk')
  stackPop.hidden = true
  barRest.append(stackPop)
  let stackOpenedAt = 0
  // Dragged this far out of it, a line leaves the stack.
  const STACK_OUT_PX = 28

  function openStackPop(id) {
    if (popFor) closePop()
    if (imeOpen) closeImeList()
    if (wgFor) closeWidgetPop()
    if (menuFor) closeMenu()
    stackFor = id
    stackOpenedAt = Date.now()
    stackPop._drawn = ''
    drawStackPop()
    watchPresses()
    for (const node of entryNodes.values()) node.classList.toggle('open', node.dataset.entry === id && node._entry?.type === 'stack')
  }

  function closeStackPop() {
    stackFor = null
    stackDrag = null
    stackPop.hidden = true
    for (const node of entryNodes.values()) node.classList.remove('open')
    if (wgFor && wgPick) closeWidgetPop()
    watchPresses()
  }

  function drawStackPop() {
    const node = stackFor && entryNodes.get(stackFor)
    const e = node?._entry
    if (!e || e.type !== 'stack' || !node.isConnected || editLayout || home() !== 'taskbar') return closeStackPop()
    if (stackDrag?.moved) return
    const ctx = popCtx()
    const slides = Widgets.slidesOf(e, allWidgets)
    const drawn = JSON.stringify([stackFor, slides.map(s => (s.w ? [s.w.id, s.w.label, s.w.value, s.w.detail, s.w.private] : Widgets.memberKey(s.member))), ctx.detailed, ctx.light, lang])
    if (drawn !== stackPop._drawn || stackPop.hidden) {
      stackPop._drawn = drawn
      const rows = slides.map(s => {
        const index = e.items.indexOf(s.member)
        const body = s.w
          ? WCards.stackRowHTML(s.w, WCards.titleOf(s.w, ctx), ctx)
          : `<span class="sgl">${WCards.memberGlyph(s.member)}</span><b class="sname">${WCards.esc(memberName(s.member))}</b><span class="sval"></span><span class="ssay">${WCards.esc(t(lang, 'taskbar.w.none'))}</span>`
        return `<div class="srow${s.w ? '' : ' empty'}" data-member="${index}" data-widget="${s.w ? WCards.esc(s.w.id) : ''}">${body}</div>`
      })
      stackPop.innerHTML = `<div class="shead"><b>${WCards.esc(t(lang, 'taskbar.stack.title'))}</b><span>${WCards.esc(t(lang, 'taskbar.stack.count', { n: slides.length }))}</span></div>${rows.join('')}`
      fillHer(stackPop, 16)
    }
    stackPop.hidden = false
    // Rising as it opens, carried on (not begun again) when drawn anew.
    const into = Date.now() - stackOpenedAt
    stackPop.classList.toggle('rising', into < 700)
    if (into < 700) {
      stackPop.style.animationDelay = `-${into}ms`
      for (const [i, row] of [...stackPop.querySelectorAll('.srow')].entries()) row.style.animationDelay = `${i * 40 - into}ms`
    }
    const at = besideOrOver(node, stackPop, 8)
    stackPop.style.left = at.left === undefined ? '' : `${at.left}px`
    stackPop.style.top = at.top === undefined ? '' : `${at.top}px`
  }

  // A line held: { row, member, x, y, moved, out }.
  let stackDrag = null
  stackPop.addEventListener('pointerdown', e => {
    const row = e.target.closest('.srow')
    if (!row || e.button !== 0) return
    stackDrag = { row, member: Number(row.dataset.member), widget: row.dataset.widget, x: e.clientX, y: e.clientY, moved: false, out: false }
    try {
      stackPop.setPointerCapture(e.pointerId)
    } catch {}
  })
  stackPop.addEventListener('pointermove', e => {
    const d = stackDrag
    if (!d) return
    if (!d.moved && Math.hypot(e.clientX - d.x, e.clientY - d.y) < 5) return
    d.moved = true
    d.row.classList.add('dragging')
    d.row.style.transform = `translate(${e.clientX - d.x}px, ${e.clientY - d.y}px)`
    const r = stackPop.getBoundingClientRect()
    d.out = e.clientX < r.left - STACK_OUT_PX || e.clientX > r.right + STACK_OUT_PX || e.clientY < r.top - STACK_OUT_PX || e.clientY > r.bottom + STACK_OUT_PX
    d.row.classList.toggle('out', d.out)
    d.row.dataset.say = d.out ? t(lang, 'taskbar.stack.out') : ''
  })
  stackPop.addEventListener('pointerup', e => {
    const d = stackDrag
    stackDrag = null
    if (!d) return
    const id = stackFor
    if (!d.moved) return d.widget ? openWidgetPop(id, d.widget) : undefined
    d.row.classList.remove('dragging', 'out')
    d.row.style.transform = ''
    if (d.out) return keepLayout(Widgets.takeOut(tbLayout(), id, d.member, newId))
    // Before the line it is let go over (after it past its middle).
    const rows = [...stackPop.querySelectorAll('.srow')].filter(r => r !== d.row)
    const below = rows.find(r => {
      const b = r.getBoundingClientRect()
      return e.clientY < b.top + b.height / 2
    })
    const members = [...new Set(rows.map(r => Number(r.dataset.member)))]
    const to = below ? members.indexOf(Number(below.dataset.member)) : members.length
    const order = [...members.slice(0, to), d.member, ...members.slice(to)]
    const at = order.indexOf(d.member)
    if (at !== d.member) {
      stackPop._drawn = ''
      keepLayout(Widgets.reorder(tbLayout(), id, d.member, at))
    } else drawStackPop()
  })
  stackPop.addEventListener('pointercancel', () => {
    if (stackDrag) {
      stackDrag.row.classList.remove('dragging', 'out')
      stackDrag.row.style.transform = ''
    }
    stackDrag = null
  })
  stackPop.addEventListener('contextmenu', e => {
    e.preventDefault()
    e.stopPropagation()
    const row = e.target.closest('.srow')
    if (row && stackFor) openMenu({ where: 'bar', entry: stackFor, member: Number(row.dataset.member), widget: row.dataset.widget || null }, row)
  })

  // --- A widget's menu (the spec's §10) -------------------------------------------------
  // A right press on a widget, on the strip or in the open island: its size
  // (small, medium, large; one it has not greyed, why under the pointer),
  // where it goes (the taskbar, the island; the desktop to come), with the
  // one beside it in one place (a stack, a carousel) or apart again, its
  // settings, off. menuFor: { where: 'bar' | 'isle', entry, member, widget }.
  const wmenu = el('div', 'wmenu')
  wmenu.hidden = true
  let menuFor = null

  // What a menu is for: its widget (the one picked, else the one shown),
  // the entry on the strip it is part of, and which of a group's it is.
  // On a group's place itself: the group (member -1), the widget it shows
  // and that one's member (shown) to take out.
  function menuTarget(m) {
    const node = m.entry ? entryNodes.get(m.entry) : null
    const entry = node?._entry || null
    const group = !!entry && Widgets.isGroup(entry)
    let w = m.widget ? allWidgets.find(x => x.id === m.widget) || null : null
    if (!w && entry) w = group ? node._slides?.[node._at || 0]?.w || null : Widgets.sourcesOf(entry, allWidgets)[0] || null
    const memberOf = x => (x ? entry.items.findIndex(mm => Widgets.sourcesOf(mm, allWidgets).includes(x)) : -1)
    const member = group ? (m.member ?? (m.widget ? memberOf(w) : -1)) : -1
    const slide = group && member < 0 ? node._slides?.[node._at || 0] : null
    const shown = slide ? entry.items.findIndex(x => Widgets.memberKey(x) === Widgets.memberKey(slide.member)) : -1
    return { node, entry, w, member, shown }
  }

  // The widget or group next to one in its run (after it, else before it).
  function neighbourOf(id) {
    const layout = tbLayout()
    const at = Widgets.findEntry(layout, id)
    if (!at) return null
    const run = layout[at.run]
    const fits = p => p && typeof p !== 'string' && (Widgets.isSource(p) || Widgets.isGroup(p))
    return [run[at.at + 1], run[at.at - 1]].find(fits) || null
  }

  // Why a size is not there, by kind (the spec's §4).
  function noSize(w, size) {
    const kind = WCards.kindOf(w)
    if (size === 'L' && kind === 'pomodoro') return t(lang, 'menu.why.pomodoro')
    if (kind === 'countdown') return t(lang, size === 'M' ? 'menu.why.countdownM' : 'menu.why.countdownL')
    return t(lang, size === 'M' ? 'menu.noMedium' : 'menu.noLarge')
  }

  function menuHTML(m) {
    const { node, entry, w: shownW, member, shown } = menuTarget(m)
    const esc = WCards.esc
    const group = !!entry && Widgets.isGroup(entry)
    // A group's own menu speaks for the group, not the one it shows.
    const w = group && member < 0 ? null : shownW
    const bar = m.where === 'bar'
    const ctx = bar ? popCtx() : isleCtx()
    const opt = (act, v, label, on, why = '') => `<button type="button" class="wmo${on ? ' on' : ''}${why ? ' off' : ''}" data-act="${act}" data-v="${v}"${why ? ` aria-disabled="true" title="${esc(why)}" data-why="${esc(why)}"` : ''}>${esc(label)}</button>`
    const item = (act, label, why = '', v = '') => `<button type="button" class="wmi${why ? ' off' : ''}" data-act="${act}" data-v="${esc(v)}"${why ? ` aria-disabled="true" title="${esc(why)}" data-why="${esc(why)}"` : ''}>${esc(label)}</button>`
    const line = (label, html) => `<div class="wmr"><span class="wml">${esc(label)}</span><span class="wms">${html}</span></div>`
    const name = w ? WCards.titleOf(w, ctx) : group && member < 0 ? Widgets.nameOf(lang, entry, allWidgets, isDetailed()) : entry ? memberName(group ? entry.items[member] : entry) : ''
    const glyph = w ? WCards.glyphHTML(w, 'wmg') : entry ? WCards.memberGlyph(group ? entry.items[Math.max(0, member)] : entry) : ''
    const parts = [`<div class="wmh" style="--c:${w ? WCards.colorOf(w, ctx) : 'currentColor'}">${glyph}<b>${esc(name)}</b></div>`]
    const sizes = w ? Widgets.sizesOf(w) : ['s', 'm']
    // Its size: on the strip S or M (never L: its card opens by a press);
    // in the island its row S or M, and L its card unfolded.
    if (!(group && member < 0)) {
      if (bar && !group) {
        const now = node?.dataset.size === 'S' || node?.classList.contains('S') ? 'S' : 'M'
        parts.push(line(t(lang, 'menu.size'), opt('size', 'S', t(lang, 'menu.s'), now === 'S') + opt('size', 'M', t(lang, 'menu.m'), now === 'M', sizes.includes('m') || !w ? '' : noSize(w, 'M')) + opt('size', 'L', t(lang, 'menu.l'), false, t(lang, 'menu.noLargeBar'))))
      } else if (!bar && w) {
        const now = rowSize(w)
        const open = cardFor === w.id
        parts.push(line(t(lang, 'menu.size'), opt('size', 'S', t(lang, 'menu.s'), now === 'S' && !open) + opt('size', 'M', t(lang, 'menu.m'), now === 'M' && !open, sizes.includes('m') ? '' : noSize(w, 'M')) + opt('size', 'L', t(lang, 'menu.l'), open, WCards.hasCard(w) ? '' : noSize(w, 'L'))))
      }
    }
    // Where it goes: the taskbar (only where she lives there), the island, the desktop.
    // Where it is now (one place, §7): the strip's own on the strip, her
    // capsule's in the open island (it lists those alone).
    if (w || entry) {
      const place = entry ? 'bar' : placeOf(w)
      const notBar = home() === 'taskbar' ? '' : t(lang, 'menu.notTaskbar')
      parts.push(line(t(lang, 'menu.where'), opt('where', 'bar', t(lang, 'menu.bar'), place === 'bar', place === 'bar' ? '' : notBar) + opt('where', 'isle', t(lang, 'menu.isle'), place === 'isle') + opt('where', 'desk', t(lang, 'menu.desk'), place === 'desk', window.pet.mac ? t(lang, 'menu.noDesk') : '')))
    }
    parts.push('<div class="wmsep"></div>')
    // With others in one place, or apart again.
    if (bar && entry) {
      if (group && member < 0) {
        parts.push(line(t(lang, 'menu.group'), opt('regroup', 'carousel', t(lang, 'menu.carousel'), entry.type === 'carousel') + opt('regroup', 'stack', t(lang, 'menu.stack'), entry.type === 'stack')))
        // The one it shows, out of it.
        if (shown >= 0) parts.push(item('takeOut', t(lang, 'menu.takeOut', { name: shownW ? WCards.titleOf(shownW, ctx) : memberName(entry.items[shown]) }), '', String(shown)))
        parts.push(item('ungroup', t(lang, 'menu.ungroup')))
      } else if (group) {
        parts.push(item('takeOut', t(lang, entry.type === 'stack' ? 'menu.outOfStack' : 'menu.outOfCarousel'), '', String(member)))
      } else {
        // The one beside it: a group it may join (its kind kept), or both
        // made one of either kind; a single one by its name (天气).
        const next = neighbourOf(entry.id)
        if (!next) {
          const why = t(lang, 'menu.noNeighbour')
          parts.push(item('group', t(lang, 'menu.stackWithNone'), why, 'stack'), item('group', t(lang, 'menu.carouselWithNone'), why, 'carousel'))
        } else if (Widgets.isGroup(next)) {
          const n = next.items.length
          const other = next.type === 'stack' ? 'carousel' : 'stack'
          const full = n >= 8 ? t(lang, 'menu.groupFull') : ''
          parts.push(item('join', t(lang, `menu.join.${next.type}`, { n }), full), item('group', t(lang, `menu.${other}WithGroup.${next.type}`), '', other))
        } else {
          const w = Widgets.sourcesOf(next, allWidgets)[0]
          const name = w ? WCards.titleOf(w, ctx) : memberName(next)
          parts.push(item('group', t(lang, 'menu.carouselWith', { name }), '', 'carousel'), item('group', t(lang, 'menu.stackWith', { name }), '', 'stack'))
        }
      }
    }
    // Into a board on the desktop (§7): each there is (three at most), or a
    // new one; never a group, never on a Mac.
    if (window.pet.mac) parts.push(item('board', t(lang, 'menu.board'), t(lang, 'menu.noDesk')))
    else if (group && member < 0) parts.push(item('board', t(lang, 'menu.board'), t(lang, 'desk.noStackInBoard')))
    else if (w || entry) {
      const it = menuItemOf(w, entry, member)
      const inBoard = b => !!it && b.items.some(x => DeskGrid.keyOf(x) === keyFor(it))
      for (const b of deskBoards().slice(0, 3)) {
        if (inBoard(b)) continue
        parts.push(item('board', t(lang, 'desk.toBoard', { name: boardName(b) }), it && DeskGrid.boardFirst(b, boardSize(it)) ? '' : t(lang, 'desk.boardFull'), b.id))
      }
      parts.push(item('board', t(lang, 'desk.newBoard'), '', 'new'))
    }
    if (w) parts.push(item('settings', t(lang, 'menu.settings')))
    // picker.js: in the open island, a member's way of showing in her
    // capsule (under its name), and out of it (移出胶囊).
    const isle = !bar && w ? window.Picker?.isleMenu?.(w, { opt, line }) : null
    if (isle) parts.splice(1, 0, ...isle.lines)
    parts.push(item('remove', isle?.remove || t(lang, bar && entry ? 'menu.remove' : 'capsule.out')))
    parts.push('<div class="wmwhy" hidden></div>')
    return parts.join('')
  }

  function openMenu(m, near, at = null) {
    if (popFor) closePop()
    if (imeOpen) closeImeList()
    if (wgFor && m.where === 'bar') closeWidgetPop()
    menuFor = m
    wmenu.innerHTML = menuHTML(m)
    fillHer(wmenu, 16)
    wmenu.hidden = false
    wmenu.classList.toggle('isle', m.where === 'isle')
    if (m.where === 'bar') {
      if (wmenu.parentElement !== barRest) barRest.append(wmenu)
      const over = stackFor && !stackPop.hidden && stackPop.contains(near) ? stackPop : near
      const pos = besideOrOver(over, wmenu, 8)
      wmenu.style.left = pos.left === undefined ? '' : `${pos.left}px`
      wmenu.style.top = pos.top === undefined ? '' : `${pos.top}px`
      wmenu.style.bottom = ''
      wmenu.classList.toggle('above-stack', over === stackPop)
    } else {
      // In the open island, at the pointer, kept inside it; the island as
      // tall as it needs to hold it (sizeOf follows #island-expanded).
      if (wmenu.parentElement !== island) island.append(wmenu)
      const box = island.getBoundingClientRect()
      const x = Math.max(8, Math.min((at?.x ?? box.left + 20) - box.left, box.width - wmenu.offsetWidth - 8))
      const y = Math.max(8, (at?.y ?? box.top + 20) - box.top)
      wmenu.style.left = `${Math.round(x)}px`
      wmenu.style.top = `${Math.round(y)}px`
      wmenu.style.bottom = ''
      menuRoom = Math.ceil(y + wmenu.offsetHeight + 10)
      update()
    }
    watchPresses()
  }

  function closeMenu() {
    if (!menuFor) return
    const wasIsle = menuFor.where === 'isle'
    menuFor = null
    wmenu.hidden = true
    wmenu.querySelector('.wmwhy')?.setAttribute('hidden', '')
    watchPresses()
    if (wasIsle) {
      menuRoom = 0
      update()
    }
  }
  // The open island's height for its menu (0: none open).
  let menuRoom = 0

  // A greyed one under the pointer: why, at the menu's foot.
  wmenu.addEventListener('mouseover', e => {
    const why = wmenu.querySelector('.wmwhy')
    const off = e.target.closest('[data-why]')
    if (!why) return
    why.hidden = !off
    if (off) why.textContent = off.dataset.why
  })
  wmenu.addEventListener('contextmenu', e => {
    e.preventDefault()
    e.stopPropagation()
  })
  wmenu.addEventListener('click', e => {
    e.stopPropagation()
    const b = e.target.closest('[data-act]')
    if (!b || b.classList.contains('off') || !menuFor) return
    const m = menuFor
    const { entry, w, member } = menuTarget(m)
    const v = b.dataset.v
    closeMenu()
    switch (b.dataset.act) {
      case 'size':
        if (m.where === 'bar' && entry) return keepLayout(withEntry(entry.id, p => ({ ...p, size: v })))
        if (!w) return
        if (v === 'L') return showCard(cardFor === w.id ? null : w.id)
        if (cardFor === w.id) showCard(null)
        return setRowSize(w.id, v)
      case 'where':
        if (v === 'bar' && w && !entry) return placeWidget(menuItemOf(w), 'bar')
        if (v === 'isle' && entry) return backToIsland(w, entry, member)
        if (v === 'isle' && w) return placeWidget(menuItemOf(w), 'isle')
        if (v === 'desk') return toDesk(w, entry, member)
        return
      case 'board': {
        const it = menuItemOf(w, entry, member)
        // Off the strip (its group, or its place) and into the board.
        return it && toBoard(it, v)
      }
      case 'group': {
        const next = entry && neighbourOf(entry.id)
        return next && keepLayout(Widgets.groupWith(tbLayout(), next.id, entry.id, v, newId))
      }
      // Into the group beside it, as its last; its kind kept.
      case 'join': {
        const next = entry && neighbourOf(entry.id)
        return next && Widgets.isGroup(next) && keepLayout(Widgets.groupWith(tbLayout(), entry.id, next.id, next.type, newId))
      }
      case 'regroup':
        return entry && keepLayout(Widgets.regroup(tbLayout(), entry.id, v))
      case 'ungroup':
        if (stackFor === entry?.id) closeStackPop()
        return entry && keepLayout(Widgets.ungroup(tbLayout(), entry.id, newId))
      case 'takeOut':
        return entry && keepLayout(Widgets.takeOut(tbLayout(), entry.id, Number(v), newId))
      case 'settings':
        return w && openWidgetSettings(w)
      // picker.js: its way of showing in her capsule.
      case 'cmode':
        return w && window.Picker?.setMode(w, v)
      case 'remove':
        // picker.js: taken off, with a way back (undo), and off once it is nowhere.
        if (window.Picker?.remove(m.where === 'bar' && entry ? { entry: entry.id, member } : { widget: w })) return
        if (m.where === 'bar' && entry) return Widgets.isGroup(entry) && member >= 0 ? removeMember(entry.id, member) : removeEntry(entry.id)
        // Out of her capsule with no picker to do it: off (put nowhere now).
        if (w) return window.pet.settings.set({ widgetsOff: [...new Set([...(config.widgetsOff || []), w.id])] })
    }
  })

  // The layout with one entry changed.
  function withEntry(id, change) {
    const layout = tbLayout()
    const at = Widgets.findEntry(layout, id)
    if (at) layout[at.run][at.at] = change(at.entry)
    return layout
  }

  // Put on the taskbar: at the right run's start (as the library's press
  // puts one), medium; a plugin's widget as its plugin (all of them, in turn).
  function putOnBar(w) {
    const source = w.plugin ? { type: 'plugin', of: w.plugin } : { type: 'widget', of: w.id }
    if (Widgets.shownOn(tbLayout(), allWidgets).has(w.id)) return
    const named = w.private ? {} : w.plugin ? ownName(w.plugin) : { name: [...String(Widgets.words(lang, w).label)].slice(0, 40).join('') }
    const entry = { id: newId(), ...source, size: Widgets.sizesOf(w).includes('m') ? 'M' : 'S', ...(w.icon in Widgets.ICONS ? { icon: w.icon } : {}), ...named }
    const layout = tbLayout()
    layout.right.unshift(entry)
    placedAt.set(Widgets.memberKey(source), Date.now())
    keepLayout(layout)
  }

  // Where a widget ({ id }) or a plugin she runs ({ plugin }) is, the
  // taskbar, the island or the desktop, and put there (a menu's 放到, the
  // desktop's asks). On the taskbar alone or in a group; put there, at the
  // right run's start; back in the island, out of wherever it was.
  const keyFor = item => (item.plugin ? `plugin:${item.plugin}` : `widget:${item.id}`)
  // The monitor on the strip is its part (#53): there while her home is the taskbar.
  const isMonitorItem = item => !item.plugin && item.id === 'monitor'
  // A widget as placeWidget takes it: as the desktop has it (its plugin's
  // entry, or its own), else its plugin's, else its own; a member of the
  // strip's (a group's by its index) as it is there.
  function menuItemOf(w, entry = null, member = -1) {
    const m = entry ? (Widgets.isGroup(entry) ? entry.items[member] : entry) : null
    if (m) return m.type === 'plugin' ? { plugin: m.of } : { id: m.of }
    if (!w) return null
    if (w.plugin && !DeskGrid.has(config.deskLayout, { id: w.id })) return { plugin: w.plugin }
    return { id: w.id }
  }
  // Where a widget is now: on the desktop (alone, stacked, in a board), on
  // the strip (the taskbar her home), else in the island.
  function placeOf(w) {
    if (!w) return 'isle'
    if (DeskGrid.has(config.deskLayout, { id: w.id }) || (w.plugin && DeskGrid.has(config.deskLayout, { plugin: w.plugin }))) return 'desk'
    if (placedOut.has(w.id) || (w.id === 'monitor' && home() === 'taskbar' && hasMonitorPart())) return 'bar'
    return 'isle'
  }
  function whereOf(item) {
    if (DeskGrid.has(config.deskLayout, item)) return 'desk'
    if (isMonitorItem(item) && home() === 'taskbar' && hasMonitorPart()) return 'bar'
    const keys = new Set(Object.values(tbLayout()).flatMap(run => run.filter(p => typeof p !== 'string').flatMap(Widgets.membersOf).map(Widgets.memberKey)))
    return keys.has(keyFor(item)) ? 'bar' : 'isle'
  }
  // For the Plugins page (settings.js): where one is put now, as placing.js
  // whereIs says (the strip, the desktop, a board by its name, her capsule
  // while it is on, or nowhere: null), a plugin's widgets standing on the
  // desktop by themselves counted as it.
  function placeNow(item, on) {
    const keys = [keyFor(item), ...(item.plugin ? allWidgets.filter(w => w.plugin === item.plugin).map(w => `widget:${w.id}`) : [])]
    const p = window.Placing.whereIs({ keys, on, taskbar: home() === 'taskbar' ? tbLayout() : null, desk: config.deskLayout, home: home() })
    return p.board ? { at: 'board', name: boardName(p.board) } : { at: p.at }
  }
  function placeWidget(item, where) {
    const key = keyFor(item)
    // Off the desktop as it goes elsewhere (the desktop's pages keep the places).
    if (where !== 'desk' && DeskGrid.has(config.deskLayout, item)) deskOff([item])
    if (where === 'bar') {
      if (whereOf(item) === 'bar') return
      const layout = tbLayout()
      // The monitor: its part, at the right run's start.
      if (isMonitorItem(item)) {
        for (const k of Object.keys(layout)) layout[k] = layout[k].filter(p => p !== 'monitor')
        layout.right.unshift('monitor')
        return keepLayout(layout)
      }
      const w = item.plugin ? null : allWidgets.find(x => x.id === item.id)
      const source = item.plugin ? { type: 'plugin', of: item.plugin } : { type: 'widget', of: item.id }
      const named = w && !w.private ? { name: [...String(Widgets.words(lang, w).label)].slice(0, 40).join('') } : item.plugin ? ownName(item.plugin) : {}
      const icon = (w?.icon || item.icon) in Widgets.ICONS ? { icon: w?.icon || item.icon } : {}
      layout.right.unshift({ id: newId(), ...source, size: !w || Widgets.sizesOf(w).includes('m') ? 'M' : 'S', ...icon, ...named })
      placedAt.set(key, Date.now())
      return keepLayout(layout)
    }
    if (where !== 'isle' && where !== 'desk') return
    let layout = tbLayout()
    let off = false
    // The monitor's part off the strip: always for the desktop (one place,
    // whatever her home); for the island while the taskbar is her home (in
    // the island's home the strip is not there, its part left for it).
    if (isMonitorItem(item) && (where === 'desk' || home() === 'taskbar') && hasMonitorPart(layout)) {
      for (const k of Object.keys(layout)) layout[k] = layout[k].filter(p => p !== 'monitor')
      off = true
    }
    const dropped = dropFromBar(layout, key)
    layout = dropped.layout
    off ||= dropped.off
    // The taskbar's layout kept only when it was on it (not written as a
    // preset's for one put on the desktop from the capsule).
    if (off || where === 'isle') keepLayout(layout)
    if (where === 'desk') return putOnDesk(item)
    if (item.id && (config.widgetsOff || []).includes(item.id)) window.pet.settings.set({ widgetsOff: config.widgetsOff.filter(x => x !== item.id) })
  }

  // A widget or plugin (by its key) off a taskbar layout: alone, or out of
  // its group (one left: alone).
  function dropFromBar(layout, key) {
    let off = false
    for (const k of Object.keys(layout))
      for (const p of [...layout[k]]) {
        if (typeof p === 'string') continue
        if (Widgets.isGroup(p)) {
          const at = p.items.findIndex(m => Widgets.memberKey(m) === key)
          if (at >= 0) {
            layout = Widgets.dropMember(layout, p.id, at, newId)
            off = true
          }
        } else if (Widgets.isSource(p) && Widgets.memberKey(p) === key) {
          layout[k] = layout[k].filter(x => x !== p)
          off = true
        }
      }
    return { layout, off }
  }
  // Off the taskbar, these (a desktop's board took them, desk.js): the
  // monitor its part. Kept only if any was there.
  function offBar(items) {
    let layout = tbLayout()
    let off = false
    for (const item of items) {
      if (isMonitorItem(item) && hasMonitorPart(layout)) {
        for (const k of Object.keys(layout)) layout[k] = layout[k].filter(p => p !== 'monitor')
        off = true
      }
      const dropped = dropFromBar(layout, keyFor(item))
      layout = dropped.layout
      off ||= dropped.off
    }
    if (off) keepLayout(layout)
  }

  // --- Boards on the desktop (§7 工作台) ---------------------------------------------
  // Their names as the desktop's pages give them: their own, else 工作台,
  // numbered when several have none.
  const deskBoards = () => (Array.isArray(config.deskLayout) ? config.deskLayout : []).filter(DeskGrid.isBoard)
  function boardName(b) {
    if (b.name) return b.name
    const unnamed = deskBoards().filter(x => !x.name)
    return unnamed.length > 1 ? t(lang, 'desk.boardN', { n: unnamed.findIndex(x => x.id === b.id) + 1 }) : t(lang, 'desk.board')
  }
  // The size a widget takes in a board: small where it has it.
  function boardSize(item) {
    const m = item.plugin ? { type: 'plugin', of: item.plugin } : { type: 'widget', of: item.id }
    const sizes = DeskGrid.sizesOfEntry(m, allWidgets)
    return sizes.includes('s') ? 's' : sizes[0] || 's'
  }
  // Into a board (its first empty cell), or a new board with it: off the
  // taskbar and wherever else it was on the desktop first (one place).
  function toBoard(item, boardId) {
    const old = Array.isArray(config.deskLayout) ? config.deskLayout : []
    const m = item.plugin ? { type: 'plugin', of: item.plugin } : { type: 'widget', of: item.id }
    const size = boardSize(item)
    const board = old.find(e => e.id === boardId && DeskGrid.isBoard(e))
    let next
    if (board) {
      const spot = DeskGrid.boardFirst(board, size)
      if (!spot) return
      next = DeskGrid.intoBoard(old, board.id, m, size, spot, newId)
    } else {
      next = [...DeskGrid.without(old, item, newId), { id: newId(), type: 'board', items: [{ ...m, size, at: [0, 0] }] }]
    }
    offBar([item])
    keepDeskLayout(next)
  }
  // A whole desktop layout kept as what changed in it (desk.rs desk_keep
  // merges it with the pages'; a new board goes to the pointer's display).
  function keepDeskLayout(next) {
    const old = Array.isArray(config.deskLayout) ? config.deskLayout : []
    const was = new Map(old.map(e => [e.id, JSON.stringify(e)]))
    const ids = new Set(next.map(e => e.id))
    config.deskLayout = next
    window.pet.desk.keep(
      next.filter(e => was.get(e.id) !== JSON.stringify(e)),
      old.filter(e => !ids.has(e.id)).map(e => e.id),
    )
    takeWidgets(allWidgets)
    update()
  }

  // On the desktop (desk.rs, deskgrid.js): medium where it has it; where it
  // stands the desktop's page says (the display the pointer is on, the top
  // right, clear of the icons).
  function putOnDesk(item) {
    if (DeskGrid.has(config.deskLayout, item)) return
    const w = item.plugin ? allWidgets.find(x => x.plugin === item.plugin) : allWidgets.find(x => x.id === item.id)
    const sizes = w ? Widgets.sizesOf(w) : ['s', 'm']
    const source = item.plugin ? { type: 'plugin', of: item.plugin } : { type: 'widget', of: item.id }
    const label = w && !w.private && !item.plugin ? [...String(Widgets.words(lang, w).label)].slice(0, 40).join('') : ''
    const icon = (w?.icon || item.icon) in Widgets.ICONS ? { icon: w?.icon || item.icon } : {}
    keepDesk([{ id: newId(), ...source, size: sizes.includes('m') ? 'm' : 's', ...icon, ...(label ? { name: label } : {}) }])
  }
  function keepDesk(entries) {
    config.deskLayout = [...(Array.isArray(config.deskLayout) ? config.deskLayout : []), ...entries]
    window.pet.desk.keep(entries, [])
    takeWidgets(allWidgets)
    update()
  }
  function deskOff(items) {
    let next = Array.isArray(config.deskLayout) ? config.deskLayout : []
    for (const it of items) next = DeskGrid.without(next, it, newId)
    config.deskLayout = next
    window.pet.settings.set({ deskLayout: next })
  }
  // From the strip or the capsule to the desktop: a group as a stack there
  // (medium where all of it has that), one alone as itself.
  function toDesk(w, entry, member) {
    if (entry && Widgets.isGroup(entry) && member < 0) {
      const items = entry.items.map(m => ({ type: m.type, of: m.of, ...(m.name ? { name: m.name } : {}), ...(m.icon ? { icon: m.icon } : {}) }))
      const sizes = DeskGrid.sizesOfEntry({ type: 'stack', items }, allWidgets)
      if (stackFor === entry.id) closeStackPop()
      removeEntry(entry.id)
      return keepDesk([{ id: newId(), type: 'stack', items, size: sizes.includes('m') ? 'm' : sizes[0] || 's' }])
    }
    const m = entry ? (Widgets.isGroup(entry) ? entry.items[member] : entry) : null
    const item = m ? (m.type === 'plugin' ? { plugin: m.of } : { id: m.of }) : w?.plugin ? { plugin: w.plugin } : w ? { id: w.id } : null
    if (item) placeWidget(item, 'desk')
  }
  // A desktop page's asking (desk.rs desk_ask): a widget's settings, or
  // widgets put elsewhere; a stack onto the taskbar as a stack there.
  function deskPlace({ items = [], where, stack } = {}) {
    if (!['bar', 'isle'].includes(where) || !items.length) return
    if (where === 'bar' && home() !== 'taskbar') return
    if (where === 'bar' && stack && items.length >= 2) {
      deskOff(items)
      const members = items.slice(0, 8).map(it => (it.plugin ? { type: 'plugin', of: it.plugin } : { type: 'widget', of: it.id }))
      const layout = tbLayout()
      layout.right.unshift({ id: newId(), type: 'stack', items: members })
      return keepLayout(layout)
    }
    for (const it of items) placeWidget(it, where)
  }

  // Back in the island: off the strip (out of its group), and on there.
  function backToIsland(w, entry, member) {
    if (Widgets.isGroup(entry) && member >= 0) removeMember(entry.id, member)
    else removeEntry(entry.id)
    const id = w?.id || (entry.type === 'widget' ? entry.of : null)
    if (id && (config.widgetsOff || []).includes(id)) window.pet.settings.set({ widgetsOff: config.widgetsOff.filter(x => x !== id) })
  }

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
        // A window with no title (tasks.rs is_task): its program's name.
        top.append(el('span', 'wt', w.title || app.name))
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
      // Windows' End task while its setting is on (zhoupengjie/Wakuwaku#36),
      // over the close as on Windows' menu: the program's windows' processes
      // ended (tasks.rs end_task). None for File Explorer, as Windows offers
      // none: its windows are the shell's own (explorer.exe).
      if (windows.length && look.endTask && !isExplorer(app.path)) rows.push(item(t(lang, windows.length > 1 ? 'taskbar.endAll' : 'taskbar.end'), 'end'))
      if (windows.length) rows.push(item(t(lang, windows.length > 1 ? 'taskbar.closeAll' : 'taskbar.close'), 'close'))
    }
    if (!rows.length) return closePop()
    // Another program's windows taking the place of the last's (the pointer
    // gone along the buttons): where the list was and how big, to slide from.
    const isList = popFor.kind === 'list'
    const from = isList && !pop.hidden && pop.classList.contains('list') && popShown && popShown !== app.app ? { [isSide() ? 'top' : 'left']: isSide() ? pop.offsetTop : pop.offsetLeft, width: pop.offsetWidth, height: pop.offsetHeight } : null
    const opening = pop.hidden
    // Drawn again only when what it shows changed: a card replaced under a
    // press is never clicked (the windows' list comes again often).
    const drawn = JSON.stringify([popFor.app, popFor.kind, app.name, app.pinned, windows.map(w => [w.id, w.title, w.front, (w.png || '').length]), lang, look.endTask])
    if (drawn !== popDrawn || pop.hidden) {
      popDrawn = drawn
      pop.replaceChildren(...rows, popReach)
    }
    pop.classList.toggle('menu', popFor.kind === 'menu')
    pop.classList.toggle('list', isList)
    pop.hidden = false
    popShown = app.app
    // Mid-slide, the list keeps going where it was going.
    if (popMoving && !from) return
    // Over its button (beside it on a side bar, 6 px off the bar), kept
    // within the screen, at its own size.
    pop.classList.remove('sliding', 'rising')
    pop.style.width = ''
    pop.style.height = ''
    const to = { ...besideOrOver(button, pop, isSide() ? 8 : 4), width: pop.offsetWidth, height: pop.offsetHeight }
    if (from) return slidePop(from, to)
    pop.style.left = to.left === undefined ? '' : `${to.left}px`
    pop.style.top = to.top === undefined ? '' : `${to.top}px`
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

  // Where a pop goes in the bar (its px): centred over a node along the bar,
  // or beside it on a side bar (its place there by style.css), kept this
  // far from the bar's ends.
  function besideOrOver(node, popNode, keep) {
    const r = node.getBoundingClientRect()
    const base = barRest.getBoundingClientRect()
    if (isSide()) return { top: Math.round(Math.max(keep, Math.min(r.top - base.top + r.height / 2 - popNode.offsetHeight / 2, barRest.offsetHeight - popNode.offsetHeight - keep))) }
    return { left: Math.round(Math.max(keep, Math.min(r.left - base.left + r.width / 2 - popNode.offsetWidth / 2, barRest.offsetWidth - popNode.offsetWidth - keep))) }
  }

  function slidePop(from, to) {
    holdThumbs()
    // Along the bar: across a bottom or top one, up and down a side one.
    const key = 'top' in to ? 'top' : 'left'
    pop.style.setProperty('--come', `${to[key] > from[key] ? 16 : -16}px`)
    pop.classList.add('sliding')
    const box = b => ({ [key]: `${b[key]}px`, width: `${b.width}px`, height: `${b.height}px` })
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
  // to the front, or is minimized when in front; several, their list at
  // once to pick one from, as Windows 11's own (zhoupengjie/Wakuwaku#34:
  // they all came to the front together, or all went down), and pressed
  // again it goes. One the pointer rested on stays open for the press.
  // With the keys and the other buttons as on Windows' own
  // (zhoupengjie/Wakuwaku#36, presses.js pressOf): the middle button or
  // Shift, another of it; Ctrl and Shift, another as administrator; Ctrl on
  // one with several windows, the next of them; a right press, its menu,
  // with Shift its window's own menu (one for all its windows when it has
  // several).
  function pressApp(app, what) {
    const windows = app.windows || []
    if (!what) return
    if (what === 'menu') return openPop(app.app, 'menu')
    if (what !== 'press') closePop()
    if (what === 'launch' || what === 'admin') return window.pet.taskbar.app(app.path, what, app.id)
    if (what === 'windowMenu') {
      menuUp = true
      return window.pet.taskbar.windowMenu(windows.map(w => w.id)).finally(() => (menuUp = false))
    }
    if (what === 'next') {
      const last = cycled?.app === app.app && Date.now() - cycled.at < window.Presses.CYCLED_MS ? cycled.id : null
      const next = window.Presses.nextWindow(windows, last)
      cycled = { app: app.app, id: next.id, at: Date.now() }
      return window.pet.taskbar.window(next.id, 'front')
    }
    if (!windows.length) return window.pet.taskbar.app(app.path, 'launch', app.id)
    if (windows.length > 1) {
      const listed = popFor?.kind === 'list' && popFor.app === app.app && !pop.hidden
      if (listed && popFor.pressed) return closePop()
      if (!listed) openPop(app.app, 'list')
      popFor.pressed = true
      return
    }
    closePop()
    window.pet.taskbar.window(windows[0].id, 'press')
  }
  // The window Ctrl and a press last brought, for the next to go on from
  // it while its mark in front has not come yet ({ app, id, at }).
  let cycled = null
  // A window's own menu open (Shift and a right press): no list of windows
  // on the pointer resting on the button, which came up over the menu.
  let menuUp = false
  // Shift and Ctrl as Windows has them at the press (taskbar_held), there
  // by the time the button comes up: her window never takes the keyboard,
  // and what the page's own events say of them may be behind. Theirs only
  // when it could not say.
  let heldAtPress = null
  windowsBox.addEventListener('pointerdown', e => {
    if (e.target.closest('[data-app]')) heldAtPress = window.pet.taskbar.held()
  })
  function pressAt(e) {
    const key = e.target.closest('[data-app]')?.dataset.app
    const press = { type: e.type, button: e.button }
    const own = { shift: e.shiftKey, ctrl: e.ctrlKey }
    const held = heldAtPress
    heldAtPress = null
    Promise.resolve(held).then(keys => {
      const app = appOf(key)
      if (app) pressApp(app, window.Presses.pressOf(press, keys || own, (app.windows || []).length))
    })
  }
  windowsBox.addEventListener('click', e => {
    if (!appOf(e.target.closest('[data-app]')?.dataset.app)) return
    e.stopPropagation()
    // The end of a drag, not a press.
    if (winDragged) {
      winDragged = false
      return
    }
    pressAt(e)
  })
  windowsBox.addEventListener('auxclick', e => {
    if (e.button !== 1 || !appOf(e.target.closest('[data-app]')?.dataset.app)) return
    e.stopPropagation()
    pressAt(e)
  })
  // The middle button pressed: no scrolling by the pointer (Chromium's), on
  // a side bar's column of buttons.
  windowsBox.addEventListener('mousedown', e => e.button === 1 && e.preventDefault())
  windowsBox.addEventListener('contextmenu', e => {
    if (!appOf(e.target.closest('[data-app]')?.dataset.app)) return
    e.preventDefault()
    e.stopPropagation()
    pressAt(e)
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
    const [a, c] = [along(x, y), across(x, y)]
    if (c < crossLo(strip) - 24 || c > crossHi(strip) + 24) return null
    const buttons = [...windowsBox.querySelectorAll('[data-app]')]
    const self = buttons.find(b => b.dataset.app === key)
    const r0 = self?.getBoundingClientRect()
    if (r0 && a >= lo(r0) && a <= hi(r0)) return null
    const others = buttons.filter(b => b !== self)
    if (!others.length) return null
    for (const b of others) {
      const r = b.getBoundingClientRect()
      if (a < hi(r)) return { target: b.dataset.app, after: a > lo(r) + len(r) / 2, slot: b }
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

  // The pointer resting on a running program's button: its windows. With
  // another's list open, only once it has been on this button as long as
  // Windows' own taskbar waits, and not while it is on its way up into
  // the list (aim.js, zhoupengjie/Wakuwaku#56): it changed at once to any
  // button crossed on the way to a card.
  const aim = window.Aim.create()
  const aimList = () => (popFor?.kind === 'list' && !pop.hidden ? { key: popFor.app, rect: pop.getBoundingClientRect(), edge: tbEdge() } : null)
  function aimPop() {
    clearTimeout(popTimer)
    const app = appOf(aim.key())
    if (!app || winPress?.ghost || menuUp || popFor?.kind === 'menu' || !(app.windows || []).length) return
    const wait = aim.due(aimList(), performance.now())
    if (wait === null) return
    if (wait === 0) return openPop(app.app, 'list')
    popTimer = setTimeout(aimPop, wait)
  }
  windowsBox.addEventListener('mousemove', e => aim.move(e.clientX, e.clientY, performance.now()))
  windowsBox.addEventListener('mouseover', e => {
    const at = performance.now()
    aim.move(e.clientX, e.clientY, at)
    aim.over(e.target.closest('[data-app]')?.dataset.app ?? null, at)
    aimPop()
  })
  // Gone from the button and the list a moment: it closes. Between them it
  // is on the list (style.css .wpop::after: its room reaches the strip, as
  // Windows' own does).
  for (const node of [windowsBox, pop]) {
    node.addEventListener('mouseleave', () => {
      if (node === windowsBox) aim.over(null, performance.now())
      clearTimeout(popTimer)
      popTimer = setTimeout(closePop, window.Aim.CLOSE_MS)
    })
    node.addEventListener('mouseenter', () => popFor && clearTimeout(popTimer))
  }

  // A file dragged from another program and held over a running program's
  // button half a second: its window to the front, or with several their
  // list open, a card in it held over as long bringing that one, for the
  // file to be let go of in the window, as on Windows' own
  // (zhoupengjie/Wakuwaku#36). The drag's place comes from Tauri
  // (bridge.js onFileDrag, which also plays it as the pointer's: the list
  // goes once the drag has left it and the buttons a moment, as for the
  // pointer). What it is held over: 'win:<id>', 'app:<key>', or null.
  let fileHeld = null
  let fileTimer
  window.pet.onFileDrag((type, at) => {
    const live = at && (type === 'enter' || type === 'over') && isOn() && home() === 'taskbar' && !editLayout
    const node = live ? document.elementFromPoint(at.x, at.y) : null
    const card = node && pop.contains(node) ? node.closest('[data-win]') : null
    const button = !card && node && windowsBox.contains(node) ? node.closest('[data-app]') : null
    const held = card ? `win:${card.dataset.win}` : button ? `app:${button.dataset.app}` : null
    if (held === fileHeld) return
    fileHeld = held
    clearTimeout(fileTimer)
    if (!held) return
    fileTimer = setTimeout(() => {
      if (card) return window.pet.taskbar.window(card.dataset.win, 'drag')
      const app = appOf(button.dataset.app)
      const windows = app?.windows || []
      if (windows.length === 1) {
        closePop()
        window.pet.taskbar.window(windows[0].id, 'drag')
      } else if (windows.length > 1 && !(popFor?.kind === 'list' && popFor.app === app.app && !pop.hidden)) {
        openPop(app.app, 'list')
      }
    }, window.Presses.DRAG_HOLD_MS)
  })

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
    // A window picked: to the front, never minimized (taskbar.rs 'front').
    if (row) {
      closePop()
      return window.pet.taskbar.window(row.dataset.win, 'front')
    }
    const act = e.target.closest('[data-act]')?.dataset.act
    if (!act) return
    closePop()
    if (act === 'close' || act === 'end') for (const w of app.windows || []) window.pet.taskbar.window(w.id, act)
    else window.pet.taskbar.app(app.path, act, app.id)
  })
  pop.addEventListener('contextmenu', e => {
    e.preventDefault()
    e.stopPropagation()
  })

  // The script's mark by the layout's language: its own and plain letters.
  const SCRIPTS = { 0x04: ['中', '英'], 0x11: ['あ', 'A'], 0x12: ['한', 'A'] }
  const LANGS = { 0x09: 'EN', 0x07: 'DE', 0x0c: 'FR', 0x0a: 'ES', 0x10: 'IT', 0x16: 'PT', 0x19: 'RU', 0x13: 'NL', 0x1f: 'TR' }
  function langMark(langId, native) {
    const primary = (langId || 0) & 0x3ff
    const script = SCRIPTS[primary]
    return { script, text: script ? script[native === false ? 1 : 0] : LANGS[primary] || primary.toString(16).toUpperCase() }
  }
  // The mode's word in the tip where Windows gives none of its own.
  const MODE_WORDS = { 中: '中文', 英: '英文' }
  // The input methods' icons (taskbar.rs taskbar:imeIcons), by key: each
  // sent once, named by the keys after it.
  const imeIcons = {}
  let keysNow = null
  let imeDrawn = ''
  // As Windows' own taskbar shows the input method in front (which one it
  // is, inputs.rs): Microsoft's own, and a plain layout, by the script's
  // mark (中 or 英), as Windows draws its glyph; one of another maker
  // (Sogou's, WeChat's) by the picture it gives of its mode, as Windows
  // shows it, or else its own icon beside 中 or 英, so two of one language
  // can be told apart. Its name, its mode and its language in the tip.
  function showIme(keys) {
    const { script, text } = langMark(keys?.lang, keys?.native)
    const ime = keys?.ime || null
    const theirs = !!ime && !ime.own
    const modePic = (theirs && ime.mode?.icon && imeIcons[ime.mode.icon]) || ''
    const ownPic = (theirs && !modePic && ime.icon && imeIcons[ime.icon]) || ''
    const mode = ime?.mode?.tip || (script ? MODE_WORDS[text] || text : '')
    const tip = ime?.name ? [[ime.name, mode].filter(Boolean).join(' · '), ime.lang].filter(Boolean).join('\n') : ''
    const drawn = JSON.stringify([text, modePic, ownPic, tip])
    if (drawn !== imeDrawn) {
      imeDrawn = drawn
      const pic = modePic || ownPic
      const img = pic ? el('img', 'pic') : null
      if (img) img.src = pic
      imeMark.replaceChildren(...(img ? [img] : []), ...(modePic ? [] : [el('span', 'script', text)]))
      imeMark.title = tip
    }
    imeMark.classList.toggle('unknown', !modePic && !!script && (keys?.native == null || !!ime?.mode?.off))
  }
  window.pet.onImeIcons(icons => {
    Object.assign(imeIcons, icons || {})
    if (keysNow) showIme(keysNow)
  })
  window.pet.onKeys(keys => {
    keysNow = keys
    showIme(keys)
    capsMark.classList.toggle('on', !!keys?.caps)
    capsMark.title = keys?.caps ? 'Caps Lock' : ''
    showNet(keys?.net, keys?.bars)
    showPrivacy(keys?.use)
    showVolume(keys?.volume)
    showBattery(keys?.battery)
  })

  // A right press on the mark: the input methods to pick from, as Windows'
  // own list has them (Win+Space), in its order, each with its icon (a
  // layout by its language's mark), the one in front marked; a press on one
  // switches the window in front to it (inputs.rs pick), and the last row
  // opens Windows' languages. Windows' own list where none can be had.
  async function openImeList() {
    if (popFor) closePop()
    if (wgFor) closeWidgetPop()
    const list = await window.pet.taskbar.inputs()
    if (!Array.isArray(list) || !list.length) return window.pet.taskbar.open('inputs')
    const rows = list.map(m => {
      const row = el('div', m.current ? 'wrow irow front' : 'wrow irow')
      row.dataset.input = m.id
      if (m.icon) {
        const img = el('img')
        img.src = m.icon
        row.append(img)
      } else row.append(el('span', 'itile', langMark(m.lang, true).text))
      const lines = el('span', 'ilines')
      // As Windows' list has them: the language, the input method under it.
      lines.append(el('span', 'wt', m.langName), el('span', 'iname', m.name))
      row.append(lines)
      return row
    })
    const more = el('div', 'witem', t(lang, 'taskbar.imeMore'))
    more.dataset.input = ''
    imePop.replaceChildren(...rows, more)
    imePop.hidden = false
    imeOpen = true
    const at = besideOrOver(imeMark, imePop, isSide() ? 8 : 4)
    imePop.style.left = at.left === undefined ? '' : `${at.left}px`
    imePop.style.top = at.top === undefined ? '' : `${at.top}px`
    watchPresses()
  }

  function closeImeList() {
    imeOpen = false
    imePop.hidden = true
    watchPresses()
  }

  imePop.addEventListener('click', e => {
    const row = e.target.closest('[data-input]')
    if (!row) return
    closeImeList()
    if (row.dataset.input) window.pet.taskbar.input(row.dataset.input)
    else window.pet.taskbar.open('languages')
  })
  imePop.addEventListener('contextmenu', e => {
    e.preventDefault()
    e.stopPropagation()
  })

  // --- The taskbar's look: Mica, light or dark, Windows' accent -------------------------

  // Behind the strip's parts: the desktop's picture under it, blurred and
  // tinted (mica.js) for Windows' mode, as taskbar.rs sent it last; drawn
  // again only when the picture, the mode or the strip's width changes. None
  // on a solid strip. On a clear one (tb-clear) it is drawn too, unseen, if
  // the strip is to fill in as Mica (tb-clear-mica): it fades in at once
  // while a window is maximized or full screen on its display (tb-filled,
  // drawn again as the windows change); filled in solid, tb-solid. The
  // strip light in Windows' light mode (tb-light), its own words dark on
  // it or on a light desktop under the clear strip (tb-ink-light), her end
  // a capsule wherever the strip is not black (tb-capsule).
  // The strip as thick as it lies on the screen (island.rs tell_strip): its
  // physical px over the scale, a fraction of a CSS px if need be, so its
  // inner edge is the work area's, where maximized windows stop (a hairline
  // gap at 175%: zhoupengjie/Wakuwaku#2). 48 until told.
  let stripThick = 0
  // The monitor's scale as taskbar:strip says it (sf); 0 until told.
  let stripScale = 0
  function setStrip() {
    if (!stripThick) return
    document.documentElement.style.setProperty('--strip-h', `${stripThick / (devicePixelRatio || 1)}px`)
    fitPage()
    // A monitor's scale or Windows text scaling may have changed while the
    // settings are open; recompute their body height with the new CSS room.
    if (window.Settings?.isOpen?.()) window.Settings.redraw()
    // Once laid out for it (its edge may have just changed too).
    setTimeout(() => {
      const r = island.parentElement.getBoundingClientRect()
      const dpr = devicePixelRatio || 1
      console.log(`taskbar: strip in the page ${[r.left, r.top, r.right, r.bottom].map(v => +(v * dpr).toFixed(2)).join(', ')} (x ${dpr}, told ${stripThick} px)`)
    }, 400)
  }
  // Her tray icon clicked on a Mac: the island in the notch drops open, or
  // goes back. Open, it goes back once the pointer has been in it and left,
  // or after a while if the pointer never comes.
  function setPeek(on) {
    if (on !== peek) console.log(`island: peek ${on ? 'open' : 'closed'}`)
    if (!on) endNudge()
    peek = on
    clearTimeout(peekTimer)
    if (peek) peekTimer = setTimeout(() => !isHover && setPeek(false), PEEK_MS)
    update()
  }
  // Her tray icon toggles it; a double-click on her only opens it ({ open }).
  window.pet.onPeek(p => setPeek(p?.open === true ? true : !peek))
  window.pet.onTop(({ height, notch } = {}) => {
    const next = height > 0 ? { height, notch: notch > 0 ? notch : 0 } : null
    if (JSON.stringify(next) === JSON.stringify(screenTop)) return
    screenTop = next
    body.classList.toggle('at-top', !!screenTop)
    body.classList.toggle('notched', !!screenTop?.notch)
    document.documentElement.style.setProperty('--top-h', `${screenTop ? screenTop.height : 36}px`)
    // Round a notch, only ever in the middle.
    placeAnchor()
    update()
  })
  // The page as big as its window (told with the strip, its physical size).
  // At 175% Chromium makes the page a little bigger than its window (the
  // window's 1204 px over 1.75 come to 688.00006 in its sums, so 689 CSS
  // px): what lies along the page's foot or right end lay past the window's,
  // the strip 2 px low and a gap above it (zhoupengjie/Wakuwaku#19). The
  // overhang cut off (style.css body.home-taskbar); a few px at most, else
  // it is the window being resized under a size told before.
  let winPx = null
  function fitPage() {
    const root = document.documentElement
    const r = root.getBoundingClientRect()
    const dpr = devicePixelRatio || 1
    const over = (have, want) => (have - want / dpr > 0.01 && have - want / dpr < 4 ? have - want / dpr : 0)
    root.style.setProperty('--page-cut-x', `${winPx ? over(r.width, winPx[0]) : 0}px`)
    root.style.setProperty('--page-cut-y', `${winPx ? over(r.height, winPx[1]) : 0}px`)
  }
  window.pet.onStrip(({ thick, win, sf } = {}) => {
    if (!(thick > 0)) return
    stripThick = thick
    if (Number.isFinite(sf) && sf > 0) stripScale = sf
    if (Array.isArray(win) && win[0] > 0 && win[1] > 0) winPx = win
    setStrip()
  })
  window.addEventListener('resize', () => {
    if (!stripThick) return
    document.documentElement.style.setProperty('--strip-h', `${stripThick / (devicePixelRatio || 1)}px`)
    fitPage()
    if (window.Settings?.isOpen?.()) window.Settings.redraw()
    sendSolid()
  })

  const micaBox = el('div')
  micaBox.id = 'mica'
  island.parentElement.prepend(micaBox)
  const mica = Mica.mount(micaBox)
  let paper = null

  function drawMica() {
    const clearMica = isClear() && clearWhen() === 'mica'
    body.classList.toggle('tb-mica', isMica())
    body.classList.toggle('tb-light', isLight())
    body.classList.toggle('tb-ink-light', isStripLight())
    setAccent('--strip-accent', Mica.accentShade(look.accent, isStripLight()))
    body.classList.toggle('tb-capsule', isCapsule())
    body.classList.toggle('tb-clear', isClear())
    body.classList.toggle('tb-clear-mica', clearMica)
    body.classList.toggle('tb-filled', isFilled())
    body.classList.toggle('tb-solid', isFilled() && clearWhen() === 'solid')
    if (isMica() || clearMica) mica.set(paper, isSide() ? TASKBAR_H : innerWidth, isSide() ? innerHeight : TASKBAR_H, look.mode)
    else mica.clear()
    drawIslandLook()
    measureBackdrop()
    inkParts()
    // The strip's words turned light or dark (a window maximized over the
    // clear strip fills it in, back to Windows' mode): what is drawn with
    // their colour (stripHue, the widgets' cards) drawn again.
    const light = isStripLight()
    if (light !== stripWasLight) {
      stripWasLight = light
      requestAnimationFrame(() => update())
    }
  }
  let stripWasLight = null

  // Each of the strip's parts its own words (style.css ink-light), and
  // Windows' accent in their shade (--strip-accent), by the desktop right
  // under it on the clear strip (mica.js inkOn): the clock over a dark
  // corner of a picture light in the middle keeps light words, the
  // programs' buttons over the light middle dark ones. Elsewhere, or while
  // it is not known, all as the strip's (isStripLight). Done again once
  // they are laid out (fillTaskbar), the middle run has slid (alignApps)
  // or the picture is known; a widget's face is drawn in its part's
  // (drawEntries), so it is drawn again when that turns. The glide over a
  // part as it is.
  const stripParts = () => [...barRest.querySelectorAll(':scope > .tzone > [data-part]'), deskEdge]
  function partLight(node) {
    if (!backdrop || !isClear() || isFilled()) return isStripLight()
    const strip = island.parentElement.getBoundingClientRect()
    const r = node.getBoundingClientRect()
    const [from, to] = isSide() ? [r.top - strip.top, r.bottom - strip.top] : [r.left - strip.left, r.right - strip.left]
    return Mica.stripInkLight({ light: isLight(), clear: true, filled: false, backdrop: Mica.inkOn(backdrop, from, to) })
  }
  const inkOf = node => (node.closest('[data-part]') || node).classList.contains('ink-light')
  function inkParts() {
    if (!taskbarBuilt) return
    let turned = false
    for (const node of stripParts()) {
      const light = partLight(node)
      if (!node._inked || node.classList.contains('ink-light') !== light) {
        node.classList.toggle('ink-light', light)
        node._inked = true
        turned = true
      }
      setAccent('--strip-accent', Mica.accentShade(look.accent, light), node)
    }
    if (glideOn) glide.classList.toggle('ink-light', inkOf(glideOn))
    if (turned) drawEntries()
  }

  // The desktop under the clear strip looked at again when the picture or
  // the strip's size changes; drawn anew once it is known.
  let backdropKey = ''
  function measureBackdrop() {
    if (!isClear()) return
    const width = isSide() ? TASKBAR_H : innerWidth
    const height = isSide() ? innerHeight : TASKBAR_H
    const key = JSON.stringify([paper, width, height])
    if (!paper || key === backdropKey) return
    backdropKey = key
    Mica.lightness(paper, width, height, isSide()).then(profile => {
      if (key !== backdropKey) return
      if (profile) {
        const ys = profile.ys
        console.log(`taskbar: the desktop under the clear strip ${profile.mean.toFixed(3)} light (${Math.min(...ys).toFixed(3)} to ${Math.max(...ys).toFixed(3)} along it), its words ${Mica.wantsDarkInk(profile.mean) ? 'dark' : 'light'} as a whole`)
      } else console.log('taskbar: the desktop under the clear strip not read')
      backdrop = profile
      drawMica()
    })
  }

  // Windows' mode: as taskbar.rs tells the taskbar (taskbar:look), and in
  // pet:update at the top of the screen (winLight), where there is no
  // taskbar to hear it from.
  const modeNow = () => (home() === 'taskbar' ? look.mode : now.winLight === true ? 'light' : 'dark')
  // The look as the settings give it now (none: the defaults): on the
  // taskbar where her end is a capsule (islandLook), at the top of the
  // screen its own (islandTopLook, zhoupengjie/Wakuwaku#27); and its
  // ground's colour (null: none given), how solid (0.2 to 1; solid when no
  // colour is given) and blurred.
  const lookNow = () => (home() === 'taskbar' ? (isCapsule() && config.islandLook?.[look.mode]) || {} : config.islandTopLook?.[modeNow()] || {})
  function groundOf(mine) {
    const bg = /^#[0-9a-f]{6}$/i.test(mine.bg || '') ? parseInt(mine.bg.slice(1), 16) : null
    const op = bg !== null && Number.isFinite(mine.op) ? Math.min(100, Math.max(20, mine.op)) / 100 : 1
    const blur = Number.isFinite(mine.blur) ? Math.min(60, Math.max(0, mine.blur)) : 0
    return { bg, op, blur }
  }

  // Her island's look on the taskbar, as the settings may set it for each of
  // Windows' modes (islandLook: { dark, light }, each { bg: '#rrggbb', op: 20
  // to 100, blur: 0 to 60, edge: 'line' | 'ring' | 'none', bare }), over
  // Windows' own flyout grey (style.css --i-bg, --i-blur, --i-edge,
  // island-bare); a part left out keeps its default. The ring is the
  // session's colour (--ring, as the island draws). At the top of the
  // screen the same from its own (islandTopLook), over black; there the
  // page has nothing behind the island to blur: only Windows' acrylic
  // blurs it (placePlate).
  function drawIslandLook() {
    const mine = lookNow()
    const { bg, op, blur } = groundOf(mine)
    const rgba = a => `rgba(${(bg >> 16) & 255}, ${(bg >> 8) & 255}, ${bg & 255}, ${a})`
    if (bg === null) body.style.removeProperty('--i-bg')
    else body.style.setProperty('--i-bg', rgba(op))
    // Deeper over a maximized window, with nothing to blur it (setDeep).
    if (bg === null || op >= DEEP_OP) body.style.removeProperty('--i-bg-deep')
    else body.style.setProperty('--i-bg-deep', rgba(DEEP_OP))
    // The rim over the acrylic's edge (sendPlate): three quarters of the
    // way from the ground's opacity to solid, half that as it fades in.
    const rim = op + (1 - op) * 0.75
    if (bg === null || op >= 1) {
      body.style.removeProperty('--i-rim')
      body.style.removeProperty('--i-rim-half')
    } else {
      body.style.setProperty('--i-rim', rgba(rim.toFixed(3)))
      body.style.setProperty('--i-rim-half', rgba((rim / 2).toFixed(3)))
    }
    if (blur && home() === 'taskbar') island.style.setProperty('--i-blur', `blur(${blur}px) saturate(1.25)`)
    else island.style.removeProperty('--i-blur')
    const edge = {
      ring: '0 0 0 1.5px var(--ring), 0 0 12px color-mix(in srgb, var(--ring) 35%, transparent)',
      none: '0 0 0 0 transparent',
    }[mine.edge]
    if (edge) island.style.setProperty('--i-edge', edge)
    else island.style.removeProperty('--i-edge')
    // Bare unless said otherwise in the dark mode (the default since
    // 2026-10-13, settings.js ILOOK_DEFAULT), not in the light.
    body.classList.toggle('island-bare', isCapsule() && (mine.bare ?? look.mode === 'dark') === true)
    groundLight = bg === null ? null : lightness(bg) > 0.35
    setInk()
  }

  // A colour's relative lightness (WCAG), 0 to 1.
  function lightness(rgb) {
    const [r, g, b] = [(rgb >> 16) & 255, (rgb >> 8) & 255, rgb & 255].map(v => {
      v /= 255
      return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4
    })
    return 0.2126 * r + 0.7152 * g + 0.0722 * b
  }

  // Her island's words dark (style.css isl-light, hue, ink) on a light
  // ground: Windows' light mode's, or a light one of the settings' own
  // (groundLight; null: none given); in the settings, theirs, as Windows'
  // mode.
  function setInk() {
    const own = view === 'settings' ? null : groundLight
    const was = inkLight
    inkLight = own === null ? isLight() : own
    body.classList.toggle('isl-light', inkLight)
    // A card's plugin view told her theme, and its card's colours.
    if (was !== inkLight) islandViews?.relook(allWidgets)
  }

  window.pet.onWallpaper(p => {
    paper = p
    drawMica()
  })

  // Windows' mode and accent colour (taskbar.rs). The accent as its own
  // taskbar draws with it: a lighter one on the dark strip, a darker one on
  // the light (Mica.accentShade). --win-accent by Windows' mode, for the
  // settings and what opens from the strip; --strip-accent by the strip's
  // own words (drawMica), for what is drawn on it: the line under the window
  // in front, a program's count, the privacy marks, where a dragged button
  // lands; on each part, by its own (inkParts). Their own blue until it
  // comes. Not --accent: that is her mood's colour (pet.js).
  function setAccent(name, colour, on = document.documentElement) {
    if (colour) on.style.setProperty(name, colour)
    else on.style.removeProperty(name)
  }

  window.pet.onLook(l => {
    const seconds = look.seconds
    look = { mode: l?.mode === 'light' ? 'light' : 'dark', accent: l?.accent || null, endTask: l?.endTask === true, seconds: l?.seconds === true, taskView: l?.taskView === true }
    if (look.seconds !== seconds) clockLoop()
    // How many times a button flashes for attention (style.css .flash).
    if (l?.flashes !== undefined) barRest.style.setProperty('--flashes', l.flashes === 0 ? 'infinite' : String(l.flashes))
    setAccent('--win-accent', Mica.accentShade(look.accent, look.mode === 'light'))
    setAccent('--strip-accent', Mica.accentShade(look.accent, isStripLight()))
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
      alignApps()
      snapStart()
      drawMica()
      setDeep(true)
    }
  })

  // The time and the date, as Windows' taskbar has them: with the seconds
  // while Windows' own clock shows them (taskbar.rs seconds_on), the time
  // anew at each second's start then, else every five seconds. Its digits
  // as wide as each other (style.css .tclock), so the tray beside it keeps
  // still as they change.
  let clockTick = 0
  function tickClock() {
    const d = new Date()
    const time = d.toLocaleTimeString(lang === 'zh' ? 'zh-CN' : undefined, { hour: '2-digit', minute: '2-digit', second: look.seconds ? '2-digit' : undefined })
    const date = d.toLocaleDateString(lang === 'zh' ? 'zh-CN' : undefined, isSide() ? { month: 'numeric', day: 'numeric' } : { year: 'numeric', month: 'numeric', day: 'numeric' })
    if (clockBox.dataset.at !== time + date) {
      clockBox.dataset.at = time + date
      clockBox.replaceChildren(el('b', '', time), el('i', '', date))
    }
    clockBox.classList.toggle('secs', look.seconds === true)
    // The tip each minute, not each second.
    const minute = `${date} ${d.getHours()}:${d.getMinutes()}`
    if (clockBox.dataset.tipAt !== minute) {
      clockBox.dataset.tipAt = minute
      tipClock()
    }
  }
  function clockLoop() {
    clearTimeout(clockTick)
    if (isOn() && home() === 'taskbar') tickClock()
    clockTick = setTimeout(clockLoop, look.seconds ? 1000 - (Date.now() % 1000) : 5000)
  }
  clockLoop()
  // Its tip as Windows' own (zhoupengjie/Wakuwaku#36, clock.rs): the date in
  // full with its weekday, in the person's own format, and Windows'
  // additional clocks; read anew each minute and as the pointer comes.
  function tipClock() {
    window.pet.taskbar.clockTip().then(tip => {
      if (typeof tip === 'string' && tip) clockBox.title = tip
    })
  }
  clockBox.addEventListener('pointerenter', tipClock)

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
  chevron.setAttribute('role', 'button')
  const outBox = el('span', 'tout')
  const flyout = el('span', 'tfly')
  let trayOpen = false

  // One a press does not reach (reach false: its program handed its icon to
  // Windows' own taskbar only, before hers was up, and never named hers the
  // message to send it; systray.rs) says so in its tip.
  function trayIcon(i) {
    const slot = el('span', 'ticon')
    slot.dataset.tray = i.key
    slot.dataset.name = i.name
    const unreached = i.reach === false ? `\n${t(lang, 'taskbar.trayUnreached')}` : ''
    slot.title = (i.tip || i.exe) + unreached
    slot.setAttribute('role', 'button')
    slot.setAttribute('aria-label', (i.tip || i.name || i.exe || '') + unreached)
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

  // Starting, the tray kept as wide as it was last time until as many icons
  // are back (the programs add theirs one by one once the tray is ours,
  // seconds after the rest is drawn), or a few seconds: what is left of it
  // never jumps leftwards as the last ones come. Kept here, as it is, while
  // on the taskbar.
  const TRAY_KEPT = 'trayWidth'
  let trayHold = null
  try {
    const kept = JSON.parse(localStorage.getItem(TRAY_KEPT) || 'null')
    if (kept?.w > 0 && kept.w < 2000 && kept.n > 0) trayHold = kept
  } catch {}
  let trayKept = ''
  function holdTray() {
    if (trayHold && (trayIcons.length >= trayHold.n || performance.now() > 8000)) trayHold = null
    trayBox.style.minWidth = trayHold ? `${trayHold.w}px` : ''
    trayBox.style.justifyContent = trayHold ? 'flex-end' : ''
    if (trayHold) return
    requestAnimationFrame(() => {
      if (home() !== 'taskbar' || !trayIcons.length) return
      const w = Math.round(trayBox.getBoundingClientRect().width)
      const kept = JSON.stringify({ w, n: trayIcons.length })
      if (w <= 0 || kept === trayKept) return
      trayKept = kept
      try {
        localStorage.setItem(TRAY_KEPT, kept)
      } catch {}
    })
  }
  setTimeout(holdTray, 8100)

  function drawTray() {
    const icons = orderedTray()
    const folded = icons.filter(i => !isOut(i))
    if (!folded.length) trayOpen = false
    const drawn = JSON.stringify([icons.map(i => [i.key, i.png.length, i.png.slice(-24), i.tip, isOut(i), i.reach]), trayOpen])
    if (drawn !== trayDrawn) {
      trayDrawn = drawn
      chevron.hidden = !folded.length
      chevron.classList.toggle('open', trayOpen)
      chevron.setAttribute('aria-label', t(lang, 'taskbar.showHidden'))
      chevron.setAttribute('aria-expanded', String(trayOpen))
      outBox.replaceChildren(...icons.filter(isOut).map(trayIcon))
      flyout.replaceChildren(...folded.map(trayIcon))
      flyout.hidden = !trayOpen
      if (trayBox.firstChild !== flyout) trayBox.replaceChildren(flyout, chevron, outBox)
    }
    holdTray()
    requestAnimationFrame(sendTrayRects)
  }

  function openTray(open) {
    if (trayOpen === open) return
    trayOpen = open
    trayBox.classList.toggle('upper', isSide() && trayBox.getBoundingClientRect().top < innerHeight / 2)
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

  // The icons' pictures kept here: taskbar.rs (send_icons) sends each once,
  // and again only when it is another; a list without one's png means the
  // one kept.
  const trayPngs = new Map()
  window.pet.onTray(list => {
    trayIcons = (Array.isArray(list) ? list : []).map(i => {
      if (i.png === undefined) i.png = trayPngs.get(i.key) ?? ''
      trayPngs.set(i.key, i.png)
      return i
    })
    for (const key of [...trayPngs.keys()]) if (!trayIcons.some(i => i.key === key)) trayPngs.delete(key)
    if (isOn() && home() === 'taskbar') drawTray()
  })

  // --- The keyboard on the taskbar (Win+T, Win+B) ---------------------------------------
  // As on Windows' own taskbar: Win+T puts the keyboard on the programs'
  // buttons (the first; Win+Shift+T the last; again, the next one), Win+B
  // on the tray (^ first, then the icons kept out). The arrows along the bar
  // go from one to the next (up and down on a side bar; Home, End), Tab
  // goes between the two, Enter or Space is a press, Shift+F10 or the menu
  // key a right press, Esc gives the keyboard back to the window that had
  // it. What a press opens over a button (its windows to pick from, its
  // menu, the folded tray icons) is gone through the same way, any arrow
  // along it, Esc back to the button. A ring round the one it is on, as
  // Windows draws it (style.css .kfocus). The keys come from the hook that
  // takes them from Windows' put-away taskbar (winkeys.rs), which has given
  // her window the keyboard already; a press that brings another window to
  // the front takes it away again (island:blur), and that ends it too.
  // Where it is: the part ('apps', 'tray'; 'pop', 'fly' for what opened
  // over them), the one it is on (by its key, and its place), the part and
  // the one it came from into what opened, and when the settings were
  // closed for it.
  let keyPart = null
  let keyAt = null
  let keyFrom = null
  let keyedAt = 0
  // Win pressed on the strip: the keyboard given back while it is held
  // (winkeys.rs pause), the ring kept for a Win+T or Win+B after it.
  let keyPaused = false
  const keyId = node => node.dataset.app ?? node.dataset.tray ?? node.dataset.win ?? node.dataset.act ?? (node === chevron ? '^' : '')
  function keyNodes(part) {
    if (part === 'apps') return [...windowsBox.querySelectorAll('[data-app]')]
    if (part === 'tray') return [...(chevron.hidden ? [] : [chevron]), ...outBox.querySelectorAll('[data-tray]')]
    if (part === 'pop') return pop.hidden ? [] : [...pop.querySelectorAll('[data-win], [data-act]')]
    if (part === 'fly') return trayOpen ? [...flyout.querySelectorAll('[data-tray]')] : []
    return []
  }

  // The ring on the one it is at, found again after the bar is drawn anew
  // (by its key, else at its place); what opened over a button gone, back
  // on that button.
  function keyRing() {
    if ((keyPart === 'pop' || keyPart === 'fly') && !keyNodes(keyPart).length) {
      keyPart = keyFrom?.part || 'apps'
      keyAt = keyFrom?.at || null
    }
    const nodes = keyNodes(keyPart)
    const node = nodes.find(n => keyId(n) === keyAt?.id) || nodes[Math.min(keyAt?.i ?? 0, nodes.length - 1)] || null
    for (const n of barRest.querySelectorAll('.kfocus')) if (n !== node) n.classList.remove('kfocus')
    if (!node) return
    keyAt = { id: keyId(node), i: nodes.indexOf(node) }
    node.classList.add('kfocus')
    node.tabIndex = -1
    if (!node.hasAttribute('role')) node.setAttribute('role', 'button')
    // A window's card by its title (its ✕ is no part of its name).
    if (node.dataset.win) node.setAttribute('aria-label', node.querySelector('.wt')?.textContent || '')
    if (document.activeElement !== node) node.focus({ preventScroll: true })
    // In sight in a side bar's programs' column, should it scroll.
    if (isSide() && windowsBox.contains(node)) {
      const box = windowsBox.getBoundingClientRect()
      const r = node.getBoundingClientRect()
      windowsBox.scrollTop += r.top < box.top ? r.top - box.top : r.bottom > box.bottom ? r.bottom - box.bottom : 0
    }
  }
  // Drawn anew (the windows changed, a tray icon came or went): the ring found again.
  new MutationObserver(() => keyPart && keyRing()).observe(barRest, { childList: true, subtree: true })

  function keyStep(by) {
    const nodes = keyNodes(keyPart)
    if (!nodes.length) return
    const at = Math.max(0, nodes.findIndex(n => keyId(n) === keyAt?.id))
    keyAt = { id: keyId(nodes[(at + by + nodes.length) % nodes.length]) }
    keyRing()
  }

  // Into what a press opened over a button, its first.
  function keyInto(part) {
    keyFrom = { part: keyPart, at: keyAt }
    keyPart = part
    keyAt = null
    keyRing()
  }

  function keysOff() {
    keyPaused = false
    if (!keyPart) return
    keyPart = keyAt = keyFrom = null
    keyRing()
    if (barRest.contains(document.activeElement)) document.activeElement.blur()
    window.pet.keyboard(false)
  }

  // Enter, Space: as a press with the pointer, by the same handlers (a
  // program started, its window to the front, its windows listed; a window
  // or an item picked from them; a tray icon's program told; ^ opening the
  // folded ones). A program's button pressed at once (pressApp), not by a
  // click: the pointer's waits for the keys held (taskbar_held), and its
  // list would not be open yet to go into.
  function keyPress(node) {
    if (keyPart === 'apps') {
      const app = appOf(node.dataset.app)
      if (app) pressApp(app, 'press')
      if (popFor?.app === node.dataset.app && !pop.hidden) keyInto('pop')
      return
    }
    if (keyPart === 'pop') {
      node.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, cancelable: true, button: 0 }))
      return keyRing()
    }
    if (node === chevron) {
      openTray(!trayOpen)
      if (trayOpen) keyInto('fly')
      return
    }
    window.pet.taskbar.tray(node.dataset.tray, 'leftDown')
    window.pet.taskbar.tray(node.dataset.tray, 'leftUp')
    if (keyPart === 'fly') {
      openTray(false)
      keyRing()
    }
  }

  // Shift+F10, the menu key: as a right press (a program's menu, the keyboard
  // in it; a tray icon's program's own).
  function keyMenu(node) {
    if (keyPart === 'apps') {
      const app = appOf(node.dataset.app)
      if (app) pressApp(app, 'menu')
      if (popFor?.kind === 'menu' && !pop.hidden) keyInto('pop')
    } else if (node.dataset.tray) {
      window.pet.taskbar.tray(node.dataset.tray, 'rightDown')
      window.pet.taskbar.tray(node.dataset.tray, 'rightUp')
    }
  }

  // Esc: out of what opened, back on its button; else the keyboard given back.
  function keyBack() {
    if (keyPart === 'pop') closePop()
    else if (keyPart === 'fly') openTray(false)
    else return keysOff()
    keyRing()
  }

  window.pet.onFocusBar(({ part, back } = {}) => {
    // Win let go after something else than Win+T or Win+B (winkeys.rs win_up).
    if (part === 'end') return keysOff()
    keyPaused = false
    barRest.setAttribute('aria-label', t(lang, 'taskbar.aria'))
    if (!isOn() || home() !== 'taskbar' || editLayout) return window.pet.keyboard(false)
    // The settings open: closed (theirs went back to the window before as
    // they closed, which blurs this one); the keyboard comes once Win is let
    // go (winkeys.rs).
    keyedAt = 0
    if (isSetting()) {
      keyedAt = Date.now()
      window.Settings?.close?.('keys')
    }
    const want = part === 'tray' ? 'tray' : 'apps'
    // Win+T again with the keyboard on the buttons: the next one (Shift: the one before), as Windows' does.
    const again = keyPart === 'apps' && want === 'apps'
    if (popFor) closePop()
    if (trayOpen) openTray(false)
    keyPart = want
    keyFrom = null
    const nodes = keyNodes(want)
    if (!nodes.length) return keysOff()
    if (again) return keyStep(back ? -1 : 1)
    keyAt = { id: keyId(nodes[back ? nodes.length - 1 : 0]) }
    keyRing()
  })
  // Gone to another window: over. Not for the settings closing as it began
  // (keyedAt: then).
  window.pet.onBlur(() => keyPart && !keyPaused && Date.now() - keyedAt > 600 && keysOff())

  document.addEventListener(
    'keydown',
    e => {
      // Win pressed with the keyboard here (on the strip, or in the
      // settings): given back while Win is held, for what comes with it
      // reaches the hook only so (winkeys.rs); the settings close with it.
      if (e.key === 'Meta' && !e.repeat && home() === 'taskbar' && (keyPart || isSetting())) {
        keyPaused = !!keyPart
        return window.pet.taskbar.keysPause()
      }
      // Editing the taskbar's layout: Esc closes what the picker has open
      // (picker.js: a drag, its popover, the search's words, the picker),
      // then ends it, as 完成: kept as it goes, there is no 取消 (#53).
      if (editLayout && e.key === 'Escape') {
        e.preventDefault()
        if (window.Picker?.escape()) return
        return endEdit(true)
      }
      if (!keyPart) return
      // The settings opened meanwhile (a press on her capsule): the keys are theirs.
      if (isSetting()) return keysOff()
      const k = e.key
      const node = keyNodes(keyPart).find(n => keyId(n) === keyAt?.id)
      // Along the bar, or in what opened over it any way.
      const [prev, next] = keyPart === 'pop' || keyPart === 'fly' ? [null, null] : isSide() ? ['ArrowUp', 'ArrowDown'] : ['ArrowLeft', 'ArrowRight']
      if (next ? k === next : k === 'ArrowRight' || k === 'ArrowDown') keyStep(1)
      else if (prev ? k === prev : k === 'ArrowLeft' || k === 'ArrowUp') keyStep(-1)
      else if (k === 'Home' || k === 'End') {
        const nodes = keyNodes(keyPart)
        if (nodes.length) keyAt = { id: keyId(nodes[k === 'Home' ? 0 : nodes.length - 1]) }
        keyRing()
      } else if (k === 'Tab' && (keyPart === 'apps' || keyPart === 'tray')) {
        const other = keyPart === 'apps' ? 'tray' : 'apps'
        if (keyNodes(other).length) {
          keyPart = other
          keyAt = null
          keyRing()
        }
      } else if ((k === 'Enter' || k === ' ') && node) {
        if (!e.repeat) keyPress(node)
      } else if ((k === 'ContextMenu' || (k === 'F10' && e.shiftKey)) && node) keyMenu(node)
      else if (k === 'Escape') keyBack()
      else if (k !== 'Tab') return
      e.preventDefault()
      e.stopPropagation()
    },
    true,
  )

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
    // The folded ones in their grid; those kept out along the bar.
    const a = out ? along(x, y) : x
    const [start, size] = out && isSide() ? [r.top, r.height] : [r.left, r.width]
    return { out, target: mine.dataset.name, after: a > start + size / 2, slot: mine }
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

  // The taskbar's own buttons open what Windows' do: Start, and from the
  // clock the notifications and the calendar (once the button is up).
  barRest.addEventListener('click', e => {
    const what = e.target.closest('[data-bar]')?.dataset.bar
    if (what && what !== 'settings') window.pet.taskbar.open(what)
    if (what === 'ime' && imeOpen) closeImeList()
    // A widget put there: its card, or closed again; a carousel's, the one
    // it shows; a stack: its others opened over it, or closed again.
    const widget = e.target.closest('.tw-widget, .tw-plugin, .tw-carousel')
    if (widget) wgFor === widget.dataset.entry ? closeWidgetPop() : openWidgetPop(widget.dataset.entry)
    const stack = e.target.closest('.tw-stack')
    if (stack) stackFor === stack.dataset.entry ? closeStackPop() : openStackPop(stack.dataset.entry)
  })
  // A right press: a tray icon's program has it; on the input method, the
  // input methods to pick from; on a widget put there, its card; on the
  // volume, the network or the clock, the menu Windows' own taskbar has
  // there (zhoupengjie/Wakuwaku#36: tray.rs part_menu); elsewhere, her menu.
  barRest.addEventListener('contextmenu', e => {
    e.preventDefault()
    if (trayKey(e)) return
    if (wgPop.contains(e.target) || wmenu.contains(e.target)) return
    // A widget put there (one, or several in one place): its menu.
    const widget = e.target.closest('.tw-widget, .tw-plugin, .tw-group')
    if (widget) return openMenu({ where: 'bar', entry: widget.dataset.entry }, widget)
    if (e.target.closest('[data-bar="ime"]')) return imeOpen ? closeImeList() : openImeList()
    const mark = quickBox.contains(e.target) ? quickMarkAt(e.clientX, e.clientY) : null
    window.pet.menu(mark === volMark ? 'volume' : mark === netMark ? 'network' : clockBox.contains(e.target) ? 'clock' : null)
  })
  // Which of the quick settings' marks a press on its button is for: the
  // nearest along the strip, each having the button's height (its width
  // along a side), as Windows' own do. The battery's menu stays hers.
  function quickMarkAt(x, y) {
    const a = along(x, y)
    let nearest = null
    let gap = Infinity
    for (const mark of [netMark, volMark, batMark]) {
      if (mark.hidden) continue
      const r = mark.getBoundingClientRect()
      const d = a < lo(r) ? lo(r) - a : a > hi(r) ? a - hi(r) : 0
      if (d < gap) [nearest, gap] = [mark, d]
    }
    return nearest
  }
  // It takes the pointer while it is under it (the window lets clicks through elsewhere).
  barRest.addEventListener('mouseenter', () => window.pet.hover(true))
  barRest.addEventListener('mouseleave', () => window.pet.hover(false))

  // --- Editing the layout: the parts dragged about on the strip itself -------------------
  // As KDE edits a panel (the picker of 2026-10-17): each part outlined, the
  // three runs shown (an empty one a place to drop into), a bar above the
  // strip to open the picker (picker.js: the widgets there are, a sidebar at
  // the screen's side, to drag onto the strip, into her capsule or onto the
  // desktop) and to be done. What changes is kept as it goes (update:
  // picker.js persist), so there is no 取消: a widget taken off comes back by
  // its toast's 撤销. The drags, the popover over a widget and her capsule's
  // panel are picker.js's: the strip's presses go there while editing. The
  // parts' own presses are held back meanwhile; her island lets the pointer
  // through to its slot, to be dragged by it.
  const editBar = el('div', 'tbedit')

  // A new widget's id, as settings.rs takes it: "w" and up to 15 of a-z 0-9.
  const newId = () => `w${Date.now().toString(36).slice(-6)}${Math.random().toString(36).slice(2, 6)}`

  // Pointer in or out of what editing puts over the strip: the window takes it.
  editBar.addEventListener('mouseenter', () => window.pet.hover(true))
  editBar.addEventListener('mouseleave', () => window.pet.hover(false))
  // And whatever the page hears of the pointer: where they are goes to the
  // Rust side as they change (island.rs set_editing), and the window takes
  // the mouse over them there (pointer.rs solid). By enter and leave alone
  // its mode flipped while the pointer was on the bar, a press going to the
  // window under it and only its release coming here, so 完成 and 取消 did
  // nothing (zhoupengjie/Wakuwaku#53). A couple of px more all round.
  // picker.js: the picker, its popover, the capsule's panel and a toast
  // with 撤销 too, editing or not, in either home (on: the layout being
  // edited, for her menu's 退出编辑; the rects whatever it is).
  let solidSent = ''
  function sendSolid() {
    if (!isOn()) return
    const extra = window.Picker?.solids() || []
    const on = !!editLayout
    // Settings are a large interactive surface, often much bigger than the
    // capsule. Keep the native window catching the mouse over them even if
    // WebView2 misses an enter/leave while its click-through mode changes.
    // Put this first because Rust caps the list at eight rectangles.
    const settings = window.Settings?.isOpen?.() ? window.Settings.layer : null
    const rects = [...(settings ? [settings] : []), ...(editLayout ? [editBar] : []), ...extra]
      .filter(n => n.isConnected)
      .map(n => n.getBoundingClientRect())
      .filter(r => r.width && r.height)
      .map(r => [Math.floor(r.left) - 2, Math.floor(r.top) - 2, Math.ceil(r.width) + 4, Math.ceil(r.height) + 4])
    const sent = JSON.stringify([on, rects, devicePixelRatio || 1])
    if (sent === solidSent) return
    solidSent = sent
    window.pet.editing(on, rects)
  }
  const solidWatch = new ResizeObserver(() => sendSolid())
  solidWatch.observe(editBar)
  // The keyboard while editing, for Esc to end it (taken a moment after
  // it starts: from her menu, which gives the keyboard back as it closes).
  let editKeys = false
  // A press whose button went down where the window let it through and
  // came up on one of the bar's buttons: only its mouseup here, no click
  // (#53). Done as the click would have been.
  let pressedHere = false
  for (const type of ['pointerdown', 'mousedown']) document.addEventListener(type, () => (pressedHere = true), true)
  document.addEventListener(
    'mouseup',
    e => {
      const pressed = pressedHere
      pressedHere = false
      const what = !pressed && e.button === 0 && editLayout && editBar.contains(e.target) && e.target.closest('[data-edit]')?.dataset.edit
      if (what) editAct(what)
    },
    true,
  )

  // opts.picker: from her menu's 添加小工具…, the picker open with it.
  function editTaskbar(opts) {
    if (home() !== 'taskbar') return
    if (editLayout) return opts?.picker && window.Picker?.open()
    if (isSetting()) window.Settings?.close?.('edit')
    if (wgFor) closeWidgetPop()
    if (popFor) closePop()
    if (stackFor) closeStackPop()
    if (menuFor) closeMenu()
    editLayout = JSON.parse(JSON.stringify(tbLayout()))
    // Each part's name: shown on one with nothing in it now, and dragged by.
    for (const [part, node] of Object.entries(PART_NODES)) node.dataset.label = t(lang, `taskbar.part.${part}`)
    // Closed, should it have been open under the pointer (its menu came
    // from it): it lets the pointer through now, and would hear no leaving.
    clearTimeout(hoverTimer)
    isHover = false
    body.classList.add('tb-editing')
    drawEditBar()
    document.body.append(editBar)
    arranged = ''
    update()
    if (opts?.picker) window.Picker?.open()
    requestAnimationFrame(sendSolid)
    setTimeout(() => {
      if (!editLayout || editKeys) return
      editKeys = true
      window.pet.keyboard(true)
    }, 250)
  }

  // Its words (editing the taskbar, kept as it goes), 添加小工具 (pressed
  // while the picker is open) and 完成.
  function drawEditBar() {
    const button = (what, label) => {
      const b = el('button', `tbe tbe-${what}`, label)
      b.type = 'button'
      b.dataset.edit = what
      return b
    }
    const add = button('add', t(lang, 'taskbar.edit.add'))
    add.setAttribute('aria-pressed', String(!!window.Picker?.isOpen()))
    editBar.replaceChildren(el('span', 'hint', t(lang, 'taskbar.edit.title')), el('span', 'hint kept', t(lang, 'taskbar.edit.kept')), add, button('done', t(lang, 'taskbar.edit.done')))
  }

  // What a layout puts on the strip, as placeWidget takes them: its widgets
  // and plugins, and the monitor while it has its part.
  const barItems = layout => [...Object.values(layout).flatMap(run => run.filter(p => typeof p !== 'string').flatMap(Widgets.membersOf).map(m => (m.type === 'plugin' ? { plugin: m.of } : { id: m.of }))), ...(hasMonitorPart(layout) ? [{ id: 'monitor' }] : [])]

  function endEdit(keep) {
    if (!editLayout) return
    const layout = editLayout
    editLayout = null
    window.Picker?.ended()
    body.classList.remove('tb-editing')
    // The pointer on the bar as it goes (its 完成 pressed): taken out, it
    // hears no leave, and the window kept the mouse over the room above the
    // strip until the pointer left the window (the desktop there not to be
    // pressed). Left here.
    if (editBar.matches(':hover')) window.pet.hover(false)
    editBar.remove()
    sendSolid()
    if (editKeys) {
      editKeys = false
      window.pet.keyboard(false)
    }
    if (keep) {
      // Those put on the strip now are there alone (§7): off the desktop.
      const was = tbLayout(config.taskbarLayout)
      const onDeskToo = barItems(layout).filter(it => !barItems(was).some(x => keyFor(x) === keyFor(it)) && DeskGrid.has(config.deskLayout, it))
      if (onDeskToo.length) deskOff(onDeskToo)
      config.taskbarLayout = layout
      const patch = { taskbarLayout: layout }
      // A plugin put there that was not running: it runs now.
      const plugins = config.plugins && typeof config.plugins === 'object' ? config.plugins : {}
      const off = [...new Set(Object.values(layout).flatMap(run => run.filter(p => p.type === 'plugin').map(p => p.of)))].filter(id => plugins[id]?.on !== true)
      if (off.length) patch.plugins = { ...plugins, ...Object.fromEntries(off.map(id => [id, { ...(plugins[id] || {}), on: true }])) }
      // Then which of the plugins put there want their settings first.
      Promise.resolve(window.pet.settings.set(patch)).then(() => askPlugins(true))
    }
    arranged = ''
    update()
  }

  editBar.addEventListener('click', e => editAct(e.target.closest('[data-edit]')?.dataset.edit))

  // One of the bar's buttons, pressed (a click, or a release alone: above).
  function editAct(what) {
    if (what === 'done') endEdit(true)
    else if (what === 'add') window.Picker?.toggle()
  }

  // The parts as Windows 11 has them; the widgets kept, where they were
  // (the picker's ⋯ 恢复默认).
  function resetParts() {
    editLayout = tbLayout(Widgets.withParts(TB_PRESETS.win11, tbLayout()))
    arranged = ''
    update()
  }

  // While editing, the parts' own presses (Start, the buttons, the tray, the
  // menu) are held back: they are being moved, not used. A right press
  // anywhere on the strip opens her menu, not a part's: 退出编辑 is its
  // first item then (tray.rs menu), a way out whatever the bar does (#53;
  // held back too, the strip's right press opened nothing while editing, the
  // test VM showed).
  for (const type of ['click', 'auxclick', 'dblclick', 'contextmenu', 'pointerup']) {
    barRest.addEventListener(
      type,
      e => {
        if (!editLayout) return
        if (type !== 'pointerup') e.preventDefault()
        e.stopPropagation()
        if (type === 'contextmenu') window.pet.menu()
      },
      true,
    )
  }
  // The same on the bar.
  editBar.addEventListener('contextmenu', e => {
    e.preventDefault()
    if (editLayout) window.pet.menu()
  })
  // A press on a part or a widget: picker.js drags it (on the strip, into
  // her capsule, off it, onto the desktop), or, let go where it was, opens
  // a widget's popover or her capsule's panel.
  barRest.addEventListener(
    'pointerdown',
    e => {
      if (!editLayout) return
      e.preventDefault()
      e.stopPropagation()
      if (e.button === 0) window.Picker?.pressStrip(e, e.target.closest('[data-part]'))
    },
    true,
  )

  function update() {
    clearInterval(clockTimer)
    // While she is being pulled out, the pull has the island.
    if (!isOn() || (pull?.started && !pull.carrying) || isAbsorbing) return
    // A prompt answered: the island closes rather than lingering open.
    if (view === 'ask' && !isAsking()) nudge = null
    const was = view
    view = isSetting() ? 'settings' : isAsking() ? 'ask' : isHover || isNudging() || peek ? 'expanded' : 'compact'
    // Open, for whatever reason: what happened has been seen (its dot and
    // its ring go), and what the island would have shown by itself is in it.
    if (view !== 'compact') {
      news = null
      if (isRinging()) stopRing()
    }
    if (view === 'expanded') takePending()
    // Closed: a card unfolded in it folded again, its widget's menu gone.
    if (view !== 'expanded' && was === 'expanded') {
      cardFor = null
      linesTop = 0
      if (menuFor?.where === 'isle') {
        menuFor = null
        wmenu.hidden = true
        menuRoom = 0
      }
    }
    // In the notch and closed it is out of the way: the pointer passes to
    // what is under it (the notch's own app), and a hover opens nothing.
    body.classList.toggle('aside', !!screenTop?.notch && view === 'compact')
    // Opened in the notch (from her menu) and the pointer never comes: back
    // after a while, as a peek goes back (PEEK_MS). Counted from their
    // opening, not from each redraw (the widgets redraw every few seconds).
    // Under a still pointer they are not being used: only a move, a press, a
    // wheel or a key in them since they opened is (settingsUsed): they opened
    // under the pointer that had just chosen them from her menu, and stayed.
    if (screenTop?.notch && view === 'settings' && !settingsUsed) {
      settingsIdleTimer ||= setTimeout(() => {
        settingsIdleTimer = 0
        if (!settingsUsed && isSetting()) window.Settings?.close('unused')
      }, PEEK_MS)
    } else if (view !== 'settings') {
      clearTimeout(settingsIdleTimer)
      settingsIdleTimer = 0
      settingsUsed = false
      settingsFrom = null
    }
    island.dataset.view = view
    // Closing on the taskbar, the blur under it goes first, then it shrinks.
    const late = plateSent && home() === 'taskbar' && view === 'compact' && was !== 'compact' && !stillMotion() ? PLATE_OUT_MS : 0
    holdGround(was, late)
    // The deeper ground at once as it opens; changing, over a moment.
    setDeep(was === view || view === 'compact')
    setInk()
    island.style.setProperty('--ring', hue()[now.mood])
    island.classList.toggle('wants', now.mood === 'waiting')
    seatPanel()
    body.classList.toggle('has-her', isHome())
    for (const e of ['top', 'left', 'right']) body.classList.toggle(`tb-${e}`, tbEdge() === e)
    body.classList.toggle('hinting', isHinting() && isHome())
    document.documentElement.style.setProperty('--hint-accent', hintAccent())
    drawNews()
    body.classList.toggle('tb-side', isSide())
    body.classList.toggle('compact-her', view === 'compact' || view === 'settings')
    if (spriteUrl) sheetImage(herSheet, spriteUrl)
    fillCompact()
    fillExpanded()
    fillBar()

    const want = sizeOf(view)
    if (view === 'settings' && isSide()) placeSideSettings(want.height)
    // Reaching for her keeps the room to reach in; the picker and the
    // capsule's panel in the island's home want theirs (picker.js).
    const grows = setRoom(isReaching ? PULL_ROOM : window.Picker ? window.Picker.roomOver(roomFor(want), baseRoom()) : roomFor(want))
    // The window has its room before the island (and she) grow into it.
    const grow = () => {
      const resized = island.style.width !== `${want.width}px` || island.style.height !== `${want.height}px`
      // Where it is, for the plate under it to stay covered as it changes.
      const from = resized && plateSent ? island.getBoundingClientRect() : null
      const fromRadius = from ? parseFloat(getComputedStyle(island).borderTopLeftRadius) : 0
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
      placePlate(resized, from, fromRadius)
      placeNews()
      placeFlow()
    }
    if (grows) setTimeout(grow, 40)
    else if (late) {
      hidePlate(PLATE_OUT_MS)
      setTimeout(grow, late)
    } else grow()

    // The clocks run: the turn's, the step's, the other sessions'. Their words
    // may grow (a step timed once it has run a while), so the shape follows.
    const isTimed = time() || now.stepSince || Status.othersOf(now.list, now).some(x => Status.time(x))
    if (isTimed && (view === 'compact' || view === 'expanded')) clockTimer = setInterval(tickTimes, 1000)
    spin()
    // Editing the layout: kept as it goes (picker.js); where its bar and the
    // rest are now (the picker's too, editing or not), for the Rust side.
    if (editLayout) window.Picker?.persist()
    sendSolid()
    // Up out of the way of a window under it, or down (setStow).
    setStow()
    // Opened on the taskbar: over any window come over hers.
    openedUp('island', view !== 'compact')
  }

  // Each second while something is timed: the times' words alone, put in
  // place (the compact island's clock); the island drawn
  // anew (update) only when a word grew or shrank, as the shape follows the
  // words, or while the island is open, whose lines carry the step's time
  // too. It was drawn anew every second (2026-10-11).
  function tickTimes() {
    if (view !== 'compact') return update()
    let resized = false
    const put = (node, text) => {
      if (!node || node.textContent === text) return
      if (node.textContent.length !== text.length) resized = true
      node.textContent = text
    }
    put(compact.querySelector('[data-live="time"]'), time())
    if (resized) update()
  }

  // Open by itself for ms: what the 「她有事告诉你时」 setting governs as
  // long as the hold says (Flow.openFor, zhoupengjie/Wakuwaku#65), Infinity
  // until seen: open until the pointer has been over it (seenNudge), its own
  // time (after: NUDGE_MS, NUDGE_READ_MS) from then on. kind: what opened it
  // (an ending's goes as its session is at work again: movedOn).
  function nudgeFor(clip, text, ms = NUDGE_MS, widget = null, { kind = null, after = ms } = {}) {
    // In the notch nothing opens by itself (done, failed, a hello, a widget):
    // only a prompt, which Claude waits on (the user's call, 2026-10-14).
    if (screenTop?.notch) return update()
    nudge = { until: Date.now() + ms, clip, text, widget, kind, after: Number.isFinite(after) ? after : NUDGE_MS }
    clearTimeout(nudgeTimer)
    if (Number.isFinite(ms)) nudgeTimer = setTimeout(endNudgeLater, ms)
    update()
  }

  // Its time up.
  function endNudgeLater() {
    nudge = null
    update()
  }

  // Held until seen, and the pointer has come over it: open by itself from
  // now on as any other, its own time from here and the pointer resting on
  // it and leaving closing it (mouseleave, below). A pointer only passing
  // across has seen it too (main counts it so: island.rs hover).
  function seenNudge() {
    if (!nudge || Number.isFinite(nudge.until)) return
    nudge.until = Date.now() + nudge.after
    clearTimeout(nudgeTimer)
    nudgeTimer = setTimeout(endNudgeLater, nudge.after)
  }

  // Open by itself no more: what opened it has been seen.
  function endNudge() {
    nudge = null
    clearTimeout(nudgeTimer)
  }

  // A press on the island while it is open by itself (zhoupengjie/Wakuwaku#45):
  // what a widget's words point to opens (a letter, the updates), as it
  // always did; any other press closes it, rather than opening the settings
  // and leaving it to its time.
  function pressNudge() {
    const open = nudge?.widget?.open
    endNudge()
    // A word that points somewhere (an agent answered about a letter): there.
    if (open?.tab === 'mail') {
      window.Settings?.openLetter?.({ account: open.account, uid: open.uid, folder: open.folder || 'inbox' })
      return window.pet.openSettings('mail')
    }
    // A page, or a part of one (a new version: the updates).
    if (open?.tab) return window.pet.openSettings(open.tab)
    dismiss()
  }

  // Closed under the pointer: its hover forgotten, and none taken again
  // until it has left the island (mouseleave) and come back.
  function dismiss() {
    endNudge()
    clearTimeout(hoverTimer)
    isHover = false
    dismissed = true
    update()
  }

  // --- The flow light (flow.js) -------------------------------------------------------
  // What opened the island by itself (a turn done, changes to review, an
  // error, a hello, a plugin's nudge) keeps it closed now: a ring of colour
  // runs round her capsule (round her portrait on the black strip, where
  // she has none) twice, in steps (15 a second), stays lit and still
  // until the hold setting's time from the event is over (until seen: until
  // the island opens; zhoupengjie/Wakuwaku#65) and fades, and a dot
  // stays on her portrait until the island has been open for any reason
  // (seen). What it would have shown is there as it opens next (pending):
  // an ending's own words are its session's anyway. Only a session waiting on the person
  // opens it, or all of them with endCue open (all as before). What
  // happens inside a turn (a tool failed, an item of its list done) only
  // plays her clip (playReaction): no ring, no dot.
  //
  // The ring: a frame by the island in its row (the island clips what is in
  // it), laid over the island by anchor positioning (style.css #flow), so it
  // follows the capsule's size; cut to a 2 px ring by its mask, a square of
  // conic gradient turning in it, and under it the same blurred on a dark
  // ground (glow). Its own element, never drawn anew by update(): the island
  // drawn again (the widgets' words every few seconds, the clock) neither
  // starts it over nor makes it flicker. Animated by the page (Web
  // Animations), only its turning (transform) and its light (opacity);
  // held still, nothing animated (holdRing); gone (display: none,
  // will-change with it) once it has played.
  // The ring's colours by Windows' mode (the strip's, or the screen's at
  // the top): deeper in the light, where the dark set's pale ones are lost.
  const flowLight = () => modeNow() === 'light'
  // A ring would not be seen now, or should not turn: something full
  // screen, nobody at the computer, do not disturb, her window hidden, a
  // Mac's notch (nothing there moves by itself). The dot alone then, seen
  // once she is; no ring afterwards to catch up.
  const flowBlocked = () => fullScreen || quiet || config.dnd === true || document.hidden || !!screenTop?.notch

  // Something happened: kind (waiting, done, review, error, hello, plugin,
  // notice, process), and what the island would have opened with ({ clip,
  // text, ms, widget }; a reaction's name, react).
  function cue(kind, show) {
    const how = Flow.cueFor(kind, { setting: Flow.endCue(config), view, blocked: flowBlocked(), herOut: herTalks() })
    // Inside a turn: her clip, and nothing kept, no ring, no dot.
    if (how === 'clip') return playReaction(show.react)
    // The newest wins: what is kept for the next look is this one's (an
    // ending's words are its session's: the open island has them), or none
    // when it shows now; a prompt leaves it for after.
    if (kind !== 'waiting') pending = how && how !== 'open' && how !== 'show' && !Flow.isEnding(kind) ? { ...show, until: Date.now() + Flow.keepFor(kind, config.hold) } : null
    // Opened by itself: as long as the hold says (it stayed its few seconds
    // whatever was chosen, zhoupengjie/Wakuwaku#65). Already open (show):
    // it is being looked at, its own few seconds.
    if (how === 'open' || how === 'show') return nudgeFor(show.clip, show.text, how === 'open' ? Flow.openFor(kind, config.hold, show.ms) : show.ms, show.widget, { kind, after: show.ms })
    if (!how || how === 'later') return update()
    news = { kind, colour: show.widget?.color, shown: news?.shown === true }
    if (how === 'dot') showNews('in')
    else ring(kind)
    update()
  }

  // A reaction inside a turn (a tool failed, an item of its list done): her
  // portrait plays its clip as many times as she does out on the desktop
  // (Sprite.REACTIONS), then her mood's again; the island stays as it is.
  function playReaction(react) {
    const r = REACTIONS[react]
    const clip = r && CLIPS[r.clip]
    if (!clip) return
    const span = clip.frames * clip.ms * (r.times || 1)
    reacting = { clip: r.clip, until: Date.now() + span }
    clearTimeout(reactTimer)
    reactTimer = setTimeout(() => {
      reacting = null
      herClip = null
      animateHer()
    }, span)
    herClip = null
    animateHer()
  }

  // The colours for a kind, on Windows' mode now: a plugin's from its own.
  const flowStops = kind => Flow.gradient(Flow.palette(kind, flowLight(), news?.colour, hintAccent()))

  function ring(kind) {
    // Held lit after its turns for as long as the hold says (until seen: until
    // the island opens), zhoupengjie/Wakuwaku#65.
    const step = Flow.play(flowState, kind, performance.now(), { still: stillMotionNow(), hold: Flow.holdMs(config.hold) })
    flowState = step.state
    if (step.act === 'start') {
      // A dot from before goes, and comes again in the new colour as it fades.
      news.shown = false
      for (const b of blends) b.anim.cancel()
      blends = []
      for (const s of spins) s.firstChild.style.background = flowStops(kind)
      flow.classList.toggle('glow', Flow.glows(flowLight(), step.ring.still))
      flow.classList.add('on')
      placeFlow()
      animateRing(step.ring)
    } else if (step.act === 'blend') {
      blendTo(kind)
      // Taken on as it faded (its dot come already, its session at work
      // again a moment before): the dot goes again, and comes as it fades
      // this time.
      if (step.retimed && news?.shown && step.ring.hold > 0 && step.ring.dotAt > performance.now()) {
        news.shown = false
        drawNews()
      }
      if (step.retimed) animateRing(step.ring)
    } else if (step.act === 'hop' || step.act === 'swap') {
      showNews(step.act)
    }
    // hold: the ring keeps its colour; the dot has the new one as it comes.
  }

  // Its turning and its light from where they are now (Flow.frames), its
  // dot as it fades and its end, timed anew. Its turning in steps (15 a
  // second, Flow.stepsOf: each a new angle set, nothing animated between
  // them; the user, 2026-10-18), its light smoothly. Held (the hold setting,
  // zhoupengjie/Wakuwaku#65): animated only until it is held still
  // (holdRing), down again at the hold's end (fadeRing); until seen, no end
  // of its own (the island opening ends it: stopRing).
  function animateRing(r) {
    for (const a of flowAnims) a.cancel()
    for (const t of flowTimers) clearTimeout(t)
    flowAnims = []
    flowTimers = []
    stopStepping()
    unholdRing()
    const f = Flow.frames(r, performance.now())
    const later = (fn, ms) => Number.isFinite(ms) && flowTimers.push(setTimeout(fn, Math.max(0, ms)))
    if (f.still == null || f.still > 0) {
      flowAnims.push(flow.animate(f.light.keyframes, { delay: f.light.delay, duration: f.light.duration, fill: 'both' }))
      if (f.steps.length) stepRing(f.steps)
    }
    if (f.still != null) {
      if (f.still > 0) later(() => holdRing(f.rest), f.still)
      else holdRing(f.rest)
    }
    if (f.out) later(() => fadeRing(r), f.out.delay)
    later(() => showNews('in'), f.dotIn)
    later(endRing, f.end)
  }

  // Its turning in steps: the one showing now set at once, each after it at
  // its time (a timer for the next alone, so nothing wakes between them),
  // on both squares (the glow's too, when it glows).
  let stepping = 0
  function stopStepping() {
    clearTimeout(stepping)
    stepping = 0
  }
  function stepRing(steps) {
    const made = performance.now()
    const glow = flow.classList.contains('glow')
    const turning = spins.filter(s => glow || !s.closest('.flow-glow'))
    let i = 0
    const show = () => {
      stepping = 0
      // Late (a busy page, a hidden one): the step due now, those passed skipped.
      const now = performance.now() - made
      while (i + 1 < steps.length && steps[i + 1].delay <= now) i++
      for (const s of turning) s.style.transform = `rotate(${steps[i].angle}deg)`
      if (++i < steps.length) stepping = setTimeout(show, Math.max(0, steps[i].delay - (performance.now() - made)))
    }
    show()
  }

  // Held still (its turns over, its hold not yet): nothing of it running,
  // not even a light held at full (an animation filling forwards still
  // costs its frames): the ring left lit, its gradient where it stopped,
  // the glow under it the same, no will-change (held).
  function holdRing(rest) {
    for (const a of flowAnims) a.cancel()
    flowAnims = []
    stopStepping()
    flow.classList.add('held')
    flow.style.opacity = '1'
    for (const s of spins) s.style.transform = `rotate(${rest}deg)`
  }

  // Not held still: as the animations have it.
  function unholdRing() {
    flow.classList.remove('held')
    flow.style.opacity = ''
    for (const s of spins) s.style.transform = ''
  }

  // The hold over: its light down from lit (as Flow.frames has it now),
  // the gradient resting where it was held.
  function fadeRing(r) {
    const o = Flow.frames(r, performance.now()).out
    if (!o) return
    for (const a of flowAnims) a.cancel()
    flow.classList.remove('held')
    flow.style.opacity = ''
    flowAnims = [flow.animate(o.keyframes, { delay: o.delay, duration: o.duration, fill: 'both' })]
  }

  // Into another kind's colours over BLEND_MS: the upper of each square's
  // two layers takes them and comes up, then the lower has them and the
  // upper goes again. One coming mid-way: the last finished at once.
  function blendTo(kind) {
    const stops = flowStops(kind)
    for (const b of blends) settle(b)
    blends = spins.map(s => {
      const [under, over] = s.children
      over.style.background = stops
      const b = { under, stops, anim: over.animate([{ opacity: 0 }, { opacity: 1 }], { duration: stillMotionNow() ? 0 : Flow.BLEND_MS, fill: 'forwards' }) }
      b.anim.onfinish = () => settle(b)
      return b
    })
  }

  // A blend done (or cut short by the next): the lower layer has its colours.
  function settle(b) {
    b.under.style.background = b.stops
    b.anim.cancel()
  }

  // Played: nothing of it left running; the dot there if it has not come.
  function endRing() {
    for (const b of blends) settle(b)
    for (const a of flowAnims) a.cancel()
    for (const t of flowTimers) clearTimeout(t)
    flowAnims = []
    blends = []
    flowTimers = []
    stopStepping()
    unholdRing()
    flow.classList.remove('on', 'glow')
    if (news && !news.shown) showNews('in')
    // Back up out of the way, if a window under it had it there.
    setStow()
  }

  // The island opened as it played: gone at once (it would grow with the
  // island), and the next rings anew.
  function stopRing() {
    flowState = Flow.stop(flowState)
    endRing()
  }

  // The ring's frame where the island is: 3 px out all round, its corners
  // 3 px rounder than the island's own; on the black strip a circle of 36
  // round her 30 px portrait. Where anchor positioning is missing (an older
  // WebKit), set from the island's box as it is now.
  function placeFlow() {
    if (!isRinging()) return
    const round = home() === 'taskbar' && !isCapsule()
    const a = avatarBox()
    const [ax, ay] = [a.x + a.px / 2 - 18, a.y + a.px / 2 - 18]
    flow.classList.toggle('round', round)
    if (round) {
      flow.style.setProperty('--flow-x', `${ax}px`)
      flow.style.setProperty('--flow-y', `${ay}px`)
    } else {
      flow.style.setProperty('--flow-r', (island.style.borderRadius || '18px').split(/\s+/).map(r => `${(parseFloat(r) || 0) + 3}px`).join(' '))
    }
    if (ANCHORED) return
    // From the box, which leaves the island's own translate out (OLED's
    // guard): style.css moves it the same way (placed).
    flow.classList.add('placed')
    const [x, y, w, h] = round ? [island.offsetLeft + ax, island.offsetTop + ay, 36, 36] : [island.offsetLeft - 3, island.offsetTop - 3, island.offsetWidth + 6, island.offsetHeight + 6]
    Object.assign(flow.style, { left: `${x}px`, top: `${y}px`, width: `${w}px`, height: `${h}px`, right: 'auto', bottom: 'auto' })
  }

  // Her portrait in the closed island (x, y, px across): her own, or her
  // still one while she is out.
  function avatarBox() {
    if (isHome()) return { x: parseFloat(her.style.left) || 0, y: parseFloat(her.style.top) || 0, px: parseFloat(her.style.width) || TASK_HEAD }
    const still = compact.querySelector('.still-her')
    if (still) return { x: still.offsetLeft, y: still.offsetTop, px: still.offsetWidth }
    return { x: 9, y: 9, px: TASK_HEAD }
  }

  // The dot over her portrait's top-right, as the hint's (placeHer): 2 px
  // out; 1 px in on an upright capsule, whose round top would cut it.
  function placeNews() {
    if (view !== 'compact') return
    const a = avatarBox()
    const [dx, dy] = isSide() ? [a.px - 10, 1] : [a.px - 7, -2]
    Object.assign(newsDot.style, { left: `${a.x + dx}px`, top: `${a.y + dy}px` })
  }

  // The dot's colour: a mood's as the island's words have it, a hello's and
  // a notice's Windows' accent, a plugin's its own (or the accent).
  function newsColour() {
    if (Flow.isEnding(news.kind)) return hue()[news.kind]
    if (news.kind === 'plugin') return ink(news.colour) || hintAccent()
    return hintAccent()
  }

  // It comes (in: grown from nothing, a little past full size and back),
  // hops in a new colour, or only takes it (swap, and with reduced motion).
  function showNews(how) {
    if (!news) return
    const was = news.shown
    news.shown = true
    drawNews()
    const cls = !was ? 'pop' : how === 'hop' ? 'hop' : ''
    if (!cls || stillMotionNow()) return
    newsDot.classList.remove('pop', 'hop')
    void newsDot.offsetWidth
    newsDot.classList.add(cls)
  }

  function drawNews() {
    const on = !!news?.shown && view === 'compact' && !isHinting()
    body.classList.toggle('has-news', on)
    if (on) newsDot.style.setProperty('--news', newsColour())
  }

  // Opened (by a hover, mostly) with something kept: it is what the island
  // shows, for as long as it would have by itself; gone if its time is up.
  function takePending() {
    if (!pending || isNudging()) return
    const p = pending
    pending = null
    if (Date.now() > p.until) return
    nudge = { until: Date.now() + p.ms, clip: p.clip, text: p.text, widget: p.widget }
    clearTimeout(nudgeTimer)
    nudgeTimer = setTimeout(() => {
      nudge = null
      update()
    }, p.ms)
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
    hidePlate(0)
  }

  function startDropHer() {
    clearTimeout(dropTimer)
    sheetImage(dropSheet, spriteUrl)
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
    const [ix, iy] = isSide() ? [14, 24] : [24, 14]
    return [Math.min(Math.max(x, r.left + ix), r.right - ix), Math.min(Math.max(y, r.top + iy), r.bottom - iy)]
  }

  // Her seat in her home, in the page: where she springs back to.
  function seatIn(r) {
    if (home() === 'taskbar' && isSide()) return [r.left + r.width / 2, r.top + r.width / 2]
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
    // From the top of the screen (her island, a top bar) she can only come
    // down, from a bottom bar only up, from a side bar only into the screen.
    const edge = home() === 'taskbar' ? tbEdge() : 'top'
    const cx = edge === 'left' ? Math.max(x, r.right - 10) : edge === 'right' ? Math.min(x, r.left + 10) : x
    const cy = edge === 'bottom' ? Math.min(y + 30, r.top + 10) : edge === 'top' ? Math.max(y + 30, r.bottom - 10) : y
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
      // A click on her, not a pull: the settings, as anywhere on the island;
      // open by itself, it closes instead, as anywhere on it (pressNudge).
      // The click the browser sends after, on the island that held the
      // pointer, is this press too: not one to open the settings.
      if (view === 'ask') return
      if (!isNudging()) return window.pet.openSettings()
      pulledAt = Date.now()
      return pressNudge()
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
      // Down from out of the way, to reach for her.
      setStow()
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
    setStow()
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

  // At a top corner (anchor.js): the island at that side of its window
  // (style.css isl-at-left, isl-at-right), growing toward the screen's
  // middle; the picker and its toasts under it there too (picker.css).
  function placeAnchor() {
    const at = isOn() ? anchor() : 'center'
    body.classList.toggle('isl-at-left', at === 'left')
    body.classList.toggle('isl-at-right', at === 'right')
  }

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
    placeAnchor()
    drawMica()
    seatPanel()
    if (!isOn() && panel.parentElement !== stage) stage.prepend(panel)
    if (!isOn()) {
      hidePlate(0)
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
    if (!!config.oledShift !== oledOn) {
      oledOn = !!config.oledShift
      oledShift()
    }
    if (config.out !== true) isComingHome = false
    herUp = data.herUp === true
    spriteUrl = data.sprite
    second = data.second || null
    // Where island.rs put her window at the top of the screen (anchor.js).
    islandAt = Anchor.posOf(data.islandAt)
    quiet = data.quiet === true
    islandViews?.set({ on: !quiet })
    fullScreen = data.fullScreen === true
    takeWidgets(data.widgets)
    place()
    // Something new that wants you: the island opens for a moment. One that
    // is finished: as the setting says (endCue, cue): a ring round it (the
    // default), the island open for a moment as before (a longer one with
    // Claude's words to read), or only the dot. A hello and a plugin's
    // nudge the same (onReact, onNudge).
    const isReading = isDetailed() && Status.isEnding(now) && !!now.reply
    if (isOn() && before !== now.mood && NUDGE_CLIP[now.mood]) cue(now.mood, { clip: NUDGE_CLIP[now.mood], text: null, ms: isReading ? NUDGE_READ_MS : NUDGE_MS })
    else {
      if (isOn() && before !== now.mood && now.mood === 'working') movedOn()
      update()
    }
  })

  // A turn begun again (the one shown at work once more, from an ending):
  // what said how the last one ended has said it. Held long (the hold
  // setting, zhoupengjie/Wakuwaku#65: up to 10 minutes, or until seen), the
  // island open by itself for it would stay open over her at work, its ring
  // lit round it: the one closes, the other goes down into its dot as at
  // its time. Not as it settles back to rest (main's hold over, as theirs
  // is, or the pointer having passed: their own rules), nor for a hello or
  // a plugin's nudge, which no session's turn ends.
  function movedOn() {
    if (nudge && Flow.isEnding(nudge.kind)) endNudge()
    if (!isRinging() || !Flow.isEnding(flowState.ring?.kind)) return
    const next = Flow.release(flowState, performance.now())
    if (next === flowState) return
    flowState = next
    animateRing(next.ring)
  }

  // Only the widgets' words changed (the monitor's readings every couple
  // of seconds, main.rs redraw_widgets): on the strip, the readings' node
  // alone; the island drawn anew where it shows them in its lines, or when
  // another widget's words changed (the capsule shows them in turn).
  window.pet.onDeskAsk(({ what, args } = {}) => {
    if (what === 'settings') {
      const w = allWidgets.find(x => x.id === args?.widget)
      return w && openWidgetSettings(w)
    }
    if (what === 'place') deskPlace(args)
    // A board took them from the library: off the taskbar (one place).
    if (what === 'unbar' && Array.isArray(args?.items)) offBar(args.items.filter(it => it && (typeof it.id === 'string' || typeof it.plugin === 'string')).slice(0, 8))
  })

  window.pet.onWidgets(({ widgets: all }) => {
    const others = JSON.stringify(widgets)
    takeWidgets(all)
    islandViews?.widgets(allWidgets)
    if (!isOn()) return
    if (home() === 'taskbar' && view === 'compact' && JSON.stringify(widgets) === others) {
      facesBox.replaceChildren(...widgetFaces())
      drawEntries()
    } else update()
  })

  // Nobody at the keyboard or mouse for a while, or someone again: she
  // stands still, or plays on.
  window.pet.onQuiet(({ on }) => {
    quiet = on === true
    // A plugin's view in a card: away while nobody is about.
    islandViews?.set({ on: !quiet })
    herClip = null
    animateHer()
    playPortraits()
    // The blur behind the island too: none while nobody is about.
    if (isOn()) placePlate(false)
    // The carousels on the strip hold still while nobody is about, and go
    // on (their turns anew) once someone is.
    if (!quiet) for (const [id, state] of carousels) carousels.set(id, { ...state, since: Date.now() })
    tickCarousels()
    // The capsule's widgets the same: held, or taking turns again.
    spin()
  })

  // Her reactions with words: a hello rings (its words in the island as it
  // opens next, already open: in it now); a task done or a tool failed is
  // inside a turn: her portrait plays its clip, and nothing else. Set to
  // open, any of them opens the island to say it, her waving, as before.
  window.pet.onReact(({ react, say: text }) => {
    if (isOn() && text) cue(Flow.kindOfReact(react), { clip: 'waving', text: say(lang, text), ms: NUDGE_MS, react })
  })

  // An app full screen on her display, or none now: no ring while one is.
  window.pet.onFullScreen(({ on } = {}) => {
    fullScreen = on === true
  })

  // A widget asks to open the island (main checked that nothing wants you):
  // a ring in its colour, its card in the island as it opens next (or, as
  // the setting says, the island opened as before, or the dot alone). A new
  // version or a crash report (not a widget's): the dot alone, in Windows'
  // accent, the island opened next saying it; set to open, it opens.
  window.pet.onNudge(n => {
    if (!isOn()) return
    // News for a carousel on the strip that has it: it cuts in.
    for (const [id] of carousels) {
      const slide = entryNodes.get(id)?._slides?.findIndex(s => s.w?.id === n.id) ?? -1
      if (slide >= 0) turnCarousel(id, slide)
    }
    const at = widgets.findIndex(w => w.id === n.id)
    if (at >= 0) widgetAt = at
    // Not a widget's (a new version): its own icon.
    const own = widgets[at] || allWidgets.find(w => w.id === n.id)
    const widget = { ...n, icon: own?.icon || n.icon, color: own?.color, detail: own?.detail }
    cue(own ? 'plugin' : 'notice', { clip: 'waving', text: null, ms: NUDGE_READ_MS, widget })
  })

  // --- The pointer ----------------------------------------------------------------

  for (const target of [island]) {
    target.addEventListener('mouseenter', () => {
      window.pet.hover(true)
      // Open by itself until seen: seen now.
      seenNudge()
      clearTimeout(hoverTimer)
      hoverTimer = setTimeout(() => {
        // Closed by a press under this pointer (dismiss): not opened by it again.
        if (dismissed) return
        isHover = true
        update()
      }, HOVER_OPEN_MS)
    })
    target.addEventListener('mouseleave', () => {
      // Pulling her out keeps the window's pointer, wherever it goes.
      if (!pull?.started) window.pet.hover(false)
      // Off the island it was closed under: coming back is a new hover.
      dismissed = false
      clearTimeout(hoverTimer)
      hoverTimer = setTimeout(() => {
        // Open by itself, rested on and left: it has been seen (as Rust
        // counts it once the pointer leaves, island.rs hover), so it closes
        // now, not when its time is up. Only passed across, the hover never
        // taken, it stays open.
        if (isHover && isNudging()) endNudge()
        isHover = false
        // In the notch, whatever was open goes back once the pointer has
        // left, as a notch app's does: the settings too, unless a field in
        // them is being typed in (the user's call, 2026-10-14).
        const typing = /^(INPUT|TEXTAREA|SELECT)$/.test(document.activeElement?.tagName || '')
        if (screenTop?.notch && isSetting() && !typing) window.Settings?.close('left')
        if (peek) setPeek(false)
        else update()
      }, HOVER_CLOSE_MS)
    })
    // One click opens the settings (no double-click to wait for), except on a
    // prompt and its buttons, in the settings themselves, or on her (her
    // press may be a pull; pullUp decides). In the open island, a click on a
    // session goes to its window instead. Open by itself, a click closes it
    // (pressNudge), but on a session (to its window, the island closed too)
    // or a widget (its settings).
    target.addEventListener('click', e => {
      // A click in the settings (✕, the head) may just have closed them: not a click to open.
      if (isSetting() || window.Settings?.layer?.contains(e.target) || Date.now() - pulledAt < 500) return
      if (view === 'ask' || panel.contains(e.target) || her.contains(e.target)) return
      // A widget's menu open in it: a press elsewhere closes it, and is all.
      if (menuFor?.where === 'isle') {
        if (!wmenu.contains(e.target)) closeMenu()
        return
      }
      // In a card unfolded: nothing more (it is read there).
      if (view === 'expanded' && e.target.closest('.icard')) return
      // The press, if the clocks redrew the session between it and the release.
      const id = view === 'expanded' && (e.target.closest('[data-jump]')?.dataset.jump || (Date.now() - pressed.at < 1000 && pressed.id))
      pressed = { id: '', at: 0 }
      if (id) {
        if (isNudging()) dismiss()
        return jumpTo(id)
      }
      if (isNudging() && !e.target.closest('[data-widget]')) return pressNudge()
      // Not connected: "don't remind me" puts the hint away; any other
      // press but on a session's or a widget's row opens the settings'
      // welcome (a session listed still goes to its window: the hooks
      // removed while it runs).
      if (e.target.closest('[data-hint-off]')) return window.pet.settings.set({ connectHintOff: true })
      if (isHinting() && !e.target.closest('[data-widget]')) return window.pet.openSettings('connect')
      // A widget in the open island: the one shown from now on, and its
      // settings open (a press that only lit it felt like none: the user,
      // 2026-10-13).
      const row = view === 'expanded' && e.target.closest('[data-widget]')
      if (row) {
        const at = widgets.findIndex(w => w.id === row.dataset.widget)
        const w = at >= 0 ? widgets[at] : allWidgets.find(x => x.id === row.dataset.widget)
        // One with a large size: its card unfolded in place, or folded again
        // (one at a time); the island stays open for it.
        if (w && WCards.hasCard(w)) {
          if (isNudging()) endNudge()
          isHover = true
          return showCard(cardFor === w.id ? null : w.id)
        }
        if (at >= 0) turnWidget(at - (widgetAt % widgets.length))
        // Open by itself no more: the settings open over it.
        endNudge()
        return openWidgetSettings(w || { id: row.dataset.widget })
      }
      window.pet.openSettings()
    })
    // The wheel turns to the next widget, while the widgets have the island.
    target.addEventListener(
      'wheel',
      e => {
        if (!isQuiet() || widgets.length < 2 || isSetting() || view === 'ask') return
        // The open island's lines scrolling (more than it has room for): they scroll.
        if (view === 'expanded' && expanded.classList.contains('scrolls')) return
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
      if (wmenu.contains(e.target)) return
      // On a widget's row (or its card) in the open island: its menu.
      const row = view === 'expanded' && !isSetting() && (e.target.closest('[data-widget]') || e.target.closest('[data-card]'))
      const id = row && (row.dataset.widget || row.dataset.card)
      if (id && allWidgets.some(w => w.id === id)) return openMenu({ where: 'isle', widget: id }, row, { x: e.clientX, y: e.clientY })
      window.pet.menu()
    })
  }

  blink()

  // The prompt waiting asked for from her capsule on another display's
  // taskbar (bars.rs): it is open here already; a ring in the waiting
  // colour goes round the island twice so the eye finds it (style.css
  // look-here; held still a moment instead with reduced motion).
  window.pet.onLookHere(() => {
    if (!isAsking()) return
    island.style.setProperty('--look', hue().waiting)
    island.classList.remove('look-here')
    void island.offsetWidth
    island.classList.add('look-here')
  })
  island.addEventListener('animationend', e => e.animationName.startsWith('look-here') && island.classList.remove('look-here'))
  // Risen over an app full screen for the prompts alone: her island and its
  // panel, not the strip under them (style.css over-full).
  window.pet.onOverFull(({ on } = {}) => body.classList.toggle('over-full', on === true))

  // panel.js tells the island when its prompt changes.
  // The most the settings may be high: above the taskbar, its room (the
  // window never moves for them); elsewhere, none said (the screen's).
  // TASKBAR_ROOM is in the native monitor's logical px. WebView CSS px use
  // devicePixelRatio, which can differ from that monitor scale (Windows text
  // scaling): convert the room before sizing the settings page or its head
  // and tabs can land above the native window and be clipped. Until the
  // scale is told (before the first taskbar:strip; the preview never sends
  // one), the two are taken as the same.
  const settingsMax = () => (home() === 'taskbar' ? (isSide() ? innerHeight - 2 * TASKBAR_GAP - TASKBAR_SHADE : (TASKBAR_ROOM - TASKBAR_GAP - TASKBAR_SHADE) * (stripScale ? stripScale / (devicePixelRatio || 1) : 1)) : null)

  // The taskbar's layout, for the settings: its presets, which one it is, editing it on the strip.
  // From her menu: editing it (添加小工具…: with the picker open, { picker }),
  // or (退出编辑, while editing) out of it, kept as it went (picker.js).
  window.pet.onTaskbarEdit(p => (p?.end ? (endEdit(true), window.Picker?.closeAll()) : editTaskbar(p)))
  // picker.js, capsule-panel.js (loaded after this file): what they work
  // with here, read live. Internal to her home's page; settings.js and
  // panel.js use window.Island.
  window.Isle = {
    get config() {
      return config
    },
    get lang() {
      return lang
    },
    get allWidgets() {
      return allWidgets
    },
    get editLayout() {
      return editLayout
    },
    // The layout being edited, changed (the picker's drops): drawn anew.
    set editLayout(layout) {
      editLayout = layout
      arranged = ''
    },
    get members() {
      return membersNow
    },
    get view() {
      return view
    },
    // Windows' own mode (the picker's ground follows it, in either home).
    get winLight() {
      return look.mode === 'light'
    },
    isOn,
    home,
    // At the top of the screen, the side the island is at (anchor.js).
    anchor,
    tbEdge,
    isSide,
    isLight,
    isDetailed,
    along,
    lo,
    hi,
    len,
    el,
    stripInk,
    stripHue,
    ink,
    hue,
    tbLayout,
    hasMonitorPart,
    keepLayout,
    removeEntry,
    keyFor,
    whereOf,
    placeWidget,
    deskOff,
    keepDesk,
    keepDeskLayout,
    deskBoards,
    takeWidgets,
    placedAt,
    newId,
    update,
    sendSolid,
    watchPresses,
    drawEditBar,
    editTaskbar,
    endEdit,
    resetParts,
    askPlugins,
    openWidgetSettings,
    closeMenu,
    alignApps,
    placeSlot,
    entryNodes,
    zones,
    PART_NODES,
    TB_OPTIONAL,
    barRest,
    herSlot,
    island,
    compact,
    expanded,
  }
  // The preview's gallery (tools/preview boot.js freeze): the ring drawn as
  // it is ms into its turning (the step showing then), its steps stopped.
  function ringAt(ms) {
    const r = flowState.ring
    const s = r?.turnings?.[r.turnings.length - 1]
    if (!s || !isRinging()) return
    stopStepping()
    const angle = Flow.stepsOf(r, s.t0 + ms)[0]?.angle ?? Flow.angleAt(r, s.t0 + ms)
    for (const sp of spins) sp.style.transform = `rotate(${angle}deg)`
  }
  window.Island = { accent: () => look.accent || null, isOn, isOpen: () => isOn() && view !== 'compact' && !body.classList.contains('isl-stowed'), peek: on => setPeek(typeof on === 'boolean' ? on : view === 'compact'), changed: update, reach, absorb, settingsMax, tbPresets: TB_PRESETS, tbPreset: () => presetOf(config.taskbarLayout), tbWithPreset: name => TB_PRESETS[name] && Widgets.withParts(TB_PRESETS[name], tbLayout(config.taskbarLayout)), editTaskbar, whereOf, placeWidget, placeNow, anchor, notched: () => !!screenTop?.notch, ringAt }
})()
