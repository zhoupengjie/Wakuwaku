const assert = require('node:assert/strict')
const { test } = require('node:test')

const Flow = require('../src/pet/flow')

const near = (a, b, within, what) => assert.ok(Math.abs(a - b) <= within, `${what}: ${a} not within ${within} of ${b}`)

test('only a session waiting on the person opens the island; the rest ring', () => {
  assert.equal(Flow.cueFor('waiting', {}), 'open')
  for (const kind of ['done', 'review', 'error', 'hello', 'plugin']) assert.equal(Flow.cueFor(kind, {}), 'ring', kind)
  // A new version, a crash report: never urgent, the dot alone.
  assert.equal(Flow.cueFor('notice', {}), 'dot')
})

test('the setting decides all that needs no answer: open, a ring, or just the dot', () => {
  for (const kind of ['done', 'review', 'error', 'hello', 'plugin']) {
    assert.equal(Flow.cueFor(kind, { setting: 'open' }), 'open', kind)
    assert.equal(Flow.cueFor(kind, { setting: 'ring' }), 'ring', kind)
    assert.equal(Flow.cueFor(kind, { setting: 'none' }), 'dot', kind)
  }
  // Waiting opens whatever it says.
  for (const setting of ['open', 'ring', 'none']) assert.equal(Flow.cueFor('waiting', { setting }), 'open')
  // Open is all as before: a notice and what happens inside a turn open it too.
  assert.equal(Flow.cueFor('notice', { setting: 'open' }), 'open')
  assert.equal(Flow.cueFor('process', { setting: 'open' }), 'open')
  assert.equal(Flow.cueFor('notice', { setting: 'none' }), 'dot')
})

test('out on the desktop the prompt is over her head: the island does not open for it', () => {
  for (const setting of ['open', 'ring', 'none']) assert.equal(Flow.cueFor('waiting', { setting, herOut: true }), null, setting)
  // The rest as ever.
  assert.equal(Flow.cueFor('done', { herOut: true }), 'ring')
  assert.equal(Flow.cueFor('done', { setting: 'open', herOut: true }), 'open')
})

test('inside a turn (a tool failed, an item done): her clip alone, never a ring or a dot', () => {
  for (const setting of ['ring', 'none']) {
    for (const view of ['compact', 'expanded', 'ask', 'settings']) assert.equal(Flow.cueFor('process', { setting, view }), 'clip', `${setting} ${view}`)
    assert.equal(Flow.cueFor('process', { setting, blocked: true }), 'clip')
  }
})

test('the setting from before it was a choice: openOnEnd off is nothing, else the ring', () => {
  assert.equal(Flow.endCue({}), 'ring')
  assert.equal(Flow.endCue({ openOnEnd: true }), 'ring')
  assert.equal(Flow.endCue({ openOnEnd: false }), 'none')
  assert.equal(Flow.endCue({ endCue: 'open', openOnEnd: false }), 'open')
  assert.equal(Flow.endCue({ endCue: 'spin' }), 'ring')
  assert.equal(Flow.endCue(null), 'ring')
})

test('full screen, nobody about, do not disturb: no ring, the dot alone', () => {
  for (const kind of ['done', 'review', 'error', 'hello', 'plugin']) assert.equal(Flow.cueFor(kind, { blocked: true }), 'dot', kind)
  // Waiting still opens; the setting's open still opens.
  assert.equal(Flow.cueFor('waiting', { blocked: true }), 'open')
  assert.equal(Flow.cueFor('done', { setting: 'open', blocked: true }), 'open')
})

test('the island open or the settings up: neither ring nor dot', () => {
  for (const kind of ['done', 'review', 'error']) {
    assert.equal(Flow.cueFor(kind, { view: 'expanded' }), null, kind)
    assert.equal(Flow.cueFor(kind, { view: 'settings' }), null, kind)
    assert.equal(Flow.cueFor(kind, { view: 'ask' }), null, kind)
  }
  // A hello or a plugin's words go into the open island; behind a prompt or
  // the settings, they wait for the next look.
  for (const kind of ['hello', 'plugin', 'notice']) {
    assert.equal(Flow.cueFor(kind, { view: 'expanded' }), 'show', kind)
    assert.equal(Flow.cueFor(kind, { view: 'settings' }), 'later', kind)
    assert.equal(Flow.cueFor(kind, { view: 'ask', blocked: true }), 'later', kind)
  }
})

test('of her reactions only a hello rings; a task done or a tool failed is inside the turn', () => {
  assert.equal(Flow.kindOfReact('wave'), 'hello')
  assert.equal(Flow.kindOfReact('jump'), 'process')
  assert.equal(Flow.kindOfReact('failed'), 'process')
  assert.equal(Flow.kindOfReact('other'), 'process')
})

test('what the island would have shown stays as long as an ending does; a notice until seen', () => {
  assert.equal(Flow.keepFor('hello', 8), 8000)
  assert.equal(Flow.keepFor('plugin', 120), 120000)
  assert.equal(Flow.keepFor('hello', 'seen'), Infinity)
  assert.equal(Flow.keepFor('notice', 8), Infinity)
  // A custom hold as long as it says (#59).
  assert.equal(Flow.keepFor('hello', 45), 45000)
  assert.equal(Flow.keepFor('plugin', 600), 600000)
})

test("Custom's seconds as typed: whole, within 3 to 600; no number is none (#59)", () => {
  assert.equal(Flow.holdSecs('45'), 45)
  assert.equal(Flow.holdSecs(' 45 '), 45)
  assert.equal(Flow.holdSecs('3'), 3)
  assert.equal(Flow.holdSecs('600'), 600)
  assert.equal(Flow.holdSecs('2'), 3)
  assert.equal(Flow.holdSecs('0'), 3)
  assert.equal(Flow.holdSecs('-8'), 3)
  assert.equal(Flow.holdSecs('601'), 600)
  assert.equal(Flow.holdSecs('99999'), 600)
  assert.equal(Flow.holdSecs('7.5'), 8)
  assert.equal(Flow.holdSecs('7.4'), 7)
  // An input method's full-width digits; a unit after them.
  assert.equal(Flow.holdSecs('４５'), 45)
  assert.equal(Flow.holdSecs('45秒'), 45)
  assert.equal(Flow.holdSecs('45 s'), 45)
  assert.equal(Flow.holdSecs('90 seconds'), 90)
  for (const none of ['', '   ', 'x', '4x5', '秒', 'Infinity', null, undefined]) assert.equal(Flow.holdSecs(none), null, String(none))
})

test('which hold choice is on: until seen, a quick one, or Custom for any other number (#59)', () => {
  assert.equal(Flow.holdChoice('seen'), 'seen')
  for (const secs of Flow.HOLD_QUICK) assert.equal(Flow.holdChoice(secs), secs)
  assert.deepEqual(Flow.HOLD_QUICK, [8, 30, 120])
  assert.equal(Flow.holdChoice(45), 'custom')
  assert.equal(Flow.holdChoice(3), 'custom')
  assert.equal(Flow.holdChoice(600), 'custom')
  // Custom picked, its box open before anything is typed: on, whatever is kept.
  assert.equal(Flow.holdChoice(8, true), 'custom')
  assert.equal(Flow.holdChoice('seen', true), 'custom')
  // No whole number is until seen, as main reads it (main.rs hold).
  for (const odd of [undefined, null, 'x', '8', 7.5, -8]) assert.equal(Flow.holdChoice(odd), 'seen', String(odd))
  assert.equal(Flow.HOLD_MIN, 3)
  assert.equal(Flow.HOLD_MAX, 600)
})

test('a ring: two turns over 2 s on its curve, lit 2.25 s, the dot as it fades', () => {
  const { act, ring } = Flow.play(Flow.idle(), 'done', 1000)
  assert.equal(act, 'start')
  assert.equal(ring.turns, 2)
  assert.equal(Flow.angleAt(ring, 1000), 0)
  assert.equal(Flow.angleAt(ring, 3000), 720)
  // Symmetric: half way round at half time; slow at the ends, fast between.
  near(Flow.angleAt(ring, 2000), 360, 0.5, 'half way')
  assert.ok(Flow.speedAt(ring, 1100) < Flow.speedAt(ring, 2000) / 3)
  assert.ok(Flow.speedAt(ring, 2900) < Flow.speedAt(ring, 2000) / 3)
  assert.deepEqual(
    [1000, 1280, 2690, 2970, 3250].map(t => Flow.lightAt(ring, t)),
    [0, 1, 1, 0.5, 0],
  )
  assert.deepEqual([ring.dotAt, ring.until], [2690, 3250])
  // As the page draws it: the turning begun, the light as keyframes.
  const f = Flow.frames(ring, 1500)
  assert.deepEqual([f.turn.from, f.turn.to, f.turn.delay, f.turn.duration, f.turn.easing], [0, 720, -500, 2000, 'cubic-bezier(0.3, 0, 0.7, 1)'])
  assert.deepEqual(
    f.light.keyframes.map(k => [+k.offset.toFixed(3), k.opacity]),
    [
      [0, 0],
      [0.124, 1],
      [0.751, 1],
      [1, 0],
    ],
  )
  assert.deepEqual([f.light.delay, f.light.duration, f.dotIn, f.end], [-500, 2250, 1190, 1750])
})

test('its turning in steps: 15 a second from its start, the last at its end, never back, no step past 35° (the user, 2026-10-18)', () => {
  const { ring } = Flow.play(Flow.idle(), 'done', 1000, { hold: 8000 })
  assert.equal(Flow.SPIN_FPS, 15)
  const steps = Flow.stepsOf(ring, 1000)
  assert.equal(steps.length, 31)
  steps.forEach((st, k) => near(st.delay, (k * 1000) / 15, 1e-9, `step ${k}`))
  assert.deepEqual([steps[0].angle, steps[30].angle, steps[30].delay], [0, 720, 2000])
  for (let k = 1; k < steps.length; k++) {
    const d = steps[k].angle - steps[k - 1].angle
    assert.ok(d >= 0 && d <= 35, `step ${k}: ${d}°`)
  }
  // Drawn again part way: the step showing now first, on the same grid.
  const mid = Flow.stepsOf(ring, 1500)
  assert.equal(mid.length, 24)
  near(mid[0].delay, (7 * 1000) / 15 - 500, 1e-9, 'the step showing now')
  near(mid[0].angle, steps[7].angle, 1e-9, 'its angle')
  assert.equal(Flow.frames(ring, 1500).steps.length, 24)
  // Turned: none; with reduced motion: none.
  assert.deepEqual(Flow.stepsOf(ring, 3000), [])
  assert.deepEqual(Flow.stepsOf(Flow.play(Flow.idle(), 'done', 0, { still: true }).ring, 0), [])
  // Turning again from where it rests (another while held still): its own grid.
  const again = Flow.play({ ring, until: ring.until }, 'error', 5000, { hold: 8000 }).ring
  const twice = Flow.stepsOf(again, 5000)
  assert.deepEqual([twice.length, twice[0].delay, twice[0].angle, twice[30].angle], [31, 0, 720, 1440])
})

test('another while it turns takes its colour, its turning as it is: no turns begun again (the user, 2026-10-18)', () => {
  const first = Flow.play(Flow.idle(), 'done', 0)
  for (const t of [600, 1800, 2000]) {
    const next = Flow.play(first.state, 'error', t)
    assert.deepEqual([next.act, next.retimed, next.ring.kind, next.ring.turns, next.ring.turnings.length, next.ring.until], ['blend', false, 'error', 2, 1, 2250], `at ${t}`)
  }
  // Held: its hold counted from it, its turning the same.
  const held = Flow.play(Flow.idle(), 'done', 0, { hold: 8000 })
  const next = Flow.play(held.state, 'error', 1500, { hold: 8000 })
  assert.deepEqual([next.act, next.retimed, next.ring.kind, next.ring.turnings.length, next.ring.stillAt, next.ring.fadeAt], ['blend', true, 'error', 1, 2000, 9500])
  near(Flow.angleAt(next.ring, 1500), Flow.angleAt(held.ring, 1500), 1e-9, 'its angle as it was')
  assert.deepEqual(Flow.stepsOf(next.ring, 1500), Flow.stepsOf(held.ring, 1500))
})

test('a lower one does not take the colour of a higher one playing; the dot has it', () => {
  const order = ['error', 'review', 'done', 'hello', 'plugin']
  for (let i = 0; i < order.length; i++) {
    const first = Flow.play(Flow.idle(), order[i], 0)
    for (let j = 0; j < order.length; j++) {
      const next = Flow.play(first.state, order[j], 500)
      assert.equal(next.act, j > i ? 'hold' : 'blend', `${order[j]} on ${order[i]}`)
      assert.equal(next.ring.kind, j > i ? order[i] : order[j])
    }
  }
})

test('within 8 s of a ring the next does not turn: the dot hops; after, a ring again', () => {
  const first = Flow.play(Flow.idle(), 'done', 0)
  const soon = Flow.play(first.state, 'hello', 2250 + 7999)
  assert.equal(soon.act, 'hop')
  // The 8 s are from the last ring's end, not from the hop.
  assert.equal(Flow.play(soon.state, 'error', 2250 + 7999.5).act, 'hop')
  assert.equal(Flow.play(soon.state, 'error', 2250 + 8000).act, 'start')
  // Cut short because the island opened: the next rings anew.
  assert.equal(Flow.play(Flow.stop(first.state), 'hello', 3000).act, 'start')
})

test('reduced motion: no turning, lit still 1.4 s, no glow, no hop', () => {
  const { act, ring } = Flow.play(Flow.idle(), 'done', 0, { still: true })
  assert.equal(act, 'start')
  assert.equal(ring.still, true)
  assert.equal(Flow.angleAt(ring, 700), 0)
  assert.equal(Flow.frames(ring, 0).turn, null)
  assert.deepEqual(
    [0, 200, 1200, 1300, 1400].map(t => Flow.lightAt(ring, t)),
    [0, 1, 1, 0.5, 0],
  )
  assert.deepEqual([ring.dotAt, ring.until], [1200, 1400])
  assert.equal(Flow.glows(false, true), false)
  assert.equal(Flow.glows(false, false), true)
  assert.equal(Flow.glows(true, false), false)
  // Another while lit: the colour only; soon after: the dot's colour only.
  const next = Flow.play({ ring, until: ring.until }, 'error', 600, { still: true })
  assert.deepEqual([next.act, next.retimed, next.ring.until], ['blend', false, 1400])
  assert.equal(Flow.play({ ring, until: ring.until }, 'error', 5000, { still: true }).act, 'swap')
})

test('the hold as a time: its seconds, or until seen for anything else, as main reads it (#65)', () => {
  assert.equal(Flow.holdMs(8), 8000)
  assert.equal(Flow.holdMs(3), 3000)
  assert.equal(Flow.holdMs(600), 600000)
  for (const seen of ['seen', undefined, null, '8', 7.5, 0, -8]) assert.equal(Flow.holdMs(seen), Infinity, String(seen))
})

test('open by itself, what endCue governs stays as long as the hold says; the rest their own time (#65)', () => {
  assert.deepEqual(Flow.CUED, ['done', 'review', 'error', 'hello', 'plugin'])
  for (const kind of Flow.CUED) {
    assert.equal(Flow.openFor(kind, 30, 3600), 30000, kind)
    assert.equal(Flow.openFor(kind, 8, 6500), 8000, kind)
    assert.equal(Flow.openFor(kind, 'seen', 3600), Infinity, kind)
  }
  // A prompt, a new version, a tool failed (set to open): as before.
  assert.equal(Flow.openFor('waiting', 30, 3600), 3600)
  assert.equal(Flow.openFor('notice', 'seen', 6500), 6500)
  assert.equal(Flow.openFor('process', 120, 3600), 3600)
})

test('held: the same two turns, then lit and still until the hold from the event is over, then down and the dot (#65)', () => {
  const { act, ring } = Flow.play(Flow.idle(), 'done', 1000, { hold: 8000 })
  assert.equal(act, 'start')
  assert.equal(ring.turns, 2)
  assert.equal(Flow.angleAt(ring, 3000), 720)
  assert.deepEqual([ring.stillAt, ring.fadeAt, ring.dotAt, ring.until], [3000, 9000, 9000, 9560])
  // Still: not turning, lit.
  assert.equal(Flow.angleAt(ring, 6000), 720)
  assert.equal(Flow.speedAt(ring, 6000), 0)
  assert.deepEqual(
    [1000, 1280, 3000, 6000, 9000, 9280, 9560].map(t => Flow.lightAt(ring, t)),
    [0, 1, 1, 1, 1, 0.5, 0],
  )
  // As the page draws it: its steps and its light coming up until it is
  // held still, nothing from then; down once the hold is over.
  const f = Flow.frames(ring, 1500)
  assert.deepEqual([f.turn.from, f.turn.to, f.turn.delay, f.turn.duration], [0, 720, -500, 2000])
  assert.deepEqual(
    f.light.keyframes.map(k => [+k.offset.toFixed(3), k.opacity]),
    [
      [0, 0],
      [0.14, 1],
      [1, 1],
    ],
  )
  assert.deepEqual([f.light.delay, f.light.duration, f.still, f.rest], [-500, 2000, 1500, 0])
  assert.deepEqual([f.out.delay, f.out.duration, f.dotIn, f.end], [7500, 560, 7500, 8060])
  assert.deepEqual(
    f.out.keyframes.map(k => [k.offset, k.opacity]),
    [
      [0, 1],
      [1, 0],
    ],
  )
  // Drawn again while held: held already, no steps, down later.
  const g = Flow.frames(ring, 6000)
  assert.deepEqual([g.still, g.steps.length, g.out.delay], [-3000, 0, 3000])
  // The least hold (3 s) still holds it a moment after its turns.
  const least = Flow.play(Flow.idle(), 'done', 0, { hold: 3000 }).ring
  assert.deepEqual([least.stillAt, least.fadeAt, least.until], [2000, 3000, 3560])
  // Until seen: lit and still with no end of its own (island.js: until the
  // island opens, or its session is at work again: release).
  const seen = Flow.play(Flow.idle(), 'error', 0, { hold: Infinity }).ring
  assert.deepEqual([seen.stillAt, seen.fadeAt, seen.until], [2000, Infinity, Infinity])
  assert.equal(Flow.lightAt(seen, 1e7), 1)
  const h = Flow.frames(seen, 5000)
  assert.deepEqual([h.out, h.dotIn, h.end], [null, Infinity, Infinity])
})

test('held still, another turns it again in its colour from where it rests, its hold from it; the rules after a ring as ever (#65)', () => {
  const hold = 30000
  const first = Flow.play(Flow.idle(), 'done', 0, { hold })
  const again = Flow.play(first.state, 'error', 10000, { hold })
  assert.deepEqual([again.act, again.retimed, again.ring.kind, again.ring.turns], ['blend', true, 'error', 2])
  // From where it rests, no jump, two turns on; lit all along.
  assert.equal(Flow.angleAt(again.ring, 10000), 720)
  assert.equal(Flow.angleAt(again.ring, 12000), 1440)
  assert.deepEqual([again.ring.stillAt, again.ring.fadeAt, again.ring.until], [12000, 40000, 40560])
  for (const t of [10000, 11000, 12000, 39000]) assert.equal(Flow.lightAt(again.ring, t), 1, `lit at ${t}`)
  assert.equal(Flow.frames(again.ring, 10000).rest, 0)
  // Turning again, the rules of a ring: one more only takes its colour, its
  // hold from it.
  const more = Flow.play(again.state, 'error', 11000, { hold })
  assert.deepEqual([more.ring.turnings.length, more.ring.stillAt, more.ring.fadeAt], [2, 12000, 41000])
  // A lower one keeps its colour (the dot has it), its hold as it was.
  const lower = Flow.play(again.state, 'hello', 20000, { hold })
  assert.deepEqual([lower.act, lower.ring.fadeAt], ['hold', 40000])
  // Come as it fades after its hold: lit again over 0.3 s, its turns again.
  const fading = Flow.play(first.state, 'review', 30280, { hold })
  assert.equal(Flow.lightAt(first.ring, 30280), 0.5)
  assert.equal(Flow.lightAt(fading.ring, 30280), 0.5)
  assert.equal(Flow.lightAt(fading.ring, 30580), 1)
  assert.deepEqual([fading.ring.turns, fading.ring.turnings.length, fading.ring.stillAt], [2, 2, 32280])
  // Gone: within 8 s the dot hops, as ever; after, a ring again.
  assert.equal(Flow.play(first.state, 'hello', 30560 + 7999, { hold }).act, 'hop')
  assert.equal(Flow.play(first.state, 'hello', 30560 + 8000, { hold }).act, 'start')
})

test('reduced motion, held: lit still for the whole hold, no 1.4 s, then down over 0.2 s (#65)', () => {
  const { ring } = Flow.play(Flow.idle(), 'done', 0, { still: true, hold: 30000 })
  assert.deepEqual(
    [0, 200, 1400, 15000, 30000, 30100, 30200].map(t => Flow.lightAt(ring, t)),
    [0, 1, 1, 1, 1, 0.5, 0],
  )
  assert.deepEqual([ring.stillAt, ring.fadeAt, ring.dotAt, ring.until], [200, 30000, 30000, 30200])
  const f = Flow.frames(ring, 0)
  assert.deepEqual([f.turn, f.still, f.rest, f.out.delay, f.out.duration], [null, 200, 0, 30000, 200])
  // Another while lit: its colour, the hold from it.
  const next = Flow.play({ ring, until: ring.until }, 'error', 10000, { still: true, hold: 30000 })
  assert.deepEqual([next.act, next.retimed, next.ring.kind, next.ring.fadeAt], ['blend', true, 'error', 40000])
})

test('held, and its session at work again: down now, as at its time, after its turns should it still turn (#65)', () => {
  const held = Flow.play(Flow.idle(), 'done', 0, { hold: Infinity }).state
  const now = Flow.release(held, 10000)
  assert.deepEqual([now.ring.fadeAt, now.ring.dotAt, now.ring.until, now.until], [10000, 10000, 10560, 10560])
  assert.equal(Flow.lightAt(now.ring, 9000), 1)
  assert.equal(Flow.lightAt(now.ring, 10280), 0.5)
  assert.deepEqual([Flow.frames(now.ring, 10000).still, Flow.frames(now.ring, 10000).out.delay], [-8000, 0])
  // Still turning: its turns played out first.
  assert.equal(Flow.release(held, 1000).ring.fadeAt, 2000)
  // Reduced motion: down over its 0.2 s.
  const still = Flow.play(Flow.idle(), 'done', 0, { still: true, hold: 30000 }).state
  assert.equal(Flow.release(still, 5000).ring.until, 5200)
  // Held by nothing, gone, going down already, or none: as it is.
  const plain = Flow.play(Flow.idle(), 'done', 0).state
  assert.equal(Flow.release(plain, 1000), plain)
  assert.equal(Flow.release(now, 20000), now)
  assert.equal(Flow.release(now, 10100), now)
  const none = Flow.idle()
  assert.equal(Flow.release(none, 0), none)
})

test('each event its own colours, the gradient closed; deeper on a light ground', () => {
  for (const kind of ['done', 'review', 'error', 'hello']) {
    for (const light of [false, true]) {
      const stops = Flow.palette(kind, light)
      assert.equal(stops[0], stops.at(-1), `${kind} ${light}`)
    }
  }
  assert.deepEqual(Flow.palette('error', false), ['#ff453a', '#ff9f0a', '#ff453a', '#ff9f0a', '#ff453a'])
  assert.equal(Flow.palette('done', true)[2], '#c99a00')
  // A plugin's: its colour and 45 % white by turns; deeper by 15 % on a light ground.
  assert.deepEqual(Flow.palette('plugin', false, '#ff0000'), ['#ff0000', '#ff7373', '#ff0000', '#ff7373', '#ff0000'])
  assert.deepEqual(Flow.palette('plugin', true, '#ff0000').slice(0, 2), ['#d90000', '#ea7373'])
  // None of its own: the fallback given.
  assert.equal(Flow.palette('plugin', false, undefined, '#4cc2ff')[0], '#4cc2ff')
  assert.equal(Flow.gradient(['#000', '#fff', '#000']), 'conic-gradient(#000, #fff, #000)')
})

test('the curve is CSS cubic-bezier', () => {
  const ease = Flow.bezier([0.25, 0.1, 0.25, 1])
  // CSS's ease at a few points (as browsers compute it).
  near(ease.y(0.5), 0.8024, 0.001, 'ease at 0.5')
  near(ease.y(0.25), 0.4085, 0.001, 'ease at 0.25')
  assert.equal(ease.y(0), 0)
  assert.equal(ease.y(1), 1)
  // A straight line is a straight line.
  const line = Flow.bezier([1 / 3, 1 / 3, 2 / 3, 2 / 3])
  near(line.y(0.37), 0.37, 1e-6, 'line')
  near(line.dy(0.6), 1, 1e-6, 'line slope')
})
