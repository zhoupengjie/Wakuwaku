// Writing a letter (a new one, a reply, a forward) and sending it; a letter
// read; the Mail page drawn, with its accounts and the codes come by mail;
// the Network page (the proxy, how she reaches the web, its test); and the
// menus a letter and a button open.
// Made by settings.ts in the section's place as the settings are made
// (letterPart(core), imported by settings.ts). The settings' core (SettingsCore):
// their constants and functions as they are, what they remember in core.st
// (settings-vars.ts, read and written as st.<name>), and the parts made
// before this one in core.pieces (a later one's reached once the settings
// are up). What it hands back is the settings' again.
import * as Widgets from './widgets.ts'
import { pet } from './bridge.ts'
import type { SettingsCore } from './settings.ts'
import type { Choices } from './settings-mail.ts'
import type { Draft, DraftBox, Letter, Note } from './settings-vars.ts'

// Someone a letter is from or to: a name, and the address.
interface Person {
  name?: string
  address: string
}
// A way out (proxy.rs, mail/proxy.rs): the General page's Network proxy,
// or the Mail page's.
export type Way = 'netProxy' | 'mailProxy'
// What the Network proxy's Test found (proxy.rs test): running, or GitHub
// reached (through which proxy, how fast) or not (why; a proxy heard on
// this computer at a usual port).
interface NetTest {
  running?: boolean
  ok?: boolean
  via?: string
  ms?: number
  error?: string
  listening?: { address: string } | null
}
// A letter's frame (mailBodyHTML), with what fitFrames keeps on it: the
// page it watches, its measure, whether it hears its loads, its timers.
type LetterFrame = HTMLIFrameElement & { _fitDoc?: Document; _fit?: () => void; _fitHooked?: boolean; _fitTimers?: Timer[] }

// Whether a letter's HTML (as letters.rs leaves it) asks for something from
// elsewhere: a picture's address, a background, a url() in its styles
// (http, https, or // which is http in her letter page). One too many is no
// harm: the bar to show its pictures is there for nothing.
const REMOTE = /(?:\b(?:src|srcset|background|poster)\s*=\s*["']?|url\(\s*(?:["']|&quot;|&#39;)?)\s*(?:https?:)?\/\//i
export const hasRemote = (html: string) => REMOTE.test(html)

export function letterPart(core: SettingsCore) {
  const {
    st, pieces, AGENTS, COLOR, MOST_FILES, T, Widgets, body, draw, esc, expanded, inbox, keepDraft, layer,
    patch, redrawMail, row, sec, seg, sw,
  } = core
  const {
    PROXY_KINDS, accountOf, agentsHTML, choicesOf, confOf, firstAgent, handButtons, inboxHTML, inboxId,
    isLetter, isReading, labelOf, listed, loginWhy, mailDate, mailError, mailFormHTML, mailList, mailNote,
    mailSize, people, providerOf, setConf, talkHTML, talkOf, twoPanes, where,
  } = pieces.mail

  const personLine = (p: Person) => (p.name ? `${/[,;"<>]/.test(p.name) ? `"${p.name.replace(/"/g, '')}"` : p.name} <${p.address}>` : p.address)
  const peopleLine = (list: Person[]) => list.map(personLine).join(', ')
  const samePerson = (a: Person, b: Person) => a.address.toLowerCase() === b.address.toLowerCase()
  const eachOnce = (list: Person[]) => list.filter((p, i) => p.address && list.findIndex(q => samePerson(p, q)) === i)
  const quoted = (text: unknown) => String(text || '').split('\n').map(line => (line ? `> ${line}` : '>')).join('\n')

  // The letter passed on, as mail programs write it under the new one.
  function forwarded(l: Letter) {
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
    const all = st.snap.mail || []
    const shown = accountOf(inboxId())
    if (shown?.on) return shown.id
    return (all.find(a => a.on && a.smtp) || all.find(a => a.on) || all[0])?.id
  }

  // A letter begun: new (from the account fromAccount picks), a reply to
  // the sender or to all (from the account it came to; the letter quoted
  // under it, words an agent wrote on top, if given), or one passed on
  // (with its attachments). One half written is not thrown away for it:
  // it shows.
  function write(kind: string, l?: Letter, words?: string) {
    closeMenu()
    if (st.draft && (st.draft.touched || st.draft.files.length)) {
      st.draft.note = T('mail.w.unsent')
      st.writing = true
      return redrawMail()
    }
    const account = accountOf(l?.account)?.id || fromAccount()
    if (!account) return
    const me = (accountOf(account)?.address || '').toLowerCase()
    const notMe = (list: Person[] | null | undefined) => (list || []).filter(p => p.address && p.address.toLowerCase() !== me)
    const d: Draft = { kind, account, to: '', cc: '', bcc: '', showCc: false, subject: '', text: '', reply: null, forward: null, files: [], touched: !!words, busy: false, error: null, note: null }
    if (l && (kind === 'reply' || kind === 'all')) {
      // To whom it asks replies to go, else who wrote it; one's own letter
      // (in Sent), to whom it went.
      const mine = (l.from || []).some((p: Person) => (p.address || '').toLowerCase() === me)
      const back: Person[] = l.replyTo?.length ? l.replyTo : mine && l.to?.length ? l.to : l.from
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
      d.forward = { account: l.account, folder: l.folder || 'inbox', uid: l.uid, files: (l.attachments || []).map((f: { name: string; size: number }, i: number) => ({ index: i, name: f.name, size: f.size })) }
    }
    st.draft = d
    st.writing = true
    st.dropSure = false
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
  async function writeAbout(kind: string, account: string, uid: number, folder: string, words?: string) {
    if (!(st.reading?.letter && isReading(account, folder, uid))) await openLetter(account, uid, folder)
    if (st.reading?.letter && isReading(account, folder, uid)) write(kind, st.reading.letter, words)
  }

  // The reply in an agent's words: the longest ``` block, if it wrote one; else all of it.
  function replyIn(text: unknown) {
    const blocks = [...String(text || '').matchAll(/```[^\n]*\n([\s\S]*?)```/g)].map(m => m[1].trim())
    return (blocks.sort((a, b) => b.length - a.length)[0] || String(text || '')).trim()
  }

  // The letter being written: back (it stays), discard, send; from
  // (where there are several accounts), to, cc and bcc (when asked for),
  // the subject, the text; the files, each with ×, and more.
  // Drawn only while there is one (pageMail).
  function writeHTML(note = '') {
    const d = st.draft!
    const all = st.snap.mail || []
    const off = d.busy ? 'disabled' : ''
    const two = twoPanes()
    // Back: to the letter answered, or the inbox; beside the list (two
    // panes), put aside.
    const back = two ? 'mail.w.aside' : d.reply || d.forward ? (st.reading?.letter ? 'mail.w.backLetter' : 'mail.inbox') : 'mail.inbox'
    const bar = `<div class="mail-bar"><span class="link" data-w-back>‹ ${esc(T(back))}</span><span class="grow"></span>
      <button class="pbtn sm" data-w-drop ${off}>${esc(T(st.dropSure ? 'mail.w.dropSure' : 'mail.w.drop'))}</button>
      <button class="pbtn sm al" data-w-send title="${esc(T('mail.w.ctrlEnter'))}" ${off}>${esc(T(d.busy ? 'mail.w.sending' : 'mail.w.send'))}</button></div>`
    const box = (key: DraftBox, label: string, extra = '') =>
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
  const readBase64 = (file: File) =>
    new Promise<string>((ok, no) => {
      const r = new FileReader()
      r.onload = () => ok(String(r.result).split(',')[1] || '')
      r.onerror = () => no(r.error)
      r.readAsDataURL(file)
    })

  async function addFiles(input: HTMLInputElement) {
    st.picking = false
    const d = st.draft
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
  // got: what send.rs answered (its error, whether the outgoing server's).
  function sendError(got: { error?: any; out?: boolean }) {
    const err = got.error || {}
    const own = ['address', 'noOne', 'tooMany', 'tooBig', 'tooLong', 'file', 'noSmtp', 'gone', 'noLetter']
    if (own.includes(err.kind)) return T(`mail.err.${err.kind}`, { what: err.text || '' })
    const a = accountOf(st.draft?.account)
    if (!got.out) return mailError(err, a)
    const hint = err.kind === 'login' && !loginWhy(err) && providerOf(a?.host) ? ` ${T(`mail.hint.${providerOf(a!.host)}`)}` : ''
    return T('mail.w.outPrefix') + mailError(err, a?.smtp ? { ...a.smtp, proxy: a.proxy } : { host: '?', port: '' }) + hint
  }

  // Sent: the letter gone from here, back where one was, and what came of
  // the copy in Sent.
  async function sendDraft() {
    const d = st.draft
    if (!d || d.busy) return
    if (!`${d.to}${d.cc}${d.bcc}`.trim()) {
      d.error = T('mail.err.noOne')
      return redrawMail('[data-w="to"]')
    }
    Object.assign(d, { busy: true, error: null, note: null })
    st.dropSure = false
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
    if (st.draft !== d) return
    d.busy = false
    if (!got.ok) {
      d.error = sendError(got)
      return redrawMail()
    }
    // The letter answered shows it (the list comes again with the reply in it).
    const answered = d.reply && listed(d.reply.account, d.reply.folder, d.reply.uid)
    if (answered) answered.answered = true
    st.draft = null
    st.writing = false
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
  function avatarHTML(l: Letter) {
    const from = (l.from || [])[0] || {}
    const name = String(from.name || from.address || '').trim()
    const first = Array.from(name)[0] || '?'
    let hash = 0
    for (const ch of String(from.address || name)) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0
    return `<span class="av" style="--av-h:${hash % 360}" aria-hidden="true">${esc(first.toLocaleUpperCase())}</span>`
  }

  // A letter open: back to the inbox, its star and the hand-off, who, when,
  // its attachments, the talk about it, the text or HTML.
  // Drawn only while one is open (pageMail).
  function readingHTML(note = '') {
    const r = st.reading!
    const l = r.letter
    const talk = l ? talkOf(l.key) || r.talk : null
    // Beside the list (two panes) there is no inbox to go back to.
    const two = twoPanes()
    const back = two ? '' : `<span class="link" data-mail-back>‹ ${esc(T('mail.inbox'))}</span>`
    const bar = `<div class="mail-bar">${back}<span class="grow"></span>${l ? handButtons(l, talk) : ''}</div>`
    const hand = st.handNote && !two ? `<div class="note${st.handNote.bad ? ' err' : ''}">${esc(st.handNote.text)}</div>` : ''
    // Two panes: the bar stays, the letter (and the note about its hand-off)
    // scrolls under it. One pane: the whole page scrolls, as it did.
    const wrap = (inner: string) => (two ? `${bar}<div class="pane-r-body">${note}${inner}</div>` : `${bar}${hand}${inner}`)
    if (r.busy) return wrap(`<div class="grp"><div class="note">${esc(T('mail.opening'))}</div></div>`)
    if (r.error) return wrap(`<div class="grp"><div class="note err">${esc(handError(r.error))}</div></div>`)
    const files = l.attachments.length ? `<div class="m-files">${l.attachments.map((f: { name?: string; size: number }) => `<span class="chip">📎 ${esc(f.name || '?')} · ${mailSize(f.size)}</span>`).join('')}</div>` : ''
    // Whose (several accounts), and one's own, from Sent.
    const address = accountOf(l.account)?.address || ''
    const inSent = l.folder === 'sent' ? T('mail.inSent', { address }) : ''
    const whose = inSent || (st.snap.mail || []).length > 1 ? `<div class="d">${esc(inSent || T('mail.in', { address }))}</div>` : ''
    return wrap(`<div class="grp mail-read" ${where(l)}>
      <div class="m-subj">${esc(l.subject || T('mail.noSubject'))}</div>
      <div class="from">${avatarHTML(l)}<div class="grow"><div class="who">${esc(people(l.from))}</div><div class="d">${esc(mailDate(l.date, true))}</div></div></div>
      ${l.to.length ? `<div class="d">${esc(T('mail.to'))}: ${esc(people(l.to))}</div>` : ''}
      ${l.cc.length ? `<div class="d">${esc(T('mail.cc'))}: ${esc(people(l.cc))}</div>` : ''}
      ${whose}${files}${talk ? talkHTML(talk, l) : ''}
      ${mailBodyHTML(l)}
    </div>`)
  }

  // The letters' HTML as their frames are to show it, by the frame's
  // data-doc (a letter, in the light look or the dark, with its pictures or
  // not): put here by mailBodyHTML, into the frame by fillFrame. The last
  // few alone.
  const letterDocs = new Map<string, string>()

  // A letter's pictures from elsewhere (its sender's server): none until
  // asked for, as Thunderbird has it, since one asked for tells the sender
  // the letter was opened, when and from where (a tracking pixel). Asked
  // for this letter (显示图片, until the page goes), or for its sender
  // always (the setting mailPictures, their addresses in lower case).
  const picturesOnce = new Set<string>()
  const letterKey = (l: Letter) => `${l.account}|${l.folder || 'inbox'}|${l.uid}`
  const senderOf = (l: Letter) => String(l.from?.[0]?.address || '').trim().toLowerCase()
  const picturesFrom = (): string[] => (Array.isArray(st.snap.settings?.mailPictures) ? st.snap.settings.mailPictures : [])
  const isSenderShown = (l: Letter) => !!senderOf(l) && picturesFrom().includes(senderOf(l))
  const isPicturesShown = (l: Letter) => picturesOnce.has(letterKey(l)) || isSenderShown(l)

  // 显示图片 for this letter, for its sender always, or not for its sender
  // any more (the letter back to none at once).
  function setPictures(how: string) {
    const l = st.reading?.letter
    if (!l) return
    const key = letterKey(l)
    const from = senderOf(l)
    closeMenu()
    if (how === 'once') picturesOnce.add(key)
    if (how === 'always' && from) {
      picturesOnce.add(key)
      if (!picturesFrom().includes(from)) patch({ mailPictures: [...picturesFrom(), from] })
    }
    if (how === 'never') {
      picturesOnce.delete(key)
      if (from) patch({ mailPictures: picturesFrom().filter(a => a !== from) })
    }
    redrawMail()
  }

  // The letter's body: as it was written (its styles, its cards, its
  // pictures), in a frame that cannot run anything, or the plain text when
  // there is no HTML. The frame is sandboxed: no scripts, no forms, nothing
  // of hers in it. Its pictures are the letter's own (the sender's server),
  // so they are read as they come. The text it was written with is not put
  // under it: a letter shows as its sender made it (the user, 2026-10-20).
  // The frame shows letter.html, a page of her own with a policy of its own,
  // or letter-pictures.html, the same with the pictures from elsewhere let
  // in, and the letter is put into it once it is there (fillFrame): a
  // srcdoc was kept from loading by the island's frame guard and would have
  // kept the island's policy (zhoupengjie/Wakuwaku#129). Pictures it has
  // from elsewhere and not shown: a bar over it to show them.
  function mailBodyHTML(l: Letter) {
    const html = String(l.html || '')
    if (!html.trim()) {
      return `<div class="m-text">${esc(l.text)}</div>${l.cut ? `<div class="d">${esc(T('mail.cut'))}</div>` : ''}`
    }
    const light = document.body.classList.contains('set-light')
    const fg = light ? '#1b1b1b' : '#f5f5f7'
    const link = light ? '#005fb8' : '#5e9eff'
    const doc = `<!doctype html><html><head><style>
      html,body{width:100%;max-width:100%;margin:0;padding:0;overflow-x:hidden;background:transparent;color:${fg};color-scheme:${light ? 'light' : 'dark'};font-family:system-ui,-apple-system,'Segoe UI',sans-serif;font-size:13px;line-height:1.5}
      *,*::before,*::after{box-sizing:border-box;max-width:100%!important;min-width:0!important}
      img,video,svg,canvas,iframe{height:auto!important}
      td,th,div,p,span,a,li{overflow-wrap:anywhere}
      a{color:${link}}
    </style></head><body>${html}</body></html>`
    const remote = hasRemote(html)
    const shown = remote && isPicturesShown(l)
    const id = `${letterKey(l)}|${light ? 'light' : 'dark'}|${shown ? 'pictures' : 'none'}`
    letterDocs.delete(id)
    letterDocs.set(id, doc)
    for (const old of letterDocs.keys()) {
      if (letterDocs.size <= 8) break
      letterDocs.delete(old)
    }
    const from = senderOf(l)
    const bar =
      remote && !shown
        ? `<div class="m-pics"><span class="grow">${esc(T('mail.pics.blocked'))}</span><button class="pbtn sm" data-pics="once">${esc(T('mail.pics.show'))}</button>${
            from ? `<button class="pbtn sm" data-pics="always" title="${esc(from)}">${esc(T('mail.pics.always'))}</button>` : ''
          }</div>`
        : ''
    const page = shown ? 'letter-pictures.html' : 'letter.html'
    return `${bar}<div class="m-html-wrap"><iframe class="m-html" src="${page}" data-doc="${esc(id)}" sandbox="allow-same-origin" referrerpolicy="no-referrer" scrolling="no" title="${esc(l.subject || T('mail.noSubject'))}"></iframe></div>`
  }

  // The letter put into its frame's page once letter.html is there (not
  // the empty page a frame has first), and again for another letter or the
  // other look; marked on the page itself, so a frame that loads it again
  // gets the letter again. Its styles go to the page's head, its body for
  // the page's. DOMParser reads it here without fetching or running
  // anything, so its pictures are first asked for in the letter's page,
  // under that page's policy and with no referrer.
  function fillFrame(frame: HTMLIFrameElement, doc: Document) {
    const id = frame.dataset.doc
    const page = doc.documentElement
    if (!id || !page.hasAttribute('data-letter') || page.getAttribute('data-filled') === id) return
    const letter = letterDocs.get(id)
    if (letter == null) return
    const got = new DOMParser().parseFromString(letter, 'text/html')
    for (const s of doc.head.querySelectorAll('style')) s.remove()
    for (const s of got.head.querySelectorAll('style')) doc.head.append(doc.importNode(s, true))
    doc.body.replaceWith(doc.importNode(got.body, true))
    page.setAttribute('data-filled', id)
  }

  // The frame as tall as the letter in it: the page measures it (the frame
  // is of her own origin and runs nothing) and gives it the height. Measured
  // again as its pictures arrive and its fonts settle — the first measure is
  // of a letter whose pictures are not there yet, and a letter 3,000px tall
  // was left showing its first 160 (the user's letter, 2026-10-20).
  function fitFrames() {
    for (const frame of body.querySelectorAll<LetterFrame>('iframe.m-html')) {
      const fitWidth = (doc: Document) => {
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
        try {
          const doc = frame.contentDocument
          if (doc) fillFrame(frame, doc)
        } catch (e) {}
        fit()
        try {
          const doc = frame.contentDocument
          if (!doc || frame._fitDoc === doc) return
          frame._fitDoc = doc
          doc.addEventListener('load', fit, true)
          doc.addEventListener('error', fit, true)
          doc.addEventListener('click', e => {
            const a = (e.target as Element).closest?.('a[href]')
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
      const note = st.handNote ? `<div class="note${st.handNote.bad ? ' err' : ''}">${esc(st.handNote.text)}</div>` : ''
      const body = (inner: string) => `<div class="pane-r-body">${note}${inner}</div>`
      const right = st.writing && st.draft ? writeHTML(note) : st.reading ? readingHTML(note) : body(`${accountsHTML()}${id ? agentsHTML() : ''}`)
      return `<div class="panes"><div class="pane-l">${left}</div><div class="pane-r">${right}</div></div>`
    }
    if (st.writing && st.draft) return writeHTML()
    if (st.reading) return readingHTML()
    // The inbox comes in the first time it is shown (and for another one or filter).
    if (id && inbox.for !== `${id}|${inbox.filter}` && !inbox.busy) setTimeout(() => loadInbox(false))
    return `${accountsHTML()}${id ? inboxHTML(id) + agentsHTML() : ''}`
  }

  // The accounts: each with its switch and its settings; the form for one.
  function accountsHTML() {
    const accounts = st.snap.mail || []
    const rows = accounts
      .map(a => {
        const note = mailNote(a)
        return `<div class="r"><span class="wi" style="color:${a.on ? '#5e9bff' : COLOR.idle}">${Widgets.icon('mail')}</span><div class="grow"><div class="ellip">${esc(a.address)}</div><div class="d${note.bad ? ' bad' : ''}">${esc(note.text)}</div></div><button class="pbtn sm" data-mail-edit="${esc(a.id)}">${esc(T('pl.settings'))}</button>${sw('mail:' + a.id, a.on)}</div>`
      })
      .join('')
    const add = st.mailForm ? '' : `<div class="r"><button class="pbtn wide" data-mail-add>${esc(T('mail.add'))}</button></div>`
    return `${sec(T('mail.section'))}<div class="grp"><div class="note">${esc(T('mail.note'))}</div>${rows}${add}</div>
      ${accounts.length ? offlineHTML() + codesHTML() : ''}${proxyHTML()}${st.mailForm ? mailFormHTML() : ''}`
  }

  // How much is kept whole on this computer (the setting mailOffline: the
  // last year unless set), and the room every account's takes now.
  function offlineHTML() {
    const value = ['90d', 'year', 'all', 'opened'].includes(st.snap.settings.mailOffline) ? st.snap.settings.mailOffline : 'year'
    const used = (st.snap.mail || []).reduce((n, a) => n + (a.status?.offline?.bytes || 0), 0)
    const note = T(`mail.keep.${value}Note`) + (used ? ` ${T('mail.keep.used', { size: mailSize(used) })}` : '')
    const choices = ['90d', 'year', 'all', 'opened'].map(v => [v, T(`mail.keep.${v}`)])
    return `<div class="grp keep">${row(esc(T('mail.keep')), esc(note), seg('mailOffline', choices, value))}</div>`
  }

  // A code or a link to sign in with, in new mail, on the island (the
  // setting mailCodes, on unless turned off; mail/codes.rs).
  function codesHTML() {
    return `<div class="grp">${row(esc(T('mail.codes')), esc(T('mail.codesNote')), sw('mailCodes', st.snap.settings.mailCodes !== false))}</div>`
  }

  // The ways out, as set: the General page's Network proxy (netProxy,
  // proxy.rs: her updates, crash reports, plugins, LHM's download) and the
  // Mail page's (mailProxy, mail/proxy.rs): through Windows' proxy
  // (straight while none is set), through one typed in (HTTP or SOCKS5), or
  // straight; mail's may follow the General page's (its default since
  // zhoupengjie/Wakuwaku#68; one set before keeps its way). A manual one's
  // kind and address are kept while another is on.
  const WAY_MODES: Record<Way, string[]> = { netProxy: ['system', 'manual', 'none'], mailProxy: ['follow', 'system', 'manual', 'none'] }
  function waySetting(key: Way) {
    const p = st.snap.settings[key] || {}
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
    const sys = st.snap.netSystemProxy
    const at = (x: { address: string; kind: string }) => T('s.net.at', { proxy: x.address, kind: labelOf(PROXY_KINDS, x.kind) })
    const how = p.mode === 'manual' ? at(p) : p.mode === 'none' ? T('s.net.straight') : sys ? at(sys) : T('s.net.noSystem')
    return `${T(`s.net.mode.${p.mode}`)} · ${how}`
  }

  // A manual one's kind and address (its box: data-proxy-at, the setting's key); `extra` after the box.
  function manualRow(key: Way, p: ReturnType<typeof waySetting>, extra = '') {
    const typed = st.proxyTyped?.key === key ? st.proxyTyped.text : p.address
    const bad = st.proxyTyped?.key === key && st.proxyTyped.bad ? `<div class="note err">${esc(T('mail.proxy.bad'))}</div>` : ''
    return `<div class="r">${seg(`${key}.kind`, PROXY_KINDS, p.kind)}<input class="in grow" data-proxy-at="${key}" value="${esc(typed)}" placeholder="127.0.0.1:7890" spellcheck="false" autocomplete="off">${extra}</div>${bad}`
  }

  function proxyHTML() {
    const p = waySetting('mailProxy')
    const sys = st.snap.mailSystemProxy
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
  let netTest: NetTest | null = null

  // The General page's Network proxy: its way, a manual one's kind and
  // address, the Test and what it found (Windows' way: on a row of its own).
  function pageNet() {
    const p = waySetting('netProxy')
    const sys = st.snap.netSystemProxy
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
  function netTestHTML(p: ReturnType<typeof waySetting>) {
    const t = netTest
    if (!t) return ''
    if (t.running) return `<div class="note">${esc(T('s.net.testing'))}</div>`
    if (t.ok) return `<div class="note ok">${esc(T(t.via ? 's.net.okVia' : 's.net.okStraight', { proxy: t.via || '', s: ((t.ms || 0) / 1000).toFixed(1) }))}</div>`
    const heard = t.listening && !(p.mode === 'manual' && t.listening.address === p.address) ? ' ' + T('s.net.listening', { proxy: t.listening.address }) : ''
    return `<div class="note err">${esc(T(t.via ? 's.net.badVia' : 's.net.badStraight', { proxy: t.via || '', error: t.error || '' }) + heard)}</div>`
  }

  // A way changed, kept whole; manual for the first time: the address known
  // (the General page's typed one, for mail; Windows' proxy), else Clash's usual port, to change.
  function setProxy(key: Way, change: { mode?: string; kind?: string; address?: string }) {
    const next = { ...waySetting(key), ...change }
    if (next.mode === 'manual' && !next.address) {
      const net = waySetting('netProxy')
      const known = key === 'mailProxy' && net.address ? net : key === 'netProxy' ? st.snap.netSystemProxy : st.snap.mailSystemProxy
      Object.assign(next, known ? { kind: known.kind, address: known.address } : { address: '127.0.0.1:7890' })
    }
    if (st.proxyTyped?.key === key) st.proxyTyped = null
    if (key === 'netProxy') netTest = null
    patch({ [key]: next })
  }

  // An address as typed (Enter, or leaving the box): a scheme before it
  // (socks5://, http://) says the kind; kept once it reads as address:port.
  function proxyAddress(key: Way, text: string) {
    let address = String(text || '').trim()
    let kind = waySetting(key).kind
    const scheme = /^([a-z0-9]+):\/\//i.exec(address)
    if (scheme) {
      kind = /^socks/i.test(scheme[1]) ? 'socks5' : 'http'
      address = address.slice(scheme[0].length).replace(/\/+$/, '')
    }
    const port = Number(address.split(':').pop())
    if (!/^(\[[0-9a-f:.]+\]|[a-z0-9._-]+):\d{1,5}$/i.test(address) || !(port > 0 && port < 65536)) {
      st.proxyTyped = { key, text, bad: true }
      return draw()
    }
    if (address === waySetting(key).address && kind === waySetting(key).kind) {
      st.proxyTyped = null
      return draw()
    }
    setProxy(key, { kind, address })
  }

  // The Test: GitHub asked the way the page has it now.
  function runNetTest() {
    netTest = { running: true }
    draw()
    const p = waySetting('netProxy')
    const typed = st.proxyTyped?.key === 'netProxy' && !st.proxyTyped.bad ? st.proxyTyped.text.trim() : p.address
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
  function step(by: number) {
    const all = shown()
    const at = all.findIndex(({ l, t }) => isReading(l.account, l.folder, l.uid) || (!expanded.has(t.key) && t.letters.some(x => isReading(x.account, x.folder, x.uid))))
    const next = all[at < 0 ? (by > 0 ? 0 : all.length - 1) : at + by]?.l
    if (!next) return
    openLetter(next.account, next.uid, next.folder)
    requestAnimationFrame(() => body.querySelector(`.inbox [data-letter="${next.uid}"][data-folder="${next.folder}"][data-account="${CSS.escape(next.account)}"]`)?.scrollIntoView({ block: 'nearest' }))
  }

  // The inbox's conversations as kept, as filtered; with more, 50 more.
  // Asked again, quietly, whenever what is kept changed (snap.mailRev).
  async function loadInbox(more: boolean, quiet = false) {
    const id = inboxId()
    if (!id || inbox.busy) return
    const which = `${id}|${inbox.filter}`
    if (inbox.for !== which) Object.assign(inbox, { threads: [], total: 0, letters: 0, count: 50, error: null, errors: [], rev: 0 })
    if (more) inbox.count += 50
    const rev = st.snap?.mailRev || 0
    Object.assign(inbox, { id, for: which, busy: true, error: null })
    if (!quiet) redrawMail()
    const got = await pet.mail.letters(id, inbox.count, inbox.filter)
    inbox.busy = false
    if (inbox.for !== which) return redrawMail()
    if (got.ok) Object.assign(inbox, { threads: got.threads, total: got.total, letters: got.letters, errors: got.errors || [], syncing: got.syncing, rev })
    else inbox.error = got.error
    redrawMail()
    // It changed again meanwhile.
    if (got.ok && (st.snap?.mailRev || 0) !== rev) loadInbox(false, true)
  }

  // A letter opened: read whole, from what is kept (or the server), and
  // marked read.
  async function openLetter(account: string, uid: number, folder = 'inbox') {
    if (!accountOf(account)) return
    st.reading = { account, folder, uid, busy: true }
    draw(true)
    core.relayout()
    const got = await pet.mail.letter(account, uid, folder)
    if (!isReading(account, folder, uid)) return
    st.reading = got.ok ? { account, folder, uid, letter: got.letter, talk: got.talk } : { account, folder, uid, error: got.error }
    const l = listed(account, folder, uid)
    if (l && got.ok) Object.assign(l, { seen: true, flagged: got.letter.flagged })
    draw()
    core.relayout()
  }

  // Why a letter could not be had or handed on, in words.
  // err: what mail.rs or agent.rs said went wrong (its JSON).
  function handError(err: { kind?: string; text?: string } | null | undefined, agent?: string) {
    const kind = err?.kind || 'other'
    const own = ['noAgent', 'start', 'save', 'noLetter', 'gone', 'busy', 'noTalk', 'noSession', 'empty']
    if (!own.includes(kind)) return mailError(err, accountOf(st.reading?.account))
    // (No err: its kind is other, answered above.)
    return T(`mail.err.${kind}`, { agent: AGENTS[agent as string] || err!.text || '' }) + (kind === 'start' || kind === 'save' ? `${st.lang === 'zh' ? '：' : ': '}${err!.text}` : '')
  }

  function noteFor(note: Note | null) {
    clearTimeout(st.handNoteTimer)
    st.handNote = note
    redrawMail()
    if (note) st.handNoteTimer = setTimeout(() => noteFor(null), 10000)
  }

  // A letter handed to an agent: a talk about it under the letter (opened,
  // if it was not), or a session in a terminal.
  async function hand(account: string, uid: number, agent: string, how: string, folder = 'inbox') {
    closeMenu()
    if (how === 'talk' && !isReading(account, folder, uid)) await openLetter(account, uid, folder)
    noteFor({ text: T('mail.hand.busy', { agent: AGENTS[agent] }) })
    const got = await pet.mail.hand(account, uid, agent, how, folder)
    if (got.ok && how === 'talk' && st.reading?.letter && isReading(account, folder, uid)) st.reading.talk = null
    noteFor(got.ok ? (how === 'open' ? { text: T('mail.hand.opened', { agent: AGENTS[agent] }) } : null) : { text: handError(got.error, agent), bad: true })
  }

  // More asked in a letter's talk.
  async function say(box: HTMLInputElement) {
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
  async function star(account: string, uid: number, folder = 'inbox') {
    const inList = listed(account, folder, uid)
    const open = st.reading?.letter && isReading(account, folder, uid) ? st.reading.letter : null
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

  function openMenu(account: string, uid: number, folder: string, x: number, y: number) {
    const found = st.snap.mailAgents || {}
    const at = `data-account="${esc(account)}" data-folder="${folder || 'inbox'}" data-uid="${uid}"`
    const item = (ag: string, how: string, label: string) =>
      `<button class="mi" data-hand="${ag}:${how}" ${at} ${found[ag] ? '' : 'disabled'}>${esc(label)}${found[ag] ? '' : ` <span class="d">${esc(T('mail.hand.missing'))}</span>`}</button>`
    const ag = firstAgent()
    const talks = item(ag, 'talk', T('mail.hand.talk', { agent: AGENTS[ag] }))
    const terminals = item(ag, 'open', T('mail.hand.open', { agent: AGENTS[ag] }))
    const l = listed(account, folder, uid) || (isReading(account, folder, uid) ? st.reading!.letter : null)
    const starItem = `<button class="mi" data-star="${esc(account)}" data-folder="${folder || 'inbox'}" data-uid="${uid}">${esc(T(l?.flagged ? 'mail.unstar' : 'mail.star'))}</button>`
    const replies = [
      ['reply', 'mail.reply'],
      ['all', 'mail.replyAll'],
      ['forward', 'mail.forward'],
    ]
      .map(([kind, key]) => `<button class="mi" data-w-reply="${kind}" ${at}>${esc(T(key))}</button>`)
      .join('')
    // The letter open, from a sender whose pictures are always shown: not
    // any more (the list's rows have no address to go by).
    const open = isReading(account, folder, uid) ? st.reading!.letter : null
    const never = open && isSenderShown(open) ? `<button class="mi" data-pics="never" title="${esc(senderOf(open))}">${esc(T('mail.pics.never'))}</button>` : ''
    menuEl.innerHTML = `${replies}<div class="mi-sep"></div>${talks}<div class="mi-sep"></div>${terminals}<div class="mi-sep"></div>${starItem}${never}`
    st.menuPick = null
    placeMenu(x, y)
  }

  function placeMenu(x: number, y: number) {
    menuEl.hidden = false
    const box = layer.getBoundingClientRect()
    menuEl.style.left = `${Math.max(4, Math.min(x - box.left, box.width - menuEl.offsetWidth - 4))}px`
    menuEl.style.top = `${Math.max(4, Math.min(y - box.top, box.height - menuEl.offsetHeight - 4))}px`
  }

  // A menu of [value, label, off] under a button, the first (the agent's or
  // server's own, autodetect, every account) apart unless `together`; one
  // that is off is there, greyed, with why (off's words, else "not
  // supported").
  function openChoices(button: HTMLElement, choices: Choices, value: unknown, onPick: (value: string) => void, together = false) {
    menuEl.innerHTML = choices
      .map(
        ([v, label, off], i) =>
          `${i === 1 && !together ? '<div class="mi-sep"></div>' : ''}<button class="mi${v === value ? ' on' : ''}" data-pick="${esc(v)}" ${off ? 'disabled' : ''}>${esc(label)}${off ? ` <span class="d">${esc(typeof off === 'string' ? off : T('mail.unsupported'))}</span>` : ''}</button>`,
      )
      .join('')
    st.menuPick = onPick
    const r = button.getBoundingClientRect()
    placeMenu(r.left, r.bottom + 4)
  }

  // The split button's ▾ on an open letter: the other agents found, each
  // to hand this letter to (the first agent stays as it is), and under
  // them each made the first instead.
  function handMenu(button: HTMLElement) {
    const ag = firstAgent()
    const others = mailList().filter(a => a.found && a.id !== ag)
    menuEl.innerHTML = `${others.map(a => `<button class="mi" data-pick="hand:${a.id}">${esc(T('mail.hand.talk', { agent: a.short }))}</button>`).join('')}<div class="mi-sep"></div>${others
      .map(a => `<button class="mi" data-pick="first:${a.id}">${esc(T('mail.hand.makeFirst', { agent: a.short }))}</button>`)
      .join('')}`
    st.menuPick = v => {
      const [what, id] = v.split(':')
      if (what === 'first') return patch({ mailAgent: id })
      if (st.reading) hand(st.reading.account, Number(st.reading.uid), id, 'talk', st.reading.folder || 'inbox')
    }
    const r = button.getBoundingClientRect()
    placeMenu(r.right - 220, r.bottom + 4)
  }

  // (A model's or an effort's button: its data-picker says which.)
  function openPicker(button: HTMLElement) {
    const [ag, key] = button.dataset.picker!.split('.')
    openChoices(button, choicesOf(ag, key), confOf(ag)[key] || '', v => setConf(ag, key, v))
  }

  function closeMenu() {
    menuEl.hidden = true
    st.menuPick = null
  }

  layer.addEventListener('contextmenu', e => {
    const letter = st.tab === 'mail' && (e.target as HTMLElement).closest('[data-letter]')
    if (!letter || (e.target as HTMLElement).closest('.talk')) return
    // Ours, not the island's menu.
    e.preventDefault()
    e.stopPropagation()
    // A letter's row has its account with it (where).
    openMenu(letter.dataset.account!, Number(letter.dataset.letter), letter.dataset.folder || 'inbox', e.clientX, e.clientY)
  })
  body.addEventListener('scroll', closeMenu)

  return { addFiles, closeMenu, fitFrames, hand, handMenu, loadInbox, menuEl, netNow, openChoices, openLetter, openMenu, openPicker, pageMail, pageNet, proxyAddress, proxySetting, replyIn, runNetTest, say, sendDraft, setPictures, setProxy, star, step, waySetting, write, writeAbout }
}
