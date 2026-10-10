// The windows the taskbar has buttons for (taskbar.rs): those Windows' own
// taskbar shows, as near as can be told from outside it (shown, not on
// another desktop, not owned by another, not a tool window; or marked to be
// on the taskbar). And what a press on a button does: the window to the
// front, restored if minimized, or minimized if it is the one in front.
// Windows only.

pub struct Task {
    pub hwnd: isize,
    pub title: String,
    pub exe: String,
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

    fn exe_of(pid: u32) -> String {
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
            let path = String::from_utf16_lossy(&name[..size as usize]);
            path.rsplit('\\').next().unwrap_or(&path).to_string()
        }
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
    fn app_exe(hwnd: Hwnd, exe: String) -> String {
        if !exe.eq_ignore_ascii_case("ApplicationFrameHost.exe") {
            return exe;
        }
        // SAFETY: a class name of our own; looking among the frame's children.
        let core = unsafe { FindWindowExW(hwnd, std::ptr::null_mut(), wide("Windows.UI.Core.CoreWindow").as_ptr(), std::ptr::null()) };
        if core.is_null() {
            return exe;
        }
        let inner = exe_of(pid_of(core));
        if inner.is_empty() { exe } else { inner }
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
                Task { hwnd: h, title: title_of(hwnd), exe: app_exe(hwnd, exe_of(pid_of(hwnd))), min }
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

pub use imp::{alive, close, front, icon_of, keys, list, press, toggle_native};
