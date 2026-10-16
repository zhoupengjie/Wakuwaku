// The settings, grown out of the island (src/pet/settings.js): what they
// show (a snapshot, pushed again as things change) and what they change.
use std::sync::atomic::Ordering;
use std::sync::Arc;

use serde_json::{json, Map, Value};
use tauri::{AppHandle, WebviewWindow};

use crate::{connection, data, fetch, fullscreen, i18n, island, mail, now_ms, pet, scripts, set_display, set_out, shared, Shared};

const REPO: &str = "https://github.com/zhoupengjie/wakuwaku";

// Each agent's own page, by its id (src/pet/agents.js), for the Agent page's
// other supported tools: the only pages it may open.
const AGENT_SITES: [(&str, &str); 7] = [
    ("claude", "https://claude.com/product/claude-code"),
    ("codex", "https://developers.openai.com/codex"),
    ("agy", "https://antigravity.google/"),
    ("opencode", "https://opencode.ai/"),
    ("omp", "https://github.com/can1357/oh-my-pi"),
    ("zcode", "https://zcode.z.ai/"),
    ("dsh", "https://github.com/deepseek-ai/deepseek-harness"),
];

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
    // Whether either is connected, as the island tells it (connection.rs).
    let conn = connection::connection(sh.port);
    let omp = connection::omp_hooks_status(sh.port);
    if connection::note_connected(conn != "none" || connection::is_codex_connected() || connection::is_agy_connected() || connection::is_opencode_connected(sh.port) || matches!(omp, "ok" | "stale" | "partial")) {
        sh.redraw();
    }
    let omp_files = connection::omp_files();
    // The monitor's source in effect: the one chosen, else LHM while it answers.
    let lhm = crate::lhm::view();
    let lhm_status = lhm["status"].as_str().unwrap_or("").to_string();
    let monitor_source = crate::hw::Source::of(&sh.setting("monitor"), &sh.setting("monitorSource"), lhm_status == "found");
    let lhm_shown = monitor_source == crate::hw::Source::Lhm && sh.island.lock().unwrap().settings_open;
    json!({
        "settings": Value::Object(sh.settings.lock().unwrap().clone()),
        "lang": sh.lang(),
        "pets": pets,
        "now": now,
        "sessions": sessions,
        "asks": sh.asks.lock().unwrap().views(),
        "isVisible": sh.is_visible(),
        "hidden": sh.hidden.load(Ordering::SeqCst),
        "connection": conn,
        "pluginCommands": connection::PLUGIN_COMMANDS,
        "hooks": connection::hooks_status(sh.port),
        // Whether any Claude Code event came since she started: installed,
        // and a session seen (connected), or still waiting for one.
        "claudeSeen": sh.claude_seen.load(Ordering::SeqCst) > 0,
        "settingsFile": connection::claude_settings_file().to_string_lossy(),
        // The same as read (~ for the home folder), whether a copy of it was
        // kept (none: there was no file before), and whether Claude Code is
        // on this computer at all (its folder): the welcome offers the ones
        // that are.
        "settingsShown": connection::shown_path(&connection::claude_settings_file()),
        "settingsBackup": connection::backup_of(&connection::claude_settings_file()).exists(),
        "claudeHere": connection::claude_here(),
        // Codex's hooks.json, whether any Codex event came (they run once
        // trusted), and its version: one before 0.148 gets no background hooks.
        "codex": {
            "hooks": connection::codex_hooks_status(),
            "file": connection::codex_hooks_file().to_string_lossy(),
            "fileShown": connection::shown_path(&connection::codex_hooks_file()),
            "seen": sh.codex_seen.load(Ordering::SeqCst) > 0,
            "version": connection::codex_version().map(|(a, b, c)| format!("{a}.{b}.{c}")),
            "async": connection::codex_runs_async(),
        },
        // Antigravity's hooks.json (connection.rs), the same, and whether it
        // is on this computer at all (agy's folder, or its 2.0 app's).
        "agy": {
            "hooks": connection::agy_hooks_status(),
            "file": connection::agy_hooks_file().to_string_lossy(),
            "fileShown": connection::shown_path(&connection::agy_hooks_file()),
            "backup": connection::backup_of(&connection::agy_hooks_file()).exists(),
            "seen": sh.agy_seen.load(Ordering::SeqCst) > 0,
        },
        // Our plugin in OpenCode's plugin folder (connection.rs): absent (no
        // OpenCode here), missing, ok, stale (another port or version), and
        // whether any of its events came since she started.
        "opencode": {
            "plugin": connection::opencode_status(sh.port),
            "file": connection::opencode_plugin_file().to_string_lossy(),
            "fileShown": connection::shown_path(&connection::opencode_plugin_file()),
            "seen": sh.opencode_seen.load(Ordering::SeqCst) > 0,
        },
        // omp's extension (connection.rs): in its agent folder, and each
        // profile's (more: how many besides the first); whether any omp
        // event came; whether omp is on this computer at all.
        "omp": {
            "hooks": omp,
            "file": omp_files.first().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default(),
            "fileShown": omp_files.first().map(|f| connection::shown_path(f)).unwrap_or_default(),
            "more": omp_files.len().saturating_sub(1),
            "seen": sh.omp_seen.load(Ordering::SeqCst) > 0,
        },
        // ZCode's config.json (connection.rs), the same; absent without its folder.
        "zcode": {
            "hooks": connection::zcode_hooks_status(),
            "file": connection::zcode_config_file().to_string_lossy(),
            "fileShown": connection::shown_path(&connection::zcode_config_file()),
            "backup": connection::backup_of(&connection::zcode_config_file()).exists(),
            "seen": sh.zcode_seen.load(Ordering::SeqCst) > 0,
        },
        // DeepSeek Harness's home-level patch file, where its hook bridge is
        // inserted (connection.rs), the same; absent without its folder.
        "dsh": {
            "hooks": connection::dsh_hooks_status(),
            "file": connection::dsh_patch_file().to_string_lossy(),
            "fileShown": connection::shown_path(&connection::dsh_patch_file()),
            "backup": connection::backup_of(&connection::dsh_patch_file()).exists(),
            // The bridge's hook list, in her data folder (the Agent page shows both).
            "listShown": connection::shown_path(&connection::dsh_hooks_file()),
            "seen": sh.dsh_seen.load(Ordering::SeqCst) > 0,
        },
        // The agents' versions where a file of theirs says (connection.rs
        // versions; Codex's is its block's): the Agent page shows them.
        "agentVersions": connection::versions(),
        // How many displays there are: her taskbar on all of them is offered
        // only with more than one (Look, taskbarDisplays).
        "displayCount": sh.screens().areas.len(),
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
        // Whether Windows' acrylic can go under the open island (backplate.rs).
        "plate": crate::backplate::available(),
        "widgets": sh.widgets_view(),
        // The monitor's hardware sensors: LibreHardwareMonitor found or
        // not, and its sensors (lhm.rs); the built-in ones (hw.rs); which
        // of them it shows: the one chosen, else LHM while it answers.
        "lhm": lhm,
        "hw": crate::hw::view(),
        "monitorSource": monitor_source.name(),
        // The CPU temperature/power helper's state (sensors.rs, #18 part B):
        // off / pawnioMissing / settingUp / running / notAnswering / needsUpdate.
        "cpuSensors": crate::sensors::view(&sh.dir),
        // LibreHardwareMonitor got, set up, started (lhm_setup.rs): what is
        // offered, what was found, a download under way; looked at again
        // while the settings show it.
        "lhmSetup": crate::lhm_setup::view(sh, &lhm_status, lhm_shown),
        // The plugins she runs, and where waku is for the terminal.
        "plugins": scripts::view(sh),
        // The mail accounts, each with how its watch is going.
        "mail": mail::view(sh),
        // Who a letter can go to (claude, codex, agy: found or not; looked
        // for again while the settings are open), and the letters being
        // read in the background or read this time.
        "mailAgents": mail::agent::available(),
        // The models each can be asked for, with the efforts they take.
        "mailModels": mail::agent::models(sh),
        "mailTalks": mail::agent::talks(sh),
        // Windows' proxy as mail would take it (the Mail page's "follow the system").
        "mailSystemProxy": mail::proxy::system_view(),
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
const TASKBAR_PARTS: [&str; 8] = ["her", "start", "apps", "monitor", "tray", "keys", "quick", "clock"];

// A widget put on the taskbar (widgets.js isEntry): { id, type, of, size,
// name, icon }, its id "w" and up to 15 of a-z 0-9; a widget's or a
// plugin's (of: its id), or a spacer (flex, gap, sep); or several in one
// place (carousel: in turn; stack: the first, the rest opened over it),
// { id, type, items }, 2 to 8 of them, each { type, of, name, icon } as a
// widget's or a plugin's are, none twice.
fn is_taskbar_widget(v: &Value) -> Option<&str> {
    let w = v.as_object()?;
    let id = w.get("id")?.as_str()?;
    let ok_id = id.len() >= 2 && id.len() <= 16 && id.starts_with('w') && id[1..].chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    let kind = w.get("type")?.as_str()?;
    let of = w.get("of").map(|of| of.as_str().filter(|of| crate::widgets::is_id(of)));
    let ok_of = match kind {
        "widget" | "plugin" => of.is_some_and(|of| of.is_some_and(|of| is_source_of(kind, of))),
        "flex" | "gap" | "sep" | "carousel" | "stack" => of.is_none(),
        _ => false,
    };
    let group = matches!(kind, "carousel" | "stack");
    let ok_items = match w.get("items").and_then(Value::as_array) {
        Some(items) if group => (2..=8).contains(&items.len()) && items.iter().all(is_member) && items.iter().enumerate().all(|(i, m)| !items[..i].iter().any(|x| x["type"] == m["type"] && x["of"] == m["of"])),
        None => !group,
        _ => false,
    };
    let ok_rest = w.iter().all(|(key, v)| match key.as_str() {
        "id" | "type" | "of" | "items" => true,
        "size" => !group && matches!(v.as_str(), Some("S" | "M")),
        "name" => v.as_str().is_some_and(is_name),
        "icon" => v.as_str().is_some_and(|i| crate::widgets::ICONS.contains(&i)),
        _ => false,
    });
    (ok_id && ok_of && ok_items && ok_rest).then_some(id)
}

fn is_name(n: &str) -> bool {
    n.chars().count() <= 40 && !n.chars().any(char::is_control)
}

// A widget by its id, or a plugin she runs.
fn is_source_of(kind: &str, of: &str) -> bool {
    match kind {
        "widget" => crate::widgets::is_id(of),
        "plugin" => crate::scripts::SCRIPTS.iter().any(|s| s.id == of),
        _ => false,
    }
}

// One of several in one place: a widget's or a plugin's, as put there alone.
fn is_member(v: &Value) -> bool {
    v.as_object().is_some_and(|m| {
        let kind = m.get("type").and_then(Value::as_str).unwrap_or("");
        m.get("of").and_then(Value::as_str).is_some_and(|of| is_source_of(kind, of))
            && m.iter().all(|(key, v)| match key.as_str() {
                "type" | "of" => true,
                "name" => v.as_str().is_some_and(is_name),
                "icon" => v.as_str().is_some_and(|i| crate::widgets::ICONS.contains(&i)),
                _ => false,
            })
    })
}

// Whether a taskbar layout shows this widget: put there by itself, or its
// plugin's; alone or with others in one place.
pub fn is_on_taskbar(layout: &Value, widget: &str) -> bool {
    let owner = crate::scripts::owner_of(widget);
    let shows = |e: &Value| match (e["type"].as_str(), e["of"].as_str()) {
        (Some("widget"), Some(of)) => of == widget,
        (Some("plugin"), Some(of)) => Some(of) == owner,
        _ => false,
    };
    layout
        .as_object()
        .into_iter()
        .flat_map(|runs| runs.values())
        .filter_map(Value::as_array)
        .flatten()
        .any(|e| shows(e) || e["items"].as_array().is_some_and(|items| items.iter().any(shows)))
}

// The desktop's widgets (deskgrid.js isEntry, isLayout): up to 32, each
// { id, type, of, size, name, icon, d, at } alone (a widget's or a
// plugin's) or { id, type: "stack", items, size, d, at } several in one, 2
// to 8 as the taskbar's groups have them, or { id, type: "board", name,
// items, folded, d, at } a board (its name 16 characters at most, its tiles
// is_board_tiles); size s, m or l; d the display it
// is on (desk.rs key_of: a-z 0-9), at where it stood on each display it has
// been on, 8 at most, { key: [column, row] }. Each id once, each widget or
// plugin once.
fn is_desk_entry(v: &Value) -> bool {
    let Some(e) = v.as_object() else { return false };
    let Some(id) = e.get("id").and_then(Value::as_str) else { return false };
    let ok_id = id.len() >= 2 && id.len() <= 16 && id.starts_with('w') && id[1..].chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    let is_key = |k: &str| !k.is_empty() && k.len() <= 40 && k.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    let is_cell = |v: &Value| v.as_array().is_some_and(|a| a.len() == 2 && a.iter().all(|n| n.as_u64().is_some_and(|n| n <= 255)));
    if e.get("type").and_then(Value::as_str) == Some("board") {
        let ok_rest = e.iter().all(|(key, v)| match key.as_str() {
            "id" | "type" | "items" => true,
            "name" => v.as_str().is_some_and(|n| n.chars().count() <= 16 && !n.chars().any(char::is_control)),
            "folded" => v.is_boolean(),
            "d" => v.as_str().is_some_and(is_key),
            "at" => v.as_object().is_some_and(|at| at.len() <= 8 && at.iter().all(|(k, c)| is_key(k) && is_cell(c))),
            _ => false,
        });
        return ok_id && ok_rest && e.get("items").is_some_and(is_board_tiles);
    }
    let stack = e.get("type").and_then(Value::as_str) == Some("stack");
    let ok_kind = if stack {
        e.get("of").is_none()
            && e.get("items").and_then(Value::as_array).is_some_and(|items| (2..=8).contains(&items.len()) && items.iter().all(is_member) && items.iter().enumerate().all(|(i, m)| !items[..i].iter().any(|x| x["type"] == m["type"] && x["of"] == m["of"])))
    } else {
        e.get("items").is_none() && is_member(&json!({ "type": e.get("type"), "of": e.get("of") }))
    };
    let ok_rest = e.iter().all(|(key, v)| match key.as_str() {
        "id" | "type" | "of" | "items" => true,
        "size" => matches!(v.as_str(), Some("s" | "m" | "l")),
        "name" => !stack && v.as_str().is_some_and(is_name),
        "icon" => !stack && v.as_str().is_some_and(|i| crate::widgets::ICONS.contains(&i)),
        "d" => v.as_str().is_some_and(is_key),
        "at" => v.as_object().is_some_and(|at| at.len() <= 8 && at.iter().all(|(k, c)| is_key(k) && is_cell(c))),
        _ => false,
    });
    ok_id && e.contains_key("size") && ok_kind && ok_rest
}

// A board's tiles (deskgrid.js isBoardEntry): 16 at most, each a member
// with its size and its cell on the board's grid of 8 columns and 4 rows,
// whole on it, none over another, each widget or plugin once.
fn is_board_tiles(v: &Value) -> bool {
    let Some(tiles) = v.as_array() else { return false };
    let span = |s: &str| match s {
        "s" => Some((2, 2)),
        "m" => Some((4, 2)),
        "l" => Some((4, 4)),
        _ => None,
    };
    let mut placed: Vec<(u64, u64, u64, u64)> = Vec::new();
    let mut members: Vec<(&Value, &Value)> = Vec::new();
    for t in tiles {
        let Some(o) = t.as_object() else { return false };
        let Some((w, h)) = o.get("size").and_then(Value::as_str).and_then(span) else { return false };
        let Some([c, r]) = o.get("at").and_then(Value::as_array).and_then(|a| <[Value; 2]>::try_from(a.clone()).ok()) else { return false };
        let (Some(c), Some(r)) = (c.as_u64(), r.as_u64()) else { return false };
        let member: serde_json::Map<String, Value> = o.iter().filter(|(k, _)| !matches!(k.as_str(), "size" | "at")).map(|(k, v)| (k.clone(), v.clone())).collect();
        if c + w > 8 || r + h > 4 || !is_member(&Value::Object(member)) {
            return false;
        }
        if placed.iter().any(|&(pc, pr, pw, ph)| c < pc + pw && pc < c + w && r < pr + ph && pr < r + h) {
            return false;
        }
        let key = (&o["type"], &o["of"]);
        if members.contains(&key) {
            return false;
        }
        placed.push((c, r, w, h));
        members.push(key);
    }
    tiles.len() <= 16
}

pub fn is_desk_layout(v: &Value) -> bool {
    let Some(entries) = v.as_array() else { return false };
    if entries.len() > 32 || !entries.iter().all(is_desk_entry) {
        return false;
    }
    let ids: Vec<&Value> = entries.iter().map(|e| &e["id"]).collect();
    let members: Vec<(Value, Value)> = entries
        .iter()
        .flat_map(|e| match e["items"].as_array() {
            Some(items) => items.iter().map(|m| (m["type"].clone(), m["of"].clone())).collect::<Vec<_>>(),
            None => vec![(e["type"].clone(), e["of"].clone())],
        })
        .collect();
    ids.iter().enumerate().all(|(i, id)| !ids[..i].contains(id)) && members.iter().enumerate().all(|(i, m)| !members[..i].contains(m))
}

// Whether the desktop shows this widget: put there by itself, or its
// plugin's; alone or in a stack.
pub fn is_on_desk(layout: &Value, widget: &str) -> bool {
    let owner = crate::scripts::owner_of(widget);
    let shows = |e: &Value| match (e["type"].as_str(), e["of"].as_str()) {
        (Some("widget"), Some(of)) => of == widget,
        (Some("plugin"), Some(of)) => Some(of) == owner,
        _ => false,
    };
    layout.as_array().into_iter().flatten().any(|e| shows(e) || e["items"].as_array().is_some_and(|items| items.iter().any(shows)))
}

// Put elsewhere than the island: on the desktop, or on the taskbar while
// that is her home. A widget is in one place (the widget design's §7): it
// neither takes turns in the closed island nor stays pinned at its end
// there (island.js placedOut); the open island lists it all the same.
pub fn is_placed_out(desk: &Value, taskbar: &Value, taskbar_home: bool, widget: &str) -> bool {
    (cfg!(windows) && is_on_desk(desk, widget)) || (taskbar_home && is_on_taskbar(taskbar, widget))
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
                    (None, Some(id)) if !seen.contains(&id) && widgets < 32 => {
                        seen.push(id);
                        widgets += 1;
                        true
                    }
                    _ => false,
                })
            })
    })
}

// How long done, review and error stay (main.rs hold): until seen, or whole
// seconds from 3 to 600; the settings offer 8, 30 and 120, and Custom for
// any other (#59; flow.js holdSecs keeps what is typed within).
const HOLD_SECS: std::ops::RangeInclusive<u64> = 3..=600;

fn is_hold(v: &Value) -> bool {
    v.as_str() == Some("seen") || v.as_u64().is_some_and(|s| HOLD_SECS.contains(&s))
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
        // The taskbar home stands in for Windows' own: elsewhere, only the island.
        "display" => v.as_str() == Some("island") || (cfg!(windows) && v.as_str() == Some("taskbar")),
        "islandWidth" => matches!(v.as_str(), Some("narrow" | "normal" | "wide")),
        // At the top of the screen, up out of the way of a window under it (island.rs look_under).
        "islandHideUnder" => is_bool,
        "taskbarMaterial" => matches!(v.as_str(), Some("mica" | "black" | "clear")),
        "taskbarClearWhen" => matches!(v.as_str(), Some("mica" | "solid" | "always")),
        "taskbarButtons" => matches!(v.as_str(), Some("icons" | "labels")),
        "taskbarAlign" => matches!(v.as_str(), Some("center" | "left")),
        "taskbarEdge" => matches!(v.as_str(), Some("bottom" | "top" | "left" | "right")),
        "taskbarDisplays" => matches!(v.as_str(), Some("main" | "all")),
        // The taskbar's parts in three runs (island.js tbLayout), each once.
        "taskbarLayout" => v.is_null() || is_taskbar_layout(v),
        // The desktop's widgets (desk.rs, deskgrid.js).
        "deskLayout" => is_desk_layout(v),
        // The programs' buttons in the order dragged: their keys (paths).
        "taskbarOrder" => v.as_array().is_some_and(|keys| keys.len() <= 200 && keys.iter().all(|k| k.as_str().is_some_and(|s| s.len() <= 1024))),
        "hold" => is_hold(v),
        // When she has something to tell that needs no answer (a turn's end,
        // a hello, a plugin's nudge): the island open by itself, a ring round
        // it (the flow light), or only the dot (island.js cue).
        "endCue" => matches!(v.as_str(), Some("open" | "ring" | "none")),
        // Minutes a session at rest stays in sight, or 0: always (state.rs is_shown).
        "idleHideMin" => matches!(v.as_u64(), Some(0 | 15 | 30 | 60 | 120)),
        "promptWaitSec" => matches!(v.as_u64(), Some(30 | 60 | 120 | 290)),
        "notify" => ["waiting", "done", "error"].iter().all(|k| v.get(k).is_some_and(Value::is_boolean)),
        "widgetsOff" | "widgetOrder" => v.as_array().is_some_and(|ids| ids.len() <= 64 && ids.iter().all(|id| id.as_str().is_some_and(crate::widgets::is_id))),
        "widgetSpin" => matches!(v.as_u64(), Some(0 | 5 | 8 | 15)),
        // Each widget's row in the open island, small or medium, by its id
        // (island.js rowSize).
        "widgetSize" => v.as_object().is_some_and(|m| m.len() <= 64 && m.iter().all(|(id, s)| crate::widgets::is_id(id) && matches!(s.as_str(), Some("S" | "M")))),
        "plugins" => scripts::is_ok(v),
        "monitor" => v.as_object().is_some_and(|m| {
            m.iter().all(|(k, v)| match k.as_str() {
                "cpu" | "mem" | "net" | "battery" | "pin" => v.is_boolean(),
                "every" => v.as_u64().is_some_and(|s| (1..=10).contains(&s)),
                // The hardware's sensors: the built-in ones picked, and
                // LibreHardwareMonitor's.
                _ => crate::hw::is_setting(k, v).or_else(|| crate::lhm::is_setting(k, v)).unwrap_or(false),
            })
        }),
        // Where the monitor's sensors come from (hw.rs Source).
        "monitorSource" => crate::hw::is_source(v),
        "widgetNudge" => is_bool,
        // Not to be reminded that nothing is connected (island.js isHinting).
        "connectHintOff" => is_bool,
        // Her island's look on the taskbar, by Windows' mode (island.js
        // drawIslandLook): none, or a mode's none, is Windows' own grey.
        // And at the top of the screen (island.js lookNow), kept apart:
        // none is black, as it always was.
        "islandLook" | "islandTopLook" => v.is_null() || v.as_object().is_some_and(|m| m.iter().all(|(mode, look)| matches!(mode.as_str(), "dark" | "light") && is_island_look(look))),
        "mailAgent" => matches!(v.as_str(), Some("claude" | "codex" | "agy")),
        // The Mail page in two panes (list, letter), or one.
        "mailPanes" => matches!(v.as_str(), Some("one" | "two")),
        // How much mail is kept whole offline (mail/store.rs Offline).
        "mailOffline" => matches!(v.as_str(), Some("90d" | "year" | "all" | "opened")),
        "mailAgentConf" => mail::agent::is_conf_ok(v),
        // How mail reaches its servers (mail/proxy.rs).
        "mailProxy" => mail::proxy::Via::of(v).is_some(),
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
            // LibreHardwareMonitor's password, typed: to the Credential Manager.
            "monitor" => {
                rest.insert(key.clone(), crate::lhm::keep_password(value.clone()));
            }
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
    if rest.contains_key("dnd") && rest["dnd"] == true && sh.asks.lock().unwrap().dismiss_all() > 0 {
        sh.push_asks();
    }
    let plugins = rest.contains_key("plugins");
    let offline = rest.contains_key("mailOffline");
    let mail_way = rest.contains_key("mailProxy");
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
    // Mail kept offline for more or less: each account's sync at it now,
    // once the setting is there to be read.
    if offline {
        mail::kick_all(sh);
    }
    // Mail through another way: the accounts that follow the page's, connected again.
    if mail_way && mail::sync(sh) {
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
    if let Err(err) = connection::set_open_at_login(on, sh.home() == "taskbar") {
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
        Err(error) => json!({ "ok": false, "kind": connection::error_kind(&error), "error": error, "snapshot": snapshot(&sh) }),
    }
}

// install, remove: Codex's hooks.json.
#[tauri::command]
pub async fn settings_codex_hooks(app: AppHandle, action: String) -> Value {
    let sh = shared(&app);
    match connection::write_codex_hooks(&action, &i18n::t(sh.lang(), "codex.waiting")) {
        Ok(()) => json!({ "ok": true, "snapshot": snapshot(&sh) }),
        Err(error) => json!({ "ok": false, "kind": connection::error_kind(&error), "error": error, "snapshot": snapshot(&sh) }),
    }
}

// install, remove: Antigravity's hooks.json.
#[tauri::command]
pub async fn settings_agy_hooks(app: AppHandle, action: String) -> Value {
    let sh = shared(&app);
    match connection::write_agy_hooks(&action) {
        Ok(()) => json!({ "ok": true, "snapshot": snapshot(&sh) }),
        // The folder she is in, for one whose path agy can't run (kind "path").
        Err(error) => {
            let folder = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|p| p.to_string_lossy().into_owned())).unwrap_or_default();
            json!({ "ok": false, "kind": connection::error_kind(&error), "error": error, "folder": folder, "snapshot": snapshot(&sh) })
        }
    }
}

// install, remove: our plugin in OpenCode's plugin folder.
#[tauri::command]
pub async fn settings_opencode(app: AppHandle, action: String) -> Value {
    let sh = shared(&app);
    match connection::write_opencode_plugin(&action, sh.port) {
        Ok(()) => json!({ "ok": true, "snapshot": snapshot(&sh) }),
        Err(error) => json!({ "ok": false, "kind": connection::error_kind(&error), "error": error, "snapshot": snapshot(&sh) }),
    }
}

// install, remove: ZCode's config.json.
#[tauri::command]
pub async fn settings_zcode_hooks(app: AppHandle, action: String) -> Value {
    let sh = shared(&app);
    match connection::write_zcode_hooks(&action) {
        Ok(()) => json!({ "ok": true, "snapshot": snapshot(&sh) }),
        Err(error) => json!({ "ok": false, "kind": connection::error_kind(&error), "error": error, "snapshot": snapshot(&sh) }),
    }
}

// install, remove: omp's extension, in each of its agent folders.
#[tauri::command]
pub async fn settings_omp_hooks(app: AppHandle, action: String) -> Value {
    let sh = shared(&app);
    match connection::write_omp_hooks(&action, sh.port) {
        Ok(()) => json!({ "ok": true, "snapshot": snapshot(&sh) }),
        Err(error) => json!({ "ok": false, "kind": connection::error_kind(&error), "error": error, "snapshot": snapshot(&sh) }),
    }
}

// install, remove: DeepSeek Harness's hook bridge (its home-level patch file).
#[tauri::command]
pub async fn settings_dsh_hooks(app: AppHandle, action: String) -> Value {
    let sh = shared(&app);
    match connection::write_dsh_hooks(&action) {
        Ok(()) => json!({ "ok": true, "snapshot": snapshot(&sh) }),
        Err(error) => json!({ "ok": false, "kind": connection::error_kind(&error), "error": error, "snapshot": snapshot(&sh) }),
    }
}

// The CPU temperature/power helper (sensors.rs, #18 part B). She is not
// elevated, so each action launches her own exe elevated (one UAC) and
// returns at once; the switch's state follows as the helper starts (or does
// not) answering. "install": set up the task (PawnIO must be present);
// "installPawnIO": download and install PawnIO first; "remove": take the
// task away (one more UAC) and leave PawnIO installed.
#[tauri::command]
pub async fn settings_cpu_sensors(app: AppHandle, action: String) -> Value {
    let sh = shared(&app);
    let exe = data::exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    let sid = crate::logon::user_sid().unwrap_or_default();
    let data_dir = sh.dir.to_string_lossy().into_owned();
    let started = if action == "remove" {
        let params = format!("{} {} \"{}\"", crate::sensors::REMOVE_FLAG, sid, data_dir);
        crate::tasks::launch_as_admin_with(&exe, &params) > 32
    } else {
        let with_pawnio = if action == "installPawnIO" { " pawnio" } else { "" };
        let params = format!("{} {} {} \"{}\"{}", crate::sensors::SETUP_FLAG, sh.port, sid, data_dir, with_pawnio);
        let ok = crate::tasks::launch_as_admin_with(&exe, &params) > 32;
        if ok {
            crate::sensors::note_setting_up();
        }
        ok
    };
    // The person turned the UAC prompt down (ShellExecute gave at most 32).
    if started {
        json!({ "ok": true, "snapshot": snapshot(&sh) })
    } else {
        json!({ "ok": false, "kind": "cancelled", "snapshot": snapshot(&sh) })
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

// LibreHardwareMonitor for the monitor, from its settings (lhm_setup.rs):
// get (download, set up, start), start (one found: its web server turned
// on, started), startup and noStartup (its own task, at logon), usePort
// (hers set to the one its config says), folder (opened), look, dismiss.
// The snapshot as it is then (a job goes on on its own thread, pushed as it
// goes).
#[tauri::command]
pub async fn settings_lhm(app: AppHandle, what: String) -> Value {
    let sh = shared(&app);
    crate::lhm_setup::act(&sh, &what);
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
// where a mail provider makes app passwords (the Mail page's hint), or how
// Antigravity's command line agy is installed (the Mail page, with
// Antigravity here but not agy).
#[tauri::command]
pub async fn settings_open_site(app: AppHandle, place: String) {
    let url = match place.as_str() {
        "repo" => REPO,
        "agyCli" => "https://antigravity.google/docs/cli/install",
        "googleAppPasswords" => "https://myaccount.google.com/apppasswords",
        "appleAppPasswords" => "https://account.apple.com/account/manage",
        "yahooAppPasswords" => "https://login.yahoo.com/account/security",
        "lhm" => "https://github.com/LibreHardwareMonitor/LibreHardwareMonitor/releases",
        // Where to get an agent not on this computer (the Agent page, agents.js site).
        other => match other.strip_prefix("agent:").and_then(|id| AGENT_SITES.iter().find(|(a, _)| *a == id)) {
            Some((_, url)) => *url,
            None => fetch::SITE,
        },
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

    // How long endings stay: until seen, or whole seconds from 3 to 600 (#59).
    #[test]
    fn holds() {
        for ok in [json!("seen"), json!(3), json!(8), json!(30), json!(45), json!(120), json!(600)] {
            assert!(super::is_hold(&ok), "{ok}");
        }
        for bad in [json!(2), json!(601), json!(0), json!(-8), json!(7.5), json!(8.0), json!("x"), json!("8"), json!(null), json!(true)] {
            assert!(!super::is_hold(&bad), "{bad}");
        }
    }
}

#[cfg(test)]
mod layout_tests {
    use serde_json::json;

    #[test]
    fn taskbar_layouts() {
        assert!(super::is_taskbar_layout(&json!({ "left": ["her"], "center": ["start", "apps"], "right": ["monitor", "tray", "keys", "quick", "clock"] })));
        // The sessions' tags are no part any more (data.rs drops them as it reads).
        assert!(!super::is_taskbar_layout(&json!({ "center": ["start", "apps", "tags"] })));
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

    // Several in one place: a carousel or a stack of 2 to 8 widgets' and
    // plugins', each as one put there alone, none twice; no size of its own.
    #[test]
    fn several_widgets_in_one_place() {
        let ok = |w| super::is_taskbar_layout(&json!({ "right": [w, "clock"] }));
        let today = json!({ "type": "widget", "of": "today", "icon": "today" });
        let stock = json!({ "type": "plugin", "of": "stock", "name": "股票" });
        let mail = json!({ "type": "widget", "of": "inbox-a1", "name": "me@example.com", "icon": "mail" });
        assert!(ok(json!({ "id": "wc1", "type": "carousel", "items": [today, stock] })));
        assert!(ok(json!({ "id": "wc1", "type": "stack", "items": [today, stock, mail] })));
        // One alone, more than eight, the same one twice: not.
        assert!(!ok(json!({ "id": "wc1", "type": "stack", "items": [today] })));
        let nine: Vec<_> = (0..9).map(|i| json!({ "type": "widget", "of": format!("w{i}") })).collect();
        assert!(!ok(json!({ "id": "wc1", "type": "carousel", "items": nine })));
        assert!(ok(json!({ "id": "wc1", "type": "carousel", "items": nine[..8] })));
        assert!(!ok(json!({ "id": "wc1", "type": "stack", "items": [today, today] })));
        // A member as a widget alone is, nothing else: no spacer, no size, no id.
        assert!(!ok(json!({ "id": "wc1", "type": "stack", "items": [today, { "type": "sep" }] })));
        assert!(!ok(json!({ "id": "wc1", "type": "stack", "items": [today, { "type": "plugin", "of": "nope" }] })));
        assert!(!ok(json!({ "id": "wc1", "type": "stack", "items": [today, { "type": "widget", "of": "tokens", "size": "M" }] })));
        assert!(!ok(json!({ "id": "wc1", "type": "stack", "items": [today, { "id": "wd", "type": "widget", "of": "tokens" }] })));
        // The group: items and no of, no size; items only on a group.
        assert!(!ok(json!({ "id": "wc1", "type": "stack" })));
        assert!(!ok(json!({ "id": "wc1", "type": "stack", "of": "today", "items": [today, stock] })));
        assert!(!ok(json!({ "id": "wc1", "type": "carousel", "size": "M", "items": [today, stock] })));
        assert!(!ok(json!({ "id": "wc1", "type": "widget", "of": "tokens", "items": [today, stock] })));
        // What it holds is on the taskbar.
        let layout = json!({ "right": [{ "id": "wc1", "type": "stack", "items": [today, stock] }, "clock"] });
        assert!(super::is_on_taskbar(&layout, "today") && super::is_on_taskbar(&layout, "stock-nvda"));
        assert!(!super::is_on_taskbar(&layout, "tokens"));
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

    // The desktop's widgets, as deskgrid.js isLayout has them.
    #[test]
    fn desk_layouts() {
        let ok = |v: serde_json::Value| super::is_desk_layout(&v);
        assert!(ok(json!([])));
        assert!(ok(json!([
            { "id": "wa", "type": "widget", "of": "weather", "size": "m", "d": "d0a1b2c3d4e5f", "at": { "d0a1b2c3d4e5f": [3, 0], "d1": [0, 2] } },
            { "id": "wb", "type": "plugin", "of": "stock", "size": "l", "name": "股票", "icon": "chart" },
            { "id": "ws", "type": "stack", "size": "s", "items": [{ "type": "widget", "of": "monitor" }, { "type": "widget", "of": "countdown", "name": "倒计时" }] }
        ])));
        let bad = [
            json!({}),
            json!([{ "id": "wa", "type": "widget", "of": "weather", "size": "S" }]),
            json!([{ "id": "wa", "type": "widget", "of": "weather" }]),
            json!([{ "id": "Wa", "type": "widget", "of": "weather", "size": "s" }]),
            json!([{ "id": "wa", "type": "widget", "of": "weather", "size": "s", "d": "D1" }]),
            json!([{ "id": "wa", "type": "widget", "of": "weather", "size": "s", "at": { "d1": [1, 256] } }]),
            json!([{ "id": "wa", "type": "widget", "of": "weather", "size": "s", "at": { "d1": [1] } }]),
            json!([{ "id": "wa", "type": "widget", "of": "weather", "size": "s", "x": 1 }]),
            json!([{ "id": "wa", "type": "plugin", "of": "nope", "size": "s" }]),
            json!([{ "id": "wa", "type": "carousel", "size": "s", "items": [{ "type": "widget", "of": "a" }, { "type": "widget", "of": "b" }] }]),
            json!([{ "id": "ws", "type": "stack", "size": "s", "items": [{ "type": "widget", "of": "a" }] }]),
            json!([{ "id": "ws", "type": "stack", "size": "s", "items": [{ "type": "widget", "of": "a" }, { "type": "widget", "of": "a" }] }]),
            json!([{ "id": "ws", "type": "stack", "size": "s", "name": "x", "items": [{ "type": "widget", "of": "a" }, { "type": "widget", "of": "b" }] }]),
            json!([{ "id": "wa", "type": "widget", "of": "a", "size": "s" }, { "id": "wa", "type": "widget", "of": "b", "size": "s" }]),
            json!([{ "id": "wa", "type": "widget", "of": "a", "size": "s" }, { "id": "ws", "type": "stack", "size": "s", "items": [{ "type": "widget", "of": "a" }, { "type": "widget", "of": "b" }] }]),
        ];
        for b in bad {
            assert!(!ok(b.clone()), "{b}");
        }
        assert!(!ok(serde_json::Value::Array((0..33).map(|i| json!({ "id": format!("w{i}"), "type": "widget", "of": format!("x{i}"), "size": "s" })).collect())));
        // Boards: their tiles on their grid of 8 by 4, none over another.
        assert!(ok(json!([{ "id": "wb", "type": "board", "name": "行情", "folded": true, "d": "d1", "at": { "d1": [1, 1] }, "items": [
            { "type": "widget", "of": "monitor", "size": "m", "at": [0, 0] },
            { "type": "plugin", "of": "stock", "size": "l", "at": [4, 0] },
            { "type": "widget", "of": "today", "size": "s", "at": [0, 2], "name": "今天" }
        ] }, { "id": "wa", "type": "widget", "of": "weather", "size": "s" }])));
        assert!(ok(json!([{ "id": "wb", "type": "board", "items": [] }])));
        let tiles = |items: serde_json::Value| json!([{ "id": "wb", "type": "board", "items": items }]);
        let bad_boards = [
            tiles(json!([{ "type": "widget", "of": "a", "size": "s", "at": [0, 0] }, { "type": "widget", "of": "b", "size": "s", "at": [1, 1] }])),
            tiles(json!([{ "type": "widget", "of": "a", "size": "m", "at": [5, 0] }])),
            tiles(json!([{ "type": "widget", "of": "a", "size": "l", "at": [0, 1] }])),
            tiles(json!([{ "type": "widget", "of": "a", "size": "s" }])),
            tiles(json!([{ "type": "widget", "of": "a", "size": "s", "at": [0, 0], "x": 1 }])),
            tiles(json!([{ "type": "widget", "of": "a", "size": "s", "at": [0, 0] }, { "type": "widget", "of": "a", "size": "s", "at": [2, 0] }])),
            tiles(json!((0..17).map(|i| json!({ "type": "widget", "of": format!("t{i}"), "size": "s", "at": [0, 0] })).collect::<Vec<_>>())),
            json!([{ "id": "wb", "type": "board", "items": [], "name": "一二三四五六七八九十一二三四五六七" }]),
            json!([{ "id": "wb", "type": "board", "items": [], "size": "s" }]),
            json!([{ "id": "wb", "type": "board", "items": [], "folded": 1 }]),
            json!([{ "id": "wb", "type": "board" }]),
            json!([{ "id": "wb", "type": "board", "items": [{ "type": "widget", "of": "a", "size": "s", "at": [0, 0] }] }, { "id": "wa", "type": "widget", "of": "a", "size": "s" }]),
        ];
        for b in bad_boards {
            assert!(!ok(b.clone()), "{b}");
        }
        // What it shows.
        let layout = json!([{ "id": "wa", "type": "widget", "of": "monitor", "size": "m" }, { "id": "ws", "type": "stack", "size": "s", "items": [{ "type": "plugin", "of": "stock" }, { "type": "widget", "of": "today" }] }]);
        assert!(super::is_on_desk(&layout, "monitor"));
        assert!(super::is_on_desk(&layout, "stock-aapl"));
        assert!(super::is_on_desk(&layout, "today"));
        assert!(!super::is_on_desk(&layout, "weather"));
        assert!(!super::is_on_desk(&json!(null), "monitor"));
        // In a board.
        assert!(super::is_on_desk(&json!([{ "id": "wb", "type": "board", "items": [{ "type": "widget", "of": "weather", "size": "s", "at": [0, 0] }] }]), "weather"));
    }

    // A widget in one place: on the desktop, or on the taskbar while that is
    // her home, it is out of the island (not pinned there either).
    #[test]
    fn a_widget_in_one_place() {
        let desk = json!([{ "id": "wa", "type": "widget", "of": "monitor", "size": "m" }]);
        let bar = json!({ "left": ["her"], "center": ["start", "apps"], "right": [{ "id": "w1", "type": "widget", "of": "today", "size": "S" }, "tray", "clock"] });
        assert_eq!(super::is_placed_out(&desk, &bar, false, "monitor"), cfg!(windows));
        assert!(super::is_placed_out(&json!([]), &bar, true, "today"));
        // The taskbar not her home: its layout is not there to show it.
        assert!(!super::is_placed_out(&json!([]), &bar, false, "today"));
        assert!(!super::is_placed_out(&desk, &bar, true, "weather"));
    }
}
