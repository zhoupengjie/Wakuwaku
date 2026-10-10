// The desktop's picture on a display, for the taskbar's Mica (taskbar.rs
// sends it, mica.js draws it): which file Windows shows there, how it lays
// it on the display (fill, fit, stretch, center, tile, or one picture across
// all of them), and the colour round it. Asked of Windows' own
// IDesktopWallpaper, which knows each display's (they may differ) and the
// slideshow's picture of the moment. And Windows' accent colour, for what
// the taskbar colours as Windows' own does (accent).

// How the picture is laid, by DESKTOP_WALLPAPER_POSITION.
const POSITIONS: [&str; 6] = ["center", "tile", "stretch", "fit", "fill", "span"];

// Windows' accent colour as #rrggbb: itself, and the lighter one its own
// dark taskbar and menus draw with (on a dark ground the colour itself is
// too dim). From its palette (Explorer\Accent's AccentPalette): eight
// colours of four bytes (red, green, blue, unused), from the lightest
// (Light3, Light2, Light1) through the colour itself to the darkest.
pub fn accent_of(palette: &[u8]) -> Option<(String, String)> {
    let at = |i: usize| palette.get(i * 4..i * 4 + 3).map(|c| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]));
    Some((at(3)?, at(1)?))
}

#[cfg(windows)]
pub fn accent() -> Option<(String, String)> {
    let key = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Accent").ok()?;
    accent_of(&key.get_raw_value("AccentPalette").ok()?.bytes)
}

#[cfg(not(windows))]
pub fn accent() -> Option<(String, String)> {
    None
}

#[derive(Clone, Debug, PartialEq)]
pub struct Paper {
    // None: a plain colour, no picture.
    pub file: Option<std::path::PathBuf>,
    pub position: &'static str,
    // #rrggbb, round a picture that does not cover the display.
    pub color: String,
    // The file's last change (ms since 1970): the same name may hold a new
    // picture (Windows' own copy of the slideshow's or Spotlight's).
    pub stamp: u64,
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::path::PathBuf;

    use super::{Paper, POSITIONS};

    #[repr(C)]
    struct Guid(u32, u16, u16, [u8; 8]);

    const CLSID_DESKTOP_WALLPAPER: Guid = Guid(0xC2CF_3110, 0x460E, 0x4FC1, [0xB9, 0xD0, 0x8A, 0x1C, 0x0C, 0x9C, 0xC4, 0xBD]);
    const IID_IDESKTOP_WALLPAPER: Guid = Guid(0xB92B_56A9, 0x8B55, 0x4E14, [0x9A, 0x89, 0x01, 0x99, 0xBB, 0xB6, 0xF9, 0x3B]);
    const COINIT_APARTMENTTHREADED: u32 = 0x2;
    const CLSCTX_ALL: u32 = 0x17;

    #[repr(C)]
    #[derive(Default, PartialEq)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    // IDesktopWallpaper's table, as far as what is asked of it (the
    // setters' places kept, never called).
    #[repr(C)]
    struct Vtbl {
        _query_interface: usize,
        _add_ref: usize,
        release: extern "system" fn(*mut Obj) -> u32,
        _set_wallpaper: usize,
        get_wallpaper: extern "system" fn(*mut Obj, *const u16, *mut *mut u16) -> i32,
        get_monitor_device_path_at: extern "system" fn(*mut Obj, u32, *mut *mut u16) -> i32,
        get_monitor_device_path_count: extern "system" fn(*mut Obj, *mut u32) -> i32,
        get_monitor_rect: extern "system" fn(*mut Obj, *const u16, *mut Rect) -> i32,
        _set_background_color: usize,
        get_background_color: extern "system" fn(*mut Obj, *mut u32) -> i32,
        _set_position: usize,
        get_position: extern "system" fn(*mut Obj, *mut i32) -> i32,
    }

    #[repr(C)]
    struct Obj {
        vtbl: *const Vtbl,
    }

    #[link(name = "ole32")]
    extern "system" {
        fn CoInitializeEx(reserved: *mut c_void, flags: u32) -> i32;
        fn CoUninitialize();
        fn CoCreateInstance(clsid: *const Guid, outer: *mut c_void, ctx: u32, iid: *const Guid, out: *mut *mut c_void) -> i32;
        fn CoTaskMemFree(p: *mut c_void);
    }

    // A string Windows handed over (CoTaskMemAlloc'd), read and freed.
    fn take(p: *mut u16) -> String {
        if p.is_null() {
            return String::new();
        }
        // SAFETY: a NUL-terminated string from COM, freed once read.
        unsafe {
            let n = (0..).take_while(|&i| *p.add(i) != 0).count();
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, n));
            CoTaskMemFree(p as *mut c_void);
            s
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    // Plain colour, picture, slideshow or Spotlight (Settings' choice).
    fn background_type() -> Option<u32> {
        let key = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Wallpapers").ok()?;
        key.get_value::<u32, _>("BackgroundType").ok()
    }

    // The picture Windows last drew for the desktop, its own copy: what a
    // slideshow or Spotlight shows when their file is not to be had.
    fn transcoded() -> Option<PathBuf> {
        let file = PathBuf::from(std::env::var_os("APPDATA")?).join(r"Microsoft\Windows\Themes\TranscodedWallpaper");
        file.is_file().then_some(file)
    }

    fn stamp(file: &std::path::Path) -> u64 {
        std::fs::metadata(file).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as u64)
    }

    // The desktop's picture on the display at this rectangle (physical).
    pub fn read(mon: (i32, i32, i32, i32)) -> Option<Paper> {
        // SAFETY: COM on this thread for the call, released and left as found;
        // each pointer is ours, of the size the call writes.
        unsafe {
            let init = CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED);
            let mut obj: *mut Obj = std::ptr::null_mut();
            let made = CoCreateInstance(&CLSID_DESKTOP_WALLPAPER, std::ptr::null_mut(), CLSCTX_ALL, &IID_IDESKTOP_WALLPAPER, &mut obj as *mut *mut Obj as *mut *mut c_void);
            let paper = if made >= 0 && !obj.is_null() {
                let v = &*(*obj).vtbl;
                // This display's id: the one where Windows has it.
                let want = Rect { left: mon.0, top: mon.1, right: mon.0 + mon.2, bottom: mon.1 + mon.3 };
                let mut count = 0u32;
                (v.get_monitor_device_path_count)(obj, &mut count);
                let mut id = None;
                for i in 0..count {
                    let mut p = std::ptr::null_mut();
                    if (v.get_monitor_device_path_at)(obj, i, &mut p) < 0 {
                        continue;
                    }
                    let path = take(p);
                    let mut r = Rect::default();
                    if (v.get_monitor_rect)(obj, wide(&path).as_ptr(), &mut r) >= 0 && r == want {
                        id = Some(path);
                        break;
                    }
                }
                let id = id.map(|s| wide(&s));
                let mut p = std::ptr::null_mut();
                (v.get_wallpaper)(obj, id.as_ref().map_or(std::ptr::null(), |w| w.as_ptr()), &mut p);
                let file = take(p);
                let mut color = 0u32;
                (v.get_background_color)(obj, &mut color);
                let mut position = 4i32;
                (v.get_position)(obj, &mut position);
                (v.release)(obj);
                let file = match background_type() {
                    Some(1) => None,
                    _ => Some(PathBuf::from(&file)).filter(|f| !file.is_empty() && f.is_file()).or_else(transcoded),
                };
                Some(Paper {
                    stamp: file.as_deref().map_or(0, stamp),
                    file,
                    position: POSITIONS.get(position as usize).copied().unwrap_or("fill"),
                    // COLORREF: 0x00bbggrr.
                    color: format!("#{:02x}{:02x}{:02x}", color & 0xFF, (color >> 8) & 0xFF, (color >> 16) & 0xFF),
                })
            } else {
                None
            };
            if init >= 0 {
                CoUninitialize();
            }
            paper
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::Paper;

    pub fn read(_mon: (i32, i32, i32, i32)) -> Option<Paper> {
        None
    }
}

pub use imp::read;

#[cfg(test)]
mod tests {
    use super::*;

    // On a desktop: the main display's picture, as Windows says (none
    // where there is no desktop to ask).
    #[test]
    fn reads_the_main_displays_picture() {
        let Some(paper) = read((0, 0, 1920, 1080)) else { return };
        println!("{paper:?}");
        assert!(POSITIONS.contains(&paper.position));
        assert!(paper.color.len() == 7 && paper.color.starts_with('#'));
        assert!(paper.file.as_ref().is_none_or(|f| f.is_file()));
        assert_eq!(paper.stamp == 0, paper.file.is_none());
    }

    // Windows' default blue, as its palette holds it.
    #[test]
    fn reads_the_accent_and_its_light_one_from_the_palette() {
        let palette = [0x99, 0xEB, 0xFF, 0, 0x4C, 0xC2, 0xFF, 0, 0x00, 0x91, 0xF8, 0, 0x00, 0x78, 0xD4, 0, 0x00, 0x67, 0xC0, 0, 0x00, 0x3E, 0x92, 0, 0x00, 0x1A, 0x68, 0, 0xF7, 0x63, 0x0C, 0];
        assert_eq!(accent_of(&palette), Some(("#0078d4".into(), "#4cc2ff".into())));
        assert_eq!(accent_of(&palette[..12]), None);
    }
}
