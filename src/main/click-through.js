// Clicks through a transparent window, except on what it draws.
//
// The window lets clicks through its transparent parts. To still hear the
// pointer arrive over her, Electron forwards mouse moves, which on Windows is
// a system-wide mouse hook: every move anywhere passes through this process.
// So forwarding is on only while the cursor is near the window; a poll of
// the cursor (cheap) decides. The page says when the pointer is over
// something of hers (setOver), and the window then takes the click.
const { screen } = require('electron')

const { debugLog } = require('./debug-log')

const NEAR_PX = 24

// getWin: the window (or null); isShown: whether it is up; isHeld: a drag in
// progress (the window keeps the pointer); onPoll(cursor, near): anything
// else to do with the cursor, returning a delay in ms to poll again sooner.
function createClickThrough({ name = '', getWin, isShown, isHeld = () => false, onPoll = () => undefined }) {
  let isOver = false
  let mode = ''
  let timer

  function setMode(next) {
    const win = getWin()
    if (!win || next === mode) return
    debugLog(name, 'mode', mode || '-', '->', next)
    mode = next
    if (next === 'catch') win.setIgnoreMouseEvents(false)
    else if (next === 'forward') win.setIgnoreMouseEvents(true, { forward: true })
    else win.setIgnoreMouseEvents(true)
  }

  function isWithin(cursor, margin) {
    const b = getWin().getBounds()
    return cursor.x >= b.x - margin && cursor.x <= b.x + b.width + margin && cursor.y >= b.y - margin && cursor.y <= b.y + b.height + margin
  }

  function schedule(ms = 0) {
    clearTimeout(timer)
    timer = setTimeout(poll, ms)
  }

  function poll() {
    if (!getWin()) return
    if (!isShown()) {
      setMode('pass')
      return schedule(500)
    }
    const cursor = screen.getCursorScreenPoint()
    const near = isWithin(cursor, NEAR_PX)
    // Over something of ours, yet outside the window: the page missed the
    // pointer leaving (it was held elsewhere). Stop taking clicks there.
    if (isOver && !isHeld() && !isWithin(cursor, 0)) {
      debugLog(name, 'over cleared: pointer outside')
      isOver = false
    }
    setMode(isHeld() || isOver ? 'catch' : near ? 'forward' : 'pass')
    const sooner = onPoll(cursor, near)
    schedule(sooner ?? (near ? 80 : 200))
  }

  return {
    schedule,
    setMode,
    setOver(over) {
      isOver = over === true
      debugLog(name, 'over', isOver)
      if (!isHeld()) setMode(isOver ? 'catch' : 'forward')
    },
    // Forget the pointer was over anything (the window moved from under it).
    reset() {
      isOver = false
      mode = ''
      setMode('forward')
    },
  }
}

module.exports = { createClickThrough }
