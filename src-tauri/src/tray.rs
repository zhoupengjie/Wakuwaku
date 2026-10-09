// The tray icon, whose face shows her mood, and the menu it shares with a
// right-click on her or the island. The faces are drawn by
// scripts/make-icons.js (icons/tray).
use std::sync::{Arc, Mutex};

use serde_json::json;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Wry;

use crate::{data, i18n, now_ms, pet, Shared};

const TRAY_ID: &str = "main";

fn face(mood: &str) -> &'static [u8] {
    match mood {
        "working" => include_bytes!("../icons/tray/working.png"),
        "waiting" => include_bytes!("../icons/tray/waiting.png"),
        "done" => include_bytes!("../icons/tray/done.png"),
        "review" => include_bytes!("../icons/tray/review.png"),
        "error" => include_bytes!("../icons/tray/error.png"),
        _ => include_bytes!("../icons/tray/idle.png"),
    }
}

// The menu, over her (is_tray false) or the tray.
pub fn menu(sh: &Shared, is_tray: bool) -> tauri::Result<Menu<Wry>> {
    let app = &sh.app;
    let lang = sh.lang();
    let t = |key: &str| i18n::t(lang, key);
    let settings = sh.settings.lock().unwrap().clone();
    let flag = |key: &str| settings.get(key).and_then(|v| v.as_bool()).unwrap_or(false);
    let home = sh.home();
    let menu = Menu::new(app)?;

    if is_tray {
        let label = if sh.is_visible() { t("menu.hide") } else { t("menu.show") };
        menu.append(&MenuItem::with_id(app, "toggle", label, !flag("dnd"), None::<&str>)?)?;
    }

    let pets = Submenu::new(app, t("menu.pet"), true)?;
    let found = data::pets(&sh.dir);
    if found.is_empty() {
        pets.append(&MenuItem::with_id(app, "none", t("menu.noPets"), false, None::<&str>)?)?;
    }
    let current = settings.get("pet").and_then(|v| v.as_str()).unwrap_or("");
    for p in &found {
        pets.append(&CheckMenuItem::with_id(app, format!("pet:{}", p.id), &p.name, true, p.id == current, None::<&str>)?)?;
    }
    menu.append(&pets)?;

    let sizes = Submenu::new(app, t("menu.size"), true)?;
    let scale = settings.get("scale").and_then(|v| v.as_f64()).unwrap_or(0.55);
    for (name, value) in data::SCALES {
        let label = t(&format!("menu.{name}"));
        sizes.append(&CheckMenuItem::with_id(app, format!("size:{name}"), label, true, (scale - value).abs() < 1e-9, None::<&str>)?)?;
    }
    menu.append(&sizes)?;

    for key in ["bubble", "walk", "look"] {
        menu.append(&CheckMenuItem::with_id(app, key, t(&format!("menu.{key}")), true, flag(key), None::<&str>)?)?;
    }
    // Her home: the corner, the island or the bar.
    let homes = Submenu::new(app, t("menu.home"), true)?;
    for id in ["corner", "island", "bar"] {
        homes.append(&CheckMenuItem::with_id(app, format!("display:{id}"), t(&format!("menu.{id}")), true, home == id, None::<&str>)?)?;
    }
    menu.append(&homes)?;
    // Let her out onto the desktop, or call her back home.
    let label = if flag("out") { t("menu.callBack") } else { t("menu.letOut") };
    menu.append(&MenuItem::with_id(app, "out", label, true, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(app, "corner", t("menu.backToCorner"), true, None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&CheckMenuItem::with_id(app, "dnd", t("menu.dnd"), true, flag("dnd"), None::<&str>)?)?;
    menu.append(&MenuItem::with_id(app, "settings", t("menu.settings"), true, None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "quit", t("menu.quit"), true, None::<&str>)?)?;
    Ok(menu)
}

pub fn on_menu(sh: &Arc<Shared>, id: &str) {
    match id {
        "toggle" => sh.set_hidden(sh.is_visible()),
        "bubble" | "walk" | "look" | "dnd" => sh.change(json!({ id: !sh.flag(id) })),
        "out" => crate::set_out(sh, !sh.flag("out")),
        "corner" => crate::come_home(sh),
        "settings" => crate::island::open_settings(sh, None),
        "quit" => sh.app.exit(0),
        _ => {
            if let Some(home) = id.strip_prefix("display:") {
                crate::set_display(sh, home);
            } else if let Some(pet_id) = id.strip_prefix("pet:") {
                sh.change(json!({ "pet": pet_id }));
            } else if let Some(name) = id.strip_prefix("size:") {
                if let Some((_, scale)) = data::SCALES.iter().find(|(n, _)| *n == name) {
                    pet::resize(sh, *scale);
                }
            }
        }
    }
}

pub fn create(sh: &Shared) -> tauri::Result<()> {
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(face("idle"))?)
        .menu(&menu(sh, true)?)
        .show_menu_on_left_click(false)
        // Click: show or hide her (in do-not-disturb, the settings); right-click: the menu.
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                let sh = crate::shared(tray.app_handle());
                if sh.flag("dnd") {
                    crate::island::open_settings(&sh, None);
                } else {
                    sh.set_hidden(sh.is_visible());
                }
            }
        })
        .build(&sh.app)?;
    refresh(sh);
    Ok(())
}

static SHOWN_MOOD: Mutex<String> = Mutex::new(String::new());

// The face and the tooltip, on every change of mood.
pub fn refresh(sh: &Shared) {
    let Some(tray) = sh.app.tray_by_id(TRAY_ID) else { return };
    let now = sh.pet.lock().unwrap().get(now_ms());
    let mood = now["mood"].as_str().unwrap_or("idle").to_string();
    let lang = sh.lang();
    // Whose: the session's name, unless the specifics are kept off screen.
    let name: String = now["name"].as_str().unwrap_or("").chars().take(40).collect();
    let project = if name.is_empty() || sh.setting("details") == false { now["project"].as_str().unwrap_or("") } else { &name };
    let status = if sh.flag("dnd") {
        i18n::t(lang, "menu.dnd")
    } else if project.is_empty() {
        i18n::t(lang, &format!("mood.{mood}"))
    } else {
        format!("{} · {project}", i18n::t(lang, &format!("mood.{mood}")))
    };
    let _ = tray.set_tooltip(Some(format!("Wakuwaku{}{status}", if lang == "zh" { "：" } else { ": " })));
    let mut shown = SHOWN_MOOD.lock().unwrap();
    if *shown != mood {
        if let Ok(image) = Image::from_bytes(face(&mood)) {
            let _ = tray.set_icon(Some(image));
            *shown = mood;
        }
    }
}

// The tray's menu, when the settings or her being hidden change.
pub fn refresh_menu(sh: &Shared) {
    if let (Some(tray), Ok(menu)) = (sh.app.tray_by_id(TRAY_ID), menu(sh, true)) {
        let _ = tray.set_menu(Some(menu));
    }
}
