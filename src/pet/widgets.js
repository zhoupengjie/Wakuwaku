// Widgets, the first kind of plugin in the island (widgets.rs): their icons,
// and their words in her language. Loaded by the page before island.js, and
// required by the tests.
//
// A widget is { id, label, value, icon, color, builtIn, on, leftMs, private };
// a built-in one's label and value are { key, vars } to translate. A private
// one's label stays off screen while the specifics are (details off). The
// monitor's value is { parts: [{ icon, text, kind?, warn? }] }: shown as
// icon and number.
;(function (root) {
  const { render } = root.I18n || require('../shared/i18n')

  // Outline icons on a 24 grid, drawn with currentColor (widgets.rs ICONS).
  const ICONS = {
    cpu: '<rect x="6" y="6" width="12" height="12" rx="2"/><path d="M9 2v4M15 2v4M9 18v4M15 18v4M2 9h4M2 15h4M18 9h4M18 15h4"/>',
    weather: '<path d="M7 18a4 4 0 0 1 0-8 5 5 0 0 1 9.6-1.5A3.5 3.5 0 1 1 17.5 18z"/>',
    note: '<path d="M6 3h9l4 4v14H6z"/><path d="M9 11h6M9 15h6"/>',
    calendar: '<rect x="4" y="5" width="16" height="15" rx="2"/><path d="M4 10h16M9 3v4M15 3v4"/>',
    bell: '<path d="M6 16V11a6 6 0 0 1 12 0v5l1.5 2h-15z"/><path d="M10 20a2 2 0 0 0 4 0"/>',
    stock: '<path d="M3 17l6-6 4 4 7-7"/><path d="M15 8h5v5"/>',
    today: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M2 12h2M20 12h2M5 5l1.5 1.5M17.5 17.5L19 19M5 19l1.5-1.5M17.5 6.5L19 5"/>',
    clock: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
    mail: '<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M3 7l9 6 9-6"/>',
    music: '<path d="M9 18V5l11-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="17" cy="16" r="3"/>',
    code: '<path d="M8 8l-4 4 4 4M16 8l4 4-4 4M13.5 5l-3 14"/>',
    star: '<path d="M12 3l2.7 5.6 6.1.9-4.4 4.3 1 6.1-5.4-2.9-5.4 2.9 1-6.1L3.2 9.5l6.1-.9z"/>',
    dot: '<circle cx="12" cy="12" r="4"/>',
    chart: '<path d="M4 20V10M10 20V4M16 20v-7M22 20H2"/>',
    battery: '<rect x="2" y="7" width="17" height="10" rx="2"/><path d="M22 11v2M6 10v4M10 10v4"/>',
    timer: '<circle cx="12" cy="13" r="8"/><path d="M12 9v4l2 2M9 2h6"/>',
    flag: '<path d="M5 21V4M5 4h11l-2 4 2 4H5"/>',
    server: '<rect x="3" y="4" width="18" height="7" rx="2"/><rect x="3" y="13" width="18" height="7" rx="2"/><path d="M7 7.5h.01M7 16.5h.01"/>',
    check: '<circle cx="12" cy="12" r="9"/><path d="M8 12l3 3 5-6"/>',
    terminal: '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M7 9l3 3-3 3M13 15h4"/>',
    coin: '<circle cx="12" cy="12" r="9"/><path d="M14.5 9.5c-.5-1-1.5-1.5-2.5-1.5-1.5 0-2.5.8-2.5 2s1 1.7 2.5 2 2.5.8 2.5 2-1 2-2.5 2c-1 0-2-.5-2.5-1.5M12 6v2M12 16v2"/>',
    gauge: '<path d="M4 17a8 8 0 1 1 16 0"/><path d="M12 17l4-6"/><path d="M4 17h2M18 17h2M12 9V7"/>',
    memory: '<rect x="3" y="7" width="18" height="10" rx="1.5"/><path d="M7 7v10M11 7v10M15 7v10M6 17v3M10 17v3M14 17v3M18 17v3"/>',
    down: '<path d="M12 4v15M6 13l6 6 6-6"/>',
    up: '<path d="M12 20V5M6 11l6-6 6 6"/>',
    bolt: '<path d="M13 2L5 14h6l-1 8 8-12h-6z"/>',
  }

  // Icons Windows has itself, from its icon font (Segoe Fluent Icons; Segoe
  // MDL2 Assets on Windows 10) rather than drawn: the monitor's, as Task
  // Manager's performance page has it.
  const GLYPHS = { gauge: 'E9D9' }
  // The monitor's readings' glyphs, as Windows' own: the processor, memory,
  // the network's down and up as small solid triangles (drawn smaller,
  // style.css .wg.tri); the battery by its level (Battery0 to Battery10),
  // charging too (BatteryCharging0 to BatteryCharging10). The hardware's
  // sensors (hw.rs, lhm.rs) by their hardware: a graphics card (F211), the
  // motherboard (F20C), a drive (EDA2, HardDrive), a fan turning (E72C),
  // another (E9D9, the monitor's own); the unit in the text says what of it.
  const PART_GLYPH = { cpu: 'E950', memory: 'E964', down: 'EDDC', up: 'EDDB', gpu: 'F211', board: 'F20C', disk: 'EDA2', fan: 'E72C', sensor: 'E9D9' }
  const BATTERY = ['E850', 'E851', 'E852', 'E853', 'E854', 'E855', 'E856', 'E857', 'E858', 'E859', 'E83F']
  const CHARGING = ['E85A', 'E85B', 'E85C', 'E85D', 'E85E', 'E85F', 'E860', 'E861', 'E862', 'E863', 'E83E']
  const esc = value => String(value ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c])

  // One without an icon of its own (a script that sent none: widgets.rs
  // makes it 'dot'): wherever its icon would be, the first character of its
  // name, weight 600, on a 16 px rounded square (the design session,
  // 2026-10-16: a dot said nothing, and was lost on the dark), tinted as the
  // stack cards' icon tiles are: its colour at 18% under it (14% on a light
  // ground), the letter in its colour (one step deeper on a light ground,
  // --mark-deep: deepen); with none, a grey tint and the secondary grey
  // (.wmark.none). One helper for every place (style.css, desk.css .wmark).
  const hasIcon = name => typeof name === 'string' && name !== 'dot' && name in ICONS

  // A third-party plugin's own picture for its icon (addons.rs: an .svg in
  // its folder, served by her scheme; widgets.rs iconUrl), drawn in one
  // colour as her own icons are: the colour around it, through it as a
  // mask. Only one from there (or, in the simulator, a data: picture), each
  // part of its path escaped; anything else is none (its mark then).
  const ADDON_ORIGIN = 'http://wakuplugin.localhost/'
  function maskUrl(url) {
    if (typeof url !== 'string') return ''
    if (url.startsWith(ADDON_ORIGIN)) {
      const parts = url.slice(ADDON_ORIGIN.length).split('/')
      if (parts.length < 2 || !/^[a-z][a-z0-9-]{1,39}$/.test(parts[0]) || !/\.svg$/i.test(url) || parts.some(p => !p || p === '.' || p === '..')) return ''
      return ADDON_ORIGIN + parts.map(encodeURIComponent).join('/')
    }
    return /^data:image\/svg\+xml[;,]/.test(url) ? url.replace(/["\\\n\r]/g, c => encodeURIComponent(c)) : ''
  }
  const hasPicture = w => !!maskUrl(w?.iconUrl)
  const maskHTML = url => `<span class="wicon wmask" style="${esc(`--mask:url("${maskUrl(url)}")`)}" aria-hidden="true"></span>`

  function markHTML(name, color, cls = '') {
    const ch = Array.from(String(name ?? '').trim())[0] || '·'
    const own = typeof color === 'string' && /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.test(color.trim()) ? color.trim() : ''
    const look = own ? ` style="--mark:${own};--mark-deep:${deepen(own)}"` : ''
    return `<span class="wmark${own ? '' : ' none'}${cls ? ` ${cls}` : ''}"${look} aria-hidden="true">${esc(ch.toLocaleUpperCase())}</span>`
  }
  // A widget's name for its mark: its label as it reads (an object of
  // words rendered when a language is given), else its id.
  const markName = (w, lang) => (typeof w?.label === 'string' ? w.label : lang && w?.label ? render(lang, w.label) : '') || w?.name || w?.id || ''
  // Its icon (a plugin's own picture, else one of hers), or that mark.
  const iconHTML = (w, lang) => (hasPicture(w) ? maskHTML(w.iconUrl) : hasIcon(w?.icon) ? icon(w.icon) : markHTML(markName(w, lang), w?.color))

  // The monitor's readings, or null for a widget that has none.
  const partsOf = w => (Array.isArray(w?.value?.parts) ? w.value.parts : null)

  // The icon as markup in the colour of what surrounds it: Windows' own
  // glyph where it has one, else its strokes.
  const icon = name =>
    GLYPHS[name]
      ? `<span class="wicon wglyph" aria-hidden="true">&#x${GLYPHS[name]};</span>`
      : `<svg class="wicon" viewBox="0 0 24 24" aria-hidden="true">${ICONS[name] || ICONS.dot}</svg>`

  // A widget's detail (widgets.rs detail_of): which sizes it has, a series
  // (a curve or bars), rows (a list) and its extra (a few words more). A
  // string, as main had it before, is its extra. Checked there already.
  function detailOf(w) {
    const d = w?.detail
    if (typeof d === 'string') return d ? { extra: d } : {}
    return d && typeof d === 'object' && !Array.isArray(d) ? d : {}
  }

  // The sizes a widget has: as its detail says; the built-in ones' own; a
  // script's that says none, small and medium (its value over its label:
  // nothing more to show large). One that says it is small alone (a
  // countdown) is small alone, on the taskbar too.
  const BUILT_SIZES = { monitor: ['s', 'm', 'l'], today: ['s', 'm', 'l'], tokens: ['s', 'm'] }
  function sizesOf(w) {
    const said = detailOf(w).sizes
    if (Array.isArray(said) && said.length) return ['s', 'm', 'l'].filter(z => said.includes(z))
    return BUILT_SIZES[w?.id] || ['s', 'm']
  }

  // What a widget says: its label and its value, in her language; a private
  // one's label not, unless the specifics may show. And its detail's extra,
  // for the open island alone (the weather's range for the day).
  const words = (lang, w, detailed = true) => ({
    label: w.private && !detailed ? '' : render(lang, w.label),
    value: partsOf(w) ? partsOf(w).map(p => p.text).join(' ') : render(lang, w.value),
    detail: detailOf(w).extra || '',
  })

  // How full a reading is, 0 to 100. The processor, memory and the battery
  // are shares already ("23%"). The network's rate ("5.1K", "337K", "48M",
  // widgets.rs rate) has no 100%: by its order of magnitude, 1 KB/s empty
  // and 100 MB/s full, each tenfold a fifth, so a rate is always as full.
  // A sensor's by its kind: a load a share, a temperature from 30 °C empty
  // to 100 °C full, a drive read or written (hw.rs) as the network's but
  // full at 1 GB/s, the others (watts, a fan, a clock) none.
  function shareOf(p) {
    const n = parseFloat(p.text)
    if (!Number.isFinite(n)) return 0
    const clamp = v => Math.max(0, Math.min(100, Math.round(v)))
    const bytes = () => n * ({ K: 1e3, M: 1e6, G: 1e9 }[String(p.text).trim().slice(-1)] || 1)
    const decades = (tens, from = 3) => clamp(((Math.log10(Math.max(bytes(), 10 ** from)) - from) / tens) * 100)
    if (p.kind === 'read' || p.kind === 'write') return decades(6)
    if (p.kind) return p.kind === 'load' ? clamp(n) : p.kind === 'temp' ? clamp(((n - 30) / 70) * 100) : 0
    if (p.icon !== 'down' && p.icon !== 'up') return clamp(n)
    return decades(5)
  }

  // A reading's glyph: Windows' own, the battery's by its level.
  function glyphOf(p) {
    if (p.icon === 'battery' || p.icon === 'bolt') return (p.icon === 'bolt' ? CHARGING : BATTERY)[Math.round(Math.max(0, Math.min(100, parseFloat(p.text) || 0)) / 10)]
    return PART_GLYPH[p.icon] || PART_GLYPH.cpu
  }

  // A reading as markup: Windows' glyph and the number in one small cell,
  // its ground filled from the left as far as the reading goes (style.css
  // .wcell), one that wants a look marked high. A drive read or written
  // has the network's small triangle, down or up, at its glyph's corner
  // (.wg.rd, .wg.wr), the cell no wider.
  const MARK = { read: ' rd', write: ' wr' }
  const cellHTML = p =>
    `<span class="wcell${isHigh(p) ? ' high' : ''}" style="--p:${shareOf(p)}%"><span class="wg${p.icon === 'down' || p.icon === 'up' ? ' tri' : MARK[p.kind] || ''}" aria-hidden="true">&#x${glyphOf(p)};</span><b>${esc(p.text)}</b></span>`

  // The monitor's readings as markup, in a line.
  const partsHTML = w => (partsOf(w) || []).map(cellHTML).join('')

  // A colour (#rgb or #rrggbb) darkened, its hue kept, until it reads on
  // the ground at the ratio WCAG asks of text (4.5:1); one that already
  // does, and one that is not such a colour, as it is.
  function deepen(colour, ground = '#fbfbfb', ratio = 4.5) {
    const rgb = hex => {
      const m = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(String(hex).trim())
      if (!m) return null
      const h = m[1].length === 3 ? [...m[1]].map(c => c + c).join('') : m[1]
      return [0, 2, 4].map(i => parseInt(h.slice(i, i + 2), 16))
    }
    const lum = c => {
      const [r, g, b] = c.map(v => (v / 255 <= 0.03928 ? v / 255 / 12.92 : ((v / 255 + 0.055) / 1.055) ** 2.4))
      return 0.2126 * r + 0.7152 * g + 0.0722 * b
    }
    let c = rgb(colour)
    const g = rgb(ground)
    if (!c || !g) return colour
    const reads = x => (lum(g) + 0.05) / (lum(x) + 0.05) >= ratio
    if (reads(c)) return colour
    while (!reads(c) && c.some(v => v > 0)) c = c.map(v => Math.floor(v * 0.94))
    return `#${c.map(v => v.toString(16).padStart(2, '0')).join('')}`
  }

  // The readings in two rows, as the taskbar has its clock's time over the
  // date: the machine's (CPU, memory, battery) over the network's, the
  // hardware's sensors after them, as many over as under (one more over);
  // with only one kind, half over half.
  function rowsOf(parts) {
    const isNet = p => p.icon === 'down' || p.icon === 'up'
    const net = parts.filter(isNet)
    const own = parts.filter(p => !isNet(p) && !p.kind)
    const sensors = parts.filter(p => !isNet(p) && p.kind)
    if (net.length && own.length) {
      const over = Math.max(0, Math.ceil(parts.length / 2) - own.length)
      return [[...own, ...sensors.slice(0, over)], [...net, ...sensors.slice(over)]]
    }
    const all = net.length ? [...net, ...sensors] : [...own, ...sensors]
    const half = Math.ceil(all.length / 2)
    return [all.slice(0, half), all.slice(half)].filter(row => row.length)
  }

  // A reading that wants a look: the CPU or the memory at 90% or more, the
  // battery (not charging) at 20% or less; a sensor's load at 90% or more,
  // its temperature at 90 °C or more, or from its own warning on (a drive's
  // warn, hw.rs: 84 °C for one, 90 for another).
  function isHigh(p) {
    const n = parseFloat(p.text)
    if (p.kind === 'temp' && Number.isFinite(p.warn)) return n >= p.warn
    if (p.kind) return (p.kind === 'load' || p.kind === 'temp') && n >= 90
    if (p.icon === 'cpu' || p.icon === 'memory') return n >= 90
    return p.icon === 'battery' && n <= 20
  }

  // The rows as markup (the taskbar's): each reading its cell, in the
  // text's own colour. The battery not: the tray has Windows' own, beside
  // the network's and the sound's.
  // One reading a row (a side bar's one column), or in two rows.
  const rowsHTML = (w, column = false) =>
    (column ? parts => parts.map(p => [p]) : rowsOf)((partsOf(w) || []).filter(p => p.icon !== 'battery' && p.icon !== 'bolt'))
      .map(row => `<span class="wrow">${row.map(cellHTML).join('')}</span>`)
      .join('')

  // --- On the taskbar (island.js) -----------------------------------------------------
  // The taskbar's layout (taskbarLayout) is three runs, { left, center,
  // right }, of its parts by name ('start', 'clock', …) and of the widgets
  // put there, each { id, type, of, size, name, icon }: type 'widget' (of: a
  // widget's id), 'plugin' (of: a plugin's: its widgets, in turn), or a
  // spacer: 'flex' (takes the room there is), 'gap', 'sep' (a line); size
  // 'S' (its icon and value) or 'M' (its name over its value, as the clock
  // has the date under the time). name and icon: as it was put there, for
  // while it has nothing to say. Or several in one place (the spec's §5):
  // 'carousel' (they take turns there) or 'stack' (the first shown, all of
  // them opened over it by a press), { id, type, items }, 2 to 8 of them,
  // each { type, of, name, icon } as one put there alone, none twice, shown
  // medium. settings.rs checks the same.
  const ENTRY_TYPES = ['widget', 'plugin', 'flex', 'gap', 'sep', 'carousel', 'stack']
  const GROUP_TYPES = ['carousel', 'stack']
  const MAX_ENTRIES = 32
  const MAX_MEMBERS = 8
  const isOf = of => typeof of === 'string' && /^[a-z0-9_-]{1,40}$/.test(of)
  const isName = n => n === undefined || (typeof n === 'string' && [...n].length <= 40)
  const isIcon = i => i === undefined || (typeof i === 'string' && i in ICONS)
  const isGroup = e => !!e && GROUP_TYPES.includes(e.type)
  // One of a group's: a widget's or a plugin's, nothing more.
  const isMember = m =>
    !!m && typeof m === 'object' && (m.type === 'widget' || m.type === 'plugin') && isOf(m.of) && isName(m.name) && isIcon(m.icon) && Object.keys(m).every(k => ['type', 'of', 'name', 'icon'].includes(k))
  const memberKey = m => `${m.type}:${m.of}`
  const isEntry = e =>
    !!e &&
    typeof e === 'object' &&
    typeof e.id === 'string' &&
    /^w[a-z0-9]{1,15}$/.test(e.id) &&
    ENTRY_TYPES.includes(e.type) &&
    (e.type === 'widget' || e.type === 'plugin' ? isOf(e.of) : e.of === undefined) &&
    (isGroup(e)
      ? Array.isArray(e.items) && e.items.length >= 2 && e.items.length <= MAX_MEMBERS && e.items.every(isMember) && new Set(e.items.map(memberKey)).size === e.items.length && e.size === undefined
      : e.items === undefined) &&
    (e.size === undefined || e.size === 'S' || e.size === 'M') &&
    isName(e.name) &&
    isIcon(e.icon)
  const isSource = e => e.type === 'widget' || e.type === 'plugin'
  // A part by its name, a widget by its id.
  const keyOf = item => (typeof item === 'string' ? item : item.id)

  // A layout as kept, made whole: each of the parts once, and one missing
  // (new since) where the default has it, but for those that may be taken
  // off (optional: the monitor's, #53), missing from one kept because they
  // were; each widget once, 32 at most.
  function layoutOf(saved, parts, base, optional = []) {
    const kept = !!saved && typeof saved === 'object' && ['left', 'center', 'right'].some(k => Array.isArray(saved[k]))
    const seen = new Set()
    let entries = 0
    const keep = p => {
      if (typeof p === 'string') return parts.includes(p) && !seen.has(p) && !!seen.add(p)
      if (!isEntry(p) || seen.has(p.id) || entries >= MAX_ENTRIES) return false
      entries++
      return !!seen.add(p.id)
    }
    const copy = p => (typeof p === 'string' ? p : isGroup(p) ? { ...p, items: p.items.map(m => ({ ...m })) } : { ...p })
    const run = k => (Array.isArray(saved?.[k]) ? saved[k] : []).filter(keep).map(copy)
    const out = { left: run('left'), center: run('center'), right: run('right') }
    for (const [k, list] of Object.entries(base)) for (const p of list) if (!seen.has(p) && !(kept && optional.includes(p))) out[k].push(p)
    return out
  }

  // The monitor on the strip is its part (its readings in two rows, as the
  // clock has its two: island-right.js widgetFaces), never besides a widget put
  // there for it (#53, and §7: a widget in one place). One put there alone
  // (a layout of before) becomes the part where it stood; one in a group
  // stays there and the part goes.
  function monitorAsPart(layout) {
    const isMon = p => typeof p !== 'string' && p?.type === 'widget' && p.of === 'monitor'
    const runs = Object.keys(layout)
    const inGroup = runs.some(k => layout[k].some(p => isGroup(p) && p.items.some(m => m.type === 'widget' && m.of === 'monitor')))
    let first = null
    for (const k of runs) {
      const at = layout[k].findIndex(isMon)
      if (at >= 0 && !first) first = { k, at }
    }
    if (!first && !inGroup) return layout
    const out = {}
    for (const k of runs)
      out[k] = layout[k].flatMap((p, i) => {
        if (isMon(p)) return !inGroup && first.k === k && first.at === i ? ['monitor'] : []
        return p === 'monitor' ? [] : [p]
      })
    return out
  }

  // A layout for the top bar (island.js split) from the one kept: Start and
  // the programs (and quick settings and the clock, if set so) left out,
  // they are in the dock (dock: those parts). One kept for the strip along
  // an edge (with them in it) that has her at its start and nothing else in
  // the middle, as Windows 11's (the default), has her in the middle; one
  // edited on the top bar is as it was left. The program in front, the top
  // bar's own part, first at the left where it has not been put elsewhere.
  function splitOf(layout, dock) {
    const runs = ['left', 'center', 'right']
    const forEdge = runs.some(k => Array.isArray(layout?.[k]) && layout[k].some(p => p === 'start' || p === 'apps'))
    const run = k => (Array.isArray(layout?.[k]) ? layout[k] : []).filter(p => !dock.includes(p))
    const out = { left: run('left'), center: run('center'), right: run('right') }
    if (forEdge && out.left[0] === 'her' && !out.center.length) out.center.push(out.left.shift())
    if (!runs.some(k => out[k].includes('front'))) out.left.unshift('front')
    return out
  }
  // The other way: a layout for the strip along an edge from one kept on
  // the top bar (with neither Start nor the programs in it: they were in
  // the dock). The program in front is the top bar's (put there by splitOf,
  // moved about there): taken out, the strip was never given it; her alone
  // in the middle, where splitOf moved her from the left's start, is there
  // again; Start and the programs come back where the default has them
  // (layoutOf). One kept for the strip itself is as it was, the program in
  // front where it was put, if it was.
  function edgeOf(layout) {
    const kept = ['left', 'center', 'right'].filter(k => Array.isArray(layout?.[k]))
    if (!kept.length || kept.some(k => layout[k].some(p => p === 'start' || p === 'apps'))) return layout
    const out = { ...layout }
    for (const k of kept) out[k] = layout[k].filter(p => p !== 'front')
    if (out.center?.length === 1 && out.center[0] === 'her') {
      out.center = []
      out.left = ['her', ...(out.left || [])]
    }
    return out
  }

  // --- Several in one place: made, changed, taken apart -----------------------------
  // Each takes a layout and gives a new one (the one given left as it was);
  // newId makes an entry's id (island.js: "w" and a-z 0-9).
  const cloneLayout = layout => JSON.parse(JSON.stringify(layout))
  const findEntry = (layout, id) => {
    for (const k of Object.keys(layout)) {
      const at = layout[k].findIndex(p => typeof p !== 'string' && p.id === id)
      if (at >= 0) return { run: k, at, entry: layout[k][at] }
    }
    return null
  }
  // Its members: a group's, or the one a widget alone is.
  const membersOf = e => (isGroup(e) ? e.items : isSource(e) ? [{ type: e.type, of: e.of, ...(e.name ? { name: e.name } : {}), ...(e.icon ? { icon: e.icon } : {}) }] : [])
  const alone = (m, id) => ({ id, type: m.type, of: m.of, size: 'M', ...(m.name ? { name: m.name } : {}), ...(m.icon ? { icon: m.icon } : {}) })

  // One (a widget alone, or a group) put with another where that one is: a
  // group of the two (a widget there alone becomes one, of `type`), or into
  // the group there (its own type kept), 8 at most, none twice. Unchanged
  // when either is not a widget or a group, or it is itself.
  function groupWith(layout, id, intoId, type, newId) {
    if (id === intoId || !GROUP_TYPES.includes(type)) return layout
    const out = cloneLayout(layout)
    const from = findEntry(out, id)
    const into = findEntry(out, intoId)
    if (!from || !into || !membersOf(from.entry).length || !membersOf(into.entry).length) return layout
    const items = [...membersOf(into.entry)]
    for (const m of membersOf(from.entry)) if (items.length < MAX_MEMBERS && !items.some(x => memberKey(x) === memberKey(m))) items.push(m)
    if (items.length < 2) return layout
    const group = isGroup(into.entry) ? { ...into.entry, items } : { id: newId(), type, items }
    out[into.run][into.at] = group
    const left = findEntry(out, id)
    out[left.run].splice(left.at, 1)
    return out
  }

  // A group taken apart: its members each alone where it was, in its order.
  function ungroup(layout, id, newId) {
    const out = cloneLayout(layout)
    const at = findEntry(out, id)
    if (!at || !isGroup(at.entry)) return layout
    out[at.run].splice(at.at, 1, ...at.entry.items.map(m => alone(m, newId())))
    return out
  }

  // One member taken out of a group (dragged out of a stack): alone, just
  // after it; one member left, the group is that one alone.
  function takeOut(layout, id, index, newId) {
    const out = cloneLayout(layout)
    const at = findEntry(out, id)
    if (!at || !isGroup(at.entry) || !at.entry.items[index]) return layout
    const [m] = at.entry.items.splice(index, 1)
    const rest = at.entry.items.length === 1 ? alone(at.entry.items[0], newId()) : at.entry
    out[at.run].splice(at.at, 1, rest, alone(m, newId()))
    return out
  }

  // One member gone from a group (off the taskbar); one left, it alone.
  function dropMember(layout, id, index, newId) {
    const out = cloneLayout(layout)
    const at = findEntry(out, id)
    if (!at || !isGroup(at.entry) || !at.entry.items[index]) return layout
    at.entry.items.splice(index, 1)
    if (at.entry.items.length === 1) out[at.run][at.at] = alone(at.entry.items[0], newId())
    return out
  }

  // A group's members in another order: the one at `from` put at `to`.
  function reorder(layout, id, from, to) {
    const out = cloneLayout(layout)
    const at = findEntry(out, id)
    const items = at && isGroup(at.entry) ? at.entry.items : null
    if (!items || !items[from] || to < 0 || to >= items.length || from === to) return layout
    items.splice(to, 0, ...items.splice(from, 1))
    return out
  }

  // A group made the other kind (a carousel a stack, or back).
  function regroup(layout, id, type) {
    const out = cloneLayout(layout)
    const at = findEntry(out, id)
    if (!at || !isGroup(at.entry) || !GROUP_TYPES.includes(type)) return layout
    at.entry.type = type
    return out
  }

  // Its parts alone: what a preset says.
  const partsAlone = layout => Object.fromEntries(Object.entries(layout).map(([k, run]) => [k, run.filter(p => typeof p === 'string')]))

  // A preset's parts, and the widgets where they were: in their run, after
  // its parts (before them in the right run, not past the clock).
  function withParts(preset, old) {
    const out = {}
    for (const k of ['left', 'center', 'right']) {
      const mine = (Array.isArray(old?.[k]) ? old[k] : []).filter(p => typeof p !== 'string')
      const parts = [...(preset[k] || [])]
      out[k] = k === 'right' ? [...mine, ...parts] : [...parts, ...mine]
    }
    return out
  }

  // What a widget put there shows, of the widgets there are now: the one,
  // or a plugin's; a group's, each of its members'.
  const sourcesOf = (entry, all) =>
    entry.type === 'widget' ? all.filter(w => w.id === entry.of) : entry.type === 'plugin' ? all.filter(w => w.plugin === entry.of) : isGroup(entry) ? entry.items.flatMap(m => sourcesOf(m, all)) : []

  // A group's slides: each widget of each member, in the members' order;
  // a member with nothing to say one slide still, without a widget (its
  // name greyed).
  const slidesOf = (entry, all) =>
    membersOf(entry).flatMap(m => {
      const from = sourcesOf(m, all)
      return from.length ? from.map(w => ({ member: m, w })) : [{ member: m, w: null }]
    })

  // --- A carousel's turns (the spec's §5, §9) ------------------------------------------
  // Every CAROUSEL_MS the next slides up, while it may move: not under the
  // pointer, not while nobody is about, never with motion reduced (island.js
  // holds it then and calls nothing). A slide with news (newsIn) cuts in at
  // once, for a turn of its own, and the turns go on from where they were.
  // { at, since, back }: the slide shown, since when, and where to go back
  // to after news (-1 none).
  const CAROUSEL_MS = 6000
  const carouselStart = now => ({ at: 0, since: now, back: -1 })
  function carouselTurn(state, n, now, news = -1) {
    if (n < 2) return { at: 0, since: state.since, back: -1 }
    const at = Math.min(state.at, n - 1)
    if (news >= 0 && news < n && news !== at) return { at: news, since: now, back: state.back >= 0 ? state.back : (at + 1) % n }
    if (now - state.since < CAROUSEL_MS) return { ...state, at }
    return { at: state.back >= 0 && state.back < n ? state.back : (at + 1) % n, since: now, back: -1 }
  }
  // Turned by hand (the wheel): the next or the one before, its turn anew.
  const carouselStep = (state, n, by, now) => ({ at: n ? (((state.at + by) % n) + n) % n : 0, since: now, back: -1 })
  // How long until it turns by itself.
  const carouselDue = (state, now) => Math.max(0, CAROUSEL_MS - (now - state.since))

  // News on a slide: a mailbox's unread grew (a letter came); its own
  // nudge (island.js) is news too. A clock or a reading changing is not.
  function newsIn(before, after) {
    if (!before || !after || after.from !== 'mail') return false
    const n = w => Number(w?.value?.vars?.n ?? (typeof w?.value === 'string' ? parseInt(w.value, 10) : NaN))
    return n(after) > (Number.isFinite(n(before)) ? n(before) : Infinity)
  }

  // The widgets the layout shows (not to take their turns in the island too).
  function shownOn(layout, all) {
    const ids = new Set()
    for (const run of Object.values(layout)) for (const e of run) if (typeof e !== 'string') for (const w of sourcesOf(e, all)) ids.add(w.id)
    return ids
  }

  // A plugin's name: one of hers by her words for it (w.ex.<id>); someone
  // else's (addons.rs: its widgets' label is its name until it gives one) by
  // its first widget's label, a private one's never; '' for one she does
  // not know.
  function pluginName(lang, id, all = []) {
    const key = `w.ex.${id}`
    const hers = render(lang, { key })
    if (hers !== key) return hers
    const w = all.find(x => x.plugin === id && !x.private && typeof x.label === 'string' && x.label)
    return w ? w.label : ''
  }

  // Its name: a plugin's, a built-in widget's, a spacer's; another widget's
  // label (as it was put there, when gone).
  function nameOf(lang, entry, all = [], detailed = true) {
    if (entry.type === 'plugin') return pluginName(lang, entry.of, all) || entry.name || render(lang, { key: 'taskbar.w.widget' })
    // A group: its kind and how many ("叠放 · 3 个").
    if (isGroup(entry)) return render(lang, { key: `taskbar.w.${entry.type}`, vars: { n: String(entry.items.length) } })
    if (entry.type !== 'widget') return render(lang, { key: `taskbar.w.${entry.type}` })
    const w = all.find(x => x.id === entry.of)
    if (w) return words(lang, w, detailed).label || render(lang, { key: 'taskbar.w.widget' })
    return entry.name || render(lang, { key: 'taskbar.w.widget' })
  }

  const api = {
    ICONS,
    icon,
    hasIcon,
    maskUrl,
    hasPicture,
    maskHTML,
    markHTML,
    iconHTML,
    pluginName,
    detailOf,
    sizesOf,
    words,
    partsOf,
    partsHTML,
    rowsOf,
    isHigh,
    rowsHTML,
    shareOf,
    glyphOf,
    deepen,
    ENTRY_TYPES,
    GROUP_TYPES,
    isEntry,
    isSource,
    isGroup,
    isMember,
    memberKey,
    membersOf,
    keyOf,
    layoutOf,
    monitorAsPart,
    splitOf,
    edgeOf,
    partsAlone,
    withParts,
    findEntry,
    groupWith,
    ungroup,
    takeOut,
    dropMember,
    reorder,
    regroup,
    sourcesOf,
    slidesOf,
    shownOn,
    nameOf,
    CAROUSEL_MS,
    carouselStart,
    carouselTurn,
    carouselStep,
    carouselDue,
    newsIn,
  }

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api
  } else {
    root.Widgets = api
  }
})(this ?? window)
