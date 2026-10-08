// Whether another app is full screen, so the pet can step aside. Windows only
// (user32 through koffi); elsewhere it never reports one.
//
// Full screen: the foreground window covers its whole monitor and has no title
// bar. A maximised window has one (and stops at the taskbar), so it is not.

const SKIP_CLASSES = new Set(['Progman', 'WorkerW', 'Shell_TrayWnd', 'Shell_SecondaryTrayWnd'])
const GWL_STYLE = -16
const WS_CAPTION = 0x00c00000
const MONITOR_DEFAULTTONEAREST = 2

function loadUser32() {
  if (process.platform !== 'win32') return null
  try {
    const koffi = require('koffi')
    const user32 = koffi.load('user32.dll')
    const RECT = koffi.struct('CP_RECT', { left: 'long', top: 'long', right: 'long', bottom: 'long' })
    const MONITORINFO = koffi.struct('CP_MONITORINFO', { cbSize: 'uint32', rcMonitor: RECT, rcWork: RECT, dwFlags: 'uint32' })
    return {
      koffi,
      foreground: user32.func('void* __stdcall GetForegroundWindow()'),
      isVisible: user32.func('bool __stdcall IsWindowVisible(void* hwnd)'),
      rect: user32.func('bool __stdcall GetWindowRect(void* hwnd, _Out_ CP_RECT* rect)'),
      style: user32.func('intptr_t __stdcall GetWindowLongPtrW(void* hwnd, int index)'),
      className: user32.func('int __stdcall GetClassNameW(void* hwnd, _Out_ uint16_t* name, int max)'),
      monitorFrom: user32.func('void* __stdcall MonitorFromWindow(void* hwnd, uint32 flags)'),
      monitorInfo: user32.func('bool __stdcall GetMonitorInfoW(void* monitor, _Inout_ CP_MONITORINFO* info)'),
    }
  } catch {
    return null
  }
}

// ownHandles(): BigInts of our own windows' handles, never counted.
function createFullscreenWatch({ ownHandles, onChange, intervalMs = 1500 }) {
  const api = loadUser32()
  let isFull = false
  let timer

  function check() {
    const hwnd = api.foreground()
    if (!hwnd) return false
    if (ownHandles().includes(BigInt(api.koffi.address(hwnd)))) return isFull
    if (!api.isVisible(hwnd)) return false

    const name = Buffer.alloc(512)
    const n = api.className(hwnd, name, 256)
    if (SKIP_CLASSES.has(name.toString('utf16le', 0, n * 2))) return false
    if (Number(api.style(hwnd, GWL_STYLE)) & WS_CAPTION) return false

    const rect = {}
    if (!api.rect(hwnd, rect)) return false
    const info = { cbSize: 40, rcMonitor: {}, rcWork: {}, dwFlags: 0 }
    if (!api.monitorInfo(api.monitorFrom(hwnd, MONITOR_DEFAULTTONEAREST), info)) return false
    const m = info.rcMonitor
    return rect.left <= m.left && rect.top <= m.top && rect.right >= m.right && rect.bottom >= m.bottom
  }

  function tick() {
    let now = false
    try {
      now = check()
    } catch {
      now = false
    }
    if (now !== isFull) {
      isFull = now
      onChange(isFull)
    }
  }

  return {
    isAvailable: !!api,
    start() {
      if (api && !timer) timer = setInterval(tick, intervalMs)
    },
    stop() {
      clearInterval(timer)
      timer = undefined
      if (isFull) {
        isFull = false
        onChange(false)
      }
    },
  }
}

module.exports = { createFullscreenWatch }
