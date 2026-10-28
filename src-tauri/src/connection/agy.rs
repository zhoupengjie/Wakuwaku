// Antigravity: hooks in ~/.gemini/config/hooks.json
//
// Antigravity (Google's agy, and its 2.0 app, which reads the same file) runs
// hooks only as commands, each tool's in a block of its own by name: ours is
// "wakuwaku". Its events: SessionStart; PreInvocation, each call of the model
// (the first of a turn, invocationNum 0); PreToolUse (a tool call that asks
// the person is held on her island/panel, answered with auto_approve / deny /
// allow: events_agy.rs); PostToolUse; Stop.
//
// Tried with agy 1.3.4 (2026-10-13): it runs a hook as `cmd /c "<command>"`,
// the command's own double quotes escaped as Go escapes them (\"), which cmd
// does not read, so a path with a space can not be quoted: its 8.3 name is
// used for one, and with none it is not installed. And a hook that fails is
// told back in cmd's code page, not UTF-8: agy's turn failed on it (invalid
// UTF-8). Ours answers at once whatever comes (main.rs agy_hook).

use super::*;

pub const AGY_FLAG: &str = "--wakuwaku-agy-hook";
const AGY_BLOCK: &str = "wakuwaku";
const AGY_TIMEOUT: u64 = 10;
pub const AGY_PROMPT_TIMEOUT: u64 = 300;

pub fn gemini_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).unwrap_or_default()).join(".gemini")
}

pub fn agy_hooks_file() -> PathBuf {
    gemini_dir().join("config").join("hooks.json")
}

// Whether Antigravity is on this computer: agy's folder, or the 2.0 app's
// (the standalone IDE's alone does not count: it runs no hooks).
pub fn agy_here() -> bool {
    let gemini = gemini_dir();
    gemini.join("antigravity-cli").is_dir() || gemini.join("antigravity").is_dir()
}

// A path cmd reads as one word with no quotes around it.
fn is_bare(path: &str) -> bool {
    !path.is_empty() && !path.chars().any(|c| c.is_whitespace() || "&|<>^()%!\"',;=`".contains(c))
}

#[cfg(windows)]
fn short_path(path: &str) -> Option<String> {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetShortPathNameW(long: *const u16, short: *mut u16, size: u32) -> u32;
    }
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut out = vec![0u16; 1024];
    // SAFETY: both buffers ours, the second as long as said.
    let n = unsafe { GetShortPathNameW(wide.as_ptr(), out.as_mut_ptr(), out.len() as u32) } as usize;
    (n > 0 && n < out.len()).then(|| String::from_utf16_lossy(&out[..n]))
}

#[cfg(not(windows))]
fn short_path(_path: &str) -> Option<String> {
    None
}

// This program as agy's cmd reads it: as it is, or by its 8.3 name; None
// when neither will do (a space in it, and short names turned off there).
pub fn agy_program(exe: &str) -> Option<String> {
    if is_bare(exe) {
        return Some(exe.to_string());
    }
    short_path(exe).filter(|s| is_bare(s))
}

// Our block: the event's name after the flag (the payload has none).
fn agy_block(program: &str) -> Value {
    let hook = |event: &str| json!({ "type": "command", "command": format!("{program} {AGY_FLAG} {event}"), "timeout": AGY_TIMEOUT });
    let prompt_hook = |event: &str| json!({ "type": "command", "command": format!("{program} {AGY_FLAG} {event}"), "timeout": AGY_PROMPT_TIMEOUT });
    json!({
        "SessionStart": [hook("SessionStart")],
        "PreInvocation": [hook("PreInvocation")],
        "PreToolUse": [{ "matcher": "", "hooks": [prompt_hook("PreToolUse")] }],
        "PostToolUse": [{ "matcher": "", "hooks": [hook("PostToolUse")] }],
        "Stop": [hook("Stop")],
    })
}

pub fn agy_install(file: &Value, program: &str) -> Value {
    let mut out = file.as_object().cloned().unwrap_or_default();
    out.insert(AGY_BLOCK.into(), agy_block(program));
    Value::Object(out)
}

pub fn agy_uninstall(file: &Value) -> Value {
    let mut out = file.as_object().cloned().unwrap_or_default();
    out.remove(AGY_BLOCK);
    Value::Object(out)
}

// How our block stands against what this copy would write: missing / ok /
// stale (another copy of the program, or another version's) / off (turned
// off in the file: enabled false).
pub fn agy_status_of(file: &Value, program: Option<&str>) -> &'static str {
    let Some(ours) = file.get(AGY_BLOCK) else { return "missing" };
    if ours.get("enabled").and_then(Value::as_bool) == Some(false) {
        return "off";
    }
    let mut ours = ours.clone();
    if let Some(m) = ours.as_object_mut() {
        m.remove("enabled");
    }
    match program {
        Some(p) if ours == agy_block(p) => "ok",
        _ => "stale",
    }
}

// absent (no Antigravity here), unreadable, or how our block stands.
pub fn agy_hooks_status() -> &'static str {
    if !agy_here() {
        return "absent";
    }
    match read_json(&agy_hooks_file()) {
        Ok(file) => agy_status_of(&file, agy_program(&launch_command()).as_deref()),
        Err(_) => "unreadable",
    }
}

// What keeps this program from agy's hooks, as write_agy_hooks says it
// (error_kind: "path").
pub(super) const AGY_NO_NAME: &str = "no name for this program without quotes";

// Change Antigravity's hooks.json, backed up once beside itself. action: install, remove.
pub fn write_agy_hooks(action: &str) -> Result<(), String> {
    let file = agy_hooks_file();
    // Read from the disk, not as last read: nothing of someone else's is written back.
    let before = read_json_now(&file)?;
    let after = match action {
        "remove" if !file.exists() => return Ok(()),
        "remove" => return write_back(&file, &agy_uninstall(&before)),
        "install" => {
            let exe = launch_command();
            let Some(program) = agy_program(&exe) else { return Err(format!("{exe}: {AGY_NO_NAME} (a space in its path, and no short name for it)")) };
            agy_install(&before, &program)
        }
        _ => return Err(format!("unknown action {action}")),
    };
    write_json(&file, &after)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agy_hooks_install_status_and_uninstall() {
        let program = r"D:\pet\wakuwaku.exe";
        let theirs = json!({ "my-linter-hook": { "PostToolUse": [{ "matcher": "run_command", "hooks": [{ "command": "./lint.sh" }] }] } });
        assert_eq!(agy_status_of(&theirs, Some(program)), "missing");
        let with = agy_install(&theirs, program);
        assert_eq!(agy_status_of(&with, Some(program)), "ok");
        assert_eq!(with["wakuwaku"]["Stop"][0]["command"], r"D:\pet\wakuwaku.exe --wakuwaku-agy-hook Stop");
        assert_eq!(with["wakuwaku"]["PreToolUse"][0]["matcher"], "");
        assert_eq!(with["wakuwaku"]["PostToolUse"][0]["matcher"], "");
        // Theirs kept, ours alone taken out.
        assert_eq!(with["my-linter-hook"], theirs["my-linter-hook"]);
        assert_eq!(agy_uninstall(&with), theirs);
        assert_eq!(agy_status_of(&with, Some(r"E:\elsewhere\wakuwaku.exe")), "stale");
        assert_eq!(agy_status_of(&with, None), "stale");
        let mut off = with.clone();
        off["wakuwaku"]["enabled"] = json!(false);
        assert_eq!(agy_status_of(&off, Some(program)), "off");
        off["wakuwaku"]["enabled"] = json!(true);
        assert_eq!(agy_status_of(&off, Some(program)), "ok");
    }

    #[test]
    fn a_path_cmd_reads_without_quotes() {
        assert!(is_bare(r"D:\pet\wakuwaku.exe"));
        assert!(is_bare(r"C:\Users\张三\wakuwaku.exe"));
        assert!(!is_bare(r"C:\Program Files\Wakuwaku\wakuwaku.exe"));
        assert!(!is_bare(r"C:\Program Files (x86)\w.exe"));
        assert!(!is_bare(r"C:\a&b\w.exe"));
        assert_eq!(agy_program(r"D:\pet\wakuwaku.exe").as_deref(), Some(r"D:\pet\wakuwaku.exe"));
        assert_eq!(super::error_kind(&format!("x: {AGY_NO_NAME} (a space)")), "path");
    }
}
