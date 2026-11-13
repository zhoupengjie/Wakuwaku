// What her pages call about her, and what both pages call alike: ready, the
// pointer over them, her drags and walks, her menu, the prompt panel and
// its answers, the keyboard, a line for her log. `window` says which page:
// "pet" or "island".
use serde_json::Value;
use tauri::{AppHandle, Emitter, WebviewWindow};

use super::is_island;
use crate::{i18n, island, mail, pet, shared, shortcuts, taskbar, tray};
use crate::agents::app_asks;

#[tauri::command]
pub async fn pet_ready(app: AppHandle, window: WebviewWindow) {
    let sh = shared(&app);
    if is_island(&window) { island::ready(&sh) } else { pet::ready(&sh) }
    let _ = app.emit_to(window.label(), "pet:asks", sh.asks_views());
}

#[tauri::command]
pub async fn pet_hover(app: AppHandle, window: WebviewWindow, is_over: bool, her: Option<bool>) {
    let sh = shared(&app);
    if is_island(&window) { island::hover(&sh, is_over) } else { pet::hover(&sh, is_over, her != Some(false)) }
}

#[tauri::command]
pub async fn pet_drag_start(app: AppHandle, window: WebviewWindow) {
    if !is_island(&window) {
        pet::drag_start(&shared(&app));
    }
}

// The button went up over her: that ends a carry as well (the island may
// have lost the button on the way).
#[tauri::command]
pub async fn pet_drag_end(app: AppHandle, window: WebviewWindow) {
    if !is_island(&window) {
        pet::let_go(&shared(&app));
    }
}

// Her menu; on the volume, the network or the clock of her taskbar (part),
// the one Windows' own taskbar has there (tray.rs part_menu).
#[tauri::command]
pub async fn pet_menu(app: AppHandle, window: WebviewWindow, part: Option<String>) {
    let sh = shared(&app);
    let over = if is_island(&window) {
        tray::Over::Home
    } else if window.label().starts_with("dock-") {
        // Windows' Start closed first, as for a program's menu there (bars.rs bar_pop).
        taskbar::close_flyout();
        tray::Over::Dock
    } else {
        tray::Over::Her
    };
    let menu = part.as_deref().and_then(|p| tray::part_menu(&sh, p)).unwrap_or_else(|| tray::menu(&sh, over));
    if let Ok(menu) = menu {
        // A Windows menu closes at once unless its window can come to the
        // front, which these cannot (they never take focus): for the menu, it may.
        let was_open = sh.settings_open() && is_island(&window);
        let _ = window.set_focusable(true);
        crate::main_wait::check("popup_menu");
        let _ = window.popup_menu(&menu);
        if !was_open {
            let _ = window.set_focusable(false);
        }
    }
}

// On her: out of the island, she flies back into it; as the pet on her own,
// the settings. (The island has no double-click: one click opens the settings.)
#[tauri::command]
pub async fn pet_double_click(app: AppHandle, window: WebviewWindow) {
    let sh = shared(&app);
    if !is_island(&window) {
        pet::double_click(&sh);
    }
}

// The room a shape of the island needs; above her, the prompt panel's size.
#[tauri::command]
pub async fn pet_panel(app: AppHandle, window: WebviewWindow, measured: Value) {
    let sh = shared(&app);
    if is_island(&window) {
        island::set_room(&sh, &measured);
    } else {
        let size = match (measured["width"].as_f64(), measured["height"].as_f64()) {
            (Some(w), Some(h)) if w > 0.0 && h > 0.0 => Some((w.ceil(), h.ceil())),
            _ => None,
        };
        pet::set_panel(&sh, size);
    }
}

#[tauri::command]
pub async fn pet_answer(app: AppHandle, id: u64, choice: Value) -> bool {
    let sh = shared(&app);
    // Another program's: allow or deny handed back to it (app_asks.rs; no
    // approval counted, as it is no agent's).
    if app_asks::holds(id) {
        let action = choice["action"].as_str().unwrap_or("?");
        let ok = app_asks::answer(id, action);
        sh.log(&format!("app ask {id}: {action} -> {ok}"));
        sh.push_asks();
        return ok;
    }
    // A code come by mail: copied, opened or let go (nothing of it logged,
    // and no approval counted).
    if mail::codes::holds(id) {
        let ok = mail::codes::answer(&sh, id, &choice);
        sh.log(&format!("mail code {id}: {} -> {ok}", choice["action"].as_str().unwrap_or("?")));
        sh.push_asks();
        return ok;
    }
    let lang = sh.lang();
    let ok = sh.asks.lock().unwrap().answer(id, &choice, &i18n::t(lang, "deny.message"));
    sh.log(&format!("answer {id}: {choice} -> {ok}"));
    if ok {
        sh.push_asks();
        // An approval given on her, for today's count.
        if matches!(choice["action"].as_str(), Some("allow" | "always")) {
            sh.count_today(|t| t.approvals += 1);
        }
    }
    ok
}

#[tauri::command]
pub async fn pet_dismiss(app: AppHandle, id: u64) {
    let sh = shared(&app);
    // Another program's: back to it, to ask its own way.
    if app_asks::dismiss(id) {
        sh.log(&format!("app ask {id} handed back to its program: asked to on her"));
        sh.push_asks();
        return;
    }
    if mail::codes::dismiss(id) {
        sh.push_asks();
        return;
    }
    if sh.asks.lock().unwrap().dismiss(id) {
        sh.log(&format!("prompt {id} handed to the terminal: asked to on her"));
        sh.push_asks();
    }
}

// Typing needs the keyboard, which these windows do not take otherwise.
#[tauri::command]
pub async fn pet_keyboard(app: AppHandle, window: WebviewWindow, needs: bool) {
    let sh = shared(&app);
    // The open settings keep it whatever a prompt's box does.
    if !needs && is_island(&window) && sh.settings_open() {
        return;
    }
    sh.set_keyboard(&window, needs);
}

// A shortcut being recorded in the settings: her global ones let go
// meanwhile (shortcuts.rs pause).
#[tauri::command]
pub async fn pet_shortcuts_pause(on: bool) {
    shortcuts::pause(on);
}

#[tauri::command]
pub async fn pet_walk(app: AppHandle, window: WebviewWindow, dx: f64, ms: f64) -> bool {
    if is_island(&window) {
        return false;
    }
    let sh = shared(&app);
    tauri::async_runtime::spawn_blocking(move || pet::walk(&sh, dx, ms)).await.unwrap_or(false)
}

#[tauri::command]
pub async fn pet_walk_stop(app: AppHandle) {
    pet::stop_walking(&shared(&app));
}

#[tauri::command]
pub async fn pet_log(app: AppHandle, window: WebviewWindow, line: String) {
    shared(&app).log(&format!("{} page: {line}", window.label()));
}
