// A session in words, for her bubble, the island and the settings: what it
// is called, where it is (the step it is on, how far down its to-do list,
// for how long) and how its turn ended. Imported by the pages and the
// tests.
//
// A session is what state.rs view() makes of one:
//   { id, project, mood, detail, since, took, name, step, stepSince, todo,
//     files, reply, error }
// With the specifics kept off screen (the details setting off) only the
// mood, the project and the tool's name show, as before they were there.
import { t, render } from '../shared/i18n.ts'

// A session as state.rs view() makes it: the fields read here. The said
// parts (detail, step, error) are i18n.ts render's { key, vars }.
export interface Session {
  id?: string
  project?: string
  mood?: string
  detail?: any
  since?: number | null
  took?: number | null
  name?: string
  step?: any
  stepSince?: number | null
  todo?: { done: number; total: number; active?: string }
  files?: number
  reply?: string
  error?: any
  fileList?: string[]
  recentSteps?: { step?: any; ok?: boolean }[]
  others?: number
}

// A step has run long enough to say for how long.
const STEP_CLOCK_MS = 10000

function clock(ms: number) {
  const s = Math.max(0, Math.floor(ms / 1000))
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  const sec = String(s % 60).padStart(2, '0')
  return h ? `${h}:${String(m).padStart(2, '0')}:${sec}` : `${m}:${sec}`
}

const isEnding = (s: Session) => s.mood === 'done' || s.mood === 'review' || s.mood === 'error'
const isBusy = (s: Session) => s.mood === 'working' || s.mood === 'waiting'

// How long this turn has run, or how long the finished one took.
function time(s: Session, now = Date.now()) {
  if (isBusy(s) && s.since) return clock(now - s.since)
  if (isEnding(s) && s.took) return clock(s.took)
  return ''
}

// What it is called: its name, or else its project.
const nameOf = (s: Session, detailed = true) => (detailed && s.name) || s.project || ''

// The step it is on: n/m of its to-do list, the step, and for how long.
function stepOf(lang: string, s: Session, { now = Date.now(), withClock = true } = {}) {
  const parts: string[] = []
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

// How a line is said: with its specifics or not, at what time, with the
// step's clock or without (status's and brief's opts).
interface SayOpts {
  detailed?: boolean
  now?: number
  withClock?: boolean
}

// Where it is, in a line. opts: { detailed, now, withClock }.
function status(lang: string, s: Session, opts: SayOpts = {}) {
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
function brief(lang: string, s: Session, opts: SayOpts = {}) {
  if (!s.mood || s.mood === 'idle') return ''
  if (opts.detailed === false) return [s.project, t(lang, `island.${s.mood}`)].filter(Boolean).join(' · ')
  if (isBusy(s)) return status(lang, s, { ...opts, withClock: false })
  return [t(lang, `island.${s.mood}`), nameOf(s)].filter(Boolean).join(' · ')
}

// The others: every session but the one shown that is busy or has an ending.
const othersOf = (list: Session[] | null | undefined, shown: Session) => (list || []).filter(x => x.id !== shown.id && x.mood && x.mood !== 'idle')

// The one of them that most wants you, if one does.
const urgentOf = (list: Session[] | null | undefined, shown: Session) => othersOf(list, shown).find(x => x.mood === 'waiting' || x.mood === 'error')

// What she says over her head out on the desktop (pet.ts), as the closed
// island would: nothing at rest; a busy one what it is doing and for how
// long; an ending how it went, how long it took and (with the specifics)
// Claude's last words. { mood, name, what, clock, reply, others }, or null.
function talkOf(lang: string, s: Session | null | undefined, { detailed = true, now = Date.now() } = {}) {
  if (!s || !s.mood || s.mood === 'idle') return null
  const name = nameOf(s, detailed)
  const what = s.mood === 'done' ? t(lang, 'island.done') : status(lang, s, { detailed, now, withClock: false })
  const took = isEnding(s) && s.took ? t(lang, 'detail.took', { time: clock(s.took) }) : ''
  return {
    mood: s.mood,
    name,
    what: what === name ? '' : what,
    clock: isBusy(s) ? time(s, now) : took,
    reply: detailed && isEnding(s) && s.reply ? s.reply : '',
    others: Math.max(0, Number(s.others) || 0),
  }
}

// A session's row in the list she opens under the pointer: whose, where it
// is (or how it ended), and its clock. { id, mood, name, what, clock }.
function rowOf(lang: string, s: Session, { detailed = true, now = Date.now() } = {}) {
  const mood = s.mood || 'idle'
  const name = nameOf(s, detailed) || t(lang, `mood.${mood}`)
  const what = mood === 'idle' ? t(lang, 'mood.idle') : isBusy(s) ? status(lang, s, { detailed, now, withClock: false }) : t(lang, `island.${mood}`)
  return { id: s.id || '', mood, name, what: what === name ? '' : what, clock: time(s, now) }
}

// What a session's expanded row shows: the files its turn edited (up to 15)
// and its last few tool steps; null when specifics are off or it has neither.
function detailsOf(lang: string, s: Session | null | undefined, { detailed = true } = {}) {
  if (!detailed || !s) return null
  const files = Array.isArray(s.fileList) ? s.fileList.slice(0, 15) : []
  const totalFiles = Math.max(Number(s.files) || 0, files.length)
  const moreFiles = Math.max(0, totalFiles - files.length)
  const steps = Array.isArray(s.recentSteps)
    ? s.recentSteps.map(x => ({ text: render(lang, x?.step), ok: x?.ok !== false })).filter(x => x.text)
    : []
  if (!files.length && !steps.length) return null
  return { files, moreFiles, totalFiles, steps }
}

export { STEP_CLOCK_MS, clock, time, isEnding, nameOf, stepOf, status, brief, talkOf, rowOf, othersOf, urgentOf, detailsOf }
