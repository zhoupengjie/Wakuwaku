// The default speakers' volume and whether they are muted, for the
// taskbar's volume icon (taskbar/win/keys.rs keys_loop, island-right.ts showVolume): drawn
// as Windows' own taskbar draws it, its waves up to the volume. Asked of
// Windows' Core Audio (IMMDeviceEnumerator, then the default endpoint's
// IAudioEndpointVolume).

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use crate::win32::{Guid, CoCreateInstance, CoInitializeEx, CoUninitialize};

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

    // The volume asked for again and again on one thread (taskbar/
    // keys_loop): the enumerator and the default speakers' volume object are
    // kept between asks (made anew for each one, every ask was a trip to
    // Windows' audio service), the speakers looked up again every so often
    // (the default ones may change) and whenever an ask fails. Lives on the
    // thread that made it, which holds COM up (Com).
    pub struct Reader {
        list: *mut Obj,
        volume: *mut Obj,
        since: Option<std::time::Instant>,
    }

    const RELOOK: std::time::Duration = std::time::Duration::from_secs(10);

    impl Reader {
        pub fn new() -> Self {
            Reader { list: std::ptr::null_mut(), volume: std::ptr::null_mut(), since: None }
        }

        fn let_go(&mut self) {
            if !self.volume.is_null() {
                // SAFETY: ours, released once.
                unsafe { ((*((*self.volume).vtbl as *const VolumeVtbl)).release)(self.volume) };
                self.volume = std::ptr::null_mut();
            }
        }

        // The default speakers' volume object, anew. False with no speakers.
        fn find(&mut self) -> bool {
            self.let_go();
            // SAFETY: as read() below: each object released once used, each
            // out-parameter ours.
            unsafe {
                if self.list.is_null() && (CoCreateInstance(&CLSID_MM_DEVICE_ENUMERATOR, std::ptr::null_mut(), CLSCTX_ALL, &IID_IMM_DEVICE_ENUMERATOR, &mut self.list as *mut *mut Obj as *mut *mut c_void) < 0 || self.list.is_null()) {
                    self.list = std::ptr::null_mut();
                    return false;
                }
                let lv = &*((*self.list).vtbl as *const EnumeratorVtbl);
                let mut device: *mut Obj = std::ptr::null_mut();
                if (lv.get_default_audio_endpoint)(self.list, E_RENDER, E_CONSOLE, &mut device) < 0 || device.is_null() {
                    return false;
                }
                let dv = &*((*device).vtbl as *const DeviceVtbl);
                let made = (dv.activate)(device, &IID_IAUDIO_ENDPOINT_VOLUME, CLSCTX_ALL, std::ptr::null(), &mut self.volume);
                (dv.release)(device);
                if made < 0 || self.volume.is_null() {
                    self.volume = std::ptr::null_mut();
                    return false;
                }
            }
            self.since = Some(std::time::Instant::now());
            true
        }

        pub fn read(&mut self) -> Option<(u8, bool)> {
            if (self.volume.is_null() || self.since.is_some_and(|t| t.elapsed() >= RELOOK)) && !self.find() {
                return None;
            }
            // SAFETY: the volume object is ours and alive; the values are written into our own variables.
            let got = unsafe {
                let vv = &*((*self.volume).vtbl as *const VolumeVtbl);
                let (mut level, mut muted) = (0f32, 0i32);
                let ok = (vv.get_master_volume_level_scalar)(self.volume, &mut level) >= 0 && (vv.get_mute)(self.volume, &mut muted) >= 0;
                ok.then(|| ((level * 100.0).round().clamp(0.0, 100.0) as u8, muted != 0))
            };
            if got.is_none() {
                self.let_go();
            }
            got
        }
    }

    impl Drop for Reader {
        fn drop(&mut self) {
            self.let_go();
            if !self.list.is_null() {
                // SAFETY: ours, released once.
                unsafe { ((*((*self.list).vtbl as *const EnumeratorVtbl)).release)(self.list) };
            }
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

    pub struct Reader;

    impl Reader {
        pub fn new() -> Self {
            Reader
        }
        pub fn read(&mut self) -> Option<(u8, bool)> {
            None
        }
    }
}

pub use imp::{Com, Reader};

#[cfg(test)]
mod tests {
    // Asks this machine's speakers: run by hand (cargo test audio -- --ignored --nocapture).
    #[test]
    #[ignore]
    fn reads_the_speakers() {
        let _com = super::Com::new();
        let got = super::Reader::new().read();
        println!("volume: {got:?}");
        assert!(got.is_some_and(|(level, _)| level <= 100));
    }
}
