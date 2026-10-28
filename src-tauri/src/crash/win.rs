// Windows' side of crash reports: the unhandled-exception filter, a stack
// as module+offset, a hang seen from the guard (her windows not answering,
// a thread's stack read from outside), the box the guard asks in, WebView2's
// ProcessFailed, and what Windows says of itself (its version, WebView2's;
// its proxy is proxy.rs's).
use std::ffi::c_void;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use crate::Shared;

#[repr(C)]
struct ExceptionRecord {
    code: u32,
    flags: u32,
    record: *mut ExceptionRecord,
    address: *mut c_void,
    count: u32,
    info: [usize; 15],
}

#[repr(C)]
struct ExceptionPointers {
    record: *mut ExceptionRecord,
    context: *mut c_void,
}

type Filter = unsafe extern "system" fn(*mut ExceptionPointers) -> i32;

#[link(name = "kernel32")]
extern "system" {
    fn SetUnhandledExceptionFilter(filter: Option<Filter>) -> Option<Filter>;
    fn RtlCaptureStackBackTrace(skip: u32, count: u32, frames: *mut *mut c_void, hash: *mut u32) -> u16;
    fn GetModuleHandleExW(flags: u32, name: *const u16, module: *mut isize) -> i32;
    fn GetModuleFileNameW(module: isize, name: *mut u16, size: u32) -> u32;
    fn CreateFileW(name: *const u16, access: u32, share: u32, security: *mut c_void, disposition: u32, flags: u32, template: isize) -> isize;
    fn WriteFile(file: isize, buf: *const u8, len: u32, written: *mut u32, overlapped: *mut c_void) -> i32;
    fn CloseHandle(handle: isize) -> i32;
    fn GetCurrentThreadId() -> u32;
    fn GetCurrentProcess() -> isize;
    fn TerminateProcess(process: isize, code: u32) -> i32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
    fn OpenThread(access: u32, inherit: i32, thread: u32) -> isize;
    fn SuspendThread(thread: isize) -> u32;
    fn ResumeThread(thread: isize) -> u32;
    #[cfg(target_arch = "x86_64")]
    fn GetThreadContext(thread: isize, context: *mut Context) -> i32;
    fn K32GetModuleBaseNameW(process: isize, module: isize, name: *mut u16, size: u32) -> u32;
    fn LoadLibraryExW(name: *const u16, file: isize, flags: u32) -> isize;
    fn GetProcAddress(module: isize, name: *const u8) -> *const ();
}

type Hwnd = *mut c_void;

#[link(name = "user32")]
extern "system" {
    fn MessageBoxW(hwnd: isize, text: *const u16, caption: *const u16, kind: u32) -> i32;
    fn EnumWindows(each: extern "system" fn(Hwnd, isize) -> i32, param: isize) -> i32;
    fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
    fn IsHungAppWindow(hwnd: Hwnd) -> i32;
    fn GetClassNameW(hwnd: Hwnd, name: *mut u16, max: i32) -> i32;
}

const FROM_ADDRESS: u32 = 0x4;
const UNCHANGED_REFCOUNT: u32 = 0x2;
const GENERIC_WRITE: u32 = 0x4000_0000;
const CREATE_ALWAYS: u32 = 2;
const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;
const INVALID_HANDLE: isize = -1;
const ACCESS_VIOLATION: u32 = 0xC000_0005;

// Where the filter writes, made before (it allocates nothing); when she started.
static NATIVE_FILE: OnceLock<Vec<u16>> = OnceLock::new();
static STARTED: AtomicU64 = AtomicU64::new(0);
static WRITING: AtomicBool = AtomicBool::new(false);

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// Text put together without allocating: the heap may be what broke.
struct Buf {
    b: [u8; 3072],
    n: usize,
}

impl Buf {
    fn new() -> Self {
        Buf { b: [0; 3072], n: 0 }
    }

    fn text(&mut self, s: &str) {
        for &c in s.as_bytes() {
            if self.n < self.b.len() {
                self.b[self.n] = c;
                self.n += 1;
            }
        }
    }

    fn hex(&mut self, v: u64, min: usize) {
        let mut digits = [0u8; 16];
        let mut len = 0;
        let mut v = v;
        while v > 0 || len < min.max(1) {
            digits[len] = b"0123456789abcdef"[(v & 0xf) as usize];
            v >>= 4;
            len += 1;
            if len == 16 {
                break;
            }
        }
        for i in (0..len).rev() {
            self.text(std::str::from_utf8(&digits[i..i + 1]).unwrap_or("0"));
        }
    }

    fn dec(&mut self, v: u64) {
        let mut digits = [0u8; 20];
        let mut len = 0;
        let mut v = v;
        loop {
            digits[len] = b'0' + (v % 10) as u8;
            v /= 10;
            len += 1;
            if v == 0 || len == 20 {
                break;
            }
        }
        for i in (0..len).rev() {
            self.text(std::str::from_utf8(&digits[i..i + 1]).unwrap_or("0"));
        }
    }

    // An address as the module it is in and how far into it: ntdll.dll+0x1a2b.
    fn place(&mut self, addr: usize) {
        let mut module = 0isize;
        // SAFETY: asks which module holds an address; nothing is loaded or kept.
        let ok = unsafe { GetModuleHandleExW(FROM_ADDRESS | UNCHANGED_REFCOUNT, addr as *const u16, &mut module) } != 0;
        if !ok || module == 0 {
            self.text("unknown+0x");
            self.hex(addr as u64, 1);
            return;
        }
        let mut name = [0u16; 260];
        // SAFETY: our own buffer, its size passed along.
        let len = unsafe { GetModuleFileNameW(module, name.as_mut_ptr(), 260) } as usize;
        let name = &name[..len.min(260)];
        let start = name.iter().rposition(|&c| c == u16::from(b'\\')).map_or(0, |i| i + 1);
        let mut put = 0;
        for &c in &name[start..] {
            if put == 64 {
                break;
            }
            let c = u8::try_from(c).ok().filter(|c| c.is_ascii_alphanumeric() || b"_.-".contains(c)).unwrap_or(b'_');
            self.text(std::str::from_utf8(&[c]).unwrap_or("_"));
            put += 1;
        }
        if put == 0 {
            self.text("unknown");
        }
        self.text("+0x");
        self.hex((addr - module as usize) as u64, 1);
    }

    fn as_str(&self) -> &str {
        std::str::from_utf8(&self.b[..self.n]).unwrap_or("")
    }
}

// Her stack as module+offset, the newest first (at most 32; `skip` of them
// left out at the top).
pub fn frames(skip: u32) -> Vec<String> {
    let mut frames = [std::ptr::null_mut::<c_void>(); 32];
    // SAFETY: our own array, its length passed along.
    let n = unsafe { RtlCaptureStackBackTrace(skip, 32, frames.as_mut_ptr(), std::ptr::null_mut()) } as usize;
    frames[..n.min(32)]
        .iter()
        .map(|&f| {
            let mut b = Buf::new();
            b.place(f as usize);
            b.as_str().to_string()
        })
        .collect()
}

// Called by Windows for an exception nobody handled, on the thread it
// happened on, before she is ended: what and where, written down. Nothing
// allocated, nothing locked; once.
unsafe extern "system" fn on_crash(pointers: *mut ExceptionPointers) -> i32 {
    if WRITING.swap(true, Ordering::SeqCst) {
        return 0;
    }
    let Some(path) = NATIVE_FILE.get() else { return 0 };
    let mut out = Buf::new();
    // SAFETY: Windows hands the exception's pointers, alive for this call.
    let record = if pointers.is_null() { std::ptr::null_mut() } else { unsafe { (*pointers).record } };
    if !record.is_null() {
        // SAFETY: as above.
        let r = unsafe { &*record };
        out.text("code=");
        out.hex(u64::from(r.code), 8);
        out.text("\nwhere=");
        out.place(r.address as usize);
        out.text("\n");
        if r.code == ACCESS_VIOLATION && r.count >= 2 {
            out.text("access=");
            out.text(match r.info[0] {
                0 => "read",
                1 => "write",
                8 => "execute",
                _ => "other",
            });
            if r.info[1] < 0x10000 {
                out.text(" null+0x");
                out.hex(r.info[1] as u64, 1);
            }
            out.text("\n");
        }
    }
    out.text("started=");
    out.dec(STARTED.load(Ordering::SeqCst));
    out.text("\nthread=");
    // SAFETY: no arguments.
    out.dec(u64::from(unsafe { GetCurrentThreadId() }));
    out.text("\n");
    let mut frames = [std::ptr::null_mut::<c_void>(); 32];
    // SAFETY: our own array, its length passed along.
    let n = unsafe { RtlCaptureStackBackTrace(0, 32, frames.as_mut_ptr(), std::ptr::null_mut()) } as usize;
    for &f in &frames[..n.min(32)] {
        out.text("frame=");
        out.place(f as usize);
        out.text("\n");
    }
    // SAFETY: a path made before, null-terminated; our own buffer written.
    unsafe {
        let file = CreateFileW(path.as_ptr(), GENERIC_WRITE, 0, std::ptr::null_mut(), CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, 0);
        if file != INVALID_HANDLE && file != 0 {
            let mut written = 0u32;
            WriteFile(file, out.b.as_ptr(), out.n as u32, &mut written, std::ptr::null_mut());
            CloseHandle(file);
        }
    }
    // Ended now, her exit code the exception's: left to Windows, its error
    // reporting kept her (and the taskbar she held, which the guard gives
    // back once she is gone) eight or nine seconds more for a dump nobody
    // reads (2026-10-11). No WER entry in the event log for it.
    if !record.is_null() {
        // SAFETY: as above; this process's own pseudo-handle.
        unsafe { TerminateProcess(GetCurrentProcess(), (*record).code) };
    }
    0
}

// The filter set, writing to `file`; `started`, her start (ms).
pub fn catch_native(file: &Path, started: u64) {
    STARTED.store(started, Ordering::SeqCst);
    if NATIVE_FILE.set(wide(&file.to_string_lossy())).is_ok() {
        // SAFETY: a function of ours that lives as long as the process.
        unsafe { SetUnhandledExceptionFilter(Some(on_crash)) };
    }
}

// Ended at once with `code`, nothing written (a crash test).
pub fn end_with(code: u32) {
    // SAFETY: this process's own pseudo-handle.
    unsafe { TerminateProcess(GetCurrentProcess(), code) };
}

// --- A hang, seen from the guard --------------------------------------------------------

fn class_of(hwnd: Hwnd) -> String {
    let mut name = [0u16; 64];
    // SAFETY: a buffer of our own, its size passed along.
    let n = unsafe { GetClassNameW(hwnd, name.as_mut_ptr(), 64) }.max(0) as usize;
    String::from_utf16_lossy(&name[..n])
}

// Every GUI thread has these; they say nothing of whose it is.
fn is_ime(class: &str) -> bool {
    class == "IME" || class == "MSCTFIME UI"
}

// The threads of process `pid` with a window Windows takes for not
// answering (IsHungAppWindow: five seconds and more without a look at its
// messages, as for its ghost window and the "not responding" box), each with
// the class of one of its windows (not an IME's, if it has another).
pub fn hung(pid: u32) -> Vec<(u32, String)> {
    struct Seen {
        pid: u32,
        found: Vec<(u32, String)>,
    }
    extern "system" fn each(hwnd: Hwnd, param: isize) -> i32 {
        // SAFETY: param is the Seen hung() passed, alive for the enumeration.
        let seen = unsafe { &mut *(param as *mut Seen) };
        let mut pid = 0u32;
        // SAFETY: a window Windows hands us; our own u32.
        let thread = unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        // SAFETY: as above.
        if pid == seen.pid && unsafe { IsHungAppWindow(hwnd) } != 0 {
            let class = class_of(hwnd);
            match seen.found.iter_mut().find(|(t, _)| *t == thread) {
                Some((_, was)) if is_ime(was) && !is_ime(&class) => *was = class,
                Some(_) => {}
                None => seen.found.push((thread, class)),
            }
        }
        1
    }
    let mut seen = Seen { pid, found: Vec::new() };
    // SAFETY: a callback of ours and the Seen it fills, alive for the call.
    unsafe { EnumWindows(each, &mut seen as *mut Seen as isize) };
    seen.found
}

// dbghelp's functions, from Windows' own (System32 alone), looked for once:
// SymInitializeW, SymCleanup, SymSetOptions, StackWalk64,
// SymFunctionTableAccess64, SymGetModuleBase64. Only the guard asks, and
// only for a hang: her start loads none of it.
#[cfg(target_arch = "x86_64")]
fn dbghelp() -> Option<[usize; 6]> {
    const SEARCH_SYSTEM32: u32 = 0x800;
    static FOUND: OnceLock<Option<[usize; 6]>> = OnceLock::new();
    *FOUND.get_or_init(|| {
        // SAFETY: a name of our own, null-terminated; the module kept loaded.
        let module = unsafe { LoadLibraryExW(wide("dbghelp.dll").as_ptr(), 0, SEARCH_SYSTEM32) };
        if module == 0 {
            return None;
        }
        let mut found = [0usize; 6];
        for (at, name) in [&b"SymInitializeW\0"[..], b"SymCleanup\0", b"SymSetOptions\0", b"StackWalk64\0", b"SymFunctionTableAccess64\0", b"SymGetModuleBase64\0"].iter().enumerate() {
            // SAFETY: a module we loaded; names null-terminated.
            found[at] = unsafe { GetProcAddress(module, name.as_ptr()) } as usize;
            if found[at] == 0 {
                return None;
            }
        }
        Some(found)
    })
}

// A thread's registers, as GetThreadContext fills them on x64 (CONTEXT:
// 1232 bytes; the flags at 0x30, Rsp 0x98, Rbp 0xA0, Rip 0xF8).
#[cfg(target_arch = "x86_64")]
#[repr(C, align(16))]
struct Context([u64; 154]);

// The stack of thread `thread` of process `pid`, as module+offset, the
// newest first (at most 32), read from outside: the thread stopped for the
// moment it takes, and walked by dbghelp (StackWalk64), which unwinds by the
// modules' own tables, no symbols needed. Empty when it cannot be read
// (dbghelp missing, the thread gone, not ours to open).
#[cfg(target_arch = "x86_64")]
pub fn stack_of(pid: u32, thread: u32) -> Vec<String> {
    const PROCESS_QUERY_INFORMATION: u32 = 0x0400;
    const PROCESS_VM_READ: u32 = 0x0010;
    const THREAD_SUSPEND_RESUME: u32 = 0x0002;
    const THREAD_GET_CONTEXT: u32 = 0x0008;
    const THREAD_QUERY_INFORMATION: u32 = 0x0040;
    const CONTEXT_FULL: u64 = 0x0010_000B;
    const MACHINE_AMD64: u32 = 0x8664;
    // ADDRESS64's Mode (AddrModeFlat) in the upper half of its second word.
    const FLAT: u64 = 3 << 32;
    // SYMOPT_DEFERRED_LOADS | SYMOPT_FAIL_CRITICAL_ERRORS | SYMOPT_NO_PROMPTS.
    const OPTIONS: u32 = 0x4 | 0x200 | 0x8_0000;
    type Init = unsafe extern "system" fn(isize, *const u16, i32) -> i32;
    type Cleanup = unsafe extern "system" fn(isize) -> i32;
    type Options = unsafe extern "system" fn(u32) -> u32;
    // STACKFRAME64 is 264 bytes; room for more.
    type Walk = unsafe extern "system" fn(u32, isize, isize, *mut [u64; 40], *mut Context, usize, usize, usize, usize) -> i32;
    type ModuleBase = unsafe extern "system" fn(isize, u64) -> u64;

    let Some([init, cleanup, options, walk, table, base]) = dbghelp() else { return Vec::new() };
    // SAFETY: dbghelp's functions by their own signatures; handles we close;
    // the thread resumed whatever the walk did; buffers of our own.
    unsafe {
        let (init, cleanup, options, walk, module_base) = (std::mem::transmute::<usize, Init>(init), std::mem::transmute::<usize, Cleanup>(cleanup), std::mem::transmute::<usize, Options>(options), std::mem::transmute::<usize, Walk>(walk), std::mem::transmute::<usize, ModuleBase>(base));
        let process = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid);
        if process == 0 {
            return Vec::new();
        }
        let handle = OpenThread(THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT | THREAD_QUERY_INFORMATION, 0, thread);
        let mut pcs = Vec::with_capacity(32);
        options(OPTIONS);
        // Its modules listed (invade), for their tables and bases.
        if handle != 0 && init(process, std::ptr::null(), 1) != 0 {
            if SuspendThread(handle) != u32::MAX {
                let mut context = Context([0; 154]);
                context.0[6] = CONTEXT_FULL;
                if GetThreadContext(handle, &mut context) != 0 {
                    let mut frame = [0u64; 40];
                    (frame[0], frame[1]) = (context.0[31], FLAT);
                    (frame[4], frame[5]) = (context.0[20], FLAT);
                    (frame[6], frame[7]) = (context.0[19], FLAT);
                    while pcs.len() < 32 && walk(MACHINE_AMD64, process, handle, &mut frame, &mut context, 0, table, base, 0) != 0 && frame[0] != 0 {
                        pcs.push(frame[0]);
                    }
                }
                ResumeThread(handle);
            }
            let frames = pcs.iter().map(|&pc| place_in(process, module_base(process, pc), pc)).collect();
            cleanup(process);
            CloseHandle(handle);
            CloseHandle(process);
            return frames;
        }
        if handle != 0 {
            CloseHandle(handle);
        }
        CloseHandle(process);
        Vec::new()
    }
}

#[cfg(test)]
pub fn thread_id() -> u32 {
    // SAFETY: no arguments.
    unsafe { GetCurrentThreadId() }
}

#[cfg(not(target_arch = "x86_64"))]
pub fn stack_of(_pid: u32, _thread: u32) -> Vec<String> {
    Vec::new()
}

// An address in another process as Buf::place writes one in hers:
// module+offset, the module's name as a report takes it.
#[cfg(target_arch = "x86_64")]
fn place_in(process: isize, module: u64, pc: u64) -> String {
    let mut name = [0u16; 260];
    // SAFETY: a module of that process (dbghelp's base for the address), our own buffer.
    let n = if module == 0 { 0 } else { (unsafe { K32GetModuleBaseNameW(process, module as isize, name.as_mut_ptr(), 260) }) as usize };
    let name: String = String::from_utf16_lossy(&name[..n.min(260)]).chars().take(64).map(|c| if c.is_ascii_alphanumeric() || "_.-".contains(c) { c } else { '_' }).collect();
    let name = if name.is_empty() { "unknown".to_string() } else { name };
    format!("{name}+0x{:x}", pc - module)
}

// Windows' own Yes/No box, on top: Yes or not.
pub fn ask(title: &str, text: &str) -> bool {
    const MB_YESNO: u32 = 0x4;
    const MB_ICONWARNING: u32 = 0x30;
    const MB_SETFOREGROUND: u32 = 0x1_0000;
    const MB_TOPMOST: u32 = 0x4_0000;
    const IDYES: i32 = 6;
    // SAFETY: two strings of our own, null-terminated.
    unsafe { MessageBoxW(0, wide(text).as_ptr(), wide(title).as_ptr(), MB_YESNO | MB_ICONWARNING | MB_SETFOREGROUND | MB_TOPMOST) == IDYES }
}

// "Windows 11 25H2 (26200.6584) x64": its build, from the registry (its
// ProductName says 10 on 11 too).
pub fn os() -> String {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => other,
    };
    let Ok(key) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion") else {
        return format!("Windows {arch}");
    };
    let build: String = key.get_value("CurrentBuild").unwrap_or_default();
    let ubr: Option<u32> = key.get_value("UBR").ok();
    let display: String = key.get_value("DisplayVersion").unwrap_or_default();
    let major = if build.parse::<u32>().is_ok_and(|b| b >= 22000) { "11" } else { "10" };
    let build = match ubr {
        Some(ubr) => format!("{build}.{ubr}"),
        None => build,
    };
    let display = if display.is_empty() { String::new() } else { format!(" {display}") };
    clean(&format!("Windows {major}{display} ({build}) {arch}"))
}

fn clean(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric() || " .()-_".contains(*c)).take(80).collect()
}

// WebView2's version, as EdgeUpdate keeps it.
pub fn webview_version() -> Option<String> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;
    const CLIENT: &str = r"Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
    let places = [(HKEY_LOCAL_MACHINE, format!(r"SOFTWARE\WOW6432Node\{CLIENT}")), (HKEY_LOCAL_MACHINE, format!(r"SOFTWARE\{CLIENT}")), (HKEY_CURRENT_USER, format!(r"Software\{CLIENT}"))];
    places.iter().find_map(|(root, path)| {
        let v: String = RegKey::predef(*root).open_subkey(path).ok()?.get_value("pv").ok()?;
        (v.chars().all(|c| c.is_ascii_digit() || c == '.') && !v.is_empty() && v != "0.0.0.0").then(|| v.chars().take(24).collect())
    })
}

// One of WebView2's processes gone, for this window: told (crash.rs
// webview_failed), and its page loaded again when that says so.
pub fn watch(sh: &Arc<Shared>, window: &tauri::WebviewWindow) -> tauri::Result<()> {
    use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2ProcessFailedEventArgs2, COREWEBVIEW2_PROCESS_FAILED_KIND, COREWEBVIEW2_PROCESS_FAILED_REASON};
    use webview2_com::ProcessFailedEventHandler;
    use windows_core::Interface;

    let label = window.label().to_string();
    let sh = sh.clone();
    window.with_webview(move |view| {
        // SAFETY: WebView2's own objects, on its thread, as it hands them.
        unsafe {
            let Ok(core) = view.controller().CoreWebView2() else { return };
            let handler = ProcessFailedEventHandler::create(Box::new(move |webview, args| {
                let Some(args) = args else { return Ok(()) };
                let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND::default();
                args.ProcessFailedKind(&mut kind)?;
                let mut reason = COREWEBVIEW2_PROCESS_FAILED_REASON::default();
                let mut code = 0i32;
                if let Ok(more) = args.cast::<ICoreWebView2ProcessFailedEventArgs2>() {
                    let _ = more.Reason(&mut reason);
                    let _ = more.ExitCode(&mut code);
                }
                if super::webview_failed(&sh, &label, kind.0, reason.0, code) {
                    if let Some(webview) = webview {
                        let _ = webview.Reload();
                    }
                }
                Ok(())
            }));
            let mut token = 0i64;
            let _ = core.add_ProcessFailed(&handler, &mut token);
        }
    })
}
