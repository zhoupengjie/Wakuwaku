// The default speakers' volume and whether they are muted, for the
// taskbar's volume icon (taskbar.rs keys_loop, island.js showVolume): drawn
// as Windows' own taskbar draws it, its waves up to the volume. Asked of
// Windows' Core Audio (IMMDeviceEnumerator, then the default endpoint's
// IAudioEndpointVolume).

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;

    #[repr(C)]
    struct Guid(u32, u16, u16, [u8; 8]);

    const CLSID_MM_DEVICE_ENUMERATOR: Guid = Guid(0xBCDE_0395, 0xE52F, 0x467C, [0x8E, 0x3D, 0xC4, 0x57, 0x92, 0x91, 0x69, 0x2E]);
    const IID_IMM_DEVICE_ENUMERATOR: Guid = Guid(0xA956_64D2, 0x9614, 0x4F35, [0xA7, 0x46, 0xDE, 0x8D, 0xB6, 0x36, 0x17, 0xE6]);
    const IID_IAUDIO_ENDPOINT_VOLUME: Guid = Guid(0x5CDF_2C82, 0x841E, 0x4546, [0x97, 0x22, 0x0C, 0xF7, 0x40, 0x78, 0x22, 0x9A]);
    const COINIT_MULTITHREADED: u32 = 0x0;
    const CLSCTX_ALL: u32 = 0x17;
    // eRender, eConsole: the speakers Windows plays to by default.
    const E_RENDER: i32 = 0;
    const E_CONSOLE: i32 = 0;

    #[repr(C)]
    struct Obj {
        vtbl: *const c_void,
    }

    // The three tables, as far as what is asked of them (the rest's places kept).
    #[repr(C)]
    struct EnumeratorVtbl {
        _query_interface: usize,
        _add_ref: usize,
        release: extern "system" fn(*mut Obj) -> u32,
        _enum_audio_endpoints: usize,
        get_default_audio_endpoint: extern "system" fn(*mut Obj, i32, i32, *mut *mut Obj) -> i32,
    }

    #[repr(C)]
    struct DeviceVtbl {
        _query_interface: usize,
        _add_ref: usize,
        release: extern "system" fn(*mut Obj) -> u32,
        activate: extern "system" fn(*mut Obj, *const Guid, u32, *const c_void, *mut *mut Obj) -> i32,
    }

    #[repr(C)]
    struct VolumeVtbl {
        _query_interface: usize,
        _add_ref: usize,
        release: extern "system" fn(*mut Obj) -> u32,
        _register_control_change_notify: usize,
        _unregister_control_change_notify: usize,
        _get_channel_count: usize,
        _set_master_volume_level: usize,
        _set_master_volume_level_scalar: usize,
        _get_master_volume_level: usize,
        get_master_volume_level_scalar: extern "system" fn(*mut Obj, *mut f32) -> i32,
        _set_channel_volume_level: usize,
        _set_channel_volume_level_scalar: usize,
        _get_channel_volume_level: usize,
        _get_channel_volume_level_scalar: usize,
        _set_mute: usize,
        get_mute: extern "system" fn(*mut Obj, *mut i32) -> i32,
    }

    #[link(name = "ole32")]
    extern "system" {
        fn CoInitializeEx(reserved: *mut c_void, flags: u32) -> i32;
        fn CoUninitialize();
        fn CoCreateInstance(clsid: *const Guid, outer: *mut c_void, ctx: u32, iid: *const Guid, out: *mut *mut c_void) -> i32;
    }

    // COM kept up on this thread while it lives: each read() then only
    // counts it up and down (no loading and unloading four times a second).
    pub struct Com(bool);

    impl Com {
        pub fn new() -> Self {
            // SAFETY: balanced by the drop.
            Com(unsafe { CoInitializeEx(std::ptr::null_mut(), COINIT_MULTITHREADED) } >= 0)
        }
    }

    impl Drop for Com {
        fn drop(&mut self) {
            if self.0 {
                // SAFETY: the initialization this undoes.
                unsafe { CoUninitialize() };
            }
        }
    }

    // The volume (0 to 100) and whether muted; None with no speakers.
    pub fn read() -> Option<(u8, bool)> {
        // SAFETY: COM on this thread for the call and left as found; each
        // object released once used; each out-parameter ours.
        unsafe {
            let init = CoInitializeEx(std::ptr::null_mut(), COINIT_MULTITHREADED);
            let got = (|| {
                let mut list: *mut Obj = std::ptr::null_mut();
                if CoCreateInstance(&CLSID_MM_DEVICE_ENUMERATOR, std::ptr::null_mut(), CLSCTX_ALL, &IID_IMM_DEVICE_ENUMERATOR, &mut list as *mut *mut Obj as *mut *mut c_void) < 0 || list.is_null() {
                    return None;
                }
                let lv = &*((*list).vtbl as *const EnumeratorVtbl);
                let mut device: *mut Obj = std::ptr::null_mut();
                let found = (lv.get_default_audio_endpoint)(list, E_RENDER, E_CONSOLE, &mut device);
                (lv.release)(list);
                if found < 0 || device.is_null() {
                    return None;
                }
                let dv = &*((*device).vtbl as *const DeviceVtbl);
                let mut volume: *mut Obj = std::ptr::null_mut();
                let made = (dv.activate)(device, &IID_IAUDIO_ENDPOINT_VOLUME, CLSCTX_ALL, std::ptr::null(), &mut volume);
                (dv.release)(device);
                if made < 0 || volume.is_null() {
                    return None;
                }
                let vv = &*((*volume).vtbl as *const VolumeVtbl);
                let (mut level, mut muted) = (0f32, 0i32);
                let ok = (vv.get_master_volume_level_scalar)(volume, &mut level) >= 0 && (vv.get_mute)(volume, &mut muted) >= 0;
                (vv.release)(volume);
                ok.then(|| ((level * 100.0).round().clamp(0.0, 100.0) as u8, muted != 0))
            })();
            if init >= 0 {
                CoUninitialize();
            }
            got
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub struct Com;

    impl Com {
        pub fn new() -> Self {
            Com
        }
    }

    pub fn read() -> Option<(u8, bool)> {
        None
    }
}

pub use imp::{read, Com};

#[cfg(test)]
mod tests {
    // Asks this machine's speakers: run by hand (cargo test audio -- --ignored --nocapture).
    #[test]
    #[ignore]
    fn reads_the_speakers() {
        let _com = super::Com::new();
        let got = super::read();
        println!("volume: {got:?}");
        assert!(got.is_some_and(|(level, _)| level <= 100));
    }
}
