import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import { decideHotkeyAction, holderOf } from '../src/pet/panel.ts'
import { STRINGS } from '../src/shared/i18n.ts'

// What the hotkey decision reads; a test gives only what it is about.
type HotkeyState = Parameters<typeof decideHotkeyAction>[0]

describe('global shortcuts behavioral tests', () => {
  describe('frontend hotkey decision state machine (panel.js decideHotkeyAction)', () => {
    it('which === "island" always requests island toggle', () => {
      const res = decideHotkeyAction({ which: 'island' } as HotkeyState)
      assert.deepEqual(res, { type: 'toggle' })
    })

    it('ignores unknown hotkey actions', () => {
      const res = decideHotkeyAction({ which: 'unknown' } as HotkeyState)
      assert.deepEqual(res, { type: 'ignore' })
    })

    it('if island is closed, only opens island and NEVER answers', () => {
      const ask = { id: 101, kind: 'permission', tool: 'write' }
      const res = decideHotkeyAction({
        which: 'allow',
        ask,
        shownId: 101,
        isArmed: true,
        islandOpen: false,
      })
      assert.deepEqual(res, { type: 'open' })
    })

    it('if prompt just appeared (<600ms un-armed), only opens island and does not answer', () => {
      const ask = { id: 102, kind: 'permission', tool: 'bash' }
      const res = decideHotkeyAction({
        which: 'allow',
        ask,
        shownId: 102,
        isArmed: false,
        islandOpen: true,
      })
      assert.deepEqual(res, { type: 'open' })
    })

    it('a code come by mail: approve copies it, deny lets it go; once copied the keys only open the island', () => {
      const ask = { id: 2 ** 40, kind: 'mailCode', code: '482910' }
      const on = { ask, shownId: 2 ** 40, isArmed: true, islandOpen: true }
      assert.deepEqual(decideHotkeyAction({ which: 'allow', ...on }), { type: 'answer', which: 'allow', id: 2 ** 40 })
      assert.deepEqual(decideHotkeyAction({ which: 'deny', ...on }), { type: 'answer', which: 'deny', id: 2 ** 40 })
      const link = { id: 2 ** 40 + 1, kind: 'mailLink', url: 'https://a.test/verify?token=1' }
      assert.deepEqual(decideHotkeyAction({ which: 'allow', ask: link, shownId: link.id, isArmed: true, islandOpen: true }).type, 'answer')
      assert.deepEqual(decideHotkeyAction({ which: 'allow', ...on, ask: { ...ask, copied: true } }), { type: 'open' })
    })

    it("another program's approval (POST /ask, #149) is answered by the keys as by its buttons", () => {
      const ask = { id: 2 ** 41, kind: 'app', app: 'Discord 桥', title: 'Claude 想运行 npm test' }
      for (const which of ['allow', 'deny']) {
        assert.deepEqual(decideHotkeyAction({ which, ask, shownId: ask.id, isArmed: true, islandOpen: true }), { type: 'answer', which, id: ask.id })
      }
      // Just come: the keys open the island, as for an agent's.
      assert.deepEqual(decideHotkeyAction({ which: 'allow', ask, shownId: ask.id, isArmed: false, islandOpen: true }), { type: 'open' })
    })

    it('if prompt is a question (ask.kind === "question"), only opens island and never answers blindly', () => {
      const ask = { id: 103, kind: 'question', questions: [{ question: 'Which branch?' }] }
      const res = decideHotkeyAction({
        which: 'allow',
        ask,
        shownId: 103,
        isArmed: true,
        islandOpen: true,
      })
      assert.deepEqual(res, { type: 'open' })
    })

    it('ZCode and DeepSeek Harness approvals are answered by the keys as by the buttons (#97)', () => {
      for (const from of ['zcode', 'dsh']) {
        for (const which of ['allow', 'deny']) {
          assert.deepEqual(
            decideHotkeyAction({
              which,
              ask: { id: 104, kind: 'permission', from },
              shownId: 104,
              isArmed: true,
              islandOpen: true,
            }),
            { type: 'answer', which, id: 104 }
          )
        }
      }
    })

    it('if prompt cannot be answered on the island (canAnswer === false), only opens island', () => {
      assert.deepEqual(
        decideHotkeyAction({
          which: 'allow',
          ask: { id: 104, kind: 'permission', canAnswer: false },
          shownId: 104,
          isArmed: true,
          islandOpen: true,
        }),
        { type: 'open' }
      )
    })

    it('when prompt is open, armed (>600ms), and answerable: executes allow answer for shownId', () => {
      const ask = { id: 105, kind: 'permission', tool: 'edit' }
      const res = decideHotkeyAction({
        which: 'allow',
        ask,
        shownId: 105,
        isArmed: true,
        islandOpen: true,
      })
      assert.deepEqual(res, { type: 'answer', which: 'allow', id: 105 })
    })

    it('when prompt is open, armed (>600ms), and answerable: executes deny answer for shownId', () => {
      const ask = { id: 106, kind: 'plan' }
      const res = decideHotkeyAction({
        which: 'deny',
        ask,
        shownId: 106,
        isArmed: true,
        islandOpen: true,
      })
      assert.deepEqual(res, { type: 'answer', which: 'deny', id: 106 })
    })
  })

  describe('shortcut i18n keys', () => {
    it('all shortcut i18n keys exist in both Chinese and English', () => {
      const keys = [
        's.shortcuts',
        's.shortcutsEnable',
        's.shortcutsEnableNote',
        's.shortcutApprove',
        's.shortcutApproveNote',
        's.shortcutDeny',
        's.shortcutDenyNote',
        's.shortcutIsland',
        's.shortcutIslandNote',
        's.shortcutNone',
        's.shortcutRecording',
        's.shortcutOccupied',
        's.shortcutNeedMod',
        's.shortcutNoWin',
        's.shortcutDuplicate',
      ]
      for (const k of keys) {
        assert.ok(k in STRINGS.zh, `missing in zh: ${k}`)
        assert.ok(k in STRINGS.en, `missing in en: ${k}`)
      }
    })
  })
})

describe('who holds a prompt (panel.js holderOf)', () => {
  it('her window while she is out and up, the island otherwise: one of the two', () => {
    const cases = [
      { out: false, herUp: false, hidden: true },
      { out: true, herUp: true, hidden: false },
      // Out, but hidden (full screen, do not disturb): the island has it.
      { out: true, herUp: false, hidden: true },
      // Main says she is up before her page has shown: not hers yet.
      { out: true, herUp: true, hidden: true },
    ]
    for (const c of cases) {
      const hers = holderOf({ role: 'pet', ...c })
      const island = holderOf({ role: 'island', ...c, hidden: false })
      assert.ok(!(hers && island), JSON.stringify(c))
    }
    assert.equal(holderOf({ role: 'pet', out: true, herUp: true, hidden: false }), true)
    assert.equal(holderOf({ role: 'island', out: true, herUp: true, hidden: false }), false)
    assert.equal(holderOf({ role: 'island', out: true, herUp: false, hidden: false }), true)
    assert.equal(holderOf({ role: 'island', out: false, herUp: false, hidden: false }), true)
    assert.equal(holderOf({ role: 'pet', out: false, herUp: false, hidden: false }), false)
  })
})
