// Pets downloaded by `npm run fetch-pet` into pets/<id>/.
const fs = require('fs')
const path = require('path')
const { pathToFileURL } = require('url')
const { app } = require('electron')

const PETS_DIR = path.join(app.getAppPath(), 'pets')

// Every pet folder that holds a spritesheet, with its display name.
function list() {
  let ids = []
  try {
    ids = fs.readdirSync(PETS_DIR, { withFileTypes: true }).filter(d => d.isDirectory()).map(d => d.name)
  } catch {
    return []
  }

  return ids
    .filter(id => fs.existsSync(path.join(PETS_DIR, id, 'spritesheet.webp')))
    .map(id => {
      let name = id
      try {
        name = JSON.parse(fs.readFileSync(path.join(PETS_DIR, id, 'pet.json'), 'utf8')).displayName || id
      } catch {}
      return { id, name }
    })
}

// The spritesheet's file URL, or null when that pet is not downloaded.
function spriteUrl(id) {
  const file = path.join(PETS_DIR, id, 'spritesheet.webp')
  return fs.existsSync(file) ? pathToFileURL(file).href : null
}

module.exports = { list, spriteUrl }
