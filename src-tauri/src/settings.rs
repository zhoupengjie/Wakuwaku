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
    let (win_light, win_accent) = crate::wallpaper::look();
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
        // Windows' own mode and accent colour (settings_win_look sets them).
        "winLook": { "light": win_light, "accent": crate::theme::current_accent().or(win_accent.map(|a| a.base)) },
        // The desktop's picture as it is (paper.rs).
        "winPaper": crate::paper::now(),
        "version": sh.app.package_info().version.to_string(),
        // Her updates: this version, the latest, an update under way (updates.rs).
        "update": crate::updates::view(&sh),
        // Her crash reports: as set, those waiting, the last sent (crash.rs).
        "crash": crate::crash::view(sh),
        "fullscreenAvailable": fullscreen::AVAILABLE,
        "widgets": sh.widgets_view(),
        // The plugins she runs, and where waku is for the terminal.
        "plugins": scripts::view(sh),
        // The mail accounts, each with how its watch is going.
        "mail": mail::view(sh),
        // Who a letter can go to (claude, codex: found or not), and the
        // letters being read in the background or read this time.
        "mailAgents": mail::agent::available(),
        // The models each can be asked for, with the efforts they take.
        "mailModels": mail::agent::models(),
        "mailTalks": mail::agent::talks(sh),
        // The mail kept changed: the Mail page asks for its list again.
        "mailRev": mail::rev(),
        "waku": scripts::waku_path(sh).to_string_lossy(),
        "port": sh.port,
    })
}

// One mode's island look: its ground (#rrggbb), how solid (20 to 100 %),
// how blurred what is behind (0 to 60 px), its edge, no capsule collapsed;
// each part may be left out (its default), nothing else.
fn is_island_look(v: &Value) -> bool {
    v.is_null()
        || v.as_object().is_some_and(|m| {
            m.iter().all(|(k, v)| match k.as_str() {
                "bg" => v.as_str().is_some_and(|s| s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())),
                "op" => v.as_u64().is_some_and(|n| (20..=100).contains(&n)),
                "blur" => v.as_u64().is_some_and(|n| n <= 60),
                "edge" => matches!(v.as_str(), Some("line" | "ring" | "none")),
                "bare" => v.is_boolean(),
                _ => false,
            })
        })
}

// The taskbar's layout: { left, center, right }, each a list of its parts
// (island.js TB_PARTS), none twice and nothing else; one left out goes where
// the default has it.
const TASKBAR_PARTS: [&str; 9] = ["her", "start", "apps", "monitor", "tray", "keys", "quick", "clock", "gear"];

// A widget put on the taskbar (widgets.js isEntry): { id, type, of, size,
// name, icon, cfg }, its id "w" and up to 15 of a-z 0-9; a widget's or a
// plugin's (of: its id), one of the taskbar's own (tool, of: its kind,
// tools.rs KINDS; cfg: its settings, a few short values), or a spacer
// (flex, gap, sep).
fn is_taskbar_widget(v: &Value) -> Option<&str> {
    widget_id(v, false)
}

// The same, L (2 × 2) allowed too when large: one in her island's grid.
fn widget_id(v: &Value, large: bool) -> Option<&str> {
    let w = v.as_object()?;
    let id = w.get("id")?.as_str()?;
    let ok_id = id.len() >= 2 && id.len() <= 16 && id.starts_with('w') && id[1..].chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    let kind = w.get("type")?.as_str()?;
    let of = w.get("of").map(|of| of.as_str().filter(|of| crate::widgets::is_id(of)));
    let ok_of = match kind {
        "widget" => matches!(of, Some(Some(_))),
        "plugin" => matches!(of, Some(Some(of)) if crate::scripts::SCRIPTS.iter().any(|s| s.id == of)),
        "tool" => matches!(of, Some(Some(of)) if crate::tools::KINDS.contains(&of)),
        "flex" | "gap" | "sep" => of.is_none(),
        _ => false,
    };
    let ok_rest = w.iter().all(|(key, v)| match key.as_str() {
        "id" | "type" | "of" => true,
        "size" => matches!(v.as_str(), Some("S" | "M")) || (large && v.as_str() == Some("L")),
        "name" => v.as_str().is_some_and(|n| n.chars().count() <= 40 && !n.chars().any(char::is_control)),
        "icon" => v.as_str().is_some_and(|i| crate::widgets::ICONS.contains(&i)),
        "cfg" => kind == "tool" && is_tool_cfg(v),
        _ => false,
    });
    (ok_id && ok_of && ok_rest).then_some(id)
}

// A widget there and its ids: its own, and a stack's (2 to 8 widgets, one
// shown at a time; smart: turning by itself) its own and its items'.
fn taskbar_widget_ids(v: &Value) -> Option<Vec<&str>> {
    widget_ids(v, false)
}

fn widget_ids(v: &Value, large: bool) -> Option<Vec<&str>> {
    let w = v.as_object()?;
    if w.get("type")?.as_str()? != "stack" {
        return widget_id(v, large).map(|id| vec![id]);
    }
    let id = w.get("id")?.as_str()?;
    let ok_id = id.len() >= 2 && id.len() <= 16 && id.starts_with('w') && id[1..].chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    let items = w.get("items")?.as_array()?;
    let ok_rest = w.iter().all(|(key, v)| match key.as_str() {
        "id" | "type" | "items" => true,
        "size" => matches!(v.as_str(), Some("S" | "M")) || (large && v.as_str() == Some("L")),
        "smart" => v.is_boolean(),
        _ => false,
    });
    if !ok_id || !ok_rest || !(2..=8).contains(&items.len()) {
        return None;
    }
    let mut ids = vec![id];
    for item in items {
        // A widget, a plugin's or a tool: no spacer, no stack in a stack.
        if !matches!(item.get("type").and_then(Value::as_str), Some("widget" | "plugin" | "tool")) {
            return None;
        }
        ids.push(widget_id(item, large)?);
    }
    Some(ids)
}

// Her island's grid (island.js): its widgets and stacks, no spacers, each
// once, 32 at most (a stack's items each one).
fn is_island_grid(v: &Value) -> bool {
    let Some(list) = v.as_array() else { return false };
    let mut seen: Vec<&str> = Vec::new();
    let mut widgets = 0;
    list.iter().all(|p| {
        let spacer = matches!(p.get("type").and_then(Value::as_str), Some("flex" | "gap" | "sep"));
        match widget_ids(p, true) {
            Some(ids) if !spacer && widgets + ids.len().max(2) - 1 <= 32 && ids.iter().all(|id| !seen.contains(id)) && (1..ids.len()).all(|i| !ids[..i].contains(&ids[i])) => {
                widgets += ids.len().max(2) - 1;
                seen.extend(ids);
                true
            }
            _ => false,
        }
    })
}

// Whether what is kept (a taskbar layout, her island's grid, anything)
// shows this widget: put there by itself, or its plugin's, in a stack too.
pub fn shows(v: &Value, widget: &str) -> bool {
    match v {
        Value::Object(o) => {
            (match (o.get("type").and_then(Value::as_str), o.get("of").and_then(Value::as_str)) {
                (Some("widget"), Some(of)) => of == widget,
                (Some("plugin"), Some(of)) => Some(of) == crate::scripts::owner_of(widget),
                _ => false,
            }) || o.values().any(|v| shows(v, widget))
        }
        Value::Array(a) => a.iter().any(|v| shows(v, widget)),
        _ => false,
    }
}

// A tool's settings (a clock's city, a launcher's program): up to 8, each
// a word for its key and a short text, a number or a flag.
fn is_tool_cfg(v: &Value) -> bool {
    v.as_object().is_some_and(|cfg| {
        cfg.len() <= 8
            && cfg.iter().all(|(key, v)| {
                (1..=16).contains(&key.len())
                    && key.chars().all(|c| c.is_ascii_alphanumeric())
                    && (v.is_boolean() || v.is_number() || v.as_str().is_some_and(|t| t.chars().count() <= 300 && !t.chars().any(char::is_control)))
            })
    })
}

// Whether a taskbar layout shows this widget: put there by itself, or its
// plugin's.
pub fn is_on_taskbar(layout: &Value, widget: &str) -> bool {
    let owner = crate::scripts::owner_of(widget);
    let entries = layout.as_object().into_iter().flat_map(|runs| runs.values()).filter_map(Value::as_array).flatten();
    // A stack's items, each.
    let each = entries.flat_map(|e| match e["items"].as_array() {
        Some(items) if e["type"] == "stack" => items.iter().collect::<Vec<_>>(),
        _ => vec![e],
    });
    each.into_iter().any(|e| match (e["type"].as_str(), e["of"].as_str()) {
        (Some("widget"), Some(of)) => of == widget,
        (Some("plugin"), Some(of)) => Some(of) == owner,
        _ => false,
    })
}

// Its runs: the parts each once, and up to 32 widgets, each once.
fn is_taskbar_layout(v: &Value) -> bool {
    let Some(runs) = v.as_object() else { return false };
    let mut seen: Vec<&str> = Vec::new();
    let mut widgets = 0;
    runs.iter().all(|(run, parts)| {
        matches!(run.as_str(), "left" | "center" | "right")
            && parts.as_array().is_some_and(|parts| {
                parts.iter().all(|p| match (p.as_str(), is_taskbar_widget(p)) {
                    (Some(p), _) if TASKBAR_PARTS.contains(&p) && !seen.contains(&p) => {
                        seen.push(p);
                        true
                    }
                    (None, _) => match taskbar_widget_ids(p) {
                        // Each once; 32 at most, a stack's items each one.
                        Some(ids) if widgets + ids.len().max(2) - 1 <= 32 && ids.iter().all(|id| !seen.contains(id)) && (1..ids.len()).all(|i| !ids[..i].contains(&ids[i])) => {
                            widgets += ids.len().max(2) - 1;
                            seen.extend(ids);
                            true
                        }
                        _ => false,
                    },
                    _ => false,
                })
            })
    })
}

// What a settings patch may hold, checked: anything else is dropped.
fn is_ok(sh: &Shared, key: &str, v: &Value) -> bool {
    let is_bool = v.is_boolean();
    match key {
        "lang" => matches!(v.as_str(), Some("auto" | "zh" | "en")),
        "pet" => v.as_str().is_some_and(|id| data::pets(&sh.dir).iter().any(|p| p.id == id)),
        "scale" => v.as_f64().is_some_and(|s| data::SCALES.iter().any(|(_, x)| (x - s).abs() < 1e-9)),
        "details" | "walk" | "look" | "sound" | "dnd" | "hideInFullscreen" | "onboarded" | "out" | "settingsPin" | "settingsTop" | "oledShift" | "autoUpdateCheck" => is_bool,
        // Where an update is downloaded from first (updates.rs gitee_first).
        "updateFrom" => matches!(v.as_str(), Some("auto" | "github" | "gitee")),
        // Crash reports sent by themselves, never, or asked about (crash.rs).
        "crashReports" => matches!(v.as_str(), Some("auto" | "off" | "ask")),
        "display" => matches!(v.as_str(), Some("island" | "taskbar")),
        "islandWidth" => matches!(v.as_str(), Some("narrow" | "normal" | "wide")),
        "taskbarMaterial" => matches!(v.as_str(), Some("mica" | "black" | "clear")),
        "taskbarClearWhen" => matches!(v.as_str(), Some("mica" | "solid" | "always")),
        "taskbarButtons" => matches!(v.as_str(), Some("icons" | "labels")),
        "taskbarAlign" => matches!(v.as_str(), Some("center" | "left")),
        // The taskbar's parts in three runs (island.js tbLayout), each once.
        "taskbarLayout" => v.is_null() || is_taskbar_layout(v),
        // The widgets in her island's grid, in the order they fill it.
        "islandGrid" => v.is_null() || is_island_grid(v),
        // The notes on the taskbar, by the widget's id: up to 64, 4000 characters each.
        "notes" => v.as_object().is_some_and(|n| n.len() <= 64 && n.iter().all(|(id, text)| id.len() <= 16 && text.as_str().is_some_and(|t| t.chars().count() <= 4000))),
        // The colours picked last (tools.rs picker), up to 12.
        "pickedColors" => v.as_array().is_some_and(|c| c.len() <= 12 && c.iter().all(|c| c.as_str().is_some_and(|c| c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|d| d.is_ascii_hexdigit())))),
        // The programs' buttons in the order dragged: their keys (paths).
        "taskbarOrder" => v.as_array().is_some_and(|keys| keys.len() <= 200 && keys.iter().all(|k| k.as_str().is_some_and(|s| s.len() <= 1024))),
        "hold" => v.as_str() == Some("seen") || matches!(v.as_u64(), Some(8 | 30 | 120)),
        "promptWaitSec" => matches!(v.as_u64(), Some(30 | 60 | 120 | 290)),
        "notify" => ["waiting", "done", "error"].iter().all(|k| v.get(k).is_some_and(Value::is_boolean)),
        "widgetsOff" | "widgetOrder" => v.as_array().is_some_and(|ids| ids.len() <= 64 && ids.iter().all(|id| id.as_str().is_some_and(crate::widgets::is_id))),
        "widgetSpin" => matches!(v.as_u64(), Some(0 | 5 | 8 | 15)),
        "plugins" => scripts::is_ok(v),
        "monitor" => v.as_object().is_some_and(|m| {
            m.iter().all(|(k, v)| match k.as_str() {
                "cpu" | "mem" | "net" | "battery" | "pin" => v.is_boolean(),
                "every" => v.as_u64().is_some_and(|s| (1..=10).contains(&s)),
                _ => false,
            })
        }),
        "widgetNudge" => is_bool,
        // Her island's look on the taskbar, by Windows' mode (island.js
        // drawIslandLook): none, or a mode's none, is Windows' own grey.
        "islandLook" => v.is_null() || v.as_object().is_some_and(|m| m.iter().all(|(mode, look)| matches!(mode.as_str(), "dark" | "light") && is_island_look(look))),
        "mailAgent" => matches!(v.as_str(), Some("claude" | "codex")),
        // The Mail page in two panes (list, letter), or one.
        "mailPanes" => matches!(v.as_str(), Some("one" | "two")),
        // How much mail is kept whole offline (mail/store.rs Offline).
        "mailOffline" => matches!(v.as_str(), Some("90d" | "year" | "all" | "opened")),
        "mailAgentConf" => mail::agent::is_conf_ok(v),
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
    let tools = rest.contains_key("taskbarLayout") || rest.contains_key("islandGrid") || display.is_some();
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
    let offline = rest.contains_key("mailOffline");
    let crash_auto = rest.get("crashReports").and_then(Value::as_str) == Some("auto");
    if !rest.is_empty() {
        sh.change(Value::Object(rest));
    }
    // Crash reports turned on: those waiting sent now.
    if crash_auto {
        crate::crash::send_now(sh);
    }
    if plugins && scripts::sync(sh) {
        sh.redraw();
    }
    // A tool put out (or taken away): read now, not at its next look.
    if tools {
        crate::tools::poke();
    }
    // Mail kept offline for more or less: each account's sync at it now,
    // once the setting is there to be read.
    if offline {
        mail::kick_all(sh);
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

// Windows' own light or dark mode, or its accent colour ("#rrggbb"), set
// as Settings → Personalization → Colors sets them (theme.rs).
#[tauri::command]
pub async fn settings_win_look(app: AppHandle, light: Option<bool>, accent: Option<String>) -> Value {
    let sh = shared(&app);
    if let Some(light) = light {
        let ok = crate::theme::set_mode(light);
        sh.log(&format!("settings: Windows' mode set {}: {ok}", if light { "light" } else { "dark" }));
    }
    if let Some(rgb) = accent.as_deref().and_then(crate::theme::parse) {
        let picked = accent.unwrap_or_default();
        let (later_sh, later_picked) = (sh.clone(), picked.clone());
        let came = crate::theme::set_accent(rgb, move |note| later_sh.log(&format!("settings: Windows' accent {later_picked}: {note}")));
        sh.log(&format!("settings: Windows' accent set {picked}: {came}"));
    }
    snapshot(&sh)
}

// The desktop's picture (paper.rs): a picture or a folder picked in Windows'
// file dialog ("picture", "folder"), how it is laid ("position"), the
// folder's turns ("every", "shuffle"), the next or the one before ("next",
// "previous"); a change of these undone ("undo").
#[tauri::command]
pub async fn settings_wallpaper(app: AppHandle, what: String, value: Option<String>) -> Value {
    let sh = shared(&app);
    let owner = crate::island::hwnd(&sh);
    let lang = sh.lang();
    let t = move |key: &str| crate::i18n::t(lang, key);
    let what_done = what.clone();
    // What changes the picture is kept to be undone ("undo"), once picked.
    let changed = |done: bool| {
        crate::paper::remember();
        done
    };
    let done = tauri::async_runtime::spawn_blocking(move || match what.as_str() {
        "picture" => crate::paper::pick(false, &t("wallpaper.pickPicture"), &t("wallpaper.pictures"), owner).map(|p| (changed(true) && crate::paper::set_picture(&p), p)),
        "folder" => crate::paper::pick(true, &t("wallpaper.pickFolder"), "", owner).map(|p| (changed(true) && crate::paper::set_folder(&p), p)),
        "position" => value.map(|v| (changed(true) && crate::paper::set_position(&v), v)),
        "every" => value.map(|v| (crate::paper::set_turns(v.parse().ok(), None), v)),
        "shuffle" => value.map(|v| (crate::paper::set_turns(None, Some(v == "true")), v)),
        "next" => Some((changed(true) && crate::paper::next(), String::new())),
        "previous" => Some((changed(true) && crate::paper::previous(), String::new())),
        "undo" => Some((crate::paper::undo(), String::new())),
        _ => None,
    })
    .await
    .ok()
    .flatten();
    for line in crate::paper::notes() {
        sh.log(&format!("settings: wallpaper: {line}"));
    }
    if let Some((ok, value)) = done {
        sh.log(&format!("settings: wallpaper {what_done} {value}: {ok}"));
        // The taskbar's Mica follows the new picture: Windows says nothing
        // of a change made through IDesktopWallpaper, and the taskbar would
        // look on its own only every half minute. A moment after, for the
        // picture to be in place.
        if ok {
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_millis(600));
                crate::taskbar::look_now();
            });
        }
    }
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
pub async fn settings_gallery(app: AppHandle, page: Option<u64>, sort: Option<String>, query: Option<String>) -> Value {
    let sh = shared(&app);
    let sort = sort.unwrap_or_default();
    let query = query.unwrap_or_default();
    let got = tauri::async_runtime::spawn_blocking(move || fetch::gallery(page.unwrap_or(1), &sort, &query)).await;
    match got {
        Ok(Ok(page)) => json!({ "ok": true, "items": page["items"], "page": page["page"], "totalPages": page["totalPages"], "total": page["total"] }),
        Ok(Err(e)) => json!({ "ok": false, "error": fetch::describe(sh.lang(), &e) }),
        Err(e) => json!({ "ok": false, "error": e.to_string() }),
    }
}

// Her updates, from the General page: look now, the latest release's page,
// install it (updates.rs). The snapshot as it is then (a look or an
// install goes on on its own thread, pushed as it goes).
#[tauri::command]
pub async fn settings_update(app: AppHandle, what: String) -> Value {
    let sh = shared(&app);
    match what.as_str() {
        "check" => {
            let sh = sh.clone();
            std::thread::spawn(move || crate::updates::check_now(&sh));
            // Its "looking" shown at once.
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        "page" => crate::updates::open_page(&sh),
        "update" => crate::updates::update_now(&sh),
        _ => {}
    }
    snapshot(&sh)
}

// Her crash reports, from the General page: the answer to her asking
// (yes: sent from now on; no: never), sent now, or the last one's issue
// opened (crash.rs). The snapshot as it is then.
#[tauri::command]
pub async fn settings_crash(app: AppHandle, what: String) -> Value {
    let sh = shared(&app);
    match what.as_str() {
        "yes" => crate::crash::answer(&sh, true),
        "no" => crate::crash::answer(&sh, false),
        "send" => crate::crash::send_now(&sh),
        "open" => {
            if let Some(url) = crate::crash::open_last(&sh) {
                crate::jump::open_url(&url);
            }
        }
        _ => {}
    }
    snapshot(&sh)
}

// Only these places, whatever the page asks: the project, codex-pets.net,
// or where a mail provider makes app passwords (the Mail page's hint).
#[tauri::command]
pub async fn settings_open_site(app: AppHandle, place: String) {
    let url = match place.as_str() {
        "repo" => REPO,
        "googleAppPasswords" => "https://myaccount.google.com/apppasswords",
        "appleAppPasswords" => "https://account.apple.com/account/manage",
        "yahooAppPasswords" => "https://login.yahoo.com/account/security",
        _ => fetch::SITE,
    };
    // By ShellExecute, not `cmd /c start`: run from the installed pet (no
    // console), cmd got handles that were none and never started ("the
    // handle is invalid").
    if !crate::jump::open_url(url) {
        shared(&app).log(&format!("open {url}: ShellExecute failed"));
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn island_looks() {
        assert!(super::is_island_look(&json!(null)));
        assert!(super::is_island_look(&json!({})));
        assert!(super::is_island_look(&json!({ "bg": "#2C2c2c", "op": 62, "blur": 40, "edge": "ring", "bare": true })));
        assert!(!super::is_island_look(&json!({ "bg": "#2c2c2" })));
        assert!(!super::is_island_look(&json!({ "bg": "red" })));
        assert!(!super::is_island_look(&json!({ "op": 10 })));
        assert!(!super::is_island_look(&json!({ "blur": 61 })));
        assert!(!super::is_island_look(&json!({ "edge": "glow" })));
        assert!(!super::is_island_look(&json!({ "bare": 1 })));
        assert!(!super::is_island_look(&json!({ "ink": "light" })));
    }
}

#[cfg(test)]
mod layout_tests {
    use serde_json::json;

    #[test]
    fn taskbar_layouts() {
        assert!(super::is_taskbar_layout(&json!({ "left": ["her"], "center": ["start", "apps"], "right": ["monitor", "tray", "keys", "quick", "clock", "gear"] })));
        assert!(super::is_taskbar_layout(&json!({ "left": ["start", "apps"], "right": ["clock", "her"] })));
        assert!(!super::is_taskbar_layout(&json!({ "left": ["her", "her"] })));
        assert!(!super::is_taskbar_layout(&json!({ "left": ["her"], "right": ["her"] })));
        assert!(!super::is_taskbar_layout(&json!({ "top": ["her"] })));
        assert!(!super::is_taskbar_layout(&json!({ "left": ["desk"] })));
        assert!(!super::is_taskbar_layout(&json!({ "left": "her" })));
        assert!(!super::is_taskbar_layout(&json!(["her"])));
    }

    #[test]
    fn widgets_on_the_taskbar() {
        let ok = |w| super::is_taskbar_layout(&json!({ "right": [w, "clock"] }));
        assert!(ok(json!({ "id": "wa1", "type": "widget", "of": "today", "size": "M" })));
        assert!(ok(json!({ "id": "wa1", "type": "widget", "of": "inbox-x2", "name": "me@example.com", "icon": "mail" })));
        assert!(ok(json!({ "id": "wa1", "type": "plugin", "of": "weather", "size": "S", "icon": "weather" })));
        assert!(ok(json!({ "id": "wa1", "type": "flex" })));
        assert!(ok(json!({ "id": "wa1", "type": "sep" })));
        // Not a plugin she runs; no widget's id; a spacer of something.
        assert!(!ok(json!({ "id": "wa1", "type": "plugin", "of": "nope" })));
        assert!(!ok(json!({ "id": "wa1", "type": "widget" })));
        assert!(!ok(json!({ "id": "wa1", "type": "widget", "of": "Not An Id" })));
        assert!(!ok(json!({ "id": "wa1", "type": "gap", "of": "today" })));
        // Its id, size, icon, name; nothing else.
        assert!(!ok(json!({ "id": "start", "type": "flex" })));
        assert!(!ok(json!({ "id": "w", "type": "flex" })));
        assert!(!ok(json!({ "id": "wa1", "type": "widget", "of": "today", "size": "L" })));
        assert!(!ok(json!({ "id": "wa1", "type": "widget", "of": "today", "icon": "nope" })));
        assert!(!ok(json!({ "id": "wa1", "type": "widget", "of": "today", "name": "x".repeat(41) })));
        assert!(!ok(json!({ "id": "wa1", "type": "flex", "colour": "#fff" })));
        assert!(!ok(json!({ "id": "wa1", "type": "clock" })));
        // Each once, 32 at most.
        assert!(!super::is_taskbar_layout(&json!({ "left": [{ "id": "wa1", "type": "sep" }], "right": [{ "id": "wa1", "type": "sep" }] })));
        let many: Vec<_> = (0..33).map(|i| json!({ "id": format!("w{i}"), "type": "gap" })).collect();
        assert!(!super::is_taskbar_layout(&json!({ "right": many })));
        assert!(super::is_taskbar_layout(&json!({ "right": many[..32] })));
    }

    #[test]
    fn the_taskbars_own_widgets() {
        let ok = |w| super::is_taskbar_layout(&json!({ "right": [w, "clock"] }));
        assert!(ok(json!({ "id": "wa1", "type": "tool", "of": "media", "size": "M" })));
        assert!(ok(json!({ "id": "wa1", "type": "tool", "of": "clock", "cfg": { "tz": "Asia/Tokyo", "style": "analog" } })));
        assert!(ok(json!({ "id": "wa1", "type": "tool", "of": "launch", "cfg": { "path": "C:\\Windows\\notepad.exe", "name": "记事本" } })));
        assert!(!ok(json!({ "id": "wa1", "type": "tool", "of": "nope" })));
        assert!(!ok(json!({ "id": "wa1", "type": "tool" })));
        // Settings for a tool only, a few short ones.
        assert!(!ok(json!({ "id": "wa1", "type": "widget", "of": "today", "cfg": {} })));
        assert!(!ok(json!({ "id": "wa1", "type": "tool", "of": "clock", "cfg": { "tz": "x".repeat(301) } })));
        assert!(!ok(json!({ "id": "wa1", "type": "tool", "of": "clock", "cfg": { "a-b": 1 } })));
        assert!(!ok(json!({ "id": "wa1", "type": "tool", "of": "clock", "cfg": { "a": [1] } })));
    }

    #[test]
    fn stacks() {
        let w = |id: &str, of: &str| json!({ "id": id, "type": "widget", "of": of });
        let t = |id: &str, of: &str| json!({ "id": id, "type": "tool", "of": of });
        let ok = |s| super::is_taskbar_layout(&json!({ "right": [s, "clock"] }));
        assert!(ok(json!({ "id": "wa", "type": "stack", "items": [w("wb", "today"), t("wc", "media")], "size": "M", "smart": true })));
        // Two to eight, each once, no spacer or stack inside, nothing else.
        assert!(!ok(json!({ "id": "wa", "type": "stack", "items": [w("wb", "today")] })));
        assert!(!ok(json!({ "id": "wa", "type": "stack", "items": (0..9).map(|i| t(&format!("w{i}"), "clock")).collect::<Vec<_>>() })));
        assert!(!ok(json!({ "id": "wa", "type": "stack", "items": [w("wb", "today"), w("wb", "tokens")] })));
        assert!(!ok(json!({ "id": "wa", "type": "stack", "items": [w("wa", "today"), w("wb", "tokens")] })));
        assert!(!ok(json!({ "id": "wa", "type": "stack", "items": [w("wb", "today"), json!({ "id": "wc", "type": "sep" })] })));
        assert!(!ok(json!({ "id": "wa", "type": "stack", "items": [w("wb", "today"), json!({ "id": "wc", "type": "stack", "items": [w("wd", "a"), w("we", "b")] })] })));
        assert!(!ok(json!({ "id": "wa", "type": "stack", "items": [w("wb", "today"), w("wc", "tokens")], "cfg": {} })));
        // An item's id once in the whole layout.
        assert!(!super::is_taskbar_layout(&json!({ "left": [w("wb", "x")], "right": [{ "id": "wa", "type": "stack", "items": [w("wb", "today"), w("wc", "tokens")] }] })));
        // What the taskbar shows: a stack's items too.
        assert!(super::is_on_taskbar(&json!({ "right": [{ "id": "wa", "type": "stack", "items": [w("wb", "today"), w("wc", "tokens")] }] }), "tokens"));
    }

    #[test]
    fn her_islands_grid() {
        let w = |id: &str, of: &str, size: &str| json!({ "id": id, "type": "widget", "of": of, "size": size });
        assert!(super::is_island_grid(&json!([w("wa", "today", "L"), w("wb", "tokens", "S"), { "id": "wc", "type": "tool", "of": "media", "size": "L" }])));
        assert!(super::is_island_grid(&json!([{ "id": "wa", "type": "stack", "size": "L", "items": [w("wb", "today", "L"), w("wc", "tokens", "M")] }])));
        assert!(super::is_island_grid(&json!([])));
        // No spacers there; each once; L only there.
        assert!(!super::is_island_grid(&json!([{ "id": "wa", "type": "gap" }])));
        assert!(!super::is_island_grid(&json!([w("wa", "today", "L"), w("wa", "tokens", "S")])));
        assert!(!super::is_island_grid(&json!({ "left": [] })));
        assert!(!super::is_taskbar_layout(&json!({ "right": [w("wa", "today", "L")] })));
        // What it shows, as the strip's.
        assert!(super::shows(&json!([w("wa", "tokens", "S")]), "tokens"));
        assert!(super::shows(&json!([{ "id": "wa", "type": "plugin", "of": "stock" }]), "stock-aapl"));
        assert!(!super::shows(&json!([w("wa", "today", "S")]), "tokens"));
    }

    #[test]
    fn what_the_taskbar_shows() {
        let layout = json!({ "left": ["her"], "right": [{ "id": "wa", "type": "widget", "of": "tokens" }, { "id": "wb", "type": "plugin", "of": "stock" }, "clock"] });
        assert!(super::is_on_taskbar(&layout, "tokens"));
        assert!(super::is_on_taskbar(&layout, "stock-aapl"));
        assert!(!super::is_on_taskbar(&layout, "today"));
        assert!(!super::is_on_taskbar(&layout, "weather"));
        assert!(!super::is_on_taskbar(&json!(null), "tokens"));
    }
}
