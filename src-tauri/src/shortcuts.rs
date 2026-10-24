// Global shortcuts for quick approval / denial and island toggle (zhoupengjie/Wakuwaku)
//
// Listens for global hotkeys via Windows RegisterHotKey in a dedicated message-pump
// thread and informs the island (island:hotkey), allowing the island's own state
// machine to decide whether to answer or expand safely.
use std::sync::{Arc, Mutex};
#[cfg(windows)]
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
#[cfg(windows)]
use std::time::Duration;

#[cfg(windows)]
use serde_json::json;
#[cfg(windows)]
use tauri::Emitter;

use crate::Shared;

const MOD_ALT: u32 = 0x0001;
const MOD_CONTROL: u32 = 0x0002;
const MOD_SHIFT: u32 = 0x0004;
const MOD_NOREPEAT: u32 = 0x4000;

static FAILED: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub fn failed() -> Vec<String> {
    FAILED.lock().unwrap().clone()
}

#[cfg(windows)]
fn set_failed(list: Vec<String>) {
    *FAILED.lock().unwrap() = list;
}

// Parses shortcut string like "Alt+Y", "Ctrl+Shift+F5".
// Requirements:
// - Must include Ctrl or Alt modifier (Shift alone is rejected).
// - Win modifier is rejected (Windows reserves Win+W, Win+N, etc.).
// - Must have a single valid main key.
pub fn parse_shortcut(s: &str) -> Option<(u32, u32)> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parts: Vec<&str> = trimmed.split('+').map(str::trim).filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return None;
    }
    let mut mods = MOD_NOREPEAT;
    let mut has_primary_mod = false;
    let mut key_part = None;

    for &p in &parts {
        if p.eq_ignore_ascii_case("alt") {
            mods |= MOD_ALT;
            has_primary_mod = true;
        } else if p.eq_ignore_ascii_case("ctrl") || p.eq_ignore_ascii_case("control") {
            mods |= MOD_CONTROL;
            has_primary_mod = true;
        } else if p.eq_ignore_ascii_case("shift") {
            mods |= MOD_SHIFT;
        } else if p.eq_ignore_ascii_case("win") || p.eq_ignore_ascii_case("meta") || p.eq_ignore_ascii_case("super") {
            // Win combinations are rejected for RegisterHotKey
            return None;
        } else if key_part.is_none() {
            key_part = Some(p);
        } else {
            // Multiple non-modifier keys is invalid
            return None;
        }
    }

    // Must contain at least Ctrl or Alt
    if !has_primary_mod {
        return None;
    }

    let key = key_part?;
    let vk = match key {
        k if k.len() == 1 => {
            let c = k.chars().next()?.to_ascii_uppercase();
            if c.is_ascii_alphanumeric() {
                c as u32
            } else {
                return None;
            }
        }
        k if k.eq_ignore_ascii_case("enter") || k.eq_ignore_ascii_case("return") => 0x0D,
        k if k.eq_ignore_ascii_case("space") => 0x20,
        k if k.eq_ignore_ascii_case("tab") => 0x09,
        k if k.eq_ignore_ascii_case("esc") || k.eq_ignore_ascii_case("escape") => 0x1B,
        k if k.len() >= 2 && (k.starts_with('f') || k.starts_with('F')) => {
            let num: u32 = k[1..].parse().ok()?;
            if (1..=12).contains(&num) {
                0x70 + num - 1
            } else {
                return None;
            }
        }
        _ => return None,
    };

    Some((mods, vk))
}

// Check duplicate shortcuts
pub fn has_duplicates(keys: &[&str]) -> bool {
    let non_empty: Vec<&str> = keys.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    for (i, &left) in non_empty.iter().enumerate() {
        for &right in &non_empty[(i + 1)..] {
            if left.eq_ignore_ascii_case(right) || parse_shortcut(left).is_some_and(|a| parse_shortcut(right) == Some(a)) {
                return true;
            }
        }
    }
    false
}

#[cfg(windows)]
mod win {
    use super::*;
    use std::ptr::null_mut;

    const WM_HOTKEY: u32 = 0x0312;
    const WM_USER_RELOAD: u32 = 0x0400 + 101;

    const HOTKEY_APPROVE: i32 = 1;
    const HOTKEY_DENY: i32 = 2;
    const HOTKEY_ISLAND: i32 = 3;

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    // As winkeys.rs declares it (GetMessageW is declared once per signature).
    #[repr(C)]
    struct Msg {
        hwnd: *mut std::ffi::c_void,
        message: u32,
        wparam: usize,
        lparam: isize,
        time: u32,
        pt: Point,
        private: u32,
    }

    extern "system" {
        fn RegisterHotKey(hwnd: *mut std::ffi::c_void, id: i32, fs_modifiers: u32, vk: u32) -> i32;
        fn UnregisterHotKey(hwnd: *mut std::ffi::c_void, id: i32) -> i32;
        fn GetMessageW(msg: *mut Msg, hwnd: *mut std::ffi::c_void, min: u32, max: u32) -> i32;
        fn PostThreadMessageW(thread_id: u32, msg: u32, wparam: usize, lparam: isize) -> i32;
        fn GetCurrentThreadId() -> u32;
        fn GetLastError() -> u32;
    }

    static THREAD_ID: AtomicU32 = AtomicU32::new(0);
    // A shortcut being recorded in the open settings: the keys are let go
    // meanwhile, so the combination pressed reaches the settings instead of
    // answering a prompt or folding the island.
    static PAUSED: AtomicBool = AtomicBool::new(false);

    pub fn pause(on: bool) {
        if PAUSED.swap(on, Ordering::SeqCst) != on {
            notify_reload();
        }
    }

    pub fn start_worker(sh: Arc<Shared>) {
        if THREAD_ID.load(Ordering::SeqCst) != 0 {
            return;
        }
        let sh_worker = sh.clone();
        crate::spawn_named("shortcuts", move || {
            let tid = unsafe { GetCurrentThreadId() };
            THREAD_ID.store(tid, Ordering::SeqCst);

            // The failed ones; None while recording (the last say stays shown).
            let register_all = |sh: &Shared| -> Option<Vec<String>> {
                unsafe {
                    UnregisterHotKey(null_mut(), HOTKEY_APPROVE);
                    UnregisterHotKey(null_mut(), HOTKEY_DENY);
                    UnregisterHotKey(null_mut(), HOTKEY_ISLAND);
                }
                // Only while the settings are open: closed, the keys are back
                // whatever their page last said.
                if PAUSED.load(Ordering::SeqCst) && sh.island.lock().unwrap().settings_open {
                    return None;
                }
                // Yield hotkeys when shortcuts disabled, DND on, or an app is fullscreen
                if !sh.flag("shortcutsEnabled") || sh.flag("dnd") || sh.is_fullscreen() {
                    return Some(Vec::new());
                }

                let approve = sh.setting("shortcutApprove").as_str().unwrap_or("Alt+Y").to_string();
                let deny = sh.setting("shortcutDeny").as_str().unwrap_or("Alt+N").to_string();
                let island = sh.setting("shortcutIsland").as_str().unwrap_or("Alt+W").to_string();

                let mut failed = Vec::new();

                let list = [
                    (HOTKEY_APPROVE, "shortcutApprove", &approve),
                    (HOTKEY_DENY, "shortcutDeny", &deny),
                    (HOTKEY_ISLAND, "shortcutIsland", &island),
                ];

                for (id, name, spec) in list {
                    if spec.trim().is_empty() {
                        continue;
                    }
                    if let Some((m, vk)) = parse_shortcut(spec) {
                        let res = unsafe { RegisterHotKey(null_mut(), id, m, vk) };
                        if res == 0 {
                            let err = unsafe { GetLastError() };
                            sh.log(&format!("shortcut {name} ({spec}) register failed: {err}"));
                            failed.push(name.to_string());
                        }
                    } else {
                        failed.push(name.to_string());
                    }
                }
                Some(failed)
            };

            let mut last_failed = register_all(&sh_worker).unwrap_or_default();
            set_failed(last_failed.clone());
            let _ = sh_worker.app.emit_to("island", "shortcuts:status", json!({ "failed": last_failed }));

            // Retry worker: in case an older running instance is still exiting during update
            if !last_failed.is_empty() {
                retry_registration();
            }

            let mut msg: Msg = unsafe { std::mem::zeroed() };
            while unsafe { GetMessageW(&mut msg, null_mut(), 0, 0) } > 0 {
                match msg.message {
                    WM_HOTKEY => {
                        let id = msg.wparam as i32;
                        let which = match id {
                            HOTKEY_APPROVE => "allow",
                            HOTKEY_DENY => "deny",
                            HOTKEY_ISLAND => "island",
                            _ => continue,
                        };
                        let _ = sh_worker.app.emit_to("island", "island:hotkey", json!({ "which": which }));
                    }
                    WM_USER_RELOAD => {
                        let Some(failed) = register_all(&sh_worker) else { continue };
                        let changed = failed != last_failed;
                        last_failed = failed.clone();
                        set_failed(failed.clone());
                        let _ = sh_worker.app.emit_to("island", "shortcuts:status", json!({ "failed": failed }));
                        if changed {
                            sh_worker.push_settings();
                        }
                    }
                    _ => {}
                }
            }
            THREAD_ID.store(0, Ordering::SeqCst);
        });
    }

    pub fn notify_reload() {
        let tid = THREAD_ID.load(Ordering::SeqCst);
        if tid != 0 {
            unsafe { PostThreadMessageW(tid, WM_USER_RELOAD, 0, 0) };
        }
    }

    // When upgrading, the new instance starts before the old exits, which
    // takes up to 8 s (Shared::quit) and longer while it waits on the
    // taskbar's handover. If registration failed initially, retry each
    // second for 30 s to reacquire hotkeys.
    pub fn retry_registration() {
        crate::spawn_named("shortcuts retry", move || {
            for _ in 0..30 {
                std::thread::sleep(Duration::from_millis(1000));
                if failed().is_empty() {
                    break;
                }
                notify_reload();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_parser_validation() {
        // Empty / whitespace
        assert_eq!(parse_shortcut(""), None);
        assert_eq!(parse_shortcut("   "), None);

        // Single key without modifier (must be rejected)
        assert_eq!(parse_shortcut("Y"), None);
        assert_eq!(parse_shortcut("Space"), None);
        assert_eq!(parse_shortcut("Enter"), None);

        // Shift only (must be rejected)
        assert_eq!(parse_shortcut("Shift+Y"), None);
        assert_eq!(parse_shortcut("Shift+F5"), None);

        // Win combinations (must be rejected for RegisterHotKey)
        assert_eq!(parse_shortcut("Win+W"), None);
        assert_eq!(parse_shortcut("Win+N"), None);
        assert_eq!(parse_shortcut("Meta+Y"), None);

        // Valid Alt and Ctrl combinations
        assert_eq!(parse_shortcut("Alt+Y"), Some((MOD_NOREPEAT | MOD_ALT, b'Y' as u32)));
        assert_eq!(parse_shortcut("alt+n"), Some((MOD_NOREPEAT | MOD_ALT, b'N' as u32)));
        assert_eq!(parse_shortcut("Ctrl+Shift+F5"), Some((MOD_NOREPEAT | MOD_CONTROL | MOD_SHIFT, 0x74)));
        assert_eq!(parse_shortcut("Alt+Space"), Some((MOD_NOREPEAT | MOD_ALT, 0x20)));
        assert_eq!(parse_shortcut("Ctrl+Enter"), Some((MOD_NOREPEAT | MOD_CONTROL, 0x0D)));
    }

    #[test]
    fn shortcut_duplicates() {
        assert!(has_duplicates(&["Alt+Y", "Alt+y"]));
        assert!(has_duplicates(&["Alt+Y", "Alt+N", "Alt+Y"]));
        assert!(has_duplicates(&["Ctrl+Y", "Control+y"]));
        assert!(!has_duplicates(&["Alt+Y", "Alt+N", "Alt+W"]));
        assert!(!has_duplicates(&["Alt+Y", "", ""]));
    }
}

pub fn start(sh: &Arc<Shared>) {
    #[cfg(windows)]
    win::start_worker(sh.clone());
    #[cfg(not(windows))]
    let _ = sh;
}

pub fn update(_sh: &Arc<Shared>) {
    #[cfg(windows)]
    win::notify_reload();
}

// The settings recording a shortcut (on), or done with it (off).
pub fn pause(on: bool) {
    #[cfg(windows)]
    win::pause(on);
    #[cfg(not(windows))]
    let _ = on;
}
