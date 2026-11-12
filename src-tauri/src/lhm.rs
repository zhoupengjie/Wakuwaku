// The hardware's sensors in the monitor (#18, its first step): temperatures,
// loads, power, fans, clocks and a card's memory, from a LibreHardwareMonitor
// (LHM) the person runs. She downloads, sets up and starts it only when they
// click for it (lhm_setup.rs: its driver, and the administrator's rights most
// sensors need, are theirs to give); she reads what its Remote Web Server
// serves (LHM's Options → Remote Web Server → Run): GET
// http://127.0.0.1:<port>/data.json, the whole tree.
// Its WMI classes (root\LibreHardwareMonitor) would have needed nothing
// turned on, but LHM 0.9.5 dropped them with .NET 10 (LibreHardwareMonitor
// #2143): the web server is the one way into a current LHM.
//
// Read on the monitor's thread alone (widgets::Monitor::read), 300 ms at
// most, at most every 3 s while found (every 10 s while quiet), every 30 s
// while not; a reading that fails once keeps the last, twice and it is not
// found. Off, or never found, nothing in the monitor changes. Asked only
// while it is the monitor's source ("monitorSource": "lhm"; the other is the
// built-in one, hw.rs, which also says which a setting from before there was
// a choice means).
//
// Its settings, in "monitor": lhm (false: never asked; from before the
// source was chosen), lhmPort (8085),
// lhmUser and lhmPass (LHM's Basic authentication: the password kept in
// Windows' Credential Manager, the setting only saying true), sensors (the
// SensorIds shown, in the tree's order; unset, the CPU's package temperature
// and the discrete card's temperature and load), lhmStartup (false: an LHM
// she downloads is not made to start at logon by its task; lhm_setup.rs).
//
// data.json as LHM writes it (HttpServer.cs): nodes of { id, Text, Min,
// Value, Max, ImageURL, Children }; a sensor's also SensorId
// ("/amdcpu/0/temperature/2", its hardware's id and its own) and Type
// ("Temperature"); since 0.9.5 a hardware's HardwareId and a sensor's Raw*
// values; in 0.9.6 those are the same words as its Value ("62.4 °C",
// written in LHM's language: "62,4 °C" in German), since March 2026 numbers,
// and the root has LHM's Version. A value is taken from RawValue as a
// number, else from its words, else from Value's.
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use crate::lock::Lock;

pub const PORT: u16 = 8085;
const TIMEOUT: Duration = Duration::from_millis(300);
const EVERY: Duration = Duration::from_secs(3);
const EVERY_QUIET: Duration = Duration::from_secs(10);
const AGAIN: Duration = Duration::from_secs(30);
// The password's name in Windows' Credential Manager.
const SECRET: &str = "Wakuwaku LibreHardwareMonitor";
pub const MAX_SENSORS: usize = 32;

// The kinds shown; LHM's others (voltages, throughput, …) are left out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Temperature,
    Load,
    Power,
    Fan,
    Clock,
    // SmallData: a card's memory, in MB.
    Data,
    // A drive read and written, bytes a second: the built-in source's alone (hw.rs).
    Read,
    Write,
}

impl Kind {
    fn of(t: &str) -> Option<Kind> {
        Some(match t {
            "Temperature" => Kind::Temperature,
            "Load" => Kind::Load,
            "Power" => Kind::Power,
            "Fan" => Kind::Fan,
            "Clock" => Kind::Clock,
            "SmallData" => Kind::Data,
            _ => return None,
        })
    }

    // As the page knows it (widgets.ts shareOf, isHigh).
    pub fn name(self) -> &'static str {
        match self {
            Kind::Temperature => "temp",
            Kind::Load => "load",
            Kind::Power => "power",
            Kind::Fan => "fan",
            Kind::Clock => "clock",
            Kind::Data => "data",
            Kind::Read => "read",
            Kind::Write => "write",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sensor {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    // Its hardware: the id (its SensorId's first part) and name.
    pub hardware_id: String,
    pub hardware: String,
    pub value: Option<f64>,
}

// The number in one of LHM's words for a value ("62.4 °C", "62,4 °C" where
// its language writes a comma, "1180 RPM", "-5.0 °C"; "-" for none): the
// first, the one point or comma between its digits taken for the decimal
// point (LHM writes no thousands' separators).
pub fn number_in(s: &str) -> Option<f64> {
    let b = s.as_bytes();
    let start = b.iter().position(u8::is_ascii_digit)?;
    let digits = |from: usize| (from..b.len()).find(|&i| !b[i].is_ascii_digit()).unwrap_or(b.len());
    let end = digits(start);
    let mut n = s[start..end].to_string();
    if end + 1 < b.len() && matches!(b[end], b'.' | b',') && b[end + 1].is_ascii_digit() {
        n = format!("{n}.{}", &s[end + 1..digits(end + 1)]);
    }
    let v: f64 = n.parse().ok()?;
    Some(if start > 0 && b[start - 1] == b'-' { -v } else { v })
}

// The tree as LHM sends it: its version (none before March 2026), and its
// sensors of the kinds shown, in its order; None for what is not LHM's.
pub fn parse(tree: &Value) -> Option<(Option<String>, Vec<Sensor>)> {
    let children = tree.get("Children")?.as_array()?;
    let mut out = Vec::new();
    for child in children {
        walk(child, &mut Vec::new(), &mut out);
    }
    Some((tree["Version"].as_str().map(str::to_string), out))
}

// A node and those under it; `up` the names above it. A sensor's hardware
// is two above it (hardware, then its kind's node: "Temperatures"), its id
// the SensorId's first part (HardwareId, where LHM sends it, says the same).
fn walk<'a>(node: &'a Value, up: &mut Vec<&'a str>, out: &mut Vec<Sensor>) {
    let text = node["Text"].as_str().unwrap_or("");
    if let (Some(id), Some(kind)) = (node["SensorId"].as_str(), node["Type"].as_str().and_then(Kind::of)) {
        let value = node["RawValue"]
            .as_f64()
            .or_else(|| node["RawValue"].as_str().and_then(number_in))
            .or_else(|| node["Value"].as_str().and_then(number_in))
            .filter(|v| v.is_finite())
            // A temperature of 0.0 °C is one it cannot read (the CPU's
            // without its PawnIO driver, seen in the test VM): none, not 0°.
            .filter(|v| !(kind == Kind::Temperature && *v == 0.0));
        let hardware_id = id.rsplitn(3, '/').nth(2).unwrap_or("").to_string();
        let hardware = up.len().checked_sub(2).map_or("", |i| up[i]).to_string();
        out.push(Sensor { id: id.into(), name: text.into(), kind, hardware_id, hardware, value });
    }
    up.push(text);
    for child in node["Children"].as_array().into_iter().flatten() {
        walk(child, up, out);
    }
    up.pop();
}

fn is_cpu(hardware_id: &str) -> bool {
    ["/amdcpu/", "/intelcpu/", "/cpu/"].iter().any(|p| hardware_id.starts_with(p))
}

fn is_gpu(hardware_id: &str) -> bool {
    hardware_id.starts_with("/gpu")
}

// How surely a card is a discrete one: NVIDIA's always; AMD's and Intel's
// by their names (Radeon RX, Radeon Pro, Arc A770 and B580: a letter and
// three digits), the others (Radeon Graphics, Radeon 780M, UHD, Iris, Arc
// 140V) taken for the processor's own.
fn discrete(s: &Sensor) -> u8 {
    discrete_card(&s.hardware_id, &s.hardware)
}

// The same by the card's id and name (the built-in source's too, hw.rs).
pub fn discrete_card(hardware_id: &str, name: &str) -> u8 {
    let words: Vec<&str> = name.split(|c: char| c.is_whitespace() || c == '(' || c == ')').collect();
    let model = |w: &&str| w.len() == 4 && (w.starts_with('A') || w.starts_with('B')) && w[1..].bytes().all(|c| c.is_ascii_digit());
    if hardware_id.starts_with("/gpu-nvidia/") {
        2
    } else if words.contains(&"RX") || name.contains("Radeon Pro") || name.contains("FirePro") || words.iter().any(model) {
        1
    } else {
        0
    }
}

// The ones shown when none were picked: the CPU's package temperature (by
// its name, as Intel's and AMD's are called, else its first), the discrete
// card's core temperature and load.
pub fn defaults(sensors: &[Sensor]) -> Vec<String> {
    let mut picked = Vec::new();
    let named = |list: Vec<&Sensor>, names: &[&str]| names.iter().find_map(|n| list.iter().find(|s| s.name == *n).map(|s| s.id.clone())).or_else(|| list.first().map(|s| s.id.clone()));
    if let Some(cpu) = sensors.iter().find(|s| is_cpu(&s.hardware_id)).map(|s| s.hardware_id.clone()) {
        let temps = sensors.iter().filter(|s| s.hardware_id == cpu && s.kind == Kind::Temperature).collect();
        picked.extend(named(temps, &["CPU Package", "Package", "Core (Tctl/Tdie)", "Core (Tdie)", "Core (Tctl)", "Core Max"]));
    }
    let card = sensors.iter().filter(|s| is_gpu(&s.hardware_id) && discrete(s) > 0).max_by_key(|s| (discrete(s), std::cmp::Reverse(sensors.iter().position(|x| x.hardware_id == s.hardware_id))));
    if let Some(card) = card.map(|s| s.hardware_id.clone()) {
        for kind in [Kind::Temperature, Kind::Load] {
            picked.extend(named(sensors.iter().filter(|s| s.hardware_id == card && s.kind == kind).collect(), &["GPU Core"]));
        }
    }
    picked
}

// Its glyph in the monitor (widgets.ts PART_GLYPH): the hardware's, a fan's own.
fn icon(s: &Sensor) -> &'static str {
    let h = s.hardware_id.as_str();
    if s.kind == Kind::Fan {
        "fan"
    } else if is_cpu(h) {
        "cpu"
    } else if is_gpu(h) {
        "gpu"
    } else if ["/motherboard", "/mainboard", "/lpc/", "/ec/", "/superio"].iter().any(|p| h.starts_with(p)) {
        "board"
    } else if ["/nvme/", "/hdd/", "/ssd/", "/storage/", "/ata/"].iter().any(|p| h.starts_with(p)) {
        "disk"
    } else if h.starts_with("/ram") || h.starts_with("/memory") {
        "memory"
    } else {
        "sensor"
    }
}

// A value in a cell's few characters, its unit or a letter for it: 62°,
// 37%, 8.5W and 122W, 1180 (a fan, revolutions a minute), 4.5G and 800M
// (a clock), 3.0G and 512M (memory), 1.2M (a drive's bytes a second).
pub fn short(kind: Kind, v: f64) -> String {
    let one_or_none = |v: f64, unit: &str| if v < 10.0 { format!("{v:.1}{unit}") } else { format!("{v:.0}{unit}") };
    match kind {
        Kind::Temperature => format!("{v:.0}°"),
        Kind::Load => format!("{v:.0}%"),
        Kind::Power => one_or_none(v, "W"),
        Kind::Fan => format!("{v:.0}"),
        Kind::Clock if v >= 1000.0 => one_or_none(v / 1000.0, "G"),
        Kind::Clock => format!("{v:.0}M"),
        Kind::Data if v >= 1024.0 => one_or_none(v / 1024.0, "G"),
        Kind::Data => format!("{v:.0}M"),
        Kind::Read | Kind::Write => crate::widgets::rate(v),
    }
}

// The same in the settings' list, as LHM shows it: 62.4 °C, 1180 RPM.
fn long(kind: Kind, v: Option<f64>) -> String {
    let Some(v) = v else { return "—".into() };
    match kind {
        Kind::Temperature => format!("{v:.1} °C"),
        Kind::Load => format!("{v:.1} %"),
        Kind::Power => format!("{v:.1} W"),
        Kind::Fan => format!("{v:.0} RPM"),
        Kind::Clock => format!("{v:.0} MHz"),
        Kind::Data => format!("{v:.0} MB"),
        Kind::Read | Kind::Write => format!("{}/s", crate::widgets::rate(v)),
    }
}

// How she reaches it, from the "monitor" setting.
#[derive(Clone, Debug, PartialEq)]
pub struct Conf {
    pub on: bool,
    pub port: u16,
    pub user: String,
    // A password kept (in the Credential Manager).
    pub pass: bool,
    // The sensors picked; None: the defaults.
    pub sensors: Option<Vec<String>>,
}

impl Conf {
    pub fn from(m: &Value) -> Conf {
        Conf {
            on: m["lhm"] != false,
            port: m["lhmPort"].as_u64().and_then(|p| u16::try_from(p).ok()).filter(|&p| p > 0).unwrap_or(PORT),
            user: m["lhmUser"].as_str().unwrap_or("").to_string(),
            pass: m["lhmPass"] == true,
            sensors: m["sensors"].as_array().map(|ids| ids.iter().filter_map(Value::as_str).map(str::to_string).collect()),
        }
    }
}

// The settings' "monitor" value may carry these, checked (settings.rs is_ok).
pub fn is_setting(key: &str, v: &Value) -> Option<bool> {
    Some(match key {
        "lhm" | "lhmStartup" => v.is_boolean(),
        "lhmPort" => v.as_u64().is_some_and(|p| (1..=65535).contains(&p)),
        "lhmUser" => v.as_str().is_some_and(|s| s.chars().count() <= 64 && !s.contains(':')),
        // A password typed (kept, keep_password) or "" to forget it, or true: one kept.
        "lhmPass" => v.is_boolean() || v.as_str().is_some_and(|s| s.chars().count() <= 256),
        "sensors" => v.as_array().is_some_and(|ids| ids.len() <= MAX_SENSORS && ids.iter().all(|id| id.as_str().is_some_and(|s| s.starts_with('/') && s.len() <= 160))),
        _ => return None,
    })
}

// The password, when one was kept, bumped each time a new one is: asked
// with the new one at once.
static SECRET_REV: AtomicU64 = AtomicU64::new(0);
// Bumped to have it asked at once (its web server just answered, after
// lhm_setup.rs started it), and the monitor's thread woken to read.
static ASKED_AGAIN: AtomicU64 = AtomicU64::new(0);
static READ_NOW: AtomicBool = AtomicBool::new(false);

// LHM asked again at its next reading, which comes now.
pub fn ask_now() {
    ASKED_AGAIN.fetch_add(1, Ordering::SeqCst);
    READ_NOW.store(true, Ordering::SeqCst);
    crate::loops::wake_monitor();
}

// Whether a reading is wanted now (ask_now), for the monitor's thread.
pub fn take_read_now() -> bool {
    READ_NOW.swap(false, Ordering::SeqCst)
}

// A password typed in the settings goes to the Credential Manager, the
// setting kept saying only that there is one; "" forgets it.
pub fn keep_password(mut monitor: Value) -> Value {
    if let Some(typed) = monitor.get("lhmPass").and_then(Value::as_str).map(str::to_string) {
        let kept = if typed.is_empty() {
            crate::mail::secret::delete_named(SECRET);
            false
        } else {
            crate::mail::secret::write_named(SECRET, &typed).is_ok()
        };
        SECRET_REV.fetch_add(1, Ordering::SeqCst);
        monitor["lhmPass"] = json!(kept);
    }
    monitor
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Status {
    Off,
    // Not asked yet.
    Unread,
    // Nothing answered there (not running, its web server not on, another port).
    Missing,
    // It wants a user and password, or other ones.
    Auth,
    // Something answered that is not LHM's tree.
    Strange,
    Found,
}

impl Status {
    fn name(self) -> &'static str {
        match self {
            Status::Off => "off",
            Status::Unread => "unread",
            Status::Missing => "missing",
            Status::Auth => "auth",
            Status::Strange => "strange",
            Status::Found => "found",
        }
    }
}

// What the settings show (lhm::view): kept by the monitor's thread.
static VIEW: Lock<Value> = Lock::new("lhm::VIEW", Value::Null);
static CHANGED: AtomicBool = AtomicBool::new(false);

// { status, version, port, sensors: [{ id, name, hardware, hardwareId,
// kind, reading, on }] }, or null before the monitor first read it.
pub fn view() -> Value {
    VIEW.lock().unwrap().clone()
}

// Whether that changed since last asked (the open settings then drawn again).
pub fn take_changed() -> bool {
    CHANGED.swap(false, Ordering::SeqCst)
}

pub struct Lhm {
    agent: Option<ureq::Agent>,
    asked: Option<Instant>,
    // What it was last asked with (port, user, a password, which one, and
    // ask_now's count): any of them changed, it is asked again at once.
    with: Option<(u16, String, bool, u64, u64)>,
    password: String,
    status: Status,
    fails: u8,
    version: Option<String>,
    sensors: Vec<Sensor>,
}

impl Lhm {
    pub fn new() -> Lhm {
        Lhm { agent: None, asked: None, with: None, password: String::new(), status: Status::Unread, fails: 0, version: None, sensors: Vec::new() }
    }

    // Its web server answered (and is kept as answering for one more try).
    pub fn found(&self) -> bool {
        self.status == Status::Found
    }

    // Asked when it is time, or at once when how to ask changed.
    pub fn turn(&mut self, conf: &Conf, quiet: bool) {
        if !conf.on {
            if self.status != Status::Off {
                *self = Lhm { agent: self.agent.take(), status: Status::Off, ..Lhm::new() };
            }
            return self.publish(conf);
        }
        let with = (conf.port, conf.user.clone(), conf.pass, SECRET_REV.load(Ordering::SeqCst), ASKED_AGAIN.load(Ordering::SeqCst));
        let every = match self.status {
            Status::Found if quiet => EVERY_QUIET,
            Status::Found => EVERY,
            Status::Off | Status::Unread => Duration::ZERO,
            _ => AGAIN,
        };
        if self.with.as_ref() == Some(&with) && self.asked.is_some_and(|t| t.elapsed() < every) {
            return self.publish(conf);
        }
        if self.with.as_ref() != Some(&with) {
            self.password = if conf.pass { crate::mail::secret::read_named(SECRET).unwrap_or_default() } else { String::new() };
            self.with = Some(with);
        }
        self.asked = Some(Instant::now());
        match self.ask(conf) {
            Ok((version, sensors)) => {
                (self.status, self.fails, self.version, self.sensors) = (Status::Found, 0, version, sensors);
            }
            Err(status) => {
                self.fails = self.fails.saturating_add(1);
                // Found a moment ago: its last kept for one more try.
                if self.status != Status::Found || self.fails >= 2 {
                    (self.status, self.version) = (status, None);
                    self.sensors.clear();
                }
            }
        }
        self.publish(conf)
    }

    fn ask(&mut self, conf: &Conf) -> Result<(Option<String>, Vec<Sensor>), Status> {
        // Never through a proxy: it is this computer's.
        let agent = self.agent.get_or_insert_with(|| ureq::Agent::config_builder().timeout_global(Some(TIMEOUT)).proxy(None).build().new_agent());
        let mut asking = agent.get(&format!("http://127.0.0.1:{}/data.json", conf.port));
        if !conf.user.is_empty() || !self.password.is_empty() {
            use base64::Engine;
            let both = base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", conf.user, self.password));
            asking = asking.header("authorization", &format!("Basic {both}"));
        }
        match asking.call() {
            Ok(mut answer) => answer.body_mut().read_json::<Value>().ok().as_ref().and_then(parse).ok_or(Status::Strange),
            Err(ureq::Error::StatusCode(401 | 403)) => Err(Status::Auth),
            Err(ureq::Error::StatusCode(_)) => Err(Status::Strange),
            Err(_) => Err(Status::Missing),
        }
    }

    // The ones picked, or the defaults.
    fn picked(&self, conf: &Conf) -> Vec<String> {
        conf.sensors.clone().unwrap_or_else(|| defaults(&self.sensors))
    }

    // The monitor's parts for the sensors picked that have a value, in the
    // tree's order: { icon, kind, text }.
    pub fn parts(&self, conf: &Conf) -> Vec<Value> {
        if !conf.on || self.status != Status::Found {
            return Vec::new();
        }
        let picked = self.picked(conf);
        self.sensors
            .iter()
            .filter(|s| picked.contains(&s.id))
            .filter_map(|s| s.value.map(|v| json!({ "icon": icon(s), "kind": s.kind.name(), "text": short(s.kind, v) })))
            .collect()
    }

    // What the settings show of it (view).
    fn view_of(&self, conf: &Conf) -> Value {
        let picked = self.picked(conf);
        let sensors: Vec<Value> = self
            .sensors
            .iter()
            .map(|s| json!({ "id": s.id, "name": s.name, "hardware": s.hardware, "hardwareId": s.hardware_id, "kind": s.kind.name(), "reading": long(s.kind, s.value), "on": picked.contains(&s.id) }))
            .collect();
        json!({ "status": self.status.name(), "version": self.version, "port": conf.port, "sensors": sensors })
    }

    fn publish(&self, conf: &Conf) {
        let view = self.view_of(conf);
        let mut now = VIEW.lock().unwrap();
        if *now != view {
            *now = view;
            CHANGED.store(true, Ordering::SeqCst);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Value {
        serde_json::from_str(include_str!("../../test/fixtures/lhm/data.json")).unwrap()
    }

    #[test]
    fn numbers_are_read_out_of_lhms_words() {
        assert_eq!(number_in("62.4 °C"), Some(62.4));
        assert_eq!(number_in("62,4 °C"), Some(62.4));
        assert_eq!(number_in("1180 RPM"), Some(1180.0));
        assert_eq!(number_in("-5.0 °C"), Some(-5.0));
        assert_eq!(number_in("3120.0 MB"), Some(3120.0));
        assert_eq!(number_in("12."), Some(12.0));
        assert_eq!(number_in("-"), None);
        assert_eq!(number_in(""), None);
    }

    // The tree from LHM built since March 2026 (numbers, its version): the
    // kinds shown, each with its hardware; none without a value.
    #[test]
    fn the_tree_is_read_by_sensor_id_and_type() {
        let (version, sensors) = parse(&fixture()).unwrap();
        assert_eq!(version.as_deref(), Some("0.9.6"));
        let cpu = sensors.iter().find(|s| s.id == "/amdcpu/0/temperature/2").unwrap();
        assert_eq!((cpu.name.as_str(), cpu.kind, cpu.hardware.as_str(), cpu.hardware_id.as_str(), cpu.value), ("Core (Tctl/Tdie)", Kind::Temperature, "AMD Ryzen 9 7950X", "/amdcpu/0", Some(62.375)));
        // A Super I/O chip under the motherboard is its own hardware.
        let board = sensors.iter().find(|s| s.id == "/lpc/nct6799d/0/temperature/2").unwrap();
        assert_eq!((board.hardware.as_str(), board.hardware_id.as_str()), ("Nuvoton NCT6799D", "/lpc/nct6799d/0"));
        assert_eq!(sensors.iter().find(|s| s.id == "/lpc/nct6799d/0/fan/2").unwrap().value, None);
        // Voltages, controls and throughput are not shown.
        assert!(!sensors.iter().any(|s| s.id.contains("/voltage/") || s.id.contains("/control/") || s.id.contains("/throughput/")));
        assert_eq!(sensors.iter().find(|s| s.id == "/gpu-nvidia/0/smalldata/1").unwrap().kind, Kind::Data);
        // Not LHM's.
        assert_eq!(parse(&json!({ "error": "no" })), None);
        assert_eq!(parse(&json!([1, 2])), None);
    }

    // LHM 0.9.6 as released: its Raw* values words too, no version; and
    // 0.9.4 in German: no Raw* values, no HardwareId, a comma.
    #[test]
    fn older_lhms_are_read_from_their_words() {
        let sensor = |raw: Value, value: &str| json!({ "Text": "Sensor", "Children": [{ "Text": "PC", "Children": [{ "Text": "Intel Core i7-12700H", "Children": [{ "Text": "Temperatures", "Children": [
            { "Text": "CPU Package", "SensorId": "/intelcpu/0/temperature/12", "Type": "Temperature", "Value": value, "RawValue": raw, "Children": [] }
        ] }] }] }] });
        let read = |tree: Value| parse(&tree).unwrap().1.into_iter().next().unwrap();
        let released = read(sensor(json!("71.0 °C"), "71.0 °C"));
        assert_eq!((released.value, released.hardware.as_str(), released.hardware_id.as_str()), (Some(71.0), "Intel Core i7-12700H", "/intelcpu/0"));
        assert_eq!(read(sensor(Value::Null, "71,5 °C")).value, Some(71.5));
        assert_eq!(read(sensor(json!("-"), "-")).value, None);
        // 0.0 °C: not read (LHM without PawnIO), as none.
        assert_eq!(read(sensor(json!(0.0), "0.0 °C")).value, None);
        assert_eq!(read(sensor(json!("0,0 °C"), "0,0 °C")).value, None);
        assert_eq!(read(sensor(json!(-0.5), "-0.5 °C")).value, Some(-0.5));
        assert_eq!(parse(&sensor(Value::Null, "1")).unwrap().0, None);
    }

    // Without a choice: the CPU's package temperature, the discrete card's
    // temperature and load (NVIDIA's over the Radeon in the processor).
    #[test]
    fn the_defaults_are_the_cpus_heat_and_the_cards() {
        let (_, sensors) = parse(&fixture()).unwrap();
        assert_eq!(defaults(&sensors), ["/amdcpu/0/temperature/2", "/gpu-nvidia/0/temperature/0", "/gpu-nvidia/0/load/0"]);
        let s = |hardware: &str, hardware_id: &str| Sensor { id: format!("{hardware_id}/load/0"), name: "GPU Core".into(), kind: Kind::Load, hardware_id: hardware_id.into(), hardware: hardware.into(), value: None };
        assert_eq!(discrete(&s("AMD Radeon RX 7900 XTX", "/gpu-amd/1")), 1);
        assert_eq!(discrete(&s("Intel(R) Arc(TM) A770 Graphics", "/gpu-intel/1")), 1);
        assert_eq!(discrete(&s("Intel(R) Arc(TM) 140V GPU (16GB)", "/gpu-intel/0")), 0);
        assert_eq!(discrete(&s("AMD Radeon 780M", "/gpu-amd/0")), 0);
        assert_eq!(discrete(&s("Intel(R) UHD Graphics 770", "/gpu-intel/0")), 0);
        // A laptop with its processor's card alone: its CPU only.
        let only = vec![s("AMD Radeon 780M", "/gpu-amd/0")];
        assert!(defaults(&only).is_empty());
    }

    #[test]
    fn values_fit_a_cell() {
        assert_eq!(short(Kind::Temperature, 62.375), "62°");
        assert_eq!(short(Kind::Load, 99.6), "100%");
        assert_eq!([short(Kind::Power, 8.54), short(Kind::Power, 121.63)], ["8.5W", "122W"]);
        assert_eq!(short(Kind::Fan, 1180.4), "1180");
        assert_eq!([short(Kind::Clock, 5341.2), short(Kind::Clock, 800.0)], ["5.3G", "800M"]);
        assert_eq!([short(Kind::Data, 3120.0), short(Kind::Data, 512.0), short(Kind::Data, 16384.0)], ["3.0G", "512M", "16G"]);
        assert_eq!(long(Kind::Temperature, Some(62.375)), "62.4 °C");
        assert_eq!(long(Kind::Fan, None), "—");
    }

    #[test]
    fn its_settings_are_checked() {
        let conf = Conf::from(&json!({ "cpu": true }));
        assert_eq!((conf.on, conf.port, conf.user.as_str(), conf.pass, conf.sensors), (true, 8085, "", false, None));
        let conf = Conf::from(&json!({ "lhm": false, "lhmPort": 9000, "lhmUser": "me", "lhmPass": true, "sensors": ["/amdcpu/0/load/0"] }));
        assert_eq!((conf.on, conf.port, conf.pass, conf.sensors.unwrap().len()), (false, 9000, true, 1));
        assert_eq!(Conf::from(&json!({ "lhmPort": 70000 })).port, 8085);
        assert_eq!(is_setting("lhmPort", &json!(0)), Some(false));
        assert_eq!(is_setting("lhmUser", &json!("a:b")), Some(false));
        assert_eq!(is_setting("sensors", &json!(["/amdcpu/0/temperature/2"])), Some(true));
        assert_eq!(is_setting("sensors", &json!(["amdcpu"])), Some(false));
        assert_eq!(is_setting("sensors", &json!(vec!["/x"; 33])), Some(false));
        assert_eq!(is_setting("cpu", &json!(true)), None);
    }

    // Asked through HTTP, as the monitor does: a little server answering
    // the fixture (with Basic auth, then without), nothing on a port, and a
    // page that is not LHM's.
    #[test]
    fn it_is_asked_and_found_or_not() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let body = include_str!("../../test/fixtures/lhm/data.json");
        std::thread::spawn(move || {
            for stream in listener.incoming().take(3) {
                let mut stream = stream.unwrap();
                let mut asked = [0u8; 2048];
                let n = stream.read(&mut asked).unwrap_or(0);
                let asked = String::from_utf8_lossy(&asked[..n]).to_string();
                // "me:pw"
                let answer = if asked.contains("/data.json") && asked.to_ascii_lowercase().contains("authorization: basic bwu6chc=") {
                    format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
                } else if asked.contains("/data.json") && !asked.to_ascii_lowercase().contains("authorization") {
                    "HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
                } else {
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 9\r\nConnection: close\r\n\r\n<p>no</p>".to_string()
                };
                let _ = stream.write_all(answer.as_bytes());
            }
        });
        let mut lhm = Lhm::new();
        lhm.password = "pw".into();
        let conf = Conf { on: true, port, user: "me".into(), pass: false, sensors: None };
        lhm.with = Some((conf.port, conf.user.clone(), conf.pass, SECRET_REV.load(Ordering::SeqCst), ASKED_AGAIN.load(Ordering::SeqCst)));
        assert!(lhm.ask(&conf).is_ok());
        lhm.password.clear();
        assert_eq!(lhm.ask(&Conf { user: String::new(), ..conf.clone() }).err(), Some(Status::Auth));
        assert_eq!(lhm.ask(&Conf { user: "x".into(), ..conf.clone() }).err(), Some(Status::Strange));
        // Nobody on a port just let go of.
        let free = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let t = Instant::now();
        assert_eq!(lhm.ask(&Conf { port: free, ..conf.clone() }).err(), Some(Status::Missing));
        assert!(t.elapsed() < Duration::from_millis(1500), "{:?}", t.elapsed());
    }

    // Found: the defaults as parts, then the ones picked; off, none, and the
    // view says so.
    #[test]
    fn the_parts_are_the_sensors_picked() {
        let (version, sensors) = parse(&fixture()).unwrap();
        let mut lhm = Lhm { status: Status::Found, version, sensors, ..Lhm::new() };
        let conf = Conf { on: true, port: PORT, user: String::new(), pass: false, sensors: None };
        assert_eq!(
            lhm.parts(&conf),
            [json!({ "icon": "cpu", "kind": "temp", "text": "62°" }), json!({ "icon": "gpu", "kind": "temp", "text": "58°" }), json!({ "icon": "gpu", "kind": "load", "text": "37%" })]
        );
        let picked = Conf { sensors: Some(vec!["/nvme/0/temperature/0".into(), "/lpc/nct6799d/0/fan/1".into(), "/lpc/nct6799d/0/fan/2".into()]), ..conf.clone() };
        // In the tree's order; a fan without a value left out.
        assert_eq!(lhm.parts(&picked), [json!({ "icon": "fan", "kind": "fan", "text": "1180" }), json!({ "icon": "disk", "kind": "temp", "text": "44°" })]);
        assert!(lhm.parts(&Conf { on: false, ..conf.clone() }).is_empty());
        let view = lhm.view_of(&picked);
        assert_eq!((view["status"].clone(), view["version"].clone(), view["port"].clone()), (json!("found"), json!("0.9.6"), json!(8085)));
        let fan = view["sensors"].as_array().unwrap().iter().find(|s| s["id"] == "/lpc/nct6799d/0/fan/1").unwrap();
        assert_eq!((fan["on"].clone(), fan["reading"].clone(), fan["hardware"].clone()), (json!(true), json!("1180 RPM"), json!("Nuvoton NCT6799D")));
        lhm.turn(&Conf { on: false, ..conf.clone() }, false);
        assert_eq!((lhm.status, lhm.view_of(&conf)["status"].clone(), lhm.view_of(&conf)["sensors"].clone()), (Status::Off, json!("off"), json!([])));
    }
}
