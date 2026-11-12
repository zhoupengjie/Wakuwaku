// The tray taken over (systray/): its icons drawn into PNGs and sent to
// the page (taskbar:tray), what Windows keeps out on its taskbar, a press on
// one handed to its program, programs asked again for their icons (claim,
// ask_soon).
use super::*;

// Asking programs for their icons: from this tick (0: asked), the last
// time, and how many times right after one.
pub(super) static ASK_AT: AtomicU32 = AtomicU32::new(0);
pub(super) static LAST_ASK: AtomicU32 = AtomicU32::new(0);
pub(super) static QUICK_ASKS: AtomicU32 = AtomicU32::new(0);
// The list sent last (a new page is sent it whole: resend).
pub(super) static SENT: Lock<Value> = Lock::new("taskbar::win::tray::SENT", Value::Null);

// The icons' pictures, the taskbar's thread's alone (send_icons runs there,
// on WM_TRAY_CHANGED; a new page's resend asks it, WM_RESEND_TRAY), gone
// with it: each icon drawn, (key, its handle) to its PNG; which picture
// (its handle) the page has for each, sent again only when it is another.
#[derive(Default)]
struct Pictures {
    pngs: HashMap<u64, (isize, String)>,
    sent: HashMap<u64, isize>,
}

thread_local! {
    static PICTURES: std::cell::RefCell<Pictures> = std::cell::RefCell::new(Pictures::default());
}
// Windows' own record of the tray icons it keeps out (windows_kept), read once.
pub(super) static KEPT: Lock<Option<Vec<Kept>>> = Lock::new("taskbar::win::tray::KEPT", None);

// The tray's icons and the buttons as last sent, for a page that has just come up.
pub fn resend(sh: &Shared) {
    // The tray's by its thread, whose its pictures are.
    let ours = OURS.load(Ordering::SeqCst);
    if ours != 0 {
        // SAFETY: the thread's own window; a posted message, no pointers.
        unsafe { PostMessageW(ours as Hwnd, WM_RESEND_TRAY, 0, 0) };
    }
    let windows = SENT_WINDOWS.lock().unwrap().clone();
    if !windows.is_null() {
        let _ = sh.app.emit_to("island", "taskbar:windows", crate::bars::for_main(sh, &windows));
    }
    let keys = SENT_KEYS.lock().unwrap().clone();
    if !keys.is_null() {
        send_ime_icons(sh, true);
        let _ = sh.app.emit_to("island", "taskbar:keys", keys);
    }
    let paper = PAPER.lock().unwrap().clone();
    if !paper.is_null() {
        let _ = sh.app.emit_to("island", "taskbar:wallpaper", paper);
    }
    let look = LOOK.lock().unwrap().clone();
    if !look.is_null() {
        let _ = sh.app.emit_to("island", "taskbar:look", look);
    }
}

// A press on a tray icon in the page, told to its program. The press
// itself brings the home's window to the front first, as the taskbar
// comes there when pressed, so it may hand the front on (a menu not in
// front does not close when you press elsewhere).
pub fn press(sh: &Shared, key: u64, press: &str, home_hwnd: isize) -> bool {
    let press = match press {
        "leftDown" => Press::LeftDown,
        "leftUp" => Press::LeftUp,
        "rightDown" => Press::RightDown,
        "rightUp" => Press::RightUp,
        "double" => Press::Double,
        "move" => Press::Move,
        "in" => Press::HoverIn,
        "out" => Press::HoverOut,
        _ => return false,
    };
    if matches!(press, Press::LeftDown | Press::RightDown) && home_hwnd != 0 {
        close_flyout();
        // SAFETY: our own window; the press is the input that lets it.
        unsafe { SetForegroundWindow(home_hwnd as Hwnd) };
    }
    let mut at = Point::default();
    // SAFETY: our own out-parameter.
    unsafe { GetCursorPos(&mut at) };
    let told = systray::tell(key, press, (at.x, at.y));
    for line in systray::notes() {
        sh.log(&format!("taskbar: {line}"));
    }
    told
}

// An app bar's place asked of Explorer itself, ours set aside for it
// (systray/ explorer_first): its own (island.rs, bars.rs) never
// reached it by ours. The programs asked for their icons soon after,
// should one have spoken to Explorer's alone meanwhile.
pub fn claim<R>(f: impl FnOnce() -> R) -> R {
    let (r, aside) = systray::explorer_first(f);
    if aside {
        ask_soon();
    }
    r
}

// Explorer's window came first for a moment: what programs handed the
// tray then went to it. They are asked again once it settles: at most
// every ten seconds, but right away (twice at most) when it came first
// while they were answering.
pub(super) fn ask_soon() {
    let (ticks, last) = (TICKS.load(Ordering::SeqCst), LAST_ASK.load(Ordering::SeqCst));
    if ASK_AT.load(Ordering::SeqCst) != 0 {
        return;
    }
    let answering = ticks <= last + 6;
    if !answering {
        QUICK_ASKS.store(0, Ordering::SeqCst);
    }
    if (answering && QUICK_ASKS.fetch_add(1, Ordering::SeqCst) < 2) || ticks >= last + 20 {
        ASK_AT.store(ticks + SETTLE_TICKS, Ordering::SeqCst);
    }
}

// Where Windows' own taskbar keeps each tray icon: Control Panel\
// NotifyIconSettings, a key per icon with its program's path, its uID
// (or IconGuid, for one named by a guid) and IsPromoted 1 when kept out
// on the taskbar (folded away otherwise). (file name, uID, guid, kept
// out), read once while the taskbar is up.
pub(super) type Kept = (String, Option<u32>, Option<[u8; 16]>, bool);

pub(super) fn windows_kept() -> Vec<Kept> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let mut kept = KEPT.lock().unwrap();
    if let Some(list) = kept.as_ref() {
        return list.clone();
    }
    let list: Vec<Kept> = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Control Panel\NotifyIconSettings")
        .map(|root| {
            root.enum_keys()
                .flatten()
                .filter_map(|name| {
                    let key = root.open_subkey(&name).ok()?;
                    let path: String = key.get_value("ExecutablePath").ok()?;
                    let file = path.rsplit('\\').next().unwrap_or(&path).to_lowercase();
                    let promoted: u32 = key.get_value("IsPromoted").unwrap_or(0);
                    let guid = key.get_value::<String, _>("IconGuid").ok().and_then(|g| systray::guid_bytes(&g));
                    Some((file, key.get_value("UID").ok(), guid, promoted != 0))
                })
                .collect()
        })
        .unwrap_or_default();
    *kept = Some(list.clone());
    list
}

// Whether Windows keeps this icon out on its taskbar: its program's, by
// its guid or its uID when the program has several.
pub(super) fn kept_out(kept: &[Kept], exe: &str, uid: u32, guid: Option<[u8; 16]>) -> bool {
    let exe = exe.to_lowercase();
    let its: Vec<&Kept> = kept.iter().filter(|k| k.0 == exe).collect();
    let this = match guid {
        Some(g) => its.iter().find(|k| k.2 == Some(g)),
        None => its.iter().find(|k| k.2.is_none() && k.1 == Some(uid)),
    };
    this.or(its.first()).is_some_and(|k| k.3)
}

#[cfg(test)]
mod kept_tests {
    use super::systray;

    #[test]
    fn windows_placement_is_found_by_guid_too() {
        let a = systray::guid_bytes("{D36B22A6-995E-4340-8339-3DE08C6840DF}");
        let b = systray::guid_bytes("{D68540CC-E0D8-4298-B55C-E6E324121F77}");
        let kept = vec![("nvdisplay.container.exe".to_string(), None, a, false), ("nvdisplay.container.exe".to_string(), None, b, true), ("everything.exe".to_string(), Some(0), None, true)];
        assert!(super::kept_out(&kept, "NVDisplay.Container.exe", 0, b));
        assert!(!super::kept_out(&kept, "NVDisplay.Container.exe", 0, a));
        assert!(super::kept_out(&kept, "Everything.exe", 0, None));
        assert!(!super::kept_out(&kept, "other.exe", 0, None));
    }
}

// The icons as the page draws them: each drawn once per handle, and sent
// The whole list, pictures and all, to a page just come up (resend): it
// keeps none yet. On the taskbar's thread.
pub(super) fn resend_tray() {
    let Some(sh) = sh() else { return };
    let sent = SENT.lock().unwrap().clone();
    if !sent.is_null() {
        PICTURES.with(|p| p.borrow_mut().sent.clear());
        let _ = sh.app.emit_to("island", "taskbar:tray", sent);
    }
}

// to the page once per handle too (the page keeps them, island.ts
// onTray): a tip changing (Task Manager's every second) sends the tips,
// not every picture again.
pub(super) fn send_icons() {
    let Some(sh) = sh() else { return };
    for line in systray::notes() {
        sh.log(&format!("taskbar: {line}"));
    }
    let size = icon_px(&sh);
    let shown = systray::shown();
    let kept = windows_kept();
    let (whole, light): (Vec<Value>, Vec<Value>) = {
        // This thread's own (no lock); drawing takes none of hers.
        let mut pictures = PICTURES.with(|p| std::mem::take(&mut *p.borrow_mut()));
        let Pictures { pngs, sent } = &mut pictures;
        pngs.retain(|k, _| shown.iter().any(|s| s.key == *k));
        sent.retain(|k, _| shown.iter().any(|s| s.key == *k));
        let made = shown
            .iter()
            .map(|s| {
                // One only Explorer's tray holds: Windows' own picture of it.
                let png = if !s.snapshot.is_empty() {
                    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(&s.snapshot))
                } else if s.icon == 0 {
                    String::new()
                } else {
                    match pngs.get(&s.key) {
                        Some((h, png)) if *h == s.icon => png.clone(),
                        _ => {
                            let png = png_of(s.icon, size).unwrap_or_default();
                            pngs.insert(s.key, (s.icon, png.clone()));
                            png
                        }
                    }
                };
                let out = s.out.unwrap_or_else(|| kept_out(&kept, &s.exe, s.uid, s.guid));
                let item = json!({ "key": s.key, "png": png, "tip": s.tip, "exe": s.exe, "name": s.name, "windowsOut": out, "reach": s.reach });
                let mut lite = item.clone();
                if sent.get(&s.key) == Some(&s.icon) {
                    if let Some(o) = lite.as_object_mut() {
                        o.remove("png");
                    }
                }
                sent.insert(s.key, s.icon);
                (item, lite)
            })
            .unzip();
        PICTURES.with(|p| *p.borrow_mut() = pictures);
        made
    };
    *SENT.lock().unwrap() = Value::Array(whole);
    let _ = sh.app.emit_to("island", "taskbar:tray", Value::Array(light));
}

// An icon drawn as a PNG (a data: URL), size px across. Drawn twice, on
// black and on white: how much the white shows through is how clear each
// pixel is, for icons with an alpha channel and those with a mask alike.
#[repr(C)]
pub(super) struct IconInfo {
    is_icon: i32,
    hot_x: u32,
    hot_y: u32,
    mask: isize,
    color: isize,
}

#[repr(C)]
pub(super) struct BitmapObj {
    pub(super) kind: i32,
    pub(super) width: i32,
    pub(super) height: i32,
    pub(super) width_bytes: i32,
    pub(super) planes: u16,
    pub(super) bits_pixel: u16,
    pub(super) bits: *mut c_void,
}

#[link(name = "user32")]
extern "system" {
    pub(super) fn GetIconInfo(icon: isize, info: *mut IconInfo) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    pub(super) fn GetObjectW(obj: isize, size: i32, out: *mut c_void) -> i32;
}

// An icon's own size, its picture's (half its mask's for a two-colour one).
pub(super) fn icon_size(icon: isize) -> Option<(i32, i32)> {
    let mut info = IconInfo { is_icon: 0, hot_x: 0, hot_y: 0, mask: 0, color: 0 };
    // SAFETY: our own structs; the bitmaps the call makes are deleted.
    unsafe {
        if GetIconInfo(icon, &mut info) == 0 {
            return None;
        }
        let mut bm = BitmapObj { kind: 0, width: 0, height: 0, width_bytes: 0, planes: 0, bits_pixel: 0, bits: std::ptr::null_mut() };
        let of = if info.color != 0 { info.color } else { info.mask };
        let ok = GetObjectW(of, std::mem::size_of::<BitmapObj>() as i32, &mut bm as *mut BitmapObj as *mut c_void) != 0;
        for b in [info.color, info.mask] {
            if b != 0 {
                DeleteObject(b);
            }
        }
        let h = if info.color != 0 { bm.height } else { bm.height / 2 };
        (ok && bm.width > 0 && h > 0).then_some((bm.width, h))
    }
}

// Drawn at its own size, then made `size` square smoothly (super::resize):
// drawn straight at another, Windows stretches it pixel by pixel, which
// shows as jagged edges (a 32px icon at 30, a 16px one at 20).
pub(super) fn png_of(icon: isize, size: i32) -> Option<String> {
    if size <= 0 {
        return None;
    }
    let (w, h) = icon_size(icon).filter(|&(w, h)| w <= 256 && h <= 256).unwrap_or((size, size));
    let n = (w * h) as usize;
    let draw = |fill: u8| -> Option<Vec<u8>> {
        // SAFETY: a DIB section of our own, drawn into and read, then deleted.
        unsafe {
            let dc = CreateCompatibleDC(0);
            let info = BitmapInfoHeader { size: 40, width: w, height: -h, planes: 1, bit_count: 32, compression: 0, size_image: 0, x_ppm: 0, y_ppm: 0, clr_used: 0, clr_important: 0 };
            let mut bits: *mut u8 = std::ptr::null_mut();
            let bmp = CreateDIBSection(dc, &info, 0, &mut bits, 0, 0);
            if bmp == 0 || bits.is_null() {
                DeleteDC(dc);
                return None;
            }
            let old = SelectObject(dc, bmp);
            std::ptr::write_bytes(bits, fill, n * 4);
            DrawIconEx(dc, 0, 0, icon, w, h, 0, 0, DI_NORMAL);
            let pixels = std::slice::from_raw_parts(bits, n * 4).to_vec();
            SelectObject(dc, old);
            DeleteObject(bmp);
            DeleteDC(dc);
            Some(pixels)
        }
    };
    let (black, white) = (draw(0)?, draw(255)?);
    let mut rgba = vec![0u8; n * 4];
    for i in 0..n {
        let (b, w) = (&black[i * 4..i * 4 + 3], &white[i * 4..i * 4 + 3]);
        let through = (0..3).map(|c| w[c].saturating_sub(b[c])).max().unwrap_or(255);
        let alpha = 255 - through;
        if alpha == 0 {
            continue;
        }
        // On black, a pixel is its colour times its alpha (BGR to RGB).
        for (c, from) in [(0, 2), (1, 1), (2, 0)] {
            rgba[i * 4 + c] = ((b[from] as u32 * 255 + alpha as u32 / 2) / alpha as u32).min(255) as u8;
        }
        rgba[i * 4 + 3] = alpha;
    }
    let rgba = if (w, h) == (size, size) { rgba } else { crate::taskbar::resize(&rgba, w as usize, h as usize, size as usize, size as usize) };
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, size as u32, size as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&rgba).ok()?;
    }
    Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(out)))
}
