// Her playing (the play setting, off unless turned on): what she does with
// her own body beyond the moods' clips. Lifted, she swings from the pointer,
// and shaken she is dizzy after. Let go, she drops to what is under her: the
// top of a window or the floor (the foot of the work area), under a little
// umbrella from high up. Left alone a good while, she runs up the side of a
// window and sits on it or peeks over its top, or lies down on the floor;
// the window moved, she goes with it (pet.rs poll); gone from under her, she
// falls. Stroked on the head, hearts. Nobody about, she sleeps.
//
// The art has no sitting or lying down, so those are made of what every pet's
// sheet has: the crouch that starts a jump, her box turned on its side, her
// lower half cut off behind the top of a window. Nothing here repeats for good
// and nothing moves while nobody is about (docs/design/style.md); with
// Windows' animations off she does none of it.
//
// What is decided is here, the moves themselves in pet.rs (physical pixels,
// as its surfaces are given); pet.js draws what it is told. Loaded by the page
// before pet.js, and required by the tests.
;(function (root) {
  const { lookCell } = root.Sprite || require('./sprite')

  // Her soles lie this far up her cell (of its height): on a surface, the
  // foot of her window is this far under it.
  const SOLE = 8 / 208
  // Peeking, so much of her is cut off behind the top of the window.
  const PEEK_CUT = 0.46
  // A surface this close to her feet (logical px): she is on it.
  const ON_IT = 8
  // Physics, in logical px and seconds.
  const FALL = 2600
  const FLOAT_SPEED = 170
  const SWAY = 14
  const RUN_SPEED = 260
  const UP_SPEED = 340
  // From this many times her height up, she opens her umbrella.
  const FLOAT_FROM = 3
  // A jump this far (her height up, her width across) she just makes.
  const HOP_HIGH = 1.6
  const HOP_FAR = 5

  const IGNORED_MS = 5 * 60 * 1000
  // Three times out of four up onto a window, if there is one; else the floor.
  const UP_CHANCE = 0.75
  const HOP_EVERY_MS = 2 * 60 * 1000
  const HOP_CHANCE = 0.5
  // Asleep longer than this, she waves as she wakes.
  const WAVE_AFTER_MS = 5 * 60 * 1000
  const PAT_EVERY_MS = 1500
  const HANG_MS = 110

  // Frames of the jump's row, and the one looking straight up (at the hand).
  const JUMP = 4
  const FRAME = {
    crouch: { row: JUMP, frame: 0 },
    air: { row: JUMP, frame: 2 },
    down: { row: JUMP, frame: 3 },
    landed: { row: JUMP, frame: 4 },
    still: { row: 0, frame: 0 },
    up: lookCell(0, -1),
  }

  // --- Where she can stand --------------------------------------------------------

  // Spans (from, to) less what lies between from and to.
  function cut(spans, from, to) {
    const out = []
    for (const [a, b] of spans) {
      if (to <= a || from >= b) {
        out.push([a, b])
        continue
      }
      if (from > a) out.push([a, from])
      if (to < b) out.push([to, b])
    }
    return out
  }

  // The tops of windows she can stand on (pet.rs surfaces: its windows are
  // front to back): each with the spans of it her middle can be over. One too
  // high has no room for her under the top of the screen (a maximized one
  // never does), and a window in front takes away what it reaches into of
  // where she would stand, her width either side.
  function topsOf({ windows, cell: [cw, ch], area: [ax, ay, aw, ah] }) {
    const out = []
    windows.forEach((win, i) => {
      if (win.top < ay + ch || win.top >= ay + ah) return
      let spans = [[Math.max(win.left, ax + cw / 2), Math.min(win.right, ax + aw - cw / 2)]]
      for (const over of windows.slice(0, i)) {
        if (over.bottom > win.top - ch && over.top < win.top) spans = cut(spans, over.left - cw / 2, over.right + cw / 2)
      }
      spans = spans.filter(([a, b]) => a <= b)
      if (spans.length) out.push({ ...win, spans })
    })
    return out
  }

  const soleOf = ([, ch]) => Math.round(ch * SOLE)
  const floorOf = ([, ay, , ah]) => ay + ah
  const isOver = (x, spans) => spans.some(([a, b]) => x >= a && x <= b)

  // What is under her feet, the highest there: where her feet go, and the
  // window it is (null: the floor).
  function spotUnder(s) {
    const [x, y] = s.at
    let spot = { y: floorOf(s.area), id: null }
    for (const t of topsOf(s)) {
      const feet = t.top + soleOf(s.cell)
      if (feet < y - ON_IT * s.sf || feet >= spot.y || !isOver(x, t.spans)) continue
      spot = { y: feet, id: t.id }
    }
    return spot
  }

  // The nearest place to x in these spans.
  function nearestIn(spans, x) {
    let best = null
    for (const [a, b] of spans) {
      const at = Math.min(b, Math.max(a, x))
      if (best === null || Math.abs(at - x) < Math.abs(best - x)) best = at
    }
    return best
  }

  // The window she goes up onto: the one in front if she can, else any she
  // can. None: none to go up onto.
  function goalOf(s, random = Math.random) {
    const tops = topsOf(s)
    const top = tops.find(t => t.front) || tops[Math.floor(random() * tops.length)]
    if (!top) return null
    return { id: top.id, y: top.top + soleOf(s.cell), x: nearestIn(top.spans, s.at[0]), top }
  }

  // The steps of the way up onto it: a jump if it is close; else down to the
  // floor if she is not on it, along it to the near side of the window (the
  // side of the screen leaving room to run up), up that side, a jump to it
  // first if the window hangs high over the floor, and over the top. Her box
  // on its side, her feet are half her height from her middle: so far from
  // the window's side her window's middle runs up it.
  function routeUp(s, goal) {
    const [x, y] = s.at
    const [cw, ch] = s.cell
    const [ax, , aw] = s.area
    const floor = floorOf(s.area)
    const hop = [{ step: 'hop', to: [goal.x, goal.y] }]
    if (y - goal.y <= ch * HOP_HIGH && Math.abs(goal.x - x) <= cw * HOP_FAR) return hop
    const half = s.size[0] / 2
    const walls = [
      { wall: 'r', x: Math.round(goal.top.left - ch / 2), over: goal.top.left + cw },
      { wall: 'l', x: Math.round(goal.top.right + ch / 2), over: goal.top.right - cw },
    ].filter(w => w.x - half >= ax && w.x + half <= ax + aw)
    if (!walls.length) return hop
    const wall = walls.reduce((a, b) => (Math.abs(b.x - x) < Math.abs(a.x - x) ? b : a))
    const steps = []
    if (y < floor - ON_IT * s.sf) steps.push({ step: 'fall', to: [x, floor] })
    steps.push({ step: 'run', to: [wall.x, floor] })
    if (goal.top.bottom < floor - ch) steps.push({ step: 'hop', to: [wall.x, goal.top.bottom] })
    steps.push({ step: 'up', wall: wall.wall, to: [wall.x, goal.top.top + Math.round((ch - cw) / 2)] })
    steps.push({ step: 'over', to: [nearestIn(goal.top.spans, wall.over), goal.y] })
    return steps
  }

  const fallMs = (drop, sf) => Math.max(120, Math.round(Math.sqrt((2 * drop) / (FALL * sf)) * 1000))
  const hopMs = (far, sf) => Math.round(300 + 12 * Math.sqrt(far / sf))
  // High enough to come down onto where she is going.
  const hopHigh = (rise, far, ch) => Math.round(Math.max(ch * 0.45, rise * 0.35 + far * 0.08))

  // --- Lifted ------------------------------------------------------------------------

  // How she hangs from the pointer: a spring towards trailing behind the way
  // she is carried, so she leans back going fast and swings on as it stops.
  // Degrees, turning clockwise (her feet to the left, the pointer going right).
  const TILT_MOST = 32
  const TILT_PER_SPEED = 0.03
  const STRETCH_MOST = 0.06
  const STRETCH_PER_SPEED = 0.00008
  const SPRING = 90
  const DAMP = 7
  // Her pace is smoothed, twice, over about this long (s). Moves come in their
  // own time, not one to a step: taken as they came, it was nothing every
  // other step and double the next, and she shivered between stretched and
  // not 30 times a second, which reads as two of her, one over the other.
  const PACE_OVER = 0.07
  // No more steps than this far apart (ms), whatever the display's rate.
  const SWING_STEP_MS = 8

  function swinging() {
    let angle = 0
    let speed = 0
    let moved = 0
    let rough = 0
    let pace = 0
    return {
      // Moved this far (logical px) since the last step.
      push(dx) {
        moved += dx
      },
      // A step of dt seconds; how she hangs now.
      step(dt) {
        const k = Math.min(1, dt / PACE_OVER)
        rough += (moved / dt - rough) * k
        pace += (rough - pace) * k
        if (Math.abs(pace) < 1 && Math.abs(rough) < 1) rough = pace = 0
        moved = 0
        const want = Math.max(-TILT_MOST, Math.min(TILT_MOST, pace * TILT_PER_SPEED))
        speed += (SPRING * (want - angle) - DAMP * speed) * dt
        angle = Math.max(-TILT_MOST, Math.min(TILT_MOST, angle + speed * dt))
        return { angle, stretch: 1 + Math.min(STRETCH_MOST, Math.abs(pace) * STRETCH_PER_SPEED) }
      },
      settled: () => Math.abs(angle) < 0.3 && Math.abs(speed) < 2 && pace === 0,
    }
  }

  // Shaken hard: this many turns of direction, each at this speed (logical
  // px a move) or more, within this long.
  const SHAKE_TURNS = 6
  const SHAKE_MOVE = 5
  const SHAKE_WITHIN_MS = 1400

  function shaking() {
    let dir = 0
    let turns = []
    return {
      add(dx, at) {
        if (Math.abs(dx) < SHAKE_MOVE) return
        const d = Math.sign(dx)
        if (dir && d !== dir) turns.push(at)
        dir = d
        turns = turns.filter(t => at - t <= SHAKE_WITHIN_MS)
      },
      shaken: at => turns.filter(t => at - t <= SHAKE_WITHIN_MS).length >= SHAKE_TURNS,
    }
  }

  // Stroked: the pointer turning back and forth over her this many times,
  // going this far (page px) each way, within this long.
  const RUB_TURNS = 3
  const RUB_FAR = 6
  const RUB_WITHIN_MS = 1100

  function rubbing() {
    let last = null
    let dir = 0
    let turns = []
    return {
      // The pointer at x: true once it has been stroking her.
      add(x, at) {
        if (last === null) last = x
        const far = x - last
        if (Math.abs(far) >= RUB_FAR) {
          const d = Math.sign(far)
          if (dir && d !== dir) turns.push(at)
          dir = d
          last = x
        }
        turns = turns.filter(t => at - t <= RUB_WITHIN_MS)
        return turns.length >= RUB_TURNS
      },
      // On the way to it, for the list that would open under the pointer (pet.js).
      started: at => turns.some(t => at - t <= RUB_WITHIN_MS),
      reset() {
        turns = []
      },
    }
  }

  // --- What she does -----------------------------------------------------------------

  // d: the bridge's pet (surfaces, go, walkStop); film: what pet.js draws
  // (pose, react, sound, closeList); stage: her box and what is round it (set,
  // tilt, bits, umbrella, asleep); hidden, reduced, sheet (2: with the look
  // rows); now, wait, random, frame (the next frame of the display).
  function create(d) {
    const { pet, film, stage, now, wait, random } = d
    let config = {}
    let mood = 'idle'
    let quiet = false
    // A newer doing stops the one before: each looks whether it is still the one.
    let doing = 0
    let busy = false
    // A press on her (she is not lifted until she moves), and lifted.
    let pressed = false
    let held = false
    // stand, sit, peek or lie.
    let pose = 'stand'
    let asleep = false
    let sleptAt = 0
    let settled = false
    let seenAt = now()
    let hoppedAt = now()
    let pattedAt = -Infinity
    let planned = null
    let swung = null
    let unsaid = null
    let lastStep = 0
    let shake = shaking()
    const rub = rubbing()

    const on = () => config.play === true && !d.reduced()

    const seen = () => {
      seenAt = now()
    }

    async function act(fn) {
      const me = ++doing
      const alive = () => me === doing
      busy = true
      try {
        await fn(alive)
      } finally {
        if (alive()) {
          busy = false
          rested()
        }
      }
    }

    // Over whatever she was doing, her body as it is at rest.
    function plain() {
      pose = 'stand'
      for (const name of ['peek', 'lie-l', 'lie-r', 'wall-l', 'wall-r', 'land']) stage.set(name, false)
      stage.umbrella(false)
      film.pose(null)
    }

    function stop() {
      doing++
      busy = false
      plain()
      wake(false)
    }

    // A doing done: asleep if nobody is about, and the clock for the next.
    function rested() {
      if (on() && quiet && mood === 'idle' && !held) sleep()
      plan()
    }

    // --- Lifted and let go

    function stepSwing() {
      if (!swung) return
      const at = now()
      if (at - lastStep >= SWING_STEP_MS) {
        const dt = Math.min(0.05, (at - lastStep) / 1000)
        lastStep = at
        const { angle, stretch } = swung.step(dt)
        stage.tilt(angle, stretch)
        if (!held && swung.settled()) return endSwing()
      }
      d.frame(stepSwing)
    }

    function endSwing() {
      swung = null
      stage.tilt(0, 1)
      stage.set('held', false)
    }

    function liftUp() {
      stop()
      held = true
      seen()
      shake = shaking()
      stage.set('held', true)
      stage.bits('gasp')
      film.sound('pick')
      film.pose(d.sheet() === 2 ? FRAME.up : FRAME.still)
      swung = swinging()
      lastStep = now()
      stepSwing()
    }

    function dragged(dx, sf = 1) {
      if (!on() || !pressed) return
      if (!held) liftUp()
      swung?.push(dx / sf)
      shake.add(dx / sf, now())
    }

    function pressedOn() {
      clearTimeout(unsaid)
      pressed = true
    }

    function letGo({ moved, carried }) {
      clearTimeout(unsaid)
      pressed = false
      if (!on()) return
      seen()
      if (!moved && !carried) return clicked()
      const dizzy = held && shake.shaken(now())
      held = false
      land({ dizzy })
    }

    // No word of her being let go: she was taken into her home (pet.rs let_go).
    function dragEnd() {
      pressed = false
      clearTimeout(unsaid)
      unsaid = setTimeout(() => {
        if (!held) return
        held = false
        endSwing()
        stop()
      }, 500)
    }

    // A click: Rust has her jump. Out of whatever she was resting in.
    function clicked() {
      if (asleep) wake(false)
      leavePose()
    }

    function leavePose() {
      if (pose === 'peek') return popUp()
      if (pose !== 'stand') plain()
      plan()
    }

    // Up from behind the top of the window, onto it.
    function popUp() {
      return act(async alive => {
        stage.set('peek', false)
        pose = 'stand'
        const s = await pet.surfaces()
        if (!alive() || !s) return
        await pet.go(s.at[0], s.at[1] - Math.round(s.cell[1] * PEEK_CUT) + soleOf(s.cell), 220, 'line', 0, on() ? s.on : null)
      })
    }

    // Down to what is under her, however high up she is.
    function land({ dizzy = false, surprised = false, still = false } = {}) {
      return act(async alive => {
        if (!still) {
          film.pose(FRAME.down)
          await wait(surprised ? 2 * HANG_MS : HANG_MS)
          if (!alive()) return
        }
        const s = await pet.surfaces()
        if (!alive()) return
        if (!s) return film.pose(null)
        const spot = spotUnder(s)
        const [x, y] = s.at
        const drop = spot.y - y
        let arrived
        if (drop <= ON_IT * s.sf) {
          arrived = await pet.go(x, spot.y, 0, 'line', 0, spot.id)
        } else if (drop >= FLOAT_FROM * s.cell[1]) {
          stage.umbrella(true)
          film.pose(FRAME.still)
          arrived = await pet.go(x, spot.y, Math.round((drop / (FLOAT_SPEED * s.sf)) * 1000), 'float', SWAY * s.sf, spot.id)
          stage.umbrella(false)
        } else {
          film.pose(FRAME.down)
          arrived = await pet.go(x, spot.y, fallMs(drop, s.sf), 'fall', 0, spot.id)
        }
        settled = true
        if (!alive()) return
        if (!arrived) return plain()
        // Just looking whether she is on something (after a walk, say): she is.
        if (still && drop <= ON_IT * s.sf) return film.pose(null)
        await touchDown(alive, dizzy)
      })
    }

    // On her feet: down into a crouch and up, with dust; and dizzy after, if
    // she was shaken.
    async function touchDown(alive, dizzy = false) {
      endSwing()
      stage.set('land', true)
      stage.bits('dust')
      film.sound('land')
      film.pose(FRAME.crouch)
      await wait(130)
      if (!alive()) return
      film.pose(FRAME.landed)
      await wait(110)
      if (!alive()) return
      stage.set('land', false)
      film.pose(null)
      if (dizzy) {
        film.react('failed', 2)
        stage.bits('stars')
        film.sound('dizzy')
      }
    }

    // --- Left alone

    // Looked at again only as what keeps her from going changes (rested,
    // leavePose, quietChanged, update, hiddenChanged): nothing ticks meanwhile.
    function plan() {
      clearTimeout(planned)
      if (!on() || d.hidden()) return
      planned = setTimeout(ignored, Math.max(1000, seenAt + IGNORED_MS * (1 + random() * 0.5) - now()))
    }

    function ignored() {
      if (!on() || d.hidden() || quiet || busy || held || asleep || mood !== 'idle' || pose !== 'stand') return
      if (now() - seenAt < IGNORED_MS) return plan()
      wander()
    }

    function wander() {
      return act(async alive => {
        const s = await pet.surfaces()
        if (!alive() || !s) return
        const goal = random() < UP_CHANCE ? goalOf(s, random) : null
        if (!goal) return lieDown(alive, s)
        let at = s.at
        for (const step of routeUp(s, goal)) {
          if (!(await take(step, at, s, goal, alive))) {
            if (alive()) plain()
            return
          }
          at = step.to
        }
        await touchDown(alive)
        if (alive()) await settleIn(alive, random() < 0.5 ? 'sit' : 'peek')
      })
    }

    // One step of the way from where she is: its clip, then the move; true
    // when it is done. The last of them leaves her standing on the window.
    async function take({ step, wall, to }, [x, y], s, goal, alive) {
      const last = step === 'over' || (step === 'hop' && to[0] === goal.x && to[1] === goal.y)
      const stands = last ? goal.id : null
      const far = Math.hypot(to[0] - x, to[1] - y)
      if (step === 'fall') {
        film.pose(FRAME.down)
        return pet.go(to[0], to[1], fallMs(to[1] - y, s.sf), 'fall', 0, stands)
      }
      if (step === 'run') {
        film.pose({ clip: to[0] >= x ? 'running-right' : 'running-left' })
        return pet.go(to[0], to[1], Math.round((far / (RUN_SPEED * s.sf)) * 1000), 'line', 0, stands)
      }
      if (step === 'up') {
        stage.set(`wall-${wall}`, true)
        film.pose({ clip: wall === 'r' ? 'running-right' : 'running-left' })
        return pet.go(to[0], to[1], Math.round((far / (UP_SPEED * s.sf)) * 1000), 'line', 0, stands)
      }
      // A jump; over the top, with her box turning back upright as she goes.
      if (step === 'over') {
        stage.set('wall-l', false)
        stage.set('wall-r', false)
      } else {
        film.pose(FRAME.crouch)
        await wait(90)
        if (!alive()) return false
      }
      film.pose(FRAME.air)
      film.sound('hop')
      return pet.go(to[0], to[1], hopMs(far, s.sf), 'arc', hopHigh(y - to[1], far, s.cell[1]), stands)
    }

    async function settleIn(alive, how) {
      const s = await pet.surfaces()
      if (!alive() || !s) return
      pose = how
      if (how === 'sit') return film.pose(FRAME.crouch)
      film.pose(null)
      stage.set('peek', true)
      await pet.go(s.at[0], s.at[1] + Math.round(s.cell[1] * PEEK_CUT) - soleOf(s.cell), 240, 'line', 0, s.on)
    }

    // Down to the floor, and down on her side there.
    async function lieDown(alive, s) {
      const floor = floorOf(s.area)
      if (s.at[1] < floor - ON_IT * s.sf) {
        film.pose(FRAME.down)
        if (!(await pet.go(s.at[0], floor, fallMs(floor - s.at[1], s.sf), 'fall', 0, null))) return plain()
        if (!alive()) return
        await touchDown(alive)
        if (!alive()) return
      }
      pose = 'lie'
      film.pose(FRAME.crouch)
      stage.set(random() < 0.5 ? 'lie-l' : 'lie-r', true)
    }

    // --- The windows change

    async function front() {
      if (!on() || held || busy || mood !== 'idle' || quiet || d.hidden()) return
      const was = doing
      const s = await pet.surfaces()
      if (!s || was !== doing || held || busy) return
      const tops = topsOf(s)
      const mine = s.on !== null && tops.find(t => t.id === s.on)
      const covered = s.on !== null && !(mine && isOver(s.at[0], mine.spans))
      const up = tops.find(t => t.front)
      const hops = up && up.id !== s.on && now() - hoppedAt >= HOP_EVERY_MS && random() < HOP_CHANCE
      if (hops) return hopOnto(up, s)
      if (covered) {
        plain()
        land({ surprised: true })
      }
    }

    function hopOnto(top, s) {
      hoppedAt = now()
      const rest = pose === 'stand' ? null : pose === 'peek' ? 'peek' : 'sit'
      plain()
      return act(async alive => {
        const to = [nearestIn(top.spans, s.at[0]), top.top + soleOf(s.cell)]
        const goal = { id: top.id, x: to[0], y: to[1], top }
        if (!(await take({ step: 'hop', to }, s.at, s, goal, alive))) {
          if (alive()) plain()
          return
        }
        await touchDown(alive)
        if (alive() && rest) await settleIn(alive, rest)
      })
    }

    function perchGone() {
      if (!on() || held || d.hidden()) return
      if (asleep) wake(false)
      plain()
      stage.bits('gasp')
      film.sound('oops')
      land({ surprised: true })
    }

    // --- Stroked

    function stroked(x, onHead) {
      if (!on()) return
      seen()
      if (held || !onHead) return
      if (!rub.add(x, now()) || now() - pattedAt < PAT_EVERY_MS) return
      pattedAt = now()
      rub.reset()
      film.closeList()
      stage.bits('hearts')
      film.sound('heart')
      if (asleep) wake(false)
      if (mood === 'idle') film.react('wave', 1)
    }

    // --- Nobody about

    function sleep() {
      if (asleep) return
      asleep = true
      sleptAt = now()
      stage.asleep(true)
      if (pose === 'stand' || pose === 'sit') film.pose(FRAME.crouch)
    }

    function wake(waves) {
      if (!asleep) return
      asleep = false
      stage.asleep(false)
      if (pose === 'stand') film.pose(null)
      // Back after a long while: hello, and she stays to be seen a while.
      if (waves && mood === 'idle' && now() - sleptAt >= WAVE_AFTER_MS) {
        seen()
        film.react('wave', 1)
        film.sound('hello')
      }
    }

    function quietChanged(q) {
      quiet = q
      if (!on()) return
      if (q) {
        if (!busy && !held && mood === 'idle') sleep()
        return
      }
      wake(true)
      plan()
    }

    // --- From main

    function update(data) {
      const was = on()
      config = data.config || config
      quiet = data.quiet === true
      const wasMood = mood
      mood = data.mood || 'idle'
      stage.set('plays', on())
      if (was && !on()) return turnedOff()
      if (!on()) return
      // Someone's session wants her: awake and on her feet, to show it.
      if (mood !== 'idle' && wasMood === 'idle') {
        wake(false)
        leavePose()
      } else if (mood === 'idle' && wasMood !== 'idle') {
        plan()
      }
      if ((!was || !settled) && !d.hidden() && !busy && !held) {
        settled = true
        land({ still: true })
      }
    }

    // Off: as she always was, standing where she is, on nothing of hers.
    function turnedOff() {
      clearTimeout(planned)
      settled = false
      const peeking = pose === 'peek'
      wake(false)
      if (peeking) return popUp()
      doing++
      busy = false
      held = false
      pressed = false
      endSwing()
      plain()
      pet.surfaces().then(s => s && pet.go(s.at[0], s.at[1], 0, 'line', 0, null))
    }

    // Shown again: on her window still (pet.rs keeps it), or down to what is
    // under her wherever she has come out. Put away, what she was doing ends.
    async function hiddenChanged(hidden) {
      if (!on()) return
      if (hidden) {
        clearTimeout(planned)
        if (held || busy) stop()
        held = false
        endSwing()
        return
      }
      const was = doing
      const s = await pet.surfaces()
      if (was !== doing || held || busy) return
      if (s && s.on === null) land({ still: true })
      plan()
    }

    return {
      update,
      pressed: pressedOn,
      dragged,
      dragEnd,
      letGo,
      perchGone,
      front,
      stroked,
      seen,
      quietChanged,
      hiddenChanged,
      // pet.js: not free for a walk or a glance.
      holding: () => on() && (busy || held || asleep || pose !== 'stand'),
      // pet.js: an idle walk over, perhaps off the edge of a window.
      walked: () => {
        if (on() && !busy && !held && !d.hidden()) land({ still: true })
      },
      rubbing: () => on() && rub.started(now()),
      // For the tests, and /debug/eval on a copy of her (window.Play.machine).
      state: () => ({ pose, asleep, busy, held }),
      wander,
    }
  }

  // --- In her page -------------------------------------------------------------------

  const UMBRELLA =
    '<svg viewBox="0 0 64 56" aria-hidden="true"><path d="M32 6c15 0 27 10 29 24-3-3-7-3-10 0-3-3-8-3-11 0-2-2-5-3-8-3s-6 1-8 3c-3-3-8-3-11 0-3-3-7-3-10 0C5 16 17 6 32 6z" fill="#ff8fa3"/><path d="M32 6c-7 2-12 12-13 24 3-3 8-3 13-3s10 0 13 3C44 18 39 8 32 6z" fill="#ffe1e6"/><path d="M32 2v4M32 27v22c0 5 7 5 7 0" fill="none" stroke="#6b4a4a" stroke-width="2.4" stroke-linecap="round"/></svg>'

  // Her box's classes and how she hangs; what comes off her (each gone once
  // played), and what stays until taken away.
  function stageOf(sprite, fx) {
    const made = (cls, html) => {
      const node = document.createElement('span')
      node.className = cls
      if (html) node.innerHTML = html
      return node
    }
    const between = (a, b) => a + Math.random() * (b - a)
    const BITS = {
      dust: () =>
        [0, 1, 2, 3, 4, 5].map(i => {
          const node = made('her-dust')
          const dir = i % 2 ? 1 : -1
          node.style.setProperty('--to-x', `${dir * between(16, 62)}px`)
          node.style.setProperty('--to-y', `${-between(2, 18)}px`)
          node.style.animationDelay = `${i * 12}ms`
          return node
        }),
      hearts: () =>
        [0, 1, 2].map(i => {
          const node = made('her-heart')
          node.textContent = '♥︎'
          node.style.setProperty('--from-x', `${(i - 1) * 16}px`)
          node.style.setProperty('--to-x', `${(i - 1) * between(18, 30)}px`)
          node.style.animationDelay = `${i * 110}ms`
          return node
        }),
      stars: () => {
        const ring = made('her-stars')
        for (let i = 0; i < 3; i++) {
          const star = made('her-star')
          star.textContent = '★︎'
          star.style.setProperty('--turn', `${i * 120}deg`)
          ring.append(star)
        }
        return [ring]
      },
      gasp: () => {
        const node = made('her-gasp')
        node.textContent = '!'
        return [node]
      },
    }
    let umbrella = null
    let zzz = null
    let hung = ''
    return {
      set: (name, on) => sprite.classList.toggle(`her-${name}`, on),
      // Written only as it changes: each write is a frame of her window.
      tilt(angle, stretch) {
        const style = sprite.style
        const now = `${angle.toFixed(1)} ${stretch.toFixed(3)}`
        if (now === hung) return
        hung = now
        style.setProperty('--her-tilt', `${angle.toFixed(1)}deg`)
        style.setProperty('--her-sy', stretch.toFixed(3))
        style.setProperty('--her-sx', (1 / stretch).toFixed(3))
        if (angle === 0 && stretch === 1) for (const v of ['--her-tilt', '--her-sy', '--her-sx']) style.removeProperty(v)
      },
      bits(name) {
        for (const node of BITS[name]()) {
          // Its own animation's end, or the first of what is in it (the stars).
          node.addEventListener('animationend', () => node.remove())
          fx.append(node)
        }
      },
      umbrella(on) {
        if (on && !umbrella) {
          umbrella = made('her-umbrella', UMBRELLA)
          fx.append(umbrella)
        } else if (!on && umbrella) {
          const going = umbrella
          umbrella = null
          going.classList.add('going')
          going.addEventListener('animationend', () => going.remove())
        }
      },
      asleep(on) {
        if (on && !zzz) {
          zzz = made('her-zzz', '<i>z</i><i>z</i><i>Z</i>')
          fx.append(zzz)
        } else if (!on && zzz) {
          zzz.remove()
          zzz = null
        }
      },
    }
  }

  // Her page's: the machine above, on the bridge and her box.
  function attach({ pet, sprite, fx, film, sheet, sf }) {
    const machine = create({
      pet,
      film,
      stage: stageOf(sprite, fx),
      sheet,
      hidden: () => document.hidden,
      reduced: () => matchMedia('(prefers-reduced-motion: reduce)').matches,
      now: () => performance.now(),
      wait: ms => new Promise(done => setTimeout(done, ms)),
      random: Math.random,
      frame: fn => requestAnimationFrame(fn),
    })
    pet.onUpdate(data => machine.update(data))
    pet.onDragStart(() => machine.pressed())
    pet.onDrag(dx => machine.dragged(dx, sf()))
    pet.onDragEnd(() => machine.dragEnd())
    pet.onLetGo(word => machine.letGo(word))
    pet.onPerchGone(() => machine.perchGone())
    pet.onFront(() => machine.front())
    pet.onQuiet(({ on }) => machine.quietChanged(on === true))
    document.addEventListener('visibilitychange', () => machine.hiddenChanged(document.hidden))
    sprite.addEventListener('mouseenter', () => machine.seen())
    sprite.addEventListener('mousemove', e => {
      const box = sprite.getBoundingClientRect()
      machine.stroked(e.clientX, e.clientY <= box.top + box.height * 0.55)
    })
    api.machine = machine
    return machine
  }

  const api = { cut, topsOf, spotUnder, nearestIn, goalOf, routeUp, fallMs, hopMs, hopHigh, swinging, shaking, rubbing, create, attach, IGNORED_MS, PEEK_CUT, FLOAT_FROM, HOP_EVERY_MS, WAVE_AFTER_MS }

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api
  } else {
    root.Play = api
  }
})(this ?? window)
