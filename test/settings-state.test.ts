// Where the settings are (src/pet/settings-state.ts): the events settings.ts
// and its parts send, and the state they leave.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import * as S from '../src/pet/settings-state.ts'
import type { SettingsEvent, SettingsState } from '../src/pet/settings-state.ts'

const run = (events: SettingsEvent[], from: SettingsState = S.initial()) => events.reduce(S.next, from)
const open = (to: string, { market = false, now = 1000 } = {}): SettingsEvent => ({ type: 'open', to, market, now })

test('opened at a page, a part of one by its old name, a widget row, or the market', () => {
  assert.deepEqual([run([open('mail')]).tab, run([open('mail')]).isOpen, run([open('mail')]).openedAt], ['mail', true, 1000])
  const pets = run([open('pets')])
  assert.deepEqual([pets.tab, pets.partToShow], ['look', 'pets'])
  const net = run([open('net')])
  assert.deepEqual([net.tab, net.partToShow], ['now', 'net'])
  const row = run([open('widgets:weather')])
  assert.deepEqual([row.tab, row.partToShow], ['widgets', 'widgets:weather'])
  const market = run([open('widgets:market:pet', { market: true })])
  assert.deepEqual([market.tab, market.sub, market.partToShow], ['widgets', { page: 'market', back: null }, ''])
  // Nothing known: open on the page it was on.
  assert.equal(run([{ type: 'tab', to: 'agents' }, open('nothing')]).tab, 'agents')
})

test('asked for again while open: the page changes, not when they opened', () => {
  const s = run([open('now', { now: 1000 }), open('look', { now: 5000 })])
  assert.deepEqual([s.tab, s.openedAt], ['look', 1000])
  const again = run([{ type: 'close' }, open('look', { now: 9000 })], s)
  assert.equal(again.openedAt, 9000)
})

test('connecting: the Agent page, or the welcome at General while its card shows', () => {
  const asked = run([open('connect')])
  assert.deepEqual([asked.tab, asked.partToShow, asked.welcomeOpen], ['agents', 'connect', true])
  const card = S.next(asked, { type: 'shown', cardShows: true })
  assert.deepEqual([card.tab, card.partToShow], ['now', 'welcome'])
  assert.equal(S.next(asked, { type: 'shown', cardShows: false }), asked)
  // Only for connecting.
  const pets = run([open('pets')])
  assert.equal(S.next(pets, { type: 'shown', cardShows: true }), pets)
})

test('closed: the welcome folded, the Agent page and the Plugins page as themselves next time', () => {
  const busy = run([open('connect'), { type: 'agent', id: 'codex', keep: false }, { type: 'others' }, { type: 'tab', to: 'widgets' }, { type: 'sub', sub: { page: 'addon', id: 'x' } }])
  const closed = S.next(busy, { type: 'close' })
  assert.deepEqual(
    [closed.isOpen, closed.welcomeOpen, closed.agentOpen, closed.othersOpen, closed.sub],
    [false, false, '', false, null],
  )
  // The page stays what it was, for the next open.
  assert.equal(closed.tab, 'widgets')
})

test('tabs: a new one closes what was opened in the Plugins page; the same, or an unknown one, nothing', () => {
  const sub = run([{ type: 'tab', to: 'widgets' }, { type: 'sub', sub: { page: 'market' } }])
  assert.equal(S.next(sub, { type: 'tab', to: 'widgets' }), sub)
  assert.equal(S.next(sub, { type: 'tab', to: 'pets' }), sub)
  const look = S.next(sub, { type: 'tab', to: 'look' })
  assert.deepEqual([look.tab, look.sub], ['look', null])
  // Gone to from within: a page and the part of it to scroll to.
  const toLook = S.next(sub, { type: 'page', tab: 'look', part: 'paper' })
  assert.deepEqual([toLook.tab, toLook.sub, toLook.partToShow], ['look', null, 'paper'])
  assert.equal(S.next({ ...sub, partToShow: 'x' }, { type: 'page', tab: 'now' }).partToShow, 'x')
  assert.equal(run([{ type: 'part', part: 'net' }, { type: 'part-shown' }]).partToShow, '')
})

test('the Plugins page: parts opened over each other and back, one taken away goes', () => {
  const market = run([{ type: 'sub', sub: { page: 'market' } }])
  const item = S.next(market, { type: 'sub', sub: { page: 'mkItem', id: 'clock' } })
  assert.deepEqual(item.sub, { page: 'mkItem', id: 'clock', back: { page: 'market', back: null } })
  assert.deepEqual(S.next(item, { type: 'back' }).sub, market.sub)
  assert.equal(run([{ type: 'back' }, { type: 'back' }], item).sub, null)
  assert.equal(S.next(item, { type: 'sub-off' }).sub, null)
  const addon = run([{ type: 'sub', sub: { page: 'addon', id: 'board' } }])
  assert.equal(S.next(addon, { type: 'gone', id: 'board' }).sub, null)
  assert.equal(S.next(addon, { type: 'gone', id: 'other' }), addon)
  const pack = run([{ type: 'sub', sub: { page: 'pack', kind: 'pet', id: 'p1' } }])
  assert.equal(S.next(pack, { type: 'gone', id: 'p1' }).sub, null)
  // The market's own page is not a plugin's.
  assert.equal(S.next(run([{ type: 'sub', sub: { page: 'mkItem', id: 'board' } }]), { type: 'gone', id: 'board' }).sub?.page, 'mkItem')
})

test('the Agent page: one row open at a time, kept open by its keep, a failed one opened there alone', () => {
  const codex = run([{ type: 'tab', to: 'agents' }, { type: 'agent', id: 'codex', keep: false }])
  assert.equal(codex.agentOpen, 'codex')
  assert.equal(S.next(codex, { type: 'agent', id: 'codex', keep: false }).agentOpen, '')
  assert.equal(S.next(codex, { type: 'agent', id: 'codex', keep: true }).agentOpen, 'codex')
  assert.equal(S.next(codex, { type: 'agent', id: 'agy', keep: false }).agentOpen, 'agy')
  assert.equal(S.next(codex, { type: 'failed', id: 'dsh' }).agentOpen, 'dsh')
  assert.equal(run([{ type: 'failed', id: 'dsh' }]).agentOpen, '', 'not on the Agent page')
  assert.equal(run([{ type: 'others' }, { type: 'others' }]).othersOpen, false)
  assert.equal(run([{ type: 'welcome', open: true }]).welcomeOpen, true)
})

test('next never changes the state it is given, and stateOf takes it alone', () => {
  const s = Object.freeze({ ...S.initial(), sub: Object.freeze({ page: 'addon', id: 'x', back: null }) })
  const all: SettingsEvent[] = [
    open('connect'),
    { type: 'shown', cardShows: true },
    { type: 'close' },
    { type: 'tab', to: 'look' },
    { type: 'page', tab: 'now', part: 'net' },
    { type: 'part', part: 'x' },
    { type: 'part-shown' },
    { type: 'sub', sub: { page: 'market' } },
    { type: 'back' },
    { type: 'sub-off' },
    { type: 'gone', id: 'x' },
    { type: 'agent', id: 'a', keep: false },
    { type: 'failed', id: 'a' },
    { type: 'others' },
    { type: 'welcome', open: true },
  ]
  for (const e of all) S.next(s, e)
  run(all, s)
  assert.deepEqual(S.stateOf({ ...S.initial(), tab: 'mail', snap: {} } as SettingsState), { ...S.initial(), tab: 'mail' })
})
