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

    // The app a window says it is (its AppUserModelID; a store app's frame
    // says its app's from the moment it shows), what Windows' taskbar takes a
    // button's icon from (taskbar.rs app_png). None for a window that says
    // nothing: a program is then known by its file.
    pub fn app_id(hwnd: isize) -> Option<String> {
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
    const WM_SYSCOMMAND: u32 = 0x0112;
    const SC_MINIMIZE: usize = 0xF020;
    const SC_RESTORE: usize = 0xF120;
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
            let up = |h: isize| IsIconic(h as Hwnd) == 0 && IsWindowVisible(h as Hwnd) != 0;
            let wait_for = |done: &dyn Fn() -> bool, ms: u64| {
                let asked = std::time::Instant::now();
                while !done() && asked.elapsed() < std::time::Duration::from_millis(ms) {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
            };
            if may_minimize && windows.contains(&(GetForegroundWindow() as isize)) {
                // As press(): each asked, forced once it has had its time.
                for &h in &windows {
                    if up(h) {
                        PostMessageW(h as Hwnd, WM_SYSCOMMAND, SC_MINIMIZE, 0);
                    }
                }
                wait_for(&|| !windows.iter().any(|&h| up(h)), 1000);
                for &h in windows.iter().filter(|&&h| up(h)) {
                    ShowWindow(h as Hwnd, SW_MINIMIZE);
                }
                return true;
            }
            // The minimized ones asked to come back all at once (jump::bring),
            // then each to the front from the lowest, the top one last.
            for &h in &windows {
                if IsIconic(h as Hwnd) != 0 {
                    PostMessageW(h as Hwnd, WM_SYSCOMMAND, SC_RESTORE, 0);
                }
            }
            wait_for(&|| !windows.iter().any(|&h| IsIconic(h as Hwnd) != 0), 1500);
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
    pub fn is_shell_flyout(_hwnd: isize) -> bool {
        false
    }
    pub fn in_use() -> [(&'static str, Vec<String>); 3] {
        [("mic", Vec::new()), ("cam", Vec::new()), ("loc", Vec::new())]
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
    pub fn with_com() {}
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
    pub fn press_all(_hwnds: &[isize], _may_minimize: bool) -> bool {
        false
    }
    pub fn press(_hwnd: isize) -> bool {
        false
    }
    pub fn close(_hwnd: isize) -> bool {
        false
    }
}

pub use imp::{alive, app_id, app_name, close, destroy_icon, file_icon, front, icon_of, in_use, is_shell_flyout, is_start, keys, launch, list, net, plain_program_icon, press, press_all, toggle_native, with_com};
