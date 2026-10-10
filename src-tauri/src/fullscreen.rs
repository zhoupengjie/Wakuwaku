// Whether another app is full screen, so she can step aside. Windows only;
// elsewhere it never reports one.
//
// Full screen: the foreground window covers its whole monitor and has no title
// bar. A maximised window has one (and stops at the taskbar), so it is not.

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;

    type Hwnd = *mut c_void;

    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct MonitorInfo {
        size: u32,
        monitor: Rect,
        work: Rect,
        flags: u32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetForegroundWindow() -> Hwnd;
        fn IsWindowVisible(hwnd: Hwnd) -> i32;
        fn GetClassNameW(hwnd: Hwnd, name: *mut u16, max: i32) -> i32;
        fn GetWindowLongPtrW(hwnd: Hwnd, index: i32) -> isize;
        fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
        fn MonitorFromWindow(hwnd: Hwnd, flags: u32) -> *mut c_void;
        fn GetMonitorInfoW(monitor: *mut c_void, info: *mut MonitorInfo) -> i32;
    }

    // The desktop, the taskbars, and task view and Alt+Tab (they cover the screen).
    const SKIP_CLASSES: [&str; 5] = ["Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd", "XamlExplorerHostIslandWindow"];
    const GWL_STYLE: i32 = -16;
    const WS_CAPTION: isize = 0x00c0_0000;
    const MONITOR_DEFAULTTONEAREST: u32 = 2;

    // None: the foreground window is one of ours (keep what was decided).
    pub fn check(own: &[isize]) -> Option<bool> {
        // SAFETY: plain user32 calls; the buffers and structs are ours and sized.
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_null() {
                return Some(false);
            }
            if own.contains(&(hwnd as isize)) {
                return None;
            }
            if IsWindowVisible(hwnd) == 0 {
                return Some(false);
            }
            let mut name = [0u16; 256];
            let n = GetClassNameW(hwnd, name.as_mut_ptr(), 256).max(0) as usize;
            if SKIP_CLASSES.contains(&String::from_utf16_lossy(&name[..n]).as_str()) {
                return Some(false);
            }
            if GetWindowLongPtrW(hwnd, GWL_STYLE) & WS_CAPTION != 0 {
                return Some(false);
            }
            let mut rect = Rect::default();
            if GetWindowRect(hwnd, &mut rect) == 0 {
                return Some(false);
            }
            let mut info = MonitorInfo { size: std::mem::size_of::<MonitorInfo>() as u32, ..Default::default() };
            if GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &mut info) == 0 {
                return Some(false);
            }
            let m = &info.monitor;
            Some(rect.left <= m.left && rect.top <= m.top && rect.right >= m.right && rect.bottom >= m.bottom)
        }
    }

    pub const AVAILABLE: bool = true;
}

#[cfg(not(windows))]
mod imp {
    pub fn check(_own: &[isize]) -> Option<bool> {
        Some(false)
    }

    pub const AVAILABLE: bool = false;
}

pub use imp::{check, AVAILABLE};
