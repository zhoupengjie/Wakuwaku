// The icons as shown (ours, then those only Explorer's holds), and a press
// on one told to its program.
use super::*;

// On the tray's thread (the taskbar's sends them from there: WM_TRAY_CHANGED).
pub fn shown() -> Vec<Shown> {
    let mut list: Vec<Shown> = icons_read(|icons| {
        icons
            .iter()
            .filter(|i| !i.hidden && i.had_icon)
            .map(|i| Shown { key: i.key, icon: i.icon, tip: i.tip.clone(), exe: i.exe.clone(), uid: i.id, name: icon_name(&i.exe, i.guid, i.id), guid: i.guid, reach: !i.taken || i.callback != 0, snapshot: Vec::new(), out: None })
            .collect()
    });
    let held: Vec<Shown> = HELD.with_borrow(|held| {
        held.iter()
            .filter(|h| !list.iter().any(|s| h.is(&s.exe, s.uid, s.guid)))
            .map(|h| Shown { key: h.key, icon: 0, tip: h.tip.clone(), exe: h.exe.clone(), uid: h.uid, name: icon_name(&h.exe, h.guid, h.uid), guid: h.guid, reach: false, snapshot: h.png.clone(), out: Some(h.out) })
            .collect()
    });
    list.extend(held);
    list
}

// The spike writes it down (tray.txt); the pet only logs the icons as they come.
#[allow(dead_code)]
pub fn describe() -> String {
    icons_read(|icons| {
        icons
            .iter()
            .map(|i| format!("{} id={} v{} cb={:#x} changed ×{}{}{} \"{}\"", i.exe, i.id, i.version, i.callback, i.modified, if i.hidden { " hidden" } else { "" }, if i.icon == 0 { " no-icon" } else { "" }, i.tip))
            .collect::<Vec<_>>()
            .join("\n")
    })
}

// A press on an icon, told to its program as Explorer tells it. at: the
// pointer on the screen. From the page's command, on another thread than
// the tray's: what it needs of the icon as the tray's thread published it
// (REACH), nothing of ours waited on (her own icon's menu, opened on the
// press, held the list until it closed, every program's tray messages
// waiting, when the list was a lock).
pub fn tell(key: u64, press: Press, at: (i32, i32)) -> bool {
    let found = reach().iter().find(|i| i.key == key).cloned();
    let Some(i) = found else { return false };
    if i.callback == 0 {
        return false;
    }
    // Her own icon's window is on the thread a press comes on (a
    // command's), where a send is a call: posted, its menu opens after
    // the press is done with, not inside it.
    let hers = i.pid == std::process::id();
    let send = |event: u32| {
        let (w, l) = if i.version >= 4 {
            ((at.0 as u16 as usize) | ((at.1 as u16 as usize) << 16), ((event & 0xffff) | ((i.id & 0xffff) << 16)) as isize)
        } else {
            (i.id as usize, event as isize)
        };
        // SAFETY: the program's window and the message it asked for; no pointers.
        unsafe {
            if hers {
                PostMessageW(hwnd_of(i.hwnd), i.callback, w, l);
            } else {
                SendNotifyMessageW(hwnd_of(i.hwnd), i.callback, w, l);
            }
        }
    };
    // The program's window put in front, as the taskbar does: a menu
    // opened while its program is not in front does not close on a press
    // elsewhere, and some do not bring themselves there (WeChat's,
    // Voicemeeter's). It may come to the front after, too.
    // Not when the program has the front already: what it opened on the
    // press (QQ's menu, a window of its own) would lose it, and close.
    let allow = || {
        let mut pid = 0;
        // SAFETY: plain queries; our own out-parameter.
        unsafe { GetWindowThreadProcessId(GetForegroundWindow(), &mut pid) };
        if pid == i.pid {
            return;
        }
        // SAFETY: plain calls; they fail unless we may set the front ourselves.
        let allowed = unsafe { AllowSetForegroundWindow(i.pid) } != 0;
        let put = unsafe { SetForegroundWindow(hwnd_of(i.hwnd)) } != 0;
        if !allowed || !put {
            note(format!("tray: {} not put in front (let: {allowed}, put: {put})", i.exe));
        }
    };
    // Some open what they open on the press itself (Electron's), not on letting go.
    match press {
        Press::LeftDown => {
            allow();
            send(WM_LBUTTONDOWN);
        }
        Press::LeftUp => {
            allow();
            send(WM_LBUTTONUP);
            if i.version >= 3 {
                send(NIN_SELECT);
            }
        }
        Press::RightDown => {
            allow();
            send(WM_RBUTTONDOWN);
        }
        Press::RightUp => {
            allow();
            send(WM_RBUTTONUP);
            if i.version >= 3 {
                send(WM_CONTEXTMENU);
            }
        }
        Press::Double => {
            allow();
            send(WM_LBUTTONDBLCLK);
        }
        Press::Move => send(WM_MOUSEMOVE),
        Press::HoverIn => {
            if i.version >= 4 {
                send(NIN_POPUPOPEN);
            }
        }
        Press::HoverOut => {
            if i.version >= 4 {
                send(NIN_POPUPCLOSE);
            }
        }
    }
    true
}

