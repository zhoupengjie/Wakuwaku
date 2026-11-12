// The input method on the taskbar: the layout's mark and the mode of its
// input method (taskbar:keys), Windows' own icons for it where it has them,
// and the list of input methods a press on it opens.
// Made by island.ts in the section's place as the page starts
// (imePart(core), imported by island.ts): what it takes of the island's core
// comes in core, its functions and constants as they are; what the island
// remembers is core.st (island-vars.ts), read and written as st.<name>;
// what it hands back is the island's again.
import { pet } from './bridge.ts'
import type { ImeCore } from './island.ts'

export function imePart(core: ImeCore) {
  const {
    st, besideOrOver, capsMark, closePop, closeWidgetPop, el, imeMark, imePop, isSide, showBattery, showNet,
    showPrivacy, showVolume, t, watchPresses,
  } = core

  // The script's mark by the layout's language: its own and plain letters.
  const SCRIPTS = { 0x04: ['中', '英'], 0x11: ['あ', 'A'], 0x12: ['한', 'A'] }
  const LANGS = { 0x09: 'EN', 0x07: 'DE', 0x0c: 'FR', 0x0a: 'ES', 0x10: 'IT', 0x16: 'PT', 0x19: 'RU', 0x13: 'NL', 0x1f: 'TR' }
  function langMark(langId, native) {
    const primary = (langId || 0) & 0x3ff
    const script = SCRIPTS[primary]
    return { script, text: script ? script[native === false ? 1 : 0] : LANGS[primary] || primary.toString(16).toUpperCase() }
  }
  // The mode's word in the tip where Windows gives none of its own.
  const MODE_WORDS = { 中: '中文', 英: '英文' }
  // The input methods' icons (taskbar/ taskbar:imeIcons), by key: each
  // sent once, named by the keys after it.
  const imeIcons = {}
  let keysNow = null
  let imeDrawn = ''
  // As Windows' own taskbar shows the input method in front (which one it
  // is, inputs.rs): Microsoft's own, and a plain layout, by the script's
  // mark (中 or 英), as Windows draws its glyph; one of another maker
  // (Sogou's, WeChat's) by the picture it gives of its mode, as Windows
  // shows it, or else its own icon beside 中 or 英, so two of one language
  // can be told apart. Its name, its mode and its language in the tip.
  function showIme(keys) {
    const { script, text } = langMark(keys?.lang, keys?.native)
    const ime = keys?.ime || null
    const theirs = !!ime && !ime.own
    const modePic = (theirs && ime.mode?.icon && imeIcons[ime.mode.icon]) || ''
    const ownPic = (theirs && !modePic && ime.icon && imeIcons[ime.icon]) || ''
    const mode = ime?.mode?.tip || (script ? MODE_WORDS[text] || text : '')
    const tip = ime?.name ? [[ime.name, mode].filter(Boolean).join(' · '), ime.lang].filter(Boolean).join('\n') : ''
    const drawn = JSON.stringify([text, modePic, ownPic, tip])
    if (drawn !== imeDrawn) {
      imeDrawn = drawn
      const pic = modePic || ownPic
      const img = pic ? el('img', 'pic') : null
      if (img) img.src = pic
      imeMark.replaceChildren(...(img ? [img] : []), ...(modePic ? [] : [el('span', 'script', text)]))
      imeMark.title = tip
    }
    imeMark.classList.toggle('unknown', !modePic && !!script && (keys?.native == null || !!ime?.mode?.off))
  }
  pet.onImeIcons(icons => {
    Object.assign(imeIcons, icons || {})
    if (keysNow) showIme(keysNow)
  })
  pet.onKeys(keys => {
    keysNow = keys
    showIme(keys)
    capsMark.classList.toggle('on', !!keys?.caps)
    capsMark.title = keys?.caps ? 'Caps Lock' : ''
    showNet(keys?.net, keys?.bars)
    showPrivacy(keys?.use)
    showVolume(keys?.volume)
    showBattery(keys?.battery)
  })

  // A right press on the mark: the input methods to pick from, as Windows'
  // own list has them (Win+Space), in its order, each with its icon (a
  // layout by its language's mark), the one in front marked; a press on one
  // switches the window in front to it (inputs.rs pick), and the last row
  // opens Windows' languages. Windows' own list where none can be had.
  async function openImeList() {
    if (st.popFor) closePop()
    if (st.wgFor) closeWidgetPop()
    const list = await pet.taskbar.inputs()
    if (!Array.isArray(list) || !list.length) return pet.taskbar.open('inputs')
    const rows = list.map(m => {
      const row = el('div', m.current ? 'wrow irow front' : 'wrow irow')
      row.dataset.input = m.id
      if (m.icon) {
        const img = el('img')
        img.src = m.icon
        row.append(img)
      } else row.append(el('span', 'itile', langMark(m.lang, true).text))
      const lines = el('span', 'ilines')
      // As Windows' list has them: the language, the input method under it.
      lines.append(el('span', 'wt', m.langName), el('span', 'iname', m.name))
      row.append(lines)
      return row
    })
    const more = el('div', 'witem', t(st.lang, 'taskbar.imeMore'))
    more.dataset.input = ''
    imePop.replaceChildren(...rows, more)
    imePop.hidden = false
    st.imeOpen = true
    const at = besideOrOver(imeMark, imePop, isSide() ? 8 : 4)
    imePop.style.left = at.left === undefined ? '' : `${at.left}px`
    imePop.style.top = at.top === undefined ? '' : `${at.top}px`
    watchPresses()
  }

  function closeImeList() {
    st.imeOpen = false
    imePop.hidden = true
    watchPresses()
  }

  imePop.addEventListener('click', e => {
    const row = e.target.closest('[data-input]')
    if (!row) return
    closeImeList()
    if (row.dataset.input) pet.taskbar.input(row.dataset.input)
    else pet.taskbar.open('languages')
  })
  imePop.addEventListener('contextmenu', e => {
    e.preventDefault()
    e.stopPropagation()
  })

  return { closeImeList, openImeList }
}
