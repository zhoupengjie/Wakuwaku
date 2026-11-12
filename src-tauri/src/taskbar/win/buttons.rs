// The windows' buttons: the list of windows that have one (tasks.rs), made
// again on a thread of its own as windows come, go and change, and sent
// (taskbar:windows), each with the sessions that run in it (jump.rs); a
// program started, a window brought up, minimized or closed, its menu.
use super::*;
use crate::lock::Lock;

pub(super) fn wake_tasks() {
    let (dirty, cv) = &WAKE;
    *dirty.lock().unwrap() = true;
    cv.notify_one();
}

// From outside: the quiet over (shared.rs check_quiet), the buttons and
// the keys looked at now.
pub fn wake() {
    wake_tasks();
    wake_keys();
}

// The buttons' list kept: made again when windows change (a moment
// after, so a burst of changes makes it once) and every two seconds;
// the sessions' windows looked for every three (it looks through the
// processes).
// Asked to stop, it stops at once, whatever it was at: woken by a
// change while making the buttons, its wake was cleared after, and it
// waited two seconds more before it looked (a quit for an update,
// 2026-10-11). What it is doing, for the log should it not stop.
pub(super) static TASKS_DOING: Lock<&'static str> = Lock::new("taskbar::win::buttons::TASKS_DOING", "");

// Not easy on the power for good (power.rs thread_eco), as the loops
// that only look now and then are: the buttons it makes are seen as
// windows come and go, and with the CPU busy they came late (the user,
// 2026-11-08: 高负载的情况下整个界面很容易卡死). Slowed with the process
// while nobody is about (shared.rs check_quiet).
pub(super) fn tasks_loop(sh: Arc<Shared>) {
    let doing = |what: &'static str| *TASKS_DOING.lock().unwrap() = what;
    let stopping = || TASKS_STOP.load(Ordering::SeqCst);
    let mut marked_at = Instant::now() - Duration::from_secs(10);
    loop {
        doing("waiting");
        {
            let (dirty, cv) = &WAKE;
            let guard = dirty.lock().unwrap();
            // Looked at anyway every five seconds (what the hooks miss:
            // an icon changed alone); quiet (nobody at the keyboard),
            // every twenty, woken as it ends.
            let every = Duration::from_secs(if sh.is_quiet() { 20 } else { 5 });
            let (mut guard, _) = guard.wait_timeout_while(cv, every, |d| !*d && !stopping()).unwrap();
            *guard = false;
        }
        if stopping() {
            return;
        }
        std::thread::sleep(Duration::from_millis(120));
        *WAKE.0.lock().unwrap() = false;
        if stopping() {
            return;
        }
        if marked_at.elapsed() >= Duration::from_secs(3) {
            marked_at = Instant::now();
            doing("marking the sessions' windows");
            mark_sessions(&sh);
        }
        doing("making the buttons");
        send_windows(&sh);
    }
}

// Each session with a window of its own to go to, by that window; one
// gone out of sight (state.rs is_shown) marks none, its window's button
// there as ever.
pub(super) fn mark_sessions(sh: &Shared) {
    let targets: Vec<(String, String, crate::state::Chain, Vec<String>)> = {
        let pet = sh.pet.lock().unwrap();
        let list = pet.list();
        list.as_array()
            .map(|all| {
                all.iter()
                    .filter(|s| s["jump"] == true)
                    .filter_map(|s| {
                        let id = s["id"].as_str()?.to_string();
                        let (chain, hints) = pet.jump_target(&id)?;
                        Some((id, s["mood"].as_str().unwrap_or("idle").to_string(), chain, hints))
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    // Found before and still there: kept, looked for anew after a
    // minute (a session may move to another window); not found before:
    // looked for again after a while (many have no window of their own).
    // Taken out while the windows are looked for (jump.rs takes its own
    // lock, and walks every process) and put back after: this thread's
    // alone, none of them held over another (lock.rs).
    let mut found = SESSION_WINDOWS.lock().unwrap().take().unwrap_or_default();
    let mut looked = SESSION_LOOKED.lock().unwrap().take().unwrap_or_default();
    found.retain(|id, _| targets.iter().any(|t| t.0 == *id));
    looked.retain(|id, _| targets.iter().any(|t| t.0 == *id));
    let retry = Duration::from_secs(if sh.is_quiet() { 60 } else { 15 });
    let mut marks: HashMap<isize, Vec<Value>> = HashMap::new();
    let mut asks: Vec<(crate::state::Chain, Vec<String>)> = Vec::new();
    let mut asked: Vec<(String, String)> = Vec::new();
    for (id, mood, chain, hints) in targets {
        if let Some(&(hwnd, pid, at)) = found.get(&id) {
            // Kept while the session's own process runs and the window is there.
            if at.elapsed() < Duration::from_secs(60) && chain.first().is_some_and(|&(p, t)| jump::still_running(p, t)) && jump::still_there(hwnd, pid) {
                marks.entry(hwnd).or_default().push(json!({ "id": id, "mood": mood }));
                continue;
            }
            found.remove(&id);
        }
        if looked.get(&id).is_some_and(|t| t.elapsed() < retry) {
            continue;
        }
        asks.push((chain, hints));
        asked.push((id, mood));
    }
    for ((id, mood), got) in asked.into_iter().zip(jump::windows_for(&asks)) {
        looked.insert(id.clone(), Instant::now());
        if let Some((hwnd, pid)) = got {
            found.insert(id.clone(), (hwnd, pid, Instant::now()));
            marks.entry(hwnd).or_default().push(json!({ "id": id, "mood": mood }));
        }
    }
    *SESSION_WINDOWS.lock().unwrap() = Some(found);
    *SESSION_LOOKED.lock().unwrap() = Some(looked);
    *MARKS.lock().unwrap() = Some(marks);
}

// A window's app id, asked once (tasks::app_id goes through COM).
pub(super) fn app_id_of(hwnd: isize) -> Option<String> {
    let mut ids = APP_IDS.lock().unwrap();
    ids.get_or_insert_with(HashMap::new).entry(hwnd).or_insert_with(|| tasks::app_id(hwnd)).clone()
}

// A window's icon handle, asked of the window (three messages to its
// process) only when the shell said it was redrawn, or every ten seconds.
pub(super) fn icon_of(hwnd: isize, redrawn: &[isize]) -> isize {
    let mut icons = ICONS.lock().unwrap();
    let icons = icons.get_or_insert_with(HashMap::new);
    match icons.get(&hwnd) {
        Some(&(icon, at)) if !redrawn.contains(&hwnd) && at.elapsed() < Duration::from_secs(10) => icon,
        _ => {
            let icon = tasks::icon_of(hwnd);
            icons.insert(hwnd, (icon, Instant::now()));
            icon
        }
    }
}

// On the strip's display (a window maximized elsewhere leaves it clear).
pub(super) fn on_strip(hwnd: isize) -> bool {
    let Some(s) = *STRIP.lock().unwrap() else { return false };
    // SAFETY: plain queries.
    unsafe { MonitorFromWindow(hwnd as Hwnd, MONITOR_DEFAULTTONEAREST) == MonitorFromPoint(Point { x: s.0 + s.2 / 2, y: s.1 + s.3 / 2 }, MONITOR_DEFAULTTONEAREST) }
}

#[link(name = "user32")]
extern "system" {
    pub(super) fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
}

// Over its display: maximized on it, or full screen there (all of the
// display, as a game or a video has it; one minimized lies far off), in
// front or behind another. The clear strip on that display is not clear
// then: hers (island.ts isFilled, by "max", on the strip's display) and
// those on the others (bar.ts, by "fills" and the window's "mon").
pub(super) fn fills_display(hwnd: isize) -> bool {
    if tasks::maximized(hwnd) {
        return true;
    }
    let mut rect = Rect::default();
    let mut info = MonitorInfo { size: std::mem::size_of::<MonitorInfo>() as u32, ..Default::default() };
    // SAFETY: plain queries into our own structs, their sizes set.
    unsafe {
        if GetWindowRect(hwnd as Hwnd, &mut rect) == 0 || GetMonitorInfoW(MonitorFromWindow(hwnd as Hwnd, MONITOR_DEFAULTTONEAREST), &mut info) == 0 {
            return false;
        }
    }
    let m = &info.monitor;
    rect.left <= m.left && rect.top <= m.top && rect.right >= m.right && rect.bottom >= m.bottom
}

// A program that only hosts what its windows show, whose icons are
// theirs: a store app (its frame, before its app is in it; then the
// app's program, under WindowsApps), Windows' own apps, Java, Python.
pub(super) fn hosts(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    ["\\windowsapps\\", "\\systemapps\\", "\\immersivecontrolpanel\\"].iter().any(|d| p.contains(d))
        || ["applicationframehost.exe", "java.exe", "javaw.exe", "python.exe", "pythonw.exe", "py.exe", "pyw.exe"].iter().any(|n| p.ends_with(&format!("\\{n}")))
}

// Windows' plain program icon, drawn once at this size (plain_program_icon).
pub(super) fn plain_png(size: i32) -> String {
    static PLAIN: Lock<Option<(i32, String)>> = Lock::new("taskbar::win::buttons::PLAIN", None);
    let mut plain = PLAIN.lock().unwrap();
    if let Some((_, png)) = plain.as_ref().filter(|(at, _)| *at == size) {
        return png.clone();
    }
    let icon = tasks::plain_program_icon();
    let png = if icon == 0 { String::new() } else { png_of(icon, size).unwrap_or_default() };
    tasks::destroy_icon(icon);
    *plain = Some((size, png.clone()));
    png
}

// The buttons as the page draws them: one per program, as Windows' 11
// has them by default. The programs kept on the taskbar first, in the
// order they were kept (taskbarPinned: [{ path, name }]), whether they run
// or not; then the others in the order their first windows came. Each
// with its windows (each with the sessions in it), its name, its icon (its
// first window's, else its file's).
pub(super) fn send_windows(sh: &Shared) {
    let found = tasks::list();
    let front = tasks::front();
    let order: Vec<isize> = {
        let mut order = ORDER.lock().unwrap();
        order.retain(|h| found.iter().any(|t| t.hwnd == *h));
        for t in &found {
            if !order.contains(&t.hwnd) {
                order.push(t.hwnd);
            }
        }
        order.clone()
    };
    // What is kept per window goes with the window; the titles as shown
    // now; which windows the shell said were redrawn since the last look.
    APP_IDS.lock().unwrap().get_or_insert_with(HashMap::new).retain(|h, _| order.contains(h));
    ICONS.lock().unwrap().get_or_insert_with(HashMap::new).retain(|h, _| order.contains(h));
    *TITLES.lock().unwrap() = Some(found.iter().map(|t| (t.hwnd, t.title.clone())).collect());
    let redrawn: Vec<isize> = std::mem::take(&mut *REDRAWN.lock().unwrap());
    let flashing = {
        let mut flashing = FLASHING.lock().unwrap();
        flashing.retain(|h| *h != front && tasks::alive(*h));
        flashing.clone()
    };
    // As big as the buttons show them alone (island.ts, 24px); smaller
    // with their titles, and in the lists above them.
    let size = (24.0 * crate::island::scale(sh)).round() as i32;
    let marks = MARKS.lock().unwrap().clone().unwrap_or_default();
    // Kept on the taskbar: its path, and the app id of one Windows only
    // starts by it (a store app: Settings, Calculator).
    let pinned: Vec<(String, Option<String>)> = sh
        .config()
        .taskbar_pinned
        .as_array()
        .map(|all| all.iter().filter_map(|p| Some((p["path"].as_str()?.to_string(), p["id"].as_str().filter(|s| !s.is_empty()).map(str::to_string)))).collect())
        .unwrap_or_default();
    // A button each: (key, path, app id, pinned, windows), the keys' case
    // aside. A program by its file; one that only hosts what its windows
    // show (a store app) by the app they say they are, so its frame, which
    // first shows the app under ApplicationFrameHost.exe, and the app in it
    // stay one button. One kept by its id takes those that say it.
    let mut apps: Vec<(String, String, Option<String>, bool, Vec<&tasks::Task>)> =
        pinned.iter().map(|(p, id)| (id.clone().unwrap_or_else(|| p.to_lowercase()), p.clone(), id.clone(), true, Vec::new())).collect();
    for t in order.iter().filter_map(|h| found.iter().find(|t| t.hwnd == *h)) {
        let id = hosts(&t.path).then(|| app_id_of(t.hwnd)).flatten();
        let key = id.clone().unwrap_or_else(|| t.path.to_lowercase());
        match apps.iter_mut().find(|a| a.0.eq_ignore_ascii_case(&key)) {
            Some(app) => app.4.push(t),
            None => apps.push((key, t.path.clone(), id, false, vec![t])),
        }
    }
    let list: Vec<Value> = {
        // This thread's caches, taken out while the list is made (each
        // window asked for its icon, the other caches read) and put back
        // after: none of them held over another (lock.rs).
        let mut pngs = WIN_PNGS.lock().unwrap().take().unwrap_or_default();
        pngs.retain(|h, _| order.contains(h));
        let mut names = APP_NAMES.lock().unwrap().take().unwrap_or_default();
        let mut file_pngs = FILE_PNGS.lock().unwrap().take().unwrap_or_default();
        let list = apps
            .iter()
            .map(|(key, path, id, is_pinned, windows)| {
                // The app each window says it is, and that app's icon as
                // Windows' taskbar shows it (app_png): the button's, and a
                // hosted window's own (a store app's frame shows a plain
                // icon until its app is in it).
                let app_pngs: Vec<Option<String>> = windows.iter().map(|t| app_id_of(t.hwnd).and_then(|id| app_png(&id, size))).collect();
                let hosted = hosts(path);
                let windows: Vec<Value> = windows
                    .iter()
                    .zip(&app_pngs)
                    .map(|(t, app)| {
                        let icon = icon_of(t.hwnd, &redrawn);
                        let png = match pngs.get(&t.hwnd) {
                            Some((at, png)) if *at == icon => png.clone(),
                            _ => {
                                let png = if icon == 0 { String::new() } else { png_of(icon, size).unwrap_or_default() };
                                pngs.insert(t.hwnd, (icon, png.clone()));
                                png
                            }
                        };
                        let png = match app {
                            Some(app) if hosted || png.is_empty() => app.clone(),
                            _ => png,
                        };
                        let fills = fills_display(t.hwnd);
                        json!({
                            "id": t.hwnd,
                            "title": t.title,
                            "png": png,
                            "front": t.hwnd == front,
                            "min": t.min,
                            "max": fills && on_strip(t.hwnd),
                            "fills": fills,
                            "flash": flashing.contains(&t.hwnd),
                            "sessions": marks.get(&t.hwnd).cloned().unwrap_or_default(),
                            // The display it is on: its button on that display's taskbar (bars.rs).
                            "mon": display_of(t.hwnd),
                        })
                    })
                    .collect();
                // An app by its id: its name as the Start menu has it (not
                // its frame's program's, "Application Frame Host").
                let name = names.entry(key.to_lowercase()).or_insert_with(|| id.as_deref().and_then(app_title).unwrap_or_else(|| tasks::app_name(path))).clone();
                // As Windows' taskbar has it on a program's button: the
                // app's icon by the id its windows give (Settings' grey cog,
                // from its first moment); else the program's own icon (File
                // Explorer's for every folder, drive or This PC its windows
                // show); its window's where the program only hosts what it
                // shows (hosts) or has no icon of its own (Windows' plain
                // one), and while it has none.
                let window_png = windows.iter().find_map(|w| w["png"].as_str().filter(|p| !p.is_empty()).map(str::to_string));
                let file_png = file_pngs
                    .entry(path.to_lowercase())
                    .or_insert_with(|| {
                        let icon = tasks::file_icon(path);
                        let png = if icon == 0 { String::new() } else { png_of(icon, size).unwrap_or_default() };
                        tasks::destroy_icon(icon);
                        png
                    })
                    .clone();
                let own = !file_png.is_empty() && !hosts(path) && file_png != plain_png(size);
                // One kept by its id with no window: its app's icon too.
                let png = match app_pngs.into_iter().flatten().next().or_else(|| id.as_deref().and_then(|id| app_png(id, size))) {
                    Some(app) => app,
                    None if own => file_png,
                    None => window_png.unwrap_or(file_png),
                };
                json!({ "app": key, "path": path, "id": id, "name": name, "pinned": is_pinned, "png": png, "windows": windows })
            })
            .collect();
        *WIN_PNGS.lock().unwrap() = Some(pngs);
        *APP_NAMES.lock().unwrap() = Some(names);
        *FILE_PNGS.lock().unwrap() = Some(file_pngs);
        list
    };
    let list = Value::Array(list);
    let mut sent = SENT_WINDOWS.lock().unwrap();
    if *sent != list {
        *sent = list.clone();
        drop(sent);
        let _ = sh.app.emit_to("island", "taskbar:windows", crate::bars::for_main(sh, &list));
        crate::bars::refresh(sh);
    }
}

// The display a window is on (x, y, w, h).
pub(super) fn display_of(hwnd: isize) -> [i32; 4] {
    let mut info = MonitorInfo { size: std::mem::size_of::<MonitorInfo>() as u32, ..Default::default() };
    // SAFETY: our own struct, of the size the call writes; a gone window gives the nearest display.
    unsafe { GetMonitorInfoW(MonitorFromWindow(hwnd as Hwnd, MONITOR_DEFAULTTONEAREST), &mut info) };
    let m = &info.monitor;
    [m.left, m.top, m.right - m.left, m.bottom - m.top]
}

// The buttons as last made, each window with its display (bars.rs).
pub fn windows_now() -> Value {
    SENT_WINDOWS.lock().unwrap().clone()
}

// A press on a program's button kept on the taskbar with no window: it
// starts. Kept on the taskbar, or no longer.
// A program by its file, or an app (a store app) by its id: started as
// Windows starts it (its file would not start it), kept by its id.
// Another started (the middle button, Shift and a press; its menu's New
// window), or one run as administrator (Ctrl, Shift and a press, as on
// Windows' own taskbar, zhoupengjie/Wakuwaku#36): Windows asks first,
// and what cannot run so (most apps from the Store) is not started
// otherwise, only told in the log.
pub fn app(sh: &Shared, what: &str, path: &str, id: Option<&str>) -> bool {
    let done = match what {
        "launch" | "admin" => {
            let admin = what == "admin";
            close_flyout();
            // One taken from Windows' pins by its shortcut: started by it,
            // as Windows' taskbar starts it (what the shortcut adds, a
            // browser's profile), while it is there.
            let lnk = sh
                .config()
                .taskbar_pinned
                .as_array()
                .and_then(|all| all.iter().find(|p| p["path"].as_str().is_some_and(|p| p.eq_ignore_ascii_case(path)) && !path.is_empty()).and_then(|p| p["lnk"].as_str().map(str::to_string)))
                .filter(|l| std::path::Path::new(l).exists());
            let target = match (id, lnk) {
                (Some(id), _) => format!("shell:AppsFolder\\{id}"),
                (None, Some(lnk)) => lnk,
                (None, None) => path.to_string(),
            };
            // File Explorer opened where it is set to open, by its folder
            // (no explorer.exe, nor its shortcut, to go through).
            let explorer = (is_explorer(path) && !admin).then(explorer_opens);
            // On a thread of its own, with COM, as the shell wants: asked
            // on the main thread, the shell held it 80 to 400 ms on
            // Explorer's shortcut alone, and the press seemed to come
            // before the program (2026-10-12). Run as administrator, it
            // waits there for Windows' asking to be answered.
            let log_sh = crate::shared(&sh.app);
            crate::spawn_named("launch", move || {
                tasks::with_com();
                let started = Instant::now();
                if admin {
                    let answer = tasks::launch_as_admin(&target);
                    return log_sh.log(&format!("taskbar: started {target} as administrator in {} ms: {} ({answer})", started.elapsed().as_millis(), answer > 32));
                }
                let ok = match explorer {
                    Some(folder) => tasks::launch(folder) || tasks::launch(&target),
                    None => tasks::launch(&target),
                };
                log_sh.log(&format!("taskbar: started {} in {} ms: {ok}", explorer.unwrap_or(&target), started.elapsed().as_millis()));
            });
            true
        }
        "pin" | "unpin" => {
            let mut kept: Vec<Value> = sh.config().taskbar_pinned.as_array().cloned().unwrap_or_default();
            kept.retain(|p| match (id, p["id"].as_str()) {
                (Some(id), Some(theirs)) => !theirs.eq_ignore_ascii_case(id),
                (None, None) => !p["path"].as_str().is_some_and(|p| p.eq_ignore_ascii_case(path)),
                _ => true,
            });
            if what == "pin" {
                let name = id.and_then(app_title).unwrap_or_else(|| tasks::app_name(path));
                kept.push(match id {
                    Some(id) => json!({ "path": path, "id": id, "name": name }),
                    None => json!({ "path": path, "name": name }),
                });
            }
            crate::shared(&sh.app).change(json!({ "taskbarPinned": kept }));
            true
        }
        _ => false,
    };
    wake_tasks();
    done
}

// Windows' File Explorer, by its file (its pin's shortcut points there).
pub(super) fn is_explorer(path: &str) -> bool {
    path.rsplit(['\\', '/']).next().is_some_and(|name| name.eq_ignore_ascii_case("explorer.exe"))
}

// Where File Explorer opens, as set in its options (Open File Explorer
// to: LaunchTo, 1 This PC, 3 Downloads, else Home), by the folder's own
// name: opened by it in about 20 ms, its window up in 300; explorer.exe
// in 10 and 400; its shortcut in 80 to 400 and 350 to 700 (2026-10-12).
// Home is Windows 11's: where it is not, the launch fails and the
// program goes as before.
pub(super) fn explorer_opens() -> &'static str {
    use winreg::enums::HKEY_CURRENT_USER;
    let to = winreg::RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced")
        .and_then(|k| k.get_value::<u32, _>("LaunchTo"))
        .unwrap_or(2);
    match to {
        1 => "shell:::{20D04FE0-3AEA-1069-A2D8-08002B30309D}",
        3 => "shell:Downloads",
        _ => "shell:::{f874310e-b6b7-47dc-bc84-b9e6b38f5903}",
    }
}

#[cfg(test)]
mod launch_tests {
    #[test]
    fn file_explorer_by_its_file() {
        assert!(super::is_explorer(r"C:\Windows\explorer.exe"));
        assert!(super::is_explorer("c:/windows/EXPLORER.EXE"));
        assert!(!super::is_explorer(r"C:\Program Files\Internet Explorer\iexplore.exe"));
        assert!(!super::is_explorer(""));
        assert!(super::explorer_opens().starts_with("shell:"));
    }
}

// A press on a window's button: to the front, or minimized; or closed,
// or ended. To the front alone, never minimized: the next of a
// program's windows (Ctrl and a press, front), one picked from its list
// (front: the one in front too, picked to go to it; minimized, the
// window behind came up as if the press went through, the user,
// 2026-10-14), or one a file is dragged over (drag), which closes
// nothing first: Start in front (a file dragged from it) would be closed
// with Esc, and Esc ends the drag.
pub fn window(what: &str, hwnd: isize) -> bool {
    let done = match what {
        // Off the page's thread: a window may take its time to come up
        // or go down (tasks::press waits for it).
        "press" | "front" => {
            let front = what == "front";
            std::thread::spawn(move || {
                // Start closed for it: the window to the front, even the
                // one in front before Start (not minimized), as on Windows'.
                if front {
                    close_flyout();
                    let brought = crate::jump::bring_window(hwnd);
                    log(&format!("the next of its windows: {hwnd} brought {brought}"));
                } else if close_flyout() {
                    crate::jump::bring_window(hwnd);
                } else {
                    tasks::press(hwnd);
                }
                wake_tasks();
            });
            true
        }
        "drag" => {
            std::thread::spawn(move || {
                let brought = crate::jump::bring_window(hwnd);
                log(&format!("a file held over its button: {hwnd} brought {brought}"));
                wake_tasks();
            });
            true
        }
        "close" => tasks::close(hwnd),
        // Windows' End task (tasks::end_task), off the page's thread as
        // Windows does it off its own.
        "end" => {
            std::thread::spawn(move || {
                let ended = tasks::end_task(hwnd);
                log(&format!("end task {hwnd:#x}: {ended}"));
                wake_tasks();
            });
            true
        }
        _ => false,
    };
    wake_tasks();
    done
}

// A press on the button of a program with several windows: all of them
// to the front, or all minimized (tasks::press_all); off the page's
// thread, as window().
pub fn windows(hwnds: Vec<isize>) -> bool {
    std::thread::spawn(move || {
        let closed = close_flyout();
        tasks::press_all(&hwnds, !closed);
        wake_tasks();
    });
    true
}

// Shift and a right press on a program's button: its window's own menu,
// or one for all its windows in her words (tasks::window_menu), at the
// pointer; the calling thread held while it is open (the command's
// own, off the main one), so the page hears when it has gone.
pub fn window_menu(hwnds: Vec<isize>, lang: &'static str) -> bool {
    let words = ["taskbar.restoreAll", "taskbar.minimizeAll", "taskbar.closeAll"].map(|k| crate::i18n::t(lang, k));
    let shown = tasks::window_menu(&hwnds, [&words[0], &words[1], &words[2]]);
    let what = match &shown {
        Some((front, picked)) => format!("in front {front}, picked {picked:#x}"),
        None => "none to show".into(),
    };
    log(&format!("window menu for {} window(s): {what}", hwnds.len()));
    wake_tasks();
    shown.is_some()
}
