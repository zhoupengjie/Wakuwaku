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

    // What the desktop shows now, for the settings: a slideshow or one
    // picture, its file (the slideshow's of the moment), the slideshow's
    // folder, how it is laid, the turns.
    pub fn now() -> Value {
        // SAFETY: COM objects of our own, released once read; each
        // out-parameter ours, of the size the call writes.
        with_com(|| unsafe {
            let Some(w) = wallpaper() else { return Value::Null };
            let mut status = 0i32;
            let f: extern "system" fn(*mut Obj, *mut i32) -> i32 = method(w, GET_STATUS);
            f(w, &mut status);
            let mut position = 4i32;
            let f: extern "system" fn(*mut Obj, *mut i32) -> i32 = method(w, GET_POSITION);
            f(w, &mut position);
            let mut file: *mut u16 = std::ptr::null_mut();
            let f: extern "system" fn(*mut Obj, *const u16, *mut *mut u16) -> i32 = method(w, GET_WALLPAPER);
            f(w, std::ptr::null(), &mut file);
            let file = take(file);
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
            json!({
                "slideshow": status & DSS_SLIDESHOW != 0,
                "file": file,
                "folder": folder,
                "position": POSITIONS.get(position as usize).copied().unwrap_or("fill"),
                "every": tick,
                "shuffle": options & DSO_SHUFFLEIMAGES != 0,
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
    // of them at once, every half hour should no turn be set yet.
    pub fn set_folder(path: &str) -> bool {
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
                let f: extern "system" fn(*mut Obj, *const u16, i32) -> i32 = method(w, ADVANCE_SLIDESHOW);
                f(w, std::ptr::null(), 0);
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
    pub fn next() -> bool {
        // SAFETY: a COM object of our own, released.
        with_com(|| unsafe {
            let Some(w) = wallpaper() else { return false };
            let f: extern "system" fn(*mut Obj, *const u16, i32) -> i32 = method(w, ADVANCE_SLIDESHOW);
            let done = f(w, std::ptr::null(), 0) >= 0;
            release(w);
            done
        })
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
    pub fn next() -> bool {
        false
    }
    pub fn pick(_folder: bool, _title: &str, _pictures: &str, _owner: isize) -> Option<String> {
        None
    }
}

pub use imp::{next, now, pick, set_folder, set_picture, set_position, set_turns};

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
