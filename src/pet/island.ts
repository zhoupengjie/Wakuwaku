// Her home: by display, a black pill at the top centre after the iPhone's
// Dynamic Island (island; or at a top corner, anchor.ts), or a strip along
// the bottom of the screen in place of Windows' taskbar (taskbar): its left
// end hers, the other sessions as tags along the rest, with the Start
// button, the tray's icons (taskbar/) and the clock besides. The same element in each, which
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
// next one that wants something. The words come from status.ts.
//
// While no session needs anything, plugins (widgets.rs, widgets.ts) take
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
import * as Anchor from './anchor.ts'
import * as Capsule from './capsule.ts'
import * as DeskGrid from './deskgrid.ts'
import * as Flow from './flow.ts'
import * as Mica from './mica.ts'
import * as Status from './status.ts'
import * as Views from './views.ts'
import * as WCards from './wcards.ts'
import * as Widgets from './widgets.ts'
import { t, render as say, list as nameList } from '../shared/i18n.ts'
import { CELL_W, CELL_H, CLIPS, MOOD_CLIP, REACTIONS } from './sprite.ts'
import { Picker, Settings, provide } from './pages.ts'
import { pet } from './bridge.ts'
import { platePart } from './island-plate.ts'
import { rightPart } from './island-right.ts'
import { layoutPart } from './island-layout.ts'
import { glidePart } from './island-glide.ts'
import { wpopPart } from './island-wpop.ts'
import { wmenuPart } from './island-wmenu.ts'
import { boardsPart } from './island-boards.ts'
import { winlistPart } from './island-winlist.ts'
import { imePart } from './island-ime.ts'
import { lookPart } from './island-look.ts'
import { keysPart } from './island-keys.ts'
import { ringPart } from './island-ring.ts'
import { dropPart } from './island-drop.ts'
import type { Config } from '../shared/config.ts'


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
// one, any other darkened until it reads on white (widgets.ts).
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
// that follows it (anchor.ts). None said (an older island.rs), the
// centre, where it is.
let islandAt = 'center'
const anchor = () => Anchor.anchorOf({ home: home(), at: islandAt, notch: !!screenTop?.notch })
// And the edge of the screen it placed the window along (pet:update
// islandEdge; zhoupengjie/Wakuwaku#105): the top, or the bottom, laid over
// Windows' own taskbar, whose thickness it says too (islandStrip, physical
// px; 0 with none there). Along the foot the island lies at the window's
// foot (isl-at-bottom, --isl-foot) and opens upward; the window, as tall
// as the taskbar home's (BOTTOM), never grows.
let islandEdge = 'top'
let islandStrip = 0
const atBottom = () => Anchor.edgeAt({ home: home(), edge: islandEdge, notch: !!screenTop?.notch }) === 'bottom'
// The strip under it, and how far up from the window's foot the closed
// island lies, in the page's px (anchor.ts foot).
const stripUnder = () => (islandStrip > 0 ? islandStrip / (devicePixelRatio || 1) : 0)
const footAt = () => Anchor.foot(stripUnder())
// Where the settings' foot stands above it: off the strip as off the
// taskbar's (TASKBAR_GAP), or the island's own foot with no strip.
const settingsFoot = () => Math.max(stripUnder() + TASKBAR_GAP, footAt())
// The island inside Windows' taskbar (pet:update islandEmbed; embed.rs,
// zhoupengjie/Wakuwaku#105): its window a child of the taskbar's while it
// is closed, clipped to the strip, so the closed island is the capsule in
// the taskbar; laid about islandMx (the capsule's middle, physical px
// from the window's left) as the taskbar's middle run is (isl-embed,
// placeMid). islandRoom is the room the capsule has in the taskbar
// (physical px; 0 none), its width kept within it (slotWidth).
let islandEmbed = false
let islandMx = null
let islandRoom = 0
// Windows' taskbar along the top of the screen, the island over it there
// (the window's top on the screen's): the closed island centred in its
// strip (--isl-foot down from the top, isl-in-top), as it is centred in
// one along the bottom.
const inTopStrip = () => isOn() && home() === 'island' && !screenTop && !atBottom() && islandStrip > 0
const isEmbedded = () => isOn() && islandEmbed && (atBottom() || inTopStrip())
const roomPx = () => (islandRoom > 0 ? islandRoom / (devicePixelRatio || 1) : 0)
// Dropped open from the notch by her tray icon (setPeek), for PEEK_MS at
// most if the pointer never comes.
let peek = false
let peekTimer: Timer = 0
const PEEK_MS = 6000
let settingsIdleTimer: Timer = 0
let settingsUsed = false
// Where the pointer was first seen in the settings: a move counts once it
// goes a few px from there (WebKit may send one as they open under it).
let settingsFrom = null
// Listened for on a Mac only: the notch is there alone.
for (const type of pet.mac ? ['pointermove', 'pointerdown', 'wheel', 'keydown'] : []) {
  document.addEventListener(
    type,
    e => {
      if (view !== 'settings') return
      if (type !== 'pointermove') settingsUsed = true
      else if (!settingsFrom) settingsFrom = { x: (e as MouseEvent).screenX, y: (e as MouseEvent).screenY }
      else if (Math.hypot((e as MouseEvent).screenX - settingsFrom.x, (e as MouseEvent).screenY - settingsFrom.y) > 8) settingsUsed = true
    },
    true,
  )
}
// Within the room the capsule has in Windows' taskbar (roomPx; 0: no
// limit): as set, else narrow, else 0, her portrait alone (anchor.ts
// capsuleWidth; isTight, TIGHT_W: a round capsule).
const slotWidth = () => (screenTop?.notch ? screenTop.notch + 2 * EAR : Anchor.capsuleWidth(WIDTHS[config.islandWidth] || WIDTHS.normal, roomPx(), WIDTHS))
const TIGHT_W = 36
const isTight = () => !screenTop?.notch && slotWidth() === 0
// The window without extra room (main's ISLAND), and the island's distance
// from the top of it. As wide as a pull needs (PULL_ROOM), so the window
// only ever grows down: a window whose top-left corner moves shows its old
// picture from the new corner for a frame or two, and the island jumped
// sideways as she was pulled out or taken back.
const BASE = { width: 760, height: 132 }
const TOP = 8
// Along the foot of the screen (island.rs ISLAND_BOTTOM): as tall as the
// taskbar's strip and its room, fixed, what opens growing up into it.
const BOTTOM = { width: 760, height: 48 + 640 }
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
// Split in two (taskbarEdge split, docs/design/specs/dock-topbar.md), the
// top bar thinner unless set to 48 (splitBar; island.rs SLIM_H): her
// capsule in it 26 high, 3 off its edge, her portrait 20 (stripH).
const SLIM_H = 32
const SLIM_HEAD = 20
const SLIM_CAPSULE_Y = 3
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
// The settings' width (the Mail page in two panes is wider: settings.ts).
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
// An app full screen on her display (pet:full-screen, shared.rs look_fullscreen).
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
// Its state (flow.ts), its animations running, its colours' blends, its
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
// What pet:update said last (shared.rs payload): the session shown, its
// mood and words, the other sessions, and what else comes with it (her
// plate there to have, Windows' light mode, not connected yet and the
// tools here).
interface Now {
  mood: string
  detail: string
  project: string
  since: number
  took: number
  others: number
  sessions: number
  list: any[]
  id?: string
  reply?: string
  stepSince?: number
  todo?: { done: number; total: number; active?: string }
  plate?: boolean
  winLight?: boolean
  unconnected?: boolean
  tools?: string[]
}
let now: Now = { mood: 'idle', detail: '', project: '', since: null, took: null, others: 0, sessions: 0, list: [] }
let config: Config = {}
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
// Windows' look as taskbar/ sent it last: its mode (dark, light) and
// accent colour; and whether its End task is on (a program's menu offers it).
// Its accent comes as its own colour and its shades for a light and a dark
// ground (taskbar/win/look.rs); the clock's seconds and the Task View
// button on or off as Windows has them.
interface Look {
  mode: string
  accent?: { base: string; light: string; dark: string }
  endTask?: boolean
  seconds?: boolean
  taskView?: boolean
}
let look: Look = { mode: 'dark' }
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
// Her window up on the desktop (shared.rs payload herUp): out there she says
// it herself, the prompts over her head too (pet.ts, panel.ts), and the
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
// picker.ts: her capsule's members (capsule.ts: the capsule setting's
// order and modes), and those pinned at its right end.
let membersNow = []
let pinsNow = []

// The widgets as sent: those that take turns are those on, but for the
// monitor pinned and what is put elsewhere (deskgrid.ts placement, the
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
// The top bar and the dock (split) are her taskbar too, split in two (isSplit).
const home = () => (config.display === 'taskbar' || config.display === 'split' ? 'taskbar' : 'island')
// A strip across the screen (the taskbar): the other sessions have tags in it.
// The taskbar's strip the desktop's picture through (mica.ts), neither
// solid nor clear.
const isMica = () => isOn() && home() === 'taskbar' && !['black', 'clear'].includes(config.taskbarMaterial)
// The taskbar's strip clear, the desktop itself through it; while a window
// is maximized or full screen on its display (taskbar/ "max") it fills
// in, as Mica (its default) or solid, unless it is to stay clear always
// (taskbarClearWhen).
const isClear = () => isOn() && home() === 'taskbar' && config.taskbarMaterial === 'clear'
const clearWhen = () => (['solid', 'always'].includes(config.taskbarClearWhen) ? config.taskbarClearWhen : 'mica')
const isFilled = () => isClear() && clearWhen() !== 'always' && taskWindows.some(a => (a.windows || []).some(w => w.max && !w.min))
// How light the desktop under the clear strip is, along it (mica.ts
// lightness): what each part's words go by (inkParts); null, not known
// (yet). backdropLight: the strip's as a whole, by its mean, true light
// enough for dark words.
let backdrop = null
const backdropLight = () => (backdrop ? Mica.wantsDarkInk(backdrop.mean) : null)
// The taskbar light, as Windows' own is in its light mode (taskbar/):
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
const tbEdge = () => (isSplit() ? 'top' : home() === 'taskbar' && ['top', 'left', 'right'].includes(config.taskbarEdge) ? config.taskbarEdge : 'bottom')
// The top bar and the dock (display split, a home of its own beside the
// island and the taskbar): the strip is a top bar, Start and the programs
// in the dock at the foot (bars.rs, bar.ts), not on it.
const isSplit = () => config.display === 'split'
// The strip's thickness: Windows' own, or split, the top bar's (style.css tb-slim).
const stripH = () => (isSplit() && config.splitBar !== 48 ? SLIM_H : TASKBAR_H)
const isSlim = () => stripH() === SLIM_H
const taskHead = () => (isSlim() ? SLIM_HEAD : TASK_HEAD)
const capsuleY = () => (isSlim() ? SLIM_CAPSULE_Y : CAPSULE_Y)
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
const taskCompactH = () => (isCapsule() ? stripH() - 2 * capsuleY() : stripH())
// In her seat: there is a pet to show, and she is not out on the desktop.
const isHome = () => !!spriteUrl && (config.out !== true || isComingHome)
const isAsking = () => isOn() && !panel.hidden
// The settings open in the island (settings.ts).
const isSetting = () => isOn() && !!Settings?.isOpen()
const isNudging = () => !!nudge && Date.now() < nudge.until
// The specifics on screen (session names, commands, replies), unless turned off.
const isDetailed = () => config.details !== false
// Neither Claude Code nor Codex connected (shared.rs payload, unconnected),
// and no "don't remind me" (connectHintOff): the island asks for it, and a
// press on it opens the settings' welcome, where they are connected.
const isHinting = () => now.unconnected === true && config.connectHintOff !== true
// The tools on this computer by name, as one says them (shared.rs payload: with
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

function el(tag, cls?, text?) {
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
    compact.replaceChildren(...(isHome() ? [] : [stillHer(taskHead())]), ...(hint || at ? [clock] : []))
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
  // picker.ts: the members pinned at its end (five at most, capsule.ts): the
  // monitor's readings as they were, another its glyph and value.
  // The room too narrow for words (isTight: a short stretch of Windows'
  // taskbar): her portrait alone, nothing pinned.
  const tight = isTight()
  const pinned = pinsNow.length && !tight ? el('span', 'pinned') : null
  if (pinned) pinned.innerHTML = pinsNow.map(pinHTML).join('')
  compact.replaceChildren(...(isHome() ? [] : [stillHer(24)]), ...(tight || (isBare && pinned) ? [] : [turning]), ...(pinned ? [pinned] : []))
  compact.classList.toggle('bare', isBare || tight)
  compact.classList.toggle('tight', tight)
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
    : tight
      ? `${TIGHT_W}px`
      : isBare && !screenTop?.notch
        ? ''
        : `${slotWidth() + (pinned?.offsetWidth ? pinned.offsetWidth + 12 : 0)}px`
  // Inside Windows' taskbar (roomPx, embed.rs): the pinned members go
  // first when they and the words together are more than the room; the
  // words narrow after them (slotWidth), then her portrait alone (tight).
  if (pinned && !tucked && roomPx() > 0 && parseFloat(compact.style.width) > roomPx()) {
    pinned.remove()
    compact.style.width = isBare ? '' : `${slotWidth()}px`
  }
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
// with its value and as its glyph alone, capsule.ts fitPins says which
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
  const fit = Capsule.fitPins(sizes, room - own, { gap: parseFloat(cs.columnGap) || 8, plus: more.offsetWidth })
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

// A widget's line in the open island (the spec's §6, wcards.ts rowHTML),
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
  pet.settings.set({ widgetSize: { ...(config.widgetSize || {}), [id]: size } })
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

// A plugin's own view in its unfolded card (views.ts; the spec's §4):
// its view.html over the card, a medium one there (as wide as the card,
// 128 px high), where its widget gives one; the card drawn from its data
// until the view is ready, and whenever views may not show (nobody about,
// animations off, the watchdog's while; and where this page cannot keep a
// frame loaded as it draws its lines anew each second: moveBefore).
const islandStyle = name => getComputedStyle(island).getPropertyValue(name).trim()
const islandViews =
  ROLE === 'island'
    ? Views.boot({
        ask: widget => pet.addons.view('data', widget),
        send: (widget, message) => pet.addons.view('send', widget, message),
        styleOf: w => ({ fg: islandStyle('--wg-fg'), muted: islandStyle('--wg-2'), color: WCards.colorOf(w, isleCtx()), font: getComputedStyle(expanded).fontFamily, fontDisplay: islandStyle('--wg-num') }),
        look: () => ({ theme: inkLight ? 'light' : 'dark', lang }),
        // Drawn again once what told it is done (pet:update, pet:quiet).
        changed: () => queueMicrotask(() => isOn() && update()),
        log: line => console.log(line),
        redrawn: true,
      })
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
  // their order (deskgrid.ts placement); those on the taskbar or the
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
  // A card's plugin view kept loaded as its card is drawn anew (views.ts park, mount).
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
  pet.jump(id).then(went => {
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
    const [px, at] = home() === 'taskbar' ? [taskHead(), (taskCompactH() - taskHead()) / 2] : [HEAD, screenTop ? (screenTop.height - HEAD) / 2 : 6]
    const left = home() === 'taskbar' ? at : screenTop ? 10 : 6
    if (home() === 'taskbar') portrait(taskHead(), taskHead() * PER_PX, at, at)
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
// long (as she does out on the desktop, pet.ts): each frame is a frame of
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
const baseRoom = () => (home() === 'taskbar' ? { width: innerWidth, height: stripH() + TASKBAR_ROOM } : atBottom() ? BOTTOM : BASE)

function roomFor(want) {
  // The taskbar: what grows up from it, or the settings standing above it,
  // past its room (the window then grows up too, which flashes: its room
  // is kept as high as the settings ever are, settingsMax).
  if (home() === 'taskbar' && isSide()) return null
  if (home() === 'taskbar') {
    const height = want.height + TASKBAR_SHADE + (view === 'settings' ? stripH() + TASKBAR_GAP : isCapsule() ? capsuleY() : 0)
    if (height <= stripH() + TASKBAR_ROOM) return null
    return { width: innerWidth, height }
  }
  const width = want.width + 48
  // Along the foot of the screen the window's height is its room, never
  // grown (openMaxH, settingsMax keep within it); only its width may.
  if (atBottom()) return width <= BOTTOM.width ? null : { width, height: BOTTOM.height }
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
// scroll inside it past that (fillExpanded). Along the foot of the screen
// (the island home, #105) the same: within the window, which never grows.
function openMaxH() {
  if (home() !== 'taskbar') return atBottom() ? innerHeight - footAt() - TASKBAR_SHADE : Infinity
  if (isSide()) {
    const at = parseFloat(document.documentElement.style.getPropertyValue('--isl-y')) || 0
    return innerHeight - at - TASKBAR_GAP - TASKBAR_SHADE
  }
  const tall = Math.min(stripH() + TASKBAR_ROOM, (screen.height || Infinity) - TASKBAR_GAP)
  return tall - TASKBAR_SHADE - (isCapsule() ? capsuleY() : 0)
}

// Opened up from the strip, or closed (part: island, pops): opening when
// nothing was, her window over any other that lies over it.
function openedUp(part, on) {
  const was = openUp.island || openUp.pops
  openUp[part] = on
  const open = openUp.island || openUp.pops
  // On the taskbar, and along the foot of the screen over Windows' own (#105).
  if (on && !was && isOn() && (home() === 'taskbar' || atBottom())) pet.raise()
  // Inside Windows' taskbar (embed.rs): the window is out of the taskbar
  // while something is open in it (or the pointer is on it): told as
  // something opens, and, nothing open any more, once the island has
  // shrunk back (the spring), so it goes back in with nothing moving in it.
  if (open !== was) tellOpened(open)
}
let openedTimer: Timer = 0
function tellOpened(on) {
  clearTimeout(openedTimer)
  if (on) return pet.opened(true)
  openedTimer = setTimeout(() => pet.opened(false), SPRING_MS + 100)
}

// Returns true when the window has to grow first.
function setRoom(next) {
  const size = r => r || baseRoom()
  clearTimeout(roomTimer)
  if (JSON.stringify(next) === JSON.stringify(room)) return false
  const grows = size(next).width > size(room).width || size(next).height > size(room).height
  const apply = () => {
    room = next
    pet.panel(next)
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
// What the plate was last told (where, as JSON; '': none), the wait for
// the spring before it fades in, and the wait while it moves with it.
let plateSent = ''
// In island-plate.ts:
const plateCore = {
  Anchor, afterSpring, anchor, atBottom, body, home, inTopStrip, isCapsule, isEmbedded, isOn, isRinging,
  isSide, island, sizeOf, stillMotion, tbEdge,
  get config() { return config },
  get groundOf() { return groundOf },
  get isAbsorbing() { return isAbsorbing },
  get isComingHome() { return isComingHome },
  get isHover() { return isHover },
  get isReaching() { return isReaching },
  get lookNow() { return lookNow },
  get midLeft() { return midLeft },
  get modeNow() { return modeNow },
  get now() { return now },
  get placeMid() { return placeMid },
  get pull() { return pull },
  get quiet() { return quiet },
  get screenTop() { return screenTop },
  get slotMiddle() { return slotMiddle },
  get taskWindows() { return taskWindows },
  get view() { return view },
  get plateSent() { return plateSent }, set plateSent(v) { plateSent = v },
}
export type PlateCore = typeof plateCore
const {
  DEEP_OP, PLATE_OUT_MS, hidePlate, placePlate, setDeep, setSize, setStow,
} = platePart(plateCore)

// --- Deciding ---------------------------------------------------------------

function sizeOf(name) {
  const withHer = isHome()
  if (name === 'settings') return { width: Settings.layer.offsetWidth || SETTINGS_W, height: Settings.layer.offsetHeight }
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
// The program last in front (drawFront).
let frontWas = null
let taskbarBuilt = false
// In island-right.ts:
const rightCore = {
  Widgets, el, home, isDetailed, isPinned, isSide, island, stripHue, stripInk, t,
  get editLayout() { return editLayout },
  get fillTaskbar() { return fillTaskbar },
  get lang() { return lang },
  get monitor() { return monitor },
  get widgetAt() { return widgetAt },
  get widgets() { return widgets },
}
export type RightCore = typeof rightCore
const {
  barRest, batMark, capsMark, clockBox, deskEdge, facesBox, fillBar, frontBox, frontIcon, frontName,
  growNode, imeMark, keysBox, netMark, onPixels, privBox, quickBox, showBattery, showNet, showPrivacy,
  showVolume, snapStart, startButton, startPart, taskViewButton, trayBox, volMark, widgetFaces, windowsBox,
} = rightPart(rightCore)

// --- The taskbar's layout (taskbarLayout) -------------------------------------------
// A layout of the strip (widgets.ts layoutOf): its three runs, each its
// parts by name and the widgets put there.
export interface TbLayout {
  left: any[]
  center: any[]
  right: any[]
}
// While the layout is being edited: the one being made, not yet kept.
let editLayout: TbLayout = null
let arranged = ''
// Her slot's middle along the strip while it is in the middle run (placeSlot),
// and where her island's left edge lies to be about it at a width: the
// middle, kept 8 px off the screen's ends (as the settings at an end are).
let slotMiddle = null
// The tray's icons as last sent ({ key, png, tip, exe }), and as drawn.
let trayIcons = []
let trayDrawn = ''
let trayRectsSent = ''
// The windows' buttons as last sent (taskbar/), and as drawn.
let taskWindows = []
let windowsDrawn = ''
// The programs' buttons drawn once already: a new one after that grows its line in.
let buttonsDrawn = false
// In island-layout.ts:
const layoutCore = {
  CAPSULE_X, CELL_W, CLIPS, MOOD_CLIP, PER_PX, TASKBAR_GAP, Widgets, barRest, body, clockBox, deskEdge, el,
  face, facesBox, frontBox, growNode, home, isCapsule, isDetailed, isLight, isOn, isSide, isSlim, isSplit,
  isStripLight, island, keysBox, len, lo, onPixels, privBox, quickBox, sheetImage, showFrame, sizeOf,
  snapStart, startButton, startPart, t, takeWidgets, taskCompactH, taskViewButton, trayBox, widgetFaces,
  windowsBox,
  get STACK_CARD_W() { return STACK_CARD_W },
  get allWidgets() { return allWidgets },
  get config() { return config },
  get drawFront() { return drawFront },
  get drawPop() { return drawPop },
  get drawStackPop() { return drawStackPop },
  get drawTray() { return drawTray },
  get drawWidgetPop() { return drawWidgetPop },
  get drawWindows() { return drawWindows },
  get editLayout() { return editLayout },
  get fadeWins() { return fadeWins },
  get glide() { return glide },
  get glideOn() { return glideOn },
  get glideTo() { return glideTo },
  get imePop() { return imePop },
  get inkParts() { return inkParts },
  get lang() { return lang },
  get look() { return look },
  get oledOn() { return oledOn },
  get pop() { return pop },
  get popFor() { return popFor },
  get quiet() { return quiet },
  get sendTrayRects() { return sendTrayRects },
  get spriteUrl() { return spriteUrl },
  get stackFor() { return stackFor },
  get stackPop() { return stackPop },
  get tickClock() { return tickClock },
  get view() { return view },
  get wgFor() { return wgFor },
  get wgPop() { return wgPop },
  get arranged() { return arranged }, set arranged(v) { arranged = v },
  get slotMiddle() { return slotMiddle }, set slotMiddle(v) { slotMiddle = v },
  get taskbarBuilt() { return taskbarBuilt }, set taskbarBuilt(v) { taskbarBuilt = v },
  get trayDrawn() { return trayDrawn }, set trayDrawn(v) { trayDrawn = v },
  get windowsDrawn() { return windowsDrawn }, set windowsDrawn(v) { windowsDrawn = v },
}
export type LayoutCore = typeof layoutCore
const {
  PART_NODES, RUNS_APART, TB_OPTIONAL, TB_PRESETS, alignApps, askPlugins, carousels, drawEntries,
  entryNodes, fillHer, fillTaskbar, flashSince, hasMonitorPart, herSlot, memberName, midLeft, needsSettings,
  ownName, placeMid, placeSideSettings, placeSlot, placedAt, popCtx, presetOf, stillMotionNow, tbLayout,
  tickCarousels, turnCarousel, zones,
} = layoutPart(layoutCore)

// --- The pointer's glide --------------------------------------------------------
let glideOn = null
// { app, kind: 'list' | 'menu' } while open, and what it showed last.
let popFor = null
let popDrawn = ''
let popTimer
// The widget on the taskbar whose card is open (openWidgetPop: its entry,
// and in a group the widget picked); the stack opened (openStackPop); a
// widget's menu on the strip (openMenu); the input methods' list open
// (openImeList).
let wgFor = null
let wgPick = null
let stackFor = null
let imeOpen = false
// In island-glide.ts:
const glideCore = {
  alignApps, barRest, body, crossLo, el, entryNodes, flashSince, hi, home, imeMark, isOn, isSide, lo,
  openedUp, placeSlot, t, trayBox, windowsBox,
  get closeImeList() { return closeImeList },
  get closeMenu() { return closeMenu },
  get closeStackPop() { return closeStackPop },
  get closeWidgetPop() { return closeWidgetPop },
  get config() { return config },
  get drawPop() { return drawPop },
  get imeOpen() { return imeOpen },
  get imePop() { return imePop },
  get inkOf() { return inkOf },
  get lang() { return lang },
  get look() { return look },
  get menuFor() { return menuFor },
  get openTray() { return openTray },
  get popMoveTimer() { return popMoveTimer },
  get popTimer() { return popTimer },
  get sendThumbs() { return sendThumbs },
  get stackFor() { return stackFor },
  get stackPop() { return stackPop },
  get taskWindows() { return taskWindows },
  get trayOpen() { return trayOpen },
  get wgFor() { return wgFor },
  get wgPop() { return wgPop },
  get wmenu() { return wmenu },
  get buttonsDrawn() { return buttonsDrawn }, set buttonsDrawn(v) { buttonsDrawn = v },
  get glideOn() { return glideOn }, set glideOn(v) { glideOn = v },
  get popFor() { return popFor }, set popFor(v) { popFor = v },
  get popMoving() { return popMoving }, set popMoving(v) { popMoving = v },
  get popShown() { return popShown }, set popShown(v) { popShown = v },
  get windowsDrawn() { return windowsDrawn }, set windowsDrawn(v) { windowsDrawn = v },
}
export type GlideCore = typeof glideCore
const {
  appOf, appOrder, closePop, drawWindows, fadeWins, glide, glideTo, isExplorer, openPop, orderedApps, pop,
  popReach, watchPresses,
} = glidePart(glideCore)

// --- A widget on the taskbar, opened ------------------------------------------------
// In island-wpop.ts:
const wpopCore = {
  Widgets, barRest, closePop, el, entryNodes, fillHer, home, isDetailed, isSide, memberName, needsSettings,
  popCtx, t, tbLayout, update, watchPresses,
  get allWidgets() { return allWidgets },
  get besideOrOver() { return besideOrOver },
  get closeImeList() { return closeImeList },
  get closeMenu() { return closeMenu },
  get config() { return config },
  get imeOpen() { return imeOpen },
  get lang() { return lang },
  get menuFor() { return menuFor },
  get newId() { return newId },
  get openMenu() { return openMenu },
  get popFor() { return popFor },
  get arranged() { return arranged }, set arranged(v) { arranged = v },
  get editLayout() { return editLayout }, set editLayout(v) { editLayout = v },
  get stackFor() { return stackFor }, set stackFor(v) { stackFor = v },
  get wgFor() { return wgFor }, set wgFor(v) { wgFor = v },
  get wgPick() { return wgPick }, set wgPick(v) { wgPick = v },
}
export type WpopCore = typeof wpopCore
const {
  STACK_CARD_W, closeStackPop, closeWidgetPop, drawStackPop, drawWidgetPop, imePop, keepLayout,
  openStackPop, openWidgetPop, openWidgetSettings, removeEntry, removeMember, stackPop, wgPop,
} = wpopPart(wpopCore)

// --- A widget's menu (the spec's §10) -------------------------------------------------
let menuFor = null
// The open island's height for its menu (0: none open).
let menuRoom = 0
// In island-wmenu.ts:
const wmenuCore = {
  Widgets, barRest, closePop, closeStackPop, closeWidgetPop, el, entryNodes, fillHer, hasMonitorPart, home,
  isDetailed, island, isleCtx, keepLayout, memberName, openWidgetSettings, ownName, placedAt, popCtx,
  removeEntry, removeMember, rowSize, setRowSize, showCard, stackPop, t, tbLayout, update, watchPresses,
  get allWidgets() { return allWidgets },
  get backToIsland() { return backToIsland },
  get besideOrOver() { return besideOrOver },
  get boardName() { return boardName },
  get boardSize() { return boardSize },
  get cardFor() { return cardFor },
  get closeImeList() { return closeImeList },
  get config() { return config },
  get deskBoards() { return deskBoards },
  get deskOff() { return deskOff },
  get imeOpen() { return imeOpen },
  get lang() { return lang },
  get newId() { return newId },
  get placedOut() { return placedOut },
  get popFor() { return popFor },
  get putOnDesk() { return putOnDesk },
  get stackFor() { return stackFor },
  get toBoard() { return toBoard },
  get toDesk() { return toDesk },
  get wgFor() { return wgFor },
  get menuFor() { return menuFor }, set menuFor(v) { menuFor = v },
  get menuRoom() { return menuRoom }, set menuRoom(v) { menuRoom = v },
}
export type WmenuCore = typeof wmenuCore
const {
  closeMenu, keyFor, offBar, openMenu, placeNow, placeWidget, whereOf, wmenu,
} = wmenuPart(wmenuCore)

// --- Boards on the desktop (§7 工作台) ---------------------------------------------
// In island-boards.ts:
const boardsCore = {
  Widgets, closeStackPop, home, keepLayout, offBar, placeWidget, removeEntry, removeMember, t, takeWidgets,
  tbLayout, update,
  get allWidgets() { return allWidgets },
  get config() { return config },
  get lang() { return lang },
  get newId() { return newId },
  get stackFor() { return stackFor },
}
export type BoardsCore = typeof boardsCore
const {
  backToIsland, boardName, boardSize, deskBoards, deskOff, deskPlace, keepDesk, keepDeskLayout, putOnDesk,
  toBoard, toDesk,
} = boardsPart(boardsCore)

let popShown = null
let popMoving = false
let popMoveTimer
// --- A program's windows, listed over its button -------------------------------------
// In island-winlist.ts:
const winlistCore = {
  across, along, appOf, appOrder, barRest, body, closePop, crossHi, crossLo, el, hi, home, isExplorer, isOn,
  isSide, island, len, lo, openPop, orderedApps, pop, popReach, t, tbEdge, windowsBox,
  get editLayout() { return editLayout },
  get lang() { return lang },
  get look() { return look },
  get popFor() { return popFor },
  get popDrawn() { return popDrawn }, set popDrawn(v) { popDrawn = v },
  get popMoveTimer() { return popMoveTimer }, set popMoveTimer(v) { popMoveTimer = v },
  get popMoving() { return popMoving }, set popMoving(v) { popMoving = v },
  get popShown() { return popShown }, set popShown(v) { popShown = v },
  get popTimer() { return popTimer }, set popTimer(v) { popTimer = v },
}
export type WinlistCore = typeof winlistCore
const { besideOrOver, drawPop, pressApp, sendThumbs } = winlistPart(winlistCore)

// --- The input method on the taskbar -------------------------------------------------
// In island-ime.ts:
const imeCore = {
  besideOrOver, capsMark, closePop, closeWidgetPop, el, imeMark, imePop, isSide, showBattery, showNet,
  showPrivacy, showVolume, t, watchPresses,
  get lang() { return lang },
  get popFor() { return popFor },
  get wgFor() { return wgFor },
  get imeOpen() { return imeOpen }, set imeOpen(v) { imeOpen = v },
}
export type ImeCore = typeof imeCore
const { closeImeList, openImeList } = imePart(imeCore)

// --- The taskbar's look: Mica, light or dark, Windows' accent -------------------------
// The monitor's scale as taskbar:strip says it (sf); 0 until told.
let stripScale = 0
let trayOpen = false
// In island-look.ts:
const lookCore = {
  DEEP_OP, PEEK_MS, TASKBAR_H, alignApps, barRest, body, clearWhen, clockBox, deskEdge, drawEntries,
  drawWindows, el, endNudge, frontBox, frontIcon, frontName, glide, home, isCapsule, isClear, isFilled,
  isLight, isMica, isOn, isSide, isSlim, isStripLight, island, islandViews, placeAnchor, sendSolid, setDeep,
  snapStart, stripH, t, trayBox, update, watchPresses,
  get allWidgets() { return allWidgets },
  get config() { return config },
  get glideOn() { return glideOn },
  get isHover() { return isHover },
  get lang() { return lang },
  get now() { return now },
  get taskbarBuilt() { return taskbarBuilt },
  get view() { return view },
  get backdrop() { return backdrop }, set backdrop(v) { backdrop = v },
  get frontWas() { return frontWas }, set frontWas(v) { frontWas = v },
  get groundLight() { return groundLight }, set groundLight(v) { groundLight = v },
  get inkLight() { return inkLight }, set inkLight(v) { inkLight = v },
  get look() { return look }, set look(v) { look = v },
  get peek() { return peek }, set peek(v) { peek = v },
  get peekTimer() { return peekTimer }, set peekTimer(v) { peekTimer = v },
  get screenTop() { return screenTop }, set screenTop(v) { screenTop = v },
  get stripScale() { return stripScale }, set stripScale(v) { stripScale = v },
  get taskWindows() { return taskWindows }, set taskWindows(v) { taskWindows = v },
  get trayDrawn() { return trayDrawn }, set trayDrawn(v) { trayDrawn = v },
  get trayIcons() { return trayIcons }, set trayIcons(v) { trayIcons = v },
  get trayOpen() { return trayOpen }, set trayOpen(v) { trayOpen = v },
  get trayRectsSent() { return trayRectsSent }, set trayRectsSent(v) { trayRectsSent = v },
}
export type LookCore = typeof lookCore
const {
  chevron, drawFront, drawMica, drawTray, flyout, groundOf, inkOf, inkParts, isOut, lookNow, modeNow,
  openTray, orderedTray, outBox, sendTrayRects, setInk, setPeek, tickClock, trayOrder,
} = lookPart(lookCore)

// --- The keyboard on the taskbar (Win+T, Win+B) ---------------------------------------
// In island-keys.ts:
const keysCore = {
  along, appOf, barRest, batMark, body, chevron, clockBox, closeImeList, closePop, closeStackPop,
  closeWidgetPop, el, endEdit, flyout, hi, home, isOn, isOut, isSetting, isSide, lo, netMark, openImeList,
  openMenu, openStackPop, openTray, openWidgetPop, orderedTray, outBox, pop, pressApp, quickBox, t, trayBox,
  trayOrder, volMark, wgPop, windowsBox, wmenu,
  get editLayout() { return editLayout },
  get imeOpen() { return imeOpen },
  get lang() { return lang },
  get popFor() { return popFor },
  get stackFor() { return stackFor },
  get trayIcons() { return trayIcons },
  get trayOpen() { return trayOpen },
  get wgFor() { return wgFor },
}
export type KeysCore = typeof keysCore
keysPart(keysCore)

// --- Editing the layout: the parts dragged about on the strip itself -------------------
// As KDE edits a panel (the picker of 2026-10-17): each part outlined, the
// three runs shown (an empty one a place to drop into), a bar above the
// strip to open the picker (picker.ts: the widgets there are, a sidebar at
// the screen's side, to drag onto the strip, into her capsule or onto the
// desktop) and to be done. What changes is kept as it goes (update:
// picker.ts persist), so there is no 取消: a widget taken off comes back by
// its toast's 撤销. The drags, the popover over a widget and her capsule's
// panel are picker.ts's: the strip's presses go there while editing. The
// parts' own presses are held back meanwhile; her island lets the pointer
// through to its slot, to be dragged by it.
const editBar = el('div', 'tbedit')

// A new widget's id, as settings.rs takes it: "w" and up to 15 of a-z 0-9.
const newId = () => `w${Date.now().toString(36).slice(-6)}${Math.random().toString(36).slice(2, 6)}`

// Pointer in or out of what editing puts over the strip: the window takes it.
editBar.addEventListener('mouseenter', () => pet.hover(true))
editBar.addEventListener('mouseleave', () => pet.hover(false))
// And whatever the page hears of the pointer: where they are goes to the
// Rust side as they change (island.rs set_editing), and the window takes
// the mouse over them there (pointer.rs solid). By enter and leave alone
// its mode flipped while the pointer was on the bar, a press going to the
// window under it and only its release coming here, so 完成 and 取消 did
// nothing (zhoupengjie/Wakuwaku#53). A couple of px more all round.
// picker.ts: the picker, its popover, the capsule's panel and a toast
// with 撤销 too, editing or not, in either home (on: the layout being
// edited, for her menu's 退出编辑; the rects whatever it is).
let solidSent = ''
function sendSolid() {
  if (!isOn()) return
  const extra = Picker?.solids() || []
  const on = !!editLayout
  // Settings are a large interactive surface, often much bigger than the
  // capsule. Keep the native window catching the mouse over them even if
  // WebView2 misses an enter/leave while its click-through mode changes.
  // Put this first because Rust caps the list at eight rectangles.
  const settings = Settings?.isOpen?.() ? Settings.layer : null
  const rects = [...(settings ? [settings] : []), ...(editLayout ? [editBar] : []), ...extra]
    .filter(n => n.isConnected)
    .map(n => n.getBoundingClientRect())
    .filter(r => r.width && r.height)
    .map(r => [Math.floor(r.left) - 2, Math.floor(r.top) - 2, Math.ceil(r.width) + 4, Math.ceil(r.height) + 4])
  const sent = JSON.stringify([on, rects, devicePixelRatio || 1])
  if (sent === solidSent) return
  solidSent = sent
  pet.editing(on, rects)
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
    const what = !pressed && e.button === 0 && editLayout && editBar.contains(e.target) && (e.target as HTMLElement).closest('[data-edit]')?.dataset.edit
    if (what) editAct(what)
  },
  true,
)

// opts.picker: from her menu's 添加小工具…, the picker open with it.
function editTaskbar(opts?) {
  if (home() !== 'taskbar') return
  if (editLayout) return opts?.picker && Picker?.open()
  if (isSetting()) Settings?.close?.('edit')
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
  if (opts?.picker) Picker?.open()
  requestAnimationFrame(sendSolid)
  setTimeout(() => {
    if (!editLayout || editKeys) return
    editKeys = true
    pet.keyboard(true)
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
  add.setAttribute('aria-pressed', String(!!Picker?.isOpen()))
  editBar.replaceChildren(el('span', 'hint', t(lang, 'taskbar.edit.title')), el('span', 'hint kept', t(lang, 'taskbar.edit.kept')), add, button('done', t(lang, 'taskbar.edit.done')))
}

// What a layout puts on the strip, as placeWidget takes them: its widgets
// and plugins, and the monitor while it has its part.
const barItems = (layout: TbLayout) => [...Object.values(layout).flatMap(run => run.filter(p => typeof p !== 'string').flatMap(Widgets.membersOf).map(m => (m.type === 'plugin' ? { plugin: m.of } : { id: m.of }))), ...(hasMonitorPart(layout) ? [{ id: 'monitor' }] : [])]

function endEdit(keep) {
  if (!editLayout) return
  const layout = editLayout
  editLayout = null
  Picker?.ended()
  body.classList.remove('tb-editing')
  // The pointer on the bar as it goes (its 完成 pressed): taken out, it
  // hears no leave, and the window kept the mouse over the room above the
  // strip until the pointer left the window (the desktop there not to be
  // pressed). Left here.
  if (editBar.matches(':hover')) pet.hover(false)
  editBar.remove()
  sendSolid()
  if (editKeys) {
    editKeys = false
    pet.keyboard(false)
  }
  if (keep) {
    // Those put on the strip now are there alone (§7): off the desktop.
    const was = tbLayout(config.taskbarLayout)
    const onDeskToo = barItems(layout).filter(it => !barItems(was).some(x => keyFor(x) === keyFor(it)) && DeskGrid.has(config.deskLayout, it))
    if (onDeskToo.length) deskOff(onDeskToo)
    config.taskbarLayout = layout
    const patch: Config = { taskbarLayout: layout }
    // A plugin put there that was not running: it runs now.
    const plugins = config.plugins && typeof config.plugins === 'object' ? config.plugins : {}
    const off = [...new Set(Object.values(layout).flatMap(run => run.filter(p => p.type === 'plugin').map(p => p.of)))].filter(id => plugins[id]?.on !== true)
    if (off.length) patch.plugins = { ...plugins, ...Object.fromEntries(off.map(id => [id, { ...(plugins[id] || {}), on: true }])) }
    // Then which of the plugins put there want their settings first.
    Promise.resolve(pet.settings.set(patch)).then(() => askPlugins(true))
  }
  arranged = ''
  update()
}

editBar.addEventListener('click', e => editAct(e.target.closest('[data-edit]')?.dataset.edit))

// One of the bar's buttons, pressed (a click, or a release alone: above).
function editAct(what) {
  if (what === 'done') endEdit(true)
  else if (what === 'add') Picker?.toggle()
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
      if (type === 'contextmenu') pet.menu()
    },
    true,
  )
}
// The same on the bar.
editBar.addEventListener('contextmenu', e => {
  e.preventDefault()
  if (editLayout) pet.menu()
})
// A press on a part or a widget: picker.ts drags it (on the strip, into
// her capsule, off it, onto the desktop), or, let go where it was, opens
// a widget's popover or her capsule's panel.
barRest.addEventListener(
  'pointerdown',
  e => {
    if (!editLayout) return
    e.preventDefault()
    e.stopPropagation()
    if (e.button === 0) Picker?.pressStrip(e, (e.target as HTMLElement).closest('[data-part]'))
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
      if (!settingsUsed && isSetting()) Settings?.close('unused')
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
  // Split: the top bar (no Start, no programs, no show-the-desktop sliver), thin or not (style.css).
  body.classList.toggle('tb-split', isSplit())
  body.classList.toggle('tb-slim', isSlim())
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
  // capsule's panel in the island's home want theirs (picker.ts).
  const grows = setRoom(isReaching ? PULL_ROOM : Picker ? Picker.roomOver(roomFor(want), baseRoom()) : roomFor(want))
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
  // Editing the layout: kept as it goes (picker.ts); where its bar and the
  // rest are now (the picker's too, editing or not), for the Rust side.
  if (editLayout) Picker?.persist()
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
    Settings?.openLetter?.({ account: open.account, uid: open.uid, folder: open.folder || 'inbox' })
    return pet.openSettings('mail')
  }
  // A page, or a part of one (a new version: the updates).
  if (open?.tab) return pet.openSettings(open.tab)
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

// --- The flow light (flow.ts) -------------------------------------------------------
// In island-ring.ts:
const ringCore = {
  ANCHORED, CLIPS, Flow, REACTIONS, animateHer, body, compact, flow, her, herTalks, hintAccent, home, hue,
  ink, isCapsule, isHinting, isHome, isNudging, isOn, isRinging, isSide, island, modeNow, newsDot, nudgeFor,
  setStow, spins, stillMotionNow, taskHead, update,
  get config() { return config },
  get fullScreen() { return fullScreen },
  get quiet() { return quiet },
  get screenTop() { return screenTop },
  get view() { return view },
  get blends() { return blends }, set blends(v) { blends = v },
  get blinkTimer() { return blinkTimer }, set blinkTimer(v) { blinkTimer = v },
  get flowAnims() { return flowAnims }, set flowAnims(v) { flowAnims = v },
  get flowState() { return flowState }, set flowState(v) { flowState = v },
  get flowTimers() { return flowTimers }, set flowTimers(v) { flowTimers = v },
  get herClip() { return herClip }, set herClip(v) { herClip = v },
  get news() { return news }, set news(v) { news = v },
  get nudge() { return nudge }, set nudge(v) { nudge = v },
  get nudgeTimer() { return nudgeTimer }, set nudgeTimer(v) { nudgeTimer = v },
  get pending() { return pending }, set pending(v) { pending = v },
  get reactTimer() { return reactTimer }, set reactTimer(v) { reactTimer = v },
  get reacting() { return reacting }, set reacting(v) { reacting = v },
}
export type RingCore = typeof ringCore
const {
  animateRing, blink, cue, drawNews, placeFlow, placeNews, stopRing, stopStepping, takePending,
} = ringPart(ringCore)

// --- Pulling her out: the drop --------------------------------------------------
// In island-drop.ts:
const dropCore = {
  BODY_H, BODY_W, BREAK_PX, CLIPS, DROP_R, PULL_ROOM, REACH_R, REACH_SHARE, atBottom, body, dropHer,
  dropSheet, gooDrop, gooIsland, gooNeck, her, hidePlate, home, isHome, isNudging, isOn, isSetting, isSide,
  island, pressNudge, setRoom, setStow, sheetImage, showFrame, tbEdge, update,
  get clockTimer() { return clockTimer },
  get nudgeTimer() { return nudgeTimer },
  get spriteUrl() { return spriteUrl },
  get view() { return view },
  get comingHomeTimer() { return comingHomeTimer }, set comingHomeTimer(v) { comingHomeTimer = v },
  get dropTimer() { return dropTimer }, set dropTimer(v) { dropTimer = v },
  get herClip() { return herClip }, set herClip(v) { herClip = v },
  get isAbsorbing() { return isAbsorbing }, set isAbsorbing(v) { isAbsorbing = v },
  get isComingHome() { return isComingHome }, set isComingHome(v) { isComingHome = v },
  get isReaching() { return isReaching }, set isReaching(v) { isReaching = v },
  get nudge() { return nudge }, set nudge(v) { nudge = v },
  get pull() { return pull }, set pull(v) { pull = v },
  get pulledAt() { return pulledAt }, set pulledAt(v) { pulledAt = v },
  get retractTimer() { return retractTimer }, set retractTimer(v) { retractTimer = v },
}
export type DropCore = typeof dropCore
const { absorb, reach } = dropPart(dropCore)

// --- On and off ---------------------------------------------------------------

// At a top corner (anchor.ts): the island at that side of its window
// (style.css isl-at-left, isl-at-right), growing toward the screen's
// middle; the picker and its toasts under it there too (picker.css). Along
// the foot of the screen (isl-at-bottom, zhoupengjie/Wakuwaku#105): at the
// window's foot, --isl-foot up from it (centred in Windows' taskbar's
// strip under it, --isl-strip thick; 8 px with none), growing upward; the
// settings standing --isl-set-foot up from the foot (off the strip), the
// picker, its toasts and the capsule's panel above the island.
function placeAnchor() {
  const at = isOn() ? anchor() : 'center'
  body.classList.toggle('isl-at-left', at === 'left')
  body.classList.toggle('isl-at-right', at === 'right')
  const low = isOn() && atBottom()
  body.classList.toggle('isl-at-bottom', low)
  // Over Windows' taskbar along the top: centred in its strip too (isl-in-top).
  const high = inTopStrip()
  body.classList.toggle('isl-in-top', high)
  const root = document.documentElement.style
  root.setProperty('--isl-foot', `${low || high ? footAt() : TOP}px`)
  root.setProperty('--isl-strip', `${low || high ? stripUnder() : 0}px`)
  root.setProperty('--isl-set-foot', `${low || high ? settingsFoot() : TOP}px`)
  // Inside Windows' taskbar: the island about its capsule's middle there
  // (islandMx), as the taskbar's middle run has it (isl-mid, placeMid);
  // the picker and its toasts about it too (picker.css --isl-cx).
  body.classList.toggle('isl-embed', isEmbedded())
  if (home() !== 'taskbar') {
    slotMiddle = isEmbedded() && Number.isFinite(islandMx) ? islandMx / (devicePixelRatio || 1) : null
    if (slotMiddle != null) root.setProperty('--isl-cx', `${onPixels(slotMiddle)}px`)
    else root.removeProperty('--isl-cx')
    placeMid()
  }
}

// The prompt panel lives in the island while it is on (a banner in the
// settings while they are open), above the pet otherwise.
function seatPanel() {
  if (!isOn()) return
  const seat = isSetting() ? Settings.askSlot : island
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

pet.onUpdate(data => {
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
  // Where island.rs put her window at the top of the screen (anchor.ts),
  // or along its foot, over Windows' taskbar so thick (#105).
  islandAt = Anchor.posOf(data.islandAt)
  islandEdge = Anchor.edgeOf(data.islandEdge)
  islandStrip = Number.isFinite(data.islandStrip) && data.islandStrip > 0 ? data.islandStrip : 0
  // Her capsule inside Windows' taskbar (embed.rs): the island's window
  // about its middle, the capsule's width within its room.
  islandEmbed = data.islandEmbed === true
  islandMx = Number.isFinite(data.islandMx) ? data.islandMx : null
  islandRoom = Number.isFinite(data.islandRoom) && data.islandRoom > 0 ? data.islandRoom : 0
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
// of seconds, shared.rs redraw_widgets): on the strip, the readings' node
// alone; the island drawn anew where it shows them in its lines, or when
// another widget's words changed (the capsule shows them in turn).
pet.onDeskAsk(({ what, args }: { what?: string; args?: any } = {}) => {
  if (what === 'settings') {
    const w = allWidgets.find(x => x.id === args?.widget)
    return w && openWidgetSettings(w)
  }
  if (what === 'place') deskPlace(args)
  // A board took them from the library: off the taskbar (one place).
  if (what === 'unbar' && Array.isArray(args?.items)) offBar(args.items.filter(it => it && (typeof it.id === 'string' || typeof it.plugin === 'string')).slice(0, 8))
})

pet.onWidgets(({ widgets: all }) => {
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
pet.onQuiet(({ on }) => {
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
pet.onReact(({ react, say: text }) => {
  if (isOn() && text) cue(Flow.kindOfReact(react), { clip: 'waving', text: say(lang, text), ms: NUDGE_MS, react })
})

// An app full screen on her display, or none now: no ring while one is.
pet.onFullScreen(({ on }: { on?: boolean } = {}) => {
  fullScreen = on === true
})

// A widget asks to open the island (main checked that nothing wants you):
// a ring in its colour, its card in the island as it opens next (or, as
// the setting says, the island opened as before, or the dot alone). A new
// version or a crash report (not a widget's): the dot alone, in Windows'
// accent, the island opened next saying it; set to open, it opens.
pet.onNudge(n => {
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
    pet.hover(true)
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
    if (!pull?.started) pet.hover(false)
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
      if (screenTop?.notch && isSetting() && !typing) Settings?.close('left')
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
    if (isSetting() || Settings?.layer?.contains(e.target as Node) || Date.now() - pulledAt < 500) return
    if (view === 'ask' || panel.contains(e.target as Node) || her.contains(e.target as Node)) return
    // A widget's menu open in it: a press elsewhere closes it, and is all.
    if (menuFor?.where === 'isle') {
      if (!wmenu.contains(e.target)) closeMenu()
      return
    }
    // A button of a widget's own in its card (what is playing: previous,
    // play or pause, next; wcards.ts actionsHTML): to its plugin.
    const wa = view === 'expanded' && (e.target as HTMLElement).closest('[data-wact]')
    if (wa) {
      if (!wa.classList.contains('off')) pet.widgets?.act(wa.dataset.widget, wa.dataset.wact)
      return
    }
    // In a card unfolded: nothing more (it is read there).
    if (view === 'expanded' && (e.target as HTMLElement).closest('.icard')) return
    // The press, if the clocks redrew the session between it and the release.
    const id = view === 'expanded' && ((e.target as HTMLElement).closest('[data-jump]')?.dataset.jump || (Date.now() - pressed.at < 1000 && pressed.id))
    pressed = { id: '', at: 0 }
    if (id) {
      if (isNudging()) dismiss()
      return jumpTo(id)
    }
    if (isNudging() && !(e.target as HTMLElement).closest('[data-widget]')) return pressNudge()
    // Not connected: "don't remind me" puts the hint away; any other
    // press but on a session's or a widget's row opens the settings'
    // welcome (a session listed still goes to its window: the hooks
    // removed while it runs).
    if ((e.target as HTMLElement).closest('[data-hint-off]')) return pet.settings.set({ connectHintOff: true })
    if (isHinting() && !(e.target as HTMLElement).closest('[data-widget]')) return pet.openSettings('connect')
    // A widget in the open island: the one shown from now on, and its
    // settings open (a press that only lit it felt like none: the user,
    // 2026-10-13).
    const row = view === 'expanded' && (e.target as HTMLElement).closest('[data-widget]')
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
    pet.openSettings()
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
    const session = e.button === 0 && view === 'expanded' && (e.target as HTMLElement).closest('[data-jump]')
    pressed = { id: session ? session.dataset.jump : '', at: Date.now() }
  })
  target.addEventListener('contextmenu', e => {
    if ((e.target as HTMLElement).matches('input, textarea')) return
    e.preventDefault()
    if (wmenu.contains(e.target)) return
    // On a widget's row (or its card) in the open island: its menu.
    const row = view === 'expanded' && !isSetting() && ((e.target as HTMLElement).closest('[data-widget]') || (e.target as HTMLElement).closest('[data-card]'))
    const id = row && (row.dataset.widget || row.dataset.card)
    if (id && allWidgets.some(w => w.id === id)) return openMenu({ where: 'isle', widget: id }, row, { x: e.clientX, y: e.clientY })
    pet.menu()
  })
}

blink()

// The prompt waiting asked for from her capsule on another display's
// taskbar (bars.rs): it is open here already; a ring in the waiting
// colour goes round the island twice so the eye finds it (style.css
// look-here; held still a moment instead with reduced motion).
pet.onLookHere(() => {
  if (!isAsking()) return
  island.style.setProperty('--look', hue().waiting)
  island.classList.remove('look-here')
  void island.offsetWidth
  island.classList.add('look-here')
})
island.addEventListener('animationend', e => e.animationName.startsWith('look-here') && island.classList.remove('look-here'))
// Risen over an app full screen for the prompts alone: her island and its
// panel, not the strip under them (style.css over-full).
pet.onOverFull(({ on }: { on?: boolean } = {}) => body.classList.toggle('over-full', on === true))

// panel.ts tells the island when its prompt changes.
// The most the settings may be high: above the taskbar, its room (the
// window never moves for them); along the foot of the screen (the island
// home, #105), the window's height over where they stand (settingsFoot)
// and their shadow, in the page's own px; elsewhere, none said (the screen's).
// TASKBAR_ROOM is in the native monitor's logical px. WebView CSS px use
// devicePixelRatio, which can differ from that monitor scale (Windows text
// scaling): convert the room before sizing the settings page or its head
// and tabs can land above the native window and be clipped. Until the
// scale is told (before the first taskbar:strip; the preview never sends
// one), the two are taken as the same.
const settingsMax = () => (home() === 'taskbar' ? (isSide() ? innerHeight - 2 * TASKBAR_GAP - TASKBAR_SHADE : (TASKBAR_ROOM - TASKBAR_GAP - TASKBAR_SHADE) * (stripScale ? stripScale / (devicePixelRatio || 1) : 1)) : atBottom() ? innerHeight - settingsFoot() - TASKBAR_SHADE : null)

// The taskbar's layout, for the settings: its presets, which one it is, editing it on the strip.
// From her menu: editing it (添加小工具…: with the picker open, { picker }),
// or (退出编辑, while editing) out of it, kept as it went (picker.ts).
pet.onTaskbarEdit(p => (p?.end ? (endEdit(true), Picker?.closeAll()) : editTaskbar(p)))
// picker.ts, capsule-panel.ts (loaded after this file): what they work
// with here, read live. Internal to her home's page; settings.ts and
// panel.ts use Island.
const isle = {
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
  // At the top of the screen, the side the island is at (anchor.ts), and
  // the edge of the screen it is along: the top, or the bottom (#105);
  // and, there, whether the island is inside Windows' taskbar (embed.rs).
  anchor,
  islandEdge: () => (atBottom() ? 'bottom' : 'top'),
  isEmbedded,
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
export type IsleFace = typeof isle
provide('Isle', isle)
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
const islandFace = { accent: () => look.accent || null, isOn, isOpen: () => isOn() && view !== 'compact' && !body.classList.contains('isl-stowed'), peek: (on?: boolean) => setPeek(typeof on === 'boolean' ? on : view === 'compact'), changed: update, reach, absorb, settingsMax, tbPresets: TB_PRESETS, tbPreset: () => presetOf(config.taskbarLayout), tbWithPreset: name => TB_PRESETS[name] && Widgets.withParts(TB_PRESETS[name], tbLayout(config.taskbarLayout)), editTaskbar, whereOf, placeWidget, placeNow, anchor, notched: () => !!screenTop?.notch, ringAt, views: islandViews }
export type IslandFace = typeof islandFace
provide('Island', islandFace)
