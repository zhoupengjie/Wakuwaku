// A letter's HTML on the Mail page (settings-letter.ts, src/pet/letter.html
// and letter-pictures.html, zhoupengjie/Wakuwaku#129): which letters ask
// for something from elsewhere, and the pages that keep it out or let it in.
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { test } from 'node:test'
import { hasRemote } from '../src/pet/settings-letter.ts'

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
