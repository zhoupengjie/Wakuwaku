// What the pet is doing, from the messages the hooks bring (see
// src/shared/hook-events.js). A message is
//   { session?, project?, mood?, detail?, event?, react?, say? }
//   mood     idle | working | waiting | done | review | error
//   detail   a tool name, or { key, vars } to translate
//   event    turn-start | tool-done | session-end
//   react    wave | jump | failed: played once over the mood, `say` in the bubble
//
// Each Claude Code session keeps its own mood; the pet shows the one that most
// wants you (waiting, then error, review, done, working, idle; the latest of
// equals), and how many others are busy.
//
// A turn that edited files ends in `review` rather than `done`, as a Codex task
// ends ready for review. done / review / error stay until you have seen them
// (seen(): the pointer on the pet) or, with hold set to a number of seconds,
// that long.
const MOODS = ['idle', 'working', 'waiting', 'done', 'review', 'error']
const REACTS = ['wave', 'jump', 'failed']
const EDIT_TOOLS = new Set(['Edit', 'Write', 'MultiEdit', 'NotebookEdit'])
const PRIORITY = { waiting: 5, error: 4, review: 3, done: 2, working: 1, idle: 0 }
const ENDINGS = new Set(['done', 'review', 'error'])

// A session that died mid-turn never says idle: give up on it after this.
const STALE_MS = 15 * 60 * 1000
// An ending nobody came to see, or a session gone quiet, after this.
const FORGET_MS = 2 * 60 * 60 * 1000

// A text a message may carry: a string, or { key, vars } of strings.
function text(value, max) {
  if (typeof value === 'string') return value.slice(0, max)
  if (value && typeof value === 'object' && typeof value.key === 'string') {
    const vars = {}
    for (const [k, v] of Object.entries(value.vars || {})) {
      if (typeof v === 'string' || typeof v === 'number') vars[k] = String(v).slice(0, max)
    }
    return { key: value.key.slice(0, 100), vars }
  }
  return ''
}

// hold(): 'seen', or seconds an ending stays.
function createPet({ onChange, onReact, onAlert = () => {}, hold = () => 'seen', now = Date.now, timers = globalThis }) {
  const sessions = new Map()

  function session(id, project) {
    let s = sessions.get(id)
    if (!s) {
      s = { id, project: '', mood: 'idle', detail: '', at: now(), since: null, took: null, hasEdited: false, settle: undefined }
      sessions.set(id, s)
    }
    if (project) s.project = project
    return s
  }

  function set(s, mood, detail) {
    const was = s.mood
    timers.clearTimeout(s.settle)
    s.settle = undefined
    s.mood = mood
    s.detail = detail
    s.at = now()

    if (mood === 'working' || mood === 'waiting') {
      if (s.since === null) s.since = s.at
      s.took = null
    } else {
      if (s.since !== null && ENDINGS.has(mood)) s.took = s.at - s.since
      s.since = null
    }

    const seconds = hold()
    if (ENDINGS.has(mood) && typeof seconds === 'number') {
      s.settle = timers.setTimeout(() => set(s, 'idle', ''), seconds * 1000)
    }
    if (mood !== was && (mood === 'waiting' || ENDINGS.has(mood))) {
      onAlert({ session: s.id, project: s.project, mood, detail })
    }
  }

  // Apply a message; false when it carries nothing we know.
  function apply(msg) {
    if (!msg || typeof msg !== 'object') return false
    const hasMood = MOODS.includes(msg.mood)
    const hasReact = REACTS.includes(msg.react)
    if (!hasMood && !hasReact) return false

    if (hasMood) {
      const id = typeof msg.session === 'string' ? msg.session.slice(0, 200) : ''
      const s = session(id, typeof msg.project === 'string' ? msg.project.slice(0, 200) : '')
      const detail = text(msg.detail, 80)

      if (msg.event === 'session-end') {
        timers.clearTimeout(s.settle)
        sessions.delete(id)
      } else {
        if (msg.event === 'turn-start') {
          s.hasEdited = false
          s.since = null
        }
        if (msg.event === 'tool-done' && typeof detail === 'string' && EDIT_TOOLS.has(detail)) s.hasEdited = true

        let mood = msg.mood
        if (mood === 'done' && s.hasEdited) mood = 'review'
        if (mood === 'done' || mood === 'review') s.hasEdited = false
        set(s, mood, detail)
      }
      onChange(get())
    }

    if (hasReact) {
      onReact({ react: msg.react, say: text(msg.say, 80) })
    }

    return true
  }

  // You looked: every ending goes back to rest.
  function seen() {
    let changed = false
    for (const s of sessions.values()) {
      if (ENDINGS.has(s.mood)) {
        set(s, 'idle', '')
        changed = true
      }
    }
    if (changed) onChange(get())
    return changed
  }

  function checkStale() {
    const t = now()
    let changed = false
    for (const s of [...sessions.values()]) {
      if ((s.mood === 'working' || s.mood === 'waiting') && t - s.at > STALE_MS) {
        set(s, 'idle', '')
        changed = true
      } else if (ENDINGS.has(s.mood) && t - s.at > FORGET_MS) {
        set(s, 'idle', '')
        changed = true
      } else if (s.mood === 'idle' && t - s.at > FORGET_MS) {
        sessions.delete(s.id)
      }
    }
    if (changed) onChange(get())
  }

  // What the pet shows: the session that most wants you, and the rest.
  function get() {
    const all = [...sessions.values()]
    const top = all.reduce(
      (best, s) => (!best || PRIORITY[s.mood] > PRIORITY[best.mood] || (PRIORITY[s.mood] === PRIORITY[best.mood] && s.at >= best.at) ? s : best),
      null,
    )
    if (!top) return { mood: 'idle', detail: '', project: '', since: null, took: null, at: now(), others: 0, sessions: 0 }
    const others = all.filter(s => s !== top && s.mood !== 'idle').length
    return {
      mood: top.mood,
      detail: top.detail,
      project: top.project,
      since: top.since,
      took: top.took,
      at: top.at,
      others,
      sessions: all.length,
    }
  }

  // Every session, the one that most wants you first, for the main window.
  function list() {
    return [...sessions.values()]
      .filter(s => s.id || s.mood !== 'idle')
      .sort((a, b) => PRIORITY[b.mood] - PRIORITY[a.mood] || b.at - a.at)
      .map(({ id, project, mood, detail, at, since, took }) => ({ id, project, mood, detail, at, since, took }))
  }

  return { apply, seen, checkStale, get, list }
}

module.exports = { createPet, MOODS, REACTS, STALE_MS, FORGET_MS }
