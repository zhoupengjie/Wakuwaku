// Widgets, the first kind of plugin in the island (widgets.rs): their icons,
// and their words in her language. Loaded by the page before island.js, and
// required by the tests.
//
// A widget is { id, label, value, icon, color, builtIn, on, leftMs, private };
// a built-in one's label and value are { key, vars } to translate. A private
// one's label stays off screen while the specifics are (details off). The
// monitor's value is { parts: [{ icon, text }] }: shown as icon and number.
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
  // charging too (BatteryCharging0 to BatteryCharging10).
  const PART_GLYPH = { cpu: 'E950', memory: 'E964', down: 'EDDC', up: 'EDDB' }
  const BATTERY = ['E850', 'E851', 'E852', 'E853', 'E854', 'E855', 'E856', 'E857', 'E858', 'E859', 'E83F']
  const CHARGING = ['E85A', 'E85B', 'E85C', 'E85D', 'E85E', 'E85F', 'E860', 'E861', 'E862', 'E863', 'E83E']
  const esc = value => String(value ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c])

  // The monitor's readings, or null for a widget that has none.
  const partsOf = w => (Array.isArray(w?.value?.parts) ? w.value.parts : null)

  // The icon as markup in the colour of what surrounds it: Windows' own
  // glyph where it has one, else its strokes.
  const icon = name =>
    GLYPHS[name]
      ? `<span class="wicon wglyph" aria-hidden="true">&#x${GLYPHS[name]};</span>`
      : `<svg class="wicon" viewBox="0 0 24 24" aria-hidden="true">${ICONS[name] || ICONS.dot}</svg>`

  // What a widget says: its label and its value, in her language; a private
  // one's label not, unless the specifics may show.
  const words = (lang, w, detailed = true) => ({
    label: w.private && !detailed ? '' : render(lang, w.label),
    value: partsOf(w) ? partsOf(w).map(p => p.text).join(' ') : render(lang, w.value),
  })

  // How full a reading is, 0 to 100. The processor, memory and the battery
  // are shares already ("23%"). The network's rate ("5.1K", "337K", "48M",
  // widgets.rs rate) has no 100%: by its order of magnitude, 1 KB/s empty
  // and 100 MB/s full, each tenfold a fifth, so a rate is always as full.
  function shareOf(p) {
    const n = parseFloat(p.text)
    if (!Number.isFinite(n)) return 0
    if (p.icon !== 'down' && p.icon !== 'up') return Math.max(0, Math.min(100, Math.round(n)))
    const bytes = n * ({ K: 1e3, M: 1e6, G: 1e9 }[String(p.text).trim().slice(-1)] || 1)
    return Math.max(0, Math.min(100, Math.round(((Math.log10(Math.max(bytes, 1e3)) - 3) / 5) * 100)))
  }

  // A reading's glyph: Windows' own, the battery's by its level.
  function glyphOf(p) {
    if (p.icon === 'battery' || p.icon === 'bolt') return (p.icon === 'bolt' ? CHARGING : BATTERY)[Math.round(Math.max(0, Math.min(100, parseFloat(p.text) || 0)) / 10)]
    return PART_GLYPH[p.icon] || PART_GLYPH.cpu
  }

  // A reading as markup: Windows' glyph and the number in one small cell,
  // its ground filled from the left as far as the reading goes (style.css
  // .wcell), one that wants a look marked high.
  const cellHTML = p =>
    `<span class="wcell${isHigh(p) ? ' high' : ''}" style="--p:${shareOf(p)}%"><span class="wg${p.icon === 'down' || p.icon === 'up' ? ' tri' : ''}" aria-hidden="true">&#x${glyphOf(p)};</span><b>${esc(p.text)}</b></span>`

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
  // date: the machine's (CPU, memory, battery) over the network's; with only
  // one kind, half over half.
  function rowsOf(parts) {
    const isNet = p => p.icon === 'down' || p.icon === 'up'
    const net = parts.filter(isNet)
    const rest = parts.filter(p => !isNet(p))
    if (net.length && rest.length) return [rest, net]
    const all = net.length ? net : rest
    const half = Math.ceil(all.length / 2)
    return [all.slice(0, half), all.slice(half)].filter(row => row.length)
  }

  // A reading that wants a look: the CPU or the memory at 90% or more, the
  // battery (not charging) at 20% or less.
  function isHigh(p) {
    const n = parseFloat(p.text)
    if (p.icon === 'cpu' || p.icon === 'memory') return n >= 90
    return p.icon === 'battery' && n <= 20
  }

  // The rows as markup (the taskbar's): each reading its cell, in the
  // text's own colour. The battery not: the tray has Windows' own, beside
  // the network's and the sound's.
  const rowsHTML = w =>
    rowsOf((partsOf(w) || []).filter(p => p.icon !== 'battery' && p.icon !== 'bolt'))
      .map(row => `<span class="wrow">${row.map(cellHTML).join('')}</span>`)
      .join('')

  // --- On the taskbar (island.js) -----------------------------------------------------
  // The taskbar's layout (taskbarLayout) is three runs, { left, center,
  // right }, of its parts by name ('start', 'clock', …) and of the widgets
  // put there, each { id, type, of, size, name, icon, cfg }: type 'widget'
  // (of: a widget's id), 'plugin' (of: a plugin's: its widgets, in turn),
  // 'tool' (of: one of the taskbar's own, tools.js; cfg: its settings),
  // 'stack' (items: 2 to 8 of those, one shown at a time; smart: turning by
  // itself to the one with news), or a spacer: 'flex' (takes the room
  // there is), 'gap', 'sep' (a line); size
  // 'S' (its icon and value) or 'M' (its name over its value, as the clock
  // has the date under the time). name and icon: as it was put there, for
  // while it has nothing to say. settings.rs checks the same.
  const ENTRY_TYPES = ['widget', 'plugin', 'tool', 'stack', 'flex', 'gap', 'sep']
  // A tool's settings: up to 8, a word each for a short text, a number or a flag.
  const isCfg = c =>
    !!c &&
    typeof c === 'object' &&
    !Array.isArray(c) &&
    Object.keys(c).length <= 8 &&
    Object.entries(c).every(
      ([k, v]) => /^[A-Za-z0-9]{1,16}$/.test(k) && (typeof v === 'boolean' || (typeof v === 'number' && Number.isFinite(v)) || (typeof v === 'string' && [...v].length <= 300 && !/[\u0000-\u001f\u007f-\u009f]/.test(v))),
    )
  const MAX_ENTRIES = 32
  const isEntry = e =>
    !!e &&
    typeof e === 'object' &&
    typeof e.id === 'string' &&
    /^w[a-z0-9]{1,15}$/.test(e.id) &&
    ENTRY_TYPES.includes(e.type) &&
    (e.type === 'stack'
      ? Array.isArray(e.items) && e.items.length >= 2 && e.items.length <= 8 && e.items.every(i => isEntry(i) && isOne(i)) && new Set(e.items.map(i => i.id)).size === e.items.length && (e.smart === undefined || typeof e.smart === 'boolean')
      : e.items === undefined && e.smart === undefined) &&
    (e.type === 'widget' || e.type === 'plugin' || e.type === 'tool' ? typeof e.of === 'string' && /^[a-z0-9_-]{1,40}$/.test(e.of) : e.of === undefined) &&
    (e.cfg === undefined || (e.type === 'tool' && isCfg(e.cfg))) &&
    (e.size === undefined || e.size === 'S' || e.size === 'M') &&
    (e.name === undefined || (typeof e.name === 'string' && [...e.name].length <= 40)) &&
    (e.icon === undefined || (typeof e.icon === 'string' && e.icon in ICONS))
  // One thing shown (what a stack holds), or that and a stack: with a size, not a spacer.
  const isOne = e => e.type === 'widget' || e.type === 'plugin' || e.type === 'tool'
  const isSource = e => isOne(e) || e.type === 'stack'
  // A stack's items, or the one.
  const itemsOf = e => (e.type === 'stack' ? e.items : [e])
  // Every widget in a layout, those in stacks too.
  const entriesOf = layout => Object.values(layout).flatMap(run => run.filter(p => typeof p !== 'string')).flatMap(e => [e, ...(e.type === 'stack' ? e.items : [])])
  // A part by its name, a widget by its id.
  const keyOf = item => (typeof item === 'string' ? item : item.id)

  // A layout as kept, made whole: each of the parts once, and one missing
  // (new since) where the default has it; each widget once, 32 at most.
  function layoutOf(saved, parts, base) {
    const seen = new Set()
    let entries = 0
    const keep = p => {
      if (typeof p === 'string') return parts.includes(p) && !seen.has(p) && !!seen.add(p)
      const ids = [p.id, ...(p?.type === 'stack' && Array.isArray(p.items) ? p.items.map(i => i?.id) : [])]
      if (!isEntry(p) || ids.some(id => seen.has(id)) || entries + ids.length - (p.type === 'stack' ? 1 : 0) > MAX_ENTRIES) return false
      entries += ids.length - (p.type === 'stack' ? 1 : 0)
      ids.forEach(id => seen.add(id))
      return true
    }
    const copy = p => (typeof p === 'string' ? p : p.type === 'stack' ? { ...p, items: p.items.map(i => ({ ...i })) } : { ...p })
    const run = k => (Array.isArray(saved?.[k]) ? saved[k] : []).filter(keep).map(copy)
    const out = { left: run('left'), center: run('center'), right: run('right') }
    for (const [k, list] of Object.entries(base)) for (const p of list) if (!seen.has(p)) out[k].push(p)
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
  // or a plugin's.
  const sourcesOf = (entry, all) =>
    entry.type === 'stack' ? entry.items.flatMap(i => sourcesOf(i, all)) : entry.type === 'widget' ? all.filter(w => w.id === entry.of) : entry.type === 'plugin' ? all.filter(w => w.plugin === entry.of) : []

  // The widgets the layout shows (not to take their turns in the island too).
  function shownOn(layout, all) {
    const ids = new Set()
    for (const run of Object.values(layout)) for (const e of run) if (typeof e !== 'string') for (const w of sourcesOf(e, all)) ids.add(w.id)
    return ids
  }

  // Its name: a plugin's, a built-in widget's, a spacer's; another widget's
  // label (as it was put there, when gone).
  function nameOf(lang, entry, all = [], detailed = true) {
    if (entry.type === 'plugin') return render(lang, { key: `w.ex.${entry.of}` })
    if (entry.type === 'tool') return render(lang, { key: `tool.${entry.of}` })
    if (entry.type === 'stack') return render(lang, { key: 'taskbar.w.stack', vars: { n: entry.items.length } })
    if (entry.type !== 'widget') return render(lang, { key: `taskbar.w.${entry.type}` })
    const w = all.find(x => x.id === entry.of)
    if (w) return words(lang, w, detailed).label || render(lang, { key: 'taskbar.w.widget' })
    return entry.name || render(lang, { key: 'taskbar.w.widget' })
  }

  const api = {
    ICONS,
    icon,
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
    isEntry,
    isOne,
    isSource,
    itemsOf,
    entriesOf,
    keyOf,
    layoutOf,
    partsAlone,
    withParts,
    sourcesOf,
    shownOn,
    nameOf,
  }

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api
  } else {
    root.Widgets = api
  }
})(this)
