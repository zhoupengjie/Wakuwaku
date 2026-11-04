// Her playing (src/pet/play.js): where she can stand, how she gets there, and
// what she does when, on a pretend bridge. pet.rs has its own tests of the moves.
const assert = require('node:assert/strict')
const { test } = require('node:test')

const { cut, topsOf, spotUnder, nearestIn, goalOf, routeUp, fallMs, hopHigh, swinging, shaking, rubbing, create, attach, IGNORED_MS, PEEK_CUT, FLOAT_FROM, HOP_EVERY_MS, WAVE_AFTER_MS } = require('../src/pet/play')

// 2560 x 1335 of work area at 150%, a medium pet, as on the user's screen.
const CELL = [158, 172]
const screen = (over = {}) => ({ at: [1000, 400], size: [297, 264], cell: CELL, sf: 1.5, area: [0, 0, 2560, 1335], on: null, windows: [], ...over })
const win = (id, left, top, right, bottom, front = false) => ({ id, left, top, right, bottom, front })
const SOLE = 7
// What her dice come up as, always: up onto a window and sitting, up and
// peeking, down on the floor.
const SIT = 0.1
const PEEK = 0.6
const FLOOR = 0.8

test('spans lose what is cut out of them', () => {
  assert.deepEqual(cut([[0, 100]], 20, 30), [
    [0, 20],
    [30, 100],
  ])
  assert.deepEqual(cut([[0, 100]], -5, 30), [[30, 100]])
  assert.deepEqual(cut([[0, 100]], 50, 200), [[0, 50]])
  assert.deepEqual(cut([[0, 100]], 200, 300), [[0, 100]])
  assert.deepEqual(cut([[0, 100]], -5, 200), [])
})

test('a top she can stand on has room for her under the top of the screen', () => {
  const ids = (...windows) => topsOf(screen({ windows })).map(t => t.id)
  assert.deepEqual(ids(win(1, 300, 171, 900, 700)), [])
  assert.deepEqual(ids(win(1, 300, 172, 900, 700)), [1])
  // Maximized, its frame past the screen; and one all below the work area.
  assert.deepEqual(ids(win(1, -8, -8, 2568, 1343)), [])
  assert.deepEqual(ids(win(1, 300, 1335, 900, 1400)), [])
})

test('she keeps her whole width on the screen, along a top that runs off it', () => {
  const [top] = topsOf(screen({ windows: [win(1, -100, 500, 3000, 900)] }))
  assert.deepEqual(top.spans, [[79, 2560 - 79]])
})

test('a window in front takes away where she would stand, her half width either side', () => {
  const windows = [win(1, 600, 300, 900, 600), win(2, 300, 500, 1600, 1000)]
  const [front, behind] = topsOf(screen({ windows }))
  assert.deepEqual(front.spans, [[600, 900]])
  assert.deepEqual(behind.spans, [
    [300, 600 - 79],
    [900 + 79, 1600],
  ])
  // In front but all below that top, or above where she would stand: nothing taken.
  const below = topsOf(screen({ windows: [win(1, 600, 501, 900, 800), win(2, 300, 500, 1600, 1000)] }))
  assert.deepEqual(below[1].spans, [[300, 1600]])
  const above = topsOf(screen({ windows: [win(1, 600, 180, 900, 328), win(2, 300, 500, 1600, 1000)] }))
  assert.deepEqual(above[1].spans, [[300, 1600]])
  // All of it covered: no top to stand on.
  const over = topsOf(screen({ windows: [win(1, 200, 200, 1800, 1000), win(2, 300, 500, 1600, 1000)] }))
  assert.deepEqual(
    over.map(t => t.id),
    [1],
  )
})

test('what is under her feet: the highest top there, else the floor', () => {
  const windows = [win(1, 800, 700, 1400, 1100), win(2, 900, 900, 1500, 1300)]
  assert.deepEqual(spotUnder(screen({ at: [100, 400], windows })), { y: 1335, id: null })
  assert.deepEqual(spotUnder(screen({ at: [1000, 400], windows })), { y: 700 + SOLE, id: 1 })
  // Past the front one's side and its width: the one behind.
  assert.deepEqual(spotUnder(screen({ at: [1490, 400], windows })), { y: 900 + SOLE, id: 2 })
  // Already lower than the top: not back up to it; just there: on it.
  assert.deepEqual(spotUnder(screen({ at: [1000, 760], windows: [windows[0]] })), { y: 1335, id: null })
  assert.deepEqual(spotUnder(screen({ at: [1000, 710], windows: [windows[0]] })), { y: 700 + SOLE, id: 1 })
})

test('the nearest place in the spans', () => {
  const spans = [
    [100, 200],
    [400, 500],
  ]
  assert.equal(nearestIn(spans, 150), 150)
  assert.equal(nearestIn(spans, 250), 200)
  assert.equal(nearestIn(spans, 390), 400)
  assert.equal(nearestIn(spans, 900), 500)
})

test('she goes up onto the window in front if she can, else any she can', () => {
  const windows = [win(1, 1500, 300, 1900, 800, true), win(2, 300, 500, 1200, 1000)]
  assert.equal(goalOf(screen({ windows })).id, 1)
  // The one in front too high to stand on.
  const high = [win(1, 1500, 20, 1900, 800, true), win(2, 300, 500, 1200, 1000)]
  assert.equal(goalOf(screen({ windows: high }), () => 0).id, 2)
  assert.equal(goalOf(screen()), null)
  const goal = goalOf(screen({ at: [100, 1335], windows }))
  assert.deepEqual([goal.x, goal.y], [1500, 300 + SOLE])
})

test('close by, she jumps up onto it', () => {
  const s = screen({ at: [1000, 700], windows: [win(1, 900, 500, 1500, 1200, true)] })
  assert.deepEqual(
    routeUp(s, goalOf(s)).map(r => r.step),
    ['hop'],
  )
})

test('far from it, she runs along the floor and up the near side, her feet on it', () => {
  const windows = [win(1, 900, 400, 1500, 1335, true)]
  const s = screen({ at: [200, 1335], windows })
  const steps = routeUp(s, goalOf(s))
  assert.deepEqual(
    steps.map(r => r.step),
    ['run', 'up', 'over'],
  )
  // Her box on its side, her middle half her height from the wall.
  assert.deepEqual(steps[0].to, [900 - CELL[1] / 2, 1335])
  assert.equal(steps[1].wall, 'r')
  assert.deepEqual(steps[1].to, [900 - CELL[1] / 2, 400 + (CELL[1] - CELL[0]) / 2])
  assert.deepEqual(steps[2].to, [900 + CELL[0], 400 + SOLE])
  // From the other side, the other wall.
  const other = screen({ at: [2400, 1335], windows })
  const back = routeUp(other, goalOf(other))
  assert.equal(back[1].wall, 'l')
  assert.equal(back[0].to[0], 1500 + CELL[1] / 2)
  assert.equal(back[2].to[0], 1500 - CELL[0])
})

test('up high on another, she comes down first; a window hanging over the floor, she jumps to its foot', () => {
  const windows = [win(1, 900, 200, 1500, 700, true)]
  const s = screen({ at: [2200, 600], windows })
  const steps = routeUp(s, goalOf(s))
  assert.deepEqual(
    steps.map(r => r.step),
    ['fall', 'run', 'hop', 'up', 'over'],
  )
  assert.deepEqual(steps[0].to, [2200, 1335])
  assert.deepEqual(steps[2].to, [1500 + CELL[1] / 2, 700])
})

test('with no room beside a window as wide as the screen, she jumps straight up', () => {
  const s = screen({ at: [200, 1335], windows: [win(1, 0, 300, 2560, 1335, true)] })
  assert.deepEqual(
    routeUp(s, goalOf(s)).map(r => r.step),
    ['hop'],
  )
})

test('a fall takes longer from higher up, and a jump comes down on where it goes', () => {
  assert.ok(fallMs(1000, 1.5) > fallMs(100, 1.5))
  assert.equal(fallMs(1, 1.5), 120)
  // Coming down at the end of it: more than a quarter of the rise over the line.
  assert.ok(hopHigh(800, 100, 172) > 800 / 4)
  assert.equal(hopHigh(-800, 100, 172), 77)
})

test('lifted, she trails behind the way she is carried, and comes to rest swinging', () => {
  const swing = swinging()
  let hung
  // Carried right at 500 px/s: her feet behind, so turned clockwise.
  for (let i = 0; i < 60; i++) {
    swing.push(8)
    hung = swing.step(0.016)
  }
  assert.ok(hung.angle > 5, `leaning ${hung.angle}`)
  assert.ok(hung.stretch > 1)
  // Stopped: past upright and back, then still.
  let least = hung.angle
  for (let i = 0; i < 300; i++) {
    hung = swing.step(0.016)
    least = Math.min(least, hung.angle)
  }
  assert.ok(least < 0, 'swings on past upright')
  assert.equal(hung.stretch, 1)
  assert.ok(swing.settled())
})

test('moves coming in their own time, not one to a step, she does not shiver', () => {
  const swing = swinging()
  const hung = []
  // Carried at one pace, a move every other step, as the pointer's loop and
  // her frames fall out.
  for (let i = 0; i < 120; i++) {
    if (i % 2) swing.push(16)
    hung.push(swing.step(0.016))
  }
  const settled = hung.slice(60)
  for (let i = 1; i < settled.length; i++) {
    assert.ok(Math.abs(settled[i].stretch - settled[i - 1].stretch) < 0.005, `stretch ${settled[i - 1].stretch} then ${settled[i].stretch}`)
    assert.ok(Math.abs(settled[i].angle - settled[i - 1].angle) < 0.5, `angle ${settled[i - 1].angle} then ${settled[i].angle}`)
  }
  assert.ok(settled.at(-1).stretch > 1, 'still stretched by going fast')
})

test('however hard she is carried, she leans only so far', () => {
  const swing = swinging()
  for (let i = 0; i < 200; i++) {
    swing.push(i % 2 ? 900 : -900)
    assert.ok(Math.abs(swing.step(0.016).angle) <= 32)
  }
})

test('shaken: several turns of direction at speed, not long ago', () => {
  const shake = shaking()
  for (let i = 0; i < 6; i++) shake.add(i % 2 ? 20 : -20, i * 50)
  assert.equal(shake.shaken(300), false)
  shake.add(-20, 300)
  shake.add(20, 350)
  assert.equal(shake.shaken(350), true)
  assert.equal(shake.shaken(350 + 1400 + 400), false)
  // Slowly back and forth is not being shaken.
  const slow = shaking()
  for (let i = 0; i < 20; i++) slow.add(i % 2 ? 3 : -3, i * 50)
  assert.equal(slow.shaken(1000), false)
})

test('stroked: the pointer back and forth a few times, far enough each way', () => {
  const rub = rubbing()
  const xs = [0, 10, 0, 10, 0]
  const said = xs.map((x, i) => rub.add(x, i * 100))
  assert.deepEqual(said, [false, false, false, false, true])
  assert.equal(rub.started(400), true)
  rub.reset()
  assert.equal(rub.started(400), false)
  // Shaking a pixel or two, or passing by: no.
  const by = rubbing()
  assert.equal([0, 2, 0, 2, 0, 2, 0].some((x, i) => by.add(x, i * 100)), false)
  assert.equal([0, 40, 80, 120].some((x, i) => rubbing().add(x, i * 100)), false)
  // Too slowly.
  const slow = rubbing()
  assert.equal([0, 10, 0, 10, 0].some((x, i) => slow.add(x, i * 900)), false)
})

test("in her page, lifted by the bridge's words, she leans: the page's own calls made as a page makes them", t => {
  t.mock.timers.enable({ apis: ['setTimeout'] })
  const clock = { now: 0 }
  t.mock.method(performance, 'now', () => clock.now)
  const frames = []
  const node = () => ({ style: { setProperty() {} }, classList: { add() {} }, addEventListener() {}, append() {}, remove() {} })
  Object.assign(globalThis, {
    document: { hidden: false, addEventListener() {}, createElement: node },
    matchMedia: () => ({ matches: false }),
    // As the browser's: called on anything but the window, an illegal invocation.
    requestAnimationFrame(fn) {
      if (this !== undefined && this !== globalThis) throw new TypeError('Illegal invocation')
      frames.push(fn)
    },
  })
  t.after(() => {
    delete globalThis.document
    delete globalThis.matchMedia
    delete globalThis.requestAnimationFrame
  })
  const vars = new Map()
  const sprite = {
    classList: { toggle() {} },
    style: { setProperty: (name, value) => vars.set(name, value), removeProperty: name => vars.delete(name) },
    addEventListener() {},
    getBoundingClientRect: () => ({ top: 0, height: 100 }),
  }
  const heard = {}
  const on = name => fn => {
    heard[name] = fn
  }
  const pet = { onUpdate: on('update'), onDragStart: on('start'), onDrag: on('drag'), onDragEnd: on('end'), onLetGo: on('letGo'), onPerchGone: on('gone'), onFront: on('front'), onQuiet: on('quiet'), surfaces: async () => null, go: async () => true }
  const film = { pose() {}, react() {}, sound() {}, closeList() {} }
  attach({ pet, sprite, fx: { append() {} }, film, sheet: () => 2, sf: () => 1.5 })
  heard.update({ config: { play: true }, mood: 'idle' })
  heard.start()
  for (let i = 0; i < 40; i++) {
    clock.now += 10
    heard.drag(30)
    for (const fn of frames.splice(0)) fn()
  }
  assert.ok(parseFloat(vars.get('--her-tilt')) > 5, `leaning ${vars.get('--her-tilt')}`)
  assert.ok(parseFloat(vars.get('--her-sy')) > 1, 'stretched')
})

// --- What she does, on a pretend bridge ------------------------------------------------

const flush = async () => {
  for (let i = 0; i < 40; i++) await Promise.resolve()
}

function pretend(t, { s = screen(), config = { play: true }, random = () => 0.9, reduced = false, sheet = 2 } = {}) {
  t.mock.timers.enable({ apis: ['setTimeout'] })
  const world = { s, moves: [], poses: [], reacts: [], sounds: [], bits: [], sets: {}, umbrella: [], zzz: false, time: 0, hidden: false, arrives: true }
  world.machine = create({
    pet: {
      surfaces: async () => (world.s ? JSON.parse(JSON.stringify(world.s)) : null),
      go: async (x, y, ms, how, bend, on) => {
        world.moves.push({ x, y, ms, how, bend, on })
        if (!world.arrives) return false
        world.s.at = [x, y]
        world.s.on = on ?? null
        return true
      },
    },
    film: {
      pose: p => world.poses.push(p),
      react: (name, times) => world.reacts.push([name, times]),
      sound: name => world.sounds.push(name),
      closeList: () => {
        world.closed = true
      },
    },
    stage: {
      set: (name, on) => {
        world.sets[name] = on
      },
      tilt: (angle, stretch) => {
        world.tilt = { angle, stretch }
      },
      bits: name => world.bits.push(name),
      umbrella: on => world.umbrella.push(on),
      asleep: on => {
        world.zzz = on
      },
    },
    sheet: () => sheet,
    hidden: () => world.hidden,
    reduced: () => reduced,
    now: () => world.time,
    wait: async () => {},
    random,
    frame: fn => setTimeout(fn, 16),
  })
  world.tick = async ms => {
    world.time += ms
    t.mock.timers.tick(ms)
    await flush()
  }
  world.update = async (more = {}) => {
    world.machine.update({ mood: 'idle', quiet: false, config, ...more })
    await flush()
  }
  return world
}

// Up, carried a little, and let go.
async function carried(world, dxs = [10, 10, 10]) {
  world.machine.pressed()
  for (const dx of dxs) world.machine.dragged(dx, 1)
}

test('off, she does none of it, and stays where she is put', async t => {
  const w = pretend(t, { config: { play: false } })
  await w.update()
  await carried(w)
  w.machine.letGo({ moved: true, carried: false })
  await w.tick(IGNORED_MS * 2)
  assert.deepEqual(w.moves, [])
  assert.deepEqual(w.poses, [])
  assert.equal(w.sets.plays, false)
  assert.equal(w.machine.holding(), false)
})

test("with Windows' animations off, the same", async t => {
  const w = pretend(t, { reduced: true })
  await w.update()
  await carried(w)
  w.machine.letGo({ moved: true, carried: false })
  await flush()
  assert.deepEqual(w.moves, [])
})

test('turned on, she comes down to what is under her, with no fuss if she is on it', async t => {
  const w = pretend(t, { s: screen({ at: [1000, 1330] }) })
  await w.update()
  assert.equal(w.sets.plays, true)
  assert.deepEqual(w.moves, [{ x: 1000, y: 1335, ms: 0, how: 'line', bend: 0, on: null }])
  assert.deepEqual(w.bits, [])
  assert.deepEqual(w.sounds, [])
  assert.ok(!w.poses.some(p => p?.row === 4), 'no crouching about it')
})

test('lifted, she hangs and gasps, looking up at the hand; a mere press is not being lifted', async t => {
  const w = pretend(t, { s: screen({ at: [1000, 1335] }) })
  await w.update()
  w.machine.pressed()
  assert.ok(!w.sets.held)
  w.machine.dragged(10, 1)
  assert.equal(w.sets.held, true)
  assert.deepEqual(w.bits, ['gasp'])
  assert.deepEqual(w.sounds, ['pick'])
  assert.deepEqual(w.poses.at(-1), { row: 9, frame: 0, index: 0 })
  await w.tick(16)
  assert.ok(w.tilt.angle > 0)
  assert.equal(w.machine.holding(), true)
})

test('a sheet without the look rows hangs on its first frame', async t => {
  const w = pretend(t, { sheet: 1, s: screen({ at: [1000, 1335] }) })
  await w.update()
  await carried(w)
  assert.deepEqual(w.poses.at(-1), { row: 0, frame: 0 })
})

test('let go in the air she falls to the top under her, stands on it, and dust flies', async t => {
  const windows = [win(7, 800, 900, 1400, 1300)]
  const w = pretend(t, { s: screen({ at: [1000, 1335], windows }) })
  await w.update()
  w.moves.length = 0
  await carried(w)
  w.s.at = [1000, 400]
  w.machine.letGo({ moved: true, carried: false })
  await flush()
  assert.equal(w.moves.length, 1)
  const [fall] = w.moves
  assert.deepEqual([fall.x, fall.y, fall.how, fall.on], [1000, 900 + SOLE, 'fall', 7])
  assert.ok(fall.ms > 100)
  assert.deepEqual(w.bits, ['gasp', 'dust'])
  assert.deepEqual(w.sounds, ['pick', 'land'])
  assert.equal(w.poses.at(-1), null)
  assert.equal(w.sets.land, false)
  assert.equal(w.sets.held, false)
  assert.deepEqual(w.tilt, { angle: 0, stretch: 1 })
  assert.equal(w.machine.state().busy, false)
})

test('from high up, under her umbrella', async t => {
  const w = pretend(t, { s: screen({ at: [1000, 1335] }) })
  await w.update()
  w.moves.length = 0
  await carried(w)
  w.s.at = [1000, 1335 - CELL[1] * FLOAT_FROM]
  w.machine.letGo({ moved: true, carried: false })
  await flush()
  assert.equal(w.moves[0].how, 'float')
  assert.ok(w.moves[0].bend > 0)
  assert.ok(w.moves[0].ms > fallMs(CELL[1] * FLOAT_FROM, 1.5))
  assert.deepEqual(w.umbrella.filter(Boolean).length, 1)
  assert.equal(w.umbrella.at(-1), false)
})

test('shaken hard before she is let go, she is dizzy once down', async t => {
  const w = pretend(t, { s: screen({ at: [1000, 1335] }) })
  await w.update()
  await carried(w, [20, -20, 20, -20, 20, -20, 20, -20])
  w.machine.letGo({ moved: true, carried: false })
  await flush()
  assert.deepEqual(w.reacts, [['failed', 2]])
  assert.ok(w.bits.includes('stars'))
  assert.ok(w.sounds.includes('dizzy'))
})

test('stopped on the way (put away, say), she does not count as landed', async t => {
  const w = pretend(t, { s: screen({ at: [1000, 1335] }) })
  await w.update()
  await carried(w)
  w.s.at = [1000, 900]
  w.arrives = false
  w.machine.letGo({ moved: true, carried: false })
  await flush()
  assert.ok(!w.bits.includes('dust'))
  assert.equal(w.machine.state().busy, false)
})

test('no word of being let go: taken into her home, and no longer held', async t => {
  const w = pretend(t, { s: screen({ at: [1000, 1335] }) })
  await w.update()
  await carried(w)
  w.machine.dragEnd()
  await w.tick(600)
  assert.equal(w.machine.state().held, false)
  assert.equal(w.sets.held, false)
})

test('left alone long enough she runs up onto the window in front', async t => {
  const windows = [win(7, 900, 400, 1500, 1335, true)]
  const w = pretend(t, { s: screen({ at: [200, 1335], windows }), random: () => PEEK })
  await w.update()
  w.moves.length = 0
  await w.tick(IGNORED_MS - 1000)
  assert.deepEqual(w.moves, [])
  await w.tick(IGNORED_MS)
  assert.deepEqual(
    w.moves.map(m => m.how),
    ['line', 'line', 'arc', 'line'],
  )
  // Only the last step of the way leaves her on it, peeking over its top.
  assert.deepEqual(
    w.moves.map(m => m.on ?? null),
    [null, null, 7, 7],
  )
  assert.equal(w.machine.state().pose, 'peek')
  assert.equal(w.sets.peek, true)
  assert.equal(w.sets['wall-r'], false)
  assert.equal(w.moves.at(-1).y - w.moves.at(-2).y, Math.round(CELL[1] * PEEK_CUT) - SOLE)
  assert.equal(w.machine.holding(), true)
})

test('or sits on it', async t => {
  const windows = [win(7, 900, 400, 1500, 1335, true)]
  const w = pretend(t, { s: screen({ at: [200, 1335], windows }), random: () => SIT })
  await w.update()
  await w.tick(IGNORED_MS * 2)
  assert.equal(w.machine.state().pose, 'sit')
  assert.deepEqual(w.poses.at(-1), { row: 4, frame: 0 })
})

test('with no window to go up onto, she comes down and lies on the floor', async t => {
  const w = pretend(t, { s: screen({ at: [200, 900] }), random: () => 0.1 })
  await w.update()
  w.s.at = [200, 900]
  w.moves.length = 0
  await w.tick(IGNORED_MS * 2)
  assert.deepEqual(w.moves.map(m => [m.y, m.how]), [[1335, 'fall']])
  assert.equal(w.machine.state().pose, 'lie')
  assert.equal(w.sets['lie-l'], true)
})

test('one time in four she lies down though there is a window, right there on the floor', async t => {
  const w = pretend(t, { s: screen({ at: [200, 1335], windows: [win(7, 900, 400, 1500, 1335, true)] }), random: () => FLOOR })
  await w.update()
  w.moves.length = 0
  await w.tick(IGNORED_MS * 2)
  assert.deepEqual(w.moves, [])
  assert.equal(w.machine.state().pose, 'lie')
  assert.equal(w.sets['lie-r'], true)
})

test('attended to, she stays where she is a while longer', async t => {
  const w = pretend(t, { s: screen({ at: [200, 1335], windows: [win(7, 900, 400, 1500, 1335, true)] }), random: () => 0.1 })
  await w.update()
  w.moves.length = 0
  await w.tick(IGNORED_MS - 1000)
  w.machine.seen()
  await w.tick(IGNORED_MS - 1000)
  assert.deepEqual(w.moves, [])
  await w.tick(IGNORED_MS * 2)
  assert.ok(w.moves.length > 0)
})

test('while something else is on, nothing looks at the clock', async t => {
  const w = pretend(t, { s: screen({ at: [200, 1335], windows: [win(7, 900, 400, 1500, 1335, true)] }), random: () => 0.1 })
  await w.update()
  w.moves.length = 0
  w.machine.quietChanged(true)
  await w.tick(IGNORED_MS * 3)
  assert.deepEqual(w.moves, [])
  assert.equal(w.machine.state().asleep, true)
  // Back after a long while: hello, and she stays to be seen a while.
  w.machine.quietChanged(false)
  await w.tick(2000)
  assert.deepEqual(w.moves, [])
  await w.tick(IGNORED_MS * 2)
  assert.ok(w.moves.length > 0)
})

test('a short while with nobody about does not put off her going', async t => {
  const w = pretend(t, { s: screen({ at: [200, 1335], windows: [win(7, 900, 400, 1500, 1335, true)] }), random: () => SIT })
  await w.update()
  w.moves.length = 0
  await w.tick(IGNORED_MS - 60 * 1000)
  w.machine.quietChanged(true)
  await w.tick(90 * 1000)
  w.machine.quietChanged(false)
  await w.tick(IGNORED_MS / 2)
  assert.ok(w.moves.length > 0)
})

test('a click brings her up from behind the window', async t => {
  const windows = [win(7, 900, 400, 1500, 1335, true)]
  const w = pretend(t, { s: screen({ at: [200, 1335], windows }), random: () => PEEK })
  await w.update()
  await w.tick(IGNORED_MS * 2)
  assert.equal(w.machine.state().pose, 'peek')
  const y = w.s.at[1]
  w.machine.letGo({ moved: false, carried: false })
  await flush()
  assert.equal(w.machine.state().pose, 'stand')
  assert.equal(w.sets.peek, false)
  assert.equal(w.s.at[1], y - Math.round(CELL[1] * PEEK_CUT) + SOLE)
  assert.equal(w.s.on, 7)
})

test('a click brings a sitter to her feet, where she is', async t => {
  const windows = [win(7, 900, 400, 1500, 1335, true)]
  const sitter = pretend(t, { s: screen({ at: [200, 1335], windows }), random: () => SIT })
  await sitter.update()
  await sitter.tick(IGNORED_MS * 2)
  assert.equal(sitter.machine.state().pose, 'sit')
  sitter.moves.length = 0
  sitter.machine.letGo({ moved: false, carried: false })
  await flush()
  assert.equal(sitter.machine.state().pose, 'stand')
  assert.deepEqual(sitter.moves, [])
  assert.equal(sitter.poses.at(-1), null)
})

test('the window gone from under her: a gasp, and down she goes', async t => {
  const w = pretend(t, { s: screen({ at: [1000, 900 + SOLE], on: 7, windows: [win(7, 800, 900, 1400, 1300)] }) })
  await w.update()
  w.moves.length = 0
  w.s.windows = []
  w.machine.perchGone()
  await flush()
  assert.deepEqual([w.moves[0].y, w.moves[0].how], [1335, 'fall'])
  assert.ok(w.bits.includes('gasp') && w.bits.includes('dust'))
  assert.ok(w.sounds.includes('oops'))
})

test('another window to the front: sometimes she jumps onto it, not too often', async t => {
  const windows = [win(8, 1500, 500, 2000, 900, true), win(7, 300, 900, 1000, 1300)]
  const w = pretend(t, { s: screen({ at: [600, 900 + SOLE], on: 7, windows }), random: () => 0.1 })
  await w.update()
  w.moves.length = 0
  await w.machine.front()
  await flush()
  assert.deepEqual(w.moves, [], 'not at once after the last')
  w.time += HOP_EVERY_MS
  await w.machine.front()
  await flush()
  assert.deepEqual([w.moves[0].x, w.moves[0].y, w.moves[0].how, w.moves[0].on], [1500, 500 + SOLE, 'arc', 8])
  w.moves.length = 0
  w.time += 1000
  w.s.windows = [win(7, 300, 900, 1000, 1300, true), win(8, 1500, 500, 2000, 900)]
  await w.machine.front()
  await flush()
  assert.deepEqual(w.moves, [], 'not again so soon')
})

test('coin says no: she stays', async t => {
  const windows = [win(8, 1500, 500, 2000, 900, true), win(7, 300, 900, 1000, 1300)]
  const w = pretend(t, { s: screen({ at: [600, 900 + SOLE], on: 7, windows }), random: () => 0.9 })
  await w.update()
  w.moves.length = 0
  w.time += HOP_EVERY_MS
  await w.machine.front()
  await flush()
  assert.deepEqual(w.moves, [])
})

test('the window in front now covers where she stands: down she goes', async t => {
  const windows = [win(8, 300, 700, 1000, 1200, true), win(7, 300, 900, 1000, 1300)]
  const w = pretend(t, { s: screen({ at: [600, 900 + SOLE], on: 7, windows }), random: () => 0.9 })
  await w.update()
  w.moves.length = 0
  w.s.at = [600, 900 + SOLE]
  w.s.on = 7
  await w.machine.front()
  await flush()
  assert.equal(w.moves[0].how, 'fall')
  assert.equal(w.moves[0].y, 1335)
})

test('stroked on the head: hearts and a wave, the list under the pointer shut; not on her skirt', async t => {
  const w = pretend(t, { s: screen({ at: [1000, 1335] }) })
  await w.update()
  w.time = 1000
  for (const x of [0, 10, 0, 10, 0]) {
    w.time += 100
    w.machine.stroked(x, false)
  }
  assert.deepEqual(w.bits, [])
  for (const x of [0, 10, 0, 10, 0]) {
    w.time += 100
    w.machine.stroked(x, true)
  }
  assert.deepEqual(w.bits, ['hearts'])
  assert.deepEqual(w.reacts, [['wave', 1]])
  assert.ok(w.sounds.includes('heart'))
  assert.equal(w.closed, true)
})

test('nobody about she sleeps, nothing moving; back soon she only wakes, back late she waves', async t => {
  const w = pretend(t, { s: screen({ at: [1000, 1335] }) })
  await w.update()
  w.machine.quietChanged(true)
  assert.equal(w.zzz, true)
  assert.equal(w.machine.state().asleep, true)
  assert.deepEqual(w.poses.at(-1), { row: 4, frame: 0 })
  assert.equal(w.machine.holding(), true)
  w.time += 60 * 1000
  w.machine.quietChanged(false)
  assert.equal(w.zzz, false)
  assert.deepEqual(w.reacts, [])
  assert.equal(w.poses.at(-1), null)
  w.machine.quietChanged(true)
  w.time += WAVE_AFTER_MS
  w.machine.quietChanged(false)
  assert.deepEqual(w.reacts, [['wave', 1]])
  assert.ok(w.sounds.includes('hello'))
})

test('a session wanting her: awake and on her feet, and no sleeping through it', async t => {
  const windows = [win(7, 900, 400, 1500, 1335, true)]
  const w = pretend(t, { s: screen({ at: [200, 1335], windows }), random: () => SIT })
  await w.update()
  await w.tick(IGNORED_MS * 2)
  assert.equal(w.machine.state().pose, 'sit')
  w.machine.quietChanged(true)
  await w.update({ mood: 'working', quiet: true })
  assert.equal(w.machine.state().pose, 'stand')
  assert.equal(w.machine.state().asleep, false)
  assert.equal(w.zzz, false)
  w.machine.quietChanged(true)
  assert.equal(w.machine.state().asleep, false)
})

test('turned off while she sits, she stands, and rides on no window', async t => {
  const windows = [win(7, 900, 400, 1500, 1335, true)]
  const w = pretend(t, { s: screen({ at: [200, 1335], windows }), random: () => SIT })
  await w.update()
  await w.tick(IGNORED_MS * 2)
  assert.equal(w.s.on, 7)
  await w.update({ config: { play: false } })
  assert.equal(w.machine.state().pose, 'stand')
  assert.equal(w.sets.plays, false)
  assert.equal(w.s.on, null)
  assert.equal(w.machine.holding(), false)
})

test('an idle walk off the end of a window: down she goes; still on it: she stays on it', async t => {
  const windows = [win(7, 800, 900, 1400, 1300)]
  const w = pretend(t, { s: screen({ at: [1000, 900 + SOLE], windows }) })
  await w.update()
  w.moves.length = 0
  w.s.at = [1100, 900 + SOLE]
  w.machine.walked()
  await flush()
  assert.deepEqual(w.moves.map(m => [m.how, m.ms, m.on]), [['line', 0, 7]])
  w.moves.length = 0
  w.s.at = [1500, 900 + SOLE]
  w.machine.walked()
  await flush()
  assert.deepEqual(w.moves.map(m => [m.how, m.y, m.on]), [['fall', 1335, null]])
})

test('put away while lifted, she is not lifted any more; shown again on her window, she stays', async t => {
  const windows = [win(7, 800, 900, 1400, 1300)]
  const w = pretend(t, { s: screen({ at: [1000, 900 + SOLE], windows }) })
  await w.update()
  await carried(w)
  w.hidden = true
  await w.machine.hiddenChanged(true)
  assert.equal(w.machine.state().held, false)
  w.moves.length = 0
  w.s.on = 7
  w.hidden = false
  await w.machine.hiddenChanged(false)
  assert.deepEqual(w.moves, [])
  // Out of her home, on nothing: down.
  w.hidden = true
  await w.machine.hiddenChanged(true)
  w.s.on = null
  w.s.at = [100, 900]
  w.hidden = false
  await w.machine.hiddenChanged(false)
  await flush()
  assert.deepEqual([w.moves[0].y, w.moves[0].how], [1335, 'fall'])
})
