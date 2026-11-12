// Widgets on the desktop, each size (the widget design of 2026-10-16, §4,
// §8): small 156 x 156, medium 324 x 156, large 324 x 324, what each kind
// shows in each. Markup as text for desk.ts to put in a glass card (the
// card itself, its glass and its glow are desk.ts's); the drawing helpers,
// the words and the colours are wcards.ts's, as on the taskbar and in the
// capsule, so a stock's rise is the same colour everywhere.
//
// ctx: { lang, light, detailed (the specifics may show), now (ms), hour,
// all (every widget: a stock's others, today's tokens) }.
import * as I18n from '../shared/i18n.ts'
import { render } from '../shared/i18n.ts'
import * as WCards from './wcards.ts'
import { esc, spark, ring, kindOf, colorOf, briefOf, titleOf, trendOf } from './wcards.ts'
import * as Widgets from './widgets.ts'

const t = (ctx, key, vars?) => I18n.t(ctx.lang, key, vars)

const UP = ['#34d27b', '#0f7b3f']
const DOWN = ['#ff6b6b', '#c42b3c']
const pick = (pair, ctx) => pair[ctx.light ? 1 : 0]
const trendColor = (w, ctx) => (trendOf(w) === 'red' ? pick(DOWN, ctx) : trendOf(w) === 'green' ? pick(UP, ctx) : '')
const track = ctx => (ctx.light ? 'rgba(0,0,0,.09)' : 'rgba(255,255,255,.12)')
const detailOf = w => Widgets.detailOf(w)
const rowsOf = w => (Array.isArray(detailOf(w).rows) ? detailOf(w).rows : [])
const row = (w, kind) => rowsOf(w).find(r => r.kind === kind)
const varsOf = w => (w?.value && typeof w.value === 'object' ? w.value.vars || {} : {})
// A private widget's words blurred until the pointer is on them (desk.css
// .wpriv.hid), as the specifics setting says.
const priv = (w, text, ctx) => (w?.private && !ctx.detailed ? `<span class="wpriv hid">${esc(text)}</span>` : esc(text))
const nodata = ctx => `<span class="nodata">${esc(t(ctx, 'taskbar.w.none'))}</span>`
const meter = (pct, color) => `<div class="meter"><i style="width:${Math.max(0, Math.min(100, pct || 0))}%;background:${color}"></i></div>`
// A number and its unit after it, half as big (46px, "天" 23px).
const big = (n, unit = '', cls = '') => `<div class="big${cls ? ` ${cls}` : ''}">${esc(n)}${unit ? `<small>${esc(unit)}</small>` : ''}</div>`
// A share "17%" as its number and its sign.
const share = text => {
  const m = /^\s*(-?[\d.]+)\s*%\s*$/.exec(String(text ?? ''))
  return m ? { n: m[1], unit: '%', pct: parseFloat(m[1]) } : { n: String(text ?? '—'), unit: '', pct: 0 }
}

// The head: its glyph in its colour (her face for today), its name, and
// something on the right (a place, how many).
function head(w, name, ctx, right = '') {
  return `<div class="hd">${WCards.glyphHTML(w, 'gl')}<span class="hn">${esc(name)}</span>${right ? `<span class="r">${right}</span>` : ''}</div>`
}

// --- Weather (s, m, l) ------------------------------------------------------------------
const SKY = { sun: 'E706', moon: 'E708', partly: 'E753', cloud: 'E753', fog: 'E753', rain: 'EB42', snow: 'EA38', storm: 'E945' }
function weather(w, size, ctx) {
  const said = Widgets.words(ctx.lang, w, true)
  const [city = '', sky = ''] = String(said.label).split(' · ')
  const range = detailOf(w).extra || ''
  const color = colorOf(w, ctx)
  const hours = rowsOf(w).filter(r => r.kind === 'hour')
  const strip = n =>
    hours.length
      ? `<div class="wx-hours" style="grid-template-columns:repeat(${Math.min(n, hours.length)},1fr)">${hours
          .slice(0, n)
          .map(h => `<div>${esc(h.when)}<i style="color:${color}">&#x${SKY[h.icon] || 'E753'};</i><b>${esc(h.value)}</b></div>`)
          .join('')}</div>`
      : `<div class="ter">${nodata(ctx)}</div>`
  if (size === 's') return `${head(w, titleOf(w, ctx), ctx)}<div class="grow"></div>${big(said.value)}<div class="sub" style="margin-top:6px">${esc([sky, city].filter(Boolean).join(' · '))}</div><div class="ter">${esc(range)}</div>`
  const top = `<div class="row top">${big(said.value)}<div class="beside"><div class="sub">${esc(sky)}</div><div class="ter">${esc(range)}</div></div></div>`
  if (size === 'm') return `${head(w, titleOf(w, ctx), ctx, esc(city))}${top}<div class="grow"></div>${strip(6)}`
  const series = detailOf(w).series
  const curve = series ? `<div class="curve">${spark(series.points, 292, 64, color, { pad: 8 })}</div>` : `<div class="curve none">${nodata(ctx)}</div>`
  const days = rowsOf(w).filter(r => r.kind === 'day').slice(0, 2)
  const foot = days.length ? `<div class="row ter foot">${days.map((d, i) => `<span${i === 0 ? ' class="n"' : ''}>${esc([d.when, d.value, d.name].filter(Boolean).join(' '))}</span>`).join('')}</div>` : ''
  return `${head(w, titleOf(w, ctx), ctx, esc(city))}${top}${curve}${strip(7)}<div class="grow"></div>${foot}`
}

// --- The system monitor (s, m, l) ---------------------------------------------------------
function monitor(w, size, ctx) {
  const color = colorOf(w, ctx)
  const cpu = row(w, 'cpu')
  const mem = row(w, 'memory')
  const gpu = row(w, 'gpu')
  const disk = row(w, 'disk')
  const net = row(w, 'net')
  const up = row(w, 'uptime')
  const c = share(cpu?.value)
  const name = titleOf(w, ctx)
  if (size === 's') {
    const inner = cpu ? `<span class="ringn">${esc(c.n)}<small>%</small></span>` : '<span class="ringn">—</span>'
    const lines = [`<div class="ter">CPU</div>`, `<div class="sub">${mem ? esc(`${t(ctx, 'card.memory')} ${mem.value}`) : nodata(ctx)}</div>`, net ? `<div class="ter num">↓ ${esc(net.value)}</div>` : '']
    return `${head(w, name, ctx)}<div class="grow"></div><div class="row" style="gap:12px"><div class="ringbox" style="width:58px;height:58px">${ring(c.pct / 100, 58, 5, color, track(ctx))}${inner}</div><div class="lines" style="gap:4px">${lines.join('')}</div></div>`
  }
  if (size === 'm') {
    // One not read (no graphics card in a VM): greyed with "no data", as
    // the large one says it (the design session, §8).
    const col = (label, r) => {
      const s = share(r?.value)
      if (!r) return `<div class="col off"><div class="row"><span class="ter n">${esc(label)}</span>${nodata(ctx)}</div>${meter(0, color)}</div>`
      return `<div class="col"><div class="row"><span class="ter n">${esc(label)}</span><span class="num mid">${esc(s.n)}<small>${esc(s.unit)}</small></span></div>${meter(s.pct, color)}</div>`
    }
    const points = Array.isArray(net?.points) ? net.points : []
    const netLine = net ? `<span class="sub num">↓ ${esc(net.value)} · ↑ ${esc(net.text || '')}</span>` : `<span class="sub">${nodata(ctx)}</span>`
    return `${head(w, name, ctx)}<div class="row cols">${col('CPU', cpu)}${col(t(ctx, 'card.memory'), mem)}${col('GPU', gpu)}</div><div class="grow"></div><div class="row" style="align-items:flex-end"><div class="lines" style="gap:2px"><span class="ter">${esc(t(ctx, 'card.net'))}</span>${netLine}</div><div style="margin-left:auto">${points.length > 1 ? spark(points, 110, 28, color) : ''}</div></div>`
  }
  const about = cpu ? ['CPU', cpu.name && cpu.unit ? t(ctx, 'card.cores', { cores: cpu.name, threads: cpu.unit }) : '', cpu.text || ''].filter(Boolean).join(' · ') : 'CPU'
  const series = detailOf(w).series
  const line = (label, r) => {
    if (!r) return `<div class="mline off"><div class="row"><span class="n sub">${esc(label)}</span>${nodata(ctx)}</div></div>`
    const s = share(r.value)
    const sub = [r.name, r.text].filter(Boolean).join(' · ')
    return `<div class="mline"><div class="row"><span class="n sub">${esc(label)}</span><span class="ter">${esc(sub)}</span><span class="num pct">${esc(r.value)}</span></div>${meter(s.pct, color)}</div>`
  }
  const foot = `<div class="row ter foot"><span class="n">${net ? esc(`${t(ctx, 'card.net')} ↓ ${net.value} ↑ ${net.text || ''}`) : `${esc(t(ctx, 'card.net'))} ${nodata(ctx)}`}</span><span>${up ? esc(t(ctx, 'card.up', { time: WCards.upFor(Number(up.value), ctx) })) : ''}</span></div>`
  return `${head(w, name, ctx)}<div class="row top">${big(cpu ? c.n : '—', cpu ? '%' : '', 'md')}<span class="sub beside">${esc(about)}</span></div><div class="curve">${series ? spark(series.points, 292, 70, color, { pad: 4, lo: 0, hi: 100 }) : nodata(ctx)}</div><div class="lines mlines">${line(t(ctx, 'card.memory'), mem)}${line('GPU', gpu)}${line(t(ctx, 'card.disk'), disk)}</div><div class="grow"></div>${foot}`
}

// --- Stocks (s, m, l): the first, and the plugin's others -------------------------------
function stockLine(x, ctx) {
  const b = briefOf(x, ctx)
  const r = rowsOf(x)[0] || {}
  return { code: r.code || Widgets.words(ctx.lang, x, true).label, name: Widgets.words(ctx.lang, x, true).label, price: b.big, change: b.unit, color: trendColor(x, ctx) || colorOf(x, ctx), up: trendOf(x), series: detailOf(x).series, volume: r.value }
}
const chip = (s, extra = '') => (s.change ? `<span class="chip" style="--t:${s.color}${extra}">${esc(s.change)}</span>` : '')
function stock(w, size, ctx) {
  const mine = (ctx.all || []).filter(x => x.plugin && x.plugin === w.plugin)
  const list = [w, ...mine.filter(x => x !== w)]
  const first = stockLine(w, ctx)
  const name = t(ctx, 'desk.stocks')
  if (size === 's') {
    const curve = first.series ? spark(first.series.points.slice(-20), 54, 18, first.color, { fill: false, dot: false }) : ''
    return `${head(w, name, ctx)}<div class="grow"></div><div class="ter">${esc(first.code)}</div>${big(first.price, '', 'sm')}<div class="row" style="margin-top:6px">${chip(first)}<span style="margin-left:auto">${curve}</span></div>`
  }
  if (size === 'm') {
    const lines = list
      .slice(0, 3)
      .map(x => {
        const s = stockLine(x, ctx)
        const curve = s.series ? spark(s.series.points.slice(-24), 90, 22, s.color, { fill: false, dot: false }) : '<span class="nocurve"></span>'
        const label = s.name && s.name !== s.code ? `<div class="ter">${esc(s.name)}</div>` : ''
        return `<div class="row sline"><div class="who"><div class="code">${esc(s.code)}</div>${label}</div>${curve}<span class="num price">${esc(s.price)}</span>${chip(s, ';width:52px;justify-content:center')}</div>`
      })
      .join('')
    return `${head(w, name, ctx)}<div class="lines slines">${lines}</div>`
  }
  const range = first.series && Number.isFinite(first.series.hi) && Number.isFinite(first.series.lo) ? t(ctx, 'card.highLow', { hi: first.series.hi, lo: first.series.lo }) : ''
  const off = esc(t(ctx, 'card.onlyDay'))
  const tabs = `<div class="tabs"><span class="on">${esc(t(ctx, 'card.day'))}</span><span class="off" title="${off}">${esc(t(ctx, 'card.week'))}</span><span class="off" title="${off}">${esc(t(ctx, 'card.month'))}</span><span class="off" title="${off}">${esc(t(ctx, 'card.year'))}</span></div>`
  const others = list
    .slice(1, 3)
    .map(x => {
      const s = stockLine(x, ctx)
      return `<div class="row"><span class="code" style="width:60px">${esc(s.code)}</span><span class="n ter">${esc(s.name !== s.code ? s.name : '')}</span><span class="num">${esc(s.price)}</span>${chip(s, ';width:52px;justify-content:center')}</div>`
    })
    .join('')
  const title = [first.code, first.name !== first.code ? first.name : ''].filter(Boolean).join(' · ')
  return `${head(w, name, ctx)}<div class="row top"><div><div class="code big-title">${esc(title)}</div>${big(first.price, '', 'md')}</div><div class="right">${chip(first)}${first.volume ? `<div class="ter">${esc(t(ctx, 'card.volume', { n: first.volume }))}</div>` : ''}</div></div><div class="curve">${first.series ? spark(first.series.points, 292, 96, first.color, { pad: 4 }) : nodata(ctx)}</div><div class="row">${tabs}<span class="ter" style="margin-left:auto">${esc(range)}</span></div><div class="grow"></div><div class="lines" style="gap:6px">${others}</div>`
}

// --- Mail (s, m, l): a mailbox's unread, its newest ----------------------------------------
function mail(w, size, ctx) {
  const n = varsOf(w).n ?? Widgets.words(ctx.lang, w, true).value
  const rows = rowsOf(w)
  const label = render(ctx.lang, w.label)
  const name = t(ctx, 'card.mail')
  const letter = r => `<div class="mailrow"><b>${priv(w, r.from || '', ctx)}</b><em>${esc(WCards.whenOf(r.at, ctx))}</em><span>${priv(w, r.subject || '', ctx)}</span></div>`
  if (size === 's') {
    const newest = rows[0] ? `${rows[0].from || ''}：${rows[0].subject || ''}` : ''
    return `${head(w, name, ctx)}<div class="grow"></div>${big(n)}<div class="sub" style="margin-top:4px">${esc(t(ctx, 'card.unread'))}</div><div class="ter ell">${newest ? priv(w, newest, ctx) : priv(w, label, ctx)}</div>`
  }
  const count = esc(t(ctx, 'desk.unread', { n }))
  const list = rows.length ? rows.slice(0, size === 'm' ? 3 : 6).map(letter).join('') : `<div class="ter">${nodata(ctx)}</div>`
  if (size === 'm') return `${head(w, name, ctx, count)}<div class="lines mails">${list}</div>`
  // Its mailbox as the one tab (each mailbox is a widget of its own).
  return `${head(w, name, ctx, count)}<div class="tabs" style="margin:10px 0 4px;align-self:flex-start"><span class="on">${priv(w, label, ctx)}</span></div><div class="lines mails">${list}</div>`
}

// --- Claude today (s, m, l), and today's tokens (s, m) ---------------------------------------
function today(w, size, ctx) {
  const v = varsOf(w)
  const tokens = varsOf((ctx.all || []).find(x => x.id === 'tokens'))
  const time = render(ctx.lang, v.time) || '—'
  const name = titleOf(w, ctx)
  if (size === 's') return `${head(w, name, ctx)}<div class="grow"></div>${big(v.turns ?? '—')}<div class="sub" style="margin-top:4px">${esc([t(ctx, 'card.turnsUnit'), time].join(' · '))}</div><div class="ter">${esc([t(ctx, 'card.approved', { n: v.approvals ?? '—' }), tokens.total ? `${tokens.total} token` : ''].filter(Boolean).join(' · '))}</div>`
  const cell = (value, label, cls = '') => `<div><div class="${cls || 'num mid'}">${esc(value)}</div><div class="ter" style="margin-top:2px">${esc(label)}</div></div>`
  const lit = Number.isFinite(ctx.hour) ? ctx.hour : new Date(ctx.now || Date.now()).getHours()
  const series = detailOf(w).series
  const color = colorOf(w, ctx)
  const right = esc(t(ctx, 'desk.today'))
  // The hours as fixed slots (wcards.ts hourBars): twelve at least, 18 px at most each.
  if (size === 'm') {
    const chart = series ? WCards.hourBars(series.points, lit, 292, 30, color, track(ctx)).svg : nodata(ctx)
    return `${head(w, name, ctx, right)}<div class="row nums">${cell(v.turns ?? '—', t(ctx, 'card.turnsUnit'), 'big md')}${cell(time, t(ctx, 'card.worked'))}${cell(tokens.total || '—', 'token')}</div><div class="grow"></div>${chart}`
  }
  const b = series ? WCards.hourBars(series.points, lit, 292, 54, color, track(ctx)) : null
  const chart = b ? `<div style="margin-top:6px">${b.svg}</div><div class="row ter" style="margin-top:4px"><span class="n">${esc(t(ctx, 'card.hour', { h: b.from }))}</span><span>${esc(t(ctx, 'card.hour', { h: b.to }))}</span></div>` : `<div>${nodata(ctx)}</div>`
  return `${head(w, name, ctx, right)}<div class="row nums">${cell(v.turns ?? '—', t(ctx, 'card.turnsUnit'), 'big md')}${cell(time, t(ctx, 'card.worked'))}${cell(v.approvals ?? '—', t(ctx, 'card.approvals'))}</div><div class="ter" style="margin-top:14px">${esc(t(ctx, 'card.byHour'))}</div>${chart}<div class="ter" style="margin-top:12px">${esc(t(ctx, 'card.tokenCurve'))}</div><div class="off">${nodata(ctx)}</div><div class="grow"></div><div class="row off"><span class="sub n">${esc(t(ctx, 'card.busiest'))}</span>${nodata(ctx)}</div>`
}
// Today's tokens: small, the total over "token"; medium, the total and
// the output larger, in the middle of it, or over their curve when the
// hours come (the design session: not an empty half).
function tokens(w, size, ctx) {
  const v = varsOf(w)
  const said = Widgets.words(ctx.lang, w, true)
  const name = render(ctx.lang, w.label) || t(ctx, 'widget.tokens')
  if (size === 's') return `${head(w, name, ctx)}<div class="grow"></div>${big(v.total || said.value)}<div class="sub" style="margin-top:4px">token</div><div class="ter">${esc(v.output ? t(ctx, 'card.output', { n: v.output }) : '')}</div>`
  const series = detailOf(w).series
  const nums = `<div class="row nums tokens"><div>${big(v.total || said.value, '', series ? 'md' : '')}<div class="ter" style="margin-top:4px">token</div></div><div>${big(v.output || '—', '', series ? 'sm' : 'md')}<div class="ter" style="margin-top:4px">${esc(t(ctx, 'desk.output'))}</div></div></div>`
  if (!series) return `${head(w, name, ctx, esc(t(ctx, 'desk.today')))}<div class="grow"></div>${nums}<div class="grow"></div>`
  return `${head(w, name, ctx, esc(t(ctx, 'desk.today')))}${nums}<div class="grow"></div>${spark(series.points, 292, 40, colorOf(w, ctx))}`
}

// --- Pomodoro (s, m), countdown (s) ---------------------------------------------------------
function pomodoro(w, size, ctx) {
  const said = Widgets.words(ctx.lang, w, true)
  const s = detailOf(w).series
  const left = s && Number.isFinite(s.points?.[0]) && Number.isFinite(s.hi) && s.hi > 0 ? s.points[0] / s.hi : 0
  const color = colorOf(w, ctx)
  const name = titleOf(w, ctx)
  const face = px => `<div class="ringbox" style="width:${px}px;height:${px}px">${ring(left, px, 6, color, track(ctx))}<span class="ringn tomato"><span>${esc(said.value)}</span><span class="ter">${esc(said.label)}</span></span></div>`
  if (size === 's') return `${head(w, name, ctx)}<div class="grow"></div><div style="align-self:center">${face(84)}</div><div class="grow"></div>`
  const round = row(w, 'round')
  const n = Number(round?.value) || 0
  const of = Math.max(n, Number(round?.unit) || 4)
  const dots = round ? `<div class="row" style="gap:5px">${Array.from({ length: of }, (_, i) => `<i class="dot" style="background:${i < n ? color : track(ctx)}"></i>`).join('')}<span class="ter" style="margin-left:4px">${esc(t(ctx, 'desk.round', { n, of }))}</span></div>` : ''
  const why = esc(t(ctx, 'desk.noControl'))
  const right = round ? esc(t(ctx, 'desk.nth', { n })) : ''
  return `${head(w, name, ctx, right)}<div class="row" style="margin-top:8px;gap:18px;flex:1">${face(96)}<div class="lines" style="gap:8px">${dots}<div class="sub">${esc(detailOf(w).extra || '')}</div><div class="btns"><button class="wbtn off" type="button" title="${why}">${esc(t(ctx, 'desk.pause'))}</button><button class="wbtn off" type="button" title="${why}">${esc(t(ctx, 'desk.skip'))}</button></div></div></div>`
}
function countdown(w, size, ctx) {
  const said = Widgets.words(ctx.lang, w, true)
  const days = row(w, 'days')
  return `${head(w, titleOf(w, ctx), ctx)}<div class="grow"></div>${days ? big(days.value, days.unit || '') : big(said.value)}<div class="sub ell" style="margin-top:4px">${esc(said.label)}</div><div class="ter">${esc(detailOf(w).extra || '')}</div>`
}

// --- The calendar (s, m, l) --------------------------------------------------------------------
// Small: the day of the month large, the month and weekday, the lunar
// date (or the week). Medium: those on the left, the month's grid on the
// right (wcards.ts monthGrid: today lit, a dot on a day with something
// on). Large: and what is coming under them, a line each.
function calendar(w, size, ctx) {
  const d = detailOf(w)
  const date = row(w, 'date') || {}
  const day = date.value || String(new Date(ctx.now || Date.now()).getDate())
  const color = colorOf(w, ctx)
  const name = titleOf(w, ctx)
  const monthWeekday = esc([date.name, date.unit].filter(Boolean).join(' · '))
  const events = rowsOf(w).filter(r => r.kind === 'event')
  if (size === 's') return `${head(w, name, ctx)}<div class="grow"></div>${big(day)}<div class="sub" style="margin-top:4px">${monthWeekday}</div><div class="ter ell">${esc(d.extra || '')}</div>`
  // The year, the lunar date and the day's marks a line each (one long
  // line would wrap mid-word).
  const notes = String(date.text || d.extra || '')
    .split(' · ')
    .filter(Boolean)
    .map(part => `<div class="ter ell">${esc(part)}</div>`)
    .join('')
  const block = `<div class="lines cal-date">${big(day, '', 'md')}<div class="sub">${monthWeekday}</div>${notes}</div>`
  const pair = `<div class="row cal-m">${block}${WCards.monthGrid(w, ctx, color)}</div>`
  if (size === 'm') return `${head(w, name, ctx)}${pair}`
  const when = e => (e.unit ? `${e.when}–${e.unit}` : e.when || '')
  const list = events
    .slice(0, 4)
    .map(e => `<div class="row evrow"><span class="ter n">${esc(e.name || '')}</span><span class="sub ell">${esc([e.subject, e.text].filter(Boolean).join(' · '))}</span><span class="ter num">${esc(when(e))}</span></div>`)
    .join('')
  const coming = events.length ? `<div class="ter" style="margin-top:10px">${esc(t(ctx, 'cal.coming'))}</div><div class="lines evlist">${list}</div>` : ''
  return `${head(w, name, ctx)}${pair}${coming}<div class="grow"></div>`
}

// --- What is playing (s, m, l) ------------------------------------------------------------------
// The title and artist (the album after), the app at the head's right,
// how far along as a bar with the times (paused said), and its buttons
// (wcards.ts actionsHTML: previous, play or pause, next, pressed back to
// its plugin). Nothing playing: said so, no buttons.
function media(w, size, ctx) {
  const said = Widgets.words(ctx.lang, w, true)
  const state = row(w, 'state')?.value || ''
  const name = titleOf(w, ctx)
  const color = colorOf(w, ctx)
  if (state === 'none' || !rowsOf(w).length) return `${head(w, name, ctx)}<div class="grow"></div><div class="sub">${esc(said.label)}</div><div class="grow"></div>`
  const track = row(w, 'track') || {}
  const title = track.subject || said.label
  const artist = track.text || ''
  const album = row(w, 'album')?.text || ''
  const app = row(w, 'app')?.name || ''
  const s = detailOf(w).series
  const share = s && s.points?.length === 1 && Number.isFinite(s.hi) && s.hi > 0 ? Math.round(Math.max(0, Math.min(100, (s.points[0] / s.hi) * 100)) * 10) / 10 : null
  const paused = state === 'paused' ? `${esc(t(ctx, 'act.pause'))} · ` : ''
  const times = share == null ? (detailOf(w).extra ? `<div class="ter num">${esc(detailOf(w).extra)}</div>` : '') : `<div class="row ter num" style="justify-content:space-between"><span>${esc(WCards.clockOf(s.points[0]))}</span><span>${paused}${esc(WCards.clockOf(s.hi))}</span></div>`
  const prog = share == null ? '' : meter(share, color)
  if (size === 's') return `${head(w, name, ctx)}<div class="grow"></div><div class="mtitle two">${esc(title)}</div><div class="ter ell">${esc(artist)}</div><div class="grow"></div>${WCards.actionsHTML(w, ctx, 'sm')}`
  if (size === 'm') return `${head(w, name, ctx, esc(app))}<div class="mtitle">${esc(title)}</div><div class="sub ell">${esc([artist, album].filter(Boolean).join(' · '))}</div><div class="grow"></div>${prog}${times}${WCards.actionsHTML(w, ctx)}`
  return `${head(w, name, ctx, esc(app))}<div class="grow"></div><div class="mtitle big two">${esc(title)}</div><div class="sub ell" style="margin-top:4px">${esc(artist)}</div><div class="ter ell">${esc(album)}</div><div class="grow"></div>${prog}${times}<div class="grow"></div>${WCards.actionsHTML(w, ctx, 'lg')}`
}

// --- Anyone else's (s, m; l when it says so) ---------------------------------------------------
// A script that gives only a label and a value (the spec's §1): medium in
// two lines, its value over its name, left, in the middle; no empty block.
function generic(w, size, ctx) {
  const said = Widgets.words(ctx.lang, w, ctx.detailed)
  const d = detailOf(w)
  const color = colorOf(w, ctx)
  const label = nameOf(w, ctx)
  if (size === 's') return `${head(w, label, ctx)}<div class="grow"></div>${big(said.value, '', 'sm')}<div class="ter ell">${esc(d.extra || '')}</div>`
  const rows = size === 'l' ? rowsOf(w).slice(0, 6).map(r => `<div class="row"><span class="n sub">${priv(w, r.name || r.from || r.code || r.when || '', ctx)}</span><span class="ter">${priv(w, r.subject || r.text || '', ctx)}</span><span class="num">${esc(r.value || r.price || '')}</span></div>`) : []
  if (!d.series && !rows.length) return `<div class="grow"></div><div class="twoline">${big(said.value, '', 'md')}<div class="row" style="margin-top:6px">${WCards.glyphHTML(w, 'gl')}<span class="sub n">${esc(label)}</span></div>${d.extra ? `<div class="ter ell">${esc(d.extra)}</div>` : ''}</div><div class="grow"></div>`
  const curve = d.series ? `<div class="curve">${spark(d.series.points, 292, size === 'l' ? 96 : 40, color, { lo: d.series.lo, hi: d.series.hi })}</div>` : ''
  return `${head(w, label, ctx)}<div class="row top">${big(said.value, '', 'md')}<span class="sub beside">${esc(d.extra || '')}</span></div>${curve}<div class="grow"></div>${rows.length ? `<div class="lines">${rows.join('')}</div>` : ''}`
}

// --- Names: never an id, never a bare "widget" (the design session) --------------------------
// A plugin's own name ("天气"; someone else's, as its widgets say it:
// widgets.ts pluginName), none for one she does not know.
const pluginName = (plugin, ctx) => (plugin ? Widgets.pluginName(ctx.lang, plugin, ctx.all || []) : '')
// The plugin a widget comes from: its own say, else its id's first part
// as plugins name theirs ("stock-aapl").
const pluginOf = id => (/^([a-z]+)-/.exec(String(id || '')) || [])[1] || ''
const BUILT_IN = { monitor: 'card.monitor', today: 'card.today', tokens: 'widget.tokens', weather: 'card.weather', pomodoro: 'card.pomodoro', countdown: 'card.countdown', calendar: 'card.calendar', media: 'card.media' }
// A widget's name: its own label; with none (or private, the specifics
// hidden), its plugin's name and its number among that plugin's ("天气
// 2"); else "小工具" and its number.
function nameOf(w, ctx) {
  const label = Widgets.words(ctx.lang, w, ctx.detailed).label
  if (label) return label
  return baseName({ type: w.plugin ? 'plugin' : 'widget', of: w.plugin || w.id }, ctx, w.id)
}
// A member put there (no words come yet): the name it was put there with,
// a built-in one's, else its plugin's; n its number among the same name's
// on the display (desk.ts), when more than one.
function memberName(m, ctx, n = 0) {
  if (m.name) return m.name
  const base = baseName(m, ctx, m.of)
  return n > 1 ? `${base} ${n}` : base
}
function baseName(m, ctx, id) {
  if (m.type === 'widget' && BUILT_IN[m.of]) {
    const said = t(ctx, BUILT_IN[m.of])
    if (said !== BUILT_IN[m.of]) return said
  }
  const plugin = m.type === 'plugin' ? m.of : pluginOf(id)
  const named = pluginName(plugin, ctx) || (/^inbox-/.test(String(id)) ? t(ctx, 'card.mail') : '')
  if (named) {
    const mates = (ctx.all || []).filter(x => (plugin && (x.plugin === plugin || pluginOf(x.id) === plugin)) || (!plugin && x.from === 'mail'))
    const at = mates.findIndex(x => x.id === id)
    return mates.length > 1 && at >= 0 ? `${named} ${at + 1}` : named
  }
  return t(ctx, 'desk.widget')
}

const BY_KIND = { weather, monitor, stock, mail, today, tokens, pomodoro, countdown, calendar, media }
// A widget's face at a size (one it has: desk.ts keeps to them).
function faceHTML(w, size, ctx) {
  const draw = BY_KIND[kindOf(w)] || generic
  return draw(w, size, ctx)
}

// One with nothing to say (§8): greyed, its name and "no data"; loading,
// a glint where its value goes (once, never endless); stopped, its glyph
// amber and why.
function emptyHTML(member, name, state, ctx) {
  const glyph = state === 'error' ? '<span class="gl werr" aria-hidden="true">&#xE7BA;</span>' : WCards.memberGlyph(member)
  const what = state === 'loading' ? '<span class="wshim"></span>' : `<span class="nodata">${esc(t(ctx, state === 'needs' ? 'taskbar.w.needs' : state === 'error' ? 'card.stopped' : 'taskbar.w.none'))}</span>`
  return `<div class="hd">${glyph}<span class="hn">${esc(name)}</span></div><div class="grow"></div><div class="empty">${what}</div><div class="grow"></div>`
}

export { faceHTML, emptyHTML, share, nameOf, memberName }
