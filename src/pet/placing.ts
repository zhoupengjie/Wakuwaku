// Putting widgets in places from the picker (picker.ts), as plain data for
// the tests: where on the taskbar's strip one goes, whether the strip has
// room for it, how one taken off is put back (undo), and which widgets and
// plugins are on (placed is on: put somewhere, it runs; taken from its last
// place, it stops; no switch of its own, the Plugins page has none since
// 2026-10-17), and where one is for that page.
import type { Config } from '../shared/config.ts'

// The strip's runs, and its layout (taskbarLayout, widgets.ts layoutOf):
// in each run its parts by name ('tray', …) and the entries put there
// ({ id, type, of, size } or a group's { id, type, items }), as JSON gives them.
export type Run = 'left' | 'center' | 'right'
export interface StripLayout {
  left?: any[]
  center?: any[]
  right?: any[]
}
// A run on the screen (px along the strip): its ends and its parts'.
export interface RunBox {
  run: string
  lo: number
  hi: number
  items: { lo: number; hi: number }[]
}
// An entry or a part taken off the strip, and where it stood (undo).
export interface StripRemoval {
  entry: any
  run: string
  index: number
}
// Where widgets are put: her taskbar's strip, the desktop's layout
// (deskLayout), and her home (the strip counts while it is the taskbar).
export interface Places {
  taskbar?: StripLayout | null
  desk?: any[]
  home?: string
}
// A plugin as the settings' snapshot has it (plugins.rs): what it misses,
// or its parameters.
export interface PluginParams {
  id?: string
  missing?: string | null
  params?: { name: string; required?: boolean; value?: unknown }[]
}

const RUNS: Run[] = ['left', 'center', 'right']
const keyOf = (item: { id?: string; plugin?: string }) => (item.plugin ? `plugin:${item.plugin}` : `widget:${item.id}`)
const itemOf = (key: string) => (key.startsWith('plugin:') ? { plugin: key.slice(7) } : { id: key.slice(key.indexOf(':') + 1) })
const clone = (layout: unknown) => JSON.parse(JSON.stringify(layout))
const isWidgetEntry = (p: any) => !!p && typeof p === 'object' && (p.type === 'widget' || p.type === 'plugin')
const isGroupEntry = (p: any) => !!p && typeof p === 'object' && (p.type === 'carousel' || p.type === 'stack')
const membersOf = (p: any): { type: string; of: string }[] => (isWidgetEntry(p) ? [p] : isGroupEntry(p) ? p.items : [])

// --- On the strip --------------------------------------------------------------------
// Where one let go of at x (along the strip) goes: the run under it (or
// the nearest), before the first of its parts whose middle is past x. Its
// front half: before it; its back half: after it; between two: after the
// one before. runs: [{ run, lo, hi, items: [{ lo, hi }] }], what is there
// but the one dragged, in order.
function slotAt(runs: RunBox[], x: number) {
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
function beforeTray(layout: StripLayout) {
  for (const run of RUNS) {
    const at = (layout[run] || []).indexOf('tray')
    if (at >= 0) return { run, index: at }
  }
  return { run: 'right', index: (layout.right || []).length }
}

// The layout with an entry (or a part) put in a run at an index (clamped).
function insertAt<L extends StripLayout>(layout: L, run: string, index: number, entry: any): L {
  const out = clone(layout)
  const list = out[run] || (out[run] = [])
  list.splice(Math.max(0, Math.min(index, list.length)), 0, entry)
  return out
}

// Whether the strip has room for one more this wide (used: what its runs
// take now, room: its length): as they are, or with medium ones made
// small (the most room they give first: shrinkable [{ id, width }], each
// giving width - small), or not at all. { ok, shrink: [ids] }.
function fitStrip({ room, used, adding, shrinkable = [], small = 64 }: { room: number; used: number; adding: number; shrinkable?: { id: string; width: number }[]; small?: number }) {
  let over = used + adding - room
  if (over <= 0) return { ok: true, shrink: [] }
  const shrink: string[] = []
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
function withSizes<L extends StripLayout>(layout: L, ids: string[], size: string): L {
  const out = clone(layout)
  for (const run of RUNS) for (const p of out[run] || []) if (isWidgetEntry(p) && ids.includes(p.id)) p.size = size
  return out
}

// --- Taken off, put back (undo) ------------------------------------------------------
// Where an entry (by its id) or a part (by its name) is: what to put back
// and where; null when it is not there.
function removalOf(layout: StripLayout, key: string): StripRemoval | null {
  for (const run of RUNS) {
    const index = (layout[run] || []).findIndex(p => (typeof p === 'string' ? p === key : p.id === key))
    if (index >= 0) return { entry: clone(layout[run]![index]), run, index }
  }
  return null
}
// The layout without it.
function without<L extends StripLayout>(layout: L, key: string): L {
  const out = clone(layout)
  for (const run of RUNS) if (out[run]) out[run] = out[run].filter((p: any) => (typeof p === 'string' ? p !== key : p.id !== key))
  return out
}
// Put back where it was (its run, its index, or the run's end now); the
// layout unchanged when it is there again already (by its id, or any of
// its widgets put back meanwhile: a widget in one place).
function restoreEntry<L extends StripLayout>(layout: L, record: StripRemoval | null | undefined): L {
  if (!record?.entry) return layout
  const e = record.entry
  const there = new Set(RUNS.flatMap(run => (layout[run] || []).flatMap(p => (typeof p === 'string' ? [p] : [p.id, ...membersOf(p).map(m => `${m.type}:${m.of}`)]))))
  if (typeof e === 'string' ? there.has(e) : there.has(e.id) || membersOf(e).some(m => there.has(`${m.type}:${m.of}`))) return layout
  return insertAt(layout, RUNS.includes(record.run as Run) ? record.run : 'right', record.index, e)
}

// --- Placed is on --------------------------------------------------------------------
// The widgets and plugins put on the strip (its widgets and groups, the
// monitor's part: her home the taskbar) and on the desktop, by key.
function placedKeys({ taskbar = null, desk = [], home = 'taskbar' }: Places = {}) {
  const keys = new Set<string>()
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
// a widget by widgetsOff (shared.rs is_widget_on). null: already so.
function switchPatch(config: Config = {}, key: string, on: boolean) {
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
function switchesPatch(config: Config = {}, keys: string[], on: boolean) {
  let c = { ...config }
  const patch: Config = {}
  for (const key of keys) {
    const p = switchPatch(c, key, on)
    if (!p) continue
    Object.assign(patch, p)
    c = { ...c, ...p }
  }
  return Object.keys(patch).length ? patch : null
}

// Those put on the strip or the desktop and off, turned on (a switch of
// before turned one off where it stood: its desktop copy said nothing
// since). null: none.
const placedOnPatch = (config: Config = {}, where: Places = {}) => switchesPatch(config, [...placedKeys(where)], true)

// --- Where one is (the Plugins page) -------------------------------------------------
// Where a widget or plugin is put, by its keys (its own, and a plugin's
// widgets', which may stand on the desktop by themselves): on the desktop
// ('desk', alone or stacked; 'board', in that board), on the strip ('bar',
// her home the taskbar), in her capsule ('isle': on and nowhere else, as
// her capsule's rule has it), or nowhere (null: off). { at, board? }.
function whereIs({ keys = [], on = false, taskbar = null, desk = [], home = 'taskbar' }: Places & { keys?: Iterable<string>; on?: boolean } = {}) {
  const want = new Set(keys)
  const has = (m: any) => !!m && want.has(`${m.type}:${m.of}`)
  for (const e of Array.isArray(desk) ? desk : []) {
    const ms = e?.type === 'stack' || e?.type === 'board' ? e.items || [] : [e]
    if (ms.some(has)) return e.type === 'board' ? { at: 'board', board: e } : { at: 'desk' }
  }
  if (home === 'taskbar' && taskbar) {
    for (const run of RUNS) for (const p of taskbar[run] || []) if ((p === 'monitor' && want.has('widget:monitor')) || membersOf(p).some(has)) return { at: 'bar' }
  }
  return { at: on ? 'isle' : null }
}

// A plugin's first parameter it needs and has not been given (plugins.rs
// missing, as the settings' snapshot says it; else from its parameters),
// or null: put somewhere, it shows nothing until that is filled in.
function missingParam(plugin: PluginParams | null | undefined) {
  if (!plugin || typeof plugin !== 'object') return null
  if ('missing' in plugin) return plugin.missing || null
  const f = (plugin.params || []).find(x => x.required && !String(x.value ?? '').trim())
  return f ? f.name : null
}

export {
  RUNS, keyOf, itemOf, slotAt, beforeTray, insertAt, fitStrip, withSizes, removalOf, without, restoreEntry,
  placedKeys, switchPatch, switchesPatch, placedOnPatch, whereIs, missingParam,
}
