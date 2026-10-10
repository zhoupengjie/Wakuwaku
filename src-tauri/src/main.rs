// Wakuwaku: a transparent, frameless, always-on-top desktop pet that shows
// what Claude Code is doing; its hooks report to a local port. The pages are
// src/pet (her window and the island's), talking to this side through
// src/pet/bridge.js.
//
// Where she is: her home (display) is the island at the top centre, or a
// taskbar in place of Windows' own; she is in it, or out on the desktop
// (out), and her home stays up and talks for her. The settings grow out of
// her home; with none up, one rises for them.
//
//   pet.rs         her window: size, place, eyes, drags, being carried, walks
//   island.rs      her home's window (island, taskbar), and the settings
//   appbar.rs      the taskbar's strip, kept from other windows
//   taskbar.rs     the taskbar's side of Windows: its own put away (shell.rs),
//                  the tray taken over (systray.rs), the windows' buttons (tasks.rs),
//                  the volume (audio.rs)
//   settings.rs    what the settings show and change
//   pointer.rs     click-through, for both windows; screen.rs the work areas
//   asks.rs        prompts answered on her or the island
//   connection.rs  the plugin, hooks in settings.json and Codex's hooks.json, start at login
//   fetch.rs       pets from codex-pets.net
//   fullscreen.rs, focus.rs, notify.rs   bits of Windows
//   jump.rs        to a session's window, when you click the session
//   tray.rs        the tray icon and the menu
//   server.rs      the port the hooks report to
//   state.rs       per-session moods; events.rs (Claude Code) and
//                  events_codex.rs (Codex) hook events into messages
//   data.rs        her folder, settings and pets
//   widgets.rs     plugins in the island: widgets from scripts, and the built-in ones
//   tokens.rs      today's tokens, from Claude Code's and Codex's own records
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod appbar;
mod asks;
mod audio;
mod connection;
mod data;
mod events;
mod events_codex;
mod fetch;
mod focus;
mod fullscreen;
mod i18n;
mod island;
mod jump;
mod mail;
mod notify;
mod pet;
mod pointer;
mod screen;
mod scripts;
mod server;
mod settings;
mod shell;
mod state;
mod systray;
mod taskbar;
mod tasks;
mod tokens;
mod tray;
mod wallpaper;
mod widgets;

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
    // Plugins in the island (widgets.rs), and what today has seen.
    pub widgets: Mutex<widgets::Widgets>,
    pub today: Mutex<widgets::Today>,
    // The plugins she runs for you (scripts.rs).
    pub scripts: Mutex<scripts::Runner>,
    // The mail accounts being watched (mail.rs).
    pub mail: Mutex<mail::Runner>,
    screens: Mutex<screen::Screens>,
    // Out of sight: hidden from the tray, or another app is full screen.
    pub hidden: AtomicBool,
    by_fullscreen: AtomicBool,
    greeted: AtomicBool,
    // When the last Codex event came (ms), 0 never: Codex runs its hooks
    // only once the person trusts them.
    pub codex_seen: std::sync::atomic::AtomicU64,
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

    // Another app is full screen (watched while the setting asks, or the taskbar is her home).
    pub fn is_fullscreen(&self) -> bool {
        self.by_fullscreen.load(Ordering::SeqCst)
    }

    // Her home: 'island' or 'taskbar' (island.rs).
    pub fn home(&self) -> &'static str {
        match self.setting("display").as_str() {
            Some("taskbar") => "taskbar",
            _ => "island",
        }
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
        let (mut now, second, list) = {
            let pet = self.pet.lock().unwrap();
            (pet.get(now_ms()), pet.second(), pet.list())
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
        // Every session, the one shown first: the open island lists the others.
        now["list"] = list;
        // The island risen only for the settings, and whether they are open:
        // then the island holds any prompt.
        now["islandTemp"] = json!(temp);
        now["settingsOpen"] = json!(settings_open);
        // The widgets that are on and have something to say, for the island
        // while nothing needs you.
        now["widgets"] = Value::Array(self.widgets_view().as_array().into_iter().flatten().filter(|w| w["on"] == true && !w["value"].is_null()).cloned().collect());
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
        if out.turns > 0 || out.worked_ms > 0 {
            self.count_today(|t| {
                t.turns += out.turns;
                t.worked_ms += out.worked_ms;
            });
        }
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

    // --- Widgets ----------------------------------------------------------------------

    fn list_setting(&self, key: &str) -> Vec<String> {
        self.setting(key).as_array().into_iter().flatten().filter_map(|v| v.as_str().map(str::to_string)).collect()
    }

    pub fn is_widget_on(&self, id: &str) -> bool {
        !self.list_setting("widgetsOff").iter().any(|o| o == id)
    }

    // Every widget, in the person's order, on or off.
    pub fn widgets_view(&self) -> Value {
        let (off, order) = (self.list_setting("widgetsOff"), self.list_setting("widgetOrder"));
        self.widgets.lock().unwrap().view(now_ms(), &off, &order, &scripts::owner_of)
    }

    // A widget from a script: there it is, and it may open the island for itself.
    pub fn put_widget(&self, v: &Value) -> Result<(), &'static str> {
        let put = self.widgets.lock().unwrap().put(v, now_ms())?;
        self.redraw();
        if let Some(words) = put.nudge {
            self.nudge_widget(&put.id, &words);
        }
        Ok(())
    }

    // The island opens for a widget, once: only in sight, when nudges are
    // allowed and the widget is on, and while no session needs you.
    pub fn nudge_widget(&self, id: &str, words: &str) {
        self.nudge_widget_to(id, words, Value::Null);
    }

    // The same, and a click on the island while it says so opens what
    // `open` names ({ tab, … }: a letter on the Mail page) instead of the
    // settings as they were.
    pub fn nudge_widget_to(&self, id: &str, words: &str, open: Value) {
        if !self.is_visible() || self.setting("widgetNudge") == false || !self.is_widget_on(id) || self.pet.lock().unwrap().wants_you() {
            return;
        }
        let Some((label, value, private)) = self.widgets.lock().unwrap().label_of(id) else { return };
        self.to_pages("pet:nudge", json!({ "id": id, "words": words, "label": label, "value": value, "private": private, "open": open }));
    }

    // Today's counts changed: kept, and the widget says so.
    pub fn count_today(&self, change: impl FnOnce(&mut widgets::Today)) {
        let words = {
            let mut today = self.today.lock().unwrap();
            today.roll(&widgets::local_date());
            change(&mut today);
            today.save(&self.dir);
            today.words()
        };
        if self.widgets.lock().unwrap().set_built_in("today", words, now_ms()) {
            self.redraw();
        }
    }

    // A chime from her home, and a system notification if asked for.
    fn alert(&self, mood: &str, project: &str) {
        let _ = self.app.emit_to("island", "pet:alert", json!({ "mood": mood }));
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
        let is_stale = connection::hooks_status(self.port) == "stale" || connection::codex_hooks_status() == "stale";
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

// A new home: the island or the taskbar (she goes into it).
pub fn set_display(sh: &Arc<Shared>, display: &str) {
    if sh.setting("display").as_str() == Some(display) && !sh.flag("out") {
        return;
    }
    if sh.flag("out") {
        pet::hide_now(sh);
    }
    sh.change(json!({ "display": display, "out": false }));
}

// Out on the desktop where she was last left, or back home, from the menu.
pub fn set_out(sh: &Arc<Shared>, out: bool) {
    if sh.flag("out") == out {
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
        sh.island.lock().unwrap().absorbing = false;
        sh.apply_visibility();
    });
}

// Started again while running, or "back to the corner": find her.
pub fn come_home(sh: &Arc<Shared>) {
    sh.hidden.store(false, Ordering::SeqCst);
    if sh.flag("out") {
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
        // An approval given on her, for today's count.
        if matches!(choice["action"].as_str(), Some("allow" | "always")) {
            sh.count_today(|t| t.approvals += 1);
        }
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
    if !sh.flag("out") {
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

// A session clicked (in the island, the settings, the panel): to its window.
// False when there is nowhere to go.
#[tauri::command]
async fn session_jump(app: AppHandle, id: String) -> bool {
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

#[tauri::command]
async fn pet_log(app: AppHandle, window: WebviewWindow, line: String) {
    shared(&app).log(&format!("{} page: {line}", window.label()));
}

// --- Started by the SessionStart hook ----------------------------------------------------

// The event a hook gives on stdin, if it comes soon.
fn read_event() -> Option<Value> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut text = String::new();
        let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut text);
        let _ = tx.send(text);
    });
    rx.recv_timeout(Duration::from_millis(1000)).ok().and_then(|text| serde_json::from_str(&text).ok())
}

// Start her, detached so she outlives the hook and the session (she greets on
// her own). On Windows she inherits no handle at all: a hook's process holds
// the ones its agent left inheritable, its output pipes among them, and
// whoever reads those to the end (Codex, or what runs Codex) would wait on
// her for good. std's Command always lets a child inherit, so CreateProcessW.
#[cfg(windows)]
fn start_detached() {
    use std::os::windows::ffi::OsStrExt;

    #[repr(C)]
    struct StartupInfo {
        cb: u32,
        reserved: *mut u16,
        desktop: *mut u16,
        title: *mut u16,
        x: u32,
        y: u32,
        x_size: u32,
        y_size: u32,
        x_chars: u32,
        y_chars: u32,
        fill: u32,
        flags: u32,
        show: u16,
        reserved2_len: u16,
        reserved2: *mut u8,
        stdin: isize,
        stdout: isize,
        stderr: isize,
    }
    #[repr(C)]
    struct ProcessInfo {
        process: isize,
        thread: isize,
        process_id: u32,
        thread_id: u32,
    }
    #[link(name = "kernel32")]
    extern "system" {
        #[allow(clippy::too_many_arguments)]
        fn CreateProcessW(
            app: *const u16,
            command_line: *mut u16,
            process_attrs: *const u8,
            thread_attrs: *const u8,
            inherit_handles: i32,
            flags: u32,
            env: *const u8,
            dir: *const u16,
            startup: *const StartupInfo,
            info: *mut ProcessInfo,
        ) -> i32;
        fn CloseHandle(handle: isize) -> i32;
    }

    let Ok(exe) = std::env::current_exe() else { return };
    let wide = |s: &std::ffi::OsStr| s.encode_wide().chain(Some(0)).collect::<Vec<u16>>();
    let app = wide(exe.as_os_str());
    let mut command_line = wide(std::ffi::OsStr::new(&format!("\"{}\"", exe.display())));
    // Her own folder, not the hook's (the project's): she would hold on to it.
    let dir = exe.parent().map(|d| wide(d.as_os_str()));
    // SAFETY: plain Win32 structs, zero is a valid start for both.
    let mut startup: StartupInfo = unsafe { std::mem::zeroed() };
    startup.cb = std::mem::size_of::<StartupInfo>() as u32;
    let mut info: ProcessInfo = unsafe { std::mem::zeroed() };
    // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP; no handles inherited (0).
    let flags = 0x0000_0008 | 0x0000_0200;
    let null = std::ptr::null();
    // SAFETY: every pointer is valid for the call or null where Win32 allows it.
    let dir = dir.as_ref().map_or(std::ptr::null(), |d| d.as_ptr());
    let ok = unsafe { CreateProcessW(app.as_ptr(), command_line.as_mut_ptr(), null, null, 0, flags, null, dir, &startup, &mut info) };
    if ok != 0 {
        unsafe {
            CloseHandle(info.thread);
            CloseHandle(info.process);
        }
    }
}

#[cfg(not(windows))]
fn start_detached() {
    let Ok(exe) = std::env::current_exe() else { return };
    let mut command = std::process::Command::new(exe);
    command.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    let _ = command.spawn();
}

// If no pet answers on the port, start one; if one does, hand her the event.
// Then leave, quick and quiet, whatever happens.
fn ensure_running(port: u16) {
    let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_millis(800))).build().new_agent();
    let up = agent
        .get(&format!("http://127.0.0.1:{port}/health"))
        .call()
        .ok()
        .and_then(|mut res| res.body_mut().read_to_string().ok())
        .is_some_and(|body| body.contains("wakuwaku"));
    let event = read_event();
    if up {
        if let Some(event) = event {
            let _ = agent.post(&format!("http://127.0.0.1:{port}/hook?from=wakuwaku")).send_json(&event);
        }
    } else {
        start_detached();
    }
}

// --- Run by Codex's hooks -----------------------------------------------------------------

// Codex runs its hooks as commands: this program with CODEX_FLAG, for every
// event. Hand her the event, and print her answer to a prompt, the decision
// Codex reads; nothing else, since Codex takes what a hook prints as words
// for the model. SessionStart starts her when she is not up. Whatever
// happens, leave with 0, so Codex never shows a failed hook.
fn codex_hook(port: u16) {
    let Some(mut event) = read_event() else { return };
    // Where the Codex session runs (the processes above this one), for going
    // to its window from the pet (jump.rs).
    if let Some(fields) = event.as_object_mut() {
        fields.insert(jump::CODEX_CHAIN.into(), jump::chain_json(&jump::hook_chain()));
    }
    let name = event["hook_event_name"].as_str().unwrap_or("").to_string();
    let is_prompt = name == "PermissionRequest";
    // A prompt waits for the person, just under the hook's own timeout.
    let wait = if is_prompt { Duration::from_secs(connection::CODEX_PROMPT_TIMEOUT - 5) } else { Duration::from_secs(3) };
    let agent = ureq::Agent::config_builder().timeout_global(Some(wait)).build().new_agent();
    match agent.post(&format!("http://127.0.0.1:{port}/hook?from=wakuwaku&agent=codex")).send_json(&event) {
        Ok(mut res) if is_prompt => {
            let body = res.body_mut().read_to_string().unwrap_or_default();
            if serde_json::from_str::<Value>(&body).is_ok_and(|v| v.get("hookSpecificOutput").is_some()) {
                let mut out = std::io::stdout();
                let _ = out.write_all(body.as_bytes());
                let _ = out.flush();
            }
        }
        Ok(_) | Err(ureq::Error::StatusCode(_)) => {}
        Err(_) if name == "SessionStart" => start_detached(),
        Err(_) => {}
    }
}

fn main() {
    // Her exe again, as the taskbar's guard (shell.rs): it waits for her to
    // end, and gives Windows' taskbar back if she could not.
    let args: Vec<String> = std::env::args().collect();
    if let Some((pid, file)) = shell::guard_args(&args) {
        return shell::guard(pid, file);
    }
    let port: u16 = std::env::var("WAKUWAKU_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(47213);
    if std::env::args().any(|a| a == connection::ENSURE_FLAG || a == connection::LEGACY_ENSURE_FLAG) {
        return ensure_running(port);
    }
    if std::env::args().any(|a| a == connection::CODEX_FLAG) {
        return codex_hook(port);
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
            session_jump,
            settings::settings_get,
            settings::settings_set,
            settings::settings_open,
            settings::settings_closed,
            settings::settings_show_pet,
            settings::settings_login,
            settings::settings_hooks,
            settings::settings_codex_hooks,
            settings::settings_fetch,
            settings::settings_gallery,
            settings::settings_open_site,
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
            taskbar::taskbar_tray,
            taskbar::taskbar_open,
            taskbar::taskbar_tray_rects,
            taskbar::taskbar_window,
            taskbar::taskbar_tray_pin,
            taskbar::taskbar_app,
            taskbar::taskbar_thumbs,
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
                // Codex's pets are only read, never made a folder for.
                if folder != data::codex_pets() {
                    let _ = std::fs::create_dir_all(&folder);
                }
                let _ = app.asset_protocol_scope().allow_directory(&folder, true);
            }
            notify::register(&dir);

            let today = widgets::Today::load(&dir, &widgets::local_date());
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
                widgets: Mutex::new(widgets::Widgets::default()),
                scripts: Mutex::new(scripts::Runner::default()),
                mail: Mutex::new(mail::Runner::default()),
                today: Mutex::new(today),
                screens: Mutex::new(screen::read(app.handle())),
                hidden: AtomicBool::new(false),
                by_fullscreen: AtomicBool::new(false),
                greeted: AtomicBool::new(false),
                codex_seen: std::sync::atomic::AtomicU64::new(0),
                own: Mutex::new(Vec::new()),
                prev_focus: Mutex::new(0),
                push_pending: AtomicBool::new(false),
                shuffle_until: std::sync::atomic::AtomicU64::new(0),
                evals: Mutex::new((0, std::collections::HashMap::new())),
            });
            app.manage(sh.clone());
            sh.log(&format!("up on port {}, data in {}", sh.port, sh.dir.display()));
            taskbar::recover(&sh);

            pet::create(&sh)?;
            // An app full screen as she starts, known before her home first
            // shows: the taskbar takes its strip only once it can show (one
            // taken with its window hidden keeps no room).
            if fullscreen::AVAILABLE && (sh.flag("hideInFullscreen") || sh.home() == "taskbar") {
                sh.by_fullscreen.store(fullscreen::check(&[]) == Some(true), Ordering::SeqCst);
            }
            island::apply_visibility(&sh);
            tray::create(&sh)?;
            server::serve(sh.clone(), listener);
            // The mail accounts that are on, watched from the start.
            mail::sync(&sh);

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

            // The widgets: today's from the start; the tokens' each minute
            // while they are on; a script's gone once it stops sending.
            sh.count_today(|_| {});
            let watcher = sh.clone();
            std::thread::spawn(move || {
                let mut tokens = tokens::Tokens::default();
                let (claude_dir, codex_dir) = (tokens::claude_dir(), tokens::codex_dir());
                for n in 0u64.. {
                    let now = now_ms();
                    let mut changed = watcher.widgets.lock().unwrap().expire(now);
                    // A built-in widget: its words while it is on, gone when off.
                    let mut built_in = |id: &str, words: Option<Value>| {
                        let mut widgets = watcher.widgets.lock().unwrap();
                        changed |= match words {
                            Some(value) => widgets.set_built_in(id, value, now),
                            None => widgets.remove_built_in(id),
                        };
                    };
                    // Just turned on: read now, not at its next turn.
                    let unread = |id: &str| watcher.is_widget_on(id) && watcher.widgets.lock().unwrap().label_of(id).is_none();
                    if n % (tokens::EVERY.as_secs() / 3) == 0 || unread("tokens") {
                        let count = watcher.is_widget_on("tokens").then(|| {
                            tokens.update(&claude_dir, &codex_dir, widgets::local_midnight());
                            tokens::words(tokens.count())
                        });
                        built_in("tokens", count);
                    }
                    // A new day: today's counts start again.
                    if watcher.today.lock().unwrap().date != widgets::local_date() {
                        watcher.count_today(|_| {});
                    }
                    // The plugins she runs: started, stopped, started again.
                    changed |= scripts::sync(&watcher);
                    if changed {
                        watcher.redraw();
                    }
                    std::thread::sleep(Duration::from_secs(3));
                }
            });

            // The monitor: the parts that are on, every so many seconds (its
            // setting, 1 to 10), while it is on; looked at again within a
            // quarter second when it is turned on or its pace changes.
            let monitored = sh.clone();
            std::thread::spawn(move || {
                let mut monitor = widgets::Monitor::new();
                let mut next = std::time::Instant::now();
                let mut was = (false, 0);
                loop {
                    let watching = widgets::Watching::from(&monitored.setting("monitor"));
                    let on = monitored.is_widget_on("monitor");
                    if std::time::Instant::now() >= next || (on, watching.every) != was {
                        was = (on, watching.every);
                        next = std::time::Instant::now() + Duration::from_secs(watching.every);
                        let words = if on { monitor.read(&watching) } else { None };
                        let changed = {
                            let mut widgets = monitored.widgets.lock().unwrap();
                            match words {
                                Some(value) => widgets.set_built_in("monitor", value, now_ms()),
                                None => widgets.remove_built_in("monitor"),
                            }
                        };
                        if changed {
                            monitored.redraw();
                        }
                    }
                    std::thread::sleep(Duration::from_millis(250));
                }
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
                        // The taskbar steps aside for an app full screen, as Windows' own does.
                        let watching = fullscreen::AVAILABLE && (ticker.flag("hideInFullscreen") || ticker.home() == "taskbar");
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
            // Never set up: the settings, where Claude Code or Codex gets connected.
            if !sh.flag("onboarded") && connection::connection(port) == "none" && !connection::is_codex_connected() {
                island::open_settings(&sh, Some("connect"));
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while running wakuwaku")
        // Quitting: the bar's strip back to the other windows, and the
        // plugins she ran stopped.
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                island::release_bar(&shared(app));
                scripts::stop_all(&shared(app));
                mail::stop_all(&shared(app));
            }
        });
}
