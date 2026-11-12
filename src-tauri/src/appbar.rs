// A bar's room on the screen: registered with Windows as an app bar, as the
// taskbar is, so the work area loses its height and maximized windows stop
// at it. The taskbar (display 'taskbar') takes a strip along an edge of the
// screen (taskbarEdge: the bottom, as Windows' own, or the top, the left,
// the right), Windows' own being put away (shell.rs, which reads and sets
// its state here). Given back when the bar goes (another home, out of
// sight, quitting). Windows only.

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

impl Edge {
    // The setting's word (taskbarEdge); anything else is the bottom.
    pub fn of(word: &str) -> Edge {
        match word {
            "top" => Edge::Top,
            "left" => Edge::Left,
            "right" => Edge::Right,
            _ => Edge::Bottom,
        }
    }

    // The setting's word for it.
    pub fn word(self) -> &'static str {
        match self {
            Edge::Top => "top",
            Edge::Bottom => "bottom",
            Edge::Left => "left",
            Edge::Right => "right",
        }
    }

    // Along the left or right of the screen: upright.
    pub fn is_side(self) -> bool {
        matches!(self, Edge::Left | Edge::Right)
    }

    // The strip this thick along this edge of a rect (x, y, w, h).
    pub fn strip(self, mon: (i32, i32, i32, i32), thickness: i32) -> (i32, i32, i32, i32) {
        match self {
            Edge::Top => (mon.0, mon.1, mon.2, thickness),
            Edge::Bottom => (mon.0, mon.1 + mon.3 - thickness, mon.2, thickness),
            Edge::Left => (mon.0, mon.1, thickness, mon.3),
            Edge::Right => (mon.0 + mon.2 - thickness, mon.1, thickness, mon.3),
        }
    }

    // A strip's inner side along this edge: where the work area stops.
    pub fn inner(self, strip: (i32, i32, i32, i32)) -> i32 {
        match self {
            Edge::Top => strip.1 + strip.3,
            Edge::Bottom => strip.1,
            Edge::Left => strip.0 + strip.2,
            Edge::Right => strip.0,
        }
    }

    // A rect's side along this edge (x, y, w, h).
    pub fn side_of(self, r: (i32, i32, i32, i32)) -> i32 {
        match self {
            Edge::Top => r.1,
            Edge::Bottom => r.1 + r.3,
            Edge::Left => r.0,
            Edge::Right => r.0 + r.2,
        }
    }

    // Whether a work area reaches into a strip along this edge.
    pub fn reaches(self, work: (i32, i32, i32, i32), strip: (i32, i32, i32, i32)) -> bool {
        let (side, inner) = (self.side_of(work), self.inner(strip));
        match self {
            Edge::Top | Edge::Left => side < inner,
            Edge::Bottom | Edge::Right => side > inner,
        }
    }

    // Whether a work area stops short of a strip along this edge (a gap
    // between them no maximized window reaches).
    pub fn short_of(self, work: (i32, i32, i32, i32), strip: (i32, i32, i32, i32)) -> bool {
        let (side, inner) = (self.side_of(work), self.inner(strip));
        match self {
            Edge::Top | Edge::Left => side > inner,
            Edge::Bottom | Edge::Right => side < inner,
        }
    }

    // Whether a strip lies along this edge of its display (x, y, w, h). One
    // told before the display changed lies where that edge was until it is
    // told anew: back from 1920×1080 to 2560×1440, the strip still at 1032
    // had the work area's bottom set there, and the new strip at 1392 had
    // 360 px above it no maximized window reached (2026-10-26).
    pub fn lies_along(self, strip: (i32, i32, i32, i32), mon: (i32, i32, i32, i32)) -> bool {
        self.side_of(strip) == self.side_of(mon)
    }

    // Where a work area's side along this edge is set: the strip's inner
    // side, when the work area reaches into the strip, or stops short of it
    // at the side last set for a strip of hers (`ours`: the strip moved, the
    // display at another scale, and the side set for the old one stayed);
    // None to leave it be (short of it otherwise: another bar's room, or
    // Windows' own).
    pub fn work_fix(self, work: (i32, i32, i32, i32), strip: (i32, i32, i32, i32), ours: Option<i32>) -> Option<i32> {
        let left_short = self.short_of(work, strip) && ours == Some(self.side_of(work));
        (self.reaches(work, strip) || left_short).then_some(self.inner(strip))
    }

    // The work area's sides other than this edge's (and the bottom, the
    // room Windows' own keeps: taskbar/ band_left) held in from the
    // display by as much as the strip is thick: a strip of hers along that
    // edge before, which Explorer, counting app bars again, kept (from the
    // right to the left, her taskbar on the main display alone, the right's
    // 60 px stayed: the test session, 2026-10-13). Each with the display's
    // side to give back. None along the bottom.
    pub fn stale_sides(self, work: (i32, i32, i32, i32), strip: (i32, i32, i32, i32), mon: (i32, i32, i32, i32)) -> Vec<(Edge, i32)> {
        if self == Edge::Bottom {
            return Vec::new();
        }
        let thick = if self.is_side() { strip.2 } else { strip.3 };
        [Edge::Top, Edge::Left, Edge::Right]
            .into_iter()
            .filter(|&e| e != self && (e.side_of(work) - e.side_of(mon)).abs() == thick)
            .map(|e| (e, e.side_of(mon)))
            .collect()
    }

    // A rect with its side along this edge moved to `side`.
    pub fn with_side(self, r: (i32, i32, i32, i32), side: i32) -> (i32, i32, i32, i32) {
        let (l, t, rt, b) = (r.0, r.1, r.0 + r.2, r.1 + r.3);
        let (l, t, rt, b) = match self {
            Edge::Top => (l, side, rt, b),
            Edge::Bottom => (l, t, rt, side),
            Edge::Left => (side, t, rt, b),
            Edge::Right => (l, t, side, b),
        };
        (l, t, rt - l, b - t)
    }
}

#[cfg(windows)]
mod imp {
    use super::Edge;
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    pub struct AppBarData {
        size: u32,
        hwnd: isize,
        callback: u32,
        edge: u32,
        rc: Rect,
        lparam: isize,
    }

    #[link(name = "shell32")]
    extern "system" {
        fn SHAppBarMessage(message: u32, data: *mut AppBarData) -> usize;
    }

    #[repr(C)]
    #[derive(Default)]
    struct MonitorInfo {
        size: u32,
        monitor: Rect,
        work: Rect,
        flags: u32,
    }

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    type Hwnd = *mut c_void;

    #[link(name = "user32")]
    extern "system" {
        fn MonitorFromPoint(pt: Point, flags: u32) -> *mut c_void;
        fn GetMonitorInfoW(monitor: *mut c_void, info: *mut MonitorInfo) -> i32;
        fn SystemParametersInfoW(action: u32, param: u32, pv: *mut c_void, flags: u32) -> i32;
        fn EnumWindows(each: extern "system" fn(Hwnd, isize) -> i32, param: isize) -> i32;
        fn IsWindowVisible(hwnd: Hwnd) -> i32;
        fn IsZoomed(hwnd: Hwnd) -> i32;
        fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
        fn SetWindowPos(hwnd: Hwnd, after: isize, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
        fn MonitorFromWindow(hwnd: Hwnd, flags: u32) -> *mut c_void;
    }

    const MONITOR_DEFAULTTONEAREST: u32 = 2;
    const SPI_SETWORKAREA: u32 = 0x2F;
    const SWP_NOZORDER: u32 = 0x4;
    const SWP_NOACTIVATE: u32 = 0x10;
    const SWP_NOOWNERZORDER: u32 = 0x200;
    const SWP_ASYNCWINDOWPOS: u32 = 0x4000;

    extern "system" fn collect_maximized(hwnd: Hwnd, param: isize) -> i32 {
        // SAFETY: param is the Vec refit_maximized passed, alive for the
        // enumeration; plain queries about a window Windows handed us.
        unsafe {
            if IsWindowVisible(hwnd) != 0 && IsZoomed(hwnd) != 0 {
                (*(param as *mut Vec<isize>)).push(hwnd as isize);
            }
        }
        1
    }

    // The windows maximized on a display laid on its work area as it now is:
    // set quietly, the work area moves none of them, and one maximized
    // before her strip went along the left of a display lay under it (the
    // test session, 2026-10-13). Each one lying over the work area as it
    // was (its frame past the edges, as Windows lays a maximized window)
    // has each side moved as far as the work area's moved; one already
    // over the new one, or laid otherwise, is left be. How many were moved.
    // Not waiting on their programs (SWP_ASYNCWINDOWPOS): it is called on
    // the taskbar's thread, the tray's too, which a busy one would hold.
    pub fn refit_maximized(was: (i32, i32, i32, i32), now: (i32, i32, i32, i32)) -> usize {
        if was == now {
            return 0;
        }
        let mut found: Vec<isize> = Vec::new();
        // SAFETY: the callback only pushes onto the Vec, which outlives the call.
        unsafe { EnumWindows(collect_maximized, &mut found as *mut Vec<isize> as isize) };
        // SAFETY: a plain query.
        let display = unsafe { MonitorFromPoint(Point { x: now.0 + now.2 / 2, y: now.1 + now.3 / 2 }, MONITOR_DEFAULTTONEAREST) };
        let mut info = MonitorInfo { size: std::mem::size_of::<MonitorInfo>() as u32, ..Default::default() };
        // SAFETY: our own struct, of the size the call writes.
        if unsafe { GetMonitorInfoW(display, &mut info) } == 0 {
            return 0;
        }
        let m = &info.monitor;
        let whole = (m.left, m.top, m.right - m.left, m.bottom - m.top);
        let mut moved = 0;
        for h in found {
            let mut r = Rect::default();
            // SAFETY: plain queries into our own struct; a gone window fails them.
            let ok = unsafe { MonitorFromWindow(h as Hwnd, MONITOR_DEFAULTTONEAREST) == display && GetWindowRect(h as Hwnd, &mut r) != 0 };
            let at = (r.left, r.top, r.right - r.left, r.bottom - r.top);
            let Some(to) = super::refit(at, was, now, whole).filter(|_| ok) else {
                continue;
            };
            // SAFETY: another program's window moved, not activated, not waited on; one we may not move (run as administrator) refuses.
            if unsafe { SetWindowPos(h as Hwnd, 0, to.0, to.1, to.2, to.3, SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_ASYNCWINDOWPOS) } != 0 {
                moved += 1;
            }
        }
        moved
    }

    // A display's work area (x, y, w, h, physical), the display by its rect.
    pub fn work_of(mon: (i32, i32, i32, i32)) -> Option<(i32, i32, i32, i32)> {
        let mut info = MonitorInfo { size: std::mem::size_of::<MonitorInfo>() as u32, ..Default::default() };
        let at = Point { x: mon.0 + mon.2 / 2, y: mon.1 + mon.3 / 2 };
        // SAFETY: our own struct, of the size the call writes.
        let ok = unsafe { GetMonitorInfoW(MonitorFromPoint(at, MONITOR_DEFAULTTONEAREST), &mut info) } != 0;
        let w = &info.work;
        ok.then_some((w.left, w.top, w.right - w.left, w.bottom - w.top))
    }

    // The work area of the display a rect lies on set to it, quietly, as
    // the main strip's is (taskbar/win/work.rs set_work_area: told to every window,
    // Explorer set it back at once).
    pub fn set_work(r: (i32, i32, i32, i32)) -> bool {
        let mut rect = Rect { left: r.0, top: r.1, right: r.0 + r.2, bottom: r.1 + r.3 };
        // SAFETY: our own struct, handed back.
        unsafe { SystemParametersInfoW(SPI_SETWORKAREA, 0, &mut rect as *mut Rect as *mut c_void, 0) != 0 }
    }

    const ABM_NEW: u32 = 0;
    const ABM_REMOVE: u32 = 1;
    const ABM_QUERYPOS: u32 = 2;
    const ABM_SETPOS: u32 = 3;
    const ABM_GETSTATE: u32 = 4;
    const ABM_SETSTATE: u32 = 10;
    const ABE_LEFT: u32 = 0;
    const ABE_TOP: u32 = 1;
    const ABE_RIGHT: u32 = 2;
    const ABE_BOTTOM: u32 = 3;
    // What Windows sends a bar about changes, as the message's wParam (the top
    // bar does not listen: it asks again whenever the screens change, see
    // island/). For ABN_FULLSCREENAPP, lParam is 1 when a full-screen
    // window opens on the bar's display and 0 when the last one goes.
    pub const CALLBACK: u32 = 0x8000 + 0x57;
    #[allow(dead_code)]
    pub const ABN_STATECHANGE: usize = 0;
    #[allow(dead_code)]
    pub const ABN_POSCHANGED: usize = 1;
    #[allow(dead_code)]
    pub const ABN_FULLSCREENAPP: usize = 2;
    // The taskbar's state: hiding itself (ABS_AUTOHIDE), and always on top.
    pub const ABS_AUTOHIDE: u32 = 1;

    fn data(hwnd: isize, edge: Edge) -> AppBarData {
        let edge = match edge {
            Edge::Top => ABE_TOP,
            Edge::Bottom => ABE_BOTTOM,
            Edge::Left => ABE_LEFT,
            Edge::Right => ABE_RIGHT,
        };
        AppBarData { size: std::mem::size_of::<AppBarData>() as u32, hwnd, callback: CALLBACK, edge, rc: Rect::default(), lparam: 0 }
    }

    pub fn register(hwnd: isize) -> bool {
        let mut d = data(hwnd, Edge::Top);
        // SAFETY: a struct of the size the call asks for, about our own window.
        unsafe { SHAppBarMessage(ABM_NEW, &mut d) != 0 }
    }

    pub fn remove(hwnd: isize) {
        let mut d = data(hwnd, Edge::Top);
        // SAFETY: as above.
        unsafe { SHAppBarMessage(ABM_REMOVE, &mut d) };
    }

    // Where a strip this thick along an edge of a display (x, y, w, h) would
    // go, as Windows would grant it now (another bar may be there first).
    fn ask(hwnd: isize, mon: (i32, i32, i32, i32), edge: Edge, thickness: i32) -> AppBarData {
        let mut d = data(hwnd, edge);
        let (x, y, w, h) = edge.strip(mon, thickness);
        d.rc = Rect { left: x, top: y, right: x + w, bottom: y + h };
        // SAFETY: as above.
        unsafe { SHAppBarMessage(ABM_QUERYPOS, &mut d) };
        // The edge may have moved in past another bar: the thickness stays
        // ours (a side bar's length may be shortened by one along the top or
        // the bottom).
        match edge {
            Edge::Top => d.rc.bottom = d.rc.top + thickness,
            Edge::Bottom => d.rc.top = d.rc.bottom - thickness,
            Edge::Left => d.rc.right = d.rc.left + thickness,
            Edge::Right => d.rc.left = d.rc.right - thickness,
        }
        d
    }

    fn rect(d: &AppBarData) -> (i32, i32, i32, i32) {
        (d.rc.left, d.rc.top, d.rc.right - d.rc.left, d.rc.bottom - d.rc.top)
    }

    // Asked only, nothing taken: x, y, w, h. When another bar moves
    // (ABN_POSCHANGED), taking the strip again only if this changed keeps two
    // bars from waking each other, back and forth.
    #[allow(dead_code)]
    pub fn query(hwnd: isize, mon: (i32, i32, i32, i32), edge: Edge, thickness: i32) -> (i32, i32, i32, i32) {
        rect(&ask(hwnd, mon, edge, thickness))
    }

    // The strip taken, as Windows grants it: x, y, w, h.
    pub fn place(hwnd: isize, mon: (i32, i32, i32, i32), edge: Edge, thickness: i32) -> (i32, i32, i32, i32) {
        let mut d = ask(hwnd, mon, edge, thickness);
        // SAFETY: as above.
        unsafe { SHAppBarMessage(ABM_SETPOS, &mut d) };
        rect(&d)
    }

    // Windows' own taskbar's state (ABS_*).
    pub fn taskbar_state() -> u32 {
        let mut d = data(0, Edge::Bottom);
        // SAFETY: as above; the call reads no window.
        unsafe { SHAppBarMessage(ABM_GETSTATE, &mut d) as u32 }
    }

    // Sets it, as the taskbar's settings do (Windows keeps it, across restarts).
    pub fn set_taskbar_state(taskbar: isize, state: u32) {
        let mut d = data(taskbar, Edge::Bottom);
        d.lparam = state as isize;
        // SAFETY: as above, about the taskbar's window.
        unsafe { SHAppBarMessage(ABM_SETSTATE, &mut d) };
    }
}

#[cfg(not(windows))]
mod imp {
    use super::Edge;

    pub const CALLBACK: u32 = 0;
    pub const ABN_STATECHANGE: usize = 0;
    pub const ABN_POSCHANGED: usize = 1;
    pub const ABN_FULLSCREENAPP: usize = 2;
    pub const ABS_AUTOHIDE: u32 = 1;

    pub fn register(_hwnd: isize) -> bool {
        false
    }
    pub fn remove(_hwnd: isize) {}
    pub fn place(_hwnd: isize, mon: (i32, i32, i32, i32), edge: Edge, thickness: i32) -> (i32, i32, i32, i32) {
        edge.strip(mon, thickness)
    }
    pub fn query(hwnd: isize, mon: (i32, i32, i32, i32), edge: Edge, thickness: i32) -> (i32, i32, i32, i32) {
        place(hwnd, mon, edge, thickness)
    }
    pub fn taskbar_state() -> u32 {
        0
    }
    pub fn set_taskbar_state(_taskbar: isize, _state: u32) {}
    pub fn work_of(_mon: (i32, i32, i32, i32)) -> Option<(i32, i32, i32, i32)> {
        None
    }
    pub fn set_work(_r: (i32, i32, i32, i32)) -> bool {
        false
    }
    pub fn refit_maximized(_was: (i32, i32, i32, i32), _now: (i32, i32, i32, i32)) -> usize {
        0
    }
}

// Whether a window's rect (x, y, w, h) lies over a work area as a maximized
// one does: past each of its edges by its frame, no more (40 px at most:
// 12 at 175%).
fn lies_over(r: (i32, i32, i32, i32), work: (i32, i32, i32, i32)) -> bool {
    let past = |d: i32| (0..=40).contains(&d);
    past(work.0 - r.0) && past(work.1 - r.1) && past((r.0 + r.2) - (work.0 + work.2)) && past((r.1 + r.3) - (work.1 + work.3))
}

// Where a maximized window at `at` goes as its display's work area moves
// from `was` to `now`: each side as far as the work area's, if it lay over
// the old one and not yet over the new; None to leave it be. One over all of
// its display (`whole`) is full screen, not laid on a work area: a borderless
// app's full screen, a video's, a browser's F11 may keep the maximized state,
// and over a work area that was the whole display (Windows' own set to hide
// itself, or the work area put back so by Explorer while her strip is along
// a side) it was squeezed into the new one, full screen no more.
fn refit(at: (i32, i32, i32, i32), was: (i32, i32, i32, i32), now: (i32, i32, i32, i32), whole: (i32, i32, i32, i32)) -> Option<(i32, i32, i32, i32)> {
    if at == whole || lies_over(at, now) || !lies_over(at, was) {
        return None;
    }
    Some(moved_with(at, was, now))
}

// A rect with each side moved as far as the work area's side moved.
fn moved_with(r: (i32, i32, i32, i32), was: (i32, i32, i32, i32), now: (i32, i32, i32, i32)) -> (i32, i32, i32, i32) {
    let l = r.0 + (now.0 - was.0);
    let t = r.1 + (now.1 - was.1);
    let rt = r.0 + r.2 + ((now.0 + now.2) - (was.0 + was.2));
    let b = r.1 + r.3 + ((now.1 + now.3) - (was.1 + was.3));
    (l, t, rt - l, b - t)
}

// What a window that hears Windows about its bar uses (examples/taskbar_spike.rs
// does; her home's window, a webview's, does not hear it).
#[allow(unused_imports)]
pub use imp::{query, ABN_FULLSCREENAPP, ABN_POSCHANGED, ABN_STATECHANGE, CALLBACK};
pub use imp::{place, refit_maximized, register, remove, set_taskbar_state, set_work, taskbar_state, work_of, ABS_AUTOHIDE};

#[cfg(all(test, windows, target_pointer_width = "64"))]
mod tests {
    #[test]
    fn the_struct_is_the_size_windows_expects() {
        // APPBARDATA on 64-bit Windows: 48 bytes.
        assert_eq!(std::mem::size_of::<super::imp::AppBarData>(), 48);
    }
}

#[cfg(test)]
mod edge_tests {
    use super::Edge;

    #[test]
    fn a_strip_along_each_edge() {
        let mon = (0, 0, 2560, 1440);
        assert_eq!(Edge::of("bottom").strip(mon, 72), (0, 1368, 2560, 72));
        assert_eq!(Edge::of("top").strip(mon, 72), (0, 0, 2560, 72));
        assert_eq!(Edge::of("left").strip(mon, 72), (0, 0, 72, 1440));
        assert_eq!(Edge::of("right").strip((-1920, 0, 1920, 1080), 48), (-48, 0, 48, 1080));
        assert_eq!(Edge::of("nope"), Edge::Bottom);
        assert!(Edge::Left.is_side() && Edge::Right.is_side() && !Edge::Top.is_side());
    }

    #[test]
    fn a_work_area_against_a_strip_on_another_display() {
        // A display right of the main one at 175%, its strip 84 px thick.
        let mon = (2560, -149, 2560, 1600);
        let left = Edge::Left.strip(mon, 84);
        // The whole display: it reaches under a strip along the left; made to stop at its inner side.
        assert!(Edge::Left.reaches(mon, left));
        assert_eq!(Edge::Left.with_side(mon, Edge::Left.inner(left)), (2644, -149, 2476, 1600));
        // Once made so, it does not.
        assert!(!Edge::Left.reaches((2644, -149, 2476, 1600), left));
        // Along the bottom, Windows' own room there (84 px) already keeps it clear.
        let bottom = Edge::Bottom.strip(mon, 84);
        assert_eq!(Edge::Bottom.inner(bottom), 1367);
        assert!(!Edge::Bottom.reaches((2560, -149, 2560, 1516), bottom));
        assert!(Edge::Bottom.reaches(mon, bottom));
        // Along the top and the right.
        assert_eq!(Edge::Top.with_side(mon, Edge::Top.inner(Edge::Top.strip(mon, 84))), (2560, -65, 2560, 1516));
        assert_eq!(Edge::Right.with_side(mon, Edge::Right.inner(Edge::Right.strip(mon, 84))), (2560, -149, 2476, 1600));
        assert_eq!(Edge::Right.side_of(mon), 5120);
    }

    #[test]
    fn a_strip_told_before_the_display_changed_lies_off_its_edge() {
        // Told at 1920×1080, the display 2560×1440 again (2026-10-26).
        let old = Edge::Bottom.strip((0, 0, 1920, 1080), 48);
        let mon = (0, 0, 2560, 1440);
        assert!(!Edge::Bottom.lies_along(old, mon));
        assert!(Edge::Bottom.lies_along(Edge::Bottom.strip(mon, 48), mon));
        // Along the right, the display wider now; along the left, its x the same.
        assert!(!Edge::Right.lies_along(Edge::Right.strip((0, 0, 1920, 1080), 48), mon));
        assert!(Edge::Left.lies_along(Edge::Left.strip((0, 0, 1920, 1080), 48), mon));
        // On another display, left of the main one.
        let side = (-1920, 0, 1920, 1200);
        assert!(Edge::Top.lies_along(Edge::Top.strip(side, 60), side));
        assert!(!Edge::Bottom.lies_along(Edge::Bottom.strip((-1920, 0, 1920, 1080), 48), side));
    }

    #[test]
    fn the_work_area_follows_the_strip_it_was_set_for() {
        let mon = (0, 0, 2560, 1440);
        // At 125% the strip 60 px, at 100% 48: Windows' room moved to 1392,
        // the side set for the old strip (1380) stayed.
        let now = Edge::Bottom.strip(mon, 48);
        assert!(Edge::Bottom.short_of((0, 0, 2560, 1380), now));
        assert_eq!(Edge::Bottom.work_fix((0, 0, 2560, 1380), now, Some(1380)), Some(1392));
        // Short of it at a side not set by her (another bar's room): left be.
        assert_eq!(Edge::Bottom.work_fix((0, 0, 2560, 1380), now, None), None);
        assert_eq!(Edge::Bottom.work_fix((0, 0, 2560, 1380), now, Some(1032)), None);
        // Reaching into it: set, whoever set it.
        assert_eq!(Edge::Bottom.work_fix(mon, now, None), Some(1392));
        // Where it should be: left be.
        assert_eq!(Edge::Bottom.work_fix((0, 0, 2560, 1392), now, Some(1380)), None);
        // Along the left, a thinner strip now: the side moves in.
        let left = Edge::Left.strip(mon, 48);
        assert!(Edge::Left.short_of((60, 0, 2500, 1440), left));
        assert_eq!(Edge::Left.work_fix((60, 0, 2500, 1440), left, Some(60)), Some(48));
        // Along the top and the right.
        assert_eq!(Edge::Top.work_fix((0, 60, 2560, 1380), Edge::Top.strip(mon, 48), Some(60)), Some(48));
        assert_eq!(Edge::Right.work_fix((0, 0, 2500, 1440), Edge::Right.strip(mon, 48), Some(2500)), Some(2512));
    }

    #[test]
    fn a_side_an_earlier_strip_held_in_is_given_back() {
        // The main display at 125%, her strip along the left (60 px); Explorer
        // kept the right's 60 px from when it was along the right.
        let mon = (0, 0, 2560, 1440);
        let left = Edge::Left.strip(mon, 60);
        assert_eq!(Edge::Left.stale_sides((60, 0, 2440, 1440), left, mon), vec![(Edge::Right, 2560)]);
        // Only the strip's own side held in: nothing else to give back.
        assert!(Edge::Left.stale_sides((60, 0, 2500, 1440), left, mon).is_empty());
        // Another bar's side of another thickness is not hers: left be.
        assert!(Edge::Left.stale_sides((60, 0, 2400, 1440), left, mon).is_empty());
        // Along the top, the left's 60 px kept from before.
        assert_eq!(Edge::Top.stale_sides((60, 60, 2500, 1380), Edge::Top.strip(mon, 60), mon), vec![(Edge::Left, 0)]);
        // Along the bottom, none (the room there is Windows' own).
        assert!(Edge::Bottom.stale_sides((60, 0, 2500, 1380), Edge::Bottom.strip(mon, 60), mon).is_empty());
    }

    #[test]
    fn a_maximized_window_moved_with_the_work_area() {
        use super::{lies_over, moved_with};
        // Maximized on the display at 175%, its frame 12 px past each edge of the work area.
        let was = (2560, -149, 2560, 1516);
        let win = (2548, -161, 2584, 1540);
        assert!(lies_over(win, was));
        // The strip along the left, the work area 84 px less there and the whole height.
        let now = (2644, -149, 2476, 1600);
        assert!(!lies_over(win, now));
        let to = moved_with(win, was, now);
        assert_eq!(to, (2632, -161, 2500, 1624));
        assert!(lies_over(to, now));
        // A window laid otherwise (restored, half the screen) is not over either.
        assert!(!lies_over((2560, -149, 1280, 800), was));
    }

    #[test]
    fn a_full_screen_window_is_left_be() {
        use super::refit;
        let mon = (0, 0, 2560, 1440);
        // Windows' own set to hide itself: the work area the whole display,
        // her strip then along the bottom.
        let (was, now) = (mon, (0, 0, 2560, 1380));
        // Maximized with its frame past the edges: laid on the new one.
        assert_eq!(refit((-9, -9, 2578, 1458), was, now, mon), Some((-9, -9, 2578, 1398)));
        // Full screen and maximized (F11 from a maximized browser, a
        // borderless app's full screen): over all of the display, left be.
        assert_eq!(refit(mon, was, now, mon), None);
        // As it was before: from Windows' room to the strip along the left.
        let (was, now) = ((0, 0, 2560, 1380), (60, 0, 2500, 1440));
        assert_eq!(refit((-9, -9, 2578, 1398), was, now, mon), Some((51, -9, 2518, 1458)));
        assert_eq!(refit(mon, was, now, mon), None);
        // Already over the new one: left be.
        assert_eq!(refit((51, -9, 2518, 1458), was, now, mon), None);
    }
}
