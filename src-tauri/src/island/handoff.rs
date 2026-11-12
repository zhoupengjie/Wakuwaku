// The handover with Windows' taskbar, and the window kept on its screen.
use super::*;

// --- The handover with Windows' taskbar (handoff.ts) ----------------------------------
// Windows' taskbar never gone for a moment. Taking over: a picture of it
// laid over ours, shown exactly over it, before it is put away; then its
// picture turns to dust that settles into ours. Quitting: ours turns to
// dust, Windows' shown again under it, ours fading over it before she
// goes. Each answer of the page's (covered, bare, done) waited for so long
// at most: neither side can hold the other. Not on the main thread: its
// events reach the page through the main thread.
pub(super) static HANDOFF: Lock<Vec<String>> = Lock::new("island::handoff::HANDOFF", Vec::new());
pub(super) static HANDOFF_SAID: std::sync::Condvar = std::sync::Condvar::new();

// What the page answers (taskbar_handoff).
pub fn handoff_answer(what: &str) {
    HANDOFF.lock().unwrap().push(what.to_string());
    HANDOFF_SAID.notify_all();
}

// A step told the page; its answers from before forgotten.
pub(super) fn handoff_ask(sh: &Shared, step: Value) {
    HANDOFF.lock().unwrap().clear();
    let _ = sh.app.emit_to("island", "taskbar:handoff", step);
}

// An answer, within ms at most: true once it came.
pub(super) fn handoff_wait(what: &str, ms: u64) -> bool {
    let said = HANDOFF.lock().unwrap();
    let (said, _) = said.wait_timeout_while(&HANDOFF_SAID, std::time::Duration::from_millis(ms), |s| !s.iter().any(|x| x == what)).unwrap();
    said.iter().any(|x| x == what)
}

// Her page ready, the taskbar to be taken (not yet up, nothing full screen,
// no other copy of hers holding it): Windows' taskbar pictured, her window
// shown exactly over it with the picture laid on, before it is put away.
// True once covered: then `go` follows the taking. A picture that cannot
// be had (auto-hidden, already put away) is none: the dust comes from above.
pub(super) fn handoff_cover(sh: &Arc<Shared>) -> bool {
    if sh.home() != "taskbar" || taskbar::is_up() {
        return false;
    }
    taskbar::look_for_other(sh);
    if !is_shown(sh) {
        return false;
    }
    let Some(area) = area(sh) else { return false };
    // Windows' strip pictured only where hers takes its place: along the
    // bottom. Along another edge none (the dust gathers from above).
    let picture = (edge(sh) == Edge::Bottom).then(|| taskbar::strip_picture(area.mon)).flatten();
    let _ = fit(sh);
    let (pos, size, hwnd) = {
        let mut isl = sh.island.lock().unwrap();
        isl.shown = true;
        (isl.pos, isl.size, isl.hwnd)
    };
    let Some(win) = window(sh) else { return false };
    lay(&win, pos, size);
    let _ = win.show();
    let shown = std::time::Instant::now();
    // Over Windows' own, which is kept on top too.
    crate::tasks::raise_top(hwnd);
    let pictured = picture.is_some();
    // Its window's size (physical) too: the page lays the picture only once
    // its own size has come to that (a window made bigger reaches the page
    // some frames later; laid before, it sat too small, up and to the left).
    handoff_ask(sh, json!({ "step": "in", "picture": picture, "size": [size.0, size.1] }));
    let covered = handoff_wait("covered", 800);
    // Drawn by the page is not yet on the screen: a window just shown hands
    // WebView2's frames to DWM some 40 ms later, and Windows' strip put away
    // at once left the bare desktop under the strip that long (whatever the
    // material: the test session's films, 2026-10-13). So two of DWM's passes
    // waited for, and 150 ms since the window showed at least; the picture is
    // Windows' strip itself, so the wait cannot be seen.
    if pictured {
        crate::tasks::dwm_flush();
        crate::tasks::dwm_flush();
        let least = std::time::Duration::from_millis(150);
        if let Some(rest) = least.checked_sub(shown.elapsed()) {
            std::thread::sleep(rest);
        }
    }
    sh.log(&format!(
        "handoff: in ({}), {} ({} ms after the window showed)",
        if pictured { "pictured" } else { "no picture" },
        if covered { "covered" } else { "no answer in 800 ms" },
        shown.elapsed().as_millis()
    ));
    true
}

// Quitting with the taskbar up (and nothing full screen): hers turns to
// dust, Windows' shown under it once the page says so (bare), hers kept
// over it while it fades (done). Then she goes, and gives it back as ever.
pub fn handoff_out(sh: &Shared) {
    let shown = sh.island.lock().unwrap().shown;
    if sh.home() != "taskbar" || !taskbar::is_up() || !shown || sh.is_fullscreen() {
        return;
    }
    handoff_ask(sh, json!({ "step": "out" }));
    let bare = handoff_wait("bare", 500);
    taskbar::handing_back();
    crate::shell::show_bars();
    crate::tasks::raise_top(hwnd(sh));
    let done = bare && handoff_wait("done", 1500);
    sh.log(&format!("handoff: out, {}, {}", if bare { "bare" } else { "no answer in 500 ms" }, if done { "done" } else { "not waited for" }));
}

// Minimized by another program. Claude's computer use hides every window of
// a program it was not given as it starts, hers too, and gives them back as
// its turn ends: taken for Windows moving it, hers was laid again at its size
// every 2 s, minimized still, for 35 min, a bar across the screen with its
// name and a close button left on it after (2026-11-03; frameless.rs). Left
// so now, as the other program wants it, till it is given back.
pub(super) static MINIMIZED: AtomicBool = AtomicBool::new(false);

pub(super) fn is_minimized(_win: &WebviewWindow) -> bool {
    #[cfg(windows)]
    if let Some(h) = crate::main_wait::hwnd(&_win) {
        return crate::frameless::minimized(h);
    }
    false
}

// Kept in its place (a display gone, the scale changed, another bar). And
// put back where it was put if Windows moved it: the taskbar's window, which
// reaches over its own strip, was pushed up into the work area by the strip's
// height (2026-10-10, as a game went full screen and back). Not while it is
// minimized (MINIMIZED); given back, laid where it belongs at once.
pub fn keep_on_screen(sh: &Shared) {
    // A display come or gone: her taskbar on the others too (bars.rs).
    crate::bars::sync(&crate::shared(&sh.app));
    let (is_up, pos, size) = {
        let isl = sh.island.lock().unwrap();
        (isl.shown && !isl.holding && !isl.reaching, isl.pos, isl.size)
    };
    if !is_up {
        return;
    }
    let Some(win) = window(sh) else { return };
    let minimized = is_minimized(&win);
    let was = MINIMIZED.swap(minimized, Ordering::SeqCst);
    if minimized {
        if !was {
            sh.log(&format!("island: minimized by another program ({} in front); left so till it is given back", crate::fullscreen::front()));
        }
        return;
    }
    let given_back = was;
    // Where it belongs now (a display come or gone moves it), then its size
    // as Windows left it: place compares only with what it asked for last,
    // the same, and would never set it again (#37).
    place(sh);
    if given_back {
        let (pos, size) = {
            let isl = sh.island.lock().unwrap();
            (isl.pos, isl.size)
        };
        sh.log(&format!("island: given back after it was minimized; laid at {pos:?} {size:?}"));
        lay(&win, pos, size);
        // Any frame drawn on it meanwhile wiped (frameless.rs).
        #[cfg(windows)]
        if let Some(h) = crate::main_wait::hwnd(&win) {
            crate::frameless::wipe(h);
        }
    }
    lay_again(sh, "seen by the look");
    // Along the foot of the screen, over Windows' taskbar again should it have come over her (#105).
    uncover(sh);
    if sh.home() != "taskbar" {
        return;
    }
    if let Some(at) = crate::main_wait::outer_position(&win) {
        if (at.x, at.y) != pos && sh.island.lock().unwrap().pos == pos {
            sh.log(&format!("island: moved by Windows to ({}, {}); put back at {pos:?}", at.x, at.y));
            lay(&win, pos, size);
            // Windows took it for a window of its own: any frame it drew on it wiped (frameless.rs).
            #[cfg(windows)]
            if let Some(h) = crate::main_wait::hwnd(&win) {
                crate::frameless::wipe(h);
            }
        }
    }
}

