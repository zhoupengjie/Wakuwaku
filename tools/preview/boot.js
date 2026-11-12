// Her real page, run with the stand-in for Tauri, set to a scene and shot
// (page.html?role=island|desk&scene=<id>&mode=dark|light), or, without a
// scene, played with in the playground (page.html?role=island&play=1: its
// world the playground's, nothing held still or frozen; ?still=1 holds the
// transitions at their end there too, for headless shots). In the head of
// page.html, after fake-tauri.js, fixtures.js and scenes.js:
//
// 1. The page's own file (dist/pet/index.html, desk.html) is read and written
//    into this document as it is, but for its Content-Security-Policy, its
//    links made absolute, and a style that holds transitions at their end
//    (headless Edge leaves them at their start). Its scripts run as they
//    would there, in order, before DOMContentLoaded.
// 2. Once loaded, the scene's world goes to the page (Fake.push), and its
//    steps run (window.Preview: wait, hover, click, context, emit...).
// 3. Then the page is frozen: animations run to their end (or held at the
//    scene's moment), every timer cleared and none taken, so nothing moves
//    before the shot. The page holding it (shot.html) is told, with any error.
;(function () {
  const Q = new URLSearchParams(location.search)
  const role = Q.get('role') || 'island'
  const mode = Q.get('mode') === 'light' ? 'light' : 'dark'
  const scene = window.Scenes.byId(Q.get('scene') || '') || null
  const FILES = { island: 'index.html', desk: 'desk.html', bar: 'bar.html' }
  const src = new URL(FILES[role] || FILES.island, window.Fake.PAGES).href
  // Transitions held at their end: in the gallery's shots, or when asked.
  const holdStill = !!scene || Q.get('still') === '1'
  const errors = []
  const tell = message => window.parent !== window && window.parent.postMessage({ wk: true, ...message }, '*')

  window.addEventListener('error', e => errors.push(`${e.message} @ ${(e.filename || '').split('/').pop()}:${e.lineno}`))
  window.addEventListener('unhandledrejection', e => errors.push(`unhandled: ${e.reason?.stack || e.reason}`))

  // --- The page, written in -------------------------------------------------------------
  function pageHTML() {
    const xhr = new XMLHttpRequest()
    xhr.open('GET', src, false)
    try {
      xhr.send()
    } catch (err) {
      return `<body><pre class="preview-error">Could not read ${src} (${err.message}).\nOpen it through render.ps1, or in Edge with --allow-file-access-from-files.</pre></body>`
    }
    const doc = new DOMParser().parseFromString(xhr.responseText, 'text/html')
    doc.querySelectorAll('meta[http-equiv]').forEach(m => m.remove())
    for (const [attr, sel] of [['src', 'script[src], img[src]'], ['href', 'link[href]']]) {
      for (const node of doc.querySelectorAll(sel)) node.setAttribute(attr, new URL(node.getAttribute(attr), src).href)
    }
    const head = [...doc.head.children].filter(n => n.tagName !== 'META').map(n => n.outerHTML).join('\n')
    const still = holdStill ? '<style id="preview-still">*, *::before, *::after { transition: none !important; caret-color: transparent !important; }</style>' : ''
    const attrs = [...doc.body.attributes].map(a => ` ${a.name}="${a.value.replace(/"/g, '&quot;')}"`).join('')
    return `${head}\n${still}\n</head><body${attrs}>${doc.body.innerHTML}`
  }

  // --- Helpers for the scenes' steps ----------------------------------------------------
  const $ = sel => (typeof sel === 'string' ? document.querySelector(sel) : sel)
  const needed = sel => {
    const node = $(sel)
    if (!node) throw new Error(`no ${sel}`)
    return node
  }
  const wait = ms => new Promise(resolve => setTimeout(resolve, ms))
  const at = (node, dx, dy) => {
    const r = node.getBoundingClientRect()
    return { clientX: r.left + (dx ?? r.width / 2), clientY: r.top + (dy ?? r.height / 2) }
  }
  let frozen = false

  const P = {
    scene,
    mode,
    get F() {
      return window.Fixtures
    },
    get world() {
      return window.Fake.world
    },
    $,
    $$: sel => [...document.querySelectorAll(sel)],
    wait,
    emit: (channel, payload) => window.Fake.emit(channel, payload),
    set: patch => window.Fake.set(patch),
    push: () => window.Fake.push(),
    // The pointer comes onto it (as bridge.ts plays it for a window that
    // lets clicks through: mouseenter, not bubbling).
    hover(sel) {
      const node = needed(sel)
      const where = at(node)
      node.dispatchEvent(new PointerEvent('pointerenter', { ...where, bubbles: false }))
      node.dispatchEvent(new MouseEvent('mouseenter', { ...where, bubbles: false, view: window }))
    },
    click(sel) {
      const node = needed(sel)
      const where = at(node)
      node.dispatchEvent(new PointerEvent('pointerdown', { ...where, bubbles: true, cancelable: true, button: 0, buttons: 1, pointerId: 1, isPrimary: true }))
      node.dispatchEvent(new PointerEvent('pointerup', { ...where, bubbles: true, cancelable: true, button: 0, pointerId: 1, isPrimary: true }))
      node.dispatchEvent(new MouseEvent('click', { ...where, bubbles: true, cancelable: true, button: 0, view: window }))
    },
    // A right press: its contextmenu, dx/dy in from its corner.
    context(sel, dx, dy) {
      const node = needed(sel)
      node.dispatchEvent(new MouseEvent('contextmenu', { ...at(node, dx, dy), bubbles: true, cancelable: true, button: 2, view: window }))
    },
    // The settings' controls side by side (the controls scene): taken from
    // the General page as drawn (a switch on and off, a segmented control,
    // a button), an accent one as its newer-version button is, a box as a
    // plugin's settings have, and the Mail page's dropdown.
    specimen() {
      const body = needed('#island-settings .s-body')
      const grab = sel => body.querySelector(sel)?.outerHTML || `<span class="preview-missing">${sel}</span>`
      const on = grab('.sw.on')
      const off = grab('.sw:not(.on)')
      const seg = grab('.seg[data-seg="lang"]')
      const button = grab('.pbtn[data-update]')
      const accent = button.replace('class="pbtn', 'class="pbtn al')
      const row = (title, note, control) => `<div class="r"><div class="grow"><div>${title}</div>${note ? `<div class="d">${note}</div>` : ''}</div>${control}</div>`
      body.innerHTML = `<div class="sec">控件</div><div class="grp">
        ${row('开关', '开着', on)}
        ${row('开关', '关着', off)}
        ${row('分段', '三选一', seg)}
        ${row('按钮', '普通和强调色', `<span class="btns">${button}${accent}</span>`)}
        ${row('输入框', '插件的设置', '<input class="in" value="Dresden" spellcheck="false" style="width:160px">')}
        ${row('下拉', '邮件交给谁读', '<button class="pick" data-mail-agent>Claude Code ▾</button>')}
      </div>`
    },
    freeze,
    // The page's own window (a scene's deskPlay reads window.Desk from it, as in the playground).
    win: window,
  }
  window.Preview = P

  // --- Freezing --------------------------------------------------------------------------
  // flow: the flow light's ring held this many ms into its play, from when
  // it began turning: its animations drawn anew since (a blend, the hold's
  // retimings, island-ring.ts animateRing) begin later, their own time less; its
  // turning, drawn in steps by timers, set to the step showing then
  // (Island.ringAt).
  function freeze({ flow } = {}) {
    if (frozen) return
    frozen = true
    // Nobody there (the world quiet: docs/PERFORMANCE.md, section 2's last
    // row): nothing may be playing endlessly; each that is, an error on the shot.
    if (window.Fake.world.quiet) {
      for (const a of document.getAnimations()) {
        if (a.playState !== 'running' || a.effect?.getComputedTiming?.().iterations !== Infinity) continue
        const el = a.effect.target
        const cls = el && typeof el.className === 'string' ? el.className.trim().split(/\s+/).filter(Boolean).join('.') : ''
        const what = el ? `${el.tagName.toLowerCase()}${el.id ? `#${el.id}` : ''}${cls ? `.${cls}` : ''}` : '?'
        errors.push(`endless animation while quiet: ${a.animationName || a.constructor.name} on ${what}`)
      }
    }
    const ring = document.getElementById('flow')
    const ringAnims = flow != null && ring ? document.getAnimations().filter(a => ring.contains(a.effect?.target)) : []
    // When it began turning, on the timeline: a turning's start and its delay (≤ 0).
    const started = new Map(ringAnims.map(a => [a, a.startTime]))
    const begun = Math.min(...ringAnims.filter(a => a.startTime != null && a.effect?.getKeyframes?.().some(k => k.transform)).map(a => a.startTime + a.effect.getTiming().delay))
    if (flow != null) window.Island?.ringAt?.(flow)
    for (const a of document.getAnimations()) {
      try {
        if (ringAnims.includes(a)) {
          const at = started.get(a)
          a.pause()
          a.currentTime = Number.isFinite(begun) && at != null ? begun + flow - at : flow
          continue
        }
        const end = a.effect?.getComputedTiming?.().endTime
        if (end === Infinity) {
          a.pause()
          a.currentTime = 0
        } else a.finish()
      } catch (err) {
        errors.push(`freeze: ${err.message}`)
      }
    }
    // No timer from here on: nothing changes before the shot.
    const last = setTimeout(() => {}, 0)
    for (let id = 0; id <= last; id++) {
      clearTimeout(id)
      clearInterval(id)
    }
    window.setTimeout = window.setInterval = () => 0
    window.requestAnimationFrame = () => 0
    const all = [...errors, ...window.Fake.errors]
    if (all.length) {
      const pre = document.createElement('pre')
      pre.className = 'preview-error'
      pre.textContent = all.join('\n')
      document.body.append(pre)
    }
    // ?debug=1: what the page heard and asked, over it.
    if (Q.get('debug')) {
      const pre = document.createElement('pre')
      pre.className = 'preview-error preview-debug'
      const heard = [...window.Fake.listeners].map(([ch, fns]) => `${ch}:${fns.size}`).join(' ')
      // ?probe=<selector>,<selector>: where each is, and its inline size.
      const probes = (Q.get('probe') || '').split(',').filter(Boolean).map(sel => {
        const n = document.querySelector(sel)
        if (!n) return `${sel}: none`
        const r = n.getBoundingClientRect()
        return `${sel}: ${Math.round(r.left)},${Math.round(r.top)} ${Math.round(r.width)}x${Math.round(r.height)} style="${n.getAttribute('style') || ''}"`
      })
      pre.textContent = [`body: ${document.body.className}`, `view: ${document.getElementById('island')?.dataset.view}`, `screen: ${screen.width}x${screen.height} (${screen.availHeight}) window: ${innerWidth}x${innerHeight}`, `heard: ${heard}`, `asked: ${[...new Set(window.Fake.calls.map(c => c.cmd))].join(' ')}`, ...probes, ...all].join('\n')
      document.body.append(pre)
    }
    document.documentElement.dataset.frozen = '1'
    tell({ frozen: true, errors: all })
  }

  // --- Running ---------------------------------------------------------------------------
  // Windows' mode, then the shot's world (its scene's, or its own: a scene
  // about one mode says it). Played with (no scene), the world is the
  // playground's as it is.
  if (scene) window.Fake.set({ mode, ...window.Scenes.worldFor(scene, window.Fixtures) })

  window.addEventListener('load', async () => {
    await wait(60)
    // No handover with Windows' own strip here (handoff.ts): hers in sight at once.
    document.documentElement.classList.remove('handoff-wait')
    window.Fake.push()
    if (!scene) return tell({ ready: true, role })
    try {
      // The scene's play is her home's page's; the shot's steps the page shot's.
      const play = role === 'island' ? window.Scenes.playOf(scene) : role === 'desk' ? window.Scenes.deskPlayOf(scene) : null
      if (play) await play(P)
      if (scene.steps && role === window.Scenes.stepsOn(scene)) await scene.steps(P)
    } catch (err) {
      errors.push(`steps: ${err.message}`)
    }
    if (!frozen) {
      await wait(scene?.settle ?? 500)
      freeze()
    }
  })

  const style = document.createElement('style')
  style.textContent = '.preview-error { position: fixed; left: 0; top: 0; right: 0; z-index: 99999; margin: 0; padding: 6px 8px; font: 12px/1.4 Consolas, monospace; color: #fff; background: #b00020; white-space: pre-wrap; } .preview-debug { background: #222; top: auto; bottom: 60px; }'
  document.head.append(style)
  document.write(pageHTML())
})()
