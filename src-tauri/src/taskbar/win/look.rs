// Windows' look, for the strip to be as Windows' own taskbar is: the
// desktop's picture under it (for its Mica), light or dark, the accent,
// end task on the buttons' menus, the clock's seconds, Task View; sent to the
// page as they change (taskbar:wallpaper, taskbar:look).
//
// Looked at on a thread of its own (look_loop), one look at a time:
// what it sent last is that thread's alone. Others ask it (send_look,
// forget_look: a LookAsk over a channel) and read what it sent as it
// published it (LOOK, PAPER, never written by another).
use std::cell::Cell;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::OnceLock;

use super::*;

// What the look's thread is asked.
pub(super) enum LookAsk {
    // Looked at (again, should it be looking now: the picture it read
    // may be the one before the change).
    Look,
    // Windows' taskbar given back: what it sent forgotten, so all of it
    // is sent anew once the strip is hers again.
    Forget,
}

// The way to the look's thread, started at the first ask and there for
// good (a thread asleep on its channel).
static TO_LOOK: OnceLock<Sender<LookAsk>> = OnceLock::new();
// The desktop's picture under the strip and Windows' look as the look's
// thread sent them last, published by it (a new page's resend, bars.rs).
pub(super) static PAPER: Lock<Option<Arc<Value>>> = Lock::ranked("taskbar::win::look::PAPER", crate::lock::LEAF, None);
pub(super) static LOOK: Lock<Option<Arc<Value>>> = Lock::ranked("taskbar::win::look::LOOK", crate::lock::LEAF, None);

thread_local! {
    // The looks left after Windows' word (settle): the taskbar's
    // thread's, whose timer counts them; gone with it.
    static LOOKS_LEFT: Cell<u32> = const { Cell::new(0) };
}

// Windows' look as last sent: light or dark, the accent (bars.rs).
pub fn look_value() -> Value {
    let look = LOOK.lock().unwrap().clone();
    look.map_or(Value::Null, |look| (*look).clone())
}

// The display the strip is on (physical), and its scale.
pub(super) fn monitor_of(strip: (i32, i32, i32, i32)) -> ((i32, i32, i32, i32), f64) {
    let mut info = MonitorInfo { size: std::mem::size_of::<MonitorInfo>() as u32, ..Default::default() };
    let (mut dx, mut dy) = (96u32, 96u32);
    // SAFETY: our own out-parameters, of the sizes the calls write.
    unsafe {
        let mon = MonitorFromPoint(Point { x: strip.0 + strip.2 / 2, y: strip.1 + strip.3 / 2 }, MONITOR_DEFAULTTONEAREST);
        GetMonitorInfoW(mon, &mut info);
        GetDpiForMonitor(mon, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
    }
    let r = &info.monitor;
    ((r.left, r.top, r.right - r.left, r.bottom - r.top), dx.max(1) as f64 / 96.0)
}

// The picture on the strip's display (wallpaper.rs), sent when it changes
// (taskbar:wallpaper): its file (by the asset protocol, allowed for it;
// its last change after it, so a new picture under the same name is
// loaded anew), how it is laid, the colour round it, and where the
// display and all of them together are from the strip's top-left (the
// page's px). With it Windows' look (taskbar:look: { mode: dark | light,
// accent: { base, light, dark } }), for the strip to be as Windows' own
// taskbar is; the live pictures' window's ground with it (thumbs_ground).
// Looked at when the strip moves, when Windows says a setting or its
// colours changed (and each half second for five after: settle), and
// every half minute (a slideshow, Spotlight); on a thread of its own,
// one at a time (send_look): Windows' answer goes through Explorer,
// which may be busy, and the tray's messages come to this one.
// How many times Windows flashes a taskbar button for a window asking
// for attention (Control Panel\Desktop\ForegroundFlashCount, a string;
// 7 unless set; 0: until the window comes to the front).
pub(super) fn flash_count() -> u32 {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Control Panel\Desktop");
    let text = key.as_ref().ok().and_then(|k| k.get_value::<String, _>("ForegroundFlashCount").ok()).and_then(|s| s.trim().parse::<u32>().ok());
    let number = || key.as_ref().ok().and_then(|k| k.get_value::<u32, _>("ForegroundFlashCount").ok());
    text.or_else(number).unwrap_or(7)
}

// Windows' End task on a program's button (Settings → System → For
// developers → End task), as its settings page keeps it:
// TaskbarDeveloperSettings\TaskbarEndTask, a number. Off unless set.
pub fn end_task_on() -> bool {
    use winreg::enums::HKEY_CURRENT_USER;
    let key = winreg::RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced\TaskbarDeveloperSettings");
    crate::taskbar::set_on(key.ok().and_then(|k| k.get_value::<u32, _>("TaskbarEndTask").ok()))
}

// Windows' seconds in the taskbar's clock (Settings → Personalization →
// Taskbar → Taskbar behaviors → Show seconds in system tray clock), as
// its settings page keeps it: Explorer\Advanced\ShowSecondsInSystemClock,
// a number. Off unless set.
pub(super) fn seconds_on() -> bool {
    use winreg::enums::HKEY_CURRENT_USER;
    let key = winreg::RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced");
    crate::taskbar::set_on(key.ok().and_then(|k| k.get_value::<u32, _>("ShowSecondsInSystemClock").ok()))
}

// Windows' Task View button (Settings → Personalization → Taskbar →
// Taskbar items → Task view), as its settings page keeps it:
// Explorer\Advanced\ShowTaskViewButton, a number. On unless set to 0, as
// Windows has it by default.
pub(super) fn task_view_on() -> bool {
    use winreg::enums::HKEY_CURRENT_USER;
    let key = winreg::RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced");
    key.ok().and_then(|k| k.get_value::<u32, _>("ShowTaskViewButton").ok()) != Some(0)
}

// From outside (bars.rs, a bar's page ready): the desktop looked at now.
pub fn look_now() {
    send_look();
}

// From outside (settings.rs, the wallpaper changed by her own settings:
// Windows says nothing of a change through IDesktopWallpaper): looked at
// now and until the picture is in place, as after Windows' word.
pub fn paper_set() {
    let ours = OURS.load(Ordering::SeqCst);
    if ours == 0 {
        send_look();
        return;
    }
    // SAFETY: a posted message to the host's own window, no pointers.
    unsafe { PostMessageW(ours as Hwnd, WM_PAPER_SET, 0, 0) };
}

// Asked while a look is under way (which may have read the picture
// before it changed): once more after it, not dropped; all asked
// meanwhile answered by that one more.
pub(super) fn send_look() {
    ask_look(LookAsk::Look);
}

pub(super) fn forget_look() {
    ask_look(LookAsk::Forget);
}

fn ask_look(ask: LookAsk) {
    let to = TO_LOOK.get_or_init(|| {
        let (to, from) = std::sync::mpsc::channel();
        crate::spawn_named("taskbar look", move || look_loop(from));
        to
    });
    let _ = to.send(ask);
}

// What the look's thread sent last.
#[derive(Default)]
pub(super) struct Sent {
    look: Value,
    paper: Value,
}

fn look_loop(asks: Receiver<LookAsk>) {
    let mut sent = Sent::default();
    while let Ok(ask) = asks.recv() {
        let mut look = false;
        for ask in std::iter::once(ask).chain(asks.try_iter()) {
            match ask {
                LookAsk::Look => look = true,
                LookAsk::Forget => {
                    sent = Sent::default();
                    *PAPER.lock().unwrap() = None;
                    *LOOK.lock().unwrap() = None;
                }
            }
        }
        if look {
            look_at_desktop(&mut sent);
        }
    }
}

// Windows' word that a setting changed, before the picture it speaks of
// is in place (wallpaper.rs SETTLE_MS): looked at now and every half
// second for five seconds, on the host's thread (its window's timer).
pub(super) fn settle(hwnd: Hwnd) {
    send_look();
    LOOKS_LEFT.set(wallpaper::SETTLE_LOOKS);
    // SAFETY: our own window's timer, on its thread; set again, it starts over.
    unsafe { SetTimer(hwnd, SETTLE, wallpaper::SETTLE_MS, std::ptr::null()) };
}

pub(super) fn settling(hwnd: Hwnd) {
    let left = LOOKS_LEFT.get().saturating_sub(1);
    LOOKS_LEFT.set(left);
    if left == 0 {
        // SAFETY: our own window's timer, on its thread.
        unsafe { KillTimer(hwnd, SETTLE) };
    }
    send_look();
}

pub(super) fn look_at_desktop(sent: &mut Sent) {
    let Some(sh) = sh() else { return };
    let (light, accent) = wallpaper::look();
    thumbs_ground(light);
    let look = json!({
        "mode": if light { "light" } else { "dark" },
        "accent": accent.map(|a| json!({ "base": a.base, "light": a.light, "dark": a.dark })),
        "flashes": flash_count(),
        // Whether a program's menu offers End task (island-winlist.ts drawPop).
        "endTask": end_task_on(),
        // Whether the clock shows the seconds (island-look.ts tickClock, bar.ts).
        "seconds": seconds_on(),
        // Whether Task View has a button, after Start (island.ts, bar.ts).
        "taskView": task_view_on(),
    });
    if sent.look != look {
        log(&format!("look: {look}"));
        sent.look = look.clone();
        *LOOK.lock().unwrap() = Some(Arc::new(look.clone()));
        let _ = sh.app.emit_to("island", "taskbar:look", look);
    }
    // Her taskbar on the other displays: each one's wallpaper too, for its Mica.
    crate::bars::read_papers(&sh);
    let Some(strip) = strip_now() else { return };
    let (mon, sf) = monitor_of(strip);
    let Some(paper) = wallpaper::read(mon) else { return };
    // SAFETY: plain reads.
    let screen = unsafe { (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_XVIRTUALSCREEN + 1), GetSystemMetrics(SM_XVIRTUALSCREEN + 2), GetSystemMetrics(SM_XVIRTUALSCREEN + 3)) };
    let rel = |(x, y, w, h): (i32, i32, i32, i32)| json!([(x - strip.0) as f64 / sf, (y - strip.1) as f64 / sf, w as f64 / sf, h as f64 / sf]);
    let url = paper.file.as_ref().map(|f| format!("{}?v={}", crate::data::asset_url(f), paper.stamp));
    let now = json!({ "url": url, "position": paper.position, "color": paper.color, "mon": rel(mon), "screen": rel(screen), "sf": sf });
    if sent.paper == now {
        return;
    }
    if let Some(file) = &paper.file {
        let _ = sh.app.asset_protocol_scope().allow_file(file);
    }
    log(&format!("wallpaper: {:?}, {}, {}", paper.file, paper.position, paper.color));
    sent.paper = now.clone();
    *PAPER.lock().unwrap() = Some(Arc::new(now.clone()));
    let _ = sh.app.emit_to("island", "taskbar:wallpaper", now);
}
