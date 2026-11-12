import assert from 'node:assert/strict'
import { test } from 'node:test'
import { pressOf, nextWindow, DRAG_HOLD_MS } from '../src/pet/presses.ts'

const click = { type: 'click', button: 0 }
const middle = { type: 'auxclick', button: 1 }
const right = { type: 'contextmenu', button: 2 }
const none = { shift: false, ctrl: false }
const shift = { shift: true, ctrl: false }
const ctrl = { shift: false, ctrl: true }
const both = { shift: true, ctrl: true }

test('a plain press is a press, whatever the program has open', () => {
  for (const open of [0, 1, 3] as any[]) assert.equal(pressOf(click, none, open), 'press', open)
  // The keys not known: as if none were held.
  assert.equal(pressOf(click, null, 2), 'press')
})

test('the middle button and Shift start another, Ctrl and Shift as administrator', () => {
  for (const open of [0, 1, 3] as any[]) {
    assert.equal(pressOf(middle, none, open), 'launch', open)
    assert.equal(pressOf(click, shift, open), 'launch', open)
    assert.equal(pressOf(click, both, open), 'admin', open)
  }
  // The middle button starts another whatever is held.
  assert.equal(pressOf(middle, both, 1), 'launch')
})

test('Ctrl goes along the windows of a program with several; with one or none it is a press', () => {
  assert.equal(pressOf(click, ctrl, 3), 'next')
  assert.equal(pressOf(click, ctrl, 2), 'next')
  assert.equal(pressOf(click, ctrl, 1), 'press')
  assert.equal(pressOf(click, ctrl, 0), 'press')
})

test('a right press is its menu; with Shift its window menu while it has windows', () => {
  assert.equal(pressOf(right, none, 2), 'menu')
  assert.equal(pressOf(right, ctrl, 2), 'menu')
  assert.equal(pressOf(right, shift, 1), 'windowMenu')
  assert.equal(pressOf(right, both, 3), 'windowMenu')
  // Pinned and not running: its menu, there being no window to have one.
  assert.equal(pressOf(right, shift, 0), 'menu')
})

test('other buttons do nothing (the right one has its menu)', () => {
  assert.equal(pressOf({ type: 'auxclick', button: 2 }, none, 1), null)
  assert.equal(pressOf({ type: 'auxclick', button: 3 }, shift, 1), null)
})

test('Ctrl and a press go along the windows in the list order, round again', () => {
  const ws = front => [1, 2, 3].map(id => ({ id, front: id === front }))
  assert.equal(nextWindow(ws(1)).id, 2)
  assert.equal(nextWindow(ws(2)).id, 3)
  assert.equal(nextWindow(ws(3)).id, 1)
  // None of them in front: the first.
  assert.equal(nextWindow(ws(0)).id, 1)
  // The one just brought, its mark in front not come yet: after it.
  assert.equal(nextWindow(ws(1), 2).id, 3)
  assert.equal(nextWindow(ws(1), '3').id, 1)
  // One no longer there: after the one in front.
  assert.equal(nextWindow(ws(2), 9).id, 3)
  assert.equal(nextWindow([]), null)
  assert.equal(nextWindow(undefined), null)
})

test('a file is held over a button about half a second', () => {
  assert.equal(DRAG_HOLD_MS, 500)
})
