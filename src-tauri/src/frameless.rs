// Her windows with no frame left to draw. tao makes every window with
// WS_CAPTION in its style, decorated or not, and takes the frame away only by
// its answer to WM_NCCALCSIZE (the client area all of the window). Hers are
// layered as well (click-through: tao's WS_EX_LAYERED | WS_EX_TRANSPARENT), and
// a layered window's frame is not DWM's to draw: when another program calls
// PrintWindow on the active one, a WM_NCPAINT(1) comes inside the WM_PAINT it
// asks for, and DefWindowProc draws the Basic frame of old on the window's own
// surface: title bar, icon, close button and all, over the whole window.
// Nothing paints that surface again (her page is WebView2's, above it), so it
// stays until she quits, and it goes along with her. A Windhawk mod did it
// (macos-minimize-animation: a snapshot of every new window with a title bar,
// as it first shows); docs/DEVELOPMENT.md, 踩过的坑, has the rest.
//
// So the style goes, and stays gone: tao writes its own back at every change of
// its flags (each click-through, each show and hide), which WM_STYLECHANGING
// lets us correct as it is asked.
//
// Minimized, a window is all frame, title bar or none in its style, and the
// same Basic one is drawn on that surface; restored at the same size, it stays
// there (her island in the taskbar home, 2560 x 860: a bar across the screen
// with "wakuwaku island" and a close button, 2026-11-03, minimized by Claude's
// computer use and laid again by her look). Minimized, then, no frame is
// drawn: no WM_NCPAINT, and WM_NCACTIVATE's own drawing waved off (lparam -1,
// as Windows lets one).
use std::ffi::c_void;

type Hwnd = *mut c_void;
type Subclass = unsafe extern "system" fn(Hwnd, u32, usize, isize, usize, usize) -> isize;

const GWL_STYLE: i32 = -16;
// Two bits, WS_BORDER and WS_DLGFRAME: a title bar is both.
const WS_CAPTION: u32 = 0x00c0_0000;
const WM_NCDESTROY: u32 = 0x0082;
const WM_STYLECHANGING: u32 = 0x007c;
const WM_NCPAINT: u32 = 0x0085;
const WM_NCACTIVATE: u32 = 0x0086;
const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOACTIVATE: u32 = 0x0010;
const SWP_FRAMECHANGED: u32 = 0x0020;
const SUBCLASS_ID: usize = 1;

#[repr(C)]
struct StyleStruct {
    old: u32,
    new: u32,
}

#[link(name = "comctl32")]
extern "system" {
    fn SetWindowSubclass(hwnd: Hwnd, proc: Subclass, id: usize, data: usize) -> i32;
    fn RemoveWindowSubclass(hwnd: Hwnd, proc: Subclass, id: usize) -> i32;
    fn DefSubclassProc(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize;
}

#[link(name = "user32")]
extern "system" {
    fn GetWindowLongPtrW(hwnd: Hwnd, index: i32) -> isize;
    fn SetWindowLongPtrW(hwnd: Hwnd, index: i32, value: isize) -> isize;
    fn SetWindowPos(hwnd: Hwnd, after: isize, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
    fn IsIconic(hwnd: Hwnd) -> i32;
}

fn without_title_bar(style: u32) -> u32 {
    style & !WS_CAPTION
}

unsafe extern "system" fn hold_off_title_bar(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize, _id: usize, _data: usize) -> isize {
    if msg == WM_STYLECHANGING && wparam as i32 == GWL_STYLE {
        // SAFETY: for this message, lparam is Windows' STYLESTRUCT, ours to change.
        let change = &mut *(lparam as *mut StyleStruct);
        change.new = without_title_bar(change.new);
    } else if msg == WM_NCPAINT && IsIconic(hwnd) != 0 {
        return 0;
    } else if msg == WM_NCACTIVATE && IsIconic(hwnd) != 0 {
        return DefSubclassProc(hwnd, msg, wparam, -1);
    } else if msg == WM_NCDESTROY {
        RemoveWindowSubclass(hwnd, hold_off_title_bar, SUBCLASS_ID);
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}

// Whether a window of hers is minimized: island.rs and pet.rs leave it so,
// never laying it again till it is given back.
pub fn minimized(hwnd: isize) -> bool {
    // SAFETY: a plain call about a window; a stale handle answers no.
    unsafe { IsIconic(hwnd as Hwnd) != 0 }
}

// Before the window first shows. On its own thread, as a subclass must be.
pub fn keep(hwnd: isize) {
    let hwnd = hwnd as Hwnd;
    // SAFETY: plain calls about our own window; a stale handle only makes them fail.
    unsafe {
        if SetWindowSubclass(hwnd, hold_off_title_bar, SUBCLASS_ID, 0) == 0 {
            return;
        }
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        SetWindowLongPtrW(hwnd, GWL_STYLE, without_title_bar(style) as isize);
        SetWindowPos(hwnd, 0, 0, 0, 0, 0, SWP_NOSIZE | SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_title_bar_goes() {
        // The style of her window as tao leaves it, hidden and shown.
        assert_eq!(without_title_bar(0x04c8_0000), 0x0408_0000);
        assert_eq!(without_title_bar(0x14c8_0000), 0x1408_0000);
        // Neither of its two bits stays alone.
        assert_eq!(without_title_bar(WS_CAPTION), 0);
        assert_eq!(without_title_bar(0x0080_0000), 0);
        assert_eq!(without_title_bar(0x0040_0000), 0);
    }
}
