// Over a program's button on her taskbar on another display (bars.rs
// bar_pop): its windows, a card each with its live picture (Windows draws
// them where the cards leave room, taskbar/mod.rs thumbs), or, on a right
// press, what can be done with it (Windows' End task too while it is on);
// as her own taskbar's (island-winlist.ts drawPop).
// The window is laid against the strip at the size this lays it out at
// (barpop_place). The windows' list goes once the pointer has been off the
// button and off it a moment; either goes at a press elsewhere (bars.rs
// pressed) or once something in it is pressed.
import * as Presses from './presses.ts'
import { t } from '../shared/i18n.ts'

const { invoke } = window.__TAURI__.core
const { listen } = window.__TAURI__.event
const target: TauriTarget = { kind: 'WebviewWindow', label: 'barpop' }

const pop = document.getElementById('pop')
// Its room for the pointer out to the strip (barpop.css .reach, #56).
const reach = document.createElement('span')
reach.className = 'reach'
// Room round it for its shadow, but on the strip's side: there it is this
// far off the strip (as her own: 6 px, beside a side bar 8).
const SHADE = 14
const GAP = { bottom: 6, top: 6, left: 8, right: 8 }
// What it shows ({ kind, app, edge, look, lang }), as drawn last.
let shown = null
let drawn = ''
// The pointer on the strip's buttons (bars.rs bar_pop_over), on the list.
let overBar = true
let overPop = false
let closeTimer
let thumbTimer
let thumbsSent = ''

const el = (tag, cls?, text?) => {
  const n = document.createElement(tag)
  if (cls) n.className = cls
  if (text != null) n.textContent = text
  return n
}

function rows(s) {
  const app = s.app
  const windows = app.windows || []
  if (s.kind === 'list') {
    // A card for each: its icon, title and ✕, and room under them for its live picture.
    return windows.map(w => {
      const card = el('div', w.front ? 'wcard front' : 'wcard')
      card.dataset.win = w.id
      const top = el('div', 'wtop')
      if (w.png || app.png) {
        const img = el('img')
        img.src = w.png || app.png
        img.alt = ''
        top.append(img)
      }
      top.append(el('span', 'wt', w.title))
      const x = el('span', 'wx', '✕')
      x.dataset.close = w.id
      top.append(x)
      const room = el('div', 'wthumb')
      room.dataset.thumb = w.id
      card.append(top, room)
      return card
    })
  }
  const item = (text, act) => {
    const row = el('div', 'witem', text)
    row.dataset.act = act
    return row
  }
  const out = [el('div', 'whead', app.name), item(t(s.lang, windows.length ? 'taskbar.newWindow' : 'taskbar.open'), 'launch'), item(t(s.lang, app.pinned ? 'taskbar.unpin' : 'taskbar.pin'), app.pinned ? 'unpin' : 'pin')]
  // Windows' End task while its setting is on (bars.rs, read as it
  // opens), but for File Explorer, as on her own taskbar (island-winlist.ts drawPop).
  if (windows.length && s.endTask && !/(^|[\\/])explorer\.exe$/i.test(app.path || '')) out.push(item(t(s.lang, windows.length > 1 ? 'taskbar.endAll' : 'taskbar.end'), 'end'))
  if (windows.length) out.push(item(t(s.lang, windows.length > 1 ? 'taskbar.closeAll' : 'taskbar.close'), 'close'))
  return out
}

function draw(s) {
  const opening = pop.hidden || shown?.kind !== s.kind || shown?.app?.app !== s.app?.app
  shown = s
  const light = s.look?.mode === 'light'
  document.body.classList.toggle('light', light)
  const accent = s.look?.accent
  document.body.style.setProperty('--accent', (accent && (light ? accent.dark || accent.base : accent.light || accent.base)) || (light ? '#0067c0' : '#5e9bff'))
  const edge = ['top', 'left', 'right'].includes(s.edge) ? s.edge : 'bottom'
  for (const e of ['bottom', 'top', 'left', 'right']) document.body.classList.toggle(`e-${e}`, e === edge)
  // Drawn again only when what it shows changed: a card replaced under a press is never clicked.
  const key = JSON.stringify([s.kind, s.app.app, s.app.name, s.app.pinned, (s.app.windows || []).map(w => [w.id, w.title, w.front, (w.png || '').length]), s.lang, edge, light, s.endTask])
  if (key === drawn && !opening) return sendThumbs()
  drawn = key
  pop.replaceChildren(...rows(s), reach)
  pop.className = s.kind
  pop.hidden = false
  const pad = { top: SHADE, right: SHADE, bottom: SHADE, left: SHADE, [edge]: GAP[edge] }
  Object.assign(pop.style, { left: `${pad.left}px`, top: `${pad.top}px` })
  if (opening) {
    overBar = true
    overPop = false
    pop.classList.add('rising')
  }
  const w = pop.offsetWidth + pad.left + pad.right
  const h = pop.offsetHeight + pad.top + pad.bottom
  invoke('barpop_place', { w, h, dpr: devicePixelRatio || 1 }).catch(() => {})
  // The live pictures once it is in place (it rises a little first).
  thumbsSent = ''
  clearTimeout(thumbTimer)
  thumbTimer = setTimeout(sendThumbs, opening ? 200 : 30)
}

function sendThumbs() {
  const items =
    shown?.kind === 'list' && !pop.hidden
      ? [...pop.querySelectorAll('[data-thumb]')].map(r => {
          const b = r.getBoundingClientRect()
          return [Number(r.dataset.thumb), b.left, b.top, b.width, b.height]
        })
      : []
  const sent = JSON.stringify(items)
  if (sent === thumbsSent) return
  thumbsSent = sent
  invoke('barpop_thumbs', { items, dpr: devicePixelRatio || 1 }).catch(() => {})
}

function gone() {
  clearTimeout(closeTimer)
  clearTimeout(thumbTimer)
  shown = null
  drawn = ''
  thumbsSent = ''
  pop.hidden = true
  pop.replaceChildren()
}

function close() {
  gone()
  invoke('barpop_hide').catch(() => {})
}

// Off the button and off the list a moment: the windows' list goes.
function watchPointer() {
  clearTimeout(closeTimer)
  if (shown?.kind === 'list' && !overBar && !overPop) closeTimer = setTimeout(close, 400)
}
pop.addEventListener('mouseenter', () => {
  overPop = true
  watchPointer()
})
pop.addEventListener('mouseleave', () => {
  overPop = false
  watchPointer()
})
pop.addEventListener('animationend', () => pop.classList.remove('rising'))

// On the press, not the click: what was pressed may be drawn anew before the button comes up.
pop.addEventListener('pointerdown', e => {
  if (e.button !== 0 || !shown) return
  const app = shown.app
  const x = (e.target as HTMLElement).closest('[data-close]')
  if (x) return invoke('taskbar_window', { id: Number(x.dataset.close), what: 'close' }).catch(() => {})
  const card = (e.target as HTMLElement).closest('[data-win]')
  if (card) {
    close()
    return invoke('taskbar_window', { id: Number(card.dataset.win), what: 'front' }).catch(() => {})
  }
  const act = (e.target as HTMLElement).closest('[data-act]')?.dataset.act
  if (!act) return
  close()
  if (act === 'close' || act === 'end') for (const w of app.windows || []) invoke('taskbar_window', { id: Number(w.id), what: act }).catch(() => {})
  else invoke('taskbar_app', { path: String(app.path), what: act, id: app.id ? String(app.id) : null }).catch(() => {})
})
document.addEventListener('contextmenu', e => e.preventDefault())

// A file dragged from another program over the list (its program's button
// held over: bar.ts): a card held over half a second brings its window to
// the front, for the file to be let go of in it, as on Windows' own
// (zhoupengjie/Wakuwaku#36). The page hears no mouse while the drag is on
// it (Tauri's drop handler has it, tauri://drag-*, in physical px): the
// list goes once the drag has left it and the buttons a moment, as for
// the pointer.
let fileHeld = null
let fileTimer
for (const type of ['enter', 'over', 'leave', 'drop']) {
  listen(
    `tauri://drag-${type}`,
    ({ payload }) => {
      const p = type === 'enter' || type === 'over' ? payload?.position : null
      const node = p ? document.elementFromPoint(p.x / devicePixelRatio, p.y / devicePixelRatio) : null
      const onPop = !!node && pop.contains(node)
      if (onPop !== overPop) {
        overPop = onPop
        watchPointer()
      }
      const card = shown?.kind === 'list' && onPop ? node.closest('[data-win]') : null
      if ((card?.dataset.win ?? null) === fileHeld) return
      fileHeld = card?.dataset.win ?? null
      clearTimeout(fileTimer)
      if (card) fileTimer = setTimeout(() => invoke('taskbar_window', { id: Number(card.dataset.win), what: 'drag' }).catch(() => {}), Presses.DRAG_HOLD_MS)
    },
    { target },
  )
}

Promise.all([
  listen('barpop:show', e => e.payload?.app && draw(e.payload), { target }),
  listen(
    'barpop:bar',
    e => {
      overBar = e.payload?.over === true
      watchPointer()
    },
    { target },
  ),
  listen('barpop:gone', gone, { target }),
]).then(() => invoke('barpop_ready').catch(() => {}))
