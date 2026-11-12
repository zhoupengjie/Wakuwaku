// The taskbar's right part: the tray's box, the input method, Caps Lock, the
// network, the volume and the battery (taskbar:keys), what is using the
// microphone, camera or location, the clock and the program in front.
// Made by island.ts in the section's place as the page starts
// (rightPart(core), imported by island.ts). The island's core (IslandCore):
// its constants and functions as they are, what it remembers in core.st
// (island-vars.ts, read and written as st.<name>), and the parts made
// before this one in core.pieces (a later one's reached once the page is
// up). What it hands back is the island's again.
import * as Widgets from './widgets.ts'
import { pet } from './bridge.ts'
import type { IslandCore } from './island.ts'

export function rightPart(core: IslandCore) {
  const {
    st, pieces, Widgets, el, home, isDetailed, isPinned, isSide, island, stripHue, stripInk, t,
  } = core

  // Start and the programs, the monitor, the tray's icons, the clock and the
  // settings. Its left edge stays where the compact island ends, so what
  // opens from the island grows up over it. The other sessions have no tags
  // on it: their windows' buttons go to them, and the island says how they
  // are (the user's call, 2026-10-12).
  const barRest = document.createElement('div')
  barRest.id = 'bar-rest'
  // Keys as Windows' own taskbar takes them: a screen reader in a page
  // reads it by its arrows (Narrator's scan mode) and the arrows never came
  // to her buttons; as an application they come (tried, 2026-10-14). Named
  // as the keyboard comes to it (keys, below).
  barRest.setAttribute('role', 'application')
  island.after(barRest)

  // The widget shown, and the monitor, pinned: always there by the settings.
  // In the taskbar only the monitor (before the tray), in two rows as the
  // clock has its two, coloured as Windows' own taskbar would (style.css):
  // the widgets take their turns at its left end, in her place (fillCompact).
  function widgetFaces() {
    const faces = []
    const w = st.widgets.length && home() !== 'taskbar' ? st.widgets[st.widgetAt % st.widgets.length] : null
    if (w) {
      const { label, value } = Widgets.words(st.lang, w, isDetailed())
      const face = el('span', 'face')
      face.insertAdjacentHTML('afterbegin', Widgets.iconHTML(w, st.lang))
      ;(face.firstChild as HTMLElement).style.color = stripInk(w.color) || stripHue().idle
      const v = el('span', 'v', value)
      v.style.color = stripInk(w.color) || stripHue().idle
      if (Widgets.partsOf(w)) v.innerHTML = Widgets.partsHTML(w)
      face.append(Widgets.partsOf(w) ? '' : label, v)
      faces.push(face)
    }
    if (isPinned()) {
      const inRows = home() === 'taskbar'
      const readings = el('span', inRows ? 'face mon rows' : 'face mon')
      readings.innerHTML = inRows ? Widgets.rowsHTML(st.monitor, isSide()) : Widgets.partsHTML(st.monitor)
      faces.push(readings)
    }
    return faces
  }

  function fillBar() {
    if (home() === 'taskbar') pieces.layout.fillTaskbar()
  }

  // The taskbar's parts that stay put: rebuilt every second with the
  // clocks, the tray's icons would lose the pointer and blink.
  const startButton = el('span', 'start')
  startButton.dataset.bar = 'start'
  // Windows 11's mark, drawn as Windows draws it (its own is drawn by its
  // taskbar's code: no file or icon font holds it), as measured on its own
  // taskbar: as big as the programs' icons (29 of the 30 screen pixels of
  // 24px at 125%), four panes in one blue running from light at the top left
  // to deep at the bottom right (style.css .s0, .s1), a one-pixel gap. On
  // the screen's own pixels, as Windows': the panes in whole device pixels
  // for the screen's scale, and the mark moved by the part of a pixel it
  // lands off one (snapStart), or the gap blurs.
  let startScale = 0
  function drawStartMark() {
    const dpr = devicePixelRatio || 1
    if (dpr === startScale) return
    startScale = dpr
    const box = Math.round(24 * dpr)
    const gap = Math.max(1, Math.round(0.8 * dpr))
    const pane = Math.floor((box - 1 - gap) / 2)
    const at = Math.floor((box - 2 * pane - gap) / 2)
    const far = at + pane + gap
    const end = far + pane
    const r = Math.max(0.5, 0.8 * dpr).toFixed(2)
    const rect = (x, y) => `<rect x="${x}" y="${y}" width="${pane}" height="${pane}" rx="${r}"/>`
    startButton.innerHTML = `<svg viewBox="0 0 ${box} ${box}" style="width:${box / dpr}px;height:${box / dpr}px"><defs><linearGradient id="start-blue" gradientUnits="userSpaceOnUse" x1="${at}" y1="${at}" x2="${end}" y2="${end}"><stop class="s0" offset="0"/><stop class="s1" offset="1"/></linearGradient></defs><g fill="url(#start-blue)">${rect(at, at)}${rect(far, at)}${rect(at, far)}${rect(far, far)}</g></svg>`
  }
  // Moved by its layout (position: relative), not by a transform: a moved
  // layer is drawn again off the grid, blurring it (2026-10-10, on the
  // live taskbar). The part of the taskbar it is in starts on the grid too
  // (onPixels in fillTaskbar).
  function snapStart() {
    const mark = startButton.firstElementChild as SVGSVGElement | null
    if (!mark || !mark.isConnected || startButton.classList.contains('down')) return
    const dpr = devicePixelRatio || 1
    mark.style.left = '0px'
    mark.style.top = '0px'
    const r = mark.getBoundingClientRect()
    const off = v => (Math.round(v * dpr) - v * dpr) / dpr
    mark.style.left = `${off(r.left).toFixed(3)}px`
    mark.style.top = `${off(r.top).toFixed(3)}px`
  }
  // A length that starts and ends on the screen's own pixels.
  const onPixels = v => Math.round(v * (devicePixelRatio || 1)) / (devicePixelRatio || 1)
  drawStartMark()
  window.addEventListener('resize', () => {
    drawStartMark()
    snapStart()
  })
  // As Windows 11's: its mark pressed in under the pointer and springing
  // back; a bounce when Windows' Start menu opens by the Windows key, and
  // lit while it is open (taskbar/ taskbar:start). Pressed just now, the
  // press was its bounce.
  let startPressedAt = 0
  startButton.addEventListener('pointerdown', e => {
    if (e.button !== 0) return
    startButton.classList.add('down')
    startPressedAt = Date.now()
  })
  for (const type of ['pointerup', 'pointerleave', 'pointercancel']) startButton.addEventListener(type, () => startButton.classList.remove('down'))
  startButton.addEventListener('animationend', () => startButton.classList.remove('bounce'))
  pet.onStart(({ open }: { open?: boolean } = {}) => {
    startButton.classList.toggle('open', !!open)
    if (!open || Date.now() - startPressedAt < 800) return
    startButton.classList.remove('bounce')
    void startButton.offsetWidth
    startButton.classList.add('bounce')
  })
  // Task View, after Start as on Windows' own taskbar, while Windows has its
  // button on (taskbar:look taskView): a press opens it (Win+Tab). Windows'
  // own pictures (taskbar_task_view_icons: Settings' two, for a light strip
  // and a dark one, style.css .tv-light, .tv-dark); where Windows has none
  // (Windows 10), the Task View glyph of its icon font. One part with Start
  // in the layout (PART_NODES.start), as on Windows' they go together.
  const taskViewButton = el('span', 'tview')
  taskViewButton.dataset.bar = 'tasks'
  taskViewButton.hidden = true
  taskViewButton.innerHTML = '<i class="tv-glyph"></i>'
  Promise.resolve(pet.taskbar.taskViewIcons?.()).then(icons => {
    if (!icons?.light || !icons?.dark) return
    const img = (svg, cls) => `<img class="${cls}" alt="" draggable="false" src="data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}">`
    taskViewButton.innerHTML = img(icons.light, 'tv-light') + img(icons.dark, 'tv-dark')
  })
  const startPart = el('span', 'tstart')
  startPart.append(startButton, taskViewButton)
  const windowsBox = el('span', 'wins')
  const facesBox = el('span', 'faces')
  const trayBox = el('span', 'tray')
  // The keyboard: Caps Lock while it is on, and the input method's script
  // or picture (a press switches its script; a right press, the input
  // methods to pick from).
  const keysBox = el('span', 'kbd')
  const capsMark = el('span', 'caps', 'A')
  const imeMark = el('span', 'ime')
  imeMark.dataset.bar = 'ime'
  keysBox.append(capsMark, imeMark)
  // Windows' quick settings (Wi-Fi, Bluetooth, volume…), as its own taskbar
  // has them: the network's way out (by cable, Wi-Fi, a VPN's, or none) and
  // the volume, in its own icon font (.fi: Segoe Fluent Icons).
  const quickBox = el('span', 'tquick')
  quickBox.dataset.bar = 'quick'
  const NET = { wired: '', wifi: '', other: '', none: '' }
  const netMark = el('span', 'fi net')
  // The Wi-Fi as Windows 11 draws it, as the volume is: all its arcs faint,
  // those its signal reaches solid over them (keys.bars, tasks.rs
  // wifi_bars: Windows' own 0 to 5 bars, to its four: the dot alone, one
  // arc, two, all); all of them solid while not known. It was always all.
  const WIFI = ['', '', '', '']
  const netBack = el('span', 'back')
  const netFront = el('span', 'front')
  netMark.append(netBack, netFront)
  // The volume as Windows draws it: the speaker's three waves faint, those up
  // to the volume solid over them; struck through, muted.
  const volMark = el('span', 'fi vol')
  const volBack = el('span', 'back', '')
  const volFront = el('span', 'front', '')
  volMark.append(volBack, volFront)
  // The battery after them, as Windows' own taskbar has it on a laptop (none
  // on a desktop), in Windows' own battery font (SysBatt Fluent Icons, read
  // from Windows: taskbar_battery_font) as its taskbar draws it: an outline,
  // a bolt in it while charging, a plug while plugged in and not charging (a
  // battery its laptop keeps at 80% too), and over it the level in tenths, in
  // the fills cut for the bolt or the plug; green while charging, yellow with
  // the energy saver on, red when critically low. Its percentage beside it as
  // Windows' "Battery percentage" has it. Without the font (before Windows
  // 11 has it), Segoe's battery glyphs, one for each tenth.
  const batMark = el('span', 'bat')
  batMark.hidden = true
  const batBase = el('span', 'fi base')
  const batFill = el('span', 'fi fill')
  const batGlyph = el('span', 'glyph')
  batGlyph.append(batBase, batFill)
  const batText = el('span', 'pct')
  batMark.append(batGlyph, batText)
  // Windows': each outline; the plain fills for tenths 1 to 10 after the
  // plain outline, the cut ones for 3 to 10 after the bolt's and the plug's
  // (1 and 2 do not reach the bolt or the plug, and are the plain ones).
  const SYSBATT = { plain: 0xf8d0, bolt: 0xf8db, plug: 0xf8e4 }
  const BATTERY = {
    level: ['', '', '', '', '', '', '', '', '', '', ''],
    charging: ['', '', '', '', '', '', '', '', '', '', ''],
    saver: ['', '', '', '', '', '', '', '', '', '', ''],
  }
  let sysBatt = false
  let batShown = null
  pet.taskbar
    .batteryFont()
    .then(bytes => {
      // Bytes as they are, or as numbers where the IPC had to send them so.
      const font = Array.isArray(bytes) ? new Uint8Array(bytes).buffer : bytes
      if (!(font instanceof ArrayBuffer) || !font.byteLength) return
      const face = new FontFace('SysBatt Fluent Icons', font)
      return face.load().then(() => {
        document.fonts.add(face)
        sysBatt = true
        showBattery(batShown)
      })
    })
    .catch(() => {})
  quickBox.append(netMark, volMark, batMark)
  // The wheel turned on it: the volume, as on Windows' own taskbar
  // (zhoupengjie/Wakuwaku#35): a notch a step up or down (2%, by Windows'
  // volume keys, its own flyout shown); a touchpad's or a fine wheel's
  // small turns gathered to a notch's worth. Not while the layout is edited.
  let wheelGathered = 0
  quickBox.addEventListener(
    'wheel',
    e => {
      if (st.editLayout) return
      e.preventDefault()
      const dy = e.deltaMode === 1 ? e.deltaY * 33 : e.deltaY
      wheelGathered = Math.abs(dy) >= 50 ? Math.sign(dy) * 100 : wheelGathered + dy
      const steps = Math.trunc(wheelGathered / 100)
      if (!steps) return
      wheelGathered -= steps * 100
      pet.taskbar.volume(-steps)
    },
    { passive: false },
  )
  function showNet(kind, bars?) {
    const level = kind === 'wifi' && Number.isInteger(bars) ? Math.min(Math.max(bars, 1), 4) : 0
    netFront.textContent = level ? WIFI[level - 1] : NET[kind] || NET.other
    netBack.textContent = level ? NET.wifi : ''
    netBack.hidden = !level
  }
  showNet('other')
  function showVolume(v) {
    const [level, muted] = Array.isArray(v) ? v : [null, false]
    volFront.textContent = muted ? '' : level == null || level >= 67 ? '' : level >= 34 ? '' : level > 0 ? '' : ''
    // Muted, the waves' room is the struck-through speaker's, unseen.
    volBack.textContent = muted ? volFront.textContent : ''
    volBack.hidden = !!muted
    volMark.title = level == null ? '' : muted ? t(st.lang, 'taskbar.muted') : t(st.lang, 'taskbar.volume', { n: level })
  }
  // { percent, plugged, charging, saver, critical, left (s), shown } (widgets.rs battery_state), or none.
  function showBattery(b) {
    batShown = b
    batMark.hidden = !b
    if (!b) return
    const n = b.percent
    // In tenths as Windows has them: 96% full, 86% nine, and some left a
    // tenth (red when critically low); empty at none.
    const tenth = Math.max(0, Math.min(10, Math.ceil(n / 10)))
    const plugged = b.plugged || b.charging
    batMark.classList.toggle('sysbatt', sysBatt)
    if (sysBatt) {
      const outline = SYSBATT[b.charging ? 'bolt' : plugged ? 'plug' : 'plain']
      batBase.textContent = String.fromCharCode(outline)
      batFill.textContent = !tenth ? '' : String.fromCharCode(outline === SYSBATT.plain || tenth < 3 ? SYSBATT.plain + tenth : outline + tenth - 2)
    } else {
      batBase.textContent = (plugged ? BATTERY.charging : b.saver ? BATTERY.saver : BATTERY.level)[tenth]
      batFill.textContent = ''
    }
    // The fill's color, on battery alone but for charging's.
    batMark.dataset.tint = b.charging ? 'green' : plugged ? '' : b.critical ? 'red' : b.saver ? 'yellow' : ''
    batText.textContent = `${n}%`
    batText.hidden = !b.shown
    const left = Number.isFinite(b.left) ? Math.round(b.left / 60) : null
    const time = left == null ? '' : left >= 60 ? t(st.lang, 'taskbar.hm', { h: Math.floor(left / 60), m: left % 60 }) : t(st.lang, 'taskbar.m', { m: left })
    const said = plugged ? t(st.lang, 'taskbar.batteryPlugged', { n }) : time ? t(st.lang, 'taskbar.batteryLeft', { n, time }) : t(st.lang, 'taskbar.battery', { n })
    batMark.title = b.saver ? `${said}\n${t(st.lang, 'taskbar.batterySaver')}` : said
  }
  // What is using the microphone, the camera, the location, as Windows'
  // taskbar shows it before the input method; a press, its privacy settings.
  const PRIVACY = { mic: '', cam: '', loc: '' }
  const privBox = el('span', 'tpriv')
  let privDrawn = ''
  function showPrivacy(use) {
    const drawn = JSON.stringify(use || {})
    if (drawn === privDrawn) return
    privDrawn = drawn
    privBox.replaceChildren(
      ...Object.keys(PRIVACY)
        .filter(k => use?.[k]?.length)
        .map(k => {
          const mark = el('span', 'fi', PRIVACY[k])
          mark.dataset.bar = `privacy-${k}`
          mark.title = t(st.lang, `taskbar.inUse.${k}`, { apps: use[k].join(st.lang === 'zh' ? '、' : ', ') })
          return mark
        }),
    )
  }
  const clockBox = el('span', 'tclock')
  clockBox.dataset.bar = 'notifications'
  // The program in front, by its icon and name, as a menu bar has it
  // (macOS, GNOME): first on the top bar (split), its window's title in its
  // tip (drawFront). Elsewhere a part to be put on the strip if wanted.
  const frontBox = el('span', 'tfront')
  const frontIcon = el('img')
  frontIcon.alt = ''
  frontIcon.draggable = false
  frontIcon.hidden = true
  const frontName = el('b')
  frontBox.append(frontIcon, frontName)
  // Show the desktop: a sliver at the screen's right edge, as Windows' own (Win+D).
  const deskEdge = el('span', 'tdesk')
  deskEdge.dataset.bar = 'desktop'
  // The room between the middle run and the right one: what centring the
  // middle moves it into (alignApps).
  const growNode = el('span', 'grow')

  return { barRest, batMark, capsMark, clockBox, deskEdge, facesBox, fillBar, frontBox, frontIcon, frontName, growNode, imeMark, keysBox, netMark, onPixels, privBox, quickBox, showBattery, showNet, showPrivacy, showVolume, snapStart, startButton, startPart, taskViewButton, trayBox, volMark, widgetFaces, windowsBox }
}
