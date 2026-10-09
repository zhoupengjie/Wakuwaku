// Wakuwaku: a transparent, frameless, always-on-top desktop pet that shows
// what Claude Code is doing; its hooks report to a local port. The pages are
// src/pet (her window and the island's), talking to this side through
// src/pet/bridge.js.
//
// Where she is: display 'pet' is the pet alone; display 'island' is the
// island, with her in it, or out on the desktop (out) while it stays. The
// settings grow out of the island; with no island up, one rises for them.
//
//   pet.rs         her window: size, place, eyes, drags, being carried, walks
//   island.rs      the island at the top of the screen, her home, and the settings
//   settings.rs    what the settings show and change
//   pointer.rs     click-through, for both windows; screen.rs the work areas
//   asks.rs        prompts answered on her or the island
//   connection.rs  the plugin, hooks in settings.json, start at login
//   fetch.rs       pets from codex-pets.net
//   fullscreen.rs, focus.rs, notify.rs   bits of Windows
//   tray.rs        the tray icon and the menu
//   server.rs      the port the hooks report to
//   state.rs       per-session moods; events.rs hook events into messages
//   data.rs        her folder, settings and pets
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod asks;
mod connection;
mod data;
mod events;
mod fetch;
mod focus;
mod fullscreen;
mod i18n;
mod island;
mod notify;
mod pet;
mod pointer;
mod screen;
mod server;
mod settings;
mod state;
mod tray;

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{json, Map, Value};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

// The island's own animation of taking her in, before she counts as in.
const ABSORB_MS: u64 = 380;

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

// Answers a held hook request: Some(output), or None to drop it.
pub type Responder = Box<dyn FnOnce(Option<Value>) + Send>;

// What every part shares. No two of its locks are ever held at once.
pub struct Shared {
    pub app: AppHandle,
    pub dir: PathBuf,
    pub port: u16,
    pub settings: Mutex<Map<String, Value>>,
    pub pet: Mutex<state::Pet>,
    pub win: Mutex<pet::Win>,
    pub island: Mutex<island::Island>,
    pub asks: Mutex<asks::Asks<Responder>>,
    screens: Mutex<screen::Screens>,
    // Out of sight: hidden from the tray, or another app is full screen.
    pub hidden: AtomicBool,
    by_fullscreen: AtomicBool,
    greeted: AtomicBool,
    is_logging: bool,
    // Our windows' handles: never counted as another app full screen.
    own: Mutex<Vec<isize>>,
    // The window that had the keyboard before one of hers took it.
    prev_focus: Mutex<isize>,
    push_pending: AtomicBool,
    // Until when (ms) a loss of focus is our own doing: showing a window of
    // hers makes it the active one, and that is no click elsewhere.
    pub shuffle_until: std::sync::atomic::AtomicU64,
    // Debug only: answers to /debug/eval, by id.
    pub evals: Mutex<(u64, std::collections::HashMap<u64, std::sync::mpsc::Sender<Value>>)>,
}

impl Shared {
    pub fn setting(&self, key: &str) -> Value {
        self.settings.lock().unwrap().get(key).cloned().unwrap_or(Value::Null)
    }

    pub fn flag(&self, key: &str) -> bool {
        self.setting(key).as_bool().unwrap_or(false)
    }

    pub fn lang(&self) -> &'static str {
        i18n::lang(self.setting("lang").as_str().unwrap_or("auto"))
    }

    pub fn hold(&self) -> state::Hold {
        match self.setting("hold").as_u64() {
            Some(secs) => state::Hold::Secs(secs),
            None => state::Hold::Seen,
        }
    }

    pub fn screens(&self) -> screen::Screens {
        self.screens.lock().unwrap().clone()
    }

    pub fn is_visible(&self) -> bool {
        !self.hidden.load(Ordering::SeqCst)
            && !self.flag("dnd")
            && !(self.flag("hideInFullscreen") && self.by_fullscreen.load(Ordering::SeqCst))
    }

    pub fn is_island_mode(&self) -> bool {
        self.setting("display").as_str() == Some("island")
    }

    pub fn own_window(&self, _win: &WebviewWindow) {
        #[cfg(windows)]
        if let Ok(hwnd) = _win.hwnd() {
            self.own.lock().unwrap().push(hwnd.0 as isize);
        }
    }

    pub fn log(&self, line: &str) {
        if !self.is_logging {
            return;
        }
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(self.dir.join("debug.log")) {
            let _ = writeln!(f, "{} [tauri] {line}", now_ms());
        }
    }

    // Everything the pages draw from.
    pub fn payload(&self) -> Value {
        let (mut now, second) = {
            let pet = self.pet.lock().unwrap();
            (pet.get(now_ms()), pet.second())
        };
        let settings = self.settings.lock().unwrap().clone();
        let id = settings.get("pet").and_then(Value::as_str).unwrap_or("");
        let found = data::pets(&self.dir);
        let sprite = found.iter().find(|p| p.id == id);
        let (temp, settings_open) = {
            let isl = self.island.lock().unwrap();
            (isl.temp, isl.settings_open)
        };
        now["config"] = Value::Object(settings);
        now["lang"] = json!(self.lang());
        now["sprite"] = sprite.map_or(Value::Null, |p| json!(data::asset_url(&p.sheet)));
        now["spriteVersion"] = json!(sprite.map_or(2, |p| p.version));
        // The next session that wants something, for the island's second bubble.
        now["second"] = json!(second);
        // The island risen only for the settings, and whether they are open:
        // then the island holds any prompt.
        now["islandTemp"] = json!(temp);
        now["settingsOpen"] = json!(settings_open);
        now
    }

    fn to_pages(&self, event: &str, payload: Value) {
        for label in ["pet", "island"] {
            let _ = self.app.emit_to(label, event, payload.clone());
        }
    }

    // The mood or the settings changed: the pages, the tray, the settings.
    pub fn redraw(&self) {
        self.to_pages("pet:update", self.payload());
        tray::refresh(self);
        self.push_settings();
    }

    // The prompts waiting changed.
    pub fn push_asks(&self) {
        let views = self.asks.lock().unwrap().views();
        self.to_pages("pet:asks", views);
        self.push_settings();
    }

    // The open settings follow what happens, a few times a second at most.
    pub fn push_settings(&self) {
        if !self.island.lock().unwrap().settings_open || self.push_pending.swap(true, Ordering::SeqCst) {
            return;
        }
        let sh = shared(&self.app);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(250));
            sh.push_pending.store(false, Ordering::SeqCst);
            let _ = sh.app.emit_to("island", "settings:changed", settings::snapshot(&sh));
        });
    }

    // A message from the hooks (or a reaction of our own).
    pub fn apply(&self, msg: &Value) -> bool {
        let out = self.pet.lock().unwrap().apply(msg, now_ms(), self.hold());
        let Some(out) = out else { return false };
        if out.changed {
            self.redraw();
        }
        if let Some(react) = out.react {
            self.to_pages("pet:react", react);
        }
        if !self.flag("dnd") {
            for (mood, project) in out.alerts {
                self.alert(&mood, &project);
            }
        }
        true
    }

    // A chime from the window that is up, and a system notification if asked for.
    fn alert(&self, mood: &str, project: &str) {
        let front = if self.is_island_mode() { "island" } else { "pet" };
        let _ = self.app.emit_to(front, "pet:alert", json!({ "mood": mood }));
        let group = match mood {
            "waiting" => "waiting",
            "error" => "error",
            _ => "done",
        };
        if self.setting("notify").get(group).and_then(Value::as_bool) != Some(true) {
            return;
        }
        let lang = self.lang();
        let whose = if project.is_empty() { String::new() } else { i18n::t(lang, "notify.project").replace("{project}", project) };
        let body = i18n::t(lang, &format!("notify.{mood}")).replace("{project}", &whose);
        let sh = shared(&self.app);
        // Off this thread: a toast can take a moment.
        std::thread::spawn(move || {
            if let Err(err) = notify::show(&body) {
                sh.log(&format!("notification failed: {err}"));
            }
        });
    }

    // A hello when she first appears: once, from whichever window is the one up.
    pub fn greet(&self) {
        if self.greeted.swap(true, Ordering::SeqCst) {
            return;
        }
        let is_stale = connection::hooks_status(self.port) == "stale";
        self.apply(&json!({ "react": "wave", "say": { "key": if is_stale { "say.hooksStale" } else { "say.arrived" } } }));
    }

    pub fn change(self: &Arc<Self>, patch: Value) {
        let Value::Object(patch) = patch else { return };
        let touches_visibility = ["dnd", "display", "out", "hideInFullscreen"].iter().any(|k| patch.contains_key(*k));
        {
            let mut settings = self.settings.lock().unwrap();
            settings.extend(patch);
            data::save(&self.dir, &settings);
        }
        self.redraw();
        tray::refresh_menu(self);
        if touches_visibility {
            self.apply_visibility();
        }
    }

    pub fn apply_visibility(self: &Arc<Self>) {
        pet::apply_visibility(self);
        island::apply_visibility(self);
        // Out of sight, nobody can answer here: the terminal has them.
        if !self.is_visible() && self.asks.lock().unwrap().dismiss_all() {
            self.push_asks();
        }
        tray::refresh(self);
    }

    pub fn set_hidden(self: &Arc<Self>, is_hidden: bool) {
        self.hidden.store(is_hidden, Ordering::SeqCst);
        self.apply_visibility();
        tray::refresh_menu(self);
        self.push_settings();
    }

    // Show a window of hers. Once created, a Tauri window shows activated
    // (SW_SHOW): the active one of this app, and the foreground one if this
    // app had it (the settings open). Neither is the person clicking
    // elsewhere, so the island ignores the loss of focus for a moment; and
    // open settings get the keyboard straight back.
    pub fn show_window(&self, win: &WebviewWindow) {
        self.shuffle_until.store(now_ms() + 500, Ordering::SeqCst);
        let _ = win.show();
        if self.island.lock().unwrap().settings_open && win.label() != "island" {
            if let Some(island) = self.app.get_webview_window("island") {
                let _ = island.set_focus();
            }
        }
    }

    pub fn is_shuffling(&self) -> bool {
        now_ms() < self.shuffle_until.load(Ordering::SeqCst)
    }

    // A window of hers takes the keyboard (the settings, a box to type in),
    // or lets go of it: then the window that had it before gets it back, if
    // hers still has it (a click elsewhere already moved it on).
    pub fn set_keyboard(&self, window: &WebviewWindow, on: bool) {
        #[cfg(windows)]
        let mine = window.hwnd().map(|h| h.0 as isize).unwrap_or(0);
        #[cfg(not(windows))]
        let mine = 0isize;
        if on {
            let before = focus::foreground();
            if before != mine && !self.own.lock().unwrap().contains(&before) {
                *self.prev_focus.lock().unwrap() = before;
            }
            let _ = window.set_focusable(true);
            let _ = window.set_focus();
        } else {
            let _ = window.set_focusable(false);
            let prev = *self.prev_focus.lock().unwrap();
            if focus::foreground() == mine {
                focus::set_foreground(prev);
            }
        }
        self.log(&format!("{}: keyboard {}", window.label(), if on { "taken" } else { "let go" }));
    }
}

// --- Where she is -----------------------------------------------------------------

// The pet alone, or the island (she goes into it).
pub fn set_display(sh: &Arc<Shared>, display: &str) {
    if sh.setting("display").as_str() == Some(display) && !sh.flag("out") {
        return;
    }
    if display == "pet" {
        pet::return_to_spot(sh);
    }
    sh.change(json!({ "display": display, "out": false }));
}

// Out on the desktop where she was last left, or back in the island, from the menu.
pub fn set_out(sh: &Arc<Shared>, out: bool) {
    if !sh.is_island_mode() || sh.flag("out") == out {
        return;
    }
    if out {
        pet::return_to_spot(sh);
    }
    sh.change(json!({ "out": out }));
}

// Brought back close to the island: it takes her in from where she is.
pub fn absorb(sh: &Arc<Shared>, point: (i32, i32)) {
    sh.log(&format!("absorbed from {point:?}"));
    pet::hide_now(sh);
    island::absorb(sh, point);
    let sh = sh.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(ABSORB_MS));
        sh.change(json!({ "out": false }));
    });
}

// Started again while running, or "back to the corner": find her.
pub fn come_home(sh: &Arc<Shared>) {
    sh.hidden.store(false, Ordering::SeqCst);
    if !sh.is_island_mode() || sh.flag("out") {
        pet::come_home(sh);
    }
    sh.apply_visibility();
    tray::refresh_menu(sh);
}

pub(crate) fn shared(app: &AppHandle) -> Arc<Shared> {
    app.state::<Arc<Shared>>().inner().clone()
}

// --- From the pages -----------------------------------------------------------------
// Async, so none of them runs on the main thread while it waits on a lock.
// `window` says which page: "pet" or "island".

fn is_island(window: &WebviewWindow) -> bool {
    window.label() == "island"
}

#[tauri::command]
async fn pet_ready(app: AppHandle, window: WebviewWindow) {
    let sh = shared(&app);
    if is_island(&window) { island::ready(&sh) } else { pet::ready(&sh) }
    let views = sh.asks.lock().unwrap().views();
    let _ = app.emit_to(window.label(), "pet:asks", views);
}

#[tauri::command]
async fn pet_hover(app: AppHandle, window: WebviewWindow, is_over: bool) {
    let sh = shared(&app);
    if is_island(&window) { island::hover(&sh, is_over) } else { pet::hover(&sh, is_over) }
}

#[tauri::command]
async fn pet_drag_start(app: AppHandle, window: WebviewWindow) {
    if !is_island(&window) {
        pet::drag_start(&shared(&app));
    }
}

// The button went up over her: that ends a carry as well (the island may
// have lost the button on the way).
#[tauri::command]
async fn pet_drag_end(app: AppHandle, window: WebviewWindow) {
    if !is_island(&window) {
        pet::let_go(&shared(&app));
    }
}

#[tauri::command]
async fn pet_menu(app: AppHandle, window: WebviewWindow) {
    let sh = shared(&app);
    if let Ok(menu) = tray::menu(&sh, false) {
        // A Windows menu closes at once unless its window can come to the
        // front, which these cannot (they never take focus): for the menu, it may.
        let was_open = sh.island.lock().unwrap().settings_open && is_island(&window);
        let _ = window.set_focusable(true);
        let _ = window.popup_menu(&menu);
        if !was_open {
            let _ = window.set_focusable(false);
        }
    }
}

// On her: out of the island, she flies back into it; as the pet on her own,
// the settings. (The island has no double-click: one click opens the settings.)
#[tauri::command]
async fn pet_double_click(app: AppHandle, window: WebviewWindow) {
    let sh = shared(&app);
    if !is_island(&window) {
        pet::double_click(&sh);
    }
}

// The room a shape of the island needs; above her, the prompt panel's size.
#[tauri::command]
async fn pet_panel(app: AppHandle, window: WebviewWindow, measured: Value) {
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
async fn pet_answer(app: AppHandle, id: u64, choice: Value) -> bool {
    let sh = shared(&app);
    let lang = sh.lang();
    let ok = sh.asks.lock().unwrap().answer(id, &choice, lang);
    sh.log(&format!("answer {id}: {choice} -> {ok}"));
    if ok {
        sh.push_asks();
    }
    ok
}

#[tauri::command]
async fn pet_dismiss(app: AppHandle, id: u64) {
    let sh = shared(&app);
    if sh.asks.lock().unwrap().dismiss(id) {
        sh.push_asks();
    }
}

// Typing needs the keyboard, which these windows do not take otherwise.
#[tauri::command]
async fn pet_keyboard(app: AppHandle, window: WebviewWindow, needs: bool) {
    let sh = shared(&app);
    // The open settings keep it whatever a prompt's box does.
    if !needs && is_island(&window) && sh.island.lock().unwrap().settings_open {
        return;
    }
    sh.set_keyboard(&window, needs);
}

#[tauri::command]
async fn pet_walk(app: AppHandle, window: WebviewWindow, dx: f64, ms: f64) -> bool {
    if is_island(&window) {
        return false;
    }
    let sh = shared(&app);
    tauri::async_runtime::spawn_blocking(move || pet::walk(&sh, dx, ms)).await.unwrap_or(false)
}

#[tauri::command]
async fn pet_walk_stop(app: AppHandle) {
    pet::stop_walking(&shared(&app));
}

#[tauri::command]
async fn island_holding(app: AppHandle, is_holding: bool) {
    island::set_holding(&shared(&app), is_holding);
}

// The drop pinched off: she is out, under the cursor, her middle this far
// from it. The button is still held on the island; island_drop says when it
// is let go.
#[tauri::command]
async fn island_release(app: AppHandle, offset: Value) {
    let sh = shared(&app);
    sh.log(&format!("island: release {offset}"));
    if sh.is_island_mode() && !sh.flag("out") {
        pet::carry(&sh, &offset);
    }
}

#[tauri::command]
async fn island_drop(app: AppHandle) {
    let sh = shared(&app);
    sh.log("island: drop");
    pet::let_go(&sh);
}

// A page's answer to /debug/eval.
#[tauri::command]
async fn debug_result(app: AppHandle, id: u64, value: Value) {
    if let Some(tx) = shared(&app).evals.lock().unwrap().1.remove(&id) {
        let _ = tx.send(value);
    }
}

#[tauri::command]
async fn pet_log(app: AppHandle, window: WebviewWindow, line: String) {
    shared(&app).log(&format!("{} page: {line}", window.label()));
}

// --- Started by the SessionStart hook ----------------------------------------------------

// If no pet answers on the port, start one, detached so it outlives the hook
// and the session (she greets on her own); if one does, hand her the event.
// Then leave, quick and quiet, whatever happens.
fn ensure_running(port: u16) {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut text = String::new();
        let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut text);
        let _ = tx.send(text);
    });
    let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_millis(800))).build().new_agent();
    let up = agent
        .get(&format!("http://127.0.0.1:{port}/health"))
        .call()
        .ok()
        .and_then(|mut res| res.body_mut().read_to_string().ok())
        .is_some_and(|body| body.contains("wakuwaku"));
    let event: Option<Value> = rx.recv_timeout(Duration::from_millis(1000)).ok().and_then(|text| serde_json::from_str(&text).ok());
    if up {
        if let Some(event) = event {
            let _ = agent.post(&format!("http://127.0.0.1:{port}/hook?from=wakuwaku")).send_json(&event);
        }
    } else if let Ok(exe) = std::env::current_exe() {
        let mut command = std::process::Command::new(exe);
        command.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
            command.creation_flags(0x0000_0008 | 0x0000_0200);
        }
        let _ = command.spawn();
    }
}

fn main() {
    let port: u16 = std::env::var("WAKUWAKU_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(47213);
    if std::env::args().any(|a| a == connection::ENSURE_FLAG || a == connection::LEGACY_ENSURE_FLAG) {
        return ensure_running(port);
    }

    let dir = data::folder();
    data::adopt(&dir);

    // One pet at a time: whoever holds the port is the pet. Started again
    // while she runs: she comes back into sight, and this copy leaves.
    let listener = match tiny_http::Server::http(("127.0.0.1", port)) {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("wakuwaku: port {port} is taken ({err}); another pet is up");
            let _ = ureq::post(&format!("http://127.0.0.1:{port}/come-home")).send_empty();
            return;
        }
    };

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            pet_ready,
            pet_hover,
            pet_drag_start,
            pet_drag_end,
            pet_menu,
            pet_double_click,
            pet_panel,
            pet_answer,
            pet_dismiss,
            pet_keyboard,
            pet_walk,
            pet_walk_stop,
            island_holding,
            island_release,
            island_drop,
            debug_result,
            pet_log,
            settings::settings_get,
            settings::settings_set,
            settings::settings_open,
            settings::settings_closed,
            settings::settings_show_pet,
            settings::settings_login,
            settings::settings_hooks,
            settings::settings_fetch,
            settings::settings_gallery,
            settings::settings_open_site,
        ])
        .on_menu_event(|app, event| tray::on_menu(&shared(app), event.id().as_ref()))
        .setup(move |app| {
            let mut settings = data::load(&dir);
            // Her pet gone: the first one there is.
            let found = data::pets(&dir);
            let id = settings.get("pet").and_then(Value::as_str).unwrap_or("").to_string();
            if !found.is_empty() && !found.iter().any(|p| p.id == id) {
                settings.insert("pet".into(), json!(found[0].id));
            }
            for folder in data::pet_dirs(&dir) {
                let _ = std::fs::create_dir_all(&folder);
                let _ = app.asset_protocol_scope().allow_directory(&folder, true);
            }
            notify::register(&dir);

            let sh = Arc::new(Shared {
                app: app.handle().clone(),
                is_logging: dir.join("debug.on").exists(),
                dir,
                port,
                settings: Mutex::new(settings),
                pet: Mutex::new(state::Pet::default()),
                win: Mutex::new(pet::Win::default()),
                island: Mutex::new(island::Island::default()),
                asks: Mutex::new(asks::Asks::default()),
                screens: Mutex::new(screen::read(app.handle())),
                hidden: AtomicBool::new(false),
                by_fullscreen: AtomicBool::new(false),
                greeted: AtomicBool::new(false),
                own: Mutex::new(Vec::new()),
                prev_focus: Mutex::new(0),
                push_pending: AtomicBool::new(false),
                shuffle_until: std::sync::atomic::AtomicU64::new(0),
                evals: Mutex::new((0, std::collections::HashMap::new())),
            });
            app.manage(sh.clone());
            sh.log(&format!("up on port {}, data in {}", sh.port, sh.dir.display()));

            pet::create(&sh)?;
            island::apply_visibility(&sh);
            tray::create(&sh)?;
            server::serve(sh.clone(), listener);

            // The mouse: click-through, her eyes, drags; for both windows.
            let pointer = sh.clone();
            std::thread::spawn(move || loop {
                let wait = match pointer.app.cursor_position() {
                    Ok(c) => {
                        let cursor = (c.x.round() as i32, c.y.round() as i32);
                        pet::poll(&pointer, cursor).min(island::poll(&pointer, cursor))
                    }
                    Err(_) => 200,
                };
                std::thread::sleep(Duration::from_millis(wait));
            });

            // Endings held for a while; prompts out of time; sessions gone
            // quiet; another app full screen; the windows on screen.
            let ticker = sh.clone();
            std::thread::spawn(move || {
                for n in 1u64.. {
                    std::thread::sleep(Duration::from_millis(500));
                    let now = now_ms();
                    let hold = ticker.hold();
                    if n % 2 == 0 {
                        let mut changed = ticker.pet.lock().unwrap().tick(now, hold);
                        if n % 120 == 0 {
                            changed |= ticker.pet.lock().unwrap().check_stale(now, hold);
                        }
                        if changed {
                            ticker.redraw();
                        }
                        if ticker.asks.lock().unwrap().expire(now) {
                            ticker.push_asks();
                        }
                    }
                    if n % 3 == 0 {
                        let watching = fullscreen::AVAILABLE && ticker.flag("hideInFullscreen");
                        let own = ticker.own.lock().unwrap().clone();
                        let is_full = if watching { fullscreen::check(&own) } else { Some(false) };
                        if let Some(is_full) = is_full {
                            if ticker.by_fullscreen.swap(is_full, Ordering::SeqCst) != is_full {
                                ticker.log(&format!("another app full screen: {is_full}"));
                                ticker.apply_visibility();
                            }
                        }
                    }
                    if n % 4 == 0 {
                        let screens = screen::read(&ticker.app);
                        *ticker.screens.lock().unwrap() = screens;
                        pet::keep_on_screen(&ticker);
                        island::keep_on_screen(&ticker);
                    }
                }
            });

            // No pet at all yet: get the default one (Claude小姐). Offline,
            // or the site down: the settings, to pick one later.
            if found.is_empty() {
                let first = sh.clone();
                std::thread::spawn(move || match fetch::download_pet(fetch::DEFAULT_PET, &first.dir.join("pets")) {
                    Ok(got) => {
                        first.log(&format!("default pet downloaded: {}", got["id"]));
                        first.change(json!({ "pet": got["id"] }));
                    }
                    Err(e) => {
                        first.log(&format!("default pet: {}", fetch::describe("en", &e)));
                        island::open_settings(&first, Some("pets"));
                    }
                });
            }
            // Never set up: the settings, where Claude Code gets connected.
            if !sh.flag("onboarded") && connection::connection(port) == "none" {
                island::open_settings(&sh, Some("connect"));
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running wakuwaku");
}
