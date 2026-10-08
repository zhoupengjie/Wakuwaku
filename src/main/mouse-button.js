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

module.exports = { isPrimaryDown }
