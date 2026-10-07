const assert = require('node:assert/strict')
const { test, mock } = require('node:test')

const { createPet, SETTLE_MS, STALE_MS } = require('../src/main/state')

function setup() {
  mock.timers.enable({ apis: ['setTimeout', 'Date'] })
  const changes = []
  const reactions = []
  const pet = createPet({
    onChange: s => changes.push(s.mood),
    onReact: r => reactions.push(r),
  })
  return { pet, changes, reactions }
}

test('a turn without edits ends done, one with edits ends in review', t => {
  t.after(() => mock.timers.reset())
  const { pet } = setup()

  pet.apply({ mood: 'working', event: 'turn-start' })
  pet.apply({ mood: 'working', detail: 'Read', event: 'tool-done' })
  pet.apply({ mood: 'done' })
  assert.equal(pet.get().mood, 'done')

  pet.apply({ mood: 'working', event: 'turn-start' })
  pet.apply({ mood: 'working', detail: 'Edit' })
  pet.apply({ mood: 'working', detail: 'Edit', event: 'tool-done' })
  pet.apply({ mood: 'done' })
  assert.equal(pet.get().mood, 'review')

  // The next turn starts clean.
  pet.apply({ mood: 'working', event: 'turn-start' })
  pet.apply({ mood: 'done' })
  assert.equal(pet.get().mood, 'done')
})

test('an edit only counts once it has run', t => {
  t.after(() => mock.timers.reset())
  const { pet } = setup()

  pet.apply({ mood: 'working', event: 'turn-start' })
  pet.apply({ mood: 'working', detail: 'Write' }) // asked for, then denied
  pet.apply({ mood: 'done' })
  assert.equal(pet.get().mood, 'done')
})

test('done, review and error settle back to idle', t => {
  t.after(() => mock.timers.reset())
  const { pet } = setup()

  for (const mood of ['done', 'error']) {
    pet.apply({ mood })
    mock.timers.tick(SETTLE_MS[mood] - 1)
    assert.equal(pet.get().mood, mood)
    mock.timers.tick(1)
    assert.equal(pet.get().mood, 'idle')
  }

  pet.apply({ mood: 'working', detail: 'Edit', event: 'tool-done' })
  pet.apply({ mood: 'done' })
  mock.timers.tick(SETTLE_MS.review)
  assert.equal(pet.get().mood, 'idle')
})

test('new work cancels the settle', t => {
  t.after(() => mock.timers.reset())
  const { pet } = setup()

  pet.apply({ mood: 'done' })
  mock.timers.tick(3000)
  pet.apply({ mood: 'working', event: 'turn-start' })
  mock.timers.tick(60000)
  assert.equal(pet.get().mood, 'working')
})

test('a stale mood gives up after 15 minutes', t => {
  t.after(() => mock.timers.reset())
  const { pet } = setup()

  pet.apply({ mood: 'working' })
  mock.timers.tick(STALE_MS - 1)
  pet.checkStale()
  assert.equal(pet.get().mood, 'working')
  mock.timers.tick(2)
  pet.checkStale()
  assert.equal(pet.get().mood, 'idle')
})

test('reactions play over the mood without changing it', t => {
  t.after(() => mock.timers.reset())
  const { pet, reactions, changes } = setup()

  pet.apply({ mood: 'working' })
  assert.equal(pet.apply({ react: 'jump', say: '完成：写测试' }), true)
  assert.equal(pet.get().mood, 'working')
  assert.deepEqual(reactions, [{ react: 'jump', say: '完成：写测试' }])
  assert.deepEqual(changes, ['working'])

  pet.apply({ mood: 'working', detail: 'Bash', react: 'failed', say: 'Bash 失败了' })
  assert.equal(reactions.at(-1).react, 'failed')
  assert.equal(pet.get().detail, 'Bash')
})

test('messages that say nothing known are refused', t => {
  t.after(() => mock.timers.reset())
  const { pet, changes } = setup()

  for (const msg of [null, 'working', {}, { mood: 'sleeping' }, { react: 'dance' }, { detail: 'x' }]) {
    assert.equal(pet.apply(msg), false, JSON.stringify(msg))
  }
  assert.deepEqual(changes, [])
})

test('detail is cut to 80 characters and must be text', t => {
  t.after(() => mock.timers.reset())
  const { pet } = setup()

  pet.apply({ mood: 'working', detail: 'x'.repeat(200) })
  assert.equal(pet.get().detail.length, 80)
  pet.apply({ mood: 'working', detail: { evil: true } })
  assert.equal(pet.get().detail, '')
})
