// Windows' own keys for its taskbar, taken while hers holds the strip
// (zhoupengjie/Wakuwaku#36). Windows' taskbar is put away but still runs,
// and these went to it: Win+1…9 and Win+0 (the tenth) opened the program in
// that place by its own order, which is not hers (another program than the
// one there on her strip); Win+T and Win+B put the keyboard on its buttons
// and its tray, unseen, and what was typed after went to them. Now:
//   - Win+digit: the program in that place on her strip, in her order (as
//     island-glide.js orderedApps lays them out): started when it does not run;
//     one window to the front, or minimized when it is in front; several,
//     the one last in front, then the next of them at each press, as
//     Windows 11 does. Win+Shift+digit another of it, Win+Ctrl+Shift+digit
//     another as administrator, Win+Ctrl+digit its window last in front.
//     Win+Alt+digit would open its jump list: there is none on her taskbar
//     yet, so it does nothing (and does not reach Windows' either).
//   - Win+T (Win+Shift+T from the last) the keyboard on her programs'
//     buttons, Win+B on her tray (island.js: the arrows, Enter, Esc).
// Everything else with Win (Win+D, Win+E, Win+Shift+S, Win+Space…) is
// Windows' as ever, and Win itself is never taken.
//
// A low-level keyboard hook (WH_KEYBOARD_LL) on a thread of its own that
// does nothing else: Windows hands it every key on the machine and waits
// for its answer, and drops a hook that answers later than
// LowLevelHooksTimeout without a word. So the hook only looks (the keys
// held with Win, by GetAsyncKeyState), takes a key of ours, and posts what
// to do to another thread (the keys' work), which may take its time (a
// window coming up, a program starting). Should a key reach the hook late,
// or its thread be held up, the hook is put in again and the log says so.
//
// Win let go with nothing else seen since it went down opens Start: a key
// of ours taken is never seen. So with it an unassigned key (0xE8, down and
// up) is sent at once, from inside the hook, before Win comes up; as
// AutoHotkey's menu mask key (vkE8, its default) and PowerToys' dummy key
// do. It also keeps a held Alt from opening the menu bar of the window in
// front. Sent marked as ours, so the hook lets it by.
//
// Only while her taskbar holds the strip (taskbar/win/hold.rs take, give_back): with
// her island at the top of the screen, or the taskbar given back, the keys
// are Windows' own. A window run as administrator in front does not show
// its keys to a hook of a program that is not (UIPI): there they still go
// to Windows' taskbar.
#![cfg_attr(not(windows), allow(dead_code))]

use serde_json::Value;

// What a press of a digit with Win asks of the program in that place, by
// the keys held with them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum How {
    // Started, to the front, minimized, its windows in turn (Win alone).
    Press,
    // Another of it (Shift).
    New,
    // Another of it, as administrator (Ctrl+Shift).
    Admin,
    // Its window last in front (Ctrl).
    Last,
    // Its jump list (Alt): none on her taskbar yet.
    Jump,
}

const HOWS: [How; 5] = [How::Press, How::New, How::Admin, How::Last, How::Jump];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    // The program in this place on her strip (0 the first; Win+0 is 9).
    App(usize, How),
    // The keyboard on her programs' buttons (from the last: back) or her tray.
    Focus { tray: bool, back: bool },
}

// The keys held with a key, as the hook reads them.
#[derive(Clone, Copy, Debug, Default)]
pub struct Mods {
    pub win: bool,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

// What the hook does with a key: lets it go on; takes it; takes it and
// acts (the mask key sent with it, mask_now).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Take,
    Act(Act),
}

const VK_LWIN: u32 = 0x5B;
const VK_RWIN: u32 = 0x5C;
const VK_B: u32 = 0x42;
const VK_T: u32 = 0x54;

// Each key of ours, a bit of what is held: the ten digits, T, B.
fn bit(vk: u32) -> Option<u16> {
    match vk {
        0x30..=0x39 => Some(1 << (vk - 0x30)),
        VK_T => Some(1 << 10),
        VK_B => Some(1 << 11),
        _ => None,
    }
}

// A key the hook needs to look at more closely: one of ours, or Win.
pub fn watched(vk: u32) -> bool {
    bit(vk).is_some() || vk == VK_LWIN || vk == VK_RWIN
}

// What a press of one of ours asks for, with these held; None: Windows'
// own (no Win, or with keys Windows' taskbar does not use with it: Win+Alt+B
// is Windows' HDR, Win+Ctrl+Shift+B starts the display driver again). The
// top row's digits alone, as Windows' (the number pad's are not its).
pub fn act_of(vk: u32, m: Mods) -> Option<Act> {
    if !m.win {
        return None;
    }
    match vk {
        0x30..=0x39 => {
            let slot = if vk == 0x30 { 9 } else { (vk - 0x31) as usize };
            let how = match (m.shift, m.ctrl, m.alt) {
                (false, false, false) => How::Press,
                (true, false, false) => How::New,
                (true, true, false) => How::Admin,
                (false, true, false) => How::Last,
                (false, false, true) => How::Jump,
                _ => return None,
            };
            Some(Act::App(slot, how))
        }
        VK_T if !m.ctrl && !m.alt => Some(Act::Focus { tray: false, back: m.shift }),
        VK_B if !m.ctrl && !m.alt && !m.shift => Some(Act::Focus { tray: true, back: false }),
        _ => None,
    }
}

// A key through the hook, and those of ours held after it: a press of ours
// with Win acts, and is held; its repeats (held down) are taken with
// nothing done, and its release is taken too, wherever Win is by then.
// Win let go, none is held any more: a release missed (the secure desktop
// of Ctrl+Alt+Del came between) never takes a later press. One held
// without Win now is not ours any more either.
pub fn decide(vk: u32, up: bool, m: Mods, held: u16) -> (Verdict, u16) {
    if vk == VK_LWIN || vk == VK_RWIN {
        return (Verdict::Pass, if up { 0 } else { held });
    }
    let Some(b) = bit(vk) else { return (Verdict::Pass, held) };
    if up {
        return if held & b != 0 { (Verdict::Take, held & !b) } else { (Verdict::Pass, held) };
    }
    if held & b != 0 && m.win {
        return (Verdict::Take, held);
    }
    match act_of(vk, m) {
        Some(act) => (Verdict::Act(act), held | b),
        None => (Verdict::Pass, held & !b),
    }
}

// Start kept closed: the mask key goes with each press of ours that acts,
// Win being held then; not with a repeat nor a release (one is enough
// before Win comes up).
pub fn masks(v: Verdict) -> bool {
    matches!(v, Verdict::Act(_))
}

// An act as a thread message's wParam, and back.
pub fn encode(act: Act) -> usize {
    match act {
        Act::App(slot, how) => (slot & 0xF) | (HOWS.iter().position(|h| *h == how).unwrap_or(0) << 4),
        Act::Focus { tray, back } => 0x100 | tray as usize | (back as usize) << 1,
    }
}

pub fn decode(w: usize) -> Option<Act> {
    if w & 0x100 != 0 {
        return Some(Act::Focus { tray: w & 1 != 0, back: w & 2 != 0 });
    }
    let how = *HOWS.get(w >> 4)?;
    (w & 0xF < 10).then_some(Act::App(w & 0xF, how))
}

// The programs' buttons in the order her strip shows them (island.js
// orderedApps): those dragged into an order first, in it (taskbarOrder,
// their keys), the rest after, as they came.
pub fn ordered(list: &Value, order: &[String]) -> Vec<Value> {
    let rank = |a: &Value| a["app"].as_str().and_then(|k| order.iter().position(|o| o == k)).unwrap_or(usize::MAX);
    let mut apps: Vec<(usize, &Value)> = list.as_array().into_iter().flatten().enumerate().collect();
    apps.sort_by_key(|&(n, a)| (rank(a), n));
    apps.into_iter().map(|(_, a)| a.clone()).collect()
}

// What a press asks of a program: started; its one window to the front or
// minimized when in front (as a press on its button: Toggle); one window
// to the front (Bring); nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Start,
    Toggle(isize),
    Bring(isize),
    Nothing,
}

// By its windows as its button lists them, the window in front, and its
// windows as they lie (the top one first: the one last in front). Several:
// the one last in front first, then each press the next after the one in
// front, round again after the last, as Windows 11's Win+digit goes
// through them. A window already in front is not brought again.
pub fn target(how: How, windows: &[isize], front: isize, stack: &[isize]) -> Target {
    let last = || stack.iter().copied().find(|h| windows.contains(h)).or(windows.first().copied());
    let bring = |h: Option<isize>| h.filter(|&h| h != front).map_or(Target::Nothing, Target::Bring);
    match how {
        How::Jump => Target::Nothing,
        How::New | How::Admin => Target::Start,
        _ if windows.is_empty() => Target::Start,
        How::Last => bring(last()),
        How::Press if windows.len() == 1 => Target::Toggle(windows[0]),
        How::Press => match windows.iter().position(|&w| w == front) {
            Some(i) => bring(Some(windows[(i + 1) % windows.len()])),
            None => bring(last()),
        },
    }
}

// The key as said in the log: Win+Shift+2.
fn said(act: Act) -> String {
    match act {
        Act::App(slot, how) => {
            let mods = match how {
                How::Press => "",
                How::New => "Shift+",
                How::Admin => "Ctrl+Shift+",
                How::Last => "Ctrl+",
                How::Jump => "Alt+",
            };
            format!("Win+{mods}{}", (slot + 1) % 10)
        }
        Act::Focus { tray: true, .. } => "Win+B".into(),
        Act::Focus { back, .. } => format!("Win+{}T", if back { "Shift+" } else { "" }),
    }
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU16, AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread::JoinHandle;
    use std::time::{Duration, Instant};

    use serde_json::json;
    use tauri::{Emitter, Manager};

    use super::{decide, decode, encode, masks, ordered, said, target, watched, Act, How, Mods, Target, Verdict};
    use crate::{tasks, Shared};

    type Hwnd = *mut c_void;

    #[repr(C)]
    struct KbdLl {
        vk: u32,
        scan: u32,
        flags: u32,
        time: u32,
        extra: usize,
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

    type HookProc = extern "system" fn(i32, usize, isize) -> isize;

    #[link(name = "user32")]
    extern "system" {
        fn SetWindowsHookExW(id: i32, proc_: HookProc, module: isize, thread: u32) -> isize;
        fn UnhookWindowsHookEx(hook: isize) -> i32;
        fn CallNextHookEx(hook: isize, code: i32, wparam: usize, lparam: isize) -> isize;
        fn GetAsyncKeyState(vk: i32) -> i16;
        fn SendInput(count: u32, inputs: *const Input, size: i32) -> u32;
        fn GetMessageW(msg: *mut Msg, hwnd: Hwnd, min: u32, max: u32) -> i32;
        fn PeekMessageW(msg: *mut Msg, hwnd: Hwnd, min: u32, max: u32, remove: u32) -> i32;
        fn PostThreadMessageW(thread: u32, msg: u32, wparam: usize, lparam: isize) -> i32;
        fn SetTimer(hwnd: Hwnd, id: usize, ms: u32, func: *const c_void) -> usize;
        fn KillTimer(hwnd: Hwnd, id: usize) -> i32;
        fn EnumWindows(each: extern "system" fn(Hwnd, isize) -> i32, param: isize) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleW(name: *const u16) -> isize;
        fn GetCurrentThreadId() -> u32;
        fn GetTickCount() -> u32;
    }

    const WH_KEYBOARD_LL: i32 = 13;
    const HC_ACTION: i32 = 0;
    const LLKHF_UP: u32 = 0x80;
    const VK_SHIFT: i32 = 0x10;
    const VK_CONTROL: i32 = 0x11;
    const VK_MENU: i32 = 0x12;
    const VK_LWIN: i32 = 0x5B;
    const VK_RWIN: i32 = 0x5C;
    // Unassigned: no program takes it for anything (AutoHotkey's menu mask key).
    const VK_MASK: u16 = 0xE8;
    const KEYEVENTF_KEYUP: u32 = 0x2;
    // On the keys this sends, so the hook knows them for its own: "WAKU".
    const MARK: usize = 0x5741_4B55;
    const WM_QUIT: u32 = 0x0012;
    const WM_TIMER: u32 = 0x0113;
    // To the work's thread: an act (wParam, encode). To the hook's: a key
    // reached it this late (wParam, ms).
    const WM_ACT: u32 = 0x8001;
    const WM_LATE: u32 = 0x8002;
    // To the work's thread: Win let go; the page's Win pressed (pause).
    const WM_WIN_UP: u32 = 0x8003;
    const WM_PAUSE: u32 = 0x8004;
    const PM_NOREMOVE: u32 = 0;
    // A key this late into the hook, or the hook this long on it: Windows
    // may have given up waiting (LowLevelHooksTimeout, 300 ms to 1 s) and
    // taken the hook away. The hook's thread looked at this often, to tell
    // whether it was held up.
    const LATE_MS: u32 = 200;
    const LOOK_MS: u32 = 5000;

    // The hook, and the two threads' ids (their message queues) and handles.
    static HOOK: AtomicIsize = AtomicIsize::new(0);
    static HOOK_THREAD: AtomicU32 = AtomicU32::new(0);
    static WORK_THREAD: AtomicU32 = AtomicU32::new(0);
    static THREADS: Mutex<Option<(JoinHandle<()>, JoinHandle<()>)>> = Mutex::new(None);
    // Our keys held down after their press was taken (decide), on the hook's thread alone.
    static HELD: AtomicU16 = AtomicU16::new(0);
    // A late key told already (one is enough until the hook is in again).
    static LATE_TOLD: AtomicBool = AtomicBool::new(false);
    static SH: Mutex<Option<Arc<Shared>>> = Mutex::new(None);

    fn log(line: &str) {
        if let Some(sh) = SH.lock().unwrap().clone() {
            sh.log(&format!("winkeys: {line}"));
        }
    }

    fn down(vk: i32) -> bool {
        // SAFETY: a plain query; the high bit is the key's state now.
        unsafe { GetAsyncKeyState(vk) as u16 & 0x8000 != 0 }
    }

    // In every key's way through Windows: as quick as can be, nothing waited
    // on but the mask key's own way through the hooks.
    extern "system" fn key_hook(code: i32, wparam: usize, lparam: isize) -> isize {
        if code == HC_ACTION {
            // SAFETY: Windows hands a KBDLLHOOKSTRUCT with HC_ACTION.
            let k = unsafe { &*(lparam as *const KbdLl) };
            if k.extra != MARK && watched(k.vk) {
                let m = Mods { win: down(VK_LWIN) || down(VK_RWIN), shift: down(VK_SHIFT), ctrl: down(VK_CONTROL), alt: down(VK_MENU) };
                let (verdict, held) = decide(k.vk, k.flags & LLKHF_UP != 0, m, HELD.load(Ordering::Relaxed));
                HELD.store(held, Ordering::Relaxed);
                if masks(verdict) {
                    mask_now();
                }
                if let Verdict::Act(act) = verdict {
                    // SAFETY: a posted message to our own thread, no pointers.
                    unsafe { PostThreadMessageW(WORK_THREAD.load(Ordering::Relaxed), WM_ACT, encode(act), 0) };
                }
                // Win let go: the keyboard waited for it (focus).
                if (k.vk == VK_LWIN as u32 || k.vk == VK_RWIN as u32) && k.flags & LLKHF_UP != 0 {
                    // SAFETY: as above.
                    unsafe { PostThreadMessageW(WORK_THREAD.load(Ordering::Relaxed), WM_WIN_UP, 0, 0) };
                }
                // Late in, or late out (the mask key held up by another's hook).
                // SAFETY: a plain read.
                let late = unsafe { GetTickCount() }.wrapping_sub(k.time);
                if late > LATE_MS && late < 60_000 && !LATE_TOLD.swap(true, Ordering::Relaxed) {
                    // SAFETY: a posted message to this thread's own queue.
                    unsafe { PostThreadMessageW(HOOK_THREAD.load(Ordering::Relaxed), WM_LATE, late as usize, 0) };
                }
                if verdict != Verdict::Pass {
                    return 1;
                }
            }
        }
        // SAFETY: passed on, as every hook must.
        unsafe { CallNextHookEx(0, code, wparam, lparam) }
    }

    // The unassigned key, down and up, before Win comes up: Start stays
    // closed. Sent from inside the hook, so it goes ahead of the keys still
    // to come (a program sending Win, the digit and Win's release all at
    // once too).
    fn mask_now() {
        let key = |up: bool| Input {
            kind: 1,
            u: InputUnion { ki: KeybdInput { vk: VK_MASK, scan: 0, flags: if up { KEYEVENTF_KEYUP } else { 0 }, time: 0, extra: MARK } },
        };
        let inputs = [key(false), key(true)];
        // SAFETY: an array of INPUTs of the size passed.
        unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<Input>() as i32) };
    }

    fn install() -> isize {
        // SAFETY: a hook of our own, on this thread, which runs its loop.
        let hook = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, key_hook, GetModuleHandleW(std::ptr::null()), 0) };
        HOOK.store(hook, Ordering::SeqCst);
        hook
    }

    // Put in again, should Windows have taken it away (it says nothing):
    // taken out first, which should fail for one it took.
    fn renew(why: &str) {
        let old = HOOK.swap(0, Ordering::SeqCst);
        // SAFETY: a hook of our own, put in on this thread.
        let was = if old == 0 {
            "it was not in"
        } else if unsafe { UnhookWindowsHookEx(old) } != 0 {
            "it was still in"
        } else {
            "taking it out failed: Windows had taken it away"
        };
        let now = install();
        LATE_TOLD.store(false, Ordering::Relaxed);
        log(&format!("hook put in again ({why}): {was}; {}", if now != 0 { "in" } else { "could not be put in" }));
    }

    // The hook's thread: its loop, and nothing else.
    fn hook_loop(ready: std::sync::mpsc::Sender<bool>) {
        crate::power::thread_brisk();
        // SAFETY: a plain struct of numbers and a handle, all zero.
        let mut msg: Msg = unsafe { std::mem::zeroed() };
        // SAFETY: our own struct; the call makes this thread's message queue.
        unsafe { PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_NOREMOVE) };
        // SAFETY: a plain call.
        HOOK_THREAD.store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
        HELD.store(0, Ordering::Relaxed);
        let _ = ready.send(install() != 0);
        // SAFETY: a timer of this thread's own (no window).
        let timer = unsafe { SetTimer(std::ptr::null_mut(), 0, LOOK_MS, std::ptr::null()) };
        let mut looked = Instant::now();
        // Put in again at most every ten seconds for late keys: a program
        // that sends keys with times of its own would have it done at each.
        let mut renewed: Option<Instant> = None;
        // SAFETY: our own struct, for this thread's loop.
        while unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } > 0 {
            match msg.message {
                WM_LATE if renewed.is_some_and(|t| t.elapsed() < Duration::from_secs(10)) => LATE_TOLD.store(false, Ordering::Relaxed),
                WM_LATE => {
                    renewed = Some(Instant::now());
                    renew(&format!("a key reached it {} ms late", msg.wparam));
                }
                WM_TIMER => {
                    let gap = looked.elapsed();
                    looked = Instant::now();
                    if gap > Duration::from_millis(u64::from(LOOK_MS) + 1000) {
                        renewed = Some(Instant::now());
                        renew(&format!("its thread was held up: {} ms between looks", gap.as_millis()));
                    } else if HOOK.load(Ordering::SeqCst) == 0 {
                        renew("it was not in");
                    }
                }
                _ => {}
            }
        }
        // SAFETY: our own timer and hook, on this thread.
        unsafe {
            KillTimer(std::ptr::null_mut(), timer);
            let hook = HOOK.swap(0, Ordering::SeqCst);
            if hook != 0 {
                UnhookWindowsHookEx(hook);
            }
        }
        HOOK_THREAD.store(0, Ordering::SeqCst);
    }

    // The work's thread: each act in turn, as long as it takes.
    fn work_loop(sh: Arc<Shared>, ready: std::sync::mpsc::Sender<()>) {
        // SAFETY: as hook_loop: a zeroed struct, this thread's message queue, its id.
        let mut msg: Msg = unsafe { std::mem::zeroed() };
        unsafe { PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_NOREMOVE) };
        WORK_THREAD.store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
        let _ = ready.send(());
        let mut waits = Waits::default();
        // SAFETY: our own struct, for this thread's loop.
        while unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } > 0 {
            match msg.message {
                WM_ACT => {
                    if let Some(act) = decode(msg.wparam) {
                        act_on(&sh, act, &mut waits);
                    }
                }
                WM_WIN_UP => win_up(&sh, &mut waits),
                WM_PAUSE => paused(&sh, &mut waits),
                _ => {}
            }
        }
        WORK_THREAD.store(0, Ordering::SeqCst);
    }

    // The keyboard and Win. While her own window has the keyboard, Windows
    // shows no key to this hook (it is in the process of a WebView2 that has
    // the focus: wudaming00/wh-keyboard-ll-chromium; tried 2026-10-14, the
    // hook put in again or not), and Win+T went to Windows' put-away
    // taskbar, the keyboard with it. So the keyboard is taken once Win is
    // let go (wanted): Win held, T pressed again goes on to the next button
    // as on Windows'. And Win pressed on her strip (the page tells it,
    // island.js keysPause) gives the keyboard back for as long as Win is
    // held (paused), the page keeping its place: let go after a Win+T or
    // Win+B, taken again; after anything else (Win alone, Start opening;
    // Win+D, Win+digit) the keys on her strip are over.
    #[derive(Default)]
    struct Waits {
        wanted: bool,
        paused: bool,
    }

    fn island(sh: &Arc<Shared>) -> Option<tauri::WebviewWindow> {
        sh.app.get_webview_window("island")
    }

    fn win_up(sh: &Arc<Shared>, waits: &mut Waits) {
        if std::mem::take(&mut waits.wanted) {
            waits.paused = false;
            if let Some(win) = island(sh) {
                sh.set_keyboard(&win, true);
            }
        } else if std::mem::take(&mut waits.paused) {
            let _ = sh.app.emit_to("island", "taskbar:focus", json!({ "part": "end" }));
            log("Win let go with nothing of hers: the keys on her strip over");
        }
    }

    fn paused(sh: &Arc<Shared>, waits: &mut Waits) {
        let Some(win) = island(sh) else { return };
        waits.paused = true;
        sh.set_keyboard(&win, false);
        log("Win pressed on her strip: the keyboard given back while it is held");
        // Let go already (a quick press): no release will come by the hook.
        if !(down(VK_LWIN) || down(VK_RWIN)) {
            win_up(sh, waits);
        }
    }

    // The page's Win pressed while her window has the keyboard.
    pub fn pause() {
        let id = WORK_THREAD.load(Ordering::SeqCst);
        if id != 0 {
            // SAFETY: a posted message, no pointers.
            unsafe { PostThreadMessageW(id, WM_PAUSE, 0, 0) };
        }
    }

    // Her taskbar holds the strip: the keys hers (taskbar/win/hold.rs take).
    pub fn start(sh: &Arc<Shared>) {
        let mut threads = THREADS.lock().unwrap();
        if threads.is_some() {
            return;
        }
        *SH.lock().unwrap() = Some(sh.clone());
        let (work_tx, work_rx) = std::sync::mpsc::channel();
        let work_sh = sh.clone();
        let work = crate::spawn_named("taskbar keys work", move || work_loop(work_sh, work_tx));
        let _ = work_rx.recv_timeout(Duration::from_secs(2));
        let (hook_tx, hook_rx) = std::sync::mpsc::channel();
        let hook = crate::spawn_named("taskbar keys hook", move || hook_loop(hook_tx));
        let put_in = hook_rx.recv_timeout(Duration::from_secs(2)).unwrap_or(false);
        sh.log(&format!("winkeys: Win+1…9, Win+T, Win+B are hers: hook {}", if put_in { "in" } else { "could not be put in" }));
        *threads = Some((hook, work));
    }

    // The strip given back: the keys Windows' own again (taskbar/
    // give_back), its threads waited for two seconds at most.
    pub fn stop() {
        let Some((hook, work)) = THREADS.lock().unwrap().take() else { return };
        for id in [HOOK_THREAD.load(Ordering::SeqCst), WORK_THREAD.load(Ordering::SeqCst)] {
            if id != 0 {
                // SAFETY: a posted message, no pointers.
                unsafe { PostThreadMessageW(id, WM_QUIT, 0, 0) };
            }
        }
        let asked = Instant::now();
        while !(hook.is_finished() && work.is_finished()) && asked.elapsed() < Duration::from_secs(2) {
            std::thread::sleep(Duration::from_millis(10));
        }
        log(&format!("the keys Windows' again: hook {}, work {}", if hook.is_finished() { "out" } else { "left" }, if work.is_finished() { "stopped" } else { "left" }));
    }

    extern "system" fn each(hwnd: Hwnd, param: isize) -> i32 {
        // SAFETY: param is the Vec stack() passed, alive for the enumeration.
        unsafe { (*(param as *mut Vec<isize>)).push(hwnd as isize) };
        1
    }

    // These windows as they lie, the one on top (last in front) first.
    fn stack(of: &[isize]) -> Vec<isize> {
        let mut all: Vec<isize> = Vec::new();
        // SAFETY: each() only pushes onto the Vec, which outlives the call.
        unsafe { EnumWindows(each, &mut all as *mut Vec<isize> as isize) };
        all.into_iter().filter(|h| of.contains(h)).collect()
    }

    fn act_on(sh: &Arc<Shared>, act: Act, waits: &mut Waits) {
        let key = said(act);
        match act {
            Act::Focus { tray, back } => waits.wanted = focus(sh, &key, tray, back),
            Act::App(slot, how) => {
                // Win+digit with the keys paused on her strip: they are over.
                if std::mem::take(&mut waits.paused) {
                    let _ = sh.app.emit_to("island", "taskbar:focus", json!({ "part": "end" }));
                }
                app(sh, &key, slot, how)
            }
        }
    }

    // The keyboard on her strip (island.js keys): the page puts the ring on
    // the button at once; her window takes the keyboard once Win is let go
    // (win_up). True when it is to.
    fn focus(sh: &Arc<Shared>, key: &str, tray: bool, back: bool) -> bool {
        if sh.home() != "taskbar" || !crate::island::is_shown(sh) {
            log(&format!("{key}: her taskbar is not shown (full screen): nothing"));
            return false;
        }
        let _ = sh.app.emit_to("island", "taskbar:focus", json!({ "part": if tray { "tray" } else { "apps" }, "back": back }));
        log(&format!("{key}: the keyboard on her {}", if tray { "tray" } else { "programs' buttons" }));
        true
    }

    // The program in that place on her strip, as its button shows it now.
    fn app(sh: &Arc<Shared>, key: &str, slot: usize, how: How) {
        let all = crate::bars::for_main(sh, &crate::taskbar::windows_now());
        let order: Vec<String> = sh.setting("taskbarOrder").as_array().map(|o| o.iter().filter_map(|k| k.as_str().map(str::to_string)).collect()).unwrap_or_default();
        let apps = ordered(&all, &order);
        let Some(app) = apps.get(slot) else {
            log(&format!("{key}: no program in that place ({} on her strip)", apps.len()));
            return;
        };
        let path = app["path"].as_str().unwrap_or_default().to_string();
        let id = app["id"].as_str().filter(|s| !s.is_empty()).map(str::to_string);
        let name = app["name"].as_str().unwrap_or_default();
        let windows: Vec<isize> = app["windows"].as_array().into_iter().flatten().filter_map(|w| w["id"].as_i64()).map(|h| h as isize).collect();
        let front = tasks::front();
        let to = target(how, &windows, front, &stack(&windows));
        log(&format!("{key}: {name} ({} window(s)): {to:?}", windows.len()));
        match to {
            // Another of it, or as administrator (Win+Ctrl+Shift+digit), as
            // Ctrl, Shift and a press on its button start one (taskbar/
            // app: by the shortcut it was pinned from, Windows asking first).
            Target::Start => {
                crate::taskbar::app(sh, if how == How::Admin { "admin" } else { "launch" }, &path, id.as_deref());
            }
            // As a press on its button (Start, should it be open, closed
            // first); the next key waits for it to land, so a quick second
            // press goes on from the window it brought.
            Target::Toggle(h) | Target::Bring(h) => {
                crate::taskbar::taskbar_window(h as i64, "press".into());
                let asked = Instant::now();
                while tasks::front() == front && asked.elapsed() < Duration::from_millis(600) {
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
            // Its window last in front is in front already (Ctrl). Or
            // Win+Alt+digit: its jump list (recent and pinned files, the
            // program's own tasks), as Windows' taskbar opens it above the
            // button, the keyboard in it. Her taskbar has none yet
            // (zhoupengjie/Wakuwaku#36, "jump lists"): taken all the same, so
            // Windows' own does not open another program's.
            Target::Nothing => {}
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use std::sync::Arc;

    use crate::Shared;

    pub fn start(_sh: &Arc<Shared>) {}
    pub fn stop() {}
    pub fn pause() {}
}

pub use imp::{pause, start, stop};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const WIN: Mods = Mods { win: true, shift: false, ctrl: false, alt: false };
    const NONE: Mods = Mods { win: false, shift: false, ctrl: false, alt: false };

    fn with(shift: bool, ctrl: bool, alt: bool) -> Mods {
        Mods { win: true, shift, ctrl, alt }
    }

    #[test]
    fn a_digit_with_win_is_the_program_in_that_place() {
        assert_eq!(act_of(0x31, WIN), Some(Act::App(0, How::Press)));
        assert_eq!(act_of(0x39, WIN), Some(Act::App(8, How::Press)));
        // Win+0, the tenth.
        assert_eq!(act_of(0x30, WIN), Some(Act::App(9, How::Press)));
        assert_eq!(act_of(0x33, with(true, false, false)), Some(Act::App(2, How::New)));
        assert_eq!(act_of(0x33, with(false, true, false)), Some(Act::App(2, How::Last)));
        assert_eq!(act_of(0x33, with(true, true, false)), Some(Act::App(2, How::Admin)));
        assert_eq!(act_of(0x33, with(false, false, true)), Some(Act::App(2, How::Jump)));
        // Combinations Windows' taskbar has none for are Windows' own.
        assert_eq!(act_of(0x33, with(false, true, true)), None);
        assert_eq!(act_of(0x33, with(true, false, true)), None);
        // No Win: typing.
        assert_eq!(act_of(0x31, NONE), None);
        // The number pad's digits are not Windows' taskbar's.
        assert_eq!(act_of(0x61, WIN), None);
    }

    #[test]
    fn win_t_and_win_b_and_nothing_else() {
        assert_eq!(act_of(0x54, WIN), Some(Act::Focus { tray: false, back: false }));
        assert_eq!(act_of(0x54, with(true, false, false)), Some(Act::Focus { tray: false, back: true }));
        assert_eq!(act_of(0x42, WIN), Some(Act::Focus { tray: true, back: false }));
        // Win+Alt+B (HDR), Win+Ctrl+Shift+B (the display driver started again): Windows'.
        assert_eq!(act_of(0x42, with(false, false, true)), None);
        assert_eq!(act_of(0x42, with(true, true, false)), None);
        // Win+D, Win+E, Win+Shift+S, Win+Space, Win+Tab, Win+L, Win+A: Windows'.
        for vk in [0x44, 0x45, 0x53, 0x20, 0x09, 0x4C, 0x41] {
            assert_eq!(act_of(vk, WIN), None, "{vk:#x}");
            assert_eq!(decide(vk, false, with(true, false, false), 0).0, Verdict::Pass, "{vk:#x}");
        }
    }

    #[test]
    fn a_press_taken_is_taken_to_its_release_and_win_never() {
        // Win down: passed, whatever.
        let (v, held) = decide(0x5B, false, NONE, 0);
        assert_eq!((v, held), (Verdict::Pass, 0));
        // Win+2: acts, the mask key with it.
        let (v, held) = decide(0x32, false, WIN, held);
        assert_eq!(v, Verdict::Act(Act::App(1, How::Press)));
        assert!(masks(v));
        assert_ne!(held, 0);
        // Held down, it repeats: taken, nothing done, no mask.
        let (v, held) = decide(0x32, false, WIN, held);
        assert_eq!(v, Verdict::Take);
        assert!(!masks(v));
        // Its release: taken.
        let (v, held) = decide(0x32, true, WIN, held);
        assert_eq!(v, Verdict::Take);
        assert!(!masks(v));
        assert_eq!(held, 0);
        // Win up: passed (Windows sees it; the mask key went before it).
        assert_eq!(decide(0x5B, true, NONE, held), (Verdict::Pass, 0));
        // RWin as LWin.
        assert_eq!(decide(0x5C, false, NONE, 0).0, Verdict::Pass);
    }

    #[test]
    fn a_release_after_win_went_up_and_typing_after() {
        let (_, held) = decide(0x31, false, WIN, 0);
        // Win let go first: none held any more; the digit's release is the window's.
        let (_, held) = decide(0x5B, true, NONE, held);
        assert_eq!(held, 0);
        assert_eq!(decide(0x31, true, NONE, held).0, Verdict::Pass);
        // A release missed (the secure desktop between): typing the digit
        // without Win is typing, and it is no longer held.
        let (_, held) = decide(0x31, false, WIN, 0);
        let (v, held) = decide(0x31, false, NONE, held);
        assert_eq!((v, held), (Verdict::Pass, 0));
        // Plain typing never touches what is held.
        assert_eq!(decide(0x35, false, NONE, 0), (Verdict::Pass, 0));
        assert_eq!(decide(0x41, false, WIN, 0b1), (Verdict::Pass, 0b1));
    }

    #[test]
    fn acts_go_through_a_message_and_back() {
        for slot in 0..10 {
            for how in HOWS {
                assert_eq!(decode(encode(Act::App(slot, how))), Some(Act::App(slot, how)));
            }
        }
        for tray in [false, true] {
            for back in [false, true] {
                assert_eq!(decode(encode(Act::Focus { tray, back })), Some(Act::Focus { tray, back }));
            }
        }
        assert_eq!(decode(0x0A), None);
        assert_eq!(decode(0x50), None);
    }

    #[test]
    fn in_the_order_her_strip_shows() {
        let list = json!([{ "app": "a" }, { "app": "b" }, { "app": "c" }, { "app": "d" }]);
        let keys = |v: Vec<Value>| v.iter().map(|a| a["app"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        assert_eq!(keys(ordered(&list, &[])), ["a", "b", "c", "d"]);
        // Dragged: c first, then a; the rest as they came; a key no longer there skipped.
        let order = ["c".to_string(), "gone".to_string(), "a".to_string()];
        assert_eq!(keys(ordered(&list, &order)), ["c", "a", "b", "d"]);
        assert!(ordered(&Value::Null, &order).is_empty());
    }

    #[test]
    fn what_a_press_does_to_a_programs_windows() {
        // Not running: started; so too for another of it, whatever runs.
        assert_eq!(target(How::Press, &[], 0, &[]), Target::Start);
        assert_eq!(target(How::Last, &[], 0, &[]), Target::Start);
        assert_eq!(target(How::New, &[5], 5, &[5]), Target::Start);
        assert_eq!(target(How::Admin, &[5], 5, &[5]), Target::Start);
        assert_eq!(target(How::Jump, &[5], 0, &[5]), Target::Nothing);
        // One window: as a press on its button (to the front, or minimized in front).
        assert_eq!(target(How::Press, &[5], 9, &[5]), Target::Toggle(5));
        assert_eq!(target(How::Press, &[5], 5, &[5]), Target::Toggle(5));
        // Several, none in front: the one last in front (on top of them).
        assert_eq!(target(How::Press, &[1, 2, 3], 9, &[9, 3, 1, 2]), Target::Bring(3));
        // One in front: the next, round again after the last.
        assert_eq!(target(How::Press, &[1, 2, 3], 1, &[1, 3, 2]), Target::Bring(2));
        assert_eq!(target(How::Press, &[1, 2, 3], 3, &[3, 1, 2]), Target::Bring(1));
        // Ctrl: the one last in front, not again when it is in front.
        assert_eq!(target(How::Last, &[1, 2, 3], 9, &[2, 9, 3]), Target::Bring(2));
        assert_eq!(target(How::Last, &[1, 2, 3], 2, &[2, 3]), Target::Nothing);
        // None of them found lying anywhere (gone since): the first listed.
        assert_eq!(target(How::Last, &[1, 2], 9, &[]), Target::Bring(1));
    }

    #[test]
    fn the_keys_as_the_log_says_them() {
        assert_eq!(said(Act::App(0, How::Press)), "Win+1");
        assert_eq!(said(Act::App(9, How::New)), "Win+Shift+0");
        assert_eq!(said(Act::App(3, How::Admin)), "Win+Ctrl+Shift+4");
        assert_eq!(said(Act::Focus { tray: false, back: true }), "Win+Shift+T");
        assert_eq!(said(Act::Focus { tray: true, back: false }), "Win+B");
    }
}
