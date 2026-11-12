// Widgets drawn by size (the widget design of 2026-10-16: the spec's §2,
// §4, §6, §8): what each shows on the taskbar (small: its glyph and value;
// medium: its value over its name), as a row in the open island (small: a
// line; medium: two, or a small curve), in its card there (its large size,
// unfolded in place), and as a line in a stack opened on the taskbar. One
// colour each, on its glyph, its progress and its glow; numbers in the text's
// own colour; ups and downs in theirs. Markup as text, for island.js to put
// in place; loaded before it, and required by the tests.
//
// ctx: { lang, light (a light ground), detailed (the specifics may show),
// width (a card's), hour (now, for the bars), now (ms) }.
import * as I18n from '../shared/i18n.ts'
import { render } from '../shared/i18n.ts'
import * as Widgets from './widgets.ts'

const t = (ctx, key, vars?) => I18n.t(ctx.lang, key, vars)
const esc = value => String(value ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c])

// --- Who it is: its kind, colour and glyph ----------------------------------------
const KINDS = ['weather', 'monitor', 'stock', 'mail', 'today', 'tokens', 'pomodoro', 'countdown', 'calendar', 'media']
function kindOf(w) {
  if (!w) return 'generic'
  if (w.id === 'monitor' || w.id === 'today' || w.id === 'tokens' || w.id === 'weather' || w.id === 'pomodoro' || w.id === 'countdown' || w.id === 'calendar' || w.id === 'media') return w.id
  if (w.plugin === 'stock' || /^stock-/.test(w.id)) return 'stock'
  if (w.from === 'mail') return 'mail'
  return 'generic'
}

// Each kind's colour, dark ground and light (the spec's table); today's
// tokens as today. Ups and downs apart (their chip, their line).
const COLORS = {
  weather: ['#8cc8ff', '#1a78c2'],
  monitor: ['#34d27b', '#0f7b3f'],
  stock: ['#ffd36b', '#a46a00'],
  mail: ['#6fa6ff', '#0067c0'],
  today: ['#f08a62', '#c45a2c'],
  tokens: ['#f08a62', '#c45a2c'],
  pomodoro: ['#ff7a6b', '#c4362b'],
  countdown: ['#b99bff', '#6b45c7'],
  calendar: ['#9d8cff', '#5a4bc9'],
  media: ['#ff7ab6', '#c2317a'],
}
const UP = ['#34d27b', '#0f7b3f']
const DOWN = ['#ff6b6b', '#c42b3c']
const GREY = ['#a1a1a6', '#5d5d5d']
const pick = (pair, ctx) => pair[ctx.light ? 1 : 0]
// A script's own colour (deepened on a light ground, widgets.js), else grey.
function colorOf(w, ctx) {
  const kind = kindOf(w)
  if (COLORS[kind]) return pick(COLORS[kind], ctx)
  if (w?.color) return ctx.light ? Widgets.deepen(w.color) : w.color
  return pick(GREY, ctx)
}

// Which colour its rise or fall takes: the colour its script gave it, the
// one place that decides, for every place it is drawn (the taskbar's faces,
// carousels and stacks, the capsule's rows and cards; the desktop's to
// come), as the compact island's value always took it. The stock plugin
// (plugins/stock.rs) sends red up or green up as its colours setting says (plugins.rs:
// until chosen, red up in Chinese as Chinese markets show them, green up
// in any other language). Drawn in the design's two: its green UP's, its
// red DOWN's.
function trendOf(w) {
  const c = String(w?.color || '').toLowerCase()
  if (c === '#ff5c6c' || c === '#ff6b6b') return 'red'
  if (c === '#34d27b') return 'green'
  return ''
}
const trendColor = (trend, ctx) => (trend === 'red' ? pick(DOWN, ctx) : trend === 'green' ? pick(UP, ctx) : '')

// Windows' own glyphs (Segoe Fluent Icons): the weather's by the sky, the
// rest by kind; a script's own drawn icon (widgets.js), her face for today.
const SKY = { sun: 'E706', moon: 'E708', partly: 'E753', cloud: 'E753', fog: 'E753', rain: 'EB42', snow: 'EA38', storm: 'E945' }
const GLYPH = { monitor: 'E950', stock: 'E9D9', mail: 'E715', pomodoro: 'E916', countdown: 'E787', tokens: 'E943', calendar: 'E8BF', media: 'EC4F' }
const rowsOf = w => (Array.isArray(Widgets.detailOf(w).rows) ? Widgets.detailOf(w).rows : [])
const skyNow = w => rowsOf(w).find(r => r.kind === 'hour')?.icon
function glyphOf(w) {
  const kind = kindOf(w)
  if (kind === 'weather') return SKY[skyNow(w)] || 'E753'
  return GLYPH[kind] || ''
}
// As markup: a glyph, her face (island.js puts her in .wher), or the
// script's icon; `cls` for its place.
function glyphHTML(w, cls = '') {
  if (kindOf(w) === 'today') return `<span class="wher ${cls}" aria-hidden="true"></span>`
  const g = glyphOf(w)
  if (g) return `<span class="wgl ${cls}" aria-hidden="true">&#x${g};</span>`
  // A plugin's own picture (widgets.js maskHTML), drawn as her icons are.
  if (Widgets.hasPicture(w)) return `<span class="wsvg ${cls}">${Widgets.maskHTML(w.iconUrl)}</span>`
  // No icon of its own: its name's first character on its colour (widgets.js markHTML).
  if (!Widgets.hasIcon(w?.icon)) return Widgets.markHTML(w?.markName ?? (typeof w?.label === 'string' ? w.label : w?.id), w?.color, cls)
  return `<span class="wsvg ${cls}">${Widgets.icon(w.icon)}</span>`
}
// A member with nothing to say: its glyph as it was put there (its mark
// by the name it was put there with).
function memberGlyph(m) {
  const like = m.type === 'plugin' ? { id: m.of, plugin: m.of, icon: m.icon, markName: m.name || m.of } : { id: m.of, icon: m.icon, from: /^inbox-/.test(m.of) ? 'mail' : undefined, markName: m.name || m.of }
  return glyphHTML(like)
}

// --- Words ---------------------------------------------------------------------------
// Its name as a stack's line and its card say it: the kind's (天气, 系统
// 监控, Claude 今天, 邮件, 番茄钟, 倒计时), else its label (a stock's
// name).
const TITLES = { weather: 'card.weather', monitor: 'card.monitor', today: 'card.today', mail: 'card.mail', pomodoro: 'card.pomodoro', countdown: 'card.countdown', calendar: 'card.calendar', media: 'card.media' }
function titleOf(w, ctx) {
  const kind = kindOf(w)
  if (TITLES[kind]) return t(ctx, TITLES[kind])
  return Widgets.words(ctx.lang, w, ctx.detailed).label || Widgets.words(ctx.lang, w, true).value
}
// The monitor's readings by their glyph (widgets.rs: cpu, memory, down, up).
const partText = (w, icon) => (Widgets.partsOf(w) || []).find(p => p.icon === icon)?.text || ''
const varsOf = w => (w?.value && typeof w.value === 'object' ? w.value.vars || {} : {})
// A private widget's words: as they are while the specifics may show;
// else blurred until the pointer is on them (style.css .wpriv.hid).
const priv = (w, text, ctx) => (w?.private && !ctx.detailed ? `<span class="wpriv hid">${esc(text)}</span>` : esc(text))

// Its words for each place, by kind: value (small), top and line (medium:
// its value over its name), big and unit (a stack's line, 18px), say (a
// stack's line's sentence), second (an island row's second line). All
// plain text; a private part says so (lineIsPrivate).
function briefOf(w, ctx) {
  const said = Widgets.words(ctx.lang, w, true)
  const extra = Widgets.detailOf(w).extra || ''
  const kind = kindOf(w)
  const out = { value: said.value, top: said.value, line: said.label, big: said.value, unit: '', say: [said.label, extra].filter(Boolean).join(' · '), second: extra || said.label, private: false }
  if (kind === 'weather') {
    const [city, sky] = String(said.label).split(' · ')
    Object.assign(out, { top: [said.value, sky].filter(Boolean).join(' '), line: city || '', say: [sky, city, extra].filter(Boolean).join(' · '), second: [sky, extra].filter(Boolean).join(' · ') })
  } else if (kind === 'monitor') {
    const cpu = partText(w, 'cpu')
    const mem = partText(w, 'memory')
    const down = partText(w, 'down')
    const memWords = mem ? `${t(ctx, 'card.memory')} ${mem}` : ''
    Object.assign(out, {
      value: cpu || said.value,
      top: cpu ? `CPU ${cpu}` : said.value,
      line: [memWords, down ? `↓ ${down}` : ''].filter(Boolean).join(' · '),
      big: cpu || '—',
      unit: 'CPU',
      say: [cpu ? `CPU ${cpu}` : '', memWords].filter(Boolean).join(' · '),
      second: [memWords, down ? `↓ ${down}` : ''].filter(Boolean).join(' · '),
    })
  } else if (kind === 'today') {
    const v = varsOf(w)
    const time = render(ctx.lang, v.time)
    const turns = v.turns ?? ''
    const approved = v.approvals ?? ''
    Object.assign(out, {
      value: t(ctx, 'card.turns', { n: turns }),
      top: t(ctx, 'card.turns', { n: turns }),
      line: [time, t(ctx, 'card.approved', { n: approved })].filter(Boolean).join(' · '),
      big: String(turns),
      unit: t(ctx, 'card.turnsUnit'),
      say: [time, t(ctx, 'card.approved', { n: approved })].filter(Boolean).join(' · '),
      second: [time, t(ctx, 'card.approved', { n: approved })].filter(Boolean).join(' · '),
    })
  } else if (kind === 'tokens') {
    const v = varsOf(w)
    Object.assign(out, { value: v.total || said.value, top: `${v.total || said.value} token`, line: v.output ? t(ctx, 'card.output', { n: v.output }) : '', big: v.total || said.value, unit: 'token', say: v.output ? t(ctx, 'card.output', { n: v.output }) : '', second: v.output ? t(ctx, 'card.output', { n: v.output }) : '' })
  } else if (kind === 'stock') {
    const row = rowsOf(w)[0] || {}
    const name = said.label || row.code || ''
    const price = row.price || String(said.value).split(' ')[0]
    const change = row.change || String(said.value).split(' ').slice(1).join(' ')
    Object.assign(out, { value: said.value, top: [name, price].filter(Boolean).join(' '), line: change, big: price, unit: change, say: [row.code, row.value ? t(ctx, 'card.volume', { n: row.value }) : ''].filter(Boolean).join(' · '), second: change, trend: trendOf(w) })
  } else if (kind === 'mail') {
    const n = varsOf(w).n
    Object.assign(out, { big: n ?? said.value, unit: n != null ? t(ctx, 'card.unread') : '', say: render(ctx.lang, w.label), line: render(ctx.lang, w.label), second: render(ctx.lang, w.label), private: !!w.private })
  } else if (kind === 'countdown') {
    const days = rowsOf(w).find(r => r.kind === 'days')
    if (days) {
      const short = days.unit ? `${days.value} ${days.unit}` : days.value
      Object.assign(out, { value: short, top: short, big: days.value, unit: days.unit || '', say: [said.label, extra].filter(Boolean).join(' · '), second: extra || said.label })
    }
  } else if (kind === 'calendar') {
    // The date (11月7日; Nov 7) with the next event under it, or the
    // lunar date (the week) where none is coming; the day of the month
    // as its number, the month what it counts.
    const date = rowsOf(w).find(r => r.kind === 'date') || {}
    const next = rowsOf(w).find(r => r.kind === 'event')
    const day = date.value || said.value
    const words = date.name ? (ctx.lang === 'zh' ? `${date.name}${day}日` : `${date.name} ${day}`) : said.label
    const coming = next ? [next.when, next.subject].filter(Boolean).join(' ') : ''
    Object.assign(out, { value: words, top: words, line: coming || extra, big: String(day), unit: date.name || '', say: [extra, coming].filter(Boolean).join(' · '), second: coming || extra })
  } else if (kind === 'media') {
    // The song (short, as the taskbar has room) over its artist; a stack's
    // line says both, no number to count.
    const track = rowsOf(w).find(r => r.kind === 'track') || {}
    const title = shorten(track.subject || said.label, 24)
    const artist = track.text || (said.value === '—' ? '' : said.value)
    Object.assign(out, { value: title, top: title, line: artist, big: '', unit: '', say: [artist, extra].filter(Boolean).join(' · '), second: [artist, extra].filter(Boolean).join(' · ') })
  }
  return out
}
// Words cut to so many characters, "…" the last when cut (as widgets.rs cuts).
function shorten(s, max) {
  const chars = Array.from(String(s ?? ''))
  return chars.length > max ? `${chars.slice(0, max - 1).join('')}…` : chars.join('')
}

// --- Drawing: a curve, a ring, bars, each to its own scale ----------------------------
let uid = 0
const num = v => (Math.round(v * 10) / 10).toString()
// A line through the points, w by h: its gradient fill under it and a dot
// at its end (the spec's 1.6px line); lo and hi its scale (else its own
// least and most).
function spark(points, w, h, color, { fill = true, dot = true, pad = 3, lo, hi } = {}) {
  const data = (points || []).filter(Number.isFinite)
  if (data.length < 2) return ''
  const min = Number.isFinite(lo) ? Math.min(lo, ...data) : Math.min(...data)
  const max = Number.isFinite(hi) ? Math.max(hi, ...data) : Math.max(...data)
  const x = i => pad + (i * (w - pad * 2)) / (data.length - 1)
  const y = v => h - pad - ((v - min) / (max - min || 1)) * (h - pad * 2)
  const pts = data.map((v, i) => `${num(x(i))},${num(y(v))}`)
  const id = `wg${++uid}`
  const last = data.length - 1
  const area = fill ? `<path d="M${pts[0]} L${pts.join(' L')} L${num(x(last))},${h} L${num(x(0))},${h} Z" fill="url(#${id})"/>` : ''
  const end = dot ? `<circle cx="${num(x(last))}" cy="${num(y(data[last]))}" r="2.6" fill="${color}"/>` : ''
  const defs = fill ? `<defs><linearGradient id="${id}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${color}" stop-opacity=".28"/><stop offset="1" stop-color="${color}" stop-opacity="0"/></linearGradient></defs>` : ''
  return `<svg class="wspark" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" aria-hidden="true">${defs}${area}<polyline points="${pts.join(' ')}" fill="none" stroke="${color}" stroke-width="1.6" stroke-linejoin="round" stroke-linecap="round"/>${end}</svg>`
}

// A ring filled to a share (0 to 1) from the top, clockwise.
function ring(share, size, stroke, color, track) {
  const r = (size - stroke) / 2
  const c = 2 * Math.PI * r
  const s = Math.max(0, Math.min(1, share || 0))
  return `<svg class="wring" width="${size}" height="${size}" viewBox="0 0 ${size} ${size}" aria-hidden="true"><circle cx="${size / 2}" cy="${size / 2}" r="${r}" fill="none" stroke="${track}" stroke-width="${stroke}"/><circle cx="${size / 2}" cy="${size / 2}" r="${r}" fill="none" stroke="${color}" stroke-width="${stroke}" stroke-linecap="round" stroke-dasharray="${num(c * s)} ${num(c)}" transform="rotate(-90 ${size / 2} ${size / 2})"/></svg>`
}

// Bars, w by h, 3px apart: one lit (the hour it is), the rest the track.
function bars(points, w, h, color, track, lit = -1) {
  const data = (points || []).map(v => (Number.isFinite(v) ? v : 0))
  if (!data.length) return ''
  const gap = data.length > 16 ? 2 : 3
  const bw = (w - gap * (data.length - 1)) / data.length
  const max = Math.max(1, ...data)
  const rects = data.map((v, i) => {
    const bh = Math.max(2, (v / max) * h)
    return `<rect x="${num(i * (bw + gap))}" y="${num(h - bh)}" width="${num(bw)}" height="${num(bh)}" rx="1.5" fill="${i === lit ? color : track}"/>`
  })
  return `<svg class="wbars" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" aria-hidden="true">${rects.join('')}</svg>`
}

const track = ctx => (ctx.light ? 'rgba(0,0,0,.09)' : 'rgba(255,255,255,.12)')

// Today's hours as slots (the design session, 2026-10-16): from the hour
// of the first turn to the hour it is, twelve at least (earlier ones
// before it, or, early in the day, the hours to come); each bar as wide as
// its slot allows but 18 px at most, at the slot's left; an hour without
// a turn a 2 px line in the track's colour, the hour it is in its colour.
// Gives the slots' first hour and last, for the labels under them.
function hourSlots(points, lit) {
  const hours = Array.isArray(points) ? points : []
  const first = hours.findIndex(n => n > 0)
  const start = first < 0 ? lit : Math.min(first, lit)
  const from = Math.max(0, Math.min(start, lit - 11))
  const n = Math.max(12, lit - from + 1)
  return { from, to: from + n - 1, data: Array.from({ length: n }, (_, i) => (from + i <= lit ? Number(hours[from + i]) || 0 : 0)) }
}
function hourBars(points, lit, w, h, color, trackColor) {
  const { from, to, data } = hourSlots(points, lit)
  const slot = w / data.length
  const bw = Math.min(18, Math.max(2, slot - 3))
  const max = Math.max(1, ...data)
  const rects = data.map((v, i) => {
    const x = num(i * slot)
    if (!v) return `<rect x="${x}" y="${num(h - 2)}" width="${num(bw)}" height="2" rx="1" fill="${trackColor}"/>`
    const bh = Math.max(3, (v / max) * h)
    return `<rect x="${x}" y="${num(h - bh)}" width="${num(bw)}" height="${num(bh)}" rx="1.5" fill="${from + i === lit ? color : trackColor}"/>`
  })
  return { svg: `<svg class="wbars" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" aria-hidden="true">${rects.join('')}</svg>`, from, to }
}
const seriesOf = w => Widgets.detailOf(w).series || null

// --- On the taskbar: small and medium (the spec's §5) -------------------------------
// Small: its glyph and value; medium: its value over its name, as the
// clock has the date under the time. A stock's change in its own colour;
// a private line "···" while the specifics may not show. The glyph white
// (or black) as the tray's are: the text's colour (style.css .tw .wgl).
function faceHTML(w, size, ctx) {
  const b = briefOf(w, ctx)
  const glyph = glyphHTML(w, 'twg')
  if (size === 'S') return `${glyph}<b class="twv">${esc(b.value || '—')}</b>`
  const lineColor = b.trend ? ` style="color:${trendColor(b.trend, ctx)}"` : ''
  const line = b.private && !ctx.detailed ? '···' : esc(b.line)
  return `${glyph}<span class="twt"><b class="twv">${esc(b.top || '—')}</b><i${lineColor}>${line}</i></span>`
}

// One with nothing to say: its glyph as it was put there, greyed, and its
// name; loading (just put there, its plugin starting) a shimmer where its
// value goes, for a few seconds (style.css .wshim: never endless); one
// that stopped, its glyph amber and why.
function emptyFaceHTML(member, name, size, state, ctx) {
  const glyph = state === 'error' ? `<span class="wgl twg werr" aria-hidden="true">&#xE7BA;</span>` : memberGlyph(member)
  const what = state === 'loading' ? '<span class="wshim"></span>' : esc(t(ctx, state === 'needs' ? 'taskbar.w.needs' : state === 'error' ? 'card.stopped' : 'taskbar.w.none'))
  if (size === 'S') return `${glyph}<b class="twv">${state === 'loading' ? what : '—'}</b>`
  return `${glyph}<span class="twt"><b class="twv">${what}</b><i>${esc(name)}</i></span>`
}

// --- In the open island: rows (the spec's §6) -----------------------------------------
// Small: its glyph (its colour), name, value (its colour, semibold);
// medium: name over its second line (11px), and a small curve (56 by 16)
// when it has a series. One with a large size has a › (its card); the
// card open, the › turned.
// A row's words: its name (the kind's for the weather, the monitor,
// today and the pomodoro, as the sample has them; else its label), its
// value (short: the monitor's three readings, today's turns and time), and
// medium its second line (the weather's sky, range and place; today's
// approvals and tokens, the tokens widget's: ctx.all) with its value then.
function rowWords(w, size, ctx) {
  const b = briefOf(w, ctx)
  const said = Widgets.words(ctx.lang, w, true)
  const kind = kindOf(w)
  const out = { name: ['weather', 'monitor', 'today', 'pomodoro'].includes(kind) ? titleOf(w, ctx) : render(ctx.lang, w.label), value: said.value, second: b.second }
  if (kind === 'weather') {
    const [city, sky] = String(said.label).split(' · ')
    out.second = [sky, Widgets.detailOf(w).extra, city].filter(Boolean).join(' · ')
  } else if (kind === 'monitor') {
    const [cpu, mem, down] = ['cpu', 'memory', 'down'].map(i => partText(w, i))
    out.value = size === 'M' ? cpu || '—' : [cpu, mem, down ? `↓${down}` : ''].filter(Boolean).join(' · ')
    out.second = b.second
  } else if (kind === 'today') {
    const v = varsOf(w)
    const tokens = varsOf((ctx.all || []).find(x => x.id === 'tokens')).total
    out.value = [t(ctx, 'card.turns', { n: v.turns ?? '' }), render(ctx.lang, v.time)].filter(Boolean).join(' · ')
    out.second = [t(ctx, 'card.approved', { n: v.approvals ?? '' }), tokens ? `${tokens} token` : ''].filter(Boolean).join(' · ')
  } else if (kind === 'tokens') {
    out.value = b.value
  } else if (kind === 'stock') {
    const row = rowsOf(w)[0] || {}
    if (size === 'M') out.value = b.big
    out.second = [row.code !== out.name ? row.code : '', b.unit].filter(Boolean).join(' · ')
  } else if (kind === 'pomodoro') {
    out.second = [said.label, Widgets.detailOf(w).extra].filter(Boolean).join(' · ')
  } else if (kind === 'calendar') {
    // The date as its name; the next event its value (medium, nothing
    // where none is coming: the second line has the lunar date), small
    // the lunar date (the week) where none is coming.
    const next = rowsOf(w).find(r => r.kind === 'event')
    const coming = next ? [next.when, next.subject].filter(Boolean).join(' ') : ''
    out.value = coming || (size === 'M' ? '' : Widgets.detailOf(w).extra || '')
    out.second = Widgets.detailOf(w).extra || ''
  } else if (kind === 'media') {
    // The song as its name, its artist the value, how far along under
    // (medium: a small ring of it, from its series).
    out.name = b.top
    out.value = b.line
    out.second = Widgets.detailOf(w).extra || ''
  }
  return out
}

function rowHTML(w, size, ctx, { open = false, card = false } = {}) {
  const b = briefOf(w, ctx)
  const color = colorOf(w, ctx)
  const rw = rowWords(w, size, ctx)
  const kind = kindOf(w)
  const name = kind === 'mail' ? priv(w, rw.name, ctx) : esc(rw.name)
  // Ups and downs in theirs; the rest in its colour.
  const valueColor = b.trend ? trendColor(b.trend, ctx) : color
  const series = seriesOf(w)
  let curve = ''
  if (size === 'M' && series && kind !== 'today') {
    // A pomodoro's: a small ring of the time left.
    if (series.points.length === 1 && Number.isFinite(series.hi)) curve = ring(series.points[0] / (series.hi || 1), 16, 2.5, color, track(ctx))
    else curve = spark(series.points.slice(-40), 56, 16, valueColor, { fill: false, dot: false, pad: 1.5, lo: series.lo, hi: series.hi })
  }
  const second = b.private && !ctx.detailed ? priv(w, rw.second, ctx) : esc(rw.second)
  const words = size === 'M' && rw.second && kind !== 'mail' ? `<span class="wtwo"><span class="wl">${name}</span><span class="wsec">${second}</span></span>` : `<span class="wl">${name}</span>`
  const chev = card ? '<span class="wchev" aria-hidden="true">&#xE76C;</span>' : '<span class="wchev none"></span>'
  return `${glyphHTML(w, 'wrg')}${words}${curve ? `<span class="wcurve">${curve}</span>` : ''}<span class="wv" style="color:${valueColor}">${esc(rw.value)}</span>${chev}`
}

// --- A stack opened on the taskbar: a line each (the spec's §5) ---------------------
// Its glyph in a block of its colour (15%), name, a sentence; its number
// (18px) and what it counts on the right.
function stackRowHTML(w, name, ctx) {
  const b = briefOf(w, ctx)
  const color = colorOf(w, ctx)
  const say = b.private && !ctx.detailed ? priv(w, b.say, ctx) : esc(b.say)
  const unitColor = b.trend ? ` style="color:${trendColor(b.trend, ctx)}"` : ''
  return `<span class="sgl" style="--c:${color}">${glyphHTML(w)}</span><b class="sname">${esc(name)}</b><span class="sval">${esc(b.big)}<small${unitColor}>${esc(b.unit)}</small></span><span class="ssay">${say}</span>`
}

// --- Cards: the large size in the open island (the spec's §4, §6) -------------------
// As wide as the island's lines (ctx.width); simpler than the desktop's
// large size. What is not read greys with "no data" (the spec's §8).
const nodata = ctx => `<span class="wnone">${esc(t(ctx, 'taskbar.w.none'))}</span>`
const meter = (pct, color) => `<span class="wmeter"><i style="width:${Math.max(0, Math.min(100, pct || 0))}%;background:${color}"></i></span>`

function weatherCard(w, ctx) {
  const said = Widgets.words(ctx.lang, w, true)
  const d = Widgets.detailOf(w)
  const [city, sky] = String(said.label).split(' · ')
  const color = colorOf(w, ctx)
  const hours = rowsOf(w).filter(r => r.kind === 'hour').slice(0, 7)
  const days = rowsOf(w).filter(r => r.kind === 'day')
  const top = `<div class="crow"><span class="cbig">${esc(said.value)}</span><span class="cwords"><b>${esc([sky, city].filter(Boolean).join(' · '))}</b><span>${esc(d.extra || '')}</span></span></div>`
  const curve = d.series ? `<div class="ccurve">${spark(d.series.points, ctx.width, 40, color, { pad: 6 })}</div>` : ''
  const strip = hours.length
    ? `<div class="chours" style="grid-template-columns:repeat(${hours.length},1fr)">${hours.map(h => `<span>${esc(h.when)}<i style="color:${color}">&#x${SKY[h.icon] || 'E753'};</i><b>${esc(h.value)}</b></span>`).join('')}</div>`
    : ''
  const ahead = days.length ? `<div class="cfoot">${days.map(x => `<span>${esc([x.when, x.value, x.name].filter(Boolean).join(' '))}</span>`).join('')}</div>` : ''
  return top + curve + strip + ahead
}

function monitorCard(w, ctx) {
  const d = Widgets.detailOf(w)
  const color = colorOf(w, ctx)
  const row = kind => rowsOf(w).find(r => r.kind === kind)
  const cpu = row('cpu')
  const pct = r => parseFloat(r?.value) || 0
  const about = cpu ? [cpu.name && cpu.unit ? t(ctx, 'card.cores', { cores: cpu.name, threads: cpu.unit }) : '', cpu.text || ''].filter(Boolean).join(' · ') : ''
  const top = `<div class="crow"><span class="cbig">${cpu ? esc(String(cpu.value).replace('%', '')) : '—'}<small>%</small></span><span class="cwords"><b>CPU</b><span>${esc(about)}</span></span></div>`
  const curve = d.series ? `<div class="ccurve">${spark(d.series.points, ctx.width, 40, color, { pad: 4, lo: 0, hi: 100 })}</div>` : ''
  const line = (kind, name) => {
    const r = row(kind)
    if (!r) return `<div class="cline off"><span class="cn">${esc(name)}</span>${nodata(ctx)}</div>`
    const sub = [r.name, r.text].filter(Boolean).join(' · ')
    return `<div class="cline"><span class="cn">${esc(name)}</span><span class="cd">${esc(sub)}</span><span class="cv">${esc(r.value)}</span>${meter(pct(r), color)}</div>`
  }
  const lines = `<div class="clines">${line('memory', t(ctx, 'card.memory'))}${line('gpu', 'GPU')}${line('disk', t(ctx, 'card.disk'))}</div>`
  const net = row('net')
  const up = row('uptime')
  const foot = `<div class="cfoot"><span>${net ? esc(`${t(ctx, 'card.net')} ↓ ${net.value} ↑ ${net.text || ''}`) : `${esc(t(ctx, 'card.net'))} ${nodata(ctx)}`}</span><span>${up ? esc(t(ctx, 'card.up', { time: upFor(Number(up.value), ctx) })) : ''}</span></div>`
  return top + curve + lines + foot
}

// A stock's card: its price and change, the day's curve, its high and
// low; the others of its plugin, a line each. Only the day's prices come
// (plugins/stock.rs): the week, month and year greyed.
function stockCard(w, ctx, all = []) {
  const b = briefOf(w, ctx)
  const d = Widgets.detailOf(w)
  const row = rowsOf(w)[0] || {}
  const trend = trendColor(b.trend, ctx) || colorOf(w, ctx)
  const name = [row.code, Widgets.words(ctx.lang, w, true).label].filter((x, i, a) => x && a.indexOf(x) === i).join(' · ')
  const chip = b.unit ? `<span class="cchip" style="--c:${trend}">${esc(b.unit)}</span>` : ''
  const top = `<div class="crow"><span class="cwords"><b>${esc(name)}</b><span class="cbig sm">${esc(b.big)}</span></span><span class="cright">${chip}${row.value ? `<span>${esc(t(ctx, 'card.volume', { n: row.value }))}</span>` : ''}</span></div>`
  const curve = d.series ? `<div class="ccurve">${spark(d.series.points, ctx.width, 56, trend, { pad: 4 })}</div>` : `<div class="ccurve">${nodata(ctx)}</div>`
  const range = d.series && Number.isFinite(d.series.hi) && Number.isFinite(d.series.lo) ? t(ctx, 'card.highLow', { hi: d.series.hi, lo: d.series.lo }) : ''
  const tabs = `<div class="crow"><span class="ctabs"><span class="on">${esc(t(ctx, 'card.day'))}</span><span class="off" title="${esc(t(ctx, 'card.onlyDay'))}">${esc(t(ctx, 'card.week'))}</span><span class="off" title="${esc(t(ctx, 'card.onlyDay'))}">${esc(t(ctx, 'card.month'))}</span><span class="off" title="${esc(t(ctx, 'card.onlyDay'))}">${esc(t(ctx, 'card.year'))}</span></span><span class="cter">${esc(range)}</span></div>`
  const others = all.filter(x => x !== w && x.plugin && x.plugin === w.plugin).slice(0, 4)
  const lines = others.length
    ? `<div class="clines">${others
        .map(x => {
          const o = briefOf(x, ctx)
          return `<div class="cline st"><span class="cn">${esc(Widgets.words(ctx.lang, x, true).label)}</span><span class="cv">${esc(o.big)}</span><span class="cchip" style="--c:${trendColor(o.trend, ctx) || colorOf(x, ctx)}">${esc(o.unit)}</span></div>`
        })
        .join('')}</div>`
    : ''
  return top + curve + tabs + lines
}

// Today: its turns, the time worked, approvals and tokens (the tokens
// widget's); the turns hour by hour, the hour it is lit. Tokens hour by
// hour and the busiest session are not counted yet: greyed.
function todayCard(w, ctx, all = []) {
  const v = varsOf(w)
  const d = Widgets.detailOf(w)
  const color = colorOf(w, ctx)
  const tokens = all.find(x => x.id === 'tokens')
  const tv = varsOf(tokens)
  const cell = (n, label) => `<span class="cnum"><b>${esc(n)}</b><span>${esc(label)}</span></span>`
  const top = `<div class="crow nums">${cell(v.turns ?? '—', t(ctx, 'card.turnsUnit'))}${cell(render(ctx.lang, v.time) || '—', t(ctx, 'card.worked'))}${cell(v.approvals ?? '—', t(ctx, 'card.approvals'))}${cell(tv.total || '—', 'token')}</div>`
  const lit = Number.isFinite(ctx.hour) ? ctx.hour : new Date().getHours()
  const b = hourBars(d.series?.points, lit, ctx.width, 40, color, track(ctx))
  const chart = d.series ? `<div class="ccap">${esc(t(ctx, 'card.byHour'))}</div><div class="ccurve">${b.svg}</div><div class="cfoot"><span>${esc(t(ctx, 'card.hour', { h: b.from }))}</span><span>${esc(t(ctx, 'card.hour', { h: b.to }))}</span></div>` : ''
  const grey = `<div class="cline off"><span class="cn">${esc(t(ctx, 'card.tokenCurve'))}</span>${nodata(ctx)}</div><div class="cline off"><span class="cn">${esc(t(ctx, 'card.busiest'))}</span>${nodata(ctx)}</div>`
  return top + chart + `<div class="clines">${grey}</div>`
}

// A mailbox: its newest unread, each who (bold), when, and what about (a
// private line: blurred while the specifics may not show).
function mailCard(w, ctx) {
  const rows = rowsOf(w).slice(0, 6)
  if (!rows.length) return `<div class="crow">${nodata(ctx)}</div>`
  return `<div class="cmails">${rows
    .map(r => `<div class="cmail"><b>${priv(w, r.from || '', ctx)}</b><em>${esc(whenOf(r.at, ctx))}</em><span>${priv(w, r.subject || '', ctx)}</span></div>`)
    .join('')}</div>`
}

// Anyone else's with a large size: its words, its curve, its list.
function genericCard(w, ctx) {
  const said = Widgets.words(ctx.lang, w, true)
  const d = Widgets.detailOf(w)
  const color = colorOf(w, ctx)
  const top = `<div class="crow"><span class="cbig sm">${esc(said.value)}</span><span class="cwords"><b>${priv(w, render(ctx.lang, w.label), ctx)}</b><span>${esc(d.extra || '')}</span></span></div>`
  const curve = d.series ? `<div class="ccurve">${spark(d.series.points, ctx.width, 40, color, { lo: d.series.lo, hi: d.series.hi })}</div>` : ''
  const rows = rowsOf(w)
    .slice(0, 6)
    .map(r => `<div class="cline st"><span class="cn">${priv(w, r.name || r.from || r.code || r.when || '', ctx)}</span><span class="cd">${priv(w, r.subject || r.text || '', ctx)}</span><span class="cv">${esc(r.value || r.price || '')}</span></div>`)
    .join('')
  return top + curve + (rows ? `<div class="clines">${rows}</div>` : '')
}

// --- Buttons of a widget's own (widgets.rs actions) ---------------------------------------
// What is playing has previous, play or pause, next: round buttons with
// Windows' glyphs, each pressed back to its plugin (island.js, desk.js:
// data-wact, data-widget; plugins.rs act); play or pause larger, in the
// widget's colour; one the player has not, greyed. Any widget with
// actions gets them, drawn the same wherever its card is.
const ACTION_GLYPH = { play: 'E768', pause: 'E769', prev: 'E892', next: 'E893', stop: 'E71A' }
const actionsOf = w => (Array.isArray(Widgets.detailOf(w).actions) ? Widgets.detailOf(w).actions.filter(a => a && typeof a.id === 'string' && a.icon in ACTION_GLYPH) : [])
function actionsHTML(w, ctx, cls = '') {
  const acts = actionsOf(w)
  if (!acts.length) return ''
  const button = a =>
    `<button type="button" class="wact${a.off ? ' off' : ''}${a.icon === 'play' || a.icon === 'pause' ? ' main' : ''}" data-wact="${esc(a.id)}" data-widget="${esc(w.id)}" title="${esc(t(ctx, `act.${a.icon}`))}"${a.off ? ' aria-disabled="true"' : ''}>&#x${ACTION_GLYPH[a.icon]};</button>`
  return `<span class="wacts${cls ? ` ${cls}` : ''}" style="--c:${colorOf(w, ctx)}">${acts.map(button).join('')}</span>`
}

// --- The month's grid (the calendar's cards) -----------------------------------------------
// The month it is (ctx.now): a row of the weekdays' letters (Monday first
// in Chinese, Sunday first in English), then its days, today lit in the
// widget's colour, a day with something on (the event rows' at) marked
// with a dot; the other months' days left blank.
function monthGrid(w, ctx, color) {
  const now = new Date(ctx.now || Date.now())
  const [y, m, today] = [now.getFullYear(), now.getMonth(), now.getDate()]
  const mondayFirst = ctx.lang === 'zh'
  const heads = mondayFirst ? ['一', '二', '三', '四', '五', '六', '日'] : ['S', 'M', 'T', 'W', 'T', 'F', 'S']
  const first = new Date(y, m, 1).getDay()
  const lead = mondayFirst ? (first + 6) % 7 : first
  const days = new Date(y, m + 1, 0).getDate()
  const marked = new Set(
    rowsOf(w)
      .filter(r => r.kind === 'event' && Number.isFinite(r.at))
      .map(r => new Date(r.at))
      .filter(d => d.getFullYear() === y && d.getMonth() === m)
      .map(d => d.getDate()),
  )
  const cells = [
    ...heads.map(h => `<span class="cwd">${h}</span>`),
    ...Array.from({ length: lead }, () => '<span></span>'),
    ...Array.from({ length: days }, (_, i) => {
      const d = i + 1
      const cls = ['cday', d === today ? 'on' : '', marked.has(d) ? 'ev' : ''].filter(Boolean).join(' ')
      return `<span class="${cls}">${d}</span>`
    }),
  ]
  return `<div class="cgrid" style="--c:${color}">${cells.join('')}</div>`
}

// The calendar: the day of the month large, the month and weekday, the
// lunar date (or the week) and the day's marks; the month's grid; what is
// coming, a line each (its day, what, when).
function calendarCard(w, ctx) {
  const d = Widgets.detailOf(w)
  const date = rowsOf(w).find(r => r.kind === 'date') || {}
  const events = rowsOf(w).filter(r => r.kind === 'event').slice(0, 4)
  const color = colorOf(w, ctx)
  const top = `<div class="crow"><span class="cbig">${esc(date.value || new Date(ctx.now || Date.now()).getDate())}</span><span class="cwords"><b>${esc([date.name, date.unit].filter(Boolean).join(' · '))}</b><span>${esc(date.text || d.extra || '')}</span></span></div>`
  const when = e => (e.unit ? `${e.when}–${e.unit}` : e.when || '')
  const lines = events.length
    ? `<div class="ccap">${esc(t(ctx, 'cal.coming'))}</div><div class="clines">${events.map(e => `<div class="cline st"><span class="cn">${esc(e.name || '')}</span><span class="cd">${esc([e.subject, e.text].filter(Boolean).join(' · '))}</span><span class="cv">${esc(when(e))}</span></div>`).join('')}</div>`
    : ''
  return top + monthGrid(w, ctx, color) + lines
}

// What is playing: its title and artist (the album after), the app on the
// right, how far along as a bar with the times (paused said), and its
// buttons. Nothing playing: said so.
function mediaCard(w, ctx) {
  const rows = rowsOf(w)
  const said = Widgets.words(ctx.lang, w, true)
  const state = rows.find(r => r.kind === 'state')?.value || ''
  if (state === 'none' || !rows.length) return `<div class="crow"><span class="cwords"><b>${esc(said.label)}</b></span></div>`
  const track = rows.find(r => r.kind === 'track') || {}
  const album = rows.find(r => r.kind === 'album')?.text || ''
  const app = rows.find(r => r.kind === 'app')?.name || ''
  const title = track.subject || said.label
  const artist = track.text || ''
  const color = colorOf(w, ctx)
  const s = seriesOf(w)
  const top = `<div class="crow"><span class="cwords"><b>${esc(title)}</b><span>${esc([artist, album].filter(Boolean).join(' · '))}</span></span>${app ? `<span class="cright"><span>${esc(app)}</span></span>` : ''}</div>`
  const paused = state === 'paused' ? `${esc(t(ctx, 'act.pause'))} · ` : ''
  const bar =
    s && s.points.length === 1 && Number.isFinite(s.hi) && s.hi > 0
      ? `<span class="wmeter"><i style="width:${num(Math.max(0, Math.min(100, (s.points[0] / s.hi) * 100)))}%;background:${color}"></i></span><div class="cfoot"><span>${esc(clockOf(s.points[0]))}</span><span>${paused}${esc(clockOf(s.hi))}</span></div>`
      : Widgets.detailOf(w).extra
        ? `<div class="cfoot"><span>${esc(Widgets.detailOf(w).extra)}</span></div>`
        : ''
  return top + bar + `<div class="crow cacts">${actionsHTML(w, ctx)}</div>`
}
// Seconds as a player shows them: m:ss, or h:mm:ss past an hour.
function clockOf(secs) {
  const n = Math.max(0, Math.floor(secs || 0))
  const two = v => String(v).padStart(2, '0')
  return n >= 3600 ? `${Math.floor(n / 3600)}:${two(Math.floor((n % 3600) / 60))}:${two(n % 60)}` : `${Math.floor(n / 60)}:${two(n % 60)}`
}

// Whether it has a card (a large size).
const hasCard = w => !!w && Widgets.sizesOf(w).includes('l')

function cardHTML(w, ctx, all = []) {
  switch (kindOf(w)) {
    case 'weather':
      return weatherCard(w, ctx)
    case 'monitor':
      return monitorCard(w, ctx)
    case 'stock':
      return stockCard(w, ctx, all)
    case 'today':
      return todayCard(w, ctx, all)
    case 'mail':
      return mailCard(w, ctx)
    case 'calendar':
      return calendarCard(w, ctx)
    case 'media':
      return mediaCard(w, ctx)
    default:
      return genericCard(w, ctx)
  }
}

// --- Times ---------------------------------------------------------------------------
// How long up: days and hours, or hours and minutes.
function upFor(secs, ctx) {
  const s = Math.max(0, Math.floor(secs || 0))
  const days = Math.floor(s / 86400)
  const hours = Math.floor((s % 86400) / 3600)
  const minutes = Math.floor((s % 3600) / 60)
  return days ? t(ctx, 'card.daysHours', { d: days, h: hours }) : t(ctx, 'card.hoursMinutes', { h: hours, m: minutes })
}

// When a letter came: its time today, yesterday, its weekday this week,
// else its date.
function whenOf(at, ctx) {
  if (!Number.isFinite(at) || at <= 0) return ''
  const now = new Date(ctx.now || Date.now())
  const d = new Date(at)
  const day = x => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime()
  const ago = Math.round((day(now) - day(d)) / 86400000)
  if (ago <= 0) return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
  if (ago === 1) return t(ctx, 'card.yesterday')
  if (ago < 7) return t(ctx, `card.weekday${d.getDay()}`)
  return `${d.getMonth() + 1}/${d.getDate()}`
}

export {
  KINDS, kindOf, colorOf, trendOf, glyphOf, glyphHTML, memberGlyph, titleOf, briefOf, spark, ring, bars,
  hourSlots, hourBars, faceHTML, emptyFaceHTML, rowHTML, stackRowHTML, hasCard, cardHTML, upFor, whenOf, esc,
  actionsHTML, monthGrid, clockOf, shorten,
}
