// Reasonix: hooks in its global settings.json
//
// Reasonix (DeepSeek-Reasonix: Reasonix Studio, its Electron desktop app,
// whose kernel reasonix-studio-host.exe is its Go engine, and the 2.x
// command line; zhoupengjie/Wakuwaku#60) runs the hooks in <its home>\
// settings.json for every session of either, with no trust asked for (a
// workspace's own .reasonix\settings.json is, as a program approved by its
// fingerprint, again after each change: ours go in the global one). Its
// schema is its own, not Claude Code's: { "hooks": { "<Event>": [{ "match",
// "command", "timeout" (ms), "description", "cwd", "env" }] } }, a hook a
// row, one written Claude Code's way (matcher, nested hooks) skipped
// without a word, and a file that does not parse (a comment, a timeout as
// text) taken as no hooks at all. Read as a session's runtime is built (the
// command line started, a new Studio session or pane, /reload; not /new):
// nothing to restart, but sessions open before it do not have ours.
//
// Each hook is a shell command, run and waited for (no background for the
// settings' hooks), its payload on stdin: on Windows `cmd.exe /d /s /c
// "<command>"` with no window (reasonix_command), so this program with
// REASONIX_FLAG and the event's name hands each to her port and is gone
// (hooks.rs reasonix_hook), some 0.1 s. cmd waits for it, a program with a
// window of its own as it is; one that holds the hook's output open after it
// (a child that inherited it) is a failure, which on its gating events
// (PreToolUse, UserPromptSubmit), as a timeout is, blocks the call or the
// prompt: she starts with no handle of the hook's (detach.rs). A hook that
// cannot run (this program gone, its hooks left) fails in cmd with 1: a
// warning in Reasonix, no block. What a hook prints is Reasonix's: put in the
// model's context at SessionStart, so ours prints nothing; and a
// PermissionRequest of the settings' hooks can only be told, not answered
// (a Claude Code-shaped hook of a plugin package can, before its dialog:
// zhoupengjie/Wakuwaku#60's phase 2).
//
// Tried against its source, the studio branch at 2.43 (2026-10-17); its Go
// 1.x reads the same file the same way (no SubagentStart), its Node 0.x only
// ~/.reasonix/settings.json, 4 of these events and no session id
// (reasonix_line).

use super::*;

pub const REASONIX_FLAG: &str = "--wakuwaku-reasonix-hook";
// The events she follows, each with a hook of ours; how long each may run
// (ms): Reasonix waits on every one of them, and one of its gating events
// (PreToolUse, UserPromptSubmit) timing out blocks the call or the prompt,
// so no less than its own default for them (5 s). Ours is gone in some
// 0.1 s.
pub const REASONIX_EVENTS: [&str; 10] = ["SessionStart", "SessionEnd", "UserPromptSubmit", "PreToolUse", "PostToolUse", "PostToolUseFailure", "PermissionRequest", "Notification", "Stop", "StopFailure"];
const REASONIX_TIMEOUT_MS: u64 = 5_000;
// What `/hooks` in Reasonix names ours by.
const REASONIX_DESCRIPTION: &str = "Wakuwaku: tells her how this session is doing";

// Its home, as its config package's ReasonixHomeDir finds it: REASONIX_HOME,
// else %APPDATA%\reasonix on Windows (~\AppData\Roaming\reasonix without
// %APPDATA%), else ~/.reasonix.
pub fn reasonix_home() -> PathBuf {
    if let Some(home) = env_dir("REASONIX_HOME") {
        return home;
    }
    if cfg!(windows) {
        env_dir("APPDATA").unwrap_or_else(|| home_dir().join("AppData").join("Roaming")).join("reasonix")
    } else {
        home_dir().join(".reasonix")
    }
}

// The older home's settings.json (~/.reasonix), which Reasonix reads in
// place of its home's while that one is not there; none while REASONIX_HOME
// isolates it, or where the two are the same.
fn reasonix_legacy_file() -> Option<PathBuf> {
    if env_dir("REASONIX_HOME").is_some() {
        return None;
    }
    let legacy = home_dir().join(".reasonix");
    (same_path(&legacy.to_string_lossy()) != same_path(&reasonix_home().to_string_lossy())).then(|| legacy.join("settings.json"))
}

// The settings.json Reasonix reads its global hooks from: its home's, or
// the older one while that is missing and the older one is there (hook.go
// Load). Ours are always written into its home's, which then is the one
// read: made from the older one's (the person's hooks there carried over,
// as Reasonix would no longer read them), and taken away again on a remove
// that leaves it as that one is.
pub fn reasonix_settings_file() -> PathBuf {
    reasonix_file_of(reasonix_home().join("settings.json"), reasonix_legacy_file())
}

fn reasonix_file_of(home_file: PathBuf, legacy: Option<PathBuf>) -> PathBuf {
    match legacy {
        Some(legacy) if !home_file.exists() && legacy.is_file() => legacy,
        _ => home_file,
    }
}

// Whether Reasonix is on this computer: its home (made as it first runs),
// or a Reasonix 0.x (whose home is ~/.reasonix: reasonix_line).
pub fn reasonix_here() -> bool {
    reasonix_home().is_dir() || reasonix_line() == Some(0)
}

// Its line, the major version of the Reasonix found here (versions): 2 for
// Reasonix Studio and the 2.x command line, 1 for the Go 1.x (its desktop
// app and command line), 0 for the Node 0.x. 1.x and 2.x run the same
// hooks from the same settings.json, with the same payload (1.x has no
// SubagentStart, which she does not hook); 0.x reads only
// ~/.reasonix/settings.json, runs 4 of the events and gives no session id:
// not one she writes to. None where no file says.
pub fn reasonix_line() -> Option<u64> {
    versions()["reasonix"].as_str().and_then(|v| v.split('.').next()?.parse().ok())
}

// A Reasonix 0.x alone: here, and nothing of hers written to it.
fn reasonix_unsupported() -> bool {
    !reasonix_home().is_dir() && reasonix_line() == Some(0)
}

// The command line Reasonix runs for an event. Its settings' hooks are
// shell commands: on Windows, one like ours goes to `cmd.exe /d /s /c
// "<command>"` as it is written (its hook package's windowsCmdCommandLine),
// so the program is in double quotes (a space in its path is fine); single
// quotes, $ and ` would send it to Git Bash instead (requiresPOSIXShell),
// and % or ! are cmd's own: a path with any of those, or a quote, gets
// none. Elsewhere it goes to sh -c, in single quotes.
pub fn reasonix_command(exe: &str, event: &str) -> Option<String> {
    if exe.is_empty() {
        return None;
    }
    if cfg!(windows) {
        (!exe.chars().any(|c| "\"'$`%!".contains(c) || c.is_control())).then(|| format!("\"{exe}\" {REASONIX_FLAG} {event}"))
    } else {
        Some(format!("'{}' {REASONIX_FLAG} {event}", exe.replace('\'', r"'\''")))
    }
}

fn reasonix_hook(command: String) -> Value {
    json!({ "command": command, "timeout": REASONIX_TIMEOUT_MS, "description": REASONIX_DESCRIPTION })
}

// Its hooks without ours: each event's list (Reasonix's is flat, a hook a
// row, its tools by `match`), events left empty taken out.
fn reasonix_strip(hooks: Option<&Value>) -> Map<String, Value> {
    let mut out = Map::new();
    let Some(Value::Object(hooks)) = hooks else { return out };
    for (event, list) in hooks {
        let Value::Array(list) = list else {
            out.insert(event.clone(), list.clone());
            continue;
        };
        let kept: Vec<Value> = list.iter().filter(|h| !is_ours(h)).cloned().collect();
        if !kept.is_empty() {
            out.insert(event.clone(), Value::Array(kept));
        }
    }
    out
}

// Its settings.json with ours (in place of ours before), after the person's
// own of each event; everything else in it kept. None: no command for this
// program (reasonix_command).
pub fn reasonix_install(file: &Value, exe: &str) -> Option<Value> {
    let mut hooks = reasonix_strip(file.get("hooks"));
    for event in REASONIX_EVENTS {
        let mut list = hooks.get(event).and_then(Value::as_array).cloned().unwrap_or_default();
        list.push(reasonix_hook(reasonix_command(exe, event)?));
        hooks.insert(event.into(), Value::Array(list));
    }
    let mut out = file.as_object().cloned().unwrap_or_default();
    out.insert("hooks".into(), Value::Object(hooks));
    Some(Value::Object(out))
}

// Ours taken out; the hooks too when nothing else is left in them.
pub fn reasonix_uninstall(file: &Value) -> Value {
    let mut out = file.as_object().cloned().unwrap_or_default();
    let hooks = reasonix_strip(file.get("hooks"));
    if hooks.is_empty() {
        out.remove("hooks");
    } else {
        out.insert("hooks".into(), Value::Object(hooks));
    }
    Value::Object(out)
}

// How our hooks stand against what this copy would write: missing / ok /
// stale (another copy of the program, or another version's) / partial (some
// events lack them).
pub fn reasonix_status_of(file: &Value, exe: &str) -> &'static str {
    let mut ours: Vec<(String, Value)> = Vec::new();
    for (event, list) in file.get("hooks").and_then(Value::as_object).into_iter().flatten() {
        for hook in list.as_array().into_iter().flatten().filter(|h| is_ours(h)) {
            ours.push((event.clone(), hook.clone()));
        }
    }
    if ours.is_empty() {
        return "missing";
    }
    let is_want = |event: &str, hook: &Value| {
        let Some(command) = reasonix_command(exe, event) else { return false };
        let mut hook = hook.clone();
        if let Some(h) = hook.as_object_mut() {
            if let Some(found) = h.get("command").and_then(Value::as_str).map(same_path) {
                h.insert("command".into(), json!(found));
            }
        }
        hook == reasonix_hook(same_path(&command))
    };
    if !ours.iter().all(|(event, hook)| REASONIX_EVENTS.contains(&event.as_str()) && is_want(event, hook)) {
        return "stale";
    }
    if REASONIX_EVENTS.iter().all(|want| ours.iter().any(|(event, _)| event == want)) {
        "ok"
    } else {
        "partial"
    }
}

// A JSON file as Reasonix reads it: a byte order mark first is no error.
fn read_json_bom(file: &Path) -> Result<Value, String> {
    match fs::read_to_string(file) {
        Ok(text) => serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(e.to_string()),
    }
}

// absent (no Reasonix here), unsupported (a Reasonix 0.x alone:
// reasonix_line), unreadable, or how our hooks stand in the settings.json it
// reads.
pub fn reasonix_hooks_status() -> &'static str {
    if !reasonix_here() {
        return "absent";
    }
    if reasonix_unsupported() {
        return "unsupported";
    }
    match read_json_bom(&reasonix_settings_file()) {
        Ok(found) => reasonix_status_of(&found, &launch_command()),
        Err(_) => "unreadable",
    }
}

// Change Reasonix's global settings.json, backed up once beside itself.
// action: install (into its home's, made from the older one's while it has
// none: reasonix_settings_file), remove (from the one it reads; its home's
// taken away again when what is left is the older one's as it is, and
// nothing of the person's was there before). Never into a Reasonix 0.x.
pub fn write_reasonix_hooks(action: &str) -> Result<(), String> {
    if action == "install" && reasonix_unsupported() {
        return Err(format!("{}: {REASONIX_OLD}", reasonix_legacy_file().unwrap_or_default().display()));
    }
    write_reasonix_files(action, &reasonix_home().join("settings.json"), reasonix_legacy_file(), &launch_command())
}

fn is_empty_object(value: &Value) -> bool {
    value.as_object().is_some_and(|o| o.is_empty())
}

// The same between its home's file and the older one (None: none read).
fn write_reasonix_files(action: &str, home_file: &Path, legacy: Option<PathBuf>, exe: &str) -> Result<(), String> {
    let legacy = legacy.filter(|l| l.is_file());
    match action {
        "remove" => {
            let file = reasonix_file_of(home_file.to_path_buf(), legacy.clone());
            if !file.exists() {
                return Ok(());
            }
            // Read from the disk, not as last read: nothing of someone else's is written back.
            let after = reasonix_uninstall(&read_json_bom(&file)?);
            let backup = backup_of(&file);
            // A copy that held nothing but hers (an older version's write
            // over a file of hers alone): no copy of the person's, gone too.
            if backup.exists() && read_json_bom(&backup).is_ok_and(|kept| is_empty_object(&reasonix_uninstall(&kept))) {
                fs::remove_file(&backup).map_err(|e| e.to_string())?;
            }
            let as_older = legacy.as_ref().filter(|_| file == home_file).and_then(|l| read_json_bom(l).ok()).is_some_and(|older| older == after);
            if as_older && !backup.exists() {
                let gone = fs::remove_file(&file).map_err(|e| e.to_string());
                forget_read(&file);
                return gone;
            }
            write_back(&file, &after)
        }
        "install" => {
            let before = match (home_file.exists(), &legacy) {
                (false, Some(older)) => read_json_bom(older)?,
                _ => read_json_bom(home_file)?,
            };
            let Some(after) = reasonix_install(&before, exe) else { return Err(format!("{exe}: {REASONIX_NO_NAME}")) };
            // Its home's holding nothing but ours (a repair, ours moved): no
            // copy of it kept, as none of it is the person's (a copy would
            // then keep a remove from giving the file back as it was).
            if home_file.exists() && is_empty_object(&reasonix_uninstall(&before)) {
                return write_back(home_file, &after);
            }
            write_json(home_file, &after)
        }
        _ => Err(format!("unknown action {action}")),
    }
}

// Reasonix's version as its files say it (for versions): Reasonix Studio's
// app (2.x; Electron's, per user, where its installer puts it by default),
// else the 1.x desktop app's (Reasonix, its exe in app\ or beside), else its
// npm package's (2.x's launcher of its Go CLI, 1.x's, or the Node 0.x).
// The 2.x command line from its zip carries none (a Go exe, anywhere).
pub(super) fn reasonix_version(in_local: &dyn Fn(&[&str]) -> Option<PathBuf>, in_npm: &dyn Fn(&[&str]) -> Option<PathBuf>) -> Option<String> {
    let app = |rest: &[&str]| in_local(rest).and_then(|f| file_version(&f, "Reasonix"));
    app(&["Programs", "Reasonix Studio", "Reasonix Studio.exe"])
        .or_else(|| app(&["Programs", "Reasonix", "app", "Reasonix.exe"]))
        .or_else(|| app(&["Programs", "Reasonix", "Reasonix.exe"]))
        .or_else(|| in_npm(&["reasonix"]).and_then(|f| package_version(&f)))
}

pub(super) const REASONIX_NO_NAME: &str = "no command line for this program that cmd and Reasonix both read as it is (a quote, $, `, % or ! in its path)";
pub(super) const REASONIX_OLD: &str = "a Reasonix 0.x, which runs hooks of another kind from here: nothing written";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reasonix_hooks_install_status_and_uninstall() {
        let exe = r"C:\Program Files\Wakuwaku\wakuwaku.exe";
        // As the person may have it: a hook of their own (for one tool), another
        // event's, a key we do not know.
        let theirs = json!({
            "hooks": {
                "PreToolUse": [{ "match": "bash", "command": "node guard.js", "timeout": 3000 }],
                "PreCompact": [{ "command": "echo keep the plan" }]
            },
            "future": { "x": 1 }
        });
        assert_eq!(reasonix_status_of(&theirs, exe), "missing");
        let with = reasonix_install(&theirs, exe).unwrap();
        assert_eq!(reasonix_status_of(&with, exe), "ok");
        assert_eq!(with["future"], theirs["future"]);
        // Theirs first and as it was, then ours: a flat list a hook a row, every tool.
        assert_eq!(with["hooks"]["PreToolUse"][0], theirs["hooks"]["PreToolUse"][0]);
        assert_eq!(
            with["hooks"]["PreToolUse"][1],
            json!({ "command": r#""C:\Program Files\Wakuwaku\wakuwaku.exe" --wakuwaku-reasonix-hook PreToolUse"#, "timeout": 5000, "description": REASONIX_DESCRIPTION })
        );
        assert_eq!(with["hooks"]["PreCompact"], theirs["hooks"]["PreCompact"]);
        assert_eq!(with["hooks"].as_object().unwrap().len(), REASONIX_EVENTS.len() + 1);
        for event in REASONIX_EVENTS {
            assert!(with["hooks"][event].as_array().unwrap().iter().any(|h| h["command"].as_str().unwrap().ends_with(&format!("--wakuwaku-reasonix-hook {event}"))), "{event}");
        }
        // Installing twice replaces, never doubles; removing gives theirs back.
        let again = reasonix_install(&with, exe).unwrap();
        assert_eq!(again["hooks"]["PreToolUse"].as_array().unwrap().len(), 2);
        assert_eq!(reasonix_uninstall(&again), theirs);
        // Ours alone: no hooks left at all.
        assert_eq!(reasonix_uninstall(&reasonix_install(&json!({}), exe).unwrap()), json!({}));
        assert_eq!(reasonix_uninstall(&reasonix_install(&json!({ "hooks": {} }), exe).unwrap()), json!({}));

        // Another copy, another version's, an event of ours missing.
        assert_eq!(reasonix_status_of(&with, r"D:\elsewhere\wakuwaku.exe"), "stale");
        if cfg!(windows) {
            assert_eq!(reasonix_status_of(&with, &exe.to_lowercase()), "ok");
        }
        let mut old = with.clone();
        old["hooks"]["Stop"][0]["timeout"] = json!(30000);
        assert_eq!(reasonix_status_of(&old, exe), "stale");
        let mut part = with.clone();
        part["hooks"].as_object_mut().unwrap().remove("Notification");
        assert_eq!(reasonix_status_of(&part, exe), "partial");
        // One of ours under an event we no longer hook: stale (a repair takes it out).
        let mut extra = with.clone();
        extra["hooks"]["PreCompact"].as_array_mut().unwrap().push(json!({ "command": format!("\"{exe}\" {REASONIX_FLAG} PreCompact"), "timeout": 5000, "description": REASONIX_DESCRIPTION }));
        assert_eq!(reasonix_status_of(&extra, exe), "stale");
        assert_eq!(reasonix_uninstall(&extra), theirs);
    }

    #[test]
    fn reasonix_commands_for_cmd() {
        if cfg!(windows) {
            // In double quotes, as cmd /d /s /c "<command>" takes it whole.
            assert_eq!(reasonix_command(r"D:\pet\wakuwaku.exe", "Stop").as_deref(), Some(r#""D:\pet\wakuwaku.exe" --wakuwaku-reasonix-hook Stop"#));
            assert_eq!(reasonix_command(r"C:\Program Files (x86)\W\wakuwaku.exe", "Stop").as_deref(), Some(r#""C:\Program Files (x86)\W\wakuwaku.exe" --wakuwaku-reasonix-hook Stop"#));
            // What would send it to Git Bash, or that cmd reads as its own: none.
            for odd in [r"C:\Users\O'Brien\w.exe", r"C:\$w\w.exe", r"C:\a`b\w.exe", r"C:\100%\w.exe", r"C:\hi!\w.exe", r#"C:\"\w.exe"#] {
                assert_eq!(reasonix_command(odd, "Stop"), None, "{odd}");
                assert_eq!(reasonix_install(&json!({}), odd), None, "{odd}");
            }
            assert_eq!(super::error_kind(&format!(r"C:\100%\w.exe: {REASONIX_NO_NAME}")), "pathChars");
        } else {
            assert_eq!(reasonix_command("/opt/it's/wakuwaku", "Stop").as_deref(), Some(r"'/opt/it'\''s/wakuwaku' --wakuwaku-reasonix-hook Stop"));
        }
        assert_eq!(reasonix_command("", "Stop"), None);
        assert_eq!(super::error_kind(&format!("x: {REASONIX_OLD}")), "old");
    }

    #[test]
    fn reasonix_reads_its_older_settings_while_its_home_has_none() {
        let dir = std::env::temp_dir().join(format!("wakuwaku-reasonix-{}", std::process::id()));
        let (home, legacy) = (dir.join("roaming").join("reasonix").join("settings.json"), dir.join(".reasonix").join("settings.json"));
        // Neither there: its home's, made there.
        assert_eq!(reasonix_file_of(home.clone(), Some(legacy.clone())), home);
        // Only the older one: that one, as Reasonix reads it.
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::write(&legacy, "{}").unwrap();
        assert_eq!(reasonix_file_of(home.clone(), Some(legacy.clone())), legacy);
        // Its home's there too: its home's, the older one no longer read.
        fs::create_dir_all(home.parent().unwrap()).unwrap();
        fs::write(&home, "\u{feff}{ \"hooks\": {} }").unwrap();
        assert_eq!(reasonix_file_of(home.clone(), Some(legacy.clone())), home);
        assert_eq!(reasonix_file_of(home.clone(), None), home);
        // A byte order mark first is no error; a file that is no JSON is.
        assert_eq!(read_json_bom(&home).unwrap(), json!({ "hooks": {} }));
        assert_eq!(read_json_bom(&dir.join("none.json")).unwrap(), json!({}));
        fs::write(&home, "{ hooks: ").unwrap();
        assert_eq!(super::error_kind(&read_json_bom(&home).unwrap_err()), "invalid");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reasonix_settings_written_and_given_back() {
        let exe = r"D:\pet\wakuwaku.exe";
        let dir = std::env::temp_dir().join(format!("wakuwaku-reasonix-write-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let (home, legacy) = (dir.join("roaming").join("reasonix").join("settings.json"), dir.join(".reasonix").join("settings.json"));
        let theirs = json!({ "hooks": { "Stop": [{ "command": "notify-me" }] } });
        let read = |f: &Path| read_json_bom(f).unwrap();

        // Only the older file, the person's hooks in it: ours go into its
        // home's, made from it; the older one is left as it was, no copy needed.
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::write(&legacy, theirs.to_string()).unwrap();
        write_reasonix_files("install", &home, Some(legacy.clone()), exe).unwrap();
        assert_eq!(reasonix_status_of(&read(&home), exe), "ok");
        assert_eq!(read(&home)["hooks"]["Stop"][0], theirs["hooks"]["Stop"][0]);
        assert_eq!(read(&legacy), theirs);
        assert!(!backup_of(&home).exists());
        // Removed: its home's goes again (what is left is the older one's), and Reasonix reads that one.
        write_reasonix_files("remove", &home, Some(legacy.clone()), exe).unwrap();
        assert!(!home.exists());
        assert_eq!(read(&legacy), theirs);

        // Its home's the person's own: backed up once, ours taken out again, theirs kept.
        let mine = json!({ "hooks": { "PreCompact": [{ "command": "echo plan" }] }, "x": 1 });
        fs::create_dir_all(home.parent().unwrap()).unwrap();
        fs::write(&home, mine.to_string()).unwrap();
        write_reasonix_files("install", &home, Some(legacy.clone()), exe).unwrap();
        assert_eq!(read(&backup_of(&home)), mine);
        assert_eq!(read(&home)["hooks"]["PreCompact"], mine["hooks"]["PreCompact"]);
        write_reasonix_files("remove", &home, Some(legacy.clone()), exe).unwrap();
        assert_eq!(read(&home), mine);
        assert_eq!(read(&legacy), theirs);
        fs::remove_file(backup_of(&home)).unwrap();
        fs::remove_file(&home).unwrap();

        // Neither there: made, and taken away again.
        fs::remove_file(&legacy).unwrap();
        write_reasonix_files("install", &home, Some(legacy.clone()), exe).unwrap();
        assert_eq!(reasonix_status_of(&read(&home), exe), "ok");
        write_reasonix_files("remove", &home, None, exe).unwrap();
        assert!(!home.exists());
        // Nothing to remove; an action it does not know.
        write_reasonix_files("remove", &home, None, exe).unwrap();
        assert!(write_reasonix_files("frob", &home, None, exe).is_err());

        // Ours moved (another copy, stale), repaired: no copy kept of a file
        // of ours alone, and a remove gives no file back, as there was none
        // (seen in the test VM: a repair kept one, and the remove left {}).
        write_reasonix_files("install", &home, None, r"D:\old\wakuwaku.exe").unwrap();
        assert_eq!(reasonix_status_of(&read(&home), exe), "stale");
        write_reasonix_files("install", &home, None, exe).unwrap();
        assert_eq!(reasonix_status_of(&read(&home), exe), "ok");
        assert!(!backup_of(&home).exists());
        // A copy an older version kept of ours alone goes with the remove.
        fs::write(backup_of(&home), reasonix_install(&json!({}), r"D:\old\wakuwaku.exe").unwrap().to_string()).unwrap();
        write_reasonix_files("remove", &home, None, exe).unwrap();
        assert!(!home.exists() && !backup_of(&home).exists());
        // One of the person's own stays, and so does their file.
        fs::write(&home, mine.to_string()).unwrap();
        fs::write(backup_of(&home), mine.to_string()).unwrap();
        write_reasonix_files("install", &home, None, exe).unwrap();
        write_reasonix_files("remove", &home, None, exe).unwrap();
        assert_eq!((read(&home), read(&backup_of(&home))), (mine.clone(), mine));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reasonix_s_version_and_line() {
        let dir = std::env::temp_dir().join(format!("wakuwaku-reasonix-version-{}", std::process::id()));
        fs::create_dir_all(dir.join("reasonix")).unwrap();
        fs::write(dir.join("reasonix").join("package.json"), r#"{ "name": "reasonix", "version": "0.52.0" }"#).unwrap();
        let none = |_: &[&str]| -> Option<PathBuf> { None };
        let npm = |rest: &[&str]| Some(rest.iter().fold(dir.clone(), |p, part| p.join(part)).join("package.json"));
        // No app found: its npm package's, the Node 0.x here.
        assert_eq!(reasonix_version(&none, &npm).as_deref(), Some("0.52.0"));
        assert_eq!(reasonix_version(&none, &none), None);
        // An app's file that is not Reasonix's says nothing.
        #[cfg(windows)]
        {
            let system = |_: &[&str]| Some(PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32").join("kernel32.dll"));
            assert_eq!(reasonix_version(&system, &none), None);
        }
        let _ = fs::remove_dir_all(&dir);
    }
}
