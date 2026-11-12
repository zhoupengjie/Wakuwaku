// What the desktop's pages ask (desk_* commands).
use super::*;

pub(super) fn shared_of(app: &tauri::AppHandle) -> Arc<Shared> {
    crate::shared(app)
}

// Its listeners up: told what to show, and its display's picture.
#[tauri::command]
pub async fn desk_ready(app: tauri::AppHandle, window: WebviewWindow) {
    count(&COMMANDS);
    let sh = shared_of(&app);
    let key = edit(|desks| {
        desks.iter_mut().find(|d| d.label == window.label()).map(|d| {
            d.ready = true;
            d.key.clone()
        })
    });
    // A page come up (again: reloaded) has been told nothing.
    if key.is_some() {
        told(window.label(), |t| *t = Told { label: t.label.clone(), ..Told::default() });
    }
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
    edit(|desks| {
        if let Some(d) = desks.iter_mut().find(|d| d.label == window.label()) {
            d.shapes = shapes.into_iter().take(64).collect();
            d.menu = menu;
        }
    });
}

// A press held (a drag): the window keeps the mouse till it is let go (the
// pointer's next look sees it held; let go, it looks as ever).
#[tauri::command]
pub fn desk_hold(window: WebviewWindow, on: bool) {
    count(&COMMANDS);
    let Some(hwnd) = edit(|desks| {
        desks.iter_mut().find(|d| d.label == window.label()).map(|d| {
            d.held = on;
            d.hwnd
        })
    }) else {
        return;
    };
    // Taken at once, not at the pointer's next look (set_through does
    // nothing to a window that already takes it). A command without async
    // runs on the main thread, the window's: set here.
    if on {
        imp::set_through(hwnd, false);
    }
}

// A board's name being typed (desk.ts rename): the window takes the keyboard
// for it (it never takes the focus otherwise: WS_EX_NOACTIVATE), and lets go
// of it after (Enter, Esc, or a press elsewhere): the window that had it gets
// it back, and this one goes back down right over the desktop.
#[tauri::command]
pub fn desk_type(app: tauri::AppHandle, window: WebviewWindow, on: bool) {
    count(&COMMANDS);
    let sh = shared_of(&app);
    #[cfg(windows)]
    if let Some(h) = crate::main_wait::hwnd(&window) {
        imp::take_keys(h, on);
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
    let old = sh.config().desk_layout.clone();
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
    let (mon, sf) = desks().iter().find(|d| d.label == window.label()).map(|d| (d.mon, d.sf))?;
    let _ = app;
    let icons = tauri::async_runtime::spawn_blocking(move || imp::icons(mon, sf)).await.ok().flatten();
    // Kept for the pointer's look too: Ctrl over an icon is Explorer's.
    if let Some(icons) = &icons {
        keep_icons(window.label(), icons.clone());
    }
    icons
}

// Ctrl and the wheel over its bare desktop (desk.ts): the window takes the
// mouse there while Ctrl is held (poll), so Explorer heard nothing of it.
// Handed on to the desktop's icon view as Windows would have sent it
// (WM_MOUSEWHEEL, Ctrl among its keys, the same turn, where the pointer is,
// the screen's px): its icons larger or smaller, as without her.
#[tauri::command]
pub fn desk_wheel(window: WebviewWindow, x: f64, y: f64, delta: i32) {
    count(&COMMANDS);
    let Some((mon, sf)) = desks().iter().find(|d| d.label == window.label()).map(|d| (d.mon, d.sf)) else { return };
    if !(x.is_finite() && y.is_finite()) || delta == 0 {
        return;
    }
    imp::wheel_to_icons(mon.0 + (x * sf).round() as i32, mon.1 + (y * sf).round() as i32, delta.clamp(-1200, 1200));
}

// Some of its widgets picked with Ctrl, or none any more (desk.ts): a press
// off them is looked for (poll), and told (desk:outside).
#[tauri::command]
pub fn desk_picked(window: WebviewWindow, on: bool) {
    count(&COMMANDS);
    edit(|desks| {
        if let Some(d) = desks.iter_mut().find(|d| d.label == window.label()) {
            d.picked = on;
        }
    });
}

// The icons' rects (x, y, w, h: the screen's px) that lie on a display, in
// its window's px (deskgrid.ts takes them as blocks to keep clear of).
#[cfg_attr(not(windows), allow(dead_code))]
pub(super) fn blocks_on(rects: &[(i32, i32, i32, i32)], mon: Mon, sf: f64) -> Vec<[f64; 4]> {
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
    let Some((mon, sf)) = desks().iter().find(|d| d.label == window.label()).map(|d| (d.mon, d.sf)) else { return Value::Null };
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

