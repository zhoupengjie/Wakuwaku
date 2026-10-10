// The desktop's picture, set from the settings (settings.rs
// settings_wallpaper), as Settings → Personalization → Background sets it:
// one picture, or a folder's pictures in turn, and how the picture is laid
// on the display (fill, fit, stretch, tile, centre, span).
//
// The turns are Windows' own slideshow (IDesktopWallpaper's), so they go
// on with her not running, and the taskbar's Mica follows them as it
// follows any picture (wallpaper.rs reads it). One picture is set as
// Windows' classic call sets it (SPI_SETDESKWALLPAPER), which ends a
// slideshow; the folder, the laying and the turns through IDesktopWallpaper.
// The picture or the folder is picked in Windows' own file dialog
// (IFileOpenDialog).

// How a picture is laid, by DESKTOP_WALLPAPER_POSITION; and the slideshow's
// turns Windows offers, in ms (a minute to a day).
pub const POSITIONS: [&str; 6] = ["center", "tile", "stretch", "fit", "fill", "span"];
pub const EVERY: [u32; 6] = [60_000, 600_000, 1_800_000, 3_600_000, 21_600_000, 86_400_000];

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::sync::Mutex;

    use serde_json::{json, Value};

    use super::{EVERY, POSITIONS};

    #[repr(C)]
    struct Guid(u32, u16, u16, [u8; 8]);

    const CLSID_DESKTOP_WALLPAPER: Guid = Guid(0xC2CF_3110, 0x460E, 0x4FC1, [0xB9, 0xD0, 0x8A, 0x1C, 0x0C, 0x9C, 0xC4, 0xBD]);
    const IID_IDESKTOP_WALLPAPER: Guid = Guid(0xB92B_56A9, 0x8B55, 0x4E14, [0x9A, 0x89, 0x01, 0x99, 0xBB, 0xB6, 0xF9, 0x3B]);
    const CLSID_FILE_OPEN_DIALOG: Guid = Guid(0xDC1C_5A9C, 0xE88A, 0x4DDE, [0xA5, 0xA1, 0x60, 0xF8, 0x2A, 0x20, 0xAE, 0xF7]);
    const IID_FILE_OPEN_DIALOG: Guid = Guid(0xD57C_7288, 0xD4AD, 0x4768, [0xBE, 0x02, 0x9D, 0x96, 0x95, 0x32, 0xD9, 0x60]);
    const IID_SHELL_ITEM: Guid = Guid(0x4382_6D1E, 0xE718, 0x42EE, [0xBC, 0x55, 0xA1, 0xE2, 0x61, 0xC3, 0x7B, 0xFE]);
    const IID_SHELL_ITEM_ARRAY: Guid = Guid(0xB63E_A76D, 0x1F85, 0x456F, [0xA1, 0x9C, 0x48, 0x15, 0x9E, 0xFA, 0x85, 0x8B]);
    const COINIT_APARTMENTTHREADED: u32 = 0x2;
    const CLSCTX_ALL: u32 = 0x17;
    const SIGDN_FILESYSPATH: u32 = 0x8005_8000;
    const SPI_SETDESKWALLPAPER: u32 = 0x0014;
    const SPIF_UPDATEINIFILE: u32 = 0x1;
    const SPIF_SENDCHANGE: u32 = 0x2;
    const DSS_SLIDESHOW: i32 = 0x2;
    const DSO_SHUFFLEIMAGES: u32 = 0x1;
    const DSD_BACKWARD: i32 = 1;
    const FOS_PICKFOLDERS: u32 = 0x20;
    const FOS_FORCEFILESYSTEM: u32 = 0x40;
    const FOS_FILEMUSTEXIST: u32 = 0x1000;

    // A COM object: its table of methods first.
    #[repr(C)]
    struct Obj {
        vtbl: *const usize,
    }

    // The methods asked of each, by their place in its table.
    // IDesktopWallpaper:
    const SET_WALLPAPER: usize = 3;
    const GET_WALLPAPER: usize = 4;
    const GET_MONITOR_DEVICE_PATH_AT: usize = 5;
    const GET_MONITOR_DEVICE_PATH_COUNT: usize = 6;
    const SET_POSITION: usize = 10;
    const GET_POSITION: usize = 11;
    const SET_SLIDESHOW: usize = 12;
    const GET_SLIDESHOW: usize = 13;
    const SET_SLIDESHOW_OPTIONS: usize = 14;
    const GET_SLIDESHOW_OPTIONS: usize = 15;
    const ADVANCE_SLIDESHOW: usize = 16;
    const GET_STATUS: usize = 17;
    // IShellItem, IShellItemArray:
    const ITEM_GET_DISPLAY_NAME: usize = 5;
    const ARRAY_GET_ITEM_AT: usize = 8;
    // IFileOpenDialog:
    const DIALOG_SHOW: usize = 3;
    const DIALOG_SET_FILE_TYPES: usize = 4;
    const DIALOG_SET_OPTIONS: usize = 9;
    const DIALOG_GET_OPTIONS: usize = 10;
    const DIALOG_SET_TITLE: usize = 17;
    const DIALOG_GET_RESULT: usize = 20;

    #[repr(C)]
    struct FilterSpec {
        name: *const u16,
        spec: *const u16,
    }

    #[link(name = "ole32")]
    extern "system" {
        fn CoInitializeEx(reserved: *mut c_void, flags: u32) -> i32;
        fn CoUninitialize();
        fn CoCreateInstance(clsid: *const Guid, outer: *mut c_void, ctx: u32, iid: *const Guid, out: *mut *mut c_void) -> i32;
        fn CoTaskMemFree(p: *mut c_void);
    }

    #[link(name = "shell32")]
    extern "system" {
        fn SHCreateItemFromParsingName(path: *const u16, bind: *mut c_void, iid: *const Guid, out: *mut *mut c_void) -> i32;
        fn SHCreateShellItemArrayFromShellItem(item: *mut Obj, iid: *const Guid, out: *mut *mut c_void) -> i32;
    }

    #[link(name = "user32")]
    extern "system" {
        fn SystemParametersInfoW(action: u32, param: u32, pv: *mut c_void, flags: u32) -> i32;
    }

    // An object's method by its place, as a function of the shape asked.
    // SAFETY (callers): the object is alive and has that method there.
    unsafe fn method<F: Copy>(obj: *mut Obj, at: usize) -> F {
        std::mem::transmute_copy(&*(*obj).vtbl.add(at))
    }

    unsafe fn release(obj: *mut Obj) {
        if !obj.is_null() {
            let f: extern "system" fn(*mut Obj) -> u32 = method(obj, 2);
            f(obj);
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    // A string Windows handed over (CoTaskMemAlloc'd), read and freed.
    unsafe fn take(p: *mut u16) -> String {
        if p.is_null() {
            return String::new();
        }
        let n = (0..).take_while(|&i| *p.add(i) != 0).count();
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, n));
        CoTaskMemFree(p as *mut c_void);
        s
    }

    // COM on this thread for the call, left as found.
    fn with_com<T>(f: impl FnOnce() -> T) -> T {
        // SAFETY: balanced below when it took.
        let init = unsafe { CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED) };
        let out = f();
        if init >= 0 {
            // SAFETY: the initialization above undone.
            unsafe { CoUninitialize() };
        }
        out
    }

    // IDesktopWallpaper, released by the caller.
    unsafe fn wallpaper() -> Option<*mut Obj> {
        let mut w: *mut Obj = std::ptr::null_mut();
        let made = CoCreateInstance(&CLSID_DESKTOP_WALLPAPER, std::ptr::null_mut(), CLSCTX_ALL, &IID_IDESKTOP_WALLPAPER, &mut w as *mut *mut Obj as *mut *mut c_void);
        (made >= 0 && !w.is_null()).then_some(w)
    }

    // A shell item's file or folder path.
    unsafe fn path_of(item: *mut Obj) -> Option<String> {
        let mut name: *mut u16 = std::ptr::null_mut();
        let f: extern "system" fn(*mut Obj, u32, *mut *mut u16) -> i32 = method(item, ITEM_GET_DISPLAY_NAME);
        (f(item, SIGDN_FILESYSPATH, &mut name) >= 0).then(|| take(name)).filter(|p| !p.is_empty())
    }

    // The picture shown: every display's one. In a slideshow Windows tells
    // none for all at once (S_FALSE, an empty path), so the first display's
    // that tells one: read for all, a slideshow's pictures went unseen and
    // "previous" had none to go back to (2026-10-10).
    unsafe fn shown(w: *mut Obj) -> String {
        let get: extern "system" fn(*mut Obj, *const u16, *mut *mut u16) -> i32 = method(w, GET_WALLPAPER);
        let mut file: *mut u16 = std::ptr::null_mut();
        get(w, std::ptr::null(), &mut file);
        let file = take(file);
        if !file.is_empty() {
            return file;
        }
        let mut count = 0u32;
        let f: extern "system" fn(*mut Obj, *mut u32) -> i32 = method(w, GET_MONITOR_DEVICE_PATH_COUNT);
        f(w, &mut count);
        let at: extern "system" fn(*mut Obj, u32, *mut *mut u16) -> i32 = method(w, GET_MONITOR_DEVICE_PATH_AT);
        for i in 0..count {
            let mut id: *mut u16 = std::ptr::null_mut();
            if at(w, i, &mut id) < 0 || id.is_null() {
                continue;
            }
            let id = wide(&take(id));
            let mut file: *mut u16 = std::ptr::null_mut();
            get(w, id.as_ptr(), &mut file);
            let file = take(file);
            if !file.is_empty() {
                return file;
            }
        }
        String::new()
    }

    // What the calls did, for the log (settings.rs takes them after each).
    static NOTES: Mutex<Vec<String>> = Mutex::new(Vec::new());

    fn note(line: String) {
        NOTES.lock().unwrap().push(line);
    }

    pub fn notes() -> Vec<String> {
        std::mem::take(&mut *NOTES.lock().unwrap())
    }

    // A file's name, for the log.
    fn name(path: &str) -> &str {
        path.rsplit(['\\', '/']).next().unwrap_or(path)
    }

    // What the desktop shows: a slideshow or one picture, its file (the
    // slideshow's of the moment), the slideshow's folder, how it is laid,
    // the turns.
    struct State {
        slideshow: bool,
        file: String,
        folder: Option<String>,
        position: &'static str,
        every: u32,
        shuffle: bool,
    }

    // The desktop as it was before each change made here, the latest last:
    // undo walks back through them. And the slideshow's pictures as seen,
    // for stepping back should Windows not.
    static BEFORE: Mutex<Vec<State>> = Mutex::new(Vec::new());
    static SEEN: Mutex<Vec<String>> = Mutex::new(Vec::new());

    fn seen(file: &str) {
        let mut seen = SEEN.lock().unwrap();
        if !file.is_empty() && seen.last().map(String::as_str) != Some(file) {
            seen.push(file.to_string());
            if seen.len() > 50 {
                seen.remove(0);
            }
        }
    }

    // For the settings: the desktop now, and whether a change of ours can be
    // undone.
    pub fn now() -> Value {
        let Some(s) = state() else { return Value::Null };
        if s.slideshow {
            seen(&s.file);
        }
        json!({
            "slideshow": s.slideshow,
            "file": s.file,
            "folder": s.folder,
            "position": s.position,
            "every": s.every,
            "shuffle": s.shuffle,
            "canUndo": !BEFORE.lock().unwrap().is_empty(),
        })
    }

    // The desktop as it is, kept before a change (picked, laid, stepped).
    pub fn remember() {
        if let Some(s) = state() {
            if s.slideshow {
                seen(&s.file);
            }
            let mut before = BEFORE.lock().unwrap();
            before.push(s);
            if before.len() > 20 {
                before.remove(0);
            }
        }
    }

    // Back to how it was before the last change made here: its picture, or
    // its folder's slideshow on the picture it showed then, laid as it was.
    // The slideshow set again with no step (a step showed another of its
    // pictures, and each undo seemed a new one at random, 2026-10-10), kept
    // as it is should it be that folder's already.
    pub fn undo() -> bool {
        let Some(s) = BEFORE.lock().unwrap().pop() else { return false };
        note(format!("undo: back to {} ({})", name(&s.file), if s.slideshow { "the slideshow" } else { "one picture" }));
        let back = match (s.slideshow, s.folder.as_deref()) {
            (true, Some(folder)) => {
                let turning = state().is_some_and(|n| n.slideshow && n.folder.as_deref().is_some_and(|f| f.eq_ignore_ascii_case(folder)));
                let on = (turning || slideshow(folder, false)) && set_turns(Some(s.every), Some(s.shuffle));
                on && (s.file.is_empty() || show(&s.file, &s, "undo"))
            }
            _ => !s.file.is_empty() && set_picture(&s.file),
        };
        back && set_position(s.position)
    }

    // The picture shown once it is another than this one, waited for up to
    // so long (Windows changes it a moment after it is asked, with a fade).
    fn changes_from(was: &str, most_ms: u64) -> Option<String> {
        let asked = std::time::Instant::now();
        loop {
            if let Some(file) = state().map(|s| s.file).filter(|f| !f.is_empty() && !f.eq_ignore_ascii_case(was)) {
                return Some(file);
            }
            if asked.elapsed() >= std::time::Duration::from_millis(most_ms) {
                return None;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    // Whether this picture is shown, waited for up to so long.
    fn shows(file: &str, most_ms: u64) -> bool {
        let asked = std::time::Instant::now();
        loop {
            if state().is_some_and(|s| s.file.eq_ignore_ascii_case(file)) {
                return true;
            }
            if asked.elapsed() >= std::time::Duration::from_millis(most_ms) {
                return false;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    // The slideshow's step either way: Windows' answer (an HRESULT).
    fn advance(direction: i32) -> i32 {
        // SAFETY: a COM object of our own, released.
        with_com(|| unsafe {
            let Some(w) = wallpaper() else { return E_FAIL };
            let f: extern "system" fn(*mut Obj, *const u16, i32) -> i32 = method(w, ADVANCE_SLIDESHOW);
            let hr = f(w, std::ptr::null(), direction);
            release(w);
            hr
        })
    }

    const E_FAIL: i32 = 0x8000_4005_u32 as i32;

    // The picture before this one in its folder by name (the slideshow's
    // order, unshuffled), the last before the first: for a step back with
    // none seen yet (her started since).
    fn before_in_folder(folder: &str, file: &str) -> Option<String> {
        const PICTURES: [&str; 14] = ["jpg", "jpeg", "jfif", "png", "bmp", "dib", "gif", "tif", "tiff", "heic", "webp", "avif", "jxr", "wdp"];
        let mut all: Vec<String> = std::fs::read_dir(folder)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|x| x.to_str()).is_some_and(|x| PICTURES.contains(&x.to_ascii_lowercase().as_str())))
            .filter_map(|p| p.to_str().map(str::to_string))
            .collect();
        all.sort_by_key(|p| p.to_lowercase());
        let at = all.iter().position(|p| p.eq_ignore_ascii_case(file))?;
        (all.len() > 1).then(|| all[(at + all.len() - 1) % all.len()].clone())
    }

    // The slideshow's picture before this one: Windows' own step back; should
    // it not take one (the same picture a moment after), the one seen before
    // this (the seen after it let go, so another step goes further back),
    // else the one before it in the folder; shown, the slideshow going on.
    pub fn previous() -> bool {
        let Some(now) = state() else {
            note("previous: the desktop not read".into());
            return false;
        };
        seen(&now.file);
        let hr = advance(DSD_BACKWARD);
        let stepped = if hr >= 0 { changes_from(&now.file, 1500) } else { None };
        note(format!("previous: from {}; Windows' step back {:#010x}, to {}", name(&now.file), hr as u32, stepped.as_deref().map_or("none", name)));
        if stepped.is_some() {
            return true;
        }
        let (earlier, count) = {
            let mut seen = SEEN.lock().unwrap();
            let count = seen.len();
            let at = seen.iter().rposition(|f| f.eq_ignore_ascii_case(&now.file)).filter(|&i| i > 0);
            let earlier = at.map(|i| {
                seen.truncate(i);
                seen[i - 1].clone()
            });
            (earlier, count)
        };
        let (earlier, from) = match earlier {
            Some(file) => (Some(file), "seen"),
            None => (now.folder.as_deref().and_then(|d| before_in_folder(d, &now.file)), "the folder"),
        };
        let Some(file) = earlier else {
            note(format!("previous: none before it ({count} seen, none in the folder)"));
            return false;
        };
        note(format!("previous: back to {} ({from}; {count} seen)", name(&file)));
        show(&file, &now, "previous")
    }

    // A picture shown now: Windows' call for it; should it not take (the
    // slideshow's own), the classic one, and the slideshow it ended on again
    // from there.
    fn show(file: &str, was: &State, why: &str) -> bool {
        // SAFETY: a COM object of our own, released; the path alive for the call.
        let hr = with_com(|| unsafe {
            let Some(w) = wallpaper() else { return E_FAIL };
            let put: extern "system" fn(*mut Obj, *const u16, *const u16) -> i32 = method(w, SET_WALLPAPER);
            let path = wide(file);
            let hr = put(w, std::ptr::null(), path.as_ptr());
            release(w);
            hr
        });
        let mut done = hr >= 0 && shows(file, 1500);
        note(format!("{why}: SetWallpaper {:#010x}, shown: {done}", hr as u32));
        if !done {
            let set = set_picture(file);
            done = set && shows(file, 1500);
            note(format!("{why}: SPI_SETDESKWALLPAPER {set}, shown: {done}"));
        }
        let ended = state().is_some_and(|s| !s.slideshow);
        if let (true, true, true, Some(folder)) = (done, was.slideshow, ended, was.folder.as_deref()) {
            let on = slideshow(folder, false) && set_turns(Some(was.every), Some(was.shuffle));
            note(format!("{why}: the slideshow ended; on again: {on}, the picture kept: {}", shows(file, 0)));
        }
        done
    }

    fn state() -> Option<State> {
        // SAFETY: COM objects of our own, released once read; each
        // out-parameter ours, of the size the call writes.
        with_com(|| unsafe {
            let w = wallpaper()?;
            let mut status = 0i32;
            let f: extern "system" fn(*mut Obj, *mut i32) -> i32 = method(w, GET_STATUS);
            f(w, &mut status);
            let mut position = 4i32;
            let f: extern "system" fn(*mut Obj, *mut i32) -> i32 = method(w, GET_POSITION);
            f(w, &mut position);
            let file = shown(w);
            let (mut options, mut tick) = (0u32, 0u32);
            let f: extern "system" fn(*mut Obj, *mut u32, *mut u32) -> i32 = method(w, GET_SLIDESHOW_OPTIONS);
            f(w, &mut options, &mut tick);
            let mut folder = None;
            let mut items: *mut Obj = std::ptr::null_mut();
            let f: extern "system" fn(*mut Obj, *mut *mut Obj) -> i32 = method(w, GET_SLIDESHOW);
            if f(w, &mut items) >= 0 && !items.is_null() {
                let mut item: *mut Obj = std::ptr::null_mut();
                let at: extern "system" fn(*mut Obj, u32, *mut *mut Obj) -> i32 = method(items, ARRAY_GET_ITEM_AT);
                if at(items, 0, &mut item) >= 0 && !item.is_null() {
                    folder = path_of(item);
                    release(item);
                }
                release(items);
            }
            release(w);
            Some(State {
                slideshow: status & DSS_SLIDESHOW != 0,
                file,
                folder,
                position: POSITIONS.get(position as usize).copied().unwrap_or("fill"),
                every: tick,
                shuffle: options & DSO_SHUFFLEIMAGES != 0,
            })
        })
    }

    // One picture, on every display; a slideshow ends.
    pub fn set_picture(path: &str) -> bool {
        let mut path = wide(path);
        // SAFETY: a path of our own, alive for the call.
        unsafe { SystemParametersInfoW(SPI_SETDESKWALLPAPER, 0, path.as_mut_ptr() as *mut c_void, SPIF_UPDATEINIFILE | SPIF_SENDCHANGE) != 0 }
    }

    // A folder's pictures in turn (Windows' slideshow), from now: the first
    // of them at once (seen, once shown), every half hour should no turn be
    // set yet.
    pub fn set_folder(path: &str) -> bool {
        let was = state().map(|s| s.file).unwrap_or_default();
        let set = slideshow(path, true);
        if let Some(file) = set.then(|| changes_from(&was, 1500)).flatten() {
            seen(&file);
        }
        set
    }

    // The slideshow set to a folder; its next picture at once, or (back
    // from a step back) the one shown kept.
    fn slideshow(path: &str, advance: bool) -> bool {
        // SAFETY: COM objects of our own, released; the path alive for the calls.
        with_com(|| unsafe {
            let Some(w) = wallpaper() else { return false };
            let mut item: *mut Obj = std::ptr::null_mut();
            let mut items: *mut Obj = std::ptr::null_mut();
            let found = SHCreateItemFromParsingName(wide(path).as_ptr(), std::ptr::null_mut(), &IID_SHELL_ITEM, &mut item as *mut *mut Obj as *mut *mut c_void) >= 0 && !item.is_null();
            let listed = found && SHCreateShellItemArrayFromShellItem(item, &IID_SHELL_ITEM_ARRAY, &mut items as *mut *mut Obj as *mut *mut c_void) >= 0 && !items.is_null();
            let set = listed && {
                let f: extern "system" fn(*mut Obj, *mut Obj) -> i32 = method(w, SET_SLIDESHOW);
                f(w, items) >= 0
            };
            if set {
                let (mut options, mut tick) = (0u32, 0u32);
                let f: extern "system" fn(*mut Obj, *mut u32, *mut u32) -> i32 = method(w, GET_SLIDESHOW_OPTIONS);
                f(w, &mut options, &mut tick);
                if tick == 0 {
                    let f: extern "system" fn(*mut Obj, u32, u32) -> i32 = method(w, SET_SLIDESHOW_OPTIONS);
                    f(w, options, EVERY[2]);
                }
                if advance {
                    let f: extern "system" fn(*mut Obj, *const u16, i32) -> i32 = method(w, ADVANCE_SLIDESHOW);
                    f(w, std::ptr::null(), 0);
                }
            }
            release(items);
            release(item);
            release(w);
            set
        })
    }

    // How the picture is laid (one of POSITIONS).
    pub fn set_position(name: &str) -> bool {
        let Some(at) = POSITIONS.iter().position(|p| *p == name) else { return false };
        // SAFETY: a COM object of our own, released.
        with_com(|| unsafe {
            let Some(w) = wallpaper() else { return false };
            let f: extern "system" fn(*mut Obj, i32) -> i32 = method(w, SET_POSITION);
            let set = f(w, at as i32) >= 0;
            let mut path: *mut u16 = std::ptr::null_mut();
            // Laid anew at once, as Settings shows it: the picture set again.
            let get: extern "system" fn(*mut Obj, *const u16, *mut *mut u16) -> i32 = method(w, GET_WALLPAPER);
            get(w, std::ptr::null(), &mut path);
            let path = take(path);
            if set && !path.is_empty() {
                let put: extern "system" fn(*mut Obj, *const u16, *const u16) -> i32 = method(w, SET_WALLPAPER);
                put(w, std::ptr::null(), wide(&path).as_ptr());
            }
            release(w);
            set
        })
    }

    // The slideshow's turn (one of EVERY) and whether in no order.
    pub fn set_turns(every: Option<u32>, shuffle: Option<bool>) -> bool {
        // SAFETY: a COM object of our own, released.
        with_com(|| unsafe {
            let Some(w) = wallpaper() else { return false };
            let (mut options, mut tick) = (0u32, 0u32);
            let f: extern "system" fn(*mut Obj, *mut u32, *mut u32) -> i32 = method(w, GET_SLIDESHOW_OPTIONS);
            f(w, &mut options, &mut tick);
            if let Some(every) = every.filter(|e| EVERY.contains(e)) {
                tick = every;
            }
            if let Some(shuffle) = shuffle {
                options = if shuffle { options | DSO_SHUFFLEIMAGES } else { options & !DSO_SHUFFLEIMAGES };
            }
            let f: extern "system" fn(*mut Obj, u32, u32) -> i32 = method(w, SET_SLIDESHOW_OPTIONS);
            let set = f(w, options, tick) >= 0;
            release(w);
            set
        })
    }

    // The slideshow's next picture now.
    // Waited for until shown, and seen: a step back has it to go back to,
    // and the settings name it at once.
    pub fn next() -> bool {
        let was = state().map(|s| s.file).unwrap_or_default();
        seen(&was);
        let hr = advance(0);
        let now = if hr >= 0 { changes_from(&was, 2000) } else { None };
        note(format!("next: from {}; Windows' step {:#010x}, to {}", name(&was), hr as u32, now.as_deref().map_or("none yet", name)));
        if let Some(file) = &now {
            seen(file);
        }
        hr >= 0
    }

    // A picture, or a folder, picked in Windows' own file dialog over this
    // window; None when it was closed instead.
    pub fn pick(folder: bool, title: &str, pictures: &str, owner: isize) -> Option<String> {
        // SAFETY: COM objects of our own, released; the strings alive for the
        // calls; the dialog's own loop runs while it is open.
        with_com(|| unsafe {
            let mut dialog: *mut Obj = std::ptr::null_mut();
            if CoCreateInstance(&CLSID_FILE_OPEN_DIALOG, std::ptr::null_mut(), CLSCTX_ALL, &IID_FILE_OPEN_DIALOG, &mut dialog as *mut *mut Obj as *mut *mut c_void) < 0 || dialog.is_null() {
                return None;
            }
            let mut options = 0u32;
            let get: extern "system" fn(*mut Obj, *mut u32) -> i32 = method(dialog, DIALOG_GET_OPTIONS);
            get(dialog, &mut options);
            let set: extern "system" fn(*mut Obj, u32) -> i32 = method(dialog, DIALOG_SET_OPTIONS);
            set(dialog, options | FOS_FORCEFILESYSTEM | if folder { FOS_PICKFOLDERS } else { FOS_FILEMUSTEXIST });
            let (name, spec) = (wide(pictures), wide("*.jpg;*.jpeg;*.jfif;*.png;*.bmp;*.dib;*.gif;*.tif;*.tiff;*.heic;*.webp;*.avif;*.jxr;*.wdp"));
            if !folder {
                let types = [FilterSpec { name: name.as_ptr(), spec: spec.as_ptr() }];
                let f: extern "system" fn(*mut Obj, u32, *const FilterSpec) -> i32 = method(dialog, DIALOG_SET_FILE_TYPES);
                f(dialog, 1, types.as_ptr());
            }
            let title = wide(title);
            let f: extern "system" fn(*mut Obj, *const u16) -> i32 = method(dialog, DIALOG_SET_TITLE);
            f(dialog, title.as_ptr());
            let show: extern "system" fn(*mut Obj, isize) -> i32 = method(dialog, DIALOG_SHOW);
            let mut picked = None;
            if show(dialog, owner) >= 0 {
                let mut item: *mut Obj = std::ptr::null_mut();
                let f: extern "system" fn(*mut Obj, *mut *mut Obj) -> i32 = method(dialog, DIALOG_GET_RESULT);
                if f(dialog, &mut item) >= 0 && !item.is_null() {
                    picked = path_of(item);
                    release(item);
                }
            }
            release(dialog);
            picked
        })
    }
}

#[cfg(not(windows))]
mod imp {
    use serde_json::Value;

    pub fn now() -> Value {
        Value::Null
    }
    pub fn set_picture(_path: &str) -> bool {
        false
    }
    pub fn set_folder(_path: &str) -> bool {
        false
    }
    pub fn set_position(_name: &str) -> bool {
        false
    }
    pub fn set_turns(_every: Option<u32>, _shuffle: Option<bool>) -> bool {
        false
    }
    pub fn remember() {}
    pub fn notes() -> Vec<String> {
        Vec::new()
    }
    pub fn undo() -> bool {
        false
    }
    pub fn previous() -> bool {
        false
    }
    pub fn next() -> bool {
        false
    }
    pub fn pick(_folder: bool, _title: &str, _pictures: &str, _owner: isize) -> Option<String> {
        None
    }
}

pub use imp::{next, notes, now, pick, previous, remember, set_folder, set_picture, set_position, set_turns, undo};

#[cfg(test)]
mod tests {
    // Reads this machine's desktop, changing nothing: run by hand
    // (cargo test paper -- --ignored --nocapture).
    #[test]
    #[ignore]
    fn reads_the_desktop() {
        let now = super::now();
        println!("desktop: {now}");
        assert!(now.get("position").is_some());
    }
}
