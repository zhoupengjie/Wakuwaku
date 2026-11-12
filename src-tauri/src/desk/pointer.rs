// The pointer on the desktop's widgets: whether it is on one (in_shape),
// how often to look (pace, poll), the desktop's icons under it.
use super::*;

// Whether a point (the page's px) is in a rounded rect, its corners round.
pub fn in_shape(x: f64, y: f64, s: &[f64; 5], margin: f64) -> bool {
    let [sx, sy, w, h, r] = *s;
    let (l, t, rr, b) = (sx - margin, sy - margin, sx + w + margin, sy + h + margin);
    if x < l || x > rr || y < t || y > b {
        return false;
    }
    let r = (r + margin).min(w / 2.0 + margin).min(h / 2.0 + margin).max(0.0);
    let cx = x.clamp(l + r, rr - r);
    let cy = y.clamp(t + r, b - r);
    (x - cx).powi(2) + (y - cy).powi(2) <= r * r
}

// How soon the pointer's look is wanted (ms); None: not at all, the check
// stopped (the design session, 2026-10-16): nobody about, no widget on the
// display the pointer is on, or that desktop covered (a window maximized or
// full screen over it). On a display with widgets every 250 ms; within 40 px
// of one, or in the taskbar's strip (the display less its work area), every
// 100 ms, so that a quick move onto one is caught (the VM, 2026-10-16: from
// the taskbar 1 in 4 at 250 ms, 6 in 6 at 100 ms). Some picked with Ctrl,
// as near one (a press off them is looked for). Ctrl held over that desktop,
// every 40 ms (CTRL_MS): Ctrl let go, the mouse let go soon after; the Ctrl
// going down is caught at once by its key (imp: a keyboard hook wakes the
// pointer's loop), so with Ctrl up nothing is looked at more than before,
// but while one of her own windows has the keyboard, when the hook hears
// nothing (poll): then 40 ms too.
pub fn pace(on_widgets: bool, seen: bool, quiet: bool, near: bool, in_strip: bool, ctrl: bool) -> Option<u64> {
    if quiet || !on_widgets || !seen {
        return None;
    }
    Some(if ctrl {
        CTRL_MS
    } else if near || in_strip {
        100
    } else {
        250
    })
}
pub(super) const CTRL_MS: u64 = 40;

// Whether the pointer is looked at at all: someone about, and a desktop
// window there.
pub(super) fn looks_wanted(quiet: bool, desks: usize) -> bool {
    !quiet && desks > 0
}

// What a stopped check gives the pointer's loop: never sooner than her own looks.
pub(super) const IDLE_MS: u64 = 60_000;

pub(super) fn contains(r: Mon, (x, y): (i32, i32)) -> bool {
    x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3
}

// One look at the pointer (physical px): each window takes the mouse while
// it is on one of its widgets (or held, or a menu open there), lets it
// through elsewhere. How long until the next look (ms; IDLE_MS: none wanted).
//
// Several widgets picked with Ctrl (desk.ts, the user's call of 2026-10-17:
// Ctrl and a drag over the bare desktop draws a rectangle, Ctrl and a press
// on one picks it): while Ctrl is down (GetAsyncKeyState) and the pointer is
// over a display's desktop that has widgets, seen, in its work area and off
// Explorer's icons there (the rects its page had read, desk_icons: Ctrl and
// a press on an icon is still Explorer's), that window takes the mouse as
// over a widget, and lets it go as Ctrl is let go (a rectangle begun is
// held, desk_hold). Without Ctrl the desktop is Windows' own (its icons'
// rectangle, its menu). Ctrl's going down wakes this look at once (imp's
// keyboard hook), so a press right after it reaches the page; then every
// 40 ms while it is held there. While some are picked (desk_picked), a
// press off them goes on to Explorer and the page is told (desk:outside),
// as a press elsewhere with a menu open is.
pub fn poll(sh: &Shared, cursor: (i32, i32)) -> u64 {
    // Nobody about: the pointer rests; each window keeps what it does.
    if !looks_wanted(sh.is_quiet(), 1) {
        return IDLE_MS;
    }
    // The display under the pointer, if it has widgets: whether its desktop is seen.
    let under = {
        let desks = DESKS.lock().unwrap();
        if !looks_wanted(false, desks.len()) {
            return IDLE_MS;
        }
        desks.iter().find(|d| d.ready && !d.shapes.is_empty() && contains(d.mon, cursor)).map(|d| d.mon)
    };
    count(&POLLS);
    let seen = under.is_some_and(|mon| seen_now(sh, mon));
    let down = crate::pet::any_down().unwrap_or(false);
    let ctrl = under.is_some() && imp::ctrl_down();
    let ctrl_turned = CTRL_WAS.swap(ctrl, Ordering::Relaxed) != ctrl;
    let ctrl_came = ctrl_turned && ctrl;
    if ctrl_turned && tracing() {
        sh.log(&format!("desk: Ctrl {} for the look, {} ms after its key", if ctrl { "down" } else { "up" }, imp::ctrl_age()));
    }
    let mut changes: Vec<(String, isize, Option<bool>, bool, (f64, f64))> = Vec::new();
    let (mut near, mut in_strip, mut ctrl_over) = (false, false, false);
    let mut stale_icons = None;
    {
        let mut desks = DESKS.lock().unwrap();
        for d in desks.iter_mut().filter(|d| d.ready) {
            let on_display = contains(d.mon, cursor);
            let (x, y) = ((cursor.0 - d.mon.0) as f64 / d.sf, (cursor.1 - d.mon.1) as f64 / d.sf);
            // Under a window over its desktop, nothing of it is under the pointer.
            let visible = on_display && seen;
            let inside = visible && d.shapes.iter().any(|s| in_shape(x, y, s, 0.0));
            if visible {
                near |= d.picked || d.shapes.iter().any(|s| in_shape(x, y, s, NEAR_PX));
                in_strip |= !contains(d.work, cursor);
            }
            // Ctrl held over its bare desktop: taken there too, but on an icon.
            let ctrl_here = ctrl && visible && !d.shapes.is_empty() && contains(d.work, cursor);
            ctrl_over |= ctrl_here;
            if ctrl_here && ctrl_came && d.icons_at.is_none_or(|at| at.elapsed() > ICONS_FRESH) {
                stale_icons = Some((d.label.clone(), d.mon, d.sf));
            }
            let ctrl_on_icon = ctrl_here && on_icon(&d.icons, x, y);
            if ctrl_came && on_display && tracing() {
                sh.log(&format!("desk: Ctrl over {} at ({x:.0}, {y:.0}): seen {seen}, {} icons read {:?} ago, on one {ctrl_on_icon}", d.label, d.icons.len(), d.icons_at.map(|at| at.elapsed())));
            }
            let ctrl_here = ctrl_here && !ctrl_on_icon;
            // A menu open: the whole display's desktop, for a press off it
            // to close it (the page), as Windows' own menus take that press.
            let catch = d.held || inside || (d.menu && on_display) || ctrl_here;
            // A press off its widgets with a menu open: the menu goes. Some
            // picked: they are let go of (not with Ctrl on the bare desktop,
            // a rectangle to pick more with).
            let outside = (d.menu || (d.picked && !ctrl_here)) && down && !d.was_down && !inside && !d.held;
            d.was_down = down;
            if catch != d.catching || outside {
                let changed = (catch != d.catching).then_some(catch);
                d.catching = catch;
                changes.push((d.label.clone(), d.hwnd, changed, outside, (x, y)));
            }
        }
    }
    for (label, hwnd, changed, outside, at) in changes {
        if let Some(catch) = changed {
            let _ = sh.app.run_on_main_thread(move || imp::set_through(hwnd, !catch));
            // Where the pointer is as the window takes it (the page hears
            // no move till the next: its hover shown at once), or gone.
            emit(sh, &label, "desk:pointer", if catch { json!({ "x": at.0, "y": at.1 }) } else { Value::Null });
        }
        if outside {
            emit(sh, &label, "desk:outside", Value::Null);
        }
    }
    // Ctrl down over a desktop whose icons were read a while ago: read
    // again (the person may have moved them), off this thread.
    if let Some((label, mon, sf)) = stale_icons {
        read_icons(label, mon, sf);
    }
    // One of her windows has the keyboard (the picker or the settings open,
    // widgets picked, a name typed): the keyboard hook hears nothing then
    // (WebView2 in this process has the focus, DEVELOPMENT.md's Win keys;
    // seen again here, 2026-10-17), so Ctrl is looked for at its own pace
    // over a desktop with widgets.
    let keys_ours = under.is_some() && seen && !ctrl_over && sh.is_own(crate::focus::foreground());
    pace(under.is_some(), seen, false, near, in_strip, ctrl_over || keys_ours).unwrap_or(IDLE_MS)
}

// Ctrl as the last look had it (its going down: the icons read again).
pub(super) static CTRL_WAS: AtomicBool = AtomicBool::new(false);

// WAKUWAKU_DESK_TRACE=1: what the look made of Ctrl, logged.
pub(super) fn tracing() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("WAKUWAKU_DESK_TRACE").is_some())
}
// The icons as read this long ago are read again as Ctrl goes down.
pub(super) const ICONS_FRESH: std::time::Duration = std::time::Duration::from_secs(3);

// Whether a point of a desktop's page is on one of its icons (their rects).
pub(super) fn on_icon(icons: &[[f64; 4]], x: f64, y: f64) -> bool {
    icons.iter().any(|r| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3])
}

// A display's icons read again for its window, on a thread of their own
// (Explorer answers), one at a time.
pub(super) fn read_icons(label: String, mon: Mon, sf: f64) {
    static READING: AtomicBool = AtomicBool::new(false);
    if READING.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        if let Some(icons) = imp::icons(mon, sf) {
            keep_icons(&label, icons);
        }
        READING.store(false, Ordering::SeqCst);
    });
}

pub(super) fn keep_icons(label: &str, icons: Vec<[f64; 4]>) {
    if let Some(d) = DESKS.lock().unwrap().iter_mut().find(|d| d.label == label) {
        d.icons = icons;
        d.icons_at = Some(std::time::Instant::now());
    }
}

