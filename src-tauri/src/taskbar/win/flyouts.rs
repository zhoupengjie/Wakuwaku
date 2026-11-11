// Windows' own pieces the strip opens or stands by: its flyouts (the quick
// settings, the calendar, the notifications, …) opened and closed, the
// volume, Start opened and seen, the desktop shown, and the Recycle Bin for
// the dock.
use super::*;

// Windows' own Start (or search, the notifications, the quick settings)
// in front: closed first, as a press on Windows' taskbar closes it. Ours
// takes no focus away from it, and the window pressed for could not come
// up past it (it holds the foreground). Esc to it, and a moment for it to
// go. Whether there was one. The dock's menu too (bars.rs bar_pop).
pub fn close_flyout() -> bool {
    let front = tasks::front();
    if !tasks::is_shell_flyout(front) {
        return false;
    }
    let key = |up: bool| Input {
        kind: 1,
        u: InputUnion { ki: KeybdInput { vk: VK_ESCAPE, scan: 0, flags: if up { KEYEVENTF_KEYUP } else { 0 }, time: 0, extra: 0 } },
    };
    let inputs = [key(false), key(true)];
    // Esc goes to whatever is in front: looked at again right before it
    // goes (telling the program took a moment), so it never lands in
    // another (in a terminal it stops what runs there).
    if tasks::front() != front {
        return false;
    }
    // SAFETY: an array of INPUTs of the size passed.
    unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<Input>() as i32) };
    let asked = Instant::now();
    while tasks::front() == front && asked.elapsed() < Duration::from_millis(400) {
        std::thread::sleep(Duration::from_millis(20));
    }
    true
}

// What the taskbar's own buttons open, by the keys that open them.
pub fn open(what: &str) -> bool {
    // The input method of the window in front: its own script or plain
    // letters; its mark read again at once (keys_loop).
    if what == "ime" {
        let done = tasks::toggle_native();
        KEYS_NOW.store(true, Ordering::SeqCst);
        wake_keys();
        return done;
    }
    // A press on what shows the microphone, camera or location in use:
    // its page in Windows' privacy settings. From the input methods' list,
    // Windows' languages and their keyboards. From the menus of a right
    // press on the volume, the network and the clock (tray.rs
    // part_menu), what Windows' own open (SystemTray.dll): the volume
    // mixer, the sound settings, Get Help's network troubleshooter, the
    // network settings, the date and time, the notifications.
    let page = match what {
        "privacy-mic" => Some("ms-settings:privacy-microphone"),
        "privacy-cam" => Some("ms-settings:privacy-webcam"),
        "privacy-loc" => Some("ms-settings:privacy-location"),
        "languages" => Some("ms-settings:regionlanguage"),
        "mixer" => Some("ms-settings:apps-volume"),
        "sound" => Some("ms-settings:sound"),
        "netFix" => Some("ms-contact-support://?ActivationType=NetworkDiagnostics&invoker=SystemTrayIcon"),
        "network" => Some("ms-settings:network"),
        "dateTime" => Some("ms-settings:dateandtime"),
        "notifySettings" => Some("ms-settings:notifications"),
        // The dock's Recycle Bin (split).
        "recycleBin" => Some("shell:RecycleBinFolder"),
        _ => None,
    };
    if let Some(page) = page {
        return tasks::launch(page);
    }
    // The Recycle Bin emptied, as its own menu does: Windows asks first
    // and shows its progress; off this thread, as it waits for the answer.
    if what == "emptyBin" {
        std::thread::spawn(|| {
            #[link(name = "shell32")]
            extern "system" {
                fn SHEmptyRecycleBinW(hwnd: Hwnd, root: *const u16, flags: u32) -> i32;
            }
            // SAFETY: no window, every drive's bin, Windows' own dialogs.
            let done = unsafe { SHEmptyRecycleBinW(std::ptr::null_mut(), std::ptr::null(), 0) };
            log(&format!("the Recycle Bin emptied: {}", if done == 0 { "yes" } else { "no (not asked to, or none in it)" }));
        });
        return true;
    }
    // Get Help's sound troubleshooter; where it cannot be opened, the
    // old one, as Windows falls back to it.
    if what == "soundFix" {
        let msdt = format!("{}\\System32\\msdt.exe", std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into()));
        return tasks::launch("ms-contact-support://windows-speaker-icon") || tasks::launch_with(&msdt, "-id AudioPlaybackDiagnostic -skip true -ep SndVolTrayMenu");
    }
    // Search pressed again while it is open: closed, as Windows' own
    // search button does (Win+S again left it open: the test session).
    if what == "search" && tasks::is_search(tasks::front()) {
        return close_flyout();
    }
    let keys: &[u16] = match what {
        "start" => &[VK_LWIN],
        "search" => &[VK_LWIN, b'S' as u16],
        "tasks" => &[VK_LWIN, VK_TAB],
        "widgets" => &[VK_LWIN, b'W' as u16],
        "desktop" => &[VK_LWIN, b'D' as u16],
        "quick" => &[VK_LWIN, b'A' as u16],
        "notifications" => &[VK_LWIN, b'N' as u16],
        // The input methods to pick from, as Win+Space has them.
        "inputs" => &[VK_LWIN, VK_SPACE],
        _ => return false,
    };
    let key = |vk: u16, up: bool| Input {
        kind: 1,
        u: InputUnion { ki: KeybdInput { vk, scan: 0, flags: if vk == VK_LWIN { KEYEVENTF_EXTENDEDKEY } else { 0 } | if up { KEYEVENTF_KEYUP } else { 0 }, time: 0, extra: 0 } },
    };
    let inputs: Vec<Input> = keys.iter().map(|&k| key(k, false)).chain(keys.iter().rev().map(|&k| key(k, true))).collect();
    // SAFETY: a slice of INPUTs of the size passed.
    unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<Input>() as i32) == inputs.len() as u32 }
}

// The wheel turned on the quick settings' button: the volume that many
// steps up (down, below 0) by Windows' own volume keys, as on Windows'
// own taskbar (zhoupengjie/Wakuwaku#35): 2% a step, its volume flyout
// shown, a muted one unmuted going up. The mark read again once Windows
// has turned it (keys_loop).
pub fn volume(steps: i32) -> bool {
    const VK_VOLUME_DOWN: u16 = 0xAE;
    const VK_VOLUME_UP: u16 = 0xAF;
    let vk = if steps > 0 { VK_VOLUME_UP } else { VK_VOLUME_DOWN };
    let key = |up: bool| Input {
        kind: 1,
        u: InputUnion { ki: KeybdInput { vk, scan: 0, flags: if up { KEYEVENTF_KEYUP } else { 0 }, time: 0, extra: 0 } },
    };
    let inputs: Vec<Input> = (0..steps.unsigned_abs().min(10)).flat_map(|_| [key(false), key(true)]).collect();
    if inputs.is_empty() {
        return false;
    }
    // SAFETY: a slice of INPUTs of the size passed.
    let sent = unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<Input>() as i32) } == inputs.len() as u32;
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(80));
        KEYS_NOW.store(true, Ordering::SeqCst);
        wake_keys();
    });
    sent
}

// The desktop to the front: "show the desktop" (Win+D, four fingers
// down on the touchpad, the sliver at the strip's end) lifts its window
// over every other, hers kept on top too, and her taskbar went with the
// windows (Windows' own stays, in the shell's own band:
// zhoupengjie/Wakuwaku#33). Hers raised over it again, the keyboard left
// with the desktop (or Win+D would not bring the windows back): at once,
// and twice more while its animation ends; the clock's look sees to it
// later on (tick).
pub(super) static RAISING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(super) fn over_desktop() {
    if RAISING.swap(true, Ordering::SeqCst) {
        return;
    }
    log("the desktop to the front: her windows raised over it");
    std::thread::spawn(|| {
        for wait in [0, 300, 500] {
            std::thread::sleep(Duration::from_millis(wait));
            if let Some(sh) = sh() {
                raise_own_with_thumbs(&sh);
            }
        }
        RAISING.store(false, Ordering::SeqCst);
    });
}

// Windows' Start menu opened or closed (its window came to the front,
// or another did): told to the page at once (taskbar:start), for the
// Start button to do as Windows' own does.
pub(super) fn start_seen(open: bool) {
    if START_OPEN.swap(open, Ordering::SeqCst) == open {
        return;
    }
    log(&format!("Windows' Start {}; in front: {}", if open { "open" } else { "closed" }, crate::fullscreen::front()));
    if let Some(sh) = sh() {
        let _ = sh.app.emit_to("island", "taskbar:start", json!({ "open": open }));
        // Split: Start is in the dock, which comes up while it is open (bars.rs).
        crate::bars::start_seen(&sh, open);
    }
}

// The Recycle Bin, for the dock (split): whether anything is in it, and
// Windows' own picture of it so (empty or full: its stock icons), drawn
// once each.
pub fn recycle_bin() -> Value {
    #[repr(C)]
    struct StockIconInfo {
        size: u32,
        icon: isize,
        sys_index: i32,
        icon_index: i32,
        path: [u16; 260],
    }
    #[repr(C)]
    struct BinInfo {
        size: u32,
        bytes: i64,
        items: i64,
    }
    #[link(name = "shell32")]
    extern "system" {
        fn SHGetStockIconInfo(siid: i32, flags: u32, info: *mut StockIconInfo) -> i32;
        fn SHQueryRecycleBinW(root: *const u16, info: *mut BinInfo) -> i32;
    }
    #[link(name = "user32")]
    extern "system" {
        fn DestroyIcon(icon: isize) -> i32;
    }
    const SIID_RECYCLER: i32 = 31;
    const SIID_RECYCLERFULL: i32 = 32;
    const SHGSI_ICON: u32 = 0x100;
    static BIN_PNGS: Mutex<[Option<String>; 2]> = Mutex::new([None, None]);
    let mut info = BinInfo { size: std::mem::size_of::<BinInfo>() as u32, bytes: 0, items: 0 };
    // SAFETY: our own struct, of the size it says; null: every drive's bin.
    let full = unsafe { SHQueryRecycleBinW(std::ptr::null(), &mut info) } == 0 && info.items > 0;
    let mut pngs = BIN_PNGS.lock().unwrap();
    let slot = &mut pngs[full as usize];
    if slot.is_none() {
        let mut stock = StockIconInfo { size: std::mem::size_of::<StockIconInfo>() as u32, icon: 0, sys_index: 0, icon_index: 0, path: [0; 260] };
        // SAFETY: our own struct, of the size it says; the icon is ours to destroy.
        if unsafe { SHGetStockIconInfo(if full { SIID_RECYCLERFULL } else { SIID_RECYCLER }, SHGSI_ICON, &mut stock) } == 0 && stock.icon != 0 {
            *slot = png_of(stock.icon, 32);
            // SAFETY: the icon the call made for us.
            unsafe { DestroyIcon(stock.icon) };
        }
    }
    json!({ "full": full, "png": slot.clone() })
}

// Windows' Start menu open, as last seen (start_seen).
pub fn start_open() -> bool {
    START_OPEN.load(Ordering::SeqCst)
}
