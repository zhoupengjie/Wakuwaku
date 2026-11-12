// Plugins' own views (tier 2: their view.html, docs/PLUGINS.md) in her cards:
// the desktop's medium and large ones (desk.ts) and the capsule's unfolded
// card (island.ts), nowhere else (docs/design/specs/plugins.md §4). The
// card is hers, its glass, corners, buttons and menu; the view's frame (an
// <iframe sandbox="allow-scripts"> at the plugin's own page, addons.rs
// serve) lies over the card's body, and lets the pointer through to the
// card, so the card's own gestures stay as they are. Until the view says it
// is ready the card shows what the widget's data draws (tier 1), and again
// whenever the view is not shown: while nobody is about, while the desktop
// is covered (desk.rs views, as its words are not sent then), while Windows
// has animations off, while the island's watchdog has them taken away
// (addons.ts), and for the rest of the session once a view did not load or
// did not answer twice in a row.
//
// What a view shows: its widget's view (waku.widget.set's view, up to 32 KB)
// is not in every widgets' event; they carry its revision (viewRev), and
// the view is asked of her (addon_view data) when that changes, for the
// frames showing it alone. Its frame is given it at once in its name, with
// her theme, language and the card's colours and fonts (the SDK reads it
// before the view's own scripts run), then by messages. What a view sends
// (waku.send) goes to her (addon_view send) in the name of the widget whose
// card this frame is, known by the frame (event.source), never by what the
// message says; she tells the island's page, whose host hands it to main.js.
//
// Each frame is pinged every 5 s while any shows, its answer wanted within
// 3 s. No frame, no timer.
//
// createViews is plain logic over what it is given, for the tests
// (test/views.test.js); boot() makes a page's.
const PING_EVERY = 5000
const ANSWER_MS = 3000
// A frame not ready this long after it was made: its view did not load.
const LOAD_MS = 6000
// Missed in a row so many times: gone for the session.
const MISSES = 2
// What a view may send its main.js: so many a second, so big as JSON.
const SENDS = 10
const SEND_MAX = 64 * 1024
const LOGS = 20

const json = (v: unknown) => {
  try {
    return JSON.stringify(v) ?? null
  } catch {
    return null
  }
}

// Whether a widget is one a view draws at that size: a plugin's, its
// plugin with a view, a view given (viewRev), medium or large.
// A widget as views read it (widgets.rs, a plugin's: its plugin, its view's revision).
interface ViewWidget {
  id: string
  plugin?: string
  from?: string
  viewRev?: number
}

const viewable = (w: any, size: unknown): w is ViewWidget => !!w && w.from === 'addon' && typeof w.plugin === 'string' && Number.isFinite(w.viewRev) && (size === 'm' || size === 'l')

// deps: ask(widget) → a promise of { page, data, rev } (or { error });
// send(widget, message); make(name, src) → { el, win } a frame not yet
// put anywhere (win: its window once it is in the page); post(win,
// message); move(card, el) puts a frame into a card; live(el) whether it
// is in the page; styleOf(w) the card's colours and fonts; look() → {
// theme, lang }; changed() the page to draw its cards again; log(line);
// now(); the timers. canShow: whether this page can keep a frame loaded
// where it draws it (the island's, drawn anew each second, needs
// moveBefore).
// Those deps, as createViews calls them: a frame's el and win are the
// page's iframe and its window, or the tests' fakes.
interface ViewDeps {
  ask: (id: string) => any
  send: (id: string, message: any) => unknown
  make: (name: string, src: string) => { el: any; win: any }
  post: (win: any, message: any) => void
  move: (card: any, el: any) => unknown
  live?: (el: any) => boolean
  styleOf?: (w: any) => any
  look?: () => { theme: string; lang: string }
  changed?: () => void
  log?: (line: string) => void
  now?: () => number
  setTimer?: (fn: () => void, ms: number) => any
  clearTimer?: (timer: any) => void
  canShow?: boolean
}
// A frame of a card's view, and what is kept of it.
interface Frame {
  key: string
  widget: string
  plugin?: string
  size: string
  made: { el: any; win: any }
  el: any
  rev: unknown
  ready: boolean
  answered: number
  misses: number
  sends: number[]
  logs: number[]
  loadTimer: any
  look: string | null
  style: string | null
  fetching: boolean
  placed?: boolean
}
// What the view's page and data come back as (ask).
interface Got {
  page: string
  data?: unknown
  rev?: unknown
  error?: unknown
}
function createViews({ ask, send, make, post, move, live = () => true, styleOf = () => ({}), look = () => ({ theme: 'dark', lang: 'zh' }), changed = () => {}, log = () => {}, now = () => Date.now(), setTimer = setTimeout, clearTimer = clearTimeout, canShow = true }: ViewDeps) {
  const frames = new Map<string, Frame>()
  // The plugins whose view failed here, for the session.
  const failed = new Set<unknown>()
  // The view's page and data being asked for a card, by key.
  const asking = new Map<string, boolean>()
  const state = { on: true, reduced: false, paused: false }
  let stamp = 0
  let pings = 0
  let pingTimer: any = 0
  let checkTimer: any = 0

  // Whether views may show here now, and this one.
  const allowed = () => state.on && !state.reduced && !state.paused && canShow
  const mayShow = (w: ViewWidget | null | undefined, size: string): w is ViewWidget => allowed() && viewable(w, size) && !failed.has(w.plugin)
  const bump = () => {
    stamp++
    changed()
  }
  const winOf = (f: Frame) => f.made.win

  function drop(f: Frame) {
    frames.delete(f.key)
    clearTimer(f.loadTimer)
    f.el.remove()
    keepWatch()
  }

  // A card's view (w: its widget, none for one that may not have one):
  // its frame put in `card` and true, or false: the card draws its
  // widget's data (and any frame of it before goes). A frame is made once
  // the view's page and data are had (asked for here; the page drawn again
  // then: changed).
  function place(card: any, key: string, w: ViewWidget | null | undefined, size: string) {
    const f = frames.get(key)
    if (!mayShow(w, size)) {
      if (f) drop(f)
      return false
    }
    // Another widget or size there now, or its card drawn anew without it
    // (put in one before, out of the page now): a frame of its own again.
    if (f && (f.widget !== w.id || f.size !== size || (f.placed && !live(f.el) && live(card)))) drop(f)
    const kept = frames.get(key)
    if (kept) {
      if (kept.el.parentElement !== card) move(card, kept.el)
      kept.placed = true
      return true
    }
    if (!asking.has(key)) {
      const plugin = w.plugin
      asking.set(key, true)
      Promise.resolve()
        .then(() => ask(w.id))
        .then(got => {
          asking.delete(key)
          if (!got || typeof got !== 'object' || got.error || typeof got.page !== 'string') return
          if (frames.has(key) || failed.has(plugin) || !allowed()) return
          open(key, w, size, got)
          bump()
        })
        .catch(() => asking.delete(key))
    }
    return false
  }

  // Its frame made: the page given in its name what it shows and how.
  function open(key: string, w: ViewWidget, size: string, got: Got) {
    const { theme, lang } = look()
    const style = styleOf(w)
    const name = json({ waku: 1, id: w.plugin, size, data: got.data ?? null, theme, lang, style }) || '{}'
    const made = make(name, got.page)
    const f: Frame = { key, widget: w.id, plugin: w.plugin, size, made, el: made.el, rev: got.rev, ready: false, answered: 0, misses: 0, sends: [], logs: [], loadTimer: 0, look: json({ theme, lang }), style: json(style), fetching: false }
    f.loadTimer = setTimer(() => {
      if (frames.get(key) === f && !f.ready) fail(f, 'did not load')
    }, LOAD_MS)
    frames.set(key, f)
    keepWatch()
    return f
  }

  function fail(f: Frame, why: string) {
    log(`plugin ${f.plugin}: its view ${why}: its data drawn instead`)
    failed.add(f.plugin)
    for (const x of [...frames.values()]) if (x.plugin === f.plugin) drop(x)
    bump()
  }

  // Each frame's widget as the page has it now (each widgets' event): a
  // new revision of its view asked for and handed to it; its colour told
  // if it changed.
  function widgets(all: unknown) {
    const list = Array.isArray(all) ? all : []
    for (const f of frames.values()) {
      const w = list.find(x => x?.id === f.widget)
      if (!w) continue
      restyle(f, w)
      if (!Number.isFinite(w.viewRev) || w.viewRev === f.rev || f.fetching) continue
      f.fetching = true
      Promise.resolve()
        .then(() => ask(f.widget))
        .then(got => {
          f.fetching = false
          if (frames.get(f.key) !== f || !got || got.error) return
          f.rev = got.rev
          post(winOf(f), { waku: 1, t: 'event', what: 'data', value: got.data ?? null })
        })
        .catch(() => (f.fetching = false))
    }
  }

  // Her theme and language as they are now (the cards' colours with
  // them), told the frames where they changed.
  function relook(all: any[] = []) {
    const { theme, lang } = look()
    for (const f of frames.values()) {
      const w = (Array.isArray(all) ? all : []).find(x => x?.id === f.widget) || { id: f.widget, plugin: f.plugin, from: 'addon' }
      const was = JSON.parse(f.look!)
      const style = styleOf(w)
      if (was.theme !== theme) post(winOf(f), { waku: 1, t: 'event', what: 'theme', value: theme, style })
      else restyle(f, w)
      if (was.lang !== lang) post(winOf(f), { waku: 1, t: 'event', what: 'lang', value: lang })
      f.look = json({ theme, lang })
      f.style = json(style)
    }
  }
  function restyle(f: Frame, w: ViewWidget) {
    const style = styleOf(w)
    const styled = json(style)
    if (styled === f.style) return
    f.style = styled
    post(winOf(f), { waku: 1, t: 'event', what: 'style', style })
  }

  // Shown here or not (desk:views; nobody about), Windows' animations,
  // the watchdog's while: on any change the cards drawn again (their
  // frames gone, or made).
  function set(patch: { on?: boolean; reduced?: boolean; paused?: boolean } = {}) {
    let any = false
    for (const k of ['on', 'reduced', 'paused'] as const) {
      if (typeof patch[k] !== 'boolean' || state[k] === patch[k]) continue
      state[k] = patch[k]!
      any = true
    }
    if (!any) return
    if (!allowed()) for (const f of [...frames.values()]) drop(f)
    for (const f of frames.values()) f.misses = 0
    bump()
  }

  // A message from a frame: the card's view whose frame sent it, or nobody's.
  function receive(source: unknown, data: any) {
    if (!source) return false
    const f = [...frames.values()].find(x => winOf(x) === source)
    if (!f || !data || typeof data !== 'object' || data.waku !== 1 || typeof data.t !== 'string') return false
    switch (data.t) {
      case 'ready':
        if (!f.ready) {
          f.ready = true
          clearTimer(f.loadTimer)
          f.el.classList.add('ready')
        }
        break
      case 'pong':
        if (Number.isFinite(data.n)) f.answered = Math.max(f.answered, data.n)
        break
      case 'send': {
        const t = now()
        f.sends = f.sends.filter(at => t - at < 1000)
        const text = json(data.message ?? null)
        if (f.sends.length >= SENDS || text === null || text.length > SEND_MAX) break
        f.sends.push(t)
        Promise.resolve()
          .then(() => send(f.widget, JSON.parse(text)))
          .catch(() => {})
        break
      }
      case 'log':
      case 'error': {
        const t = now()
        f.logs = f.logs.filter(at => t - at < 60_000)
        if (f.logs.length >= LOGS) break
        f.logs.push(t)
        log(`plugin ${f.plugin} view: ${data.t === 'error' ? 'error: ' : ''}${String(data.line ?? '').slice(0, 300)}`)
        break
      }
    }
    return true
  }

  // --- Pings ---------------------------------------------------------------------------------
  function keepWatch() {
    if (!frames.size) {
      clearTimer(pingTimer)
      clearTimer(checkTimer)
      pingTimer = checkTimer = 0
      return
    }
    if (!pingTimer) pingTimer = setTimer(pingAll, PING_EVERY)
  }
  function pingAll() {
    pingTimer = 0
    if (!frames.size) return
    const n = ++pings
    const at = now()
    const asked = [...frames.values()].filter(f => f.ready)
    for (const f of asked) post(winOf(f), { waku: 1, t: 'ping', n })
    if (asked.length) {
      clearTimer(checkTimer)
      checkTimer = setTimer(() => {
        checkTimer = 0
        // Late (the page itself held, the computer asleep), or taken away meanwhile: nothing.
        if (now() - at > ANSWER_MS + 1500 || !allowed()) return
        for (const f of asked) {
          if (frames.get(f.key) !== f) continue
          if (f.answered >= n) f.misses = 0
          else if (++f.misses >= MISSES) return fail(f, 'did not answer')
        }
      }, ANSWER_MS)
    }
    keepWatch()
  }

  // Frames not kept (keep(frame): by default, still in the page).
  function sweep(keep = (f: Frame) => live(f.el)) {
    for (const f of [...frames.values()]) if (!keep(f)) drop(f)
  }

  return {
    place,
    widgets,
    relook,
    set,
    receive,
    sweep,
    pause: (paused: boolean) => set({ paused }),
    frames,
    failed,
    state,
    allowed,
    get stamp() {
      return stamp
    },
    get watching() {
      return !!(pingTimer || checkTimer)
    },
  }
}

// --- In a page ------------------------------------------------------------------------------
// The page's host: its frames made as views' are. A frame moved in the
// page loads its page again, unless moved by moveBefore; the island's
// page, which draws its lines anew each second, parks its frames aside
// before (park) and puts them in the new cards after (mount), and shows
// none where it cannot so. opts: { ask, send, styleOf, look, changed,
// log, redrawn: true for a page that draws its cards anew }.
function boot(opts: Pick<ViewDeps, 'ask' | 'send' | 'styleOf' | 'look' | 'changed' | 'log'> & { redrawn?: boolean }) {
  const canMoveBefore = typeof Element.prototype.moveBefore === 'function'
  const holder = document.createElement('div')
  holder.hidden = true
  holder.className = 'wview-park'
  document.body.appendChild(holder)
  // Moves to make once their card is in the page.
  const later: [any, any][] = []
  const host: ReturnType<typeof createViews> & { park?: () => void; mount?: () => void } = createViews({
    ...opts,
    canShow: !opts.redrawn || canMoveBefore,
    make: (name, src) => {
      const el = document.createElement('iframe')
      el.className = 'wview'
      el.setAttribute('sandbox', 'allow-scripts')
      el.setAttribute('referrerpolicy', 'no-referrer')
      el.setAttribute('aria-hidden', 'true')
      el.tabIndex = -1
      el.name = name
      el.src = src
      return {
        el,
        get win() {
          return el.contentWindow
        },
      }
    },
    live: el => el.isConnected,
    move: (card, el) => {
      if (el.isConnected && !card.isConnected) return later.push([card, el])
      if (el.isConnected && canMoveBefore) card.moveBefore(el, null)
      else card.appendChild(el)
    },
    post: (win, message) => {
      try {
        win?.postMessage(message, '*')
      } catch {}
    },
  })
  window.addEventListener('message', e => host.receive(e.source, e.data))
  const motion = matchMedia('(prefers-reduced-motion: reduce)')
  host.set({ reduced: motion.matches })
  motion.addEventListener?.('change', () => host.set({ reduced: motion.matches }))
  // Before the page takes its old cards away: their frames aside, loaded.
  host.park = () => {
    if (!canMoveBefore) return
    for (const f of host.frames.values()) if (f.el.isConnected && f.el.parentElement !== holder) holder.moveBefore(f.el, null)
  }
  // After it put its new ones in: each frame in its card; those with no
  // card now gone.
  host.mount = () => {
    for (const [card, el] of later.splice(0)) if (card.isConnected && el.isConnected && canMoveBefore) card.moveBefore(el, null)
    host.sweep(f => f.el.isConnected && f.el.parentElement !== holder)
  }
  return host
}

export { createViews, viewable, boot, PING_EVERY, ANSWER_MS, LOAD_MS, MISSES, SENDS }
