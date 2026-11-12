// An icon's messages (Shell_NotifyIcon's NIM_ADD, NIM_MODIFY, NIM_DELETE,
// NIM_SETVERSION) taken in or handed on to Explorer's tray, and where an
// icon is (Shell_NotifyIconGetRect).
use super::*;

pub(super) fn changed() {
    let to = NOTIFY.load(Ordering::SeqCst);
    if to != 0 {
        // SAFETY: the host's window; a posted message, no pointers.
        unsafe { PostMessageW(to as Hwnd, NOTIFY_MSG.load(Ordering::SeqCst), 0, 0) };
    }
}

// Handed on to Explorer's window as it came; its answer, or None.
pub(super) fn hand_on(msg: u32, wparam: usize, lparam: isize) -> Option<usize> {
    let explorer = explorers();
    if explorer.is_null() {
        return None;
    }
    let mut result = 0usize;
    // SAFETY: Explorer's window; a WM_COPYDATA's struct is ours for the call and is copied across.
    let ok = unsafe { SendMessageTimeoutW(explorer, msg, wparam, lparam, SMTO_ABORTIFHUNG, 3000, &mut result) };
    (ok != 0).then_some(result)
}

// Explorer's process, when an icon is its battery (EXPLORERS_BATTERY):
// the window's process looked at for that uID alone.
pub(super) fn explorers_battery_of(nid: &IconData) -> Option<u32> {
    if nid.id != EXPLORERS_BATTERY || nid.hwnd == 0 {
        return None;
    }
    let mut pid = 0;
    // SAFETY: our own out-parameter; a gone window leaves it 0.
    unsafe { GetWindowThreadProcessId(hwnd_of(nid.hwnd), &mut pid) };
    (pid != 0 && explorers_battery(nid.id, &exe_of(pid))).then_some(pid)
}

pub(super) fn same(icon: &Icon, nid: &IconData) -> bool {
    match (icon.guid, nid.flags & NIF_GUID != 0 && nid.guid != [0; 16]) {
        (Some(g), true) => g == nid.guid,
        _ => icon.hwnd == nid.hwnd && icon.id == nid.id,
    }
}

pub(super) fn update(icon: &mut Icon, nid: &IconData) {
    if nid.flags & NIF_MESSAGE != 0 {
        icon.callback = nid.callback;
    }
    if nid.flags & NIF_ICON != 0 {
        // SAFETY: the program's icon, copied: it may destroy its own after this.
        // None at all is an icon too: a blink's dark half (WeChat's).
        let copy = if nid.icon == 0 { 0 } else { unsafe { CopyIcon(nid.icon as i32 as isize) } };
        if copy != 0 || nid.icon == 0 {
            if icon.icon != 0 {
                // SAFETY: our own copy, replaced.
                unsafe { DestroyIcon(icon.icon) };
            }
            icon.icon = copy;
            icon.had_icon |= copy != 0;
        }
    }
    if nid.flags & NIF_TIP != 0 {
        icon.tip = text(&nid.tip);
    }
    if nid.flags & NIF_STATE != 0 && nid.state_mask & NIS_HIDDEN != 0 {
        icon.hidden = nid.state & NIS_HIDDEN != 0;
    }
}

// A program's tip noted in the log at most every ten seconds: Task
// Manager's changes every second.
pub(super) fn tip_note_due(exe: &str) -> bool {
    static NOTED: Lock<Option<std::collections::HashMap<String, std::time::Instant>>> = Lock::ranked("systray::NOTED", crate::lock::LEAF, None);
    let mut noted = NOTED.lock().unwrap();
    let noted = noted.get_or_insert_with(std::collections::HashMap::new);
    let now = std::time::Instant::now();
    match noted.get(exe) {
        Some(at) if now.duration_since(*at) < std::time::Duration::from_secs(10) => false,
        _ => {
            noted.insert(exe.to_string(), now);
            true
        }
    }
}

// An icon new to ours, from an add (path: its program's, as the add
// carries it) or a change.
pub(super) fn new_icon(nid: &IconData, path: Option<&str>) -> Icon {
    let mut pid = 0;
    // SAFETY: our own out-parameter; a gone window leaves it 0.
    unsafe { GetWindowThreadProcessId(hwnd_of(nid.hwnd), &mut pid) };
    let guid = (nid.flags & NIF_GUID != 0 && nid.guid != [0; 16]).then_some(nid.guid);
    let exe = path.map(file_name).filter(|e| !e.is_empty()).unwrap_or_else(|| exe_of(pid));
    let mut icon = Icon { key: NEXT_KEY.fetch_add(1, Ordering::SeqCst), hwnd: nid.hwnd, id: nid.id, guid, callback: 0, icon: 0, tip: String::new(), version: 0, hidden: false, pid, exe, modified: 0, had_icon: false, taken: false };
    update(&mut icon, nid);
    // One shown from Windows' record until now: this one in its place,
    // once it has a picture.
    if icon.had_icon {
        held_edit(|held| held.retain(|h| !h.is(&icon.exe, icon.id, icon.guid)));
    }
    icon
}

// A NIM_ message: whether ours took it. held: Explorer's tray answered
// a change of it yes (it holds the icon).
pub(super) fn on_icon(message: u32, nid: &IconData, path: Option<&str>, held: bool) -> bool {
    icons_edit(|icons| on_icon_in(icons, message, nid, path, held))
}

fn on_icon_in(icons: &mut Vec<Icon>, message: u32, nid: &IconData, path: Option<&str>, held: bool) -> bool {
    let at = icons.iter().position(|i| same(i, nid));
    match (message, at) {
        (NIM_ADD | NIM_MODIFY, Some(i)) => {
            let tip = icons[i].tip.clone();
            update(&mut icons[i], nid);
            icons[i].modified += 1;
            if icons[i].tip != tip && tip_note_due(&icons[i].exe) {
                note(format!("tray: {} tip \"{}\"", icons[i].exe, icons[i].tip));
            }
            true
        }
        (NIM_ADD, None) => {
            let icon = new_icon(nid, path);
            note(format!("tray: added {} (id {}, {}) \"{}\"{}", icon.exe, icon.id, if icon.guid.is_some() { "by guid" } else { "by window" }, icon.tip, if icon.hidden { ", hidden" } else { "" }));
            icons.push(icon);
            true
        }
        // A change of an icon ours never had, which Explorer's holds: its
        // add went there (before ours was up, or while Explorer's came
        // first), and its program does not add it again when asked (or
        // answers with this change, as Windows Security's does). Taken
        // from the change; a press reaches it only if the change names
        // its callback message.
        (NIM_MODIFY, None) if held && nid.hwnd != 0 => {
            let mut icon = new_icon(nid, None);
            icon.taken = true;
            note(format!("tray: took {}'s icon (id {}, {}) \"{}\" from a change: its add went to Windows' taskbar{}", icon.exe, icon.id, if icon.guid.is_some() { "by guid" } else { "by window" }, icon.tip, if icon.callback == 0 { "; no callback named, presses cannot reach it" } else { "" }));
            icons.push(icon);
            true
        }
        (NIM_DELETE, Some(i)) => {
            let icon = icons.remove(i);
            note(format!("tray: removed {} \"{}\"", icon.exe, icon.tip));
            if icon.icon != 0 {
                // SAFETY: our own copy.
                unsafe { DestroyIcon(icon.icon) };
            }
            true
        }
        // One shown from Windows' record, removed by its program: gone
        // now, not at the next look.
        (NIM_DELETE, None) => {
            let guid = (nid.flags & NIF_GUID != 0 && nid.guid != [0; 16]).then_some(nid.guid);
            held_edit(|held| {
                let before = held.len();
                held.retain(|h| match guid {
                    Some(g) => h.guid != Some(g),
                    None => h.guid.is_some() || h.hwnd != nid.hwnd || h.uid != nid.id,
                });
                held.len() != before
            })
        }
        (NIM_SETVERSION, Some(i)) => {
            icons[i].version = nid.version;
            note(format!("tray: {} speaks version {}", icons[i].exe, nid.version));
            true
        }
        _ => false,
    }
}

// Where an icon is, as Shell_NotifyIconGetRect asks it: half the rect,
// packed as two 16-bit numbers. None for an icon not shown here.
pub(super) fn answer_rect(cds: &CopyData) -> Option<isize> {
    let n = (cds.size as usize).min(std::mem::size_of::<RectQuestion>());
    if cds.ptr.is_null() || n < 24 {
        return None;
    }
    if RAW_NOTED.fetch_add(1, Ordering::SeqCst) < 2 {
        // SAFETY: n bytes of the sender's struct.
        let bytes = unsafe { std::slice::from_raw_parts(cds.ptr as *const u8, n) };
        note(format!("tray: where-is question, {} bytes: {}", cds.size, bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")));
    }
    let mut q: RectQuestion = unsafe { std::mem::zeroed() };
    // SAFETY: n bytes into ours of at least that size.
    unsafe { std::ptr::copy_nonoverlapping(cds.ptr as *const u8, &mut q as *mut RectQuestion as *mut u8, n) };
    if q.signature != SIGNATURE {
        return None;
    }
    let by_guid = q.guid != [0; 16];
    let key = icons_read(|icons| icons.iter().find(|i| if by_guid { i.guid == Some(q.guid) } else { i.hwnd == q.hwnd && i.id == q.id }).map(|i| i.key));
    let Some(key) = key else {
        if RAW_NOTED.fetch_add(1, Ordering::SeqCst) < 6 {
            note(format!("tray: where-is for an icon not here: window {:#x} id {}", q.hwnd, q.id));
        }
        return None;
    };
    let (l, t, r, b) = RECTS.with_borrow(|rects| rects.iter().find(|(k, _)| *k == key).map(|(_, rect)| *rect))?;
    let (x, y) = if q.message == 2 { (r, b) } else { (l, t) };
    Some(((x as u16 as u32) | ((y as u16 as u32) << 16)) as i32 as isize)
}

