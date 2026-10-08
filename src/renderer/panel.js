// The prompt panel above the pet: permission prompts, plan approvals and
// questions from Claude Code, answered with a click. The first waiting one is
// shown; the terminal can still answer it, whichever comes first.
//
// Everything shown comes from the tool call, so it is set as text, never HTML.
;(function () {
  // Buttons wake up a moment after a prompt appears, so a click already on
  // its way to where the panel opens cannot answer it.
  const ARM_MS = 600
  const STEP_ARM_MS = 150

  const panel = document.getElementById('panel')
  let asks = []
  let shownId = null
  let step = 0
  let picks = []
  let armTimer

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

  function arm(ms) {
    clearTimeout(armTimer)
    armTimer = setTimeout(() => {
      for (const b of panel.querySelectorAll('button[data-arm]')) b.disabled = false
      for (const b of panel.querySelectorAll('button:not([data-arm])')) b.disabled = false
      // A multi-select confirm waits for a pick.
      const confirm = panel.querySelector('button[data-action="confirm"]')
      if (confirm) confirm.disabled = !(picks[step] && picks[step].length)
    }, ms)
  }

  const answer = choice => window.pet.answer(shownId, choice)
  const dismiss = () => window.pet.dismiss(shownId)

  function header(title, ask) {
    return el(
      'div',
      { class: 'head' },
      el('span', { class: 'title' }, title),
      ask.project ? el('span', { class: 'chip' }, ask.project) : null,
    )
  }

  function permissionBody(ask) {
    return [
      header(`${ask.tool} 需要你批准`, ask),
      el('pre', { class: 'code' }, ask.summary || ''),
      el(
        'div',
        { class: 'row' },
        button('允许', 'allow', () => answer({ action: 'allow' }), 'primary'),
        ask.always ? button('以后都允许', 'always', () => answer({ action: 'always' }), 'always') : null,
        button('拒绝', 'deny', () => answer({ action: 'deny' }), 'danger'),
      ),
      ask.always ? el('div', { class: 'note' }, `以后都允许：${ask.always.rules}（${ask.always.where}）`) : null,
    ]
  }

  function planBody(ask) {
    return [
      header('计划等你确认', ask),
      el('pre', { class: 'code plan' }, ask.summary || '（没有内容）'),
      el(
        'div',
        { class: 'row' },
        button('批准', 'allow', () => answer({ action: 'allow' }), 'primary'),
        button('拒绝', 'deny', () => answer({ action: 'deny' }), 'danger'),
      ),
    ]
  }

  function next() {
    const ask = asks[0]
    if (step < ask.questions.length - 1) {
      step += 1
      render(STEP_ARM_MS)
    } else {
      answer({ action: 'answer', answers: picks })
    }
  }

  function questionBody(ask) {
    if (!ask.canAnswer) {
      return [header('Claude 有问题问你', ask), el('div', { class: 'text' }, '这道题需要在终端里回答。')]
    }

    const q = ask.questions[step]
    const count = ask.questions.length > 1 ? `（${step + 1}/${ask.questions.length}）` : ''
    const options = q.options.map(o => {
      const isPicked = (picks[step] || []).includes(o.label)
      return el(
        'button',
        {
          class: `btn option${isPicked ? ' picked' : ''}`,
          'data-action': 'option',
          'data-option': o.label,
          disabled: true,
          title: o.description,
          onclick: () => {
            if (q.multiSelect) {
              const now = picks[step] || []
              picks[step] = now.includes(o.label) ? now.filter(l => l !== o.label) : [...now, o.label]
              render(0)
            } else {
              picks[step] = [o.label]
              next()
            }
          },
        },
        el('span', { class: 'label' }, o.label),
        o.description ? el('span', { class: 'desc' }, o.description) : null,
      )
    })

    return [
      header(q.header ? `${q.header}${count}` : `Claude 有问题问你${count}`, ask),
      el('div', { class: 'text' }, q.question),
      el('div', { class: 'options' }, ...options),
      q.multiSelect ? el('div', { class: 'row' }, button('确定', 'confirm', next, 'primary')) : null,
    ]
  }

  // Tell main how much room the panel needs; null when it is gone.
  function report() {
    requestAnimationFrame(() => {
      window.pet.panel(panel.hidden ? null : { width: panel.offsetWidth + 16, height: panel.offsetHeight + 10 })
    })
  }

  function render(armMs = ARM_MS) {
    const ask = asks[0]
    panel.replaceChildren()
    document.body.classList.toggle('asking', !!ask)

    if (!ask) {
      panel.hidden = true
      shownId = null
      window.pet.hover(false)
      return report()
    }

    if (ask.id !== shownId) {
      shownId = ask.id
      step = 0
      picks = []
      armMs = ARM_MS
    }

    const body = ask.kind === 'question' ? questionBody(ask) : ask.kind === 'plan' ? planBody(ask) : permissionBody(ask)
    const more = asks.length > 1 ? el('span', { class: 'more' }, `还有 ${asks.length - 1} 个`) : null
    panel.append(
      ...body.filter(Boolean),
      el(
        'div',
        { class: 'foot' },
        button(ask.kind === 'question' && !ask.canAnswer ? '知道了' : '去终端处理', 'dismiss', dismiss, 'ghost'),
        more,
      ),
    )
    panel.hidden = false
    arm(armMs)
    report()
  }

  // Main moves the pet within a grown window so she stays put on screen.
  window.pet.onShift(px => document.documentElement.style.setProperty('--shift', `${px}px`))

  window.pet.onAsks(list => {
    asks = Array.isArray(list) ? list : []
    render()
  })

  // The panel takes clicks; the rest of the window lets them through.
  panel.addEventListener('mouseenter', () => window.pet.hover(true))
  panel.addEventListener('mouseleave', () => window.pet.hover(false))
})()
