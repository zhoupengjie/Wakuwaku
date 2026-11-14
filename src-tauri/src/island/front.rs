// What comes to the front of her home: the settings grown out of it, the
// prompts asked for from another display, the window raised and kept on
// top; and where it all is (where_, for /health).
use super::*;

// --- The settings, grown out of her home ------------------------------------------------

// Open them on a page (or where they were). With no home up (she is out of
// sight), one rises for them and goes once they close.
pub fn open_settings(sh: &Arc<Shared>, tab: Option<&str>) {
    let is_up = sh.is_visible();
    let ready = {
        let mut isl = sh.island.lock().unwrap();
        isl.temp |= !is_up;
        isl.settings_open = true;
        if !isl.ready {
            isl.pending = Some(tab.unwrap_or("").to_string());
        }
        isl.ready
    };
    sh.log(&format!("island: settings open on {:?}{}", tab, if is_up { "" } else { " (risen for them)" }));
    apply_visibility(sh);
    keep_on_top(sh);
    // Mail's agents looked for again while they are open; the monitor's
    // sensors read whole for their list.
    crate::mail::agent::watch(sh);
    crate::loops::wake_monitor();
    sh.redraw();
    if ready {
        let _ = sh.app.emit_to("island", "island:settings", json!({ "tab": tab.unwrap_or("") }));
    }
}

// The page closed them (Esc, a click outside, ✕, the head). Once the home
// has shrunk back: a risen one goes, and so does one the settings kept up
// (she was hidden, or do not disturb turned on, while they were open).
pub fn settings_closed(sh: &Arc<Shared>) {
    sh.island.lock().unwrap().settings_open = false;
    sh.log("island: settings closed");
    keep_on_top(sh);
    // Set up once Claude Code or Codex is connected, not as the settings
    // close (a click outside closed them, and a new user, nothing
    // connected, was never asked again: the test session, 2026-10-12).
    if !sh.config().onboarded && crate::agents::connection::seems_connected() {
        sh.change(json!({ "onboarded": true }));
    } else {
        sh.redraw();
    }
    let sh = sh.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(620));
        {
            let mut isl = sh.island.lock().unwrap();
            if isl.settings_open {
                return;
            }
            isl.temp = false;
        }
        apply_visibility(&sh);
        sh.redraw();
    });
}

// --- The prompts, asked for from another display ---------------------------------------

// Her capsule on another display's taskbar pressed (bars.rs bar_asks): the
// prompt waiting shown in her island, as a press on hers would (it opens
// for a prompt by itself, so it is there already): risen should an app
// full screen have hidden it, kept up until the prompts are answered (as
// for the settings), raised over any window on top, and the page told to
// draw the eye to it (island:look-here) on its own display.
pub fn show_asks(sh: &Arc<Shared>) {
    let risen = {
        let up = is_shown(sh);
        let mut isl = sh.island.lock().unwrap();
        isl.for_asks |= !up;
        !up
    };
    sh.log(&format!("island: the prompt asked for from another display{}", if risen { " (risen for it)" } else { "" }));
    if risen {
        apply_visibility(sh);
    }
    // (Not while the settings, locked, are let go under other windows.)
    let hwnd = hwnd(sh);
    if hwnd != 0 && TOPMOST.load(Ordering::SeqCst) {
        crate::tasks::raise_top(hwnd);
    }
    let _ = sh.app.emit_to("island", "island:look-here", json!({}));
}

// Something opened up from her taskbar's strip (island.ts openedUp): the
// island, a program's windows, a flyout. Her window is kept on top, but a
// window can lie over it all the same: one kept on top itself that came to
// the front after hers, or hers no longer kept on top. A maximized window
// stops at the strip, so only what opens above the strip is under it
// (zhoupengjie/Wakuwaku#55: the open island cut off at the strip's top, an
// editor over the rest). Windows' own taskbar opens its flyouts in a band
// above every window kept on top; hers is raised to the top of them as it
// opens, should any other lie over it (not while the settings, locked, are
// let go under other windows), and the line says which. Her island along
// the foot of the screen (#105), laid over Windows' own taskbar, the same.
pub fn raise(sh: &Arc<Shared>) {
    let hwnd = hwnd(sh);
    let (shown, over) = {
        let isl = sh.island.lock().unwrap();
        (isl.shown, isl.bottom || isl.strip.is_some())
    };
    // Not as a child of Windows' taskbar (embed.rs): in its order there.
    if hwnd == 0 || !shown || !(sh.home() == "taskbar" || over) || !TOPMOST.load(Ordering::SeqCst) || crate::embed::is_parented() {
        return;
    }
    let own = sh.own_windows();
    let Some((class, on_top, _)) = crate::tasks::cover(hwnd, &own) else { return };
    let mine = crate::tasks::is_topmost(hwnd);
    crate::tasks::raise_top(hwnd);
    sh.log(&format!(
        "island: opened under another window ({class}, {}; hers {}): raised over it",
        if on_top { "kept on top" } else { "not kept on top" },
        if mine { "kept on top" } else { "not kept on top" }
    ));
}

// Her island along the foot of the screen, laid over Windows' own taskbar
// (islandEdge taskbar, the embedding off, zhoupengjie/Wakuwaku#105): Explorer brings its
// taskbar to the top of the windows kept on top as it is pressed (Start, a
// button, the tray), over hers, which is then hidden under it, the pointer
// never reaching it. So it is owned by Windows' taskbar there (embed.rs
// detach, over_wanted), which Windows keeps it over; and, should it be found
// under a taskbar all the same (another's kept on top, Windows' own along a
// side or hiding itself, not its owner), raised over it again: every
// 2 s (keep_on_screen), and as the pointer comes near it (poll). Only from
// under a taskbar: Windows' own (Shell_TrayWnd, a second display's), or
// another's kept on top within the strip's band (Start11's and the like);
// the Start menu and the flyouts open above every window kept on top, and
// over her island is right (the user's call, 2026-11-07). Logged as it is
// first found under one, not at each raising.
pub(super) static UNDER_TASKBAR: AtomicBool = AtomicBool::new(false);

pub fn uncover(sh: &Shared) {
    // The strip's band: from the window's foot up along the bottom, from
    // its top down along the top (a taskbar there). Not with her capsule
    // inside the taskbar: the window shows only as it is used, raised then.
    let (hwnd, band) = {
        let isl = sh.island.lock().unwrap();
        (isl.hwnd, (isl.shown && !isl.embedded).then_some(isl.strip).flatten().map(|thick| if isl.bottom { (true, isl.pos.1 + isl.size.1 - thick) } else { (false, isl.pos.1 + thick) }))
    };
    let Some((low, band)) = band else { return };
    if hwnd == 0 || !TOPMOST.load(Ordering::SeqCst) || crate::embed::is_parented() {
        return;
    }
    let own = sh.own_windows();
    let taskbar = crate::tasks::cover(hwnd, &own).filter(|(class, on_top, r)| matches!(class.as_str(), "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") || (*on_top && if low { r.1 >= band - 2 } else { r.3 <= band + 2 }));
    let Some((class, _, _)) = taskbar else {
        UNDER_TASKBAR.store(false, Ordering::SeqCst);
        return;
    };
    crate::tasks::raise_top(hwnd);
    if !UNDER_TASKBAR.swap(true, Ordering::SeqCst) {
        sh.log(&format!("island: under the taskbar ({class}): raised over it"));
    }
}

// The prompts answered: an island risen for them goes.
pub fn asks_answered(sh: &Arc<Shared>) {
    let was = std::mem::take(&mut sh.island.lock().unwrap().for_asks);
    if was {
        apply_visibility(sh);
    }
}

// The settings' two buttons by ✕ (settings.ts): locked ("settingsPin"), a
// click outside does not close them, and other windows come over them as
// over any window (hers kept on top only while they are not open); on top
// as well ("settingsTop"), they stay over every window, another program's
// own "on top" one too: looked at every 0.7 s, raised should one be over
// them. Unlocked, a click outside closes them: on top as ever.
pub(super) static TOPMOST: AtomicBool = AtomicBool::new(true);
pub(super) static WATCHING_TOP: AtomicBool = AtomicBool::new(false);

pub fn keep_on_top(sh: &Arc<Shared>) {
    let open = sh.settings_open();
    let (locked, top) = (sh.config().settings_pin, sh.config().settings_top);
    let topmost = !(open && locked && !top);
    if TOPMOST.swap(topmost, Ordering::SeqCst) != topmost {
        if let Some(win) = window(sh) {
            let _ = win.set_always_on_top(topmost);
            // On top again (or not) takes a Mac's window off the menu bar's level.
            if topmost {
                crate::notch::settle(sh);
            }
        }
        sh.log(&format!("island: settings {}", if topmost { "on top of other windows" } else { "under the windows pressed" }));
    }
    if !(open && locked && top) || WATCHING_TOP.swap(true, Ordering::SeqCst) {
        return;
    }
    let sh = sh.clone();
    std::thread::spawn(move || loop {
        let watched = || sh.settings_open() && sh.config().settings_pin && sh.config().settings_top;
        while watched() {
            let hwnd = hwnd(&sh);
            let own = sh.own_windows();
            if hwnd != 0 && crate::tasks::covered(hwnd, &own) {
                crate::tasks::raise_top(hwnd);
            }
            std::thread::sleep(std::time::Duration::from_millis(700));
        }
        WATCHING_TOP.store(false, Ordering::SeqCst);
        // Asked again in the moment between the last look and letting go.
        if !watched() || WATCHING_TOP.swap(true, Ordering::SeqCst) {
            return;
        }
    });
}

pub fn where_(sh: &Shared) -> Value {
    let actual = window(sh).and_then(|win| crate::main_wait::outer_position(&win)).map(|p| json!([p.x, p.y]));
    // Read before the island's lock: embed.rs takes EMBED's alone.
    let embed = crate::embed::where_();
    let isl = sh.island.lock().unwrap();
    json!({
        "actual": actual,
        "pos": [isl.pos.0, isl.pos.1],
        "size": [isl.size.0, isl.size.1],
        "room": isl.room.map(|(w, h)| json!([w, h])),
        "shown": isl.shown,
        "over": isl.ct.over,
        "caught": isl.ct.caught(),
        "editing": isl.editing,
        "solid": isl.solid.len(),
        "holding": isl.holding,
        "reaching": isl.reaching,
        "temp": isl.temp,
        "settingsOpen": isl.settings_open,
        "bar": isl.bar.map(|b| json!([b.0, b.1, b.2, b.3])),
        "at": isl.at.name(),
        "edge": if isl.bottom { "bottom" } else { "top" },
        "strip": isl.strip,
        "embedded": isl.embedded,
        "cap": isl.cap.map(|c| json!([c.0, c.1, c.2, c.3])),
        "capRoom": isl.cap_room,
        "mx": isl.mx,
        "opened": isl.opened,
        "embed": embed,
        "stow": {
            "closed": closed_rect(&isl).map(|r| json!([r.0, r.1, r.2, r.3])),
            "under": isl.stow.under,
            "stowed": isl.stow.stowed,
            "called": isl.stow.called,
        },
    })
}
