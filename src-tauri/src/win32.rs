// Windows' own calls that more than one of her parts makes, each declared
// here once, with the structs they take: a part that needs one takes it
// from here (use crate::win32::{...}). Until 2026-11-13 every part
// declared what it called for itself (CloseHandle fourteen times over),
// kept the same by hand and by rustc's clashing_extern_declarations.
//
// A call only one part makes stays with that part, beside what it is for;
// the day a second part needs it, it moves here (the test below reads her
// source and fails on one declared in two files, or there and here).
//
// Handles are as her parts have always written them: a window's a pointer
// (Hwnd), every other an isize (Handle; GDI's, a module's, a hook's said
// plainly as isize). Nothing here is checked: what a call needs of its
// arguments, and what it leaves to be closed, is the caller's to keep to.
use std::ffi::c_void;

pub type Hwnd = *mut c_void;
pub type Handle = isize;

// RECT.
#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

// POINT; a SIZE too, where a call takes one (UpdateLayeredWindow).
#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

// MSG.
#[repr(C)]
pub struct Msg {
    pub hwnd: Hwnd,
    pub message: u32,
    pub wparam: usize,
    pub lparam: isize,
    pub time: u32,
    pub pt: Point,
    pub private: u32,
}

// WNDCLASSEXW.
#[repr(C)]
pub struct WndClassExW {
    pub size: u32,
    pub style: u32,
    pub wndproc: extern "system" fn(Hwnd, u32, usize, isize) -> isize,
    pub cls_extra: i32,
    pub wnd_extra: i32,
    pub instance: isize,
    pub icon: isize,
    pub cursor: isize,
    pub background: isize,
    pub menu_name: *const u16,
    pub class_name: *const u16,
    pub icon_sm: isize,
}

// MONITORINFO. The longer MONITORINFOEXW (the display's name after it) is
// gpu.rs's own, handed to GetMonitorInfoW as this one: the call tells
// them apart by the size written in them.
#[repr(C)]
#[derive(Default)]
pub struct MonitorInfo {
    pub size: u32,
    pub monitor: Rect,
    pub work: Rect,
    pub flags: u32,
}

// PROCESSENTRY32W: every process's file name, SYSTEM's too, which
// OpenProcess does not give a person's process (nvcontainer.exe's).
#[repr(C)]
pub struct ProcessEntry {
    pub size: u32,
    pub usage: u32,
    pub pid: u32,
    pub heap: usize,
    pub module: u32,
    pub threads: u32,
    pub parent: u32,
    pub priority: i32,
    pub flags: u32,
    pub exe: [u16; 260],
}

// GUID.
#[repr(C)]
pub struct Guid(pub u32, pub u16, pub u16, pub [u8; 8]);

// FILETIME.
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct FileTime {
    pub low: u32,
    pub high: u32,
}

// SYSTEMTIME.
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct SystemTime {
    pub year: u16,
    pub month: u16,
    pub weekday: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
    pub ms: u16,
}

// MEMORYSTATUSEX.
#[repr(C)]
pub struct MemoryStatus {
    pub length: u32,
    pub load: u32,
    pub total_phys: u64,
    pub avail_phys: u64,
    pub total_page: u64,
    pub avail_page: u64,
    pub total_virtual: u64,
    pub avail_virtual: u64,
    pub avail_extended: u64,
}

// INPUT, a key's or the mouse's (SendInput).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KeybdInput {
    pub vk: u16,
    pub scan: u16,
    pub flags: u32,
    pub time: u32,
    pub extra: usize,
}

// The union's largest member (MOUSEINPUT), so INPUT has the size Windows expects.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MouseInput {
    pub dx: i32,
    pub dy: i32,
    pub data: u32,
    pub flags: u32,
    pub time: u32,
    pub extra: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union InputUnion {
    pub mi: MouseInput,
    pub ki: KeybdInput,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Input {
    pub kind: u32,
    pub u: InputUnion,
}

// BLENDFUNCTION.
#[repr(C)]
pub struct Blend {
    pub op: u8,
    pub flags: u8,
    pub alpha: u8,
    pub format: u8,
}

// BITMAPINFOHEADER (BITMAPINFO's colours unused at 32 bits).
#[repr(C)]
pub struct BitmapInfoHeader {
    pub size: u32,
    pub width: i32,
    pub height: i32,
    pub planes: u16,
    pub bit_count: u16,
    pub compression: u32,
    pub size_image: u32,
    pub x_ppm: i32,
    pub y_ppm: i32,
    pub clr_used: u32,
    pub clr_important: u32,
}

// A low-level hook's (SetWindowsHookExW) and a WinEvent hook's (SetWinEventHook).
pub type HookProc = extern "system" fn(i32, usize, isize) -> isize;
pub type WinEventProc = extern "system" fn(isize, u32, Hwnd, i32, i32, u32, u32);

#[link(name = "advapi32")]
extern "system" {
    pub fn ConvertSecurityDescriptorToStringSecurityDescriptorW(descriptor: *mut c_void, revision: u32, info: u32, sddl: *mut *mut u16, len: *mut u32) -> i32;
    pub fn ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl: *const u16, revision: u32, descriptor: *mut *mut c_void, size: *mut u32) -> i32;
    pub fn GetTokenInformation(token: Handle, class: i32, info: *mut c_void, len: u32, needed: *mut u32) -> i32;
    pub fn OpenProcessToken(process: Handle, access: u32, token: *mut Handle) -> i32;
}

#[link(name = "dwmapi")]
extern "system" {
    pub fn DwmFlush() -> i32;
    pub fn DwmSetWindowAttribute(hwnd: Hwnd, attr: u32, value: *const c_void, size: u32) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    pub fn BitBlt(dst: isize, x: i32, y: i32, w: i32, h: i32, src: isize, sx: i32, sy: i32, rop: u32) -> i32;
    pub fn CreateCompatibleDC(hdc: isize) -> isize;
    pub fn CreateDIBSection(hdc: isize, info: *const BitmapInfoHeader, usage: u32, bits: *mut *mut u8, section: isize, offset: u32) -> isize;
    pub fn CreateSolidBrush(color: u32) -> isize;
    pub fn DeleteDC(hdc: isize) -> i32;
    pub fn DeleteObject(obj: isize) -> i32;
    pub fn SelectObject(hdc: isize, obj: isize) -> isize;
}

#[link(name = "kernel32")]
extern "system" {
    pub fn CloseHandle(handle: isize) -> i32;
    pub fn CreateFileW(name: *const u16, access: u32, share: u32, security: *mut c_void, disposition: u32, flags: u32, template: isize) -> isize;
    pub fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> Handle;
    pub fn GetCurrentProcess() -> isize;
    pub fn GetCurrentProcessId() -> u32;
    pub fn GetCurrentThreadId() -> u32;
    pub fn GetExitCodeProcess(process: Handle, code: *mut u32) -> i32;
    pub fn GetLastError() -> u32;
    pub fn GetLocalTime(time: *mut SystemTime);
    pub fn GetLocaleInfoEx(locale: *const u16, what: u32, out: *mut u16, len: i32) -> i32;
    pub fn GetModuleHandleW(name: *const u16) -> isize;
    pub fn GetProcAddress(module: isize, name: *const u8) -> *const ();
    pub fn GetProcessTimes(process: Handle, created: *mut FileTime, exited: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> i32;
    pub fn GetTickCount() -> u32;
    pub fn GlobalMemoryStatusEx(status: *mut MemoryStatus) -> i32;
    pub fn LoadLibraryExW(name: *const u16, file: isize, flags: u32) -> isize;
    pub fn LoadLibraryW(name: *const u16) -> isize;
    pub fn LocalFree(memory: *mut c_void) -> *mut c_void;
    pub fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
    pub fn Process32FirstW(snap: Handle, entry: *mut ProcessEntry) -> i32;
    pub fn Process32NextW(snap: Handle, entry: *mut ProcessEntry) -> i32;
    pub fn QueryFullProcessImageNameW(process: Handle, flags: u32, name: *mut u16, size: *mut u32) -> i32;
    pub fn TerminateProcess(process: isize, code: u32) -> i32;
    pub fn WaitForSingleObject(handle: isize, ms: u32) -> u32;
}

#[link(name = "ole32")]
extern "system" {
    pub fn CoCreateInstance(clsid: *const Guid, outer: *mut c_void, ctx: u32, iid: *const Guid, out: *mut *mut c_void) -> i32;
    pub fn CoInitializeEx(reserved: *mut c_void, flags: u32) -> i32;
    pub fn CoTaskMemFree(p: *mut c_void);
    pub fn CoUninitialize();
}

#[link(name = "shell32")]
extern "system" {
    pub fn SHCreateItemFromParsingName(path: *const u16, bind: *mut c_void, iid: *const Guid, out: *mut *mut c_void) -> i32;
    pub fn ShellExecuteW(hwnd: Hwnd, op: *const u16, file: *const u16, params: *const u16, dir: *const u16, show: i32) -> isize;
}

#[link(name = "user32")]
extern "system" {
    pub fn CallNextHookEx(hook: isize, code: i32, wparam: usize, lparam: isize) -> isize;
    pub fn CreateWindowExW(ex: u32, class: *const u16, title: *const u16, style: u32, x: i32, y: i32, w: i32, h: i32, parent: Hwnd, menu: isize, instance: isize, param: *mut c_void) -> Hwnd;
    pub fn DefWindowProcW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize;
    pub fn DestroyIcon(icon: isize) -> i32;
    pub fn DestroyWindow(hwnd: Hwnd) -> i32;
    pub fn DispatchMessageW(msg: *const Msg) -> isize;
    pub fn EnumWindows(each: extern "system" fn(Hwnd, isize) -> i32, param: isize) -> i32;
    pub fn FindWindowExW(parent: Hwnd, after: Hwnd, class: *const u16, title: *const u16) -> Hwnd;
    pub fn FindWindowW(class: *const u16, title: *const u16) -> Hwnd;
    pub fn GetAncestor(hwnd: Hwnd, flags: u32) -> Hwnd;
    pub fn GetAsyncKeyState(key: i32) -> i16;
    pub fn GetClassNameW(hwnd: Hwnd, name: *mut u16, max: i32) -> i32;
    pub fn GetCursorPos(pt: *mut Point) -> i32;
    pub fn GetDC(hwnd: Hwnd) -> isize;
    pub fn GetDesktopWindow() -> Hwnd;
    pub fn GetForegroundWindow() -> Hwnd;
    pub fn GetMessageW(msg: *mut Msg, hwnd: Hwnd, min: u32, max: u32) -> i32;
    pub fn GetMonitorInfoW(monitor: *mut c_void, info: *mut MonitorInfo) -> i32;
    pub fn GetShellWindow() -> Hwnd;
    pub fn GetSystemMetrics(index: i32) -> i32;
    pub fn GetWindow(hwnd: Hwnd, cmd: u32) -> Hwnd;
    pub fn GetWindowLongPtrW(hwnd: Hwnd, index: i32) -> isize;
    pub fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
    pub fn GetWindowTextW(hwnd: Hwnd, text: *mut u16, max: i32) -> i32;
    pub fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
    pub fn IsIconic(hwnd: Hwnd) -> i32;
    pub fn IsWindow(hwnd: Hwnd) -> i32;
    pub fn IsWindowVisible(hwnd: Hwnd) -> i32;
    pub fn IsZoomed(hwnd: Hwnd) -> i32;
    pub fn KillTimer(hwnd: Hwnd, id: usize) -> i32;
    pub fn MonitorFromPoint(pt: Point, flags: u32) -> *mut c_void;
    pub fn MonitorFromWindow(hwnd: Hwnd, flags: u32) -> *mut c_void;
    pub fn PeekMessageW(msg: *mut Msg, hwnd: Hwnd, min: u32, max: u32, remove: u32) -> i32;
    pub fn PostMessageW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> i32;
    pub fn PostQuitMessage(code: i32);
    pub fn PostThreadMessageW(thread: u32, msg: u32, wparam: usize, lparam: isize) -> i32;
    pub fn RegisterClassExW(class: *const WndClassExW) -> u16;
    pub fn RegisterWindowMessageW(name: *const u16) -> u32;
    pub fn ReleaseDC(hwnd: Hwnd, dc: isize) -> i32;
    pub fn SendInput(count: u32, inputs: *const Input, size: i32) -> u32;
    pub fn SendMessageTimeoutW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize, flags: u32, ms: u32, result: *mut usize) -> isize;
    pub fn SetForegroundWindow(hwnd: Hwnd) -> i32;
    pub fn SetLayeredWindowAttributes(hwnd: Hwnd, key: u32, alpha: u8, flags: u32) -> i32;
    pub fn SetTimer(hwnd: Hwnd, id: usize, ms: u32, f: *const c_void) -> usize;
    pub fn SetWinEventHook(min: u32, max: u32, module: isize, func: WinEventProc, pid: u32, tid: u32, flags: u32) -> isize;
    pub fn SetWindowLongPtrW(hwnd: Hwnd, index: i32, value: isize) -> isize;
    pub fn SetWindowPos(hwnd: Hwnd, after: isize, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
    pub fn SetWindowsHookExW(id: i32, proc_: HookProc, module: isize, thread: u32) -> isize;
    pub fn ShowWindow(hwnd: Hwnd, cmd: i32) -> i32;
    pub fn SystemParametersInfoW(action: u32, param: u32, pv: *mut c_void, flags: u32) -> i32;
    pub fn TranslateMessage(msg: *const Msg) -> i32;
    pub fn UnhookWinEvent(hook: isize) -> i32;
    pub fn UnhookWindowsHookEx(hook: isize) -> i32;
    pub fn UpdateLayeredWindow(hwnd: Hwnd, dst: isize, at: *const Point, size: *const Point, src: isize, from: *const Point, key: u32, blend: *const Blend, flags: u32) -> i32;
}

#[link(name = "version")]
extern "system" {
    pub fn GetFileVersionInfoSizeW(name: *const u16, handle: *mut u32) -> u32;
    pub fn GetFileVersionInfoW(name: *const u16, handle: u32, len: u32, data: *mut c_void) -> i32;
    pub fn VerQueryValueW(block: *const c_void, sub: *const u16, buffer: *mut *mut c_void, len: *mut u32) -> i32;
}

#[cfg(test)]
mod tests {
    // Each of Windows' calls declared in one place: in one part's file, or
    // here. Read from her source, as lock.rs reads her locks' names.
    #[test]
    fn each_call_is_declared_once() {
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    out.push(p);
                }
            }
        }
        // The calls a file declares: the fn lines inside its extern blocks.
        fn declared(text: &str) -> Vec<String> {
            let mut names = Vec::new();
            let mut inside = false;
            for line in text.lines() {
                let line = line.trim();
                if line.starts_with("extern \"system\" {") || line.starts_with("extern \"C\" {") {
                    inside = true;
                } else if inside && line == "}" {
                    inside = false;
                } else if inside {
                    let line = line.strip_prefix("pub(super) ").or_else(|| line.strip_prefix("pub(crate) ")).or_else(|| line.strip_prefix("pub ")).unwrap_or(line);
                    if let Some(rest) = line.strip_prefix("fn ") {
                        names.push(rest[..rest.find('(').unwrap_or(rest.len())].to_string());
                    }
                }
            }
            names
        }
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        walk(&src, &mut files);
        // Here first, so one declared here and in a part is said as the part's.
        files.sort_by_key(|f| (!f.ends_with("win32.rs"), f.clone()));
        let mut seen: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let mut twice = Vec::new();
        for f in &files {
            let at = f.strip_prefix(&src).unwrap().display().to_string();
            let mut names = declared(&std::fs::read_to_string(f).unwrap());
            // One file may say a call again in another of its own functions.
            names.sort();
            names.dedup();
            for name in names {
                if let Some(first) = seen.insert(name.clone(), at.clone()) {
                    twice.push(format!("{name}: {first} and {at}"));
                }
            }
        }
        assert!(seen.len() > 200, "found only {} declarations", seen.len());
        assert!(twice.is_empty(), "declared in two files (move it to win32.rs): {twice:#?}");
    }
}
