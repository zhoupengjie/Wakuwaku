// Her window: transparent, frameless, always on top, her whole self on the
// desktop. Where it sits and how big it is, the mouse (click-through, her
// eyes, dragging and being carried) and her walks. Up when she is out of
// her home (out); at home, her home's window has her (island.rs).
//
// Everything here is in physical pixels; the page gets logical (CSS) ones.
//
// Locks: a window getter waits on the main thread, and menu clicks run there
// and take these locks; so no getter is ever called with a lock held, and no
// two locks are held at once.
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::utils::config::BackgroundThrottlingPolicy;
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::pointer::{ClickThrough, Mode};
use crate::{island, now_ms, Shared};

// Window size at scale 1: room for the bubble above a 192x208 cell, and
// wide enough that the bubble says something before it has to cut it short.
const BASE_W: f64 = 360.0;
const BASE_H: f64 = 320.0;
const CELL_H: f64 = 208.0;
// The cursor further than this (logical px) from her face does not catch her eye.
const LOOK_FAR: f64 = 900.0;
// Two clicks on her this close together are a double-click.
const DOUBLE_MS: u64 = 500;
// How long a click on her still goes to the session she showed as the pointer came.
const CLICK_GOES_MS: u64 = 60_000;
// Flying back into the island, from wherever she is.
const FLY_MS: f64 = 380.0;
// Room under her feet (logical) for the hint while she is carried out of
// her home: a double-click sends her back (pet.js, style.css #her-hint).
const HINT_H: f64 = 30.0;

struct Drag {
    // The cursor's offset in the window.
    dx: i32,
    dy: i32,
    x: i32,
    y: i32,
    x0: i32,
    y0: i32,
    moved: bool,
    // Pulled out of the island: the button is the island's, she takes no mouse.
    carried: bool,
}

#[derive(Default)]
pub struct Win {
    pos: (i32, i32),
    size: (i32, i32),
    sf: f64,
    ready: bool,
    shown: bool,
    ct: ClickThrough,
    drag: Option<Drag>,
    walk_gen: u64,
    walking: bool,
    flying: bool,
    last_click: Option<Instant>,
    // The session she was showing when the pointer came over her (seeing it
    // sends it to rest), and when: a click on her goes to its window.
    click_goes: Option<(String, Instant)>,
    // Watching the button while the pointer is on her (see poll).
    watching: bool,
    was_down: bool,
    // The pointer is on her herself, not on what is over her head.
    on_her: bool,
    // The room (logical) over her head for what is there, as the page
    // measured it: her words, her list, a prompt panel (pet.js room).
    panel: Option<(f64, f64)>,
    // How far she sits right of the window's centre: when the screen edge
    // stops the grown window from centring on her, she shifts so she stays put.
    shift: i32,
    // Where the window was before the room over her head grew it, to go back
    // to exactly; and where she stood (her middle, her feet) as it was last
    // grown: moved since, she goes back to where she is instead.
    parked: Option<((i32, i32), (i32, i32))>,
    // Room under her feet (physical) while she is carried out: the hint.
    below: i32,
}

// Her window's display's scale.
pub fn sf(sh: &Shared) -> f64 {
    sh.win.lock().unwrap().sf()
}

impl Win {
    fn sf(&self) -> f64 {
        if self.sf > 0.0 { self.sf } else { 1.0 }
    }

    fn base_size(&self, scale: f64) -> (i32, i32) {
        ((BASE_W * scale * self.sf()).round() as i32, (BASE_H * scale * self.sf()).round() as i32)
    }

    // The window now: taller, and wider, should what is over her head need
    // more room than her own window has above her; taller by the room under
    // her while she is carried out (below).
    fn size_for(&self, scale: f64) -> (i32, i32) {
        let base = self.base_size(scale);
        let (w, h) = match self.panel {
            Some((w, h)) => (base.0.max((w * self.sf()).ceil() as i32), base.1.max(((CELL_H * scale + h) * self.sf()).ceil() as i32)),
            None => base,
        };
        (w, h + self.below)
    }

    // Her middle and her feet on the screen.
    fn feet(&self) -> (i32, i32) {
        (self.pos.0 + self.size.0 / 2 + self.shift, self.pos.1 + self.size.1 - self.below)
    }

    // Room under her feet for the hint, or none: the window grows or
    // shrinks at its bottom, she where she is.
    fn hint(&mut self, on: bool, scale: f64) {
        self.below = if on { (HINT_H * self.sf()).round() as i32 } else { 0 };
        self.size = self.size_for(scale);
    }

    // The room over her head as measured (set_panel): the window's size and
    // place for it, kept on screen by clamp, she staying put. False when
    // nothing changes.
    fn make_room(&mut self, measured: Option<(f64, f64)>, scale: f64, clamp: impl Fn((i32, i32), (i32, i32)) -> (i32, i32)) -> bool {
        if self.panel == measured {
            return false;
        }
        let old = self.pos;
        let (pet_x, pet_bottom) = self.feet();
        // Where to go back to once nothing is over her head: where she was
        // before it came, unless she has been moved since (dragged, a walk:
        // her words are up all through a turn); then where she is now.
        let parked = match self.parked.take() {
            Some((back, at)) if at == (pet_x, pet_bottom) => Some(back),
            _ if self.panel.is_none() => Some(old),
            _ => None,
        };
        self.panel = measured;
        self.size = self.size_for(scale);
        match (measured, parked) {
            (None, Some(back)) => {
                self.pos = back;
                self.shift = 0;
            }
            (panel, back) => {
                let want_x = pet_x - self.size.0 / 2;
                let p = clamp((want_x, pet_bottom + self.below - self.size.1), self.size);
                self.pos = p;
                self.shift = if panel.is_some() { want_x - p.0 } else { 0 };
                if panel.is_some() {
                    let base = self.base_size(scale);
                    let back = back.unwrap_or((pet_x - base.0 / 2, pet_bottom - base.1));
                    self.parked = Some((back, self.feet()));
                }
            }
        }
        true
    }

    // Her middle on the screen, for the island to reach for.
    fn her_point(&self, scale: f64) -> (i32, i32) {
        let (x, y) = self.feet();
        (x, y - (CELL_H * scale * self.sf() / 2.0).round() as i32)
    }
}

fn window(sh: &Shared) -> Option<WebviewWindow> {
    sh.app.get_webview_window("pet")
}

fn scale(sh: &Shared) -> f64 {
    sh.setting("scale").as_f64().unwrap_or(0.55)
}

// Up: she is out of her home.
pub fn is_shown(sh: &Shared) -> bool {
    sh.is_visible() && sh.flag("out")
}

// The display her window is on (x, y, w, h).
pub fn display(sh: &Shared) -> Option<(i32, i32, i32, i32)> {
    let (pos, size) = {
        let win = sh.win.lock().unwrap();
        (win.pos, win.size)
    };
    sh.screens().near(pos.0 + size.0 / 2, pos.1 + size.1 / 2).map(|a| a.mon)
}

fn set_ignore(sh: &Shared, ignore: Option<bool>) {
    if let (Some(ignore), Some(win)) = (ignore, window(sh)) {
        sh.log(&format!("pet: mouse {}", if ignore { "passes through" } else { "caught" }));
        let _ = win.set_ignore_cursor_events(ignore);
    }
}

fn place(sh: &Shared, (x, y): (i32, i32)) {
    if let Some(win) = window(sh) {
        // On a Mac a window's physical place is in its own display's scale,
        // which changes as she crosses to a display of another: placed in
        // points, from the scale her place was reckoned in (Win::sf), she
        // stays under the pointer across (dragged over, she drifted away
        // from it, 2026-10-14). Elsewhere the physical place is one space.
        #[cfg(target_os = "macos")]
        {
            let sf = sh.win.lock().unwrap().sf();
            let _ = win.set_position(tauri::LogicalPosition::new(x as f64 / sf, y as f64 / sf));
        }
        #[cfg(not(target_os = "macos"))]
        let _ = win.set_position(PhysicalPosition::new(x, y));
    }
}

// Move the window, kept on screen unless `free`.
fn move_to(sh: &Shared, x: i32, y: i32, free: bool) -> (i32, i32) {
    let screens = sh.screens();
    let p = {
        let mut w = sh.win.lock().unwrap();
        let p = if free { (x, y) } else { screens.clamp((x, y), w.size) };
        w.pos = p;
        p
    };
    place(sh, p);
    p
}

// Save where the window is, as where it would be without a prompt panel.
fn remember(sh: &Shared) {
    let scale = scale(sh);
    let (x, y) = {
        let w = sh.win.lock().unwrap();
        let base = w.base_size(scale);
        let (x, y) = w.feet();
        (x - base.0 / 2, y - base.1)
    };
    let mut settings = sh.settings.lock().unwrap();
    settings.insert("x".into(), json!(x));
    settings.insert("y".into(), json!(y));
    crate::data::save(&sh.dir, &settings);
}

// Bottom right of the primary display.
fn home_spot(sh: &Shared) -> (i32, i32) {
    let primary = sh.screens().primary;
    let w = sh.win.lock().unwrap();
    let margin = (24.0 * w.sf()).round() as i32;
    match primary {
        Some(a) => (a.x + a.w - w.size.0 - margin, a.y + a.h - w.size.1 - margin),
        None => (0, 0),
    }
}

// Where she was last left, or home.
fn saved_spot(sh: &Shared) -> (i32, i32) {
    let spot = match (sh.setting("x").as_i64(), sh.setting("y").as_i64()) {
        (Some(x), Some(y)) => (x as i32, y as i32),
        _ => home_spot(sh),
    };
    let size = sh.win.lock().unwrap().size;
    sh.screens().clamp(spot, size)
}

// --- The window ---------------------------------------------------------------------

pub fn create(sh: &Shared) -> tauri::Result<()> {
    let primary = sh.screens().primary;
    {
        let mut w = sh.win.lock().unwrap();
        w.sf = primary.map_or(1.0, |a| a.sf);
        w.size = w.base_size(scale(sh));
        w.ct.force(Mode::Pass);
    }
    let pos = saved_spot(sh);
    let (size, sf) = {
        let mut w = sh.win.lock().unwrap();
        w.pos = pos;
        (w.size, w.sf())
    };

    let win = WebviewWindowBuilder::new(&sh.app, "pet", WebviewUrl::App("pet/index.html?role=pet".into()))
        .title("wakuwaku")
        .inner_size(size.0 as f64 / sf, size.1 as f64 / sf)
        .additional_browser_args(crate::webview::browser_args(sh))
        .data_directory(crate::webview::webview_dir(sh))
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
        // An unfocused, half-transparent window still has to animate.
        .background_throttling(BackgroundThrottlingPolicy::Disabled)
        .build()?;
    win.set_size(PhysicalSize::new(size.0 as u32, size.1 as u32))?;
    win.set_position(PhysicalPosition::new(pos.0, pos.1))?;
    // Clicks pass through until the pointer is on her.
    win.set_ignore_cursor_events(true)?;
    sh.own_window(&win);
    // Its renderer gone: told, and the page loaded again (crash.rs).
    crate::crash::watch(&crate::shared(&sh.app), &win);
    Ok(())
}

// Whether her page has said it is ready (for /health).
pub fn is_ready(sh: &Shared) -> bool {
    sh.win.lock().unwrap().ready
}

// Her page has its listeners up: draw and show (her home says hello).
pub fn ready(sh: &Shared) {
    sh.log("pet: page ready");
    sh.win.lock().unwrap().ready = true;
    sh.redraw();
    // Her window made hidden, its page was not: put away with it while she
    // is home (as she starts, mostly; apply_visibility has nothing to change).
    if !is_shown(sh) {
        if let Some(win) = window(sh) {
            hide_window(&win);
        }
    }
    apply_visibility(sh);
}

pub fn apply_visibility(sh: &Shared) {
    let want = is_shown(sh);
    {
        let mut w = sh.win.lock().unwrap();
        if !w.ready || w.shown == want {
            return;
        }
        w.shown = want;
        if !want {
            w.drag = None;
            w.walk_gen += 1;
            w.walking = false;
        }
    }
    if let Some(win) = window(sh) {
        if want {
            let _ = win.as_ref().show();
            sh.show_window(&win);
        } else {
            hide_window(&win);
        }
    }
    // Hidden, her page heard no updates (shared.rs to_pages): the whole state
    // now, whether quiet with it; and her home told she is up or not (herUp:
    // who holds a prompt), an app full screen changing it as well.
    sh.redraw();
}

// Her window put away, and its page with it (WebView2's IsVisible): with
// the window alone hidden, the page went on playing her clips and walking
// her about unseen, its timers never throttled (#14: half her renderer's and
// most of her GPU process's time while she was home).
fn hide_window(win: &WebviewWindow) {
    let _ = win.hide();
    let _ = win.as_ref().hide();
}

// Her window on the screen now. Asked from anywhere (shared.rs to_pages),
// perhaps while the window's state is held: then taken as up.
pub fn is_up(sh: &Shared) -> bool {
    sh.win.try_lock().map(|w| w.shown).unwrap_or(true)
}

// A new size keeps her spot: her bottom centre.
pub fn resize(sh: &Arc<Shared>, scale: f64) {
    sh.change(json!({ "scale": scale }));
    let screens = sh.screens();
    let (p, size) = {
        let mut w = sh.win.lock().unwrap();
        let old = w.size;
        w.size = w.size_for(scale);
        let (x, y) = w.pos;
        let p = screens.clamp((x + (old.0 - w.size.0) / 2, y + old.1 - w.size.1), w.size);
        w.pos = p;
        (p, w.size)
    };
    if let Some(win) = window(sh) {
        let _ = win.set_size(PhysicalSize::new(size.0 as u32, size.1 as u32));
    }
    place(sh, p);
    remember(sh);
}

pub fn come_home(sh: &Shared) {
    stop_walking(sh);
    let home = home_spot(sh);
    move_to(sh, home.0, home.1, false);
    remember(sh);
}

// Back to where she was left (out of the island, or the pet again).
pub fn return_to_spot(sh: &Shared) {
    let scale = scale(sh);
    let size = {
        let mut w = sh.win.lock().unwrap();
        w.panel = None;
        w.parked = None;
        w.shift = 0;
        w.size = w.size_for(scale);
        w.size
    };
    let _ = sh.app.emit_to("pet", "pet:shift", 0);
    if let Some(win) = window(sh) {
        let _ = win.set_size(PhysicalSize::new(size.0 as u32, size.1 as u32));
    }
    let p = saved_spot(sh);
    move_to(sh, p.0, p.1, false);
}

// The room over her head (logical) for what is there (her words, her list,
// a prompt panel), as the page measured it, or None once nothing is. More
// than her window has above her grows it upwards and both ways, her staying put.
pub fn set_panel(sh: &Shared, measured: Option<(f64, f64)>) {
    let scale = scale(sh);
    let screens = sh.screens();
    let (pos, size, shift, sf) = {
        let mut w = sh.win.lock().unwrap();
        if !w.make_room(measured, scale, |p, size| screens.clamp(p, size)) {
            return;
        }
        (w.pos, w.size, w.shift, w.sf())
    };
    if let Some(win) = window(sh) {
        let _ = win.set_size(PhysicalSize::new(size.0 as u32, size.1 as u32));
    }
    place(sh, pos);
    let _ = sh.app.emit_to("pet", "pet:shift", (shift as f64 / sf).round());
}

// Minimized by another program (island.rs MINIMIZED, Claude's computer use):
// left so till it is given back, not put back on screen from (-32000, -32000)
// at its size, minimized still, nor that place remembered.
#[cfg(windows)]
static MINIMIZED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

// Put the window back on screen at its own size if anything moved it off or
// stretched it (a display unplugged, the scale changed).
pub fn keep_on_screen(sh: &Shared) {
    let Some(win) = window(sh) else { return };
    {
        let w = sh.win.lock().unwrap();
        if !w.ready || !w.shown || w.drag.is_some() || w.walking || w.flying {
            return;
        }
    }
    #[cfg(windows)]
    {
        use std::sync::atomic::Ordering;
        let minimized = win.hwnd().is_ok_and(|h| crate::frameless::minimized(h.0 as isize));
        if MINIMIZED.swap(minimized, Ordering::SeqCst) != minimized {
            sh.log(if minimized { "pet: minimized by another program; left so till it is given back" } else { "pet: given back after it was minimized" });
        }
        if minimized {
            return;
        }
    }
    let (Ok(pos), Ok(size)) = (win.outer_position(), win.outer_size()) else { return };
    let screens = sh.screens();
    let scale = scale(sh);
    let fix = {
        let mut w = sh.win.lock().unwrap();
        if w.drag.is_some() || w.walking || w.flying {
            return;
        }
        let centre = (pos.x + size.width as i32 / 2, pos.y + size.height as i32 / 2);
        if let Some(a) = screens.near(centre.0, centre.1) {
            w.sf = a.sf;
        }
        w.size = w.size_for(scale);
        let is_stretched = (size.width as i32 - w.size.0).abs() > 3 || (size.height as i32 - w.size.1).abs() > 3;
        let p = screens.clamp((pos.x, pos.y), w.size);
        w.pos = p;
        (is_stretched || p != (pos.x, pos.y)).then_some((p, w.size, is_stretched))
    };
    if let Some((p, size, is_stretched)) = fix {
        sh.log(&format!("pet: back on screen at {p:?}{}", if is_stretched { ", resized" } else { "" }));
        let _ = win.set_size(PhysicalSize::new(size.0 as u32, size.1 as u32));
        place(sh, p);
        remember(sh);
    }
}

// --- The pointer ----------------------------------------------------------------------

// Whether the primary button is held right now, wherever the pointer is
// (None: cannot tell). With the buttons swapped, the primary is the right.
#[cfg(windows)]
pub fn primary_down() -> Option<bool> {
    #[link(name = "user32")]
    extern "system" {
        fn GetAsyncKeyState(key: i32) -> i16;
        fn GetSystemMetrics(index: i32) -> i32;
    }
    // SAFETY: plain user32 calls with no pointers.
    unsafe {
        let key = if GetSystemMetrics(23) != 0 { 0x02 } else { 0x01 };
        Some(GetAsyncKeyState(key) as u16 & 0x8000 != 0)
    }
}

#[cfg(not(windows))]
pub fn primary_down() -> Option<bool> {
    None
}

// Whether any of the mouse's buttons is held right now: the left, the
// right, the middle (a tab closed), the two at the side.
#[cfg(windows)]
pub fn any_down() -> Option<bool> {
    #[link(name = "user32")]
    extern "system" {
        fn GetAsyncKeyState(key: i32) -> i16;
    }
    // SAFETY: a plain user32 call with no pointers.
    Some([0x01, 0x02, 0x04, 0x05, 0x06].iter().any(|&key| unsafe { GetAsyncKeyState(key) } as u16 & 0x8000 != 0))
}

#[cfg(not(windows))]
pub fn any_down() -> Option<bool> {
    None
}

// One look at the mouse (cursor in physical px): the window's mode, the
// page's pointer, her eyes, a drag. Returns ms until the next look.
pub fn poll(sh: &Arc<Shared>, cursor: (i32, i32)) -> u64 {
    let is_idle = sh.pet.lock().unwrap().mood(now_ms()) == "idle";
    let look = sh.flag("look");
    let scale = scale(sh);
    let down = primary_down();

    let mut events: Vec<(&str, Value)> = Vec::new();
    let mut moved_to = None;
    let mut ignore = None;
    let mut reach = None;
    let mut let_go_now = false;
    let mut press_now = false;
    let wait = {
        let mut w = sh.win.lock().unwrap();
        let sf = w.sf();
        if !w.ready {
            200
        } else if !w.shown {
            ignore = w.ct.force(Mode::Pass);
            w.watching = false;
            500
        } else if w.flying {
            16
        } else if let Some(carried) = w.drag.as_ref().map(|d| d.carried) {
            // Carried, the button is the island's: she takes no mouse at all,
            // so she never sees half of a press.
            ignore = w.ct.force(if carried { Mode::Pass } else { Mode::Catch });
            // Follow the cursor from here, so a flick that outruns the page's
            // own mouse events still carries her; the button up anywhere ends it.
            if down == Some(false) {
                let_go_now = true;
            } else {
                let d = w.drag.as_mut().unwrap();
                let (nx, ny) = (cursor.0 - d.dx, cursor.1 - d.dy);
                if (nx, ny) != (d.x, d.y) {
                    d.moved = d.moved || (nx - d.x0).abs() + (ny - d.y0).abs() > (4.0 * sf) as i32;
                    let moved = d.moved;
                    if moved {
                        events.push(("pet:drag", json!(nx - d.x)));
                    }
                    d.x = nx;
                    d.y = ny;
                    w.pos = (nx, ny);
                    moved_to = Some((nx, ny));
                    if moved {
                        reach = Some(w.her_point(scale));
                    }
                }
            }
            16
        } else {
            let (pos, size) = (w.pos, w.size);
            let seen = w.ct.look(cursor, pos, size, sf, sf, false, false, &[]);
            if w.ct.calmed() {
                sh.log(&format!("pet: mouse mode bouncing, calmed: cursor {cursor:?}, window {pos:?} {size:?}, scale {sf}"));
            }
            ignore = seen.ignore;
            if let Some(at) = seen.pointer {
                events.push(("pet:pointer", at));
            }
            // After she has been in the island and out again, Windows can eat
            // the button-down on her window (the page sees only the button-up).
            // So while the pointer is on her, the button itself is watched too,
            // and a press starts the drag, whichever hears it first.
            let is_down = down == Some(true);
            // On her alone: a press on what is over her head (her words, her
            // list, a prompt's buttons) is on that.
            if w.ct.over && w.on_her {
                if w.watching && is_down && !w.was_down {
                    press_now = true;
                }
                w.watching = true;
            } else {
                w.watching = false;
            }
            w.was_down = is_down;
            // Her eyes follow only while idle (up at her list, too).
            let mut is_looking = false;
            if seen.moved && look && is_idle {
                let face = (pos.0 as f64 + size.0 as f64 / 2.0 + w.shift as f64, pos.1 as f64 + size.1 as f64 - CELL_H * scale * sf * 0.62);
                let (dx, dy) = ((cursor.0 as f64 - face.0) / sf, (cursor.1 as f64 - face.1) / sf);
                if dx.hypot(dy) <= LOOK_FAR {
                    events.push(("pet:cursor", json!({ "dx": dx, "dy": dy })));
                    is_looking = true;
                }
            }
            if w.ct.over { 16 } else if seen.forward { 30 } else if seen.near || is_looking { 80 } else { 200 }
        }
    };

    set_ignore(sh, ignore);
    if let Some(p) = moved_to {
        place(sh, p);
    }
    for (name, payload) in events {
        let _ = sh.app.emit_to("pet", name, payload);
    }
    if let Some(p) = reach {
        island::reach(sh, Some(p), false);
    }
    if press_now {
        sh.log("pet: press seen from the button");
        drag_start(sh);
    }
    if let_go_now {
        let_go(sh);
    }
    wait
}

// Being dragged from her own window (not carried out of the island).
pub fn is_dragged(sh: &Shared) -> bool {
    sh.win.lock().unwrap().drag.as_ref().is_some_and(|d| !d.carried)
}

// The pointer on her, or off; on what is over her head (her = false): that
// catches the mouse too, but no press there drags her, and it is not her
// seen (her words stay to be read and pressed).
pub fn hover(sh: &Shared, is_over: bool, her: bool) {
    let ignore = {
        let mut w = sh.win.lock().unwrap();
        let (held, passive) = match &w.drag {
            Some(d) => (!d.carried, d.carried),
            None => (false, false),
        };
        w.on_her = is_over && her;
        w.ct.set_over(is_over, held, passive)
    };
    set_ignore(sh, ignore);
    if is_over && !her {
        return;
    }
    if is_over {
        let shown = {
            let now = sh.pet.lock().unwrap().get(now_ms());
            let id = now["id"].as_str().unwrap_or("");
            (now["mood"] != "idle" && now["jump"] == true && !id.is_empty()).then(|| id.to_string())
        };
        if let Some(id) = shown {
            sh.win.lock().unwrap().click_goes = Some((id, Instant::now()));
        }
    }
    // The pointer on her: you have seen what she had to say.
    if is_over && sh.pet.lock().unwrap().seen(now_ms()) {
        sh.redraw();
    }
}

pub fn drag_start(sh: &Shared) {
    let Some((cx, cy)) = crate::screen::cursor(sh, sf(sh)) else { return };
    let ignore = {
        let mut w = sh.win.lock().unwrap();
        // Already started from the button: the same press.
        if w.drag.is_some() || !w.shown || w.flying {
            return;
        }
        w.walk_gen += 1;
        w.walking = false;
        let (x, y) = w.pos;
        w.drag = Some(Drag { dx: cx - x, dy: cy - y, x, y, x0: x, y0: y, moved: false, carried: false });
        w.ct.force(Mode::Catch)
    };
    sh.log("pet: drag start");
    set_ignore(sh, ignore);
}

// Let go: back into the island if she is close enough to it, else she lands
// where she is, kept on screen. A click makes her jump, and goes to the
// window of the session she shows; two, see double_click.
pub fn let_go(sh: &Arc<Shared>) {
    let scale = scale(sh);
    let (drag, is_double, her) = {
        let mut w = sh.win.lock().unwrap();
        let Some(drag) = w.drag.take() else { return };
        let is_double = !drag.moved && !drag.carried && w.last_click.is_some_and(|t| t.elapsed() < Duration::from_millis(DOUBLE_MS));
        if !drag.moved && !drag.carried {
            w.last_click = if is_double { None } else { Some(Instant::now()) };
        }
        w.was_down = false;
        (drag, is_double, w.her_point(scale))
    };
    sh.log(&format!("pet: let go, {}{}", if drag.moved { "moved" } else { "a click" }, if drag.carried { " (carried)" } else { "" }));
    let _ = sh.app.emit_to("pet", "pet:drag-end", ());
    if drag.carried {
        island::end_hold(sh);
        // Let go: the hint under her goes (absorbed, it goes with her).
        unhint(sh);
    }
    if drag.moved {
        // Not into a notch's out-of-sight home by a drag (notch::in_notch).
        if !crate::notch::in_notch() && island::reach(sh, Some(her), true) == island::Reach::Snap {
            return crate::shared::absorb(sh, her);
        }
        island::reach(sh, None, false);
    }
    move_to(sh, drag.x, drag.y, false);
    remember(sh);
    if is_double {
        return double_click(sh);
    }
    // A click: a jump. Carried out of the island: she lands with one.
    if !drag.moved || drag.carried {
        sh.react("jump", json!(""));
    }
    // A click, and no second one after it: to the window of the session she
    // was showing (jump.rs).
    if !drag.moved && !drag.carried {
        let sh = sh.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(DOUBLE_MS + 20));
            let goes = {
                let mut w = sh.win.lock().unwrap();
                let is_single = w.last_click.is_some_and(|t| t.elapsed() >= Duration::from_millis(DOUBLE_MS));
                match w.click_goes.take() {
                    Some((id, at)) if is_single && at.elapsed() < Duration::from_millis(CLICK_GOES_MS) => Some(id),
                    other => {
                        w.click_goes = other;
                        None
                    }
                }
            };
            if let Some(id) = goes {
                crate::commands::jump_to(&sh, &id);
            }
        });
    }
}

// A double-click: she flies back into her home (else the settings, grown
// out of a home risen for them).
pub fn double_click(sh: &Arc<Shared>) {
    if sh.flag("out") && crate::notch::in_notch() {
        // Her home in a Mac's notch is out of sight: she stays, and the
        // island drops open from it to show what is going on (island.js peek):
        // open, not toggled, so a second double-click does not shut it again.
        sh.log("pet: double-click; the island drops open from the notch");
        let _ = sh.app.emit_to("island", "island:peek", serde_json::json!({ "open": true }));
    } else if sh.flag("out") {
        fly_home(sh);
    } else {
        island::open_settings(sh, None);
    }
}

// Back to her home from wherever she is: drawn in faster and faster,
// running that way, her home reaching out for her; then it takes her in.
// Her spot on the desktop stays where it was, for when she is let out again.
fn fly_home(sh: &Arc<Shared>) {
    let Some(end) = island::landing(sh) else { return };
    let scale = scale(sh);
    let (start, offset, end) = {
        let mut w = sh.win.lock().unwrap();
        if w.flying || !w.shown {
            return;
        }
        w.flying = true;
        w.drag = None;
        w.walk_gen += 1;
        w.walking = false;
        let start = w.her_point(scale);
        (start, (start.0 - w.pos.0, start.1 - w.pos.1), end)
    };
    let dir = if end.0 == start.0 { 1 } else { end.0 - start.0 };
    let sh = sh.clone();
    std::thread::spawn(move || {
        let started = Instant::now();
        let mut p;
        loop {
            let t = (started.elapsed().as_secs_f64() * 1000.0 / FLY_MS).min(1.0);
            let k = t * t;
            p = (start.0 + ((end.0 - start.0) as f64 * k) as i32, start.1 + ((end.1 - start.1) as f64 * k) as i32);
            move_to(&sh, p.0 - offset.0, p.1 - offset.1, true);
            let _ = sh.app.emit_to("pet", "pet:drag", dir);
            island::reach(&sh, Some(p), false);
            if t >= 1.0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(16));
        }
        sh.win.lock().unwrap().flying = false;
        let _ = sh.app.emit_to("pet", "pet:drag-end", ());
        crate::shared::absorb(&sh, p);
    });
}

// Pulled out of the island, the button still held on the island: up she
// comes under the cursor, her middle where the drop had it (offset, logical
// px from the cursor), and follows it.
pub fn carry(sh: &Arc<Shared>, offset: &Value) {
    let Some(cursor) = crate::screen::cursor(sh, sf(sh)) else { return };
    let scale = scale(sh);
    let (p, size) = {
        let mut w = sh.win.lock().unwrap();
        let sf = w.sf();
        // Room under her for the hint while the button is held (let_go).
        w.hint(true, scale);
        let dx = offset["dx"].as_f64().unwrap_or(0.0) * sf;
        let dy = offset["dy"].as_f64().unwrap_or(0.0) * sf;
        let x = cursor.0 + dx as i32 - w.size.0 / 2 - w.shift;
        let y = cursor.1 + dy as i32 - (w.size.1 - w.below - (CELL_H * scale * sf / 2.0) as i32);
        w.pos = (x, y);
        w.walk_gen += 1;
        w.walking = false;
        w.drag = Some(Drag { dx: cursor.0 - x, dy: cursor.1 - y, x, y, x0: x, y0: y, moved: true, carried: true });
        w.ct.reset(true);
        ((x, y), w.size)
    };
    sh.log(&format!("pet: carried out of the island at {p:?}"));
    let _ = sh.app.emit_to("pet", "pet:carried", json!({ "on": true }));
    if let Some(win) = window(sh) {
        let _ = win.set_size(PhysicalSize::new(size.0 as u32, size.1 as u32));
    }
    place(sh, p);
    set_ignore(sh, Some(true));
    sh.change(json!({ "out": true }));
}

// The room under her for the hint given back (let go, gone home): the
// window shrinks at its bottom, she where she is.
fn unhint(sh: &Shared) {
    let scale = scale(sh);
    let size = {
        let mut w = sh.win.lock().unwrap();
        if w.below == 0 {
            return;
        }
        w.hint(false, scale);
        w.size
    };
    let _ = sh.app.emit_to("pet", "pet:carried", json!({ "on": false }));
    if let Some(win) = window(sh) {
        let _ = win.set_size(PhysicalSize::new(size.0 as u32, size.1 as u32));
    }
}

// Gone into the island at once, before the island's animation starts.
pub fn hide_now(sh: &Shared) {
    {
        let mut w = sh.win.lock().unwrap();
        w.drag = None;
        w.walk_gen += 1;
        w.walking = false;
        w.shown = false;
        w.ct.reset(true);
    }
    set_ignore(sh, Some(true));
    if let Some(win) = window(sh) {
        hide_window(&win);
    }
    unhint(sh);
}

// --- Walks ------------------------------------------------------------------------------

// A short walk: move the window by dx (logical) over ms, stopping at the
// screen edge. True when it got there, false when stopped on the way.
pub fn walk(sh: &Shared, dx: f64, ms: f64) -> bool {
    if !is_shown(sh) {
        return false;
    }
    let screens = sh.screens();
    let (gen, x0, y0, target) = {
        let mut w = sh.win.lock().unwrap();
        if !w.shown || w.drag.is_some() || w.flying {
            return false;
        }
        w.walk_gen += 1;
        w.walking = true;
        let (x0, y0) = w.pos;
        let target = screens.clamp((x0 + (dx * w.sf()).round() as i32, y0), w.size).0;
        (w.walk_gen, x0, y0, target)
    };
    let started = Instant::now();
    let ms = ms.max(1.0);
    let arrived = loop {
        std::thread::sleep(Duration::from_millis(16));
        let t = (started.elapsed().as_secs_f64() * 1000.0 / ms).min(1.0);
        let p = {
            let mut w = sh.win.lock().unwrap();
            if w.walk_gen != gen || w.drag.is_some() || w.flying {
                break false;
            }
            let p = screens.clamp((x0 + ((target - x0) as f64 * t).round() as i32, y0), w.size);
            w.pos = p;
            p
        };
        place(sh, p);
        if t >= 1.0 {
            break true;
        }
    };
    {
        let mut w = sh.win.lock().unwrap();
        if w.walk_gen == gen {
            w.walking = false;
        }
    }
    remember(sh);
    arrived
}

pub fn stop_walking(sh: &Shared) {
    let mut w = sh.win.lock().unwrap();
    w.walk_gen += 1;
    w.walking = false;
}

// Where the window is, for checks.
pub fn where_(sh: &Shared) -> Value {
    let actual = window(sh).and_then(|win| win.outer_position().ok()).map(|p| json!([p.x, p.y]));
    let w = sh.win.lock().unwrap();
    json!({
        "actual": actual,
        "pos": [w.pos.0, w.pos.1],
        "size": [w.size.0, w.size.1],
        "scaleFactor": w.sf(),
        "shown": w.shown,
        "over": w.ct.over,
        "walking": w.walking,
        "flying": w.flying,
        "dragging": w.drag.as_ref().map(|d| if d.carried { "carried" } else { "dragged" }),
        "room": w.panel.map(|(rw, rh)| json!([rw, rh])),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // A 2560×1380 work area, at 125% as on the user's screen.
    fn clamp(p: (i32, i32), size: (i32, i32)) -> (i32, i32) {
        (p.0.max(0).min(2560 - size.0), p.1.max(0).min(1380 - size.1))
    }

    fn feet(w: &Win) -> (i32, i32) {
        w.feet()
    }

    fn standing(at: (i32, i32)) -> Win {
        let mut w = Win { sf: 1.25, ..Win::default() };
        w.size = w.base_size(0.75);
        w.pos = at;
        w
    }

    #[test]
    fn room_over_her_head_grows_her_window_and_she_stays_put() {
        let mut w = standing((1700, 900));
        let (start, stood) = (w.pos, feet(&w));
        // Her words fit in the room her window has above her: no bigger.
        assert!(w.make_room(Some((200.0, 40.0)), 0.75, clamp));
        assert_eq!((w.pos, w.size), (start, w.base_size(0.75)));
        // A prompt: taller and wider, she where she was.
        assert!(w.make_room(Some((340.0, 220.0)), 0.75, clamp));
        assert!(w.size.0 > 338 && w.size.1 > 300, "{:?}", w.size);
        assert_eq!(feet(&w), stood);
        // Gone: back exactly.
        assert!(w.make_room(None, 0.75, clamp));
        assert_eq!((w.pos, w.size), (start, w.base_size(0.75)));
        assert!(!w.make_room(None, 0.75, clamp));
    }

    #[test]
    fn moved_while_her_words_are_up_she_stays_where_she_was_moved_to() {
        let mut w = standing((1700, 900));
        w.make_room(Some((340.0, 220.0)), 0.75, clamp);
        // Dragged 400 px left and 100 up meanwhile.
        w.pos = (w.pos.0 - 400, w.pos.1 - 100);
        let stood = feet(&w);
        w.make_room(Some((300.0, 60.0)), 0.75, clamp);
        assert_eq!(feet(&w), stood);
        w.make_room(None, 0.75, clamp);
        assert_eq!((feet(&w), w.size), (stood, w.base_size(0.75)));
    }

    #[test]
    fn with_no_room_above_her_she_steps_down_for_it_and_back_up_after() {
        let mut w = standing((1700, 10));
        let (start, stood) = (w.pos, feet(&w));
        w.make_room(Some((340.0, 220.0)), 0.75, clamp);
        assert_eq!(w.pos.1, 0);
        assert!(feet(&w).1 > stood.1);
        w.make_room(None, 0.75, clamp);
        assert_eq!(w.pos, start);
    }

    #[test]
    fn carried_out_the_room_for_the_hint_is_under_her_feet() {
        let mut w = standing((1700, 900));
        let stood = feet(&w);
        w.hint(true, 0.75);
        assert_eq!((feet(&w), w.size.1), (stood, w.base_size(0.75).1 + 38));
        // A prompt comes meanwhile: over her head, she where she was.
        w.make_room(Some((340.0, 220.0)), 0.75, clamp);
        assert_eq!(feet(&w), stood);
        // Let go, then the prompt answered: all back as it was.
        w.hint(false, 0.75);
        assert_eq!(feet(&w), stood);
        w.make_room(None, 0.75, clamp);
        assert_eq!((w.pos, w.size), ((1700, 900), w.base_size(0.75)));
    }
}
