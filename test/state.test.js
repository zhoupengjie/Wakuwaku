const assert = require('node:assert/strict')
const { test, mock } = require('node:test')

const { createPet, STALE_MS, FORGET_MS } = require('../src/main/state')

function setup({ hold = 'seen' } = {}) {
  mock.timers.enable({ apis: ['setTimeout', 'Date'] })
  const changes = []
  const reactions = []
  const alerts = []
  const pet = createPet({
    onChange: s => changes.push(s.mood),
    onReact: r => reactions.push(r),
    onAlert: a => alerts.push(a),
    hold: () => hold,
  })
  const a = (msg, session = 'A', project = 'alpha') => pet.apply({ session, project, ...msg })
  return { pet, changes, reactions, alerts, a }
}

test('a turn without edits ends done, one with edits ends in review', t => {
  t.after(() => mock.timers.reset())
  const { pet, a } = setup()

  a({ mood: 'working', event: 'turn-start' })
  a({ mood: 'working', detail: 'Read', event: 'tool-done' })
  a({ mood: 'done' })
  assert.equal(pet.get().mood, 'done')

  a({ mood: 'working', event: 'turn-start' })
  a({ mood: 'working', detail: 'Edit' })
  a({ mood: 'working', detail: 'Edit', event: 'tool-done' })
  a({ mood: 'done' })
  assert.equal(pet.get().mood, 'review')

  a({ mood: 'working', event: 'turn-start' })
  a({ mood: 'done' })
  assert.equal(pet.get().mood, 'done')
})

test('an edit only counts once it has run', t => {
  t.after(() => mock.timers.reset())
  const { pet, a } = setup()
  a({ mood: 'working', event: 'turn-start' })
  a({ mood: 'working', detail: 'Write' }) // asked for, then denied
  a({ mood: 'done' })
  assert.equal(pet.get().mood, 'done')
})

test('sessions keep their own mood; the one that most wants you is shown', t => {
  t.after(() => mock.timers.reset())
  const { pet, a } = setup()

  a({ mood: 'working', event: 'turn-start' }, 'A', 'alpha')
  mock.timers.tick(1000)
  a({ mood: 'done' }, 'B', 'beta')
  // A still working does not hide that B is done...
  a({ mood: 'working', detail: 'Bash' }, 'A', 'alpha')
  assert.deepEqual([pet.get().mood, pet.get().project, pet.get().others, pet.get().sessions], ['done', 'beta', 1, 2])

  // ...and a prompt anywhere comes first.
  a({ mood: 'waiting', detail: { key: 'detail.needsApproval', vars: { tool: 'Bash' } } }, 'A', 'alpha')
  assert.deepEqual([pet.get().mood, pet.get().project], ['waiting', 'alpha'])
  assert.deepEqual(pet.get().detail, { key: 'detail.needsApproval', vars: { tool: 'Bash' } })

  // A session that ends is forgotten.
  a({ mood: 'idle', event: 'session-end' }, 'A', 'alpha')
  assert.deepEqual([pet.get().mood, pet.get().sessions], ['done', 1])
})

test('endings wait until seen by default', t => {
  t.after(() => mock.timers.reset())
  const { pet, a } = setup()

  a({ mood: 'done' })
  a({ mood: 'error' }, 'B')
  mock.timers.tick(60 * 60 * 1000)
  assert.equal(pet.get().mood, 'error')

  assert.equal(pet.seen(), true)
  assert.equal(pet.get().mood, 'idle')
  // Nothing left to see: no change, no redraw.
  assert.equal(pet.seen(), false)
})

test('seen leaves work and prompts alone', t => {
  t.after(() => mock.timers.reset())
  const { pet, a } = setup()
  a({ mood: 'waiting' })
  a({ mood: 'done' }, 'B')
  pet.seen()
  assert.equal(pet.get().mood, 'waiting')
  assert.equal(pet.get().others, 0)
})

test('with hold set to seconds, endings settle on their own', t => {
  t.after(() => mock.timers.reset())
  const { pet, a } = setup({ hold: 8 })

  for (const mood of ['done', 'error']) {
    a({ mood })
    mock.timers.tick(7999)
    assert.equal(pet.get().mood, mood)
    mock.timers.tick(1)
    assert.equal(pet.get().mood, 'idle')
  }
})

test('new work cancels a settle', t => {
  t.after(() => mock.timers.reset())
  const { pet, a } = setup({ hold: 8 })
  a({ mood: 'done' })
  mock.timers.tick(3000)
  a({ mood: 'working', event: 'turn-start' })
  mock.timers.tick(60000)
  assert.equal(pet.get().mood, 'working')
})

test('a turn is timed from its start; the ending keeps how long it took', t => {
  t.after(() => mock.timers.reset())
  const { pet, a } = setup()

  a({ mood: 'working', event: 'turn-start' })
  const since = pet.get().since
  assert.equal(typeof since, 'number')
  mock.timers.tick(30000)
  a({ mood: 'waiting' })
  mock.timers.tick(15000)
  a({ mood: 'working', detail: 'Bash' })
  // One clock through the whole turn, prompts included.
  assert.equal(pet.get().since, since)
  mock.timers.tick(5000)
  a({ mood: 'done' })
  assert.equal(pet.get().took, 50000)
  assert.equal(pet.get().since, null)

  a({ mood: 'working', event: 'turn-start' })
  assert.equal(pet.get().took, null)
})

test('alerts fire when a session starts to want you, once', t => {
  t.after(() => mock.timers.reset())
  const { a, alerts } = setup()

  a({ mood: 'working' })
  a({ mood: 'waiting' })
  a({ mood: 'waiting' })
  a({ mood: 'working' })
  a({ mood: 'done' })
  a({ mood: 'error' }, 'B', 'beta')
  assert.deepEqual(
    alerts.map(x => `${x.project}:${x.mood}`),
    ['alpha:waiting', 'alpha:done', 'beta:error'],
  )
})

test('a stale mood gives up; quiet sessions are forgotten', t => {
  t.after(() => mock.timers.reset())
  const { pet, a } = setup()

  a({ mood: 'working' })
  mock.timers.tick(STALE_MS + 1)
  pet.checkStale()
  assert.equal(pet.get().mood, 'idle')

  a({ mood: 'done' }, 'B')
  mock.timers.tick(FORGET_MS + 1)
  pet.checkStale()
  assert.equal(pet.get().mood, 'idle')
  mock.timers.tick(FORGET_MS + 1)
  pet.checkStale()
  assert.equal(pet.get().sessions, 0)
})

test('reactions play over the mood without changing it', t => {
  t.after(() => mock.timers.reset())
  const { pet, a, reactions, changes } = setup()

  a({ mood: 'working' })
  assert.equal(pet.apply({ react: 'jump', say: { key: 'say.taskDone', vars: { task: 'x' } } }), true)
  assert.equal(pet.get().mood, 'working')
  assert.deepEqual(reactions, [{ react: 'jump', say: { key: 'say.taskDone', vars: { task: 'x' } } }])
  assert.deepEqual(changes, ['working'])
})

test('messages that say nothing known are refused', t => {
  t.after(() => mock.timers.reset())
  const { pet, changes } = setup()
  for (const msg of [null, 'working', {}, { mood: 'sleeping' }, { react: 'dance' }, { detail: 'x' }]) {
    assert.equal(pet.apply(msg), false, JSON.stringify(msg))
  }
  assert.deepEqual(changes, [])
})

test('texts are cut and must be strings or translation keys', t => {
  t.after(() => mock.timers.reset())
  const { pet, a } = setup()

  a({ mood: 'working', detail: 'x'.repeat(200) })
  assert.equal(pet.get().detail.length, 80)
  a({ mood: 'working', detail: { evil: true } })
  assert.equal(pet.get().detail, '')
  a({ mood: 'working', detail: { key: 'detail.needsApproval', vars: { tool: { nested: 1 }, ok: 'Bash' } } })
  assert.deepEqual(pet.get().detail, { key: 'detail.needsApproval', vars: { ok: 'Bash' } })
})
