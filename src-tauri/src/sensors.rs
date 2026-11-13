// The sensor helper (#18, part B): the CPU's temperature and power, which
// live in the CPU's own registers (AMD's SMN, Intel's MSRs) and only the
// kernel can read. Windows has no public interface for them, so this reads
// them through PawnIO (namazso's signed input-output driver, which LHM 0.9.5
// and FanControl use): it executes only signed module blobs, each allowed to
// touch only particular registers, so Defender does not treat it as WinRing0
// is treated. A non-elevated process gets E_ACCESSDENIED from pawnio_open
// (verified on the user's PC), so the reading runs in an elevated helper:
// her own exe with a flag (SENSORS_FLAG), registered by an elevated one-time
// setup as a scheduled task "Wakuwaku sensors-<SID>" (RunLevel Highest, at
// this user's logon, restart on failure), started at once. The helper loads
// PawnIOLib.dll at run time (LoadLibrary; no link-time dependency), loads the
// module for this CPU, reads every 2 s, and hands the readings to her on
// 127.0.0.1 (POST /hw), which she accepts only when they carry the secret
// token the setup left in her data folder (a file only she and her helper
// read). The helper never takes commands; it only reads. It stops on its own
// when that token is gone (the switch turned off removes the task and the
// token).
//
// The module blobs are namazso's PawnIO.Modules (LGPL-2.1-or-later). PawnIO
// runs only his signed builds, so we cannot compile our own; and the
// relinking duty LGPL puts on a work that embeds the library makes "may she
// ship them inside her exe" less than clear. So she does not: the setup
// downloads the official module release (its zip's SHA-256 pinned and
// checked, the blobs inside it his and signed) and keeps the two she needs
// in her data folder. PawnIO's own installer is offered the same way when the
// driver is missing: downloaded only on the person's click, its SHA-256
// checked, run elevated as part of the one setup. Turning the switch off
// removes the task (one more UAC) and leaves PawnIO installed: it is not hers.
//
// What is read, as LibreHardwareMonitor reads it (MPL-2.0; its Amd17Cpu.cs
// and IntelCpu.cs were read for the register numbers and the maths, not
// copied): on AMD family 17h-1Ah, the SMN THM register for Tctl/Tdie and the
// per-CCD registers, and MSR C001_029B (package energy) as a rate with
// C001_0299's unit; on Intel, IA32_PACKAGE_THERM_STATUS against the TjMax in
// IA32_TEMPERATURE_TARGET, and MSR 0x611 (package energy) as a rate with
// 0x606's unit.
//
// Off Windows there is no helper and no setup: what is here is Windows' own
// (and its tests').
#![cfg_attr(not(windows), allow(dead_code))]

use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};

use crate::lhm::Kind;
use crate::lock::Lock;

// Her exe with this is the elevated helper (main.rs): `SENSORS_FLAG <port>
// <dataDir>`.
pub const SENSORS_FLAG: &str = "--wakuwaku-sensors";
// The elevated one-time setup: `SETUP_FLAG <port> <sid> <dataDir> [pawnio]`.
pub const SETUP_FLAG: &str = "--wakuwaku-sensors-setup";
// The elevated removal: `REMOVE_FLAG <sid> <dataDir>`.
pub const REMOVE_FLAG: &str = "--wakuwaku-sensors-remove";

// The task's name for a user (its SID): the Task Scheduler's folder is the
// whole computer's, so the name carries the SID (as her logon task does).
pub fn task_name(sid: &str) -> String {
    format!("Wakuwaku sensors-{sid}")
}

// The helper's protocol. The task runs the helper copy as it was at setup;
// if a newer build of hers needs a newer helper, the switch asks for an
// update (one UAC) rather than quietly running an old one against new code.
// The exe the task runs lives where only administrators can write (see
// admin_dir), because the task runs it with the highest rights at every
// logon: her exe usually sits where the user can write (a portable exe in
// Downloads), and a logon task pointed there would let anything at medium
// integrity replace it and be started elevated — a UAC bypass.
pub const HELPER_PROTOCOL: u32 = 1;
// The helper copy's name in its admin-only folder (see admin_dir); a name of
// its own, so it is clear in the Task Scheduler and Task Manager what runs.
pub const HELPER_EXE: &str = "wakuwaku-sensors.exe";

// The DACL the helper's folder is given and then checked against: it and the
// exe in it are writable by administrators and SYSTEM alone, and the user
// may read and run them, with no ACE inherited from the parent that could
// widen it (D:P, protected). SYSTEM and Administrators full, Users
// read+execute (FRFX). Set and read back by the elevated setup; the setup
// refuses to register the task if the folder is not admin-only.
pub fn admin_only_sddl() -> &'static str {
    "D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FRFX;;;BU)"
}

// Whether a DACL (as SDDL) is admin-only: protected (no inherited ACEs), and
// no broad group (Users, Everyone, Authenticated, Interactive) granted more
// than read and execute. A folder that is not is not safe to run elevated
// from, so the setup stops.
pub fn acl_is_admin_only(sddl: &str) -> bool {
    let Some(dacl) = sddl.split("D:").nth(1) else { return false };
    // Protected: the parent's ACEs do not apply, so a loose parent cannot
    // widen it. SDDL renders it as P (often PAI on read-back).
    if !dacl.starts_with('P') {
        return false;
    }
    // Broad groups, by their SDDL aliases and well-known SIDs.
    const BROAD: [&str; 7] = ["BU", "WD", "AU", "IU", "S-1-1-0", "S-1-5-11", "S-1-5-32-545"];
    // Rights that let a program replace the exe (or the folder's files).
    const WRITE: [&str; 9] = ["FA", "FW", "GA", "GW", "WD", "WO", "DC", "0x10000000", "0x40000000"];
    for ace in dacl.split('(').skip(1) {
        let fields: Vec<&str> = ace.trim_end_matches(')').split(';').collect();
        if fields.len() < 6 {
            continue;
        }
        let (kind, rights, sid) = (fields[0], fields[2], fields[5]);
        // Only allow-ACEs grant; a deny-ACE only narrows.
        if kind.starts_with('A') && BROAD.contains(&sid) && WRITE.iter().any(|w| rights.contains(w)) {
            return false;
        }
    }
    true
}

// Whether the helper copy the task runs is older than this build needs.
pub fn needs_update(installed: Option<u32>, current: u32) -> bool {
    installed.is_some_and(|v| v < current)
}

// The folder in her data where the token and the module blobs live.
pub fn dir(data: &std::path::Path) -> std::path::PathBuf {
    data.join("sensors")
}

// The official downloads, their SHA-256 pinned (from the releases' metadata;
// the blobs inside the zip are namazso's and signed, so a zip that hashes
// right is his). Only github.com and its download host are reached.
const PAWNIO_SETUP_URL: &str = "https://github.com/namazso/PawnIO.Setup/releases/download/2.2.0/PawnIO_setup.exe";
const PAWNIO_SETUP_SHA: &str = "1f519a22e47187f70a1379a48ca604981c4fcf694f4e65b734aaa74a9fba3032";
const MODULES_URL: &str = "https://github.com/namazso/PawnIO.Modules/releases/download/0.2.11/release_0_2_11.zip";
const MODULES_SHA: &str = "43608cb89bc84247fef1368a139013f7d043e17db6d6c8dfc9b46bf0905a81f4";

// The two modules she may need, by the CPU's maker: the file she keeps it
// as, and the stem to find inside the zip (its extension may be .bin or .amx).
const AMD_MODULE: &str = "AMDFamily17";
const INTEL_MODULE: &str = "IntelMSR";

// --- The readings' ids and labels --------------------------------------------------------

// The readings join the built-in CPU's (hw::JOINED), under "/cpu/0":
// /cpu/0/temperature/0 is the package (Tctl/Tdie on AMD, the package on
// Intel), /cpu/0/temperature/N (N >= 1) is CCD N on AMD, /cpu/0/power/0 is
// package power. Her strings name each (i18n hw.*).
pub const CPU_ID: &str = "/cpu/0";
const CCD_LABELS: [&str; 8] = ["hw.ccd1", "hw.ccd2", "hw.ccd3", "hw.ccd4", "hw.ccd5", "hw.ccd6", "hw.ccd7", "hw.ccd8"];

// The label a reading's id carries (none for an id not ours), so the payload
// need not be trusted for it.
pub fn label_of(id: &str) -> Option<&'static str> {
    let rest = id.strip_prefix(CPU_ID)?.strip_prefix('/')?;
    if let Some(n) = rest.strip_prefix("temperature/") {
        return match n.parse::<usize>().ok()? {
            0 => Some("hw.cpuTemp"),
            n => CCD_LABELS.get(n - 1).copied(),
        };
    }
    (rest == "power/0").then_some("hw.cpuPower")
}

fn kind_of(id: &str) -> Option<Kind> {
    let rest = id.strip_prefix(CPU_ID)?.strip_prefix('/')?;
    if rest.starts_with("temperature/") {
        Some(Kind::Temperature)
    } else if rest == "power/0" {
        Some(Kind::Power)
    } else {
        None
    }
}

// The id for CCD n (1-based), the package temperature, and the power.
pub fn ccd_id(n: usize) -> String {
    format!("{CPU_ID}/temperature/{n}")
}
pub fn package_temp_id() -> String {
    format!("{CPU_ID}/temperature/0")
}
pub fn power_id() -> String {
    format!("{CPU_ID}/power/0")
}

// --- The maths, told apart from Windows so they can be tested --------------------------------

// AMD's Tctl, °C: the THM_TCON_CUR_TMP register's bits [31:21] are the
// temperature in steps of 0.125 °C; a range bit (0x80000) or the TjSel bits
// (0x30000 both set) mean 49 °C is to be taken off (LHM's Amd17Cpu). Tdie is
// Tctl plus a per-part offset; Zen 4/5 (and most parts) have none, so Tdie =
// Tctl there.
pub fn amd_tctl(raw: u32) -> f64 {
    let offset_flag = (raw & 0x8_0000) != 0 || (raw & 0x3_0000) == 0x3_0000;
    let t = ((raw >> 21) * 125) as f64 * 0.001;
    if offset_flag {
        t - 49.0
    } else {
        t
    }
}

// A CCD's temperature, °C, from its SMN register: bits [11:0] in steps of
// 0.125 °C with a 305 °C bias; present when the valid bit is set (Turin) or
// those bits are non-zero (older), and only when below 125 °C (a part can
// report 0xFFF while a CCD is idle). None when absent (LHM's Amd17Cpu).
pub fn amd_ccd(raw: u32, valid_bit: bool) -> Option<f64> {
    let present = if valid_bit { (raw & 0x800) != 0 } else { (raw & 0xFFF) > 0 };
    let t = ((raw & 0xFFF) as f64 * 125.0 - 305_000.0) * 0.001;
    (present && t < 125.0).then_some(t)
}

// Intel's package temperature, °C: IA32_PACKAGE_THERM_STATUS is valid only
// with bit 31 set; bits [22:16] are how far below TjMax the package is, and
// TjMax (bits [23:16] of IA32_TEMPERATURE_TARGET) is its hottest allowed
// (LHM's IntelCpu). None when not valid.
pub fn intel_package_temp(therm_status: u32, tjmax: u32) -> Option<f64> {
    if therm_status & 0x8000_0000 == 0 || tjmax == 0 {
        return None;
    }
    let delta = (therm_status >> 16) & 0x7F;
    Some(f64::from(tjmax) - f64::from(delta))
}

// TjMax from IA32_TEMPERATURE_TARGET (bits [23:16]).
pub fn tjmax_of(temperature_target: u32) -> u32 {
    (temperature_target >> 16) & 0xFF
}

// The energy unit (joules per increment) a RAPL unit register gives: its
// bits [12:8] are the ESU, and the unit is (1/2)^ESU joule (AMD's
// C001_0299, Intel's 0x606).
pub fn energy_unit(unit_reg: u32) -> f64 {
    let esu = (unit_reg >> 8) & 0x1F;
    0.5f64.powi(esu as i32)
}

// Package power, watts, from two energy-counter readings: their difference
// (the 32-bit counter wraps at 4 Gi) times the energy unit, over the seconds
// between. None when no time passed.
pub fn energy_rate(unit: f64, last: u32, now: u32, secs: f64) -> Option<f64> {
    if secs <= 0.0 {
        return None;
    }
    let delta = now.wrapping_sub(last);
    let watts = unit * f64::from(delta) / secs;
    watts.is_finite().then_some(watts)
}

// The CCD registers for an AMD part: the first CCD's SMN base, how many CCDs
// there may be, and whether the valid bit is used (LHM's Amd17Cpu, by family
// and model). None for a family without per-CCD temperatures.
pub fn amd_ccd_plan(family: u32, model: u32) -> Option<(u32, u32, bool)> {
    const THM_BASE: u32 = 0x0005_9800;
    // Zen 5 Turin / Shimada Peak (family 1Ah, model <= 2Fh): 16 CCDs, a
    // valid bit, k10temp's 0x1F0 offset.
    if family == 0x1A && model <= 0x2F {
        return Some((THM_BASE + 0x1F0, 16, true));
    }
    match (family, model) {
        // Raphael / Granite Ridge / Fire Range (Zen 4/5 desktop and HX).
        (_, 0x61 | 0x44) => Some((0x0005_9B08, 8, false)),
        // Threadripper 3000, Zen 2, Zen 3.
        (0x17, 0x31) | (_, 0x71 | 0x21) => Some((0x0005_9954, 8, false)),
        _ => None,
    }
}

// --- The task the setup registers --------------------------------------------------------

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

const DESCRIPTION: &str = "Reads the CPU's temperature and power through PawnIO and hands them to Wakuwaku. Runs as administrator (PawnIO needs it), only reads, and is taken away when the reading is turned off. 读取 CPU 温度和功耗（经 PawnIO 驱动），交给 Wakuwaku；以管理员身份运行，只读。";

// The task for this user (`sid`): her exe as the helper, at this user's
// logon, with the highest rights available (PawnIO needs them), restarted if
// it fails, on battery too, no time limit.
pub fn task_xml(sid: &str, exe: &str, args: &str) -> String {
    let (sid, exe, args) = (escape(sid), escape(exe), escape(args));
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Author>Wakuwaku</Author>
    <Description>{DESCRIPTION}</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{sid}</UserId>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{sid}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>6</Priority>
    <RestartOnFailure>
      <Interval>PT1M</Interval>
      <Count>3</Count>
    </RestartOnFailure>
    <Enabled>true</Enabled>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{exe}</Command>
      <Arguments>{args}</Arguments>
    </Exec>
  </Actions>
</Task>
"#
    )
}

// The helper's arguments in the task: the flag, her port, her data folder.
pub fn helper_args(port: u16, data: &std::path::Path) -> String {
    format!("{SENSORS_FLAG} {port} \"{}\"", data.display())
}

// --- What she keeps of the helper's readings ------------------------------------------------

// The last time (ms) the helper's readings came in, and whether the setup is
// under way (the elevated child launched, not yet answering).
static LAST_POST: AtomicU64 = AtomicU64::new(0);
static SETTING_UP: AtomicU64 = AtomicU64::new(0);
// The CPU's temperature and power as last read, for the switch's line.
static READING: Lock<(Option<f64>, Option<f64>)> = Lock::new("sensors::READING", (None, None));

const ANSWERING_MS: u64 = 8_000;
const SETTING_UP_MS: u64 = 120_000;

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

// The helper's POST /hw accepted (server.rs): its token checked against the
// one the setup left, then its readings joined to the CPU's (hw::JOINED).
// The payload is { token, readings: [{ id, value }] }; the id says what each
// is (label_of, kind_of), so the payload is not trusted for those. Returns
// whether the token matched.
pub fn accept(data: &std::path::Path, body: &Value) -> bool {
    let Some(token) = body["token"].as_str() else { return false };
    if !token_ok(data, token) {
        return false;
    }
    let mut readings = Vec::new();
    let mut temp = None;
    let mut power = None;
    for r in body["readings"].as_array().into_iter().flatten().take(64) {
        let Some(id) = r["id"].as_str() else { continue };
        let (Some(label), Some(kind)) = (label_of(id), kind_of(id)) else { continue };
        let value = r["value"].as_f64().filter(|v| v.is_finite());
        if id == package_temp_id() {
            temp = value;
        } else if id == power_id() {
            power = value;
        }
        readings.push(crate::hw::Reading {
            id: id.to_string(),
            hardware_id: CPU_ID.to_string(),
            label,
            kind,
            value,
            of: None,
            warn: None,
            toggle: None,
            info: false,
        });
    }
    *READING.lock().unwrap() = (temp, power);
    LAST_POST.store(now_ms(), Ordering::SeqCst);
    crate::hw::set_joined(readings);
    true
}

// Whether the token matches the one the setup left (constant time, so a
// wrong guess learns nothing from how long it took).
fn token_ok(data: &std::path::Path, token: &str) -> bool {
    let Ok(kept) = std::fs::read_to_string(dir(data).join("token")) else { return false };
    let kept = kept.trim();
    if kept.is_empty() || kept.len() != token.len() {
        return false;
    }
    kept.bytes().zip(token.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
}

// The setup was launched (the elevated child): the switch says "setting up"
// until the helper answers or two minutes pass.
pub fn note_setting_up() {
    SETTING_UP.store(now_ms(), Ordering::SeqCst);
}

fn answering() -> bool {
    now_ms().saturating_sub(LAST_POST.load(Ordering::SeqCst)) < ANSWERING_MS
}

fn setting_up() -> bool {
    now_ms().saturating_sub(SETTING_UP.load(Ordering::SeqCst)) < SETTING_UP_MS
}

// The switch's state, for the settings. "off": no task; "pawnioMissing": no
// PawnIO (offer to install it); "settingUp": the setup is under way;
// "running": the helper answers (with the CPU's temperature); "notAnswering":
// the task is there but nothing comes (PawnIO gone, or a VM that does not
// expose the registers). `on` is whether the switch shows on.
pub fn view(data: &std::path::Path) -> Value {
    let (installed, task) = cached_state(data);
    let (temp, power) = *READING.lock().unwrap();
    let stale = needs_update(imp::installed_protocol(), HELPER_PROTOCOL);
    let state = state_name(installed, task, answering(), setting_up(), temp.is_some(), stale);
    json!({
        "state": state,
        "on": matches!(state, "running" | "notAnswering" | "settingUp" | "needsUpdate"),
        "installed": installed,
        "temp": temp,
        "power": power,
    })
}

// The switch's state from what is known. "running" means a reading actually
// comes through: the helper may be posting (answering) yet read nothing — a
// Hyper-V guest does not expose the CPU's temperature registers — which is
// "not answering" to the person (its note says a VM may be why).
// "needsUpdate": the task runs a helper copy older than this build needs,
// so the person is asked to set it up again (one UAC) rather than trust it.
fn state_name(installed: bool, task: bool, answering: bool, setting_up: bool, has_temp: bool, needs_update: bool) -> &'static str {
    if task && needs_update {
        "needsUpdate"
    } else if answering && has_temp {
        "running"
    } else if task || answering {
        if setting_up {
            "settingUp"
        } else {
            "notAnswering"
        }
    } else if setting_up {
        "settingUp"
    } else if !installed {
        "pawnioMissing"
    } else {
        "off"
    }
}

// PawnIO present, and her task registered: both looked at afresh at most
// every few seconds (the task's check talks to the Task Scheduler over COM).
fn cached_state(data: &std::path::Path) -> (bool, bool) {
    static CACHE: Lock<Option<(std::time::Instant, bool, bool)>> = Lock::new("sensors::CACHE", None);
    let mut cache = CACHE.lock().unwrap();
    if let Some((at, installed, task)) = *cache {
        if at.elapsed() < std::time::Duration::from_secs(3) {
            return (installed, task);
        }
    }
    let installed = imp::pawnio_installed();
    let task = imp::task_present(data);
    *cache = Some((std::time::Instant::now(), installed, task));
    (installed, task)
}

// --- Windows: the helper, the setup, the removal -------------------------------------------

#[cfg(windows)]
mod imp {
    use std::io::Write;
    use std::path::Path;
    use std::time::{Duration, Instant};

    use serde_json::json;
    use sha2::{Digest, Sha256};

    use super::*;
    use crate::win32::{CloseHandle, ConvertSecurityDescriptorToStringSecurityDescriptorW, ConvertStringSecurityDescriptorToSecurityDescriptorW, GetProcAddress, LoadLibraryExW, LocalFree, WaitForSingleObject};

    // The CPU's maker and its family/model, from CPUID.
    enum Vendor {
        Amd,
        Intel,
        Other,
    }

    fn cpu_id() -> (Vendor, u32, u32) {
        // SAFETY: CPUID is available on every x86-64 CPU she runs on.
        #[cfg(target_arch = "x86_64")]
        unsafe {
            use std::arch::x86_64::__cpuid;
            let zero = __cpuid(0);
            let mut vendor = [0u8; 12];
            vendor[0..4].copy_from_slice(&zero.ebx.to_le_bytes());
            vendor[4..8].copy_from_slice(&zero.edx.to_le_bytes());
            vendor[8..12].copy_from_slice(&zero.ecx.to_le_bytes());
            let maker = match &vendor {
                b"AuthenticAMD" => Vendor::Amd,
                b"GenuineIntel" => Vendor::Intel,
                _ => Vendor::Other,
            };
            let one = __cpuid(1);
            let base_family = (one.eax >> 8) & 0xF;
            let ext_family = (one.eax >> 20) & 0xFF;
            let family = if base_family == 0xF { base_family + ext_family } else { base_family };
            let base_model = (one.eax >> 4) & 0xF;
            let ext_model = (one.eax >> 16) & 0xF;
            let model = if base_family == 0xF || base_family == 0x6 { (ext_model << 4) | base_model } else { base_model };
            (maker, family, model)
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            (Vendor::Other, 0, 0)
        }
    }

    // --- PawnIOLib.dll, loaded at run time ----------------------------------------------

    type Version = unsafe extern "system" fn(*mut u32) -> i32;
    type Open = unsafe extern "system" fn(*mut isize) -> i32;
    type Load = unsafe extern "system" fn(isize, *const u8, usize) -> i32;
    type Execute = unsafe extern "system" fn(isize, *const u8, *const u64, usize, *mut u64, usize, *mut usize) -> i32;
    type CloseFn = unsafe extern "system" fn(isize) -> i32;

    struct PawnIoLib {
        _version: Version,
        open: Open,
        load: Load,
        execute: Execute,
        close: CloseFn,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn SetDefaultDllDirectories(flags: u32) -> i32;
        fn CreateMutexW(attrs: *const core::ffi::c_void, owner: i32, name: *const u16) -> isize;
        fn ReleaseMutex(handle: isize) -> i32;
    }

    // A program's DLL search, named so nothing is picked up from a folder a
    // user can write: System32 for the default, and for PawnIOLib (loaded by
    // full path) its own admin-only folder for the DLLs it needs.
    const LOAD_LIBRARY_SEARCH_SYSTEM32: u32 = 0x800;
    const LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR: u32 = 0x100;

    #[link(name = "bcrypt")]
    extern "system" {
        fn BCryptGenRandom(algo: isize, buf: *mut u8, len: u32, flags: u32) -> i32;
    }

    #[link(name = "advapi32")]
    extern "system" {
        fn SetFileSecurityW(file: *const u16, info: u32, sd: *const core::ffi::c_void) -> i32;
        fn GetFileSecurityW(file: *const u16, info: u32, sd: *mut u8, len: u32, needed: *mut u32) -> i32;
    }

    const DACL_SECURITY_INFORMATION: u32 = 0x4;
    const PROTECTED_DACL_SECURITY_INFORMATION: u32 = 0x8000_0000;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain([0]).collect()
    }

    // PawnIOLib.dll from where PawnIO was installed (the registry's
    // InstallLocation, else %ProgramFiles%\PawnIO), its functions by name.
    fn load_lib() -> Option<PawnIoLib> {
        let path = pawnio_dll_path()?;
        // SAFETY: the driver's own signed library, by its full path; its own
        // (admin-only) folder and System32 are searched for the DLLs it
        // needs, never a folder a user can write.
        let module = unsafe { LoadLibraryExW(wide(&path.to_string_lossy()).as_ptr(), 0, LOAD_LIBRARY_SEARCH_SYSTEM32 | LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR) };
        if module == 0 {
            return None;
        }
        let f = |name: &std::ffi::CStr| -> Option<*const ()> {
            // SAFETY: a module held for the program's life; the name ends in 0.
            let p = unsafe { GetProcAddress(module, name.as_ptr() as *const u8) };
            (!p.is_null()).then_some(p)
        };
        // SAFETY: each export cast to the shape PawnIOLib.h documents.
        unsafe {
            Some(PawnIoLib {
                _version: std::mem::transmute::<*const (), Version>(f(c"pawnio_version")?),
                open: std::mem::transmute::<*const (), Open>(f(c"pawnio_open")?),
                load: std::mem::transmute::<*const (), Load>(f(c"pawnio_load")?),
                execute: std::mem::transmute::<*const (), Execute>(f(c"pawnio_execute")?),
                close: std::mem::transmute::<*const (), CloseFn>(f(c"pawnio_close")?),
            })
        }
    }

    fn pawnio_install_dir() -> Option<std::path::PathBuf> {
        use winreg::enums::HKEY_LOCAL_MACHINE;
        let key = winreg::RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\PawnIO").ok()?;
        let loc: String = key.get_value("InstallLocation").ok()?;
        (!loc.trim().is_empty()).then(|| std::path::PathBuf::from(loc.trim()))
    }

    fn pawnio_dll_path() -> Option<std::path::PathBuf> {
        let dll = pawnio_install_dir().map(|d| d.join("PawnIOLib.dll")).filter(|p| p.exists());
        dll.or_else(|| {
            let pf = std::env::var("ProgramFiles").ok()?;
            let p = std::path::Path::new(&pf).join("PawnIO").join("PawnIOLib.dll");
            p.exists().then_some(p)
        })
    }

    // Whether PawnIO is installed: the registry has its version, as LHM tells
    // it (PawnIo.IsInstalled).
    pub fn pawnio_installed() -> bool {
        use winreg::enums::HKEY_LOCAL_MACHINE;
        let has = |view| {
            winreg::RegKey::predef(HKEY_LOCAL_MACHINE)
                .open_subkey_with_flags(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\PawnIO", view)
                .ok()
                .and_then(|k| k.get_value::<String, _>("DisplayVersion").ok())
                .is_some_and(|v| !v.trim().is_empty())
        };
        has(winreg::enums::KEY_READ) || has(winreg::enums::KEY_READ | winreg::enums::KEY_WOW64_64KEY)
    }

    // Whether her sensors task is registered (COM, through logon.rs).
    pub fn task_present(_data: &Path) -> bool {
        let Some(sid) = crate::logon::user_sid() else { return false };
        matches!(crate::logon::xml(&task_name(&sid)), Ok(Some(_)))
    }

    // The admin-only folder the helper copy lives in, and the copy itself:
    // %ProgramFiles%\Wakuwaku\sensors, writable by administrators alone. The
    // task runs it with the highest rights at every logon, so it must not sit
    // where the user can replace it.
    fn admin_dir() -> Option<std::path::PathBuf> {
        Some(std::path::Path::new(&std::env::var("ProgramFiles").ok()?).join("Wakuwaku").join("sensors"))
    }

    // The protocol of the helper copy that is set up (its version left beside
    // the exe), for telling an old copy from this build's.
    pub fn installed_protocol() -> Option<u32> {
        std::fs::read_to_string(admin_dir()?.join("helper.ver")).ok()?.trim().parse().ok()
    }

    // The folder given a protected, admin-only DACL (admin_only_sddl), then
    // read back and checked: it must be admin-only, or the setup will not run
    // a task from it. The exe and files copied in inherit it (OICI).
    fn set_admin_only(dir: &Path) -> Result<(), String> {
        let wide_dir = wide(&dir.to_string_lossy());
        let mut sd: *mut core::ffi::c_void = std::ptr::null_mut();
        // SAFETY: our SDDL (a constant), the descriptor it makes is ours to free.
        let made = unsafe { ConvertStringSecurityDescriptorToSecurityDescriptorW(wide(admin_only_sddl()).as_ptr(), 1, &mut sd, std::ptr::null_mut()) };
        if made == 0 || sd.is_null() {
            return Err("the security descriptor was not made".into());
        }
        // SAFETY: the descriptor just made, applied to our own folder; the
        // protected flag keeps the parent's ACEs out.
        let set = unsafe { SetFileSecurityW(wide_dir.as_ptr(), DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION, sd) };
        // SAFETY: the descriptor is ours, freed once.
        unsafe { LocalFree(sd) };
        if set == 0 {
            return Err("the folder's security was not set".into());
        }
        if acl_is_admin_only(&read_acl(dir)?) {
            Ok(())
        } else {
            Err("the folder is not administrator-only after setting it".into())
        }
    }

    // The folder's DACL read back as SDDL.
    fn read_acl(dir: &Path) -> Result<String, String> {
        let wide_dir = wide(&dir.to_string_lossy());
        let mut needed = 0u32;
        // SAFETY: first the size with an empty buffer, then a buffer of it.
        unsafe { GetFileSecurityW(wide_dir.as_ptr(), DACL_SECURITY_INFORMATION, std::ptr::null_mut(), 0, &mut needed) };
        if needed == 0 {
            return Err("the folder's security has no size".into());
        }
        let mut buf = vec![0u8; needed as usize];
        // SAFETY: a buffer of the size asked for.
        if unsafe { GetFileSecurityW(wide_dir.as_ptr(), DACL_SECURITY_INFORMATION, buf.as_mut_ptr(), needed, &mut needed) } == 0 {
            return Err("the folder's security was not read".into());
        }
        let mut text: *mut u16 = std::ptr::null_mut();
        let mut len = 0u32;
        // SAFETY: our own descriptor; the string it makes is ours to free.
        if unsafe { ConvertSecurityDescriptorToStringSecurityDescriptorW(buf.as_mut_ptr() as *mut core::ffi::c_void, 1, DACL_SECURITY_INFORMATION, &mut text, &mut len) } == 0 || text.is_null() {
            return Err("the folder's security did not read as text".into());
        }
        // SAFETY: a null-terminated wide string of `len`, freed once read.
        let sddl = unsafe {
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(text, len as usize));
            LocalFree(text as *mut core::ffi::c_void);
            s
        };
        Ok(sddl)
    }

    // A loaded PawnIO executor with one module.
    struct Module {
        lib: PawnIoLib,
        handle: isize,
    }

    impl Module {
        // Opened and the blob loaded; None when PawnIO cannot be used (not
        // elevated: E_ACCESSDENIED) or the blob is not its own (pawnio_load
        // checks the signature).
        fn load(lib: PawnIoLib, blob: &[u8]) -> Option<Module> {
            let mut handle = 0isize;
            // SAFETY: our own handle out-parameter.
            if unsafe { (lib.open)(&mut handle) } != 0 || handle == 0 {
                return None;
            }
            // SAFETY: our own handle; the blob is read, not kept.
            if unsafe { (lib.load)(handle, blob.as_ptr(), blob.len()) } != 0 {
                // SAFETY: the handle opened just above, closed once.
                unsafe { (lib.close)(handle) };
                return None;
            }
            Some(Module { lib, handle })
        }

        // A module function (ioctl_read_msr, ioctl_read_smn): the inputs in,
        // one value out, or None when it would not run (a register the module
        // does not allow: STATUS_ACCESS_DENIED).
        fn call(&self, name: &std::ffi::CStr, input: &[u64]) -> Option<u64> {
            let mut out = [0u64; 1];
            let mut written = 0usize;
            // SAFETY: our own handle; the name ends in 0; the buffers are
            // ours and their counts are given.
            let hr = unsafe { (self.lib.execute)(self.handle, name.as_ptr() as *const u8, input.as_ptr(), input.len(), out.as_mut_ptr(), out.len(), &mut written) };
            (hr == 0 && written >= 1).then_some(out[0])
        }

        fn read_msr(&self, msr: u32) -> Option<u64> {
            self.call(c"ioctl_read_msr", &[u64::from(msr)])
        }

        fn read_smn(&self, offset: u32) -> Option<u32> {
            self.call(c"ioctl_read_smn", &[u64::from(offset)]).map(|v| v as u32)
        }
    }

    impl Drop for Module {
        fn drop(&mut self) {
            // SAFETY: our own handle, closed once.
            unsafe { (self.lib.close)(self.handle) };
        }
    }

    // The PCI-access mutex PawnIO's SMN read asks to be held (LHM's
    // Mutexes.WaitPciBus around \BaseNamedObjects\Access_PCI).
    struct PciBus(isize);

    impl PciBus {
        fn open() -> PciBus {
            // SAFETY: a named mutex, ours to hold; null attributes.
            PciBus(unsafe { CreateMutexW(std::ptr::null(), 0, wide("Global\\Access_PCI").as_ptr()) })
        }
        fn wait(&self) -> bool {
            // SAFETY: our own handle; 0 (signalled) or 0x80 (abandoned) is held.
            self.0 != 0 && matches!(unsafe { WaitForSingleObject(self.0, 10) }, 0 | 0x80)
        }
        fn release(&self) {
            // SAFETY: our own handle; harmless if we did not hold it.
            unsafe { ReleaseMutex(self.0) };
        }
    }

    impl Drop for PciBus {
        fn drop(&mut self) {
            if self.0 != 0 {
                // SAFETY: our own handle, closed once.
                unsafe { CloseHandle(self.0) };
            }
        }
    }

    // What one reading cycle found: the readings to send, by id.
    struct Last {
        energy: Option<(u32, Instant)>,
    }

    fn read_amd(module: &Module, pci: &PciBus, family: u32, model: u32, last: &mut Last) -> Vec<(String, f64)> {
        let mut out = Vec::new();
        if pci.wait() {
            if let Some(raw) = module.read_smn(0x0005_9800) {
                out.push((package_temp_id(), amd_tctl(raw)));
            }
            if let Some((base, count, valid)) = amd_ccd_plan(family, model) {
                for i in 0..count.min(CCD_LABELS.len() as u32) {
                    if let Some(raw) = module.read_smn(base + i * 4) {
                        if let Some(t) = amd_ccd(raw, valid) {
                            out.push((ccd_id(i as usize + 1), t));
                        }
                    }
                }
            }
            pci.release();
        }
        // Package power (RAPL): the energy counter as a rate.
        if let (Some(unit_reg), Some(energy)) = (module.read_msr(0xC001_0299), module.read_msr(0xC001_029B)) {
            let unit = energy_unit(unit_reg as u32);
            let now = energy as u32;
            let when = Instant::now();
            if let Some((prev, at)) = last.energy {
                if let Some(w) = energy_rate(unit, prev, now, when.duration_since(at).as_secs_f64()) {
                    out.push((power_id(), w));
                }
            }
            last.energy = Some((now, when));
        }
        out
    }

    fn read_intel(module: &Module, last: &mut Last) -> Vec<(String, f64)> {
        let mut out = Vec::new();
        if let (Some(status), Some(target)) = (module.read_msr(0x0000_01B1), module.read_msr(0x0000_01A2)) {
            if let Some(t) = intel_package_temp(status as u32, tjmax_of(target as u32)) {
                out.push((package_temp_id(), t));
            }
        }
        if let (Some(unit_reg), Some(energy)) = (module.read_msr(0x0000_0606), module.read_msr(0x0000_0611)) {
            let unit = energy_unit(unit_reg as u32);
            let now = energy as u32;
            let when = Instant::now();
            if let Some((prev, at)) = last.energy {
                if let Some(w) = energy_rate(unit, prev, now, when.duration_since(at).as_secs_f64()) {
                    out.push((power_id(), w));
                }
            }
            last.energy = Some((now, when));
        }
        out
    }

    // The helper (SENSORS_FLAG): load the module for this CPU, read every
    // 2 s, POST to her /hw. It stops when its token is gone (the switch
    // turned off), and gives up (exits non-zero, the task restarts it later)
    // when PawnIO cannot be used at all.
    pub fn run_helper(port: u16, data: &Path) {
        // Nothing loaded from a folder a user can write: System32 for the
        // default search (PawnIOLib is loaded by its full path with its own
        // admin-only folder added for its dependencies). The helper's own exe
        // sits in an admin-only folder too (admin_dir).
        // SAFETY: a process-wide setting, before any PawnIOLib load.
        unsafe { SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32) };
        let log = |line: &str| append(&dir(data).join("helper.log"), line);
        let token = match std::fs::read_to_string(dir(data).join("token")) {
            Ok(t) if !t.trim().is_empty() => t.trim().to_string(),
            _ => {
                log("no token: the reading is not set up");
                std::process::exit(1);
            }
        };
        let (vendor, family, model) = cpu_id();
        let module_name = match vendor {
            Vendor::Amd => AMD_MODULE,
            Vendor::Intel => INTEL_MODULE,
            Vendor::Other => {
                log("this CPU is neither AMD nor Intel: nothing to read");
                std::process::exit(1);
            }
        };
        let Some(lib) = load_lib() else {
            log("PawnIOLib.dll could not be loaded (is PawnIO installed?)");
            std::process::exit(1);
        };
        let blob = match std::fs::read(dir(data).join(format!("{module_name}.bin"))) {
            Ok(b) => b,
            Err(e) => {
                log(&format!("the module {module_name}.bin is not there: {e}"));
                std::process::exit(1);
            }
        };
        let Some(module) = Module::load(lib, &blob) else {
            log("PawnIO would not open or load the module (not elevated, or the module is not its own)");
            std::process::exit(1);
        };
        log(&format!("reading started: {module_name}, family {family:#x} model {model:#x}"));
        let pci = PciBus::open();
        let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(2))).proxy(None).build().new_agent();
        let mut last = Last { energy: None };
        let mut quiet_since: Option<Instant> = None;
        loop {
            // The token gone (the switch turned off): stop.
            match std::fs::read_to_string(dir(data).join("token")) {
                Ok(t) if t.trim() == token => {}
                _ => {
                    log("the token is gone: the reading was turned off");
                    return;
                }
            }
            let readings = match vendor {
                Vendor::Amd => read_amd(&module, &pci, family, model, &mut last),
                Vendor::Intel => read_intel(&module, &mut last),
                Vendor::Other => Vec::new(),
            };
            let body = json!({
                "token": token,
                "readings": readings.iter().map(|(id, v)| json!({ "id": id, "value": v })).collect::<Vec<_>>(),
            });
            let sent = agent.post(&format!("http://127.0.0.1:{port}/hw")).send_json(&body).is_ok();
            // Nobody answering for a while: slow down (no busy loop), but
            // keep trying (she may come back).
            if sent {
                quiet_since = None;
            } else if quiet_since.is_none() {
                quiet_since = Some(Instant::now());
            }
            let slow = quiet_since.is_some_and(|t| t.elapsed() > Duration::from_secs(60));
            std::thread::sleep(Duration::from_secs(if slow { 10 } else { 2 }));
        }
    }

    fn append(path: &Path, line: &str) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "{} {line}", now_ms());
        }
    }

    // --- The elevated one-time setup ------------------------------------------------

    // A random 256-bit token, hex, from the system's RNG.
    fn new_token() -> String {
        let mut buf = [0u8; 32];
        // SAFETY: our own buffer; BCRYPT_USE_SYSTEM_PREFERRED_RNG (2).
        unsafe { BCryptGenRandom(0, buf.as_mut_ptr(), buf.len() as u32, 2) };
        buf.iter().map(|b| format!("{b:02x}")).collect()
    }

    fn sha256_file(path: &Path) -> std::io::Result<String> {
        let mut f = std::fs::File::open(path)?;
        let mut hash = Sha256::new();
        std::io::copy(&mut f, &mut hash)?;
        Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
    }

    // Only github.com and the host its releases redirect to.
    fn allowed(url: &str) -> bool {
        let u = url.to_ascii_lowercase();
        u.starts_with("https://github.com/") || u.starts_with("https://objects.githubusercontent.com/") || u.starts_with("https://release-assets.githubusercontent.com/")
    }

    // Downloaded to `to` and its SHA-256 checked; the file is removed if it
    // does not match.
    fn download_verify(url: &str, expect: &str, to: &Path) -> Result<(), String> {
        if !allowed(url) {
            return Err(format!("not an allowed download host: {url}"));
        }
        let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(600))).build().new_agent();
        let mut res = agent.get(url).header("user-agent", "wakuwaku").call().map_err(|e| e.to_string())?;
        let mut reader = res.body_mut().with_config().limit(64 * 1024 * 1024).reader();
        let mut file = std::fs::File::create(to).map_err(|e| e.to_string())?;
        std::io::copy(&mut reader, &mut file).map_err(|e| e.to_string())?;
        file.flush().map_err(|e| e.to_string())?;
        drop(file);
        let got = sha256_file(to).map_err(|e| e.to_string())?;
        if got.eq_ignore_ascii_case(expect) {
            Ok(())
        } else {
            let _ = std::fs::remove_file(to);
            Err(format!("SHA-256 {got}, not {expect}"))
        }
    }

    // The module blob for this CPU, put in her data folder from the official
    // release (the zip checked, a blob by its stem taken out with Windows'
    // own tar). Done once: a blob already there is kept.
    fn ensure_module(data: &Path, stem: &str, log: &dyn Fn(&str)) -> Result<(), String> {
        let kept = dir(data).join(format!("{stem}.bin"));
        if kept.exists() {
            return Ok(());
        }
        let temp = std::env::temp_dir().join("wakuwaku-pawnio");
        let _ = std::fs::create_dir_all(&temp);
        let zip = temp.join("modules.zip");
        log("downloading the PawnIO modules");
        download_verify(MODULES_URL, MODULES_SHA, &zip)?;
        let unzip = temp.join("modules");
        let _ = std::fs::remove_dir_all(&unzip);
        std::fs::create_dir_all(&unzip).map_err(|e| e.to_string())?;
        // Windows' own tar (bsdtar) extracts a zip.
        let out = std::process::Command::new("tar").args(["-xf"]).arg(&zip).arg("-C").arg(&unzip).output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!("tar: {}", String::from_utf8_lossy(&out.stderr)));
        }
        let found = find_by_stem(&unzip, stem).ok_or_else(|| format!("no {stem} in the modules zip"))?;
        std::fs::create_dir_all(dir(data)).map_err(|e| e.to_string())?;
        std::fs::copy(&found, &kept).map_err(|e| e.to_string())?;
        log(&format!("kept the module {stem}"));
        Ok(())
    }

    // A file under `root` (or a folder of it) whose name starts with `stem`
    // (its extension may be .bin or .amx).
    fn find_by_stem(root: &Path, stem: &str) -> Option<std::path::PathBuf> {
        let entries = std::fs::read_dir(root).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = find_by_stem(&path, stem) {
                    return Some(found);
                }
            } else if path.file_stem().and_then(|s| s.to_str()).is_some_and(|s| s.eq_ignore_ascii_case(stem)) {
                return Some(path);
            }
        }
        None
    }

    // PawnIO's own installer, downloaded (its SHA-256 checked) and run
    // silently; this process is elevated, so no further UAC.
    fn install_pawnio(log: &dyn Fn(&str)) -> Result<(), String> {
        let temp = std::env::temp_dir().join("wakuwaku-pawnio");
        let _ = std::fs::create_dir_all(&temp);
        let setup = temp.join("PawnIO_setup.exe");
        log("downloading the PawnIO installer");
        download_verify(PAWNIO_SETUP_URL, PAWNIO_SETUP_SHA, &setup)?;
        log("installing PawnIO");
        let status = std::process::Command::new(&setup).args(["-install", "-silent"]).status().map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&setup);
        if status.success() {
            Ok(())
        } else {
            Err(format!("the PawnIO installer exited with {status}"))
        }
    }

    // The setup (SETUP_FLAG, elevated): install PawnIO when asked, put the
    // module in her data, leave a fresh token, register and start the task.
    pub fn run_setup(port: u16, sid: &str, data: &Path, with_pawnio: bool) {
        let log_path = dir(data).join("setup.log");
        let log = |line: &str| append(&log_path, line);
        log("setup started");
        if with_pawnio && !pawnio_installed() {
            if let Err(e) = install_pawnio(&log) {
                log(&format!("PawnIO not installed: {e}"));
                return;
            }
        }
        if !pawnio_installed() {
            log("PawnIO is not installed: nothing to set up");
            return;
        }
        let (vendor, _, _) = cpu_id();
        let stem = match vendor {
            Vendor::Amd => AMD_MODULE,
            Vendor::Intel => INTEL_MODULE,
            Vendor::Other => {
                log("this CPU is neither AMD nor Intel: nothing to read");
                return;
            }
        };
        if let Err(e) = ensure_module(data, stem, &log) {
            log(&format!("the module could not be put in place: {e}"));
            return;
        }
        if let Err(e) = (|| -> std::io::Result<()> {
            std::fs::create_dir_all(dir(data))?;
            std::fs::write(dir(data).join("token"), new_token())
        })() {
            log(&format!("the token could not be written: {e}"));
            return;
        }
        // The helper runs with the highest rights at logon, so its exe must
        // sit where only administrators can write: her exe copied into a
        // protected %ProgramFiles% folder, the task pointed at the copy. Her
        // own exe usually lives where the user can write (a portable exe), so
        // the task must never run it directly.
        let Some(admin) = admin_dir() else {
            log("there is no ProgramFiles to put the helper in");
            return;
        };
        let source = crate::data::exe().unwrap_or_default();
        let copy = admin.join(HELPER_EXE);
        if let Err(e) = (|| -> Result<(), String> {
            std::fs::create_dir_all(&admin).map_err(|e| e.to_string())?;
            set_admin_only(&admin)?;
            if source != copy {
                std::fs::copy(&source, &copy).map_err(|e| e.to_string())?;
            }
            std::fs::write(admin.join("helper.ver"), HELPER_PROTOCOL.to_string()).map_err(|e| e.to_string())
        })() {
            log(&format!("the helper copy was not set up (left no task): {e}"));
            return;
        }
        log("the helper is in an administrator-only folder");
        let exe = copy.to_string_lossy().into_owned();
        let xml = task_xml(sid, &exe, &helper_args(port, data));
        match crate::logon::register(&task_name(sid), &xml) {
            Ok(()) => log("the task is registered"),
            Err(e) => {
                log(&format!("the task was not registered: {e}"));
                return;
            }
        }
        // Start it now; the Task Scheduler runs it at the highest rights.
        let run = std::process::Command::new("schtasks").args(["/Run", "/TN", &task_name(sid)]).output();
        match run {
            Ok(out) if out.status.success() => log("the task is started"),
            Ok(out) => log(&format!("the task was registered but did not start: {}", String::from_utf8_lossy(&out.stderr))),
            Err(e) => log(&format!("the task was registered but did not start: {e}")),
        }
    }

    // The removal (REMOVE_FLAG, elevated): the task taken away and the token
    // deleted, so the running helper stops; PawnIO is left installed.
    pub fn run_remove(sid: &str, data: &Path) {
        let log = |line: &str| append(&dir(data).join("setup.log"), line);
        // The running helper ended first: it only notices its token is gone
        // on its next round (up to 10 s while she is away), and until then
        // its exe is held open, so the copy could not be deleted (seen in
        // the test VM: the folder stayed with the exe in it).
        let _ = std::process::Command::new("schtasks").args(["/End", "/TN", &task_name(sid)]).output();
        match crate::logon::remove(&task_name(sid)) {
            Ok(true) => log("the task is removed"),
            Ok(false) => log("there was no task to remove"),
            Err(e) => log(&format!("the task was not removed: {e}")),
        }
        let _ = std::fs::remove_file(dir(data).join("token"));
        log("the token is deleted: a helper still running stops");
        // The admin-only helper copy too (PawnIO stays: it is not hers),
        // tried for a while as the ended helper's exe is let go; then the
        // Wakuwaku folder above it, if nothing else is in it.
        if let Some(admin) = admin_dir() {
            let mut gone = false;
            for _ in 0..30 {
                match std::fs::remove_dir_all(&admin) {
                    Ok(()) => gone = true,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => gone = true,
                    Err(_) => std::thread::sleep(Duration::from_millis(500)),
                }
                if gone {
                    break;
                }
            }
            if let Some(parent) = admin.parent() {
                let _ = std::fs::remove_dir(parent);
            }
            log(if gone { "the helper copy is removed" } else { "the helper copy could not be removed (still in use)" });
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use std::path::Path;

    pub fn pawnio_installed() -> bool {
        false
    }
    pub fn task_present(_data: &Path) -> bool {
        false
    }
    pub fn installed_protocol() -> Option<u32> {
        None
    }
    pub fn run_helper(_port: u16, _data: &Path) {}
    pub fn run_setup(_port: u16, _sid: &str, _data: &Path, _with_pawnio: bool) {}
    pub fn run_remove(_sid: &str, _data: &Path) {}
}

pub use imp::{run_helper, run_remove, run_setup};

#[cfg(test)]
mod tests {
    use super::*;

    // Every label the readings carry, and every line the switch shows, is in
    // her strings in both languages.
    #[test]
    fn its_words_are_in_both_languages() {
        let strings = include_str!("../../src/shared/i18n.ts");
        let mut keys = vec!["hw.cpuTemp", "hw.cpuPower", "sensors.title", "sensors.note", "sensors.settingUp", "sensors.running", "sensors.pawnioMissing", "sensors.notAnswering", "sensors.needsUpdate", "sensors.removeNote"];
        keys.extend(CCD_LABELS);
        for key in keys {
            assert_eq!(strings.matches(&format!("'{key}':")).count(), 2, "{key}");
        }
    }

    // The switch's state from what is known: off until PawnIO is there, then
    // set up, then running once a reading comes; a helper that posts but
    // reads nothing (a VM) is "not answering", not "running".
    #[test]
    fn the_switch_state_follows_what_is_known() {
        // installed, task, answering, setting_up, has_temp, needs_update.
        assert_eq!(state_name(false, false, false, false, false, false), "pawnioMissing");
        assert_eq!(state_name(true, false, false, false, false, false), "off");
        assert_eq!(state_name(false, false, false, true, false, false), "settingUp");
        assert_eq!(state_name(true, true, true, false, true, false), "running");
        // Answering but no temperature yet (a Hyper-V guest): not answering.
        assert_eq!(state_name(true, true, true, false, false, false), "notAnswering");
        // The task is there but nothing comes.
        assert_eq!(state_name(true, true, false, false, false, false), "notAnswering");
        // Still setting up while the task is registering.
        assert_eq!(state_name(true, true, false, true, false, false), "settingUp");
        // An old helper copy the task runs: update it, even if it answers.
        assert_eq!(state_name(true, true, true, false, true, true), "needsUpdate");
        assert_eq!(state_name(true, false, false, false, false, true), "off");
    }

    // The helper's folder is set administrator-only, and that is checked: a
    // folder a user can write is refused, because the task runs its exe
    // elevated at every logon.
    #[test]
    fn the_helper_folder_must_be_administrator_only() {
        assert!(acl_is_admin_only(admin_only_sddl()), "{}", admin_only_sddl());
        // As Windows renders it on read-back (auto-inherited flag, 0x1200a9
        // for FRFX): still admin-only.
        assert!(acl_is_admin_only("D:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;0x1200a9;;;BU)"));
        // Users granted full or write: refused.
        assert!(!acl_is_admin_only("D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FA;;;BU)"));
        assert!(!acl_is_admin_only("D:P(A;OICI;FA;;;BA)(A;OICI;FW;;;BU)"));
        assert!(!acl_is_admin_only("D:P(A;OICI;FA;;;BA)(A;OICI;FA;;;WD)"));
        // Not protected (inherits from the parent, which could be loose): refused.
        assert!(!acl_is_admin_only("D:AI(A;OICI;FA;;;SY)(A;OICI;FRFX;;;BU)"));
        assert!(!acl_is_admin_only("not a dacl"));
        // The helper's protocol: an older copy needs an update, the same or
        // newer does not, none installed does not.
        assert!(needs_update(Some(0), 1) && needs_update(Some(1), 2));
        assert!(!needs_update(Some(1), 1) && !needs_update(Some(2), 1) && !needs_update(None, 1));
    }

    #[test]
    fn ids_and_labels_line_up() {
        assert_eq!(label_of("/cpu/0/temperature/0"), Some("hw.cpuTemp"));
        assert_eq!(label_of("/cpu/0/temperature/1"), Some("hw.ccd1"));
        assert_eq!(label_of("/cpu/0/temperature/8"), Some("hw.ccd8"));
        assert_eq!(label_of("/cpu/0/temperature/9"), None);
        assert_eq!(label_of("/cpu/0/power/0"), Some("hw.cpuPower"));
        assert_eq!(label_of("/cpu/0/load/0"), None);
        assert_eq!(label_of("/gpu-nvidia/0/temperature/0"), None);
        assert_eq!(kind_of("/cpu/0/temperature/2"), Some(Kind::Temperature));
        assert_eq!(kind_of("/cpu/0/power/0"), Some(Kind::Power));
        assert_eq!(kind_of("/cpu/0/clock/0"), None);
        assert_eq!((package_temp_id(), ccd_id(2), power_id()), ("/cpu/0/temperature/0".into(), "/cpu/0/temperature/2".into(), "/cpu/0/power/0".into()));
    }

    // The Ryzen 9 9955HX's THM register (LHM reads 0x5F60_0000 -> (0x2FB >>
    // ... ) here a worked value): bits [31:21] at 0x1FB = 507 steps of 0.125
    // = 63.375 °C, no offset.
    #[test]
    fn amd_temperature_decodes_as_lhm_does() {
        // 507 << 21 = raw; (507 * 125) * 0.001 = 63.375.
        let raw = 507u32 << 21;
        assert!((amd_tctl(raw) - 63.375).abs() < 1e-9, "{}", amd_tctl(raw));
        // The range bit set: 49 °C comes off.
        assert!((amd_tctl(raw | 0x8_0000) - (63.375 - 49.0)).abs() < 1e-9);
        // TjSel both bits set: the same 49 °C.
        assert!((amd_tctl(raw | 0x3_0000) - (63.375 - 49.0)).abs() < 1e-9);
        // A CCD at ~50 °C: (50000 + 305000) / 125 = 2840 = 0xB18. The valid
        // bit (0x800) sits inside the temperature field, so any real reading
        // has it set (LHM uses & 0xFFF including it).
        assert!((amd_ccd(0x0B18, false).unwrap() - 50.0).abs() < 1e-9);
        assert!((amd_ccd(0x0B18, true).unwrap() - 50.0).abs() < 1e-9);
        // Absent: non-Turin reads 0 (or 0xFFF, which decodes above 125 °C and
        // is dropped); Turin has the valid bit clear.
        assert_eq!(amd_ccd(0, false), None);
        assert_eq!(amd_ccd(0x0FFF, false), None);
        assert_eq!(amd_ccd(0, true), None);
        // The 9955HX (family 1Ah, model 44h) uses the Granite Ridge map.
        assert_eq!(amd_ccd_plan(0x1A, 0x44), Some((0x0005_9B08, 8, false)));
        // Turin (family 1Ah, model <= 2Fh): 16 CCDs, a valid bit.
        assert_eq!(amd_ccd_plan(0x1A, 0x08), Some((0x0005_99F0, 16, true)));
        // Zen 2/3 desktop.
        assert_eq!(amd_ccd_plan(0x17, 0x71), Some((0x0005_9954, 8, false)));
        // A family without per-CCD temperatures.
        assert_eq!(amd_ccd_plan(0x17, 0x08), None);
    }

    // Intel: TjMax 100 °C (0x64 in bits 23:16), the package 30 below it (bit
    // 31 set, bits 22:16 = 30) -> 70 °C.
    #[test]
    fn intel_temperature_decodes_as_lhm_does() {
        let target = 0x64u32 << 16;
        assert_eq!(tjmax_of(target), 100);
        let status = 0x8000_0000 | (30u32 << 16);
        assert_eq!(intel_package_temp(status, tjmax_of(target)), Some(70.0));
        // Not valid (bit 31 clear): none.
        assert_eq!(intel_package_temp(30u32 << 16, 100), None);
        assert_eq!(intel_package_temp(status, 0), None);
    }

    // RAPL: the default ESU 0x10 is 15.3 µJ an increment; a counter that
    // climbed by 65536 increments over 2 s is 0.5 W.
    #[test]
    fn power_is_an_energy_rate_with_wraparound() {
        let unit = energy_unit(0x10 << 8);
        assert!((unit - 0.5f64.powi(16)).abs() < 1e-18);
        let watts = energy_rate(unit, 0, 65536, 2.0).unwrap();
        assert!((watts - 0.5).abs() < 1e-9, "{watts}");
        // The 32-bit counter wrapped: the difference is still right.
        let near = u32::MAX - 100;
        let after = near.wrapping_add(200);
        assert_eq!(after, 99);
        let watts = energy_rate(unit, near, after, 1.0).unwrap();
        assert!((watts - unit * 200.0).abs() < 1e-9, "{watts}");
        // No time passed: none.
        assert_eq!(energy_rate(unit, 0, 10, 0.0), None);
        // Intel's unit register the same bits.
        assert!((energy_unit(0x0005_0E03) - 0.5f64.powi(14)).abs() < 1e-18);
    }

    #[test]
    fn the_task_runs_the_helper_elevated_at_logon() {
        let exe = r"D:\Program Files\张三 & co\wakuwaku.exe";
        let args = helper_args(47213, std::path::Path::new(r"C:\data\x"));
        assert_eq!(args, "--wakuwaku-sensors 47213 \"C:\\data\\x\"");
        let xml = task_xml("S-1-5-21-1-2-3-1001", exe, &args);
        assert!(xml.contains("<UserId>S-1-5-21-1-2-3-1001</UserId>"));
        assert!(xml.contains("<RunLevel>HighestAvailable</RunLevel>"));
        assert!(xml.contains(r"<Command>D:\Program Files\张三 &amp; co\wakuwaku.exe</Command>"));
        assert!(xml.contains("--wakuwaku-sensors 47213"));
        // Started again if it fails, on battery too, never ended for running long.
        assert!(xml.contains("<RestartOnFailure>"));
        assert!(xml.contains("<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>"));
        assert!(xml.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"));
        assert_eq!(task_name("S-1-5-21-9"), "Wakuwaku sensors-S-1-5-21-9");
    }

    // The token is checked, and only a reading with a known id and a finite
    // value is kept.
    #[test]
    fn only_her_helper_with_the_token_is_accepted() {
        let tmp = std::env::temp_dir().join(format!("wkhw-{}", std::process::id()));
        let _ = std::fs::create_dir_all(dir(&tmp));
        std::fs::write(dir(&tmp).join("token"), "abc123").unwrap();
        assert!(token_ok(&tmp, "abc123"));
        assert!(!token_ok(&tmp, "abc124"));
        assert!(!token_ok(&tmp, "abc1234"));
        assert!(!token_ok(&tmp, ""));
        let body = json!({ "token": "abc123", "readings": [
            { "id": "/cpu/0/temperature/0", "value": 63.5 },
            { "id": "/cpu/0/power/0", "value": 42.0 },
            { "id": "/cpu/0/temperature/1", "value": 60.0 },
            { "id": "/cpu/0/load/0", "value": 99.0 },
            { "id": "/cpu/0/power/0", "value": f64::INFINITY }
        ]});
        assert!(accept(&tmp, &body));
        let joined = crate::hw::joined_now();
        let ids: Vec<&str> = joined.iter().map(|r| r.id.as_str()).collect();
        assert!(ids.contains(&"/cpu/0/temperature/0") && ids.contains(&"/cpu/0/power/0") && ids.contains(&"/cpu/0/temperature/1"));
        assert!(!ids.contains(&"/cpu/0/load/0"), "an unknown id is dropped");
        assert_eq!(*READING.lock().unwrap(), (Some(63.5), None));
        // The wrong token: nothing taken.
        assert!(!accept(&tmp, &json!({ "token": "nope", "readings": [] })));
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
