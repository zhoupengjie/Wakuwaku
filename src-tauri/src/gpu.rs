// The graphics card that drives a display, for WebView2 to render her pages
// on (#13). WebView2 renders on the card Windows prefers for it (the
// power-saving one: the integrated), wherever her windows are: on a display
// the discrete card drives, each frame was drawn on the integrated one and
// copied across, and her animations stuttered while the integrated one was
// busy (a video playing in Chrome, there too). Rendered on the card that
// drives her display, the copy is gone; and on a display the integrated
// card drives, the discrete one is never woken for her.
//
// Asked of Windows' display kernel by the display's GDI name: its card's
// LUID, and whether that card renders at all (a remote desktop's virtual
// display, an indirect one, does not: then Chromium chooses, as before).
// The monitor asks the same kernel about its cards (hw.rs, through kmt).

// In D3DKMT_ADAPTERTYPE: RenderSupported, SoftwareDevice, IndirectDisplayDevice.
const RENDERS: u32 = 0x1;
const SOFTWARE: u32 = 0x4;
const INDIRECT: u32 = 0x40;

// A card that renders and is neither Microsoft's software renderer nor an
// indirect display (a virtual one, posing as the card it sits on).
pub fn renders(flags: u32) -> bool {
    flags & RENDERS != 0 && flags & (SOFTWARE | INDIRECT) == 0
}

// The display kernel's adapters, asked by their handle (gdi32's D3DKMT*).
#[cfg(windows)]
pub mod kmt {
    use std::ffi::c_void;

    // D3DKMT_QUERYADAPTERINFO.
    #[repr(C)]
    struct QueryInfo {
        adapter: u32,
        kind: u32,
        data: *mut c_void,
        size: u32,
    }

    // D3DKMT_CLOSEADAPTER.
    #[repr(C)]
    struct Close {
        adapter: u32,
    }

    // D3DKMT_ENUMADAPTERS2, and its D3DKMT_ADAPTERINFO (20 bytes each).
    #[repr(C)]
    struct Enum2 {
        count: u32,
        adapters: *mut Info,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Info {
        adapter: u32,
        luid_low: u32,
        luid_high: i32,
        sources: u32,
        precise: i32,
    }

    #[link(name = "gdi32")]
    extern "system" {
        fn D3DKMTQueryAdapterInfo(query: *const QueryInfo) -> i32;
        fn D3DKMTCloseAdapter(close: *const Close) -> i32;
    }

    // KMTQAITYPE_*: what is asked.
    pub const GETSEGMENTSIZE: u32 = 3;
    pub const ADAPTERREGISTRYINFO: u32 = 8;
    pub const ADAPTERTYPE: u32 = 15;
    pub const NODEMETADATA: u32 = 25;
    pub const PHYSICALADAPTERDEVICEIDS: u32 = 31;
    pub const NODEPERFDATA: u32 = 61;
    pub const ADAPTERPERFDATA: u32 = 62;
    pub const DRIVER_DESCRIPTION: u32 = 65;

    // Asked of an adapter, the answer in `data` (8-aligned; `size` bytes of
    // it, as large as the answer's struct exactly: some are refused
    // otherwise). True when it answered.
    pub fn query(adapter: u32, kind: u32, data: &mut [u64], size: usize) -> bool {
        if size > data.len() * 8 {
            return false;
        }
        let query = QueryInfo { adapter, kind, data: data.as_mut_ptr() as *mut c_void, size: size as u32 };
        // SAFETY: a buffer of at least `size` bytes, ours.
        unsafe { D3DKMTQueryAdapterInfo(&query) == 0 }
    }

    pub fn close(adapter: u32) {
        // SAFETY: an adapter opened by the display kernel for us, closed once.
        unsafe { D3DKMTCloseAdapter(&Close { adapter }) };
    }

    // Every adapter there is, opened (each to be closed): its handle and its
    // LUID (high << 32 | low). D3DKMTEnumAdapters2 looked up in gdi32 when
    // asked, not linked.
    pub fn adapters() -> Vec<(u32, u64)> {
        type Enumerate = unsafe extern "system" fn(e: *mut Enum2) -> i32;
        let Some(f) = crate::hw::pdh::system_library("gdi32.dll").and_then(|f| f(c"D3DKMTEnumAdapters2")) else { return Vec::new() };
        // SAFETY: the export in the shape documented for it; first the count,
        // then a list that long (again a little longer should one have come
        // between).
        unsafe {
            let enumerate = std::mem::transmute::<*const (), Enumerate>(f);
            let mut e = Enum2 { count: 0, adapters: std::ptr::null_mut() };
            if enumerate(&mut e) != 0 {
                return Vec::new();
            }
            for more in [0, 4] {
                let mut list = vec![Info::default(); e.count as usize + more];
                e = Enum2 { count: list.len() as u32, adapters: list.as_mut_ptr() };
                if enumerate(&mut e) == 0 {
                    list.truncate(e.count as usize);
                    return list.iter().map(|a| (a.adapter, ((a.luid_high as u32 as u64) << 32) | u64::from(a.luid_low))).collect();
                }
            }
            Vec::new()
        }
    }
}

#[cfg(windows)]
mod imp {
    use crate::win32::{MonitorInfo, Point, Rect, GetMonitorInfoW, MonitorFromPoint};

    // MONITORINFOEXW: MONITORINFO with the display's name after it, which
    // GetMonitorInfoW fills as well when the size says so.
    #[repr(C)]
    #[derive(Default)]
    struct MonitorInfoEx {
        size: u32,
        monitor: Rect,
        work: Rect,
        flags: u32,
        device: [u16; 32],
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Luid {
        low: u32,
        high: i32,
    }

    // D3DKMT_OPENADAPTERFROMGDIDISPLAYNAME.
    #[repr(C)]
    #[derive(Default)]
    struct OpenFromName {
        device: [u16; 32],
        adapter: u32,
        luid: Luid,
        source: u32,
    }

    #[link(name = "gdi32")]
    extern "system" {
        fn D3DKMTOpenAdapterFromGdiDisplayName(open: *mut OpenFromName) -> i32;
    }

    const MONITOR_DEFAULTTONEAREST: u32 = 2;

    // The card driving the display at (x, y) (physical px): its LUID (high,
    // low), and the display's name, if it renders.
    pub fn card_of(x: i32, y: i32) -> Option<((i32, u32), String)> {
        // SAFETY: our own structs, sized as the calls ask; the adapter opened is closed.
        unsafe {
            let monitor = MonitorFromPoint(Point { x, y }, MONITOR_DEFAULTTONEAREST);
            let mut info = MonitorInfoEx { size: std::mem::size_of::<MonitorInfoEx>() as u32, ..Default::default() };
            if monitor.is_null() || GetMonitorInfoW(monitor, &mut info as *mut MonitorInfoEx as *mut MonitorInfo) == 0 {
                return None;
            }
            let mut open = OpenFromName { device: info.device, ..Default::default() };
            if D3DKMTOpenAdapterFromGdiDisplayName(&mut open) != 0 {
                return None;
            }
            let mut kind = [0u64; 1];
            let asked = super::kmt::query(open.adapter, super::kmt::ADAPTERTYPE, &mut kind, 4);
            super::kmt::close(open.adapter);
            let len = info.device.iter().position(|&c| c == 0).unwrap_or(32);
            let name = String::from_utf16_lossy(&info.device[..len]);
            (asked && super::renders(kind[0] as u32)).then_some(((open.luid.high, open.luid.low), name))
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn card_of(_x: i32, _y: i32) -> Option<((i32, u32), String)> {
        None
    }
}

pub use imp::card_of;

// Chromium's switch for it (ui/gl/gl_display.cc reads "<high>,<low>", in
// decimal), and the display's name for the log; none when the card cannot
// be told.
pub fn switch_for(display: (i32, i32, i32, i32)) -> Option<(String, String)> {
    let (x, y, w, h) = display;
    card_of(x + w / 2, y + h / 2).map(|((high, low), name)| (format!("--use-adapter-luid={high},{low}"), name))
}
