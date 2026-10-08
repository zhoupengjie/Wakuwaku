// Getting a pet from codex-pets.net: its id from whatever the person pasted,
// and the download into <dir>/<id>/ (spritesheet.webp, pet.json). Shared by
// scripts/fetch-pet.js and the settings window.
//
// Errors carry a `code` (and `vars`) for the caller to word in its language.
const fs = require('fs')
const path = require('path')

const SITE = 'https://codex-pets.net'
const ID = /^[a-z0-9][a-z0-9-]*$/i

// The layout src/renderer/pet.js plays: 8 x 11 cells of 192 x 208.
const ATLAS = '1536x2288'

class PetError extends Error {
  constructor(code, vars = {}) {
    super(`${code} ${JSON.stringify(vars)}`)
    this.code = code
    this.vars = vars
  }
}

function isSiteHost(hostname) {
  return hostname === 'codex-pets.net' || hostname.endsWith('.codex-pets.net')
}

// A pet's id from an id or any codex-pets.net link to it: the page
// (https://codex-pets.net/#/pets/<id>), with or without https://, or its API
// or asset URLs.
function parsePetRef(input) {
  const text = String(input ?? '').trim()
  if (ID.test(text)) return text

  let url
  try {
    url = new URL(/^[a-z][a-z0-9+.-]*:\/\//i.test(text) ? text : `https://${text}`)
  } catch {
    throw new PetError('unreadable', { ref: text })
  }
  if (!isSiteHost(url.hostname)) throw new PetError('otherSite', { ref: text })

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
  throw new PetError('noId', { ref: text })
}

// Downloads the pet into <dir>/<id>/. Resolves { id, name, author, dir, warning? }.
async function downloadPet(ref, dir, { fetch = globalThis.fetch } = {}) {
  const id = parsePetRef(ref)
  const headers = { 'user-agent': 'claude-pets' }

  const res = await fetch(`${SITE}/api/pets/${id}`, { headers })
  if (res.status === 404) throw new PetError('notFound', { id })
  if (!res.ok) throw new PetError('http', { status: res.status, url: `${SITE}/api/pets/${id}` })
  const { pet } = await res.json()

  // Only ever download from the site itself.
  const sheetUrl = new URL(pet.spritesheetUrl, SITE)
  if (sheetUrl.protocol !== 'https:' || !isSiteHost(sheetUrl.hostname)) throw new PetError('sheetElsewhere', { url: sheetUrl.href })

  const sheetRes = await fetch(sheetUrl.href, { headers })
  if (!sheetRes.ok) throw new PetError('http', { status: sheetRes.status, url: sheetUrl.href })
  const sheet = Buffer.from(await sheetRes.arrayBuffer())

  const out = path.join(dir, id)
  fs.mkdirSync(out, { recursive: true })
  fs.writeFileSync(path.join(out, 'spritesheet.webp'), sheet)
  fs.writeFileSync(
    path.join(out, 'pet.json'),
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

  const atlas = pet.validationReport?.atlasSize
  return {
    id,
    name: pet.displayName,
    author: pet.ownerName,
    dir: out,
    warning: atlas && atlas !== ATLAS ? { code: 'atlas', vars: { id, atlas, want: ATLAS } } : undefined,
  }
}

// An error from here in the person's language.
const WORDS = {
  zh: {
    unreadable: '看不懂这个地址或 id：{ref}',
    otherSite: '只支持 codex-pets.net 上的宠物：{ref}',
    noId: `地址里没找到宠物 id：{ref}（宠物页面的地址形如 ${SITE}/#/pets/<id>）`,
    notFound: 'codex-pets.net 上没有这只宠物：{id}',
    http: '{status}：{url}',
    sheetElsewhere: '图集不在 codex-pets.net 上，不下载：{url}',
    atlas: '注意：{id} 的图集是 {atlas}，不是 v2 的 {want}，动画可能对不上。',
  },
  en: {
    unreadable: "Can't read this URL or id: {ref}",
    otherSite: 'Only pets on codex-pets.net are supported: {ref}',
    noId: `No pet id in this URL: {ref} (a pet page looks like ${SITE}/#/pets/<id>)`,
    notFound: 'No such pet on codex-pets.net: {id}',
    http: '{status}: {url}',
    sheetElsewhere: 'The sprite sheet is not on codex-pets.net, not downloading: {url}',
    atlas: 'Note: {id} has a {atlas} sheet, not the v2 {want}; the animation may be off.',
  },
}

function describe(lang, { code, vars = {}, message }) {
  const text = (WORDS[lang] || WORDS.en)[code]
  if (!text) return message || String(code)
  return text.replace(/\{(\w+)\}/g, (all, name) => (vars[name] !== undefined ? String(vars[name]) : all))
}

module.exports = { SITE, parsePetRef, downloadPet, describe, PetError }
