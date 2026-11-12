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
// computer use and laid again by her look).
//
// And in 0.4.0 it was there again, along the top bar, the strip clear (the
// user, 2026-11-04: the island active for the settings as the strip moved to
// the top, then moved by Windows to the work area's top), though its style
// had no title bar. So no frame is ever drawn on hers, minimized or not, by
// any of the ways Windows has: WM_NCPAINT answered with nothing, WM_NCACTIVATE's
// own drawing waved off (lparam -1, as Windows lets one), the theme's caption
// and frame (WM_NCUAHDRAWCAPTION, WM_NCUAHDRAWFRAME, which a window's text or
// icon set brings) not drawn: they have no frame to show, the client area
// all of the window. And should one be drawn all the same, the surface is
// wiped (wipe: drawn anew, erased, the page above it left alone), as the
// window is given back after it was minimized, is activated or let go, and
// as Windows moves it (island.rs keep_on_screen).
//
// Nor are they in Alt+Tab: tao gives every window WS_EX_APPWINDOW and keeps a
// window off the taskbar by taking its button away (ITaskbarList DeleteTab),
// which Windows puts back once a hidden one shows again, and the dock was
// listed in Alt+Tab as "wakuwaku bar" (the user, 2026-11-04). Tool windows,
// as tao writes its own style back.
//
// And her capsule's window inside Windows' taskbar (embed.rs) is layered
// (WS_EX_LAYERED), or the taskbar's XAML takes every pointer meant for it:
// kept so the same way (keep_layered), as tao writes its style back.
use std::ffi::c_void;
use crate::lock::Lock;

type Hwnd = *mut c_void;
type Subclass = unsafe extern "system" fn(Hwnd, u32, usize, isize, usize, usize) -> isize;

const GWL_STYLE: i32 = -16;
const GWL_EXSTYLE: i32 = -20;
// Two bits, WS_BORDER and WS_DLGFRAME: a title bar is both.
const WS_CAPTION: u32 = 0x00c0_0000;
const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
const WS_EX_LAYERED: u32 = 0x0008_0000;
const WS_EX_APPWINDOW: u32 = 0x0004_0000;
const WM_SIZE: u32 = 0x0005;
const WM_ACTIVATE: u32 = 0x0006;
const WM_NCDESTROY: u32 = 0x0082;
const WM_STYLECHANGING: u32 = 0x007c;
const WM_NCPAINT: u32 = 0x0085;
const WM_NCACTIVATE: u32 = 0x0086;
// Undocumented, sent by the theme to draw a window's caption and frame.
const WM_NCUAHDRAWCAPTION: u32 = 0x00ae;
const WM_NCUAHDRAWFRAME: u32 = 0x00af;
const SIZE_MINIMIZED: usize = 1;
const RDW_INVALIDATE: u32 = 0x0001;
const RDW_ERASE: u32 = 0x0004;
const RDW_NOCHILDREN: u32 = 0x0040;
const RDW_ERASENOW: u32 = 0x0200;
const RDW_FRAME: u32 = 0x0400;
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
    fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
    fn RedrawWindow(hwnd: Hwnd, rect: *const c_void, region: isize, flags: u32) -> i32;
    fn RegisterWindowMessageW(name: *const u16) -> u32;
    fn SendMessageW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize;
}

// --- A job on a window's own thread ---------------------------------------------------
//
// Run inside the window's procedure (the subclass below, on the thread that
// has the window), the caller waiting for it (SendMessage; called on that
// thread, it runs at once). The Win32 calls a job makes on the window go
// through there one after another with nothing between them, where from
// another thread each is a message the window's thread answers in its
// turn, a frame or more apart while it is busy with her page: embed.rs
// takes the island's window into Windows' taskbar and out of it in one
// such job, so that DWM never draws a frame of it half done (the user,
// 2026-11-08: 展开后收缩到胶囊，胶囊会闪一下; caught on the screen: the
// capsule gone for a frame going in, for four going out). A window without
// the subclass (none of hers), or gone: the job runs on the caller's thread.
static RUN: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
// In wparam, so that nothing else sent the registered message is taken for a job.
const RUN_MARK: usize = 0x7761_6b75;

pub fn on_its_thread<T>(hwnd: isize, job: impl FnOnce() -> T) -> T {
    let msg = *RUN.get_or_init(|| {
        let name: Vec<u16> = "wakuwaku.frameless.run".encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: a name of ours, nul-terminated.
        unsafe { RegisterWindowMessageW(name.as_ptr()) }
    });
    let mut job = Some(job);
    let mut out = None;
    if msg != 0 {
        let mut call = || {
            if let Some(job) = job.take() {
                out = Some(job());
            }
        };
        let mut call: &mut dyn FnMut() = &mut call;
        // SAFETY: lparam points at `call` on this stack, which outlives the
        // call: SendMessage returns once the window's procedure has run it
        // (or not: a window without the subclass answers 0 and runs nothing).
        unsafe { SendMessageW(hwnd as Hwnd, msg, RUN_MARK, &mut call as *mut &mut dyn FnMut() as isize) };
    }
    match (out, job) {
        (Some(done), _) => done,
        (None, Some(job)) => job(),
        (None, None) => unreachable!("a job taken is run"),
    }
}

fn without_title_bar(style: u32) -> u32 {
    style & !WS_CAPTION
}

// Out of Alt+Tab (and off the taskbar for good): a tool window, not an app's.
fn as_tool_window(style_ex: u32) -> u32 {
    (style_ex & !WS_EX_APPWINDOW) | WS_EX_TOOLWINDOW
}

// Windows of hers minimized now, for their surface to be wiped once given back.
static MINIMIZED: Lock<Vec<isize>> = Lock::new("frameless::MINIMIZED", Vec::new());
// Windows of hers kept layered (embed.rs): the bit put back as tao writes
// its own extended style.
static LAYERED: Lock<Vec<isize>> = Lock::new("frameless::LAYERED", Vec::new());

fn kept_layered(hwnd: isize) -> bool {
    LAYERED.lock().unwrap_or_else(|e| e.into_inner()).contains(&hwnd)
}

// Her capsule's window in Windows' taskbar stays layered whatever tao writes back.
pub fn keep_layered(hwnd: isize) {
    let mut kept = LAYERED.lock().unwrap_or_else(|e| e.into_inner());
    if !kept.contains(&hwnd) {
        kept.push(hwnd);
    }
}

unsafe extern "system" fn hold_off_title_bar(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize, _id: usize, _data: usize) -> isize {
    if wparam == RUN_MARK && RUN.get() == Some(&msg) {
        // SAFETY: on_its_thread sent a pointer to its `&mut dyn FnMut()`,
        // alive on its stack while it waits for this answer.
        let call = &mut *(lparam as *mut &mut dyn FnMut());
        call();
        return 1;
    }
    match msg {
        WM_STYLECHANGING if wparam as i32 == GWL_STYLE => {
            // SAFETY: for this message, lparam is Windows' STYLESTRUCT, ours to change.
            let change = &mut *(lparam as *mut StyleStruct);
            change.new = without_title_bar(change.new);
        }
        WM_STYLECHANGING if wparam as i32 == GWL_EXSTYLE => {
            // SAFETY: as above.
            let change = &mut *(lparam as *mut StyleStruct);
            change.new = as_tool_window(change.new);
            if kept_layered(hwnd as isize) {
                change.new |= WS_EX_LAYERED;
            }
        }
        WM_NCPAINT | WM_NCUAHDRAWCAPTION | WM_NCUAHDRAWFRAME => return 0,
        WM_NCACTIVATE => return DefSubclassProc(hwnd, msg, wparam, -1),
        WM_SIZE => {
            let answer = DefSubclassProc(hwnd, msg, wparam, lparam);
            let mut minimized = MINIMIZED.lock().unwrap_or_else(|e| e.into_inner());
            if wparam == SIZE_MINIMIZED {
                if !minimized.contains(&(hwnd as isize)) {
                    minimized.push(hwnd as isize);
                }
            } else if let Some(at) = minimized.iter().position(|&h| h == hwnd as isize) {
                minimized.remove(at);
                drop(minimized);
                wipe(hwnd as isize);
            }
            return answer;
        }
        WM_ACTIVATE => {
            let answer = DefSubclassProc(hwnd, msg, wparam, lparam);
            wipe(hwnd as isize);
            return answer;
        }
        WM_NCDESTROY => {
            RemoveWindowSubclass(hwnd, hold_off_title_bar, SUBCLASS_ID);
            MINIMIZED.lock().unwrap_or_else(|e| e.into_inner()).retain(|&h| h != hwnd as isize);
            LAYERED.lock().unwrap_or_else(|e| e.into_inner()).retain(|&h| h != hwnd as isize);
        }
        _ => {}
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}

// Whatever was drawn on the window's own surface wiped: drawn anew and
// erased, its page (WebView2's, a child window above it) left as it is.
// Tried with a box painted on the island's surface from outside: it stayed,
// and this took it away.
pub fn wipe(hwnd: isize) {
    // SAFETY: a plain call about a window; a stale handle only makes it fail.
    unsafe { RedrawWindow(hwnd as Hwnd, std::ptr::null(), 0, RDW_INVALIDATE | RDW_ERASE | RDW_FRAME | RDW_ERASENOW | RDW_NOCHILDREN) };
}

// Whether a window of hers is minimized: island.rs and pet.rs leave it so,
// never laying it again till it is given back.
pub fn minimized(hwnd: isize) -> bool {
    // SAFETY: a plain call about a window; a stale handle answers no.
    unsafe { IsIconic(hwnd as Hwnd) != 0 }
}

#[repr(C)]
#[derive(Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

// Where a window of hers is and how big (physical: x, y, w, h), asked of
// Windows directly, on any thread (no wait on the main thread, as a window
// getter's): island.rs lay sizes it down before it moves it. None for a
// window gone.
pub fn rect(hwnd: isize) -> Option<(i32, i32, i32, i32)> {
    let mut r = Rect::default();
    // SAFETY: a plain query into our own struct; a stale handle fails it.
    if unsafe { GetWindowRect(hwnd as Hwnd, &mut r) } == 0 {
        return None;
    }
    Some((r.left, r.top, r.right - r.left, r.bottom - r.top))
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
        let style_ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, as_tool_window(style_ex) as isize);
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

    #[test]
    fn a_tool_window_and_no_apps() {
        // The dock's as tao left it (topmost, layered, click-through, an app's).
        assert_eq!(as_tool_window(0x080c_0038), 0x0808_00b8);
        // A tool window already stays one.
        assert_eq!(as_tool_window(0x0000_0080), 0x0000_0080);
    }
}
