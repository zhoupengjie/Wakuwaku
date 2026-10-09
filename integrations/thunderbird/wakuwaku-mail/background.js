// Wakuwaku Mail: new mail in Wakuwaku's island, from Thunderbird. The unread
// count of the inboxes; for a new letter, the island opens with who it is
// from and what about. Private: while Wakuwaku's 显示具体内容 is off, only the
// count shows. Thunderbird keeps the accounts and how they sign in (OAuth for
// Gmail and Outlook too); this only reads and tells the pet, as any widget
// script does (examples/widgets).
//
// Thunderbird 115 lists an account's folders in .folders, each with a .type;
// later ones give a .rootFolder, and .specialUse instead of .type: both work.
// Loaded by Thunderbird as the background page, and required by the tests.
const PORT = 47213
const ID = 'mail-thunderbird'
// Gone from the island this long after Thunderbird stops sending.
const TTL = 300
const EVERY_MS = 60_000

// Who a letter is from: the name if there is one, else the address.
function who(author) {
  const s = String(author || '').trim()
  const named = s.match(/^"?([^"<]+?)"?\s*<[^>]+>$/)
  if (named) return named[1].trim()
  const bare = s.match(/<([^>]+)>/)
  return bare ? bare[1] : s
}

// The inboxes of every account.
async function inboxes(messenger) {
  const found = []
  const walk = folders => {
    for (const f of folders || []) {
      if (f.type === 'inbox' || (f.specialUse || []).includes('inbox')) found.push(f)
      walk(f.subFolders)
    }
  }
  for (const account of await messenger.accounts.list(true)) {
    walk(account.rootFolder ? [account.rootFolder] : account.folders)
  }
  return found
}

async function unreadCount(messenger) {
  let n = 0
  for (const f of await inboxes(messenger)) {
    const info = await messenger.folders.getFolderInfo(f.id ?? f).catch(() => messenger.folders.getFolderInfo(f))
    n += info.unreadMessageCount || 0
  }
  return n
}

// The widget: the count, and the newest letter's sender and subject.
function widgetFor(unread, newest, isNew) {
  const widget = {
    id: ID,
    icon: 'mail',
    color: '#5e9bff',
    private: true,
    ttl: TTL,
    label: newest ? `${who(newest.author)}：${newest.subject || '（无主题）'}` : 'Thunderbird',
    value: unread ? `${unread} 封未读` : '没有未读',
  }
  if (isNew && newest) widget.nudge = `新邮件 · ${widget.label}`
  return widget
}

async function send(fetchImpl, widget) {
  // As text/plain, so the request is a simple one (no CORS preflight).
  await fetchImpl(`http://127.0.0.1:${PORT}/widget`, { method: 'POST', headers: { 'Content-Type': 'text/plain' }, body: JSON.stringify(widget) }).catch(() => {})
}

function start(messenger, fetchImpl, every = EVERY_MS) {
  let newest = null
  const refresh = async isNew => {
    try {
      await send(fetchImpl, widgetFor(await unreadCount(messenger), newest, isNew))
    } catch (err) {
      console.warn('wakuwaku:', err)
    }
  }
  messenger.messages.onNewMailReceived.addListener(async (folder, list) => {
    const letters = (list?.messages || []).filter(m => !m.read)
    if (!letters.length) return
    newest = letters.reduce((a, b) => (new Date(b.date) > new Date(a.date) ? b : a))
    await refresh(true)
  })
  // Read elsewhere, or marked: the count follows, a moment later.
  let soon = null
  messenger.messages.onUpdated?.addListener(() => {
    clearTimeout(soon)
    soon = setTimeout(() => refresh(false), 1000)
  })
  refresh(false)
  const timer = setInterval(() => refresh(false), every)
  return () => clearInterval(timer)
}

if (typeof module !== 'undefined' && module.exports) {
  module.exports = { who, inboxes, unreadCount, widgetFor, start }
} else {
  start(globalThis.messenger, globalThis.fetch.bind(globalThis))
}
