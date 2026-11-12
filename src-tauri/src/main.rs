// Wakuwaku: a transparent, frameless, always-on-top desktop pet that shows
// what Claude Code is doing; its hooks report to a local port. The pages are
// src/pet (her window and the island's), talking to this side through
// src/pet/bridge.ts.
//
// Where she is: her home (display) is the island at the top centre, or a
// taskbar in place of Windows' own; she is in it, or out on the desktop
// (out), and her home stays up and talks for her. The settings grow out of
// her home; with none up, one rises for them.
//
//   shared.rs      what every part shares (Shared): the settings, her moods, the windows'
//                  states, the prompts, the widgets, each behind its lock; the log; the quit
//   commands/      what the pages call: her window's and both pages' (pet.rs), the island's
//                  (island.rs), the rest (mod.rs); others' in their own modules
//   hooks.rs       her exe run as an agent's hook: the event handed to her port
//   loops.rs       her threads that look every so often: the pointer, the widgets, the
//                  monitor, full screen, the ticker
//   webview.rs     what WebView2 is started with, and where it keeps its files
//   pet.rs         her window: size, place, eyes, drags, being carried, walks
//   island.rs      her home's window (island, taskbar), and the settings
//   appbar.rs      the taskbar's strip, kept from other windows
//   taskbar/       the taskbar's side of Windows: its own put away (shell.rs),
//                  the tray taken over (systray.rs), the windows' buttons (tasks.rs),
//                  the volume (audio.rs); its parts on Windows in taskbar/win/
//   settings.rs    what the settings show and change
//   pointer.rs     click-through, for both windows; screen.rs the work areas
//   asks.rs        prompts answered on her or the island
//   connection/    each agent's hooks, plugin or extension, written and read (one file each)
//   logon.rs       start at login: the Run value and her logon task; the session's logon
//   hw.rs          the monitor's built-in hardware readings (Windows' own: PDH, D3DKMT, the drives)
//   lhm.rs         hardware sensors from LibreHardwareMonitor's web server (the monitor)
//   lhm_setup.rs   LibreHardwareMonitor downloaded, set up and started, on the person's click
//   fetch.rs       pets from codex-pets.net
//   fullscreen.rs, focus.rs, notify.rs   bits of Windows
//   jump.rs        to a session's window, when you click the session
//   tray.rs        the tray icon and the menu
//   server.rs      the port the hooks report to
//   state.rs       per-session moods; events.rs the agents' events into her
//                  words (events_agy.rs, events_dsh.rs: their log lines;
//                  events_tests.rs: what she makes of the others')
//   data.rs        her folder, settings and pets; the mark of a quit by choice
//   starts.rs      what starts her (the hooks, at login, by hand), whether it may, and who did
//   widgets.rs     plugins in the island: widgets from scripts, and the built-in ones
//   plugins.rs     the plugins she runs for you (weather, stocks, timers, CI, dev servers)
//   addons.rs      third-party plugins: installed from a zip, run in sandboxed frames of
//                  the island's page (src/pet/addons.ts), their views in the cards
//                  (src/pet/views.ts), served by wakuplugin://
//   zip.rs         zips read and unpacked (LHM's download, a plugin's package)
//   tokens.rs      today's tokens, from Claude Code's, Codex's, Antigravity's, omp's, DSH's and Reasonix's own records
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// The settings' snapshot (settings.rs) is one json! with every agent's part
// in it: past the default 128 it no longer expands.
#![recursion_limit = "256"]

mod addons;
mod appbar;
mod asks;
mod audio;
mod backplate;
mod bars;
mod clock;
mod commands;
mod config;
mod connection;
mod crash;
mod data;
mod desk;
mod detach;
mod events;
mod events_agy;
mod events_dsh;
#[cfg(test)]
mod events_tests;
mod dsh_bridge;
mod embed;
mod fetch;
mod focus;
#[cfg(windows)]
mod frameless;
mod fullscreen;
mod gpu;
mod hooks;
mod hw;
mod i18n;
mod icons;
mod inputs;
mod island;
mod jump;
mod lhm;
mod lhm_setup;
mod lock;
mod logon;
mod loops;
mod mail;
mod market;
mod notch;
mod notify;
mod packs;
mod paper;
mod pet;
mod pins;
mod plugins;
mod pointer;
mod power;
mod pri;
mod proxy;
#[cfg(test)]
mod replay;
mod screen;
mod scripts;
mod sensors;
mod server;
mod settings;
mod shared;
mod shell;
mod shortcuts;
mod starts;
mod state;
mod systray;
mod taskbar;
mod tasks;
mod theme;
mod tokens;
mod tray;
mod updates;
mod wallpaper;
mod webview;
mod widgets;
mod winkeys;
mod zip;

use std::path::PathBuf;

use serde_json::{json, Value};
use tauri::Manager;

// What nearly every part uses of shared.rs, by its short name. Shared is
// pub here, as it was when it lived here: what it holds (its pub fields'
// types) counts as used, as before.
pub use shared::{now_ms, spawn_named, Responder, Shared};
pub(crate) use shared::shared;
use shared::DEFAULT_PORT;

fn main() {
    // Her exe again, as the taskbar's guard (shell.rs): it waits for her to
    // end, and gives Windows' taskbar back if she could not; and sees
    // whether she stops answering or crashed (crash.rs; her data folder is
    // the file's).
    let args: Vec<String> = std::env::args().collect();
    if let Some((pid, file)) = shell::guard_args(&args) {
        let data = file.parent().map(PathBuf::from).unwrap_or_default();
        let after = data.clone();
        let mut hang = crash::Hang::default();
        // Her main thread or her taskbar's not answering ten seconds while
        // her taskbar holds the strip: Windows' own shown again meanwhile.
        let mut back = shell::HangBack::default();
        let watch = || {
            let hung = hang.look(&data, pid);
            back.look(file, pid, hung, |line| crash::guard_said(&data, line));
        };
        shell::guard(pid, file, watch, || drop(icons::hold(&data)), move |code, started| crash::after_exit(&after, pid, code, started));
        // Her desktop's icons as they were, should she have left them moved (icons.rs).
        icons::given_back(&data);
        return;
    }
    // One whole reading of the monitor's built-in sensors as JSON, then
    // gone (hw.rs): to check them on real hardware without starting her.
    // Printed (the exe has no console of its own: piped, or redirected),
    // and written to a file when one is named after it.
    if let Some(at) = args.iter().position(|a| a == hw::DUMP_FLAG) {
        let dump = serde_json::to_string_pretty(&widgets::hw_dump()).unwrap_or_default();
        println!("{dump}");
        if let Some(file) = args.get(at + 1) {
            let _ = std::fs::write(file, &dump);
        }
        return;
    }
    // The sensor helper and its elevated setup/removal (sensors.rs, #18 part
    // B): each ends without starting a pet. The helper takes her port and
    // data folder; the setup takes the port, the user's SID and the data
    // folder (and "pawnio" to install the driver first); the removal the SID
    // and the folder.
    if let Some(at) = args.iter().position(|a| a == sensors::SENSORS_FLAG) {
        let port = args.get(at + 1).and_then(|p| p.parse().ok()).unwrap_or(DEFAULT_PORT);
        let data = args.get(at + 2).map(PathBuf::from).unwrap_or_else(data::folder);
        return sensors::run_helper(port, &data);
    }
    if let Some(at) = args.iter().position(|a| a == sensors::SETUP_FLAG) {
        let port = args.get(at + 1).and_then(|p| p.parse().ok()).unwrap_or(DEFAULT_PORT);
        let sid = args.get(at + 2).cloned().unwrap_or_default();
        let data = args.get(at + 3).map(PathBuf::from).unwrap_or_else(data::folder);
        let with_pawnio = args.iter().any(|a| a == "pawnio");
        return sensors::run_setup(port, &sid, &data, with_pawnio);
    }
    if let Some(at) = args.iter().position(|a| a == sensors::REMOVE_FLAG) {
        let sid = args.get(at + 1).cloned().unwrap_or_default();
        let data = args.get(at + 2).map(PathBuf::from).unwrap_or_else(data::folder);
        return sensors::run_remove(&sid, &data);
    }
    // One step for LibreHardwareMonitor run as administrator (lhm_setup.rs
    // admin: put in Program Files, its config, its task, removed), then
    // gone with its exit code; nothing of her started.
    if let Some(at) = args.iter().position(|a| a == lhm_setup::ADMIN_FLAG) {
        std::process::exit(lhm_setup::admin(&args[at + 1..]));
    }
    let port: u16 = std::env::var("WAKUWAKU_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(DEFAULT_PORT);
    if let Some(at) = args.iter().position(|a| a == connection::ENSURE_FLAG) {
        return hooks::ensure_running(port, &args[at + 1..]);
    }
    if std::env::args().any(|a| a == connection::CODEX_FLAG) {
        return hooks::codex_hook(port);
    }
    if let Some(at) = args.iter().position(|a| a == connection::AGY_FLAG) {
        return hooks::agy_hook(port, args.get(at + 1).map_or("", String::as_str));
    }
    if let Some(at) = args.iter().position(|a| a == connection::ZCODE_FLAG) {
        return hooks::zcode_hook(port, args.get(at + 1).map_or("", String::as_str));
    }
    if let Some(at) = args.iter().position(|a| a == connection::DSH_FLAG) {
        return hooks::dsh_hook(port, args.get(at + 1).map_or("", String::as_str));
    }
    if let Some(at) = args.iter().position(|a| a == connection::REASONIX_FLAG) {
        return hooks::reasonix_hook(port, args.get(at + 1).map_or("", String::as_str));
    }

    // Started at login (logon.rs): by her logon task, or by the Run value.
    // The task's, with start at login turned off since (the Run value taken
    // away, or turned off in Task Manager), takes the task away and goes.
    let at_login = logon::started_by(&args);
    if at_login == Some(logon::BY_TASK) && logon::login_task_unwanted() {
        eprintln!("wakuwaku: started by her logon task, but start at login is off: the task is taken away");
        return;
    }

    // In the job of whoever started her (a hook of an older her, a terminal,
    // an app's shell, the Task Scheduler), which may end her as it closes:
    // started again out of it, once, and this copy goes (detach.rs). Not run
    // from the source tree (cargo run waits on this copy), nor started by an
    // update or a restart: the one before her saw to its own job as it
    // started, and hers is that one's.
    // What started her goes with it (starts.rs: the one that stays has
    // this copy above it, not that).
    let after_update = args.iter().any(|a| a == updates::AFTER_UPDATE);
    if !after_update && !args.iter().any(|a| a == detach::FLAG) && !data::from_source() && detach::should_leave() {
        let by = starts::by_of(&args).is_none().then(|| starts::starter(&args, || jump::parent_names(3)));
        let by = by.iter().flat_map(|by| [starts::BY_FLAG, by.as_str()]);
        let rest: Vec<&str> = args.iter().skip(1).map(String::as_str).chain(by).chain([detach::FLAG]).collect();
        if let Ok(exe) = data::exe() {
            if detach::spawn(&exe, &rest) {
                return;
            }
        }
    }

    // Started by an update: the one it replaces told she is up, and waited
    // for, so this one gets the port (and the taskbar), not just wakes that one.
    updates::wait_after_update(&args, port);

    let dir = data::folder();
    let _ = std::fs::create_dir_all(&dir);

    // One pet at a time: whoever holds the port is the pet. Started again
    // while she runs: she comes back into sight, and this copy leaves. The
    // second of the two starts at login (her logon task, then the Run value)
    // leaves without a word: she is just up, and she may be out on the
    // desktop where she was left, which coming back would undo.
    let listener = match tiny_http::Server::http(("127.0.0.1", port)) {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("wakuwaku: port {port} is taken ({err}); another pet is up");
            if at_login.is_none() {
                let _ = ureq::post(&format!("http://127.0.0.1:{port}/come-home")).send_empty();
            }
            return;
        }
    };

    // She is up: quit by choice before, she is wanted again now (by hand,
    // at login, an update), and the sessions' hooks may start her from here
    // on. Who started her, said once she is (starts.rs).
    let cleared = data::clear_quit(&dir);
    let started_by = starts::starter(&args, || jump::parent_names(3));

    // Started as the exe an update left, its new one gone since: her own
    // name back before anything names her exe (updates.rs).
    updates::take_name_back(&dir);

    // Her crashes caught from here on, and a guard to see her end while
    // her reports can go and are not off (the taskbar home starts one
    // anyway): crash.rs.
    crash::install(&dir);
    if crash::wants_guard(&dir) {
        shell::spawn_guard(&dir.join("taskbar.json"));
    }

    // Tauri's async commands on two threads: its own runtime starts one for
    // each logical processor, nearly all of them idle all day (31 of 32,
    // #14). Kept for as long as she runs.
    let runtime = Box::leak(Box::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().expect("a runtime")));
    tauri::async_runtime::set(runtime.handle().clone());

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::widget_act,
            commands::pet::pet_ready,
            commands::pet::pet_hover,
            commands::pet::pet_drag_start,
            commands::pet::pet_drag_end,
            commands::pet::pet_menu,
            commands::pet::pet_double_click,
            commands::pet::pet_panel,
            commands::pet::pet_answer,
            commands::pet::pet_dismiss,
            commands::pet::pet_keyboard,
            commands::pet::pet_shortcuts_pause,
            commands::pet::pet_walk,
            commands::pet::pet_walk_stop,
            commands::island::island_holding,
            commands::island::island_editing,
            commands::island::island_opened,
            commands::island::island_seat,
            commands::island::island_stow,
            commands::island::island_raise,
            commands::island::island_plate,
            commands::island::island_release,
            commands::island::island_drop,
            commands::debug_result,
            commands::pet::pet_log,
            commands::session_jump,
            settings::settings_get,
            settings::settings_set,
            settings::settings_open,
            settings::settings_closed,
            settings::settings_show_pet,
            settings::settings_login,
            settings::settings_win_look,
            settings::settings_wallpaper,
            settings::settings_agent,
            settings::settings_cpu_sensors,
            bars::bar_ready,
            bars::dock_glass,
            bars::dock_thumbs,
            desk::desk_ready,
            desk::desk_shapes,
            desk::desk_hold,
            desk::desk_keep,
            desk::desk_icons,
            desk::desk_where,
            desk::desk_ask,
            desk::desk_type,
            desk::desk_drag_preview,
            desk::desk_drop,
            desk::desk_drag_answer,
            desk::desk_picked,
            desk::desk_wheel,
            bars::bar_asks,
            bars::bar_pop,
            bars::bar_pop_over,
            bars::barpop_ready,
            bars::barpop_place,
            bars::barpop_thumbs,
            bars::barpop_hide,
            settings::settings_fetch,
            settings::settings_gallery,
            settings::settings_open_site,
            settings::settings_update,
            settings::settings_lhm,
            settings::settings_crash,
            settings::settings_net_test,
            mail::mail_discover,
            mail::mail_probe,
            mail::mail_save,
            mail::mail_remove,
            mail::mail_switch,
            mail::letters::mail_letters,
            mail::letters::mail_letter,
            mail::agent::mail_hand,
            mail::agent::mail_say,
            mail::letters::mail_flag,
            mail::send::mail_send,
            mail::mail_kick,
            mail::mail_draft,
            mail::mail_keep_draft,
            taskbar::taskbar_tray,
            taskbar::taskbar_open,
            taskbar::taskbar_volume,
            taskbar::taskbar_clock_tip,
            taskbar::taskbar_end_task,
            taskbar::taskbar_inputs,
            taskbar::taskbar_input,
            taskbar::taskbar_battery_font,
            taskbar::taskbar_task_view_icons,
            commands::island::taskbar_handoff,
            taskbar::taskbar_tray_rects,
            taskbar::taskbar_window,
            taskbar::taskbar_windows,
            taskbar::taskbar_held,
            taskbar::taskbar_keys_pause,
            taskbar::taskbar_window_menu,
            taskbar::taskbar_watch_presses,
            taskbar::taskbar_tray_pin,
            taskbar::taskbar_app,
            taskbar::taskbar_pin_files,
            taskbar::taskbar_thumbs,
            addons::addons_running,
            addons::addon_call,
            addons::addon_manage,
            addons::addon_view,
            market::market,
        ])
        // Third-party plugins' files, for their sandboxed frames in the
        // island's page and their views in the cards (addons.rs serve):
        // http://wakuplugin.localhost/<id>/… on Windows. Off the main
        // thread: a file read from the disk.
        .register_asynchronous_uri_scheme_protocol(addons::SCHEME, |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            tauri::async_runtime::spawn_blocking(move || responder.respond(addons::serve(&app, &request)));
        })
        .on_menu_event(|app, event| tray::on_menu(&shared(app), event.id().as_ref()))
        .setup(move |app| {
            // On macOS she is an accessory, as a menu-bar app is: no Dock icon,
            // and the menu bar is never hers, even while her settings are open.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let mut settings = data::load(&dir);
            // Her pet gone: the first one there is.
            let found = data::pets(&dir);
            let id = settings.get("pet").and_then(Value::as_str).unwrap_or("").to_string();
            if !found.is_empty() && !found.iter().any(|p| p.id == id) {
                settings.insert("pet".into(), json!(found[0].id));
            }
            for folder in data::pet_dirs(&dir) {
                // Codex's pets are only read, never made a folder for.
                if folder != data::codex_pets() {
                    let _ = std::fs::create_dir_all(&folder);
                }
                let _ = app.asset_protocol_scope().allow_directory(&folder, true);
            }
            // The wallpaper packs' previews, for the settings (packs.rs);
            // what an install of a pack cut short left, cleared away.
            let _ = app.asset_protocol_scope().allow_directory(packs::root(&dir, addons::Kind::Wallpaper).join(".previews"), true);
            packs::start(&dir);
            notify::register(&dir);

            let today = widgets::Today::load(&dir, &widgets::local_date());
            // The hub's token in her data folder; one she could not keep there
            // is hers alone this run, and /v1 asks for it of everybody.
            let hub_folder = dir.join("agent-hub");
            let (hub, hub_failed) = match agent_hub::Hub::open(&hub_folder, now_ms()) {
                Ok(hub) => (hub, None),
                Err(e) => (agent_hub::Hub::with_token(agent_hub::new_token(), hub_folder.join("token"), now_ms()), Some(e)),
            };
            let sh = Shared::new(app.handle().clone(), dir, port, settings, hub, today);
            app.manage(sh.clone());
            // Her requests to the web the way the General page says (proxy.rs).
            proxy::set_web(&sh.setting(config::NET_PROXY));
            sh.log(&format!("up on port {}, data in {}", sh.port, sh.dir.display()));
            // The third-party plugins here, before anything checks a setting naming one.
            addons::start(&sh);
            if let Some(e) = hub_failed {
                sh.log(&format!("agent-hub: its token not kept in her data folder ({e}): /v1 serves nobody this run"));
            }
            let cleared = cleared.as_ref().map(|note| format!("; the mark of her last quit taken away ({note})")).unwrap_or_default();
            sh.log(&format!("started by: {started_by}{cleared}"));
            sh.log(&format!("displays: {}", sh.screens().describe()));
            // How long after the logon, and start at login kept to where she is now.
            sh.keep_login(at_login);
            taskbar::recover(&sh);

            pet::create(&sh)?;
            // Windows' own taskbar's pinned programs, once: hers seeded from
            // them, those she kept before after them (pins.rs).
            if !sh.flag(config::TASKBAR_PINS_IMPORTED) {
                let first = sh.clone();
                std::thread::spawn(move || pins::take_from_windows(&first));
            }
            // An app full screen as she starts, known before her home first
            // shows: the taskbar takes its strip only once it can show (one
            // taken with its window hidden keeps no room).
            // Said: her home then waits unseen, and the start looks like
            // none. The lock screen is one, as a session begins locked
            // (Windows signing in by itself to finish an update): started at
            // login, she came up behind it and took the taskbar the moment
            // it was unlocked, 170 s on, and the log said nothing of it (the
            // user's PC, 2026-10-13).
            if fullscreen::AVAILABLE && fullscreen::check(&[], &sh.displays()) == Some(true) {
                sh.starts_full_screen();
                sh.log(&format!("an app full screen as she starts: {}; her home waits for it to go", fullscreen::front()));
            }
            island::apply_visibility(&sh);
            tray::create(&sh)?;
            server::serve(sh.clone(), listener);
            shortcuts::start(&sh);
            // The mail accounts that are on, watched from the start.
            mail::sync(&sh);
            // The desktop's widgets (desk.rs).
            desk::sync(&sh);
            // Her updates looked for (a downloaded exe only).
            updates::start(&sh);
            // The plugin market's: what was read of it before, and its looks
            // for updates (none with none of its plugins installed).
            market::start(&sh);
            // Crashes since: their reports made, sent or asked about.
            crash::start(&sh);

            // Her threads that look every so often (loops.rs): the mouse's,
            // for both windows (click-through, her eyes, drags).
            loops::pointer(&sh);
            // The widgets: today's from the start; the tokens' each minute
            // while they are on; a script's gone once it stops sending.
            sh.count_today(|_| {});
            loops::widgets(&sh);
            loops::monitor(&sh);

            // Her island inside Windows' taskbar, looked after every second
            // while it is set to be there (embed.rs).
            embed::start(&sh);
            // Another app full screen; then endings held, prompts out of
            // time, sessions gone quiet, the windows on screen.
            loops::fullscreen(&sh);
            loops::ticker(&sh);

            // No pet at all yet: the default one (Claude小姐), from the exe
            // (fetch.rs seed_default_pet); written nowhere (the disk full,
            // the folder not to be written), downloaded; that failing too,
            // the settings, to pick one later.
            if found.is_empty() {
                let first = sh.clone();
                std::thread::spawn(move || {
                    let pets = first.dir.join("pets");
                    let got = match fetch::seed_default_pet(&pets) {
                        Ok(got) => Ok(got),
                        Err(e) => {
                            first.log(&format!("default pet not written ({e}): downloading it"));
                            fetch::download_pet(fetch::DEFAULT_PET, &pets).map_err(|e| fetch::describe("en", &e))
                        }
                    };
                    match got {
                        Ok(got) => {
                            first.log(&format!("default pet in place: {}", got["id"]));
                            first.change(json!({ "pet": got["id"] }));
                        }
                        Err(e) => {
                            first.log(&format!("default pet: {e}"));
                            island::open_settings(&first, Some("pets"));
                        }
                    }
                });
            }
            // DeepSeek Harness's rows and files seen to as she starts
            // (connection/dsh.rs dsh_at_start: 0.3.2's broken bridge rows to her
            // plugin, the bridge's hook list for where she is now, her
            // plugin's files for this version); her own copy only.
            if sh.keeps_login() {
                let dsh = sh.clone();
                spawn_named("dsh at start", move || {
                    for line in connection::dsh_at_start(dsh.port, &dsh.setting(config::DSH_VIA)) {
                        dsh.log(&line);
                    }
                });
            }
            // Never set up: the settings, where Claude Code or Codex gets
            // connected, once (welcomeOpened). Every start until one was
            // connected was too much for one who keeps her as a pet only,
            // with no way to stop it (the user, 2026-10-13); nor once "not
            // now" was said (connectHintOff, the welcome's or the island's).
            if !connection::look_connected(port) && !sh.flag(config::ONBOARDED) && !sh.flag(config::WELCOME_OPENED) && !sh.flag(config::CONNECT_HINT_OFF) {
                sh.change(json!({ "welcomeOpened": true }));
                island::open_settings(&sh, Some("connect"));
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while running wakuwaku")
        // Quitting: the bar's strip back to the other windows, and the
        // mail stopped (the plugins she runs end with her: plugins.rs).
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                let sh = shared(app);
                sh.log("quitting: the bar given back, the mail stopped");
                // Her island out of Explorer's taskbar before it goes with her.
                embed::stop();
                island::release_bar(&sh);
                mail::stop_all(&sh);
                sh.log("quitting: done");
            }
        });
}
