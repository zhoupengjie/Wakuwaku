// The displays' work areas (and the whole of each), in physical pixels, read now and then (a window
// getter waits on the main thread, so not on every move).
use tauri::{AppHandle, Monitor};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub sf: f64,
    // The whole display: x, y, width, height (the top bar takes its strip from it).
    pub mon: (i32, i32, i32, i32),
}

#[derive(Clone, Default)]
pub struct Screens {
    pub areas: Vec<Area>,
    pub primary: Option<Area>,
    // Each display's device name as Windows gives it (\\.\DISPLAY1; none on
    // some systems) and its key (desk/ key_for: its monitor's device, the
    // same through replugging and renumbering), which the setting
    // homeDisplay names her display by (zhoupengjie/Wakuwaku#104).
    pub names: Vec<((i32, i32, i32, i32), Option<String>, String)>,
}

fn area(m: &Monitor) -> Area {
    let wa = m.work_area();
    let (p, s) = (m.position(), m.size());
    Area { x: wa.position.x, y: wa.position.y, w: wa.size.width as i32, h: wa.size.height as i32, sf: m.scale_factor(), mon: (p.x, p.y, s.width as i32, s.height as i32) }
}

pub fn read(app: &AppHandle) -> Screens {
    let monitors = crate::main_wait::monitors(app);
    Screens {
        areas: monitors.iter().map(area).collect(),
        primary: crate::main_wait::primary_monitor(app).map(|m| area(&m)),
        names: monitors
            .iter()
            .map(|m| {
                let mon = area(m).mon;
                (mon, m.name().cloned(), crate::desk::key_for(m.name().map(String::as_str), mon))
            })
            .collect(),
    }
}

// The number in a display's device name (\\.\DISPLAY2: 2), as Windows
// numbers them; none in a name of another shape.
pub fn number(name: &str) -> Option<u32> {
    let head = name.trim_end_matches(|c: char| c.is_ascii_digit());
    let digits = &name[head.len()..];
    (!digits.is_empty() && head.to_ascii_uppercase().ends_with("DISPLAY")).then(|| digits.parse().ok()).flatten()
}

impl Screens {
    // A display's device name, and its key.
    pub fn name(&self, mon: (i32, i32, i32, i32)) -> Option<&str> {
        self.names.iter().find(|(m, _, _)| *m == mon).and_then(|(_, name, _)| name.as_deref())
    }

    pub fn key(&self, mon: (i32, i32, i32, i32)) -> Option<&str> {
        self.names.iter().find(|(m, _, _)| *m == mon).map(|(_, _, key)| key.as_str())
    }

    // The display with this key, while it is attached.
    pub fn by_key(&self, key: &str) -> Option<Area> {
        let mon = self.names.iter().find(|(_, _, k)| k == key)?.0;
        self.areas.iter().copied().find(|a| a.mon == mon)
    }

    // For the log: each display, the main one marked, with its scale and
    // work area (multi-display reports could not tell them: #4).
    pub fn describe(&self) -> String {
        self.areas
            .iter()
            .map(|a| {
                let main = self.primary.is_some_and(|p| p.mon == a.mon);
                format!("{:?} at {}%{}, work area ({}, {}, {}, {})", a.mon, (a.sf * 100.0).round(), if main { ", main" } else { "" }, a.x, a.y, a.w, a.h)
            })
            .collect::<Vec<_>>()
            .join("; ")
    }

    // The work area holding this point, or the nearest one.
    pub fn near(&self, x: i32, y: i32) -> Option<Area> {
        let gap = |a: &Area| {
            let dx = (a.x - x).max(0).max(x - (a.x + a.w)) as i64;
            let dy = (a.y - y).max(0).max(y - (a.y + a.h)) as i64;
            dx * dx + dy * dy
        };
        self.areas.iter().copied().min_by_key(gap).or(self.primary)
    }

    // A window of this size at (x, y), kept on some display's work area.
    pub fn clamp(&self, (x, y): (i32, i32), (w, h): (i32, i32)) -> (i32, i32) {
        let Some(a) = self.near(x + w / 2, y + h / 2) else { return (x, y) };
        (x.max(a.x).min(a.x + a.w - w), y.max(a.y).min(a.y + a.h - h))
    }
}

// The pointer, in the physical pixels of a window whose display has the scale
// sf. On a Mac Tauri gives it in the primary display's scale, wherever it is,
// while each window's place is in its own display's: on a second display of
// another scale she never saw the pointer over her and could not be clicked
// (2026-10-14). Elsewhere the two agree.
pub fn cursor(sh: &crate::Shared, sf: f64) -> Option<(i32, i32)> {
    let c = crate::main_wait::cursor(&sh.app)?;
    #[cfg(target_os = "macos")]
    let (x, y) = {
        let primary = sh.screens().primary.map_or(1.0, |a| a.sf);
        (c.x / primary * sf, c.y / primary * sf)
    };
    #[cfg(not(target_os = "macos"))]
    let (x, y) = {
        let _ = sf;
        (c.x, c.y)
    };
    Some((x.round() as i32, y.round() as i32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_displays_number_is_read_from_its_device_name() {
        assert_eq!(number(r"\\.\DISPLAY1"), Some(1));
        assert_eq!(number(r"\\.\DISPLAY12"), Some(12));
        assert_eq!(number(r"\\.\DISPLAY"), None);
        assert_eq!(number("Built-in Retina Display"), None);
        assert_eq!(number("DP-1"), None);
        assert_eq!(number(""), None);
    }

    #[test]
    fn a_display_is_found_by_its_key() {
        let a = Area { x: 0, y: 0, w: 2560, h: 1392, sf: 1.25, mon: (0, 0, 2560, 1440) };
        let b = Area { x: 2560, y: 0, w: 1920, h: 1032, sf: 1.0, mon: (2560, 0, 1920, 1080) };
        let screens = Screens { areas: vec![a, b], primary: Some(a), names: vec![(a.mon, Some(r"\\.\DISPLAY1".into()), "d1".into()), (b.mon, None, "d2".into())] };
        assert_eq!(screens.by_key("d2"), Some(b));
        assert_eq!(screens.by_key("d3"), None);
        assert_eq!(screens.name(a.mon), Some(r"\\.\DISPLAY1"));
        assert_eq!(screens.name(b.mon), None);
        assert_eq!(screens.key(b.mon), Some("d2"));
    }
}
