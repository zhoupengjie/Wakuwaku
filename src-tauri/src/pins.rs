// The programs pinned to Windows' own taskbar, in its order: read once, on
// her first start with this (main.rs), to seed hers (taskbarPinned), the
// ones she had kept after them, none twice. Never written back: Windows
// has no way for a program to pin to its taskbar, and its list is not ours
// to change. Pinned, unpinned or moved on hers after that, it is hers.
//
// The list is Explorer's (HKCU\…\Explorer\Taskband, Favorites): a 0, then
// for each pin its length (u32) and an item ID list, a 0 between them, 0xFF
// at the end (2026-10-11, Windows 11; not documented). Each list is a
// shortcut in Quick Launch\User Pinned\TaskBar (a program: its target) or
// an app of the Apps folder (a store app: its AppUserModelID), both read by
// the shell itself. One it cannot read is left out, and said in the log.

use serde_json::Value;

// The item ID lists in Explorer's Favorites, each copied out (aligned, as
// the shell reads them) and checked whole: its items' sizes within it, and
// it ends with an empty one. Stops at whatever does not look so.
pub fn favorites_lists(blob: &[u8]) -> Vec<Vec<u8>> {
    let mut lists = Vec::new();
    if blob.first() != Some(&0) {
        return lists;
    }
    let mut at = 1;
    while at + 4 <= blob.len() {
        let len = u32::from_le_bytes([blob[at], blob[at + 1], blob[at + 2], blob[at + 3]]) as usize;
        let start = at + 4;
        if len < 2 || start + len > blob.len() {
            break;
        }
        let list = &blob[start..start + len];
        if is_id_list(list) {
            lists.push(list.to_vec());
        }
        at = start + len;
        // A 0 between them; 0xFF, the end.
        match blob.get(at) {
            Some(0) => at += 1,
            _ => break,
        }
    }
    lists
}

fn is_id_list(list: &[u8]) -> bool {
    let mut at = 0;
    while at + 2 <= list.len() {
        let size = u16::from_le_bytes([list[at], list[at + 1]]) as usize;
        if size == 0 {
            return true;
        }
        if size < 2 {
            return false;
        }
        at += size;
    }
    false
}

// A pin's key, as the taskbar's buttons have theirs (taskbar.rs
// send_windows): an app by its id, a program by its file, the case aside.
pub fn key_of(pin: &Value) -> String {
    match pin["id"].as_str().filter(|s| !s.is_empty()) {
        Some(id) => id.to_lowercase(),
        None => pin["path"].as_str().unwrap_or("").to_lowercase(),
    }
}

// Windows' pins in its order, then hers that are not among them (the same
// program by its file, or the same app by its id, whatever its name),
// none twice.
pub fn merge(windows: Vec<Value>, mine: &[Value]) -> Vec<Value> {
    let mut all: Vec<Value> = Vec::new();
    for pin in windows.into_iter().chain(mine.iter().cloned()) {
        let key = key_of(&pin);
        if !key.is_empty() && !all.iter().any(|p| key_of(p) == key) {
            all.push(pin);
        }
    }
    all
}

// Once (taskbarPinsImported): hers seeded from Windows' pins, those she
// kept before after them; what was taken and what was left out, in the log.
pub fn take_from_windows(sh: &std::sync::Arc<crate::Shared>) {
    let (windows, notes) = read();
    let mine = sh.setting("taskbarPinned").as_array().cloned().unwrap_or_default();
    let all = merge(windows.clone(), &mine);
    let from_windows: Vec<String> = windows.iter().map(key_of).collect();
    let names = |pins: Vec<&Value>| pins.iter().map(|p| p["name"].as_str().unwrap_or("?").to_string()).collect::<Vec<_>>().join(", ");
    let (theirs, hers): (Vec<&Value>, Vec<&Value>) = all.iter().partition(|p| from_windows.contains(&key_of(p)));
    sh.log(&format!("taskbar: Windows' pins taken: {}; hers kept after them: {}", names(theirs), names(hers)));
    for why in notes {
        sh.log(&format!("taskbar: a pin of Windows' left out: {why}"));
    }
    sh.change(serde_json::json!({ "taskbarPinned": all, "taskbarPinsImported": true }));
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;

    use serde_json::{json, Value};

    #[repr(C)]
    struct Guid(u32, u16, u16, [u8; 8]);

    #[repr(C)]
    struct PropertyKey {
        fmtid: Guid,
        pid: u32,
    }

    const IID_SHELL_ITEM2: Guid = Guid(0x7E9F_B0D3, 0x919F, 0x4307, [0xAB, 0x2E, 0x9B, 0x18, 0x60, 0x31, 0x0C, 0x93]);
    // System.AppUserModel.ID; System.Link.TargetParsingPath.
    const PKEY_APP_ID: PropertyKey = PropertyKey { fmtid: Guid(0x9F4C_2855, 0x9F79, 0x4B39, [0xA8, 0xD0, 0xE1, 0xD4, 0x2D, 0xE1, 0xD5, 0xF3]), pid: 5 };
    const PKEY_LINK_TARGET: PropertyKey = PropertyKey { fmtid: Guid(0xB9B4_B3FC, 0x2B51, 0x4A42, [0xB5, 0xD8, 0x32, 0x41, 0x46, 0xAF, 0xCF, 0x25]), pid: 2 };
    const SIGDN_NORMALDISPLAY: u32 = 0;
    const SIGDN_FILESYSPATH: u32 = 0x8005_8000;
    // IShellItem2's methods by their place.
    const GET_DISPLAY_NAME: usize = 5;
    const GET_STRING: usize = 17;

    #[repr(C)]
    struct Obj {
        vtbl: *const usize,
    }

    #[link(name = "shell32")]
    extern "system" {
        fn SHCreateItemFromIDList(pidl: *const c_void, iid: *const Guid, out: *mut *mut c_void) -> i32;
    }

    #[link(name = "ole32")]
    extern "system" {
        fn CoTaskMemFree(p: *mut c_void);
    }

    // An object's method by its place, as a function of the shape asked.
    // SAFETY (callers): the object is alive and has that method there.
    unsafe fn method<F: Copy>(obj: *mut Obj, at: usize) -> F {
        std::mem::transmute_copy(&*(*obj).vtbl.add(at))
    }

    // A string the shell handed over, read and freed.
    unsafe fn take(p: *mut u16) -> String {
        if p.is_null() {
            return String::new();
        }
        let n = (0..).take_while(|&i| *p.add(i) != 0).count();
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, n));
        CoTaskMemFree(p as *mut c_void);
        s
    }

    unsafe fn display(item: *mut Obj, how: u32) -> String {
        let mut name: *mut u16 = std::ptr::null_mut();
        let f: extern "system" fn(*mut Obj, u32, *mut *mut u16) -> i32 = method(item, GET_DISPLAY_NAME);
        if f(item, how, &mut name) < 0 {
            return String::new();
        }
        take(name)
    }

    unsafe fn string(item: *mut Obj, key: &PropertyKey) -> String {
        let mut value: *mut u16 = std::ptr::null_mut();
        let f: extern "system" fn(*mut Obj, *const PropertyKey, *mut *mut u16) -> i32 = method(item, GET_STRING);
        if f(item, key, &mut value) < 0 {
            return String::new();
        }
        take(value)
    }

    // One pin: a shortcut's program ({ path, name, lnk }; one that starts a
    // site's app through a browser's proxy, by its app id too), or a store
    // app ({ path: "", id, name }). Err: why it is left out.
    fn pin_of(list: &[u8]) -> Result<Value, String> {
        // SAFETY: a list checked whole (favorites_lists), alive for the call;
        // the item ours, released.
        unsafe {
            let mut item: *mut Obj = std::ptr::null_mut();
            let hr = SHCreateItemFromIDList(list.as_ptr() as *const c_void, &IID_SHELL_ITEM2, &mut item as *mut *mut Obj as *mut *mut c_void);
            if hr < 0 || item.is_null() {
                return Err(format!("an item the shell could not read ({:#010x})", hr as u32));
            }
            let name = display(item, SIGDN_NORMALDISPLAY);
            let file = display(item, SIGDN_FILESYSPATH);
            let id = string(item, &PKEY_APP_ID);
            let target = string(item, &PKEY_LINK_TARGET);
            let release: extern "system" fn(*mut Obj) -> u32 = method(item, 2);
            release(item);
            if file.to_ascii_lowercase().ends_with(".lnk") {
                // File Explorer's leads to a place in the shell, not a file:
                // its windows are explorer.exe's, so its button is too.
                if id.eq_ignore_ascii_case("Microsoft.Windows.Explorer") {
                    let windows = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
                    return Ok(json!({ "path": format!("{windows}\\explorer.exe"), "name": name, "lnk": file }));
                }
                if target.is_empty() || !std::path::Path::new(&target).exists() {
                    // Another app's own (its windows say the id): by it.
                    if !id.is_empty() {
                        return Ok(json!({ "path": "", "id": id, "name": name, "lnk": file }));
                    }
                    return Err(format!("{name}: its shortcut leads nowhere ({target:?})"));
                }
                let proxy = target.to_ascii_lowercase().ends_with("_proxy.exe");
                return Ok(if proxy && !id.is_empty() {
                    json!({ "path": target, "id": id, "name": name, "lnk": file })
                } else {
                    json!({ "path": target, "name": name, "lnk": file })
                });
            }
            if !id.is_empty() {
                return Ok(json!({ "path": "", "id": id, "name": name }));
            }
            Err(format!("{name:?}: neither a shortcut nor an app"))
        }
    }

    // Windows' pins in its order, and what was left out (or why none).
    pub fn read() -> (Vec<Value>, Vec<String>) {
        use winreg::enums::HKEY_CURRENT_USER;
        let key = winreg::RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Taskband");
        let blob = match key.and_then(|k| k.get_raw_value("Favorites")) {
            Ok(v) => v.bytes,
            Err(e) => return (Vec::new(), vec![format!("no list of Windows' pins ({e})")]),
        };
        crate::tasks::with_com();
        let mut pins = Vec::new();
        let mut notes = Vec::new();
        for list in super::favorites_lists(&blob) {
            match pin_of(&list) {
                Ok(pin) => pins.push(pin),
                Err(why) => notes.push(why),
            }
        }
        (pins, notes)
    }
}

#[cfg(not(windows))]
mod imp {
    use serde_json::Value;

    pub fn read() -> (Vec<Value>, Vec<String>) {
        (Vec::new(), vec!["no Windows taskbar here".into()])
    }
}

pub use imp::read;

#[cfg(test)]
mod tests {
    use serde_json::json;

    // A list of one item of `n` bytes (its size first), then the empty one.
    fn list(n: u16) -> Vec<u8> {
        let mut l = n.to_le_bytes().to_vec();
        l.extend(std::iter::repeat(7).take(n as usize - 2));
        l.extend([0, 0]);
        l
    }

    fn entry(l: &[u8]) -> Vec<u8> {
        let mut e = (l.len() as u32).to_le_bytes().to_vec();
        e.extend(l);
        e
    }

    #[test]
    fn lists_read_out_of_the_favorites() {
        let (a, b) = (list(6), list(9));
        let mut blob = vec![0];
        blob.extend(entry(&a));
        blob.push(0);
        blob.extend(entry(&b));
        blob.push(0xff);
        assert_eq!(super::favorites_lists(&blob), vec![a.clone(), b]);
        // Cut short, or not one at all: what came whole before it.
        let mut short = vec![0];
        short.extend(entry(&a));
        short.push(0);
        short.extend([50, 0, 0, 0, 1, 2]);
        assert_eq!(super::favorites_lists(&short), vec![a]);
        assert!(super::favorites_lists(&[1, 2, 3]).is_empty());
        // A list whose item runs past its end: left out.
        let mut bad = vec![0];
        bad.extend(entry(&[9, 0, 1, 2]));
        bad.push(0xff);
        assert!(super::favorites_lists(&bad).is_empty());
    }

    #[test]
    fn hers_after_windows_none_twice() {
        let windows = vec![
            json!({ "path": "C:\\Tools\\Everything.exe", "name": "Everything" }),
            json!({ "path": "C:\\Windows\\explorer.exe", "name": "File Explorer" }),
            json!({ "path": "", "id": "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App", "name": "Calculator" }),
        ];
        let mine = vec![
            json!({ "path": "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe", "name": "Chrome" }),
            json!({ "path": "C:\\WINDOWS\\explorer.exe", "name": "Windows 资源管理器" }),
            json!({ "path": "C:\\Program Files\\WindowsApps\\Calc\\Calculator.exe", "id": "microsoft.windowscalculator_8wekyb3d8bbwe!app", "name": "计算器" }),
        ];
        let names: Vec<String> = super::merge(windows, &mine).iter().map(|p| p["name"].as_str().unwrap().to_string()).collect();
        assert_eq!(names, ["Everything", "File Explorer", "Calculator", "Chrome"]);
    }

    // Reads this machine's pins, changing nothing: run by hand
    // (cargo test pins -- --ignored --nocapture).
    #[test]
    #[ignore]
    fn reads_windows_pins() {
        let (pins, notes) = super::read();
        for p in &pins {
            println!("pin: {p}");
        }
        for n in &notes {
            println!("left out: {n}");
        }
    }
}
