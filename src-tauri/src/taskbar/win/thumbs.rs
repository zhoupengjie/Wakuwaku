// The windows' live pictures (DWM's thumbnails) over a button's list, on the
// taskbar and the dock, in windows of their own; the dock's badges drawn over
// its icons.
use super::*;
use crate::lock::Lock;

// The live pictures shown (window, thumbnail), over the windows listed above a button.
pub(super) static THUMBS: Lock<Vec<(isize, isize)>> = Lock::new("taskbar::win::thumbs::THUMBS", Vec::new());
pub(super) static THUMB_HOST: AtomicIsize = AtomicIsize::new(0);
// The same for the windows minimized into the dock (split, bars.rs
// show_mins): a window of their own, there as long as the dock is up.
pub(super) static DOCK_THUMBS: Lock<Vec<(isize, isize)>> = Lock::new("taskbar::win::thumbs::DOCK_THUMBS", Vec::new());
pub(super) static DOCK_HOST: AtomicIsize = AtomicIsize::new(0);
// The live pictures' window's ground (COLORREF), as set last (thumbs_ground).
pub(super) static THUMBS_GROUND: AtomicU32 = AtomicU32::new(0x001E_1C1C);

// The live pictures' window as the cards round it are: dark, or light in
// Windows' light mode (style.css .wpop, its ground #1c1c1e or #f9f9f9).
pub(super) fn thumbs_ground(light: bool) {
    let host = THUMB_HOST.load(Ordering::SeqCst);
    let colour: u32 = if light { 0x00F9_F9F9 } else { 0x001E_1C1C };
    if host == 0 || THUMBS_GROUND.swap(colour, Ordering::SeqCst) == colour {
        return;
    }
    // SAFETY: our own window's class; the brush it had, ours, deleted
    // once replaced.
    unsafe {
        let old = SetClassLongPtrW(host as Hwnd, GCLP_HBRBACKGROUND, CreateSolidBrush(colour));
        if old != 0 {
            DeleteObject(old as isize);
        }
        InvalidateRect(host as Hwnd, std::ptr::null(), 1);
    }
}

// Windows' live pictures of windows (DWM thumbnails) where the page left
// room for them (the windows listed above a program's button): each
// window's at (x, y, w, h) on the screen, physical, its shape kept, on
// the window of their own laid over those rooms. None: all taken away.
pub fn thumbs(items: Vec<(isize, (i32, i32, i32, i32))>) {
    place_thumbs(THUMB_HOST.load(Ordering::SeqCst) as Hwnd, &mut THUMBS.lock().unwrap(), items, None);
}

// The windows minimized into the dock, each its live picture where the
// dock's page left room for it (bars.rs show_mins); none, taken away.
// Their window laid over the whole dock (`area`, its window's rect), not
// about the pictures: it never moves as they do, so a picture kept is
// never a frame out of its room. On the main thread, the window's own
// (make_dock_host).
pub fn dock_thumbs(items: Vec<(isize, (i32, i32, i32, i32))>, area: Option<(i32, i32, i32, i32)>) {
    place_thumbs(DOCK_HOST.load(Ordering::SeqCst) as Hwnd, &mut DOCK_THUMBS.lock().unwrap(), items, area);
}

// Each minimized window's program's icon over its live picture's corner
// in the dock (bars.rs show_mins: where its page has it, physical), as
// macOS's: the pictures are DWM's, drawn over the dock in their own
// window, and the page's icon there lay under them; so the icons are a
// window of their own over that one (owned by it: kept over it however
// either is raised), drawn with UpdateLayeredWindow. The dock's cells
// narrower for it (the user, 2026-11-08: 应用的icon往他们略缩图窗口挤一挤，
// 不需要占那么大位置). Each window's icon read once from what the taskbar
// last sent (its program's png), kept while the window is in the dock.
pub(super) static DOCK_BADGES: AtomicIsize = AtomicIsize::new(0);
pub(super) static BADGE_ICONS: Lock<Vec<(isize, (u32, u32, Vec<u8>))>> = Lock::new("taskbar::win::thumbs::BADGE_ICONS", Vec::new());

pub fn dock_badges(items: Vec<(isize, (i32, i32, i32, i32))>) {
    let host = DOCK_BADGES.load(Ordering::SeqCst) as Hwnd;
    // One lock at a time, and none over the window's own calls: the window
    // is the main thread's (make_dock_host), and the taskbar's thread hides
    // it here as it ends, the main thread maybe waiting on BADGE_ICONS.
    let missing: Vec<isize> = {
        let mut icons = BADGE_ICONS.lock().unwrap();
        icons.retain(|(h, _)| items.iter().any(|i| i.0 == *h));
        items.iter().filter(|i| i.1 .2 > 0 && i.1 .3 > 0).map(|i| i.0).filter(|h| !icons.iter().any(|(k, _)| k == h)).collect()
    };
    if host.is_null() {
        return;
    }
    let items: Vec<_> = items.into_iter().filter(|i| i.1 .2 > 0 && i.1 .3 > 0).collect();
    if items.is_empty() {
        // SAFETY: our own window.
        unsafe { ShowWindow(host, SW_HIDE) };
        return;
    }
    // The icons not read yet: from the programs as last sent (their png).
    if !missing.is_empty() {
        let read: Vec<(isize, (u32, u32, Vec<u8>))> = {
            let sent = SENT_WINDOWS.lock().unwrap();
            missing
                .into_iter()
                .filter_map(|h| {
                    let png = sent.as_array().into_iter().flatten().find(|a| a["windows"].as_array().is_some_and(|ws| ws.iter().any(|w| w["id"].as_i64() == Some(h as i64)))).and_then(|a| a["png"].as_str());
                    png.and_then(rgba_of).map(|px| (h, px))
                })
                .collect()
        };
        let mut icons = BADGE_ICONS.lock().unwrap();
        for (h, px) in read {
            if !icons.iter().any(|(k, _)| *k == h) {
                icons.push((h, px));
            }
        }
    }
    let left = items.iter().map(|i| i.1 .0).min().unwrap_or(0);
    let top = items.iter().map(|i| i.1 .1).min().unwrap_or(0);
    let w = items.iter().map(|i| i.1 .0 + i.1 .2).max().unwrap_or(0) - left;
    let h = items.iter().map(|i| i.1 .1 + i.1 .3).max().unwrap_or(0) - top;
    // Premultiplied BGRA, top-down, as UpdateLayeredWindow takes it.
    let mut canvas = vec![0u8; (w * h * 4) as usize];
    {
        let icons = BADGE_ICONS.lock().unwrap();
        for (hwnd, (x, y, iw, ih)) in &items {
            let Some((_, (sw, sh, src))) = icons.iter().find(|(k, _)| k == hwnd) else { continue };
            draw_scaled(&mut canvas, w, (x - left, y - top, *iw, *ih), (*sw, *sh, src));
        }
    }
    show_layered(host, (left, top, w, h), &canvas);
}

// A png (a data URL, as the taskbar sends its icons) as straight RGBA.
pub(super) fn rgba_of(url: &str) -> Option<(u32, u32, Vec<u8>)> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(url.strip_prefix("data:image/png;base64,")?).ok()?;
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width, info.height);
    let px = (w * h) as usize;
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf[..px * 4].to_vec(),
        png::ColorType::Rgb => buf[..px * 3].chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        _ => return None,
    };
    Some((w, h, rgba))
}

// An icon (straight RGBA) into its rect on the canvas (premultiplied
// BGRA, `cw` wide), scaled to it, each pixel the mean of what it covers.
pub(super) fn draw_scaled(canvas: &mut [u8], cw: i32, (x, y, w, h): (i32, i32, i32, i32), (sw, sh, src): (u32, u32, &Vec<u8>)) {
    let ch = canvas.len() as i32 / 4 / cw.max(1);
    for dy in 0..h {
        for dx in 0..w {
            let (cx, cy) = (x + dx, y + dy);
            if cx < 0 || cy < 0 || cx >= cw || cy >= ch {
                continue;
            }
            // The source pixels this one covers.
            let (x0, x1) = ((dx as u32 * sw) / w as u32, (((dx + 1) as u32 * sw) / w as u32).max(dx as u32 * sw / w as u32 + 1).min(sw));
            let (y0, y1) = ((dy as u32 * sh) / h as u32, (((dy + 1) as u32 * sh) / h as u32).max(dy as u32 * sh / h as u32 + 1).min(sh));
            let (mut r, mut g, mut b, mut a, mut n) = (0u32, 0u32, 0u32, 0u32, 0u32);
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let i = ((sy * sw + sx) * 4) as usize;
                    let al = src[i + 3] as u32;
                    r += src[i] as u32 * al;
                    g += src[i + 1] as u32 * al;
                    b += src[i + 2] as u32 * al;
                    a += al;
                    n += 1;
                }
            }
            if n == 0 {
                continue;
            }
            let o = ((cy * cw + cx) * 4) as usize;
            canvas[o] = (b / 255 / n) as u8;
            canvas[o + 1] = (g / 255 / n) as u8;
            canvas[o + 2] = (r / 255 / n) as u8;
            canvas[o + 3] = (a / n) as u8;
        }
    }
}

// A layered window of ours shown at (x, y, w, h) with this picture
// (premultiplied BGRA, top-down, w x h).
pub(super) fn show_layered(host: Hwnd, (x, y, w, h): (i32, i32, i32, i32), canvas: &[u8]) {
    #[repr(C)]
    struct Blend {
        op: u8,
        flags: u8,
        alpha: u8,
        format: u8,
    }
    #[link(name = "user32")]
    extern "system" {
        fn GetDC(hwnd: Hwnd) -> isize;
        fn ReleaseDC(hwnd: Hwnd, dc: isize) -> i32;
        fn UpdateLayeredWindow(hwnd: Hwnd, dst: isize, at: *const Point, size: *const Point, src: isize, from: *const Point, key: u32, blend: *const Blend, flags: u32) -> i32;
    }
    const ULW_ALPHA: u32 = 0x2;
    const SWP_NOSIZE: u32 = 0x1;
    const SWP_NOMOVE: u32 = 0x2;
    // SAFETY: the screen's DC and GDI objects of our own, each let go of
    // here; the DIB's bits (w x h, 32 bits, top-down) written within them;
    // our own window drawn and shown.
    unsafe {
        let screen = GetDC(std::ptr::null_mut());
        if screen == 0 {
            return;
        }
        let mem = CreateCompatibleDC(screen);
        let info = BitmapInfoHeader { size: 40, width: w, height: -h, planes: 1, bit_count: 32, compression: 0, size_image: 0, x_ppm: 0, y_ppm: 0, clr_used: 0, clr_important: 0 };
        let mut bits: *mut u8 = std::ptr::null_mut();
        let dib = CreateDIBSection(screen, &info, 0, &mut bits, 0, 0);
        if mem != 0 && dib != 0 && !bits.is_null() {
            std::ptr::copy_nonoverlapping(canvas.as_ptr(), bits, canvas.len().min((w * h * 4) as usize));
            let old = SelectObject(mem, dib);
            let blend = Blend { op: 0, flags: 0, alpha: 255, format: 1 };
            UpdateLayeredWindow(host, screen, &Point { x, y }, &Point { x: w, y: h }, mem, &Point { x: 0, y: 0 }, 0, &blend, ULW_ALPHA);
            SelectObject(mem, old);
            SetWindowPos(host, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW);
        }
        if dib != 0 {
            DeleteObject(dib);
        }
        if mem != 0 {
            DeleteDC(mem);
        }
        ReleaseDC(std::ptr::null_mut(), screen);
    }
}

// The window of the dock's minimized windows' pictures: owned by the
// dock's window, and so made on its thread (the main one, as all her
// Tauri windows; owned from another thread, their input would be tied
// together), that Windows keeps it over the dock however either is
// raised, at once. Raised apart (hers as a batch, it after them), the
// dock came over it again and again as the desktop was shown and Start
// opened and closed, and its pictures showed through the glass's grey
// tint a second each time (the user, 2026-11-04). Of a class of its own:
// its ground a colour keyed out, so that between its pictures the dock
// shows through, not a black block (2026-11-02). The pictures are DWM's,
// over it, whole.
pub(super) static DOCK_CLASS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn make_dock_host(owner: isize) {
    let class = wide("WakuwakuDockThumbs");
    // SAFETY: our own class and window, on this thread (the owner's).
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        if !DOCK_CLASS.swap(true, Ordering::SeqCst) {
            let wc = WndClassExW {
                size: std::mem::size_of::<WndClassExW>() as u32,
                style: 0,
                wndproc: thumbs_proc,
                cls_extra: 0,
                wnd_extra: 0,
                instance,
                icon: 0,
                cursor: 0,
                background: CreateSolidBrush(DOCK_KEY),
                menu_name: std::ptr::null(),
                class_name: class.as_ptr(),
                icon_sm: 0,
            };
            RegisterClassExW(&wc);
        }
        let host = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT,
            class.as_ptr(),
            std::ptr::null(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            owner as Hwnd,
            0,
            instance,
            std::ptr::null_mut(),
        );
        if !host.is_null() {
            SetLayeredWindowAttributes(host, DOCK_KEY, 255, LWA_COLORKEY);
            DOCK_HOST.store(host as isize, Ordering::SeqCst);
            // The icons over the pictures (dock_badges): owned by their
            // window, so Windows keeps them over it; drawn by
            // UpdateLayeredWindow, so no colour of their own to key out.
            let badges = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT,
                class.as_ptr(),
                std::ptr::null(),
                WS_POPUP,
                0,
                0,
                0,
                0,
                host,
                0,
                instance,
                std::ptr::null_mut(),
            );
            DOCK_BADGES.store(badges as isize, Ordering::SeqCst);
        }
    }
}

// The dock gone: its pictures with it (the window goes with its owner).
pub fn drop_dock_host() {
    place_thumbs(std::ptr::null_mut(), &mut DOCK_THUMBS.lock().unwrap(), Vec::new(), None);
    DOCK_HOST.store(0, Ordering::SeqCst);
    // Its icons' window goes with it (owned by it).
    DOCK_BADGES.store(0, Ordering::SeqCst);
    BADGE_ICONS.lock().unwrap().clear();
}

// The windows of the live pictures (the lists', the dock's) back over
// hers, as hers were raised (raise_own, tasks::raise_top): not hers to
// Tauri, so left under them, the dock's minimized windows' pictures
// seen through its glass's grey tint once the desktop had been pressed
// (the user, 2026-11-04). Not waiting on their thread.
pub fn thumbs_on_top() {
    const SWP_NOSIZE: u32 = 0x1;
    const SWP_NOMOVE: u32 = 0x2;
    const SWP_ASYNCWINDOWPOS: u32 = 0x4000;
    for host in [THUMB_HOST.load(Ordering::SeqCst), DOCK_HOST.load(Ordering::SeqCst)] {
        // SAFETY: our own windows; a place in the stack alone.
        unsafe {
            if host != 0 && IsWindowVisible(host as Hwnd) != 0 {
                SetWindowPos(host as Hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS);
            }
        }
    }
}

// The dock's pictures over the dock again, should anything have put the
// dock over them: looked at with the pointer (bars.rs poll), as raising
// them after each of her raises left ways round (Win+D, then Start
// opened and closed: grey a second more, the user, 2026-11-04). Whether
// the dock lies above its pictures' window, walking up from that
// window through the windows over it; only then raised, not waiting.
pub fn dock_thumbs_over(dock: isize) {
    #[link(name = "user32")]
    extern "system" {
        fn GetWindow(hwnd: Hwnd, cmd: u32) -> Hwnd;
    }
    const GW_HWNDPREV: u32 = 3;
    const SWP_NOSIZE: u32 = 0x1;
    const SWP_NOMOVE: u32 = 0x2;
    const SWP_ASYNCWINDOWPOS: u32 = 0x4000;
    let host = DOCK_HOST.load(Ordering::SeqCst);
    if host == 0 || dock == 0 {
        return;
    }
    // SAFETY: plain queries along the z-order from our own window; a move of it.
    unsafe {
        if IsWindowVisible(host as Hwnd) == 0 {
            return;
        }
        let mut at = host as Hwnd;
        for _ in 0..512 {
            at = GetWindow(at, GW_HWNDPREV);
            if at.is_null() {
                return;
            }
            if at as isize == dock {
                SetWindowPos(host as Hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS);
                // Said at most every few seconds, should something keep at it.
                static SAID: Lock<Option<Instant>> = Lock::new("taskbar::win::thumbs::SAID", None);
                let mut said = SAID.lock().unwrap();
                if said.is_none_or(|at| at.elapsed() >= Duration::from_secs(5)) {
                    *said = Some(Instant::now());
                    log("the dock's pictures lay under it: raised over it again");
                }
                return;
            }
        }
    }
}

// Hers raised as a batch (raise_own), each waiting its turn on the main
// thread: the pictures' windows after them, from there (in its queue
// after those raises).
pub(super) fn raise_own_with_thumbs(sh: &Shared) {
    let own = sh.own_windows();
    tasks::raise_own(&own);
    let _ = sh.app.run_on_main_thread(thumbs_on_top);
}

// Windows' live pictures of windows (DWM thumbnails) in a host window of
// ours, laid over the rooms (screen, physical; or over `area`, kept so)
// and above the rest. Those asked for again kept, only laid anew; the
// rest unregistered, the new ones registered. All were unregistered and
// registered again at each call: the dock's flashed its grey rooms as a
// window went into it or came out (the user, 2026-11-08: 每次最小化到dock
// 和点开后，dock的略缩图都会闪灰黑一下然后又出现略缩图).
pub(super) fn place_thumbs(host: Hwnd, shown: &mut Vec<(isize, isize)>, items: Vec<(isize, (i32, i32, i32, i32))>, area: Option<(i32, i32, i32, i32)>) {
    shown.retain(|&(hwnd, thumb)| {
        let keep = !host.is_null() && items.iter().any(|i| i.0 == hwnd);
        if !keep {
            // SAFETY: a thumbnail of ours.
            unsafe { DwmUnregisterThumbnail(thumb) };
        }
        keep
    });
    if host.is_null() {
        return;
    }
    if items.is_empty() {
        // SAFETY: our own window.
        unsafe { ShowWindow(host, SW_HIDE) };
        return;
    }
    // Over all the rooms at once (or the area given), above the home's window.
    let (left, top, right, bottom) = match area {
        Some((x, y, w, h)) => (x, y, x + w, y + h),
        None => (
            items.iter().map(|i| i.1 .0).min().unwrap_or(0),
            items.iter().map(|i| i.1 .1).min().unwrap_or(0),
            items.iter().map(|i| i.1 .0 + i.1 .2).max().unwrap_or(0),
            items.iter().map(|i| i.1 .1 + i.1 .3).max().unwrap_or(0),
        ),
    };
    // SAFETY: our own window.
    unsafe { SetWindowPos(host, HWND_TOPMOST, left, top, right - left, bottom - top, SWP_NOACTIVATE | SWP_SHOWWINDOW) };
    let mut next = Vec::with_capacity(items.len());
    for (hwnd, (x, y, w, h)) in items {
        let (x, y) = (x - left, y - top);
        let mut thumb = shown.iter().find(|s| s.0 == hwnd).map_or(0, |s| s.1);
        // SAFETY: our own window and another's; the handle is ours to unregister.
        if thumb == 0 && (unsafe { DwmRegisterThumbnail(host, hwnd as Hwnd, &mut thumb) } != 0 || thumb == 0) {
            continue;
        }
        let mut size = Size::default();
        // SAFETY: our own struct.
        unsafe { DwmQueryThumbnailSourceSize(thumb, &mut size) };
        // Fitted into the room, its shape kept, in the middle.
        let (sw, sh) = (size.cx.max(1) as f64, size.cy.max(1) as f64);
        let k = (w as f64 / sw).min(h as f64 / sh);
        let (dw, dh) = ((sw * k).round() as i32, (sh * k).round() as i32);
        let (dx, dy) = (x + (w - dw) / 2, y + (h - dh) / 2);
        let props = ThumbnailProperties {
            flags: DWM_TNP_RECTDESTINATION | DWM_TNP_OPACITY | DWM_TNP_VISIBLE,
            dest: Rect { left: dx, top: dy, right: dx + dw, bottom: dy + dh },
            source: Rect::default(),
            opacity: 255,
            visible: 1,
            client_only: 0,
        };
        // SAFETY: our own thumbnail and struct.
        unsafe { DwmUpdateThumbnailProperties(thumb, &props) };
        next.push((hwnd, thumb));
    }
    *shown = next;
}

// The live pictures' window: nothing of its own but its colour.
pub(super) extern "system" fn thumbs_proc(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize {
    // SAFETY: the default for everything.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}
