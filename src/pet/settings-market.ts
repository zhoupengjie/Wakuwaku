// The plugin market (market.rs; addonui.js the words and steps): its list,
// an item's page, installing and updating, taken away, permissions allowed,
// installing from a file; a word at the panel's foot (toast).
// Made by settings.js in the section's place as the page starts
// (marketPart(core), imported by settings.js): what it takes of the settings' core
// comes in core, its functions and constants as they are, its lets read
// and written through core.<name> as they change; what it hands back is the
// settings' again.
import * as AddonUI from './addonui.ts'
import * as WCards from './wcards.ts'
import * as Widgets from './widgets.ts'
import { pet } from './bridge.ts'

export function marketPart(core) {
  const {
    AddonUI, COLOR, T, Widgets, addonById, andList, body, canDoHTML, crumbHTML, draw, esc, freshRows, market,
    packById, sec, seg, setHTML, sheet, showPart, toastEl,
  } = core

  // One of the index's, by its id; the version of it here (null: none).
  const marketItem = id => (market.items || []).find(i => i.id === id) || null
  // The version of it here: a plugin's, a pack's of its kind (null: none).
  const installedOf = id => AddonUI.installedOf(marketItem(id), core.snap || {})
  // Its icon in the list and on its page: its own .svg as a picture in one
  // colour (as her pages draw a plugin's: a mask, never its markup), one of
  // hers by name, else its name's first letter on its colour.
  const marketTile = item => (item.icon ? Widgets.maskHTML(item.icon) : item.glyph && Widgets.hasIcon(item.glyph) ? Widgets.icon(item.glyph) : Widgets.markHTML(AddonUI.wordsIn(item.name, core.lang), item.color))
  const marketColor = item => item.color || COLOR.idle

  // The index read (market.rs index: again when asked, or an hour old);
  // what it says kept, the page drawn again.
  async function loadMarket(force) {
    if (market.loading) return
    market.loading = true
    draw()
    const got = await pet.market.index(force).catch(e => ({ items: market.items, at: market.at, error: { code: 'failed', detail: String(e?.message || e || '') } }))
    market.loading = false
    // As main has it: the last list read (none: not open, or never read).
    market.items = Array.isArray(got?.items) ? got.items : null
    market.at = got?.at || 0
    market.error = got?.error || null
    draw()
    core.relayout()
  }

  // The market (spec §2): 插件 › 获取更多 and back; a search, the
  // categories; each one's row (its icon, its name and author, its kind,
  // 自绘界面, what it is; 安装, 已安装, 更新, or how far its download is);
  // the line at its foot. Before its index is read: opening; not read:
  // why (not open yet: no error at all; not reached: the way it went and
  // to the Network proxy, 再试一次).
  function marketPage() {
    const crumb = crumbHTML([T('s.tabWidgets'), T('mk.title')])
    const trouble = AddonUI.marketTroubleOf(market.error, T)
    if (!market.items) return `${crumb}${trouble && !market.loading ? marketTrouble(trouble) : `<div class="grp"><div class="note">${esc(T('mk.loading'))}</div></div>`}`
    const search = `<input class="in mk-q" id="mk-q" value="${esc(market.query)}" placeholder="${esc(T('mk.search'))}" spellcheck="false" autocomplete="off">`
    const cats = seg('mk-cat', AddonUI.MARKET_CATS.map(c => [c, T(`mk.cat.${c}`)]), market.cat)
    const head = `${crumb}<div class="mk-top">${search}${cats}</div>`
    const shown = AddonUI.marketList(market.items, market)
    const q = market.query.trim()
    const rows = shown.map(marketRow).join('') || `<div class="note">${esc(q ? T('mk.none', { q }) : T('mk.empty'))}</div>`
    return `${head}${trouble ? marketTrouble(trouble) : ''}<div class="grp mk-list">${rows}</div><div class="note mk-foot">${esc(T('mk.foot'))}</div>`
  }
  function marketTrouble(t) {
    if (t.closed) return `<div class="grp mk-closed"><div class="mk-closed-t">${esc(t.text)}</div><div class="d">${esc(t.note)}</div></div>`
    const way = t.proxy ? `<div class="r"><div class="grow d">${esc(T('s.net.now', { what: core.netNow() }))}</div><span class="link" data-mk-net>${esc(T('s.netProxy'))} ›</span></div>` : ''
    return `<div class="grp"><div class="r"><div class="grow"><div class="d bad mk-bad">${esc(t.text)}</div>${t.note ? `<div class="d">${esc(t.note)}</div>` : ''}</div><button class="pbtn sm" data-mk-retry${market.loading ? ' disabled' : ''}>${esc(T('mk.retry'))}</button></div>${way}</div>`
  }
  function marketRow(item) {
    const b = AddonUI.marketButton(item, { installed: installedOf(item.id), job: market.jobs[item.id], T })
    return `<div class="r jumps mk-row" data-mk-item="${esc(item.id)}" role="button" tabindex="0"><span class="wi" style="color:${esc(marketColor(item))}">${marketTile(item)}</span><div class="grow"><div class="ellip">${esc(AddonUI.wordsIn(item.name, core.lang))}${item.author ? `<span class="d"> @${esc(item.author)}</span>` : ''}</div><div class="d ellip">${esc(AddonUI.marketAbout(item, T, core.lang))}</div></div>${marketButtonHTML(item, b)}</div>`
  }
  // Its button (addonui.js marketButton); a download under way, how far, filling.
  function marketButtonHTML(item, b) {
    if (b.what === 'busy') return `<span class="mk-prog" style="--p:${b.pct}%"><span>${esc(b.label)}</span></span>`
    return `<button class="pbtn sm${b.primary ? ' al' : ''}" data-mk-do="${b.what}" data-id="${esc(item.id)}"${b.disabled ? ' disabled' : ''}>${esc(b.label)}</button>`
  }

  // One of the market's own page (spec §2): 获取更多 › 名字; its icon, name,
  // the green 已审核, who made it, its version, size and kind, its button;
  // its cards as she would draw them from what it says it shows (its
  // index's sample, her own templates: wdesk.js), a pack's picture, or
  // neither; what it is; what it can do.
  function marketItemPage(item) {
    const name = AddonUI.wordsIn(item.name, core.lang)
    const b = AddonUI.marketButton(item, { installed: installedOf(item.id), job: market.jobs[item.id], detail: true, T })
    const top = `<div class="ad-head"><span class="ad-tile" style="--c:${esc(marketColor(item))}">${marketTile(item)}</span><div class="grow"><div class="ad-name ellip">${esc(name)} <span class="chip mk-ok">${esc(T('mk.reviewed'))}</span></div><div class="d ellip">${esc(AddonUI.marketByline(item, T))}</div></div>${marketButtonHTML(item, b)}</div>`
    const about = AddonUI.wordsIn(item.description, core.lang)
    const said = about ? `${sec(T('addons.about'))}<div class="grp"><div class="ad-text">${esc(about)}</div></div>` : ''
    return `${crumbHTML([T('mk.title'), name])}${top}${marketLooks(item)}${said}${sec(T('addons.canDo'))}<div class="grp">${canDoHTML(item.perms, item.kind)}</div>`
  }
  function marketLooks(item) {
    if (item.preview) return `${sec(T('mk.picture'))}<div class="mk-preview"><img src="${esc(item.preview)}" alt=""></div>`
    if (!AddonUI.sampleWidget(item, core.lang)) return ''
    const note = item.view ? `<div class="note mk-cap">${esc(T('mk.viewNote'))}</div>` : ''
    const light = document.body.classList.contains('set-light')
    return `${sec(T('mk.looks'))}<div class="mk-cards${light ? ' lw' : ''}" data-mk-cards="${esc(item.id)}"></div>${note}`
  }

  // Its cards drawn into their place, once the page is: the desktop's own
  // (wdesk.js, loaded the first time one is shown), each in its glass at
  // its size, with the desktop's stylesheet in a shadow of their own so
  // neither page's styles reach the other's (desk.css takes its colours
  // from :host there, light as Windows is: set-light). Beside this script
  // (the preview writes the page into one of its own, so location is not
  // where these files are).
  const PAGE_BASE = import.meta.url
  let WDesk = null
  let deskCards = null
  function loadDeskCards() {
    if (WDesk) return Promise.resolve()
    deskCards ||= import('./wdesk.ts')
      .then(m => {
        WDesk = m
      })
      .catch(() => (deskCards = null))
    return deskCards
  }
  function fillCards() {
    const hosts = [...body.querySelectorAll('[data-mk-cards]')]
    if (!hosts.length) return
    if (!WDesk) return void loadDeskCards().then(() => core.isOpen && fillCards())
    const light = document.body.classList.contains('set-light')
    for (const host of hosts) {
      const item = marketItem(host.dataset.mkCards)
      const w = item && AddonUI.sampleWidget(item, core.lang)
      if (!w) continue
      const key = `${item.id}|${item.version}|${core.lang}|${light}`
      if (host.shadowRoot && host._drawn === key) continue
      const root = host.shadowRoot || host.attachShadow({ mode: 'open' })
      host._drawn = key
      const ctx = { lang: core.lang, light, detailed: true, now: Date.now(), hour: new Date().getHours(), all: [w] }
      const color = WCards.colorOf(w, ctx)
      const cards = AddonUI.sampleSizes(item).map(size => `<div class="wd ${size}" style="--c:${esc(color)}"><div class="wtint"></div><div class="wbody">${WDesk.faceHTML(w, size, ctx)}</div></div>`)
      root.innerHTML = `<link rel="stylesheet" href="${esc(new URL('desk.css', PAGE_BASE).href)}"><style>:host { color: var(--w-fg); font: 12px/1.35 var(--font-ui); user-select: none; }</style>${cards.join('')}`
    }
  }

  // What is asked before one goes in (addonui.js marketAskOf): installing,
  // what it may do beyond the web; updating one here, what more it asks for.
  const marketAsk = id => AddonUI.marketAskOf(marketItem(id), T, andList, core.lang, (core.snap?.addons || []).find(a => a.id === id)?.perms || null)

  // 安装 (spec §2): one asking for more than the web asked first over the
  // panel; else, or once asked, downloaded and put in (market.rs install,
  // its steps told meanwhile: market:progress); then the word at the foot,
  // its row on the Plugins page edged, 新 in the picker until it is put
  // somewhere; or why it did not go in.
  function marketInstall(id, asked = false) {
    const item = marketItem(id)
    if (!item || (market.jobs[id] && !['done', 'failed'].includes(market.jobs[id].stage))) return
    if (!asked && marketAsk(id)) {
      core.mkAsk = { kind: 'install', id }
      return drawSheet()
    }
    market.jobs[id] = { id, got: 0, total: item.size, stage: 'download' }
    draw()
    pet.market
      .install(id)
      .catch(e => ({ ok: false, error: { code: 'other', detail: String(e?.message || e || '') } }))
      .then(got => {
        market.jobs[id] = { ...(market.jobs[id] || {}), stage: got?.ok ? 'done' : 'failed' }
        if (got?.snapshot) core.snap = got.snapshot
        if (got?.ok) {
          // A pack's row is in 资源包, and no pack goes in the picker.
          if (AddonUI.isPack(item)) freshRows.add(AddonUI.packKey(item))
          else {
            freshRows.add(id)
            AddonUI.news.add(id)
          }
          toast(AddonUI.marketDoneOf(item, T, core.lang))
        } else {
          const why = AddonUI.marketFailOf(got?.error, T)
          if (why) toast(why)
        }
        draw()
      })
  }

  // 全部更新 (spec §3, §5): asked first when one asks for more; then each,
  // one after another (market.rs updateAll); the word at the foot.
  function updateAll(asked = false) {
    const ups = core.snap?.marketUpdates || []
    if (!ups.length || market.updating) return
    if (!asked && AddonUI.updatesAskOf(ups, T, andList)) {
      core.mkAsk = { kind: 'updates' }
      return drawSheet()
    }
    market.updating = true
    draw()
    pet.market
      .updateAll()
      .catch(e => ({ ok: false, done: [], failed: [{ name: '', error: { code: 'other', detail: String(e?.message || e || '') } }] }))
      .then(got => {
        market.updating = false
        if (got?.snapshot) core.snap = got.snapshot
        const names = (got?.done || []).map(id => ups.find(u => u.id === id)?.name || id)
        const failed = (got?.failed || []).map(f => T('mk.updateFailed', { name: f.name || f.id || '', why: AddonUI.marketFailOf(f.error, T) }))
        toast([names.length ? T('mk.updated', { names: andList(names) }) : '', ...failed].filter(Boolean).join(' · '))
        draw()
      })
  }

  // How a download goes (market.rs, market:progress): its row and its
  // page say it, whoever began it (the daily look's updates too).
  pet.onMarket(p => {
    if (!p?.id) return
    market.jobs[p.id] = p
    if (core.isOpen && core.tab === 'widgets' && core.sub) draw()
  })

  // The question over the panel (addonui.js layerOf, uninstallLayerOf): its
  // title, the amber note, what it will do, its notes, a line to the
  // developer docs, its buttons; nothing while none is asked.
  function sheetHTML() {
    const mk = core.mkAsk?.kind === 'install' ? marketAsk(core.mkAsk.id) : core.mkAsk?.kind === 'updates' ? AddonUI.updatesAskOf(core.snap?.marketUpdates, T, andList) : null
    const out =
      core.gone &&
      (core.gone.del
        ? AddonUI.petDeleteLayerOf((core.snap.pets || []).find(p => p.id === core.gone.id) || { id: core.gone.id }, T, core.gone.busy)
        : core.gone.kind
          ? AddonUI.packUninstallLayerOf(packById(core.gone.kind, core.gone.id) || { id: core.gone.id, kind: core.gone.kind }, T, core.gone.busy)
          : AddonUI.uninstallLayerOf(addonById(core.gone.id) || { id: core.gone.id }, T, core.gone.busy))
    const ap = core.approveFor ? AddonUI.approveLayerOf(addonById(core.approveFor), T, andList) : null
    const l = out || mk || ap || AddonUI.layerOf(core.flow, T, andList)
    if (!l) return ''
    const warn = l.warn ? `<div class="sh-warn">${l.warn.head ? `<b>${esc(l.warn.head)}</b>` : ''}${esc(l.warn.text)}</div>` : ''
    const will = l.will.length ? `<div class="sh-will"><div class="d">${esc(l.willHead)}</div><ul>${l.will.map(x => `<li>${esc(x)}</li>`).join('')}</ul></div>` : ''
    const notes = l.notes.map(n => `<div class="d">${esc(n)}</div>`).join('')
    const line = l.line ? `<div class="d">${esc(l.line.text)} <span class="link" data-site="pluginDocs">${esc(l.line.docs)}</span></div>` : ''
    const buttons = l.buttons.map(([what, label, kind]) => `<button class="pbtn${kind ? ` ${kind}` : ''}" data-sheet="${what}"${l.busy ? ' disabled' : ''}>${esc(label)}</button>`).join('')
    return `<div class="sh sh-${l.kind}" role="alertdialog" aria-label="${esc(l.title)}"><div class="sh-q">${esc(l.title)}</div>${warn}${will}${notes}${line}<div class="sh-btns">${buttons}</div></div>`
  }
  function drawSheet() {
    const html = core.isOpen ? sheetHTML() : ''
    setHTML(sheet, html)
    const was = sheet.hidden
    sheet.hidden = !html
    // Asked: the keyboard on 取消 (知道了 where that is its one button), so
    // Enter does nothing rash, as the desktop's board asks.
    if (was && html) requestAnimationFrame(() => sheet.querySelector('[data-sheet="cancel"]')?.focus({ preventScroll: true }))
  }

  // Installing from a file (spec §3), its steps (addonui.js next): Windows'
  // dialog for a .zip, the .zip looked at, asked about (or why it cannot
  // go in), installed; then a word, its row edged until the panel closes,
  // 新 in the picker until it is put somewhere, the page at its row.
  function flowStep(e) {
    core.flow = AddonUI.next(core.flow, e)
    drawSheet()
  }
  async function installFromFile() {
    if (core.flow.step !== 'idle' || core.gone) return
    flowStep({ type: 'pick' })
    core.pickingZip = true
    let path = null
    try {
      path = await pet.addons.pickZip()
    } finally {
      core.pickingZip = false
    }
    flowStep({ type: 'picked', path })
    if (core.flow.step !== 'looking') return
    const summary = await pet.addons.inspect(path).catch(() => null)
    flowStep({ type: 'looked', path, summary })
  }
  async function installGo() {
    if (core.flow.step !== 'ask') return
    flowStep({ type: 'go' })
    const result = await pet.addons.install(core.flow.path, core.flow.summary?.sha256).catch(e => ({ ok: false, refused: { reason: 'write', file: null, detail: String(e?.message || e || '') } }))
    flowStep({ type: 'installed', result })
    const done = core.flow.done
    if (!done) return
    core.flow = AddonUI.IDLE
    // A pack's row is in 资源包 (its key its kind's and id), and none goes in the picker.
    const key = AddonUI.isPack(done) ? AddonUI.packKey(done) : done.id
    freshRows.add(key)
    if (!AddonUI.isPack(done)) AddonUI.news.add(done.id)
    core.sub = null
    toast(AddonUI.installedWords(done, T))
    core.snap = await pet.settings.get()
    draw()
    core.relayout()
    core.partToShow = `widgets:${key}`
    showPart()
  }
  // Taken away, once asked (卸载 on its page): its page goes back to the
  // list. A pack (gone.kind): its folder, what was in use falling back
  // (packs.rs uninstall).
  async function uninstallGo() {
    if (!core.gone || core.gone.busy) return
    const { id, kind, del } = core.gone
    core.gone = { id, kind, del, busy: true }
    drawSheet()
    const asked = kind ? pet.addons.uninstallPack(kind, id) : pet.addons.uninstall(id)
    const got = await asked.catch(e => ({ ok: false, error: String(e?.message || e || '') }))
    core.gone = null
    if (got?.ok) {
      if (got.snapshot) core.snap = got.snapshot
      freshRows.delete(kind ? AddonUI.packKey({ kind, id }) : id)
      if (!kind) AddonUI.news.placed(id)
      if ((core.sub?.page === 'addon' || core.sub?.page === 'pack') && core.sub.id === id) core.sub = null
    } else if (got?.error) toast(got.error)
    draw(true)
    core.relayout()
  }

  // The new permissions allowed (允许 on its row): its grant set to its
  // manifest's, so it runs again (addon_manage approve, finding 1).
  async function approveGo() {
    const id = core.approveFor
    if (!id) return
    core.approveFor = null
    drawSheet()
    const got = await pet.addons.approve(id).catch(() => null)
    if (got && typeof got === 'object' && got.settings) core.snap = got
    draw(true)
    core.relayout()
  }

  // A word for a few seconds, at the panel's foot.
  let toastTimer
  function toast(text) {
    clearTimeout(toastTimer)
    toastEl.textContent = text
    toastEl.hidden = false
    toastTimer = setTimeout(() => (toastEl.hidden = true), 3600)
  }

  return { approveGo, drawSheet, fillCards, flowStep, installFromFile, installGo, loadMarket, marketInstall, marketItem, marketItemPage, marketPage, toast, uninstallGo, updateAll }
}
