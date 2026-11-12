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
// Whoever gives it back then tells every window that Explorer's is the
// taskbar again (systray/ handed_back), as Explorer does when its taskbar
// comes up: hers, which was the first Shell_TrayWnd, is gone.
// Explorer shows it again now and then (a restart, the Start menu): the host
// puts it away again (rehide) when it sees it (visible(), and the message
// taskbar_created() names, sent when Explorer starts). Windows only.
use std::path::Path;

// The guard's command line: <exe> --taskbar-guard <pid> <file>.
pub const GUARD_ARG: &str = "--taskbar-guard";

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, Ordering};

    use serde_json::{json, Value};

    use crate::appbar::{self, Edge};

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
        fn MonitorFromWindow(hwnd: Hwnd, flags: u32) -> *mut c_void;
        fn GetMonitorInfoW(monitor: *mut c_void, info: *mut MonitorInfo) -> i32;
        fn GetShellWindow() -> Hwnd;
        fn FindWindowW(class: *const u16, title: *const u16) -> Hwnd;
        fn IsWindow(hwnd: Hwnd) -> i32;
        fn ShowWindowAsync(hwnd: Hwnd, cmd: i32) -> i32;
        fn SetWindowPos(hwnd: Hwnd, after: isize, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
    }

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

    const MONITOR_DEFAULTTONEAREST: u32 = 2;

    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
        fn CloseHandle(handle: Handle) -> i32;
        fn WaitForSingleObject(handle: Handle, ms: u32) -> u32;
        fn GetProcessTimes(process: Handle, created: *mut FileTime, exited: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> i32;
        fn GetExitCodeProcess(process: Handle, code: *mut u32) -> i32;
        fn QueryFullProcessImageNameW(process: Handle, flags: u32, name: *mut u16, size: *mut u32) -> i32;
        fn GetModuleHandleW(name: *const u16) -> isize;
        fn GetProcAddress(module: isize, name: *const u8) -> *const ();
    }

    const SW_HIDE: i32 = 0;
    const SW_SHOWNA: i32 = 8;
    const HWND_TOPMOST: isize = -1;
    const SWP_NOSIZE: u32 = 0x1;
    const SWP_NOMOVE: u32 = 0x2;
    const SWP_NOACTIVATE: u32 = 0x10;
    const SWP_ASYNCWINDOWPOS: u32 = 0x4000;
    const SYNCHRONIZE: u32 = 0x0010_0000;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const WAIT_TIMEOUT: u32 = 0x102;

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

    // A process's file name (explorer.exe), or "pid N" when it cannot be opened.
    fn exe_name(pid: u32) -> String {
        // SAFETY: a handle we close; the name goes into our own buffer.
        let name = unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process == 0 {
                None
            } else {
                let mut name = [0u16; 512];
                let mut size = 512u32;
                let ok = QueryFullProcessImageNameW(process, 0, name.as_mut_ptr(), &mut size) != 0;
                CloseHandle(process);
                ok.then(|| String::from_utf16_lossy(&name[..size as usize]))
            }
        };
        name.and_then(|path| path.rsplit('\\').next().map(str::to_string)).unwrap_or_else(|| format!("pid {pid}"))
    }

    fn is_explorer(pid: u32) -> bool {
        exe_name(pid).eq_ignore_ascii_case("explorer.exe")
    }

    // A window as the lone-Win check names it; None for none, or one gone.
    fn seen(hwnd: Hwnd) -> Option<super::Seen> {
        // SAFETY: a plain check of a window that may be gone.
        if hwnd.is_null() || unsafe { IsWindow(hwnd) } == 0 {
            return None;
        }
        let mut pid = 0;
        // SAFETY: our own out-parameter.
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        Some(super::Seen { class: class_of(hwnd), exe: exe_name(pid) })
    }

    // The desktop's task manager window (user32's GetTaskmanWindow, exported
    // by name but in no header); null when there is none, or no such export.
    fn taskman() -> Hwnd {
        // SAFETY: user32 is loaded in a process with windows; the export
        // takes nothing and returns a window handle.
        unsafe {
            let f = GetProcAddress(GetModuleHandleW(wide("user32.dll").as_ptr()), c"GetTaskmanWindow".as_ptr() as *const u8);
            if f.is_null() {
                return std::ptr::null_mut();
            }
            let f: extern "system" fn() -> Hwnd = std::mem::transmute(f);
            f()
        }
    }

    // What Win pressed alone reaches now (super::LoneWin): read only.
    pub fn lone_win() -> super::LoneWin {
        // SAFETY: plain lookups.
        let (shell, tray) = unsafe { (GetShellWindow(), FindWindowW(wide("Shell_TrayWnd").as_ptr(), std::ptr::null())) };
        super::LoneWin { taskman: seen(taskman()), shell: seen(shell), tray: seen(tray) }
    }

    // The main display's taskbar, Explorer's, or 0 while Explorer is not up.
    // Not one of the same class of ours (systray/), which programs are to
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

    // Whether one of Explorer's taskbars shows, on any display: hers is the
    // taskbar while she is up, and the displays without hers have none.
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

    // How long a process has run, or None when it is gone.
    pub fn running_for(pid: u32) -> Option<std::time::Duration> {
        let at = started(pid)?;
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?;
        // As started() has it: 100 ns since 1601.
        let now = (now.as_nanos() / 100) as u64 + 116_444_736_000_000_000;
        Some(std::time::Duration::from_nanos(now.saturating_sub(at) * 100))
    }

    // The process of Explorer's taskbar, or 0 while Explorer has none.
    pub fn explorer_pid() -> u32 {
        let tray = explorer_tray();
        let mut pid = 0;
        if tray != 0 {
            // SAFETY: our own out-parameter; a gone window leaves it 0.
            unsafe { GetWindowThreadProcessId(tray as Hwnd, &mut pid) };
        }
        pid
    }

    // Whether the desktop's shell window (Progman) is Explorer's yet.
    pub fn desktop_up() -> bool {
        // SAFETY: a plain lookup.
        let shell = unsafe { GetShellWindow() };
        let mut pid = 0;
        // SAFETY: our own out-parameter.
        !shell.is_null() && unsafe { GetWindowThreadProcessId(shell, &mut pid) } != 0 && is_explorer(pid)
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

    // Whether Windows' taskbar, put away, keeps its room along the bottom:
    // for her strip along the bottom, which lies over it; not for one along
    // another edge, where it would leave an empty band at the bottom of the
    // screen (set to hide itself it keeps none; restore gives its own state
    // back). On the displays without a strip of hers the band it keeps is
    // taken back (bars.rs keep_work).
    static KEEP_ROOM: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

    pub fn keep_room(keep: bool) {
        KEEP_ROOM.store(keep, Ordering::SeqCst);
    }

    // Whether it keeps it, as last set (keep_room).
    pub fn kept_room() -> bool {
        KEEP_ROOM.load(Ordering::SeqCst)
    }

    // Windows' taskbar put away on every display: hers is the taskbar while
    // she is up, on her display alone or on all of them (taskbarDisplays). A
    // display without hers has none at all, as with Windows' own "show my
    // taskbar on all displays" off: Windows' own was left to show there
    // (#4), but her taskbar on the main display alone is to mean no other
    // taskbar anywhere (the user, 2026-10-13).
    pub fn rehide() {
        let main = explorer_tray();
        let state = appbar::taskbar_state();
        let want = if KEEP_ROOM.load(Ordering::SeqCst) { state & !appbar::ABS_AUTOHIDE } else { state | appbar::ABS_AUTOHIDE };
        if main != 0 && want != state {
            appbar::set_taskbar_state(main, want);
        }
        for h in taskbars() {
            // SAFETY: a window Explorer made; hiding a gone one does nothing.
            unsafe { ShowWindow(h as Hwnd, SW_HIDE) };
        }
    }

    // Each taskbar of Explorer's shown again given its room: the work area of
    // its display stops at it, and the windows maximized there are laid on
    // that (appbar.rs refit_maximized). While she was up the room was taken
    // back where no strip of hers lay over it (bars.rs keep_work), and as it
    // shows again its work area need not be worked out anew. One hiding
    // itself (slid off the screen) has none.
    fn rooms_back() {
        for h in taskbars() {
            let mut r = Rect::default();
            let mut info = MonitorInfo { size: std::mem::size_of::<MonitorInfo>() as u32, ..Default::default() };
            // SAFETY: our own structs, of the sizes the calls write; a gone window fails them.
            let ok = unsafe { GetWindowRect(h as Hwnd, &mut r) != 0 && GetMonitorInfoW(MonitorFromWindow(h as Hwnd, MONITOR_DEFAULTTONEAREST), &mut info) != 0 };
            let m = &info.monitor;
            let mon = (m.left, m.top, m.right - m.left, m.bottom - m.top);
            let along_foot = r.bottom == m.bottom && r.left <= m.left && r.right >= m.right && r.top > m.top + mon.3 / 2;
            let Some(work) = appbar::work_of(mon).filter(|_| ok && along_foot) else { continue };
            if work.1 + work.3 > r.top {
                let to = appbar::Edge::Bottom.with_side(work, r.top);
                if appbar::set_work(to) {
                    appbar::refit_maximized(work, to);
                }
            }
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

    // The room of a taskbar of Explorer's along the bottom of a display while
    // it shows there (not hidden, not slid away): what the handover pictures.
    pub fn shown_room(mon: (i32, i32, i32, i32)) -> Option<(i32, i32, i32, i32)> {
        shown_bar(mon).filter(|(_, _, edge)| *edge == Edge::Bottom).map(|(_, r, _)| r)
    }

    // A taskbar of Explorer's shown along an edge of a display (the work
    // area stopping at it): its window (Shell_TrayWnd, or a second
    // display's Shell_SecondaryTrayWnd), its strip (physical, the display's
    // whole length) and the edge it is along, for her capsule to go into
    // it (embed.rs) and her island to lie over it (island/ fit); none
    // while none shows along any edge (hidden, slid away).
    pub fn shown_bar(mon: (i32, i32, i32, i32)) -> Option<(isize, (i32, i32, i32, i32), Edge)> {
        let (left, top, right, bottom) = (mon.0, mon.1, mon.0 + mon.2, mon.1 + mon.3);
        taskbars().into_iter().find_map(|h| {
            let mut r = Rect::default();
            // SAFETY: our own struct; a gone window fails both calls.
            let ok = unsafe { GetWindowRect(h as Hwnd, &mut r) } != 0 && unsafe { IsWindowVisible(h as Hwnd) } != 0;
            if !ok {
                return None;
            }
            let across = r.left <= left && r.right >= right;
            let along = r.top <= top && r.bottom >= bottom;
            if across && r.bottom == bottom && r.top > top + mon.3 / 2 {
                Some((h, (left, r.top, mon.2, r.bottom - r.top), Edge::Bottom))
            } else if across && r.top == top && r.bottom < top + mon.3 / 2 {
                Some((h, (left, top, mon.2, r.bottom - top), Edge::Top))
            } else if along && r.left == left && r.right < left + mon.2 / 2 {
                Some((h, (left, top, r.right - left, mon.3), Edge::Left))
            } else if along && r.right == right && r.left > left + mon.2 / 2 {
                Some((h, (r.left, top, right - r.left, mon.3), Edge::Right))
            } else {
                None
            }
        })
    }

    // Every taskbar of Explorer's shown again, its state (auto-hide) and the
    // file left for restore: the handover's quitting shows it under hers a
    // moment before she goes.
    pub fn show_bars() {
        for h in taskbars() {
            // SAFETY: as in rehide().
            unsafe { ShowWindow(h as Hwnd, SW_SHOWNA) };
        }
        rooms_back();
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
        rooms_back();
        std::fs::remove_file(file).is_ok()
    }

    // At start: put away by a process no longer running (both it and its guard gone)?
    pub fn recover(file: &Path) -> bool {
        // A mark of a run before (its guard ended before it could take it away).
        if read(&hang_mark(file)).and_then(|v| v["pid"].as_u64()) != Some(u64::from(std::process::id())) {
            let _ = std::fs::remove_file(hang_mark(file));
        }
        let Some(v) = read(file) else { return false };
        let (pid, at) = (v["pid"].as_u64().unwrap_or(0) as u32, v["started"].as_u64());
        if pid != std::process::id() && at.is_some() && started(pid) == at {
            return false;
        }
        restore(file)
    }

    // Once per process: the guard, started as any program starts another
    // (detach::start: out of her job should it let go, else in it, which
    // for a downloaded exe is Explorer's or none: detach::should_leave saw
    // to that as she started). Only while she is held in another program's
    // job (run from the source tree in a terminal's or an app's) is it
    // spawned with Explorer as its parent, as that job's end would take it
    // with her (detach.rs).
    pub fn spawn_guard(file: &Path) -> bool {
        if GUARDED.swap(true, Ordering::SeqCst) {
            return true;
        }
        let ok = crate::data::exe().is_ok_and(|exe| {
            let (pid, file) = (std::process::id().to_string(), file.to_string_lossy());
            let args = [super::GUARD_ARG, pid.as_str(), file.as_ref()];
            if crate::detach::held_elsewhere() { crate::detach::spawn(&exe, &args) } else { crate::detach::start(&exe, &args).is_ok() }
        });
        GUARDED.store(ok, Ordering::SeqCst);
        ok
    }

    // --- Her not answering --------------------------------------------------------
    // Her main thread or her taskbar's not answering (the guard's look every
    // two seconds, crash.rs Hang: Windows takes a window for hung after five
    // seconds without a look at its messages) left the screen with no
    // taskbar at all, hers stuck and Windows' put away, till the person
    // found Ctrl+Shift+Esc and ended her (the user, 2026-11-12: 卡住 10 秒就
    // 先还回). Ten seconds of it in all while hers holds the strip (the file
    // hers, written by her: not one left by a run before), and the guard
    // shows Windows' own again over hers. Nothing more: its state (auto-hide)
    // is set by a message to the first Shell_TrayWnd, hers, and the
    // windows refitted to a work area are hers among others, any of which
    // may be the one not answering; the guard waits on none of them. The
    // file is kept: her quitting, or the guard as she ends, gives the taskbar
    // back as ever. Set over hers (topmost, first), Explorer's tray is the
    // one a program finds meanwhile (FindWindow), so its icons go on
    // changing there; once she answers, hers is put first again and the
    // programs asked for their icons anew (taskbar/win/hold.rs
    // put_away_again: systray keep_first, ask_soon).
    // Beside the file, taskbar-hang.json says so while it lasts: her
    // taskbar's thread, should it still answer, leaves Windows' own up
    // (shown_for_hang) till the guard takes the mark away as she answers
    // again; its tick then sees it shown and puts it away (hold.rs).
    const HANG_BACK_AFTER: std::time::Duration = std::time::Duration::from_secs(5);

    pub fn hang_mark(file: &Path) -> std::path::PathBuf {
        file.with_file_name("taskbar-hang.json")
    }

    // Whether her guard has shown Windows' taskbar for a hang of this run's.
    pub fn shown_for_hang(file: &Path) -> bool {
        read(&hang_mark(file)).and_then(|v| v["pid"].as_u64()) == Some(u64::from(std::process::id()))
    }

    // Windows' taskbar as the person had it before she took it (its state in
    // the file: hidden by itself or not): she sets it to hide itself for a
    // strip along another edge than the bottom and for the top bar and dock
    // out of the way (rehide), and shown so it would stay slid away. Asked of
    // the first Shell_TrayWnd (SHAppBarMessage): her own taskbar's thread
    // asks it when it is the one answering (taskbar/win/hold.rs), the guard
    // only once that window is Explorer's, never hers.
    pub fn hang_state(file: &Path) -> bool {
        let Some(state) = read(file).and_then(|v| v["state"].as_u64()) else { return false };
        let main = explorer_tray();
        if main == 0 {
            return false;
        }
        appbar::set_taskbar_state(main, state as u32);
        true
    }

    // Whether the first Shell_TrayWnd (the one a message to the taskbar
    // reaches) is Explorer's.
    fn explorers_first() -> bool {
        // SAFETY: a plain lookup; our own out-parameter.
        let first = unsafe { FindWindowW(wide("Shell_TrayWnd").as_ptr(), std::ptr::null()) };
        let mut pid = 0;
        !first.is_null() && unsafe { GetWindowThreadProcessId(first, &mut pid) } != 0 && is_explorer(pid)
    }

    // Windows' taskbars shown and set over hers, the mark written first; none
    // when hers does not hold the strip or Explorer has no taskbar. Neither
    // call waits on Explorer (async), nor anything on her.
    fn show_for_hang(file: &Path, pid: u32) -> bool {
        if read(file).and_then(|v| v["pid"].as_u64()) != Some(u64::from(pid)) {
            return false;
        }
        let bars = taskbars();
        if bars.is_empty() || std::fs::write(hang_mark(file), json!({ "pid": pid }).to_string()).is_err() {
            return false;
        }
        for h in bars {
            // SAFETY: Explorer's windows; a gone one does nothing.
            unsafe {
                ShowWindowAsync(h as Hwnd, SW_SHOWNA);
                SetWindowPos(h as Hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS);
            }
        }
        true
    }

    // The guard's side: told every two seconds whether she is hung (her main
    // thread or her taskbar's), it shows Windows' taskbar once that has gone
    // on HANG_BACK_AFTER past Windows' five seconds, and takes the mark away
    // once she answers again; each said through `say`.
    #[derive(Default)]
    pub struct HangBack {
        since: Option<std::time::Instant>,
        shown: bool,
        // Its own state given back meanwhile (hang_state).
        state_back: bool,
    }

    // What a look comes to (HangBack::next).
    #[derive(Debug, PartialEq)]
    enum Next {
        Wait,
        Show,
        Answered,
    }

    impl HangBack {
        pub fn look(&mut self, file: &Path, pid: u32, hung: bool, say: impl Fn(&str)) {
            match self.next(hung, std::time::Instant::now()) {
                Next::Show if show_for_hang(file, pid) => {
                    self.shown = true;
                    say(&format!("pid {pid}: not answering ten seconds and more while her taskbar holds the strip: Windows' shown again over it till she answers (taskbar.json kept)"));
                }
                Next::Answered => {
                    self.state_back = false;
                    let _ = std::fs::remove_file(hang_mark(file));
                    say(&format!("pid {pid}: answering again: Windows' taskbar left for her to put away"));
                }
                _ => {}
            }
            // Shown, and the taskbar's first window Explorer's now (set over
            // hers, which answers nothing meanwhile): its own state back, so
            // that one set to stay slid away comes up whole.
            if self.shown && hung && !self.state_back && explorers_first() && hang_state(file) {
                self.state_back = true;
                say(&format!("pid {pid}: Windows' taskbar as it was before her (its own auto-hide or not)"));
            }
        }

        fn next(&mut self, hung: bool, now: std::time::Instant) -> Next {
            if !hung {
                self.since = None;
                return if std::mem::take(&mut self.shown) { Next::Answered } else { Next::Wait };
            }
            let since = *self.since.get_or_insert(now);
            if !self.shown && now.duration_since(since) >= HANG_BACK_AFTER {
                Next::Show
            } else {
                Next::Wait
            }
        }
    }

    #[cfg(test)]
    mod hang_tests {
        use super::*;
        use std::time::{Duration, Instant};

        // Hung: shown once it has gone on five seconds past Windows' own
        // five, once; answering again, the mark taken away, once; hung
        // again later, counted anew.
        #[test]
        fn shown_after_ten_seconds_and_once() {
            let mut b = HangBack::default();
            let t0 = Instant::now();
            assert_eq!(b.next(true, t0), Next::Wait);
            assert_eq!(b.next(true, t0 + Duration::from_secs(2)), Next::Wait);
            assert_eq!(b.next(true, t0 + Duration::from_secs(4)), Next::Wait);
            assert_eq!(b.next(true, t0 + Duration::from_secs(6)), Next::Show);
            b.shown = true;
            assert_eq!(b.next(true, t0 + Duration::from_secs(8)), Next::Wait);
            assert_eq!(b.next(false, t0 + Duration::from_secs(10)), Next::Answered);
            assert_eq!(b.next(false, t0 + Duration::from_secs(12)), Next::Wait);
            let t1 = t0 + Duration::from_secs(20);
            assert_eq!(b.next(true, t1), Next::Wait);
            assert_eq!(b.next(true, t1 + Duration::from_secs(5)), Next::Show);
        }

        // Not shown (hers did not hold the strip): asked again at each look,
        // and an answer then is nothing to say.
        #[test]
        fn not_shown_is_asked_again() {
            let mut b = HangBack::default();
            let t0 = Instant::now();
            assert_eq!(b.next(true, t0), Next::Wait);
            assert_eq!(b.next(true, t0 + Duration::from_secs(6)), Next::Show);
            assert_eq!(b.next(true, t0 + Duration::from_secs(8)), Next::Show);
            assert_eq!(b.next(false, t0 + Duration::from_secs(10)), Next::Wait);
        }

        // The mark is this run's only: one of another's is none, and gone
        // as she starts (recover).
        #[test]
        fn the_mark_is_this_runs() {
            let dir = std::env::temp_dir().join(format!("waku-hang-{}", std::process::id()));
            let _ = std::fs::create_dir_all(&dir);
            let file = dir.join("taskbar.json");
            std::fs::write(hang_mark(&file), json!({ "pid": std::process::id() }).to_string()).unwrap();
            assert!(shown_for_hang(&file));
            // No taskbar.json: nothing to give back, the mark kept.
            assert!(!recover(&file));
            assert!(hang_mark(&file).exists());
            std::fs::write(hang_mark(&file), json!({ "pid": std::process::id() + 1 }).to_string()).unwrap();
            assert!(!shown_for_hang(&file));
            assert!(!recover(&file));
            assert!(!hang_mark(&file).exists());
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    // The guard's whole life: wait for her to end, `watch` every two seconds
    // meanwhile (main.rs: whether she answers, crash.rs Hang), then `ended`
    // (main.rs: the desktop's icons where they are, icons.rs), then give the
    // taskbar back if she did not (trying for a minute, in case Explorer is
    // restarting), and meanwhile `after` (main.rs: whether she crashed,
    // crash.rs) with her exit code and when she started (ms since 1970).
    pub fn guard(pid: u32, file: &Path, mut watch: impl FnMut(), ended: impl FnOnce(), after: impl FnOnce(Option<u32>, Option<u64>) + Send + 'static) {
        // When she started, as ms since 1970.
        let started = started(pid).map(|t| (t / 10_000).saturating_sub(11_644_473_600_000));
        let mut code = None;
        // SAFETY: a handle we close; the code written into our own u32.
        unsafe {
            let process = OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process != 0 {
                while WaitForSingleObject(process, 2_000) == WAIT_TIMEOUT {
                    watch();
                }
                let mut c = 0u32;
                if GetExitCodeProcess(process, &mut c) != 0 {
                    code = Some(c);
                }
                CloseHandle(process);
            }
        }
        // Ended: no hang of hers to leave Windows' taskbar up for.
        let _ = std::fs::remove_file(hang_mark(file));
        ended();
        let meanwhile = std::thread::spawn(move || after(code, started));
        for _ in 0..30 {
            if !file.exists() {
                break;
            }
            // She ended holding it (killed): given back, and every window
            // told, which her own quitting does (zhoupengjie/Wakuwaku#54).
            if restore(file) {
                crate::systray::handed_back();
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
        let _ = meanwhile.join();
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
    pub fn keep_room(_keep: bool) {}
    pub fn kept_room() -> bool {
        true
    }
    pub fn room(_mon: (i32, i32, i32, i32)) -> Option<(i32, i32, i32, i32)> {
        None
    }
    pub fn shown_room(_mon: (i32, i32, i32, i32)) -> Option<(i32, i32, i32, i32)> {
        None
    }
    pub fn shown_bar(_mon: (i32, i32, i32, i32)) -> Option<(isize, (i32, i32, i32, i32), crate::appbar::Edge)> {
        None
    }
    pub fn show_bars() {}
    pub fn restore(_file: &Path) -> bool {
        false
    }
    pub fn recover(_file: &Path) -> bool {
        false
    }
    pub fn spawn_guard(_file: &Path) -> bool {
        false
    }
    pub fn guard(_pid: u32, _file: &Path, _watch: impl FnMut(), _ended: impl FnOnce(), _after: impl FnOnce(Option<u32>, Option<u64>) + Send + 'static) {}
    #[derive(Default)]
    pub struct HangBack;
    impl HangBack {
        pub fn look(&mut self, _file: &Path, _pid: u32, _hung: bool, _say: impl Fn(&str)) {}
    }
    pub fn shown_for_hang(_file: &Path) -> bool {
        false
    }
    pub fn hang_state(_file: &Path) -> bool {
        false
    }
    pub fn lone_win() -> super::LoneWin {
        super::LoneWin { taskman: None, shell: None, tray: None }
    }
    pub fn running_for(_pid: u32) -> Option<std::time::Duration> {
        None
    }
    pub fn explorer_pid() -> u32 {
        0
    }
    pub fn desktop_up() -> bool {
        false
    }
}

pub use imp::{desktop_up, explorer_pid, explorer_tray, guard, hang_state, hide, shown_for_hang, HangBack, keep_room, kept_room, lone_win, recover, rehide, restore, room, running_for, show_bars, shown_bar, shown_room, spawn_guard, started, taskbar_created, taskbars, visible};

// The shell's windows Win pressed alone may go by, none of them hers
// (zhoupengjie/Wakuwaku#54, where it opened no Start after she was quit,
// until Explorer was started again; not seen again in the test VM). Nothing
// documents its way; Open-Shell catches it at Progman, the desktop's shell
// window. The first Shell_TrayWnd is what a program reaches as the taskbar:
// hers while she holds it, Explorer's again once she gives it back. The
// task manager window (Explorer's task band, MSTaskSwWClass) is told as
// well, though it is not the way on 26300: cleared in the VM, Win alone
// still opened Start (2026-10-16); WM_SYSCOMMAND SC_TASKLIST posted to any
// of the three opens it. Read at each take and give back, and at a start
// after one was left put away, for the log: whether the shell window and
// the first tray window are Explorer's.
#[derive(Clone, Debug, PartialEq)]
pub struct Seen {
    pub class: String,
    // Its program's file name.
    pub exe: String,
}

impl Seen {
    fn explorers(&self) -> bool {
        self.exe.eq_ignore_ascii_case("explorer.exe")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LoneWin {
    pub taskman: Option<Seen>,
    pub shell: Option<Seen>,
    pub tray: Option<Seen>,
}

impl LoneWin {
    // The shell window and the first tray window Explorer's (the task
    // manager window only told).
    pub fn ok(&self) -> bool {
        [&self.shell, &self.tray].iter().all(|w| w.as_ref().is_some_and(Seen::explorers))
    }

    // For the log: "the shell's windows Explorer's: …", or not all.
    pub fn said(&self) -> String {
        let one = |what: &str, w: &Option<Seen>| w.as_ref().map_or_else(|| format!("no {what}"), |s| format!("{what} {} of {}", s.class, s.exe));
        let parts = [one("shell window", &self.shell), one("first tray window", &self.tray), one("taskman window", &self.taskman)];
        format!("{}: {}", if self.ok() { "the shell's windows Explorer's" } else { "the shell's windows not all Explorer's (Win alone may not open Start)" }, parts.join(", "))
    }
}

// The guard's arguments, when this process is one: its pid and file.
pub fn guard_args(args: &[String]) -> Option<(u32, &Path)> {
    match args {
        [_, flag, pid, file, ..] if flag == GUARD_ARG => Some((pid.parse().ok()?, Path::new(file))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(class: &str, exe: &str) -> Option<Seen> {
        Some(Seen { class: class.into(), exe: exe.into() })
    }

    #[test]
    fn the_shells_windows_are_explorers_or_not() {
        // As on 26300 with Explorer's taskbar its own.
        let fine = LoneWin { taskman: win("MSTaskSwWClass", "explorer.exe"), shell: win("Progman", "Explorer.EXE"), tray: win("Shell_TrayWnd", "explorer.exe") };
        assert!(fine.ok());
        assert_eq!(fine.said(), "the shell's windows Explorer's: shell window Progman of Explorer.EXE, first tray window Shell_TrayWnd of explorer.exe, taskman window MSTaskSwWClass of explorer.exe");
        // No task manager window: Win alone still opened Start in the VM; told only.
        let none = LoneWin { taskman: None, ..fine.clone() };
        assert!(none.ok());
        assert!(none.said().ends_with(", no taskman window"), "{}", none.said());
        // Hers still first after she gave it back (her window not gone).
        let hers = LoneWin { tray: win("Shell_TrayWnd", "wakuwaku.exe"), ..fine.clone() };
        assert!(!hers.ok());
        assert!(hers.said().starts_with("the shell's windows not all Explorer's (Win alone may not open Start): "), "{}", hers.said());
        assert!(hers.said().contains("first tray window Shell_TrayWnd of wakuwaku.exe"), "{}", hers.said());
        // Explorer not running: none of them.
        assert!(!LoneWin { taskman: None, shell: None, tray: None }.ok());
    }

    #[test]
    fn this_desktop_answers_the_lone_win_check() {
        // Whatever this machine has; only that it can be asked (on Windows
        // a desktop with Explorer has a shell window). Seen with --nocapture.
        let now = lone_win();
        println!("{}", now.said());
        if cfg!(windows) && now.shell.is_some() {
            assert!(now.shell.as_ref().is_some_and(|s| !s.class.is_empty()));
        }
    }

    // On a desktop with Explorer: its taskbar's process found, and how long
    // it has run, as taskbar/ waits on it (zhoupengjie/Wakuwaku#85).
    // Seen with --nocapture.
    #[test]
    fn this_desktops_explorer_has_run_so_long() {
        let pid = explorer_pid();
        println!("Explorer pid {pid}, up {:?}, desktop its: {}", running_for(pid), desktop_up());
        if cfg!(windows) && pid != 0 {
            assert!(running_for(pid).is_some_and(|d| d.as_secs() < 400 * 24 * 3600));
            assert!(running_for(std::process::id()).is_some_and(|d| d.as_secs() < 3600));
        }
    }
}
