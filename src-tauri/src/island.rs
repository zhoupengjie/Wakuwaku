// Her home's window (the page draws it, src/pet/island.js), by display:
//   corner  a round portrait in a corner of the screen (the corner setting:
//           br, bl, tr, tl); with her out on the desktop it goes, and she
//           talks herself, with her bubble and panel
//   island  a black pill at the top centre
//   bar     a strip along the top of the screen whose height is kept from
//           other windows, as the taskbar's is (appbar.rs)
//   taskbar a strip along the bottom in place of Windows' own, which is put
//           away while it is up, its tray taken over (taskbar.rs); what
//           opens grows up from it, into room kept above it
// The island, the bar and the taskbar stay up while she is out, and do the
// talking.
//
// Pulled out of her home, she becomes the pet window (carried under the
// cursor while the button is still held on the home); brought close to it
// again, it reaches for her and takes her back. From the corner, the home
// comes up for that while she is near.
//
// Sizes here are logical (the page's) unless they say physical.
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;

use serde_json::{json, Value};
use tauri::utils::config::BackgroundThrottlingPolicy;
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

use crate::pointer::{ClickThrough, Mode};
use crate::screen::Area;
use crate::{appbar, taskbar, Shared};

// The window without extra room, by home (island.js BASE, CORNER_BASE): the
// island as wide as pulling her out needs, so it only ever grows down; the
// corner's room for the hover card and for a pull. A window whose top-left
// corner moves shows its old picture from there for a frame or two, so these
// never move it. The bar's height (its width is the screen's). The page
// asks for more (pet:panel) when a shape needs it.
const ISLAND: (f64, f64) = (760.0, 132.0);
const CORNER: (f64, f64) = (760.0, 440.0);
pub const BAR_H: f64 = 30.0;
// The taskbar's strip (as high as Windows' own, so the Start menu and the
// quick settings open just above it), and the room above it for what opens
// (the corner's, for the same reason as the corner's: a window growing up
// moves its top-left corner). The window is the two.
pub const TASKBAR_H: f64 = 48.0;
const TASKBAR_ROOM: f64 = 440.0;
// The middle of her portrait at the taskbar's left end.
const TASKBAR_SEAT_X: f64 = 24.0;
// The island's top edge in the window, and its compact height.
const TOP: f64 = 8.0;
const COMPACT_H: f64 = 36.0;
// The corner's circle, and how far it keeps from the screen's edges.
const CIRCLE: f64 = 56.0;
const CORNER_M: f64 = 14.0;
// The middle of her portrait at the bar's left end.
const BAR_SEAT_X: f64 = 17.0;
// Her middle this close to her home: it reaches out for her; this close,
// letting go puts her back in.
const REACH_PX: f64 = 280.0;
const SNAP_PX: f64 = 140.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Reach {
    Far,
    Near,
    Snap,
}

#[derive(Default)]
pub struct Island {
    // Physical.
    pos: (i32, i32),
    size: (i32, i32),
    sf: f64,
    // Extra room the page asked for.
    room: Option<(f64, f64)>,
    created: bool,
    ready: bool,
    shown: bool,
    ct: ClickThrough,
    // The window, once made (for the app bar).
    hwnd: isize,
    // The bar's or the taskbar's strip, as Windows granted it (physical x,
    // y, w, h), and what it was asked for (the display, the height, the
    // home): asked again only on a change.
    bar: Option<(i32, i32, i32, i32)>,
    bar_for: Option<((i32, i32, i32, i32), i32, &'static str)>,
    // The button held on her (a pull, or carrying her out): the window keeps the pointer.
    holding: bool,
    reaching: bool,
    // Taking her in: the window stays up for it (the corner's goes once she is out).
    pub absorbing: bool,
    // Risen only for the settings (she is out from the corner, or out of
    // sight): it goes again once they close.
    pub temp: bool,
    pub settings_open: bool,
    // Settings asked for before its page was up: the page they open on.
    pending: Option<String>,
}

impl Island {
    fn sf(&self) -> f64 {
        if self.sf > 0.0 { self.sf } else { 1.0 }
    }

    // Physical. The bar and the taskbar are as wide as their display.
    fn size_for(&self, home: &str, area: &Area) -> (i32, i32) {
        let sf = self.sf();
        let base = match home {
            "corner" => CORNER,
            "bar" => (area.mon.2 as f64 / sf, BAR_H),
            "taskbar" => (area.mon.2 as f64 / sf, TASKBAR_H + TASKBAR_ROOM),
            _ => ISLAND,
        };
        let (w, h) = match self.room {
            Some((w, h)) => (w.max(base.0), h.max(base.1)),
            None => base,
        };
        let w = if home == "bar" || home == "taskbar" { base.0 } else { w };
        ((w * sf).round() as i32, (h * sf).round() as i32)
    }
}

fn window(sh: &Shared) -> Option<WebviewWindow> {
    sh.app.get_webview_window("island")
}

// Up whenever she is in sight, home or out (out from the corner, it keeps
// her portrait while she does the talking); or risen for the settings; or
// the settings open in it, whatever they just hid (she, do not disturb):
// they never go from under the person, only once closed.
// The taskbar is up whatever hides her (out of sight, do not disturb): it is
// the person's taskbar; only an app full screen hides it, as Windows' own.
pub fn is_shown(sh: &Shared) -> bool {
    let up = if sh.home() == "taskbar" { !sh.is_fullscreen() } else { sh.is_visible() };
    let isl = sh.island.lock().unwrap();
    up || isl.temp || isl.settings_open
}

// Its window's handle, once made (0 before).
pub fn hwnd(sh: &Shared) -> isize {
    sh.island.lock().unwrap().hwnd
}

// Its window is up now (shown).
pub fn is_up_now(sh: &Shared) -> bool {
    sh.island.lock().unwrap().shown
}

// Where its window is (physical) and its scale: the page's px to the screen's.
pub fn origin(sh: &Shared) -> ((i32, i32), f64) {
    let isl = sh.island.lock().unwrap();
    (isl.pos, isl.sf())
}

fn set_ignore(sh: &Shared, ignore: Option<bool>) {
    if let (Some(ignore), Some(win)) = (ignore, window(sh)) {
        sh.log(&format!("island: mouse {}", if ignore { "passes through" } else { "caught" }));
        let _ = win.set_ignore_cursor_events(ignore);
    }
}

// The display it is on: where it is, else the primary one.
fn area(sh: &Shared) -> Option<Area> {
    let screens = sh.screens();
    let at = {
        let isl = sh.island.lock().unwrap();
        (isl.created && isl.size.0 > 0).then(|| (isl.pos.0 + isl.size.0 / 2, isl.pos.1 + isl.size.1 / 2))
    };
    match at {
        Some((x, y)) => screens.near(x, y),
        None => screens.primary,
    }
}

// Which corner, for the corner home.
fn corner(sh: &Shared) -> String {
    sh.setting("corner").as_str().filter(|c| matches!(*c, "br" | "bl" | "tr" | "tl")).unwrap_or("br").to_string()
}

// Where the window goes, by home. Returns the physical rect if it moved.
fn fit(sh: &Shared) -> Option<((i32, i32), (i32, i32))> {
    let (home, corner) = (sh.home(), corner(sh));
    let area = area(sh)?;
    let mut isl = sh.island.lock().unwrap();
    isl.sf = area.sf;
    let size = isl.size_for(home, &area);
    let pos = match home {
        // Its corner of the work area, the page drawing the circle in from it.
        "corner" => (
            if corner.ends_with('l') { area.x } else { area.x + area.w - size.0 },
            if corner.starts_with('t') { area.y } else { area.y + area.h - size.1 },
        ),
        // The strip Windows granted, or the display's top until then.
        "bar" => isl.bar.map_or((area.mon.0, area.mon.1), |b| (b.0, b.1)),
        // The window's foot on the strip's, the room above it.
        "taskbar" => isl.bar.map_or((area.mon.0, area.mon.1 + area.mon.3 - size.1), |b| (b.0, b.1 + b.3 - size.1)),
        // The top centre, below a taskbar that sits at the top.
        _ => (area.x + (area.w - size.0) / 2, area.y),
    };
    if (pos, size) == (isl.pos, isl.size) {
        return None;
    }
    isl.pos = pos;
    isl.size = size;
    Some((pos, size))
}

// One at a time: two at once took the taskbar twice (two tray threads, one
// never stopped). Asked while one runs, it runs again after, with the last
// ask: 1 give back, 2 keep.
static SYNCING: AtomicBool = AtomicBool::new(false);
static SYNC_ASK: AtomicU8 = AtomicU8::new(0);

fn sync_bar(sh: &Shared, keep: bool) {
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

// The strip, the bar's along the top or the taskbar's along the bottom:
// asked of Windows while the home is one of those and keep (up; for the
// taskbar, also while an app full screen hides its window: giving the strip
// back would move every window), given back otherwise. Asked again only
// when the display, the height or the home change. The taskbar's comes
// with Windows' own put away first and goes with it given back (taskbar.rs).
fn sync_bar_now(sh: &Shared, keep: bool) {
    let home = sh.home();
    let edge = match home {
        "bar" => Some((appbar::Edge::Top, BAR_H)),
        "taskbar" => Some((appbar::Edge::Bottom, TASKBAR_H)),
        _ => None,
    };
    let want = match (keep, edge) {
        (true, Some((edge, h))) => area(sh).map(|a| (a, edge, (h * a.sf).round() as i32)),
        _ => None,
    };
    let (hwnd, held) = {
        let isl = sh.island.lock().unwrap();
        (isl.hwnd, isl.bar_for)
    };
    if hwnd == 0 {
        return;
    }
    let ask = want.map(|(a, _, h)| (a.mon, h, home));
    if held == ask {
        return;
    }
    // Another home's strip (or none wanted): given back first.
    if let Some((_, _, was)) = held.filter(|h| Some(h.2) != ask.map(|a| a.2)) {
        appbar::remove(hwnd);
        {
            let mut isl = sh.island.lock().unwrap();
            isl.bar = None;
            isl.bar_for = None;
        }
        if was == "taskbar" {
            taskbar::give_back(sh);
        }
        sh.log(&format!("island: {was} strip given back"));
    }
    let Some((area, edge, height)) = want else { return };
    let mon_bottom = area.mon.1 + area.mon.3;
    if home == "taskbar" {
        // Windows' own away first: the strip then goes to the bottom, not above its room.
        taskbar::take(sh, (area.mon.0, mon_bottom - height, area.mon.2, height), mon_bottom);
    }
    let registered = sh.island.lock().unwrap().bar_for.is_some();
    if !registered && !appbar::register(hwnd) {
        sh.log("island: Windows would not take the bar");
        return;
    }
    let granted = appbar::place(hwnd, area.mon, edge, height);
    {
        let mut isl = sh.island.lock().unwrap();
        isl.bar = Some(granted);
        isl.bar_for = ask;
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
        isl.hwnd
    };
    // Should it remember after all, a second registration would be refused.
    appbar::remove(hwnd);
    place(sh);
}

fn place(sh: &Shared) {
    sync_bar(sh, true);
    let Some((pos, size)) = fit(sh) else { return };
    if let Some(win) = window(sh) {
        let _ = win.set_size(PhysicalSize::new(size.0 as u32, size.1 as u32));
        let _ = win.set_position(PhysicalPosition::new(pos.0, pos.1));
    }
}

// Made the first time her home shows, then kept (hidden while she talks
// herself), so coming back is instant.
fn create(sh: &Arc<Shared>) {
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
        let built = WebviewWindowBuilder::new(&sh.app, "island", WebviewUrl::App("pet/index.html?role=island".into()))
            .title("wakuwaku island")
            .inner_size(size.0 as f64 / sf, size.1 as f64 / sf)
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
                let _ = win.set_size(PhysicalSize::new(size.0 as u32, size.1 as u32));
                let _ = win.set_position(PhysicalPosition::new(pos.0, pos.1));
                let _ = win.set_ignore_cursor_events(true);
                // A click anywhere else takes the focus from the settings: they close.
                let app = sh.app.clone();
                win.on_window_event(move |event| {
                    // Not when it is her own window being shown (Shared::show_window).
                    if let WindowEvent::Focused(false) = event {
                        if !crate::shared(&app).is_shuffling() {
                            let _ = app.emit_to("island", "island:blur", ());
                        }
                    }
                });
                #[cfg(windows)]
                if let Ok(hwnd) = win.hwnd() {
                    sh.island.lock().unwrap().hwnd = hwnd.0 as isize;
                }
                sh.own_window(&win);
            }
            Err(err) => sh.log(&format!("island: could not make its window: {err}")),
        }
    });
}

// Its page has its listeners up: draw, show, say hello.
pub fn ready(sh: &Arc<Shared>) {
    sh.log("island: page ready");
    let pending = {
        let mut isl = sh.island.lock().unwrap();
        isl.ready = true;
        isl.pending.take()
    };
    sh.redraw();
    taskbar::resend(sh);
    if !sh.she_talks() {
        sh.greet();
    }
    apply_visibility(sh);
    if let Some(tab) = pending {
        let _ = sh.app.emit_to("island", "island:settings", json!({ "tab": tab }));
    }
}

pub fn apply_visibility(sh: &Arc<Shared>) {
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
        // The taskbar keeps its strip while an app full screen hides it.
        sync_bar(sh, sh.home() == "taskbar");
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
}

// Kept in its place (a display gone, the scale changed, another bar).
pub fn keep_on_screen(sh: &Shared) {
    let is_up = {
        let isl = sh.island.lock().unwrap();
        isl.shown && !isl.holding && !isl.reaching
    };
    if is_up {
        place(sh);
    }
}

// One look at the mouse. Returns ms until the next look.
pub fn poll(sh: &Shared, cursor: (i32, i32)) -> u64 {
    // She is being dragged past: the home is not being hovered.
    let passive = crate::pet::is_dragged(sh);
    let (ignore, pointer, wait) = {
        let mut isl = sh.island.lock().unwrap();
        if !isl.ready {
            return 500;
        }
        if !isl.shown {
            (isl.ct.force(Mode::Pass), None, 500)
        } else {
            let (pos, size, sf, held) = (isl.pos, isl.size, isl.sf(), isl.holding);
            let seen = isl.ct.look(cursor, pos, size, sf, held, passive && !held);
            let wait = if held || isl.ct.over { 50 } else if seen.forward { 30 } else if seen.near { 80 } else { 200 };
            (seen.ignore, seen.pointer, wait)
        }
    };
    set_ignore(sh, ignore);
    if let Some(at) = pointer {
        let _ = sh.app.emit_to("island", "pet:pointer", at);
    }
    wait
}

pub fn hover(sh: &Shared, is_over: bool) {
    let ignore = {
        let mut isl = sh.island.lock().unwrap();
        let held = isl.holding;
        isl.ct.set_over(is_over, held, false)
    };
    set_ignore(sh, ignore);
    // The home opens up to say what happened while hovered, so it counts
    // as seen once the pointer leaves.
    if !is_over && sh.pet.lock().unwrap().seen(crate::now_ms(), sh.hold()) {
        sh.redraw();
    }
}

// The room a shape needs, as the page measured it.
pub fn set_room(sh: &Shared, measured: &Value) {
    let room = match (measured["width"].as_f64(), measured["height"].as_f64()) {
        (Some(w), Some(h)) if w > 0.0 && h > 0.0 => Some((w.ceil(), h.ceil())),
        _ => None,
    };
    sh.island.lock().unwrap().room = room;
    place(sh);
}

// The button is held on her in her home (a pull, or carrying her out).
pub fn set_holding(sh: &Shared, is_holding: bool) {
    sh.log(&format!("island: holding {is_holding}"));
    let ignore = {
        let mut isl = sh.island.lock().unwrap();
        isl.holding = is_holding;
        if is_holding { isl.ct.force(Mode::Catch) } else { None }
    };
    set_ignore(sh, ignore);
}

// Where her home is on the screen, physical, whether it is up or not: what
// she is reached for from and taken back into. The island: the middle of
// its lower edge (compact); the corner: the circle's middle; the bar: under
// her portrait at its left end; the taskbar: above it.
pub fn seat(sh: &Shared) -> Option<(i32, i32)> {
    let (home, corner) = (sh.home(), corner(sh));
    let area = area(sh)?;
    let bar = sh.island.lock().unwrap().bar;
    let px = |v: f64| (v * area.sf).round() as i32;
    Some(match home {
        "corner" => {
            let m = px(CORNER_M + CIRCLE / 2.0);
            (
                if corner.ends_with('l') { area.x + m } else { area.x + area.w - m },
                if corner.starts_with('t') { area.y + m } else { area.y + area.h - m },
            )
        }
        "bar" => {
            let (x, y, _, h) = bar.unwrap_or((area.mon.0, area.mon.1, area.mon.2, px(BAR_H)));
            (x + px(BAR_SEAT_X), y + h)
        }
        "taskbar" => {
            let (x, y, _, _) = bar.unwrap_or((area.mon.0, area.mon.1 + area.mon.3 - px(TASKBAR_H), area.mon.2, px(TASKBAR_H)));
            (x + px(TASKBAR_SEAT_X), y)
        }
        _ => (area.x + area.w / 2, area.y + px(TOP + COMPACT_H)),
    })
}

// Where she flies to before her home takes her in: just under the island or
// the bar, just above the taskbar, where the drop can take her; into the
// corner's circle itself.
pub fn landing(sh: &Shared) -> Option<(i32, i32)> {
    let seat = seat(sh)?;
    let sf = area(sh).map_or(1.0, |a| a.sf);
    let below = match sh.home() {
        "corner" => 0.0,
        "bar" => 50.0,
        "taskbar" => -70.0,
        _ => 70.0,
    };
    Some((seat.0, seat.1 + (below * sf) as i32))
}

// She is being moved near her home (point: her middle on the screen,
// physical), or no longer (None). Tells the page, which reaches out a drop
// for her, and says how close she is (Snap: let go, and she is back). The
// corner's home comes up for it while she is near.
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
// window stays up for it until she is in (main.rs absorb).
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

// --- The settings, grown out of her home ------------------------------------------------

// Open them on a page (or where they were). With no home up (she is out
// from the corner, or out of sight), one rises for them and goes once they close.
pub fn open_settings(sh: &Arc<Shared>, tab: Option<&str>) {
    let is_up = sh.is_visible();
    let ready = {
        let mut isl = sh.island.lock().unwrap();
        isl.temp |= !is_up;
        isl.settings_open = true;
        if !isl.ready {
            isl.pending = Some(tab.unwrap_or("").to_string());
        }
        isl.ready
    };
    sh.log(&format!("island: settings open on {:?}{}", tab, if is_up { "" } else { " (risen for them)" }));
    apply_visibility(sh);
    sh.redraw();
    if ready {
        let _ = sh.app.emit_to("island", "island:settings", json!({ "tab": tab.unwrap_or("") }));
    }
}

// The page closed them (Esc, a click outside, ✕, the head). Once the home
// has shrunk back: a risen one goes, and so does one the settings kept up
// (she was hidden, or do not disturb turned on, while they were open).
pub fn settings_closed(sh: &Arc<Shared>) {
    sh.island.lock().unwrap().settings_open = false;
    sh.log("island: settings closed");
    if !sh.flag("onboarded") {
        sh.change(json!({ "onboarded": true }));
    } else {
        sh.redraw();
    }
    let sh = sh.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(620));
        {
            let mut isl = sh.island.lock().unwrap();
            if isl.settings_open {
                return;
            }
            isl.temp = false;
        }
        apply_visibility(&sh);
        sh.redraw();
    });
}

pub fn where_(sh: &Shared) -> Value {
    let actual = window(sh).and_then(|win| win.outer_position().ok()).map(|p| json!([p.x, p.y]));
    let isl = sh.island.lock().unwrap();
    json!({
        "actual": actual,
        "pos": [isl.pos.0, isl.pos.1],
        "size": [isl.size.0, isl.size.1],
        "room": isl.room.map(|(w, h)| json!([w, h])),
        "shown": isl.shown,
        "over": isl.ct.over,
        "holding": isl.holding,
        "reaching": isl.reaching,
        "temp": isl.temp,
        "settingsOpen": isl.settings_open,
        "bar": isl.bar.map(|b| json!([b.0, b.1, b.2, b.3])),
    })
}
