// A bar's room on the screen: registered with Windows as an app bar, as the
// taskbar is, so the work area loses its height and maximized windows stop
// at it. The top bar (display 'bar') takes a strip along the top; the taskbar
// (display 'taskbar') one along the bottom, Windows' own being put away
// (shell.rs, which reads and sets its state here). Given back when the bar
// goes (another home, out of sight, quitting). Windows only.

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Edge {
    Top,
    Bottom,
}

#[cfg(windows)]
mod imp {
    use super::Edge;

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

    const ABM_NEW: u32 = 0;
    const ABM_REMOVE: u32 = 1;
    const ABM_QUERYPOS: u32 = 2;
    const ABM_SETPOS: u32 = 3;
    const ABM_GETSTATE: u32 = 4;
    const ABM_SETSTATE: u32 = 10;
    const ABE_TOP: u32 = 1;
    const ABE_BOTTOM: u32 = 3;
    // What Windows sends a bar about changes, as the message's wParam (the top
    // bar does not listen: it asks again whenever the screens change, see
    // island.rs). For ABN_FULLSCREENAPP, lParam is 1 when a full-screen
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
        let (left, right) = (mon.0, mon.0 + mon.2);
        d.rc = match edge {
            Edge::Top => Rect { left, top: mon.1, right, bottom: mon.1 + thickness },
            Edge::Bottom => Rect { left, top: mon.1 + mon.3 - thickness, right, bottom: mon.1 + mon.3 },
        };
        // SAFETY: as above.
        unsafe { SHAppBarMessage(ABM_QUERYPOS, &mut d) };
        // The edge may have moved in past another bar: the thickness stays ours.
        match edge {
            Edge::Top => d.rc.bottom = d.rc.top + thickness,
            Edge::Bottom => d.rc.top = d.rc.bottom - thickness,
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
        match edge {
            Edge::Top => (mon.0, mon.1, mon.2, thickness),
            Edge::Bottom => (mon.0, mon.1 + mon.3 - thickness, mon.2, thickness),
        }
    }
    pub fn query(hwnd: isize, mon: (i32, i32, i32, i32), edge: Edge, thickness: i32) -> (i32, i32, i32, i32) {
        place(hwnd, mon, edge, thickness)
    }
    pub fn taskbar_state() -> u32 {
        0
    }
    pub fn set_taskbar_state(_taskbar: isize, _state: u32) {}
}

// What a window that hears Windows about its bar uses (examples/taskbar_spike.rs
// does; her home's window, a webview's, does not hear it).
#[allow(unused_imports)]
pub use imp::{query, ABN_FULLSCREENAPP, ABN_POSCHANGED, ABN_STATECHANGE, CALLBACK};
pub use imp::{place, register, remove, set_taskbar_state, taskbar_state, ABS_AUTOHIDE};

#[cfg(all(test, windows, target_pointer_width = "64"))]
mod tests {
    #[test]
    fn the_struct_is_the_size_windows_expects() {
        // APPBARDATA on 64-bit Windows: 48 bytes.
        assert_eq!(std::mem::size_of::<super::imp::AppBarData>(), 48);
    }
}
