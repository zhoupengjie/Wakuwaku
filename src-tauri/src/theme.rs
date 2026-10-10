// Windows' own light or dark mode and accent colour, set from the settings
// (settings.rs settings_win_look), as Settings → Personalization → Colors
// sets them. The taskbar then follows (wallpaper.rs look, re-read on the
// change Windows announces).
//
// The mode: Themes\Personalize's SystemUsesLightTheme (Windows: the taskbar,
// Start) and AppsUseLightTheme (the programs), then "ImmersiveColorSet"
// announced to every window. The accent: Windows' own call for it
// (uxtheme's SetUserColorPreference, unnamed, ordinal 122, whose reading
// twin, 120, gives Explorer\Accent's StartColorMenu and AccentColorMenu),
// which works out the shades the rest of Windows uses. Should Windows not
// take it (AccentColorMenu, the colour picked, unchanged), the shades are
// worked out here (palette) and written as Windows keeps them. The windows'
// frames' colour is DWM's, kept in its memory and written from there over
// the registry: written to the registry before Windows' call (which reads
// it there for DWM a moment later), then looked at in DWM (dwmapi's
// unnamed 127) once Windows is done, and set through DWM (131) if it is not.

// Lighter and darker shades of a colour, as Windows keeps its accent's
// (Explorer\Accent's AccentPalette, wallpaper.rs accent_of): Light3, Light2,
// Light1, the colour, Dark1, Dark2, Dark3, and an eighth Windows keeps
// there (an orange of its own), each red, green, blue and a zero.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn palette(rgb: [u8; 3]) -> [u8; 32] {
    let (h, s, l) = to_hsl(rgb);
    let shades = [l + (1.0 - l) * 0.62, l + (1.0 - l) * 0.38, l + (1.0 - l) * 0.14, l, l * 0.86, l * 0.6, l * 0.34];
    let mut out = [0u8; 32];
    for (i, l) in shades.iter().enumerate() {
        let c = if i == 3 { rgb } else { from_hsl(h, s, l.clamp(0.0, 1.0)) };
        out[i * 4..i * 4 + 3].copy_from_slice(&c);
    }
    out[28..31].copy_from_slice(&[0xF7, 0x63, 0x0C]);
    out
}

fn to_hsl([r, g, b]: [u8; 3]) -> (f64, f64, f64) {
    let (r, g, b) = (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) / 2.0;
    if max == min {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == r { (g - b) / d + if g < b { 6.0 } else { 0.0 } } else if max == g { (b - r) / d + 2.0 } else { (r - g) / d + 4.0 };
    (h / 6.0, s, l)
}

fn from_hsl(h: f64, s: f64, l: f64) -> [u8; 3] {
    if s == 0.0 {
        let v = (l * 255.0).round() as u8;
        return [v, v, v];
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let hue = |t: f64| {
        let t = t.rem_euclid(1.0);
        let v = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (v * 255.0).round().clamp(0.0, 255.0) as u8
    };
    [hue(h + 1.0 / 3.0), hue(h), hue(h - 1.0 / 3.0)]
}

// "#rrggbb" to its bytes.
pub fn parse(hex: &str) -> Option<[u8; 3]> {
    let hex = hex.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(hex, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

#[cfg(windows)]
mod imp {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
    use winreg::{RegKey, RegValue};

    // Its two colours, 0x00bbggrr each (the top byte as Windows keeps it, 0xff).
    #[repr(C)]
    struct ColorPreference {
        start: u32,
        accent: u32,
    }

    type SetUserColorPreference = extern "system" fn(*const ColorPreference, i32) -> i32;

    // DWM's colours for the windows' frames, as it keeps them in memory (and
    // writes them over the registry's DWM values from there): read by
    // dwmapi's export 127 (DwmGetColorizationParameters), set by 131.
    #[repr(C)]
    #[derive(Default)]
    struct Colorization {
        color: u32,
        afterglow: u32,
        intensity: u32,
        afterglow_balance: u32,
        blur_balance: u32,
        reflection: u32,
        opaque: u32,
    }

    type GetColorization = extern "system" fn(*mut Colorization) -> i32;
    type SetColorization = extern "system" fn(*const Colorization, u32) -> i32;

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(name: *const u16) -> isize;
        fn GetProcAddress(module: isize, name: *const u8) -> *const ();
    }

    #[link(name = "user32")]
    extern "system" {
        fn SendMessageTimeoutW(hwnd: *mut std::ffi::c_void, msg: u32, wparam: usize, lparam: isize, flags: u32, ms: u32, result: *mut usize) -> isize;
    }

    // A system library's export that has no name, by its number; null
    // should it be missing.
    fn export(dll: &str, number: usize) -> *const () {
        let name: Vec<u16> = dll.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: a system library (loaded already in any program with
        // windows), and an export asked for by its number, as Windows allows.
        unsafe {
            let module = LoadLibraryW(name.as_ptr());
            if module == 0 {
                return std::ptr::null();
            }
            GetProcAddress(module, number as *const u8)
        }
    }

    // uxtheme's export 122.
    fn set_user_color_preference() -> Option<SetUserColorPreference> {
        let f = export("uxtheme.dll", 122);
        // SAFETY: the export of this number has this shape (its reading twin,
        // 120, checked against the registry, 2026-10-10).
        (!f.is_null()).then(|| unsafe { std::mem::transmute::<*const (), SetUserColorPreference>(f) })
    }

    fn colorization() -> Option<Colorization> {
        let f = export("dwmapi.dll", 127);
        if f.is_null() {
            return None;
        }
        // SAFETY: as set_user_color_preference; its seven fields read back sound.
        let get = unsafe { std::mem::transmute::<*const (), GetColorization>(f) };
        let mut c = Colorization::default();
        (get(&mut c) >= 0).then_some(c)
    }

    // DWM's frames' colour (and its afterglow) set to this one, the rest of
    // its colours as they were; its colour then, read back.
    fn set_colorization(argb: u32) -> Option<u32> {
        let mut c = colorization()?;
        let f = export("dwmapi.dll", 131);
        if f.is_null() {
            return None;
        }
        // SAFETY: as colorization, the same struct handed back.
        let set = unsafe { std::mem::transmute::<*const (), SetColorization>(f) };
        c.color = argb;
        c.afterglow = argb;
        set(&c, 1);
        colorization().map(|c| c.color)
    }

    const HWND_BROADCAST: isize = 0xffff;
    const WM_SETTINGCHANGE: u32 = 0x001A;
    const SMTO_ABORTIFHUNG: u32 = 0x2;

    // Every window told the colours changed, as Settings tells them.
    fn announce() {
        let what: Vec<u16> = "ImmersiveColorSet".encode_utf16().chain(std::iter::once(0)).collect();
        let mut answer = 0usize;
        // SAFETY: a string of our own, alive for the call.
        unsafe { SendMessageTimeoutW(HWND_BROADCAST as *mut std::ffi::c_void, WM_SETTINGCHANGE, 0, what.as_ptr() as isize, SMTO_ABORTIFHUNG, 200, &mut answer) };
    }

    pub fn set_mode(light: bool) -> bool {
        let Ok((key, _)) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize") else { return false };
        let v = light as u32;
        let ok = key.set_value("SystemUsesLightTheme", &v).is_ok() && key.set_value("AppsUseLightTheme", &v).is_ok();
        announce();
        ok
    }

    fn abgr([r, g, b]: [u8; 3]) -> u32 {
        0xff00_0000 | (b as u32) << 16 | (g as u32) << 8 | r as u32
    }

    // The accent colour picked, as "#rrggbb": Explorer\Accent's
    // AccentColorMenu (0xffbbggrr), not its palette's middle shade, which
    // Windows makes a little off it (#0078d4 for the #0078d7 picked).
    pub fn current_accent() -> Option<String> {
        let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Accent").ok()?;
        let v: u32 = key.get_value("AccentColorMenu").ok()?;
        Some(format!("#{:02x}{:02x}{:02x}", v & 0xff, (v >> 8) & 0xff, (v >> 16) & 0xff))
    }

    // The registry's DWM colours (its frames' and their afterglow, its
    // accent), and its frames' as read there.
    fn write_dwm_registry(rgb: [u8; 3], argb: u32) {
        if let Ok((dwm, _)) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(r"Software\Microsoft\Windows\DWM") {
            let _ = dwm.set_value("AccentColor", &abgr(rgb));
            let _ = dwm.set_value("ColorizationColor", &argb);
            let _ = dwm.set_value("ColorizationAfterglow", &argb);
        }
    }

    fn dwm_registry() -> Option<u32> {
        RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Software\Microsoft\Windows\DWM").ok()?.get_value("ColorizationColor").ok()
    }

    fn hex(c: Option<u32>) -> String {
        c.map_or("none".to_string(), |c| format!("{c:#010x}"))
    }

    // What came of it, for the log; what came of the frames' colour a moment
    // later told to `later`.
    pub fn set_accent(rgb: [u8; 3], later: impl Fn(String) + Send + 'static) -> String {
        let shades = super::palette(rgb);
        let shade = |i: usize| [shades[i * 4], shades[i * 4 + 1], shades[i * 4 + 2]];
        let [r, g, b] = rgb;
        let argb = 0xc400_0000 | (r as u32) << 16 | (g as u32) << 8 | b as u32;
        // Not from the desktop's picture any more: this colour.
        if let Ok(desktop) = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(r"Control Panel\Desktop", KEY_SET_VALUE) {
            let _ = desktop.set_value("AutoColorization", &"0");
        }
        // The windows' frames' colour first, where Windows' call goes on to
        // read it for DWM, a moment after it returns: left as it was, DWM
        // ended on the colour picked before (2026-10-10), every time.
        write_dwm_registry(rgb, argb);
        // Start's colour a shade darker than the accent, as Windows pairs them.
        let pref = ColorPreference { start: abgr(shade(5)), accent: abgr(rgb) };
        let set = set_user_color_preference().is_some_and(|f| f(&pref, 1) >= 0);
        // Taken by Windows (its picked colour now this one, its shades worked
        // out by it), which writes it down a moment after; else written here
        // as Windows keeps them.
        let want = format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]);
        let asked = std::time::Instant::now();
        let mut taken = false;
        while set && !taken && asked.elapsed() < std::time::Duration::from_millis(600) {
            taken = current_accent().as_deref() == Some(want.as_str());
            if !taken {
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
        }
        if !taken {
            if let Ok((accent, _)) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Accent") {
                let _ = accent.set_raw_value("AccentPalette", &RegValue { bytes: shades.to_vec().into(), vtype: winreg::enums::RegType::REG_BINARY });
                let _ = accent.set_value("AccentColorMenu", &abgr(rgb));
                let _ = accent.set_value("StartColorMenu", &abgr(shade(5)));
            }
            // Told round only when written here: Windows' call tells it itself.
            announce();
        }
        let first = colorization().map(|c| c.color);
        // DWM's, once Windows has done with it (it hands DWM the colour a
        // while after): looked at again, and set through DWM should it be
        // another, the registry's with it; looked at once more.
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(1200));
            let seen = colorization().map(|c| c.color);
            let mut note = format!("frames (DWM) 1.2 s on {}", hex(seen));
            if seen != Some(argb) {
                let _ = set_colorization(argb);
                write_dwm_registry(rgb, argb);
                std::thread::sleep(std::time::Duration::from_millis(1000));
                note += &format!(", set again, 1 s on {}", hex(colorization().map(|c| c.color)));
            }
            later(format!("{note}; registry's {}; wanted {argb:#010x}", hex(dwm_registry())));
        });
        format!("called {set}, taken by Windows {taken}; frames (DWM) right after {}, wanted {argb:#010x}", hex(first))
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn set_mode(_light: bool) -> bool {
        false
    }
    pub fn current_accent() -> Option<String> {
        None
    }
    pub fn set_accent(_rgb: [u8; 3], _later: impl Fn(String) + Send + 'static) -> String {
        String::new()
    }
}

pub use imp::{current_accent, set_accent, set_mode};

#[cfg(test)]
mod tests {
    use super::{palette, parse};

    #[test]
    fn palette_keeps_the_colour_in_the_middle_and_shades_round_it() {
        let rgb = parse("#0078d7").unwrap();
        let p = palette(rgb);
        assert_eq!(&p[12..15], &rgb);
        let lum = |i: usize| p[i * 4] as u32 * 299 + p[i * 4 + 1] as u32 * 587 + p[i * 4 + 2] as u32 * 114;
        // Lightest first, darkest last.
        assert!((0..6).all(|i| lum(i) > lum(i + 1)));
        assert_eq!(&p[28..31], &[0xF7, 0x63, 0x0C]);
    }

    #[test]
    fn parse_takes_only_six_hex_digits() {
        assert_eq!(parse("#ff8c00"), Some([0xff, 0x8c, 0x00]));
        assert_eq!(parse("ff8c00"), None);
        assert_eq!(parse("#fff"), None);
    }
}
