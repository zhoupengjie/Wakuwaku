// A PermissionRequest hook event the pet can answer: what to show, and the
// hook output for each choice. Pure, for the main process and the tests.
//
// Claude Code shows its own dialog at the same time and takes whichever
// answer comes first, so answering here never blocks the terminal.
const path = require('path')

const MAX_SUMMARY = 400

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

// What an "always allow" would add, as the terminal says it: Bash(npm test:*).
function describeSuggestion(s) {
  switch (s?.type) {
    case 'addRules':
    case 'replaceRules':
      return (s.rules || []).map(r => (r.ruleContent ? `${r.toolName}(${r.ruleContent})` : r.toolName)).join('、')
    case 'addDirectories':
      return `访问目录 ${(s.directories || []).join('、')}`
    case 'setMode':
      return `切换到 ${s.mode} 模式`
    default:
      return ''
  }
}

const WHERE = {
  session: '本次会话',
  localSettings: '本项目（仅自己）',
  projectSettings: '本项目',
  userSettings: '所有项目',
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

// A choice question the pet can answer; text and number ones stay in the terminal.
function isChoice(q) {
  return Array.isArray(q?.options) && q.options.length > 0 && (q.type === undefined || q.type === 'choice')
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
      canAnswer: questions.length > 0 && questions.every(isChoice),
      questions: questions.map(q => ({
        header: String(q.header || ''),
        question: String(q.question || ''),
        multiSelect: q.multiSelect === true,
        options: (q.options || []).map(o => ({ label: String(o.label || ''), description: String(o.description || '') })),
      })),
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
    always: always.length
      ? {
          rules: always.map(describeSuggestion).filter(Boolean).join('；'),
          where: [...new Set(always.map(s => WHERE[s.destination] || s.destination))].join('、'),
        }
      : null,
  }
}

function output(decision) {
  return { hookSpecificOutput: { hookEventName: 'PermissionRequest', decision } }
}

// The hook output for a choice, or null when the choice does not fit the event.
//   { action: 'allow' }                     allow once
//   { action: 'always' }                    allow, adding the suggested rules
//   { action: 'deny' }                      deny
//   { action: 'answer', answers: [[...]] }  AskUserQuestion: labels per question
function replyFor(e, choice) {
  const view = viewOf(e)
  if (!view) return null
  const input = e.tool_input && typeof e.tool_input === 'object' ? e.tool_input : {}

  switch (choice?.action) {
    case 'deny':
      return output({ behavior: 'deny', message: '用户在桌宠上拒绝了。' })
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
        const picked = choice.answers[n]
        const labels = q.options.map(o => o.label)
        if (!Array.isArray(picked) || picked.length === 0 || !picked.every(l => labels.includes(l))) return null
        if (!q.multiSelect && picked.length !== 1) return null
        answers[q.question] = picked.join(', ')
      }
      return output({ behavior: 'allow', updatedInput: { ...input, answers } })
    }
    default:
      return null
  }
}

module.exports = { viewOf, replyFor, summarize, describeSuggestion }
