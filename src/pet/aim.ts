// The way from a program's button on her taskbar up to its windows' list
// (zhoupengjie/Wakuwaku#56): with one program's list open, when another's
// button the pointer is on takes it over. A hand goes up in an arc, often
// out to the side first, and the cards at a wide list's ends lie over the
// neighbouring buttons: the list changed at once to whichever button it
// crossed. Now it changes only once the pointer has been on the other
// button as long as Windows' own taskbar waits, and not while it is on its
// way into the list (the menus' "safe triangle"). Imported by the main
// taskbar's page (island.ts) and the other displays' (bar.ts), and by the
// tests.
//
// Windows 11's own taskbar, measured in the test VM (26300, a 2560 x 1440
// display at 125%, its taskbar at the bottom, 2026-10-16; SendInput moves,
// the list read off the screen every 25 ms):
// - a program's list showed 0.63 to 0.76 s after the pointer came onto its
//   button, its fade counted: hers opens after OPEN_MS and rises in 0.18 s;
// - with one open, the pointer on another program's button for 211 ms
//   changed nothing, for 235 ms it changed it (at 225 ms, once of twice);
//   the same with the pointer moving on that button the while (slowly
//   across it, up it towards the list, to and fro): Windows counts the time
//   on the button and nothing else, so a slow way up across a neighbour
//   lost the list; hers does not count the time spent heading into the list;
// - between the button and the list it stayed, however long (its room
//   reaches down to the strip: style.css, barpop.css);
// - gone from both, it went 0.45 to 0.48 s later, its fade counted:
//   CLOSE_MS (island.ts, barpop.ts).
const OPEN_MS = 500
const SWITCH_MS = 225
const CLOSE_MS = 400
// Heading into the list: from where the pointer was about this long ago
// to where it is. One move's step alone is a pixel or two, too little to
// say where it goes.
const TRAIL_MS = 80
// Not moved for this long: at rest, heading nowhere.
const REST_MS = 60
// The list's near side taken this much longer at each end (px).
const SLACK_PX = 6

// p in the triangle a, b, c (on an edge counts).
// A point on the screen, and a list's box (a DOMRect's sides).
interface Point {
  x: number
  y: number
}
interface Box {
  left: number
  top: number
  right: number
  bottom: number
}
// The list shown: its button's key, its box, the strip's edge it opens from.
interface List {
  key: unknown
  rect: Box
  edge: string
}

function inTriangle(p: Point, a: Point, b: Point, c: Point) {
  const side = (u: Point, v: Point, w: Point) => (v.x - u.x) * (w.y - u.y) - (v.y - u.y) * (w.x - u.x)
  const d1 = side(p, a, b)
  const d2 = side(p, b, c)
  const d3 = side(p, c, a)
  const neg = d1 < 0 || d2 < 0 || d3 < 0
  const pos = d1 > 0 || d2 > 0 || d3 > 0
  return !(neg && pos)
}

// The two ends of the list's side that faces the strip ({ left, top,
// right, bottom }), by the strip's edge of the screen: a bottom bar's
// list stands above it, a left bar's to its right.
function nearSide(list: Box, edge: string, slack = SLACK_PX): [Point, Point] {
  if (edge === 'top') return [{ x: list.left - slack, y: list.top }, { x: list.right + slack, y: list.top }]
  if (edge === 'left') return [{ x: list.left, y: list.top - slack }, { x: list.left, y: list.bottom + slack }]
  if (edge === 'right') return [{ x: list.right, y: list.top - slack }, { x: list.right, y: list.bottom + slack }]
  return [{ x: list.left - slack, y: list.bottom }, { x: list.right + slack, y: list.bottom }]
}

// Gone from `from` to `to`, heading into the list: `to` in the triangle
// from `from` to the ends of the list's near side. Along the strip, or
// away from the list, is not; nor staying put.
function heading(from: Point | null | undefined, to: Point | null | undefined, list: Box | null | undefined, edge: string, slack = SLACK_PX) {
  if (!from || !to || !list || (from.x === to.x && from.y === to.y)) return false
  const [b, c] = nearSide(list, edge, slack)
  return inTriangle(to, from, b, c)
}

// The pointer's way along the buttons, for one strip: told where it moves
// (move) and which program's button it is on (over); due says how long
// until that program's list is to show.
function create({ openMs = OPEN_MS, switchMs = SWITCH_MS, trailMs = TRAIL_MS, restMs = REST_MS, slack = SLACK_PX } = {}) {
  // Where it was, oldest first: the newest point older than trailMs, and
  // all since.
  const trail: (Point & { t: number })[] = []
  // The button it is on ({ key, since }), counted from when it came, or
  // from when it last headed into the list.
  let on: { key: unknown; since: number } | null = null
  const api = {
    move(x: number, y: number, t: number) {
      const last = trail[trail.length - 1]
      if (last && last.x === x && last.y === y) return
      trail.push({ x, y, t })
      while (trail.length > 2 && trail[1].t <= t - trailMs) trail.shift()
    },
    // On a program's button (its key), or on none (null).
    over(key: unknown, t: number) {
      if ((key ?? null) === (on?.key ?? null)) return
      on = key == null ? null : { key, since: t }
    },
    key: () => on?.key ?? null,
    // Heading into the list ({ key, rect, edge }) now, at t.
    heading(list: List | null | undefined, t: number) {
      const now = trail[trail.length - 1]
      if (!list || !now || t - now.t > restMs || trail.length < 2) return false
      return heading(trail[0], now, list.rect, list.edge, slack)
    },
    // How long until the list is to be the button's: ms (0: now), or null
    // for nothing to do (on no button, or its list the one shown). list:
    // the list shown ({ key, rect: { left, top, right, bottom }, edge }),
    // or null. Heading into it, the time on the button counts from its
    // last move, and it is asked again in restMs.
    due(list: List | null | undefined, t: number): number | null {
      if (!on) return null
      if (!list) return Math.max(0, openMs - (t - on.since))
      if (list.key === on.key) return null
      if (api.heading(list, t)) {
        on.since = Math.max(on.since, trail[trail.length - 1].t)
        return restMs
      }
      return Math.max(0, switchMs - (t - on.since))
    },
  }
  return api
}

export { create, heading, inTriangle, nearSide, OPEN_MS, SWITCH_MS, CLOSE_MS, TRAIL_MS, REST_MS, SLACK_PX }
