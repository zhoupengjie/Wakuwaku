// The clock's tip on her taskbar, as Windows' own taskbar has it
// (SystemTray.dll, read 2026-10-14, zhoupengjie/Wakuwaku#36): the date in
// full, in the person's own format (Region: the long date). Windows shows
// that as it is; here the weekday comes after it where it has none, as
// the issue asks (2026年10月14日 星期三; an English long date has it
// already: Wednesday, October 14, 2026). With Windows' additional clocks
// on (Date and time → Additional clocks), as Windows writes them: a blank
// line, and a line for each clock, the weekday and the time there and its
// name, this one first as the local time:
//
//   2026年10月14日 星期三
//
//   星期三 14:32 (本地时间)
//   星期三 15:32 (东京)
//
// Windows keeps the clocks under Control Panel\TimeDate\AdditionalClocks\1
// and \2: Enable (1 while shown), DisplayName, TzRegKeyName (the time
// zone's key, as Windows names it: "Tokyo Standard Time").

// A clock Windows shows: its name, and its time zone's key.
#[derive(Debug, PartialEq)]
pub struct Clock {
    pub name: String,
    pub zone: String,
}

// The clocks as Windows keeps them (each one's Enable, DisplayName and
// TzRegKeyName, as read): those shown that have a time zone.
pub fn clocks_of(kept: Vec<(Option<u32>, Option<String>, Option<String>)>) -> Vec<Clock> {
    kept.into_iter()
        .filter(|(on, _, _)| on.is_some_and(|on| on != 0))
        .filter_map(|(_, name, zone)| {
            let zone = zone?.trim().to_string();
            (!zone.is_empty()).then(|| Clock { name: name.unwrap_or_default().trim().to_string(), zone })
        })
        .collect()
}

// Whether a date's pattern says the weekday (ddd or dddd), outside the
// text it quotes ('年' and the like).
pub fn has_weekday(pattern: &str) -> bool {
    let mut quoted = false;
    let mut run = 0;
    for c in pattern.chars() {
        if c == '\'' {
            quoted = !quoted;
            run = 0;
        } else if !quoted && c == 'd' {
            run += 1;
            if run >= 3 {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

// The tip: the date, the weekday after it where it lacks one; and each
// clock's line (its weekday, time and name), as Windows writes them.
pub fn tip_text(date: &str, weekday: Option<&str>, lines: &[(String, String, String)]) -> String {
    let mut tip = match weekday.filter(|w| !w.is_empty()) {
        Some(w) => format!("{date} {w}"),
        None => date.to_string(),
    };
    if !lines.is_empty() {
        tip.push_str("\n\n");
        tip.push_str(&lines.iter().map(|(day, time, name)| format!("{day} {time} ({name})")).collect::<Vec<_>>().join("\n"));
    }
    tip
}

pub use imp::tip;

#[cfg(windows)]
mod imp {
    use crate::win32::{SystemTime, GetLocalTime, GetLocaleInfoEx};

    // DYNAMIC_TIME_ZONE_INFORMATION.
    #[repr(C)]
    struct Zone {
        bias: i32,
        standard_name: [u16; 32],
        standard_date: SystemTime,
        standard_bias: i32,
        daylight_name: [u16; 32],
        daylight_date: SystemTime,
        daylight_bias: i32,
        key: [u16; 128],
        dynamic_off: u8,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetSystemTime(time: *mut SystemTime);
        fn SystemTimeToTzSpecificLocalTimeEx(zone: *const Zone, utc: *const SystemTime, local: *mut SystemTime) -> i32;
        fn GetDateFormatEx(locale: *const u16, flags: u32, date: *const SystemTime, format: *const u16, out: *mut u16, len: i32, calendar: *const u16) -> i32;
        fn GetTimeFormatEx(locale: *const u16, flags: u32, time: *const SystemTime, format: *const u16, out: *mut u16, len: i32) -> i32;
    }

    #[link(name = "advapi32")]
    extern "system" {
        fn EnumDynamicTimeZoneInformation(index: u32, zone: *mut Zone) -> u32;
    }

    const DATE_LONGDATE: u32 = 0x2;
    const TIME_NOSECONDS: u32 = 0x2;
    const LOCALE_SLONGDATE: u32 = 0x20;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn text(u: &[u16]) -> String {
        String::from_utf16_lossy(&u[..u.iter().position(|&c| c == 0).unwrap_or(u.len())])
    }

    // What one of Windows' formatting calls writes (its length with the 0 at its end; 0: none).
    fn written(call: impl FnOnce(*mut u16, i32) -> i32) -> String {
        let mut out = [0u16; 256];
        let n = call(out.as_mut_ptr(), out.len() as i32);
        if n <= 0 { String::new() } else { text(&out[..n as usize]) }
    }

    // In the person's own format (the user's locale: null). Without the
    // reading-order marks Windows' own asks for (DATE_AUTOLAYOUT): they came
    // round every number of a Chinese date, unseen but there in the text.
    fn long_date(t: &SystemTime) -> String {
        // SAFETY: our own time and buffer, its length passed.
        written(|out, len| unsafe { GetDateFormatEx(std::ptr::null(), DATE_LONGDATE, t, std::ptr::null(), out, len, std::ptr::null()) })
    }

    fn weekday(t: &SystemTime) -> String {
        let format = wide("dddd");
        // SAFETY: as above, with our own format.
        written(|out, len| unsafe { GetDateFormatEx(std::ptr::null(), 0, t, format.as_ptr(), out, len, std::ptr::null()) })
    }

    fn time(t: &SystemTime) -> String {
        // SAFETY: as above.
        written(|out, len| unsafe { GetTimeFormatEx(std::ptr::null(), TIME_NOSECONDS, t, std::ptr::null(), out, len) })
    }

    // The time zone Windows knows by this key, as it is this year.
    fn zone(key: &str) -> Option<Zone> {
        for index in 0.. {
            // SAFETY: zeroed is a valid Zone (numbers and arrays), filled by the call.
            let mut z: Zone = unsafe { std::mem::zeroed() };
            // SAFETY: our own struct, of the size the call writes.
            if unsafe { EnumDynamicTimeZoneInformation(index, &mut z) } != 0 {
                return None;
            }
            if text(&z.key).eq_ignore_ascii_case(key) {
                return Some(z);
            }
        }
        None
    }

    fn kept() -> Vec<(Option<u32>, Option<String>, Option<String>)> {
        use winreg::enums::HKEY_CURRENT_USER;
        let Ok(root) = winreg::RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Control Panel\TimeDate\AdditionalClocks") else { return Vec::new() };
        ["1", "2"]
            .iter()
            .filter_map(|n| root.open_subkey(n).ok())
            .map(|k| (k.get_value::<u32, _>("Enable").ok(), k.get_value::<String, _>("DisplayName").ok(), k.get_value::<String, _>("TzRegKeyName").ok()))
            .collect()
    }

    // The tip now; local: the words for this clock's line ("Local time").
    pub fn tip(local: &str) -> String {
        let mut now = SystemTime::default();
        // SAFETY: fills our own struct.
        unsafe { GetLocalTime(&mut now) };
        // The long date's pattern, to tell whether it says the weekday.
        // SAFETY: our own buffer, its length passed.
        let pattern = written(|out, len| unsafe { GetLocaleInfoEx(std::ptr::null(), LOCALE_SLONGDATE, out, len) });
        let day = weekday(&now);
        let clocks = super::clocks_of(kept());
        let mut lines = Vec::new();
        if !clocks.is_empty() {
            lines.push((day.clone(), time(&now), local.to_string()));
            let mut utc = SystemTime::default();
            // SAFETY: fills our own struct.
            unsafe { GetSystemTime(&mut utc) };
            for clock in clocks {
                let Some(z) = zone(&clock.zone) else { continue };
                let mut there = SystemTime::default();
                // SAFETY: our own structs.
                if unsafe { SystemTimeToTzSpecificLocalTimeEx(&z, &utc, &mut there) } == 0 {
                    continue;
                }
                // Named as Windows names it, else by its time zone.
                let name = if clock.name.is_empty() { text(&z.standard_name) } else { clock.name };
                lines.push((weekday(&there), time(&there), name));
            }
        }
        super::tip_text(&long_date(&now), (!super::has_weekday(&pattern)).then_some(day.as_str()), &lines)
    }

    #[cfg(test)]
    mod tests {
        // Windows' own answers here: a date, a weekday, a time, and a
        // zone found by its key and turned to.
        #[test]
        fn windows_formats_and_zones() {
            let mut now = super::SystemTime::default();
            unsafe { super::GetLocalTime(&mut now) };
            assert!(!super::long_date(&now).is_empty());
            assert!(!super::weekday(&now).is_empty());
            assert!(super::time(&now).chars().any(|c| c.is_ascii_digit()));
            let z = super::zone("Tokyo Standard Time").expect("Windows knows Tokyo");
            assert_eq!(z.bias, -540);
            assert!(super::zone("No Such Standard Time").is_none());
            assert!(!super::tip("Local time").is_empty());
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn tip(_local: &str) -> String {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clocks_shown_with_a_zone() {
        let s = |v: &str| Some(v.to_string());
        let kept = vec![(Some(1), s("东京"), s("Tokyo Standard Time")), (Some(0), s("Off"), s("UTC")), (Some(1), s("  "), s("GMT Standard Time")), (Some(1), s("No zone"), s(" ")), (None, s("Never set"), s("UTC"))];
        assert_eq!(clocks_of(kept), vec![Clock { name: "东京".into(), zone: "Tokyo Standard Time".into() }, Clock { name: String::new(), zone: "GMT Standard Time".into() }]);
        assert!(clocks_of(Vec::new()).is_empty());
    }

    #[test]
    fn a_pattern_says_the_weekday_or_not() {
        assert!(!has_weekday("yyyy'年'M'月'd'日'"));
        assert!(has_weekday("yyyy'年'M'月'd'日', dddd"));
        assert!(has_weekday("dddd, MMMM d, yyyy"));
        assert!(has_weekday("ddd d MMM yyyy"));
        assert!(!has_weekday("dd MMMM yyyy"));
        // Quoted, it is words, not the weekday.
        assert!(!has_weekday("'dddd' d"));
        assert!(!has_weekday(""));
    }

    #[test]
    fn the_tip_as_windows_writes_it() {
        assert_eq!(tip_text("2026年10月14日", Some("星期三"), &[]), "2026年10月14日 星期三");
        assert_eq!(tip_text("Wednesday, October 14, 2026", None, &[]), "Wednesday, October 14, 2026");
        assert_eq!(tip_text("2026年10月14日", Some(""), &[]), "2026年10月14日");
        let lines = [("星期三".to_string(), "14:32".to_string(), "本地时间".to_string()), ("星期三".to_string(), "15:32".to_string(), "东京".to_string())];
        assert_eq!(tip_text("2026年10月14日", Some("星期三"), &lines), "2026年10月14日 星期三\n\n星期三 14:32 (本地时间)\n星期三 15:32 (东京)");
    }
}
