// A session in words, for her bubble, the island and the settings: what it
// is called, where it is (the step it is on, how far down its to-do list,
// for how long) and how its turn ended. Loaded by the page before pet.js,
// and required by the tests.
//
// A session is what state.rs view() makes of one:
//   { id, project, mood, detail, since, took, name, step, stepSince, todo,
//     files, reply, error }
// With the specifics kept off screen (the details setting off) only the
// mood, the project and the tool's name show, as before they were there.
;(function (root) {
  const { t, render } = root.I18n || require('../shared/i18n')

  // A step has run long enough to say for how long.
  const STEP_CLOCK_MS = 10000

  function clock(ms) {
    const s = Math.max(0, Math.floor(ms / 1000))
    const h = Math.floor(s / 3600)
    const m = Math.floor((s % 3600) / 60)
    const sec = String(s % 60).padStart(2, '0')
    return h ? `${h}:${String(m).padStart(2, '0')}:${sec}` : `${m}:${sec}`
  }

  const isEnding = s => s.mood === 'done' || s.mood === 'review' || s.mood === 'error'
  const isBusy = s => s.mood === 'working' || s.mood === 'waiting'

  // How long this turn has run, or how long the finished one took.
  function time(s, now = Date.now()) {
    if (isBusy(s) && s.since) return clock(now - s.since)
    if (isEnding(s) && s.took) return clock(s.took)
    return ''
  }

  // What it is called: its name, or else its project.
  const nameOf = (s, detailed = true) => (detailed && s.name) || s.project || ''

  // The step it is on: n/m of its to-do list, the step, and for how long.
  function stepOf(lang, s, { now = Date.now(), withClock = true } = {}) {
    const parts = []
    if (s.todo && s.todo.total) parts.push(`${s.todo.done}/${s.todo.total}`)
    const ran = s.stepSince ? now - s.stepSince : 0
    const isTimed = withClock && ran >= STEP_CLOCK_MS
    if (s.step && s.step.key === 'step.thinking' && isTimed) {
      parts.push(t(lang, 'step.thinkingFor', { time: clock(ran) }))
    } else {
      parts.push(render(lang, s.step) || t(lang, 'mood.working'))
      if (isTimed) parts.push(t(lang, 'step.for', { time: clock(ran) }))
    }
    return parts.join(' · ')
  }

  // Where it is, in a line. opts: { detailed, now, withClock }.
  function status(lang, s, opts = {}) {
    const detailed = opts.detailed !== false
    switch (s.mood) {
      case 'working':
        return detailed ? stepOf(lang, s, opts) : [t(lang, 'mood.working'), render(lang, s.detail)].filter(Boolean).join(' · ')
      case 'waiting':
        // What it waits for may name a command or a file.
        return ((detailed || s.detail?.key !== 'detail.approve') && render(lang, s.detail)) || t(lang, 'mood.waiting')
      case 'review':
        return detailed && s.files ? t(lang, s.files === 1 ? 'status.reviewFile' : 'status.reviewFiles', { n: s.files }) : t(lang, 'mood.review')
      case 'error':
        return detailed && s.error ? t(lang, 'status.error', { why: render(lang, s.error) }) : t(lang, 'mood.error')
      default:
        return t(lang, `mood.${s.mood || 'idle'}`)
    }
  }

  // The compact island's few words: where a busy one is, whose an ending is.
  function brief(lang, s, opts = {}) {
    if (!s.mood || s.mood === 'idle') return ''
    if (opts.detailed === false) return [s.project, t(lang, `island.${s.mood}`)].filter(Boolean).join(' · ')
    if (isBusy(s)) return status(lang, s, { ...opts, withClock: false })
    return [t(lang, `island.${s.mood}`), nameOf(s)].filter(Boolean).join(' · ')
  }

  // The others: every session but the one shown that is busy or has an ending.
  const othersOf = (list, shown) => (list || []).filter(x => x.id !== shown.id && x.mood && x.mood !== 'idle')

  // The one of them that most wants you, if one does.
  const urgentOf = (list, shown) => othersOf(list, shown).find(x => x.mood === 'waiting' || x.mood === 'error')

  const api = { STEP_CLOCK_MS, clock, time, isEnding, nameOf, stepOf, status, brief, othersOf, urgentOf }

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api
  } else {
    root.Status = api
  }
})(this)
