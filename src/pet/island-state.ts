// Her island's shape and what is being done with it, as a state and the
// events that move it on (next): which shape it has (compact, open, a
// prompt, the settings), the pointer resting on it, a press that closed
// it, something that opened it by itself (a nudge), the peek from the
// notch, and her pulled out of it, reached for and taken back in. Nothing
// here touches the page or a timer: island.ts and its parts send the
// events (go) and draw what the state says; the waits (the hover's, a
// nudge's time) stay theirs. Tested in test/island-state.test.ts.

export type View = '' | 'compact' | 'expanded' | 'ask' | 'settings'

// Something that opened the island by itself (island.ts nudgeFor; a kept
// one as it opens next, island-ring.ts takePending): open until `until`
// (Infinity: until seen), her playing clip, text (a hello) in place of the
// mood's line, a widget's card; kind, what opened it (an ending's goes as
// its session is at work again), and after, its own time once seen.
export interface Nudge {
  readonly until: number
  readonly clip: string
  readonly text: string | null
  readonly widget: any
  readonly kind?: string | null
  readonly after?: number
}

// Her being pulled out of it (island-drop.ts): pressed at (x0, y0); started
// once moved a few px (the drop stretches); broken once the drop pinches off
// (at: where the drop has her); carrying once her own window has her.
export interface Pull {
  readonly x0: number
  readonly y0: number
  readonly started: boolean
  readonly broken: boolean
  readonly at: { readonly cx: number; readonly cy: number } | null
  readonly carrying: boolean
}

export interface IslandState {
  // The shape drawn: '' before the first draw and while the island is off.
  readonly view: View
  // The pointer rested on it (HOVER_OPEN_MS after it came).
  readonly isHover: boolean
  // Closed by a press while it was open by itself (dismiss): the pointer
  // still on it opens it no more, until it has left and come back.
  readonly dismissed: boolean
  // Dropped open from the notch by her tray icon (setPeek).
  readonly peek: boolean
  readonly nudge: Nudge | null
  readonly pull: Pull | null
  // Reaching for her (her middle on the screen, close), or taking her in.
  readonly isReaching: boolean
  readonly isAbsorbing: boolean
  // Taken back in: she is home from then on for this page, before main
  // says so, so she lands in her live portrait and not in the still one.
  readonly isComingHome: boolean
}

export type IslandEvent =
  // The shape decided anew (update): the settings open, a prompt shown, now.
  | { type: 'draw'; settings: boolean; asking: boolean; now: number }
  // The pointer rested on it; came off it (at once); gone a while (now).
  | { type: 'rest' }
  | { type: 'leave' }
  | { type: 'gone'; now: number }
  // Held open (a card unfolded in it) or closed (editing the layout) as it is.
  | { type: 'hover'; on: boolean }
  // Closed by a press while open by itself.
  | { type: 'dismiss' }
  // Opened by itself; its time up (or what opened it seen); held until
  // seen and the pointer come (now): its own time from now on.
  | { type: 'nudge'; nudge: Nudge }
  | { type: 'unnudge' }
  | { type: 'seen'; now: number }
  // The peek from the notch, open or closed.
  | { type: 'peek'; on: boolean }
  // Her pulled out: pressed, moved (the drop), stretched, pinched off (her
  // window has her), the button up, back in her seat; or her window let
  // her land with no button up heard here.
  | { type: 'press'; x: number; y: number }
  | { type: 'pull' }
  | { type: 'stretch'; broken: boolean; at: { cx: number; cy: number } }
  | { type: 'carry' }
  | { type: 'release' }
  | { type: 'settled' }
  | { type: 'landed' }
  // Reaching for her, or not; taking her in; taken in or not after all
  // (her window out no more, or main never said she was in).
  | { type: 'reach'; on: boolean }
  | { type: 'absorb' }
  | { type: 'homecoming-over' }
  // The page her own window's (role=pet), or the island off: nothing of its own.
  | { type: 'off' }

export function initial(): IslandState {
  return { view: '', isHover: false, dismissed: false, peek: false, nudge: null, pull: null, isReaching: false, isAbsorbing: false, isComingHome: false }
}

// The state alone, out of what holds it among other things (island.ts st).
export function stateOf(s: IslandState): IslandState {
  const { view, isHover, dismissed, peek, nudge, pull, isReaching, isAbsorbing, isComingHome } = s
  return { view, isHover, dismissed, peek, nudge, pull, isReaching, isAbsorbing, isComingHome }
}

// Open by itself, now.
export const nudging = (s: IslandState, now: number): boolean => !!s.nudge && now < s.nudge.until

// While she is being pulled out (not yet carried) or taken back in, the
// pull has the island: its shape is not drawn anew.
export const held = (s: IslandState): boolean => !!(s.pull?.started && !s.pull.carrying) || s.isAbsorbing

// Its shape: the settings over all, a prompt, then open while the pointer
// rests on it, something opened it by itself or the notch's peek is down.
export function viewOf(s: IslandState, settings: boolean, asking: boolean, now: number): View {
  if (settings) return 'settings'
  if (asking) return 'ask'
  return s.isHover || nudging(s, now) || s.peek ? 'expanded' : 'compact'
}

export function next(s: IslandState, e: IslandEvent): IslandState {
  switch (e.type) {
    case 'draw': {
      // A prompt answered: the island closes rather than lingering open.
      const t = s.view === 'ask' && !e.asking ? { ...s, nudge: null } : s
      return { ...t, view: viewOf(t, e.settings, e.asking, e.now) }
    }
    case 'rest':
      return s.dismissed ? s : { ...s, isHover: true }
    case 'leave':
      return { ...s, dismissed: false }
    case 'gone':
      // Open by itself, rested on and left: it has been seen, so it closes
      // now. Only passed across, the hover never taken, it stays open.
      return { ...s, isHover: false, nudge: s.isHover && nudging(s, e.now) ? null : s.nudge }
    case 'hover':
      return { ...s, isHover: e.on }
    case 'dismiss':
      return { ...s, nudge: null, isHover: false, dismissed: true }
    case 'nudge':
      return { ...s, nudge: e.nudge }
    case 'unnudge':
      return { ...s, nudge: null }
    case 'seen':
      if (!s.nudge || Number.isFinite(s.nudge.until)) return s
      // Held until seen: only island.ts nudgeFor holds one so, and it always
      // gives it its own time (after).
      return { ...s, nudge: { ...s.nudge, until: e.now + s.nudge.after! } }
    case 'peek':
      return { ...s, peek: e.on }
    case 'press':
      return { ...s, pull: { x0: e.x, y0: e.y, started: false, broken: false, at: null, carrying: false } }
    case 'pull':
      // The drop has her: what opened the island by itself is over.
      return s.pull ? { ...s, pull: { ...s.pull, started: true }, nudge: null } : s
    case 'stretch':
      return s.pull ? { ...s, pull: { ...s.pull, broken: e.broken, at: e.at } } : s
    case 'carry':
      return s.pull ? { ...s, pull: { ...s.pull, carrying: true } } : s
    case 'release':
      // Let go: a press that never moved, or her carried off, is over; one
      // let go in the drop springs back into her seat first (settled).
      return { ...s, pull: s.pull?.started && !s.pull.carrying ? s.pull : null }
    case 'settled':
      return { ...s, pull: null, isAbsorbing: false }
    case 'landed':
      return { ...s, pull: null }
    case 'reach':
      return { ...s, isReaching: e.on }
    case 'absorb':
      return { ...s, isReaching: false, isAbsorbing: true, isComingHome: true }
    case 'homecoming-over':
      return { ...s, isComingHome: false }
    case 'off':
      return { ...s, pull: null, isReaching: false, isAbsorbing: false, view: '' }
  }
}
