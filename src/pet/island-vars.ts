// What her home's page remembers (island.ts and its parts, island-*.ts):
// every value that changes as the page runs, in one object made as the page
// starts (islandVars: st in island.ts, handed to each part in its core),
// read and written as st.<name> wherever it is used; its shape and what is
// being done with it (IslandState: st.view, st.isHover, st.nudge, st.pull...)
// read alone, moved on by island-state.ts's events (island.ts go). Only
// types and the values they start at: nothing here runs or touches the page.
import type * as Flow from './flow.ts'
import { initial, type IslandState } from './island-state.ts'
import type { Config } from '../shared/config.ts'

// The compact island at its narrowest: just her, when there is nothing to say.
export const MIN_W = 112

// What pet:update said last (shared.rs payload): the session shown, its
// mood and words, the other sessions, and what else comes with it (her
// plate there to have, Windows' light mode, not connected yet and the
// tools here).
export interface Now {
  mood: string
  detail: string
  project: string
  since: number | null
  took: number | null
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

// Windows' look as taskbar/ sent it last: its mode (dark, light) and
// accent colour; and whether its End task is on (a program's menu offers it).
// Its accent comes as its own colour and its shades for a light and a dark
// ground (taskbar/win/look.rs); the clock's seconds and the Task View
// button on or off as Windows has them.
export interface Look {
  mode: string
  accent?: { base: string; light: string; dark: string }
  endTask?: boolean
  seconds?: boolean
  taskView?: boolean
}

// A Mac's screen top as notch.rs says it (island:top): the band's height
// and the notch's width (0 without one).
export interface ScreenTop {
  height: number
  notch: number
}

// A program's button as taskbar/ sends it (taskbar:windows, buttons.rs):
// its key, its path and app id, its name, whether it is kept on the
// taskbar, its icon, and its windows.
export interface TaskApp {
  app: string
  path?: string
  id?: string | null
  name?: string
  pinned?: boolean
  png?: string
  windows?: TaskWindow[]
}
// One of its windows: its handle, title and picture; in front, minimized,
// maximized on the strip's display (max), flashing for attention.
export interface TaskWindow {
  id: number
  title?: string
  png?: string
  front?: boolean
  min?: boolean
  max?: boolean
  flash?: boolean
}

// A tray icon as taskbar/ sends it (taskbar:tray, tray.rs): its key, its
// picture (sent once: kept by the page, island-look.ts), tip, program and
// name, kept out on the taskbar by Windows, reached by a press.
export interface TrayIcon {
  key: number
  png: string
  tip?: string
  exe?: string
  name: string
  windowsOut?: boolean
  reach?: boolean
}

// A widget or a plugin she runs, as one is put somewhere (island-wmenu.ts
// placeWidget, the desktop's asks): by its id or its plugin's, its icon
// as it was put there.
export interface WidgetItem {
  id?: string
  plugin?: string
  icon?: string
}

// A widget's menu (island-wmenu.ts): on the strip or in the open island,
// for an entry there (a group's member by its index; -1, the group) or a
// widget.
export interface MenuFor {
  where: 'bar' | 'isle'
  entry?: string
  member?: number
  widget?: string | null
}

// A layout of the strip (widgets.ts layoutOf): its three runs, each its
// parts by name and the widgets put there.
export interface TbLayout {
  left: any[]
  center: any[]
  right: any[]
  [run: string]: any[]
}

export interface IslandVars extends IslandState {
  // --- What she is told ------------------------------------------------------------
  // The page's language, and what pet:update said last.
  lang: string
  now: Now
  // Her settings (Config), as pet:update brought them.
  config: Config
  // Her sheet (the sprite's URL; none: no pet yet), and a second session's.
  spriteUrl: string | null
  second: any
  // Windows' look (taskbar/win/look.rs).
  look: Look
  // Nobody at the keyboard or mouse for a while (pet:quiet): she stands still.
  quiet: boolean
  // An app full screen on her display (pet:full-screen, shared.rs look_fullscreen).
  fullScreen: boolean
  // Her window up on the desktop (shared.rs payload herUp): out there she says
  // it herself, the prompts over her head too (pet.ts, panel.ts), and the
  // island does not open for them.
  herUp: boolean

  // --- Where the island's window is ------------------------------------------------
  // On a Mac the island hangs from the screen's top edge over the menu bar
  // (notch.rs, island:top): { height, notch }, the band's height and the
  // notch's width (0 without one); null elsewhere. With a notch the compact
  // island wraps round it: her in the ear at its left, the clock in the ear at
  // its right, the words (which the notch would hide) left out.
  screenTop: ScreenTop | null
  // At the top of the screen, the side island.rs placed her window at
  // (pet:update islandAt: left, center, right; zhoupengjie/Wakuwaku#59): the
  // island laid out at that side of it (isl-at-left, isl-at-right), and all
  // that follows it (anchor.ts). None said (an older island.rs), the
  // centre, where it is.
  islandAt: string
  // The edge of the screen it placed the window along (islandEdge: top, or
  // bottom over Windows' own taskbar), and that taskbar's thickness
  // (islandStrip, physical px; 0 with none there): island.ts atBottom.
  islandEdge: string
  islandStrip: number
  // The island inside Windows' taskbar (islandEmbed; embed.rs), laid about
  // islandMx (the capsule's middle, physical px from the window's left),
  // within islandRoom (physical px; 0 none): island.ts isEmbedded, slotWidth.
  islandEmbed: boolean
  islandMx: number | null
  islandRoom: number

  // --- The shapes -----------------------------------------------------------------
  // The waits for the pointer resting on it (HOVER_OPEN_MS) and gone from it.
  hoverTimer: Timer | undefined
  // A nudge's time (st.nudge: island.ts nudgeFor).
  nudgeTimer: Timer | undefined
  // The notch's peek (st.peek) going back by itself after PEEK_MS.
  peekTimer: Timer
  // The settings opened in the notch and not used yet (island.ts update):
  // their wait to close, whether a move, a press, a wheel or a key came
  // since, and where the pointer was first seen in them (a move counts once
  // it goes a few px from there: WebKit may send one as they open under it).
  settingsIdleTimer: Timer
  settingsUsed: boolean
  settingsFrom: { x: number; y: number } | null
  // The window's room as last asked for (setRoom), and its wait to shrink.
  room: any
  roomTimer: Timer | undefined
  // The compact island's clocks ticking (tickTimes), and its width as drawn last.
  clockTimer: Timer | undefined
  compactWidth: number
  // What the compact island showed last, and when something else came: it
  // slides in, and carries on sliding in when the island is drawn again.
  shownKey: string
  turnedAt: number
  // The open island's lines scrolled this far (fillExpanded), and a card
  // just unfolded in them, to be brought into sight.
  linesTop: number
  cardFresh: boolean
  // The widget whose card is unfolded, and since when (its unfolding carries
  // on, not begun again, as the island is drawn anew every second).
  cardFor: string | null
  cardSince: number
  // The session a press in the open island landed on: the clocks redraw it
  // every second, so the release may come down on its redrawn self.
  pressed: { id: string; at: number }
  // Opened on the taskbar: told to the Rust side a moment after (tellOpened).
  openedTimer: Timer
  // The open island's ground held while it shrinks (holdGround).
  groundWait: any
  // What the plate was last told (where, as JSON; '': none): island-plate.ts.
  plateSent: string

  // --- Her in it -------------------------------------------------------------------
  // The clip she plays in it (animateHer) and its frames' clock; a blink's.
  herClip: string | null
  herTimer: Timer | undefined
  blinkTimer: Timer | undefined
  // A reaction inside a turn her portrait plays: { clip, until }.
  reacting: { clip: string; until: number } | null
  reactTimer: Timer | undefined
  // Her live portrait was showing at the last draw: one showing again is put
  // in its place at once, not sprung there from where she last stood (her
  // whole self, if she was pulled out of the open island).
  herShown: boolean
  // The portraits' clock while she is out (playPortraits).
  portraitTimer: Timer | null
  // When the last pull ended: the click the browser sends after it (the press
  // on her, the release on the island) is not a click on the island.
  pulledAt: number
  // The drop's frames (she kicks her legs in it).
  dropTimer: Timer | undefined
  // The reach's drop drawing back (st.isReaching); the wait for main to say
  // she is home (st.isComingHome).
  retractTimer: Timer | undefined
  comingHomeTimer: Timer | undefined

  // --- The flow light (island-ring.ts) ---------------------------------------------
  // The dot's event: its kind and (a plugin's) colour, and whether it shows
  // yet (it comes as the ring fades).
  news: { kind: string; colour?: string; shown: boolean } | null
  // What the island would have shown by itself: { clip, text, ms, widget,
  // until }, shown as it next opens (takePending), until then.
  pending: any
  // Its state (flow.ts), its animations running, its colours' blends, its
  // dot's and its end's timers.
  flowState: ReturnType<typeof Flow.idle>
  flowAnims: any[]
  blends: any[]
  flowTimers: any[]

  // --- The widgets -----------------------------------------------------------------
  // The widgets that are on, the one shown, and the turn to the next; all
  // with something to say, on or off (any may be on the taskbar).
  widgets: any[]
  widgetAt: number
  allWidgets: any[]
  spinTimer: Timer | null
  // The monitor, pinned (its setting, on by default): always in sight at the
  // island's right end (the bar's), never one of the turns. Unpinned it is
  // one of the widgets taking turns.
  monitor: any
  // Those put elsewhere: on the desktop (desk.rs), or on the taskbar while
  // it is her home. A widget is in one place (§7): neither taking turns in
  // the closed island nor pinned at its end, nor listed in the open island
  // (that lists her capsule's own, the picker of 2026-10-17).
  placedOut: Set<unknown>
  pinnedNow: boolean
  listedNow: any[]
  // picker.ts: her capsule's members (capsule.ts: the capsule setting's
  // order and modes), and those pinned at its right end.
  membersNow: any[]
  pinsNow: any[]

  // --- The taskbar -----------------------------------------------------------------
  // The moods' colours and the words on a light ground in her island
  // (setInk), and whether its ground is light (null: not known).
  inkLight: boolean
  groundLight: boolean | null
  // How light the desktop under the clear strip is, along it (mica.ts
  // lightness): what each part's words go by (inkParts); null, not known
  // (yet).
  backdrop: any
  // OLED's guard against burn-in (oledShift): on, and its next step.
  oledOn: boolean
  oledTimer: Timer | undefined
  // The monitor's scale as taskbar:strip says it (sf); 0 until told.
  stripScale: number
  // The program last in front (drawFront), and the strip built once.
  frontWas: any
  taskbarBuilt: boolean
  // While the layout is being edited: the one being made, not yet kept; and
  // the layout as last laid out (alignApps).
  editLayout: TbLayout | null
  arranged: string
  // Her slot's middle along the strip while it is in the middle run (placeSlot),
  // and where her island's left edge lies to be about it at a width: the
  // middle, kept 8 px off the screen's ends (as the settings at an end are).
  slotMiddle: number | null
  // The tray's icons as last sent ({ key, png, tip, exe }), as drawn, and
  // their places as last told; its overflow open.
  trayIcons: TrayIcon[]
  trayDrawn: string
  trayRectsSent: string
  trayOpen: boolean
  // The windows' buttons as last sent (taskbar/), and as drawn.
  taskWindows: TaskApp[]
  windowsDrawn: string
  // The programs' buttons drawn once already: a new one after that grows its line in.
  buttonsDrawn: boolean
  // The pointer gliding along the buttons (island-glide.ts).
  glideOn: HTMLElement | null
  // { app, kind: 'list' | 'menu' } while open (pressed: by a press on its
  // button), what it showed last, and its wait to open; shown and moving
  // along with the pointer (island-winlist.ts).
  popFor: { app: string; kind: 'list' | 'menu'; pressed?: boolean } | null
  popDrawn: string
  popTimer: Timer | undefined
  popShown: any
  popMoving: boolean
  popMoveTimer: Timer | undefined
  // The widget on the taskbar whose card is open (openWidgetPop: its entry,
  // and in a group the widget picked); the stack opened (openStackPop); a
  // widget's menu on the strip (openMenu); the input methods' list open
  // (openImeList).
  wgFor: any
  wgPick: any
  stackFor: any
  imeOpen: boolean
  // A widget's menu open (the spec's §10), and the open island's height for
  // it (0: none open).
  menuFor: MenuFor | null
  menuRoom: number
  // What the window was last told to catch the mouse over (sendSolid).
  solidSent: string
  // The keyboard while editing, for Esc to end it (taken a moment after
  // it starts: from her menu, which gives the keyboard back as it closes).
  editKeys: boolean
  // A press whose button went down where the window let it through and
  // came up on one of the bar's buttons: only its mouseup here, no click
  // (#53). Done as the click would have been.
  pressedHere: boolean
}

export function islandVars(flowIdle: ReturnType<typeof Flow.idle>): IslandVars {
  return {
    ...initial(),
    lang: 'en',
    now: { mood: 'idle', detail: '', project: '', since: null, took: null, others: 0, sessions: 0, list: [] },
    config: {},
    spriteUrl: null,
    second: null,
    look: { mode: 'dark' },
    quiet: false,
    fullScreen: false,
    herUp: false,

    screenTop: null,
    islandAt: 'center',
    islandEdge: 'top',
    islandStrip: 0,
    islandEmbed: false,
    islandMx: null,
    islandRoom: 0,

    hoverTimer: undefined,
    nudgeTimer: undefined,
    peekTimer: 0,
    settingsIdleTimer: 0,
    settingsUsed: false,
    settingsFrom: null,
    room: null,
    roomTimer: undefined,
    clockTimer: undefined,
    compactWidth: MIN_W,
    shownKey: '',
    turnedAt: 0,
    linesTop: 0,
    cardFresh: false,
    cardFor: null,
    cardSince: 0,
    pressed: { id: '', at: 0 },
    openedTimer: 0,
    groundWait: null,
    plateSent: '',

    herClip: null,
    herTimer: undefined,
    blinkTimer: undefined,
    reacting: null,
    reactTimer: undefined,
    herShown: false,
    portraitTimer: null,
    pulledAt: 0,
    dropTimer: undefined,
    retractTimer: undefined,
    comingHomeTimer: undefined,

    news: null,
    pending: null,
    flowState: flowIdle,
    flowAnims: [],
    blends: [],
    flowTimers: [],

    widgets: [],
    widgetAt: 0,
    allWidgets: [],
    spinTimer: null,
    monitor: null,
    placedOut: new Set(),
    pinnedNow: false,
    listedNow: [],
    membersNow: [],
    pinsNow: [],

    inkLight: false,
    groundLight: null,
    backdrop: null,
    oledOn: false,
    oledTimer: undefined,
    stripScale: 0,
    frontWas: null,
    taskbarBuilt: false,
    editLayout: null,
    arranged: '',
    slotMiddle: null,
    trayIcons: [],
    trayDrawn: '',
    trayRectsSent: '',
    trayOpen: false,
    taskWindows: [],
    windowsDrawn: '',
    buttonsDrawn: false,
    glideOn: null,
    popFor: null,
    popDrawn: '',
    popTimer: undefined,
    popShown: null,
    popMoving: false,
    popMoveTimer: undefined,
    wgFor: null,
    wgPick: null,
    stackFor: null,
    imeOpen: false,
    menuFor: null,
    menuRoom: 0,
    solidSent: '',
    editKeys: false,
    pressedHere: false,
  }
}
