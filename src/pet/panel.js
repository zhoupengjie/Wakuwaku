// The prompt panel above the pet: permission prompts, plan approvals and
// questions from Claude Code, answered with a click (or typed). The first
// waiting one is shown; the terminal can still answer it, whichever comes first.
// Codex's permission prompts too (allow or deny): Codex shows its own dialog
// only once the panel hands it over.
//
// Everything shown comes from the tool call, so it is set as text, never HTML.
;(function () {
  const { t } = window.I18n

  // Buttons wake up a moment after a prompt appears, so a click already on
  // its way to where the panel opens cannot answer it.
  const ARM_MS = 600
  const STEP_ARM_MS = 150

  const panel = document.getElementById('panel')
  let lang = 'en'
  let asks = []
  let shownId = null
  let step = 0
  // Per question: ['label', ...] picked, { text } typed, or { number }.
  let picks = []
  // What is typed in the current question's box, kept across redraws.
  let typed = ''
  let armTimer
  let hasKeyboard = false

  const T = (key, vars) => t(lang, key, vars)

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

  function button(label, action, onClick, kind = '') {
    return el('button', { class: `btn ${kind}`, 'data-action': action, disabled: true, onclick: onClick }, label)
  }

  // A box to type in. The window takes the keyboard only once you click it.
  function input(props, onEnter) {
    const box = el('input', { class: 'input', 'data-input': 'answer', value: typed, ...props })
    box.addEventListener('mousedown', () => {
      if (!hasKeyboard) {
        hasKeyboard = true
        window.pet.keyboard(true)
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
      window.pet.keyboard(false)
    }
  }

  function arm(ms) {
    clearTimeout(armTimer)
    armTimer = setTimeout(() => {
      for (const b of panel.querySelectorAll('button')) b.disabled = false
      refreshConfirm()
    }, ms)
  }

  const answer = choice => window.pet.answer(shownId, choice)
  // To the terminal: the prompt is its again, and its window comes to the front.
  const dismiss = () => {
    const session = asks.find(a => a.id === shownId)?.session
    window.pet.dismiss(shownId)
    if (session) window.pet.jump(session)
  }

  function header(title, ask) {
    return el(
      'div',
      { class: 'head' },
      el('span', { class: 'title' }, title),
      ask.from === 'codex' ? el('span', { class: 'chip' }, 'Codex') : null,
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

  function permissionBody(ask) {
    return [
      header(T('panel.needsApproval', { tool: ask.tool }), ask),
      el('pre', { class: 'code' }, ask.summary || ''),
      el(
        'div',
        { class: 'row' },
        button(T('panel.allow'), 'allow', () => answer({ action: 'allow' }), 'primary'),
        ask.always ? button(T('panel.always'), 'always', () => answer({ action: 'always' }), 'always') : null,
        button(T('panel.deny'), 'deny', () => answer({ action: 'deny' }), 'danger'),
      ),
      ask.always ? alwaysNote(ask.always) : null,
    ]
  }

  function planBody(ask) {
    return [
      header(T('panel.planTitle'), ask),
      el('pre', { class: 'code plan' }, ask.summary || T('panel.emptyPlan')),
      el(
        'div',
        { class: 'row' },
        button(T('panel.approve'), 'allow', () => answer({ action: 'allow' }), 'primary'),
        button(T('panel.deny'), 'deny', () => answer({ action: 'deny' }), 'danger'),
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
    const confirm = panel.querySelector('button[data-action="confirm"]')
    if (!ask || ask.kind !== 'question' || !confirm || armTimer === undefined) return
    if (!panel.querySelector('button[data-action="dismiss"]')?.disabled) {
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
    if (!ask.canAnswer) {
      return [header(ask.title || T('panel.questionTitle'), ask), el('div', { class: 'text' }, T('panel.cannotAnswer'))]
    }

    const q = ask.questions[step]
    const isLast = step === ask.questions.length - 1
    const count = ask.questions.length > 1 ? ` (${step + 1}/${ask.questions.length})` : ''
    const confirm = () => button(isLast ? T('panel.confirm') : T('panel.next'), 'confirm', next, 'primary')
    const parts = [
      header(`${q.header || ask.title || T('panel.questionTitle')}${count}`, ask),
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
    // In the island, the island sizes itself and the window around it.
    if (window.Island?.isOn()) return window.Island.changed()
    requestAnimationFrame(() => {
      window.pet.panel(panel.hidden ? null : { width: panel.offsetWidth + 16, height: panel.offsetHeight + 10 })
    })
  }

  // Which page holds the panel: her home's (the island's window), which
  // talks for her; the open settings have it as a banner.
  const ROLE = new URLSearchParams(location.search).get('role') === 'island' ? 'island' : 'pet'
  let settingsOpen = false
  const holdsPanel = () => ROLE === 'island'

  function render(armMs = ARM_MS) {
    const ask = holdsPanel() ? asks[0] : null
    const hadFocus = document.activeElement?.dataset?.input === 'answer'
    panel.replaceChildren()
    document.body.classList.toggle('asking', !!ask)

    if (!ask) {
      panel.hidden = true
      shownId = null
      typed = ''
      letGoOfKeyboard()
      // The island is still under the pointer when its prompt goes.
      if (!window.Island?.isOn()) window.pet.hover(false)
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

    const body = ask.kind === 'question' ? questionBody(ask) : ask.kind === 'plan' ? planBody(ask) : permissionBody(ask)
    const more = asks.length > 1 ? el('span', { class: 'more' }, T('panel.more', { n: asks.length - 1 })) : null
    panel.append(
      ...body.filter(Boolean),
      el(
        'div',
        { class: 'foot' },
        button(ask.kind === 'question' && !ask.canAnswer ? T('panel.gotIt') : T('panel.toTerminal'), 'dismiss', dismiss, 'ghost'),
        more,
      ),
    )
    panel.hidden = false
    arm(armMs)
    report()

    // Typing on: keep the box focused through a redraw.
    if (hadFocus && hasKeyboard) {
      const box = panel.querySelector('[data-input="answer"]')
      if (box) {
        box.focus()
        box.setSelectionRange?.(box.value.length, box.value.length)
      }
    }
  }

  // Typing shows the confirm button for a single-select "other" answer.
  panel.addEventListener('input', e => {
    if (e.target?.dataset?.input !== 'answer') return
    const row = panel.querySelector('button[data-action="confirm"]')?.parentElement
    if (row?.hidden && typed.trim()) {
      row.hidden = false
      report()
    }
  })

  // Main moves the pet within a grown window so she stays put on screen.
  window.pet.onShift(px => document.documentElement.style.setProperty('--shift', `${px}px`))

  window.pet.onUpdate(data => {
    const wasHolding = holdsPanel()
    settingsOpen = data.settingsOpen === true
    if ((data.lang && data.lang !== lang) || wasHolding !== holdsPanel()) {
      lang = data.lang || lang
      render(0)
    }
  })

  window.pet.onAsks(list => {
    asks = Array.isArray(list) ? list : []
    render()
  })

  // The panel takes clicks; the rest of the window lets them through. In the
  // island, the island itself does this for the panel inside it.
  panel.addEventListener('mouseenter', () => window.Island?.isOn() || window.pet.hover(true))
  panel.addEventListener('mouseleave', () => window.Island?.isOn() || window.pet.hover(false))
})()
