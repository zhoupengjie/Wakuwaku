// What her pages and the Rust side call each other by: the commands the
// pages ask for (invoke) and the events the Rust side sends them (emit),
// each a name written once on either side with nothing else between them.
// A name that is on one side alone fails nothing as she runs: bridge.ts
// answers a command that is not there with false (.catch), and a listener
// for an event nobody sends just never hears. So both sides' source is
// read here, as config.test.ts reads the settings' keys, and the names
// compared.
//
// Names alone: what a command takes and gives back, and what an event
// carries, is not looked at.
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { test } from 'node:test'

const root = path.join(import.meta.dirname, '..')
const read = (...p: string[]) => fs.readFileSync(path.join(root, ...p), 'utf8').replace(/\r\n/g, '\n')

// Every file under a folder with one of these endings, by its path from the root (forward slashes).
function files(dir: string, ...endings: string[]): string[] {
  const out: string[] = []
  const walk = (at: string) => {
    for (const e of fs.readdirSync(path.join(root, at), { withFileTypes: true })) {
      const p = `${at}/${e.name}`
      if (e.isDirectory()) walk(p)
      else if (endings.some(x => e.name.endsWith(x))) out.push(p)
    }
  }
  walk(dir)
  return out.sort()
}

const only = (a: Iterable<string>, b: Set<string>) => [...new Set(a)].filter(x => !b.has(x)).sort()

// --- The Rust side -----------------------------------------------------------

const RUST = files('src-tauri/src', '.rs').map(file => ({ file, text: read(file) }))

// The commands main.rs hands to Tauri: generate_handler![a::b::name, ...], by their names.
function registered(): Set<string> {
  const main = read('src-tauri', 'src', 'main.rs')
  const from = main.indexOf('generate_handler![')
  assert.ok(from > 0, 'main.rs: generate_handler![ found')
  const block = main.slice(from, main.indexOf('])', from))
  return new Set([...block.matchAll(/^\s+((?:[a-z_]+::)*[a-z_]+),/gm)].map(m => m[1].split('::').pop()!))
}

// The functions written as commands: #[tauri::command] over a fn, anywhere.
function written(): Map<string, string> {
  const out = new Map<string, string>()
  for (const { file, text } of RUST) {
    for (const m of text.matchAll(/#\[tauri::command[^\]]*\]\s*(?:#\[[^\]]*\]\s*)*pub(?:\([a-z]+\))? (?:async )?fn (\w+)/g)) out.set(m[1], file)
  }
  return out
}

// An event's name: a namespace, a colon, words with dashes ("taskbar:windows").
const EVENT = '[a-z]+:[a-z]+(?:-[a-z]+)*'

// Every string of that shape the Rust side has: the events it sends are
// among them, however the name reaches the emit (a call, a constant, a
// helper of a module's own). Its tests' strings are among them too (they
// sit anywhere in a file); one of an event's shape that is not an event,
// in a namespace the pages listen in, is said in NOT_EVENTS below.
function rustNames(): Map<string, string> {
  const out = new Map<string, string>()
  for (const { file, text } of RUST) {
    for (const m of text.matchAll(new RegExp(`"(${EVENT})"`, 'g'))) if (!out.has(m[1])) out.set(m[1], file)
  }
  return out
}

// --- The pages ---------------------------------------------------------------

// addons.ts's invoke is not Tauri's: bridge.ts gives it one that asks
// addon_call, and what it names ('widget', 'stop') is that command's
// `what`. sdk.ts runs in a plugin's frame and asks nothing of Tauri.
const NOT_TAURIS = new Set(['src/pet/addons.ts', 'src/addon/sdk.ts'])
const PAGES = files('src', '.ts').map(file => ({ file, text: read(file) }))

// The commands the pages ask for: invoke('name', bridge.ts's send('name' (one
// at a time, in order) and bar.ts's call('name' (a failure let go).
function asked(): Map<string, string> {
  const out = new Map<string, string>()
  for (const { file, text } of PAGES) {
    if (NOT_TAURIS.has(file)) continue
    for (const m of text.matchAll(/(?<![.\w])(?:invoke|send|call)\(\s*'([a-z_]+)'/g)) if (!out.has(m[1])) out.set(m[1], file)
  }
  return out
}

// The events the pages listen for: Tauri's listen('name', and bridge.ts's on('name').
function heard(): Map<string, string> {
  const out = new Map<string, string>()
  for (const { file, text } of PAGES) {
    for (const m of text.matchAll(new RegExp(`(?<![.\\w])(?:listen|on)\\(\\s*'(${EVENT})'`, 'g'))) if (!out.has(m[1])) out.set(m[1], file)
  }
  return out
}

// --- Compared ----------------------------------------------------------------

test('the readers find what is there', () => {
  // Each reader is a pattern over source: one that stops matching would pass everything below.
  assert.ok(registered().size > 100, `commands registered: ${registered().size}`)
  assert.ok(written().size > 100, `commands written: ${written().size}`)
  assert.ok(asked().size > 100, `commands asked for: ${asked().size}`)
  assert.ok(heard().size > 50, `events listened for: ${heard().size}`)
  assert.ok(rustNames().size > 50, `event-shaped names on the Rust side: ${rustNames().size}`)
  for (const known of ['pet_ready', 'settings_set', 'desk_ready', 'bar_ready', 'market']) {
    assert.ok(registered().has(known), `${known} registered`)
    assert.ok(asked().has(known), `${known} asked for`)
  }
  for (const known of ['pet:update', 'settings:changed']) {
    assert.ok(heard().has(known), `${known} listened for`)
    assert.ok(rustNames().has(known), `${known} on the Rust side`)
  }
})

test('every command a page asks for is one the Rust side registers', () => {
  const there = registered()
  const lost = [...asked()].filter(([name]) => !there.has(name)).map(([name, file]) => `${name} (${file})`)
  assert.deepEqual(lost, [], 'asked for by a page, not in main.rs generate_handler!: it would answer false, or nothing')
})

test('every command the Rust side registers is asked for by a page', () => {
  // One no page asks for is dead, or its page's call was renamed away from it.
  assert.deepEqual(only(registered(), new Set(asked().keys())), [], 'in main.rs generate_handler!, asked for by no page')
})

test('every function written as a command is registered, and nothing else is', () => {
  const there = registered()
  const made = written()
  assert.deepEqual([...made].filter(([name]) => !there.has(name)).map(([name, file]) => `${name} (${file})`), [], '#[tauri::command] but not in generate_handler!: no page can reach it')
  assert.deepEqual(only(there, new Set(made.keys())), [], 'in generate_handler! with no #[tauri::command] fn of that name')
})

test('every event a page listens for is one the Rust side names', () => {
  const there = rustNames()
  const lost = [...heard()].filter(([name]) => !there.has(name)).map(([name, file]) => `${name} (${file})`)
  assert.deepEqual(lost, [], 'listened for by a page, written nowhere on the Rust side: it never comes')
})

// Strings of an event's shape on the Rust side, in a namespace the pages
// listen in, that are not events: each with what it is.
const NOT_EVENTS: Record<string, string> = {}

test('every event the Rust side names is listened for by a page', () => {
  const listened = heard()
  const spaces = new Set([...listened.keys()].map(name => name.split(':')[0]))
  const unheard = [...rustNames()].filter(([name]) => spaces.has(name.split(':')[0]) && !listened.has(name) && !(name in NOT_EVENTS)).map(([name, file]) => `${name} (${file})`)
  assert.deepEqual(unheard, [], 'named on the Rust side, listened for by no page: sent to nobody, or its listener was renamed away (not an event at all: NOT_EVENTS here)')
  // The list above kept to what it is for.
  const names = rustNames()
  assert.deepEqual(Object.keys(NOT_EVENTS).filter(name => !names.has(name) || listened.has(name)), [], 'NOT_EVENTS: no longer on the Rust side, or listened for now')
})

// --- The preview's stand-in for Tauri (tools/preview) ---------------------------

test("the preview's stand-in answers only commands there are, and sends only events the pages hear", () => {
  const fake = read('tools', 'preview', 'fake-tauri.js')
  const from = fake.indexOf('const ANSWERS = {')
  assert.ok(from > 0, 'fake-tauri.js: ANSWERS found')
  const block = fake.slice(from, fake.indexOf('\n  }\n', from))
  const answers = [...block.matchAll(/^ {4}(?:async )?([a-z_]+)\s*(?::|\()/gm)].map(m => m[1])
  assert.ok(answers.length > 30, `the stand-in's answers: ${answers.length}`)
  const there = registered()
  assert.deepEqual(answers.filter(name => !there.has(name)), [], 'answered by fake-tauri.js ANSWERS, not a command of hers: the scene shows what she cannot do')
  // Its scenes' own answers (Fake.handle('name', ...)) and what they send (emit('name', ...)).
  const listened = heard()
  for (const file of files('tools/preview', '.js')) {
    const text = read(file)
    const handled = [...text.matchAll(/\.handle\(\s*'([a-z_]+)'/g)].map(m => m[1])
    assert.deepEqual(handled.filter(name => !there.has(name)), [], `${file}: handle() for a command there is not`)
    const sent = [...text.matchAll(new RegExp(`(?<![.\\w])emit\\(\\s*'(${EVENT})'`, 'g'))].map(m => m[1])
    assert.deepEqual(sent.filter(name => !listened.has(name)), [], `${file}: emit() of an event no page listens for`)
  }
})
