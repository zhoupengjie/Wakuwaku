// The Codex pet v2 sheet: 1536x2288, 8 columns x 11 rows of 192x208 cells.
// Loaded by the page before pet.js, and required by the tests.
;(function (root) {
  const CELL_W = 192
  const CELL_H = 208

  // Every row of the sheet, by the name codex-pets.net gives it.
  const CLIPS = {
    idle: { row: 0, frames: 6, ms: 260 },
    'running-right': { row: 1, frames: 8, ms: 110 },
    'running-left': { row: 2, frames: 8, ms: 110 },
    waving: { row: 3, frames: 4, ms: 180 },
    jumping: { row: 4, frames: 5, ms: 130 },
    failed: { row: 5, frames: 8, ms: 160 },
    waiting: { row: 6, frames: 6, ms: 200 },
    running: { row: 7, frames: 6, ms: 140 },
    review: { row: 8, frames: 6, ms: 180 },
  }

  // What each mood loops.
  const MOOD_CLIP = {
    idle: 'idle',
    working: 'running',
    waiting: 'waiting',
    done: 'waving',
    review: 'review',
    error: 'failed',
  }

  // One-off reactions over the mood: the clip, and how many times it plays.
  const REACTIONS = {
    wave: { clip: 'waving', times: 2 },
    jump: { clip: 'jumping', times: 1 },
    failed: { clip: 'failed', times: 1 },
  }

  // Rows 9 and 10 hold 16 look directions: index 0 looks straight up, and
  // each next one turns 22.5 degrees clockwise (as codex-pets.net maps them).
  const LOOK_ROW = 9
  const DIRECTIONS = 16

  // The look cell for a point (dx, dy) away from the pet, screen axes (y down).
  function lookCell(dx, dy) {
    const degrees = ((Math.atan2(dx, -dy) * 180) / Math.PI + 360) % 360
    const index = Math.round(degrees / (360 / DIRECTIONS)) % DIRECTIONS
    return { row: LOOK_ROW + Math.floor(index / 8), frame: index % 8, index }
  }

  const api = { CELL_W, CELL_H, CLIPS, MOOD_CLIP, REACTIONS, lookCell }

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api
  } else {
    root.Sprite = api
  }
})(this)
