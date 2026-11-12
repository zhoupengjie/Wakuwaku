// What every part of her shares: one Shared for the whole run, made as she
// starts (main.rs setup) and kept in Tauri's state (shared()). The
// settings, her sessions' moods, her windows' states, the prompts waiting,
// the widgets, the plugins and the mail, each behind a lock of its own;
// what needs no lock (out of sight, full screen, quiet); the log; and what
// the pages are told (redraw, push_asks, push_settings). With it what
// every part uses alongside: the time (now_ms), a thread with a name, where
// she is (set_display, set_out, absorb, come_home), and the quit.
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Map, Value};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

#[cfg(windows)]
use crate::frameless;
use crate::lock::Lock;
use crate::{asks, backplate, bars, connection, crash, data, desk, focus, fullscreen, i18n, island, logon, mail, notify, pet, plugins, power, screen, settings, shortcuts, state, taskbar, tray, wallpaper, widgets};
use crate::config;

// The island's own animation of taking her in, before she counts as in.
const ABSORB_MS: u64 = 380;

// Her port, unless WAKUWAKU_PORT says another (a test's copy, mostly): the
// agents' hub's (agent-hub PORT), where its other apps find it.
pub const DEFAULT_PORT: u16 = agent_hub::PORT;

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

// Answers a held hook request: Some(output), or None to drop it.
pub type Responder = Box<dyn FnOnce(Option<Value>) + Send>;

// What every part shares. No two of its locks are ever held at once: each
// a Lock (lock.rs), which says so should a thread take one holding another.
pub struct Shared {
    pub app: AppHandle,
    pub dir: PathBuf,
    pub port: u16,
    pub settings: Lock<Map<String, Value>>,
    pub pet: Lock<state::Pet>,
    pub win: Lock<pet::Win>,
    pub island: Lock<island::Island>,
    pub asks: Lock<asks::Asks<Responder>>,
    // The agents' sessions and events, for other apps on this machine
    // (agent-hub, server.rs /v1).
    pub hub: agent_hub::Hub,
    // Plugins in the island (widgets.rs), and what today has seen.
    pub widgets: Lock<widgets::Widgets>,
    pub today: Lock<widgets::Today>,
    // The plugins she runs for you (plugins.rs).
    pub plugins: Lock<plugins::Runner>,
    // The mail accounts being watched (mail.rs).
    pub mail: Lock<mail::Runner>,
    screens: Lock<screen::Screens>,
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
    own: Lock<Vec<isize>>,
    // The window that had the keyboard before one of hers took it.
    prev_focus: Lock<isize>,
    push_pending: AtomicBool,
    // Until when (ms) a loss of focus is our own doing: showing a window of
    // hers makes it the active one, and that is no click elsewhere.
    pub shuffle_until: std::sync::atomic::AtomicU64,
    // Debug only: answers to /debug/eval, by id.
    pub evals: Lock<(u64, std::collections::HashMap<u64, std::sync::mpsc::Sender<Value>>)>,
}

impl Shared {
    // Made once, as she starts (main.rs setup), and kept in Tauri's state.
    // Two of its locks held at once are said in her log (lock.rs: a debug
    // build's only).
    pub fn new(app: AppHandle, dir: PathBuf, port: u16, settings: Map<String, Value>, hub: agent_hub::Hub, today: widgets::Today) -> Arc<Self> {
        let is_logging = dir.join("debug.on").exists();
        let screens = Lock::new("screens", screen::read(&app));
        let sh = Arc::new(Shared {
            app,
            is_logging,
            dir,
            port,
            settings: Lock::new("settings", settings),
            pet: Lock::new("pet", state::Pet::default()),
            win: Lock::new("win", pet::Win::default()),
            island: Lock::new("island", island::Island::default()),
            asks: Lock::new("asks", asks::Asks::default()),
            hub,
            widgets: Lock::new("widgets", widgets::Widgets::default()),
            plugins: Lock::new("plugins", plugins::Runner::default()),
            mail: Lock::new("mail", mail::Runner::default()),
            today: Lock::new("today", today),
            screens,
            hidden: AtomicBool::new(false),
            by_fullscreen: AtomicBool::new(false),
            quiet: AtomicBool::new(false),
            greeted: AtomicBool::new(false),
            seen: Default::default(),
            own: Lock::new("own", Vec::new()),
            prev_focus: Lock::new("prev_focus", 0),
            push_pending: AtomicBool::new(false),
            shuffle_until: std::sync::atomic::AtomicU64::new(0),
            evals: Lock::new("evals", (0, std::collections::HashMap::new())),
        });
        let said = Arc::downgrade(&sh);
        crate::lock::say_with(move |line| {
            if let Some(sh) = said.upgrade() {
                sh.log(line);
            }
        });
        sh
    }

    // A setting as it is (config.rs: each key declared once, a constant
    // there, never a string here): Null while unset.
    pub fn setting(&self, key: impl config::Key) -> Value {
        self.settings.lock().unwrap().get(key.name()).cloned().unwrap_or(Value::Null)
    }

    // A switch: on only when it is true.
    pub fn flag(&self, key: config::Flag) -> bool {
        config::is_on(&self.setting(key))
    }

    pub fn lang(&self) -> &'static str {
        i18n::lang(self.setting(config::LANG).as_str().unwrap_or("auto"))
    }

    pub fn hold(&self) -> state::Hold {
        match self.setting(config::HOLD).as_u64() {
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
        test.unwrap_or_else(|| self.setting(config::IDLE_HIDE_MIN).as_u64().unwrap_or(state::IDLE_HIDE_MIN) * 60 * 1000)
    }

    pub fn screens(&self) -> screen::Screens {
        self.screens.lock().unwrap().clone()
    }

    // The displays read again (loops.rs ticker): whether their work areas
    // changed.
    pub fn put_screens(&self, screens: screen::Screens) -> bool {
        let mut now = self.screens.lock().unwrap();
        let moved = now.areas != screens.areas;
        *now = screens;
        moved
    }

    // Whether the settings are open in her home now.
    pub fn settings_open(&self) -> bool {
        self.island.lock().unwrap().settings_open
    }

    // Our windows' handles (own_window): never another app's.
    pub fn own_windows(&self) -> Vec<isize> {
        self.own.lock().unwrap().clone()
    }

    pub fn is_own(&self, hwnd: isize) -> bool {
        self.own.lock().unwrap().contains(&hwnd)
    }

    // Writing debug.log (debug.on in her data folder as she started).
    pub fn is_logging(&self) -> bool {
        self.is_logging
    }

    // Where a prompt can be answered: her up, or, put away only by an app
    // full screen on her display, her taskbar still up on another (bars.rs),
    // whose capsule shows it waiting and brings up her island for it. Else
    // nobody can answer here, and the terminal has it at once (server.rs,
    // apply_visibility); it had them even while her capsule on another
    // display showed (the test session, 2026-10-13).
    pub fn asks_seen(&self) -> bool {
        self.is_visible() || (!self.hidden.load(Ordering::SeqCst) && !self.flag(config::DND) && self.is_fullscreen() && bars::any_shown())
    }

    pub fn is_visible(&self) -> bool {
        !self.hidden.load(Ordering::SeqCst)
            && !self.flag(config::DND)
            && !(self.flag(config::HIDE_IN_FULLSCREEN) && self.by_fullscreen.load(Ordering::SeqCst))
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
            crate::loops::wake_monitor();
            // The plugins she runs at their quiet pace, or back to their own.
            plugins::wake();
            if !quiet {
                taskbar::wake();
            }
        }
        self.keep_brisk(quiet);
        quiet
    }

    // Her processes and WebView2's for her pages before others' on the CPU
    // and the GPU while someone is at it and no app is full screen; as any
    // other's while nobody is about (and easy on the power, above) or a game
    // has the screen (power.rs brisk; the user, 2026-11-08: GPU或者CPU高负载的
    // 情况下，Wakuwaku的整个界面很容易卡死). Logged as it changes; done again
    // every half minute meanwhile, WebView2 making processes as her windows
    // come (a list's, the dock's) and Chromium setting a hidden page's lower.
    // Off this thread (the pointer's): it looks through every process.
    fn keep_brisk(&self, quiet: bool) {
        static AT: Mutex<(Option<bool>, Option<Instant>)> = Mutex::new((None, None));
        let want = !quiet && !self.is_fullscreen();
        let changed = {
            let mut at = AT.lock().unwrap();
            let changed = at.0 != Some(want);
            let due = want && at.1.is_none_or(|t| t.elapsed() >= Duration::from_secs(30));
            if !changed && !due {
                return;
            }
            *at = (Some(want), Some(Instant::now()));
            changed
        };
        let sh = shared(&self.app);
        std::thread::spawn(move || {
            let said = power::brisk(want);
            if changed {
                sh.log(&format!("priority: {} ({said})", if want { "before others'" } else { "as any other's" }));
            }
        });
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

    // An app full screen already as she starts (main.rs setup), before her
    // home first shows.
    pub fn starts_full_screen(&self) {
        self.by_fullscreen.store(true, Ordering::SeqCst);
    }

    // Her home: 'island' or 'taskbar' (island.rs).
    // Her home as her windows have it: the island, or the taskbar, which the
    // top bar and the dock are too (display split: island.rs is_split).
    pub fn home(&self) -> &'static str {
        match self.setting(config::DISPLAY).as_str() {
            Some("taskbar" | "split") => "taskbar",
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

    // A window of hers, just made: known as hers, and with no frame to draw
    // (frameless.rs), on the main thread that has it, ahead of anything that
    // would show it (her page, not ready for a good while yet).
    pub fn own_window(&self, _win: &WebviewWindow) {
        #[cfg(windows)]
        if let Ok(hwnd) = _win.hwnd() {
            let hwnd = hwnd.0 as isize;
            self.own.lock().unwrap().push(hwnd);
            let _ = _win.run_on_main_thread(move || frameless::keep(hwnd));
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
        // dot alone (island-ring.js cue; pet:full-screen as it changes).
        now["fullScreen"] = json!(self.is_fullscreen());
        // Her window up on the desktop: she says it herself there, a prompt
        // over her head, and her home does not open for it (panel.js holderOf).
        now["herUp"] = json!(pet::is_shown(self));
        // Whether a blur of what is behind the island can be put under it
        // (backplate.rs): Windows 11, transparency effects on.
        now["plate"] = json!(backplate::available());
        // Windows' mode, for her island's look at the top of the screen (the
        // taskbar hears it from taskbar/, taskbar:look).
        if self.home() != "taskbar" {
            now["winLight"] = json!(wallpaper::look().0);
            // The side of the top of the screen her island's window is at
            // (islandPos, zhoupengjie/Wakuwaku#59): the page lays it out there.
            now["islandAt"] = json!(island::side(self).name());
            // The edge of the screen it is along (islandEdge,
            // zhoupengjie/Wakuwaku#105): the top, or the bottom, over
            // Windows' own taskbar shown there, whose thickness (physical
            // px; 0 with none) the page centres the closed island in.
            now["islandEdge"] = json!(island::edge_name(self));
            now["islandStrip"] = json!(island::strip_under(self));
            // The island inside Windows' taskbar (embed.rs): the page lays
            // the island about islandMx (physical px from the window's left,
            // where its capsule lies in the strip), and keeps the capsule's
            // width within the room it has there (islandRoom, physical px; 0
            // none; anchor.js capsuleWidth).
            let (embedded, mx, room) = island::embedding(self);
            now["islandEmbed"] = json!(embedded);
            now["islandMx"] = json!(mx);
            now["islandRoom"] = json!(room);
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

    pub(crate) fn to_pages(&self, event: &str, payload: Value) {
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

    // What the prompt panel shows: the agents' prompts, then the codes and
    // links come by mail (mail/codes.rs), which never stand before a prompt.
    pub fn asks_views(&self) -> Value {
        let mut views = self.asks.lock().unwrap().views();
        if let Value::Array(list) = &mut views {
            list.extend(mail::codes::views());
        }
        views
    }

    // The prompts waiting changed.
    pub fn push_asks(&self) {
        let (none, handed) = {
            let mut asks = self.asks.lock().unwrap();
            (asks.is_empty() && mail::codes::is_empty(), asks.take_handed())
        };
        let views = self.asks_views();
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
        if !self.settings_open() || self.push_pending.swap(true, Ordering::SeqCst) {
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
        // How long an ending stays, read before the sessions' lock: as an
        // argument it was read with that lock held (the settings' inside it).
        let hold = self.hold();
        let out = self.pet.lock().unwrap().apply(ev, chain, now_ms(), hold);
        let Some(out) = out else { return false };
        if out.turns > 0 || out.worked_ms > 0 {
            // Reasonix's approvals never come to her, nor
            // DeepSeek Harness's through its hook bridge (through her plugin
            // they may: those given on her are counted all the same).
            // Antigravity's approvals come to her since 2026-10-26 (events_agy).
            let without_approvals = matches!(ev.agent, Agent::Dsh | Agent::Reasonix);
            self.count_today(|t| {
                t.begun(out.turns);
                if without_approvals {
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
        if !self.flag(config::DND) {
            for (mood, project) in out.alerts {
                self.alert(&mood, &project);
            }
        }
        true
    }

    // --- Widgets ----------------------------------------------------------------------

    fn list_setting(&self, key: config::Setting) -> Vec<String> {
        self.setting(key).as_array().into_iter().flatten().filter_map(|v| v.as_str().map(str::to_string)).collect()
    }

    pub fn is_widget_on(&self, id: &str) -> bool {
        !self.list_setting(config::WIDGETS_OFF).iter().any(|o| o == id)
    }

    // In the closed island: on, and not put on the desktop, nor on the
    // taskbar while that is her home (a widget in one place, settings.rs
    // is_placed_out).
    pub fn is_in_island(&self, id: &str) -> bool {
        self.is_widget_on(id) && !settings::is_placed_out(&self.setting(config::DESK_LAYOUT), &self.setting(config::TASKBAR_LAYOUT), self.home() == "taskbar", id)
    }

    // On in the island, put on the taskbar (taskbarLayout) while she is the
    // taskbar, or on the desktop (deskLayout): a built-in one is worked out then.
    pub fn is_widget_wanted(&self, id: &str) -> bool {
        self.is_widget_on(id) || (self.home() == "taskbar" && settings::is_on_taskbar(&self.setting(config::TASKBAR_LAYOUT), id)) || (cfg!(windows) && settings::is_on_desk(&self.setting(config::DESK_LAYOUT), id))
    }

    // The widgets with something to say, each saying whether it is on.
    pub(crate) fn widgets_said(&self) -> Value {
        Value::Array(self.widgets_view().as_array().into_iter().flatten().filter(|w| !w["value"].is_null()).cloned().collect())
    }

    // Every widget, in the person's order, on or off.
    pub fn widgets_view(&self) -> Value {
        let (off, order) = (self.list_setting(config::WIDGETS_OFF), self.list_setting(config::WIDGET_ORDER));
        self.widgets.lock().unwrap().view(now_ms(), &off, &order, &|id: &str| plugins::owner_of(id))
    }

    // A widget from a script: there it is, and it may open the island for
    // itself. Never one by the id of a third-party plugin's widget that is
    // up now (not any id merely beginning like a plugin's: finding 11), so a
    // plugin cannot claim the person's `cpu_temp` next to a `cpu` plugin.
    pub fn put_widget(&self, v: &Value) -> Result<(), &'static str> {
        self.put_widget_from(v, false)
    }

    // The same from a plugin of hers (plugins.rs), whose buttons are kept
    // (widgets.rs actions).
    pub fn put_plugin_widget(&self, v: &Value) -> Result<(), &'static str> {
        self.put_widget_from(v, true)
    }

    fn put_widget_from(&self, v: &Value, may_act: bool) -> Result<(), &'static str> {
        if v.get("id").and_then(Value::as_str).is_some_and(|id| self.widgets.lock().unwrap().is_addon(id)) {
            return Err("id: taken by a plugin");
        }
        let put = self.widgets.lock().unwrap().put_from(v, now_ms(), may_act)?;
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
        if !self.is_visible() || self.setting(config::WIDGET_NUDGE) == false || !self.is_widget_on(id) || self.pet.lock().unwrap().wants_you() {
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
        if self.setting(config::NOTIFY).get(group).and_then(Value::as_bool) != Some(true) {
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
        let touches = |keys: &[&str]| keys.iter().any(|k| patch.contains_key(*k));
        let touches_visibility = touches(&[config::DND.name(), config::DISPLAY.name(), config::OUT.name(), config::HIDE_IN_FULLSCREEN.name(), config::TASKBAR_EDGE.name(), config::TASKBAR_DISPLAYS.name(), config::SPLIT_BAR.name(), config::SPLIT_DOCK.name()]);
        let touches_top = touches(&[config::SETTINGS_PIN.name(), config::SETTINGS_TOP.name()]);
        let touches_monitor = touches(&[config::MONITOR.name(), config::MONITOR_SOURCE.name(), config::WIDGETS_OFF.name()]);
        let touches_stow = touches(&[config::ISLAND_HIDE_UNDER.name()]);
        let touches_shortcuts = touches(&[config::SHORTCUTS_ENABLED.name(), config::SHORTCUT_APPROVE.name(), config::SHORTCUT_DENY.name(), config::SHORTCUT_ISLAND.name(), config::DND.name()]);
        {
            let mut settings = self.settings.lock().unwrap();
            settings.extend(patch);
            data::save(&self.dir, &settings);
        }
        if touches_monitor {
            crate::loops::wake_monitor();
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
        let own = self.own_windows();
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
        // What is in front either way: back, which window it was (Start over
        // a game, or another program) tells why the taskbar came back.
        let which = format!(", {}", fullscreen::front());
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
            }
            // A code come by mail is a minute's thing: gone, not kept for later.
            let codes = mail::codes::clear();
            if n > 0 || codes > 0 {
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
        if self.settings_open() && win.label() != "island" {
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
            if before != mine && !self.is_own(before) {
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

// A new home: the island, the taskbar, or the top bar and the dock (she goes into it).
pub fn set_display(sh: &Arc<Shared>, display: &str) {
    if sh.setting(config::DISPLAY).as_str() == Some(display) && !sh.flag(config::OUT) {
        return;
    }
    if sh.flag(config::OUT) {
        pet::hide_now(sh);
    }
    sh.change(json!({ "display": display, "out": false }));
}

// Out on the desktop where she was last left, or back home, from the menu.
pub fn set_out(sh: &Arc<Shared>, out: bool) {
    if sh.flag(config::OUT) == out {
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
    if sh.flag(config::OUT) {
        pet::come_home(sh);
    }
    sh.apply_visibility();
    tray::refresh_menu(sh);
}

pub(crate) fn shared(app: &AppHandle) -> Arc<Shared> {
    app.state::<Arc<Shared>>().inner().clone()
}

// Nobody at the keyboard or mouse this long: quiet (Shared::check_quiet).
// WAKUWAKU_QUIET_AFTER_MS, for a test that cannot wait; 0 never.
pub const QUIET_AFTER_MS: u64 = 90_000;

fn quiet_after_ms() -> u64 {
    static AFTER: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *AFTER.get_or_init(|| std::env::var("WAKUWAKU_QUIET_AFTER_MS").ok().and_then(|s| s.parse().ok()).unwrap_or(QUIET_AFTER_MS))
}

// A thread with a name, for whoever looks at her threads' CPU.
pub fn spawn_named<F: FnOnce() + Send + 'static>(name: &str, f: F) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new().name(name.to_string()).spawn(f).expect("a thread")
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
