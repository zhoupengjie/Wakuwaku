// A PermissionRequest hook event the pet can answer: what to show, and the
// hook output for each choice. Pure, for the main process and the tests.
//
// Claude Code shows its own dialog at the same time and takes whichever
// answer comes first, so answering here never blocks the terminal.
const path = require('path')

const { t } = require('../../shared/i18n')

const MAX_SUMMARY = 400
const MAX_ANSWER = 2000

// What a tool call does, in one short text.
function summarize(tool, input) {
  const i = input && typeof input === 'object' ? input : {}
  const pick = () => {
    switch (tool) {
      case 'Bash':
      case 'PowerShell':
        return i.command
      case 'Edit':
      case 'MultiEdit':
      case 'Write':
      case 'Read':
      case 'NotebookEdit':
        return i.file_path || i.notebook_path
      case 'WebFetch':
        return i.url
      case 'WebSearch':
        return i.query
      case 'Glob':
      case 'Grep':
        return i.pattern
      default:
        return undefined
    }
  }
  const text = typeof pick() === 'string' ? pick() : JSON.stringify(i)
  return text.length > MAX_SUMMARY ? `${text.slice(0, MAX_SUMMARY)}…` : text
}

// Only suggestions that add an allowance; never one that removes or denies.
function allowSuggestions(list) {
  return (Array.isArray(list) ? list : []).filter(
    s =>
      (s?.type === 'addRules' && s.behavior === 'allow') ||
      s?.type === 'addDirectories' ||
      (s?.type === 'setMode' && s.mode === 'acceptEdits'),
  )
}

// What an "always allow" would add, as data the page words in its language:
// { rules: ['Bash(npm test:*)'], dirs: [...], modes: [...], where: ['localSettings'] }
function describeAlways(list) {
  const always = { rules: [], dirs: [], modes: [], where: [] }
  for (const s of list) {
    if (s.type === 'addRules') always.rules.push(...(s.rules || []).map(r => (r.ruleContent ? `${r.toolName}(${r.ruleContent})` : r.toolName)))
    if (s.type === 'addDirectories') always.dirs.push(...(s.directories || []))
    if (s.type === 'setMode') always.modes.push(s.mode)
    if (s.destination && !always.where.includes(s.destination)) always.where.push(s.destination)
  }
  return always
}

// How a question is answered: pick options (choice, the default), type a text, or a number.
function kindOf(q) {
  return q?.kind === 'text' || q?.kind === 'number' ? q.kind : 'choice'
}

function isAnswerable(q) {
  const kind = kindOf(q)
  if (kind === 'choice') return Array.isArray(q.options) && q.options.length > 0
  if (kind === 'number') return Number.isFinite(q.min) && Number.isFinite(q.max) && q.min <= q.max
  return true
}

function questionView(q) {
  const kind = kindOf(q)
  const view = {
    kind,
    header: String(q.header || ''),
    question: String(q.question || ''),
    description: String(q.description || ''),
    multiSelect: kind === 'choice' && q.multiSelect === true,
    options: kind === 'choice' ? (q.options || []).map(o => ({ label: String(o.label || ''), description: String(o.description || '') })) : [],
  }
  if (kind === 'text') view.placeholder = String(q.placeholder || '')
  if (kind === 'number') Object.assign(view, { min: q.min, max: q.max, step: q.step ?? 1, defaultValue: q.defaultValue ?? q.min, unit: String(q.unit || '') })
  return view
}

// The view of a PermissionRequest event, or null when it is not one.
function viewOf(e) {
  if (e?.hook_event_name !== 'PermissionRequest' || typeof e.tool_name !== 'string') return null
  const input = e.tool_input && typeof e.tool_input === 'object' ? e.tool_input : {}
  const base = {
    session: e.session_id || '',
    agent: e.agent_id || '',
    project: e.cwd ? path.basename(String(e.cwd)) : '',
    tool: e.tool_name,
  }

  if (e.tool_name === 'AskUserQuestion') {
    const questions = Array.isArray(input.questions) ? input.questions : []
    return {
      ...base,
      kind: 'question',
      title: typeof input.title === 'string' ? input.title : '',
      canAnswer: questions.length > 0 && questions.every(isAnswerable),
      questions: questions.map(questionView),
    }
  }

  if (e.tool_name === 'ExitPlanMode') {
    const plan = typeof input.plan === 'string' ? input.plan : ''
    return { ...base, kind: 'plan', summary: plan.length > MAX_SUMMARY ? `${plan.slice(0, MAX_SUMMARY)}…` : plan }
  }

  const always = allowSuggestions(e.permission_suggestions)
  return {
    ...base,
    kind: 'permission',
    summary: summarize(e.tool_name, input),
    always: always.length ? describeAlways(always) : null,
  }
}

function output(decision) {
  return { hookSpecificOutput: { hookEventName: 'PermissionRequest', decision } }
}

// One question's answer as Claude Code takes it, or null when it does not fit.
//   choice   ['label', ...] picked options, or { text } typed instead ("Other")
//   text     { text }
//   number   { number }
function answerText(q, picked) {
  if (picked && typeof picked === 'object' && !Array.isArray(picked) && 'text' in picked) {
    if (q.kind === 'number') return null
    const text = String(picked.text ?? '').trim()
    return text && text.length <= MAX_ANSWER ? text : null
  }
  if (q.kind === 'number') {
    const n = Number(picked?.number)
    return Number.isFinite(n) && n >= q.min && n <= q.max ? String(n) : null
  }
  if (q.kind !== 'choice' || !Array.isArray(picked) || picked.length === 0) return null
  const labels = q.options.map(o => o.label)
  if (!picked.every(l => labels.includes(l))) return null
  if (!q.multiSelect && picked.length !== 1) return null
  return picked.join(', ')
}

// The hook output for a choice, or null when the choice does not fit the event.
//   { action: 'allow' }                  allow once
//   { action: 'always' }                 allow, adding the suggested rules
//   { action: 'deny' }                   deny
//   { action: 'answer', answers: [...] } AskUserQuestion, one answer per question
// lang: the language of the note a deny leaves for Claude.
function replyFor(e, choice, lang = 'en') {
  const view = viewOf(e)
  if (!view) return null
  const input = e.tool_input && typeof e.tool_input === 'object' ? e.tool_input : {}

  switch (choice?.action) {
    case 'deny':
      return output({ behavior: 'deny', message: t(lang, 'deny.message') })
    case 'allow':
      if (view.kind === 'question') return null
      // A tool that needs the person (ExitPlanMode) only takes an allow that
      // carries its input back.
      return output(view.kind === 'plan' ? { behavior: 'allow', updatedInput: input } : { behavior: 'allow' })
    case 'always': {
      const rules = allowSuggestions(e.permission_suggestions)
      if (view.kind !== 'permission' || !rules.length) return null
      return output({ behavior: 'allow', updatedPermissions: rules })
    }
    case 'answer': {
      if (view.kind !== 'question' || !view.canAnswer || !Array.isArray(choice.answers)) return null
      if (choice.answers.length !== view.questions.length) return null
      const answers = {}
      for (const [n, q] of view.questions.entries()) {
        const text = answerText(q, choice.answers[n])
        if (text === null) return null
        answers[q.question] = text
      }
      return output({ behavior: 'allow', updatedInput: { ...input, answers } })
    }
    default:
      return null
  }
}

module.exports = { viewOf, replyFor, summarize }
