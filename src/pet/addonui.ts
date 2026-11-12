// Other people's plugins on the Plugins page and in the picker (settings.ts,
// picker.ts; docs/design/specs/plugins.md §3), as plain logic for the tests:
// what a plugin's row says under its name, the picker's kind it goes under,
// what it may do in words, which cards come first in the picker (one
// installed here and never placed since: 新), the steps of installing one
// from a file and the confirm layer each shows, and a folder's line that
// did not install; the market's list, buttons, layers, words for what went
// wrong, and the widget its cards are drawn from (spec §1, §2, §5); the
// packs' rows on the Plugins page and the Look page (packs.rs; spec §1,
// §3). The words come from T(key, vars) as the page has them; the markup
// stays the page's.
// A plugin (addons.rs view), a pack (packs.rs view), a market's item, a
// .zip looked at: the Rust side's JSON, any field maybe missing, so any.
// T: her words for a key, its {names} from vars (I18n.t in the page's
// language); list: names in a row as the language says them (I18n.list).
type Words = (key: string, vars?: Record<string, unknown>) => string
type InRow = (names: string[]) => string

// The picker's kinds (picker.ts CATS) for a manifest's category.
const CATS: Record<string, string> = { system: 'system', productivity: 'work', info: 'info' }
const catOf = (a: any) => CATS[a?.category] || 'info'

// Who made it and which it is: "@lin · v1.2.0" (no author: the version).
const byline = (a: any) => [a?.author ? `@${a.author}` : '', a?.version ? `v${a.version}` : ''].filter(Boolean).join(' · ')

// A setting's label by its key (what is to be filled in first).
const labelOf = (a: any, key: string | null) => (Array.isArray(a?.settings) ? a.settings.find((f: { key: string; label?: string }) => f.key === key)?.label : '') || key || ''

// A time of day as her clock says it: 10:12.
const pad = (n: number) => String(n).padStart(2, '0')
const clockOf = (ms: number) => {
  const d = new Date(ms)
  return Number.isFinite(d.getTime()) ? `${pad(d.getHours())}:${pad(d.getMinutes())}` : ''
}

// Why the watchdog stopped it (addons.rs view: stopped { why, since,
// times }), in words: not answering, since when and how many times (spec
// §3: 无响应，已停用 · 10:12 起 3 次没回答); too busy (§8: 太忙，已停用);
// its page loading again and again. Without the watchdog's say (an older
// one): not responding, and what it said.
function stoppedWords(a: any, T: Words) {
  const s = a?.stopped
  if (!s || typeof s !== 'object') return [T('addons.stopped'), a?.said].filter(Boolean).join(' · ')
  if (s.why === 'busy') return T('addons.busy')
  if (s.why === 'crashed') return T('addons.crashed')
  const time = clockOf(s.since)
  return [T('addons.stopped'), time && s.times > 0 ? T('addons.stoppedSince', { time, n: s.times }) : ''].filter(Boolean).join(' · ')
}

// What its row says under its name (spec §3), as the built-ins' do
// (settings.ts pluginNote), with who made it and its version after: one
// that waits for a newer Wakuwaku says so; one the watchdog stopped, why
// (stoppedWords), with 重新启用; put nowhere, 没放; put somewhere:
// starting, a setting to fill in first, what went wrong while it shows
// nothing, or 运行中. placed: put somewhere; mine: its widgets up now.
// { text, bad, revive }.
interface Status {
  text: string
  bad?: boolean
  approve?: boolean
  revive?: boolean
}
function statusOf(a: any, { placed = false, mine = [], T }: { placed?: boolean; mine?: unknown[]; T: Words }): Status {
  const by = byline(a)
  const tail = (text: string) => [text, by].filter(Boolean).join(' · ')
  if (a.state === 'unsupported') return { text: tail(T('addons.newerApp')), bad: true }
  // It asks for more than was allowed (an edited folder, a changed manifest):
  // 要更多权限, with 允许… (finding 1).
  if (a.state === 'needsApproval') return { text: tail(T('addons.needsApproval')), bad: true, approve: true }
  if (a.state === 'stopped') return { text: tail(stoppedWords(a, T)), bad: true, revive: true }
  if (!placed) return { text: tail(T('pl.unplaced')) }
  if (a.state === 'off') return { text: tail(T('pl.starting')) }
  if (a.state === 'needs') return { text: T('pl.needs', { what: labelOf(a, a.missing) }), bad: true }
  if (!mine.length) return a.said ? { text: T('pl.trouble', { line: a.said }), bad: true } : { text: tail(T('pl.starting')) }
  return { text: tail(T('pl.running')) }
}

// What it may do, a line each (its detail, the install's 它会：): the
// web, by the hosts named; the agents' sessions; the hardware's readings;
// nudges. None: an empty list (the page says it shows its own alone).
// list: names in a row as the language says them (I18n.list).
// Perms: a manifest's perms, Partial where any may be missing.
interface Perms {
  net: string[]
  agents: boolean
  hardware: boolean
  notify: boolean
}
interface PermLine {
  icon: string
  text: string
}
function permLines(perms: Partial<Perms> | null | undefined, T: Words, list: InRow = names => names.join(', ')) {
  const p: Partial<Perms> = perms && typeof perms === 'object' ? perms : {}
  const out: PermLine[] = []
  if (Array.isArray(p.net) && p.net.length) out.push({ icon: 'net', text: T('addons.perm.net', { hosts: list(p.net) }) })
  if (p.agents) out.push({ icon: 'agents', text: T('addons.perm.agents') })
  if (p.hardware) out.push({ icon: 'hardware', text: T('addons.perm.hardware') })
  if (p.notify) out.push({ icon: 'notify', text: T('addons.perm.notify') })
  return out
}
// Those asked about before installing (spec §2): all but the web.
const isSensitive = (perms: Partial<Perms> | null | undefined) => !!(perms && (perms.agents || perms.hardware || perms.notify))

// --- The picker ------------------------------------------------------------------------
// The cards in their order, one installed here and not placed since
// first (fresh: its key, `plugin:<id>`), the rest as they were.
const freshFirst = <C extends { key: string }>(cards: C[], fresh: (key: string) => boolean) => [...cards.filter(c => fresh(c.key)), ...cards.filter(c => !fresh(c.key))]
// Its badge where 已放 N goes: that, or 新 for a fresh one put nowhere yet.
const badgeOf = (n: number, fresh: boolean, T: Words) => (n ? { text: T('picker.placed', { n }), fresh: false } : fresh ? { text: T('addons.new'), fresh: true } : { text: '', fresh: false })

// Those installed while the page has been up and not placed since, by id:
// 新 in the picker until put somewhere once (spec §2). The page's alone.
// The page's own: those installed since it came up (settings-mail.ts marks
// them, picker.ts shows 新 until placed).
const news = makeNews()
function makeNews() {
  const ids = new Set<string>()
  return {
    add: (id: string) => ids.add(String(id)),
    has: (id: string) => ids.has(String(id)),
    // Put somewhere: new no more.
    placed: (id: string) => ids.delete(String(id)),
    ids: () => [...ids],
  }
}

// --- Installing from a file (spec §3) --------------------------------------------------
// Step by step, each a plain object; next(state, event) the one after it:
//   idle        nothing under way; done: { id, name } once one went in
//   picking     Windows' dialog open (addon_manage pick)
//   looking     the .zip looked at (inspect), path
//   ask         安装「X」？ path, summary
//   installing  path, summary (the ask, its buttons off)
//   refused     装不了 X.zip: file, refused { reason, file, detail }
// Events: pick; picked { path } (null: closed); looked { path, summary };
// go; cancel; installed { result }.
interface FileStep {
  step: string
  path?: string
  summary?: any
  file?: string
  refused?: Refused
  done?: Done
}
interface Done {
  id: string
  name: string
  kind: string
}
type FileEvent =
  | { type: 'pick' | 'go' | 'cancel' }
  | { type: 'picked'; path: string | null }
  | { type: 'looked'; path: string; summary: any }
  | { type: 'installed'; result: any }
// Why a package will not go in (addons.rs Refused): the file at fault
// (null: none), the Rust side's words.
interface Refused {
  reason: string
  file?: string | null
  detail?: string
}
const IDLE: FileStep = { step: 'idle' }
const fileOf = (path: string | null | undefined) => String(path || '').split(/[\\/]/).pop()

// What inspect said, as a step: one to ask about (a plugin, a pack), or
// why not (a kind she does not know; no answer at all: the file not read).
const KINDS = ['plugin', 'pet', 'wallpaper']
const isPack = (x: any) => x?.kind === 'pet' || x?.kind === 'wallpaper'
function verdict(path: string | undefined, summary: any): FileStep {
  const file = summary?.file || fileOf(path)
  if (!summary || typeof summary !== 'object') return { step: 'refused', file, refused: { reason: 'read', file: null, detail: '' } }
  if (!summary.ok) return { step: 'refused', file, refused: summary.refused || { reason: 'read', file: null, detail: '' } }
  if (!KINDS.includes(summary.kind)) return { step: 'refused', file, refused: { reason: 'kind', file: null, detail: '' } }
  return { step: 'ask', path, summary }
}

function next(s: FileStep, e: FileEvent): FileStep {
  const step = s?.step || 'idle'
  switch (e?.type) {
    case 'pick':
      return step === 'idle' ? { step: 'picking' } : s
    case 'picked':
      if (step !== 'picking') return s
      return e.path ? { step: 'looking', path: e.path } : IDLE
    case 'looked':
      return step === 'looking' && e.path === s.path ? verdict(s.path, e.summary) : s
    case 'go':
      return step === 'ask' ? { step: 'installing', path: s.path, summary: s.summary } : s
    case 'cancel':
      return step === 'ask' || step === 'refused' ? IDLE : s
    case 'installed': {
      if (step !== 'installing') return s
      const r = e.result
      if (r && r.ok) return { step: 'idle', done: { id: r.id, name: r.name || s.summary.name, kind: r.kind || s.summary.kind || 'plugin' } }
      return { step: 'refused', file: r?.file || fileOf(s.path), refused: r?.refused || { reason: 'write', file: null, detail: r?.error || '' } }
    }
  }
  return s
}

// Why a package will not go in (addons.rs Refused), in words: a head (in
// the amber note, stressed) and the rest; the Rust side's own words kept
// where they are a developer's (a manifest's fault), its sizes read.
interface Warn {
  head: string
  text: string
}
function refusedWords(r: Refused | undefined, T: Words): Warn {
  const file = r?.file || ''
  const detail = String(r?.detail || '')
  switch (r?.reason) {
    case 'program':
      return { head: T('addons.no.programHead'), text: T('addons.no.program', { file }) }
    case 'type':
      return { head: T('addons.no.typeHead'), text: T('addons.no.type', { file }) }
    // A pack's (packs.rs check, may_go_in): a file no pack holds; a pet
    // without its pet.json or sheet, or a pet.json that is none; a
    // wallpaper pack with no pictures at its top; a pet or folder of that
    // name there already that is the person's own.
    case 'packType':
      return { head: T('addons.no.packTypeHead'), text: T('addons.no.packType', { file }) }
    case 'pet':
      return { head: '', text: file ? T('addons.no.petJson', { detail: cut(detail, 120) }) : T('addons.no.pet') }
    case 'pictures':
      return { head: '', text: T('addons.no.pictures') }
    case 'clash':
      return { head: '', text: T(detail.startsWith('wallpaper') ? 'addons.no.clashWall' : 'addons.no.clashPet', { id: file }) }
    case 'path':
      return { head: '', text: T('addons.no.path', { file }) }
    case 'dup':
      return { head: '', text: T('addons.no.dup', { file }) }
    case 'big': {
      const m = /over (\d+) MB( unpacked)?/.exec(detail)
      return { head: '', text: m ? T(m[2] ? 'addons.no.bigUnpacked' : 'addons.no.big', { n: m[1] }) : T('addons.no.bigAny') }
    }
    case 'files': {
      const m = /^(\d+) files, (\d+) at most/.exec(detail)
      return { head: '', text: T('addons.no.files', { n: m ? m[1] : '', most: m ? m[2] : '500' }) }
    }
    case 'zip':
      return { head: '', text: T(detail === 'ZIP64' ? 'addons.no.zip64' : detail === 'encrypted' ? 'addons.no.encrypted' : 'addons.no.zip') }
    case 'manifest':
      return { head: '', text: /no manifest\.json/.test(detail) ? T('addons.no.noManifest') : T('addons.no.manifest', { detail: cut(detail, 120) }) }
    case 'kind':
      return { head: '', text: T('addons.no.kind') }
    case 'sha':
      return { head: '', text: T('addons.no.changed') }
    case 'write':
      return { head: '', text: detail ? T('addons.no.write', { detail: cut(detail, 120) }) : T('addons.no.writeAny') }
    default:
      return { head: '', text: detail ? T('addons.no.read', { detail: cut(detail, 120) }) : T('addons.no.readAny') }
  }
}

// The confirm layer a step shows (null: none), as the page draws it:
// its title, the amber note (head stressed), what it will do, a note,
// the line under it (a link to the docs: docs), its buttons ([what,
// label, kind], kind al: the accent one, no: the red one; busy: all off).
interface Layer {
  kind: 'ask' | 'refused' | 'uninstall'
  title: string
  warn: Warn | null
  willHead: string
  will: string[]
  notes: string[]
  line: { text: string; docs: string } | null
  busy: boolean
  buttons: [what: string, label: string, kind: '' | 'al' | 'no'][]
}
function layerOf(s: FileStep, T: Words, list?: InRow): Layer | null {
  if (s?.step === 'ask' || s?.step === 'installing') {
    const m = s.summary || {}
    const will = permLines(m.perms, T, list)
    // A pack: pictures only, nothing it can do; replacing one keeps it in use.
    const pack = isPack(m)
    return {
      kind: 'ask',
      title: T('addons.installAsk', { name: m.name || m.id || '' }),
      warn: { head: T('addons.unreviewedHead'), text: T(pack ? 'addons.unreviewedPackText' : 'addons.unreviewedText') },
      willHead: T('addons.will'),
      will: pack ? [T('mk.packOnly')] : will.length ? will.map(x => x.text) : [T('addons.perm.none')],
      notes: [m.replaces ? T(pack ? 'addons.replacesPack' : 'addons.replaces', { version: m.replaces }) : '', m.supported === false && !pack ? T('addons.needsNewer') : ''].filter(Boolean),
      line: null,
      busy: s.step === 'installing',
      buttons: [
        ['cancel', T('addons.cancel'), ''],
        ['go', T(s.step === 'installing' ? 'addons.installing' : 'addons.installAnyway'), 'al'],
      ],
    }
  }
  if (s?.step === 'refused') {
    const external = s.refused?.reason === 'program' || s.refused?.reason === 'type'
    return {
      kind: 'refused',
      title: T('addons.cannot', { file: s.file || '' }),
      warn: refusedWords(s.refused, T),
      willHead: '',
      will: [],
      notes: [],
      line: { text: T(external ? 'addons.refusedExternal' : 'addons.refusedDocs'), docs: T('addons.docs') },
      busy: false,
      buttons: [['cancel', T('addons.gotIt'), 'al']],
    }
  }
  return null
}

// Allowing a plugin the permissions its manifest now asks for (finding 1):
// 允许「X」要的权限？它会：…, listing only the new ones (a.newPerms, as
// market.rs words them), 取消 / 允许.
function approveLayerOf(a: any, T: Words, list: InRow | undefined, busy = false): Layer {
  const more = permsOfNew(a?.newPerms)
  return {
    kind: 'ask',
    title: T('addons.approveAsk', { name: a?.name || a?.id || '' }),
    warn: null,
    willHead: T('addons.approveWill'),
    will: permLines(more, T, list).map(x => x.text),
    notes: [],
    line: null,
    busy,
    buttons: [
      ['cancel', T('addons.cancel'), ''],
      ['approve', T('addons.allow'), 'al'],
    ],
  }
}

// Taking one away, asked first (spec §3: 卸载「X」？它存的数据也一起删掉。).
const uninstallLayerOf = (a: any, T: Words, busy = false): Layer => ({
  kind: 'uninstall',
  title: T('addons.uninstallAsk', { name: a?.name || a?.id || '' }),
  warn: null,
  willHead: '',
  will: [],
  notes: [T('addons.uninstallNote')],
  line: null,
  busy,
  buttons: [
    ['cancel', T('addons.cancel'), ''],
    ['uninstall', T('addons.uninstall'), 'no'],
  ],
})

// A pack taken away, asked first: its files go, and what was in use falls
// back (packs.rs uninstall: her default pet; the desktop's picture kept).
const packUninstallLayerOf = (p: any, T: Words, busy = false): Layer => ({
  ...uninstallLayerOf(p, T, busy),
  notes: [T(p?.kind === 'wallpaper' ? 'packs.uninstallNote.wallpaper' : 'packs.uninstallNote.pet')],
})
// A pet deleted from the Look page's list (删除: downloaded, or a pack),
// asked first in its own words; the rest as a pack's (packs.rs uninstall).
const petDeleteLayerOf = (p: any, T: Words, busy = false): Layer => ({
  ...packUninstallLayerOf({ ...p, kind: 'pet' }, T, busy),
  title: T('pets.deleteAsk', { name: p?.name || p?.id || '' }),
  buttons: [
    ['cancel', T('addons.cancel'), ''],
    ['uninstall', T('pets.delete'), 'no'],
  ],
})

// --- Packs (packs.rs view; spec §1, §3) ------------------------------------------------
// A pack by its kind and id, as the page names it (data-pack, its row's part).
const packKey = (p: any) => `${p?.kind}:${p?.id}`
// Its row's line on the Plugins page (spec §3): 「宠物 · @作者 · 大小」.
const packLine = (p: any, T: Words) => [T(`mk.kind.${p?.kind}`), p?.author ? `@${p.author}` : '', p?.size ? sizeOf(p.size, T) : ''].filter(Boolean).join(' · ')
// Its own page's, under its name: @author · v1.1.0 · 2.1 MB · 宠物.
const packByline = (p: any, T: Words) => [p?.author ? `@${p.author}` : '', p?.version ? `v${p.version}` : '', p?.size ? sizeOf(p.size, T) : '', T(`mk.kind.${p?.kind}`)].filter(Boolean).join(' · ')
// Where it is picked (its 在「外观」里选): the Look page's part for its kind.
const lookPartOf = (kind: string) => (kind === 'wallpaper' ? 'paper' : 'pets')
// A pet's line in the Look page's list (spec §1): who made it, then where
// it came from (Codex's; a pack's: 来自市场, or 未经审核 from a file), and
// an old sheet's note.
const petNote = (p: any, T: Words) => [p?.author || '', p?.codex ? T('s.fromCodex') : '', p?.pack ? T(p.pack.reviewed ? 'packs.fromMarket' : 'addons.unreviewed') : '', p?.version === 1 ? T('home.v1') : ''].filter(Boolean).join(' · ')
// A wallpaper pack's line on the Look page: 「壁纸包 · N 张」, who made it,
// 未经审核 for one from a file.
const wallLine = (p: any, T: Words) => [T(p?.pictures === 1 ? 'packs.wallLineOne' : 'packs.wallLine', { n: p?.pictures || 0 }), p?.author ? `@${p.author}` : '', p?.reviewed ? '' : T('addons.unreviewed')].filter(Boolean).join(' · ')
// The version of a market's item here (null: none): a plugin among the
// plugins, a pack among the packs of its kind.
function installedOf(item: any, { addons = [], packs = [] }: { addons?: any[] | null; packs?: any[] | null } = {}): string | null {
  if (!item) return null
  const here = isPack(item) ? (packs || []).find(p => p.kind === item.kind && p.id === item.id) : (addons || []).find(a => a.id === item.id && a.kind !== 'pet' && a.kind !== 'wallpaper')
  return here?.version || null
}
// Installed from a file: the word at the panel's foot (spec §2, §3): a
// plugin, where to put it; a pack, the Look page.
const installedWords = (done: Done | undefined, T: Words) => T(isPack(done) ? 'mk.installedPack' : 'addons.installed', { name: done?.name || '' })

// --- What did not go in ----------------------------------------------------------------
// A folder in plugins whose manifest will not do (addons.rs broken_view),
// as one quiet line: 「plugins\foo 没装上：…」, the reason in words where
// she knows it, else as the Rust side says it (for the one who wrote it),
// short.
function brokenLine(b: any, T: Words) {
  const folder = `plugins\\${b?.folder || ''}`
  const reason = String(b?.reason || '')
  let why = cut(reason, 80)
  let m
  if (reason === 'no manifest.json') why = T('addons.why.noManifest')
  else if ((m = /^its folder is (\S+), its id (\S+)$/.exec(reason))) why = T('addons.why.folder', { folder: m[1], id: m[2] })
  else if ((m = /^(.+): not in its folder$/.exec(reason))) why = T('addons.why.missing', { file: m[1] })
  return T('addons.broken', { folder, why })
}

// Words cut to n characters, with … where they were longer.
function cut(text: string | null | undefined, n: number) {
  const chars = [...String(text ?? '')]
  return chars.length > n ? `${chars.slice(0, n - 1).join('')}…` : chars.join('')
}

// What its page says of it after who made it (spec §2's 自绘界面 too):
// its version and author, its kind in the library, and that it draws a
// view of its own.
const aboutOf = (a: any, T: Words) => [byline(a), T(`picker.cat.${catOf(a)}`), a?.view ? T('addons.ownView') : ''].filter(Boolean).join(' · ')

// --- The market (market.rs; spec §1, §2, §3's banner, §5) ------------------------------
// Its categories, the chips over its list: all, widgets (plugins), pets, wallpapers.
const MARKET_CATS = ['all', 'widget', 'pet', 'wallpaper']
const marketCat = (item: any) => (item?.kind === 'pet' ? 'pet' : item?.kind === 'wallpaper' ? 'wallpaper' : 'widget')
// Words the index gives as { en, zh } (or a string): hers, else English.
const wordsIn = (v: string | Record<string, string> | null | undefined, lang: string) => (v && typeof v === 'object' ? v[lang] || v.en || '' : String(v ?? ''))

// A link that opens the settings on the market, by category (spec §1):
// widgets:market (all), widgets:market:widget (the picker's 获取更多…),
// :pet and :wallpaper (the Look page's, phase 4). null for any other.
function marketLink(next: string | null | undefined) {
  const m = /^widgets:market(?::(widget|pet|wallpaper))?$/.exec(String(next ?? ''))
  return m ? { cat: m[1] || 'all' } : null
}

// Those its list shows: of the category, with the words looked for in
// either language's name or description, its author or its id, any case.
function marketList(items: any[] | null | undefined, { cat = 'all', query = '' }: { cat?: string; query?: string } = {}) {
  const q = String(query).trim().toLowerCase()
  const said = (i: any) => [i.name?.en, i.name?.zh, i.name, i.description?.en, i.description?.zh, i.description, i.author, i.id].filter(w => typeof w === 'string')
  return (Array.isArray(items) ? items : []).filter(i => (cat === 'all' || marketCat(i) === cat) && (!q || said(i).some(w => w.toLowerCase().includes(q))))
}

// Whether one version is newer than another, part by part (1.10.0 after 1.9.0).
function isNewer(a: string | null | undefined, b: string | null | undefined) {
  const x = String(a ?? '').split('.').map(Number)
  const y = String(b ?? '').split('.').map(Number)
  for (let i = 0; i < Math.max(x.length, y.length); i++) if ((x[i] || 0) !== (y[i] || 0)) return (x[i] || 0) > (y[i] || 0)
  return false
}

// The button at its row's end, and on its page (spec §2): a download
// under way, how far (busy: its share, then 校验中… and 安装中…); here and
// older, 更新; here, 已安装 (on its page: 添加小工具… for a plugin, 去
// 「外观」里选 for a pack); else 安装. installed: the version here (null:
// none); job: its download's last word (market:progress).
interface Job {
  stage: string
  got?: number
  total?: number
}
interface MarketButton {
  what: string
  label: string
  pct: number
  primary: boolean
  disabled: boolean
}
function marketButton(item: any, { installed = null, job = null, detail = false, T }: { installed?: string | null; job?: Job | null; detail?: boolean; T: Words }): MarketButton {
  if (job && !['done', 'failed'].includes(job.stage)) {
    const pct = job.total ? Math.min(100, Math.floor((100 * (job.got || 0)) / job.total)) : 0
    const fetching = job.stage === 'download'
    return { what: 'busy', label: fetching ? `${pct}%` : T(job.stage === 'verify' ? 'mk.verify' : 'mk.installing'), pct: fetching ? pct : 100, primary: false, disabled: true }
  }
  if (installed && isNewer(item?.version, installed)) return { what: 'update', label: T('mk.update'), pct: 0, primary: true, disabled: false }
  if (installed && !detail) return { what: 'installed', label: T('mk.installed'), pct: 0, primary: false, disabled: true }
  if (installed) return item?.kind === 'plugin' ? { what: 'add', label: T('pl.add'), pct: 0, primary: false, disabled: false } : { what: 'look', label: T('mk.toLook'), pct: 0, primary: false, disabled: false }
  return { what: 'install', label: T('mk.install'), pct: 0, primary: true, disabled: false }
}

// A size as the market says it: 48 KB, 3.2 MB.
const sizeOf = (n: number, T: Words) => (n >= 1048576 ? T('mk.mb', { n: (n / 1048576).toFixed(1) }) : T('mk.kb', { n: Math.max(1, Math.round((n || 0) / 1024)) }))
// Its line in the list, under its name: its kind, 自绘界面, what it is.
const marketAbout = (item: any, T: Words, lang: string) => [T(`mk.kind.${item?.kind}`), item?.view ? T('addons.ownView') : '', wordsIn(item?.description, lang)].filter(Boolean).join(' · ')
// Its page's, under its name: @author · v1.2.0 · 48 KB · 小工具 (· 自绘界面).
const marketByline = (item: any, T: Words) => [item?.author ? `@${item.author}` : '', item?.version ? `v${item.version}` : '', item?.size ? sizeOf(item.size, T) : '', T(`mk.kind.${item?.kind}`), item?.view ? T('addons.ownView') : ''].filter(Boolean).join(' · ')

// What `wanted` asks for beyond what `had` has (a newer version's new
// permissions), as permissions: a host none of those had covers (the
// same name, or under a *. one, as addons.rs host_allowed), each flag
// not had.
function permsBeyond(had: Partial<Perms> | null | undefined, wanted: Partial<Perms> | null | undefined): Perms {
  const h: Partial<Perms> = had && typeof had === 'object' ? had : {}
  const w: Partial<Perms> = wanted && typeof wanted === 'object' ? wanted : {}
  const rules: string[] = Array.isArray(h.net) ? h.net : []
  const covered = (host: string) => rules.some(r => r === host || (r.startsWith('*.') && host.endsWith(r.slice(1)) && host.length > r.length - 1))
  return { net: (Array.isArray(w.net) ? w.net : []).filter((x: string) => !covered(x)), agents: !!w.agents && !h.agents, hardware: !!w.hardware && !h.hardware, notify: !!w.notify && !h.notify }
}
const asksAny = (p: Perms) => !!(p.net.length || p.agents || p.hardware || p.notify)

// Installing one that asks for more than the web (spec §2): 安装「X」？
// 它会：…, 取消 / 安装, over the panel. null for one asking for none of it:
// it goes straight in (the hosts it reaches are on its page). Updating
// one here (had: what the one here may do): asked only when the new one
// asks for anything more, that alone listed (spec §5).
function marketAskOf(item: any, T: Words, list: InRow | undefined, lang: string, had: Partial<Perms> | null = null): Layer | null {
  if (!item) return null
  const name = wordsIn(item.name, lang)
  const more = had ? permsBeyond(had, item.perms) : item.perms
  if (had ? !asksAny(more) : !isSensitive(more)) return null
  return {
    kind: 'ask',
    title: T(had ? 'mk.updateAsk' : 'addons.installAsk', { name }),
    warn: null,
    willHead: T(had ? 'mk.updateWill' : 'addons.will'),
    will: permLines(more, T, list).map(x => x.text),
    notes: [],
    line: null,
    busy: false,
    buttons: [
      ['cancel', T('addons.cancel'), ''],
      ['mkgo', T(had ? 'mk.update' : 'mk.install'), 'al'],
    ],
  }
}

// What an update newly asks for (market.rs: "net:<host>", "agents",
// "hardware", "notify") as permissions, for permLines.
const permsOfNew = (list: string[] | null | undefined): Perms => {
  const l: string[] = Array.isArray(list) ? list : []
  return { net: l.filter(p => String(p).startsWith('net:')).map(p => String(p).slice(4)), agents: l.includes('agents'), hardware: l.includes('hardware'), notify: l.includes('notify') }
}
// The line under 获取更多 when updates wait for the person (spec §3):
// 「N 个插件有更新」 and 全部更新; null with none.
interface Update {
  id: string
  name: string
  from: string
  to: string
  newPerms: string[]
}
const updatesBanner = (updates: Update[] | null | undefined, T: Words) => (Array.isArray(updates) && updates.length ? { text: T('mk.updates', { n: updates.length }), button: T('mk.updateAll') } : null)
// 全部更新, asked first when any of them asks for more (spec §5): each of
// those, its versions and what it newly asks for; how many others there
// are. null: none asks for more (they go at once).
function updatesAskOf(updates: Update[] | null | undefined, T: Words, list?: InRow): Layer | null {
  const all: Update[] = Array.isArray(updates) ? updates : []
  const asking = all.filter(u => Array.isArray(u.newPerms) && u.newPerms.length)
  if (!asking.length) return null
  const rest = all.length - asking.length
  return {
    kind: 'ask',
    title: T('mk.updatesAsk', { n: all.length }),
    warn: null,
    willHead: T('mk.updatesHead'),
    will: asking.map(u => T('mk.updateLine', { name: u.name, from: u.from, to: u.to, perms: permLines(permsOfNew(u.newPerms), T, list).map(p => p.text).join(T('mk.permSep')) })),
    notes: rest > 0 ? [T('mk.updatesRest', { n: rest })] : [],
    line: null,
    busy: false,
    buttons: [
      ['cancel', T('addons.cancel'), ''],
      ['mkall', T('mk.updateAll'), 'al'],
    ],
  }
}

// Why the market will not show (its index's error, market.rs), in words:
// not open yet (404: closed, no error at all); not reached, as her
// updates say it, with the Network proxy to look at (proxy); its index
// unreadable. null: no error.
interface MarketError {
  code?: string
  detail?: string
  refused?: Refused
}
function marketTroubleOf(err: MarketError | null | undefined, T: Words) {
  if (!err || typeof err !== 'object') return null
  if (err.code === 'closed') return { closed: true, text: T('mk.closed'), note: T('mk.closedNote'), proxy: false }
  if (err.code === 'bad') return { closed: false, text: T('mk.errorBad', { detail: cut(err.detail || '', 160) }), note: '', proxy: false }
  return { closed: false, text: T('mk.error', { detail: cut(err.detail || '', 240) }), note: T('mk.proxyHint'), proxy: true }
}
// Why one did not go in (market.rs install's error), in words; '' for one called off.
function marketFailOf(err: MarketError | null | undefined, T: Words) {
  const detail = cut(err?.detail || '', 160)
  switch (err?.code) {
    case 'download':
      return T('mk.no.download', { detail })
    case 'size':
      return T('mk.no.size')
    case 'digest':
      return T('mk.no.digest')
    case 'mismatch':
      return T('mk.no.mismatch', { detail })
    case 'refused': {
      const w = refusedWords(err.refused || { reason: 'read', detail: err.detail }, T)
      return T('mk.no.refused', { detail: `${w.head}${w.text}` })
    }
    case 'notFound':
      return T('mk.no.notFound')
    case 'busy':
      return T('mk.no.busy')
    case 'cancelled':
      return ''
    default:
      return T('mk.no.other', { detail })
  }
}
// Installed: the word at the panel's foot (spec §2): a plugin, where to
// put it; a pack, the Look page.
const marketDoneOf = (item: any, T: Words, lang: string) => T(item?.kind === 'plugin' ? 'addons.installed' : 'mk.installedPack', { name: wordsIn(item?.name, lang) })

// A plugin's sample widget as sampleWidget makes it, its own .svg as its
// picture (iconUrl) where its icon is the dot.
interface SampleWidget {
  id: any
  plugin: any
  from: string
  on: boolean
  label: any
  value: any
  icon: any
  color: any
  private: boolean
  detail: any
  iconUrl?: any
}

// What its widget would say (the index's sample), as her pages take a
// widget of a plugin's: its name for a label where it gives none, its
// sizes the manifest's, its colour and icon there unless it gives its
// own, its own .svg as its picture; for her card templates (wdesk.ts) to
// draw before it is installed. null: no sample.
function sampleWidget(item: any, lang: string): SampleWidget | null {
  const s = item?.sample
  if (!s || typeof s !== 'object') return null
  const icon = s.icon || item.glyph || 'dot'
  const w: SampleWidget = {
    id: item.id,
    plugin: item.id,
    from: 'addon',
    on: true,
    label: s.label || wordsIn(item.name, lang),
    value: s.value || '',
    icon,
    color: s.color || item.color || '',
    private: false,
    detail: { ...(s.detail && typeof s.detail === 'object' ? s.detail : {}), sizes: sampleSizes(item) },
  }
  if (icon === 'dot' && item.icon) w.iconUrl = item.icon
  return w
}
// The sizes its cards are drawn at: those it has of s, m, l.
const sampleSizes = (item: any) => ['s', 'm', 'l'].filter(z => (Array.isArray(item?.sizes) && item.sizes.length ? item.sizes : ['s', 'm']).includes(z))

export {
  news, CATS, catOf, byline, labelOf, clockOf, stoppedWords, statusOf, aboutOf, permLines, isSensitive, freshFirst,
  badgeOf, makeNews, IDLE, verdict, next, refusedWords, layerOf, uninstallLayerOf, brokenLine, cut, fileOf,
  MARKET_CATS, marketCat, wordsIn, marketLink, marketList, isNewer, marketButton, sizeOf, marketAbout,
  marketByline, permsBeyond, marketAskOf, permsOfNew, updatesBanner, updatesAskOf, marketTroubleOf, marketFailOf,
  marketDoneOf, sampleWidget, sampleSizes, isPack, packKey, packLine, packByline, lookPartOf, petNote, wallLine,
  installedOf, installedWords, packUninstallLayerOf, petDeleteLayerOf, approveLayerOf,
}
