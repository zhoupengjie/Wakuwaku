#!/usr/bin/env node
// Downloads a pet from codex-pets.net into pets/<id>/ (spritesheet.webp, pet.json).
//
//   npm run fetch-pet                  deepseek-chan
//   npm run fetch-pet -- <id>          any pet, by the id in its page URL
//                                      (https://codex-pets.net/#/pets/<id>)
//
// The pets are their authors' work and stay out of this repository.
const fs = require('fs')
const path = require('path')

const SITE = 'https://codex-pets.net'
const id = process.argv[2] || 'deepseek-chan'

// The layout src/renderer/pet.js plays: 8 x 11 cells of 192 x 208.
const ATLAS = '1536x2288'

async function get(url) {
  const res = await fetch(url, { headers: { 'user-agent': 'claude-pets' } })
  if (!res.ok) throw new Error(`${res.status} ${res.statusText}：${url}`)
  return res
}

async function main() {
  if (!/^[a-z0-9][a-z0-9-]*$/i.test(id)) {
    throw new Error(`不是有效的宠物 id：${id}`)
  }

  const { pet } = await (await get(`${SITE}/api/pets/${id}`)).json()
  const atlas = pet.validationReport?.atlasSize
  if (atlas && atlas !== ATLAS) {
    console.warn(`注意：${id} 的图集是 ${atlas}，不是 v2 的 ${ATLAS}，动画可能对不上。`)
  }

  const sheet = Buffer.from(await (await get(pet.spritesheetUrl)).arrayBuffer())
  const dir = path.join(__dirname, '..', 'pets', id)
  fs.mkdirSync(dir, { recursive: true })
  fs.writeFileSync(path.join(dir, 'spritesheet.webp'), sheet)
  fs.writeFileSync(
    path.join(dir, 'pet.json'),
    `${JSON.stringify(
      {
        id: pet.id,
        displayName: pet.displayName,
        description: pet.description,
        author: pet.ownerName,
        source: `${SITE}/#/pets/${pet.id}`,
        spritesheetPath: 'spritesheet.webp',
        spriteVersionNumber: pet.spriteVersionNumber,
        kind: pet.kind,
      },
      null,
      2,
    )}\n`,
  )

  console.log(`已下载 ${pet.displayName}（作者 ${pet.ownerName}）→ ${dir}`)
  console.log('在桌宠上右键 → 宠物，即可切换。')
}

main().catch(err => {
  console.error(`下载失败：${err.message}`)
  process.exit(1)
})
