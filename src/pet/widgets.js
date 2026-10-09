// Widgets, the first kind of plugin in the island (widgets.rs): their icons,
// and their words in her language. Loaded by the page before island.js, and
// required by the tests.
//
// A widget is { id, label, value, icon, color, builtIn, on, leftMs }; a
// built-in one's label and value are { key, vars } to translate.
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
  }

  // The icon as markup: its own strokes, in the colour of what surrounds it.
  const icon = name => `<svg class="wicon" viewBox="0 0 24 24" aria-hidden="true">${ICONS[name] || ICONS.dot}</svg>`

  // What a widget says: its label and its value, in her language.
  const words = (lang, w) => ({ label: render(lang, w.label), value: render(lang, w.value) })

  const api = { ICONS, icon, words }

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api
  } else {
    root.Widgets = api
  }
})(this)
