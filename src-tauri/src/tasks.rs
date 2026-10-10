// The windows the taskbar has buttons for (taskbar.rs): those Windows' own
// taskbar shows, as near as can be told from outside it (shown, not on
// another desktop, not owned by another, not a tool window; or marked to be
// on the taskbar). And what a press on a button does: the window to the
// front, restored if minimized, or minimized if it is the one in front.
// Windows only.

pub struct Task {
    pub hwnd: isize,
    pub title: String,
    // Its program's path (a store app's own, not its frame's): the button it
    // goes on, one per program.
    pub path: String,
    pub min: bool,
}

// The keyboard as Windows' taskbar shows it: the language of the window in
// front (its keyboard layout's), whether its input method types the
// language's own script or plain letters (中 or 英), and Caps Lock.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Keys {
    pub lang: u16,
    pub native: Option<bool>,
    pub caps: bool,
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;

    use super::{Keys, Task};

    type Hwnd = *mut c_void;
    type Handle = isize;

    #[link(name = "imm32")]
    extern "system" {
        fn ImmGetDefaultIMEWnd(hwnd: Hwnd) -> Hwnd;
    }

    #[link(name = "iphlpapi")]
    extern "system" {
        fn GetBestInterface(dest: u32, index: *mut u32) -> u32;
        fn ConvertInterfaceIndexToLuid(index: u32, luid: *mut u64) -> u32;
    }

    #[repr(C)]
    struct ShFileInfo {
        icon: isize,
        index: i32,
        attributes: u32,
        display_name: [u16; 260],
        type_name: [u16; 80],
    }

    #[link(name = "shell32")]
    extern "system" {
        fn SHGetFileInfoW(path: *const u16, attributes: u32, info: *mut ShFileInfo, size: u32, flags: u32) -> usize;
        fn ShellExecuteW(hwnd: Hwnd, op: *const u16, file: *const u16, params: *const u16, dir: *const u16, show: i32) -> isize;
    }

    #[link(name = "version")]
    extern "system" {
        fn GetFileVersionInfoSizeW(name: *const u16, handle: *mut u32) -> u32;
        fn GetFileVersionInfoW(name: *const u16, handle: u32, len: u32, data: *mut c_void) -> i32;
        fn VerQueryValueW(block: *const c_void, sub: *const u16, buffer: *mut *mut c_void, len: *mut u32) -> i32;
    }

    #[link(name = "user32")]
    extern "system" {
        fn EnumWindows(each: extern "system" fn(Hwnd, isize) -> i32, param: isize) -> i32;
        fn IsWindowVisible(hwnd: Hwnd) -> i32;
        fn IsIconic(hwnd: Hwnd) -> i32;
        fn IsWindow(hwnd: Hwnd) -> i32;
        fn GetWindow(hwnd: Hwnd, cmd: u32) -> Hwnd;
        fn GetWindowLongPtrW(hwnd: Hwnd, index: i32) -> isize;
        fn GetWindowTextW(hwnd: Hwnd, text: *mut u16, max: i32) -> i32;
        fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
        fn GetClassNameW(hwnd: Hwnd, name: *mut u16, max: i32) -> i32;
        fn GetForegroundWindow() -> Hwnd;
        fn ShowWindow(hwnd: Hwnd, cmd: i32) -> i32;
        fn PostMessageW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> i32;
        fn SendMessageTimeoutW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize, flags: u32, ms: u32, result: *mut usize) -> isize;
        fn GetClassLongPtrW(hwnd: Hwnd, index: i32) -> usize;
        fn FindWindowExW(parent: Hwnd, after: Hwnd, class: *const u16, title: *const u16) -> Hwnd;
        fn GetKeyboardLayout(thread: u32) -> isize;
        fn GetKeyState(vk: i32) -> i16;
        fn DestroyIcon(icon: isize) -> i32;
    }

    #[link(name = "dwmapi")]
    extern "system" {
        fn DwmGetWindowAttribute(hwnd: Hwnd, attr: u32, value: *mut c_void, size: u32) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
        fn CloseHandle(handle: Handle) -> i32;
        fn QueryFullProcessImageNameW(process: Handle, flags: u32, name: *mut u16, size: *mut u32) -> i32;
    }

    const GW_OWNER: u32 = 4;
    const GWL_EXSTYLE: i32 = -20;
    const WS_EX_TOOLWINDOW: isize = 0x80;
    const WS_EX_APPWINDOW: isize = 0x0004_0000;
    const WS_EX_NOACTIVATE: isize = 0x0800_0000;
    const DWMWA_CLOAKED: u32 = 14;
    const WM_GETICON: u32 = 0x007F;
    const WM_CLOSE: u32 = 0x0010;
    const ICON_SMALL: usize = 0;
    const ICON_BIG: usize = 1;
    const ICON_SMALL2: usize = 2;
    const GCLP_HICON: i32 = -14;
    const GCLP_HICONSM: i32 = -34;
    const SMTO_ABORTIFHUNG: u32 = 0x2;
    const SW_MINIMIZE: i32 = 6;
    const SW_SHOWNORMAL: i32 = 1;
    const SHGFI_ICON: u32 = 0x100;
    const SHGFI_SMALLICON: u32 = 0x1;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    // The desktop and the taskbars themselves: shown, unowned, never buttons.
    const NOT_TASKS: [&str; 5] = ["Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd", "WakuwakuTaskbarHost"];

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn class_of(hwnd: Hwnd) -> String {
        let mut name = [0u16; 128];
        // SAFETY: our own buffer, its size passed.
        let n = unsafe { GetClassNameW(hwnd, name.as_mut_ptr(), 128) }.max(0) as usize;
        String::from_utf16_lossy(&name[..n])
    }

    fn title_of(hwnd: Hwnd) -> String {
        let mut text = [0u16; 512];
        // SAFETY: as above.
        let n = unsafe { GetWindowTextW(hwnd, text.as_mut_ptr(), 512) }.max(0) as usize;
        String::from_utf16_lossy(&text[..n])
    }

    fn pid_of(hwnd: Hwnd) -> u32 {
        let mut pid = 0;
        // SAFETY: our own out-parameter.
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        pid
    }

    // A process's program, its whole path.
    fn path_of(pid: u32) -> String {
        // SAFETY: a handle we close; the name goes into our own buffer.
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process == 0 {
                return String::new();
            }
            let mut name = [0u16; 512];
            let mut size = 512u32;
            let ok = QueryFullProcessImageNameW(process, 0, name.as_mut_ptr(), &mut size);
            CloseHandle(process);
            if ok == 0 {
                return String::new();
            }
            String::from_utf16_lossy(&name[..size as usize])
        }
    }

    pub fn file_name(path: &str) -> String {
        path.rsplit('\\').next().unwrap_or(path).to_string()
    }

    // A program's name as it gives it (its file's description: "Google
    // Chrome", not chrome.exe), else its file's name without .exe.
    pub fn app_name(path: &str) -> String {
        let stem = || {
            let name = file_name(path);
            name.strip_suffix(".exe").or_else(|| name.strip_suffix(".EXE")).unwrap_or(&name).to_string()
        };
        let file = wide(path);
        // SAFETY: a buffer of the size the call asks for; the values read
        // point into it and are copied before it goes.
        unsafe {
            let size = GetFileVersionInfoSizeW(file.as_ptr(), std::ptr::null_mut());
            if size == 0 {
                return stem();
            }
            let mut data = vec![0u8; size as usize];
            if GetFileVersionInfoW(file.as_ptr(), 0, size, data.as_mut_ptr() as *mut c_void) == 0 {
                return stem();
            }
            let mut at: *mut c_void = std::ptr::null_mut();
            let mut len = 0u32;
            let translation = wide("\\VarFileInfo\\Translation");
            if VerQueryValueW(data.as_ptr() as *const c_void, translation.as_ptr(), &mut at, &mut len) == 0 || len < 4 {
                return stem();
            }
            let (lang, page) = (*(at as *const u16), *(at as *const u16).add(1));
            let key = wide(&format!("\\StringFileInfo\\{lang:04x}{page:04x}\\FileDescription"));
            if VerQueryValueW(data.as_ptr() as *const c_void, key.as_ptr(), &mut at, &mut len) == 0 || len <= 1 {
                return stem();
            }
            let text = std::slice::from_raw_parts(at as *const u16, len as usize);
            let name = String::from_utf16_lossy(text).trim_end_matches('\0').trim().to_string();
            if name.is_empty() { stem() } else { name }
        }
    }

    // A program file's small icon, ours to destroy (destroy_icon).
    pub fn file_icon(path: &str) -> isize {
        let mut info: ShFileInfo = unsafe { std::mem::zeroed() };
        // SAFETY: our own struct, its size passed.
        unsafe { SHGetFileInfoW(wide(path).as_ptr(), 0, &mut info, std::mem::size_of::<ShFileInfo>() as u32, SHGFI_ICON | SHGFI_SMALLICON) };
        info.icon
    }

    pub fn destroy_icon(icon: isize) {
        if icon != 0 {
            // SAFETY: an icon of ours (file_icon's).
            unsafe { DestroyIcon(icon) };
        }
    }

    // A program started, as a double press in Explorer starts it.
    pub fn launch(path: &str) -> bool {
        let dir = path.rsplit_once('\\').map(|(d, _)| d.to_string()).unwrap_or_default();
        // SAFETY: strings of our own, null-terminated.
        unsafe { ShellExecuteW(std::ptr::null_mut(), wide("open").as_ptr(), wide(path).as_ptr(), std::ptr::null(), wide(&dir).as_ptr(), SW_SHOWNORMAL) > 32 }
    }

    // Cloaked: on another virtual desktop, or a store app's window not running.
    fn is_cloaked(hwnd: Hwnd) -> bool {
        let mut cloaked = 0u32;
        // SAFETY: our own u32, of the size passed.
        let got = unsafe { DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, &mut cloaked as *mut u32 as *mut c_void, 4) };
        got == 0 && cloaked != 0
    }

    fn is_task(hwnd: Hwnd, me: u32) -> bool {
        // SAFETY: plain queries about a window being enumerated.
        unsafe {
            if IsWindowVisible(hwnd) == 0 || pid_of(hwnd) == me {
                return false;
            }
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let app = ex & WS_EX_APPWINDOW != 0;
            if !app && (!GetWindow(hwnd, GW_OWNER).is_null() || ex & (WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE) != 0) {
                return false;
            }
        }
        !is_cloaked(hwnd) && !NOT_TASKS.contains(&class_of(hwnd).as_str()) && !title_of(hwnd).is_empty()
    }

    extern "system" fn collect(hwnd: Hwnd, param: isize) -> i32 {
        // SAFETY: param is the Vec list() passed, alive for the enumeration.
        let found = unsafe { &mut *(param as *mut Vec<isize>) };
        found.push(hwnd as isize);
        1
    }

    // A store app's frame (ApplicationFrameHost) is not the app: the app is
    // the process of the window inside it.
    fn app_path(hwnd: Hwnd, path: String) -> String {
        if !file_name(&path).eq_ignore_ascii_case("ApplicationFrameHost.exe") {
            return path;
        }
        // SAFETY: a class name of our own; looking among the frame's children.
        let core = unsafe { FindWindowExW(hwnd, std::ptr::null_mut(), wide("Windows.UI.Core.CoreWindow").as_ptr(), std::ptr::null()) };
        if core.is_null() {
            return path;
        }
        let inner = path_of(pid_of(core));
        if inner.is_empty() { path } else { inner }
    }

    // The windows with buttons, front to back.
    pub fn list() -> Vec<Task> {
        let mut all: Vec<isize> = Vec::new();
        // SAFETY: collect() only pushes, while all lives.
        unsafe { EnumWindows(collect, &mut all as *mut Vec<isize> as isize) };
        let me = std::process::id();
        all.into_iter()
            .filter(|&h| is_task(h as Hwnd, me))
            .map(|h| {
                let hwnd = h as Hwnd;
                // SAFETY: a plain query.
                let min = unsafe { IsIconic(hwnd) } != 0;
                let path = app_path(hwnd, path_of(pid_of(hwnd)));
                Task { hwnd: h, title: title_of(hwnd), path, min }
            })
            .collect()
    }

    // Its icon (the window's own, else its class's), the small one first;
    // the program's handle, not ours: drawn at once, never kept.
    pub fn icon_of(hwnd: isize) -> isize {
        let h = hwnd as Hwnd;
        for which in [ICON_SMALL2, ICON_SMALL, ICON_BIG] {
            let mut icon = 0usize;
            // SAFETY: a window's own answer, waited for at most 50 ms (a hung one gives none).
            let ok = unsafe { SendMessageTimeoutW(h, WM_GETICON, which, 0, SMTO_ABORTIFHUNG, 50, &mut icon) };
            if ok != 0 && icon != 0 {
                return icon as isize;
            }
        }
        // SAFETY: plain queries.
        unsafe {
            let small = GetClassLongPtrW(h, GCLP_HICONSM);
            if small != 0 {
                return small as isize;
            }
            GetClassLongPtrW(h, GCLP_HICON) as isize
        }
    }

    pub fn front() -> isize {
        // SAFETY: a plain query.
        unsafe { GetForegroundWindow() as isize }
    }

    pub fn alive(hwnd: isize) -> bool {
        // SAFETY: a plain query.
        unsafe { IsWindow(hwnd as Hwnd) != 0 }
    }

    // A press on its button: minimized when it is the one in front, else to
    // the front (restored if minimized), as Windows' taskbar does.
    pub fn press(hwnd: isize) -> bool {
        let h = hwnd as Hwnd;
        // SAFETY: plain calls about a window; a stale handle only makes them fail.
        unsafe {
            if GetForegroundWindow() == h && IsIconic(h) == 0 {
                return ShowWindow(h, SW_MINIMIZE) != 0;
            }
        }
        crate::jump::bring_window(hwnd)
    }

    // Asked to close, as its own ✕ does.
    pub fn close(hwnd: isize) -> bool {
        // SAFETY: a posted message, no pointers.
        unsafe { PostMessageW(hwnd as Hwnd, WM_CLOSE, 0, 0) != 0 }
    }

    const WM_IME_CONTROL: u32 = 0x0283;
    const IMC_GETCONVERSIONMODE: usize = 0x1;
    const IMC_SETCONVERSIONMODE: usize = 0x2;
    const IMC_GETOPENSTATUS: usize = 0x5;
    const IME_CMODE_NATIVE: usize = 0x1;
    const VK_CAPITAL: i32 = 0x14;

    // An input method's answer about the window in front (its default IME
    // window, as input-method indicators ask): None if it gives none.
    fn ime_ask(ime: Hwnd, what: usize, value: isize) -> Option<usize> {
        let mut answer = 0usize;
        // SAFETY: the IME window's answer, waited for at most 50 ms.
        let ok = unsafe { SendMessageTimeoutW(ime, WM_IME_CONTROL, what, value, SMTO_ABORTIFHUNG, 50, &mut answer) };
        (ok != 0).then_some(answer)
    }

    fn front_ime() -> (u16, Hwnd) {
        // SAFETY: plain queries about the window in front.
        unsafe {
            let front = GetForegroundWindow();
            let thread = GetWindowThreadProcessId(front, std::ptr::null_mut());
            let lang = (GetKeyboardLayout(thread) & 0xffff) as u16;
            (lang, ImmGetDefaultIMEWnd(front))
        }
    }

    pub fn keys() -> Keys {
        let (lang, ime) = front_ime();
        let native = (!ime.is_null()).then(|| {
            let open = ime_ask(ime, IMC_GETOPENSTATUS, 0)?;
            let mode = ime_ask(ime, IMC_GETCONVERSIONMODE, 0)?;
            Some(open != 0 && mode & IME_CMODE_NATIVE != 0)
        });
        // SAFETY: a plain query; the low bit is the toggle's.
        let caps = unsafe { GetKeyState(VK_CAPITAL) } & 1 != 0;
        Keys { lang, native: native.flatten(), caps }
    }

    // The network's way out, as Windows' taskbar shows it: "wired", "wifi",
    // "other" (a VPN's, a phone's) or "none": the interface Windows would
    // send to a public address by, its type in its LUID's top 16 bits.
    pub fn net() -> &'static str {
        let mut index = 0u32;
        let mut luid = 0u64;
        // SAFETY: our own out-parameters; 1.1.1.1 reads the same in either byte order.
        unsafe {
            if GetBestInterface(u32::from_ne_bytes([1, 1, 1, 1]), &mut index) != 0 {
                return "none";
            }
            if ConvertInterfaceIndexToLuid(index, &mut luid) != 0 {
                return "other";
            }
        }
        match luid >> 48 {
            6 => "wired",
            71 => "wifi",
            _ => "other",
        }
    }

    // The window in front's input method between its own script and plain letters.
    pub fn toggle_native() -> bool {
        let (_, ime) = front_ime();
        if ime.is_null() {
            return false;
        }
        let Some(mode) = ime_ask(ime, IMC_GETCONVERSIONMODE, 0) else { return false };
        ime_ask(ime, IMC_SETCONVERSIONMODE, (mode ^ IME_CMODE_NATIVE) as isize).is_some()
    }
}

#[cfg(not(windows))]
mod imp {
    use super::{Keys, Task};

    pub fn keys() -> Keys {
        Keys::default()
    }
    pub fn toggle_native() -> bool {
        false
    }
    pub fn net() -> &'static str {
        "other"
    }
    pub fn file_name(path: &str) -> String {
        path.rsplit('/').next().unwrap_or(path).to_string()
    }
    pub fn app_name(path: &str) -> String {
        file_name(path)
    }
    pub fn file_icon(_path: &str) -> isize {
        0
    }
    pub fn destroy_icon(_icon: isize) {}
    pub fn launch(_path: &str) -> bool {
        false
    }

    pub fn list() -> Vec<Task> {
        Vec::new()
    }
    pub fn icon_of(_hwnd: isize) -> isize {
        0
    }
    pub fn front() -> isize {
        0
    }
    pub fn alive(_hwnd: isize) -> bool {
        false
    }
    pub fn press(_hwnd: isize) -> bool {
        false
    }
    pub fn close(_hwnd: isize) -> bool {
        false
    }
}

pub use imp::{alive, app_name, close, destroy_icon, file_icon, front, icon_of, keys, launch, list, net, press, toggle_native};
