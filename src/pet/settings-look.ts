// The Look page: her home (island, taskbar, the top bar and dock), her
// island's look by Windows' mode (presets, or its colour and parts), Windows'
// own mode and accent, and the desktop's picture.
// Made by settings.js in the section's place as the page starts
// (lookPart(core), imported by settings.js): what it takes of the settings' core
// comes in core, its functions and constants as they are, its lets read
// and written through core.<name> as they change; what it hands back is the
// settings' again.
import * as AddonUI from './addonui.ts'
import { Island } from './pages.ts'
import { pet } from './bridge.ts'

export function lookPart(core) {
  const {
    AddonUI, COLOR, SIZES, T, draw, esc, onTaskbar, pagePets, part, patch, row, sec, seg, sw, toMarketRow,
  } = core

  // By Windows' mode: a preset, or its parts one by one, over Windows' own
  // flyout grey (none set: that grey). On the taskbar only where her end is
  // a capsule (the Mica, clear or light strip); on the black one she is part
  // of it. Folded out under the taskbar's options, or the island's at the
  // top of the screen (its own look, ITOP_*). Open, the settings are the
  // island itself, so a small one here shows the look (hovered, it opens).
  const ILOOK_DEFAULT = {
    // Bare in the dark mode, a new user's since 2026-10-13 (the person's own
    // look; island-look.js drawIslandLook takes it as the default too).
    dark: { bg: '#2c2c2c', op: 100, blur: 0, edge: 'line', bare: true },
    light: { bg: '#f3f3f3', op: 100, blur: 0, edge: 'line', bare: false },
  }
  // Each preset its own look: the dark grey was the default's, the bare look
  // once that became the default, and a press on the bare one lit the grey
  // (zhoupengjie/Wakuwaku#17).
  const ILOOK_PRESETS = {
    dark: [
      ['black', { bg: '#000000', op: 100, blur: 0, edge: 'line', bare: false }],
      ['gray', { bg: '#2c2c2c', op: 100, blur: 0, edge: 'line', bare: false }],
      ['mica', { bg: '#242424', op: 62, blur: 40, edge: 'line', bare: false }],
      ['ring', { bg: '#2c2c2c', op: 100, blur: 0, edge: 'ring', bare: false }],
      ['bare', { bg: '#2c2c2c', op: 100, blur: 0, edge: 'line', bare: true }],
    ],
    light: [
      ['white', { bg: '#fbfbfb', op: 100, blur: 0, edge: 'line', bare: false }],
      ['gray', { bg: '#f3f3f3', op: 100, blur: 0, edge: 'line', bare: false }],
      ['mica', { bg: '#f6f6f6', op: 62, blur: 40, edge: 'line', bare: false }],
      ['ring', { bg: '#fbfbfb', op: 100, blur: 0, edge: 'ring', bare: false }],
      ['bare', { bg: '#f3f3f3', op: 100, blur: 0, edge: 'line', bare: true }],
      ['black', { bg: '#000000', op: 100, blur: 0, edge: 'none', bare: false }],
    ],
  }
  // At the top of the screen its own look, kept apart (islandTopLook,
  // zhoupengjie/Wakuwaku#27): black by default in either mode, as it always
  // was (the iPhone's, its faint line, white words), so nobody's changes;
  // no bare capsule (no strip to be part of up there), and no 「云母毛玻璃」
  // where Windows' acrylic cannot go under it (nothing else would blur).
  const ITOP_DEFAULT = {
    dark: { bg: '#000000', op: 100, blur: 0, edge: 'line', bare: false },
    light: { bg: '#000000', op: 100, blur: 0, edge: 'line', bare: false },
  }
  const ITOP_PRESETS = {
    dark: [
      ['black', { bg: '#000000', op: 100, blur: 0, edge: 'line', bare: false }],
      ['gray', { bg: '#2c2c2c', op: 100, blur: 0, edge: 'line', bare: false }],
      ['mica', { bg: '#242424', op: 62, blur: 40, edge: 'line', bare: false }],
      ['ring', { bg: '#2c2c2c', op: 100, blur: 0, edge: 'ring', bare: false }],
    ],
    light: [
      ['black', { bg: '#000000', op: 100, blur: 0, edge: 'line', bare: false }],
      ['gray', { bg: '#f3f3f3', op: 100, blur: 0, edge: 'line', bare: false }],
      ['white', { bg: '#fbfbfb', op: 100, blur: 0, edge: 'line', bare: false }],
      ['mica', { bg: '#f6f6f6', op: 62, blur: 40, edge: 'line', bare: false }],
      ['ring', { bg: '#fbfbfb', op: 100, blur: 0, edge: 'ring', bare: false }],
    ],
  }
  // The moods' colours on a light ground, as island.js has them (COLOR_LIGHT).
  const COLOR_DEEP = { idle: '#6e6e73', working: '#0067c0', waiting: '#b25e00', done: '#0f7b3f', review: '#7346c9', error: '#c42b3c' }
  // Grounds to pick for each mode: as dark (or light) as its words need.
  const ILOOK_SWATCHES = {
    dark: ['#000000', '#1c1c1c', '#202020', '#2c2c2c', '#3a3a3a', '#1f2733', '#2a2433', '#1f2d27', '#33271f'],
    light: ['#ffffff', '#fbfbfb', '#f3f3f3', '#e8e8e8', '#dddddd', '#eef3fb', '#f5effa', '#eef7f1', '#fbf3ec'],
  }
  const winMode = () => (core.snap.winLook?.light ? 'light' : 'dark')
  const ilookFor = () => core.ilookMode || winMode()
  // Her home's: the taskbar's, or the top of the screen's.
  const isTop = () => !onTaskbar(core.snap.settings)
  const ilookKey = () => (isTop() ? 'islandTopLook' : 'islandLook')
  const ilookDefault = mode => (isTop() ? ITOP_DEFAULT : ILOOK_DEFAULT)[mode]
  const ilookPresets = mode => (isTop() ? ITOP_PRESETS[mode].filter(([id]) => id !== 'mica' || core.snap.plate === true) : ILOOK_PRESETS[mode])
  const ilookOf = mode => ({ ...ilookDefault(mode), ...(core.snap.settings[ilookKey()]?.[mode] || {}) })
  const sameLook = (a, b) => a.bg.toLowerCase() === b.bg.toLowerCase() && a.op === b.op && a.blur === b.blur && a.edge === b.edge && !!a.bare === !!b.bare
  const ilookPreset = mode => (ilookPresets(mode).find(([, p]) => sameLook(p, ilookOf(mode))) || ['custom'])[0]
  const ilookName = (mode, id) => T(id === 'custom' ? 's.ilookCustom' : isTop() && id === 'black' ? 's.ilookP.top.black' : `s.ilookP.${mode}.${id}`)

  // A mode's look kept (its default kept as none); while a slider moves,
  // only shown here, kept once it is let go.
  function ilookSave(mode, look, keep = true) {
    const key = ilookKey()
    const all = { ...(core.snap.settings[key] || {}) }
    all[mode] = sameLook(look, ilookDefault(mode)) ? null : look
    if (keep) return patch({ [key]: all })
    core.snap.settings[key] = all
    draw()
  }

  // A colour's relative luminance (WCAG), as island.js judges a ground:
  // above 0.35, the words on it go dark.
  function luminance(hex) {
    const [r, g, b] = [1, 3, 5].map(i => {
      const v = parseInt(hex.slice(i, i + 2), 16) / 255
      return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4
    })
    return 0.2126 * r + 0.7152 * g + 0.0722 * b
  }
  const inkDarkOn = hex => luminance(hex) > 0.35

  function toHsl(hex) {
    const [r, g, b] = [1, 3, 5].map(i => parseInt(hex.slice(i, i + 2), 16) / 255)
    const max = Math.max(r, g, b)
    const min = Math.min(r, g, b)
    const l = (max + min) / 2
    const d = max - min
    const s = d === 0 ? 0 : d / (1 - Math.abs(2 * l - 1))
    const h = d === 0 ? 0 : max === r ? ((g - b) / d + 6) % 6 : max === g ? (b - r) / d + 2 : (r - g) / d + 4
    return [h, s, l]
  }

  function fromHsl(h, s, l) {
    const c = (1 - Math.abs(2 * l - 1)) * s
    const x = c * (1 - Math.abs((h % 2) - 1))
    const [r, g, b] = [[c, x, 0], [x, c, 0], [0, c, x], [0, x, c], [x, 0, c], [c, 0, x]][Math.floor(h) % 6]
    const m = l - c / 2
    return `#${[r, g, b].map(v => Math.round(Math.min(1, Math.max(0, v + m)) * 255).toString(16).padStart(2, '0')).join('')}`
  }

  // A ground picked, any colour: the words on it go light or dark as it is
  // (island.js), so only a mid one reads poorly either way (between 0.18
  // and 0.36, under 4.5 to 1 with white words, and dark ones not yet):
  // moved out to the nearer side, its hue kept.
  function ilookSteer(hex) {
    hex = hex.toLowerCase()
    const y = luminance(hex)
    if (y <= 0.18 || y >= 0.36) return hex
    const [h, s, l] = toHsl(hex)
    const darker = y - 0.18 < 0.36 - y
    let [lo, hi] = darker ? [0, l] : [l, 1]
    // The lightness where it crosses out of the middle, a little past it.
    for (let i = 0; i < 24; i++) {
      const mid = (lo + hi) / 2
      const y2 = luminance(fromHsl(h, s, mid))
      if (darker) {
        if (y2 <= 0.175) lo = mid
        else hi = mid
      } else if (y2 >= 0.365) hi = mid
      else lo = mid
    }
    return fromHsl(h, s, darker ? lo : hi)
  }

  // The island as the look makes it, on a strip over a desktop; big, it
  // opens under the pointer (open, above the strip, the page has nothing
  // behind it to blur: its ground alone, as on the taskbar; blurred where
  // Windows' acrylic goes under it, island-plate.js placePlate). At the top of
  // the screen, a pill at the top of a desktop, opening down; blurred only
  // by that acrylic, open or not.
  function ilookPreview(mode, look, big = false) {
    const [r, g, b] = [1, 3, 5].map(i => parseInt(look.bg.slice(i, i + 2), 16))
    // Its words, and the moods' colours, as light or dark as its ground is.
    const inkDark = inkDarkOn(look.bg)
    const moods = inkDark ? COLOR_DEEP : COLOR
    const ring = moods[core.snap.now?.mood] || moods.working
    const top = isTop()
    const line = top ? (inkDark ? 'rgba(0,0,0,0.08)' : 'rgba(255,255,255,0.07)') : mode === 'dark' ? 'rgba(255,255,255,0.1)' : 'rgba(0,0,0,0.08)'
    const edge = look.edge === 'ring' ? `0 0 0 1.5px ${ring}, 0 0 12px ${ring}59` : look.edge === 'none' ? '0 0 0 0 transparent' : `0 0 0 1px ${line}`
    const plate = core.snap.plate === true && look.op < 100 && look.blur > 0
    const blur = look.blur && (plate || !top) ? `blur(${look.blur}px) saturate(1.25)` : 'none'
    const style = `--pv-bg:rgba(${r},${g},${b},${look.op / 100});--pv-blur:${blur};--pv-edge:${edge};--pv-ring:${ring}`
    const more = big ? `<span class="more"><span>${esc(T('s.ilookSample2'))}</span><span>${esc(T('s.ilookSample3'))}</span></span>` : ''
    return `<div class="ilook-pv ${mode}${inkDark ? ' ink-dark' : ''}${look.bare ? ' bare' : ''}${big ? ' big' : ''}${plate ? ' plate' : ''}${top ? ' top' : ''}" style="${style}"><span class="cap"><span class="top"><i class="face"></i><span class="w">${esc(T('s.ilookSample'))}</span><span class="t">1:42</span></span>${more}</span></div>`
  }

  function ilookPanel() {
    const s = core.snap.settings
    const mode = ilookFor()
    const look = ilookOf(mode)
    const active = ilookPreset(mode)
    const top = isTop()
    // On the black strip in the dark, she is part of it: none of this
    // shows. At the top of the screen, its own look.
    const isCapsule = (s.taskbarMaterial || 'clear') !== 'black' || winMode() === 'light'
    const note = top ? T('s.ilookTop') : isCapsule ? '' : T('s.ilookBlack')
    const cards = ilookPresets(mode)
      .map(([id, p]) => `<button class="ilook-card${id === active ? ' on' : ''}" data-ilook-preset="${id}">${ilookPreview(mode, p)}<span>${esc(ilookName(mode, id))}</span></button>`)
      .join('')
    const swatches = ILOOK_SWATCHES[mode]
      .map(c => `<button class="swc${c === look.bg.toLowerCase() ? ' on' : ''}" data-ilook-bg="${c}" style="background:${c}" title="${c}"></button>`)
      .join('')
    const picker = `<span class="ilook-sw">${swatches}<input type="color" class="swc-in" data-ilook="bg" value="${esc(look.bg)}" title="${esc(T('s.ilookPick'))}"></span>`
    const range = (key, min, max, unit) => `<span class="ilook-rng"><input type="range" class="rng" data-ilook="${key}" min="${min}" max="${max}" step="1" value="${look[key]}"><span class="rv">${look[key]}${unit}</span></span>`
    // At the top no bare capsule, and a blur only where Windows' acrylic
    // can go under the island.
    const blurRow = top && core.snap.plate !== true ? '' : row(esc(T('s.ilookBlur')), esc(T(top ? 's.ilookBlurTopNote' : 's.ilookBlurNote')), range('blur', 0, 60, 'px'))
    return `<div class="ilook">
        ${note ? `<div class="note">${esc(note)}</div>` : ''}
        <div class="ilook-top">${seg('ilookMode', [['dark', T('s.ilookDark')], ['light', T('s.ilookLight')]], mode)}<span class="grow"></span><button class="pbtn sm" data-ilook-reset ${sameLook(look, ilookDefault(mode)) ? 'disabled' : ''}>${esc(T('s.ilookReset'))}</button></div>
        ${mode === winMode() ? '' : `<div class="note">${esc(T('s.ilookOther'))}</div>`}
        ${ilookPreview(mode, look, true)}
        <div class="ilook-cards">${cards}</div>
        ${row(esc(T('s.ilookBg')), esc(T('s.ilookBgNote')), picker)}
        ${row(esc(T('s.ilookOp')), '', range('op', 20, 100, '%'))}
        ${blurRow}
        ${row(esc(T('s.ilookEdge')), '', seg('ilookEdge', ['none', 'line', 'ring'].map(e => [e, T(`s.ilookEdge.${e}`)]), look.edge))}
        ${top ? '' : row(esc(T('s.ilookBare')), esc(T('s.ilookBareNote')), sw('ilookBare', look.bare))}
      </div>`
  }

  // A slider moved (shown here) or let go (kept); a colour picked, moved
  // out of the greys no words read on.
  function ilookInput(target, keep) {
    const mode = ilookFor()
    const key = target.dataset.ilook
    const value = key === 'bg' ? ilookSteer(target.value) : Number(target.value)
    if (key === 'bg' && value !== target.value.toLowerCase()) target.value = value
    ilookSave(mode, { ...ilookOf(mode), [key]: value }, keep)
  }

  // The taskbar's layout (island-layout.js tbLayout): a preset, or one dragged
  // into shape on the strip itself (the button: the settings close for it).
  // Split in two (taskbarEdge split, docs/design/specs/dock-topbar.md): the
  // top bar's thickness, the dock out of the way of windows or in the room
  // Windows' own keeps, quick settings and the clock in the top bar or the dock.
  function splitRows() {
    const s = core.snap.settings
    if (s.display !== 'split') return ''
    const bar = s.splitBar === 48 ? 48 : 32
    const dock = s.splitDock === 'room' ? 'room' : 'hide'
    const keys = s.splitKeys === 'dock' ? 'dock' : 'top'
    return (
      row(esc(T('s.splitBar')), esc(T('s.splitBarNote')), seg('splitBar', [32, 48].map(h => [h, T('s.splitBar.px', { n: h })]), bar)) +
      row(esc(T('s.splitDock')), esc(T(`s.splitDockNote.${dock}`)), seg('splitDock', ['hide', 'room'].map(d => [d, T(`s.splitDock.${d}`)]), dock)) +
      row(esc(T('s.splitKeys')), esc(T(`s.splitKeysNote.${keys}`)), seg('splitKeys', ['top', 'dock'].map(k => [k, T(`s.splitKeys.${k}`)]), keys))
    )
  }

  function layoutRow() {
    // The top bar (split): no presets, its own default (island.js SPLIT_PRESET); edited as ever.
    if (core.snap.settings.display === 'split') return row(esc(T('s.taskbarLayout')), esc(T('s.layoutSplit')), `<button class="pbtn sm" data-tb-edit>${esc(T('s.layoutEdit'))}</button>`)
    const preset = Island?.tbPreset?.() || 'win11'
    const presets = ['win11', 'left', 'herRight'].map(p => [p, T(`s.layout.${p}`)])
    const note = T(preset === 'custom' ? 's.layoutCustom' : 's.layoutNote')
    return row(esc(T('s.taskbarLayout')), esc(note), `<span class="btns">${seg('taskbarLayout', presets, preset)}<button class="pbtn sm" data-tb-edit>${esc(T('s.layoutEdit'))}</button></span>`)
  }

  // Where her island sits at the top of the screen (zhoupengjie/Wakuwaku#59):
  // a top corner, or the top centre (as ever, the default); it opens
  // downward and toward the screen's middle (island.js anchor). Round a
  // Mac's notch, the centre only. On the taskbar she is where her capsule
  // is dragged: a line saying so instead, and the way there, the layout
  // edited as her menu's 编辑任务栏布局 does (data-tb-edit).
  function islandPosRow() {
    const s = core.snap.settings
    // On the top bar (split), its words: the top bar's layout, as her menu's (tray.rs editTopBar).
    if (onTaskbar(s)) {
      const top = s.display === 'split'
      return row(esc(T('s.islandPos')), esc(T(top ? 's.islandPosTopBar' : 's.islandPosTaskbar')), `<button class="pbtn sm" data-tb-edit>${esc(T(top ? 'menu.editTopBar' : 's.islandPosEdit'))}</button>`)
    }
    const notch = Island?.notched?.() === true
    // In Windows' taskbar (islandEdgeRow): its left end or its right end,
    // where the capsule inside it takes its room (embed.rs); laid over it
    // (the embedding off), the middle of it too (s.islandPosBar.*).
    const bar = !notch && s.islandEdge === 'taskbar'
    const embed = bar && s.islandEmbed !== false
    const pos = !notch && ['left', 'right'].includes(s.islandPos) ? s.islandPos : embed ? 'right' : 'center'
    const places = (embed ? ['left', 'right'] : ['left', 'center', 'right']).map(p => [p, T(`s.islandPos${bar ? 'Bar' : ''}.${p}`), notch && p !== 'center'])
    return row(esc(T('s.islandPos')), esc(T(notch ? 's.islandPosNotch' : 's.islandPosNote')), seg('islandPos', places, pos))
  }

  // Where her island is (zhoupengjie/Wakuwaku#105): at the top of the
  // screen (as ever, the default); laid over Windows' taskbar, along the
  // bottom or the top; or inside it, its window a child of the taskbar's
  // while closed, as TrafficMonitor's (embed.rs). Three choices side by
  // side for two settings (islandEdge top | taskbar, islandEmbed), with
  // no words under them (the user, 2026-11-07: 嵌入开关取消，直接并行; 放在不需要小字解释).
  // Round a Mac's notch, the top only; on a Mac no taskbar to go into.
  // On the taskbar home none: the strip has its own edge row.
  function islandEdgeRow() {
    const s = core.snap.settings
    const notch = Island?.notched?.() === true
    const where = notch || s.islandEdge !== 'taskbar' ? 'top' : s.islandEmbed === false ? 'over' : 'taskbar'
    const places = ['top', 'over', 'taskbar'].map(e => [e, T(`s.islandWhere.${e}`), (notch && e !== 'top') || (pet.mac && e === 'taskbar')])
    return row(esc(T('s.islandEdge')), notch ? esc(T('s.islandEdgeNotch')) : '', seg('islandWhere', places, where))
  }

  // At the top of the screen, up out of the way while a window lies under
  // it (island-plate.js setStow, zhoupengjie/Wakuwaku#49): on unless turned off.
  // Not on a Mac, where it hangs over the menu bar.
  function hideUnderRow() {
    return row(esc(T('s.islandHideUnder')), esc(T('s.islandHideUnderNote')), sw('islandHideUnder', core.snap.settings.islandHideUnder !== false))
  }

  // The fold's own row: how the island looks in Windows' mode now.
  function ilookRow() {
    const mode = winMode()
    const now = T('s.ilookNow', { mode: T(mode === 'dark' ? 's.ilookDark' : 's.ilookLight'), name: ilookName(mode, ilookPreset(mode)) })
    return `${row(esc(T('s.ilook')), esc(now), `<button class="pbtn sm" data-ilook-fold>${esc(T(core.ilookOpen ? 's.ilookFold' : 's.ilookAdjust'))}</button>`)}${core.ilookOpen ? ilookPanel() : ''}`
  }

  function pageLook() {
    const s = core.snap.settings
    return `${sec(T('home.display'))}<div class="modes">
        <div class="mode${onTaskbar(s) ? '' : ' on'}" data-mode="island"><div class="pv"><i></i></div><div class="l"><span class="rd"></span>${esc(T('s.displayIsland'))}</div></div>
        <div class="mode${s.display === 'taskbar' ? ' on' : ''}" data-mode="taskbar"><div class="pv"><s></s></div><div class="l"><span class="rd"></span>${esc(T('s.displayTaskbar'))}</div></div>
        <div class="mode${s.display === 'split' ? ' on' : ''}" data-mode="split"><div class="pv"><u></u><b></b></div><div class="l"><span class="rd"></span>${esc(T('s.displaySplit'))}</div></div>
      </div>
      ${onTaskbar(s) ? `<div class="grp"><div class="note">${esc(T(s.display === 'split' ? 's.splitNote' : 's.taskbarNote'))}</div></div><div class="grp">${s.display === 'split' ? splitRows() : row(esc(T('s.taskbarEdge')), esc(T('s.taskbarEdgeNote')), seg('taskbarEdge', ['bottom', 'top', 'left', 'right'].map(e => [e, T(`s.edge.${e}`)]), s.taskbarEdge || 'bottom'))}${core.screenRow()}${core.displaysRow()}${row(esc(T('s.taskbarMaterial')), esc(T('s.taskbarMaterialNote')), seg('taskbarMaterial', ['mica', 'black', 'clear'].map(m => [m, T(`s.material.${m}`)]), s.taskbarMaterial || 'mica'))}${s.taskbarMaterial === 'clear' ? row(esc(T('s.clearWhen')), esc(T('s.clearWhenNote')), seg('taskbarClearWhen', ['mica', 'solid', 'always'].map(w => [w, T(`s.clearWhen.${w}`)]), ['solid', 'always'].includes(s.taskbarClearWhen) ? s.taskbarClearWhen : 'mica')) : ''}${s.display === 'split' || ['left', 'right'].includes(s.taskbarEdge) ? row(esc(T('s.taskbarButtons')), esc(T(s.display === 'split' ? 's.taskbarButtonsDock' : 's.taskbarButtonsSide')), `<span class="seg off">${['icons', 'labels'].map(b => `<span class="${b === 'icons' ? 'on' : ''}">${esc(T(`s.buttons.${b}`))}</span>`).join('')}</span>`) : row(esc(T('s.taskbarButtons')), esc(T('s.taskbarButtonsNote')), seg('taskbarButtons', ['icons', 'labels'].map(b => [b, T(`s.buttons.${b}`)]), s.taskbarButtons || 'icons'))}${layoutRow()}${ilookRow()}</div>` : ''}
      ${part('place', '')}<div class="grp">${onTaskbar(s) ? '' : core.screenRow()}${islandPosRow()}${onTaskbar(s) ? '' : islandEdgeRow()}${row(esc(T('s.islandWidth')), esc(T('s.islandWidthNote')), seg('islandWidth', ['narrow', 'normal', 'wide'].map(w => [w, T(`s.width.${w}`)]), s.islandWidth || 'normal'))}${onTaskbar(s) || pet.mac || s.islandEdge === 'taskbar' ? '' : hideUnderRow()}${row(esc(T('s.oledShift')), esc(T('s.oledShiftNote')), sw('oledShift', s.oledShift))}${onTaskbar(s) ? '' : ilookRow()}</div>
      ${pageWindows()}
      ${sec(T('s.her'))}<div class="grp">
        ${row(esc(T('menu.size')), '', seg('scale', SIZES.map(([name, scale]) => [scale, T(`menu.${name}`)]), s.scale))}
        ${row(esc(T('s.details')), esc(T('s.detailsNote')), sw('details', s.details !== false))}
        ${row(esc(T('s.walk')), esc(T('s.walkNote')), sw('walk', s.walk))}
        ${row(esc(T('s.look')), '', sw('look', s.look))}
      </div>
      ${part('pets', pagePets())}`
  }

  // Windows' own mode and accent colour, set as Settings → Personalization →
  // Colors sets them (theme.rs): its 48 colours to pick from.
  const ACCENTS = [
    '#ffb900', '#ff8c00', '#f7630c', '#ca5010', '#da3b01', '#ef6950', '#d13438', '#ff4343',
    '#e74856', '#e81123', '#ea005e', '#c30052', '#e3008c', '#bf0077', '#c239b3', '#9a0089',
    '#0078d7', '#0063b1', '#8e8cd8', '#6b69d6', '#8764b8', '#744da9', '#b146c2', '#881798',
    '#0099bc', '#2d7d9a', '#00b7c3', '#038387', '#00b294', '#018574', '#00cc6a', '#10893e',
    '#7a7574', '#5d5a58', '#68768a', '#515c6b', '#567c73', '#486860', '#498205', '#107c10',
    '#767676', '#4c4a48', '#69797e', '#4a5459', '#647c64', '#525e54', '#847545', '#7e735f',
  ]
  // The accent colours held back for now: DWM's frames' colour does not yet
  // follow them for sure (theme.rs set_accent), 2026-10-10.
  const ACCENT_PICKER = false
  function pageWindows() {
    const look = core.snap.winLook || {}
    const accent = String(look.accent || '').toLowerCase()
    // One picked outside these (its own colour in Windows' settings): last, ringed.
    const colours = /^#[0-9a-f]{6}$/.test(accent) && !ACCENTS.includes(accent) ? [...ACCENTS, accent] : ACCENTS
    const swatches = colours.map(c => `<span class="acc${c === accent ? ' on' : ''}" data-accent="${c}" style="background:${c}" title="${c}"></span>`).join('')
    return `${sec(T('s.windows'))}<div class="grp">
        ${row(esc(T('s.winMode')), esc(T('s.winModeNote')), seg('winMode', [['light', T('s.winMode.light')], ['dark', T('s.winMode.dark')]], look.light ? 'light' : 'dark'))}
        ${ACCENT_PICKER ? `<div class="r col"><div>${esc(T('s.winAccent'))}</div><div class="accents">${swatches}</div></div>` : ''}
      </div>
      ${pagePaper()}`
  }

  const lhmAsks = () => Date.now() - core.lhmAsking < 5000 || ['install', 'elevate', 'start'].includes(core.snap?.lhmSetup?.job?.stage)
  // One ask at a time: its buttons grey until Windows has done it (a step
  // waits up to a few seconds to see the picture change; presses meanwhile
  // went on stepping, 2026-10-10).
  let paperBusy = false

  // Asked of Windows for the desktop's picture; the snapshot as it is after.
  function paperDo(what, value) {
    if (paperBusy) return
    paperBusy = true
    core.pickingPaper = what === 'picture' || what === 'folder'
    draw()
    return pet.settings
      .wallpaper(what, value)
      .then(got => {
        core.snap = got
      })
      .finally(() => {
        paperBusy = false
        core.pickingPaper = false
        draw()
      })
  }

  // The desktop's picture (paper.rs): one picture or a folder's in turn,
  // picked in Windows' file dialog; how it is laid; the turns.
  const PAPER_POSITIONS = ['fill', 'fit', 'stretch', 'tile', 'center', 'span']
  const PAPER_EVERY = [60000, 600000, 1800000, 3600000, 21600000, 86400000]
  const lastPart = path => String(path || '').split(/[\\/]/).filter(Boolean).pop() || ''
  function pagePaper() {
    const paper = core.snap.winPaper
    if (!paper) return ''
    // The slideshow on a wallpaper pack's folder: named as the pack.
    const inUse = (core.snap.packs || []).find(p => p.kind === 'wallpaper' && p.inUse)
    const now = inUse
      ? T('s.paperNowPack', { name: inUse.name })
      : paper.slideshow
        ? T('s.paperNowFolder', { name: lastPart(paper.folder) || T('s.paperUnknown') })
        : T('s.paperNowPicture', { name: lastPart(paper.file) || T('s.paperUnknown') })
    const every = PAPER_EVERY.map(ms => [ms, ms >= 86400000 ? T('s.paperDay') : ms >= 3600000 ? T('s.paperHours', { n: ms / 3600000 }) : T('settings.minutes', { n: ms / 60000 })])
    const button = (what, label) => `<button class="pbtn sm" data-paper="${what}" ${paperBusy ? 'disabled' : ''}>${esc(T(label))}</button>`
    // Back to the picture before the last change made here, while there is one.
    const pick = `<span class="btns">${button('picture', 's.paperPicture')}${button('folder', 's.paperFolder')}${paper.canUndo ? button('undo', 's.paperUndo') : ''}</span>`
    // Under it the wallpaper packs (spec §1; packs.rs): each its picture,
    // name, 「壁纸包 · N 张」, and 用这一包 (its folder as 选文件夹… would
    // set it: settings_wallpaper pack), or 在用 while the slideshow is on
    // it; then 获取更多壁纸…, with them.
    const packs = (core.snap.packs || [])
      .filter(p => p.kind === 'wallpaper')
      .map(p => {
        const end = p.inUse ? `<span class="d in-use">${esc(T('packs.inUse'))}</span>` : `<button class="pbtn sm" data-paper="pack" data-pack-id="${esc(p.id)}" ${paperBusy ? 'disabled' : ''}>${esc(T('packs.use'))}</button>`
        return `<div class="r pk-row">${core.packTile(p)}<div class="grow"><div class="ellip">${esc(p.name)}</div><div class="d ellip">${esc(AddonUI.wallLine(p, T))}</div></div>${end}</div>`
      })
      .join('')
    return `${part('paper', '')}${sec(T('s.wallpaper'))}<div class="grp">
        ${row(esc(T('s.paperNow')), esc(now), pick)}
        ${packs}${toMarketRow('wallpaper', 'packs.moreWalls', 'packs.moreWallsNote')}
        ${row(esc(T('s.paperFit')), '', seg('paperPosition', PAPER_POSITIONS.map(p => [p, T(`s.paperPos.${p}`)]), paper.position))}
        ${
          paper.slideshow
            ? `${row(esc(T('s.paperEvery')), '', seg('paperEvery', every, paper.every))}
        ${row(esc(T('s.paperShuffle')), '', sw('paperShuffle', paper.shuffle))}
        ${row(esc(T('s.paperNext')), '', `<span class="btns">${button('previous', 's.paperPrevButton')}${button('next', 's.paperNextButton')}</span>`)}`
            : ''
        }
      </div>`
  }

  return { ACCENT_PICKER, ilookDefault, ilookFor, ilookInput, ilookOf, ilookPresets, ilookSave, lhmAsks, pageLook, paperDo, winMode }
}
