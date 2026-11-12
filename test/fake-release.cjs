// A GitHub and a Gitee on 127.0.0.1 for trying the pet's updates without
// publishing anything (src-tauri/src/updates.rs): GitHub's latest release
// as its API gives it; Gitee as the pet reads it without its API, its tags
// as git lists them (/gitee.git/info/refs) and its release's files where
// Gitee keeps them (/gitee/releases/download/<tag>/<file>), sent on to its
// storage by a redirect as gitee.com does.
//
//   node test/fake-release.cjs --exe <a release exe> [--version 0.2.2] [--port 47390]
//
// Start an older exe (outside the source tree) with
//   WAKUWAKU_UPDATE_URL=http://127.0.0.1:47390/github/latest
//   WAKUWAKU_UPDATE_GITEE_URL=http://127.0.0.1:47390/gitee/
// (or only one of them: then only that place is asked).
//
// What goes wrong is set while it runs, GET /mode?<name>=1 (0 to undo; /mode
// alone says them all), so a pet already running needs no restart:
//   githubDown, giteeDown    GitHub's API, Gitee's tags answer 503
//   githubEmpty, giteeEmpty  no release yet (GitHub 404, Gitee no tags)
//   githubOld, giteeOld      that place's newest is 0.0.1 (lagging behind;
//                            on Gitee, a tag with no release)
//   githubFail, giteeFail    the exe's download answers 500
//   githubSlow, giteeSlow    the exe comes at 64 KB a second
//   giteeBad                 Gitee's exe is another (one byte changed)
//   noDigest                 GitHub gives no digest
//   noSums                   Gitee has no .sha256 (its release not made yet)
//   badSums                  Gitee's .sha256 says another SHA-256
// Every request is logged.

const http = require('http')
const fs = require('fs')
const crypto = require('crypto')

const arg = (name, fallback) => {
  const i = process.argv.indexOf(`--${name}`)
  return i > 0 ? process.argv[i + 1] : fallback
}
const exePath = arg('exe')
if (!exePath) {
  console.error('usage: node test/fake-release.cjs --exe <a release exe> [--version 0.2.2] [--port 47390]')
  process.exit(2)
}
const version = arg('version', '0.2.2').replace(/^v/i, '')
const port = Number(arg('port', 47390))
const exe = fs.readFileSync(exePath)
const bad = Buffer.from(exe)
bad[bad.length - 1] ^= 1
const sha = buf => crypto.createHash('sha256').update(buf).digest('hex')
const digest = sha(exe)
const name = `wakuwaku-v${version}-windows-x64.exe`
const base = `http://127.0.0.1:${port}`

const mode = {}
const MODES = ['githubDown', 'giteeDown', 'githubEmpty', 'giteeEmpty', 'githubOld', 'giteeOld', 'githubFail', 'giteeFail', 'githubSlow', 'giteeSlow', 'giteeBad', 'noDigest', 'noSums', 'badSums']

function github() {
  const v = mode.githubOld ? '0.0.1' : version
  const file = `wakuwaku-v${v}-windows-x64.exe`
  return {
    tag_name: `v${v}`, name: `Wakuwaku v${v}`, draft: false, prerelease: false,
    html_url: `${base}/github/page/v${v}`,
    body: 'Fake release notes.',
    assets: [{ name: file, size: exe.length, digest: mode.noDigest ? null : `sha256:${digest}`, browser_download_url: `${base}/github/dl/${file}` }],
  }
}

// Gitee's tags as git's smart HTTP lists them: pkt-lines, a peeled line
// after each annotated tag, a prerelease and a tag that is no version among them.
function giteeRefs() {
  const sha = 'a'.repeat(40)
  const pkt = line => (line.length + 4).toString(16).padStart(4, '0') + line
  const tags = mode.giteeEmpty ? [] : mode.giteeOld ? ['v0.0.1'] : ['v0.0.1', `v${version}`, `v${version}-beta`, 'nightly']
  let out = pkt('# service=git-upload-pack\n') + '0000'
  out += pkt(`${sha} HEAD\0multi_ack thin-pack side-band side-band-64k ofs-delta symref=HEAD:refs/heads/main agent=git/2\n`)
  out += pkt(`${sha} refs/heads/main\n`)
  for (const tag of tags) out += pkt(`${sha} refs/tags/${tag}\n`) + pkt(`${sha} refs/tags/${tag}^{}\n`)
  return out + '0000'
}

function send(res, status, body, type = 'application/json') {
  const buf = Buffer.isBuffer(body) ? body : Buffer.from(typeof body === 'string' ? body : JSON.stringify(body))
  res.writeHead(status, { 'content-type': type, 'content-length': buf.length })
  res.end(buf)
}

// The exe, whole or 64 KB a second.
function file(res, buf, slow) {
  res.writeHead(200, { 'content-type': 'application/octet-stream', 'content-length': buf.length })
  if (!slow) return res.end(buf)
  let at = 0
  const timer = setInterval(() => {
    if (res.destroyed) return clearInterval(timer)
    res.write(buf.subarray(at, at + 65536))
    at += 65536
    if (at >= buf.length) clearInterval(timer), res.end()
  }, 1000)
}

http
  .createServer((req, res) => {
    const url = new URL(req.url, base)
    const path = decodeURIComponent(url.pathname)
    console.log(new Date().toISOString().slice(11, 19), req.method, req.url, req.headers['user-agent'] || '')
    if (path === '/mode') {
      for (const [k, v] of url.searchParams) if (MODES.includes(k)) mode[k] = v !== '0'
      return send(res, 200, Object.fromEntries(MODES.map(k => [k, !!mode[k]])))
    }
    if (path === '/github/latest') {
      if (mode.githubDown) return send(res, 503, { message: 'down' })
      if (mode.githubEmpty) return send(res, 404, { message: 'Not Found' })
      return send(res, 200, github())
    }
    if (path === '/gitee.git/info/refs') {
      if (mode.giteeDown) return send(res, 503, 'down', 'text/plain')
      return send(res, 200, giteeRefs(), 'application/x-git-upload-pack-advertisement')
    }
    if (path.startsWith('/github/page/') || path.startsWith('/gitee/releases/tag/')) return send(res, 200, `<h1>${path}</h1>`, 'text/html')
    if (path === `/github/dl/${name}`) {
      if (mode.githubFail) return send(res, 500, 'failed', 'text/plain')
      return file(res, exe, mode.githubSlow)
    }
    // Gitee: a release's file sent on to its storage, as gitee.com does;
    // one that is not there, 404 at once.
    const dl = path.match(/^\/gitee\/releases\/download\/([^/]+)\/([^/]+)$/)
    if (dl) {
      const [, tag, leaf] = dl
      const there = tag === `v${version}` && (leaf === name || (leaf === `${name}.sha256` && !mode.noSums))
      if (!there) return send(res, 404, '<h1>404</h1>', 'text/html')
      res.writeHead(302, { location: `${base}/gitee/store/${encodeURIComponent(leaf)}?token=x` })
      return res.end()
    }
    if (path === `/gitee/store/${name}`) {
      if (mode.giteeFail) return send(res, 500, 'failed', 'text/plain')
      return file(res, mode.giteeBad ? bad : exe, mode.giteeSlow)
    }
    if (path === `/gitee/store/${name}.sha256`) return send(res, 200, `${mode.badSums ? sha(bad) : digest}  ${name}\n`, 'application/octet-stream')
    send(res, 404, { message: '404 Not Found' })
  })
  .listen(port, '127.0.0.1', () => {
    console.log(`fake GitHub and Gitee on ${base}: v${version}, ${name}, SHA-256 ${digest}`)
    console.log(`  WAKUWAKU_UPDATE_URL=${base}/github/latest`)
    console.log(`  WAKUWAKU_UPDATE_GITEE_URL=${base}/gitee/`)
  })
