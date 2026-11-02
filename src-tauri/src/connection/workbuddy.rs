// WorkBuddy: hooks in ~/.workbuddy/settings.json
//
// WorkBuddy (Tencent's desktop coding agent: an Electron shell around its
// bundled CodeBuddy CLI) speaks Claude Code's hook protocol, HTTP hooks
// included. What differs is where its desktop reads them: the main process
// sets CODEBUDDY_CONFIG_DIR and WORKBUDDY_CONFIG_DIR to its data folder
// (~/.workbuddy, or the edition's dataFolderName) and starts the CLI with
// --setting-sources user, so the user-level settings file is that folder's
// settings.json, not ~/.codebuddy/settings.json. Its 11 events are Claude
// Code's, so they are read as Claude Code's (agent-core workbuddy), and each
// is an HTTP hook: no command, no process. SessionStart is HTTP here too
// (unlike Claude Code, which runs none), so all 11 go in.
//
// Its desktop writes settings.json by reading the whole file and writing it
// back expanded, unknown keys (hooks among them) kept.

use super::*;

// Each event WorkBuddy reports, and whether it takes a matcher; the prompt
// waits for the person (up to 300 s) instead of the default 2, as in Claude
// Code.
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

// WorkBuddy's desktop folder: WORKBUDDY_CONFIG_DIR or CODEBUDDY_CONFIG_DIR
// when set (the desktop sets both), else ~/.workbuddy. An edition whose
// dataFolderName is not .workbuddy needs the variable set for her too.
pub fn workbuddy_dir() -> PathBuf {
    std::env::var_os("WORKBUDDY_CONFIG_DIR")
        .or_else(|| std::env::var_os("CODEBUDDY_CONFIG_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).unwrap_or_default()).join(".workbuddy"))
}

pub fn workbuddy_settings_file() -> PathBuf {
    workbuddy_dir().join("settings.json")
}

// Whether WorkBuddy is on this computer: its folder (made as it first runs).
pub fn workbuddy_here() -> bool {
    workbuddy_dir().is_dir()
}

fn hook_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/hook?from=wakuwaku&agent=workbuddy")
}

// What we add, event by event. Ours already there (from this or an older
// copy, or one written before WorkBuddy had a name of its own) are stripped
// first, so installing twice replaces rather than doubles.
fn entries(port: u16) -> Map<String, Value> {
    let url = hook_url(port);
    let mut out = Map::new();
    for (event, matcher, timeout) in EVENTS {
        let hooks = json!([{ "type": "http", "url": url, "timeout": timeout }]);
        let group = if matcher { json!({ "matcher": "*", "hooks": hooks }) } else { json!({ "hooks": hooks }) };
        out.insert(event.into(), json!([group]));
    }
    out
}

pub fn workbuddy_install(settings: &Value, port: u16) -> Value {
    let mut hooks = strip(settings.get("hooks"));
    for (event, groups) in entries(port) {
        let mut all = hooks.get(&event).and_then(Value::as_array).cloned().unwrap_or_default();
        all.extend(groups.as_array().cloned().unwrap_or_default());
        hooks.insert(event, Value::Array(all));
    }
    let mut out = settings.as_object().cloned().unwrap_or_default();
    out.insert("hooks".into(), Value::Object(hooks));
    Value::Object(out)
}

pub fn workbuddy_uninstall(settings: &Value) -> Value {
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
//   missing / ok / stale (another port or copy, a command hook, or one
//   written before WorkBuddy had a name of its own) / partial (some events
//   lack them).
pub fn workbuddy_status_of(settings: &Value, port: u16) -> &'static str {
    let found = ours_by_event(settings);
    if !EVENTS.iter().any(|(name, ..)| found.contains_key(*name)) {
        return "missing";
    }
    let list = |name: &str| found.get(name).and_then(Value::as_array).cloned().unwrap_or_default();
    let want_url = hook_url(port);
    let is_old = EVENTS.iter().any(|(name, ..)| list(name).iter().any(|h| h["type"] != "http"));
    let is_other = EVENTS.iter().any(|(name, ..)| list(name).iter().any(|h| h["type"] == "http" && h["url"] != want_url.as_str()));
    if is_old || is_other {
        return "stale";
    }
    if EVENTS.iter().any(|(name, ..)| !found.contains_key(*name)) {
        return "partial";
    }
    "ok"
}

// absent (no WorkBuddy here), unreadable, or how our hooks stand.
pub fn workbuddy_hooks_status(port: u16) -> &'static str {
    if !workbuddy_here() {
        return "absent";
    }
    match read_json(&workbuddy_settings_file()) {
        Ok(settings) => workbuddy_status_of(&settings, port),
        Err(_) => "unreadable",
    }
}

// Change WorkBuddy's settings.json, backed up once beside itself.
// action: install, remove.
pub fn write_workbuddy_hooks(action: &str, port: u16) -> Result<(), String> {
    // Read from the disk, not as last read: nothing of someone else's is written back.
    let before = read_json_now(&workbuddy_settings_file())?;
    let after = match action {
        "install" => workbuddy_install(&before, port),
        "remove" => workbuddy_uninstall(&before),
        _ => return Err(format!("unknown action {action}")),
    };
    write_json(&workbuddy_settings_file(), &after)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_status_and_uninstall() {
        let exe_port = 47213;
        // As the person may have it: a model, their own hook, a field we do not know.
        let theirs = json!({
            "model": "auto",
            "hooks": { "Stop": [{ "hooks": [{ "type": "command", "command": "notify-me" }] }] },
            "future": { "x": 1 }
        });
        assert_eq!(workbuddy_status_of(&theirs, exe_port), "missing");
        let with = workbuddy_install(&theirs, exe_port);
        assert_eq!(workbuddy_status_of(&with, exe_port), "ok");
        assert_eq!(workbuddy_status_of(&with, 47299), "stale");
        // Theirs first, then ours; every event HTTP, SessionStart too.
        assert_eq!(with["hooks"]["Stop"][0], theirs["hooks"]["Stop"][0]);
        assert_eq!(
            with["hooks"]["Stop"][1],
            json!({ "hooks": [{ "type": "http", "url": "http://127.0.0.1:47213/hook?from=wakuwaku&agent=workbuddy", "timeout": 2 }] })
        );
        assert_eq!(with["hooks"]["SessionStart"][0]["hooks"][0]["type"], "http");
        assert_eq!(with["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"], 300);
        assert_eq!(with["hooks"].as_object().unwrap().len(), 11);
        assert_eq!((&with["model"], &with["future"]), (&theirs["model"], &theirs["future"]));
        // Installing twice replaces, never doubles.
        let again = workbuddy_install(&with, exe_port);
        assert_eq!(again["hooks"]["Stop"].as_array().unwrap().len(), 2);
        // Removed: theirs as it was.
        assert_eq!(workbuddy_uninstall(&with), theirs);
        assert_eq!(workbuddy_uninstall(&workbuddy_install(&json!({}), exe_port)), json!({}));

        // Some events missing is partial.
        let mut part = with.clone();
        part["hooks"].as_object_mut().unwrap().remove("Elicitation");
        assert_eq!(workbuddy_status_of(&part, exe_port), "partial");
        // A hook of ours from before WorkBuddy had a name of its own: stale,
        // and replaced (not kept beside) on the next install.
        let old = json!({ "hooks": { "Stop": [{ "hooks": [{ "type": "http", "url": "http://127.0.0.1:47213/hook?from=wakuwaku", "timeout": 2 }] }] } });
        assert_eq!(workbuddy_status_of(&old, exe_port), "stale");
        let migrated = workbuddy_install(&old, exe_port);
        assert_eq!(migrated["hooks"]["Stop"].as_array().unwrap().len(), 1);
        assert_eq!(workbuddy_status_of(&migrated, exe_port), "ok");
        // A command hook of ours (nothing WorkBuddy needs) is stale too.
        let command = json!({ "hooks": { "Stop": [{ "hooks": [{ "type": "command", "command": "wakuwaku.exe", "args": ["--wakuwaku-ensure-running"] }] }] } });
        assert_eq!(workbuddy_status_of(&command, exe_port), "stale");
    }
}
