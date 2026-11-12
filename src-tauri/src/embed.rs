// Her island inside Windows' own taskbar (zhoupengjie/Wakuwaku#105; the
// user, 2026-11-07: 做嵌入模式, as TrafficMonitor does it, at both ends, and
// one window, not two). Her island set to the taskbar (islandEdge taskbar)
// along the bottom or the top of her display: the island's own window
// (island.rs) is made a child of Explorer's taskbar window (Shell_TrayWnd,
// or a second display's Shell_SecondaryTrayWnd) with SetParent, as
// TrafficMonitor's is, while nothing is open in it. A child is clipped to
// its parent, so of the window (760 x 688, the closed island at its foot,
// island.rs ISLAND_BOTTOM) only the closed capsule shows, in the strip, on
// the taskbar's own ground, in the free stretch at one end of it: at the
// right, just left of the tray's icons (TrafficMonitor's place); at the
// left, at the strip's start (left of Start, where Start is there). As the
// pointer comes onto the capsule, or anything opens in it (a prompt, the
// settings, pulling her out), the window is taken out again (SetParent to
// none, kept on top, where it was), whole over the taskbar, and the island
// opens from the capsule as ever; closed again and the pointer gone, it goes
// back in. One window, one capsule: nothing to line up, nothing drawn twice
// (two windows, the capsule's own and the island's over it, showed two
// capsules at the island's corners and as it closed: the user, the same day).
//
// What examples/embed_spike.rs found (docs/DEVELOPMENT.md, 嵌进 Windows 的任务栏):
// the window keeps its own style (not made WS_CHILD), is made WS_EX_LAYERED
// (wholly opaque) before it goes in, else neither it nor a plain window of
// ours gets the pointer in Windows 11's taskbar (its XAML takes it); made
// layered it is a layer of DWM's own over the XAML, drawn as ever (WebView2
// and all), and hovered and pressed as ever. Nothing of Explorer's is
// changed, and no code goes into its process.
//
// The taskbar's buttons cannot be made to make room: Windows 11's taskbar is
// one XAML tree in one child window (the content, Start, the programs'
// buttons centred, the tray and the clock at its end, the strip's ground),
// which can only be narrowed or moved whole (tried, 2026-11-07: narrowed,
// the tray goes along to its new end and the stretch left is bare desktop),
// and TrafficMonitor on Windows 11 makes none either (its source read). So
// the capsule goes where there is room, and narrows, or shows her portrait
// alone, or hides, as the room goes (anchor.ts capsuleWidth): UI Automation
// reads every button on the taskbar (Taskbar.TaskListButtonAutomationPeer
// and the rest: Start, search, the widgets, task view), the tray
// (TrayNotifyWnd) is a window of its own, and the stretches left between
// them are the gaps (gaps), the first for the left, the last (before the
// tray) for the right (pick).
//
// Only while Explorer's taskbar is shown along the bottom or the top of her
// display, the work area stopping at it (shell.rs shown_bar): along a side,
// hiding itself, or put away, the window is taken out and the island lies
// over the work area's edge as it does with the embedding turned off
// (island.rs fit). Looked at every second (the programs' buttons come and
// go), and at once as the settings or the capsule's size change (look_soon).
// Explorer started again takes its taskbar's children with it, hers among
// them: the island's window gone, she starts herself again (updates.rs
// start_again, as after WebView2's browser going), a couple of seconds.
use crate::island::Side;

// Physical px: x and w along the strip.
pub type Span = (i32, i32);
// Physical px: x, y, w, h.
pub type Rect = (i32, i32, i32, i32);

// The capsule's room off the stretch's end (logical px): the flow light's
// ring runs 3 px outside it (style.css #flow), and the island keeps 8 px
// off its window's edge (island.ts TOP, midLeft).
pub const MARGIN: f64 = 8.0;
// The closed capsule before its page has said its size (island.ts WIDTHS normal, 36 high).
pub const CAPSULE: (f64, f64) = (300.0, 36.0);
// The narrowest the page lays the closed capsule (island.ts WIDTHS narrow),
// and its round capsule with her portrait alone (island.ts TIGHT_W), logical.
pub const NARROW: f64 = 240.0;
pub const TIGHT: f64 = 36.0;

// The free stretches of a bar (x, w) once what is on it (taken: x, w each,
// in any order, overlapping or not) is left out, left to right; none
// shorter than `least`, and nothing outside the bar.
pub fn gaps(bar: Span, taken: &[Span], least: i32) -> Vec<Span> {
    let (left, right) = (bar.0, bar.0 + bar.1);
    let mut used: Vec<(i32, i32)> = taken.iter().filter(|t| t.1 > 0).map(|t| (t.0.max(left), (t.0 + t.1).min(right))).filter(|(a, b)| b > a).collect();
    used.sort_unstable();
    let mut out = Vec::new();
    let mut at = left;
    for (a, b) in used {
        if a - at >= least.max(1) {
            out.push((at, a - at));
        }
        at = at.max(b);
    }
    if right - at >= least.max(1) {
        out.push((at, right - at));
    }
    out
}

// The stretch her capsule goes in, by where it is set to sit (islandPos):
// at the left, the first (the strip's left end, or past Start where Start
// is at the left); at the right, the last (just before the tray, as
// TrafficMonitor's place is); the middle (kept from the top of the screen)
// is the right. None with no gap at all.
pub fn pick(gaps: &[Span], side: Side) -> Option<Span> {
    match side {
        Side::Left => gaps.first().copied(),
        Side::Right | Side::Center => gaps.last().copied(),
    }
}

// Where a capsule `width` wide lies in a stretch, by the side: at its start
// (the left), or at its end, against the tray (the right), `margin` off the
// stretch's end.
pub fn place_in(gap: Span, side: Side, width: i32, margin: i32) -> i32 {
    match side {
        Side::Left => gap.0 + margin,
        Side::Right | Side::Center => gap.0 + gap.1 - margin - width,
    }
}

// Whether a capsule `width` wide fits a stretch with its margins.
pub fn fits(gap: Span, width: i32, margin: i32) -> bool {
    width > 0 && width + 2 * margin <= gap.1
}

// The capsule's width in a stretch as its page will lay it there, told the
// room (anchor.ts capsuleWidth): as wide as it is now while that fits; else
// the narrowest (`narrow`, never wider than it is); else her portrait alone
// (`tight`); none with not even that fitting. Laid at that width at once, so
// a capsule set to a stretch too short for it as it is (the right, just
// left of the tray, 363 px here for a capsule 375 wide) is not taken for one
// that does not fit at all, the island's window put in the screen's corner
// over the tray meanwhile, till its page had narrowed it (the user,
// 2026-11-08: 点右侧，第一次，直接会在任务栏的最右边，而不是在托盘图标的左边).
pub fn width_in(gap: Span, width: i32, narrow: i32, tight: i32, margin: i32) -> Option<i32> {
    [width, narrow.min(width), tight].into_iter().find(|&w| fits(gap, w, margin))
}

// Where two rects (x, y, w, h) meet; none where they do not.
pub fn overlap(a: Rect, b: Rect) -> Option<Rect> {
    let (left, top) = (a.0.max(b.0), a.1.max(b.1));
    let (right, bottom) = ((a.0 + a.2).min(b.0 + b.2), (a.1 + a.3).min(b.1 + b.3));
    (right > left && bottom > top).then_some((left, top, right - left, bottom - top))
}

// Her capsule as it lies now (physical), for the island: the capsule's rect
// (none: no room for it, or not in the taskbar), the room the page is told
// (physical px, 0 none) and whether the island is in the taskbar.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Laid {
    pub cap: Option<Rect>,
    pub room: i32,
    pub embedded: bool,
}

#[cfg(windows)]
mod imp {
    use crate::lock::Lock;
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
    use std::sync::{Arc, Condvar};
    use std::time::{Duration, Instant};

    use serde_json::{json, Value};

    use super::{gaps, overlap, pick, place_in, width_in, Laid, Rect, Side, Span, CAPSULE, MARGIN, NARROW, TIGHT};
    use crate::appbar::Edge;
    use crate::{island, shell, Shared};

    type Hwnd = *mut c_void;

    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct WinRect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetWindowRect(hwnd: Hwnd, rect: *mut WinRect) -> i32;
        fn IsWindow(hwnd: Hwnd) -> i32;
        fn IsWindowVisible(hwnd: Hwnd) -> i32;
        fn IsChild(parent: Hwnd, hwnd: Hwnd) -> i32;
        fn SetParent(child: Hwnd, parent: Hwnd) -> Hwnd;
        fn GetWindowLongPtrW(hwnd: Hwnd, index: i32) -> isize;
        fn SetWindowLongPtrW(hwnd: Hwnd, index: i32, value: isize) -> isize;
        fn SetWindowPos(hwnd: Hwnd, after: isize, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
        fn ScreenToClient(hwnd: Hwnd, pt: *mut Point) -> i32;
        fn GetDpiForWindow(hwnd: Hwnd) -> u32;
        fn SetLayeredWindowAttributes(hwnd: Hwnd, key: u32, alpha: u8, flags: u32) -> i32;
        fn FindWindowExW(parent: Hwnd, after: Hwnd, class: *const u16, title: *const u16) -> Hwnd;
        fn GetWindow(hwnd: Hwnd, cmd: u32) -> Hwnd;
        fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
        fn GetAncestor(hwnd: Hwnd, flags: u32) -> Hwnd;
        fn GetDesktopWindow() -> Hwnd;
        fn IsWindowEnabled(hwnd: Hwnd) -> i32;
        fn GetForegroundWindow() -> Hwnd;
        fn GetClassNameW(hwnd: Hwnd, name: *mut u16, max: i32) -> i32;
        fn EnumThreadWindows(thread: u32, each: unsafe extern "system" fn(Hwnd, isize) -> i32, param: isize) -> i32;
        // GDI's handles as isize, the module's too, as the other modules declare them.
        fn GetDC(hwnd: Hwnd) -> isize;
        fn ReleaseDC(hwnd: Hwnd, dc: isize) -> i32;
        fn UpdateLayeredWindow(hwnd: Hwnd, dst: isize, at: *const Point, size: *const Point, src: isize, from: *const Point, key: u32, blend: *const Blend, flags: u32) -> i32;
        fn RegisterClassW(class: *const WndClass) -> u16;
        fn CreateWindowExW(ex: u32, class: *const u16, title: *const u16, style: u32, x: i32, y: i32, w: i32, h: i32, parent: Hwnd, menu: isize, instance: isize, param: *mut c_void) -> Hwnd;
        fn DefWindowProcW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize;
        fn GetCursorPos(pt: *mut Point) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetLastError() -> u32;
        fn GetCurrentThreadId() -> u32;
        fn GetModuleHandleW(name: *const u16) -> isize;
    }

    #[link(name = "gdi32")]
    extern "system" {
        fn CreateCompatibleDC(hdc: isize) -> isize;
        fn CreateDIBSection(hdc: isize, info: *const BitmapInfoHeader, usage: u32, bits: *mut *mut u8, section: isize, offset: u32) -> isize;
        fn SelectObject(hdc: isize, obj: isize) -> isize;
        fn BitBlt(dst: isize, x: i32, y: i32, w: i32, h: i32, src: isize, sx: i32, sy: i32, rop: u32) -> i32;
        fn GdiFlush() -> i32;
        fn DeleteObject(obj: isize) -> i32;
        fn DeleteDC(hdc: isize) -> i32;
    }

    // WNDCLASSW.
    #[repr(C)]
    struct WndClass {
        style: u32,
        proc_: unsafe extern "system" fn(Hwnd, u32, usize, isize) -> isize,
        cls_extra: i32,
        wnd_extra: i32,
        instance: isize,
        icon: isize,
        cursor: isize,
        background: isize,
        menu: *const u16,
        name: *const u16,
    }

    // BITMAPINFOHEADER (BITMAPINFO's colours unused at 32 bits), as taskbar/ has it.
    #[repr(C)]
    struct BitmapInfoHeader {
        size: u32,
        width: i32,
        height: i32,
        planes: u16,
        bit_count: u16,
        compression: u32,
        size_image: u32,
        x_ppm: i32,
        y_ppm: i32,
        clr_used: u32,
        clr_important: u32,
    }

    // BLENDFUNCTION.
    #[repr(C)]
    struct Blend {
        op: u8,
        flags: u8,
        alpha: u8,
        format: u8,
    }

    #[link(name = "dwmapi")]
    extern "system" {
        // Waits for DWM's next frame to be put on the screen.
        fn DwmFlush() -> i32;
    }

    const GWL_EXSTYLE: i32 = -20;
    const GWL_STYLE: i32 = -16;
    // A top-level window's owner (set this way, as WPF sets a window's Owner).
    const GWLP_HWNDPARENT: i32 = -8;
    // The first of a window's children (the top of their order), and the next.
    const GW_CHILD: u32 = 5;
    const GW_HWNDNEXT: u32 = 2;
    const GW_OWNER: u32 = 4;
    const GA_PARENT: u32 = 1;
    const WS_EX_LAYERED: isize = 0x0008_0000;
    const LWA_ALPHA: u32 = 0x2;
    const HWND_TOP: isize = 0;
    const HWND_TOPMOST: isize = -1;
    const SWP_NOSIZE: u32 = 0x1;
    const SWP_NOMOVE: u32 = 0x2;
    const SWP_NOZORDER: u32 = 0x4;
    const SWP_NOACTIVATE: u32 = 0x10;
    const SWP_FRAMECHANGED: u32 = 0x20;
    const SWP_SHOWWINDOW: u32 = 0x40;
    const SWP_HIDEWINDOW: u32 = 0x80;
    const WS_POPUP: u32 = 0x8000_0000;
    // The still's: layered (drawn by UpdateLayeredWindow), the pointer
    // passing through it, on top, never on the taskbar or Alt+Tab, never active.
    const STILL_EX: u32 = 0x0008_0000 | 0x20 | 0x8 | 0x80 | 0x0800_0000;
    const SRCCOPY: u32 = 0x00CC_0020;
    // Layered windows in the copy too (DWM's whole picture).
    const CAPTUREBLT: u32 = 0x4000_0000;
    const ULW_ALPHA: u32 = 0x2;
    // How near the capsule the pointer still counts as on it (physical px; apply).
    const POINTER_SLACK: i32 = 4;
    // How often the taskbar is looked at while the island is wanted in it.
    const EVERY: Duration = Duration::from_secs(1);

    #[derive(Default)]
    struct Embed {
        // The island's window (island.rs hwnd) as last seen here, and the
        // taskbar window it is in now (0: none, out on its own).
        hwnd: isize,
        tray: isize,
        parented: bool,
        // Out of the taskbar and owned by it (release), kept above it by Windows.
        owned: bool,
        // Where it was laid in the taskbar last (physical), to lay again only on a change.
        child: Option<Rect>,
        // Made layered (once; frameless.rs keeps the bit after).
        layered: bool,
        // What the island was last told, and what for: the side it was set
        // to and the closed capsule's size (physical) then (look's held).
        laid: Laid,
        basis: Option<(Side, i32, i32)>,
        // The stretches seen last, and the one picked, for the log.
        said: String,
        // The window gone with Explorer's taskbar: her restart under way.
        restarting: bool,
    }

    static EMBED: Lock<Embed> = Lock::new("embed::EMBED", Embed { hwnd: 0, tray: 0, parented: false, owned: false, child: None, layered: false, laid: Laid { cap: None, room: 0, embedded: false }, basis: None, said: String::new(), restarting: false });
    static WAKE: (Lock<bool>, Condvar) = (Lock::new("embed::WAKE", false), Condvar::new());
    // One apply at a time (the pointer's thread and the page's command both
    // call it): two at once both saw the window out and both put it in. Only
    // ever tried (try_lock), never waited for, and held around the apply,
    // which takes the island's lock and EMBED: an outer lock (lock.rs).
    static APPLYING: Lock<()> = Lock::ranked("embed::APPLYING", crate::lock::OUTER, ());
    static STARTED: AtomicBool = AtomicBool::new(false);
    // Moves into the taskbar or out of it failing in a row (tried again at
    // every look at the pointer): since when, how many, and when last told.
    // The first is logged with what was seen of the window, then one line
    // every half minute while it goes on, and how long it took once one
    // works: logged once till one worked, SetParent refused the island's
    // window with error 87 for four minutes and the log said nothing more
    // (2026-11-08, the island out of the taskbar all that while).
    struct Fails {
        since: Instant,
        tries: u32,
        told: Option<Instant>,
    }
    static FAILS: Lock<Option<Fails>> = Lock::new("embed::FAILS", None);
    const FAILS_TOLD_EVERY: Duration = Duration::from_secs(30);

    // A move that failed: `line` says what (made only when it is logged).
    fn failed(sh: &Shared, line: impl FnOnce() -> String) {
        let now = Instant::now();
        let (tries, secs, again) = {
            let mut fails = FAILS.lock().unwrap();
            let f = fails.get_or_insert(Fails { since: now, tries: 0, told: None });
            f.tries += 1;
            if f.told.is_some_and(|t| now - t < FAILS_TOLD_EVERY) {
                return;
            }
            (f.tries, (now - f.since).as_secs(), f.told.replace(now).is_some())
        };
        let line = line();
        if again {
            sh.log(&format!("island: {line}; still, {tries} tries in {secs} s"));
        } else {
            sh.log(&format!("island: {line}; trying again"));
        }
    }

    // A move that worked: how the ones before it went, for its line (empty with none failed).
    fn worked() -> String {
        match FAILS.lock().unwrap().take() {
            Some(f) => format!(", after {} failed tries in {:.1} s", f.tries, f.since.elapsed().as_secs_f64()),
            None => String::new(),
        }
    }

    fn class_of(hwnd: Hwnd) -> String {
        let mut buf = [0u16; 128];
        // SAFETY: our own buffer, its length given; a stale handle answers nothing.
        let n = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }

    // What was seen of the island's window as SetParent refused it, for the
    // log: its style, its owner, whether it is enabled and in front, and
    // the windows it owns (the input method's, a menu's).
    fn seen(hwnd: Hwnd) -> String {
        unsafe extern "system" fn owned_by(h: Hwnd, out: isize) -> i32 {
            // SAFETY: `out` is the pair seen() hands EnumThreadWindows, alive through the call.
            let (owner, classes) = unsafe { &mut *(out as *mut (Hwnd, Vec<String>)) };
            // SAFETY: a plain query on a window the enumeration gave.
            if unsafe { GetWindow(h, GW_OWNER) } == *owner {
                classes.push(class_of(h));
            }
            1
        }
        // SAFETY: plain queries on our own window (a stale handle answers
        // nothing); the enumeration's pair lives on this stack through it.
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let owner = GetWindow(hwnd, GW_OWNER);
            let enabled = IsWindowEnabled(hwnd) != 0;
            let fg = GetForegroundWindow();
            let front = if fg == hwnd { "ours".to_string() } else { format!("‹{}›", class_of(fg)) };
            let thread = GetWindowThreadProcessId(hwnd, std::ptr::null_mut());
            let mut found: (Hwnd, Vec<String>) = (hwnd, Vec::new());
            EnumThreadWindows(thread, owned_by, &mut found as *mut (Hwnd, Vec<String>) as isize);
            format!("style {style:#x}, ex {ex:#x}, owner {owner:?}, enabled {enabled}, in front {front}, owning {:?}", found.1)
        }
    }
    static QUITTING: AtomicBool = AtomicBool::new(false);

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn rect_of(hwnd: Hwnd) -> Option<Rect> {
        let mut r = WinRect::default();
        // SAFETY: our own struct, of the size the call writes.
        let ok = unsafe { GetWindowRect(hwnd, &mut r) } != 0;
        (ok && r.right > r.left && r.bottom > r.top).then(|| (r.left, r.top, r.right - r.left, r.bottom - r.top))
    }

    // The thread that looks at the taskbar: every second while her home is
    // the island in Windows' taskbar; at once when asked (look_soon). Started once.
    pub fn start(sh: &Arc<Shared>) {
        if STARTED.swap(true, Ordering::SeqCst) {
            return;
        }
        let sh = sh.clone();
        crate::spawn_named("embed", move || {
            crate::tasks::with_com();
            loop {
                {
                    let (woken, cv) = &WAKE;
                    let guard = woken.lock().unwrap();
                    let (mut guard, _) = guard.wait_timeout_while(cv, EVERY, |w| !*w).unwrap();
                    *guard = false;
                }
                if QUITTING.load(Ordering::SeqCst) {
                    return;
                }
                look(&sh);
            }
        });
    }

    // Looked at now, not at the next second (the settings changed, the
    // capsule's size, her home's display).
    pub fn look_soon() {
        let (woken, cv) = &WAKE;
        *woken.lock().unwrap() = true;
        cv.notify_all();
    }

    // Quitting: the island's window out of Explorer's taskbar before it goes
    // with her (a child of another process's window left behind).
    pub fn stop() {
        QUITTING.store(true, Ordering::SeqCst);
        look_soon();
        let mut e = EMBED.lock().unwrap();
        if e.hwnd != 0 && (e.parented || e.owned) {
            // SAFETY: plain calls on our own window; a stale handle only makes them fail.
            unsafe {
                if e.parented {
                    SetParent(e.hwnd as Hwnd, std::ptr::null_mut());
                }
                SetWindowLongPtrW(e.hwnd as Hwnd, GWLP_HWNDPARENT, 0);
            }
            e.parented = false;
            e.owned = false;
        }
        e.tray = 0;
    }

    // Whether the island's window is a child of the taskbar now: laid by
    // lay_child then (island.rs lay), and never raised (island.rs raise, uncover).
    pub fn is_parented() -> bool {
        EMBED.lock().unwrap().parented
    }

    // Whether a window of hers still exists: Explorer's taskbar destroyed
    // (Explorer killed) takes its children with it, the island's window
    // among them, and tao's set_size on a window that is no more panics
    // (adjust_window_rect failed; the release build aborts on a panic:
    // seen in the test VM, 2026-11-07, she died with Explorer).
    pub fn is_window(hwnd: isize) -> bool {
        // SAFETY: a plain query; a stale handle answers no.
        hwnd != 0 && unsafe { IsWindow(hwnd as Hwnd) } != 0
    }

    // The island's window laid in the taskbar (its rect physical, on the
    // screen): in the taskbar's client coordinates, first among its children.
    // Laid again whenever it is not where it is meant to be, not only when
    // that changes: tao's set_position is posted to the window's thread
    // (SWP_ASYNCWINDOWPOS) and, asked for just before the window went in, it
    // came after, its place on the screen taken as one in the taskbar's
    // client area (1380 px down here, off the strip, the capsule gone).
    pub fn lay_child((x, y): (i32, i32), (w, h): (i32, i32)) {
        let (hwnd, tray) = {
            let mut e = EMBED.lock().unwrap();
            if !e.parented || e.hwnd == 0 || e.tray == 0 {
                return;
            }
            if rect_of(e.hwnd as Hwnd) == Some((x, y, w, h)) {
                return;
            }
            e.child = Some((x, y, w, h));
            (e.hwnd as Hwnd, e.tray as Hwnd)
        };
        put_child(hwnd, tray, (x, y), (w, h), false);
        settle_webview(hwnd);
    }

    // The window, a child of the taskbar's window, at its rect on the screen
    // (physical) in the taskbar's client coordinates, first among its
    // children. `quick` (go_in, the window just put in, the capsule unseen
    // till this is done): only moved, not sized where its size is already
    // that, and no frame recalculated: sizing it has wry size WebView2's
    // windows, in their own process, and the frame recalculated with it
    // took 2 to 4 ms more of the capsule's going.
    fn put_child(hwnd: Hwnd, tray: Hwnd, (x, y): (i32, i32), (w, h): (i32, i32), quick: bool) {
        let mut pt = Point { x, y };
        let sized = rect_of(hwnd).is_some_and(|r| (r.2, r.3) == (w, h));
        let flags = SWP_NOACTIVATE | if quick { if sized { SWP_NOSIZE } else { 0 } } else { SWP_FRAMECHANGED };
        // SAFETY: plain calls on our own window and its parent.
        unsafe {
            ScreenToClient(tray, &mut pt);
            SetWindowPos(hwnd, HWND_TOP, pt.x, pt.y, w, h, flags);
        }
    }

    // WebView2's window inside ours (wry's WRY_WEBVIEW, the browser's own
    // in it) back over the whole of ours: as the window changes parent it is
    // left where the parent's client area would have put it (seen
    // 2026-11-07: our window a child of the taskbar, at (0, 580) on the
    // screen, laid at (0, -800) in the taskbar's client area; wry's at (0,
    // -800) on the screen, 1380 px up, and nothing drawn in the strip).
    // Moves and sizes after that keep it in place; this puts it right once.
    fn settle_webview(hwnd: Hwnd) {
        // SAFETY: plain calls on our own window and its child.
        unsafe {
            let child = GetWindow(hwnd, GW_CHILD);
            if child.is_null() {
                return;
            }
            let (Some(ours), Some(theirs)) = (rect_of(hwnd), rect_of(child)) else { return };
            if ours != theirs {
                SetWindowPos(child, HWND_TOP, 0, 0, ours.2, ours.3, SWP_NOZORDER | SWP_NOACTIVATE);
            }
        }
    }

    // Where it is, for /health.
    pub fn where_() -> Value {
        let e = EMBED.lock().unwrap();
        json!({
            "hwnd": e.hwnd,
            "tray": e.tray,
            "parented": e.parented,
            "owned": e.owned,
            "child": e.child.map(|r| [r.0, r.1, r.2, r.3]),
            "cap": e.laid.cap.map(|r| [r.0, r.1, r.2, r.3]),
            "room": e.laid.room,
            "embedded": e.laid.embedded,
            "gaps": e.said,
        })
    }

    // The island's window into the taskbar, or out of it, as it is used
    // (island.rs parenting): in while the island is closed and nobody is
    // on it, out as the pointer comes or anything opens. Cheap when
    // nothing changes; called from the island's every look at the pointer.
    pub fn apply(sh: &Shared) {
        let Ok(_one) = APPLYING.try_lock() else { return };
        let Some((hwnd, pos, size, shown, want_child)) = island::parenting(sh) else { return };
        let (tray, parented, layered, cap) = {
            let e = EMBED.lock().unwrap();
            (e.tray, e.parented, e.layered, e.laid.cap)
        };
        // Out of the taskbar, never put back while the pointer is on the
        // capsule, whatever the page says. Each move in or out leaves the
        // window from under the pointer for a moment (laid off the strip as
        // a child, or a window of its own still under the taskbar), and
        // WebView2 gives the page a mouseleave then (seen at y -439, the
        // window laid off the strip), and a mouseenter once it is back: told
        // the pointer had gone the window went in, told it was back it came
        // out, 30 times a second with the pointer resting on it, the cursor
        // flickering and the island never opening (the user, 2026-11-08:
        // 光标会一直在忙和闲来回闪，且灵动岛展不开). The pointer gone off it,
        // the next look at the pointer (50 ms on) puts it in.
        let want_child = want_child && !(!parented && cap.is_some_and(pointer_on));
        if parented && !is_window(hwnd) {
            // Gone with Explorer's taskbar: looked at now (look), which starts her anew.
            look_soon();
            return;
        }
        if want_child && tray != 0 && !QUITTING.load(Ordering::SeqCst) {
            if parented {
                // Explorer's own children made after ours (its XAML, as it
                // lays itself out anew) lie over it: first among them again.
                // SAFETY: plain calls on windows that exist.
                unsafe {
                    if IsWindow(tray as Hwnd) != 0 && GetWindow(tray as Hwnd, GW_CHILD) != hwnd as Hwnd {
                        SetWindowPos(hwnd as Hwnd, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                    }
                }
                // Where it is meant to be, should a late move have put it elsewhere (lay_child).
                lay_child(pos, size);
                settle_webview(hwnd as Hwnd);
                return;
            }
            let (ok, said) = go_in(hwnd as Hwnd, tray as Hwnd, layered, pos, size, shown);
            {
                let mut e = EMBED.lock().unwrap();
                e.hwnd = hwnd;
                e.layered = true;
                e.child = None;
                if ok {
                    e.parented = true;
                    e.owned = false;
                    e.child = Some((pos.0, pos.1, size.0, size.1));
                }
            }
            if !ok {
                // Not taken in: tried again at the next look at the pointer.
                failed(sh, || format!("could not go into Explorer's taskbar {tray:#x} ({said}; {})", seen(hwnd as Hwnd)));
                return;
            }
            let after = worked();
            sh.log(&format!("island: into Explorer's taskbar {tray:#x}, the capsule in the strip ({said}){after}"));
            // A change to the capsule's place held while it was in use (look): laid now.
            look_soon();
        } else if parented {
            let (ok, said) = release(hwnd as Hwnd, pos, size, shown, tray as Hwnd);
            if !ok {
                // Still the taskbar's child, clipped to its strip: the island
                // opening in it showed only the strip's height of it (the
                // user, 2026-11-07: 展开会被任务栏自己截断; seen once in the test
                // VM, SetParent having left it where it was). Left counted as
                // in, so the next look at the pointer (50 ms on) tries again.
                failed(sh, || format!("could not come out of the taskbar ({said}; {})", seen(hwnd as Hwnd)));
                return;
            }
            let after = worked();
            {
                let mut e = EMBED.lock().unwrap();
                e.parented = false;
                e.owned = tray != 0;
                e.child = None;
            }
            sh.log(&format!("island: out of the taskbar, whole over it, owned by it ({said}){after}"));
        }
    }

    // The pointer on a rect (physical), or within a few px of it.
    fn pointer_on(r: Rect) -> bool {
        let mut at = Point::default();
        // SAFETY: our own struct, of the size the call writes.
        unsafe { GetCursorPos(&mut at) != 0 && crate::pointer::within((at.x, at.y), (r.0, r.1), (r.2, r.3), POINTER_SLACK) }
    }

    // One look: Explorer's taskbar along the bottom or the top of her
    // display, what is on it, the stretch for the capsule, and the island
    // told where its capsule lies (set_capsule lays its window about it);
    // then the window into the taskbar or out of it (apply).
    fn look(sh: &Arc<Shared>) {
        // First of all: the window in the taskbar gone with it (Explorer
        // killed takes its taskbar's children: the island's window, her
        // home, with the settings and all). Tauri keeps its label, so she
        // starts herself anew (as after WebView2's browser going,
        // crash.rs), once; nothing else is done to the window that is no
        // more (tao's set_size on it panics, and the release build aborts;
        // the test VM, 2026-11-07: she died with Explorer).
        let dead = {
            let e = EMBED.lock().unwrap();
            e.hwnd != 0 && (e.parented || e.owned || e.restarting) && !is_window(e.hwnd)
        };
        if dead {
            let first = {
                let mut e = EMBED.lock().unwrap();
                e.parented = false;
                e.owned = false;
                e.tray = 0;
                e.child = None;
                !std::mem::replace(&mut e.restarting, true)
            };
            if first {
                sh.log("island: its window is gone with Explorer's taskbar (Explorer started again?): she starts anew");
                let sh = sh.clone();
                std::thread::spawn(move || match crate::updates::start_again(&sh, "the island's window gone with Explorer's taskbar") {
                    Ok(()) => sh.quit(crate::data::Quit::Wakeable, "her restart, the island's window gone with Explorer's taskbar"),
                    Err(e) => sh.log(&format!("island: could not start anew: {e}")),
                });
            }
            return;
        }
        let wanted = island::embedding_wanted(sh);
        let over = !wanted && island::over_wanted(sh);
        let shown = (wanted || over).then(|| island::home_area(sh)).flatten().and_then(|a| shell::shown_bar(a.mon)).filter(|(_, _, edge)| matches!(edge, Edge::Top | Edge::Bottom));
        let Some((tray, strip, edge)) = shown.filter(|_| wanted) else {
            // Laid over the taskbar (the embedding off): owned by it (detach).
            let owner = shown.filter(|_| over).map_or(0, |b| b.0);
            return detach(sh, if wanted { "no taskbar of Explorer's shown along the bottom or the top of her display" } else { "not wanted" }, owner);
        };
        let hwnd = island::hwnd(sh);
        if !is_window(hwnd) {
            return;
        }
        // SAFETY: a plain query about a window that exists.
        let sf = unsafe { GetDpiForWindow(tray as Hwnd) } as f64 / 96.0;
        let margin = (MARGIN * sf).round() as i32;
        let side = island::side_wanted(sh);
        // What is on the taskbar, and the stretches between.
        let taken = taken_on(tray as Hwnd, hwnd as Hwnd);
        let free = gaps((strip.0, strip.2), &taken, 2 * margin + 8);
        let gap = pick(&free, side);
        let room = gap.map_or(0, |g| (g.1 - 2 * margin).max(0));
        // The closed capsule as its page has it (logical, at its scale).
        let (cw, ch, page_sf) = island::closed_size(sh).unwrap_or((CAPSULE.0, CAPSULE.1, sf));
        let (w, h) = ((cw * page_sf).round() as i32, (ch * page_sf).round() as i32);
        let px = |v: f64| (v * page_sf).round() as i32;
        // At the width its page will lay it there (narrowed for a short stretch).
        let fit = gap.and_then(|g| width_in(g, w, px(NARROW), px(TIGHT), margin));
        let cap = gap.zip(fit).map(|(g, cw)| (place_in(g, side, cw, margin), strip.1 + (strip.3 - h) / 2, cw, h));
        let said = format!("{free:?} of {taken:?} on {strip:?} along the {edge:?}; {} {:?}", side.name(), gap);
        let laid = Laid { cap, room, embedded: true };
        // In use (the pointer on it, something open in it), its window out
        // of the taskbar: the capsule's place and room held as they were, so
        // the open island is never moved, narrowed or hidden under the
        // pointer (a program's button come or gone meanwhile); laid anew as
        // it goes back in (apply looks again then). Held only against the
        // taskbar's own changes, never against what the person or her page
        // asked for: the side set (in the settings, open in it: they are in
        // use), the island just put into the taskbar (nothing laid there
        // yet), or the closed capsule's size changed (its page narrowing it
        // to the room it was told) are laid at once (the user, 2026-11-08:
        // 在嵌入模式下，点击左侧或者右侧，不会立即生效; it moved only once
        // the settings were closed).
        let basis = (side, w, h);
        let held = island::in_use(sh) && {
            let e = EMBED.lock().unwrap();
            !e.parented && e.laid.embedded && e.basis == Some(basis)
        };
        let before = {
            let mut e = EMBED.lock().unwrap();
            e.hwnd = hwnd;
            let tray_was = std::mem::replace(&mut e.tray, tray);
            if tray_was != tray && tray_was != 0 && e.parented {
                // Another taskbar window (another display's): adopted anew by it (apply).
                e.parented = false;
                e.child = None;
            }
            if e.said != said {
                e.said = said.clone();
                let how = match fit {
                    _ if held => "held while it is in use".to_string(),
                    Some(cw) if cw == w => "fits".to_string(),
                    Some(cw) => format!("fits {cw} wide"),
                    None => "does not fit: the island hidden".to_string(),
                };
                sh.log(&format!("island: free {said}; the capsule {w}x{h} {how}"));
            }
            if held {
                e.laid
            } else {
                e.basis = Some(basis);
                std::mem::replace(&mut e.laid, laid)
            }
        };
        if !held && laid != before {
            island::set_capsule(sh, laid);
        }
        apply(sh);
    }

    // Out of the taskbar, and the island laid as it is with no capsule in
    // the taskbar (island.rs fit). Laid over the taskbar instead (the island
    // set to Windows' taskbar, the embedding off: island.rs over_wanted),
    // owned by the taskbar's window (`owner`; 0: nobody's), as it is out of
    // the taskbar with the embedding on (release): Windows keeps it over
    // the taskbar however Explorer raises it. Not owned, the taskbar pressed
    // came over it and hid it till island.rs uncover's next look found it
    // under it, up to 2 s (the user, 2026-11-08: 切换到叠在任务栏上后，点一下
    // 任务栏，灵动岛会被藏在后面几秒钟). An owner destroyed takes its owned
    // windows with it, so Explorer started again takes the island's window
    // as with the embedding (owned: look's first check, she starts anew).
    fn detach(sh: &Arc<Shared>, why: &str, owner: isize) {
        let parenting = island::parenting(sh);
        // Read before EMBED is taken: the island's lock is never taken
        // inside it (island.rs where_ takes EMBED's after its own).
        let island_hwnd = island::hwnd(sh);
        let (was, hwnd, parented, owned) = {
            let mut e = EMBED.lock().unwrap();
            let was = std::mem::take(&mut e.laid);
            e.basis = None;
            let parented = std::mem::take(&mut e.parented);
            let owned = std::mem::take(&mut e.owned);
            e.tray = 0;
            e.child = None;
            e.said.clear();
            if owner != 0 && e.hwnd == 0 {
                e.hwnd = island_hwnd;
            }
            (was, e.hwnd, parented, owned)
        };
        if hwnd != 0 && is_window(hwnd) {
            if parented {
                let (pos, size, shown) = parenting.map_or(((0, 0), (1, 1), false), |p| (p.1, p.2, p.3));
                let (ok, said) = release(hwnd as Hwnd, pos, size, shown, owner as Hwnd);
                if ok {
                    sh.log(&format!("island: out of the taskbar ({why})"));
                } else {
                    // Tried again at the next look.
                    EMBED.lock().unwrap().parented = true;
                    sh.log(&format!("island: could not come out of the taskbar ({why}; {said}); trying again"));
                }
            }
            // SAFETY: a plain query and a plain call on our own window.
            let now = unsafe { GetWindow(hwnd as Hwnd, GW_OWNER) } as isize;
            if owner != 0 {
                if now != owner {
                    unsafe { SetWindowLongPtrW(hwnd as Hwnd, GWLP_HWNDPARENT, owner) };
                    sh.log(&format!("island: over Windows' taskbar, owned by it ({owner:#x})"));
                }
                EMBED.lock().unwrap().owned = true;
            } else if owned && now != 0 {
                // A window of its own again, nobody's: the taskbar it was owned by is not hers to follow.
                unsafe { SetWindowLongPtrW(hwnd as Hwnd, GWLP_HWNDPARENT, 0) };
            }
        }
        if was != Laid::default() {
            island::set_capsule(sh, Laid::default());
        }
    }

    // Into the taskbar in one go, on the window's own thread, just after
    // DWM has put a frame on the screen (frameless.rs on_its_thread,
    // DwmFlush): adopted, laid in the strip, its webview put back over it,
    // shown. Done step by step from another thread, each step a wait for
    // the window's thread, DWM drew a frame between them with the window
    // under the taskbar (no longer on top) or off the strip (a child where
    // it stood on the screen, read in the taskbar's coordinates): the
    // capsule went for a frame as the island closed back into it (the
    // user, 2026-11-08: 展开后收缩到胶囊，胶囊会闪一下). In one go it still
    // took longer than a frame, so it is done under a still of the strip
    // (stilled). Nothing is locked in the job: whoever holds a lock may be
    // waiting for the window's thread.
    fn go_in(hwnd: Hwnd, tray: Hwnd, layered: bool, pos: (i32, i32), size: (i32, i32), shown: bool) -> (bool, String) {
        let (h, t) = (hwnd as isize, tray as isize);
        crate::frameless::on_its_thread(h, move || {
            let (hwnd, tray) = (h as Hwnd, t as Hwnd);
            let mut steps = Steps::new();
            let (ok, said) = stilled(hwnd, tray, &mut steps, |steps| {
                let (ok, said) = adopt(hwnd, tray, layered);
                steps.mark("in");
                if ok {
                    put_child(hwnd, tray, pos, size, true);
                    steps.mark("laid");
                    settle_webview(hwnd);
                    steps.mark("page");
                    // SAFETY: a plain query and a plain call on our own window.
                    if shown && unsafe { IsWindowVisible(hwnd) } == 0 {
                        unsafe { SetWindowPos(hwnd, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW) };
                        steps.mark("shown");
                    }
                }
                (ok, said)
            });
            (ok, format!("{said}; {}", steps.said(on_thread(hwnd))))
        })
    }

    // A still of the island's part of the strip, over it while its window
    // changes parent (go_in, release). SetParent alone takes 3 to 14 ms
    // here, and a frame of DWM's 4.2 (240 Hz); all through it the window is
    // in neither place: a child where it stood on the screen, read in the
    // taskbar's coordinates (off the strip), or a window of its own not yet
    // on top (under the taskbar). However quick the rest, DWM drew a frame
    // without the capsule every time it went in and every time it came out
    // (caught frame by frame with Desktop Duplication, 2026-11-08; the user:
    // 还是会轻微快速闪一下). So the strip is copied as DWM shows it (BitBlt
    // with CAPTUREBLT: the capsule, layered, in it) into a window of its
    // own, layered, kept on top, the pointer passing through it, shown
    // before the window moves and taken away once DWM has put its new place
    // on the screen: the capsule is seen all through. A picture only, up
    // for a couple of frames; nothing in it answers the pointer.
    static STILL: AtomicIsize = AtomicIsize::new(0);

    // The job (go_in's, release's) under the still (still_on), just after
    // DWM's frame (the still on the screen in it); the still taken away
    // after DWM's next. The job alone where no still was made (the window
    // not shown or not on the strip, not on its own thread, the copy failed).
    fn stilled(hwnd: Hwnd, strip: Hwnd, steps: &mut Steps, job: impl FnOnce(&mut Steps) -> (bool, String)) -> (bool, String) {
        // SAFETY: a plain query on our own window.
        let seen = !strip.is_null() && on_thread(hwnd) && unsafe { IsWindowVisible(hwnd) } != 0;
        let still = if seen { still_on(hwnd, strip) } else { None };
        steps.mark(if still.is_some() { "still" } else { "no still" });
        // SAFETY: a plain wait for DWM's next frame.
        unsafe { DwmFlush() };
        steps.mark("frame");
        let done = job(steps);
        if let Some(still) = still {
            // SAFETY: as above, then a plain call on our own window.
            unsafe {
                DwmFlush();
                SetWindowPos(still, 0, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_HIDEWINDOW);
            }
            steps.mark("seen");
        }
        done
    }

    // What of the island's window lies on the strip (`strip`, the taskbar's
    // window), copied off the screen, shown over it; the still's window.
    fn still_on(hwnd: Hwnd, strip: Hwnd) -> Option<Hwnd> {
        let (x, y, w, h) = overlap(rect_of(hwnd)?, rect_of(strip)?)?;
        let still = still_window()?;
        // SAFETY: the screen's DC and GDI objects of our own, each let go of
        // here; the DIB's bits (w x h, 32 bits, top-down) written only within
        // them, after GdiFlush; our own window laid and shown.
        unsafe {
            let screen = GetDC(std::ptr::null_mut());
            if screen == 0 {
                return None;
            }
            let mem = CreateCompatibleDC(screen);
            let info = BitmapInfoHeader { size: 40, width: w, height: -h, planes: 1, bit_count: 32, compression: 0, size_image: 0, x_ppm: 0, y_ppm: 0, clr_used: 0, clr_important: 0 };
            let mut bits: *mut u8 = std::ptr::null_mut();
            let dib = CreateDIBSection(screen, &info, 0, &mut bits, 0, 0);
            let mut ok = false;
            if mem != 0 && dib != 0 && !bits.is_null() {
                let old = SelectObject(mem, dib);
                if BitBlt(mem, 0, 0, w, h, screen, x, y, SRCCOPY | CAPTUREBLT) != 0 {
                    GdiFlush();
                    // Wholly opaque: it is what is on the screen, the taskbar's ground and all.
                    let px = std::slice::from_raw_parts_mut(bits, (w * h * 4) as usize);
                    px.iter_mut().skip(3).step_by(4).for_each(|a| *a = 255);
                    let blend = Blend { op: 0, flags: 0, alpha: 255, format: 1 };
                    ok = UpdateLayeredWindow(still, screen, &Point { x, y }, &Point { x: w, y: h }, mem, &Point::default(), 0, &blend, ULW_ALPHA) != 0
                        && SetWindowPos(still, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW) != 0;
                }
                SelectObject(mem, old);
            }
            if dib != 0 {
                DeleteObject(dib);
            }
            if mem != 0 {
                DeleteDC(mem);
            }
            ReleaseDC(std::ptr::null_mut(), screen);
            if !ok {
                SetWindowPos(still, 0, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_HIDEWINDOW);
            }
            ok.then_some(still)
        }
    }

    // The still's window, made on first use on this thread (the island's,
    // whose loop runs its messages); made again should it be gone.
    fn still_window() -> Option<Hwnd> {
        let have = STILL.load(Ordering::SeqCst) as Hwnd;
        // SAFETY: a plain query; a stale handle answers 0.
        if !have.is_null() && unsafe { IsWindow(have) } != 0 {
            return Some(have);
        }
        let name = wide("WakuwakuIslandStill");
        // SAFETY: a class of our own (registered once: again it fails, harmlessly),
        // its name alive through both calls; a window of that class, nobody's.
        let hwnd = unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let class = WndClass { style: 0, proc_: DefWindowProcW, cls_extra: 0, wnd_extra: 0, instance, icon: 0, cursor: 0, background: 0, menu: std::ptr::null(), name: name.as_ptr() };
            RegisterClassW(&class);
            CreateWindowExW(STILL_EX, name.as_ptr(), name.as_ptr(), WS_POPUP, 0, 0, 0, 0, std::ptr::null_mut(), 0, instance, std::ptr::null_mut())
        };
        if hwnd.is_null() {
            return None;
        }
        STILL.store(hwnd as isize, Ordering::SeqCst);
        Some(hwnd)
    }

    // How long each step of a move into the taskbar or out of it took
    // (ms), for its line: one frame of DWM's is all it has (go_in).
    struct Steps {
        at: Instant,
        said: Vec<String>,
    }

    impl Steps {
        fn new() -> Self {
            Steps { at: Instant::now(), said: Vec::new() }
        }
        fn mark(&mut self, what: &str) {
            let now = Instant::now();
            self.said.push(format!("{what} {:.1}", (now - self.at).as_secs_f64() * 1000.0));
            self.at = now;
        }
        fn said(&self, own_thread: bool) -> String {
            format!("{} ms{}", self.said.join(", "), if own_thread { "" } else { ", not on its own thread" })
        }
    }

    // Whether this is the window's own thread (frameless.rs on_its_thread ran the job there).
    fn on_thread(hwnd: Hwnd) -> bool {
        // SAFETY: plain queries.
        unsafe { GetWindowThreadProcessId(hwnd, std::ptr::null_mut()) == GetCurrentThreadId() }
    }

    // Into the taskbar, as TrafficMonitor's window goes (embed_spike.rs
    // --layered --popup): made layered and wholly opaque (once; frameless.rs
    // keeps the bit as tao writes its style), then a child of the taskbar's
    // window. Left kept on top: it was taken off the top first, and so lay
    // under the taskbar, unseen, all through SetParent (10 to 20 ms here,
    // 2026-11-08: the capsule's going as the island closed back into it);
    // owned by the topmost taskbar (release) it stayed on top anyway, and a
    // child kept on top draws, and is hovered, as any (seen all along).
    // Whether it went in (its parent the taskbar's window now), and what was seen, for the log.
    fn adopt(hwnd: Hwnd, tray: Hwnd, layered: bool) -> (bool, String) {
        // SAFETY: plain calls on our own window; the parent a window that exists.
        unsafe {
            if !layered {
                crate::frameless::keep_layered(hwnd as isize);
                let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | WS_EX_LAYERED);
                SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);
            }
            // Owned by the taskbar while it was out (release): nobody's, then its child.
            SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, 0);
            let was = SetParent(hwnd, tray);
            let err = if was.is_null() { GetLastError() } else { 0 };
            let now = GetAncestor(hwnd, GA_PARENT);
            (now == tray, format!("SetParent gave {was:?}, error {err}, its parent now {now:?}"))
        }
    }

    // Out of the taskbar: a window of its own again, kept on top, where
    // the island has it (physical), shown if it is to be; and owned by
    // the taskbar's window (`owner`, none to own nobody): Windows keeps an
    // owned window above its owner, so the island stays over the taskbar
    // however Explorer raises it. Explorer brings its taskbar to the top
    // of the windows kept on top as it is pressed, and keeps it there
    // while it is the active window: raised over it again at every look at
    // the pointer (the first try) the two fought at 60 times a second, and
    // the whole machine lagged (the user, 2026-11-07: 点一下任务栏，再放光标到胶囊上，
    // 整个电脑直接变得很卡). Owned, nothing is raised at all.
    //
    // SetParent keeps a window's coordinates as they are, read in its new
    // parent's: the child at (1589, -800) in the taskbar's client area (on
    // the screen at 580) stood at (1589, -800) on the screen once out, till
    // laid again, its foot (the capsule) over the top of the screen for a
    // moment (seen in the test VM, 2026-11-08). So, still a child, it is laid
    // first where its place on the screen is in the taskbar's coordinates (off
    // the strip and unseen, along the bottom), and comes out right there.
    //
    // Whether it came out (its parent the desktop now), and what was seen.
    // In one go on the window's own thread after DWM's frame, as go_in: done
    // from another thread the capsule went for four frames as the island
    // opened out of it (caught on the screen, 2026-11-08).
    fn release(hwnd: Hwnd, (x, y): (i32, i32), (w, h): (i32, i32), shown: bool, owner: Hwnd) -> (bool, String) {
        let (hw, ow) = (hwnd as isize, owner as isize);
        crate::frameless::on_its_thread(hw, move || {
            let (hwnd, owner) = (hw as Hwnd, ow as Hwnd);
            // SAFETY: plain queries; a stale handle answers nothing.
            let (desktop, parent) = unsafe {
                if IsWindow(hwnd) == 0 {
                    return (true, "gone".into());
                }
                (GetDesktopWindow(), GetAncestor(hwnd, GA_PARENT))
            };
            let mut steps = Steps::new();
            // The strip it shows in: the taskbar's window it is a child of.
            let strip = if parent != desktop { parent } else { std::ptr::null_mut() };
            let (ok, said) = stilled(hwnd, strip, &mut steps, |steps| {
                // SAFETY: plain calls on our own window; a stale handle only makes them fail.
                let (ok, said) = unsafe {
                    if parent != desktop {
                        SetWindowPos(hwnd, 0, x, y, w, h, SWP_NOZORDER | SWP_NOACTIVATE);
                    }
                    steps.mark("laid");
                    let was = SetParent(hwnd, std::ptr::null_mut());
                    let err = if was.is_null() { GetLastError() } else { 0 };
                    let now = GetAncestor(hwnd, GA_PARENT);
                    steps.mark("out");
                    if now != desktop {
                        return (false, format!("SetParent gave {was:?}, error {err}, its parent still {now:?}"));
                    }
                    SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, owner as isize);
                    // Where it is meant to be already (laid so as a child first):
                    // only raised, and shown; sized and its frame recalculated
                    // only should it not be (2026-11-08: the move out took 7 to
                    // 15 ms here with them, the capsule unseen meanwhile).
                    let placed = rect_of(hwnd) == Some((x, y, w, h));
                    let lay = if placed { SWP_NOMOVE | SWP_NOSIZE } else { SWP_FRAMECHANGED };
                    SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE | lay | if shown { SWP_SHOWWINDOW } else { 0 });
                    steps.mark("on top");
                    (true, format!("owner {:?}", GetWindow(hwnd, GW_OWNER)))
                };
                settle_webview(hwnd);
                steps.mark("page");
                (ok, said)
            });
            (ok, format!("{said}; {}", steps.said(on_thread(hwnd))))
        })
    }

    // What lies on the taskbar (x, w each, physical): the tray's window
    // (its icons, the clock, the show-the-desktop sliver), Start's where it
    // is a window, and every button UI Automation knows on it (Start,
    // search, task view, the widgets, the programs'); nothing of ours (the
    // island's own window, a child of it or owned by it).
    fn taken_on(tray: Hwnd, ours: Hwnd) -> Vec<Span> {
        let mut out = Vec::new();
        for class in ["TrayNotifyWnd", "Start"] {
            let name = wide(class);
            // SAFETY: a class name of ours; a child window of the taskbar's, or none.
            let h = unsafe { FindWindowExW(tray, std::ptr::null_mut(), name.as_ptr(), std::ptr::null()) };
            if h.is_null() || unsafe { IsWindowVisible(h) } == 0 {
                continue;
            }
            if let Some(r) = rect_of(h) {
                out.push((r.0, r.2));
            }
        }
        out.extend(buttons_on(tray, ours).unwrap_or_default());
        out
    }

    // The buttons on the taskbar by UI Automation: each shown one's stretch.
    // Looked for only in Explorer's own windows in the taskbar (its XAML's
    // Windows.UI.Composition.DesktopWindowContentBridge holds them all: Start,
    // search, the widgets, the programs', the tray's icons), never from the
    // taskbar's own element: UI Automation counts a window owned by the
    // taskbar among its children, and a child of it too, so the island's
    // window, out of the taskbar and owned by it (release) or in it, had its
    // page's buttons taken for the taskbar's, the open island's among them
    // as it grew. The stretch left for the capsule shrank under the pointer,
    // the capsule did not fit and was hidden, the page lost the pointer, and
    // so on, out and back in every couple of seconds (the user, 2026-11-07:
    // 光标移动到灵动岛上面，灵动岛一直在到处乱动). Its page's elements walked
    // every second, closed too (136 of the taskbar's 160 in the test VM),
    // also kept WebView2's accessibility tree up for nothing; the bridge
    // alone: the same 16 buttons in 41 ms, not 215.
    fn buttons_on(tray: Hwnd, ours: Hwnd) -> Option<Vec<Span>> {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
        use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, TreeScope_Descendants, UIA_ButtonControlTypeId, UIA_MenuItemControlTypeId, UIA_SplitButtonControlTypeId};
        let mut out = Vec::new();
        let me = std::process::id();
        // SAFETY: COM calls on this thread, which has COM up (with_com); the
        // handles are windows that exist (or not: the calls then fail).
        unsafe {
            let uia: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
            let all = uia.CreateTrueCondition().ok()?;
            let mut kid = GetWindow(tray, GW_CHILD);
            while !kid.is_null() {
                let this = kid;
                kid = GetWindow(kid, GW_HWNDNEXT);
                let mut pid = 0u32;
                GetWindowThreadProcessId(this, &mut pid);
                if this == ours || pid == me || IsWindowVisible(this) == 0 {
                    continue;
                }
                let Ok(root) = uia.ElementFromHandle(HWND(this)) else { continue };
                let Ok(found) = root.FindAll(TreeScope_Descendants, &all) else { continue };
                let n = found.Length().unwrap_or(0);
                for i in 0..n {
                    let Ok(el) = found.GetElement(i) else { continue };
                    let Ok(kind) = el.CurrentControlType() else { continue };
                    if kind != UIA_ButtonControlTypeId && kind != UIA_SplitButtonControlTypeId && kind != UIA_MenuItemControlTypeId {
                        continue;
                    }
                    if el.CurrentIsOffscreen().map(|b| b.as_bool()).unwrap_or(false) {
                        continue;
                    }
                    let h = el.CurrentNativeWindowHandle().map(|h| h.0).unwrap_or(std::ptr::null_mut());
                    if !h.is_null() && (h == ours || IsChild(ours, h) != 0) {
                        continue;
                    }
                    let Ok(r) = el.CurrentBoundingRectangle() else { continue };
                    if r.right > r.left && r.bottom > r.top {
                        out.push((r.left, r.right - r.left));
                    }
                }
            }
        }
        Some(out)
    }
}

#[cfg(not(windows))]
mod imp {
    use std::sync::Arc;

    use crate::Shared;

    pub fn start(_sh: &Arc<Shared>) {}
    pub fn look_soon() {}
    pub fn stop() {}
    pub fn is_parented() -> bool {
        false
    }
    pub fn is_window(hwnd: isize) -> bool {
        hwnd != 0
    }
    pub fn lay_child(_pos: (i32, i32), _size: (i32, i32)) {}
    pub fn apply(_sh: &Shared) {}
    pub fn where_() -> serde_json::Value {
        serde_json::Value::Null
    }
}

pub use imp::{apply, is_parented, is_window, lay_child, look_soon, start, stop, where_};

#[cfg(test)]
mod tests {
    use super::*;

    // Windows 11 26200 at 125%, as embed_spike.rs saw it: Start from 1004,
    // the programs' buttons to 1555, the tray from 1898 on a 2560 strip.
    const BAR: Span = (0, 2560);
    const TAKEN: [Span; 4] = [(1004, 60), (1064, 491), (1898, 662), (1200, 50)];

    #[test]
    fn the_free_stretches_between_the_buttons_and_the_tray() {
        // Left of Start, between the buttons and the tray; nothing past the
        // tray, which reaches the strip's end; a button inside another
        // (overlapping) counted once.
        assert_eq!(gaps(BAR, &TAKEN, 16), vec![(0, 1004), (1555, 343)]);
        // Nothing taken: the whole strip; everything taken: none.
        assert_eq!(gaps(BAR, &[], 16), vec![(0, 2560)]);
        assert_eq!(gaps(BAR, &[(0, 2560)], 16), Vec::<Span>::new());
        // A stretch too short to count is left out; what lies off the strip is cut to it.
        assert_eq!(gaps(BAR, &[(0, 1000), (1010, 1550)], 16), Vec::<Span>::new());
        assert_eq!(gaps((100, 1000), &[(-50, 200), (1050, 200)], 16), vec![(150, 900)]);
    }

    #[test]
    fn the_stretch_by_where_the_island_sits() {
        let free = gaps(BAR, &TAKEN, 16);
        // The left: the strip's left end; the right (and the middle, kept
        // from the top of the screen): before the tray.
        assert_eq!(pick(&free, Side::Left), Some((0, 1004)));
        assert_eq!(pick(&free, Side::Right), Some((1555, 343)));
        assert_eq!(pick(&free, Side::Center), Some((1555, 343)));
        // Buttons at the left (taskbarAlign left): the first gap is between them and the tray.
        let left_aligned = gaps(BAR, &[(0, 600), (1898, 662)], 16);
        assert_eq!(pick(&left_aligned, Side::Left), Some((600, 1298)));
        assert_eq!(pick(&[], Side::Center), None);
    }

    #[test]
    fn the_capsule_in_its_stretch() {
        // 300 x 36 at 125%: 375 wide, 10 px of margin (MARGIN 8).
        let (w, m) = (375, 10);
        assert_eq!(place_in((0, 1004), Side::Left, w, m), 10);
        assert_eq!(place_in((1555, 343), Side::Right, w, m), 1555 + 343 - 10 - 375);
        assert_eq!(place_in((1555, 343), Side::Center, w, m), 1555 + 343 - 10 - 375);
        // It fits with its margins, or not: the page is told the room
        // (the stretch less the margins) and narrows, or shows her alone.
        assert!(fits((0, 1004), w, m));
        assert!(!fits((1555, 343), w, m));
        assert!(fits((1555, 343), 300, m));
        assert!(!fits((1555, 343), 0, m));
    }

    #[test]
    fn the_capsule_as_wide_as_its_page_will_lay_it() {
        // At 125%: normal 375, narrow 300, her portrait alone 45; 10 px of margin.
        let (w, narrow, tight, m) = (375, 300, 45, 10);
        // Room for it as it is: that.
        assert_eq!(width_in((0, 949), w, narrow, tight, m), Some(375));
        // Just left of the tray, 363 px (the user's machine, 2026-11-08):
        // narrowed, laid there at once, not taken for no room at all.
        assert_eq!(width_in((1610, 363), w, narrow, tight, m), Some(300));
        assert_eq!(place_in((1610, 363), Side::Right, 300, m), 1610 + 363 - 10 - 300);
        // Too short for the narrowest: her portrait alone; too short for that: none.
        assert_eq!(width_in((1610, 100), w, narrow, tight, m), Some(45));
        assert_eq!(width_in((1610, 60), w, narrow, tight, m), None);
        // Set narrower than the narrowest (or narrowed already): never widened here.
        assert_eq!(width_in((1610, 363), 280, narrow, tight, m), Some(280));
        assert_eq!(width_in((0, 949), 300, narrow, tight, m), Some(300));
    }

    #[test]
    fn the_still_covers_only_the_islands_part_of_the_strip() {
        // The island's window (950 x 860 at (0, 580)) over the strip along
        // the bottom (2560 x 60 at (0, 1380)): its foot, the strip's height.
        let strip = (0, 1380, 2560, 60);
        assert_eq!(overlap((0, 580, 950, 860), strip), Some((0, 1380, 950, 60)));
        // Along the top, its head; at the right, cut at the strip's end.
        assert_eq!(overlap((1800, -20, 950, 860), (0, 0, 2560, 48)), Some((1800, 0, 760, 48)));
        // Off the strip, or only touching it: nothing.
        assert_eq!(overlap((0, 0, 950, 860), strip), None);
        assert_eq!(overlap((0, 520, 950, 860), strip), None);
    }
}
