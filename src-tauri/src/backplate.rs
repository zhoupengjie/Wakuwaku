// A blur of what is behind her island (zhoupengjie/Wakuwaku#20). The page's
// own blur (style.css --i-blur, a backdrop-filter) blurs only what the page
// draws: open above the taskbar, or at the top of the screen, there is
// nothing of the page behind the island but other programs' windows, and a
// see-through ground showed their words sharp through it. So a window of
// our own goes right under the island's window, the island's shape, with
// Windows' acrylic: DWM blurs whatever is behind it. The page's ground, at
// its own opacity, lies over it, as Windows' own flyouts are made.
//
// Its acrylic is the window's accent (SetWindowCompositionAttribute,
// ACCENT_ENABLE_ACRYLICBLURBEHIND, unnamed in the SDK but what Windows'
// own shell and many programs use): DWM's documented backdrop for flyouts
// (DWMWA_SYSTEMBACKDROP_TYPE, DWMSBT_TRANSIENTWINDOW) drew a flat grey on a
// window never active, as this one never is, and neither honours a window
// region; the accent blurs either way. Rounded by DWM's own corners
// (DWMWA_WINDOW_CORNER_PREFERENCE, ROUND: 8 px), set in from the island's
// edge (by the page, island.js plateInset) far enough that they stay inside
// the island's rounder ones; the page paints the band left over (its rim).
// Layered, so it lets the pointer through and fades (its alpha). Where the
// accent is not there or is refused (SetWindowCompositionAttribute missing,
// or answering 0, when tried at first and whenever set), there is none:
// available says so, and the page deepens its ground instead.
//
// The page says where and when (island.js placePlate): for a look that is
// see-through and blurred, under the island as it is, moved with it as it
// changes size; nothing otherwise, nor while she is quiet. Made on first
// use, on a thread of its own that waits for messages, so nothing runs
// while it is not moving.
//
// A second one, the same, under her dock (split: bars.rs, bar.js glass):
// its window stays as it is while its glass grows and shrinks in its page,
// as the island's does, the plate under the glass where it settles.
use crate::Shared;

// Which plate: under her island, or under the dock.
pub const ISLAND: usize = 0;
pub const DOCK: usize = 1;
#[cfg_attr(not(windows), allow(dead_code))]
const PLATES: usize = 2;

// Where the plate goes on the screen (physical px), Windows' mode for its tint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plate {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub dark: bool,
}

// A box of the page's (x, y, w, h, its px) on the screen: from the window's
// origin at its scale, rounded inwards, so the plate never reaches past
// where the page set it in to.
pub fn on_screen((ox, oy): (i32, i32), sf: f64, (x, y, w, h): (f64, f64, f64, f64)) -> (i32, i32, i32, i32) {
    let (left, top) = ((x * sf).ceil() as i32, (y * sf).ceil() as i32);
    let (right, bottom) = (((x + w) * sf).floor() as i32, ((y + h) * sf).floor() as i32);
    (ox + left, oy + top, right - left, bottom - top)
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;

    use super::{Plate, ISLAND, PLATES};

    type Hwnd = *mut c_void;

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
    #[derive(Default, Clone, Copy)]
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

    // ACCENT_POLICY and WINDOWCOMPOSITIONATTRIBDATA, as the shell has them.
    #[repr(C)]
    struct AccentPolicy {
        state: u32,
        flags: u32,
        gradient: u32,
        animation: u32,
    }

    #[repr(C)]
    struct CompositionData {
        attribute: u32,
        data: *const c_void,
        size: usize,
    }

    type SetComposition = unsafe extern "system" fn(Hwnd, *const CompositionData) -> i32;

    #[link(name = "user32")]
    extern "system" {
        fn RegisterClassExW(wc: *const WndClassExW) -> u16;
        fn CreateWindowExW(ex: u32, class: *const u16, name: *const u16, style: u32, x: i32, y: i32, w: i32, h: i32, parent: Hwnd, menu: isize, instance: isize, param: *mut c_void) -> Hwnd;
        fn DefWindowProcW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize;
        fn GetMessageW(msg: *mut Msg, hwnd: Hwnd, min: u32, max: u32) -> i32;
        fn DispatchMessageW(msg: *const Msg) -> isize;
        fn PostMessageW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> i32;
        fn SetWindowPos(hwnd: Hwnd, after: isize, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
        fn ShowWindow(hwnd: Hwnd, cmd: i32) -> i32;
        fn SetLayeredWindowAttributes(hwnd: Hwnd, key: u32, alpha: u8, flags: u32) -> i32;
        fn SetTimer(hwnd: Hwnd, id: usize, ms: u32, f: *const c_void) -> usize;
        fn KillTimer(hwnd: Hwnd, id: usize) -> i32;
        fn DestroyWindow(hwnd: Hwnd) -> i32;
    }

    #[link(name = "gdi32")]
    extern "system" {
        fn CreateSolidBrush(colour: u32) -> isize;
    }

    #[link(name = "dwmapi")]
    extern "system" {
        fn DwmSetWindowAttribute(hwnd: Hwnd, attribute: u32, value: *const c_void, size: u32) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleW(name: *const u16) -> isize;
        fn LoadLibraryW(name: *const u16) -> isize;
        fn GetProcAddress(module: isize, name: *const u8) -> *const ();
    }

    const WS_POPUP: u32 = 0x8000_0000;
    const WS_EX_TOOLWINDOW: u32 = 0x80;
    const WS_EX_TOPMOST: u32 = 0x8;
    const WS_EX_NOACTIVATE: u32 = 0x0800_0000;
    const WS_EX_LAYERED: u32 = 0x8_0000;
    const WS_EX_TRANSPARENT: u32 = 0x20;
    const LWA_ALPHA: u32 = 0x2;
    const SW_HIDE: i32 = 0;
    const SWP_NOACTIVATE: u32 = 0x10;
    const SWP_SHOWWINDOW: u32 = 0x40;
    const WM_TIMER: u32 = 0x0113;
    const WM_NCHITTEST: u32 = 0x0084;
    const WM_MOUSEACTIVATE: u32 = 0x0021;
    const WM_APP_WANT: u32 = 0x8000 + 0x50;
    const HTTRANSPARENT: isize = -1;
    const MA_NOACTIVATE: isize = 3;
    const DWMWA_TRANSITIONS_FORCEDISABLED: u32 = 3;
    const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
    const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    const DWMWA_BORDER_COLOR: u32 = 34;
    const DWMWCP_ROUND: u32 = 2;
    const DWMWA_COLOR_NONE: u32 = 0xFFFF_FFFE;
    const WCA_ACCENT_POLICY: u32 = 19;
    const ACCENT_ENABLE_ACRYLICBLURBEHIND: u32 = 4;
    // The accent's own tint (ABGR) over its blur, as faint as reads as
    // Windows' acrylic in each mode: its luminosity comes from it (alpha
    // 0 is not drawn at all). The island's ground does the rest.
    const TINT_DARK: u32 = 0x1020_2020;
    const TINT_LIGHT: u32 = 0x10F3_F3F3;
    const FADE_TICK_MS: u32 = 15;

    // What the page wants: the plate there, or none; how long to fade.
    #[derive(Clone, Copy)]
    struct Want {
        plate: Option<Plate>,
        under: isize,
        fade_ms: u32,
    }

    // Each plate's: what is wished of it, its window once made.
    static WANT: Mutex<[Option<Want>; PLATES]> = Mutex::new([None; PLATES]);
    static HWND: [AtomicIsize; PLATES] = [AtomicIsize::new(0), AtomicIsize::new(0)];
    static STARTED: OnceLock<()> = OnceLock::new();

    // Each window's own state, on their thread.
    #[derive(Clone, Copy)]
    struct Shown {
        plate: Option<Plate>,
        alpha: f64,
        fade: Option<(Instant, f64, f64, u32)>,
        tint: Option<bool>,
    }

    const NOT_SHOWN: Shown = Shown { plate: None, alpha: 0.0, fade: None, tint: None };
    static SHOWN: Mutex<[Shown; PLATES]> = Mutex::new([NOT_SHOWN; PLATES]);

    // Which plate a window of theirs is.
    fn which(hwnd: Hwnd) -> Option<usize> {
        HWND.iter().position(|h| h.load(Ordering::SeqCst) == hwnd as isize)
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    fn set_composition() -> Option<SetComposition> {
        static F: OnceLock<usize> = OnceLock::new();
        let f = *F.get_or_init(|| {
            // SAFETY: user32 is loaded in every GUI process; the name is ours.
            unsafe { GetProcAddress(LoadLibraryW(wide("user32.dll").as_ptr()), c"SetWindowCompositionAttribute".as_ptr() as *const u8) as usize }
        });
        // SAFETY: the export's signature, as the shell declares it.
        (f != 0).then(|| unsafe { std::mem::transmute::<usize, SetComposition>(f) })
    }

    // The accent set on a window, for Windows' mode: whether it took.
    fn accent(hwnd: Hwnd, dark: bool) -> bool {
        let Some(f) = set_composition() else { return false };
        let policy = AccentPolicy { state: ACCENT_ENABLE_ACRYLICBLURBEHIND, flags: 2, gradient: if dark { TINT_DARK } else { TINT_LIGHT }, animation: 0 };
        let data = CompositionData { attribute: WCA_ACCENT_POLICY, data: &policy as *const AccentPolicy as *const c_void, size: std::mem::size_of::<AccentPolicy>() };
        // SAFETY: a window of ours and structs alive for the call.
        unsafe { f(hwnd, &data) != 0 }
    }

    // Tried once on a hidden window of the system's own class, made and
    // gone on the calling thread.
    fn accent_takes() -> bool {
        // SAFETY: a window of a system class, never shown, destroyed on this thread.
        unsafe {
            let hwnd = CreateWindowExW(WS_EX_TOOLWINDOW, wide("STATIC").as_ptr(), std::ptr::null(), WS_POPUP, 0, 0, 1, 1, std::ptr::null_mut(), 0, GetModuleHandleW(std::ptr::null()), std::ptr::null_mut());
            if hwnd.is_null() {
                return false;
            }
            let took = accent(hwnd, true);
            DestroyWindow(hwnd);
            took
        }
    }

    // Refused once it was set on the plate (SetWindowCompositionAttribute
    // answering 0): none from then on.
    static REFUSED: AtomicBool = AtomicBool::new(false);

    // Windows 11 (DWM's round corners, build 22000 on), the accent there and
    // taken (tried at first, and as set), and transparency effects on
    // (Settings → Personalization → Colors): Windows' own acrylic is solid
    // with them off.
    pub fn available() -> bool {
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        use winreg::RegKey;
        static OS: OnceLock<bool> = OnceLock::new();
        let os = *OS.get_or_init(|| {
            let build: String = RegKey::predef(HKEY_LOCAL_MACHINE)
                .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
                .and_then(|k| k.get_value("CurrentBuild"))
                .unwrap_or_default();
            build.parse::<u32>().is_ok_and(|b| b >= 22000) && accent_takes()
        });
        if !os || REFUSED.load(Ordering::SeqCst) {
            return false;
        }
        let transparency: u32 = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
            .and_then(|k| k.get_value("EnableTransparency"))
            .unwrap_or(1);
        transparency != 0
    }

    // The page's last wish, kept while the island's window is hidden.
    static LAST: Mutex<Option<(Plate, isize)>> = Mutex::new(None);

    // The page's wish (kept: shown again with the window).
    pub fn wish(plate: Option<Plate>, under: isize, fade_ms: u32, shown: bool) {
        *LAST.lock().unwrap() = plate.map(|p| (p, under));
        let plate = if shown { plate } else { None };
        set(ISLAND, plate, under, fade_ms);
    }

    // Whether the dock's plate is in: shown, wholly, no fade going.
    pub fn dock_in() -> bool {
        let all = SHOWN.lock().unwrap();
        let s = &all[super::DOCK];
        s.plate.is_some() && s.alpha >= 255.0 && s.fade.is_none()
    }

    // The window shown again: its plate back, as the page last wished.
    pub fn again() {
        let last = *LAST.lock().unwrap();
        if let Some((plate, under)) = last {
            set(ISLAND, Some(plate), under, 0);
        }
    }

    pub fn set(which: usize, plate: Option<Plate>, under: isize, fade_ms: u32) {
        WANT.lock().unwrap()[which] = Some(Want { plate, under, fade_ms });
        let hwnd = HWND[which].load(Ordering::SeqCst);
        if hwnd != 0 {
            // SAFETY: our own window; the message carries nothing.
            unsafe { PostMessageW(hwnd as Hwnd, WM_APP_WANT, 0, 0) };
        } else if plate.is_some() && STARTED.set(()).is_ok() {
            crate::spawn_named("plate", run);
        }
    }

    // Not easy on the power for good: the plates follow the island's and the
    // dock's glass as they grow and shrink, frame by frame, and with the CPU
    // busy they lagged behind them (the user, 2026-11-08: 高负载的情况下整个界面
    // 很容易卡死). Slowed with the process while nobody is about.
    fn run() {
        // SAFETY: our own class and window, on this thread, which runs its loop.
        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let class = wide("WakuwakuPlate");
            let wc = WndClassExW {
                size: std::mem::size_of::<WndClassExW>() as u32,
                style: 0,
                wndproc,
                cls_extra: 0,
                wnd_extra: 0,
                instance,
                icon: 0,
                cursor: 0,
                background: CreateSolidBrush(0),
                menu_name: std::ptr::null(),
                class_name: class.as_ptr(),
                icon_sm: 0,
            };
            RegisterClassExW(&wc);
            let ex = WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT;
            // Each plate a window of its own, on this thread.
            for slot in &HWND {
                let hwnd = CreateWindowExW(ex, class.as_ptr(), std::ptr::null(), WS_POPUP, 0, 0, 0, 0, std::ptr::null_mut(), 0, instance, std::ptr::null_mut());
                if hwnd.is_null() {
                    continue;
                }
                SetLayeredWindowAttributes(hwnd, 0, 0, LWA_ALPHA);
                let attr = |a: u32, v: u32| DwmSetWindowAttribute(hwnd, a, &v as *const u32 as *const c_void, 4);
                attr(DWMWA_TRANSITIONS_FORCEDISABLED, 1);
                attr(DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND);
                attr(DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE);
                slot.store(hwnd as isize, Ordering::SeqCst);
                PostMessageW(hwnd, WM_APP_WANT, 0, 0);
            }
            let mut msg: Msg = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                DispatchMessageW(&msg);
            }
        }
    }

    // The accent's tint for Windows' mode, set again only when it changes;
    // whether it took.
    fn tint(hwnd: Hwnd, which: usize, dark: bool) -> bool {
        let mut shown = SHOWN.lock().unwrap();
        if shown[which].tint == Some(dark) {
            return true;
        }
        shown[which].tint = Some(dark);
        drop(shown);
        let mode = dark as u32;
        // SAFETY: our own window and value, alive for the call.
        unsafe { DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &mode as *const u32 as *const c_void, 4) };
        accent(hwnd, dark)
    }

    // The page's latest wish: placed under the island and faded in, or
    // faded out and hidden. The accent refused: none, and the pages drawn
    // anew to deepen their ground instead (available).
    fn apply(hwnd: Hwnd, which: usize) {
        let Some(want) = WANT.lock().unwrap()[which].take() else { return };
        let mut all = SHOWN.lock().unwrap();
        let from = all[which].alpha;
        match want.plate {
            Some(_) if REFUSED.load(Ordering::SeqCst) => fade(hwnd, &mut all[which], from, 0.0, 0),
            Some(p) => {
                drop(all);
                if !tint(hwnd, which, p.dark) {
                    REFUSED.store(true, Ordering::SeqCst);
                    all = SHOWN.lock().unwrap();
                    fade(hwnd, &mut all[which], from, 0.0, 0);
                    drop(all);
                    super::refused();
                    return;
                }
                // SAFETY: our own window, placed right under the island's (the dock's).
                unsafe { SetWindowPos(hwnd, want.under, p.x, p.y, p.w, p.h, SWP_NOACTIVATE | SWP_SHOWWINDOW) };
                all = SHOWN.lock().unwrap();
                all[which].plate = Some(p);
                fade(hwnd, &mut all[which], from, 255.0, want.fade_ms);
                // In at once (no fade, or it was in already): said so now.
                if all[which].fade.is_none() {
                    drop(all);
                    super::plate_in(which);
                }
            }
            None if all[which].plate.is_some() => fade(hwnd, &mut all[which], from, 0.0, want.fade_ms),
            None => {}
        }
    }

    fn fade(hwnd: Hwnd, shown: &mut Shown, from: f64, to: f64, ms: u32) {
        if ms == 0 || from == to {
            shown.fade = None;
            // SAFETY: our own window.
            unsafe { KillTimer(hwnd, 1) };
            set_alpha(hwnd, shown, to);
            return;
        }
        shown.fade = Some((Instant::now(), from, to, ms));
        // SAFETY: our own window; the timer posts to its loop.
        unsafe { SetTimer(hwnd, 1, FADE_TICK_MS, std::ptr::null()) };
    }

    fn set_alpha(hwnd: Hwnd, shown: &mut Shown, alpha: f64) {
        shown.alpha = alpha;
        // SAFETY: our own window.
        unsafe {
            SetLayeredWindowAttributes(hwnd, 0, alpha.round().clamp(0.0, 255.0) as u8, LWA_ALPHA);
            if alpha <= 0.0 {
                ShowWindow(hwnd, SW_HIDE);
                shown.plate = None;
            }
        }
    }

    // A step of the fade, eased out as the page's own are.
    fn tick(hwnd: Hwnd, which: usize) {
        let mut all = SHOWN.lock().unwrap();
        let shown = &mut all[which];
        let Some((at, from, to, ms)) = shown.fade else {
            // SAFETY: our own window.
            unsafe { KillTimer(hwnd, 1) };
            return;
        };
        let t = (at.elapsed().as_secs_f64() * 1000.0 / ms as f64).min(1.0);
        let eased = 1.0 - (1.0 - t).powi(2);
        set_alpha(hwnd, shown, from + (to - from) * eased);
        if t >= 1.0 {
            shown.fade = None;
            // SAFETY: our own window.
            unsafe { KillTimer(hwnd, 1) };
            if to >= 255.0 {
                drop(all);
                super::plate_in(which);
            }
        }
    }

    extern "system" fn wndproc(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize {
        match (msg, which(hwnd)) {
            (WM_APP_WANT, Some(n)) => {
                apply(hwnd, n);
                0
            }
            (WM_TIMER, Some(n)) if wparam == 1 => {
                tick(hwnd, n);
                0
            }
            // The pointer goes through to what is behind (it is layered and
            // transparent too), and it never takes the focus.
            (WM_NCHITTEST, _) => HTTRANSPARENT,
            (WM_MOUSEACTIVATE, _) => MA_NOACTIVATE,
            // SAFETY: the default handling of a message sent to our window.
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::Plate;
    pub fn available() -> bool {
        false
    }
    pub fn set(_which: usize, _plate: Option<Plate>, _under: isize, _fade_ms: u32) {}
    pub fn wish(_plate: Option<Plate>, _under: isize, _fade_ms: u32, _shown: bool) {}
    pub fn again() {}
    pub fn dock_in() -> bool {
        false
    }
}

pub use imp::{again, available, dock_in};

// Her app, for the pages to be drawn anew should the accent be refused.
static APP: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();

// The plate under her island's window as the page wishes it, faded in over
// fade_ms; none: faded out and hidden.
pub fn set(sh: &Shared, plate: Option<Plate>, fade_ms: u32) {
    APP.get_or_init(|| sh.app.clone());
    // Her island's window hidden (an app full screen, hideInFullscreen; the
    // taskbar her home): the wish kept, nothing shown till it is again
    // (again). The page, hidden with it, still asks: opened by itself for
    // an ending, a blur with nothing over it stood on the screen.
    crate::island::with_shown(sh, |under, shown| imp::wish(plate, under, fade_ms.min(1000), shown));
}

// Gone at once with her island's window (island.rs show_or_hide), and back
// with it (again).
pub fn hide() {
    imp::set(ISLAND, None, 0, 0);
}

// The dock's (bars.rs dock_glass): under its window (under), faded in over
// fade_ms; none, faded out (fade_ms) and hidden.
pub fn dock(sh: &Shared, plate: Option<Plate>, under: isize, fade_ms: u32) {
    APP.get_or_init(|| sh.app.clone());
    imp::set(DOCK, plate, under, fade_ms.min(1000));
}

// The accent refused on the plate: the pages told there is none
// (pet:update's plate), so they deepen their ground instead.
#[cfg_attr(not(windows), allow(dead_code))]
fn refused() {
    if let Some(app) = APP.get() {
        let sh = crate::shared(app);
        sh.log("plate: Windows refused the acrylic; the ground deepens instead");
        sh.redraw();
        // The dock's page told too: the desktop's picture under it instead.
        crate::bars::refresh(&sh);
    }
}

// A plate wholly in (faded in, or at once): the dock's page told, which
// keeps its glass solid till then (bars.rs plate_in, bar.js plateCame).
#[cfg_attr(not(windows), allow(dead_code))]
fn plate_in(which: usize) {
    if which != DOCK {
        return;
    }
    if let Some(app) = APP.get() {
        crate::bars::plate_in(&crate::shared(app));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_lies_inside_where_the_page_set_it() {
        // At 125 %, 4.1 px in from an island at 220.0, 8.8 (the page's
        // px): never past it, by a part of a pixel at most.
        assert_eq!(super::on_screen((805, 0), 1.25, (224.1, 12.9, 311.8, 107.8)), (805 + 281, 17, 388, 133));
        // Whole pixels at 100 % stay as they are.
        assert_eq!(super::on_screen((0, 0), 1.0, (10.0, 20.0, 30.0, 40.0)), (10, 20, 30, 40));
    }
}
