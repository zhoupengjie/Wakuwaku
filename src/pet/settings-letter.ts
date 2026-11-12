// Writing a letter (a new one, a reply, a forward) and sending it; a letter
// read; the Mail page drawn, with its accounts and the codes come by mail;
// the Network page (the proxy, how she reaches the web, its test); and the
// menus a letter and a button open.
// Made by settings.ts in the section's place as the page starts
// (letterPart(core), imported by settings.ts): what it takes of the settings' core
// comes in core, its functions and constants as they are, its lets read
// and written through core.<name> as they change; what it hands back is the
// settings' again.
import * as Widgets from './widgets.ts'
import { pet } from './bridge.ts'

// What this part takes of the settings' core: the names it reads (and,
// of its lets, writes); settings.ts's call is checked against them.
export interface LetterCore {
  AGENTS: any
  COLOR: any
  MOST_FILES: any
  PROXY_KINDS: any
  T: any
  Widgets: any
  accountOf: any
  agentsHTML: any
  body: any
  choicesOf: any
  confOf: any
  draft: any
  draw: any
  dropSure: any
  esc: any
  expanded: any
  firstAgent: any
  handButtons: any
  handNote: any
  handNoteTimer: any
  inbox: any
  inboxHTML: any
  inboxId: any
  isLetter: any
  isReading: any
  keepDraft: any
  labelOf: any
  lang: any
  layer: any
  listed: any
  loginWhy: any
  mailDate: any
  mailError: any
  mailForm: any
  mailFormHTML: any
  mailList: any
  mailNote: any
  mailSize: any
  menuPick: any
  patch: any
  people: any
  picking: any
  providerOf: any
  proxyTyped: any
  reading: any
  redrawMail: any
  relayout: any
  row: any
  sec: any
  seg: any
  setConf: any
  snap: any
  sw: any
  tab: any
  talkHTML: any
  talkOf: any
  twoPanes: any
  where: any
  writing: any
}

export function letterPart(core: LetterCore) {
  const {
    AGENTS, COLOR, MOST_FILES, PROXY_KINDS, T, Widgets, accountOf, agentsHTML, body, choicesOf, confOf, draw,
    esc, expanded, firstAgent, handButtons, inbox, inboxHTML, inboxId, isLetter, isReading, keepDraft,
    labelOf, layer, listed, loginWhy, mailDate, mailError, mailFormHTML, mailList, mailNote, mailSize, patch,
    people, providerOf, redrawMail, row, sec, seg, setConf, sw, talkHTML, talkOf, twoPanes, where,
  } = core

  const personLine = p => (p.name ? `${/[,;"<>]/.test(p.name) ? `"${p.name.replace(/"/g, '')}"` : p.name} <${p.address}>` : p.address)
  const peopleLine = list => list.map(personLine).join(', ')
  const samePerson = (a, b) => a.address.toLowerCase() === b.address.toLowerCase()
  const eachOnce = list => list.filter((p, i) => p.address && list.findIndex(q => samePerson(p, q)) === i)
  const quoted = text => String(text || '').split('\n').map(line => (line ? `> ${line}` : '>')).join('\n')

  // The letter passed on, as mail programs write it under the new one.
  function forwarded(l) {
    const head = [
      `---------- ${T('mail.w.fwdHead')} ----------`,
      `${T('mail.w.from')}: ${people(l.from)}`,
      `${T('mail.w.date')}: ${mailDate(l.date, true)}`,
      `${T('mail.w.subject')}: ${l.subject || ''}`,
      `${T('mail.to')}: ${people(l.to)}`,
      l.cc.length ? `${T('mail.cc')}: ${people(l.cc)}` : '',
    ]
    return `${head.filter(Boolean).join('\n')}\n\n${l.text || ''}`
  }

  // The account a new letter goes from: the one whose inbox shows, if it is
  // on; else the first that is on and has an outgoing server, the first on,
  // the first.
  function fromAccount() {
    const all = core.snap.mail || []
    const shown = accountOf(inboxId())
    if (shown?.on) return shown.id
    return (all.find(a => a.on && a.smtp) || all.find(a => a.on) || all[0])?.id
  }

  // A letter begun: new (from the account fromAccount picks), a reply to
  // the sender or to all (from the account it came to; the letter quoted
  // under it, words an agent wrote on top, if given), or one passed on
  // (with its attachments). One half written is not thrown away for it:
  // it shows.
  function write(kind, l?, words?) {
    closeMenu()
    if (core.draft && (core.draft.touched || core.draft.files.length)) {
      core.draft.note = T('mail.w.unsent')
      core.writing = true
      return redrawMail()
    }
    const account = accountOf(l?.account)?.id || fromAccount()
    if (!account) return
    const me = (accountOf(account)?.address || '').toLowerCase()
    const notMe = list => (list || []).filter(p => p.address && p.address.toLowerCase() !== me)
    const d = { kind, account, to: '', cc: '', bcc: '', showCc: false, subject: '', text: '', reply: null, forward: null, files: [], touched: !!words, busy: false, error: null, note: null }
    if (l && (kind === 'reply' || kind === 'all')) {
      // To whom it asks replies to go, else who wrote it; one's own letter
      // (in Sent), to whom it went.
      const mine = (l.from || []).some(p => (p.address || '').toLowerCase() === me)
      const back = l.replyTo?.length ? l.replyTo : mine && l.to?.length ? l.to : l.from
      const everyone = eachOnce([...notMe(back), ...notMe(l.to)])
      const to = kind === 'all' && everyone.length ? everyone : back
      d.to = peopleLine(to)
      if (kind === 'all') d.cc = peopleLine(eachOnce(notMe(l.cc)).filter(p => !to.some(t => samePerson(t, p))))
      d.showCc = !!d.cc
      d.subject = /^\s*(re|aw|sv|回复|答复)\s*[:：]/i.test(l.subject || '') ? l.subject : `Re: ${l.subject || ''}`
      d.text = `${words ? `${words}\n\n` : '\n\n'}${T('mail.w.wrote', { date: mailDate(l.date, true), who: people(l.from) })}\n${quoted(l.text)}`
      d.reply = { account: l.account, folder: l.folder || 'inbox', uid: l.uid, messageId: l.messageId, references: l.references || [] }
    }
    if (l && kind === 'forward') {
      d.subject = /^\s*(fwd?|wg|转发)\s*[:：]/i.test(l.subject || '') ? l.subject : `Fwd: ${l.subject || ''}`
      d.text = `\n\n${forwarded(l)}`
      d.forward = { account: l.account, folder: l.folder || 'inbox', uid: l.uid, files: (l.attachments || []).map((f, i) => ({ index: i, name: f.name, size: f.size })) }
    }
    core.draft = d
    core.writing = true
    core.dropSure = false
    keepDraft()
    draw(true)
    core.relayout()
    // The caret where one writes first: the people, or above the quote.
    requestAnimationFrame(() => {
      const box = body.querySelector(kind === 'reply' || kind === 'all' ? '[data-w="text"]' : '[data-w="to"]')
      box?.focus()
      if (box?.matches('textarea')) {
        box.setSelectionRange(0, 0)
        box.scrollTop = 0
      }
    })
  }

  // The same about a letter in the list: opened first (for its text).
  async function writeAbout(kind, account, uid, folder, words?) {
    if (!(core.reading?.letter && isReading(account, folder, uid))) await openLetter(account, uid, folder)
    if (core.reading?.letter && isReading(account, folder, uid)) write(kind, core.reading.letter, words)
  }

  // The reply in an agent's words: the longest ``` block, if it wrote one; else all of it.
  function replyIn(text) {
    const blocks = [...String(text || '').matchAll(/```[^\n]*\n([\s\S]*?)```/g)].map(m => m[1].trim())
    return (blocks.sort((a, b) => b.length - a.length)[0] || String(text || '')).trim()
  }

  // The letter being written: back (it stays), discard, send; from
  // (where there are several accounts), to, cc and bcc (when asked for),
  // the subject, the text; the files, each with ×, and more.
  function writeHTML(note = '') {
    const d = core.draft
    const all = core.snap.mail || []
    const off = d.busy ? 'disabled' : ''
    const two = twoPanes()
    // Back: to the letter answered, or the inbox; beside the list (two
    // panes), put aside.
    const back = two ? 'mail.w.aside' : d.reply || d.forward ? (core.reading?.letter ? 'mail.w.backLetter' : 'mail.inbox') : 'mail.inbox'
    const bar = `<div class="mail-bar"><span class="link" data-w-back>‹ ${esc(T(back))}</span><span class="grow"></span>
      <button class="pbtn sm" data-w-drop ${off}>${esc(T(core.dropSure ? 'mail.w.dropSure' : 'mail.w.drop'))}</button>
      <button class="pbtn sm al" data-w-send title="${esc(T('mail.w.ctrlEnter'))}" ${off}>${esc(T(d.busy ? 'mail.w.sending' : 'mail.w.send'))}</button></div>`
    const box = (key, label, extra = '') =>
      `<div class="fr"><span class="fl">${esc(label)}</span><input class="in" data-w="${key}" value="${esc(d[key] ?? '')}" spellcheck="false" autocomplete="off" ${off}>${extra}</div>`
    const address = accountOf(d.account)?.address || ''
    const from =
      all.length > 1
        ? `<div class="fr"><span class="fl">${esc(T('mail.w.from'))}</span><button class="pick" data-w-from ${off}>${esc(address)} ▾</button></div>`
        : `<div class="fr"><span class="fl">${esc(T('mail.w.from'))}</span><span class="d ellip">${esc(address)}</span></div>`
    const ccLink = d.showCc ? '' : `<span class="link" data-w-cc>${esc(T('mail.w.ccBcc'))}</span>`
    const files = [
      ...(d.forward?.files || []).map((f, i) => ({ ...f, key: `o${i}` })),
      ...d.files.map((f, i) => ({ ...f, key: `f${i}` })),
    ]
      .map(f => `<span class="chip">📎 ${esc(f.name || '?')} · ${mailSize(f.size || 0)}<button class="unfile" data-w-unfile="${f.key}" title="${esc(T('mail.w.unfile'))}" ${off}>×</button></span>`)
      .join('')
    const notes = [d.note ? `<div class="note">${esc(d.note)}</div>` : '', d.error ? `<div class="note err">${esc(d.error)}</div>` : ''].join('')
    const inner = `${sec(T(`mail.w.kind.${d.kind || 'new'}`))}<div class="grp mail-write">${notes}
      ${from}
      ${box('to', T('mail.to'), ccLink)}
      ${d.showCc ? box('cc', T('mail.cc')) + box('bcc', T('mail.w.bcc')) : ''}
      ${box('subject', T('mail.w.subject'))}
      <textarea class="in ta" data-w="text" rows="10" spellcheck="false" ${off}>${esc(d.text)}</textarea>
      <div class="w-files">${files}<button class="pbtn sm" data-w-attach ${off}>📎 ${esc(T('mail.w.attach'))}</button><input type="file" multiple hidden data-w-file></div>
    </div>`
    return two ? `${bar}<div class="pane-r-body">${note}${inner}</div>` : `${bar}${inner}`
  }

  // Files picked, read for the letter; no more than it may take.
  const readBase64 = file =>
    new Promise((ok, no) => {
      const r = new FileReader()
      r.onload = () => ok(String(r.result).split(',')[1] || '')
      r.onerror = () => no(r.error)
      r.readAsDataURL(file)
    })

  async function addFiles(input) {
    core.picking = false
    const d = core.draft
    const list = [...(input.files || [])]
    input.value = ''
    if (!d || !list.length) return
    let total = [...d.files, ...(d.forward?.files || [])].reduce((n, f) => n + (f.size || 0), 0)
    d.error = null
    for (const file of list) {
      if (total + file.size > MOST_FILES) {
        d.error = T('mail.err.tooBig')
        break
      }
      try {
        d.files.push({ name: file.name, type: file.type || 'application/octet-stream', size: file.size, data: await readBase64(file) })
        total += file.size
      } catch {
        d.error = T('mail.err.file', { what: file.name })
      }
    }
    d.touched = true
    redrawMail()
  }

  // Why a letter did not go, in words: the page's own reasons, or the
  // outgoing server's.
  function sendError(got) {
    const err = got.error || {}
    const own = ['address', 'noOne', 'tooMany', 'tooBig', 'tooLong', 'file', 'noSmtp', 'gone', 'noLetter']
    if (own.includes(err.kind)) return T(`mail.err.${err.kind}`, { what: err.text || '' })
    const a = accountOf(core.draft?.account)
    if (!got.out) return mailError(err, a)
    const hint = err.kind === 'login' && !loginWhy(err) && providerOf(a?.host) ? ` ${T(`mail.hint.${providerOf(a.host)}`)}` : ''
    return T('mail.w.outPrefix') + mailError(err, a?.smtp ? { ...a.smtp, proxy: a.proxy } : { host: '?', port: '' }) + hint
  }

  // Sent: the letter gone from here, back where one was, and what came of
  // the copy in Sent.
  async function sendDraft() {
    const d = core.draft
    if (!d || d.busy) return
    if (!`${d.to}${d.cc}${d.bcc}`.trim()) {
      d.error = T('mail.err.noOne')
      return redrawMail('[data-w="to"]')
    }
    Object.assign(d, { busy: true, error: null, note: null })
    core.dropSure = false
    redrawMail()
    const letter = {
      account: d.account,
      to: d.to,
      cc: d.cc,
      bcc: d.bcc,
      subject: d.subject,
      text: d.text,
      reply: d.reply,
      forward: d.forward ? { account: d.forward.account, folder: d.forward.folder || 'inbox', uid: d.forward.uid, keep: d.forward.files.map(f => f.index) } : null,
      files: d.files.map(f => ({ name: f.name, type: f.type, data: f.data })),
    }
    const got = await pet.mail.send(letter)
    if (core.draft !== d) return
    d.busy = false
    if (!got.ok) {
      d.error = sendError(got)
      return redrawMail()
    }
    // The letter answered shows it (the list comes again with the reply in it).
    const answered = d.reply && listed(d.reply.account, d.reply.folder, d.reply.uid)
    if (answered) answered.answered = true
    core.draft = null
    core.writing = false
    keepDraft()
    draw(true)
    core.relayout()
    noteFor({ text: T(`mail.w.sent.${got.kept || 'server'}`), bad: got.kept === 'failed' })
  }

  // Whose letter: the sender's face at its head, as a mail program has one.
  // Drawn of her own, with no network look (a picture fetched from outside
  // is read without telling its sender it was opened): the first letter of
  // the name, on the colour of the address, the same address always the
  // same one.
  function avatarHTML(l) {
    const from = (l.from || [])[0] || {}
    const name = String(from.name || from.address || '').trim()
    const first = Array.from(name)[0] || '?'
    let hash = 0
    for (const ch of String(from.address || name)) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0
    return `<span class="av" style="--av-h:${hash % 360}" aria-hidden="true">${esc(first.toLocaleUpperCase())}</span>`
  }

  // A letter open: back to the inbox, its star and the hand-off, who, when,
  // its attachments, the talk about it, the text or HTML.
  function readingHTML(note = '') {
    const r = core.reading
    const l = r.letter
    const talk = l ? talkOf(l.key) || r.talk : null
    // Beside the list (two panes) there is no inbox to go back to.
    const two = twoPanes()
    const back = two ? '' : `<span class="link" data-mail-back>‹ ${esc(T('mail.inbox'))}</span>`
    const bar = `<div class="mail-bar">${back}<span class="grow"></span>${l ? handButtons(l, talk) : ''}</div>`
    const hand = core.handNote && !two ? `<div class="note${core.handNote.bad ? ' err' : ''}">${esc(core.handNote.text)}</div>` : ''
    // Two panes: the bar stays, the letter (and the note about its hand-off)
    // scrolls under it. One pane: the whole page scrolls, as it did.
    const wrap = inner => (two ? `${bar}<div class="pane-r-body">${note}${inner}</div>` : `${bar}${hand}${inner}`)
    if (r.busy) return wrap(`<div class="grp"><div class="note">${esc(T('mail.opening'))}</div></div>`)
    if (r.error) return wrap(`<div class="grp"><div class="note err">${esc(handError(r.error))}</div></div>`)
    const files = l.attachments.length ? `<div class="m-files">${l.attachments.map(f => `<span class="chip">📎 ${esc(f.name || '?')} · ${mailSize(f.size)}</span>`).join('')}</div>` : ''
    // Whose (several accounts), and one's own, from Sent.
    const address = accountOf(l.account)?.address || ''
    const inSent = l.folder === 'sent' ? T('mail.inSent', { address }) : ''
    const whose = inSent || (core.snap.mail || []).length > 1 ? `<div class="d">${esc(inSent || T('mail.in', { address }))}</div>` : ''
    return wrap(`<div class="grp mail-read" ${where(l)}>
      <div class="m-subj">${esc(l.subject || T('mail.noSubject'))}</div>
      <div class="from">${avatarHTML(l)}<div class="grow"><div class="who">${esc(people(l.from))}</div><div class="d">${esc(mailDate(l.date, true))}</div></div></div>
      ${l.to.length ? `<div class="d">${esc(T('mail.to'))}: ${esc(people(l.to))}</div>` : ''}
      ${l.cc.length ? `<div class="d">${esc(T('mail.cc'))}: ${esc(people(l.cc))}</div>` : ''}
      ${whose}${files}${talk ? talkHTML(talk, l) : ''}
      ${mailBodyHTML(l)}
    </div>`)
  }

  // The letter's body: as it was written (its styles, its cards, its
  // pictures), in a frame that cannot run anything, or the plain text when
  // there is no HTML. The frame is sandboxed: no scripts, no forms, nothing
  // of hers in it. Its pictures are the letter's own (the sender's server),
  // so they are read as they come. The text it was written with is not put
  // under it: a letter shows as its sender made it (the user, 2026-10-20).
  function mailBodyHTML(l) {
    const html = String(l.html || '')
    if (!html.trim()) {
      return `<div class="m-text">${esc(l.text)}</div>${l.cut ? `<div class="d">${esc(T('mail.cut'))}</div>` : ''}`
    }
    const light = document.body.classList.contains('set-light')
    const fg = light ? '#1b1b1b' : '#f5f5f7'
    const link = light ? '#005fb8' : '#5e9eff'
    // no-referrer in the letter itself: the frame's referrerpolicy is for
    // its own request alone, and its pictures would tell their servers her
    // page's address.
    const doc = `<!doctype html><html><head><meta charset="utf-8"><meta name="referrer" content="no-referrer"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src http: https: data: cid:; style-src 'unsafe-inline'; font-src data:"><style>
      html,body{width:100%;max-width:100%;margin:0;padding:0;overflow-x:hidden;background:transparent;color:${fg};color-scheme:${light ? 'light' : 'dark'};font-family:system-ui,-apple-system,'Segoe UI',sans-serif;font-size:13px;line-height:1.5}
      *,*::before,*::after{box-sizing:border-box;max-width:100%!important;min-width:0!important}
      img,video,svg,canvas,iframe{height:auto!important}
      td,th,div,p,span,a,li{overflow-wrap:anywhere}
      a{color:${link}}
    </style></head><body>${html}</body></html>`
    return `<div class="m-html-wrap"><iframe class="m-html" sandbox="allow-same-origin" referrerpolicy="no-referrer" scrolling="no" title="${esc(l.subject || T('mail.noSubject'))}" srcdoc="${esc(doc)}"></iframe></div>`
  }

  // The frame as tall as the letter in it: the page measures it (the frame
  // is of her own origin and runs nothing) and gives it the height. Measured
  // again as its pictures arrive and its fonts settle — the first measure is
  // of a letter whose pictures are not there yet, and a letter 3,000px tall
  // was left showing its first 160 (the user's letter, 2026-10-20).
  function fitFrames() {
    for (const frame of body.querySelectorAll('iframe.m-html')) {
      const fitWidth = doc => {
        const w = doc.documentElement.clientWidth
        if (!w) return
        for (const el of doc.body.querySelectorAll('table,img,div')) {
          if (el.getBoundingClientRect().width <= w + 1) continue
          el.style.setProperty('max-width', '100%', 'important')
          el.style.setProperty(el.tagName === 'IMG' ? 'height' : 'width', el.tagName === 'IMG' ? 'auto' : '100%', 'important')
        }
      }
      const fit = () => {
        if (!frame.isConnected) return
        try {
          const doc = frame.contentDocument
          if (!doc || !doc.body) return
          fitWidth(doc)
          const rawH = Math.max(120, doc.body.scrollHeight + 8)
          const h = Math.min(32000, rawH)
          if (rawH > 32000) {
            frame.scrolling = 'yes'
            doc.documentElement.style.overflowY = 'auto'
          }
          if (frame.style.height !== `${h}px`) frame.style.height = `${h}px`
        } catch (e) {
          // The frame's document is not to be read (it is of another origin,
          // or it is not ready): the height it has is left as it is.
        }
      }
      const watch = () => {
        fit()
        try {
          const doc = frame.contentDocument
          if (!doc || frame._fitDoc === doc) return
          frame._fitDoc = doc
          doc.addEventListener('load', fit, true)
          doc.addEventListener('error', fit, true)
          doc.addEventListener('click', e => {
            const a = e.target.closest?.('a[href]')
            if (!a) return
            const href = a.getAttribute('href') || ''
            if (href.startsWith('#') && href.length > 1) {
              const target = doc.getElementById(href.slice(1))
              if (target) target.scrollIntoView({ block: 'nearest' })
            }
            e.preventDefault()
          })
          doc.fonts?.ready?.then(fit).catch(() => {})
        } catch (e) {}
      }
      frame._fit = watch
      if (!frame._fitHooked) {
        frame._fitHooked = true
        frame.addEventListener('load', () => frame._fit?.())
      }
      watch()
      for (const t of frame._fitTimers || []) clearTimeout(t)
      frame._fitTimers = [250, 900, 2200, 5000, 10000, 20000].map(ms => setTimeout(() => frame.isConnected && frame._fit?.(), ms))
    }
  }

  // Mail: each account with its switch and setting one up; the inbox
  // (every account's, or one), and a letter open or being written. In two
  // panes the inbox is on the left, and on the right the letter being
  // written, the one open, or (neither) the accounts and the agents; in one,
  // each takes the page.
  function pageMail() {
    const id = inboxId()
    if (twoPanes()) {
      if (id && inbox.for !== `${id}|${inbox.filter}` && !inbox.busy) setTimeout(() => loadInbox(false))
      const left = id ? inboxHTML(id) : `<div class="note">${esc(T('mail.noBox'))}</div>`
      const note = core.handNote ? `<div class="note${core.handNote.bad ? ' err' : ''}">${esc(core.handNote.text)}</div>` : ''
      const body = inner => `<div class="pane-r-body">${note}${inner}</div>`
      const right = core.writing && core.draft ? writeHTML(note) : core.reading ? readingHTML(note) : body(`${accountsHTML()}${id ? agentsHTML() : ''}`)
      return `<div class="panes"><div class="pane-l">${left}</div><div class="pane-r">${right}</div></div>`
    }
    if (core.writing && core.draft) return writeHTML()
    if (core.reading) return readingHTML()
    // The inbox comes in the first time it is shown (and for another one or filter).
    if (id && inbox.for !== `${id}|${inbox.filter}` && !inbox.busy) setTimeout(() => loadInbox(false))
    return `${accountsHTML()}${id ? inboxHTML(id) + agentsHTML() : ''}`
  }

  // The accounts: each with its switch and its settings; the form for one.
  function accountsHTML() {
    const accounts = core.snap.mail || []
    const rows = accounts
      .map(a => {
        const note = mailNote(a)
        return `<div class="r"><span class="wi" style="color:${a.on ? '#5e9bff' : COLOR.idle}">${Widgets.icon('mail')}</span><div class="grow"><div class="ellip">${esc(a.address)}</div><div class="d${note.bad ? ' bad' : ''}">${esc(note.text)}</div></div><button class="pbtn sm" data-mail-edit="${esc(a.id)}">${esc(T('pl.settings'))}</button>${sw('mail:' + a.id, a.on)}</div>`
      })
      .join('')
    const add = core.mailForm ? '' : `<div class="r"><button class="pbtn wide" data-mail-add>${esc(T('mail.add'))}</button></div>`
    return `${sec(T('mail.section'))}<div class="grp"><div class="note">${esc(T('mail.note'))}</div>${rows}${add}</div>
      ${accounts.length ? offlineHTML() + codesHTML() : ''}${proxyHTML()}${core.mailForm ? mailFormHTML() : ''}`
  }

  // How much is kept whole on this computer (the setting mailOffline: the
  // last year unless set), and the room every account's takes now.
  function offlineHTML() {
    const value = ['90d', 'year', 'all', 'opened'].includes(core.snap.settings.mailOffline) ? core.snap.settings.mailOffline : 'year'
    const used = (core.snap.mail || []).reduce((n, a) => n + (a.status?.offline?.bytes || 0), 0)
    const note = T(`mail.keep.${value}Note`) + (used ? ` ${T('mail.keep.used', { size: mailSize(used) })}` : '')
    const choices = ['90d', 'year', 'all', 'opened'].map(v => [v, T(`mail.keep.${v}`)])
    return `<div class="grp keep">${row(esc(T('mail.keep')), esc(note), seg('mailOffline', choices, value))}</div>`
  }

  // A code or a link to sign in with, in new mail, on the island (the
  // setting mailCodes, on unless turned off; mail/codes.rs).
  function codesHTML() {
    return `<div class="grp">${row(esc(T('mail.codes')), esc(T('mail.codesNote')), sw('mailCodes', core.snap.settings.mailCodes !== false))}</div>`
  }

  // The ways out, as set: the General page's Network proxy (netProxy,
  // proxy.rs: her updates, crash reports, plugins, LHM's download) and the
  // Mail page's (mailProxy, mail/proxy.rs): through Windows' proxy
  // (straight while none is set), through one typed in (HTTP or SOCKS5), or
  // straight; mail's may follow the General page's (its default since
  // zhoupengjie/Wakuwaku#68; one set before keeps its way). A manual one's
  // kind and address are kept while another is on.
  const WAY_MODES = { netProxy: ['system', 'manual', 'none'], mailProxy: ['follow', 'system', 'manual', 'none'] }
  function waySetting(key) {
    const p = core.snap.settings[key] || {}
    const modes = WAY_MODES[key]
    return { mode: modes.includes(p.mode) ? p.mode : modes[0], kind: p.kind === 'socks5' ? 'socks5' : 'http', address: p.address || '' }
  }

  // The Mail page's way as it goes: the General page's while it follows that.
  function proxySetting() {
    const p = waySetting('mailProxy')
    return p.mode === 'follow' ? waySetting('netProxy') : p
  }

  // The General page's way in a few words: "手动 · 127.0.0.1:7890（HTTP）",
  // "跟随系统 · Windows 没有设代理，直连".
  function netNow() {
    const p = waySetting('netProxy')
    const sys = core.snap.netSystemProxy
    const at = x => T('s.net.at', { proxy: x.address, kind: labelOf(PROXY_KINDS, x.kind) })
    const how = p.mode === 'manual' ? at(p) : p.mode === 'none' ? T('s.net.straight') : sys ? at(sys) : T('s.net.noSystem')
    return `${T(`s.net.mode.${p.mode}`)} · ${how}`
  }

  // A manual one's kind and address (its box: data-proxy-at, the setting's key); `extra` after the box.
  function manualRow(key, p, extra = '') {
    const typed = core.proxyTyped?.key === key ? core.proxyTyped.text : p.address
    const bad = core.proxyTyped?.key === key && core.proxyTyped.bad ? `<div class="note err">${esc(T('mail.proxy.bad'))}</div>` : ''
    return `<div class="r">${seg(`${key}.kind`, PROXY_KINDS, p.kind)}<input class="in grow" data-proxy-at="${key}" value="${esc(typed)}" placeholder="127.0.0.1:7890" spellcheck="false" autocomplete="off">${extra}</div>${bad}`
  }

  function proxyHTML() {
    const p = waySetting('mailProxy')
    const sys = core.snap.mailSystemProxy
    const now =
      p.mode === 'follow'
        ? T('mail.proxy.followNote', { what: netNow() })
        : p.mode === 'system'
          ? sys
            ? T('mail.proxy.systemNow', { proxy: sys.address, kind: labelOf(PROXY_KINDS, sys.kind) })
            : T('mail.proxy.systemNone')
          : T(`mail.proxy.${p.mode}Note`)
    const modes = WAY_MODES.mailProxy.map(m => [m, T(`mail.proxy.${m}`)])
    const manual = p.mode === 'manual' ? manualRow('mailProxy', p) : ''
    return `<div class="grp proxy">${row(esc(T('mail.proxy')), esc(`${now} ${T('mail.proxy.perAccount')}`), seg('mailProxy.mode', modes, p.mode))}${manual}</div>`
  }

  // The Network proxy's Test (proxy.rs test): running, or what it found.
  let netTest = null

  // The General page's Network proxy: its way, a manual one's kind and
  // address, the Test and what it found (Windows' way: on a row of its own).
  function pageNet() {
    const p = waySetting('netProxy')
    const sys = core.snap.netSystemProxy
    const note = p.mode === 'system' ? (sys ? T('s.net.systemNow', { proxy: sys.address, kind: labelOf(PROXY_KINDS, sys.kind) }) : T('s.net.systemNone')) : T(`s.net.${p.mode}Note`)
    const modes = WAY_MODES.netProxy.map(m => [m, T(`s.net.mode.${m}`)])
    const test = `<button class="pbtn sm" data-net-test ${netTest?.running ? 'disabled' : ''}>${esc(T('s.net.test'))}</button>`
    const found = netTestHTML(p)
    const more =
      p.mode === 'manual' ? manualRow('netProxy', p, test) + found : p.mode === 'system' ? `<div class="r"><div class="grow d">${esc(T('s.net.testNote'))}</div>${test}</div>${found}` : ''
    return `${sec(T('s.net'))}<div class="grp">${row(esc(T('s.netProxy')), esc(note), seg('netProxy.mode', modes, p.mode))}${more}</div>`
  }

  // What the Test found: GitHub reached (through which proxy, how fast), or
  // not (and a proxy heard on this computer at a usual port, to try).
  function netTestHTML(p) {
    const t = netTest
    if (!t) return ''
    if (t.running) return `<div class="note">${esc(T('s.net.testing'))}</div>`
    if (t.ok) return `<div class="note ok">${esc(T(t.via ? 's.net.okVia' : 's.net.okStraight', { proxy: t.via || '', s: ((t.ms || 0) / 1000).toFixed(1) }))}</div>`
    const heard = t.listening && !(p.mode === 'manual' && t.listening.address === p.address) ? ' ' + T('s.net.listening', { proxy: t.listening.address }) : ''
    return `<div class="note err">${esc(T(t.via ? 's.net.badVia' : 's.net.badStraight', { proxy: t.via || '', error: t.error || '' }) + heard)}</div>`
  }

  // A way changed, kept whole; manual for the first time: the address known
  // (the General page's typed one, for mail; Windows' proxy), else Clash's usual port, to change.
  function setProxy(key, change) {
    const next = { ...waySetting(key), ...change }
    if (next.mode === 'manual' && !next.address) {
      const net = waySetting('netProxy')
      const known = key === 'mailProxy' && net.address ? net : key === 'netProxy' ? core.snap.netSystemProxy : core.snap.mailSystemProxy
      Object.assign(next, known ? { kind: known.kind, address: known.address } : { address: '127.0.0.1:7890' })
    }
    if (core.proxyTyped?.key === key) core.proxyTyped = null
    if (key === 'netProxy') netTest = null
    patch({ [key]: next })
  }

  // An address as typed (Enter, or leaving the box): a scheme before it
  // (socks5://, http://) says the kind; kept once it reads as address:port.
  function proxyAddress(key, text) {
    let address = String(text || '').trim()
    let kind = waySetting(key).kind
    const scheme = /^([a-z0-9]+):\/\//i.exec(address)
    if (scheme) {
      kind = /^socks/i.test(scheme[1]) ? 'socks5' : 'http'
      address = address.slice(scheme[0].length).replace(/\/+$/, '')
    }
    const port = Number(address.split(':').pop())
    if (!/^(\[[0-9a-f:.]+\]|[a-z0-9._-]+):\d{1,5}$/i.test(address) || !(port > 0 && port < 65536)) {
      core.proxyTyped = { key, text, bad: true }
      return draw()
    }
    if (address === waySetting(key).address && kind === waySetting(key).kind) {
      core.proxyTyped = null
      return draw()
    }
    setProxy(key, { kind, address })
  }

  // The Test: GitHub asked the way the page has it now.
  function runNetTest() {
    netTest = { running: true }
    draw()
    const p = waySetting('netProxy')
    const typed = core.proxyTyped?.key === 'netProxy' && !core.proxyTyped.bad ? core.proxyTyped.text.trim() : p.address
    pet.settings
      .netTest(p.mode === 'manual' ? { ...p, address: typed } : { mode: p.mode })
      .then(got => ((netTest = got), draw()))
      .catch(e => ((netTest = { ok: false, error: String(e) }), draw()))
  }

  // The letters as the list shows them: a conversation folded in is the
  // letter a click on it opens; folded out, each of its letters.
  function shown() {
    return inbox.threads.flatMap(t => {
      if (t.count > 1 && expanded.has(t.key)) return t.letters.map(l => ({ l, t }))
      const l = t.letters.find(x => isLetter(x, t.account, t.open?.folder, t.open?.uid)) || t.letters.at(-1)
      return [{ l, t }]
    })
  }

  // The letter beside this one in the list (two panes, ↑ ↓), opened.
  function step(by) {
    const all = shown()
    const at = all.findIndex(({ l, t }) => isReading(l.account, l.folder, l.uid) || (!expanded.has(t.key) && t.letters.some(x => isReading(x.account, x.folder, x.uid))))
    const next = all[at < 0 ? (by > 0 ? 0 : all.length - 1) : at + by]?.l
    if (!next) return
    openLetter(next.account, next.uid, next.folder)
    requestAnimationFrame(() => body.querySelector(`.inbox [data-letter="${next.uid}"][data-folder="${next.folder}"][data-account="${CSS.escape(next.account)}"]`)?.scrollIntoView({ block: 'nearest' }))
  }

  // The inbox's conversations as kept, as filtered; with more, 50 more.
  // Asked again, quietly, whenever what is kept changed (snap.mailRev).
  async function loadInbox(more, quiet = false) {
    const id = inboxId()
    if (!id || inbox.busy) return
    const which = `${id}|${inbox.filter}`
    if (inbox.for !== which) Object.assign(inbox, { threads: [], total: 0, letters: 0, count: 50, error: null, errors: [], rev: 0 })
    if (more) inbox.count += 50
    const rev = core.snap?.mailRev || 0
    Object.assign(inbox, { id, for: which, busy: true, error: null })
    if (!quiet) redrawMail()
    const got = await pet.mail.letters(id, inbox.count, inbox.filter)
    inbox.busy = false
    if (inbox.for !== which) return redrawMail()
    if (got.ok) Object.assign(inbox, { threads: got.threads, total: got.total, letters: got.letters, errors: got.errors || [], syncing: got.syncing, rev })
    else inbox.error = got.error
    redrawMail()
    // It changed again meanwhile.
    if (got.ok && (core.snap?.mailRev || 0) !== rev) loadInbox(false, true)
  }

  // A letter opened: read whole, from what is kept (or the server), and
  // marked read.
  async function openLetter(account, uid, folder = 'inbox') {
    if (!accountOf(account)) return
    core.reading = { account, folder, uid, busy: true }
    draw(true)
    core.relayout()
    const got = await pet.mail.letter(account, uid, folder)
    if (!isReading(account, folder, uid)) return
    core.reading = got.ok ? { account, folder, uid, letter: got.letter, talk: got.talk } : { account, folder, uid, error: got.error }
    const l = listed(account, folder, uid)
    if (l && got.ok) Object.assign(l, { seen: true, flagged: got.letter.flagged })
    draw()
    core.relayout()
  }

  // Why a letter could not be had or handed on, in words.
  function handError(err, agent?) {
    const kind = err?.kind || 'other'
    const own = ['noAgent', 'start', 'save', 'noLetter', 'gone', 'busy', 'noTalk', 'noSession', 'empty']
    if (!own.includes(kind)) return mailError(err, accountOf(core.reading?.account))
    return T(`mail.err.${kind}`, { agent: AGENTS[agent] || err.text || '' }) + (kind === 'start' || kind === 'save' ? `${core.lang === 'zh' ? '：' : ': '}${err.text}` : '')
  }

  function noteFor(note) {
    clearTimeout(core.handNoteTimer)
    core.handNote = note
    redrawMail()
    if (note) core.handNoteTimer = setTimeout(() => noteFor(null), 10000)
  }

  // A letter handed to an agent: a talk about it under the letter (opened,
  // if it was not), or a session in a terminal.
  async function hand(account, uid, agent, how, folder = 'inbox') {
    closeMenu()
    if (how === 'talk' && !isReading(account, folder, uid)) await openLetter(account, uid, folder)
    noteFor({ text: T('mail.hand.busy', { agent: AGENTS[agent] }) })
    const got = await pet.mail.hand(account, uid, agent, how, folder)
    if (got.ok && how === 'talk' && core.reading?.letter && isReading(account, folder, uid)) core.reading.talk = null
    noteFor(got.ok ? (how === 'open' ? { text: T('mail.hand.opened', { agent: AGENTS[agent] }) } : null) : { text: handError(got.error, agent), bad: true })
  }

  // More asked in a letter's talk.
  async function say(box) {
    const text = box.value.trim()
    if (!text) return
    box.value = ''
    const got = await pet.mail.say(box.dataset.talk, text)
    if (!got.ok) {
      box.value = text
      noteFor({ text: handError(got.error), bad: true })
    }
  }

  // A star put on a letter or taken off: at once on the page, then on the
  // server (back as it was if that failed).
  async function star(account, uid, folder = 'inbox') {
    const inList = listed(account, folder, uid)
    const open = core.reading?.letter && isReading(account, folder, uid) ? core.reading.letter : null
    const on = !(inList || open)?.flagged
    for (const l of [inList, open]) if (l) l.flagged = on
    redrawMail()
    const got = await pet.mail.flag(account, uid, on, folder)
    if (!got.ok) {
      for (const l of [inList, open]) if (l) l.flagged = !on
      noteFor({ text: mailError(got.error, accountOf(account)), bad: true })
    }
  }

  // The menu a right-click on a letter opens: mail's AI (a talk, or a
  // terminal), greyed out when not found; its star.
  const menuEl = document.createElement('div')
  menuEl.className = 'mmenu'
  menuEl.hidden = true
  layer.append(menuEl)

  function openMenu(account, uid, folder, x, y) {
    const found = core.snap.mailAgents || {}
    const at = `data-account="${esc(account)}" data-folder="${folder || 'inbox'}" data-uid="${uid}"`
    const item = (ag, how, label) =>
      `<button class="mi" data-hand="${ag}:${how}" ${at} ${found[ag] ? '' : 'disabled'}>${esc(label)}${found[ag] ? '' : ` <span class="d">${esc(T('mail.hand.missing'))}</span>`}</button>`
    const ag = firstAgent()
    const talks = item(ag, 'talk', T('mail.hand.talk', { agent: AGENTS[ag] }))
    const terminals = item(ag, 'open', T('mail.hand.open', { agent: AGENTS[ag] }))
    const l = listed(account, folder, uid) || (isReading(account, folder, uid) ? core.reading.letter : null)
    const starItem = `<button class="mi" data-star="${esc(account)}" data-folder="${folder || 'inbox'}" data-uid="${uid}">${esc(T(l?.flagged ? 'mail.unstar' : 'mail.star'))}</button>`
    const replies = [
      ['reply', 'mail.reply'],
      ['all', 'mail.replyAll'],
      ['forward', 'mail.forward'],
    ]
      .map(([kind, key]) => `<button class="mi" data-w-reply="${kind}" ${at}>${esc(T(key))}</button>`)
      .join('')
    menuEl.innerHTML = `${replies}<div class="mi-sep"></div>${talks}<div class="mi-sep"></div>${terminals}<div class="mi-sep"></div>${starItem}`
    core.menuPick = null
    placeMenu(x, y)
  }

  function placeMenu(x, y) {
    menuEl.hidden = false
    const box = layer.getBoundingClientRect()
    menuEl.style.left = `${Math.max(4, Math.min(x - box.left, box.width - menuEl.offsetWidth - 4))}px`
    menuEl.style.top = `${Math.max(4, Math.min(y - box.top, box.height - menuEl.offsetHeight - 4))}px`
  }

  // A menu of [value, label, off] under a button, the first (the agent's or
  // server's own, autodetect, every account) apart unless `together`; one
  // that is off is there, greyed, with why (off's words, else "not
  // supported").
  function openChoices(button, choices, value, onPick, together = false) {
    menuEl.innerHTML = choices
      .map(
        ([v, label, off], i) =>
          `${i === 1 && !together ? '<div class="mi-sep"></div>' : ''}<button class="mi${v === value ? ' on' : ''}" data-pick="${esc(v)}" ${off ? 'disabled' : ''}>${esc(label)}${off ? ` <span class="d">${esc(typeof off === 'string' ? off : T('mail.unsupported'))}</span>` : ''}</button>`,
      )
      .join('')
    core.menuPick = onPick
    const r = button.getBoundingClientRect()
    placeMenu(r.left, r.bottom + 4)
  }

  // The split button's ▾ on an open letter: the other agents found, each
  // to hand this letter to (the first agent stays as it is), and under
  // them each made the first instead.
  function handMenu(button) {
    const ag = firstAgent()
    const others = mailList().filter(a => a.found && a.id !== ag)
    menuEl.innerHTML = `${others.map(a => `<button class="mi" data-pick="hand:${a.id}">${esc(T('mail.hand.talk', { agent: a.short }))}</button>`).join('')}<div class="mi-sep"></div>${others
      .map(a => `<button class="mi" data-pick="first:${a.id}">${esc(T('mail.hand.makeFirst', { agent: a.short }))}</button>`)
      .join('')}`
    core.menuPick = v => {
      const [what, id] = v.split(':')
      if (what === 'first') return patch({ mailAgent: id })
      if (core.reading) hand(core.reading.account, Number(core.reading.uid), id, 'talk', core.reading.folder || 'inbox')
    }
    const r = button.getBoundingClientRect()
    placeMenu(r.right - 220, r.bottom + 4)
  }

  function openPicker(button) {
    const [ag, key] = button.dataset.picker.split('.')
    openChoices(button, choicesOf(ag, key), confOf(ag)[key] || '', v => setConf(ag, key, v))
  }

  function closeMenu() {
    menuEl.hidden = true
    core.menuPick = null
  }

  layer.addEventListener('contextmenu', e => {
    const letter = core.tab === 'mail' && e.target.closest('[data-letter]')
    if (!letter || e.target.closest('.talk')) return
    // Ours, not the island's menu.
    e.preventDefault()
    e.stopPropagation()
    openMenu(letter.dataset.account, Number(letter.dataset.letter), letter.dataset.folder || 'inbox', e.clientX, e.clientY)
  })
  body.addEventListener('scroll', closeMenu)

  return { addFiles, closeMenu, fitFrames, hand, handMenu, loadInbox, menuEl, netNow, openChoices, openLetter, openMenu, openPicker, pageMail, pageNet, proxyAddress, proxySetting, replyIn, runNetTest, say, sendDraft, setProxy, star, step, waySetting, write, writeAbout }
}
