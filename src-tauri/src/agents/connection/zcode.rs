// ZCode: hooks in ~/.zcode/cli/config.json
//
// ZCode (Z.ai's agent: its desktop app, whose engine is its CLI bundled,
// and the CLI) runs the hooks in the user's config.json, under hooks.events,
// once hooks.enabled is true: no trust asked for (a workspace's own
// .zcode/config.json is, again after each change, so ours go in the user's).
// Its 7 events, each with Claude Code's fields beside its own camelCase ones
// (events/others.rs zcode). A hook is a command for a shell or, ours, a program and
// its arguments (process: no shell, no quoting); none over HTTP. So this
// program with ZCODE_FLAG and the event's name hands each to her port
// (hooks.rs zcode_hook), and prints her answer to a PermissionRequest in
// Claude Code's words. ZCode shows its own dialog at the same time and
// takes the first answer: hers, or the person's there (which ends our hook).
// The others answer at once; ZCode waits on each (no background for a
// process hook), some 0.1 s. A hook may be turned off one by one in ZCode's
// settings (enabled: false), which is off here too.
//
// Tried with the desktop app 3.15.3 (2026-10-15): it reads the file for each
// new session, no restart needed.

use super::*;

pub const ZCODE_FLAG: &str = "--wakuwaku-zcode-hook";
// ZCode's 7 events, and how long each may run (ms): the prompt waits for the
// person, past her own wait (agent-core asks.rs ASK_TIMEOUT_MS); ZCode's default is 60 s.
pub const ZCODE_PROMPT_TIMEOUT_MS: u64 = 600_000;
const ZCODE_TIMEOUT_MS: u64 = 10_000;
const ZCODE_EVENTS: [&str; 7] = ["SessionStart", "UserPromptSubmit", "PreToolUse", "PermissionRequest", "PostToolUse", "PostToolUseFailure", "Stop"];

fn zcode_timeout(event: &str) -> u64 {
    if event == "PermissionRequest" { ZCODE_PROMPT_TIMEOUT_MS } else { ZCODE_TIMEOUT_MS }
}

pub fn zcode_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).unwrap_or_default()).join(".zcode")
}

pub fn zcode_config_file() -> PathBuf {
    zcode_dir().join("cli").join("config.json")
}

// Whether ZCode is on this computer: its folder (made as it first runs).
pub fn zcode_here() -> bool {
    zcode_dir().is_dir()
}

fn zcode_hook(exe: &str, event: &str) -> Value {
    json!({ "type": "process", "command": exe, "args": [ZCODE_FLAG, event], "timeoutMs": zcode_timeout(event) })
}

pub fn zcode_install(file: &Value, exe: &str) -> Value {
    let root = file.get("hooks").cloned().unwrap_or_else(|| json!({}));
    let mut events = strip(root.get("events"));
    for event in ZCODE_EVENTS {
        let mut all = events.get(event).and_then(Value::as_array).cloned().unwrap_or_default();
        all.push(json!({ "matcher": "*", "hooks": [zcode_hook(exe, event)] }));
        events.insert(event.into(), Value::Array(all));
    }
    let mut root = root.as_object().cloned().unwrap_or_default();
    root.insert("enabled".into(), json!(true));
    root.insert("events".into(), Value::Object(events));
    let mut out = file.as_object().cloned().unwrap_or_default();
    out.insert("hooks".into(), Value::Object(root));
    Value::Object(out)
}

// Ours taken out, and hooks.enabled as it was before we turned it on
// (enabled_before: what it was, null for none; None when we did not, or
// cannot tell: then it stays, unless no hook is left to run). An events
// left empty goes, and the hooks with it when nothing else is there.
pub fn zcode_uninstall(file: &Value, enabled_before: Option<&Value>) -> Value {
    let mut out = file.as_object().cloned().unwrap_or_default();
    let Some(Value::Object(root)) = file.get("hooks") else { return Value::Object(out) };
    let mut root = root.clone();
    let events = strip(root.get("events"));
    match enabled_before {
        Some(Value::Null) => drop(root.remove("enabled")),
        Some(was) => drop(root.insert("enabled".into(), was.clone())),
        None if events.is_empty() => drop(root.remove("enabled")),
        None => {}
    }
    if events.is_empty() {
        root.remove("events");
    } else {
        root.insert("events".into(), Value::Object(events));
    }
    if root.is_empty() {
        out.remove("hooks");
    } else {
        out.insert("hooks".into(), Value::Object(root));
    }
    Value::Object(out)
}

// Where she keeps what hooks.enabled was before she turned it on, for
// putting it back as she is removed: in her data folder (ZCode's file takes
// no field it does not know).
fn zcode_note() -> PathBuf {
    crate::data::folder().join("zcode-hooks.json")
}

// How our hooks stand against what this copy would write: missing / ok /
// stale (another copy of the program, or another version's) / partial (some
// events lack them) / off (hooks turned off, all of them or one of ours).
pub fn zcode_status_of(file: &Value, exe: &str) -> &'static str {
    let root = file.get("hooks").unwrap_or(&Value::Null);
    let mut ours: Vec<(String, Value)> = Vec::new();
    for (event, groups) in root.get("events").and_then(Value::as_object).into_iter().flatten() {
        for group in groups.as_array().into_iter().flatten() {
            for hook in group.get("hooks").and_then(Value::as_array).into_iter().flatten().filter(|h| is_ours(h)) {
                ours.push((event.clone(), hook.clone()));
            }
        }
    }
    if ours.is_empty() {
        return "missing";
    }
    if root.get("enabled").and_then(Value::as_bool) != Some(true) || ours.iter().any(|(_, h)| h.get("enabled").and_then(Value::as_bool) == Some(false)) {
        return "off";
    }
    let is_want = |event: &str, hook: &Value| {
        let mut hook = hook.clone();
        if let Some(h) = hook.as_object_mut() {
            h.remove("enabled");
            if let Some(command) = h.get("command").and_then(Value::as_str).map(same_path) {
                h.insert("command".into(), json!(command));
            }
        }
        hook == zcode_hook(&same_path(exe), event)
    };
    if !ours.iter().all(|(event, hook)| is_want(event, hook)) {
        return "stale";
    }
    if ZCODE_EVENTS.iter().all(|want| ours.iter().any(|(event, _)| event == want)) {
        "ok"
    } else {
        "partial"
    }
}

// absent (no ZCode here), unreadable, or how our hooks stand.
pub fn zcode_hooks_status() -> &'static str {
    if !zcode_here() {
        return "absent";
    }
    match read_json(&zcode_config_file()) {
        Ok(file) => zcode_status_of(&file, &launch_command()),
        Err(_) => "unreadable",
    }
}

// Change ZCode's config.json, backed up once beside itself. action: install, remove.
pub fn write_zcode_hooks(action: &str) -> Result<(), String> {
    let file = zcode_config_file();
    // Read from the disk, not as last read: nothing of someone else's is written back.
    let before = read_json_now(&file)?;
    let note = zcode_note();
    match action {
        "remove" if !file.exists() => Ok(()),
        "remove" => {
            let was = read_json_now(&note).ok().and_then(|n| n.get("enabledBefore").cloned());
            write_back(&file, &zcode_uninstall(&before, was.as_ref()))?;
            let _ = fs::remove_file(&note);
            Ok(())
        }
        "install" => {
            // Turned on by us: what it was, kept from the first install on.
            let enabled = before.pointer("/hooks/enabled").cloned().unwrap_or(Value::Null);
            if enabled != json!(true) && !note.exists() {
                fs::write(&note, json!({ "enabledBefore": enabled }).to_string()).map_err(|e| e.to_string())?;
            }
            write_json(&file, &zcode_install(&before, &launch_command()))
        }
        _ => Err(format!("unknown action {action}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zcode_hooks_install_status_and_uninstall() {
        let exe = r"C:\Program Files\Wakuwaku\wakuwaku.exe";
        // As the person may have it: a model, their own hook, a field we do not know.
        let theirs = json!({
            "model": "deepseek/deepseek-flash",
            "hooks": { "timeoutMs": 30000, "events": { "Stop": [{ "matcher": "*", "hooks": [{ "type": "command", "command": "notify-me" }] }] } },
            "future": { "x": 1 }
        });
        assert_eq!(zcode_status_of(&theirs, exe), "missing");
        let with = zcode_install(&theirs, exe);
        assert_eq!(zcode_status_of(&with, exe), "ok");
        assert_eq!((&with["hooks"]["enabled"], &with["hooks"]["timeoutMs"], &with["model"], &with["future"]), (&json!(true), &json!(30000), &theirs["model"], &theirs["future"]));
        // Theirs first, then ours; a path with a space needs no quotes (no shell).
        assert_eq!(with["hooks"]["events"]["Stop"][0], theirs["hooks"]["events"]["Stop"][0]);
        assert_eq!(
            with["hooks"]["events"]["Stop"][1],
            json!({ "matcher": "*", "hooks": [{ "type": "process", "command": exe, "args": ["--wakuwaku-zcode-hook", "Stop"], "timeoutMs": 10000 }] })
        );
        assert_eq!(with["hooks"]["events"]["PermissionRequest"][0]["hooks"][0]["timeoutMs"], 600_000);
        assert_eq!(with["hooks"]["events"].as_object().unwrap().len(), 7);
        // Installing twice replaces, never doubles.
        let again = zcode_install(&with, exe);
        assert_eq!(again["hooks"]["events"]["Stop"].as_array().unwrap().len(), 2);
        // Removed: theirs as it was, hooks.enabled too (none before: null).
        assert_eq!(zcode_uninstall(&with, Some(&Value::Null)), theirs);
        let mut was_off = theirs.clone();
        was_off["hooks"]["enabled"] = json!(false);
        assert_eq!(zcode_uninstall(&zcode_install(&was_off, exe), Some(&json!(false))), was_off);
        // On before (no note): it stays on while a hook of theirs is left.
        let mut was_on = theirs.clone();
        was_on["hooks"]["enabled"] = json!(true);
        assert_eq!(zcode_uninstall(&zcode_install(&was_on, exe), None), was_on);
        assert_eq!(zcode_uninstall(&zcode_install(&json!({}), exe), None), json!({}));
        assert_eq!(zcode_uninstall(&zcode_install(&json!({ "x": 1 }), exe), Some(&Value::Null)), json!({ "x": 1 }));

        assert_eq!(zcode_status_of(&with, r"D:\elsewhere\wakuwaku.exe"), "stale");
        if cfg!(windows) {
            assert_eq!(zcode_status_of(&with, &exe.to_lowercase()), "ok");
        }
        let mut part = with.clone();
        part["hooks"]["events"].as_object_mut().unwrap().remove("PostToolUseFailure");
        assert_eq!(zcode_status_of(&part, exe), "partial");
        // Turned off in ZCode: all hooks, or one of ours; on again, as it was.
        let mut off = with.clone();
        off["hooks"]["enabled"] = json!(false);
        assert_eq!(zcode_status_of(&off, exe), "off");
        let mut one = with.clone();
        one["hooks"]["events"]["Stop"][1]["hooks"][0]["enabled"] = json!(false);
        assert_eq!(zcode_status_of(&one, exe), "off");
        one["hooks"]["events"]["Stop"][1]["hooks"][0]["enabled"] = json!(true);
        assert_eq!(zcode_status_of(&one, exe), "ok");
    }
}
