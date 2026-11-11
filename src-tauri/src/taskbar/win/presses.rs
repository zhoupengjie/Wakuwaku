// Presses anywhere, watched while a list or flyout of the strip is open (a
// low-level mouse hook on the taskbar's thread): one outside closes it.
use super::*;

// A press anywhere, told to the page while it has something open (the
// windows listed above a button, the folded tray icons) that closes on
// a press elsewhere, as Windows' do: ours never take the focus whose
// loss would tell. A low-level mouse hook, on the host's thread (whose
// loop it needs) and only while asked for.
pub(super) const WM_WATCH_PRESSES: u32 = 0x8010;
pub(super) const WH_MOUSE_LL: i32 = 14;
pub(super) const WM_LBUTTONDOWN: u32 = 0x0201;
pub(super) const WM_RBUTTONDOWN: u32 = 0x0204;
pub(super) const WM_MBUTTONDOWN: u32 = 0x0207;
pub(super) static PRESS_HOOK: AtomicIsize = AtomicIsize::new(0);

#[repr(C)]
pub(super) struct MouseLl {
    pt: Point,
    data: u32,
    flags: u32,
    time: u32,
    extra: usize,
}

pub(super) type HookProc = extern "system" fn(i32, usize, isize) -> isize;

#[link(name = "user32")]
extern "system" {
    pub(super) fn SetWindowsHookExW(id: i32, proc_: HookProc, module: isize, thread: u32) -> isize;
    pub(super) fn UnhookWindowsHookEx(hook: isize) -> i32;
    pub(super) fn CallNextHookEx(hook: isize, code: i32, wparam: usize, lparam: isize) -> isize;
}

pub fn watch_presses(on: bool) {
    // SAFETY: a posted message to the host's own window.
    unsafe { PostMessageW(OURS.load(Ordering::SeqCst) as Hwnd, WM_WATCH_PRESSES, on as usize, 0) };
}

// On the host's thread.
pub(super) fn watch(on: bool) {
    let hook = PRESS_HOOK.load(Ordering::SeqCst);
    // SAFETY: a hook of our own, put in and taken out on this thread.
    unsafe {
        if on && hook == 0 {
            PRESS_HOOK.store(SetWindowsHookExW(WH_MOUSE_LL, press_hook, GetModuleHandleW(std::ptr::null()), 0), Ordering::SeqCst);
        } else if !on && hook != 0 {
            UnhookWindowsHookEx(hook);
            PRESS_HOOK.store(0, Ordering::SeqCst);
        }
    }
}

// In every press's way through Windows: only the point posted on (to the
// host's own loop, press_seen), and passed on at once. Held up here, every
// click on the machine would wait, and Windows would drop the hook.
pub(super) const WM_PRESS_SEEN: u32 = 0x8011;

pub(super) extern "system" fn press_hook(code: i32, wparam: usize, lparam: isize) -> isize {
    if code >= 0 && matches!(wparam as u32, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN) {
        // SAFETY: Windows hands a MSLLHOOKSTRUCT with these messages; a
        // posted message to the host's own window, no pointers.
        unsafe {
            let at = (*(lparam as *const MouseLl)).pt;
            PostMessageW(OURS.load(Ordering::SeqCst) as Hwnd, WM_PRESS_SEEN, at.x as u32 as usize, at.y as isize);
        }
    }
    // SAFETY: passed on, as every hook must.
    unsafe { CallNextHookEx(0, code, wparam, lparam) }
}

// On the host's thread, after the hook let the press go: where on the
// page (its px), for it to tell what was pressed.
pub(super) fn press_seen(x: i32, y: i32) {
    if let Some(sh) = sh() {
        let ((ox, oy), sf) = crate::island::origin(&sh);
        let _ = sh.app.emit_to("island", "taskbar:press", json!({ "x": (x - ox) as f64 / sf, "y": (y - oy) as f64 / sf }));
        // A list or menu open over her taskbar on another display.
        crate::bars::pressed(&sh, (x, y));
    }
}
