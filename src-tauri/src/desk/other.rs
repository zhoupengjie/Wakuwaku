// Elsewhere than Windows: nothing of the desktop's windows runs.
use std::sync::Arc;

use crate::Shared;

pub fn device_of(_name: &str) -> Option<String> {
    None
}
pub fn start(_sh: &Arc<Shared>) {}
pub fn ctrl_down() -> bool {
    false
}
pub fn ctrl_age() -> u32 {
    0
}
pub fn wheel_to_icons(_x: i32, _y: i32, _delta: i32) -> bool {
    false
}
pub fn keep_all_low() {}
pub fn set_through(_hwnd: isize, _through: bool) {}
pub fn desktop_shown(_own: &[isize]) -> bool {
    true
}
pub fn stop() {}
pub fn arm_stale() {}
pub fn trace_this_thread() {}
pub fn icons(_mon: super::Mon, _sf: f64) -> Option<Vec<[f64; 4]>> {
    Some(Vec::new())
}
}
