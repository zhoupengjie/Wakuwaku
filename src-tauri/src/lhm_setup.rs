// LibreHardwareMonitor got, set up and started for the monitor (#18, part
// C): the monitor's other source (monitorSource "lhm"; lhm.rs reads it) for
// someone who does not have it, or has it with its web server off. Nothing
// here runs but on the person's click in the settings (settings_lhm), and
// LibreHardwareMonitor (LHM) stays a program of its own, with its own
// licence (MPL-2.0), in a folder they can remove it from (or she does).
//
// Not found anywhere: 下载 LibreHardwareMonitor. Its latest release on GitHub
// (the API's releases/latest), the asset that needs nothing installed (choose:
// its .NET Framework 4.7.2 build, "LibreHardwareMonitor.zip" since 0.9.5,
// "-net472.zip" before, which Windows 10 and 11 run as they are; the .NET 10
// build only if a release had no other and the .NET desktop runtime it needs
// is here), downloaded into her data folder only with the SHA-256 GitHub
// gives for it and checked against it (none given: not downloaded); then one
// step of her exe as administrator (admin, below: the one question Windows
// asks) checks it again, unpacks it (zip::extract: stored and deflated
// entries, every name checked, no path out of the folder) into
// %ProgramFiles%\LibreHardwareMonitor, a folder only administrators can
// change (DACL_SDDL), writes its config before its first start
// (config::turned_on), makes its task if asked, and starts it as
// administrator (its manifest asks for it: requireAdministrator).
//
// Its config is LibreHardwareMonitor.config beside its exe (PersistentSettings.cs:
// <configuration><appSettings><add key value/>; the exe's .config, which
// is another file than the .NET Framework's LibreHardwareMonitor.exe.config),
// read once as it starts and written again as it ends (SaveConfiguration),
// so a running LHM's is never changed. Its keys (MainForm.cs at v0.9.6):
// runWebServerMenuItem ("true": Options → Remote Web Server → Run),
// listenerPort, listenerIp (an address of the computer's own name, else "+",
// every one: 0.9.6 takes no 127.0.0.1, which it turns into "+"; LHM built
// since takes it, and so listens on this computer alone: written for an LHM
// of hers, and in place of another's set to one address 127.0.0.1 cannot
// reach), startMinMenuItem and minTrayMenuItem (started in the tray, its
// window not shown: hers only). Its Basic authentication is left as it is:
// 0.9.6 keeps its password's SHA-256 and takes that as the password the next
// time (HttpServer's Password hashed again), so one survives no restart.
// PawnIO (the driver it reads the CPU's temperatures through) has no key:
// without it (HKLM's Uninstall\PawnIO DisplayVersion, as LHM looks) or
// older than 2.0, LHM asks in a message box of its own before its web
// server starts, each time it starts; the settings say so.
//
// Its own "Run On Windows Startup" (Options): StartupManager.cs, run as
// administrator, makes the Task Scheduler's task "LibreHardwareMonitor"
// (root folder; at logon, the highest rights, the interactive user, no time
// limit, on battery too; its exe, unquoted, and its folder) and ticks the
// menu while that task runs its exe. Offered beside the download for hers
// (lhmStartup, on unless turned off): the same task (task_xml, for this
// user's logon), made in the step as administrator, which then runs it
// (logon::run); at every logon after, the task starts LHM with no question,
// and LHM's own menu can turn it off. Without it every start of LHM is a
// question of Windows'. Hers alone: a task with the highest rights must
// start an exe only administrators can change.
//
// Found (running, its task's exe, Program Files' folder for it, the Run
// value LHM writes without administrator, winget's or Scoop's folder) but
// not answering: stopped, a button turns its web server on in its config
// (hers by the step as administrator; someone's own as she may, their config
// backed up first: <config>.wakuwaku.bak) and starts it; running, the steps
// in LHM's own menu, or a button for the port its config says. Someone's own
// stays theirs: she says where it is.
//
// For tests: WAKUWAKU_LHM_RELEASE_URL, the release's JSON from elsewhere
// (its downloads then taken from anywhere); WAKUWAKU_TEST_ELEVATE=<folder>,
// what would be run as administrator written there (elevate-<pid>-<n>.json)
// and its exit code waited for (elevate-<pid>-<n>.done), for a test machine where no
// one can answer Windows' question.
#![cfg_attr(not(windows), allow(dead_code))]

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{zip, Shared};
use crate::lock::Lock;

const LATEST: &str = "https://api.github.com/repos/LibreHardwareMonitor/LibreHardwareMonitor/releases/latest";
const DOWNLOADS: &str = "https://github.com/LibreHardwareMonitor/LibreHardwareMonitor/releases/download/";
const RELEASE_TEST: &str = "WAKUWAKU_LHM_RELEASE_URL";
const ELEVATE_TEST: &str = "WAKUWAKU_TEST_ELEVATE";
// LHM's task (StartupManager.cs: nameof(LibreHardwareMonitor)).
pub const TASK: &str = "LibreHardwareMonitor";
// Hers, in Program Files (install_dir): a folder only administrators can
// change, as the one its task starts with the highest rights must be.
const FOLDER: &str = "LibreHardwareMonitor";
// In that folder, saying she put it there (admin): what she may replace,
// change and remove; one without it is someone's own.
const MARKER: &str = "wakuwaku-installed.json";
// Her exe with this, run as administrator, does that one step (admin): the
// one way she writes there.
pub const ADMIN_FLAG: &str = "--wakuwaku-lhm";
// That folder's DACL, protected from Program Files' own: SYSTEM and the
// Administrators all, Users read and run (FILE_GENERIC_READ |
// FILE_GENERIC_EXECUTE), each to its files and folders; its owner the
// Administrators (OWNER_SDDL with it, as administrator only).
pub const DACL_SDDL: &str = "D:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;0x1200a9;;;BU)";
const OWNER_SDDL: &str = "O:BA";
// Its exe as released, and as built from its source since (the project renamed).
const EXES: [&str; 2] = ["LibreHardwareMonitor.exe", "LibreHardwareMonitor.Windows.Forms.exe"];
const MAX_ZIP: u64 = 64 << 20;
// Its web server's answer waited for after its start: its first start may
// wait on its PawnIO question.
const ANSWER_WAIT: Duration = Duration::from_secs(180);
// What is found looked at again so often while the settings show it.
const LOOK_EVERY: Duration = Duration::from_secs(5);
const LOOK_EVERY_FOUND: Duration = Duration::from_secs(30);

fn error(code: &str, detail: impl Into<String>) -> Value {
    json!({ "code": code, "detail": detail.into() })
}

// --- The release's asset --------------------------------------------------------------

// What a build needs to run: the .NET Framework (4.7.2, in Windows 10 since
// 1803 and in 11), or that major version of .NET's desktop runtime.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Runtime {
    Framework,
    Net(u32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
    // GitHub's SHA-256 of it, lowercase hex (assets since mid-2025 have one).
    pub digest: Option<String>,
    pub runtime: Runtime,
}

// What an asset's name says it needs; None for what is not LHM's program
// (its library, sources, symbols): "LibreHardwareMonitor.NET.10.zip" .NET
// 10's, "LibreHardwareMonitor-net472.zip" and the plain
// "LibreHardwareMonitor.zip" the .NET Framework's (the release's main build,
// the one winget installs).
pub fn runtime_of(name: &str) -> Option<Runtime> {
    let n = name.to_ascii_lowercase();
    if !n.ends_with(".zip") || !n.starts_with("librehardwaremonitor") || n.starts_with("librehardwaremonitorlib") {
        return None;
    }
    if ["source", "symbols", "pdb", "debug"].iter().any(|w| n.contains(w)) {
        return None;
    }
    let mut from = 0;
    while let Some(at) = n[from..].find("net").map(|i| from + i) {
        let rest = n[at + 3..].trim_start_matches(['.', '-', '_', ' ']);
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if digits.starts_with('4') && digits.len() >= 2 {
            return Some(Runtime::Framework);
        }
        if let Some(major) = digits.parse::<u32>().ok().filter(|&m| (5..100).contains(&m)) {
            return Some(Runtime::Net(major));
        }
        from = at + 3;
    }
    Some(Runtime::Framework)
}

// The processor an asset's name is for, if it says: Rust's names.
fn arch_of(name: &str) -> Option<&'static str> {
    let n = name.to_ascii_lowercase();
    if n.contains("arm64") || n.contains("aarch64") {
        Some("aarch64")
    } else if n.contains("x64") || n.contains("amd64") || n.contains("x86_64") || n.contains("x86-64") {
        Some("x86_64")
    } else if n.contains("x86") {
        Some("x86")
    } else {
        None
    }
}

fn sha256_hex(h: &str) -> Option<String> {
    let h = h.trim();
    (h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit())).then(|| h.to_ascii_lowercase())
}

// The asset to download from a release (GitHub's JSON): of LHM's program,
// for this processor (or any), the one that needs nothing installed first,
// else a .NET build whose desktop runtime is here (`desktop`: the major
// versions of Microsoft.WindowsDesktop.App found).
pub fn choose(release: &Value, desktop: &[u32], arch: &str) -> Option<Asset> {
    let assets = release["assets"].as_array()?;
    assets
        .iter()
        .filter_map(|a| {
            let name = a["name"].as_str()?;
            let runtime = runtime_of(name)?;
            let fits = arch_of(name);
            if fits.is_some_and(|f| f != arch) {
                return None;
            }
            let rank = match runtime {
                Runtime::Framework => 0,
                Runtime::Net(major) if desktop.contains(&major) => 1,
                Runtime::Net(_) => return None,
            };
            let asset = Asset {
                name: name.to_string(),
                url: a["browser_download_url"].as_str()?.to_string(),
                size: a["size"].as_u64().unwrap_or(0),
                digest: a["digest"].as_str().and_then(|d| d.strip_prefix("sha256:")).and_then(sha256_hex),
                runtime,
            };
            Some(((rank, fits.is_none()), asset))
        })
        .min_by_key(|(rank, _)| *rank)
        .map(|(_, asset)| asset)
}

// A release's version: its tag without the v; none for a draft or a prerelease.
pub fn version_of(release: &Value) -> Option<String> {
    if release["draft"] == true || release["prerelease"] == true {
        return None;
    }
    let v = release["tag_name"].as_str()?.trim().trim_start_matches(['v', 'V']).to_string();
    (!v.is_empty()).then_some(v)
}

// --- Its config ---------------------------------------------------------------------

pub mod config {
    // Its web server as a config has it.
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct WebServer {
        pub on: Option<bool>,
        pub port: Option<u16>,
        pub ip: Option<String>,
    }

    impl WebServer {
        // Whether 127.0.0.1 reaches it at `port`: on, at that port, and
        // listening on every address or this computer's alone (or on one it
        // does not have, which LHM turns into every one).
        pub fn serves(&self, port: u16) -> bool {
            self.on == Some(true) && self.port.unwrap_or(crate::lhm::PORT) == port && self.ip.as_deref().is_none_or(reaches_loopback)
        }
    }

    // A listenerIp 127.0.0.1 can reach: LHM's "?" (never set), "+" (every
    // one), 0.0.0.0 (offered by its dialog, turned into "+"), this
    // computer's own. Another address is one of its network's, which a
    // request to 127.0.0.1 does not match.
    pub fn reaches_loopback(ip: &str) -> bool {
        matches!(ip.trim(), "" | "?" | "+" | "*" | "0.0.0.0" | "127.0.0.1" | "localhost" | "::1" | "[::1]")
    }

    fn unescape(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut rest = s;
        while let Some(at) = rest.find('&') {
            out.push_str(&rest[..at]);
            rest = &rest[at..];
            let Some(end) = rest.find(';').filter(|&e| e <= 10) else {
                out.push('&');
                rest = &rest[1..];
                continue;
            };
            let name = &rest[1..end];
            let ch = match name {
                "lt" => Some('<'),
                "gt" => Some('>'),
                "amp" => Some('&'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ if name.starts_with("#x") || name.starts_with("#X") => u32::from_str_radix(&name[2..], 16).ok().and_then(char::from_u32),
                _ if name.starts_with('#') => name[1..].parse().ok().and_then(char::from_u32),
                _ => None,
            };
            match ch {
                Some(c) => {
                    out.push(c);
                    rest = &rest[end + 1..];
                }
                None => {
                    out.push('&');
                    rest = &rest[1..];
                }
            }
        }
        out.push_str(rest);
        out
    }

    // As .NET's XmlDocument writes an attribute's value.
    fn escape(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '"' => out.push_str("&quot;"),
                '\n' => out.push_str("&#xA;"),
                '\r' => out.push_str("&#xD;"),
                '\t' => out.push_str("&#x9;"),
                c => out.push(c),
            }
        }
        out
    }

    // An element's attributes, from just after its name to its end.
    fn attributes(tag: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut rest = tag;
        while let Some(eq) = rest.find('=') {
            let name = rest[..eq].trim().to_string();
            let after = rest[eq + 1..].trim_start();
            let Some(quote) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else { break };
            let Some(close) = after[1..].find(quote) else { break };
            out.push((name, unescape(&after[1..1 + close])));
            rest = &after[close + 2..];
        }
        out
    }

    // Its keys and values, in its order; None for what is not LHM's config
    // (no <configuration> with an <appSettings>).
    pub fn read(text: &str) -> Option<Vec<(String, String)>> {
        let mut text = text.trim_start_matches('\u{feff}').to_string();
        // Comments away (an <add> in one is not one).
        while let Some(at) = text.find("<!--") {
            let end = text[at..].find("-->").map_or(text.len(), |e| at + e + 3);
            text.replace_range(at..end, "");
        }
        let root = text.find("<configuration")?;
        let at = root + text[root..].find("<appSettings")?;
        let open_end = at + text[at..].find('>')?;
        if text[..open_end].ends_with('/') {
            return Some(Vec::new());
        }
        let close = open_end + text[open_end..].find("</appSettings>")?;
        let inside = &text[open_end + 1..close];
        let mut pairs: Vec<(String, String)> = Vec::new();
        let mut rest = inside;
        while let Some(i) = rest.find("<add") {
            let after = &rest[i + 4..];
            if !after.starts_with(|c: char| c.is_whitespace() || c == '/' || c == '>') {
                rest = after;
                continue;
            }
            let end = after.find('>').unwrap_or(after.len());
            let attrs = attributes(after[..end].trim_end_matches('/'));
            let get = |name: &str| attrs.iter().find(|(n, _)| n == name).map(|(_, v)| v.clone());
            if let (Some(key), Some(value)) = (get("key"), get("value")) {
                // LHM's Load adds each, the same key twice failing it: the first kept.
                if !pairs.iter().any(|(k, _)| *k == key) {
                    pairs.push((key, value));
                }
            }
            rest = &after[end.min(after.len())..];
        }
        Some(pairs)
    }

    // Written as LHM writes it (PersistentSettings.Save: UTF-8 with its
    // mark, indented by two).
    pub fn write(pairs: &[(String, String)]) -> String {
        let mut out = String::from("\u{feff}<?xml version=\"1.0\" encoding=\"utf-8\"?>\r\n<configuration>\r\n  <appSettings>\r\n");
        for (k, v) in pairs {
            out.push_str(&format!("    <add key=\"{}\" value=\"{}\" />\r\n", escape(k), escape(v)));
        }
        out.push_str("  </appSettings>\r\n</configuration>");
        out
    }

    fn get<'a>(pairs: &'a [(String, String)], key: &str) -> Option<&'a str> {
        pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    fn set(pairs: &mut Vec<(String, String)>, key: &str, value: &str) {
        match pairs.iter_mut().find(|(k, _)| k == key) {
            Some(pair) => pair.1 = value.to_string(),
            None => pairs.push((key.to_string(), value.to_string())),
        }
    }

    pub fn web_server(pairs: &[(String, String)]) -> WebServer {
        WebServer {
            on: get(pairs, "runWebServerMenuItem").map(|v| v == "true"),
            port: get(pairs, "listenerPort").and_then(|p| p.trim().parse().ok()).filter(|&p| p > 0),
            ip: get(pairs, "listenerIp").map(str::to_string),
        }
    }

    // The config with its web server on at `port` where 127.0.0.1 reaches
    // it, the rest as it was (`existing`: none, or one that is not LHM's,
    // which LHM itself would throw away); `hers`: an LHM she downloaded,
    // started in the tray too, listening on this computer alone where LHM
    // can.
    pub fn turned_on(existing: Option<&str>, port: u16, hers: bool) -> String {
        let mut pairs = existing.and_then(read).unwrap_or_default();
        set(&mut pairs, "runWebServerMenuItem", "true");
        set(&mut pairs, "listenerPort", &port.to_string());
        let ip = get(&pairs, "listenerIp").map(str::to_string);
        if ip.is_none() && hers || ip.is_some_and(|ip| !reaches_loopback(&ip)) {
            set(&mut pairs, "listenerIp", "127.0.0.1");
        }
        if hers {
            set(&mut pairs, "startMinMenuItem", "true");
            set(&mut pairs, "minTrayMenuItem", "true");
        }
        write(&pairs)
    }
}

// --- Its task ----------------------------------------------------------------------

fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

// LHM's own task as its StartupManager makes it (at logon, the highest
// rights, no time limit, on battery too, started late if missed; its exe
// unquoted, as LHM compares it with its own path to tick its menu), for
// this user's logon (`user`: their SID) where LHM's is anyone's.
pub fn task_xml(user: &str, exe: &str, dir: &str) -> String {
    let (user, exe, dir) = (escape_xml(user), escape_xml(exe), escape_xml(dir));
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Starts LibreHardwareMonitor on Windows startup.</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{user}</UserId>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{user}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>false</AllowHardTerminate>
    <StartWhenAvailable>true</StartWhenAvailable>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Enabled>true</Enabled>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{exe}</Command>
      <WorkingDirectory>{dir}</WorkingDirectory>
    </Exec>
  </Actions>
</Task>
"#
    )
}

// --- What is found, and what the settings offer -------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct Install {
    pub exe: PathBuf,
    // Where it was found: running, task, hers, run (the Run value), winget, scoop.
    pub from: &'static str,
    pub hers: bool,
    // Its config there, and its web server as that says.
    pub config: bool,
    pub web: config::WebServer,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Seen {
    pub install: Option<Install>,
    pub running: bool,
    // The exe LHM's task runs.
    pub task: Option<PathBuf>,
    // PawnIO's version, if it is installed.
    pub pawnio: Option<String>,
}

impl Seen {
    // LHM's task runs this one.
    pub fn starts_at_logon(&self) -> bool {
        matches!((&self.task, &self.install), (Some(task), Some(i)) if same_path(task, &i.exe))
    }
}

fn same_path(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| p.to_string_lossy().replace('/', "\\").trim_matches('"').to_lowercase();
    norm(a) == norm(b)
}

// What the settings offer, LHM being the monitor's source: busy (something
// under way), answering (its web server answers, or wants a password, or
// something else is on the port: lhm.rs says which), get (none found:
// download it), start (stopped, its web server on as hers would reach it),
// turnOn (stopped, its web server to turn on first), otherPort (running,
// its config's web server on another port), steps (running, not answering:
// what to do in LHM).
pub fn step(status: &str, busy: bool, seen: &Seen, port: u16) -> &'static str {
    if busy {
        return "busy";
    }
    if matches!(status, "found" | "auth" | "strange") {
        return "answering";
    }
    if seen.running {
        let other = seen.install.as_ref().and_then(|i| i.web.port.filter(|&p| i.web.on == Some(true) && p != port && config::reaches_loopback(i.web.ip.as_deref().unwrap_or(""))));
        return if other.is_some() { "otherPort" } else { "steps" };
    }
    match &seen.install {
        None => "get",
        Some(i) if i.web.serves(port) => "start",
        Some(_) => "turnOn",
    }
}

// The installs looked for, in order: the first found is the one (Program
// Files' own folder for LHM, hers or someone's, after a running one and
// its task's).
fn pick(running: Option<PathBuf>, task: Option<PathBuf>, program_files: Option<PathBuf>, others: Vec<(&'static str, PathBuf)>) -> Option<(PathBuf, &'static str)> {
    running
        .map(|p| (p, "running"))
        .into_iter()
        .chain(task.filter(|p| p.is_file()).map(|p| (p, "task")))
        .chain(program_files.map(|p| (p, "programFiles")))
        .chain(others.into_iter().map(|(from, p)| (p, from)))
        .next()
}

// --- Its state, for the settings -------------------------------------------------------

#[derive(Clone, Debug)]
struct Job {
    stage: &'static str,
    got: u64,
    total: u64,
    version: String,
    asset: String,
}

#[derive(Default)]
struct State {
    job: Option<Job>,
    error: Option<Value>,
    seen: Seen,
    looked: Option<Instant>,
    looking: bool,
    // The settings drawn again for the download's progress no more often.
    pushed: Option<Instant>,
}

static STATE: Lock<Option<State>> = Lock::new("lhm_setup::STATE", None);

fn with<T>(f: impl FnOnce(&mut State) -> T) -> T {
    let mut s = STATE.lock().unwrap();
    f(s.get_or_insert_with(State::default))
}

// Where the download waits for the step as administrator (in her data folder).
pub fn tools(sh: &Shared) -> PathBuf {
    sh.dir.join("tools")
}

fn port(sh: &Shared) -> u16 {
    crate::lhm::Conf::from(&sh.config().monitor).port
}

// What the settings show of it: the step offered (looking, till it was
// first looked for), where hers would go, what was found, a job under way,
// what went wrong. `status`: lhm.rs's (its web server answering or not);
// `shown`: LHM the source on the open settings, so what is found is looked
// at again now and then (on a thread of its own: the processes, its task,
// its config).
pub fn view(sh: &Shared, status: &str, shown: bool) -> Value {
    let port = port(sh);
    let (seen, job, error, stale, looking, looked) = with(|s| {
        let every = if status == "found" { LOOK_EVERY_FOUND } else { LOOK_EVERY };
        (s.seen.clone(), s.job.clone(), s.error.clone(), s.looked.is_none_or(|t| t.elapsed() >= every), s.looking, s.looked.is_some())
    });
    if shown && stale && !looking && job.is_none() {
        look_soon(&crate::shared(&sh.app));
    }
    let offered = step(status, job.is_some(), &seen, port);
    // LHM is Windows' alone: nothing offered elsewhere.
    let step = if !cfg!(windows) {
        "off"
    } else if !looked && !matches!(offered, "answering" | "busy") {
        "looking"
    } else {
        offered
    };
    let install = seen.install.as_ref().map(|i| {
        json!({
            "exe": i.exe.to_string_lossy(),
            "dir": i.exe.parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default(),
            "shown": crate::connection::shown_path(i.exe.parent().unwrap_or(&i.exe)),
            "from": i.from,
            "hers": i.hers,
            "config": i.config,
            "webServer": i.web.on,
            "port": i.web.port,
        })
    });
    json!({
        "step": step,
        // Where hers goes: Program Files' (only administrators change it).
        "dir": install_dir().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default(),
        "install": install,
        "running": seen.running,
        "startup": seen.starts_at_logon(),
        "taskOther": seen.task.as_ref().filter(|_| !seen.starts_at_logon()).map(|t| t.to_string_lossy().into_owned()),
        "pawnio": seen.pawnio,
        "startupWanted": sh.config().monitor["lhmStartup"] != false,
        "job": job.map(|j| json!({ "stage": j.stage, "got": j.got, "total": j.total, "version": j.version, "asset": j.asset })),
        "error": error,
    })
}

// Looked at again on a thread of its own; the settings drawn again if that changed.
fn look_soon(sh: &Arc<Shared>) {
    with(|s| s.looking = true);
    let sh = sh.clone();
    std::thread::spawn(move || {
        let seen = look(&sh);
        // Running and not (yet) answering: asked now, not at the monitor's
        // next try (30 s apart while not found), before the steps are shown.
        if seen.running && crate::lhm::view()["status"] != "found" {
            crate::lhm::ask_now();
        }
        let changed = with(|s| {
            s.looking = false;
            s.looked = Some(Instant::now());
            let changed = s.seen != seen;
            s.seen = seen;
            changed
        });
        if changed {
            sh.push_settings();
        }
    });
}

// What is on this computer of LHM's now.
fn look(_sh: &Shared) -> Seen {
    let dir = install_dir();
    let running = imp::running();
    let task = crate::logon::xml(TASK).ok().flatten().map(|x| PathBuf::from(crate::logon::seen(&x).program)).filter(|p| !p.as_os_str().is_empty());
    let program_files = dir.as_ref().and_then(|d| EXES.iter().map(|e| d.join(e)).find(|p| p.is_file()));
    let found = pick(running.clone().flatten(), task.clone(), program_files, imp::elsewhere());
    let install = found.map(|(exe, from)| {
        let text = std::fs::read_to_string(exe.with_extension("config")).ok();
        let web = text.as_deref().and_then(config::read).map(|pairs| config::web_server(&pairs)).unwrap_or_default();
        // Hers: in her folder, with her marker.
        let hers = dir.as_deref().is_some_and(|d| exe.parent().is_some_and(|p| same_path(p, d)) && hers_in(d).is_some());
        Install { hers, config: text.is_some(), exe, from, web }
    });
    Seen { install, running: running.is_some(), task, pawnio: imp::pawnio() }
}

fn set_stage(sh: &Shared, stage: &'static str) {
    with(|s| {
        if let Some(job) = s.job.as_mut() {
            job.stage = stage;
        }
    });
    sh.log(&format!("lhm: {stage}"));
    sh.push_settings();
}

// A job of the settings' (get, start, startup, noStartup, remove), on its
// own thread, one at a time; or at once: usePort, folder, look, dismiss.
pub fn act(sh: &Arc<Shared>, what: &str) {
    match what {
        "look" => return look_soon(sh),
        "dismiss" => {
            with(|s| s.error = None);
            return sh.push_settings();
        }
        "folder" => {
            if let Some(i) = with(|s| s.seen.install.clone()) {
                imp::open_folder(i.exe.parent().unwrap_or(&i.exe));
            }
            return;
        }
        "usePort" => {
            let other = with(|s| s.seen.install.as_ref().and_then(|i| i.web.port));
            if let Some(port) = other {
                let mut monitor = sh.config().monitor.clone();
                if !monitor.is_object() {
                    monitor = json!({});
                }
                monitor["lhmPort"] = json!(port);
                crate::settings::apply_patch(sh, &json!({ "monitor": monitor }));
                crate::lhm::ask_now();
            }
            return;
        }
        "get" | "start" | "startup" | "noStartup" | "remove" => {}
        _ => return,
    }
    let what = match what {
        "get" => "get",
        "start" => "start",
        "startup" => "startup",
        "noStartup" => "noStartup",
        _ => "remove",
    };
    let began = with(|s| {
        if s.job.is_some() {
            return false;
        }
        s.job = Some(Job { stage: if what == "get" { "release" } else { "look" }, got: 0, total: 0, version: String::new(), asset: String::new() });
        s.error = None;
        true
    });
    if !began {
        return;
    }
    sh.push_settings();
    let sh = sh.clone();
    std::thread::spawn(move || {
        sh.log(&format!("lhm: {what}"));
        let done = match what {
            "get" => get(&sh),
            "start" => start_found(&sh),
            "startup" => startup_on(&sh),
            "noStartup" => startup_off(&sh),
            _ => remove(&sh),
        };
        if let Err(e) = &done {
            sh.log(&format!("lhm: {what} failed: {e}"));
        }
        let seen = look(&sh);
        with(|s| {
            s.job = None;
            s.error = done.err();
            s.seen = seen;
            s.looked = Some(Instant::now());
        });
        sh.push_settings();
    });
}

// The settings window's handle, for Windows' question to come up over it.
fn owner(sh: &Shared) -> isize {
    #[cfg(windows)]
    {
        use tauri::Manager;
        sh.app.get_webview_window("island").and_then(|w| crate::main_wait::hwnd(&w)).unwrap_or(0)
    }
    #[cfg(not(windows))]
    {
        let _ = sh;
        0
    }
}

// --- Download ------------------------------------------------------------------------

fn testing() -> bool {
    std::env::var(RELEASE_TEST).is_ok_and(|v| !v.is_empty())
}

// Through Windows' proxy first, where one is set and none is in the
// environment (ureq takes that by itself), then straight: proxy::web_ways,
// as the updates and the crash reports.
fn user_agent() -> String {
    format!("wakuwaku/{}", crate::updates::current())
}

fn latest() -> Result<Value, String> {
    let url = std::env::var(RELEASE_TEST).ok().filter(|v| !v.is_empty()).unwrap_or_else(|| LATEST.to_string());
    let mut last = String::from("no way");
    for way in crate::proxy::web_ways(&url) {
        let Some(agent) = way.agent(Duration::from_secs(20)) else { continue };
        match agent.get(&url).header("user-agent", &user_agent()).header("accept", "application/vnd.github+json").call() {
            Ok(mut res) => return res.body_mut().read_json::<Value>().map_err(|e| e.to_string()),
            Err(e) => last = format!("{}: {e}", way.name()),
        }
    }
    Err(last)
}

// Downloaded to `to`, its SHA-256 as it comes; the progress for the settings.
fn download(sh: &Shared, url: &str, to: &Path) -> Result<String, String> {
    let mut last = String::from("no way");
    for way in crate::proxy::web_ways(url) {
        let Some(agent) = way.agent(Duration::from_secs(600)) else { continue };
        let mut res = match agent.get(url).header("user-agent", &user_agent()).call() {
            Ok(res) => res,
            Err(e) => {
                last = format!("{}: {e}", way.name());
                continue;
            }
        };
        if let Some(total) = res.body().content_length() {
            with(|s| s.job.iter_mut().for_each(|j| j.total = total));
        }
        let mut reader = res.body_mut().with_config().limit(MAX_ZIP).reader();
        let mut file = std::fs::File::create(to).map_err(|e| format!("{}: {e}", to.display()))?;
        let mut hash = Sha256::new();
        let mut buf = vec![0u8; 64 * 1024];
        let mut got = 0u64;
        loop {
            let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            hash.update(&buf[..n]);
            got += n as u64;
            let push = with(|s| {
                s.job.iter_mut().for_each(|j| j.got = got);
                let due = s.pushed.is_none_or(|t| t.elapsed() >= Duration::from_millis(250));
                if due {
                    s.pushed = Some(Instant::now());
                }
                due
            });
            if push {
                sh.push_settings();
            }
        }
        file.flush().map_err(|e| e.to_string())?;
        return Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect());
    }
    Err(last)
}

// The download's checks: the URL GitHub's own for LHM's releases (a test's
// from anywhere), a SHA-256 given, the file as it says.
pub fn verified(asset: &Asset, got: Option<&str>, testing: bool) -> Result<(), Value> {
    if !testing && !asset.url.starts_with(DOWNLOADS) {
        return Err(error("elsewhere", asset.url.clone()));
    }
    let Some(digest) = asset.digest.as_deref() else { return Err(error("noDigest", asset.name.clone())) };
    match got {
        None => Ok(()),
        Some(got) if got == digest => Ok(()),
        Some(got) => Err(error("digest", format!("{got} ≠ {digest}"))),
    }
}

// Download and check; then the one step as administrator (admin: checked
// again there, put in Program Files, its config, its task, started); then
// its answer waited for.
fn get(sh: &Arc<Shared>) -> Result<(), Value> {
    let release = latest().map_err(|e| error("release", e))?;
    let version = version_of(&release).unwrap_or_default();
    let asset = choose(&release, &imp::desktop_runtimes(), std::env::consts::ARCH).ok_or_else(|| error("noAsset", version.clone()))?;
    verified(&asset, None, testing())?;
    with(|s| {
        if let Some(j) = s.job.as_mut() {
            (j.version, j.asset, j.total) = (version.clone(), asset.name.clone(), asset.size);
        }
    });
    sh.log(&format!("lhm: v{version}, {} ({} bytes, {:?}) from {}", asset.name, asset.size, asset.runtime, asset.url));
    set_stage(sh, "download");
    let tools = tools(sh);
    std::fs::create_dir_all(&tools).map_err(|e| error("download", format!("{}: {e}", tools.display())))?;
    let part = tools.join("LibreHardwareMonitor.zip.part");
    let got = download(sh, &asset.url, &part).map_err(|e| {
        let _ = std::fs::remove_file(&part);
        error("download", e)
    })?;
    set_stage(sh, "verify");
    let checked = verified(&asset, Some(&got), testing()).and_then(|_| {
        // Whole, with its exe, before anyone is asked anything.
        let bytes = std::fs::read(&part).map_err(|e| error("zip", e.to_string()))?;
        let entries = zip::entries(&bytes).map_err(|e| error("extract", e))?;
        if !entries.iter().any(|e| EXES.iter().any(|x| e.name.rsplit(['/', '\\']).next() == Some(*x))) {
            return Err(error("extract", "no LibreHardwareMonitor.exe in it"));
        }
        Ok(())
    });
    if let Err(e) = checked {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    sh.log(&format!("lhm: {} downloaded, its SHA-256 as GitHub gives it ({got})", asset.name));
    let user = crate::logon::user_sid().ok_or_else(|| error("task", "no SID"))?;
    let startup = sh.config().monitor["lhmStartup"] != false;
    let asked = ask_admin(sh, &AdminAsk::Install { zip: part.clone(), sha256: got, port: port(sh), user, startup }, "install");
    let _ = std::fs::remove_file(&part);
    asked?;
    wait_answer(sh)
}

// Someone's own LHM's config, its web server turned on (kept first as
// <config>.wakuwaku.bak), as she may write it: one in a folder only
// administrators change is not (an error: the steps then).
fn write_config(exe: &Path, port: u16) -> Result<(), Value> {
    let file = exe.with_extension("config");
    let existing = std::fs::read_to_string(&file).ok();
    if existing.is_some() {
        let backup = PathBuf::from(format!("{}.wakuwaku.bak", file.display()));
        if !backup.exists() {
            std::fs::copy(&file, &backup).map_err(|e| error("config", format!("{}: {e}", backup.display())))?;
        }
    }
    std::fs::write(&file, config::turned_on(existing.as_deref(), port, false)).map_err(|e| error("config", format!("{}: {e}", file.display())))
}

// --- Its start --------------------------------------------------------------------------

// What Windows' question (UAC) came to.
#[derive(Debug, PartialEq)]
pub enum Asked {
    // Started: its exit code when waited for.
    Ran(Option<u32>),
    Declined,
    Failed(String),
}

static ASKED_N: AtomicU32 = AtomicU32::new(0);

// `file` run as administrator (Windows asks the person), waited for up to
// `wait_ms` if given; for a test, written down and its answer waited for
// (WAKUWAKU_TEST_ELEVATE).
fn elevate(sh: &Shared, file: &Path, params: &str, dir: &Path, wait_ms: Option<u32>) -> Asked {
    sh.log(&format!("lhm: as administrator: {} {params}", file.display()));
    if let Some(test) = std::env::var_os(ELEVATE_TEST).filter(|v| !v.is_empty()) {
        // Hers alone and never used before (another run of hers left its own).
        let name = format!("{}-{}", std::process::id(), ASKED_N.fetch_add(1, Ordering::SeqCst));
        let asked = asked_by_test(Path::new(&test), &name, file, params, dir, wait_ms, Duration::from_secs(300));
        sh.log(&format!("lhm: the test answered for Windows: {asked:?}"));
        return asked;
    }
    imp::elevate(owner(sh), file, params, dir, wait_ms)
}

// For a test machine where no one answers Windows' question: what would be
// run as administrator written to <test>/elevate-<name>.json ({ file,
// params, dir, wait }; the name her pid and a count), and the answer read
// from <test>/elevate-<name>.done once whoever runs it there has (its exit
// code, or "declined").
fn asked_by_test(test: &Path, n: &str, file: &Path, params: &str, dir: &Path, wait_ms: Option<u32>, patience: Duration) -> Asked {
    let ask = json!({ "file": file, "params": params, "dir": dir, "wait": wait_ms.is_some() });
    // Whole or not there: written aside, then named.
    let part = test.join(format!("elevate-{n}.part"));
    if let Err(e) = std::fs::write(&part, ask.to_string()).and_then(|_| std::fs::rename(&part, test.join(format!("elevate-{n}.json")))) {
        return Asked::Failed(e.to_string());
    }
    let done = test.join(format!("elevate-{n}.done"));
    let began = Instant::now();
    while began.elapsed() < patience {
        if let Ok(said) = std::fs::read_to_string(&done) {
            let said = said.trim();
            return if said == "declined" { Asked::Declined } else { Asked::Ran(said.parse().ok()) };
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Asked::Failed("no answer".into())
}

// Her exe run as administrator for one step (admin), waited for; its exit
// code as an error of the settings'.
fn ask_admin(sh: &Shared, ask: &AdminAsk, stage: &'static str) -> Result<(), Value> {
    set_stage(sh, stage);
    let exe = crate::data::exe().map_err(|e| error("start", e.to_string()))?;
    let args = admin_args(ask).ok_or_else(|| error("args", ""))?;
    let dir = exe.parent().map(Path::to_path_buf).unwrap_or_default();
    match elevate(sh, &exe, &args, &dir, Some(300_000)) {
        Asked::Ran(Some(0)) => {
            sh.log(&format!("lhm: as administrator: {} done", args.split(' ').next().unwrap_or("")));
            Ok(())
        }
        Asked::Ran(Some(code)) => Err(error(AdminFail::of(code).map_or("admin", AdminFail::code), code.to_string())),
        Asked::Ran(None) => Err(error("admin", "")),
        Asked::Declined => Err(error("declined", "")),
        Asked::Failed(e) => Err(error("start", e)),
    }
}

// Started as administrator: by its task where that runs it (no question
// then), else Windows asks (its manifest asks for administrator).
fn start(sh: &Shared, exe: &Path) -> Result<(), Value> {
    set_stage(sh, "start");
    let task = crate::logon::xml(TASK).ok().flatten().map(|x| crate::logon::seen(&x).program);
    if task.is_some_and(|t| same_path(Path::new(&t), exe)) {
        match crate::logon::run(TASK) {
            Ok(()) => {
                sh.log("lhm: started by its task");
                return Ok(());
            }
            Err(e) => sh.log(&format!("lhm: its task would not run it ({e}): started as administrator")),
        }
    }
    let dir = exe.parent().unwrap_or(exe);
    match elevate(sh, exe, "", dir, None) {
        Asked::Ran(_) => Ok(()),
        Asked::Declined => Err(error("declined", "")),
        Asked::Failed(e) => Err(error("start", e)),
    }
}

// Found, stopped: its web server turned on where it is not (hers by the
// step as administrator, which starts it too; someone's own as she may),
// and started.
fn start_found(sh: &Arc<Shared>) -> Result<(), Value> {
    let seen = look(sh);
    let install = seen.install.ok_or_else(|| error("gone", ""))?;
    if seen.running {
        return Err(error("running", ""));
    }
    let port = port(sh);
    if install.web.serves(port) {
        start(sh, &install.exe)?;
    } else if install.hers {
        ask_admin(sh, &AdminAsk::Config { port }, "elevate")?;
    } else {
        set_stage(sh, "config");
        write_config(&install.exe, port)?;
        start(sh, &install.exe)?;
    }
    wait_answer(sh)
}

// Hers alone: its task made or taken away, or the whole of it removed.
fn hers_only(sh: &Shared) -> Result<(), Value> {
    match look(sh).install {
        Some(i) if i.hers => Ok(()),
        _ => Err(error("gone", "")),
    }
}

fn startup_on(sh: &Arc<Shared>) -> Result<(), Value> {
    hers_only(sh)?;
    let user = crate::logon::user_sid().ok_or_else(|| error("task", "no SID"))?;
    ask_admin(sh, &AdminAsk::Startup { user }, "elevate")
}

fn startup_off(sh: &Arc<Shared>) -> Result<(), Value> {
    hers_only(sh)?;
    ask_admin(sh, &AdminAsk::NoStartup, "elevate")
}

fn remove(sh: &Arc<Shared>) -> Result<(), Value> {
    hers_only(sh)?;
    ask_admin(sh, &AdminAsk::Remove, "elevate")
}

// Its web server's answer (any: a password asked for counts), every second
// for a few minutes; then the monitor asks it at once.
fn wait_answer(sh: &Arc<Shared>) -> Result<(), Value> {
    set_stage(sh, "wait");
    let port = port(sh);
    let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_millis(500))).proxy(None).build().new_agent();
    let began = Instant::now();
    while began.elapsed() < ANSWER_WAIT {
        let answered = match agent.get(&format!("http://127.0.0.1:{port}/data.json")).call() {
            Ok(_) | Err(ureq::Error::StatusCode(_)) => true,
            Err(_) => false,
        };
        if answered {
            sh.log(&format!("lhm: its web server answers on {port}, {} s after its start", began.elapsed().as_secs()));
            crate::lhm::ask_now();
            return Ok(());
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    Err(error("noAnswer", port.to_string()))
}

// --- The step as administrator -------------------------------------------------------
//
// LHM's task starts its exe with the highest rights at every logon: an exe
// any program of the user's could replace (or put a DLL beside) would be
// theirs to start so, a way round Windows' question. So hers lives in
// %ProgramFiles%\LibreHardwareMonitor (install_dir), which only
// administrators change (DACL_SDDL, set as the folder is made and read back
// before anything goes in), and everything that writes there, or makes its
// task, is one step of her exe run as administrator (ADMIN_FLAG; the one
// question Windows asks): install (the zip read once, its SHA-256 checked
// against the one given, then unpacked from those same bytes, so nothing
// swapped meanwhile gets in; its config, the marker, its task, its start),
// config (its web server turned on, then started), startup and noStartup
// (its task, for hers alone), remove (its task and the folder). The exe it
// starts is the one in that folder, never one named on its command line;
// nothing it writes is outside it (its answer is its exit code). An LHM the
// person put elsewhere stays theirs: she says where it is, and its own
// Options → Run On Windows Startup is LHM's to offer.

// One step asked of her exe as administrator.
#[derive(Clone, Debug, PartialEq)]
pub enum AdminAsk {
    Install { zip: PathBuf, sha256: String, port: u16, user: String, startup: bool },
    Config { port: u16 },
    Startup { user: String },
    NoStartup,
    Remove,
}

// Why a step failed: its exit code.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AdminFail {
    Args = 10,
    Zip = 11,
    Digest = 12,
    NotOurs = 13,
    Running = 14,
    Folder = 15,
    Extract = 16,
    Config = 17,
    Task = 18,
    Start = 19,
    Gone = 20,
}

impl AdminFail {
    const ALL: [AdminFail; 11] = [AdminFail::Args, AdminFail::Zip, AdminFail::Digest, AdminFail::NotOurs, AdminFail::Running, AdminFail::Folder, AdminFail::Extract, AdminFail::Config, AdminFail::Task, AdminFail::Start, AdminFail::Gone];

    pub fn of(exit: u32) -> Option<AdminFail> {
        AdminFail::ALL.into_iter().find(|f| *f as u32 == exit)
    }

    // As the settings' errors are named (lhm.err*).
    pub fn code(self) -> &'static str {
        match self {
            AdminFail::Args => "args",
            AdminFail::Zip => "zip",
            AdminFail::Digest => "digest",
            AdminFail::NotOurs => "notOurs",
            AdminFail::Running => "running",
            AdminFail::Folder => "folder",
            AdminFail::Extract => "extract",
            AdminFail::Config => "config",
            AdminFail::Task => "task",
            AdminFail::Start => "start",
            AdminFail::Gone => "gone",
        }
    }
}

fn is_sid(s: &str) -> bool {
    s.starts_with("S-1-") && s.len() <= 184 && s.bytes().all(|b| b.is_ascii_digit() || b == b'-' || b == b'S')
}

// Its command line after ADMIN_FLAG; None for what cannot be said there (a
// path with a quote in it).
pub fn admin_args(ask: &AdminAsk) -> Option<String> {
    Some(match ask {
        AdminAsk::Install { zip, sha256, port, user, startup } => {
            let zip = zip.to_string_lossy();
            if zip.contains('"') || zip.ends_with('\\') {
                return None;
            }
            format!("{ADMIN_FLAG} install --zip \"{zip}\" --sha256 {sha256} --port {port} --user {user}{}", if *startup { " --startup" } else { "" })
        }
        AdminAsk::Config { port } => format!("{ADMIN_FLAG} config --port {port}"),
        AdminAsk::Startup { user } => format!("{ADMIN_FLAG} startup --user {user}"),
        AdminAsk::NoStartup => format!("{ADMIN_FLAG} noStartup"),
        AdminAsk::Remove => format!("{ADMIN_FLAG} remove"),
    })
}

// The step, from what follows ADMIN_FLAG on her command line; each value checked.
pub fn admin_ask(args: &[String]) -> Result<AdminAsk, String> {
    let (action, rest) = args.split_first().ok_or("no step")?;
    let value = |name: &str| rest.iter().position(|a| a == name).and_then(|i| rest.get(i + 1)).map(String::as_str);
    let port = || value("--port").and_then(|p| p.parse::<u16>().ok()).filter(|&p| p > 0).ok_or("--port");
    let user = || value("--user").filter(|u| is_sid(u)).map(str::to_string).ok_or("--user");
    Ok(match action.as_str() {
        "install" => AdminAsk::Install {
            zip: value("--zip").filter(|z| !z.is_empty()).map(PathBuf::from).ok_or("--zip")?,
            sha256: value("--sha256").and_then(sha256_hex).ok_or("--sha256")?,
            port: port()?,
            user: user()?,
            startup: rest.iter().any(|a| a == "--startup"),
        },
        "config" => AdminAsk::Config { port: port()? },
        "startup" => AdminAsk::Startup { user: user()? },
        "noStartup" => AdminAsk::NoStartup,
        "remove" => AdminAsk::Remove,
        other => return Err(format!("no step {other:?}")),
    })
}

// Where hers goes: <Program Files>\LibreHardwareMonitor (the known folder,
// as the registry has it, not the environment's word).
pub fn install_dir() -> Option<PathBuf> {
    imp::program_files().map(|p| p.join(FOLDER))
}

// The allowed (A) entries of a security descriptor's DACL in SDDL, each
// its rights and whom, and whether it is protected (P: nothing inherited
// from above); None without a DACL. Rights as hex or as SDDL's letters.
pub fn granted(sddl: &str) -> Option<(bool, Vec<(u32, String)>)> {
    let d = sddl.find("D:")?;
    let rest = &sddl[d + 2..];
    let rest = &rest[..rest.find("S:").unwrap_or(rest.len())];
    let flags = &rest[..rest.find('(').unwrap_or(rest.len())];
    let letters = |s: &str| -> Option<u32> {
        if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
            return u32::from_str_radix(hex, 16).ok();
        }
        let mut mask = 0;
        for pair in s.as_bytes().chunks(2) {
            mask |= match pair {
                b"FA" => 0x001F_01FF,
                b"FR" => 0x0012_0089,
                b"FW" => 0x0012_0116,
                b"FX" => 0x0012_00A0,
                b"GA" => 0x1000_0000,
                b"GX" => 0x2000_0000,
                b"GW" => 0x4000_0000,
                b"GR" => 0x8000_0000,
                b"SD" => 0x0001_0000,
                b"RC" => 0x0002_0000,
                b"WD" => 0x0004_0000,
                b"WO" => 0x0008_0000,
                _ => return None,
            };
        }
        Some(mask)
    };
    let mut out = Vec::new();
    for ace in rest.split('(').skip(1) {
        let parts: Vec<&str> = ace.trim_end_matches(')').split(';').collect();
        if parts.len() < 6 {
            return None;
        }
        if parts[0] == "A" {
            out.push((letters(parts[2])?, parts[5].to_string()));
        }
    }
    Some((flags.contains('P'), out))
}

// Whether a folder's DACL lets only administrators change it: protected,
// all and only to SYSTEM and the Administrators, and nothing that writes,
// deletes or changes its rights to anyone else (Users read and run).
pub fn admin_only(sddl: &str) -> bool {
    // FILE_WRITE_DATA, APPEND, WRITE_EA, DELETE_CHILD, WRITE_ATTRIBUTES,
    // DELETE, WRITE_DAC, WRITE_OWNER, GENERIC_WRITE and GENERIC_ALL.
    const WRITES: u32 = 0x2 | 0x4 | 0x10 | 0x40 | 0x100 | 0x0001_0000 | 0x0004_0000 | 0x0008_0000 | 0x4000_0000 | 0x1000_0000;
    let Some((protected, aces)) = granted(sddl) else { return false };
    let admins = ["SY", "BA", "S-1-5-18", "S-1-5-32-544"];
    protected && aces.iter().any(|(_, who)| admins.contains(&who.as_str())) && aces.iter().all(|(mask, who)| admins.contains(&who.as_str()) || mask & WRITES == 0)
}

// The marker she leaves in hers.
fn marker(sha256: &str) -> String {
    json!({ "installedBy": "Wakuwaku", "wakuwaku": crate::updates::current(), "sha256": sha256, "at": crate::now_ms() }).to_string()
}

// Hers there: its exe, when the marker says she put it there.
fn hers_in(dir: &Path) -> Option<PathBuf> {
    dir.join(MARKER).is_file().then(|| EXES.iter().map(|e| dir.join(e)).find(|p| p.is_file())).flatten()
}

// The zip checked against its SHA-256 and unpacked into `dir`: the folder
// made by `make` (as administrator: only administrators may change it),
// hers before it kept as its config (one that is not hers, with anything in
// it, left alone: NotOurs); its config turned on at `port`, in the tray,
// and the marker. Its exe.
pub fn place(dir: &Path, bytes: &[u8], sha256: &str, port: u16, make: impl Fn(&Path) -> Result<(), AdminFail>) -> Result<PathBuf, AdminFail> {
    let got: String = Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect();
    if got != sha256 {
        return Err(AdminFail::Digest);
    }
    zip::entries(bytes).map_err(|_| AdminFail::Extract)?;
    let mut kept = None;
    if dir.exists() {
        let empty = std::fs::read_dir(dir).map_err(|_| AdminFail::Folder)?.next().is_none();
        if !empty && !dir.join(MARKER).is_file() {
            return Err(AdminFail::NotOurs);
        }
        kept = EXES.iter().find_map(|e| std::fs::read_to_string(dir.join(e).with_extension("config")).ok());
        std::fs::remove_dir_all(dir).map_err(|_| AdminFail::Folder)?;
    }
    make(dir)?;
    zip::extract(bytes, dir).map_err(|_| AdminFail::Extract)?;
    let exe = EXES.iter().map(|e| dir.join(e)).find(|p| p.is_file()).ok_or(AdminFail::Extract)?;
    std::fs::write(exe.with_extension("config"), config::turned_on(kept.as_deref(), port, true)).map_err(|_| AdminFail::Config)?;
    std::fs::write(dir.join(MARKER), marker(sha256)).map_err(|_| AdminFail::Folder)?;
    Ok(exe)
}

// Her exe run with ADMIN_FLAG (main.rs, before anything of hers starts): the
// step, its exit code.
pub fn admin(args: &[String]) -> i32 {
    match admin_ask(args) {
        Ok(ask) => match admin_do(&ask) {
            Ok(()) => 0,
            Err(f) => f as i32,
        },
        Err(_) => AdminFail::Args as i32,
    }
}

// The folder made as administrator with DACL_SDDL, its owner the
// Administrators, and read back: only administrators may change it.
fn make_admin_only(dir: &Path) -> Result<(), AdminFail> {
    imp::make_dir(dir, &format!("{OWNER_SDDL}{DACL_SDDL}")).map_err(|_| AdminFail::Folder)?;
    match imp::sddl_of(dir) {
        Some(sddl) if admin_only(&sddl) && sddl.contains("O:BA") => Ok(()),
        _ => {
            let _ = std::fs::remove_dir_all(dir);
            Err(AdminFail::Folder)
        }
    }
}

// LHM's task, as hers: none, or one for hers, or for an exe no longer there.
fn task_is_hers(exe: &Path) -> Result<bool, AdminFail> {
    let task = crate::logon::xml(TASK).map_err(|_| AdminFail::Task)?;
    let Some(program) = task.map(|x| PathBuf::from(crate::logon::seen(&x).program)) else { return Ok(false) };
    if same_path(&program, exe) {
        Ok(true)
    } else if program.is_file() {
        // Someone's own LHM's: not hers to change.
        Err(AdminFail::Task)
    } else {
        Ok(false)
    }
}

fn make_lhm_task(exe: &Path, user: &str) -> Result<(), AdminFail> {
    task_is_hers(exe)?;
    let dir = exe.parent().unwrap_or(exe);
    crate::logon::register(TASK, &task_xml(user, &exe.to_string_lossy(), &dir.to_string_lossy())).map_err(|_| AdminFail::Task)
}

// Started, as administrator already: by its task where it is hers, else
// itself (in its folder: it unpacks PawnIO's setup into the current one).
fn admin_start(exe: &Path) -> Result<(), AdminFail> {
    if task_is_hers(exe).unwrap_or(false) && crate::logon::run(TASK).is_ok() {
        return Ok(());
    }
    std::process::Command::new(exe).current_dir(exe.parent().unwrap_or(exe)).spawn().map(drop).map_err(|_| AdminFail::Start)
}

fn running_from(dir: &Path) -> bool {
    imp::running().flatten().is_some_and(|p| p.parent().is_some_and(|d| same_path(d, dir)))
}

fn admin_do(ask: &AdminAsk) -> Result<(), AdminFail> {
    let dir = install_dir().ok_or(AdminFail::Folder)?;
    match ask {
        AdminAsk::Install { zip, sha256, port, user, startup } => {
            let mut bytes = Vec::new();
            std::fs::File::open(zip).and_then(|f| f.take(MAX_ZIP + 1).read_to_end(&mut bytes)).map_err(|_| AdminFail::Zip)?;
            if bytes.len() as u64 > MAX_ZIP {
                return Err(AdminFail::Zip);
            }
            if running_from(&dir) {
                return Err(AdminFail::Running);
            }
            let exe = place(&dir, &bytes, sha256, *port, make_admin_only)?;
            if *startup {
                make_lhm_task(&exe, user)?;
            }
            admin_start(&exe)
        }
        AdminAsk::Config { port } => {
            let exe = hers_in(&dir).ok_or(AdminFail::Gone)?;
            if running_from(&dir) {
                return Err(AdminFail::Running);
            }
            let file = exe.with_extension("config");
            let existing = std::fs::read_to_string(&file).ok();
            std::fs::write(&file, config::turned_on(existing.as_deref(), *port, true)).map_err(|_| AdminFail::Config)?;
            admin_start(&exe)
        }
        AdminAsk::Startup { user } => make_lhm_task(&hers_in(&dir).ok_or(AdminFail::Gone)?, user),
        AdminAsk::NoStartup => {
            let exe = hers_in(&dir).ok_or(AdminFail::Gone)?;
            if task_is_hers(&exe)? {
                crate::logon::remove(TASK).map_err(|_| AdminFail::Task)?;
            }
            Ok(())
        }
        AdminAsk::Remove => {
            let exe = hers_in(&dir).ok_or(AdminFail::Gone)?;
            if running_from(&dir) {
                return Err(AdminFail::Running);
            }
            if task_is_hers(&exe).unwrap_or(false) {
                crate::logon::remove(TASK).map_err(|_| AdminFail::Task)?;
            }
            std::fs::remove_dir_all(&dir).map_err(|_| AdminFail::Folder)
        }
    }
}

// --- Windows ----------------------------------------------------------------------------

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::path::{Path, PathBuf};

    use super::Asked;
    use crate::win32::{Guid, Handle, ProcessEntry, CloseHandle, CoInitializeEx, CoTaskMemFree, CoUninitialize, ConvertSecurityDescriptorToStringSecurityDescriptorW, ConvertStringSecurityDescriptorToSecurityDescriptorW, CreateToolhelp32Snapshot, GetExitCodeProcess, GetLastError, LocalFree, OpenProcess, Process32FirstW, Process32NextW, QueryFullProcessImageNameW, ShellExecuteW, WaitForSingleObject};
    #[cfg(test)]
    use crate::win32::{GetCurrentProcess, GetTokenInformation, OpenProcessToken};

    // SHELLEXECUTEINFOW.
    #[repr(C)]
    struct ExecInfo {
        size: u32,
        mask: u32,
        hwnd: isize,
        verb: *const u16,
        file: *const u16,
        params: *const u16,
        dir: *const u16,
        show: i32,
        inst_app: isize,
        id_list: *mut c_void,
        class: *const u16,
        class_key: isize,
        hot_key: u32,
        icon_or_monitor: isize,
        process: Handle,
    }

    #[link(name = "shell32")]
    extern "system" {
        fn ShellExecuteExW(info: *mut ExecInfo) -> i32;
        fn SHGetKnownFolderPath(id: *const Guid, flags: u32, token: Handle, path: *mut *mut u16) -> i32;
    }

    // FOLDERID_ProgramFiles.
    const PROGRAM_FILES: Guid = Guid(0x905E_63B6, 0xC1BF, 0x494E, [0xB2, 0x9C, 0x65, 0xB7, 0x32, 0xD3, 0xD2, 0x1A]);

    // SECURITY_ATTRIBUTES.
    #[repr(C)]
    struct SecurityAttributes {
        size: u32,
        descriptor: *mut c_void,
        inherit: i32,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateDirectoryW(path: *const u16, attributes: *const SecurityAttributes) -> i32;
    }

    #[link(name = "advapi32")]
    extern "system" {
        fn GetNamedSecurityInfoW(name: *const u16, kind: u32, info: u32, owner: *mut *mut c_void, group: *mut *mut c_void, dacl: *mut *mut c_void, sacl: *mut *mut c_void, descriptor: *mut *mut c_void) -> u32;
    }

    const SDDL_REVISION_1: u32 = 1;
    const SE_FILE_OBJECT: u32 = 1;
    const OWNER_SECURITY_INFORMATION: u32 = 0x1;
    const DACL_SECURITY_INFORMATION: u32 = 0x4;
    #[cfg(test)]
    const TOKEN_QUERY: u32 = 0x8;
    #[cfg(test)]
    const TOKEN_ELEVATION: i32 = 20;

    // Program Files, as the registry knows it (64-bit Windows' own for her 64-bit exe).
    pub fn program_files() -> Option<PathBuf> {
        let mut path: *mut u16 = std::ptr::null_mut();
        // SAFETY: Windows' string, read up to its end and freed once.
        unsafe {
            let ok = SHGetKnownFolderPath(&PROGRAM_FILES, 0, 0, &mut path) >= 0 && !path.is_null();
            let out = ok.then(|| {
                let len = (0..).take_while(|&i| *path.add(i) != 0).count();
                PathBuf::from(String::from_utf16_lossy(std::slice::from_raw_parts(path, len)))
            });
            CoTaskMemFree(path.cast());
            out
        }
    }

    // A folder made with this security descriptor (SDDL), not one there already.
    pub fn make_dir(dir: &Path, sddl: &str) -> std::io::Result<()> {
        let (path, sddl) = (wide(&dir.to_string_lossy()), wide(sddl));
        let mut descriptor: *mut c_void = std::ptr::null_mut();
        // SAFETY: the descriptor Windows made from our string, freed once used.
        unsafe {
            if ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), SDDL_REVISION_1, &mut descriptor, std::ptr::null_mut()) == 0 {
                return Err(std::io::Error::last_os_error());
            }
            let attributes = SecurityAttributes { size: std::mem::size_of::<SecurityAttributes>() as u32, descriptor, inherit: 0 };
            let made = CreateDirectoryW(path.as_ptr(), &attributes) != 0;
            let e = std::io::Error::last_os_error();
            LocalFree(descriptor);
            if made { Ok(()) } else { Err(e) }
        }
    }

    // A file's or folder's owner and DACL, in SDDL, as Windows reads them.
    pub fn sddl_of(path: &Path) -> Option<String> {
        let path = wide(&path.to_string_lossy());
        let info = OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION;
        let (mut owner, mut dacl, mut descriptor) = (std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut());
        // SAFETY: the descriptor and the text Windows made, each freed once read.
        unsafe {
            if GetNamedSecurityInfoW(path.as_ptr(), SE_FILE_OBJECT, info, &mut owner, std::ptr::null_mut(), &mut dacl, std::ptr::null_mut(), &mut descriptor) != 0 {
                return None;
            }
            let mut text: *mut u16 = std::ptr::null_mut();
            let ok = ConvertSecurityDescriptorToStringSecurityDescriptorW(descriptor, SDDL_REVISION_1, info, &mut text, std::ptr::null_mut()) != 0 && !text.is_null();
            let out = ok.then(|| {
                let len = (0..).take_while(|&i| *text.add(i) != 0).count();
                String::from_utf16_lossy(std::slice::from_raw_parts(text, len))
            });
            if !text.is_null() {
                LocalFree(text.cast());
            }
            LocalFree(descriptor);
            out
        }
    }

    // Whether this process runs as administrator (its token elevated).
    #[cfg(test)]
    pub fn is_elevated() -> bool {
        // SAFETY: a token we close; TOKEN_ELEVATION is one DWORD.
        unsafe {
            let mut token = 0;
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return false;
            }
            let (mut elevated, mut needed) = (0u32, 0u32);
            let ok = GetTokenInformation(token, TOKEN_ELEVATION, (&mut elevated as *mut u32).cast(), 4, &mut needed) != 0;
            CloseHandle(token);
            ok && elevated != 0
        }
    }

    const TH32CS_SNAPPROCESS: u32 = 0x2;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const SEE_MASK_NOCLOSEPROCESS: u32 = 0x40;
    const SEE_MASK_NOASYNC: u32 = 0x100;
    const SEE_MASK_FLAG_NO_UI: u32 = 0x400;
    const SW_HIDE: i32 = 0;
    const SW_SHOWNORMAL: i32 = 1;
    const ERROR_CANCELLED: u32 = 1223;
    const WAIT_OBJECT_0: u32 = 0;
    const COINIT_APARTMENTTHREADED: u32 = 0x2;
    const COINIT_DISABLE_OLE1DDE: u32 = 0x4;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    // A running LHM: Some(its exe) (None inside: running, its path not ours to read), or None.
    pub fn running() -> Option<Option<PathBuf>> {
        let names: Vec<String> = super::EXES.iter().map(|e| e.to_lowercase()).collect();
        let mut found = None;
        // SAFETY: a snapshot we close; the entry sized as the API asks; a
        // process handle closed once its path is read into our buffer.
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap == 0 || snap == -1 {
                return None;
            }
            let mut e: ProcessEntry = std::mem::zeroed();
            e.size = std::mem::size_of::<ProcessEntry>() as u32;
            let mut ok = Process32FirstW(snap, &mut e);
            while ok != 0 {
                let len = e.exe.iter().position(|&c| c == 0).unwrap_or(e.exe.len());
                if names.contains(&String::from_utf16_lossy(&e.exe[..len]).to_lowercase()) {
                    let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, e.pid);
                    let mut path = None;
                    if process != 0 {
                        let mut buf = [0u16; 1024];
                        let mut size = buf.len() as u32;
                        if QueryFullProcessImageNameW(process, 0, buf.as_mut_ptr(), &mut size) != 0 {
                            path = Some(PathBuf::from(String::from_utf16_lossy(&buf[..size as usize])));
                        }
                        CloseHandle(process);
                    }
                    found = Some(path);
                    break;
                }
                ok = Process32NextW(snap, &mut e);
            }
            CloseHandle(snap);
        }
        found
    }

    // LHM where else it is put: the Run value it writes without
    // administrator's rights, winget's portable packages (the user's and the
    // computer's), Scoop's.
    pub fn elsewhere() -> Vec<(&'static str, PathBuf)> {
        use winreg::enums::HKEY_CURRENT_USER;
        let mut out = Vec::new();
        let run = winreg::RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run").and_then(|k| k.get_value::<String, _>("LibreHardwareMonitor"));
        if let Ok(path) = run {
            let path = PathBuf::from(path.trim().trim_matches('"'));
            if path.is_file() {
                out.push(("run", path));
            }
        }
        let env = |name: &str| std::env::var_os(name).map(PathBuf::from).filter(|p| !p.as_os_str().is_empty());
        for root in [env("LOCALAPPDATA").map(|d| d.join(r"Microsoft\WinGet\Packages")), env("ProgramFiles").map(|d| d.join(r"WinGet\Packages"))].into_iter().flatten() {
            let Ok(dirs) = std::fs::read_dir(&root) else { continue };
            for d in dirs.flatten() {
                if d.file_name().to_string_lossy().to_lowercase().starts_with("librehardwaremonitor.librehardwaremonitor_") {
                    if let Some(exe) = super::EXES.iter().map(|e| d.path().join(e)).find(|p| p.is_file()) {
                        out.push(("winget", exe));
                    }
                }
            }
        }
        let scoop = env("SCOOP").or_else(|| env("USERPROFILE").map(|h| h.join("scoop")));
        if let Some(exe) = scoop.and_then(|s| super::EXES.iter().map(|e| s.join(r"apps\librehardwaremonitor\current").join(e)).find(|p| p.is_file())) {
            out.push(("scoop", exe));
        }
        out
    }

    // PawnIO's version, as LHM looks for it (Uninstall\PawnIO's DisplayVersion).
    pub fn pawnio() -> Option<String> {
        use winreg::enums::HKEY_LOCAL_MACHINE;
        winreg::RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\PawnIO").and_then(|k| k.get_value::<String, _>("DisplayVersion")).ok().filter(|v| !v.trim().is_empty())
    }

    // The .NET desktop runtimes here, by major version (their folders).
    pub fn desktop_runtimes() -> Vec<u32> {
        let root = std::env::var_os("ProgramFiles").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Program Files"));
        let Ok(dirs) = std::fs::read_dir(root.join(r"dotnet\shared\Microsoft.WindowsDesktop.App")) else { return Vec::new() };
        let mut out: Vec<u32> = dirs.flatten().filter_map(|d| d.file_name().to_string_lossy().split('.').next()?.parse().ok()).collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    // SHELLEXECUTEINFOW's size as Windows checks it (cbSize).
    #[cfg(test)]
    pub fn exec_info_size() -> usize {
        std::mem::size_of::<ExecInfo>()
    }

    pub fn open_folder(dir: &Path) {
        let dir = wide(&dir.to_string_lossy());
        // SAFETY: strings of our own, null-terminated.
        unsafe { ShellExecuteW(std::ptr::null_mut(), wide("open").as_ptr(), dir.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL) };
    }

    // Run as administrator: ShellExecuteEx's "runas", Windows asking first
    // over `owner`; waited for when asked to (hidden then: her step).
    pub fn elevate(owner: isize, file: &Path, params: &str, dir: &Path, wait_ms: Option<u32>) -> Asked {
        let (verb, file_w, params_w, dir_w) = (wide("runas"), wide(&file.to_string_lossy()), wide(params), wide(&dir.to_string_lossy()));
        let mut info = ExecInfo {
            size: std::mem::size_of::<ExecInfo>() as u32,
            mask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
            hwnd: owner,
            verb: verb.as_ptr(),
            file: file_w.as_ptr(),
            params: if params.is_empty() { std::ptr::null() } else { params_w.as_ptr() },
            dir: dir_w.as_ptr(),
            show: if wait_ms.is_some() { SW_HIDE } else { SW_SHOWNORMAL },
            inst_app: 0,
            id_list: std::ptr::null_mut(),
            class: std::ptr::null(),
            class_key: 0,
            hot_key: 0,
            icon_or_monitor: 0,
            process: 0,
        };
        // SAFETY: COM up on this thread for ShellExecuteEx (as it asks),
        // undone if this did it; the struct sized as the API asks, its
        // strings ours and alive through the call; the process handle
        // closed below.
        unsafe {
            let com = CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) >= 0;
            let ran = ShellExecuteExW(&mut info) != 0;
            let e = GetLastError();
            if com {
                CoUninitialize();
            }
            if !ran {
                return if e == ERROR_CANCELLED { Asked::Declined } else { Asked::Failed(format!("error {e}")) };
            }
            let mut code = None;
            if info.process != 0 {
                if let Some(ms) = wait_ms {
                    if WaitForSingleObject(info.process, ms) == WAIT_OBJECT_0 {
                        let mut c = 0u32;
                        if GetExitCodeProcess(info.process, &mut c) != 0 {
                            code = Some(c);
                        }
                    }
                }
                CloseHandle(info.process);
            }
            Asked::Ran(code)
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use std::path::{Path, PathBuf};

    use super::Asked;

    pub fn running() -> Option<Option<PathBuf>> {
        None
    }

    pub fn elsewhere() -> Vec<(&'static str, PathBuf)> {
        Vec::new()
    }

    pub fn pawnio() -> Option<String> {
        None
    }

    pub fn desktop_runtimes() -> Vec<u32> {
        Vec::new()
    }

    pub fn open_folder(_dir: &Path) {}

    pub fn elevate(_owner: isize, _file: &Path, _params: &str, _dir: &Path, _wait_ms: Option<u32>) -> Asked {
        Asked::Failed("not on this system".into())
    }

    pub fn program_files() -> Option<PathBuf> {
        None
    }

    pub fn make_dir(_dir: &Path, _sddl: &str) -> std::io::Result<()> {
        Err(std::io::ErrorKind::Unsupported.into())
    }

    pub fn sddl_of(_path: &Path) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // LHM 0.9.6's release as GitHub's API gives it (2026-02-14; digests as
    // there, and as winget's manifest has the first), and 0.9.4's (no digest).
    fn release_096() -> Value {
        json!({
            "tag_name": "v0.9.6", "draft": false, "prerelease": false,
            "assets": [
                { "name": "LibreHardwareMonitor.NET.10.zip", "size": 8889121, "digest": "sha256:29739c4959b01b348fddad87664066634bcfd4f46e9bf41e4e916c318bcfdb99",
                  "browser_download_url": "https://github.com/LibreHardwareMonitor/LibreHardwareMonitor/releases/download/v0.9.6/LibreHardwareMonitor.NET.10.zip" },
                { "name": "LibreHardwareMonitor.zip", "size": 6632626, "digest": "sha256:086d9f1b5a99e643edc2cfaaac16051685b551e4c5ac0b32a57c58c0e529c001",
                  "browser_download_url": "https://github.com/LibreHardwareMonitor/LibreHardwareMonitor/releases/download/v0.9.6/LibreHardwareMonitor.zip" }
            ]
        })
    }

    #[test]
    fn the_asset_is_the_one_that_needs_nothing_installed() {
        assert_eq!(runtime_of("LibreHardwareMonitor.zip"), Some(Runtime::Framework));
        assert_eq!(runtime_of("LibreHardwareMonitor-net472.zip"), Some(Runtime::Framework));
        assert_eq!(runtime_of("LibreHardwareMonitor.NET.10.zip"), Some(Runtime::Net(10)));
        assert_eq!(runtime_of("LibreHardwareMonitor-net8.0-windows.zip"), Some(Runtime::Net(8)));
        assert_eq!(runtime_of("LibreHardwareMonitor.Windows.Forms.zip"), Some(Runtime::Framework));
        for not in ["LibreHardwareMonitorLib.0.9.6.nupkg", "LibreHardwareMonitorLib.zip", "Source code.zip", "LibreHardwareMonitor.7z", "OpenHardwareMonitor.zip"] {
            assert_eq!(runtime_of(not), None, "{not}");
        }
        // .NET 10's desktop runtime here (as on the user's PC) or not: the
        // .NET Framework's all the same.
        for desktop in [&[][..], &[8, 9, 10][..]] {
            let a = choose(&release_096(), desktop, "x86_64").unwrap();
            assert_eq!((a.name.as_str(), a.size, a.runtime), ("LibreHardwareMonitor.zip", 6632626, Runtime::Framework));
            assert_eq!(a.digest.as_deref(), Some("086d9f1b5a99e643edc2cfaaac16051685b551e4c5ac0b32a57c58c0e529c001"));
        }
        // A release with only a .NET 10 build: taken only with its runtime here.
        let only_net = json!({ "tag_name": "v1.0.0", "assets": [release_096()["assets"][0].clone()] });
        assert_eq!(choose(&only_net, &[8], "x86_64"), None);
        assert_eq!(choose(&only_net, &[10], "x86_64").map(|a| a.runtime), Some(Runtime::Net(10)));
        // Builds for each processor: this one's, else one for any.
        let arches = json!({ "assets": [
            { "name": "LibreHardwareMonitor-arm64.zip", "browser_download_url": "https://x/arm" },
            { "name": "LibreHardwareMonitor-x64.zip", "browser_download_url": "https://x/x64" },
            { "name": "LibreHardwareMonitor.zip", "browser_download_url": "https://x/any" } ] });
        assert_eq!(choose(&arches, &[], "x86_64").unwrap().url, "https://x/x64");
        assert_eq!(choose(&arches, &[], "aarch64").unwrap().url, "https://x/arm");
        assert_eq!(choose(&arches, &[], "x86").unwrap().url, "https://x/any");
        assert_eq!(version_of(&release_096()).as_deref(), Some("0.9.6"));
        assert_eq!(version_of(&json!({ "tag_name": "v0.9.7-pre", "prerelease": true })), None);
    }

    // Only from GitHub's downloads for LHM, only with GitHub's SHA-256, and
    // only the file it is of.
    #[test]
    fn the_download_is_checked() {
        let a = choose(&release_096(), &[], "x86_64").unwrap();
        assert_eq!(verified(&a, None, false), Ok(()));
        assert_eq!(verified(&a, Some("086d9f1b5a99e643edc2cfaaac16051685b551e4c5ac0b32a57c58c0e529c001"), false), Ok(()));
        assert_eq!(verified(&a, Some(&"0".repeat(64)), false).unwrap_err()["code"], "digest");
        let elsewhere = Asset { url: "https://example.com/LibreHardwareMonitor.zip".into(), ..a.clone() };
        assert_eq!(verified(&elsewhere, None, false).unwrap_err()["code"], "elsewhere");
        assert_eq!(verified(&elsewhere, None, true), Ok(()));
        let r094 = json!({ "tag_name": "v0.9.4", "assets": [{ "name": "LibreHardwareMonitor-net472.zip", "size": 1930741, "digest": null,
            "browser_download_url": "https://github.com/LibreHardwareMonitor/LibreHardwareMonitor/releases/download/v0.9.4/LibreHardwareMonitor-net472.zip" }] });
        let old = choose(&r094, &[], "x86_64").unwrap();
        assert_eq!(verified(&old, None, false).unwrap_err()["code"], "noDigest");
    }

    // A config as LHM 0.9.6 leaves it (its keys as MainForm and HttpServer
    // write them), a sensor's own name with the letters XML escapes.
    const SAVED: &str = "\u{feff}<?xml version=\"1.0\" encoding=\"utf-8\"?>\r\n<configuration>\r\n  <appSettings>\r\n    <add key=\"startMinMenuItem\" value=\"false\" />\r\n    <add key=\"minTrayMenuItem\" value=\"true\" />\r\n    <add key=\"runWebServerMenuItem\" value=\"false\" />\r\n    <add key=\"/lpc/nct6799d/0/fan/1/name\" value=\"CPU &amp; &lt;pump&gt; &quot;1&quot;&#xA;\" />\r\n    <add key=\"listenerIp\" value=\"?\" />\r\n    <add key=\"listenerPort\" value=\"8085\" />\r\n    <add key=\"authenticationEnabled\" value=\"false\" />\r\n    <add key=\"authenticationPassword\" value=\"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\" />\r\n  </appSettings>\r\n</configuration>";

    #[test]
    fn its_config_is_read_and_written_back_as_lhm_writes_it() {
        let pairs = config::read(SAVED).unwrap();
        assert_eq!(pairs.len(), 8);
        assert_eq!(pairs[3], ("/lpc/nct6799d/0/fan/1/name".to_string(), "CPU & <pump> \"1\"\n".to_string()));
        assert_eq!(config::write(&pairs), SAVED);
        assert_eq!(config::web_server(&pairs), config::WebServer { on: Some(false), port: Some(8085), ip: Some("?".into()) });
        // Single quotes, attributes the other way round, a comment, an empty one.
        let hand = "<configuration><!-- <add key=\"x\" value=\"y\"/> --><appSettings><add value='9000' key='listenerPort'/><add key=\"runWebServerMenuItem\" value=\"true\"></add></appSettings></configuration>";
        let pairs = config::read(hand).unwrap();
        assert_eq!(pairs, [("listenerPort".to_string(), "9000".to_string()), ("runWebServerMenuItem".to_string(), "true".to_string())]);
        assert_eq!(config::read("<configuration><appSettings /></configuration>"), Some(vec![]));
        assert_eq!(config::read("not xml"), None);
        assert_eq!(config::read("<appSettings></appSettings>"), None);
    }

    #[test]
    fn its_web_server_is_turned_on_the_rest_kept() {
        // The person's own LHM: on, at her port, listening as before (an
        // address 127.0.0.1 reaches), nothing else touched.
        let on = config::read(&config::turned_on(Some(SAVED), 8086, false)).unwrap();
        let before = config::read(SAVED).unwrap();
        assert_eq!(on.len(), before.len());
        assert_eq!(config::web_server(&on), config::WebServer { on: Some(true), port: Some(8086), ip: Some("?".into()) });
        assert!(on.iter().zip(&before).all(|(a, b)| a.0 == b.0 && (a.1 == b.1 || ["runWebServerMenuItem", "listenerPort"].contains(&a.0.as_str()))));
        assert!(config::web_server(&on).serves(8086) && !config::web_server(&on).serves(8085));
        // Listening on one of its network's addresses alone: 127.0.0.1 put in its place.
        let lan = SAVED.replace("value=\"?\"", "value=\"192.168.1.20\"");
        assert!(!config::web_server(&config::read(&lan).unwrap()).serves(8085));
        let fixed = config::read(&config::turned_on(Some(&lan), 8085, false)).unwrap();
        assert_eq!(config::web_server(&fixed).ip.as_deref(), Some("127.0.0.1"));
        // Hers, new: in the tray, on this computer alone.
        let hers = config::read(&config::turned_on(None, 8085, true)).unwrap();
        let get = |k: &str| hers.iter().find(|(key, _)| key == k).map(|(_, v)| v.as_str());
        assert_eq!((get("runWebServerMenuItem"), get("listenerPort"), get("listenerIp"), get("startMinMenuItem"), get("minTrayMenuItem")), (Some("true"), Some("8085"), Some("127.0.0.1"), Some("true"), Some("true")));
        // Not LHM's: as none.
        assert_eq!(config::turned_on(Some("garbage"), 8085, true), config::turned_on(None, 8085, true));
        for ip in ["?", "+", "0.0.0.0", "127.0.0.1"] {
            assert!(config::reaches_loopback(ip), "{ip}");
        }
        assert!(!config::reaches_loopback("10.0.0.5"));
    }

    use crate::zip::tests::zip_of;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wakuwaku-lhm-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn the_zip_is_unpacked_inside_its_folder_alone() {
        let exe = b"MZ\x90\x00 an exe of sorts".repeat(200);
        let zip = zip_of(&[("LibreHardwareMonitor.exe", &exe, true), ("LibreHardwareMonitorLib.dll", b"lib", false), ("de/", b"", false), ("de/LibreHardwareMonitor.resources.dll", b"de", true), ("说明.txt", "中文".as_bytes(), true)]);
        let dir = scratch("flat");
        assert_eq!(zip::extract(&zip, &dir), Ok(4));
        assert_eq!(std::fs::read(dir.join("LibreHardwareMonitor.exe")).unwrap(), exe);
        assert_eq!(std::fs::read(dir.join("de").join("LibreHardwareMonitor.resources.dll")).unwrap(), b"de");
        assert_eq!(std::fs::read_to_string(dir.join("说明.txt")).unwrap(), "中文");
        // In one folder: its files land in the folder itself.
        let nested = zip_of(&[("LibreHardwareMonitor/", b"", false), ("LibreHardwareMonitor/LibreHardwareMonitor.exe", b"MZ", true), ("LibreHardwareMonitor/x/y.dll", b"y", false)]);
        let dir2 = scratch("nested");
        assert_eq!(zip::extract(&nested, &dir2), Ok(2));
        assert!(dir2.join("LibreHardwareMonitor.exe").is_file() && dir2.join("x").join("y.dll").is_file());
        // A name out of the folder: nothing written.
        for bad in ["../evil.exe", "/abs.exe", "C:/Windows/evil.exe", "a/../../b", "x\\..\\..\\y", "LibreHardwareMonitor.exe:stream", "NUL.txt", "con", "a/b./c", "a /b", "COM1"] {
            assert_eq!(zip::safe(bad), None, "{bad}");
            let dir3 = scratch("bad");
            assert!(zip::extract(&zip_of(&[("ok.txt", b"ok", false), (bad, b"x", false)]), &dir3).is_err(), "{bad}");
            assert!(!dir3.join("ok.txt").exists(), "{bad}");
        }
        assert_eq!(zip::safe("de/a.dll"), Some(vec!["de".to_string(), "a.dll".to_string()]));
        assert_eq!(zip::safe("COM10.txt").map(|p| p.len()), Some(1));
        // Damaged: a byte changed (its CRC), cut short, not a zip.
        // (the exe's deflated bytes begin after its 30-byte header and its name)
        let mut bad = zip.clone();
        bad[30 + "LibreHardwareMonitor.exe".len() + 4] ^= 0xFF;
        assert!(zip::extract(&bad, &scratch("crc")).is_err());
        assert!(zip::entries(&zip[..zip.len() - 30]).is_err());
        assert!(zip::entries(b"PK not really").is_err());
        for d in [dir, dir2] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    // Zips made by other hands: Windows' own tar.exe (bsdtar: its folder
    // in it, entries stored), PowerShell 7's Compress-Archive (.NET's
    // ZipArchive: the files alone, deflated, a folder's name with a slash).
    #[test]
    fn zips_made_by_windows_tools_unpack() {
        let exe = "MZ not a real exe ".repeat(300);
        for (file, files) in [("bsdtar.zip", 3), ("compress-archive.zip", 3)] {
            let bytes = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../test/fixtures/lhm").join(file)).unwrap();
            let dir = scratch(file);
            assert_eq!(zip::extract(&bytes, &dir), Ok(files), "{file}");
            assert_eq!(std::fs::read_to_string(dir.join("LibreHardwareMonitor.exe")).unwrap(), exe, "{file}");
            assert_eq!(std::fs::read_to_string(dir.join("de").join("x.resources.dll")).unwrap(), "de", "{file}");
            assert!(dir.join("LibreHardwareMonitor.exe.config").is_file(), "{file}");
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn its_task_is_lhms_own() {
        let xml = task_xml("S-1-5-21-1-2-3-1001", r"D:\Programme & co\张三\LibreHardwareMonitor\LibreHardwareMonitor.exe", r"D:\Programme & co\张三\LibreHardwareMonitor");
        for part in ["<RunLevel>HighestAvailable</RunLevel>", "<LogonType>InteractiveToken</LogonType>", "<UserId>S-1-5-21-1-2-3-1001</UserId>", "<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>", "<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>", "<StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>", "<StartWhenAvailable>true</StartWhenAvailable>", "Starts LibreHardwareMonitor on Windows startup."] {
            assert!(xml.contains(part), "{part}");
        }
        // Its exe unquoted, as LHM compares it with its own path.
        let seen = crate::logon::seen(&xml);
        assert_eq!(seen.program, r"D:\Programme & co\张三\LibreHardwareMonitor\LibreHardwareMonitor.exe");
        assert!(xml.contains(r"<Command>D:\Programme &amp; co\"));
    }

    fn install(web: config::WebServer) -> Install {
        Install { exe: PathBuf::from(r"C:\t\LibreHardwareMonitor\LibreHardwareMonitor.exe"), from: "hers", hers: true, config: true, web }
    }

    #[test]
    fn what_is_offered_follows_what_is_found() {
        let none = Seen::default();
        assert_eq!(step("missing", false, &none, 8085), "get");
        assert_eq!(step("unread", true, &none, 8085), "busy");
        for s in ["found", "auth", "strange"] {
            assert_eq!(step(s, false, &none, 8085), "answering");
        }
        let on = config::WebServer { on: Some(true), port: Some(8085), ip: Some("+".into()) };
        let off = config::WebServer { on: Some(false), ..on.clone() };
        let stopped = |web| Seen { install: Some(install(web)), ..Seen::default() };
        assert_eq!(step("missing", false, &stopped(on.clone()), 8085), "start");
        assert_eq!(step("missing", false, &stopped(on.clone()), 9000), "turnOn");
        assert_eq!(step("missing", false, &stopped(off.clone()), 8085), "turnOn");
        assert_eq!(step("missing", false, &stopped(config::WebServer::default()), 8085), "turnOn");
        let running = |web| Seen { install: Some(install(web)), running: true, ..Seen::default() };
        assert_eq!(step("missing", false, &running(off), 8085), "steps");
        assert_eq!(step("missing", false, &running(on.clone()), 9000), "otherPort");
        assert_eq!(step("missing", false, &running(on), 8085), "steps");
        // Running where its path cannot be read: the steps.
        assert_eq!(step("missing", false, &Seen { running: true, ..Seen::default() }, 8085), "steps");
        // Its task runs this one, whatever the case of its letters.
        let mut seen = stopped(config::WebServer::default());
        seen.task = Some(PathBuf::from(r"c:\T\librehardwaremonitor\LibreHardwareMonitor.exe"));
        assert!(seen.starts_at_logon());
        seen.task = Some(PathBuf::from(r"D:\other\LibreHardwareMonitor.exe"));
        assert!(!seen.starts_at_logon());
    }

    #[test]
    fn the_install_is_the_one_running_first() {
        let program_files = PathBuf::from(r"C:\Program Files\LibreHardwareMonitor\LibreHardwareMonitor.exe");
        let run = PathBuf::from(r"D:\LHM\LibreHardwareMonitor.exe");
        assert_eq!(pick(Some(run.clone()), None, Some(program_files.clone()), vec![]), Some((run.clone(), "running")));
        assert_eq!(pick(None, None, Some(program_files.clone()), vec![("winget", run.clone())]), Some((program_files.clone(), "programFiles")));
        assert_eq!(pick(None, None, None, vec![("winget", run.clone())]), Some((run, "winget")));
        // A task whose exe is gone is passed over.
        assert_eq!(pick(None, Some(PathBuf::from(r"Z:\gone\LibreHardwareMonitor.exe")), None, vec![]), None);
        assert_eq!(pick(None, None, None, vec![]), None);
    }
    // Her step's command line, as she writes it and as her exe takes it
    // back (split as Windows splits it: quotes round a path with spaces).
    #[test]
    fn the_step_as_administrator_is_said_and_read_back() {
        let split = |line: &str| -> Vec<String> {
            let (mut out, mut word, mut quoted) = (Vec::new(), String::new(), false);
            for c in line.chars() {
                match c {
                    '"' => quoted = !quoted,
                    ' ' if !quoted => {
                        if !word.is_empty() {
                            out.push(std::mem::take(&mut word));
                        }
                    }
                    c => word.push(c),
                }
            }
            out.extend((!word.is_empty()).then_some(word));
            out
        };
        let sha = "086d9f1b5a99e643edc2cfaaac16051685b551e4c5ac0b32a57c58c0e529c001".to_string();
        let asks = [
            AdminAsk::Install { zip: PathBuf::from(r"C:\Users\张 三\AppData\Roaming\Wakuwaku\tools\LibreHardwareMonitor.zip.part"), sha256: sha.clone(), port: 8085, user: "S-1-5-21-1-2-3-1001".into(), startup: true },
            AdminAsk::Install { zip: PathBuf::from(r"C:\d\x.zip.part"), sha256: sha.clone(), port: 9000, user: "S-1-5-21-9-1001".into(), startup: false },
            AdminAsk::Config { port: 8086 },
            AdminAsk::Startup { user: "S-1-5-21-1-2-3-1001".into() },
            AdminAsk::NoStartup,
            AdminAsk::Remove,
        ];
        for ask in asks {
            let line = admin_args(&ask).unwrap();
            let words = split(&line);
            assert_eq!(words[0], ADMIN_FLAG);
            assert_eq!(admin_ask(&words[1..]), Ok(ask.clone()), "{line}");
        }
        let words = |s: &str| split(s);
        for bad in ["install --zip x --sha256 abc --port 8085 --user S-1-5-21-1", "install --zip x --sha256 SHA --port 0 --user S-1-5-21-1", "install --sha256 SHA --port 8085 --user S-1-5-21-1", "startup --user root", "config --port 70000", "uninstall", ""] {
            let bad = bad.replace("SHA", &sha);
            assert!(admin_ask(&words(&bad)).is_err(), "{bad}");
        }
        // A path Windows' command line could not hold: not asked.
        assert_eq!(admin_args(&AdminAsk::Install { zip: PathBuf::from("C:\\a\"b"), sha256: sha.clone(), port: 1, user: "S-1-5-1".into(), startup: false }), None);
        for f in AdminFail::ALL {
            assert_eq!(AdminFail::of(f as u32), Some(f));
        }
        assert_eq!((AdminFail::of(0), AdminFail::of(1)), (None, None));
        assert_eq!((AdminFail::NotOurs.code(), AdminFail::Digest as i32), ("notOurs", 12));
    }

    // The folder's DACL: only administrators change it; Users read and run.
    #[test]
    fn hers_is_a_folder_only_administrators_change() {
        let (protected, aces) = granted(DACL_SDDL).unwrap();
        assert!(protected);
        assert_eq!(aces, [(0x1F01FF, "SY".to_string()), (0x1F01FF, "BA".to_string()), (0x1200A9, "BU".to_string())]);
        assert!(admin_only(&format!("{OWNER_SDDL}{DACL_SDDL}")));
        // As Windows may write it back: the letters for read and run.
        assert!(admin_only("O:BAD:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FRFX;;;BU)"));
        // Program Files' own (inherited, its creator owner and TrustedInstaller
        // in it), a user's folder, Users changing it, nobody's: not that.
        assert!(!admin_only("D:AI(A;OICIID;FA;;;SY)(A;OICIID;FA;;;BA)(A;OICIID;0x1200a9;;;BU)"));
        assert!(!admin_only("D:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FA;;;S-1-5-21-1-2-3-1001)"));
        assert!(!admin_only("D:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;0x1301bf;;;BU)"));
        assert!(!admin_only("D:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FRFX;;;AU)(A;;GA;;;WD)"));
        assert!(!admin_only("D:P"));
        assert!(!admin_only("O:BA"));
        assert_eq!(granted("D:PAI(A;OICI;XX;;;SY)"), None);
    }

    // Placed: checked by its SHA-256, unpacked, its config and the marker;
    // again over hers (its config's changes kept); never over someone's.
    #[test]
    fn hers_is_placed_only_where_it_is_hers() {
        let zip = zip_of(&[("LibreHardwareMonitor.exe", b"MZ", true), ("LibreHardwareMonitorLib.dll", b"lib", false)]);
        let sha: String = Sha256::digest(&zip).iter().map(|b| format!("{b:02x}")).collect();
        let make = |d: &Path| std::fs::create_dir_all(d).map_err(|_| AdminFail::Folder);
        let root = scratch("place");
        let dir = root.join("LibreHardwareMonitor");
        let exe = place(&dir, &zip, &sha, 8085, make).unwrap();
        assert_eq!(exe, dir.join("LibreHardwareMonitor.exe"));
        assert_eq!(hers_in(&dir), Some(exe.clone()));
        let config = || config::web_server(&config::read(&std::fs::read_to_string(exe.with_extension("config")).unwrap()).unwrap());
        assert_eq!(config(), config::WebServer { on: Some(true), port: Some(8085), ip: Some("127.0.0.1".into()) });
        // LHM wrote its own back (another port, a key of its own): kept, on at hers.
        std::fs::write(exe.with_extension("config"), config::write(&[("listenerPort".into(), "9001".into()), ("theme".into(), "dark".into())])).unwrap();
        place(&dir, &zip, &sha, 8086, make).unwrap();
        assert_eq!(config().port, Some(8086));
        assert!(std::fs::read_to_string(exe.with_extension("config")).unwrap().contains("\"theme\" value=\"dark\""));
        // Another file than the one checked: nothing touched.
        assert_eq!(place(&dir, &zip, &"0".repeat(64), 8085, make), Err(AdminFail::Digest));
        assert!(exe.is_file());
        // Someone's own LHM there (no marker): left alone.
        let theirs = root.join("Theirs");
        std::fs::create_dir_all(&theirs).unwrap();
        std::fs::write(theirs.join("LibreHardwareMonitor.exe"), b"theirs").unwrap();
        assert_eq!(place(&theirs, &zip, &sha, 8085, make), Err(AdminFail::NotOurs));
        assert_eq!(std::fs::read(theirs.join("LibreHardwareMonitor.exe")).unwrap(), b"theirs");
        assert_eq!(hers_in(&theirs), None);
        // An empty folder is taken; a zip with no exe is not placed.
        let empty = root.join("Empty");
        std::fs::create_dir_all(&empty).unwrap();
        assert!(place(&empty, &zip, &sha, 8085, make).is_ok());
        let no_exe = zip_of(&[("readme.txt", b"hi", false)]);
        let no_exe_sha: String = Sha256::digest(&no_exe).iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(place(&root.join("NoExe"), &no_exe, &no_exe_sha, 8085, make), Err(AdminFail::Extract));
        let _ = std::fs::remove_dir_all(root);
    }

    // Program Files as the registry has it, and a folder made with her DACL
    // read back as Windows keeps it: not hers to write unless elevated.
    #[cfg(windows)]
    #[test]
    fn a_folder_made_with_her_dacl_is_closed_to_the_user() {
        let pf = imp::program_files().unwrap();
        if let Some(env) = std::env::var_os("ProgramFiles") {
            assert!(same_path(&pf, Path::new(&env)), "{pf:?} {env:?}");
        }
        assert_eq!(install_dir().unwrap(), pf.join("LibreHardwareMonitor"));
        let dir = scratch("dacl");
        // Its owner left as whoever makes it: O:BA takes an administrator.
        imp::make_dir(&dir, DACL_SDDL).unwrap();
        assert!(imp::make_dir(&dir, DACL_SDDL).is_err(), "not made over one there");
        let sddl = imp::sddl_of(&dir).unwrap();
        println!("{sddl}");
        assert!(admin_only(&sddl), "{sddl}");
        assert!(sddl.contains("OICI"), "{sddl}");
        let wrote = std::fs::write(dir.join("planted.dll"), b"x");
        assert_eq!(wrote.is_ok(), imp::is_elevated(), "{wrote:?}");
        let _ = std::fs::remove_file(dir.join("planted.dll"));
        std::fs::remove_dir(&dir).unwrap();
    }

    // This machine looked at as the settings look, read-only: its processes,
    // LHM's task, the Run value, winget's and Scoop's folders, PawnIO, the
    // .NET desktop runtimes; and ShellExecuteEx's struct as Windows sizes it.
    #[cfg(windows)]
    #[test]
    fn this_machine_is_looked_at() {
        assert_eq!(imp::exec_info_size(), if cfg!(target_pointer_width = "64") { 112 } else { 60 });
        let running = imp::running();
        let task = crate::logon::xml(TASK);
        println!("running: {running:?}; task: {:?}; elsewhere: {:?}; PawnIO: {:?}; desktop runtimes: {:?}", task.as_ref().map(|t| t.as_ref().map(|x| crate::logon::seen(x).program)), imp::elsewhere(), imp::pawnio(), imp::desktop_runtimes());
        assert!(task.is_ok(), "{task:?}");
        assert!(imp::elsewhere().iter().all(|(_, p)| p.is_file()));
    }

    // Windows' question answered by a test (WAKUWAKU_TEST_ELEVATE): what
    // would be run written down, its exit code (or a no) read back; no
    // answer in time, failed.
    #[test]
    fn the_question_can_be_answered_by_a_test() {
        let dir = scratch("elevate");
        std::fs::create_dir_all(&dir).unwrap();
        let answer = |n: &'static str, said: &'static str| {
            let dir = dir.clone();
            std::thread::spawn(move || {
                let ask = dir.join(format!("elevate-{n}.json"));
                for _ in 0..100 {
                    if let Some(v) = std::fs::read_to_string(&ask).ok().and_then(|text| serde_json::from_str::<Value>(&text).ok()) {
                        std::fs::write(dir.join(format!("elevate-{n}.done")), said).unwrap();
                        return v;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                panic!("not asked");
            })
        };
        let patience = Duration::from_secs(5);
        let t = answer("7-0", "0\r\n");
        assert_eq!(asked_by_test(&dir, "7-0", Path::new(r"C:\w\wakuwaku.exe"), &format!("{ADMIN_FLAG} noStartup"), &dir, Some(60_000), patience), Asked::Ran(Some(0)));
        let v = t.join().unwrap();
        assert_eq!((v["params"].as_str(), v["wait"].as_bool()), (Some("--wakuwaku-lhm noStartup"), Some(true)));
        assert!(v["file"].as_str().unwrap().ends_with("wakuwaku.exe"));
        let t = answer("7-1", "declined");
        assert_eq!(asked_by_test(&dir, "7-1", Path::new(r"C:\x\LibreHardwareMonitor.exe"), "", &dir, None, patience), Asked::Declined);
        assert_eq!(t.join().unwrap()["wait"], false);
        assert_eq!(asked_by_test(&dir, "7-2", Path::new("x.exe"), "", &dir, None, Duration::from_millis(200)), Asked::Failed("no answer".into()));
        let _ = std::fs::remove_dir_all(dir);
    }
}
