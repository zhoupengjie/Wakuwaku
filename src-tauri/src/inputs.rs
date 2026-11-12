// The input method in front as Windows' own taskbar knows it, for the
// taskbar's input-method mark (island.ts): which one it is (微软拼音 or
// 搜狗拼音, its language, its own icon), its mode as Windows' taskbar draws it
// (a glyph of Windows' own, 中 or 英, for Microsoft's input methods; the input
// method's own picture for one such as Sogou's), and the input methods to
// pick from with their icons (the mark's right press), one picked switched
// to (zhoupengjie/Wakuwaku#26).
//
// The keyboard layout of the window in front says only the language: every
// Chinese text service is 0x08040804 there. Which one is active is the
// focused thread's alone, and nothing asked from here answers for it (tried
// 2026-10-14): TSF's GetActiveProfile answers each thread for itself, and a
// thread here kept saying 微软拼音 while the window in front had switched to
// English, with or without a message loop and TSF started of its own;
// ITfLangBarMgr::GetInputProcessorProfiles for the thread in front answered
// with our own thread's; CTF\Assemblies in the registry is not written when
// one switches (last written the day Windows was set up). Windows' taskbar
// learns it from its input switcher (InputSwitch.dll,
// InputSwitchControl), which the text services tell each time the input
// method or its mode changes, whichever window has the focus; one made here
// is told the same, and calls us (its callback), so nothing is asked between
// changes. Undocumented: its table as ExplorerPatcher has it (InputSwitch.h),
// checked against InputSwitch.dll 10.0.26100's code (from Windows 11 22H2,
// build 22621, the "SV2" table, with ClickImeModeItemWithAnchor after
// ClickImeModeItem); what it fills in for the profile, read from what it
// wrote (112 bytes, its strings copies made with SHStrDupW). Every string it
// hands over is checked to be a block of the process heap before it is read,
// and freed once read; every method called, to be its DLL's. Without it (its
// making failed), the keyboard layout and the language list answer what
// they can: a layout, an old IME, or a language with one input method.
//
// On a thread of its own (the switcher calls back on the thread that made
// it, through its windows' messages), from when the taskbar is taken to
// when it is given back (taskbar/).

#[cfg(windows)]
mod imp {
    use crate::lock::Lock;
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
    use std::thread::JoinHandle;

    use serde_json::{json, Value};
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    type Hwnd = *mut c_void;

    #[repr(C)]
    struct Guid(u32, u16, u16, [u8; 8]);

    // Windows' input switcher: its class, IInputSwitchControl, and
    // IInputSwitchCallback (the one after it), which it calls.
    const CLSID_INPUT_SWITCH: Guid = Guid(0xB9BC_2A50, 0x43C3, 0x41AA, [0xA0, 0x86, 0x5D, 0xB1, 0x4E, 0x18, 0x4B, 0xAE]);
    const IID_INPUT_SWITCH: Guid = Guid(0xB9BC_2A50, 0x43C3, 0x41AA, [0xA0, 0x82, 0x5D, 0xB1, 0x4E, 0x18, 0x4B, 0xAE]);
    const IID_INPUT_SWITCH_CALLBACK: Guid = Guid(0xB9BC_2A50, 0x43C3, 0x41AA, [0xA0, 0x83, 0x5D, 0xB1, 0x4E, 0x18, 0x4B, 0xAE]);
    const IID_UNKNOWN: Guid = Guid(0, 0, 0, [0xC0, 0, 0, 0, 0, 0, 0, 0x46]);
    const COINIT_APARTMENTTHREADED: u32 = 0x2;
    const CLSCTX_INPROC_SERVER: u32 = 0x1;
    // Init's client: the desktop's taskbar, as Explorer's own.
    const ISCT_IDL_DESKTOP: u32 = 0;
    // The "SV2" table, from Windows 11 22H2.
    const SV2_BUILD: u32 = 22621;

    #[repr(C)]
    struct Obj {
        vtbl: *const c_void,
    }

    // IInputSwitchControl's table, as far as what is asked of it: Init,
    // SetCallback and GetCurrentProfile on any Windows that has it; the IME
    // mode item and switching to an input method only with the SV2 table
    // (before it they come a place sooner, and are not asked).
    #[repr(C)]
    struct ControlVtbl {
        _query_interface: usize,
        _add_ref: usize,
        release: unsafe extern "system" fn(*mut Obj) -> u32,
        init: unsafe extern "system" fn(*mut Obj, u32) -> i32,
        set_callback: unsafe extern "system" fn(*mut Obj, *const Callback) -> i32,
        _show_input_switch: usize,
        _get_profile_count: usize,
        get_current_profile: unsafe extern "system" fn(*mut Obj, *mut u8) -> i32,
        _register_hotkeys: usize,
        _click_ime_mode_item: usize,
        _click_ime_mode_item_with_anchor: usize,
        _force_hide: usize,
        _show_touch_keyboard_input_switch: usize,
        _get_context_flags: usize,
        _set_context_override_mode: usize,
        get_current_ime_mode_item: unsafe extern "system" fn(*mut Obj, *mut u8) -> i32,
        activate_input_profile: unsafe extern "system" fn(*mut Obj, *const u16) -> i32,
    }

    // Our IInputSwitchCallback: whatever it is told (the profile, the mode
    // item, how many there are, shown or hidden; slots 3, 7 and 9 heard on
    // 10.0.26100), the input method is read again, once, on this thread
    // (WM_LOOK). Its methods' places beyond IUnknown are not known for sure,
    // so every one does the same; room for more than it has.
    #[repr(C)]
    struct CallbackVtbl {
        query_interface: extern "system" fn(*const Callback, *const Guid, *mut *const Callback) -> i32,
        add_ref: extern "system" fn(*const Callback) -> u32,
        release: extern "system" fn(*const Callback) -> u32,
        told: [extern "system" fn(*const Callback, usize, usize, usize) -> i32; 29],
    }

    #[repr(C)]
    struct Callback {
        vtbl: &'static CallbackVtbl,
    }

    static CALLBACK_VTBL: CallbackVtbl = CallbackVtbl { query_interface: callback_query, add_ref: callback_count, release: callback_count, told: [callback_told; 29] };
    static CALLBACK: Callback = Callback { vtbl: &CALLBACK_VTBL };

    extern "system" fn callback_query(this: *const Callback, iid: *const Guid, out: *mut *const Callback) -> i32 {
        // SAFETY: the caller's own GUID and out-parameter.
        unsafe {
            let same = |g: &Guid| (*iid).0 == g.0 && (*iid).1 == g.1 && (*iid).2 == g.2 && (*iid).3 == g.3;
            if same(&IID_UNKNOWN) || same(&IID_INPUT_SWITCH_CALLBACK) {
                *out = this;
                return 0;
            }
            *out = std::ptr::null();
        }
        E_NOINTERFACE
    }

    // A static object: never freed.
    extern "system" fn callback_count(_this: *const Callback) -> u32 {
        1
    }

    extern "system" fn callback_told(_this: *const Callback, _a: usize, _b: usize, _c: usize) -> i32 {
        // SAFETY: a posted message, no pointers.
        unsafe { PostThreadMessageW(THREAD_ID.load(Ordering::SeqCst), WM_LOOK, 0, 0) };
        0
    }

    const E_NOINTERFACE: i32 = 0x8000_4002_u32 as i32;
    const WM_QUIT: u32 = 0x0012;
    const WM_USER: u32 = 0x0400;
    const WM_INPUTLANGCHANGEREQUEST: u32 = 0x0050;
    // The input method read again (the switcher told us, or the layout in front changed).
    const WM_LOOK: u32 = 0x8001;
    // One picked from the list switched to: its id, boxed, in lParam.
    const WM_PICK: u32 = 0x8002;
    const PM_NOREMOVE: u32 = 0x0;
    const PM_REMOVE: u32 = 0x1;
    const LOCALE_SLOCALIZEDDISPLAYNAME: u32 = 0x2;

    #[repr(C)]
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
    struct OsVersion {
        size: u32,
        major: u32,
        minor: u32,
        build: u32,
        platform: u32,
        service_pack: [u16; 128],
    }

    #[link(name = "ole32")]
    extern "system" {
        fn CoInitializeEx(reserved: *mut c_void, flags: u32) -> i32;
        fn CoUninitialize();
        fn CoCreateInstance(clsid: *const Guid, outer: *mut c_void, ctx: u32, iid: *const Guid, out: *mut *mut c_void) -> i32;
        fn CoTaskMemFree(p: *mut c_void);
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetMessageW(msg: *mut Msg, hwnd: Hwnd, min: u32, max: u32) -> i32;
        fn PeekMessageW(msg: *mut Msg, hwnd: Hwnd, min: u32, max: u32, remove: u32) -> i32;
        fn DispatchMessageW(msg: *const Msg) -> isize;
        fn PostThreadMessageW(thread: u32, msg: u32, wparam: usize, lparam: isize) -> i32;
        fn PostMessageW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> i32;
        fn GetForegroundWindow() -> Hwnd;
        fn DestroyIcon(icon: isize) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentThreadId() -> u32;
        fn GetModuleHandleW(name: *const u16) -> isize;
        fn GetProcessHeap() -> isize;
        fn HeapValidate(heap: isize, flags: u32, mem: *const c_void) -> i32;
        fn HeapSize(heap: isize, flags: u32, mem: *const c_void) -> usize;
        fn GetLocaleInfoEx(name: *const u16, what: u32, out: *mut u16, len: i32) -> i32;
        fn ExpandEnvironmentStringsW(from: *const u16, to: *mut u16, len: u32) -> u32;
        fn GetWindowsDirectoryW(out: *mut u16, len: u32) -> u32;
    }

    #[link(name = "ntdll")]
    extern "system" {
        fn RtlGetVersion(info: *mut OsVersion) -> i32;
    }

    #[link(name = "shlwapi")]
    extern "system" {
        fn SHLoadIndirectString(source: *const u16, out: *mut u16, len: u32, reserved: *mut c_void) -> i32;
    }

    #[link(name = "version")]
    extern "system" {
        fn GetFileVersionInfoSizeW(name: *const u16, handle: *mut u32) -> u32;
        fn GetFileVersionInfoW(name: *const u16, handle: u32, len: u32, data: *mut c_void) -> i32;
        fn VerQueryValueW(block: *const c_void, sub: *const u16, buffer: *mut *mut c_void, len: *mut u32) -> i32;
    }

    #[link(name = "shell32")]
    extern "system" {
        fn SHDefExtractIconW(file: *const u16, index: i32, flags: u32, large: *mut isize, small: *mut isize, sizes: u32) -> i32;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn until_nul(buf: &[u16]) -> String {
        String::from_utf16_lossy(&buf[..buf.iter().position(|&c| c == 0).unwrap_or(buf.len())])
    }

    // The switcher's thread, its id (for the messages to it), the layout in
    // front as last told (keys_loop), the icons' size (physical px).
    static THREAD: Lock<Option<JoinHandle<()>>> = Lock::new("inputs::THREAD", None);
    static THREAD_ID: AtomicU32 = AtomicU32::new(0);
    static LAYOUT: AtomicU32 = AtomicU32::new(0);
    static SIZE: AtomicI32 = AtomicI32::new(16);
    // What the page is told (now()), and the icons drawn for it: by key (an
    // input method's id, or "mode:" and a hash of the mode's picture), each
    // with whether it was sent.
    static NOW: Lock<Value> = Lock::new("inputs::NOW", Value::Null);
    static PNGS: Lock<Option<HashMap<String, (String, bool)>>> = Lock::new("inputs::PNGS", None);
    // The input methods to pick from, as last read (methods()), and when.
    static METHODS: Lock<Option<(Vec<Method>, std::time::Instant)>> = Lock::new("inputs::METHODS", None);
    // Asked to stop before its thread had given out its id (stop).
    static STOPPING: AtomicBool = AtomicBool::new(false);
    // How an icon is drawn into a PNG (taskbar/win/tray.rs png_of).
    type Draw = fn(isize, i32) -> Option<String>;
    static DRAW: Lock<Option<Draw>> = Lock::new("inputs::DRAW", None);

    // One of the input methods to pick from, as Windows' language list has
    // it (Control Panel\International\User Profile): its id there
    // ("0804:{CLSID}{profile}" for a text service, "0409:00000409" for a
    // layout, the form ActivateInputProfile takes), its language's tag,
    // number and name, its own name, its keyboard layout's handle (a
    // layout's or an old IME's; 0 for a text service), its icon (file and
    // index, as the text service or the old IME registered it), and whether
    // it is Windows' own (its icon among Windows' files).
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct Method {
        pub id: String,
        pub tag: String,
        pub lang: u16,
        pub lang_name: String,
        pub name: String,
        pub hkl: u32,
        pub icon: Option<(String, i32)>,
        pub own: bool,
    }

    // What the switcher says of the input method in front.
    #[derive(Clone, Debug, Default, PartialEq)]
    struct Profile {
        hkl: u32,
        lang_name: String,
        name: String,
        tag: String,
    }

    // Its mode as Windows' taskbar draws it: its tip's first line (中文模式),
    // whether Windows draws a glyph for it (Microsoft's own), the input
    // method's own picture of it as a PNG (for one such as Sogou's), and
    // whether it is greyed.
    #[derive(Clone, Debug, Default, PartialEq)]
    struct Mode {
        tip: String,
        glyph: bool,
        png: Option<String>,
        off: bool,
    }

    fn build() -> u32 {
        let mut v = OsVersion { size: std::mem::size_of::<OsVersion>() as u32, major: 0, minor: 0, build: 0, platform: 0, service_pack: [0; 128] };
        // SAFETY: our own struct, its size set.
        if unsafe { RtlGetVersion(&mut v) } == 0 {
            v.build
        } else {
            0
        }
    }

    // A string the switcher handed over, read if it is a block of the
    // process heap (where SHStrDupW's CoTaskMemAlloc puts it); a field that
    // is not one (another Windows' layout) is left alone.
    fn peek_string(p: usize) -> Option<String> {
        if p == 0 {
            return None;
        }
        // SAFETY: read only within a block of the heap, as the heap says it is.
        unsafe {
            let heap = GetProcessHeap();
            if HeapValidate(heap, 0, p as *const c_void) == 0 {
                return None;
            }
            let units = HeapSize(heap, 0, p as *const c_void) / 2;
            Some(until_nul(std::slice::from_raw_parts(p as *const u16, units.min(4096))))
        }
    }

    // Freed once read (only when what it filled in was as known).
    fn free_string(p: usize) {
        // SAFETY: one of its SHStrDupW copies, given to us, freed once.
        unsafe { CoTaskMemFree(p as *mut c_void) };
    }

    fn word(buf: &[u8], at: usize) -> usize {
        let mut b = [0u8; 8];
        b.copy_from_slice(&buf[at..at + 8]);
        usize::from_ne_bytes(b)
    }

    // The switcher made on this thread and told to call us; None where it
    // cannot be had, or its methods are not its DLL's.
    struct Control {
        obj: *mut Obj,
        sv2: bool,
    }

    impl Control {
        fn new() -> Result<Control, String> {
            let mut obj: *mut Obj = std::ptr::null_mut();
            // SAFETY: COM is up on this thread (run); the object released
            // here should it not do, else by drop.
            unsafe {
                let made = CoCreateInstance(&CLSID_INPUT_SWITCH, std::ptr::null_mut(), CLSCTX_INPROC_SERVER, &IID_INPUT_SWITCH, &mut obj as *mut *mut Obj as *mut *mut c_void);
                if made < 0 || obj.is_null() {
                    return Err(format!("not made ({made:#x})"));
                }
                let sv2 = build() >= SV2_BUILD;
                let v = &*((*obj).vtbl as *const ControlVtbl);
                let mut used = vec![v.init as usize, v.set_callback as usize, v.get_current_profile as usize];
                if sv2 {
                    used.extend([v.get_current_ime_mode_item as usize, v.activate_input_profile as usize]);
                }
                let failed = if !used.iter().all(|&f| in_input_switch(f)) {
                    Some("its table is not as known".to_string())
                } else {
                    let init = (v.init)(obj, ISCT_IDL_DESKTOP);
                    let told = if init < 0 { init } else { (v.set_callback)(obj, &CALLBACK) };
                    (init < 0 || told < 0).then(|| format!("not started (Init {init:#x}, SetCallback {told:#x})"))
                };
                if let Some(failed) = failed {
                    (v.release)(obj);
                    return Err(failed);
                }
                Ok(Control { obj, sv2 })
            }
        }

        fn vtbl(&self) -> &ControlVtbl {
            // SAFETY: a live COM object's table, as far as checked (new).
            unsafe { &*((*self.obj).vtbl as *const ControlVtbl) }
        }

        // The input method in front: at 0 a layout's handle (0 for a text
        // service), at 16 its language's name, at 32 its own, at 64 its
        // language's tag; strings at 8 (a short name: 简体, ENG), 24 (a
        // layout's: US), 72 (a font) and 96 (its icon's file) freed too.
        fn profile(&self) -> Option<Profile> {
            let mut buf = [0u8; 512];
            // SAFETY: room for more than the 112 bytes it writes.
            if unsafe { (self.vtbl().get_current_profile)(self.obj, buf.as_mut_ptr()) } < 0 {
                return None;
            }
            let mut strings: HashMap<usize, String> = HashMap::new();
            for at in [8, 16, 24, 32, 64, 72, 96] {
                if let Some(s) = peek_string(word(&buf, at)) {
                    strings.insert(at, s);
                }
            }
            let tag = strings.get(&64).cloned().unwrap_or_default();
            let name = strings.get(&32).cloned().unwrap_or_default();
            // Not as it was on 10.0.26100: neither believed nor freed.
            if name.is_empty() || tag.is_empty() || !tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return None;
            }
            strings.keys().for_each(|&at| free_string(word(&buf, at)));
            Some(Profile { hkl: word(&buf, 0) as u32, lang_name: strings.remove(&16).unwrap_or_default(), name, tag })
        }

        // Its mode item (tip, icon, disabled, hidden, glyph), from the SV2
        // table; None while hidden or where it has none.
        fn mode(&self, size: i32) -> Option<Mode> {
            if !self.sv2 {
                return None;
            }
            let mut buf = [0u8; 128];
            // SAFETY: room for more than the 40 bytes it writes.
            if unsafe { (self.vtbl().get_current_ime_mode_item)(self.obj, buf.as_mut_ptr()) } < 0 {
                return None;
            }
            let (tip, glyph) = (peek_string(word(&buf, 0)), peek_string(word(&buf, 24)));
            for (at, s) in [(0, &tip), (24, &glyph)] {
                if s.is_some() {
                    free_string(word(&buf, at));
                }
            }
            let (tip, glyph) = (tip.unwrap_or_default(), glyph.unwrap_or_default());
            let icon = word(&buf, 8) as isize;
            let flag = |at: usize| u32::from_ne_bytes([buf[at], buf[at + 1], buf[at + 2], buf[at + 3]]) != 0;
            // Its icon is a copy (CopyImage), ours to destroy.
            let png = (icon != 0).then(|| draw(icon, size)).flatten();
            if icon != 0 {
                // SAFETY: the copy handed over, destroyed once.
                unsafe { DestroyIcon(icon) };
            }
            if flag(20) || (tip.is_empty() && glyph.is_empty() && png.is_none()) {
                return None;
            }
            Some(Mode { tip: tip.lines().next().unwrap_or("").trim().to_string(), glyph: !glyph.is_empty(), png, off: flag(16) })
        }

        // An input method switched to, by its id in the language list.
        fn activate(&self, id: &[u16]) -> bool {
            // SAFETY: a NUL-ended string of ours, for the call.
            self.sv2 && unsafe { (self.vtbl().activate_input_profile)(self.obj, id.as_ptr()) } >= 0
        }
    }

    impl Drop for Control {
        fn drop(&mut self) {
            // SAFETY: ours, told to stop calling, released once.
            unsafe {
                (self.vtbl().set_callback)(self.obj, std::ptr::null());
                (self.vtbl().release)(self.obj);
            }
        }
    }

    // Whether a method's code is InputSwitch.dll's (within its image).
    fn in_input_switch(f: usize) -> bool {
        let name = wide("InputSwitch.dll");
        // SAFETY: the loaded module's own headers, read where they say.
        unsafe {
            let base = GetModuleHandleW(name.as_ptr()) as usize;
            if base == 0 {
                return false;
            }
            let pe = base + *((base + 0x3c) as *const u32) as usize;
            // SizeOfImage: the optional header's, after the signature and the file header.
            let size = *((pe + 24 + 56) as *const u32) as usize;
            f >= base && f < base + size
        }
    }

    fn draw(icon: isize, size: i32) -> Option<String> {
        let draw = (*DRAW.lock().unwrap())?;
        draw(icon, size)
    }

    // An icon file's (environment strings in it as the registry may have
    // them; a negative index a resource's id), drawn at size px.
    fn draw_file(file: &str, index: i32, size: i32) -> Option<String> {
        let path = expand(file);
        let path = wide(&path);
        let (mut large, mut small) = (0isize, 0isize);
        // SAFETY: our own out-parameters; the icon made is destroyed.
        unsafe {
            if SHDefExtractIconW(path.as_ptr(), index, 0, &mut large, &mut small, size as u32 & 0xffff) != 0 || large == 0 {
                return None;
            }
            let png = draw(large, size);
            DestroyIcon(large);
            if small != 0 {
                DestroyIcon(small);
            }
            png
        }
    }

    fn expand(s: &str) -> String {
        let from = wide(s);
        let mut out = [0u16; 1024];
        // SAFETY: our own buffers, the size passed.
        let n = unsafe { ExpandEnvironmentStringsW(from.as_ptr(), out.as_mut_ptr(), out.len() as u32) } as usize;
        if n == 0 || n > out.len() {
            return s.to_string();
        }
        until_nul(&out)
    }

    // A string such as "@C:\Windows\system32\input.dll,-5000" (MUI), as the
    // user's language has it; the string itself if it is not one.
    fn indirect(s: &str) -> String {
        if !s.starts_with('@') {
            return s.to_string();
        }
        let from = wide(s);
        let mut out = [0u16; 512];
        // SAFETY: our own buffers, the size passed.
        if unsafe { SHLoadIndirectString(from.as_ptr(), out.as_mut_ptr(), out.len() as u32, std::ptr::null_mut()) } < 0 {
            return String::new();
        }
        until_nul(&out)
    }

    fn lang_name(tag: &str) -> String {
        let name = wide(tag);
        let mut out = [0u16; 256];
        // SAFETY: our own buffers, the size passed.
        let n = unsafe { GetLocaleInfoEx(name.as_ptr(), LOCALE_SLOCALIZEDDISPLAYNAME, out.as_mut_ptr(), out.len() as i32) };
        if n > 0 {
            until_nul(&out)
        } else {
            tag.to_string()
        }
    }

    fn windows_dir() -> String {
        let mut out = [0u16; 260];
        // SAFETY: our own buffer, its size passed.
        unsafe { GetWindowsDirectoryW(out.as_mut_ptr(), out.len() as u32) };
        until_nul(&out)
    }

    // Windows' own input method: its icon among Windows' files and made by
    // Microsoft. Among Windows' files alone was not enough: 小狼毫 (Weasel)
    // keeps its text service and its icon in System32 too, and was taken
    // for Windows' own, 中 / 英 drawn for it rather than its own picture
    // (zhoupengjie/Wakuwaku#26). windows: lower case.
    pub fn is_windows_own(file: &str, windows: &str, company: &str) -> bool {
        file.to_lowercase().starts_with(windows) && company.to_lowercase().contains("microsoft")
    }

    // Who made a file, as its version information says (CompanyName), or
    // nothing.
    fn company(path: &str) -> String {
        let file = wide(path);
        // SAFETY: a buffer of the size the call asks for; the value read
        // points into it and is copied before it goes.
        unsafe {
            let size = GetFileVersionInfoSizeW(file.as_ptr(), std::ptr::null_mut());
            if size == 0 {
                return String::new();
            }
            let mut data = vec![0u8; size as usize];
            if GetFileVersionInfoW(file.as_ptr(), 0, size, data.as_mut_ptr() as *mut c_void) == 0 {
                return String::new();
            }
            let mut at: *mut c_void = std::ptr::null_mut();
            let mut len = 0u32;
            let translation = wide("\\VarFileInfo\\Translation");
            if VerQueryValueW(data.as_ptr() as *const c_void, translation.as_ptr(), &mut at, &mut len) == 0 || len < 4 {
                return String::new();
            }
            let (lang, page) = (*(at as *const u16), *(at as *const u16).add(1));
            let key = wide(&format!("\\StringFileInfo\\{lang:04x}{page:04x}\\CompanyName"));
            if VerQueryValueW(data.as_ptr() as *const c_void, key.as_ptr(), &mut at, &mut len) == 0 || len <= 1 {
                return String::new();
            }
            String::from_utf16_lossy(std::slice::from_raw_parts(at as *const u16, len as usize)).trim_end_matches('\0').trim().to_string()
        }
    }

    // The input methods to pick from, in the order of Windows' language
    // list: each language (Languages), then its input methods by their
    // order there. A text service's name and icon as it registered them
    // (CTF\TIP, the user's own first, then the machine's), a layout's (and
    // an old IME's) as Keyboard Layouts has them.
    pub fn methods() -> Vec<Method> {
        let user = RegKey::predef(HKEY_CURRENT_USER);
        let machine = RegKey::predef(HKEY_LOCAL_MACHINE);
        let Ok(profile) = user.open_subkey(r"Control Panel\International\User Profile") else { return Vec::new() };
        let tags: Vec<String> = profile.get_value("Languages").unwrap_or_default();
        let windows = windows_dir().to_lowercase();
        let mut all = Vec::new();
        for tag in tags {
            let Ok(key) = profile.open_subkey(&tag) else { continue };
            let lang_name = lang_name(&tag);
            let mut ids: Vec<(u32, String)> = key
                .enum_values()
                .flatten()
                .filter(|(name, _)| name.len() > 5 && name.is_ascii() && name.as_bytes()[4] == b':' && u16::from_str_radix(&name[..4], 16).is_ok())
                .map(|(name, value)| (u32::from_ne_bytes(value.bytes.get(..4).and_then(|b| b.try_into().ok()).unwrap_or([0xff; 4])), name))
                .collect();
            ids.sort();
            for (_, id) in ids {
                let lang = u16::from_str_radix(&id[..4], 16).unwrap_or(0);
                let rest = &id[5..];
                let mut m = Method { id: id.clone(), tag: tag.clone(), lang, lang_name: lang_name.clone(), ..Default::default() };
                if rest.starts_with('{') && rest.len() >= 76 {
                    // A text service: {CLSID}{profile}.
                    let (clsid, guid) = (&rest[..38], &rest[38..76]);
                    let path = format!(r"Software\Microsoft\CTF\TIP\{clsid}\LanguageProfile\0x{lang:08x}\{guid}");
                    let Some(reg) = user.open_subkey(&path).ok().filter(|k| k.get_raw_value("Description").is_ok()).or_else(|| machine.open_subkey(&path).ok()) else { continue };
                    let shown: String = reg.get_value("Display Description").unwrap_or_default();
                    let plain: String = reg.get_value("Description").unwrap_or_default();
                    m.name = Some(indirect(&shown)).filter(|s| !s.is_empty()).unwrap_or(plain);
                    let file: String = reg.get_value("IconFile").unwrap_or_default();
                    let index: u32 = reg.get_value("IconIndex").unwrap_or(0);
                    if !file.is_empty() {
                        let path = expand(&file);
                        m.own = is_windows_own(&path, &windows, &company(&path));
                        m.icon = Some((file, index as i32));
                    }
                } else if let Ok(klid) = u32::from_str_radix(rest, 16) {
                    // A layout (or an old IME, E0xxxxxx): its handle, the
                    // language's in the low word, the layout's in the high
                    // (its Layout Id's for a variant of another).
                    let Ok(reg) = machine.open_subkey(format!(r"SYSTEM\CurrentControlSet\Control\Keyboard Layouts\{rest}")) else { continue };
                    let shown: String = reg.get_value("Layout Display Name").unwrap_or_default();
                    let plain: String = reg.get_value("Layout Text").unwrap_or_default();
                    m.name = Some(indirect(&shown)).filter(|s| !s.is_empty()).unwrap_or(plain);
                    let variant: Option<String> = reg.get_value("Layout Id").ok();
                    m.hkl = if klid & 0xf000_0000 == 0xe000_0000 {
                        klid
                    } else if let Some(v) = variant.and_then(|v| u32::from_str_radix(&v, 16).ok()) {
                        (0xf000 | v) << 16 | lang as u32
                    } else {
                        (klid & 0xffff) << 16 | lang as u32
                    };
                    let ime: String = reg.get_value("Ime File").unwrap_or_default();
                    if !ime.is_empty() {
                        m.icon = Some((format!(r"{windows}\system32\{ime}"), 0));
                        m.own = false;
                    }
                } else {
                    continue;
                }
                all.push(m);
            }
        }
        all
    }

    // Which of them the switcher's profile is (a layout by its handle; a
    // text service by its language and name, or the language's only one);
    // without it, the layout in front (a layout or an old IME by its handle,
    // else the language's only text service).
    pub fn find<'a>(methods: &'a [Method], profile: Option<(u32, &str, &str)>, layout: u32) -> Option<&'a Method> {
        if let Some((hkl, tag, name)) = profile {
            // (A text service standing in for a layout may say one too.)
            if let Some(m) = methods.iter().find(|m| hkl != 0 && m.hkl == hkl) {
                return Some(m);
            }
            let same: Vec<&Method> = methods.iter().filter(|m| m.hkl == 0 && m.tag.eq_ignore_ascii_case(tag)).collect();
            return same.iter().find(|m| m.name == name).or(if same.len() == 1 { same.first() } else { None }).copied();
        }
        if let Some(m) = methods.iter().find(|m| m.hkl != 0 && m.hkl == layout) {
            return Some(m);
        }
        let same: Vec<&Method> = methods.iter().filter(|m| m.hkl == 0 && m.lang == layout as u16).collect();
        (same.len() == 1).then(|| same[0])
    }

    // As last read; read again when asked for one not among them (one may
    // have been added), at most every half minute.
    fn cached_methods(again: bool) -> Vec<Method> {
        let mut kept = METHODS.lock().unwrap();
        if kept.as_ref().is_none_or(|(_, at)| again && at.elapsed() > std::time::Duration::from_secs(30)) {
            *kept = Some((methods(), std::time::Instant::now()));
        }
        kept.as_ref().map(|(m, _)| m.clone()).unwrap_or_default()
    }

    // An icon kept for the page under its key; drawn once.
    // Drawn with PNGS let go (drawing takes DRAW): one lock at a time.
    fn keep_png(key: &str, make: impl FnOnce() -> Option<String>) -> Option<String> {
        if PNGS.lock().unwrap().as_ref().is_some_and(|p| p.contains_key(key)) {
            return Some(key.to_string());
        }
        let png = make()?;
        PNGS.lock().unwrap().get_or_insert_with(HashMap::new).entry(key.to_string()).or_insert((png, false));
        Some(key.to_string())
    }

    fn method_png(m: &Method, size: i32) -> Option<String> {
        let (file, index) = m.icon.clone()?;
        keep_png(&m.id, || draw_file(&file, index, size))
    }

    // FNV-1a, for a mode picture's key.
    fn hash(s: &str) -> u64 {
        s.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3))
    }

    // The input method in front read again: told to the page (taskbar/
    // keys_loop) when it changed; which one it was taken for, in the log,
    // when that changed (a person's log then says which way it went: its
    // name, id, icon and Windows' own or not, #26).
    fn look(control: Option<&Control>, changed: &dyn Fn(), log: &dyn Fn(&str)) {
        let size = SIZE.load(Ordering::SeqCst);
        let profile = control.and_then(Control::profile);
        let mode = control.and_then(|c| c.mode(size));
        let layout = LAYOUT.load(Ordering::SeqCst);
        let as_told = profile.as_ref().map(|p| (p.hkl, p.tag.as_str(), p.name.as_str()));
        let mut found = find(&cached_methods(false), as_told, layout).cloned();
        if found.is_none() {
            found = find(&cached_methods(true), as_told, layout).cloned();
        }
        let now = if profile.is_none() && found.is_none() {
            Value::Null
        } else {
            let mode = mode.map(|m| {
                let icon = m.png.and_then(|png| keep_png(&format!("mode:{:016x}", hash(&png)), || Some(png)));
                json!({ "tip": m.tip, "glyph": m.glyph, "icon": icon, "off": m.off })
            });
            json!({
                "id": found.as_ref().map(|m| m.id.clone()),
                "name": found.as_ref().map(|m| m.name.clone()).filter(|n| !n.is_empty()).or(profile.as_ref().map(|p| p.name.clone())),
                "lang": profile.as_ref().map(|p| p.lang_name.clone()).filter(|n| !n.is_empty()).or(found.as_ref().map(|m| m.lang_name.clone())),
                "icon": found.as_ref().and_then(|m| method_png(m, size)),
                "own": found.as_ref().is_none_or(|m| m.own),
                "mode": mode,
            })
        };
        let mut was = NOW.lock().unwrap();
        if *was != now {
            if was["id"] != now["id"] || was["own"] != now["own"] {
                let said = match &found {
                    Some(m) => format!("{} ({}), icon {}, {}", m.name, m.id, m.icon.as_ref().map_or("none", |i| i.0.as_str()), if m.own { "Windows' own" } else { "not Windows' own" }),
                    None => format!("{} (not in the language list)", profile.as_ref().map_or("?", |p| p.name.as_str())),
                };
                log(&format!("input method in front: {said}"));
            }
            *was = now;
            drop(was);
            changed();
        }
    }

    fn run(changed: Box<dyn Fn() + Send>, log: Box<dyn Fn(&str) + Send>) {
        // SAFETY: balanced at the end of this thread.
        let com = unsafe { CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED) } >= 0;
        let mut msg = Msg { hwnd: std::ptr::null_mut(), message: 0, wparam: 0, lparam: 0, time: 0, pt: Point { x: 0, y: 0 }, private: 0 };
        // SAFETY: our own struct; makes this thread's queue before its id is
        // given out, so nothing posted to it is lost.
        unsafe { PeekMessageW(&mut msg, std::ptr::null_mut(), WM_USER, WM_USER, PM_NOREMOVE) };
        // SAFETY: a plain query.
        THREAD_ID.store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
        // Asked to stop before its id was given out: the quit never came.
        if STOPPING.load(Ordering::SeqCst) {
            THREAD_ID.store(0, Ordering::SeqCst);
            if com {
                // SAFETY: the initialization above.
                unsafe { CoUninitialize() };
            }
            return;
        }
        let control = match Control::new() {
            Ok(c) => {
                log(&format!("input switcher: up ({} table)", if c.sv2 { "SV2" } else { "older" }));
                Some(c)
            }
            Err(e) => {
                log(&format!("input switcher: {e}; the layout in front alone"));
                None
            }
        };
        look(control.as_ref(), &*changed, &*log);
        // SAFETY: this thread's own queue, its messages dispatched (the
        // switcher's windows' among them).
        unsafe {
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                match msg.message {
                    WM_LOOK => {
                        // A burst of what it is told, read once.
                        while PeekMessageW(&mut msg, std::ptr::null_mut(), WM_LOOK, WM_LOOK, PM_REMOVE) != 0 {}
                        look(control.as_ref(), &*changed, &*log);
                    }
                    WM_PICK => {
                        let id = Box::from_raw(msg.lparam as *mut Vec<u16>);
                        if !control.as_ref().is_some_and(|c| c.activate(&id)) {
                            pick_by_layout(&until_nul(&id));
                        }
                        look(control.as_ref(), &*changed, &*log);
                    }
                    _ => {
                        DispatchMessageW(&msg);
                    }
                }
            }
        }
        drop(control);
        THREAD_ID.store(0, Ordering::SeqCst);
        if com {
            // SAFETY: the initialization above.
            unsafe { CoUninitialize() };
        }
    }

    // Without the switcher: the window in front asked to take the input
    // method's layout (a text service's, its language's), as Windows' own
    // language bar asks.
    fn pick_by_layout(id: &str) {
        let Some(m) = cached_methods(false).into_iter().find(|m| m.id == id) else { return };
        let hkl = if m.hkl != 0 { m.hkl } else { (m.lang as u32) << 16 | m.lang as u32 };
        // SAFETY: a posted message, no pointers.
        unsafe { PostMessageW(GetForegroundWindow(), WM_INPUTLANGCHANGEREQUEST, 0, hkl as i32 as isize) };
    }

    // Started with the taskbar's keys (taskbar/win/hold.rs take): told when the
    // input method in front changed; its log lines.
    pub fn start(draw: Draw, changed: Box<dyn Fn() + Send>, log: Box<dyn Fn(&str) + Send>) {
        // One lock at a time: whether it runs, then what it draws with,
        // then the thread (started by the taskbar's take alone).
        if THREAD.lock().unwrap().is_some() {
            return;
        }
        *DRAW.lock().unwrap() = Some(draw);
        STOPPING.store(false, Ordering::SeqCst);
        // A new page may have none of them.
        if let Some(pngs) = PNGS.lock().unwrap().as_mut() {
            pngs.values_mut().for_each(|v| v.1 = false);
        }
        let mut thread = THREAD.lock().unwrap();
        if thread.is_none() {
            *thread = Some(std::thread::Builder::new().name("taskbar input".into()).spawn(move || run(changed, log)).expect("a thread"));
        }
    }

    // Asked to stop (taskbar/win/hold.rs give_back): its thread, to be waited for.
    pub fn stop() -> Option<JoinHandle<()>> {
        let thread = THREAD.lock().unwrap().take()?;
        STOPPING.store(true, Ordering::SeqCst);
        let id = THREAD_ID.load(Ordering::SeqCst);
        if id != 0 {
            // SAFETY: a posted message, no pointers.
            unsafe { PostThreadMessageW(id, WM_QUIT, 0, 0) };
        }
        *NOW.lock().unwrap() = Value::Null;
        Some(thread)
    }

    // The keyboard layout in front (taskbar/win/keys.rs keys_loop, four times a
    // second): read again only when it changed.
    pub fn layout(hkl: isize) {
        let hkl = hkl as u32;
        if LAYOUT.swap(hkl, Ordering::SeqCst) != hkl {
            post_look();
        }
    }

    fn post_look() {
        let id = THREAD_ID.load(Ordering::SeqCst);
        if id != 0 {
            // SAFETY: a posted message, no pointers.
            unsafe { PostThreadMessageW(id, WM_LOOK, 0, 0) };
        }
    }

    // The icons' size (physical px); drawn anew at another.
    pub fn set_size(px: i32) {
        if px > 0 && SIZE.swap(px, Ordering::SeqCst) != px {
            *PNGS.lock().unwrap() = None;
            post_look();
        }
    }

    // The input method in front, as the page is told it: { id, name, lang,
    // icon (a key of icons()), own, mode: { tip, glyph, icon, off } }, or null.
    pub fn now() -> Value {
        NOW.lock().unwrap().clone()
    }

    // The icons the page has not had yet (all of them for a page just come
    // up), by key; then counted as had.
    pub fn icons(all: bool) -> Vec<(String, String)> {
        let mut pngs = PNGS.lock().unwrap();
        let Some(pngs) = pngs.as_mut() else { return Vec::new() };
        pngs.iter_mut()
            .filter(|(_, v)| all || !v.1)
            .map(|(k, v)| {
                v.1 = true;
                (k.clone(), v.0.clone())
            })
            .collect()
    }

    // The input methods to pick from, read anew, each with its icon (a PNG,
    // or null for a layout) and whether it is the one in front.
    pub fn list() -> Value {
        let methods = self::methods();
        *METHODS.lock().unwrap() = Some((methods.clone(), std::time::Instant::now()));
        let size = SIZE.load(Ordering::SeqCst);
        let now = NOW.lock().unwrap().get("id").and_then(Value::as_str).map(str::to_string);
        Value::Array(
            methods
                .iter()
                .map(|m| {
                    let png = method_png(m, size).and_then(|key| PNGS.lock().unwrap().as_ref().and_then(|p| p.get(&key)).map(|v| v.0.clone()));
                    json!({ "id": m.id, "lang": m.lang, "langName": m.lang_name, "name": m.name, "icon": png, "own": m.own, "current": now.as_deref() == Some(m.id.as_str()) })
                })
                .collect(),
        )
    }

    // One picked from the list switched to, on the switcher's thread.
    pub fn pick(id: &str) -> bool {
        let thread = THREAD_ID.load(Ordering::SeqCst);
        if thread == 0 || id.is_empty() || id.len() > 100 {
            return false;
        }
        let boxed = Box::into_raw(Box::new(wide(id)));
        // SAFETY: the box goes with the message, taken back by the thread;
        // here should the message not go.
        unsafe {
            if PostThreadMessageW(thread, WM_PICK, 0, boxed as isize) == 0 {
                drop(Box::from_raw(boxed));
                return false;
            }
        }
        true
    }
}

#[cfg(not(windows))]
mod imp {
    use serde_json::Value;

    type Draw = fn(isize, i32) -> Option<String>;

    pub fn start(_draw: Draw, _changed: Box<dyn Fn() + Send>, _log: Box<dyn Fn(&str) + Send>) {}
    pub fn stop() -> Option<std::thread::JoinHandle<()>> {
        None
    }
    pub fn layout(_hkl: isize) {}
    pub fn set_size(_px: i32) {}
    pub fn now() -> Value {
        Value::Null
    }
    pub fn icons(_all: bool) -> Vec<(String, String)> {
        Vec::new()
    }
    pub fn list() -> Value {
        Value::Array(Vec::new())
    }
    pub fn pick(_id: &str) -> bool {
        false
    }
}

pub use imp::{icons, layout, list, now, pick, set_size, start, stop};

#[cfg(all(test, windows))]
mod tests {
    use super::imp::{find, Method};

    fn method(id: &str, tag: &str, name: &str, hkl: u32) -> Method {
        let lang = u16::from_str_radix(&id[..4], 16).unwrap();
        Method { id: id.into(), tag: tag.into(), lang, name: name.into(), hkl, ..Default::default() }
    }

    fn list() -> Vec<Method> {
        vec![
            method("0804:{81D4E9C9-1D3B-41BC-9E6C-4B40BF79E35E}{FA550B04-5AD7-411F-A5AC-CA038EC515D7}", "zh-Hans-CN", "微软拼音", 0),
            method("0804:{E7EA138E-69F8-11D7-A6EA-00065B844310}{E7EA138F-69F8-11D7-A6EA-00065B844311}", "zh-Hans-CN", "搜狗拼音输入法", 0),
            method("0804:E0200804", "zh-Hans-CN", "老式输入法", 0xE020_0804),
            method("0409:00000409", "en-US", "美式键盘", 0x0409_0409),
            method("0411:{03B5835F-F03C-411B-9CE2-AA23E1171E36}{A76C93D9-5523-4E90-AAFA-4DB112F9AC76}", "ja-JP", "Microsoft IME", 0),
        ]
    }

    #[test]
    fn the_switchers_profile_by_its_layout_or_its_name() {
        let all = list();
        let id = |m: Option<&Method>| m.map(|m| m.name.clone());
        // A layout by its handle, whatever the switcher calls it.
        assert_eq!(id(find(&all, Some((0x0409_0409, "en-US", "美式键盘 键盘")), 0)), Some("美式键盘".into()));
        // Two text services of one language: by name.
        assert_eq!(id(find(&all, Some((0, "zh-Hans-CN", "搜狗拼音输入法")), 0x0804_0804)), Some("搜狗拼音输入法".into()));
        assert_eq!(id(find(&all, Some((0, "zh-hans-cn", "微软拼音")), 0x0804_0804)), Some("微软拼音".into()));
        // A name not among them: not guessed between two.
        assert_eq!(id(find(&all, Some((0, "zh-Hans-CN", "五笔")), 0x0804_0804)), None);
        // The language's only one, named otherwise.
        assert_eq!(id(find(&all, Some((0, "ja-JP", "Microsoft IME (日本語)")), 0x0411_0411)), Some("Microsoft IME".into()));
    }

    #[test]
    fn without_the_switcher_the_layout_in_front() {
        let all = list();
        let id = |m: Option<&Method>| m.map(|m| m.name.clone());
        assert_eq!(id(find(&all, None, 0x0409_0409)), Some("美式键盘".into()));
        assert_eq!(id(find(&all, None, 0xE020_0804)), Some("老式输入法".into()));
        // Japanese has one text service; Chinese two, which the layout cannot tell apart.
        assert_eq!(id(find(&all, None, 0x0411_0411)), Some("Microsoft IME".into()));
        assert_eq!(id(find(&all, None, 0x0804_0804)), None);
        assert_eq!(id(find(&all, None, 0x0407_0407)), None);
    }

    #[test]
    fn windows_own_by_its_maker_not_only_its_folder() {
        use super::imp::is_windows_own;
        let windows = r"c:\windows";
        assert!(is_windows_own(r"C:\Windows\System32\InputMethod\CHS\ChsIME.exe", windows, "Microsoft Corporation"));
        // 小狼毫 keeps its icon in System32 (#26): not Windows' own.
        assert!(!is_windows_own(r"C:\WINDOWS\system32\weasel.dll", windows, "式恕堂"));
        assert!(!is_windows_own(r"C:\WINDOWS\system32\weasel.dll", windows, ""));
        // Microsoft's, elsewhere (an input method from the Store): not Windows' own either.
        assert!(!is_windows_own(r"C:\Program Files\WindowsApps\x\ime.dll", windows, "Microsoft Corporation"));
    }
}
