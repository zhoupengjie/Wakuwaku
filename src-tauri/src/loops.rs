// Her threads that look every so often, started as she starts (main.rs
// setup): the pointer's, the widgets', the monitor's, full screen's and the
// ticker's; and the waits of the pointer's and the monitor's, woken before
// their time (wake_pointer, wake_monitor).
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use crate::{bars, desk, fullscreen, hw, island, lhm, mail, now_ms, pet, plugins, power, screen, settings, spawn_named, tokens, widgets, Shared};

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

// The mouse: click-through, her eyes, drags; for both windows.
// Quiet (nobody touching it), looked at once a second. Woken
// before its time by wake_pointer (desk.rs: Ctrl pressed or let go).
pub fn pointer(sh: &Arc<Shared>) {
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
            (Some(her), Some(home)) => pet::poll(&pointer, her).min(island::poll(&pointer, home)).min(desk::poll(&pointer, home)).min(bars::poll(&pointer, home)),
            _ => 200,
        };
        let wait = if quiet { wait.max(1000) } else { wait };
        std::thread::park_timeout(Duration::from_millis(wait));
    });
}

// The widgets: the tokens' each minute while they are on; a script's
// gone once it stops sending; today's counts begun again at midnight.
pub fn widgets(sh: &Arc<Shared>) {
    let watcher = sh.clone();
    spawn_named("widgets", move || {
        power::thread_eco();
        let mut tokens = tokens::Tokens::default();
        let dirs = tokens::AgentDirs::discover();
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
                    tokens.update(&dirs, widgets::local_midnight(), &widgets::local_date());
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
}

// The monitor: the parts that are on, every so many seconds (its
// setting, 1 to 10), while it is on; read again at once when it
// is turned on, its pace or source changes, the quiet does, or
// the settings open (their list of sensors, read whole while they
// are) (woken by Shared::change, check_quiet and open_settings:
// it looked four times a second whether they had, #14).
pub fn monitor(sh: &Arc<Shared>) {
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
}

// Another app full screen, looked at on this thread alone
// (Shared::look_fullscreen); with it, a window maximized on her
// island's display, and one under it.
pub fn fullscreen(sh: &Arc<Shared>) {
    let watcher = sh.clone();
    spawn_named("fullscreen", move || loop {
        let asked = fullscreen::wait(Duration::from_millis(1500));
        watcher.look_fullscreen(asked);
        island::look_over_max(&watcher);
        island::look_under(&watcher);
        // The dock out of the way of a window touching it (bars.rs).
        bars::look_under(&watcher);
    });
}

// Endings held for a while; prompts out of time; sessions gone
// quiet, out of sight and forgotten; the windows on screen.
pub fn ticker(sh: &Arc<Shared>) {
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
                for id in &gone {
                    ticker.log(&format!("prompt {id} handed to the terminal: its time is up"));
                }
                // A code come by mail, its minute up (or its "Copied" read).
                if mail::codes::expire(now) || !gone.is_empty() {
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
}
