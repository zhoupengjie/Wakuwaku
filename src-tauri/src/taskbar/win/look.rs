// Windows' look, for the strip to be as Windows' own taskbar is: the
// desktop's picture under it (for its Mica), light or dark, the accent,
// end task on the buttons' menus, the clock's seconds, Task View; sent to the
// page as they change (taskbar:wallpaper, taskbar:look).
use super::*;

// The desktop's picture under the strip and Windows' look, as
// sent last (send_look), whether they are being looked at, and whether
// asked again meanwhile; the looks left after Windows' word (settle).
pub(super) static PAPER: Lock<Value> = Lock::new("taskbar::win::look::PAPER", Value::Null);
pub(super) static LOOK: Lock<Value> = Lock::new("taskbar::win::look::LOOK", Value::Null);
pub(super) static LOOK_BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub(super) static LOOK_AGAIN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub(super) static LOOKS_LEFT: AtomicU32 = AtomicU32::new(0);

// Windows' look as last sent: light or dark, the accent (bars.rs).
pub fn look_value() -> Value {
    LOOK.lock().unwrap().clone()
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
// before it changed): once more after it, not dropped. Asked before the
// busy mark is tried, and cleared as each look begins, so a word between
// a look's end and its check is not lost.
pub(super) fn send_look() {
    LOOK_AGAIN.store(true, Ordering::SeqCst);
    if LOOK_BUSY.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| loop {
        LOOK_AGAIN.store(false, Ordering::SeqCst);
        look_at_desktop();
        LOOK_BUSY.store(false, Ordering::SeqCst);
        if !LOOK_AGAIN.load(Ordering::SeqCst) || LOOK_BUSY.swap(true, Ordering::SeqCst) {
            break;
        }
    });
}

// Windows' word that a setting changed, before the picture it speaks of
// is in place (wallpaper.rs SETTLE_MS): looked at now and every half
// second for five seconds, on the host's thread (its window's timer).
pub(super) fn settle(hwnd: Hwnd) {
    send_look();
    LOOKS_LEFT.store(wallpaper::SETTLE_LOOKS, Ordering::SeqCst);
    // SAFETY: our own window's timer, on its thread; set again, it starts over.
    unsafe { SetTimer(hwnd, SETTLE, wallpaper::SETTLE_MS, std::ptr::null()) };
}

pub(super) fn settling(hwnd: Hwnd) {
    let left = LOOKS_LEFT.load(Ordering::SeqCst).saturating_sub(1);
    LOOKS_LEFT.store(left, Ordering::SeqCst);
    if left == 0 {
        // SAFETY: our own window's timer, on its thread.
        unsafe { KillTimer(hwnd, SETTLE) };
    }
    send_look();
}

pub(super) fn look_at_desktop() {
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
    let mut sent = LOOK.lock().unwrap();
    if *sent != look {
        log(&format!("look: {look}"));
        *sent = look.clone();
        let _ = sh.app.emit_to("island", "taskbar:look", look);
    }
    drop(sent);
    // Her taskbar on the other displays: each one's wallpaper too, for its Mica.
    crate::bars::read_papers(&sh);
    let Some(strip) = *STRIP.lock().unwrap() else { return };
    let (mon, sf) = monitor_of(strip);
    let Some(paper) = wallpaper::read(mon) else { return };
    // SAFETY: plain reads.
    let screen = unsafe { (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_XVIRTUALSCREEN + 1), GetSystemMetrics(SM_XVIRTUALSCREEN + 2), GetSystemMetrics(SM_XVIRTUALSCREEN + 3)) };
    let rel = |(x, y, w, h): (i32, i32, i32, i32)| json!([(x - strip.0) as f64 / sf, (y - strip.1) as f64 / sf, w as f64 / sf, h as f64 / sf]);
    let url = paper.file.as_ref().map(|f| format!("{}?v={}", crate::data::asset_url(f), paper.stamp));
    let now = json!({ "url": url, "position": paper.position, "color": paper.color, "mon": rel(mon), "screen": rel(screen), "sf": sf });
    let mut sent = PAPER.lock().unwrap();
    if *sent == now {
        return;
    }
    if let Some(file) = &paper.file {
        let _ = sh.app.asset_protocol_scope().allow_file(file);
    }
    log(&format!("wallpaper: {:?}, {}, {}", paper.file, paper.position, paper.color));
    *sent = now.clone();
    let _ = sh.app.emit_to("island", "taskbar:wallpaper", now);
}
