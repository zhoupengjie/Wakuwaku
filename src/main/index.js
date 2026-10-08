// wakuwaku: a transparent, frameless, always-on-top desktop pet that shows
// what Claude Code is doing. Claude Code's hooks report to it over a local port.
//
// One program, three jobs, by its arguments:
//   --wakuwaku-ensure-running  (the SessionStart hook) start the pet if it is not up, then leave
//   --wakuwaku-cleanup         (the uninstaller) remove the hooks and start at login, then leave
//   anything else                 be the pet
const fs = require('fs')
const path = require('path')
const { app } = require('electron')

// Where her settings, downloaded pets and caches live. Run from source or as
// the portable build, nothing goes outside her own folder; installed, they go
// where installed apps keep theirs. Tests bring their own (its own settings
// and single-instance lock).
const APP_DATA = path.join(app.getPath('appData'), 'wakuwaku')
const userData = dataFolder()
if (userData === APP_DATA) moveOldFolder(path.join(app.getPath('appData'), 'claude-pets'), userData)
else if (!process.env.WAKUWAKU_USER_DATA) adoptSettings(APP_DATA, userData)
app.setPath('userData', userData)

const launch = require('./launch')

const PORT = Number(process.env.WAKUWAKU_PORT || 47213)

// The old flag too: settings.json hooks from before the rename still use it.
if (process.argv.includes(launch.ENSURE_FLAG) || process.argv.includes(launch.LEGACY_ENSURE_FLAG)) {
  launch.ensureRunning(PORT)
} else if (process.argv.includes(launch.CLEANUP_FLAG)) {
  app.whenReady().then(() => launch.cleanup())
} else {
  require('./app').start({ port: PORT })
}

function dataFolder() {
  if (process.env.WAKUWAKU_USER_DATA) return process.env.WAKUWAKU_USER_DATA
  // From source: data/ in the project (not in git).
  if (!app.isPackaged) return path.join(app.getAppPath(), 'data')
  // The portable build runs from a temporary copy; this is where its .exe is.
  if (process.env.PORTABLE_EXECUTABLE_DIR) return path.join(process.env.PORTABLE_EXECUTABLE_DIR, 'wakuwaku-data')
  return APP_DATA
}

// A folder of her own for the first time: start from the settings and pets
// she had in AppData, if any (the caches are not worth copying).
function adoptSettings(from, to) {
  if (fs.existsSync(to)) return
  try {
    fs.mkdirSync(to, { recursive: true })
    for (const name of ['config.json', 'pets']) {
      if (fs.existsSync(path.join(from, name))) fs.cpSync(path.join(from, name), path.join(to, name), { recursive: true })
    }
  } catch {}
}

// Move the whole folder; if something holds it (the old app still running),
// carry over just the settings and the downloaded pets.
function moveOldFolder(from, to) {
  if (fs.existsSync(to) || !fs.existsSync(from)) return
  try {
    fs.renameSync(from, to)
  } catch {
    try {
      fs.mkdirSync(to, { recursive: true })
      for (const name of ['config.json', 'pets']) {
        if (fs.existsSync(path.join(from, name))) fs.cpSync(path.join(from, name), path.join(to, name), { recursive: true })
      }
    } catch {}
  }
}
