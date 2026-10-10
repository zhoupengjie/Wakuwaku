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

  // Each monitor part in a colour of its own.
  const PART_COLOR = { cpu: '#5e9bff', memory: '#b18cff', down: '#64d2ff', up: '#64d2ff', battery: '#34d27b', bolt: '#34d27b' }
  const esc = value => String(value ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c])

  // The monitor's readings, or null for a widget that has none.
  const partsOf = w => (Array.isArray(w?.value?.parts) ? w.value.parts : null)

  // The icon as markup: its own strokes, in the colour of what surrounds it.
  const icon = name => `<svg class="wicon" viewBox="0 0 24 24" aria-hidden="true">${ICONS[name] || ICONS.dot}</svg>`

  // What a widget says: its label and its value, in her language; a private
  // one's label not, unless the specifics may show.
  const words = (lang, w, detailed = true) => ({
    label: w.private && !detailed ? '' : render(lang, w.label),
    value: partsOf(w) ? partsOf(w).map(p => p.text).join(' ') : render(lang, w.value),
  })

  // The monitor's readings as markup: each icon and number in its colour.
  const partsHTML = w =>
    (partsOf(w) || []).map(p => `<span class="wpart" style="color:${PART_COLOR[p.icon] || 'inherit'}">${icon(p.icon)}<b>${esc(p.text)}</b></span>`).join('')

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

  // The rows as markup, uncoloured: the page colours them (the taskbar: its
  // icons in Windows' accent colour, its numbers plain), one that wants a
  // look marked high.
  const rowsHTML = w =>
    rowsOf(partsOf(w) || [])
      .map(row => `<span class="wrow">${row.map(p => `<span class="wpart${isHigh(p) ? ' high' : ''}">${icon(p.icon)}<b>${esc(p.text)}</b></span>`).join('')}</span>`)
      .join('')

  const api = { ICONS, icon, words, partsOf, partsHTML, rowsOf, isHigh, rowsHTML }

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api
  } else {
    root.Widgets = api
  }
})(this)
