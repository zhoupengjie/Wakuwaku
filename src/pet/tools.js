// The taskbar's own widgets, no plugins (tools.rs reads and does what is
// Windows' part): media, notes, a timer, a calculator, the Recycle Bin,
// power, awake, the windows, a launcher, a clock, drives, a colour picker,
// search. Each has its face on the strip (face: its icon, a value, its
// name), what a press on it does (press: most open their card), and its
// card (card: markup, its buttons data-t / data-arg, done by act). Loaded
// by the page before island.js; required by the tests.
//
// x, what each is drawn from: { lang, entry, cfg, state (tools.rs: media,
// trash, disk, awake), local (the page's own for this one: a timer, the
// calculator), now (ms), apps (the taskbar's programs: { app, name, path,
// png, windows }), notes (by id), picked (colours), iconOf (a path's icon,
// once known) }.
// api, what they do: call(what, arg) (tools.rs), setCfg(patch),
// setLocal(patch), setNotes(text), taskbar(what), window(id), keyboard().
;(function (root) {
  const { t } = root.I18n || require('../shared/i18n')

  const esc = value => String(value ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c])

  // Outline icons on a 24 grid, as widgets.js draws its own.
  const ICONS = {
    media: '<path d="M9 18V5l11-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="17" cy="16" r="3"/>',
    notes: '<path d="M6 3h9l4 4v14H6z"/><path d="M9 11h6M9 15h6"/>',
    timer: '<circle cx="12" cy="13" r="8"/><path d="M12 9v4l2 2M9 2h6"/>',
    calc: '<rect x="5" y="3" width="14" height="18" rx="2"/><path d="M8 7h8M8 12h.01M12 12h.01M16 12h.01M8 16h.01M12 16h.01M16 16h.01"/>',
    trash: '<path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3"/>',
    power: '<path d="M12 3v9"/><path d="M6.5 6.5a8 8 0 1 0 11 0"/>',
    awake: '<path d="M5 9h11v5a5 5 0 0 1-5 5h-1a5 5 0 0 1-5-5z"/><path d="M16 10h1.5a2.5 2.5 0 0 1 0 5H16M8 3v3M11 3v3"/>',
    windows: '<rect x="3" y="5" width="13" height="10" rx="1.5"/><path d="M8 19h13V9"/>',
    launch: '<path d="M12 3l2.7 5.6 6.1.9-4.4 4.3 1 6.1-5.4-2.9-5.4 2.9 1-6.1L3.2 9.5l6.1-.9z"/>',
    clock: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
    disk: '<rect x="3" y="6" width="18" height="12" rx="2"/><path d="M7 14h.01M11 14h6"/>',
    picker: '<path d="M14.5 4.5l5 5M17 2l5 5-3 3-5-5zM13.5 7.5l-9 9V20h3.5l9-9"/>',
    search: '<circle cx="11" cy="11" r="7"/><path d="M20 20l-4-4"/>',
    link: '<path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1"/><path d="M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1"/>',
  }
  const KINDS = Object.keys(ICONS).filter(k => k !== 'link')
  // One of a kind on the taskbar, or as many as one likes; put there small
  // (its icon, a number) or medium at first.
  const MANY = ['notes', 'timer', 'launch', 'clock']
  const SMALL = ['calc', 'trash', 'power', 'awake', 'windows', 'launch', 'picker', 'search']
  // Their colours, Windows' accent-like tones.
  const COLOUR = { media: '#ff6482', notes: '#ffd60a', timer: '#ff9f0a', calc: '#8e8e93', trash: '#8e8e93', power: '#ff453a', awake: '#bf8b5e', windows: '#5e9bff', launch: '#ffd60a', clock: '#64d2ff', disk: '#5e9bff', picker: '#bf5af2', search: '#5e9bff' }
  // Windows' own media glyphs (Segoe Fluent Icons).
  const GLYPH = { prev: 'E892', play: 'E768', pause: 'E769', next: 'E893' }

  const icon = kind => `<svg class="wicon" viewBox="0 0 24 24" aria-hidden="true">${ICONS[kind] || ICONS.launch}</svg>`
  const glyph = name => `<span class="tg" aria-hidden="true">&#x${GLYPH[name]};</span>`
  const T = (x, key, vars) => t(x.lang, key, vars)
  const button = (what, label, { arg = '', cls = '', title = '' } = {}) =>
    `<button type="button" class="tbtn${cls ? ` ${cls}` : ''}" data-t="${what}"${arg !== '' ? ` data-arg="${esc(arg)}"` : ''}${title ? ` title="${esc(title)}"` : ''}>${label}</button>`

  // --- Words and numbers ------------------------------------------------------------

  // Bytes as a size: 0 B, 980 B, 12 KB, 1.2 MB, 512 GB, 1.8 TB.
  function size(bytes) {
    const n = Number(bytes) || 0
    const units = ['B', 'KB', 'MB', 'GB', 'TB']
    let i = 0
    let v = n
    while (v >= 1000 && i < units.length - 1) {
      v /= 1024
      i++
    }
    return `${i && v < 10 ? v.toFixed(1) : Math.round(v)} ${units[i]}`
  }

  // A time left: 4:59, 1:02:03.
  function clock(ms) {
    const s = Math.max(0, Math.ceil(ms / 1000))
    const h = Math.floor(s / 3600)
    const m = Math.floor((s % 3600) / 60)
    const pad = n => String(n).padStart(2, '0')
    return h ? `${h}:${pad(m)}:${pad(s % 60)}` : `${m}:${pad(s % 60)}`
  }

  // The cities a clock may keep: their zones and names.
  const CITIES = [
    ['', 'tool.clock.here'],
    ['Asia/Shanghai', 'tool.city.shanghai'],
    ['Asia/Tokyo', 'tool.city.tokyo'],
    ['Asia/Singapore', 'tool.city.singapore'],
    ['Asia/Kolkata', 'tool.city.delhi'],
    ['Europe/London', 'tool.city.london'],
    ['Europe/Berlin', 'tool.city.berlin'],
    ['America/New_York', 'tool.city.newyork'],
    ['America/Los_Angeles', 'tool.city.la'],
    ['Australia/Sydney', 'tool.city.sydney'],
    ['UTC', 'tool.city.utc'],
  ]
  const cityOf = tz => CITIES.find(c => c[0] === (tz || '')) || CITIES[0]

  // The hours and minutes (and seconds), in a zone ('': here), as numbers.
  function timeIn(tz, now) {
    const parts = new Intl.DateTimeFormat('en-GB', { timeZone: tz || undefined, hour: '2-digit', minute: '2-digit', second: '2-digit', hourCycle: 'h23' }).formatToParts(new Date(now))
    const get = type => Number(parts.find(p => p.type === type)?.value || 0)
    return { h: get('hour'), m: get('minute'), s: get('second') }
  }
  const hm = ({ h, m }) => `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}`

  // --- The calculator ----------------------------------------------------------------
  // What is typed: numbers and + − × ÷, worked out as arithmetic has it
  // (× and ÷ first), never by eval. null for what is not a sum.
  function calculate(expr) {
    const tokens = String(expr).replace(/×/g, '*').replace(/÷/g, '/').replace(/−/g, '-').match(/\d+(?:\.\d+)?|\.\d+|[-+*/()]/g)
    if (!tokens || tokens.join('') !== String(expr).replace(/×/g, '*').replace(/÷/g, '/').replace(/−/g, '-').replace(/\s+/g, '')) return null
    let i = 0
    const peek = () => tokens[i]
    const factor = () => {
      const tok = tokens[i++]
      if (tok === '-') return -factor()
      if (tok === '+') return factor()
      if (tok === '(') {
        const v = sum()
        if (tokens[i++] !== ')') throw new Error('(')
        return v
      }
      const v = Number(tok)
      if (!Number.isFinite(v)) throw new Error(tok)
      return v
    }
    const product = () => {
      let v = factor()
      while (peek() === '*' || peek() === '/') v = tokens[i++] === '*' ? v * factor() : v / factor()
      return v
    }
    const sum = () => {
      let v = product()
      while (peek() === '+' || peek() === '-') v = tokens[i++] === '+' ? v + product() : v - product()
      return v
    }
    try {
      const v = sum()
      return i === tokens.length && Number.isFinite(v) ? v : null
    } catch {
      return null
    }
  }

  // A result as the calculator shows it: ten significant digits at most, no
  // binary noise (0.1 + 0.2 is 0.3).
  const shown = v => (v == null ? '' : String(Number(Number(v).toPrecision(10))))

  // A key pressed on the calculator: what is typed after it.
  function typed(expr, key) {
    const s = String(expr || '')
    if (key === 'C') return ''
    if (key === '⌫') return s.slice(0, -1)
    if (key === '=') return shown(calculate(s)) || s
    if (key === '%') {
      const m = s.match(/(\d+(?:\.\d+)?)$/)
      return m ? s.slice(0, -m[1].length) + shown(Number(m[1]) / 100) : s
    }
    if ('+−×÷'.includes(key)) return /[+−×÷]$/.test(s) ? s.slice(0, -1) + key : s || key === '−' ? s + key : s
    if (key === '.') return /\d*\.\d*$/.test(s.split(/[+−×÷]/).pop()) ? s : s + (/\d$/.test(s) ? '.' : '0.')
    return s + key
  }

  // --- Each kind -----------------------------------------------------------------------

  const timerOf = x => {
    const l = x.local || {}
    const total = l.total || (Number(x.cfg?.minutes) || 25) * 60000
    const left = l.endAt ? Math.max(0, l.endAt - x.now) : l.left ?? total
    return { total, left, running: !!l.endAt, done: !!l.done }
  }
  const notesOf = x => String(x.notes?.[x.entry.id] || '')
  const diskOf = x => {
    const all = Array.isArray(x.state?.disk) ? x.state.disk : []
    return all.find(d => d.drive === x.cfg?.drive) || all.find(d => d.drive === 'C:') || all[0] || null
  }
  const windowsOf = x => (x.apps || []).flatMap(a => (a.windows || []).map(w => ({ ...w, png: w.png || a.png, name: a.name })))

  // Its face: { icon (markup), colour, img, value, label, short, lit,
  // ring, empty }: on the strip S is its icon and short (if any), M its
  // value over its label.
  function face(kind, x) {
    const name = T(x, `tool.${kind}`)
    const base = { icon: icon(kind), colour: COLOUR[kind], value: '', label: name }
    switch (kind) {
      case 'media': {
        const m = x.state?.media
        if (!m || (!m.title && !m.artist)) return { ...base, value: T(x, 'tool.media.none'), empty: true }
        return { ...base, value: m.title || m.artist, label: [m.artist, m.app].filter(Boolean).join(' · ') || name, lit: m.playing }
      }
      case 'notes': {
        const first = notesOf(x).split('\n').find(l => l.trim()) || ''
        return { ...base, value: first || T(x, 'tool.notes.empty'), empty: !first }
      }
      case 'timer': {
        const tm = timerOf(x)
        return { ...base, value: tm.done ? T(x, 'tool.timer.done') : clock(tm.left), short: tm.running || tm.done ? (tm.done ? '0:00' : clock(tm.left)) : '', lit: tm.running || tm.done, ring: tm.done }
      }
      case 'calc':
        return { ...base, value: x.local?.expr || '0' }
      case 'trash': {
        const tr = x.state?.trash
        if (!tr) return { ...base, value: '—', empty: true }
        return { ...base, value: tr.items ? T(x, 'tool.trash.items', { n: tr.items }) : T(x, 'tool.trash.empty'), short: tr.items ? String(tr.items) : '', empty: !tr.items }
      }
      case 'power':
        return { ...base, value: name, label: T(x, 'tool.power.hint') }
      case 'awake': {
        const a = x.state?.awake
        if (!a?.on) return { ...base, value: T(x, 'tool.awake.off') }
        return { ...base, value: a.until ? clock(a.until - x.now) : T(x, 'tool.awake.on'), short: a.until ? clock(a.until - x.now) : '', lit: true }
      }
      case 'windows': {
        const n = windowsOf(x).length
        return { ...base, value: T(x, 'tool.windows.n', { n }), short: String(n) }
      }
      case 'launch': {
        const path = x.cfg?.path
        if (!path) return { ...base, value: T(x, 'tool.launch.none'), empty: true }
        const isLink = /^[a-z][a-z0-9+.-]*:\/\//i.test(path)
        return { ...base, icon: icon(isLink ? 'link' : 'launch'), img: !isLink && x.iconOf?.(path), value: x.cfg.name || path.split(/[\\/]/).filter(Boolean).pop() || path }
      }
      case 'clock': {
        const city = cityOf(x.cfg?.tz)
        const time = hm(timeIn(x.cfg?.tz, x.now))
        return { ...base, value: time, short: time, label: T(x, city[1]) }
      }
      case 'disk': {
        const d = diskOf(x)
        if (!d) return { ...base, value: '—', empty: true }
        const used = Math.round(((d.total - d.free) / d.total) * 100)
        return { ...base, value: T(x, 'tool.disk.free', { size: size(d.free) }), label: `${d.drive} ${d.label || ''}`.trim(), short: `${used}%`, lit: used >= 90 }
      }
      case 'picker': {
        const last = (x.picked || [])[0]
        return { ...base, colour: last || COLOUR.picker, value: last || T(x, 'tool.picker.none'), empty: !last }
      }
      case 'search':
        return { ...base, value: name, label: T(x, 'tool.search.hint') }
    }
    return base
  }

  // A press on its face: done there and then (true), or its card opens.
  function press(kind, x, api) {
    if (kind === 'search') return api.taskbar('search'), true
    if (kind === 'picker') return api.call('start'), true
    if (kind === 'launch' && x.cfg?.path) return api.call('open', x.cfg.path), true
    if (kind === 'awake') return x.state?.awake?.on ? api.call('off') : api.call('on', '0'), true
    return false
  }

  // Its card: what it shows and its buttons (data-t, data-arg); a field's
  // value goes with a button that names it (data-from).
  function card(kind, x) {
    switch (kind) {
      case 'media': {
        const m = x.state?.media
        if (!m || (!m.title && !m.artist)) return `<div class="tnone">${esc(T(x, 'tool.media.none'))}</div>`
        return `<div class="tbig">${esc(m.title || m.artist)}</div><div class="tsub">${esc([m.artist, m.app].filter(Boolean).join(' · '))}</div>
          <div class="trow tcenter">${button('prev', glyph('prev'), { cls: m.prev ? 'round' : 'round off', title: T(x, 'tool.media.prev') })}${button('toggle', glyph(m.playing ? 'pause' : 'play'), { cls: 'round main', title: T(x, m.playing ? 'tool.media.pause' : 'tool.media.play') })}${button('next', glyph('next'), { cls: m.next ? 'round' : 'round off', title: T(x, 'tool.media.next') })}</div>`
      }
      case 'notes':
        return `<textarea class="tnote" data-field="note" spellcheck="false" placeholder="${esc(T(x, 'tool.notes.hint'))}">${esc(notesOf(x))}</textarea><div class="trow">${button('copy', esc(T(x, 'tool.copy')))}</div>`
      case 'timer': {
        const tm = timerOf(x)
        const presets = [1, 3, 5, 10, 15, 25, 45, 60].map(m => button('set', esc(T(x, 'tool.timer.min', { n: m })), { arg: m, cls: tm.total === m * 60000 && !tm.running ? 'on' : '' })).join('')
        return `<div class="tbig tnum${tm.done ? ' ring' : ''}">${tm.done ? esc(T(x, 'tool.timer.done')) : clock(tm.left)}</div>
          <div class="trow twrap">${presets}</div>
          <div class="trow">${button(tm.running ? 'pause' : 'start', esc(T(x, tm.running ? 'tool.timer.pause' : 'tool.timer.start')), { cls: 'main' })}${button('reset', esc(T(x, 'tool.timer.reset')))}</div>`
      }
      case 'calc': {
        const expr = x.local?.expr || ''
        const live = calculate(expr)
        const keys = ['C', '⌫', '%', '÷', '7', '8', '9', '×', '4', '5', '6', '−', '1', '2', '3', '+', '0', '.', '=']
        return `<div class="tcalc"><div class="texpr">${esc(expr || '0')}</div><div class="tres">${live != null && expr && !/^[\d.]+$/.test(expr) ? `= ${esc(shown(live))}` : '&nbsp;'}</div>
          <div class="tkeys">${keys.map(k => button('key', esc(k), { arg: k, cls: k === '=' ? 'main wide2' : '+−×÷'.includes(k) ? 'op' : k === '0' ? 'wide' : '' })).join('')}</div></div>
          <div class="trow">${button('copy', esc(T(x, 'tool.copy')))}</div>`
      }
      case 'trash': {
        const tr = x.state?.trash
        const confirm = x.local?.confirm === 'empty'
        const what = tr ? (tr.items ? `${esc(T(x, 'tool.trash.items', { n: tr.items }))} · ${esc(size(tr.bytes))}` : esc(T(x, 'tool.trash.empty'))) : '—'
        if (confirm) return `<div class="tbig">${what}</div><div class="twarn">${esc(T(x, 'tool.trash.sure'))}</div><div class="trow">${button('empty', esc(T(x, 'tool.trash.emptyNow')), { cls: 'danger' })}${button('cancel', esc(T(x, 'tool.cancel')))}</div>`
        return `<div class="tbig">${what}</div><div class="trow">${button('open', esc(T(x, 'tool.trash.open')))}${tr?.items ? button('ask', esc(T(x, 'tool.trash.emptyAsk')), { arg: 'empty' }) : ''}</div>`
      }
      case 'power': {
        const sure = x.local?.confirm
        if (sure) return `<div class="twarn">${esc(T(x, 'tool.power.sure', { what: T(x, `tool.power.${sure}`) }))}</div><div class="trow">${button('do', esc(T(x, `tool.power.${sure}`)), { arg: sure, cls: 'danger' })}${button('cancel', esc(T(x, 'tool.cancel')))}</div>`
        return `<div class="tgrid">${['lock', 'sleep', 'signout', 'restart', 'shutdown'].map(w => button(w === 'lock' || w === 'sleep' ? 'do' : 'ask', esc(T(x, `tool.power.${w}`)), { arg: w })).join('')}</div>`
      }
      case 'awake': {
        const a = x.state?.awake
        const now = a?.on ? (a.until ? T(x, 'tool.awake.until', { left: clock(a.until - x.now) }) : T(x, 'tool.awake.always')) : T(x, 'tool.awake.off')
        return `<div class="tbig">${esc(now)}</div><div class="tsub">${esc(T(x, 'tool.awake.hint'))}</div>
          <div class="trow twrap">${[0, 30, 60, 120, 240].map(m => button('on', esc(m ? T(x, 'tool.awake.for', { n: m >= 60 ? T(x, 'tool.hours', { n: m / 60 }) : T(x, 'tool.minutes', { n: m }) }) : T(x, 'tool.awake.always')), { arg: m })).join('')}${a?.on ? button('off', esc(T(x, 'tool.awake.stop')), { cls: 'danger' }) : ''}</div>`
      }
      case 'windows': {
        const list = windowsOf(x)
        if (!list.length) return `<div class="tnone">${esc(T(x, 'tool.windows.none'))}</div>`
        return `<div class="tlist">${list.map(w => `<button type="button" class="titem${w.front ? ' on' : ''}" data-t="window" data-arg="${esc(w.id)}">${w.png ? `<img src="${esc(w.png)}" alt="">` : icon('windows')}<span>${esc(w.title || w.name)}</span></button>`).join('')}</div>`
      }
      case 'launch': {
        const path = x.cfg?.path || ''
        const apps = (x.apps || []).filter(a => a.path && !/\\ApplicationFrameHost\.exe$/i.test(a.path))
        return `${path ? `<div class="tsub tpath">${esc(path)}</div>` : `<div class="tnone">${esc(T(x, 'tool.launch.none'))}</div>`}
          <div class="tlabel">${esc(T(x, 'tool.launch.pick'))}</div>
          <div class="tlist tapps">${apps.map(a => `<button type="button" class="titem${a.path === path ? ' on' : ''}" data-t="pick" data-arg="${esc(a.path)}" data-name="${esc(a.name || '')}">${a.png ? `<img src="${esc(a.png)}" alt="">` : icon('launch')}<span>${esc(a.name || a.path)}</span></button>`).join('') || `<span class="tsub">${esc(T(x, 'tool.launch.noApps'))}</span>`}</div>
          <div class="tlabel">${esc(T(x, 'tool.launch.own'))}</div>
          <div class="trow"><input class="tin" data-field="path" spellcheck="false" placeholder="${esc(T(x, 'tool.launch.hint'))}" value="${esc(path)}">${button('setPath', esc(T(x, 'tool.save')), { cls: 'main' })}</div>`
      }
      case 'clock': {
        const tz = x.cfg?.tz || ''
        const tm = timeIn(tz, x.now)
        const analog = x.cfg?.style === 'analog'
        const hand = (deg, len, w) => `<line x1="50" y1="50" x2="${50 + len * Math.sin((deg * Math.PI) / 180)}" y2="${50 - len * Math.cos((deg * Math.PI) / 180)}" stroke-width="${w}" stroke-linecap="round"/>`
        const dial = analog
          ? `<svg class="tdial" viewBox="0 0 100 100"><circle cx="50" cy="50" r="46"/>${[...Array(12)].map((_, i) => `<line class="tick" x1="50" y1="8" x2="50" y2="${i % 3 ? 12 : 15}" transform="rotate(${i * 30} 50 50)"/>`).join('')}<g class="hands">${hand((tm.h % 12) * 30 + tm.m / 2, 24, 4)}${hand(tm.m * 6, 34, 3)}</g><g class="sec">${hand(tm.s * 6, 38, 1.5)}</g></svg>`
          : `<div class="tbig tnum thuge">${hm(tm)}</div>`
        return `${dial}<div class="tsub tcenter">${esc(T(x, cityOf(tz)[1]))}</div>
          <div class="trow">${button('style', esc(T(x, 'tool.clock.digital')), { arg: 'digital', cls: analog ? '' : 'on' })}${button('style', esc(T(x, 'tool.clock.analog')), { arg: 'analog', cls: analog ? 'on' : '' })}</div>
          <div class="trow twrap">${CITIES.map(([zone, key]) => button('tz', esc(T(x, key)), { arg: zone, cls: zone === tz ? 'on' : '' })).join('')}</div>`
      }
      case 'disk': {
        const all = Array.isArray(x.state?.disk) ? x.state.disk : []
        if (!all.length) return `<div class="tnone">—</div>`
        const mine = diskOf(x)
        return `<div class="tlist">${all
          .map(d => {
            const used = Math.round(((d.total - d.free) / d.total) * 100)
            return `<div class="tdisk${d === mine ? ' on' : ''}"><button type="button" class="tdhead" data-t="drive" data-arg="${esc(d.drive)}"><b>${esc(d.drive)} ${esc(d.label || '')}</b><span>${esc(T(x, 'tool.disk.of', { free: size(d.free), total: size(d.total) }))}</span></button><span class="tbar${used >= 90 ? ' high' : ''}"><i style="width:${used}%"></i></span>${button('open', esc(T(x, 'tool.disk.open')), { arg: `${d.drive}\\`, cls: 'small' })}</div>`
          })
          .join('')}</div>`
      }
      case 'picker': {
        const all = x.picked || []
        return `<div class="trow">${button('start', esc(T(x, 'tool.picker.start')), { cls: 'main' })}<span class="tsub">${esc(T(x, 'tool.picker.hint'))}</span></div>
          ${all.length ? `<div class="tswatches">${all.map(c => `<button type="button" class="tsw" data-t="copy" data-arg="${esc(c)}" title="${esc(T(x, 'tool.picker.copy', { hex: c }))}"><i style="background:${esc(c)}"></i><span>${esc(c)}</span></button>`).join('')}</div>` : `<div class="tnone">${esc(T(x, 'tool.picker.none'))}</div>`}`
      }
      case 'search':
        return `<div class="trow">${button('search', esc(T(x, 'tool.search.open')), { cls: 'main' })}</div>`
    }
    return ''
  }

  // A button on its card. arg: the button's (or the field it names').
  function act(kind, what, arg, x, api) {
    switch (`${kind}:${what}`) {
      case 'media:prev':
      case 'media:toggle':
      case 'media:next':
        return api.call(what)
      case 'notes:copy':
        return api.call('copy', notesOf(x))
      case 'timer:set': {
        const total = Math.max(1, Number(arg) || 25) * 60000
        api.setCfg({ minutes: Number(arg) || 25 })
        return api.setLocal({ total, left: total, endAt: null, done: false })
      }
      case 'timer:start': {
        const tm = timerOf(x)
        const left = tm.left > 0 ? tm.left : tm.total
        return api.setLocal({ total: tm.total, endAt: x.now + left, left, done: false })
      }
      case 'timer:pause':
        return api.setLocal({ left: timerOf(x).left, endAt: null })
      case 'timer:reset':
        return api.setLocal({ left: timerOf(x).total, endAt: null, done: false })
      case 'calc:key':
        return api.setLocal({ expr: typed(x.local?.expr, arg) })
      case 'calc:copy':
        return api.call('copy', shown(calculate(x.local?.expr || '')) || x.local?.expr || '')
      case 'trash:open':
        return api.call('open')
      case 'trash:ask':
      case 'power:ask':
        return api.setLocal({ confirm: arg })
      case 'trash:cancel':
      case 'power:cancel':
        return api.setLocal({ confirm: null })
      case 'trash:empty':
        api.setLocal({ confirm: null })
        return api.call('empty')
      case 'power:do':
        api.setLocal({ confirm: null })
        return api.call(arg)
      case 'awake:on':
        return api.call('on', String(Number(arg) || 0))
      case 'awake:off':
        return api.call('off')
      case 'windows:window':
        return api.window(arg)
      case 'launch:pick':
        return api.setCfg({ path: arg, name: x.data?.name || '' })
      case 'launch:setPath': {
        const path = String(arg || '').trim()
        return api.setCfg({ path, name: '' })
      }
      case 'clock:style':
        return api.setCfg({ style: arg === 'analog' ? 'analog' : 'digital' })
      case 'clock:tz':
        return api.setCfg({ tz: arg })
      case 'disk:drive':
        return api.setCfg({ drive: arg })
      case 'disk:open':
        return api.call('open', arg)
      case 'picker:start':
        return api.call('start')
      case 'picker:copy':
        return api.call('copy', arg)
      case 'search:search':
        return api.taskbar('search')
    }
  }

  // A timer run out since it was last looked at (true once): its bell.
  function rings(x) {
    const l = x.local || {}
    return !!l.endAt && l.endAt <= x.now && !l.done
  }

  const api = { ICONS, KINDS, MANY, SMALL, COLOUR, icon, face, press, card, act, rings, size, clock, calculate, typed, shown, timeIn, CITIES }

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api
  } else {
    root.Tools = api
  }
})(this)
