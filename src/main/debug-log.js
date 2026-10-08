// Lines in <userData>/debug.log, for what the screen cannot show (which
// window takes the pointer, a drag's start and end). On in debug builds
// (WAKUWAKU_DEBUG=1), or when <userData>/debug.on exists: a switch for
// finding out what happens on someone's own machine, with no debug routes.
const fs = require('fs')
const path = require('path')
const { app } = require('electron')

let isOn

function debugLog(...parts) {
  try {
    if (isOn === undefined) isOn = process.env.WAKUWAKU_DEBUG === '1' || fs.existsSync(path.join(app.getPath('userData'), 'debug.on'))
    if (!isOn) return
    fs.appendFileSync(path.join(app.getPath('userData'), 'debug.log'), `${new Date().toISOString().slice(11, 23)} ${parts.join(' ')}\n`)
  } catch {}
}

module.exports = { debugLog }
