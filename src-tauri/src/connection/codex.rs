// Codex: hooks in ~/.codex/hooks.json
//
// Codex runs hooks only as commands, through the session's shell (on Windows
// `pwsh -NoProfile -Command`, about 0.3 s to start): this program with
// CODEX_FLAG, which hands the event to her port and prints her answer to a
// prompt (hooks.rs). So few hooks make Codex wait: the session starting and
// ending, a prompt, the turn ending (Codex runs no Stop or Interrupt hook in
// the background: none ran in 0.162.1, a slow SessionEnd keeping it up). The
// turn starting and each tool call come from hooks run in the background
// (async), which Codex has from 0.148 on; older ones skip async hooks with a
// warning, so they get those few in the foreground instead: the turn
// starting, an edit (apply_patch), a question.
//
// Codex runs a hook only once the person trusts it (/hooks in Codex), and
// again after it changes, such as this program moving: we never trust them
// on their behalf.

use super::*;

pub const CODEX_FLAG: &str = "--wakuwaku-codex-hook";
pub const CODEX_PROMPT_TIMEOUT: u64 = 300;
// A version: major, minor, patch.
pub type Version = (u64, u64, u64);
// The first Codex that runs async hooks.
const CODEX_ASYNC_SINCE: Version = (0, 148, 0);

// One hook we add: its event, the tools it is for (a regex, None for all),
// how long it may run (s), whether in the background (only for a Codex that
// runs those), and whether only for one that does not.
struct CodexHook {
    event: &'static str,
    matcher: Option<&'static str>,
    timeout: u64,
    is_async: bool,
    is_older: bool,
}

// A prompt waits for the person; Codex gives Interrupt and SessionEnd 3 s at most.
const CODEX_HOOKS: [CodexHook; 11] = [
    CodexHook { event: "SessionStart", matcher: None, timeout: 10, is_async: false, is_older: false },
    CodexHook { event: "PermissionRequest", matcher: None, timeout: CODEX_PROMPT_TIMEOUT, is_async: false, is_older: false },
    CodexHook { event: "Stop", matcher: None, timeout: 10, is_async: false, is_older: false },
    CodexHook { event: "Interrupt", matcher: None, timeout: 3, is_async: false, is_older: false },
    CodexHook { event: "SessionEnd", matcher: None, timeout: 3, is_async: false, is_older: false },
    // The turn starting and every step, in the background.
    CodexHook { event: "UserPromptSubmit", matcher: None, timeout: 10, is_async: true, is_older: false },
    CodexHook { event: "PreToolUse", matcher: None, timeout: 10, is_async: true, is_older: false },
    CodexHook { event: "PostToolUse", matcher: None, timeout: 10, is_async: true, is_older: false },
    // Or, for a Codex without those, the few steps that matter most, waited on.
    CodexHook { event: "UserPromptSubmit", matcher: None, timeout: 10, is_async: false, is_older: true },
    CodexHook { event: "PreToolUse", matcher: Some("^request_user_input$"), timeout: 10, is_async: false, is_older: true },
    CodexHook { event: "PostToolUse", matcher: Some("^(apply_patch|request_user_input)$"), timeout: 10, is_async: false, is_older: true },
];

fn codex_hooks_for(with_async: bool) -> impl Iterator<Item = &'static CodexHook> {
    CODEX_HOOKS.iter().filter(move |h| if with_async { !h.is_older } else { !h.is_async })
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
// upgradable (only background ones lack: Codex was updated since, or put in
// before the turn starting moved there) /
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

// Whether Codex trusts our hooks as they are in hooks.json now
// (zhoupengjie/Wakuwaku#66), read from its config.toml and never written:
// it keeps each hook it trusted as [hooks.state.'<hooks.json>:<event>:<group>:<index>']
// with trusted_hash, the hash of the hook as it was then (enabled = false
// turns one off). The hash is Codex's own (codex-rs hooks discovery.rs
// hook_hash, config fingerprint.rs version_for_toml): "sha256:" and the hex
// of the hook's identity as JSON, keys sorted, nothing between:
// { event_name, hooks: [the hook], matcher } with the timeout as Codex runs
// it and no empty field. Some(true): every one of ours trusted and on;
// Some(false): one is not (new, changed, moved, turned off); None: none of
// ours, or Codex's settings not to be read.
pub fn codex_trusted() -> Option<bool> {
    let hooks = read_json(&codex_hooks_file()).ok()?;
    let config = std::fs::read_to_string(codex_home().join("config.toml")).ok()?;
    codex_trust_of(&codex_hooks_file().to_string_lossy(), &hooks, &config)
}

fn codex_trust_of(hooks_path: &str, hooks: &Value, config: &str) -> Option<bool> {
    let states = codex_hook_states(config);
    let mut any = false;
    for (event, groups) in hooks.get("hooks").and_then(Value::as_object).into_iter().flatten() {
        let label = snake(event);
        for (g, group) in groups.as_array().into_iter().flatten().enumerate() {
            for (h, hook) in group.get("hooks").and_then(Value::as_array).into_iter().flatten().enumerate() {
                if !is_ours(hook) {
                    continue;
                }
                any = true;
                let key = same_key(&format!("{hooks_path}:{label}:{g}:{h}"));
                let trusted = states.get(&key).is_some_and(|(hash, enabled)| *enabled && *hash == codex_hook_hash(&label, group, hook));
                if !trusted {
                    return Some(false);
                }
            }
        }
    }
    any.then_some(true)
}

// SessionStart → session_start.
fn snake(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

// A key as Codex's and ours both name it: Windows' paths in any case.
fn same_key(key: &str) -> String {
    if cfg!(windows) {
        key.replace('/', "\\").to_lowercase()
    } else {
        key.to_string()
    }
}

// Codex's hash of one hook of a group (codex_trusted).
fn codex_hook_hash(event: &str, group: &Value, hook: &Value) -> String {
    use sha2::Digest;
    let timeout = hook.get("timeout").and_then(Value::as_u64);
    let timeout = if matches!(event, "session_end" | "interrupt") { timeout.unwrap_or(1).clamp(1, 3) } else { timeout.unwrap_or(600).max(1) };
    let mut handler = json!({
        "type": hook.get("type").cloned().unwrap_or(json!("command")),
        "command": hook.get("command").cloned().unwrap_or(json!("")),
        "timeout": timeout,
        "async": hook.get("async").and_then(Value::as_bool).unwrap_or(false),
    });
    for (from, to) in [("commandWindows", "commandWindows"), ("command_windows", "commandWindows"), ("statusMessage", "statusMessage"), ("additionalContextLimit", "additionalContextLimit")] {
        if let Some(v) = hook.get(from).filter(|v| !v.is_null()) {
            handler[to] = v.clone();
        }
    }
    let mut identity = json!({ "event_name": event, "hooks": [handler] });
    if let Some(m) = group.get("matcher").filter(|m| !m.is_null()) {
        identity["matcher"] = m.clone();
    }
    let digest = sha2::Sha256::digest(sorted_json(&identity).as_bytes());
    format!("sha256:{}", digest.iter().map(|b| format!("{b:02x}")).collect::<String>())
}

// JSON with every object's keys sorted, nothing between (serde_json's
// sort_all_objects and to_vec, as Codex hashes it).
fn sorted_json(v: &Value) -> String {
    match v {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let parts: Vec<String> = keys.iter().map(|k| format!("{}:{}", Value::String((*k).clone()), sorted_json(&map[*k]))).collect();
            format!("{{{}}}", parts.join(","))
        }
        Value::Array(items) => format!("[{}]", items.iter().map(sorted_json).collect::<Vec<_>>().join(",")),
        other => other.to_string(),
    }
}

// The hooks Codex trusted, by key (same_key): their hash, and whether on.
// Read from its config.toml as Codex writes it: a [hooks.state.'…'] table
// for each, trusted_hash and enabled within.
fn codex_hook_states(config: &str) -> std::collections::HashMap<String, (String, bool)> {
    let mut states = std::collections::HashMap::new();
    let mut at: Option<String> = None;
    for line in config.lines().map(str::trim) {
        if line.starts_with('[') {
            at = line.strip_prefix("[hooks.state.").and_then(|rest| rest.strip_suffix(']')).and_then(toml_key).map(|k| same_key(&k));
            if let Some(key) = &at {
                states.insert(key.clone(), (String::new(), true));
            }
            continue;
        }
        let (Some(key), Some((name, value))) = (&at, line.split_once('=')) else { continue };
        let Some(state) = states.get_mut(key) else { continue };
        match name.trim() {
            "trusted_hash" => state.0 = toml_key(value.trim()).unwrap_or_default(),
            "enabled" => state.1 = value.trim() != "false",
            _ => {}
        }
    }
    states
}

// A quoted TOML key or string: 'literal', or "basic" with its escapes.
fn toml_key(text: &str) -> Option<String> {
    if let Some(rest) = text.strip_prefix('\'') {
        return rest.strip_suffix('\'').map(str::to_string);
    }
    let rest = text.strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next()? {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            other => out.push(other),
        }
    }
    Some(out)
}

// Change Codex's hooks.json, backed up once beside itself. action: install, remove.
pub fn write_codex_hooks(action: &str, waiting: &str) -> Result<(), String> {
    let file = codex_hooks_file();
    // Read from the disk, not as last read: nothing of someone else's is written back.
    let before = read_json_now(&file)?;
    let after = match action {
        "remove" if !file.exists() => return Ok(()),
        "remove" => return write_back(&file, &uninstall(&before)),
        "install" => codex_install(&before, &codex_command(&launch_command()), waiting, codex_runs_async()),
        _ => return Err(format!("unknown action {action}")),
    };
    write_json(&file, &after)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // The turn starting and every step in the background, nothing else
        // for them; the turn ending waited on (Codex runs no Stop in the background).
        for event in ["UserPromptSubmit", "PreToolUse", "PostToolUse"] {
            let groups = with["hooks"][event].as_array().unwrap();
            assert_eq!(groups.len(), 1, "{event}");
            assert_eq!((groups[0].get("matcher"), &groups[0]["hooks"][0]["async"]), (None, &json!(true)), "{event}");
        }
        assert_eq!(with["hooks"]["Stop"][1]["hooks"][0].get("async"), None);
        // Another copy of the program: a repair.
        assert_eq!(codex_status_of(&with, &codex_command(r"E:\elsewhere\wakuwaku.exe"), true), "stale");

        let again = codex_install(&with, &cmd, "waiting", true);
        assert_eq!(again["hooks"]["Stop"].as_array().unwrap().len(), 2);
        assert_eq!(again["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
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
        assert_eq!(old["hooks"]["PreToolUse"][0]["matcher"], "^request_user_input$");
        assert_eq!(old["hooks"]["PostToolUse"][0]["matcher"], "^(apply_patch|request_user_input)$");
        assert_eq!(codex_status_of(&old, &cmd, false), "ok");
        // Codex updated since: only the background ones lack.
        assert_eq!(codex_status_of(&old, &cmd, true), "upgradable");
        let mut part = old.clone();
        part["hooks"].as_object_mut().unwrap().remove("Stop");
        assert_eq!(codex_status_of(&part, &cmd, true), "partial");
        // Installed for a newer Codex, read for an older one: still ours,
        // nothing stale, but the turn starting it would not run.
        assert_eq!(codex_status_of(&codex_install(&json!({}), &cmd, "w", true), &cmd, false), "partial");
    }

    // As put in before the turn starting moved to the background (0.3.3):
    // still working, the background one to be had by a repair, which leaves
    // no foreground one for it or the steps.
    #[test]
    fn hooks_from_before_the_turn_moved_are_upgradable() {
        let cmd = codex_command(r"D:\pet\wakuwaku.exe");
        let hook = |is_async: bool, timeout: u64| {
            let mut h = json!({ "type": "command", "command": cmd, "timeout": timeout });
            if is_async {
                h["async"] = json!(true);
            }
            h
        };
        let group = |matcher: Option<&str>, h: Value| match matcher {
            Some(m) => json!({ "matcher": m, "hooks": [h] }),
            None => json!({ "hooks": [h] }),
        };
        let mut ask = hook(false, CODEX_PROMPT_TIMEOUT);
        ask["statusMessage"] = json!("w");
        let before = json!({ "hooks": {
            "SessionStart": [group(None, hook(false, 10))],
            "UserPromptSubmit": [group(None, hook(false, 10))],
            "PreToolUse": [group(Some("^request_user_input$"), hook(false, 10)), group(None, hook(true, 10))],
            "PostToolUse": [group(Some("^(apply_patch|request_user_input)$"), hook(false, 10)), group(None, hook(true, 10))],
            "PermissionRequest": [group(None, ask)],
            "Stop": [group(None, hook(false, 10))],
            "Interrupt": [group(None, hook(false, 3))],
            "SessionEnd": [group(None, hook(false, 3))],
        } });
        assert_eq!(codex_status_of(&before, &cmd, true), "upgradable");
        let after = codex_install(&before, &cmd, "w", true);
        assert_eq!(codex_status_of(&after, &cmd, true), "ok");
        assert_eq!(after, codex_install(&json!({}), &cmd, "w", true));
    }

    // Codex's own hashes of these hooks (the ones its trust keeps).
    #[test]
    fn codex_hashes_a_hook_as_codex_does() {
        let cmd = codex_command(r"D:\pet\wakuwaku.exe");
        let hook = |more: Value| {
            let mut h = json!({ "type": "command", "command": cmd, "timeout": 10 });
            for (k, v) in more.as_object().unwrap() {
                h[k] = v.clone();
            }
            h
        };
        let none = json!({});
        assert_eq!(codex_hook_hash("session_end", &none, &hook(json!({ "timeout": 3 }))), "sha256:16edcbf293c15c5ee4fb0676738d9120abacbe1847b436a5cb228fe4e7eab2a4");
        // As Codex runs it: SessionEnd's timeout 3 at most.
        assert_eq!(codex_hook_hash("session_end", &none, &hook(json!({ "timeout": 10 }))), codex_hook_hash("session_end", &none, &hook(json!({ "timeout": 3 }))));
        let ask = hook(json!({ "timeout": 300, "statusMessage": "等她" }));
        assert_eq!(codex_hook_hash("permission_request", &none, &ask), "sha256:6be114b27c0c0f23cf1c67e5ee6df5f203500bd1922bee8bf07ed42e87689358");
        assert_eq!(codex_hook_hash("pre_tool_use", &none, &hook(json!({ "async": true }))), "sha256:c663281847ce4e2d721144353a7c412486f17c9c56ca6146c6d75dba1c3d4178");
        let question = json!({ "matcher": "^request_user_input$" });
        assert_eq!(codex_hook_hash("pre_tool_use", &question, &hook(json!({}))), "sha256:9924d21717040e6d0e1d416b7b63f52481e509233655651f512ff85297a788e3");
    }

    // Trusted as they are, or not (new, changed, moved, off): never the
    // events she has or has not heard since she started.
    #[test]
    fn codex_trust_is_read_from_its_config() {
        let cmd = codex_command(r"D:\pet\wakuwaku.exe");
        let path = r"C:\Users\someone\.codex\hooks.json";
        let hooks = codex_install(&json!({ "hooks": { "Stop": [{ "hooks": [{ "type": "command", "command": "notify-me" }] }] } }), &cmd, "等她", true);
        // What Codex writes on trusting them all (keys in another case too),
        // each key quoted as given.
        let config_with = |quote: &dyn Fn(&str) -> String| {
            let mut config = String::from("model = \"x\"\n\n[hooks.state]\n\n");
            for (event, groups) in hooks["hooks"].as_object().unwrap() {
                for (g, group) in groups.as_array().unwrap().iter().enumerate() {
                    let key = format!("{}:{}:{g}:0", if cfg!(windows) { path.to_uppercase() } else { path.to_string() }, snake(event));
                    config.push_str(&format!("[hooks.state.{}]\ntrusted_hash = \"{}\"\n\n", quote(&key), codex_hook_hash(&snake(event), group, &group["hooks"][0])));
                }
            }
            config
        };
        let config = config_with(&|k| format!("'{k}'"));
        assert_eq!(codex_trust_of(path, &hooks, &config), Some(true));
        // Only someone else's trusted, or nothing of ours there.
        assert_eq!(codex_trust_of(path, &hooks, "[hooks.state]\n"), Some(false));
        assert_eq!(codex_trust_of(path, &json!({ "hooks": {} }), &config), None);
        // One turned off in Codex.
        let off = config.replacen("trusted_hash", "enabled = false\ntrusted_hash", 1);
        assert_eq!(codex_trust_of(path, &hooks, &off), Some(false));
        // One changed since (another copy of the program): not trusted as it is.
        let mut changed = hooks.clone();
        changed["hooks"]["SessionEnd"][0]["hooks"][0]["command"] = json!(codex_command(r"E:\elsewhere\wakuwaku.exe"));
        assert_eq!(codex_trust_of(path, &changed, &config), Some(false));
        // Another hooks.json: none of these keys are its.
        assert_eq!(codex_trust_of(r"D:\other\hooks.json", &hooks, &config), Some(false));
        // A key written as a basic string, its backslashes escaped.
        let basic = config_with(&|k| format!("\"{}\"", k.replace('\\', "\\\\")));
        assert_eq!(codex_trust_of(path, &hooks, &basic), Some(true));
    }

    // Whether this computer's Codex trusts her hooks (cargo test -- --ignored --nocapture).
    #[test]
    #[ignore]
    fn codex_trust_here() {
        println!("codex trusted: {:?}", codex_trusted());
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
