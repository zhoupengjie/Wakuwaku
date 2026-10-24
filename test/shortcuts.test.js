const assert = require('node:assert/strict')
const { describe, it } = require('node:test')

const { decideHotkeyAction } = require('../src/pet/panel')
const { STRINGS } = require('../src/shared/i18n')

describe('global shortcuts behavioral tests', () => {
  describe('frontend hotkey decision state machine (panel.js decideHotkeyAction)', () => {
    it('which === "island" always requests island toggle', () => {
      const res = decideHotkeyAction({ which: 'island' })
      assert.deepEqual(res, { type: 'toggle' })
    })

    it('ignores unknown hotkey actions', () => {
      const res = decideHotkeyAction({ which: 'unknown' })
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

    it('if prompt must be answered in-app (IN_APP or canAnswer === false), only opens island', () => {
      assert.deepEqual(
        decideHotkeyAction({
          which: 'allow',
          ask: { id: 104, kind: 'permission', from: 'zcode' },
          shownId: 104,
          isArmed: true,
          islandOpen: true,
        }),
        { type: 'open' }
      )
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
