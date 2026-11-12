// Third-party plugins' frames, in her island's page (addons.rs runs the
// rest; docs/PLUGINS.md is the contract). One hidden <iframe
// sandbox="allow-scripts"> for each plugin she says to run (addons_running,
// then addons:changed), at its own first page (__waku/main.html, whose SDK,
// src/addon/sdk.ts, loads its main). A plugin is known here by its frame
// alone: a message counts as one plugin's only when it comes from that
// frame's window (event.source), whatever it says it is, and what it asks
// is asked of her in that plugin's name. Its settings, her language, her
// theme and whether anybody is about are told to it as they change; the
// agents' sessions (from what the page already hears) and the hardware's
// readings only to one whose manifest asks for them, and only once it
// listens; its view's messages (addons:view, from a card's frame on the
// desktop or in the capsule: views.ts) as they come. A widget's words go at
// most once a second, a few requests at once, a few log lines a minute.
//
// The watchdog (docs/design/specs/plugins.md §8), while any runs: each frame
// pinged every 5 s (15 s while nobody is about), the SDK answering with how
// long its own long tasks took since. One that does not answer within 3 s
// is loaded again; one that does not again within ten minutes is stopped
// (hung), as is one over 2 s of long tasks in a minute (busy), and one whose
// page keeps loading again (crashed): stopped in her, until turned on again
// on the Plugins page. All their frames share one renderer process, so one
// plugin held in a loop holds them all: when every one is silent at the same
// ping, no one is blamed for it; all go (and the process with them, the
// cards' views taken away meanwhile, as they may be in it too), then, once
// it has had time to end, start again one at a time, 2 s apart, each
// answering before the next: the one
// after whose start they fall silent again is the one, stopped; the others
// are started again. Found no one (it hangs a while after it starts), the
// next time each is given as long as that took. A check that comes late
// (the page itself held, or the computer asleep) counts for nothing. No
// timer runs while no plugin does.
//
// The host itself (createHost) is plain logic over what it is given, for
// the tests (test/addons.test.js); the page's part is addonsPage(), made
// by main.ts on the island's page alone (the tests import this with no page).
import { pet } from './bridge.ts'
import type { IslandFace } from './island.ts'

const ORIGIN = 'http://wakuplugin.localhost'
// A widget's words sent at most once in so long; requests on the web at
// once; log lines a minute; the agents told at most once in so long; the
// hardware read every so often while one listens (three times as long
// while nobody is about); a message's widget, as JSON, at most; starts
// over in a minute before a frame is left stopped.
const WIDGET_EVERY = 1000
const FETCHES = 4
// Requests of every kind at once for one plugin (storage, remove,
// hardware, notify, fetch, a widget's words): so a storm cannot queue
// unbounded work on her side. Messages a second from one frame, dropped
// over that (before its data is even read). Widgets one plugin may have,
// as many as her side keeps (addons.rs WIDGETS): no more entries made.
const INFLIGHT = 8
const MESSAGES = 60
const WIDGETS = 8
const LOGS = 30
const AGENTS_EVERY = 1000
const HARDWARE_EVERY = 5000
const DATA_MAX = 64 * 1024
const RESTARTS = 3
// The watchdog: a ping so often (three times as long while nobody is
// about), its answer within so long, a check later than that by more than
// so much not counted; a frame not pinged until it has had so long to load;
// two misses within so long stop it; long tasks over so much in a minute
// stop it; frames started again one at a time so far apart (a minute at
// most); and so long after their old frames went. A process held in a
// loop is not ended as its frames go: the next frame made is put in it
// again, held as it is, and only once that one goes too is it ended (on a
// test copy, 2026-10-21, each time; within 1.5 s then). So the first one
// started again not answering is not blamed at once (step).
const PING_EVERY = 5000
const ANSWER_MS = 3000
const LATE_MS = 1500
const GRACE_MS = 3000
const AGAIN_MS = 10 * 60_000
const BUSY_MS = 2000
const STAGGER_MS = 2000
const HUNT_MAX_MS = 60_000
const RELOAD_MS = 4000

const pageOf = id => `${ORIGIN}/${id}/__waku/main.html`
const json = v => {
  try {
    return JSON.stringify(v) ?? null
  } catch {
    return null
  }
}
const str = (v, max = 4096) => String(v ?? '').slice(0, max)

// The agents' sessions as a plugin sees them (waku.agents): each one's id,
// agent, project, mood, since, and the line beside it; no more.
const MOODS = ['idle', 'working', 'waiting', 'done', 'review', 'error']
function agentsOf(list) {
  return (Array.isArray(list) ? list : [])
    .filter(s => s && typeof s === 'object' && typeof s.id === 'string' && s.id)
    .map(s => ({
      id: str(s.id, 80),
      agent: str(s.agent || 'claude', 20),
      project: str(s.project, 80),
      mood: MOODS.includes(s.mood) ? s.mood : 'idle',
      since: Number.isFinite(s.since) ? s.since : null,
      detail: typeof s.detail === 'string' ? s.detail.slice(0, 80) : '',
    }))
}

// deps: invoke(cmd, args) a promise; make(id, src) → { el, win } a frame;
// drop(rec) takes one away; post(win, message); log(line); now(); the
// timers; views(on), the cards' views of this page taken away or back
// (the desktop's are told by her: invoke views).
// Those deps as createHost takes them; a frame's el and win are the page's
// iframe and its window, or a test's stand-ins.
interface HostDeps {
  invoke: (cmd: string, args?: any) => Promise<any>
  make: (id: string, src: string) => { el: any; win: any }
  drop: (rec: any) => void
  post: (win: any, message: any) => void
  log?: (line: string) => void
  now?: () => number
  setTimer?: (fn: () => void, ms: number) => any
  clearTimer?: (timer: any) => void
  views?: (on: boolean) => void
  rand?: () => number
}
function createHost({ invoke, make, drop, post, log = () => {}, now = () => Date.now(), setTimer = setTimeout, clearTimer = clearTimeout, views = () => {}, rand = Math.random }: HostDeps) {
  const recs = new Map()
  const bySource = new Map()
  // Kept stopped (given up on), by id: its rev then, until she sends it changed.
  const held = new Map()
  // The last list she sent, by id.
  const want = new Map()
  const env = { lang: 'zh', theme: 'dark', quiet: false }
  let known = false
  let pending = null
  let agents = []
  let agentsJson = '[]'
  let agentsAt = -Infinity
  let agentsTimer = 0
  let hardwareTimer = 0

  const tell = (rec, what, value) => post(rec.win, { waku: 1, t: 'event', what, value })
  const reply = (rec, n, ok, value) => post(rec.win, ok ? { waku: 1, t: 'reply', n, ok: true, value } : { waku: 1, t: 'reply', n, ok: false, error: String(value?.message ?? value) })
  const refusal = r => (r && typeof r === 'object' && r.error ? String(r.error) : null)
  // Asked of her in its name: { error } a refusal, else its value (pick).
  // One slot of its in-flight budget held until the answer comes, so a
  // plugin cannot have more than INFLIGHT asking at once (whatever the kind).
  function ask(rec, n, cmd, args, pick = r => r ?? null) {
    if (rec.inflight >= INFLIGHT) return reply(rec, n, false, `${INFLIGHT} requests at once at most`)
    rec.inflight++
    return Promise.resolve()
      .then(() => invoke(cmd, { id: rec.id, ...args }))
      .then(
        r => (refusal(r) ? reply(rec, n, false, refusal(r)) : reply(rec, n, true, pick(r))),
        e => reply(rec, n, false, e),
      )
      .finally(() => rec.inflight--)
  }
  const quietly = promise => Promise.resolve(promise).catch(() => {})

  function start(item) {
    const { el, win } = make(item.id, pageOf(item.id))
    const rec = { id: item.id, item, el, win, ready: false, loads: 0, restarts: [], fetches: 0, inflight: 0, logs: [], msgs: [], listens: new Set(), widgets: new Map(), startedAt: now(), ping: null, ponged: null }
    recs.set(item.id, rec)
    bySource.set(win, rec)
    keepWatch()
    return rec
  }
  function stop(rec) {
    recs.delete(rec.id)
    bySource.delete(rec.win)
    for (const w of rec.widgets.values()) clearTimer(w.timer)
    drop(rec)
    keepWatch()
  }

  // The list she sent: those not in it stopped, new ones started, one
  // changed (another version, its manifest edited, other permissions)
  // started over, one whose settings changed told them. While the frames
  // start again one by one (recovering), the new and the changed wait their
  // turn there.
  function apply(list) {
    const items = (Array.isArray(list) ? list : []).filter(i => i && typeof i.id === 'string' && /^[a-z][a-z0-9-]{1,39}$/.test(i.id))
    if (!known) {
      pending = items
      return
    }
    want.clear()
    for (const item of items) want.set(item.id, item)
    for (const rec of [...recs.values()]) if (!want.has(rec.id)) stop(rec)
    for (const [id, rev] of [...held]) if (!want.has(id) || want.get(id).rev !== rev) held.delete(id)
    for (const id of [...trouble.keys()]) if (!want.has(id)) trouble.delete(id)
    if (recovering) recovering.queue = recovering.queue.filter(i => want.has(i.id)).map(i => want.get(i.id))
    for (const item of items) {
      if (held.has(item.id)) continue
      const rec = recs.get(item.id)
      if (!rec) {
        if (recovering) {
          if (!recovering.queue.some(i => i.id === item.id)) recovering.queue.push(item)
        } else start(item)
        continue
      }
      if (rec.item.rev !== item.rev || rec.item.main !== item.main || json(rec.item.perms) !== json(item.perms)) {
        stop(rec)
        if (recovering) recovering.queue.push(item)
        else start(item)
        continue
      }
      const changed = json(rec.item.settings) !== json(item.settings)
      rec.item = item
      if (changed && rec.ready) tell(rec, 'settings', item.settings)
    }
    keepHardware()
  }

  // Her language, theme, quiet as they are now: those changed told.
  function setEnv(patch: { lang?: string; theme?: string; quiet?: boolean } = {}) {
    const changed = []
    for (const key of ['lang', 'theme', 'quiet']) {
      if (!(key in patch) || patch[key] === undefined) continue
      const v = key === 'quiet' ? patch.quiet === true : key === 'lang' ? (patch.lang === 'en' ? 'en' : 'zh') : patch.theme === 'light' ? 'light' : 'dark'
      if (env[key] !== v) {
        env[key] = v
        changed.push(key)
      }
    }
    for (const rec of recs.values()) if (rec.ready) for (const key of changed) tell(rec, key, env[key])
    if (changed.includes('quiet')) {
      keepHardware(true)
      keepWatch(true)
    }
    // Her language known: the frames may start.
    if (!known && 'lang' in patch) {
      known = true
      if (pending) apply(pending)
      pending = null
    }
  }

  // The agents' sessions as the page heard them: told to those that may
  // and listen, when they changed, once a second at most.
  function setAgents(list) {
    const next = agentsOf(list)
    const text = json(next)
    if (text === agentsJson) return
    agents = next
    agentsJson = text
    if (agentsTimer) return
    const wait = agentsAt + AGENTS_EVERY - now()
    const go = () => {
      agentsTimer = 0
      agentsAt = now()
      for (const rec of recs.values()) if (rec.ready && rec.item.perms?.agents && rec.listens.has('agents')) tell(rec, 'agents', agents)
    }
    if (wait <= 0) go()
    else agentsTimer = setTimer(go, wait)
  }

  // The hardware read for those that may and listen, while any does.
  function keepHardware(again = false) {
    const listening = [...recs.values()].filter(r => r.ready && r.item.perms?.hardware && r.listens.has('hardware'))
    if (again || !listening.length) {
      clearTimer(hardwareTimer)
      hardwareTimer = 0
    }
    if (!listening.length || hardwareTimer) return
    hardwareTimer = setTimer(
      () => {
        hardwareTimer = 0
        const now = [...recs.values()].filter(r => r.ready && r.item.perms?.hardware && r.listens.has('hardware'))
        if (!now.length) return
        Promise.resolve()
          .then(() => invoke('hardware', { id: now[0].id }))
          .then(r => {
            if (!refusal(r)) for (const rec of now) if (recs.get(rec.id) === rec) tell(rec, 'hardware', r)
          })
          .catch(() => {})
          .finally(() => keepHardware())
      },
      HARDWARE_EVERY * (env.quiet ? 3 : 1),
    )
  }

  function logLine(rec, line) {
    const t = now()
    rec.logs = rec.logs.filter(at => t - at < 60_000)
    if (rec.logs.length >= LOGS) return false
    rec.logs.push(t)
    log(`plugin ${rec.id}: ${str(line, 500)}`)
    return true
  }

  // A widget's words: sent at once, or once its second is up (the last
  // words then); each asking answered as that send is.
  function widget(rec, m) {
    const sub = typeof m.sub === 'string' ? m.sub : ''
    const text = json(m.data)
    if (text === null || text.length > DATA_MAX) return reply(rec, m.n, false, 'a widget is JSON, up to 64 KB')
    let w = rec.widgets.get(sub)
    // No more entries than her side keeps widgets: a plugin naming numberless
    // subs cannot make the map (and its timers) grow without end.
    if (!w && rec.widgets.size >= WIDGETS) return reply(rec, m.n, false, `${WIDGETS} widgets at most`)
    if (!w) rec.widgets.set(sub, (w = { at: -Infinity, timer: 0, data: null, waiting: [] }))
    w.data = JSON.parse(text)
    w.waiting.push(m.n)
    if (w.timer) return
    const go = () => {
      w.timer = 0
      w.at = now()
      const [data, waiting] = [w.data, w.waiting]
      w.waiting = []
      rec.inflight++
      Promise.resolve()
        .then(() => invoke('widget', { id: rec.id, sub: sub || null, data }))
        .then(
          r => {
            // Her side would not take it (over her own 8): forget the sub,
            // so it is not kept as one she has.
            if (refusal(r) && !w.waiting.length && rec.widgets.get(sub) === w) rec.widgets.delete(sub)
            waiting.forEach(n => (refusal(r) ? reply(rec, n, false, refusal(r)) : reply(rec, n, true, null)))
          },
          e => waiting.forEach(n => reply(rec, n, false, e)),
        )
        .finally(() => rec.inflight--)
    }
    const wait = w.at + WIDGET_EVERY - now()
    if (wait <= 0) go()
    else w.timer = setTimer(go, wait)
  }

  function handle(rec, m) {
    const perms = rec.item.perms || {}
    switch (m.t) {
      case 'ready':
        rec.ready = true
        post(rec.win, { waku: 1, t: 'init', id: rec.id, version: rec.item.version, lang: env.lang, theme: env.theme, quiet: env.quiet, settings: rec.item.settings || {}, main: rec.item.main })
        return
      case 'widget':
        return widget(rec, m)
      case 'remove':
        return ask(rec, m.n, 'remove', { sub: typeof m.sub === 'string' && m.sub ? m.sub : null }, () => null)
      case 'fetch': {
        if (rec.fetches >= FETCHES) return reply(rec, m.n, false, `${FETCHES} requests at once at most`)
        rec.fetches++
        const headers = {}
        if (m.headers && typeof m.headers === 'object') for (const [k, v] of Object.entries(m.headers).slice(0, 32)) headers[str(k, 64)] = str(v)
        const body = m.body == null ? null : String(m.body)
        if (body && body.length > 1 << 20) {
          rec.fetches--
          return reply(rec, m.n, false, 'a body over 1 MB')
        }
        // Refused at once (INFLIGHT asks out already), ask gives no promise:
        // the count comes back down all the same.
        return Promise.resolve(ask(rec, m.n, 'fetch', { url: str(m.url, 8192), method: str(m.method, 16), headers, body })).finally(() => rec.fetches--)
      }
      case 'storage': {
        const op = ['get', 'set', 'remove', 'all'].includes(m.op) ? m.op : 'get'
        const value = op === 'set' ? (json(m.value) === null ? null : m.value) : null
        return ask(rec, m.n, 'storage', { op, key: m.key == null ? null : str(m.key, 200), value }, r => r?.value ?? null)
      }
      case 'agents':
        return perms.agents ? reply(rec, m.n, true, agents) : reply(rec, m.n, false, 'no agents permission')
      case 'hardware':
        return perms.hardware ? ask(rec, m.n, 'hardware', {}) : reply(rec, m.n, false, 'no hardware permission')
      case 'notify':
        return perms.notify ? ask(rec, m.n, 'notify', { text: str(m.text, 200) }, () => null) : reply(rec, m.n, false, 'no notify permission')
      case 'listen':
        if ((m.what === 'agents' && perms.agents) || (m.what === 'hardware' && perms.hardware)) {
          rec.listens.add(m.what)
          if (m.what === 'agents') tell(rec, 'agents', agents)
          keepHardware()
        }
        return
      case 'log':
        logLine(rec, m.line)
        return
      case 'error':
        if (logLine(rec, `error: ${str(m.line, 300)}`)) quietly(invoke('said', { id: rec.id, line: str(m.line, 300) }))
        return
      case 'pong':
        // Only the pong for the nonce she last sent this frame counts: a
        // plugin cannot forge one (a huge number, say) to look answered
        // while it is hung, because it cannot know the next nonce.
        if (m.n === rec.ping) rec.ponged = m.n
        busy(rec, m.busy)
        return
    }
  }

  // A message from a frame: the plugin whose frame sent it, or nobody's.
  // Counted per second before its data is looked at, so a flood of
  // messages is dropped early, one line said the first time in a window.
  function receive(source, data) {
    const rec = bySource.get(source)
    if (!rec || !data || typeof data !== 'object' || data.waku !== 1 || typeof data.t !== 'string') return false
    const t = now()
    rec.msgs = rec.msgs.filter(at => t - at < 1000)
    if (rec.msgs.length >= MESSAGES) {
      if (!rec.flooding) {
        rec.flooding = true
        logLine(rec, `too many messages at once: some dropped (${MESSAGES} a second at most)`)
      }
      return true
    }
    rec.flooding = false
    rec.msgs.push(t)
    handle(rec, data)
    return true
  }

  // A message of a plugin's view (her addons:view: who it is, the view's
  // card known by its frame where it is), to its main.js as it runs.
  function fromView(id, sub, message) {
    const rec = recs.get(id)
    if (!rec?.ready) return false
    tell(rec, 'view', { message: message ?? null, sub: typeof sub === 'string' ? sub : '' })
    return true
  }

  // A frame's page loaded: the first time, as it should; again (sent
  // elsewhere and kept back, or reloaded), it starts over, and one that
  // keeps doing so is stopped (crashed).
  function loaded(id, el) {
    const rec = recs.get(id)
    if (!rec || rec.el !== el) return
    rec.loads++
    if (rec.loads === 1) return
    const t = now()
    const restarts = [...rec.restarts.filter(at => t - at < 60_000), t]
    stop(rec)
    if (restarts.length > RESTARTS) {
      log(`plugin ${id}: loaded its page again ${restarts.length} times in a minute`)
      return giveUp(id, 'crashed', restarts)
    }
    log(`plugin ${id}: its page loaded again: started over`)
    start(rec.item).restarts = restarts
  }

  // --- The watchdog ----------------------------------------------------------------------
  // Each plugin's misses (unanswered pings, by when) and its long tasks
  // ([when, ms]), kept across its frames; the pings' count; the timers.
  const trouble = new Map()
  const tasks = new Map()
  // A fresh unguessable nonce for each ping, so only the frame it was sent
  // to can answer it (not a plugin forging a pong). Whether a frame has
  // answered the nonce she last sent it.
  let seq = 0
  const nonce = () => `${++seq}.${Math.floor(rand() * 0x100000000).toString(36)}`
  const answered = rec => rec.ping != null && rec.ponged === rec.ping
  let pingTimer = 0
  let checkTimer = 0
  let reloadTimer = 0
  // Starting them again one at a time: { queue, started, timer }, or null.
  let recovering = null
  let viewsOn = true

  // The cards' views here and on the desktop: taken away while frames are
  // started again (they may share their process), back after.
  function setViews(on) {
    if (viewsOn === on) return
    viewsOn = on
    views(on)
    quietly(invoke('views', { id: '', on }))
  }

  // The next ping armed while any frame is up (again: from now, its pace
  // changed); none, none.
  function keepWatch(again = false) {
    if (again || !recs.size || recovering) {
      clearTimer(pingTimer)
      pingTimer = 0
    }
    if (!recs.size || recovering) {
      clearTimer(checkTimer)
      checkTimer = 0
      return
    }
    if (!pingTimer) pingTimer = setTimer(pingAll, PING_EVERY * (env.quiet ? 3 : 1))
  }

  // Each frame that has had time to load pinged; their answers looked at
  // in 3 s; the next ping in 5.
  function pingAll() {
    pingTimer = 0
    if (!recs.size || recovering) return
    const at = now()
    const asked = [...recs.values()].filter(rec => at - rec.startedAt >= GRACE_MS)
    for (const rec of asked) {
      rec.ping = nonce()
      post(rec.win, { waku: 1, t: 'ping', n: rec.ping })
    }
    if (asked.length) {
      clearTimer(checkTimer)
      checkTimer = setTimer(() => check(at, asked), ANSWER_MS)
    }
    keepWatch()
  }

  function check(at, asked) {
    checkTimer = 0
    const live = asked.filter(rec => recs.get(rec.id) === rec)
    const missed = live.filter(rec => !answered(rec))
    if (now() - at <= ANSWER_MS + LATE_MS && missed.length) {
      // All those asked silent, and another there (one just started, too
      // soon to be asked, among them): their process held, by any of them.
      if (recs.size >= 2 && missed.length === live.length) {
        recover()
        return
      }
      for (const rec of missed) miss(rec)
    }
    keepWatch()
  }

  // A miss, kept; the second within ten minutes stops it (hung), the
  // first loads it again.
  function missed(id) {
    const t = now()
    const times = [...(trouble.get(id) || []).filter(at => t - at < AGAIN_MS), t]
    trouble.set(id, times)
    return times
  }
  function miss(rec) {
    const times = missed(rec.id)
    if (times.length >= 2) return giveUp(rec.id, 'hung', times)
    log(`plugin ${rec.id}: did not answer: loaded again`)
    reload(rec)
  }
  // Its frame taken away, the views with it; started again a moment later.
  function reload(rec) {
    const item = rec.item
    stop(rec)
    setViews(false)
    clearTimer(reloadTimer)
    reloadTimer = setTimer(() => {
      reloadTimer = 0
      const now = want.get(item.id)
      if (now && !held.has(item.id) && !recs.has(item.id) && !recovering) start(now)
      if (!recovering) setViews(true)
    }, RELOAD_MS)
  }

  // Stopped in her (stop: why, since its first miss, how many), its frame
  // gone, not started again here until she sends it changed.
  function giveUp(id, why, times = []) {
    const rec = recs.get(id)
    if (rec) stop(rec)
    held.set(id, want.get(id)?.rev ?? rec?.item.rev)
    trouble.delete(id)
    tasks.delete(id)
    log(`plugin ${id}: stopped (${why})`)
    quietly(invoke('stop', { id, why, since: times[0] ?? now(), times: why === 'hung' ? times.length : 0 }))
  }

  // Its own long tasks since the last ping: over 2 s in the last minute,
  // stopped (busy).
  function busy(rec, ms) {
    if (!Number.isFinite(ms) || ms <= 0) return
    const t = now()
    const list = [...(tasks.get(rec.id) || []).filter(([at]) => t - at < 60_000), [t, ms]]
    tasks.set(rec.id, list)
    if (list.reduce((sum, [, x]) => sum + x, 0) > BUSY_MS) giveUp(rec.id, 'busy')
  }

  // All silent at once: no one blamed for it; all their frames taken
  // away, the views too; then started again one at a time (step), each
  // given 2 s. One that hangs only a while after it starts gets past that,
  // and they all fall silent again once all are up: the next time within
  // ten minutes, each is given as long as that took (a minute at most)
  // before the next starts.
  let fruitless = null
  function recover() {
    const order = [...recs.values()].map(rec => rec.item)
    const t = now()
    const wait = fruitless && t - fruitless < AGAIN_MS ? Math.min(HUNT_MAX_MS, Math.max(STAGGER_MS, t - fruitless)) : STAGGER_MS
    log(`plugins ${order.map(i => i.id).join(', ')}: none answered: started again one at a time, ${Math.round(wait / 1000)} s each`)
    for (const rec of [...recs.values()]) stop(rec)
    clearTimer(reloadTimer)
    reloadTimer = 0
    setViews(false)
    recovering = { queue: order, started: [], timer: 0, since: t, wait, at: 0 }
    keepWatch()
    recovering.timer = setTimer(step, RELOAD_MS)
  }
  // The next one started; every 2 s, all those started pinged: all
  // answer, again until it has had its while, then the next; one does not,
  // the one just started stopped (hung: silent with the rest, then again),
  // the others taken away and started again (their process held by it),
  // one by one.
  function step() {
    const r = recovering
    if (!r) return
    r.timer = 0
    let item = r.queue.shift()
    while (item && (!want.has(item.id) || held.has(item.id) || recs.has(item.id))) item = r.queue.shift()
    if (!item) return finish()
    const rec = start(item)
    r.started.push(rec)
    r.at = now()
    probe(r, rec)
  }
  function probe(r, rec) {
    r.timer = setTimer(() => {
      r.timer = 0
      const up = r.started.filter(x => recs.get(x.id) === x)
      for (const x of up) {
        x.ping = nonce()
        post(x.win, { waku: 1, t: 'ping', n: x.ping })
      }
      r.timer = setTimer(() => {
        r.timer = 0
        if (recovering !== r) return
        const live = up.filter(x => recs.get(x.id) === x)
        if (live.every(answered)) {
          r.answered = true
          return now() - r.at >= r.wait ? step() : probe(r, rec)
        }
        // None has answered since their frames went: the old process may
        // be there still (held); all away again, and twice as long a wait,
        // once, before the one just started is blamed.
        if (!r.answered && !r.again) {
          r.again = true
          log(`plugin ${rec.id}: no answer yet, their old process may be there still: all away, again in ${(2 * RELOAD_MS) / 1000} s`)
          for (const x of live) stop(x)
          r.queue = [...live.map(x => x.item), ...r.queue]
          r.started = []
          r.timer = setTimer(step, 2 * RELOAD_MS)
          return
        }
        const others = live.filter(x => x !== rec)
        giveUp(rec.id, 'hung', [r.since, ...missed(rec.id)])
        r.found = true
        for (const x of others) stop(x)
        r.queue = [...others.map(x => x.item), ...r.queue]
        r.started = []
        r.wait = STAGGER_MS
        r.answered = r.again = false
        r.timer = setTimer(step, RELOAD_MS)
      }, ANSWER_MS)
    }, STAGGER_MS)
  }
  // All up again: watched as before; when the hunt found no one, when the
  // last of them started kept in mind (the next one gives each as long).
  function finish() {
    fruitless = recovering?.found ? null : recovering?.at || now()
    recovering = null
    setViews(true)
    keepWatch()
  }

  // One pinged now, by hand (the tests): the nonce kept, so its pong counts.
  function ping(id, n = nonce()) {
    const rec = recs.get(id)
    if (rec?.ready) {
      rec.ping = n
      post(rec.win, { waku: 1, t: 'ping', n })
    }
    return !!rec
  }

  return {
    apply,
    setEnv,
    setAgents,
    receive,
    fromView,
    loaded,
    ping,
    recs,
    env,
    held,
    trouble,
    get recovering() {
      return recovering
    },
    get viewsOn() {
      return viewsOn
    },
    get watching() {
      return !!(pingTimer || checkTimer || recovering)
    },
    get agents() {
      return agents
    },
  }
}

// --- In her island's page -------------------------------------------------------------
export function addonsPage(Island: IslandFace | null) {
  // Their frames out of sight, off the page's left, never under the
  // pointer; not display: none, under which WebView2 tells a frame none of
  // its long tasks (the watchdog's budget: the SDK's PerformanceObserver).
  const box = document.createElement('div')
  box.id = 'addon-frames'
  box.setAttribute('aria-hidden', 'true')
  box.style.cssText = 'position:fixed;left:-10000px;top:0;width:300px;height:150px;overflow:hidden;pointer-events:none;contain:strict'
  document.body.appendChild(box)
  let payload = null
  let look = null
  // Her theme as her island shows it: Windows' mode, from the taskbar's
  // look there, else from pet:update at the top of the screen.
  const themeNow = () => (['taskbar', 'split'].includes(payload?.config?.display) ? (look?.mode === 'light' ? 'light' : 'dark') : payload?.winLight === true ? 'light' : 'dark')
  const host = createHost({
    invoke: (cmd, args) => pet.addons.call(cmd, args),
    make: (id, src) => {
      const el = document.createElement('iframe')
      el.setAttribute('sandbox', 'allow-scripts')
      el.setAttribute('referrerpolicy', 'no-referrer')
      el.setAttribute('aria-hidden', 'true')
      el.tabIndex = -1
      el.addEventListener('load', () => host.loaded(id, el))
      el.src = src
      box.appendChild(el)
      return { el, win: el.contentWindow }
    },
    drop: rec => rec.el.remove(),
    post: (win, message) => {
      try {
        win.postMessage(message, '*')
      } catch {}
    },
    log: line => console.log(line),
    // The capsule's card's view (views.ts, this page's), taken away a while.
    views: on => Island?.views?.pause(!on),
  })
  window.addEventListener('message', e => host.receive(e.source, e.data))
  pet.onAddons(list => host.apply(list))
  pet.onAddonView?.(v => host.fromView(String(v?.id || ''), v?.sub, v?.message))
  pet.onUpdate(p => {
    payload = p
    host.setEnv({ lang: p?.lang, quiet: p?.quiet === true, theme: themeNow() })
    host.setAgents(p?.list)
  })
  pet.onQuiet(q => host.setEnv({ quiet: q?.on === true }))
  pet.onLook(l => {
    look = l
    host.setEnv({ theme: themeNow() })
  })
  pet.addons.running().then(list => host.apply(list))
}

export { createHost, agentsOf, pageOf, ORIGIN, PING_EVERY, ANSWER_MS, AGAIN_MS, BUSY_MS, STAGGER_MS, RELOAD_MS, GRACE_MS }
