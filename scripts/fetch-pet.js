#!/usr/bin/env node
// Downloads pets from codex-pets.net into pets/<id>/ (spritesheet.webp, pet.json).
//
//   npm run fetch-pet                                        deepseek-chan
//   npm run fetch-pet -- https://codex-pets.net/#/pets/<id>  a pet by its page URL
//   npm run fetch-pet -- <id> [<id or URL> ...]              by id, several at once
//
// The pets are their authors' work and stay out of this repository.
const path = require('path')

const { detectLang } = require('../src/shared/i18n')
const { parsePetRef, downloadPet, describe } = require('../src/shared/pet-fetch')

const DEFAULT_PET = 'deepseek-chan'
const PETS_DIR = path.join(__dirname, '..', 'pets')
const lang = detectLang(process.env.LANG || process.env.LC_ALL || Intl.DateTimeFormat().resolvedOptions().locale)

const SAY = {
  zh: {
    done: '已下载 {name}（作者 {author}）→ {dir}',
    failed: '下载失败：{message}',
    switch: '在桌宠上右键 → 宠物，即可切换。',
  },
  en: {
    done: 'Downloaded {name} by {author} → {dir}',
    failed: 'Download failed: {message}',
    switch: 'Right-click the pet → Pet to switch to it.',
  },
}

function say(key, vars = {}) {
  return SAY[lang][key].replace(/\{(\w+)\}/g, (all, name) => (vars[name] !== undefined ? String(vars[name]) : all))
}

async function main() {
  const refs = process.argv.slice(2)
  const all = refs.length ? refs : [DEFAULT_PET]
  let failed = 0

  for (const ref of all) {
    try {
      const pet = await downloadPet(ref, PETS_DIR)
      if (pet.warning) console.warn(describe(lang, pet.warning))
      console.log(say('done', pet))
    } catch (err) {
      failed += 1
      console.error(say('failed', { message: describe(lang, err) }))
    }
  }

  if (failed < all.length) console.log(say('switch'))
  if (failed) process.exit(1)
}

if (require.main === module) main()

module.exports = { parsePetRef }
