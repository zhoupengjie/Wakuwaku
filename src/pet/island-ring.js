// The flow light (flow.js says how): what opened the island by itself, the
// ring turning round the closed capsule in the colour of what happened, held
// lit, faded, blended into the next; her reactions and the news dot.
// Made by island.js in the section's place as the page starts
// (ringPart(core), imported by island.js): what it takes of the island's core
// comes in core, its functions and constants as they are, its lets read
// and written through core.<name> as they change; what it hands back is the
// island's again.
import * as Flow from './flow.js'

export function ringPart(core) {
  const {
    ANCHORED, CLIPS, Flow, REACTIONS, animateHer, body, compact, flow, her, herTalks, hintAccent, home, hue,
    ink, isCapsule, isHinting, isHome, isNudging, isOn, isRinging, isSide, island, modeNow, newsDot, nudgeFor,
    setStow, spins, stillMotionNow, taskHead, update,
  } = core

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
  const flowBlocked = () => core.fullScreen || core.quiet || core.config.dnd === true || document.hidden || !!core.screenTop?.notch

  // Something happened: kind (waiting, done, review, error, hello, plugin,
  // notice, process), and what the island would have opened with ({ clip,
  // text, ms, widget }; a reaction's name, react).
  function cue(kind, show) {
    const how = Flow.cueFor(kind, { setting: Flow.endCue(core.config), view: core.view, blocked: flowBlocked(), herOut: herTalks() })
    // Inside a turn: her clip, and nothing kept, no ring, no dot.
    if (how === 'clip') return playReaction(show.react)
    // The newest wins: what is kept for the next look is this one's (an
    // ending's words are its session's: the open island has them), or none
    // when it shows now; a prompt leaves it for after.
    if (kind !== 'waiting') core.pending = how && how !== 'open' && how !== 'show' && !Flow.isEnding(kind) ? { ...show, until: Date.now() + Flow.keepFor(kind, core.config.hold) } : null
    // Opened by itself: as long as the hold says (it stayed its few seconds
    // whatever was chosen, zhoupengjie/Wakuwaku#65). Already open (show):
    // it is being looked at, its own few seconds.
    if (how === 'open' || how === 'show') return nudgeFor(show.clip, show.text, how === 'open' ? Flow.openFor(kind, core.config.hold, show.ms) : show.ms, show.widget, { kind, after: show.ms })
    if (!how || how === 'later') return update()
    core.news = { kind, colour: show.widget?.color, shown: core.news?.shown === true }
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
    core.reacting = { clip: r.clip, until: Date.now() + span }
    clearTimeout(core.reactTimer)
    core.reactTimer = setTimeout(() => {
      core.reacting = null
      core.herClip = null
      animateHer()
    }, span)
    core.herClip = null
    animateHer()
  }

  // The colours for a kind, on Windows' mode now: a plugin's from its own.
  const flowStops = kind => Flow.gradient(Flow.palette(kind, flowLight(), core.news?.colour, hintAccent()))

  function ring(kind) {
    // Held lit after its turns for as long as the hold says (until seen: until
    // the island opens), zhoupengjie/Wakuwaku#65.
    const step = Flow.play(core.flowState, kind, performance.now(), { still: stillMotionNow(), hold: Flow.holdMs(core.config.hold) })
    core.flowState = step.state
    if (step.act === 'start') {
      // A dot from before goes, and comes again in the new colour as it fades.
      core.news.shown = false
      for (const b of core.blends) b.anim.cancel()
      core.blends = []
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
      if (step.retimed && core.news?.shown && step.ring.hold > 0 && step.ring.dotAt > performance.now()) {
        core.news.shown = false
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
    for (const a of core.flowAnims) a.cancel()
    for (const t of core.flowTimers) clearTimeout(t)
    core.flowAnims = []
    core.flowTimers = []
    stopStepping()
    unholdRing()
    const f = Flow.frames(r, performance.now())
    const later = (fn, ms) => Number.isFinite(ms) && core.flowTimers.push(setTimeout(fn, Math.max(0, ms)))
    if (f.still == null || f.still > 0) {
      core.flowAnims.push(flow.animate(f.light.keyframes, { delay: f.light.delay, duration: f.light.duration, fill: 'both' }))
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
    for (const a of core.flowAnims) a.cancel()
    core.flowAnims = []
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
    for (const a of core.flowAnims) a.cancel()
    flow.classList.remove('held')
    flow.style.opacity = ''
    core.flowAnims = [flow.animate(o.keyframes, { delay: o.delay, duration: o.duration, fill: 'both' })]
  }

  // Into another kind's colours over BLEND_MS: the upper of each square's
  // two layers takes them and comes up, then the lower has them and the
  // upper goes again. One coming mid-way: the last finished at once.
  function blendTo(kind) {
    const stops = flowStops(kind)
    for (const b of core.blends) settle(b)
    core.blends = spins.map(s => {
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
    for (const b of core.blends) settle(b)
    for (const a of core.flowAnims) a.cancel()
    for (const t of core.flowTimers) clearTimeout(t)
    core.flowAnims = []
    core.blends = []
    core.flowTimers = []
    stopStepping()
    unholdRing()
    flow.classList.remove('on', 'glow')
    if (core.news && !core.news.shown) showNews('in')
    // Back up out of the way, if a window under it had it there.
    setStow()
  }

  // The island opened as it played: gone at once (it would grow with the
  // island), and the next rings anew.
  function stopRing() {
    core.flowState = Flow.stop(core.flowState)
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
    if (isHome()) return { x: parseFloat(her.style.left) || 0, y: parseFloat(her.style.top) || 0, px: parseFloat(her.style.width) || taskHead() }
    const still = compact.querySelector('.still-her')
    if (still) return { x: still.offsetLeft, y: still.offsetTop, px: still.offsetWidth }
    return { x: 9, y: 9, px: taskHead() }
  }

  // The dot over her portrait's top-right, as the hint's (placeHer): 2 px
  // out; 1 px in on an upright capsule, whose round top would cut it.
  function placeNews() {
    if (core.view !== 'compact') return
    const a = avatarBox()
    const [dx, dy] = isSide() ? [a.px - 10, 1] : [a.px - 7, -2]
    Object.assign(newsDot.style, { left: `${a.x + dx}px`, top: `${a.y + dy}px` })
  }

  // The dot's colour: a mood's as the island's words have it, a hello's and
  // a notice's Windows' accent, a plugin's its own (or the accent).
  function newsColour() {
    if (Flow.isEnding(core.news.kind)) return hue()[core.news.kind]
    if (core.news.kind === 'plugin') return ink(core.news.colour) || hintAccent()
    return hintAccent()
  }

  // It comes (in: grown from nothing, a little past full size and back),
  // hops in a new colour, or only takes it (swap, and with reduced motion).
  function showNews(how) {
    if (!core.news) return
    const was = core.news.shown
    core.news.shown = true
    drawNews()
    const cls = !was ? 'pop' : how === 'hop' ? 'hop' : ''
    if (!cls || stillMotionNow()) return
    newsDot.classList.remove('pop', 'hop')
    void newsDot.offsetWidth
    newsDot.classList.add(cls)
  }

  function drawNews() {
    const on = !!core.news?.shown && core.view === 'compact' && !isHinting()
    body.classList.toggle('has-news', on)
    if (on) newsDot.style.setProperty('--news', newsColour())
  }

  // Opened (by a hover, mostly) with something kept: it is what the island
  // shows, for as long as it would have by itself; gone if its time is up.
  function takePending() {
    if (!core.pending || isNudging()) return
    const p = core.pending
    core.pending = null
    if (Date.now() > p.until) return
    core.nudge = { until: Date.now() + p.ms, clip: p.clip, text: p.text, widget: p.widget }
    clearTimeout(core.nudgeTimer)
    core.nudgeTimer = setTimeout(() => {
      core.nudge = null
      update()
    }, p.ms)
  }

  // Now and then a blink, as a class for a moment (no running animation).
  function blink() {
    clearTimeout(core.blinkTimer)
    core.blinkTimer = setTimeout(() => {
      if (isOn() && !document.hidden) {
        island.classList.add('blink')
        setTimeout(() => island.classList.remove('blink'), 140)
      }
      blink()
    }, 3500 + Math.random() * 4000)
  }

  return { animateRing, blink, cue, drawNews, placeFlow, placeNews, stopRing, stopStepping, takePending }
}
