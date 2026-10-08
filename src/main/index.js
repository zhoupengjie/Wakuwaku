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

// Settings live in one folder whatever the build calls itself; a separate one
// for tests (its own settings and single-instance lock). The folder from
// before the rename (claude-pets) moves over the first time.
const userData = process.env.WAKUWAKU_USER_DATA || path.join(app.getPath('appData'), 'wakuwaku')
if (!process.env.WAKUWAKU_USER_DATA) moveOldFolder(path.join(app.getPath('appData'), 'claude-pets'), userData)
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
