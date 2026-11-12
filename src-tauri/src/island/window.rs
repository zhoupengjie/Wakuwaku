// The island's window: laid, made, and how it looks over a maximized one.
use super::*;

// The window laid on its rect (physical), in steps (lay_steps): sized down
// first where it is wider or taller than it is going to be (so sized it
// lies within its old rect still, and within its new one once moved: at
// no step does it reach past the two), then its place, its size, and its
// place again, as bars.rs lay does. Moved first, at its old size, the
// taskbar's window reached past its display: from along the bottom to
// along the right, still as wide as the display, its right end lay over
// the display to the right by the display's width less the side bar's,
// and on a display wider than twice the side bar (2560 px at 100 %, 4K at
// 150 %, an ultrawide) more of it lay there than here, so Windows took it
// for that display's and, that display at another scale, told it so: tao
// then put it on that display at the size Windows suggested, till the look
// (keep_on_screen) put it back (the user's report, 2026-11-07: set to the
// right, her taskbar flew over to the other display; on this machine, at
// 125 %, the window laid by the old strip meanwhile, 1285 x 60 at the
// bottom right, went to the display at 150 % for a moment: sized by
// Windows to 1071 x 50, the page's scale 1.5 and back). Sized first and then
// moved to a display of another scale, Windows gave it the size for that
// one (the scales' ratio over it): an external display at 125% plugged back
// in and made the main one while it sat on the laptop's at 175%, her taskbar
// came out 1830 x 615 for 2560 x 860, the strip floating up the screen and
// short of its end, until she was restarted (zhoupengjie/Wakuwaku#37); so
// its size is set again after the move. Never while minimized
// (keep_on_screen lays it once it is given back).
pub(super) fn lay(win: &WebviewWindow, (x, y): (i32, i32), (w, h): (i32, i32)) {
    // Gone with Explorer's taskbar (embed.rs, which starts her anew): tao's
    // set_size on a window that is no more panics (see window).
    if !alive(win) {
        return;
    }
    if is_minimized(win) {
        return;
    }
    // A child of Windows' taskbar (embed.rs): laid in its client area, not
    // on the screen as tao would.
    if crate::embed::is_parented() {
        crate::embed::lay_child((x, y), (w, h));
        return;
    }
    for step in lay_steps(current_size(win), (w, h)) {
        match step {
            Step::Pos => {
                let _ = win.set_position(PhysicalPosition::new(x, y));
            }
            Step::Size(sw, sh) => {
                let _ = win.set_size(PhysicalSize::new(sw.max(1) as u32, sh.max(1) as u32));
            }
        }
    }
}

// A step of lay: the window's place, or a size (physical).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Step {
    Pos,
    Size(i32, i32),
}

// The steps that lay a window at a size, from its size now (None: not
// known): down to no more than the new size either way first, where it is
// more, then its place, the new size, and its place again.
pub(super) fn lay_steps(now: Option<(i32, i32)>, (w, h): (i32, i32)) -> Vec<Step> {
    let mut steps = Vec::new();
    if let Some((nw, nh)) = now {
        let within = (nw.min(w), nh.min(h));
        if within != (nw, nh) {
            steps.push(Step::Size(within.0, within.1));
        }
    }
    steps.extend([Step::Pos, Step::Size(w, h), Step::Pos]);
    steps
}

// The window's size now (physical), asked of Windows directly (frameless.rs
// rect, on any thread); None where it cannot be asked.
pub(super) fn current_size(_win: &WebviewWindow) -> Option<(i32, i32)> {
    #[cfg(windows)]
    if let Some(h) = crate::main_wait::hwnd(&_win) {
        return crate::frameless::rect(h).map(|r| (r.2, r.3));
    }
    None
}

// Laid again where it is meant to be (a change of scale under it).
pub(super) fn lay_again(sh: &Shared, why: &str) {
    let (pos, size, bar) = {
        let isl = sh.island.lock().unwrap();
        (isl.pos, isl.size, isl.bar)
    };
    // The display it is meant for: the one her strip lies on, or the
    // island's middle; none between strips (a display gone, the new one not
    // yet taken by the taskbar's thread).
    let areas = sh.screens().areas;
    let on = |r: (i32, i32, i32, i32)| areas.iter().find(|a| r.0 >= a.mon.0 && r.1 >= a.mon.1 && r.0 + r.2 <= a.mon.0 + a.mon.2 && r.1 + r.3 <= a.mon.1 + a.mon.3);
    let meant = match bar {
        _ if sh.home() != "taskbar" => on((pos.0 + size.0 / 2, pos.1 + size.1 / 2, 1, 1)),
        Some(b) => on(b),
        None => None,
    };
    let Some(meant) = meant else { return };
    let Some(win) = window(sh) else { return };
    // Laid again only once it has that display's scale: of another (Windows
    // moving it off a display unplugged, the list of displays not yet
    // without it), its place is still to come, and laid now it went back
    // on the display going away for a moment.
    if crate::main_wait::scale_factor(&win).is_some_and(|sf| (sf - meant.sf).abs() > 0.01) {
        return;
    }
    let now = crate::main_wait::outer_size(&win).map(|s| (s.width as i32, s.height as i32)).unwrap_or(size);
    if now != size {
        sh.log(&format!("island: sized by Windows to {}x{} ({why}); laid again at {pos:?} {size:?} on {:?} at {}", now.0, now.1, meant.mon, meant.sf));
        lay(&win, pos, size);
    }
}

pub(super) fn place(sh: &Shared) {
    sync_bar(sh, true);
    let fitted = fit(sh);
    // Its home changed, perhaps (the taskbar's zoom, or none).
    fit_zoom(sh);
    tell_strip(sh, false);
    // Split or no longer: the dock comes or goes with the strip (bars.rs).
    crate::bars::sync(&crate::shared(&sh.app));
    let Some((pos, size)) = fitted else { return };
    if let Some(win) = window(sh) {
        lay(&win, pos, size);
    }
    // On another display, another notch (or none).
    crate::notch::settle(sh);
}

// Made the first time her home shows, then kept (hidden while she talks
// herself), so coming back is instant.
pub(super) fn create(sh: &Arc<Shared>) {
    fit(sh);
    let (pos, size, sf) = {
        let mut isl = sh.island.lock().unwrap();
        if isl.created {
            return;
        }
        isl.created = true;
        isl.ct.force(Mode::Pass);
        (isl.pos, isl.size, isl.sf())
    };
    // Built off the main thread: a menu click (main thread) may be what asks
    // for it, and building there waits on itself.
    let sh = sh.clone();
    std::thread::spawn(move || {
        crate::main_wait::check("building a window");
        let built = WebviewWindowBuilder::new(&sh.app, "island", WebviewUrl::App("pet/index.html?role=island".into()))
            .title("wakuwaku island")
            .inner_size(size.0 as f64 / sf, size.1 as f64 / sf)
            .additional_browser_args(crate::webview::browser_args(&sh))
            .data_directory(crate::webview::webview_dir(&sh))
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
                lay(&win, pos, size);
                let _ = win.set_ignore_cursor_events(true);
                crate::notch::settle(&sh);
                // A click anywhere else takes the focus from the settings: they close.
                let app = sh.app.clone();
                win.on_window_event(move |event| {
                    // Not when it is her own window being shown (Shared::show_window).
                    if let WindowEvent::Focused(false) = event {
                        if !crate::shared(&app).is_shuffling() {
                            let _ = app.emit_to("island", "island:blur", ());
                        }
                    }
                    // Its display's scale changed under it (a display plugged
                    // in or out, another made the main one): Windows sized it
                    // for the new scale; once it has, put where it belongs on
                    // the displays there are now, at its size (#37), not at
                    // the next look.
                    if let WindowEvent::ScaleFactorChanged { .. } = event {
                        let app = app.clone();
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            keep_on_screen(&crate::shared(&app));
                        });
                    }
                });
                #[cfg(windows)]
                if let Some(hwnd) = crate::main_wait::hwnd(&win) {
                    sh.island.lock().unwrap().hwnd = hwnd;
                    crate::tasks::no_transitions(hwnd);
                }
                sh.own_window(&win);
                // Its renderer gone: told, and the page loaded again (crash.rs).
                crate::crash::watch(&sh, &win);
                // Its plugins' frames kept to their own page (addons/ guard_frames).
                crate::addons::guard_frames(&sh, &win, true);
            }
            Err(err) => sh.log(&format!("island: could not make its window: {err}")),
        }
    });
}

// At the top of the screen, a see-through look (islandTopLook) that
// Windows' acrylic does not blur (none set under 90 %, or it cannot be:
// backplate.rs) deepens over a maximized window on her display (island.ts
// setDeep, zhoupengjie/Wakuwaku#27): looked at with the full screen's look
// (every 1.5 s, or a window to the front), told the page as it changes
// (island:over-max). Windows' mode with it: the page has it in pet:update
// alone there (no taskbar to hear it from), so a change of it draws anew.
// Nothing for the default black look, nor while she is quiet.
pub(super) static OVER_MAX: AtomicBool = AtomicBool::new(false);
pub(super) static TOP_LIGHT: AtomicU8 = AtomicU8::new(2);

pub fn look_over_max(sh: &Arc<Shared>) {
    let looks = sh.config().island_top_look.clone();
    if sh.home() == "taskbar" || sh.is_quiet() || !looks.as_object().is_some_and(|m| m.values().any(Value::is_object)) {
        return;
    }
    let light = crate::wallpaper::look().0;
    if TOP_LIGHT.swap(light as u8, Ordering::SeqCst) != light as u8 {
        sh.redraw();
    }
    let look = &looks[if light { "light" } else { "dark" }];
    let deepens = look["bg"].is_string() && look["op"].as_u64().is_some_and(|op| op < 90) && (look["blur"].as_u64().unwrap_or(0) == 0 || !crate::backplate::available());
    let on = deepens && display(sh).is_some_and(crate::tasks::maximized_on);
    if OVER_MAX.swap(on, Ordering::SeqCst) != on {
        let _ = sh.app.emit_to("island", "island:over-max", json!({ "on": on }));
    }
}
