// Whether the primary mouse button is held right now, wherever the pointer
// is. Windows only (user32 through koffi); elsewhere it is unknown (null),
// and a drag ends only when a page hears the button go up.
//
// For carrying her after she is pulled out of the island: the button went
// down on the island's window, and its release can land anywhere.

// GetAsyncKeyState reads the physical buttons; with the buttons swapped
// (left-handed), the primary one is the physical right.
const VK_LBUTTON = 0x01
const VK_RBUTTON = 0x02
const SM_SWAPBUTTON = 23

let getState
let isSwapped = () => false
let getCapture = () => null
let releaseCapture = () => false

function load() {
  if (getState !== undefined) return getState
  getState = null
  if (process.platform !== 'win32') return getState
  try {
    const koffi = require('koffi')
    const user32 = koffi.load('user32.dll')
    getState = user32.func('short __stdcall GetAsyncKeyState(int key)')
    const metrics = user32.func('int __stdcall GetSystemMetrics(int index)')
    isSwapped = () => metrics(SM_SWAPBUTTON) !== 0
    getCapture = user32.func('void* __stdcall GetCapture()')
    releaseCapture = user32.func('bool __stdcall ReleaseCapture()')
  } catch {
    getState = null
  }
  return getState
}

// true / false, or null when it cannot be told.
function isPrimaryDown() {
  const fn = load()
  if (!fn) return null
  try {
    return (fn(isSwapped() ? VK_RBUTTON : VK_LBUTTON) & 0x8000) !== 0
  } catch {
    return null
  }
}

// Whichever of this thread's windows holds the mouse, let go of it. The
// button went down on the island's window and came up elsewhere; Windows can
// leave that window holding the mouse, and every click, on her too, goes to
// it until something (a menu) takes it away. Returns whether one held it.
function releaseMouse() {
  if (!load()) return false
  try {
    const held = getCapture()
    if (!held) return false
    releaseCapture()
    return true
  } catch {
    return false
  }
}

module.exports = { isPrimaryDown, releaseMouse }
