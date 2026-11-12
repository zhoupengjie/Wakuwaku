// The island's shape and what is done with it (src/pet/island-state.ts):
// the events island.ts and its parts send, and the state they leave.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import * as S from '../src/pet/island-state.ts'
import type { IslandEvent, IslandState, Nudge } from '../src/pet/island-state.ts'

const run = (events: IslandEvent[], from: IslandState = S.initial()) => events.reduce(S.next, from)
const draw = (s: IslandState, { settings = false, asking = false, now = 1000 } = {}) => S.next(s, { type: 'draw', settings, asking, now })
const nudge = (until: number, extra: Partial<Nudge> = {}): Nudge => ({ until, clip: 'waving', text: null, widget: null, ...extra })

test('its shape: the settings over all, then a prompt, then open while rested on, nudged or peeking', () => {
  const s = S.initial()
  assert.equal(s.view, '')
  assert.equal(draw(s).view, 'compact')
  const open = { ...s, isHover: true, peek: true, nudge: nudge(5000) }
  assert.equal(draw(open, { settings: true, asking: true }).view, 'settings')
  assert.equal(draw(open, { asking: true }).view, 'ask')
  for (const one of [{ isHover: true }, { peek: true }, { nudge: nudge(5000) }]) assert.equal(draw({ ...s, ...one }).view, 'expanded', JSON.stringify(one))
  // A nudge whose time is up opens nothing.
  assert.equal(draw({ ...s, nudge: nudge(1000) }, { now: 1000 }).view, 'compact')
  assert.equal(S.nudging({ ...s, nudge: nudge(1000) }, 999), true)
})

test('a prompt answered closes it rather than leaving it open by itself', () => {
  const asked = draw({ ...S.initial(), nudge: nudge(Infinity) }, { asking: true })
  assert.equal(asked.view, 'ask')
  assert.ok(asked.nudge, 'kept while the prompt is up')
  const answered = draw(asked)
  assert.equal(answered.nudge, null)
  assert.equal(answered.view, 'compact')
  // Not after a prompt: a nudge stays.
  assert.ok(draw(draw({ ...S.initial(), nudge: nudge(Infinity) })).nudge)
})

test('the pointer: rested on opens it, a press that closed it holds until the pointer has left', () => {
  let s = run([{ type: 'rest' }])
  assert.equal(s.isHover, true)
  s = run([{ type: 'nudge', nudge: nudge(5000) }, { type: 'dismiss' }], s)
  assert.deepEqual([s.isHover, s.dismissed, s.nudge], [false, true, null])
  assert.equal(draw(s).view, 'compact')
  // Resting again under the same pointer: nothing.
  assert.equal(S.next(s, { type: 'rest' }), s)
  // Left and come back: a hover as any other.
  s = run([{ type: 'leave' }, { type: 'rest' }], s)
  assert.deepEqual([s.isHover, s.dismissed], [true, false])
})

test('open by itself: rested on and left, it closes; only passed across, it stays', () => {
  const nudged = run([{ type: 'nudge', nudge: nudge(5000) }])
  const rested = run([{ type: 'rest' }, { type: 'leave' }, { type: 'gone', now: 1000 }], nudged)
  assert.deepEqual([rested.isHover, rested.nudge], [false, null])
  const passed = run([{ type: 'leave' }, { type: 'gone', now: 1000 }], nudged)
  assert.equal(passed.nudge, nudged.nudge)
  // Its time already up: nothing to close, the nudge left as it is.
  const late = run([{ type: 'rest' }, { type: 'gone', now: 6000 }], nudged)
  assert.equal(late.nudge, nudged.nudge)
})

test('held until seen: the pointer coming gives it its own time from then', () => {
  const held = run([{ type: 'nudge', nudge: nudge(Infinity, { after: 3600, kind: 'done' }) }])
  const seen = S.next(held, { type: 'seen', now: 2000 })
  assert.equal(seen.nudge.until, 5600)
  assert.equal(seen.nudge.kind, 'done')
  // One with a time of its own is left as it is.
  const timed = run([{ type: 'nudge', nudge: nudge(4000, { after: 3600 }) }])
  assert.equal(S.next(timed, { type: 'seen', now: 2000 }), timed)
  assert.equal(S.next(S.initial(), { type: 'seen', now: 2000 }).nudge, null)
  assert.equal(run([{ type: 'unnudge' }], held).nudge, null)
})

test('the peek from the notch, and held open or closed as it is', () => {
  const peeking = run([{ type: 'peek', on: true }])
  assert.equal(draw(peeking).view, 'expanded')
  assert.equal(draw(S.next(peeking, { type: 'peek', on: false })).view, 'compact')
  assert.equal(run([{ type: 'hover', on: true }]).isHover, true)
  assert.equal(run([{ type: 'rest' }, { type: 'hover', on: false }]).isHover, false)
})

test('her pulled out: a press that never moved is a click, the drop holds the island until let go', () => {
  const pressed = run([{ type: 'press', x: 10, y: 20 }])
  assert.deepEqual(pressed.pull, { x0: 10, y0: 20, started: false, broken: false, at: null, carrying: false })
  assert.equal(S.held(pressed), false)
  assert.equal(S.next(pressed, { type: 'release' }).pull, null)

  const pulling = run([{ type: 'nudge', nudge: nudge(Infinity) }, { type: 'press', x: 10, y: 20 }, { type: 'pull' }])
  assert.equal(pulling.nudge, null, 'the drop has her: what opened it is over')
  assert.equal(S.held(pulling), true)
  const stretched = S.next(pulling, { type: 'stretch', broken: false, at: { cx: 12, cy: 60 } })
  assert.deepEqual([stretched.pull.broken, stretched.pull.at], [false, { cx: 12, cy: 60 }])
  // Let go in the drop: she springs back first, the pull kept until she is in her seat.
  const letGo = S.next(stretched, { type: 'release' })
  assert.equal(letGo.pull, stretched.pull)
  assert.equal(S.held(letGo), true)
  assert.equal(S.next(letGo, { type: 'settled' }).pull, null)

  // Pinched off: her window carries her, the island is drawn again; let go, over.
  const carried = run([{ type: 'stretch', broken: true, at: { cx: 12, cy: 200 } }, { type: 'carry' }], pulling)
  assert.equal(carried.pull.carrying, true)
  assert.equal(S.held(carried), false)
  assert.equal(S.next(carried, { type: 'release' }).pull, null)
  assert.equal(S.next(carried, { type: 'landed' }).pull, null)
  // Nothing to stretch or carry with no press.
  assert.equal(S.next(S.initial(), { type: 'carry' }).pull, null)
})

test('reached for and taken back in: held while she is taken in, home until main says or the wait is over', () => {
  const reaching = run([{ type: 'reach', on: true }])
  assert.equal(reaching.isReaching, true)
  assert.equal(S.next(reaching, { type: 'reach', on: false }).isReaching, false)
  const taking = S.next(reaching, { type: 'absorb' })
  assert.deepEqual([taking.isReaching, taking.isAbsorbing, taking.isComingHome], [false, true, true])
  assert.equal(S.held(taking), true)
  const settled = S.next(taking, { type: 'settled' })
  assert.deepEqual([settled.isAbsorbing, settled.isComingHome], [false, true])
  assert.equal(S.next(settled, { type: 'homecoming-over' }).isComingHome, false)
})

test('her own window, or the island off: nothing of its own left, drawn anew from nothing', () => {
  const busy = run([{ type: 'press', x: 0, y: 0 }, { type: 'pull' }, { type: 'reach', on: true }, { type: 'absorb' }])
  const off = S.next(draw(busy), { type: 'off' })
  assert.deepEqual([off.view, off.pull, off.isReaching, off.isAbsorbing], ['', null, false, false])
})

test('next never changes the state it is given, and stateOf takes it alone', () => {
  const s = Object.freeze({ ...S.initial(), nudge: Object.freeze(nudge(Infinity, { after: 10 })) })
  const all: IslandEvent[] = [
    { type: 'draw', settings: false, asking: true, now: 0 },
    { type: 'rest' },
    { type: 'leave' },
    { type: 'gone', now: 0 },
    { type: 'hover', on: true },
    { type: 'dismiss' },
    { type: 'nudge', nudge: nudge(1) },
    { type: 'unnudge' },
    { type: 'seen', now: 0 },
    { type: 'peek', on: true },
    { type: 'press', x: 0, y: 0 },
    { type: 'pull' },
    { type: 'stretch', broken: true, at: { cx: 0, cy: 0 } },
    { type: 'carry' },
    { type: 'release' },
    { type: 'settled' },
    { type: 'landed' },
    { type: 'reach', on: true },
    { type: 'absorb' },
    { type: 'homecoming-over' },
    { type: 'off' },
  ]
  // Each event once on the frozen state (frozen: a change in place throws), then all in turn.
  for (const e of all) S.next(s, e)
  run(all, s)
  const held = { ...S.initial(), view: 'compact' as const, other: 1 }
  assert.deepEqual(S.stateOf(held), { ...S.initial(), view: 'compact' })
})
