// Widgets on the desktop (desk.js): each container of them ("desktop"
// setting) its own small window, as big as it is, above the desktop's
// icons and below every program's window, never taking the focus. Shown
// the desktop (Win+D), they come above it; a program to the front again,
// they go back under it (a hook on the front window, as Rainmeter does: not
// made a child of Explorer's desktop, whose input it would then share).
//
// A container: { id, x, y (physical, its top-left), mode (grid: in columns,
// stack: one at a time), cols (1 to 6), style (glass, clear), items (the
// widgets, as the taskbar's and her island's grid have them, L too) }.
// Edited with the taskbar (desk_edit): a head to move it by (snapped to
// 16 px), its mode, columns, ground, off; a library card let go over the
// desktop makes one there (desk_drop), or goes into the one it is over.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::Shared;

const PREFIX: &str = "desk-";
// The snap, in px at the display's scale.
const SNAP: f64 = 16.0;

// The desktop as it was when editing began (a cancel puts it back), and
// each window's handle (for its place among the windows).
static EDITING: Mutex<Option<Value>> = Mutex::new(None);
static HWNDS: Mutex<Vec<(String, isize)>> = Mutex::new(Vec::new());
// A window moved by its head: where to, kept a moment after it stops.
static MOVED: Mutex<Option<HashMap<String, (i32, i32, u64)>>> = Mutex::new(None);

pub fn label(id: &str) -> String {
    format!("{PREFIX}{id}")
}

fn boxes(sh: &Shared) -> Vec<Value> {
    sh.setting("desktop").as_array().cloned().unwrap_or_default()
}

pub fn is_editing() -> bool {
    EDITING.lock().unwrap().is_some()
}

// --- Its windows ---------------------------------------------------------------------

// A window for each container there is; those of containers gone closed.
pub fn sync(sh: &Arc<Shared>) {
    let boxes = boxes(sh);
    let ids: Vec<String> = boxes.iter().filter_map(|b| b["id"].as_str().map(str::to_string)).collect();
    for (label, win) in sh.app.webview_windows() {
        if let Some(id) = label.strip_prefix(PREFIX) {
            if !ids.iter().any(|x| x == id) {
                HWNDS.lock().unwrap().retain(|(l, _)| *l != label);
                let _ = win.destroy();
            }
        }
    }
    for b in boxes {
        let Some(id) = b["id"].as_str() else { continue };
        if sh.app.get_webview_window(&label(id)).is_none() {
            create(sh, &b);
        }
    }
    push(sh);
}

// Where a container goes: its corner, kept on a display's work area.
fn spot(sh: &Shared, b: &Value) -> Option<(i32, i32, f64)> {
    let (x, y) = (b["x"].as_i64()? as i32, b["y"].as_i64()? as i32);
    let area = sh.screens().near(x, y)?;
    let x = x.clamp(area.x, (area.x + area.w - 48).max(area.x));
    let y = y.clamp(area.y, (area.y + area.h - 48).max(area.y));
    Some((x, y, area.sf))
}

// Made off the main thread (its events wait on the main thread), hidden
// until its page says how big it is (desk_size).
fn create(sh: &Arc<Shared>, b: &Value) {
    let Some(id) = b["id"].as_str().map(str::to_string) else { return };
    let Some((x, y, sf)) = spot(sh, b) else { return };
    let sh = sh.clone();
    std::thread::spawn(move || {
        let label = label(&id);
        let built = WebviewWindowBuilder::new(&sh.app, &label, WebviewUrl::App(format!("pet/desk.html?box={id}").into()))
            .title("wakuwaku desktop")
            .inner_size(120.0, 120.0)
            .additional_browser_args(crate::BROWSER_ARGS)
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
        match built {
            Ok(win) => {
                let _ = win.set_position(PhysicalPosition::new(x, y));
                #[cfg(windows)]
                if let Ok(hwnd) = win.hwnd() {
                    HWNDS.lock().unwrap().push((label.clone(), hwnd.0 as isize));
                }
                sh.own_window(&win);
                crate::crash::watch(&sh, &win);
                // Moved by its head: snapped and kept once it stops.
                let moved = sh.clone();
                let id = id.clone();
                win.on_window_event(move |event| {
                    if let WindowEvent::Moved(at) = event {
                        if is_editing() {
                            moved_to(&moved, &id, at.x, at.y);
                        }
                    }
                });
                imp::start_watch();
                sh.log(&format!("desk: window for {label} at ({x}, {y}), scale {sf}"));
            }
            Err(err) => sh.log(&format!("desk: could not make a window: {err}")),
        }
    });
}

// A container's window moved by its head: kept 0.4 s after the last move,
// snapped to the display's 16 px.
fn moved_to(sh: &Arc<Shared>, id: &str, x: i32, y: i32) {
    let at = crate::now_ms();
    MOVED.lock().unwrap().get_or_insert_with(HashMap::new).insert(id.to_string(), (x, y, at));
    let sh = sh.clone();
    let id = id.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(400));
        let Some((x, y, when)) = MOVED.lock().unwrap().as_ref().and_then(|m| m.get(&id).copied()) else { return };
        if when != at {
            return;
        }
        MOVED.lock().unwrap().as_mut().map(|m| m.remove(&id));
        let Some(area) = sh.screens().near(x, y) else { return };
        let step = SNAP * area.sf;
        let snap = |v: i32, from: i32| from + ((v - from) as f64 / step).round() as i32 * step as i32;
        let (sx, sy) = (snap(x, area.x), snap(y, area.y));
        if let Some(win) = sh.app.get_webview_window(&label(&id)) {
            if (sx, sy) != (x, y) {
                let _ = win.set_position(PhysicalPosition::new(sx, sy));
            }
        }
        change_box(&sh, &id, |b| {
            b["x"] = json!(sx);
            b["y"] = json!(sy);
        });
    });
}

// One container changed and kept (none left with no widgets: it goes).
fn change_box(sh: &Arc<Shared>, id: &str, f: impl FnOnce(&mut Value)) {
    let mut all = boxes(sh);
    if let Some(b) = all.iter_mut().find(|b| b["id"] == id) {
        f(b);
    }
    all.retain(|b| b["items"].as_array().is_some_and(|i| !i.is_empty()));
    keep(sh, all);
}

fn keep(sh: &Arc<Shared>, all: Vec<Value>) {
    let patch = json!({ "desktop": all });
    crate::settings::apply_patch(sh, &patch);
    sync(sh);
}

// --- What its pages are told ------------------------------------------------------------

// What every container's page draws from: her language, the specifics
// shown or not, every widget with something to say, what the tools read,
// the notes and colours kept, whether the desktop is being edited.
fn state(sh: &Shared) -> Value {
    json!({
        "lang": sh.lang(),
        "details": sh.setting("details") != false,
        "boxes": sh.setting("desktop"),
        "widgets": sh.widgets_said(),
        "tools": crate::tools::state(),
        "notes": sh.setting("notes"),
        "picked": sh.setting("pickedColors"),
        "editing": is_editing(),
    })
}

pub fn push(sh: &Shared) {
    let labels: Vec<String> = HWNDS.lock().unwrap().iter().map(|(l, _)| l.clone()).collect();
    if labels.is_empty() {
        return;
    }
    let now = state(sh);
    for l in labels {
        let _ = sh.app.emit_to(l.as_str(), "desk:update", now.clone());
    }
}

// An event of the tools' (tools.rs) told the containers' pages too.
pub fn tell(sh: &Shared, event: &str, payload: &Value) {
    for (l, _) in HWNDS.lock().unwrap().iter() {
        let _ = sh.app.emit_to(l.as_str(), event, payload.clone());
    }
}

// --- What its pages (and the island's) ask -------------------------------------------------

#[tauri::command]
pub fn desk_state(app: tauri::AppHandle) -> Value {
    state(&crate::shared(&app))
}

// The page drew itself: its window as big (CSS px), shown, under the programs.
#[tauri::command]
pub fn desk_size(app: tauri::AppHandle, id: String, w: f64, h: f64) {
    let sh = crate::shared(&app);
    let Some(win) = sh.app.get_webview_window(&label(&id)) else { return };
    let sf = win.scale_factor().unwrap_or(1.0);
    let (w, h) = ((w.clamp(40.0, 2000.0) * sf).round() as u32, (h.clamp(40.0, 2000.0) * sf).round() as u32);
    let _ = win.set_size(PhysicalSize::new(w, h));
    if !win.is_visible().unwrap_or(false) {
        let _ = win.show();
        if let Ok(hwnd) = win.hwnd() {
            imp::under_programs(hwnd.0 as isize);
        }
    }
}

// A container changed by its page (its mode, columns, ground, widgets).
#[tauri::command]
pub fn desk_change(app: tauri::AppHandle, id: String, change: Value) {
    let sh = crate::shared(&app);
    change_box(&sh, &id, |b| {
        for key in ["mode", "cols", "style", "items"] {
            if let Some(v) = change.get(key) {
                b[key] = v.clone();
            }
        }
    });
}

#[tauri::command]
pub fn desk_remove(app: tauri::AppHandle, id: String) {
    let sh = crate::shared(&app);
    let all = boxes(&sh).into_iter().filter(|b| b["id"] != id.as_str()).collect();
    keep(&sh, all);
}

// Moved by its head: the system's own move (desk_moved keeps it).
#[tauri::command]
pub fn desk_move(app: tauri::AppHandle, id: String) {
    if !is_editing() {
        return;
    }
    if let Some(win) = app.get_webview_window(&label(&id)) {
        let _ = win.start_dragging();
    }
}

// Editing begins (the taskbar's), or ends: kept, or put back as it was.
#[tauri::command]
pub fn desk_edit(app: tauri::AppHandle, on: bool, keep_it: bool) {
    let sh = crate::shared(&app);
    if on {
        *EDITING.lock().unwrap() = Some(sh.setting("desktop"));
    } else if let Some(was) = EDITING.lock().unwrap().take() {
        if !keep_it {
            crate::settings::apply_patch(&sh, &json!({ "desktop": was }));
        }
        sync(&sh);
    }
    push(&sh);
}

// A library card let go over the desktop (the island's page, its CSS px):
// into the container it is over, or a new one there.
#[tauri::command]
pub fn desk_drop(app: tauri::AppHandle, entry: Value, x: f64, y: f64) -> bool {
    let sh = crate::shared(&app);
    let (pos, sf) = crate::island::origin(&sh);
    let (px, py) = (pos.0 + (x * sf).round() as i32, pos.1 + (y * sf).round() as i32);
    let mut all = boxes(&sh);
    let over = HWNDS.lock().unwrap().iter().find(|(_, h)| imp::contains(*h, px, py)).map(|(l, _)| l[PREFIX.len()..].to_string());
    if let Some(id) = over {
        if let Some(b) = all.iter_mut().find(|b| b["id"] == id.as_str()) {
            if let Some(items) = b["items"].as_array_mut() {
                items.push(entry);
            }
        }
    } else {
        let Some(area) = sh.screens().near(px, py) else { return false };
        let step = SNAP * area.sf;
        let snap = |v: i32, from: i32| from + ((v - from) as f64 / step).round() as i32 * step as i32;
        let large = matches!(entry["size"].as_str(), Some("M" | "L"));
        all.push(json!({
            "id": new_id(),
            "x": snap(px - (40.0 * area.sf) as i32, area.x),
            "y": snap(py - (40.0 * area.sf) as i32, area.y),
            "mode": "grid",
            "cols": if large { 2 } else { 1 },
            "style": "glass",
            "items": [entry],
        }));
    }
    let before = sh.setting("desktop");
    keep(&sh, all);
    sh.setting("desktop") != before
}

// A container's id: "w" and a-z 0-9, as the widgets' are.
fn new_id() -> String {
    let n = crate::now_ms() ^ (std::process::id() as u64).rotate_left(20);
    let mut s = String::from("wd");
    let mut v = n;
    while v > 0 && s.len() < 14 {
        let d = (v % 36) as u32;
        s.push(std::char::from_digit(d, 36).unwrap_or('0'));
        v /= 36;
    }
    s
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::HWNDS;

    type Hwnd = *mut c_void;
    type WinEventProc = extern "system" fn(isize, u32, Hwnd, i32, i32, u32, u32);

    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    // As taskbar.rs has it (one declaration of GetMessageW for both).
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

    #[allow(clashing_extern_declarations)]
    #[link(name = "user32")]
    extern "system" {
        fn FindWindowW(class: *const u16, title: *const u16) -> Hwnd;
        fn FindWindowExW(parent: Hwnd, after: Hwnd, class: *const u16, title: *const u16) -> Hwnd;
        fn GetWindow(hwnd: Hwnd, cmd: u32) -> Hwnd;
        fn SetWindowPos(hwnd: Hwnd, after: isize, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
        fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
        fn IsWindowVisible(hwnd: Hwnd) -> i32;
        fn GetClassNameW(hwnd: Hwnd, name: *mut u16, max: i32) -> i32;
        fn SetWinEventHook(min: u32, max: u32, module: isize, func: WinEventProc, pid: u32, tid: u32, flags: u32) -> isize;
        fn GetMessageW(msg: *mut Msg, hwnd: Hwnd, min: u32, max: u32) -> i32;
    }

    const GW_HWNDPREV: u32 = 3;
    const SWP_NOSIZE: u32 = 0x1;
    const SWP_NOMOVE: u32 = 0x2;
    const SWP_NOACTIVATE: u32 = 0x10;
    const HWND_TOP: isize = 0;
    const EVENT_SYSTEM_FOREGROUND: u32 = 0x3;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    fn class_of(hwnd: Hwnd) -> String {
        let mut buf = [0u16; 64];
        // SAFETY: our own buffer, its length given.
        let n = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }

    // The desktop's icons' window: Progman, or the WorkerW Explorer moved
    // SHELLDLL_DefView into (a picture slideshow, a live wallpaper).
    fn icons_host() -> Hwnd {
        // SAFETY: class names of our own; windows Explorer made, read only.
        unsafe {
            let progman = FindWindowW(wide("Progman").as_ptr(), std::ptr::null());
            if !progman.is_null() && !FindWindowExW(progman, std::ptr::null_mut(), wide("SHELLDLL_DefView").as_ptr(), std::ptr::null()).is_null() {
                return progman;
            }
            let mut w = std::ptr::null_mut();
            loop {
                w = FindWindowExW(std::ptr::null_mut(), w, wide("WorkerW").as_ptr(), std::ptr::null());
                if w.is_null() {
                    return progman;
                }
                if !FindWindowExW(w, std::ptr::null_mut(), wide("SHELLDLL_DefView").as_ptr(), std::ptr::null()).is_null() {
                    return w;
                }
            }
        }
    }

    // Just above the desktop's icons, under every program's window.
    pub fn under_programs(hwnd: isize) {
        let host = icons_host();
        if host.is_null() {
            return;
        }
        // SAFETY: window handles and flags; nothing pointed to.
        unsafe {
            let above = GetWindow(host, GW_HWNDPREV);
            if above as isize == hwnd {
                return;
            }
            // Put after (under) the window just above the desktop: on it.
            let after = if above.is_null() { HWND_TOP } else { above as isize };
            SetWindowPos(hwnd as Hwnd, after, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
    }

    // Over every program's window (the desktop shown).
    fn over_programs(hwnd: isize) {
        // SAFETY: as above.
        unsafe { SetWindowPos(hwnd as Hwnd, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
    }

    pub fn contains(hwnd: isize, x: i32, y: i32) -> bool {
        let mut r = Rect::default();
        // SAFETY: our own struct; a gone window fails both.
        unsafe { IsWindowVisible(hwnd as Hwnd) != 0 && GetWindowRect(hwnd as Hwnd, &mut r) != 0 && x >= r.left && x < r.right && y >= r.top && y < r.bottom }
    }

    // The front window changing: the desktop (Win+D, a click on it) brings
    // the containers over the programs; a program, back under them.
    extern "system" fn front(_hook: isize, _event: u32, hwnd: Hwnd, _object: i32, _child: i32, _thread: u32, _time: u32) {
        let class = class_of(hwnd);
        let desktop = class == "WorkerW" || class == "Progman";
        let ours: Vec<isize> = HWNDS.lock().unwrap().iter().map(|(_, h)| *h).collect();
        if ours.contains(&(hwnd as isize)) {
            return;
        }
        for h in ours {
            if desktop {
                over_programs(h);
            } else {
                under_programs(h);
            }
        }
    }

    static WATCHING: AtomicBool = AtomicBool::new(false);

    // Once: a thread with the hook on the front window, and its messages.
    pub fn start_watch() {
        if WATCHING.swap(true, Ordering::SeqCst) {
            return;
        }
        crate::spawn_named("desk watch", || {
            // SAFETY: the hook on this thread, out of context (called back
            // here, through this thread's messages, which it pumps for ever).
            unsafe {
                SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, 0, front, 0, 0, 0);
                let mut msg: Msg = std::mem::zeroed();
                while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {}
            }
        });
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn under_programs(_hwnd: isize) {}
    pub fn contains(_hwnd: isize, _x: i32, _y: i32) -> bool {
        false
    }
    pub fn start_watch() {}
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_containers_id_is_a_widgets() {
        let id = super::new_id();
        assert!(id.starts_with('w') && id.len() <= 16 && id[1..].chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()), "{id}");
    }
}
