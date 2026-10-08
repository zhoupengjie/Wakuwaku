// How Claude Code reaches her: the plugin, or hooks written into its
// settings.json. Read on demand (the person may change either at any time),
// and written only when they ask, from the settings.
//
// Every event is an HTTP hook: Claude Code POSTs it to her port, no process at
// all. SessionStart also gets a command, run in the background (async), that
// starts her when she is not up, which no HTTP hook can do. That command is
// this program with ENSURE_FLAG, given as an argument list (no shell).
use std::fs;
use std::path::PathBuf;

use serde_json::{json, Map, Value};

pub const ENSURE_FLAG: &str = "--wakuwaku-ensure-running";
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

// Claude Code's settings: {} when there are none, Err when unreadable.
fn read_settings() -> Result<Value, String> {
    match fs::read_to_string(claude_settings_file()) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(e.to_string()),
    }
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
    [URL_MARK, ENSURE_FLAG, LEGACY_URL_MARK, LEGACY_ENSURE_FLAG, "claude-hook.js"].iter().any(|m| text.contains(m))
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
    let mut found: Map<String, Value> = Map::new();
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
    let file = claude_settings_file();
    let before = read_settings()?;
    let after = match action {
        "remove" => uninstall(&before),
        "install" => install(&before, port, false),
        "install-http" => install(&before, port, true),
        _ => return Err(format!("unknown action {action}")),
    };
    let backup = PathBuf::from(format!("{}.wakuwaku.bak", file.display()));
    if file.exists() && !backup.exists() {
        fs::copy(&file, &backup).map_err(|e| e.to_string())?;
    }
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(&after).map_err(|e| e.to_string())?;
    fs::write(&file, format!("{text}\n")).map_err(|e| e.to_string())
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
}
