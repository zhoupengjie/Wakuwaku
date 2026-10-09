// Plugins in the island, the first kind: widgets. Each is a few words a
// script keeps up to date there (or the pet herself, for the built-in ones),
// shown only while no session needs you: sessions always come first.
//
//   POST /widget { id, label, value?, icon?, color?, ttl?, nudge? }
//     id     a-z 0-9 _ -, up to 40; the same id again replaces it
//     label  up to 40 characters; value up to 24
//     icon   one of ICONS, else a dot
//     color  #rgb or #rrggbb, for the value
//     ttl    how long (s) it stays without a new one: 300 by default, 10 to 86400
//     nudge  true, or a few words: the island opens for it once, if nothing
//            else wants you; once a minute at most, per widget
//     private  true: the label (and the nudge's words) are kept off screen
//            while the specifics are (the details setting off); the value,
//            like "3 封未读", still shows
//   POST /widget { id, remove: true }
//
// Built in (texts as { key, vars } for the page to say in her language):
//   sys      CPU and memory, every few seconds
//   net      how fast the network is moving now
//   battery  charge, and charging or time left (only with a battery)
//   today    turns today, how long Claude worked, approvals given on her
//   tokens   today's tokens, Claude Code's and Codex's (tokens.rs)
use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Value};

pub const ICONS: [&str; 21] = [
    "cpu", "weather", "note", "calendar", "bell", "stock", "today", "clock", "mail", "music", "code", "star", "dot", "chart", "battery", "timer", "flag", "server",
    "check", "terminal", "coin",
];
// The built-in ones: id, icon, colour.
pub const BUILT_IN: [(&str, &str, &str); 5] =
    [("today", "today", "#34d27b"), ("tokens", "code", "#ff9f0a"), ("sys", "cpu", "#5e9bff"), ("net", "chart", "#64d2ff"), ("battery", "battery", "#34d27b")];
const MAX_SCRIPTED: usize = 32;
const DEFAULT_TTL: u64 = 300;
const NUDGE_EVERY_MS: u64 = 60_000;

pub struct Widget {
    pub id: String,
    label: Value,
    value: Value,
    icon: String,
    color: String,
    // When it goes, if no new one comes (ms); never, for the built-in ones.
    until: Option<u64>,
    at: u64,
    private: bool,
}

#[derive(Default)]
pub struct Widgets {
    // In the order first seen; the built-in ones first.
    list: Vec<Widget>,
    nudged: HashMap<String, u64>,
}

fn clip(s: &str, max: usize) -> String {
    s.trim().chars().take(max).collect()
}

pub fn is_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

fn is_color(c: &str) -> bool {
    let hex = c.strip_prefix('#').unwrap_or("");
    matches!(hex.len(), 3 | 6) && hex.chars().all(|c| c.is_ascii_hexdigit())
}

// What a script sent, checked.
pub struct Put {
    pub id: String,
    // The words to open the island with, when it asked to.
    pub nudge: Option<String>,
}

impl Widgets {
    // A widget from a script: Err with why, when it will not do.
    pub fn put(&mut self, v: &Value, now: u64) -> Result<Put, &'static str> {
        let s = |key: &str| v.get(key).and_then(Value::as_str).unwrap_or("");
        let id = s("id");
        if !is_id(id) {
            return Err("id: a-z 0-9 _ -, up to 40");
        }
        if BUILT_IN.iter().any(|b| b.0 == id) {
            return Err("id: taken by a built-in widget");
        }
        if v.get("remove").and_then(Value::as_bool) == Some(true) {
            self.list.retain(|w| w.id != id);
            return Ok(Put { id: id.into(), nudge: None });
        }
        let label = clip(s("label"), 40);
        if label.is_empty() {
            return Err("label: some words, up to 40");
        }
        let ttl = v.get("ttl").and_then(Value::as_u64).unwrap_or(DEFAULT_TTL).clamp(10, 86_400);
        let widget = Widget {
            id: id.into(),
            label: json!(label),
            value: json!(clip(s("value"), 24)),
            icon: if ICONS.contains(&s("icon")) { s("icon").into() } else { "dot".into() },
            color: if is_color(s("color")) { s("color").into() } else { String::new() },
            until: Some(now + ttl * 1000),
            at: now,
            private: v.get("private").and_then(Value::as_bool) == Some(true),
        };
        let scripted = self.list.iter().filter(|w| w.until.is_some()).count();
        match self.list.iter_mut().find(|w| w.id == id) {
            Some(old) => *old = widget,
            None if scripted >= MAX_SCRIPTED => return Err("too many widgets: 32 at most"),
            None => self.list.push(widget),
        }
        let nudge = match v.get("nudge") {
            Some(Value::Bool(true)) => Some(String::new()),
            Some(Value::String(words)) if !words.trim().is_empty() => Some(clip(words, 60)),
            _ => None,
        };
        // Once a minute at most, per widget.
        let nudge = nudge.filter(|_| self.nudged.get(id).is_none_or(|&t| now.saturating_sub(t) >= NUDGE_EVERY_MS));
        if nudge.is_some() {
            self.nudged.insert(id.into(), now);
        }
        Ok(Put { id: id.into(), nudge })
    }

    // A built-in widget's words, now. True when they changed.
    pub fn set_built_in(&mut self, id: &str, value: Value, now: u64) -> bool {
        if let Some(w) = self.list.iter_mut().find(|w| w.id == id) {
            if w.value == value {
                return false;
            }
            w.value = value;
            w.at = now;
            return true;
        }
        let at = self.list.iter().take_while(|w| w.until.is_none()).count();
        self.list.insert(at, Widget { value, at: now, ..built_in(id) });
        true
    }

    pub fn remove_built_in(&mut self, id: &str) -> bool {
        let before = self.list.len();
        self.list.retain(|w| w.id != id);
        self.list.len() != before
    }

    // Those whose script stopped sending go. True when any did.
    pub fn expire(&mut self, now: u64) -> bool {
        let before = self.list.len();
        self.list.retain(|w| w.until.is_none_or(|t| t > now));
        self.list.len() != before
    }

    // A widget's words, and whether they are private.
    pub fn label_of(&self, id: &str) -> Option<(Value, Value, bool)> {
        self.list.iter().find(|w| w.id == id).map(|w| (w.label.clone(), w.value.clone(), w.private))
    }

    // A script's widgets that are gone with it. True when any were there.
    pub fn remove_scripted(&mut self, gone: impl Fn(&str) -> bool) -> bool {
        let before = self.list.len();
        self.list.retain(|w| w.until.is_none() || !gone(&w.id));
        self.list.len() != before
    }

    // All of them, in the order the person put them in (then as first seen),
    // each saying whether it is on and which plugin it comes from (`owner`:
    // one she runs, scripts.rs), which places it when the plugin was placed.
    // The built-in ones not measured (off, or nothing to read yet, like a
    // battery on a desktop) are there too, with no value, so they can be
    // turned on.
    pub fn view(&self, now: u64, off: &[String], order: &[String], owner: &dyn Fn(&str) -> Option<&'static str>) -> Value {
        let unread: Vec<Widget> = BUILT_IN.iter().filter(|b| !self.list.iter().any(|w| w.id == b.0)).map(|b| built_in(b.0)).collect();
        let measured = self.list.iter().take_while(|w| w.until.is_none()).count();
        let mut all: Vec<&Widget> = self.list[..measured].iter().chain(&unread).chain(&self.list[measured..]).collect();
        let rank = |w: &Widget| order.iter().position(|o| *o == w.id || Some(o.as_str()) == owner(&w.id)).unwrap_or(usize::MAX);
        all.sort_by_key(|w| rank(w));
        Value::Array(
            all.into_iter()
                .map(|w| {
                    json!({
                        "id": w.id,
                        "label": w.label,
                        "value": w.value,
                        "icon": w.icon,
                        "color": w.color,
                        "builtIn": w.until.is_none(),
                        "plugin": owner(&w.id),
                        "on": !off.contains(&w.id),
                        "leftMs": w.until.map(|t| t.saturating_sub(now)),
                        "at": w.at,
                        "private": w.private,
                    })
                })
                .collect(),
        )
    }
}

// A built-in widget with nothing to say yet.
fn built_in(id: &str) -> Widget {
    let (icon, color) = BUILT_IN.iter().find(|b| b.0 == id).map_or(("dot", ""), |b| (b.1, b.2));
    Widget { id: id.into(), label: json!({ "key": format!("widget.{id}") }), value: Value::Null, icon: icon.into(), color: color.into(), until: None, at: 0, private: false }
}

// --- Today: what the day has seen ---------------------------------------------------

#[derive(Default, Clone, PartialEq, Debug)]
pub struct Today {
    pub date: String,
    pub turns: u64,
    pub worked_ms: u64,
    pub approvals: u64,
}

impl Today {
    pub fn load(dir: &Path, date: &str) -> Today {
        let saved: Value = std::fs::read_to_string(dir.join("today.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
        if saved["date"].as_str() != Some(date) {
            return Today { date: date.into(), ..Today::default() };
        }
        let n = |k: &str| saved[k].as_u64().unwrap_or(0);
        Today { date: date.into(), turns: n("turns"), worked_ms: n("workedMs"), approvals: n("approvals") }
    }

    pub fn save(&self, dir: &Path) {
        let body = json!({ "date": self.date, "turns": self.turns, "workedMs": self.worked_ms, "approvals": self.approvals });
        let _ = std::fs::write(dir.join("today.json"), body.to_string());
    }

    // A new day starts from nothing. True when it did.
    pub fn roll(&mut self, date: &str) -> bool {
        if self.date == date {
            return false;
        }
        *self = Today { date: date.into(), ..Today::default() };
        true
    }

    pub fn words(&self) -> Value {
        let minutes = self.worked_ms / 60_000;
        json!({ "key": "widget.todayValue", "vars": { "turns": self.turns.to_string(), "time": format!("{}:{:02}", minutes / 60, minutes % 60), "approvals": self.approvals.to_string() } })
    }
}

// The local date, YYYY-MM-DD.
pub fn local_date() -> String {
    let (y, m, d, _) = local_now();
    format!("{y:04}-{m:02}-{d:02}")
}

// Local midnight, in ms since 1970.
pub fn local_midnight() -> u64 {
    crate::now_ms().saturating_sub(local_now().3)
}

// The local year, month, day, and ms since midnight.
fn local_now() -> (u16, u16, u16, u64) {
    #[cfg(windows)]
    {
        #[repr(C)]
        #[derive(Default)]
        struct SystemTime {
            year: u16,
            month: u16,
            weekday: u16,
            day: u16,
            hour: u16,
            minute: u16,
            second: u16,
            ms: u16,
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GetLocalTime(time: *mut SystemTime);
        }
        let mut t = SystemTime::default();
        // SAFETY: fills our own struct.
        unsafe { GetLocalTime(&mut t) };
        let ms = ((u64::from(t.hour) * 60 + u64::from(t.minute)) * 60 + u64::from(t.second)) * 1000 + u64::from(t.ms);
        (t.year, t.month, t.day, ms)
    }
    #[cfg(not(windows))]
    {
        // Without the local time: the day in UTC.
        let now = crate::now_ms();
        let days = (now / 86_400_000) as u16;
        (1970, 1, days, now % 86_400_000)
    }
}

// --- The machine: CPU and memory ----------------------------------------------------------

#[cfg(windows)]
mod sys {
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    #[repr(C)]
    struct MemoryStatus {
        length: u32,
        load: u32,
        total_phys: u64,
        avail_phys: u64,
        total_page: u64,
        avail_page: u64,
        total_virtual: u64,
        avail_virtual: u64,
        avail_extended: u64,
    }

    #[repr(C)]
    #[derive(Default)]
    struct PowerStatus {
        ac_line: u8,
        battery_flag: u8,
        percent: u8,
        saver: u8,
        life_secs: u32,
        full_life_secs: u32,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetSystemTimes(idle: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> i32;
        fn GlobalMemoryStatusEx(status: *mut MemoryStatus) -> i32;
        fn GetSystemPowerStatus(status: *mut PowerStatus) -> i32;
    }

    #[link(name = "iphlpapi")]
    extern "system" {
        fn GetIfTable(table: *mut u8, size: *mut u32, order: i32) -> u32;
    }

    // (percent, charging, seconds left if known), or None without a battery.
    pub fn battery() -> Option<(u8, bool, Option<u32>)> {
        let mut p = PowerStatus::default();
        // SAFETY: fills our own struct.
        if unsafe { GetSystemPowerStatus(&mut p) } == 0 || p.battery_flag & 128 != 0 || p.battery_flag == 255 || p.percent > 100 {
            return None;
        }
        let is_charging = p.ac_line == 1;
        Some((p.percent, is_charging, (p.life_secs != u32::MAX && !is_charging).then_some(p.life_secs)))
    }

    // Bytes in and out so far, over the network adapters that are up: each
    // adapter once (Windows lists one several times, through its filters).
    pub fn network_bytes() -> Option<(u64, u64)> {
        // MIB_IFROW: 860 bytes; the type at 516, the MAC at 532 (8), the
        // state at 544, bytes in at 552, bytes out at 576.
        const ROW: usize = 860;
        let mut size = 0u32;
        // SAFETY: first the size, then a buffer of that size.
        unsafe { GetIfTable(std::ptr::null_mut(), &mut size, 0) };
        let mut buf = vec![0u8; size as usize + ROW];
        // SAFETY: the buffer is as big as the call asked for.
        if unsafe { GetIfTable(buf.as_mut_ptr(), &mut size, 0) } != 0 {
            return None;
        }
        let dword = |at: usize| u32::from_le_bytes([buf[at], buf[at + 1], buf[at + 2], buf[at + 3]]);
        let count = dword(0) as usize;
        let mut by_mac: std::collections::HashMap<[u8; 8], (u32, u32)> = std::collections::HashMap::new();
        for i in 0..count {
            let row = 4 + i * ROW;
            if row + ROW > buf.len() {
                break;
            }
            let (kind, state) = (dword(row + 516), dword(row + 544));
            let mut mac = [0u8; 8];
            mac.copy_from_slice(&buf[row + 532..row + 540]);
            // Ethernet or Wi-Fi, up, with an address of its own.
            if !(kind == 6 || kind == 71) || state != 5 || mac == [0; 8] {
                continue;
            }
            let (got, sent) = (dword(row + 552), dword(row + 576));
            let seen = by_mac.entry(mac).or_insert((got, sent));
            *seen = (seen.0.max(got), seen.1.max(sent));
        }
        Some(by_mac.values().fold((0, 0), |(a, b), &(got, sent)| (a + u64::from(got), b + u64::from(sent))))
    }

    fn n(t: FileTime) -> u64 {
        (u64::from(t.high) << 32) | u64::from(t.low)
    }

    // (idle, busy) time so far, all cores.
    pub fn times() -> Option<(u64, u64)> {
        let (mut idle, mut kernel, mut user) = (FileTime::default(), FileTime::default(), FileTime::default());
        // SAFETY: fills our own structs.
        let ok = unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } != 0;
        // Kernel time includes the idle time.
        ok.then(|| (n(idle), n(kernel) + n(user) - n(idle)))
    }

    pub fn memory_load() -> Option<u32> {
        // SAFETY: a struct of the size the call asks for.
        unsafe {
            let mut s: MemoryStatus = std::mem::zeroed();
            s.length = std::mem::size_of::<MemoryStatus>() as u32;
            (GlobalMemoryStatusEx(&mut s) != 0).then_some(s.load)
        }
    }
}

#[cfg(not(windows))]
mod sys {
    pub fn times() -> Option<(u64, u64)> {
        None
    }
    pub fn memory_load() -> Option<u32> {
        None
    }
    pub fn battery() -> Option<(u8, bool, Option<u32>)> {
        None
    }
    pub fn network_bytes() -> Option<(u64, u64)> {
        None
    }
}

// The battery, as words; None without one.
pub fn battery_words() -> Option<Value> {
    let (percent, is_charging, left) = sys::battery()?;
    Some(match (is_charging, left) {
        (true, _) => json!({ "key": "widget.batteryCharging", "vars": { "percent": percent.to_string() } }),
        (false, Some(secs)) => json!({ "key": "widget.batteryLeft", "vars": { "percent": percent.to_string(), "time": format!("{}:{:02}", secs / 3600, secs / 60 % 60) } }),
        (false, None) => json!(format!("{percent}%")),
    })
}

// Bytes a second, as a few characters: 1.2 MB/s, 80 KB/s.
pub fn rate(bytes_per_sec: f64) -> String {
    let (n, unit) = match bytes_per_sec {
        b if b >= 1e9 => (b / 1e9, "GB/s"),
        b if b >= 1e6 => (b / 1e6, "MB/s"),
        b if b >= 1e3 => (b / 1e3, "KB/s"),
        b => (b, "B/s"),
    };
    if n >= 100.0 || unit == "B/s" { format!("{n:.0} {unit}") } else { format!("{n:.1} {unit}") }
}

// How fast the network moves, between two readings.
pub struct Network {
    last: Option<(u64, u64, std::time::Instant)>,
}

impl Network {
    pub fn new() -> Self {
        Network { last: sys::network_bytes().map(|(a, b)| (a, b, std::time::Instant::now())) }
    }

    pub fn words(&mut self) -> Option<Value> {
        let (got, sent) = sys::network_bytes()?;
        let now = std::time::Instant::now();
        let words = self.last.map(|(g, s, at)| {
            let secs = now.duration_since(at).as_secs_f64().max(0.5);
            // The counters are 32 bits: they come round again after 4 GB.
            let d = |a: u64, b: u64| (a as u32).wrapping_sub(b as u32) as f64 / secs;
            json!(format!("↓ {} ↑ {}", rate(d(got, g)), rate(d(sent, s))))
        });
        self.last = Some((got, sent, now));
        words
    }
}

// CPU use between two readings of sys::times, and memory in use, as words.
pub struct Machine {
    last: Option<(u64, u64)>,
}

impl Machine {
    pub fn new() -> Self {
        Machine { last: sys::times() }
    }

    pub fn words(&mut self) -> Option<Value> {
        let now = sys::times()?;
        let cpu = self.last.map(|(idle, busy)| {
            let (di, db) = (now.0.saturating_sub(idle), now.1.saturating_sub(busy));
            if di + db == 0 { 0 } else { (db * 100 + (di + db) / 2) / (di + db) }
        });
        self.last = Some(now);
        let memory = sys::memory_load()?;
        Some(json!(format!("{}% · {}%", cpu.unwrap_or(0), memory)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The view without the built-in ones, which are always there.
    fn scripted(view: Value) -> Value {
        Value::Array(view.as_array().unwrap().iter().filter(|w| w["builtIn"] == false).cloned().collect())
    }

    #[test]
    fn a_script_puts_replaces_and_removes_its_widget() {
        let mut w = Widgets::default();
        let put = w.put(&json!({ "id": "weather", "label": "上海 · 多云", "value": "22°", "icon": "weather", "color": "#ffb340" }), 1_000).unwrap();
        assert_eq!((put.id.as_str(), put.nudge), ("weather", None));
        w.put(&json!({ "id": "weather", "label": "上海 · 晴", "value": "24°", "icon": "nope", "color": "red" }), 2_000).unwrap();
        let view = scripted(w.view(2_000, &[], &[], &|_| None));
        assert_eq!(view.as_array().unwrap().len(), 1);
        assert_eq!((view[0]["label"].as_str(), view[0]["icon"].as_str(), view[0]["color"].as_str()), (Some("上海 · 晴"), Some("dot"), Some("")));
        assert_eq!(view[0]["leftMs"], 300_000);
        w.put(&json!({ "id": "weather", "remove": true }), 3_000).unwrap();
        assert!(scripted(w.view(3_000, &[], &[], &|_| None)).as_array().unwrap().is_empty());
    }

    #[test]
    fn bad_widgets_say_why() {
        let mut w = Widgets::default();
        assert!(w.put(&json!({ "id": "Has Space", "label": "x" }), 0).is_err());
        assert!(w.put(&json!({ "id": "x" }), 0).is_err());
        assert!(w.put(&json!({ "id": "sys", "label": "mine" }), 0).is_err());
        for n in 0..MAX_SCRIPTED {
            w.put(&json!({ "id": format!("w{n}"), "label": "x" }), 0).unwrap();
        }
        assert!(w.put(&json!({ "id": "one-more", "label": "x" }), 0).is_err());
    }

    #[test]
    fn widgets_go_when_their_script_stops_and_nudge_once_a_minute() {
        let mut w = Widgets::default();
        let put = w.put(&json!({ "id": "cal", "label": "15:00 设计评审", "ttl": 10, "nudge": "5 分钟后开会" }), 0).unwrap();
        assert_eq!(put.nudge.as_deref(), Some("5 分钟后开会"));
        assert!(w.put(&json!({ "id": "cal", "label": "15:00 设计评审", "ttl": 10, "nudge": true }), 30_000).unwrap().nudge.is_none());
        assert_eq!(w.put(&json!({ "id": "cal", "label": "15:00 设计评审", "ttl": 10, "nudge": true }), 61_000).unwrap().nudge.as_deref(), Some(""));
        assert!(!w.expire(70_000));
        assert!(w.expire(71_001));
    }

    #[test]
    fn built_ins_stay_first_and_the_order_is_the_persons() {
        let mut w = Widgets::default();
        w.put(&json!({ "id": "a", "label": "A" }), 0).unwrap();
        assert!(w.set_built_in("sys", json!("1"), 0));
        assert!(!w.set_built_in("sys", json!("1"), 5));
        let ids = |v: Value| v.as_array().unwrap().iter().map(|w| w["id"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        // The ones not measured are listed too, without a value.
        let none = |_: &str| None;
        let all = w.view(0, &[], &[], &none);
        assert_eq!(ids(all.clone()), ["sys", "today", "tokens", "net", "battery", "a"]);
        assert_eq!((all[0]["value"].clone(), all[1]["value"].clone(), all[1]["icon"].clone()), (json!("1"), Value::Null, json!("today")));
        assert_eq!(ids(w.view(0, &[], &["a".into()], &none))[0], "a");
        assert_eq!(w.view(0, &["net".into()], &[], &none)[3]["on"], false);
        // A plugin's widgets go where the plugin was put, and say whose they are.
        let mine = |id: &str| (id == "a").then_some("mine");
        let view = w.view(0, &[], &["mine".into()], &mine);
        assert_eq!((view[0]["id"].clone(), view[0]["plugin"].clone()), (json!("a"), json!("mine")));
        assert!(w.remove_scripted(|id| id == "a") && !w.remove_scripted(|id| id == "sys"));
        w.put(&json!({ "id": "a", "label": "A" }), 0).unwrap();
        // Built-in ones never go; a script's does.
        assert!(w.expire(u64::MAX));
        assert_eq!(ids(w.view(0, &[], &[], &none)), ["sys", "today", "tokens", "net", "battery"]);
    }

    #[test]
    fn private_widgets_say_so_and_rates_read_short() {
        let mut w = Widgets::default();
        w.put(&json!({ "id": "mail", "label": "王总：周五的方案", "value": "3 封未读", "private": true }), 0).unwrap();
        assert_eq!(scripted(w.view(0, &[], &[], &|_| None))[0]["private"], true);
        assert_eq!(w.label_of("mail").map(|l| l.2), Some(true));
        assert_eq!((rate(80.0), rate(81_234.0), rate(1_234_567.0), rate(250_000_000.0)), ("80 B/s".into(), "81.2 KB/s".into(), "1.2 MB/s".into(), "250 MB/s".into()));
        // On this machine: a reading, or none, but never a panic.
        let _ = (battery_words(), Network::new().words());
    }

    #[test]
    fn today_counts_and_starts_again_each_day() {
        let dir = std::env::temp_dir().join(format!("wakuwaku-today-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut t = Today::load(&dir, "2026-10-09");
        t.turns = 12;
        t.worked_ms = 103 * 60_000;
        t.approvals = 5;
        t.save(&dir);
        assert_eq!(Today::load(&dir, "2026-10-09"), t);
        assert_eq!(t.words(), json!({ "key": "widget.todayValue", "vars": { "turns": "12", "time": "1:43", "approvals": "5" } }));
        assert_eq!(Today::load(&dir, "2026-10-10").turns, 0);
        assert!(t.roll("2026-10-10") && t.turns == 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
