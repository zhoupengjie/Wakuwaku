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
//                  the island's page (src/pet/addons.js), their views in the cards
//                  (src/pet/views.js), served by wakuplugin://
//   zip.rs         zips read and unpacked (LHM's download, a plugin's package)
//   tokens.rs      today's tokens, from Claude Code's, Codex's, Antigravity's and DSH's own records
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
mod fetch;
mod focus;
mod fullscreen;
mod gpu;
mod hw;
mod i18n;
mod icons;
mod inputs;
mod island;
mod jump;
mod lhm;
mod lhm_setup;
mod logon;
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
mod proxy;
#[cfg(test)]
mod replay;
mod screen;
mod scripts;
mod sensors;
mod server;
mod settings;
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
mod widgets;
mod winkeys;
mod zip;

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Map, Value};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

// The island's own animation of taking her in, before she counts as in.
const ABSORB_MS: u64 = 380;

// Her port, unless WAKUWAKU_PORT says another (a test's copy, mostly): the
// agents' hub's (agent-hub PORT), where its other apps find it.
const DEFAULT_PORT: u16 = agent_hub::PORT;

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
    // The agents' sessions and events, for other apps on this machine
    // (agent-hub, server.rs /v1).
    pub hub: agent_hub::Hub,
    // Plugins in the island (widgets.rs), and what today has seen.
    pub widgets: Mutex<widgets::Widgets>,
    pub today: Mutex<widgets::Today>,
    // The plugins she runs for you (plugins.rs).
    pub plugins: Mutex<plugins::Runner>,
    // The mail accounts being watched (mail.rs).
    pub mail: Mutex<mail::Runner>,
    screens: Mutex<screen::Screens>,
    // Out of sight: hidden from the tray, or another app is full screen.
    pub hidden: AtomicBool,
    by_fullscreen: AtomicBool,
    // Nobody at the keyboard or mouse for a while (check_quiet).
    quiet: AtomicBool,
    greeted: AtomicBool,
    // When each agent's last event came (ms), 0 never, by its place in
    // connection::ALL (connection::slot): the settings tell connected from
    // installed, waiting for a session (agents.js). Codex runs its hooks only
    // once the person trusts them; omp loads its extension only as it
    // starts after it was written.
    pub seen: [std::sync::atomic::AtomicU64; 8],
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

    // How long a session at rest stays in sight with nothing new from it (ms,
    // 0 always): the idleHideMin setting, or WAKUWAKU_IDLE_HIDE_MS, for a
    // test that cannot wait (state.rs is_shown).
    pub fn idle_hide_ms(&self) -> u64 {
        static TEST: std::sync::OnceLock<Option<u64>> = std::sync::OnceLock::new();
        let test = *TEST.get_or_init(|| std::env::var("WAKUWAKU_IDLE_HIDE_MS").ok().and_then(|s| s.parse().ok()));
        test.unwrap_or_else(|| self.setting("idleHideMin").as_u64().unwrap_or(state::IDLE_HIDE_MIN) * 60 * 1000)
    }

    pub fn screens(&self) -> screen::Screens {
        self.screens.lock().unwrap().clone()
    }

    // Where a prompt can be answered: her up, or, put away only by an app
    // full screen on her display, her taskbar still up on another (bars.rs),
    // whose capsule shows it waiting and brings up her island for it. Else
    // nobody can answer here, and the terminal has it at once (server.rs,
    // apply_visibility); it had them even while her capsule on another
    // display showed (the test session, 2026-10-13).
    pub fn asks_seen(&self) -> bool {
        self.is_visible() || (!self.hidden.load(Ordering::SeqCst) && !self.flag("dnd") && self.is_fullscreen() && bars::any_shown())
    }

    pub fn is_visible(&self) -> bool {
        !self.hidden.load(Ordering::SeqCst)
            && !self.flag("dnd")
            && !(self.flag("hideInFullscreen") && self.by_fullscreen.load(Ordering::SeqCst))
    }

    // Nobody at the keyboard or mouse for a while: she holds still, the
    // loops look less often, and Windows may run her on its efficient cores
    // at lower clocks (power.rs). Over the moment anything is touched.
    pub fn is_quiet(&self) -> bool {
        self.quiet.load(Ordering::SeqCst)
    }

    // Looked at by the clock (every second) and by the mouse's own loop.
    pub fn check_quiet(&self) -> bool {
        let after = quiet_after_ms();
        let quiet = after > 0 && power::idle_ms() >= after;
        if self.quiet.swap(quiet, Ordering::SeqCst) != quiet {
            let eco = power::process_eco(quiet);
            // WebView2's processes, where her pages' work is, with her; and
            // their memory (#14).
            let webview_eco = power::webview_eco(quiet);
            let windows = self.app.webview_windows();
            for window in windows.values() {
                power::webview_memory(window, quiet);
            }
            self.log(&format!("quiet: {quiet} (efficiency mode set: {eco}; for {webview_eco} of WebView2's processes; memory {} for {} pages)", if quiet { "low" } else { "normal" }, windows.len()));
            self.to_pages("pet:quiet", json!({ "on": quiet }));
            // The plugins' views on the desktop away while nobody is about.
            desk::views(self);
            wake_monitor();
            // The plugins she runs at their quiet pace, or back to their own.
            plugins::wake();
            if !quiet {
                taskbar::wake();
            }
        }
        quiet
    }

    // Another app is full screen (watched always: look_fullscreen).
    // The displays she is on (x, y, w, h): her island's or taskbar's, and
    // hers while she is out. An app full screen on another is none of hers.
    pub fn displays(&self) -> Vec<(i32, i32, i32, i32)> {
        let mut on: Vec<(i32, i32, i32, i32)> = island::display(self).into_iter().collect();
        if let Some(mon) = pet::is_shown(self).then(|| pet::display(self)).flatten() {
            if !on.contains(&mon) {
                on.push(mon);
            }
        }
        on
    }

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

    // Whether she keeps start at login to where she is (logon.rs):
    // on her own port, with her own data folder; a test's copy on another
    // port, or with a folder of its own, leaves it as it is.
    pub fn keeps_login(&self) -> bool {
        self.port == DEFAULT_PORT && std::env::var_os("WAKUWAKU_USER_DATA").is_none()
    }

    // Start at login kept up as she starts, off the thread starting her
    // (logged too: how long after the logon she came up and what started
    // her, `by`: logon::started_by).
    pub fn keep_login(self: &Arc<Self>, by: Option<&'static str>) {
        let sh = self.clone();
        spawn_named("login", move || {
            if let Some(since) = logon::since_logon() {
                sh.log(&format!("started {} after logon{}", logon::how_long(since), logon::said_by(by)));
            }
            if !sh.keeps_login() {
                return;
            }
            for line in logon::login_at_start() {
                sh.log(&format!("login: {line}"));
            }
        });
    }

    pub fn own_window(&self, _win: &WebviewWindow) {
        #[cfg(windows)]
        if let Ok(hwnd) = _win.hwnd() {
            self.own.lock().unwrap().push(hwnd.0 as isize);
        }
    }

    // A line in recent.log always (what a crash report tells of what came
    // before, crash.rs), and in debug.log while debugging.
    pub fn log(&self, line: &str) {
        let whole = format!("{} [tauri] {line}\n", now_ms());
        crash::tail(&self.dir, &whole);
        if !self.is_logging {
            return;
        }
        // A whole line in one write, one at a time: written in its parts
        // (writeln!), two threads' lines in the same moment came out woven
        // into each other (2026-10-11, the quit's steps). The file is kept
        // open (it was opened and closed for every line); opened again
        // should a write fail (the file taken away).
        static LOG: Mutex<Option<std::fs::File>> = Mutex::new(None);
        let mut log = LOG.lock().unwrap_or_else(|e| e.into_inner());
        let written = log.as_mut().is_some_and(|f| f.write_all(whole.as_bytes()).is_ok());
        if !written {
            *log = OpenOptions::new().create(true).append(true).open(self.dir.join("debug.log")).ok();
            if let Some(f) = log.as_mut() {
                let _ = f.write_all(whole.as_bytes());
            }
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
        now["sprite"] = sprite.map_or(Value::Null, |p| json!(p.url()));
        now["spriteVersion"] = json!(sprite.map_or(2, |p| p.version));
        // The next session that wants something, for the island's second bubble.
        now["second"] = json!(second);
        // Every session, the one shown first: the open island lists the others.
        now["list"] = list;
        // The island risen only for the settings, and whether they are open:
        // then the island holds any prompt.
        now["islandTemp"] = json!(temp);
        now["settingsOpen"] = json!(settings_open);
        now["quiet"] = json!(self.is_quiet());
        // An app full screen on her display: no flow light's ring then, the
        // dot alone (island.js cue; pet:full-screen as it changes).
        now["fullScreen"] = json!(self.is_fullscreen());
        // Whether a blur of what is behind the island can be put under it
        // (backplate.rs): Windows 11, transparency effects on.
        now["plate"] = json!(backplate::available());
        // Windows' mode, for her island's look at the top of the screen (the
        // taskbar hears it from taskbar.rs, taskbar:look).
        if self.home() != "taskbar" {
            now["winLight"] = json!(wallpaper::look().0);
            // The side of the top of the screen her island's window is at
            // (islandPos, zhoupengjie/Wakuwaku#59): the page lays it out there.
            now["islandAt"] = json!(island::side(self).name());
        }
        // The widgets that have something to say: those on take their turns
        // in the island while nothing needs you; any may be on the taskbar.
        now["widgets"] = self.widgets_said();
        // Nothing connected (Claude Code, Codex, Antigravity): the island
        // says so, naming those on this computer, and a press on it opens
        // the settings where they are (island.js).
        let unconnected = !connection::seems_connected();
        now["unconnected"] = json!(unconnected);
        if unconnected {
            now["tools"] = json!(connection::tools_here());
        }
        now
    }

    // Everything the pages draw from, for her window as it shows.
    pub fn send_to_pet(&self) {
        let _ = self.app.emit_to("pet", "pet:update", self.payload());
    }

    fn to_pages(&self, event: &str, payload: Value) {
        // Her window hidden (she is home) hears no updates: it gets the
        // whole state as it shows (pet.rs apply_visibility).
        let pet_hears = event != "pet:update" || pet::is_up(self);
        for label in ["pet", "island"] {
            if label == "pet" && !pet_hears {
                continue;
            }
            let _ = self.app.emit_to(label, event, payload.clone());
        }
    }

    // The mood or the settings changed: the pages, the tray, the settings.
    pub fn redraw(&self) {
        self.to_pages("pet:update", self.payload());
        tray::refresh(self);
        self.push_settings();
        // The desktop's widgets (desk.rs), their words.
        desk::push_widgets(self);
    }

    // Only a widget's words changed (the monitor's readings every couple of
    // seconds): the island's widgets alone, not the whole state to both
    // pages, the tray and the settings.
    pub fn redraw_widgets(&self) {
        let _ = self.app.emit_to("island", "pet:widgets", json!({ "widgets": self.widgets_said() }));
        self.push_settings();
        desk::push_widgets(self);
    }

    // The prompts waiting changed.
    pub fn push_asks(&self) {
        let (views, none, handed) = {
            let mut asks = self.asks.lock().unwrap();
            (asks.views(), asks.is_empty(), asks.take_handed())
        };
        // DeepSeek Harness's handed back to its own window: its session waits
        // to be answered there (asks.rs handed_event), as her line says and
        // the apps listening are told.
        for handed in &handed {
            let ev = asks::handed_event(handed);
            self.hub.publish(&ev, now_ms());
            self.apply_event(&ev, None);
        }
        self.to_pages("pet:asks", views);
        self.push_settings();
        // Her taskbar on the other displays says how many wait (bars.rs);
        // her island risen for them goes once they are answered.
        bars::refresh(self);
        if none {
            island::asks_answered(&shared(&self.app));
        }
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

    // A reaction of her own, played once over her mood: { react, say }.
    pub fn react(&self, react: &str, say: Value) {
        self.to_pages("pet:react", json!({ "react": react, "say": say }));
    }

    // An agent's event (from its hook, or POST /state) into her sessions,
    // with where its session runs when that was looked up (server.rs); false
    // when it is nothing to a session she shows.
    pub fn apply_event(&self, ev: &agent_core::event::AgentEvent, chain: Option<state::Chain>) -> bool {
        use agent_core::event::Agent;
        let out = self.pet.lock().unwrap().apply(ev, chain, now_ms(), self.hold());
        let Some(out) = out else { return false };
        if out.turns > 0 || out.worked_ms > 0 {
            // Antigravity's and Reasonix's approvals never come to her, nor
            // DeepSeek Harness's through its hook bridge (through her plugin
            // they may: those given on her are counted all the same).
            let is_agy = matches!(ev.agent, Agent::Agy | Agent::Dsh | Agent::Reasonix);
            self.count_today(|t| {
                t.begun(out.turns);
                if is_agy {
                    t.agy_turns += out.turns;
                }
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

    // In the closed island: on, and not put on the desktop, nor on the
    // taskbar while that is her home (a widget in one place, settings.rs
    // is_placed_out).
    pub fn is_in_island(&self, id: &str) -> bool {
        self.is_widget_on(id) && !settings::is_placed_out(&self.setting("deskLayout"), &self.setting("taskbarLayout"), self.home() == "taskbar", id)
    }

    // On in the island, put on the taskbar (taskbarLayout) while she is the
    // taskbar, or on the desktop (deskLayout): a built-in one is worked out then.
    pub fn is_widget_wanted(&self, id: &str) -> bool {
        self.is_widget_on(id) || (self.setting("display") == "taskbar" && settings::is_on_taskbar(&self.setting("taskbarLayout"), id)) || (cfg!(windows) && settings::is_on_desk(&self.setting("deskLayout"), id))
    }

    // The widgets with something to say, each saying whether it is on.
    fn widgets_said(&self) -> Value {
        Value::Array(self.widgets_view().as_array().into_iter().flatten().filter(|w| !w["value"].is_null()).cloned().collect())
    }

    // Every widget, in the person's order, on or off.
    pub fn widgets_view(&self) -> Value {
        let (off, order) = (self.list_setting("widgetsOff"), self.list_setting("widgetOrder"));
        self.widgets.lock().unwrap().view(now_ms(), &off, &order, &|id: &str| plugins::owner_of(id))
    }

    // A widget from a script: there it is, and it may open the island for
    // itself. Never one by the id of a third-party plugin's widget that is
    // up now (not any id merely beginning like a plugin's: finding 11), so a
    // plugin cannot claim the person's `cpu_temp` next to a `cpu` plugin.
    pub fn put_widget(&self, v: &Value) -> Result<(), &'static str> {
        if v.get("id").and_then(Value::as_str).is_some_and(|id| self.widgets.lock().unwrap().is_addon(id)) {
            return Err("id: taken by a plugin");
        }
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
        let (words, detail) = {
            let mut today = self.today.lock().unwrap();
            today.roll(&widgets::local_date());
            change(&mut today);
            today.save(&self.dir);
            (today.words(), today.detail())
        };
        if self.widgets.lock().unwrap().set_built_in_with("today", words, detail, now_ms()) {
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
        let is_stale = connection::hooks_status(self.port) == "stale" || connection::codex_hooks_status() == "stale" || connection::opencode_status(self.port) == "stale";
        self.react("wave", json!({ "key": if is_stale { "say.hooksStale" } else { "say.arrived" }, "vars": {} }));
    }

    pub fn change(self: &Arc<Self>, patch: Value) {
        let Value::Object(patch) = patch else { return };
        let touches_visibility = ["dnd", "display", "out", "hideInFullscreen", "taskbarEdge", "taskbarDisplays"].iter().any(|k| patch.contains_key(*k));
        let touches_top = ["settingsPin", "settingsTop"].iter().any(|k| patch.contains_key(*k));
        let touches_monitor = ["monitor", "monitorSource", "widgetsOff"].iter().any(|k| patch.contains_key(*k));
        let touches_stow = patch.contains_key("islandHideUnder");
        let touches_shortcuts = ["shortcutsEnabled", "shortcutApprove", "shortcutDeny", "shortcutIsland", "dnd"].iter().any(|k| patch.contains_key(*k));
        {
            let mut settings = self.settings.lock().unwrap();
            settings.extend(patch);
            data::save(&self.dir, &settings);
        }
        if touches_monitor {
            wake_monitor();
        }
        // A window under her island looked for at once (island.rs look_under).
        if touches_stow {
            fullscreen::ask();
        }
        self.redraw();
        tray::refresh_menu(self);
        if touches_visibility {
            self.apply_visibility();
        }
        if touches_shortcuts {
            shortcuts::update(self);
        }
        // Her taskbar on the other displays: its material, its buttons' alignment, her language (bars.rs).
        bars::refresh(self);
        // The desktop's widgets: made or gone as the layout says, told the rest (desk.rs).
        desk::sync(self);
        if touches_top {
            island::keep_on_top(self);
        }
    }

    // Another app full screen or no longer: she (and the taskbar, as Windows'
    // own does) steps aside or comes back. Looked at on a thread of its own
    // (setup): at once when a window comes to the front or the one in front
    // changes its size (fullscreen::ask, from the taskbar's hooks), else
    // every 1.5 s. Never on the taskbar's thread: the strip shown again from
    // there waited on the main thread while that waited on it, and coming
    // back after an app full screen took 3.8 s (2026-10-10). Every look
    // logged for 10 s after a change: asked how, and what it saw. Watched
    // whatever the settings say: her island left up over an app full screen
    // (hideInFullscreen off) still plays no flow light's ring over it
    // (island.js), told so as it changes (pet:full-screen).
    pub fn look_fullscreen(self: &Arc<Self>, asked: Option<Instant>) {
        static CHANGED: Mutex<Option<Instant>> = Mutex::new(None);
        let watching = fullscreen::AVAILABLE;
        let own = self.own.lock().unwrap().clone();
        let cover = if watching { fullscreen::covering(&own) } else { Some(None) };
        // Her taskbar on the other displays steps aside on its own display alone.
        if let Some(cover) = cover {
            bars::full_screen(self, cover);
        }
        let on = self.displays();
        let is_full = cover.map(|c| c.is_some_and(|mon| on.is_empty() || on.contains(&mon)));
        let how = match asked {
            Some(at) => format!("event, {} ms after", at.elapsed().as_millis()),
            None => "timer".to_string(),
        };
        let changed = is_full.is_some_and(|f| self.by_fullscreen.swap(f, Ordering::SeqCst) != f);
        if !changed {
            if CHANGED.lock().unwrap().is_some_and(|at| at.elapsed() < Duration::from_secs(10)) {
                let saw = is_full.map_or("one of hers in front".to_string(), |f| f.to_string());
                self.log(&format!("full screen looked at ({how}): {saw}"));
            }
            return;
        }
        *CHANGED.lock().unwrap() = Some(Instant::now());
        let is_full = is_full == Some(true);
        let which = if is_full { format!(", {}", fullscreen::front()) } else { String::new() };
        let which = format!("{which}; hers on {:?}", self.displays());
        self.log(&format!("another app full screen: {is_full}{which} ({how})"));
        let _ = self.app.emit_to("island", "pet:full-screen", json!({ "on": is_full }));
        let t = Instant::now();
        self.apply_visibility();
        shortcuts::update(self);
        self.log(&format!("full screen: {} in {} ms", if is_full { "stepped aside" } else { "back" }, t.elapsed().as_millis()));
    }

    pub fn apply_visibility(self: &Arc<Self>) {
        pet::apply_visibility(self);
        island::apply_visibility(self);
        // Out of sight, nobody can answer here: the terminal has them.
        if !self.asks_seen() {
            let n = self.asks.lock().unwrap().dismiss_all();
            if n > 0 {
                self.log(&format!("{n} prompt(s) handed to the terminal: out of sight"));
                self.push_asks();
            }
        }
        tray::refresh(self);
        // Back in sight: the pomodoro's count and the dev servers at their
        // own pace again, not the slower one kept while unseen.
        plugins::wake();
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

    // Quit (the menu's Quit and POST /quit by choice; POST /quit?wake=1, an
    // update's or a restart's handover so that a session may start her:
    // data.rs note_quit; `why`, for the mark and the log), never taking
    // longer than 8 s: a quit once hung (80 s, Windows' taskbar with it,
    // 2026-10-11). Ended then, whatever is stuck; the guard gives Windows'
    // taskbar back once she is gone. A second quit while one is under way
    // does nothing more, but mark one by choice.
    pub fn quit(self: &Arc<Self>, how: data::Quit, why: &str) {
        let first = !QUITTING.swap(true, Ordering::SeqCst);
        data::note_quit(&self.dir, how, first, why);
        let after = match how {
            data::Quit::ByChoice => "by choice: no session's hook starts her until she is opened again",
            data::Quit::Wakeable => "a session may start her again",
        };
        if !first {
            if how == data::Quit::ByChoice {
                self.log(&format!("quitting already; {why}: {after}"));
            }
            return;
        }
        self.log(&format!("quitting ({why}), {after}"));
        let watched = self.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(8));
            watched.log("quitting: not done in 8 s: ended");
            end_now();
        });
        // The handover first (the taskbar's dust over Windows' shown again,
        // two seconds at most), off the main thread: its events reach the
        // page through it.
        let going = self.clone();
        std::thread::spawn(move || {
            island::handoff_out(&going);
            going.app.exit(0);
        });
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
            // To the front by borrowing the input of the window there
            // (jump::bring_window): taken after Win came up (winkeys.rs), the
            // foreground was not hers to take, and tao's way (an Alt pressed
            // for it) went to the window in front, its menu bar opened, the
            // keyboard left there (with Narrator on, 2026-10-14).
            #[cfg(windows)]
            crate::jump::bring_window(mine);
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

// Her menu; on the volume, the network or the clock of her taskbar (part),
// the one Windows' own taskbar has there (tray.rs part_menu).
#[tauri::command]
async fn pet_menu(app: AppHandle, window: WebviewWindow, part: Option<String>) {
    let sh = shared(&app);
    let menu = part.as_deref().and_then(|p| tray::part_menu(&sh, p)).unwrap_or_else(|| tray::menu(&sh, if is_island(&window) { tray::Over::Home } else { tray::Over::Her }));
    if let Ok(menu) = menu {
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
async fn pet_dismiss(app: AppHandle, id: u64) {
    let sh = shared(&app);
    if sh.asks.lock().unwrap().dismiss(id) {
        sh.log(&format!("prompt {id} handed to the terminal: asked to on her"));
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

// A shortcut being recorded in the settings: her global ones let go
// meanwhile (shortcuts.rs pause).
#[tauri::command]
async fn pet_shortcuts_pause(on: bool) {
    shortcuts::pause(on);
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

// The taskbar's layout being edited (island.js editTaskbar), and where its
// bar, library and tools are in the page: the window keeps the mouse there.
#[tauri::command]
async fn island_editing(app: AppHandle, on: bool, rects: Vec<[f64; 4]>, page_sf: Option<f64>) {
    island::set_editing(&shared(&app), on, rects, page_sf);
}

// The handover with Windows' taskbar: the page's answers (covered, bare, done).
#[tauri::command]
fn taskbar_handoff(what: String) {
    island::handoff_answer(&what);
}

// Where the page laid her island out on the taskbar: her seat, along the
// strip from its start (x on a bottom or top bar, y on a side bar).
#[tauri::command]
async fn island_seat(app: AppHandle, x: f64) {
    island::set_seat(&shared(&app), x);
}

// Something opened up from her taskbar's strip (the island, a list, a
// flyout): its window over any other that came over it (island.rs raise,
// zhoupengjie/Wakuwaku#55).
#[tauri::command]
async fn island_raise(app: AppHandle, window: WebviewWindow) {
    if is_island(&window) {
        island::raise(&shared(&app));
    }
}

// Her island at the top of the screen: up out of the way of the window
// under it or not, and its closed size (logical px), for the look for a
// window under it and the pointer calling it down (island.rs set_stow).
#[tauri::command]
async fn island_stow(app: AppHandle, window: WebviewWindow, stowed: bool, width: f64, height: f64) {
    if is_island(&window) {
        island::set_stow(&shared(&app), stowed, width, height);
    }
}

// Under her island, a blur of what is behind it (backplate.rs): where the
// page puts it ({ x, y, width, height } in its px, set in from the island's
// edge; the size of the page it measured in; Windows' mode), faded in over
// fade ms; none, faded out. Measured in a window of another size (resized
// meanwhile), it is not shown: the page sends it again once resized.
#[tauri::command]
async fn island_plate(app: AppHandle, rect: Option<Value>, fade: u32) {
    let sh = shared(&app);
    let Some(r) = rect else { return backplate::set(&sh, None, fade) };
    let ((ox, oy), (ww, wh), sf) = island::frame(&sh);
    let num = |key: &str| r[key].as_f64().unwrap_or(0.0);
    if (num("vw") * sf - ww as f64).abs() > 2.0 || (num("vh") * sf - wh as f64).abs() > 2.0 {
        sh.log(&format!("plate: measured in a {}x{} page, the window {ww}x{wh} at {sf}: not shown", num("vw"), num("vh")));
        return;
    }
    let (x, y, w, h) = backplate::on_screen((ox, oy), sf, (num("x"), num("y"), num("width"), num("height")));
    let plate = (w > 0 && h > 0).then_some(backplate::Plate { x, y, w, h, dark: r["dark"].as_bool() != Some(false) });
    backplate::set(&sh, plate, fade);
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

// The monitor's thread woken from its wait (setup): its settings changed
// (Shared::change), the quiet began or ended (Shared::check_quiet).
static MONITOR_WAKE: (Mutex<bool>, std::sync::Condvar) = (Mutex::new(false), std::sync::Condvar::new());

pub(crate) fn wake_monitor() {
    let (woken, cv) = &MONITOR_WAKE;
    *woken.lock().unwrap() = true;
    cv.notify_one();
}

// The pointer's loop woken from its wait (setup): Ctrl pressed or let go
// (desk.rs, its keyboard hook), so the desktop takes or lets go of the
// mouse at once. Called from that hook: no more than an unpark.
static POINTER_LOOP: std::sync::OnceLock<std::thread::Thread> = std::sync::OnceLock::new();

pub(crate) fn wake_pointer() {
    if let Some(t) = POINTER_LOOP.get() {
        t.unpark();
    }
}

// Nobody at the keyboard or mouse this long: quiet (Shared::check_quiet).
// WAKUWAKU_QUIET_AFTER_MS, for a test that cannot wait; 0 never.
pub const QUIET_AFTER_MS: u64 = 90_000;

fn quiet_after_ms() -> u64 {
    static AFTER: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *AFTER.get_or_init(|| std::env::var("WAKUWAKU_QUIET_AFTER_MS").ok().and_then(|s| s.parse().ok()).unwrap_or(QUIET_AFTER_MS))
}

// What WebView2 is started with: wry's own (its Office and PDF parts and
// SmartScreen off); her two pages in one renderer process, not one each
// (same origin, same show; the second was 90 MB and its own idle); the
// network service in its browser process, not a process of its own (#14;
// the storage service's process has no such switch any more: it stays);
// WebRTC's non-proxied UDP off, so a plugin cannot reach the network by
// WebRTC past its frame's Content-Security-Policy and past her proxy
// (the plugins review, finding 2; she uses WebRTC nowhere herself, and
// with no proxy set this leaves it no way out by UDP); and rendering on the
// graphics card that drives her display (gpu.rs, #13). WebView2 takes them
// once, for all her windows (another set refused): worked out as the first
// is made, her home's display then the primary one. A self-update changes
// them across versions safely because the new exe makes its WebView2 only
// after the old one has ended (updates.rs wait_after_update, hand_over).
// WAKUWAKU_BROWSER_ARGS, more of them (to try one).
pub fn browser_args(sh: &Shared) -> &'static str {
    static ARGS: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ARGS.get_or_init(|| {
        let mut args = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --enable-features=NetworkServiceInProcess2 --renderer-process-limit=1 --force-webrtc-ip-handling-policy=disable_non_proxied_udp".to_string();
        match island::display(sh).and_then(gpu::switch_for) {
            Some((switch, display)) => {
                sh.log(&format!("webview: rendering on the card driving {display} ({switch})"));
                args = format!("{args} {switch}");
            }
            None => sh.log("webview: its card left to Chromium (her display's card not told, or it renders nothing)"),
        }
        if let Ok(more) = std::env::var("WAKUWAKU_BROWSER_ARGS") {
            args = format!("{args} {more}");
        }
        args
    })
}

// Where WebView2 keeps its own files for her pages: in her data folder
// (portable, as the rest), not Windows' shared place for the app id. Two
// copies of her sharing that place with different browser arguments cannot
// both start (WebView2 refuses the second, 0x8007139F): an older build
// beside this one, as in a seamless swap, or a test copy.
pub fn webview_dir(sh: &Shared) -> PathBuf {
    sh.dir.join("webview")
}

// A thread with a name, for whoever looks at her threads' CPU.
pub fn spawn_named<F: FnOnce() + Send + 'static>(name: &str, f: F) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new().name(name.to_string()).spawn(f).expect("a thread")
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
// her own), out of its agent's job and inheriting no handle (detach.rs);
// told who starts her (`who`, a hook's words: starts.rs), for her log.
fn start_detached(who: &str) {
    let Ok(exe) = data::exe() else { return };
    if !detach::spawn(&exe, &[starts::BY_FLAG, &format!("a session's hook, {who}")]) {
        hook_log(&format!("{who}: she would not start"));
    }
}

// A hook's line in her log, she not up to write it (hers once she is).
fn hook_log(line: &str) {
    crash::tail(&data::folder(), &format!("{} [tauri] hook: {line}", now_ms()));
}

// A hook finding her not up: she is started as its session starts
// (starts.rs starts_her), unless she was quit by choice, which her log says.
fn start_or_say(hook: starts::Hook, event: &str, who: impl FnOnce() -> String) {
    let dir = data::folder();
    let mut note = None;
    let start = starts::starts_her(hook, event, || {
        note = (!data::may_start(&dir)).then(|| data::quit_note(&dir).unwrap_or_default());
        note.is_some()
    });
    if start {
        start_detached(&who());
    } else if let Some(note) = note {
        hook_log(&format!("{}: her not up, and not started: {note}", who()));
    }
}

// The names above a hook, for its words (starts.rs hook_who).
fn hook_parents() -> Vec<String> {
    jump::parent_names(3)
}

// If no pet answers on the port, start one (unless the person quit her:
// data.rs Quit); if one does, hand her the event. Then leave, quick and
// quiet, whatever happens. `after_flag`: omp's extension says it is omp's,
// and how its session started.
fn ensure_running(port: u16, after_flag: &[String]) {
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
        let claude = std::env::var("CLAUDE_CODE_ENTRYPOINT").ok().or_else(|| (std::env::var("CLAUDECODE").as_deref() == Ok("1")).then(String::new));
        start_or_say(starts::Hook::Ensure, "", || starts::ensure_who(after_flag, event.as_ref(), claude.as_deref(), &hook_parents()));
    }
}

// --- Run by Codex's hooks -----------------------------------------------------------------

// Codex runs its hooks as commands: this program with CODEX_FLAG, for every
// event. Hand her the event, and print her answer to a prompt, the decision
// Codex reads; nothing else, since Codex takes what a hook prints as words
// for the model. SessionStart starts her when she is not up (unless the
// person quit her: data.rs Quit). Whatever
// happens, leave with 0, so Codex never shows a failed hook.
// An event handed to her port with no answer waited for (none is needed):
// the request written whole and the connection closed at once. False when
// she is not up: no one listens there (asked of Windows' table in a moment:
// a connection tried would take it 2 s to give up on, and the agent waits
// on its hook meanwhile). Up but not yet answering (her first second or two:
// her port is held at once, answered once she is set up), it waits in her
// port's queue till she is, and nothing is lost.
fn post_and_go(port: u16, agent: &str, event: &Value) -> bool {
    if !jump::is_listening(port) {
        return false;
    }
    let to = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let Ok(mut stream) = std::net::TcpStream::connect_timeout(&to, Duration::from_millis(100)) else { return false };
    let body = event.to_string();
    let head = format!(
        "POST /hook?from=wakuwaku&agent={agent} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let sent = stream.write_all(head.as_bytes()).and_then(|()| stream.write_all(body.as_bytes())).is_ok();
    let _ = stream.shutdown(std::net::Shutdown::Write);
    sent
}

// Codex's hook (connection::CODEX_FLAG): the event handed to her port, with
// where the Codex session runs (the processes above this one, for going to
// its window from the pet, jump.rs). A prompt waits for her answer, which is
// printed for Codex; any other event is handed on and let go (post_and_go).
// Its session starting with her not up starts her, unless the person quit
// her (data.rs Quit).
fn codex_hook(port: u16) {
    let Some(mut event) = read_event() else { return };
    if let Some(fields) = event.as_object_mut() {
        fields.insert(jump::CODEX_CHAIN.into(), jump::chain_json(&jump::hook_chain(Some("codex.exe"))));
    }
    let name = event["hook_event_name"].as_str().unwrap_or("").to_string();
    if name != "PermissionRequest" {
        if !post_and_go(port, "codex", &event) {
            let source = event["source"].as_str().unwrap_or("").to_string();
            start_or_say(starts::Hook::Codex, &name, || starts::hook_who("Codex", &name, &[&source], &hook_parents()));
        }
        return;
    }
    // A prompt waits for the person, just under the hook's own timeout.
    let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(connection::CODEX_PROMPT_TIMEOUT - 5))).build().new_agent();
    if let Ok(mut res) = agent.post(&format!("http://127.0.0.1:{port}/hook?from=wakuwaku&agent=codex")).send_json(&event) {
        let body = res.body_mut().read_to_string().unwrap_or_default();
        if serde_json::from_str::<Value>(&body).is_ok_and(|v| v.get("hookSpecificOutput").is_some()) {
            let mut out = std::io::stdout();
            let _ = out.write_all(body.as_bytes());
            let _ = out.flush();
        }
    }
}

// Antigravity's hook (connection::AGY_FLAG and the event's name, which its
// payload does not carry): its answer printed first, whatever comes after
// (a hook that fails or answers nothing breaks agy's turn: connection/agy.rs),
// then the event handed to her port with its name and let go (post_and_go:
// agy waits on each hook, and her answer is nothing to it). Quit by the
// person, she is not up: nothing tried. Its session starting with her not
// up starts her, as Codex's does. Where the session runs only as it starts
// or a turn begins (kept till then; some 10 ms: most of a hook is its
// process starting, some 50 to 80 ms).
fn agy_hook(port: u16, name: &str) {
    let answer = if name == "Stop" { r#"{"decision":"allow"}"# } else { "{}" };
    let mut out = std::io::stdout();
    let _ = out.write_all(answer.as_bytes());
    let _ = out.flush();
    let Some(mut event) = read_event() else { return };
    let who = || starts::hook_who("Antigravity", name, &[], &hook_parents());
    if !data::may_start(&data::folder()) {
        // Said in her log as its session starts (start_or_say).
        start_or_say(starts::Hook::Agy, name, who);
        return;
    }
    let turn = name == "SessionStart" || (name == "PreInvocation" && event.get("invocationNum").and_then(Value::as_u64) == Some(0));
    if let Some(fields) = event.as_object_mut() {
        fields.insert("hook_event_name".into(), json!(name));
        if turn {
            fields.insert(jump::CODEX_CHAIN.into(), jump::chain_json(&jump::hook_chain(None)));
        }
    }
    if !post_and_go(port, "agy", &event) {
        start_or_say(starts::Hook::Agy, name, who);
    }
}

// ZCode's hook (connection::ZCODE_FLAG and the event's name): the event
// handed to her port and let go (post_and_go: ZCode waits on each hook),
// with where the session runs as it starts or a turn begins (jump.rs); its
// session starting with her not up starts her, unless the person quit her,
// as Codex's does. A prompt waits for her answer, printed for ZCode in Claude
// Code's words (asks.rs); ZCode shows its own dialog meanwhile, takes the
// first answer, and ends this hook should the person answer there. Nothing
// printed otherwise: ZCode takes JSON a hook prints as its output, and fails
// the hook on one that does not fit. Her not up, nothing to wait on.
fn zcode_hook(port: u16, name: &str) {
    let Some(mut event) = read_event() else { return };
    let name = event["hook_event_name"].as_str().unwrap_or(name).to_string();
    if let Some(fields) = event.as_object_mut() {
        fields.insert("hook_event_name".into(), json!(name));
        if matches!(name.as_str(), "SessionStart" | "UserPromptSubmit") {
            fields.insert(jump::CODEX_CHAIN.into(), jump::chain_json(&jump::hook_chain(None)));
        }
    }
    if name != "PermissionRequest" {
        if !post_and_go(port, "zcode", &event) {
            let source = event["source"].as_str().unwrap_or("").to_string();
            start_or_say(starts::Hook::Zcode, &name, || starts::hook_who("ZCode", &name, &[&source], &hook_parents()));
        }
        return;
    }
    if !jump::is_listening(port) {
        return;
    }
    // Just under the hook's own timeout.
    let wait = Duration::from_millis(connection::ZCODE_PROMPT_TIMEOUT_MS - 5_000);
    let agent = ureq::Agent::config_builder().timeout_global(Some(wait)).build().new_agent();
    if let Ok(mut res) = agent.post(&format!("http://127.0.0.1:{port}/hook?from=wakuwaku&agent=zcode")).send_json(&event) {
        let body = res.body_mut().read_to_string().unwrap_or_default();
        if serde_json::from_str::<Value>(&body).is_ok_and(|v| v.get("hookSpecificOutput").is_some()) {
            let mut out = std::io::stdout();
            let _ = out.write_all(body.as_bytes());
            let _ = out.flush();
        }
    }
}

// DeepSeek Harness's hook (connection::DSH_FLAG and the event's name), as
// her old rows had it run (until 0.3.2; her own plugin in DSH replaces them,
// connection/dsh.rs, and they are no longer written: this stays for a DSH still
// running them), by its Claude Code bridge through its PowerShell, in its sandbox: the event
// handed to her port and let go. As a session starts or a turn begins it
// waits for her answer instead (some ms), this connection held while she
// looks up where the session runs from this process (server.rs caller_of:
// in its sandbox it may not open the processes above itself). It never
// starts her: she would run in DSH's sandbox (a low-integrity,
// write-restricted token) and could keep nothing. Nothing printed: the
// bridge would read it as a decision.
fn dsh_hook(port: u16, name: &str) {
    let Some(mut event) = read_event() else { return };
    let name = event["hook_event_name"].as_str().unwrap_or(name).to_string();
    if let Some(fields) = event.as_object_mut() {
        fields.insert("hook_event_name".into(), json!(name));
    }
    if !matches!(name.as_str(), "SessionStart" | "UserPromptSubmit") {
        post_and_go(port, "dsh", &event);
        return;
    }
    if jump::is_listening(port) {
        let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(3))).build().new_agent();
        let _ = agent.post(&format!("http://127.0.0.1:{port}/hook?from=wakuwaku&agent=dsh")).send_json(&event);
    }
}

// Reasonix's hook (connection::REASONIX_FLAG and the event's name), run by
// Reasonix through cmd.exe for each event of its sessions, its CLI's and
// its Studio's alike: the event handed to her port and let go (post_and_go:
// Reasonix waits on every hook, and one of its gating events that times out
// is blocked), with where the session runs as it starts or a turn begins
// (jump.rs, past cmd); its session starting with her not up starts her,
// unless the person quit her, as Codex's does. Nothing printed, whatever
// the event: Reasonix shows what a hook prints, and its PermissionRequest
// stays its own (zhoupengjie/Wakuwaku#60, phase 1: server.rs answers_none).
// Her not up, nothing tried but that; and always gone with 0.
fn reasonix_hook(port: u16, name: &str) {
    let Some(mut event) = read_event() else { return };
    let name = event["event"].as_str().filter(|n| !n.is_empty()).unwrap_or(name).to_string();
    if let Some(fields) = event.as_object_mut() {
        fields.insert("event".into(), json!(name));
        // The name her server reads every agent's events by.
        fields.insert("hook_event_name".into(), json!(name));
        if matches!(name.as_str(), "SessionStart" | "UserPromptSubmit") {
            fields.insert(jump::CODEX_CHAIN.into(), jump::chain_json(&jump::hook_chain(None)));
        }
    }
    if !post_and_go(port, "reasonix", &event) {
        let source = event["source"].as_str().unwrap_or("").to_string();
        start_or_say(starts::Hook::Reasonix, &name, || starts::hook_who("Reasonix", &name, &[&source], &hook_parents()));
    }
}

fn main() {
    // Her exe again, as the taskbar's guard (shell.rs): it waits for her to
    // end, and gives Windows' taskbar back if she could not; and sees
    // whether she crashed (crash.rs; her data folder is the file's).
    let args: Vec<String> = std::env::args().collect();
    if let Some((pid, file)) = shell::guard_args(&args) {
        let data = file.parent().map(PathBuf::from).unwrap_or_default();
        let after = data.clone();
        shell::guard(pid, file, || drop(icons::hold(&data)), move |code, started| crash::after_exit(&after, pid, code, started));
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
        return ensure_running(port, &args[at + 1..]);
    }
    if std::env::args().any(|a| a == connection::CODEX_FLAG) {
        return codex_hook(port);
    }
    if let Some(at) = args.iter().position(|a| a == connection::AGY_FLAG) {
        return agy_hook(port, args.get(at + 1).map_or("", String::as_str));
    }
    if let Some(at) = args.iter().position(|a| a == connection::ZCODE_FLAG) {
        return zcode_hook(port, args.get(at + 1).map_or("", String::as_str));
    }
    if let Some(at) = args.iter().position(|a| a == connection::DSH_FLAG) {
        return dsh_hook(port, args.get(at + 1).map_or("", String::as_str));
    }
    if let Some(at) = args.iter().position(|a| a == connection::REASONIX_FLAG) {
        return reasonix_hook(port, args.get(at + 1).map_or("", String::as_str));
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
            pet_shortcuts_pause,
            pet_walk,
            pet_walk_stop,
            island_holding,
            island_editing,
            island_seat,
            island_stow,
            island_raise,
            island_plate,
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
            settings::settings_win_look,
            settings::settings_wallpaper,
            settings::settings_agent,
            settings::settings_cpu_sensors,
            bars::bar_ready,
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
            taskbar_handoff,
            taskbar::taskbar_tray_rects,
            taskbar::taskbar_window,
            taskbar::taskbar_windows,
            taskbar::taskbar_held,
            taskbar::taskbar_keys_pause,
            taskbar::taskbar_window_menu,
            taskbar::taskbar_watch_presses,
            taskbar::taskbar_tray_pin,
            taskbar::taskbar_app,
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
                hub,
                widgets: Mutex::new(widgets::Widgets::default()),
                plugins: Mutex::new(plugins::Runner::default()),
                mail: Mutex::new(mail::Runner::default()),
                today: Mutex::new(today),
                screens: Mutex::new(screen::read(app.handle())),
                hidden: AtomicBool::new(false),
                by_fullscreen: AtomicBool::new(false),
                quiet: AtomicBool::new(false),
                greeted: AtomicBool::new(false),
                seen: Default::default(),
                own: Mutex::new(Vec::new()),
                prev_focus: Mutex::new(0),
                push_pending: AtomicBool::new(false),
                shuffle_until: std::sync::atomic::AtomicU64::new(0),
                evals: Mutex::new((0, std::collections::HashMap::new())),
            });
            app.manage(sh.clone());
            // Her requests to the web the way the General page says (proxy.rs).
            proxy::set_web(&sh.setting("netProxy"));
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
            if !sh.flag("taskbarPinsImported") {
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
                sh.by_fullscreen.store(true, Ordering::SeqCst);
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

            // The mouse: click-through, her eyes, drags; for both windows.
            // Quiet (nobody touching it), looked at once a second. Woken
            // before its time by wake_pointer (desk.rs: Ctrl pressed or let go).
            let pointer = sh.clone();
            spawn_named("pointer", move || loop {
                let _ = POINTER_LOOP.set(std::thread::current());
                // Each window's pointer in its own display's scale on a Mac
                // (screen::cursor); elsewhere the one pointer, read once.
                #[cfg(target_os = "macos")]
                let at = (screen::cursor(&pointer, pet::sf(&pointer)), screen::cursor(&pointer, island::origin(&pointer).1));
                #[cfg(not(target_os = "macos"))]
                let at = {
                    let c = screen::cursor(&pointer, 1.0);
                    (c, c)
                };
                // Quiet or not first: woken by a key after a quiet while (Ctrl,
                // desk.rs), the looks are someone's at once, not a turn later.
                let quiet = pointer.check_quiet();
                let wait = match at {
                    (Some(her), Some(home)) => pet::poll(&pointer, her).min(island::poll(&pointer, home)).min(desk::poll(&pointer, home)),
                    _ => 200,
                };
                let wait = if quiet { wait.max(1000) } else { wait };
                std::thread::park_timeout(Duration::from_millis(wait));
            });

            // The widgets: today's from the start; the tokens' each minute
            // while they are on; a script's gone once it stops sending.
            sh.count_today(|_| {});
            let watcher = sh.clone();
            spawn_named("widgets", move || {
                power::thread_eco();
                let mut tokens = tokens::Tokens::default();
                let (claude_dir, codex_dir, agy_dirs, dsh_dir) = (tokens::claude_dir(), tokens::codex_dir(), tokens::agy_dirs(), tokens::dsh_dir());
                let mut tokens_at: Option<std::time::Instant> = None;
                loop {
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
                    let unread = |id: &str| watcher.is_widget_wanted(id) && watcher.widgets.lock().unwrap().label_of(id).is_none();
                    if tokens_at.is_none_or(|t| t.elapsed() >= tokens::EVERY) || unread("tokens") {
                        tokens_at = Some(std::time::Instant::now());
                        let count = watcher.is_widget_wanted("tokens").then(|| {
                            tokens.update(&claude_dir, &codex_dir, &agy_dirs, &dsh_dir, widgets::local_midnight());
                            tokens::words(tokens.count())
                        });
                        built_in("tokens", count);
                    }
                    // A new day: today's counts start again.
                    if watcher.today.lock().unwrap().date != widgets::local_date() {
                        watcher.count_today(|_| {});
                    }
                    // The plugins she runs: started, stopped, started again
                    // (her language changed, too: the weather's words).
                    changed |= plugins::sync(&watcher);
                    if changed {
                        watcher.redraw_widgets();
                    }
                    // Quiet (nobody at the keyboard): every ten seconds.
                    std::thread::sleep(Duration::from_secs(if watcher.is_quiet() { 10 } else { 3 }));
                }
            });

            // The monitor: the parts that are on, every so many seconds (its
            // setting, 1 to 10), while it is on; read again at once when it
            // is turned on, its pace or source changes, the quiet does, or
            // the settings open (their list of sensors, read whole while they
            // are) (woken by Shared::change, check_quiet and open_settings:
            // it looked four times a second whether they had, #14).
            let monitored = sh.clone();
            spawn_named("monitor", move || {
                power::thread_eco();
                let mut monitor = widgets::Monitor::new();
                let mut next = std::time::Instant::now();
                // Read with the settings open since they last opened.
                let mut watching_listed = false;
                let mut was = (false, Value::Null, false, false);
                loop {
                    let setting = json!([monitored.setting("monitor"), monitored.setting("monitorSource")]);
                    let mut watching = widgets::Watching::from(&setting[0], &setting[1]);
                    // On in the island, or put on the taskbar (its widget there).
                    let (on, quiet) = (monitored.is_widget_wanted("monitor"), monitored.is_quiet());
                    let listing = monitored.island.lock().unwrap().settings_open;
                    // The pointer on her home: its widget's card may be looked at;
                    // on the desktop medium or large, its card's readings show.
                    let looking = !quiet && (island::is_over(&monitored) || desk::shows_more(&monitored, "monitor"));
                    (watching.quiet, watching.listing, watching.looking) = (quiet, listing, looking);
                    // Any of its settings changed (a sensor picked, too), the
                    // settings opened, the pointer came, or LibreHardwareMonitor
                    // just started answering (lhm::ask_now): read now.
                    if lhm::take_read_now() | (std::time::Instant::now() >= next) || (on, &setting, quiet) != (was.0, &was.1, was.2) || (looking && !was.3) || (listing && !watching_listed) {
                        was = (on, setting, quiet, looking);
                        watching_listed = listing;
                        // Quiet (nobody looking), every ten seconds at least.
                        let every = if quiet { watching.every.max(10) } else { watching.every };
                        next = std::time::Instant::now() + Duration::from_secs(every);
                        // On the desktop alone (not in the closed island, nor on
                        // the taskbar), no display shows it (a window maximized or
                        // full screen over it) and the open island is not being
                        // looked at (its overview lists it): not read, kept as
                        // it was, looked at again at its pace (desk.rs seen).
                        let unseen = on && !monitored.is_in_island("monitor") && !(monitored.home() == "taskbar" && settings::is_on_taskbar(&monitored.setting("taskbarLayout"), "monitor")) && !desk::seen(&monitored, "monitor") && !island::is_over(&monitored);
                        let words = if unseen {
                            None
                        } else if on {
                            monitor.read(&watching)
                        } else {
                            monitor.rest();
                            None
                        };
                        let changed = {
                            let mut widgets = monitored.widgets.lock().unwrap();
                            match words {
                                Some(value) => widgets.set_built_in_with("monitor", value, monitor.detail(), now_ms()),
                                None if unseen => false,
                                None => widgets.remove_built_in("monitor"),
                            }
                        };
                        if changed {
                            monitored.redraw_widgets();
                        }
                        // LibreHardwareMonitor found or lost, its sensors' readings,
                        // or the built-in ones': the open settings list them.
                        if lhm::take_changed() | hw::take_changed() {
                            monitored.push_settings();
                        }
                    }
                    watching_listed &= listing;
                    // Off, till it is turned on.
                    let wait = if on { next.saturating_duration_since(std::time::Instant::now()) } else { Duration::from_secs(3600) };
                    let (woken, cv) = &MONITOR_WAKE;
                    let guard = woken.lock().unwrap();
                    let (mut guard, _) = cv.wait_timeout_while(guard, wait, |w| !*w).unwrap();
                    *guard = false;
                }
            });

            // Another app full screen, looked at on this thread alone
            // (Shared::look_fullscreen); with it, a window maximized on her
            // island's display, and one under it.
            let watcher = sh.clone();
            spawn_named("fullscreen", move || loop {
                let asked = fullscreen::wait(Duration::from_millis(1500));
                watcher.look_fullscreen(asked);
                island::look_over_max(&watcher);
                island::look_under(&watcher);
            });

            // Endings held for a while; prompts out of time; sessions gone
            // quiet, out of sight and forgotten; the windows on screen.
            let ticker = sh.clone();
            spawn_named("ticker", move || {
                for n in 1u64.. {
                    std::thread::sleep(Duration::from_millis(500));
                    let now = now_ms();
                    if n % 2 == 0 {
                        ticker.check_quiet();
                        let hide = ticker.idle_hide_ms();
                        let mut changed = ticker.pet.lock().unwrap().tick(now);
                        if n % 120 == 0 {
                            changed |= ticker.pet.lock().unwrap().check_stale(now);
                            // The hub's sessions the same, the apps listening told.
                            ticker.hub.tick(now);
                        }
                        changed |= ticker.pet.lock().unwrap().hide_quiet(now, hide);
                        if changed {
                            ticker.redraw();
                        }
                        // Each one handed over on its own line, as an answer
                        // is: a prompt let wait its whole time said nothing
                        // (the test session, 2026-10-13).
                        let gone = ticker.asks.lock().unwrap().expire(now);
                        if !gone.is_empty() {
                            for id in gone {
                                ticker.log(&format!("prompt {id} handed to the terminal: its time is up"));
                            }
                            ticker.push_asks();
                        }
                    }
                    // The desktop's widgets' counts, each minute (desk.rs minute).
                    if n % 120 == 0 {
                        desk::minute(&ticker);
                    }
                    if n % 4 == 0 {
                        let screens = screen::read(&ticker.app);
                        // Displays come or go, or change: the desktop's widgets' windows
                        // with them (only while there are any).
                        let moved = desk::any() && ticker.screens.lock().unwrap().areas != screens.areas;
                        *ticker.screens.lock().unwrap() = screens;
                        if moved {
                            desk::sync(&ticker);
                        }
                        pet::keep_on_screen(&ticker);
                        island::keep_on_screen(&ticker);
                    }
                }
            });

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
                    for line in connection::dsh_at_start(dsh.port, &dsh.setting("dshVia")) {
                        dsh.log(&line);
                    }
                });
            }
            // Never set up: the settings, where Claude Code or Codex gets
            // connected, once (welcomeOpened). Every start until one was
            // connected was too much for one who keeps her as a pet only,
            // with no way to stop it (the user, 2026-10-13); nor once "not
            // now" was said (connectHintOff, the welcome's or the island's).
            if !connection::look_connected(port) && !sh.flag("onboarded") && !sh.flag("welcomeOpened") && !sh.flag("connectHintOff") {
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
                island::release_bar(&sh);
                mail::stop_all(&sh);
                sh.log("quitting: done");
            }
        });
}

// This process ended at once, whatever its threads are doing.
// Quitting once: a second quit (the tray, an update) while the first hands over does nothing.
static QUITTING: AtomicBool = AtomicBool::new(false);

fn end_now() -> ! {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetCurrentProcess() -> isize;
            fn TerminateProcess(process: isize, code: u32) -> i32;
        }
        // SAFETY: this process's own pseudo-handle.
        unsafe { TerminateProcess(GetCurrentProcess(), 0) };
    }
    std::process::exit(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Handed on and let go while her port is held but not yet answered (her
    // first second or two): the event waits in its queue, whole, and is read
    // once she answers; then as she answers at once.
    #[test]
    fn an_event_let_go_is_read_whole_later() {
        let server = tiny_http::Server::http(("127.0.0.1", 0)).unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let event = json!({ "hook_event_name": "Stop", "conversationId": "c", "text": "x".repeat(20_000) });
        let t = std::time::Instant::now();
        assert!(post_and_go(port, "agy", &event));
        assert!(t.elapsed() < Duration::from_millis(200), "let go at once");
        std::thread::sleep(Duration::from_millis(300));
        let mut req = server.recv_timeout(Duration::from_secs(2)).unwrap().expect("a request");
        assert_eq!(req.url(), "/hook?from=wakuwaku&agent=agy");
        let mut body = String::new();
        std::io::Read::read_to_string(req.as_reader(), &mut body).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&body).unwrap(), event);
        // Her answer goes nowhere, harmlessly.
        let _ = req.respond(tiny_http::Response::from_string("{}"));
        // No one there: nothing tried.
        drop(server);
        assert!(!post_and_go(port, "agy", &event));
    }
}
