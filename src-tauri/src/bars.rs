// Her taskbar on the other displays (taskbarDisplays: all): a strip on each
// display but hers, a window of its own (bar-1, bar-2, ...: pet/bar.html),
// its app bar along the same edge as hers, with Start, the windows there and
// the clock; her capsule, the tray and the widgets stay on hers, as Windows'
// own keeps its tray on the main display (the appearance session's spec,
// 2026-10-13), but for a prompt waiting: then her capsule comes at the
// start of each, "waiting for your approval · N", a press showing it in her
// island. Which windows show on which follows Windows' own setting for
// taskbars on several displays (MMTaskbarMode). The main display only (the
// default): no taskbar on the others at all, Windows' own put away there too
// (shell.rs rehide), and their work areas whole (keep_work).
// Not while her home is the island, nor while her taskbar is not up; an
// app full screen on a display hides that display's alone.
// Split (island.rs is_split, docs/design/specs/dock-topbar.md): one on her
// display too, the dock: Start and the programs at the foot of the screen,
// her strip the top bar. A window of its own (dock-1, ...) as big as its
// glass, as wide as its page asks (dock_size), in the middle of the foot in
// the band as high as Windows' room there; Windows' acrylic on the window
// itself (backplate.rs acrylic), so what is behind it shows blurred through
// it, the windows too, not the desktop's picture; in that room (splitDock
// room), or keeping none and sliding down out of the way of a window
// touching it (hide: look_under, poll, slide).
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::appbar::{self, Edge};
use crate::{island, screen::Area, taskbar, Shared};

type Mon = (i32, i32, i32, i32);

struct Bar {
    label: String,
    mon: Mon,
    sf: f64,
    edge: Edge,
    // Its window's handle once made (0 before), and its app bar's rect.
    hwnd: isize,
    strip: Option<Mon>,
    ready: bool,
    // An app full screen on its display.
    hidden: bool,
    // What its page was last told.
    sent: Value,
    // Its display's wallpaper, for its Mica (read_papers): as the main
    // strip's (taskbar.rs look_at_desktop), from its own strip's corner.
    paper: Value,
    // The work area's side on its edge as last set clear of the strip, and
    // when (keep_work); given back as it goes (give_work).
    work_side: Option<i32>,
    work_at: Option<Instant>,
    // Its page's zoom as set last (fit_zoom).
    zoom: f64,
    // The dock on her display (split), not a strip on another; how wide its
    // page asks it to be (its px, and their scale), whether a window of the
    // programs on it flashes for attention (refresh), and Windows' acrylic
    // on it as set (dark or light; none: its page draws Mica or solid).
    dock: bool,
    asked: Option<(f64, f64)>,
    flashing: bool,
    acrylic: Option<bool>,
    // The windows minimized into it, each where its page left room for its
    // live picture (its px from its corner) and their scale (show_mins).
    mins: Vec<(isize, (f64, f64, f64, f64))>,
    mins_sf: f64,
    // The Recycle Bin as last told it (empty or full), and when looked at.
    bin: Value,
    bin_at: Option<Instant>,
    // Shown yet (reveal): the dock comes once its programs are in, at once.
    shown: bool,
    // The dock out of the way of a window touching it (stow_look).
    stow: Stow,
}

// The dock out of the way of a window touching it (splitDock hide): a
// window's frame reaching into it (under, look_under), it slides down off
// the screen, a line left at the foot, and the pointer goes through to the
// window; the pointer resting at the foot of the screen calls it up
// (called), and it goes again once the pointer has been off it a while.
#[derive(Default)]
struct Stow {
    stowed: bool,
    under: bool,
    called: bool,
    edge_since: Option<Instant>,
    pressed: bool,
    away_since: Option<Instant>,
}

// The dock: in the band as high as Windows' room along the foot (logical),
// its glass (its window) 44 high and 2 px off the foot (bar.css .b-dock);
// out of the way, its top 3 px left on the screen, the line the pointer
// calls it up from.
const DOCK_H: f64 = 48.0;
const DOCK_GLASS: f64 = 44.0;
const DOCK_GAP: f64 = 2.0;
const DOCK_PEEK: f64 = 3.0;
const DOCK_SLIDE_MS: u64 = 180;
// The pointer this close to the foot of the screen (logical), this long, no
// button held meanwhile, calls it up (as her island at the top: island.rs
// call_look, but sooner: the user, 2026-11-02); off it (and this much
// beside or above it, or the foot of the screen) this long, it goes.
const DOCK_EDGE_PX: f64 = 2.0;
const DOCK_EDGE_MS: u128 = 150;
const DOCK_BAND: f64 = 12.0;
const DOCK_LEAVE_MS: u128 = 600;

static BARS: Mutex<Vec<Bar>> = Mutex::new(Vec::new());

// The strips' pages and their list's as the main taskbar's (island.rs
// fit_zoom): zoomed so that Windows' text size is taken out of their scale,
// each laid out at 48 of its px as thick as Windows' (zhoupengjie/Wakuwaku#90).
// The text size as her island's page has it; each once its page is up.
static POP_ZOOM: Mutex<f64> = Mutex::new(1.0);

pub fn fit_zoom(sh: &Shared) {
    let want = 1.0 / island::text_size(sh);
    let differs = |zoom: f64| (zoom - want).abs() > 0.001;
    let mut labels: Vec<String> = BARS
        .lock()
        .unwrap()
        .iter_mut()
        .filter(|b| b.ready && differs(b.zoom))
        .map(|b| {
            b.zoom = want;
            b.label.clone()
        })
        .collect();
    if POP_READY.load(Ordering::SeqCst) {
        let mut pop = POP_ZOOM.lock().unwrap();
        if differs(*pop) {
            *pop = want;
            labels.push(POP.to_string());
        }
    }
    if labels.is_empty() {
        return;
    }
    sh.log(&format!("bars: page zoom {want:.4} for {}", labels.join(", ")));
    for label in labels {
        if let Some(win) = sh.app.get_webview_window(&label) {
            let _ = win.set_zoom(want);
        }
    }
}
static NEXT: AtomicU32 = AtomicU32::new(1);
// The displays without a strip of hers whose work area was last made whole, and when (keep_work).
static BARE_AT: Mutex<Vec<(Mon, Instant)>> = Mutex::new(Vec::new());

// Her taskbar wanted on every display: her home the taskbar, and set so.
pub fn wanted(sh: &Shared) -> bool {
    sh.home() == "taskbar" && sh.setting("taskbarDisplays").as_str() == Some("all")
}

fn window(sh: &Shared, label: &str) -> Option<WebviewWindow> {
    sh.app.get_webview_window(label)
}

// Made or taken away as the displays and her settings say: one for each
// display but hers, while her taskbar is up.
pub fn sync(sh: &Arc<Shared>) {
    let up = taskbar::is_up();
    let all = wanted(sh) && up;
    let home = island::display(sh);
    // Split, the others' along the bottom (island.rs bars_edge).
    let edge = island::bars_edge(sh);
    let areas = sh.screens().areas;
    // (display, the dock?): a strip on each of the others; split, the dock on hers.
    let mut want: Vec<(Area, bool)> = if all { areas.iter().filter(|a| Some(a.mon) != home).map(|a| (*a, false)).collect() } else { Vec::new() };
    if up && island::is_split(sh) {
        want.extend(areas.iter().find(|a| Some(a.mon) == home).map(|a| (*a, true)));
    }
    let (gone, made, moved) = {
        let mut bars = BARS.lock().unwrap();
        let mut gone = Vec::new();
        bars.retain(|b| {
            let keep = want.iter().any(|(a, dock)| a.mon == b.mon && a.sf == b.sf && *dock == b.dock) && b.edge == edge;
            if !keep {
                gone.push(Gone::of(b));
            }
            keep
        });
        let mut made = Vec::new();
        for (a, dock) in &want {
            if !bars.iter().any(|b| b.mon == a.mon && b.dock == *dock) {
                let label = format!("{}-{}", if *dock { "dock" } else { "bar" }, NEXT.fetch_add(1, Ordering::SeqCst));
                bars.push(Bar {
                    label: label.clone(),
                    mon: a.mon,
                    sf: a.sf,
                    edge,
                    hwnd: 0,
                    strip: None,
                    ready: false,
                    hidden: false,
                    sent: Value::Null,
                    paper: Value::Null,
                    work_side: None,
                    work_at: None,
                    zoom: 1.0,
                    dock: *dock,
                    asked: None,
                    flashing: false,
                    acrylic: None,
                    mins: Vec::new(),
                    mins_sf: 1.0,
                    bin: Value::Null,
                    bin_at: None,
                    shown: false,
                    stow: Stow::default(),
                });
                made.push((label, *a, *dock));
            }
        }
        let moved = gone.len() + made.len() > 0;
        (gone, made, moved)
    };
    for g in gone {
        close(sh, g);
    }
    for (label, area, dock) in made {
        create(sh, label, area, edge, dock);
    }
    if moved {
        sh.log(&format!("bars: {} on the other displays and as the dock", BARS.lock().unwrap().len()));
        // A list open over one gone goes with it.
        send_pop(sh);
    }
}

// Every one of them gone (quitting, another home): their app bars given back first.
pub fn close_all(sh: &Shared) {
    hide_pop(sh);
    let all: Vec<Gone> = BARS.lock().unwrap().drain(..).map(|b| Gone::of(&b)).collect();
    for g in all {
        close(sh, g);
    }
}

// One going: what closing it needs, taken out of the list first.
struct Gone {
    label: String,
    hwnd: isize,
    mon: Mon,
    edge: Edge,
    work_side: Option<i32>,
    // The dock, which has no app bar.
    dock: bool,
}

impl Gone {
    fn of(b: &Bar) -> Gone {
        Gone { label: b.label.clone(), hwnd: b.hwnd, mon: b.mon, edge: b.edge, work_side: b.work_side, dock: b.dock }
    }
}

fn close(sh: &Shared, g: Gone) {
    if g.hwnd != 0 && !g.dock {
        appbar::remove(g.hwnd);
    }
    if g.dock {
        taskbar::dock_thumbs(Vec::new());
    }
    if let Some(win) = window(sh, &g.label) {
        let _ = win.destroy();
    }
    if let Some(side) = g.work_side {
        give_work(sh, &g.label, g.mon, g.edge, side);
    }
    sh.log(&format!("bars: {} gone", g.label));
}

// The work area on its display kept clear of its strip, as hers is on hers
// (taskbar.rs tick): Windows does not count the app bar, and a window
// maximized on a display at 175% lay 84 px under a strip along its left,
// the work area the whole display (the test session, 2026-10-13). Along
// the bottom the room Windows' own keeps there mostly does it already. Its
// side on the strip's edge set where the strip starts: at once for a new
// one, then at most every two seconds should something set it back
// (Explorer, as app bars come and go).
pub fn keep_work(sh: &Shared) {
    let now = Instant::now();
    let due: Vec<(String, Mon, Edge, Mon)> = {
        let mut bars = BARS.lock().unwrap();
        // The dock keeps no work area of its own: Windows' room along the
        // foot is it, or none (it steps out of the way).
        bars.iter_mut()
            .filter(|b| !b.dock)
            .filter_map(|b| {
                let strip = b.strip?;
                if b.work_at.is_some_and(|at| now.duration_since(at) < Duration::from_secs(2)) {
                    return None;
                }
                let work = appbar::work_of(b.mon)?;
                if !b.edge.reaches(work, strip) {
                    return None;
                }
                b.work_side = Some(b.edge.inner(strip));
                b.work_at = Some(now);
                Some((b.label.clone(), work, b.edge, strip))
            })
            .collect()
    };
    for (label, work, edge, strip) in due {
        let to = edge.with_side(work, edge.inner(strip));
        let ok = appbar::set_work(to);
        // The windows maximized there laid on it anew: set quietly, it moves none.
        let moved = if ok { appbar::refit_maximized(work, to) } else { 0 };
        sh.log(&format!("bars: {label}'s display's work area {work:?} reaches into its strip {strip:?}: made {to:?} ({ok}), {moved} maximized window(s) laid on it anew"));
    }
    keep_bare(sh, now);
}

// The displays without a strip of hers (her taskbar on the main display
// alone, or one not made yet): no taskbar there at all, Windows' own put
// away there too (shell.rs rehide), so the band it keeps along the foot
// while it keeps its room (her strip along the bottom) is taken back, the
// work area the whole display, and the windows maximized there laid on
// it. With hers on the main display alone, the second showed Windows' own
// (the user, 2026-10-13: whosever it is, none is to show there). At most
// every two seconds a display; Windows' own given its room back as it
// shows again (shell.rs rooms_back).
fn keep_bare(sh: &Shared, now: Instant) {
    let home = island::display(sh);
    let with_bars: Vec<Mon> = BARS.lock().unwrap().iter().map(|b| b.mon).collect();
    for a in sh.screens().areas {
        if Some(a.mon) == home || with_bars.contains(&a.mon) {
            continue;
        }
        let foot = a.mon.1 + a.mon.3;
        let Some(work) = appbar::work_of(a.mon) else { continue };
        let Some(room) = crate::shell::room(a.mon) else { continue };
        if work.1 + work.3 != room.1 || room.1 >= foot {
            continue;
        }
        {
            let mut at = BARE_AT.lock().unwrap();
            if at.iter().any(|(m, t)| *m == a.mon && now.duration_since(*t) < Duration::from_secs(2)) {
                continue;
            }
            at.retain(|(m, _)| *m != a.mon);
            at.push((a.mon, now));
        }
        let to = Edge::Bottom.with_side(work, foot);
        let ok = appbar::set_work(to);
        let moved = if ok { appbar::refit_maximized(work, to) } else { 0 };
        sh.log(&format!("bars: the display {:?} without a strip of hers: Windows' kept room taken back, work area {work:?} made {to:?} ({ok}), {moved} maximized window(s) laid on it anew", a.mon));
    }
}

// Given back as it goes: the work area's side where it was set for the
// strip moved back to the display's edge (Windows' own is put away there
// too, its room taken back: keep_bare; it is given it again as it shows,
// shell.rs rooms_back); left as it is if something else has set it since
// (Explorer, counting again).
fn give_work(sh: &Shared, label: &str, mon: Mon, edge: Edge, side: i32) {
    let Some(work) = appbar::work_of(mon) else { return };
    if edge.side_of(work) != side {
        return;
    }
    let back = match edge {
        Edge::Bottom => mon.1 + mon.3,
        Edge::Top => mon.1,
        Edge::Left => mon.0,
        Edge::Right => mon.0 + mon.2,
    };
    let to = edge.with_side(work, back);
    let ok = appbar::set_work(to);
    let moved = if ok { appbar::refit_maximized(work, to) } else { 0 };
    sh.log(&format!("bars: {label}'s display's work area given back: {work:?} made {to:?} ({ok}), {moved} maximized window(s) laid on it anew"));
}

// Its window made off the main thread (a menu click may be what asks, and
// building there waits on itself), its strip asked of Windows along the
// edge of its own display, as thick as hers at that display's scale.
fn create(sh: &Arc<Shared>, label: String, area: Area, edge: Edge, dock: bool) {
    let sh = sh.clone();
    std::thread::spawn(move || {
        let thick = (island::TASKBAR_H * area.sf).round() as i32;
        // The dock: in the middle of the foot, as wide as its page will ask (dock_size).
        let strip = if dock { dock_rect(area.mon, area.sf, DOCK_H * 8.0) } else { edge.strip(area.mon, thick) };
        let built = WebviewWindowBuilder::new(&sh.app, &label, WebviewUrl::App("pet/bar.html".into()))
            .title("wakuwaku bar")
            .inner_size(strip.2 as f64 / area.sf, strip.3 as f64 / area.sf)
            .additional_browser_args(crate::browser_args(&sh))
            .data_directory(crate::webview_dir(&sh))
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
            .build();
        let win = match built {
            Ok(win) => win,
            Err(err) => return sh.log(&format!("bars: {label} not made: {err}")),
        };
        #[cfg(windows)]
        let hwnd = win.hwnd().map(|h| h.0 as isize).unwrap_or(0);
        #[cfg(not(windows))]
        let hwnd = 0;
        crate::tasks::no_transitions(hwnd);
        sh.own_window(&win);
        // The dock asks Windows for no room: Windows' own keeps it, or none is kept.
        let granted = if dock { strip } else if hwnd != 0 && appbar::register(hwnd) { appbar::place(hwnd, area.mon, edge, thick) } else { strip };
        lay(&win, granted);
        // Zoomed before its page lays out (fit_zoom).
        let zoom = 1.0 / island::text_size(&sh);
        let _ = win.set_zoom(zoom);
        let gone = {
            let mut bars = BARS.lock().unwrap();
            match bars.iter_mut().find(|b| b.label == label) {
                Some(b) => {
                    b.hwnd = hwnd;
                    b.strip = Some(granted);
                    b.zoom = zoom;
                    false
                }
                None => true,
            }
        };
        // Taken away again while it was being made.
        if gone {
            return close(&sh, Gone { label, hwnd, mon: area.mon, edge, work_side: None, dock });
        }
        // The dock shows once its page has its programs (dock_size ready), or
        // a moment on whatever it has, sliding up from the foot all at once.
        if dock {
            let sh = sh.clone();
            let label = label.clone();
            std::thread::spawn(move || {
                std::thread::sleep(DOCK_READY_AT_MOST);
                reveal(&sh, &label);
            });
            return;
        }
        let _ = win.show();
        let laid = match (win.outer_position(), win.outer_size()) {
            (Ok(p), Ok(s)) => format!("({}, {}, {}, {})", p.x, p.y, s.width, s.height),
            _ => "?".into(),
        };
        sh.log(&format!("bars: {label} on {:?} at {}%, strip {granted:?} along the {edge:?}, window {laid}", area.mon, (area.sf * 100.0).round()));
        // The work area there clear of it at once, not at the next look.
        keep_work(&sh);
    });
}

// A window laid on a rect of the screen, in physical px. Its place first:
// moved to a display of another scale, Windows gives it a size for that one
// (the scales' ratio over what it had), which the size then replaces; and
// its place again, should the new size have moved it. Sized first, a strip
// made on the main display at 125% came out 1.4 times too long and thick on
// one at 175% (3584 x 118 for 2560 x 84), its clock and the desktop's
// sliver past the screen's end and its buttons off centre (the test session,
// 2026-10-13).
fn lay(win: &WebviewWindow, rect: Mon) {
    let _ = win.set_position(PhysicalPosition::new(rect.0, rect.1));
    let _ = win.set_size(PhysicalSize::new(rect.2.max(1) as u32, rect.3.max(1) as u32));
    let _ = win.set_position(PhysicalPosition::new(rect.0, rect.1));
}

// --- The dock (split) -----------------------------------------------------------------

// Its window (physical): w wide (physical, no narrower than it is high, no
// wider than the display), its glass high at sf, in the middle of the
// display's foot DOCK_GAP off it.
fn dock_rect(mon: Mon, sf: f64, w: f64) -> Mon {
    let height = (DOCK_GLASS * sf).round() as i32;
    let gap = (DOCK_GAP * sf).round() as i32;
    let width = (w.round() as i32).clamp(height, mon.2.max(height));
    (mon.0 + (mon.2 - width) / 2, mon.1 + mon.3 - gap - height, width, height)
}

// Its window's top (physical) up, or out of the way: down past the foot
// but for its top DOCK_PEEK.
fn dock_top(rect: Mon, mon: Mon, sf: f64, stowed: bool) -> i32 {
    if stowed { mon.1 + mon.3 - ((DOCK_PEEK * sf).round() as i32).max(1) } else { rect.1 }
}

// Its window slid to where it is to be, up or down (its acrylic goes with
// it, as the page could not take it), eased out over DOCK_SLIDE_MS; a newer
// slide stops an older one.
static SLIDE: AtomicU32 = AtomicU32::new(0);

fn slide(sh: &Shared, win: WebviewWindow, x: i32, to: i32) {
    let me = SLIDE.fetch_add(1, Ordering::SeqCst) + 1;
    // Its minimized windows' pictures off as it goes, back once it is up.
    show_mins(sh, true);
    let sh = crate::shared(&sh.app);
    std::thread::spawn(move || {
        let from = win.outer_position().map_or(to, |p| p.y);
        let steps = (DOCK_SLIDE_MS / 15).max(1);
        for n in 1..=steps {
            if SLIDE.load(Ordering::SeqCst) != me {
                return;
            }
            let t = n as f64 / steps as f64;
            let eased = 1.0 - (1.0 - t).powi(3);
            let _ = win.set_position(PhysicalPosition::new(x, from + ((to - from) as f64 * eased).round() as i32));
            std::thread::sleep(Duration::from_millis(15));
        }
        if SLIDE.load(Ordering::SeqCst) == me {
            show_mins(&sh, false);
        }
    });
}

// The dock shown the first time: from below the foot of the screen, slid
// up to where it is to be (out of the way already, a window touching it:
// its top at the foot), once.
const DOCK_READY_AT_MOST: Duration = Duration::from_millis(1500);

fn reveal(sh: &Shared, label: &str) {
    let at = {
        let mut bars = BARS.lock().unwrap();
        let Some(b) = bars.iter_mut().find(|b| b.label == label && b.dock && !b.shown) else { return };
        let Some(rect) = b.strip else { return };
        b.shown = true;
        (rect, b.mon, b.sf, b.stow.stowed, b.hidden)
    };
    let (rect, mon, sf, stowed, hidden) = at;
    let Some(win) = window(sh, label) else { return };
    lay(&win, (rect.0, mon.1 + mon.3, rect.2, rect.3));
    if hidden {
        return;
    }
    let _ = win.show();
    sh.log(&format!("bars: the dock up, its programs in ({rect:?})"));
    slide(sh, win, rect.0, dock_top(rect, mon, sf, stowed));
}

// Where the dock's page left room for the windows minimized into it (each
// window's id and x, y, w, h in its px; dpr, their scale): their live
// pictures there while it is up.
#[tauri::command]
pub fn dock_thumbs(app: tauri::AppHandle, window: WebviewWindow, items: Vec<(i64, f64, f64, f64, f64)>, dpr: Option<f64>) {
    let sh = crate::shared(&app);
    {
        let mut bars = BARS.lock().unwrap();
        let Some(b) = bars.iter_mut().find(|b| b.label == window.label() && b.dock) else { return };
        b.mins = items.into_iter().map(|(h, x, y, w, ht)| (h as isize, (x, y, w, ht))).collect();
        b.mins_sf = page_scale(dpr, b.sf);
    }
    show_mins(&sh, false);
}

// The minimized windows' live pictures on the dock, where its page left
// room for them, while it is up (not out of the way, nor sliding: off,
// nor hidden by an app full screen); none else.
fn show_mins(_sh: &Shared, off: bool) {
    let items = {
        let bars = BARS.lock().unwrap();
        bars.iter()
            .find(|b| b.dock && b.ready && b.shown && !b.hidden && !b.stow.stowed && !off)
            .and_then(|b| b.strip.map(|r| (r, b)))
            .map(|(r, b)| {
                let px = |v: f64| (v * b.mins_sf).round() as i32;
                b.mins.iter().map(|&(h, (x, y, w, ht))| (h, (r.0 + px(x), r.1 + px(y), px(w), px(ht)))).collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    taskbar::dock_thumbs(items);
}

// Its page laid out: as wide as it asks (w, its px; dpr, their scale), laid
// again in the middle of the foot, its wallpaper read for where it is now.
#[tauri::command]
pub fn dock_size(app: tauri::AppHandle, window: WebviewWindow, w: f64, dpr: Option<f64>, ready: Option<bool>) {
    let sh = crate::shared(&app);
    // Its programs in: it shows now (as wide as it is with them), once.
    let shown = BARS.lock().unwrap().iter().any(|b| b.label == window.label() && b.shown);
    if ready == Some(true) && !shown {
        let label = window.label().to_string();
        let sh = sh.clone();
        std::thread::spawn(move || {
            // Laid at its width first (below), then up.
            std::thread::sleep(Duration::from_millis(30));
            reveal(&sh, &label);
        });
    }
    let laid = {
        let mut bars = BARS.lock().unwrap();
        let Some(b) = bars.iter_mut().find(|b| b.label == window.label() && b.dock) else { return };
        let page_sf = page_scale(dpr, b.sf);
        b.asked = Some((w, page_sf));
        let rect = dock_rect(b.mon, b.sf, w * page_sf);
        (b.strip != Some(rect)).then(|| {
            b.strip = Some(rect);
            (rect, dock_top(rect, b.mon, b.sf, b.stow.stowed))
        })
    };
    let Some((rect, top)) = laid else { return };
    // Out of the way, it stays down there at its new width; not shown yet,
    // below the foot, where it will slide up from (reveal).
    let not_yet = BARS.lock().unwrap().iter().any(|b| b.label == window.label() && !b.shown);
    let top = if not_yet { BARS.lock().unwrap().iter().find(|b| b.label == window.label()).map_or(top, |b| b.mon.1 + b.mon.3) } else { top };
    lay(&window, (rect.0, top, rect.2, rect.3));
    show_mins(&sh, false);
    // Its Mica's picture where it is now; none to draw under Windows' acrylic
    // (as it grows a frame at a time, each frame read it again).
    if BARS.lock().unwrap().iter().any(|b| b.label == window.label() && b.acrylic.is_none()) {
        sh.log(&format!("bars: the dock {} px wide, laid at {rect:?}", w.round()));
        std::thread::spawn(move || read_papers(&sh));
    }
}

// One look at the pointer (poll; physical) for the dock out of the way:
// whether it is to be now, called up or let go as stow_look says. Returns
// ms until the next look: sooner near the foot of the screen.
pub fn poll(sh: &Shared, cursor: (i32, i32)) -> u64 {
    let hides = island::is_split(sh) && !island::dock_keeps_room(sh);
    let held = OPEN.lock().unwrap().as_ref().map(|p| p.bar.clone());
    let changed = {
        let mut bars = BARS.lock().unwrap();
        // Not before it has come up (reveal).
        let Some(b) = bars.iter_mut().find(|b| b.dock && b.ready && !b.hidden && b.shown) else { return 500 };
        let Some(rect) = b.strip else { return 500 };
        // Kept up while its list or menu is open, Windows' Start menu is, or
        // a window of its programs flashes for attention.
        let keep = !hides || held.as_deref() == Some(b.label.as_str()) || taskbar::start_open() || b.flashing;
        let now = Instant::now();
        let stowed = stow_look(&mut b.stow, rect, b.mon, b.sf, cursor, keep, now, || crate::pet::any_down().unwrap_or(false));
        (stowed != b.stow.stowed).then(|| {
            b.stow.stowed = stowed;
            (b.label.clone(), stowed, rect.0, dock_top(rect, b.mon, b.sf, stowed))
        })
    };
    if let Some((label, stowed, x, top)) = changed {
        sh.log(&format!("bars: the dock {}", if stowed { "out of the way of the window touching it" } else { "up" }));
        if let Some(win) = window(sh, &label) {
            // Out of the way, the pointer goes through it to the window.
            let _ = win.set_ignore_cursor_events(stowed);
            slide(sh, win, x, top);
        }
    }
    look_at_bin(sh);
    let near = BARS.lock().unwrap().iter().find(|b| b.dock).is_some_and(|b| cursor.1 >= b.mon.1 + b.mon.3 - (60.0 * b.sf) as i32);
    if hides && near { 40 } else { 300 }
}

// Out of the way or not, at one look: a window touching it (under), not
// kept up (keep), not called up by the pointer resting at the display's
// foot (cursor; mon, the display; sf its scale), or let go by it since.
// down: whether a button is held, asked only at the foot.
fn stow_look(s: &mut Stow, rect: Mon, mon: Mon, sf: f64, cursor: (i32, i32), keep: bool, now: Instant, down: impl FnOnce() -> bool) -> bool {
    let px = |v: f64| (v * sf).round() as i32;
    let foot = mon.1 + mon.3;
    // On it or this near it, down to the foot of the screen (the gap under
    // its glass is its own: the pointer resting there is on it).
    let on = |band: i32| cursor.0 >= rect.0 - band && cursor.0 < rect.0 + rect.2 + band && cursor.1 >= rect.1 - band && cursor.1 < foot;
    let at_foot = cursor.0 >= mon.0 && cursor.0 < mon.0 + mon.2 && cursor.1 >= foot - px(DOCK_EDGE_PX).max(1) && cursor.1 < foot;
    if keep || !s.under {
        s.called = false;
        s.edge_since = None;
        s.away_since = None;
        s.pressed = false;
        return false;
    }
    if s.called || (!s.stowed && on(0)) {
        s.called = true;
        // Up while the pointer is on it or near it, or still at the foot of
        // the screen it was called up from: gone from there and back, it was
        // let go and called again, over and over (the user, 2026-11-02).
        if on(px(DOCK_BAND)) || at_foot {
            s.away_since = None;
            return false;
        }
        if now.duration_since(*s.away_since.get_or_insert(now)).as_millis() < DOCK_LEAVE_MS {
            return false;
        }
        s.called = false;
        s.away_since = None;
        return true;
    }
    if !at_foot {
        s.edge_since = None;
        s.pressed = false;
        return true;
    }
    s.pressed = s.pressed || down();
    let since = *s.edge_since.get_or_insert(now);
    if s.pressed || now.duration_since(since).as_millis() < DOCK_EDGE_MS {
        return true;
    }
    s.called = true;
    s.edge_since = None;
    false
}

// The dock's Recycle Bin: looked at every few seconds (empty or full, as
// Windows draws it: taskbar.rs recycle_bin), its page told as it changes.
const BIN_EVERY: Duration = Duration::from_secs(3);

fn look_at_bin(sh: &Shared) {
    let due = BARS.lock().unwrap().iter().find(|b| b.dock && b.ready).map(|b| (b.label.clone(), b.bin_at.is_none_or(|at| at.elapsed() >= BIN_EVERY)));
    let Some((label, true)) = due else { return };
    let now = taskbar::recycle_bin();
    let changed = {
        let mut bars = BARS.lock().unwrap();
        let Some(b) = bars.iter_mut().find(|b| b.label == label) else { return };
        b.bin_at = Some(Instant::now());
        (b.bin != now).then(|| {
            b.bin = now.clone();
        })
    };
    if changed.is_some() {
        let _ = sh.app.emit_to(label.as_str(), "dock:bin", now);
    }
}

// A window touching the dock, or none now (with her island's look_under,
// main.rs): kept for poll.
pub fn look_under(sh: &Shared) {
    let hides = island::is_split(sh) && !island::dock_keeps_room(sh);
    let rect = BARS.lock().unwrap().iter().find(|b| b.dock && b.ready && !b.hidden).and_then(|b| b.strip);
    // Kept as it was while nobody is about.
    let Some(rect) = rect.filter(|_| !sh.is_quiet()) else { return };
    let on = hides && crate::tasks::reaches_into(rect);
    let mut bars = BARS.lock().unwrap();
    if let Some(b) = bars.iter_mut().find(|b| b.dock) {
        if std::mem::replace(&mut b.stow.under, on) != on {
            sh.log(&format!("bars: {} the dock {rect:?}", if on { "a window touching" } else { "no window touching" }));
        }
    }
}

// The keyboard, the network and the volume as they change (taskbar.rs
// keys_loop): told the dock, for quick settings in it (splitKeys dock).
pub fn keys(sh: &Shared, now: &Value) {
    let docks: Vec<String> = BARS.lock().unwrap().iter().filter(|b| b.dock && b.ready).map(|b| b.label.clone()).collect();
    for label in docks {
        let _ = sh.app.emit_to(label.as_str(), "dock:keys", now.clone());
    }
}

// Windows' Start menu opened or closed (taskbar.rs start_seen): the dock's
// Start button lit as it is open, the dock up meanwhile (poll).
pub fn start_seen(sh: &Shared, open: bool) {
    let docks: Vec<String> = BARS.lock().unwrap().iter().filter(|b| b.dock).map(|b| b.label.clone()).collect();
    for label in docks {
        let _ = sh.app.emit_to(label.as_str(), "dock:start", json!({ "open": open }));
    }
}

// Its page has its listeners up: told what to show; its display's
// wallpaper read for it (off this thread, with the desktop's look).
#[tauri::command]
pub fn bar_ready(app: tauri::AppHandle, window: WebviewWindow) {
    let sh = crate::shared(&app);
    let new = match BARS.lock().unwrap().iter_mut().find(|b| b.label == window.label()) {
        Some(b) => {
            b.ready = true;
            b.sent = Value::Null;
            b.paper.is_null()
        }
        None => false,
    };
    fit_zoom(&sh);
    refresh(&sh);
    if new {
        taskbar::look_now();
    }
}

// Each one's display's wallpaper, for its Mica: which picture Windows shows
// there and how it lays it (wallpaper.rs, each display's own), where that
// display and all of them are from the strip's corner, in its px; told
// with the rest (refresh). With the desktop's look (taskbar.rs
// look_at_desktop: as it changes, and every half minute), off the main thread.
pub fn read_papers(sh: &Shared) {
    let bars: Vec<(String, Mon, f64)> = BARS.lock().unwrap().iter().filter_map(|b| b.strip.map(|s| (b.label.clone(), s, b.sf))).collect();
    if bars.is_empty() {
        return refresh(sh);
    }
    let areas = sh.screens().areas;
    let left = areas.iter().map(|a| a.mon.0).min().unwrap_or(0);
    let top = areas.iter().map(|a| a.mon.1).min().unwrap_or(0);
    let right = areas.iter().map(|a| a.mon.0 + a.mon.2).max().unwrap_or(0);
    let bottom = areas.iter().map(|a| a.mon.1 + a.mon.3).max().unwrap_or(0);
    let screen = (left, top, right - left, bottom - top);
    for (label, strip, sf) in bars {
        let Some(mon) = BARS.lock().unwrap().iter().find(|b| b.label == label).map(|b| b.mon) else { continue };
        let Some(paper) = crate::wallpaper::read(mon) else { continue };
        if let Some(file) = &paper.file {
            let _ = sh.app.asset_protocol_scope().allow_file(file);
        }
        let now = paper_from(&paper, strip, mon, screen, sf);
        if let Some(b) = BARS.lock().unwrap().iter_mut().find(|b| b.label == label) {
            if b.paper != now {
                sh.log(&format!("bars: {label} wallpaper {:?}, {}, {}", paper.file, paper.position, paper.color));
                b.paper = now;
            }
        }
    }
    refresh(sh);
}

// A display's wallpaper for a strip's page (mica.js place): the display and
// all of them from the strip's corner, in its px.
fn paper_from(paper: &crate::wallpaper::Paper, strip: Mon, mon: Mon, screen: Mon, sf: f64) -> Value {
    let rel = |(x, y, w, h): Mon| json!([(x - strip.0) as f64 / sf, (y - strip.1) as f64 / sf, w as f64 / sf, h as f64 / sf]);
    let url = paper.file.as_ref().map(|f| format!("{}?v={}", crate::data::asset_url(f), paper.stamp));
    json!({ "url": url, "position": paper.position, "color": paper.color, "mon": rel(mon), "screen": rel(screen), "sf": sf })
}

// The prompts waiting, for her capsule there (the appearance session's spec,
// 2026-10-13): how many, and "answer" when the one shown first is a
// question, "approve" otherwise (a permission, a plan); none when none wait.
fn waiting(views: &Value) -> Value {
    let list = views.as_array().map(Vec::as_slice).unwrap_or_default();
    match list.first() {
        Some(first) => json!({ "n": list.len(), "kind": if first["kind"] == "question" { "answer" } else { "approve" } }),
        None => Value::Null,
    }
}

// Her capsule there pressed: the prompt shown in her island on the main
// display, as a press on hers (island.rs show_asks).
#[tauri::command]
pub fn bar_asks(app: tauri::AppHandle) {
    let sh = crate::shared(&app);
    if !sh.asks.lock().unwrap().is_empty() {
        island::show_asks(&sh);
    }
}

// One of them up on a display no app full screen covers: her capsule there
// shows the prompts waiting (main.rs asks_seen).
pub fn any_shown() -> bool {
    BARS.lock().unwrap().iter().any(|b| b.ready && !b.hidden && b.strip.is_some() && !b.dock)
}

// An app full screen covering a display (or none): that display's taskbar
// steps aside, the others come back.
pub fn full_screen(sh: &Shared, cover: Option<Mon>) {
    let changed: Vec<(String, bool)> = {
        let mut bars = BARS.lock().unwrap();
        bars.iter_mut()
            .filter_map(|b| {
                let hide = cover == Some(b.mon);
                (hide != b.hidden).then(|| {
                    b.hidden = hide;
                    (b.label.clone(), hide)
                })
            })
            .collect()
    };
    for (label, hide) in changed {
        let shown = BARS.lock().unwrap().iter().find(|b| b.label == label).is_none_or(|b| !b.dock || b.shown);
        if let Some(win) = window(sh, &label).filter(|_| shown || hide) {
            let _ = if hide { win.hide() } else { win.show() };
        }
        sh.log(&format!("bars: {label} {}", if hide { "stepped aside (an app full screen there)" } else { "back" }));
        show_mins(sh, false);
        // A list open over one stepped aside goes with it.
        if hide {
            send_pop(sh);
        }
    }
}

// --- A program's windows or what can be done with it, over its button -----------------
// As on her own taskbar (island.js openPop): the pointer resting on a
// running program's button, its windows, each a card with its live picture
// (Windows draws them, taskbar.rs thumbs); a right press, its menu (another
// window, keep it on the taskbar or no longer, close). A strip there is too
// thin to hold them: one window of their own (barpop: pet/barpop.html),
// made the first time, laid against the strip over the button, for any of
// them. It goes once the pointer has left the button and it a moment
// (barpop.js), at a press anywhere else, or when its program goes.
const POP: &str = "barpop";

struct Pop {
    // The strip it is for, list or menu, the program, its button there (the
    // strip's own px, and their scale: its page's devicePixelRatio), and
    // where the window is once laid (physical).
    bar: String,
    kind: String,
    key: String,
    at: (f64, f64, f64, f64),
    bar_sf: f64,
    rect: Option<Mon>,
}

// A page's px to the screen's: its devicePixelRatio as it told it, which
// Windows' text size makes bigger than the display's scale (sf) it is on
// (zhoupengjie/Wakuwaku#90); that scale where it told none.
fn page_scale(dpr: Option<f64>, sf: f64) -> f64 {
    crate::island::valid_scale(dpr).unwrap_or(sf)
}

static OPEN: Mutex<Option<Pop>> = Mutex::new(None);
static POP_MADE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static POP_READY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

// A strip's display, scale, edge and rect, by its window's label.
fn bar_of(label: &str) -> Option<(Mon, f64, Edge, Mon)> {
    BARS.lock().unwrap().iter().find(|b| b.label == label && !b.hidden).and_then(|b| b.strip.map(|s| (b.mon, b.sf, b.edge, s)))
}

// From a strip's page: the windows (list) or the menu of a program, its
// button at (x, y, w, h) in the page's px.
#[tauri::command]
pub fn bar_pop(app: tauri::AppHandle, window: WebviewWindow, kind: String, key: String, at: (f64, f64, f64, f64), dpr: Option<f64>) {
    let sh = crate::shared(&app);
    let Some((_, sf, _, _)) = bar_of(window.label()) else { return };
    let bar_sf = page_scale(dpr, sf);
    *OPEN.lock().unwrap() = Some(Pop { bar: window.label().to_string(), kind, key, at, bar_sf, rect: None });
    if !POP_MADE.swap(true, Ordering::SeqCst) {
        return make_pop(&sh);
    }
    send_pop(&sh);
}

// The pointer on a strip's buttons or off them (bar.js), for the list to
// stay while it goes from one to the other (barpop.js).
#[tauri::command]
pub fn bar_pop_over(app: tauri::AppHandle, window: WebviewWindow, over: bool) {
    let open = OPEN.lock().unwrap().as_ref().is_some_and(|p| p.bar == window.label());
    if open {
        let _ = app.emit_to(POP, "barpop:bar", json!({ "over": over }));
    }
}

fn make_pop(sh: &Arc<Shared>) {
    let sh = sh.clone();
    std::thread::spawn(move || {
        let built = WebviewWindowBuilder::new(&sh.app, POP, WebviewUrl::App("pet/barpop.html".into()))
            .title("wakuwaku bar list")
            .inner_size(420.0, 300.0)
            .additional_browser_args(crate::browser_args(&sh))
            .data_directory(crate::webview_dir(&sh))
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
            .build();
        match built {
            Ok(win) => {
                #[cfg(windows)]
                crate::tasks::no_transitions(win.hwnd().map(|h| h.0 as isize).unwrap_or(0));
                sh.own_window(&win);
                // Zoomed before its page lays out the first list (fit_zoom).
                let want = 1.0 / island::text_size(&sh);
                *POP_ZOOM.lock().unwrap() = want;
                let _ = win.set_zoom(want);
            }
            Err(err) => {
                POP_MADE.store(false, Ordering::SeqCst);
                sh.log(&format!("bars: the list's window not made: {err}"));
            }
        }
    });
}

// Its page has its listeners up.
#[tauri::command]
pub fn barpop_ready(app: tauri::AppHandle) {
    POP_READY.store(true, Ordering::SeqCst);
    let sh = crate::shared(&app);
    fit_zoom(&sh);
    send_pop(&sh);
}

// What the open one shows, told its page (again as the windows change): the
// program as that strip shows it, and the look; gone with its program, or
// with its last window for a list.
fn send_pop(sh: &Shared) {
    if !POP_READY.load(Ordering::SeqCst) {
        return;
    }
    let Some((bar, kind, key)) = OPEN.lock().unwrap().as_ref().map(|p| (p.bar.clone(), p.kind.clone(), p.key.clone())) else { return };
    let Some((mon, _, edge, _)) = bar_of(&bar) else { return hide_pop(sh) };
    let all = buttons_for(&taskbar::windows_now(), mon, false, spread());
    let app = all.as_array().into_iter().flatten().find(|a| a["app"].as_str() == Some(key.as_str())).cloned();
    let Some(app) = app.filter(|a| kind == "menu" || a["windows"].as_array().is_some_and(|w| !w.is_empty())) else { return hide_pop(sh) };
    let _ = sh.app.emit_to(
        POP,
        "barpop:show",
        // A menu with Windows' End task while its setting is on, read now.
        json!({ "kind": kind, "app": app, "edge": edge.word(), "look": taskbar::look_value(), "lang": sh.lang(), "endTask": kind == "menu" && taskbar::end_task_on() }),
    );
}

// Its page laid out (w, h: its px, with room for its shadow away from the
// strip; dpr, its scale): the window laid against the strip over the button,
// and shown.
#[tauri::command]
pub fn barpop_place(app: tauri::AppHandle, w: f64, h: f64, dpr: Option<f64>) {
    let sh = crate::shared(&app);
    let Some((bar, at, sf)) = OPEN.lock().unwrap().as_ref().map(|p| (p.bar.clone(), p.at, p.bar_sf)) else { return };
    let Some((mon, mon_sf, edge, strip)) = bar_of(&bar) else { return hide_pop(&sh) };
    let px = |v: f64| (v * sf).round() as i32;
    let pop_px = |v: f64| (v * page_scale(dpr, mon_sf)).round() as i32;
    let button = (strip.0 + px(at.0), strip.1 + px(at.1), px(at.2), px(at.3));
    let rect = pop_rect(edge, strip, mon, button, (pop_px(w), pop_px(h)));
    let Some(win) = sh.app.get_webview_window(POP) else { return };
    lay(&win, rect);
    let _ = win.show();
    if let Some(p) = OPEN.lock().unwrap().as_mut() {
        p.rect = Some(rect);
    }
    // Where it lies, told the strip's page in its px: the pointer on its way
    // up to it across another button keeps it (bar.js aimPop, #56).
    let (left, top) = ((rect.0 - strip.0) as f64 / sf, (rect.1 - strip.1) as f64 / sf);
    let _ = sh.app.emit_to(bar.as_str(), "bar:pop-at", json!({ "left": left, "top": top, "right": left + rect.2 as f64 / sf, "bottom": top + rect.3 as f64 / sf }));
    taskbar::watch_presses(taskbar::WATCH_BARS, true);
}

// Where the list goes (physical): against the strip on the screen's side,
// centred on the button along it, within its display.
fn pop_rect(edge: Edge, strip: Mon, mon: Mon, button: Mon, (w, h): (i32, i32)) -> Mon {
    let along_x = (button.0 + button.2 / 2 - w / 2).clamp(mon.0, (mon.0 + mon.2 - w).max(mon.0));
    let along_y = (button.1 + button.3 / 2 - h / 2).clamp(mon.1, (mon.1 + mon.3 - h).max(mon.1));
    match edge {
        Edge::Bottom => (along_x, strip.1 - h, w, h),
        Edge::Top => (along_x, strip.1 + strip.3, w, h),
        Edge::Left => (strip.0 + strip.2, along_y, w, h),
        Edge::Right => (strip.0 - w, along_y, w, h),
    }
}

// Its live pictures where its cards left room for them (window, x, y, w, h
// in its px; dpr, its scale); none, taken away.
#[tauri::command]
pub fn barpop_thumbs(items: Vec<(i64, f64, f64, f64, f64)>, dpr: Option<f64>) {
    let Some((bar, rect)) = OPEN.lock().unwrap().as_ref().and_then(|p| p.rect.map(|r| (p.bar.clone(), r))) else {
        return taskbar::thumbs(taskbar::WATCH_BARS, Vec::new());
    };
    let Some((_, sf, _, _)) = bar_of(&bar) else { return };
    let sf = page_scale(dpr, sf);
    let px = |v: f64| (v * sf).round() as i32;
    let items = items.into_iter().map(|(h, x, y, w, ht)| (h as isize, (rect.0 + px(x), rect.1 + px(y), px(w), px(ht)))).collect();
    taskbar::thumbs(taskbar::WATCH_BARS, items);
}

// Its page done with it (the pointer gone, a card or an item pressed).
#[tauri::command]
pub fn barpop_hide(app: tauri::AppHandle) {
    hide_pop(&crate::shared(&app));
}

fn hide_pop(sh: &Shared) {
    let Some(was) = OPEN.lock().unwrap().take() else { return };
    taskbar::thumbs(taskbar::WATCH_BARS, Vec::new());
    taskbar::watch_presses(taskbar::WATCH_BARS, false);
    if let Some(win) = sh.app.get_webview_window(POP) {
        let _ = win.hide();
    }
    let _ = sh.app.emit_to(POP, "barpop:gone", json!({}));
    let _ = sh.app.emit_to(was.bar.as_str(), "bar:pop-gone", json!({}));
}

// A press anywhere (taskbar.rs press hook): off the list and off its strip,
// it goes, as Windows' own; on the strip, its page has the press.
pub fn pressed(sh: &Shared, (x, y): (i32, i32)) {
    let inside = |r: Mon| x >= r.0 && y >= r.1 && x < r.0 + r.2 && y < r.1 + r.3;
    let away = {
        let open = OPEN.lock().unwrap();
        let Some(p) = open.as_ref() else { return };
        let strip = bar_of(&p.bar).map(|b| b.3);
        !p.rect.is_some_and(inside) && !strip.is_some_and(inside)
    };
    if away {
        hide_pop(sh);
    }
}

// Windows' setting for taskbars on several displays (Settings > Taskbar >
// Show my taskbar apps on): 0 all taskbars, 1 the main one and where the
// window is open, 2 only where it is open. 0 when not set.
fn spread() -> u32 {
    #[cfg(windows)]
    {
        use winreg::enums::HKEY_CURRENT_USER;
        winreg::RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced")
            .and_then(|k| k.get_value::<u32, _>("MMTaskbarMode"))
            .unwrap_or(0)
    }
    #[cfg(not(windows))]
    {
        0
    }
}

// The buttons a taskbar shows of all (taskbar.rs, each window with the
// display it is on, mon): on the main display all of them, unless only
// where open (2); on another display all of them (0), or only those of
// its own windows (1, 2), the pinned ones then on the main one alone.
fn buttons_for(all: &Value, mon: Mon, main: bool, spread: u32) -> Value {
    let on_it = |w: &Value| w["mon"].as_array().is_some_and(|m| m.iter().filter_map(Value::as_i64).map(|v| v as i32).eq([mon.0, mon.1, mon.2, mon.3]));
    let keep_all = spread == 0 || (main && spread == 1);
    let apps = all.as_array().into_iter().flatten().filter_map(|app| {
        let windows: Vec<Value> = app["windows"].as_array().into_iter().flatten().filter(|w| keep_all || on_it(w)).cloned().collect();
        let pinned = app["pinned"].as_bool() == Some(true) && (spread == 0 || main);
        (pinned || !windows.is_empty()).then(|| {
            let mut app = app.clone();
            app["windows"] = Value::Array(windows);
            app
        })
    });
    Value::Array(apps.collect())
}

// The buttons on her own taskbar, while hers are on the others too.
pub fn for_main(sh: &Shared, all: &Value) -> Value {
    if !wanted(sh) || BARS.lock().unwrap().is_empty() {
        return all.clone();
    }
    match island::display(sh) {
        Some(mon) => buttons_for(all, mon, true, spread()),
        None => all.clone(),
    }
}

// A window maximized or full screen on this display, not minimized
// (taskbar.rs fills_display), whichever taskbar has its button: a clear
// strip here fills in then, as Mica or solid (taskbarClearWhen), as hers
// does on hers (island.js isFilled).
fn fills(all: &Value, mon: Mon) -> bool {
    all.as_array().into_iter().flatten().flat_map(|a| a["windows"].as_array().into_iter().flatten()).any(|w| {
        w["fills"].as_bool() == Some(true)
            && w["min"].as_bool() != Some(true)
            && w["mon"].as_array().is_some_and(|m| m.iter().filter_map(Value::as_i64).map(|v| v as i32).eq([mon.0, mon.1, mon.2, mon.3]))
    })
}

// Each page told what to show, if it changed: the windows on its display,
// the look of hers (edge, its buttons' alignment, the material and when a
// clear one fills in, light or dark and the accent, her island's look), her
// language; the prompts waiting and her sheet, for her capsule.
pub fn refresh(sh: &Shared) {
    let bars: Vec<(String, Mon, f64, Value, Value, bool)> = BARS.lock().unwrap().iter().filter(|b| b.ready).map(|b| (b.label.clone(), b.mon, b.sf, b.sent.clone(), b.paper.clone(), b.dock)).collect();
    if bars.is_empty() {
        return;
    }
    let all = taskbar::windows_now();
    let look = taskbar::look_value();
    let spread = spread();
    let edge = island::bars_edge(sh);
    let asks = waiting(&sh.asks.lock().unwrap().views());
    let pet = sh.setting("pet");
    let sprite = crate::data::pets(&sh.dir).into_iter().find(|p| Some(p.id.as_str()) == pet.as_str());
    // The dock's: the buttons of her strip, in its order (as Win+digit counts
    // them, winkeys.rs ordered); whether a window of them flashes.
    let order: Vec<String> = sh.setting("taskbarOrder").as_array().map(|o| o.iter().filter_map(|k| k.as_str().map(str::to_string)).collect()).unwrap_or_default();
    let docked = Value::Array(crate::winkeys::ordered(&for_main(sh, &all), &order));
    let flashing = docked.as_array().into_iter().flatten().flat_map(|a| a["windows"].as_array().into_iter().flatten()).any(|w| w["flash"] == true);
    for b in BARS.lock().unwrap().iter_mut().filter(|b| b.dock) {
        b.flashing = flashing;
    }
    // Windows' acrylic on the dock, tinted for Windows' mode: not solid
    // (taskbarMaterial black), and Windows has it (transparency effects on:
    // backplate.rs available); else its page draws the desktop's picture.
    let light = look["mode"].as_str() == Some("light");
    let acrylic = (sh.setting("taskbarMaterial").as_str() != Some("black") && crate::backplate::available()).then_some(!light);
    let docks: Vec<(String, isize)> = BARS
        .lock()
        .unwrap()
        .iter_mut()
        .filter(|b| b.dock && b.hwnd != 0 && b.acrylic != acrylic)
        .map(|b| {
            b.acrylic = acrylic;
            (b.label.clone(), b.hwnd)
        })
        .collect();
    for (label, hwnd) in docks {
        // On the window's own thread, where Windows' calls on it belong.
        let _ = sh.app.run_on_main_thread(move || {
            crate::backplate::acrylic(hwnd, acrylic);
        });
        let how = match acrylic {
            Some(true) => "on, dark",
            Some(false) => "on, light",
            None => "off",
        };
        sh.log(&format!("bars: {label}'s acrylic {how}"));
    }
    let bins: std::collections::HashMap<String, Value> = BARS.lock().unwrap().iter().filter(|b| b.dock).map(|b| (b.label.clone(), b.bin.clone())).collect();
    for (label, mon, sf, sent, paper, dock) in bars {
        let state = json!({
            "dock": dock,
            // Windows' acrylic on it: its glass a tint over it, no Mica.
            "acrylic": dock && acrylic.is_some(),
            // Its Recycle Bin, as last looked at (look_at_bin).
            "bin": if dock { bins.get(&label).cloned().unwrap_or(Value::Null) } else { Value::Null },
            // Split: quick settings and the clock in the dock, or in the top bar (splitKeys).
            "keys": dock && sh.setting("splitKeys").as_str() == Some("dock"),
            "keysNow": if dock { taskbar::keys_now() } else { Value::Null },
            "edge": edge.word(),
            "align": sh.setting("taskbarAlign"),
            "material": sh.setting("taskbarMaterial"),
            "clearWhen": sh.setting("taskbarClearWhen"),
            "filled": fills(&all, mon),
            "look": look,
            "islandLook": sh.setting("islandLook"),
            "lang": sh.lang(),
            "sf": sf,
            "windows": if dock { docked.clone() } else { buttons_for(&all, mon, false, spread) },
            "asks": asks,
            "sprite": sprite.as_ref().map(|p| p.url()),
            "spriteVersion": sprite.as_ref().map_or(2, |p| p.version),
            "paper": paper,
        });
        if state != sent {
            if let Some(b) = BARS.lock().unwrap().iter_mut().find(|b| b.label == label) {
                b.sent = state.clone();
            }
            let _ = sh.app.emit_to(label.as_str(), "bar:state", state);
        }
    }
    // A list open: its program's windows as they are now.
    if OPEN.lock().unwrap().is_some() {
        send_pop(sh);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_by_windows_own_setting() {
        let main = (0, 0, 2560, 1440);
        let other = (2560, -150, 1829, 1143);
        let all = json!([
            { "app": "chrome", "pinned": true, "windows": [{ "id": 1, "mon": [0, 0, 2560, 1440] }, { "id": 2, "mon": [2560, -150, 1829, 1143] }] },
            { "app": "notes", "pinned": true, "windows": [] },
            { "app": "code", "pinned": false, "windows": [{ "id": 3, "mon": [2560, -150, 1829, 1143] }] },
        ]);
        let ids = |v: &Value| -> Vec<(String, Vec<i64>)> {
            v.as_array().unwrap().iter().map(|a| (a["app"].as_str().unwrap().to_string(), a["windows"].as_array().unwrap().iter().map(|w| w["id"].as_i64().unwrap()).collect())).collect()
        };
        // All taskbars (0): every one everywhere.
        assert_eq!(ids(&buttons_for(&all, other, false, 0)).len(), 3);
        // The main one and where open (1): the main all; the other its own, no pinned ones without a window there.
        assert_eq!(ids(&buttons_for(&all, main, true, 1)), ids(&all));
        assert_eq!(ids(&buttons_for(&all, other, false, 1)), vec![("chrome".to_string(), vec![2]), ("code".to_string(), vec![3])]);
        // Only where open (2): the main its own and the pinned; the other its own.
        assert_eq!(ids(&buttons_for(&all, main, true, 2)), vec![("chrome".to_string(), vec![1]), ("notes".to_string(), vec![])]);
        assert_eq!(ids(&buttons_for(&all, other, false, 2)), vec![("chrome".to_string(), vec![2]), ("code".to_string(), vec![3])]);
    }

    #[test]
    fn a_strip_fills_in_for_a_window_over_its_own_display() {
        let main = (0, 0, 2560, 1440);
        let other = (2560, -149, 2560, 1600);
        let all = |windows: Value| json!([{ "app": "chrome", "pinned": true, "windows": windows }]);
        // Maximized on the other display: that one fills in, the main one not.
        let maxed = all(json!([{ "id": 1, "fills": true, "min": false, "mon": [2560, -149, 2560, 1600] }]));
        assert!(fills(&maxed, other));
        assert!(!fills(&maxed, main));
        // Minimized, or only restored there: neither.
        assert!(!fills(&all(json!([{ "id": 1, "fills": true, "min": true, "mon": [2560, -149, 2560, 1600] }])), other));
        assert!(!fills(&all(json!([{ "id": 1, "fills": false, "min": false, "mon": [2560, -149, 2560, 1600] }])), other));
        // No windows at all (her taskbar not up yet).
        assert!(!fills(&Value::Null, other));
    }

    #[test]
    fn the_list_against_the_strip_over_its_button() {
        // A display right of the main one, 150% (its strip 72 px thick).
        let mon = (2560, 0, 1920, 1080);
        let bottom = (2560, 1008, 1920, 72);
        let button = (3400, 1014, 66, 60);
        // Along the bottom: on the strip, centred on the button.
        assert_eq!(pop_rect(Edge::Bottom, bottom, mon, button, (400, 300)), (3233, 708, 400, 300));
        // Near the display's ends: kept on it.
        assert_eq!(pop_rect(Edge::Bottom, bottom, mon, (2570, 1014, 66, 60), (400, 300)), (2560, 708, 400, 300));
        assert_eq!(pop_rect(Edge::Bottom, bottom, mon, (4400, 1014, 66, 60), (400, 300)), (4080, 708, 400, 300));
        // Along the top: under it.
        assert_eq!(pop_rect(Edge::Top, (2560, 0, 1920, 72), mon, (3400, 6, 66, 60), (400, 300)), (3233, 72, 400, 300));
        // Along a side: beside it, centred on the button up and down.
        assert_eq!(pop_rect(Edge::Left, (2560, 0, 72, 1080), mon, (2566, 500, 60, 66), (300, 200)), (2632, 433, 300, 200));
        assert_eq!(pop_rect(Edge::Right, (4408, 0, 72, 1080), mon, (4414, 500, 60, 66), (300, 200)), (4108, 433, 300, 200));
    }

    #[test]
    fn the_dock_in_the_middle_of_the_foot() {
        // 1920 x 1080 at 125%: its glass 55 px high, 3 off the foot (2 at
        // 125%, rounded), as wide as asked.
        let mon = (0, 0, 1920, 1080);
        assert_eq!(dock_rect(mon, 1.25, 500.0), (710, 1022, 500, 55));
        // Never narrower than high, never wider than the display.
        assert_eq!(dock_rect(mon, 1.25, 10.0), (932, 1022, 55, 55));
        assert_eq!(dock_rect(mon, 1.25, 5000.0), (0, 1022, 1920, 55));
        // A display left of the main one.
        assert_eq!(dock_rect((-1920, 0, 1920, 1080), 1.0, 400.0), (-1160, 1034, 400, 44));
        // Out of the way: down but for its top 3 px (4 at 125%).
        assert_eq!(dock_top((710, 1022, 500, 55), mon, 1.25, true), 1076);
        assert_eq!(dock_top((710, 1022, 500, 55), mon, 1.25, false), 1022);
    }

    #[test]
    fn the_dock_steps_aside_and_is_called_back() {
        let mon = (0, 0, 1920, 1080);
        let rect = (710, 1020, 500, 60);
        let t = Instant::now();
        let ms = |n: u64| t + Duration::from_millis(n);
        let mut s = Stow::default();
        // No window touching it: up, wherever the pointer is.
        assert!(!stow_look(&mut s, rect, mon, 1.25, (100, 100), false, t, || false));
        // One touching it, the pointer elsewhere: down.
        s.under = true;
        assert!(stow_look(&mut s, rect, mon, 1.25, (100, 100), false, t, || false));
        s.stowed = true;
        // Kept up (its list open, Start, a window flashing): up.
        assert!(!stow_look(&mut s, rect, mon, 1.25, (100, 100), true, t, || false));
        // At the foot, anywhere along it: up once it has rested there 150 ms.
        assert!(stow_look(&mut s, rect, mon, 1.25, (300, 1079), false, t, || false));
        assert!(stow_look(&mut s, rect, mon, 1.25, (300, 1079), false, ms(100), || false));
        assert!(!stow_look(&mut s, rect, mon, 1.25, (300, 1079), false, ms(160), || false));
        s.stowed = false;
        // Still at the foot, away from it along there: up, however long.
        assert!(!stow_look(&mut s, rect, mon, 1.25, (300, 1079), false, ms(2000), || false));
        // The gap under its glass is its own.
        assert!(!stow_look(&mut s, rect, mon, 1.25, (900, 1078), false, ms(2100), || false));
        // Then on it, or near it: up; off it, down 600 ms after.
        assert!(!stow_look(&mut s, rect, mon, 1.25, (900, 1040), false, ms(2200), || false));
        assert!(!stow_look(&mut s, rect, mon, 1.25, (900, 1010), false, ms(2300), || false));
        assert!(!stow_look(&mut s, rect, mon, 1.25, (900, 500), false, ms(2400), || false));
        assert!(stow_look(&mut s, rect, mon, 1.25, (900, 500), false, ms(3050), || false));
        s.stowed = true;
        // A button held at the foot (a window dragged down): not called up.
        assert!(stow_look(&mut s, rect, mon, 1.25, (300, 1079), false, ms(4000), || true));
        assert!(stow_look(&mut s, rect, mon, 1.25, (300, 1079), false, ms(5000), || false));
        // Off the foot, the press forgotten: called up again by resting there.
        assert!(stow_look(&mut s, rect, mon, 1.25, (300, 900), false, ms(5100), || false));
        assert!(stow_look(&mut s, rect, mon, 1.25, (300, 1079), false, ms(5200), || false));
        assert!(!stow_look(&mut s, rect, mon, 1.25, (300, 1079), false, ms(5400), || false));
    }

    #[test]
    fn how_many_wait_and_for_what() {
        assert_eq!(waiting(&json!([])), Value::Null);
        assert_eq!(waiting(&json!([{ "kind": "permission" }, { "kind": "question" }])), json!({ "n": 2, "kind": "approve" }));
        assert_eq!(waiting(&json!([{ "kind": "plan" }])), json!({ "n": 1, "kind": "approve" }));
        assert_eq!(waiting(&json!([{ "kind": "question" }, { "kind": "permission" }])), json!({ "n": 2, "kind": "answer" }));
    }
}
