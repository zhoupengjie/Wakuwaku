// The settings window: one page of sections, drawn from a snapshot main sends
// (settings, pets, hooks status...). Each control sends a patch back and the
// page redraws from the answer. Text from outside (pet names) is set as text.
;(function () {
  const { t } = window.I18n
  const api = window.settings
  const app = document.getElementById('app')

  let snap = null
  // Kept across redraws: the download box and what came of the last action.
  let fetchRef = ''
  let note = null // { text, kind: 'ok' | 'error' | 'busy' }
  let httpOnly = false

  const T = (key, vars) => t(snap?.lang || 'en', key, vars)

  function el(tag, props, ...children) {
    const node = document.createElement(tag)
    for (const [key, value] of Object.entries(props || {})) {
      if (key === 'class') node.className = value
      else if (key.startsWith('data-')) node.setAttribute(key, value)
      else if (key.startsWith('on')) node.addEventListener(key.slice(2), value)
      else node[key] = value
    }
    for (const child of children.flat()) {
      if (child === null || child === undefined || child === false) continue
      node.append(typeof child === 'string' ? document.createTextNode(child) : child)
    }
    return node
  }

  async function set(patch) {
    snap = await api.set(patch)
    draw()
  }

  function section(title, ...rows) {
    return el('section', {}, el('h2', {}, title), ...rows)
  }

  function check(label, value, onChange, props = {}) {
    return el(
      'label',
      { class: 'check' },
      el('input', { type: 'checkbox', checked: value, onchange: e => onChange(e.target.checked), ...props }),
      el('span', {}, label),
    )
  }

  function select(label, value, options, onChange, key) {
    return el(
      'label',
      { class: 'field' },
      el('span', { class: 'label' }, label),
      el(
        'select',
        { 'data-key': key, onchange: e => onChange(e.target.value) },
        options.map(([v, text]) => el('option', { value: String(v), selected: String(v) === String(value) }, text)),
      ),
    )
  }

  function radios(name, value, options, onChange) {
    return el(
      'div',
      { class: 'radios' },
      options.map(([v, text, sub]) =>
        el(
          'label',
          { class: 'radio' },
          el('input', { type: 'radio', name, value: String(v), checked: String(v) === String(value), onchange: () => onChange(v) }),
          el('span', {}, text, sub ? el('small', {}, sub) : null),
        ),
      ),
    )
  }

  async function download() {
    const ref = fetchRef.trim()
    if (!ref) return
    note = { text: T('settings.fetching'), kind: 'busy' }
    draw()
    const res = await api.fetchPet(ref)
    snap = res.snapshot
    if (res.ok) {
      fetchRef = ''
      note = { text: [T('settings.fetched', { name: res.pet.name, author: res.pet.author }), res.warning].filter(Boolean).join('\n'), kind: 'ok' }
    } else {
      note = { text: T('settings.error', { message: res.error }), kind: 'error' }
    }
    draw()
  }

  async function hooks(action) {
    const res = await api.hooks(action)
    snap = res.snapshot
    note = res.ok ? null : { text: T('settings.error', { message: res.error || '' }), kind: 'error' }
    draw()
  }

  function hooksStatusText(status) {
    return (
      {
        ok: T('settings.hooksOk'),
        httpOnly: T('settings.hooksHttpOnly'),
        missing: T('settings.hooksMissing'),
        stale: T('settings.hooksStale'),
        partial: T('settings.hooksPartial'),
      }[status] || status
    )
  }

  function draw() {
    if (!snap) return
    const s = snap.settings
    document.documentElement.lang = snap.lang === 'zh' ? 'zh-CN' : 'en'
    document.title = T('settings.title')

    const welcome = !s.onboarded
      ? el(
          'div',
          { class: 'welcome' },
          el('p', {}, T('settings.welcome')),
          el('button', { class: 'primary', 'data-action': 'onboarded', onclick: () => set({ onboarded: true }) }, T('settings.done')),
        )
      : null

    const pets = snap.pets.length
      ? radios(
          'pet',
          s.pet,
          snap.pets.map(p => [p.id, p.name, p.author ? ` · ${p.author}` : '']),
          id => set({ pet: id }),
        )
      : el('p', { class: 'muted' }, T('settings.petsEmpty'))

    const fetchRow = el(
      'div',
      { class: 'row' },
      el('input', {
        type: 'text',
        class: 'grow',
        'data-key': 'fetch',
        placeholder: T('settings.fetchPlaceholder'),
        value: fetchRef,
        oninput: e => (fetchRef = e.target.value),
        onkeydown: e => e.key === 'Enter' && download(),
      }),
      el('button', { class: 'primary', 'data-action': 'fetch', disabled: note?.kind === 'busy', onclick: download }, T('settings.fetch')),
    )

    const statusClass = { ok: 'ok', httpOnly: 'ok', stale: 'warn', partial: 'warn' }[snap.hooks] || 'muted'
    const hooksRow = el(
      'div',
      { class: 'row wrap' },
      el('span', { class: `status ${statusClass}`, 'data-status': snap.hooks }, `${T('settings.hooks')}${snap.lang === 'zh' ? '：' : ': '}${hooksStatusText(snap.hooks)}`),
      el(
        'button',
        { class: 'primary', 'data-action': 'install', onclick: () => hooks(httpOnly ? 'install-http' : 'install') },
        snap.hooks === 'stale' || snap.hooks === 'partial' ? T('settings.repair') : snap.hooks === 'missing' ? T('settings.install') : T('settings.reinstall'),
      ),
      snap.hooks !== 'missing' ? el('button', { 'data-action': 'remove', onclick: () => hooks('remove') }, T('settings.remove')) : null,
    )

    app.replaceChildren(
      el('h1', {}, T('settings.title')),
      welcome,

      section(
        T('settings.pets'),
        pets,
        fetchRow,
        note ? el('p', { class: `note ${note.kind}`, 'data-note': note.kind }, note.text) : null,
        el('p', { class: 'muted' }, T('settings.fetchNote'), ' ', el('a', { href: '#', onclick: e => (e.preventDefault(), api.openSite()) }, T('settings.browse'))),
      ),

      section(
        T('settings.claude'),
        hooksRow,
        check(T('settings.httpOnly'), httpOnly, on => ((httpOnly = on), draw()), { 'data-key': 'httpOnly' }),
        el('p', { class: 'muted path' }, snap.settingsFile),
        check(T('settings.startAtLogin'), snap.loginAtStart, async on => ((snap = await api.login(on)), draw()), { 'data-key': 'login' }),
      ),

      section(
        T('settings.look'),
        radios(
          'scale',
          s.scale,
          [
            [0.4, T('menu.small')],
            [0.55, T('menu.medium')],
            [0.75, T('menu.large')],
          ],
          v => set({ scale: Number(v) }),
        ),
        check(T('menu.bubble'), s.bubble, on => set({ bubble: on })),
        check(T('menu.walk'), s.walk, on => set({ walk: on })),
        check(T('menu.look'), s.look, on => set({ look: on })),
      ),

      section(
        T('settings.alerts'),
        select(
          T('settings.hold'),
          s.hold,
          [
            ['seen', T('settings.holdSeen')],
            [8, T('settings.holdSeconds', { n: 8 })],
            [30, T('settings.holdSeconds', { n: 30 })],
            [120, T('settings.holdSeconds', { n: 120 })],
          ],
          v => set({ hold: v === 'seen' ? 'seen' : Number(v) }),
          'hold',
        ),
        el('div', { class: 'label' }, T('settings.notify')),
        check(T('settings.notifyWaiting'), s.notify.waiting, on => set({ notify: { ...s.notify, waiting: on } }), { 'data-key': 'notify-waiting' }),
        check(T('settings.notifyDone'), s.notify.done, on => set({ notify: { ...s.notify, done: on } }), { 'data-key': 'notify-done' }),
        check(T('settings.notifyError'), s.notify.error, on => set({ notify: { ...s.notify, error: on } }), { 'data-key': 'notify-error' }),
        check(T('settings.sound'), s.sound, on => set({ sound: on }), { 'data-key': 'sound' }),
      ),

      section(
        T('settings.prompts'),
        select(
          T('settings.promptWait'),
          s.promptWaitSec,
          [
            [30, T('settings.seconds', { n: 30 })],
            [60, T('settings.minutes', { n: 1 })],
            [120, T('settings.minutes', { n: 2 })],
            [290, T('settings.minutes', { n: 5 })],
          ],
          v => set({ promptWaitSec: Number(v) }),
          'promptWait',
        ),
        el('p', { class: 'muted' }, T('settings.promptWaitNote')),
      ),

      section(
        T('settings.quiet'),
        check(T('settings.dnd'), s.dnd, on => set({ dnd: on }), { 'data-key': 'dnd' }),
        check(T('settings.hideInFullscreen'), s.hideInFullscreen, on => set({ hideInFullscreen: on }), {
          'data-key': 'fullscreen',
          disabled: !snap.fullscreenAvailable,
        }),
      ),

      section(
        T('settings.language'),
        select(
          T('settings.language'),
          s.lang,
          [
            ['auto', T('settings.languageAuto')],
            ['zh', '中文'],
            ['en', 'English'],
          ],
          v => set({ lang: v }),
          'lang',
        ),
      ),

      section(T('settings.about'), el('p', { class: 'muted' }, T('settings.version', { version: snap.version }))),
    )
  }

  api.onChanged(next => {
    snap = next
    draw()
  })

  api.get().then(first => {
    snap = first
    draw()
  })
})()
