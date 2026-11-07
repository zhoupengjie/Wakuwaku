// Her capsule inside Windows' own taskbar (zhoupengjie/Wakuwaku#105; the
// user, 2026-11-07: 做嵌入模式, as TrafficMonitor does it, at both ends).
// Her island set to the taskbar (islandEdge taskbar) along the bottom or the
// top of her display: a small window of ours, as big as the closed capsule,
// is made a child of Explorer's taskbar window (Shell_TrayWnd, or a second
// display's Shell_SecondaryTrayWnd) with SetParent, as TrafficMonitor's is,
// and lies on the taskbar's own ground in the free stretch at one end of
// it: at the right, just left of the tray's icons (TrafficMonitor's place);
// at the left, at the strip's start (left of Start, where Start is there).
// Its page (pet/index.html?role=island&part=capsule, island.js) draws the
// closed capsule alone: her portrait, the session's words, the clock, the
// widgets taking their turns, the flow light's ring. Everything that opens
// from it (hovered, a prompt, the settings, pulling her out) is the island's
// own window's (island.rs): shown as the pointer comes onto the capsule,
// its own closed island laid exactly over the capsule and unseen (island.js
// isl-ghost), so it takes the pointer over and grows from there; hidden
// again once it has shrunk back and the pointer has gone.
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
// hiding itself, or put away, the capsule's window is taken out and hidden
// and the island lies over the work area's edge as it does with the
// embedding turned off (island.rs fit). Looked at every second (the
// programs' buttons come and go, Explorer may start again, which takes its
// taskbar's children with it: the window gone, it is made again, under a
// label of its own, and put into the new taskbar), and at once as the
// settings or the capsule's size change (look_soon).
use crate::island::Side;

// Physical px: x and w along the strip.
pub type Span = (i32, i32);
// Physical px: x, y, w, h.
pub type Rect = (i32, i32, i32, i32);

// The window's margin round the capsule (logical px): the flow light's ring
// runs 3 px outside it (style.css #flow), and the island's own window keeps
// its island 8 px off its edge (island.js TOP, midLeft): laid at the
// strip's start, the capsule and the island over it then line up exactly
// (a capsule 4 px in showed 4 px past the open island's left, 2026-11-07).
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
// stretch's end (the window's margin round it).
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

// Her capsule as it lies now (physical), for the page and the island: the
// capsule's own rect, the room the page is told (physical px, 0 none) and
// whether its window is in the taskbar.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Laid {
    pub cap: Option<Rect>,
    pub room: i32,
    pub embedded: bool,
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::{Arc, Condvar, Mutex};
    use std::time::{Duration, Instant};

    use serde_json::{json, Value};
    use tauri::utils::config::BackgroundThrottlingPolicy;
    use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

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
        fn GetAncestor(hwnd: Hwnd, flags: u32) -> Hwnd;
        fn GetWindowLongPtrW(hwnd: Hwnd, index: i32) -> isize;
        fn SetWindowLongPtrW(hwnd: Hwnd, index: i32, value: isize) -> isize;
        fn SetWindowPos(hwnd: Hwnd, after: isize, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
        fn ShowWindow(hwnd: Hwnd, cmd: i32) -> i32;
        fn ScreenToClient(hwnd: Hwnd, pt: *mut Point) -> i32;
        fn GetDpiForWindow(hwnd: Hwnd) -> u32;
        fn SetLayeredWindowAttributes(hwnd: Hwnd, key: u32, alpha: u8, flags: u32) -> i32;
        fn FindWindowExW(parent: Hwnd, after: Hwnd, class: *const u16, title: *const u16) -> Hwnd;
        fn GetWindow(hwnd: Hwnd, cmd: u32) -> Hwnd;
    }

    const GWL_EXSTYLE: i32 = -20;
    // The window's parent in the hierarchy: GetParent gives a popup's owner,
    // and nothing for an overlapped window (tao's), whatever SetParent did.
    const GA_PARENT: u32 = 1;
    // The first of a window's children (the top of their order).
    const GW_CHILD: u32 = 5;
    const WS_EX_LAYERED: isize = 0x0008_0000;
    const LWA_ALPHA: u32 = 0x2;
    const HWND_TOP: isize = 0;
    const HWND_NOTOPMOST: isize = -2;
    const SWP_NOSIZE: u32 = 0x1;
    const SWP_NOMOVE: u32 = 0x2;
    const SWP_NOACTIVATE: u32 = 0x10;
    const SWP_FRAMECHANGED: u32 = 0x20;
    const SWP_SHOWWINDOW: u32 = 0x40;
    const SW_HIDE: i32 = 0;

    // Its window's label: LABEL the first, then LABEL-2, LABEL-3 as it is
    // made again after Explorer took it (Tauri keeps a webview's label for
    // one Explorer destroyed: "a webview with label `capsule` already
    // exists", 2026-11-07). The windows' pages are told by the label of the
    // one there is (label); any of them is hers (main.rs is_capsule).
    pub const LABEL: &str = "capsule";
    static MADE: AtomicU32 = AtomicU32::new(0);
    // How often the taskbar is looked at while the capsule is wanted.
    const EVERY: Duration = Duration::from_secs(1);

    // The label of the capsule's window now (the one made last).
    pub fn label() -> String {
        match MADE.load(Ordering::SeqCst) {
            0 | 1 => LABEL.to_string(),
            n => format!("{LABEL}-{n}"),
        }
    }

    #[derive(Default)]
    struct Embed {
        // Its window asked for (made off this thread), and its handle once made.
        created: bool,
        hwnd: isize,
        // The taskbar window it is in now (0: none, out on its own, hidden).
        tray: isize,
        // Its window's rect (physical) as laid last, and whether it is shown.
        rect: Option<Rect>,
        shown: bool,
        // What the island and the page were last told.
        laid: Laid,
        // The stretches seen last, and the one picked, for the log.
        said: String,
        // The pointer on the capsule (its page said), and when it was last.
        over: bool,
        over_at: Option<Instant>,
        // Its page told to draw nothing (the settings standing above the
        // taskbar in the island's window, the capsule left showing under
        // them: two islands, the user, 2026-11-07).
        covered: bool,
    }

    static EMBED: Mutex<Embed> = Mutex::new(Embed { created: false, hwnd: 0, tray: 0, rect: None, shown: false, laid: Laid { cap: None, room: 0, embedded: false }, said: String::new(), over: false, over_at: None, covered: false });
    static WAKE: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());
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

    // Quitting: the capsule's window out of Explorer's taskbar before it
    // goes with her (a child of another process's window left behind).
    pub fn stop() {
        QUITTING.store(true, Ordering::SeqCst);
        look_soon();
        let mut e = EMBED.lock().unwrap();
        if e.hwnd != 0 && e.tray != 0 {
            // SAFETY: plain calls on our own window; a stale handle only makes them fail.
            unsafe {
                ShowWindow(e.hwnd as Hwnd, SW_HIDE);
                SetParent(e.hwnd as Hwnd, std::ptr::null_mut());
            }
            e.tray = 0;
        }
    }

    // The capsule's page says the pointer is on it, or not: the island's
    // window shows over it for the pointer (island.rs is_shown).
    pub fn capsule_hover(sh: &Arc<Shared>, over: bool) {
        {
            let mut e = EMBED.lock().unwrap();
            e.over = over;
            e.over_at = Some(Instant::now());
        }
        if over {
            crate::wake_monitor();
        }
        island::apply_visibility(sh);
        // The pointer's loop looks now, not up to half a second on (it waits
        // longer while the island's window is hidden): the window shown, the
        // pointer over it is forwarded to its page at once (pointer.rs).
        crate::wake_pointer();
    }

    // Whether the capsule is being used: the pointer on it, or gone from it
    // this moment (the island's window takes it over meanwhile).
    pub fn in_use() -> bool {
        let e = EMBED.lock().unwrap();
        e.over || e.over_at.is_some_and(|at| at.elapsed() < Duration::from_millis(600))
    }

    // Its page is up: drawn, and looked at again (its size is to come).
    pub fn ready(sh: &Arc<Shared>) {
        sh.log("capsule: page ready");
        EMBED.lock().unwrap().covered = false;
        sh.redraw();
        island::tell_cover(sh);
        look_soon();
    }

    // The capsule's page told to draw nothing, or to draw again
    // (capsule:cover): while the settings stand above the taskbar in the
    // island's window, which does not cover the capsule as the open island
    // does. Told as it changes.
    pub fn cover(sh: &Shared, on: bool) {
        {
            let mut e = EMBED.lock().unwrap();
            if e.covered == on || !e.created {
                return;
            }
            e.covered = on;
        }
        let _ = sh.app.emit_to(label().as_str(), "capsule:cover", json!({ "on": on }));
    }

    pub fn is_created() -> bool {
        EMBED.lock().unwrap().created
    }

    // Where it is, for /health.
    pub fn where_() -> Value {
        let e = EMBED.lock().unwrap();
        json!({
            "label": label(),
            "created": e.created,
            "hwnd": e.hwnd,
            "tray": e.tray,
            "shown": e.shown,
            "rect": e.rect.map(|r| [r.0, r.1, r.2, r.3]),
            "cap": e.laid.cap.map(|r| [r.0, r.1, r.2, r.3]),
            "room": e.laid.room,
            "embedded": e.laid.embedded,
            "over": e.over,
            "covered": e.covered,
            "gaps": e.said,
        })
    }

    // One look: Explorer's taskbar along the bottom or the top of her
    // display, what is on it, the stretch for the capsule, its window there
    // (made, put in, laid, shown or hidden), and the island told where it lies.
    fn look(sh: &Arc<Shared>) {
        let wanted = island::embedding_wanted(sh);
        let bar = wanted.then(|| island::home_area(sh)).flatten().and_then(|a| shell::shown_bar(a.mon)).filter(|(_, _, edge)| matches!(edge, Edge::Top | Edge::Bottom));
        let Some((tray, strip, edge)) = bar else {
            return detach(sh, if wanted { "no taskbar of Explorer's shown along the bottom or the top of her display" } else { "not wanted" });
        };
        let hwnd = {
            let e = EMBED.lock().unwrap();
            if !e.created {
                drop(e);
                create(sh);
                return;
            }
            e.hwnd
        };
        if hwnd == 0 {
            return;
        }
        // SAFETY: a plain query about a window; a stale handle answers no.
        if unsafe { IsWindow(hwnd as Hwnd) } == 0 {
            // Gone with Explorer's taskbar (its children go with it): made
            // again, under the next label; the gone one let go of too,
            // whatever Tauri keeps of it.
            let gone = label();
            let mut e = EMBED.lock().unwrap();
            e.created = false;
            e.hwnd = 0;
            e.tray = 0;
            e.rect = None;
            e.shown = false;
            drop(e);
            sh.disown(hwnd);
            sh.log(&format!("capsule: its window {gone} is gone (Explorer started again?): made anew"));
            if let Some(win) = sh.app.get_webview_window(&gone) {
                let _ = win.destroy();
            }
            return look_soon();
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
        let up = fit && sh.is_visible() && !sh.is_fullscreen();
        // Into the taskbar (made layered first, its own style kept, no
        // longer kept on top: a child is in its parent's order).
        // SAFETY: a plain query about our own window.
        let parent = unsafe { GetAncestor(hwnd as Hwnd, GA_PARENT) } as isize;
        if parent != tray {
            adopt(hwnd as Hwnd, tray as Hwnd);
            sh.log(&format!("capsule: into Explorer's taskbar {tray:#x} at {strip:?} along the {edge:?} (dpi {})", (sf * 96.0).round()));
        }
        let cap = gap.filter(|_| fit).map(|g| (place_in(g, side, w, margin), strip.1 + (strip.3 - h) / 2, w, h));
        let rect = cap.map(|c| (c.0 - margin, c.1 - margin, c.2 + 2 * margin, c.3 + 2 * margin));
        let said = format!("{free:?} of {taken:?} on {strip:?} along the {edge:?}; {} {:?}", side.name(), gap);
        let (laid_before, moved) = {
            let mut e = EMBED.lock().unwrap();
            e.tray = tray;
            let moved = up && (e.rect != rect || !e.shown);
            if moved {
                e.rect = rect;
            }
            if !up && e.shown {
                // SAFETY: a plain call on our own window.
                unsafe { ShowWindow(hwnd as Hwnd, SW_HIDE) };
            }
            e.shown = up;
            if e.said != said {
                e.said = said.clone();
                sh.log(&format!("capsule: free {said}; the capsule {w}x{h} {}", if up { "shown" } else if fit { "hidden with her" } else { "does not fit: hidden" }));
            }
            (std::mem::replace(&mut e.laid, Laid { cap: cap.filter(|_| up), room, embedded: true }), moved)
        };
        if let (true, Some(r)) = (moved, rect) {
            lay(hwnd as Hwnd, tray as Hwnd, r);
        } else if up {
            // Explorer's own children made after ours (its XAML, as a new
            // Explorer starts up) lie over it, and it is unseen and gets no
            // pointer: first among its siblings again.
            // SAFETY: plain calls on windows that exist.
            unsafe {
                if GetWindow(tray as Hwnd, GW_CHILD) != hwnd as Hwnd {
                    SetWindowPos(hwnd as Hwnd, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                    sh.log("capsule: under the taskbar's own children: first among them again");
                }
            }
        }
        let laid = Laid { cap: cap.filter(|_| up), room, embedded: true };
        if laid != laid_before {
            island::set_capsule(sh, laid);
        }
    }

    // Out of the taskbar and hidden: the island lies as it does with no
    // capsule in the taskbar (island.rs fit).
    fn detach(sh: &Arc<Shared>, why: &str) {
        let (was, hwnd, tray) = {
            let mut e = EMBED.lock().unwrap();
            let was = std::mem::take(&mut e.laid);
            let (hwnd, tray) = (e.hwnd, std::mem::take(&mut e.tray));
            e.shown = false;
            e.rect = None;
            e.said.clear();
            (was, hwnd, tray)
        };
        if hwnd != 0 && tray != 0 {
            // SAFETY: plain calls on our own window; a stale handle only makes them fail.
            unsafe {
                ShowWindow(hwnd as Hwnd, SW_HIDE);
                SetParent(hwnd as Hwnd, std::ptr::null_mut());
            }
            sh.log(&format!("capsule: out of the taskbar ({why})"));
        }
        if was != Laid::default() {
            island::set_capsule(sh, Laid::default());
        }
    }

    // Its window: made once off this thread (building waits on the main
    // thread), kept, hidden while out of the taskbar. The same page as the
    // island's, as the capsule alone.
    fn create(sh: &Arc<Shared>) {
        {
            let mut e = EMBED.lock().unwrap();
            if e.created {
                return;
            }
            e.created = true;
        }
        let sh = sh.clone();
        std::thread::spawn(move || {
            MADE.fetch_add(1, Ordering::SeqCst);
            let label = label();
            let built = WebviewWindowBuilder::new(&sh.app, &label, WebviewUrl::App("pet/index.html?role=island&part=capsule".into()))
                .title("wakuwaku capsule")
                .inner_size(CAPSULE.0 + 2.0 * MARGIN, CAPSULE.1 + 2.0 * MARGIN)
                .additional_browser_args(crate::browser_args(&sh))
                .data_directory(crate::webview_dir(&sh))
                .transparent(true)
                .decorations(false)
                .resizable(false)
                .maximizable(false)
                .minimizable(false)
                .skip_taskbar(true)
                .shadow(false)
                .always_on_top(true)
                .visible_on_all_workspaces(true)
                .focusable(false)
                .focused(false)
                .visible(false)
                .background_throttling(BackgroundThrottlingPolicy::Disabled)
                .build();
            match built {
                Ok(win) => {
                    let hwnd = win.hwnd().map(|h| h.0 as isize).unwrap_or(0);
                    crate::tasks::no_transitions(hwnd);
                    sh.own_window(&win);
                    // Layered it stays, whatever tao writes back (frameless.rs).
                    crate::frameless::keep_layered(hwnd);
                    crate::crash::watch(&sh, &win);
                    crate::addons::guard_frames(&sh, &win, false);
                    EMBED.lock().unwrap().hwnd = hwnd;
                    sh.log(&format!("capsule: window {hwnd:#x} made ({label})"));
                    look_soon();
                }
                Err(err) => {
                    EMBED.lock().unwrap().created = false;
                    sh.log(&format!("capsule: could not make its window: {err}"));
                }
            }
        });
    }

    // Into the taskbar, as TrafficMonitor's window goes (embed_spike.rs
    // --layered --popup): no longer kept on top, made layered and wholly
    // opaque, then a child of the taskbar's window.
    fn adopt(hwnd: Hwnd, tray: Hwnd) {
        // SAFETY: plain calls on our own window; the parent a window that exists.
        unsafe {
            SetWindowPos(hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | WS_EX_LAYERED);
            SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);
            SetParent(hwnd, tray);
        }
    }

    // Laid on its rect (physical) in the taskbar's client area, first among
    // its siblings, and shown.
    fn lay(hwnd: Hwnd, tray: Hwnd, r: Rect) {
        let mut pt = Point { x: r.0, y: r.1 };
        // SAFETY: plain calls on our own window and its parent.
        unsafe {
            ScreenToClient(tray, &mut pt);
            SetWindowPos(hwnd, HWND_TOP, pt.x, pt.y, r.2, r.3, SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_FRAMECHANGED);
        }
    }

    // What lies on the taskbar (x, w each, physical): the tray's window
    // (its icons, the clock, the show-the-desktop sliver), Start's where it
    // is a window, and every button UI Automation knows on it (Start,
    // search, task view, the widgets, the programs'); nothing of ours (the
    // capsule's own window, a child of it).
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

    pub const LABEL: &str = "capsule";
    pub fn label() -> String {
        LABEL.to_string()
    }
    pub fn start(_sh: &Arc<Shared>) {}
    pub fn look_soon() {}
    pub fn stop() {}
    pub fn capsule_hover(_sh: &Arc<Shared>, _over: bool) {}
    pub fn cover(_sh: &Shared, _on: bool) {}
    pub fn in_use() -> bool {
        false
    }
    pub fn ready(_sh: &Arc<Shared>) {}
    pub fn is_created() -> bool {
        false
    }
    pub fn where_() -> serde_json::Value {
        serde_json::Value::Null
    }
}

pub use imp::{capsule_hover, cover, in_use, is_created, label, look_soon, ready, start, stop, where_, LABEL};

// The windows told of the island's state: the island's own, and the capsule's in the taskbar once made.
pub fn labels() -> impl Iterator<Item = String> {
    ["island".to_string(), label()].into_iter().take(if is_created() { 2 } else { 1 })
}

// Whether a window's label is the capsule's (any made so far).
pub fn is_label(label: &str) -> bool {
    label == LABEL || label.strip_prefix(LABEL).is_some_and(|rest| rest.strip_prefix('-').is_some_and(|n| n.parse::<u32>().is_ok()))
}

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
        // 300 x 36 at 125%: 375 wide, 5 px of margin (the ring's room).
        let (w, m) = (375, 5);
        assert_eq!(place_in((0, 1004), Side::Left, w, m), 5);
        assert_eq!(place_in((1555, 343), Side::Right, w, m), 1555 + 343 - 5 - 375);
        assert_eq!(place_in((1555, 343), Side::Center, w, m), 1555 + 343 - 5 - 375);
        // It fits with its margins, or not: the page is told the room
        // (the stretch less the margins) and narrows, or shows her alone.
        assert!(fits((0, 1004), w, m));
        assert!(!fits((1555, 343), w, m));
        assert!(fits((1555, 343), 300, m));
        assert!(!fits((1555, 343), 0, m));
    }

    #[test]
    fn the_windows_label_is_hers_whatever_time_it_was_made() {
        assert!(is_label("capsule"));
        assert!(is_label("capsule-2"));
        assert!(is_label("capsule-13"));
        assert!(!is_label("capsule-"));
        assert!(!is_label("capsules"));
        assert!(!is_label("island"));
        assert!(!is_label("capsule-x"));
    }
}
