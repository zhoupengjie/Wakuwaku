// Windows' side of the tray (systray/mod.rs imp): her own Shell_TrayWnd,
// its thread and window, the icons it keeps; the icons' messages, those
// only Explorer's tray holds, and the presses each in a file of its own.
use crate::lock::Lock;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, AtomicU64, Ordering};

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
// Read inside ICONS (answer_rect): ranked above it (lock.rs).
static RECTS: Lock<Vec<(u64, (i32, i32, i32, i32))>> = Lock::ranked("systray::RECTS", INSIDE_ICONS, Vec::new());
static RAW_NOTED: AtomicU32 = AtomicU32::new(0);
// When programs were last asked for their icons.
static ASKED: Lock<Option<std::time::Instant>> = Lock::new("systray::ASKED", None);
static NOTIFY: AtomicIsize = AtomicIsize::new(0);
static NOTIFY_MSG: AtomicU32 = AtomicU32::new(0);
static NEXT_KEY: AtomicU64 = AtomicU64::new(1);
static ICONS: Lock<Vec<Icon>> = Lock::new("systray::ICONS", Vec::new());
// Icons Explorer's tray holds that never came to ours (scan_held).
// Taken inside ICONS too (new_icon, on_icon), never the other way about.
static HELD: Lock<Vec<HeldIcon>> = Lock::ranked("systray::HELD", INSIDE_ICONS, Vec::new());
// What came, for the log: (kind, count).
// What to say, and the next two: the log's, taken inside the others
// (a note of what ICONS or HELD holds) and nothing inside them.
static SEEN: Lock<Vec<(String, u32)>> = Lock::ranked("systray::SEEN", crate::lock::LEAF, Vec::new());
static NOTES: Lock<Vec<String>> = Lock::ranked("systray::NOTES", crate::lock::LEAF, Vec::new());
const INSIDE_ICONS: u8 = crate::lock::PLAIN + 10;
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
// Not while it is set aside, nor put behind for an app bar's place
// (explorer_first).
pub fn keep_first() -> bool {
    let ours = OURS.load(Ordering::SeqCst);
    if ours == 0 || held_back() || first() {
        return false;
    }
    // SAFETY: our own window.
    unsafe { SetWindowPos(ours as Hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
    true
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

// The programs with an icon in her tray (their processes).
pub fn holders() -> Vec<u32> {
    ICONS.lock().unwrap().iter().map(|i| i.pid).filter(|&p| p != 0).collect()
}

pub fn place(rect: (i32, i32, i32, i32)) {
    let ours = OURS.load(Ordering::SeqCst);
    if ours != 0 {
        // Set aside, it stays where it is among the windows.
        let aside = if held_back() { SWP_NOZORDER } else { 0 };
        // SAFETY: our own window.
        unsafe { SetWindowPos(ours as Hwnd, HWND_TOPMOST, rect.0, rect.1, rect.2, rect.3, SWP_NOACTIVATE | aside) };
    }
}

// Ours put behind Explorer's, and kept there, while a new Explorer
// starts (taskbar/): out of the topmost band and under every window,
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

// Her own app bar's place asked of Explorer (appbar.rs place: ABM_QUERYPOS
// and ABM_SETPOS) with ours behind Explorer's for that moment, so the
// message goes to Explorer's window itself. To ours, a window of her own
// process, it carried the rect's address in her process where a handle
// to memory Explorer can open goes (shell32 makes none for a taskbar in
// the process asking: at offset 48 of what came, 0x2080F485000, seen in
// the test session, 2026-11-09), and handed on, Explorer could neither
// read nor answer it: the bar registered (ABM_NEW needs no memory), its
// place never told but when Explorer's window happened to be first as she
// asked (it is, for a moment, as it shows on being put away), so it was
// left out of the work area Explorer worked out each time its own taskbar
// showed again, a right-click's menu or Win+Shift+S sending the maximized
// windows up under the top bar (the user, 2026-11-09). The same message
// sent from another process, its handle made, Explorer counted the bar.
// Ours left where it is while set aside already (a new Explorer starting),
// or not made. Asked on several threads at once (the island's strip
// taken on one, its place told again on the taskbar's), they share the
// moment: the first puts ours behind, the last puts it first again, and
// meanwhile nothing else does (held_back). Nobody waits on anybody here:
// moving ours from another thread waits on the taskbar's thread, which
// asks too. What f gave, and whether ours was put behind for it (what
// programs said meanwhile went to Explorer's alone).
static CLAIMING: AtomicU32 = AtomicU32::new(0);

fn held_back() -> bool {
    ASIDE.load(Ordering::SeqCst) || CLAIMING.load(Ordering::SeqCst) > 0
}

pub fn explorer_first<R>(f: impl FnOnce() -> R) -> (R, bool) {
    let ours = OURS.load(Ordering::SeqCst);
    if ours == 0 || ASIDE.load(Ordering::SeqCst) {
        return (f(), false);
    }
    if CLAIMING.fetch_add(1, Ordering::SeqCst) == 0 {
        // SAFETY: our own window.
        unsafe {
            SetWindowPos(ours as Hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            SetWindowPos(ours as Hwnd, HWND_BOTTOM, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
        if first() {
            note("tray: put behind for her app bar's place, ours still first: Explorer's would not hear it".to_string());
        }
    }
    let r = f();
    if CLAIMING.fetch_sub(1, Ordering::SeqCst) == 1 {
        keep_first();
    }
    (r, true)
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

mod held;
mod icons;
mod press;
#[allow(unused_imports)]
pub use held::*;
#[allow(unused_imports)]
pub use icons::*;
#[allow(unused_imports)]
pub use press::*;

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
