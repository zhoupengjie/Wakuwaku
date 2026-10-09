const assert = require('node:assert/strict')
const { test } = require('node:test')

const tb = require('../integrations/thunderbird/wakuwaku-mail/background')

// Thunderbird's API as the extension sees it: two accounts, one listed the
// Thunderbird 115 way (.folders, .type) and one the later way (.rootFolder,
// .specialUse), and the events it listens to.
function fakeMessenger() {
  const listeners = { newMail: [], updated: [] }
  const counts = { inbox1: 2, inbox2: 3, archive: 9 }
  return {
    listeners,
    accounts: {
      list: async () => [
        { id: 'a1', folders: [{ id: 'inbox1', type: 'inbox', subFolders: [] }, { id: 'archive', type: 'archives' }] },
        { id: 'a2', rootFolder: { id: 'root2', specialUse: [], subFolders: [{ id: 'inbox2', specialUse: ['inbox'], subFolders: [] }] } },
      ],
    },
    folders: { getFolderInfo: async id => ({ unreadMessageCount: counts[id] }) },
    messages: {
      onNewMailReceived: { addListener: fn => listeners.newMail.push(fn) },
      onUpdated: { addListener: fn => listeners.updated.push(fn) },
    },
  }
}

test('who a letter is from', () => {
  assert.equal(tb.who('张三 <zs@example.com>'), '张三')
  assert.equal(tb.who('"Li, Si" <ls@example.com>'), 'Li, Si')
  assert.equal(tb.who('<only@example.com>'), 'only@example.com')
  assert.equal(tb.who('plain@example.com'), 'plain@example.com')
})

test('the inboxes of every account, however Thunderbird lists them', async () => {
  const m = fakeMessenger()
  assert.deepEqual((await tb.inboxes(m)).map(f => f.id), ['inbox1', 'inbox2'])
  assert.equal(await tb.unreadCount(m), 5)
})

test('a new letter opens the island, and says who and what (privately)', async () => {
  const m = fakeMessenger()
  const sent = []
  const fetch = async (url, init) => {
    sent.push({ url, init, widget: JSON.parse(init.body) })
    return { ok: true }
  }
  const stop = tb.start(m, fetch, 60_000)
  await new Promise(r => setTimeout(r, 20))
  assert.equal(sent[0].url, 'http://127.0.0.1:47213/widget')
  assert.equal(sent[0].init.headers['Content-Type'], 'text/plain')
  assert.deepEqual(sent[0].widget, { id: 'mail-thunderbird', icon: 'mail', color: '#5e9bff', private: true, ttl: 300, label: 'Thunderbird', value: '5 封未读' })

  await m.listeners.newMail[0]({ id: 'inbox1' }, {
    messages: [
      { author: '王总 <wang@example.com>', subject: '周五的方案', date: '2026-10-09T09:00:00Z', read: false },
      { author: '旧的 <old@example.com>', subject: '昨天的', date: '2026-10-08T09:00:00Z', read: false },
    ],
  })
  const last = sent.at(-1).widget
  assert.equal(last.label, '王总：周五的方案')
  assert.equal(last.nudge, '新邮件 · 王总：周五的方案')
  assert.equal(last.private, true)
  // Only letters already read: nothing to say.
  const before = sent.length
  await m.listeners.newMail[0]({ id: 'inbox1' }, { messages: [{ author: 'x', subject: 'y', date: '2026-10-09T10:00:00Z', read: true }] })
  assert.equal(sent.length, before)
  stop()
})
