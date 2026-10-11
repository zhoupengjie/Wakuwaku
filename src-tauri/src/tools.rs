// The taskbar's own widgets that are no plugins (tools.js, island.js): what
// they read from Windows and what they do there. Read only while one of
// their kind is put somewhere (taskbarLayout), on one thread ("tools"):
//
//   media    what plays (Windows' media controls, the one session it
//            calls current: a browser's video, Spotify, NetEase Cloud
//            Music), each second; played, paused, skipped
//   trash    the Recycle Bin's items and bytes, every 5 seconds; opened,
//            emptied (the page asked first)
//   disk     each fixed or removable drive's free and total, each minute
//   awake    the computer and its screen kept awake (until when, or off)
//   power    locked, put to sleep, signed out, restarted, shut down (the
//            page asked first)
//   picker   the colour under the next click anywhere (a low-level mouse
//            hook, its click kept from the window under it; a right
//            click, or 20 seconds, gives up), copied as #rrggbb
//   launch   a program, folder, file or address opened, and its icon
//
// Told to the island as "tools:state" ({ media, trash, disk, awake }) when
// it changes, and "tools:picked" ({ hex }) once a colour is picked.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::Emitter;

use crate::Shared;

pub const KINDS: [&str; 13] = ["media", "notes", "timer", "calc", "trash", "power", "awake", "windows", "launch", "clock", "disk", "picker", "search"];

// Where widgets are put: any tool in them, however deep (a stack, a
// container), counts.
const PLACES: [&str; 1] = ["taskbarLayout"];

fn find_kinds(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(o) => {
            if o.get("type").and_then(Value::as_str) == Some("tool") {
                if let Some(kind) = o.get("of").and_then(Value::as_str) {
                    if !out.iter().any(|k| k == kind) {
                        out.push(kind.to_string());
                    }
                }
            }
            o.values().for_each(|v| find_kinds(v, out));
        }
        Value::Array(a) => a.iter().for_each(|v| find_kinds(v, out)),
        _ => {}
    }
}

// The kinds put somewhere, while she is the taskbar.
pub fn placed(sh: &Shared) -> Vec<String> {
    let mut out = Vec::new();
    if sh.setting("display") == "taskbar" {
        for place in PLACES {
            find_kinds(&sh.setting(place), &mut out);
        }
    }
    out
}

// What was told last, and what the thread is asked to do next.
#[derive(Default)]
struct Said {
    state: Value,
    // Media buttons pressed (prev, toggle, next), done on the thread that
    // holds Windows' media session manager.
    asked: Vec<String>,
    // Look again now (an action changed something).
    poke: bool,
}

static SAID: Mutex<Option<Said>> = Mutex::new(None);
static WAKE: Condvar = Condvar::new();

fn with_said<T>(f: impl FnOnce(&mut Said) -> T) -> T {
    f(SAID.lock().unwrap().get_or_insert_with(Said::default))
}

// Look again at once (after an action, or a tool put out).
pub fn poke() {
    with_said(|s| s.poke = true);
    WAKE.notify_all();
}

// As last told, for a page that starts.
pub fn state() -> Value {
    with_said(|s| s.state.clone())
}

// The thread: each second while the media widget is out, else every five.
pub fn run(sh: std::sync::Arc<Shared>) {
    crate::power::thread_eco();
    imp::init_thread();
    let mut media = imp::Media::default();
    let (mut trash_at, mut disk_at): (Option<Instant>, Option<Instant>) = (None, None);
    let (mut trash, mut disk) = (Value::Null, Value::Null);
    loop {
        let kinds = placed(&sh);
        let has = |k: &str| kinds.iter().any(|x| x == k);
        let (asked, poked) = with_said(|s| (std::mem::take(&mut s.asked), std::mem::replace(&mut s.poke, false)));
        for what in &asked {
            media.press(what);
        }
        let now_media = if has("media") { media.read() } else { Value::Null };
        if !has("trash") {
            (trash, trash_at) = (Value::Null, None);
        } else if poked || trash_at.is_none_or(|t| t.elapsed() >= Duration::from_secs(5)) {
            (trash, trash_at) = (imp::trash(), Some(Instant::now()));
        }
        if !has("disk") {
            (disk, disk_at) = (Value::Null, None);
        } else if poked || disk_at.is_none_or(|t| t.elapsed() >= Duration::from_secs(60)) {
            (disk, disk_at) = (imp::disks(), Some(Instant::now()));
        }
        let state = json!({ "media": now_media, "trash": trash, "disk": disk, "awake": awake_state() });
        let changed = with_said(|s| {
            let changed = s.state != state;
            s.state = state.clone();
            changed
        });
        if changed {
            let _ = sh.app.emit_to("island", "tools:state", &state);
        }
        // Each second with the media widget out (or asked to), quiet ones less.
        let wait = if has("media") && !sh.is_quiet() { 1 } else if kinds.is_empty() { 10 } else { 5 };
        let guard = SAID.lock().unwrap();
        let _ = WAKE.wait_timeout_while(guard, Duration::from_secs(wait), |s| s.as_ref().is_none_or(|s| !s.poke && s.asked.is_empty()));
    }
}

// --- What the page asks for (tool_do) -------------------------------------------------

pub fn act(sh: &std::sync::Arc<Shared>, kind: &str, what: &str, arg: &str) -> bool {
    let done = match (kind, what) {
        ("media", "prev" | "toggle" | "next") => {
            with_said(|s| s.asked.push(what.to_string()));
            WAKE.notify_all();
            true
        }
        ("trash", "open") => crate::tasks::launch("shell:RecycleBinFolder"),
        ("trash", "empty") => imp::empty_trash(),
        ("disk", "open") => !arg.is_empty() && crate::tasks::launch(arg),
        ("power", what) => imp::power(what),
        ("awake", "on") => {
            set_awake(sh, Some(arg.parse::<u64>().unwrap_or(0)));
            true
        }
        ("awake", "off") => {
            set_awake(sh, None);
            true
        }
        ("launch", "open") => !arg.trim().is_empty() && crate::tasks::launch(arg.trim()),
        ("picker", "start") => {
            imp::pick(sh.clone());
            true
        }
        ("picker", "copy") | ("calc", "copy") | ("notes", "copy") => imp::copy(arg),
        ("timer", "done") => crate::notify::show(arg).is_ok(),
        _ => false,
    };
    sh.log(&format!("tool: {kind} {what} {}", if done { "done" } else { "not done" }));
    poke();
    done
}

// --- Awake ------------------------------------------------------------------------

// On (until when, ms; 0 for as long as it is on), or off.
static AWAKE: Mutex<Option<u64>> = Mutex::new(None);
static HOLDING: AtomicBool = AtomicBool::new(false);

fn awake_state() -> Value {
    match *AWAKE.lock().unwrap() {
        Some(until) => json!({ "on": true, "until": if until == 0 { Value::Null } else { json!(until) } }),
        None => json!({ "on": false }),
    }
}

// A thread of its own holds the computer and the screen awake (Windows
// keeps the state per thread) for as long as it runs; off, or its time up,
// it lets go and ends.
// HOLDING is only changed with AWAKE locked: a thread letting go and a
// new on never miss each other.
fn set_awake(sh: &std::sync::Arc<Shared>, minutes: Option<u64>) {
    let mut awake = AWAKE.lock().unwrap();
    *awake = minutes.map(|m| if m == 0 { 0 } else { crate::now_ms() + m * 60_000 });
    if awake.is_none() || HOLDING.swap(true, Ordering::SeqCst) {
        return;
    }
    drop(awake);
    let sh = sh.clone();
    crate::spawn_named("awake", move || {
        imp::hold_awake(true);
        loop {
            {
                let mut awake = AWAKE.lock().unwrap();
                if !awake.is_some_and(|until| until == 0 || crate::now_ms() < until) {
                    *awake = None;
                    HOLDING.store(false, Ordering::SeqCst);
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        imp::hold_awake(false);
        sh.log("awake: let go");
        poke();
    });
}

// An app's id as Windows' media controls give it, as a name: "Spotify.exe"
// Spotify, "Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic"
// ZuneMusic, "308046B0AF4A39CB" (a browser) as it is.
pub fn app_name(id: &str) -> String {
    let id = id.rsplit('!').next().unwrap_or(id);
    let id = id.rsplit(['\\', '/']).next().unwrap_or(id);
    let id = id.strip_suffix(".exe").or_else(|| id.strip_suffix(".EXE")).unwrap_or(id);
    let id = if id.contains('.') { id.rsplit('.').next().unwrap_or(id) } else { id };
    let mut chars = id.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

// A colour as the page writes it.
pub fn hex(colorref: u32) -> String {
    let (r, g, b) = (colorref & 0xff, (colorref >> 8) & 0xff, (colorref >> 16) & 0xff);
    format!("#{r:02x}{g:02x}{b:02x}")
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

    use serde_json::{json, Value};
    use tauri::Emitter;
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSessionManager as Manager, GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
    };

    use crate::Shared;

    #[repr(C)]
    struct RecycleInfo {
        size: u32,
        bytes: i64,
        items: i64,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    struct MouseHook {
        pt: Point,
        data: u32,
        flags: u32,
        time: u32,
        extra: usize,
    }

    #[repr(C)]
    struct Msg {
        hwnd: *mut c_void,
        message: u32,
        wparam: usize,
        lparam: isize,
        time: u32,
        pt: Point,
    }

    type HookProc = extern "system" fn(i32, usize, isize) -> isize;

    #[link(name = "shell32")]
    extern "system" {
        fn SHQueryRecycleBinW(root: *const u16, info: *mut RecycleInfo) -> i32;
        fn SHEmptyRecycleBinW(hwnd: *mut c_void, root: *const u16, flags: u32) -> i32;
    }
    // Declared elsewhere too, with handles as numbers: the same functions.
    #[allow(clashing_extern_declarations)]
    #[link(name = "user32")]
    extern "system" {
        fn LockWorkStation() -> i32;
        fn ExitWindowsEx(flags: u32, reason: u32) -> i32;
        fn OpenClipboard(hwnd: *mut c_void) -> i32;
        fn EmptyClipboard() -> i32;
        fn SetClipboardData(format: u32, mem: *mut c_void) -> *mut c_void;
        fn CloseClipboard() -> i32;
        fn SetWindowsHookExW(id: i32, proc: HookProc, module: *mut c_void, thread: u32) -> *mut c_void;
        fn UnhookWindowsHookEx(hook: *mut c_void) -> i32;
        fn CallNextHookEx(hook: *mut c_void, code: i32, wparam: usize, lparam: isize) -> isize;
        fn GetMessageW(msg: *mut Msg, hwnd: *mut c_void, min: u32, max: u32) -> i32;
        fn PostQuitMessage(code: i32);
        fn SetTimer(hwnd: *mut c_void, id: usize, ms: u32, proc: *const c_void) -> usize;
        fn GetDC(hwnd: *mut c_void) -> *mut c_void;
        fn ReleaseDC(hwnd: *mut c_void, dc: *mut c_void) -> i32;
    }
    #[link(name = "gdi32")]
    extern "system" {
        fn GetPixel(dc: *mut c_void, x: i32, y: i32) -> u32;
    }
    #[allow(clashing_extern_declarations)]
    #[link(name = "kernel32")]
    extern "system" {
        fn SetThreadExecutionState(flags: u32) -> u32;
        fn GetLogicalDrives() -> u32;
        fn GetDriveTypeW(root: *const u16) -> u32;
        fn GetDiskFreeSpaceExW(root: *const u16, free: *mut u64, total: *mut u64, all_free: *mut u64) -> i32;
        fn GetVolumeInformationW(root: *const u16, name: *mut u16, name_len: u32, serial: *mut u32, max: *mut u32, flags: *mut u32, fs: *mut u16, fs_len: u32) -> i32;
        fn GetModuleHandleW(name: *const u16) -> *mut c_void;
        fn GlobalAlloc(flags: u32, bytes: usize) -> *mut c_void;
        fn GlobalLock(mem: *mut c_void) -> *mut c_void;
        fn GlobalUnlock(mem: *mut c_void) -> i32;
        fn GlobalFree(mem: *mut c_void) -> *mut c_void;
        fn SetErrorMode(mode: u32) -> u32;
    }
    #[link(name = "powrprof")]
    extern "system" {
        fn SetSuspendState(hibernate: u8, force: u8, no_wake_events: u8) -> u8;
    }
    #[link(name = "ole32")]
    extern "system" {
        fn CoInitializeEx(reserved: *mut c_void, coinit: u32) -> i32;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    // Windows' media controls are WinRT: the thread in the multithreaded apartment.
    pub fn init_thread() {
        // SAFETY: once, on this thread; COINIT_MULTITHREADED.
        unsafe { CoInitializeEx(std::ptr::null_mut(), 0) };
    }

    // --- Media ---------------------------------------------------------------

    #[derive(Default)]
    pub struct Media {
        manager: Option<Manager>,
    }

    impl Media {
        fn manager(&mut self) -> Option<&Manager> {
            if self.manager.is_none() {
                self.manager = Manager::RequestAsync().and_then(|op| op.join()).ok();
            }
            self.manager.as_ref()
        }

        // What plays now: its title, artist, app, whether it plays, and
        // which buttons it takes; null with nothing that does.
        pub fn read(&mut self) -> Value {
            let Some(session) = self.manager().and_then(|m| m.GetCurrentSession().ok()) else { return Value::Null };
            let props = session.TryGetMediaPropertiesAsync().and_then(|op| op.join()).ok();
            let text = |f: &dyn Fn(&windows::Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties) -> windows_core::Result<windows_core::HSTRING>| {
                props.as_ref().and_then(|p| f(p).ok()).map(|h| h.to_string()).unwrap_or_default()
            };
            let (title, artist) = (text(&|p| p.Title()), text(&|p| p.Artist()));
            let info = session.GetPlaybackInfo().ok();
            let playing = info.as_ref().and_then(|i| i.PlaybackStatus().ok()) == Some(Status::Playing);
            let controls = info.as_ref().and_then(|i| i.Controls().ok());
            let can = |f: &dyn Fn(&windows::Media::Control::GlobalSystemMediaTransportControlsSessionPlaybackControls) -> windows_core::Result<bool>| {
                controls.as_ref().and_then(|c| f(c).ok()).unwrap_or(false)
            };
            let app = session.SourceAppUserModelId().map(|h| super::app_name(&h.to_string())).unwrap_or_default();
            json!({
                "title": title,
                "artist": artist,
                "app": app,
                "playing": playing,
                "prev": can(&|c| c.IsPreviousEnabled()),
                "next": can(&|c| c.IsNextEnabled()),
                "toggle": can(&|c| c.IsPlayPauseToggleEnabled()),
            })
        }

        pub fn press(&mut self, what: &str) {
            let Some(session) = self.manager().and_then(|m| m.GetCurrentSession().ok()) else { return };
            let op = match what {
                "prev" => session.TrySkipPreviousAsync(),
                "next" => session.TrySkipNextAsync(),
                _ => session.TryTogglePlayPauseAsync(),
            };
            let _ = op.and_then(|op| op.join());
        }
    }

    // --- The Recycle Bin, drives ---------------------------------------------

    pub fn trash() -> Value {
        let mut info = RecycleInfo { size: std::mem::size_of::<RecycleInfo>() as u32, bytes: 0, items: 0 };
        // SAFETY: our own struct, its size given; all drives' bins.
        let ok = unsafe { SHQueryRecycleBinW(std::ptr::null(), &mut info) } >= 0;
        if ok { json!({ "items": info.items, "bytes": info.bytes }) } else { Value::Null }
    }

    pub fn empty_trash() -> bool {
        // SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND: the page asked.
        // SAFETY: all drives' bins, no window.
        unsafe { SHEmptyRecycleBinW(std::ptr::null_mut(), std::ptr::null(), 0x7) >= 0 }
    }

    pub fn disks() -> Value {
        // SAFETY: a plain query.
        let mask = unsafe { GetLogicalDrives() };
        // An empty card reader not asked to insert a disk.
        // SAFETY: SEM_FAILCRITICALERRORS, put back after.
        let mode = unsafe { SetErrorMode(1) };
        let mut out = Vec::new();
        for i in 0..26u32 {
            if mask & (1 << i) == 0 {
                continue;
            }
            let letter = (b'A' + i as u8) as char;
            let root = wide(&format!("{letter}:\\"));
            // SAFETY: a root of our own; DRIVE_REMOVABLE 2, DRIVE_FIXED 3.
            let kind = unsafe { GetDriveTypeW(root.as_ptr()) };
            if kind != 2 && kind != 3 {
                continue;
            }
            let (mut free, mut total, mut all_free) = (0u64, 0u64, 0u64);
            // SAFETY: our own numbers.
            if unsafe { GetDiskFreeSpaceExW(root.as_ptr(), &mut free, &mut total, &mut all_free) } == 0 || total == 0 {
                continue;
            }
            let mut name = [0u16; 128];
            // SAFETY: our own buffer, its length given; the rest not wanted.
            let named = unsafe {
                GetVolumeInformationW(root.as_ptr(), name.as_mut_ptr(), name.len() as u32, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), 0)
            } != 0;
            let label = if named { String::from_utf16_lossy(&name[..name.iter().position(|&c| c == 0).unwrap_or(0)]) } else { String::new() };
            out.push(json!({ "drive": format!("{letter}:"), "label": label, "free": free, "total": total, "removable": kind == 2 }));
        }
        // SAFETY: as it was.
        unsafe { SetErrorMode(mode) };
        Value::Array(out)
    }

    // --- Awake, power --------------------------------------------------------

    pub fn hold_awake(on: bool) {
        // ES_CONTINUOUS, and ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED while on.
        // SAFETY: this thread's own state.
        unsafe { SetThreadExecutionState(if on { 0x8000_0003 } else { 0x8000_0000 }) };
    }

    pub fn power(what: &str) -> bool {
        // SAFETY: plain requests to Windows; it may refuse.
        unsafe {
            match what {
                "lock" => LockWorkStation() != 0,
                "sleep" => SetSuspendState(0, 0, 0) != 0,
                // EWX_LOGOFF.
                "signout" => ExitWindowsEx(0, 0) != 0,
                "restart" | "shutdown" => {
                    use std::os::windows::process::CommandExt;
                    std::process::Command::new("shutdown.exe")
                        .args([if what == "restart" { "/r" } else { "/s" }, "/t", "0"])
                        .creation_flags(0x0800_0000)
                        .spawn()
                        .is_ok()
                }
                _ => false,
            }
        }
    }

    // --- The clipboard ---------------------------------------------------------

    pub fn copy(text: &str) -> bool {
        let units = super::wide_text(text);
        // SAFETY: the clipboard opened (a few tries: another program may
        // hold it a moment), our moveable memory handed to it on success
        // and freed by us on failure.
        unsafe {
            let mut opened = false;
            for _ in 0..10 {
                if OpenClipboard(std::ptr::null_mut()) != 0 {
                    opened = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            if !opened {
                return false;
            }
            EmptyClipboard();
            let bytes = units.len() * 2;
            // GMEM_MOVEABLE.
            let mem = GlobalAlloc(0x2, bytes);
            let mut ok = false;
            if !mem.is_null() {
                let to = GlobalLock(mem) as *mut u16;
                if !to.is_null() {
                    std::ptr::copy_nonoverlapping(units.as_ptr(), to, units.len());
                    GlobalUnlock(mem);
                    // CF_UNICODETEXT.
                    ok = !SetClipboardData(13, mem).is_null();
                }
                if !ok {
                    GlobalFree(mem);
                }
            }
            CloseClipboard();
            ok
        }
    }

    // --- The colour picker -------------------------------------------------------

    static PICKING: AtomicBool = AtomicBool::new(false);
    // Where the click came, or that it was a right one (given up); and
    // that its release is ours to keep too.
    static PICKED: Mutex<Option<Option<(i32, i32)>>> = Mutex::new(None);
    static SWALLOW_UP: AtomicBool = AtomicBool::new(false);

    const WM_LBUTTONDOWN: usize = 0x201;
    const WM_LBUTTONUP: usize = 0x202;
    const WM_RBUTTONDOWN: usize = 0x204;
    const WM_RBUTTONUP: usize = 0x205;

    extern "system" fn hook(code: i32, wparam: usize, lparam: isize) -> isize {
        if code >= 0 {
            // SAFETY: a low-level mouse hook's lparam is its MSLLHOOKSTRUCT.
            let info = unsafe { &*(lparam as *const MouseHook) };
            match wparam {
                WM_LBUTTONDOWN => {
                    *PICKED.lock().unwrap() = Some(Some((info.pt.x, info.pt.y)));
                    SWALLOW_UP.store(true, Ordering::SeqCst);
                    return 1;
                }
                WM_RBUTTONDOWN => {
                    *PICKED.lock().unwrap() = Some(None);
                    SWALLOW_UP.store(true, Ordering::SeqCst);
                    return 1;
                }
                WM_LBUTTONUP | WM_RBUTTONUP if SWALLOW_UP.swap(false, Ordering::SeqCst) => {
                    // SAFETY: this thread's own loop ends.
                    unsafe { PostQuitMessage(0) };
                    return 1;
                }
                _ => {}
            }
        }
        // SAFETY: passed on as Windows asks.
        unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
    }

    pub fn pick(sh: std::sync::Arc<Shared>) {
        if PICKING.swap(true, Ordering::SeqCst) {
            return;
        }
        *PICKED.lock().unwrap() = None;
        SWALLOW_UP.store(false, Ordering::SeqCst);
        crate::spawn_named("picker", move || {
            // SAFETY: the hook on this thread, which pumps its messages until
            // the click is in (or 20 seconds pass: WM_TIMER), then taken off.
            let at = unsafe {
                let hook_handle = SetWindowsHookExW(14, hook, GetModuleHandleW(std::ptr::null()), 0);
                if hook_handle.is_null() {
                    None
                } else {
                    SetTimer(std::ptr::null_mut(), 0, 20_000, std::ptr::null());
                    let mut msg: Msg = std::mem::zeroed();
                    while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                        // WM_TIMER: given up.
                        if msg.message == 0x113 {
                            break;
                        }
                    }
                    UnhookWindowsHookEx(hook_handle);
                    PICKED.lock().unwrap().take().flatten()
                }
            };
            let hex = at.and_then(|(x, y)| {
                // SAFETY: the screen's DC, released; CLR_INVALID off it.
                let colour = unsafe {
                    let dc = GetDC(std::ptr::null_mut());
                    let c = GetPixel(dc, x, y);
                    ReleaseDC(std::ptr::null_mut(), dc);
                    c
                };
                (colour != 0xFFFF_FFFF).then(|| super::hex(colour))
            });
            if let Some(hex) = &hex {
                copy(hex);
            }
            PICKING.store(false, Ordering::SeqCst);
            sh.log(&format!("picker: {}", hex.as_deref().unwrap_or("given up")));
            let _ = sh.app.emit_to("island", "tools:picked", json!({ "hex": hex }));
        });
    }
}

#[cfg(not(windows))]
mod imp {
    use serde_json::Value;

    pub fn init_thread() {}
    #[derive(Default)]
    pub struct Media;
    impl Media {
        pub fn read(&mut self) -> Value {
            Value::Null
        }
        pub fn press(&mut self, _what: &str) {}
    }
    pub fn trash() -> Value {
        Value::Null
    }
    pub fn empty_trash() -> bool {
        false
    }
    pub fn disks() -> Value {
        Value::Null
    }
    pub fn hold_awake(_on: bool) {}
    pub fn power(_what: &str) -> bool {
        false
    }
    pub fn copy(_text: &str) -> bool {
        false
    }
    pub fn pick(_sh: std::sync::Arc<crate::Shared>) {}
}

// Text as the clipboard takes it: UTF-16, null-terminated, its line ends
// Windows' own.
pub fn wide_text(text: &str) -> Vec<u16> {
    text.replace("\r\n", "\n").replace('\n', "\r\n").encode_utf16().chain(Some(0)).collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn the_kinds_put_somewhere() {
        let mut out = Vec::new();
        let layout = json!({ "left": ["her"], "right": [{ "id": "wa", "type": "tool", "of": "media" }, { "id": "wb", "type": "stack", "items": [{ "id": "wc", "type": "tool", "of": "trash" }, { "id": "wd", "type": "tool", "of": "media" }] }] });
        super::find_kinds(&layout, &mut out);
        assert_eq!(out, ["media", "trash"]);
    }

    #[test]
    fn an_apps_id_as_a_name() {
        assert_eq!(super::app_name("Spotify.exe"), "Spotify");
        assert_eq!(super::app_name("Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic"), "ZuneMusic");
        assert_eq!(super::app_name("C:\\Program Files\\Netease\\CloudMusic\\cloudmusic.exe"), "Cloudmusic");
        assert_eq!(super::app_name("chrome"), "Chrome");
        assert_eq!(super::app_name(""), "");
    }

    #[test]
    fn a_colour_as_the_page_writes_it() {
        // COLORREF is 0x00bbggrr.
        assert_eq!(super::hex(0x00ff8000), "#0080ff");
        assert_eq!(super::hex(0), "#000000");
    }

    #[test]
    fn text_for_the_clipboard() {
        assert_eq!(super::wide_text("a\nb"), "a\r\nb\0".encode_utf16().collect::<Vec<_>>());
        assert_eq!(super::wide_text("a\r\nb"), "a\r\nb\0".encode_utf16().collect::<Vec<_>>());
    }
}

// What this machine says, read only (cargo test -- --ignored this_machine).
#[cfg(all(test, windows))]
mod this_machine {
    #[test]
    #[ignore]
    fn this_machine_reads() {
        super::imp::init_thread();
        let disks = super::imp::disks();
        println!("disks: {disks}");
        assert!(disks.as_array().is_some_and(|d| !d.is_empty()), "no drive read");
        println!("trash: {}", super::imp::trash());
        println!("media: {}", super::imp::Media::default().read());
    }
}
