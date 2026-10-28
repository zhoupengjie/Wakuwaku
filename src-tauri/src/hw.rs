// The monitor's built-in readings (#18, part A): what Windows itself tells a
// program without the administrator's rights and without installing
// anything: one of the monitor's two sources ("monitorSource": "builtin";
// the other, "lhm", is LibreHardwareMonitor's, lhm.rs; none chosen, LHM's
// while it answers, else this one: Source::in_effect). Measured on the user's
// PC (a Ryzen 9 9955HX, an RTX 4080 Laptop GPU, three NVMe drives) by the
// probes of 2026-10-16, whose numbers the tests keep:
//
//   CPU      its name and cores (the registry, GetLogicalProcessorInformationEx);
//            its actual speed, PDH's \Processor Information(*)\% Processor
//            Performance times its base speed (CallNtPowerInformation's
//            MaxMhz; the registry's ~MHz without it), the average and the
//            fastest core: CallNtPowerInformation's CurrentMhz never shows the
//            boost, and PDH's own Processor Frequency is not the actual speed.
//            Its use is the monitor's own (widgets.rs, Task Manager's counter).
//   memory   GlobalMemoryStatusEx and GetPerformanceInfo, as Task Manager
//            counts: in use = total - available - modified, cached = standby
//            (core, normal, reserve) + modified (PDH's \Memory lists), not
//            SystemCache; committed and its limit.
//   GPU      the display kernel's adapters (D3DKMTEnumAdapters2, gpu.rs kmt)
//            that render and are neither software nor an indirect display
//            (virtual displays pose as the card they sit on: three did on the
//            user's PC). Its temperature (KMTQAITYPE_ADAPTERPERFDATA, in
//            tenths of a degree: NVML's reading, 0.1° finer), fan (only once
//            it said more than 0), the 3D engine's clock (NODEPERFDATA); its
//            power there is 0 on NVIDIA's, so none is shown. Its use from PDH's
//            \GPU Engine(*)\Utilization Percentage, one wildcard read whole
//            (PdhGetFormattedCounterArray, ~0.8 ms for 756 instances there;
//            never enumerated per reading), summed per engine over the
//            processes, the busiest engine (Task Manager's figure); its memory
//            from \GPU Adapter Memory(*), the dedicated total from D3DKMT. No
//            NVML.
//   disks    PDH's \PhysicalDisk(*): read and written a second, the time busy
//            (100 - % Idle Time); the temperature (IOCTL_STORAGE_QUERY_PROPERTY,
//            StorageDeviceTemperatureProperty, the drive opened with access 0:
//            GENERIC_READ is refused) every 60 s, as it sends the drive a
//            command and could wake one asleep; one that fails once (a USB
//            bridge) is not asked again. An NVMe drive's health (its log page
//            2: life used, spare, hours powered on) every 10 minutes at most,
//            and only while the settings are open: never on the island.
//   network, battery: the monitor's own (widgets.rs), listed here too.
//
// Read on the monitor's thread alone, as it reads the rest (widgets::Monitor):
// only what is shown, or everything while the settings are open (their
// sensor list); a group of counters not wanted for ten minutes is closed.
// Without Windows (macOS) there is nothing to read here and nothing changes.
//
// Its picks are the monitor's "builtinSensors" (as LibreHardwareMonitor's
// are its "sensors": each source keeps its own); unset, the discrete card's
// temperature. Ids are LHM's shape, <hardware>/<kind>/<n>: /cpu/0/clock/0,
// /gpu-nvidia/0/temperature/0, /disk/2/read/0.
//
// Off Windows its parsers are the tests' alone.
#![cfg_attr(not(windows), allow(dead_code))]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use serde_json::{json, Value};

use crate::lhm::Kind;

// The setting with the built-in source's picks (in "monitor").
pub const PICKS: &str = "builtinSensors";
// Her exe with this prints one whole reading as JSON and ends (main.rs).
pub const DUMP_FLAG: &str = "--wakuwaku-hw-dump";

// Where the monitor's sensors come from: the top-level setting "monitorSource".
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Source {
    Builtin,
    Lhm,
}

impl Source {
    // The one chosen in the settings ("monitorSource"), if one was.
    pub fn chosen(chosen: &Value) -> Option<Source> {
        match chosen.as_str() {
            Some("lhm") => Some(Source::Lhm),
            Some("builtin") => Some(Source::Builtin),
            _ => None,
        }
    }

    // The one in effect: as chosen; none chosen, decided as she runs and
    // never saved (until the person picks one): LibreHardwareMonitor while
    // its web server answers (lhm.rs found), so someone who runs it keeps
    // its sensors, else the built-in one; the built-in one too where LHM's
    // old switch was turned off (it is then never asked).
    pub fn in_effect(chosen: Option<Source>, lhm_off: bool, lhm_found: bool) -> Source {
        match chosen {
            Some(source) => source,
            None if lhm_found && !lhm_off => Source::Lhm,
            None => Source::Builtin,
        }
    }

    // The same from the settings ("monitor" and "monitorSource").
    pub fn of(monitor: &Value, chosen: &Value, lhm_found: bool) -> Source {
        Source::in_effect(Source::chosen(chosen), monitor["lhm"] == false, lhm_found)
    }

    // Whether LibreHardwareMonitor is asked: while it is the one chosen, or
    // while none is (to know whether it runs) and its old switch is on.
    pub fn asks_lhm(monitor: &Value, chosen: &Value) -> bool {
        match Source::chosen(chosen) {
            Some(source) => source == Source::Lhm,
            None => monitor["lhm"] != false,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Source::Builtin => "builtin",
            Source::Lhm => "lhm",
        }
    }

    // The setting in "monitor" with its picks.
    pub fn picks(self) -> &'static str {
        match self {
            Source::Builtin => PICKS,
            Source::Lhm => "sensors",
        }
    }
}

// The top-level setting, checked (settings.rs is_ok).
pub fn is_source(v: &Value) -> bool {
    matches!(v.as_str(), Some("builtin" | "lhm"))
}

// The "monitor" value may carry these, checked (settings.rs is_ok).
pub fn is_setting(key: &str, v: &Value) -> Option<bool> {
    (key == PICKS).then(|| crate::lhm::is_setting("sensors", v).unwrap_or(false))
}

// The built-in source's settings, from "monitor".
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Conf {
    // The ones picked; None: none (the toggles' CPU, memory and network alone).
    pub picks: Option<Vec<String>>,
}

impl Conf {
    pub fn from(m: &Value) -> Conf {
        Conf { picks: m[Source::Builtin.picks()].as_array().map(|ids| ids.iter().filter_map(Value::as_str).map(str::to_string).collect()) }
    }
}

// What a piece of hardware is: its glyph on the island (widgets.js
// PART_GLYPH) and its group in the settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Class {
    Cpu,
    Gpu,
    Memory,
    Disk,
    Net,
    Battery,
}

impl Class {
    fn name(self) -> &'static str {
        match self {
            Class::Cpu => "cpu",
            Class::Gpu => "gpu",
            Class::Memory => "memory",
            Class::Disk => "disk",
            Class::Net => "net",
            Class::Battery => "battery",
        }
    }
}

// An NVMe drive's health, from its log page 2.
#[derive(Clone, Debug, PartialEq)]
pub struct Health {
    // Of its rated life, in %; may pass 100.
    pub used: u8,
    pub spare: u8,
    pub hours: u64,
    pub unsafe_shutdowns: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Hardware {
    pub id: String,
    pub class: Class,
    // Its model; none for the memory, the network and the battery, named by
    // the page in her language.
    pub name: String,
    // A few words about it, { key, vars } for the page: "16 cores, 32 threads".
    pub about: Value,
    pub health: Option<Health>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    pub id: String,
    pub hardware_id: String,
    // Its name, a key in her strings (i18n.js hw.*).
    pub label: &'static str,
    pub kind: Kind,
    pub value: Option<f64>,
    // What it is out of (a card's memory: 1.2 of 11.7 GB).
    pub of: Option<f64>,
    // Hot from here on (a drive's own warning temperature); else 90.
    pub warn: Option<f64>,
    // On or off by one of the monitor's own switches (cpu, mem, net,
    // battery), not picked.
    pub toggle: Option<&'static str>,
    // In the settings alone, never picked (memory cached, committed).
    pub info: bool,
}

impl Reading {
    fn new(hardware_id: &str, kind_path: &str, n: u32, label: &'static str, kind: Kind, value: Option<f64>) -> Reading {
        Reading { id: format!("{hardware_id}/{kind_path}/{n}"), hardware_id: hardware_id.into(), label, kind, value, of: None, warn: None, toggle: None, info: false }
    }
}

// One turn's hardware and readings, in the order shown.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Round {
    pub hardware: Vec<Hardware>,
    pub readings: Vec<Reading>,
}

// The monitor's own readings (widgets.rs), listed with the hardware's: the
// CPU's use and memory's in %, the network's pace in bytes a second, the
// battery; and whether each is on.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Basics {
    pub cpu: Option<f64>,
    pub memory: Option<f64>,
    pub net: Option<(f64, f64)>,
    pub battery: Option<(u8, bool)>,
}

// What is to be read this turn: everything (the settings are open), or the
// readings picked, by their ids.
pub struct Want<'a> {
    pub all: bool,
    pub ids: &'a [String],
}

impl Want<'_> {
    // That hardware's readings of that kind ("temperature", "load", …).
    pub fn has(&self, hardware_id: &str, kind: &str) -> bool {
        self.all || self.ids.iter().any(|id| id.strip_prefix(hardware_id).and_then(|r| r.strip_prefix('/')).and_then(|r| r.strip_prefix(kind)).is_some_and(|r| r.starts_with('/')))
    }

    // Any of those hardware's.
    pub fn any<'h>(&self, hardware: impl IntoIterator<Item = &'h str>, kind: &str) -> bool {
        hardware.into_iter().any(|h| self.has(h, kind))
    }
}

// A number, whole if it is, else to a tenth.
fn num(v: f64) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{v:.0}")
    } else {
        format!("{v:.1}")
    }
}

// A number of bytes a second in the settings' list: 512 B/s, 8.4 KB/s, 1.2 MB/s.
fn long_rate(v: f64) -> String {
    let short = crate::widgets::rate(v);
    match short.strip_suffix('B') {
        Some(b) => format!("{b} B/s"),
        None => format!("{} {}B/s", &short[..short.len() - 1], &short[short.len() - 1..]),
    }
}

// MB as the settings show it: 512 MB, 1.2 GB.
fn long_mb(v: f64) -> String {
    if v >= 1024.0 {
        format!("{:.1} GB", v / 1024.0)
    } else {
        format!("{v:.0} MB")
    }
}

// A reading in the settings' list: 41.1 °C, 15 %, 2280 MHz, 1.2 / 11.7 GB.
fn reading_text(r: &Reading) -> String {
    let Some(v) = r.value else { return "—".into() };
    match r.kind {
        Kind::Temperature => format!("{} °C", num(v)),
        Kind::Load => format!("{} %", num(v)),
        Kind::Power => format!("{v:.1} W"),
        Kind::Fan => format!("{v:.0} RPM"),
        Kind::Clock => format!("{v:.0} MHz"),
        Kind::Data => match r.of {
            Some(of) if of >= 1024.0 => format!("{:.1} / {:.1} GB", v / 1024.0, of / 1024.0),
            Some(of) => format!("{v:.0} / {of:.0} MB"),
            None => long_mb(v),
        },
        Kind::Read | Kind::Write => long_rate(v),
    }
}

// The settings' view (view()), kept by the monitor's thread.
static VIEW: Mutex<Value> = Mutex::new(Value::Null);
static CHANGED: AtomicBool = AtomicBool::new(false);

// { source: "builtin", hardware: [{ id, class, name, about, health }],
// sensors: [{ id, hardwareId, label, kind, reading, on, toggle, info }] },
// or null before the monitor first read with it (or while the source is
// another).
pub fn view() -> Value {
    VIEW.lock().unwrap().clone()
}

// Whether that changed since last asked (the open settings then drawn again).
pub fn take_changed() -> bool {
    CHANGED.swap(false, Ordering::SeqCst)
}

fn publish(view: Value) {
    let mut now = VIEW.lock().unwrap();
    if *now != view {
        *now = view;
        CHANGED.store(true, Ordering::SeqCst);
    }
}

// Readings from elsewhere that join the built-in ones each turn, under the
// hardware they name: the sensor helper's come in here (#18, part B: the
// CPU's temperature and power through PawnIO, read by an elevated helper and
// handed to her on /hw, sensors::accept). Kept with when they came, so a
// helper that stopped does not leave its last numbers on the island
// (joined_now drops them once stale).
pub static JOINED: Mutex<Vec<Reading>> = Mutex::new(Vec::new());
static JOINED_AT: Mutex<Option<std::time::Instant>> = Mutex::new(None);
// After this long with nothing new, the joined readings are let go (the
// monitor reads every 10 s at most while quiet, the helper posts every 2 s).
const JOINED_FRESH: std::time::Duration = std::time::Duration::from_secs(8);

// The helper's latest readings (sensors::accept): replace what was there,
// and mark when.
pub fn set_joined(readings: Vec<Reading>) {
    *JOINED.lock().unwrap() = readings;
    *JOINED_AT.lock().unwrap() = Some(std::time::Instant::now());
}

// The joined readings if they are fresh, else none.
pub fn joined_now() -> Vec<Reading> {
    let fresh = JOINED_AT.lock().unwrap().is_some_and(|t| t.elapsed() < JOINED_FRESH);
    if fresh {
        JOINED.lock().unwrap().clone()
    } else {
        Vec::new()
    }
}

pub struct Hw {
    reader: Option<imp::Reader>,
    round: Round,
    // The switches (cpu, mem, net, battery) as last read, for the view.
    toggles: [bool; 4],
}

impl Hw {
    pub fn new() -> Hw {
        Hw { reader: None, round: Round::default(), toggles: [true; 4] }
    }

    // A turn: what is wanted read, the view published. `listing`: the
    // settings are open, so everything is read; `toggles`: the monitor's own
    // switches (cpu, mem, net, battery).
    pub fn turn(&mut self, conf: &Conf, listing: bool, basics: &Basics, toggles: [bool; 4]) {
        self.turn_also(conf, listing, &[], basics, toggles);
    }

    // The same, with readings wanted besides those picked (`also`: the
    // monitor widget's card, while it may be looked at, widgets.rs), read
    // but neither on the island nor ticked in the settings.
    pub fn turn_also(&mut self, conf: &Conf, listing: bool, also: &[String], basics: &Basics, toggles: [bool; 4]) {
        let reader = self.reader.get_or_insert_with(imp::Reader::new);
        let picked = conf.picks.clone().unwrap_or_default();
        let wanted: Vec<String> = picked.iter().chain(also).cloned().collect();
        let mut round = reader.read(&Want { all: listing, ids: &wanted }, basics);
        let joined = joined_now();
        for r in joined {
            if let Some(at) = round.readings.iter().rposition(|x| x.hardware_id == r.hardware_id) {
                round.readings.insert(at + 1, r);
            }
        }
        self.round = round;
        self.toggles = toggles;
        publish(self.view_of(&picked));
    }

    // Another source, or the monitor off: what is open closed, the view gone.
    pub fn stop(&mut self) {
        if self.reader.is_some() || self.round != Round::default() {
            (self.reader, self.round) = (None, Round::default());
            publish(Value::Null);
        }
    }

    // None picked: none (the CPU, memory and network toggles alone, the
    // user's 2026-10-16 default).
    fn picked(&mut self, conf: &Conf) -> Vec<String> {
        conf.picks.clone().unwrap_or_default()
    }

    // The monitor's parts for the readings picked that have a value, in the
    // order listed: { icon, kind, text, warn? }.
    pub fn parts(&mut self, conf: &Conf) -> Vec<Value> {
        let picked = self.picked(conf);
        parts_of(&self.round, &picked)
    }

    fn view_of(&self, picked: &[String]) -> Value {
        view_of(&self.round, picked, self.toggles)
    }

    // What the reader saw of the hardware, for the dump.
    pub fn seen(&self) -> Value {
        self.reader.as_ref().map_or(Value::Null, imp::Reader::seen)
    }

    // The last turn's hardware and readings (the monitor widget's card).
    pub fn round(&self) -> &Round {
        &self.round
    }
}

fn class_of(round: &Round, hardware_id: &str) -> Class {
    round.hardware.iter().find(|h| h.id == hardware_id).map_or(Class::Cpu, |h| h.class)
}

fn parts_of(round: &Round, picked: &[String]) -> Vec<Value> {
    round
        .readings
        .iter()
        .filter(|r| r.toggle.is_none() && !r.info && picked.contains(&r.id))
        .filter_map(|r| {
            let v = r.value?;
            let mut part = json!({ "icon": class_of(round, &r.hardware_id).name(), "kind": r.kind.name(), "text": crate::lhm::short(r.kind, v) });
            if let Some(warn) = r.warn {
                part["warn"] = json!(warn);
            }
            Some(part)
        })
        .collect()
}

fn view_of(round: &Round, picked: &[String], toggles: [bool; 4]) -> Value {
    let toggle_on = |t: &str| ["cpu", "mem", "net", "battery"].iter().position(|k| *k == t).is_none_or(|i| toggles[i]);
    let hardware: Vec<Value> = round
        .hardware
        .iter()
        .map(|h| {
            json!({
                "id": h.id,
                "class": h.class.name(),
                "name": h.name,
                "about": h.about,
                "health": h.health.as_ref().map(|x| json!({ "used": x.used, "spare": x.spare, "hours": x.hours, "unsafe": x.unsafe_shutdowns })),
            })
        })
        .collect();
    let sensors: Vec<Value> = round
        .readings
        .iter()
        .map(|r| {
            let on = match r.toggle {
                Some(t) => toggle_on(t),
                None => !r.info && picked.contains(&r.id),
            };
            json!({ "id": r.id, "hardwareId": r.hardware_id, "label": r.label, "kind": r.kind.name(), "reading": reading_text(r), "on": on, "toggle": r.toggle, "info": r.info })
        })
        .collect();
    json!({ "source": "builtin", "hardware": hardware, "sensors": sensors })
}

// The monitor's own readings as rows of the network's and the battery's
// hardware (the CPU's and memory's are put with theirs by the reader).
fn basic_hardware(round: &mut Round, basics: &Basics) {
    round.hardware.push(Hardware { id: "/net/0".into(), class: Class::Net, name: String::new(), about: Value::Null, health: None });
    for (n, (label, kind)) in [("hw.down", Kind::Read), ("hw.up", Kind::Write)].into_iter().enumerate() {
        let value = basics.net.map(|(down, up)| if n == 0 { down } else { up });
        round.readings.push(Reading { toggle: Some("net"), ..Reading::new("/net/0", "rate", n as u32, label, kind, value) });
    }
    if let Some((percent, charging)) = basics.battery {
        let about = if charging { json!({ "key": "hw.charging" }) } else { Value::Null };
        round.hardware.push(Hardware { id: "/battery/0".into(), class: Class::Battery, name: String::new(), about, health: None });
        round.readings.push(Reading { toggle: Some("battery"), ..Reading::new("/battery/0", "level", 0, "hw.charge", Kind::Load, Some(f64::from(percent))) });
    }
}

// --- Parsing what Windows hands back (tested with what it handed back) -----------------

fn u16_at(b: &[u8], at: usize) -> u16 {
    b.get(at..at + 2).map_or(0, |x| u16::from_le_bytes([x[0], x[1]]))
}

fn i16_at(b: &[u8], at: usize) -> i16 {
    u16_at(b, at) as i16
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    b.get(at..at + 4).map_or(0, |x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]))
}

fn u64_at(b: &[u8], at: usize) -> u64 {
    b.get(at..at + 8).map_or(0, |x| u64::from_le_bytes(x.try_into().unwrap()))
}

// Eight hex digits at the start of s.
fn hex8(s: &str) -> Option<u32> {
    s.get(..8).and_then(|h| u32::from_str_radix(h, 16).ok())
}

// The adapter (its LUID, high << 32 | low) in a PDH GPU instance's name:
// "pid_1234_luid_0x00000000_0x000123DC_phys_0_eng_0_engtype_3D",
// "luid_0x00000000_0x000123DC_phys_0"; and the rest after it.
pub fn luid_in(name: &str) -> Option<(u64, &str)> {
    let at = name.find("luid_0x")? + 7;
    let high = hex8(&name[at..])?;
    let rest = name[at + 8..].strip_prefix("_0x")?;
    let low = hex8(rest)?;
    Some(((u64::from(high) << 32) | u64::from(low), &rest[8..]))
}

// A GPU engine's instance: its adapter, and the engine (phys << 16 | eng),
// the same over every process using it.
pub fn engine_in(name: &str) -> Option<(u64, u32)> {
    let (luid, rest) = luid_in(name)?;
    let number = |key: &str| -> Option<u32> {
        let at = rest.find(key)? + key.len();
        let digits: String = rest[at..].chars().take_while(char::is_ascii_digit).collect();
        digits.parse().ok()
    };
    Some((luid, (number("_phys_")? << 16) | number("_eng_")?))
}

// A PhysicalDisk instance's drive: "0 C:" is \\.\PhysicalDrive0; _Total none.
pub fn drive_in(name: &str) -> Option<u32> {
    let digits: String = name.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

// A Processor Information instance, a core's ("0,5": group 0, core 5) or
// not ("_Total", "0,_Total").
pub fn is_core(name: &str) -> bool {
    name.contains(',') && !name.contains("_Total")
}

// The busiest engine's use per adapter: each engine's summed over the
// processes using it (one instance each), the most of them, at 100 at most.
pub fn gpu_loads(instances: impl IntoIterator<Item = ((u64, u32), f64)>) -> Vec<(u64, f64)> {
    let mut per_engine: Vec<((u64, u32), f64)> = Vec::new();
    for (key, v) in instances {
        match per_engine.iter_mut().find(|e| e.0 == key) {
            Some(e) => e.1 += v,
            None => per_engine.push((key, v)),
        }
    }
    let mut per_adapter: Vec<(u64, f64)> = Vec::new();
    for ((luid, _), v) in per_engine {
        match per_adapter.iter_mut().find(|a| a.0 == luid) {
            Some(a) => a.1 = a.1.max(v),
            None => per_adapter.push((luid, v)),
        }
    }
    for a in &mut per_adapter {
        a.1 = a.1.clamp(0.0, 100.0);
    }
    per_adapter
}

// D3DKMT_ADAPTER_PERFDATA (64 bytes): FanRPM at 48, Temperature at 56 in
// tenths of a degree; (°C, RPM), each None at 0.
pub fn adapter_perf(b: &[u8]) -> (Option<f64>, Option<f64>) {
    let (fan, temp) = (u32_at(b, 48), u32_at(b, 56));
    ((temp > 0).then(|| f64::from(temp) / 10.0), (fan > 0).then(|| f64::from(fan)))
}

// D3DKMT_NODE_PERFDATA (56 bytes): Frequency at 8, in Hz; in MHz, None at 0.
pub fn node_mhz(b: &[u8]) -> Option<f64> {
    let hz = u64_at(b, 8);
    (hz > 0).then(|| hz as f64 / 1e6)
}

// STORAGE_TEMPERATURE_DATA_DESCRIPTOR: Warning at 10, InfoCount at 12, the
// infos from 24, 16 bytes each, Temperature at 2 of each; 0x8000 is none
// reported. The first (the drive's composite) and the warning, in °C.
pub fn drive_temperature(b: &[u8]) -> (Option<f64>, Option<f64>) {
    let reported = |t: i16| (t != i16::MIN && t > -100).then_some(f64::from(t));
    let warn = reported(i16_at(b, 10)).filter(|&w| w > 0.0);
    let temp = (u16_at(b, 12) > 0 && b.len() >= 40).then(|| reported(i16_at(b, 26))).flatten();
    (temp, warn)
}

// NVME_HEALTH_INFO_LOG (512 bytes): AvailableSpare at 3, PercentageUsed at
// 5, PowerOnHours at 128 and UnsafeShutdowns at 144 (128-bit, the low half
// read).
pub fn nvme_health(b: &[u8]) -> Option<Health> {
    (b.len() >= 512).then(|| Health { used: b[5], spare: b[3], hours: u64_at(b, 128), unsafe_shutdowns: u64_at(b, 144) })
}

// STORAGE_DEVICE_DESCRIPTOR: the vendor's and product's words at the offsets
// at 12 and 16 (0 for none), the bus at 28 (7 USB, 11 SATA, 17 NVMe).
pub fn drive_model(b: &[u8]) -> (String, u32) {
    let text = |at: usize| -> String {
        let from = u32_at(b, at) as usize;
        if from == 0 || from >= b.len() {
            return String::new();
        }
        let end = b[from..].iter().position(|&c| c == 0).map_or(b.len(), |n| from + n);
        String::from_utf8_lossy(&b[from..end]).trim().to_string()
    };
    let (vendor, product) = (text(12), text(16));
    let model = if vendor.is_empty() || product.starts_with(&vendor) { product } else { format!("{vendor} {product}") };
    (model.split_whitespace().collect::<Vec<_>>().join(" "), u32_at(b, 28))
}

// Task Manager's memory: in use and cached, from the total, what is
// available, the modified list and the standby lists (bytes).
pub fn memory_split(total: u64, available: u64, modified: u64, standby: u64) -> (u64, u64) {
    (total.saturating_sub(available).saturating_sub(modified), standby + modified)
}

// The CPU's speed, MHz: its base speed times its performance (%), the
// average and the fastest core's.
pub fn cpu_mhz(base: f64, total: Option<f64>, cores: &[f64]) -> (Option<f64>, Option<f64>) {
    let at = |p: f64| (p.is_finite() && p > 0.0 && base > 0.0).then(|| base * p / 100.0);
    (total.and_then(at), cores.iter().copied().filter(|p| p.is_finite()).reduce(f64::max).and_then(at))
}

// How surely a card is a discrete one: the hybrid flags (a laptop's two),
// else NVIDIA's always, else by its name as LHM's are told (lhm.rs).
pub fn discrete(flags: u32, vendor: u32, id: &str, name: &str) -> u8 {
    const HYBRID_DISCRETE: u32 = 0x10;
    const HYBRID_INTEGRATED: u32 = 0x20;
    if flags & HYBRID_INTEGRATED != 0 {
        0
    } else if flags & HYBRID_DISCRETE != 0 || vendor == 0x10DE {
        2
    } else {
        crate::lhm::discrete_card(id, name)
    }
}

// The card's id, by its maker as LHM has them: /gpu-nvidia/0, /gpu-amd/0,
// /gpu-intel/0, else /gpu-other/0; n of that maker's, in Windows' order.
pub fn gpu_id(vendor: u32, n: usize) -> String {
    let maker = match vendor {
        0x10DE => "nvidia",
        0x1002 | 0x1022 => "amd",
        0x8086 => "intel",
        _ => "other",
    };
    format!("/gpu-{maker}/{n}")
}

// --- PDH, Windows' performance counters ---------------------------------------------------

#[cfg(windows)]
pub mod pdh {
    // pdh.dll, loaded from System32 the first time a counter is opened, not
    // at her start, as the monitor is off by default; None should it or one
    // of its functions be missing.
    type OpenQuery = unsafe extern "system" fn(source: *const u16, user: usize, query: *mut isize) -> u32;
    type AddCounter = unsafe extern "system" fn(query: isize, path: *const u16, user: usize, counter: *mut isize) -> u32;
    type Collect = unsafe extern "system" fn(query: isize) -> u32;
    type Formatted = unsafe extern "system" fn(counter: isize, format: u32, kind: *mut u32, value: *mut CounterValue) -> u32;
    type FormattedArray = unsafe extern "system" fn(counter: isize, format: u32, size: *mut u32, count: *mut u32, items: *mut u8) -> u32;
    type CloseQuery = unsafe extern "system" fn(query: isize) -> u32;

    pub struct Pdh {
        open: OpenQuery,
        add: AddCounter,
        collect: Collect,
        formatted: Formatted,
        array: FormattedArray,
        close: CloseQuery,
    }

    // PDH_FMT_COUNTERVALUE: its status, then the value (a union of 8 bytes).
    #[repr(C)]
    #[derive(Default)]
    pub struct CounterValue {
        status: u32,
        value: f64,
    }

    pub const DOUBLE: u32 = 0x200;
    pub const LARGE: u32 = 0x400;
    // Past 100: a performance of 141%.
    pub const NOCAP100: u32 = 0x8000;
    const MORE_DATA: u32 = 0x800007D2;

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryExW(name: *const u16, file: isize, flags: u32) -> isize;
        fn GetProcAddress(module: isize, name: *const u8) -> *const ();
    }

    // One of System32's libraries, and its functions by name.
    pub fn system_library(name: &str) -> Option<impl Fn(&std::ffi::CStr) -> Option<*const ()>> {
        const LOAD_LIBRARY_SEARCH_SYSTEM32: u32 = 0x800;
        let wide: Vec<u16> = name.encode_utf16().chain([0]).collect();
        // SAFETY: Windows' own library, from System32 alone.
        let module = unsafe { LoadLibraryExW(wide.as_ptr(), 0, LOAD_LIBRARY_SEARCH_SYSTEM32) };
        // SAFETY: a module loaded and never let go; the name ends in 0.
        (module != 0).then_some(move |f: &std::ffi::CStr| Some(unsafe { GetProcAddress(module, f.as_ptr() as *const u8) }).filter(|f| !f.is_null()))
    }

    fn pdh() -> Option<&'static Pdh> {
        static PDH: std::sync::OnceLock<Option<Pdh>> = std::sync::OnceLock::new();
        PDH.get_or_init(|| {
            let f = system_library("pdh.dll")?;
            // SAFETY: each export by its name, cast to the shape PDH documents for it.
            unsafe {
                Some(Pdh {
                    open: std::mem::transmute::<*const (), OpenQuery>(f(c"PdhOpenQueryW")?),
                    add: std::mem::transmute::<*const (), AddCounter>(f(c"PdhAddEnglishCounterW")?),
                    collect: std::mem::transmute::<*const (), Collect>(f(c"PdhCollectQueryData")?),
                    formatted: std::mem::transmute::<*const (), Formatted>(f(c"PdhGetFormattedCounterValue")?),
                    array: std::mem::transmute::<*const (), FormattedArray>(f(c"PdhGetFormattedCounterArrayW")?),
                    close: std::mem::transmute::<*const (), CloseQuery>(f(c"PdhCloseQuery")?),
                })
            }
        })
        .as_ref()
    }

    // A query of Windows' counters by their English paths (the same whatever
    // Windows' language): opened once (the counters' names looked up the
    // first time: ~0.3 s on this thread, ~4 MB that PDH keeps), collected
    // each turn. A wildcard path ("\GPU Engine(*)\…") is one counter with
    // every instance there is at each collection.
    pub struct Query {
        pdh: &'static Pdh,
        query: isize,
        counters: Vec<isize>,
    }

    impl Query {
        // None unless every one of them is there. Its first sample taken: a
        // rate needs two.
        pub fn open(paths: &[&str]) -> Option<Query> {
            let pdh = pdh()?;
            let mut query = 0isize;
            // SAFETY: PDH fills our own handle.
            if unsafe { (pdh.open)(std::ptr::null(), 0, &mut query) } != 0 {
                return None;
            }
            let mut q = Query { pdh, query, counters: Vec::new() };
            for path in paths {
                let path: Vec<u16> = path.encode_utf16().chain([0]).collect();
                let mut counter = 0isize;
                // SAFETY: our own query; the path ends in 0. A query whose
                // counter would not be added is closed again (dropped).
                if unsafe { (pdh.add)(query, path.as_ptr(), 0, &mut counter) } != 0 {
                    return None;
                }
                q.counters.push(counter);
            }
            q.collect();
            Some(q)
        }

        pub fn collect(&mut self) -> bool {
            // SAFETY: our own query, open till dropped.
            unsafe { (self.pdh.collect)(self.query) == 0 }
        }

        // Counter i's value as last collected (DOUBLE, with NOCAP100 or not;
        // LARGE, given as a float), or None when Windows has none to give
        // (too soon after the first sample, or the counter failed).
        pub fn value(&self, i: usize, format: u32) -> Option<f64> {
            let (mut kind, mut v) = (0u32, CounterValue::default());
            // SAFETY: our own counter; the value is written into our own struct.
            let ok = unsafe { (self.pdh.formatted)(*self.counters.get(i)?, format, &mut kind, &mut v) == 0 };
            // PDH_CSTATUS_VALID_DATA or PDH_CSTATUS_NEW_DATA.
            (ok && v.status <= 1).then(|| if format & LARGE != 0 { v.value.to_bits() as i64 as f64 } else { v.value })
        }

        // Each instance of counter i (a wildcard's) with its value, as last
        // collected, read into `buf` (kept by the caller, so it is not
        // allocated each turn): its name (UTF-16, no 0) and value.
        pub fn each(&self, i: usize, format: u32, buf: &mut Vec<u64>, mut f: impl FnMut(&[u16], f64)) {
            let Some(&counter) = self.counters.get(i) else { return };
            for _ in 0..3 {
                let mut size = (buf.len() * 8) as u32;
                let mut count = 0u32;
                // SAFETY: a buffer of `size` bytes, 8-aligned; PDH writes at
                // most that, else says how much it needs.
                let status = unsafe { (self.pdh.array)(counter, format, &mut size, &mut count, if buf.is_empty() { std::ptr::null_mut() } else { buf.as_mut_ptr() as *mut u8 }) };
                if status == MORE_DATA {
                    buf.resize(size as usize / 8 + 512, 0);
                    continue;
                }
                if status != 0 {
                    return;
                }
                // PDH_FMT_COUNTERVALUE_ITEM_W, 24 bytes: the name's pointer,
                // then the value's status at 8 and the value at 16.
                for n in 0..count as usize {
                    let item = &buf[n * 3..n * 3 + 3];
                    if item[1] as u32 > 1 {
                        continue;
                    }
                    let name = item[0] as *const u16;
                    if name.is_null() {
                        continue;
                    }
                    // SAFETY: the name PDH wrote into our buffer, ending in 0.
                    let name = unsafe {
                        let mut len = 0;
                        while *name.add(len) != 0 {
                            len += 1;
                        }
                        std::slice::from_raw_parts(name, len)
                    };
                    let v = if format & LARGE != 0 { item[2] as i64 as f64 } else { f64::from_bits(item[2]) };
                    f(name, v);
                }
                return;
            }
        }
    }

    impl Drop for Query {
        fn drop(&mut self) {
            // SAFETY: our own query, closed once.
            unsafe { (self.pdh.close)(self.query) };
        }
    }
}

// --- Reading it, on Windows ------------------------------------------------------------------

#[cfg(windows)]
mod imp {
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    use serde_json::{json, Value};

    use super::pdh::{self, Query};
    use super::*;
    use crate::gpu::kmt;

    // A rate is not read from a collection this long after the last (it
    // would be the average since then).
    const STALE: Duration = Duration::from_secs(30);
    // A group of counters not wanted this long is closed.
    const REST: Duration = Duration::from_secs(600);
    const DRIVES_EVERY: Duration = Duration::from_secs(60);
    const HEALTH_EVERY: Duration = Duration::from_secs(600);

    // A group of counters, opened when first wanted.
    struct Counters {
        paths: &'static [&'static str],
        query: Option<Query>,
        tried: bool,
        collected: Option<Instant>,
        wanted: Option<Instant>,
    }

    impl Counters {
        const fn new(paths: &'static [&'static str]) -> Counters {
            Counters { paths, query: None, tried: false, collected: None, wanted: None }
        }

        // Collected now: the query, when its values are to be read (a rate's
        // only from a collection soon after the one before).
        fn collect(&mut self, rates: bool) -> Option<&Query> {
            let now = Instant::now();
            self.wanted = Some(now);
            if self.query.is_none() {
                if self.tried {
                    return None;
                }
                self.tried = true;
                self.query = Query::open(self.paths);
                self.collected = Some(now);
                return if rates { None } else { self.query.as_ref() };
            }
            let fresh = self.collected.is_some_and(|t| now.duration_since(t) < STALE);
            let q = self.query.as_mut()?;
            let ok = q.collect();
            self.collected = Some(now);
            (ok && (fresh || !rates)).then_some(&*q)
        }

        fn rest(&mut self) {
            if self.query.is_some() && self.wanted.is_none_or(|t| t.elapsed() > REST) {
                (self.query, self.tried, self.collected) = (None, false, None);
            }
        }
    }

    struct Cpu {
        name: String,
        cores: u32,
        threads: u32,
        base_mhz: f64,
    }

    struct Gpu {
        handle: u32,
        luid: u64,
        id: String,
        name: String,
        flags: u32,
        vendor: u32,
        // Dedicated memory, MB.
        vram: Option<f64>,
        node3d: u32,
        temp: bool,
        fan: bool,
        clock: bool,
    }

    struct Drive {
        number: u32,
        model: String,
        bus: u32,
        // Its temperature read once at least; one that failed is not asked again.
        temp_ok: bool,
        temp_failed: bool,
        temp: Option<f64>,
        warn: Option<f64>,
        health: Option<Health>,
        health_failed: bool,
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

    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalMemoryStatusEx(status: *mut MemoryStatus) -> i32;
        fn K32GetPerformanceInfo(info: *mut u8, size: u32) -> i32;
        fn GetLogicalProcessorInformationEx(relation: i32, buf: *mut u8, len: *mut u32) -> i32;
        fn GetActiveProcessorCount(group: u16) -> u32;
        fn CreateFileW(name: *const u16, access: u32, share: u32, security: *mut std::ffi::c_void, disposition: u32, flags: u32, template: isize) -> isize;
        fn DeviceIoControl(device: isize, code: u32, input: *const u8, input_size: u32, output: *mut u8, output_size: u32, returned: *mut u32, overlapped: *mut u8) -> i32;
        fn CloseHandle(handle: isize) -> i32;
    }

    const IOCTL_STORAGE_QUERY_PROPERTY: u32 = 0x002D_1400;

    // Each drive's bytes read and written a second, and % Idle Time (PDH).
    type DrivesIo = Vec<(u32, [f64; 3])>;

    // A u64 buffer's bytes (aligned for what Windows writes into it).
    fn bytes(b: &[u64]) -> &[u8] {
        // SAFETY: u8 has no alignment, and the length is the same memory.
        unsafe { std::slice::from_raw_parts(b.as_ptr() as *const u8, b.len() * 8) }
    }

    fn bytes_mut(b: &mut [u64]) -> &mut [u8] {
        // SAFETY: as above, the slice borrowed mutably once.
        unsafe { std::slice::from_raw_parts_mut(b.as_mut_ptr() as *mut u8, b.len() * 8) }
    }

    // A drive opened with access 0 (enough for these questions; GENERIC_READ
    // is refused without the administrator's rights), closed when dropped.
    struct Opened(isize);

    impl Opened {
        fn drive(number: u32) -> Option<Opened> {
            let path: Vec<u16> = format!(r"\\.\PhysicalDrive{number}").encode_utf16().chain([0]).collect();
            // SAFETY: the path ends in 0; FILE_SHARE_READ | FILE_SHARE_WRITE, OPEN_EXISTING.
            let h = unsafe { CreateFileW(path.as_ptr(), 0, 3, std::ptr::null_mut(), 3, 0, 0) };
            // Made only if opened: one made anyway closes what it holds as it goes.
            (h != -1 && h != 0).then(|| Opened(h))
        }

        // IOCTL_STORAGE_QUERY_PROPERTY with `input` (a STORAGE_PROPERTY_QUERY
        // and what follows it), into `out`.
        fn query(&self, input: &[u8], out: &mut [u64]) -> bool {
            let mut got = 0u32;
            let out = bytes_mut(out);
            // SAFETY: our own handle and buffers, their sizes given.
            unsafe { DeviceIoControl(self.0, IOCTL_STORAGE_QUERY_PROPERTY, input.as_ptr(), input.len() as u32, out.as_mut_ptr(), out.len() as u32, &mut got, std::ptr::null_mut()) != 0 }
        }

        // A standard property: StorageDeviceProperty (0), StorageDeviceTemperatureProperty (52).
        fn property(&self, id: u32, out: &mut [u64]) -> bool {
            let mut q = [0u8; 12];
            q[..4].copy_from_slice(&id.to_le_bytes());
            self.query(&q, out)
        }

        // An NVMe log page (StorageDeviceProtocolSpecificProperty, 50; the
        // protocol NVMe, 3; a log page, 2): page `page`, `len` bytes of it.
        fn nvme_log(&self, page: u32, len: usize) -> Option<Vec<u8>> {
            // STORAGE_PROPERTY_QUERY's id and type (8), then STORAGE_PROTOCOL_SPECIFIC_DATA
            // (40): ProtocolType, DataType, the request's value, its sub-value,
            // the data's offset (40: right after it) and length.
            let mut q = vec![0u64; (8 + 40 + len).div_ceil(8)];
            {
                let b = bytes_mut(&mut q);
                for (at, v) in [(0, 50u32), (8, 3), (12, 2), (16, page), (24, 40), (28, len as u32)] {
                    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
                }
            }
            let input = bytes(&q).to_vec();
            if !self.query(&input, &mut q) {
                return None;
            }
            // STORAGE_PROTOCOL_DATA_DESCRIPTOR: its version and size, then the
            // same 40 bytes, the data's offset at 16 of them and length at 20.
            let b = bytes(&q);
            let (from, n) = (8 + u32_at(b, 24) as usize, u32_at(b, 28) as usize);
            b.get(from..from + n).map(<[u8]>::to_vec)
        }
    }

    impl Drop for Opened {
        fn drop(&mut self) {
            // SAFETY: our own handle, closed once.
            unsafe { CloseHandle(self.0) };
        }
    }

    pub struct Reader {
        cpu: Option<Cpu>,
        gpus: Option<Vec<Gpu>>,
        // Every adapter seen (kept or not, and why), for the dump.
        adapters_seen: Vec<Value>,
        drives: Vec<Drive>,
        drives_at: Option<Instant>,
        health_at: Option<Instant>,
        cpu_q: Counters,
        mem_q: Counters,
        engines_q: Counters,
        vram_q: Counters,
        disks_q: Counters,
        // GPU engine instances' names, parsed once each.
        engines: HashMap<Vec<u16>, Option<(u64, u32)>>,
        buf: Vec<u64>,
        // What the last turns read, kept for a turn whose rates are not yet
        // to be had (a group just opened), if read lately.
        last: HashMap<String, (f64, Instant)>,
        // Each group's cost the last time it was read, µs (for the dump).
        costs: Vec<(&'static str, u128)>,
    }

    impl Reader {
        pub fn new() -> Reader {
            Reader {
                cpu: None,
                gpus: None,
                adapters_seen: Vec::new(),
                drives: Vec::new(),
                drives_at: None,
                health_at: None,
                cpu_q: Counters::new(&[r"\Processor Information(*)\% Processor Performance"]),
                mem_q: Counters::new(&[r"\Memory\Modified Page List Bytes", r"\Memory\Standby Cache Core Bytes", r"\Memory\Standby Cache Normal Priority Bytes", r"\Memory\Standby Cache Reserve Bytes"]),
                engines_q: Counters::new(&[r"\GPU Engine(*)\Utilization Percentage"]),
                vram_q: Counters::new(&[r"\GPU Adapter Memory(*)\Dedicated Usage", r"\GPU Adapter Memory(*)\Shared Usage"]),
                disks_q: Counters::new(&[r"\PhysicalDisk(*)\Disk Read Bytes/sec", r"\PhysicalDisk(*)\Disk Write Bytes/sec", r"\PhysicalDisk(*)\% Idle Time"]),
                engines: HashMap::new(),
                buf: Vec::new(),
                last: HashMap::new(),
                costs: Vec::new(),
            }
        }

        fn cpu(&mut self) -> &Cpu {
            self.cpu.get_or_insert_with(read_cpu)
        }

        fn gpus(&mut self) -> &mut Vec<Gpu> {
            if self.gpus.is_none() {
                let (gpus, seen) = read_gpus();
                (self.gpus, self.adapters_seen) = (Some(gpus), seen);
            }
            self.gpus.as_mut().unwrap()
        }

        // The value read now; else, with none to be had yet (`fresh` false:
        // a group just opened), the last one, if read in the last 30 s.
        fn keep(&mut self, id: &str, now: Option<f64>, fresh: bool) -> Option<f64> {
            match now {
                Some(v) => {
                    self.last.insert(id.to_string(), (v, Instant::now()));
                    Some(v)
                }
                None if !fresh => self.last.get(id).filter(|l| l.1.elapsed() < STALE).map(|l| l.0),
                None => {
                    self.last.remove(id);
                    None
                }
            }
        }

        fn timed<T>(&mut self, name: &'static str, f: impl FnOnce(&mut Self) -> T) -> T {
            let t = Instant::now();
            let out = f(self);
            let us = t.elapsed().as_micros();
            match self.costs.iter_mut().find(|c| c.0 == name) {
                Some(c) => c.1 = us,
                None => self.costs.push((name, us)),
            }
            out
        }

        pub fn read(&mut self, want: &Want, basics: &Basics) -> Round {
            let mut round = Round::default();
            self.read_cpu(want, basics, &mut round);
            self.read_gpus(want, &mut round);
            self.read_memory(want, basics, &mut round);
            self.read_drives(want, &mut round);
            basic_hardware(&mut round, basics);
            for c in [&mut self.cpu_q, &mut self.mem_q, &mut self.engines_q, &mut self.vram_q, &mut self.disks_q] {
                c.rest();
            }
            round
        }

        fn read_cpu(&mut self, want: &Want, basics: &Basics, round: &mut Round) {
            let id = "/cpu/0";
            let (name, cores, threads, base) = {
                let cpu = self.cpu();
                (cpu.name.clone(), cpu.cores, cpu.threads, cpu.base_mhz)
            };
            let (mut avg, mut fastest, mut fresh) = (None, None, false);
            if want.has(id, "clock") {
                (avg, fastest, fresh) = self.timed("cpu", |r| {
                    let buf = &mut r.buf;
                    let Some(q) = r.cpu_q.collect(true) else { return (None, None, false) };
                    let (mut total, mut each) = (None, Vec::new());
                    q.each(0, pdh::DOUBLE | pdh::NOCAP100, buf, |name, v| {
                        let name = String::from_utf16_lossy(name);
                        if name == "_Total" {
                            total = Some(v);
                        } else if is_core(&name) {
                            each.push(v);
                        }
                    });
                    let (avg, fastest) = cpu_mhz(base, total, &each);
                    (avg, fastest, true)
                });
            }
            let about = json!({ "key": "hw.cores", "vars": { "cores": cores, "threads": threads } });
            round.hardware.push(Hardware { id: id.into(), class: Class::Cpu, name, about, health: None });
            round.readings.push(Reading { toggle: Some("cpu"), ..Reading::new(id, "load", 0, "hw.load", Kind::Load, basics.cpu) });
            let avg = self.keep("/cpu/0/clock/0", avg, fresh || !want.has(id, "clock"));
            let fastest = self.keep("/cpu/0/clock/1", fastest, fresh || !want.has(id, "clock"));
            round.readings.push(Reading::new(id, "clock", 0, "hw.clock", Kind::Clock, avg));
            round.readings.push(Reading::new(id, "clock", 1, "hw.fastest", Kind::Clock, fastest));
        }

        fn read_gpus(&mut self, want: &Want, round: &mut Round) {
            let ids: Vec<String> = self.gpus().iter().map(|g| g.id.clone()).collect();
            if ids.is_empty() {
                return;
            }
            // Use and memory, every card's at once (PDH).
            let loads: Option<Option<Vec<(u64, f64)>>> = want.any(ids.iter().map(String::as_str), "load").then(|| {
                self.timed("gpuLoad", |r| {
                    let (buf, engines) = (&mut r.buf, &mut r.engines);
                    let q = r.engines_q.collect(true)?;
                    let mut seen = Vec::new();
                    q.each(0, pdh::DOUBLE | pdh::NOCAP100, buf, |name, v| {
                        let key = match engines.get(name) {
                            Some(k) => *k,
                            None => {
                                // Processes come and go, and their names with
                                // them: past a few thousand, begun again.
                                if engines.len() > 4096 {
                                    engines.clear();
                                }
                                let k = engine_in(&String::from_utf16_lossy(name));
                                engines.insert(name.to_vec(), k);
                                k
                            }
                        };
                        if let Some(k) = key {
                            seen.push((k, v));
                        }
                    });
                    Some(gpu_loads(seen))
                })
            });
            let vram: Option<Option<Vec<(u64, f64, f64)>>> = want.any(ids.iter().map(String::as_str), "data").then(|| {
                self.timed("gpuMemory", |r| {
                    let buf = &mut r.buf;
                    let q = r.vram_q.collect(false)?;
                    let mut per: Vec<(u64, f64, f64)> = Vec::new();
                    for (i, dedicated) in [(0, true), (1, false)] {
                        q.each(i, pdh::LARGE, buf, |name, v| {
                            if let Some((luid, _)) = luid_in(&String::from_utf16_lossy(name)) {
                                let mb = v / 1_048_576.0;
                                match per.iter_mut().find(|p| p.0 == luid) {
                                    Some(p) if dedicated => p.1 += mb,
                                    Some(p) => p.2 += mb,
                                    None => per.push((luid, if dedicated { mb } else { 0.0 }, if dedicated { 0.0 } else { mb })),
                                }
                            }
                        });
                    }
                    Some(per)
                })
            });
            let n = self.gpus().len();
            for i in 0..n {
                let (id, name, luid, vram_total, handle, node3d) = {
                    let g = &self.gpus()[i];
                    (g.id.clone(), g.name.clone(), g.luid, g.vram, g.handle, g.node3d)
                };
                // Temperature and fan; the 3D engine's clock (D3DKMT).
                let (mut temp, mut fan, mut clock) = (None, None, None);
                if want.has(&id, "temperature") || want.has(&id, "fan") {
                    let got = self.timed("gpuPerf", |_| {
                        let mut b = [0u64; 8];
                        kmt::query(handle, kmt::ADAPTERPERFDATA, &mut b, 64).then(|| adapter_perf(bytes(&b)))
                    });
                    if let Some((t, f)) = got {
                        (temp, fan) = (t, f);
                        let g = &mut self.gpus()[i];
                        g.temp |= t.is_some();
                        g.fan |= f.is_some();
                    }
                }
                if want.has(&id, "clock") {
                    clock = self.timed("gpuClock", |_| {
                        let mut b = [0u64; 7];
                        bytes_mut(&mut b)[..4].copy_from_slice(&node3d.to_le_bytes());
                        if kmt::query(handle, kmt::NODEPERFDATA, &mut b, 56) {
                            node_mhz(bytes(&b))
                        } else {
                            None
                        }
                    });
                    self.gpus()[i].clock |= clock.is_some();
                }
                let load = loads.as_ref().and_then(Option::as_ref).map(|l| l.iter().find(|x| x.0 == luid).map_or(0.0, |x| x.1));
                let memory = vram.as_ref().and_then(Option::as_ref).and_then(|v| v.iter().find(|x| x.0 == luid).map(|x| (x.1, x.2)));
                let fresh_load = loads.as_ref().is_some_and(Option::is_some);
                let load = self.keep(&format!("{id}/load/0"), load, fresh_load || !want.has(&id, "load"));
                let about = vram_total.map_or(Value::Null, |mb| json!({ "key": "hw.vramTotal", "vars": { "gb": format!("{:.1}", mb / 1024.0) } }));
                round.hardware.push(Hardware { id: id.clone(), class: Class::Gpu, name, about, health: None });
                let g = &self.gpus()[i];
                let (has_temp, has_fan, has_clock) = (g.temp, g.fan, g.clock);
                if has_temp {
                    round.readings.push(Reading::new(&id, "temperature", 0, "hw.temp", Kind::Temperature, temp));
                }
                round.readings.push(Reading::new(&id, "load", 0, "hw.load", Kind::Load, load));
                if has_clock || want.all {
                    round.readings.push(Reading::new(&id, "clock", 0, "hw.coreClock", Kind::Clock, clock));
                }
                round.readings.push(Reading { of: vram_total, ..Reading::new(&id, "data", 0, "hw.vram", Kind::Data, memory.map(|m| m.0)) });
                round.readings.push(Reading { info: true, ..Reading::new(&id, "data", 1, "hw.shared", Kind::Data, memory.map(|m| m.1)) });
                if has_fan {
                    round.readings.push(Reading::new(&id, "fan", 0, "hw.fan", Kind::Fan, fan));
                }
            }
        }

        fn read_memory(&mut self, want: &Want, basics: &Basics, round: &mut Round) {
            let id = "/ram/0";
            let detail = want.all.then(|| {
                self.timed("memory", |r| {
                    // SAFETY: structs of the sizes the calls ask for.
                    let (status, perf) = unsafe {
                        let mut s: MemoryStatus = std::mem::zeroed();
                        s.length = std::mem::size_of::<MemoryStatus>() as u32;
                        let mut p = [0u64; 13];
                        bytes_mut(&mut p)[..4].copy_from_slice(&104u32.to_le_bytes());
                        let ok = GlobalMemoryStatusEx(&mut s) != 0;
                        let perf_ok = K32GetPerformanceInfo(p.as_mut_ptr() as *mut u8, 104) != 0;
                        (ok.then_some(s), perf_ok.then_some(p))
                    };
                    let lists = r.mem_q.collect(false).map(|q| (0..4).map(|i| q.value(i, pdh::LARGE).unwrap_or(0.0) as u64).collect::<Vec<_>>());
                    (status, perf, lists)
                })
            });
            let total = detail.as_ref().and_then(|d| d.0.as_ref()).map(|s| s.total_phys);
            let about = total.map_or(Value::Null, |t| json!({ "key": "hw.memTotal", "vars": { "gb": format!("{:.1}", t as f64 / 1_073_741_824.0) } }));
            round.hardware.push(Hardware { id: id.into(), class: Class::Memory, name: String::new(), about, health: None });
            round.readings.push(Reading { toggle: Some("mem"), ..Reading::new(id, "load", 0, "hw.inUseShare", Kind::Load, basics.memory) });
            let mb = |b: u64| b as f64 / 1_048_576.0;
            let (mut in_use, mut available, mut cached, mut committed, mut limit) = (None, None, None, None, None);
            if let Some((Some(s), perf, lists)) = &detail {
                available = Some(mb(s.avail_phys));
                if let Some(l) = lists {
                    let (used, cache) = memory_split(s.total_phys, s.avail_phys, l[0], l[1] + l[2] + l[3]);
                    (in_use, cached) = (Some(mb(used)), Some(mb(cache)));
                }
                if let Some(p) = perf {
                    // PERFORMANCE_INFORMATION: CommitTotal at 8, CommitLimit at
                    // 16, PageSize at 80 (pages, then bytes a page).
                    let b = bytes(p);
                    let page = u64_at(b, 80);
                    (committed, limit) = (Some(mb(u64_at(b, 8) * page)), Some(mb(u64_at(b, 16) * page)));
                }
            }
            for (n, label, value, of) in [(0, "hw.inUse", in_use, total.map(mb)), (1, "hw.available", available, None), (2, "hw.cached", cached, None), (3, "hw.committed", committed, limit)] {
                round.readings.push(Reading { of, info: true, ..Reading::new(id, "data", n, label, Kind::Data, value) });
            }
        }

        fn read_drives(&mut self, want: &Want, round: &mut Round) {
            let any = want.all || want.ids.iter().any(|id| id.starts_with("/disk/"));
            if !any {
                return;
            }
            // Which there are, and their temperature, every minute; at once
            // for the settings when one was never asked.
            let unasked = want.all && self.drives.iter().any(|d| !d.temp_ok && !d.temp_failed);
            if unasked || self.drives_at.is_none_or(|t| t.elapsed() >= DRIVES_EVERY) {
                self.drives_at = Some(Instant::now());
                self.timed("drives", |r| r.look_drives(want));
            }
            // An NVMe drive's health, for the settings alone.
            if want.all && self.health_at.is_none_or(|t| t.elapsed() >= HEALTH_EVERY) {
                self.health_at = Some(Instant::now());
                self.timed("health", |r| {
                    for d in r.drives.iter_mut().filter(|d| d.bus == 17 && !d.health_failed) {
                        let health = Opened::drive(d.number).and_then(|o| o.nvme_log(2, 512)).as_deref().and_then(nvme_health);
                        d.health_failed = health.is_none();
                        d.health = health;
                    }
                });
            }
            let numbers: Vec<String> = self.drives.iter().map(|d| format!("/disk/{}", d.number)).collect();
            let io_wanted = ["load", "read", "write"].iter().any(|k| want.any(numbers.iter().map(String::as_str), k));
            let io: Option<Option<DrivesIo>> = io_wanted.then(|| {
                self.timed("disks", |r| {
                    let buf = &mut r.buf;
                    let q = r.disks_q.collect(true)?;
                    let mut per: Vec<(u32, [f64; 3])> = Vec::new();
                    for i in 0..3 {
                        q.each(i, pdh::DOUBLE, buf, |name, v| {
                            if let Some(n) = drive_in(&String::from_utf16_lossy(name)) {
                                match per.iter_mut().find(|p| p.0 == n) {
                                    Some(p) => p.1[i] = v,
                                    None => {
                                        let mut values = [0.0; 3];
                                        values[i] = v;
                                        per.push((n, values));
                                    }
                                }
                            }
                        });
                    }
                    Some(per)
                })
            });
            let fresh = io.as_ref().is_some_and(Option::is_some);
            for i in 0..self.drives.len() {
                let (number, model, bus, temp_ok, temp, warn, health) = {
                    let d = &self.drives[i];
                    (d.number, d.model.clone(), d.bus, d.temp_ok, d.temp, d.warn, d.health.clone())
                };
                let id = format!("/disk/{number}");
                let bus_name = match bus {
                    7 => "USB",
                    11 => "SATA",
                    17 => "NVMe",
                    8 => "RAID",
                    3 => "ATA",
                    10 => "SAS",
                    _ => "",
                };
                let about = if bus_name.is_empty() { Value::Null } else { json!(bus_name) };
                round.hardware.push(Hardware { id: id.clone(), class: Class::Disk, name: model, about, health: if want.all { health } else { None } });
                if temp_ok {
                    round.readings.push(Reading { warn, ..Reading::new(&id, "temperature", 0, "hw.temp", Kind::Temperature, temp) });
                }
                let values = io.as_ref().and_then(|p| p.as_ref()).and_then(|p| p.iter().find(|x| x.0 == number).map(|x| x.1));
                let busy = values.map(|v| (100.0 - v[2]).clamp(0.0, 100.0));
                let not_wanted = |k: &str| !want.has(&id, k);
                let busy = self.keep(&format!("{id}/load/0"), busy, fresh || not_wanted("load"));
                let read = self.keep(&format!("{id}/read/0"), values.map(|v| v[0]), fresh || not_wanted("read"));
                let write = self.keep(&format!("{id}/write/0"), values.map(|v| v[1]), fresh || not_wanted("write"));
                round.readings.push(Reading::new(&id, "load", 0, "hw.active", Kind::Load, busy));
                round.readings.push(Reading::new(&id, "read", 0, "hw.read", Kind::Read, read));
                round.readings.push(Reading::new(&id, "write", 0, "hw.write", Kind::Write, write));
            }
        }

        // The drives there are (\\.\PhysicalDrive0 to 31), each's model and
        // bus; the temperatures wanted read (every one while the settings are
        // open, so they list those that have one).
        fn look_drives(&mut self, want: &Want) {
            let mut found = Vec::new();
            for number in 0..32u32 {
                let Some(opened) = Opened::drive(number) else { continue };
                let mut b = [0u64; 128];
                if !opened.property(0, &mut b) {
                    continue;
                }
                let (model, bus) = drive_model(bytes(&b));
                let mut drive = match self.drives.iter().position(|d| d.number == number && d.model == model) {
                    Some(at) => self.drives.swap_remove(at),
                    None => Drive { number, model, bus, temp_ok: false, temp_failed: false, temp: None, warn: None, health: None, health_failed: false },
                };
                let id = format!("/disk/{number}");
                if !drive.temp_failed && want.has(&id, "temperature") {
                    let mut t = [0u64; 64];
                    match opened.property(52, &mut t).then(|| drive_temperature(bytes(&t))) {
                        Some((Some(temp), warn)) => (drive.temp_ok, drive.temp, drive.warn) = (true, Some(temp), warn),
                        // Asked and not answered (a USB bridge): never again.
                        _ if !drive.temp_ok => drive.temp_failed = true,
                        _ => drive.temp = None,
                    }
                }
                found.push(drive);
            }
            found.sort_by_key(|d| d.number);
            self.drives = found;
        }

        // What it found, for the dump: the CPU, every adapter (kept or not),
        // the drives, each group's cost the last time.
        pub fn seen(&self) -> Value {
            json!({
                "cpu": self.cpu.as_ref().map(|c| json!({ "name": c.name, "cores": c.cores, "threads": c.threads, "baseMhz": c.base_mhz })),
                "adapters": self.adapters_seen,
                "gpus": self.gpus.iter().flatten().map(|g| json!({ "id": g.id, "name": g.name, "luid": format!("{:#018x}", g.luid), "flags": format!("{:#x}", g.flags), "vendor": format!("{:#06x}", g.vendor), "vramMb": g.vram, "node3d": g.node3d, "discrete": discrete(g.flags, g.vendor, &g.id, &g.name) })).collect::<Vec<_>>(),
                "drives": self.drives.iter().map(|d| json!({ "number": d.number, "model": d.model, "bus": d.bus, "tempRead": d.temp_ok, "tempFailed": d.temp_failed, "warn": d.warn, "healthFailed": d.health_failed })).collect::<Vec<_>>(),
                "costUs": self.costs.iter().map(|c| (c.0.to_string(), json!(c.1 as u64))).collect::<serde_json::Map<_, _>>(),
            })
        }
    }

    impl Drop for Reader {
        fn drop(&mut self) {
            for g in self.gpus.iter().flatten() {
                kmt::close(g.handle);
            }
        }
    }

    // The CPU's name (the registry's), cores and threads
    // (GetLogicalProcessorInformationEx), and base speed.
    fn read_cpu() -> Cpu {
        use winreg::enums::HKEY_LOCAL_MACHINE;
        let key = winreg::RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0").ok();
        let name = key.as_ref().and_then(|k| k.get_value::<String, _>("ProcessorNameString").ok()).unwrap_or_default();
        let registry_mhz = key.as_ref().and_then(|k| k.get_value::<u32, _>("~MHz").ok()).unwrap_or(0);
        let (mut cores, mut threads) = (0u32, 0u32);
        let mut len = 0u32;
        // SAFETY: first the size, then a buffer of that size; each record's
        // size is its own, read within the buffer. RelationProcessorCore (0):
        // a record a core, its group masks from 32 (GroupCount at 30).
        unsafe {
            GetLogicalProcessorInformationEx(0, std::ptr::null_mut(), &mut len);
            let mut buf = vec![0u64; (len as usize).div_ceil(8)];
            if len > 0 && GetLogicalProcessorInformationEx(0, buf.as_mut_ptr() as *mut u8, &mut len) != 0 {
                let b = &bytes(&buf)[..len as usize];
                let mut at = 0;
                while at + 8 <= b.len() {
                    let size = u32_at(b, at + 4) as usize;
                    if size == 0 {
                        break;
                    }
                    if u32_at(b, at) == 0 {
                        cores += 1;
                        for g in 0..u16_at(b, at + 30) as usize {
                            threads += u64_at(b, at + 32 + g * 16).count_ones();
                        }
                    }
                    at += size;
                }
            }
        }
        if threads == 0 {
            // SAFETY: a plain count.
            threads = unsafe { GetActiveProcessorCount(0xFFFF) };
        }
        let base = base_mhz(threads).unwrap_or(f64::from(registry_mhz));
        Cpu { name: name.split_whitespace().collect::<Vec<_>>().join(" "), cores, threads, base_mhz: base }
    }

    // CallNtPowerInformation's ProcessorInformation (powrprof.dll, loaded
    // when asked): the first processor's MaxMhz, Task Manager's base speed.
    fn base_mhz(threads: u32) -> Option<f64> {
        type Power = unsafe extern "system" fn(level: i32, input: *const u8, input_size: u32, output: *mut u8, output_size: u32) -> i32;
        let f = pdh::system_library("powrprof.dll")?;
        // SAFETY: the export by its name, in the shape documented; a buffer
        // for each processor's PROCESSOR_POWER_INFORMATION (24 bytes).
        unsafe {
            let call = std::mem::transmute::<*const (), Power>(f(c"CallNtPowerInformation")?);
            let mut buf = vec![0u64; (threads.max(1) as usize * 24).div_ceil(8)];
            let size = (buf.len() * 8) as u32;
            (call(11, std::ptr::null(), 0, buf.as_mut_ptr() as *mut u8, size) == 0).then(|| f64::from(u32_at(bytes(&buf), 4))).filter(|&m| m > 0.0)
        }
    }

    // The display kernel's adapters: those that render and are neither
    // software nor an indirect display kept open; every one, for the dump.
    fn read_gpus() -> (Vec<Gpu>, Vec<Value>) {
        let (mut gpus, mut seen) = (Vec::new(), Vec::new());
        let mut per_maker: Vec<(u32, usize)> = Vec::new();
        for (handle, luid) in kmt::adapters() {
            let mut b = [0u64; 1];
            let flags = if kmt::query(handle, kmt::ADAPTERTYPE, &mut b, 4) { u32_at(bytes(&b), 0) } else { 0 };
            let name = adapter_name(handle);
            let mut ids = [0u64; 4];
            // D3DKMT_QUERY_DEVICE_IDS: the physical adapter's index, then the vendor at 4.
            let vendor = if kmt::query(handle, kmt::PHYSICALADAPTERDEVICEIDS, &mut ids, 28) { u32_at(bytes(&ids), 4) } else { 0 };
            let kept = crate::gpu::renders(flags);
            seen.push(json!({ "name": name, "luid": format!("{luid:#018x}"), "flags": format!("{flags:#x}"), "vendor": format!("{vendor:#06x}"), "kept": kept }));
            if !kept {
                kmt::close(handle);
                continue;
            }
            let n = match per_maker.iter_mut().find(|m| super::gpu_id(m.0, 0) == super::gpu_id(vendor, 0)) {
                Some(m) => {
                    m.1 += 1;
                    m.1
                }
                None => {
                    per_maker.push((vendor, 0));
                    0
                }
            };
            // D3DKMT_SEGMENTSIZEINFO: the dedicated video memory first.
            let mut seg = [0u64; 3];
            let vram = kmt::query(handle, kmt::GETSEGMENTSIZE, &mut seg, 24).then(|| seg[0] as f64 / 1_048_576.0).filter(|&mb| mb > 0.0);
            // The 3D engine's node (D3DKMT_NODEMETADATA, 78 bytes: the node's
            // ordinal in, its engine type at 4; 1 is 3D), else the first.
            let node3d = (0..64u32)
                .map_while(|node| {
                    let mut m = [0u64; 10];
                    bytes_mut(&mut m)[..4].copy_from_slice(&node.to_le_bytes());
                    kmt::query(handle, kmt::NODEMETADATA, &mut m, 78).then(|| (node, u32_at(bytes(&m), 4)))
                })
                .find(|&(_, kind)| kind == 1)
                .map_or(0, |(node, _)| node);
            // Whether it reports a temperature or a fan at all, asked once.
            let mut perf = [0u64; 8];
            let (temp, fan) = if kmt::query(handle, kmt::ADAPTERPERFDATA, &mut perf, 64) { adapter_perf(bytes(&perf)) } else { (None, None) };
            gpus.push(Gpu { handle, luid, id: super::gpu_id(vendor, n), name, flags, vendor, vram, node3d, temp: temp.is_some(), fan: fan.is_some(), clock: false });
        }
        (gpus, seen)
    }

    // Its driver's description (KMTQAITYPE_DRIVER_DESCRIPTION: 4096 UTF-16
    // units), else the registry's adapter string (ADAPTERREGISTRYINFO: the
    // first of four of 260).
    fn adapter_name(handle: u32) -> String {
        let text = |b: &[u8], units: usize| {
            let wide: Vec<u16> = (0..units).map(|i| u16_at(b, i * 2)).take_while(|&c| c != 0).collect();
            String::from_utf16_lossy(&wide).trim().to_string()
        };
        let mut b = vec![0u64; 1024];
        if kmt::query(handle, kmt::DRIVER_DESCRIPTION, &mut b, 8192) {
            let name = text(bytes(&b), 4096);
            if !name.is_empty() {
                return name;
            }
        }
        let mut b = vec![0u64; 260];
        if kmt::query(handle, kmt::ADAPTERREGISTRYINFO, &mut b, 2080) {
            return text(bytes(&b), 260);
        }
        String::new()
    }
}

#[cfg(not(windows))]
mod imp {
    use serde_json::Value;

    use super::*;

    // Nothing to read here: no hardware, no readings.
    pub struct Reader;

    impl Reader {
        pub fn new() -> Reader {
            Reader
        }
        pub fn read(&mut self, _want: &Want, _basics: &Basics) -> Round {
            Round::default()
        }
        pub fn seen(&self) -> Value {
            Value::Null
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The user's PC as the probes saw it (2026-10-16), from Windows' own
    // answers: three virtual displays and Microsoft's software renderer
    // around the one card.
    #[test]
    fn gpu_instances_are_told_by_their_adapter_and_engine() {
        assert_eq!(luid_in("luid_0x00000000_0x000123DC_phys_0").map(|l| l.0), Some(0x123DC));
        assert_eq!(luid_in("pid_9400_luid_0x00000001_0x0001E39F_phys_0_eng_3_engtype_Copy").map(|l| l.0), Some((1 << 32) | 0x1E39F));
        assert_eq!(engine_in("pid_9400_luid_0x00000000_0x000123DC_phys_0_eng_0_engtype_3D"), Some((0x123DC, 0)));
        assert_eq!(engine_in("pid_9400_luid_0x00000000_0x000123DC_phys_1_eng_14_engtype_Security_1"), Some((0x123DC, (1 << 16) | 14)));
        assert_eq!(engine_in("pid_1_luid_0x0000_0x1"), None);
        assert_eq!(engine_in("_Total"), None);
        // Two processes on the 3D engine (14.8% between them, as the probe
        // read), one on a copy engine, the software renderer's own.
        let three_d = (0x123DC, 0);
        let loads = gpu_loads([(three_d, 10.0), (three_d, 4.8), ((0x123DC, 3), 1.0), ((0x13AD6, 0), 0.4)]);
        assert_eq!(loads.len(), 2);
        assert!((loads[0].1 - 14.8).abs() < 1e-9 && loads[0].0 == 0x123DC, "{loads:?}");
        // Two video encoders at 60% each: the busiest, not their sum.
        assert_eq!(gpu_loads([((1, 6), 60.0), ((1, 7), 60.0)]), [(1, 60.0)]);
        assert_eq!(gpu_loads([((1, 0), 70.0), ((1, 0), 50.0)]), [(1, 100.0)]);
    }

    #[test]
    fn disks_and_cores_are_told_by_their_instance() {
        assert_eq!(drive_in("0 C:"), Some(0));
        assert_eq!(drive_in("1 D: E:"), Some(1));
        assert_eq!(drive_in("12"), Some(12));
        assert_eq!(drive_in("_Total"), None);
        assert!(is_core("0,5") && is_core("1,63"));
        assert!(!is_core("_Total") && !is_core("0,_Total"));
    }

    // D3DKMT's answers as the RTX 4080 Laptop GPU gave them: 40.3 °C (403),
    // no fan, power 0; the 3D engine at 2280 MHz.
    #[test]
    fn the_cards_temperature_and_clock_are_read_where_windows_puts_them() {
        let mut perf = [0u8; 64];
        perf[8..16].copy_from_slice(&9_001_000_000u64.to_le_bytes());
        perf[56..60].copy_from_slice(&403u32.to_le_bytes());
        assert_eq!(adapter_perf(&perf), (Some(40.3), None));
        perf[48..52].copy_from_slice(&1180u32.to_le_bytes());
        assert_eq!(adapter_perf(&perf).1, Some(1180.0));
        assert_eq!(adapter_perf(&[0u8; 64]), (None, None));
        let mut node = [0u8; 56];
        node[8..16].copy_from_slice(&2_280_000_000u64.to_le_bytes());
        assert_eq!(node_mhz(&node), Some(2280.0));
        assert_eq!(node_mhz(&[0u8; 56]), None);
    }

    // The WD PC SN560's answer: critical 88, warning 84, three sensors, the
    // composite 56 °C first; the Samsung 990 PRO's warning 82; none reported.
    #[test]
    fn a_drives_temperature_and_health_are_read_where_windows_puts_them() {
        let mut t = [0u8; 72];
        t[0..4].copy_from_slice(&40u32.to_le_bytes());
        t[4..8].copy_from_slice(&72u32.to_le_bytes());
        t[8..10].copy_from_slice(&88i16.to_le_bytes());
        t[10..12].copy_from_slice(&84i16.to_le_bytes());
        t[12..14].copy_from_slice(&3u16.to_le_bytes());
        for (i, temp) in [56i16, 78, 54].into_iter().enumerate() {
            t[24 + i * 16 + 2..24 + i * 16 + 4].copy_from_slice(&temp.to_le_bytes());
        }
        assert_eq!(drive_temperature(&t), (Some(56.0), Some(84.0)));
        t[26..28].copy_from_slice(&i16::MIN.to_le_bytes());
        t[10..12].copy_from_slice(&i16::MIN.to_le_bytes());
        assert_eq!(drive_temperature(&t), (None, None));
        t[12..14].copy_from_slice(&0u16.to_le_bytes());
        assert_eq!(drive_temperature(&t), (None, None));
        // Its health log: 329 K, 100% spare, 0% used, 1826 hours, 39 unsafe shutdowns.
        let mut h = vec![0u8; 512];
        h[1..3].copy_from_slice(&329u16.to_le_bytes());
        (h[3], h[4], h[5]) = (100, 10, 0);
        h[128..136].copy_from_slice(&1826u64.to_le_bytes());
        h[144..152].copy_from_slice(&39u64.to_le_bytes());
        assert_eq!(nvme_health(&h), Some(Health { used: 0, spare: 100, hours: 1826, unsafe_shutdowns: 39 }));
        assert_eq!(nvme_health(&h[..100]), None);
        // STORAGE_DEVICE_DESCRIPTOR: no vendor for NVMe, a USB drive's split
        // in two ("WDC  WDS" "100T2B0A-00SM50").
        let descriptor = |vendor: &str, product: &str, bus: u32| {
            let mut b = vec![0u8; 40];
            let at = |text: &str, field: usize, b: &mut Vec<u8>| {
                if !text.is_empty() {
                    let from = b.len() as u32;
                    b[field..field + 4].copy_from_slice(&from.to_le_bytes());
                    b.extend_from_slice(text.as_bytes());
                    b.push(0);
                }
            };
            at(vendor, 12, &mut b);
            at(product, 16, &mut b);
            b[28..32].copy_from_slice(&bus.to_le_bytes());
            b
        };
        assert_eq!(drive_model(&descriptor("", "WD PC SN560 SDDPNQE-1T00-1102", 17)), ("WD PC SN560 SDDPNQE-1T00-1102".into(), 17));
        assert_eq!(drive_model(&descriptor("WDC  WDS", "100T2B0A-00SM50", 7)), ("WDC WDS 100T2B0A-00SM50".into(), 7));
        assert_eq!(drive_model(&descriptor("Samsung", "SSD 870 QVO 8TB", 7)).0, "Samsung SSD 870 QVO 8TB");
    }

    // Task Manager's figures from the probe's sample (GiB): 63.681 total,
    // 31.951 available, 0.088 modified, 31.794 standby: 31.642 in use,
    // 31.882 cached.
    #[test]
    fn memory_is_split_as_task_manager_does() {
        let gib = |g: f64| (g * 1_073_741_824.0) as u64;
        let (in_use, cached) = memory_split(gib(63.681), gib(31.951), gib(0.088), gib(31.794));
        assert!((in_use as f64 / 1_073_741_824.0 - 31.642).abs() < 0.001, "{in_use}");
        assert!((cached as f64 / 1_073_741_824.0 - 31.882).abs() < 0.001, "{cached}");
        assert_eq!(memory_split(10, 20, 1, 2), (0, 3));
    }

    // 2500 MHz base, 141.44% on the whole: 3536 MHz; the fastest core at
    // 182.6%: 4565 MHz.
    #[test]
    fn the_cpus_speed_is_its_base_times_its_performance() {
        let (avg, fastest) = cpu_mhz(2500.0, Some(141.44), &[59.7, 182.6, 120.0]);
        assert_eq!((avg.map(f64::round), fastest.map(f64::round)), (Some(3536.0), Some(4565.0)));
        assert_eq!(cpu_mhz(0.0, Some(141.0), &[]), (None, None));
        assert_eq!(cpu_mhz(2500.0, None, &[]), (None, None));
    }

    // The probe's five adapters: the virtual displays are indirect, the
    // basic renderer software; the card, hybrid-discrete, is the discrete
    // one; an AMD or Intel one by its name. Nothing of it is shown unless
    // picked (the CPU, memory and network alone by default).
    #[test]
    fn the_discrete_card_is_told() {
        assert!(!crate::gpu::renders(0x342) && !crate::gpu::renders(0x105));
        assert!(crate::gpu::renders(0x231b));
        assert_eq!(discrete(0x231b, 0x10DE, "/gpu-nvidia/0", "NVIDIA GeForce RTX 4080 Laptop GPU"), 2);
        assert_eq!(discrete(0x0021, 0x1002, "/gpu-amd/0", "AMD Radeon(TM) 610M"), 0);
        assert_eq!(discrete(0x0003, 0x1002, "/gpu-amd/0", "AMD Radeon RX 7900 XTX"), 1);
        assert_eq!(discrete(0x0003, 0x8086, "/gpu-intel/0", "Intel(R) UHD Graphics 770"), 0);
        assert_eq!(discrete(0x0003, 0x8086, "/gpu-intel/0", "Intel(R) Arc(TM) A770 Graphics"), 1);
        assert_eq!((gpu_id(0x10DE, 0), gpu_id(0x1002, 1), gpu_id(0x8086, 0), gpu_id(0x5143, 0)), ("/gpu-nvidia/0".into(), "/gpu-amd/1".into(), "/gpu-intel/0".into(), "/gpu-other/0".into()));
        assert!(Hw::new().picked(&Conf::default()).is_empty());
    }

    // What is read: everything while the settings are open, else the picks'.
    #[test]
    fn only_what_is_picked_is_read() {
        let ids = vec!["/gpu-nvidia/0/temperature/0".to_string(), "/disk/2/read/0".to_string()];
        let want = Want { all: false, ids: &ids };
        assert!(want.has("/gpu-nvidia/0", "temperature") && !want.has("/gpu-nvidia/0", "load"));
        assert!(!want.has("/gpu-nvidia/1", "temperature") && !want.has("/gpu-nvidia", "temperature"));
        assert!(want.has("/disk/2", "read") && !want.has("/disk/2", "write") && !want.has("/disk/20", "read"));
        assert!(want.any(["/disk/0", "/disk/2"], "read") && !want.any(["/disk/0"], "read"));
        assert!(Want { all: true, ids: &[] }.has("/cpu/0", "clock"));
    }

    // None chosen: LibreHardwareMonitor while it answers, else the built-in
    // one, never saved; chosen: as chosen, whatever answers.
    #[test]
    fn the_source_is_as_chosen_else_lhm_while_it_answers() {
        assert_eq!(Source::of(&json!({}), &Value::Null, true), Source::Lhm);
        assert_eq!(Source::of(&json!({}), &Value::Null, false), Source::Builtin);
        assert_eq!(Source::of(&json!({ "sensors": ["/amdcpu/0/temperature/2"] }), &Value::Null, false), Source::Builtin);
        assert_eq!(Source::of(&json!({ "cpu": true }), &json!("lhm"), false), Source::Lhm);
        assert_eq!(Source::of(&json!({ "sensors": [] }), &json!("builtin"), true), Source::Builtin);
        // LHM's old switch off: never asked, so the built-in one.
        assert_eq!(Source::of(&json!({ "lhm": false }), &Value::Null, true), Source::Builtin);
        assert_eq!(Source::of(&json!({ "lhm": false }), &json!("lhm"), false), Source::Lhm);
        assert_eq!(Source::in_effect(None, false, true), Source::Lhm);
        assert_eq!(Source::chosen(&json!("hwinfo")), None);
        // Asked while chosen, or while none is (to know whether it runs).
        assert!(Source::asks_lhm(&json!({}), &Value::Null) && Source::asks_lhm(&json!({ "lhm": false }), &json!("lhm")));
        assert!(!Source::asks_lhm(&json!({}), &json!("builtin")) && !Source::asks_lhm(&json!({ "lhm": false }), &Value::Null));
        assert_eq!((Source::Builtin.picks(), Source::Lhm.picks()), ("builtinSensors", "sensors"));
        assert!(is_source(&json!("builtin")) && is_source(&json!("lhm")) && !is_source(&json!("hwinfo")));
        assert_eq!(is_setting(PICKS, &json!(["/gpu-nvidia/0/temperature/0"])), Some(true));
        assert_eq!(is_setting(PICKS, &json!(["gpu"])), Some(false));
        assert_eq!(is_setting("sensors", &json!([])), None);
        assert_eq!(Conf::from(&json!({ PICKS: ["/cpu/0/clock/0"], "sensors": ["/x"] })).picks, Some(vec!["/cpu/0/clock/0".to_string()]));
        assert_eq!(Conf::from(&json!({ "sensors": ["/x"] })).picks, None);
    }

    fn round() -> Round {
        let mut round = Round::default();
        round.hardware.push(Hardware { id: "/cpu/0".into(), class: Class::Cpu, name: "AMD Ryzen 9 9955HX 16-Core Processor".into(), about: Value::Null, health: None });
        round.readings.push(Reading { toggle: Some("cpu"), ..Reading::new("/cpu/0", "load", 0, "hw.load", Kind::Load, Some(29.0)) });
        round.readings.push(Reading::new("/cpu/0", "clock", 0, "hw.clock", Kind::Clock, Some(3536.1)));
        round.hardware.push(Hardware { id: "/gpu-nvidia/0".into(), class: Class::Gpu, name: "NVIDIA GeForce RTX 4080 Laptop GPU".into(), about: Value::Null, health: None });
        round.readings.push(Reading::new("/gpu-nvidia/0", "temperature", 0, "hw.temp", Kind::Temperature, Some(41.1)));
        round.readings.push(Reading::new("/gpu-nvidia/0", "load", 0, "hw.load", Kind::Load, Some(14.8)));
        round.readings.push(Reading { of: Some(11_994.0), ..Reading::new("/gpu-nvidia/0", "data", 0, "hw.vram", Kind::Data, Some(1204.7)) });
        round.readings.push(Reading { info: true, ..Reading::new("/gpu-nvidia/0", "data", 1, "hw.shared", Kind::Data, Some(62.9)) });
        let health = Some(Health { used: 0, spare: 100, hours: 1826, unsafe_shutdowns: 39 });
        round.hardware.push(Hardware { id: "/disk/0".into(), class: Class::Disk, name: "WD PC SN560 SDDPNQE-1T00-1102".into(), about: json!("NVMe"), health });
        round.readings.push(Reading { warn: Some(84.0), ..Reading::new("/disk/0", "temperature", 0, "hw.temp", Kind::Temperature, Some(56.0)) });
        round.readings.push(Reading::new("/disk/0", "read", 0, "hw.read", Kind::Read, Some(1_234_567.0)));
        round.readings.push(Reading::new("/disk/0", "write", 0, "hw.write", Kind::Write, None));
        basic_hardware(&mut round, &Basics { net: Some((81_234.0, 512.0)), battery: Some((85, true)), ..Basics::default() });
        round
    }

    // The parts: the ones picked with a value, in the order listed, their
    // hardware's glyph; a drive's warning with its temperature. Never one of
    // the monitor's own (on by their switches) nor one for the settings alone.
    #[test]
    fn the_parts_are_the_readings_picked() {
        let round = round();
        let picked: Vec<String> = ["/disk/0/temperature/0", "/gpu-nvidia/0/temperature/0", "/cpu/0/load/0", "/gpu-nvidia/0/data/1", "/disk/0/write/0", "/disk/0/read/0", "/gpu-nvidia/0/data/0", "/cpu/0/clock/0"].map(String::from).to_vec();
        assert_eq!(
            parts_of(&round, &picked),
            [
                json!({ "icon": "cpu", "kind": "clock", "text": "3.5G" }),
                json!({ "icon": "gpu", "kind": "temp", "text": "41°" }),
                json!({ "icon": "gpu", "kind": "data", "text": "1.2G" }),
                json!({ "icon": "disk", "kind": "temp", "text": "56°", "warn": 84.0 }),
                json!({ "icon": "disk", "kind": "read", "text": "1.2M" }),
            ]
        );
        assert!(parts_of(&round, &[]).is_empty());
    }

    // The settings' view: each reading with its words, on by its pick or by
    // the monitor's switch, the network's and battery's after the hardware.
    #[test]
    fn the_view_lists_every_reading_by_hardware() {
        let view = view_of(&round(), &["/gpu-nvidia/0/temperature/0".to_string()], [true, true, false, true]);
        assert_eq!(view["source"], "builtin");
        let ids: Vec<&str> = view["hardware"].as_array().unwrap().iter().map(|h| h["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["/cpu/0", "/gpu-nvidia/0", "/disk/0", "/net/0", "/battery/0"]);
        assert_eq!(view["hardware"][2]["health"], json!({ "used": 0, "spare": 100, "hours": 1826, "unsafe": 39 }));
        assert_eq!(view["hardware"][4]["about"], json!({ "key": "hw.charging" }));
        let sensor = |id: &str| view["sensors"].as_array().unwrap().iter().find(|s| s["id"] == id).unwrap().clone();
        assert_eq!((sensor("/gpu-nvidia/0/temperature/0")["on"].clone(), sensor("/gpu-nvidia/0/temperature/0")["reading"].clone()), (json!(true), json!("41.1 °C")));
        assert_eq!((sensor("/cpu/0/load/0")["on"].clone(), sensor("/cpu/0/load/0")["toggle"].clone(), sensor("/cpu/0/load/0")["reading"].clone()), (json!(true), json!("cpu"), json!("29 %")));
        assert_eq!(sensor("/cpu/0/clock/0")["reading"], "3536 MHz");
        assert_eq!(sensor("/gpu-nvidia/0/data/0")["reading"], "1.2 / 11.7 GB");
        assert_eq!((sensor("/gpu-nvidia/0/data/1")["reading"].clone(), sensor("/gpu-nvidia/0/data/1")["info"].clone()), (json!("63 MB"), json!(true)));
        assert_eq!(sensor("/disk/0/temperature/0")["reading"], "56 °C");
        assert_eq!((sensor("/disk/0/read/0")["reading"].clone(), sensor("/disk/0/write/0")["reading"].clone()), (json!("1.2 MB/s"), json!("—")));
        // The network's switch off.
        assert_eq!((sensor("/net/0/rate/0")["on"].clone(), sensor("/net/0/rate/0")["reading"].clone(), sensor("/net/0/rate/1")["reading"].clone()), (json!(false), json!("81 KB/s"), json!("512 B/s")));
        assert_eq!(sensor("/battery/0/level/0")["reading"], "85 %");
    }

    // Every name and word the view gives the page is in her strings, in
    // both languages.
    #[test]
    fn its_words_are_in_both_languages() {
        let strings = include_str!("../../src/shared/i18n.js");
        let keys = ["hw.load", "hw.clock", "hw.fastest", "hw.temp", "hw.coreClock", "hw.vram", "hw.shared", "hw.fan", "hw.inUseShare", "hw.inUse", "hw.available", "hw.cached", "hw.committed", "hw.active", "hw.read", "hw.write", "hw.down", "hw.up", "hw.charge", "hw.charging", "hw.cores", "hw.vramTotal", "hw.memTotal"];
        for key in keys {
            assert_eq!(strings.matches(&format!("'{key}':")).count(), 2, "{key}");
        }
    }

    // On this machine: the CPU has a name, cores and a base speed; the
    // second turn has its speed; the memory and the network are listed.
    #[cfg(windows)]
    #[test]
    fn this_machine_is_read() {
        let mut reader = imp::Reader::new();
        let want = Want { all: true, ids: &[] };
        reader.read(&want, &Basics::default());
        std::thread::sleep(std::time::Duration::from_millis(300));
        let round = reader.read(&want, &Basics::default());
        let cpu = &round.hardware[0];
        assert!(cpu.class == Class::Cpu && !cpu.name.is_empty(), "{cpu:?}");
        let clock = round.readings.iter().find(|r| r.id == "/cpu/0/clock/0").unwrap();
        assert!(clock.value.is_some_and(|mhz| mhz > 100.0), "{clock:?}");
        assert!(round.hardware.iter().any(|h| h.class == Class::Memory) && round.hardware.iter().any(|h| h.class == Class::Net));
        let in_use = round.readings.iter().find(|r| r.id == "/ram/0/data/0").unwrap();
        assert!(in_use.value.is_some_and(|mb| mb > 100.0) && in_use.of.is_some(), "{in_use:?}");
        assert!(reader.seen()["cpu"]["threads"].as_u64().unwrap() >= 1);
    }
}
