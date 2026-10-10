// Other programs' icons in the notification area (the tray), taken over from
// Explorer while hers is the taskbar. A program hands its icon to the first
// window of class Shell_TrayWnd that FindWindow finds (Shell_NotifyIcon, as
// a WM_COPYDATA), so one of ours is put ahead of Explorer's (topmost, and put
// back there when Explorer's moves up). Each message is handed on to
// Explorer's as well, so its tray stays whole for when the taskbar is given
// back; the same for what else comes to the taskbar's window (app bars,
// SHAppBarMessage, come this way too). Programs that added their icons before
// are asked to add them again, with the message Explorer sends when it starts.
//
// A press on an icon is told to its program as Explorer tells it, by the
// version of the protocol it asked for (NIM_SETVERSION). Windows only.
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

// An icon as shown: its key (for tell), its icon (a copy of ours), its tip,
// the program it is from.
pub struct Shown {
    pub key: u64,
    pub icon: isize,
    pub tip: String,
    pub exe: String,
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, AtomicU64, Ordering};
    use std::sync::Mutex;

    use super::{Press, Shown};

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
    // for the left and top, 2 for the right and bottom). The window in 64
    // bits here, whoever asks.
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct RectQuestion {
        signature: u32,
        message: u32,
        size: u32,
        padding: u32,
        hwnd: u64,
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
        fn CopyIcon(icon: isize) -> isize;
        fn DestroyIcon(icon: isize) -> i32;
        fn AllowSetForegroundWindow(pid: u32) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleW(name: *const u16) -> isize;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
        fn CloseHandle(handle: Handle) -> i32;
        fn QueryFullProcessImageNameW(process: Handle, flags: u32, name: *mut u16, size: *mut u32) -> i32;
    }

    const WM_COMMAND: u32 = 0x0111;
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
    const HWND_BROADCAST: isize = 0xffff;
    const SWP_NOSIZE: u32 = 0x1;
    const SWP_NOMOVE: u32 = 0x2;
    const SWP_NOACTIVATE: u32 = 0x10;
    const SMTO_ABORTIFHUNG: u32 = 0x2;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

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
    }

    static OURS: AtomicIsize = AtomicIsize::new(0);
    // Where each shown icon is on the screen (key, left, top, right, bottom), from the host.
    static RECTS: Mutex<Vec<(u64, (i32, i32, i32, i32))>> = Mutex::new(Vec::new());
    static RAW_NOTED: AtomicU32 = AtomicU32::new(0);
    static NOTIFY: AtomicIsize = AtomicIsize::new(0);
    static NOTIFY_MSG: AtomicU32 = AtomicU32::new(0);
    static NEXT_KEY: AtomicU64 = AtomicU64::new(1);
    // Something could not be handed on to Explorer: on stopping, every program is asked again.
    static LOST: AtomicBool = AtomicBool::new(false);
    static ICONS: Mutex<Vec<Icon>> = Mutex::new(Vec::new());
    // What came, for the log: (kind, count).
    static SEEN: Mutex<Vec<(String, u32)>> = Mutex::new(Vec::new());
    static NOTES: Mutex<Vec<String>> = Mutex::new(Vec::new());

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

    fn exe_of(pid: u32) -> String {
        // SAFETY: a handle we close; the name goes into our own buffer.
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process == 0 {
                return format!("pid {pid}");
            }
            let mut name = [0u16; 512];
            let mut size = 512u32;
            let ok = QueryFullProcessImageNameW(process, 0, name.as_mut_ptr(), &mut size);
            CloseHandle(process);
            if ok == 0 {
                return format!("pid {pid}");
            }
            let path = String::from_utf16_lossy(&name[..size as usize]);
            path.rsplit('\\').next().unwrap_or(&path).to_string()
        }
    }

    // Explorer's taskbar window: the first Shell_TrayWnd that is not ours.
    pub fn explorers() -> Hwnd {
        let class = wide("Shell_TrayWnd");
        let me = std::process::id();
        let mut at: Hwnd = std::ptr::null_mut();
        loop {
            // SAFETY: a class name of our own; walking the top-level windows.
            at = unsafe { FindWindowExW(std::ptr::null_mut(), at, class.as_ptr(), std::ptr::null()) };
            if at.is_null() {
                return at;
            }
            let mut pid = 0;
            // SAFETY: our own out-parameter.
            unsafe { GetWindowThreadProcessId(at, &mut pid) };
            if pid != me {
                return at;
            }
        }
    }

    // Whether programs reach ours first.
    pub fn first() -> bool {
        let ours = OURS.load(Ordering::SeqCst);
        // SAFETY: a plain lookup.
        ours != 0 && unsafe { FindWindowW(wide("Shell_TrayWnd").as_ptr(), std::ptr::null()) } as isize == ours
    }

    // Ours put ahead of Explorer's again; true if it had fallen behind.
    pub fn keep_first() -> bool {
        let ours = OURS.load(Ordering::SeqCst);
        if ours == 0 || first() {
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
            let copy = unsafe { CopyIcon(nid.icon as i32 as isize) };
            if copy != 0 {
                if icon.icon != 0 {
                    // SAFETY: our own copy, replaced.
                    unsafe { DestroyIcon(icon.icon) };
                }
                icon.icon = copy;
            }
        }
        if nid.flags & NIF_TIP != 0 {
            icon.tip = text(&nid.tip);
        }
        if nid.flags & NIF_STATE != 0 && nid.state_mask & NIS_HIDDEN != 0 {
            icon.hidden = nid.state & NIS_HIDDEN != 0;
        }
    }

    // A NIM_ message: whether ours took it.
    fn on_icon(message: u32, nid: &IconData) -> bool {
        let mut icons = ICONS.lock().unwrap();
        let at = icons.iter().position(|i| same(i, nid));
        match (message, at) {
            (NIM_ADD | NIM_MODIFY, Some(i)) => {
                let tip = icons[i].tip.clone();
                update(&mut icons[i], nid);
                icons[i].modified += 1;
                if icons[i].tip != tip {
                    note(format!("tray: {} tip \"{}\"", icons[i].exe, icons[i].tip));
                }
                true
            }
            (NIM_ADD, None) => {
                let mut pid = 0;
                // SAFETY: our own out-parameter; a gone window leaves it 0.
                unsafe { GetWindowThreadProcessId(hwnd_of(nid.hwnd), &mut pid) };
                let guid = (nid.flags & NIF_GUID != 0 && nid.guid != [0; 16]).then_some(nid.guid);
                let mut icon = Icon { key: NEXT_KEY.fetch_add(1, Ordering::SeqCst), hwnd: nid.hwnd, id: nid.id, guid, callback: 0, icon: 0, tip: String::new(), version: 0, hidden: false, pid, exe: exe_of(pid), modified: 0 };
                update(&mut icon, nid);
                note(format!("tray: added {} (id {}, {}) \"{}\"{}", icon.exe, icon.id, if guid.is_some() { "by guid" } else { "by window" }, icon.tip, if icon.hidden { ", hidden" } else { "" }));
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
        if cds.ptr.is_null() || n < 28 {
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
        let icon = icons.iter().find(|i| if by_guid { i.guid == Some(q.guid) } else { i.hwnd as u64 == q.hwnd & 0xffff_ffff && i.id == q.id })?;
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
            let answer = hand_on(msg, wparam, lparam);
            if answer.is_none() {
                LOST.store(true, Ordering::SeqCst);
            }
            let header = std::mem::size_of::<TrayData>() - std::mem::size_of::<IconData>();
            if cds.data == 1 && cds.size as usize >= header + 24 && !cds.ptr.is_null() {
                // Read what came, the rest left zero (older programs send less).
                let mut data: TrayData = unsafe { std::mem::zeroed() };
                let n = (cds.size as usize).min(std::mem::size_of::<TrayData>());
                // SAFETY: n bytes of the sender's struct, into ours of at least that size.
                unsafe { std::ptr::copy_nonoverlapping(cds.ptr as *const u8, &mut data as *mut TrayData as *mut u8, n) };
                if data.signature == SIGNATURE {
                    count(match data.message {
                        NIM_ADD => "NIM_ADD",
                        NIM_MODIFY => "NIM_MODIFY",
                        NIM_DELETE => "NIM_DELETE",
                        NIM_SETVERSION => "NIM_SETVERSION",
                        _ => "NIM_other",
                    });
                    if on_icon(data.message, &data.nid) {
                        changed();
                        return 1;
                    }
                }
            } else if cds.data != 3 {
                count(&format!("copydata {}", cds.data));
            }
            return answer.unwrap_or(0) as isize;
        }
        if msg == WM_COMMAND || msg >= WM_USER {
            count(&format!("message {msg:#x}"));
            return hand_on(msg, wparam, lparam).unwrap_or(0) as isize;
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

    // Every program asked to add its icons again (they come to ours, ahead).
    pub fn ask_again() {
        // SAFETY: a broadcast with no pointers.
        unsafe { SendNotifyMessageW(HWND_BROADCAST as Hwnd, RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()), 0, 0) };
    }

    pub fn place(rect: (i32, i32, i32, i32)) {
        let ours = OURS.load(Ordering::SeqCst);
        if ours != 0 {
            // SAFETY: our own window.
            unsafe { SetWindowPos(ours as Hwnd, HWND_TOPMOST, rect.0, rect.1, rect.2, rect.3, SWP_NOACTIVATE) };
        }
    }

    // Ours gone; Explorer's has had everything, unless something could not
    // be handed on: then every program is asked again.
    pub fn stop() {
        let ours = OURS.swap(0, Ordering::SeqCst);
        if ours != 0 {
            // SAFETY: our own window.
            unsafe { DestroyWindow(ours as Hwnd) };
        }
        for icon in ICONS.lock().unwrap().drain(..) {
            if icon.icon != 0 {
                // SAFETY: our own copies.
                unsafe { DestroyIcon(icon.icon) };
            }
        }
        if LOST.load(Ordering::SeqCst) {
            ask_again();
        }
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

    pub fn shown() -> Vec<Shown> {
        ICONS.lock().unwrap().iter().filter(|i| !i.hidden && i.icon != 0).map(|i| Shown { key: i.key, icon: i.icon, tip: i.tip.clone(), exe: i.exe.clone() }).collect()
    }

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
        let icons = ICONS.lock().unwrap();
        let Some(i) = icons.iter().find(|i| i.key == key) else { return false };
        if i.callback == 0 {
            return false;
        }
        let send = |event: u32| {
            let (w, l) = if i.version >= 4 {
                ((at.0 as u16 as usize) | ((at.1 as u16 as usize) << 16), ((event & 0xffff) | ((i.id & 0xffff) << 16)) as isize)
            } else {
                (i.id as usize, event as isize)
            };
            // SAFETY: the program's window and the message it asked for; no pointers.
            unsafe { SendNotifyMessageW(hwnd_of(i.hwnd), i.callback, w, l) };
        };
        // What opens may come to the front (a menu has to, to close again).
        let allow = || {
            // SAFETY: a plain call; it fails unless we may set the front ourselves.
            if unsafe { AllowSetForegroundWindow(i.pid) } == 0 {
                note(format!("tray: {} could not be let come to the front", i.exe));
            }
        };
        match press {
            Press::LeftDown => send(WM_LBUTTONDOWN),
            Press::LeftUp => {
                allow();
                send(WM_LBUTTONUP);
                if i.version >= 3 {
                    send(NIN_SELECT);
                }
            }
            Press::RightDown => send(WM_RBUTTONDOWN),
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
        #[test]
        fn the_structs_are_the_size_the_tray_is_handed() {
            // NOTIFYICONDATAW in its 32-bit form, and SHELLTRAYDATA around it.
            assert_eq!(std::mem::size_of::<super::IconData>(), 956);
            assert_eq!(std::mem::size_of::<super::TrayData>(), 964);
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
    pub fn place(_rect: (i32, i32, i32, i32)) {}
    pub fn first() -> bool {
        false
    }
    pub fn keep_first() -> bool {
        false
    }
    pub fn ask_again() {}
    pub fn sweep() {}
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
pub use imp::{ask_again, describe, first, keep_first, notes, place, seen, set_rects, shown, start, stop, sweep, tell};
