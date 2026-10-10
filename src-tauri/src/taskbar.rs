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
//   - the desktop's picture changing (wallpaper.rs): sent for the strip's
//     Mica (taskbar:wallpaper); Windows' mode and accent colour, for the
//     strip to be as Windows' own taskbar is (taskbar:look)
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
    use tauri::{Emitter, Manager};

    use crate::systray::{self, Press};
    use crate::{jump, shell, tasks, wallpaper, Shared};

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
        fn SetWindowPos(hwnd: Hwnd, after: isize, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
        fn ShowWindow(hwnd: Hwnd, cmd: i32) -> i32;
        fn SetLayeredWindowAttributes(hwnd: Hwnd, key: u32, alpha: u8, flags: u32) -> i32;
        fn SetClassLongPtrW(hwnd: Hwnd, index: i32, value: isize) -> usize;
        fn InvalidateRect(hwnd: Hwnd, rect: *const Rect, erase: i32) -> i32;
    }

    #[link(name = "gdi32")]
    extern "system" {
        fn CreateCompatibleDC(hdc: isize) -> isize;
        fn CreateDIBSection(hdc: isize, info: *const BitmapInfoHeader, usage: u32, bits: *mut *mut u8, section: isize, offset: u32) -> isize;
        fn SelectObject(hdc: isize, obj: isize) -> isize;
        fn DeleteObject(obj: isize) -> i32;
        fn DeleteDC(hdc: isize) -> i32;
        fn CreateSolidBrush(color: u32) -> isize;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleW(name: *const u16) -> isize;
    }

    #[repr(C)]
    #[derive(Default)]
    struct MonitorInfo {
        size: u32,
        monitor: Rect,
        work: Rect,
        flags: u32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn MonitorFromPoint(pt: Point, flags: u32) -> *mut c_void;
        fn GetMonitorInfoW(monitor: *mut c_void, info: *mut MonitorInfo) -> i32;
        fn GetSystemMetrics(index: i32) -> i32;
    }

    #[link(name = "shcore")]
    extern "system" {
        fn GetDpiForMonitor(mon: *mut c_void, kind: u32, x: *mut u32, y: *mut u32) -> i32;
    }

    #[repr(C)]
    #[derive(Default)]
    struct Size {
        cx: i32,
        cy: i32,
    }

    #[repr(C)]
    struct ThumbnailProperties {
        flags: u32,
        dest: Rect,
        source: Rect,
        opacity: u8,
        visible: i32,
        client_only: i32,
    }

    #[link(name = "dwmapi")]
    extern "system" {
        fn DwmRegisterThumbnail(dest: Hwnd, src: Hwnd, thumb: *mut isize) -> i32;
        fn DwmUnregisterThumbnail(thumb: isize) -> i32;
        fn DwmUpdateThumbnailProperties(thumb: isize, props: *const ThumbnailProperties) -> i32;
        fn DwmQueryThumbnailSourceSize(thumb: isize, size: *mut Size) -> i32;
    }

    const DWM_TNP_RECTDESTINATION: u32 = 0x1;
    const DWM_TNP_OPACITY: u32 = 0x4;
    const DWM_TNP_VISIBLE: u32 = 0x8;

    const WM_TIMER: u32 = 0x0113;
    // Explorer's taskbar seen shown (the hook), lParam its window; the tray's
    // icons changed (systray.rs); the thread asked to end.
    const WM_TASKBAR_SHOWN: u32 = 0x8001;
    const WM_TRAY_CHANGED: u32 = 0x8002;
    const WM_STOP: u32 = 0x8003;
    // The strip moved: the desktop's picture under it looked at again (so
    // too when a setting or Windows' colours change).
    const WM_PAPER: u32 = 0x8004;
    const WM_SETTINGCHANGE: u32 = 0x001A;
    const WM_DWMCOLORIZATIONCOLORCHANGED: u32 = 0x0320;
    const GCLP_HBRBACKGROUND: i32 = -10;
    const MONITOR_DEFAULTTONEAREST: u32 = 2;
    const MDT_EFFECTIVE_DPI: u32 = 0;
    const SM_XVIRTUALSCREEN: i32 = 76;
    const WS_POPUP: u32 = 0x8000_0000;
    const WS_EX_TOOLWINDOW: u32 = 0x80;
    const WS_EX_TOPMOST: u32 = 0x8;
    const WS_EX_TRANSPARENT: u32 = 0x20;
    const WS_EX_LAYERED: u32 = 0x0008_0000;
    const WS_EX_NOACTIVATE: u32 = 0x0800_0000;
    const LWA_ALPHA: u32 = 0x2;
    const HWND_TOPMOST: isize = -1;
    const SWP_NOACTIVATE: u32 = 0x10;
    const SWP_SHOWWINDOW: u32 = 0x40;
    const SW_HIDE: i32 = 0;
    const SPI_GETWORKAREA: u32 = 0x30;
    const SPI_SETWORKAREA: u32 = 0x2F;

    const EVENT_OBJECT_SHOW: u32 = 0x8002;
    const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;
    // What changes the buttons: a window made, shown, hidden, gone (0x8001
    // to 0x8003), renamed, cloaked or not (another desktop), brought to the
    // front, minimized or restored; moved or sized, for one maximized or
    // no longer (on_event lets only those through).
    const WINDOW_EVENTS: [(u32, u32); 6] = [(0x8001, 0x8003), (0x800C, 0x800C), (0x8017, 0x8018), (0x0003, 0x0003), (0x0016, 0x0017), (0x800B, 0x800B)];
    const EVENT_OBJECT_LOCATIONCHANGE: u32 = 0x800B;
    const OBJID_WINDOW: i32 = 0;
    // The shell hook's: a window flashing for attention, one activated.
    const HSHELL_FLASH: usize = 0x8006;
    const HSHELL_WINDOWACTIVATED: usize = 4;
    const HSHELL_RUDEAPPACTIVATED: usize = 0x8004;
    const KEYEVENTF_EXTENDEDKEY: u32 = 0x1;
    const KEYEVENTF_KEYUP: u32 = 0x2;
    const VK_LWIN: u16 = 0x5B;
    const VK_ESCAPE: u16 = 0x1B;
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
    // The live pictures shown (window, thumbnail), over the windows listed above a button.
    static THUMBS: Mutex<Vec<(isize, isize)>> = Mutex::new(Vec::new());
    static THUMB_HOST: AtomicIsize = AtomicIsize::new(0);
    static SHELL_MSG: AtomicU32 = AtomicU32::new(0);
    // The keyboard's thread, and what it sent last.
    static KEYS: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);
    static SENT_KEYS: Mutex<Value> = Mutex::new(Value::Null);
    // Windows' own record of the tray icons it keeps out (windows_kept), read once.
    static KEPT: Mutex<Option<Vec<(String, Option<u32>, bool)>>> = Mutex::new(None);
    // The desktop's picture under the strip and Windows' look, as
    // sent last (send_look), and whether they are being looked at.
    static PAPER: Mutex<Value> = Mutex::new(Value::Null);
    static LOOK: Mutex<Value> = Mutex::new(Value::Null);
    // The live pictures' window's ground (COLORREF), as set last (thumbs_ground).
    static THUMBS_GROUND: AtomicU32 = AtomicU32::new(0x001E_1C1C);
    static LOOK_BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    // Windows' Start menu open, as told to the page last (start_seen).
    static START_OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    // The windows maximized, as last seen moving (on_event).
    static ZOOMED: Mutex<Vec<isize>> = Mutex::new(Vec::new());

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

    // The room Windows' own taskbar, put away, keeps along the bottom of the
    // display (physical): the strip goes over it, with no bar of ours (that
    // would stack above it). Waited for a moment: one set to hide itself
    // until just now takes its room after a slide. None: no room there (its
    // taskbar on another edge or display); a bar of ours then.
    pub fn explorer_room(mon: (i32, i32, i32, i32)) -> Option<(i32, i32, i32, i32)> {
        let t = Instant::now();
        loop {
            let room = shell::room(mon);
            if room.is_some() || t.elapsed() > Duration::from_millis(1500) {
                return room;
            }
            std::thread::sleep(Duration::from_millis(30));
        }
    }

    // Windows' taskbar put away and the tray taken, before the strip is laid.
    pub fn take(sh: &Shared, strip: (i32, i32, i32, i32)) {
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

    // The keyboard (tasks::keys) and the volume (audio.rs) looked at four
    // times a second; the network's way out (the quick settings' button) and
    // what is using the microphone, camera or location (tasks::in_use) every
    // two; sent when any of it changes.
    fn keys_loop(sh: Arc<Shared>) {
        let _com = crate::audio::Com::new();
        let mut sent = Value::Null;
        let using = || Value::Object(tasks::in_use().into_iter().map(|(what, names)| (what.to_string(), json!(names))).collect());
        let (mut net, mut used) = (tasks::net(), using());
        let mut looks = 0u32;
        while !TASKS_STOP.load(Ordering::SeqCst) {
            looks += 1;
            if looks % 8 == 0 {
                net = tasks::net();
                used = using();
            }
            let keys = tasks::keys();
            let volume = crate::audio::read().map(|(level, muted)| json!([level, muted]));
            let now = json!({ "lang": keys.lang, "native": keys.native, "caps": keys.caps, "net": net, "use": used, "volume": volume });
            if now != sent {
                let _ = sh.app.emit_to("island", "taskbar:keys", now.clone());
                *SENT_KEYS.lock().unwrap() = now.clone();
                sent = now;
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
        *PAPER.lock().unwrap() = Value::Null;
        *LOOK.lock().unwrap() = Value::Null;
        START_OPEN.store(false, Ordering::SeqCst);
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

    // On the strip's display (a window maximized elsewhere leaves it clear).
    fn on_strip(hwnd: isize) -> bool {
        let Some(s) = *STRIP.lock().unwrap() else { return false };
        // SAFETY: plain queries.
        unsafe { MonitorFromWindow(hwnd as Hwnd, MONITOR_DEFAULTTONEAREST) == MonitorFromPoint(Point { x: s.0 + s.2 / 2, y: s.1 + s.3 / 2 }, MONITOR_DEFAULTTONEAREST) }
    }

    // A program that only hosts what its windows show, whose icons are
    // theirs: a store app (its frame, before its app is in it; then the
    // app's program, under WindowsApps), Windows' own apps, Java, Python.
    fn hosts(path: &str) -> bool {
        let p = path.to_ascii_lowercase();
        ["\\windowsapps\\", "\\systemapps\\", "\\immersivecontrolpanel\\"].iter().any(|d| p.contains(d))
            || ["applicationframehost.exe", "java.exe", "javaw.exe", "python.exe", "pythonw.exe", "py.exe", "pyw.exe"].iter().any(|n| p.ends_with(&format!("\\{n}")))
    }

    // Windows' plain program icon, drawn once at this size (plain_program_icon).
    fn plain_png(size: i32) -> String {
        static PLAIN: Mutex<Option<(i32, String)>> = Mutex::new(None);
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
        // As big as the buttons show them alone (island.js, 24px); smaller
        // with their titles, and in the lists above them.
        let size = (24.0 * sh.screens().primary.map_or(1.0, |a| a.sf)).round() as i32;
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
                    // The app each window says it is, and that app's icon as
                    // Windows' taskbar shows it (app_png): the button's, and a
                    // hosted window's own (a store app's frame shows a plain
                    // icon until its app is in it).
                    let app_pngs: Vec<Option<String>> = windows.iter().map(|t| tasks::app_id(t.hwnd).and_then(|id| app_png(&id, size))).collect();
                    let hosted = hosts(path);
                    let windows: Vec<Value> = windows
                        .iter()
                        .zip(&app_pngs)
                        .map(|(t, app)| {
                            let icon = tasks::icon_of(t.hwnd);
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
                            json!({
                                "id": t.hwnd,
                                "title": t.title,
                                "png": png,
                                "front": t.hwnd == front,
                                "min": t.min,
                                "max": tasks::maximized(t.hwnd) && on_strip(t.hwnd),
                                "flash": flashing.contains(&t.hwnd),
                                "sessions": marks.get(&t.hwnd).cloned().unwrap_or_default(),
                            })
                        })
                        .collect();
                    let key = path.to_lowercase();
                    let name = names.entry(key.clone()).or_insert_with(|| tasks::app_name(path)).clone();
                    // As Windows' taskbar has it on a program's button: the
                    // app's icon by the id its windows give (Settings' grey cog,
                    // from its first moment); else the program's own icon (File
                    // Explorer's for every folder, drive or This PC its windows
                    // show); its window's where the program only hosts what it
                    // shows (hosts) or has no icon of its own (Windows' plain
                    // one), and while it has none.
                    let window_png = windows.iter().find_map(|w| w["png"].as_str().filter(|p| !p.is_empty()).map(str::to_string));
                    let file_png = file_pngs
                        .entry(key.clone())
                        .or_insert_with(|| {
                            let icon = tasks::file_icon(path);
                            let png = if icon == 0 { String::new() } else { png_of(icon, size).unwrap_or_default() };
                            tasks::destroy_icon(icon);
                            png
                        })
                        .clone();
                    let own = !file_png.is_empty() && !hosts(path) && file_png != plain_png(size);
                    let png = match app_pngs.into_iter().flatten().next() {
                        Some(app) => app,
                        None if own => file_png,
                        None => window_png.unwrap_or(file_png),
                    };
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

    // The live pictures' window as the cards round it are: dark, or light in
    // Windows' light mode (style.css .wpop, its ground #1c1c1e or #f9f9f9).
    fn thumbs_ground(light: bool) {
        let host = THUMB_HOST.load(Ordering::SeqCst);
        let colour: u32 = if light { 0x00F9_F9F9 } else { 0x001E_1C1C };
        if host == 0 || THUMBS_GROUND.swap(colour, Ordering::SeqCst) == colour {
            return;
        }
        // SAFETY: our own window's class; the brush it had, ours, deleted
        // once replaced.
        unsafe {
            let old = SetClassLongPtrW(host as Hwnd, GCLP_HBRBACKGROUND, CreateSolidBrush(colour));
            if old != 0 {
                DeleteObject(old as isize);
            }
            InvalidateRect(host as Hwnd, std::ptr::null(), 1);
        }
    }

    // Windows' live pictures of windows (DWM thumbnails) where the page left
    // room for them (the windows listed above a program's button): each
    // window's at (x, y, w, h) on the screen, physical, its shape kept, on
    // the window of their own laid over those rooms. None: all taken away.
    pub fn thumbs(items: Vec<(isize, (i32, i32, i32, i32))>) {
        let host = THUMB_HOST.load(Ordering::SeqCst) as Hwnd;
        let mut shown = THUMBS.lock().unwrap();
        for (_, thumb) in shown.drain(..) {
            // SAFETY: a thumbnail of ours.
            unsafe { DwmUnregisterThumbnail(thumb) };
        }
        if host.is_null() {
            return;
        }
        if items.is_empty() {
            // SAFETY: our own window.
            unsafe { ShowWindow(host, SW_HIDE) };
            return;
        }
        // Over all the rooms at once, above the home's window.
        let left = items.iter().map(|i| i.1 .0).min().unwrap_or(0);
        let top = items.iter().map(|i| i.1 .1).min().unwrap_or(0);
        let right = items.iter().map(|i| i.1 .0 + i.1 .2).max().unwrap_or(0);
        let bottom = items.iter().map(|i| i.1 .1 + i.1 .3).max().unwrap_or(0);
        // SAFETY: our own window.
        unsafe { SetWindowPos(host, HWND_TOPMOST, left, top, right - left, bottom - top, SWP_NOACTIVATE | SWP_SHOWWINDOW) };
        for (hwnd, (x, y, w, h)) in items {
            let (x, y) = (x - left, y - top);
            let mut thumb = 0isize;
            // SAFETY: our own window and another's; the handle is ours to unregister.
            if unsafe { DwmRegisterThumbnail(host, hwnd as Hwnd, &mut thumb) } != 0 || thumb == 0 {
                continue;
            }
            let mut size = Size::default();
            // SAFETY: our own struct.
            unsafe { DwmQueryThumbnailSourceSize(thumb, &mut size) };
            // Fitted into the room, its shape kept, in the middle.
            let (sw, sh) = (size.cx.max(1) as f64, size.cy.max(1) as f64);
            let k = (w as f64 / sw).min(h as f64 / sh);
            let (dw, dh) = ((sw * k).round() as i32, (sh * k).round() as i32);
            let (dx, dy) = (x + (w - dw) / 2, y + (h - dh) / 2);
            let props = ThumbnailProperties {
                flags: DWM_TNP_RECTDESTINATION | DWM_TNP_OPACITY | DWM_TNP_VISIBLE,
                dest: Rect { left: dx, top: dy, right: dx + dw, bottom: dy + dh },
                source: Rect::default(),
                opacity: 255,
                visible: 1,
                client_only: 0,
            };
            // SAFETY: our own thumbnail and struct.
            unsafe { DwmUpdateThumbnailProperties(thumb, &props) };
            shown.push((hwnd, thumb));
        }
    }

    // A press on a program's button kept on the taskbar with no window: it
    // starts. Kept on the taskbar, or no longer.
    pub fn app(sh: &Shared, what: &str, path: &str) -> bool {
        let done = match what {
            "launch" => {
                close_flyout();
                tasks::launch(path)
            }
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
            // Off the page's thread: a window may take its time to come up
            // or go down (tasks::press waits for it).
            "press" => {
                std::thread::spawn(move || {
                    // Start closed for it: the window to the front, even the
                    // one in front before Start (not minimized), as on Windows'.
                    if close_flyout() {
                        crate::jump::bring_window(hwnd);
                    } else {
                        tasks::press(hwnd);
                    }
                    wake_tasks();
                });
                true
            }
            "close" => tasks::close(hwnd),
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

    // A press anywhere, told to the page while it has something open (the
    // windows listed above a button, the folded tray icons) that closes on
    // a press elsewhere, as Windows' do: ours never take the focus whose
    // loss would tell. A low-level mouse hook, on the host's thread (whose
    // loop it needs) and only while asked for.
    const WM_WATCH_PRESSES: u32 = 0x8010;
    const WH_MOUSE_LL: i32 = 14;
    const WM_LBUTTONDOWN: u32 = 0x0201;
    const WM_RBUTTONDOWN: u32 = 0x0204;
    const WM_MBUTTONDOWN: u32 = 0x0207;
    static PRESS_HOOK: AtomicIsize = AtomicIsize::new(0);

    #[repr(C)]
    struct MouseLl {
        pt: Point,
        data: u32,
        flags: u32,
        time: u32,
        extra: usize,
    }

    type HookProc = extern "system" fn(i32, usize, isize) -> isize;

    #[link(name = "user32")]
    extern "system" {
        fn SetWindowsHookExW(id: i32, proc_: HookProc, module: isize, thread: u32) -> isize;
        fn UnhookWindowsHookEx(hook: isize) -> i32;
        fn CallNextHookEx(hook: isize, code: i32, wparam: usize, lparam: isize) -> isize;
    }

    pub fn watch_presses(on: bool) {
        // SAFETY: a posted message to the host's own window.
        unsafe { PostMessageW(OURS.load(Ordering::SeqCst) as Hwnd, WM_WATCH_PRESSES, on as usize, 0) };
    }

    // On the host's thread.
    fn watch(on: bool) {
        let hook = PRESS_HOOK.load(Ordering::SeqCst);
        // SAFETY: a hook of our own, put in and taken out on this thread.
        unsafe {
            if on && hook == 0 {
                PRESS_HOOK.store(SetWindowsHookExW(WH_MOUSE_LL, press_hook, GetModuleHandleW(std::ptr::null()), 0), Ordering::SeqCst);
            } else if !on && hook != 0 {
                UnhookWindowsHookEx(hook);
                PRESS_HOOK.store(0, Ordering::SeqCst);
            }
        }
    }

    // In every press's way through Windows: only the point posted on (to the
    // host's own loop, press_seen), and passed on at once. Held up here, every
    // click on the machine would wait, and Windows would drop the hook.
    const WM_PRESS_SEEN: u32 = 0x8011;

    extern "system" fn press_hook(code: i32, wparam: usize, lparam: isize) -> isize {
        if code >= 0 && matches!(wparam as u32, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN) {
            // SAFETY: Windows hands a MSLLHOOKSTRUCT with these messages; a
            // posted message to the host's own window, no pointers.
            unsafe {
                let at = (*(lparam as *const MouseLl)).pt;
                PostMessageW(OURS.load(Ordering::SeqCst) as Hwnd, WM_PRESS_SEEN, at.x as u32 as usize, at.y as isize);
            }
        }
        // SAFETY: passed on, as every hook must.
        unsafe { CallNextHookEx(0, code, wparam, lparam) }
    }

    // On the host's thread, after the hook let the press go: where on the
    // page (its px), for it to tell what was pressed.
    fn press_seen(x: i32, y: i32) {
        if let Some(sh) = sh() {
            let ((ox, oy), sf) = crate::island::origin(&sh);
            let _ = sh.app.emit_to("island", "taskbar:press", json!({ "x": (x - ox) as f64 / sf, "y": (y - oy) as f64 / sf }));
        }
    }

    // Where the strip is now (physical): ours of the taskbar's class goes
    // there too, for those who ask the taskbar's window where it is.
    pub fn strip_moved(strip: (i32, i32, i32, i32)) {
        *STRIP.lock().unwrap() = Some(strip);
        if is_up() {
            systray::place(strip);
            // SAFETY: our own window; a posted message, no pointers.
            unsafe { PostMessageW(OURS.load(Ordering::SeqCst) as Hwnd, WM_PAPER, 0, 0) };
        }
    }

    // --- The desktop's picture, for the strip's Mica ----------------------------------

    // The display the strip is on (physical), and its scale.
    fn monitor_of(strip: (i32, i32, i32, i32)) -> ((i32, i32, i32, i32), f64) {
        let mut info = MonitorInfo { size: std::mem::size_of::<MonitorInfo>() as u32, ..Default::default() };
        let (mut dx, mut dy) = (96u32, 96u32);
        // SAFETY: our own out-parameters, of the sizes the calls write.
        unsafe {
            let mon = MonitorFromPoint(Point { x: strip.0 + strip.2 / 2, y: strip.1 + strip.3 / 2 }, MONITOR_DEFAULTTONEAREST);
            GetMonitorInfoW(mon, &mut info);
            GetDpiForMonitor(mon, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
        }
        let r = &info.monitor;
        ((r.left, r.top, r.right - r.left, r.bottom - r.top), dx.max(1) as f64 / 96.0)
    }

    // The picture on the strip's display (wallpaper.rs), sent when it changes
    // (taskbar:wallpaper): its file (by the asset protocol, allowed for it;
    // its last change after it, so a new picture under the same name is
    // loaded anew), how it is laid, the colour round it, and where the
    // display and all of them together are from the strip's top-left (the
    // page's px). With it Windows' look (taskbar:look: { mode: dark | light,
    // accent: { base, light, dark } }), for the strip to be as Windows' own
    // taskbar is; the live pictures' window's ground with it (thumbs_ground).
    // Looked at when the strip moves, when Windows says a setting or its
    // colours changed, and every five seconds (a slideshow, Spotlight); on
    // a thread of its own, one at a time: Windows' answer goes through
    // Explorer, which may be busy, and the tray's messages come to this one.
    fn send_look() {
        if LOOK_BUSY.swap(true, Ordering::SeqCst) {
            return;
        }
        std::thread::spawn(|| {
            look_at_desktop();
            LOOK_BUSY.store(false, Ordering::SeqCst);
        });
    }

    fn look_at_desktop() {
        let Some(sh) = sh() else { return };
        let (light, accent) = wallpaper::look();
        thumbs_ground(light);
        let look = json!({
            "mode": if light { "light" } else { "dark" },
            "accent": accent.map(|a| json!({ "base": a.base, "light": a.light, "dark": a.dark })),
        });
        let mut sent = LOOK.lock().unwrap();
        if *sent != look {
            log(&format!("look: {look}"));
            *sent = look.clone();
            let _ = sh.app.emit_to("island", "taskbar:look", look);
        }
        drop(sent);
        let Some(strip) = *STRIP.lock().unwrap() else { return };
        let (mon, sf) = monitor_of(strip);
        let Some(paper) = wallpaper::read(mon) else { return };
        // SAFETY: plain reads.
        let screen = unsafe { (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_XVIRTUALSCREEN + 1), GetSystemMetrics(SM_XVIRTUALSCREEN + 2), GetSystemMetrics(SM_XVIRTUALSCREEN + 3)) };
        let rel = |(x, y, w, h): (i32, i32, i32, i32)| json!([(x - strip.0) as f64 / sf, (y - strip.1) as f64 / sf, w as f64 / sf, h as f64 / sf]);
        let url = paper.file.as_ref().map(|f| format!("{}?v={}", crate::data::asset_url(f), paper.stamp));
        let now = json!({ "url": url, "position": paper.position, "color": paper.color, "mon": rel(mon), "screen": rel(screen), "sf": sf });
        let mut sent = PAPER.lock().unwrap();
        if *sent == now {
            return;
        }
        if let Some(file) = &paper.file {
            let _ = sh.app.asset_protocol_scope().allow_file(file);
        }
        log(&format!("wallpaper: {:?}, {}, {}", paper.file, paper.position, paper.color));
        *sent = now.clone();
        let _ = sh.app.emit_to("island", "taskbar:wallpaper", now);
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
        let paper = PAPER.lock().unwrap().clone();
        if !paper.is_null() {
            let _ = sh.app.emit_to("island", "taskbar:wallpaper", paper);
        }
        let look = LOOK.lock().unwrap().clone();
        if !look.is_null() {
            let _ = sh.app.emit_to("island", "taskbar:look", look);
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
            close_flyout();
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

    // Windows' own Start (or search, the notifications, the quick settings)
    // in front: closed first, as a press on Windows' taskbar closes it. Ours
    // takes no focus away from it, and the window pressed for could not come
    // up past it (it holds the foreground). Esc to it, and a moment for it to
    // go. Whether there was one.
    fn close_flyout() -> bool {
        let front = tasks::front();
        if !tasks::is_shell_flyout(front) {
            return false;
        }
        let key = |up: bool| Input {
            kind: 1,
            u: InputUnion { ki: KeybdInput { vk: VK_ESCAPE, scan: 0, flags: if up { KEYEVENTF_KEYUP } else { 0 }, time: 0, extra: 0 } },
        };
        let inputs = [key(false), key(true)];
        // Esc goes to whatever is in front: looked at again right before it
        // goes (telling the program took a moment), so it never lands in
        // another (in a terminal it stops what runs there).
        if tasks::front() != front {
            return false;
        }
        // SAFETY: an array of INPUTs of the size passed.
        unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<Input>() as i32) };
        let asked = Instant::now();
        while tasks::front() == front && asked.elapsed() < Duration::from_millis(400) {
            std::thread::sleep(Duration::from_millis(20));
        }
        true
    }

    // What the taskbar's own buttons open, by the keys that open them.
    pub fn open(what: &str) -> bool {
        // The input method of the window in front: its own script or plain letters.
        if what == "ime" {
            return tasks::toggle_native();
        }
        // A press on what shows the microphone, camera or location in use:
        // its page in Windows' privacy settings.
        let page = match what {
            "privacy-mic" => Some("ms-settings:privacy-microphone"),
            "privacy-cam" => Some("ms-settings:privacy-webcam"),
            "privacy-loc" => Some("ms-settings:privacy-location"),
            _ => None,
        };
        if let Some(page) = page {
            return tasks::launch(page);
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
            // The windows' live pictures go on a window of their own over the
            // cards (taskbar.rs thumbs): drawn on the home's, a webview's,
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
            thumbs(Vec::new());
            let host = THUMB_HOST.swap(0, Ordering::SeqCst);
            if host != 0 {
                DestroyWindow(host as Hwnd);
            }
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
        // Moved or sized: only a window maximized, or no longer, changes
        // the buttons (the strip clear but for one, island.js); every move
        // of every window comes here.
        if event == EVENT_OBJECT_LOCATIONCHANGE {
            let h = hwnd as isize;
            let now = tasks::maximized(h);
            let mut zoomed = ZOOMED.lock().unwrap();
            if now == zoomed.contains(&h) {
                return;
            }
            if now {
                zoomed.push(h);
            } else {
                zoomed.retain(|z| *z != h);
            }
        }
        if event == EVENT_SYSTEM_FOREGROUND {
            start_seen(tasks::is_start(hwnd as isize));
        }
        wake_tasks();
    }

    // Windows' Start menu opened or closed (its window came to the front,
    // or another did): told to the page at once (taskbar:start), for the
    // Start button to do as Windows' own does.
    fn start_seen(open: bool) {
        if START_OPEN.swap(open, Ordering::SeqCst) == open {
            return;
        }
        if let Some(sh) = sh() {
            let _ = sh.app.emit_to("island", "taskbar:start", json!({ "open": open }));
        }
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
        if ticks % 10 == 0 {
            send_look();
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
    #[repr(C)]
    struct IconInfo {
        is_icon: i32,
        hot_x: u32,
        hot_y: u32,
        mask: isize,
        color: isize,
    }

    #[repr(C)]
    struct BitmapObj {
        kind: i32,
        width: i32,
        height: i32,
        width_bytes: i32,
        planes: u16,
        bits_pixel: u16,
        bits: *mut c_void,
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetIconInfo(icon: isize, info: *mut IconInfo) -> i32;
    }

    #[link(name = "gdi32")]
    extern "system" {
        fn GetObjectW(obj: isize, size: i32, out: *mut c_void) -> i32;
    }

    // An icon's own size, its picture's (half its mask's for a two-colour one).
    fn icon_size(icon: isize) -> Option<(i32, i32)> {
        let mut info = IconInfo { is_icon: 0, hot_x: 0, hot_y: 0, mask: 0, color: 0 };
        // SAFETY: our own structs; the bitmaps the call makes are deleted.
        unsafe {
            if GetIconInfo(icon, &mut info) == 0 {
                return None;
            }
            let mut bm = BitmapObj { kind: 0, width: 0, height: 0, width_bytes: 0, planes: 0, bits_pixel: 0, bits: std::ptr::null_mut() };
            let of = if info.color != 0 { info.color } else { info.mask };
            let ok = GetObjectW(of, std::mem::size_of::<BitmapObj>() as i32, &mut bm as *mut BitmapObj as *mut c_void) != 0;
            for b in [info.color, info.mask] {
                if b != 0 {
                    DeleteObject(b);
                }
            }
            let h = if info.color != 0 { bm.height } else { bm.height / 2 };
            (ok && bm.width > 0 && h > 0).then_some((bm.width, h))
        }
    }

    // Drawn at its own size, then made `size` square smoothly (super::resize):
    // drawn straight at another, Windows stretches it pixel by pixel, which
    // shows as jagged edges (a 32px icon at 30, a 16px one at 20).
    fn png_of(icon: isize, size: i32) -> Option<String> {
        if size <= 0 {
            return None;
        }
        let (w, h) = icon_size(icon).filter(|&(w, h)| w <= 256 && h <= 256).unwrap_or((size, size));
        let n = (w * h) as usize;
        let draw = |fill: u8| -> Option<Vec<u8>> {
            // SAFETY: a DIB section of our own, drawn into and read, then deleted.
            unsafe {
                let dc = CreateCompatibleDC(0);
                let info = BitmapInfoHeader { size: 40, width: w, height: -h, planes: 1, bit_count: 32, compression: 0, size_image: 0, x_ppm: 0, y_ppm: 0, clr_used: 0, clr_important: 0 };
                let mut bits: *mut u8 = std::ptr::null_mut();
                let bmp = CreateDIBSection(dc, &info, 0, &mut bits, 0, 0);
                if bmp == 0 || bits.is_null() {
                    DeleteDC(dc);
                    return None;
                }
                let old = SelectObject(dc, bmp);
                std::ptr::write_bytes(bits, fill, n * 4);
                DrawIconEx(dc, 0, 0, icon, w, h, 0, 0, DI_NORMAL);
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
        let rgba = if (w, h) == (size, size) { rgba } else { super::resize(&rgba, w as usize, h as usize, size as usize, size as usize) };
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

    // An app's icon as Windows' taskbar shows it, asked of the shell by the
    // app's id (tasks::app_id): its Apps folder's item, drawn by the shell at
    // `size` from the app's own pictures (a store app's plain one for the
    // taskbar, Settings' grey cog). Each asked once; None where the shell
    // knows no such app.
    #[repr(C)]
    struct KnownGuid(u32, u16, u16, [u8; 8]);

    #[repr(C)]
    struct ImageSize {
        cx: i32,
        cy: i32,
    }

    // IShellItemImageFactory's table.
    #[repr(C)]
    struct FactoryVtbl {
        _query_interface: usize,
        _add_ref: usize,
        release: extern "system" fn(*mut Factory) -> u32,
        get_image: extern "system" fn(*mut Factory, ImageSize, i32, *mut isize) -> i32,
    }

    #[repr(C)]
    struct Factory {
        vtbl: *const FactoryVtbl,
    }

    #[link(name = "shell32")]
    extern "system" {
        fn SHCreateItemInKnownFolder(folder: *const KnownGuid, flags: u32, item: *const u16, iid: *const KnownGuid, out: *mut *mut c_void) -> i32;
    }

    #[link(name = "gdi32")]
    extern "system" {
        fn GetDIBits(dc: isize, bmp: isize, start: u32, lines: u32, bits: *mut u8, info: *mut BitmapInfoHeader, usage: u32) -> i32;
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetDC(hwnd: Hwnd) -> isize;
        fn ReleaseDC(hwnd: Hwnd, dc: isize) -> i32;
        fn MonitorFromWindow(hwnd: Hwnd, flags: u32) -> *mut c_void;
    }

    const FOLDERID_APPS: KnownGuid = KnownGuid(0x1E87_508D, 0x89C2, 0x42F0, [0x8A, 0x7E, 0x64, 0x5A, 0x0F, 0x50, 0xCA, 0x58]);
    const IID_IMAGE_FACTORY: KnownGuid = KnownGuid(0xBCC1_8B79, 0xBA16, 0x442F, [0x80, 0xC4, 0x8A, 0x59, 0xC3, 0x0C, 0x46, 0x3B]);
    const SIIGBF_ICONONLY: i32 = 0x4;

    static APP_PNGS: Mutex<Option<HashMap<(String, i32), Option<String>>>> = Mutex::new(None);

    fn app_png(app_id: &str, size: i32) -> Option<String> {
        let key = (app_id.to_string(), size);
        if let Some(known) = APP_PNGS.lock().unwrap().get_or_insert_with(HashMap::new).get(&key) {
            return known.clone();
        }
        tasks::with_com();
        // SAFETY: the shell's item and bitmap, released and deleted; the
        // pixels read into our own buffer of the size asked for.
        let rgba = unsafe {
            let mut factory: *mut Factory = std::ptr::null_mut();
            let made = SHCreateItemInKnownFolder(&FOLDERID_APPS, 0, wide(app_id).as_ptr(), &IID_IMAGE_FACTORY, &mut factory as *mut *mut Factory as *mut *mut c_void);
            let mut bmp = 0isize;
            let drawn = made >= 0 && !factory.is_null() && ((*(*factory).vtbl).get_image)(factory, ImageSize { cx: size, cy: size }, SIIGBF_ICONONLY, &mut bmp) >= 0 && bmp != 0;
            if !factory.is_null() {
                ((*(*factory).vtbl).release)(factory);
            }
            let mut bm: BitmapObj = std::mem::zeroed();
            let sized = drawn && GetObjectW(bmp, std::mem::size_of::<BitmapObj>() as i32, &mut bm as *mut BitmapObj as *mut c_void) != 0 && bm.width > 0 && bm.height > 0 && bm.width <= 256 && bm.height <= 256;
            let pixels = sized.then(|| {
                let (w, h) = (bm.width, bm.height);
                let mut info = BitmapInfoHeader { size: 40, width: w, height: -h, planes: 1, bit_count: 32, compression: 0, size_image: 0, x_ppm: 0, y_ppm: 0, clr_used: 0, clr_important: 0 };
                let mut bgra = vec![0u8; (w * h * 4) as usize];
                let dc = GetDC(std::ptr::null_mut());
                let lines = GetDIBits(dc, bmp, 0, h as u32, bgra.as_mut_ptr(), &mut info, 0);
                ReleaseDC(std::ptr::null_mut(), dc);
                (lines == h).then(|| {
                    for px in bgra.chunks_exact_mut(4) {
                        px.swap(0, 2);
                    }
                    if (w, h) == (size, size) { bgra } else { super::resize(&bgra, w as usize, h as usize, size as usize, size as usize) }
                })
            });
            if bmp != 0 {
                DeleteObject(bmp);
            }
            pixels.flatten()
        };
        let png = rgba.filter(|p| p.chunks_exact(4).any(|px| px[3] != 0)).and_then(|rgba| {
            let mut out = Vec::new();
            {
                let mut encoder = png::Encoder::new(&mut out, size as u32, size as u32);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                let mut writer = encoder.write_header().ok()?;
                writer.write_image_data(&rgba).ok()?;
            }
            Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(out)))
        });
        APP_PNGS.lock().unwrap().get_or_insert_with(HashMap::new).insert(key, png.clone());
        png
    }

    #[cfg(test)]
    mod app_icon_tests {
        // On a desktop: Settings' icon by its app id, at the size asked;
        // nothing for an id the shell does not know.
        #[test]
        fn the_shell_draws_an_app_by_its_id() {
            assert_eq!(super::app_png("no.such.app_0000000000000!Nothing", 30), None);
            let Some(png) = super::app_png("windows.immersivecontrolpanel_cw5n1h2txyewy!microsoft.windows.immersivecontrolpanel", 30) else { return };
            use base64::Engine;
            let bytes = base64::engine::general_purpose::STANDARD.decode(png.trim_start_matches("data:image/png;base64,")).unwrap();
            let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
            let info = decoder.read_info().unwrap();
            assert_eq!((info.info().width, info.info().height), (30, 30));
        }
    }

    // The live pictures' window: nothing of its own but its colour.
    extern "system" fn thumbs_proc(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize {
        // SAFETY: the default for everything.
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
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
            WM_PAPER | WM_DWMCOLORIZATIONCOLORCHANGED | WM_SETTINGCHANGE => send_look(),
            WM_STOP => {
                watch(false);
                // SAFETY: ending this thread's loop.
                unsafe { PostQuitMessage(0) }
            }
            WM_WATCH_PRESSES => watch(wparam != 0),
            WM_PRESS_SEEN => press_seen(wparam as u32 as i32, lparam as i32),
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
    pub fn take(_sh: &Shared, _strip: (i32, i32, i32, i32)) {}
    pub fn explorer_room(_mon: (i32, i32, i32, i32)) -> Option<(i32, i32, i32, i32)> {
        None
    }
    pub fn give_back(_sh: &Shared) {}
    pub fn strip_moved(_strip: (i32, i32, i32, i32)) {}
    pub fn resend(_sh: &Shared) {}
    pub fn press(_sh: &Shared, _key: u64, _press: &str, _home_hwnd: isize) -> bool {
        false
    }
    pub fn open(_what: &str) -> bool {
        false
    }
    pub fn windows(_hwnds: Vec<isize>) -> bool {
        false
    }
    pub fn watch_presses(_on: bool) {}
    pub fn window(_what: &str, _hwnd: isize) -> bool {
        false
    }
    pub fn app(_sh: &Shared, _what: &str, _path: &str) -> bool {
        false
    }
    pub fn thumbs(_items: Vec<(isize, (i32, i32, i32, i32))>) {}
    pub fn work_area_bottom() -> i32 {
        0
    }
}

pub use imp::{explorer_room, give_back, resend, strip_moved, take, work_area_bottom};

// RGBA pixels (sw × sh) made dw × dh smoothly: each pixel out the weighted
// mean of those it covers, under a tent as wide as a pixel of the coarser
// of the two (bilinear growing, area-like shrinking), on colour premultiplied
// by its alpha so edges keep no dark fringe.
#[cfg_attr(not(windows), allow(dead_code))]
fn resize(src: &[u8], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<u8> {
    let pre: Vec<[f32; 4]> = src
        .chunks_exact(4)
        .map(|p| {
            let a = p[3] as f32 / 255.0;
            [p[0] as f32 * a, p[1] as f32 * a, p[2] as f32 * a, p[3] as f32]
        })
        .collect();
    // For each pixel out along one side: the pixels in, and their weights.
    let weights = |s: usize, d: usize| -> Vec<Vec<(usize, f32)>> {
        let scale = s as f32 / d as f32;
        let radius = scale.max(1.0);
        (0..d)
            .map(|i| {
                let centre = (i as f32 + 0.5) * scale;
                let lo = (centre - radius).floor().max(0.0) as usize;
                let hi = ((centre + radius).ceil() as usize).min(s);
                let mut ws: Vec<(usize, f32)> = (lo..hi).map(|j| (j, 1.0 - ((j as f32 + 0.5 - centre).abs() / radius))).filter(|w| w.1 > 0.0).collect();
                let sum: f32 = ws.iter().map(|w| w.1).sum();
                if sum > 0.0 {
                    ws.iter_mut().for_each(|w| w.1 /= sum);
                } else {
                    ws = vec![((centre as usize).min(s - 1), 1.0)];
                }
                ws
            })
            .collect()
    };
    let (across, down) = (weights(sw, dw), weights(sh, dh));
    let mut wide = vec![[0f32; 4]; dw * sh];
    for y in 0..sh {
        for (x, ws) in across.iter().enumerate() {
            let mut acc = [0f32; 4];
            for &(j, w) in ws {
                let p = pre[y * sw + j];
                (0..4).for_each(|c| acc[c] += p[c] * w);
            }
            wide[y * dw + x] = acc;
        }
    }
    let mut out = vec![0u8; dw * dh * 4];
    for (y, ws) in down.iter().enumerate() {
        for x in 0..dw {
            let mut acc = [0f32; 4];
            for &(j, w) in ws {
                let p = wide[j * dw + x];
                (0..4).for_each(|c| acc[c] += p[c] * w);
            }
            let i = (y * dw + x) * 4;
            let a = acc[3].clamp(0.0, 255.0);
            if a >= 0.5 {
                (0..3).for_each(|c| out[i + c] = (acc[c] * 255.0 / a).round().clamp(0.0, 255.0) as u8);
                out[i + 3] = a.round() as u8;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::resize;

    #[test]
    fn resize_keeps_a_flat_colour_and_the_size() {
        let src: Vec<u8> = (0..16 * 16).flat_map(|_| [200, 100, 50, 255]).collect();
        for (dw, dh) in [(20, 20), (12, 12), (30, 30)] {
            let out = resize(&src, 16, 16, dw, dh);
            assert_eq!(out.len(), dw * dh * 4);
            assert!(out.chunks_exact(4).all(|p| p == [200, 100, 50, 255]));
        }
    }

    #[test]
    fn resize_softens_a_hard_edge_without_a_dark_fringe() {
        // Left half opaque white, right half fully transparent black.
        let src: Vec<u8> = (0..8 * 8).flat_map(|i| if i % 8 < 4 { [255, 255, 255, 255] } else { [0, 0, 0, 0] }).collect();
        let out = resize(&src, 8, 8, 10, 10);
        let row: Vec<&[u8]> = out.chunks_exact(4).take(10).collect();
        // Somewhere between, partly see-through; what shows is still white.
        assert!(row.iter().any(|p| p[3] > 0 && p[3] < 255));
        assert!(row.iter().filter(|p| p[3] > 0).all(|p| p[0] == 255 && p[1] == 255 && p[2] == 255));
    }
}

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

// A press on the button of a program with several windows: all of them to
// the front, or all minimized when one is in front.
#[tauri::command]
pub fn taskbar_windows(ids: Vec<i64>) -> bool {
    imp::windows(ids.into_iter().map(|id| id as isize).collect())
}

// While the page has something open that closes on a press elsewhere: the
// presses told to it (taskbar:press).
#[tauri::command]
pub fn taskbar_watch_presses(on: bool) {
    imp::watch_presses(on);
}

// Where the page left room for windows' live pictures (window, x, y, w, h
// in its own px): shown there; none, taken away.
#[tauri::command]
pub fn taskbar_thumbs(app: tauri::AppHandle, items: Vec<(i64, f64, f64, f64, f64)>) {
    let sh = crate::shared(&app);
    let ((ox, oy), sf) = crate::island::origin(&sh);
    let px = |v: f64| (v * sf).round() as i32;
    let items = items.into_iter().map(|(h, x, y, w, ht)| (h as isize, (ox + px(x), oy + px(y), px(w), px(ht)))).collect();
    imp::thumbs(items);
}

// A program's button: launch (a program kept on the taskbar, not running;
// or another of its windows), pin, unpin.
#[tauri::command]
pub fn taskbar_app(app: tauri::AppHandle, path: String, what: String) -> bool {
    imp::app(&crate::shared(&app), &what, &path)
}

// A tray icon kept out on the taskbar or folded away (dragged there), by
// its name (systray.rs Shown), in the settings (trayPinned); Windows' own
// choice until the person makes one. And the tray's order as dragged
// (trayOrder, names), the two in one change.
#[tauri::command]
pub fn taskbar_tray_pin(app: tauri::AppHandle, name: String, pinned: Option<bool>, order: Option<Vec<String>>) {
    let sh = crate::shared(&app);
    let mut change = serde_json::Map::new();
    if let Some(pinned) = pinned {
        let mut kept = sh.setting("trayPinned").as_object().cloned().unwrap_or_default();
        kept.insert(name, serde_json::json!(pinned));
        change.insert("trayPinned".into(), serde_json::Value::Object(kept));
    }
    if let Some(order) = order {
        change.insert("trayOrder".into(), serde_json::json!(order.into_iter().take(200).collect::<Vec<_>>()));
    }
    if !change.is_empty() {
        sh.change(serde_json::Value::Object(change));
    }
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
