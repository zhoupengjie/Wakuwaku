// The top bar's room on the screen (display 'bar'): registered with Windows
// as an app bar, as the taskbar is, so the work area loses its height and
// maximized windows start below it. Given back when the bar goes (another
// home, out of sight, quitting). Windows only.

#[cfg(windows)]
mod imp {
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
    const ABE_TOP: u32 = 1;
    // What Windows would send the bar about changes (it is not listened to:
    // the bar asks again whenever the screens change, see island.rs).
    const CALLBACK: u32 = 0x8000 + 0x57;

    fn data(hwnd: isize) -> AppBarData {
        AppBarData { size: std::mem::size_of::<AppBarData>() as u32, hwnd, callback: CALLBACK, edge: ABE_TOP, rc: Rect::default(), lparam: 0 }
    }

    pub fn register(hwnd: isize) -> bool {
        let mut d = data(hwnd);
        // SAFETY: a struct of the size the call asks for, about our own window.
        unsafe { SHAppBarMessage(ABM_NEW, &mut d) != 0 }
    }

    pub fn remove(hwnd: isize) {
        let mut d = data(hwnd);
        // SAFETY: as above.
        unsafe { SHAppBarMessage(ABM_REMOVE, &mut d) };
    }

    // A strip this high along the top of a display (x, y, w, h), as Windows
    // grants it (another bar may be there first): x, y, w, h.
    pub fn place_top(hwnd: isize, mon: (i32, i32, i32, i32), height: i32) -> (i32, i32, i32, i32) {
        let mut d = data(hwnd);
        d.rc = Rect { left: mon.0, top: mon.1, right: mon.0 + mon.2, bottom: mon.1 + height };
        // SAFETY: as above.
        unsafe { SHAppBarMessage(ABM_QUERYPOS, &mut d) };
        // The top may have moved down past another bar: the height stays ours.
        d.rc.bottom = d.rc.top + height;
        // SAFETY: as above.
        unsafe { SHAppBarMessage(ABM_SETPOS, &mut d) };
        (d.rc.left, d.rc.top, d.rc.right - d.rc.left, d.rc.bottom - d.rc.top)
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn register(_hwnd: isize) -> bool {
        false
    }
    pub fn remove(_hwnd: isize) {}
    pub fn place_top(_hwnd: isize, mon: (i32, i32, i32, i32), height: i32) -> (i32, i32, i32, i32) {
        (mon.0, mon.1, mon.2, height)
    }
}

pub use imp::{place_top, register, remove};

#[cfg(all(test, windows, target_pointer_width = "64"))]
mod tests {
    #[test]
    fn the_struct_is_the_size_windows_expects() {
        // APPBARDATA on 64-bit Windows: 48 bytes.
        assert_eq!(std::mem::size_of::<super::imp::AppBarData>(), 48);
    }
}
