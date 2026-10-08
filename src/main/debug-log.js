// Debug only (WAKUWAKU_DEBUG=1): lines in <userData>/debug.log, for what the
// screen cannot show (which window takes the pointer, a drag's start and end).
const fs = require('fs')
const path = require('path')
const { app } = require('electron')

const IS_DEBUG = process.env.WAKUWAKU_DEBUG === '1'

function debugLog(...parts) {
  if (!IS_DEBUG) return
  try {
    fs.appendFileSync(path.join(app.getPath('userData'), 'debug.log'), `${new Date().toISOString().slice(11, 23)} ${parts.join(' ')}\n`)
  } catch {}
}

module.exports = { debugLog }
