// Out of the way of the window under it, and shown or hidden as she is.
use super::*;

// --- Out of the way of the window under it (zhoupengjie/Wakuwaku#49) ------------------
// At the top of the screen, the closed island lay over the window under it
// (a maximized browser's tab strip): a tab there was hard to reach or close,
// the island took the press. So, unless turned off (islandHideUnder), while
// a window lies under the closed island the page slides it up out of the
// way, a sliver left at the screen's edge and every press let through, as
// long as nothing wants the person (island-plate.ts setStow). Here: whether a
// window lies under it, looked at with the full screen's look (every 1.5 s,
// at once as the pointer comes near or its size changes); and the pointer
// calling it back down: resting at the screen's edge over it EDGE_MS with
// no button held (a press there is on the tab, as is one held, a tab or a
// window dragged up), or on the island. Once called it stays until the
// pointer has been away from about it (its band: the screen's top to a
// little below it) for LEAVE_MS.
pub(super) const EDGE_PX: f64 = 2.0;
pub(super) const EDGE_SIDE: f64 = 8.0;
pub(super) const EDGE_MS: u128 = 350;
pub(super) const BAND_SIDE: f64 = 24.0;
pub(super) const BAND_BELOW: f64 = 12.0;
pub(super) const LEAVE_MS: u128 = 600;

// The closed island on the screen (physical x, y, w, h), TOP below its
// window's top edge, or along the foot of the screen (#105) foot above the
// window's bottom edge: in the middle of the window's width, or at a
// corner (zhoupengjie/Wakuwaku#59) TOP in from the window's side there, as
// the page lays it out (anchor.ts, style.css isl-at-left / isl-at-right /
// isl-at-bottom); the stowed band and the edge that calls it down
// (call_look) follow from this. None before the page said its size, or in
// another home.
pub(super) fn closed_rect(isl: &Island) -> Option<(i32, i32, i32, i32)> {
    // Inside Windows' taskbar (embed.rs): the capsule itself, wherever this window is.
    if isl.embedded {
        return isl.cap;
    }
    let (w, h) = isl.stow.closed;
    if !isl.created || w <= 0.0 || h <= 0.0 {
        return None;
    }
    // In the page's px, by its scale (origin).
    let sf = isl.page_sf();
    let (w, h) = ((w * sf).round() as i32, (h * sf).round() as i32);
    let top = (TOP * sf).round() as i32;
    let x = match isl.at {
        Side::Left => isl.pos.0 + top,
        Side::Right => isl.pos.0 + isl.size.0 - w - top,
        Side::Center => isl.pos.0 + (isl.size.0 - w) / 2,
    };
    // Along the bottom foot up from the window's foot; else foot down from
    // its top (TOP with no taskbar's strip there, centred in one along the top).
    let y = if isl.bottom { isl.pos.1 + isl.size.1 - (isl.foot() * sf).round() as i32 - h } else { isl.pos.1 + (isl.foot() * sf).round() as i32 };
    Some((x, y, w, h))
}

// A window under the closed island, or none now: told the page as it
// changes (island:under).
pub fn look_under(sh: &Arc<Shared>) {
    // Kept as it was while it is not up, or nobody is about. Never along
    // the foot of the screen (#105): over Windows' taskbar, or on the work
    // area's foot, no window's top lies under the closed island.
    // The settings read before the island's lock: never two of Shared's at once.
    let asked = sh.home() == "island" && sh.config().island_hide_under;
    let (rect, wanted) = {
        let isl = sh.island.lock().unwrap();
        let wanted = asked && !isl.bottom && isl.strip.is_none();
        if wanted && (!isl.shown || sh.is_quiet()) {
            return;
        }
        (closed_rect(&isl), wanted)
    };
    let on = wanted && rect.is_some_and(crate::tasks::reaches_into);
    let called = {
        let mut isl = sh.island.lock().unwrap();
        if std::mem::replace(&mut isl.stow.under, on) == on {
            return;
        }
        // Nothing under it: nothing to call it down from.
        !on && std::mem::take(&mut isl.stow.called)
    };
    sh.log(&format!("island: {} {rect:?}", if on { "a window under it" } else { "no window under it now" }));
    let _ = sh.app.emit_to("island", "island:under", json!({ "on": on }));
    if called {
        let _ = sh.app.emit_to("island", "island:called", json!({ "on": false }));
    }
}

// The page's island at the top of the screen: up out of the way or not, and
// its closed size (logical). Of another size, looked at again at once.
pub fn set_stow(sh: &Shared, stowed: bool, width: f64, height: f64) {
    let resized = {
        let mut isl = sh.island.lock().unwrap();
        let s = &mut isl.stow;
        let size = (width.max(0.0), height.max(0.0));
        if stowed != s.stowed {
            sh.log(&format!("island: {}", if stowed { "up out of the way of the window under it" } else { "down again" }));
        }
        s.stowed = stowed;
        if !stowed {
            s.edge_since = None;
        }
        std::mem::replace(&mut s.closed, size) != size
    };
    if resized {
        crate::fullscreen::ask();
        // The capsule in Windows' taskbar laid again at its new size (embed.rs).
        crate::embed::look_soon();
    }
}

// One look at the pointer for it (poll; physical), at now; down: whether a
// button is held, asked only at the edge. Some(true) once it has called the
// island down, Some(false) once it has gone from it. Not while she is
// dragged past (passive), nor with no window under it.
pub(super) fn call_look(isl: &mut Island, cursor: (i32, i32), passive: bool, now: std::time::Instant, down: impl FnOnce() -> bool) -> Option<bool> {
    let rect = closed_rect(isl).filter(|_| isl.stow.under && !passive);
    let (sf, top, over) = (isl.sf(), isl.pos.1, isl.ct.over);
    let s = &mut isl.stow;
    let Some((x, y, w, h)) = rect else {
        s.edge_since = None;
        s.away_since = None;
        s.pressed = false;
        return std::mem::take(&mut s.called).then_some(false);
    };
    let px = |v: f64| (v * sf).round() as i32;
    if s.called {
        let about = cursor.0 >= x - px(BAND_SIDE) && cursor.0 < x + w + px(BAND_SIDE) && cursor.1 >= top - 1 && cursor.1 < y + h + px(BAND_BELOW);
        if about || over {
            s.away_since = None;
            return None;
        }
        if now.duration_since(*s.away_since.get_or_insert(now)).as_millis() < LEAVE_MS {
            return None;
        }
        s.called = false;
        s.away_since = None;
        return Some(false);
    }
    // On it (the page's island taking the pointer): it is wanted there.
    if over {
        s.called = true;
        s.edge_since = None;
        return Some(true);
    }
    let at_edge = s.stowed && cursor.0 >= x - px(EDGE_SIDE) && cursor.0 < x + w + px(EDGE_SIDE) && cursor.1 >= top - 1 && cursor.1 < top + px(EDGE_PX).max(1);
    if !at_edge {
        s.edge_since = None;
        s.pressed = false;
        return None;
    }
    s.pressed = s.pressed || down();
    let since = *s.edge_since.get_or_insert(now);
    if s.pressed || now.duration_since(since).as_millis() < EDGE_MS {
        return None;
    }
    s.called = true;
    s.edge_since = None;
    Some(true)
}

// Whether its page has said it is ready (for /health): never while no
// window is made for it (another copy holding the taskbar, taskbar/).
pub fn is_ready(sh: &Shared) -> bool {
    sh.island.lock().unwrap().ready
}

// Its page has its listeners up: draw, show, say hello.
pub fn ready(sh: &Arc<Shared>) {
    sh.log("island: page ready");
    // A new page knows of no maximized window yet (look_over_max), nor of
    // one under it (look_under), and has it down.
    OVER_MAX.store(false, Ordering::SeqCst);
    let pending = {
        let mut isl = sh.island.lock().unwrap();
        isl.ready = true;
        let closed = isl.stow.closed;
        isl.stow = Stow { closed, ..Stow::default() };
        isl.pending.take()
    };
    crate::fullscreen::ask();
    sh.redraw();
    taskbar::resend(sh);
    tell_strip(sh, true);
    // The band at a Mac's screen top, now that the page hears it.
    crate::notch::settle(sh);
    sh.greet();
    // Taking the taskbar over: Windows' own laid over ours first, put away
    // under it, then turned to dust that settles into ours.
    let covered = handoff_cover(sh);
    apply_visibility(sh);
    if covered {
        handoff_ask(sh, json!({ "step": "go" }));
        sh.log("handoff: go");
    }
    if let Some(tab) = pending {
        let _ = sh.app.emit_to("island", "island:settings", json!({ "tab": tab }));
    }
}

pub fn apply_visibility(sh: &Arc<Shared>) {
    // Told before it shows, so the strip never shows over the full screen;
    // told it is over once it has gone (or, the full screen gone, stays up).
    let over = over_full(sh);
    let told = sh.island.lock().unwrap().over_full;
    if over && !told {
        tell_over_full(sh, true);
    }
    show_or_hide(sh);
    if !over && told {
        tell_over_full(sh, false);
    }
    // Into Windows' taskbar or out of it, as it is used now (embed.rs).
    crate::embed::apply(sh);
}

// Up over an app full screen for the prompts alone (show_asks), her taskbar
// the home: the page shows her island and what opens from it, not the strip
// under them (style.css over-full), and the pointer goes through there to
// what is full screen. The whole taskbar, the tray and the clock, came up
// over the video with them (the test session, 2026-10-13). Not with the
// settings open: they stand on the strip.
pub(super) fn over_full(sh: &Shared) -> bool {
    sh.home() == "taskbar" && sh.is_fullscreen() && {
        let isl = sh.island.lock().unwrap();
        isl.for_asks && !isl.temp && !isl.settings_open
    }
}

pub(super) fn tell_over_full(sh: &Shared, on: bool) {
    sh.island.lock().unwrap().over_full = on;
    let _ = sh.app.emit_to("island", "island:over-full", json!({ "on": on }));
    sh.log(&format!("island: {}", if on { "over the full screen for the prompts alone, the strip unseen" } else { "the strip seen again" }));
}

pub(super) fn show_or_hide(sh: &Arc<Shared>) {
    // Another copy of her holding the taskbar: this one waits, its window
    // not shown over the other's.
    if sh.home() == "taskbar" {
        taskbar::look_for_other(sh);
    }
    let want = is_shown(sh);
    let (created, ready, shown) = {
        let isl = sh.island.lock().unwrap();
        (isl.created, isl.ready, isl.shown)
    };
    if !created {
        if want {
            create(sh);
        }
        return;
    }
    if !ready {
        return;
    }
    if want {
        place(sh);
    } else {
        // The taskbar keeps the strip it holds while an app full screen
        // hides it; it takes none then: one taken with its window hidden
        // keeps no room (2026-10-10), so it waits for the window to show.
        // Given back, though, to another copy of her holding it too.
        let held = sh.island.lock().unwrap().bar_for.is_some();
        sync_bar(sh, sh.home() == "taskbar" && held && !taskbar::waiting());
    }
    if want == shown {
        return;
    }
    {
        let mut isl = sh.island.lock().unwrap();
        isl.shown = want;
        if !want {
            isl.holding = false;
            isl.reaching = false;
        }
    }
    if let Some(win) = window(sh) {
        let _ = if want { win.show() } else { win.hide() };
    }
    // The blur under the island goes with it, and comes back with it (backplate.rs).
    if want {
        crate::backplate::again();
    } else {
        crate::backplate::hide();
    }
    // Her taskbar on the other displays, as the settings say (bars.rs).
    crate::bars::sync(sh);
}
