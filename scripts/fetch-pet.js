#!/usr/bin/env node
// Downloads pets from codex-pets.net into pets/<id>/ (spritesheet.webp, pet.json).
//
//   npm run fetch-pet                                        deepseek-chan
//   npm run fetch-pet -- https://codex-pets.net/#/pets/<id>  a pet by its page URL
//   npm run fetch-pet -- <id> [<id or URL> ...]              by id, several at once
//
// The pets are their authors' work and stay out of this repository.
const fs = require('fs')
const path = require('path')

const SITE = 'https://codex-pets.net'
const DEFAULT_PET = 'deepseek-chan'
const ID = /^[a-z0-9][a-z0-9-]*$/i

// The layout src/renderer/pet.js plays: 8 x 11 cells of 192 x 208.
const ATLAS = '1536x2288'

function isSiteHost(hostname) {
  return hostname === 'codex-pets.net' || hostname.endsWith('.codex-pets.net')
}

// A pet's id from an id or any codex-pets.net link to it: the page
// (https://codex-pets.net/#/pets/<id>), with or without https://, or its API
// or asset URLs. Throws, in words for the person, when it is neither.
function parsePetRef(input) {
  const text = String(input ?? '').trim()
  if (ID.test(text)) return text

  let url
  try {
    url = new URL(/^[a-z][a-z0-9+.-]*:\/\//i.test(text) ? text : `https://${text}`)
  } catch {
    throw new Error(`看不懂这个地址或 id：${text}`)
  }
  if (!isSiteHost(url.hostname)) {
    throw new Error(`只支持 codex-pets.net 上的宠物：${text}`)
  }

  const routes = [
    [url.hash, /^#\/pets\/([^/?#]+)/], // the page: #/pets/<id>
    [url.pathname, /^\/api\/pets\/([^/?#]+)/], // the API: /api/pets/<id>[/download]
    [url.pathname, /^\/assets\/pets\/v\/\d+\/([^/?#]+)\//], // an asset: /assets/pets/v/<n>/<id>/...
    [url.pathname, /^\/pets\/([^/?#]+)/], // a page without the hash
  ]
  for (const [part, route] of routes) {
    const id = part.match(route)?.[1]
    if (id && ID.test(decodeURIComponent(id))) return decodeURIComponent(id)
  }
  throw new Error(`地址里没找到宠物 id：${text}（宠物页面的地址形如 ${SITE}/#/pets/<id>）`)
}

async function get(url) {
  const res = await fetch(url, { headers: { 'user-agent': 'claude-pets' } })
  if (!res.ok) throw new Error(`${res.status} ${res.statusText}：${url}`)
  return res
}

async function fetchPet(id) {
  const res = await fetch(`${SITE}/api/pets/${id}`, { headers: { 'user-agent': 'claude-pets' } })
  if (res.status === 404) throw new Error(`codex-pets.net 上没有这只宠物：${id}`)
  if (!res.ok) throw new Error(`${res.status} ${res.statusText}：${SITE}/api/pets/${id}`)
  const { pet } = await res.json()

  // Only ever download from the site itself.
  const sheetUrl = new URL(pet.spritesheetUrl, SITE)
  if (sheetUrl.protocol !== 'https:' || !isSiteHost(sheetUrl.hostname)) {
    throw new Error(`图集不在 codex-pets.net 上，不下载：${sheetUrl.href}`)
  }

  const atlas = pet.validationReport?.atlasSize
  if (atlas && atlas !== ATLAS) {
    console.warn(`注意：${id} 的图集是 ${atlas}，不是 v2 的 ${ATLAS}，动画可能对不上。`)
  }

  const sheet = Buffer.from(await (await get(sheetUrl.href)).arrayBuffer())
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
}

async function main() {
  const refs = process.argv.slice(2)
  let failed = 0

  for (const ref of refs.length ? refs : [DEFAULT_PET]) {
    try {
      await fetchPet(parsePetRef(ref))
    } catch (err) {
      failed += 1
      console.error(`下载失败：${err.message}`)
    }
  }

  if (failed < (refs.length || 1)) console.log('在桌宠上右键 → 宠物，即可切换。')
  if (failed) process.exit(1)
}

if (require.main === module) main()

module.exports = { parsePetRef }
