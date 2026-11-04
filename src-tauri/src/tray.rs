// The tray icon, whose face shows her mood, and the menu it shares with a
// right-click on her or the island. The faces are drawn by
// scripts/make-icons.js (icons/tray).
use std::sync::{Arc, Mutex};

use serde_json::json;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Wry};

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

// Where the menu opens: the tray icon, her, or her home's window.
#[derive(Clone, Copy, PartialEq)]
pub enum Over {
    Tray,
    Her,
    Home,
}

// The menu, over the tray, her or her home. Over the taskbar (her home as
// the taskbar): what a taskbar's right press is for (do not disturb, the
// settings, Windows' own, quit). On her and the tray: what is about her
// (home or out, her pet, her size), do not disturb, the settings, quit.
// Over her island at the top of the screen: its widgets and the rest of hers.
pub fn menu(sh: &Shared, over: Over) -> tauri::Result<Menu<Wry>> {
    let app = &sh.app;
    let lang = sh.lang();
    let t = |key: &str| i18n::t(lang, key);
    let settings = sh.settings.lock().unwrap().clone();
    let flag = |key: &str| settings.get(key).and_then(|v| v.as_bool()).unwrap_or(false);
    let home = sh.home();
    let is_tray = over == Over::Tray;
    let menu = Menu::new(app)?;
    // The taskbar's layout being edited: a way out first, whatever its bar
    // does (zhoupengjie/Wakuwaku#53: its buttons took no press, Esc went to
    // the window in front, and only quitting from the tray ended it).
    if sh.island.lock().unwrap().editing {
        menu.append(&MenuItem::with_id(app, "endEdit", t("menu.endEdit"), true, None::<&str>)?)?;
        menu.append(&PredefinedMenuItem::separator(app)?)?;
    }

    // The widgets (picker.js): 添加小工具… opens the picker, with the taskbar's
    // layout being edited while the taskbar is her home; 编辑胶囊 (her island
    // at the top of the screen) her capsule's panel. Greyed while her island
    // is not up to show them (she hidden, do not disturb).
    let isle_up = home == "taskbar" || crate::island::is_shown(sh);
    if over == Over::Home && home == "taskbar" {
        menu.append(&MenuItem::with_id(app, "addWidgets", t("menu.addWidgets"), true, None::<&str>)?)?;
        menu.append(&CheckMenuItem::with_id(app, "dnd", t("menu.dnd"), true, flag("dnd"), None::<&str>)?)?;
        menu.append(&MenuItem::with_id(app, "settings", t("menu.settings"), true, None::<&str>)?)?;
        menu.append(&MenuItem::with_id(app, "editTaskbar", t("menu.editTaskbar"), true, None::<&str>)?)?;
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        menu.append(&MenuItem::with_id(app, "taskManager", t("menu.taskManager"), true, None::<&str>)?)?;
        menu.append(&MenuItem::with_id(app, "winSettings", t("menu.winSettings"), true, None::<&str>)?)?;
        menu.append(&MenuItem::with_id(app, "controlPanel", t("menu.controlPanel"), true, None::<&str>)?)?;
        menu.append(&MenuItem::with_id(app, "deviceManager", t("menu.deviceManager"), true, None::<&str>)?)?;
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        menu.append(&MenuItem::with_id(app, "quit", t("menu.quit"), true, None::<&str>)?)?;
        return Ok(menu);
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
    let sizes = Submenu::new(app, t("menu.size"), true)?;
    let scale = settings.get("scale").and_then(|v| v.as_f64()).unwrap_or(0.55);
    for (name, value) in data::SCALES {
        let label = t(&format!("menu.{name}"));
        sizes.append(&CheckMenuItem::with_id(app, format!("size:{name}"), label, true, (scale - value).abs() < 1e-9, None::<&str>)?)?;
    }
    // Let her out onto the desktop, or call her back home.
    let out_label = if flag("out") { t("menu.callBack") } else { t("menu.letOut") };

    // On her, and the tray's: what is about her alone (the user's call,
    // 2026-10-28). Calling her home first; her walk, her eyes and her home
    // are the settings' (Appearance), the taskbar's own things its menu's.
    // The tray keeps showing or hiding her, and the corner, to find her by.
    if over != Over::Home {
        if is_tray {
            let label = if sh.is_visible() { t("menu.hide") } else { t("menu.show") };
            menu.append(&MenuItem::with_id(app, "toggle", label, !flag("dnd"), None::<&str>)?)?;
        }
        menu.append(&MenuItem::with_id(app, "out", out_label, true, None::<&str>)?)?;
        if is_tray {
            menu.append(&MenuItem::with_id(app, "corner", t("menu.backToCorner"), true, None::<&str>)?)?;
        }
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        menu.append(&pets)?;
        menu.append(&sizes)?;
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        menu.append(&CheckMenuItem::with_id(app, "dnd", t("menu.dnd"), true, flag("dnd"), None::<&str>)?)?;
        menu.append(&MenuItem::with_id(app, "settings", t("menu.settings"), true, None::<&str>)?)?;
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        menu.append(&MenuItem::with_id(app, "quit", t("menu.quit"), true, None::<&str>)?)?;
        return Ok(menu);
    }

    // Her island's own, at the top of the screen.
    menu.append(&MenuItem::with_id(app, "addWidgets", t("menu.addWidgets"), isle_up, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(app, "editCapsule", t("menu.editCapsule"), isle_up, None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&pets)?;
    menu.append(&sizes)?;
    for key in ["walk", "look", "play"] {
        menu.append(&CheckMenuItem::with_id(app, key, t(&format!("menu.{key}")), true, flag(key), None::<&str>)?)?;
    }
    // Her home: the island, the taskbar, or the top bar and the dock.
    let homes = Submenu::new(app, t("menu.home"), true)?;
    let display = sh.setting("display");
    for id in ["island", "taskbar", "split"] {
        let on = if id == "island" { home == "island" } else { display.as_str() == Some(id) };
        homes.append(&CheckMenuItem::with_id(app, format!("display:{id}"), t(&format!("menu.{id}")), true, on, None::<&str>)?)?;
    }
    menu.append(&homes)?;
    menu.append(&MenuItem::with_id(app, "out", out_label, true, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(app, "corner", t("menu.backToCorner"), true, None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&CheckMenuItem::with_id(app, "dnd", t("menu.dnd"), true, flag("dnd"), None::<&str>)?)?;
    menu.append(&MenuItem::with_id(app, "settings", t("menu.settings"), true, None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "quit", t("menu.quit"), true, None::<&str>)?)?;
    Ok(menu)
}

// A right press on the volume, the network or the clock of her taskbar:
// the menu Windows' own taskbar has there (SystemTray.dll, read
// 2026-10-14), its items in its order and words, a native menu as hers on
// the strip is; each opens what Windows' opens (taskbar.rs open). None for
// another part: hers.
pub fn part_menu(sh: &Shared, part: &str) -> Option<tauri::Result<Menu<Wry>>> {
    let items: &[&str] = match part {
        "volume" => &["soundFix", "-", "mixer", "sound"],
        "network" => &["netFix", "-", "network"],
        "clock" => &["dateTime", "notifySettings"],
        // The dock's Recycle Bin (split): open it, or empty it (Windows asks first).
        "recycleBin" => &["recycleBin", "-", "emptyBin"],
        _ => return None,
    };
    let app = &sh.app;
    let lang = sh.lang();
    let build = || -> tauri::Result<Menu<Wry>> {
        let menu = Menu::new(app)?;
        for item in items {
            if *item == "-" {
                menu.append(&PredefinedMenuItem::separator(app)?)?;
            } else {
                menu.append(&MenuItem::with_id(app, format!("win:{item}"), i18n::t(lang, &format!("menu.{item}")), true, None::<&str>)?)?;
            }
        }
        Ok(menu)
    };
    Some(build())
}

pub fn on_menu(sh: &Arc<Shared>, id: &str) {
    match id {
        "toggle" => sh.set_hidden(sh.is_visible()),
        "walk" | "look" | "play" | "dnd" => sh.change(json!({ id: !sh.flag(id) })),
        "out" => crate::set_out(sh, !sh.flag("out")),
        "corner" => crate::come_home(sh),
        "settings" => crate::island::open_settings(sh, None),
        // The taskbar's parts dragged about, on the strip itself (island.js).
        "editTaskbar" => {
            let _ = sh.app.emit_to("island", "taskbar:edit", ());
        }
        // Out of editing it, as its 完成: what was changed is kept as it
        // went (picker.js), there is no 取消; the picker and the panels close.
        "endEdit" => {
            let _ = sh.app.emit_to("island", "taskbar:edit", json!({ "end": true }));
        }
        // The picker (picker.js fromMenu): on the taskbar with its layout
        // being edited, at the top of the screen on its own.
        "addWidgets" => {
            let _ = if sh.home() == "taskbar" { sh.app.emit_to("island", "taskbar:edit", json!({ "picker": true })) } else { sh.app.emit_to("island", "island:picker", json!({ "open": "picker" })) };
        }
        // Her capsule's panel (capsule-panel.js): its members, their order
        // and how each shows there.
        "editCapsule" => {
            let _ = sh.app.emit_to("island", "island:picker", json!({ "open": "capsule" }));
        }
        "winSettings" => {
            crate::tasks::launch("ms-settings:");
        }
        // Windows' Task Manager, as its own taskbar's menu has it first.
        "taskManager" => {
            crate::tasks::launch("taskmgr.exe");
        }
        "controlPanel" => {
            crate::tasks::launch("control.exe");
        }
        "deviceManager" => {
            crate::tasks::launch("devmgmt.msc");
        }
        // Quit by the person: no session's hook starts her again until she
        // is opened (data.rs Quit).
        "quit" => sh.quit(crate::data::Quit::ByChoice, "her menu's Quit"),
        _ => {
            if let Some(what) = id.strip_prefix("win:") {
                crate::taskbar::taskbar_open(what.to_string());
            } else if let Some(home) = id.strip_prefix("display:") {
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
        .menu(&menu(sh, Over::Tray)?)
        .show_menu_on_left_click(false)
        // Click: show or hide her (in do-not-disturb, the settings); right-click: the menu.
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                let sh = crate::shared(tray.app_handle());
                if sh.flag("dnd") {
                    crate::island::open_settings(&sh, None);
                } else if cfg!(target_os = "macos") && sh.home() == "island" && !sh.flag("out") {
                    // On a Mac the notch is another app's to hover (Atoll and
                    // the like): her island stays in it, and this drops it
                    // open, or puts it back (island.js peek).
                    let _ = sh.app.emit_to("island", "island:peek", ());
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

// The tray icon is only ever reached on the main thread: Tauri's
// tray_by_id hands back a clone of the icon, and the icon is counted by an
// Rc (tray-icon's), not made for two threads. Reached from a settings
// command on a worker thread while the main thread had it too, its count
// went wrong, the icon was dropped from the tray (as at its last clone) and
// the next count panicked (the dev build, 2026-11-04: "removed wakuwaku.exe",
// then rc.rs's check, from tray_by_id in refresh). Each refresh goes there.
fn on_main(sh: &Shared, f: fn(&Shared)) {
    let app = sh.app.clone();
    let _ = sh.app.run_on_main_thread(move || f(&crate::shared(&app)));
}

// The face and the tooltip, on every change of mood (on the main thread).
pub fn refresh(sh: &Shared) {
    on_main(sh, refresh_here);
}

fn refresh_here(sh: &Shared) {
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
    // Set only as it changes: setting it is a message to the tray (ours, and
    // on to Explorer's) on every redraw.
    static TIP: Mutex<String> = Mutex::new(String::new());
    let tip = format!("Wakuwaku{}{status}", if lang == "zh" { "：" } else { ": " });
    if *TIP.lock().unwrap() != tip {
        let _ = tray.set_tooltip(Some(tip.clone()));
        *TIP.lock().unwrap() = tip;
    }
    let mut shown = SHOWN_MOOD.lock().unwrap();
    if *shown != mood {
        if let Ok(image) = Image::from_bytes(face(&mood)) {
            let _ = tray.set_icon(Some(image));
            *shown = mood;
        }
    }
}

// The tray's menu, when the settings or her being hidden change (on the main thread).
pub fn refresh_menu(sh: &Shared) {
    on_main(sh, refresh_menu_here);
}

fn refresh_menu_here(sh: &Shared) {
    if let (Some(tray), Ok(menu)) = (sh.app.tray_by_id(TRAY_ID), menu(sh, Over::Tray)) {
        let _ = tray.set_menu(Some(menu));
    }
}
