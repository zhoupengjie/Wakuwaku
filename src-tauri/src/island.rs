// Her home's window (the page draws it, src/pet/island.js), by display:
//   island  a black pill at the top centre, or at a top corner (islandPos,
//           zhoupengjie/Wakuwaku#59, see fit); or along the foot of the
//           screen, laid over Windows' own taskbar, which stays as it is
//           (islandEdge taskbar, zhoupengjie/Wakuwaku#105), or, there, its
//           capsule inside the taskbar itself (islandEmbed, embed.rs), this
//           window only for what opens from it
//   taskbar a strip along the bottom in place of Windows' own, which is put
//           away while it is up, its tray taken over (taskbar.rs); what
//           opens grows up from it, into room kept above it (appbar.rs)
// Both stay up while she is out, and do the talking.
//
// Pulled out of her home, she becomes the pet window (carried under the
// cursor while the button is still held on the home); brought close to it
// again, it reaches for her and takes her back.
//
// Sizes here are logical (the page's) unless they say physical.
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use tauri::utils::config::BackgroundThrottlingPolicy;
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

use crate::pointer::{ClickThrough, Mode};
use crate::screen::Area;
use crate::appbar::Edge;
use crate::{appbar, taskbar, Shared};

// The window without extra room, by home (island.js BASE): the island as
// wide as pulling her out needs, so it only ever grows down. A window whose
// top-left corner moves shows its old picture from there for a frame or two,
// so these never move it. The page asks for more (pet:panel) when a shape
// needs it.
const ISLAND: (f64, f64) = (760.0, 132.0);
// Along the foot of the screen (islandEdge taskbar, zhoupengjie/Wakuwaku#105):
// the island at the window's foot, what opens from it growing up into the
// room above it, as the taskbar's does. So the window is as tall as the
// taskbar's strip and room together and never grows (growing up would move
// its top-left corner); the page keeps what opens within it (island.js
// openMaxH, settingsMax). Only its width may grow, as at the top.
const ISLAND_BOTTOM: (f64, f64) = (ISLAND.0, TASKBAR_H + TASKBAR_ROOM);
// The taskbar's strip (as high as Windows' own, so the Start menu and the
// quick settings open just above it), and the room above it for what opens
// (a window growing up moves its top-left corner). The window is the two.
pub const TASKBAR_H: f64 = 48.0;
// Split in two (taskbarEdge split): the top bar thinner by default, as a
// menu bar (splitBar 32; 48 as Windows'), docs/design/specs/dock-topbar.md.
pub const SLIM_H: f64 = 32.0;
// As high as the settings ever are (they stand above the strip, island.js
// settingsMax): opening them never moves the window, which would flash.
const TASKBAR_ROOM: f64 = 640.0;
// Beside a side bar (taskbarEdge left or right): as wide as the settings
// ever are (the Mail page in two panes, settings.js WIDE_W), their gap off
// the strip and their shadow, for the same reason.
const SIDE_ROOM: f64 = 960.0 + 8.0 + 12.0;
// The middle of her portrait at the taskbar's start (its left end, or a
// side bar's top), along the strip.
const TASKBAR_SEAT: f64 = 24.0;
// The island's top edge in the window, and its compact height.
const TOP: f64 = 8.0;
const COMPACT_H: f64 = 36.0;
// Half the closed capsule at its usual width: her seat at a top corner
// before the page has said its size.
const CAPSULE_HALF: f64 = 150.0;
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
    // WebView CSS px to physical cursor px. This can differ from the
    // monitor's scale factor when Windows text scaling changes WebView2's
    // devicePixelRatio without changing the native window's DPI.
    page_sf: f64,
    // The page's zoom, as set last (fit_zoom); 0: never set, 1.
    zoom: f64,
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
    // home, the edge, whether Windows' own keeps its room along the foot):
    // asked again only on a change.
    bar: Option<(i32, i32, i32, i32)>,
    bar_for: Option<((i32, i32, i32, i32), i32, &'static str, Edge, bool)>,
    // The strip is a bar of ours Windows knows (appbar.rs), not one laid over
    // the room Windows' own taskbar keeps (taskbar.rs explorer_room).
    bar_own: bool,
    // The button held on her (a pull, or carrying her out): the window keeps the pointer.
    holding: bool,
    reaching: bool,
    // Taking her in: the window stays up for it.
    pub absorbing: bool,
    // Risen only for the settings (she is out of sight): it goes again once
    // they close.
    pub temp: bool,
    pub settings_open: bool,
    // Risen for the prompts waiting (asked from her taskbar on another
    // display while an app full screen hid it): it goes once they are answered.
    pub for_asks: bool,
    // The page told it is up over an app full screen for them alone (over_full).
    over_full: bool,
    // Settings asked for before its page was up: the page they open on.
    pending: Option<String>,
    // On the taskbar, her seat along the strip from its start (logical: from
    // its left, or a side bar's top), where the page has laid her island out
    // (taskbarLayout: it may be anywhere along the strip); none told yet: at
    // its start.
    seat_at: Option<f64>,
    // The strip, its edge and the window as the page was last told of them
    // (tell_strip).
    strip_told: Option<((i32, i32, i32, i32), Edge, (i32, i32), (i32, i32), f64)>,
    // At the top of the screen, up out of the way of a window under it.
    stow: Stow,
    // The taskbar's layout being edited (island.js editTaskbar): her menu
    // offers to stop. And where the window takes the mouse whatever the
    // page says of the pointer (solid, page px: x, y, w, h; pointer.rs
    // look): the edit bar (zhoupengjie/Wakuwaku#53), the widget picker, its
    // popover, her capsule's panel and the toasts (set_editing).
    pub editing: bool,
    solid: Vec<[f64; 4]>,
    // At the top of the screen, the side its window is at (Side; islandPos,
    // zhoupengjie/Wakuwaku#59), as fit put it there: the page is told it
    // (pet:update islandAt) and lays the island out at that side.
    at: Side,
    // Along the foot of the screen instead (islandEdge taskbar,
    // zhoupengjie/Wakuwaku#105), as fit put it there; and the thickness
    // (physical) of Windows' own taskbar shown along the foot under it, the
    // island laid over it, centred in it (foot). The page is told both
    // (pet:update islandEdge, islandStrip).
    bottom: bool,
    strip: Option<i32>,
    // Her island inside Windows' taskbar (embed.rs; islandEmbed): where its
    // closed capsule lies there (physical; none: no room for it), the room
    // it has (physical px; 0 none: the page keeps its width within it,
    // anchor.js capsuleWidth), and whether it is in. The window is laid
    // about the capsule, its middle mx (physical px from the window's left;
    // pet:update islandMx) where the page lays the island, and is a child of
    // the taskbar, clipped to the strip, while nothing is open in it and
    // nobody is on it (parenting; the page says what is open: island_opened,
    // opened); out, whole over the taskbar, as it is used.
    cap: Option<(i32, i32, i32, i32)>,
    cap_room: i32,
    embedded: bool,
    mx: Option<i32>,
    opened: bool,
}

// Where the island's window sits at the top of the screen.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Side {
    Left,
    #[default]
    Center,
    Right,
}

impl Side {
    // The setting (islandPos); anything else the centre.
    fn of(pos: &str) -> Side {
        match pos {
            "left" => Side::Left,
            "right" => Side::Right,
            _ => Side::Center,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Side::Left => "left",
            Side::Center => "center",
            Side::Right => "right",
        }
    }

    // The window's place (physical) at this side of a work area, at its top:
    // at a corner flush with it (the page lays the island TOP in from that
    // side, as from the top); grown wider, its side kept there (the centre
    // keeps its middle).
    fn place(self, area: &Area, width: i32) -> (i32, i32) {
        match self {
            Side::Left => (area.x, area.y),
            Side::Right => (area.x + area.w - width, area.y),
            Side::Center => (area.x + (area.w - width) / 2, area.y),
        }
    }

    // The window's place (physical) at this side of a work area with its foot
    // on foot (zhoupengjie/Wakuwaku#105): the screen's bottom edge, over
    // Windows' own taskbar shown there, or the work area's foot (fit).
    fn place_above(self, area: &Area, size: (i32, i32), foot: i32) -> (i32, i32) {
        (self.place(area, size.0).0, foot - size.1)
    }
}

// The window's left (physical) about her capsule inside Windows' taskbar
// (embed.rs): its middle on the capsule's, kept on its display (a capsule
// near the strip's end would have it off the screen); and where the
// capsule's middle lies in it then (mx, physical px from its left), for the
// page to lay the island about (island.js placeMid).
fn about(area: &Area, cap: (i32, i32, i32, i32), width: i32) -> (i32, i32) {
    let mid = cap.0 + cap.2 / 2;
    let x = (mid - width / 2).clamp(area.x, (area.x + area.w - width).max(area.x));
    (x, mid - x)
}

// Her island at the top of the screen, up out of the way of the window
// under it (look_under, call_look; island.js setStow).
#[derive(Default)]
struct Stow {
    // The closed island's size as the page has it (logical), and whether
    // the page has it up out of the way now.
    closed: (f64, f64),
    stowed: bool,
    // A window under the closed island, and the pointer having called it
    // down: as the page was told (island:under, island:called).
    under: bool,
    called: bool,
    // Since when the pointer has rested at the edge over it, and whether a
    // button was held there meanwhile; since when, having called it, the
    // pointer has been away from it.
    edge_since: Option<std::time::Instant>,
    pressed: bool,
    away_since: Option<std::time::Instant>,
    // The pointer near the window at the last look.
    near: bool,
}

impl Island {
    fn sf(&self) -> f64 {
        if self.sf > 0.0 { self.sf } else { 1.0 }
    }

    fn page_sf(&self) -> f64 {
        valid_scale(Some(self.page_sf)).unwrap_or_else(|| self.sf())
    }

    // Over Windows' taskbar (#105), how far in from the window's edge along
    // it (its foot along the bottom, its top along the top) the closed
    // island lies (the page's px; island.js placeAnchor, anchor.js foot):
    // centred in the taskbar's strip (6 px in Windows 11's 48), or TOP with
    // none there, as down from the top of the screen.
    fn foot(&self) -> f64 {
        match self.strip {
            Some(thick) => ((thick as f64 / self.page_sf()) - COMPACT_H).max(0.0) / 2.0,
            None => TOP,
        }
    }

    // The strip as Windows granted it (bar), if it was asked for this edge
    // of this display (bar_for); none granted for another edge or display.
    // The edge changed, the strip is asked anew (sync_bar_now), which first
    // waits on the desktop's icons (up to 1.5 s); another thread laying the
    // window meanwhile (the page asking for room, the look) went by the
    // old edge's strip: set to the right, the window was laid as a side
    // bar's, 1285 x 60, at the bottom strip's right end, and the page told
    // of a strip 2560 px thick (2026-11-07). The edge's own strip stands in
    // till the new one is granted (fit, size_for, seat).
    fn strip_now(&self, edge: Edge, mon: (i32, i32, i32, i32)) -> Option<(i32, i32, i32, i32)> {
        self.bar.filter(|_| self.bar_for.is_some_and(|f| f.0 == mon && f.3 == edge))
    }

    // Physical. The taskbar is as long as its strip (its display, or what
    // Windows granted of it), with its room beside it: above a bottom bar,
    // below a top one, inside a side one's. The island along the foot of the
    // screen (bottom) has its room above it, the window's height.
    // The strip strip_h thick (logical).
    fn size_for(&self, home: &str, area: &Area, edge: Edge, strip_h: f64, bottom: bool) -> (i32, i32) {
        let sf = self.sf();
        let side = home == "taskbar" && edge.is_side();
        let base = match home {
            "taskbar" if side => (strip_h + SIDE_ROOM, self.strip_now(edge, area.mon).map_or(area.mon.3, |b| b.3) as f64 / sf),
            "taskbar" => (area.mon.2 as f64 / sf, strip_h + TASKBAR_ROOM),
            _ if bottom => ISLAND_BOTTOM,
            _ => ISLAND,
        };
        let (w, h) = match self.room {
            Some((w, h)) => (w.max(base.0), h.max(base.1)),
            None => base,
        };
        // The strip's length is the window's; only its room grows. Along the
        // foot of the screen the height is the room, fixed: only the width grows.
        let (w, h) = match home {
            "taskbar" if side => (w, base.1),
            "taskbar" => (base.0, h),
            _ if bottom => (w, base.1),
            _ => (w, h),
        };
        ((w * sf).round() as i32, (h * sf).round() as i32)
    }
}

fn window(sh: &Shared) -> Option<WebviewWindow> {
    // None once the window is no more: gone with Explorer's taskbar (its
    // child, embed.rs, which starts her anew). Tauri still has it, and
    // tao's calls about its place and size (outer_size, outer_position,
    // set_size) panic on a window that is no more ("Unexpected
    // GetWindowRect failure", "adjust_window_rect failed"), and the
    // release build aborts on a panic: she died with Explorer in the test
    // VM (2026-11-07), and her quitting process left a panic record after
    // the first fix (lay alone looked).
    sh.app.get_webview_window("island").filter(alive)
}

// Whether the window still exists (see window).
fn alive(win: &WebviewWindow) -> bool {
    #[cfg(windows)]
    {
        crate::embed::is_window(win.hwnd().map(|h| h.0 as isize).unwrap_or(0))
    }
    #[cfg(not(windows))]
    {
        let _ = win;
        true
    }
}

// Up whenever she is in sight, home or out; or risen for the settings; or
// the settings open in it, whatever they just hid (she, do not disturb):
// they never go from under the person, only once closed.
// The taskbar is up whatever hides her (out of sight, do not disturb): it is
// the person's taskbar; only an app full screen hides it, as Windows' own.
pub fn is_shown(sh: &Shared) -> bool {
    // Nor while another copy of her holds the taskbar (taskbar.rs waiting).
    let up = if sh.home() == "taskbar" { !sh.is_fullscreen() && !taskbar::waiting() } else { sh.is_visible() };
    let isl = sh.island.lock().unwrap();
    // Inside Windows' taskbar (embed.rs) with no room for the capsule: hidden
    // until there is, but for the settings and the prompts, which rise.
    if up && isl.embedded && isl.cap.is_none() {
        return isl.temp || isl.settings_open || isl.for_asks;
    }
    up || isl.temp || isl.settings_open || isl.for_asks
}

// Its window's handle, once made (0 before).
pub fn hwnd(sh: &Shared) -> isize {
    sh.island.lock().unwrap().hwnd
}

// Its handle and whether its window is shown, read together, and f run
// before either can change: the plate's wish (backplate.rs set) goes in so,
// and the window hidden meanwhile (show_or_hide) never leaves a plate
// standing over the screen on its own (zhoupengjie/Wakuwaku#112).
pub fn with_shown<R>(sh: &Shared, f: impl FnOnce(isize, bool) -> R) -> R {
    let isl = sh.island.lock().unwrap();
    f(isl.hwnd, isl.shown)
}

// Where its window is (physical) and its scale: the page's px to the screen's.
// The page's, as it told it (page_sf): Windows' text size makes it bigger
// than its display's (110% at 150%: 1.65), the windows' live pictures
// smaller than their cards' rooms and up to the left of them else, the tray's
// icons told where they are not (zhoupengjie/Wakuwaku#90).
pub fn origin(sh: &Shared) -> ((i32, i32), f64) {
    let isl = sh.island.lock().unwrap();
    (isl.pos, isl.page_sf())
}

// Where its window is and its size (physical), and its scale (the page's, as origin).
pub fn frame(sh: &Shared) -> ((i32, i32), (i32, i32), f64) {
    let isl = sh.island.lock().unwrap();
    (isl.pos, isl.size, isl.page_sf())
}

fn set_ignore(sh: &Shared, ignore: Option<bool>) {
    if let (Some(ignore), Some(win)) = (ignore, window(sh)) {
        sh.log(&format!("island: mouse {}", if ignore { "passes through" } else { "caught" }));
        let _ = win.set_ignore_cursor_events(ignore);
    }
}

// The screen's edge her taskbar is along (taskbarEdge); split, the top: her
// strip is the top bar, Start and the programs in the dock (bars.rs).
pub fn edge(sh: &Shared) -> Edge {
    if is_split(sh) {
        return Edge::Top;
    }
    Edge::of(sh.setting("taskbarEdge").as_str().unwrap_or("bottom"))
}

// The top bar and the dock (display split, a home beside the island and the
// taskbar, docs/design/specs/dock-topbar.md): her taskbar split in two, a
// top bar (the strip, here) and a dock at the foot with Start and the
// programs (bars.rs); a taskbar as her windows have it (Shared::home).
pub fn is_split(sh: &Shared) -> bool {
    sh.setting("display").as_str() == Some("split")
}

// The strip's thickness (logical): Windows' own, or split, the top bar's
// (splitBar: 32 unless set to 48).
pub fn strip_h(sh: &Shared) -> f64 {
    if is_split(sh) && sh.setting("splitBar").as_u64() != Some(48) { SLIM_H } else { TASKBAR_H }
}

// Split, the dock kept in the room Windows' own keeps along the foot
// (splitDock room), maximized windows stopping at it; else (hide, the
// default) it keeps none and steps out of the way of a window touching it.
pub fn dock_keeps_room(sh: &Shared) -> bool {
    is_split(sh) && sh.setting("splitDock").as_str() == Some("room")
}

// The edge of the strips on the other displays (bars.rs): hers, or split,
// the bottom, as they are now.
pub fn bars_edge(sh: &Shared) -> Edge {
    if is_split(sh) { Edge::Bottom } else { edge(sh) }
}

// The display chosen for her home (the setting homeDisplay, a display's key
// as screen.rs keeps it: zhoupengjie/Wakuwaku#104), while it is attached;
// none chosen (the default), or it unplugged, none.
fn chosen(sh: &Shared, screens: &crate::screen::Screens) -> Option<Area> {
    let key = sh.setting("homeDisplay");
    screens.by_key(key.as_str().filter(|k| !k.is_empty())?)
}

// Her display's scale: the chosen one's, else the primary's (taskbar.rs
// draws its icons at it). Not where the window is: asked on the taskbar's
// threads, which the window's own state is not read on.
pub fn scale(sh: &Shared) -> f64 {
    let screens = sh.screens();
    chosen(sh, &screens).or(screens.primary).map_or(1.0, |a| a.sf)
}

// The display it is on: the one chosen (#104), else where it is, else the primary one.
fn area(sh: &Shared) -> Option<Area> {
    let screens = sh.screens();
    if let Some(a) = chosen(sh, &screens) {
        return Some(a);
    }
    let at = {
        let isl = sh.island.lock().unwrap();
        (isl.created && isl.size.0 > 0).then(|| (isl.pos.0 + isl.size.0 / 2, isl.pos.1 + isl.size.1 / 2))
    };
    match at {
        Some((x, y)) => screens.near(x, y),
        None => screens.primary,
    }
}

// The display it is on (x, y, w, h), as area finds it.
pub fn display(sh: &Shared) -> Option<(i32, i32, i32, i32)> {
    area(sh).map(|a| a.mon)
}

// Where the window goes, by home. Returns the physical rect if it moved.
fn fit(sh: &Shared) -> Option<((i32, i32), (i32, i32))> {
    let home = sh.home();
    let edge = edge(sh);
    let area = area(sh)?;
    let wanted = Side::of(sh.setting("islandPos").as_str().unwrap_or("center"));
    // In Windows' own taskbar instead of at the top of the screen (islandEdge
    // taskbar, #105): along its edge, the bottom or the top, while Explorer's
    // taskbar is shown there; along the foot of the work area with none.
    let in_bar = home != "taskbar" && sh.setting("islandEdge").as_str() == Some("taskbar");
    let bar = in_bar.then(|| crate::shell::shown_bar(area.mon)).flatten().filter(|(_, _, e)| matches!(e, Edge::Top | Edge::Bottom));
    let low = in_bar && !matches!(bar, Some((_, _, Edge::Top)));
    let thick = strip_h(sh);
    let mut isl = sh.island.lock().unwrap();
    isl.sf = area.sf;
    let mut size = isl.size_for(home, &area, edge, thick, low);
    let pos = match home {
        // The window's edge on the strip's outer one, the room inside it.
        "taskbar" => window_at(edge, isl.strip_now(edge, area.mon).unwrap_or_else(|| edge.strip(area.mon, (thick * area.sf).round() as i32)), size),
        // On a Mac the notch's place, over the menu bar (notch.rs), the
        // centre at the top whatever the settings; elsewhere at the top of
        // the work area (below a taskbar that sits at the top), at the side
        // islandPos says (zhoupengjie/Wakuwaku#59): the centre, or a top
        // corner. The page is told the side (pet:update islandAt) and lays
        // the island out at that side of its window by it, never by the
        // setting itself; the closed island (closed_rect) and her seat
        // follow it too. In Windows' taskbar (islandEdge taskbar,
        // zhoupengjie/Wakuwaku#105), the same, told the edge as well
        // (islandEdge, islandStrip): over Windows' own taskbar shown along
        // the bottom or the top of her display, the work area stopping at
        // it (not one hidden, nor one slid away: shell.rs shown_bar), the
        // window's foot on the screen's (its top on the screen's top, for a
        // taskbar there) and the closed island centred in the strip (foot);
        // her capsule inside the taskbar (embed.rs), the window about it.
        // With no such taskbar (along a side, or hiding itself), on the work
        // area's foot, the island TOP up from it as from the top. Windows'
        // taskbar is left as it is, shown, and whatever Start11 and the like
        // make of it: the island home never puts it away.
        _ => match crate::notch::at_top(area.mon, size.0) {
            Some(pos) => {
                isl.at = Side::Center;
                isl.bottom = false;
                isl.strip = None;
                isl.mx = None;
                size = isl.size_for(home, &area, edge, thick, false);
                pos
            }
            None if in_bar => {
                let strip = bar.map(|(_, s, _)| s);
                isl.at = wanted;
                isl.bottom = low;
                isl.strip = strip.map(|s| s.3);
                // Along the bottom its foot on the strip's foot (the work
                // area's with no strip), along the top its top on the strip's.
                let foot = strip.map_or(area.y + area.h, |s| s.1 + s.3);
                let y = if low { foot - size.1 } else { strip.map_or(area.y, |s| s.1) };
                // Her capsule inside Windows' taskbar (embed.rs): the window
                // about it, the page told where its middle lies in it (mx).
                match isl.cap.filter(|_| isl.embedded) {
                    Some(cap) => {
                        let (x, mx) = about(&area, cap, size.0);
                        isl.mx = Some(mx);
                        (x, y)
                    }
                    None => {
                        isl.mx = None;
                        if low { wanted.place_above(&area, size, foot) } else { (wanted.place(&area, size.0).0, y) }
                    }
                }
            }
            None => {
                isl.at = wanted;
                isl.bottom = false;
                isl.strip = None;
                isl.mx = None;
                wanted.place(&area, size.0)
            }
        },
    };
    if (pos, size) == (isl.pos, isl.size) {
        return None;
    }
    isl.pos = pos;
    isl.size = size;
    Some((pos, size))
}

// The side its window is at the top of the screen (the centre on the
// taskbar), for the page (main.rs payload islandAt).
pub fn side(sh: &Shared) -> Side {
    sh.island.lock().unwrap().at
}

// The edge of the screen its window is along, for the page (payload
// islandEdge): the bottom (islandEdge taskbar, zhoupengjie/Wakuwaku#105,
// with Windows' taskbar along the bottom or none shown), or the top (as
// ever, or with Windows' taskbar along the top; on the taskbar home too).
pub fn edge_name(sh: &Shared) -> &'static str {
    if sh.island.lock().unwrap().bottom { "bottom" } else { "top" }
}

// Windows' own taskbar's thickness (physical) along the edge the island
// is along, for the page (payload islandStrip); 0 with none there.
pub fn strip_under(sh: &Shared) -> i32 {
    sh.island.lock().unwrap().strip.unwrap_or(0)
}

fn at_bottom(sh: &Shared) -> bool {
    sh.island.lock().unwrap().bottom
}

// --- Her capsule inside Windows' taskbar (embed.rs) ----------------------------------

// Wanted while her home is the island in Windows' taskbar (islandEdge
// taskbar) and the setting (islandEmbed, on unless turned off) says so:
// embed.rs then looks for Explorer's taskbar along the bottom or the top
// of her display every second.
pub fn embedding_wanted(sh: &Shared) -> bool {
    sh.home() != "taskbar" && sh.setting("islandEdge").as_str() == Some("taskbar") && sh.setting("islandEmbed").as_bool() != Some(false)
}

// Her island laid over Windows' taskbar (islandEdge taskbar, the embedding
// off): embed.rs makes the taskbar's window its owner, so Windows keeps it
// over the taskbar as Explorer raises the taskbar (uncover only finds it under one).
pub fn over_wanted(sh: &Shared) -> bool {
    sh.home() != "taskbar" && sh.setting("islandEdge").as_str() == Some("taskbar") && sh.setting("islandEmbed").as_bool() == Some(false)
}

// The display her home is on, for the capsule to go into its taskbar.
pub fn home_area(sh: &Shared) -> Option<Area> {
    area(sh)
}

// Where the island is set to sit (islandPos), for the stretch of the taskbar the capsule goes in.
pub fn side_wanted(sh: &Shared) -> Side {
    Side::of(sh.setting("islandPos").as_str().unwrap_or("center"))
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
fn used(isl: &Island) -> bool {
    isl.ct.over || isl.opened || isl.settings_open || isl.holding || isl.reaching || isl.absorbing || isl.for_asks || isl.temp
}

// For embed.rs: whether the island is in use now (used), its window out of
// Windows' taskbar for it: the capsule's place there is held meanwhile.
pub fn in_use(sh: &Shared) -> bool {
    used(&sh.island.lock().unwrap())
}

// For the pages (main.rs payload): whether her capsule is inside Windows'
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
// next started (main.rs browser_args).
pub fn home_display_changed(sh: &Shared) {
    place(sh);
    sh.log(&format!("island: her home on {:?} (homeDisplay {})", display(sh), sh.setting("homeDisplay")));
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
fn window_at(edge: Edge, b: (i32, i32, i32, i32), size: (i32, i32)) -> (i32, i32) {
    match edge {
        Edge::Bottom => (b.0, b.1 + b.3 - size.1),
        Edge::Top | Edge::Left => (b.0, b.1),
        Edge::Right => (b.0 + b.2 - size.0, b.1),
    }
}

// The strip's thickness across it (physical), as it lies: what the page
// draws it as (its CSS px times the scale), so that its inner edge is the
// work area's, at any scale (175% left a hairline gap: zhoupengjie/Wakuwaku#2).
fn thickness(edge: Edge, b: (i32, i32, i32, i32)) -> i32 {
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
    // With the window's size: the page may be a little bigger (island.js
    // fitPage, zhoupengjie/Wakuwaku#19).
    let _ = sh.app.emit_to("island", "taskbar:strip", json!({ "thick": thick, "win": [size.0, size.1], "sf": sf }));
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

// The taskbar's strip along its edge (taskbarEdge): asked of Windows while
// the home is the taskbar and keep (up; also while an app full screen hides
// its window: giving the strip back would move every window), given back
// otherwise. Asked again only when the display, the height, the home or the
// edge change. It comes with Windows' own put away first and goes with it
// given back (taskbar.rs).
fn sync_bar_now(sh: &Shared, keep: bool) {
    let home = sh.home();
    let edge = match home {
        "taskbar" => Some((edge(sh), strip_h(sh))),
        _ => None,
    };
    // Windows' own keeps its room along the foot for a strip there to lie
    // over, or for the dock kept in it (split); none else (taskbar.rs set_edge).
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
        // and none for another edge, but for the dock kept in it (taskbar.rs set_edge).
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
    // Asked of Explorer itself, not by the taskbar's window of ours (taskbar.rs claim).
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
// (taskbar.rs keep_work): its place told again (ABM_QUERYPOS, ABM_SETPOS,
// to Explorer itself: taskbar.rs claim), as an app bar is to do when
// Explorer says the bars moved (ABN_POSCHANGED, which her home's window, a
// webview's, does not hear). Explorer had the bar registered but left it
// out of every work area it worked out: its place, asked by the taskbar's
// window of ours, never reached it (systray.rs explorer_first), and its own
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

// The window laid on its rect (physical), in steps (lay_steps): sized down
// first where it is wider or taller than it is going to be (so sized it
// lies within its old rect still, and within its new one once moved: at
// no step does it reach past the two), then its place, its size, and its
// place again, as bars.rs lay does. Moved first, at its old size, the
// taskbar's window reached past its display: from along the bottom to
// along the right, still as wide as the display, its right end lay over
// the display to the right by the display's width less the side bar's,
// and on a display wider than twice the side bar (2560 px at 100 %, 4K at
// 150 %, an ultrawide) more of it lay there than here, so Windows took it
// for that display's and, that display at another scale, told it so: tao
// then put it on that display at the size Windows suggested, till the look
// (keep_on_screen) put it back (the user's report, 2026-11-07: set to the
// right, her taskbar flew over to the other display; on this machine, at
// 125 %, the window laid by the old strip meanwhile, 1285 x 60 at the
// bottom right, went to the display at 150 % for a moment: sized by
// Windows to 1071 x 50, the page's scale 1.5 and back). Sized first and then
// moved to a display of another scale, Windows gave it the size for that
// one (the scales' ratio over it): an external display at 125% plugged back
// in and made the main one while it sat on the laptop's at 175%, her taskbar
// came out 1830 x 615 for 2560 x 860, the strip floating up the screen and
// short of its end, until she was restarted (zhoupengjie/Wakuwaku#37); so
// its size is set again after the move. Never while minimized
// (keep_on_screen lays it once it is given back).
fn lay(win: &WebviewWindow, (x, y): (i32, i32), (w, h): (i32, i32)) {
    // Gone with Explorer's taskbar (embed.rs, which starts her anew): tao's
    // set_size on a window that is no more panics (see window).
    if !alive(win) {
        return;
    }
    if is_minimized(win) {
        return;
    }
    // A child of Windows' taskbar (embed.rs): laid in its client area, not
    // on the screen as tao would.
    if crate::embed::is_parented() {
        crate::embed::lay_child((x, y), (w, h));
        return;
    }
    for step in lay_steps(current_size(win), (w, h)) {
        match step {
            Step::Pos => {
                let _ = win.set_position(PhysicalPosition::new(x, y));
            }
            Step::Size(sw, sh) => {
                let _ = win.set_size(PhysicalSize::new(sw.max(1) as u32, sh.max(1) as u32));
            }
        }
    }
}

// A step of lay: the window's place, or a size (physical).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Step {
    Pos,
    Size(i32, i32),
}

// The steps that lay a window at a size, from its size now (None: not
// known): down to no more than the new size either way first, where it is
// more, then its place, the new size, and its place again.
fn lay_steps(now: Option<(i32, i32)>, (w, h): (i32, i32)) -> Vec<Step> {
    let mut steps = Vec::new();
    if let Some((nw, nh)) = now {
        let within = (nw.min(w), nh.min(h));
        if within != (nw, nh) {
            steps.push(Step::Size(within.0, within.1));
        }
    }
    steps.extend([Step::Pos, Step::Size(w, h), Step::Pos]);
    steps
}

// The window's size now (physical), asked of Windows directly (frameless.rs
// rect, on any thread); None where it cannot be asked.
fn current_size(_win: &WebviewWindow) -> Option<(i32, i32)> {
    #[cfg(windows)]
    if let Ok(h) = _win.hwnd() {
        return crate::frameless::rect(h.0 as isize).map(|r| (r.2, r.3));
    }
    None
}

// Laid again where it is meant to be (a change of scale under it).
fn lay_again(sh: &Shared, why: &str) {
    let (pos, size, bar) = {
        let isl = sh.island.lock().unwrap();
        (isl.pos, isl.size, isl.bar)
    };
    // The display it is meant for: the one her strip lies on, or the
    // island's middle; none between strips (a display gone, the new one not
    // yet taken by the taskbar's thread).
    let areas = sh.screens().areas;
    let on = |r: (i32, i32, i32, i32)| areas.iter().find(|a| r.0 >= a.mon.0 && r.1 >= a.mon.1 && r.0 + r.2 <= a.mon.0 + a.mon.2 && r.1 + r.3 <= a.mon.1 + a.mon.3);
    let meant = match bar {
        _ if sh.home() != "taskbar" => on((pos.0 + size.0 / 2, pos.1 + size.1 / 2, 1, 1)),
        Some(b) => on(b),
        None => None,
    };
    let Some(meant) = meant else { return };
    let Some(win) = window(sh) else { return };
    // Laid again only once it has that display's scale: of another (Windows
    // moving it off a display unplugged, the list of displays not yet
    // without it), its place is still to come, and laid now it went back
    // on the display going away for a moment.
    if win.scale_factor().is_ok_and(|sf| (sf - meant.sf).abs() > 0.01) {
        return;
    }
    let now = win.outer_size().map(|s| (s.width as i32, s.height as i32)).unwrap_or(size);
    if now != size {
        sh.log(&format!("island: sized by Windows to {}x{} ({why}); laid again at {pos:?} {size:?} on {:?} at {}", now.0, now.1, meant.mon, meant.sf));
        lay(&win, pos, size);
    }
}

fn place(sh: &Shared) {
    sync_bar(sh, true);
    let fitted = fit(sh);
    // Its home changed, perhaps (the taskbar's zoom, or none).
    fit_zoom(sh);
    tell_strip(sh, false);
    // Split or no longer: the dock comes or goes with the strip (bars.rs).
    crate::bars::sync(&crate::shared(&sh.app));
    let Some((pos, size)) = fitted else { return };
    if let Some(win) = window(sh) {
        lay(&win, pos, size);
    }
    // On another display, another notch (or none).
    crate::notch::settle(sh);
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
            .background_throttling(BackgroundThrottlingPolicy::Disabled)
            .build();
        match built {
            Ok(win) => {
                lay(&win, pos, size);
                let _ = win.set_ignore_cursor_events(true);
                crate::notch::settle(&sh);
                // A click anywhere else takes the focus from the settings: they close.
                let app = sh.app.clone();
                win.on_window_event(move |event| {
                    // Not when it is her own window being shown (Shared::show_window).
                    if let WindowEvent::Focused(false) = event {
                        if !crate::shared(&app).is_shuffling() {
                            let _ = app.emit_to("island", "island:blur", ());
                        }
                    }
                    // Its display's scale changed under it (a display plugged
                    // in or out, another made the main one): Windows sized it
                    // for the new scale; once it has, put where it belongs on
                    // the displays there are now, at its size (#37), not at
                    // the next look.
                    if let WindowEvent::ScaleFactorChanged { .. } = event {
                        let app = app.clone();
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            keep_on_screen(&crate::shared(&app));
                        });
                    }
                });
                #[cfg(windows)]
                if let Ok(hwnd) = win.hwnd() {
                    sh.island.lock().unwrap().hwnd = hwnd.0 as isize;
                    crate::tasks::no_transitions(hwnd.0 as isize);
                }
                sh.own_window(&win);
                // Its renderer gone: told, and the page loaded again (crash.rs).
                crate::crash::watch(&sh, &win);
                // Its plugins' frames kept to their own page (addons.rs guard_frames).
                crate::addons::guard_frames(&sh, &win, true);
            }
            Err(err) => sh.log(&format!("island: could not make its window: {err}")),
        }
    });
}

// At the top of the screen, a see-through look (islandTopLook) that
// Windows' acrylic does not blur (none set under 90 %, or it cannot be:
// backplate.rs) deepens over a maximized window on her display (island.js
// setDeep, zhoupengjie/Wakuwaku#27): looked at with the full screen's look
// (every 1.5 s, or a window to the front), told the page as it changes
// (island:over-max). Windows' mode with it: the page has it in pet:update
// alone there (no taskbar to hear it from), so a change of it draws anew.
// Nothing for the default black look, nor while she is quiet.
static OVER_MAX: AtomicBool = AtomicBool::new(false);
static TOP_LIGHT: AtomicU8 = AtomicU8::new(2);

pub fn look_over_max(sh: &Arc<Shared>) {
    let looks = sh.setting("islandTopLook");
    if sh.home() == "taskbar" || sh.is_quiet() || !looks.as_object().is_some_and(|m| m.values().any(Value::is_object)) {
        return;
    }
    let light = crate::wallpaper::look().0;
    if TOP_LIGHT.swap(light as u8, Ordering::SeqCst) != light as u8 {
        sh.redraw();
    }
    let look = &looks[if light { "light" } else { "dark" }];
    let deepens = look["bg"].is_string() && look["op"].as_u64().is_some_and(|op| op < 90) && (look["blur"].as_u64().unwrap_or(0) == 0 || !crate::backplate::available());
    let on = deepens && display(sh).is_some_and(crate::tasks::maximized_on);
    if OVER_MAX.swap(on, Ordering::SeqCst) != on {
        let _ = sh.app.emit_to("island", "island:over-max", json!({ "on": on }));
    }
}

// --- Out of the way of the window under it (zhoupengjie/Wakuwaku#49) ------------------
// At the top of the screen, the closed island lay over the window under it
// (a maximized browser's tab strip): a tab there was hard to reach or close,
// the island took the press. So, unless turned off (islandHideUnder), while
// a window lies under the closed island the page slides it up out of the
// way, a sliver left at the screen's edge and every press let through, as
// long as nothing wants the person (island.js setStow). Here: whether a
// window lies under it, looked at with the full screen's look (every 1.5 s,
// at once as the pointer comes near or its size changes); and the pointer
// calling it back down: resting at the screen's edge over it EDGE_MS with
// no button held (a press there is on the tab, as is one held, a tab or a
// window dragged up), or on the island. Once called it stays until the
// pointer has been away from about it (its band: the screen's top to a
// little below it) for LEAVE_MS.
const EDGE_PX: f64 = 2.0;
const EDGE_SIDE: f64 = 8.0;
const EDGE_MS: u128 = 350;
const BAND_SIDE: f64 = 24.0;
const BAND_BELOW: f64 = 12.0;
const LEAVE_MS: u128 = 600;

// The closed island on the screen (physical x, y, w, h), TOP below its
// window's top edge, or along the foot of the screen (#105) foot above the
// window's bottom edge: in the middle of the window's width, or at a
// corner (zhoupengjie/Wakuwaku#59) TOP in from the window's side there, as
// the page lays it out (anchor.js, style.css isl-at-left / isl-at-right /
// isl-at-bottom); the stowed band and the edge that calls it down
// (call_look) follow from this. None before the page said its size, or in
// another home.
fn closed_rect(isl: &Island) -> Option<(i32, i32, i32, i32)> {
    // Inside Windows' taskbar (embed.rs): the capsule itself, wherever this window is.
    if isl.embedded {
        return isl.cap;
    }
    let (w, h) = isl.stow.closed;
    if !isl.created || w <= 0.0 || h <= 0.0 {
        return None;
    }
    // In the page's px, by its scale (origin).
    let sf = isl.page_sf();
    let (w, h) = ((w * sf).round() as i32, (h * sf).round() as i32);
    let top = (TOP * sf).round() as i32;
    let x = match isl.at {
        Side::Left => isl.pos.0 + top,
        Side::Right => isl.pos.0 + isl.size.0 - w - top,
        Side::Center => isl.pos.0 + (isl.size.0 - w) / 2,
    };
    // Along the bottom foot up from the window's foot; else foot down from
    // its top (TOP with no taskbar's strip there, centred in one along the top).
    let y = if isl.bottom { isl.pos.1 + isl.size.1 - (isl.foot() * sf).round() as i32 - h } else { isl.pos.1 + (isl.foot() * sf).round() as i32 };
    Some((x, y, w, h))
}

// A window under the closed island, or none now: told the page as it
// changes (island:under).
pub fn look_under(sh: &Arc<Shared>) {
    // Kept as it was while it is not up, or nobody is about. Never along
    // the foot of the screen (#105): over Windows' taskbar, or on the work
    // area's foot, no window's top lies under the closed island.
    let (rect, wanted) = {
        let isl = sh.island.lock().unwrap();
        let wanted = sh.home() == "island" && sh.flag("islandHideUnder") && !isl.bottom && isl.strip.is_none();
        if wanted && (!isl.shown || sh.is_quiet()) {
            return;
        }
        (closed_rect(&isl), wanted)
    };
    let on = wanted && rect.is_some_and(crate::tasks::reaches_into);
    let called = {
        let mut isl = sh.island.lock().unwrap();
        if std::mem::replace(&mut isl.stow.under, on) == on {
            return;
        }
        // Nothing under it: nothing to call it down from.
        !on && std::mem::take(&mut isl.stow.called)
    };
    sh.log(&format!("island: {} {rect:?}", if on { "a window under it" } else { "no window under it now" }));
    let _ = sh.app.emit_to("island", "island:under", json!({ "on": on }));
    if called {
        let _ = sh.app.emit_to("island", "island:called", json!({ "on": false }));
    }
}

// The page's island at the top of the screen: up out of the way or not, and
// its closed size (logical). Of another size, looked at again at once.
pub fn set_stow(sh: &Shared, stowed: bool, width: f64, height: f64) {
    let resized = {
        let mut isl = sh.island.lock().unwrap();
        let s = &mut isl.stow;
        let size = (width.max(0.0), height.max(0.0));
        if stowed != s.stowed {
            sh.log(&format!("island: {}", if stowed { "up out of the way of the window under it" } else { "down again" }));
        }
        s.stowed = stowed;
        if !stowed {
            s.edge_since = None;
        }
        std::mem::replace(&mut s.closed, size) != size
    };
    if resized {
        crate::fullscreen::ask();
        // The capsule in Windows' taskbar laid again at its new size (embed.rs).
        crate::embed::look_soon();
    }
}

// One look at the pointer for it (poll; physical), at now; down: whether a
// button is held, asked only at the edge. Some(true) once it has called the
// island down, Some(false) once it has gone from it. Not while she is
// dragged past (passive), nor with no window under it.
fn call_look(isl: &mut Island, cursor: (i32, i32), passive: bool, now: std::time::Instant, down: impl FnOnce() -> bool) -> Option<bool> {
    let rect = closed_rect(isl).filter(|_| isl.stow.under && !passive);
    let (sf, top, over) = (isl.sf(), isl.pos.1, isl.ct.over);
    let s = &mut isl.stow;
    let Some((x, y, w, h)) = rect else {
        s.edge_since = None;
        s.away_since = None;
        s.pressed = false;
        return std::mem::take(&mut s.called).then_some(false);
    };
    let px = |v: f64| (v * sf).round() as i32;
    if s.called {
        let about = cursor.0 >= x - px(BAND_SIDE) && cursor.0 < x + w + px(BAND_SIDE) && cursor.1 >= top - 1 && cursor.1 < y + h + px(BAND_BELOW);
        if about || over {
            s.away_since = None;
            return None;
        }
        if now.duration_since(*s.away_since.get_or_insert(now)).as_millis() < LEAVE_MS {
            return None;
        }
        s.called = false;
        s.away_since = None;
        return Some(false);
    }
    // On it (the page's island taking the pointer): it is wanted there.
    if over {
        s.called = true;
        s.edge_since = None;
        return Some(true);
    }
    let at_edge = s.stowed && cursor.0 >= x - px(EDGE_SIDE) && cursor.0 < x + w + px(EDGE_SIDE) && cursor.1 >= top - 1 && cursor.1 < top + px(EDGE_PX).max(1);
    if !at_edge {
        s.edge_since = None;
        s.pressed = false;
        return None;
    }
    s.pressed = s.pressed || down();
    let since = *s.edge_since.get_or_insert(now);
    if s.pressed || now.duration_since(since).as_millis() < EDGE_MS {
        return None;
    }
    s.called = true;
    s.edge_since = None;
    Some(true)
}

// Whether its page has said it is ready (for /health): never while no
// window is made for it (another copy holding the taskbar, taskbar.rs).
pub fn is_ready(sh: &Shared) -> bool {
    sh.island.lock().unwrap().ready
}

// Its page has its listeners up: draw, show, say hello.
pub fn ready(sh: &Arc<Shared>) {
    sh.log("island: page ready");
    // A new page knows of no maximized window yet (look_over_max), nor of
    // one under it (look_under), and has it down.
    OVER_MAX.store(false, Ordering::SeqCst);
    let pending = {
        let mut isl = sh.island.lock().unwrap();
        isl.ready = true;
        let closed = isl.stow.closed;
        isl.stow = Stow { closed, ..Stow::default() };
        isl.pending.take()
    };
    crate::fullscreen::ask();
    sh.redraw();
    taskbar::resend(sh);
    tell_strip(sh, true);
    // The band at a Mac's screen top, now that the page hears it.
    crate::notch::settle(sh);
    sh.greet();
    // Taking the taskbar over: Windows' own laid over ours first, put away
    // under it, then turned to dust that settles into ours.
    let covered = handoff_cover(sh);
    apply_visibility(sh);
    if covered {
        handoff_ask(sh, json!({ "step": "go" }));
        sh.log("handoff: go");
    }
    if let Some(tab) = pending {
        let _ = sh.app.emit_to("island", "island:settings", json!({ "tab": tab }));
    }
}

pub fn apply_visibility(sh: &Arc<Shared>) {
    // Told before it shows, so the strip never shows over the full screen;
    // told it is over once it has gone (or, the full screen gone, stays up).
    let over = over_full(sh);
    let told = sh.island.lock().unwrap().over_full;
    if over && !told {
        tell_over_full(sh, true);
    }
    show_or_hide(sh);
    if !over && told {
        tell_over_full(sh, false);
    }
    // Into Windows' taskbar or out of it, as it is used now (embed.rs).
    crate::embed::apply(sh);
}

// Up over an app full screen for the prompts alone (show_asks), her taskbar
// the home: the page shows her island and what opens from it, not the strip
// under them (style.css over-full), and the pointer goes through there to
// what is full screen. The whole taskbar, the tray and the clock, came up
// over the video with them (the test session, 2026-10-13). Not with the
// settings open: they stand on the strip.
fn over_full(sh: &Shared) -> bool {
    sh.home() == "taskbar" && sh.is_fullscreen() && {
        let isl = sh.island.lock().unwrap();
        isl.for_asks && !isl.temp && !isl.settings_open
    }
}

fn tell_over_full(sh: &Shared, on: bool) {
    sh.island.lock().unwrap().over_full = on;
    let _ = sh.app.emit_to("island", "island:over-full", json!({ "on": on }));
    sh.log(&format!("island: {}", if on { "over the full screen for the prompts alone, the strip unseen" } else { "the strip seen again" }));
}

fn show_or_hide(sh: &Arc<Shared>) {
    // Another copy of her holding the taskbar: this one waits, its window
    // not shown over the other's.
    if sh.home() == "taskbar" {
        taskbar::look_for_other(sh);
    }
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
        // The taskbar keeps the strip it holds while an app full screen
        // hides it; it takes none then: one taken with its window hidden
        // keeps no room (2026-10-10), so it waits for the window to show.
        // Given back, though, to another copy of her holding it too.
        let held = sh.island.lock().unwrap().bar_for.is_some();
        sync_bar(sh, sh.home() == "taskbar" && held && !taskbar::waiting());
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
    // The blur under the island goes with it, and comes back with it (backplate.rs).
    if want {
        crate::backplate::again();
    } else {
        crate::backplate::hide();
    }
    // Her taskbar on the other displays, as the settings say (bars.rs).
    crate::bars::sync(sh);
}

// --- The handover with Windows' taskbar (handoff.js) ----------------------------------
// Windows' taskbar never gone for a moment. Taking over: a picture of it
// laid over ours, shown exactly over it, before it is put away; then its
// picture turns to dust that settles into ours. Quitting: ours turns to
// dust, Windows' shown again under it, ours fading over it before she
// goes. Each answer of the page's (covered, bare, done) waited for so long
// at most: neither side can hold the other. Not on the main thread: its
// events reach the page through the main thread.
static HANDOFF: Mutex<Vec<String>> = Mutex::new(Vec::new());
static HANDOFF_SAID: std::sync::Condvar = std::sync::Condvar::new();

// What the page answers (taskbar_handoff).
pub fn handoff_answer(what: &str) {
    HANDOFF.lock().unwrap().push(what.to_string());
    HANDOFF_SAID.notify_all();
}

// A step told the page; its answers from before forgotten.
fn handoff_ask(sh: &Shared, step: Value) {
    HANDOFF.lock().unwrap().clear();
    let _ = sh.app.emit_to("island", "taskbar:handoff", step);
}

// An answer, within ms at most: true once it came.
fn handoff_wait(what: &str, ms: u64) -> bool {
    let said = HANDOFF.lock().unwrap();
    let (said, _) = HANDOFF_SAID.wait_timeout_while(said, std::time::Duration::from_millis(ms), |s| !s.iter().any(|x| x == what)).unwrap();
    said.iter().any(|x| x == what)
}

// Her page ready, the taskbar to be taken (not yet up, nothing full screen,
// no other copy of hers holding it): Windows' taskbar pictured, her window
// shown exactly over it with the picture laid on, before it is put away.
// True once covered: then `go` follows the taking. A picture that cannot
// be had (auto-hidden, already put away) is none: the dust comes from above.
fn handoff_cover(sh: &Arc<Shared>) -> bool {
    if sh.home() != "taskbar" || taskbar::is_up() {
        return false;
    }
    taskbar::look_for_other(sh);
    if !is_shown(sh) {
        return false;
    }
    let Some(area) = area(sh) else { return false };
    // Windows' strip pictured only where hers takes its place: along the
    // bottom. Along another edge none (the dust gathers from above).
    let picture = (edge(sh) == Edge::Bottom).then(|| taskbar::strip_picture(area.mon)).flatten();
    let _ = fit(sh);
    let (pos, size, hwnd) = {
        let mut isl = sh.island.lock().unwrap();
        isl.shown = true;
        (isl.pos, isl.size, isl.hwnd)
    };
    let Some(win) = window(sh) else { return false };
    lay(&win, pos, size);
    let _ = win.show();
    let shown = std::time::Instant::now();
    // Over Windows' own, which is kept on top too.
    crate::tasks::raise_top(hwnd);
    let pictured = picture.is_some();
    // Its window's size (physical) too: the page lays the picture only once
    // its own size has come to that (a window made bigger reaches the page
    // some frames later; laid before, it sat too small, up and to the left).
    handoff_ask(sh, json!({ "step": "in", "picture": picture, "size": [size.0, size.1] }));
    let covered = handoff_wait("covered", 800);
    // Drawn by the page is not yet on the screen: a window just shown hands
    // WebView2's frames to DWM some 40 ms later, and Windows' strip put away
    // at once left the bare desktop under the strip that long (whatever the
    // material: the test session's films, 2026-10-13). So two of DWM's passes
    // waited for, and 150 ms since the window showed at least; the picture is
    // Windows' strip itself, so the wait cannot be seen.
    if pictured {
        crate::tasks::dwm_flush();
        crate::tasks::dwm_flush();
        let least = std::time::Duration::from_millis(150);
        if let Some(rest) = least.checked_sub(shown.elapsed()) {
            std::thread::sleep(rest);
        }
    }
    sh.log(&format!(
        "handoff: in ({}), {} ({} ms after the window showed)",
        if pictured { "pictured" } else { "no picture" },
        if covered { "covered" } else { "no answer in 800 ms" },
        shown.elapsed().as_millis()
    ));
    true
}

// Quitting with the taskbar up (and nothing full screen): hers turns to
// dust, Windows' shown under it once the page says so (bare), hers kept
// over it while it fades (done). Then she goes, and gives it back as ever.
pub fn handoff_out(sh: &Shared) {
    let shown = sh.island.lock().unwrap().shown;
    if sh.home() != "taskbar" || !taskbar::is_up() || !shown || sh.is_fullscreen() {
        return;
    }
    handoff_ask(sh, json!({ "step": "out" }));
    let bare = handoff_wait("bare", 500);
    taskbar::handing_back();
    crate::shell::show_bars();
    crate::tasks::raise_top(hwnd(sh));
    let done = bare && handoff_wait("done", 1500);
    sh.log(&format!("handoff: out, {}, {}", if bare { "bare" } else { "no answer in 500 ms" }, if done { "done" } else { "not waited for" }));
}

// Minimized by another program. Claude's computer use hides every window of
// a program it was not given as it starts, hers too, and gives them back as
// its turn ends: taken for Windows moving it, hers was laid again at its size
// every 2 s, minimized still, for 35 min, a bar across the screen with its
// name and a close button left on it after (2026-11-03; frameless.rs). Left
// so now, as the other program wants it, till it is given back.
static MINIMIZED: AtomicBool = AtomicBool::new(false);

fn is_minimized(_win: &WebviewWindow) -> bool {
    #[cfg(windows)]
    if let Ok(h) = _win.hwnd() {
        return crate::frameless::minimized(h.0 as isize);
    }
    false
}

// Kept in its place (a display gone, the scale changed, another bar). And
// put back where it was put if Windows moved it: the taskbar's window, which
// reaches over its own strip, was pushed up into the work area by the strip's
// height (2026-10-10, as a game went full screen and back). Not while it is
// minimized (MINIMIZED); given back, laid where it belongs at once.
pub fn keep_on_screen(sh: &Shared) {
    // A display come or gone: her taskbar on the others too (bars.rs).
    crate::bars::sync(&crate::shared(&sh.app));
    let (is_up, pos, size) = {
        let isl = sh.island.lock().unwrap();
        (isl.shown && !isl.holding && !isl.reaching, isl.pos, isl.size)
    };
    if !is_up {
        return;
    }
    let Some(win) = window(sh) else { return };
    let minimized = is_minimized(&win);
    let was = MINIMIZED.swap(minimized, Ordering::SeqCst);
    if minimized {
        if !was {
            sh.log(&format!("island: minimized by another program ({} in front); left so till it is given back", crate::fullscreen::front()));
        }
        return;
    }
    let given_back = was;
    // Where it belongs now (a display come or gone moves it), then its size
    // as Windows left it: place compares only with what it asked for last,
    // the same, and would never set it again (#37).
    place(sh);
    if given_back {
        let (pos, size) = {
            let isl = sh.island.lock().unwrap();
            (isl.pos, isl.size)
        };
        sh.log(&format!("island: given back after it was minimized; laid at {pos:?} {size:?}"));
        lay(&win, pos, size);
        // Any frame drawn on it meanwhile wiped (frameless.rs).
        #[cfg(windows)]
        if let Ok(h) = win.hwnd() {
            crate::frameless::wipe(h.0 as isize);
        }
    }
    lay_again(sh, "seen by the look");
    // Along the foot of the screen, over Windows' taskbar again should it have come over her (#105).
    uncover(sh);
    if sh.home() != "taskbar" {
        return;
    }
    if let Ok(at) = win.outer_position() {
        if (at.x, at.y) != pos && sh.island.lock().unwrap().pos == pos {
            sh.log(&format!("island: moved by Windows to ({}, {}); put back at {pos:?}", at.x, at.y));
            lay(&win, pos, size);
            // Windows took it for a window of its own: any frame it drew on it wiped (frameless.rs).
            #[cfg(windows)]
            if let Ok(h) = win.hwnd() {
                crate::frameless::wipe(h.0 as isize);
            }
        }
    }
}

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
    if came && sh.home() == "island" && sh.flag("islandHideUnder") {
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
        crate::wake_monitor();
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
// capsule's panel and a toast with 撤销 (picker.js solids) whenever they
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
// as Windows' (TASKBAR_H, island.js the same), the strip as thick as Windows
// gives it (island.js --strip-h): with the text at 110% the page's px were
// 1.65 of the screen's at 150%, the strip 43.6 of them, her end 48 of them
// laid on its foot, up over its top, the flow light's ring past it
// (zhoupengjie/Wakuwaku#90); bigger text sizes, more. The text size is what
// the page's scale (page_sf) is over the display's (sf) and the zoom it has
// (zoom). None: as it is.
fn zoom_for(taskbar: bool, sf: f64, page_sf: f64, zoom: f64) -> Option<f64> {
    let text = page_sf / (sf * zoom);
    let want = if taskbar && text > 1.0 { 1.0 / text } else { 1.0 };
    ((want - zoom).abs() > 0.001).then_some(want)
}

// The page's zoom as its home and scale want it (zoom_for), set when it is
// to change, its scale taken to be what it will be once set (the page tells
// it again then).
fn fit_zoom(sh: &Shared) {
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

// Open them on a page (or where they were). With no home up (she is out of
// sight), one rises for them and goes once they close.
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
    keep_on_top(sh);
    // Mail's agents looked for again while they are open; the monitor's
    // sensors read whole for their list.
    crate::mail::agent::watch(sh);
    crate::wake_monitor();
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
    keep_on_top(sh);
    // Set up once Claude Code or Codex is connected, not as the settings
    // close (a click outside closed them, and a new user, nothing
    // connected, was never asked again: the test session, 2026-10-12).
    if !sh.flag("onboarded") && crate::connection::seems_connected() {
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

// --- The prompts, asked for from another display ---------------------------------------

// Her capsule on another display's taskbar pressed (bars.rs bar_asks): the
// prompt waiting shown in her island, as a press on hers would (it opens
// for a prompt by itself, so it is there already): risen should an app
// full screen have hidden it, kept up until the prompts are answered (as
// for the settings), raised over any window on top, and the page told to
// draw the eye to it (island:look-here) on its own display.
pub fn show_asks(sh: &Arc<Shared>) {
    let risen = {
        let up = is_shown(sh);
        let mut isl = sh.island.lock().unwrap();
        isl.for_asks |= !up;
        !up
    };
    sh.log(&format!("island: the prompt asked for from another display{}", if risen { " (risen for it)" } else { "" }));
    if risen {
        apply_visibility(sh);
    }
    // (Not while the settings, locked, are let go under other windows.)
    let hwnd = hwnd(sh);
    if hwnd != 0 && TOPMOST.load(Ordering::SeqCst) {
        crate::tasks::raise_top(hwnd);
    }
    let _ = sh.app.emit_to("island", "island:look-here", json!({}));
}

// Something opened up from her taskbar's strip (island.js openedUp): the
// island, a program's windows, a flyout. Her window is kept on top, but a
// window can lie over it all the same: one kept on top itself that came to
// the front after hers, or hers no longer kept on top. A maximized window
// stops at the strip, so only what opens above the strip is under it
// (zhoupengjie/Wakuwaku#55: the open island cut off at the strip's top, an
// editor over the rest). Windows' own taskbar opens its flyouts in a band
// above every window kept on top; hers is raised to the top of them as it
// opens, should any other lie over it (not while the settings, locked, are
// let go under other windows), and the line says which. Her island along
// the foot of the screen (#105), laid over Windows' own taskbar, the same.
pub fn raise(sh: &Arc<Shared>) {
    let hwnd = hwnd(sh);
    let (shown, over) = {
        let isl = sh.island.lock().unwrap();
        (isl.shown, isl.bottom || isl.strip.is_some())
    };
    // Not as a child of Windows' taskbar (embed.rs): in its order there.
    if hwnd == 0 || !shown || !(sh.home() == "taskbar" || over) || !TOPMOST.load(Ordering::SeqCst) || crate::embed::is_parented() {
        return;
    }
    let own = sh.own.lock().unwrap().clone();
    let Some((class, on_top, _)) = crate::tasks::cover(hwnd, &own) else { return };
    let mine = crate::tasks::is_topmost(hwnd);
    crate::tasks::raise_top(hwnd);
    sh.log(&format!(
        "island: opened under another window ({class}, {}; hers {}): raised over it",
        if on_top { "kept on top" } else { "not kept on top" },
        if mine { "kept on top" } else { "not kept on top" }
    ));
}

// Her island along the foot of the screen, laid over Windows' own taskbar
// (islandEdge taskbar, the embedding off, zhoupengjie/Wakuwaku#105): Explorer brings its
// taskbar to the top of the windows kept on top as it is pressed (Start, a
// button, the tray), over hers, which is then hidden under it, the pointer
// never reaching it. So it is owned by Windows' taskbar there (embed.rs
// detach, over_wanted), which Windows keeps it over; and, should it be found
// under a taskbar all the same (another's kept on top, Windows' own along a
// side or hiding itself, not its owner), raised over it again: every
// 2 s (keep_on_screen), and as the pointer comes near it (poll). Only from
// under a taskbar: Windows' own (Shell_TrayWnd, a second display's), or
// another's kept on top within the strip's band (Start11's and the like);
// the Start menu and the flyouts open above every window kept on top, and
// over her island is right (the user's call, 2026-11-07). Logged as it is
// first found under one, not at each raising.
static UNDER_TASKBAR: AtomicBool = AtomicBool::new(false);

pub fn uncover(sh: &Shared) {
    // The strip's band: from the window's foot up along the bottom, from
    // its top down along the top (a taskbar there). Not with her capsule
    // inside the taskbar: the window shows only as it is used, raised then.
    let (hwnd, band) = {
        let isl = sh.island.lock().unwrap();
        (isl.hwnd, (isl.shown && !isl.embedded).then_some(isl.strip).flatten().map(|thick| if isl.bottom { (true, isl.pos.1 + isl.size.1 - thick) } else { (false, isl.pos.1 + thick) }))
    };
    let Some((low, band)) = band else { return };
    if hwnd == 0 || !TOPMOST.load(Ordering::SeqCst) || crate::embed::is_parented() {
        return;
    }
    let own = sh.own.lock().unwrap().clone();
    let taskbar = crate::tasks::cover(hwnd, &own).filter(|(class, on_top, r)| matches!(class.as_str(), "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") || (*on_top && if low { r.1 >= band - 2 } else { r.3 <= band + 2 }));
    let Some((class, _, _)) = taskbar else {
        UNDER_TASKBAR.store(false, Ordering::SeqCst);
        return;
    };
    crate::tasks::raise_top(hwnd);
    if !UNDER_TASKBAR.swap(true, Ordering::SeqCst) {
        sh.log(&format!("island: under the taskbar ({class}): raised over it"));
    }
}

// The prompts answered: an island risen for them goes.
pub fn asks_answered(sh: &Arc<Shared>) {
    let was = std::mem::take(&mut sh.island.lock().unwrap().for_asks);
    if was {
        apply_visibility(sh);
    }
}

// The settings' two buttons by ✕ (settings.js): locked ("settingsPin"), a
// click outside does not close them, and other windows come over them as
// over any window (hers kept on top only while they are not open); on top
// as well ("settingsTop"), they stay over every window, another program's
// own "on top" one too: looked at every 0.7 s, raised should one be over
// them. Unlocked, a click outside closes them: on top as ever.
static TOPMOST: AtomicBool = AtomicBool::new(true);
static WATCHING_TOP: AtomicBool = AtomicBool::new(false);

pub fn keep_on_top(sh: &Arc<Shared>) {
    let open = sh.island.lock().unwrap().settings_open;
    let (locked, top) = (sh.flag("settingsPin"), sh.flag("settingsTop"));
    let topmost = !(open && locked && !top);
    if TOPMOST.swap(topmost, Ordering::SeqCst) != topmost {
        if let Some(win) = window(sh) {
            let _ = win.set_always_on_top(topmost);
            // On top again (or not) takes a Mac's window off the menu bar's level.
            if topmost {
                crate::notch::settle(sh);
            }
        }
        sh.log(&format!("island: settings {}", if topmost { "on top of other windows" } else { "under the windows pressed" }));
    }
    if !(open && locked && top) || WATCHING_TOP.swap(true, Ordering::SeqCst) {
        return;
    }
    let sh = sh.clone();
    std::thread::spawn(move || loop {
        let watched = || sh.island.lock().unwrap().settings_open && sh.flag("settingsPin") && sh.flag("settingsTop");
        while watched() {
            let hwnd = hwnd(&sh);
            let own = sh.own.lock().unwrap().clone();
            if hwnd != 0 && crate::tasks::covered(hwnd, &own) {
                crate::tasks::raise_top(hwnd);
            }
            std::thread::sleep(std::time::Duration::from_millis(700));
        }
        WATCHING_TOP.store(false, Ordering::SeqCst);
        // Asked again in the moment between the last look and letting go.
        if !watched() || WATCHING_TOP.swap(true, Ordering::SeqCst) {
            return;
        }
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
        "caught": isl.ct.caught(),
        "editing": isl.editing,
        "solid": isl.solid.len(),
        "holding": isl.holding,
        "reaching": isl.reaching,
        "temp": isl.temp,
        "settingsOpen": isl.settings_open,
        "bar": isl.bar.map(|b| json!([b.0, b.1, b.2, b.3])),
        "at": isl.at.name(),
        "edge": if isl.bottom { "bottom" } else { "top" },
        "strip": isl.strip,
        "embedded": isl.embedded,
        "cap": isl.cap.map(|c| json!([c.0, c.1, c.2, c.3])),
        "capRoom": isl.cap_room,
        "mx": isl.mx,
        "opened": isl.opened,
        "embed": crate::embed::where_(),
        "stow": {
            "closed": closed_rect(&isl).map(|r| json!([r.0, r.1, r.2, r.3])),
            "under": isl.stow.under,
            "stowed": isl.stow.stowed,
            "called": isl.stow.called,
        },
    })
}

#[cfg(test)]
mod zoom_tests {
    use super::zoom_for;

    // 150% with the text at 110% (zhoupengjie/Wakuwaku#90): the taskbar's
    // page zoomed to 1/1.1, its scale 1.5 again; then as it is. In the
    // island at the top, or with no text size, 1.
    #[test]
    fn the_text_size_taken_out_of_the_taskbar() {
        let z = zoom_for(true, 1.5, 1.6500000953674316, 1.0).unwrap();
        assert!((z - 1.0 / 1.1).abs() < 1e-6, "{z}");
        assert_eq!(zoom_for(true, 1.5, 1.5, z), None, "told its new scale: kept");
        assert_eq!(zoom_for(true, 1.5, 1.5, 1.0), None, "no text size");
        // Back in the island: unzoomed, its scale the text size's again.
        assert_eq!(zoom_for(false, 1.5, 1.5, z), Some(1.0));
        assert_eq!(zoom_for(false, 1.5, 1.65, 1.0), None);
        // The text at 225% on a display at 100%.
        assert!((zoom_for(true, 1.0, 2.25, 1.0).unwrap() - 1.0 / 2.25).abs() < 1e-9);
    }
}

#[cfg(test)]
mod lay_tests {
    use super::{lay_steps, Island, Step};
    use crate::appbar::Edge;
    use crate::screen::Area;

    // The rect (x, y, w, h) after each step, from where the window is.
    fn rects(from: (i32, i32, i32, i32), to: (i32, i32), steps: &[Step]) -> Vec<(i32, i32, i32, i32)> {
        let mut r = from;
        steps
            .iter()
            .map(|s| {
                match s {
                    Step::Pos => (r.0, r.1) = to,
                    Step::Size(w, h) => (r.2, r.3) = (*w, *h),
                }
                r
            })
            .collect()
    }

    fn within(r: (i32, i32, i32, i32), of: (i32, i32, i32, i32)) -> bool {
        r.0 >= of.0 && r.1 >= of.1 && r.0 + r.2 <= of.0 + of.2 && r.1 + r.3 <= of.1 + of.3
    }

    // From along the bottom of a 2560-wide display at 125 % to along its
    // right, and back: at no step does the window reach past its old rect
    // and its new one (moved first at its old width, it lay over the
    // display to the right, 2026-11-07).
    #[test]
    fn the_window_keeps_within_its_old_rect_and_its_new_one() {
        let (bottom, right) = ((0, 580, 2560, 860), (1275, 0, 1285, 1440));
        let steps = lay_steps(Some((bottom.2, bottom.3)), (right.2, right.3));
        assert_eq!(steps, vec![Step::Size(1285, 860), Step::Pos, Step::Size(1285, 1440), Step::Pos]);
        for r in rects(bottom, (right.0, right.1), &steps) {
            assert!(within(r, bottom) || within(r, right), "{r:?}");
        }
        let steps = lay_steps(Some((right.2, right.3)), (bottom.2, bottom.3));
        assert_eq!(steps, vec![Step::Size(1285, 860), Step::Pos, Step::Size(2560, 860), Step::Pos]);
        for r in rects(right, (bottom.0, bottom.1), &steps) {
            assert!(within(r, bottom) || within(r, right), "{r:?}");
        }
        // Grown only (room asked for): its place first, as before.
        assert_eq!(lay_steps(Some((950, 165)), (950, 400)), vec![Step::Pos, Step::Size(950, 400), Step::Pos]);
        // Its size not known: as before.
        assert_eq!(lay_steps(None, (2560, 860)), vec![Step::Pos, Step::Size(2560, 860), Step::Pos]);
    }

    // The strip granted for another edge (the edge changed, the new one not
    // yet asked) is not used: the side bar as tall as its display, not as
    // thick as the bottom strip was.
    #[test]
    fn a_strip_granted_for_another_edge_is_not_used() {
        let mon = (0, 0, 2560, 1440);
        let area = Area { x: 0, y: 0, w: 2560, h: 1380, sf: 1.25, mon };
        let isl = Island { created: true, sf: 1.25, bar: Some((0, 1380, 2560, 60)), bar_for: Some((mon, 60, "taskbar", Edge::Bottom, true)), ..Island::default() };
        assert_eq!(isl.strip_now(Edge::Bottom, mon), Some((0, 1380, 2560, 60)));
        assert_eq!(isl.strip_now(Edge::Right, mon), None);
        assert_eq!(isl.strip_now(Edge::Bottom, (2560, -149, 2560, 1600)), None, "another display's");
        assert_eq!(isl.size_for("taskbar", &area, Edge::Right, 48.0, false), (1285, 1440));
        assert_eq!(isl.size_for("taskbar", &area, Edge::Bottom, 48.0, false), (2560, 860));
        // Granted for the right, shortened by another bar: as long as granted.
        let isl = Island { bar: Some((2500, 0, 60, 1380)), bar_for: Some((mon, 60, "taskbar", Edge::Right, false)), ..isl };
        assert_eq!(isl.size_for("taskbar", &area, Edge::Right, 48.0, false), (1285, 1380));
        // Asked anew (take_strip_again): none till it is granted.
        assert_eq!(Island { bar_for: None, ..isl }.strip_now(Edge::Right, mon), None);
    }
}

#[cfg(test)]
mod stow_tests {
    use super::{about, call_look, closed_rect, Island, Side, Stow, EDGE_MS, LEAVE_MS, TOP};
    use crate::screen::Area;
    use std::time::{Duration, Instant};

    // Her island at 125 % on a 2560-wide display: the window 950 wide at
    // the top centre, the closed island 300 x 36 in it (375 x 45 on the
    // screen), up out of the way of a window under it.
    fn stowed() -> Island {
        Island { created: true, pos: (805, 0), size: (950, 165), sf: 1.25, stow: Stow { closed: (300.0, 36.0), under: true, stowed: true, ..Stow::default() }, ..Island::default() }
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn the_closed_island_on_the_screen() {
        assert_eq!(closed_rect(&stowed()), Some((1092, 10, 375, 45)));
        let mut isl = stowed();
        isl.stow.closed = (0.0, 0.0);
        assert_eq!(closed_rect(&isl), None, "no size said yet");
    }

    // Windows' text size at 110 %: the page's px bigger than the display's
    // scale (1.375 at 125 %), the island too (zhoupengjie/Wakuwaku#90).
    #[test]
    fn the_closed_island_by_the_pages_scale() {
        let isl = Island { page_sf: 1.375, ..stowed() };
        assert_eq!(closed_rect(&isl), Some((1073, 11, 413, 50)));
        // One not told: the display's.
        assert_eq!(closed_rect(&Island { page_sf: 0.0, ..stowed() }), Some((1092, 10, 375, 45)));
    }

    // At a top corner (islandPos, zhoupengjie/Wakuwaku#59): the window flush
    // with the work area's side at its top, the closed island TOP (8 px, 10
    // at 125 %) in from that side, as from the top.
    #[test]
    fn at_a_top_corner() {
        let area = Area { x: 0, y: 0, w: 2560, h: 1392, sf: 1.25, mon: (0, 0, 2560, 1440) };
        assert_eq!(Side::Left.place(&area, 950), (0, 0));
        assert_eq!(Side::Right.place(&area, 950), (1610, 0));
        assert_eq!(Side::Center.place(&area, 950), (805, 0));
        // Grown wider at the right: its right edge kept.
        assert_eq!(Side::Right.place(&area, 1200), (1360, 0));
        // A display left of the main one, its taskbar at the top.
        let left = Area { x: -1920, y: 48, w: 1920, h: 1032, sf: 1.0, mon: (-1920, 0, 1920, 1080) };
        assert_eq!(Side::Right.place(&left, 760), (-760, 48));
        assert_eq!(Side::Left.place(&left, 760), (-1920, 48));
        let mut isl = stowed();
        isl.at = Side::Left;
        isl.pos = (0, 0);
        assert_eq!(closed_rect(&isl), Some((10, 10, 375, 45)));
        isl.at = Side::Right;
        isl.pos = (1610, 0);
        assert_eq!(closed_rect(&isl), Some((2175, 10, 375, 45)));
        // The setting read: anything else the centre.
        assert_eq!([Side::of("left"), Side::of("right"), Side::of("center"), Side::of("top")], [Side::Left, Side::Right, Side::Center, Side::Center]);
        assert_eq!([Side::Left.name(), Side::Center.name(), Side::Right.name()], ["left", "center", "right"]);
    }

    // Along the foot of the screen (islandEdge taskbar, zhoupengjie/Wakuwaku#105)
    // at 125 %: the window as tall as the taskbar's strip and room (688, 860
    // px), its foot on the screen's bottom edge over Windows' taskbar (48,
    // 60 px), the closed island centred in the strip, 6 px (7.5, rounded to
    // 8) up from the foot; with no taskbar there, on the work area's foot,
    // TOP up from it. At a corner, in from the side as at the top.
    #[test]
    fn along_the_foot_of_the_screen() {
        let area = Area { x: 0, y: 0, w: 2560, h: 1380, sf: 1.25, mon: (0, 0, 2560, 1440) };
        let size = (950, 860);
        assert_eq!(Side::Center.place_above(&area, size, 1440), (805, 580));
        assert_eq!(Side::Left.place_above(&area, size, 1440), (0, 580));
        assert_eq!(Side::Right.place_above(&area, size, 1440), (1610, 580));
        // Windows' taskbar elsewhere: the work area's foot.
        assert_eq!(Side::Center.place_above(&area, size, 1380), (805, 520));
        let mut isl = Island { pos: (805, 580), size, bottom: true, strip: Some(60), ..stowed() };
        assert_eq!(isl.foot(), 6.0);
        assert_eq!(closed_rect(&isl), Some((1092, 1387, 375, 45)));
        isl.at = Side::Right;
        isl.pos = (1610, 580);
        assert_eq!(closed_rect(&isl), Some((2175, 1387, 375, 45)));
        // Windows 10's 40 px taskbar: 2 px up; one thinner than the island: on the foot.
        assert_eq!(Island { bottom: true, strip: Some(50), ..stowed() }.foot(), 2.0);
        assert_eq!(Island { bottom: true, strip: Some(30), ..stowed() }.foot(), 0.0);
        // No taskbar under it: TOP up from the work area's foot.
        let mut isl = Island { pos: (805, 520), size, bottom: true, ..stowed() };
        assert_eq!(isl.foot(), TOP);
        assert_eq!(closed_rect(&isl), Some((1092, 1325, 375, 45)));
        // Windows' taskbar along the top (the window's top on the screen's):
        // centred in its strip, 6 px down; with none there, TOP as ever.
        isl.bottom = false;
        isl.strip = Some(60);
        isl.pos = (805, 0);
        assert_eq!(isl.foot(), 6.0);
        assert_eq!(closed_rect(&isl), Some((1092, 8, 375, 45)));
        isl.strip = None;
        assert_eq!(isl.foot(), TOP);
        assert_eq!(closed_rect(&isl), Some((1092, 10, 375, 45)));
    }

    // Her capsule inside Windows' taskbar (embed.rs): the window about it,
    // kept on the screen, and the closed island is the capsule itself.
    #[test]
    fn about_the_capsule_in_the_taskbar() {
        let area = Area { x: 0, y: 0, w: 2560, h: 1380, sf: 1.25, mon: (0, 0, 2560, 1440) };
        // In the middle of the strip: the window's middle on the capsule's
        // (1279, its half rounded down), 475 in from its left.
        assert_eq!(about(&area, (1092, 1387, 375, 45), 950), (804, 475));
        // At the strip's left end: the window at the screen's left, the capsule's middle 192 in.
        assert_eq!(about(&area, (5, 1387, 375, 45), 950), (0, 192));
        // Before the tray at the right: the window stops at the screen's right edge.
        assert_eq!(about(&area, (2180, 1387, 375, 45), 950), (1610, 757));
        // Well within the screen: the window's middle on the capsule's.
        assert_eq!(about(&area, (1518, 1387, 375, 45), 950), (1230, 475));
        // On a second display to the right the same, from its own left.
        let right = Area { x: 2560, y: 0, w: 1920, h: 1032, sf: 1.0, mon: (2560, 0, 1920, 1080) };
        assert_eq!(about(&right, (2570, 1046, 300, 36), 760), (2560, 160));
        // The closed island is the capsule, wherever the window is; out of the taskbar, as laid.
        let isl = Island { pos: (0, 580), size: (950, 860), bottom: true, strip: Some(60), embedded: true, cap: Some((5, 1387, 375, 45)), mx: Some(192), ..stowed() };
        assert_eq!(closed_rect(&isl), Some((5, 1387, 375, 45)));
        assert_eq!(closed_rect(&Island { embedded: false, ..isl }), Some((287, 1387, 375, 45)));
    }

    // Resting at the screen's edge over it calls it down; a moment there
    // does not, nor passing beside it.
    #[test]
    fn resting_at_the_edge_calls_it() {
        let mut isl = stowed();
        let t = Instant::now();
        let edge = (1280, 0);
        assert_eq!(call_look(&mut isl, edge, false, t, || false), None);
        assert_eq!(call_look(&mut isl, edge, false, t + ms(EDGE_MS as u64 - 50), || false), None);
        assert_eq!(call_look(&mut isl, (1280, 1), false, t + ms(EDGE_MS as u64), || false), Some(true));
        assert!(isl.stow.called);
        let mut isl = stowed();
        for (i, at) in [(1000, 0), (1700, 0), (1280, 4), (1280, 30)].into_iter().enumerate() {
            assert_eq!(call_look(&mut isl, at, false, t + ms(400 * i as u64), || false), None, "{at:?}");
        }
        assert!(!isl.stow.called);
    }

    // A button held there is a press on what is under it (a tab, a window
    // dragged up): nothing until the pointer has left the edge and come back.
    #[test]
    fn a_press_at_the_edge_does_not() {
        let mut isl = stowed();
        let t = Instant::now();
        assert_eq!(call_look(&mut isl, (1280, 0), false, t, || false), None);
        assert_eq!(call_look(&mut isl, (1280, 0), false, t + ms(100), || true), None);
        assert_eq!(call_look(&mut isl, (1280, 0), false, t + ms(2000), || false), None);
        assert_eq!(call_look(&mut isl, (1280, 200), false, t + ms(2100), || false), None);
        assert_eq!(call_look(&mut isl, (1280, 0), false, t + ms(2200), || false), None);
        assert_eq!(call_look(&mut isl, (1280, 0), false, t + ms(2200 + EDGE_MS as u64), || false), Some(true));
    }

    // Called, it stays while the pointer is about it (above it, on it, a
    // little below it) and goes once the pointer has been away a while.
    #[test]
    fn it_goes_once_the_pointer_has_gone() {
        let mut isl = stowed();
        isl.stow.called = true;
        isl.stow.stowed = false;
        let t = Instant::now();
        for (i, at) in [(1280, 0), (1100, 30), (1280, 60), (1480, 20)].into_iter().enumerate() {
            assert_eq!(call_look(&mut isl, at, false, t + ms(1000 * i as u64), || false), None, "{at:?}");
        }
        let t = t + ms(5000);
        assert_eq!(call_look(&mut isl, (1280, 400), false, t, || false), None);
        // Over the open island, below the closed one's band: still wanted.
        isl.ct.over = true;
        assert_eq!(call_look(&mut isl, (1280, 400), false, t + ms(LEAVE_MS as u64 + 100), || false), None);
        isl.ct.over = false;
        assert_eq!(call_look(&mut isl, (1280, 400), false, t + ms(LEAVE_MS as u64 + 200), || false), None);
        assert_eq!(call_look(&mut isl, (1280, 400), false, t + ms(2 * LEAVE_MS as u64 + 200), || false), Some(false));
        assert!(!isl.stow.called);
    }

    // The pointer on the island (down for a nudge, a prompt) keeps it there
    // once it closes; with no window under it any more, nothing is called.
    #[test]
    fn on_it_or_nothing_under_it() {
        let mut isl = stowed();
        isl.stow.stowed = false;
        isl.ct.over = true;
        let t = Instant::now();
        assert_eq!(call_look(&mut isl, (1280, 20), false, t, || false), Some(true));
        isl.stow.under = false;
        assert_eq!(call_look(&mut isl, (1280, 20), false, t + ms(10), || false), Some(false));
        assert_eq!(call_look(&mut isl, (1280, 20), false, t + ms(20), || false), None);
        // She dragged past: none of it.
        let mut isl = stowed();
        assert_eq!(call_look(&mut isl, (1280, 0), true, t, || false), None);
        assert_eq!(call_look(&mut isl, (1280, 0), true, t + ms(2000), || false), None);
    }
}

#[cfg(test)]
mod strip_tests {
    use super::{thickness, window_at, Edge, SIDE_ROOM, TASKBAR_H, TASKBAR_ROOM};

    // At every scale, a strip as Windows grants it (its room, a pixel more
    // or less than ours as it rounds, or our own bar's): the window's outer
    // edge on the strip's, and the page's strip (its CSS px times the scale)
    // exactly as thick, so its inner edge is the work area's.
    #[test]
    fn the_strip_and_the_window_meet_at_any_scale() {
        let mon = (0, 0, 3840, 2160);
        for sf in [1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 1.37] {
            let ours = (TASKBAR_H * sf).round() as i32;
            for edge in [Edge::Bottom, Edge::Top, Edge::Left, Edge::Right] {
                for t in [ours - 1, ours, ours + 1] {
                    let b = edge.strip(mon, t);
                    let size = if edge.is_side() { (((TASKBAR_H + SIDE_ROOM) * sf).round() as i32, mon.3) } else { (mon.2, ((TASKBAR_H + TASKBAR_ROOM) * sf).round() as i32) };
                    let (x, y) = window_at(edge, b, size);
                    // The window's outer edge on the strip's, the strip inside it.
                    match edge {
                        Edge::Bottom => assert_eq!(y + size.1, b.1 + b.3, "{sf} {edge:?} {t}"),
                        Edge::Top => assert_eq!(y, b.1, "{sf} {edge:?} {t}"),
                        Edge::Left => assert_eq!(x, b.0, "{sf} {edge:?} {t}"),
                        Edge::Right => assert_eq!(x + size.0, b.0 + b.2, "{sf} {edge:?} {t}"),
                    }
                    assert!(x <= b.0 && y <= b.1 && x + size.0 >= b.0 + b.2 && y + size.1 >= b.1 + b.3, "{sf} {edge:?} {t}");
                    // The page's strip: thick / scale CSS px, back to exactly thick.
                    let thick = thickness(edge, b);
                    assert_eq!(thick, t);
                    assert_eq!(((thick as f64 / sf) * sf).round() as i32, t, "{sf} {edge:?} {t}");
                }
            }
        }
    }
}
