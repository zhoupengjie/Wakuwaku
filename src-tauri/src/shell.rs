// Windows' own taskbar, put away while hers takes its place (display
// 'taskbar'), and given back.
//
// Put away: hidden on every display, and not set to hide itself, so it keeps
// its room along the bottom (a hidden one does), where hers lays its strip
// (room()). What Windows opens from its taskbar (the Start menu, the
// notifications and calendar, the quick settings, a notification's banner)
// opens above that room, clear of hers. Set to hide itself, as "Automatically
// hide the taskbar" does, it would keep no room, and those open at the
// screen's edge, over her strip (2026-10-10). Its state is written down
// first, in a small file, so that whoever is left can give it back:
//   - her, when the taskbar home goes and on quitting (restore)
//   - a guard: her own exe again, waiting for her process to end (spawn_guard,
//     guard), as a killed process cannot give it back itself
//   - her next start, if both were gone (recover)
// Explorer shows it again now and then (a restart, the Start menu): the host
// puts it away again (rehide) when it sees it (visible(), and the message
// taskbar_created() names, sent when Explorer starts). Windows only.
use std::path::Path;

// The guard's command line: <exe> --taskbar-guard <pid> <file>.
pub const GUARD_ARG: &str = "--taskbar-guard";

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::os::windows::process::CommandExt;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, Ordering};

    use serde_json::{json, Value};

    use crate::appbar;

    type Hwnd = *mut c_void;
    type Handle = isize;

    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn FindWindowExW(parent: Hwnd, after: Hwnd, class: *const u16, title: *const u16) -> Hwnd;
        fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
        fn EnumWindows(each: extern "system" fn(Hwnd, isize) -> i32, param: isize) -> i32;
        fn GetClassNameW(hwnd: Hwnd, name: *mut u16, max: i32) -> i32;
        fn IsWindowVisible(hwnd: Hwnd) -> i32;
        fn ShowWindow(hwnd: Hwnd, cmd: i32) -> i32;
        fn RegisterWindowMessageW(name: *const u16) -> u32;
        fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
    }

    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
        fn CloseHandle(handle: Handle) -> i32;
        fn WaitForSingleObject(handle: Handle, ms: u32) -> u32;
        fn GetProcessTimes(process: Handle, created: *mut FileTime, exited: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> i32;
        fn QueryFullProcessImageNameW(process: Handle, flags: u32, name: *mut u16, size: *mut u32) -> i32;
    }

    const SW_HIDE: i32 = 0;
    const SW_SHOWNA: i32 = 8;
    const SYNCHRONIZE: u32 = 0x0010_0000;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const WAIT_TIMEOUT: u32 = 0x102;
    const INFINITE: u32 = u32::MAX;
    const DETACHED_PROCESS: u32 = 0x8;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x200;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;

    static GUARDED: AtomicBool = AtomicBool::new(false);

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn class_of(hwnd: Hwnd) -> String {
        let mut name = [0u16; 64];
        // SAFETY: a buffer of our own, its size passed along.
        let n = unsafe { GetClassNameW(hwnd, name.as_mut_ptr(), 64) }.max(0) as usize;
        String::from_utf16_lossy(&name[..n])
    }

    extern "system" fn collect(hwnd: Hwnd, param: isize) -> i32 {
        if class_of(hwnd) == "Shell_SecondaryTrayWnd" {
            // SAFETY: param is the Vec taskbars() passed, alive for the enumeration.
            unsafe { (*(param as *mut Vec<isize>)).push(hwnd as isize) };
        }
        1
    }

    fn is_explorer(pid: u32) -> bool {
        // SAFETY: a handle we close; the name goes into our own buffer.
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process == 0 {
                return false;
            }
            let mut name = [0u16; 512];
            let mut size = 512u32;
            let ok = QueryFullProcessImageNameW(process, 0, name.as_mut_ptr(), &mut size) != 0;
            CloseHandle(process);
            let path = String::from_utf16_lossy(&name[..size as usize]);
            ok && path.rsplit('\\').next().is_some_and(|n| n.eq_ignore_ascii_case("explorer.exe"))
        }
    }

    // The main display's taskbar, Explorer's, or 0 while Explorer is not up.
    // Not one of the same class of ours (systray.rs), which programs are to
    // find first, whichever of her processes made it: a guard of one copy
    // once took another copy's for Explorer's and showed it, and two copies
    // handed each other the tray's messages (2026-10-10).
    pub fn explorer_tray() -> isize {
        let class = wide("Shell_TrayWnd");
        let mut at: Hwnd = std::ptr::null_mut();
        loop {
            // SAFETY: a class name of our own; walking the top-level windows.
            at = unsafe { FindWindowExW(std::ptr::null_mut(), at, class.as_ptr(), std::ptr::null()) };
            let mut pid = 0;
            // SAFETY: our own out-parameter.
            if at.is_null() || unsafe { GetWindowThreadProcessId(at, &mut pid) } != 0 && is_explorer(pid) {
                return at as isize;
            }
        }
    }

    // Every display's taskbar: the main one first.
    pub fn taskbars() -> Vec<isize> {
        let mut list = Vec::new();
        let main = explorer_tray();
        if main != 0 {
            list.push(main);
        }
        // SAFETY: the callback only pushes onto the Vec, which outlives the call.
        unsafe { EnumWindows(collect, &mut list as *mut Vec<isize> as isize) };
        list
    }

    pub fn visible() -> bool {
        // SAFETY: windows Explorer made; a gone one reads as not visible.
        taskbars().into_iter().any(|h| unsafe { IsWindowVisible(h as Hwnd) } != 0)
    }

    pub fn taskbar_created() -> u32 {
        // SAFETY: a name of our own, null-terminated.
        unsafe { RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) }
    }

    // When a process started (100 ns since 1601), or None when it is gone.
    pub fn started(pid: u32) -> Option<u64> {
        // SAFETY: a handle we close; the times are written into our own structs.
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, 0, pid);
            if process == 0 {
                return None;
            }
            let (mut created, mut exited, mut kernel, mut user) = (FileTime::default(), FileTime::default(), FileTime::default(), FileTime::default());
            let ok = GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user);
            // Its handle lets go only when it ends.
            let running = WaitForSingleObject(process, 0) == WAIT_TIMEOUT;
            CloseHandle(process);
            (ok != 0 && running).then(|| (u64::from(created.high) << 32) | u64::from(created.low))
        }
    }

    fn read(file: &Path) -> Option<Value> {
        serde_json::from_str(&std::fs::read_to_string(file).ok()?).ok()
    }

    pub fn hide(file: &Path) -> std::io::Result<()> {
        // Put away before (by a process since gone): its state then is the one to give back.
        let original = read(file).and_then(|v| v["state"].as_u64()).map_or_else(appbar::taskbar_state, |s| s as u32);
        let pid = std::process::id();
        std::fs::write(file, json!({ "pid": pid, "started": started(pid), "state": original }).to_string())?;
        rehide();
        Ok(())
    }

    pub fn rehide() {
        let main = explorer_tray();
        let state = appbar::taskbar_state();
        if main != 0 && state & appbar::ABS_AUTOHIDE != 0 {
            appbar::set_taskbar_state(main, state & !appbar::ABS_AUTOHIDE);
        }
        for h in taskbars() {
            // SAFETY: a window Explorer made; hiding a gone one does nothing.
            unsafe { ShowWindow(h as Hwnd, SW_HIDE) };
        }
    }

    // The room a taskbar of Explorer's keeps along the bottom of a display
    // (x, y, w, h, physical), hidden or not: one there, across its width.
    pub fn room(mon: (i32, i32, i32, i32)) -> Option<(i32, i32, i32, i32)> {
        let (bottom, right) = (mon.1 + mon.3, mon.0 + mon.2);
        taskbars().into_iter().find_map(|h| {
            let mut r = Rect::default();
            // SAFETY: our own struct; a gone window fails the call.
            let ok = unsafe { GetWindowRect(h as Hwnd, &mut r) } != 0;
            let there = ok && r.bottom == bottom && r.left <= mon.0 && r.right >= right && r.top > mon.1 + mon.3 / 2;
            there.then_some((r.left, r.top, r.right - r.left, r.bottom - r.top))
        })
    }

    // Its state set back and shown, and the file gone; false (the file kept)
    // when there was nothing to give back, or no Explorer to give it to.
    pub fn restore(file: &Path) -> bool {
        let Some(state) = read(file).and_then(|v| v["state"].as_u64()) else { return false };
        let main = explorer_tray();
        if main == 0 {
            return false;
        }
        appbar::set_taskbar_state(main, state as u32);
        for h in taskbars() {
            // SAFETY: as in rehide().
            unsafe { ShowWindow(h as Hwnd, SW_SHOWNA) };
        }
        std::fs::remove_file(file).is_ok()
    }

    // At start: put away by a process no longer running (both it and its guard gone)?
    pub fn recover(file: &Path) -> bool {
        let Some(v) = read(file) else { return false };
        let (pid, at) = (v["pid"].as_u64().unwrap_or(0) as u32, v["started"].as_u64());
        if pid != std::process::id() && at.is_some() && started(pid) == at {
            return false;
        }
        restore(file)
    }

    // Once per process: the guard, outside her process tree and her job.
    pub fn spawn_guard(file: &Path) -> bool {
        if GUARDED.swap(true, Ordering::SeqCst) {
            return true;
        }
        let Ok(exe) = std::env::current_exe() else { return false };
        let spawn = |flags: u32| {
            std::process::Command::new(&exe)
                .arg(super::GUARD_ARG)
                .arg(std::process::id().to_string())
                .arg(file)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .creation_flags(flags)
                .spawn()
        };
        let flags = DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP;
        let ok = spawn(flags | CREATE_BREAKAWAY_FROM_JOB).or_else(|_| spawn(flags)).is_ok();
        GUARDED.store(ok, Ordering::SeqCst);
        ok
    }

    // The guard's whole life: wait for her to end, then give the taskbar back
    // if she did not (trying for a minute, in case Explorer is restarting).
    pub fn guard(pid: u32, file: &Path) {
        // SAFETY: a handle we close.
        unsafe {
            let process = OpenProcess(SYNCHRONIZE, 0, pid);
            if process != 0 {
                WaitForSingleObject(process, INFINITE);
                CloseHandle(process);
            }
        }
        for _ in 0..30 {
            if !file.exists() || restore(file) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use std::path::Path;

    pub fn explorer_tray() -> isize {
        0
    }
    pub fn started(_pid: u32) -> Option<u64> {
        None
    }
    pub fn taskbars() -> Vec<isize> {
        Vec::new()
    }
    pub fn visible() -> bool {
        false
    }
    pub fn taskbar_created() -> u32 {
        0
    }
    pub fn hide(_file: &Path) -> std::io::Result<()> {
        Ok(())
    }
    pub fn rehide() {}
    pub fn room(_mon: (i32, i32, i32, i32)) -> Option<(i32, i32, i32, i32)> {
        None
    }
    pub fn restore(_file: &Path) -> bool {
        false
    }
    pub fn recover(_file: &Path) -> bool {
        false
    }
    pub fn spawn_guard(_file: &Path) -> bool {
        false
    }
    pub fn guard(_pid: u32, _file: &Path) {}
}

pub use imp::{explorer_tray, guard, hide, recover, rehide, restore, room, spawn_guard, started, taskbar_created, taskbars, visible};

// The guard's arguments, when this process is one: its pid and file.
pub fn guard_args(args: &[String]) -> Option<(u32, &Path)> {
    match args {
        [_, flag, pid, file, ..] if flag == GUARD_ARG => Some((pid.parse().ok()?, Path::new(file))),
        _ => None,
    }
}
