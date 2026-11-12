// Mail accounts: what went wrong in words, setting one up as Thunderbird
// does (the address and password, the servers found or typed, its proxy),
// and the inbox: letters by thread, who they are from, their talks with the
// agents.
// Made by settings.ts in the section's place as the settings are made
// (mailPart(core), imported by settings.ts). The settings' core (SettingsCore):
// their constants and functions as they are, what they remember in core.st
// (settings-vars.ts, read and written as st.<name>), and the parts made
// before this one in core.pieces (a later one's reached once the settings
// are up). What it hands back is the settings' again.
import * as Agents from './agents.ts'
import type { SettingsCore } from './settings.ts'
import type { Letter, MailAccount, MailBox, MailForm, MailPick, Probed, Talk, Thread } from './settings-vars.ts'

// What went wrong with a mail account (mail.rs and its parts: their
// JSON): its kind, why a server said no to signing in, its own words, a
// hint, which proxy.
interface MailError {
  kind?: string
  why?: string
  text?: string
  hint?: string
  proxy?: string
}
// A server as the words name it: its host and port, the way to it.
interface Server {
  host?: string
  port?: unknown
  proxy?: { mode?: string } | null
}
// An outgoing server as found (discover.rs) or kept with an account.
interface OutServer {
  host: string
  port: number
  security: string
  auth?: string
  username: string
}
// A model an agent reads mail with (agent.rs), and the efforts it takes.
interface MailModel {
  id: string
  name: string
  efforts: string[]
}
// The choices of a list: [value, label, why it can't be picked].
export type Choices = readonly (readonly unknown[])[]

export function mailPart(core: SettingsCore) {
  const {
    st, pieces, AGENTS, AGENT_NAMES, Agents, T, esc, expanded, inbox, patch, row, sec, seg,
  } = core

  // What went wrong with a mail account, in words: through a proxy, which
  // hop (mail/proxy.rs), and what to try then.
  function mailError(err: MailError | null | undefined, server?: Server | null) {
    const kind = err?.kind || 'other'
    const why = loginWhy(err)
    if (why) return T(whyKey(why, server?.host))
    const where = server ? `${server.host}:${server.port}` : ''
    // The server's own words (or the proxy's), where they say why.
    const detail = err?.text && ['login', 'refused', 'other', 'recipient', 'notProxy', 'proxyRefused', 'proxyServer'].includes(kind) ? (st.lang === 'zh' ? '：' : ': ') + err.text : ''
    const hint = err?.hint ? (st.lang === 'zh' ? '。' : '. ') + T(`mail.err.${kind === 'notProxy' ? 'kind' : 'try'}.${err.hint}`) : ''
    // Gmail not reached with no proxy on the way: from China it takes one.
    const gmail = kind === 'connect' && providerOf(server?.host) === 'google' && straight(server?.proxy) ? ' ' + T('mail.err.gmailProxy') : ''
    return T(`mail.err.${kind}`, { where, proxy: err?.proxy || '' }) + detail + hint + gmail
  }

  // Why a server said no to signing in, as imap.rs login_why read its
  // words: said in the page's words for it (the provider's, where it has
  // its own), with the server's words under them (mailSaid), and where to
  // put it right (mailFix).
  const WHYS = ['appPassword', 'badPassword', 'webLogin', 'tooMany', 'imapOff']
  const loginWhy = (err: MailError | null | undefined) => (err?.kind === 'login' && WHYS.includes(err.why as string) ? err.why : '')
  function whyKey(why: string, host: unknown) {
    const p = providerOf(host)
    if (why === 'badPassword' && p) return `mail.why.badPassword.${p === 'netease' ? 'qq' : p === 'yahoo' ? 'apple' : p}`
    if ((why === 'appPassword' || why === 'webLogin') && p === 'google') return `mail.why.${why}.google`
    return `mail.why.${why}`
  }
  // (loginWhy is '' with no err.)
  const mailSaid = (err: MailError | null | undefined) => (loginWhy(err) && err!.text ? err!.text : '')
  function mailFix(err: MailError | null | undefined, host: unknown) {
    const why = loginWhy(err)
    const p = providerOf(host)
    if (why === 'webLogin' && p === 'google') return { site: 'gmailWeb', label: T('mail.openGmail') }
    const site = (why === 'appPassword' || why === 'badPassword') && HINT_SITE[p]
    return site ? { site, label: T('mail.hint.open') } : null
  }

  // Whether an account reaches its servers with no proxy on the way: its
  // own setting, else the Mail page's (Windows' proxy, while there is one).
  function straight(own: { mode?: string } | null | undefined) {
    const mode = own?.mode || pieces.letter.proxySetting().mode
    return mode === 'none' || (mode === 'system' && !st.snap.mailSystemProxy)
  }

  // An account's line: how its inbox is, or why it is not.
  function mailNote(a: MailAccount) {
    if (!a.on) return { text: T('mail.off') }
    const status = a.status || {}
    // How much is kept whole on this computer, while it is still coming down.
    const off = status.offline || {}
    const down = off.total && off.have < off.total ? ` · ${T('mail.offline', { have: off.have, total: off.total })}` : ''
    // Through a proxy: which.
    const via = status.via ? ` · ${T('mail.via', { proxy: status.via })}` : ''
    if (status.state === 'ok') return { text: (status.unread ? T('mail.unread', { n: status.unread }) : T('mail.none')) + down + via }
    if (status.state === 'error') return { text: mailError(status.error, a), bad: true }
    return { text: T('mail.connecting') }
  }

  // Setting an account up: the address and the password, then the server
  // found (a line, and a way to change it) or its fields to fill in.
  // Thunderbird's choices for the incoming server: how the connection is
  // secured, and how to sign in (a normal password, either way; the others
  // it offers are listed, not to be picked).
  const SECURITIES = () => [['auto', T('mail.auto')], ['plain', T('mail.plain')], ['starttls', 'STARTTLS'], ['ssl', 'SSL/TLS']]
  const AUTHS = () => [
    ['auto', T('mail.auto')],
    ['plain', T('mail.auth.plain')],
    ['login', T('mail.auth.login')],
    ['cram', T('mail.auth.cram'), true],
    ['gssapi', 'Kerberos / GSSAPI', true],
    ['ntlm', 'NTLM', true],
    ['oauth2', 'OAuth2', true],
  ]
  const labelOf = (list: Choices, value: unknown) => list.find(x => x[0] === value)?.[1] || value

  // Where a provider takes an app password (or a code) instead of the one
  // its users sign in with, by its IMAP host; and where it makes one.
  function providerOf(host: unknown) {
    const h = String(host || '').toLowerCase()
    if (/(^|\.)(gmail|googlemail)\.com$/.test(h)) return 'google'
    if (/(^|\.)mail\.me\.com$/.test(h)) return 'apple'
    if (/(^|\.)yahoo\.com$/.test(h)) return 'yahoo'
    if (/(^|\.)qq\.com$/.test(h)) return 'qq'
    if (/(^|\.)(163|126)\.com$|(^|\.)yeah\.net$/.test(h)) return 'netease'
    return ''
  }
  const HINT_SITE: Record<string, string> = { google: 'googleAppPasswords', apple: 'appleAppPasswords', yahoo: 'yahooAppPasswords' }

  // Why Gmail takes an app password here (Google's review for its browser
  // sign-in, no need of it for a one-person project); then the app password
  // in three steps, the first two with their page (the address's own, where
  // several Google accounts are signed in): 2-Step Verification on, an app
  // password made, pasted above; what 2-Step Verification takes (an
  // authenticator app will do); and why Google may not make one.
  function googleStepsHTML(address: string) {
    const step = (n: number, site?: string) =>
      `<div class="st"><span class="n">${n}</span><span>${esc(T(`mail.google.step${n}`))}${site ? ` <span class="link" data-site="${site}" data-for="${esc(address)}">${esc(T(`mail.google.open${n}`))}</span>` : ''}</span></div>`
    return `<div class="note">${marked(T('mail.google.why'))}</div><div class="note steps">${step(1, 'google2sv')}${step(2, 'googleAppPasswords')}${step(3)}</div><div class="note">${marked(T('mail.google.how2sv'))}</div><div class="note">${esc(T('mail.google.none'))}</div>`
  }

  // A note's words with *these* stressed (em.key: what is Google's to
  // decide, and what to have ready before turning 2-Step Verification on).
  const marked = (text: string) => esc(text).replace(/\*([^*]+)\*/g, '<em class="key">$1</em>')

  // Google's app password, as it shows it (four fours) or without the spaces.
  const APP_PASSWORD = /^[a-z]{4}(\s?[a-z]{4}){3}$/i

  function hintHTML(host: unknown) {
    const p = providerOf(host)
    if (!p) return ''
    const site = HINT_SITE[p]
    return `<div class="note">${esc(T(`mail.hint.${p}`))}${site ? ` <span class="link" data-site="${site}">${esc(T('mail.hint.open'))}</span>` : ''}</div>`
  }

  // The outgoing server's fields, from what was found (or none).
  const outFields = (o: OutServer | null | undefined) =>
    o
      ? { smtpHost: o.host, smtpPort: o.port, smtpSecurity: o.security, smtpAuth: o.auth || 'auto', smtpUser: o.username }
      : { smtpHost: '', smtpPort: '', smtpSecurity: 'auto', smtpAuth: 'auto', smtpUser: '' }

  // An account's own way to its servers (its "proxy", mail/proxy.rs) as the
  // form has it: a mode ('' for the Mail page's), and a manual one's kind
  // and address; and back, as sent (null: the Mail page's).
  const proxyFields = (p: { mode?: string; kind?: string; address?: string } | null | undefined) => ({ proxyMode: p?.mode || '', proxyKind: p?.kind === 'socks5' ? 'socks5' : 'http', proxyAddress: p?.address || '' })
  const formProxy = (f: MailForm) =>
    !f.proxyMode ? null : f.proxyMode === 'manual' ? { mode: 'manual', kind: f.proxyKind || 'http', address: String(f.proxyAddress || '').trim() } : { mode: f.proxyMode }
  const PROXY_KINDS = [
    ['http', 'HTTP'],
    ['socks5', 'SOCKS5'],
  ]
  const PROXY_MODES = () => [
    ['', T('mail.proxy.page', { what: T(`mail.proxy.${pieces.letter.waySetting('mailProxy').mode}Long`) })],
    ['system', T('mail.proxy.systemLong')],
    ['manual', T('mail.proxy.manualLong')],
    ['none', T('mail.proxy.noneLong')],
  ]

  // Setting an account up: the address, a name to send with, the password;
  // then the servers found (a line each) or not, and what the provider
  // wants for a password; and, folded as Thunderbird's manual setup, the
  // incoming and the outgoing server's every setting, with Re-test.
  // Drawn only while there is one (accountsHTML).
  function mailFormHTML() {
    const f = st.mailForm!
    const field = (key: MailBox, label: string, attrs = '') =>
      `<label class="fr"><span class="fl">${esc(label)}</span><input class="in" data-mf="${key}" value="${esc(f[key] ?? '')}" spellcheck="false" ${attrs}></label>`
    // Gmail (found, or typed in): its app password asked for by name, and how to make one.
    const google = f.step !== 'start' && providerOf(f.host) === 'google'
    const keep = f.id ? T('mail.passwordKeep') : google ? T('mail.appPasswordPlaceholder') : ''
    const head = [
      field('address', T('mail.address'), `placeholder="you@example.com" autocomplete="off"${f.id ? ' disabled' : ''}`),
      field('name', T('mail.name'), `placeholder="${esc(T('mail.namePlaceholder'))}" autocomplete="off"`),
      field('password', T(google ? 'mail.appPassword' : 'mail.password'), `type="password" autocomplete="off" placeholder="${esc(keep)}"`),
      google ? googleStepsHTML(String(f.address || '').trim()) : `<div class="note">${esc(T('mail.passwordNote'))}</div>`,
      f.step === 'start' || google ? '' : hintHTML(f.host),
    ].join('')
    const line = (proto: string, host: string, port: unknown, security: string, auth: string) => `${proto} · ${host}:${port} · ${labelOf(SECURITIES(), security)}${auth && auth !== 'auto' ? ` · ${labelOf(AUTHS(), auth)}` : ''}`
    const out = f.smtpHost ? `<div class="ellip">${esc(line('SMTP', f.smtpHost, f.smtpPort, f.smtpSecurity, f.smtpAuth))}</div>` : ''
    const noOut = f.smtpHost || f.findingOut ? '' : `<div class="note">${esc(T('mail.found.noOut'))}</div>`
    const found =
      f.step === 'found'
        ? `<div class="r"><div class="grow"><div class="ellip">${esc(line('IMAP', f.host, f.port, f.security, f.auth))}</div>${out}${f.source ? `<div class="d">${esc(T(`mail.source.${f.source}`))}</div>` : ''}</div></div>${noOut}`
        : ''
    const notFound = f.step === 'manual' && f.notFound ? `<div class="note">${esc(T(f.oauth ? 'mail.oauth' : 'mail.notFound'))}</div>` : ''
    const adv = f.step === 'start' ? '' : advancedHTML(field)
    const busy = f.busy ? `<div class="note">${esc(T({ find: 'mail.finding', probe: 'mail.adv.testing' }[f.busy as 'find' | 'probe'] || 'mail.signingIn'))}</div>` : ''
    const fix = f.result?.fix ? ` <span class="link" data-site="${esc(f.result.fix.site)}" data-for="${esc(String(f.address || '').trim())}">${esc(f.result.fix.label)}</span>` : ''
    const said = f.result?.said ? `<div class="said">${esc(f.result.said)}</div>` : ''
    const result = f.result ? `<div class="note${f.result.bad ? ' err' : ''}">${esc(f.result.text)}${fix}${said}</div>` : ''
    const off = f.busy ? 'disabled' : ''
    const go = f.step === 'start' ? `<button class="pbtn al" data-mail-find ${off}>${esc(T('mail.continue'))}</button>` : `<button class="pbtn al" data-mail-save ${off}>${esc(T('mail.done'))}</button>`
    const again = f.step === 'manual' && !f.id ? `<button class="pbtn" data-mail-find ${off}>${esc(T('mail.findAgain'))}</button>` : ''
    const retest = f.step !== 'start' && f.advanced ? `<button class="pbtn" data-mail-probe ${off}>${esc(T('mail.adv.retest'))}</button>` : ''
    const remove = f.id ? `<button class="pbtn" data-mail-remove="${esc(f.id)}">${esc(T(st.mailDelete === f.id ? 'mail.removeSure' : 'mail.remove'))}</button>` : ''
    return `${sec(T(f.id ? 'mail.change' : 'mail.add'))}<div class="grp mail-form">${head}${found}${notFound}${adv}${busy}${result}
      <div class="r">${remove}<span class="grow"></span><button class="pbtn" data-mail-cancel>${esc(T('mail.cancel'))}</button>${again}${retest}${go}</div></div>`
  }

  // The fold: Thunderbird's manual setup for the incoming server and the
  // outgoing one (protocol, host name, port, connection security,
  // authentication method, user name), what Re-test found of each.
  function advancedHTML(field: (key: MailBox, label: string, attrs?: string) => string) {
    const f = st.mailForm!
    const toggle = `<button class="adv-h${f.advanced ? ' on' : ''}" data-mail-adv><span class="chev">›</span>${esc(T('mail.adv'))}</button>`
    if (!f.advanced) return `<div class="adv">${toggle}</div>`
    const pick = (key: MailPick, list: Choices) => `<button class="pick" data-mfpick="${key}">${esc(labelOf(list, f[key] || 'auto'))} ▾</button>`
    const rows = [
      `<div class="adv-sec">${esc(T('mail.adv.in'))}</div>`,
      `<div class="fr"><span class="fl">${esc(T('mail.adv.protocol'))}</span><button class="pick" disabled>IMAP</button></div>`,
      field('host', T('mail.adv.host'), 'placeholder="imap.example.com"'),
      field('port', T('mail.port'), `inputmode="numeric" placeholder="${esc(T('mail.auto'))}"`),
      `<div class="fr"><span class="fl">${esc(T('mail.adv.security'))}</span>${pick('security', SECURITIES())}</div>`,
      `<div class="fr"><span class="fl">${esc(T('mail.adv.auth'))}</span>${pick('auth', AUTHS())}</div>`,
      field('username', T('mail.username'), `placeholder="${esc(f.address || 'you@example.com')}"`),
      f.security === 'plain' ? `<div class="note err">${esc(T('mail.adv.plainWarn'))}</div>` : '',
      probedHTML(f.probed),
      `<div class="adv-sec">${esc(T('mail.adv.out'))}</div>`,
      `<div class="fr"><span class="fl">${esc(T('mail.adv.protocol'))}</span><button class="pick" disabled>SMTP</button></div>`,
      field('smtpHost', T('mail.adv.host'), 'placeholder="smtp.example.com"'),
      field('smtpPort', T('mail.port'), `inputmode="numeric" placeholder="${esc(T('mail.auto'))}"`),
      `<div class="fr"><span class="fl">${esc(T('mail.adv.security'))}</span>${pick('smtpSecurity', SECURITIES())}</div>`,
      `<div class="fr"><span class="fl">${esc(T('mail.adv.auth'))}</span>${pick('smtpAuth', AUTHS())}</div>`,
      field('smtpUser', T('mail.username'), `placeholder="${esc(f.username || f.address || 'you@example.com')}"`),
      f.smtpHost && f.smtpSecurity === 'plain' ? `<div class="note err">${esc(T('mail.adv.plainWarn'))}</div>` : '',
      probedHTML(f.probedOut),
      `<div class="note">${esc(T('mail.adv.outNote'))}</div>`,
      `<div class="adv-sec">${esc(T('mail.proxy'))}</div>`,
      `<div class="fr"><span class="fl">${esc(T('mail.adv.proxyWay'))}</span><button class="pick" data-mfpick="proxyMode">${esc(labelOf(PROXY_MODES(), f.proxyMode || ''))} ▾</button></div>`,
      f.proxyMode === 'manual' ? `<div class="fr"><span class="fl">${esc(T('mail.proxy.kind'))}</span><button class="pick" data-mfpick="proxyKind">${esc(labelOf(PROXY_KINDS, f.proxyKind || 'http'))} ▾</button></div>` : '',
      f.proxyMode === 'manual' ? field('proxyAddress', T('mail.proxy.address'), 'placeholder="127.0.0.1:7890"') : '',
      `<div class="note">${esc(T('mail.adv.proxyNote'))}</div>`,
    ]
    return `<div class="adv on">${toggle}<div class="adv-b">${rows.join('')}</div></div>`
  }

  // What Re-test found: how it is reached, how it lets one sign in, and,
  // for a Windows domain's server (Exchange), what the user name may be.
  function probedHTML(p: Probed | null) {
    if (!p) return ''
    const names = p.auths.map(a => {
      const [, label, off] = AUTHS().find(x => x[0] === a) || [a, a, true]
      return off ? `${label}${T('mail.unsupported')}` : label
    })
    const how = T('mail.adv.found', { how: labelOf(SECURITIES(), p.security), port: p.port })
    const auths = p.auths.some(a => a === 'plain' || a === 'login') ? T('mail.adv.auths', { list: names.join(st.lang === 'zh' ? '、' : ', ') }) : T('mail.adv.noAuth')
    const domain = p.auths.some(a => a === 'ntlm' || a === 'gssapi') ? `<div class="note">${esc(T('mail.adv.domain'))}</div>` : ''
    return `<div class="note ok">✓ ${esc(how)} ${esc(auths)}</div>${domain}`
  }

  // Who a letter goes to first (the setting), and the others that read mail
  // (agents.ts mailAgents): Antigravity only where it is (or set first), as
  // few have it: its agy found, or Antigravity on this computer as the Agent
  // page has it (agy's folder, or the 2.0 app's: connection/agy.rs agy_here).
  // The app alone brings no agy, which mail runs: Antigravity is listed
  // then, greyed, with a line on why (zhoupengjie/Wakuwaku#42). agent.rs
  // looks for agy again while the settings are open, and once installed it
  // can be picked.
  const firstAgent = () => (AGENTS[st.snap.settings.mailAgent as string] ? st.snap.settings.mailAgent! : 'claude')
  const mailList = () => Agents.mailAgents(st.snap, firstAgent())
  const agyWanted = () => mailList().some(a => a.id === 'agy' && !a.found)
  // A letter's talk going on now, by its key (agent.rs).
  const talkOf = (key: string | undefined): Talk | null => (key && (st.snap.mailTalks || {})[key]) || null
  const letterKey = (l: Letter) => l.messageId || `${l.account}:${l.uid}`
  const accountOf = (id: string | undefined) => (st.snap.mail || []).find(a => a.id === id) || null

  // The inbox shown: "*" (every account's as one) where there are several,
  // else the one; one picked before stays while it is there.
  function inboxId() {
    const all = st.snap.mail || []
    if (inbox.id === '*' && all.length > 1) return '*'
    if (all.some(a => a.id === inbox.id)) return inbox.id
    return all.length > 1 ? '*' : all[0]?.id || ''
  }

  const unreadOf = (a: MailAccount) => (a.on && a.status?.state === 'ok' ? a.status.unread || 0 : 0)

  function mailDate(ms: number | null | undefined, full?: boolean) {
    if (!ms) return ''
    const d = new Date(ms)
    const now = new Date()
    const locale = st.lang === 'zh' ? 'zh-CN' : 'en-US'
    const time = d.toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit', hour12: false })
    if (full) return `${d.toLocaleDateString(locale, { year: 'numeric', month: 'short', day: 'numeric' })} ${time}`
    if (d.toDateString() === now.toDateString()) return time
    if (d.getFullYear() === now.getFullYear()) return d.toLocaleDateString(locale, { month: 'short', day: 'numeric' })
    return d.toLocaleDateString(locale, { year: 'numeric', month: 'numeric', day: 'numeric' })
  }

  const mailSize = (n: number) =>
    n >= 1 << 30 ? `${(n / (1 << 30)).toFixed(1)} GB` : n >= 1 << 20 ? `${(n / (1 << 20)).toFixed(1)} MB` : n >= 1024 ? `${Math.round(n / 1024)} KB` : `${n} B`
  const people = (list: { name?: string; address: string }[] | null | undefined) => (list || []).map(p => (p.name ? `${p.name} <${p.address}>` : p.address)).join(', ')
  const starHTML = (account: string, uid: number, on: boolean, folder = 'inbox') =>
    `<button class="star${on ? ' on' : ''}" data-star="${esc(account)}" data-folder="${folder}" data-uid="${uid}" title="${esc(T(on ? 'mail.unstar' : 'mail.star'))}">${on ? '★' : '☆'}</button>`

  // A letter by where it is: its account, its folder (inbox, sent), its UID
  // there (the inbox's and Sent's UIDs are their own).
  const isLetter = (l: Letter | null, account: string, folder: string | undefined, uid: number | undefined) => !!l && l.account === account && (l.folder || 'inbox') === (folder || 'inbox') && l.uid === uid
  const isReading = (account: string, folder: string | undefined, uid: number) => isLetter(st.reading, account, folder, uid)
  const listed = (account: string, folder: string, uid: number): Letter | null => inbox.threads.flatMap(t => t.letters).find(l => isLetter(l, account, folder, uid)) || null
  const where = (l: Letter) => `data-letter="${l.uid}" data-folder="${l.folder || 'inbox'}" data-account="${esc(l.account)}"`
  const whoOf = (l: Letter) => (l.me ? T('mail.me') : l.from || l.address || '?')
  const chipOf = (l: Letter) => {
    const talk = talkOf(letterKey(l))
    return talk ? `<span class="chip ${talk.state}">${esc(T(`mail.chip.${talk.state}`, { agent: AGENTS[talk.agent] || 'Agent' }))}</span>` : ''
  }
  const marks = (l: Letter) => `${l.answered ? `<span class="answered" title="${esc(T('mail.answered'))}">↩</span> ` : ''}${l.attached ? '📎 ' : ''}`

  // A letter's card in the inbox: unread or not, who, when, what about, its
  // star, whose (in every account's), and whether an agent is on it.
  function letterRow(l: Letter, every: boolean) {
    const whose = every ? `<span class="acct ellip">${esc(accountOf(l.account)?.address || '')}</span>` : ''
    const open = isReading(l.account, l.folder, l.uid) ? ' open' : ''
    return `<div class="r letter${l.seen ? '' : ' unread'}${open}" ${where(l)}><span class="udot"></span>
      <div class="grow"><div class="lt"><span class="who ellip">${esc(whoOf(l))}</span><span class="when">${esc(mailDate(l.date))}</span></div>
      <div class="lt"><span class="d ellip grow">${marks(l)}${esc(l.subject || T('mail.noSubject'))}</span>${whose}</div></div>${chipOf(l)}${starHTML(l.account, l.uid, l.flagged, l.folder)}</div>`
  }

  // A conversation, as Thunderbird's cards show one: who wrote (oneself as
  // me), how many letters, when the newest came, what about; folded out, its
  // letters under it in the order they answer one another, each indented by
  // how deep it is, with the start of its text. A click on it opens its
  // newest unread letter (else its newest) and folds it out; › folds it.
  function threadHTML(t: Thread, every: boolean) {
    const out = expanded.has(t.key)
    const target = t.letters.find(l => isLetter(l, t.account, t.open?.folder, t.open?.uid)) || t.letters.at(-1)
    const open = !out && t.letters.some(l => isReading(l.account, l.folder, l.uid)) ? ' open' : ''
    const who = t.people.map(p => (p.me ? T('mail.me') : p.name)).join(', ')
    const whose = every ? `<span class="acct ellip">${esc(accountOf(t.account)?.address || '')}</span>` : ''
    const any = { answered: t.letters.some(l => l.answered), attached: t.attached }
    const head = `<div class="r letter thread${t.unseen ? ' unread' : ''}${open}" data-thread="${esc(t.key)}" ${where(target)}>
      <span class="twisty${out ? ' out' : ''}" data-twisty="${esc(t.key)}" title="${esc(T(out ? 'mail.fold' : 'mail.unfold'))}">›</span>
      <div class="grow"><div class="lt"><span class="who ellip">${esc(who)}</span><span class="cnt">${t.count}</span><span class="when">${esc(mailDate(t.latest))}</span></div>
      <div class="lt"><span class="d ellip grow">${marks(any)}${esc(t.subject || T('mail.noSubject'))}</span>${whose}</div></div>${starHTML(target.account, target.uid, t.flagged, target.folder)}</div>`
    if (!out) return head
    const cards = t.letters
      .map(l => {
        const here = isReading(l.account, l.folder, l.uid) ? ' open' : ''
        return `<div class="r letter child${l.seen ? '' : ' unread'}${here}" style="--depth:${Math.min(l.depth, 4)}" ${where(l)}><span class="udot"></span>
        <div class="grow"><div class="lt"><span class="who ellip">${esc(whoOf(l))}</span><span class="when">${esc(mailDate(l.date))}</span></div>
        <div class="lt"><span class="d ellip grow">${marks(l)}${esc(l.snippet || l.subject || T('mail.noSubject'))}</span></div></div>${chipOf(l)}${starHTML(l.account, l.uid, l.flagged, l.folder)}</div>`
      })
      .join('')
    return `${head}<div class="thread-cards">${cards}</div>`
  }

  // Which inbox, as the picker names it: every account's (with all their
  // unread), or one (with its).
  function boxName(id: string) {
    const all = st.snap.mail || []
    if (id === '*') return T('mail.box.all', { n: all.reduce((n, a) => n + unreadOf(a), 0) })
    const a = accountOf(id)
    return a ? T('mail.box.one', { address: a.address, n: unreadOf(a) }) : ''
  }

  // The Mail page in two panes, list and letter side by side, as Thunderbird
  // has it (the setting mailPanes, two unless one), where the screen has the
  // room; the settings as wide as that then (island.ts measures them).
  const WIDE_W = 960
  const roomForTwo = () => screen.availWidth >= 820
  const twoPanes = () => st.tab === 'mail' && st.snap?.settings?.mailPanes !== 'one' && roomForTwo()

  // The inbox: which (every account's, or one), all, unread or starred, its
  // newest letters, more on asking; one pane or two.
  function inboxHTML(id: string) {
    const all = st.snap.mail || []
    const every = id === '*'
    const two = twoPanes()
    const pick = all.length > 1 ? `<button class="pick" data-mail-box>${esc(boxName(id))} ▾</button>` : ''
    const filters = seg('mail.filter', [['all', T('mail.filter.all')], ['unseen', T('mail.filter.unseen')], ['flagged', T('mail.filter.flagged')]], inbox.filter)
    // Two panes: the accounts and agents shown on the right again (the letter
    // open put away); one pane or two.
    const setup = two ? `<span class="link" data-mail-setup title="${esc(T('mail.setupTip'))}">${esc(T('mail.setup'))}</span>` : ''
    const panes = two ? `<span class="link" data-mail-panes="one">${esc(T('mail.panes.one'))}</span>` : roomForTwo() ? `<span class="link" data-mail-panes="two">${esc(T('mail.panes.two'))}</span>` : ''
    const head = `<div class="sec row-sec"><span class="ellip">${esc(T('mail.inbox'))}${inbox.letters ? ` · ${inbox.letters}` : ''}</span><span class="grow"></span><span class="link" data-w-new>${esc(T('mail.write'))}</span><span class="link" data-mail-refresh>${esc(T(inbox.busy ? 'mail.loading' : 'mail.refresh'))}</span>${setup}${panes}</div>`
    const tools = `<div class="r mail-tools">${pick}<span class="grow"></span>${filters}</div>`
    // The letter put aside, to go on with (not while it is written beside the list).
    const kept = st.draft && !(two && st.writing)
      ? `<div class="r draft-row" data-w-open><span class="chip">${esc(T('mail.w.draft'))}</span><div class="grow ellip">${esc(st.draft.subject || T('mail.noSubject'))}${st.draft.to ? ` · ${esc(st.draft.to)}` : ''}</div><span class="link">${esc(T('mail.w.goOn'))}</span></div>`
      : ''
    const note = st.handNote && !two ? `<div class="note${st.handNote.bad ? ' err' : ''}">${esc(st.handNote.text)}</div>` : ''
    // Accounts that could not be asked, in every account's.
    const missed = (inbox.errors || []).map(e => `<div class="note err">${esc(`${accountOf(e.account)?.address || ''}: ${mailError(e.error, accountOf(e.account))}`)}</div>`).join('')
    let list
    if (inbox.error) list = `<div class="note err">${esc(mailError(inbox.error, accountOf(id)))}</div>`
    else if (!inbox.threads.length) list = `<div class="note">${esc(T(inbox.syncing ? 'mail.syncing' : inbox.busy && !inbox.rev ? 'mail.loading' : `mail.empty.${inbox.filter}`))}</div>`
    else list = inbox.threads.map(t => (t.count > 1 ? threadHTML(t, every) : letterRow(t.letters[0], every))).join('')
    const left = inbox.total - inbox.threads.length
    const more = !inbox.error && left > 0 ? `<div class="r"><button class="pbtn wide" data-mail-more ${inbox.busy ? 'disabled' : ''}>${esc(T('mail.moreThreads', { n: Math.min(50, left) }))}</button></div>` : ''
    return `${head}<div class="grp inbox">${tools}${kept}${note}${missed}${list}${more}</div>`
  }

  // Mail's AI, one for every letter of every mailbox (the user, 2026-10-14:
  // which agent, its model, its effort, and no more): its model and effort
  // (mailAgentConf, agent.rs; empty is the agent's own setting), what a talk
  // may do the least it has, said under them (Antigravity's models are each
  // one effort already). Not found, it says so there. Which agent: a
  // dropdown of those that read mail, the ones whose command is not found
  // greyed with why (agentChoices).
  function agentsHTML() {
    const ag = firstAgent()
    const found = st.snap.mailAgents || {}
    const agent = row(esc(T('mail.agent')), esc(T('mail.agentNote')), `<button class="pick" data-mail-agent>${esc(AGENT_NAMES[ag])} ▾</button>`)
    const why = agyWanted() ? `<div class="note">${esc(T('mail.agyCli'))} <span class="link" data-site="agyCli">${esc(T('mail.agyCliHow'))}</span></div>` : ''
    const note = found[ag] ? T(`mail.conf.note.${ag}`) : T('mail.err.noAgent', { agent: AGENT_NAMES[ag] })
    const pickers = ag === 'agy' ? pickerHTML(ag, 'model') : pickerHTML(ag, 'model') + pickerHTML(ag, 'effort')
    const model = row(esc(T(ag === 'agy' ? 'mail.conf.model' : 'mail.conf.session')), esc(note), `<span class="btns">${pickers}</span>`)
    return `<div class="grp agent-pick">${agent}${why}${model}</div>`
  }

  // How a letter goes to an agent (mailAgentConf, agent.rs): its model and
  // effort; empty is the agent's own setting.
  const confOf = (ag: string) => ((st.snap.settings.mailAgentConf || {})[ag] || {})
  const modelsOf = (ag: string): MailModel[] => (st.snap.mailModels || {})[ag] || []

  // The efforts a model takes (all of them while it is the agent's own).
  function effortsOf(ag: string, model: unknown) {
    const all = modelsOf(ag)
    const m = all.find(x => x.id === model)
    return m ? m.efforts : [...new Set(all.flatMap(x => x.efforts))]
  }

  // A picker's choices: the agent's own first, then each model or effort.
  function choicesOf(ag: string, key: string) {
    const c = confOf(ag)
    const list = key === 'model' ? modelsOf(ag).map(m => [m.id, m.name]) : effortsOf(ag, c.model).map(e => [e, effortName(e)])
    // One set before and no longer listed stays a choice.
    if (c[key] && !list.some(([v]) => v === c[key])) list.push([c[key], c[key]])
    return [['', T('mail.conf.own')], ...list]
  }

  const effortName = (e: string) => (T(`mail.effort.${e}`) === `mail.effort.${e}` ? e : T(`mail.effort.${e}`))

  function pickerHTML(ag: string, key: string) {
    const value = confOf(ag)[key] || ''
    const label = choicesOf(ag, key).find(([v]) => v === value)?.[1] || value
    return `<button class="pick" data-picker="${ag}.${key}">${esc(label)} ▾</button>`
  }

  // One of them changed; an effort the new model does not take goes back
  // to the agent's own.
  function setConf(ag: string, key: string, value: string) {
    const all = st.snap.settings.mailAgentConf || {}
    const next = { ...(all[ag] || {}), [key]: value }
    if (key === 'model' && next.effort && !effortsOf(ag, value).includes(next.effort)) next.effort = ''
    patch({ mailAgentConf: { ...all, [ag]: next } })
  }

  // A little markdown, as agents write: lines, **bold**, *italic*, `code`,
  // headings, lists, quotes (indented under a list item too, or within
  // bold: **> …**), ``` blocks (a reply it wrote, often). The text is
  // escaped first.
  const inline = (s: string) =>
    s
      .replace(/\*\*(.+?)\*\*/g, '<b>$1</b>')
      .replace(/(^|[^*\w])\*(?!\s)([^*]+?)\*(?![*\w])/g, '$1<i>$2</i>')
      .replace(/`([^`]+)`/g, '<code>$1</code>')

  function md(text: string) {
    let html = ''
    let block: string[] | null = null
    const pre = (lines: string[]) => `<pre class="md-pre">${lines.join('\n')}</pre>`
    for (const raw of esc(text).split('\n')) {
      if (/^\s*```/.test(raw)) {
        if (block) html += pre(block)
        block = block ? null : []
        continue
      }
      if (block) {
        block.push(raw)
        continue
      }
      let m
      if ((m = /^\s*(\*\*)?&gt;\s?(.*)$/.exec(raw))) {
        // GitHub's [!NOTE] and the like: the quote says it already.
        if (!/^\[!(note|tip|important|warning|caution)\]$/i.test(m[2].trim())) html += `<div class="md-q">${inline((m[1] || '') + m[2]) || '&nbsp;'}</div>`
        continue
      }
      const line = inline(raw)
      if ((m = /^\s*([-*•]|\d+[.)])\s+(.*)$/.exec(line))) html += `<div class="md-li"><span>${m[1] === '*' ? '•' : m[1]}</span><span>${m[2]}</span></div>`
      else if ((m = /^#{1,4}\s+(.*)$/.exec(line))) html += `<div class="md-h">${m[1]}</div>`
      else html += line.trim() ? `<div>${line}</div>` : '<div class="md-gap"></div>'
    }
    return block ? html + pre(block) : html
  }

  // The talk under a letter: who with, how it is going, what was said (the
  // agent's words as they come, what it did in small), and a box to ask more.
  function talkHTML(talk: Talk, letter: Letter) {
    const agent = AGENTS[talk.agent] || 'Agent'
    const going = talk.state === 'running'
    const state = going ? T('mail.talk.going', { agent }) : talk.state === 'failed' ? T('mail.talk.failed', { agent, why: talk.error || '' }) : T('mail.talk.done', { agent })
    const turns = talk.turns
      .map((t, i) => {
        if (t.who === 'start') return `<div class="t-start">${esc(T('mail.talk.start', { agent }))}</div>`
        if (t.who === 'me') return `<div class="t-me">${esc(t.text)}</div>`
        if (t.who === 'tool') return `<div class="t-tool">· ${esc(t.text)}</div>`
        // What it said, once said, to reply with.
        const writing = going && i === talk.turns.length - 1
        return `<div class="t-agent">${md(t.text)}</div>${writing ? '' : `<div class="t-use"><span class="link" data-w-use="${i}">${esc(T('mail.w.useReply'))}</span></div>`}`
      })
      .join('')
    const thinking = going && talk.turns.at(-1)?.who !== 'agent' ? `<div class="t-wait">${esc(T('mail.talk.thinking'))}</div>` : ''
    const ask = `<div class="t-ask"><input class="in" data-talk="${esc(letter.key)}" placeholder="${esc(T('mail.talk.ask', { agent }))}" ${going ? 'disabled' : ''} spellcheck="false"><button class="pbtn sm al" data-talk-send ${going ? 'disabled' : ''}>${esc(T('mail.talk.send'))}</button></div>`
    return `<div class="talk ${talk.state}"><div class="talk-h"><span class="grow">${esc(state)}</span>${going ? '' : `<span class="link" data-hand="${talk.agent}:talk">${esc(T('mail.talk.again'))}</span>`}</div>${turns}${thinking}${ask}</div>`
  }

  // The Mail page's agents as the dropdown has them: [id, name, why not].
  const agentChoices = () => mailList().map(a => [a.id, a.name, a.found ? '' : T(`mail.agentMissing.${a.id}`)])

  // The buttons above an open letter: its star, reply, hand it to the first
  // agent (when no talk is there yet), and ⋯ for the rest. Handing it is a
  // split button: its left part to the first agent, ▾ to another one found
  // for this letter only, or that one made the first (handMenu).
  function handButtons(l: Letter, talk: Talk | null | undefined) {
    const ag = firstAgent()
    const off = !(st.snap.mailAgents || {})[ag] ? 'disabled' : ''
    const others = mailList().filter(a => a.found && a.id !== ag)
    const more = others.length ? `<button class="pbtn sm al more" data-hand-more title="${esc(T('mail.hand.others'))}">▾</button>` : ''
    const give = talk ? '' : `<span class="split${more ? ' two' : ''}"><button class="pbtn sm al" data-hand="${ag}:talk" ${off}>${esc(T('mail.hand.talk', { agent: AGENTS[ag] }))}</button>${more}</span>`
    const reply = `<button class="pbtn sm" data-w-reply="reply">${esc(T('mail.reply'))}</button>`
    return `${starHTML(l.account, l.uid, l.flagged)}${reply}${give}<button class="pbtn sm" data-mail-menu title="${esc(T('mail.hand.more'))}">⋯</button>`
  }

  return { APP_PASSWORD, AUTHS, PROXY_KINDS, PROXY_MODES, SECURITIES, WIDE_W, accountOf, agentChoices, agentsHTML, boxName, choicesOf, confOf, firstAgent, formProxy, handButtons, inboxHTML, inboxId, isLetter, isReading, labelOf, listed, loginWhy, mailDate, mailError, mailFix, mailFormHTML, mailList, mailNote, mailSaid, mailSize, outFields, people, providerOf, proxyFields, setConf, talkHTML, talkOf, twoPanes, where }
}
