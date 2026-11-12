// An app's name and icon as Windows' taskbar shows them, asked of the shell
// by the app's id (its Apps folder's item).
use super::*;

// An app's icon as Windows' taskbar shows it, asked of the shell by the
// app's id (tasks::app_id): its Apps folder's item, drawn by the shell at
// `size` from the app's own pictures (a store app's plain one for the
// taskbar, Settings' grey cog). Each asked once; None where the shell
// knows no such app.
#[repr(C)]
pub(super) struct KnownGuid(u32, u16, u16, [u8; 8]);

#[repr(C)]
pub(super) struct ImageSize {
    cx: i32,
    cy: i32,
}

// IShellItemImageFactory's table.
#[repr(C)]
pub(super) struct FactoryVtbl {
    _query_interface: usize,
    _add_ref: usize,
    release: extern "system" fn(*mut Factory) -> u32,
    get_image: extern "system" fn(*mut Factory, ImageSize, i32, *mut isize) -> i32,
}

#[repr(C)]
pub(super) struct Factory {
    vtbl: *const FactoryVtbl,
}

#[link(name = "shell32")]
extern "system" {
    pub(super) fn SHCreateItemInKnownFolder(folder: *const KnownGuid, flags: u32, item: *const u16, iid: *const KnownGuid, out: *mut *mut c_void) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    pub(super) fn BitBlt(dst: isize, x: i32, y: i32, w: i32, h: i32, src: isize, sx: i32, sy: i32, rop: u32) -> i32;
    pub(super) fn GetDIBits(dc: isize, bmp: isize, start: u32, lines: u32, bits: *mut u8, info: *mut BitmapInfoHeader, usage: u32) -> i32;
}

#[link(name = "user32")]
extern "system" {
    pub(super) fn GetDC(hwnd: Hwnd) -> isize;
    pub(super) fn ReleaseDC(hwnd: Hwnd, dc: isize) -> i32;
    pub(super) fn MonitorFromWindow(hwnd: Hwnd, flags: u32) -> *mut c_void;
}

pub(super) const FOLDERID_APPS: KnownGuid = KnownGuid(0x1E87_508D, 0x89C2, 0x42F0, [0x8A, 0x7E, 0x64, 0x5A, 0x0F, 0x50, 0xCA, 0x58]);
pub(super) const IID_IMAGE_FACTORY: KnownGuid = KnownGuid(0xBCC1_8B79, 0xBA16, 0x442F, [0x80, 0xC4, 0x8A, 0x59, 0xC3, 0x0C, 0x46, 0x3B]);
pub(super) const SIIGBF_ICONONLY: i32 = 0x4;

pub(super) static APP_PNGS: Mutex<Option<HashMap<(String, i32), Option<String>>>> = Mutex::new(None);

// IShellItem's table, as far as its name.
#[repr(C)]
pub(super) struct ItemVtbl {
    _query_interface: usize,
    _add_ref: usize,
    release: extern "system" fn(*mut Item) -> u32,
    _bind_to_handler: usize,
    _get_parent: usize,
    get_display_name: extern "system" fn(*mut Item, i32, *mut *mut u16) -> i32,
}

#[repr(C)]
pub(super) struct Item {
    vtbl: *const ItemVtbl,
}

#[link(name = "ole32")]
extern "system" {
    pub(super) fn CoTaskMemFree(p: *mut c_void);
}

pub(super) const IID_SHELL_ITEM: KnownGuid = KnownGuid(0x4382_6D1E, 0xE718, 0x42EE, [0xBC, 0x55, 0xA1, 0xE2, 0x61, 0xC3, 0x7B, 0xFE]);
pub(super) const SIGDN_NORMALDISPLAY: i32 = 0;

// An app's name as the shell's Apps folder (the Start menu) has it.
pub(super) fn app_title(app_id: &str) -> Option<String> {
    tasks::with_com();
    // SAFETY: the shell's item, released; its name freed once read.
    unsafe {
        let mut item: *mut Item = std::ptr::null_mut();
        if SHCreateItemInKnownFolder(&FOLDERID_APPS, 0, wide(app_id).as_ptr(), &IID_SHELL_ITEM, &mut item as *mut *mut Item as *mut *mut c_void) < 0 || item.is_null() {
            return None;
        }
        let mut name: *mut u16 = std::ptr::null_mut();
        let got = ((*(*item).vtbl).get_display_name)(item, SIGDN_NORMALDISPLAY, &mut name) >= 0 && !name.is_null();
        let title = got.then(|| {
            let n = (0..).take_while(|&i| *name.add(i) != 0).count();
            String::from_utf16_lossy(std::slice::from_raw_parts(name, n))
        });
        if !name.is_null() {
            CoTaskMemFree(name as *mut c_void);
        }
        ((*(*item).vtbl).release)(item);
        title.filter(|t| !t.is_empty())
    }
}

pub(super) fn app_png(app_id: &str, size: i32) -> Option<String> {
    let key = (app_id.to_string(), size);
    if let Some(known) = APP_PNGS.lock().unwrap().get_or_insert_with(HashMap::new).get(&key) {
        return known.clone();
    }
    tasks::with_com();
    // SAFETY: the shell's item and bitmap, released and deleted; the
    // pixels read into our own buffer of the size asked for.
    let rgba = unsafe {
        let mut factory: *mut Factory = std::ptr::null_mut();
        let made = SHCreateItemInKnownFolder(&FOLDERID_APPS, 0, wide(app_id).as_ptr(), &IID_IMAGE_FACTORY, &mut factory as *mut *mut Factory as *mut *mut c_void);
        let mut bmp = 0isize;
        let drawn = made >= 0 && !factory.is_null() && ((*(*factory).vtbl).get_image)(factory, ImageSize { cx: size, cy: size }, SIIGBF_ICONONLY, &mut bmp) >= 0 && bmp != 0;
        if !factory.is_null() {
            ((*(*factory).vtbl).release)(factory);
        }
        let mut bm: BitmapObj = std::mem::zeroed();
        let sized = drawn && GetObjectW(bmp, std::mem::size_of::<BitmapObj>() as i32, &mut bm as *mut BitmapObj as *mut c_void) != 0 && bm.width > 0 && bm.height > 0 && bm.width <= 256 && bm.height <= 256;
        let pixels = sized.then(|| {
            let (w, h) = (bm.width, bm.height);
            let mut info = BitmapInfoHeader { size: 40, width: w, height: -h, planes: 1, bit_count: 32, compression: 0, size_image: 0, x_ppm: 0, y_ppm: 0, clr_used: 0, clr_important: 0 };
            let mut bgra = vec![0u8; (w * h * 4) as usize];
            let dc = GetDC(std::ptr::null_mut());
            let lines = GetDIBits(dc, bmp, 0, h as u32, bgra.as_mut_ptr(), &mut info, 0);
            ReleaseDC(std::ptr::null_mut(), dc);
            (lines == h).then(|| {
                for px in bgra.chunks_exact_mut(4) {
                    px.swap(0, 2);
                }
                if (w, h) == (size, size) { bgra } else { crate::taskbar::resize(&bgra, w as usize, h as usize, size as usize, size as usize) }
            })
        });
        if bmp != 0 {
            DeleteObject(bmp);
        }
        pixels.flatten()
    };
    let png = rgba.filter(|p| p.chunks_exact(4).any(|px| px[3] != 0)).and_then(|rgba| {
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, size as u32, size as u32);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().ok()?;
            writer.write_image_data(&rgba).ok()?;
        }
        Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(out)))
    });
    APP_PNGS.lock().unwrap().get_or_insert_with(HashMap::new).insert(key, png.clone());
    png
}

#[cfg(test)]
mod app_icon_tests {
    // On a desktop: Settings' icon by its app id, at the size asked;
    // nothing for an id the shell does not know.
    #[test]
    fn the_shell_draws_an_app_by_its_id() {
        assert_eq!(super::app_png("no.such.app_0000000000000!Nothing", 30), None);
        let Some(png) = super::app_png("windows.immersivecontrolpanel_cw5n1h2txyewy!microsoft.windows.immersivecontrolpanel", 30) else { return };
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD.decode(png.trim_start_matches("data:image/png;base64,")).unwrap();
        let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        let info = decoder.read_info().unwrap();
        assert_eq!((info.info().width, info.info().height), (30, 30));
    }
}
