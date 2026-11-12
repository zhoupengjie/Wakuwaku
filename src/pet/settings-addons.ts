// Other people's plugins (addons.rs view; addonui.ts) and packs (packs.rs):
// each one's row and its own page with what it may do, and the pages under
// the Plugins page (SUBS).
// Made by settings.ts in the section's place as the page starts
// (addonsPart(core), imported by settings.ts): what it takes of the settings' core
// comes in core, its functions and constants as they are, its lets read
// and written through core.<name> as they change; what it hands back is the
// settings' again.
import * as AddonUI from './addonui.ts'
import * as Widgets from './widgets.ts'

// What this part takes of the settings' core: the names it reads (and,
// of its lets, writes); settings.ts's call is checked against them.
export interface AddonsCore {
  AddonUI: any
  COLOR: any
  T: any
  Widgets: any
  drafts: any
  draw: any
  esc: any
  freshRows: any
  lang: any
  loadGallery: any
  marketItem: any
  marketItemPage: any
  marketPage: any
  nameList: any
  openPlugin: any
  part: any
  partToShow: any
  placeHTML: any
  placeOf: any
  relayout: any
  sec: any
  seg: any
  showPart: any
  snap: any
  sub: any
  sw: any
  tab: any
}

export function addonsPart(core: AddonsCore) {
  const {
    AddonUI, COLOR, T, Widgets, drafts, draw, esc, freshRows, loadGallery, nameList, part, placeHTML, placeOf,
    sec, seg, showPart, sw,
  } = core

  const addonsNow = () => (core.snap?.addons || []).filter(a => a.kind === 'plugin')
  const addonById = id => addonsNow().find(a => a.id === id) || null
  const addonField = (id, key) => (addonById(id)?.settings || []).find(f => f.key === key) || null
  // Names in a row as her language says them (「a、b 和 c」).
  const andList = names => nameList(core.lang, names, 'and')
  // Its icon: its own picture (one colour, as hers are), one of hers, or
  // the first letter of its name on its colour.
  const addonIcon = a => (a.icon?.url ? Widgets.maskHTML(a.icon.url) : a.icon?.name && Widgets.hasIcon(a.icon.name) ? Widgets.icon(a.icon.name) : Widgets.markHTML(a.name, a.color))
  const addonColor = (a, mine = []) => a.color || mine[0]?.color || (a.on ? '#5e9bff' : COLOR.idle)
  // 未经审核, for one from a file (spec §3).
  const unreviewedChip = a => (a.reviewed ? '' : ` <span class="chip warn">${esc(T('addons.unreviewed'))}</span>`)

  // Its row, among hers (spec §3): its icon, its name (a press opens its
  // page), 未经审核 for one from a file, what its widgets say now; under it
  // how it is and who made it (addonui.ts statusOf); at its end its
  // settings' fold and where it is (or 添加小工具…), as hers have; one not
  // responding, 重新启用 there instead. Installed while the panel is open:
  // an accent edge at its left until it closes.
  function addonRow(a, mine) {
    const detailed = core.snap.settings.details !== false
    const place = placeOf({ plugin: a.id }, a.on)
    const says = mine
      .map(w => Widgets.words(core.lang, w, detailed))
      .map(({ label, value }) => [label !== a.name ? label : '', value].filter(Boolean).join(' '))
      .filter(Boolean)
      .join(' · ')
    const note = AddonUI.statusOf(a, { placed: !!place.at, mine, T })
    const open = core.openPlugin === a.id
    const more = a.settings?.length ? `<button class="pbtn sm" data-pexp="${esc(a.id)}">${esc(T(open ? 'pl.fold' : 'pl.settings'))}</button>` : ''
    const end = note.revive
      ? `<button class="pbtn sm" data-revive="${esc(a.id)}">${esc(T('addons.revive'))}</button>`
      : note.approve
        ? `<button class="pbtn sm" data-approve="${esc(a.id)}">${esc(T('addons.allow'))}</button>`
        : placeHTML(place, `plugin:${a.id}`)
    const name = `<button class="nm" data-addon="${esc(a.id)}">${esc(a.name)}</button>`
    return `<div class="r${freshRows.has(a.id) ? ' fresh' : ''}" data-part="widgets:${esc(a.id)}"><span class="wi" style="color:${esc(addonColor(a, mine))}">${addonIcon(a)}</span><div class="grow"><div class="ellip">${name}${unreviewedChip(a)}${says ? `<span class="d"> ${esc(says)}</span>` : ''}</div><div class="d ellip${note.bad ? ' bad' : ''}">${esc(note.text)}</div></div>${more}${end}</div>${open ? addonFields(a) : ''}`
  }

  // Its settings, drawn from its manifest's fields (addons.rs view: text,
  // number, flag, choice with its options' own words), as hers are
  // (pluginFields): each as set, else its default; what is being typed
  // stays. A choice of more than a few: a list under a button, as the Mail
  // page has (openChoices). Kept through the same patch (setPlugin).
  function addonFields(a) {
    const fields = (a.settings || []).map(f => {
      const key = `${a.id}.${f.key}`
      const label = esc(f.label || f.key)
      const now = f.value ?? f.default
      if (f.type === 'flag') return `<div class="fr"><span class="fl wide">${label}</span>${sw('af:' + key, now === true)}</div>`
      if (f.type === 'choice') {
        const options = (f.options || []).map((o, i) => [o, f.optionLabels?.[i] || o])
        const current = options.some(([o]) => o === now) ? now : options[0]?.[0]
        if (options.length <= 4) return `<div class="fr"><span class="fl">${label}</span><span class="grow"></span>${seg('ac:' + key, options, current)}</div>`
        const shown = options.find(([o]) => o === current)?.[1] || ''
        return `<div class="fr"><span class="fl">${label}</span><span class="grow"></span><button class="pick" data-achoice="${esc(key)}">${esc(shown)} ▾</button></div>`
      }
      const value = key in drafts ? drafts[key] : now == null ? '' : String(now)
      return `<label class="fr"><span class="fl">${label}${f.required ? ' *' : ''}</span><input class="in" data-field="${esc(key)}" value="${esc(value)}" placeholder="${esc(f.hint || '')}" maxlength="300" spellcheck="false"${f.type === 'number' ? ' inputmode="decimal"' : ''}></label>`
    })
    return `<div class="pf">${fields.join('')}</div>`
  }

  // A part of the Plugins page in its place (sub): a crumb (插件 › 名字,
  // its arrow back to where it came from), then the part. Each page by its
  // name: a plugin's own, the market, one of the market's; null: it is gone
  // (a plugin taken away meanwhile, an item no longer listed), the page
  // itself then.
  const SUBS = {
    addon: s => addonById(s.id) && addonPage(addonById(s.id)),
    pack: s => packById(s.kind, s.id) && packPage(packById(s.kind, s.id)),
    market: () => core.marketPage(),
    mkItem: s => core.marketItem(s.id) && core.marketItemPage(core.marketItem(s.id)),
  }
  function crumbHTML(path) {
    const last = path.length - 1
    const parts = path.map((p, i) => (i === last ? `<b>${esc(p)}</b>` : esc(p))).join(' › ')
    return `<div class="crumb"><button class="crumb-b" data-sub-back title="${esc(T('addons.back'))}" aria-label="${esc(T('addons.back'))}">&#xE76B;</button><span class="path ellip">${parts}</span></div>`
  }
  function openSub(next) {
    core.sub = { ...next, back: core.sub }
    draw(true)
    core.relayout()
  }
  function backSub() {
    core.sub = core.sub?.back || null
    draw(true)
    core.relayout()
  }

  // A plugin's own page (spec §3; the mockup's detail): its icon, name and
  // 未经审核, who made it, its version and kind (自绘界面 for one with a view
  // of its own), and where it is (or 添加小工具…); what it says it is; what
  // it may do, a line each, then what no plugin can; 卸载 (asked first,
  // over the panel).
  const PERM_GLYPH = { net: 'E774', agents: 'E756', hardware: 'E950', notify: 'EA8F', none: 'EA18' }
  function addonPage(a) {
    const mine = (core.snap.widgets || []).filter(w => w.plugin === a.id)
    const place = placeOf({ plugin: a.id }, a.on)
    const about = AddonUI.aboutOf(a, T)
    const top = `<div class="ad-head"><span class="ad-tile" style="--c:${esc(addonColor(a, mine))}">${addonIcon(a)}</span><div class="grow"><div class="ad-name ellip">${esc(a.name)}${unreviewedChip(a)}</div><div class="d ellip">${esc(about)}</div></div>${placeHTML(place, `plugin:${a.id}`)}</div>`
    const said = a.description ? `${sec(T('addons.about'))}<div class="grp"><div class="ad-text">${esc(a.description)}</div></div>` : ''
    return `${crumbHTML([T('s.tabWidgets'), a.name])}${top}${said}${sec(T('addons.canDo'))}<div class="grp">${canDoHTML(a.perms)}</div><div class="ad-foot"><button class="pbtn no" data-uninstall="${esc(a.id)}">${esc(T('addons.uninstall'))}</button></div>`
  }
  // 它能做的 (spec §2, §3): what it may do, a line each, then what no plugin
  // can; a pack: pictures only, no code.
  function canDoHTML(perms, kind = 'plugin') {
    const line = (glyph, text, no = false) => `<div class="r ad-perm${no ? ' no' : ''}"><span class="g">&#x${glyph};</span><div class="grow">${esc(text)}</div></div>`
    if (kind !== 'plugin') return line('E72E', T('mk.packOnly'), true)
    const list = AddonUI.permLines(perms, T, andList)
    return (list.length ? list : [{ icon: 'none', text: T('addons.perm.none') }]).map(p => line(PERM_GLYPH[p.icon], p.text)).join('') + line('E72E', T('addons.cannotDo'), true)
  }

  // --- Packs (packs.rs view; addonui.ts) ------------------------------------------------------
  // The pets and wallpapers installed as packs, each by its kind and id.
  const packsNow = () => core.snap?.packs || []
  const packById = (kind, id) => packsNow().find(p => p.kind === kind && p.id === id) || null
  // Its tile: its preview, else the first letter of its name.
  const packTile = (p, big = false) => `<span class="pk-tile${big ? ' big' : ''}">${p.preview ? `<img src="${esc(p.preview)}" alt="" loading="lazy">` : Widgets.markHTML(p.name, null)}</span>`
  // Its 在「外观」里选: the Look page, at its kind's part (her pets, the wallpaper).
  const toLookButton = kind => `<button class="pbtn sm" data-pack-look="${esc(kind)}">${esc(T('packs.toLook'))}</button>`
  function toLook(kind) {
    core.tab = 'look'
    core.sub = null
    core.partToShow = AddonUI.lookPartOf(kind)
    draw(true)
    loadGallery(false)
    core.relayout()
    showPart()
  }

  // The 资源包 group (spec §3), after 插件和小工具, when there are any:
  // each its tile, its name (a press opens its page), 未经审核 for one from
  // a file, 「宠物 · @作者 · 大小」, and 在「外观」里选.
  function packsGroup() {
    const rows = packsNow().map(p => {
      const key = AddonUI.packKey(p)
      const name = `<button class="nm" data-pack="${esc(key)}">${esc(p.name)}</button>`
      return `<div class="r pk-row${freshRows.has(key) ? ' fresh' : ''}" data-part="widgets:${esc(key)}">${packTile(p)}<div class="grow"><div class="ellip">${name}${unreviewedChip(p)}</div><div class="d ellip">${esc(AddonUI.packLine(p, T))}</div></div>${toLookButton(p.kind)}</div>`
    })
    return rows.length ? `${part('widgets:packs', '')}${sec(T('packs.section'))}<div class="grp">${rows.join('')}</div>` : ''
  }

  // A pack's own page (spec §3, as a plugin's: 插件 › 名字): its picture as
  // a tile, its name and 未经审核, who made it, its version, size and kind,
  // 在「外观」里选; its picture, what it is, 它能做的 (只有图片，没有代码), 卸载
  // (asked first).
  function packPage(p) {
    const top = `<div class="ad-head">${packTile(p, true)}<div class="grow"><div class="ad-name ellip">${esc(p.name)}${unreviewedChip(p)}</div><div class="d ellip">${esc(AddonUI.packByline(p, T))}</div></div>${toLookButton(p.kind)}</div>`
    const picture = p.preview ? `${sec(T('mk.picture'))}<div class="mk-preview"><img src="${esc(p.preview)}" alt=""></div>` : ''
    const said = p.description ? `${sec(T('addons.about'))}<div class="grp"><div class="ad-text">${esc(p.description)}</div></div>` : ''
    return `${crumbHTML([T('s.tabWidgets'), p.name])}${top}${picture}${said}${sec(T('addons.canDo'))}<div class="grp">${canDoHTML(null, p.kind)}</div><div class="ad-foot"><button class="pbtn no" data-pack-uninstall="${esc(AddonUI.packKey(p))}">${esc(T('addons.uninstall'))}</button></div>`
  }

  return { SUBS, addonById, addonField, addonRow, addonsNow, andList, backSub, canDoHTML, crumbHTML, openSub, packById, packTile, packsGroup, toLook }
}
