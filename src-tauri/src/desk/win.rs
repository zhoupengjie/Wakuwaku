// Windows' side of the desktop's windows (desk/mod.rs imp).
// Windows' calls other parts make too are win32.rs's; those below are
// the desktop's alone.
use std::sync::Arc;

use super::desks;
use crate::Shared;
use crate::win32::{Hwnd, Msg, WndClassExW, CallNextHookEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, FindWindowExW, FindWindowW, GetAsyncKeyState, GetClassNameW, GetCurrentThreadId, GetMessageW, GetModuleHandleW, GetTickCount, GetWindow, GetWindowLongPtrW, IsIconic, IsWindowVisible, KillTimer, PostMessageW, PostQuitMessage, PostThreadMessageW, RegisterClassExW, SetTimer, SetWinEventHook, SetWindowLongPtrW, SetWindowPos, SetWindowsHookExW, ShowWindow, TranslateMessage, UnhookWinEvent, UnhookWindowsHookEx};

#[repr(C)]
struct DisplayDevice {
    size: u32,
    name: [u16; 32],
    string: [u16; 128],
    flags: u32,
    id: [u16; 128],
    key: [u16; 128],
}

#[link(name = "user32")]
extern "system" {
    fn EnumDisplayDevicesW(device: *const u16, index: u32, dd: *mut DisplayDevice, flags: u32) -> i32;
}

const EDD_GET_DEVICE_INTERFACE_NAME: u32 = 1;
const GW_HWNDNEXT: u32 = 2;
const GW_HWNDPREV: u32 = 3;
const GWL_EXSTYLE: i32 = -20;
const WS_EX_TOPMOST: isize = 0x8;
const WS_EX_TOOLWINDOW: u32 = 0x80;
const WS_POPUP: u32 = 0x8000_0000;
const HWND_TOP: isize = 0;
const HWND_BOTTOM: isize = 1;
const HWND_TOPMOST: isize = -1;
const HWND_NOTOPMOST: isize = -2;
const SWP_NOSIZE: u32 = 0x1;
const SWP_NOMOVE: u32 = 0x2;
const SWP_NOACTIVATE: u32 = 0x10;
const SWP_NOOWNERZORDER: u32 = 0x200;
const SWP_ASYNCWINDOWPOS: u32 = 0x4000;
const EVENT_SYSTEM_FOREGROUND: u32 = 0x3;
const EVENT_SYSTEM_MINIMIZESTART: u32 = 0x16;
const EVENT_SYSTEM_MINIMIZEEND: u32 = 0x17;
const WINEVENT_OUTOFCONTEXT: u32 = 0;
const WINEVENT_SKIPOWNPROCESS: u32 = 2;
const WM_TIMER: u32 = 0x0113;
const WM_SETTINGCHANGE: u32 = 0x001A;
const WM_DISPLAYCHANGE: u32 = 0x007E;
const WM_DWMCOLORIZATIONCOLORCHANGED: u32 = 0x0320;
const WM_APP_LOW: u32 = 0x8000 + 0x61;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn text(w: &[u16]) -> String {
    String::from_utf16_lossy(&w[..w.iter().position(|&c| c == 0).unwrap_or(w.len())])
}

// A display's monitor's device interface, by the display's device name.
pub fn device_of(name: &str) -> Option<String> {
    // SAFETY: our own struct, of the size the call writes; the name ours.
    unsafe {
        let mut dd: DisplayDevice = std::mem::zeroed();
        dd.size = std::mem::size_of::<DisplayDevice>() as u32;
        (EnumDisplayDevicesW(wide(name).as_ptr(), 0, &mut dd, EDD_GET_DEVICE_INTERFACE_NAME) != 0 && dd.id[0] != 0).then(|| text(&dd.id))
    }
}

// The window the desktop's icons are in (the top one): Progman holding
// them (Windows 11 24H2, and before anything asked for a wallpaper of
// its own), else the WorkerW that does (Windows 10, 11 before 24H2).
fn host() -> Hwnd {
    // SAFETY: plain lookups by class.
    unsafe {
        let defview = wide("SHELLDLL_DefView");
        let progman = FindWindowW(wide("Progman").as_ptr(), std::ptr::null());
        if !progman.is_null() && !FindWindowExW(progman, std::ptr::null_mut(), defview.as_ptr(), std::ptr::null()).is_null() {
            return progman;
        }
        let worker = wide("WorkerW");
        let mut w: Hwnd = std::ptr::null_mut();
        loop {
            w = FindWindowExW(std::ptr::null_mut(), w, worker.as_ptr(), std::ptr::null());
            if w.is_null() {
                return progman;
            }
            if !FindWindowExW(w, std::ptr::null_mut(), defview.as_ptr(), std::ptr::null()).is_null() {
                return w;
            }
        }
    }
}

// Windows' own taskbars: a display's other than the main one's lies
// right over the desktop too (not kept on top in the VM, 2026-10-16),
// and Explorer puts it back there; hers stay clear of it (the work
// area), and it goes unfought.
fn is_taskbar(h: Hwnd) -> bool {
    let mut name = [0u16; 32];
    // SAFETY: a buffer of our own.
    let k = unsafe { GetClassNameW(h, name.as_mut_ptr(), 32) }.max(0) as usize;
    matches!(String::from_utf16_lossy(&name[..k]).as_str(), "Shell_TrayWnd" | "Shell_SecondaryTrayWnd")
}

fn topmost(h: Hwnd) -> bool {
    // SAFETY: a plain read.
    unsafe { GetWindowLongPtrW(h, GWL_EXSTYLE) & WS_EX_TOPMOST != 0 }
}

// Right over the host: nothing in sight between them but hers.
fn right_over(ours: Hwnd, host: Hwnd, mine: &[isize]) -> bool {
    // SAFETY: plain reads along the z-order.
    unsafe {
        let mut h = GetWindow(ours, GW_HWNDNEXT);
        let mut n = 0;
        while !h.is_null() && n < 8192 {
            if h == host {
                return true;
            }
            if IsWindowVisible(h) != 0 && !mine.contains(&(h as isize)) && !is_taskbar(h) {
                return false;
            }
            h = GetWindow(h, GW_HWNDNEXT);
            n += 1;
        }
        false
    }
}

// One of hers kept right over the window with the icons: after the
// window over that one (or at the top of its band), topmost while it is
// (the desktop shown). Some(what was done), None when it was there.
fn keep_low(ours: Hwnd, mine: &[isize]) -> Option<&'static str> {
    let host = host();
    let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_ASYNCWINDOWPOS;
    // SAFETY: our own window; the others only named.
    unsafe {
        if host.is_null() {
            SetWindowPos(ours, HWND_BOTTOM, 0, 0, 0, 0, flags);
            return Some("at the bottom (no desktop window)");
        }
        let raised = topmost(host);
        if topmost(ours) == raised && right_over(ours, host, mine) {
            return None;
        }
        if topmost(ours) != raised {
            SetWindowPos(ours, if raised { HWND_TOPMOST } else { HWND_NOTOPMOST }, 0, 0, 0, 0, flags);
        }
        let mut above = GetWindow(host, GW_HWNDPREV);
        while !above.is_null() && mine.contains(&(above as isize)) {
            above = GetWindow(above, GW_HWNDPREV);
        }
        let after = if above.is_null() || topmost(above) != raised {
            if raised {
                HWND_TOPMOST
            } else {
                HWND_TOP
            }
        } else {
            above as isize
        };
        SetWindowPos(ours, after, 0, 0, 0, 0, flags);
        Some(if raised {
            "over the desktop shown (topmost)"
        } else if after == HWND_TOP {
            "over the desktop brought to the front"
        } else {
            "right over the desktop, under the windows"
        })
    }
}

// The keyboard taken for a board's name, or let go: its WS_EX_NOACTIVATE
// off (not tao's set_focusable, which sets all its styles anew, the
// clicks let through among them), the window to the front by borrowing
// the input of the one there (jump.rs bring_window, as the settings take
// it); then that one's again, and this one back right over the desktop.
static BEFORE: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);
pub fn take_keys(hwnd: isize, on: bool) {
    const WS_EX_NOACTIVATE: isize = 0x0800_0000;
    let mine = hwnd as Hwnd;
    // SAFETY: our own window's style; the one before only named (focus.rs checks it is still there).
    unsafe {
        let ex = GetWindowLongPtrW(mine, GWL_EXSTYLE);
        if on {
            let before = crate::focus::foreground();
            if before != hwnd {
                BEFORE.store(before, std::sync::atomic::Ordering::SeqCst);
            }
            SetWindowLongPtrW(mine, GWL_EXSTYLE, ex & !WS_EX_NOACTIVATE);
            crate::agents::jump::bring_window(hwnd);
        } else {
            SetWindowLongPtrW(mine, GWL_EXSTYLE, ex | WS_EX_NOACTIVATE);
            if crate::focus::foreground() == hwnd {
                crate::focus::set_foreground(BEFORE.swap(0, std::sync::atomic::Ordering::SeqCst));
            }
            keep_all_low();
        }
    }
}

// Clicks through it, or taken: its own extended style set (not tao's
// set_ignore_cursor_events, which shows the window again, SW_SHOW, as
// it sets any style: brought up over the others).
pub fn set_through(hwnd: isize, through: bool) {
    const WS_EX_TRANSPARENT: isize = 0x20;
    const WS_EX_LAYERED: isize = 0x8_0000;
    // SAFETY: our own window, on its own thread (main.rs: run on the main thread).
    unsafe {
        let ex = GetWindowLongPtrW(hwnd as Hwnd, GWL_EXSTYLE);
        let want = if through { ex | WS_EX_TRANSPARENT | WS_EX_LAYERED } else { ex & !(WS_EX_TRANSPARENT | WS_EX_LAYERED) };
        if want != ex {
            SetWindowLongPtrW(hwnd as Hwnd, GWL_EXSTYLE, want);
        }
    }
}

// The desktop brought over the windows (Win+D): a window of someone else's
// in sight under the one with the icons.
pub fn desktop_shown(own: &[isize]) -> bool {
    let host = host();
    if host.is_null() {
        return false;
    }
    // SAFETY: plain reads along the z-order; a buffer of our own.
    unsafe {
        let mut h = GetWindow(host, GW_HWNDNEXT);
        let mut n = 0;
        while !h.is_null() && n < 8192 {
            if IsWindowVisible(h) != 0 && IsIconic(h) == 0 && !own.contains(&(h as isize)) {
                let mut name = [0u16; 32];
                let k = GetClassNameW(h, name.as_mut_ptr(), 32).max(0) as usize;
                let class = String::from_utf16_lossy(&name[..k]);
                if class != "WorkerW" && class != "Progman" {
                    return true;
                }
            }
            h = GetWindow(h, GW_HWNDNEXT);
            n += 1;
        }
        false
    }
}

// Every one of hers where it belongs: right over the desktop; or, while
// a widget is dragged over it from her home (RAISED), over the other
// windows, right under her home's (the top of those kept on top, then
// after hers).
pub fn keep_all_low() {
    let mine: Vec<isize> = desks().iter().map(|d| d.hwnd).filter(|&h| h != 0).collect();
    if super::RAISED.load(std::sync::atomic::Ordering::SeqCst) {
        let home = super::SHARED.get().map_or(0, |sh| crate::island::hwnd(sh));
        let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_ASYNCWINDOWPOS;
        for &h in &mine {
            // SAFETY: our own windows; hers only named.
            unsafe {
                SetWindowPos(h as Hwnd, HWND_TOPMOST, 0, 0, 0, 0, flags);
                if home != 0 {
                    SetWindowPos(h as Hwnd, home, 0, 0, 0, 0, flags);
                }
            }
        }
        return;
    }
    for &h in &mine {
        if let Some(what) = keep_low(h as Hwnd, &mine) {
            super::count(&super::PLACED);
            if let Some(sh) = super::SHARED.get() {
                sh.log(&format!("desk: {what}"));
            }
        }
    }
}

// The window in front changed (Win+D among others), or one came back
// from the taskbar: hers where they belong.
extern "system" fn on_event(_hook: isize, _event: u32, _hwnd: Hwnd, _object: i32, _child: i32, _thread: u32, _time: u32) {
    keep_all_low();
    // Another window in front: an open menu closes.
    if let Some(sh) = super::SHARED.get() {
        super::close_menus(sh);
    }
    // Looked at again a moment later, and once more: shown the desktop
    // (Win+D), the window with the icons comes to the front first and
    // only then over the others (the VM, 2026-10-16: hers were put right
    // over it while it still lay at the bottom, and stayed under it).
    let hwnd = WINDOW.load(std::sync::atomic::Ordering::SeqCst);
    if hwnd != 0 {
        // SAFETY: our own window, on this thread.
        unsafe {
            SetTimer(hwnd as Hwnd, AGAIN_SOON, 250, std::ptr::null());
            SetTimer(hwnd as Hwnd, AGAIN_LATER, 1000, std::ptr::null());
        }
    }
    // A desktop seen again: its words caught up.
    if let Some(sh) = super::SHARED.get() {
        super::catch_up(sh);
    }
}

// Its timers, all on its hidden window: the two looks after the window in
// front changes; a desktop waiting to be told its widgets' words (armed
// only while one waits); the picture of a slideshow or of Spotlight
// (armed only while the desktop has one: a plain picture says when it
// changes, WM_SETTINGCHANGE); the looks after that word, which comes
// before the picture is in place (wallpaper.rs SETTLE_MS: armed for
// five seconds after it, SETTLE_LEFT looks left).
const PAPER: usize = 1;
const AGAIN_SOON: usize = 2;
const AGAIN_LATER: usize = 3;
const STALE: usize = 4;
const SETTLE: usize = 5;
static SETTLE_LEFT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
const WM_APP_STALE: u32 = 0x8000 + 0x62;
const WM_APP_STOP: u32 = 0x8000 + 0x63;

// A picture that changes by itself: a slideshow (2) or Spotlight (3)
// (Settings' choice, as wallpaper.rs reads it).
fn picture_turns() -> bool {
    let key = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Wallpapers");
    matches!(key.ok().and_then(|k| k.get_value::<u32, _>("BackgroundType").ok()), Some(2 | 3))
}

// The picture's timer as the desktop's picture is: a minute while it
// turns by itself, none otherwise.
fn arm_paper(hwnd: Hwnd) {
    // SAFETY: our own window's timer, on its thread.
    unsafe {
        if picture_turns() {
            SetTimer(hwnd, PAPER, 60_000, std::ptr::null());
        } else {
            KillTimer(hwnd, PAPER);
        }
    }
}

extern "system" fn wndproc(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize {
    super::count(&super::THREAD_MSGS);
    match msg {
        // The picture, Windows' colours: looked at again, off this
        // thread (Explorer answers the picture's questions), now and
        // each half second for five (SETTLE; set again, it starts over).
        WM_SETTINGCHANGE | WM_DWMCOLORIZATIONCOLORCHANGED => {
            arm_paper(hwnd);
            std::thread::spawn(super::look_again);
            SETTLE_LEFT.store(crate::wallpaper::SETTLE_LOOKS, std::sync::atomic::Ordering::SeqCst);
            // SAFETY: our own window's timer, on its thread.
            unsafe { SetTimer(hwnd, SETTLE, crate::wallpaper::SETTLE_MS, std::ptr::null()) };
            0
        }
        WM_TIMER if wparam == SETTLE => {
            let left = SETTLE_LEFT.load(std::sync::atomic::Ordering::SeqCst).saturating_sub(1);
            SETTLE_LEFT.store(left, std::sync::atomic::Ordering::SeqCst);
            if left == 0 {
                // SAFETY: our own window's timer.
                unsafe { KillTimer(hwnd, SETTLE) };
            }
            std::thread::spawn(super::look_again);
            0
        }
        // The displays: the windows with them, once they have settled.
        WM_DISPLAYCHANGE => {
            if let Some(sh) = super::SHARED.get() {
                let sh = sh.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(1500));
                    super::sync(&sh);
                    super::look_again();
                });
            }
            0
        }
        WM_TIMER if wparam == AGAIN_SOON || wparam == AGAIN_LATER => {
            // SAFETY: our own window's one-shot timer.
            unsafe { KillTimer(hwnd, wparam) };
            keep_all_low();
            if let Some(sh) = super::SHARED.get() {
                super::catch_up(sh);
            }
            0
        }
        // A desktop waiting to be told (a window restored without
        // another coming to the front): looked at every ten seconds
        // while one waits, then the timer goes.
        WM_TIMER if wparam == STALE => {
            if let Some(sh) = super::SHARED.get() {
                super::catch_up(sh);
            }
            if !super::any_stale() {
                // SAFETY: our own window's timer.
                unsafe { KillTimer(hwnd, STALE) };
            }
            0
        }
        // A slideshow's or Spotlight's next picture says nothing: looked
        // at each minute while someone is about.
        WM_TIMER if wparam == PAPER => {
            if !super::SHARED.get().is_some_and(|sh| sh.is_quiet()) {
                std::thread::spawn(super::look_again);
            }
            0
        }
        WM_APP_STALE => {
            // SAFETY: our own window's timer.
            unsafe { SetTimer(hwnd, STALE, 10_000, std::ptr::null()) };
            0
        }
        WM_APP_LOW => {
            keep_all_low();
            0
        }
        WM_APP_STOP => {
            // SAFETY: this thread's own loop ended.
            unsafe { PostQuitMessage(0) };
            0
        }
        // SAFETY: the default for the rest.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

static WINDOW: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);
static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

// Its thread, while there is a desktop window: a hidden window that hears
// Windows' broadcasts, the hooks on the window in front, its timers (none
// running while nothing waits). Asked again: hers put where they belong,
// on that thread. Gone with the last desktop window (stop).
pub fn start(sh: &Arc<Shared>) {
    let _ = super::SHARED.set(sh.clone());
    if STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        let hwnd = WINDOW.load(std::sync::atomic::Ordering::SeqCst);
        if hwnd != 0 {
            // SAFETY: our own window; the message carries nothing.
            unsafe { PostMessageW(hwnd as Hwnd, WM_APP_LOW, 0, 0) };
        }
        return;
    }
    super::trace_main(sh);
    crate::spawn_named("desk", || {
        crate::power::thread_eco();
        // SAFETY: our own class, window and hooks, on this thread, which runs their loop.
        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let class = wide("WakuwakuDesk");
            let wc = WndClassExW { size: std::mem::size_of::<WndClassExW>() as u32, style: 0, wndproc, cls_extra: 0, wnd_extra: 0, instance, icon: 0, cursor: 0, background: 0, menu_name: std::ptr::null(), class_name: class.as_ptr(), icon_sm: 0 };
            RegisterClassExW(&wc);
            let hwnd = CreateWindowExW(WS_EX_TOOLWINDOW, class.as_ptr(), std::ptr::null(), WS_POPUP, 0, 0, 0, 0, std::ptr::null_mut(), 0, instance, std::ptr::null_mut());
            WINDOW.store(hwnd as isize, std::sync::atomic::Ordering::SeqCst);
            let hooks = [
                SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, 0, on_event, 0, 0, WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS),
                SetWinEventHook(EVENT_SYSTEM_MINIMIZEEND, EVENT_SYSTEM_MINIMIZEEND, 0, on_event, 0, 0, WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS),
                SetWinEventHook(EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZESTART, 0, on_event, 0, 0, WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS),
            ];
            arm_paper(hwnd);
            if super::any_stale() {
                SetTimer(hwnd, STALE, 10_000, std::ptr::null());
            }
            keep_all_low();
            keys_start();
            let mut msg: Msg = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            keys_stop();
            for h in hooks {
                UnhookWinEvent(h);
            }
            WINDOW.store(0, std::sync::atomic::Ordering::SeqCst);
            DestroyWindow(hwnd);
        }
        STARTED.store(false, std::sync::atomic::Ordering::SeqCst);
        if let Some(sh) = super::SHARED.get() {
            sh.log("desk: its thread gone with the last desktop window");
        }
    });
}

// A turn of the wheel with Ctrl, to the desktop's icon view (the list in
// SHELLDLL_DefView, in the window with the icons): posted, as a wheel's
// message comes (its turn in the high word of wparam, the keys held in
// the low; the point the screen's, a signed 16-bit pair in lparam).
pub fn wheel_to_icons(x: i32, y: i32, delta: i32) -> bool {
    const WM_MOUSEWHEEL: u32 = 0x020A;
    const MK_CONTROL: usize = 0x0008;
    let host = host();
    if host.is_null() {
        return false;
    }
    // SAFETY: plain lookups by class; a posted message carrying no pointers.
    unsafe {
        let view = FindWindowExW(host, std::ptr::null_mut(), wide("SHELLDLL_DefView").as_ptr(), std::ptr::null());
        if view.is_null() {
            return false;
        }
        let list = FindWindowExW(view, std::ptr::null_mut(), wide("SysListView32").as_ptr(), std::ptr::null());
        let to = if list.is_null() { view } else { list };
        let wparam = ((delta as i16 as u16 as usize) << 16) | MK_CONTROL;
        let lparam = (((y as i16 as u16 as u32) << 16) | (x as i16 as u16 as u32)) as isize;
        PostMessageW(to, WM_MOUSEWHEEL, wparam, lparam) != 0
    }
}

// How long ago Ctrl's key last went down or up (ms; the trace's).
pub fn ctrl_age() -> u32 {
    // SAFETY: a plain query.
    unsafe { GetTickCount() }.wrapping_sub(CTRL_AT.load(std::sync::atomic::Ordering::Relaxed))
}

// Shown, not made the active window (SW_SHOWNOACTIVATE). Tao's show
// (SW_SHOW) made a new desktop window the active one, the foreground
// one too while her home's window had it: a widget being dragged from
// her home's picker was let go of there, as her home's page lost the
// keyboard (tried, 2026-10-17). Tao is not told: nothing of tao's sets
// these windows' styles once they are up (set_through, take_keys are
// our own), which would hide one again.
pub fn show_quiet(hwnd: isize) {
    const SW_SHOWNOACTIVATE: i32 = 4;
    // SAFETY: our own window.
    unsafe { ShowWindow(hwnd as Hwnd, SW_SHOWNOACTIVATE) };
}

// Ctrl held, either of them, now. Its key just pressed or let go (the
// hook's word, CTRL_FRESH_MS at most): as the hook has it, for the look
// it woke runs before Windows' own key state has changed (a Ctrl and a
// press 30 ms after it went to Explorer, tried 2026-10-17); else
// Windows' (should the hook have missed a key: one let go over a window
// run as administrator).
pub fn ctrl_down() -> bool {
    const VK_CONTROL: i32 = 0x11;
    const CTRL_FRESH_MS: u32 = 150;
    use std::sync::atomic::Ordering::Relaxed;
    // SAFETY: plain queries; the high bit is the key's state now.
    unsafe {
        if GetTickCount().wrapping_sub(CTRL_AT.load(Relaxed)) < CTRL_FRESH_MS {
            return CTRL_HELD.load(Relaxed);
        }
        GetAsyncKeyState(VK_CONTROL) as u16 & 0x8000 != 0
    }
}

// Ctrl's keys going down and up anywhere: the pointer's look woken at
// once (loops.rs wake_pointer), not at its pace, so a press right after
// Ctrl reaches the desktop's page (poll). A low-level keyboard hook
// (WH_KEYBOARD_LL) on a thread of its own that does nothing else:
// Windows hands it every key on the machine and waits for its answer,
// and drops one that answers late (winkeys.rs has the same); it looks at
// Ctrl's keys alone, and wakes the loop as Ctrl changes. Nothing of it
// runs while no key is touched. In with the desk thread, out with it.
// Deaf while one of her own windows has the keyboard (WebView2 in this
// process focused: Windows calls none of its low-level hooks), when the
// look comes every 40 ms over a desktop with widgets instead (poll).
#[repr(C)]
struct KbdLl {
    vk: u32,
    scan: u32,
    flags: u32,
    time: u32,
    extra: usize,
}
static CTRL_HELD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
// When Ctrl last went down or up (the key's own time, GetTickCount's).
static CTRL_AT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static KEYS_THREAD: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

extern "system" fn on_key(code: i32, wparam: usize, lparam: isize) -> isize {
    const LLKHF_UP: u32 = 0x80;
    if code == 0 && lparam != 0 {
        // SAFETY: Windows hands a KBDLLHOOKSTRUCT with HC_ACTION.
        let k = unsafe { &*(lparam as *const KbdLl) };
        // VK_CONTROL, VK_LCONTROL, VK_RCONTROL. Windows' key state is
        // still as it was before this key: held down, a key repeats (no
        // change); let go, the other Ctrl may still be held. Read so,
        // not from what this hook last heard, which misses keys while
        // her window has the keyboard.
        if matches!(k.vk, 0x11 | 0xA2 | 0xA3) {
            let other = match k.vk {
                0xA2 => 0xA3,
                0xA3 => 0xA2,
                _ => 0,
            };
            // SAFETY: plain queries.
            let (was, other_held) = unsafe { (GetAsyncKeyState(0x11) as u16 & 0x8000 != 0, other != 0 && GetAsyncKeyState(other) as u16 & 0x8000 != 0) };
            let held = k.flags & LLKHF_UP == 0 || other_held;
            if held != was {
                CTRL_HELD.store(held, std::sync::atomic::Ordering::Relaxed);
                CTRL_AT.store(k.time, std::sync::atomic::Ordering::Relaxed);
                crate::loops::wake_pointer();
            }
        }
    }
    // SAFETY: passed on, as every hook must.
    unsafe { CallNextHookEx(0, code, wparam, lparam) }
}

fn keys_start() {
    const WH_KEYBOARD_LL: i32 = 13;
    let (tx, rx) = std::sync::mpsc::channel();
    crate::spawn_named("desk ctrl", move || {
        // SAFETY: a hook of our own, on this thread, which runs its loop.
        unsafe {
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, on_key, GetModuleHandleW(std::ptr::null()), 0);
            KEYS_THREAD.store(GetCurrentThreadId(), std::sync::atomic::Ordering::SeqCst);
            let _ = tx.send(hook != 0);
            let mut msg: Msg = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {}
            if hook != 0 {
                UnhookWindowsHookEx(hook);
            }
        }
    });
    let put_in = rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap_or(false);
    if let Some(sh) = super::SHARED.get() {
        sh.log(&format!("desk: Ctrl watched for widgets picked on the desktop: hook {}", if put_in { "in" } else { "could not be put in" }));
    }
}

fn keys_stop() {
    const WM_QUIT: u32 = 0x12;
    let id = KEYS_THREAD.swap(0, std::sync::atomic::Ordering::SeqCst);
    if id != 0 {
        // SAFETY: a posted message, no pointers.
        unsafe { PostThreadMessageW(id, WM_QUIT, 0, 0) };
    }
}

// The last desktop window gone: its thread, hooks and timers with it.
pub fn stop() {
    let hwnd = WINDOW.load(std::sync::atomic::Ordering::SeqCst);
    if hwnd != 0 {
        // SAFETY: our own window; the message carries nothing.
        unsafe { PostMessageW(hwnd as Hwnd, WM_APP_STOP, 0, 0) };
    }
}

// A desktop now waits to be told: its ten seconds' look armed.
pub fn arm_stale() {
    let hwnd = WINDOW.load(std::sync::atomic::Ordering::SeqCst);
    if hwnd != 0 {
        // SAFETY: our own window; the message carries nothing.
        unsafe { PostMessageW(hwnd as Hwnd, WM_APP_STALE, 0, 0) };
    }
}

// Debug only (WAKUWAKU_DESK_TRACE=1): every message the main thread
// takes, counted by kind (desk/ minute): what wakes it.
#[repr(C)]
struct CwpStruct {
    lparam: isize,
    wparam: usize,
    message: u32,
    hwnd: Hwnd,
}
extern "system" fn on_get(code: i32, wparam: usize, lparam: isize) -> isize {
    if code >= 0 && lparam != 0 {
        // SAFETY: the system's MSG for this hook, alive for the call.
        let m = unsafe { &*(lparam as *const Msg) };
        super::traced(m.hwnd as isize, m.message, false);
    }
    // SAFETY: the next hook, as the system asks.
    unsafe { CallNextHookEx(0, code, wparam, lparam) }
}
extern "system" fn on_send(code: i32, wparam: usize, lparam: isize) -> isize {
    if code >= 0 && lparam != 0 {
        // SAFETY: the system's CWPSTRUCT for this hook, alive for the call.
        let m = unsafe { &*(lparam as *const CwpStruct) };
        super::traced(m.hwnd as isize, m.message, true);
    }
    // SAFETY: the next hook, as the system asks.
    unsafe { CallNextHookEx(0, code, wparam, lparam) }
}
pub fn trace_this_thread() {
    const WH_GETMESSAGE: i32 = 3;
    const WH_CALLWNDPROC: i32 = 4;
    // SAFETY: hooks on this thread alone, our own procedures.
    unsafe {
        let me = GetCurrentThreadId();
        SetWindowsHookExW(WH_GETMESSAGE, on_get, 0, me);
        SetWindowsHookExW(WH_CALLWNDPROC, on_send, 0, me);
    }
}

// The desktop's icons on a display: each its grid's cell where it is
// (icons.rs), in the display's window's px.
pub fn icons(mon: super::Mon, sf: f64) -> Option<Vec<[f64; 4]>> {
    crate::icons::with_com(|| {
        let d = crate::icons::desktop()?;
        // As its list draws each (its picture and its label, both lines):
        // else where the view puts it, a place of the grid's spacing.
        let drawn: Vec<(i32, i32, i32, i32)> = d.bounds().iter().map(|r| (r.left, r.top, r.right - r.left, r.bottom - r.top)).collect();
        let rects = if drawn.is_empty() {
            let s = d.spacing();
            d.items().iter().map(|i| (i.at.x, i.at.y, s.x, s.y)).collect()
        } else {
            drawn
        };
        Some(super::blocks_on(&rects, mon, sf))
    })
}
