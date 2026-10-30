// Whether another app is full screen, so she can step aside. Windows only;
// elsewhere it never reports one.
//
// Full screen: the foreground window covers its whole monitor and has no title
// bar, or has one by its style and covers it unmaximised, or lies exactly on
// it (fills). A maximised window has one and goes past the monitor's edges
// or stops at the taskbar, so it is not. And only on a display she is on (on: their rects; none given, any): a
// video full screen on the second display hid her taskbar on the main one
// (the user, 2026-10-13); Windows' own steps aside only on its own display.

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
        fn IsZoomed(hwnd: Hwnd) -> i32;
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
    // Windows' own, which cover the screen a moment as they open: search
    // (SearchHost's CoreWindow, some 60 ms: the taskbar flashed, #5), Start,
    // the quick settings and notifications.
    const SKIP_PROGRAMS: [&str; 3] = ["searchhost.exe", "startmenuexperiencehost.exe", "shellexperiencehost.exe"];

    // The file name of the program a window is of, lower case.
    fn program_of(hwnd: Hwnd) -> String {
        let mut pid = 0u32;
        // SAFETY: our own out-parameter, buffer and size; the handle closed.
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut pid);
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process == 0 {
                return String::new();
            }
            let mut path = [0u16; 512];
            let mut size = 512u32;
            let ok = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut size) != 0;
            CloseHandle(process);
            if !ok {
                return String::new();
            }
            let path = String::from_utf16_lossy(&path[..size as usize]);
            path.rsplit('\\').next().unwrap_or(&path).to_lowercase()
        }
    }
    const GWL_STYLE: i32 = -16;
    // Two bits, WS_BORDER and WS_DLGFRAME: a title bar is both (titled).
    const WS_CAPTION: u32 = 0x00c0_0000;
    const MONITOR_DEFAULTTONEAREST: u32 = 2;

    // A title bar, by the window's style: both of WS_CAPTION's bits, as a
    // window has them maximised or not. Either alone is none: a popup with a
    // thin border alone (WS_BORDER) over all of its display is an app full
    // screen all the same. HELLDIVERS 2's (0x94880000, a pixel past the
    // display on every side) was taken for a titled window while either bit
    // was enough, and her taskbar stayed over the game
    // (zhoupengjie/Wakuwaku#63). The style's flags are all in its low 32 bits.
    fn titled(style: u32) -> bool {
        style & WS_CAPTION == WS_CAPTION
    }

    // Whether a window of this style, maximised or not, at this rect (left,
    // top, right, bottom) is full screen on a display at that one: without
    // a title bar, covering all of it (a pixel past it on every side, as
    // HELLDIVERS 2's, is all of it); with one by its style, covering it and
    // not maximised, or laid exactly on it. Kazumi's player full screen
    // (window_manager 0.5.1's setFullScreen) keeps WS_CAPTION, draws no
    // frame (its WM_NCCALCSIZE) and is set to the display's rect, maximised
    // still if it was; her taskbar stayed over its video
    // (zhoupengjie/Wakuwaku#98). Along the top, Windows' own put away and
    // set to hide itself, such a window was moved on to (-11, -11) past
    // every side, as one maximised over the whole display lies (the test
    // VM, 2026-10-30). A window maximised is never exactly on it: its frame
    // goes past the work area's edges.
    fn fills(style: u32, zoomed: bool, r: (i32, i32, i32, i32), m: (i32, i32, i32, i32)) -> bool {
        let covers = r.0 <= m.0 && r.1 <= m.1 && r.2 >= m.2 && r.3 >= m.3;
        if titled(style) {
            r == m || (covers && !zoomed)
        } else {
            covers
        }
    }

    // None: the foreground window is one of ours (keep what was decided).
    pub fn check(own: &[isize], on: &[(i32, i32, i32, i32)]) -> Option<bool> {
        covering(own).map(|c| c.is_some_and(|mon| on.is_empty() || on.contains(&mon)))
    }

    // The display an app full screen covers (x, y, w, h): Some(None) when
    // none does; None, the foreground window one of ours (keep what was
    // decided). Her taskbar on the other displays steps aside on its own
    // display alone (bars.rs).
    pub fn covering(own: &[isize]) -> Option<Option<(i32, i32, i32, i32)>> {
        // SAFETY: plain user32 calls; the buffers and structs are ours and sized.
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_null() {
                return Some(None);
            }
            if own.contains(&(hwnd as isize)) {
                return None;
            }
            if IsWindowVisible(hwnd) == 0 {
                return Some(None);
            }
            let mut name = [0u16; 256];
            let n = GetClassNameW(hwnd, name.as_mut_ptr(), 256).max(0) as usize;
            if SKIP_CLASSES.contains(&String::from_utf16_lossy(&name[..n]).as_str()) {
                return Some(None);
            }
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
            let mut rect = Rect::default();
            if GetWindowRect(hwnd, &mut rect) == 0 {
                return Some(None);
            }
            let mut info = MonitorInfo { size: std::mem::size_of::<MonitorInfo>() as u32, ..Default::default() };
            if GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &mut info) == 0 {
                return Some(None);
            }
            let m = &info.monitor;
            let mon = (m.left, m.top, m.right - m.left, m.bottom - m.top);
            let covers = fills(style, IsZoomed(hwnd) != 0, (rect.left, rect.top, rect.right, rect.bottom), (m.left, m.top, m.right, m.bottom));
            Some((covers && !SKIP_PROGRAMS.contains(&program_of(hwnd).as_str())).then_some(mon))
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
            // And the display it covers.
            let mut info = MonitorInfo { size: std::mem::size_of::<MonitorInfo>() as u32, ..Default::default() };
            GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &mut info);
            let m = &info.monitor;
            // Its class says which program it is: the person's (crash.rs private).
            format!("{} ({file}) exstyle {ex:#x} on ({}, {}, {}, {})", crate::crash::private(&class), m.left, m.top, m.right - m.left, m.bottom - m.top)
        }
    }

    pub const AVAILABLE: bool = true;

    #[cfg(test)]
    mod tests {
        use super::{fills, titled};

        #[test]
        fn what_fills_a_display() {
            let mon = (0, 0, 2560, 1440);
            // Kazumi's player full screen: WS_OVERLAPPEDWINDOW less the
            // sizing border and the maximise box, shown, on the display's
            // rect (zhoupengjie/Wakuwaku#98).
            assert!(fills(0x10ca_0000, false, mon, mon));
            // Maximised before it went full screen: still so.
            assert!(fills(0x11ca_0000, true, mon, mon));
            // Moved on past every side as one maximised over the whole
            // display lies, but not maximised.
            assert!(fills(0x10ca_0000, false, (-11, -11, 2571, 1451), mon));
            // A pixel short: a window placed, not a full screen.
            assert!(!fills(0x10ca_0000, false, (0, 0, 2560, 1439), mon));
            // A window maximised, its frame past the work area's edges: with
            // the taskbar's room kept, and with none (Windows' own hiding itself).
            assert!(!fills(0x15cf_0000, true, (-11, -11, 2571, 1379), mon));
            assert!(!fills(0x15cf_0000, true, (-11, -11, 2571, 1451), mon));
            // HELLDIVERS 2's, a pixel past it on every side (#63); and a
            // borderless one short of the foot.
            assert!(fills(0x9488_0000, false, (-1, -1, 2561, 1441), mon));
            assert!(!fills(0x9488_0000, false, (0, 0, 2560, 1368), mon));
            // On a display left of the main one.
            assert!(fills(0x10ca_0000, false, (-1920, 0, 0, 1080), (-1920, 0, 0, 1080)));
        }

        #[test]
        fn a_title_bar_is_both_bits() {
            // HELLDIVERS 2's: WS_POPUP, WS_VISIBLE, WS_CLIPSIBLINGS, WS_BORDER
            // and WS_SYSMENU, a thin border and no title bar
            // (zhoupengjie/Wakuwaku#63).
            assert!(!titled(0x9488_0000));
            // A window as most are (WS_OVERLAPPEDWINDOW, shown), and maximised.
            assert!(titled(0x14cf_0000));
            assert!(titled(0x15cf_0000));
            // A plain popup, shown or not; and one with a dialog frame alone.
            assert!(!titled(0x8000_0000));
            assert!(!titled(0x9000_0000));
            assert!(!titled(0x9040_0000));
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn check(_own: &[isize], _on: &[(i32, i32, i32, i32)]) -> Option<bool> {
        Some(false)
    }

    pub fn covering(_own: &[isize]) -> Option<Option<(i32, i32, i32, i32)>> {
        Some(None)
    }

    pub fn front() -> String {
        String::new()
    }

    pub const AVAILABLE: bool = false;
}

pub use imp::{check, covering, front, AVAILABLE};
