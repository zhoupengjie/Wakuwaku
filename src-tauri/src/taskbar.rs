// The taskbar home's side of Windows (display 'taskbar'): Windows' own
// taskbar put away while hers is up (shell.rs), and the tray's icons taken
// over from Explorer (systray.rs). On a thread of their own, which runs the
// message loop the tray's window needs, with a hidden window of its own that
// hears:
//   - Explorer started again (TaskbarCreated, from a new taskbar window):
//     its taskbar put away again, the strip taken again (island.rs), the
//     tray put first again and programs asked for their icons
//   - Explorer's taskbar shown (a WinEvent): put away again at once
//   - the tray's icons changing: drawn into PNGs and sent to the island's
//     page (taskbar:tray)
//   - windows coming, going, renamed, brought to the front, minimized, and
//     a window flashing for attention (the shell hook): the buttons' list
//     (tasks.rs) made again on a thread of its own and sent (taskbar:windows),
//     each with the sessions that run in it (jump.rs)
// Taken when the home takes its strip, given back when it gives the strip
// back (island.rs sync_bar) and on quitting. A killed process cannot give
// the taskbar back: shell.rs's guard does.
//
// Learnt in examples/taskbar_spike.rs (2026-10-10), where each of these was
// tried first.
use crate::Shared;

#[cfg(windows)]
mod imp {
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread::JoinHandle;
    use std::time::{Duration, Instant};

    use base64::Engine;
    use serde_json::{json, Value};
    use tauri::Emitter;

    use crate::systray::{self, Press};
    use crate::{jump, shell, tasks, Shared};

    type Hwnd = *mut c_void;

    #[repr(C)]
    struct WndClassExW {
        size: u32,
        style: u32,
        wndproc: extern "system" fn(Hwnd, u32, usize, isize) -> isize,
        cls_extra: i32,
        wnd_extra: i32,
        instance: isize,
        icon: isize,
        cursor: isize,
        background: isize,
        menu_name: *const u16,
        class_name: *const u16,
        icon_sm: isize,
    }

    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    struct Msg {
        hwnd: Hwnd,
        message: u32,
        wparam: usize,
        lparam: isize,
        time: u32,
        pt: Point,
        private: u32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    struct BitmapInfoHeader {
        size: u32,
        width: i32,
        height: i32,
        planes: u16,
        bit_count: u16,
        compression: u32,
        size_image: u32,
        x_ppm: i32,
        y_ppm: i32,
        clr_used: u32,
        clr_important: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct KeybdInput {
        vk: u16,
        scan: u16,
        flags: u32,
        time: u32,
        extra: usize,
    }

    // The union's largest member (MOUSEINPUT), so INPUT has the size Windows expects.
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct MouseInput {
        dx: i32,
        dy: i32,
        data: u32,
        flags: u32,
        time: u32,
        extra: usize,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    union InputUnion {
        mi: MouseInput,
        ki: KeybdInput,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Input {
        kind: u32,
        u: InputUnion,
    }

    type WinEventProc = extern "system" fn(isize, u32, Hwnd, i32, i32, u32, u32);

    #[link(name = "user32")]
    extern "system" {
        fn RegisterClassExW(class: *const WndClassExW) -> u16;
        fn CreateWindowExW(ex: u32, class: *const u16, title: *const u16, style: u32, x: i32, y: i32, w: i32, h: i32, parent: Hwnd, menu: isize, instance: isize, param: *mut c_void) -> Hwnd;
        fn DestroyWindow(hwnd: Hwnd) -> i32;
        fn DefWindowProcW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize;
        fn GetMessageW(msg: *mut Msg, hwnd: Hwnd, min: u32, max: u32) -> i32;
        fn TranslateMessage(msg: *const Msg) -> i32;
        fn DispatchMessageW(msg: *const Msg) -> isize;
        fn PostMessageW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> i32;
        fn PostQuitMessage(code: i32);
        fn SetTimer(hwnd: Hwnd, id: usize, ms: u32, func: *const c_void) -> usize;
        fn KillTimer(hwnd: Hwnd, id: usize) -> i32;
        fn SetWinEventHook(min: u32, max: u32, module: isize, func: WinEventProc, pid: u32, tid: u32, flags: u32) -> isize;
        fn UnhookWinEvent(hook: isize) -> i32;
        fn GetClassNameW(hwnd: Hwnd, name: *mut u16, max: i32) -> i32;
        fn IsWindowVisible(hwnd: Hwnd) -> i32;
        fn SystemParametersInfoW(action: u32, param: u32, pv: *mut c_void, flags: u32) -> i32;
        fn SendInput(count: u32, inputs: *const Input, size: i32) -> u32;
        fn SetForegroundWindow(hwnd: Hwnd) -> i32;
        fn GetCursorPos(pt: *mut Point) -> i32;
        fn DrawIconEx(hdc: isize, x: i32, y: i32, icon: isize, w: i32, h: i32, step: u32, brush: isize, flags: u32) -> i32;
        fn RegisterShellHookWindow(hwnd: Hwnd) -> i32;
        fn DeregisterShellHookWindow(hwnd: Hwnd) -> i32;
        fn RegisterWindowMessageW(name: *const u16) -> u32;
    }

    #[link(name = "gdi32")]
    extern "system" {
        fn CreateCompatibleDC(hdc: isize) -> isize;
        fn CreateDIBSection(hdc: isize, info: *const BitmapInfoHeader, usage: u32, bits: *mut *mut u8, section: isize, offset: u32) -> isize;
        fn SelectObject(hdc: isize, obj: isize) -> isize;
        fn DeleteObject(obj: isize) -> i32;
        fn DeleteDC(hdc: isize) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleW(name: *const u16) -> isize;
    }

    const WM_TIMER: u32 = 0x0113;
    // Explorer's taskbar seen shown (the hook), lParam its window; the tray's
    // icons changed (systray.rs); the thread asked to end.
    const WM_TASKBAR_SHOWN: u32 = 0x8001;
    const WM_TRAY_CHANGED: u32 = 0x8002;
    const WM_STOP: u32 = 0x8003;
    const WS_POPUP: u32 = 0x8000_0000;
    const WS_EX_TOOLWINDOW: u32 = 0x80;
    const SPI_GETWORKAREA: u32 = 0x30;
    const SPI_SETWORKAREA: u32 = 0x2F;

    const EVENT_OBJECT_SHOW: u32 = 0x8002;
    // What changes the buttons: a window made, shown, hidden, gone (0x8001
    // to 0x8003), renamed, cloaked or not (another desktop), brought to the
    // front, minimized or restored.
    const WINDOW_EVENTS: [(u32, u32); 5] = [(0x8001, 0x8003), (0x800C, 0x800C), (0x8017, 0x8018), (0x0003, 0x0003), (0x0016, 0x0017)];
    const OBJID_WINDOW: i32 = 0;
    // The shell hook's: a window flashing for attention, one activated.
    const HSHELL_FLASH: usize = 0x8006;
    const HSHELL_WINDOWACTIVATED: usize = 4;
    const HSHELL_RUDEAPPACTIVATED: usize = 0x8004;
    const KEYEVENTF_EXTENDEDKEY: u32 = 0x1;
    const KEYEVENTF_KEYUP: u32 = 0x2;
    const VK_LWIN: u16 = 0x5B;
    const VK_TAB: u16 = 0x09;
    const VK_SPACE: u16 = 0x20;
    const DI_NORMAL: u32 = 3;
    // The ticks (half seconds) to wait before asking programs for their icons.
    const SETTLE_TICKS: u32 = 3;

    struct Host {
        hwnd: isize,
        thread: JoinHandle<()>,
    }

    static HOST: Mutex<Option<Host>> = Mutex::new(None);
    static SH: Mutex<Option<Arc<Shared>>> = Mutex::new(None);
    static OURS: AtomicIsize = AtomicIsize::new(0);
    // Explorer's taskbar window: a new one means Explorer started again.
    static EXPLORER: AtomicIsize = AtomicIsize::new(0);
    static CREATED_MSG: AtomicU32 = AtomicU32::new(0);
    static TICKS: AtomicU32 = AtomicU32::new(0);
    static WORK_BOTTOM: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
    // The strip as granted (physical); since when (tick) the work area has
    // reached into it, and when it was last taken again for that.
    static STRIP: Mutex<Option<(i32, i32, i32, i32)>> = Mutex::new(None);
    static LOST_SINCE: AtomicU32 = AtomicU32::new(0);
    static LAST_TAKEN: AtomicU32 = AtomicU32::new(0);
    // Asking programs for their icons: from this tick (0: asked), the last
    // time, and how many times right after one.
    static ASK_AT: AtomicU32 = AtomicU32::new(0);
    static LAST_ASK: AtomicU32 = AtomicU32::new(0);
    static QUICK_ASKS: AtomicU32 = AtomicU32::new(0);
    // Each icon drawn: (key, its handle) to its PNG; the list sent last.
    static PNGS: Mutex<Option<HashMap<u64, (isize, String)>>> = Mutex::new(None);
    static SENT: Mutex<Value> = Mutex::new(Value::Null);
    // The buttons: their thread, woken when windows change; their order (as
    // they came); the windows flashing; each window's icon drawn (its
    // handle, its PNG); the sessions in each window; the list sent last.
    static TASKS: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);
    static WAKE: (Mutex<bool>, std::sync::Condvar) = (Mutex::new(false), std::sync::Condvar::new());
    static TASKS_STOP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    static ORDER: Mutex<Vec<isize>> = Mutex::new(Vec::new());
    static FLASHING: Mutex<Vec<isize>> = Mutex::new(Vec::new());
    static WIN_PNGS: Mutex<Option<HashMap<isize, (isize, String)>>> = Mutex::new(None);
    static MARKS: Mutex<Option<HashMap<isize, Vec<Value>>>> = Mutex::new(None);
    static SENT_WINDOWS: Mutex<Value> = Mutex::new(Value::Null);
    // Each program's name (tasks::app_name), and its file's icon drawn, by its path.
    static APP_NAMES: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);
    static FILE_PNGS: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);
    static SHELL_MSG: AtomicU32 = AtomicU32::new(0);
    // The keyboard's thread, and what it sent last.
    static KEYS: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);
    static SENT_KEYS: Mutex<Value> = Mutex::new(Value::Null);
    // Windows' own record of the tray icons it keeps out (windows_kept), read once.
    static KEPT: Mutex<Option<Vec<(String, Option<u32>, bool)>>> = Mutex::new(None);

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn sh() -> Option<Arc<Shared>> {
        SH.lock().unwrap().clone()
    }

    fn log(line: &str) {
        if let Some(sh) = sh() {
            sh.log(&format!("taskbar: {line}"));
        }
    }

    fn work_bottom() -> i32 {
        let mut r = Rect::default();
        // SAFETY: our own struct, of the size the call writes.
        unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut r as *mut Rect as *mut c_void, 0) };
        r.bottom
    }

    pub fn is_up() -> bool {
        HOST.lock().unwrap().is_some()
    }

    // The main display's work area's bottom (physical), for the log.
    pub fn work_area_bottom() -> i32 {
        work_bottom()
    }

    // Windows' taskbar put away and the tray taken, before the strip is
    // taken (once the work area has lost the taskbar's room, so the strip
    // goes to the bottom and not above its room). mon_bottom: the display's
    // bottom, physical.
    pub fn take(sh: &Shared, strip: (i32, i32, i32, i32), mon_bottom: i32) {
        if is_up() {
            return;
        }
        *SH.lock().unwrap() = Some(crate::shared(&sh.app));
        let file = sh.dir.join("taskbar.json");
        if let Err(e) = shell::hide(&file) {
            sh.log(&format!("taskbar: could not write {}: {e}", file.display()));
            return;
        }
        sh.log(&format!("taskbar: Windows' put away; guard started: {}", shell::spawn_guard(&file)));
        let t = Instant::now();
        while work_bottom() != mon_bottom && t.elapsed() < Duration::from_millis(3000) {
            std::thread::sleep(Duration::from_millis(20));
        }
        sh.log(&format!("taskbar: work area free after {} ms", t.elapsed().as_millis()));
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || run(strip, ready_tx));
        match ready_rx.recv_timeout(Duration::from_secs(3)) {
            Ok(hwnd) => *HOST.lock().unwrap() = Some(Host { hwnd, thread }),
            Err(_) => sh.log("taskbar: its thread did not start"),
        }
        TASKS_STOP.store(false, Ordering::SeqCst);
        let sh = crate::shared(&sh.app);
        let keys_sh = sh.clone();
        *TASKS.lock().unwrap() = Some(std::thread::spawn(move || tasks_loop(sh)));
        *KEYS.lock().unwrap() = Some(std::thread::spawn(move || keys_loop(keys_sh)));
    }

    // The keyboard (tasks::keys) looked at four times a second; sent when it changes.
    fn keys_loop(sh: Arc<Shared>) {
        let mut sent = None;
        while !TASKS_STOP.load(Ordering::SeqCst) {
            let keys = tasks::keys();
            if sent.as_ref() != Some(&keys) {
                let _ = sh.app.emit_to("island", "taskbar:keys", json!({ "lang": keys.lang, "native": keys.native, "caps": keys.caps }));
                *SENT_KEYS.lock().unwrap() = json!({ "lang": keys.lang, "native": keys.native, "caps": keys.caps });
                sent = Some(keys);
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    // The tray handed back to Explorer and its taskbar shown again.
    pub fn give_back(sh: &Shared) {
        TASKS_STOP.store(true, Ordering::SeqCst);
        if let Some(thread) = TASKS.lock().unwrap().take() {
            wake_tasks();
            let _ = thread.join();
        }
        if let Some(thread) = KEYS.lock().unwrap().take() {
            let _ = thread.join();
        }
        let host = HOST.lock().unwrap().take();
        if let Some(host) = host {
            // SAFETY: the thread's own window; a posted message, no pointers.
            unsafe { PostMessageW(host.hwnd as Hwnd, WM_STOP, 0, 0) };
            let _ = host.thread.join();
        }
        let file = sh.dir.join("taskbar.json");
        if file.exists() {
            sh.log(&format!("taskbar: Windows' given back: {}", shell::restore(&file)));
        }
        *SENT.lock().unwrap() = Value::Null;
        *SENT_WINDOWS.lock().unwrap() = Value::Null;
        *STRIP.lock().unwrap() = None;
        ORDER.lock().unwrap().clear();
        FLASHING.lock().unwrap().clear();
        *WIN_PNGS.lock().unwrap() = None;
        *MARKS.lock().unwrap() = None;
    }

    // --- The buttons ----------------------------------------------------------------

    fn wake_tasks() {
        let (dirty, cv) = &WAKE;
        *dirty.lock().unwrap() = true;
        cv.notify_one();
    }

    // The buttons' list kept: made again when windows change (a moment
    // after, so a burst of changes makes it once) and every two seconds;
    // the sessions' windows looked for every three (it looks through the
    // processes).
    fn tasks_loop(sh: Arc<Shared>) {
        let mut marked_at = Instant::now() - Duration::from_secs(10);
        loop {
            {
                let (dirty, cv) = &WAKE;
                let guard = dirty.lock().unwrap();
                let (mut guard, _) = cv.wait_timeout_while(guard, Duration::from_secs(2), |d| !*d).unwrap();
                *guard = false;
            }
            if TASKS_STOP.load(Ordering::SeqCst) {
                return;
            }
            std::thread::sleep(Duration::from_millis(120));
            *WAKE.0.lock().unwrap() = false;
            if marked_at.elapsed() >= Duration::from_secs(3) {
                marked_at = Instant::now();
                mark_sessions(&sh);
            }
            send_windows(&sh);
        }
    }

    // Each session with a window of its own to go to, by that window.
    fn mark_sessions(sh: &Shared) {
        let targets: Vec<(String, String, jump::Chain, Vec<String>)> = {
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
        let mut marks: HashMap<isize, Vec<Value>> = HashMap::new();
        for (id, mood, chain, hints) in targets {
            let hints: Vec<&str> = hints.iter().map(String::as_str).collect();
            if let Some(hwnd) = jump::window_for(&chain, &hints) {
                marks.entry(hwnd).or_default().push(json!({ "id": id, "mood": mood }));
            }
        }
        *MARKS.lock().unwrap() = Some(marks);
    }

    // The buttons as the page draws them: one per program, as Windows' 11
    // has them by default. The programs kept on the taskbar first, in the
    // order they were kept (taskbarPinned: [{ path, name }]), whether they run
    // or not; then the others in the order their first windows came. Each
    // with its windows (each with the sessions in it), its name, its icon (its
    // first window's, else its file's).
    fn send_windows(sh: &Shared) {
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
        let flashing = {
            let mut flashing = FLASHING.lock().unwrap();
            flashing.retain(|h| *h != front && tasks::alive(*h));
            flashing.clone()
        };
        let size = (16.0 * sh.screens().primary.map_or(1.0, |a| a.sf)).round() as i32;
        let marks = MARKS.lock().unwrap().clone().unwrap_or_default();
        let pinned: Vec<(String, String)> = sh
            .setting("taskbarPinned")
            .as_array()
            .map(|all| all.iter().filter_map(|p| Some((p["path"].as_str()?.to_string(), p["name"].as_str().unwrap_or("").to_string()))).collect())
            .unwrap_or_default();
        // (path, pinned, windows), the paths' case aside.
        let mut apps: Vec<(String, bool, Vec<&tasks::Task>)> = pinned.iter().map(|(p, _)| (p.clone(), true, Vec::new())).collect();
        for t in order.iter().filter_map(|h| found.iter().find(|t| t.hwnd == *h)) {
            match apps.iter_mut().find(|a| a.0.eq_ignore_ascii_case(&t.path)) {
                Some(app) => app.2.push(t),
                None => apps.push((t.path.clone(), false, vec![t])),
            }
        }
        let list: Vec<Value> = {
            let mut pngs = WIN_PNGS.lock().unwrap();
            let pngs = pngs.get_or_insert_with(HashMap::new);
            pngs.retain(|h, _| order.contains(h));
            let mut names = APP_NAMES.lock().unwrap();
            let names = names.get_or_insert_with(HashMap::new);
            let mut file_pngs = FILE_PNGS.lock().unwrap();
            let file_pngs = file_pngs.get_or_insert_with(HashMap::new);
            apps.iter()
                .map(|(path, is_pinned, windows)| {
                    let windows: Vec<Value> = windows
                        .iter()
                        .map(|t| {
                            let icon = tasks::icon_of(t.hwnd);
                            let png = match pngs.get(&t.hwnd) {
                                Some((at, png)) if *at == icon => png.clone(),
                                _ => {
                                    let png = if icon == 0 { String::new() } else { png_of(icon, size).unwrap_or_default() };
                                    pngs.insert(t.hwnd, (icon, png.clone()));
                                    png
                                }
                            };
                            json!({
                                "id": t.hwnd,
                                "title": t.title,
                                "png": png,
                                "front": t.hwnd == front,
                                "min": t.min,
                                "flash": flashing.contains(&t.hwnd),
                                "sessions": marks.get(&t.hwnd).cloned().unwrap_or_default(),
                            })
                        })
                        .collect();
                    let key = path.to_lowercase();
                    let name = names.entry(key.clone()).or_insert_with(|| tasks::app_name(path)).clone();
                    let png = windows.iter().find_map(|w| w["png"].as_str().filter(|p| !p.is_empty()).map(str::to_string)).unwrap_or_else(|| {
                        file_pngs
                            .entry(key.clone())
                            .or_insert_with(|| {
                                let icon = tasks::file_icon(path);
                                let png = if icon == 0 { String::new() } else { png_of(icon, size).unwrap_or_default() };
                                tasks::destroy_icon(icon);
                                png
                            })
                            .clone()
                    });
                    json!({ "app": key, "path": path, "name": name, "pinned": is_pinned, "png": png, "windows": windows })
                })
                .collect()
        };
        let list = Value::Array(list);
        let mut sent = SENT_WINDOWS.lock().unwrap();
        if *sent != list {
            *sent = list.clone();
            let _ = sh.app.emit_to("island", "taskbar:windows", list);
        }
    }

    // A press on a program's button kept on the taskbar with no window: it
    // starts. Kept on the taskbar, or no longer.
    pub fn app(sh: &Shared, what: &str, path: &str) -> bool {
        let done = match what {
            "launch" => tasks::launch(path),
            "pin" | "unpin" => {
                let mut kept: Vec<Value> = sh.setting("taskbarPinned").as_array().cloned().unwrap_or_default();
                kept.retain(|p| !p["path"].as_str().is_some_and(|p| p.eq_ignore_ascii_case(path)));
                if what == "pin" {
                    kept.push(json!({ "path": path, "name": tasks::app_name(path) }));
                }
                crate::shared(&sh.app).change(json!({ "taskbarPinned": kept }));
                true
            }
            _ => false,
        };
        wake_tasks();
        done
    }

    // A press on a window's button: to the front, or minimized; or closed.
    pub fn window(what: &str, hwnd: isize) -> bool {
        let done = match what {
            "press" => tasks::press(hwnd),
            "close" => tasks::close(hwnd),
            _ => false,
        };
        wake_tasks();
        done
    }

    // Where the strip is now (physical): ours of the taskbar's class goes
    // there too, for those who ask the taskbar's window where it is.
    pub fn strip_moved(strip: (i32, i32, i32, i32)) {
        *STRIP.lock().unwrap() = Some(strip);
        if is_up() {
            systray::place(strip);
        }
    }

    // The strip taken again (Explorer started again and forgot it), off
    // this thread: the island's side waits on the main thread, which may be
    // waiting on this one (its app bar's messages come to the tray's window,
    // here).
    fn take_strip_again() {
        if let Some(sh) = sh() {
            std::thread::spawn(move || crate::island::take_strip_again(&sh));
        }
    }

    // The work area's bottom set where the strip starts, as Explorer sets
    // it for a bar it counts. It does not always count ours: put away in a
    // moment (its work area free at once, not after its slide), Windows'
    // taskbar left the work area the whole display, our strip taken or not,
    // even taken again, even with it set to hide itself afresh (2026-10-10).
    // Set quietly: told to every window, Explorer set it back at once.
    fn set_work_bottom(bottom: i32) {
        let mut r = Rect::default();
        // SAFETY: our own struct, read and handed back.
        let ok = unsafe {
            SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut r as *mut Rect as *mut c_void, 0);
            r.bottom = bottom;
            SystemParametersInfoW(SPI_SETWORKAREA, 0, &mut r as *mut Rect as *mut c_void, 0)
        };
        let error = std::io::Error::last_os_error();
        let now = work_bottom();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            log(&format!("work area set: {ok} ({error}); bottom then {now}, 150 ms after {}", work_bottom()));
        });
    }

    // The tray's icons and the buttons as last sent, for a page that has just come up.
    pub fn resend(sh: &Shared) {
        let sent = SENT.lock().unwrap().clone();
        if !sent.is_null() {
            let _ = sh.app.emit_to("island", "taskbar:tray", sent);
        }
        let windows = SENT_WINDOWS.lock().unwrap().clone();
        if !windows.is_null() {
            let _ = sh.app.emit_to("island", "taskbar:windows", windows);
        }
        let keys = SENT_KEYS.lock().unwrap().clone();
        if !keys.is_null() {
            let _ = sh.app.emit_to("island", "taskbar:keys", keys);
        }
    }

    // A press on a tray icon in the page, told to its program. The press
    // itself brings the home's window to the front first, as the taskbar
    // comes there when pressed, so it may hand the front on (a menu not in
    // front does not close when you press elsewhere).
    pub fn press(sh: &Shared, key: u64, press: &str, home_hwnd: isize) -> bool {
        let press = match press {
            "leftDown" => Press::LeftDown,
            "leftUp" => Press::LeftUp,
            "rightDown" => Press::RightDown,
            "rightUp" => Press::RightUp,
            "double" => Press::Double,
            "move" => Press::Move,
            "in" => Press::HoverIn,
            "out" => Press::HoverOut,
            _ => return false,
        };
        if matches!(press, Press::LeftDown | Press::RightDown) && home_hwnd != 0 {
            // SAFETY: our own window; the press is the input that lets it.
            unsafe { SetForegroundWindow(home_hwnd as Hwnd) };
        }
        let mut at = Point::default();
        // SAFETY: our own out-parameter.
        unsafe { GetCursorPos(&mut at) };
        let told = systray::tell(key, press, (at.x, at.y));
        for line in systray::notes() {
            sh.log(&format!("taskbar: {line}"));
        }
        told
    }

    // What the taskbar's own buttons open, by the keys that open them.
    pub fn open(what: &str) -> bool {
        // The input method of the window in front: its own script or plain letters.
        if what == "ime" {
            return tasks::toggle_native();
        }
        let keys: &[u16] = match what {
            "start" => &[VK_LWIN],
            "search" => &[VK_LWIN, b'S' as u16],
            "tasks" => &[VK_LWIN, VK_TAB],
            "widgets" => &[VK_LWIN, b'W' as u16],
            "desktop" => &[VK_LWIN, b'D' as u16],
            "quick" => &[VK_LWIN, b'A' as u16],
            "notifications" => &[VK_LWIN, b'N' as u16],
            // The input methods to pick from, as Win+Space has them.
            "inputs" => &[VK_LWIN, VK_SPACE],
            _ => return false,
        };
        let key = |vk: u16, up: bool| Input {
            kind: 1,
            u: InputUnion { ki: KeybdInput { vk, scan: 0, flags: if vk == VK_LWIN { KEYEVENTF_EXTENDEDKEY } else { 0 } | if up { KEYEVENTF_KEYUP } else { 0 }, time: 0, extra: 0 } },
        };
        let inputs: Vec<Input> = keys.iter().map(|&k| key(k, false)).chain(keys.iter().rev().map(|&k| key(k, true))).collect();
        // SAFETY: a slice of INPUTs of the size passed.
        unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<Input>() as i32) == inputs.len() as u32 }
    }

    fn run(strip: (i32, i32, i32, i32), ready: std::sync::mpsc::Sender<isize>) {
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
            CREATED_MSG.store(shell::taskbar_created(), Ordering::SeqCst);
            EXPLORER.store(shell::taskbars().first().copied().unwrap_or(0), Ordering::SeqCst);
            let hooks: Vec<isize> = WINDOW_EVENTS.iter().map(|&(min, max)| SetWinEventHook(min, max, 0, on_event, 0, 0, 0)).collect();
            SHELL_MSG.store(RegisterWindowMessageW(wide("SHELLHOOK").as_ptr()), Ordering::SeqCst);
            let shell_hook = RegisterShellHookWindow(hwnd) != 0;
            SetTimer(hwnd, 1, 500, std::ptr::null());
            TICKS.store(0, Ordering::SeqCst);
            let tray = systray::start(strip, hwnd as isize, WM_TRAY_CHANGED);
            ASK_AT.store(SETTLE_TICKS, Ordering::SeqCst);
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
            KillTimer(hwnd, 1);
            log(&format!("tray handed back; it was handed {}", systray::seen()));
            systray::stop();
            DestroyWindow(hwnd);
            OURS.store(0, Ordering::SeqCst);
            *PNGS.lock().unwrap() = None;
        }
    }

    // A window changed: the buttons are made again (their thread, woken);
    // Explorer's taskbar shown: put away again.
    extern "system" fn on_event(_hook: isize, event: u32, hwnd: Hwnd, object: i32, child: i32, _thread: u32, _time: u32) {
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
        wake_tasks();
    }

    // The shell hook: a window flashing for attention (until it is in
    // front), one activated.
    fn on_shell(what: usize, hwnd: isize) {
        match what {
            HSHELL_FLASH => {
                let mut flashing = FLASHING.lock().unwrap();
                if !flashing.contains(&hwnd) {
                    flashing.push(hwnd);
                }
            }
            HSHELL_WINDOWACTIVATED | HSHELL_RUDEAPPACTIVATED => FLASHING.lock().unwrap().retain(|h| *h != hwnd),
            _ => return,
        }
        wake_tasks();
    }

    fn put_away_again(how: &str) {
        shell::rehide();
        log(&format!("Windows' shown again ({how}): put away"));
        if systray::keep_first() {
            log("tray: Explorer's window had come first; ours put ahead again");
            ask_soon();
        }
    }

    // Explorer's window came first for a moment: what programs handed the
    // tray then went to it. They are asked again once it settles: at most
    // every ten seconds, but right away (twice at most) when it came first
    // while they were answering.
    fn ask_soon() {
        let (ticks, last) = (TICKS.load(Ordering::SeqCst), LAST_ASK.load(Ordering::SeqCst));
        if ASK_AT.load(Ordering::SeqCst) != 0 {
            return;
        }
        let answering = ticks <= last + 6;
        if !answering {
            QUICK_ASKS.store(0, Ordering::SeqCst);
        }
        if (answering && QUICK_ASKS.fetch_add(1, Ordering::SeqCst) < 2) || ticks >= last + 20 {
            ASK_AT.store(ticks + SETTLE_TICKS, Ordering::SeqCst);
        }
    }

    fn tick() {
        if shell::visible() {
            put_away_again("seen by the timer");
        }
        // The work area reaching into the strip: Windows has not counted it.
        // Its bottom is set where the strip starts (after half a second of
        // it, at most every two seconds, should something set it back).
        let bottom = work_bottom();
        if bottom != WORK_BOTTOM.swap(bottom, Ordering::SeqCst) {
            log(&format!("work area's bottom now {bottom}"));
        }
        let strip = *STRIP.lock().unwrap();
        let lost = strip.is_some_and(|s| bottom > s.1);
        let ticks = TICKS.load(Ordering::SeqCst);
        if !lost {
            LOST_SINCE.store(0, Ordering::SeqCst);
        } else if LOST_SINCE.load(Ordering::SeqCst) == 0 {
            LOST_SINCE.store(ticks.max(1), Ordering::SeqCst);
        } else if ticks >= LOST_SINCE.load(Ordering::SeqCst) + 1 && ticks >= LAST_TAKEN.load(Ordering::SeqCst) + 4 {
            let top = strip.map_or(bottom, |s| s.1);
            log(&format!("work area's bottom {bottom} is in the strip ({strip:?}): set to {top}"));
            LAST_TAKEN.store(ticks, Ordering::SeqCst);
            LOST_SINCE.store(0, Ordering::SeqCst);
            set_work_bottom(top);
        }
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
        }
    }

    // Explorer started again (a new taskbar window), or a program's asking
    // (the same one: ours, or another tray's).
    fn on_created() {
        let now = shell::taskbars().first().copied().unwrap_or(0);
        if now == EXPLORER.swap(now, Ordering::SeqCst) {
            return;
        }
        log("Explorer started again");
        shell::rehide();
        take_strip_again();
        systray::keep_first();
        ask_soon();
    }

    // Where Windows' own taskbar keeps each tray icon: Control Panel\
    // NotifyIconSettings, a key per icon with its program's path, its uID
    // and IsPromoted 1 when kept out on the taskbar (folded away otherwise).
    // (file name, uID, kept out), read once while the taskbar is up.
    fn windows_kept() -> Vec<(String, Option<u32>, bool)> {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;
        let mut kept = KEPT.lock().unwrap();
        if let Some(list) = kept.as_ref() {
            return list.clone();
        }
        let list: Vec<(String, Option<u32>, bool)> = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(r"Control Panel\NotifyIconSettings")
            .map(|root| {
                root.enum_keys()
                    .flatten()
                    .filter_map(|name| {
                        let key = root.open_subkey(&name).ok()?;
                        let path: String = key.get_value("ExecutablePath").ok()?;
                        let file = path.rsplit('\\').next().unwrap_or(&path).to_lowercase();
                        let promoted: u32 = key.get_value("IsPromoted").unwrap_or(0);
                        Some((file, key.get_value("UID").ok(), promoted != 0))
                    })
                    .collect()
            })
            .unwrap_or_default();
        *kept = Some(list.clone());
        list
    }

    // Whether Windows keeps this icon out on its taskbar: its program's, by
    // its uID when the program has several.
    fn kept_out(kept: &[(String, Option<u32>, bool)], exe: &str, uid: u32) -> bool {
        let exe = exe.to_lowercase();
        let its: Vec<&(String, Option<u32>, bool)> = kept.iter().filter(|k| k.0 == exe).collect();
        its.iter().find(|k| k.1 == Some(uid)).or(its.first()).is_some_and(|k| k.2)
    }

    // The icons as the page draws them: each drawn once per handle.
    fn send_icons() {
        let Some(sh) = sh() else { return };
        for line in systray::notes() {
            sh.log(&format!("taskbar: {line}"));
        }
        let size = (16.0 * sh.screens().primary.map_or(1.0, |a| a.sf)).round() as i32;
        let shown = systray::shown();
        let kept = windows_kept();
        let list: Vec<Value> = {
            let mut pngs = PNGS.lock().unwrap();
            let pngs = pngs.get_or_insert_with(HashMap::new);
            pngs.retain(|k, _| shown.iter().any(|s| s.key == *k));
            shown
                .iter()
                .map(|s| {
                    let png = if s.icon == 0 {
                        String::new()
                    } else {
                        match pngs.get(&s.key) {
                            Some((h, png)) if *h == s.icon => png.clone(),
                            _ => {
                                let png = png_of(s.icon, size).unwrap_or_default();
                                pngs.insert(s.key, (s.icon, png.clone()));
                                png
                            }
                        }
                    };
                    json!({ "key": s.key, "png": png, "tip": s.tip, "exe": s.exe, "name": s.name, "windowsOut": kept_out(&kept, &s.exe, s.uid) })
                })
                .collect()
        };
        let list = Value::Array(list);
        *SENT.lock().unwrap() = list.clone();
        let _ = sh.app.emit_to("island", "taskbar:tray", list);
    }

    // An icon drawn as a PNG (a data: URL), size px across. Drawn twice, on
    // black and on white: how much the white shows through is how clear each
    // pixel is, for icons with an alpha channel and those with a mask alike.
    fn png_of(icon: isize, size: i32) -> Option<String> {
        if size <= 0 {
            return None;
        }
        let n = (size * size) as usize;
        let draw = |fill: u8| -> Option<Vec<u8>> {
            // SAFETY: a DIB section of our own, drawn into and read, then deleted.
            unsafe {
                let dc = CreateCompatibleDC(0);
                let info = BitmapInfoHeader { size: 40, width: size, height: -size, planes: 1, bit_count: 32, compression: 0, size_image: 0, x_ppm: 0, y_ppm: 0, clr_used: 0, clr_important: 0 };
                let mut bits: *mut u8 = std::ptr::null_mut();
                let bmp = CreateDIBSection(dc, &info, 0, &mut bits, 0, 0);
                if bmp == 0 || bits.is_null() {
                    DeleteDC(dc);
                    return None;
                }
                let old = SelectObject(dc, bmp);
                std::ptr::write_bytes(bits, fill, n * 4);
                DrawIconEx(dc, 0, 0, icon, size, size, 0, 0, DI_NORMAL);
                let pixels = std::slice::from_raw_parts(bits, n * 4).to_vec();
                SelectObject(dc, old);
                DeleteObject(bmp);
                DeleteDC(dc);
                Some(pixels)
            }
        };
        let (black, white) = (draw(0)?, draw(255)?);
        let mut rgba = vec![0u8; n * 4];
        for i in 0..n {
            let (b, w) = (&black[i * 4..i * 4 + 3], &white[i * 4..i * 4 + 3]);
            let through = (0..3).map(|c| w[c].saturating_sub(b[c])).max().unwrap_or(255);
            let alpha = 255 - through;
            if alpha == 0 {
                continue;
            }
            // On black, a pixel is its colour times its alpha (BGR to RGB).
            for (c, from) in [(0, 2), (1, 1), (2, 0)] {
                rgba[i * 4 + c] = ((b[from] as u32 * 255 + alpha as u32 / 2) / alpha as u32).min(255) as u8;
            }
            rgba[i * 4 + 3] = alpha;
        }
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, size as u32, size as u32);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().ok()?;
            writer.write_image_data(&rgba).ok()?;
        }
        Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(out)))
    }

    extern "system" fn wndproc(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize {
        match msg {
            WM_TIMER => tick(),
            WM_TASKBAR_SHOWN => {
                // SAFETY: a plain check of Explorer's window.
                if unsafe { IsWindowVisible(lparam as Hwnd) } != 0 {
                    put_away_again("its show event");
                }
            }
            WM_TRAY_CHANGED => send_icons(),
            // SAFETY: ending this thread's loop.
            WM_STOP => unsafe { PostQuitMessage(0) },
            _ if msg == CREATED_MSG.load(Ordering::SeqCst) && msg != 0 => on_created(),
            _ if msg == SHELL_MSG.load(Ordering::SeqCst) && msg != 0 => on_shell(wparam, lparam),
            // SAFETY: the default for the rest.
            _ => return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
        0
    }
}

#[cfg(not(windows))]
mod imp {
    use crate::Shared;

    pub fn is_up() -> bool {
        false
    }
    pub fn take(_sh: &Shared, _strip: (i32, i32, i32, i32), _mon_bottom: i32) {}
    pub fn give_back(_sh: &Shared) {}
    pub fn strip_moved(_strip: (i32, i32, i32, i32)) {}
    pub fn resend(_sh: &Shared) {}
    pub fn press(_sh: &Shared, _key: u64, _press: &str, _home_hwnd: isize) -> bool {
        false
    }
    pub fn open(_what: &str) -> bool {
        false
    }
    pub fn window(_what: &str, _hwnd: isize) -> bool {
        false
    }
    pub fn app(_sh: &Shared, _what: &str, _path: &str) -> bool {
        false
    }
    pub fn work_area_bottom() -> i32 {
        0
    }
}

pub use imp::{give_back, resend, strip_moved, take, work_area_bottom};

// At start: Windows' taskbar left put away by a run that is gone (its guard
// gone too) is given back.
pub fn recover(sh: &Shared) {
    if crate::shell::recover(&sh.dir.join("taskbar.json")) {
        sh.log("taskbar: Windows' taskbar was left put away: given back");
    }
}

// From the page: a press on a tray icon (taskbar_tray), a button of the
// taskbar's own (taskbar_open).
#[tauri::command]
pub fn taskbar_tray(app: tauri::AppHandle, key: u64, press: String) -> bool {
    let sh = crate::shared(&app);
    imp::press(&sh, key, &press, crate::island::hwnd(&sh))
}

#[tauri::command]
pub fn taskbar_open(what: String) -> bool {
    imp::open(&what)
}

// A press on a window's button (press: to the front or minimized; close).
#[tauri::command]
pub fn taskbar_window(id: i64, what: String) -> bool {
    imp::window(&what, id as isize)
}

// A program's button: launch (a program kept on the taskbar, not running;
// or another of its windows), pin, unpin.
#[tauri::command]
pub fn taskbar_app(app: tauri::AppHandle, path: String, what: String) -> bool {
    imp::app(&crate::shared(&app), &what, &path)
}

// A tray icon kept out on the taskbar or folded away (dragged there), by
// its name (systray.rs Shown), in the settings (trayPinned); Windows' own
// choice until the person makes one.
#[tauri::command]
pub fn taskbar_tray_pin(app: tauri::AppHandle, name: String, pinned: bool) {
    let sh = crate::shared(&app);
    let mut kept = sh.setting("trayPinned").as_object().cloned().unwrap_or_default();
    kept.insert(name, serde_json::json!(pinned));
    sh.change(serde_json::json!({ "trayPinned": kept }));
}

// Where the page drew each tray icon (key, x, y, w, h in its own px), for
// the programs that ask where theirs is (Shell_NotifyIconGetRect): WeChat
// looks for the pointer over its icon by it, QQ opens its menu from it.
#[tauri::command]
pub fn taskbar_tray_rects(app: tauri::AppHandle, rects: Vec<(u64, f64, f64, f64, f64)>) {
    let sh = crate::shared(&app);
    let ((ox, oy), sf) = crate::island::origin(&sh);
    let px = |v: f64| (v * sf).round() as i32;
    crate::systray::set_rects(rects.into_iter().map(|(k, x, y, w, h)| (k, (ox + px(x), oy + px(y), ox + px(x + w), oy + px(y + h)))).collect());
}
