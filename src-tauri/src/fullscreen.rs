// Whether another app is full screen, so she can step aside. Windows only;
// elsewhere it never reports one.
//
// Full screen: the foreground window covers its whole monitor and has no title
// bar. A maximised window has one (and stops at the taskbar), so it is not.

use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

// Asked to look now: a window came to the front, or the one in front was
// sized (the taskbar's hooks, taskbar.rs). When it was first asked, until
// looked at: however many asks come meanwhile, one look.
static ASKED: Mutex<Option<Instant>> = Mutex::new(None);
static WAKE: Condvar = Condvar::new();

pub fn ask() {
    let mut asked = ASKED.lock().unwrap();
    if asked.is_none() {
        *asked = Some(Instant::now());
        WAKE.notify_one();
    }
}

// Until asked, or the time is up: when it was asked (None: the time).
pub fn wait(most: Duration) -> Option<Instant> {
    let asked = ASKED.lock().unwrap();
    let (mut asked, _) = WAKE.wait_timeout_while(asked, most, |a| a.is_none()).unwrap();
    asked.take()
}

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
        fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
        fn CloseHandle(handle: isize) -> i32;
        fn QueryFullProcessImageNameW(process: isize, flags: u32, name: *mut u16, size: *mut u32) -> i32;
    }

    const GWL_EXSTYLE: i32 = -20;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

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

    // The window in front, for the log: its class, its program's file and
    // its extended style. To tell a screenshot tool's layer over the screen
    // a moment (the taskbar hid and came back, a flash, with every shot,
    // 2026-10-10) from a game full screen, before a rule tells them apart.
    pub fn front() -> String {
        // SAFETY: plain calls; the buffers are ours and sized; the handle closed.
        unsafe {
            let hwnd = GetForegroundWindow();
            let mut name = [0u16; 256];
            let n = GetClassNameW(hwnd, name.as_mut_ptr(), 256).max(0) as usize;
            let class = String::from_utf16_lossy(&name[..n]);
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            let mut file = String::new();
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process != 0 {
                let mut path = [0u16; 512];
                let mut size = 512u32;
                if QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut size) != 0 {
                    let path = String::from_utf16_lossy(&path[..size as usize]);
                    file = path.rsplit('\\').next().unwrap_or(&path).to_string();
                }
                CloseHandle(process);
            }
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            format!("{class} ({file}) exstyle {ex:#x}")
        }
    }

    pub const AVAILABLE: bool = true;
}

#[cfg(not(windows))]
mod imp {
    pub fn check(_own: &[isize]) -> Option<bool> {
        Some(false)
    }

    pub fn front() -> String {
        String::new()
    }

    pub const AVAILABLE: bool = false;
}

pub use imp::{check, front, AVAILABLE};
