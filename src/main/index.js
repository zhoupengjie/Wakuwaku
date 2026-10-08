// claude-pets: a transparent, frameless, always-on-top desktop pet that shows
// what Claude Code is doing. Claude Code's hooks report to it over a local port.
//
// One program, three jobs, by its arguments:
//   --claude-pets-ensure-running  (the SessionStart hook) start the pet if it is not up, then leave
//   --claude-pets-cleanup         (the uninstaller) remove the hooks and start at login, then leave
//   anything else                 be the pet
const path = require('path')
const { app } = require('electron')

// Settings live in one folder whatever the build calls itself; a separate one
// for tests (its own settings and single-instance lock).
app.setPath('userData', process.env.CLAUDE_PETS_USER_DATA || path.join(app.getPath('appData'), 'claude-pets'))

const launch = require('./launch')

const PORT = Number(process.env.CLAUDE_PETS_PORT || 47213)

if (process.argv.includes(launch.ENSURE_FLAG)) {
  launch.ensureRunning(PORT)
} else if (process.argv.includes(launch.CLEANUP_FLAG)) {
  app.whenReady().then(() => launch.cleanup())
} else {
  require('./app').start({ port: PORT })
}
