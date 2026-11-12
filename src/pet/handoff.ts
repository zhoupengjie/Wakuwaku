// The handover between Windows' own taskbar and hers (taskbar/), so the
// foot of the screen is never bare. Starting, a picture of Windows' strip,
// taken before hers shows, is laid where the strip is, over it (covered:
// Windows' is hidden under it then, unseen); what is on it, its icons and
// words, crumbles into dust that lifts and settles into hers, each of her
// parts coming as its dust lands, the picture's bare ground fading into
// hers. Without a picture (none to take: Windows' strip hides itself, or was
// hidden already) the dust gathers from above. Quitting, hers crumbles the
// same way and drifts off, Windows' shown again under it once hers is bare.
// Never while the motion is to be kept down (Windows' animations off:
// prefers-reduced-motion): then each step is answered at once.
// On the island's page alone: made there by main.ts.
import { pet } from './bridge.ts'

// A point of dust, in the page's px, and its colour; on her part it comes
// to, if it is one.
interface Dot {
  x: number
  y: number
  c: string
  el?: HTMLElement
}
// A grain: where it sets out from, its time (ms, since it came) and its
// colour; then where it lifts to and lands, or how it drifts away. Each
// says the other's are not there: one with no tx drifts (frame), and tsc
// sees which also with null checks off (tsc -p test).
interface GrainBase {
  sx: number
  sy: number
  max: number
  delay: number
  life: number
  glint: boolean
  c: string
  showWait: boolean
}
interface Landing extends GrainBase {
  lx: number
  ly: number
  tx: number
  ty: number
  dx?: undefined
  rise?: undefined
  ph?: undefined
}
interface Drifting extends GrainBase {
  lx?: undefined
  ly?: undefined
  tx?: undefined
  ty?: undefined
  dx: number
  rise: number
  ph: number
}
type Grain = Landing | Drifting
// Windows' strip's picture, and how tall it is laid (the page's px).
type Picture = HTMLCanvasElement & { tall: number }

export function handoff() {

  // Her strip's parts that the dust becomes (and crumbles from).
  const PARTS = '#island, #bar-rest .start, #bar-rest .tview, #bar-rest .win, #bar-rest .tchev, #bar-rest .ticon, #bar-rest .kbd, #bar-rest .tquick, #bar-rest .tpriv, #bar-rest .face.mon, #bar-rest .tclock'
  const STRIP_H = 48
  // At most this many grains; their size, and every how many is a glint.
  const MOST = 1500
  const GRAIN = 1.6
  const GLINT_EVERY = 11
  // How long a grain takes (lifting, then settling), and the spread of their starts.
  const SETTLE_MS = 1050
  const SPREAD_MS = 200
  const GLINTS = ['#fff6d6', '#ffd27a', '#ffffff', '#ffe9a8']
  const quiet = () => matchMedia('(prefers-reduced-motion: reduce)').matches

  // --- The layers: the picture where the strip is, the dust over the whole window ------
  function canvasLayer(css: string) {
    const c = document.createElement('canvas')
    c.style.cssText = `position:fixed;pointer-events:none;${css}`
    document.body.append(c)
    return c
  }

  let picture: Picture | null = null
  let dust: HTMLCanvasElement | null = null
  // Each step its own: a picture still loading when the next step comes is
  // not laid (it would stay over hers).
  let step = 0
  const wait = (ms: number) => new Promise(r => setTimeout(r, ms))

  // Windows' strip, as its picture shows it (the display's whole width in its
  // pixels, as tall as Windows' strip is: not always hers), laid over where
  // it is, at the window's foot; resolves once it is on the screen: two
  // frames drawn. Not a moment after whatever came of them: on a first run
  // (the settings opening, the Mica's picture being drawn) the frames came
  // later than 120 ms, Windows' was put away first, and the bare desktop
  // showed under the strip for two or three frames (2026-10-13). 600 ms at
  // most, as no frames come while the window is hidden (the program waits
  // 800 for its answer anyway).
  async function cover(url: string, size?: number[]) {
    const mine = ++step
    drop()
    const img = new Image()
    img.src = url
    await img.decode()
    await sized(size)
    if (mine !== step) return
    const tall = (img.naturalHeight * innerWidth) / img.naturalWidth
    picture = canvasLayer(`left:0;bottom:0;width:100%;height:${tall}px;z-index:2147483000`) as Picture
    picture.tall = tall
    picture.width = img.naturalWidth
    picture.height = img.naturalHeight
    picture.getContext('2d', { willReadFrequently: true })!.drawImage(img, 0, 0)
    const drawn = await Promise.race([new Promise(r => requestAnimationFrame(() => requestAnimationFrame(() => r(true)))), wait(600).then(() => false)])
    if (!drawn) console.log('handoff: the picture laid, no frame drawn in 600 ms')
  }

  // The page as big as its window ([w, h], physical) by now, or half a
  // second gone: a window just made bigger reaches the page some frames
  // later, and the picture laid at the old size sat too small, up and to the
  // left, Windows' clock showing past its end.
  async function sized(size?: number[], ms = 500) {
    const [w, h] = Array.isArray(size) ? size : []
    if (!(w > 0 && h > 0)) return
    const there = () => Math.abs(innerWidth * devicePixelRatio - w) <= 2 && Math.abs(innerHeight * devicePixelRatio - h) <= 2
    const until = performance.now() + ms
    while (!there() && performance.now() < until) await Promise.race([new Promise(r => requestAnimationFrame(r)), wait(16)])
  }

  // Hers kept out of sight from the start until the picture is over Windows'
  // (her window shows a moment before it comes): it would show over Windows'
  // strip for a frame or two. Never longer than a few seconds (no handover
  // at all: something full screen, the taskbar not to be taken).
  document.documentElement.classList.add('handoff-wait')
  const unhide = () => document.documentElement.classList.remove('handoff-wait')
  setTimeout(unhide, 3000)

  function drop() {
    picture?.remove()
    picture = null
  }

  // What is on the picture, as grains (where, in the page's px, and their
  // colour), and the picture left bare: each column its ground's colour (as
  // a little below its top edge, which no icon reaches). Only with the
  // picture laid (the go step looks).
  function crumble() {
    const c = picture!.getContext('2d', { willReadFrequently: true })!
    const { width: w, height: h } = picture!
    const sx = w / innerWidth
    const sy = h / picture!.tall
    const img = c.getImageData(0, 0, w, h)
    const d = img.data
    const groundRow = Math.min(h - 1, Math.round(4 * sy))
    const ground = new Uint8ClampedArray(w * 3)
    for (let x = 0; x < w; x++) for (let k = 0; k < 3; k++) ground[x * 3 + k] = d[(groundRow * w + x) * 4 + k]
    const stride = Math.max(1, Math.round(1.5 * sy))
    const top = innerHeight - picture!.tall
    const grains: Dot[] = []
    for (let y = Math.round(2 * sy); y < h; y += stride)
      for (let x = 0; x < w; x += stride) {
        const i = (y * w + x) * 4
        const diff = Math.abs(d[i] - ground[x * 3]) + Math.abs(d[i + 1] - ground[x * 3 + 1]) + Math.abs(d[i + 2] - ground[x * 3 + 2])
        if (diff > 60) grains.push({ x: x / sx, y: top + y / sy, c: `rgb(${d[i]},${d[i + 1]},${d[i + 2]})` })
      }
    for (let y = Math.round(2 * sy); y < h; y++)
      for (let x = 0; x < w; x++) {
        const i = (y * w + x) * 4
        d[i] = ground[x * 3]
        d[i + 1] = ground[x * 3 + 1]
        d[i + 2] = ground[x * 3 + 2]
      }
    c.putImageData(img, 0, 0)
    return thin(grains)
  }

  // A point in the window clear of the strip, on the screen's side of it
  // (its edge, style.css tb-top, tb-left, tb-right; the bottom otherwise).
  function awayFromStrip() {
    const clear = STRIP_H + 40
    const c = document.body.classList
    const r = Math.random
    if (c.contains('tb-top')) return { x: r() * innerWidth, y: clear + r() * (innerHeight - clear) }
    if (c.contains('tb-left')) return { x: clear + r() * (innerWidth - clear), y: r() * innerHeight }
    if (c.contains('tb-right')) return { x: r() * (innerWidth - clear), y: r() * innerHeight }
    return { x: r() * innerWidth, y: r() * (innerHeight - clear) }
  }

  const thin = <T>(list: T[]) => (list.length <= MOST ? list : list.filter((_, i) => i % Math.ceil(list.length / MOST) === 0))

  // Points on her parts, as many as each one's size asks, each with a
  // colour: an icon's own (its picture's pixels), else the words' colour.
  function partsPoints(n: number) {
    const parts = [...document.querySelectorAll(PARTS)].filter(el => el.getClientRects().length && el.offsetWidth)
    const boxes = parts.map(el => ({ el, r: el.getBoundingClientRect() }))
    const total = boxes.reduce((a, b) => a + b.r.width * b.r.height, 0) || 1
    return boxes.flatMap(b => pointsOn(b.el, Math.max(5, Math.round((n * b.r.width * b.r.height) / total)), b.r))
  }

  // k points on one part, each with a colour of its own.
  function pointsOn(el: HTMLElement, k: number, r = el.getBoundingClientRect()): Dot[] {
    const colours = coloursOf(el)
    return Array.from({ length: k }, () => ({ el, x: r.left + r.width * (0.15 + 0.7 * Math.random()), y: r.top + r.height * (0.2 + 0.6 * Math.random()), c: colours[(Math.random() * colours.length) | 0] }))
  }

  // Parts drawn after the dust has set out (the programs' buttons come a
  // moment after the taskbar is taken, the tray's icons later): they come
  // in as the rest did, no dust of their own. Each had a burst of its own,
  // and the start looked as if it came in batches (the user, 2026-11-02).
  let late: MutationObserver | null = null
  // When the last of it lands (performance.now()).
  let lateEnd = 0
  function watchLate() {
    lateEnd = 0
    late?.disconnect()
    const bar = document.getElementById('bar-rest')
    if (!bar) return
    late = new MutationObserver(records => {
      const fresh = records.flatMap(r => [...r.addedNodes]).filter(n => n.nodeType === 1 && (n as Element).matches(PARTS) && (n as Element).getClientRects().length)
      for (const el of fresh) comeAt(el as Element, 160)
    })
    late.observe(bar, { childList: true, subtree: true })
  }

  // Her parts in by now: none come for SETTLED_MS (the programs' buttons,
  // the tray's icons, the monitor's readings), or SETTLED_MOST_MS gone.
  const SETTLED_MS = 250
  const SETTLED_MOST_MS = 1000
  function settled() {
    return new Promise<void>(resolve => {
      const bar = document.getElementById('bar-rest')
      const fresh = (records: MutationRecord[]) => records.some(r => [...r.addedNodes].some(n => n.nodeType === 1 && ((n as Element).matches(PARTS) || !!(n as Element).querySelector?.(PARTS))))
      const done = () => {
        clearTimeout(quietly)
        clearTimeout(most)
        watch?.disconnect()
        resolve()
      }
      let quietly = setTimeout(done, SETTLED_MS)
      const most = setTimeout(done, SETTLED_MOST_MS)
      const watch =
        bar &&
        new MutationObserver(records => {
          if (!fresh(records)) return
          clearTimeout(quietly)
          quietly = setTimeout(done, SETTLED_MS)
        })
      // (A watch only with the bar there.)
      watch?.observe(bar!, { childList: true, subtree: true })
    })
  }

  // A part coming as its dust lands (hidden till then).
  const comeAt = (el: Element, at: number) => el.animate([{ opacity: 0, transform: 'scale(0.6)' }, { opacity: 1, transform: 'none' }], { duration: 320, delay: Math.max(0, at - 160), easing: 'cubic-bezier(0.3, 1.4, 0.5, 1)', fill: 'backwards' })

  function coloursOf(el: Element) {
    const img = el.querySelector<HTMLImageElement>('img')
    if (img?.complete && img.naturalWidth) {
      try {
        const c = document.createElement('canvas')
        c.width = c.height = 16
        const x = c.getContext('2d', { willReadFrequently: true })!
        x.drawImage(img, 0, 0, 16, 16)
        const d = x.getImageData(0, 0, 16, 16).data
        const out: string[] = []
        for (let i = 0; i < d.length; i += 4) if (d[i + 3] > 160) out.push(`rgb(${d[i]},${d[i + 1]},${d[i + 2]})`)
        if (out.length) return out
      } catch {}
    }
    return [getComputedStyle(el).color || '#ffffff']
  }

  // --- The dust ------------------------------------------------------------------------
  // A grain: from where it was, up to a point a little above, then down to
  // where it goes, a flash there; or, with nowhere to go, up and away.
  let grains: Grain[] = []
  let raf = 0
  let last = 0
  let dpr = 1

  function start() {
    if (!dust) {
      dust = canvasLayer('inset:0;width:100%;height:100%;z-index:2147483001')
      dpr = devicePixelRatio || 1
      dust.width = Math.round(innerWidth * dpr)
      dust.height = Math.round(innerHeight * dpr)
    }
    if (!raf) raf = requestAnimationFrame(frame)
  }

  // (The dust is there while frames come: stop() takes the next one off as
  // it takes the dust away.)
  function frame(now: number) {
    const dt = Math.min(50, now - (last || now))
    last = now
    const c = dust!.getContext('2d')!
    c.setTransform(dpr, 0, 0, dpr, 0, 0)
    c.clearRect(0, 0, innerWidth, innerHeight)
    c.globalCompositeOperation = 'lighter'
    grains = grains.filter(g => g.life < g.max + g.delay)
    for (const g of grains) {
      g.life += dt
      const t = (g.life - g.delay) / g.max
      if (t < 0) {
        if (g.showWait) dotAt(c, g, g.sx, g.sy, 1, GRAIN)
        continue
      }
      let x
      let y
      let a = 1
      let r = GRAIN
      if (g.tx === undefined) {
        // Up and away, swaying, fading.
        const e = 1 - (1 - t) * (1 - t)
        x = g.sx + g.dx * e + Math.sin(t * 6 + g.ph) * 6
        y = g.sy - g.rise * e
        a = 1 - t
      } else if (t < 0.32) {
        const e = 1 - Math.pow(1 - t / 0.32, 2)
        x = g.sx + (g.lx - g.sx) * e
        y = g.sy + (g.ly - g.sy) * e
      } else {
        const k = Math.min(1, (t - 0.32) / 0.58)
        const e = k * k * (3 - 2 * k)
        x = g.lx + (g.tx - g.lx) * e
        y = g.ly + (g.ty - g.ly) * e
        if (t > 0.9) {
          a = 1 - (t - 0.9) / 0.1
          r = GRAIN * (1 + (2 * (t - 0.9)) / 0.1)
        }
      }
      dotAt(c, g, x, y, a, r)
    }
    if (grains.length) raf = requestAnimationFrame(frame)
    else stop()
  }

  function dotAt(c: CanvasRenderingContext2D, g: Grain, x: number, y: number, a: number, r: number) {
    c.globalAlpha = Math.max(0, Math.min(1, a))
    if (g.glint) {
      const grad = c.createRadialGradient(x, y, 0, x, y, r * 3)
      grad.addColorStop(0, g.c)
      grad.addColorStop(1, 'rgba(255,255,255,0)')
      c.fillStyle = grad
      c.beginPath()
      c.arc(x, y, r * 3, 0, Math.PI * 2)
      c.fill()
    } else {
      c.fillStyle = g.c
      c.fillRect(x - r / 2, y - r / 2, r, r)
    }
  }

  function stop() {
    cancelAnimationFrame(raf)
    raf = 0
    last = 0
    grains = []
    dust?.remove()
    dust = null
  }

  // Grains from these points to those, the later the farther right they
  // start (shown where they are while they wait: they are what was there,
  // unless they come from nowhere); when each part's last grain lands.
  function settle(from: Dot[], to: Dot[], showWait = true) {
    const lands = new Map<HTMLElement, number>()
    from.forEach((p, i) => {
      const t = to[(i * 7919) % to.length]
      const max = SETTLE_MS + Math.random() * 250
      const delay = (p.x / innerWidth) * SPREAD_MS * 0.7 + Math.random() * SPREAD_MS * 0.3
      const glint = i % GLINT_EVERY === 0
      grains.push({ sx: p.x, sy: p.y, lx: p.x + (Math.random() - 0.5) * 50, ly: p.y - 18 - Math.random() * 70, tx: t.x, ty: t.y, max, delay, life: 0, glint, c: glint ? GLINTS[i % GLINTS.length] : p.c, showWait })
      if (t.el) lands.set(t.el, Math.max(lands.get(t.el) || 0, delay + max * 0.86))
    })
    start()
    return lands
  }

  // Her parts come as their dust lands (hidden till then), the picture's
  // bare ground fading into hers meanwhile.
  function arrive(lands: Map<HTMLElement, number>) {
    for (const el of document.querySelectorAll(PARTS)) comeAt(el, lands.get(el) ?? SETTLE_MS)
    watchLate()
    picture?.animate([{ opacity: 1 }, { opacity: 0 }], { duration: 600, delay: 320, easing: 'ease', fill: 'forwards' })
    // Over by then whatever came of the frames (none drawn while the window
    // is hidden or covered: the dust would stay where it was), or, a late
    // part's dust on its way, once that has landed (2.5 s more at most).
    const most = Math.max(SETTLE_MS + 250, ...lands.values())
    const giveUp = performance.now() + most + 2500
    const finish = () => {
      const wait = Math.min(lateEnd, giveUp) - performance.now()
      if (wait > 0) return setTimeout(finish, wait + 260)
      late?.disconnect()
      late = null
      drop()
      stop()
      pet.taskbar.handoff('done')
    }
    setTimeout(finish, most + 260)
  }

  // --- The steps ------------------------------------------------------------------------
  // in: Windows' strip's picture to lay over it (none: nothing to cover).
  // go: Windows' is hidden under it now: the dust.
  // out: quitting: hers crumbles, Windows' to be shown under it (bare).
  pet.onHandoff(async ({ step: next, picture: url, size }: { step?: string; picture?: string; size?: number[] } = {}) => {
    if (next === 'in') {
      if (quiet()) unhide()
      else if (url) {
        try {
          await cover(url, size)
        } catch {
          drop()
        }
      }
      return pet.taskbar.handoff('covered')
    }
    if (next === 'go') {
      const mine = ++step
      if (quiet()) {
        unhide()
        drop()
        return pet.taskbar.handoff('done')
      }
      // Hers shows now, its parts each waiting for their dust (arrive).
      if (picture) {
        unhide()
        return arrive(settle(crumble(), partsPoints(MOST)))
      }
      // Nothing to crumble (no picture; a strip along another edge than
      // Windows' own, the top bar): nothing covered meanwhile, so the dust
      // waits for her parts to be in, and gathers from the screen's side of
      // the strip into all of them at once.
      await settled()
      if (mine !== step) return
      unhide()
      const to = partsPoints(MOST)
      const from = to.map(() => ({ ...awayFromStrip(), c: GLINTS[(Math.random() * GLINTS.length) | 0] }))
      return arrive(settle(from, to, false))
    }
    if (next === 'out') {
      step++
      late?.disconnect()
      late = null
      unhide()
      drop()
      if (quiet()) {
        pet.taskbar.handoff('bare')
        return pet.taskbar.handoff('done')
      }
      const row = document.getElementById('island-row')
      const from = partsPoints(MOST)
      for (const el of document.querySelectorAll(PARTS)) el.animate([{ opacity: 0 }], { duration: 1, fill: 'forwards' })
      from.forEach((p, i) => {
        const glint = i % GLINT_EVERY === 0
        grains.push({ sx: p.x, sy: p.y, dx: (Math.random() - 0.5) * 60, rise: 50 + Math.random() * 110, ph: Math.random() * 6, max: 700 + Math.random() * 300, delay: (p.x / innerWidth) * 120, life: 0, glint, c: glint ? GLINTS[i % GLINTS.length] : p.c, showWait: true })
      })
      start()
      // Hers bare: Windows' may show under it; hers fades off it.
      setTimeout(() => pet.taskbar.handoff('bare'), 160)
      row?.animate([{ opacity: 1 }, { opacity: 0 }], { duration: 480, delay: 200, easing: 'ease', fill: 'forwards' })
      setTimeout(() => {
        stop()
        pet.taskbar.handoff('done')
      }, 1000)
    }
  })
}
