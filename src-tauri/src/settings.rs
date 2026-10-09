// The settings, grown out of the island (src/pet/settings.js): what they
// show (a snapshot, pushed again as things change) and what they change.
use std::sync::atomic::Ordering;
use std::sync::Arc;

use serde_json::{json, Map, Value};
use tauri::{AppHandle, WebviewWindow};

use crate::{connection, data, fetch, fullscreen, i18n, island, mail, now_ms, pet, scripts, set_display, set_out, shared, Shared};

const REPO: &str = "https://github.com/zhoupengjie/wakuwaku";

// Everything the settings show.
pub fn snapshot(sh: &Shared) -> Value {
    let pets: Vec<Value> = data::pets(&sh.dir)
        .iter()
        .map(|p| json!({ "id": p.id, "name": p.name, "author": p.author, "url": data::asset_url(&p.sheet), "version": p.version, "codex": p.from_codex }))
        .collect();
    let (now, sessions) = {
        let pet = sh.pet.lock().unwrap();
        (pet.get(now_ms()), pet.list())
    };
    json!({
        "settings": Value::Object(sh.settings.lock().unwrap().clone()),
        "lang": sh.lang(),
        "pets": pets,
        "now": now,
        "sessions": sessions,
        "asks": sh.asks.lock().unwrap().views(),
        "isVisible": sh.is_visible(),
        "hidden": sh.hidden.load(Ordering::SeqCst),
        "connection": connection::connection(sh.port),
        "pluginCommands": connection::PLUGIN_COMMANDS,
        "hooks": connection::hooks_status(sh.port),
        "settingsFile": connection::claude_settings_file().to_string_lossy(),
        // Codex's hooks.json, whether any Codex event came (they run once
        // trusted), and its version: one before 0.148 gets no background hooks.
        "codex": {
            "hooks": connection::codex_hooks_status(),
            "file": connection::codex_hooks_file().to_string_lossy(),
            "seen": sh.codex_seen.load(Ordering::SeqCst) > 0,
            "version": connection::codex_version().map(|(a, b, c)| format!("{a}.{b}.{c}")),
            "async": connection::codex_runs_async(),
        },
        "loginAtStart": connection::is_open_at_login(),
        "version": sh.app.package_info().version.to_string(),
        "fullscreenAvailable": fullscreen::AVAILABLE,
        "widgets": sh.widgets_view(),
        // The plugins she runs, and where waku is for the terminal.
        "plugins": scripts::view(sh),
        // The mail accounts, each with how its watch is going.
        "mail": mail::view(sh),
        "waku": scripts::waku_path(sh).to_string_lossy(),
        "port": sh.port,
    })
}

// What a settings patch may hold, checked: anything else is dropped.
fn is_ok(sh: &Shared, key: &str, v: &Value) -> bool {
    let is_bool = v.is_boolean();
    match key {
        "lang" => matches!(v.as_str(), Some("auto" | "zh" | "en")),
        "pet" => v.as_str().is_some_and(|id| data::pets(&sh.dir).iter().any(|p| p.id == id)),
        "scale" => v.as_f64().is_some_and(|s| data::SCALES.iter().any(|(_, x)| (x - s).abs() < 1e-9)),
        "bubble" | "details" | "walk" | "look" | "sound" | "dnd" | "hideInFullscreen" | "onboarded" | "out" => is_bool,
        "display" => matches!(v.as_str(), Some("corner" | "island" | "bar")),
        "corner" => matches!(v.as_str(), Some("br" | "bl" | "tr" | "tl")),
        "hold" => v.as_str() == Some("seen") || matches!(v.as_u64(), Some(8 | 30 | 120)),
        "promptWaitSec" => matches!(v.as_u64(), Some(30 | 60 | 120 | 290)),
        "notify" => ["waiting", "done", "error"].iter().all(|k| v.get(k).is_some_and(Value::is_boolean)),
        "widgetsOff" | "widgetOrder" => v.as_array().is_some_and(|ids| ids.len() <= 64 && ids.iter().all(|id| id.as_str().is_some_and(crate::widgets::is_id))),
        "widgetSpin" => matches!(v.as_u64(), Some(0 | 5 | 8 | 15)),
        "plugins" => scripts::is_ok(v),
        "monitor" => v.as_object().is_some_and(|m| {
            m.iter().all(|(k, v)| match k.as_str() {
                "cpu" | "mem" | "net" | "battery" => v.is_boolean(),
                "every" => v.as_u64().is_some_and(|s| (1..=10).contains(&s)),
                _ => false,
            })
        }),
        "widgetNudge" => is_bool,
        _ => false,
    }
}

// A patch from the settings: checked, with the size, the display and her
// being out through the windows, so she keeps her spot.
pub fn apply_patch(sh: &Arc<Shared>, patch: &Value) -> Value {
    let mut rest = Map::new();
    let (mut scale, mut display, mut out) = (None, None, None);
    for (key, value) in patch.as_object().into_iter().flatten() {
        if !is_ok(sh, key, value) {
            continue;
        }
        match key.as_str() {
            "scale" => scale = value.as_f64(),
            "display" => display = value.as_str().map(str::to_string),
            "out" => out = value.as_bool(),
            _ => {
                rest.insert(key.clone(), value.clone());
            }
        }
    }
    if let Some(scale) = scale {
        pet::resize(sh, scale);
    }
    if let Some(display) = display {
        set_display(sh, &display);
    }
    if let Some(out) = out {
        set_out(sh, out);
    }
    if rest.contains_key("dnd") && rest["dnd"] == true && sh.asks.lock().unwrap().dismiss_all() {
        sh.push_asks();
    }
    let plugins = rest.contains_key("plugins");
    if !rest.is_empty() {
        sh.change(Value::Object(rest));
    }
    if plugins && scripts::sync(sh) {
        sh.redraw();
    }
    snapshot(sh)
}

#[tauri::command]
pub async fn settings_get(app: AppHandle) -> Value {
    snapshot(&shared(&app))
}

#[tauri::command]
pub async fn settings_set(app: AppHandle, patch: Value) -> Value {
    apply_patch(&shared(&app), &patch)
}

// Open them from anywhere (the tray, her double-click): on a page, or where they were.
#[tauri::command]
pub async fn settings_open(app: AppHandle, tab: Option<String>) {
    island::open_settings(&shared(&app), tab.as_deref());
}

// The page closed them: the keyboard goes back, and a risen island goes.
#[tauri::command]
pub async fn settings_closed(app: AppHandle, window: WebviewWindow) {
    let sh = shared(&app);
    island::settings_closed(&sh);
    sh.set_keyboard(&window, false);
}

#[tauri::command]
pub async fn settings_show_pet(app: AppHandle, on: bool) -> Value {
    let sh = shared(&app);
    sh.set_hidden(!on);
    snapshot(&sh)
}

#[tauri::command]
pub async fn settings_login(app: AppHandle, on: bool) -> Value {
    let sh = shared(&app);
    if let Err(err) = connection::set_open_at_login(on) {
        sh.log(&format!("start at login: {err}"));
    }
    snapshot(&sh)
}

// install, install-http, remove: Claude Code's settings.json.
#[tauri::command]
pub async fn settings_hooks(app: AppHandle, action: String) -> Value {
    let sh = shared(&app);
    match connection::write_hooks(&action, sh.port) {
        Ok(()) => json!({ "ok": true, "snapshot": snapshot(&sh) }),
        Err(error) => json!({ "ok": false, "error": error, "snapshot": snapshot(&sh) }),
    }
}

// install, remove: Codex's hooks.json.
#[tauri::command]
pub async fn settings_codex_hooks(app: AppHandle, action: String) -> Value {
    let sh = shared(&app);
    match connection::write_codex_hooks(&action, &i18n::t(sh.lang(), "codex.waiting")) {
        Ok(()) => json!({ "ok": true, "snapshot": snapshot(&sh) }),
        Err(error) => json!({ "ok": false, "error": error, "snapshot": snapshot(&sh) }),
    }
}

// Download a pet from codex-pets.net: the first one, or one replacing a
// missing sprite, becomes her.
#[tauri::command]
pub async fn settings_fetch(app: AppHandle, reference: String) -> Value {
    let sh = shared(&app);
    let dir = sh.dir.join("pets");
    let got = tauri::async_runtime::spawn_blocking(move || fetch::download_pet(&reference, &dir)).await;
    let lang = sh.lang();
    match got {
        Ok(Ok(pet)) => {
            let current = sh.setting("pet");
            let found = data::pets(&sh.dir);
            let has_current = found.iter().any(|p| Some(p.id.as_str()) == current.as_str());
            if !has_current || found.len() == 1 {
                sh.change(json!({ "pet": pet["id"] }));
            } else {
                sh.redraw();
            }
            let warning = pet["warning"]
                .as_object()
                .map(|w| fetch::describe(lang, &fetch::PetError { code: if w["code"] == "atlas" { "atlas" } else { "unknown" }, vars: w["vars"].clone() }));
            json!({ "ok": true, "pet": pet, "warning": warning, "snapshot": snapshot(&sh) })
        }
        Ok(Err(e)) => json!({ "ok": false, "error": fetch::describe(lang, &e), "snapshot": snapshot(&sh) }),
        Err(e) => json!({ "ok": false, "error": e.to_string(), "snapshot": snapshot(&sh) }),
    }
}

#[tauri::command]
pub async fn settings_gallery(app: AppHandle, page: Option<u64>, sort: Option<String>) -> Value {
    let sh = shared(&app);
    let sort = sort.unwrap_or_default();
    let got = tauri::async_runtime::spawn_blocking(move || fetch::gallery(page.unwrap_or(1), &sort)).await;
    match got {
        Ok(Ok(page)) => json!({ "ok": true, "items": page["items"], "page": page["page"], "totalPages": page["totalPages"] }),
        Ok(Err(e)) => json!({ "ok": false, "error": fetch::describe(sh.lang(), &e) }),
        Err(e) => json!({ "ok": false, "error": e.to_string() }),
    }
}

// Only these two places, whatever the page asks: the project, or codex-pets.net.
#[tauri::command]
pub async fn settings_open_site(app: AppHandle, place: String) {
    let url = if place == "repo" { REPO } else { fetch::SITE };
    let mut command = std::process::Command::new("cmd");
    command.args(["/C", "start", "", url]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW
        command.creation_flags(0x0800_0000);
    }
    if let Err(err) = command.spawn() {
        shared(&app).log(&format!("open {url}: {err}"));
    }
}
