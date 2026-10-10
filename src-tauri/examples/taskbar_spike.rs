// Phase 0 of the taskbar home: can Windows' own taskbar be put away for good,
// with a bar of ours in its place? A plain window (no page) along the bottom
// of the main display, registered as an app bar, with buttons for what the
// taskbar opens (Start, search, quick settings, ...). What happens is written
// down in data/taskbar-spike/spike.log: the taskbar coming back and how it
// was seen, Explorer restarting, the screens changing, sleep, full screen.
//
//   taskbar_spike            take the taskbar's place (Ctrl+Alt+Shift+R, or
//                            the bar's last button: give it back and quit)
//   taskbar_spike --probe    with one running: press Win, Win+S, Win+A, ...
//                            one at a time and note what opened, where, and
//                            whether the taskbar came back (probe.log, and a
//                            screenshot of each)
//   taskbar_spike --status   the taskbar, the work area and the bar, now
//   taskbar_spike --quit     the bar closed, as its last button does
//   taskbar_spike --restore  give the taskbar back (after a crash)
//
// Uses src/appbar.rs and src/shell.rs as the pet will.
#[path = "../src/appbar.rs"]
mod appbar;
#[path = "../src/shell.rs"]
mod shell;

#[cfg(windows)]
fn main() {
    win::main();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Windows only");
}

#[cfg(windows)]
mod win {
    use std::ffi::c_void;
    use std::fs::{File, OpenOptions};
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use crate::{appbar, shell};

    type Hwnd = *mut c_void;
    type Handle = isize;

    #[repr(C)]
    #[derive(Default, Clone, Copy, PartialEq, Debug)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct Point {
        x: i32,
        y: i32,
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
    struct PaintStruct {
        hdc: isize,
        erase: i32,
        rc: Rect,
        restore: i32,
        inc_update: i32,
        reserved: [u8; 32],
    }

    #[repr(C)]
    #[derive(Default)]
    struct MonitorInfo {
        size: u32,
        monitor: Rect,
        work: Rect,
        flags: u32,
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
    struct SystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        ms: u16,
    }

    type WinEventProc = extern "system" fn(isize, u32, Hwnd, i32, i32, u32, u32);

    #[link(name = "user32")]
    extern "system" {
        fn RegisterClassExW(class: *const WndClassExW) -> u16;
        fn CreateWindowExW(ex: u32, class: *const u16, title: *const u16, style: u32, x: i32, y: i32, w: i32, h: i32, parent: Hwnd, menu: isize, instance: isize, param: *mut c_void) -> Hwnd;
        fn DefWindowProcW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize;
        fn GetMessageW(msg: *mut Msg, hwnd: Hwnd, min: u32, max: u32) -> i32;
        fn TranslateMessage(msg: *const Msg) -> i32;
        fn DispatchMessageW(msg: *const Msg) -> isize;
        fn PostQuitMessage(code: i32);
        fn PostMessageW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> i32;
        fn DestroyWindow(hwnd: Hwnd) -> i32;
        fn SetWindowPos(hwnd: Hwnd, after: isize, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
        fn ShowWindow(hwnd: Hwnd, cmd: i32) -> i32;
        fn IsWindowVisible(hwnd: Hwnd) -> i32;
        fn FindWindowW(class: *const u16, title: *const u16) -> Hwnd;
        fn GetClassNameW(hwnd: Hwnd, name: *mut u16, max: i32) -> i32;
        fn GetWindowTextW(hwnd: Hwnd, text: *mut u16, max: i32) -> i32;
        fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
        fn GetForegroundWindow() -> Hwnd;
        fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
        fn GetClientRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
        fn BeginPaint(hwnd: Hwnd, ps: *mut PaintStruct) -> isize;
        fn EndPaint(hwnd: Hwnd, ps: *const PaintStruct) -> i32;
        fn InvalidateRect(hwnd: Hwnd, rect: *const Rect, erase: i32) -> i32;
        fn FillRect(hdc: isize, rect: *const Rect, brush: isize) -> i32;
        fn DrawTextW(hdc: isize, text: *const u16, len: i32, rect: *mut Rect, format: u32) -> i32;
        fn SetTimer(hwnd: Hwnd, id: usize, ms: u32, func: *const c_void) -> usize;
        fn RegisterHotKey(hwnd: Hwnd, id: i32, mods: u32, vk: u32) -> i32;
        fn SendInput(count: u32, inputs: *const Input, size: i32) -> u32;
        fn SetWinEventHook(min: u32, max: u32, module: isize, func: WinEventProc, pid: u32, tid: u32, flags: u32) -> isize;
        fn UnhookWinEvent(hook: isize) -> i32;
        fn MonitorFromPoint(pt: Point, flags: u32) -> *mut c_void;
        fn GetMonitorInfoW(monitor: *mut c_void, info: *mut MonitorInfo) -> i32;
        fn GetDpiForWindow(hwnd: Hwnd) -> u32;
        fn SetProcessDpiAwarenessContext(ctx: isize) -> i32;
        fn SystemParametersInfoW(action: u32, param: u32, pv: *mut c_void, flags: u32) -> i32;
        fn GetCursorPos(pt: *mut Point) -> i32;
        fn SetCursorPos(x: i32, y: i32) -> i32;
        fn LoadCursorW(instance: isize, name: usize) -> isize;
        fn GetDC(hwnd: Hwnd) -> isize;
        fn ReleaseDC(hwnd: Hwnd, hdc: isize) -> i32;
        fn GetSystemMetrics(index: i32) -> i32;
    }

    #[link(name = "gdi32")]
    extern "system" {
        fn CreateSolidBrush(color: u32) -> isize;
        fn DeleteObject(obj: isize) -> i32;
        fn SelectObject(hdc: isize, obj: isize) -> isize;
        fn SetBkMode(hdc: isize, mode: i32) -> i32;
        fn SetTextColor(hdc: isize, color: u32) -> u32;
        fn CreateFontW(h: i32, w: i32, esc: i32, orient: i32, weight: i32, italic: u32, underline: u32, strike: u32, charset: u32, out: u32, clip: u32, quality: u32, pitch: u32, face: *const u16) -> isize;
        fn CreateCompatibleDC(hdc: isize) -> isize;
        fn CreateCompatibleBitmap(hdc: isize, w: i32, h: i32) -> isize;
        fn BitBlt(dst: isize, x: i32, y: i32, w: i32, h: i32, src: isize, sx: i32, sy: i32, rop: u32) -> i32;
        fn GetDIBits(hdc: isize, bitmap: isize, start: u32, lines: u32, bits: *mut c_void, info: *mut BitmapInfoHeader, usage: u32) -> i32;
        fn DeleteDC(hdc: isize) -> i32;
    }

    #[link(name = "dwmapi")]
    extern "system" {
        fn DwmGetWindowAttribute(hwnd: Hwnd, attr: u32, value: *mut c_void, size: u32) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleW(name: *const u16) -> isize;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
        fn CloseHandle(handle: Handle) -> i32;
        fn QueryFullProcessImageNameW(process: Handle, flags: u32, name: *mut u16, size: *mut u32) -> i32;
        fn GetLocalTime(time: *mut SystemTime);
    }

    const CLASS: &str = "WakuwakuTaskbarSpike";
    const WM_DESTROY: u32 = 0x0002;
    const WM_PAINT: u32 = 0x000F;
    const WM_QUERYENDSESSION: u32 = 0x0011;
    const WM_ENDSESSION: u32 = 0x0016;
    const WM_SETTINGCHANGE: u32 = 0x001A;
    const WM_MOUSEACTIVATE: u32 = 0x0021;
    const WM_DISPLAYCHANGE: u32 = 0x007E;
    const WM_TIMER: u32 = 0x0113;
    const WM_LBUTTONUP: u32 = 0x0202;
    const WM_POWERBROADCAST: u32 = 0x0218;
    const WM_DPICHANGED: u32 = 0x02E0;
    const WM_HOTKEY: u32 = 0x0312;
    // The taskbar seen shown (from the event hook), lParam its window.
    const WM_TASKBAR_SHOWN: u32 = 0x8001;
    const MA_NOACTIVATE: isize = 3;
    const WS_POPUP: u32 = 0x8000_0000;
    const WS_EX_TOPMOST: u32 = 0x8;
    const WS_EX_TOOLWINDOW: u32 = 0x80;
    const WS_EX_NOACTIVATE: u32 = 0x0800_0000;
    const HWND_TOPMOST: isize = -1;
    const HWND_NOTOPMOST: isize = -2;
    const HWND_BOTTOM: isize = 1;
    const SWP_NOSIZE: u32 = 0x1;
    const SWP_NOMOVE: u32 = 0x2;
    const SWP_NOACTIVATE: u32 = 0x10;
    const SWP_SHOWWINDOW: u32 = 0x40;
    const SW_SHOWNA: i32 = 8;
    const SPI_GETWORKAREA: u32 = 0x30;
    const SPI_SETWORKAREA: usize = 0x2F;
    const EVENT_OBJECT_SHOW: u32 = 0x8002;
    const OBJID_WINDOW: i32 = 0;
    const MOD_ALT: u32 = 0x1;
    const MOD_CONTROL: u32 = 0x2;
    const MOD_SHIFT: u32 = 0x4;
    const MOD_NOREPEAT: u32 = 0x4000;
    const VK_TAB: u16 = 0x09;
    const VK_ESCAPE: u16 = 0x1B;
    const VK_LWIN: u16 = 0x5B;
    const KEYEVENTF_EXTENDEDKEY: u32 = 0x1;
    const KEYEVENTF_KEYUP: u32 = 0x2;
    const DT_CENTER: u32 = 0x1;
    const DT_VCENTER: u32 = 0x4;
    const DT_SINGLELINE: u32 = 0x20;
    const DT_END_ELLIPSIS: u32 = 0x8000;
    const DWMWA_EXTENDED_FRAME_BOUNDS: u32 = 9;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    // DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2
    const PER_MONITOR_V2: isize = -4;
    // The Win11 taskbar's height (logical px).
    const BAR_H: i32 = 48;

    static BAR: AtomicIsize = AtomicIsize::new(0);
    static CREATED_MSG: AtomicU32 = AtomicU32::new(0);
    static SHOWN_AGAIN: AtomicU32 = AtomicU32::new(0);
    static FULLSCREEN: AtomicBool = AtomicBool::new(false);
    static RESTORED: AtomicBool = AtomicBool::new(false);
    static PLACED_FOR: Mutex<Option<(Rect, i32)>> = Mutex::new(None);
    static GRANTED: Mutex<Option<(i32, i32, i32, i32)>> = Mutex::new(None);
    static LAST: Mutex<String> = Mutex::new(String::new());
    static LOG: Mutex<Option<File>> = Mutex::new(None);

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn dir() -> PathBuf {
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../data/taskbar-spike"))
    }

    fn file() -> PathBuf {
        dir().join("taskbar.json")
    }

    fn now() -> SystemTime {
        let mut t: SystemTime = unsafe { std::mem::zeroed() };
        // SAFETY: our own struct.
        unsafe { GetLocalTime(&mut t) };
        t
    }

    fn open_log(name: &str) {
        let _ = std::fs::create_dir_all(dir());
        *LOG.lock().unwrap() = OpenOptions::new().create(true).append(true).open(dir().join(name)).ok();
    }

    fn log(line: &str) {
        let t = now();
        let line = format!("{:02}:{:02}:{:02}.{:03} {line}", t.hour, t.minute, t.second, t.ms);
        println!("{line}");
        if let Some(f) = LOG.lock().unwrap().as_mut() {
            let _ = writeln!(f, "{line}");
        }
    }

    fn note(what: &str) {
        log(what);
        *LAST.lock().unwrap() = what.to_string();
    }

    fn work_area() -> Rect {
        let mut r = Rect::default();
        // SAFETY: our own struct, of the size the call writes.
        unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut r as *mut Rect as *mut c_void, 0) };
        r
    }

    fn primary() -> Rect {
        let mut info = MonitorInfo { size: std::mem::size_of::<MonitorInfo>() as u32, ..Default::default() };
        // SAFETY: our own struct, its size set; (0, 0) is always on the primary display.
        unsafe { GetMonitorInfoW(MonitorFromPoint(Point::default(), 1), &mut info) };
        info.monitor
    }

    fn rect_of(hwnd: Hwnd) -> Rect {
        let mut r = Rect::default();
        // SAFETY: our own struct; the frame as drawn, else the window's rect.
        unsafe {
            if DwmGetWindowAttribute(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS, &mut r as *mut Rect as *mut c_void, 16) != 0 {
                GetWindowRect(hwnd, &mut r);
            }
        }
        r
    }

    fn class_of(hwnd: Hwnd) -> String {
        let mut name = [0u16; 128];
        // SAFETY: our own buffer, its size passed.
        let n = unsafe { GetClassNameW(hwnd, name.as_mut_ptr(), 128) }.max(0) as usize;
        String::from_utf16_lossy(&name[..n])
    }

    fn title_of(hwnd: Hwnd) -> String {
        let mut text = [0u16; 256];
        // SAFETY: as above.
        let n = unsafe { GetWindowTextW(hwnd, text.as_mut_ptr(), 256) }.max(0) as usize;
        String::from_utf16_lossy(&text[..n])
    }

    fn exe_of(hwnd: Hwnd) -> String {
        let mut pid = 0u32;
        // SAFETY: our own out-parameters and buffer; a handle we close.
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut pid);
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

    fn describe(hwnd: Hwnd) -> String {
        if hwnd.is_null() {
            return "(none)".into();
        }
        let r = rect_of(hwnd);
        format!("{} [{}] \"{}\" at ({},{})-({},{})", exe_of(hwnd), class_of(hwnd), title_of(hwnd), r.left, r.top, r.right, r.bottom)
    }

    fn taskbars_now() -> String {
        let list: Vec<String> = shell::taskbars()
            .into_iter()
            .map(|h| {
                let h = h as Hwnd;
                // SAFETY: Explorer's window; a gone one reads as not visible.
                let shown = unsafe { IsWindowVisible(h) } != 0;
                let r = rect_of(h);
                format!("{} {} ({},{})-({},{})", class_of(h), if shown { "SHOWN" } else { "hidden" }, r.left, r.top, r.right, r.bottom)
            })
            .collect();
        if list.is_empty() {
            "no taskbar (Explorer down?)".into()
        } else {
            list.join("; ")
        }
    }

    fn state_now() -> String {
        let w = work_area();
        let m = primary();
        format!(
            "state={} (autohide {}), work area ({},{})-({},{}) of display ({},{})-({},{}); {}",
            appbar::taskbar_state(),
            appbar::taskbar_state() & appbar::ABS_AUTOHIDE != 0,
            w.left,
            w.top,
            w.right,
            w.bottom,
            m.left,
            m.top,
            m.right,
            m.bottom,
            taskbars_now()
        )
    }

    fn press(keys: &[u16]) {
        let key = |vk: u16, up: bool| Input {
            kind: 1,
            u: InputUnion { ki: KeybdInput { vk, scan: 0, flags: if vk == VK_LWIN { KEYEVENTF_EXTENDEDKEY } else { 0 } | if up { KEYEVENTF_KEYUP } else { 0 }, time: 0, extra: 0 } },
        };
        let inputs: Vec<Input> = keys.iter().map(|&k| key(k, false)).chain(keys.iter().rev().map(|&k| key(k, true))).collect();
        // SAFETY: a slice of INPUTs of the size passed.
        unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<Input>() as i32) };
    }

    // --- The bar ------------------------------------------------------------------------

    #[derive(Clone, Copy, PartialEq)]
    enum Button {
        Start,
        Search,
        TaskView,
        Widgets,
        Desktop,
        Quick,
        Notify,
        Quit,
    }

    impl Button {
        fn label(self) -> &'static str {
            match self {
                Button::Start => "开始",
                Button::Search => "搜索",
                Button::TaskView => "任务视图",
                Button::Widgets => "小组件",
                Button::Desktop => "桌面",
                Button::Quick => "快速设置",
                Button::Notify => "通知",
                Button::Quit => "还原并退出",
            }
        }

        fn keys(self) -> &'static [u16] {
            match self {
                Button::Start => &[VK_LWIN],
                Button::Search => &[VK_LWIN, b'S' as u16],
                Button::TaskView => &[VK_LWIN, VK_TAB],
                Button::Widgets => &[VK_LWIN, b'W' as u16],
                Button::Desktop => &[VK_LWIN, b'D' as u16],
                Button::Quick => &[VK_LWIN, b'A' as u16],
                Button::Notify => &[VK_LWIN, b'N' as u16],
                Button::Quit => &[],
            }
        }
    }

    const LEFT: [Button; 4] = [Button::Start, Button::Search, Button::TaskView, Button::Widgets];
    // From the right edge in.
    const RIGHT: [Button; 4] = [Button::Quit, Button::Notify, Button::Quick, Button::Desktop];

    // The buttons' places in the bar, and the room left between them.
    fn layout(client: Rect, dpi: u32) -> (Vec<(Rect, Button)>, Rect) {
        let px = |v: i32| v * dpi as i32 / 96;
        let (top, bottom) = (client.top + px(6), client.bottom - px(6));
        let mut out = Vec::new();
        let mut x = client.left + px(6);
        for b in LEFT {
            let w = px(if b == Button::TaskView { 84 } else { 64 });
            out.push((Rect { left: x, top, right: x + w, bottom }, b));
            x += w + px(4);
        }
        let mut r = client.right - px(6);
        for b in RIGHT {
            let w = px(match b {
                Button::Quit => 104,
                Button::Quick => 84,
                _ => 56,
            });
            out.push((Rect { left: r - w, top, right: r, bottom }, b));
            r -= w + px(4);
        }
        (out, Rect { left: x + px(8), top: client.top, right: r - px(8), bottom: client.bottom })
    }

    fn client_of(hwnd: Hwnd) -> Rect {
        let mut r = Rect::default();
        // SAFETY: our own struct.
        unsafe { GetClientRect(hwnd, &mut r) };
        r
    }

    fn paint(hwnd: Hwnd) {
        // SAFETY: GDI objects we make and delete within the paint; the PAINTSTRUCT is ours.
        unsafe {
            let mut ps: PaintStruct = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            let dpi = GetDpiForWindow(hwnd).max(96);
            let client = client_of(hwnd);
            let bg = CreateSolidBrush(0x0020_2020);
            let btn = CreateSolidBrush(0x0040_3a3a);
            let quit = CreateSolidBrush(0x0030_3080);
            FillRect(hdc, &client, bg);
            let font = CreateFontW(-(15 * dpi as i32 / 96), 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 5, 0, wide("Microsoft YaHei UI").as_ptr());
            let old = SelectObject(hdc, font);
            SetBkMode(hdc, 1);
            SetTextColor(hdc, 0x00ee_eeee);
            let (buttons, mut rest) = layout(client, dpi);
            for (r, b) in buttons {
                FillRect(hdc, &r, if b == Button::Quit { quit } else { btn });
                let mut r = r;
                DrawTextW(hdc, wide(b.label()).as_ptr(), -1, &mut r, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
            }
            let t = now();
            let status = format!(
                "{:02}:{:02}:{:02}  ·  原生任务栏{}  ·  冒出来 {} 次{}  ·  {}  ·  Ctrl+Alt+Shift+R 还原",
                t.hour,
                t.minute,
                t.second,
                if shell::visible() { "：显示中" } else { "已藏起" },
                SHOWN_AGAIN.load(Ordering::SeqCst),
                if FULLSCREEN.load(Ordering::SeqCst) { "  ·  有程序全屏" } else { "" },
                LAST.lock().unwrap()
            );
            SetTextColor(hdc, 0x00b0_b0b0);
            DrawTextW(hdc, wide(&status).as_ptr(), -1, &mut rest, DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);
            SelectObject(hdc, old);
            DeleteObject(font);
            DeleteObject(bg);
            DeleteObject(btn);
            DeleteObject(quit);
            EndPaint(hwnd, &ps);
        }
    }

    // The strip along the bottom of the main display, asked of Windows again
    // only when the display or the thickness change (or Windows says to).
    fn place(hwnd: Hwnd, force: bool) {
        let mon = primary();
        // SAFETY: our own window.
        let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
        let thickness = BAR_H * dpi as i32 / 96;
        {
            let mut placed = PLACED_FOR.lock().unwrap();
            if !force && *placed == Some((mon, thickness)) {
                return;
            }
            *placed = Some((mon, thickness));
        }
        let (x, y, w, h) = appbar::place(hwnd as isize, (mon.left, mon.top, mon.right - mon.left, mon.bottom - mon.top), appbar::Edge::Bottom, thickness);
        *GRANTED.lock().unwrap() = Some((x, y, w, h));
        let after = if FULLSCREEN.load(Ordering::SeqCst) { HWND_BOTTOM } else { HWND_TOPMOST };
        // SAFETY: our own window.
        unsafe { SetWindowPos(hwnd, after, x, y, w, h, SWP_NOACTIVATE | SWP_SHOWWINDOW) };
        log(&format!("bar placed at ({x},{y}) {w}x{h}; {}", state_now()));
    }

    // Another bar moved: is the strip Windows would grant still the one we hold?
    fn still_placed(hwnd: Hwnd) -> bool {
        let Some((mon, thickness)) = *PLACED_FOR.lock().unwrap() else { return false };
        let asked = appbar::query(hwnd as isize, (mon.left, mon.top, mon.right - mon.left, mon.bottom - mon.top), appbar::Edge::Bottom, thickness);
        *GRANTED.lock().unwrap() == Some(asked)
    }

    fn put_away_again(how: &str) {
        let n = SHOWN_AGAIN.fetch_add(1, Ordering::SeqCst) + 1;
        let fg = describe(unsafe { GetForegroundWindow() });
        note(&format!("taskbar shown again #{n} ({how}); foreground {fg}"));
        let t = Instant::now();
        shell::rehide();
        log(&format!("  put away in {} ms; {}", t.elapsed().as_millis(), state_now()));
    }

    fn give_back(why: &str) {
        if RESTORED.swap(true, Ordering::SeqCst) {
            return;
        }
        let bar = BAR.load(Ordering::SeqCst);
        if bar != 0 {
            appbar::remove(bar);
        }
        let ok = shell::restore(&file());
        log(&format!("given back ({why}): {ok}; {}", state_now()));
    }

    extern "system" fn on_event(_hook: isize, _event: u32, hwnd: Hwnd, object: i32, _child: i32, _thread: u32, _time: u32) {
        if object != OBJID_WINDOW || hwnd.is_null() {
            return;
        }
        let class = class_of(hwnd);
        if class == "Shell_TrayWnd" || class == "Shell_SecondaryTrayWnd" {
            // SAFETY: our own window; handled on its thread.
            unsafe { PostMessageW(BAR.load(Ordering::SeqCst) as Hwnd, WM_TASKBAR_SHOWN, 0, hwnd as isize) };
        }
    }

    extern "system" fn wndproc(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize {
        // SAFETY: calls about our own window, from its own thread.
        unsafe {
            match msg {
                WM_PAINT => {
                    paint(hwnd);
                    return 0;
                }
                WM_MOUSEACTIVATE => return MA_NOACTIVATE,
                WM_LBUTTONUP => {
                    let x = (lparam & 0xffff) as i16 as i32;
                    let (buttons, _) = layout(client_of(hwnd), GetDpiForWindow(hwnd).max(96));
                    if let Some((_, b)) = buttons.into_iter().find(|(r, _)| x >= r.left && x < r.right) {
                        if b == Button::Quit {
                            note("quit button");
                            DestroyWindow(hwnd);
                        } else {
                            note(&format!("pressed {}", b.label()));
                            press(b.keys());
                        }
                    }
                    return 0;
                }
                WM_TIMER => {
                    if shell::visible() {
                        put_away_again("seen by the timer");
                    }
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                    return 0;
                }
                WM_TASKBAR_SHOWN => {
                    if IsWindowVisible(lparam as Hwnd) != 0 {
                        put_away_again(&format!("shown event on {}", class_of(lparam as Hwnd)));
                    }
                    return 0;
                }
                WM_HOTKEY => {
                    note("hotkey");
                    DestroyWindow(hwnd);
                    return 0;
                }
                WM_DISPLAYCHANGE => {
                    note(&format!("display change {}x{}", lparam & 0xffff, (lparam >> 16) & 0xffff));
                    place(hwnd, false);
                    return 0;
                }
                WM_DPICHANGED => {
                    note(&format!("dpi change {}", wparam & 0xffff));
                    place(hwnd, false);
                    return 0;
                }
                WM_SETTINGCHANGE => {
                    let what = if lparam != 0 {
                        let p = lparam as *const u16;
                        let n = (0..256).take_while(|&i| *p.add(i) != 0).count();
                        String::from_utf16_lossy(std::slice::from_raw_parts(p, n))
                    } else {
                        String::new()
                    };
                    if wparam == SPI_SETWORKAREA {
                        log(&format!("setting change: work area; {}", state_now()));
                    } else {
                        log(&format!("setting change {wparam:#x} \"{what}\""));
                    }
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                }
                WM_POWERBROADCAST => {
                    let what = match wparam {
                        4 => "suspend".to_string(),
                        7 => "resume (by the user)".to_string(),
                        0x12 => "resume".to_string(),
                        other => format!("{other:#x}"),
                    };
                    note(&format!("power: {what}"));
                    log(&format!("  {}", state_now()));
                    return 1;
                }
                WM_QUERYENDSESSION => return 1,
                WM_ENDSESSION => {
                    if wparam != 0 {
                        give_back("Windows ending the session");
                    }
                    return 0;
                }
                WM_DESTROY => {
                    PostQuitMessage(0);
                    return 0;
                }
                _ => {}
            }
            if msg == CREATED_MSG.load(Ordering::SeqCst) && msg != 0 {
                note("Explorer (re)started: TaskbarCreated");
                log(&format!("  before: {}", state_now()));
                shell::rehide();
                appbar::register(hwnd as isize);
                place(hwnd, true);
                return 0;
            }
            if msg == appbar::CALLBACK {
                match wparam {
                    appbar::ABN_STATECHANGE => log(&format!("ABN_STATECHANGE; {}", state_now())),
                    appbar::ABN_POSCHANGED => {
                        if still_placed(hwnd) {
                            log("ABN_POSCHANGED: still ours");
                        } else {
                            log("ABN_POSCHANGED: moved");
                            place(hwnd, true);
                        }
                    }
                    appbar::ABN_FULLSCREENAPP => {
                        let full = lparam != 0;
                        FULLSCREEN.store(full, Ordering::SeqCst);
                        note(&format!("full screen {}: {}", if full { "on" } else { "off" }, describe(GetForegroundWindow())));
                        if full {
                            SetWindowPos(hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                            SetWindowPos(hwnd, HWND_BOTTOM, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                        } else {
                            SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                        }
                    }
                    other => log(&format!("appbar notification {other}")),
                }
                return 0;
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }

    fn run() {
        open_log("spike.log");
        log("=== start ===");
        log(&format!("before: {}", state_now()));
        if shell::recover(&file()) {
            log(&format!("an earlier run had left it put away: given back; {}", state_now()));
        }
        if file().exists() {
            log("another run has it put away: quitting");
            return;
        }
        // SAFETY: Win32 calls about our own class and window, from this thread, which runs its loop.
        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let class = wide(CLASS);
            let wc = WndClassExW {
                size: std::mem::size_of::<WndClassExW>() as u32,
                style: 0,
                wndproc,
                cls_extra: 0,
                wnd_extra: 0,
                instance,
                icon: 0,
                cursor: LoadCursorW(0, 32512),
                background: 0,
                menu_name: std::ptr::null(),
                class_name: class.as_ptr(),
                icon_sm: 0,
            };
            RegisterClassExW(&wc);
            let mon = primary();
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class.as_ptr(),
                wide("Wakuwaku taskbar spike").as_ptr(),
                WS_POPUP,
                mon.left,
                mon.bottom - BAR_H,
                mon.right - mon.left,
                BAR_H,
                std::ptr::null_mut(),
                0,
                instance,
                std::ptr::null_mut(),
            );
            if hwnd.is_null() {
                log("could not make the window");
                return;
            }
            BAR.store(hwnd as isize, Ordering::SeqCst);
            CREATED_MSG.store(shell::taskbar_created(), Ordering::SeqCst);

            if let Err(e) = shell::hide(&file()) {
                log(&format!("could not write {}: {e}", file().display()));
                DestroyWindow(hwnd);
                return;
            }
            log(&format!("put away; guard started: {}", shell::spawn_guard(&file())));
            let t = Instant::now();
            while work_area().bottom != mon.bottom && t.elapsed() < Duration::from_millis(3000) {
                std::thread::sleep(Duration::from_millis(20));
            }
            log(&format!("work area free after {} ms; {}", t.elapsed().as_millis(), state_now()));

            log(&format!("app bar registered: {}", appbar::register(hwnd as isize)));
            place(hwnd, true);
            ShowWindow(hwnd, SW_SHOWNA);
            let hotkey = RegisterHotKey(hwnd, 1, MOD_CONTROL | MOD_ALT | MOD_SHIFT | MOD_NOREPEAT, b'R' as u32) != 0;
            let hook = SetWinEventHook(EVENT_OBJECT_SHOW, EVENT_OBJECT_SHOW, 0, on_event, 0, 0, 0);
            SetTimer(hwnd, 1, 500, std::ptr::null());
            log(&format!("hotkey Ctrl+Alt+Shift+R: {hotkey}; show hook: {}", hook != 0));
            note("ready");

            let mut msg: Msg = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            if hook != 0 {
                UnhookWinEvent(hook);
            }
        }
        give_back("quitting");
        log("=== end ===");
    }

    // --- The probe -----------------------------------------------------------------------

    fn screenshot(name: &str) -> String {
        // SAFETY: GDI objects we make and delete; the pixels go into our own buffer.
        unsafe {
            let (x, y, w, h) = (GetSystemMetrics(76), GetSystemMetrics(77), GetSystemMetrics(78), GetSystemMetrics(79));
            let screen = GetDC(std::ptr::null_mut());
            let mem = CreateCompatibleDC(screen);
            let bmp = CreateCompatibleBitmap(screen, w, h);
            let old = SelectObject(mem, bmp);
            BitBlt(mem, 0, 0, w, h, screen, x, y, 0x00CC_0020);
            SelectObject(mem, old);
            let mut info = BitmapInfoHeader { size: 40, width: w, height: h, planes: 1, bit_count: 32, compression: 0, size_image: 0, x_ppm: 0, y_ppm: 0, clr_used: 0, clr_important: 0 };
            let mut pixels = vec![0u8; (w * h * 4) as usize];
            GetDIBits(mem, bmp, 0, h as u32, pixels.as_mut_ptr() as *mut c_void, &mut info, 0);
            DeleteObject(bmp);
            DeleteDC(mem);
            ReleaseDC(std::ptr::null_mut(), screen);
            let path = dir().join(format!("probe-{name}.bmp"));
            let mut out = Vec::with_capacity(54 + pixels.len());
            out.extend_from_slice(b"BM");
            out.extend_from_slice(&((54 + pixels.len()) as u32).to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes());
            out.extend_from_slice(&54u32.to_le_bytes());
            for v in [40u32, w as u32, h as u32] {
                out.extend_from_slice(&v.to_le_bytes());
            }
            out.extend_from_slice(&1u16.to_le_bytes());
            out.extend_from_slice(&32u16.to_le_bytes());
            for _ in 0..6 {
                out.extend_from_slice(&0u32.to_le_bytes());
            }
            out.extend_from_slice(&pixels);
            let _ = std::fs::write(&path, out);
            path.display().to_string()
        }
    }

    // Watches for this long: when the taskbar was shown, and what was in front at the end.
    fn watch(ms: u64, shot: &str) -> (Vec<String>, String) {
        let t = Instant::now();
        let mut was = shell::visible();
        let mut changes = Vec::new();
        let mut front = String::new();
        let mut shot_taken = false;
        while t.elapsed() < Duration::from_millis(ms) {
            let is = shell::visible();
            if is != was {
                changes.push(format!("{} at {} ms", if is { "SHOWN" } else { "hidden" }, t.elapsed().as_millis()));
                was = is;
            }
            if !shot_taken && t.elapsed() >= Duration::from_millis(ms * 3 / 4) {
                // SAFETY: a plain query.
                front = describe(unsafe { GetForegroundWindow() });
                log(&format!("  screenshot {}", screenshot(shot)));
                shot_taken = true;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        (changes, front)
    }

    fn probe() {
        open_log("probe.log");
        log("=== probe ===");
        // SAFETY: a plain lookup.
        let bar = unsafe { FindWindowW(wide(CLASS).as_ptr(), std::ptr::null()) };
        let mon = primary();
        if bar.is_null() {
            log("no bar running: the probe runs anyway, against the taskbar as it is");
        } else {
            let r = rect_of(bar);
            log(&format!("bar at ({},{})-({},{})", r.left, r.top, r.right, r.bottom));
        }
        log(&format!("before: {}", state_now()));
        let cases: [(&str, &str, &[u16], u64); 6] = [
            ("start", "Win (Start)", &[VK_LWIN], 1600),
            ("search", "Win+S (search)", &[VK_LWIN, b'S' as u16], 1600),
            ("quick", "Win+A (quick settings)", &[VK_LWIN, b'A' as u16], 1600),
            ("notify", "Win+N (notifications, calendar)", &[VK_LWIN, b'N' as u16], 1600),
            ("taskview", "Win+Tab (task view)", &[VK_LWIN, VK_TAB], 1600),
            ("widgets", "Win+W (widgets)", &[VK_LWIN, b'W' as u16], 2000),
        ];
        for (name, what, keys, ms) in cases {
            log(&format!("-- {what}"));
            press(keys);
            let (changes, front) = watch(ms, name);
            log(&format!("  in front: {front}"));
            log(&format!("  taskbar: {}", if changes.is_empty() { "stayed as it was".to_string() } else { changes.join(", ") }));
            press(&[VK_ESCAPE]);
            std::thread::sleep(Duration::from_millis(700));
            log(&format!("  after Esc: taskbar {}", if shell::visible() { "SHOWN" } else { "hidden" }));
        }
        log("-- the pointer at the bottom edge");
        let mut at = Point::default();
        // SAFETY: our own out-parameter; the pointer is put back after.
        unsafe {
            GetCursorPos(&mut at);
            SetCursorPos((mon.left + mon.right) / 2, mon.bottom - 1);
        }
        let (changes, front) = watch(1600, "edge");
        unsafe { SetCursorPos(at.x, at.y) };
        log(&format!("  in front: {front}"));
        log(&format!("  taskbar: {}", if changes.is_empty() { "stayed as it was".to_string() } else { changes.join(", ") }));
        log(&format!("after: {}", state_now()));
        log("=== probe done ===");
    }

    pub fn main() {
        // SAFETY: before any window; the process's own setting.
        unsafe { SetProcessDpiAwarenessContext(PER_MONITOR_V2) };
        let args: Vec<String> = std::env::args().collect();
        if let Some((pid, file)) = shell::guard_args(&args) {
            open_log("spike.log");
            log(&format!("guard: watching {pid}"));
            shell::guard(pid, file);
            log(&format!("guard: done; {}", state_now()));
            return;
        }
        match args.get(1).map(String::as_str) {
            Some("--probe") => probe(),
            Some("--quit") => {
                // SAFETY: a plain lookup; WM_CLOSE ends it as its own button does.
                let bar = unsafe { FindWindowW(wide(CLASS).as_ptr(), std::ptr::null()) };
                println!("{}", if bar.is_null() { "none running" } else { "asked to quit" });
                if !bar.is_null() {
                    unsafe { PostMessageW(bar, 0x0010, 0, 0) };
                }
            }
            Some("--status") => {
                println!("{}", state_now());
                // SAFETY: a plain lookup.
                let bar = unsafe { FindWindowW(wide(CLASS).as_ptr(), std::ptr::null()) };
                if !bar.is_null() {
                    let r = rect_of(bar);
                    println!("bar at ({},{})-({},{})", r.left, r.top, r.right, r.bottom);
                }
                println!("put away (file): {}", file().exists());
            }
            Some("--restore") => {
                open_log("spike.log");
                if shell::restore(&file()) {
                    log(&format!("--restore: given back; {}", state_now()));
                } else {
                    // Nothing written down: only shown again, its state left alone.
                    for h in shell::taskbars() {
                        // SAFETY: Explorer's windows.
                        unsafe { ShowWindow(h as Hwnd, SW_SHOWNA) };
                    }
                    log(&format!("--restore: nothing written down; shown; {}", state_now()));
                }
            }
            _ => run(),
        }
    }
}
