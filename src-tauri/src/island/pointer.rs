// The pointer on her home, its seat and room, its zoom, holding her,
// reaching for her and taking her in.
use super::*;

// One look at the mouse. Returns ms until the next look.
pub fn poll(sh: &Shared, cursor: (i32, i32)) -> u64 {
    // She is being dragged past: the home is not being hovered.
    let passive = crate::pet::is_dragged(sh);
    let (ignore, pointer, wait, called, came, embedded) = {
        let mut isl = sh.island.lock().unwrap();
        if !isl.ready {
            return 500;
        }
        let embedded = isl.embedded && isl.shown;
        if !isl.shown {
            (isl.ct.force(Mode::Pass), None, 500, None, false, embedded)
        } else {
            let (pos, size, sf, page_sf, held) = (isl.pos, isl.size, isl.sf(), isl.page_sf(), isl.holding);
            let solid = std::mem::take(&mut isl.solid);
            let seen = isl.ct.look(cursor, pos, size, sf, page_sf, held, passive && !held, &solid);
            isl.solid = solid;
            if isl.ct.calmed() {
                sh.log(&format!("island: mouse mode bouncing, calmed: cursor {cursor:?}, window {pos:?} {size:?}, scale {sf}"));
            }
            // Editing the taskbar's layout, or the picker or a panel open:
            // looked at often wherever the pointer is, so they take the
            // mouse soon after the pointer comes onto them from afar (a
            // press at once went under the edit bar, #53).
            let wait = if held || isl.ct.over { 50 } else if seen.forward || isl.editing || !isl.solid.is_empty() { 30 } else if seen.near { 80 } else { 200 };
            // Out of the way of a window: the pointer calling it down. And
            // the pointer just come near it: a window come under it since
            // the last look is looked for now, before a press reaches it.
            let called = call_look(&mut isl, cursor, passive, std::time::Instant::now(), || crate::pet::any_down().unwrap_or(false));
            let was_near = std::mem::replace(&mut isl.stow.near, seen.near);
            let came = seen.near && !was_near && isl.stow.closed.0 > 0.0;
            (seen.ignore, seen.pointer, wait, called, came, embedded)
        }
    };
    set_ignore(sh, ignore);
    if let Some(at) = pointer {
        let _ = sh.app.emit_to("island", "pet:pointer", at);
    }
    if let Some(on) = called {
        sh.log(&format!("island: {}", if on { "called down by the pointer" } else { "the pointer gone from it" }));
        let _ = sh.app.emit_to("island", "island:called", json!({ "on": on }));
    }
    if came && sh.home() == "island" && sh.config().island_hide_under {
        crate::fullscreen::ask();
    }
    // Come near her island along the foot of the screen: over Windows'
    // taskbar before a press reaches it (#105).
    if came {
        uncover(sh);
    }
    // Inside Windows' taskbar: into it or out of it as it is used now (embed.rs).
    if embedded {
        crate::embed::apply(sh);
    }
    wait
}

pub fn hover(sh: &Shared, is_over: bool) {
    let (ignore, came) = {
        let mut isl = sh.island.lock().unwrap();
        let held = isl.holding;
        let was = isl.ct.over;
        (isl.ct.set_over(is_over, held, false), is_over && !was)
    };
    set_ignore(sh, ignore);
    // Inside Windows' taskbar: out of it as the pointer comes, back as it goes (embed.rs).
    crate::embed::apply(sh);
    // The pointer come to her home: the monitor reads what its widget's card
    // shows now, not at its next turn (widgets.rs Watching::looking).
    if came {
        crate::loops::wake_monitor();
    }
    // The home opens up to say what happened while hovered, so it counts
    // as seen once the pointer leaves.
    if !is_over && sh.pet.lock().unwrap().seen(crate::now_ms()) {
        sh.redraw();
    }
}

// The pointer on her home (the island, the strip, what opens from them).
pub fn is_over(sh: &Shared) -> bool {
    sh.island.lock().unwrap().ct.over
}

// Her seat on the taskbar as the page has laid her island out (logical px
// from the strip's left): what she is reached for from, and flies back to.
pub fn set_seat(sh: &Shared, at: f64) {
    if at.is_finite() && at >= 0.0 {
        sh.island.lock().unwrap().seat_at = Some(at);
    }
}

// The room a shape needs, as the page measured it.
pub fn set_room(sh: &Shared, measured: &Value) {
    let room = match (measured["width"].as_f64(), measured["height"].as_f64()) {
        (Some(w), Some(h)) if w > 0.0 && h > 0.0 => {
            let isl = sh.island.lock().unwrap();
            let page_sf = valid_scale(measured["dpr"].as_f64()).unwrap_or_else(|| isl.page_sf());
            let ratio = page_sf / isl.sf();
            Some(((w * ratio).ceil(), (h * ratio).ceil()))
        }
        _ => None,
    };
    sh.island.lock().unwrap().room = room;
    place(sh);
}

// A page's devicePixelRatio as it told it, if it is one.
pub(crate) fn valid_scale(scale: Option<f64>) -> Option<f64> {
    scale.filter(|v| v.is_finite() && *v >= 0.5 && *v <= 8.0)
}

// The button is held on her in her home (a pull, or carrying her out).
// The taskbar's layout being edited, or not any more, and where the window
// takes the mouse whatever the page says of the pointer (page px): the edit
// bar while editing; the widget picker, its popover over a widget, her
// capsule's panel and a toast with 撤销 (picker.ts solids) whenever they
// are open, editing or not, on the taskbar and at the top of the screen
// alike; none, the mouse as ever. Logged as each starts and ends only: the
// rects come again as they change (the picker opened, a toast gone).
pub fn set_editing(sh: &Shared, on: bool, rects: Vec<[f64; 4]>, page_sf: Option<f64>) {
    let rects: Vec<[f64; 4]> = rects.into_iter().filter(|r| r.iter().all(|v| v.is_finite()) && r[2] > 0.0 && r[3] > 0.0).take(8).collect();
    let solid = !rects.is_empty();
    let (was, had) = {
        let mut isl = sh.island.lock().unwrap();
        if let Some(page_sf) = valid_scale(page_sf) {
            isl.page_sf = page_sf;
        }
        let had = !std::mem::replace(&mut isl.solid, rects).is_empty();
        (std::mem::replace(&mut isl.editing, on), had)
    };
    if was != on {
        sh.log(&format!("island: taskbar layout {}", if on { "being edited: the window takes the mouse over its bar" } else { "edited no more" }));
    } else if had != solid && !on {
        sh.log(&format!("island: {}", if solid { "the window takes the mouse over the picker or a panel" } else { "nothing of the picker's open: the mouse as ever" }));
    }
    fit_zoom(sh);
}

// The page's zoom (its webview's): in the taskbar home, Windows' text size
// taken back out of its scale, the strip as Windows' own is whatever the text
// size; 1 elsewhere. The taskbar is laid out at 48 of the page's px, as thick
// as Windows' (TASKBAR_H, island.ts the same), the strip as thick as Windows
// gives it (island.ts --strip-h): with the text at 110% the page's px were
// 1.65 of the screen's at 150%, the strip 43.6 of them, her end 48 of them
// laid on its foot, up over its top, the flow light's ring past it
// (zhoupengjie/Wakuwaku#90); bigger text sizes, more. The text size is what
// the page's scale (page_sf) is over the display's (sf) and the zoom it has
// (zoom). None: as it is.
pub(super) fn zoom_for(taskbar: bool, sf: f64, page_sf: f64, zoom: f64) -> Option<f64> {
    let text = page_sf / (sf * zoom);
    let want = if taskbar && text > 1.0 { 1.0 / text } else { 1.0 };
    ((want - zoom).abs() > 0.001).then_some(want)
}

// The page's zoom as its home and scale want it (zoom_for), set when it is
// to change, its scale taken to be what it will be once set (the page tells
// it again then).
pub(super) fn fit_zoom(sh: &Shared) {
    let taskbar = sh.home() == "taskbar";
    let (want, text) = {
        let mut isl = sh.island.lock().unwrap();
        let Some(page_sf) = valid_scale(Some(isl.page_sf)) else { return };
        let zoom = if isl.zoom > 0.0 { isl.zoom } else { 1.0 };
        let Some(want) = zoom_for(taskbar, isl.sf(), page_sf, zoom) else { return };
        isl.zoom = want;
        isl.page_sf = page_sf / zoom * want;
        (want, page_sf / (isl.sf() * zoom))
    };
    if let Some(win) = window(sh) {
        let home = if taskbar { "taken out in the taskbar home" } else { "kept away from the taskbar" };
        match win.set_zoom(want) {
            Ok(()) => sh.log(&format!("island: page zoom {want:.4} (Windows' text size {:.0}%, {home})", text * 100.0)),
            Err(e) => sh.log(&format!("island: page zoom {want:.4} not set: {e}")),
        }
    }
    // The strips on the other displays the same.
    crate::bars::fit_zoom(sh);
}

// Windows' text size (1.1 for 110%) as the page's scale shows it, over its
// display's and its zoom; 1 until the page has told it.
pub fn text_size(sh: &Shared) -> f64 {
    let isl = sh.island.lock().unwrap();
    let zoom = if isl.zoom > 0.0 { isl.zoom } else { 1.0 };
    valid_scale(Some(isl.page_sf)).map_or(1.0, |page_sf| (page_sf / (isl.sf() * zoom)).max(1.0))
}

pub fn set_holding(sh: &Shared, is_holding: bool) {
    sh.log(&format!("island: holding {is_holding}"));
    let ignore = {
        let mut isl = sh.island.lock().unwrap();
        isl.holding = is_holding;
        if is_holding { isl.ct.force(Mode::Catch) } else { None }
    };
    set_ignore(sh, ignore);
    crate::embed::apply(sh);
}

// Where her home is on the screen, physical, whether it is up or not: what
// she is reached for from and taken back into. The island: the middle of
// its lower edge (compact), or along the foot of the screen (#105) of its
// upper edge; the taskbar: her portrait, on the strip's inner edge (above a
// bottom bar, below a top one, beside a side one).
pub fn seat(sh: &Shared) -> Option<(i32, i32)> {
    let home = sh.home();
    let edge = edge(sh);
    let area = area(sh)?;
    let thick = strip_h(sh);
    let (bar, seat_at, closed, at, low, cap) = {
        let isl = sh.island.lock().unwrap();
        // Along the foot of the screen, where the closed island's upper edge
        // lies: up from the window's foot by foot and its height (closed_rect).
        let low = isl.bottom.then(|| isl.pos.1 + isl.size.1 - ((isl.foot() + COMPACT_H) * isl.page_sf()).round() as i32);
        (isl.strip_now(edge, area.mon), isl.seat_at, closed_rect(&isl), isl.at, low, isl.cap.filter(|_| isl.embedded))
    };
    let px = |v: f64| (v * area.sf).round() as i32;
    Some(match home {
        "taskbar" => {
            let (x, y, w, h) = bar.unwrap_or_else(|| edge.strip(area.mon, px(thick)));
            let at = px(seat_at.unwrap_or(TASKBAR_SEAT));
            match edge {
                Edge::Bottom => (x + at, y),
                Edge::Top => (x + at, y + h),
                Edge::Left => (x + w, y + at),
                Edge::Right => (x, y + at),
            }
        }
        // Her capsule inside Windows' taskbar (embed.rs): the middle of its upper edge.
        _ if cap.is_some() => {
            let c = cap.unwrap_or_default();
            (c.0 + c.2 / 2, c.1)
        }
        // At a top corner (zhoupengjie/Wakuwaku#59), the middle of the closed
        // island there (closed_rect), or before the page has said its size,
        // as far in from the side as half a capsule. Along the foot of the
        // screen (low: the closed island's upper edge, #105), the middle of
        // that edge instead.
        _ => match (at, closed, low) {
            (_, Some((x, y, w, h)), None) => (x + w / 2, y + h),
            (_, Some((x, y, w, _)), Some(_)) => (x + w / 2, y),
            (Side::Center, None, None) => (area.x + area.w / 2, area.y + px(TOP + COMPACT_H)),
            (Side::Center, None, Some(y)) => (area.x + area.w / 2, y),
            (Side::Left, None, low) => (area.x + px(TOP + CAPSULE_HALF), low.unwrap_or(area.y + px(TOP + COMPACT_H))),
            (Side::Right, None, low) => (area.x + area.w - px(TOP + CAPSULE_HALF), low.unwrap_or(area.y + px(TOP + COMPACT_H))),
        },
    })
}

// Where she flies to before her home takes her in: just under the island
// (just above one along the foot of the screen, #105), just off the taskbar
// (inside it), where the drop can take her.
pub fn landing(sh: &Shared) -> Option<(i32, i32)> {
    let seat = seat(sh)?;
    let sf = area(sh).map_or(1.0, |a| a.sf);
    let off = (70.0 * sf) as i32;
    Some(match (sh.home(), edge(sh)) {
        ("taskbar", Edge::Bottom) => (seat.0, seat.1 - off),
        ("taskbar", Edge::Top) => (seat.0, seat.1 + off),
        ("taskbar", Edge::Left) => (seat.0 + off, seat.1),
        ("taskbar", Edge::Right) => (seat.0 - off, seat.1),
        _ if at_bottom(sh) => (seat.0, seat.1 - off),
        _ => (seat.0, seat.1 + off),
    })
}

// She is being moved near her home (point: her middle on the screen,
// physical), or no longer (None). Tells the page, which reaches out a drop
// for her, and says how close she is (Snap: let go, and she is back).
pub fn reach(sh: &Arc<Shared>, point: Option<(i32, i32)>, is_final: bool) -> Reach {
    let anchor = seat(sh);
    let (event, state, changed) = {
        let mut isl = sh.island.lock().unwrap();
        let sf = isl.sf();
        let was = isl.reaching;
        let state = match (point, anchor) {
            (Some(p), Some(a)) => {
                let distance = ((p.0 - a.0) as f64).hypot((p.1 - a.1) as f64) / sf;
                if distance < SNAP_PX { Reach::Snap } else if distance < REACH_PX { Reach::Near } else { Reach::Far }
            }
            _ => Reach::Far,
        };
        let event = if anchor.is_none() {
            None
        } else if state == Reach::Far {
            std::mem::take(&mut isl.reaching).then_some(Value::Null)
        } else if is_final && state == Reach::Snap {
            // Letting go close enough: the page takes over with absorb.
            None
        } else {
            isl.reaching = true;
            let p = point.unwrap();
            // Screen coordinates as the page has them (window.screenX): logical.
            Some(json!({ "x": p.0 as f64 / sf, "y": p.1 as f64 / sf, "snap": state == Reach::Snap }))
        };
        (event, state, was != isl.reaching)
    };
    if changed {
        apply_visibility(sh);
    }
    if let Some(at) = event {
        let _ = sh.app.emit_to("island", "island:reach", at);
    }
    state
}

// She has landed (or come home): whatever the home's page heard of the
// button, it is no longer held there.
pub fn end_hold(sh: &Shared) {
    let was = std::mem::take(&mut sh.island.lock().unwrap().holding);
    if was {
        let _ = sh.app.emit_to("island", "island:landed", ());
    }
}

// Take her in from where she is (her middle on the screen, physical); the
// window stays up for it until she is in (shared.rs absorb).
pub fn absorb(sh: &Arc<Shared>, point: (i32, i32)) {
    let sf = {
        let mut isl = sh.island.lock().unwrap();
        isl.reaching = false;
        isl.absorbing = true;
        isl.sf()
    };
    apply_visibility(sh);
    let _ = sh.app.emit_to("island", "island:absorb", json!({ "x": point.0 as f64 / sf, "y": point.1 as f64 / sf }));
}
