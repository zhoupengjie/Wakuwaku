// What her pages call (invoke, through src/pet/bridge.ts), by what it is
// about: her window's and what both pages share (pet.rs), the island's
// (island/); here what is neither's. Those of the settings, the bars, the
// desktop's widgets, the mail, the taskbar and the plugins are in their own
// modules (main.rs lists them all for Tauri).
use serde_json::Value;
use tauri::{AppHandle, WebviewWindow};

use crate::{plugins, shared, Shared};
use crate::agents::jump;

pub mod island;
pub mod pet;

// --- From the pages -----------------------------------------------------------------
// Async, so none of them runs on the main thread while it waits on a lock.
// `window` says which page: "pet" or "island".

pub(crate) fn is_island(window: &WebviewWindow) -> bool {
    window.label() == "island"
}

// A page's answer to /debug/eval.
#[tauri::command]
pub async fn debug_result(app: AppHandle, id: u64, value: Value) {
    if let Some(tx) = shared(&app).evals.lock().unwrap().1.remove(&id) {
        let _ = tx.send(value);
    }
}

// A session clicked (in the island, the settings, the panel): to its window.
// False when there is nowhere to go.
#[tauri::command]
pub async fn session_jump(app: AppHandle, id: String) -> bool {
    let sh = shared(&app);
    tauri::async_runtime::spawn_blocking(move || jump_to(&sh, &id)).await.unwrap_or(false)
}

// To a session's window (jump.rs), by what the pet knows of it. Blocking: it
// looks through the processes and windows.
pub fn jump_to(sh: &Shared, id: &str) -> bool {
    let target = sh.pet.lock().unwrap().jump_target(id);
    let went = target.is_some_and(|(chain, hints)| {
        let hints: Vec<&str> = hints.iter().map(String::as_str).collect();
        jump::go(id, &chain, &hints)
    });
    sh.log(&format!("jump to {id}: {}", if went { "there" } else { "nowhere to go" }));
    went
}

// A button on a widget's card pressed (what is playing: previous, play or
// pause, next): to the plugin the widget belongs to (plugins.rs act), which
// shows what it did at once. False for one of no plugin of hers.
#[tauri::command]
pub async fn widget_act(app: AppHandle, id: String, action: String) -> bool {
    plugins::act(&shared(&app), &id, &action)
}
