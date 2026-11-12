// The waku object of a plugin's main.js and of its view.html (docs/PLUGINS.md).
//
// main.js runs in a hidden iframe of her island's page, sandboxed with
// scripts alone, at http://wakuplugin.localhost/<id>/__waku/main.html
// (addons.rs serve, which sends this file as __waku/sdk.js). This runs
// first: it tells the page holding the frame it is ready, is told the
// plugin's id, version, her language, theme and quiet, its settings and its
// main, sets up `waku`, and then loads the main. Everything else goes by
// messages to that page (src/pet/addons.js), which knows the plugin by this
// frame and asks for it: its widgets, the web, its storage, a nudge, the
// agents and the hardware where its manifest has them. It answers the
// watchdog's pings, with how long the plugin's own long tasks took since.
//
// view.html (tier 2) runs in a card's frame, on the desktop or in the
// capsule (src/pet/views.js), at its own address in the plugin's folder,
// this the first script in its head (addons.rs view_page). There `waku` is
// the view's: its size, the data its widget gave, her theme and language,
// the card's colours and fonts as CSS variables (all there from the start:
// the page holding it gives them in the frame's name), and a way to tell
// main.js something; told anew by messages as they change.
//
// Nothing here is trusted on her side: a plugin may do what it likes with
// this file's objects, and what it is allowed is decided over there.
;(() => {
  'use strict'
  const parent = window.parent
  if (!parent || parent === window) return

  // WebRTC can reach the network past the frame's Content-Security-Policy
  // (connect-src 'none') and past her proxy. The real guard is WebView2
  // started with non-proxied UDP off (addons.rs browser_args); this is a
  // second layer only, and a leaky one, because a child about:blank frame
  // (window[0]) has its own fresh constructors with no contentWindow to
  // reach through. Its residual risk is in docs/PLUGINS.md.
  const RTC = ['RTCPeerConnection', 'webkitRTCPeerConnection', 'RTCDataChannel', 'RTCSessionDescription', 'RTCIceCandidate', 'RTCRtpSender', 'RTCRtpReceiver', 'RTCRtpTransceiver', 'RTCCertificate']
  function blankRtc(win) {
    if (!win) return
    for (const name of RTC) {
      try {
        Object.defineProperty(win, name, { value: undefined, writable: false, configurable: false })
      } catch {}
    }
  }
  blankRtc(window)
  // Every child frame blanked the moment the DOM changes through one of the
  // synchronous paths that connect a frame (so window[0] is blanked before
  // the plugin's next line runs). Not every path is covered — hence a second
  // layer, not the guard.
  function sweepFrames() {
    try {
      for (let i = 0; i < window.length; i++) blankRtc(window[i])
    } catch {}
  }
  if (typeof Node !== 'undefined' && typeof Element !== 'undefined') {
    const hooks = [
      [Node.prototype, ['appendChild', 'insertBefore', 'replaceChild']],
      [Element.prototype, ['append', 'prepend', 'before', 'after', 'replaceWith', 'insertAdjacentElement', 'insertAdjacentHTML']],
    ]
    for (const [proto, names] of hooks) {
      for (const name of names) {
        const orig = proto && proto[name]
        if (typeof orig !== 'function') continue
        try {
          Object.defineProperty(proto, name, { configurable: true, writable: true, value: function (...args) {
            const out = orig.apply(this, args)
            sweepFrames()
            return out
          } })
        } catch {}
      }
    }
    for (const [proto, prop] of [[Element.prototype, 'innerHTML'], [Element.prototype, 'outerHTML']]) {
      const desc = proto && Object.getOwnPropertyDescriptor(proto, prop)
      if (!desc || !desc.set) continue
      try {
        Object.defineProperty(proto, prop, { configurable: true, enumerable: desc.enumerable, get: desc.get, set: function (v) {
          desc.set.call(this, v)
          sweepFrames()
        } })
      } catch {}
    }
  }

  // A view's page is any of its folder's but her first page for main.js.
  const VIEW = !/\/__waku\/main\.html$/.test(location.pathname)
  const tell = (t, args) => parent.postMessage({ waku: 1, t, ...args }, '*')
  // A value as JSON has it (what a message carries, checked over there).
  const plain = v => (v === undefined ? null : JSON.parse(JSON.stringify(v)))

  // --- What goes wrong, told ---------------------------------------------------------------
  let base = ''
  try {
    base = new URL(VIEW ? './' : '../', location.href).href
  } catch {}
  const short = file => String(file || '').replace(base, '')
  function report(err, file, line) {
    const message = err && typeof err === 'object' ? `${err.name || 'Error'}: ${err.message}` : String(err)
    const where = file ? ` (${short(file)}${line ? `:${line}` : ''})` : ''
    tell('error', { line: `${message}${where}`.slice(0, 300) })
  }
  window.addEventListener('error', e => report(e.error || e.message, e.filename, e.lineno))
  window.addEventListener('unhandledrejection', e => report(e.reason))
  const call = (fn, ...args) => {
    try {
      const r = fn(...args)
      if (r && typeof r.then === 'function') r.then(undefined, report)
    } catch (err) {
      report(err)
    }
  }

  function words(args) {
    return args
      .map(a => {
        if (typeof a === 'string') return a
        if (a instanceof Error) return `${a.name}: ${a.message}`
        try {
          return JSON.stringify(a)
        } catch {
          return String(a)
        }
      })
      .join(' ')
  }

  if (VIEW) view()
  else main()

  // --- main.js ---------------------------------------------------------------------------------
  function main() {
    const EVENTS = ['settings', 'lang', 'theme', 'quiet', 'agents', 'hardware', 'view']
    // Asked of the page: a promise of its answer (its value, or an Error
    // with the words of why not).
    const pending = new Map()
    let next = 1
    const ask = (t, args = {}) =>
      new Promise((resolve, reject) => {
        const n = next++
        pending.set(n, { resolve, reject })
        tell(t, { n, ...args })
      })

    // Its own long tasks (the main thread held 50 ms or more by this frame's
    // code: attribution "self"), added up between two pings, for the
    // watchdog's budget (addons.js). Where the browser does not tell them:
    // null, and the pings alone watch it.
    let busy = 0
    let watched = false
    try {
      if (PerformanceObserver.supportedEntryTypes?.includes('longtask')) {
        new PerformanceObserver(list => {
          for (const e of list.getEntries()) if (e.name === 'self') busy += e.duration
        }).observe({ type: 'longtask', buffered: true })
        watched = true
      }
    } catch {}

    // --- Its state, as the page tells it ------------------------------------------------------
    let state = null
    const listeners = new Map(EVENTS.map(e => [e, new Set()]))
    const fire = (what, ...values) => {
      for (const fn of listeners.get(what)) call(fn, ...values)
    }

    // waku.every's timers: each at its interval, three times as long while
    // nobody is about; started over from now as that changes.
    const timers = new Set()
    function arm(t) {
      clearTimeout(t.handle)
      t.handle = setTimeout(
        () => {
          if (t.stopped) return
          arm(t)
          call(t.fn)
        },
        t.secs * 1000 * (state?.quiet ? 3 : 1),
      )
    }
    function every(seconds, fn) {
      if (typeof fn !== 'function') throw new TypeError('waku.every(seconds, fn)')
      const t = { secs: Math.max(1, Number(seconds) || 1), fn, handle: 0, stopped: false }
      timers.add(t)
      arm(t)
      return () => {
        t.stopped = true
        clearTimeout(t.handle)
        timers.delete(t)
      }
    }

    // An answer of waku.fetch, as a fetch's is read: its status, whether ok,
    // its headers (a few), and its text, read as text or JSON.
    function answerOf(r) {
      const text = String(r?.text ?? '')
      return Object.freeze({
        status: Number(r?.status) || 0,
        ok: r?.ok === true,
        headers: Object.freeze({ ...(r?.headers || {}) }),
        text: () => Promise.resolve(text),
        json: () => Promise.resolve().then(() => JSON.parse(text)),
      })
    }
    function headersOf(h) {
      const out = {}
      if (!h || typeof h !== 'object') return out
      const pairs = Array.isArray(h) ? h : typeof h.entries === 'function' && !(h.constructor === Object) ? [...h.entries()] : Object.entries(h)
      for (const [k, v] of pairs) out[String(k)] = String(v)
      return out
    }

    const waku = Object.freeze({
      get id() {
        return state.id
      },
      get version() {
        return state.version
      },
      get lang() {
        return state.lang
      },
      get theme() {
        return state.theme
      },
      get quiet() {
        return state.quiet
      },
      get settings() {
        return state.settings
      },
      widget: Object.freeze({
        // set(data), or set(sub, data) for a second widget.
        set(sub, data) {
          if (typeof sub !== 'string') [sub, data] = ['', sub]
          return ask('widget', { sub, data: plain(data) })
        },
        remove(sub = '') {
          return ask('remove', { sub: String(sub) })
        },
      }),
      every,
      fetch(url, opts = {}) {
        const body = opts.body == null ? null : String(opts.body)
        return ask('fetch', { url: String(url), method: String(opts.method || 'GET'), headers: headersOf(opts.headers), body }).then(answerOf)
      },
      storage: Object.freeze({
        get: key => ask('storage', { op: 'get', key: String(key) }),
        set: (key, value) => ask('storage', { op: 'set', key: String(key), value: plain(value) }),
        remove: key => ask('storage', { op: 'remove', key: String(key) }),
        all: () => ask('storage', { op: 'all' }),
      }),
      agents: Object.freeze({ list: () => ask('agents') }),
      hardware: Object.freeze({ read: () => ask('hardware') }),
      notify: text => ask('notify', { text: String(text ?? '') }),
      // view: fn(message, { sub }), a message of its view.html (waku.send)
      // from the card of its widget `sub` ('' its own).
      on(what, fn) {
        if (!listeners.has(what) || typeof fn !== 'function') throw new TypeError(`waku.on(${EVENTS.join(' | ')}, fn)`)
        listeners.get(what).add(fn)
        if (what === 'agents' || what === 'hardware') tell('listen', { what })
        return () => listeners.get(what).delete(fn)
      },
      log: (...args) => tell('log', { line: words(args).slice(0, 500) }),
    })

    const frozen = v => Object.freeze(plain(v) ?? {})

    window.addEventListener('message', e => {
      if (e.source !== parent) return
      const m = e.data
      if (!m || typeof m !== 'object' || m.waku !== 1) return
      if (m.t === 'init' && !state) {
        state = { id: String(m.id), version: String(m.version), lang: m.lang === 'en' ? 'en' : 'zh', theme: m.theme === 'light' ? 'light' : 'dark', quiet: m.quiet === true, settings: frozen(m.settings) }
        Object.defineProperty(window, 'waku', { value: waku, enumerable: true })
        import(new URL(String(m.main), base).href).catch(err => report(err))
        return
      }
      if (m.t === 'reply') {
        const p = pending.get(m.n)
        if (!p) return
        pending.delete(m.n)
        if (m.ok) p.resolve(m.value)
        else p.reject(new Error(String(m.error)))
        return
      }
      if (m.t === 'ping') {
        tell('pong', { n: m.n, busy: watched ? Math.round(busy) : null })
        busy = 0
        return
      }
      if (m.t !== 'event' || !state || !listeners.has(m.what)) return
      const v = m.value
      if (m.what === 'settings') fire('settings', (state.settings = frozen(v)))
      else if (m.what === 'lang') fire('lang', (state.lang = v === 'en' ? 'en' : 'zh'))
      else if (m.what === 'theme') fire('theme', (state.theme = v === 'light' ? 'light' : 'dark'))
      else if (m.what === 'quiet') {
        state.quiet = v === true
        for (const t of timers) arm(t)
        fire('quiet', state.quiet)
      } else if (m.what === 'view') fire('view', v?.message ?? null, Object.freeze({ sub: typeof v?.sub === 'string' ? v.sub : '' }))
      else fire(m.what, v)
    })

    tell('ready', {})
  }

  // --- view.html -------------------------------------------------------------------------------
  function view() {
    const EVENTS = ['data', 'theme', 'lang']
    // How the page holding it has it now: given in the frame's name (so it
    // is all there before the view's own scripts run), then by messages.
    let given = {}
    try {
      const n = JSON.parse(window.name || '{}')
      if (n && typeof n === 'object' && n.waku === 1) given = n
    } catch {}
    const state = {
      id: String(given.id || ''),
      size: given.size === 'l' ? 'l' : 'm',
      data: plain(given.data),
      theme: given.theme === 'light' ? 'light' : 'dark',
      lang: given.lang === 'en' ? 'en' : 'zh',
    }

    // The card's colours and fonts as CSS variables on :root; its ground
    // clear, nothing round its edge, nothing to scroll (the card is its
    // frame). The view's own styles come after these, and may say otherwise.
    const VARS = { fg: '--waku-fg', muted: '--waku-muted', color: '--waku-color', font: '--waku-font', fontDisplay: '--waku-font-display' }
    function style(s) {
      if (!s || typeof s !== 'object') return
      for (const [k, name] of Object.entries(VARS)) if (typeof s[k] === 'string' && s[k].length <= 300) document.documentElement.style.setProperty(name, s[k])
    }
    style(given.style)
    try {
      const sheet = document.createElement('style')
      sheet.textContent = 'html,body{margin:0;padding:0;background:transparent;overflow:hidden}body{color:var(--waku-fg);font-family:var(--waku-font)}'
      ;(document.head || document.documentElement).appendChild(sheet)
    } catch {}

    const listeners = new Map(EVENTS.map(e => [e, new Set()]))
    const fire = (what, value) => {
      for (const fn of listeners.get(what)) call(fn, value)
    }

    const waku = Object.freeze({
      get id() {
        return state.id
      },
      // 'm' or 'l': the desktop's medium or large card; the capsule's is 'm'.
      get size() {
        return state.size
      },
      // The view its widget gave (waku.widget.set's view), as it was last.
      get data() {
        return state.data
      },
      get theme() {
        return state.theme
      },
      get lang() {
        return state.lang
      },
      // Any JSON to its main.js (waku.on('view')).
      send: message => tell('send', { message: plain(message) }),
      on(what, fn) {
        if (!listeners.has(what) || typeof fn !== 'function') throw new TypeError(`waku.on(${EVENTS.join(' | ')}, fn)`)
        listeners.get(what).add(fn)
        return () => listeners.get(what).delete(fn)
      },
      log: (...args) => tell('log', { line: words(args).slice(0, 500) }),
    })
    Object.defineProperty(window, 'waku', { value: waku, enumerable: true })

    window.addEventListener('message', e => {
      if (e.source !== parent) return
      const m = e.data
      if (!m || typeof m !== 'object' || m.waku !== 1) return
      if (m.t === 'ping') return tell('pong', { n: m.n })
      if (m.t !== 'event') return
      style(m.style)
      if (m.what === 'data') fire('data', (state.data = plain(m.value)))
      else if (m.what === 'theme') fire('theme', (state.theme = m.value === 'light' ? 'light' : 'dark'))
      else if (m.what === 'lang') fire('lang', (state.lang = m.value === 'en' ? 'en' : 'zh'))
    })

    tell('ready', { view: true })
  }
})()
