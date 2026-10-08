// The foreground window: noted before a window of hers takes the keyboard,
// and handed back when she lets go of it (if she still has it). Windows only.

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;

    #[link(name = "user32")]
    extern "system" {
        fn GetForegroundWindow() -> *mut c_void;
        fn SetForegroundWindow(hwnd: *mut c_void) -> i32;
        fn IsWindow(hwnd: *mut c_void) -> i32;
    }

    pub fn foreground() -> isize {
        // SAFETY: no arguments.
        unsafe { GetForegroundWindow() as isize }
    }

    pub fn set_foreground(hwnd: isize) -> bool {
        // SAFETY: a handle we noted; IsWindow guards one that has gone since.
        unsafe { hwnd != 0 && IsWindow(hwnd as *mut c_void) != 0 && SetForegroundWindow(hwnd as *mut c_void) != 0 }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn foreground() -> isize {
        0
    }

    pub fn set_foreground(_hwnd: isize) -> bool {
        false
    }
}

pub use imp::{foreground, set_foreground};
