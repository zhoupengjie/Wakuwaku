// The main window: what is happening now, the pets, how she looks, alerts,
// the link to Claude Code, and about. Drawn from a snapshot main sends; each
// control sends a patch back and the page redraws from the answer.
// Everything from outside (pet names, tool input) is set as text.
;(function () {
  const { t, render: say } = window.I18n
  const api = window.home
  const app = document.getElementById('app')
  const peek = document.getElementById('peek')

  const TABS = ['now', 'pets', 'look', 'alerts', 'claude', 'about']
  const COLOR = { idle: '#9aa4b2', working: '#3d6fe0', waiting: '#e08a1e', done: '#1f9d55', review: '#7c3aed', error: '#e0245e' }

  let snap = null
  let tab = 'now'
  try {
    tab = TABS.includes(localStorage.getItem('tab')) ? localStorage.getItem('tab') : 'now'
  } catch {}
  // Kept across redraws.
  let fetchRef = ''
  let note = null // { text, kind: 'ok' | 'error' | 'busy' }
  let httpOnly = false
  let copied = -1
  const gallery = { sort: 'popular', items: [], page: 0, totalPages: 1, loading: false, error: '', getting: '' }

  const T = (key, vars) => t(snap?.lang || 'en', key, vars)

  function el(tag, props, ...children) {
    const node = document.createElement(tag)
    for (const [key, value] of Object.entries(props || {})) {
      if (value === undefined || value === null) continue
      if (key === 'class') node.className = value
      // Through the CSSOM: the page's CSP refuses style attributes.
      else if (key === 'style') node.style.cssText = value
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

  const card = (title, ...rows) => el('section', { class: 'card' }, title ? el('h2', {}, title) : null, ...rows)

  function check(label, value, onChange, key) {
    return el(
      'label',
      { class: 'check' },
      el('input', { type: 'checkbox', checked: value, 'data-key': key, onchange: e => onChange(e.target.checked) }),
      el('span', {}, label),
    )
  }

  // A switch that looks like a pill.
  function chip(label, value, onChange, key) {
    return el(
      'label',
      { class: `chip${value ? ' on' : ''}` },
      el('input', { type: 'checkbox', checked: value, 'data-key': key, onchange: e => onChange(e.target.checked) }),
      label,
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
      {},
      options.map(([v, text]) =>
        el(
          'label',
          { class: 'radio' },
          el('input', { type: 'radio', name, value: String(v), checked: String(v) === String(value), onchange: () => onChange(v) }),
          el('span', {}, text),
        ),
      ),
    )
  }

  function clock(ms) {
    const s = Math.max(0, Math.floor(ms / 1000))
    const h = Math.floor(s / 3600)
    const m = Math.floor((s % 3600) / 60)
    const sec = String(s % 60).padStart(2, '0')
    return h ? `${h}:${String(m).padStart(2, '0')}:${sec}` : `${m}:${sec}`
  }

  // --- Now -----------------------------------------------------------------------

  function sessionRow(s) {
    const what = [T(`mood.${s.mood}`), say(snap.lang, s.detail)].filter(Boolean).join(' · ')
    const isBusy = s.mood === 'working' || s.mood === 'waiting'
    const when =
      isBusy && s.since ? clock(Date.now() - s.since) : s.took ? T('detail.took', { time: clock(s.took) }) : T('home.ago', { time: clock(Date.now() - s.at) })
    return el(
      'div',
      { class: 'session', 'data-session': s.project || '' },
      el('span', { class: 'dot', style: `background:${COLOR[s.mood]}` }),
      el('span', { class: 'project' }, s.project || '—'),
      el('span', { class: 'what' }, what),
      el('span', { class: 'when', 'data-since': isBusy && s.since ? String(s.since) : undefined }, when),
    )
  }

  function askCard(ask) {
    const answer = choice => api.answer(ask.id, choice)
    if (ask.kind === 'question') return card(T('home.waitingOn', { project: ask.project || ask.tool }), el('p', { class: 'muted' }, T('home.answerOnPet')))
    const buttons = [
      el('button', { class: 'primary', 'data-action': 'ask-allow', onclick: () => answer({ action: 'allow' }) }, ask.kind === 'plan' ? T('panel.approve') : T('panel.allow')),
    ]
    if (ask.always) buttons.push(el('button', { 'data-action': 'ask-always', onclick: () => answer({ action: 'always' }) }, T('panel.always')))
    buttons.push(el('button', { class: 'danger', 'data-action': 'ask-deny', onclick: () => answer({ action: 'deny' }) }, T('panel.deny')))
    buttons.push(el('button', { 'data-action': 'ask-dismiss', onclick: () => api.dismiss(ask.id) }, T('panel.toTerminal')))
    const title = ask.kind === 'plan' ? T('panel.planTitle') : T('panel.needsApproval', { tool: ask.tool })
    return card(`${title}${ask.project ? ` · ${ask.project}` : ''}`, el('pre', {}, ask.summary || T('panel.emptyPlan')), el('div', { class: 'row' }, buttons))
  }

  function connectionLine() {
    const c = snap.connection
    const text = { plugin: T('home.connPlugin'), hooks: T('home.connHooks'), both: T('home.connBoth'), none: T('home.connNone') }[c]
    const cls = c === 'plugin' || c === 'hooks' ? 'ok' : 'warn'
    return el(
      'p',
      { class: `muted ${cls}`, 'data-connection': c },
      text,
      ' · ',
      el('a', { href: '#', onclick: e => (e.preventDefault(), go('claude')) }, T('home.tabClaude')),
    )
  }

  function nowPage() {
    const s = snap.settings
    return [
      card(T('home.sessions'), snap.sessions.length ? snap.sessions.map(sessionRow) : el('p', { class: 'muted' }, T('home.noSessions')), connectionLine()),
      ...snap.asks.slice(0, 1).map(askCard),
      card(
        T('home.quick'),
        el(
          'div',
          { class: 'row' },
          chip(T('home.showPet'), snap.isVisible, on => api.showPet(on).then(next => ((snap = next), draw())), 'show'),
          chip(T('menu.dnd'), s.dnd, on => set({ dnd: on }), 'dnd'),
          chip(T('settings.sound'), s.sound, on => set({ sound: on }), 'sound'),
          chip(T('menu.island'), s.display === 'island', on => set({ display: on ? 'island' : 'pet' }), 'island'),
        ),
      ),
    ]
  }

  // --- Pets ----------------------------------------------------------------------

  async function download(ref) {
    note = { text: T('settings.fetching'), kind: 'busy' }
    draw()
    const res = await api.fetchPet(ref)
    snap = res.snapshot
    note = res.ok
      ? { text: [T('settings.fetched', { name: res.pet.name, author: res.pet.author }), res.warning].filter(Boolean).join('\n'), kind: 'ok' }
      : { text: T('settings.error', { message: res.error }), kind: 'error' }
    if (res.ok) fetchRef = ''
    gallery.getting = ''
    draw()
  }

  async function loadGallery(more = false) {
    if (gallery.loading) return
    gallery.loading = true
    gallery.error = ''
    draw()
    const res = await api.gallery({ page: more ? gallery.page + 1 : 1, sort: gallery.sort })
    gallery.loading = false
    if (res.ok) {
      gallery.items = more ? [...gallery.items, ...res.items] : res.items
      gallery.page = res.page || 1
      gallery.totalPages = res.totalPages || 1
    } else {
      gallery.error = res.error
      gallery.page = gallery.page || -1
    }
    draw()
  }

  function installedPet(p) {
    const isOn = snap.settings.pet === p.id
    const sheetH = p.version === 2 ? 1144 : 936
    return el(
      'div',
      { class: `pet${isOn ? ' on' : ''}`, 'data-pet': p.id },
      el('div', { class: 'thumb', style: `background-image:url("${p.url}");background-size:768px ${sheetH}px` }),
      el('span', { class: 'pname' }, p.name),
      el('span', { class: 'pby' }, p.author),
      isOn ? el('span', { class: 'pby ok' }, T('home.inUse')) : el('button', { 'data-action': 'use', onclick: () => set({ pet: p.id }) }, T('home.use')),
    )
  }

  function galleryPet(p) {
    const have = snap.pets.some(x => x.id === p.id)
    return el(
      'div',
      { class: 'pet', 'data-gallery': p.id },
      p.preview ? el('img', { src: p.preview, alt: p.name, loading: 'lazy' }) : el('div', { class: 'thumb' }),
      el('span', { class: 'pname' }, p.name),
      el('span', { class: 'pby' }, `${p.author} · ♥ ${p.likes}`),
      p.version === 1 ? el('span', { class: 'pby' }, T('home.v1')) : null,
      have
        ? el('span', { class: 'pby ok' }, T('home.installed'))
        : el(
            'button',
            {
              'data-action': 'get',
              disabled: !!gallery.getting,
              onclick: () => {
                gallery.getting = p.id
                download(p.id)
              },
            },
            gallery.getting === p.id ? T('home.getting') : T('home.get'),
          ),
    )
  }

  function petsPage() {
    if (!gallery.page && !gallery.loading) setTimeout(() => loadGallery(), 0)
    return [
      card(T('home.installed'), snap.pets.length ? el('div', { class: 'grid' }, snap.pets.map(installedPet)) : el('p', { class: 'muted' }, T('settings.petsEmpty'))),
      card(
        T('settings.fetch'),
        el(
          'div',
          { class: 'row' },
          el('input', {
            type: 'text',
            class: 'grow',
            'data-key': 'fetch',
            placeholder: T('settings.fetchPlaceholder'),
            value: fetchRef,
            oninput: e => (fetchRef = e.target.value),
            onkeydown: e => e.key === 'Enter' && fetchRef.trim() && download(fetchRef.trim()),
          }),
          el(
            'button',
            { class: 'primary', 'data-action': 'fetch', disabled: note?.kind === 'busy', onclick: () => fetchRef.trim() && download(fetchRef.trim()) },
            T('settings.fetch'),
          ),
        ),
        note ? el('p', { class: `note ${note.kind === 'busy' ? 'muted' : note.kind}`, 'data-note': note.kind }, note.text) : null,
        el('p', { class: 'muted' }, T('settings.fetchNote')),
      ),
      card(
        T('home.gallery'),
        el(
          'div',
          { class: 'row' },
          chip(T('home.popular'), gallery.sort === 'popular', () => ((gallery.sort = 'popular'), loadGallery()), 'sort-popular'),
          chip(T('home.newest'), gallery.sort === 'newest', () => ((gallery.sort = 'newest'), loadGallery()), 'sort-newest'),
          el('span', { class: 'grow' }),
          el('a', { href: '#', onclick: e => (e.preventDefault(), api.open('site')) }, 'codex-pets.net'),
        ),
        gallery.error ? el('p', { class: 'note error' }, T('home.galleryError', { message: gallery.error })) : null,
        el('div', { class: 'grid', 'data-gallery-grid': 'yes' }, gallery.items.map(galleryPet)),
        gallery.loading ? el('p', { class: 'muted' }, '…') : null,
        gallery.page > 0 && gallery.page < gallery.totalPages && !gallery.loading
          ? el('div', { class: 'row' }, el('button', { 'data-action': 'more', onclick: () => loadGallery(true) }, T('home.more')))
          : null,
      ),
    ]
  }

  // --- Look, alerts --------------------------------------------------------------

  function lookPage() {
    const s = snap.settings
    return [
      card(
        T('home.display'),
        radios(
          'display',
          s.display,
          [
            ['pet', T('home.displayPet')],
            ['island', T('home.displayIsland')],
          ],
          v => set({ display: v }),
        ),
      ),
      card(
        T('menu.size'),
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
        check(T('menu.bubble'), s.bubble, on => set({ bubble: on }), 'bubble'),
        check(T('menu.walk'), s.walk, on => set({ walk: on }), 'walk'),
        check(T('menu.look'), s.look, on => set({ look: on }), 'look'),
      ),
      card(
        T('home.language'),
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
    ]
  }

  function alertsPage() {
    const s = snap.settings
    return [
      card(
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
        el('p', { class: 'muted' }, T('settings.notify')),
        check(T('settings.notifyWaiting'), s.notify.waiting, on => set({ notify: { ...s.notify, waiting: on } }), 'notify-waiting'),
        check(T('settings.notifyDone'), s.notify.done, on => set({ notify: { ...s.notify, done: on } }), 'notify-done'),
        check(T('settings.notifyError'), s.notify.error, on => set({ notify: { ...s.notify, error: on } }), 'notify-error'),
        check(T('settings.sound'), s.sound, on => set({ sound: on }), 'sound-check'),
      ),
      card(
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
      card(
        T('settings.quiet'),
        check(T('settings.dnd'), s.dnd, on => set({ dnd: on }), 'dnd-check'),
        snap.fullscreenAvailable ? check(T('settings.hideInFullscreen'), s.hideInFullscreen, on => set({ hideInFullscreen: on }), 'fullscreen') : null,
      ),
    ]
  }

  // --- Claude Code -------------------------------------------------------------------

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

  function claudePage() {
    const isPlugin = snap.connection === 'plugin' || snap.connection === 'both'
    const statusClass = { ok: 'ok', httpOnly: 'ok', stale: 'warn', partial: 'warn' }[snap.hooks] || 'muted'
    const colon = snap.lang === 'zh' ? '：' : ': '
    return [
      card(
        T('home.plugin'),
        isPlugin ? el('p', { class: 'note ok', 'data-plugin': 'on' }, `✓ ${T('home.pluginOn')}`) : el('p', { class: 'muted', 'data-plugin': 'off' }, T('home.pluginWhy')),
        el(
          'div',
          { class: 'steps' },
          snap.pluginCommands.map((command, i) =>
            el(
              'div',
              { class: 'row' },
              el('code', {}, command),
              el(
                'button',
                {
                  'data-action': `copy-${i}`,
                  onclick: async () => {
                    await api.copy(command)
                    copied = i
                    draw()
                    setTimeout(() => {
                      copied = -1
                      draw()
                    }, 1500)
                  },
                },
                copied === i ? T('home.copied') : T('home.copy'),
              ),
            ),
          ),
        ),
        el('p', { class: 'muted' }, T('home.pluginStart')),
        check(T('settings.startAtLogin'), snap.loginAtStart, async on => ((snap = await api.login(on)), draw()), 'login'),
        snap.connection === 'both'
          ? el('div', { class: 'row' }, el('span', { class: 'warn grow' }, T('home.connBoth')), el('button', { 'data-action': 'remove-old', onclick: () => hooks('remove') }, T('home.removeOld')))
          : null,
      ),
      card(
        T('home.advanced'),
        el('p', { class: 'muted' }, T('home.advancedWhy')),
        el(
          'div',
          { class: 'row' },
          el('span', { class: `grow ${statusClass}`, 'data-status': snap.hooks }, `${T('settings.hooks')}${colon}${hooksStatusText(snap.hooks)}`),
          el(
            'button',
            { 'data-action': 'install', onclick: () => hooks(httpOnly ? 'install-http' : 'install') },
            snap.hooks === 'stale' || snap.hooks === 'partial' ? T('settings.repair') : snap.hooks === 'missing' ? T('settings.install') : T('settings.reinstall'),
          ),
          snap.hooks !== 'missing' ? el('button', { 'data-action': 'remove', onclick: () => hooks('remove') }, T('settings.remove')) : null,
        ),
        check(T('settings.httpOnly'), httpOnly, on => ((httpOnly = on), draw()), 'httpOnly'),
        el('p', { class: 'muted' }, el('code', {}, snap.settingsFile)),
        note && note.kind === 'error' ? el('p', { class: 'note error' }, note.text) : null,
      ),
    ]
  }

  function aboutPage() {
    return [
      card(
        'Wakuwaku',
        el('p', {}, T('settings.version', { version: snap.version })),
        el('p', { class: 'muted' }, T('home.credits')),
        el('p', {}, el('a', { href: '#', onclick: e => (e.preventDefault(), api.open('repo')) }, T('home.repo'))),
      ),
    ]
  }

  // --- The page ------------------------------------------------------------------------

  function go(next) {
    tab = next
    note = null
    try {
      localStorage.setItem('tab', tab)
    } catch {}
    draw()
  }

  // Your pet, peeking: the first idle frame, one and a half times her size.
  function drawPeek() {
    const p = snap.pets.find(x => x.id === snap.settings.pet)
    if (p) {
      peek.className = ''
      peek.style.backgroundImage = `url("${p.url}")`
      peek.style.backgroundSize = `${1536 * 1.5}px ${(p.version === 2 ? 2288 : 1872) * 1.5}px`
      peek.style.backgroundPosition = '0 0'
    } else {
      peek.className = 'face'
      peek.style.backgroundImage = 'url("../assets/faces/idle.png")'
      peek.style.backgroundSize = 'contain'
    }
  }

  function draw() {
    if (!snap) return
    const s = snap.settings
    document.documentElement.lang = snap.lang === 'zh' ? 'zh-CN' : 'en'
    const scrollY = window.scrollY
    drawPeek()

    const welcome = !s.onboarded
      ? el(
          'section',
          { class: 'card welcome' },
          el('p', {}, T('settings.welcome')),
          el('button', { class: 'primary', 'data-action': 'onboarded', onclick: () => set({ onboarded: true }) }, T('settings.done')),
        )
      : null

    const pages = { now: nowPage, pets: petsPage, look: lookPage, alerts: alertsPage, claude: claudePage, about: aboutPage }
    app.replaceChildren(
      el(
        'header',
        {},
        el('img', { class: 'logo', src: `../assets/faces/${snap.now.mood}.png`, alt: '' }),
        el('span', { class: 'name' }, 'Wakuwaku'),
        el('span', { class: 'version' }, snap.version),
        el(
          'nav',
          {},
          TABS.map(name => el('button', { class: name === tab ? 'on' : '', 'data-tab': name, onclick: () => go(name) }, T(`home.tab${name[0].toUpperCase()}${name.slice(1)}`))),
        ),
      ),
      welcome,
      ...pages[tab](),
    )
    window.scrollTo(0, scrollY)
  }

  // The clocks in the session list tick without a redraw.
  setInterval(() => {
    for (const node of document.querySelectorAll('[data-since]')) node.textContent = clock(Date.now() - Number(node.dataset.since))
  }, 1000)

  api.onChanged(next => {
    snap = next
    // Typing in a box: do not redraw under the cursor.
    if (document.activeElement?.tagName === 'INPUT' && document.activeElement.type === 'text') return
    draw()
  })

  api.get().then(first => {
    snap = first
    draw()
  })
})()
