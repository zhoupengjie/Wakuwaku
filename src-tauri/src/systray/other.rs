// Elsewhere than Windows: no tray of hers.
use super::{Press, Shown};

pub fn start(_rect: (i32, i32, i32, i32), _notify: isize, _notify_msg: u32) -> bool {
    false
}
pub fn stop() {}
pub fn handed_back() -> bool {
    false
}
pub fn place(_rect: (i32, i32, i32, i32)) {}
pub fn holders() -> Vec<u32> {
    Vec::new()
}
pub fn first() -> bool {
    false
}
pub fn keep_first() -> bool {
    false
}
pub fn set_aside(_aside: bool) {}
pub fn aside() -> bool {
    false
}
pub fn explorer_first<R>(f: impl FnOnce() -> R) -> (R, bool) {
    (f(), false)
}
pub fn ask_again() {}
pub fn sweep() {}
pub fn scan_held() {}
pub fn shown() -> Vec<Shown> {
    Vec::new()
}
pub fn describe() -> String {
    String::new()
}
pub fn notes() -> Vec<String> {
    Vec::new()
}
pub fn seen() -> String {
    String::new()
}
pub fn tell(_key: u64, _press: Press, _at: (i32, i32)) -> bool {
    false
}
pub fn set_rects(_rects: Vec<(u64, (i32, i32, i32, i32))>) {}
}
