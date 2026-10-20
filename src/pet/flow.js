// The flow light (流光提醒, the design session's spec of 2026-10-16): what
// happens on her closed capsule when something happens that does not need
// the person at once. A turn done, changes to review, an error, a hello, a
// plugin's nudge: the island no longer opens by itself (unless set to, as
// before); a thin ring of colour runs round the capsule twice, in steps
// (15 a second), stays lit and still for as long as the hold setting says
// (or until seen), and fades, and a small dot on her portrait stays until it has been seen. Only
// a session waiting on the person (to approve, answer or choose) always
// opens it; what happens inside a turn (a tool failed) only plays her clip.
// What to do for an
// event, the ring's turns and fades as times and angles, how one coming on
// another's heels carries on from it, and the colours: all here, with no
// page, so the tests can say it. island.js draws it. Loaded by the page
// before island.js, and required by the tests.
;(function (root) {
  // The events, and which colours win while one ring is playing: an error
  // over changes to review, over a turn done, over a hello, over a plugin.
  const PRIORITY = { error: 5, review: 4, done: 3, hello: 2, plugin: 1 }
  const ENDINGS = ['done', 'review', 'error']
  const isEnding = kind => ENDINGS.includes(kind)

  // Its turns (the user, 2026-10-18: two, in steps of 15 a second, then
  // still; they were three, smooth, 2.55 s): a turn at their average speed,
  // two in 2 s.
  const TURN_MS = 1000
  const TURNS = 2
  // Its curve: easing in and out, gently enough that no step turns it more
  // than 34° (the fastest, mid-way: 1.43 times the average).
  const EASE = [0.3, 0, 0.7, 1]
  // Its turning is drawn in steps, a new angle 15 times a second (each a
  // frame; nothing animated between them), as the user chose over every
  // frame: a frame of her window costs the same whatever it shows, and her
  // screen may show 280 of them a second (docs/PERFORMANCE.md). Its light
  // comes up and goes down smoothly (opacity alone, on the compositor).
  const SPIN_FPS = 15
  const STEP_MS = 1000 / SPIN_FPS
  // Its light: 0.28 s up, held, and down over 0.56 s from 0.31 s before the
  // turning ends to 0.25 s after it (held by nothing: as before there was a
  // hold).
  const FADE_IN_MS = 280
  const FADE_LEAD_MS = 310
  const TAIL_MS = 250
  // One colour into the next, as another comes while it plays.
  const BLEND_MS = 300
  // Another within this long of a ring's end: no ring again, the dot hops.
  const AGAIN_MS = 8000
  // With Windows' animation effects off (reduced motion): no turning, the
  // ring lit still 1.4 s (0.2 s up, 0.2 s down), no glow, the dot no hop.
  const STILL_MS = 1400
  const STILL_FADE_MS = 200
  // Held (the hold setting, zhoupengjie/Wakuwaku#65): its light down over
  // the same 0.56 s once the hold is over, from lit, still.
  const FADE_OUT_MS = FADE_LEAD_MS + TAIL_MS
  // The dot's coming (0.25 s, a little past full size and back) and its hop.
  const DOT_IN_MS = 250
  const HOP_MS = 300

  // The setting (endCue) for all she has to tell that needs no answer (a
  // turn done, changes to review, an error, a hello, a plugin's nudge):
  // open the island (as before), a ring (the default), or nothing but the
  // dot. One kept from before it was a choice (openOnEnd off) is nothing.
  function endCue(config) {
    const set = config?.endCue
    if (set === 'open' || set === 'ring' || set === 'none') return set
    return config?.openOnEnd === false ? 'none' : 'ring'
  }

  // What an event does, by its kind (waiting, done, review, error, hello,
  // plugin, notice: a new version or a crash report, or process: a reaction
  // inside a turn), the setting, the island's shape now (compact, expanded,
  // ask, settings) and whether a ring may play now (blocked: something full
  // screen, nobody at the computer, do not disturb, a Mac's notch):
  //   open    the island opens by itself, as it always did
  //   show    it is open: what it says goes into it (no ring, no dot)
  //   later   it is a prompt or the settings: kept for the next look, no dot
  //   ring    a ring, then the dot
  //   dot     the dot alone (a ring would not be seen, or is not wanted)
  //   clip    her portrait plays the reaction's clip, and nothing else
  //   null    nothing (an ending while the island is open: it shows it)
  // Set to open, everything is as it was before the flow light: whatever
  // opened the island by itself still does.
  function cueFor(kind, { setting = 'ring', view = 'compact', blocked = false } = {}) {
    if (kind === 'waiting' || setting === 'open') return 'open'
    // Words sent to her to say to you (say: POST /say, main.rs say): said
    // out, whatever the setting, that being why they were sent; kept for a
    // prompt or the settings, the dot alone where a ring would be blocked.
    if (kind === 'say') return view === 'expanded' ? 'show' : view === 'ask' || view === 'settings' ? 'later' : blocked ? 'dot' : 'open'
    // A tool failed, an item of the turn's list done: how it goes, not how
    // it ended. An agent mostly finds its way round a failed tool, and a
    // dot would say something was left undone (the design session).
    if (kind === 'process') return 'clip'
    if (view === 'expanded') return isEnding(kind) ? null : 'show'
    if (view === 'ask' || view === 'settings') return isEnding(kind) ? null : 'later'
    // A new version or a crash report is never urgent: no turning in the
    // corner of the eye (the design session, 2026-10-16).
    if (kind === 'notice' || setting === 'none') return 'dot'
    return blocked ? 'dot' : 'ring'
  }

  // Her reactions with words (state.rs react): a hello (her arriving, a
  // session starting) rings; a task-list item done or a tool that failed
  // is inside a turn (process). Only a turn that ends in error rings red.
  const REACT_KIND = { wave: 'hello', jump: 'process', failed: 'process' }
  const kindOfReact = react => REACT_KIND[react] || 'process'

  // The hold setting as a time (ms): its seconds, or until seen (Infinity)
  // for "seen" and for anything that is not a number of them, as main reads
  // it (main.rs hold).
  function holdMs(hold) {
    return Number.isInteger(hold) && hold > 0 ? hold * 1000 : Infinity
  }

  // How long what the island would have shown (a hello's words, a plugin's
  // card) stays to be seen on the next hover: as long as an ending stays
  // (the hold setting: 3 to 600 s, or until seen); a notice until seen.
  function keepFor(kind, hold) {
    if (kind === 'notice' || kind === 'say') return Infinity
    return holdMs(hold)
  }

  // What the 「她有事告诉你时」 setting (endCue) governs: a turn's end,
  // changes to review, an error, a hello, a plugin's nudge.
  const CUED = ['done', 'review', 'error', 'hello', 'plugin']

  // How long the island stays open by itself (endCue open, cueFor's open):
  // what endCue governs as long as the hold says (it stayed its fixed few
  // seconds whatever was chosen, zhoupengjie/Wakuwaku#65), until seen
  // being Infinity: open until the pointer has been over it (island.js
  // seenNudge). A session waiting on you, a new version, what happens
  // inside a turn: their own time (ms), as before.
  function openFor(kind, hold, ms) {
    return CUED.includes(kind) ? holdMs(hold) : ms
  }

  // The hold setting: whole seconds from 3 to 600 (settings.rs is_hold), or
  // "seen". 8, 30 and 120 are its quick choices (settings.js); any other
  // number is a custom one (#59).
  const HOLD_MIN = 3
  const HOLD_MAX = 600
  const HOLD_QUICK = [8, 30, 120]

  // Seconds typed in Custom's box, as kept: rounded, within 3 to 600 (an
  // input method's full-width digits, or a unit after them, taken as well).
  // Nothing typed, or no number: null, and the box goes back to what it was.
  function holdSecs(typed) {
    const text = String(typed ?? '')
      .replace(/[０-９]/g, d => String(d.charCodeAt(0) - 0xff10))
      .replace(/\s*(秒|s|secs?|seconds?)$/i, '')
      .trim()
    const secs = Number(text)
    if (!text || !Number.isFinite(secs)) return null
    return Math.min(HOLD_MAX, Math.max(HOLD_MIN, Math.round(secs)))
  }

  // Which choice is on: until seen, a quick one, or custom (a number of its
  // own, or Custom picked to type one in). No whole number: until seen, as
  // main reads it (main.rs hold).
  function holdChoice(hold, custom = false) {
    if (custom) return 'custom'
    if (!Number.isInteger(hold) || hold < 0) return 'seen'
    return HOLD_QUICK.includes(hold) ? hold : 'custom'
  }

  // --- The colours ----------------------------------------------------------------------
  // Each event its own set, colourful but still saying what happened; the
  // first and last the same, so the gradient's seam does not show. Deeper on
  // a light ground (the dark set's pale yellow is lost on a light strip).
  const PALETTES = {
    done: {
      dark: ['#ff6b6b', '#ffb340', '#ffe066', '#34d27b', '#5ac8fa', '#8e7cff', '#ff6bd6', '#ff6b6b'],
      light: ['#e5484d', '#e8890c', '#c99a00', '#1f9d55', '#1a8fd6', '#6e56cf', '#d6409f', '#e5484d'],
    },
    review: {
      dark: ['#5ac8fa', '#8e7cff', '#c38bff', '#5ac8fa', '#8e7cff', '#c38bff', '#5ac8fa'],
      light: ['#1a8fd6', '#6e56cf', '#9a5cd0', '#1a8fd6', '#6e56cf', '#9a5cd0', '#1a8fd6'],
    },
    error: {
      dark: ['#ff453a', '#ff9f0a', '#ff453a', '#ff9f0a', '#ff453a'],
      light: ['#d93025', '#e8710a', '#d93025', '#e8710a', '#d93025'],
    },
    hello: {
      dark: ['#ffd36b', '#ffb3c7', '#ffd36b', '#ffb3c7', '#ffd36b'],
      light: ['#d9a400', '#e0729a', '#d9a400', '#e0729a', '#d9a400'],
    },
  }

  const rgbOf = hex => {
    const m = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(String(hex || '').trim())
    if (!m) return null
    const h = m[1].length === 3 ? [...m[1]].map(c => c + c).join('') : m[1]
    return [0, 2, 4].map(i => parseInt(h.slice(i, i + 2), 16))
  }
  const hexOf = rgb => `#${rgb.map(v => Math.round(Math.min(255, Math.max(0, v))).toString(16).padStart(2, '0')).join('')}`
  // A colour with that much of another in it (sRGB, as CSS color-mix).
  function mix(colour, other, share) {
    const a = rgbOf(colour)
    const b = rgbOf(other)
    if (!a || !b) return colour
    return hexOf(a.map((v, i) => v * (1 - share) + b[i] * share))
  }

  // A plugin's: its colour and the same with 45 % white in it, by turns; on
  // a light ground its colour 15 % deeper first. A plugin with no colour of
  // its own (or not one of ours to mix) rings in the fallback given (the
  // accent).
  function palette(kind, light, colour, fallback = '#4cc2ff') {
    const set = PALETTES[kind]
    if (set) return set[light ? 'light' : 'dark']
    let c = rgbOf(colour) ? colour : rgbOf(fallback) ? fallback : '#4cc2ff'
    if (light) c = mix(c, '#000000', 0.15)
    const pale = mix(c, '#ffffff', 0.45)
    return [c, pale, c, pale, c]
  }

  const gradient = stops => `conic-gradient(${stops.join(', ')})`

  // The soft glow under the ring (the same gradient blurred): on a dark
  // ground only (on a light one it smudges), and not with reduced motion.
  const glows = (light, still) => !light && !still

  // --- The ring's turning ---------------------------------------------------------------
  // A cubic-bezier easing (CSS's): at x, the y; and the slope there.
  function bezier([x1, y1, x2, y2]) {
    const at = (a, b, u) => 3 * a * (1 - u) * (1 - u) * u + 3 * b * (1 - u) * u * u + u * u * u
    const slope = (a, b, u) => 3 * a * (1 - u) * (1 - u) + 6 * (b - a) * (1 - u) * u + 3 * (1 - b) * u * u
    const solve = x => {
      let u = x
      for (let i = 0; i < 8; i++) {
        const d = slope(x1, x2, u)
        if (Math.abs(d) < 1e-6) break
        u -= (at(x1, x2, u) - x) / d
        u = Math.min(1, Math.max(0, u))
      }
      // Newton's steps may stall at a flat part: halve the way there.
      let lo = 0
      let hi = 1
      for (let i = 0; i < 30 && Math.abs(at(x1, x2, u) - x) > 1e-7; i++) {
        if (at(x1, x2, u) < x) lo = u
        else hi = u
        u = (lo + hi) / 2
      }
      return u
    }
    return {
      y: x => (x <= 0 ? 0 : x >= 1 ? 1 : at(y1, y2, solve(x))),
      dy: x => {
        const u = solve(Math.min(1, Math.max(0, x)))
        const dx = slope(x1, x2, u)
        return dx < 1e-9 ? 0 : slope(y1, y2, u) / dx
      },
    }
  }
  const cssEase = p => `cubic-bezier(${p.map(v => +v.toFixed(4)).join(', ')})`

  // A turning goes from a0° to a1° between t0 and t1 (ms) on its curve:
  // the ring's two turns, and two again from where it rests when another
  // comes while it is held still.
  //
  // The turning a ring is in at t: its last one begun by then.
  function turningAt(ring, t) {
    let seg = ring.turnings[0]
    for (const s of ring.turnings) if (s.t0 <= t) seg = s
    return seg
  }

  // Where a ring has turned to by t (°), and how fast it turns then (°/ms).
  function angleAt(ring, t) {
    if (ring.still) return 0
    const s = turningAt(ring, t)
    if (t <= s.t0) return s.a0
    if (t >= s.t1) return s.a1
    return s.a0 + (s.a1 - s.a0) * bezier(s.ease).y((t - s.t0) / (s.t1 - s.t0))
  }
  function speedAt(ring, t) {
    if (ring.still) return 0
    const s = turningAt(ring, t)
    if (t <= s.t0 || t >= s.t1) return 0
    return ((s.a1 - s.a0) / (s.t1 - s.t0)) * bezier(s.ease).dy((t - s.t0) / (s.t1 - s.t0))
  }

  // How lit it is at t: along its fade's points ([t, opacity], in time).
  function lightAt(ring, t) {
    const f = ring.fade
    if (t <= f[0][0]) return f[0][1]
    for (let i = 1; i < f.length; i++) {
      if (t <= f[i][0]) {
        const [ta, oa] = f[i - 1]
        const [tb, ob] = f[i]
        return tb === ta ? ob : oa + ((ob - oa) * (t - ta)) / (tb - ta)
      }
    }
    return f[f.length - 1][1]
  }

  // --- Held: the hold setting (zhoupengjie/Wakuwaku#65) ----------------------------------
  // A ring turned and faded in its 2.8 s whatever the hold said. Held (hold:
  // its ms, Infinity until seen), its turns are the same, but it is not let
  // fade as they end: lit, still, until the hold is over, counted from the
  // event (as main holds the ending itself, state.rs settle_at), then down
  // over 0.56 s, the dot coming as it does. Held still, nothing of it runs
  // (island.js holdRing): the gradient left where it stopped, the glow
  // under it the same. With reduced motion it is lit still the whole hold,
  // not 1.4 s. A ring's times then:
  //   stillAt  its turning over (the light up, with reduced motion): from
  //            then on it is held still, nothing animated
  //   fadeAt   its light going down (and the dot coming): the hold over
  //   until    gone
  // Without a hold (hold 0, as before it was one): stillAt null, its light
  // going down over its turning's end, fadeAt 0.31 s before it.
  //
  // Its light from the points given (coming up), lit after them until its
  // end: its turning's (t1), and with a hold, held still from there until
  // the hold after t is over.
  function timed(ring, t, up, t1) {
    if (!(ring.hold > 0)) {
      const fadeAt = t1 - FADE_LEAD_MS
      return { ...ring, fade: [...up, [fadeAt, 1], [t1 + TAIL_MS, 0]], stillAt: null, fadeAt, dotAt: fadeAt, until: t1 + TAIL_MS }
    }
    const fadeAt = Math.max(t1, t + ring.hold)
    return { ...ring, fade: [...up, [fadeAt, 1], [fadeAt + FADE_OUT_MS, 0]], stillAt: t1, fadeAt, dotAt: fadeAt, until: fadeAt + FADE_OUT_MS }
  }

  // The same with reduced motion: lit once the points given are, held
  // still from then until the hold after t is over, down over 0.2 s.
  function timedStill(ring, t, up) {
    const lit = up[up.length - 1][0]
    const fadeAt = Math.max(lit, t + ring.hold)
    return { ...ring, fade: [...up, [fadeAt, 1], [fadeAt + STILL_FADE_MS, 0]], stillAt: lit, fadeAt, dotAt: fadeAt, until: fadeAt + STILL_FADE_MS }
  }

  // A ring begun at t: two turns over 2 s, its light up over the first
  // 0.28 s; still (reduced motion), 1.4 s lit. Held (hold, ms), lit still
  // after its turns until the hold after t is over, then down, the dot
  // coming as it starts to; held by nothing, down over the turns' end.
  function begin(kind, t, still, hold = 0) {
    if (still) {
      if (hold > 0) return timedStill({ kind, still: true, t0: t, turns: 0, base: 0, hold, turnings: [] }, t, [[t, 0], [t + STILL_FADE_MS, 1]])
      const until = t + STILL_MS
      return { kind, still: true, t0: t, turns: 0, base: 0, hold: 0, turnings: [], fade: [[t, 0], [t + STILL_FADE_MS, 1], [until - STILL_FADE_MS, 1], [until, 0]], stillAt: null, fadeAt: until - STILL_FADE_MS, dotAt: until - STILL_FADE_MS, until }
    }
    const t1 = t + TURNS * TURN_MS
    const ring = { kind, still: false, t0: t, turns: TURNS, base: 0, hold, turnings: [{ t0: t, t1, a0: 0, a1: TURNS * 360, ease: EASE }] }
    return timed(ring, t, [[t, 0], [t + FADE_IN_MS, 1]], t1)
  }

  // No ring yet.
  const idle = () => ({ ring: null, until: -Infinity })

  // An event that rings, at t, on what plays already: what to do, and the
  // rings' state after. act is
  //   start   a new ring
  //   blend   the ring playing takes the new colour (BLEND_MS), its
  //           turning as it is (the user, 2026-10-18: no turning begun
  //           again while it turns); retimed when its hold is counted anew
  //           or it turns again (held still), carried on from where it is
  //   hold    a lower one: the ring keeps its colour, the dot takes the new
  //   hop     within AGAIN_MS of a ring's end: no ring, the dot takes the new
  //           colour and hops (with reduced motion only its colour: swap)
  // hold: how long it is held lit (ms; Infinity until seen; 0 none, as
  // before there was one). Held, each event that takes the ring counts the
  // hold anew from itself; one while it is held still (or fading after)
  // turns it again in its colour, two turns from where it rests; one while
  // it turns only gives it its colour.
  function play(state, kind, t, { still = false, hold = 0 } = {}) {
    const ring = state.ring && t < state.ring.until ? state.ring : null
    if (ring) {
      if ((PRIORITY[kind] || 0) < (PRIORITY[ring.kind] || 0)) return { state, act: 'hold', ring }
      const retimed = next => ({ state: { ring: next, until: next.until }, act: 'blend', retimed: true, ring: next })
      // Lit again over the blend, should it be fading (or coming up).
      const lit = lightAt(ring, t)
      const up = [[t, lit], ...(lit < 1 ? [[t + (ring.still ? STILL_FADE_MS : BLEND_MS), 1]] : [])]
      const held = { ...ring, kind, hold }
      if (ring.still) {
        if (hold > 0) return retimed(timedStill(held, t, up))
        const next = { ...ring, kind }
        return { state: { ...state, ring: next }, act: 'blend', retimed: false, ring: next }
      }
      const last = ring.turnings[ring.turnings.length - 1]
      // Turning yet (or, held by nothing, in its last fade): its colour, its
      // turning as it is; held, its hold from this one.
      if (t < last.t1 || !(hold > 0)) {
        if (hold > 0) return retimed(timed(held, t, up, last.t1))
        const next = { ...ring, kind }
        return { state: { ...state, ring: next }, act: 'blend', retimed: false, ring: next }
      }
      // Held still, or fading after: its two turns again, in the new
      // colour, from where it rests (its glow and its blend as ever).
      const a0 = last.a1
      const t1 = t + TURNS * TURN_MS
      return retimed(timed({ ...held, turns: TURNS, base: a0, turnings: [...ring.turnings, { t0: t, t1, a0, a1: a0 + TURNS * 360, ease: EASE }] }, t, up, t1))
    }
    if (t - state.until < AGAIN_MS) return { state: { ...state, ring: null }, act: still ? 'swap' : 'hop' }
    const next = begin(kind, t, still, hold)
    return { state: { ring: next, until: next.until }, act: 'start', ring: next }
  }

  // Cut short (the island opened: the person is looking). What comes next
  // rings anew: it was seen, not played out.
  const stop = () => idle()

  // Held, and what it says has moved on (its session at work again: island.js
  // movedOn): its hold over at t, down as at its time (after its turning,
  // should it still turn), the dot coming. Anything else as it is.
  function release(state, t) {
    const ring = state.ring && t < state.ring.until ? state.ring : null
    if (!ring || ring.stillAt == null || t >= ring.fadeAt) return state
    const fadeAt = Math.max(t, ring.stillAt)
    const until = fadeAt + (ring.still ? STILL_FADE_MS : FADE_OUT_MS)
    const next = { ...ring, fade: [...ring.fade.slice(0, -2), [fadeAt, 1], [until, 0]], fadeAt, dotAt: fadeAt, until }
    return { ring: next, until }
  }

  // A ring as the page animates it, at `now` (ms, the same clock): its
  // turning now (from, to, how long, its curve, begun `delay` ms from now:
  // less than 0 when it began before), and its light the same way, as
  // keyframes. So a ring drawn again carries on where it is.
  // Held (stillAt), the light only until it is held still; then
  //   still  ms from now until it is held still: nothing animated from then
  //   rest   its angle there (°, within a turn), the gradient left so
  //   out    its light going down, as keyframes begun delay ms from now
  //          (none while held until seen)
  //   steps  its turning in steps (stepsOf), from the one showing now
  // dotIn and end are Infinity while held until seen.
  function frames(ring, now) {
    const s = ring.still ? null : turningAt(ring, now)
    const turn = s && { from: s.a0, to: s.a1, delay: s.t0 - now, duration: s.t1 - s.t0, easing: cssEase(s.ease) }
    const steps = stepsOf(ring, now)
    const keys = (points, f0, fN) => points.map(([ti, o]) => ({ offset: fN > f0 ? (ti - f0) / (fN - f0) : 1, opacity: o }))
    const at = { dotIn: ring.dotAt - now, end: ring.until - now }
    if (ring.stillAt == null) {
      const f0 = ring.fade[0][0]
      const fN = ring.fade[ring.fade.length - 1][0]
      return { turn, steps, light: { keyframes: keys(ring.fade, f0, fN), delay: f0 - now, duration: fN - f0 }, still: null, rest: null, out: null, ...at }
    }
    const up = [...ring.fade.slice(0, -2).filter(([ti]) => ti < ring.stillAt), [ring.stillAt, 1]]
    const f0 = up[0][0]
    return {
      turn,
      steps,
      light: { keyframes: keys(up, f0, ring.stillAt), delay: f0 - now, duration: ring.stillAt - f0 },
      still: ring.stillAt - now,
      rest: ((angleAt(ring, ring.stillAt) % 360) + 360) % 360,
      out: Number.isFinite(ring.fadeAt)
        ? {
            keyframes: [
              { offset: 0, opacity: 1 },
              { offset: 1, opacity: 0 },
            ],
            delay: ring.fadeAt - now,
            duration: ring.until - ring.fadeAt,
          }
        : null,
      ...at,
    }
  }

  // Its turning as the page draws it, in steps of STEP_MS from the turning's
  // start (a grid a ring drawn again keeps): the one showing at `now`, then
  // each after it, the last at the turning's end, at its angle there.
  // { delay: ms from now (0 or less for the one showing), angle: ° }. None
  // once it has turned (or still, with reduced motion).
  function stepsOf(ring, now) {
    if (ring.still || !ring.turnings.length) return []
    const s = turningAt(ring, now)
    if (now >= s.t1) return []
    const n = Math.ceil((s.t1 - s.t0) / STEP_MS - 1e-9)
    const from = Math.max(0, Math.floor((now - s.t0) / STEP_MS + 1e-9))
    const out = []
    for (let k = from; k <= n; k++) {
      const at = Math.min(s.t1, s.t0 + k * STEP_MS)
      out.push({ delay: at - now, angle: at >= s.t1 ? s.a1 : angleAt(ring, at) })
    }
    return out
  }

  const api = {
    PRIORITY,
    TURN_MS,
    TURNS,
    SPIN_FPS,
    STEP_MS,
    EASE,
    BLEND_MS,
    AGAIN_MS,
    STILL_MS,
    FADE_OUT_MS,
    DOT_IN_MS,
    HOP_MS,
    CUED,
    isEnding,
    endCue,
    cueFor,
    kindOfReact,
    holdMs,
    keepFor,
    openFor,
    HOLD_MIN,
    HOLD_MAX,
    HOLD_QUICK,
    holdSecs,
    holdChoice,
    palette,
    gradient,
    glows,
    mix,
    bezier,
    idle,
    play,
    stop,
    release,
    angleAt,
    speedAt,
    stepsOf,
    lightAt,
    frames,
  }

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api
  } else {
    root.Flow = api
  }
})(this)
