// The taskbar home's side of Windows (display 'taskbar'): Windows' own
// taskbar put away while hers is up (shell.rs), and the tray's icons taken
// over from Explorer (systray.rs). On a thread of their own, which runs the
// message loop the tray's window needs, with a hidden window of its own that
// hears:
//   - Explorer started again (TaskbarCreated, from a new taskbar window):
//     its taskbar put away again, the strip taken again (island.rs), the
//     tray put first again and programs asked for their icons, once the new
//     Explorer has settled (zhoupengjie/Wakuwaku#85: until then hers is set
//     behind its tray; nor is the taskbar taken from an Explorer still
//     starting)
//   - Explorer's taskbar shown (a WinEvent): put away again at once
//   - the tray's icons changing: drawn into PNGs and sent to the island's
//     page (taskbar:tray)
//   - windows coming, going, renamed, brought to the front, minimized, and
//     a window flashing for attention (the shell hook): the buttons' list
//     (tasks.rs) made again on a thread of its own and sent (taskbar:windows),
//     each with the sessions that run in it (jump.rs)
//   - the desktop's picture changing (wallpaper.rs): sent for the strip's
//     Mica (taskbar:wallpaper); Windows' mode and accent colour, for the
//     strip to be as Windows' own taskbar is (taskbar:look)
// Taken when the home takes its strip, given back when it gives the strip
// back (island.rs sync_bar) and on quitting. A killed process cannot give
// the taskbar back: shell.rs's guard does.
//
// Learnt in examples/taskbar_spike.rs (2026-10-10), where each of these was
// tried first.
use crate::Shared;

// On Windows its parts are in win/ (listed in win/mod.rs); elsewhere none of
// it does anything (imp below).
#[cfg(windows)]
mod win;
#[cfg(windows)]
use win as imp;

#[cfg(not(windows))]
mod imp {
    use crate::Shared;

    pub fn is_up() -> bool {
        false
    }
    pub fn handing_back() {}
    pub fn claim<R>(f: impl FnOnce() -> R) -> R {
        f()
    }
    pub fn set_edge(_edge: crate::appbar::Edge, _room: bool) {}
    pub fn start_open() -> bool {
        false
    }
    pub fn close_flyout() -> bool {
        false
    }
    pub fn keys_now() -> serde_json::Value {
        serde_json::Value::Null
    }
    pub fn recycle_bin() -> serde_json::Value {
        serde_json::Value::Null
    }
    pub fn dock_thumbs(_items: Vec<(isize, (i32, i32, i32, i32))>, _area: Option<(i32, i32, i32, i32)>) {}
    pub fn dock_badges(_items: Vec<(isize, (i32, i32, i32, i32))>) {}
    pub fn thumbs_on_top() {}
    pub fn dock_thumbs_over(_dock: isize) {}
    pub fn make_dock_host(_owner: isize) {}
    pub fn drop_dock_host() {}
    pub fn take(_sh: &Shared, _strip: (i32, i32, i32, i32)) -> bool {
        true
    }
    pub fn look_for_other(_sh: &Shared) {}
    pub fn waiting() -> bool {
        false
    }
    pub fn explorer_room(_mon: (i32, i32, i32, i32)) -> Option<(i32, i32, i32, i32)> {
        None
    }
    pub fn give_back(_sh: &Shared) {}
    pub fn strip_moved(_strip: (i32, i32, i32, i32)) {}
    pub fn resend(_sh: &Shared) {}
    pub fn wake() {}
    pub fn look_now() {}
    pub fn paper_set() {}
    pub fn look_value() -> serde_json::Value {
        serde_json::Value::Null
    }
    pub fn windows_now() -> serde_json::Value {
        serde_json::Value::Null
    }
    pub fn press(_sh: &Shared, _key: u64, _press: &str, _home_hwnd: isize) -> bool {
        false
    }
    pub fn open(_what: &str) -> bool {
        false
    }
    pub fn volume(_steps: i32) -> bool {
        false
    }
    pub fn windows(_hwnds: Vec<isize>) -> bool {
        false
    }
    pub fn window_menu(_hwnds: Vec<isize>, _lang: &'static str) -> bool {
        false
    }
    pub fn watch_presses(_on: bool) {}
    pub fn note(_line: &str) {}
    pub fn end_task_on() -> bool {
        false
    }
    pub fn window(_what: &str, _hwnd: isize) -> bool {
        false
    }
    pub fn app(_sh: &Shared, _what: &str, _path: &str, _id: Option<&str>) -> bool {
        false
    }
    pub fn thumbs(_items: Vec<(isize, (i32, i32, i32, i32))>) {}
    pub fn work_area_bottom() -> i32 {
        0
    }
    pub fn work_area_rect() -> (i32, i32, i32, i32) {
        (0, 0, 0, 0)
    }
    pub fn strip_picture(_mon: (i32, i32, i32, i32)) -> Option<String> {
        None
    }
}

pub use imp::{claim, close_flyout, dock_badges, dock_thumbs, dock_thumbs_over, drop_dock_host, make_dock_host, thumbs_on_top, end_task_on, explorer_room, give_back, handing_back, is_up, keys_now, recycle_bin, set_edge, start_open, look_for_other, look_now, look_value, note, paper_set, resend, strip_moved, strip_picture, take, waiting, wake, windows_now, work_area_bottom, work_area_rect};

// A number Windows keeps for a setting of its own (end_task_on, seconds_on):
// on once set, but for 0.
#[cfg_attr(not(windows), allow(dead_code))]
fn set_on(value: Option<u32>) -> bool {
    value.is_some_and(|v| v != 0)
}

// RGBA pixels (sw × sh) made dw × dh smoothly: each pixel out the weighted
// mean of those it covers, under a tent as wide as a pixel of the coarser
// of the two (bilinear growing, area-like shrinking), on colour premultiplied
// by its alpha so edges keep no dark fringe.
#[cfg_attr(not(windows), allow(dead_code))]
fn resize(src: &[u8], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<u8> {
    let pre: Vec<[f32; 4]> = src
        .chunks_exact(4)
        .map(|p| {
            let a = p[3] as f32 / 255.0;
            [p[0] as f32 * a, p[1] as f32 * a, p[2] as f32 * a, p[3] as f32]
        })
        .collect();
    // For each pixel out along one side: the pixels in, and their weights.
    let weights = |s: usize, d: usize| -> Vec<Vec<(usize, f32)>> {
        let scale = s as f32 / d as f32;
        let radius = scale.max(1.0);
        (0..d)
            .map(|i| {
                let centre = (i as f32 + 0.5) * scale;
                let lo = (centre - radius).floor().max(0.0) as usize;
                let hi = ((centre + radius).ceil() as usize).min(s);
                let mut ws: Vec<(usize, f32)> = (lo..hi).map(|j| (j, 1.0 - ((j as f32 + 0.5 - centre).abs() / radius))).filter(|w| w.1 > 0.0).collect();
                let sum: f32 = ws.iter().map(|w| w.1).sum();
                if sum > 0.0 {
                    ws.iter_mut().for_each(|w| w.1 /= sum);
                } else {
                    ws = vec![((centre as usize).min(s - 1), 1.0)];
                }
                ws
            })
            .collect()
    };
    let (across, down) = (weights(sw, dw), weights(sh, dh));
    let mut wide = vec![[0f32; 4]; dw * sh];
    for y in 0..sh {
        for (x, ws) in across.iter().enumerate() {
            let mut acc = [0f32; 4];
            for &(j, w) in ws {
                let p = pre[y * sw + j];
                (0..4).for_each(|c| acc[c] += p[c] * w);
            }
            wide[y * dw + x] = acc;
        }
    }
    let mut out = vec![0u8; dw * dh * 4];
    for (y, ws) in down.iter().enumerate() {
        for x in 0..dw {
            let mut acc = [0f32; 4];
            for &(j, w) in ws {
                let p = wide[j * dw + x];
                (0..4).for_each(|c| acc[c] += p[c] * w);
            }
            let i = (y * dw + x) * 4;
            let a = acc[3].clamp(0.0, 255.0);
            if a >= 0.5 {
                (0..3).for_each(|c| out[i + c] = (acc[c] * 255.0 / a).round().clamp(0.0, 255.0) as u8);
                out[i + 3] = a.round() as u8;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{resize, set_on};

    #[test]
    fn a_setting_only_once_turned_on() {
        assert!(!set_on(None));
        assert!(!set_on(Some(0)));
        assert!(set_on(Some(1)));
    }

    #[test]
    fn resize_keeps_a_flat_colour_and_the_size() {
        let src: Vec<u8> = (0..16 * 16).flat_map(|_| [200, 100, 50, 255]).collect();
        for (dw, dh) in [(20, 20), (12, 12), (30, 30)] {
            let out = resize(&src, 16, 16, dw, dh);
            assert_eq!(out.len(), dw * dh * 4);
            assert!(out.chunks_exact(4).all(|p| p == [200, 100, 50, 255]));
        }
    }

    #[test]
    fn resize_softens_a_hard_edge_without_a_dark_fringe() {
        // Left half opaque white, right half fully transparent black.
        let src: Vec<u8> = (0..8 * 8).flat_map(|i| if i % 8 < 4 { [255, 255, 255, 255] } else { [0, 0, 0, 0] }).collect();
        let out = resize(&src, 8, 8, 10, 10);
        let row: Vec<&[u8]> = out.chunks_exact(4).take(10).collect();
        // Somewhere between, partly see-through; what shows is still white.
        assert!(row.iter().any(|p| p[3] > 0 && p[3] < 255));
        assert!(row.iter().filter(|p| p[3] > 0).all(|p| p[0] == 255 && p[1] == 255 && p[2] == 255));
    }
}

// At start: Windows' taskbar left put away by a run that is gone (its guard
// gone too) is given back.
pub fn recover(sh: &Shared) {
    if crate::shell::recover(&sh.dir.join("taskbar.json")) {
        // Every window told, as the run killed holding it never did
        // (systray.rs handed_back), and whose the shell's windows are now.
        crate::systray::handed_back();
        sh.log(&format!("taskbar: Windows' taskbar was left put away: given back, every window told; {}", crate::shell::lone_win().said()));
    }
    // The desktop's icons left moved by such a run (icons.rs): as they were,
    // before this one takes the strip (and keeps their places anew); not
    // waited for, the start goes on (icons.rs keeps them one at a time).
    if crate::icons::file(&sh.dir).exists() {
        let (dir, log) = (sh.dir.clone(), crate::shared(&sh.app));
        std::thread::spawn(move || {
            for line in crate::icons::given_back(&dir) {
                log.log(&line);
            }
        });
    }
}

// From the page: a press on a tray icon (taskbar_tray), a button of the
// taskbar's own (taskbar_open).
#[tauri::command]
pub fn taskbar_tray(app: tauri::AppHandle, key: u64, press: String) -> bool {
    let sh = crate::shared(&app);
    imp::press(&sh, key, &press, crate::island::hwnd(&sh))
}

#[tauri::command]
pub fn taskbar_open(what: String) -> bool {
    imp::open(&what)
}

// The wheel on the quick settings' button: the volume steps up or down.
#[tauri::command]
pub fn taskbar_volume(steps: i32) -> bool {
    imp::volume(steps)
}

// The clock's tip as Windows' own has it (clock.rs): the date in full, and
// Windows' additional clocks.
#[tauri::command]
pub async fn taskbar_clock_tip(app: tauri::AppHandle) -> String {
    let sh = crate::shared(&app);
    crate::clock::tip(&crate::i18n::t(sh.lang(), "clock.local"))
}

// Whether Windows' End task is on now: asked as a program's menu opens
// (the look says it only every half minute).
#[tauri::command]
pub async fn taskbar_end_task() -> bool {
    end_task_on()
}

// The input methods to pick from (the mark's right press), each with its
// icon, the one in front marked (inputs.rs list); one of them picked.
#[tauri::command]
pub async fn taskbar_inputs() -> serde_json::Value {
    crate::inputs::list()
}

#[tauri::command]
pub fn taskbar_input(id: String) -> bool {
    crate::inputs::pick(&id)
}

// Windows' own battery font, for the page to draw the battery as Windows'
// taskbar does (island.js); empty where Windows has none.
#[tauri::command]
pub fn taskbar_battery_font() -> tauri::ipc::Response {
    tauri::ipc::Response::new(crate::widgets::battery_font())
}

// Windows' own Task View pictures, for its button (island.js, bar.js): the
// two Windows 11's Settings shows by the Task view switch, as its taskbar
// has them, { light, dark } (for a light strip, for a dark one), each an
// SVG. Kept within Settings' resource index, in no file of their own
// (pri.rs); read once. Null where Windows has none (Windows 10): the page
// takes the Task View glyph of Windows' icon font then.
#[tauri::command]
pub async fn taskbar_task_view_icons() -> Option<serde_json::Value> {
    static ICONS: std::sync::OnceLock<Option<serde_json::Value>> = std::sync::OnceLock::new();
    ICONS.get_or_init(task_view_icons).clone()
}

fn task_view_icons() -> Option<serde_json::Value> {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    let file = std::path::Path::new(&root).join(r"SystemResources\Windows.UI.SettingsAppThreshold\Windows.UI.SettingsAppThreshold.pri");
    let mut got = crate::pri::embedded(&file, &["Files/SystemSettings/Assets/Taskview_Light.svg", "Files/SystemSettings/Assets/Taskview_Dark.svg"]).into_iter();
    let mut svg = || got.next().flatten().and_then(|b| String::from_utf8(b).ok()).filter(|s| s.contains("<svg"));
    let (light, dark) = (svg()?, svg()?);
    Some(serde_json::json!({ "light": light, "dark": dark }))
}

// A press on a window's button (press: to the front or minimized; front: to
// the front alone, never minimized, the next of a program's windows or one
// picked from its list; drag: to the front, a file held over its button;
// close; end: Windows' End task, tasks::end_task).
#[tauri::command]
pub fn taskbar_window(id: i64, what: String) -> bool {
    imp::window(&what, id as isize)
}

// Shift and Ctrl held now ({ shift, ctrl }), read at a press on a program's
// button: the page's own events may say otherwise, her windows never taking
// the keyboard (Windows keeps the keys' state for the thread that has it).
// Off the main thread, which a menu of hers may hold.
#[tauri::command]
pub async fn taskbar_held() -> serde_json::Value {
    let (shift, ctrl) = crate::tasks::keys_held();
    serde_json::json!({ "shift": shift, "ctrl": ctrl })
}

// Win pressed while her window has the keyboard (the keys on her strip, or
// her settings): given back while Win is held, so that what comes with it
// reaches the keys' hook (winkeys.rs pause).
#[tauri::command]
pub fn taskbar_keys_pause() {
    crate::winkeys::pause();
}

// Shift and a right press on a program's button: its window's own menu, or
// one for all its windows (Restore, Minimize, Close them all), at the pointer.
// Answered once it has gone: till then the page opens no list of windows on
// the pointer resting there, which came up over it.
#[tauri::command]
pub async fn taskbar_window_menu(app: tauri::AppHandle, ids: Vec<i64>) -> bool {
    let lang = crate::shared(&app).lang();
    let hwnds = ids.into_iter().map(|id| id as isize).collect();
    tauri::async_runtime::spawn_blocking(move || imp::window_menu(hwnds, lang)).await.unwrap_or(false)
}

// A press on the button of a program with several windows: all of them to
// the front, or all minimized when one is in front.
#[tauri::command]
pub fn taskbar_windows(ids: Vec<i64>) -> bool {
    imp::windows(ids.into_iter().map(|id| id as isize).collect())
}

// While the page has something open that closes on a press elsewhere: the
// presses told to it (taskbar:press).
#[tauri::command]
pub fn taskbar_watch_presses(on: bool) {
    watch_presses(WATCH_ISLAND, on);
}

// Who wants the presses watched: her island's page, her taskbar on another
// display (bars.rs); watched while either does, one's "no more" not ending
// the other's.
pub const WATCH_ISLAND: u8 = 1;
pub const WATCH_BARS: u8 = 2;
static WATCHING: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub fn watch_presses(who: u8, on: bool) {
    use std::sync::atomic::Ordering;
    let was = if on { WATCHING.fetch_or(who, Ordering::SeqCst) | who } else { WATCHING.fetch_and(!who, Ordering::SeqCst) & !who };
    imp::watch_presses(was != 0);
}

// Where the page left room for windows' live pictures (window, x, y, w, h
// in its own px): shown there; none, taken away.
#[tauri::command]
pub fn taskbar_thumbs(app: tauri::AppHandle, items: Vec<(i64, f64, f64, f64, f64)>) {
    let sh = crate::shared(&app);
    let ((ox, oy), sf) = crate::island::origin(&sh);
    let px = |v: f64| (v * sf).round() as i32;
    let items = items.into_iter().map(|(h, x, y, w, ht)| (h as isize, (ox + px(x), oy + px(y), px(w), px(ht)))).collect();
    thumbs(WATCH_ISLAND, items);
}

// The live pictures (one window of their own for all of them) for her
// island's list or for one on another display's taskbar (bars.rs), on the
// screen, physical: whose they are now, so that one's closing does not take
// away the other's just opened.
static THUMBS_FOR: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub fn thumbs(who: u8, items: Vec<(isize, (i32, i32, i32, i32))>) {
    use std::sync::atomic::Ordering;
    if items.is_empty() {
        if THUMBS_FOR.compare_exchange(who, 0, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            return;
        }
    } else {
        THUMBS_FOR.store(who, Ordering::SeqCst);
    }
    imp::thumbs(items);
}

// A program's button: launch (a program kept on the taskbar, not running;
// or another of its windows), pin, unpin.
#[tauri::command]
pub async fn taskbar_app(app: tauri::AppHandle, path: String, what: String, id: Option<String>) -> bool {
    imp::app(&crate::shared(&app), &what, &path, id.as_deref().filter(|s| !s.is_empty()))
}

// The same from the keys: Win+digit, Win+Shift+digit (winkeys.rs).
pub use imp::app;

// Files dropped on her taskbar or the dock (a program, a shortcut to one,
// from the desktop or File Explorer): each pinned, as Windows' own taskbar
// pins one dropped on it (pins.rs of_file); one pinned already left where
// it is, a document or a folder not. How many were pinned.
#[tauri::command]
pub async fn taskbar_pin_files(app: tauri::AppHandle, paths: Vec<String>) -> usize {
    let sh = crate::shared(&app);
    crate::tasks::with_com();
    let mut pinned = 0;
    for path in paths.iter().take(16) {
        let pin = match crate::pins::of_file(path) {
            Ok(pin) => pin,
            Err(why) => {
                sh.log(&format!("taskbar: a file dropped on it not pinned: {why}"));
                continue;
            }
        };
        let (target, id) = (pin["path"].as_str().unwrap_or(""), pin["id"].as_str().filter(|s| !s.is_empty()));
        let kept = sh.setting("taskbarPinned");
        let there = kept.as_array().into_iter().flatten().any(|p| match (id, p["id"].as_str()) {
            (Some(id), Some(theirs)) => theirs.eq_ignore_ascii_case(id),
            (None, None) => p["path"].as_str().is_some_and(|p| p.eq_ignore_ascii_case(target)),
            _ => false,
        });
        if there {
            continue;
        }
        if imp::app(&sh, "pin", target, id) {
            pinned += 1;
            sh.log(&format!("taskbar: {} pinned, dropped on it", crate::crash::private(pin["name"].as_str().unwrap_or(""))));
        }
    }
    pinned
}

// A tray icon kept out on the taskbar or folded away (dragged there), by
// its name (systray.rs Shown), in the settings (trayPinned); Windows' own
// choice until the person makes one. And the tray's order as dragged
// (trayOrder, names), the two in one change.
#[tauri::command]
pub fn taskbar_tray_pin(app: tauri::AppHandle, name: String, pinned: Option<bool>, order: Option<Vec<String>>) {
    let sh = crate::shared(&app);
    let mut change = serde_json::Map::new();
    if let Some(pinned) = pinned {
        let mut kept = sh.setting("trayPinned").as_object().cloned().unwrap_or_default();
        kept.insert(name, serde_json::json!(pinned));
        change.insert("trayPinned".into(), serde_json::Value::Object(kept));
    }
    if let Some(order) = order {
        change.insert("trayOrder".into(), serde_json::json!(order.into_iter().take(200).collect::<Vec<_>>()));
    }
    if !change.is_empty() {
        sh.change(serde_json::Value::Object(change));
    }
}

// Where the page drew each tray icon (key, x, y, w, h in its own px), for
// the programs that ask where theirs is (Shell_NotifyIconGetRect): WeChat
// looks for the pointer over its icon by it, QQ opens its menu from it.
#[tauri::command]
pub fn taskbar_tray_rects(app: tauri::AppHandle, rects: Vec<(u64, f64, f64, f64, f64)>) {
    let sh = crate::shared(&app);
    let ((ox, oy), sf) = crate::island::origin(&sh);
    let px = |v: f64| (v * sf).round() as i32;
    crate::systray::set_rects(rects.into_iter().map(|(k, x, y, w, h)| (k, (ox + px(x), oy + px(y), ox + px(x + w), oy + px(y + h)))).collect());
}
