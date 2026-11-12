// Where the settings are, as a state and the events that move it on (next):
// open or not and since when, the page shown, a part of the Plugins page
// opened in its place (and the one under it, to go back to), a part of a
// page to scroll to once drawn, and what is folded out on General and the
// Agent page. Nothing here touches the page: settings.ts and its parts send
// the events (go) and draw what the state says. Tested in
// test/settings-state.test.ts.

// Five pages: General (the sessions, the alerts, startup and the language),
// Agent (connecting them, right after General), Look (her home, Windows,
// her and the pets), Plugins and Mail. The pets, the alerts and connecting
// were pages of their own: asked for by those names (island.rs asks for the
// pets, or connecting), the page they are part of opens, scrolled to them
// (PART_OF, settings.ts showPart); connecting while the welcome shows is
// the welcome, at General's top.
export const TABS = ['now', 'agents', 'look', 'widgets', 'mail']
export const PART_OF: Record<string, string> = { pets: 'look', place: 'look', paper: 'look', shortcuts: 'now', alerts: 'now', connect: 'agents', update: 'now', net: 'now', crash: 'now' }

// A part of the Plugins page in its place: a plugin's own page, a pack's
// ({ kind } too), the market, one of the market's (mkItem); back, the one it
// was opened over.
export interface Sub {
  readonly page: string
  readonly id?: string
  readonly kind?: string
  readonly back: Sub | null
}

export interface SettingsState {
  readonly isOpen: boolean
  // When they last opened (on a Mac a blur that soon after is the keyboard
  // taken, not a click outside).
  readonly openedAt: number
  readonly tab: string
  readonly sub: Sub | null
  // A part of a page asked for by its old page's name, to scroll to once drawn.
  readonly partToShow: string
  // The welcome open at General's top; the agent whose row is open on the
  // Agent page, and the others not found folded out there.
  readonly welcomeOpen: boolean
  readonly agentOpen: string
  readonly othersOpen: boolean
}

export type SettingsEvent =
  // Opened (or asked for again while open) at what was asked: a page, a
  // part of one by its old page's name, a widget's row (widgets:<id>), the
  // market (market: AddonUI.marketLink found one), or connecting.
  | { type: 'open'; to: string; market: boolean; now: number }
  // The snapshot in, as they open: connecting while the welcome card shows
  // is the card, at General's top.
  | { type: 'shown'; cardShows: boolean }
  | { type: 'close' }
  // A tab pressed (one of TABS, not the one shown); a page gone to from
  // within (its part to scroll to, if any).
  | { type: 'tab'; to: string }
  | { type: 'page'; tab: string; part?: string }
  // A part to scroll to; scrolled to.
  | { type: 'part'; part: string }
  | { type: 'part-shown' }
  // A part of the Plugins page opened over the one shown; back to that one;
  // the Plugins page as itself; a plugin or pack taken away (its own page goes).
  | { type: 'sub'; sub: { page: string; id?: string; kind?: string } }
  | { type: 'back' }
  | { type: 'sub-off' }
  | { type: 'gone'; id: string }
  // An agent's row on the Agent page opened, or closed (pressed again,
  // unless keep); one that failed to connect opened (on the Agent page);
  // the others folded out or in; the welcome opened or folded.
  | { type: 'agent'; id: string; keep: boolean }
  | { type: 'failed'; id: string }
  | { type: 'others' }
  | { type: 'welcome'; open: boolean }

export function initial(): SettingsState {
  return { isOpen: false, openedAt: 0, tab: 'now', sub: null, partToShow: '', welcomeOpen: false, agentOpen: '', othersOpen: false }
}

// The state alone, out of what holds it among other things (settings.ts st).
export function stateOf(s: SettingsState): SettingsState {
  const { isOpen, openedAt, tab, sub, partToShow, welcomeOpen, agentOpen, othersOpen } = s
  return { isOpen, openedAt, tab, sub, partToShow, welcomeOpen, agentOpen, othersOpen }
}

export function next(s: SettingsState, e: SettingsEvent): SettingsState {
  switch (e.type) {
    case 'open': {
      let t = e.to === 'connect' ? { ...s, welcomeOpen: true } : s
      // The market, by category: on the Plugins page, in its place.
      if (e.market) t = { ...t, tab: 'widgets', sub: { page: 'market', back: null } }
      else if (PART_OF[e.to]) t = { ...t, tab: PART_OF[e.to], partToShow: e.to }
      // A widget's row on the plugins page (a press on it in the open island).
      else if (String(e.to).startsWith('widgets:')) t = { ...t, tab: 'widgets', partToShow: e.to }
      else if (TABS.includes(e.to)) t = { ...t, tab: e.to }
      return { ...t, isOpen: true, openedAt: s.isOpen ? s.openedAt : e.now }
    }
    case 'shown':
      return s.partToShow === 'connect' && e.cardShows ? { ...s, tab: 'now', partToShow: 'welcome' } : s
    case 'close':
      // The welcome opened again only for this time; the Agent page opens
      // again with its rows closed; the Plugins page as itself.
      return { ...s, isOpen: false, welcomeOpen: false, agentOpen: '', othersOpen: false, sub: null }
    case 'tab':
      return e.to === s.tab || !TABS.includes(e.to) ? s : { ...s, tab: e.to, sub: null }
    case 'page':
      return { ...s, tab: e.tab, sub: null, partToShow: e.part ?? s.partToShow }
    case 'part':
      return { ...s, partToShow: e.part }
    case 'part-shown':
      return { ...s, partToShow: '' }
    case 'sub':
      return { ...s, sub: { ...e.sub, back: s.sub } }
    case 'back':
      return { ...s, sub: s.sub?.back || null }
    case 'sub-off':
      return { ...s, sub: null }
    case 'gone':
      return (s.sub?.page === 'addon' || s.sub?.page === 'pack') && s.sub.id === e.id ? { ...s, sub: null } : s
    case 'agent':
      return { ...s, agentOpen: s.agentOpen === e.id && !e.keep ? '' : e.id }
    case 'failed':
      return s.tab === 'agents' ? { ...s, agentOpen: e.id } : s
    case 'others':
      return { ...s, othersOpen: !s.othersOpen }
    case 'welcome':
      return { ...s, welcomeOpen: e.open }
  }
}
