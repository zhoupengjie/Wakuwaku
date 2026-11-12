// The taskbar's thread: its hidden window and message loop (run, wndproc),
// the WinEvents and the shell hook it hears (on_event, on_shell), the clock
// that looks every half second (tick), and Explorer started again
// (on_created).
use super::*;

pub(super) static OURS: AtomicIsize = AtomicIsize::new(0);
// Explorer's taskbar window: a new one means Explorer started again.
pub(super) static EXPLORER: AtomicIsize = AtomicIsize::new(0);
pub(super) static CREATED_MSG: AtomicU32 = AtomicU32::new(0);
pub(super) static TICKS: AtomicU32 = AtomicU32::new(0);
// From this tick, the icons only Explorer's tray holds are looked for
// again (systray/ scan_held): some seconds after each asking, else
// every half minute.
pub(super) static HELD_AT: AtomicU32 = AtomicU32::new(0);
pub(super) static SHELL_MSG: AtomicU32 = AtomicU32::new(0);
// The windows over a display (fills_display), each with that display, as
// last seen moving (on_event).
pub(super) static OVER: Lock<Vec<(isize, isize)>> = Lock::new("taskbar::win::thread::OVER", Vec::new());

pub(super) fn run(strip: (i32, i32, i32, i32), ready: std::sync::mpsc::Sender<isize>) {
    // SAFETY: our own class and window, on this thread, which runs its loop.
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class = wide("WakuwakuTaskbarHost");
        let wc = WndClassExW {
            size: std::mem::size_of::<WndClassExW>() as u32,
            style: 0,
            wndproc,
            cls_extra: 0,
            wnd_extra: 0,
            instance,
            icon: 0,
            cursor: 0,
            background: 0,
            menu_name: std::ptr::null(),
            class_name: class.as_ptr(),
            icon_sm: 0,
        };
        RegisterClassExW(&wc);
        // Top-level (not message-only): TaskbarCreated is broadcast to those.
        let hwnd = CreateWindowExW(WS_EX_TOOLWINDOW, class.as_ptr(), std::ptr::null(), WS_POPUP, 0, 0, 0, 0, std::ptr::null_mut(), 0, instance, std::ptr::null_mut());
        if hwnd.is_null() {
            return;
        }
        OURS.store(hwnd as isize, Ordering::SeqCst);
        // The windows' live pictures go on a window of their own over the
        // cards (taskbar/mod.rs thumbs): drawn on the home's, a webview's,
        // they froze, and went under it once it drew again (2026-10-10).
        // Dark as the cards' room, and letting the pointer through to them.
        let thumbs_class = wide("WakuwakuThumbs");
        let wc = WndClassExW {
            size: std::mem::size_of::<WndClassExW>() as u32,
            style: 0,
            wndproc: thumbs_proc,
            cls_extra: 0,
            wnd_extra: 0,
            instance,
            icon: 0,
            cursor: 0,
            background: CreateSolidBrush(0x001E_1C1C),
            menu_name: std::ptr::null(),
            class_name: thumbs_class.as_ptr(),
            icon_sm: 0,
        };
        RegisterClassExW(&wc);
        let host = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT,
            thumbs_class.as_ptr(),
            std::ptr::null(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            0,
            instance,
            std::ptr::null_mut(),
        );
        if !host.is_null() {
            SetLayeredWindowAttributes(host, 0, 255, LWA_ALPHA);
            THUMB_HOST.store(host as isize, Ordering::SeqCst);
        }
        // The dock's own (its minimized windows') is the dock's: make_dock_host.
        CREATED_MSG.store(shell::taskbar_created(), Ordering::SeqCst);
        EXPLORER.store(shell::taskbars().first().copied().unwrap_or(0), Ordering::SeqCst);
        EXPLORER_PID.store(shell::explorer_pid(), Ordering::SeqCst);
        let hooks: Vec<isize> = WINDOW_EVENTS.iter().map(|&(min, max)| SetWinEventHook(min, max, 0, on_event, 0, 0, 0)).collect();
        SHELL_MSG.store(RegisterWindowMessageW(wide("SHELLHOOK").as_ptr()), Ordering::SeqCst);
        let shell_hook = RegisterShellHookWindow(hwnd) != 0;
        SetTimer(hwnd, TICK, 500, std::ptr::null());
        TICKS.store(0, Ordering::SeqCst);
        *TUG.lock().unwrap() = Tug::new();
        let tray = systray::start(strip, hwnd as isize, WM_TRAY_CHANGED);
        ASK_AT.store(SETTLE_TICKS, Ordering::SeqCst);
        HELD_AT.store(0, Ordering::SeqCst);
        log(&format!("up: window hooks {}, shell hook {shell_hook}, tray {tray} (first: {})", hooks.iter().filter(|h| **h != 0).count(), systray::first()));
        let _ = ready.send(hwnd as isize);

        let mut msg: Msg = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        for hook in hooks.into_iter().filter(|h| *h != 0) {
            UnhookWinEvent(hook);
        }
        DeregisterShellHookWindow(hwnd);
        KillTimer(hwnd, TICK);
        KillTimer(hwnd, SETTLE);
        KillTimer(hwnd, WORK_STILL);
        log(&format!("tray handed back; it was handed {}", systray::seen()));
        systray::stop();
        drop_thumbs();
        // The dock's pictures and their icons hidden by the main thread,
        // whose windows they are; not waited for: quitting, the main
        // thread is waiting for this one to end (hold.rs stop_within), and
        // each call waited on it till it gave up, two seconds.
        if let Some(sh) = sh() {
            let _ = sh.app.run_on_main_thread(|| {
                dock_thumbs(Vec::new(), None);
                dock_badges(Vec::new());
            });
        }
        let host = THUMB_HOST.swap(0, Ordering::SeqCst);
        if host != 0 {
            DestroyWindow(host as Hwnd);
        }
        DestroyWindow(hwnd);
        OURS.store(0, Ordering::SeqCst);
    }
}

// A window changed: the buttons are made again (their thread, woken);
// Explorer's taskbar shown: put away again.
pub(super) extern "system" fn on_event(_hook: isize, event: u32, hwnd: Hwnd, object: i32, child: i32, _thread: u32, _time: u32) {
    if object != OBJID_WINDOW || child != 0 || hwnd.is_null() {
        return;
    }
    if event == EVENT_OBJECT_SHOW {
        let mut name = [0u16; 32];
        // SAFETY: our own buffer, its size passed.
        let n = unsafe { GetClassNameW(hwnd, name.as_mut_ptr(), 32) }.max(0) as usize;
        let class = String::from_utf16_lossy(&name[..n]);
        if class == "Shell_TrayWnd" || class == "Shell_SecondaryTrayWnd" {
            // SAFETY: our own window, handled on its thread.
            unsafe { PostMessageW(OURS.load(Ordering::SeqCst) as Hwnd, WM_TASKBAR_SHOWN, 0, hwnd as isize) };
            return;
        }
    }
    if matches!(event, EVENT_OBJECT_SHOW | EVENT_SYSTEM_FOREGROUND | EVENT_OBJECT_LOCATIONCHANGE) {
        keep_under_bar(hwnd as isize, event == EVENT_OBJECT_LOCATIONCHANGE);
    }
    // A window to the front, or the one in front sized (full screen, or
    // no longer: a video, F11): looked at at once, not at the clock's
    // next look (up to 1.5 s late); on the look's own thread, not this
    // one (shared.rs, Shared::look_fullscreen).
    if event == EVENT_SYSTEM_FOREGROUND || (event == EVENT_OBJECT_LOCATIONCHANGE && tasks::front() == hwnd as isize) {
        crate::fullscreen::ask();
    }
    // Moved or sized: only a window over a display, or no longer, or
    // over another one now, changes the buttons (a clear strip filled
    // in while one is over its display: hers, island.ts; the others',
    // bar.ts); every move of every window comes here.
    if event == EVENT_OBJECT_LOCATIONCHANGE {
        let h = hwnd as isize;
        // SAFETY: a plain query; a gone window gives the nearest display.
        let now = fills_display(h).then(|| unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) } as isize);
        let mut over = OVER.lock().unwrap();
        let was = over.iter().find(|(w, _)| *w == h).map(|(_, m)| *m);
        if now == was {
            return;
        }
        over.retain(|(w, _)| *w != h);
        if let Some(m) = now {
            over.push((h, m));
        }
    }
    if event == EVENT_SYSTEM_FOREGROUND {
        start_seen(tasks::is_start(hwnd as isize));
        if tasks::is_desktop(hwnd as isize) {
            over_desktop();
        }
    }
    wake_tasks();
}

// The shell hook: a window flashing for attention (until it is in
// front), one activated.
// Each told to the buttons' thread (buttons.rs Note), whose they are.
pub(super) fn on_shell(what: usize, hwnd: isize) {
    match what {
        HSHELL_FLASH => tell(Note::Flash(hwnd)),
        HSHELL_WINDOWACTIVATED | HSHELL_RUDEAPPACTIVATED => tell(Note::Activated(hwnd)),
        // A title or an icon changed: the buttons' thread says whether
        // that makes the buttons again now (buttons.rs take_note).
        HSHELL_REDRAW => {
            // Never ours (no button; and asking our own window's title
            // would wait on the main thread).
            if !tasks::is_ours(hwnd) {
                tell(Note::Redrawn(hwnd));
            }
        }
        HSHELL_WINDOWCREATED | HSHELL_WINDOWDESTROYED => wake_tasks(),
        _ => {}
    }
}

pub(super) fn tick() {
    // Quiet (nobody at the computer): what follows every two seconds,
    // not every half (each look woke this thread some ten times, twenty
    // a second all night: #14). The clock keeps its pace.
    let at = TICKS.load(Ordering::SeqCst);
    if at % 4 != 0 && sh().is_some_and(|sh| sh.is_quiet()) {
        TICKS.fetch_add(1, Ordering::SeqCst);
        return;
    }
    // Another copy holding it too (both took it at once): every two
    // seconds, the younger lets go.
    if at % 4 == 0 && !waiting() {
        yield_to_elder();
    }
    if shell::visible() {
        put_away_again("seen by the timer");
    }
    // A new Explorer (one whose taskbar does not show, too): ours set
    // behind it while it starts, and first again once it has.
    explorer_check();
    come_forward();
    // The desktop lifted over her taskbar after the raises that followed
    // it to the front (over_desktop): raised again.
    if let Some(sh) = sh() {
        let me = crate::island::hwnd(&sh);
        if me != 0 && !RAISING.load(Ordering::SeqCst) && tasks::under_desktop(me) {
            log("the desktop lay over her windows (show the desktop): raised again");
            raise_own_with_thumbs(&sh);
        }
    }
    keep_work(false);
    systray::sweep();
    if systray::keep_first() {
        log("tray: Explorer's window had come first (timer); ours put ahead again");
        ask_soon();
    }
    let ticks = TICKS.fetch_add(1, Ordering::SeqCst) + 1;
    let ask = ASK_AT.load(Ordering::SeqCst);
    if ask != 0 && ticks >= ask && systray::first() {
        ASK_AT.store(0, Ordering::SeqCst);
        LAST_ASK.store(ticks, Ordering::SeqCst);
        systray::ask_again();
        log("tray: programs asked for their icons");
        HELD_AT.store(ticks + 8, Ordering::SeqCst);
    }
    // Those that answered are in by now: what only Explorer's tray holds
    // is shown from Windows' record (zhoupengjie/Wakuwaku#52). Not while
    // nobody is about (quiet): a look costs some 8 ms and 30 round trips
    // to Explorer (measured 2026-10-16, every half minute, quiet or not),
    // and no one sees the tray then; the first tick after quiet does it.
    let held = HELD_AT.load(Ordering::SeqCst);
    let quiet = sh().is_some_and(|sh| sh.is_quiet());
    if held != 0 && ticks >= held && !quiet && systray::first() {
        HELD_AT.store(ticks + 60, Ordering::SeqCst);
        systray::scan_held();
    }
    // Windows' mode, accent and wallpaper: told when they change
    // (WM_SETTINGCHANGE and the rest below); looked at anyway every
    // half minute, should a change come unannounced (by the tick it
    // began on: one quiet looks at).
    if at % 60 == 0 {
        send_look();
    }
}

// Explorer started again (a new taskbar window), or a program's asking
// (the same one: ours, or another tray's).
// Debug only (server.rs /debug/hang): this thread held still for `secs`
// seconds, as a hang of it is (her guard's ten seconds, shell.rs HangBack).
pub fn hold_still(secs: u64) -> bool {
    let ours = OURS.load(Ordering::SeqCst);
    // SAFETY: our own window; posting asks nothing of it now.
    ours != 0 && unsafe { PostMessageW(ours as Hwnd, WM_HOLD_STILL, secs as usize, 0) } != 0
}

pub(super) fn on_created() {
    explorer_check();
    let now = shell::taskbars().first().copied().unwrap_or(0);
    if now == EXPLORER.swap(now, Ordering::SeqCst) || HANDING_BACK.load(Ordering::SeqCst) {
        return;
    }
    log("Explorer started again");
    // Not while her guard shows Windows' for a hang (put_away_again).
    if !shown_for_hang() {
        shell::rehide();
    }
    take_strip_again();
    systray::keep_first();
    ask_soon();
}

pub(super) extern "system" fn wndproc(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize {
    match msg {
        WM_TIMER if wparam == SETTLE => settling(hwnd),
        WM_TIMER if wparam == WORK_STILL => {
            // SAFETY: our own window's timer, on its thread.
            unsafe { KillTimer(hwnd, WORK_STILL) };
            keep_work(true);
        }
        WM_TIMER => tick(),
        WM_TASKBAR_SHOWN => {
            // SAFETY: a plain check of Explorer's window.
            if unsafe { IsWindowVisible(lparam as Hwnd) } != 0 {
                put_away_again("its show event");
                work_after_still(hwnd);
            }
        }
        WM_DISPLAYCHANGE => displays_changed(wparam, lparam),
        WM_TRAY_CHANGED => send_icons(),
        WM_RESEND_TRAY => resend_tray(),
        WM_THUMBS => thumbs_asked(),
        WM_THUMBS_GROUND => thumbs_ground_here(wparam != 0),
        WM_PAPER => send_look(),
        WM_SETTINGCHANGE if wparam == SPI_SETWORKAREA as usize => {
            work_after_still(hwnd);
            settle(hwnd);
        }
        WM_PAPER_SET | WM_DWMCOLORIZATIONCOLORCHANGED | WM_SETTINGCHANGE => settle(hwnd),
        WM_STOP => {
            watch(false);
            // SAFETY: ending this thread's loop.
            unsafe { PostQuitMessage(0) }
        }
        WM_WATCH_PRESSES => watch(wparam != 0),
        WM_HOLD_STILL => std::thread::sleep(Duration::from_secs(wparam as u64)),
        WM_PRESS_SEEN => press_seen(wparam as u32 as i32, lparam as i32),
        _ if msg == CREATED_MSG.load(Ordering::SeqCst) && msg != 0 => on_created(),
        _ if msg == SHELL_MSG.load(Ordering::SeqCst) && msg != 0 => on_shell(wparam, lparam),
        // SAFETY: the default for the rest.
        _ => return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
    0
}
