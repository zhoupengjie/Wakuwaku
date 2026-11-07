// Can her capsule live inside Windows' own taskbar, as TrafficMonitor's
// window does (zhoupengjie/Wakuwaku#105: the island embedded in the taskbar,
// Windows' taskbar left to Start11 and the like)? A WebView2 window of ours
// (Tauri; a page of its own, a dark pill) made a child of Explorer's
// Shell_TrayWnd with SetParent and placed in the strip before the tray
// (TrayNotifyWnd), or at its left end. What happens is written to stdout:
// whether it shows, where, at what DPI, whether it goes with the taskbar
// (auto-hide), whether the page gets the pointer (hover, click: the page
// tells a server of ours), and whether it is still there at the end.
//
//   embed_spike [--seconds N] [--left] [--poke] [--opaque] [--popup] [--layered]
//       run N seconds (default 60); at the strip's left end instead of before
//       the tray; --poke: 10 s in, move the pointer onto the pill, click
//       it, and put the pointer back, so the page's events are seen without
//       a hand on the mouse; --opaque: a window that is not see-through;
//       --popup: into the taskbar with its popup style kept, not made
//       WS_CHILD, as TrafficMonitor's window goes on Windows 11; --layered:
//       made WS_EX_LAYERED (alpha 255) first, as TrafficMonitor's is
//
// Before it goes into the taskbar it is shown on its own just above it
// until its page says it is up, and 2.5 s more: what it draws there is
// what it should draw in the taskbar.
//
// Windows' own taskbar must be shown (her taskbar home puts it away, and
// makes a Shell_TrayWnd of its own: only Explorer's is looked for).
//
// A plain window of ours (WakuwakuEmbedProbe, painted with GDI) goes in
// beside the pill too, as TrafficMonitor's does, and is clicked after it.
// What it showed on Windows 11 (2026-11-07), in docs/DEVELOPMENT.md, the
// island's "嵌进 Windows 的任务栏": made layered first (--layered), its popup
// style kept (--popup), as TrafficMonitor's, both show and take the pointer;
// not layered, neither takes it, and the plain one does not even show.
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
    use std::io::Write;
    use std::sync::OnceLock;
    use std::time::{Duration, Instant};

    use tauri::{WebviewUrl, WebviewWindowBuilder};

    type Hwnd = isize;

    #[repr(C)]
    #[derive(Default, Clone, Copy, Debug)]
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
    struct MouseInput {
        dx: i32,
        dy: i32,
        data: u32,
        flags: u32,
        time: u32,
        extra: usize,
    }

    #[repr(C)]
    struct Input {
        kind: u32,
        mi: MouseInput,
    }

    #[link(name = "user32")]
    extern "system" {
        fn FindWindowExW(parent: Hwnd, after: Hwnd, class: *const u16, title: *const u16) -> Hwnd;
        fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
        fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
        fn SetParent(child: Hwnd, parent: Hwnd) -> Hwnd;
        fn GetParent(hwnd: Hwnd) -> Hwnd;
        fn GetAncestor(hwnd: Hwnd, flags: u32) -> Hwnd;
        fn GetWindowLongPtrW(hwnd: Hwnd, index: i32) -> isize;
        fn SetWindowLongPtrW(hwnd: Hwnd, index: i32, value: isize) -> isize;
        fn SetWindowPos(hwnd: Hwnd, after: isize, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
        fn IsWindowVisible(hwnd: Hwnd) -> i32;
        fn IsWindow(hwnd: Hwnd) -> i32;
        fn GetDpiForWindow(hwnd: Hwnd) -> u32;
        fn ScreenToClient(hwnd: Hwnd, pt: *mut Point) -> i32;
        fn GetCursorPos(pt: *mut Point) -> i32;
        fn SetCursorPos(x: i32, y: i32) -> i32;
        fn SendInput(count: u32, inputs: *const Input, size: i32) -> u32;
        fn RegisterClassExW(class: *const WndClassExW) -> u16;
        fn CreateWindowExW(ex: u32, class: *const u16, title: *const u16, style: u32, x: i32, y: i32, w: i32, h: i32, parent: Hwnd, menu: isize, instance: isize, param: *mut std::ffi::c_void) -> Hwnd;
        fn DefWindowProcW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize;
        fn BeginPaint(hwnd: Hwnd, ps: *mut PaintStruct) -> isize;
        fn EndPaint(hwnd: Hwnd, ps: *const PaintStruct) -> i32;
        fn FillRect(hdc: isize, rect: *const Rect, brush: isize) -> i32;
        fn GetClientRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
        fn InvalidateRect(hwnd: Hwnd, rect: *const Rect, erase: i32) -> i32;
        fn DestroyWindow(hwnd: Hwnd) -> i32;
        fn GetWindow(hwnd: Hwnd, cmd: u32) -> Hwnd;
        fn SetLayeredWindowAttributes(hwnd: Hwnd, key: u32, alpha: u8, flags: u32) -> i32;
        fn WindowFromPoint(pt: Point) -> Hwnd;
        fn GetClassNameW(hwnd: Hwnd, name: *mut u16, max: i32) -> i32;
        fn IsChild(parent: Hwnd, hwnd: Hwnd) -> i32;
    }

    #[repr(C)]
    struct WndClassExW {
        size: u32,
        style: u32,
        proc_: extern "system" fn(Hwnd, u32, usize, isize) -> isize,
        cls_extra: i32,
        wnd_extra: i32,
        instance: isize,
        icon: isize,
        cursor: isize,
        background: isize,
        menu: *const u16,
        class: *const u16,
        icon_sm: isize,
    }

    #[repr(C)]
    struct PaintStruct {
        hdc: isize,
        erase: i32,
        paint: Rect,
        restore: i32,
        inc_update: i32,
        reserved: [u8; 32],
    }

    #[link(name = "gdi32")]
    extern "system" {
        fn CreateSolidBrush(color: u32) -> isize;
        fn DeleteObject(obj: isize) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleW(name: *const u16) -> isize;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
        fn QueryFullProcessImageNameW(process: isize, flags: u32, name: *mut u16, size: *mut u32) -> i32;
        fn CloseHandle(handle: isize) -> i32;
    }

    const GWL_STYLE: i32 = -16;
    const GWL_EXSTYLE: i32 = -20;
    const WS_CHILD: isize = 0x4000_0000;
    const WS_POPUP: isize = 0x8000_0000;
    const SWP_NOSIZE: u32 = 0x1;
    const SWP_NOMOVE: u32 = 0x2;
    const SWP_NOACTIVATE: u32 = 0x10;
    const SWP_FRAMECHANGED: u32 = 0x20;
    const SWP_SHOWWINDOW: u32 = 0x40;
    const GA_ROOT: u32 = 2;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const MOUSEEVENTF_LEFTDOWN: u32 = 0x2;
    const MOUSEEVENTF_LEFTUP: u32 = 0x4;

    // The pill's size (logical px), as her compact island is.
    const PILL: (f64, f64) = (300.0, 36.0);

    static START: OnceLock<Instant> = OnceLock::new();

    fn say(line: &str) {
        let t = START.get_or_init(Instant::now).elapsed().as_secs_f64();
        let out = std::io::stdout();
        let mut out = out.lock();
        let _ = writeln!(out, "[{t:6.2}] {line}");
        let _ = out.flush();
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn rect(hwnd: Hwnd) -> Rect {
        let mut r = Rect::default();
        // SAFETY: a plain call with a rect of ours.
        unsafe { GetWindowRect(hwnd, &mut r) };
        r
    }

    fn shown(r: &Rect) -> String {
        format!("({}, {}) {}x{}", r.left, r.top, r.right - r.left, r.bottom - r.top)
    }

    // The exe behind a window, by its process.
    fn exe_of(hwnd: Hwnd) -> (u32, String) {
        let mut pid = 0u32;
        // SAFETY: plain calls; the buffer is ours and its length told.
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut pid);
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process == 0 {
                return (pid, String::from("?"));
            }
            let mut name = vec![0u16; 1024];
            let mut len = name.len() as u32;
            let ok = QueryFullProcessImageNameW(process, 0, name.as_mut_ptr(), &mut len);
            CloseHandle(process);
            if ok == 0 {
                return (pid, String::from("?"));
            }
            let path = String::from_utf16_lossy(&name[..len as usize]);
            (pid, path.rsplit('\\').next().unwrap_or(&path).to_lowercase())
        }
    }

    // Explorer's taskbar: every Shell_TrayWnd told (hers is one too while
    // her taskbar is up), the one of explorer.exe taken.
    fn explorer_tray() -> Option<Hwnd> {
        let class = wide("Shell_TrayWnd");
        let mut found = None;
        let mut prev = 0;
        loop {
            // SAFETY: plain calls with a class name of ours.
            let hwnd = unsafe { FindWindowExW(0, prev, class.as_ptr(), std::ptr::null()) };
            if hwnd == 0 {
                break;
            }
            let (pid, exe) = exe_of(hwnd);
            let visible = unsafe { IsWindowVisible(hwnd) } != 0;
            say(&format!("Shell_TrayWnd {hwnd:#x}: {exe} (pid {pid}), {}, at {}", if visible { "shown" } else { "hidden" }, shown(&rect(hwnd))));
            if exe == "explorer.exe" && (found.is_none() || visible) {
                found = Some(hwnd);
            }
            prev = hwnd;
        }
        found
    }

    fn class_of(hwnd: Hwnd) -> String {
        let mut name = [0u16; 256];
        // SAFETY: a plain call; the buffer is ours and its length told.
        let n = unsafe { GetClassNameW(hwnd, name.as_mut_ptr(), name.len() as i32) };
        String::from_utf16_lossy(&name[..n.max(0) as usize])
    }

    // The taskbar's children, top first (z order), each with its class,
    // place and process.
    fn children(tray: Hwnd, ours: Hwnd) {
        const GW_HWNDNEXT: u32 = 2;
        const GW_CHILD: u32 = 5;
        // SAFETY: plain calls walking windows that exist.
        let mut w = unsafe { GetWindow(tray, GW_CHILD) };
        let mut i = 0;
        while w != 0 && i < 40 {
            let (pid, exe) = exe_of(w);
            let visible = unsafe { IsWindowVisible(w) } != 0;
            say(&format!(
                "  child {i}: {w:#x} {}{} at {} ({exe} {pid}, {})",
                class_of(w),
                if w == ours { " (OURS)" } else { "" },
                shown(&rect(w)),
                if visible { "shown" } else { "hidden" }
            ));
            w = unsafe { GetWindow(w, GW_HWNDNEXT) };
            i += 1;
        }
    }

    // Which window the pointer would be over at the pill's middle: ours (or
    // one of our page's), or another.
    fn hit(ours: Hwnd) -> String {
        let r = rect(ours);
        let pt = Point { x: (r.left + r.right) / 2, y: (r.top + r.bottom) / 2 };
        // SAFETY: plain calls.
        let w = unsafe { WindowFromPoint(pt) };
        let mine = w == ours || unsafe { IsChild(ours, w) } != 0;
        let (pid, exe) = exe_of(w);
        format!("at ({}, {}) the window {w:#x} {} ({exe} {pid}): {}", pt.x, pt.y, class_of(w), if mine { "OURS" } else { "NOT ours" })
    }

    // A plain window of ours beside the pill, as TrafficMonitor's is: no
    // page, painted orange, green once clicked, telling of the mouse it
    // gets. Made on the main thread (tao's loop dispatches its messages).
    static PROBE_HIT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    static PROBE_MOVED: std::sync::Mutex<Option<Instant>> = std::sync::Mutex::new(None);

    extern "system" fn probe_proc(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize {
        const WM_PAINT: u32 = 0x0F;
        const WM_MOUSEMOVE: u32 = 0x200;
        const WM_LBUTTONDOWN: u32 = 0x201;
        const WM_LBUTTONUP: u32 = 0x202;
        const WM_MOUSEACTIVATE: u32 = 0x21;
        const WM_POINTERDOWN: u32 = 0x246;
        const WM_POINTERUPDATE: u32 = 0x245;
        match msg {
            WM_MOUSEMOVE | WM_POINTERUPDATE => {
                let mut last = PROBE_MOVED.lock().unwrap();
                if last.is_none_or(|t| t.elapsed() > Duration::from_millis(500)) {
                    say(&format!("probe: {} at ({}, {})", if msg == WM_MOUSEMOVE { "WM_MOUSEMOVE" } else { "WM_POINTERUPDATE" }, lparam as i16, (lparam >> 16) as i16));
                }
                *last = Some(Instant::now());
            }
            WM_LBUTTONDOWN | WM_POINTERDOWN => {
                say(&format!("probe: {}", if msg == WM_LBUTTONDOWN { "WM_LBUTTONDOWN" } else { "WM_POINTERDOWN" }));
                PROBE_HIT.store(true, std::sync::atomic::Ordering::SeqCst);
                // SAFETY: a plain call on our own window.
                unsafe { InvalidateRect(hwnd, std::ptr::null(), 1) };
            }
            WM_LBUTTONUP => say("probe: WM_LBUTTONUP"),
            WM_MOUSEACTIVATE => {
                say("probe: WM_MOUSEACTIVATE");
                // MA_NOACTIVATE: clicked without taking the taskbar's focus.
                return 3;
            }
            WM_PAINT => {
                // SAFETY: the paint struct is ours; the brush made and freed here.
                unsafe {
                    let mut ps: PaintStruct = std::mem::zeroed();
                    let hdc = BeginPaint(hwnd, &mut ps);
                    let mut r = Rect::default();
                    GetClientRect(hwnd, &mut r);
                    // COLORREF is 0x00BBGGRR: orange, or green once clicked.
                    let brush = CreateSolidBrush(if PROBE_HIT.load(std::sync::atomic::Ordering::SeqCst) { 0x0040C040 } else { 0x00208CFF });
                    FillRect(hdc, &r, brush);
                    DeleteObject(brush);
                    EndPaint(hwnd, &ps);
                }
                return 0;
            }
            _ => {}
        }
        // SAFETY: the default for the rest.
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    fn make_probe() -> Hwnd {
        const WS_POPUP_U: u32 = 0x8000_0000;
        const WS_EX_TOOLWINDOW: u32 = 0x80;
        const WS_EX_NOACTIVATE: u32 = 0x0800_0000;
        let class = wide("WakuwakuEmbedProbe");
        // SAFETY: plain calls; the class struct and names live through them.
        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let wc = WndClassExW {
                size: std::mem::size_of::<WndClassExW>() as u32,
                style: 0,
                proc_: probe_proc,
                cls_extra: 0,
                wnd_extra: 0,
                instance,
                icon: 0,
                cursor: 0,
                background: 0,
                menu: std::ptr::null(),
                class: class.as_ptr(),
                icon_sm: 0,
            };
            RegisterClassExW(&wc);
            CreateWindowExW(WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE, class.as_ptr(), class.as_ptr(), WS_POPUP_U, 0, 0, 10, 10, 0, 0, instance, std::ptr::null_mut())
        }
    }

    // A window of ours made a child of the taskbar at a place on the screen:
    // WS_CHILD, or (popup) its popup style kept, as TrafficMonitor's is.
    fn embed(hwnd: Hwnd, tray: Hwnd, x: i32, y: i32, w: i32, h: i32, popup: bool, layered: bool) {
        // SAFETY: plain calls on our own window; the parent one that exists.
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
            if !popup {
                SetWindowLongPtrW(hwnd, GWL_STYLE, (style & !WS_POPUP) | WS_CHILD);
            }
            if layered {
                make_layered(hwnd);
            }
            SetParent(hwnd, tray);
            let mut pt = Point { x, y };
            ScreenToClient(tray, &mut pt);
            SetWindowPos(hwnd, 0, pt.x, pt.y, w, h, SWP_SHOWWINDOW | SWP_FRAMECHANGED | SWP_NOACTIVATE);
        }
    }

    // Layered, wholly opaque (alpha 255), as TrafficMonitor's window is
    // layered (it with a colour key) before it goes into the taskbar.
    fn make_layered(hwnd: Hwnd) {
        const WS_EX_LAYERED: isize = 0x80000;
        const LWA_ALPHA: u32 = 2;
        // SAFETY: plain calls on our own window.
        unsafe {
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | WS_EX_LAYERED);
            let ok = SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);
            say(&format!("{hwnd:#x} made layered (alpha 255): {ok}; ex now {:#x}", GetWindowLongPtrW(hwnd, GWL_EXSTYLE)));
        }
    }

    fn child(parent: Hwnd, class: &str) -> Hwnd {
        let class = wide(class);
        // SAFETY: a plain call with a class name of ours.
        unsafe { FindWindowExW(parent, 0, class.as_ptr(), std::ptr::null()) }
    }

    // What the page is, served by our server at /page (about:blank with an
    // initialization script ran none of it): a dark pill filling the
    // window, lit while hovered, blue once clicked, telling the server of
    // each; it says the time it has been up every second, so a picture of
    // it shows whether it still draws.
    const PAGE: &str = r#"<!doctype html><html><head><meta charset="utf-8"><style>
html,body{margin:0;height:100%;background:transparent;overflow:hidden}
#p{position:absolute;inset:0;border-radius:18px;background:#000;color:#fff;font:14px "Segoe UI",sans-serif;display:flex;align-items:center;justify-content:center;user-select:none;cursor:default;box-shadow:inset 0 0 0 1px rgba(255,255,255,.25)}
#p.hot{background:#3a3a3a}#p.hit{background:#1e6fd9}
</style></head><body><div id="p">wakuwaku · embed spike · <span id="t">0</span> s</div><script>
const p = document.getElementById('p'), t = document.getElementById('t'), t0 = Date.now()
const ping = what => fetch('/' + what).catch(() => {})
p.addEventListener('pointerenter', () => { p.classList.add('hot'); ping('hover') })
p.addEventListener('pointerleave', () => { p.classList.remove('hot'); ping('leave') })
p.addEventListener('pointerdown', e => ping('down?x=' + e.screenX + '&y=' + e.screenY))
p.addEventListener('click', e => { p.classList.add('hit'); ping('click?x=' + e.screenX + '&y=' + e.screenY) })
setInterval(() => { t.textContent = Math.round((Date.now() - t0) / 1000) }, 1000)
ping('up?dpr=' + devicePixelRatio + '&w=' + innerWidth + '&h=' + innerHeight)
</script></body></html>"#;

    // The page has said it is up.
    static UP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

    struct Args {
        seconds: u64,
        left: bool,
        poke: bool,
        opaque: bool,
        raise: bool,
        popup: bool,
        layered: bool,
    }

    fn args() -> Args {
        let all: Vec<String> = std::env::args().collect();
        let mut args = Args { seconds: 60, left: false, poke: false, opaque: false, raise: false, popup: false, layered: false };
        let mut i = 1;
        while i < all.len() {
            match all[i].as_str() {
                "--seconds" => {
                    args.seconds = all.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(60);
                    i += 1;
                }
                "--left" => args.left = true,
                "--poke" => args.poke = true,
                "--opaque" => args.opaque = true,
                "--raise" => args.raise = true,
                "--popup" => args.popup = true,
                "--layered" => args.layered = true,
                other => say(&format!("unknown argument {other}")),
            }
            i += 1;
        }
        args
    }

    pub fn main() {
        START.get_or_init(Instant::now);
        let args = args();
        // The page's server: what it says of the pointer.
        let server = tiny_http::Server::http(("127.0.0.1", 0)).expect("a port");
        let port = server.server_addr().to_ip().expect("ip").port();
        std::thread::spawn(move || {
            for req in server.incoming_requests() {
                let url = req.url().to_string();
                if url == "/page" {
                    let header = tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap();
                    let _ = req.respond(tiny_http::Response::from_string(PAGE).with_header(header));
                    say("page: loaded");
                    continue;
                }
                if url.starts_with("/up") {
                    UP.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                if url != "/favicon.ico" {
                    say(&format!("page: {url}"));
                }
                let _ = req.respond(tiny_http::Response::from_string("ok"));
            }
        });
        say(&format!("page server on 127.0.0.1:{port}"));

        tauri::Builder::default()
            .setup(move |app| {
                let url = format!("http://127.0.0.1:{port}/page");
                // Its own WebView2 data folder: not the installed copy's.
                let data = std::env::temp_dir().join("wakuwaku-embed-spike");
                let win = WebviewWindowBuilder::new(app, "cap", WebviewUrl::External(url.parse().unwrap()))
                    .title("wakuwaku embed spike")
                    .decorations(false)
                    .transparent(!args.opaque)
                    .shadow(false)
                    .skip_taskbar(true)
                    .always_on_top(true)
                    .resizable(false)
                    .visible(false)
                    .inner_size(PILL.0, PILL.1)
                    .position(100.0, 100.0)
                    .data_directory(data)
                    .build()?;
                let hwnd = win.hwnd()?.0 as isize;
                say(&format!("our window {hwnd:#x} made, at {}", shown(&rect(hwnd))));
                let handle = app.handle().clone();
                std::thread::spawn(move || run(handle, hwnd, args));
                Ok(())
            })
            .run(tauri::generate_context!())
            .expect("tauri");
    }

    fn run(app: tauri::AppHandle, hwnd: Hwnd, args: Args) {
        let Some(tray) = explorer_tray() else {
            say("no Shell_TrayWnd of explorer.exe: is Windows' taskbar back?");
            app.exit(2);
            return;
        };
        // First a window of its own just above the taskbar, kept on top, to
        // see the page draws at all; in the taskbar once it has said it is
        // up and a moment more.
        {
            let tr = rect(tray);
            // SAFETY: a plain call on a window that exists.
            let scale = unsafe { GetDpiForWindow(tray) } as f64 / 96.0;
            let (w, h) = ((PILL.0 * scale).round() as i32, (PILL.1 * scale).round() as i32);
            let (x, y) = (tr.left + (tr.right - tr.left - w) / 2, tr.top - h - (12.0 * scale).round() as i32);
            const HWND_TOPMOST: isize = -1;
            // SAFETY: a plain call on our own window.
            unsafe { SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, SWP_SHOWWINDOW | SWP_NOACTIVATE) };
            say(&format!("shown on its own above the taskbar at ({x}, {y}) {w}x{h}"));
        }
        let waited = Instant::now();
        while !UP.load(std::sync::atomic::Ordering::SeqCst) && waited.elapsed() < Duration::from_secs(8) {
            std::thread::sleep(Duration::from_millis(100));
        }
        say(&format!("page {} after {:.1} s", if UP.load(std::sync::atomic::Ordering::SeqCst) { "up" } else { "NOT up" }, waited.elapsed().as_secs_f64()));
        std::thread::sleep(Duration::from_millis(800));
        if args.poke {
            // The same click on it on its own first: whether the page hears
            // such a click at all, before the taskbar is in the way or not.
            say(&format!("on its own, hit test: {}", hit(hwnd)));
            poke(hwnd);
            std::thread::sleep(Duration::from_millis(600));
            say("on its own, poked: the page's lines above, if any");
        }
        std::thread::sleep(Duration::from_millis(1700));
        // SAFETY: plain calls on windows that exist.
        let (tray_dpi, our_dpi) = unsafe { (GetDpiForWindow(tray), GetDpiForWindow(hwnd)) };
        let tr = rect(tray);
        let notify = child(tray, "TrayNotifyWnd");
        let nr = rect(notify);
        say(&format!("Explorer's taskbar {tray:#x} at {} (dpi {tray_dpi}); TrayNotifyWnd {notify:#x} at {}; our dpi {our_dpi}", shown(&tr), shown(&nr)));
        let scale = tray_dpi as f64 / 96.0;
        let (w, h) = ((PILL.0 * scale).round() as i32, (PILL.1 * scale).round() as i32);
        let gap = (8.0 * scale).round() as i32;
        let x = if args.left || notify == 0 { tr.left + gap } else { nr.left - gap - w };
        let y = tr.top + (tr.bottom - tr.top - h) / 2;
        say(&format!("wanted on the screen: ({x}, {y}) {w}x{h}"));

        // Into the taskbar: a child of it, at that place in its client area.
        // SAFETY: plain calls on our own window; the parent a window that exists.
        let (style, ex) = unsafe { (GetWindowLongPtrW(hwnd, GWL_STYLE), GetWindowLongPtrW(hwnd, GWL_EXSTYLE)) };
        say(&format!("our style {style:#x}, ex {ex:#x} before"));
        unsafe {
            // No longer kept on top: a child is in its parent's order.
            const HWND_NOTOPMOST: isize = -2;
            SetWindowPos(hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            if !args.popup {
                SetWindowLongPtrW(hwnd, GWL_STYLE, (style & !WS_POPUP) | WS_CHILD);
            }
            if args.layered {
                make_layered(hwnd);
            }
            let old = SetParent(hwnd, tray);
            say(&format!(
                "SetParent ({}) -> old parent {old:#x}, now {:#x} (tray {tray:#x}), root {:#x}; style {:#x}, ex {:#x}",
                if args.popup { "popup style kept, as TrafficMonitor" } else { "made WS_CHILD" },
                GetParent(hwnd),
                GetAncestor(hwnd, GA_ROOT),
                GetWindowLongPtrW(hwnd, GWL_STYLE),
                GetWindowLongPtrW(hwnd, GWL_EXSTYLE)
            ));
            let mut pt = Point { x, y };
            ScreenToClient(tray, &mut pt);
            let ok = SetWindowPos(hwnd, 0, pt.x, pt.y, w, h, SWP_SHOWWINDOW | SWP_FRAMECHANGED | SWP_NOACTIVATE);
            say(&format!("SetWindowPos in the taskbar's client at ({}, {}) {w}x{h}: {ok}", pt.x, pt.y));
        }
        // The plain window beside it, made on the main thread.
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = app.run_on_main_thread(move || {
            let _ = tx.send(make_probe());
        });
        let probe = rx.recv_timeout(Duration::from_secs(3)).unwrap_or(0);
        if probe != 0 {
            // Left of Start, where the taskbar is empty while its icons are
            // centred (not over a program's button: a click there is its).
            let pw = (60.0 * scale).round() as i32;
            let start = child(tray, "Start");
            let sr = rect(start);
            let px = if start != 0 && sr.left > tr.left + 4 * gap + pw { sr.left - 4 * gap - pw } else { x - gap - pw };
            embed(probe, tray, px, y, pw, h, args.popup, args.layered);
            say(&format!("probe {probe:#x} (a plain window of ours) in the taskbar at {}; hit test: {}", shown(&rect(probe)), hit(probe)));
        } else {
            say("no probe made");
        }
        std::thread::sleep(Duration::from_millis(300));
        report(hwnd, tray);
        say("the taskbar's children, top first:");
        children(tray, hwnd);
        say(&format!("hit test: {}", hit(hwnd)));
        if args.raise {
            // SAFETY: a plain call on our own window.
            unsafe { SetWindowPos(hwnd, 0, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
            say(&format!("raised to the top of its siblings; hit test: {}", hit(hwnd)));
        }

        let until = Instant::now() + Duration::from_secs(args.seconds);
        let mut poked = false;
        let mut last = Instant::now();
        while Instant::now() < until {
            std::thread::sleep(Duration::from_millis(200));
            // SAFETY: a plain call.
            if unsafe { IsWindow(hwnd) } == 0 {
                say("our window is gone (destroyed with the taskbar?)");
                break;
            }
            if args.poke && !poked && START.get().unwrap().elapsed() >= Duration::from_secs(10) {
                poked = true;
                poke(hwnd);
                if probe != 0 {
                    say("now the probe:");
                    poke(probe);
                    say("probe poked");
                }
            }
            if last.elapsed() >= Duration::from_secs(2) {
                last = Instant::now();
                report(hwnd, tray);
                say(&format!("hit test: {}", hit(hwnd)));
            }
        }

        // Out of the taskbar again, as it was, then gone.
        // SAFETY: plain calls on our own windows.
        unsafe {
            if probe != 0 && IsWindow(probe) != 0 {
                SetParent(probe, 0);
                let _ = app.run_on_main_thread(move || {
                    DestroyWindow(probe);
                });
            }
            if IsWindow(hwnd) != 0 {
                if !args.popup {
                    SetWindowLongPtrW(hwnd, GWL_STYLE, (style & !WS_CHILD) | WS_POPUP);
                }
                SetParent(hwnd, 0);
                say("taken out of the taskbar");
            }
        }
        app.exit(0);
    }

    fn report(hwnd: Hwnd, tray: Hwnd) {
        // SAFETY: plain calls on windows that exist (or not: IsWindow says).
        unsafe {
            let (visible, parent, root, dpi) = (IsWindowVisible(hwnd) != 0, GetParent(hwnd), GetAncestor(hwnd, GA_ROOT), GetDpiForWindow(hwnd));
            say(&format!(
                "ours at {} ({}, parent {}, root {}, dpi {dpi}); taskbar at {} ({})",
                shown(&rect(hwnd)),
                if visible { "shown" } else { "hidden" },
                if parent == tray { "the taskbar" } else { "not the taskbar" },
                if root == tray { "the taskbar" } else { "not the taskbar" },
                shown(&rect(tray)),
                if IsWindowVisible(tray) != 0 { "shown" } else { "hidden" }
            ));
        }
    }

    // The pointer onto the pill's middle, a click, and back where it was.
    fn poke(hwnd: Hwnd) {
        let r = rect(hwnd);
        let (cx, cy) = ((r.left + r.right) / 2, (r.top + r.bottom) / 2);
        let mut was = Point::default();
        // SAFETY: plain calls; the inputs are ours and their size told.
        unsafe {
            GetCursorPos(&mut was);
            say(&format!("poke: pointer from ({}, {}) to the pill's middle ({cx}, {cy})", was.x, was.y));
            SetCursorPos(cx, cy);
            std::thread::sleep(Duration::from_millis(900));
            let click = |flags: u32| Input { kind: 0, mi: MouseInput { dx: 0, dy: 0, data: 0, flags, time: 0, extra: 0 } };
            let inputs = [click(MOUSEEVENTF_LEFTDOWN), click(MOUSEEVENTF_LEFTUP)];
            let sent = SendInput(2, inputs.as_ptr(), std::mem::size_of::<Input>() as i32);
            say(&format!("poke: click sent ({sent} of 2)"));
            std::thread::sleep(Duration::from_millis(900));
            SetCursorPos(was.x, was.y);
            say("poke: pointer put back");
        }
    }
}
