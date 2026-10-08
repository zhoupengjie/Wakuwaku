// Downloaded pets, each a folder holding spritesheet.webp and pet.json:
// <userData>/pets/<id> (the settings window downloads there) and, for a copy
// run from source, <app>/pets/<id> (npm run fetch-pet). The first wins.
const fs = require('fs')
const path = require('path')
const { pathToFileURL } = require('url')
const { app } = require('electron')

function userDir() {
  return path.join(app.getPath('userData'), 'pets')
}

function dirs() {
  return [userDir(), path.join(app.getAppPath(), 'pets')]
}

function folderOf(id) {
  for (const dir of dirs()) {
    const folder = path.join(dir, id)
    if (fs.existsSync(path.join(folder, 'spritesheet.webp'))) return folder
  }
  return null
}

// Every pet found, with its display name and author.
function list() {
  const seen = new Map()
  for (const dir of dirs()) {
    let ids = []
    try {
      ids = fs.readdirSync(dir, { withFileTypes: true }).filter(d => d.isDirectory()).map(d => d.name)
    } catch {
      continue
    }
    for (const id of ids) {
      if (seen.has(id) || !fs.existsSync(path.join(dir, id, 'spritesheet.webp'))) continue
      let meta = {}
      try {
        meta = JSON.parse(fs.readFileSync(path.join(dir, id, 'pet.json'), 'utf8'))
      } catch {}
      seen.set(id, {
        id,
        name: meta.displayName || id,
        author: meta.author || '',
        source: meta.source || '',
        url: pathToFileURL(path.join(dir, id, 'spritesheet.webp')).href,
        version: meta.spriteVersionNumber === 2 ? 2 : 1,
      })
    }
  }
  return [...seen.values()]
}

// The spritesheet's file URL, or null when that pet is not downloaded.
function spriteUrl(id) {
  const folder = folderOf(id)
  return folder ? pathToFileURL(path.join(folder, 'spritesheet.webp')).href : null
}

// The pet as the page draws it: { url, version }, or null. Version 1 sheets
// (1536x1872) have 9 rows, without the two look-around rows.
function sprite(id) {
  return list().find(p => p.id === id) || null
}

// The spritesheet's path, or null.
function spritePath(id) {
  const folder = folderOf(id)
  return folder ? path.join(folder, 'spritesheet.webp') : null
}

module.exports = { list, sprite, spriteUrl, spritePath, userDir }
