// Her home's window (the page draws it, src/pet/island.ts), by display:
//   island  a black pill at the top centre, or at a top corner (islandPos,
//           zhoupengjie/Wakuwaku#59, see fit); or along the foot of the
//           screen, laid over Windows' own taskbar, which stays as it is
//           (islandEdge taskbar, zhoupengjie/Wakuwaku#105), or, there, its
//           capsule inside the taskbar itself (islandEmbed, embed.rs), this
//           window only for what opens from it
//   taskbar a strip along the bottom in place of Windows' own, which is put
//           away while it is up, its tray taken over (taskbar/); what
//           opens grows up from it, into room kept above it (appbar.rs)
// Both stay up while she is out, and do the talking.
//
// Pulled out of her home, she becomes the pet window (carried under the
// cursor while the button is still held on the home); brought close to it
// again, it reaches for her and takes her back.
//
// Sizes here are logical (the page's) unless they say physical.
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc};

use serde_json::{json, Value};
use tauri::utils::config::BackgroundThrottlingPolicy;
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

use crate::pointer::{ClickThrough, Mode};
use crate::screen::Area;
use crate::appbar::Edge;
use crate::{appbar, taskbar, Shared};
use crate::lock::Lock;

// The window without extra room, by home (island.ts BASE): the island as
// wide as pulling her out needs, so it only ever grows down. A window whose
// top-left corner moves shows its old picture from there for a frame or two,
// so these never move it. The page asks for more (pet:panel) when a shape
// needs it.
const ISLAND: (f64, f64) = (760.0, 132.0);
// Along the foot of the screen (islandEdge taskbar, zhoupengjie/Wakuwaku#105):
// the island at the window's foot, what opens from it growing up into the
// room above it, as the taskbar's does. So the window is as tall as the
// taskbar's strip and room together and never grows (growing up would move
// its top-left corner); the page keeps what opens within it (island.ts
// openMaxH, settingsMax). Only its width may grow, as at the top.
const ISLAND_BOTTOM: (f64, f64) = (ISLAND.0, TASKBAR_H + TASKBAR_ROOM);
// The taskbar's strip (as high as Windows' own, so the Start menu and the
// quick settings open just above it), and the room above it for what opens
// (a window growing up moves its top-left corner). The window is the two.
pub const TASKBAR_H: f64 = 48.0;
// Split in two (taskbarEdge split): the top bar thinner by default, as a
// menu bar (splitBar 32; 48 as Windows'), docs/design/specs/dock-topbar.md.
pub const SLIM_H: f64 = 32.0;
// As high as the settings ever are (they stand above the strip, island.ts
// settingsMax): opening them never moves the window, which would flash.
const TASKBAR_ROOM: f64 = 640.0;
// Beside a side bar (taskbarEdge left or right): as wide as the settings
// ever are (the Mail page in two panes, settings-mail.ts WIDE_W), their gap off
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
    // the room Windows' own taskbar keeps (taskbar/win/hold.rs explorer_room).
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
    // The taskbar's layout being edited (island.ts editTaskbar): her menu
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
    // anchor.ts capsuleWidth), and whether it is in. The window is laid
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
// page to lay the island about (island-layout.ts placeMid).
fn about(area: &Area, cap: (i32, i32, i32, i32), width: i32) -> (i32, i32) {
    let mid = cap.0 + cap.2 / 2;
    let x = (mid - width / 2).clamp(area.x, (area.x + area.w - width).max(area.x));
    (x, mid - x)
}

// Her island at the top of the screen, up out of the way of the window
// under it (look_under, call_look; island-plate.ts setStow).
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
    // island lies (the page's px; island.ts placeAnchor, anchor.ts foot):
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
        crate::embed::is_window(crate::main_wait::hwnd(&win).unwrap_or(0))
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
    // Nor while another copy of her holds the taskbar (taskbar/win/hold.rs waiting).
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
    Edge::of(sh.config().taskbar_edge.as_deref().unwrap_or("bottom"))
}

// The top bar and the dock (display split, a home beside the island and the
// taskbar, docs/design/specs/dock-topbar.md): her taskbar split in two, a
// top bar (the strip, here) and a dock at the foot with Start and the
// programs (bars.rs); a taskbar as her windows have it (Shared::home).
pub fn is_split(sh: &Shared) -> bool {
    sh.config().display.as_deref() == Some("split")
}

// The strip's thickness (logical): Windows' own, or split, the top bar's
// (splitBar: 32 unless set to 48).
pub fn strip_h(sh: &Shared) -> f64 {
    if is_split(sh) && sh.config().split_bar != Some(48) { SLIM_H } else { TASKBAR_H }
}

// Split, the dock kept in the room Windows' own keeps along the foot
// (splitDock room), maximized windows stopping at it; else (hide, the
// default) it keeps none and steps out of the way of a window touching it.
pub fn dock_keeps_room(sh: &Shared) -> bool {
    is_split(sh) && sh.config().split_dock.as_deref() == Some("room")
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
    let key = sh.config().home_display.clone();
    screens.by_key(key.as_str().filter(|k| !k.is_empty())?)
}

// Her display's scale: the chosen one's, else the primary's (taskbar/
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
    let wanted = Side::of(sh.config().island_pos.as_deref().unwrap_or("center"));
    // In Windows' own taskbar instead of at the top of the screen (islandEdge
    // taskbar, #105): along its edge, the bottom or the top, while Explorer's
    // taskbar is shown there; along the foot of the work area with none.
    let in_bar = home != "taskbar" && sh.config().island_edge.as_deref() == Some("taskbar");
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
// taskbar), for the page (shared.rs payload islandAt).
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

// Its parts, each in a file of its own; named here for the others (each
// has `use super::*`) and, what other modules call, for the crate.
mod front;
mod handoff;
mod pointer;
mod stow;
mod strip;
mod window;
pub use front::*;
pub use handoff::*;
pub use pointer::*;
pub use stow::*;
pub use strip::*;
pub use window::*;

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
