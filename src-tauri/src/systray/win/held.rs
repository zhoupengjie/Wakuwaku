// The icons only Explorer's tray holds (never handed to ours): found from
// Windows' record and Explorer's own answers (scan_held).
use super::*;

// An icon Explorer's tray holds, shown in it, that ours was never
// handed: its program (as Windows recorded it), its uID or guid, its
// window when found by its uID, and from Windows' record of it its first
// tip, its picture and whether it is kept out on the taskbar.
pub(super) struct HeldIcon {
    pub(super) key: u64,
    pub(super) exe: String,
    pub(super) uid: u32,
    pub(super) guid: Option<[u8; 16]>,
    pub(super) hwnd: u32,
    pub(super) tip: String,
    pub(super) png: Vec<u8>,
    pub(super) out: bool,
}

impl HeldIcon {
    // Whether it is this icon of ours: by its guid, else its program and
    // uID (whatever ours is named by: a record from before its program
    // named it by a guid).
    pub(super) fn is(&self, exe: &str, id: u32, guid: Option<[u8; 16]>) -> bool {
        match self.guid {
            Some(a) => guid == Some(a),
            None => self.uid == id && self.exe.eq_ignore_ascii_case(exe),
        }
    }
}

// Asked as Shell_NotifyIconGetRect asks, of Explorer's own window (not
// the first Shell_TrayWnd, which is ours): whether it holds this icon,
// shown. None: it did not answer.
pub(super) fn explorer_holds(explorer: Hwnd, hwnd: u32, id: u32, guid: [u8; 16]) -> Option<bool> {
    for message in [1, 2] {
        let q = RectQuestion { signature: SIGNATURE, message, size: 32, padding: 0, hwnd, id, guid };
        let cds = CopyData { data: 3, size: std::mem::size_of::<RectQuestion>() as u32, ptr: &q as *const RectQuestion as *const c_void };
        let mut answer = 0usize;
        // SAFETY: Explorer's window; the struct is ours for the call and copied across.
        let ok = unsafe { SendMessageTimeoutW(explorer, WM_COPYDATA, 0, &cds as *const CopyData as isize, SMTO_ABORTIFHUNG, 500, &mut answer) };
        if ok == 0 {
            return None;
        }
        match holds(answer) {
            Holds::Shown => return Some(true),
            Holds::Hidden => return Some(false),
            // A corner at 0 is one half of the answer: the other asked.
            Holds::No => {}
        }
    }
    Some(false)
}

extern "system" fn list_one(hwnd: Hwnd, list: isize) -> i32 {
    let mut pid = 0;
    // SAFETY: a window being enumerated; our own out-parameter; the Vec outlives the walk.
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut pid);
        (*(list as *mut Vec<(u32, u32)>)).push((hwnd as isize as u32, pid));
    }
    1
}

// Every window a tray icon may be named by: the top-level ones and the
// message-only ones (ACE-Tray's), each with its process.
pub(super) fn windows() -> Vec<(u32, u32)> {
    let mut list: Vec<(u32, u32)> = Vec::new();
    // SAFETY: the callback pushes onto the Vec, which outlives the call;
    // FindWindowEx walks the message-only windows one after another.
    unsafe {
        EnumWindows(list_one, &mut list as *mut Vec<(u32, u32)> as isize);
        let mut at: Hwnd = std::ptr::null_mut();
        loop {
            at = FindWindowExW(HWND_MESSAGE as Hwnd, at, std::ptr::null(), std::ptr::null());
            if at.is_null() {
                break;
            }
            let mut pid = 0;
            GetWindowThreadProcessId(at, &mut pid);
            list.push((at as isize as u32, pid));
        }
    }
    list
}

// Windows' record of every tray icon it has shown (HKCU\Control Panel\
// NotifyIconSettings, a key per icon: ExecutablePath, UID or IconGuid,
// InitialTooltip, IsPromoted, IconSnapshot, a PNG), for the programs
// that run (by file name, lower case): once per program and icon.
pub(super) fn records(running: &std::collections::HashSet<String>) -> Vec<HeldIcon> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let Ok(root) = RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Control Panel\NotifyIconSettings") else { return Vec::new() };
    let mut list: Vec<HeldIcon> = Vec::new();
    for name in root.enum_keys().flatten() {
        let Ok(key) = root.open_subkey(&name) else { continue };
        let Ok(path) = key.get_value::<String, _>("ExecutablePath") else { continue };
        let exe = file_name(&path).to_lowercase();
        // Explorer's own icons come back when asked, or are hers to draw (the volume, the network).
        if exe == "explorer.exe" || !running.contains(&exe) {
            continue;
        }
        let guid = key.get_value::<String, _>("IconGuid").ok().and_then(|g| guid_bytes(&g));
        let uid = key.get_value::<u32, _>("UID").ok();
        if guid.is_none() && uid.is_none() {
            continue;
        }
        let uid = uid.unwrap_or(0);
        // The same icon recorded again (another path to the program).
        if list.iter().any(|h| h.guid == guid && (guid.is_some() || (h.uid == uid && h.exe == exe))) {
            continue;
        }
        let png = key.get_raw_value("IconSnapshot").map(|v| v.bytes.into_owned()).unwrap_or_default();
        let tip = key.get_value::<String, _>("InitialTooltip").unwrap_or_default();
        let out = key.get_value::<u32, _>("IsPromoted").unwrap_or(0) != 0;
        list.push(HeldIcon { key: 0, exe, uid, guid, hwnd: 0, tip, png, out });
    }
    list
}

// The icons Explorer's tray holds, shown, that ours has not, of the
// records of the programs that run (but ours): each one named by a guid
// asked by it; one named by its window and uID asked of each window of
// its program until one is held (at most 400 asked in all). None:
// Explorer's did not answer (hung, or starting again).
pub(super) fn look_held(explorer: Hwnd, ours: &[(String, u32, Option<[u8; 16]>)]) -> Option<Vec<HeldIcon>> {
    let own = std::process::id();
    let procs: Vec<(u32, String)> = processes().into_iter().filter(|(pid, _)| *pid != own).map(|(pid, exe)| (pid, exe.to_lowercase())).collect();
    let running: std::collections::HashSet<String> = procs.iter().map(|(_, exe)| exe.clone()).collect();
    let mut wins: Option<Vec<(u32, u32)>> = None;
    let mut asked = 0;
    let mut found = Vec::new();
    for mut record in records(&running) {
        if record.png.is_empty() || ours.iter().any(|(exe, id, guid)| record.is(exe, *id, *guid)) {
            continue;
        }
        if let Some(guid) = record.guid {
            asked += 1;
            if explorer_holds(explorer, 0, record.uid, guid)? {
                found.push(record);
            }
            continue;
        }
        let pids: Vec<u32> = procs.iter().filter(|(_, exe)| *exe == record.exe).map(|(pid, _)| *pid).collect();
        for (hwnd, _) in wins.get_or_insert_with(windows).iter().filter(|(_, pid)| pids.contains(pid)) {
            if asked >= 400 {
                break;
            }
            asked += 1;
            if explorer_holds(explorer, *hwnd, record.uid, [0; 16])? {
                record.hwnd = *hwnd;
                found.push(record);
                break;
            }
        }
    }
    Some(found)
}

// Looked for again (some seconds after programs were asked for their
// icons, and every so often): the icons only Explorer's tray holds,
// shown in ours from Windows' record of them until their program hands
// ours one. Each keeps its key from one look to the next.
pub fn scan_held() {
    let explorer = explorers();
    if OURS.load(Ordering::SeqCst) == 0 || explorer.is_null() {
        return;
    }
    // Those ours shows (one taken from a change without a picture yet is
    // still shown from Windows').
    let ours: Vec<(String, u32, Option<[u8; 16]>)> = icons_read(|icons| icons.iter().filter(|i| !i.hidden && i.had_icon).map(|i| (i.exe.clone(), i.id, i.guid)).collect());
    // Explorer's not answering: what was found last kept until it does.
    // (Asked with nothing of ours borrowed: its answers may wait on
    // programs' messages to ours, answered here meanwhile.)
    let Some(mut found) = look_held(explorer, &ours) else { return };
    let differs = held_edit(|held| keep_found(held, &mut found));
    if differs {
        changed();
    }
}

// What was found kept, each with its key from the last look (a new one
// a new key), what is no longer there said; whether it differs.
fn keep_found(held: &mut Vec<HeldIcon>, found: &mut Vec<HeldIcon>) -> bool {
    for h in found.iter_mut() {
        match held.iter().find(|was| was.is(&h.exe, h.uid, h.guid) && was.hwnd == h.hwnd) {
            Some(was) => h.key = was.key,
            None => {
                h.key = NEXT_KEY.fetch_add(1, Ordering::SeqCst);
                let which = h.guid.map_or_else(|| format!("id {}, window {:#x}", h.uid, h.hwnd), |_| "by guid".to_string());
                note(format!("tray: Windows' taskbar holds {}'s icon ({which}) \"{}\", never handed to hers: shown from Windows' picture of it, presses cannot reach it", h.exe, h.tip));
            }
        }
    }
    for was in held.iter().filter(|was| !found.iter().any(|h| h.key == was.key)) {
        note(format!("tray: {}'s icon no longer held by Windows' taskbar alone", was.exe));
    }
    let differs = found.len() != held.len() || found.iter().zip(held.iter()).any(|(a, b)| a.key != b.key);
    *held = std::mem::take(found);
    differs
}

// The icons ours has, then those only Explorer's holds (none that ours
// has now).
