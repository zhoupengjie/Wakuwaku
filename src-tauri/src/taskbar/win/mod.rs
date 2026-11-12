// The taskbar's side of Windows, on Windows (taskbar/mod.rs says what it
// does and what not): what its parts share here (Windows' calls, messages
// and timers, the state kept between them, the log), and the parts:
//   thread.rs    its thread: the hidden window, the message loop, what it hears, the tick
//   hold.rs      Windows' own taskbar put away and given back; Explorer starting; the tug
//   work.rs      the work area kept to the strip (the self-heal), the desktop's icons
//   tray.rs      the tray taken over: icons into PNGs, presses handed on
//   buttons.rs   the windows' buttons, programs started, windows acted on
//   thumbs.rs    the windows' live pictures, the dock's badges
//   keys.rs      the keyboard, the volume, the network, the battery
//   look.rs      the desktop's picture and Windows' look, for the strip
//   flyouts.rs   Windows' flyouts, the volume, Start, the desktop shown, the Recycle Bin
//   presses.rs   presses anywhere while a list is open
//   apps.rs      an app's name and icon from the shell
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};
use std::sync::{Arc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use base64::Engine;
use serde_json::{json, Value};
use tauri::{Emitter, Manager};

use crate::appbar::Edge;
use crate::systray::{self, Press};
use crate::{jump, shell, tasks, wallpaper, Shared};
use crate::lock::Lock;

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
// A new page wants the tray's icons whole (tray.rs resend): its pictures are this thread's.
const WM_RESEND_TRAY: u32 = 0x8007;
// The strip moved: the desktop's picture under it looked at again (so
// too when a setting or Windows' colours change). The picture changed
// by her own settings: looked at until it is in place (settle).
const WM_PAPER: u32 = 0x8004;
const WM_PAPER_SET: u32 = 0x8005;
// Debug only (/debug/hang): this thread held still for wparam seconds.
const WM_HOLD_STILL: u32 = 0x8012;
// The timers on the window: the half second's tick, the looks after
// Windows' word (settle), and the work area looked at once Explorer
// has been still a moment (WORK_STILL_MS: keep_work).
const TICK: usize = 1;
const SETTLE: usize = 2;
const WORK_STILL: usize = 3;
const WORK_STILL_MS: u32 = 250;
const WM_SETTINGCHANGE: u32 = 0x001A;
const WM_DISPLAYCHANGE: u32 = 0x007E;
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
const LWA_COLORKEY: u32 = 0x1;
// The dock's pictures' window's ground, keyed out (COLORREF: magenta).
const DOCK_KEY: u32 = 0x00FF_00FF;
const HWND_TOPMOST: isize = -1;
const SWP_NOACTIVATE: u32 = 0x10;
const SWP_SHOWWINDOW: u32 = 0x40;
const SW_HIDE: i32 = 0;
const SPI_GETWORKAREA: u32 = 0x30;
const SPI_SETWORKAREA: u32 = 0x2F;

const EVENT_OBJECT_SHOW: u32 = 0x8002;
const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;
// What changes the buttons: a window made, shown, hidden, gone (0x8001
// to 0x8003), cloaked or not (another desktop), brought to the front,
// minimized or restored; moved or sized, for one over a display or no
// longer (fills_display; on_event lets only those through).
// Not renamed (0x800C): that event comes for every object of every
// program (a hundred a second, nearly all thrown away here, 2026-10-11);
// a window's title changing comes by the shell hook (HSHELL_REDRAW).
const WINDOW_EVENTS: [(u32, u32); 5] = [(0x8001, 0x8003), (0x8017, 0x8018), (0x0003, 0x0003), (0x0016, 0x0017), (0x800B, 0x800B)];
const EVENT_OBJECT_LOCATIONCHANGE: u32 = 0x800B;
const OBJID_WINDOW: i32 = 0;
// The shell hook's: a window flashing for attention, one activated; one
// made, gone, or its title changed.
const HSHELL_FLASH: usize = 0x8006;
const HSHELL_WINDOWACTIVATED: usize = 4;
const HSHELL_RUDEAPPACTIVATED: usize = 0x8004;
const HSHELL_WINDOWCREATED: usize = 1;
const HSHELL_WINDOWDESTROYED: usize = 2;
const HSHELL_REDRAW: usize = 6;
const KEYEVENTF_EXTENDEDKEY: u32 = 0x1;
const KEYEVENTF_KEYUP: u32 = 0x2;
const VK_LWIN: u16 = 0x5B;
const VK_ESCAPE: u16 = 0x1B;
const VK_TAB: u16 = 0x09;
const VK_SPACE: u16 = 0x20;
const DI_NORMAL: u32 = 3;
// The ticks (half seconds) to wait before asking programs for their icons.
const SETTLE_TICKS: u32 = 3;

// The state each part keeps is in its own file, beside the code that
// keeps it; named here for the others (each file has `use super::*`).
use buttons::{TASKS_STOP, SENT_WINDOWS};
use flyouts::START_OPEN;
use keys::{KEYS_NOW, SENT_KEYS};
use look::{PAPER, LOOK, LOOKS_LEFT};
use thread::{OURS, TICKS};
use thumbs::THUMB_HOST;
use tray::{ASK_AT, LAST_ASK, SENT};
use work::{EDGE, WORK_EDGE, STRIP, WORK_SIDE};
// Her Shared, for the threads here: only ever cloned out (sh()), so taken
// inside any lock and nothing inside it.
static SH: Lock<Option<Arc<Shared>>> = Lock::ranked("taskbar::win::SH", crate::lock::LEAF, None);

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

// A line for the log from elsewhere in the taskbar's doings (tasks.rs).
pub fn note(line: &str) {
    log(line);
}

fn work_area() -> Rect {
    let mut r = Rect::default();
    // SAFETY: our own struct, of the size the call writes.
    unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut r as *mut Rect as *mut c_void, 0) };
    r
}

// Windows' taskbar shown again by her guard while a thread of hers did not
// answer (shell.rs HangBack): left up till the guard takes its mark away
// as she answers again.
fn shown_for_hang() -> bool {
    sh().is_some_and(|sh| shell::shown_for_hang(&sh.dir.join("taskbar.json")))
}

fn work_bottom() -> i32 {
    work_area().bottom
}

// Each part's items: those of its own that the others use are pub(super);
// its pub ones are the taskbar's (taskbar/mod.rs).
mod apps;
mod buttons;
mod flyouts;
mod hold;
mod keys;
mod look;
mod presses;
mod thread;
mod thumbs;
mod tray;
mod work;

use apps::*;
pub use buttons::*;
pub use flyouts::*;
pub use hold::*;
pub use keys::*;
pub use look::*;
pub use presses::*;
pub use thread::*;
pub use thumbs::*;
pub use tray::*;
pub use work::*;
