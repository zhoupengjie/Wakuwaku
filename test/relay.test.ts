// The crash relay (relay/relay.js): what it takes, what it scrubs, how a
// crash becomes an issue or adds to one, and how it asks GitHub and Gitee.
import assert from 'node:assert/strict'
import crypto from 'node:crypto'
import path from 'node:path'
import { test } from 'node:test'
import { pathToFileURL } from 'node:url'

const load = () => import(pathToFileURL(path.join(import.meta.dirname, '..', 'relay', 'relay.js')).href)

const REPORT = {
  v: 1,
  app: '0.2.2',
  kind: 'panic',
  code: '0xC0000409',
  where: 'src\\taskbar.rs:1234:17',
  message: 'called `Option::unwrap()` on a `None` value',
  detail: 'thread pointer',
  frames: ['wakuwaku.exe+0x4c1d20', 'ntdll.dll+0x5af38'],
  os: 'Windows 11 25H2 (26200.6584) x64',
  webview2: '141.0.3537.71',
  home: 'taskbar',
  out: false,
  lang: 'zh',
  uptime: 5321,
  at: '2026-10-11T04:22Z',
  log: ['-4.1s taskbar: tray: added Weixin.exe (id 3, by window)', '-0.1s jump to <id>: there'],
}

function fakeForge() {
  const issues = []
  const view = i => ({ number: i.number, url: i.url, state: i.state, body: i.body })
  return {
    issues,
    name: 'fake',
    async find(tag) {
      const hits = issues.filter(i => i.title.includes(`[${tag}]`))
      const hit = hits.find(i => i.state === 'open') || hits[0]
      return hit ? view(hit) : null
    },
    async get(number) {
      const i = issues.find(x => x.number === number)
      if (!i) throw Object.assign(new Error('404'), { status: 404 })
      return view(i)
    },
    async create(title, body) {
      const i = { number: issues.length + 1, url: `https://example.test/${issues.length + 1}`, state: 'open', title, body, comments: [] }
      issues.push(i)
      return view(i)
    },
    async comment(number, body) {
      issues.find(x => x.number === number).comments.push(body)
    },
    async edit(number, body) {
      issues.find(x => x.number === number).body = body
    },
  }
}

const post = (body, ip = '203.0.113.9', headers: Record<string, string> = { 'x-wakuwaku': '1' }) => ({ method: 'POST', path: '/report', ip, header: n => headers[n], body: typeof body === 'string' ? body : JSON.stringify(body) })

test('a report is checked field by field, and nothing else taken', async () => {
  const { check } = await load()
  assert.equal(check(REPORT), null)
  assert.equal(check({ v: 1, app: '0.2.2', kind: 'exit', at: '2026-10-11T04:22Z', code: '0xC0000374' }), null)
  assert.equal(check({ v: 1, app: '0.2.2', kind: 'webview', at: '2026-10-11T04:22Z', code: 'render-exited/crashed/0xC0000005', detail: 'window pet' }), null)
  assert.match(check({ ...REPORT, code: 'render-exited/crashed/5' }), /code/)
  assert.match(check({ ...REPORT, extra: 1 }), /extra: not a field/)
  assert.match(check({ ...REPORT, frames: ['C:\\x.dll+0x1'] }), /frames/)
  assert.match(check({ ...REPORT, log: Array(81).fill('x') }), /log/)
  assert.match(check({ ...REPORT, log: ['a\nb'] }), /log/)
  assert.match(check({ ...REPORT, kind: 'oops' }), /kind/)
  assert.match(check({ ...REPORT, v: 2 }), /v:/)
  assert.match(check({ ...REPORT, app: '1.0' }), /app/)
  const { at, ...noTime } = REPORT
  assert.match(check(noTime), /at missing/)
  assert.match(check([]), /not an object/)
})

test("what could still be the person's is scrubbed again", async () => {
  const { scrub } = await load()
  assert.equal(scrub('mail zhang@corp.com: sent'), 'mail <email>: sent')
  assert.equal(scrub('C:\\Users\\zhang\\x.txt'), '%USERPROFILE%\\x.txt')
  assert.equal(scrub('in 我的项目 now'), 'in ‹…› now')
  assert.equal(scrub('key sk-ant-api03-abcdefgh1234 and xY9aB8cD7eF6gH5iJ4kL3mN2'), 'key <token> and <token>')
  assert.equal(scrub('open https://imap.corp.com/x?a=1: failed'), 'open https://<host>/…: failed')
  assert.equal(scrub('see https://github.com/zhoupengjie/Wakuwaku'), 'see https://github.com/…')
  assert.equal(scrub('from 203.0.113.9 and 127.0.0.1, WebView2 141.0.3537.71'), 'from <ip> and 127.0.0.1, WebView2 141.0.3537.71')
  assert.equal(scrub('jump to 3f2a9c1e-6b1d-4c3a-9e2f-0a1b2c3d4e5f'), 'jump to <id>')
  assert.equal(scrub('island: mouse caught'), 'island: mouse caught')
})

test('the same crash adds to its issue; another opens its own', async () => {
  const { handle, memoryStore } = await load()
  const forge = fakeForge()
  const store = memoryStore()
  const first = await handle(post(REPORT), {}, store, forge)
  assert.equal(first.status, 201)
  assert.equal(first.json.count, 1)
  assert.match(forge.issues[0].title, /^Crash: panic at src\/taskbar\.rs:1234:17 \(0xC0000409\) \[crash:[0-9a-f]{10}\]$/)
  assert.match(forge.issues[0].body, /\*\*Reports: 1\*\*/)
  // Another column of the same line: the same crash.
  const again = await handle(post({ ...REPORT, where: 'src\\taskbar.rs:1234:9', uptime: 12 }), {}, store, forge)
  assert.equal(again.status, 200)
  assert.equal(again.json.count, 2)
  assert.equal(forge.issues.length, 1)
  assert.equal(forge.issues[0].comments.length, 1)
  assert.match(forge.issues[0].body, /\*\*Reports: 2\*\* · versions: 0\.2\.2 ·/)
  const other = await handle(post({ ...REPORT, where: 'src\\pet.rs:10:5' }), {}, store, forge)
  assert.equal(other.status, 201)
  assert.equal(forge.issues.length, 2)
  // Not kept (another instance): found on the forge by its tag, its count read back.
  const fresh = await handle(post(REPORT), {}, memoryStore(), forge)
  assert.equal(fresh.json.count, 3)
  assert.equal(forge.issues.length, 2)
})

test('after so many comments only the count goes up, unless a new version', async () => {
  const { handle, memoryStore } = await load()
  const forge = fakeForge()
  const store = memoryStore()
  const env = { COMMENTS: '2' }
  for (let i = 0; i < 4; i++) await handle(post(REPORT), env, store, forge)
  assert.equal(forge.issues[0].comments.length, 1)
  assert.match(forge.issues[0].body, /\*\*Reports: 4\*\*/)
  await handle(post({ ...REPORT, app: '0.2.3' }), env, store, forge)
  assert.equal(forge.issues[0].comments.length, 2)
  assert.match(forge.issues[0].body, /versions: 0\.2\.2, 0\.2\.3/)
})

test('a closed issue: only counted, or a new one from a newer version', async () => {
  const { handle, memoryStore } = await load()
  const forge = fakeForge()
  const store = memoryStore()
  await handle(post(REPORT), {}, store, forge)
  forge.issues[0].state = 'closed'
  const old = await handle(post(REPORT), {}, store, forge)
  assert.equal(old.status, 200)
  assert.equal(old.json.closed, true)
  assert.equal(forge.issues.length, 1)
  assert.equal(forge.issues[0].comments.length, 0)
  const back = await handle(post({ ...REPORT, app: '0.3.0' }), {}, store, forge)
  assert.equal(back.status, 201)
  assert.equal(forge.issues.length, 2)
  assert.match(forge.issues[1].body, /Seen again after https:\/\/example\.test\/1 was closed/)
  // And from then on, to the new one.
  await handle(post({ ...REPORT, app: '0.3.0' }), {}, store, forge)
  assert.equal(forge.issues[1].comments.length, 1)
})

test('an issue since deleted is filed again', async () => {
  const { handle, memoryStore } = await load()
  const forge = fakeForge()
  const store = memoryStore()
  await handle(post(REPORT), {}, store, forge)
  forge.issues.length = 0
  const again = await handle(post(REPORT), {}, store, forge)
  assert.equal(again.status, 201)
  assert.equal(forge.issues.length, 1)
})

test('one address so many a day; so many new issues a day', async () => {
  const { handle, memoryStore } = await load()
  const forge = fakeForge()
  const store = memoryStore()
  const env = { PER_IP_DAY: '2', NEW_PER_DAY: '1' }
  assert.equal((await handle(post(REPORT), env, store, forge)).status, 201)
  assert.equal((await handle(post({ ...REPORT, where: 'src\\pet.rs:1:1' }, '198.51.100.7'), env, store, forge)).status, 429)
  assert.equal((await handle(post(REPORT), env, store, forge)).status, 200)
  assert.equal((await handle(post(REPORT), env, store, forge)).status, 429)
  // The binding, when there is one.
  const limited = { PER_IP: { limit: async () => ({ success: false }) } }
  assert.equal((await handle(post(REPORT, '192.0.2.1'), limited, memoryStore(), forge)).status, 429)
})

test('not from her, too big, not JSON, not a report: refused', async () => {
  const { handle, memoryStore } = await load()
  const forge = fakeForge()
  const store = memoryStore()
  assert.equal((await handle(post(REPORT, '1.1.1.1', {}), {}, store, forge)).status, 400)
  assert.equal((await handle(post('x'.repeat(50 * 1024)), {}, store, forge)).status, 413)
  assert.equal((await handle(post('{nope'), {}, store, forge)).status, 400)
  assert.equal((await handle(post({ ...REPORT, token: 'x' }), {}, store, forge)).status, 400)
  assert.equal((await handle({ method: 'GET', path: '/', header: () => null }, {}, store, forge)).status, 200)
  assert.equal((await handle({ method: 'GET', path: '/report', header: () => null }, {}, store, forge)).status, 405)
  assert.equal(forge.issues.length, 0)
  // The forge failing: 502, and she tries later.
  const broken = { ...fakeForge(), find: async () => Promise.reject(new Error('down')) }
  assert.equal((await handle(post(REPORT), {}, memoryStore(), broken)).status, 502)
})

test('an issue mentions nobody and its code blocks stay closed', async () => {
  const { handle, memoryStore } = await load()
  const forge = fakeForge()
  const r = { ...REPORT, message: '@zhoupengjie ~~~ fixes #1', where: 'a|b`c@d' }
  await handle(post(r), {}, memoryStore(), forge)
  const { title, body } = forge.issues[0]
  assert.ok(!/@zhoupengjie/.test(body))
  assert.ok(body.includes('@\u200bzhoupengjie'))
  assert.equal((body.match(/^~~~/gm) || []).length % 2, 0)
  assert.ok(!title.includes('`') && !title.includes('@'))
  assert.ok(body.includes('a\\|b'))
})

test('a GitHub App signs its JWT with the key GitHub gives (PKCS#1)', async () => {
  const { appJwt } = await load()
  const { privateKey, publicKey } = crypto.generateKeyPairSync('rsa', { modulusLength: 2048, privateKeyEncoding: { type: 'pkcs1', format: 'pem' }, publicKeyEncoding: { type: 'spki', format: 'pem' } })
  const jwt = await appJwt('123', privateKey, 1_800_000_000)
  const [head, claims, sig] = jwt.split('.')
  const json = s => JSON.parse(Buffer.from(s, 'base64url').toString())
  assert.deepEqual(json(head), { alg: 'RS256', typ: 'JWT' })
  assert.deepEqual(json(claims), { iat: 1_799_999_940, exp: 1_800_000_540, iss: '123' })
  assert.ok(crypto.verify('RSA-SHA256', Buffer.from(`${head}.${claims}`), publicKey, Buffer.from(sig, 'base64url')))
})

test('GitHub and Gitee are asked as each wants', async () => {
  const { github, gitee } = await load()
  const asked = []
  const fetcher = async (url, init: { method?: string, headers?: Record<string, string>, body?: string } = {}) => {
    asked.push({ url, method: init.method || 'GET', headers: init.headers, body: init.body && JSON.parse(init.body) })
    return { ok: true, status: 201, json: async () => ({ number: 7, html_url: 'u', state: 'open', body: '' }), text: async () => '' }
  }
  await github({ REPO: 'zhoupengjie/Wakuwaku', GITHUB_TOKEN: 't' }, fetcher).create('T', 'B')
  assert.equal(asked[0].url, 'https://api.github.com/repos/zhoupengjie/Wakuwaku/issues')
  assert.equal(asked[0].method, 'POST')
  assert.equal(asked[0].headers.authorization, 'Bearer t')
  assert.deepEqual(asked[0].body, { title: 'T', body: 'B', labels: ['crash'] })
  await gitee({ REPO: 'zhoupengjie/Wakuwaku', GITEE_TOKEN: 'g' }, fetcher).create('T', 'B')
  assert.equal(asked[1].url, 'https://gitee.com/api/v5/repos/zhoupengjie/issues')
  assert.equal(asked[1].headers.authorization, 'token g')
  assert.deepEqual(asked[1].body, { repo: 'Wakuwaku', title: 'T', body: 'B' })
  await gitee({ REPO: 'zhoupengjie/Wakuwaku', GITEE_TOKEN: 'g' }, fetcher).comment('I8ABC1', 'C')
  assert.equal(asked[2].url, 'https://gitee.com/api/v5/repos/zhoupengjie/Wakuwaku/issues/I8ABC1/comments')
})
