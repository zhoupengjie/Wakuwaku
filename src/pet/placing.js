// Putting widgets in places from the picker (picker.js), as plain data for
// the tests: where on the taskbar's strip one goes, whether the strip has
// room for it, how one taken off is put back (undo), and which widgets and
// plugins are on (placed is on: put somewhere, it runs; taken from its last
// place, it stops; no switch of its own, the Plugins page's aside).
;(function (root) {
  const RUNS = ['left', 'center', 'right']
  const keyOf = item => (item.plugin ? `plugin:${item.plugin}` : `widget:${item.id}`)
  const itemOf = key => (key.startsWith('plugin:') ? { plugin: key.slice(7) } : { id: key.slice(key.indexOf(':') + 1) })
  const clone = layout => JSON.parse(JSON.stringify(layout))
  const isWidgetEntry = p => !!p && typeof p === 'object' && (p.type === 'widget' || p.type === 'plugin')
  const isGroupEntry = p => !!p && typeof p === 'object' && (p.type === 'carousel' || p.type === 'stack')
  const membersOf = p => (isWidgetEntry(p) ? [p] : isGroupEntry(p) ? p.items : [])

  // --- On the strip --------------------------------------------------------------------
  // Where one let go of at x (along the strip) goes: the run under it (or
  // the nearest), before the first of its parts whose middle is past x. Its
  // front half: before it; its back half: after it; between two: after the
  // one before. runs: [{ run, lo, hi, items: [{ lo, hi }] }], what is there
  // but the one dragged, in order.
  function slotAt(runs, x) {
    let near = null
    for (const r of runs) {
      const d = x < r.lo ? r.lo - x : x > r.hi ? x - r.hi : 0
      if (!near || d < near.d) near = { r, d }
    }
    if (!near) return { run: 'right', index: 0 }
    const items = near.r.items
    const at = items.findIndex(it => x < it.lo + (it.hi - it.lo) / 2)
    return { run: near.r.run, index: at < 0 ? items.length : at }
  }

  // A press on a card: just before the tray (the tray's icons part), else
  // at the end of the right run.
  function beforeTray(layout) {
    for (const run of RUNS) {
      const at = (layout[run] || []).indexOf('tray')
      if (at >= 0) return { run, index: at }
    }
    return { run: 'right', index: (layout.right || []).length }
  }

  // The layout with an entry (or a part) put in a run at an index (clamped).
  function insertAt(layout, run, index, entry) {
    const out = clone(layout)
    const list = out[run] || (out[run] = [])
    list.splice(Math.max(0, Math.min(index, list.length)), 0, entry)
    return out
  }

  // Whether the strip has room for one more this wide (used: what its runs
  // take now, room: its length): as they are, or with medium ones made
  // small (the most room they give first: shrinkable [{ id, width }], each
  // giving width - small), or not at all. { ok, shrink: [ids] }.
  function fitStrip({ room, used, adding, shrinkable = [], small = 64 }) {
    let over = used + adding - room
    if (over <= 0) return { ok: true, shrink: [] }
    const shrink = []
    for (const s of [...shrinkable].sort((a, b) => b.width - a.width)) {
      const gives = s.width - small
      if (gives <= 0) continue
      shrink.push(s.id)
      over -= gives
      if (over <= 0) return { ok: true, shrink }
    }
    return { ok: false, shrink: [] }
  }

  // Those entries given a size (S: small, M: medium).
  function withSizes(layout, ids, size) {
    const out = clone(layout)
    for (const run of RUNS) for (const p of out[run] || []) if (isWidgetEntry(p) && ids.includes(p.id)) p.size = size
    return out
  }

  // --- Taken off, put back (undo) ------------------------------------------------------
  // Where an entry (by its id) or a part (by its name) is: what to put back
  // and where; null when it is not there.
  function removalOf(layout, key) {
    for (const run of RUNS) {
      const index = (layout[run] || []).findIndex(p => (typeof p === 'string' ? p === key : p.id === key))
      if (index >= 0) return { entry: clone(layout[run][index]), run, index }
    }
    return null
  }
  // The layout without it.
  function without(layout, key) {
    const out = clone(layout)
    for (const run of RUNS) if (out[run]) out[run] = out[run].filter(p => (typeof p === 'string' ? p !== key : p.id !== key))
    return out
  }
  // Put back where it was (its run, its index, or the run's end now); the
  // layout unchanged when it is there again already (by its id, or any of
  // its widgets put back meanwhile: a widget in one place).
  function restoreEntry(layout, record) {
    if (!record?.entry) return layout
    const e = record.entry
    const there = new Set(RUNS.flatMap(run => (layout[run] || []).flatMap(p => (typeof p === 'string' ? [p] : [p.id, ...membersOf(p).map(m => `${m.type}:${m.of}`)]))))
    if (typeof e === 'string' ? there.has(e) : there.has(e.id) || membersOf(e).some(m => there.has(`${m.type}:${m.of}`))) return layout
    return insertAt(layout, RUNS.includes(record.run) ? record.run : 'right', record.index, e)
  }

  // --- Placed is on --------------------------------------------------------------------
  // The widgets and plugins put on the strip (its widgets and groups, the
  // monitor's part: her home the taskbar) and on the desktop, by key.
  function placedKeys({ taskbar = null, desk = [], home = 'taskbar' } = {}) {
    const keys = new Set()
    if (home === 'taskbar' && taskbar) {
      for (const run of RUNS)
        for (const p of taskbar[run] || []) {
          if (p === 'monitor') keys.add('widget:monitor')
          for (const m of membersOf(p)) keys.add(`${m.type}:${m.of}`)
        }
    }
    for (const e of Array.isArray(desk) ? desk : []) {
      const ms = e?.type === 'stack' || e?.type === 'board' ? e.items || [] : [e]
      for (const m of ms) if (m && (m.type === 'widget' || m.type === 'plugin')) keys.add(`${m.type}:${m.of}`)
    }
    return keys
  }

  // The settings that turn one on or off: a plugin by its own (plugins[id].on),
  // a widget by widgetsOff (main.rs is_widget_on). null: already so.
  function switchPatch(config = {}, key, on) {
    const item = itemOf(key)
    if (item.plugin) {
      const plugins = config.plugins && typeof config.plugins === 'object' ? config.plugins : {}
      if ((plugins[item.plugin]?.on === true) === on) return null
      return { plugins: { ...plugins, [item.plugin]: { ...(plugins[item.plugin] || {}), on } } }
    }
    const off = Array.isArray(config.widgetsOff) ? config.widgetsOff : []
    if (off.includes(item.id) !== on) return null
    return { widgetsOff: on ? off.filter(id => id !== item.id) : [...off, item.id] }
  }

  // Several of those, one patch (plugins merged).
  function switchesPatch(config = {}, keys, on) {
    let c = { ...config }
    const patch = {}
    for (const key of keys) {
      const p = switchPatch(c, key, on)
      if (!p) continue
      Object.assign(patch, p)
      c = { ...c, ...p }
    }
    return Object.keys(patch).length ? patch : null
  }

  const api = { RUNS, keyOf, itemOf, slotAt, beforeTray, insertAt, fitStrip, withSizes, removalOf, without, restoreEntry, placedKeys, switchPatch, switchesPatch }
  if (typeof module !== 'undefined' && module.exports) module.exports = api
  else root.Placing = api
})(this)
