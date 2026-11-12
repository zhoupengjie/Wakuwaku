// The desktop's widgets (the widget design of 2026-10-16, §7; desk.ts draws
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
// loop (loops.rs) asks poll() where the pointer is, and the window takes the
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
// minute for a slideshow); desk.ts blurs it and gives each widget its part.
//
// The layout is the settings' deskLayout (settings.rs is_desk_layout); a page
// keeps what it changed (desk_keep, merged with the rest), and her island's
// page puts widgets on and off the desktop (island-wmenu.ts placeWidget).
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use serde_json::{json, Value};
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::{screen::Area, Shared};
use crate::lock::Lock;

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

static DESKS: Lock<Vec<Desk>> = Lock::new("desk::DESKS", Vec::new());
static NEXT: AtomicU32 = AtomicU32::new(1);
static SHARED: OnceLock<Arc<Shared>> = OnceLock::new();

// The pointer this close to a widget (the page's px): looked at often.
const NEAR_PX: f64 = 40.0;

// Each display with what it shows, from the layout: its own, and on the
// main display those whose display is not there (deskgrid.ts shownHere);
// and one a widget is being dragged over from her home, though nothing is
// on it yet (Drag keep).
fn wanted(sh: &Shared) -> Vec<(Area, String, bool)> {
    if !cfg!(windows) {
        return Vec::new();
    }
    let layout = sh.config().desk_layout.clone();
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
// is not there (deskgrid.ts shownHere).
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
    let layout = sh.config().desk_layout.clone();
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
            // Ctrl, the rectangle: desk.ts).
            "accent": accent,
            "detailed": sh.config().details,
            "home": sh.home(),
            // What the taskbar shows while it is her home (a board's Add says where each one is).
            "bar": if sh.home() == "taskbar" { sh.config().taskbar_layout.clone() } else { Value::Null },
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
    let id = sh.config().pet.clone();
    let id = id.as_str().unwrap_or("");
    crate::data::pets(&sh.dir).iter().find(|p| p.id == id).map_or(Value::Null, |p| json!(p.url()))
}

// Windows' mode and accent ({ base, light, dark }, or null), as last read
// (read again as Windows says its colours changed).
static LOOK: Lock<Option<(bool, Value)>> = Lock::new("desk::LOOK", None);
fn look_now() -> (bool, Value) {
    LOOK.lock().unwrap().get_or_insert_with(look_read).clone()
}
fn look_read() -> (bool, Value) {
    let (light, accent) = crate::wallpaper::look();
    (light, accent.map_or(Value::Null, |a| json!({ "base": a.base, "light": a.light, "dark": a.dark })))
}

// The widgets' words, to the pages (with the island's, shared.rs redraw_widgets):
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
        crate::loops::wake_monitor();
    } else {
        views(sh);
    }
}

// Whether each page's cards may show their plugins' views (tier 2,
// src/pet/views.ts), told as it changes (desk:views { on }): not while
// nobody is about, nor while its desktop is not seen (as its words are not
// sent then: push_widgets), nor while the island's watchdog has them taken
// away (addons/ pause_views). Looked at only while a plugin with a view
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
static SEEN: Lock<Option<(std::time::Instant, Vec<(Mon, bool)>)>> = Lock::new("desk::SEEN", None);
fn seen_now(sh: &Shared, mon: Mon) -> bool {
    if let Some((at, list)) = SEEN.lock().unwrap().as_ref() {
        if at.elapsed() < std::time::Duration::from_secs(1) {
            if let Some((_, s)) = list.iter().find(|(m, _)| *m == mon) {
                return *s;
            }
        }
    }
    count(&LOOKS);
    let own = sh.own_windows();
    let shown = imp::desktop_shown(&own);
    let full = crate::fullscreen::covering(&own).flatten();
    let mons: Vec<Mon> = DESKS.lock().unwrap().iter().map(|d| d.mon).collect();
    let list: Vec<(Mon, bool)> = mons.into_iter().map(|m| (m, shown || !(full == Some(m) || crate::tasks::maximized_on(m)))).collect();
    let s = list.iter().find(|(m, _)| *m == mon).map_or(true, |(_, s)| *s);
    *SEEN.lock().unwrap() = Some((std::time::Instant::now(), list));
    s
}

// Its window: the display but for its last row of pixels. A window the
// display's own size, coming or going, is a program full screen to
// Explorer: its taskbar came out again and set the work area to the whole
// display, every maximized window laid out twice as hers set it back, the
// whole screen flashing as a widget was dragged to a desktop with nothing
// on it (its window made for the drag, then let go: the user, 2026-11-04).
// Its place is the display's corner, so its page's px are the display's.
fn window_rect(mon: Mon) -> Mon {
    (mon.0, mon.1, mon.2, (mon.3 - 1).max(1))
}

fn create(sh: &Arc<Shared>, label: String, area: Area) {
    let sh = sh.clone();
    std::thread::spawn(move || {
        let rect = window_rect(area.mon);
        crate::main_wait::check("building a window");
        let built = WebviewWindowBuilder::new(&sh.app, &label, WebviewUrl::App("pet/desk.html".into()))
            .title("wakuwaku desk")
            .inner_size(rect.2 as f64 / area.sf, rect.3 as f64 / area.sf)
            .additional_browser_args(crate::webview::browser_args(&sh))
            .data_directory(crate::webview::webview_dir(&sh))
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
        let hwnd = crate::main_wait::hwnd(&win).unwrap_or(0);
        #[cfg(not(windows))]
        let hwnd = 0;
        crate::tasks::no_transitions(hwnd);
        sh.own_window(&win);
        // Its cards' plugin views kept to their own page (addons/ guard_frames).
        crate::addons::guard_frames(&sh, &win, false);
        lay(&win, rect);
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

// Its parts, each in a file of its own; named here for the others (each
// has `use super::*`) and, the commands and what other modules call, for
// the crate.
mod commands;
mod counts;
mod displays;
mod drag;
mod pointer;
pub use commands::*;
pub use counts::*;
pub use displays::*;
pub use drag::*;
pub use pointer::*;

#[cfg(windows)]
#[path = "win.rs"]
mod imp;

#[cfg(not(windows))]
#[path = "other.rs"]
mod imp;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn its_window_is_never_the_displays_own_size() {
        // From the display's corner, its row of pixels at the foot left out:
        // Explorer takes no program full screen for it.
        assert_eq!(window_rect((0, 0, 2560, 1440)), (0, 0, 2560, 1439));
        assert_eq!(window_rect((-1920, 200, 1920, 1080)), (-1920, 200, 1920, 1079));
    }

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

    // Without a device name, the rect's own key, the same as ever.
    #[test]
    fn a_display_without_a_name_is_keyed_by_its_rect() {
        assert_eq!(key_for(None, (2560, 0, 1920, 1080)), short_key("1920x1080@2560,0"));
        assert_eq!(key_for(None, (-1920, 0, 1920, 1080)), short_key("1920x1080@-1920,0"));
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
