// How Claude Code and Codex reach her: the plugin, or hooks written into
// Claude Code's settings.json; hooks written into Codex's hooks.json. Read on
// demand (the person may change any of them at any time), and written only
// when they ask, from the settings.
//
// Every Claude Code event is an HTTP hook: Claude Code POSTs it to her port,
// no process at all. SessionStart also gets a command, run in the background
// (async), that starts her when she is not up, which no HTTP hook can do.
// That command is this program with ENSURE_FLAG, given as an argument list
// (no shell). Codex runs only commands: see "Codex" below.
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};

pub const ENSURE_FLAG: &str = "--wakuwaku-ensure-running";
pub const CODEX_FLAG: &str = "--wakuwaku-codex-hook";
const URL_MARK: &str = "from=wakuwaku";
// What the app wrote before it was called Wakuwaku (claude-pets).
pub const LEGACY_ENSURE_FLAG: &str = "--claude-pets-ensure-running";
const LEGACY_URL_MARK: &str = "from=claude-pets";
pub const PLUGIN_ID: &str = "wakuwaku@wakuwaku";
pub const PLUGIN_COMMANDS: [&str; 2] = ["/plugin marketplace add zhoupengjie/wakuwaku", "/plugin install wakuwaku@wakuwaku"];

// Each event Claude Code reports, and whether it takes a matcher; the prompt
// waits for the person (up to 300 s) instead of the default 2.
const EVENTS: [(&str, bool, u64); 11] = [
    ("SessionStart", false, 2),
    ("SessionEnd", false, 2),
    ("UserPromptSubmit", false, 2),
    ("PreToolUse", true, 2),
    ("PostToolUse", true, 2),
    ("PostToolUseFailure", true, 2),
    ("PermissionRequest", true, 300),
    ("Elicitation", false, 2),
    ("TaskCompleted", false, 2),
    ("Stop", false, 2),
    ("StopFailure", false, 2),
];

pub fn claude_settings_file() -> PathBuf {
    let dir = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).unwrap_or_default()).join(".claude"));
    dir.join("settings.json")
}

// A JSON settings file: {} when there is none, Err when unreadable.
fn read_json(file: &Path) -> Result<Value, String> {
    match fs::read_to_string(file) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(e.to_string()),
    }
}

// Claude Code's settings.
fn read_settings() -> Result<Value, String> {
    read_json(&claude_settings_file())
}

// Write a settings file, backed up once beside itself.
fn write_json(file: &Path, value: &Value) -> Result<(), String> {
    let backup = PathBuf::from(format!("{}.wakuwaku.bak", file.display()));
    if file.exists() && !backup.exists() {
        fs::copy(file, &backup).map_err(|e| e.to_string())?;
    }
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    fs::write(file, format!("{text}\n")).map_err(|e| e.to_string())
}

// This program, as the hook command starts it.
pub fn launch_command() -> String {
    std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
}

fn hook_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/hook?{URL_MARK}")
}

// A path as Windows compares them: one slash, any case.
fn same_path(p: &str) -> String {
    if cfg!(windows) { p.replace('\\', "/").to_lowercase() } else { p.to_string() }
}

// Ours: what this version installs, and what older ones did.
fn is_ours(hook: &Value) -> bool {
    let mut text = [hook.get("url"), hook.get("command")].iter().flatten().map(|v| v.to_string()).collect::<Vec<_>>().join(" ");
    for arg in hook.get("args").and_then(Value::as_array).into_iter().flatten() {
        text.push(' ');
        text.push_str(&arg.to_string());
    }
    [URL_MARK, ENSURE_FLAG, CODEX_FLAG, LEGACY_URL_MARK, LEGACY_ENSURE_FLAG, "claude-hook.js"].iter().any(|m| text.contains(m))
}

// Our hooks, event by event.
fn ours_by_event(settings: &Value) -> Map<String, Value> {
    let mut found = Map::new();
    for (event, groups) in settings.get("hooks").and_then(Value::as_object).into_iter().flatten() {
        let ours: Vec<Value> = groups
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|g| g.get("hooks").and_then(Value::as_array).cloned().unwrap_or_default())
            .filter(is_ours)
            .collect();
        if !ours.is_empty() {
            found.insert(event.clone(), Value::Array(ours));
        }
    }
    found
}

// Our entries dropped from every event, and events left empty.
fn strip(hooks: Option<&Value>) -> Map<String, Value> {
    let mut out = Map::new();
    let Some(Value::Object(hooks)) = hooks else { return out };
    for (event, groups) in hooks {
        let Value::Array(groups) = groups else {
            out.insert(event.clone(), groups.clone());
            continue;
        };
        let kept: Vec<Value> = groups
            .iter()
            .filter_map(|group| {
                let mut group = group.clone();
                let hooks: Vec<Value> = group.get("hooks").and_then(Value::as_array).into_iter().flatten().filter(|h| !is_ours(h)).cloned().collect();
                if hooks.is_empty() {
                    return None;
                }
                group["hooks"] = Value::Array(hooks);
                Some(group)
            })
            .collect();
        if !kept.is_empty() {
            out.insert(event.clone(), Value::Array(kept));
        }
    }
    out
}

// What we add, event by event. http_only: no command, nothing starts her.
fn entries(port: u16, http_only: bool) -> Map<String, Value> {
    let mut out = Map::new();
    for (event, matcher, timeout) in EVENTS {
        // Claude Code runs no HTTP hook for SessionStart: the starter command
        // passes that one on.
        if event == "SessionStart" {
            continue;
        }
        let hooks = json!([{ "type": "http", "url": hook_url(port), "timeout": timeout }]);
        let group = if matcher { json!({ "matcher": "*", "hooks": hooks }) } else { json!({ "hooks": hooks }) };
        out.insert(event.into(), json!([group]));
    }
    if !http_only {
        out.insert(
            "SessionStart".into(),
            json!([{ "hooks": [{ "type": "command", "command": launch_command(), "args": [ENSURE_FLAG], "async": true, "timeout": 15 }] }]),
        );
    }
    out
}

pub fn install(settings: &Value, port: u16, http_only: bool) -> Value {
    let mut hooks = strip(settings.get("hooks"));
    for (event, groups) in entries(port, http_only) {
        let mut all = hooks.get(&event).and_then(Value::as_array).cloned().unwrap_or_default();
        all.extend(groups.as_array().cloned().unwrap_or_default());
        hooks.insert(event, Value::Array(all));
    }
    let mut out = settings.as_object().cloned().unwrap_or_default();
    out.insert("hooks".into(), Value::Object(hooks));
    Value::Object(out)
}

pub fn uninstall(settings: &Value) -> Value {
    let mut out = settings.as_object().cloned().unwrap_or_default();
    let hooks = strip(settings.get("hooks"));
    if hooks.is_empty() {
        out.remove("hooks");
    } else {
        out.insert("hooks".into(), Value::Object(hooks));
    }
    Value::Object(out)
}

// How our entries stand against what this copy would install:
//   missing / ok / httpOnly (nothing starts her) / stale (another port or
//   copy, or an old version) / partial (some events lack them).
pub fn status_of(settings: &Value, port: u16) -> &'static str {
    let found = ours_by_event(settings);
    if !EVENTS.iter().any(|(name, ..)| found.contains_key(*name)) {
        return "missing";
    }
    let list = |name: &str| found.get(name).and_then(Value::as_array).cloned().unwrap_or_default();
    let has_flag = |h: &Value| h.get("args").and_then(Value::as_array).is_some_and(|a| a.iter().any(|x| x == ENSURE_FLAG));
    let is_old = EVENTS.iter().any(|(name, ..)| list(name).iter().any(|h| h["type"] != "http" && !has_flag(h)))
        || list("SessionStart").iter().any(|h| h["type"] == "http");
    let want_url = hook_url(port);
    let is_other_port = EVENTS.iter().any(|(name, ..)| list(name).iter().any(|h| h["type"] == "http" && h["url"] != want_url.as_str()));
    let starters: Vec<Value> = list("SessionStart").into_iter().filter(|h| h["type"] == "command").collect();
    let want = vec![same_path(&launch_command()), same_path(ENSURE_FLAG)];
    let is_other_copy = starters.iter().any(|h| {
        let mut got = vec![same_path(h["command"].as_str().unwrap_or(""))];
        got.extend(h.get("args").and_then(Value::as_array).into_iter().flatten().map(|a| same_path(a.as_str().unwrap_or(""))));
        got != want
    });
    if is_old || is_other_port || is_other_copy {
        return "stale";
    }
    if EVENTS.iter().any(|(name, ..)| *name != "SessionStart" && !found.contains_key(*name)) {
        return "partial";
    }
    if starters.is_empty() { "httpOnly" } else { "ok" }
}

pub fn hooks_status(port: u16) -> &'static str {
    match read_settings() {
        Ok(settings) => status_of(&settings, port),
        Err(_) => "unreadable",
    }
}

pub fn is_plugin_enabled() -> bool {
    read_settings().is_ok_and(|s| s.get("enabledPlugins").and_then(|p| p.get(PLUGIN_ID)) == Some(&Value::Bool(true)))
}

// 'plugin', 'hooks' (written into settings.json), 'both' (every event twice), or 'none'.
pub fn connection(port: u16) -> &'static str {
    let plugin = is_plugin_enabled();
    let hooks = !matches!(hooks_status(port), "missing" | "unreadable");
    match (plugin, hooks) {
        (true, true) => "both",
        (true, false) => "plugin",
        (false, true) => "hooks",
        (false, false) => "none",
    }
}

// Change Claude Code's settings.json, backed up once beside itself.
// action: install, install-http, remove.
pub fn write_hooks(action: &str, port: u16) -> Result<(), String> {
    let before = read_settings()?;
    let after = match action {
        "remove" => uninstall(&before),
        "install" => install(&before, port, false),
        "install-http" => install(&before, port, true),
        _ => return Err(format!("unknown action {action}")),
    };
    write_json(&claude_settings_file(), &after)
}


// --- Codex: hooks in ~/.codex/hooks.json ---------------------------------------------
//
// Codex runs hooks only as commands, through the session's shell (on Windows
// `pwsh -NoProfile -Command`, about 0.3 s to start): this program with
// CODEX_FLAG, which hands the event to her port and prints her answer to a
// prompt (main.rs). So few hooks make Codex wait: the turn starting and
// ending, a prompt, an edit (apply_patch) or a question, the session. Each
// tool call's step comes from hooks run in the background (async), which
// Codex has from 0.148 on; older ones skip async hooks with a warning, so
// those are left out for them.
//
// Codex runs a hook only once the person trusts it (/hooks in Codex), and
// again after it changes, such as this program moving: we never trust them
// on their behalf.

pub const CODEX_PROMPT_TIMEOUT: u64 = 300;
// A version: major, minor, patch.
pub type Version = (u64, u64, u64);
// The first Codex that runs async hooks.
const CODEX_ASYNC_SINCE: Version = (0, 148, 0);

// One hook we add: its event, the tools it is for (a regex, None for all),
// how long it may run (s), and whether in the background.
struct CodexHook {
    event: &'static str,
    matcher: Option<&'static str>,
    timeout: u64,
    is_async: bool,
}

// A prompt waits for the person; Codex gives Interrupt and SessionEnd 3 s at most.
const CODEX_HOOKS: [CodexHook; 10] = [
    CodexHook { event: "SessionStart", matcher: None, timeout: 10, is_async: false },
    CodexHook { event: "UserPromptSubmit", matcher: None, timeout: 10, is_async: false },
    CodexHook { event: "PreToolUse", matcher: Some("^request_user_input$"), timeout: 10, is_async: false },
    CodexHook { event: "PostToolUse", matcher: Some("^(apply_patch|request_user_input)$"), timeout: 10, is_async: false },
    CodexHook { event: "PermissionRequest", matcher: None, timeout: CODEX_PROMPT_TIMEOUT, is_async: false },
    CodexHook { event: "Stop", matcher: None, timeout: 10, is_async: false },
    CodexHook { event: "Interrupt", matcher: None, timeout: 3, is_async: false },
    CodexHook { event: "SessionEnd", matcher: None, timeout: 3, is_async: false },
    // Every step, in the background.
    CodexHook { event: "PreToolUse", matcher: None, timeout: 10, is_async: true },
    CodexHook { event: "PostToolUse", matcher: None, timeout: 10, is_async: true },
];

fn codex_hooks_for(with_async: bool) -> impl Iterator<Item = &'static CodexHook> {
    CODEX_HOOKS.iter().filter(move |h| with_async || !h.is_async)
}

pub fn codex_home() -> PathBuf {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).unwrap_or_default()).join(".codex"))
}

pub fn codex_hooks_file() -> PathBuf {
    codex_home().join("hooks.json")
}

// "codex-cli 0.146.1" (or 0.149.0-alpha.1) → (0, 146, 1).
fn parse_version(text: &str) -> Option<Version> {
    text.split_whitespace().find_map(|word| {
        let mut parts = word.split('.').map(|p| p.chars().take_while(char::is_ascii_digit).collect::<String>().parse::<u64>().ok());
        Some((parts.next()??, parts.next()??, parts.next()??))
    })
}

fn ask_codex_version() -> Option<Version> {
    let mut command = std::process::Command::new("codex");
    command.arg("--version").stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW
        command.creation_flags(0x0800_0000);
    }
    parse_version(&String::from_utf8_lossy(&command.output().ok()?.stdout))
}

// Codex's version, None when there is no `codex` to ask (the desktop app
// alone). Asked at most once a minute.
pub fn codex_version() -> Option<Version> {
    static KNOWN: Mutex<Option<(Instant, Option<Version>)>> = Mutex::new(None);
    let mut known = KNOWN.lock().unwrap();
    match *known {
        Some((at, version)) if at.elapsed() < Duration::from_secs(60) => version,
        _ => {
            let version = ask_codex_version();
            *known = Some((Instant::now(), version));
            version
        }
    }
}

// Whether Codex runs async hooks; one we cannot ask is taken as a current one.
pub fn codex_runs_async() -> bool {
    codex_version().is_none_or(|v| v >= CODEX_ASYNC_SINCE)
}

// The command line Codex's shell runs: the bare path when it needs no
// quoting (any shell takes it), else quoted for PowerShell (& '...') on
// Windows or for a POSIX shell.
pub fn codex_command(exe: &str) -> String {
    let is_plain = !exe.is_empty() && exe.chars().all(|c| c.is_ascii_alphanumeric() || "\\/:._-".contains(c));
    if is_plain {
        return format!("{exe} {CODEX_FLAG}");
    }
    if cfg!(windows) {
        format!("& '{}' {CODEX_FLAG}", exe.replace('\'', "''"))
    } else {
        format!("'{}' {CODEX_FLAG}", exe.replace('\'', r"'\''"))
    }
}

// What we add, event by event; `waiting` is what Codex shows while a prompt waits on her.
fn codex_entries(command: &str, waiting: &str, with_async: bool) -> Map<String, Value> {
    let mut out = Map::new();
    for want in codex_hooks_for(with_async) {
        let mut hook = json!({ "type": "command", "command": command, "timeout": want.timeout });
        if want.is_async {
            hook["async"] = json!(true);
        }
        if want.event == "PermissionRequest" {
            hook["statusMessage"] = json!(waiting);
        }
        let mut group = json!({ "hooks": [hook] });
        if let Some(matcher) = want.matcher {
            group["matcher"] = json!(matcher);
        }
        let groups = out.entry(want.event).or_insert_with(|| json!([]));
        if let Value::Array(groups) = groups {
            groups.push(group);
        }
    }
    out
}

pub fn codex_install(file: &Value, command: &str, waiting: &str, with_async: bool) -> Value {
    let mut hooks = strip(file.get("hooks"));
    for (event, groups) in codex_entries(command, waiting, with_async) {
        let mut all = hooks.get(&event).and_then(Value::as_array).cloned().unwrap_or_default();
        all.extend(groups.as_array().cloned().unwrap_or_default());
        hooks.insert(event, Value::Array(all));
    }
    let mut out = file.as_object().cloned().unwrap_or_default();
    out.insert("hooks".into(), Value::Object(hooks));
    Value::Object(out)
}

// Our hooks in a hooks.json: (event, matcher, hook).
fn codex_ours(file: &Value) -> Vec<(String, Option<String>, Value)> {
    let mut ours = Vec::new();
    for (event, groups) in file.get("hooks").and_then(Value::as_object).into_iter().flatten() {
        for group in groups.as_array().into_iter().flatten() {
            let matcher = group.get("matcher").and_then(Value::as_str).map(str::to_string);
            for hook in group.get("hooks").and_then(Value::as_array).into_iter().flatten().filter(|h| is_ours(h)) {
                ours.push((event.clone(), matcher.clone(), hook.clone()));
            }
        }
    }
    ours
}

fn is_codex_hook(found: &(String, Option<String>, Value), want: &CodexHook, command: &str) -> bool {
    let (event, matcher, hook) = found;
    event == want.event
        && matcher.as_deref() == want.matcher
        && hook["type"] == "command"
        && hook["command"] == command
        && hook["timeout"].as_u64() == Some(want.timeout)
        && hook.get("async").and_then(Value::as_bool).unwrap_or(false) == want.is_async
}

// How our entries stand against what this copy would install: missing / ok /
// stale (another copy of the program, or another version's hooks) /
// upgradable (only the background ones lack: Codex was updated since) /
// partial (others lack).
pub fn codex_status_of(file: &Value, command: &str, with_async: bool) -> &'static str {
    let ours = codex_ours(file);
    if ours.is_empty() {
        return "missing";
    }
    if !ours.iter().all(|found| CODEX_HOOKS.iter().any(|want| is_codex_hook(found, want, command))) {
        return "stale";
    }
    let lacking: Vec<&CodexHook> = codex_hooks_for(with_async).filter(|want| !ours.iter().any(|found| is_codex_hook(found, want, command))).collect();
    if lacking.is_empty() {
        "ok"
    } else if lacking.iter().all(|want| want.is_async) {
        "upgradable"
    } else {
        "partial"
    }
}

// absent (no Codex here), unreadable, or how our entries stand.
pub fn codex_hooks_status() -> &'static str {
    if !codex_home().is_dir() {
        return "absent";
    }
    match read_json(&codex_hooks_file()) {
        Ok(file) => codex_status_of(&file, &codex_command(&launch_command()), codex_runs_async()),
        Err(_) => "unreadable",
    }
}

pub fn is_codex_connected() -> bool {
    matches!(codex_hooks_status(), "ok" | "stale" | "upgradable" | "partial")
}

// Change Codex's hooks.json, backed up once beside itself. action: install, remove.
pub fn write_codex_hooks(action: &str, waiting: &str) -> Result<(), String> {
    let file = codex_hooks_file();
    let before = read_json(&file)?;
    let after = match action {
        "remove" if !file.exists() => return Ok(()),
        "remove" => uninstall(&before),
        "install" => codex_install(&before, &codex_command(&launch_command()), waiting, codex_runs_async()),
        _ => return Err(format!("unknown action {action}")),
    };
    write_json(&file, &after)
}


// --- Start at login: this program, from where it is now -------------------------------

#[cfg(windows)]
mod login {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const NAME: &str = "Wakuwaku";

    fn command() -> String {
        format!("\"{}\"", super::launch_command())
    }

    pub fn is_on() -> bool {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(RUN, KEY_READ)
            .and_then(|k| k.get_value::<String, _>(NAME))
            .is_ok_and(|v| v.eq_ignore_ascii_case(&command()))
    }

    pub fn set(on: bool) -> Result<(), String> {
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey_with_flags(RUN, KEY_READ | KEY_WRITE).map_err(|e| e.to_string())?;
        if on {
            key.set_value(NAME, &command()).map_err(|e| e.to_string())
        } else {
            match key.delete_value(NAME) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
                _ => Ok(()),
            }
        }
    }
}

#[cfg(not(windows))]
mod login {
    pub fn is_on() -> bool {
        false
    }

    pub fn set(_on: bool) -> Result<(), String> {
        Err("not on this system".into())
    }
}

pub use login::{is_on as is_open_at_login, set as set_open_at_login};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_status_and_uninstall() {
        let mine = json!({ "hooks": { "Stop": [{ "hooks": [{ "type": "command", "command": "echo hi" }] }] }, "model": "x" });
        assert_eq!(status_of(&mine, 47213), "missing");

        let with = install(&mine, 47213, false);
        assert_eq!(status_of(&with, 47213), "ok");
        assert_eq!(status_of(&with, 47299), "stale");
        assert_eq!(with["hooks"]["Stop"].as_array().unwrap().len(), 2);
        assert_eq!(with["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"], 300);
        assert_eq!(with["hooks"]["SessionStart"][0]["hooks"][0]["args"], json!([ENSURE_FLAG]));
        assert_eq!(status_of(&install(&mine, 47213, true), 47213), "httpOnly");

        // Installing twice replaces, never doubles.
        let again = install(&with, 47213, false);
        assert_eq!(again["hooks"]["Stop"].as_array().unwrap().len(), 2);

        let without = uninstall(&with);
        assert_eq!(without, mine);
        assert_eq!(uninstall(&install(&json!({}), 47213, false)), json!({}));
    }

    #[test]
    fn some_events_missing_is_partial() {
        let mut with = install(&json!({}), 47213, false);
        with["hooks"].as_object_mut().unwrap().remove("Elicitation");
        assert_eq!(status_of(&with, 47213), "partial");
    }

    #[test]
    fn codex_hooks_install_status_and_uninstall() {
        let cmd = codex_command(r"D:\pet\wakuwaku.exe");
        assert_eq!(cmd, r"D:\pet\wakuwaku.exe --wakuwaku-codex-hook");
        let mine = json!({ "hooks": { "Stop": [{ "hooks": [{ "type": "command", "command": "notify-me" }] }] }, "other": 1 });
        assert_eq!(codex_status_of(&mine, &cmd, true), "missing");

        let with = codex_install(&mine, &cmd, "waiting", true);
        assert_eq!(codex_status_of(&with, &cmd, true), "ok");
        assert_eq!(with["hooks"]["Stop"].as_array().unwrap().len(), 2);
        let ask = &with["hooks"]["PermissionRequest"][0]["hooks"][0];
        assert_eq!((ask["timeout"].as_u64(), ask.get("async"), ask["statusMessage"].as_str()), (Some(300), None, Some("waiting")));
        // A question waits in the foreground; every step in the background.
        let pre = with["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!((pre[0]["matcher"].as_str(), pre[0]["hooks"][0].get("async")), (Some("^request_user_input$"), None));
        assert_eq!((pre[1].get("matcher"), &pre[1]["hooks"][0]["async"]), (None, &json!(true)));
        assert_eq!(with["hooks"]["PostToolUse"][0]["matcher"], "^(apply_patch|request_user_input)$");
        // Another copy of the program: a repair.
        assert_eq!(codex_status_of(&with, &codex_command(r"E:\elsewhere\wakuwaku.exe"), true), "stale");

        let again = codex_install(&with, &cmd, "waiting", true);
        assert_eq!(again["hooks"]["Stop"].as_array().unwrap().len(), 2);
        assert_eq!(again["hooks"]["PreToolUse"].as_array().unwrap().len(), 2);
        let mut part = again.clone();
        part["hooks"].as_object_mut().unwrap().remove("Interrupt");
        assert_eq!(codex_status_of(&part, &cmd, true), "partial");

        assert_eq!(uninstall(&with), mine);
        assert_eq!(uninstall(&codex_install(&json!({}), &cmd, "w", true)), json!({}));
    }

    #[test]
    fn an_older_codex_gets_no_background_hooks() {
        let cmd = codex_command(r"D:\pet\wakuwaku.exe");
        let old = codex_install(&json!({}), &cmd, "w", false);
        let all: Vec<&Value> = old["hooks"].as_object().unwrap().values().flat_map(|g| g.as_array().unwrap()).collect();
        assert_eq!(all.len(), 8);
        assert!(all.iter().all(|g| g["hooks"][0].get("async").is_none()));
        assert_eq!(codex_status_of(&old, &cmd, false), "ok");
        // Codex updated since: only the background ones lack.
        assert_eq!(codex_status_of(&old, &cmd, true), "upgradable");
        let mut part = old.clone();
        part["hooks"].as_object_mut().unwrap().remove("Stop");
        assert_eq!(codex_status_of(&part, &cmd, true), "partial");
        // Installed for a newer Codex, read for an older one: still ours, nothing stale.
        assert_eq!(codex_status_of(&codex_install(&json!({}), &cmd, "w", true), &cmd, false), "ok");
    }

    #[test]
    fn codex_versions() {
        assert_eq!(parse_version("codex-cli 0.146.1\n"), Some((0, 146, 1)));
        assert_eq!(parse_version("codex-cli 0.149.0-alpha.1"), Some((0, 149, 0)));
        assert_eq!(parse_version("nothing here"), None);
        assert!((0, 146, 1) < CODEX_ASYNC_SINCE && (0, 148, 0) >= CODEX_ASYNC_SINCE && (1, 0, 0) >= CODEX_ASYNC_SINCE);
    }

    #[test]
    fn codex_commands_quote_what_needs_it() {
        if cfg!(windows) {
            assert_eq!(codex_command(r"C:\Program Files\Waku\wakuwaku.exe"), r"& 'C:\Program Files\Waku\wakuwaku.exe' --wakuwaku-codex-hook");
            assert_eq!(codex_command(r"D:\宠物\it's.exe"), r"& 'D:\宠物\it''s.exe' --wakuwaku-codex-hook");
        } else {
            assert_eq!(codex_command("/opt/my pets/wakuwaku"), "'/opt/my pets/wakuwaku' --wakuwaku-codex-hook");
        }
    }
}
