// Other programs' icons in the notification area (the tray), taken over from
// Explorer while hers is the taskbar. A program hands its icon to the first
// window of class Shell_TrayWnd that FindWindow finds (Shell_NotifyIcon, as
// a WM_COPYDATA), so one of ours is put ahead of Explorer's (topmost, and put
// back there when Explorer's moves up). Each message is handed on to
// Explorer's as well, so its tray stays whole for when the taskbar is given
// back; the same for what else comes to the taskbar's window (app bars,
// SHAppBarMessage, come this way too). Programs that added their icons before
// are asked to add them again, with the message Explorer sends when it starts.
// Given back (or she was killed holding it), every window is told so again
// once ours is gone and Explorer's shows (handed_back): whatever took ours
// for the taskbar on hearing it finds Explorer's (zhoupengjie/Wakuwaku#54).
//
// A press on an icon is told to its program as Explorer tells it, by the
// version of the protocol it asked for (NIM_SETVERSION). Windows only.
//
// Some programs never hand theirs again (zhoupengjie/Wakuwaku#52): the
// NVIDIA App's (NvUI.dll in nvcontainer.exe, run as SYSTEM in the person's
// session) adds it once, when a timer first finds a Shell_TrayWnd, and has
// no TaskbarCreated at all, which could not reach a SYSTEM window from hers
// anyway; ACE-Tray's window is message-only and has none either; Windows
// Security's answers TaskbarCreated with a NIM_MODIFY, for it had added its
// icon already (to Explorer's). Explorer's tray, handed everything, holds
// them, and Explorer itself loses them the same way when it restarts. So an
// icon changed that ours does not have is taken from the change when
// Explorer's says it holds it, and an icon Explorer's holds that never came
// at all is shown from Windows' own record of it (held, scan_held), until
// its program speaks to ours. Neither can be pressed unless the program
// named its callback message to ours: only an add carries it.
//
// The window has to be made on a thread that runs a message loop: start()
// makes it on the caller's; what changes is posted to a window of the
// caller's (notify).

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Press {
    LeftDown,
    LeftUp,
    RightDown,
    RightUp,
    Double,
    Move,
    HoverIn,
    HoverOut,
}

// An icon as shown: its key (for tell), its icon (a copy of ours; 0 while
// it blinks dark), its tip, the program it is from.
pub struct Shown {
    pub key: u64,
    pub icon: isize,
    pub tip: String,
    pub exe: String,
    // Its id among its program's icons (uID), and a name for it that stays
    // from one run to the next (the program and its guid or id), for where
    // the person keeps it (in the taskbar, or folded away).
    pub uid: u32,
    pub name: String,
    // Its guid, when its program names it by one (where Windows keeps it).
    pub guid: Option<[u8; 16]>,
    // Whether a press on it reaches its program: not when the program never
    // told ours which message to send (an icon taken from a change, or one
    // only Explorer's holds).
    pub reach: bool,
    // Windows' own picture of it (a PNG, from its record), for an icon ours
    // was never handed; empty otherwise.
    pub snapshot: Vec<u8>,
    // Kept out on Windows' taskbar, as its record says, for such an icon.
    pub out: Option<bool>,
}

// What shell32 hands the tray with an add (seen 2026-10-15 on 26300): 520
// bytes after the 964 of SHELLTRAYDATA, the program's path (as Windows keeps
// it in NotifyIconSettings: a known folder's guid for its start); Explorer
// refuses an add without it. Empty in the other messages.
#[cfg_attr(not(windows), allow(dead_code))]
const PATH_AT: usize = 964;

#[cfg_attr(not(windows), allow(dead_code))]
fn path_in(copied: &[u8]) -> Option<String> {
    let rest = copied.get(PATH_AT..)?;
    let units: Vec<u16> = rest.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take(260).take_while(|&u| u != 0).collect();
    (!units.is_empty()).then(|| String::from_utf16_lossy(&units))
}

// A program's file name from its path, either kind of slash.
#[cfg_attr(not(windows), allow(dead_code))]
fn file_name(path: &str) -> String {
    path.rsplit(['\\', '/']).next().unwrap_or(path).to_string()
}

// A guid as Windows writes it ({7120A42D-CB02-4A51-B3B5-58E36E8B14E1}),
// as its 16 bytes are in memory (the first three parts little-endian).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn guid_bytes(text: &str) -> Option<[u8; 16]> {
    let hex: String = text.trim().trim_start_matches('{').trim_end_matches('}').split('-').collect();
    if hex.len() != 32 {
        return None;
    }
    let mut raw = [0u8; 16];
    for (i, byte) in raw.iter_mut().enumerate() {
        *byte = u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    let mut out = raw;
    out[..4].copy_from_slice(&[raw[3], raw[2], raw[1], raw[0]]);
    out[4..6].copy_from_slice(&[raw[5], raw[4]]);
    out[6..8].copy_from_slice(&[raw[7], raw[6]]);
    Some(out)
}

// An icon's name that stays from one run to the next: its program, and its
// guid or its id.
#[cfg_attr(not(windows), allow(dead_code))]
fn icon_name(exe: &str, guid: Option<[u8; 16]>, id: u32) -> String {
    let which = guid.map_or_else(|| id.to_string(), |g| g.iter().map(|b| format!("{b:02x}")).collect());
    format!("{}:{which}", exe.to_lowercase())
}

// What Explorer's tray answers when asked where an icon is (message 3): 0
// for one it does not hold, all ones for one it holds hidden (NIS_HIDDEN),
// else a corner of where it is (in its folded ones: where ^ is).
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(PartialEq, Debug)]
enum Holds {
    No,
    Hidden,
    Shown,
}

#[cfg_attr(not(windows), allow(dead_code))]
fn holds(answer: usize) -> Holds {
    match answer as u32 {
        0 => Holds::No,
        u32::MAX => Holds::Hidden,
        _ => Holds::Shown,
    }
}

// Explorer's own battery icon (uID 1225, named by a guid, its tip 电池指示器,
// Battery Meter), which Explorer adds to its tray itself on 22621 (Windows
// 11 22H2 and 23H2). On explorer.exe 10.0.22621.2792 (zhoupengjie/Wakuwaku#62)
// Explorer failed fast (0xc0000409 in ucrtbase.dll) seconds after ours took
// it, and started again, its taskbar shown and put away again: the screen
// flickered every three seconds. The reporter's build that left Explorer's
// own icons to its tray (every message handed on, its answer given back)
// ran without a crash. On 26100 its battery is no tray icon at all (its
// taskbar draws it), and its other icons (1226, safe removal; the
// Bluetooth one; a headset's battery; a microphone in use) pass through
// ours without harm, so this one alone is left to it. Hers draws the
// battery itself (widgets.rs battery).
#[cfg_attr(not(windows), allow(dead_code))]
const EXPLORERS_BATTERY: u32 = 1225;

// Whether an icon is Explorer's battery: its uID, from a window of a
// process by that name (exe: as exe_of names it).
#[cfg_attr(not(windows), allow(dead_code))]
fn explorers_battery(id: u32, exe: &str) -> bool {
    id == EXPLORERS_BATTERY && exe.eq_ignore_ascii_case("explorer.exe")
}

// The window tray-icon keeps her own tray icon on (tauri's tray).
#[cfg_attr(not(windows), allow(dead_code))]
const HER_TRAY_ICON: &str = "tray_icon_app";

// Whether a window is told TaskbarCreated, as programs are asked for their
// icons: not Explorer's taskbars, which show themselves; of hers only her
// tray icon's, her taskbar's own window hearing it as Explorer started again.
// Hers had been left out (2026-10-10), and her icon, added while Explorer's
// tray was first (she starts before her taskbar is up), never came to hers
// but from a change, which names no callback: a press on it did nothing.
#[cfg_attr(not(windows), allow(dead_code))]
fn asked(own: bool, class: &str) -> bool {
    class != "Shell_TrayWnd" && class != "Shell_SecondaryTrayWnd" && (!own || class == HER_TRAY_ICON)
}

// A message of Explorer's battery handed on to its tray, for the log
// (zhoupengjie/Wakuwaku#85: on 22621 Explorer failed fast a moment after
// one went by ours while it was starting, its icon handed on as #62 has it;
// whether its tray took it then, and how fast, is what this says): which
// message, the Explorer and how long it had run (s), what its tray answered
// (None: not at all; error, GetLastError's then), and how long it took.
#[cfg_attr(not(windows), allow(dead_code))]
fn battery_said(kind: &str, pid: u32, up: Option<f64>, tray: bool, answer: Option<usize>, error: u32, ms: u128) -> String {
    let up = up.map_or_else(|| "gone".to_string(), |s| format!("up {s:.1} s"));
    let how = match answer {
        _ if !tray => "no tray of Explorer's to hand it to".to_string(),
        Some(a) => format!("answered {a}"),
        None => format!("not answered (error {error})"),
    };
    format!("tray: Explorer's battery {kind} (pid {pid}, {up}) handed on: {how} in {ms} ms")
}

// A tray message's name, as the log names it.
#[cfg_attr(not(windows), allow(dead_code))]
fn nim_name(message: u32) -> &'static str {
    match message {
        0 => "NIM_ADD",
        1 => "NIM_MODIFY",
        2 => "NIM_DELETE",
        4 => "NIM_SETVERSION",
        _ => "NIM_other",
    }
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, AtomicU64, Ordering};
    use std::sync::Mutex;

    use super::{asked, battery_said, explorers_battery, file_name, guid_bytes, holds, icon_name, nim_name, path_in, Holds, Press, Shown, EXPLORERS_BATTERY};

    type Hwnd = *mut c_void;
    type Handle = isize;

    #[repr(C)]
    struct CopyData {
        data: usize,
        size: u32,
        ptr: *const c_void,
    }

    // NOTIFYICONDATAW as the tray is handed it: the same from 32- and 64-bit
    // programs, its handles in 32 bits (they mean the same in every process).
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct IconData {
        size: u32,
        hwnd: u32,
        id: u32,
        flags: u32,
        callback: u32,
        icon: u32,
        tip: [u16; 128],
        state: u32,
        state_mask: u32,
        info: [u16; 256],
        version: u32,
        info_title: [u16; 64],
        info_flags: u32,
        guid: [u8; 16],
        balloon_icon: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct TrayData {
        signature: u32,
        message: u32,
        nid: IconData,
    }

    // What Shell_NotifyIconGetRect hands the tray (asked twice: message 1
    // for the left and top, 2 for the right and bottom): 40 bytes, the window
    // in 32 bits (seen 2026-10-10: 23 34 75 34 | 02 00 00 00 | 20 00 00 00 |
    // 00 00 00 00 | window | id | guid).
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct RectQuestion {
        signature: u32,
        message: u32,
        size: u32,
        padding: u32,
        hwnd: u32,
        id: u32,
        guid: [u8; 16],
    }

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

    #[link(name = "user32")]
    extern "system" {
        fn RegisterClassExW(class: *const WndClassExW) -> u16;
        fn CreateWindowExW(ex: u32, class: *const u16, title: *const u16, style: u32, x: i32, y: i32, w: i32, h: i32, parent: Hwnd, menu: isize, instance: isize, param: *mut c_void) -> Hwnd;
        fn DestroyWindow(hwnd: Hwnd) -> i32;
        fn DefWindowProcW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize;
        fn FindWindowW(class: *const u16, title: *const u16) -> Hwnd;
        fn FindWindowExW(parent: Hwnd, after: Hwnd, class: *const u16, title: *const u16) -> Hwnd;
        fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
        fn SetWindowPos(hwnd: Hwnd, after: isize, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
        fn SendMessageTimeoutW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize, flags: u32, ms: u32, result: *mut usize) -> isize;
        fn SendNotifyMessageW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> i32;
        fn PostMessageW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> i32;
        fn RegisterWindowMessageW(name: *const u16) -> u32;
        fn IsWindow(hwnd: Hwnd) -> i32;
        fn EnumWindows(each: extern "system" fn(Hwnd, isize) -> i32, param: isize) -> i32;
        fn GetClassNameW(hwnd: Hwnd, name: *mut u16, max: i32) -> i32;
        fn CopyIcon(icon: isize) -> isize;
        fn DestroyIcon(icon: isize) -> i32;
        fn AllowSetForegroundWindow(pid: u32) -> i32;
        fn SetForegroundWindow(hwnd: Hwnd) -> i32;
        fn GetForegroundWindow() -> Hwnd;
        fn InSendMessageEx(reserved: *mut c_void) -> u32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleW(name: *const u16) -> isize;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
        fn CloseHandle(handle: Handle) -> i32;
        fn QueryFullProcessImageNameW(process: Handle, flags: u32, name: *mut u16, size: *mut u32) -> i32;
        fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> Handle;
        fn Process32FirstW(snapshot: Handle, entry: *mut ProcessEntry) -> i32;
        fn Process32NextW(snapshot: Handle, entry: *mut ProcessEntry) -> i32;
        fn GetLastError() -> u32;
    }

    // PROCESSENTRY32W: every process's file name, SYSTEM's too, which
    // OpenProcess does not give a person's process (nvcontainer.exe's).
    #[repr(C)]
    struct ProcessEntry {
        size: u32,
        usage: u32,
        pid: u32,
        heap: usize,
        module: u32,
        threads: u32,
        parent: u32,
        priority: i32,
        flags: u32,
        exe: [u16; 260],
    }

    const WM_COMMAND: u32 = 0x0111;
    const WM_SYSCOMMAND: u32 = 0x0112;
    const WM_HOTKEY: u32 = 0x0312;
    const WM_CLOSE: u32 = 0x0010;
    const WM_MOVE: u32 = 0x0003;
    const WM_SIZE: u32 = 0x0005;
    const WM_GETMINMAXINFO: u32 = 0x0024;
    const WM_WINDOWPOSCHANGING: u32 = 0x0046;
    const WM_WINDOWPOSCHANGED: u32 = 0x0047;
    const WM_NCCALCSIZE: u32 = 0x0083;
    const ISMEX_SEND: u32 = 0x1;
    const ISMEX_NOTIFY: u32 = 0x2;
    const ISMEX_CALLBACK: u32 = 0x4;
    const WM_COPYDATA: u32 = 0x004A;
    const WM_USER: u32 = 0x0400;
    const WM_CONTEXTMENU: u32 = 0x007B;
    const WM_MOUSEMOVE: u32 = 0x0200;
    const WM_LBUTTONDOWN: u32 = 0x0201;
    const WM_LBUTTONUP: u32 = 0x0202;
    const WM_LBUTTONDBLCLK: u32 = 0x0203;
    const WM_RBUTTONDOWN: u32 = 0x0204;
    const WM_RBUTTONUP: u32 = 0x0205;
    const NIN_SELECT: u32 = WM_USER;
    const NIN_POPUPOPEN: u32 = WM_USER + 6;
    const NIN_POPUPCLOSE: u32 = WM_USER + 7;
    const NIM_ADD: u32 = 0;
    const NIM_MODIFY: u32 = 1;
    const NIM_DELETE: u32 = 2;
    const NIM_SETVERSION: u32 = 4;
    const NIF_MESSAGE: u32 = 0x1;
    const NIF_ICON: u32 = 0x2;
    const NIF_TIP: u32 = 0x4;
    const NIF_STATE: u32 = 0x8;
    const NIF_GUID: u32 = 0x20;
    const NIS_HIDDEN: u32 = 0x1;
    const SIGNATURE: u32 = 0x3475_3423;
    const WS_POPUP: u32 = 0x8000_0000;
    const WS_EX_TOPMOST: u32 = 0x8;
    const WS_EX_TOOLWINDOW: u32 = 0x80;
    const HWND_TOPMOST: isize = -1;
    const HWND_NOTOPMOST: isize = -2;
    const HWND_BOTTOM: isize = 1;
    const SWP_NOZORDER: u32 = 0x4;
    const SWP_NOSIZE: u32 = 0x1;
    const SWP_NOMOVE: u32 = 0x2;
    const SWP_NOACTIVATE: u32 = 0x10;
    const SMTO_ABORTIFHUNG: u32 = 0x2;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const HWND_MESSAGE: isize = -3;
    const TH32CS_SNAPPROCESS: u32 = 0x2;
    const INVALID_HANDLE: Handle = -1;

    struct Icon {
        key: u64,
        hwnd: u32,
        id: u32,
        guid: Option<[u8; 16]>,
        callback: u32,
        icon: isize,
        tip: String,
        version: u32,
        hidden: bool,
        pid: u32,
        exe: String,
        modified: u32,
        // It has had an icon: its place is kept while it has none (blinking).
        had_icon: bool,
        // Taken from a change, its add having gone to Explorer's: a press
        // reaches it once a message names its callback.
        taken: bool,
    }

    static OURS: AtomicIsize = AtomicIsize::new(0);
    // Where each shown icon is on the screen (key, left, top, right, bottom), from the host.
    static RECTS: Mutex<Vec<(u64, (i32, i32, i32, i32))>> = Mutex::new(Vec::new());
    static RAW_NOTED: AtomicU32 = AtomicU32::new(0);
    // When programs were last asked for their icons.
    static ASKED: Mutex<Option<std::time::Instant>> = Mutex::new(None);
    static NOTIFY: AtomicIsize = AtomicIsize::new(0);
    static NOTIFY_MSG: AtomicU32 = AtomicU32::new(0);
    static NEXT_KEY: AtomicU64 = AtomicU64::new(1);
    static ICONS: Mutex<Vec<Icon>> = Mutex::new(Vec::new());
    // Icons Explorer's tray holds that never came to ours (scan_held).
    static HELD: Mutex<Vec<HeldIcon>> = Mutex::new(Vec::new());
    // What came, for the log: (kind, count).
    static SEEN: Mutex<Vec<(String, u32)>> = Mutex::new(Vec::new());
    static NOTES: Mutex<Vec<String>> = Mutex::new(Vec::new());
    // The Explorer whose battery icon was last noted as left to it; how
    // many of its battery's messages have been told the log (the first
    // few of each Explorer's), and how many it refused in all (each told,
    // up to a point).
    static BATTERY_NOTED: AtomicU32 = AtomicU32::new(0);
    static BATTERY_SAID: AtomicU32 = AtomicU32::new(0);
    static BATTERY_FAILED: AtomicU32 = AtomicU32::new(0);
    // Ours behind Explorer's for now (set_aside): a new Explorer starting
    // has programs, and its own battery, reach its tray (zhoupengjie/Wakuwaku#85).
    static ASIDE: AtomicBool = AtomicBool::new(false);

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn text(s: &[u16]) -> String {
        let n = s.iter().position(|&c| c == 0).unwrap_or(s.len());
        String::from_utf16_lossy(&s[..n])
    }

    fn hwnd_of(h: u32) -> Hwnd {
        // Handles are 32 bits wide, sign-extended.
        h as i32 as isize as Hwnd
    }

    fn count(kind: &str) {
        let mut seen = SEEN.lock().unwrap();
        match seen.iter_mut().find(|(k, _)| k == kind) {
            Some((_, n)) => *n += 1,
            None => seen.push((kind.to_string(), 1)),
        }
    }

    fn note(line: String) {
        NOTES.lock().unwrap().push(line);
    }

    // What happened since the last call, for the host's log.
    pub fn notes() -> Vec<String> {
        std::mem::take(&mut *NOTES.lock().unwrap())
    }

    pub fn seen() -> String {
        SEEN.lock().unwrap().iter().map(|(k, n)| format!("{k} ×{n}")).collect::<Vec<_>>().join(", ")
    }

    // A process's file name; for one ours may not open (a SYSTEM one), as
    // the list of processes has it.
    fn exe_of(pid: u32) -> String {
        if pid == 0 {
            return "pid 0".to_string();
        }
        // SAFETY: a handle we close; the name goes into our own buffer.
        let named = unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            let mut name = [0u16; 512];
            let mut size = 512u32;
            let ok = process != 0 && QueryFullProcessImageNameW(process, 0, name.as_mut_ptr(), &mut size) != 0;
            if process != 0 {
                CloseHandle(process);
            }
            ok.then(|| file_name(&String::from_utf16_lossy(&name[..size as usize])))
        };
        named.or_else(|| processes().into_iter().find(|(p, _)| *p == pid).map(|(_, exe)| exe)).unwrap_or_else(|| format!("pid {pid}"))
    }

    // Every process running, with its file name.
    fn processes() -> Vec<(u32, String)> {
        let mut list = Vec::new();
        // SAFETY: a snapshot we close; each entry into our own struct, its size set.
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot == INVALID_HANDLE {
                return list;
            }
            let mut entry: ProcessEntry = std::mem::zeroed();
            entry.size = std::mem::size_of::<ProcessEntry>() as u32;
            let mut more = Process32FirstW(snapshot, &mut entry) != 0;
            while more {
                list.push((entry.pid, text(&entry.exe)));
                more = Process32NextW(snapshot, &mut entry) != 0;
            }
            CloseHandle(snapshot);
        }
        list
    }

    // Explorer's taskbar window: never another copy's of ours (shell.rs).
    pub fn explorers() -> Hwnd {
        crate::shell::explorer_tray() as Hwnd
    }

    // Whether programs reach ours first.
    pub fn first() -> bool {
        let ours = OURS.load(Ordering::SeqCst);
        // SAFETY: a plain lookup.
        ours != 0 && unsafe { FindWindowW(wide("Shell_TrayWnd").as_ptr(), std::ptr::null()) } as isize == ours
    }

    // Ours put ahead of Explorer's again; true if it had fallen behind.
    // Not while it is set aside.
    pub fn keep_first() -> bool {
        let ours = OURS.load(Ordering::SeqCst);
        if ours == 0 || ASIDE.load(Ordering::SeqCst) || first() {
            return false;
        }
        // SAFETY: our own window.
        unsafe { SetWindowPos(ours as Hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
        true
    }

    fn changed() {
        let to = NOTIFY.load(Ordering::SeqCst);
        if to != 0 {
            // SAFETY: the host's window; a posted message, no pointers.
            unsafe { PostMessageW(to as Hwnd, NOTIFY_MSG.load(Ordering::SeqCst), 0, 0) };
        }
    }

    // Handed on to Explorer's window as it came; its answer, or None.
    fn hand_on(msg: u32, wparam: usize, lparam: isize) -> Option<usize> {
        let explorer = explorers();
        if explorer.is_null() {
            return None;
        }
        let mut result = 0usize;
        // SAFETY: Explorer's window; a WM_COPYDATA's struct is ours for the call and is copied across.
        let ok = unsafe { SendMessageTimeoutW(explorer, msg, wparam, lparam, SMTO_ABORTIFHUNG, 3000, &mut result) };
        (ok != 0).then_some(result)
    }

    // Explorer's process, when an icon is its battery (EXPLORERS_BATTERY):
    // the window's process looked at for that uID alone.
    fn explorers_battery_of(nid: &IconData) -> Option<u32> {
        if nid.id != EXPLORERS_BATTERY || nid.hwnd == 0 {
            return None;
        }
        let mut pid = 0;
        // SAFETY: our own out-parameter; a gone window leaves it 0.
        unsafe { GetWindowThreadProcessId(hwnd_of(nid.hwnd), &mut pid) };
        (pid != 0 && explorers_battery(nid.id, &exe_of(pid))).then_some(pid)
    }

    fn same(icon: &Icon, nid: &IconData) -> bool {
        match (icon.guid, nid.flags & NIF_GUID != 0 && nid.guid != [0; 16]) {
            (Some(g), true) => g == nid.guid,
            _ => icon.hwnd == nid.hwnd && icon.id == nid.id,
        }
    }

    fn update(icon: &mut Icon, nid: &IconData) {
        if nid.flags & NIF_MESSAGE != 0 {
            icon.callback = nid.callback;
        }
        if nid.flags & NIF_ICON != 0 {
            // SAFETY: the program's icon, copied: it may destroy its own after this.
            // None at all is an icon too: a blink's dark half (WeChat's).
            let copy = if nid.icon == 0 { 0 } else { unsafe { CopyIcon(nid.icon as i32 as isize) } };
            if copy != 0 || nid.icon == 0 {
                if icon.icon != 0 {
                    // SAFETY: our own copy, replaced.
                    unsafe { DestroyIcon(icon.icon) };
                }
                icon.icon = copy;
                icon.had_icon |= copy != 0;
            }
        }
        if nid.flags & NIF_TIP != 0 {
            icon.tip = text(&nid.tip);
        }
        if nid.flags & NIF_STATE != 0 && nid.state_mask & NIS_HIDDEN != 0 {
            icon.hidden = nid.state & NIS_HIDDEN != 0;
        }
    }

    // A program's tip noted in the log at most every ten seconds: Task
    // Manager's changes every second.
    fn tip_note_due(exe: &str) -> bool {
        static NOTED: Mutex<Option<std::collections::HashMap<String, std::time::Instant>>> = Mutex::new(None);
        let mut noted = NOTED.lock().unwrap();
        let noted = noted.get_or_insert_with(std::collections::HashMap::new);
        let now = std::time::Instant::now();
        match noted.get(exe) {
            Some(at) if now.duration_since(*at) < std::time::Duration::from_secs(10) => false,
            _ => {
                noted.insert(exe.to_string(), now);
                true
            }
        }
    }

    // An icon new to ours, from an add (path: its program's, as the add
    // carries it) or a change.
    fn new_icon(nid: &IconData, path: Option<&str>) -> Icon {
        let mut pid = 0;
        // SAFETY: our own out-parameter; a gone window leaves it 0.
        unsafe { GetWindowThreadProcessId(hwnd_of(nid.hwnd), &mut pid) };
        let guid = (nid.flags & NIF_GUID != 0 && nid.guid != [0; 16]).then_some(nid.guid);
        let exe = path.map(file_name).filter(|e| !e.is_empty()).unwrap_or_else(|| exe_of(pid));
        let mut icon = Icon { key: NEXT_KEY.fetch_add(1, Ordering::SeqCst), hwnd: nid.hwnd, id: nid.id, guid, callback: 0, icon: 0, tip: String::new(), version: 0, hidden: false, pid, exe, modified: 0, had_icon: false, taken: false };
        update(&mut icon, nid);
        // One shown from Windows' record until now: this one in its place,
        // once it has a picture.
        if icon.had_icon {
            HELD.lock().unwrap().retain(|h| !h.is(&icon.exe, icon.id, icon.guid));
        }
        icon
    }

    // A NIM_ message: whether ours took it. held: Explorer's tray answered
    // a change of it yes (it holds the icon).
    fn on_icon(message: u32, nid: &IconData, path: Option<&str>, held: bool) -> bool {
        let mut icons = ICONS.lock().unwrap();
        let at = icons.iter().position(|i| same(i, nid));
        match (message, at) {
            (NIM_ADD | NIM_MODIFY, Some(i)) => {
                let tip = icons[i].tip.clone();
                update(&mut icons[i], nid);
                icons[i].modified += 1;
                if icons[i].tip != tip && tip_note_due(&icons[i].exe) {
                    note(format!("tray: {} tip \"{}\"", icons[i].exe, icons[i].tip));
                }
                true
            }
            (NIM_ADD, None) => {
                let icon = new_icon(nid, path);
                note(format!("tray: added {} (id {}, {}) \"{}\"{}", icon.exe, icon.id, if icon.guid.is_some() { "by guid" } else { "by window" }, icon.tip, if icon.hidden { ", hidden" } else { "" }));
                icons.push(icon);
                true
            }
            // A change of an icon ours never had, which Explorer's holds: its
            // add went there (before ours was up, or while Explorer's came
            // first), and its program does not add it again when asked (or
            // answers with this change, as Windows Security's does). Taken
            // from the change; a press reaches it only if the change names
            // its callback message.
            (NIM_MODIFY, None) if held && nid.hwnd != 0 => {
                let mut icon = new_icon(nid, None);
                icon.taken = true;
                note(format!("tray: took {}'s icon (id {}, {}) \"{}\" from a change: its add went to Windows' taskbar{}", icon.exe, icon.id, if icon.guid.is_some() { "by guid" } else { "by window" }, icon.tip, if icon.callback == 0 { "; no callback named, presses cannot reach it" } else { "" }));
                icons.push(icon);
                true
            }
            (NIM_DELETE, Some(i)) => {
                let icon = icons.remove(i);
                note(format!("tray: removed {} \"{}\"", icon.exe, icon.tip));
                if icon.icon != 0 {
                    // SAFETY: our own copy.
                    unsafe { DestroyIcon(icon.icon) };
                }
                true
            }
            // One shown from Windows' record, removed by its program: gone
            // now, not at the next look.
            (NIM_DELETE, None) => {
                let guid = (nid.flags & NIF_GUID != 0 && nid.guid != [0; 16]).then_some(nid.guid);
                let mut held = HELD.lock().unwrap();
                let before = held.len();
                held.retain(|h| match guid {
                    Some(g) => h.guid != Some(g),
                    None => h.guid.is_some() || h.hwnd != nid.hwnd || h.uid != nid.id,
                });
                held.len() != before
            }
            (NIM_SETVERSION, Some(i)) => {
                icons[i].version = nid.version;
                note(format!("tray: {} speaks version {}", icons[i].exe, nid.version));
                true
            }
            _ => false,
        }
    }

    // Where an icon is, as Shell_NotifyIconGetRect asks it: half the rect,
    // packed as two 16-bit numbers. None for an icon not shown here.
    fn answer_rect(cds: &CopyData) -> Option<isize> {
        let n = (cds.size as usize).min(std::mem::size_of::<RectQuestion>());
        if cds.ptr.is_null() || n < 24 {
            return None;
        }
        if RAW_NOTED.fetch_add(1, Ordering::SeqCst) < 2 {
            // SAFETY: n bytes of the sender's struct.
            let bytes = unsafe { std::slice::from_raw_parts(cds.ptr as *const u8, n) };
            note(format!("tray: where-is question, {} bytes: {}", cds.size, bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")));
        }
        let mut q: RectQuestion = unsafe { std::mem::zeroed() };
        // SAFETY: n bytes into ours of at least that size.
        unsafe { std::ptr::copy_nonoverlapping(cds.ptr as *const u8, &mut q as *mut RectQuestion as *mut u8, n) };
        if q.signature != SIGNATURE {
            return None;
        }
        let icons = ICONS.lock().unwrap();
        let by_guid = q.guid != [0; 16];
        let icon = icons.iter().find(|i| if by_guid { i.guid == Some(q.guid) } else { i.hwnd == q.hwnd && i.id == q.id });
        let Some(icon) = icon else {
            if RAW_NOTED.fetch_add(1, Ordering::SeqCst) < 6 {
                note(format!("tray: where-is for an icon not here: window {:#x} id {}", q.hwnd, q.id));
            }
            return None;
        };
        let rects = RECTS.lock().unwrap();
        let (_, (l, t, r, b)) = rects.iter().find(|(k, _)| *k == icon.key)?;
        let (x, y) = if q.message == 2 { (*r, *b) } else { (*l, *t) };
        Some(((x as u16 as u32) | ((y as u16 as u32) << 16)) as i32 as isize)
    }

    pub fn set_rects(rects: Vec<(u64, (i32, i32, i32, i32))>) {
        *RECTS.lock().unwrap() = rects;
    }

    extern "system" fn wndproc(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize {
        if msg == WM_COPYDATA && lparam != 0 {
            // SAFETY: Windows' copy of the sender's struct, for the length of this call.
            let cds = unsafe { &*(lparam as *const CopyData) };
            // Where is my icon: ours are not where Explorer's would be.
            if cds.data == 3 {
                if let Some(answer) = answer_rect(cds) {
                    count("where-is, answered");
                    return answer;
                }
                count("where-is, handed on");
            }
            let header = std::mem::size_of::<TrayData>() - std::mem::size_of::<IconData>();
            let tray = (cds.data == 1 && cds.size as usize >= header + 24 && !cds.ptr.is_null()).then(|| {
                // Read what came, the rest left zero (older programs send less).
                let mut data: TrayData = unsafe { std::mem::zeroed() };
                let n = (cds.size as usize).min(std::mem::size_of::<TrayData>());
                // SAFETY: n bytes of the sender's struct, into ours of at least that size.
                unsafe { std::ptr::copy_nonoverlapping(cds.ptr as *const u8, &mut data as *mut TrayData as *mut u8, n) };
                data
            });
            let tray = tray.filter(|d| d.signature == SIGNATURE);
            // Explorer's own battery icon: each message of it, an add in
            // answer to ask_again too, handed on to Explorer's tray alone and
            // its answer given back; ours never holds it (EXPLORERS_BATTERY).
            // The first few of each Explorer's, and each it refuses, told
            // with its tray's answer (battery_said).
            if let Some((pid, data)) = tray.and_then(|d| explorers_battery_of(&d.nid).map(|pid| (pid, d))) {
                count("Explorer's battery, handed on");
                if BATTERY_NOTED.swap(pid, Ordering::SeqCst) != pid {
                    note(format!("tray: Explorer's own battery icon (id {EXPLORERS_BATTERY}) left to Windows' taskbar, not taken (zhoupengjie/Wakuwaku#62)"));
                    BATTERY_SAID.store(0, Ordering::SeqCst);
                }
                let there = !explorers().is_null();
                let began = std::time::Instant::now();
                let answer = hand_on(msg, wparam, lparam);
                // SAFETY: a plain query, of the send just made.
                let error = if answer.is_none() { unsafe { GetLastError() } } else { 0 };
                let ms = began.elapsed().as_millis();
                let refused = !matches!(answer, Some(a) if a != 0);
                if BATTERY_SAID.fetch_add(1, Ordering::SeqCst) < 4 || (refused && BATTERY_FAILED.fetch_add(1, Ordering::SeqCst) < 20) {
                    let up = crate::shell::running_for(pid).map(|d| d.as_secs_f64());
                    note(battery_said(nim_name(data.message), pid, up, there, answer, error, ms));
                }
                return answer.unwrap_or(0) as isize;
            }
            // An icon added again: Explorer has it already (an icon ours has,
            // or one coming in answer to ask_again), and an add it refuses can
            // make it show the taskbar, and come first. An icon new to both
            // in that moment, or one Explorer's did not take, comes to it
            // when every program is told again (handed_back).
            let adding = tray.filter(|d| d.message == NIM_ADD);
            let known = adding.is_some_and(|d| ICONS.lock().unwrap().iter().any(|i| same(i, &d.nid)));
            let answering = adding.is_some() && ASKED.lock().unwrap().is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(3));
            let answer = if known || answering { Some(1) } else { hand_on(msg, wparam, lparam) };
            match tray {
                Some(data) => {
                    count(match data.message {
                        NIM_ADD if known => "NIM_ADD again",
                        NIM_ADD => "NIM_ADD",
                        NIM_MODIFY => "NIM_MODIFY",
                        NIM_DELETE => "NIM_DELETE",
                        NIM_SETVERSION => "NIM_SETVERSION",
                        _ => "NIM_other",
                    });
                    // SAFETY: Windows' copy of what was sent, cds.size bytes, for this call.
                    let path = (data.message == NIM_ADD).then(|| path_in(unsafe { std::slice::from_raw_parts(cds.ptr as *const u8, cds.size as usize) })).flatten();
                    let held = data.message == NIM_MODIFY && answer.is_some_and(|a| a != 0);
                    if on_icon(data.message, &data.nid, path.as_deref(), held) {
                        changed();
                        return 1;
                    }
                }
                None if cds.data != 3 => count(&format!("copydata {}", cds.data)),
                None => {}
            }
            return answer.unwrap_or(0) as isize;
        }
        if msg == WM_COMMAND || msg >= WM_USER {
            count(&format!("message {msg:#x}"));
            return hand_on(msg, wparam, lparam).unwrap_or(0) as isize;
        }
        // Kept, not handed on: counted for the log when another thread sent
        // it, or when it is a command to the taskbar's window (the system
        // menu's, a hot key's, a close), posted or sent: whether Windows'
        // way to Start ever passes here (zhoupengjie/Wakuwaku#54). Not
        // ours being placed (place, keep_first, from her other threads).
        // SAFETY: a plain query about the message being handled.
        let sent = unsafe { InSendMessageEx(std::ptr::null_mut()) } & (ISMEX_SEND | ISMEX_NOTIFY | ISMEX_CALLBACK) != 0;
        let placed = matches!(msg, WM_MOVE | WM_SIZE | WM_GETMINMAXINFO | WM_WINDOWPOSCHANGING | WM_WINDOWPOSCHANGED | WM_NCCALCSIZE);
        if (sent && !placed) || matches!(msg, WM_SYSCOMMAND | WM_HOTKEY | WM_CLOSE) {
            count(&if msg == WM_SYSCOMMAND { format!("kept {msg:#x} {:#x}", wparam & 0xFFF0) } else { format!("kept {msg:#x}") });
        }
        // SAFETY: the default for the rest.
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    // Ours made on this thread (which must run a message loop), at this
    // rect (some ask the taskbar's window where it is), and put ahead of
    // Explorer's. What changes is posted to notify as notify_msg. The host
    // asks programs for their icons (ask_again) once ours is first and stays
    // so: Explorer's comes first again while it shows its taskbar, as it does
    // on being put away, and programs asked then hand their icons to it
    // (2026-10-10: 3 icons of 16).
    pub fn start(rect: (i32, i32, i32, i32), notify: isize, notify_msg: u32) -> bool {
        if OURS.load(Ordering::SeqCst) != 0 {
            return true;
        }
        NOTIFY.store(notify, Ordering::SeqCst);
        NOTIFY_MSG.store(notify_msg, Ordering::SeqCst);
        // SAFETY: our own class and window, on this thread.
        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let class = wide("Shell_TrayWnd");
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
            let hwnd = CreateWindowExW(WS_EX_TOPMOST | WS_EX_TOOLWINDOW, class.as_ptr(), std::ptr::null(), WS_POPUP, rect.0, rect.1, rect.2, rect.3, std::ptr::null_mut(), 0, instance, std::ptr::null_mut());
            if hwnd.is_null() {
                return false;
            }
            OURS.store(hwnd as isize, Ordering::SeqCst);
            SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
        true
    }

    extern "system" fn ask_one(hwnd: Hwnd, msg: isize) -> i32 {
        let mut pid = 0;
        // SAFETY: a window being enumerated; our own out-parameter and buffer.
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        let mut name = [0u16; 32];
        let n = unsafe { GetClassNameW(hwnd, name.as_mut_ptr(), 32) }.max(0) as usize;
        let class = String::from_utf16_lossy(&name[..n]);
        if asked(pid == std::process::id(), &class) {
            // SAFETY: a registered message with no pointers.
            unsafe { SendNotifyMessageW(hwnd, msg as u32, 0, 0) };
        }
        1
    }

    // Every window told TaskbarCreated, as Explorer tells them when its
    // taskbar comes up; not Explorer's taskbars, which, told too, show
    // themselves, and come first while programs answer (2026-10-10: 3
    // times in 4). Windows of Store apps are not among those EnumWindows
    // walks (Start's, Search's on 26300: none of theirs is told).
    fn tell_all() {
        // SAFETY: a name of our own; the callback sends only.
        unsafe {
            let msg = RegisterWindowMessageW(wide("TaskbarCreated").as_ptr());
            EnumWindows(ask_one, msg as isize);
        }
    }

    // Every program asked to add its icons again (they come to ours, ahead).
    pub fn ask_again() {
        *ASKED.lock().unwrap() = Some(std::time::Instant::now());
        tell_all();
    }

    pub fn place(rect: (i32, i32, i32, i32)) {
        let ours = OURS.load(Ordering::SeqCst);
        if ours != 0 {
            // Set aside, it stays where it is among the windows.
            let aside = if ASIDE.load(Ordering::SeqCst) { SWP_NOZORDER } else { 0 };
            // SAFETY: our own window.
            unsafe { SetWindowPos(ours as Hwnd, HWND_TOPMOST, rect.0, rect.1, rect.2, rect.3, SWP_NOACTIVATE | aside) };
        }
    }

    // Ours put behind Explorer's, and kept there, while a new Explorer
    // starts (taskbar.rs): out of the topmost band and under every window,
    // so FindWindow finds Explorer's (topmost) first. Let go, it is put
    // first again by the next keep_first.
    pub fn set_aside(aside: bool) {
        ASIDE.store(aside, Ordering::SeqCst);
        let ours = OURS.load(Ordering::SeqCst);
        if aside && ours != 0 {
            // SAFETY: our own window.
            unsafe {
                SetWindowPos(ours as Hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                SetWindowPos(ours as Hwnd, HWND_BOTTOM, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            }
        }
    }

    pub fn aside() -> bool {
        ASIDE.load(Ordering::SeqCst)
    }

    // Ours gone (on the thread that made it). Explorer's has had nearly
    // everything; the rest comes once it shows again (handed_back).
    pub fn stop() {
        let ours = OURS.swap(0, Ordering::SeqCst);
        if ours != 0 {
            // SAFETY: our own window.
            unsafe { DestroyWindow(ours as Hwnd) };
        }
        ASIDE.store(false, Ordering::SeqCst);
        *ASKED.lock().unwrap() = None;
        for icon in ICONS.lock().unwrap().drain(..) {
            if icon.icon != 0 {
                // SAFETY: our own copies.
                unsafe { DestroyIcon(icon.icon) };
            }
        }
        HELD.lock().unwrap().clear();
    }

    // The taskbar Explorer's again, shown, ours gone (the host giving it
    // back; the guard, or her next start, after she was killed holding
    // it): every window told, as Explorer tells them when its taskbar comes
    // up. Whatever took ours for the taskbar on hearing it from us finds
    // Explorer's; a killed one never told them, nor handed Explorer's what
    // came while programs answered (zhoupengjie/Wakuwaku#54). Programs add
    // their icons again, which Explorer's has mostly: it says no to those.
    // False, nobody told, while ours is still there (its thread did not
    // stop): they would find it again.
    pub fn handed_back() -> bool {
        if OURS.load(Ordering::SeqCst) != 0 {
            return false;
        }
        tell_all();
        true
    }

    // Icons whose program is gone without saying so.
    pub fn sweep() {
        let mut icons = ICONS.lock().unwrap();
        let before = icons.len();
        icons.retain(|i| {
            // SAFETY: a plain check.
            let alive = unsafe { IsWindow(hwnd_of(i.hwnd)) } != 0;
            if !alive {
                note(format!("tray: {} gone without a word", i.exe));
                if i.icon != 0 {
                    unsafe { DestroyIcon(i.icon) };
                }
            }
            alive
        });
        if icons.len() != before {
            drop(icons);
            changed();
        }
    }

    // An icon Explorer's tray holds, shown in it, that ours was never
    // handed: its program (as Windows recorded it), its uID or guid, its
    // window when found by its uID, and from Windows' record of it its first
    // tip, its picture and whether it is kept out on the taskbar.
    struct HeldIcon {
        key: u64,
        exe: String,
        uid: u32,
        guid: Option<[u8; 16]>,
        hwnd: u32,
        tip: String,
        png: Vec<u8>,
        out: bool,
    }

    impl HeldIcon {
        // Whether it is this icon of ours: by its guid, else its program and
        // uID (whatever ours is named by: a record from before its program
        // named it by a guid).
        fn is(&self, exe: &str, id: u32, guid: Option<[u8; 16]>) -> bool {
            match self.guid {
                Some(a) => guid == Some(a),
                None => self.uid == id && self.exe.eq_ignore_ascii_case(exe),
            }
        }
    }

    // Asked as Shell_NotifyIconGetRect asks, of Explorer's own window (not
    // the first Shell_TrayWnd, which is ours): whether it holds this icon,
    // shown. None: it did not answer.
    fn explorer_holds(explorer: Hwnd, hwnd: u32, id: u32, guid: [u8; 16]) -> Option<bool> {
        for message in [1, 2] {
            let q = RectQuestion { signature: SIGNATURE, message, size: 32, padding: 0, hwnd, id, guid };
            let cds = CopyData { data: 3, size: std::mem::size_of::<RectQuestion>() as u32, ptr: &q as *const RectQuestion as *const c_void };
            let mut answer = 0usize;
            // SAFETY: Explorer's window; the struct is ours for the call and copied across.
            let ok = unsafe { SendMessageTimeoutW(explorer, WM_COPYDATA, 0, &cds as *const CopyData as isize, SMTO_ABORTIFHUNG, 500, &mut answer) };
            if ok == 0 {
                return None;
            }
            match holds(answer) {
                Holds::Shown => return Some(true),
                Holds::Hidden => return Some(false),
                // A corner at 0 is one half of the answer: the other asked.
                Holds::No => {}
            }
        }
        Some(false)
    }

    extern "system" fn list_one(hwnd: Hwnd, list: isize) -> i32 {
        let mut pid = 0;
        // SAFETY: a window being enumerated; our own out-parameter; the Vec outlives the walk.
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut pid);
            (*(list as *mut Vec<(u32, u32)>)).push((hwnd as isize as u32, pid));
        }
        1
    }

    // Every window a tray icon may be named by: the top-level ones and the
    // message-only ones (ACE-Tray's), each with its process.
    fn windows() -> Vec<(u32, u32)> {
        let mut list: Vec<(u32, u32)> = Vec::new();
        // SAFETY: the callback pushes onto the Vec, which outlives the call;
        // FindWindowEx walks the message-only windows one after another.
        unsafe {
            EnumWindows(list_one, &mut list as *mut Vec<(u32, u32)> as isize);
            let mut at: Hwnd = std::ptr::null_mut();
            loop {
                at = FindWindowExW(HWND_MESSAGE as Hwnd, at, std::ptr::null(), std::ptr::null());
                if at.is_null() {
                    break;
                }
                let mut pid = 0;
                GetWindowThreadProcessId(at, &mut pid);
                list.push((at as isize as u32, pid));
            }
        }
        list
    }

    // Windows' record of every tray icon it has shown (HKCU\Control Panel\
    // NotifyIconSettings, a key per icon: ExecutablePath, UID or IconGuid,
    // InitialTooltip, IsPromoted, IconSnapshot, a PNG), for the programs
    // that run (by file name, lower case): once per program and icon.
    fn records(running: &std::collections::HashSet<String>) -> Vec<HeldIcon> {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;
        let Ok(root) = RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Control Panel\NotifyIconSettings") else { return Vec::new() };
        let mut list: Vec<HeldIcon> = Vec::new();
        for name in root.enum_keys().flatten() {
            let Ok(key) = root.open_subkey(&name) else { continue };
            let Ok(path) = key.get_value::<String, _>("ExecutablePath") else { continue };
            let exe = file_name(&path).to_lowercase();
            // Explorer's own icons come back when asked, or are hers to draw (the volume, the network).
            if exe == "explorer.exe" || !running.contains(&exe) {
                continue;
            }
            let guid = key.get_value::<String, _>("IconGuid").ok().and_then(|g| guid_bytes(&g));
            let uid = key.get_value::<u32, _>("UID").ok();
            if guid.is_none() && uid.is_none() {
                continue;
            }
            let uid = uid.unwrap_or(0);
            // The same icon recorded again (another path to the program).
            if list.iter().any(|h| h.guid == guid && (guid.is_some() || (h.uid == uid && h.exe == exe))) {
                continue;
            }
            let png = key.get_raw_value("IconSnapshot").map(|v| v.bytes.into_owned()).unwrap_or_default();
            let tip = key.get_value::<String, _>("InitialTooltip").unwrap_or_default();
            let out = key.get_value::<u32, _>("IsPromoted").unwrap_or(0) != 0;
            list.push(HeldIcon { key: 0, exe, uid, guid, hwnd: 0, tip, png, out });
        }
        list
    }

    // The icons Explorer's tray holds, shown, that ours has not, of the
    // records of the programs that run (but ours): each one named by a guid
    // asked by it; one named by its window and uID asked of each window of
    // its program until one is held (at most 400 asked in all). None:
    // Explorer's did not answer (hung, or starting again).
    fn look_held(explorer: Hwnd, ours: &[(String, u32, Option<[u8; 16]>)]) -> Option<Vec<HeldIcon>> {
        let own = std::process::id();
        let procs: Vec<(u32, String)> = processes().into_iter().filter(|(pid, _)| *pid != own).map(|(pid, exe)| (pid, exe.to_lowercase())).collect();
        let running: std::collections::HashSet<String> = procs.iter().map(|(_, exe)| exe.clone()).collect();
        let mut wins: Option<Vec<(u32, u32)>> = None;
        let mut asked = 0;
        let mut found = Vec::new();
        for mut record in records(&running) {
            if record.png.is_empty() || ours.iter().any(|(exe, id, guid)| record.is(exe, *id, *guid)) {
                continue;
            }
            if let Some(guid) = record.guid {
                asked += 1;
                if explorer_holds(explorer, 0, record.uid, guid)? {
                    found.push(record);
                }
                continue;
            }
            let pids: Vec<u32> = procs.iter().filter(|(_, exe)| *exe == record.exe).map(|(pid, _)| *pid).collect();
            for (hwnd, _) in wins.get_or_insert_with(windows).iter().filter(|(_, pid)| pids.contains(pid)) {
                if asked >= 400 {
                    break;
                }
                asked += 1;
                if explorer_holds(explorer, *hwnd, record.uid, [0; 16])? {
                    record.hwnd = *hwnd;
                    found.push(record);
                    break;
                }
            }
        }
        Some(found)
    }

    // Looked for again (some seconds after programs were asked for their
    // icons, and every so often): the icons only Explorer's tray holds,
    // shown in ours from Windows' record of them until their program hands
    // ours one. Each keeps its key from one look to the next.
    pub fn scan_held() {
        let explorer = explorers();
        if OURS.load(Ordering::SeqCst) == 0 || explorer.is_null() {
            return;
        }
        // Those ours shows (one taken from a change without a picture yet is
        // still shown from Windows').
        let ours: Vec<(String, u32, Option<[u8; 16]>)> = ICONS.lock().unwrap().iter().filter(|i| !i.hidden && i.had_icon).map(|i| (i.exe.clone(), i.id, i.guid)).collect();
        // Explorer's not answering: what was found last kept until it does.
        let Some(mut found) = look_held(explorer, &ours) else { return };
        let mut held = HELD.lock().unwrap();
        for h in found.iter_mut() {
            match held.iter().find(|was| was.is(&h.exe, h.uid, h.guid) && was.hwnd == h.hwnd) {
                Some(was) => h.key = was.key,
                None => {
                    h.key = NEXT_KEY.fetch_add(1, Ordering::SeqCst);
                    let which = h.guid.map_or_else(|| format!("id {}, window {:#x}", h.uid, h.hwnd), |_| "by guid".to_string());
                    note(format!("tray: Windows' taskbar holds {}'s icon ({which}) \"{}\", never handed to hers: shown from Windows' picture of it, presses cannot reach it", h.exe, h.tip));
                }
            }
        }
        for was in held.iter().filter(|was| !found.iter().any(|h| h.key == was.key)) {
            note(format!("tray: {}'s icon no longer held by Windows' taskbar alone", was.exe));
        }
        let differs = found.len() != held.len() || found.iter().zip(held.iter()).any(|(a, b)| a.key != b.key);
        *held = found;
        drop(held);
        if differs {
            changed();
        }
    }

    // The icons ours has, then those only Explorer's holds (none that ours
    // has now).
    pub fn shown() -> Vec<Shown> {
        let mut list: Vec<Shown> = ICONS
            .lock()
            .unwrap()
            .iter()
            .filter(|i| !i.hidden && i.had_icon)
            .map(|i| Shown { key: i.key, icon: i.icon, tip: i.tip.clone(), exe: i.exe.clone(), uid: i.id, name: icon_name(&i.exe, i.guid, i.id), guid: i.guid, reach: !i.taken || i.callback != 0, snapshot: Vec::new(), out: None })
            .collect();
        let held: Vec<Shown> = HELD
            .lock()
            .unwrap()
            .iter()
            .filter(|h| !list.iter().any(|s| h.is(&s.exe, s.uid, s.guid)))
            .map(|h| Shown { key: h.key, icon: 0, tip: h.tip.clone(), exe: h.exe.clone(), uid: h.uid, name: icon_name(&h.exe, h.guid, h.uid), guid: h.guid, reach: false, snapshot: h.png.clone(), out: Some(h.out) })
            .collect();
        list.extend(held);
        list
    }

    // The spike writes it down (tray.txt); the pet only logs the icons as they come.
    #[allow(dead_code)]
    pub fn describe() -> String {
        ICONS
            .lock()
            .unwrap()
            .iter()
            .map(|i| format!("{} id={} v{} cb={:#x} changed ×{}{}{} \"{}\"", i.exe, i.id, i.version, i.callback, i.modified, if i.hidden { " hidden" } else { "" }, if i.icon == 0 { " no-icon" } else { "" }, i.tip))
            .collect::<Vec<_>>()
            .join("\n")
    }

    // A press on an icon, told to its program as Explorer tells it. at: the
    // pointer on the screen.
    pub fn tell(key: u64, press: Press, at: (i32, i32)) -> bool {
        // What is needed of it, the list let go before anything is sent: her
        // own icon's menu, opened on the press, would hold it (every
        // program's tray messages waiting) until it closed.
        struct Told {
            hwnd: u32,
            id: u32,
            callback: u32,
            version: u32,
            pid: u32,
            exe: String,
        }
        let found = ICONS.lock().unwrap().iter().find(|i| i.key == key).map(|i| Told { hwnd: i.hwnd, id: i.id, callback: i.callback, version: i.version, pid: i.pid, exe: i.exe.clone() });
        let Some(i) = found else { return false };
        if i.callback == 0 {
            return false;
        }
        // Her own icon's window is on the thread a press comes on (a
        // command's), where a send is a call: posted, its menu opens after
        // the press is done with, not inside it.
        let hers = i.pid == std::process::id();
        let send = |event: u32| {
            let (w, l) = if i.version >= 4 {
                ((at.0 as u16 as usize) | ((at.1 as u16 as usize) << 16), ((event & 0xffff) | ((i.id & 0xffff) << 16)) as isize)
            } else {
                (i.id as usize, event as isize)
            };
            // SAFETY: the program's window and the message it asked for; no pointers.
            unsafe {
                if hers {
                    PostMessageW(hwnd_of(i.hwnd), i.callback, w, l);
                } else {
                    SendNotifyMessageW(hwnd_of(i.hwnd), i.callback, w, l);
                }
            }
        };
        // The program's window put in front, as the taskbar does: a menu
        // opened while its program is not in front does not close on a press
        // elsewhere, and some do not bring themselves there (WeChat's,
        // Voicemeeter's). It may come to the front after, too.
        // Not when the program has the front already: what it opened on the
        // press (QQ's menu, a window of its own) would lose it, and close.
        let allow = || {
            let mut pid = 0;
            // SAFETY: plain queries; our own out-parameter.
            unsafe { GetWindowThreadProcessId(GetForegroundWindow(), &mut pid) };
            if pid == i.pid {
                return;
            }
            // SAFETY: plain calls; they fail unless we may set the front ourselves.
            let allowed = unsafe { AllowSetForegroundWindow(i.pid) } != 0;
            let put = unsafe { SetForegroundWindow(hwnd_of(i.hwnd)) } != 0;
            if !allowed || !put {
                note(format!("tray: {} not put in front (let: {allowed}, put: {put})", i.exe));
            }
        };
        // Some open what they open on the press itself (Electron's), not on letting go.
        match press {
            Press::LeftDown => {
                allow();
                send(WM_LBUTTONDOWN);
            }
            Press::LeftUp => {
                allow();
                send(WM_LBUTTONUP);
                if i.version >= 3 {
                    send(NIN_SELECT);
                }
            }
            Press::RightDown => {
                allow();
                send(WM_RBUTTONDOWN);
            }
            Press::RightUp => {
                allow();
                send(WM_RBUTTONUP);
                if i.version >= 3 {
                    send(WM_CONTEXTMENU);
                }
            }
            Press::Double => {
                allow();
                send(WM_LBUTTONDBLCLK);
            }
            Press::Move => send(WM_MOUSEMOVE),
            Press::HoverIn => {
                if i.version >= 4 {
                    send(NIN_POPUPOPEN);
                }
            }
            Press::HoverOut => {
                if i.version >= 4 {
                    send(NIN_POPUPCLOSE);
                }
            }
        }
        true
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn the_structs_are_the_size_the_tray_is_handed() {
            // NOTIFYICONDATAW in its 32-bit form, and SHELLTRAYDATA around it,
            // the program's path after it.
            assert_eq!(std::mem::size_of::<IconData>(), 956);
            assert_eq!(std::mem::size_of::<TrayData>(), 964);
            assert_eq!(std::mem::size_of::<TrayData>(), super::super::PATH_AT);
            assert_eq!(std::mem::size_of::<RectQuestion>(), 40);
        }

        fn nid(hwnd: u32, id: u32, flags: u32, tip: &str) -> IconData {
            // SAFETY: plain numbers and arrays, all zero.
            let mut nid: IconData = unsafe { std::mem::zeroed() };
            nid.size = 956;
            nid.hwnd = hwnd;
            nid.id = id;
            nid.flags = flags;
            for (i, u) in tip.encode_utf16().enumerate() {
                nid.tip[i] = u;
            }
            nid
        }

        fn ours(hwnd: u32, id: u32) -> Option<(String, u32, bool, String)> {
            ICONS.lock().unwrap().iter().find(|i| i.hwnd == hwnd && i.id == id).map(|i| (i.exe.clone(), i.callback, i.had_icon, i.tip.clone()))
        }

        #[test]
        fn a_change_of_an_icon_only_explorers_tray_holds_is_taken() {
            // Windows Security's answer to TaskbarCreated: a change, its
            // icon, tip and state, no callback (seen in SecurityHealthSSO.dll).
            let change = nid(0x7a01, 100, NIF_TIP | NIF_STATE, "Windows Security");
            assert!(!on_icon(NIM_MODIFY, &change, None, false), "one Explorer's does not hold either is not taken");
            assert_eq!(ours(0x7a01, 100), None);
            assert!(on_icon(NIM_MODIFY, &change, None, true));
            let (_, callback, _, tip) = ours(0x7a01, 100).expect("taken");
            assert_eq!((callback, tip.as_str()), (0, "Windows Security"));
            // Named by no window: not taken (it would be swept at once).
            assert!(!on_icon(NIM_MODIFY, &nid(0, 101, NIF_TIP, "x"), None, true));
            // Its program's add, later, is the same icon: its callback filled in.
            let mut add = nid(0x7a01, 100, NIF_MESSAGE | NIF_TIP, "Windows Security");
            add.callback = 0x8077;
            assert!(on_icon(NIM_ADD, &add, None, false));
            assert_eq!(ours(0x7a01, 100).map(|o| o.1), Some(0x8077));
            assert!(on_icon(NIM_DELETE, &add, None, false));
        }

        #[test]
        fn an_add_names_its_program_by_the_path_it_carries() {
            // A SYSTEM process's, which ours may not open (nvcontainer.exe).
            let add = nid(0x7a02, 1, NIF_TIP, "NVIDIA");
            assert!(on_icon(NIM_ADD, &add, Some(r"{6D809377-6AF0-444B-8957-A3773F02200E}\NVIDIA Corporation\NvContainer\nvcontainer.exe"), false));
            assert_eq!(ours(0x7a02, 1).map(|o| o.0).as_deref(), Some("nvcontainer.exe"));
            assert!(on_icon(NIM_DELETE, &add, None, false));
        }

        #[test]
        fn an_icon_its_program_hands_ours_takes_the_place_of_the_one_shown_from_windows_record() {
            let guid = super::super::guid_bytes("{7120A42D-CB02-4A51-B3B5-58E36E8B14E1}").unwrap();
            HELD.lock().unwrap().push(HeldIcon { key: 9_000_001, exe: "nvcontainer.exe".into(), uid: 0, guid: Some(guid), hwnd: 0, tip: String::new(), png: vec![0x89], out: false });
            let other = HeldIcon { key: 9_000_002, exe: "ace-tray.exe".into(), uid: 10001, guid: None, hwnd: 0x7a03, tip: String::new(), png: vec![0x89], out: true };
            HELD.lock().unwrap().push(other);
            let held = shown().into_iter().filter(|s| s.key >= 9_000_001).map(|s| (s.name, s.reach, s.out)).collect::<Vec<_>>();
            assert!(held.contains(&("nvcontainer.exe:2da4207102cb514ab3b558e36e8b14e1".to_string(), false, Some(false))), "{held:?}");
            assert!(held.contains(&("ace-tray.exe:10001".to_string(), false, Some(true))), "{held:?}");
            // Its add without a picture leaves Windows' in its place; with one, not.
            let mut add = nid(0x7a04, 1, NIF_GUID | NIF_TIP, "NVIDIA");
            add.guid = guid;
            assert!(on_icon(NIM_ADD, &add, Some(r"C:\Program Files\NVIDIA Corporation\NvContainer\nvcontainer.exe"), false));
            assert!(HELD.lock().unwrap().iter().any(|h| h.key == 9_000_001));
            assert!(on_icon(NIM_DELETE, &add, None, false));
            #[link(name = "user32")]
            extern "system" {
                fn LoadIconW(instance: isize, name: usize) -> isize;
            }
            add.flags |= NIF_ICON;
            // SAFETY: Windows' own application icon.
            add.icon = unsafe { LoadIconW(0, 32512) } as u32;
            assert!(on_icon(NIM_ADD, &add, Some(r"C:\Program Files\NVIDIA Corporation\NvContainer\nvcontainer.exe"), false));
            let keys: Vec<u64> = HELD.lock().unwrap().iter().map(|h| h.key).collect();
            assert!(!keys.contains(&9_000_001) && keys.contains(&9_000_002), "{keys:?}");
            assert!(on_icon(NIM_DELETE, &add, None, false));
            // Its program removing one shown from Windows' record: gone at once.
            assert!(on_icon(NIM_DELETE, &nid(0x7a03, 10001, 0, ""), None, false));
            assert!(!HELD.lock().unwrap().iter().any(|h| h.key == 9_000_002));
        }

        #[test]
        fn explorers_battery_is_known_by_its_window_and_uid() {
            #[link(name = "user32")]
            extern "system" {
                fn GetDesktopWindow() -> Hwnd;
            }
            // No window named, or one gone: not Explorer's.
            assert_eq!(explorers_battery_of(&nid(0, 1225, NIF_GUID, "")), None);
            assert_eq!(explorers_battery_of(&nid(0x7a05, 1225, NIF_GUID, "")), None);
            // A window of another process's (the desktop's, csrss.exe's).
            // SAFETY: a plain query.
            let desktop = unsafe { GetDesktopWindow() } as isize as u32;
            assert_eq!(explorers_battery_of(&nid(desktop, 1225, NIF_GUID, "")), None);
            // A window of Explorer's, where it runs: its battery's uID, and no other.
            let explorer = explorers();
            if !explorer.is_null() {
                let window = explorer as isize as u32;
                let mut pid = 0;
                // SAFETY: our own out-parameter.
                unsafe { GetWindowThreadProcessId(explorer, &mut pid) };
                assert_eq!(explorers_battery_of(&nid(window, 1225, NIF_GUID | NIF_TIP, "电池指示器")), Some(pid));
                assert_eq!(explorers_battery_of(&nid(window, 1226, NIF_GUID, "安全删除硬件并弹出媒体")), None);
            }
        }

        // What only Explorer's tray holds on this computer, as she would
        // look (her own icons left out: there are none here). By hand:
        // cargo test only_explorers -- --ignored --nocapture
        #[test]
        #[ignore]
        fn only_explorers_tray_holds_these_here() {
            let explorer = explorers();
            assert!(!explorer.is_null(), "no Explorer taskbar");
            let started = std::time::Instant::now();
            for h in look_held(explorer, &[]).expect("Explorer's tray answers") {
                println!("{} uid {} guid {:?} window {:#x} out {} tip {:?} picture {} bytes", h.exe, h.uid, h.guid.map(|g| super::super::icon_name("", Some(g), 0)), h.hwnd, h.out, h.tip, h.png.len());
            }
            println!("looked in {:?}", started.elapsed());
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::{Press, Shown};

    pub fn start(_rect: (i32, i32, i32, i32), _notify: isize, _notify_msg: u32) -> bool {
        false
    }
    pub fn stop() {}
    pub fn handed_back() -> bool {
        false
    }
    pub fn place(_rect: (i32, i32, i32, i32)) {}
    pub fn first() -> bool {
        false
    }
    pub fn keep_first() -> bool {
        false
    }
    pub fn set_aside(_aside: bool) {}
    pub fn aside() -> bool {
        false
    }
    pub fn ask_again() {}
    pub fn sweep() {}
    pub fn scan_held() {}
    pub fn shown() -> Vec<Shown> {
        Vec::new()
    }
    pub fn describe() -> String {
        String::new()
    }
    pub fn notes() -> Vec<String> {
        Vec::new()
    }
    pub fn seen() -> String {
        String::new()
    }
    pub fn tell(_key: u64, _press: Press, _at: (i32, i32)) -> bool {
        false
    }
    pub fn set_rects(_rects: Vec<(u64, (i32, i32, i32, i32))>) {}
}

#[allow(unused_imports)]
pub use imp::{aside, ask_again, describe, first, handed_back, keep_first, notes, place, scan_held, seen, set_aside, set_rects, shown, start, stop, sweep, tell};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_add_carries_its_programs_path_after_the_icon() {
        // As shell32 sent it on 26300: SHELLTRAYDATA, then 520 bytes, the path.
        let path = r"{6D809377-6AF0-444B-8957-A3773F02200E}\PowerShell\7\pwsh.exe";
        let mut copied = vec![0u8; PATH_AT + 520];
        for (i, u) in path.encode_utf16().enumerate() {
            copied[PATH_AT + 2 * i..PATH_AT + 2 * i + 2].copy_from_slice(&u.to_le_bytes());
        }
        assert_eq!(path_in(&copied).as_deref(), Some(path));
        assert_eq!(file_name(&path_in(&copied).unwrap()), "pwsh.exe");
        // The other messages carry it empty; older senders not at all.
        assert_eq!(path_in(&vec![0u8; PATH_AT + 520]), None);
        assert_eq!(path_in(&vec![0u8; PATH_AT]), None);
        assert_eq!(path_in(&vec![0u8; 152 + 8]), None);
    }

    #[test]
    fn a_guid_as_windows_writes_it_is_its_bytes_in_memory() {
        let g = guid_bytes("{7120A42D-CB02-4A51-B3B5-58E36E8B14E1}").unwrap();
        assert_eq!(g, [0x2d, 0xa4, 0x20, 0x71, 0x02, 0xcb, 0x51, 0x4a, 0xb3, 0xb5, 0x58, 0xe3, 0x6e, 0x8b, 0x14, 0xe1]);
        assert_eq!(guid_bytes("7120a42d-cb02-4a51-b3b5-58e36e8b14e1"), Some(g));
        assert_eq!(guid_bytes("{7120A42D-CB02}"), None);
        assert_eq!(guid_bytes("{7120A42D-CB02-4A51-B3B5-58E36E8B14EZ}"), None);
        // Named as the icons handed to ours are, so where it is kept carries over.
        assert_eq!(icon_name("NVContainer.exe", Some(g), 0), "nvcontainer.exe:2da4207102cb514ab3b558e36e8b14e1");
        assert_eq!(icon_name("ACE-Tray.exe", None, 10001), "ace-tray.exe:10001");
    }

    #[test]
    fn explorers_answer_says_whether_it_holds_an_icon_shown() {
        // Seen 2026-10-15: none; one hidden (NIS_HIDDEN); one folded away
        // (where ^ is, 2159,1380).
        assert_eq!(holds(0), Holds::No);
        assert_eq!(holds(-1isize as usize), Holds::Hidden);
        assert_eq!(holds(0xFFFF_FFFF), Holds::Hidden);
        assert_eq!(holds(2159 | (1380 << 16)), Holds::Shown);
        assert_eq!(file_name("C:/x/y.exe"), "y.exe");
    }

    #[test]
    fn explorers_battery_alone_is_left_to_explorer() {
        // As #62's log has it: explorer.exe, id 1225, by guid.
        assert!(explorers_battery(1225, "explorer.exe"));
        assert!(explorers_battery(1225, "Explorer.EXE"));
        // Its other icons, which pass through ours on 26100 without harm
        // (safe removal, a headset's battery, a microphone in use, Bluetooth).
        for id in [1226, 100, 101, 0] {
            assert!(!explorers_battery(id, "explorer.exe"), "{id}");
        }
        // Another program's icon of that uID, or one whose process is gone.
        assert!(!explorers_battery(1225, "QQ.exe"));
        assert!(!explorers_battery(1225, "pid 0"));
    }

    #[test]
    fn programs_are_asked_for_their_icons_and_her_own_tray_icon_too() {
        assert!(asked(false, "WeChatMainWndForPC"));
        assert!(asked(false, HER_TRAY_ICON));
        // Hers: her tray icon's window alone; not her taskbar's, her pages'.
        assert!(asked(true, HER_TRAY_ICON));
        assert!(!asked(true, "WakuwakuTaskbarHost"));
        assert!(!asked(true, "Chrome_WidgetWin_1"));
        // Explorer's taskbars and hers.
        for own in [false, true] {
            assert!(!asked(own, "Shell_TrayWnd"));
            assert!(!asked(own, "Shell_SecondaryTrayWnd"));
        }
    }

    #[test]
    fn the_batterys_hand_on_says_what_explorers_tray_answered() {
        assert_eq!(nim_name(0), "NIM_ADD");
        assert_eq!(nim_name(4), "NIM_SETVERSION");
        assert_eq!(nim_name(9), "NIM_other");
        assert_eq!(
            battery_said("NIM_ADD", 4242, Some(6.27), true, Some(1), 0, 3),
            "tray: Explorer's battery NIM_ADD (pid 4242, up 6.3 s) handed on: answered 1 in 3 ms"
        );
        // Refused; timed out (1460, ERROR_TIMEOUT); no tray of Explorer's.
        assert!(battery_said("NIM_MODIFY", 1, None, true, Some(0), 0, 2).ends_with("(pid 1, gone) handed on: answered 0 in 2 ms"));
        assert!(battery_said("NIM_ADD", 1, Some(1.0), true, None, 1460, 3000).ends_with("not answered (error 1460) in 3000 ms"));
        assert!(battery_said("NIM_ADD", 1, Some(1.0), false, None, 0, 0).ends_with("no tray of Explorer's to hand it to in 0 ms"));
    }
}
