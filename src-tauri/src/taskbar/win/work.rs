// The work area kept to the strip: its place told to the window of the
// taskbar's class, the work area set along it and set again when Windows or
// a program moves it (the self-heal), the desktop's icons out from under it,
// the strip taken again, and windows kept from under a strip at the top.
use super::*;
use crate::lock::Lock;

// Where the strip is now (physical): ours of the taskbar's class goes
// there too, for those who ask the taskbar's window where it is.
pub fn strip_moved(strip: (i32, i32, i32, i32)) {
    *STRIP.lock().unwrap() = Some(strip);
    // Along another edge than the bottom (taken there, or moved there):
    // the work area made the display less the strip, at once. The self-heal only ever moves the
    // strip's own side, and the room the last edge took was kept: from
    // the left to the right the work area kept its 60 px on the left too,
    // then the top's as well (2026-10-12). Along the bottom Windows' own
    // takes its room back and sets it whole itself.
    let edge = *EDGE.lock().unwrap();
    let room = shell::kept_room();
    let was = std::mem::replace(&mut *WORK_EDGE.lock().unwrap(), Some((edge, room)));
    if is_up() && was != Some((edge, room)) && edge != Edge::Bottom {
        let ((x, y, w, h), _) = monitor_of(strip);
        let mut r = Rect { left: x, top: y, right: x + w, bottom: y + h };
        match edge {
            Edge::Top => r.top = strip.1 + strip.3,
            Edge::Left => r.left = strip.0 + strip.2,
            Edge::Right => r.right = strip.0,
            Edge::Bottom => r.bottom = strip.1,
        }
        // Split with the dock kept in the room Windows' own keeps along
        // the foot: the work area stops there too (as it does now, should
        // Windows not have taken its room back yet; it sets it itself as it does).
        if room {
            r.bottom = shell::room((x, y, w, h)).map_or(work_area().bottom.min(y + h), |foot| foot.1);
        }
        log(&format!("the strip now along the {edge:?}: work area made ({}, {}, {}, {})", r.left, r.top, r.right, r.bottom));
        set_work_area(r);
        *WORK_SIDE.lock().unwrap() = Some((edge, edge.inner(strip)));
        icons_out(strip, edge);
    } else if is_up() && was.is_some_and(|w| w.0 != Edge::Bottom) && edge == Edge::Bottom {
        // Back along the bottom: the work area the display less the room
        // Windows' own keeps there (or the whole of it, that room not back
        // yet), the side the last edge took given back. Windows' own made
        // it so itself as it took its room back, unless it had kept the
        // room all along, and then the left's 60 px stayed (the test
        // session, 2026-10-13). By the room, not the strip: the strip told
        // first may still be the last edge's.
        let ((x, y, w, h), _) = monitor_of(strip);
        let foot = shell::room((x, y, w, h)).map_or(y + h, |r| r.1);
        log(&format!("the strip back along the Bottom: work area made ({x}, {y}, {}, {foot})", x + w));
        set_work_area(Rect { left: x, top: y, right: x + w, bottom: foot });
        // Windows' room taken again: the desktop's
        // icons as they were (icons.rs).
        if let Some(sh) = sh() {
            std::thread::spawn(move || {
                for line in crate::icons::given_back(&sh.dir) {
                    sh.log(&line);
                }
            });
        }
    }
    if is_up() {
        systray::place(strip);
        // SAFETY: our own window; a posted message, no pointers.
        unsafe { PostMessageW(OURS.load(Ordering::SeqCst) as Hwnd, WM_PAPER, 0, 0) };
    }
}

// The strip taken again (Explorer started again and forgot it), off
// this thread: the island's side waits on the main thread, which may be
// waiting on this one (its app bar's messages come to the tray's window,
// here).
pub(super) fn take_strip_again() {
    if let Some(sh) = sh() {
        std::thread::spawn(move || crate::island::take_strip_again(&sh));
    }
}

// The desktop's icons out from under the strip, the work area set (icons.rs:
// the desktop is told it, as it is not). Off this thread: it is asked
// through Explorer, which may be busy.
pub(super) fn icons_out(strip: (i32, i32, i32, i32), edge: Edge) {
    let Some(sh) = sh() else { return };
    std::thread::spawn(move || {
        let log = sh.clone();
        crate::icons::strip_taken(&sh.dir, strip, edge.word(), move |line| log.log(&line));
    });
}

// The work area's side on the strip's edge set where the strip starts,
// as Explorer sets it for a bar it counts. It does not always count ours:
// put away in a moment (its work area free at once, not after its slide),
// Windows' taskbar left the work area the whole display, our strip taken
// or not, even taken again, even with it set to hide itself afresh
// (2026-10-10). Set quietly: told to every window, Explorer set it back
// at once.
pub(super) fn set_work_side(edge: Edge, side: i32) {
    let mut r = work_area();
    match edge {
        Edge::Bottom => r.bottom = side,
        Edge::Top => r.top = side,
        Edge::Left => r.left = side,
        Edge::Right => r.right = side,
    }
    set_work_area(r);
}

pub(super) fn xywh_of(r: &Rect) -> (i32, i32, i32, i32) {
    (r.left, r.top, r.right - r.left, r.bottom - r.top)
}

pub(super) fn set_work_area(mut r: Rect) {
    let was = work_area();
    // The windows maximized there laid on it anew: set quietly, it moves
    // none (appbar.rs). Before it is set, so as not to wait on the call,
    // which took 0 to 1.3 s (2026-11-09, at a tick or at Explorer's word
    // alike); when it took a second, though, the windows too went down
    // only as it returned.
    let (from, asked) = (xywh_of(&was), xywh_of(&r));
    let mut moved = crate::appbar::refit_maximized(from, asked);
    let t = Instant::now();
    // SAFETY: our own struct, handed back.
    let ok = unsafe { SystemParametersInfoW(SPI_SETWORKAREA, 0, &mut r as *mut Rect as *mut c_void, 0) } != 0;
    // Why, only when it failed: a call that worked leaves an old error
    // there (it read "access denied" after a set that took).
    let error = if ok { String::new() } else { format!(" ({})", std::io::Error::last_os_error()) };
    let set_ms = t.elapsed().as_millis();
    let now = work_area();
    let to = xywh_of(&now);
    // Not set, or set otherwise than asked: those just laid on the one
    // asked laid on the work area as it is.
    if to != asked {
        moved = crate::appbar::refit_maximized(asked, to);
    }
    if moved > 0 {
        log(&format!("{moved} maximized window(s) laid on the work area anew"));
    }
    // And again a moment after: set at Explorer's word (keep_work), the
    // maximized windows were going up to the whole display 10 to 25 ms
    // after it, and one gone up after the look above lay under the strip
    // (the test session, 2026-11-09). Only while the work area is still hers.
    std::thread::spawn(move || {
        let mut late = 0;
        for wait in [40, 110] {
            std::thread::sleep(Duration::from_millis(wait));
            if ok && xywh_of(&work_area()) == to {
                late += crate::appbar::refit_maximized(from, to);
            }
        }
        let later = work_area();
        let late = if late > 0 { format!("; {late} maximized window(s) laid on it late") } else { String::new() };
        log(&format!(
            "work area set: {ok}{error} in {set_ms} ms; then ({}, {}, {}, {}), 150 ms after ({}, {}, {}, {}){late}",
            now.left, now.top, now.right, now.bottom, later.left, later.top, later.right, later.bottom
        ));
    });
}

// Her strip along the top (the top bar, split, too): what pops up over it
// or opens at the foot for a taskbar goes under it. A program with an
// icon in her tray put its popup at the very top, over the bar (WeChat's
// new message, the user, 2026-11-02: the tray it hands its icon to is at
// the top, Windows' own taskbar it asks of at the foot); Windows' own
// quick settings, notifications and the rest (is_shell_flyout) open at
// the foot whatever the taskbar's edge: moved up under it, at the right,
// as under a taskbar at the top. Each time it shows or moves there, not
// waiting on its program (SWP_ASYNCWINDOWPOS); never while a button is
// held (a window being dragged), nor her own windows, nor Start and
// search (in the middle, Windows' own business); and no more than a few
// times a second for one window, should its program move it back.
// Every move of every window comes here (moved): its frame looked at
// first, its program only for one over the strip, or Windows' flyout at
// the foot as it opens (shown, to the front; known by its window after).
pub(super) static UNDER_BAR: Lock<Vec<(isize, u32, Instant)>> = Lock::new("taskbar::win::work::UNDER_BAR", Vec::new());
pub(super) static FLYOUTS: Lock<Vec<isize>> = Lock::new("taskbar::win::work::FLYOUTS", Vec::new());

pub(super) fn keep_under_bar(hwnd: isize, moved: bool) {
    const SWP_NOSIZE: u32 = 0x1;
    const SWP_NOZORDER: u32 = 0x4;
    const SWP_ASYNCWINDOWPOS: u32 = 0x4000;
    if *EDGE.lock().unwrap() != Edge::Top || !is_up() {
        return;
    }
    let Some(strip) = *STRIP.lock().unwrap() else { return };
    let Some((l, t, r, b)) = tasks::frame(hwnd) else { return };
    let ((mx, my, mw, mh), sf) = monitor_of(strip);
    if r <= mx || l >= mx + mw || b <= my || t >= my + mh {
        return;
    }
    let bar = strip.1 + strip.3;
    let over = t < bar && b > strip.1;
    let foot = b > my + mh * 2 / 3 && b - t < mh * 3 / 4 && (!moved || FLYOUTS.lock().unwrap().contains(&hwnd));
    if !over && !foot {
        return;
    }
    let Some(p) = tasks::popup_of(hwnd) else { return };
    if p.pid == std::process::id() || tasks::is_start(hwnd) {
        return;
    }
    let flyout = tasks::is_shell_flyout(hwnd);
    if flyout {
        let mut known = FLYOUTS.lock().unwrap();
        if !known.contains(&hwnd) {
            known.push(hwnd);
            let n = known.len();
            known.drain(..n.saturating_sub(16));
        }
    }
    // Windows' flyout standing on the foot; a tray program's popup over
    // the strip: one of a popup's size, not Explorer's (its icons are in
    // her tray too, and a window of its 2560 x 4 along the top was moved
    // over and over, Windows' taskbar showing itself each time: the dev
    // build, 2026-11-04; its own popups are the flyouts).
    let popup = p.tool && !p.program.eq_ignore_ascii_case("explorer.exe") && r - l < mw / 2 && b - t >= (24.0 * sf) as i32;
    let wants = if flyout { b > my + mh * 2 / 3 && b - t < mh * 3 / 4 } else { popup && t < bar && b > strip.1 && b - t < mh / 2 && crate::systray::holders().contains(&p.pid) };
    if !wants || crate::pet::any_down() == Some(true) {
        return;
    }
    {
        let mut moved = UNDER_BAR.lock().unwrap();
        moved.retain(|(_, _, at)| at.elapsed() < Duration::from_secs(5));
        let n = moved.iter().filter(|(h, _, _)| *h == hwnd).map(|(_, n, _)| *n).max().unwrap_or(0);
        if n >= 20 {
            return;
        }
        moved.push((hwnd, n + 1, Instant::now()));
        if n == 0 {
            log(&format!("{} of {} at ({l}, {t}, {r}, {b}): moved under the strip along the top", if flyout { "Windows' flyout" } else { "a tray program's popup" }, p.program));
        }
    }
    let gap = (8.0 * sf).round() as i32;
    let x = if flyout { p.rect.0 + (mx + mw - gap - r) } else { p.rect.0 };
    let y = p.rect.1 + (bar + gap - t);
    // SAFETY: another program's window, moved without waiting on it.
    unsafe { SetWindowPos(hwnd as Hwnd, 0, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS) };
}

// Explorer's taskbar showing itself (a right-click's menu, Win+Shift+S,
// Task Manager opening: put away again at once, put_away_again) sets the
// work area to the whole display, her strip not counted, and tells every
// window (WM_SETTINGCHANGE, SPI_SETWORKAREA; hers are set quietly): the
// maximized ones went up under the top bar within 10 to 25 ms, and
// stayed there till the tick saw it, half a second, then the next, and
// up to two seconds more, four when quiet (the user, 2026-11-09: 顶栏边
// 界有概率会消失，让最大化窗口直接顶上去, 在我右键的时候，或者我
// win+shift+s截图的时候; shown again on purpose in the test session:
// Explorer's word 140 ms after, the work area back 1.5 and 3.8 s after).
// So its word, or its taskbar shown, looks at the work area once
// Explorer has been still a moment (WORK_STILL_MS, the timer set again
// at each): taken at its first word, Explorer showed its taskbar again
// and again for some seconds, setting the work area back each time, the
// windows going up and down five times in seven seconds (the test
// session). So taken a few times within a while; more, and the tick's
// pace is kept.
pub(super) fn work_after_still(hwnd: Hwnd) {
    // SAFETY: our own window's timer, on its thread; set again, it starts over.
    unsafe { SetTimer(hwnd, WORK_STILL, WORK_STILL_MS, std::ptr::null()) };
}

pub(super) const EAGER_TIMES: usize = 6;
pub(super) const EAGER_WITHIN: Duration = Duration::from_secs(10);
pub(super) static EAGER: Lock<Vec<Instant>> = Lock::new("taskbar::win::work::EAGER", Vec::new());

pub(super) fn eager_allowed() -> bool {
    let mut times = EAGER.lock().unwrap();
    times.retain(|at| at.elapsed() < EAGER_WITHIN);
    if times.len() >= EAGER_TIMES {
        return false;
    }
    times.push(Instant::now());
    true
}

// The work area reaching into the strip: Windows has not counted it.
// Or stopping short of it where she set it for the strip before it
// moved. Its side on the strip's edge is set where the strip starts
// (after half a second of it, at most every two seconds, should
// something set it back; at Explorer's word once it is still, eager:
// work_after_still); and the other displays' (bars.rs).
pub(super) fn keep_work(eager: bool) {
    let work = work_area();
    let bottom = work.bottom;
    if bottom != WORK_BOTTOM.swap(bottom, Ordering::SeqCst) {
        log(&format!("work area's bottom now {bottom}"));
    }
    let edge = *EDGE.lock().unwrap();
    // The strip and its display; none while the strip as told lies off
    // its display's edge (appbar.rs lies_along): told before the display
    // changed, it would set the work area by where the edge was. Nothing
    // is set until it is told anew.
    let told = *STRIP.lock().unwrap();
    let laid = told.map(|s| (s, monitor_of(s).0)).filter(|&(s, mon)| {
        let along = edge.lies_along(s, mon);
        if !along && !STRIP_OFF.swap(true, Ordering::SeqCst) {
            log(&format!("the strip as told {s:?} lies off the {edge:?} of its display {mon:?} (the display changed): the work area left be until it is told anew"));
        } else if along {
            STRIP_OFF.store(false, Ordering::SeqCst);
        }
        along
    });
    let strip = laid.map(|(s, _)| s);
    let xywh = (work.left, work.top, work.right - work.left, work.bottom - work.top);
    let ours = WORK_SIDE.lock().unwrap().filter(|w| w.0 == edge).map(|w| w.1);
    let fix = strip.and_then(|s| edge.work_fix(xywh, s, ours));
    // A side an earlier strip of hers held in, kept by Explorer: given back (appbar.rs stale_sides).
    let stale = laid.filter(|_| edge != Edge::Bottom).map(|(s, m)| edge.stale_sides(xywh, s, m)).unwrap_or_default();
    let lost = fix.is_some() || !stale.is_empty();
    let ticks = TICKS.load(Ordering::SeqCst);
    let due = if !lost {
        LOST_SINCE.store(0, Ordering::SeqCst);
        false
    } else if eager && eager_allowed() {
        true
    } else if LOST_SINCE.load(Ordering::SeqCst) == 0 {
        LOST_SINCE.store(ticks.max(1), Ordering::SeqCst);
        false
    } else {
        ticks >= LOST_SINCE.load(Ordering::SeqCst) + 1 && ticks >= LAST_TAKEN.load(Ordering::SeqCst) + 4
    };
    if due {
        LAST_TAKEN.store(ticks, Ordering::SeqCst);
        LOST_SINCE.store(0, Ordering::SeqCst);
        let when = if eager { " at Explorer's word, it still" } else { "" };
        if let Some(side) = fix {
            let reaches = strip.is_some_and(|s| edge.reaches(xywh, s));
            let (how, why) = if reaches { ("reaches into", "") } else { ("stops short of", ", where she set it for one before") };
            // Explorer left her bar out: its place told again first
            // (island.rs reclaim_bar), Explorer then counting it, the work
            // area set by Explorer itself; set here too, should it not.
            let told = if reaches { sh().and_then(|sh| crate::island::reclaim_bar(&sh)) } else { None };
            let counted = match (told, strip) {
                (Some(_), Some(s)) => format!("; the bar's place told again, Explorer {}", if edge.reaches(xywh_of(&work_area()), s) { "still leaving it out" } else { "counting it now" }),
                _ => String::new(),
            };
            log(&format!("work area ({}, {}, {}, {}) {how} the strip ({strip:?}, {edge:?}){why}: its side set to {side}{when}{counted}", work.left, work.top, work.right, work.bottom));
            set_work_side(edge, side);
            *WORK_SIDE.lock().unwrap() = Some((edge, side));
            if let Some(strip) = strip {
                icons_out(strip, edge);
            }
        }
        for (side, at) in stale {
            log(&format!("work area ({}, {}, {}, {}) held in along the {side:?} as by a strip of hers before, the strip along the {edge:?}: that side set to {at}", work.left, work.top, work.right, work.bottom));
            set_work_side(side, at);
        }
    }
    // The other displays' work areas: clear of her strips there, and
    // whole where there is none (bars.rs).
    if let Some(sh) = sh() {
        crate::bars::keep_work(&sh, eager);
    }
}
