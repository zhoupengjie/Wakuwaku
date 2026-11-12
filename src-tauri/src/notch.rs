// Her island at the top of a Mac's screen, as the notch is: its window over
// the menu bar, and the band there (the notch's height, or the menu bar's on a
// display without one) and the notch's width told to the page (island:top),
// so the compact island wraps round the notch and nothing under the menu bar
// is covered. Elsewhere nothing: the island stays under the screen's top.

use crate::Shared;
use std::sync::Mutex;

// The band last told (height, notch width), so the log has a line only when
// it changes, and so it is known whether her island sits in a notch.
static TOLD: Mutex<Option<(f64, f64)>> = Mutex::new(None);

// Her island is in a notch, where it is out of the way (island.ts aside):
// her home there is out of sight, so she is never sent into it by a gesture
// that is easily made by mistake (a double-click, a drag towards it).
pub fn in_notch() -> bool {
    TOLD.lock().unwrap().is_some_and(|(_, notch)| notch > 0.0)
}

#[cfg(target_os = "macos")]
pub fn settle(sh: &Shared) {
    use serde_json::json;
    use tauri::{Emitter, Manager};

    let Some(win) = sh.app.get_webview_window("island") else { return };
    let sh = crate::shared(&sh.app);
    let w = win.clone();
    let _ = win.run_on_main_thread(move || {
        let Ok(ns) = w.ns_window() else { return };
        let (height, notch) = imp::raise(ns);
        if TOLD.lock().unwrap().replace((height, notch)) != Some((height, notch)) {
            sh.log(&format!("island: over the menu bar; the band at the screen's top {height} px, the notch {notch} px wide"));
        }
        let _ = sh.app.emit_to("island", "island:top", json!({ "height": height, "notch": notch }));
    });
}

#[cfg(not(target_os = "macos"))]
pub fn settle(_sh: &Shared) {}

// Where the island's window goes on a Mac: the middle of the display's top
// edge, the notch's place (physical px of the display (x, y, w, h)).
#[cfg(target_os = "macos")]
pub fn at_top(mon: (i32, i32, i32, i32), width: i32) -> Option<(i32, i32)> {
    Some((mon.0 + (mon.2 - width) / 2, mon.1))
}

#[cfg(not(target_os = "macos"))]
pub fn at_top(_mon: (i32, i32, i32, i32), _width: i32) -> Option<(i32, i32)> {
    None
}

#[cfg(target_os = "macos")]
mod imp {
    use objc2::runtime::AnyObject;
    use objc2::{msg_send, sel};
    use objc2_foundation::{NSEdgeInsets, NSRect};

    // Over the menu bar (NSMainMenuWindowLevel, 24): the status level, as the
    // menu bar's own extras and other notch apps are.
    const LEVEL: isize = 25;
    // The band's height when the screen says none (the menu bar hidden).
    const BAND: f64 = 32.0;

    // Raised over the menu bar; returns the band's height and the notch's
    // width (0 without one), in points: the page's px.
    pub fn raise(ns: *mut std::ffi::c_void) -> (f64, f64) {
        // SAFETY: Tauri's NSWindow for the island, on the main thread; the
        // screen's selectors asked first whether it answers them (macOS 12+).
        unsafe {
            let win = &*(ns as *mut AnyObject);
            let _: () = msg_send![win, setLevel: LEVEL];
            let screen: *mut AnyObject = msg_send![win, screen];
            let Some(screen) = screen.as_ref() else { return (BAND, 0.0) };
            let frame: NSRect = msg_send![screen, frame];
            let visible: NSRect = msg_send![screen, visibleFrame];
            let bar = (frame.origin.y + frame.size.height) - (visible.origin.y + visible.size.height);
            let knows: bool = msg_send![screen, respondsToSelector: sel!(auxiliaryTopLeftArea)];
            if knows {
                let insets: NSEdgeInsets = msg_send![screen, safeAreaInsets];
                let left: NSRect = msg_send![screen, auxiliaryTopLeftArea];
                let right: NSRect = msg_send![screen, auxiliaryTopRightArea];
                if insets.top > 0.0 && left.size.width > 0.0 && right.size.width > 0.0 {
                    return (insets.top, frame.size.width - left.size.width - right.size.width);
                }
            }
            (if bar > 0.0 { bar } else { BAND }, 0.0)
        }
    }
}
