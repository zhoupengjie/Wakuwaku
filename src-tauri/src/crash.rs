// Crash reports: what went wrong when she stopped on her own, kept, and on
// the person's word sent to Wakuwaku's issues, what is theirs taken out first.
//
// Caught:
//   - a panic (release builds abort on any): the hook writes
//     crashes/panic-<pid>.json, its words, where, her stack (module+offset)
//   - a native crash (an access violation, a stack overflow): the
//     unhandled-exception filter writes crashes/native-<pid>.txt, allocating
//     nothing (win.rs)
//   - neither, but the exit code of one (0xC0000374, a fast fail): seen by
//     the guard (shell.rs: her exe again, waiting on her, outside her process)
//   - her pages' renderer (or WebView2's browser process) gone: its
//     ProcessFailed; the page loaded again (the browser: she starts anew)
//   - a hang: seen by the guard, a window of hers not answering (Hang):
//     its thread's stack read from outside and kept as crashes/hang-<pid>.txt
//     until it answers again; a report should she end while it is kept
//     (Windows' "not responding" box closed her: 0xCFFFFFFF; Task Manager)
// The guard makes the report as she ends; her next start, for a record no
// guard saw (main.rs runs one while reports can be sent and are not off;
// the taskbar home runs one anyway, shell.rs). A report is crashes/<time>-<kind>-<pid>.json, what is
// sent word for word: her version, Windows' and WebView2's, her home, how long
// she ran, and the last lines of her log from the five minutes before, each
// with what is theirs taken out (redact.rs). Her log goes to recent.log
// always (256 KB, the one before kept as recent.old.log), to debug.log too
// when debugging.
//
// Sent (crashReports): "ask", as she first is, has her ask once, on the
// island and in the settings (General, Crash reports), the report shown;
// the guard asks in a box of Windows' own if she crashes three times in ten
// minutes before she can. "auto" sends each as it comes; "off", none (a no
// clears those kept). Sent to a relay (relay/ in the repo; no token in the
// exe): the Cloudflare one opens an issue on GitHub, or adds to the one for
// the same crash; the one in China, on Gitee; in China (updates.rs
// in_china) that one first, the other should it fail. Through Windows' proxy
// if one is set, else straight. Not sent: kept, tried again at her next start
// and every six hours; refused (a report the relay will not take): put in
// crashes/refused. Sent: crashes/sent, with the issue. Twenty of each at most.
// A build from the source tree keeps them, and never sends.
//
// For tests: WAKUWAKU_CRASH_URL, relays elsewhere (comma-separated; a build
// from the source tree sends then too); WAKUWAKU_CRASH_TEST=<kind>[:<s>], a
// crash of that kind so many seconds in (10 unless said): panic, native (a
// write through null), exit (ended with 0xC0000374, nothing written),
// webview (the renderer killed: reported though killed), or hang (her main
// thread held for 30 s).
mod redact;
#[cfg(windows)]
mod win;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde_json::{json, Map, Value};

use crate::Shared;
pub use redact::Redactor;

// The relays: in front of GitHub's issues (Cloudflare, the Worker
// wakuwaku-crash, issues from the GitHub App wakuwaku-crash-reports) and
// Gitee's (in China: Aliyun Function Compute's wakuwaku-crash, Hangzhou, on
// its default domain; issues from the owner's token for this repo alone).
// One empty is not deployed (relay/README.md): not asked; both empty,
// nothing is sent.
const GITHUB_RELAY: &str = "https://wakuwaku-crash.wakuwaku-crash-relay.workers.dev/report";
const GITEE_RELAY: &str = "https://wakuwaku-crash-cpprabvetg.cn-hangzhou.fcapp.run/report";
const TEST_URL: &str = "WAKUWAKU_CRASH_URL";
const TEST_CRASH: &str = "WAKUWAKU_CRASH_TEST";

const TAIL_MAX: u64 = 256 * 1024;
// The line each run begins with in recent.log ("start: pid 1234, v0.2.2").
const START_LINE: &str = "start: pid ";
const TAIL_WINDOW_MS: u64 = 5 * 60 * 1000;
const TAIL_LINES: usize = 60;
const KEEP: usize = 20;
const RETRY_MS: u64 = 6 * 3_600_000;
// So many crashes within so long, and the guard asks.
const LOOP_COUNT: usize = 3;
const LOOP_MS: u64 = 10 * 60 * 1000;

// Her crashes folder, and when she started (ms), for the hooks.
static DIR: OnceLock<PathBuf> = OnceLock::new();
static STARTED: OnceLock<u64> = OnceLock::new();

pub fn folder(data: &Path) -> PathBuf {
    data.join("crashes")
}

// --- Her log, always ------------------------------------------------------------------

static TAIL: Mutex<Option<(fs::File, u64)>> = Mutex::new(None);

// A line of her log ("<ms> [tauri] …") into recent.log, its breaks as ⏎;
// the file started anew past 256 KB, the one before kept.
pub fn tail(data: &Path, whole: &str) {
    let mut tail = TAIL.lock().unwrap_or_else(|e| e.into_inner());
    write_tail(&mut tail, data, whole);
}

// The same, from the panic hook: not waiting on a lock a panicking line may hold.
fn tail_now(data: &Path, whole: &str) {
    if let Ok(mut tail) = TAIL.try_lock() {
        write_tail(&mut tail, data, whole);
    }
}

fn write_tail(tail: &mut Option<(fs::File, u64)>, data: &Path, whole: &str) {
    let line = format!("{}\n", whole.trim_end_matches(['\r', '\n']).replace("\r\n", "⏎").replace(['\r', '\n'], "⏎"));
    let path = data.join("recent.log");
    let full = tail.as_ref().map_or_else(|| fs::metadata(&path).map_or(0, |m| m.len()), |(_, size)| *size) > TAIL_MAX;
    if tail.is_none() || full {
        *tail = None;
        if full {
            let _ = fs::rename(&path, data.join("recent.old.log"));
        }
        *tail = fs::OpenOptions::new().create(true).append(true).open(&path).ok().map(|f| {
            let size = f.metadata().map_or(0, |m| m.len());
            (f, size)
        });
    }
    let written = tail.as_mut().is_some_and(|(f, size)| {
        let ok = f.write_all(line.as_bytes()).is_ok();
        *size += line.len() as u64;
        ok
    });
    if !written {
        *tail = None;
    }
}

// Something of the person's in a log line (a program's name, say): ‹it›,
// kept in her log, taken out of a report whatever it is (redact.rs).
pub fn private(text: &str) -> String {
    format!("‹{}›", text.replace(['‹', '›'], ""))
}

fn log_line(text: &str) -> String {
    format!("{} [tauri] {text}", crate::now_ms())
}

// --- Caught -----------------------------------------------------------------------------

// As she starts (the one holding the port): the hooks set, and a crash made
// should a test ask for one.
pub fn install(data: &Path) {
    let dir = folder(data);
    let _ = fs::create_dir_all(&dir);
    let _ = DIR.set(dir.clone());
    let started = *STARTED.get_or_init(crate::now_ms);
    #[cfg(windows)]
    win::catch_native(&dir.join(format!("native-{}.txt", std::process::id())), started);
    let _ = started;
    // Where this run begins in her log: a report tells only of its own run.
    tail(data, &log_line(&format!("{}{}, v{}", START_LINE, std::process::id(), crate::updates::current())));
    let before = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        record_panic(info);
        before(info);
        // Built to abort (release): ended now, with the code the abort's
        // fast fail would give, not after Windows' error reporting has kept
        // her (and the taskbar) some seconds more.
        #[cfg(all(windows, panic = "abort"))]
        win::end_with(0xC000_0409);
    }));
    test_crash();
}

fn record_panic(info: &std::panic::PanicHookInfo) {
    let Some(dir) = DIR.get() else { return };
    let message = info.payload().downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| info.payload().downcast_ref::<String>().cloned()).unwrap_or_default();
    let place = info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column())).unwrap_or_default();
    let thread = std::thread::current().name().unwrap_or("").to_string();
    #[cfg(windows)]
    let frames = win::frames(0);
    #[cfg(not(windows))]
    let frames: Vec<String> = Vec::new();
    if let Some(data) = dir.parent() {
        tail_now(data, &log_line(&format!("panic in thread {thread}: {message} at {place}")));
    }
    let record = json!({ "time": crate::now_ms(), "started": STARTED.get(), "message": message, "place": place, "thread": thread, "frames": frames });
    // The first panic kept (only a debug build lives on after one).
    if let Ok(mut f) = fs::OpenOptions::new().write(true).create_new(true).open(dir.join(format!("panic-{}.json", std::process::id()))) {
        let _ = f.write_all(record.to_string().as_bytes());
    }
}

// The crash a test asks for, and how many seconds in.
fn test_asked() -> Option<(String, u64)> {
    let asked = std::env::var(TEST_CRASH).ok()?;
    Some(match asked.split_once(':') {
        Some((kind, secs)) => (kind.to_string(), secs.parse().unwrap_or(10)),
        None => (asked, 10),
    })
}

fn test_crash() {
    let Some((kind, secs)) = test_asked() else { return };
    // A hang needs her main thread, there once she is up (start).
    if kind == "webview" || kind == "hang" {
        return;
    }
    crate::spawn_named("crash test", move || {
        std::thread::sleep(Duration::from_secs(secs));
        match kind.as_str() {
            "panic" => panic!("crash test: a panic, as {TEST_CRASH} asked"),
            // SAFETY: none: a crash, as the test asked (an address the
            // compiler cannot see is null).
            "native" => unsafe { std::ptr::write_volatile(std::hint::black_box(0usize) as *mut u32, 1) },
            #[cfg(windows)]
            "exit" => win::end_with(0xC000_0374),
            _ => {}
        }
    });
}

// A hang, should a test ask for one: her main thread held 30 s, her windows
// not answering (the guard sees it at five).
fn test_hang(sh: &Arc<Shared>) {
    let Some(("hang", secs)) = test_asked().as_ref().map(|(k, s)| (k.as_str(), *s)) else { return };
    let sh = sh.clone();
    crate::spawn_named("crash test", move || {
        std::thread::sleep(Duration::from_secs(secs));
        sh.log("crash test: her main thread held 30 s");
        let _ = sh.app.run_on_main_thread(|| std::thread::sleep(Duration::from_secs(30)));
    });
}

fn testing_webview() -> bool {
    std::env::var(TEST_CRASH).is_ok_and(|v| v.starts_with("webview"))
}

// Exit codes of a crash: Windows' errors (0xC…: an access violation, a
// stack overflow, a fast fail as Rust's abort is, a heap broken), a
// breakpoint, a C++ exception, and Rust's 101 (a panic, built to unwind).
// Not 0, not 1 (ended by Task Manager or taskkill), not 0xFFFFFFFF (ended
// by PowerShell's Stop-Process or .NET's Kill: TerminateProcess(-1)), not
// Ctrl+C's, not 0x40010004 (ended as a debugger ends one: by Windows at a
// logoff, by a tool).
pub fn is_crash_code(code: u32) -> bool {
    match code {
        0xC000_013A | 0xFFFF_FFFF => false,
        101 | 0x8000_0003 | 0xE06D_7363 => true,
        c => c & 0xC000_0000 == 0xC000_0000,
    }
}

// A crash as found, before its report.
#[derive(Default)]
struct Found {
    kind: &'static str,
    pid: u32,
    code: Option<String>,
    place: Option<String>,
    message: Option<String>,
    detail: Option<String>,
    frames: Vec<String>,
    // When (ms), and how long she had run (s).
    at: u64,
    uptime: Option<u64>,
}

fn modified_ms(path: &Path) -> Option<u64> {
    let t = fs::metadata(path).ok()?.modified().ok()?;
    Some(t.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as u64)
}

fn is_frame(f: &str) -> bool {
    f.split_once("+0x").is_some_and(|(m, off)| (1..=64).contains(&m.len()) && m.chars().all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c)) && (1..=12).contains(&off.len()) && off.chars().all(|c| c.is_ascii_hexdigit()))
}

// What pid left: its records (a panic's, a native crash's), or else its
// exit code if one of a crash. The records gone once read.
fn found_for(dir: &Path, pid: u32, code: Option<u32>, started: Option<u64>, ended: u64) -> Option<Found> {
    let code_text = code.map(|c| format!("0x{c:08X}"));
    let uptime = |at: u64, from: Option<u64>| from.filter(|&s| s > 0 && s <= at).map(|s| (at - s) / 1000);
    // Each record taken by renaming it first: the guard and her next start
    // (the SessionStart hook starts her again at once) never both make its report.
    let take = |name: String| {
        let taken = dir.join(format!("{name}.taking"));
        fs::rename(dir.join(&name), &taken).ok().map(|_| taken)
    };
    let panic_file = take(format!("panic-{pid}.json")).unwrap_or_default();
    let native_file = take(format!("native-{pid}.txt")).unwrap_or_default();
    let hang_file = take(format!("hang-{pid}.txt")).unwrap_or_default();
    let mut found = None;
    if let Some(v) = fs::read_to_string(&panic_file).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
        let at = v["time"].as_u64().unwrap_or(ended);
        let thread = v["thread"].as_str().filter(|t| !t.is_empty()).map(|t| format!("thread {t}"));
        found = Some(Found {
            kind: "panic",
            pid,
            code: code_text.clone(),
            place: v["place"].as_str().map(str::to_string),
            message: v["message"].as_str().map(str::to_string),
            detail: thread,
            frames: v["frames"].as_array().into_iter().flatten().filter_map(Value::as_str).filter(|f| is_frame(f)).map(str::to_string).collect(),
            at,
            uptime: uptime(at, v["started"].as_u64().or(started)),
        });
    } else if let Ok(text) = fs::read_to_string(&native_file) {
        let at = modified_ms(&native_file).unwrap_or(ended);
        let field = |key: &str| text.lines().find_map(|l| l.strip_prefix(key).map(str::to_string));
        let place = field("where=").filter(|w| is_frame(w));
        let mut frames: Vec<String> = text.lines().filter_map(|l| l.strip_prefix("frame=")).filter(|f| is_frame(f)).map(str::to_string).collect();
        // From where it happened on: those above are the filter's and Windows' handling.
        if let Some(i) = place.as_ref().and_then(|p| frames.iter().position(|f| f == p)) {
            frames.drain(..i);
        }
        found = Some(Found {
            kind: "native",
            pid,
            code: field("code=").map(|c| format!("0x{}", c.to_uppercase())).or(code_text.clone()),
            place,
            message: None,
            detail: field("access=").map(|a| format!("access: {a}")),
            frames,
            at,
            uptime: uptime(at, field("started=").and_then(|s| s.parse().ok()).or(started)),
        });
    } else if let Some(text) = fs::read_to_string(&hang_file).ok().filter(|_| code.is_some_and(|c| c != 0)) {
        // Ended while not answering, whatever ended her (an exit code of a
        // crash or not: Windows' box, Task Manager); her own frame the place.
        let field = |key: &str| text.lines().find_map(|l| l.strip_prefix(key).map(str::to_string));
        let frames: Vec<String> = text.lines().filter_map(|l| l.strip_prefix("frame=")).filter(|f| is_frame(f)).map(str::to_string).collect();
        let since = field("seen=").and_then(|s| s.parse::<u64>().ok()).map_or(HUNG_AFTER_S, |seen| ended.saturating_sub(seen) / 1000 + HUNG_AFTER_S);
        found = Some(Found {
            kind: "exit",
            pid,
            code: code_text.clone(),
            place: frames.iter().find(|f| f.starts_with("wakuwaku")).cloned(),
            detail: Some(format!("hang: {} not answering, {since} s and more before she ended", thread_said(&field("window=").unwrap_or_default()))),
            frames,
            at: ended,
            uptime: uptime(ended, started),
            ..Found::default()
        });
    } else if code.is_some_and(is_crash_code) {
        found = Some(Found { kind: "exit", pid, code: code_text, at: ended, uptime: uptime(ended, started), ..Found::default() });
    }
    let _ = fs::remove_file(&panic_file);
    let _ = fs::remove_file(&native_file);
    let _ = fs::remove_file(&hang_file);
    found
}

// --- The report --------------------------------------------------------------------------

// What the settings know is theirs: the mail accounts' addresses, names, users.
fn theirs_in(settings: &Map<String, Value>) -> Vec<String> {
    let mut words = Vec::new();
    for account in settings.get("mail").and_then(Value::as_array).into_iter().flatten() {
        for key in ["address", "name", "username", "id"] {
            if let Some(s) = account[key].as_str() {
                words.push(s.to_string());
            }
        }
        if let Some(s) = account["smtp"]["username"].as_str() {
            words.push(s.to_string());
        }
    }
    words
}

// "2026-10-11T04:22Z": UTC, to the minute.
fn iso_minute(ms: u64) -> String {
    let secs = ms / 1000;
    let days = (secs / 86_400) as i64;
    let (hour, minute) = ((secs % 86_400) / 3600, (secs % 3600) / 60);
    // Days since 1970 to a date (Howard Hinnant's civil_from_days).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}Z")
}

// The log's last lines before `at` (five minutes of them, sixty at most),
// from pid's start on when its start line is there (an earlier run's
// lines are another story), as "-4.1s <line>", each redacted.
fn log_before(data: &Path, at: u64, pid: u32, r: &Redactor) -> Vec<String> {
    let mut lines = Vec::new();
    for name in ["recent.old.log", "recent.log"] {
        let Ok(text) = fs::read(data.join(name)) else { continue };
        let text = String::from_utf8_lossy(&text);
        for line in text.lines() {
            let Some((ms, rest)) = line.split_once(' ') else { continue };
            let Ok(ms) = ms.parse::<u64>() else { continue };
            if ms + TAIL_WINDOW_MS < at || ms > at + 2000 {
                continue;
            }
            let rest = rest.strip_prefix("[tauri] ").unwrap_or(rest);
            lines.push((ms, rest.to_string()));
        }
    }
    lines.sort_by_key(|(ms, _)| *ms);
    let start = format!("{START_LINE}{pid},");
    if let Some(i) = lines.iter().rposition(|(_, text)| text.starts_with(&start)) {
        lines.drain(..i);
    }
    let from = lines.len().saturating_sub(TAIL_LINES);
    lines[from..].iter().map(|(ms, text)| format!("-{:.1}s {}", at.saturating_sub(*ms) as f64 / 1000.0, r.text(text))).collect()
}

fn report_of(data: &Path, f: &Found, theirs: &[String]) -> Value {
    let settings = crate::data::load(data);
    let mut words = theirs.to_vec();
    words.extend(theirs_in(&settings));
    let r = Redactor::here(data, &words);
    let text = |s: &Option<String>| s.as_deref().map(|s| r.text(s)).filter(|s| !s.is_empty());
    let mut report = json!({
        "v": 1,
        "app": crate::updates::current(),
        "kind": f.kind,
        "code": f.code,
        "where": text(&f.place),
        "message": text(&f.message),
        "detail": text(&f.detail),
        "frames": f.frames.iter().take(32).collect::<Vec<_>>(),
        "home": settings.get("display").and_then(Value::as_str).unwrap_or("island"),
        "out": settings.get("out").and_then(Value::as_bool).unwrap_or(false),
        "lang": crate::i18n::lang(settings.get("lang").and_then(Value::as_str).unwrap_or("auto")),
        "uptime": f.uptime,
        "at": iso_minute(f.at),
        "log": log_before(data, f.at, f.pid, &r),
    });
    #[cfg(windows)]
    {
        report["os"] = json!(win::os());
        report["webview2"] = json!(win::webview_version());
    }
    // Nothing for what is not known.
    if let Value::Object(map) = &mut report {
        map.retain(|_, v| !v.is_null() && v.as_array().is_none_or(|a| !a.is_empty()));
    }
    report
}

// Kept as crashes/<time>-<kind>-<pid>.json; twenty at most waiting.
fn save(data: &Path, f: &Found, report: &Value) -> Option<PathBuf> {
    let dir = folder(data);
    let _ = fs::create_dir_all(&dir);
    let stamp = iso_minute(f.at).replace(['-', ':', 'T', 'Z'], "");
    let secs = (f.at / 1000) % 60;
    let path = dir.join(format!("{stamp}{secs:02}-{}-{}.json", f.kind, f.pid));
    fs::write(&path, serde_json::to_string_pretty(report).ok()?).ok()?;
    let waiting = pending(data);
    for old in waiting.iter().take(waiting.len().saturating_sub(KEEP)) {
        let _ = fs::remove_file(old);
    }
    Some(path)
}

// The reports waiting, oldest first.
fn pending(data: &Path) -> Vec<PathBuf> {
    let mut list: Vec<PathBuf> = fs::read_dir(folder(data))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json") && p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.as_bytes().first().is_some_and(u8::is_ascii_digit)))
        .collect();
    list.sort();
    list
}

// Into crashes/<sub>, twenty kept there.
fn keep_in(data: &Path, sub: &str, name: &str, content: &str) {
    let dir = folder(data).join(sub);
    let _ = fs::create_dir_all(&dir);
    let _ = fs::write(dir.join(name), content);
    let mut all: Vec<PathBuf> = fs::read_dir(&dir).into_iter().flatten().flatten().map(|e| e.path()).collect();
    all.sort();
    for old in all.iter().take(all.len().saturating_sub(KEEP)) {
        let _ = fs::remove_file(old);
    }
}

// --- Sent ---------------------------------------------------------------------------------

fn mode_of(settings: &Map<String, Value>) -> &'static str {
    match settings.get("crashReports").and_then(Value::as_str) {
        Some("auto") => "auto",
        Some("off") => "off",
        _ => "ask",
    }
}

fn test_urls() -> Option<Vec<String>> {
    let v = std::env::var(TEST_URL).ok()?;
    let urls: Vec<String> = v.split(',').map(|u| u.trim().to_string()).filter(|u| !u.is_empty()).collect();
    (!urls.is_empty()).then_some(urls)
}

// Where to send, the one for here first.
fn relays() -> Vec<String> {
    if let Some(urls) = test_urls() {
        return urls;
    }
    let mut list = vec![GITHUB_RELAY, GITEE_RELAY];
    if crate::updates::in_china() {
        list.reverse();
    }
    list.into_iter().filter(|u| !u.is_empty()).map(str::to_string).collect()
}

// Whether anything can be sent: a relay to send to, and a downloaded exe (or a test).
pub fn can_send() -> bool {
    !relays().is_empty() && (!crate::data::from_source() || test_urls().is_some())
}

enum Fail {
    // Not now (offline, the relay busy or down): tried again later.
    Later(String),
    // Not taken (the report itself): not tried again.
    Refused(String),
}

// One relay asked: through Windows' proxy first if one is set (and none in
// the environment, which ureq takes by itself), then as is (proxy::web_ways).
fn post(url: &str, body: &str) -> Result<Value, Fail> {
    let mut last = Fail::Later("no way".into());
    for way in crate::proxy::web_ways(url) {
        let Some(agent) = way.agent(Duration::from_secs(20)) else { continue };
        let sent = agent
            .post(url)
            .header("content-type", "application/json")
            .header("user-agent", &format!("wakuwaku/{}", crate::updates::current()))
            .header("x-wakuwaku", "1")
            .send(body);
        match sent {
            Ok(mut res) => return Ok(res.body_mut().read_json::<Value>().unwrap_or(json!({}))),
            Err(ureq::Error::StatusCode(429)) => return Err(Fail::Later("429: too many".into())),
            Err(ureq::Error::StatusCode(code)) if (400..500).contains(&code) => return Err(Fail::Refused(format!("{code}"))),
            Err(ureq::Error::StatusCode(code)) => return Err(Fail::Later(format!("{code}"))),
            Err(e) => last = Fail::Later(format!("{}: {e}", way.name())),
        }
    }
    Err(last)
}

// To the relays in turn, until one takes it or refuses it.
fn send(body: &str) -> Result<(String, Value), Fail> {
    let mut last = Fail::Later("no relay".into());
    for url in relays() {
        match post(&url, body) {
            Ok(answer) => return Ok((url, answer)),
            Err(Fail::Refused(why)) => return Err(Fail::Refused(format!("{url}: {why}"))),
            Err(Fail::Later(why)) => last = Fail::Later(format!("{url}: {why}")),
        }
    }
    Err(last)
}

// What the settings show: sending now, the last error, the last sent.
#[derive(Default)]
struct Status {
    sending: bool,
    error: Option<String>,
    next_at: u64,
}

static STATUS: Mutex<Status> = Mutex::new(Status { sending: false, error: None, next_at: 0 });

// Those waiting sent (five at a time), each claimed first (renamed
// .sending) so the guard and she never send one twice. Whether all went.
fn send_waiting(data: &Path, log: &dyn Fn(&str)) -> bool {
    for path in pending(data).into_iter().take(5) {
        let claimed = path.with_extension("sending");
        if fs::rename(&path, &claimed).is_err() {
            continue;
        }
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let body = fs::read_to_string(&claimed).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
        let Some(report) = body else {
            let _ = fs::remove_file(&claimed);
            continue;
        };
        match send(&report.to_string()) {
            Ok((url, answer)) => {
                let kept = json!({ "sentAt": crate::now_ms(), "to": url, "answer": answer, "report": report });
                keep_in(data, "sent", &name, &serde_json::to_string_pretty(&kept).unwrap_or_default());
                let _ = fs::remove_file(&claimed);
                log(&format!("crash: {name} sent: {}", answer["url"].as_str().unwrap_or("taken")));
                STATUS.lock().unwrap().error = None;
            }
            Err(Fail::Refused(why)) => {
                keep_in(data, "refused", &name, &serde_json::to_string_pretty(&report).unwrap_or_default());
                let _ = fs::remove_file(&claimed);
                log(&format!("crash: {name} refused ({why})"));
            }
            Err(Fail::Later(why)) => {
                let _ = fs::rename(&claimed, &path);
                log(&format!("crash: {name} not sent ({why}): later"));
                STATUS.lock().unwrap().error = Some(why);
                return false;
            }
        }
    }
    true
}

// Claims left by a process that ended while sending: back to waiting; a
// record taken by one that ended before its report: given back too.
fn unclaim(data: &Path) {
    for entry in fs::read_dir(folder(data)).into_iter().flatten().flatten() {
        let path = entry.path();
        let old = modified_ms(&path).is_some_and(|m| m + 10 * 60 * 1000 < crate::now_ms());
        if old && path.extension().is_some_and(|x| x == "sending") {
            let _ = fs::rename(&path, path.with_extension("json"));
        }
        if old && path.extension().is_some_and(|x| x == "taking") {
            let _ = fs::rename(&path, path.with_extension(""));
        }
    }
}

// The settings file's own entries with crashReports set (not her defaults:
// the guard writes it while she is gone).
fn set_mode_in_file(data: &Path, mode: &str) {
    let mut saved: Map<String, Value> = fs::read_to_string(data.join("config.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    saved.insert("crashReports".into(), json!(mode));
    crate::data::save(data, &saved);
}

fn clear_waiting(data: &Path) {
    for path in pending(data) {
        let _ = fs::remove_file(path);
    }
}

// --- The guard ----------------------------------------------------------------------------

// How long a window goes without answering before Windows takes it for not
// answering (IsHungAppWindow): before the guard can see it.
const HUNG_AFTER_S: u64 = 5;

// Which thread of hers a window's class says it is.
fn thread_said(class: &str) -> String {
    match class {
        "Tauri Window" | "Tao Thread Event Target" | "tray_icon_app" => "her main thread".to_string(),
        "WakuwakuTaskbarHost" | "Shell_TrayWnd" | "WakuwakuThumbs" => "her taskbar's thread".to_string(),
        "" => "a thread of hers".to_string(),
        other => format!("a thread of hers ({})", other.chars().filter(|c| c.is_ascii_alphanumeric() || " _-".contains(*c)).take(40).collect::<String>()),
    }
}

// A hang, as the guard looks for one every two seconds while she runs
// (shell.rs guard): a thread of hers with a window Windows takes for not
// answering (win::hung), its stack read from outside then (win::stack_of)
// and kept with what it is as crashes/hang-<pid>.txt, and said in her log;
// once it answers again, the record gone and that said too. Should she end
// while it is kept, her report is of it (found_for).
#[derive(Default)]
pub struct Hang {
    since: Option<u64>,
}

impl Hang {
    pub fn look(&mut self, data: &Path, pid: u32) {
        #[cfg(windows)]
        {
            let file = folder(data).join(format!("hang-{pid}.txt"));
            let log = |line: &str| tail(data, &log_line(&format!("guard: {line}")));
            match (self.since, win::hung(pid).first()) {
                (None, Some((thread, class))) => {
                    let now = crate::now_ms();
                    let frames = win::stack_of(pid, *thread);
                    let mut text = format!("seen={now}\nthread={thread}\nwindow={class}\n");
                    for f in &frames {
                        text.push_str(&format!("frame={f}\n"));
                    }
                    let _ = fs::create_dir_all(folder(data));
                    let _ = fs::write(&file, text);
                    let hers = frames.iter().find(|f| f.starts_with("wakuwaku")).map(|f| format!(", hers last {f}")).unwrap_or_default();
                    log(&format!("pid {pid}: {} not answering ({HUNG_AFTER_S} s and more): its stack kept, {} frames{hers}", thread_said(class), frames.len()));
                    self.since = Some(now);
                }
                (Some(since), None) => {
                    let _ = fs::remove_file(&file);
                    log(&format!("pid {pid}: answering again, {} s and more after it stopped", crate::now_ms().saturating_sub(since) / 1000 + HUNG_AFTER_S));
                    self.since = None;
                }
                _ => {}
            }
        }
        #[cfg(not(windows))]
        let _ = (data, pid);
    }
}

// The guard, once she has ended (pid, its exit code, when it started and
// ended, ms): a crash's report made; sent if she may; asked about in a box if
// she crashed three times in ten minutes and was never asked.
pub fn after_exit(data: &Path, pid: u32, code: Option<u32>, started: Option<u64>) {
    let ended = crate::now_ms();
    let Some(found) = found_for(&folder(data), pid, code, started, ended) else { return };
    let report = report_of(data, &found, &[]);
    let saved = save(data, &found, &report);
    let log = |line: &str| tail(data, &log_line(&format!("guard: {line}")));
    log(&format!("pid {pid} ended ({}): {} report {}", found.code.as_deref().unwrap_or("?"), found.kind, saved.map(|p| p.display().to_string()).unwrap_or_else(|| "not written".into())));
    if !can_send() {
        return;
    }
    let settings = crate::data::load(data);
    match mode_of(&settings) {
        "auto" => {
            send_waiting(data, &log);
        }
        "ask" => {
            let since = ended.saturating_sub(LOOP_MS);
            let recent = pending(data).iter().filter(|p| modified_ms(p).is_some_and(|m| m >= since)).count();
            if recent < LOOP_COUNT {
                return;
            }
            #[cfg(windows)]
            {
                let lang = crate::i18n::lang(settings.get("lang").and_then(Value::as_str).unwrap_or("auto"));
                let text = crate::i18n::t(lang, "crash.box").replace("{n}", &recent.to_string());
                if win::ask("Wakuwaku", &text) {
                    set_mode_in_file(data, "auto");
                    log("asked in a box: send, from now on");
                    send_waiting(data, &log);
                } else {
                    set_mode_in_file(data, "off");
                    clear_waiting(data);
                    log("asked in a box: never");
                }
            }
        }
        _ => {}
    }
}

// Whether she wants the guard for her reports: not off, and somewhere to
// send them (without, a panic's or a native crash's record still becomes a
// report at her next start; the taskbar home has its guard anyway).
pub fn wants_guard(data: &Path) -> bool {
    can_send() && mode_of(&crate::data::load(data)) != "off"
}

// --- In her --------------------------------------------------------------------------------

// As she starts: claims left over given back, records no guard saw made
// reports; then, twenty seconds in and every ten minutes: those waiting
// sent when they may and it is time, or asked about.
pub fn start(sh: &Arc<Shared>) {
    let data = sh.dir.clone();
    unclaim(&data);
    let me = std::process::id();
    for entry in fs::read_dir(folder(&data)).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        // A hang's record alone, its end not known (no guard saw it): gone.
        let pid = name.strip_prefix("panic-").or_else(|| name.strip_prefix("native-")).or_else(|| name.strip_prefix("hang-")).and_then(|r| r.split('.').next()).and_then(|p| p.parse::<u32>().ok());
        let Some(pid) = pid.filter(|&p| p != me && crate::shell::started(p).is_none()) else { continue };
        if let Some(found) = found_for(&folder(&data), pid, None, None, modified_ms(&entry.path()).unwrap_or_else(crate::now_ms)) {
            let report = report_of(&data, &found, &[]);
            let saved = save(&data, &found, &report);
            sh.log(&format!("crash: pid {pid} left a {} record: report {}", found.kind, saved.map(|p| p.display().to_string()).unwrap_or_default()));
        }
    }
    test_hang(sh);
    let sh = sh.clone();
    crate::spawn_named("crash reports", move || {
        std::thread::sleep(Duration::from_secs(20));
        loop {
            tick(&sh);
            std::thread::sleep(Duration::from_secs(600));
        }
    });
}

fn tick(sh: &Arc<Shared>) {
    if !can_send() || pending(&sh.dir).is_empty() {
        return;
    }
    // Bound first: sending asks for the settings again.
    let mode = mode_of(&sh.settings.lock().unwrap());
    let due = crate::now_ms() >= STATUS.lock().unwrap().next_at;
    match mode {
        "auto" if due => send_now(sh),
        "ask" => tell(sh),
        _ => {}
    }
}

static SENDING: AtomicBool = AtomicBool::new(false);

// Those waiting sent, on a thread of its own; the settings told as it goes.
pub fn send_now(sh: &Arc<Shared>) {
    if SENDING.swap(true, Ordering::SeqCst) {
        return;
    }
    STATUS.lock().unwrap().sending = true;
    sh.push_settings();
    let sh = sh.clone();
    std::thread::spawn(move || {
        let log = |line: &str| sh.log(line);
        let all = send_waiting(&sh.dir, &log);
        {
            let mut s = STATUS.lock().unwrap();
            s.sending = false;
            s.next_at = if all { 0 } else { crate::now_ms() + RETRY_MS };
        }
        SENDING.store(false, Ordering::SeqCst);
        sh.push_settings();
    });
}

// The newest waiting told on the island, once (crashTold): only while she is
// in sight and no session wants you.
fn tell(sh: &Arc<Shared>) {
    let Some(newest) = pending(&sh.dir).pop() else { return };
    let id = newest.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    if sh.setting("crashTold").as_str() == Some(id.as_str()) || !sh.is_visible() || sh.pet.lock().unwrap().wants_you() {
        return;
    }
    let lang = sh.lang();
    sh.to_pages("pet:nudge", json!({ "id": "crash", "icon": "flag", "words": crate::i18n::t(lang, "crash.out"), "label": crate::i18n::t(lang, "crash.tap"), "value": "", "private": false, "open": { "tab": "crash" } }));
    sh.change(json!({ "crashTold": id }));
    sh.log(&format!("crash: {id} told on the island"));
}

// The answer, from the settings: yes (send now, and from now on), or no
// (never; those kept cleared).
pub fn answer(sh: &Arc<Shared>, yes: bool) {
    if yes {
        sh.change(json!({ "crashReports": "auto" }));
        STATUS.lock().unwrap().next_at = 0;
        send_now(sh);
    } else {
        sh.change(json!({ "crashReports": "off" }));
        clear_waiting(&sh.dir);
    }
    sh.log(&format!("crash: reports {}", if yes { "sent from now on" } else { "never sent" }));
}

// For the settings: as set, whether it can send, those waiting (the newest
// one whole, as it would be sent), sending now, what went wrong, the last sent.
// A report's file as last read into the view, kept while it is the same
// file at the same time: the view is made for every settings snapshot (up
// to four a second while they are open), and a report holds five minutes
// of log.
static VIEWED: Mutex<Option<std::collections::HashMap<&'static str, (PathBuf, Option<u64>, Value)>>> = Mutex::new(None);

fn viewed(which: &'static str, path: Option<&Path>, read: impl FnOnce(&Path) -> Option<Value>) -> Option<Value> {
    let path = path?;
    let at = modified_ms(path);
    let mut kept = VIEWED.lock().unwrap_or_else(|e| e.into_inner());
    let kept = kept.get_or_insert_with(std::collections::HashMap::new);
    if let Some((p, t, value)) = kept.get(which) {
        if p == path && *t == at {
            return Some(value.clone());
        }
    }
    let value = read(path)?;
    kept.insert(which, (path.to_path_buf(), at, value.clone()));
    Some(value)
}

pub fn view(sh: &Shared) -> Value {
    let waiting = pending(&sh.dir);
    let newest = viewed("newest", waiting.last().map(PathBuf::as_path), |p| {
        let text = fs::read_to_string(p).ok()?;
        let report: Value = serde_json::from_str(&text).ok()?;
        Some(json!({ "id": p.file_stem()?.to_string_lossy(), "at": modified_ms(p), "kind": report["kind"], "where": report["where"], "text": text }))
    });
    let mut sent: Vec<PathBuf> = fs::read_dir(folder(&sh.dir).join("sent")).into_iter().flatten().flatten().map(|e| e.path()).collect();
    sent.sort();
    let last = viewed("sent", sent.last().map(PathBuf::as_path), |p| {
        let v = serde_json::from_str::<Value>(&fs::read_to_string(p).ok()?).ok()?;
        Some(json!({ "at": v["sentAt"], "url": v["answer"]["url"], "number": v["answer"]["number"], "count": v["answer"]["count"] }))
    });
    let s = STATUS.lock().unwrap();
    json!({
        "mode": mode_of(&sh.settings.lock().unwrap()),
        "can": can_send(),
        "waiting": waiting.len(),
        "newest": newest,
        "sending": s.sending,
        "error": s.error,
        "last": last,
    })
}

// The last sent's issue opened: one on her own repos only.
pub fn open_last(sh: &Shared) -> Option<String> {
    let mut sent: Vec<PathBuf> = fs::read_dir(folder(&sh.dir).join("sent")).into_iter().flatten().flatten().map(|e| e.path()).collect();
    sent.sort();
    let v: Value = serde_json::from_str(&fs::read_to_string(sent.last()?).ok()?).ok()?;
    let url = v["answer"]["url"].as_str()?.to_string();
    let ours = ["https://github.com/zhoupengjie/Wakuwaku/issues/", "https://gitee.com/zhoupengjie/Wakuwaku/issues/"];
    (ours.iter().any(|p| url.starts_with(p)) || test_urls().is_some()).then_some(url)
}

// --- WebView2 ------------------------------------------------------------------------------

#[cfg(windows)]
pub fn watch(sh: &Arc<Shared>, window: &tauri::WebviewWindow) {
    if let Err(e) = win::watch(sh, window) {
        sh.log(&format!("crash: {} not watched: {e}", window.label()));
    }
}

#[cfg(not(windows))]
pub fn watch(_sh: &Arc<Shared>, _window: &tauri::WebviewWindow) {}

// Whether one of WebView2's processes gone is a crash (WebView2's numbers):
// her renderer gone or not answering, or the browser process; not one ended
// from outside. WebView2 does not know who ended one: the browser process
// ended by Windows at a logoff, a debugger or a tool (0x40010004) comes as
// "unexpected", the renderer ended by Stop-Process (0xFFFFFFFF) as
// "crashed"; their exit codes tell. A renderer not answering is still there
// (no exit code); for one that could not start, the code says why, not who.
fn webview_crash(kind: i32, reason: i32, code: i32) -> bool {
    let told_by_code = matches!(kind, 0 | 1) && reason != 4;
    matches!(kind, 0..=2) && reason != 2 && (!told_by_code || is_crash_code(code as u32))
}

// One of WebView2's processes gone (its kind, why, its exit code: WebView2's
// numbers): said in the log; its report made and sent or told, for a crash
// (webview_crash; twice in ten seconds, one report: her two pages share one
// renderer); she starts anew for the browser, once she has run a minute,
// crashed or ended. Whether to load the page again (her renderer gone).
pub fn webview_failed(sh: &Arc<Shared>, label: &str, kind: i32, reason: i32, code: i32) -> bool {
    const KINDS: [&str; 10] = ["browser-exited", "render-exited", "render-unresponsive", "frame-render-exited", "utility-exited", "sandbox-helper-exited", "gpu-exited", "ppapi-plugin-exited", "ppapi-broker-exited", "unknown-exited"];
    const REASONS: [&str; 7] = ["unexpected", "unresponsive", "terminated", "crashed", "launch-failed", "out-of-memory", "profile-deleted"];
    let kind_name = KINDS.get(kind as usize).copied().unwrap_or("unknown-exited");
    let reason_name = REASONS.get(reason as usize).copied().unwrap_or("other");
    sh.log(&format!("webview: {label}: {kind_name} ({reason_name}), exit 0x{:X}", code as u32));
    let reportable = webview_crash(kind, reason, code) || (matches!(kind, 0..=2) && testing_webview());
    static LAST: Mutex<u64> = Mutex::new(0);
    let now = crate::now_ms();
    let again = {
        let mut last = LAST.lock().unwrap();
        let again = now < *last + 10_000;
        *last = now;
        again
    };
    if reportable && !again {
        let found = Found {
            kind: "webview",
            pid: std::process::id(),
            // Its exit code in the signature: out of memory, an access
            // violation, a failed check are different crashes.
            code: Some(format!("{kind_name}/{reason_name}/0x{:08X}", code as u32)),
            detail: Some(format!("window {label}")),
            at: now,
            uptime: STARTED.get().map(|s| now.saturating_sub(*s) / 1000),
            ..Found::default()
        };
        let sh = sh.clone();
        std::thread::spawn(move || {
            let names = sh.pet.lock().unwrap().names();
            let report = report_of(&sh.dir, &found, &names);
            let saved = save(&sh.dir, &found, &report);
            sh.log(&format!("crash: webview report {}", saved.map(|p| p.display().to_string()).unwrap_or_default()));
            let mode = mode_of(&sh.settings.lock().unwrap());
            if can_send() && mode == "auto" {
                send_now(&sh);
            } else {
                tell(&sh);
            }
            sh.push_settings();
        });
    }
    // Once: each of her windows hears the browser go.
    static RESTARTING: AtomicBool = AtomicBool::new(false);
    if kind == 0 && STARTED.get().is_some_and(|s| now > s + 60_000) && !RESTARTING.swap(true, Ordering::SeqCst) {
        let sh = sh.clone();
        std::thread::spawn(move || {
            sh.log("crash: WebView2's browser process gone: starting anew");
            match crate::updates::start_again(&sh) {
                Ok(()) => sh.quit(crate::data::Quit::Wakeable, "a restart's handover"),
                Err(e) => sh.log(&format!("crash: could not start anew: {e}")),
            }
        });
    }
    matches!(kind, 1 | 2)
}

#[cfg(test)]
mod tests {
    use super::{found_for, is_crash_code, is_frame, iso_minute, thread_said, webview_crash};

    #[test]
    fn crash_codes() {
        for code in [0xC000_0005, 0xC000_00FD, 0xC000_0409, 0xC000_0374, 101, 0x8000_0003] {
            assert!(is_crash_code(code), "{code:#x}");
        }
        for code in [0, 1, 0xC000_013A, 0x4001_0004, 259, 0xFFFF_FFFF] {
            assert!(!is_crash_code(code), "{code:#x}");
        }
    }

    #[test]
    fn webview_crashes() {
        // (kind, reason, exit code), WebView2's numbers.
        let crashes = [
            (0, 0, 0xC000_0005u32), // the browser process, an access violation
            (0, 3, 0x8000_0003),    // the browser process, a failed check
            (1, 3, 0xC000_0005),    // her renderer
            (1, 5, 0xE000_0008),    // her renderer, out of memory
            (1, 4, 5),              // her renderer could not start
            (2, 1, 0),              // her renderer not answering
        ];
        for (kind, reason, code) in crashes {
            assert!(webview_crash(kind, reason, code as i32), "{kind} {reason} {code:#x}");
        }
        let not = [
            (0, 0, 0x4001_0004u32), // the browser process ended from outside (#93)
            (0, 0, 1),              // by Task Manager
            (1, 3, 0xFFFF_FFFF),    // her renderer by Stop-Process
            (1, 2, 0xC000_0005),    // her renderer, ended as WebView2 says
            (3, 3, 0xC000_0005),    // a frame's renderer, not hers
            (6, 0, 0xC000_0005),    // the GPU process: WebView2 starts it anew
        ];
        for (kind, reason, code) in not {
            assert!(!webview_crash(kind, reason, code as i32), "{kind} {reason} {code:#x}");
        }
    }

    #[test]
    fn frames_checked() {
        assert!(is_frame("wakuwaku.exe+0x4c1d20"));
        assert!(is_frame("WebView2Loader.dll+0x1a2b3"));
        assert!(!is_frame("C:\\x.dll+0x10"));
        assert!(!is_frame("x.dll+0xzz"));
        assert!(!is_frame("x.dll"));
    }

    #[test]
    fn minutes() {
        assert_eq!(iso_minute(0), "1970-01-01T00:00Z");
        assert_eq!(iso_minute(1_791_692_553_000), "2026-10-11T04:22Z");
        assert_eq!(iso_minute(951_782_400_000), "2000-02-29T00:00Z");
    }

    #[test]
    fn records_read() {
        let dir = std::env::temp_dir().join(format!("wakuwaku-crash-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("native-77.txt"), "code=c0000005\nwhere=ntdll.dll+0x1a2b\naccess=write null+0x18\nstarted=1000\nthread=9\nframe=wakuwaku.exe+0x99\nframe=KERNELBASE.dll+0x5\nframe=ntdll.dll+0x1a2b\nframe=C:\\bad+0x1\nframe=wakuwaku.exe+0x10\n").unwrap();
        let f = found_for(&dir, 77, Some(0xC000_0005), None, 5000).unwrap();
        assert_eq!(f.kind, "native");
        assert_eq!(f.code.as_deref(), Some("0xC0000005"));
        assert_eq!(f.place.as_deref(), Some("ntdll.dll+0x1a2b"));
        assert_eq!(f.detail.as_deref(), Some("access: write null+0x18"));
        assert_eq!(f.frames, vec!["ntdll.dll+0x1a2b", "wakuwaku.exe+0x10"]);
        assert!(!dir.join("native-77.txt").exists());
        std::fs::write(dir.join("panic-78.json"), r#"{"time":9000,"started":4000,"message":"boom","place":"src\\pet.rs:1:2","thread":"pointer","frames":["wakuwaku.exe+0x20"]}"#).unwrap();
        let f = found_for(&dir, 78, Some(0xC000_0409), None, 9500).unwrap();
        assert_eq!((f.kind, f.uptime, f.detail.as_deref()), ("panic", Some(5), Some("thread pointer")));
        assert!(found_for(&dir, 79, Some(1), None, 1).is_none());
        let f = found_for(&dir, 80, Some(0xC000_0374), Some(1000), 61_000).unwrap();
        assert_eq!((f.kind, f.uptime), ("exit", Some(60)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn hang_records_read() {
        let dir = std::env::temp_dir().join(format!("wakuwaku-hang-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let record = "seen=50000\nthread=12\nwindow=Tauri Window\nframe=ntdll.dll+0xa1b4\nframe=win32u.dll+0x1c\nframe=wakuwaku-v0.3.8-windows-x64.exe+0x4c1d\nframe=C:\\bad+0x1\n";
        // Closed by Windows' "not responding" box.
        std::fs::write(dir.join("hang-81.txt"), record).unwrap();
        let f = found_for(&dir, 81, Some(0xCFFF_FFFF), Some(1000), 61_000).unwrap();
        assert_eq!((f.kind, f.code.as_deref(), f.uptime), ("exit", Some("0xCFFFFFFF"), Some(60)));
        assert_eq!(f.place.as_deref(), Some("wakuwaku-v0.3.8-windows-x64.exe+0x4c1d"));
        assert_eq!(f.frames, vec!["ntdll.dll+0xa1b4", "win32u.dll+0x1c", "wakuwaku-v0.3.8-windows-x64.exe+0x4c1d"]);
        assert_eq!(f.detail.as_deref(), Some("hang: her main thread not answering, 16 s and more before she ended"));
        assert!(!dir.join("hang-81.txt").exists());
        // Ended by Task Manager while not answering: a hang's report too.
        std::fs::write(dir.join("hang-82.txt"), record).unwrap();
        assert_eq!(found_for(&dir, 82, Some(1), None, 60_000).map(|f| f.kind), Some("exit"));
        // Found at her next start (how she ended not known), or ended as she
        // quits: none, the record gone.
        std::fs::write(dir.join("hang-83.txt"), record).unwrap();
        assert!(found_for(&dir, 83, None, None, 60_000).is_none());
        assert!(!dir.join("hang-83.txt").exists());
        std::fs::write(dir.join("hang-84.txt"), record).unwrap();
        assert!(found_for(&dir, 84, Some(0), None, 60_000).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn hung_threads_named() {
        assert_eq!(thread_said("Tauri Window"), "her main thread");
        assert_eq!(thread_said("WakuwakuTaskbarHost"), "her taskbar's thread");
        assert_eq!(thread_said(""), "a thread of hers");
        assert_eq!(thread_said("Odd/Class‹x›"), "a thread of hers (OddClassx)");
    }

    // A thread of this test's own process, held, its stack read as the
    // guard reads hers: Windows' frames on top, this exe's under them.
    #[cfg(all(windows, target_arch = "x86_64"))]
    #[test]
    fn a_held_threads_stack_is_read() {
        let (tell, told) = std::sync::mpsc::channel();
        let (hold, let_go) = std::sync::mpsc::channel::<()>();
        let held = std::thread::spawn(move || {
            tell.send(super::win::thread_id()).unwrap();
            let _ = let_go.recv();
        });
        let thread = told.recv().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(200));
        let frames = super::win::stack_of(std::process::id(), thread);
        drop(hold);
        held.join().unwrap();
        assert!(frames.len() >= 3, "{frames:?}");
        assert!(frames.iter().all(|f| is_frame(f)), "{frames:?}");
        assert!(frames[0].starts_with("ntdll.dll+0x"), "{frames:?}");
        let exe = std::env::current_exe().unwrap().file_name().unwrap().to_string_lossy().into_owned();
        assert!(frames.iter().any(|f| f.starts_with(&format!("{exe}+0x"))), "{frames:?}");
    }
}
