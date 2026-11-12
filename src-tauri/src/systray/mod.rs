// Other programs' icons in the notification area (the tray), taken over from
// Explorer while hers is the taskbar. A program hands its icon to the first
// window of class Shell_TrayWnd that FindWindow finds (Shell_NotifyIcon, as
// a WM_COPYDATA), so one of ours is put ahead of Explorer's (topmost, and put
// back there when Explorer's moves up). Each message is handed on to
// Explorer's as well, so its tray stays whole for when the taskbar is given
// back; the same for what else comes to the taskbar's window (app bars,
// SHAppBarMessage, come this way too). Programs that added their icons before
// are asked to add them again, with the message Explorer sends when it starts.
// Given back (or she was killed holding it), every window is told so again
// once ours is gone and Explorer's shows (handed_back): whatever took ours
// for the taskbar on hearing it finds Explorer's (zhoupengjie/Wakuwaku#54).
//
// A press on an icon is told to its program as Explorer tells it, by the
// version of the protocol it asked for (NIM_SETVERSION). Windows only.
//
// Some programs never hand theirs again (zhoupengjie/Wakuwaku#52): the
// NVIDIA App's (NvUI.dll in nvcontainer.exe, run as SYSTEM in the person's
// session) adds it once, when a timer first finds a Shell_TrayWnd, and has
// no TaskbarCreated at all, which could not reach a SYSTEM window from hers
// anyway; ACE-Tray's window is message-only and has none either; Windows
// Security's answers TaskbarCreated with a NIM_MODIFY, for it had added its
// icon already (to Explorer's). Explorer's tray, handed everything, holds
// them, and Explorer itself loses them the same way when it restarts. So an
// icon changed that ours does not have is taken from the change when
// Explorer's says it holds it, and an icon Explorer's holds that never came
// at all is shown from Windows' own record of it (held, scan_held), until
// its program speaks to ours. Neither can be pressed unless the program
// named its callback message to ours: only an add carries it.
//
// The window has to be made on a thread that runs a message loop: start()
// makes it on the caller's; what changes is posted to a window of the
// caller's (notify).

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Press {
    LeftDown,
    LeftUp,
    RightDown,
    RightUp,
    Double,
    Move,
    HoverIn,
    HoverOut,
}

// An icon as shown: its key (for tell), its icon (a copy of ours; 0 while
// it blinks dark), its tip, the program it is from.
pub struct Shown {
    pub key: u64,
    pub icon: isize,
    pub tip: String,
    pub exe: String,
    // Its id among its program's icons (uID), and a name for it that stays
    // from one run to the next (the program and its guid or id), for where
    // the person keeps it (in the taskbar, or folded away).
    pub uid: u32,
    pub name: String,
    // Its guid, when its program names it by one (where Windows keeps it).
    pub guid: Option<[u8; 16]>,
    // Whether a press on it reaches its program: not when the program never
    // told ours which message to send (an icon taken from a change, or one
    // only Explorer's holds).
    pub reach: bool,
    // Windows' own picture of it (a PNG, from its record), for an icon ours
    // was never handed; empty otherwise.
    pub snapshot: Vec<u8>,
    // Kept out on Windows' taskbar, as its record says, for such an icon.
    pub out: Option<bool>,
}

// What shell32 hands the tray with an add (seen 2026-10-15 on 26300): 520
// bytes after the 964 of SHELLTRAYDATA, the program's path (as Windows keeps
// it in NotifyIconSettings: a known folder's guid for its start); Explorer
// refuses an add without it. Empty in the other messages.
#[cfg_attr(not(windows), allow(dead_code))]
const PATH_AT: usize = 964;

#[cfg_attr(not(windows), allow(dead_code))]
fn path_in(copied: &[u8]) -> Option<String> {
    let rest = copied.get(PATH_AT..)?;
    let units: Vec<u16> = rest.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take(260).take_while(|&u| u != 0).collect();
    (!units.is_empty()).then(|| String::from_utf16_lossy(&units))
}

// A program's file name from its path, either kind of slash.
#[cfg_attr(not(windows), allow(dead_code))]
fn file_name(path: &str) -> String {
    path.rsplit(['\\', '/']).next().unwrap_or(path).to_string()
}

// A guid as Windows writes it ({7120A42D-CB02-4A51-B3B5-58E36E8B14E1}),
// as its 16 bytes are in memory (the first three parts little-endian).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn guid_bytes(text: &str) -> Option<[u8; 16]> {
    let hex: String = text.trim().trim_start_matches('{').trim_end_matches('}').split('-').collect();
    if hex.len() != 32 {
        return None;
    }
    let mut raw = [0u8; 16];
    for (i, byte) in raw.iter_mut().enumerate() {
        *byte = u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    let mut out = raw;
    out[..4].copy_from_slice(&[raw[3], raw[2], raw[1], raw[0]]);
    out[4..6].copy_from_slice(&[raw[5], raw[4]]);
    out[6..8].copy_from_slice(&[raw[7], raw[6]]);
    Some(out)
}

// An icon's name that stays from one run to the next: its program, and its
// guid or its id.
#[cfg_attr(not(windows), allow(dead_code))]
fn icon_name(exe: &str, guid: Option<[u8; 16]>, id: u32) -> String {
    let which = guid.map_or_else(|| id.to_string(), |g| g.iter().map(|b| format!("{b:02x}")).collect());
    format!("{}:{which}", exe.to_lowercase())
}

// What Explorer's tray answers when asked where an icon is (message 3): 0
// for one it does not hold, all ones for one it holds hidden (NIS_HIDDEN),
// else a corner of where it is (in its folded ones: where ^ is).
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(PartialEq, Debug)]
enum Holds {
    No,
    Hidden,
    Shown,
}

#[cfg_attr(not(windows), allow(dead_code))]
fn holds(answer: usize) -> Holds {
    match answer as u32 {
        0 => Holds::No,
        u32::MAX => Holds::Hidden,
        _ => Holds::Shown,
    }
}

// Explorer's own battery icon (uID 1225, named by a guid, its tip 电池指示器,
// Battery Meter), which Explorer adds to its tray itself on 22621 (Windows
// 11 22H2 and 23H2). On explorer.exe 10.0.22621.2792 (zhoupengjie/Wakuwaku#62)
// Explorer failed fast (0xc0000409 in ucrtbase.dll) seconds after ours took
// it, and started again, its taskbar shown and put away again: the screen
// flickered every three seconds. The reporter's build that left Explorer's
// own icons to its tray (every message handed on, its answer given back)
// ran without a crash. On 26100 its battery is no tray icon at all (its
// taskbar draws it), and its other icons (1226, safe removal; the
// Bluetooth one; a headset's battery; a microphone in use) pass through
// ours without harm, so this one alone is left to it. Hers draws the
// battery itself (widgets.rs battery).
#[cfg_attr(not(windows), allow(dead_code))]
const EXPLORERS_BATTERY: u32 = 1225;

// Whether an icon is Explorer's battery: its uID, from a window of a
// process by that name (exe: as exe_of names it).
#[cfg_attr(not(windows), allow(dead_code))]
fn explorers_battery(id: u32, exe: &str) -> bool {
    id == EXPLORERS_BATTERY && exe.eq_ignore_ascii_case("explorer.exe")
}

// The window tray-icon keeps her own tray icon on (tauri's tray).
#[cfg_attr(not(windows), allow(dead_code))]
const HER_TRAY_ICON: &str = "tray_icon_app";

// Whether a window is told TaskbarCreated, as programs are asked for their
// icons: not Explorer's taskbars, which show themselves; of hers only her
// tray icon's, her taskbar's own window hearing it as Explorer started again.
// Hers had been left out (2026-10-10), and her icon, added while Explorer's
// tray was first (she starts before her taskbar is up), never came to hers
// but from a change, which names no callback: a press on it did nothing.
#[cfg_attr(not(windows), allow(dead_code))]
fn asked(own: bool, class: &str) -> bool {
    class != "Shell_TrayWnd" && class != "Shell_SecondaryTrayWnd" && (!own || class == HER_TRAY_ICON)
}

// A message of Explorer's battery handed on to its tray, for the log
// (zhoupengjie/Wakuwaku#85: on 22621 Explorer failed fast a moment after
// one went by ours while it was starting, its icon handed on as #62 has it;
// whether its tray took it then, and how fast, is what this says): which
// message, the Explorer and how long it had run (s), what its tray answered
// (None: not at all; error, GetLastError's then), and how long it took.
#[cfg_attr(not(windows), allow(dead_code))]
fn battery_said(kind: &str, pid: u32, up: Option<f64>, tray: bool, answer: Option<usize>, error: u32, ms: u128) -> String {
    let up = up.map_or_else(|| "gone".to_string(), |s| format!("up {s:.1} s"));
    let how = match answer {
        _ if !tray => "no tray of Explorer's to hand it to".to_string(),
        Some(a) => format!("answered {a}"),
        None => format!("not answered (error {error})"),
    };
    format!("tray: Explorer's battery {kind} (pid {pid}, {up}) handed on: {how} in {ms} ms")
}

// A tray message's name, as the log names it.
#[cfg_attr(not(windows), allow(dead_code))]
fn nim_name(message: u32) -> &'static str {
    match message {
        0 => "NIM_ADD",
        1 => "NIM_MODIFY",
        2 => "NIM_DELETE",
        4 => "NIM_SETVERSION",
        _ => "NIM_other",
    }
}

#[cfg(windows)]
#[path = "win/mod.rs"]
mod imp;

#[cfg(not(windows))]
#[path = "other.rs"]
mod imp;

#[allow(unused_imports)]
pub use imp::{aside, ask_again, describe, explorer_first, first, handed_back, holders, keep_first, notes, place, scan_held, seen, set_aside, set_rects, shown, start, stop, sweep, tell};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_add_carries_its_programs_path_after_the_icon() {
        // As shell32 sent it on 26300: SHELLTRAYDATA, then 520 bytes, the path.
        let path = r"{6D809377-6AF0-444B-8957-A3773F02200E}\PowerShell\7\pwsh.exe";
        let mut copied = vec![0u8; PATH_AT + 520];
        for (i, u) in path.encode_utf16().enumerate() {
            copied[PATH_AT + 2 * i..PATH_AT + 2 * i + 2].copy_from_slice(&u.to_le_bytes());
        }
        assert_eq!(path_in(&copied).as_deref(), Some(path));
        assert_eq!(file_name(&path_in(&copied).unwrap()), "pwsh.exe");
        // The other messages carry it empty; older senders not at all.
        assert_eq!(path_in(&vec![0u8; PATH_AT + 520]), None);
        assert_eq!(path_in(&vec![0u8; PATH_AT]), None);
        assert_eq!(path_in(&vec![0u8; 152 + 8]), None);
    }

    #[test]
    fn a_guid_as_windows_writes_it_is_its_bytes_in_memory() {
        let g = guid_bytes("{7120A42D-CB02-4A51-B3B5-58E36E8B14E1}").unwrap();
        assert_eq!(g, [0x2d, 0xa4, 0x20, 0x71, 0x02, 0xcb, 0x51, 0x4a, 0xb3, 0xb5, 0x58, 0xe3, 0x6e, 0x8b, 0x14, 0xe1]);
        assert_eq!(guid_bytes("7120a42d-cb02-4a51-b3b5-58e36e8b14e1"), Some(g));
        assert_eq!(guid_bytes("{7120A42D-CB02}"), None);
        assert_eq!(guid_bytes("{7120A42D-CB02-4A51-B3B5-58E36E8B14EZ}"), None);
        // Named as the icons handed to ours are, so where it is kept carries over.
        assert_eq!(icon_name("NVContainer.exe", Some(g), 0), "nvcontainer.exe:2da4207102cb514ab3b558e36e8b14e1");
        assert_eq!(icon_name("ACE-Tray.exe", None, 10001), "ace-tray.exe:10001");
    }

    #[test]
    fn explorers_answer_says_whether_it_holds_an_icon_shown() {
        // Seen 2026-10-15: none; one hidden (NIS_HIDDEN); one folded away
        // (where ^ is, 2159,1380).
        assert_eq!(holds(0), Holds::No);
        assert_eq!(holds(-1isize as usize), Holds::Hidden);
        assert_eq!(holds(0xFFFF_FFFF), Holds::Hidden);
        assert_eq!(holds(2159 | (1380 << 16)), Holds::Shown);
        assert_eq!(file_name("C:/x/y.exe"), "y.exe");
    }

    #[test]
    fn explorers_battery_alone_is_left_to_explorer() {
        // As #62's log has it: explorer.exe, id 1225, by guid.
        assert!(explorers_battery(1225, "explorer.exe"));
        assert!(explorers_battery(1225, "Explorer.EXE"));
        // Its other icons, which pass through ours on 26100 without harm
        // (safe removal, a headset's battery, a microphone in use, Bluetooth).
        for id in [1226, 100, 101, 0] {
            assert!(!explorers_battery(id, "explorer.exe"), "{id}");
        }
        // Another program's icon of that uID, or one whose process is gone.
        assert!(!explorers_battery(1225, "QQ.exe"));
        assert!(!explorers_battery(1225, "pid 0"));
    }

    #[test]
    fn programs_are_asked_for_their_icons_and_her_own_tray_icon_too() {
        assert!(asked(false, "WeChatMainWndForPC"));
        assert!(asked(false, HER_TRAY_ICON));
        // Hers: her tray icon's window alone; not her taskbar's, her pages'.
        assert!(asked(true, HER_TRAY_ICON));
        assert!(!asked(true, "WakuwakuTaskbarHost"));
        assert!(!asked(true, "Chrome_WidgetWin_1"));
        // Explorer's taskbars and hers.
        for own in [false, true] {
            assert!(!asked(own, "Shell_TrayWnd"));
            assert!(!asked(own, "Shell_SecondaryTrayWnd"));
        }
    }

    #[test]
    fn the_batterys_hand_on_says_what_explorers_tray_answered() {
        assert_eq!(nim_name(0), "NIM_ADD");
        assert_eq!(nim_name(4), "NIM_SETVERSION");
        assert_eq!(nim_name(9), "NIM_other");
        assert_eq!(
            battery_said("NIM_ADD", 4242, Some(6.27), true, Some(1), 0, 3),
            "tray: Explorer's battery NIM_ADD (pid 4242, up 6.3 s) handed on: answered 1 in 3 ms"
        );
        // Refused; timed out (1460, ERROR_TIMEOUT); no tray of Explorer's.
        assert!(battery_said("NIM_MODIFY", 1, None, true, Some(0), 0, 2).ends_with("(pid 1, gone) handed on: answered 0 in 2 ms"));
        assert!(battery_said("NIM_ADD", 1, Some(1.0), true, None, 1460, 3000).ends_with("not answered (error 1460) in 3000 ms"));
        assert!(battery_said("NIM_ADD", 1, Some(1.0), false, None, 0, 0).ends_with("no tray of Explorer's to hand it to in 0 ms"));
    }
}
