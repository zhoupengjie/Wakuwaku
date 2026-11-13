// A letter's HTML on the Mail page (settings-letter.ts, src/pet/letter.html
// and letter-pictures.html, zhoupengjie/Wakuwaku#129): which letters ask
// for something from elsewhere, and the pages that keep it out or let it in.
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { test } from 'node:test'
import { hasRemote, linkOf, linkWarning, textWithLinks } from '../src/pet/settings-letter.ts'

const read = (...p: string[]) => fs.readFileSync(path.join(import.meta.dirname, '..', ...p), 'utf8').replace(/\r\n/g, '\n')

test('a letter asks for something from elsewhere: a picture, a background, a url() in its styles', () => {
  for (const html of [
    '<img src="http://images.example.test/header.png">',
    "<img class=x src='https://track.example.test/spacer.cfm?id=1' width=1 height=1>",
    '<img src=https://x.test/a.png>',
    '<img src="//cdn.example.test/a.png">',
    '<img srcset="https://x.test/a.png 2x">',
    '<table background="http://x.test/bg.jpg">',
    '<video poster="https://x.test/p.jpg"></video>',
    '<div style="background:url(https://x.test/bg.png)">',
    "<div style=\"background-image: url('http://x.test/bg.png')\">",
    '<div style="background:url(&quot;https://x.test/bg.png&quot;)">',
    '<style>.hero { background: url( "//x.test/bg.png" ) }</style>',
    '<IMG SRC = "HTTPS://X.TEST/A.PNG">',
  ]) {
    assert.ok(hasRemote(html), html)
  }
})

test('a letter with only its own pictures, links, or words about addresses asks for nothing', () => {
  for (const html of [
    '<p>Hello</p>',
    '<img src="data:image/png;base64,iVBORw0KGgo=">',
    '<a href="https://example.test/">a link</a>',
    '<p>See https://example.test/a.png for the picture</p>',
    '<div style="color:red">x</div>',
    '<img src="about:blank">',
  ]) {
    assert.ok(!hasRemote(html), html)
  }
})

test('the letter pages: no pictures from elsewhere in one, only those in the other; nothing runs in either', () => {
  const policy = (file: string) => read('src', 'pet', file).match(/http-equiv="Content-Security-Policy"\s+content="([^"]+)"/)?.[1]
  const none = policy('letter.html')
  const pictures = policy('letter-pictures.html')
  assert.equal(none, "default-src 'none'; img-src data:; style-src 'unsafe-inline'; font-src data:")
  assert.equal(pictures, "default-src 'none'; img-src http: https: data:; style-src 'unsafe-inline'; font-src data:")
  for (const file of ['letter.html', 'letter-pictures.html']) {
    const page = read('src', 'pet', file)
    assert.match(page, /<html data-letter>/, file)
    assert.match(page, /<meta name="referrer" content="no-referrer" \/>/, file)
    assert.doesNotMatch(page, /<script/, file)
  }
  // The island's page may frame them (its own origin), and its frame guard
  // lets its frames go to them (addons/files.rs is_letter_page).
  assert.match(read('src', 'pet', 'index.html'), /frame-src 'self' /)
  const guard = read('src-tauri', 'src', 'addons', 'files.rs')
  assert.match(guard, /LETTER_PAGES: \[&str; 2\] = \["\/pet\/letter\.html", "\/pet\/letter-pictures\.html"\]/)
  // The frame's sandbox: no scripts.
  const letter = read('src', 'pet', 'settings-letter.ts')
  assert.match(letter, /<iframe class="m-html" src="\$\{page\}" data-doc="\$\{esc\(id\)\}" sandbox="allow-same-origin" /)
})

// A letter's links (zhoupengjie/Wakuwaku#150): what a click opens, what is
// asked about first, and the addresses in a plain letter.
test('a link is a web page to open, an address to write to, or nothing', () => {
  assert.deepEqual(linkOf('https://example.test/a?b=1#c'), { kind: 'web', url: 'https://example.test/a?b=1#c' })
  assert.deepEqual(linkOf(' HTTP://Example.TEST '), { kind: 'web', url: 'http://example.test/' })
  // As the URL writes it: what a browser's command line could take for its end, escaped.
  assert.equal((linkOf('https://example.test/a"b c') as { url: string }).url, 'https://example.test/a%22b%20c')
  for (const href of ['', '#top', '/relative', '//example.test/x', 'cid:part1', 'tel:+4930123', 'about:blank', 'javascript:alert(1)', 'file:///C:/x.exe', 'ms-settings:privacy', 'https://', 'https://[nope/']) {
    assert.equal(linkOf(href), null, href)
  }
  assert.deepEqual(linkOf('mailto:sales@example.test'), { kind: 'mail', to: 'sales@example.test', cc: '', bcc: '', subject: '', text: '' })
  assert.deepEqual(linkOf('MAILTO:a@example.test,b@example.test?Subject=%E4%BD%A0%E5%A5%BD&cc=c@example.test&body=line%201%0D%0Aline%202&to=d@example.test'), {
    kind: 'mail',
    to: 'a@example.test,b@example.test, d@example.test',
    cc: 'c@example.test',
    bcc: '',
    subject: '你好',
    text: 'line 1\nline 2',
  })
  // A subject is one line; a broken escape is itself.
  assert.equal((linkOf('mailto:?subject=a%0Ab%ZZ') as { subject: string }).subject, 'a b%ZZ')
  assert.equal(linkOf('mailto:'), null)
})

test('a web link is asked about when it is not where its words say, or goes to a number or a look-alike name', () => {
  // Fine: words that are no address, the same site, a site under it.
  for (const [url, text] of [
    ['https://example.test/x', '点击这里'],
    ['https://www.paypal.com/signin', 'paypal.com'],
    ['https://paypal.com/', 'https://www.paypal.com'],
    ['https://mail.google.com/', 'google.com'],
    ['https://news.example.com/a', 'Visit example.org today'],
    ['https://files.example.test/invoice.pdf', 'invoice.pdf'],
    ['https://example.test/', 'photo.zip'],
    ['https://example.test/', 'v1.2'],
    ['https://example.test/', 'support@paypal.com'],
    ['http://example.test/', ''],
  ]) {
    assert.equal(linkWarning(url, text), null, `${url} ${text}`)
  }
  assert.deepEqual(linkWarning('https://evil.test/login', 'https://www.paypal.com/login'), { why: 'differs', shown: 'www.paypal.com', real: 'evil.test' })
  assert.deepEqual(linkWarning('https://evil.test/', ' paypal.com. '), { why: 'differs', shown: 'paypal.com', real: 'evil.test' })
  assert.deepEqual(linkWarning('https://paypal.com.evil.test/', 'paypal.com'), { why: 'differs', shown: 'paypal.com', real: 'paypal.com.evil.test' })
  assert.deepEqual(linkWarning('https://u1.ct.sendgrid.net/ls/click?upn=1', 'github.com/settings'), { why: 'differs', shown: 'github.com', real: 'u1.ct.sendgrid.net' })
  // A name before an @ is not where it goes.
  assert.deepEqual(linkWarning('https://paypal.com@evil.test/', '登录'), { why: 'differs', shown: 'paypal.com', real: 'evil.test' })
  assert.deepEqual(linkWarning('http://192.168.1.1/admin', 'Router'), { why: 'ip', real: '192.168.1.1' })
  assert.deepEqual(linkWarning('http://3232235777/', 'x'), { why: 'ip', real: '192.168.1.1' })
  assert.deepEqual(linkWarning('http://[::1]:8080/', 'x'), { why: 'ip', real: '[::1]' })
  // аpple.com with a Cyrillic а.
  assert.deepEqual(linkWarning('https://\u0430pple.com/', 'Apple'), { why: 'idn', real: 'xn--pple-43d.com' })
  assert.equal(linkWarning('mailto:a@example.test', 'evil.test'), null)
})

test('a plain letter\'s web addresses are links, without the stop after them', () => {
  const esc = (v: unknown) => String(v).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')
  const link = (url: string) => `<a class="m-link" data-mail-link="${esc(url)}">${esc(url)}</a>`
  assert.equal(textWithLinks('no links <here>', esc), 'no links &lt;here&gt;')
  assert.equal(textWithLinks('See https://example.test/a?b=1&c=2.', esc), `See ${link('https://example.test/a?b=1&c=2')}.`)
  assert.equal(textWithLinks('（详见https://example.test/x）。下一行\nhttp://b.test', esc), `（详见${link('https://example.test/x')}）。下一行\n${link('http://b.test')}`)
  assert.equal(textWithLinks('(https://en.wikipedia.org/wiki/Rust_(language))', esc), `(${link('https://en.wikipedia.org/wiki/Rust_(language)')})`)
  assert.equal(textWithLinks('<https://example.test/"x">', esc), `&lt;${link('https://example.test/')}&quot;x&quot;&gt;`)
  assert.equal(textWithLinks('http:// and https://.', esc), 'http:// and https://.')
  assert.equal(textWithLinks(null, esc), '')
})
