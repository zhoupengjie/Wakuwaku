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
// front (its keyboard layout's, and the layout itself: which input method
// it is, inputs.rs), whether its input method types the language's own
// script or plain letters (中 or 英), and Caps Lock.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Keys {
    pub lang: u16,
    pub hkl: isize,
    pub native: Option<bool>,
    pub caps: bool,
}

// The clear taskbar (taskbar.rs fills_display): a window maximized on its
// display; her island at the top of the screen (island.rs look_over_max):
// one on its display; and (look_under) one under it.
pub use imp::{maximized, maximized_on, reaches_into};

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
        fn GetIpForwardTable(table: *mut u32, size: *mut u32, order: i32) -> u32;
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
        fn SHGetPropertyStoreForWindow(hwnd: Hwnd, iid: *const Guid, out: *mut *mut c_void) -> i32;
    }

    #[repr(C)]
    struct Guid(u32, u16, u16, [u8; 8]);

    #[repr(C)]
    struct PropertyKey {
        fmtid: Guid,
        pid: u32,
    }

    #[repr(C)]
    struct PropVariant {
        vt: u16,
        reserved: [u16; 3],
        value: *mut u16,
        extra: usize,
    }

    // IPropertyStore's table, as far as what is asked of it.
    #[repr(C)]
    struct StoreVtbl {
        _query_interface: usize,
        _add_ref: usize,
        release: extern "system" fn(*mut Store) -> u32,
        _get_count: usize,
        _get_at: usize,
        get_value: extern "system" fn(*mut Store, *const PropertyKey, *mut PropVariant) -> i32,
    }

    #[repr(C)]
    struct Store {
        vtbl: *const StoreVtbl,
    }

    #[link(name = "ole32")]
    extern "system" {
        fn CoInitializeEx(reserved: *mut c_void, flags: u32) -> i32;
        fn PropVariantClear(value: *mut PropVariant) -> i32;
    }

    const IID_PROPERTY_STORE: Guid = Guid(0x886D_8EEB, 0x8CF2, 0x4446, [0x8D, 0x02, 0xCD, 0xBA, 0x1D, 0xBD, 0xCF, 0x99]);
    const PKEY_APP_USER_MODEL_ID: PropertyKey = PropertyKey { fmtid: Guid(0x9F4C_2855, 0x9F79, 0x4B39, [0xA8, 0xD0, 0xE1, 0xD4, 0x2D, 0xE1, 0xD5, 0xF3]), pid: 5 };
    const VT_LPWSTR: u16 = 31;

    thread_local! {
        // COM on the thread that asks the shell (the buttons'), from its first
        // question for as long as it runs.
        static COM: i32 = unsafe { CoInitializeEx(std::ptr::null_mut(), 0x2) };
    }

    pub fn with_com() {
        COM.with(|_| ());
    }

    // The app a window is (its AppUserModelID), what Windows' taskbar takes a
    // button's icon from (taskbar.rs app_png): the one it says (a store app's
    // frame says its app's from the moment it shows), else its program's
    // own, a packaged one's (Windows Terminal says nothing on its windows:
    // pinned by its id, its windows went to a button of their own,
    // 2026-10-11). None for an unpackaged program's window that says
    // nothing: it is known by its file.
    pub fn app_id(hwnd: isize) -> Option<String> {
        window_app_id(hwnd).or_else(|| process_app_id(pid_of(hwnd as Hwnd)))
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetApplicationUserModelId(process: Handle, length: *mut u32, id: *mut u16) -> i32;
    }

    // A packaged program's app, as its package names it.
    pub fn process_app_id(pid: u32) -> Option<String> {
        // SAFETY: a handle we close; the id goes into our own buffer of the
        // length passed.
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process == 0 {
                return None;
            }
            let mut id = [0u16; 256];
            let mut len = id.len() as u32;
            let done = GetApplicationUserModelId(process, &mut len, id.as_mut_ptr());
            CloseHandle(process);
            // Its length counts the closing 0.
            (done == 0 && len > 1 && len as usize <= id.len()).then(|| String::from_utf16_lossy(&id[..len as usize - 1]))
        }
    }

    fn window_app_id(hwnd: isize) -> Option<String> {
        with_com();
        // SAFETY: the window's property store, released; the value read
        // before it is cleared.
        unsafe {
            let mut store: *mut Store = std::ptr::null_mut();
            if SHGetPropertyStoreForWindow(hwnd as Hwnd, &IID_PROPERTY_STORE, &mut store as *mut *mut Store as *mut *mut c_void) < 0 || store.is_null() {
                return None;
            }
            let mut value: PropVariant = std::mem::zeroed();
            let got = ((*(*store).vtbl).get_value)(store, &PKEY_APP_USER_MODEL_ID, &mut value) >= 0;
            let id = (got && value.vt == VT_LPWSTR && !value.value.is_null()).then(|| {
                let n = (0..).take_while(|&i| *value.value.add(i) != 0).count();
                String::from_utf16_lossy(std::slice::from_raw_parts(value.value, n))
            });
            PropVariantClear(&mut value);
            ((*(*store).vtbl).release)(store);
            id.filter(|s| !s.is_empty())
        }
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
        fn IsZoomed(hwnd: Hwnd) -> i32;
        fn IsWindow(hwnd: Hwnd) -> i32;
        fn GetWindow(hwnd: Hwnd, cmd: u32) -> Hwnd;
        fn GetWindowLongPtrW(hwnd: Hwnd, index: i32) -> isize;
        fn GetWindowTextW(hwnd: Hwnd, text: *mut u16, max: i32) -> i32;
        fn InternalGetWindowText(hwnd: Hwnd, text: *mut u16, max: i32) -> i32;
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
        fn GetAsyncKeyState(vk: i32) -> i16;
        fn DestroyIcon(icon: isize) -> i32;
    }

    #[link(name = "dwmapi")]
    extern "system" {
        fn DwmGetWindowAttribute(hwnd: Hwnd, attr: u32, value: *mut c_void, size: u32) -> i32;
        fn DwmSetWindowAttribute(hwnd: Hwnd, attr: u32, value: *const c_void, size: u32) -> i32;
        fn DwmFlush() -> i32;
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
    const WM_SYSCOMMAND: u32 = 0x0112;
    const SC_MINIMIZE: usize = 0xF020;
    const SC_RESTORE: usize = 0xF120;
    const SC_CLOSE: usize = 0xF060;
    const ICON_SMALL: usize = 0;
    const ICON_BIG: usize = 1;
    const ICON_SMALL2: usize = 2;
    const GCLP_HICON: i32 = -14;
    const GCLP_HICONSM: i32 = -34;
    const SMTO_ABORTIFHUNG: u32 = 0x2;
    const SW_MINIMIZE: i32 = 6;
    const SW_SHOWNORMAL: i32 = 1;
    const SHGFI_ICON: u32 = 0x100;
    const SHGFI_LARGEICON: u32 = 0x0;
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

    // Maximized (not minimized: one put away keeps its maximized state).
    pub fn maximized(hwnd: isize) -> bool {
        // SAFETY: plain queries.
        unsafe { IsZoomed(hwnd as Hwnd) != 0 && IsIconic(hwnd as Hwnd) == 0 }
    }

    // Windows' Start menu, open: its window in front. StartMenuExperienceHost's
    // until Windows 11's new Start, which SearchHost holds (26200 on: the
    // other still runs but never comes to the front). There Start and search
    // are one window (typing in Start searches), so search counts as Start.
    pub fn is_start(hwnd: isize) -> bool {
        if hwnd == 0 {
            return false;
        }
        let name = file_name(&path_of(pid_of(hwnd as Hwnd)));
        ["StartMenuExperienceHost.exe", "SearchHost.exe"].iter().any(|n| name.eq_ignore_ascii_case(n))
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

    // A program file's icon, the big one (a button draws it at 24px), ours
    // to destroy (destroy_icon).
    pub fn file_icon(path: &str) -> isize {
        let mut info: ShFileInfo = unsafe { std::mem::zeroed() };
        // SAFETY: our own struct, its size passed.
        unsafe { SHGetFileInfoW(wide(path).as_ptr(), 0, &mut info, std::mem::size_of::<ShFileInfo>() as u32, SHGFI_ICON | SHGFI_LARGEICON) };
        info.icon
    }

    // The icon Windows gives a program file with none of its own (any .exe,
    // by its kind alone), ours to destroy: a program with only that one has
    // its icon on its windows.
    pub fn plain_program_icon() -> isize {
        const SHGFI_USEFILEATTRIBUTES: u32 = 0x10;
        const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;
        let mut info: ShFileInfo = unsafe { std::mem::zeroed() };
        // SAFETY: our own struct, its size passed; no file is opened.
        unsafe { SHGetFileInfoW(wide("program.exe").as_ptr(), FILE_ATTRIBUTE_NORMAL, &mut info, std::mem::size_of::<ShFileInfo>() as u32, SHGFI_ICON | SHGFI_LARGEICON | SHGFI_USEFILEATTRIBUTES) };
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
        start(path, "open", "") > 32
    }

    // The same, with what follows it on its command line.
    pub fn launch_with(path: &str, params: &str) -> bool {
        start(path, "open", params) > 32
    }

    // Started as administrator, Windows asking first, as its "Run as
    // administrator" starts it: ShellExecute's answer, above 32 once it has
    // started (Windows' asking turned down, or a thing that cannot run so,
    // such as most apps from the Store, gives less).
    pub fn launch_as_admin(path: &str) -> isize {
        start(path, "runas", "")
    }

    // The same, with what follows it on its command line (the sensor
    // helper's elevated setup and removal, sensors.rs).
    pub fn launch_as_admin_with(path: &str, params: &str) -> isize {
        start(path, "runas", params)
    }

    fn start(path: &str, verb: &str, params: &str) -> isize {
        let dir = path.rsplit_once('\\').map(|(d, _)| d.to_string()).unwrap_or_default();
        let params = (!params.is_empty()).then(|| wide(params));
        // SAFETY: strings of our own, null-terminated.
        unsafe { ShellExecuteW(std::ptr::null_mut(), wide(verb).as_ptr(), wide(path).as_ptr(), params.as_ref().map_or(std::ptr::null(), |p| p.as_ptr()), wide(&dir).as_ptr(), SW_SHOWNORMAL) }
    }

    // Cloaked: on another virtual desktop, or a store app's window not running.
    fn is_cloaked(hwnd: Hwnd) -> bool {
        let mut cloaked = 0u32;
        // SAFETY: our own u32, of the size passed.
        let got = unsafe { DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, &mut cloaked as *mut u32 as *mut c_void, 4) };
        got == 0 && cloaked != 0
    }

    // Windows' own rules for a button. A window with no title has one too
    // (DSH Desktop's has none): the page shows its program's name instead.
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
        !is_cloaked(hwnd) && !NOT_TASKS.contains(&class_of(hwnd).as_str())
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

    // A window with a button maximized on this display (x, y, w, h): its
    // middle there.
    pub fn maximized_on(mon: (i32, i32, i32, i32)) -> bool {
        let mut all: Vec<isize> = Vec::new();
        // SAFETY: collect() only pushes, while all lives.
        unsafe { EnumWindows(collect, &mut all as *mut Vec<isize> as isize) };
        let me = std::process::id();
        all.into_iter().any(|h| {
            let hwnd = h as Hwnd;
            if !maximized(h) || !is_task(hwnd, me) {
                return false;
            }
            let r = rect_of(hwnd);
            let (x, y) = ((r.left + r.right) / 2, (r.top + r.bottom) / 2);
            x >= mon.0 && x < mon.0 + mon.2 && y >= mon.1 && y < mon.1 + mon.3
        })
    }

    // A window with a button, not minimized, reaching into this rect
    // (physical x, y, w, h) where it is seen: DWM's frame, without the
    // borders Windows draws round it for sizing that cannot be seen (some
    // 7 px a side), else its rect. Her island at the top of the screen
    // goes up out of its way (island.rs look_under). The cheap questions
    // first: each of a few hundred windows is asked, every 1.5 s. Front to
    // back, so the desktop's window lifted over them ("show the desktop")
    // is met first: what lies under it is not seen.
    pub fn reaches_into(r: (i32, i32, i32, i32)) -> bool {
        let mut all: Vec<isize> = Vec::new();
        // SAFETY: collect() only pushes, while all lives.
        unsafe { EnumWindows(collect, &mut all as *mut Vec<isize> as isize) };
        let me = std::process::id();
        for h in all {
            let hwnd = h as Hwnd;
            // SAFETY: plain queries about a window being enumerated.
            if unsafe { IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 } {
                continue;
            }
            let f = frame_of(hwnd);
            if !(f.left < r.0 + r.2 && r.0 < f.right && f.top < r.1 + r.3 && r.1 < f.bottom) {
                continue;
            }
            if is_desktop(h) {
                return false;
            }
            if is_task(hwnd, me) {
                return true;
            }
        }
        false
    }

    const DWMWA_EXTENDED_FRAME_BOUNDS: u32 = 9;

    fn frame_of(hwnd: Hwnd) -> WinRect {
        let mut f = WinRect::default();
        // SAFETY: our own struct, of the size passed.
        let got = unsafe { DwmGetWindowAttribute(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS, &mut f as *mut WinRect as *mut c_void, std::mem::size_of::<WinRect>() as u32) };
        if got == 0 && f.right > f.left && f.bottom > f.top {
            f
        } else {
            rect_of(hwnd)
        }
    }

    // Its icon (the window's own, else its class's), the big one first: a
    // button draws it at 24px, and a small one blurs grown to that. The
    // program's handle, not ours: drawn at once, never kept.
    pub fn icon_of(hwnd: isize) -> isize {
        let h = hwnd as Hwnd;
        let asked = |which: usize| {
            let mut icon = 0usize;
            // SAFETY: a window's own answer, waited for at most 50 ms (a hung one gives none).
            let ok = unsafe { SendMessageTimeoutW(h, WM_GETICON, which, 0, SMTO_ABORTIFHUNG, 50, &mut icon) };
            if ok != 0 { icon as isize } else { 0 }
        };
        // SAFETY: plain queries.
        let class = |which: i32| unsafe { GetClassLongPtrW(h, which) as isize };
        [asked(ICON_BIG), class(GCLP_HICON), asked(ICON_SMALL2), asked(ICON_SMALL), class(GCLP_HICONSM)].into_iter().find(|&i| i != 0).unwrap_or(0)
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
                // Asked to minimize itself, as Windows' taskbar asks (a program
                // drawing its own frame minimizes its own way, or to its tray
                // icon); forced only once it has had its time (jump::bring).
                PostMessageW(h, WM_SYSCOMMAND, SC_MINIMIZE, 0);
                let asked = std::time::Instant::now();
                let up = || IsIconic(h) == 0 && IsWindowVisible(h) != 0;
                while up() && asked.elapsed() < std::time::Duration::from_millis(1000) {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                if up() {
                    ShowWindow(h, SW_MINIMIZE);
                }
                return true;
            }
        }
        crate::jump::bring_window(hwnd)
    }

    #[link(name = "user32")]
    extern "system" {
        fn SetWindowPos(hwnd: Hwnd, after: isize, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
    }

    const SWP_NOSIZE: u32 = 0x1;
    const SWP_NOMOVE: u32 = 0x2;
    const SWP_NOACTIVATE: u32 = 0x10;
    const SWP_NOOWNERZORDER: u32 = 0x200;
    const GW_HWNDPREV: u32 = 3;

    extern "system" fn stack(hwnd: Hwnd, param: isize) -> i32 {
        // SAFETY: param is the Vec in_stack() passed, alive for the enumeration.
        unsafe { (*(param as *mut Vec<isize>)).push(hwnd as isize) };
        1
    }

    // These windows as they lie, the one on top first.
    fn in_stack(hwnds: &[isize]) -> Vec<isize> {
        let mut all = Vec::new();
        // SAFETY: the callback only pushes onto the Vec, which outlives the call.
        unsafe { EnumWindows(stack, &mut all as *mut Vec<isize> as isize) };
        all.into_iter().filter(|h| hwnds.contains(h)).collect()
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetWindowRect(hwnd: Hwnd, rect: *mut WinRect) -> i32;
    }

    #[repr(C)]
    #[derive(Default)]
    struct WinRect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    const HWND_TOPMOST: isize = -1;
    const WS_EX_TRANSPARENT: isize = 0x20;

    fn rect_of(hwnd: Hwnd) -> WinRect {
        let mut r = WinRect::default();
        // SAFETY: our own struct, of the size the call writes.
        unsafe { GetWindowRect(hwnd, &mut r) };
        r
    }

    // Another window over this one where it is, in sight (not one of hers,
    // nor one the pointer passes through): another program's own "on top"
    // window that came over it, say.
    pub fn covered(hwnd: isize, own: &[isize]) -> bool {
        cover(hwnd, own).is_some()
    }

    // The first such window over it: its class, and whether it is kept on
    // top itself; and whether this one is (its WS_EX_TOPMOST), to log.
    pub fn cover(hwnd: isize, own: &[isize]) -> Option<(String, bool)> {
        let me = rect_of(hwnd as Hwnd);
        let mut at = hwnd as Hwnd;
        for _ in 0..2000 {
            // SAFETY: plain queries about windows above this one.
            unsafe {
                at = GetWindow(at, GW_HWNDPREV);
                if at.is_null() {
                    return None;
                }
                if own.contains(&(at as isize)) || IsWindowVisible(at) == 0 || GetWindowLongPtrW(at, GWL_EXSTYLE) & WS_EX_TRANSPARENT != 0 || is_cloaked(at) {
                    continue;
                }
            }
            let r = rect_of(at);
            if r.right > r.left && r.bottom > r.top && r.left < me.right && me.left < r.right && r.top < me.bottom && me.top < r.bottom {
                return Some((class_of(at), is_topmost(at as isize)));
            }
        }
        None
    }

    pub fn is_topmost(hwnd: isize) -> bool {
        const WS_EX_TOPMOST: isize = 0x8;
        // SAFETY: a plain query; a stale handle reads 0.
        unsafe { GetWindowLongPtrW(hwnd as Hwnd, GWL_EXSTYLE) & WS_EX_TOPMOST != 0 }
    }

    // To the top of the windows kept on top, the keyboard where it is.
    pub fn raise_top(hwnd: isize) {
        // SAFETY: a window handle and flags; nothing is pointed to.
        unsafe { SetWindowPos(hwnd as Hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
    }

    // The desktop's window (Progman, or a WorkerW behind its icons).
    pub fn is_desktop(hwnd: isize) -> bool {
        matches!(class_of(hwnd as Hwnd).as_str(), "Progman" | "WorkerW")
    }

    // The desktop's window lying over this one: lifted by "show the desktop".
    pub fn under_desktop(hwnd: isize) -> bool {
        let mut at = hwnd as Hwnd;
        for _ in 0..2000 {
            // SAFETY: plain queries about the windows above this one.
            unsafe {
                at = GetWindow(at, GW_HWNDPREV);
                if at.is_null() {
                    return false;
                }
                if IsWindowVisible(at) != 0 && is_desktop(at as isize) {
                    return true;
                }
            }
        }
        false
    }

    // Her windows kept on top and in sight raised to the top again, each
    // where it lay among them (the lowest first), the keyboard where it is;
    // not waiting on the thread that has them (SWP_ASYNCWINDOWPOS).
    pub fn raise_own(own: &[isize]) {
        const SWP_ASYNCWINDOWPOS: u32 = 0x4000;
        const WS_EX_TOPMOST: isize = 0x8;
        for h in in_stack(own).into_iter().rev() {
            // SAFETY: plain queries and a move in the z-order, by handle; a
            // window gone since only makes them fail.
            unsafe {
                if IsWindowVisible(h as Hwnd) == 0 || GetWindowLongPtrW(h as Hwnd, GWL_EXSTYLE) & WS_EX_TOPMOST == 0 {
                    continue;
                }
                SetWindowPos(h as Hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS);
            }
        }
    }

    // Shown and hidden at once, without the system's own fade and growing
    // from the middle (DWMWA_TRANSITIONS_FORCEDISABLED), as Windows' own
    // taskbar is: shown for the handover it grew from a smaller, see-through
    // copy of itself for a fifth of a second, over a screen's foot already bare.
    // Until DWM has composed the screen anew (what was drawn by then is on it).
    pub fn dwm_flush() {
        // SAFETY: no arguments; it waits for DWM's next pass.
        unsafe { DwmFlush() };
    }

    pub fn no_transitions(hwnd: isize) {
        const DWMWA_TRANSITIONS_FORCEDISABLED: u32 = 3;
        let on: i32 = 1;
        // SAFETY: our own BOOL, its size given.
        unsafe { DwmSetWindowAttribute(hwnd as Hwnd, DWMWA_TRANSITIONS_FORCEDISABLED, &on as *const i32 as *const c_void, 4) };
    }

    // A press on the button of a program with several windows: all of them
    // to the front (restored if minimized), the one on top of them on top
    // still; all minimized when one of them is the one in front, as a
    // single window's button does.
    pub fn press_all(hwnds: &[isize], may_minimize: bool) -> bool {
        let windows = in_stack(hwnds);
        let Some(&top) = windows.first() else { return false };
        // SAFETY: plain calls and posted messages about windows; a stale
        // handle only makes them fail.
        unsafe {
            if may_minimize && windows.contains(&(GetForegroundWindow() as isize)) {
                return minimize_all(&windows);
            }
            // The minimized ones asked to come back all at once (jump::bring),
            // then each to the front from the lowest, the top one last.
            for &h in &windows {
                if IsIconic(h as Hwnd) != 0 {
                    PostMessageW(h as Hwnd, WM_SYSCOMMAND, SC_RESTORE, 0);
                }
            }
            wait_until(&|| !windows.iter().any(|&h| IsIconic(h as Hwnd) != 0), 1500);
        }
        // The top one to the front, as a single window's button brings it;
        // the others laid right under it, as they lay. Each brought to the
        // front in turn, only the first came: the foreground then the
        // program's, ours may not hand it on (2026-10-10). Where a window
        // lies needs no foreground.
        // Laid once the top one is in front and its program has settled what
        // coming to the front does (it may lay its own windows then): laid
        // too soon, the others stayed behind (2026-10-10). Looked at after,
        // and laid again once should they not lie there.
        let brought = crate::jump::bring_window(top);
        // SAFETY: plain queries.
        let front = || unsafe { GetForegroundWindow() } as isize;
        let asked = std::time::Instant::now();
        while front() != top && asked.elapsed() < std::time::Duration::from_millis(400) {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        std::thread::sleep(std::time::Duration::from_millis(80));
        let others: Vec<isize> = windows.iter().copied().filter(|&h| h != top).collect();
        let lay = || {
            let mut under = top;
            others
                .iter()
                .map(|&h| {
                    // SAFETY: a plain move in the stack of windows; a stale handle fails.
                    let ok = unsafe { SetWindowPos(h as Hwnd, under, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER) } != 0;
                    under = h;
                    ok
                })
                .collect::<Vec<bool>>()
        };
        // Each right under the one before it (the top one first).
        let laid = || {
            let mut above = top;
            others.iter().all(|&h| {
                // SAFETY: a plain query.
                let ok = unsafe { GetWindow(h as Hwnd, GW_HWNDPREV) } as isize == above;
                above = h;
                ok
            })
        };
        let first = lay();
        let mut again = None;
        if !laid() {
            std::thread::sleep(std::time::Duration::from_millis(150));
            again = Some(lay());
        }
        let note = format!(
            "{} windows: top brought {brought}, in front {}; laid {first:?}{}; lie as asked {}",
            windows.len(),
            front() == top,
            again.map_or(String::new(), |a| format!(", again {a:?}")),
            laid()
        );
        crate::taskbar::note(&note);
        brought
    }

    // Asked to close, as Windows' own taskbar asks (its window menu's Close):
    // SC_CLOSE, which reaches a window run as administrator (Task Manager)
    // too, where a WM_CLOSE from here was turned away (UIPI, access denied,
    // 2026-10-12); the window then closes as from its own ✕. WM_CLOSE only if
    // SC_CLOSE could not be posted.
    pub fn close(hwnd: isize) -> bool {
        // SAFETY: posted messages, no pointers.
        unsafe { PostMessageW(hwnd as Hwnd, WM_SYSCOMMAND, SC_CLOSE, 0) != 0 || PostMessageW(hwnd as Hwnd, WM_CLOSE, 0, 0) != 0 }
    }

    // Until done, or this long has gone (looked at every 20 ms).
    fn wait_until(done: &dyn Fn() -> bool, ms: u64) {
        let asked = std::time::Instant::now();
        while !done() && asked.elapsed() < std::time::Duration::from_millis(ms) {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    // These windows minimized, as press() minimizes one: each asked, forced
    // once it has had its time.
    pub fn minimize_all(hwnds: &[isize]) -> bool {
        // SAFETY: plain queries about windows by handle; a stale one only fails them.
        let up = |h: isize| unsafe { IsIconic(h as Hwnd) == 0 && IsWindowVisible(h as Hwnd) != 0 };
        for &h in hwnds.iter().filter(|&&h| up(h)) {
            // SAFETY: a posted message, no pointers.
            unsafe { PostMessageW(h as Hwnd, WM_SYSCOMMAND, SC_MINIMIZE, 0) };
        }
        wait_until(&|| !hwnds.iter().any(|&h| up(h)), 1000);
        for &h in hwnds.iter().filter(|&&h| up(h)) {
            // SAFETY: as above.
            unsafe { ShowWindow(h as Hwnd, SW_MINIMIZE) };
        }
        true
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetSystemMenu(hwnd: Hwnd, revert: i32) -> isize;
        fn EnableMenuItem(menu: isize, id: u32, flags: u32) -> i32;
        fn SetMenuDefaultItem(menu: isize, item: u32, by_position: u32) -> i32;
        fn CreatePopupMenu() -> isize;
        fn AppendMenuW(menu: isize, flags: u32, id: usize, text: *const u16) -> i32;
        fn DestroyMenu(menu: isize) -> i32;
        fn TrackPopupMenuEx(menu: isize, flags: u32, x: i32, y: i32, hwnd: Hwnd, params: *const c_void) -> i32;
        fn CreateWindowExW(ex: u32, class: *const u16, title: *const u16, style: u32, x: i32, y: i32, w: i32, h: i32, parent: Hwnd, menu: isize, instance: isize, param: *mut c_void) -> Hwnd;
        fn DestroyWindow(hwnd: Hwnd) -> i32;
        fn SetForegroundWindow(hwnd: Hwnd) -> i32;
        fn GetCursorPos(at: *mut MenuAt) -> i32;
    }

    #[repr(C)]
    #[derive(Default)]
    struct MenuAt {
        x: i32,
        y: i32,
    }

    const GWL_STYLE: i32 = -16;
    const GCL_STYLE: i32 = -26;
    const WS_POPUP: u32 = 0x8000_0000;
    const WS_MINIMIZE: isize = 0x2000_0000;
    const WS_MAXIMIZE: isize = 0x0100_0000;
    const WS_THICKFRAME: isize = 0x0004_0000;
    const WS_MINIMIZEBOX: isize = 0x0002_0000;
    const WS_MAXIMIZEBOX: isize = 0x0001_0000;
    const CS_NOCLOSE: usize = 0x200;
    const SC_SIZE: usize = 0xF000;
    const SC_MOVE: usize = 0xF010;
    const SC_MAXIMIZE: usize = 0xF030;
    const MF_BYCOMMAND: u32 = 0x0;
    const MF_ENABLED: u32 = 0x0;
    const MF_GRAYED: u32 = 0x1;
    const MF_STRING: u32 = 0x0;
    const MF_SEPARATOR: u32 = 0x800;
    const TPM_RIGHTBUTTON: u32 = 0x2;
    const TPM_NONOTIFY: u32 = 0x80;
    const TPM_RETURNCMD: u32 = 0x100;
    const WM_NULL: u32 = 0x0;
    // The menu for several windows: what each of its items is.
    const ALL_RESTORE: usize = 1;
    const ALL_MINIMIZE: usize = 2;
    const ALL_CLOSE: usize = 3;

    // A window's own menu made ready for its state as the window makes it
    // ready itself (Windows' DefWindowProc, as the menu opens from its title
    // bar or Alt+Space; shown from here, the window is not asked): by its
    // style and its class's, which items are enabled (the rest greyed), and
    // the one in bold. Size with a sizing frame and neither minimized nor
    // maximized; Move but maximized; Minimize and Maximize where it has the
    // box and is not so already; Restore once minimized or maximized; Close
    // as the window keeps it, but greyed for a class that has none.
    fn menu_states(style: isize, class_style: usize) -> (Vec<(usize, bool)>, usize) {
        let (min, max) = (style & WS_MINIMIZE != 0, style & WS_MAXIMIZE != 0);
        let mut items = vec![
            (SC_SIZE, style & WS_THICKFRAME != 0 && !min && !max),
            (SC_MOVE, !max),
            (SC_MINIMIZE, style & WS_MINIMIZEBOX != 0 && !min),
            (SC_MAXIMIZE, style & WS_MAXIMIZEBOX != 0 && !max),
            (SC_RESTORE, min || max),
        ];
        if class_style & CS_NOCLOSE != 0 {
            items.push((SC_CLOSE, false));
        }
        (items, if min { SC_RESTORE } else { SC_CLOSE })
    }

    // Shift and a right press on a program's button, as on Windows' own
    // taskbar: one window's own menu (Restore, Move, Size, Minimize,
    // Maximize, Close, and what its program puts there), made ready for its
    // state (menu_states), at the pointer; what is picked sent to the window
    // as from that menu (window_picked). For several windows, a menu for
    // them all in these words (restore them all, minimize them all, close
    // them all), as Windows' has for a group. The menu holds the caller's
    // thread while it is open; it hangs from a window of ours made for it,
    // put in front for it (a menu not in front never closes at a press
    // elsewhere). None when there was none to show; else whether ours came
    // to the front, and what was picked (0: nothing).
    pub fn window_menu(hwnds: &[isize], words: [&str; 3]) -> Option<(bool, usize)> {
        let hwnds: Vec<isize> = hwnds.iter().copied().filter(|&h| alive(h)).collect();
        let &first = hwnds.first()?;
        let one = hwnds.len() == 1;
        // SAFETY: a menu and a window of our own, or the window's own menu by
        // its handle, used and let go of on this thread; strings of our own,
        // null-terminated, copied by the calls; our own POINT.
        unsafe {
            let menu = if one { GetSystemMenu(first as Hwnd, 0) } else { CreatePopupMenu() };
            if menu == 0 {
                return None;
            }
            let grey = |on: bool| if on { MF_ENABLED } else { MF_GRAYED };
            if one {
                let (items, bold) = menu_states(GetWindowLongPtrW(first as Hwnd, GWL_STYLE), GetClassLongPtrW(first as Hwnd, GCL_STYLE));
                for (id, on) in items {
                    EnableMenuItem(menu, id as u32, MF_BYCOMMAND | grey(on));
                }
                SetMenuDefaultItem(menu, bold as u32, 0);
            } else {
                let down = hwnds.iter().filter(|&&h| IsIconic(h as Hwnd) != 0).count();
                AppendMenuW(menu, MF_STRING | grey(down > 0), ALL_RESTORE, wide(words[0]).as_ptr());
                AppendMenuW(menu, MF_STRING | grey(down < hwnds.len()), ALL_MINIMIZE, wide(words[1]).as_ptr());
                AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
                AppendMenuW(menu, MF_STRING, ALL_CLOSE, wide(words[2]).as_ptr());
            }
            let mut at = MenuAt::default();
            GetCursorPos(&mut at);
            let owner = CreateWindowExW(WS_EX_TOOLWINDOW as u32, wide("STATIC").as_ptr(), std::ptr::null(), WS_POPUP, at.x, at.y, 0, 0, std::ptr::null_mut(), 0, 0, std::ptr::null_mut());
            if owner.is_null() {
                if !one {
                    DestroyMenu(menu);
                }
                return None;
            }
            let front = SetForegroundWindow(owner) != 0;
            let picked = TrackPopupMenuEx(menu, TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON, at.x, at.y, owner, std::ptr::null()).max(0) as usize;
            // As Windows asks after a menu shown so: its window's queue moves on.
            PostMessageW(owner, WM_NULL, 0, 0);
            if !one {
                DestroyMenu(menu);
            }
            // Done while the front is still ours to hand on.
            match (one, picked) {
                (_, 0) => {}
                (true, cmd) => window_picked(first, cmd),
                // As a press on the button did (press_all): the minimized
                // ones back, and all of them to the front as they lay.
                (false, ALL_RESTORE) => {
                    press_all(&hwnds, false);
                }
                (false, ALL_MINIMIZE) => {
                    minimize_all(&hwnds);
                }
                (false, _) => hwnds.iter().for_each(|&h| {
                    close(h);
                }),
            }
            DestroyWindow(owner);
            Some((front, picked))
        }
    }

    // What was picked in a window's own menu, sent to it as from that menu
    // (the pointer's place not given: Move and Size then go by the arrow
    // keys, as when picked with the keys). To the front first, but to go
    // down or away: Move and Size need its keys, Restore and Maximize are
    // to show it, and what its program put there may open something. One
    // minimized is restored by coming to the front, to as it was, maximized
    // or not: a Restore sent after it would undo the maximized.
    fn window_picked(hwnd: isize, cmd: usize) {
        let h = hwnd as Hwnd;
        // SAFETY: a plain query.
        let down = unsafe { IsIconic(h) } != 0;
        if cmd != SC_MINIMIZE && cmd != SC_CLOSE {
            crate::jump::bring_window(hwnd);
            if cmd == SC_RESTORE && down {
                return;
            }
        }
        // SAFETY: a posted message, no pointers.
        unsafe { PostMessageW(h, WM_SYSCOMMAND, cmd, 0) };
    }

    #[cfg(test)]
    mod menu_tests {
        use super::*;

        const WINDOW: isize = 0x00CF_0000; // WS_OVERLAPPEDWINDOW: caption, sizing frame, both boxes
        const DIALOG: isize = 0x00C8_0000; // a caption and a system menu, no frame or boxes

        fn on(style: isize, class_style: usize) -> Vec<usize> {
            menu_states(style, class_style).0.into_iter().filter(|i| i.1).map(|i| i.0).collect()
        }

        #[test]
        fn a_window_menu_as_the_window_readies_it() {
            assert_eq!(on(WINDOW, 0), [SC_SIZE, SC_MOVE, SC_MINIMIZE, SC_MAXIMIZE]);
            assert_eq!(menu_states(WINDOW, 0).1, SC_CLOSE);
            // Maximized: Restore, Minimize; neither moved nor sized.
            assert_eq!(on(WINDOW | WS_MAXIMIZE, 0), [SC_MINIMIZE, SC_RESTORE]);
            // Minimized: Restore (in bold), Move and Maximize.
            assert_eq!(on(WINDOW | WS_MINIMIZE, 0), [SC_MOVE, SC_MAXIMIZE, SC_RESTORE]);
            assert_eq!(menu_states(WINDOW | WS_MINIMIZE, 0).1, SC_RESTORE);
            // A dialog: Move alone; Close as it keeps it, greyed for a class without one.
            assert_eq!(on(DIALOG, 0), [SC_MOVE]);
            assert!(!menu_states(DIALOG, 0).0.iter().any(|i| i.0 == SC_CLOSE));
            assert!(menu_states(DIALOG, CS_NOCLOSE).0.contains(&(SC_CLOSE, false)));
        }
    }

    #[link(name = "user32")]
    extern "system" {
        fn EndTask(hwnd: Hwnd, shut_down: i32, force: i32) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn IsProcessCritical(process: Handle, critical: *mut i32) -> i32;
    }

    // A process Windows cannot run without (it stops with it).
    fn is_critical(pid: u32) -> bool {
        // SAFETY: a handle we close; the answer into our own i32.
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process == 0 {
                return false;
            }
            let mut critical = 0;
            let asked = IsProcessCritical(process, &mut critical);
            CloseHandle(process);
            asked != 0 && critical != 0
        }
    }

    // Ended, as Windows' own taskbar ends a program with End task (its
    // setting on: taskbar.rs end_task_on): user32's EndTask, forced, on the
    // window (Taskbar.dll, read 2026-10-14). Its own process goes at once,
    // unsaved work and all; what it started keeps running (tried: a window
    // that turned its close down went in 3 ms, the program it had started
    // stayed). A store app's frame (ApplicationFrameHost, which every such
    // app's frame shares) is not ended: the app's own window in it is.
    // Never File Explorer's (explorer.exe is also the desktop and Windows'
    // own taskbar; Windows offers no End task for it), hers, or a process
    // Windows cannot run without.
    pub fn end_task(hwnd: isize) -> bool {
        let mut h = hwnd as Hwnd;
        if file_name(&path_of(pid_of(h))).eq_ignore_ascii_case("ApplicationFrameHost.exe") {
            // SAFETY: a class name of our own; looking among the frame's children.
            h = unsafe { FindWindowExW(h, std::ptr::null_mut(), wide("Windows.UI.Core.CoreWindow").as_ptr(), std::ptr::null()) };
            if h.is_null() {
                return false;
            }
        }
        let pid = pid_of(h);
        if pid == 0 || pid == std::process::id() || file_name(&path_of(pid)).eq_ignore_ascii_case("explorer.exe") || is_critical(pid) {
            return false;
        }
        // SAFETY: a window's handle; a stale one only makes it fail.
        unsafe { EndTask(h, 0, 1) != 0 }
    }

    const WM_IME_CONTROL: u32 = 0x0283;
    const IMC_GETCONVERSIONMODE: usize = 0x1;
    const IMC_SETCONVERSIONMODE: usize = 0x2;
    const IMC_GETOPENSTATUS: usize = 0x5;
    const IME_CMODE_NATIVE: usize = 0x1;
    const VK_CAPITAL: i32 = 0x14;
    const VK_SHIFT: i32 = 0x10;

    // An input method's answer about the window in front (its default IME
    // window, as input-method indicators ask): None if it gives none.
    fn ime_ask(ime: Hwnd, what: usize, value: isize) -> Option<usize> {
        let mut answer = 0usize;
        // SAFETY: the IME window's answer, waited for at most 50 ms.
        let ok = unsafe { SendMessageTimeoutW(ime, WM_IME_CONTROL, what, value, SMTO_ABORTIFHUNG, 50, &mut answer) };
        (ok != 0).then_some(answer)
    }

    fn front_ime() -> (isize, Hwnd) {
        // SAFETY: plain queries about the window in front.
        unsafe {
            let front = GetForegroundWindow();
            let thread = GetWindowThreadProcessId(front, std::ptr::null_mut());
            (GetKeyboardLayout(thread), ImmGetDefaultIMEWnd(front))
        }
    }

    // The layout, its language and Caps Lock alone: answered within this
    // process, so asked many times a second (taskbar.rs keys_loop).
    pub fn keys_cheap() -> Keys {
        let (hkl, _) = front_ime();
        // SAFETY: a plain query; the low bit is the toggle's.
        let caps = unsafe { GetKeyState(VK_CAPITAL) } & 1 != 0;
        Keys { lang: (hkl & 0xffff) as u16, hkl, native: None, caps }
    }

    // A window's title now (taskbar.rs on_shell: a title said to have
    // changed, against the one shown).
    pub fn title(hwnd: isize) -> String {
        let mut text = [0u16; 512];
        // SAFETY: our own buffer, its size passed. The text as the window
        // keeps it, not asked of the window (GetWindowTextW sends WM_GETTEXT
        // to a window of this process, and waits on its thread).
        let n = unsafe { InternalGetWindowText(hwnd as Hwnd, text.as_mut_ptr(), 512) }.max(0) as usize;
        String::from_utf16_lossy(&text[..n])
    }

    // A window of this process (taskbar.rs on_shell: never a button, never asked).
    pub fn is_ours(hwnd: isize) -> bool {
        pid_of(hwnd as Hwnd) == std::process::id()
    }

    // Whether a window would have a button (taskbar.rs on_shell: one just
    // titled, about to have one).
    pub fn is_task_window(hwnd: isize) -> bool {
        is_task(hwnd as Hwnd, std::process::id())
    }

    // Shift held now (taskbar.rs keys_loop: let go, the IME's mark is read again).
    pub fn shift_down() -> bool {
        // SAFETY: a plain query; the high bit is the key's state now.
        unsafe { GetAsyncKeyState(VK_SHIFT) as u16 & 0x8000 != 0 }
    }

    // Shift and Ctrl held now (a press on a program's button, taskbar.rs
    // taskbar_held).
    pub fn keys_held() -> (bool, bool) {
        const VK_CONTROL: i32 = 0x11;
        // SAFETY: a plain query; the high bit is the key's state now.
        let ctrl = unsafe { GetAsyncKeyState(VK_CONTROL) as u16 & 0x8000 != 0 };
        (shift_down(), ctrl)
    }

    // Whether the IME in front writes the language's own script: asked of
    // its window, in another process, so less often.
    pub fn ime_native() -> Option<bool> {
        let (_, ime) = front_ime();
        if ime.is_null() {
            return None;
        }
        let open = ime_ask(ime, IMC_GETOPENSTATUS, 0)?;
        let mode = ime_ask(ime, IMC_GETCONVERSIONMODE, 0)?;
        Some(open != 0 && mode & IME_CMODE_NATIVE != 0)
    }

    // The network's way out, as Windows' taskbar shows it: "wired", "wifi",
    // "other" (a VPN's, a phone's) or "none": the interface Windows would
    // send to a public address by, its type in its LUID's top 16 bits (way_out).
    pub fn net() -> &'static str {
        let mut index = 0u32;
        // SAFETY: our own out-parameter; 1.1.1.1 reads the same in either byte order.
        if unsafe { GetBestInterface(u32::from_ne_bytes([1, 1, 1, 1]), &mut index) } != 0 {
            return "none";
        }
        way_out(if_type(index), under_tunnel)
    }

    // Interface types (ipifcons.h): a cable (Ethernet), Wi-Fi, a phone's
    // mobile broadband (WWANPP and WWANPP2).
    const IF_ETHERNET: u64 = 6;
    const IF_WIFI: u64 = 71;
    const IF_WWAN: [u64; 2] = [243, 244];

    // An interface's type, in its LUID's top 16 bits (6 a cable, 71 Wi-Fi,
    // 53 a virtual one…); None for one gone.
    fn if_type(index: u32) -> Option<u64> {
        let mut luid = 0u64;
        // SAFETY: our own out-parameter.
        (unsafe { ConvertInterfaceIndexToLuid(index, &mut luid) } == 0).then_some(luid >> 48)
    }

    // The way out by the type of the interface Windows would send by: a
    // cable, Wi-Fi; a phone's, its own ("other"). Any other rides on one of
    // those: a proxy's TUN (Clash Meta's, "Meta", of type 53, its default
    // route taken before the Wi-Fi's), a VPN's. Windows' taskbar showed the
    // Wi-Fi under the TUN, and hers "other", no Wi-Fi
    // (zhoupengjie/Wakuwaku#64): so the one under it (under), else "other"
    // as before.
    fn way_out(best: Option<u64>, under: impl FnOnce() -> Option<&'static str>) -> &'static str {
        match best {
            Some(IF_ETHERNET) => "wired",
            Some(IF_WIFI) => "wifi",
            Some(t) if IF_WWAN.contains(&t) => "other",
            _ => under().unwrap_or("other"),
        }
    }

    // MIB_IPFORWARDTABLE in DWORDs: a count, then the rows, 14 DWORDs each
    // (MIB_IPFORWARDROW: destination, mask, policy, next hop, interface
    // index, type, protocol, age, next hop's AS, five metrics).
    const ROW: usize = 14;

    // The IPv4 routing table, as above (an IPv6-only way out stays "other").
    // Its size asked first; a route that comes between the asking and the
    // reading leaves the buffer short again (ERROR_INSUFFICIENT_BUFFER, the
    // size needed given back), so asked again with that, a few times at most.
    fn forward_table() -> Option<Vec<u32>> {
        const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
        let mut words: Vec<u32> = Vec::new();
        for _ in 0..4 {
            let mut size = (words.len() * 4) as u32;
            let at = if words.is_empty() { std::ptr::null_mut() } else { words.as_mut_ptr() };
            // SAFETY: our own buffer (or none, its size 0), its size in bytes
            // given; Windows writes within it, DWORD-aligned as the table is.
            match unsafe { GetIpForwardTable(at, &mut size, 0) } {
                0 => return Some(words),
                ERROR_INSUFFICIENT_BUFFER => words = vec![0; (size as usize).div_ceil(4)],
                _ => return None,
            }
        }
        None
    }

    fn under_tunnel() -> Option<&'static str> {
        under_tunnel_in(&forward_table()?, if_type)
    }

    // The way out under a tunnel, from the routing table (forward_table) and
    // each interface's type: the way Windows would send were the tunnel not
    // there. Of the cables' and Wi-Fi's own default routes (0.0.0.0/0;
    // addresses in network order, but 0 reads the same either way), the one
    // with the lowest metric (its route's and its interface's together, as
    // GetIpForwardTable gives it), Wi-Fi on a tie. So a cable and Wi-Fi both
    // up go as they would with no tunnel: by the metrics, which Windows sets
    // by speed (the cable's mostly the lower). VMware's host-only adapters
    // (VMnet1 and VMnet8, of type 6) have no default route, nor an adapter
    // unplugged, and a VPN's two halves (0.0.0.0/1 and 128.0.0.0/1) are none
    // either: neither taken for a way out. None: no cable nor Wi-Fi has one.
    fn under_tunnel_in(table: &[u32], type_of: impl Fn(u32) -> Option<u64>) -> Option<&'static str> {
        let (&count, rows) = table.split_first()?;
        rows.chunks_exact(ROW)
            .take(count as usize)
            .filter(|r| r[0] == 0 && r[1] == 0)
            .filter_map(|r| type_of(r[4]).filter(|t| *t == IF_ETHERNET || *t == IF_WIFI).map(|t| (r[9], t != IF_WIFI, t)))
            .min()
            .map(|(_, _, t)| if t == IF_WIFI { "wifi" } else { "wired" })
    }

    // The Wi-Fi's signal as Windows' own taskbar shows it: its bars, 0 to 5,
    // as Windows' UI counts them (Windows.Networking.Connectivity's
    // ConnectionProfile.GetSignalBars), of the profile Windows goes out by;
    // under a tunnel (net's way_out: a proxy's TUN, a VPN's), of a Wi-Fi
    // connected under it. Hers was always full (all its arcs) whatever
    // Windows' showed. None: no Wi-Fi with a signal, or not to be asked.
    // Asked of WinRT by hand, as the property store above is of COM, on a
    // thread with COM (taskbar.rs keys_loop's, multithreaded).
    #[link(name = "runtimeobject")]
    extern "system" {
        fn WindowsCreateString(source: *const u16, length: u32, string: *mut *mut c_void) -> i32;
        fn WindowsDeleteString(string: *mut c_void) -> i32;
        fn RoGetActivationFactory(class: *mut c_void, iid: *const Guid, factory: *mut *mut c_void) -> i32;
    }

    const IID_NETWORK_INFORMATION_STATICS: Guid = Guid(0x5074_F851, 0x950D, 0x4165, [0x9C, 0x15, 0x36, 0x56, 0x19, 0x48, 0x1E, 0xEA]);
    const IID_CONNECTION_PROFILE2: Guid = Guid(0xE204_5145, 0x4C9F, 0x400C, [0x91, 0x50, 0x7E, 0xC7, 0xD6, 0xE2, 0x88, 0x8A]);

    // A WinRT object, released when dropped; its methods by their place in
    // its table (after IInspectable's six), as Windows' metadata has them.
    struct Rt(*mut c_void);

    impl Rt {
        // SAFETY (all): the object is live (ours till dropped), and each
        // place is asked with its own signature, as the interface it is
        // (an out-parameter of ours, an HRESULT back).
        fn method<F: Copy>(&self, at: usize) -> F {
            unsafe { std::mem::transmute_copy(&*(*(self.0 as *const *const usize)).add(at)) }
        }
        // An object handed out: ours only if there is one. Made only then
        // (then, not then_some): one made anyway was dropped, released
        // through a null pointer, and the compiler, taking that for never,
        // left out the look for null too; asked for the way out with none
        // there (Wi-Fi gone a moment: S_OK and null, as Windows says),
        // she crashed (zhoupengjie/Wakuwaku#95).
        fn held(hr: i32, out: *mut c_void) -> Option<Rt> {
            (hr >= 0 && !out.is_null()).then(|| Rt(out))
        }
        fn object(&self, at: usize) -> Option<Rt> {
            let mut out = std::ptr::null_mut();
            let hr = self.method::<extern "system" fn(*mut c_void, *mut *mut c_void) -> i32>(at)(self.0, &mut out);
            Rt::held(hr, out)
        }
        fn item(&self, at: usize, index: u32) -> Option<Rt> {
            let mut out = std::ptr::null_mut();
            let hr = self.method::<extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32>(at)(self.0, index, &mut out);
            Rt::held(hr, out)
        }
        fn value<T: Default>(&self, at: usize) -> Option<T> {
            let mut out = T::default();
            (self.method::<extern "system" fn(*mut c_void, *mut T) -> i32>(at)(self.0, &mut out) >= 0).then_some(out)
        }
        fn cast(&self, iid: &Guid) -> Option<Rt> {
            let mut out = std::ptr::null_mut();
            let hr = self.method::<extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> i32>(0)(self.0, iid, &mut out);
            Rt::held(hr, out)
        }
    }

    impl Drop for Rt {
        fn drop(&mut self) {
            self.method::<extern "system" fn(*mut c_void) -> u32>(2)(self.0);
        }
    }

    pub fn wifi_bars() -> Option<u8> {
        let name: Vec<u16> = "Windows.Networking.Connectivity.NetworkInformation".encode_utf16().collect();
        let mut class = std::ptr::null_mut();
        let mut statics = std::ptr::null_mut();
        // SAFETY: the class's name, ours, made into a string and let go of
        // once its factory is had; the factory ours to release (Rt).
        let hr = unsafe {
            if WindowsCreateString(name.as_ptr(), name.len() as u32, &mut class) < 0 {
                return None;
            }
            let hr = RoGetActivationFactory(class, &IID_NETWORK_INFORMATION_STATICS, &mut statics);
            WindowsDeleteString(class);
            hr
        };
        if hr < 0 || statics.is_null() {
            return None;
        }
        let statics = Rt(statics);
        // INetworkInformationStatics: GetConnectionProfiles (6),
        // GetInternetConnectionProfile (7); IVectorView: GetAt (6), get_Size (7).
        if let Some(bars) = statics.object(7).and_then(|way_out| bars_of(&way_out, false)) {
            return Some(bars);
        }
        let all = statics.object(6)?;
        (0..all.value::<u32>(7)?).filter_map(|i| all.item(6, i)).find_map(|p| bars_of(&p, true))
    }

    // A profile's bars, if it is a Wi-Fi's with a signal (one connected at
    // all, if asked: of all the profiles Windows keeps, the one under the
    // tunnel). IConnectionProfile: GetNetworkConnectivityLevel (7, none 0);
    // IConnectionProfile2: get_IsWlanConnectionProfile (7), GetSignalBars
    // (11, an IReference<UINT8>, none for a profile without bars);
    // IReference: get_Value (6).
    fn bars_of(profile: &Rt, connected: bool) -> Option<u8> {
        if connected && profile.value::<i32>(7)? == 0 {
            return None;
        }
        let wlan = profile.cast(&IID_CONNECTION_PROFILE2)?;
        if wlan.value::<u8>(7)? == 0 {
            return None;
        }
        wlan.object(11)?.value::<u8>(6)
    }

    #[cfg(test)]
    mod rt_tests {
        use super::*;
        use std::sync::atomic::{AtomicU32, Ordering};

        // An object of our own, as WinRT's are: QueryInterface (0), Release
        // (2), and at 6, 7 and 8 an object asked for, one by index, one
        // refused. Each hands back nothing: S_OK and null, as
        // GetInternetConnectionProfile does with no way out (#95), or an
        // error. Its releases counted.
        static RELEASED: AtomicU32 = AtomicU32::new(0);
        extern "system" fn no_interface(_: *mut c_void, _: *const Guid, out: *mut *mut c_void) -> i32 {
            // SAFETY: the out-parameter Rt::cast hands in.
            unsafe { *out = std::ptr::null_mut() };
            0x8000_4002_u32 as i32
        }
        extern "system" fn release(_: *mut c_void) -> u32 {
            RELEASED.fetch_add(1, Ordering::SeqCst);
            0
        }
        extern "system" fn nothing(_: *mut c_void, out: *mut *mut c_void) -> i32 {
            // SAFETY: the out-parameter Rt::object hands in.
            unsafe { *out = std::ptr::null_mut() };
            0
        }
        extern "system" fn nothing_at(_: *mut c_void, _: u32, out: *mut *mut c_void) -> i32 {
            // SAFETY: the out-parameter Rt::item hands in.
            unsafe { *out = std::ptr::null_mut() };
            0
        }
        extern "system" fn failing(_: *mut c_void, _: *mut *mut c_void) -> i32 {
            0x8000_4005_u32 as i32
        }

        #[test]
        fn nothing_handed_back_is_none_and_never_released() {
            let table: [usize; 9] = [no_interface as *const () as usize, 0, release as *const () as usize, 0, 0, 0, nothing as *const () as usize, nothing_at as *const () as usize, failing as *const () as usize];
            let object: *const usize = table.as_ptr();
            let rt = Rt(&object as *const *const usize as *mut c_void);
            assert!(rt.object(6).is_none());
            assert!(rt.item(7, 0).is_none());
            assert!(rt.object(8).is_none());
            assert!(rt.cast(&IID_CONNECTION_PROFILE2).is_none());
            assert_eq!(RELEASED.load(Ordering::SeqCst), 0);
            drop(rt);
            assert_eq!(RELEASED.load(Ordering::SeqCst), 1);
        }
    }

    #[cfg(test)]
    mod net_tests {
        use super::*;

        // Interfaces by index: 1 Wi-Fi, 2 a cable, 3 a proxy's TUN (53), 4
        // and 5 VMware's VMnet1 and VMnet8 (6), 6 a VPN's (53), 7 a VPN's
        // TAP adapter (6), 8 a phone's (243); 9 gone.
        fn kind(index: u32) -> Option<u64> {
            match index {
                1 => Some(71),
                2 | 4 | 5 | 7 => Some(6),
                3 | 6 => Some(53),
                8 => Some(243),
                _ => None,
            }
        }

        // A row: destination and mask (a.b.c.d, in network order as
        // Windows has them), interface, metric.
        fn row(dest: [u8; 4], mask: [u8; 4], index: u32, metric: u32) -> Vec<u32> {
            let mut r = vec![0; ROW];
            r[0] = u32::from_ne_bytes(dest);
            r[1] = u32::from_ne_bytes(mask);
            r[4] = index;
            r[9] = metric;
            r
        }

        fn table(rows: &[Vec<u32>]) -> Vec<u32> {
            let mut t = vec![rows.len() as u32];
            rows.iter().for_each(|r| t.extend_from_slice(r));
            t
        }

        const ANY: [u8; 4] = [0, 0, 0, 0];
        const HALF: [u8; 4] = [128, 0, 0, 0];
        const ONE: [u8; 4] = [255, 255, 255, 255];
        const NET24: [u8; 4] = [255, 255, 255, 0];

        // VMware's host-only adapters: their subnets and own addresses alone.
        fn vmware() -> Vec<Vec<u32>> {
            vec![row([192, 168, 56, 0], NET24, 4, 291), row([192, 168, 56, 1], ONE, 4, 291), row([192, 168, 128, 0], NET24, 5, 291), row([192, 168, 128, 1], ONE, 5, 291)]
        }

        fn under(rows: Vec<Vec<u32>>) -> Option<&'static str> {
            under_tunnel_in(&table(&rows), kind)
        }

        #[test]
        fn the_way_out_under_a_tunnel() {
            let tun = || row(ANY, ANY, 3, 1);
            // The reporter's (zhoupengjie/Wakuwaku#64): Clash Meta's TUN and
            // the Wi-Fi each with a default route, VMware's two adapters.
            let wifi = [vec![tun(), row([198, 18, 0, 0], [255, 255, 0, 0], 3, 1), row(ANY, ANY, 1, 35), row([192, 168, 5, 0], NET24, 1, 291)], vmware()].concat();
            assert_eq!(under(wifi), Some("wifi"));
            // A cable under it.
            assert_eq!(under([vec![tun(), row(ANY, ANY, 2, 25)], vmware()].concat()), Some("wired"));
            // Both: the lower metric, as Windows would send; Wi-Fi on a tie.
            assert_eq!(under(vec![tun(), row(ANY, ANY, 1, 35), row(ANY, ANY, 2, 25)]), Some("wired"));
            assert_eq!(under(vec![tun(), row(ANY, ANY, 2, 15), row(ANY, ANY, 1, 10)]), Some("wifi"));
            assert_eq!(under(vec![tun(), row(ANY, ANY, 2, 25), row(ANY, ANY, 1, 25)]), Some("wifi"));
            // VMware's adapters alone (a cable's type, no default route): none.
            assert_eq!(under([vec![tun()], vmware()].concat()), None);
            // A VPN that splits the default route in two over the Wi-Fi's:
            // the halves are no default route, on a TUN nor on a TAP (of a
            // cable's type).
            let halves = |i| vec![row(ANY, HALF, i, 1), row(HALF, HALF, i, 1)];
            assert_eq!(under([halves(6), vec![row(ANY, ANY, 1, 35)], vmware()].concat()), Some("wifi"));
            assert_eq!(under([halves(7), vec![row(ANY, ANY, 1, 35)]].concat()), Some("wifi"));
            assert_eq!(under([halves(7), vmware()].concat()), None);
            // A phone's default route, or one of an interface gone, is no
            // cable nor Wi-Fi.
            assert_eq!(under(vec![tun(), row(ANY, ANY, 8, 5), row(ANY, ANY, 9, 5)]), None);
        }

        #[test]
        fn the_routing_table_read_within_itself() {
            // Nothing, or a count alone.
            assert_eq!(under_tunnel_in(&[], kind), None);
            assert_eq!(under_tunnel_in(&[0], kind), None);
            // Rows past the count are not the table's (the buffer is larger
            // than the table, as Windows sizes it).
            let mut t = table(&[row([10, 0, 0, 0], NET24, 2, 1)]);
            t.extend(row(ANY, ANY, 1, 35));
            assert_eq!(under_tunnel_in(&t, kind), None);
            // A count past the rows there: only whole rows read.
            let mut t = table(&[row(ANY, ANY, 2, 25)]);
            t[0] = 5;
            t.extend([0; ROW - 1]);
            assert_eq!(under_tunnel_in(&t, kind), Some("wired"));
        }

        #[test]
        fn the_way_out_by_the_interface_type() {
            let none = || -> Option<&'static str> { panic!("not looked under") };
            assert_eq!(way_out(Some(6), none), "wired");
            assert_eq!(way_out(Some(71), none), "wifi");
            // A phone's is a way out of its own, not looked under.
            assert_eq!(way_out(Some(243), none), "other");
            assert_eq!(way_out(Some(244), none), "other");
            // A TUN's, a VPN's (PPP 23, a tunnel 131): the one under it, else "other".
            assert_eq!(way_out(Some(53), || Some("wifi")), "wifi");
            assert_eq!(way_out(Some(23), || Some("wired")), "wired");
            assert_eq!(way_out(Some(131), || None), "other");
            // Its type unknown (gone between the two asks): looked under too.
            assert_eq!(way_out(None, || Some("wifi")), "wifi");
        }

        // Asked of Windows as it is here (WinRT, by hand): no Wi-Fi with a
        // signal, or its bars, 0 to 5, and the same asked again; nothing
        // left over to crash on as the objects are let go of.
        #[test]
        fn the_wifi_bars_as_windows_has_them() {
            let _com = crate::audio::Com::new();
            let bars = wifi_bars();
            println!("wifi bars here: {bars:?} (net: {})", net());
            assert!(bars.is_none_or(|b| b <= 5));
            assert_eq!(wifi_bars().is_some(), bars.is_some());
        }
    }

    // What is using the microphone, the camera and the location now, each a
    // list of programs' names, as Windows' taskbar shows it. Windows notes
    // every use under CapabilityAccessManager\ConsentStore\<what>: a key per
    // program (one not from the Store under NonPackaged, its path with # for
    // \) with LastUsedTimeStart and LastUsedTimeStop, 0 while still in use.
    pub fn in_use() -> [(&'static str, Vec<String>); 3] {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;
        let root = RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore");
        let using = |key: &RegKey| key.get_value::<u64, _>("LastUsedTimeStart").unwrap_or(0) > 0 && key.get_value::<u64, _>("LastUsedTimeStop").unwrap_or(1) == 0;
        [("mic", "microphone"), ("cam", "webcam"), ("loc", "location")].map(|(what, store)| {
            let mut names = Vec::new();
            if let Some(store) = root.as_ref().ok().and_then(|r| r.open_subkey(store).ok()) {
                for name in store.enum_keys().flatten() {
                    let Ok(key) = store.open_subkey(&name) else { continue };
                    if name == "NonPackaged" {
                        for path in key.enum_keys().flatten() {
                            if key.open_subkey(&path).is_ok_and(|k| using(&k)) {
                                names.push(app_name(&path.replace('#', "\\")));
                            }
                        }
                    } else if using(&key) {
                        names.push(name.split('_').next().unwrap_or(&name).to_string());
                    }
                }
            }
            names.sort();
            names.dedup();
            (what, names)
        })
    }

    // One of what Windows opens from its taskbar and closes once anything
    // else is pressed (Start, search, the notifications and calendar, the
    // quick settings), by the program it is of.
    pub fn is_shell_flyout(hwnd: isize) -> bool {
        let name = file_name(&path_of(pid_of(hwnd as Hwnd))).to_ascii_lowercase();
        ["startmenuexperiencehost.exe", "searchhost.exe", "shellexperiencehost.exe", "shellhost.exe"].contains(&name.as_str())
    }

    // Windows' search, in front (taskbar.rs open: pressed again, it closes).
    pub fn is_search(hwnd: isize) -> bool {
        file_name(&path_of(pid_of(hwnd as Hwnd))).eq_ignore_ascii_case("searchhost.exe")
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

    pub fn keys_cheap() -> Keys {
        Keys::default()
    }
    pub fn ime_native() -> Option<bool> {
        None
    }
    pub fn shift_down() -> bool {
        false
    }
    pub fn keys_held() -> (bool, bool) {
        (false, false)
    }
    pub fn title(_hwnd: isize) -> String {
        String::new()
    }
    pub fn is_ours(_hwnd: isize) -> bool {
        false
    }
    pub fn is_task_window(_hwnd: isize) -> bool {
        false
    }
    pub fn toggle_native() -> bool {
        false
    }
    pub fn net() -> &'static str {
        "other"
    }
    pub fn wifi_bars() -> Option<u8> {
        None
    }
    pub fn is_shell_flyout(_hwnd: isize) -> bool {
        false
    }
    pub fn is_search(_hwnd: isize) -> bool {
        false
    }
    pub fn covered(_hwnd: isize, _own: &[isize]) -> bool {
        false
    }
    pub fn cover(_hwnd: isize, _own: &[isize]) -> Option<(String, bool)> {
        None
    }
    pub fn is_topmost(_hwnd: isize) -> bool {
        false
    }
    pub fn raise_top(_hwnd: isize) {}
    pub fn is_desktop(_hwnd: isize) -> bool {
        false
    }
    pub fn under_desktop(_hwnd: isize) -> bool {
        false
    }
    pub fn raise_own(_own: &[isize]) {}
    pub fn no_transitions(_hwnd: isize) {}
    pub fn dwm_flush() {}
    pub fn in_use() -> [(&'static str, Vec<String>); 3] {
        [("mic", Vec::new()), ("cam", Vec::new()), ("loc", Vec::new())]
    }
    pub fn maximized(_hwnd: isize) -> bool {
        false
    }
    pub fn maximized_on(_mon: (i32, i32, i32, i32)) -> bool {
        false
    }
    pub fn reaches_into(_r: (i32, i32, i32, i32)) -> bool {
        false
    }
    pub fn is_start(_hwnd: isize) -> bool {
        false
    }
    pub fn file_name(path: &str) -> String {
        path.rsplit('/').next().unwrap_or(path).to_string()
    }
    pub fn app_name(path: &str) -> String {
        file_name(path)
    }
    pub fn plain_program_icon() -> isize {
        0
    }
    pub fn app_id(_hwnd: isize) -> Option<String> {
        None
    }
    pub fn process_app_id(_pid: u32) -> Option<String> {
        None
    }
    pub fn with_com() {}
    pub fn file_icon(_path: &str) -> isize {
        0
    }
    pub fn destroy_icon(_icon: isize) {}
    pub fn launch(_path: &str) -> bool {
        false
    }
    pub fn launch_as_admin(_path: &str) -> isize {
        0
    }
    pub fn launch_as_admin_with(_path: &str, _params: &str) -> isize {
        0
    }
    pub fn window_menu(_hwnds: &[isize], _words: [&str; 3]) -> Option<(bool, usize)> {
        None
    }
    pub fn launch_with(_path: &str, _params: &str) -> bool {
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
    pub fn press_all(_hwnds: &[isize], _may_minimize: bool) -> bool {
        false
    }
    pub fn press(_hwnd: isize) -> bool {
        false
    }
    pub fn close(_hwnd: isize) -> bool {
        false
    }
    pub fn end_task(_hwnd: isize) -> bool {
        false
    }
}

pub use imp::{alive, app_id, app_name, process_app_id, close, cover, covered, is_topmost, destroy_icon, file_icon, front, icon_of, in_use, ime_native, is_ours, is_desktop, is_search, is_shell_flyout, is_start, is_task_window, keys_cheap, keys_held, launch, launch_as_admin, list, net, wifi_bars, dwm_flush, no_transitions, plain_program_icon, press, press_all, raise_own, raise_top, shift_down, title, toggle_native, under_desktop, window_menu, with_com, end_task, launch_with, launch_as_admin_with};

#[cfg(test)]
mod tests {
    // The windows with buttons and the app each is, changing nothing: run
    // by hand (cargo test tasks::tests -- --ignored --nocapture).
    #[test]
    #[ignore]
    fn windows_and_their_apps() {
        for t in super::list() {
            println!("{} | {:?} | {}", t.path, super::app_id(t.hwnd), t.title);
        }
    }
}
