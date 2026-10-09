// The plugins she runs for you: the example scripts (examples/widgets), each
// started hidden while its switch is on, started again when it stops, and
// stopped when the switch goes off or she quits. A job object takes them down
// with her even when she is killed. They are still only scripts sending
// words to /widget: she starts them and keeps the last line they print.
//
// Settings: "plugins": { <id>: { "on": bool, <Param>: text | bool } }
// The scripts are built into her and written to <her folder>/plugins, so any
// copy of her runs the same ones. The mail scripts are not among them yet.
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::Shared;

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Text,
    Number,
    Flag,
}

pub struct Param {
    pub name: &'static str,
    pub kind: Kind,
    pub required: bool,
}

pub struct Script {
    pub id: &'static str,
    pub icon: &'static str,
    pub params: &'static [Param],
    body: &'static [u8],
}

const fn p(name: &'static str, kind: Kind, required: bool) -> Param {
    Param { name, kind, required }
}

pub static SCRIPTS: [Script; 7] = [
    Script { id: "weather", icon: "weather", params: &[p("City", Kind::Text, false)], body: include_bytes!("../../examples/widgets/weather.ps1") },
    Script {
        id: "stock",
        icon: "stock",
        params: &[p("Symbol", Kind::Text, false), p("Name", Kind::Text, false)],
        body: include_bytes!("../../examples/widgets/stock.ps1"),
    },
    Script {
        id: "pomodoro",
        icon: "timer",
        params: &[p("Work", Kind::Number, false), p("Break", Kind::Number, false)],
        body: include_bytes!("../../examples/widgets/pomodoro.ps1"),
    },
    Script {
        id: "countdown",
        icon: "flag",
        params: &[p("At", Kind::Text, true), p("Title", Kind::Text, false)],
        body: include_bytes!("../../examples/widgets/countdown.ps1"),
    },
    Script { id: "stretch", icon: "bell", params: &[p("Minutes", Kind::Number, false)], body: include_bytes!("../../examples/widgets/stretch.ps1") },
    Script { id: "ci", icon: "check", params: &[p("Repo", Kind::Text, true), p("Prs", Kind::Flag, false)], body: include_bytes!("../../examples/widgets/ci.ps1") },
    Script { id: "devserver", icon: "server", params: &[p("Url", Kind::Text, false)], body: include_bytes!("../../examples/widgets/devserver.ps1") },
];

// waku is no plugin to switch on: a command wrapper, written out beside them
// for the terminal.
const WAKU: &[u8] = include_bytes!("../../examples/widgets/waku.ps1");

fn script(id: &str) -> Option<&'static Script> {
    SCRIPTS.iter().find(|s| s.id == id)
}

// --- Settings ---------------------------------------------------------------------------

// A "plugins" setting: only the plugins here, each with "on" and its own
// parameters, text without control characters, numbers as text.
pub fn is_ok(v: &Value) -> bool {
    let Some(all) = v.as_object() else { return false };
    all.iter().all(|(id, conf)| {
        let (Some(s), Some(conf)) = (script(id), conf.as_object()) else { return false };
        conf.iter().all(|(key, v)| match (key.as_str(), s.params.iter().find(|p| p.name == key)) {
            ("on", _) => v.is_boolean(),
            (_, Some(p)) => match p.kind {
                Kind::Flag => v.is_boolean(),
                Kind::Text => v.as_str().is_some_and(|t| t.chars().count() <= 300 && !t.chars().any(char::is_control)),
                Kind::Number => v.as_str().is_some_and(|t| t.trim().is_empty() || t.trim().parse::<f64>().is_ok_and(|n| n > 0.0 && n <= 100_000.0)),
            },
            _ => false,
        })
    })
}

fn text<'a>(conf: &'a Value, name: &str) -> &'a str {
    conf[name].as_str().map(str::trim).unwrap_or("")
}

// The first parameter it needs and has not been given.
fn missing(s: &Script, conf: &Value) -> Option<&'static str> {
    s.params.iter().find(|p| p.required && text(conf, p.name).is_empty()).map(|p| p.name)
}

// Which plugin a widget comes from, by the ids the scripts give them.
pub fn owner_of(widget: &str) -> Option<&'static str> {
    match widget {
        "weather" | "pomodoro" | "countdown" | "stretch" | "devserver" => script(widget).map(|s| s.id),
        w if w.starts_with("stock-") => Some("stock"),
        w if w.starts_with("ci-") || w.starts_with("prs-") => Some("ci"),
        _ => None,
    }
}

// --- Running them ---------------------------------------------------------------------------

// A PowerShell string: in single quotes, which PowerShell also takes in
// their curly forms, each doubled inside.
fn quote(s: &str) -> String {
    let mut out = String::from("'");
    for c in s.chars() {
        if matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}') {
            out.push(c);
        }
        out.push(c);
    }
    out.push('\'');
    out
}

// The script and its parameters, as PowerShell runs them.
fn invocation(s: &Script, conf: &Value, folder: &Path, port: u16) -> String {
    let mut line = format!("& {}", quote(&folder.join(format!("{}.ps1", s.id)).to_string_lossy()));
    for p in s.params {
        match p.kind {
            Kind::Flag if conf[p.name] == true => line += &format!(" -{}", p.name),
            Kind::Text | Kind::Number if !text(conf, p.name).is_empty() => line += &format!(" -{} {}", p.name, quote(text(conf, p.name))),
            _ => {}
        }
    }
    line + &format!(" -Port {port}")
}

// Hidden: what it prints in UTF-8 and without colours (PowerShell 7), so
// the last line can be shown.
fn arguments(s: &Script, conf: &Value, folder: &Path, port: u16) -> Vec<String> {
    let line = format!(
        "try {{ [Console]::OutputEncoding = [Text.Encoding]::UTF8; $PSStyle.OutputRendering = 'PlainText' }} catch {{}}; {}",
        invocation(s, conf, folder, port)
    );
    ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &line].map(String::from).to_vec()
}

// PowerShell 7 where it is, else the Windows PowerShell every Windows has.
fn powershell() -> PathBuf {
    static FOUND: OnceLock<PathBuf> = OnceLock::new();
    FOUND
        .get_or_init(|| {
            let on_path = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).map(|d| d.join("pwsh.exe")).collect::<Vec<_>>()).unwrap_or_default();
            let installed = std::env::var_os("ProgramFiles").map(|p| PathBuf::from(p).join("PowerShell").join("7").join("pwsh.exe"));
            let windows = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| "C:\\Windows".into()).join("System32\\WindowsPowerShell\\v1.0\\powershell.exe");
            on_path.into_iter().chain(installed).find(|p| p.is_file()).unwrap_or(windows)
        })
        .clone()
}

// The scripts in her folder, rewritten when they differ from hers.
fn write_out(folder: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(folder)?;
    for (name, body) in SCRIPTS.iter().map(|s| (s.id, s.body)).chain([("waku", WAKU)]) {
        let file = folder.join(format!("{name}.ps1"));
        if std::fs::read(&file).ok().as_deref() != Some(body) {
            std::fs::write(&file, body)?;
        }
    }
    Ok(())
}

// What a script printed: UTF-8, else as the system's code page would have it.
fn decode(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => from_code_page(bytes),
    }
}

// A line as text: without the terminal's colour codes.
fn plain(line: &str) -> String {
    let mut out = String::new();
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // ESC [ ... a letter
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

// A line worth showing: without PowerShell's own prefix, and not too long;
// nothing for one with no word in it (a brace closing an error's JSON).
fn tidy(line: &str) -> String {
    let line = plain(line);
    let line = line.trim();
    if !line.chars().any(char::is_alphanumeric) {
        return String::new();
    }
    let line = ["WARNING: ", "警告: ", "警告："].iter().find_map(|p| line.strip_prefix(p)).unwrap_or(line);
    line.trim().chars().take(160).collect()
}

struct Run {
    child: Child,
    args: Vec<String>,
    since: Instant,
}

#[derive(Default)]
struct Trouble {
    failures: u32,
    retry_at: Option<Instant>,
}

// Ending soon after starting is failing: tried again later each time.
fn backoff(failures: u32) -> Duration {
    Duration::from_secs(match failures {
        0 => 0,
        1 => 10,
        2 => 30,
        3 => 60,
        _ => 300,
    })
}

#[derive(Default)]
pub struct Runner {
    runs: HashMap<&'static str, Run>,
    trouble: HashMap<&'static str, Trouble>,
    // The last line each printed (a warning, an error).
    said: Arc<Mutex<HashMap<&'static str, String>>>,
    job: Option<job::Job>,
    written: bool,
}

impl Runner {
    // Started hidden, in the job; all it prints goes to <folder>/<id>.log,
    // new each start, and its last line that says something is kept.
    fn start(&mut self, s: &'static Script, args: &[String], folder: &Path) -> std::io::Result<Run> {
        self.said.lock().unwrap().remove(s.id);
        let mut command = Command::new(powershell());
        command.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW
            command.creation_flags(0x0800_0000);
        }
        let mut child = command.spawn()?;
        if self.job.is_none() {
            self.job = job::Job::new();
        }
        if let Some(job) = &self.job {
            job.assign(&child);
        }
        let pipes: [Option<Box<dyn Read + Send>>; 2] = [child.stdout.take().map(|p| Box::new(p) as _), child.stderr.take().map(|p| Box::new(p) as _)];
        let log = std::fs::File::create(folder.join(format!("{}.log", s.id))).ok().map(|f| Arc::new(Mutex::new(f)));
        for pipe in pipes.into_iter().flatten() {
            let (said, log) = (self.said.clone(), log.clone());
            std::thread::spawn(move || {
                for line in BufReader::new(pipe).split(b'\n').map_while(Result::ok) {
                    let text = plain(&decode(&line));
                    if let Some(log) = &log {
                        let _ = writeln!(log.lock().unwrap(), "{}", text.trim_end());
                    }
                    let line = tidy(&text);
                    if !line.is_empty() {
                        said.lock().unwrap().insert(s.id, line);
                    }
                }
            });
        }
        Ok(Run { child, args: args.to_vec(), since: Instant::now() })
    }
}

// Each running as its switch says: started, stopped, started again with new
// settings, or later when it keeps failing. True when any changed.
pub fn sync(sh: &Shared) -> bool {
    let conf = sh.setting("plugins");
    let folder = sh.dir.join("plugins");
    let now = Instant::now();
    let (mut changed, mut stale) = (false, Vec::new());
    {
        let mut r = sh.scripts.lock().unwrap();
        if !r.written {
            if let Err(err) = write_out(&folder) {
                sh.log(&format!("plugins: writing the scripts: {err}"));
            }
            r.written = true;
        }
        for s in &SCRIPTS {
            let c = &conf[s.id];
            let is_on = c["on"] == true;
            let want = is_on && missing(s, c).is_none();
            let args = arguments(s, c, &folder, sh.port);
            if let Some(mut run) = r.runs.remove(s.id) {
                match run.child.try_wait() {
                    Ok(None) if want && run.args == args => {
                        r.runs.insert(s.id, run);
                        continue;
                    }
                    Ok(None) => {
                        let _ = run.child.kill();
                        let _ = run.child.wait();
                        r.trouble.remove(s.id);
                        stale.push(s.id);
                    }
                    // It ended by itself.
                    _ => {
                        let t = r.trouble.entry(s.id).or_default();
                        t.failures = if run.since.elapsed() < Duration::from_secs(60) { t.failures + 1 } else { 0 };
                        t.retry_at = Some(now + backoff(t.failures));
                        sh.log(&format!("plugins: {} ended ({} in a row)", s.id, t.failures));
                    }
                }
                changed = true;
            }
            if !want {
                if !is_on && r.trouble.remove(s.id).is_some() {
                    stale.push(s.id);
                }
                continue;
            }
            if r.trouble.get(s.id).and_then(|t| t.retry_at).is_some_and(|at| at > now) {
                continue;
            }
            match r.start(s, &args, &folder) {
                Ok(run) => {
                    r.runs.insert(s.id, run);
                }
                Err(err) => {
                    r.said.lock().unwrap().insert(s.id, err.to_string());
                    let t = r.trouble.entry(s.id).or_default();
                    t.failures += 1;
                    t.retry_at = Some(now + backoff(t.failures));
                }
            }
            changed = true;
        }
    }
    // Stopped by its switch or for new settings: what it showed goes with it.
    if !stale.is_empty() && sh.widgets.lock().unwrap().remove_scripted(|id| owner_of(id).is_some_and(|o| stale.contains(&o))) {
        changed = true;
    }
    changed
}

// All of them, as she quits (the job would take them anyway).
pub fn stop_all(sh: &Shared) {
    for (_, mut run) in sh.scripts.lock().unwrap().runs.drain() {
        let _ = run.child.kill();
    }
}

// Each plugin for the settings: its switch, its parameters, and how it is.
//   state  off | needs (a parameter) | running | stopped (it ended; retryIn
//          seconds until it is tried again)
pub fn view(sh: &Shared) -> Value {
    let conf = sh.setting("plugins");
    let now = Instant::now();
    let r = sh.scripts.lock().unwrap();
    let said = r.said.lock().unwrap();
    Value::Array(
        SCRIPTS
            .iter()
            .map(|s| {
                let c = &conf[s.id];
                let on = c["on"] == true;
                let state = match () {
                    _ if !on => "off",
                    _ if missing(s, c).is_some() => "needs",
                    _ if r.runs.contains_key(s.id) => "running",
                    _ => "stopped",
                };
                let retry = r.trouble.get(s.id).and_then(|t| t.retry_at).map(|at| at.saturating_duration_since(now).as_secs());
                let params: Vec<Value> = s
                    .params
                    .iter()
                    .map(|p| {
                        let kind = match p.kind {
                            Kind::Text => "text",
                            Kind::Number => "number",
                            Kind::Flag => "flag",
                        };
                        json!({ "name": p.name, "kind": kind, "required": p.required, "value": c[p.name] })
                    })
                    .collect();
                json!({ "id": s.id, "icon": s.icon, "on": on, "state": state, "missing": missing(s, c), "said": said.get(s.id), "retryIn": retry, "params": params })
            })
            .collect(),
    )
}

// The waku script, written out, for the terminal.
pub fn waku_path(sh: &Shared) -> PathBuf {
    let folder = sh.dir.join("plugins");
    let _ = write_out(&folder);
    folder.join("waku.ps1")
}

#[cfg(windows)]
fn from_code_page(bytes: &[u8]) -> String {
    #[link(name = "kernel32")]
    extern "system" {
        fn MultiByteToWideChar(page: u32, flags: u32, from: *const u8, from_len: i32, to: *mut u16, to_len: i32) -> i32;
    }
    // CP_ACP
    // SAFETY: lengths are the buffers' own; the first call only measures.
    unsafe {
        let n = MultiByteToWideChar(0, 0, bytes.as_ptr(), bytes.len() as i32, std::ptr::null_mut(), 0);
        let mut wide = vec![0u16; n.max(0) as usize];
        let n = MultiByteToWideChar(0, 0, bytes.as_ptr(), bytes.len() as i32, wide.as_mut_ptr(), n);
        String::from_utf16_lossy(&wide[..n.max(0) as usize])
    }
}

#[cfg(not(windows))]
fn from_code_page(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

// A job object that ends every process in it when she ends, however she ends.
#[cfg(windows)]
mod job {
    #[repr(C)]
    #[derive(Default)]
    struct Basic {
        per_process_user_time: i64,
        per_job_user_time: i64,
        flags: u32,
        min_working_set: usize,
        max_working_set: usize,
        active_processes: u32,
        affinity: usize,
        priority: u32,
        scheduling: u32,
    }

    #[repr(C)]
    #[derive(Default)]
    pub struct Extended {
        basic: Basic,
        io: [u64; 6],
        process_memory: usize,
        job_memory: usize,
        peak_process_memory: usize,
        peak_job_memory: usize,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateJobObjectW(attributes: *const u8, name: *const u16) -> isize;
        fn SetInformationJobObject(job: isize, class: i32, info: *const Extended, len: u32) -> i32;
        fn AssignProcessToJobObject(job: isize, process: isize) -> i32;
    }

    const EXTENDED_LIMIT_INFORMATION: i32 = 9;
    const KILL_ON_JOB_CLOSE: u32 = 0x2000;

    // Its handle stays open while she runs; closed as she ends, it ends them.
    pub struct Job(isize);

    impl Job {
        pub fn new() -> Option<Job> {
            // SAFETY: no attributes, no name; the limits are a struct of the size given.
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job == 0 {
                    return None;
                }
                let limits = Extended { basic: Basic { flags: KILL_ON_JOB_CLOSE, ..Basic::default() }, ..Extended::default() };
                (SetInformationJobObject(job, EXTENDED_LIMIT_INFORMATION, &limits, std::mem::size_of::<Extended>() as u32) != 0).then_some(Job(job))
            }
        }

        pub fn assign(&self, child: &std::process::Child) -> bool {
            use std::os::windows::io::AsRawHandle;
            // SAFETY: a process we just started and still hold.
            unsafe { AssignProcessToJobObject(self.0, child.as_raw_handle() as isize) != 0 }
        }
    }

    #[cfg(test)]
    pub fn extended_size() -> usize {
        std::mem::size_of::<Extended>()
    }
}

#[cfg(not(windows))]
mod job {
    pub struct Job;
    impl Job {
        pub fn new() -> Option<Job> {
            None
        }
        pub fn assign(&self, _child: &std::process::Child) -> bool {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_hold_only_the_plugins_and_their_parameters() {
        assert!(is_ok(&json!({ "weather": { "on": true, "City": "上海" }, "ci": { "Repo": "a/b", "Prs": true } })));
        assert!(is_ok(&json!({ "pomodoro": { "Work": "25", "Break": "" } })));
        assert!(!is_ok(&json!({ "nope": { "on": true } })));
        assert!(!is_ok(&json!({ "mail-imap": { "on": true } })));
        assert!(!is_ok(&json!({ "weather": { "Town": "x" } })));
        assert!(!is_ok(&json!({ "weather": { "City": "a\nb" } })));
        assert!(!is_ok(&json!({ "pomodoro": { "Work": "-1" } })));
        assert!(!is_ok(&json!({ "ci": { "Prs": "yes" } })));
    }

    #[test]
    fn a_script_runs_with_what_it_was_given_quoted() {
        let ci = script("ci").unwrap();
        let line = invocation(ci, &json!({ "Repo": "o'neil/x’y", "Prs": true }), Path::new("C:\\d"), 47213);
        assert_eq!(line, "& 'C:\\d\\ci.ps1' -Repo 'o''neil/x’’y' -Prs -Port 47213");
        let weather = script("weather").unwrap();
        assert_eq!(invocation(weather, &json!({ "City": "  " }), Path::new("C:\\d"), 1), "& 'C:\\d\\weather.ps1' -Port 1");
        assert_eq!(missing(ci, &json!({ "Repo": " " })), Some("Repo"));
        assert_eq!(missing(weather, &json!({})), None);
    }

    #[test]
    fn widgets_are_told_apart_by_plugin() {
        assert_eq!(owner_of("weather"), Some("weather"));
        assert_eq!(owner_of("stock-600519-ss"), Some("stock"));
        assert_eq!(owner_of("prs-a-b"), Some("ci"));
        assert_eq!(owner_of("mail-thunderbird"), None);
        assert_eq!(owner_of("today"), None);
    }

    #[test]
    fn what_scripts_print_reads_short() {
        assert_eq!(tidy("WARNING: no city called X\r"), "no city called X");
        assert_eq!(tidy("警告: location not found"), "location not found");
        assert_eq!(tidy("}\x1b[0m"), "");
        assert_eq!(tidy("\x1b[31;1mInvoke-RestMethod: \x1b[31;1mbad\x1b[0m"), "Invoke-RestMethod: bad");
        assert_eq!(decode("好".as_bytes()), "好");
        assert_eq!((backoff(0), backoff(2), backoff(9)), (Duration::ZERO, Duration::from_secs(30), Duration::from_secs(300)));
    }

    #[cfg(windows)]
    #[test]
    fn the_job_limits_are_the_size_windows_wants() {
        assert_eq!(job::extended_size(), if cfg!(target_pointer_width = "64") { 144 } else { 112 });
    }
}
