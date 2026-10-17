// Her capsule as a container of widgets (the KDE-like picker of 2026-10-17):
// which widgets are in it, in what order, and how each shows there. Plain
// data in, plain data out, for island.js (through deskgrid.js placement),
// picker.js and capsule-panel.js to use and the tests to try.
//
// The setting (settings.json "capsule"):
//   { members: [{ key, mode }], spin? }
//   key   'widget:<id>' or 'plugin:<id>' (a plugin's widgets all in its place)
//   mode  'turn'  takes its turn in the closed capsule (轮播, the default)
//         'pin'   always in sight at the capsule's right end (固定), two at most
//         'open'  only in the open island's list (展开时)
//   spin  seconds a turn lasts (0 / 5 / 8 / 15), as widgetSpin
//
// What is in the capsule is not the list: it is every widget that is on and
// not put elsewhere (the taskbar, the desktop), as it always was ("placed is
// on": put somewhere, it runs; taken from its last place, it is off). The
// list gives their order and their modes; one on and not listed (a script's
// new widget, one turned on on the Plugins page) comes at its end. So the
// capsule reads the same with or without the setting, and the Rust side,
// which keeps no "capsule" yet (phase 2), still shows what the page meant:
// the order is written to widgetOrder too, the monitor's pin to monitor.pin.
//
// Without the setting (a config of before): the members in the widgets'
// order (widgets.rs sorts them by widgetOrder), each taking turns; the
// monitor pinned while its pin is on, in the island's home (the taskbar's
// home: its readings are the strip's monitor part, or, without the part, it
// takes its turns, #53).
;(function (root) {
  const MODES = ['turn', 'pin', 'open']
  const MAX_PINS = 2
  const MAX_MEMBERS = 64
  const SPINS = [0, 5, 8, 15]

  const isKey = k => typeof k === 'string' && /^(widget|plugin):[a-z0-9_-]{1,40}$/.test(k)
  const keyOfWidget = w => (w.plugin ? `plugin:${w.plugin}` : `widget:${w.id}`)
  const idOfKey = key => key.slice(key.indexOf(':') + 1)
  const isMember = m => !!m && typeof m === 'object' && isKey(m.key) && MODES.includes(m.mode)

  // The setting's members as kept: each once, known keys and modes, 64 at most.
  function storedOf(capsule) {
    const list = Array.isArray(capsule?.members) ? capsule.members : null
    if (!list) return null
    const seen = new Set()
    const out = []
    for (const m of list) {
      if (!isMember(m) || seen.has(m.key) || out.length >= MAX_MEMBERS) continue
      seen.add(m.key)
      out.push({ key: m.key, mode: m.mode })
    }
    return out
  }

  // The monitor's mode in the island's home, its pin (the Plugins page's
  // switch, monitor.pin) saying pinned or taking turns; 'open' only by the
  // capsule's own setting.
  function monitorMode(stored, home, pin) {
    if (home !== 'island') return stored || 'turn'
    if (stored === 'open') return 'open'
    return pin !== false ? 'pin' : 'turn'
  }

  // The members without the setting: in the widgets' order, each its key
  // once (a plugin's at its first widget).
  function derive(all = [], { home = 'island', pin } = {}) {
    const seen = new Set()
    const out = []
    for (const w of all) {
      const key = keyOfWidget(w)
      if (seen.has(key)) continue
      seen.add(key)
      out.push({ key, mode: key === 'widget:monitor' ? monitorMode(null, home, pin) : 'turn' })
    }
    return out
  }

  // The members as they are now, each with its widgets: those of the list
  // that are in the capsule (on, not put elsewhere), in its order; then
  // those on and not listed, in the widgets' order. inCapsule(w): whether a
  // widget is (placement decides: on, not on the taskbar or the desktop).
  // Two pinned at most: a third (a list written by hand) takes turns.
  function effective({ capsule, all = [], home = 'island', pin, inCapsule = w => w.on !== false } = {}) {
    const stored = storedOf(capsule)
    const list = stored || derive(all, { home, pin })
    const widgetsOf = new Map()
    for (const w of all) {
      if (!inCapsule(w)) continue
      const key = keyOfWidget(w)
      if (!widgetsOf.has(key)) widgetsOf.set(key, [])
      widgetsOf.get(key).push(w)
    }
    const out = []
    const seen = new Set()
    const take = (key, mode) => {
      if (seen.has(key) || !widgetsOf.has(key)) return
      seen.add(key)
      out.push({ key, mode: key === 'widget:monitor' ? monitorMode(stored ? mode : null, home, pin) : mode, widgets: widgetsOf.get(key) })
    }
    for (const m of list) take(m.key, m.mode)
    for (const key of widgetsOf.keys()) take(key, 'turn')
    let pins = 0
    for (const m of out) if (m.mode === 'pin' && ++pins > MAX_PINS) m.mode = 'turn'
    return out
  }

  // --- Changed by hand (the panel, the open island's menu, the picker) ---------------------
  // Each takes the list as it is (members, the effective ones or the
  // stored) and gives a new one, { key, mode } each.
  const plain = list => list.map(m => ({ key: m.key, mode: m.mode }))
  const pinsIn = (list, but) => list.filter(m => m.mode === 'pin' && m.key !== but).length

  // Whether one may be pinned: two others pinned already, not.
  const canPin = (list, key) => pinsIn(list, key) < MAX_PINS

  // Its mode set; a third pinned refused (the list as it was).
  function setMode(list, key, mode) {
    if (!MODES.includes(mode) || !list.some(m => m.key === key)) return plain(list)
    if (mode === 'pin' && !canPin(list, key)) return plain(list)
    return list.map(m => ({ key: m.key, mode: m.key === key ? mode : m.mode }))
  }

  // The one at `from` put at `to` (the rows dragged in the panel).
  function move(list, from, to) {
    const out = plain(list)
    if (from === to || !out[from] || to < 0 || to >= out.length) return out
    out.splice(to, 0, ...out.splice(from, 1))
    return out
  }

  // In it: at its end (or at an index), taking turns; one there already
  // keeps its place and mode.
  function add(list, key, at = -1, mode = 'turn') {
    const out = plain(list)
    if (!isKey(key) || out.some(m => m.key === key)) return out
    const m = { key, mode: mode === 'pin' && !canPin(out, key) ? 'turn' : MODES.includes(mode) ? mode : 'turn' }
    if (at >= 0 && at <= out.length) out.splice(at, 0, m)
    else out.push(m)
    return out.slice(0, MAX_MEMBERS)
  }

  // Out of it, and what to put it back with (undo): its key, mode and place.
  function remove(list, key) {
    const index = list.findIndex(m => m.key === key)
    if (index < 0) return { list: plain(list), record: null }
    return { list: plain(list.filter(m => m.key !== key)), record: { key, mode: list[index].mode, index } }
  }

  // Put back as it was (undo): where it stood, clamped to the list as it is now.
  function restore(list, record) {
    if (!record || !isKey(record.key)) return plain(list)
    const without = plain(list).filter(m => m.key !== record.key)
    return add(without, record.key, Math.min(Math.max(0, record.index), without.length), record.mode)
  }

  // The settings that say it: the capsule's own, and for the Rust side of
  // now, which keeps no capsule (phase 2), the order as widgetOrder (each
  // member's widget or plugin id) and, in the island's home, the monitor's
  // pin as monitor.pin. spin: kept, or as widgetSpin says.
  function patchFor(config = {}, list, home = 'island') {
    const members = plain(list).slice(0, MAX_MEMBERS)
    const spinWas = config.capsule?.spin
    const spin = SPINS.includes(spinWas) ? spinWas : SPINS.includes(Number(config.widgetSpin)) ? Number(config.widgetSpin) : undefined
    const patch = { capsule: { members, ...(spin !== undefined ? { spin } : {}) }, widgetOrder: members.map(m => idOfKey(m.key)) }
    const mon = members.find(m => m.key === 'widget:monitor')
    if (mon && home === 'island' && mon.mode !== 'open') {
      const pin = mon.mode === 'pin'
      if ((config.monitor?.pin !== false) !== pin) patch.monitor = { ...(config.monitor || {}), pin }
    }
    return patch
  }

  // The turns' length: widgetSpin (the Plugins page and the panel both set
  // it; the panel the capsule's spin too, for when the page has it alone).
  const spinOf = config => {
    const s = Number(config?.widgetSpin ?? config?.capsule?.spin ?? 8)
    return SPINS.includes(s) ? s : 8
  }
  function spinPatch(config = {}, sec) {
    if (!SPINS.includes(sec)) return null
    return { widgetSpin: sec, capsule: { ...(config.capsule && typeof config.capsule === 'object' && !Array.isArray(config.capsule) ? config.capsule : {}), spin: sec } }
  }

  const api = { MODES, MAX_PINS, SPINS, isKey, keyOfWidget, idOfKey, storedOf, derive, effective, canPin, setMode, move, add, remove, restore, patchFor, spinOf, spinPatch }
  if (typeof module !== 'undefined' && module.exports) module.exports = api
  else root.Capsule = api
})(this)
