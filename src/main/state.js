// What the pet is doing, from the messages hooks/claude-hook.js posts.
//
// A message is { mood?, detail?, event?, react?, say? }:
//   mood    idle | working | waiting | done | review | error
//   event   turn-start (a prompt was sent) | tool-done (a tool finished)
//   react   wave | jump | failed: played once over the mood, with `say` in the bubble
//
// A turn that edited files ends in `review` rather than `done`, as a Codex
// task ends ready for review; done / review / error settle back to idle.
const MOODS = ['idle', 'working', 'waiting', 'done', 'review', 'error']
const REACTS = ['wave', 'jump', 'failed']
const EDIT_TOOLS = new Set(['Edit', 'Write', 'MultiEdit', 'NotebookEdit'])

const SETTLE_MS = { done: 8000, review: 20000, error: 8000 }
// A session that died mid-turn never says idle: give up on it after this.
const STALE_MS = 15 * 60 * 1000

function text(value, max) {
  return typeof value === 'string' ? value.slice(0, max) : ''
}

function createPet({ onChange, onReact, now = Date.now, timers = globalThis }) {
  let state = { mood: 'idle', detail: '', at: now() }
  let hasEdited = false
  let settle

  function set(mood, detail) {
    timers.clearTimeout(settle)
    state = { mood, detail, at: now() }
    if (SETTLE_MS[mood]) {
      settle = timers.setTimeout(() => set('idle', ''), SETTLE_MS[mood])
    }
    onChange(state)
  }

  // Apply a message; false when it carries nothing we know.
  function apply(msg) {
    if (!msg || typeof msg !== 'object') return false
    const hasMood = MOODS.includes(msg.mood)
    const hasReact = REACTS.includes(msg.react)
    if (!hasMood && !hasReact) return false

    if (hasMood) {
      const detail = text(msg.detail, 80)
      if (msg.event === 'turn-start') hasEdited = false
      if (msg.event === 'tool-done' && EDIT_TOOLS.has(detail)) hasEdited = true

      let mood = msg.mood
      if (mood === 'done' && hasEdited) mood = 'review'
      if (mood === 'done' || mood === 'review') hasEdited = false
      set(mood, detail)
    }

    if (hasReact) {
      onReact({ react: msg.react, say: text(msg.say, 80) })
    }

    return true
  }

  function checkStale() {
    if (state.mood !== 'idle' && now() - state.at > STALE_MS) set('idle', '')
  }

  return { apply, checkStale, get: () => state }
}

module.exports = { createPet, MOODS, REACTS, SETTLE_MS, STALE_MS }
