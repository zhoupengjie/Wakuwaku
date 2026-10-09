// Her home's window (the page draws it, src/pet/island.js), by display:
//   corner  a round portrait in a corner of the screen (the corner setting:
//           br, bl, tr, tl); with her out on the desktop it goes, and she
//           talks herself, with her bubble and panel
//   island  a black pill at the top centre
//   bar     a strip along the top of the screen whose height is kept from
//           other windows, as the taskbar's is (appbar.rs)
// The island and the bar stay up while she is out, and do the talking.
//
// Pulled out of her home, she becomes the pet window (carried under the
// cursor while the button is still held on the home); brought close to it
// again, it reaches for her and takes her back. From the corner, the home
// comes up for that while she is near.
//
// Sizes here are logical (the page's) unless they say physical.
use std::sync::Arc;

use serde_json::{json, Value};
use tauri::utils::config::BackgroundThrottlingPolicy;
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

use crate::pointer::{ClickThrough, Mode};
use crate::screen::Area;
use crate::{appbar, Shared};

// The window without extra room, by home (island.js BASE, CORNER_BASE): the
// island as wide as pulling her out needs and the monitor's capsule beside
// the widest island, so it only ever grows down; the
// corner's room for the hover card and for a pull. A window whose top-left
// corner moves shows its old picture from there for a frame or two, so these
// never move it. The bar's height (its width is the screen's). The page
// asks for more (pet:panel) when a shape needs it.
const ISLAND: (f64, f64) = (960.0, 132.0);
const CORNER: (f64, f64) = (760.0, 440.0);
pub const BAR_H: f64 = 30.0;
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
    // The bar's strip, as Windows granted it (physical x, y, w, h), and what
    // it was asked for (the display, the height): asked again only on a change.
    bar: Option<(i32, i32, i32, i32)>,
    bar_for: Option<((i32, i32, i32, i32), i32)>,
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

    // Physical. The bar is as wide as its display.
    fn size_for(&self, home: &str, area: &Area) -> (i32, i32) {
        let sf = self.sf();
        let base = match home {
            "corner" => CORNER,
            "bar" => (area.mon.2 as f64 / sf, BAR_H),
            _ => ISLAND,
        };
        let (w, h) = match self.room {
            Some((w, h)) => (w.max(base.0), h.max(base.1)),
            None => base,
        };
        let w = if home == "bar" { base.0 } else { w };
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
pub fn is_shown(sh: &Shared) -> bool {
    let isl = sh.island.lock().unwrap();
    sh.is_visible() || isl.temp || isl.settings_open
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

// The bar's strip: asked of Windows while the home is the bar and up; given
// back otherwise. Asked again only when the display or the height change.
fn sync_bar(sh: &Shared, is_up: bool) {
    let want = is_up && sh.home() == "bar";
    let area = if want { area(sh) } else { None };
    let mut isl = sh.island.lock().unwrap();
    let hwnd = isl.hwnd;
    if hwnd == 0 {
        return;
    }
    let Some(area) = area else {
        if isl.bar_for.take().is_some() {
            appbar::remove(hwnd);
            isl.bar = None;
            sh.log("island: bar strip given back");
        }
        return;
    };
    let ask = (area.mon, (BAR_H * area.sf).round() as i32);
    if isl.bar_for == Some(ask) {
        return;
    }
    if isl.bar_for.is_none() && !appbar::register(hwnd) {
        sh.log("island: Windows would not take the bar");
        return;
    }
    isl.bar = Some(appbar::place_top(hwnd, ask.0, ask.1));
    isl.bar_for = Some(ask);
    sh.log(&format!("island: bar strip {:?}", isl.bar));
}

// Quitting, or the window going: the bar's strip back to the other windows.
pub fn release_bar(sh: &Shared) {
    sync_bar(sh, false);
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
        sync_bar(sh, false);
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
// her portrait at its left end.
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
        _ => (area.x + area.w / 2, area.y + px(TOP + COMPACT_H)),
    })
}

// Where she flies to before her home takes her in: just under the island or
// the bar, where the drop can take her; into the corner's circle itself.
pub fn landing(sh: &Shared) -> Option<(i32, i32)> {
    let seat = seat(sh)?;
    let sf = area(sh).map_or(1.0, |a| a.sf);
    let below = match sh.home() {
        "corner" => 0.0,
        "bar" => 50.0,
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
