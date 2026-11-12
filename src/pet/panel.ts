// The prompt panel above the pet: permission prompts, plan approvals and
// questions from Claude Code, answered with a click (or typed). The first
// waiting one is shown; the terminal can still answer it, whichever comes first.
// Codex's permission prompts too (allow or deny) and Antigravity's (allow,
// always allow, deny): they show their own dialog only once the panel
// hands it over. And OpenCode's (allow, always, deny),
// which OpenCode asks in its own window all the while; omp's plan reviews
// (approve: it is executed; deny: omp refines it), with omp's own review
// open meanwhile; and ZCode's (allow or deny) beside its own. DeepSeek
// Harness's approvals (allow or deny, with why it asks) and its questions,
// its own dialog open all the while. After them, a code or a link to sign
// in with come by mail (mail/codes.rs): Copy, Open for a link, Ignore.
//
// Everything shown comes from the tool call, so it is set as text, never HTML.
import { t } from '../shared/i18n.ts'
import { HerTalk, Island } from './pages.ts'
import { pet } from './bridge.ts'
import type { Config } from '../shared/config.ts'

// The other agents a prompt may come from, named on it (Claude Code's: none).
const FROM = { codex: 'Codex', agy: 'Antigravity', opencode: 'OpenCode', omp: 'omp', zcode: 'ZCode', dsh: 'DeepSeek Harness' }
// Apps whose own dialog stays open meanwhile: a prompt is handled there.
const IN_APP = new Set(['zcode', 'dsh'])

// Buttons wake up a moment after a prompt appears, so a click already on
// its way to where the panel opens cannot answer it.
const ARM_MS = 600
const STEP_ARM_MS = 150

const panel = typeof document !== 'undefined' ? document.getElementById('panel') : null
let lang = 'en'
let config: Config = {}
let asks = []
// The keys answer what the buttons do: an approval or a plan, theirs too
// (zcode, dsh), whose own dialog closes as the answer reaches it
// (zhoupengjie/Wakuwaku#97). A code come by mail: approve copies it,
// deny lets it go (until it says it is copied).
const isMail = ask => !!ask && (ask.kind === 'mailCode' || ask.kind === 'mailLink')
const canAnswerWithHotkey = ask => !!ask && (ask.kind === 'permission' || ask.kind === 'plan' || (isMail(ask) && !ask.copied)) && ask.canAnswer !== false
const keyHintOf = (key, ask) => (canAnswerWithHotkey(ask) && config.shortcutsEnabled && config[key] ? config[key] : '')
let shownId = null
let step = 0
// Per question: ['label', ...] picked, { text } typed, or { number }.
let picks = []
// What is typed in the current question's box, kept across redraws.
let typed = ''
let armTimer
let hasKeyboard = false

const T = (key, vars?) => t(lang, key, vars)

function el(tag, props, ...children) {
  const node = document.createElement(tag)
  for (const [key, value] of Object.entries(props || {})) {
    if (key === 'class') node.className = value
    else if (key.startsWith('data-')) node.setAttribute(key, value)
    else node[key] = value
  }
  for (const child of children) {
    if (child === null || child === undefined || child === false) continue
    node.append(typeof child === 'string' ? document.createTextNode(child) : child)
  }
  return node
}

function button(label, action, onClick, kind = '', keyHint = '') {
  const hint = keyHint ? el('span', { class: 'hk-hint' }, keyHint) : null
  return el('button', { class: `btn ${kind}`, 'data-action': action, disabled: true, onclick: onClick }, label, hint)
}

// A box to type in. The window takes the keyboard only once you click it.
function input(props, onEnter) {
  const box = el('input', { class: 'input', 'data-input': 'answer', value: typed, ...props })
  box.addEventListener('mousedown', () => {
    if (!hasKeyboard) {
      hasKeyboard = true
      pet.keyboard(true)
      setTimeout(() => box.focus(), 30)
    }
  })
  box.addEventListener('input', () => {
    typed = box.value
    refreshConfirm()
  })
  box.addEventListener('keydown', e => {
    if (e.key === 'Enter') onEnter()
  })
  return box
}

function letGoOfKeyboard() {
  if (hasKeyboard) {
    hasKeyboard = false
    pet.keyboard(false)
  }
}

function arm(ms) {
  clearTimeout(armTimer)
  armTimer = setTimeout(() => {
    for (const b of panel.querySelectorAll<HTMLButtonElement>('button')) b.disabled = false
    refreshConfirm()
  }, ms)
}

const answer = choice => pet.answer(shownId, choice)
// To the terminal: the prompt is its again, and its window comes to the front.
const dismiss = () => {
  const session = asks.find(a => a.id === shownId)?.session
  pet.dismiss(shownId)
  if (session) pet.jump(session)
}

function header(title, ask) {
  return el(
    'div',
    { class: 'head' },
    el('span', { class: 'title' }, title),
    FROM[ask.from] ? el('span', { class: 'chip' }, FROM[ask.from]) : null,
    ask.project ? el('span', { class: 'chip' }, ask.project) : null,
  )
}

function alwaysNote(always) {
  const rules = [
    ...always.rules,
    ...(always.dirs.length ? [T('rule.directories', { dirs: always.dirs.join(', ') })] : []),
    ...always.modes.map(mode => T('rule.mode', { mode })),
  ].join('; ')
  const where = always.where.map(w => T(`where.${w}`)).join(', ')
  return el('div', { class: 'note' }, T('panel.alwaysNote', { rules, where }))
}

// Why the app asks (DeepSeek Harness's: a sandbox to widen, and why), in
// her language when it gave both.
function reasonNote(reason) {
  const text = typeof reason === 'string' ? reason : reason[lang] || reason.en || ''
  return text ? el('div', { class: 'note' }, text) : null
}

function permissionBody(ask) {
  return [
    header(T('panel.needsApproval', { tool: ask.tool }), ask),
    el('pre', { class: 'code' }, ask.summary || ''),
    el(
      'div',
      { class: 'row' },
      button(T('panel.allow'), 'allow', () => answer({ action: 'allow' }), 'primary', keyHintOf('shortcutApprove', ask)),
      ask.always ? button(T('panel.always'), 'always', () => answer({ action: 'always' }), 'always') : null,
      button(T('panel.deny'), 'deny', () => answer({ action: 'deny' }), 'danger', keyHintOf('shortcutDeny', ask)),
    ),
    ask.always ? alwaysNote(ask.always) : null,
    ask.reason ? reasonNote(ask.reason) : null,
  ]
}

// A code or a link to sign in with, come by mail: whose, the code (a
// click on it copies it too) or where the link goes, and Copy, Open (a
// link), Ignore. The clipboard is written by main (the island never has
// the focus a page's own copy needs); copied, it says so and goes.
function mailBody(ask) {
  const isCode = ask.kind === 'mailCode'
  const copy = () => answer({ action: 'allow' })
  const copyBtn = button(T(ask.copied ? 'panel.copied' : 'panel.copy'), 'allow', copy, 'primary', keyHintOf('shortcutApprove', ask))
  return [
    el('div', { class: 'head' }, el('span', { class: 'title' }, T(isCode ? 'panel.mailCode' : 'panel.mailLink')), ask.sender ? el('span', { class: 'chip' }, ask.sender) : null),
    el(
      'pre',
      {
        class: `code ${isCode ? 'mail-code' : 'mail-link'}`,
        title: isCode ? '' : ask.url || '',
        onclick: () => {
          if (!copyBtn.disabled && !ask.copied) copy()
        },
      },
      (isCode ? ask.code : ask.shown || ask.url) || '',
    ),
    el(
      'div',
      { class: 'row' },
      copyBtn,
      isCode ? null : button(T('panel.open'), 'open', () => answer({ action: 'open' }), 'always'),
      ask.copied ? null : button(T('panel.ignore'), 'deny', () => answer({ action: 'deny' }), 'danger', keyHintOf('shortcutDeny', ask)),
    ),
    // Where a link goes, plainly: the address it opens, not what the letter calls it.
    !isCode && ask.host ? el('div', { class: 'note' }, T('panel.mailLinkTo', { host: ask.host })) : null,
  ]
}

function planBody(ask) {
  return [
    header(T('panel.planTitle'), ask),
    el('pre', { class: 'code plan' }, ask.summary || T('panel.emptyPlan')),
    el(
      'div',
      { class: 'row' },
      button(T('panel.approve'), 'allow', () => answer({ action: 'allow' }), 'primary', keyHintOf('shortcutApprove', ask)),
      button(T('panel.deny'), 'deny', () => answer({ action: 'deny' }), 'danger', keyHintOf('shortcutDeny', ask)),
    ),
  ]
}

// The current question's answer from what is picked and typed, or null.
function currentAnswer(q) {
  const text = typed.trim()
  if (q.kind === 'text') return text ? { text } : null
  if (q.kind === 'number') {
    const n = Number(typed)
    return typed !== '' && Number.isFinite(n) && n >= q.min && n <= q.max ? { number: n } : null
  }
  const labels = Array.isArray(picks[step]) ? picks[step] : []
  if (text) return { text: [...labels, text].join(', ') }
  return labels.length ? labels : null
}

function refreshConfirm() {
  const ask = asks[0]
  const confirm = panel.querySelector<HTMLButtonElement>('button[data-action="confirm"]')
  if (!ask || ask.kind !== 'question' || !confirm || armTimer === undefined) return
  if (!panel.querySelector<HTMLButtonElement>('button[data-action="dismiss"]')?.disabled) {
    confirm.disabled = currentAnswer(ask.questions[step]) === null
  }
}

function next() {
  const ask = asks[0]
  const q = ask.questions[step]
  const got = currentAnswer(q)
  if (got === null) return
  picks[step] = got
  typed = ''
  if (step < ask.questions.length - 1) {
    step += 1
    render(STEP_ARM_MS)
  } else {
    answer({ action: 'answer', answers: picks })
  }
}

function questionBody(ask) {
  // Another app's question is named as its (Claude Code's: as Claude's).
  const title = ask.title || (FROM[ask.from] ? T('panel.questionFrom', { app: FROM[ask.from] }) : T('panel.questionTitle'))
  if (!ask.canAnswer) {
    return [header(title, ask), el('div', { class: 'text' }, T('panel.cannotAnswer'))]
  }

  const q = ask.questions[step]
  const isLast = step === ask.questions.length - 1
  const count = ask.questions.length > 1 ? ` (${step + 1}/${ask.questions.length})` : ''
  const confirm = () => button(isLast ? T('panel.confirm') : T('panel.next'), 'confirm', next, 'primary')
  const parts = [
    header(`${q.header || title}${count}`, ask),
    el('div', { class: 'text' }, q.question),
    q.description ? el('div', { class: 'qdesc' }, q.description) : null,
  ]

  if (q.kind === 'text') {
    parts.push(el('div', { class: 'field' }, input({ type: 'text', placeholder: q.placeholder || T('panel.typeAnswer'), maxLength: 2000 }, next)))
    parts.push(el('div', { class: 'row' }, confirm()))
    return parts
  }

  if (q.kind === 'number') {
    if (typed === '' && Number.isFinite(q.defaultValue)) typed = String(q.defaultValue)
    parts.push(
      el(
        'div',
        { class: 'field' },
        input({ type: 'number', min: q.min, max: q.max, step: q.step }, next),
        q.unit ? el('span', { class: 'unit' }, q.unit) : null,
        el('span', { class: 'hint' }, T('panel.numberRange', { min: q.min, max: q.max })),
      ),
    )
    parts.push(el('div', { class: 'row' }, confirm()))
    return parts
  }

  const picked = Array.isArray(picks[step]) ? picks[step] : []
  const options = q.options.map(o =>
    el(
      'button',
      {
        class: `btn option${picked.includes(o.label) ? ' picked' : ''}`,
        'data-action': 'option',
        'data-option': o.label,
        disabled: true,
        title: o.description,
        onclick: () => {
          if (q.multiSelect) {
            picks[step] = picked.includes(o.label) ? picked.filter(l => l !== o.label) : [...picked, o.label]
            render(0)
          } else {
            picks[step] = [o.label]
            typed = ''
            next()
          }
        },
      },
      el('span', { class: 'label' }, o.label),
      o.description ? el('span', { class: 'desc' }, o.description) : null,
    ),
  )
  parts.push(el('div', { class: 'options' }, ...options))
  // "Other": type an answer of your own instead (or, multi-select, as well).
  parts.push(el('div', { class: 'field' }, input({ type: 'text', placeholder: T('panel.other'), maxLength: 2000 }, next)))
  if (q.multiSelect || typed) parts.push(el('div', { class: 'row' }, confirm()))
  else parts.push(el('div', { class: 'row', hidden: true }, confirm()))
  return parts
}

// Tell main how much room the panel needs; null when it is gone.
function report() {
  // In the island, the island sizes itself and the window around it; in
  // hers, she has the room over her head for whatever is there (pet.ts).
  if (Island?.isOn()) return Island.changed()
  if (HerTalk) return HerTalk.room()
  requestAnimationFrame(() => {
    pet.panel(panel.hidden ? null : { width: panel.offsetWidth + 16, height: panel.offsetHeight + 10 })
  })
}

// Which page holds the panel: her home's (the island's window), which
// talks for her; hers while she is out on the desktop and up there, where
// she talks for herself and the home does not open for it (the user's
// call, 2026-10-28). The open settings have it as a banner.
const ROLE = typeof location !== 'undefined'
  ? (new URLSearchParams(location.search).get('role') === 'island' ? 'island' : 'pet')
  : 'island'
let settingsOpen = false
// Her window up on the desktop (shared.rs payload herUp).
let herUp = false
const holdsPanel = () => holderOf({ role: ROLE, out: config.out === true, herUp, hidden: typeof document !== 'undefined' && document.hidden })

// Hers while she is out and her window up (her page not put away with
// it), else the island's: one of the two, so a prompt shows in one place.
function holderOf({ role, out, herUp, hidden }) {
  const hers = out && herUp
  return role === 'island' ? !hers : hers && !hidden
}

function render(armMs = ARM_MS) {
  if (!panel) return
  const ask = holdsPanel() ? asks[0] : null
  const hadFocus = (document.activeElement as HTMLElement)?.dataset?.input === 'answer'
  panel.replaceChildren()
  document.body.classList.toggle('asking', !!ask)

  if (!ask) {
    panel.hidden = true
    shownId = null
    typed = ''
    letGoOfKeyboard()
    // The island is still under the pointer when its prompt goes.
    if (!Island?.isOn()) pet.hover(false)
    return report()
  }

  if (ask.id !== shownId) {
    shownId = ask.id
    step = 0
    picks = []
    typed = ''
    armMs = ARM_MS
    letGoOfKeyboard()
  }

  const body = isMail(ask) ? mailBody(ask) : ask.kind === 'question' ? questionBody(ask) : ask.kind === 'plan' ? planBody(ask) : permissionBody(ask)
  const more = asks.length > 1 ? el('span', { class: 'more' }, T('panel.more', { n: asks.length - 1 })) : null
  panel.append(
    ...body.filter(Boolean),
    // A code come by mail has no terminal to go back to: its Ignore is in its row.
    ...(isMail(ask)
      ? more
        ? [el('div', { class: 'foot' }, more)]
        : []
      : [
          el(
            'div',
            { class: 'foot' },
            // An app's prompt (ZCode's, DSH's dialog stays open meanwhile) is handled there.
            button(ask.kind === 'question' && !ask.canAnswer ? T('panel.gotIt') : IN_APP.has(ask.from) ? T('panel.toApp', { app: FROM[ask.from] }) : T('panel.toTerminal'), 'dismiss', dismiss, 'ghost'),
            more,
          ),
        ]),
  )
  panel.hidden = false
  arm(armMs)
  report()

  // Typing on: keep the box focused through a redraw.
  if (hadFocus && hasKeyboard) {
    const box = panel.querySelector<HTMLInputElement>('[data-input="answer"]')
    if (box) {
      box.focus()
      box.setSelectionRange?.(box.value.length, box.value.length)
    }
  }
}

// Typing shows the confirm button for a single-select "other" answer.
panel?.addEventListener('input', e => {
  if ((e.target as HTMLElement)?.dataset?.input !== 'answer') return
  const row = panel.querySelector('button[data-action="confirm"]')?.parentElement
  if (row?.hidden && typed.trim()) {
    row.hidden = false
    report()
  }
})

if (typeof window !== 'undefined' && pet) {
  // Main moves the pet within a grown window so she stays put on screen.
  pet.onShift?.(px => document.documentElement.style.setProperty('--shift', `${px}px`))

  pet.onUpdate?.(data => {
    const wasHolding = holdsPanel()
    const prevHk = `${config.shortcutsEnabled}:${config.shortcutApprove}:${config.shortcutDeny}`
    settingsOpen = data.settingsOpen === true
    herUp = data.herUp === true
    config = data.config || config
    const nextHk = `${config.shortcutsEnabled}:${config.shortcutApprove}:${config.shortcutDeny}`
    if ((data.lang && data.lang !== lang) || wasHolding !== holdsPanel() || (shownId && prevHk !== nextHk)) {
      lang = data.lang || lang
      render(0)
    }
  })

  pet.onAsks?.(list => {
    asks = Array.isArray(list) ? list : []
    render()
  })

  // Her page put away with her window, or up again: the prompt goes, or
  // comes back over her head.
  if (ROLE === 'pet') document.addEventListener('visibilitychange', () => render(0))
}

function decideHotkeyAction({ which, ask, shownId, isArmed, islandOpen }) {
  if (which === 'island') return { type: 'toggle' }
  if (which !== 'allow' && which !== 'deny') return { type: 'ignore' }
  if (canAnswerWithHotkey(ask) && isArmed && shownId && ask.id === shownId && islandOpen) {
    return { type: 'answer', which, id: shownId }
  }
  return { type: 'open' }
}

if (typeof window !== 'undefined' && pet) {
  pet.onHotkey?.(({ which }: { which?: string } = {}) => {
    // Out on the desktop the prompt is over her head: the keys answer it
    // there (main sends them to her window then, shortcuts.rs), and the
    // island stays shut; the island's own key is the island's alone.
    if (ROLE === 'pet' ? which === 'island' : which !== 'island' && !holdsPanel()) return
    const ask = holdsPanel() ? asks[0] : null
    const actionBtn = panel?.querySelector<HTMLButtonElement>(`button[data-action="${which}"]`)
    const isArmed = !!panel && !panel.hidden && !!actionBtn && actionBtn.disabled === false
    // Over her head it is in sight whenever it is there.
    const islandOpen = ROLE === 'pet' ? !!panel && !panel.hidden : !!Island?.isOpen?.()

    const decision = decideHotkeyAction({ which, ask, shownId, isArmed, islandOpen })
    if (decision.type === 'toggle') {
      Island?.peek?.()
    } else if (decision.type === 'answer') {
      answer({ action: which })
    } else if (decision.type === 'open') {
      if (ROLE === 'island' && !islandOpen) {
        Island?.peek?.(true)
      }
    }
  })
}

// The panel takes clicks; the rest of the window lets them through. In the
// island, the island itself does this for the panel inside it.
// Over her head it is not her: no drag starts there, and it is not her
// being seen (pet.rs hover).
if (panel) {
  panel.addEventListener('mouseenter', () => Island?.isOn() || pet.hover(true, false))
  panel.addEventListener('mouseleave', () => Island?.isOn() || pet.hover(false, false))
}

export { decideHotkeyAction, holderOf, ARM_MS }
