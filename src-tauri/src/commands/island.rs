// What the island's page calls: held open, the taskbar's layout edited,
// something open in it, her seat on the taskbar, raised, stowed at the top
// of the screen, the blur under it, her dropped out of it; and the
// handover with Windows' taskbar.
use serde_json::Value;
use tauri::{AppHandle, WebviewWindow};

use super::is_island;
use crate::{backplate, island, pet, shared};

#[tauri::command]
pub async fn island_holding(app: AppHandle, is_holding: bool) {
    island::set_holding(&shared(&app), is_holding);
}

// The taskbar's layout being edited (island.ts editTaskbar), and where its
// bar, library and tools are in the page: the window keeps the mouse there.
#[tauri::command]
pub async fn island_editing(app: AppHandle, on: bool, rects: Vec<[f64; 4]>, page_sf: Option<f64>) {
    island::set_editing(&shared(&app), on, rects, page_sf);
}

// The island's page has something open in it (hovered open, a prompt, the
// settings, opened by itself), or nothing now: inside Windows' taskbar its
// window goes back in once nothing is (island.rs set_opened).
#[tauri::command]
pub async fn island_opened(app: AppHandle, window: WebviewWindow, on: bool) {
    if window.label() == "island" {
        island::set_opened(&shared(&app), on);
    }
}

// The handover with Windows' taskbar: the page's answers (covered, bare, done).
#[tauri::command]
pub fn taskbar_handoff(what: String) {
    island::handoff_answer(&what);
}

// Where the page laid her island out on the taskbar: her seat, along the
// strip from its start (x on a bottom or top bar, y on a side bar).
#[tauri::command]
pub async fn island_seat(app: AppHandle, x: f64) {
    island::set_seat(&shared(&app), x);
}

// Something opened up from her taskbar's strip (the island, a list, a
// flyout): its window over any other that came over it (island.rs raise,
// zhoupengjie/Wakuwaku#55).
#[tauri::command]
pub async fn island_raise(app: AppHandle, window: WebviewWindow) {
    if is_island(&window) {
        island::raise(&shared(&app));
    }
}

// Her island at the top of the screen: up out of the way of the window
// under it or not, and its closed size (logical px), for the look for a
// window under it and the pointer calling it down (island.rs set_stow).
#[tauri::command]
pub async fn island_stow(app: AppHandle, window: WebviewWindow, stowed: bool, width: f64, height: f64) {
    if is_island(&window) {
        island::set_stow(&shared(&app), stowed, width, height);
    }
}

// Under her island, a blur of what is behind it (backplate.rs): where the
// page puts it ({ x, y, width, height } in its px, set in from the island's
// edge; the size of the page it measured in; Windows' mode), faded in over
// fade ms; none, faded out. Measured in a window of another size (resized
// meanwhile), it is not shown: the page sends it again once resized.
#[tauri::command]
pub async fn island_plate(app: AppHandle, rect: Option<Value>, fade: u32) {
    let sh = shared(&app);
    let Some(r) = rect else { return backplate::set(&sh, None, fade) };
    let ((ox, oy), (ww, wh), sf) = island::frame(&sh);
    let num = |key: &str| r[key].as_f64().unwrap_or(0.0);
    if (num("vw") * sf - ww as f64).abs() > 2.0 || (num("vh") * sf - wh as f64).abs() > 2.0 {
        sh.log(&format!("plate: measured in a {}x{} page, the window {ww}x{wh} at {sf}: not shown", num("vw"), num("vh")));
        return;
    }
    let (x, y, w, h) = backplate::on_screen((ox, oy), sf, (num("x"), num("y"), num("width"), num("height")));
    let plate = (w > 0 && h > 0).then_some(backplate::Plate { x, y, w, h, dark: r["dark"].as_bool() != Some(false) });
    backplate::set(&sh, plate, fade);
}

// The drop pinched off: she is out, under the cursor, her middle this far
// from it. The button is still held on the island; island_drop says when it
// is let go.
#[tauri::command]
pub async fn island_release(app: AppHandle, offset: Value) {
    let sh = shared(&app);
    sh.log(&format!("island: release {offset}"));
    if !sh.config().out {
        pet::carry(&sh, &offset);
    }
}

#[tauri::command]
pub async fn island_drop(app: AppHandle) {
    let sh = shared(&app);
    sh.log("island: drop");
    pet::let_go(&sh);
}
