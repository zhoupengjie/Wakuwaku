// What waits for the main thread: Tauri's window getters (where a window
// is, how big, its handle, the displays, the pointer), building a window,
// and the tray's and the menus' setters. Each sends the main thread a
// message and waits for its answer; the main thread, in a menu's or a
// window's callback, may be waiting on one of her locks meanwhile. A
// thread holding a lock and waiting here, while the main thread waits on
// that lock, hangs both (DEVELOPMENT.md, 踩过的坑: 锁和窗口 getter). So
// every such call goes through here, and none is made with a lock held:
// lock.rs's none_held says so (a test fails, a debug build says it in her
// log). On the main thread itself Tauri answers at once, without the
// wait, so there it is not looked at.
use tauri::{AppHandle, Monitor, PhysicalPosition, PhysicalSize, WebviewWindow};

// Called on the main thread as she starts (main.rs setup).
pub fn here() {
    crate::lock::main_is_this_thread();
}

// About to wait for the main thread over `what`: no lock held here.
#[cfg_attr(debug_assertions, track_caller)]
pub fn check(what: &str) {
    if !crate::lock::on_main() {
        crate::lock::none_held(what);
    }
}

// The window's handle (HWND), as a number. Tauri asks the main thread for
// it each time.
#[cfg(windows)]
#[cfg_attr(debug_assertions, track_caller)]
pub fn hwnd(win: &WebviewWindow) -> Option<isize> {
    check("hwnd");
    win.hwnd().ok().map(|h| h.0 as isize)
}

#[cfg_attr(debug_assertions, track_caller)]
pub fn outer_position(win: &WebviewWindow) -> Option<PhysicalPosition<i32>> {
    check("outer_position");
    win.outer_position().ok()
}

#[cfg_attr(debug_assertions, track_caller)]
pub fn outer_size(win: &WebviewWindow) -> Option<PhysicalSize<u32>> {
    check("outer_size");
    win.outer_size().ok()
}

#[cfg_attr(debug_assertions, track_caller)]
pub fn scale_factor(win: &WebviewWindow) -> Option<f64> {
    check("scale_factor");
    win.scale_factor().ok()
}

#[cfg_attr(debug_assertions, track_caller)]
pub fn monitors(app: &AppHandle) -> Vec<Monitor> {
    check("available_monitors");
    app.available_monitors().unwrap_or_default()
}

#[cfg_attr(debug_assertions, track_caller)]
pub fn primary_monitor(app: &AppHandle) -> Option<Monitor> {
    check("primary_monitor");
    app.primary_monitor().ok().flatten()
}

#[cfg_attr(debug_assertions, track_caller)]
pub fn cursor(app: &AppHandle) -> Option<PhysicalPosition<f64>> {
    check("cursor_position");
    app.cursor_position().ok()
}
