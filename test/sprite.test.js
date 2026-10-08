const assert = require('node:assert/strict')
const { test } = require('node:test')

const { CLIPS, MOOD_CLIP, REACTIONS, lookCell } = require('../src/pet/sprite')

// The 11 states codex-pets.net shows for a v2 pet, by row.
const SITE_ROWS = {
  idle: 0,
  'running-right': 1,
  'running-left': 2,
  waving: 3,
  jumping: 4,
  failed: 5,
  waiting: 6,
  running: 7,
  review: 8,
}
const SITE_FRAMES = [6, 8, 8, 4, 5, 8, 6, 6, 6]

test('every row of the sheet is a clip, with the site frame counts', () => {
  for (const [name, row] of Object.entries(SITE_ROWS)) {
    assert.equal(CLIPS[name]?.row, row, name)
    assert.equal(CLIPS[name].frames, SITE_FRAMES[row], name)
  }
})

test('every mood and reaction plays a clip that exists', () => {
  for (const clip of Object.values(MOOD_CLIP)) assert.ok(CLIPS[clip], clip)
  for (const { clip } of Object.values(REACTIONS)) assert.ok(CLIPS[clip], clip)
  assert.deepEqual(Object.keys(MOOD_CLIP).sort(), ['done', 'error', 'idle', 'review', 'waiting', 'working'])
})

test('the 16 look directions map as codex-pets.net maps them', () => {
  // Index 0 looks up; each next index turns 22.5 degrees clockwise.
  assert.deepEqual(lookCell(0, -100), { row: 9, frame: 0, index: 0 })
  assert.deepEqual(lookCell(100, -100), { row: 9, frame: 2, index: 2 })
  assert.deepEqual(lookCell(100, 0), { row: 9, frame: 4, index: 4 })
  assert.deepEqual(lookCell(0, 100), { row: 10, frame: 0, index: 8 })
  assert.deepEqual(lookCell(-100, 0), { row: 10, frame: 4, index: 12 })
  assert.deepEqual(lookCell(-100, -1), { row: 10, frame: 4, index: 12 })
  // Just short of a full turn rounds back to up.
  assert.deepEqual(lookCell(-1, -100), { row: 9, frame: 0, index: 0 })
})

test('all 16 cells are reachable', () => {
  const seen = new Set()
  for (let i = 0; i < 16; i += 1) {
    const angle = (i * Math.PI) / 8
    const { row, frame } = lookCell(Math.sin(angle), -Math.cos(angle))
    seen.add(`${row}:${frame}`)
  }
  assert.equal(seen.size, 16)
})
