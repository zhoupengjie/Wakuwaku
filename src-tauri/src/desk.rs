// The desktop's widgets (the widget design of 2026-10-16, §7; desk.js draws
// them): a window for each display that has some (desk-1, desk-2, ...:
// pet/desk.html), as big as the display, clear but for the widgets.
//
// Where it lies: over the wallpaper and the desktop's icons, under every
// other window, and still seen when the desktop is shown (Win+D). Not a
// child of the desktop's windows (Progman, or a WorkerW): a window of
// another process under them shares its input with Explorer's, and 24H2
// laid them out anew; a window of her own is kept right over the one that
// holds the icons instead (keep_low): placed after the window over that one
// whenever the window in front changes (a WinEvent hook, no polling). Shown
// the desktop, Windows puts that window over all the others: hers go right
// over it again, topmost if it is. It never takes the focus (not focusable:
// WS_EX_NOACTIVATE, nor in Alt+Tab), nor shows on the taskbar.
//
// Its clicks: through it but where a widget is. The page says where they
// are (desk_shapes: rounded rects, and an open menu); the pointer's own
// loop (main.rs) asks poll() where the pointer is, and the window takes the
// mouse while it is on one, lets it through elsewhere; held during a press
// (desk_hold), so a drag keeps it anywhere. With Ctrl held, its bare desktop
// too (off Explorer's icons), for widgets to be picked with a rectangle.
//
// A widget dragged from her home's widget picker over a display: told to
// that display's page, its window made for it if there was none, and the
// desktop's windows raised over the others meanwhile ("A widget dragged
// from her home" below).
//
// Its glass: each display's own wallpaper (wallpaper.rs, as the strip's
// Mica takes it), told to its page when it changes (desk:paper; looked at
// as Windows says the picture or the displays changed, and every half
// minute for a slideshow); desk.js blurs it and gives each widget its part.
//
// The layout is the settings' deskLayout (settings.rs is_desk_layout); a page
// keeps what it changed (desk_keep, merged with the rest), and her island's
// page puts widgets on and off the desktop (island.js placeWidget).
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use serde_json::{json, Value};
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::{screen::Area, Shared};

type Mon = (i32, i32, i32, i32);

struct Desk {
    label: String,
    key: String,
    mon: Mon,
    sf: f64,
    hwnd: isize,
    ready: bool,
    // What its page was last told.
    sent: Value,
    paper: Value,
    // Where its widgets are (the page's px: x, y, w, h, radius), and an
    // open menu among them.
    shapes: Vec<[f64; 5]>,
    menu: bool,
    held: bool,
    // The window takes the mouse.
    catching: bool,
    was_down: bool,
    // Some of its widgets picked with Ctrl (desk_picked): a press off them
    // goes on to Explorer, and the page hears of it (desk:outside).
    picked: bool,
    // The desktop's icons on its display as its page last had them read
    // (desk_icons; its px), and when: Ctrl over one of them is Explorer's.
    icons: Vec<[f64; 4]>,
    icons_at: Option<std::time::Instant>,
    // The entries it shows (refresh), and words not sent it while its
    // desktop was not seen (push_widgets).
    entries: Vec<Value>,
    stale: bool,
    // Its work area (physical): the display less its taskbar's strip.
    work: Mon,
    // Whether its cards' plugin views may show, as its page was last told
    // (views; none told: they may).
    views_on: Option<bool>,
}

static DESKS: Mutex<Vec<Desk>> = Mutex::new(Vec::new());
static NEXT: AtomicU32 = AtomicU32::new(1);
static SHARED: OnceLock<Arc<Shared>> = OnceLock::new();

// The pointer this close to a widget (the page's px): looked at often.
const NEAR_PX: f64 = 40.0;

// Each display with what it shows, from the layout: its own, and on the
// main display those whose display is not there (deskgrid.js shownHere);
// and one a widget is being dragged over from her home, though nothing is
// on it yet (Drag keep).
fn wanted(sh: &Shared) -> Vec<(Area, String, bool)> {
    if !cfg!(windows) {
        return Vec::new();
    }
    let layout = sh.setting("deskLayout");
    let entries: &[Value] = layout.as_array().map_or(&[], Vec::as_slice);
    let held = DRAG.lock().unwrap().keep.clone();
    if entries.is_empty() && held.is_empty() {
        return Vec::new();
    }
    let screens = sh.screens();
    let keyed: Vec<(Area, String, bool)> = screens.areas.iter().map(|a| (*a, key_of(sh, a.mon), screens.primary.is_some_and(|p| p.mon == a.mon))).collect();
    let keys: Vec<&str> = keyed.iter().map(|(_, k, _)| k.as_str()).collect();
    keyed.iter().filter(|(_, key, primary)| held.contains(key) || entries.iter().any(|e| shown_on(e, key, *primary, &keys))).cloned().collect()
}

// An entry on a display: its own, and on the main display one whose display
// is not there (deskgrid.js shownHere).
fn shown_on(e: &Value, key: &str, primary: bool, keys: &[&str]) -> bool {
    match e["d"].as_str() {
        Some(d) => d == key || (primary && !keys.contains(&d)),
        None => primary,
    }
}

// Made or taken away as the layout and the displays say; each one told
// what it shows.
pub fn sync(sh: &Arc<Shared>) {
    let _ = SHARED.set(sh.clone());
    let want = wanted(sh);
    let (gone, made) = {
        let mut desks = DESKS.lock().unwrap();
        let mut gone = Vec::new();
        desks.retain(|d| {
            let keep = want.iter().any(|(a, key, _)| *key == d.key && a.mon == d.mon && a.sf == d.sf);
            if !keep {
                gone.push((d.label.clone(), d.hwnd));
            }
            keep
        });
        let mut made = Vec::new();
        for (a, key, _) in &want {
            if !desks.iter().any(|d| d.key == *key) {
                let label = format!("desk-{}", NEXT.fetch_add(1, Ordering::SeqCst));
                desks.push(Desk { label: label.clone(), key: key.clone(), mon: a.mon, sf: a.sf, hwnd: 0, ready: false, sent: Value::Null, paper: Value::Null, shapes: Vec::new(), menu: false, held: false, catching: false, was_down: false, picked: false, icons: Vec::new(), icons_at: None, entries: Vec::new(), stale: false, work: (a.x, a.y, a.w, a.h), views_on: None });
                made.push((label, *a));
            }
        }
        (gone, made)
    };
    for (label, _) in &gone {
        if let Some(win) = sh.app.get_webview_window(label) {
            let _ = win.destroy();
        }
        sh.log(&format!("desk: {label} gone"));
    }
    {
        let mut desks = DESKS.lock().unwrap();
        for d in desks.iter_mut() {
            if let Some((a, _, _)) = want.iter().find(|(a, _, _)| a.mon == d.mon) {
                d.work = (a.x, a.y, a.w, a.h);
            }
        }
        // None left: nothing of the desktop's runs (no thread, no hook, no timer).
        if desks.is_empty() && !gone.is_empty() {
            imp::stop();
        }
    }
    for (label, area) in made {
        create(sh, label, area);
    }
    refresh(sh);
}

// Each page told what it shows, when that changed: its display, her
// language, Windows' mode, the specifics, her home, her sheet, the layout.
pub fn refresh(sh: &Shared) {
    let ready: Vec<(String, String, Mon, f64)> = DESKS.lock().unwrap().iter().filter(|d| d.ready).map(|d| (d.label.clone(), d.key.clone(), d.mon, d.sf)).collect();
    if ready.is_empty() {
        return;
    }
    let screens = sh.screens();
    let keys: Vec<String> = screens.areas.iter().map(|a| key_of(sh, a.mon)).collect();
    let layout = sh.setting("deskLayout");
    let (light, accent) = look_now();
    let sprite = sprite_url(sh);
    for (label, key, mon, sf) in ready {
        let area = screens.areas.iter().find(|a| a.mon == mon).copied();
        let work = area.map_or(json!([0, 0, mon.2 as f64 / sf, mon.3 as f64 / sf]), |a| json!([(a.x - mon.0) as f64 / sf, (a.y - mon.1) as f64 / sf, a.w as f64 / sf, a.h as f64 / sf]));
        let state = json!({
            "key": key,
            "primary": screens.primary.is_some_and(|p| p.mon == mon),
            "keys": keys,
            "sf": sf,
            "size": [mon.2 as f64 / sf, mon.3 as f64 / sf],
            "work": work,
            "lang": sh.lang(),
            "light": light,
            // Windows' accent, as taskbar:look has it (what is picked with
            // Ctrl, the rectangle: desk.js).
            "accent": accent,
            "detailed": sh.setting("details") != false,
            "home": sh.home(),
            // What the taskbar shows while it is her home (a board's Add says where each one is).
            "bar": if sh.home() == "taskbar" { sh.setting("taskbarLayout") } else { Value::Null },
            "sprite": sprite,
            "layout": if layout.is_array() { layout.clone() } else { json!([]) },
        });
        let key_refs: Vec<&str> = keys.iter().map(String::as_str).collect();
        let primary = screens.primary.is_some_and(|p| p.mon == mon);
        let here: Vec<Value> = layout.as_array().into_iter().flatten().filter(|e| shown_on(e, &key, primary, &key_refs)).cloned().collect();
        let changed = {
            let mut desks = DESKS.lock().unwrap();
            match desks.iter_mut().find(|d| d.label == label) {
                Some(d) => {
                    d.entries = here;
                    let changed = d.sent != state;
                    if changed {
                        d.sent = state.clone();
                    }
                    changed
                }
                None => false,
            }
        };
        if changed {
            emit(sh, &label, "desk:state", state);
        }
    }
}

// Her sheet, for today's glyph (her face).
fn sprite_url(sh: &Shared) -> Value {
    let id = sh.setting("pet");
    let id = id.as_str().unwrap_or("");
    crate::data::pets(&sh.dir).iter().find(|p| p.id == id).map_or(Value::Null, |p| json!(p.url()))
}

// Windows' mode and accent ({ base, light, dark }, or null), as last read
// (read again as Windows says its colours changed).
static LOOK: Mutex<Option<(bool, Value)>> = Mutex::new(None);
fn look_now() -> (bool, Value) {
    LOOK.lock().unwrap().get_or_insert_with(look_read).clone()
}
fn look_read() -> (bool, Value) {
    let (light, accent) = crate::wallpaper::look();
    (light, accent.map_or(Value::Null, |a| json!({ "base": a.base, "light": a.light, "dark": a.dark })))
}

// The widgets' words, to the pages (with the island's, main.rs redraw_widgets):
// not to one whose display's desktop nobody sees (a window maximized or full
// screen over it: the design session, 2026-10-16), which is told them once
// it is seen again (catch_up).
pub fn push_widgets(sh: &Shared) {
    let desks: Vec<(String, Mon)> = DESKS.lock().unwrap().iter().filter(|d| d.ready).map(|d| (d.label.clone(), d.mon)).collect();
    if desks.is_empty() {
        return;
    }
    count(&PUSHES);
    let mut widgets = None;
    let mut waits = false;
    for (label, mon) in desks {
        let seen = seen_now(sh, mon);
        if let Some(d) = DESKS.lock().unwrap().iter_mut().find(|d| d.label == label) {
            waits |= !seen && !d.stale;
            d.stale = !seen;
        }
        if seen {
            count(&SENT);
            let w = widgets.get_or_insert_with(|| json!({ "widgets": sh.widgets_said() }));
            emit(sh, &label, "desk:widgets", w.clone());
        }
    }
    if waits {
        imp::arm_stale();
    }
    views(sh);
}

// Those not told while unseen, told now if they are seen again; the
// monitor woken to read for them at once. Looked at as the window in front
// changes, one is minimized or restored, and every ten seconds while one
// waits.
pub fn catch_up(sh: &Shared) {
    *SEEN.lock().unwrap() = None;
    let stale: Vec<Mon> = DESKS.lock().unwrap().iter().filter(|d| d.ready && d.stale).map(|d| d.mon).collect();
    if stale.iter().any(|&mon| seen_now(sh, mon)) {
        push_widgets(sh);
        crate::wake_monitor();
    } else {
        views(sh);
    }
}

// Whether each page's cards may show their plugins' views (tier 2,
// src/pet/views.js), told as it changes (desk:views { on }): not while
// nobody is about, nor while its desktop is not seen (as its words are not
// sent then: push_widgets), nor while the island's watchdog has them taken
// away (addons.rs pause_views). Looked at only while a plugin with a view
// runs: with none, nothing is.
pub fn views(sh: &Shared) {
    if !crate::addons::views_running() {
        return;
    }
    let desks: Vec<(String, Mon, bool)> = DESKS.lock().unwrap().iter().filter(|d| d.ready).map(|d| (d.label.clone(), d.mon, d.views_on.unwrap_or(true))).collect();
    let away = sh.is_quiet() || crate::addons::views_paused();
    for (label, mon, told) in desks {
        let on = !away && seen_now(sh, mon);
        if let Some(d) = DESKS.lock().unwrap().iter_mut().find(|d| d.label == label) {
            d.views_on = Some(on);
        }
        if on != told {
            emit(sh, &label, "desk:views", json!({ "on": on }));
        }
    }
}

// The pages' open menus closed, those picked with Ctrl let go (another
// window came to the front: a press on it, or on the desktop's own).
fn close_menus(sh: &Shared) {
    let open: Vec<String> = DESKS.lock().unwrap().iter().filter(|d| d.menu || d.picked).map(|d| d.label.clone()).collect();
    for label in open {
        emit(sh, &label, "desk:outside", Value::Null);
    }
}

fn any_stale() -> bool {
    DESKS.lock().unwrap().iter().any(|d| d.stale)
}

pub fn any() -> bool {
    !DESKS.lock().unwrap().is_empty()
}

// Whether an entry shows a widget: put there by itself, or its plugin's;
// alone or in a stack.
fn entry_shows(e: &Value, id: &str, owner: Option<&str>) -> bool {
    let shows = |m: &Value| match (m["type"].as_str(), m["of"].as_str()) {
        (Some("widget"), Some(of)) => of == id,
        (Some("plugin"), Some(of)) => Some(of) == owner,
        _ => false,
    };
    shows(e) || e["items"].as_array().is_some_and(|items| items.iter().any(shows))
}

// The same, as it shows: big, medium or large only; a board's tiles each at
// their own size, none while it is folded (its title bar alone).
fn entry_shows_at(e: &Value, id: &str, owner: Option<&str>, big: bool) -> bool {
    let big_enough = |size: &Value| !big || matches!(size.as_str(), Some("m" | "l"));
    if e["type"] == "board" {
        return e["folded"] != true && e["items"].as_array().is_some_and(|tiles| tiles.iter().any(|t| big_enough(&t["size"]) && entry_shows(t, id, owner)));
    }
    big_enough(&e["size"]) && entry_shows(e, id, owner)
}

// Whether a display's desktop shows a widget where somebody can see it.
// big: medium or large only (the monitor's card's readings, its graphics
// card and drives, are shown there: read only then).
fn seen_with(sh: &Shared, id: &str, big: bool) -> bool {
    let owner = crate::plugins::owner_of(id);
    let owner = owner.as_deref();
    let desks: Vec<(Mon, bool)> = DESKS.lock().unwrap().iter().filter(|d| d.ready).map(|d| (d.mon, d.entries.iter().any(|e| entry_shows_at(e, id, owner, big)))).collect();
    desks.into_iter().any(|(mon, has)| has && seen_now(sh, mon))
}

pub fn seen(sh: &Shared, id: &str) -> bool {
    seen_with(sh, id, false)
}

// The monitor on the desktop medium or large, where it is seen: its card's
// readings (the graphics card, the drives) read, at the monitor's pace;
// not while a window covers that desktop, nor while nobody is about.
pub fn shows_more(sh: &Shared, id: &str) -> bool {
    seen_with(sh, id, true)
}

// Each display's desktop seen or not: nothing maximized or full screen over
// it, or the desktop brought over the windows (Win+D). Looked at once a
// second at most (a window list: some 400 windows, a fraction of a ms).
static SEEN: Mutex<Option<(std::time::Instant, Vec<(Mon, bool)>)>> = Mutex::new(None);
fn seen_now(sh: &Shared, mon: Mon) -> bool {
    if let Some((at, list)) = SEEN.lock().unwrap().as_ref() {
        if at.elapsed() < std::time::Duration::from_secs(1) {
            if let Some((_, s)) = list.iter().find(|(m, _)| *m == mon) {
                return *s;
            }
        }
    }
    count(&LOOKS);
    let own = sh.own.lock().unwrap().clone();
    let shown = imp::desktop_shown(&own);
    let full = crate::fullscreen::covering(&own).flatten();
    let mons: Vec<Mon> = DESKS.lock().unwrap().iter().map(|d| d.mon).collect();
    let list: Vec<(Mon, bool)> = mons.into_iter().map(|m| (m, shown || !(full == Some(m) || crate::tasks::maximized_on(m)))).collect();
    let s = list.iter().find(|(m, _)| *m == mon).map_or(true, |(_, s)| *s);
    *SEEN.lock().unwrap() = Some((std::time::Instant::now(), list));
    s
}

fn create(sh: &Arc<Shared>, label: String, area: Area) {
    let sh = sh.clone();
    std::thread::spawn(move || {
        let built = WebviewWindowBuilder::new(&sh.app, &label, WebviewUrl::App("pet/desk.html".into()))
            .title("wakuwaku desk")
            .inner_size(area.mon.2 as f64 / area.sf, area.mon.3 as f64 / area.sf)
            .additional_browser_args(crate::browser_args(&sh))
            .data_directory(crate::webview_dir(&sh))
            .transparent(true)
            .decorations(false)
            .resizable(false)
            .maximizable(false)
            .minimizable(false)
            .skip_taskbar(true)
            .shadow(false)
            .focusable(false)
            .focused(false)
            .visible(false)
            .build();
        let win = match built {
            Ok(win) => win,
            Err(err) => return sh.log(&format!("desk: {label} not made: {err}")),
        };
        #[cfg(windows)]
        let hwnd = win.hwnd().map(|h| h.0 as isize).unwrap_or(0);
        #[cfg(not(windows))]
        let hwnd = 0;
        crate::tasks::no_transitions(hwnd);
        sh.own_window(&win);
        // Its cards' plugin views kept to their own page (addons.rs guard_frames).
        crate::addons::guard_frames(&sh, &win, false);
        lay(&win, area.mon);
        let _ = win.set_ignore_cursor_events(true);
        let gone = {
            let mut desks = DESKS.lock().unwrap();
            match desks.iter_mut().find(|d| d.label == label) {
                Some(d) => {
                    d.hwnd = hwnd;
                    false
                }
                None => true,
            }
        };
        if gone {
            let _ = win.destroy();
            return;
        }
        // Shown without taking the focus from anything (imp::show_quiet), on
        // its own thread after what tao was asked above (set there in turn:
        // shown before, tao's style for the clicks hid it again), then put low.
        #[cfg(windows)]
        let _ = sh.app.run_on_main_thread(move || imp::show_quiet(hwnd));
        #[cfg(not(windows))]
        sh.show_window(&win);
        imp::start(&sh);
        imp::keep_all_low();
        sh.log(&format!("desk: {label} on {:?} at {}%", area.mon, (area.sf * 100.0).round()));
    });
}

// As bars.rs lays its strips: the place, then the size, then the place again.
fn lay(win: &WebviewWindow, rect: Mon) {
    let _ = win.set_position(PhysicalPosition::new(rect.0, rect.1));
    let _ = win.set_size(PhysicalSize::new(rect.2.max(1) as u32, rect.3.max(1) as u32));
    let _ = win.set_position(PhysicalPosition::new(rect.0, rect.1));
}

// --- The pointer ------------------------------------------------------------------------------
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
const CTRL_MS: u64 = 40;

// Whether the pointer is looked at at all: someone about, and a desktop
// window there.
fn looks_wanted(quiet: bool, desks: usize) -> bool {
    !quiet && desks > 0
}

// What a stopped check gives the pointer's loop: never sooner than her own looks.
const IDLE_MS: u64 = 60_000;

fn contains(r: Mon, (x, y): (i32, i32)) -> bool {
    x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3
}

// One look at the pointer (physical px): each window takes the mouse while
// it is on one of its widgets (or held, or a menu open there), lets it
// through elsewhere. How long until the next look (ms; IDLE_MS: none wanted).
//
// Several widgets picked with Ctrl (desk.js, the user's call of 2026-10-17:
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
    let keys_ours = under.is_some() && seen && !ctrl_over && sh.own.lock().unwrap().contains(&crate::focus::foreground());
    pace(under.is_some(), seen, false, near, in_strip, ctrl_over || keys_ours).unwrap_or(IDLE_MS)
}

// Ctrl as the last look had it (its going down: the icons read again).
static CTRL_WAS: AtomicBool = AtomicBool::new(false);

// WAKUWAKU_DESK_TRACE=1: what the look made of Ctrl, logged.
fn tracing() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("WAKUWAKU_DESK_TRACE").is_some())
}
// The icons as read this long ago are read again as Ctrl goes down.
const ICONS_FRESH: std::time::Duration = std::time::Duration::from_secs(3);

// Whether a point of a desktop's page is on one of its icons (their rects).
fn on_icon(icons: &[[f64; 4]], x: f64, y: f64) -> bool {
    icons.iter().any(|r| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3])
}

// A display's icons read again for its window, on a thread of their own
// (Explorer answers), one at a time.
fn read_icons(label: String, mon: Mon, sf: f64) {
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

fn keep_icons(label: &str, icons: Vec<[f64; 4]>) {
    if let Some(d) = DESKS.lock().unwrap().iter_mut().find(|d| d.label == label) {
        d.icons = icons;
        d.icons_at = Some(std::time::Instant::now());
    }
}

// --- What it does, counted --------------------------------------------------------------------
// Each minute while there are desktop windows and the debug log is on
// (main.rs ticker): the pointer's looks, the words pushed and sent, the
// events to the pages, what the pages asked, the windows put back in their
// place, the looks at whether a desktop is covered, the desk thread's
// messages; and, with WAKUWAKU_DESK_TRACE=1, every message the main thread
// took, by kind (what wakes it).
static POLLS: AtomicU64 = AtomicU64::new(0);
static PUSHES: AtomicU64 = AtomicU64::new(0);
static SENT: AtomicU64 = AtomicU64::new(0);
static EMITS: AtomicU64 = AtomicU64::new(0);
static COMMANDS: AtomicU64 = AtomicU64::new(0);
static PLACED: AtomicU64 = AtomicU64::new(0);
static LOOKS: AtomicU64 = AtomicU64::new(0);
static THREAD_MSGS: AtomicU64 = AtomicU64::new(0);
static TRACE: Mutex<Option<Vec<(u32, bool, bool, u64)>>> = Mutex::new(None);

fn count(c: &AtomicU64) {
    c.fetch_add(1, Ordering::Relaxed);
}

fn emit(sh: &Shared, label: &str, event: &str, payload: Value) {
    count(&EMITS);
    let _ = sh.app.emit_to(label, event, payload);
}

// The main thread's messages counted (WAKUWAKU_DESK_TRACE=1), once.
fn trace_main(sh: &Arc<Shared>) {
    if std::env::var_os("WAKUWAKU_DESK_TRACE").is_none() {
        return;
    }
    {
        let mut trace = TRACE.lock().unwrap();
        if trace.is_some() {
            return;
        }
        *trace = Some(Vec::new());
    }
    let _ = sh.app.run_on_main_thread(imp::trace_this_thread);
    sh.log("desk: the main thread's messages counted (WAKUWAKU_DESK_TRACE)");
}

// One message the main thread took: by its kind, to a window or to the
// thread itself (another thread's wake: tao's proxy), posted or sent.
fn traced(hwnd: isize, msg: u32, sent: bool) {
    let Ok(mut trace) = TRACE.try_lock() else { return };
    let Some(list) = trace.as_mut() else { return };
    let thread = hwnd == 0;
    match list.iter_mut().find(|(m, t, s, _)| *m == msg && *t == thread && *s == sent) {
        Some(e) => e.3 += 1,
        None => list.push((msg, thread, sent, 1)),
    }
}

pub fn minute(sh: &Shared) {
    let take = |c: &AtomicU64| c.swap(0, Ordering::Relaxed);
    let counts = [take(&POLLS), take(&PUSHES), take(&SENT), take(&EMITS), take(&COMMANDS), take(&PLACED), take(&LOOKS), take(&THREAD_MSGS)];
    let traced = TRACE.lock().unwrap().as_mut().map(std::mem::take);
    if !any() || !sh.is_logging {
        return;
    }
    let [polls, pushes, sent, emits, commands, placed, looks, thread] = counts;
    let mut line = format!("desk: a minute: {polls} looks at the pointer, {pushes} pushes ({sent} sent), {emits} events, {commands} asked, {placed} put back, {looks} covered-looks, {thread} desk-thread messages");
    if let Some(mut list) = traced {
        list.sort_by(|a, b| b.3.cmp(&a.3));
        let total: u64 = list.iter().map(|e| e.3).sum();
        let top: Vec<String> = list.iter().take(8).map(|(m, t, s, n)| format!("{m:#06x}{}{}x{n}", if *t { " thread" } else { "" }, if *s { " sent" } else { "" })).collect();
        line.push_str(&format!("; main thread {total} messages: {}", top.join(", ")));
    }
    sh.log(&line);
}

// --- What the pages ask -------------------------------------------------------------------------
fn shared_of(app: &tauri::AppHandle) -> Arc<Shared> {
    crate::shared(app)
}

// Its listeners up: told what to show, and its display's picture.
#[tauri::command]
pub async fn desk_ready(app: tauri::AppHandle, window: WebviewWindow) {
    count(&COMMANDS);
    let sh = shared_of(&app);
    let key = DESKS.lock().unwrap().iter_mut().find(|d| d.label == window.label()).map(|d| {
        d.ready = true;
        d.sent = Value::Null;
        d.paper = Value::Null;
        d.views_on = None;
        d.key.clone()
    });
    refresh(&sh);
    push_widgets(&sh);
    // Made for a widget dragged over it: where it is, or its drop.
    if let Some(key) = key {
        drag_catch_up(&sh, window.label(), &key);
    }
    std::thread::spawn(move || read_papers(&sh));
}

// Where its widgets are now, and whether a menu is open.
#[tauri::command]
pub fn desk_shapes(window: WebviewWindow, shapes: Vec<[f64; 5]>) {
    count(&COMMANDS);
    let menu = shapes.iter().any(|s| s[4] == 12.0);
    if let Some(d) = DESKS.lock().unwrap().iter_mut().find(|d| d.label == window.label()) {
        d.shapes = shapes.into_iter().take(64).collect();
        d.menu = menu;
    }
}

// A press held (a drag): the window keeps the mouse till it is let go.
#[tauri::command]
pub fn desk_hold(window: WebviewWindow, on: bool) {
    count(&COMMANDS);
    let (was, hwnd) = {
        let mut desks = DESKS.lock().unwrap();
        let Some(d) = desks.iter_mut().find(|d| d.label == window.label()) else { return };
        d.held = on;
        let was = d.catching;
        if on {
            d.catching = true;
        }
        (was, d.hwnd)
    };
    // (A command without async runs on the main thread: set here.)
    if on && !was {
        imp::set_through(hwnd, false);
    }
}

// A board's name being typed (desk.js rename): the window takes the keyboard
// for it (it never takes the focus otherwise: WS_EX_NOACTIVATE), and lets go
// of it after (Enter, Esc, or a press elsewhere): the window that had it gets
// it back, and this one goes back down right over the desktop.
#[tauri::command]
pub fn desk_type(app: tauri::AppHandle, window: WebviewWindow, on: bool) {
    count(&COMMANDS);
    let sh = shared_of(&app);
    #[cfg(windows)]
    if let Ok(h) = window.hwnd() {
        imp::take_keys(h.0 as isize, on);
    }
    if on {
        let _ = window.set_focus();
    }
    sh.log(&format!("{}: keyboard {}", window.label(), if on { "taken (a board's name)" } else { "let go" }));
}

// What a page changed of the layout: these entries (new ones last), those
// gone; merged with the rest, as the others may have changed theirs. A new
// one without a display goes to the one the pointer is on.
#[tauri::command]
pub async fn desk_keep(app: tauri::AppHandle, entries: Vec<Value>, gone: Vec<String>) -> bool {
    count(&COMMANDS);
    let sh = shared_of(&app);
    let old = sh.setting("deskLayout");
    let old = old.as_array().cloned().unwrap_or_default();
    let mut given = entries;
    let pointer_key = || crate::screen::cursor(&sh, 1.0).and_then(|(x, y)| sh.screens().areas.into_iter().find(|a| x >= a.mon.0 && x < a.mon.0 + a.mon.2 && y >= a.mon.1 && y < a.mon.1 + a.mon.3)).map(|a| key_of(&sh, a.mon));
    for e in given.iter_mut() {
        let is_new = !old.iter().any(|o| o["id"] == e["id"]);
        if is_new && e.get("d").is_none() {
            if let Some(key) = pointer_key() {
                e["d"] = json!(key);
            }
        }
    }
    let merged = merge(&old, given, &gone);
    if !crate::settings::is_desk_layout(&merged) {
        sh.log(&format!("desk: a layout refused: {merged}"));
        return false;
    }
    sh.change(json!({ "deskLayout": merged }));
    true
}

// The layout with these entries in (in their places, new ones after) and those gone out.
pub fn merge(old: &[Value], given: Vec<Value>, gone: &[String]) -> Value {
    let mut given: Vec<Option<Value>> = given.into_iter().map(Some).collect();
    let mut out = Vec::new();
    for e in old {
        let id = e["id"].as_str().unwrap_or("");
        if gone.iter().any(|g| g == id) {
            continue;
        }
        match given.iter_mut().find(|g| g.as_ref().is_some_and(|g| g["id"] == e["id"])) {
            Some(g) => out.push(g.take().unwrap()),
            None => out.push(e.clone()),
        }
    }
    out.extend(given.into_iter().flatten());
    Value::Array(out)
}

// The desktop's icons on this window's display, as rects in its px, as
// Explorer's list draws each (imp::icons); None (null) when the desktop
// could not be read (Explorer starting again, busy): the page asks again.
#[tauri::command]
pub async fn desk_icons(app: tauri::AppHandle, window: WebviewWindow) -> Option<Vec<[f64; 4]>> {
    count(&COMMANDS);
    let (mon, sf) = DESKS.lock().unwrap().iter().find(|d| d.label == window.label()).map(|d| (d.mon, d.sf))?;
    let _ = app;
    let icons = tauri::async_runtime::spawn_blocking(move || imp::icons(mon, sf)).await.ok().flatten();
    // Kept for the pointer's look too: Ctrl over an icon is Explorer's.
    if let Some(icons) = &icons {
        keep_icons(window.label(), icons.clone());
    }
    icons
}

// Ctrl and the wheel over its bare desktop (desk.js): the window takes the
// mouse there while Ctrl is held (poll), so Explorer heard nothing of it.
// Handed on to the desktop's icon view as Windows would have sent it
// (WM_MOUSEWHEEL, Ctrl among its keys, the same turn, where the pointer is,
// the screen's px): its icons larger or smaller, as without her.
#[tauri::command]
pub fn desk_wheel(window: WebviewWindow, x: f64, y: f64, delta: i32) {
    count(&COMMANDS);
    let Some((mon, sf)) = DESKS.lock().unwrap().iter().find(|d| d.label == window.label()).map(|d| (d.mon, d.sf)) else { return };
    if !(x.is_finite() && y.is_finite()) || delta == 0 {
        return;
    }
    imp::wheel_to_icons(mon.0 + (x * sf).round() as i32, mon.1 + (y * sf).round() as i32, delta.clamp(-1200, 1200));
}

// Some of its widgets picked with Ctrl, or none any more (desk.js): a press
// off them is looked for (poll), and told (desk:outside).
#[tauri::command]
pub fn desk_picked(window: WebviewWindow, on: bool) {
    count(&COMMANDS);
    if let Some(d) = DESKS.lock().unwrap().iter_mut().find(|d| d.label == window.label()) {
        d.picked = on;
    }
}

// The icons' rects (x, y, w, h: the screen's px) that lie on a display, in
// its window's px (deskgrid.js takes them as blocks to keep clear of).
#[cfg_attr(not(windows), allow(dead_code))]
fn blocks_on(rects: &[(i32, i32, i32, i32)], mon: Mon, sf: f64) -> Vec<[f64; 4]> {
    rects
        .iter()
        .filter(|&&(x, y, w, h)| x + w > mon.0 && x < mon.0 + mon.2 && y + h > mon.1 && y < mon.1 + mon.3)
        .map(|&(x, y, w, h)| [(x - mon.0) as f64 / sf, (y - mon.1) as f64 / sf, w as f64 / sf, h as f64 / sf])
        .collect()
}

// Where a point of this window's (its px) lies: which display, its work
// area and the point in that display's window's px.
#[tauri::command]
pub async fn desk_where(app: tauri::AppHandle, window: WebviewWindow, x: f64, y: f64) -> Value {
    count(&COMMANDS);
    let sh = shared_of(&app);
    let Some((mon, sf)) = DESKS.lock().unwrap().iter().find(|d| d.label == window.label()).map(|d| (d.mon, d.sf)) else { return Value::Null };
    let (px, py) = (mon.0 + (x * sf).round() as i32, mon.1 + (y * sf).round() as i32);
    let Some(a) = sh.screens().areas.into_iter().find(|a| px >= a.mon.0 && px < a.mon.0 + a.mon.2 && py >= a.mon.1 && py < a.mon.1 + a.mon.3) else { return Value::Null };
    json!({
        "key": key_of(&sh, a.mon),
        "work": [(a.x - a.mon.0) as f64 / a.sf, (a.y - a.mon.1) as f64 / a.sf, a.w as f64 / a.sf, a.h as f64 / a.sf],
        "x": (px - a.mon.0) as f64 / a.sf,
        "y": (py - a.mon.1) as f64 / a.sf,
    })
}

// What her island's page does for a desktop page: a widget's settings
// opened ("settings": { widget }), widgets put elsewhere ("place":
// { items, where, stack }).
#[tauri::command]
pub async fn desk_ask(app: tauri::AppHandle, what: String, args: Value) {
    count(&COMMANDS);
    if matches!(what.as_str(), "settings" | "place" | "unbar") {
        let _ = app.emit_to("island", "desk:ask", json!({ "what": what, "args": args }));
    }
}

// --- A widget dragged from her home -------------------------------------------------------------
// The widget picker in her home's window (picker.js, the KDE-like design of
// 2026-10-17): a card, a widget off the strip or a combo dragged over the
// desktop. Her home's page says where the pointer is, in its own px
// (desk_drag_preview, as it moves; desk_drop, let go of there): that is the
// screen's (her home's window's corner and scale, island.rs origin), the
// display under it, and that display's desktop page's px; told to that
// page alone (desk:drag-preview, desk:drop: desk-drop.js shows where it
// would land and puts it there), as it came but for x and y (item and size,
// or a combo's set; the drop's ticket). A page it has left is told null.
// What a page found goes back to her home's page (desk_drag_answer →
// island:desk-answer). A display with no desktop window yet (nothing on it)
// has one made as the drag reaches it, told once its page is up, and kept
// while the drag lasts; then it stays if something was put there (the
// layout says so) and goes as ever if not.
//
// While the drag is over the desktop, its windows are raised over the
// others, right under her home's, and clicks still go through them (her
// home's window holds the pointer for the drag): the desktop's widgets
// and where it lands are seen over whatever window covers them (the user's
// call of 2026-10-17; KDE minimizes the windows, this does not). Put back
// right over the desktop as the drop is answered, as the pointer leaves the
// desktop, or should the buttons be let go of with no word (a page gone).
struct Drag {
    // Displays (keys) whose window is kept for the drag, nothing on them.
    keep: Vec<String>,
    // The desktop window the pointer was last over (told null once it leaves).
    over: Option<String>,
    // What a display's page is to be told once it is up (a window made for
    // the drag, or not ready yet): the last preview, a drop. By the display's key.
    preview: Option<(String, Value)>,
    drop: Option<(String, Value)>,
    // Counted at each word of the drag: a release of the windows kept is
    // for the turn it was asked at only.
    turn: u64,
}
static DRAG: Mutex<Drag> = Mutex::new(Drag { keep: Vec::new(), over: None, preview: None, drop: None, turn: 0 });
static RAISED: AtomicBool = AtomicBool::new(false);

// Where a point of her home's page is: the display under it (its key), and
// the point in that display's desktop page's px. None: off every display.
fn drag_target(sh: &Shared, x: f64, y: f64) -> Option<(String, (f64, f64))> {
    let (pos, sf) = crate::island::origin(sh);
    let p = (pos.0 + (x * sf).round() as i32, pos.1 + (y * sf).round() as i32);
    let area = sh.screens().areas.into_iter().find(|a| contains(a.mon, p))?;
    let key = key_of(sh, area.mon);
    // A window there (made at another scale until the displays settle): its own.
    let (mon, dsf) = DESKS.lock().unwrap().iter().find(|d| d.key == key).map_or((area.mon, area.sf), |d| (d.mon, d.sf));
    let at = ((p.0 - mon.0) as f64 / dsf, (p.1 - mon.1) as f64 / dsf);
    Some((key, at))
}

// A preview (drop: false) or a drop of her home's, to the display under it.
fn drag_to(sh: &Arc<Shared>, mut payload: Value, drop: bool) {
    let (Some(x), Some(y)) = (payload["x"].as_f64(), payload["y"].as_f64()) else { return };
    let target = if cfg!(windows) { drag_target(sh, x, y) } else { None };
    let Some((key, (dx, dy))) = target else {
        // Off every display (or no desktop windows here): as off the desktop.
        if drop {
            answer_none(sh, &payload);
        }
        return drag_off(sh, drop);
    };
    payload["x"] = json!(dx);
    payload["y"] = json!(dy);
    let event = if drop { "desk:drop" } else { "desk:drag-preview" };
    let here = DESKS.lock().unwrap().iter().find(|d| d.key == key).map(|d| (d.label.clone(), d.ready));
    let (left, make) = {
        let mut drag = DRAG.lock().unwrap();
        drag.turn += 1;
        let make = here.is_none() && !drag.keep.contains(&key);
        if here.is_none() && make {
            drag.keep.push(key.clone());
        }
        let now = here.as_ref().map(|(label, _)| label.clone());
        let left = if drag.over != now { std::mem::replace(&mut drag.over, now) } else { None };
        // Its page not up yet: told once it is (desk_ready).
        if !here.as_ref().is_some_and(|(_, ready)| *ready) {
            if drop {
                drag.drop = Some((key.clone(), payload.clone()));
            } else {
                drag.preview = Some((key.clone(), payload.clone()));
            }
        } else if !drop {
            drag.preview = Some((key.clone(), payload.clone()));
        }
        (left, make)
    };
    if let Some(label) = left {
        emit(sh, &label, "desk:drag-preview", Value::Null);
    }
    if let Some((label, true)) = &here {
        emit(sh, label, event, payload);
    }
    if make {
        sh.log(&format!("desk: a widget dragged over a display with nothing on it ({key}): its window made for it"));
        let sh = sh.clone();
        std::thread::spawn(move || sync(&sh));
    }
    if !drop {
        raise(sh);
    }
}

// Off the desktop again (a preview of null), or the drag over (dropped:
// the drop told, its answer in or not coming): every page's preview cleared
// but a dropped one's (it clears its own), the windows back right over the
// desktop, and those kept for the drag let go a moment later.
fn drag_off(sh: &Arc<Shared>, dropped: bool) {
    let turn = {
        let mut drag = DRAG.lock().unwrap();
        drag.over = None;
        drag.preview = None;
        drag.turn
    };
    if !dropped {
        let labels: Vec<String> = DESKS.lock().unwrap().iter().filter(|d| d.ready).map(|d| d.label.clone()).collect();
        for label in labels {
            emit(sh, &label, "desk:drag-preview", Value::Null);
        }
    }
    lower();
    // Let go once the page's drop is kept (desk_keep, then the layout's
    // sync): a window that has something now stays, the rest go.
    let sh = sh.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(1500));
        let released = {
            let mut drag = DRAG.lock().unwrap();
            if drag.turn != turn || drag.keep.is_empty() {
                return;
            }
            drag.drop = None;
            std::mem::take(&mut drag.keep)
        };
        sh.log(&format!("desk: the drag over, the windows made for it let go of unless kept ({})", released.join(", ")));
        sync(&sh);
    });
}

// A drop no page can take: said so at once, for her home's page.
fn answer_none(sh: &Shared, payload: &Value) {
    let _ = sh.app.emit_to("island", "island:desk-answer", json!({ "ticket": payload["ticket"], "ok": false, "where": "desk", "why": "none" }));
}

// The desktop's windows over the others, right under her home's.
fn raise(sh: &Arc<Shared>) {
    if RAISED.swap(true, Ordering::SeqCst) {
        return;
    }
    sh.log("desk: raised over the windows for a widget dragged from her home");
    imp::keep_all_low();
    // Put back should the drag end with no word (the buttons let go of,
    // her home's page gone): looked at every half second while raised.
    let sh = sh.clone();
    std::thread::spawn(move || {
        let mut up_for = 0;
        while RAISED.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(500));
            up_for = if crate::pet::any_down().unwrap_or(false) { 0 } else { up_for + 1 };
            if up_for >= 3 && RAISED.load(Ordering::SeqCst) {
                sh.log("desk: no button held for a while, the drag over: put back");
                drag_off(&sh, false);
            }
        }
    });
}

// Right over the desktop again.
fn lower() {
    if RAISED.swap(false, Ordering::SeqCst) {
        imp::keep_all_low();
        if let Some(sh) = SHARED.get() {
            sh.log("desk: put back right over the desktop");
        }
    }
}

// Her home's page: where the pointer is over the desktop ({ x, y, item,
// size } or { x, y, set }, its px), or null (off it again).
#[tauri::command]
pub async fn desk_drag_preview(app: tauri::AppHandle, window: WebviewWindow, at: Option<Value>) {
    count(&COMMANDS);
    if window.label() != "island" {
        return;
    }
    let sh = shared_of(&app);
    match at {
        Some(at) if at.is_object() => drag_to(&sh, at, false),
        _ => drag_off(&sh, false),
    }
}

// Her home's page: let go of over the desktop ({ x, y, item, size, ticket }
// or { x, y, set, ticket }).
#[tauri::command]
pub async fn desk_drop(app: tauri::AppHandle, window: WebviewWindow, at: Value) {
    count(&COMMANDS);
    if window.label() != "island" || !at.is_object() {
        return;
    }
    drag_to(&shared_of(&app), at, true);
}

// A desktop page's word on what is dragged over it ({ ticket, ok, where,
// why, name }; ticket null: the preview's), to her home's page. A drop's:
// the drag is over.
#[tauri::command]
pub async fn desk_drag_answer(app: tauri::AppHandle, window: WebviewWindow, said: Value) {
    count(&COMMANDS);
    if !window.label().starts_with("desk-") {
        return;
    }
    let dropped = said.get("ticket").is_some_and(|t| !t.is_null());
    let _ = app.emit_to("island", "island:desk-answer", said);
    if dropped {
        drag_off(&shared_of(&app), true);
    }
}

// A page just up: what the drag has for it (its window made for it).
fn drag_catch_up(sh: &Shared, label: &str, key: &str) {
    let (preview, drop) = {
        let mut drag = DRAG.lock().unwrap();
        let preview = drag.preview.as_ref().filter(|(k, _)| k == key).map(|(_, p)| p.clone());
        let drop = drag.drop.take_if(|(k, _)| k == key).map(|(_, p)| p);
        if preview.is_some() || drop.is_some() {
            drag.over = Some(label.to_string());
        }
        (preview, drop)
    };
    if let Some(p) = preview {
        emit(sh, label, "desk:drag-preview", p);
    }
    if let Some(p) = drop {
        emit(sh, label, "desk:drop", p);
    }
}

// --- Each display's picture ----------------------------------------------------------------------
// Read for each window, told when it changed: the file (by the asset
// protocol), how it is laid, the colour round it, the display and all of
// them from the window's corner (the window is the display), its scale.
pub fn read_papers(sh: &Shared) {
    let desks: Vec<(String, Mon, f64)> = DESKS.lock().unwrap().iter().filter(|d| d.ready).map(|d| (d.label.clone(), d.mon, d.sf)).collect();
    if desks.is_empty() {
        return;
    }
    let areas = sh.screens().areas;
    let left = areas.iter().map(|a| a.mon.0).min().unwrap_or(0);
    let top = areas.iter().map(|a| a.mon.1).min().unwrap_or(0);
    let right = areas.iter().map(|a| a.mon.0 + a.mon.2).max().unwrap_or(0);
    let bottom = areas.iter().map(|a| a.mon.1 + a.mon.3).max().unwrap_or(0);
    for (label, mon, sf) in desks {
        let Some(paper) = crate::wallpaper::read(mon) else { continue };
        if let Some(file) = &paper.file {
            let _ = sh.app.asset_protocol_scope().allow_file(file);
        }
        let rel = |(x, y, w, h): Mon| json!([(x - mon.0) as f64 / sf, (y - mon.1) as f64 / sf, w as f64 / sf, h as f64 / sf]);
        let url = paper.file.as_ref().map(|f| format!("{}?v={}", crate::data::asset_url(f), paper.stamp));
        let now = json!({ "url": url, "position": paper.position, "color": paper.color, "mon": rel(mon), "screen": rel((left, top, right - left, bottom - top)), "sf": sf });
        let changed = {
            let mut desks = DESKS.lock().unwrap();
            match desks.iter_mut().find(|d| d.label == label) {
                Some(d) if d.paper != now => {
                    d.paper = now.clone();
                    true
                }
                _ => false,
            }
        };
        if changed {
            sh.log(&format!("desk: {label} wallpaper {:?}, {}, {}", paper.file, paper.position, paper.color));
            emit(sh, &label, "desk:paper", now);
        }
    }
}

// Windows said its colours, the picture or the displays changed, or the
// half minute is up: looked at again.
fn look_again() {
    let Some(sh) = SHARED.get() else { return };
    let look = look_read();
    let was = LOOK.lock().unwrap().replace(look.clone());
    if was != Some(look) {
        refresh(sh);
    }
    read_papers(sh);
}

// --- A display's own name ---------------------------------------------------------------------
// The monitor's device interface (the same panel on the same port, whatever
// number Windows gives the display this time), shortened: "d" and 12 hex
// digits. Without it, its device name (\\.\DISPLAY1), and without that its
// size and place.
static KEYS: Mutex<Vec<(Mon, String)>> = Mutex::new(Vec::new());
pub fn key_of(sh: &Shared, mon: Mon) -> String {
    if let Some((_, k)) = KEYS.lock().unwrap().iter().find(|(m, _)| *m == mon) {
        return k.clone();
    }
    let name = sh.app.available_monitors().unwrap_or_default().into_iter().find(|m| (m.position().x, m.position().y, m.size().width as i32, m.size().height as i32) == mon).and_then(|m| m.name().cloned());
    let id = name.as_deref().and_then(imp::device_of).or(name).unwrap_or_else(|| format!("{}x{}@{},{}", mon.2, mon.3, mon.0, mon.1));
    let key = short_key(&id);
    let mut keys = KEYS.lock().unwrap();
    keys.retain(|(m, _)| *m != mon);
    keys.push((mon, key.clone()));
    key
}

// FNV-1a over the name, 48 bits of it.
pub fn short_key(id: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in id.to_ascii_lowercase().bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("d{:012x}", h & 0xFFFF_FFFF_FFFF)
}

// Windows' calls declared as the other modules declare them (taskbar.rs,
// backplate.rs, fullscreen.rs: the same shapes, or Rust warns).
#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::sync::Arc;

    use super::DESKS;
    use crate::Shared;

    type Hwnd = *mut c_void;

    #[repr(C)]
    struct DisplayDevice {
        size: u32,
        name: [u16; 32],
        string: [u16; 128],
        flags: u32,
        id: [u16; 128],
        key: [u16; 128],
    }

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    struct Msg {
        hwnd: Hwnd,
        message: u32,
        wparam: usize,
        lparam: isize,
        time: u32,
        pt: Point,
        private: u32,
    }

    #[repr(C)]
    struct WndClassExW {
        size: u32,
        style: u32,
        wndproc: extern "system" fn(Hwnd, u32, usize, isize) -> isize,
        cls_extra: i32,
        wnd_extra: i32,
        instance: isize,
        icon: isize,
        cursor: isize,
        background: isize,
        menu_name: *const u16,
        class_name: *const u16,
        icon_sm: isize,
    }

    type WinEventProc = extern "system" fn(isize, u32, Hwnd, i32, i32, u32, u32);

    #[link(name = "user32")]
    extern "system" {
        fn EnumDisplayDevicesW(device: *const u16, index: u32, dd: *mut DisplayDevice, flags: u32) -> i32;
        fn FindWindowW(class: *const u16, title: *const u16) -> Hwnd;
        fn FindWindowExW(parent: Hwnd, after: Hwnd, class: *const u16, title: *const u16) -> Hwnd;
        fn GetWindow(hwnd: Hwnd, cmd: u32) -> Hwnd;
        fn GetWindowLongPtrW(hwnd: Hwnd, index: i32) -> isize;
        fn SetWindowLongPtrW(hwnd: Hwnd, index: i32, value: isize) -> isize;
        fn IsWindowVisible(hwnd: Hwnd) -> i32;
        fn SetWindowPos(hwnd: Hwnd, after: isize, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
        fn SetWinEventHook(min: u32, max: u32, module: isize, func: WinEventProc, pid: u32, tid: u32, flags: u32) -> isize;
        fn RegisterClassExW(wc: *const WndClassExW) -> u16;
        fn CreateWindowExW(ex: u32, class: *const u16, name: *const u16, style: u32, x: i32, y: i32, w: i32, h: i32, parent: Hwnd, menu: isize, instance: isize, param: *mut c_void) -> Hwnd;
        fn DefWindowProcW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize;
        fn GetMessageW(msg: *mut Msg, hwnd: Hwnd, min: u32, max: u32) -> i32;
        fn TranslateMessage(msg: *const Msg) -> i32;
        fn DispatchMessageW(msg: *const Msg) -> isize;
        fn SetTimer(hwnd: Hwnd, id: usize, ms: u32, f: *const c_void) -> usize;
        fn PostMessageW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> i32;
        fn GetClassNameW(hwnd: Hwnd, name: *mut u16, max: i32) -> i32;
        fn KillTimer(hwnd: Hwnd, id: usize) -> i32;
        fn SetWindowsHookExW(id: i32, proc_: HookProc, module: isize, thread: u32) -> isize;
        fn CallNextHookEx(hook: isize, code: i32, wparam: usize, lparam: isize) -> isize;
        fn UnhookWinEvent(hook: isize) -> i32;
        fn PostQuitMessage(code: i32);
        fn DestroyWindow(hwnd: Hwnd) -> i32;
        fn IsIconic(hwnd: Hwnd) -> i32;
        fn GetAsyncKeyState(key: i32) -> i16;
        fn ShowWindow(hwnd: Hwnd, cmd: i32) -> i32;
        fn UnhookWindowsHookEx(hook: isize) -> i32;
        fn PostThreadMessageW(thread: u32, msg: u32, wparam: usize, lparam: isize) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleW(name: *const u16) -> isize;
        fn GetCurrentThreadId() -> u32;
        fn GetTickCount() -> u32;
    }

    const EDD_GET_DEVICE_INTERFACE_NAME: u32 = 1;
    const GW_HWNDNEXT: u32 = 2;
    const GW_HWNDPREV: u32 = 3;
    const GWL_EXSTYLE: i32 = -20;
    const WS_EX_TOPMOST: isize = 0x8;
    const WS_EX_TOOLWINDOW: u32 = 0x80;
    const WS_POPUP: u32 = 0x8000_0000;
    const HWND_TOP: isize = 0;
    const HWND_BOTTOM: isize = 1;
    const HWND_TOPMOST: isize = -1;
    const HWND_NOTOPMOST: isize = -2;
    const SWP_NOSIZE: u32 = 0x1;
    const SWP_NOMOVE: u32 = 0x2;
    const SWP_NOACTIVATE: u32 = 0x10;
    const SWP_NOOWNERZORDER: u32 = 0x200;
    const SWP_ASYNCWINDOWPOS: u32 = 0x4000;
    const EVENT_SYSTEM_FOREGROUND: u32 = 0x3;
    const EVENT_SYSTEM_MINIMIZESTART: u32 = 0x16;
    const EVENT_SYSTEM_MINIMIZEEND: u32 = 0x17;
    const WINEVENT_OUTOFCONTEXT: u32 = 0;
    const WINEVENT_SKIPOWNPROCESS: u32 = 2;
    const WM_TIMER: u32 = 0x0113;
    const WM_SETTINGCHANGE: u32 = 0x001A;
    const WM_DISPLAYCHANGE: u32 = 0x007E;
    const WM_DWMCOLORIZATIONCOLORCHANGED: u32 = 0x0320;
    const WM_APP_LOW: u32 = 0x8000 + 0x61;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    fn text(w: &[u16]) -> String {
        String::from_utf16_lossy(&w[..w.iter().position(|&c| c == 0).unwrap_or(w.len())])
    }

    // A display's monitor's device interface, by the display's device name.
    pub fn device_of(name: &str) -> Option<String> {
        // SAFETY: our own struct, of the size the call writes; the name ours.
        unsafe {
            let mut dd: DisplayDevice = std::mem::zeroed();
            dd.size = std::mem::size_of::<DisplayDevice>() as u32;
            (EnumDisplayDevicesW(wide(name).as_ptr(), 0, &mut dd, EDD_GET_DEVICE_INTERFACE_NAME) != 0 && dd.id[0] != 0).then(|| text(&dd.id))
        }
    }

    // The window the desktop's icons are in (the top one): Progman holding
    // them (Windows 11 24H2, and before anything asked for a wallpaper of
    // its own), else the WorkerW that does (Windows 10, 11 before 24H2).
    fn host() -> Hwnd {
        // SAFETY: plain lookups by class.
        unsafe {
            let defview = wide("SHELLDLL_DefView");
            let progman = FindWindowW(wide("Progman").as_ptr(), std::ptr::null());
            if !progman.is_null() && !FindWindowExW(progman, std::ptr::null_mut(), defview.as_ptr(), std::ptr::null()).is_null() {
                return progman;
            }
            let worker = wide("WorkerW");
            let mut w: Hwnd = std::ptr::null_mut();
            loop {
                w = FindWindowExW(std::ptr::null_mut(), w, worker.as_ptr(), std::ptr::null());
                if w.is_null() {
                    return progman;
                }
                if !FindWindowExW(w, std::ptr::null_mut(), defview.as_ptr(), std::ptr::null()).is_null() {
                    return w;
                }
            }
        }
    }

    // Windows' own taskbars: a display's other than the main one's lies
    // right over the desktop too (not kept on top in the VM, 2026-10-16),
    // and Explorer puts it back there; hers stay clear of it (the work
    // area), and it goes unfought.
    fn is_taskbar(h: Hwnd) -> bool {
        let mut name = [0u16; 32];
        // SAFETY: a buffer of our own.
        let k = unsafe { GetClassNameW(h, name.as_mut_ptr(), 32) }.max(0) as usize;
        matches!(String::from_utf16_lossy(&name[..k]).as_str(), "Shell_TrayWnd" | "Shell_SecondaryTrayWnd")
    }

    fn topmost(h: Hwnd) -> bool {
        // SAFETY: a plain read.
        unsafe { GetWindowLongPtrW(h, GWL_EXSTYLE) & WS_EX_TOPMOST != 0 }
    }

    // Right over the host: nothing in sight between them but hers.
    fn right_over(ours: Hwnd, host: Hwnd, mine: &[isize]) -> bool {
        // SAFETY: plain reads along the z-order.
        unsafe {
            let mut h = GetWindow(ours, GW_HWNDNEXT);
            let mut n = 0;
            while !h.is_null() && n < 8192 {
                if h == host {
                    return true;
                }
                if IsWindowVisible(h) != 0 && !mine.contains(&(h as isize)) && !is_taskbar(h) {
                    return false;
                }
                h = GetWindow(h, GW_HWNDNEXT);
                n += 1;
            }
            false
        }
    }

    // One of hers kept right over the window with the icons: after the
    // window over that one (or at the top of its band), topmost while it is
    // (the desktop shown). Some(what was done), None when it was there.
    fn keep_low(ours: Hwnd, mine: &[isize]) -> Option<&'static str> {
        let host = host();
        let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_ASYNCWINDOWPOS;
        // SAFETY: our own window; the others only named.
        unsafe {
            if host.is_null() {
                SetWindowPos(ours, HWND_BOTTOM, 0, 0, 0, 0, flags);
                return Some("at the bottom (no desktop window)");
            }
            let raised = topmost(host);
            if topmost(ours) == raised && right_over(ours, host, mine) {
                return None;
            }
            if topmost(ours) != raised {
                SetWindowPos(ours, if raised { HWND_TOPMOST } else { HWND_NOTOPMOST }, 0, 0, 0, 0, flags);
            }
            let mut above = GetWindow(host, GW_HWNDPREV);
            while !above.is_null() && mine.contains(&(above as isize)) {
                above = GetWindow(above, GW_HWNDPREV);
            }
            let after = if above.is_null() || topmost(above) != raised {
                if raised {
                    HWND_TOPMOST
                } else {
                    HWND_TOP
                }
            } else {
                above as isize
            };
            SetWindowPos(ours, after, 0, 0, 0, 0, flags);
            Some(if raised {
                "over the desktop shown (topmost)"
            } else if after == HWND_TOP {
                "over the desktop brought to the front"
            } else {
                "right over the desktop, under the windows"
            })
        }
    }

    // The keyboard taken for a board's name, or let go: its WS_EX_NOACTIVATE
    // off (not tao's set_focusable, which sets all its styles anew, the
    // clicks let through among them), the window to the front by borrowing
    // the input of the one there (jump.rs bring_window, as the settings take
    // it); then that one's again, and this one back right over the desktop.
    static BEFORE: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);
    pub fn take_keys(hwnd: isize, on: bool) {
        const WS_EX_NOACTIVATE: isize = 0x0800_0000;
        let mine = hwnd as Hwnd;
        // SAFETY: our own window's style; the one before only named (focus.rs checks it is still there).
        unsafe {
            let ex = GetWindowLongPtrW(mine, GWL_EXSTYLE);
            if on {
                let before = crate::focus::foreground();
                if before != hwnd {
                    BEFORE.store(before, std::sync::atomic::Ordering::SeqCst);
                }
                SetWindowLongPtrW(mine, GWL_EXSTYLE, ex & !WS_EX_NOACTIVATE);
                crate::jump::bring_window(hwnd);
            } else {
                SetWindowLongPtrW(mine, GWL_EXSTYLE, ex | WS_EX_NOACTIVATE);
                if crate::focus::foreground() == hwnd {
                    crate::focus::set_foreground(BEFORE.swap(0, std::sync::atomic::Ordering::SeqCst));
                }
                keep_all_low();
            }
        }
    }

    // Clicks through it, or taken: its own extended style set (not tao's
    // set_ignore_cursor_events, which shows the window again, SW_SHOW, as
    // it sets any style: brought up over the others).
    pub fn set_through(hwnd: isize, through: bool) {
        const WS_EX_TRANSPARENT: isize = 0x20;
        const WS_EX_LAYERED: isize = 0x8_0000;
        // SAFETY: our own window, on its own thread (main.rs: run on the main thread).
        unsafe {
            let ex = GetWindowLongPtrW(hwnd as Hwnd, GWL_EXSTYLE);
            let want = if through { ex | WS_EX_TRANSPARENT | WS_EX_LAYERED } else { ex & !(WS_EX_TRANSPARENT | WS_EX_LAYERED) };
            if want != ex {
                SetWindowLongPtrW(hwnd as Hwnd, GWL_EXSTYLE, want);
            }
        }
    }

    // The desktop brought over the windows (Win+D): a window of someone else's
    // in sight under the one with the icons.
    pub fn desktop_shown(own: &[isize]) -> bool {
        let host = host();
        if host.is_null() {
            return false;
        }
        // SAFETY: plain reads along the z-order; a buffer of our own.
        unsafe {
            let mut h = GetWindow(host, GW_HWNDNEXT);
            let mut n = 0;
            while !h.is_null() && n < 8192 {
                if IsWindowVisible(h) != 0 && IsIconic(h) == 0 && !own.contains(&(h as isize)) {
                    let mut name = [0u16; 32];
                    let k = GetClassNameW(h, name.as_mut_ptr(), 32).max(0) as usize;
                    let class = String::from_utf16_lossy(&name[..k]);
                    if class != "WorkerW" && class != "Progman" {
                        return true;
                    }
                }
                h = GetWindow(h, GW_HWNDNEXT);
                n += 1;
            }
            false
        }
    }

    // Every one of hers where it belongs: right over the desktop; or, while
    // a widget is dragged over it from her home (RAISED), over the other
    // windows, right under her home's (the top of those kept on top, then
    // after hers).
    pub fn keep_all_low() {
        let mine: Vec<isize> = DESKS.lock().unwrap().iter().map(|d| d.hwnd).filter(|&h| h != 0).collect();
        if super::RAISED.load(std::sync::atomic::Ordering::SeqCst) {
            let home = super::SHARED.get().map_or(0, |sh| crate::island::hwnd(sh));
            let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_ASYNCWINDOWPOS;
            for &h in &mine {
                // SAFETY: our own windows; hers only named.
                unsafe {
                    SetWindowPos(h as Hwnd, HWND_TOPMOST, 0, 0, 0, 0, flags);
                    if home != 0 {
                        SetWindowPos(h as Hwnd, home, 0, 0, 0, 0, flags);
                    }
                }
            }
            return;
        }
        for &h in &mine {
            if let Some(what) = keep_low(h as Hwnd, &mine) {
                super::count(&super::PLACED);
                if let Some(sh) = super::SHARED.get() {
                    sh.log(&format!("desk: {what}"));
                }
            }
        }
    }

    // The window in front changed (Win+D among others), or one came back
    // from the taskbar: hers where they belong.
    extern "system" fn on_event(_hook: isize, _event: u32, _hwnd: Hwnd, _object: i32, _child: i32, _thread: u32, _time: u32) {
        keep_all_low();
        // Another window in front: an open menu closes.
        if let Some(sh) = super::SHARED.get() {
            super::close_menus(sh);
        }
        // Looked at again a moment later, and once more: shown the desktop
        // (Win+D), the window with the icons comes to the front first and
        // only then over the others (the VM, 2026-10-16: hers were put right
        // over it while it still lay at the bottom, and stayed under it).
        let hwnd = WINDOW.load(std::sync::atomic::Ordering::SeqCst);
        if hwnd != 0 {
            // SAFETY: our own window, on this thread.
            unsafe {
                SetTimer(hwnd as Hwnd, AGAIN_SOON, 250, std::ptr::null());
                SetTimer(hwnd as Hwnd, AGAIN_LATER, 1000, std::ptr::null());
            }
        }
        // A desktop seen again: its words caught up.
        if let Some(sh) = super::SHARED.get() {
            super::catch_up(sh);
        }
    }

    // Its timers, all on its hidden window: the two looks after the window in
    // front changes; a desktop waiting to be told its widgets' words (armed
    // only while one waits); the picture of a slideshow or of Spotlight
    // (armed only while the desktop has one: a plain picture says when it
    // changes, WM_SETTINGCHANGE); the looks after that word, which comes
    // before the picture is in place (wallpaper.rs SETTLE_MS: armed for
    // five seconds after it, SETTLE_LEFT looks left).
    const PAPER: usize = 1;
    const AGAIN_SOON: usize = 2;
    const AGAIN_LATER: usize = 3;
    const STALE: usize = 4;
    const SETTLE: usize = 5;
    static SETTLE_LEFT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    const WM_APP_STALE: u32 = 0x8000 + 0x62;
    const WM_APP_STOP: u32 = 0x8000 + 0x63;

    // A picture that changes by itself: a slideshow (2) or Spotlight (3)
    // (Settings' choice, as wallpaper.rs reads it).
    fn picture_turns() -> bool {
        let key = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Wallpapers");
        matches!(key.ok().and_then(|k| k.get_value::<u32, _>("BackgroundType").ok()), Some(2 | 3))
    }

    // The picture's timer as the desktop's picture is: a minute while it
    // turns by itself, none otherwise.
    fn arm_paper(hwnd: Hwnd) {
        // SAFETY: our own window's timer, on its thread.
        unsafe {
            if picture_turns() {
                SetTimer(hwnd, PAPER, 60_000, std::ptr::null());
            } else {
                KillTimer(hwnd, PAPER);
            }
        }
    }

    extern "system" fn wndproc(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize {
        super::count(&super::THREAD_MSGS);
        match msg {
            // The picture, Windows' colours: looked at again, off this
            // thread (Explorer answers the picture's questions), now and
            // each half second for five (SETTLE; set again, it starts over).
            WM_SETTINGCHANGE | WM_DWMCOLORIZATIONCOLORCHANGED => {
                arm_paper(hwnd);
                std::thread::spawn(super::look_again);
                SETTLE_LEFT.store(crate::wallpaper::SETTLE_LOOKS, std::sync::atomic::Ordering::SeqCst);
                // SAFETY: our own window's timer, on its thread.
                unsafe { SetTimer(hwnd, SETTLE, crate::wallpaper::SETTLE_MS, std::ptr::null()) };
                0
            }
            WM_TIMER if wparam == SETTLE => {
                let left = SETTLE_LEFT.load(std::sync::atomic::Ordering::SeqCst).saturating_sub(1);
                SETTLE_LEFT.store(left, std::sync::atomic::Ordering::SeqCst);
                if left == 0 {
                    // SAFETY: our own window's timer.
                    unsafe { KillTimer(hwnd, SETTLE) };
                }
                std::thread::spawn(super::look_again);
                0
            }
            // The displays: the windows with them, once they have settled.
            WM_DISPLAYCHANGE => {
                if let Some(sh) = super::SHARED.get() {
                    let sh = sh.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(1500));
                        super::sync(&sh);
                        super::look_again();
                    });
                }
                0
            }
            WM_TIMER if wparam == AGAIN_SOON || wparam == AGAIN_LATER => {
                // SAFETY: our own window's one-shot timer.
                unsafe { KillTimer(hwnd, wparam) };
                keep_all_low();
                if let Some(sh) = super::SHARED.get() {
                    super::catch_up(sh);
                }
                0
            }
            // A desktop waiting to be told (a window restored without
            // another coming to the front): looked at every ten seconds
            // while one waits, then the timer goes.
            WM_TIMER if wparam == STALE => {
                if let Some(sh) = super::SHARED.get() {
                    super::catch_up(sh);
                }
                if !super::any_stale() {
                    // SAFETY: our own window's timer.
                    unsafe { KillTimer(hwnd, STALE) };
                }
                0
            }
            // A slideshow's or Spotlight's next picture says nothing: looked
            // at each minute while someone is about.
            WM_TIMER if wparam == PAPER => {
                if !super::SHARED.get().is_some_and(|sh| sh.is_quiet()) {
                    std::thread::spawn(super::look_again);
                }
                0
            }
            WM_APP_STALE => {
                // SAFETY: our own window's timer.
                unsafe { SetTimer(hwnd, STALE, 10_000, std::ptr::null()) };
                0
            }
            WM_APP_LOW => {
                keep_all_low();
                0
            }
            WM_APP_STOP => {
                // SAFETY: this thread's own loop ended.
                unsafe { PostQuitMessage(0) };
                0
            }
            // SAFETY: the default for the rest.
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }

    static WINDOW: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);
    static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

    // Its thread, while there is a desktop window: a hidden window that hears
    // Windows' broadcasts, the hooks on the window in front, its timers (none
    // running while nothing waits). Asked again: hers put where they belong,
    // on that thread. Gone with the last desktop window (stop).
    pub fn start(sh: &Arc<Shared>) {
        let _ = super::SHARED.set(sh.clone());
        if STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
            let hwnd = WINDOW.load(std::sync::atomic::Ordering::SeqCst);
            if hwnd != 0 {
                // SAFETY: our own window; the message carries nothing.
                unsafe { PostMessageW(hwnd as Hwnd, WM_APP_LOW, 0, 0) };
            }
            return;
        }
        super::trace_main(sh);
        crate::spawn_named("desk", || {
            crate::power::thread_eco();
            // SAFETY: our own class, window and hooks, on this thread, which runs their loop.
            unsafe {
                let instance = GetModuleHandleW(std::ptr::null());
                let class = wide("WakuwakuDesk");
                let wc = WndClassExW { size: std::mem::size_of::<WndClassExW>() as u32, style: 0, wndproc, cls_extra: 0, wnd_extra: 0, instance, icon: 0, cursor: 0, background: 0, menu_name: std::ptr::null(), class_name: class.as_ptr(), icon_sm: 0 };
                RegisterClassExW(&wc);
                let hwnd = CreateWindowExW(WS_EX_TOOLWINDOW, class.as_ptr(), std::ptr::null(), WS_POPUP, 0, 0, 0, 0, std::ptr::null_mut(), 0, instance, std::ptr::null_mut());
                WINDOW.store(hwnd as isize, std::sync::atomic::Ordering::SeqCst);
                let hooks = [
                    SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, 0, on_event, 0, 0, WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS),
                    SetWinEventHook(EVENT_SYSTEM_MINIMIZEEND, EVENT_SYSTEM_MINIMIZEEND, 0, on_event, 0, 0, WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS),
                    SetWinEventHook(EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZESTART, 0, on_event, 0, 0, WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS),
                ];
                arm_paper(hwnd);
                if super::any_stale() {
                    SetTimer(hwnd, STALE, 10_000, std::ptr::null());
                }
                keep_all_low();
                keys_start();
                let mut msg: Msg = std::mem::zeroed();
                while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                keys_stop();
                for h in hooks {
                    UnhookWinEvent(h);
                }
                WINDOW.store(0, std::sync::atomic::Ordering::SeqCst);
                DestroyWindow(hwnd);
            }
            STARTED.store(false, std::sync::atomic::Ordering::SeqCst);
            if let Some(sh) = super::SHARED.get() {
                sh.log("desk: its thread gone with the last desktop window");
            }
        });
    }

    // A turn of the wheel with Ctrl, to the desktop's icon view (the list in
    // SHELLDLL_DefView, in the window with the icons): posted, as a wheel's
    // message comes (its turn in the high word of wparam, the keys held in
    // the low; the point the screen's, a signed 16-bit pair in lparam).
    pub fn wheel_to_icons(x: i32, y: i32, delta: i32) -> bool {
        const WM_MOUSEWHEEL: u32 = 0x020A;
        const MK_CONTROL: usize = 0x0008;
        let host = host();
        if host.is_null() {
            return false;
        }
        // SAFETY: plain lookups by class; a posted message carrying no pointers.
        unsafe {
            let view = FindWindowExW(host, std::ptr::null_mut(), wide("SHELLDLL_DefView").as_ptr(), std::ptr::null());
            if view.is_null() {
                return false;
            }
            let list = FindWindowExW(view, std::ptr::null_mut(), wide("SysListView32").as_ptr(), std::ptr::null());
            let to = if list.is_null() { view } else { list };
            let wparam = ((delta as i16 as u16 as usize) << 16) | MK_CONTROL;
            let lparam = (((y as i16 as u16 as u32) << 16) | (x as i16 as u16 as u32)) as isize;
            PostMessageW(to, WM_MOUSEWHEEL, wparam, lparam) != 0
        }
    }

    // How long ago Ctrl's key last went down or up (ms; the trace's).
    pub fn ctrl_age() -> u32 {
        // SAFETY: a plain query.
        unsafe { GetTickCount() }.wrapping_sub(CTRL_AT.load(std::sync::atomic::Ordering::Relaxed))
    }

    // Shown, not made the active window (SW_SHOWNOACTIVATE). Tao's show
    // (SW_SHOW) made a new desktop window the active one, the foreground
    // one too while her home's window had it: a widget being dragged from
    // her home's picker was let go of there, as her home's page lost the
    // keyboard (tried, 2026-10-17). Tao is not told: nothing of tao's sets
    // these windows' styles once they are up (set_through, take_keys are
    // our own), which would hide one again.
    pub fn show_quiet(hwnd: isize) {
        const SW_SHOWNOACTIVATE: i32 = 4;
        // SAFETY: our own window.
        unsafe { ShowWindow(hwnd as Hwnd, SW_SHOWNOACTIVATE) };
    }

    // Ctrl held, either of them, now. Its key just pressed or let go (the
    // hook's word, CTRL_FRESH_MS at most): as the hook has it, for the look
    // it woke runs before Windows' own key state has changed (a Ctrl and a
    // press 30 ms after it went to Explorer, tried 2026-10-17); else
    // Windows' (should the hook have missed a key: one let go over a window
    // run as administrator).
    pub fn ctrl_down() -> bool {
        const VK_CONTROL: i32 = 0x11;
        const CTRL_FRESH_MS: u32 = 150;
        use std::sync::atomic::Ordering::Relaxed;
        // SAFETY: plain queries; the high bit is the key's state now.
        unsafe {
            if GetTickCount().wrapping_sub(CTRL_AT.load(Relaxed)) < CTRL_FRESH_MS {
                return CTRL_HELD.load(Relaxed);
            }
            GetAsyncKeyState(VK_CONTROL) as u16 & 0x8000 != 0
        }
    }

    // Ctrl's keys going down and up anywhere: the pointer's look woken at
    // once (main.rs wake_pointer), not at its pace, so a press right after
    // Ctrl reaches the desktop's page (poll). A low-level keyboard hook
    // (WH_KEYBOARD_LL) on a thread of its own that does nothing else:
    // Windows hands it every key on the machine and waits for its answer,
    // and drops one that answers late (winkeys.rs has the same); it looks at
    // Ctrl's keys alone, and wakes the loop as Ctrl changes. Nothing of it
    // runs while no key is touched. In with the desk thread, out with it.
    // Deaf while one of her own windows has the keyboard (WebView2 in this
    // process focused: Windows calls none of its low-level hooks), when the
    // look comes every 40 ms over a desktop with widgets instead (poll).
    #[repr(C)]
    struct KbdLl {
        vk: u32,
        scan: u32,
        flags: u32,
        time: u32,
        extra: usize,
    }
    static CTRL_HELD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    // When Ctrl last went down or up (the key's own time, GetTickCount's).
    static CTRL_AT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    static KEYS_THREAD: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

    extern "system" fn on_key(code: i32, wparam: usize, lparam: isize) -> isize {
        const LLKHF_UP: u32 = 0x80;
        if code == 0 && lparam != 0 {
            // SAFETY: Windows hands a KBDLLHOOKSTRUCT with HC_ACTION.
            let k = unsafe { &*(lparam as *const KbdLl) };
            // VK_CONTROL, VK_LCONTROL, VK_RCONTROL. Windows' key state is
            // still as it was before this key: held down, a key repeats (no
            // change); let go, the other Ctrl may still be held. Read so,
            // not from what this hook last heard, which misses keys while
            // her window has the keyboard.
            if matches!(k.vk, 0x11 | 0xA2 | 0xA3) {
                let other = match k.vk {
                    0xA2 => 0xA3,
                    0xA3 => 0xA2,
                    _ => 0,
                };
                // SAFETY: plain queries.
                let (was, other_held) = unsafe { (GetAsyncKeyState(0x11) as u16 & 0x8000 != 0, other != 0 && GetAsyncKeyState(other) as u16 & 0x8000 != 0) };
                let held = k.flags & LLKHF_UP == 0 || other_held;
                if held != was {
                    CTRL_HELD.store(held, std::sync::atomic::Ordering::Relaxed);
                    CTRL_AT.store(k.time, std::sync::atomic::Ordering::Relaxed);
                    crate::wake_pointer();
                }
            }
        }
        // SAFETY: passed on, as every hook must.
        unsafe { CallNextHookEx(0, code, wparam, lparam) }
    }

    fn keys_start() {
        const WH_KEYBOARD_LL: i32 = 13;
        let (tx, rx) = std::sync::mpsc::channel();
        crate::spawn_named("desk ctrl", move || {
            // SAFETY: a hook of our own, on this thread, which runs its loop.
            unsafe {
                let hook = SetWindowsHookExW(WH_KEYBOARD_LL, on_key, GetModuleHandleW(std::ptr::null()), 0);
                KEYS_THREAD.store(GetCurrentThreadId(), std::sync::atomic::Ordering::SeqCst);
                let _ = tx.send(hook != 0);
                let mut msg: Msg = std::mem::zeroed();
                while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {}
                if hook != 0 {
                    UnhookWindowsHookEx(hook);
                }
            }
        });
        let put_in = rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap_or(false);
        if let Some(sh) = super::SHARED.get() {
            sh.log(&format!("desk: Ctrl watched for widgets picked on the desktop: hook {}", if put_in { "in" } else { "could not be put in" }));
        }
    }

    fn keys_stop() {
        const WM_QUIT: u32 = 0x12;
        let id = KEYS_THREAD.swap(0, std::sync::atomic::Ordering::SeqCst);
        if id != 0 {
            // SAFETY: a posted message, no pointers.
            unsafe { PostThreadMessageW(id, WM_QUIT, 0, 0) };
        }
    }

    // The last desktop window gone: its thread, hooks and timers with it.
    pub fn stop() {
        let hwnd = WINDOW.load(std::sync::atomic::Ordering::SeqCst);
        if hwnd != 0 {
            // SAFETY: our own window; the message carries nothing.
            unsafe { PostMessageW(hwnd as Hwnd, WM_APP_STOP, 0, 0) };
        }
    }

    // A desktop now waits to be told: its ten seconds' look armed.
    pub fn arm_stale() {
        let hwnd = WINDOW.load(std::sync::atomic::Ordering::SeqCst);
        if hwnd != 0 {
            // SAFETY: our own window; the message carries nothing.
            unsafe { PostMessageW(hwnd as Hwnd, WM_APP_STALE, 0, 0) };
        }
    }

    // Debug only (WAKUWAKU_DESK_TRACE=1): every message the main thread
    // takes, counted by kind (desk.rs minute): what wakes it.
    type HookProc = extern "system" fn(i32, usize, isize) -> isize;
    #[repr(C)]
    struct CwpStruct {
        lparam: isize,
        wparam: usize,
        message: u32,
        hwnd: Hwnd,
    }
    extern "system" fn on_get(code: i32, wparam: usize, lparam: isize) -> isize {
        if code >= 0 && lparam != 0 {
            // SAFETY: the system's MSG for this hook, alive for the call.
            let m = unsafe { &*(lparam as *const Msg) };
            super::traced(m.hwnd as isize, m.message, false);
        }
        // SAFETY: the next hook, as the system asks.
        unsafe { CallNextHookEx(0, code, wparam, lparam) }
    }
    extern "system" fn on_send(code: i32, wparam: usize, lparam: isize) -> isize {
        if code >= 0 && lparam != 0 {
            // SAFETY: the system's CWPSTRUCT for this hook, alive for the call.
            let m = unsafe { &*(lparam as *const CwpStruct) };
            super::traced(m.hwnd as isize, m.message, true);
        }
        // SAFETY: the next hook, as the system asks.
        unsafe { CallNextHookEx(0, code, wparam, lparam) }
    }
    pub fn trace_this_thread() {
        const WH_GETMESSAGE: i32 = 3;
        const WH_CALLWNDPROC: i32 = 4;
        // SAFETY: hooks on this thread alone, our own procedures.
        unsafe {
            let me = GetCurrentThreadId();
            SetWindowsHookExW(WH_GETMESSAGE, on_get, 0, me);
            SetWindowsHookExW(WH_CALLWNDPROC, on_send, 0, me);
        }
    }

    // The desktop's icons on a display: each its grid's cell where it is
    // (icons.rs), in the display's window's px.
    pub fn icons(mon: super::Mon, sf: f64) -> Option<Vec<[f64; 4]>> {
        crate::icons::with_com(|| {
            let d = crate::icons::desktop()?;
            // As its list draws each (its picture and its label, both lines):
            // else where the view puts it, a place of the grid's spacing.
            let drawn: Vec<(i32, i32, i32, i32)> = d.bounds().iter().map(|r| (r.left, r.top, r.right - r.left, r.bottom - r.top)).collect();
            let rects = if drawn.is_empty() {
                let s = d.spacing();
                d.items().iter().map(|i| (i.at.x, i.at.y, s.x, s.y)).collect()
            } else {
                drawn
            };
            Some(super::blocks_on(&rects, mon, sf))
        })
    }
}

#[cfg(not(windows))]
mod imp {
    use std::sync::Arc;

    use crate::Shared;

    pub fn device_of(_name: &str) -> Option<String> {
        None
    }
    pub fn start(_sh: &Arc<Shared>) {}
    pub fn ctrl_down() -> bool {
        false
    }
    pub fn ctrl_age() -> u32 {
        0
    }
    pub fn wheel_to_icons(_x: i32, _y: i32, _delta: i32) -> bool {
        false
    }
    pub fn keep_all_low() {}
    pub fn set_through(_hwnd: isize, _through: bool) {}
    pub fn desktop_shown(_own: &[isize]) -> bool {
        true
    }
    pub fn stop() {}
    pub fn arm_stale() {}
    pub fn trace_this_thread() {}
    pub fn icons(_mon: super::Mon, _sf: f64) -> Option<Vec<[f64; 4]>> {
        Some(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pointers_look_stops_where_nothing_can_be_caught() {
        // Nobody about, no widget on the pointer's display, that desktop covered: none.
        assert_eq!(pace(true, true, true, true, true, false), None);
        assert_eq!(pace(false, true, false, true, true, false), None);
        assert_eq!(pace(true, false, false, true, true, false), None);
        // On a display with widgets: 250 ms; near one or in the taskbar's strip: 100 ms.
        assert_eq!(pace(true, true, false, false, false, false), Some(250));
        assert_eq!(pace(true, true, false, true, false, false), Some(100));
        assert_eq!(pace(true, true, false, false, true, false), Some(100));
        // Ctrl held over it: 40 ms (a press picks widgets); Ctrl or not, none where none is wanted.
        assert_eq!(pace(true, true, false, false, false, true), Some(40));
        assert_eq!(pace(true, false, false, false, false, true), None);
        assert_eq!(pace(true, true, true, false, false, true), None);
        assert!(IDLE_MS > 1000);
        assert!(contains((0, 0, 10, 10), (9, 0)) && !contains((0, 0, 10, 10), (10, 0)) && !contains((-1920, 0, 1920, 1080), (0, 5)));
    }

    // No desktop widget: no window, nothing waiting, no timer to arm; the
    // pointer's look and the widgets' push turn back at once.
    #[test]
    fn with_no_desktop_widget_nothing_runs() {
        assert!(DESKS.lock().unwrap().is_empty());
        assert!(!any());
        assert!(!any_stale());
        assert!(!looks_wanted(false, 0));
        assert!(!looks_wanted(true, 2));
        assert!(looks_wanted(false, 1));
    }

    // The icons on a display, in its window's px: as the test VM's list drew
    // them (its left display at 175 %, at -1920; the main one at 125 %).
    #[test]
    fn icons_on_a_display_as_drawn() {
        let left: Mon = (-1920, 0, 1920, 1080);
        let drawn = [(-360, 530, 130, 147), (1209, 5, 92, 107), (-30, 100, 60, 60)];
        let got = blocks_on(&drawn, left, 1.75);
        // The one with two lines whole (the place the view gave: -337, 527, 93 x 125, short of its label by 25 px).
        assert_eq!(got.len(), 2);
        let [x, y, w, h] = got[0];
        assert!((x - 1560.0 / 1.75).abs() < 1e-9 && (y - 530.0 / 1.75).abs() < 1e-9 && (w - 130.0 / 1.75).abs() < 1e-9 && (h - 84.0).abs() < 1e-9, "{got:?}");
        // One across the displays' edge: on both, each its part in its own px.
        assert_eq!(got[1][0], 1890.0 / 1.75);
        let main: Mon = (0, 0, 2560, 1440);
        let on_main = blocks_on(&drawn, main, 1.25);
        assert_eq!(on_main.len(), 2);
        assert_eq!(on_main[0], [1209.0 / 1.25, 4.0, 92.0 / 1.25, 107.0 / 1.25]);
    }

    // What a board shows: its tiles at their own sizes; nothing while folded.
    #[test]
    fn what_a_board_shows() {
        let tiles = json!([{ "type": "widget", "of": "monitor", "size": "m", "at": [0, 0] }, { "type": "plugin", "of": "stock", "size": "s", "at": [4, 0] }]);
        let board = json!({ "id": "wb", "type": "board", "items": tiles });
        assert!(entry_shows_at(&board, "monitor", None, true));
        assert!(entry_shows_at(&board, "stock-aapl", Some("stock"), false));
        assert!(!entry_shows_at(&board, "stock-aapl", Some("stock"), true));
        assert!(!entry_shows_at(&board, "weather", None, false));
        let folded = json!({ "id": "wb", "type": "board", "folded": true, "items": tiles });
        assert!(!entry_shows_at(&folded, "monitor", None, false));
        let alone = json!({ "id": "wa", "type": "widget", "of": "monitor", "size": "s" });
        assert!(entry_shows_at(&alone, "monitor", None, false));
        assert!(!entry_shows_at(&alone, "monitor", None, true));
    }

    // Ctrl over an icon is Explorer's: a point on one of their rects (the
    // page's px, as desk_icons reads them), its right and lower edges out.
    #[test]
    fn a_point_on_an_icon() {
        let icons = [[10.0, 20.0, 74.0, 84.0], [10.0, 110.0, 74.0, 84.0]];
        assert!(on_icon(&icons, 10.0, 20.0) && on_icon(&icons, 83.9, 103.9) && on_icon(&icons, 40.0, 150.0));
        assert!(!on_icon(&icons, 84.0, 50.0) && !on_icon(&icons, 40.0, 105.0) && !on_icon(&icons, 9.0, 50.0));
        assert!(!on_icon(&[], 40.0, 50.0));
    }

    #[test]
    fn a_point_in_a_rounded_rect() {
        let s = [100.0, 100.0, 156.0, 156.0, 22.0];
        assert!(in_shape(178.0, 178.0, &s, 0.0));
        assert!(in_shape(100.0, 178.0, &s, 0.0));
        // Its corner cut round: the rect's corner is out, a little in is in.
        assert!(!in_shape(101.0, 101.0, &s, 0.0));
        assert!(in_shape(108.0, 108.0, &s, 0.0));
        assert!(!in_shape(99.0, 178.0, &s, 0.0));
        assert!(!in_shape(300.0, 178.0, &s, 0.0));
        // Near: within the margin round it.
        assert!(in_shape(80.0, 178.0, &s, 40.0));
        assert!(!in_shape(50.0, 178.0, &s, 40.0));
    }

    #[test]
    fn a_layout_merged_with_what_one_page_changed() {
        let old = vec![json!({ "id": "wa", "size": "s" }), json!({ "id": "wb", "size": "s" }), json!({ "id": "wc", "size": "s" })];
        let merged = merge(&old, vec![json!({ "id": "wb", "size": "m" }), json!({ "id": "wd", "size": "l" })], &["wa".to_string()]);
        assert_eq!(merged, json!([{ "id": "wb", "size": "m" }, { "id": "wc", "size": "s" }, { "id": "wd", "size": "l" }]));
        assert_eq!(merge(&old, vec![], &[]), Value::Array(old.clone()));
    }

    #[test]
    fn a_displays_key_is_short_and_steady() {
        let a = short_key(r"\\?\DISPLAY#DELA0D3#5&2f1c3b0a&0&UID4352#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}");
        assert_eq!(a.len(), 13);
        assert!(a.starts_with('d') && a[1..].chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(a, short_key(r"\\?\display#dela0d3#5&2f1c3b0a&0&uid4352#{E6F07B5F-EE97-4A90-B076-33F57BF4EAA7}"));
        assert_ne!(a, short_key(r"\\.\DISPLAY2"));
    }

    // On a desktop: each display's own name, the same twice.
    #[test]
    fn this_computers_displays_have_names() {
        let Some(id) = imp::device_of(r"\\.\DISPLAY1") else { return };
        println!("{id}");
        assert!(!id.is_empty());
        assert_eq!(imp::device_of(r"\\.\DISPLAY1"), Some(id));
    }
}
