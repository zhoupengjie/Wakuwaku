// Her capsule inside Windows' taskbar (embed.rs), and her strip told to
// Windows and kept (the bar's place: sync_bar, release_bar, reclaim_bar).
use super::*;

// --- Her capsule inside Windows' taskbar (embed.rs) ----------------------------------

// Wanted while her home is the island in Windows' taskbar (islandEdge
// taskbar) and the setting (islandEmbed, on unless turned off) says so:
// embed.rs then looks for Explorer's taskbar along the bottom or the top
// of her display every second.
pub fn embedding_wanted(sh: &Shared) -> bool {
    sh.home() != "taskbar" && sh.config().island_edge.as_deref() == Some("taskbar") && sh.config().island_embed != Some(false)
}

// Her island laid over Windows' taskbar (islandEdge taskbar, the embedding
// off): embed.rs makes the taskbar's window its owner, so Windows keeps it
// over the taskbar as Explorer raises the taskbar (uncover only finds it under one).
pub fn over_wanted(sh: &Shared) -> bool {
    sh.home() != "taskbar" && sh.config().island_edge.as_deref() == Some("taskbar") && sh.config().island_embed == Some(false)
}

// The display her home is on, for the capsule to go into its taskbar.
pub fn home_area(sh: &Shared) -> Option<Area> {
    area(sh)
}

// Where the island is set to sit (islandPos), for the stretch of the taskbar the capsule goes in.
pub fn side_wanted(sh: &Shared) -> Side {
    Side::of(sh.config().island_pos.as_deref().unwrap_or("center"))
}

// The closed capsule as the page has it (logical width and height) and the
// page's scale; none before the page has said its size (set_stow).
pub fn closed_size(sh: &Shared) -> Option<(f64, f64, f64)> {
    let isl = sh.island.lock().unwrap();
    let (w, h) = isl.stow.closed;
    (w > 0.0 && h > 0.0).then(|| (w, h, isl.page_sf()))
}

// Where her capsule lies in Windows' taskbar now, with the room it has
// there, or none (out of the taskbar): the window laid about it, shown or
// hidden (is_shown), and the page told (islandEmbed, islandMx, islandRoom).
pub fn set_capsule(sh: &Arc<Shared>, laid: crate::embed::Laid) {
    let changed = {
        let mut isl = sh.island.lock().unwrap();
        let before = (isl.cap, isl.cap_room, isl.embedded);
        isl.cap = laid.cap;
        isl.cap_room = laid.room;
        isl.embedded = laid.embedded;
        before != (laid.cap, laid.room, laid.embedded)
    };
    if !changed {
        return;
    }
    place(sh);
    apply_visibility(sh);
    sh.redraw();
    crate::fullscreen::ask();
}

// The island's page has something open in it (hovered open, a prompt, the
// settings, opened by itself), or nothing now, its island shrunk back
// (island_opened): inside Windows' taskbar the window goes back into it
// once nothing is open and the pointer has gone (parenting, embed.rs apply).
pub fn set_opened(sh: &Arc<Shared>, on: bool) {
    let (was, embedded) = {
        let mut isl = sh.island.lock().unwrap();
        (std::mem::replace(&mut isl.opened, on), isl.embedded)
    };
    if was != on && embedded {
        apply_visibility(sh);
    }
}

// For embed.rs: the window's handle, where it is to be (physical), whether
// it is shown, and whether it is to be a child of Windows' taskbar now:
// inside it with room for its capsule, and not in use (the pointer on it,
// the button held on her, reaching for her or taking her in, something
// open in it, the settings, the prompts, risen). None before it is made.
pub fn parenting(sh: &Shared) -> Option<(isize, (i32, i32), (i32, i32), bool, bool)> {
    let isl = sh.island.lock().unwrap();
    if isl.hwnd == 0 {
        return None;
    }
    Some((isl.hwnd, isl.pos, isl.size, isl.shown, isl.embedded && isl.cap.is_some() && !used(&isl)))
}

// In use: the pointer on it, the button held on her, reaching for her or
// taking her in, something open in it, the settings, the prompts, risen.
pub(super) fn used(isl: &Island) -> bool {
    isl.ct.over || isl.opened || isl.settings_open || isl.holding || isl.reaching || isl.absorbing || isl.for_asks || isl.temp
}

// For embed.rs: whether the island is in use now (used), its window out of
// Windows' taskbar for it: the capsule's place there is held meanwhile.
pub fn in_use(sh: &Shared) -> bool {
    used(&sh.island.lock().unwrap())
}

// For the pages (shared.rs payload): whether her capsule is inside Windows'
// taskbar, where its middle lies in the island's window then (physical px
// from its left), and the room it has there (physical px; 0 none).
pub fn embedding(sh: &Shared) -> (bool, Option<i32>, i32) {
    let isl = sh.island.lock().unwrap();
    (isl.embedded, isl.mx.filter(|_| isl.embedded), isl.cap_room)
}

// The setting homeDisplay changed (#104): her home on that display now,
// whatever it is: the island's window laid there (fit), the taskbar's strip
// asked for along its edge and the old one given back (sync_bar), her
// taskbar on the others and the dock made or taken away (bars.rs sync), the
// page's zoom its scale's; an app full screen there looked for at once. The
// graphics card her pages are drawn on stays the old display's until she is
// next started (webview.rs browser_args).
pub fn home_display_changed(sh: &Shared) {
    place(sh);
    sh.log(&format!("island: her home on {:?} (homeDisplay {})", display(sh), sh.config().home_display));
    sh.redraw();
    crate::fullscreen::ask();
}

// The setting islandPos changed, or islandEdge (#105), or islandEmbed: the
// window put at that side of that edge now, and the page told where it is
// (islandAt, islandEdge) to lay the island out there; a window under it at
// its new place looked for at once (look_under); the capsule's place in
// Windows' taskbar looked at now (embed.rs, which lays the window again
// about it). The line says what it lies over.
pub fn side_changed(sh: &Shared) {
    if sh.home() == "taskbar" {
        return;
    }
    place(sh);
    let over = {
        let isl = sh.island.lock().unwrap();
        match (isl.embedded, isl.strip) {
            (true, _) => ", its capsule inside Windows' taskbar".to_string(),
            (false, Some(thick)) => format!(", over Windows' taskbar ({thick} px)"),
            (false, None) => String::new(),
        }
    };
    sh.log(&format!("island: at the {} of the {} of the screen{over}", side(sh).name(), edge_name(sh)));
    sh.redraw();
    crate::fullscreen::ask();
    crate::embed::look_soon();
}

// The taskbar's window (physical) for its strip along an edge: its outer
// edge on the strip's, the room inside it.
pub(super) fn window_at(edge: Edge, b: (i32, i32, i32, i32), size: (i32, i32)) -> (i32, i32) {
    match edge {
        Edge::Bottom => (b.0, b.1 + b.3 - size.1),
        Edge::Top | Edge::Left => (b.0, b.1),
        Edge::Right => (b.0 + b.2 - size.0, b.1),
    }
}

// The strip's thickness across it (physical), as it lies: what the page
// draws it as (its CSS px times the scale), so that its inner edge is the
// work area's, at any scale (175% left a hairline gap: zhoupengjie/Wakuwaku#2).
pub(super) fn thickness(edge: Edge, b: (i32, i32, i32, i32)) -> i32 {
    if edge.is_side() { b.2 } else { b.3 }
}

// The page told the strip's thickness when the strip, its edge or the window
// change (or anew once it is ready), and a line for the log with all that
// sets where its inner edge lies: Windows' room, the strip, the work area,
// the window (on a change of edge too, the thickness the same: 2026-10-12).
pub fn tell_strip(sh: &Shared, again: bool) {
    if sh.home() != "taskbar" {
        return;
    }
    let edge = edge(sh);
    let Some(area) = area(sh) else { return };
    let (bar, pos, size, sf, told) = {
        let isl = sh.island.lock().unwrap();
        (isl.strip_now(edge, area.mon), isl.pos, isl.size, isl.sf(), isl.strip_told)
    };
    let Some(b) = bar else { return };
    let thick = thickness(edge, b);
    let now = Some((b, edge, pos, size, sf));
    if now == told && !again {
        return;
    }
    sh.island.lock().unwrap().strip_told = now;
    let room = (edge == Edge::Bottom).then(|| crate::shell::room(area.mon)).flatten();
    sh.log(&format!(
        "taskbar: strip {b:?} along the {edge:?}, {thick} px ({} at {sf}); Windows' room {room:?}; work area {:?}; window at {pos:?} {size:?}",
        thick as f64 / sf,
        taskbar::work_area_rect()
    ));
    // With the window's size: the page may be a little bigger (island.ts
    // fitPage, zhoupengjie/Wakuwaku#19).
    let _ = sh.app.emit_to("island", "taskbar:strip", json!({ "thick": thick, "win": [size.0, size.1], "sf": sf }));
}

// One at a time: two at once took the taskbar twice (two tray threads, one
// never stopped). Asked while one runs, it runs again after, with the last
// ask: 1 give back, 2 keep.
pub(super) static SYNCING: AtomicBool = AtomicBool::new(false);
pub(super) static SYNC_ASK: AtomicU8 = AtomicU8::new(0);

pub(super) fn sync_bar(sh: &Shared, keep: bool) {
    SYNC_ASK.store(1 + keep as u8, Ordering::SeqCst);
    loop {
        if SYNCING.swap(true, Ordering::SeqCst) {
            return;
        }
        while let ask @ 1..=2 = SYNC_ASK.swap(0, Ordering::SeqCst) {
            sync_bar_now(sh, ask == 2);
        }
        SYNCING.store(false, Ordering::SeqCst);
        // An ask that came between the last look and letting go.
        if SYNC_ASK.load(Ordering::SeqCst) == 0 {
            return;
        }
    }
}

// The taskbar's strip along its edge (taskbarEdge): asked of Windows while
// the home is the taskbar and keep (up; also while an app full screen hides
// its window: giving the strip back would move every window), given back
// otherwise. Asked again only when the display, the height, the home or the
// edge change. It comes with Windows' own put away first and goes with it
// given back (taskbar/).
pub(super) fn sync_bar_now(sh: &Shared, keep: bool) {
    let home = sh.home();
    let edge = match home {
        "taskbar" => Some((edge(sh), strip_h(sh))),
        _ => None,
    };
    // Windows' own keeps its room along the foot for a strip there to lie
    // over, or for the dock kept in it (split); none else (taskbar/win/hold.rs set_edge).
    let room = edge.is_some_and(|(e, _)| e == Edge::Bottom) || dock_keeps_room(sh);
    let want = match (keep, edge) {
        (true, Some((edge, h))) => area(sh).map(|a| (a, edge, (h * a.sf).round() as i32)),
        _ => None,
    };
    let (hwnd, held, registered) = {
        let isl = sh.island.lock().unwrap();
        (isl.hwnd, isl.bar_for, isl.bar_own)
    };
    if hwnd == 0 {
        return;
    }
    let ask = want.map(|(a, edge, h)| (a.mon, h, home, edge, room));
    if held == ask {
        return;
    }
    // Another home's strip (or none wanted): given back first. A bar of ours
    // taken away only if there is one: over Windows' room there is none, and
    // asking goes to the taskbar's window of the tray, on the taskbar's
    // thread, waited for (quitting, the main thread).
    if let Some((_, _, was, _, _)) = held.filter(|h| Some(h.2) != ask.map(|a| a.2)) {
        // The desktop's icons where they are, before the work area goes
        // back under them (icons.rs hold).
        if was == "taskbar" {
            for line in crate::icons::hold_within(&sh.dir, std::time::Duration::from_secs(2)) {
                sh.log(&line);
            }
        }
        if registered {
            appbar::remove(hwnd);
        }
        {
            let mut isl = sh.island.lock().unwrap();
            isl.bar = None;
            isl.bar_for = None;
            isl.bar_own = false;
        }
        if was == "taskbar" {
            taskbar::give_back(sh);
        }
        sh.log(&format!("island: {was} strip given back"));
    }
    let Some((area, edge, height)) = want else { return };
    if home == "taskbar" {
        let strip = edge.strip(area.mon, height);
        // The desktop's icons where they are, before anything of a strip along
        // another edge than the bottom moves them (icons.rs keep_before).
        if edge != Edge::Bottom {
            for line in crate::icons::keep_before_within(&sh.dir, std::time::Duration::from_millis(1500)) {
                sh.log(&line);
            }
        }
        // Windows' own keeps its room along the bottom for a bottom strip,
        // and none for another edge, but for the dock kept in it (taskbar/win/hold.rs set_edge).
        taskbar::set_edge(edge, room);
        // Another copy of her holds it: nothing taken (it waits).
        if !taskbar::take(sh, strip) {
            return;
        }
        // Along the bottom, laid over the room Windows' own keeps, put away:
        // what it opens (the Start menu, the notifications) opens above it.
        // Its top and height are that room's (the work area stops there),
        // not ours worked out at this scale, when they differ by its
        // rounding (a pixel or two: one a pixel taller left a hairline gap
        // above the strip, zhoupengjie/Wakuwaku#2); a room of another size
        // (Windows 10's, 40 px) leaves ours as it is. Along another edge, a
        // bar of ours (Windows' opens at the bottom).
        if let Some(room) = (edge == Edge::Bottom).then(|| taskbar::explorer_room(area.mon)).flatten() {
            let strip = if (room.3 - height).abs() <= 2 { (area.mon.0, room.1, area.mon.2, room.3) } else { strip };
            let own = {
                let mut isl = sh.island.lock().unwrap();
                isl.bar = Some(strip);
                isl.bar_for = ask;
                std::mem::replace(&mut isl.bar_own, false)
            };
            if own {
                appbar::remove(hwnd);
            }
            taskbar::strip_moved(strip);
            sh.log(&format!("island: taskbar strip {strip:?} over Windows' room {room:?}; work area's bottom {}", taskbar::work_area_bottom()));
            return;
        }
    }
    let registered = sh.island.lock().unwrap().bar_own;
    if !registered && !appbar::register(hwnd) {
        sh.log("island: Windows would not take the bar");
        return;
    }
    // Asked of Explorer itself, not by the taskbar's window of ours (taskbar/win/tray.rs claim).
    let granted = taskbar::claim(|| appbar::place(hwnd, area.mon, edge, height));
    {
        let mut isl = sh.island.lock().unwrap();
        isl.bar = Some(granted);
        isl.bar_for = ask;
        isl.bar_own = true;
    }
    if home == "taskbar" {
        taskbar::strip_moved(granted);
    }
    sh.log(&format!("island: {home} strip {granted:?}; work area's bottom {}", taskbar::work_area_bottom()));
}

// Quitting, or the window going: the strip back to the other windows (and
// Windows' own taskbar back, for the taskbar's). Waited for, so it is back
// before she goes (should it take too long, the guard gives it back).
pub fn release_bar(sh: &Shared) {
    // Her taskbar on the other displays goes first (bars.rs).
    crate::bars::close_all(sh);
    let t = std::time::Instant::now();
    while SYNCING.load(Ordering::SeqCst) && t.elapsed() < std::time::Duration::from_secs(5) {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    sync_bar(sh, false);
}

// Explorer started again: it forgot the strip, which is asked for anew.
pub fn take_strip_again(sh: &Shared) {
    let hwnd = {
        let mut isl = sh.island.lock().unwrap();
        if isl.bar_for.take().is_none() {
            return;
        }
        isl.bar_own = false;
        isl.hwnd
    };
    // Should it remember after all, a second registration would be refused.
    appbar::remove(hwnd);
    place(sh);
}

// Explorer set the work area as though the strip were not there
// (taskbar/win/work.rs keep_work): its place told again (ABM_QUERYPOS, ABM_SETPOS,
// to Explorer itself: taskbar/win/tray.rs claim), as an app bar is to do when
// Explorer says the bars moved (ABN_POSCHANGED, which her home's window, a
// webview's, does not hear). Explorer had the bar registered but left it
// out of every work area it worked out: its place, asked by the taskbar's
// window of ours, never reached it (systray/ explorer_first), and its own
// taskbar shown again (a right-click's menu, Win+Shift+S, Click to Do, an
// app full screen going) set the work area to the whole display each time,
// the maximized windows going up under the top bar. Told its place from
// another process, the same rect, Explorer counted it from then on, and its
// taskbar shown went by with the work area as it was, no word to the
// windows (the test session, 2026-11-09). Here should Explorer forget it
// all the same. Only the bar held and its own (a strip over Windows' room
// along the foot has none), never while it is being taken or given back
// (SYNCING). Called on the taskbar's thread: nothing here waits on the
// main thread. The rect Windows grants, or None.
pub fn reclaim_bar(sh: &Shared) -> Option<(i32, i32, i32, i32)> {
    if SYNCING.load(Ordering::SeqCst) {
        return None;
    }
    let (hwnd, held, own, was) = {
        let isl = sh.island.lock().unwrap();
        (isl.hwnd, isl.bar_for, isl.bar_own, isl.bar)
    };
    let (mon, height, _, edge, _) = held?;
    if !own || hwnd == 0 {
        return None;
    }
    let granted = taskbar::claim(|| appbar::place(hwnd, mon, edge, height));
    sh.island.lock().unwrap().bar = Some(granted);
    if Some(granted) != was {
        sh.log(&format!("island: bar's place told again, granted {granted:?} where it was {was:?} (the window left where it is)"));
    }
    Some(granted)
}

