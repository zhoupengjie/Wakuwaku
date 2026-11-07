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
// alone, or hides, as the room goes (anchor.js capsuleWidth): UI Automation
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
// off its window's edge (island.js TOP, midLeft).
pub const MARGIN: f64 = 8.0;
// The closed capsule before its page has said its size (island.js WIDTHS normal, 36 high).
pub const CAPSULE: (f64, f64) = (300.0, 36.0);

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
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Condvar, Mutex};
    use std::time::Duration;

    use serde_json::{json, Value};

    use super::{fits, gaps, pick, place_in, Laid, Rect, Span, CAPSULE, MARGIN};
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
    }

    const GWL_EXSTYLE: i32 = -20;
    // A top-level window's owner (set this way, as WPF sets a window's Owner).
    const GWLP_HWNDPARENT: i32 = -8;
    // The first of a window's children (the top of their order).
    const GW_CHILD: u32 = 5;
    const WS_EX_LAYERED: isize = 0x0008_0000;
    const LWA_ALPHA: u32 = 0x2;
    const HWND_TOP: isize = 0;
    const HWND_TOPMOST: isize = -1;
    const HWND_NOTOPMOST: isize = -2;
    const SWP_NOSIZE: u32 = 0x1;
    const SWP_NOMOVE: u32 = 0x2;
    const SWP_NOZORDER: u32 = 0x4;
    const SWP_NOACTIVATE: u32 = 0x10;
    const SWP_FRAMECHANGED: u32 = 0x20;
    const SWP_SHOWWINDOW: u32 = 0x40;
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
        // What the island was last told.
        laid: Laid,
        // The stretches seen last, and the one picked, for the log.
        said: String,
        // The window gone with Explorer's taskbar: her restart under way.
        restarting: bool,
    }

    static EMBED: Mutex<Embed> = Mutex::new(Embed { hwnd: 0, tray: 0, parented: false, owned: false, child: None, layered: false, laid: Laid { cap: None, room: 0, embedded: false }, said: String::new(), restarting: false });
    static WAKE: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());
    // One apply at a time (the pointer's thread and the page's command both
    // call it): two at once both saw the window out and both put it in.
    static APPLYING: Mutex<()> = Mutex::new(());
    static STARTED: AtomicBool = AtomicBool::new(false);
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
                    let (mut guard, _) = cv.wait_timeout_while(guard, EVERY, |w| !*w).unwrap();
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
    pub fn lay_child((x, y): (i32, i32), (w, h): (i32, i32)) {
        let (hwnd, tray) = {
            let mut e = EMBED.lock().unwrap();
            if !e.parented || e.hwnd == 0 || e.tray == 0 {
                return;
            }
            if e.child == Some((x, y, w, h)) {
                return;
            }
            e.child = Some((x, y, w, h));
            (e.hwnd as Hwnd, e.tray as Hwnd)
        };
        let mut pt = Point { x, y };
        // SAFETY: plain calls on our own window and its parent.
        unsafe {
            ScreenToClient(tray, &mut pt);
            SetWindowPos(hwnd, HWND_TOP, pt.x, pt.y, w, h, SWP_NOACTIVATE | SWP_FRAMECHANGED);
        }
        settle_webview(hwnd);
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
        let (tray, parented, layered) = {
            let e = EMBED.lock().unwrap();
            (e.tray, e.parented, e.layered)
        };
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
                settle_webview(hwnd as Hwnd);
                return;
            }
            adopt(hwnd as Hwnd, tray as Hwnd, layered);
            {
                let mut e = EMBED.lock().unwrap();
                e.hwnd = hwnd;
                e.parented = true;
                e.owned = false;
                e.layered = true;
                e.child = None;
            }
            lay_child(pos, size);
            if shown {
                // SAFETY: a plain call on our own window.
                unsafe { SetWindowPos(hwnd as Hwnd, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW) };
            }
            sh.log(&format!("island: into Explorer's taskbar {tray:#x}, the capsule in the strip"));
        } else if parented {
            release(hwnd as Hwnd, pos, size, shown, tray as Hwnd);
            {
                let mut e = EMBED.lock().unwrap();
                e.parented = false;
                e.owned = tray != 0;
                e.child = None;
            }
            sh.log("island: out of the taskbar, whole over it, owned by it");
        }
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
                std::thread::spawn(move || match crate::updates::start_again(&sh) {
                    Ok(()) => sh.quit(crate::data::Quit::Wakeable, "her restart, the island's window gone with Explorer's taskbar"),
                    Err(e) => sh.log(&format!("island: could not start anew: {e}")),
                });
            }
            return;
        }
        let wanted = island::embedding_wanted(sh);
        let bar = wanted.then(|| island::home_area(sh)).flatten().and_then(|a| shell::shown_bar(a.mon)).filter(|(_, _, edge)| matches!(edge, Edge::Top | Edge::Bottom));
        let Some((tray, strip, edge)) = bar else {
            return detach(sh, if wanted { "no taskbar of Explorer's shown along the bottom or the top of her display" } else { "not wanted" });
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
        let fit = gap.is_some_and(|g| fits(g, w, margin));
        let cap = gap.filter(|_| fit).map(|g| (place_in(g, side, w, margin), strip.1 + (strip.3 - h) / 2, w, h));
        let said = format!("{free:?} of {taken:?} on {strip:?} along the {edge:?}; {} {:?}", side.name(), gap);
        let laid = Laid { cap, room, embedded: true };
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
                sh.log(&format!("island: free {said}; the capsule {w}x{h} {}", if fit { "fits" } else { "does not fit: the island hidden" }));
            }
            std::mem::replace(&mut e.laid, laid)
        };
        if laid != before {
            island::set_capsule(sh, laid);
        }
        apply(sh);
    }

    // Out of the taskbar, and the island laid as it is with no capsule in
    // the taskbar (island.rs fit).
    fn detach(sh: &Arc<Shared>, why: &str) {
        let parenting = island::parenting(sh);
        let (was, hwnd, parented, owned) = {
            let mut e = EMBED.lock().unwrap();
            let was = std::mem::take(&mut e.laid);
            let parented = std::mem::take(&mut e.parented);
            let owned = std::mem::take(&mut e.owned);
            e.tray = 0;
            e.child = None;
            e.said.clear();
            (was, e.hwnd, parented, owned)
        };
        if hwnd != 0 && is_window(hwnd) {
            if parented {
                let (pos, size, shown) = parenting.map_or(((0, 0), (1, 1), false), |p| (p.1, p.2, p.3));
                release(hwnd as Hwnd, pos, size, shown, std::ptr::null_mut());
                sh.log(&format!("island: out of the taskbar ({why})"));
            } else if owned {
                // A window of its own again, nobody's: the taskbar it was owned by is not hers to follow.
                // SAFETY: a plain call on our own window.
                unsafe { SetWindowLongPtrW(hwnd as Hwnd, GWLP_HWNDPARENT, 0) };
            }
        }
        if was != Laid::default() {
            island::set_capsule(sh, Laid::default());
        }
    }

    // Into the taskbar, as TrafficMonitor's window goes (embed_spike.rs
    // --layered --popup): no longer kept on top, made layered and wholly
    // opaque (once; frameless.rs keeps the bit as tao writes its style),
    // then a child of the taskbar's window.
    fn adopt(hwnd: Hwnd, tray: Hwnd, layered: bool) {
        // SAFETY: plain calls on our own window; the parent a window that exists.
        unsafe {
            SetWindowPos(hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            if !layered {
                crate::frameless::keep_layered(hwnd as isize);
                let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | WS_EX_LAYERED);
                SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);
            }
            // Owned by the taskbar while it was out (release): nobody's, then its child.
            SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, 0);
            SetParent(hwnd, tray);
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
    fn release(hwnd: Hwnd, (x, y): (i32, i32), (w, h): (i32, i32), shown: bool, owner: Hwnd) {
        // SAFETY: plain calls on our own window; a stale handle only makes them fail.
        unsafe {
            if IsWindow(hwnd) == 0 {
                return;
            }
            SetParent(hwnd, std::ptr::null_mut());
            SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, owner as isize);
            SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE | SWP_FRAMECHANGED | if shown { SWP_SHOWWINDOW } else { 0 });
        }
        settle_webview(hwnd);
    }

    // What lies on the taskbar (x, w each, physical): the tray's window
    // (its icons, the clock, the show-the-desktop sliver), Start's where it
    // is a window, and every button UI Automation knows on it (Start,
    // search, task view, the widgets, the programs'); nothing of ours (the
    // island's own window, a child of it).
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
    fn buttons_on(tray: Hwnd, ours: Hwnd) -> Option<Vec<Span>> {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
        use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, TreeScope_Descendants, UIA_ButtonControlTypeId, UIA_MenuItemControlTypeId, UIA_SplitButtonControlTypeId};
        let mut out = Vec::new();
        // SAFETY: COM calls on this thread, which has COM up (with_com); the
        // handles are windows that exist (or not: the calls then fail).
        unsafe {
            let uia: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
            let root = uia.ElementFromHandle(HWND(tray)).ok()?;
            let all = uia.CreateTrueCondition().ok()?;
            let found = root.FindAll(TreeScope_Descendants, &all).ok()?;
            let n = found.Length().ok()?;
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
}
