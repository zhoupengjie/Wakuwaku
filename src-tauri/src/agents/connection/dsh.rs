// DeepSeek Harness: her own plugin, inserted by its path
//
// DeepSeek Harness (dsh: its desktop app and its command line) has no hooks
// file of its own: everything in it is a Cordis plugin, mounted by the rows
// of a profile's patch layers. Ours is integrations/dsh/wakuwaku.js, a small
// plugin with no dependencies that runs in DSH's own process and tells her
// port of its sessions (events/dsh.rs reads them). Install writes it, with a
// package.json of its own (an ES module asking nothing of DSH's versions),
// into a folder in DSH's home, <DSH home>/wakuwaku/, and inserts a row for
// it into the home-level patch layer ($DSH_HOME/cordis.patch.yml, ~/.dsh by
// default) by its path beside that file (./wakuwaku/wakuwaku.js), her port
// its config. DSH turns such a path into a file URL and imports it as it is
// (app-boot's anchorInsertedPluginNames, 0.2.0-rc.2 and 0.2.1-rc.2 alike):
// no package name to resolve, nothing in a profile's node_modules, so no
// pnpm run of DSH's plugin manager can take it away, and nothing is
// downloaded. The home layer goes over every profile's own (the desktop
// app's "desktop", the command line's default, any other) and outranks
// them, and DSH never rewrites it (its self-repair renames a profile's own
// patch file only). Our rows sit between two comment lines of ours, so they
// are found and taken out again without a YAML parser.
//
// Until 0.3.2 the row was DSH's own Claude Code hook bridge
// (@deepseek-ai/dsh-hooks-claude-code), which ran this program with DSH_FLAG
// through PowerShell for each event (some 0.9 s each, DSH waiting), its hook
// list in her data folder. The desktop app 0.2.0-rc.2 carried that bridge;
// 0.2.1 no longer does (its experimental packages left the installation), so
// the import failed, DSH only warned, and no event ever came while her
// Connect page said connected (zhoupengjie/Wakuwaku#61). Those rows read as
// stale here (why: bridge): Repair puts the plugin in their place and takes
// the hook list away. The flag still works, for a DSH still running them.
// With the plugin her program's own path no longer matters (it talks HTTP
// to her port): an exe of hers renamed, moved or carrying its version in its
// name breaks nothing.
//
// Written is not loaded. The plugin says it was, itself, as DSH loads it: a
// file beside it (loaded-<profile>.json: the process, when, which version),
// taken away again as it is let go. From those and DSH's running processes
// her Connect page tells loaded, waiting for DSH to start, waiting for a
// restart (DSH was running before, or runs an older version of it), or not
// loaded although DSH has been up for a while since (dsh_load_of): what is
// so, not "connected" because a file was written. DSH watches its patch
// layers (dsh-hmr), so a row added while it runs may load at once.

use super::*;

pub const DSH_FLAG: &str = "--wakuwaku-dsh-hook";
const DSH_PLUGIN: &str = include_str!("../../../../integrations/dsh/wakuwaku.js");
const DSH_PACKAGE: &str = include_str!("../../../../integrations/dsh/package.json");
// Its folder in DSH's home, its file there, and the row's name for it: its
// path beside the patch file.
const DSH_DIR: &str = "wakuwaku";
const DSH_PLUGIN_FILE: &str = "wakuwaku.js";
const DSH_ROW_NAME: &str = "./wakuwaku/wakuwaku.js";
// The other way's rows insert DSH's own Claude Code hook bridge (and 0.3.2's did).
const DSH_BRIDGE: &str = crate::agents::dsh_bridge::HOOKS_PACKAGE;
// The two ways' rows by ids of their own (the bridge's as 0.3.2 wrote it):
// DSH's loader, as a running row of the same id changes its name, keeps the
// plugin it has and only gives it the new config, so a switch while DSH runs
// went on with the way left (seen on 0.2.1-rc.2); a row gone and another
// come, it lets the one go and loads the other.
const DSH_PLUGIN_ID: &str = "wakuwaku-plugin";
const DSH_BRIDGE_ID: &str = "wakuwaku";
// Our rows in the patch file, between these lines.
const DSH_BEGIN: &str = "# wakuwaku begin";
const DSH_END: &str = "# wakuwaku end";
// How long DSH may take from starting to loading its plugins before ours,
// not loaded, is said to be so.
const DSH_LOAD_GRACE_MS: u64 = 60_000;
// DSH's desktop app, as its processes are named (its Host, which loads the
// plugins, is one of them).
const DSH_APP: &str = "DeepSeek Harness.exe";

pub fn dsh_home() -> PathBuf {
    match std::env::var_os("DSH_HOME").filter(|h| !h.is_empty()) {
        Some(home) => PathBuf::from(home),
        None => PathBuf::from(std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).unwrap_or_default()).join(".dsh"),
    }
}

// Whether DeepSeek Harness is on this computer: its folder (made as it first runs).
pub fn dsh_here() -> bool {
    dsh_home().is_dir()
}

pub fn dsh_patch_file() -> PathBuf {
    dsh_home().join("cordis.patch.yml")
}

pub fn dsh_plugin_dir() -> PathBuf {
    dsh_home().join(DSH_DIR)
}

pub fn dsh_plugin_file() -> PathBuf {
    dsh_plugin_dir().join(DSH_PLUGIN_FILE)
}

// The old bridge's hook list in her data folder: no longer written, taken
// away as the plugin is installed or removed.
pub fn dsh_hooks_file() -> PathBuf {
    crate::data::folder().join("dsh-hooks.json")
}

// The plugin's version, as its source says it (export const VERSION = n).
pub fn dsh_plugin_version() -> u32 {
    DSH_PLUGIN.lines().find_map(|l| l.trim().strip_prefix("export const VERSION =")).and_then(|v| v.trim().trim_end_matches(';').parse().ok()).unwrap_or(0)
}

// Our rows: the plugin inserted by its path, her port its config.
fn dsh_block(port: u16) -> String {
    format!(
        "{DSH_BEGIN}: Wakuwaku follows DeepSeek Harness through her own plugin in ./wakuwaku (remove it from her settings)\n\
         - insert:\n    - id: {DSH_PLUGIN_ID}\n      name: '{DSH_ROW_NAME}'\n      config:\n        port: {port}\n{DSH_END}\n"
    )
}

// What our rows in a patch file are: none, the bridge's (the hook list it
// was pointed at), or the plugin's (the name it was inserted by, and the
// port in its config).
#[derive(Clone, Debug, PartialEq)]
pub enum DshRows {
    None,
    Bridge { config_path: String },
    Plugin { name: String, port: Option<u16> },
}

// A YAML scalar as written plainly or in quotes.
fn unquote(value: &str) -> String {
    let value = value.trim();
    if let Some(inner) = value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
        return inner.replace("''", "'");
    }
    if let Some(inner) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
        return inner.to_string();
    }
    value.to_string()
}

// The text without our rows, and what they were.
fn dsh_strip(text: &str) -> (String, DshRows) {
    let mut out = String::new();
    let (mut inside, mut rows) = (false, DshRows::None);
    for line in text.split_inclusive('\n') {
        let bare = line.trim();
        if !inside && bare.starts_with(DSH_BEGIN) {
            inside = true;
            rows = DshRows::Plugin { name: String::new(), port: None };
            continue;
        }
        if inside {
            if bare.starts_with(DSH_END) {
                inside = false;
            } else if let Some(value) = bare.strip_prefix("configPath:") {
                rows = DshRows::Bridge { config_path: unquote(value) };
            } else if let Some(value) = bare.strip_prefix("name:") {
                let given = unquote(value);
                rows = match rows {
                    DshRows::Bridge { config_path } => DshRows::Bridge { config_path },
                    _ if given == DSH_BRIDGE => DshRows::Bridge { config_path: String::new() },
                    DshRows::Plugin { port, .. } => DshRows::Plugin { name: given, port },
                    other => other,
                };
            } else if let Some(value) = bare.strip_prefix("port:") {
                if let DshRows::Plugin { port, .. } = &mut rows {
                    *port = value.trim().parse().ok();
                }
            }
            continue;
        }
        out.push_str(line);
    }
    (out, rows)
}

// The rows of a YAML list a patch file has, past its comments and blank lines.
fn dsh_content(text: &str) -> Vec<&str> {
    text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).collect()
}

// The text without a row that only turns ours off (`id:` either way's and
// `disabled: true`, nothing else): Repair puts it back to work.
fn without_off_rows(text: &str) -> String {
    let mut out = String::new();
    let mut item = String::new();
    let flush = |item: &mut String, out: &mut String| {
        let fields: Vec<&str> = item.lines().map(|l| l.trim().trim_start_matches("- ").trim()).filter(|l| !l.is_empty() && !l.starts_with('#')).collect();
        let only_off = fields.len() == 2 && (row_disabled(item, DSH_PLUGIN_ID) || row_disabled(item, DSH_BRIDGE_ID));
        if !only_off {
            out.push_str(item);
        }
        item.clear();
    };
    for line in text.split_inclusive('\n') {
        let top = line.starts_with("- ");
        let under = line.starts_with(' ') && !item.is_empty();
        if top {
            flush(&mut item, &mut out);
            item.push_str(line);
        } else if under {
            item.push_str(line);
        } else {
            flush(&mut item, &mut out);
            out.push_str(line);
        }
    }
    flush(&mut item, &mut out);
    out
}

// The patch file with the plugin's rows.
pub fn dsh_install(text: Option<&str>, port: u16) -> Result<String, String> {
    dsh_insert(text, &dsh_block(port))
}

// The patch file with the bridge's rows, pointed at a hook list.
pub fn dsh_install_bridge(text: Option<&str>, config_path: &str) -> Result<String, String> {
    dsh_insert(text, &dsh_bridge_block(config_path))
}

// The patch file with these rows of ours (in place of ours before, the
// other way's too, and of a row that turned ours off): added after the
// list's last row, or in place of an empty list ([]); the list as a YAML
// flow sequence of rows ([ ... ]) is not one to add to.
fn dsh_insert(text: Option<&str>, block: &str) -> Result<String, String> {
    let rest = without_off_rows(&dsh_strip(text.unwrap_or("")).0);
    let block = block.to_string();
    let content = dsh_content(&rest);
    if content.is_empty() {
        return Ok(if rest.trim().is_empty() { block } else { format!("{}{block}", with_newline(&rest)) });
    }
    if content == ["[]"] {
        let mut out = String::new();
        for line in rest.split_inclusive('\n') {
            out.push_str(if line.trim() == "[]" { &block } else { line });
        }
        return Ok(out);
    }
    if content[0].starts_with('[') {
        return Err(DSH_FLAT.to_string());
    }
    Ok(format!("{}{block}", with_newline(&rest)))
}

fn with_newline(text: &str) -> String {
    if text.is_empty() || text.ends_with('\n') { text.to_string() } else { format!("{text}\n") }
}

// The patch file without our rows: None when nothing else was in it (the
// file goes); an empty list ([]) when only comments are left, which DSH
// would refuse to start with.
pub fn dsh_uninstall(text: &str) -> Option<String> {
    let (rest, _) = dsh_strip(text);
    if rest.trim().is_empty() {
        return None;
    }
    if dsh_content(&rest).is_empty() {
        return Some(format!("{}[]\n", with_newline(&rest)));
    }
    Some(rest)
}

// How our rows and the plugin's files stand against what this copy writes:
// missing / ok / stale, and why stale: "bridge" (the bridge's rows: DSH's
// Claude Code hook bridge, 0.3.2's or the other way's), "row" (inserted
// otherwise, or for another port: another copy of hers), "files" (the
// plugin's files gone, or not as this version writes them).
pub fn dsh_status_of(text: &str, files_ok: bool, port: u16) -> (&'static str, &'static str) {
    match dsh_strip(text).1 {
        DshRows::None => ("missing", ""),
        DshRows::Bridge { .. } => ("stale", "bridge"),
        DshRows::Plugin { name, port: given } if name != DSH_ROW_NAME || given != Some(port) => ("stale", "row"),
        DshRows::Plugin { .. } if !files_ok => ("stale", "files"),
        DshRows::Plugin { .. } => ("ok", ""),
    }
}

// The plugin's files as this version writes them (line ends aside).
fn dsh_files() -> [(&'static str, &'static str); 2] {
    [("package.json", DSH_PACKAGE), (DSH_PLUGIN_FILE, DSH_PLUGIN)]
}

fn same_text(file: &Path, want: &str) -> bool {
    fs::read_to_string(file).is_ok_and(|text| text.replace("\r\n", "\n") == want.replace("\r\n", "\n"))
}

fn dsh_files_ok(dir: &Path) -> bool {
    dsh_files().iter().all(|(name, want)| same_text(&dir.join(name), want))
}

// --- The other way: DSH's own Claude Code hook bridge (dshVia "bridge") ---------------
//
// The person may pick it on the Agent page (the setting dshVia, the plugin
// by default): her rows insert DSH's Claude Code hook bridge
// (@deepseek-ai/dsh-hooks-claude-code) pointed at a hooks.json in her data
// folder (dsh-hooks.json), as 0.3.2 wrote them, whose command hooks run
// this program with DSH_FLAG and the event's name (hooks.rs dsh_hook). The
// bridge needs its packages in each profile from DSH 0.2.1 on
// (dsh_bridge.rs puts them there, through DSH's own pnpm, and takes those it
// added away again). It runs each hook as a command through DSH's shell,
// which on Windows is PowerShell (pwsh 7, else Windows PowerShell), in
// DSH's sandbox (a write-restricted, low-integrity token: the hook may read
// and open a socket, but not write; so it never starts her, who would run
// so too). A hook costs some 0.9 s there (desktop app 0.2.0-rc.2 in the
// test VM, Windows PowerShell 5.1), and DSH waits on each but
// SessionStart's: so only a turn's start and end, and the few tools worth it
// (an edit, the to-do list, a question to the person, PostToolUse; the
// question's start, PreToolUse) have one, by name. No approval comes this
// way (the bridge has no PermissionRequest). The rows and the hook list
// name this program's path: as she starts, they are written again for
// where she is now (dsh_at_start).

const DSH_TIMEOUT: u64 = 15;
// The tools whose end (and the question's start) is told: DSH's own names.
pub const DSH_POST_TOOLS: &str = "write|edit|str_replace_editor|todo_write|ask_user_question";
const DSH_PRE_TOOLS: &str = "ask_user_question";

// The command DSH's PowerShell runs for an event: the bare path when it
// needs no quoting, else as PowerShell calls a quoted program; its output
// piped away, so that PowerShell waits for it (it does not for a program
// with a window of its own, as this one is, and was gone before her server
// could see where the hook ran: server.rs caller_of).
pub fn dsh_command(exe: &str, event: &str) -> String {
    let is_plain = !exe.is_empty() && exe.chars().all(|c| c.is_ascii_alphanumeric() || "\\/:._-".contains(c));
    if is_plain {
        format!("{exe} {DSH_FLAG} {event} | Out-Null")
    } else {
        format!("& '{}' {DSH_FLAG} {event} | Out-Null", exe.replace('\'', "''"))
    }
}

// The hooks.json the bridge runs.
pub fn dsh_hooks(exe: &str) -> Value {
    let hook = |event: &str| json!([{ "type": "command", "command": dsh_command(exe, event), "timeout": DSH_TIMEOUT }]);
    json!({ "hooks": {
        "SessionStart": [{ "hooks": hook("SessionStart") }],
        "UserPromptSubmit": [{ "hooks": hook("UserPromptSubmit") }],
        "PreToolUse": [{ "matcher": DSH_PRE_TOOLS, "hooks": hook("PreToolUse") }],
        "PostToolUse": [{ "matcher": DSH_POST_TOOLS, "hooks": hook("PostToolUse") }],
        "Stop": [{ "hooks": hook("Stop") }],
    } })
}

// The bridge's rows, pointed at the hooks.json (0.3.2's, word for word).
fn dsh_bridge_block(config_path: &str) -> String {
    format!(
        "{DSH_BEGIN}: Wakuwaku follows DeepSeek Harness through its Claude Code hook bridge (remove it from her settings)\n\
         - insert:\n    - id: {DSH_BRIDGE_ID}\n      name: '{DSH_BRIDGE}'\n      config:\n        configPath: '{}'\n{DSH_END}\n",
        config_path.replace('\'', "''")
    )
}

// The way chosen: dshVia when set; else as the rows have it (the bridge's
// rows: the bridge, as 0.3.2 left them while they work; anything else the plugin).
pub fn dsh_way(setting: &Value, rows: &DshRows) -> &'static str {
    match setting.as_str() {
        Some("bridge") => "bridge",
        Some("plugin") => "plugin",
        _ if matches!(rows, DshRows::Bridge { .. }) => "bridge",
        _ => "plugin",
    }
}

// How the bridge's rows, hook list and packages stand against what this
// copy writes: missing / ok / stale, why: "way" (the plugin's rows there
// instead), "hooks" (the rows or the hook list not as this copy writes
// them: her exe or data folder elsewhere), "packages" (not in a profile),
// "version" (another DSH version's).
pub fn dsh_bridge_status_of(rows: &DshRows, config_path: &str, hooks: Option<&Value>, want: &Value, packages: &str) -> (&'static str, &'static str) {
    match rows {
        DshRows::None => ("missing", ""),
        DshRows::Plugin { .. } => ("stale", "way"),
        DshRows::Bridge { config_path: given } if same_path(given) != same_path(config_path) || hooks != Some(want) => ("stale", "hooks"),
        DshRows::Bridge { .. } if packages == "missing" => ("stale", "packages"),
        DshRows::Bridge { .. } if packages == "version" => ("stale", "version"),
        DshRows::Bridge { .. } => ("ok", ""),
    }
}

// The DSH version the bridge's packages must have: its desktop app's.
pub fn dsh_desktop_version() -> Option<String> {
    versions().get("dsh").and_then(Value::as_str).map(str::to_string)
}

// How the bridge's packages stand: ok (or carried by the app itself, as
// 0.2.0-rc.2 does), missing, version (dsh_bridge.rs packages_state).
pub fn dsh_packages() -> &'static str {
    if crate::agents::dsh_bridge::app_dir().is_some_and(|app| crate::agents::dsh_bridge::bundled(&app)) {
        return "ok";
    }
    let found = crate::agents::dsh_bridge::found_in(&crate::agents::dsh_bridge::profiles(&dsh_home()));
    match dsh_desktop_version() {
        Some(version) => crate::agents::dsh_bridge::packages_state(&found, &version),
        None if !found.is_empty() && found.iter().flatten().all(Option::is_some) => "ok",
        None => "missing",
    }
}

// The bridge written (its hook list's time: `written`) and DSH: waiting (DSH
// not running), effective (a hook of it came since DSH started and since
// the bridge was put in: what her plugin told before a switch is not the
// bridge's), firstEvent (DSH up, none yet). DSH's warning when it can't
// import the bridge goes to its Host's stderr only (the desktop app keeps no
// log file of it), so this is what can be told.
pub fn dsh_bridge_load(apps: &[u64], seen_at: u64, written: u64) -> &'static str {
    match apps.iter().min() {
        None => "waiting",
        Some(&first) if seen_at >= first.max(written) => "effective",
        Some(_) => "firstEvent",
    }
}

// The way chosen (dsh_way), and how its rows and files stand: absent (no
// DeepSeek Harness here), unreadable, or missing / ok / stale and why
// (dsh_status_of, dsh_bridge_status_of), DSH's loading aside.
pub fn dsh_rows_status(port: u16, setting: &Value) -> (&'static str, &'static str, &'static str) {
    if !dsh_here() {
        return (dsh_way(setting, &DshRows::None), "absent", "");
    }
    let text = match fs::read_to_string(dsh_patch_file()) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(_) => return (dsh_way(setting, &DshRows::None), "unreadable", ""),
    };
    let rows = dsh_strip(&text).1;
    let way = dsh_way(setting, &rows);
    let (status, why) = if way == "bridge" {
        let hooks = read_json(&dsh_hooks_file()).ok();
        let packages = if matches!(rows, DshRows::Bridge { .. }) { dsh_packages() } else { "ok" };
        dsh_bridge_status_of(&rows, &dsh_hooks_file().to_string_lossy(), hooks.as_ref(), &dsh_hooks(&launch_command()), packages)
    } else {
        dsh_status_of(&text, dsh_files_ok(&dsh_plugin_dir()), port)
    };
    (way, status, why)
}

// The plugin's word that DSH loaded it (loaded-<profile>.json beside it):
// which process, when (ms since 1970), which version, which code (its
// file's digest as DSH imported it: dsh_plugin_sha), in which profile.
#[derive(Clone, Debug, PartialEq)]
pub struct DshLoaded {
    pub pid: u32,
    pub at: u64,
    pub version: u32,
    pub sha: String,
    pub profile: String,
}

// The plugin's code as this version writes it, as the plugin tells its own
// (SHA-256 of the file's text with CRLF made LF, as a checkout may have
// either, its first 16 hex digits): DSH keeps a module it imported until it
// restarts, so the same name written again is not what runs until then.
pub fn dsh_plugin_sha() -> String {
    sha_of_text(DSH_PLUGIN)
}

fn sha_of_text(text: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(text.replace("\r\n", "\n").as_bytes()).iter().take(8).map(|b| format!("{b:02x}")).collect()
}

pub fn dsh_markers(dir: &Path) -> Vec<DshLoaded> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !(name.starts_with("loaded-") && name.ends_with(".json")) {
            continue;
        }
        let Some(word) = fs::read_to_string(entry.path()).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else { continue };
        let (Some(pid), Some(at)) = (word["pid"].as_u64(), word["at"].as_u64()) else { continue };
        let text = |key: &str| word[key].as_str().unwrap_or("").to_string();
        out.push(DshLoaded { pid: pid as u32, at, version: word["version"].as_u64().unwrap_or(0) as u32, sha: text("sha"), profile: text("profile") });
    }
    out
}

// How DSH stands with the plugin written.
#[derive(Clone, Debug, PartialEq)]
pub enum DshLoad {
    // Loaded by a process still running (the latest such).
    Loaded(DshLoaded),
    // Loaded, but an older version than the one written: a restart takes the new one.
    Older(DshLoaded),
    // DSH's app running since before the plugin was written: a restart loads it.
    Restart,
    // DSH's app up a while since it was written, and it did not load it.
    NotLoaded,
    // DSH not running, or only starting: it loads it as it starts.
    Waiting,
}

impl DshLoad {
    pub fn word(&self) -> &'static str {
        match self {
            DshLoad::Loaded(_) => "loaded",
            DshLoad::Older(_) => "older",
            DshLoad::Restart => "restart",
            DshLoad::NotLoaded => "notLoaded",
            DshLoad::Waiting => "waiting",
        }
    }
}

// From the markers, when each marker's process started (None: gone; ms
// since 1970), when DSH's app's processes started, when the plugin was
// written, now, and the code written (its digest; a word without one, by
// its version).
pub fn dsh_load_of(markers: &[DshLoaded], started: impl Fn(u32) -> Option<u64>, apps: &[u64], written: u64, now: u64, sha: &str, version: u32) -> DshLoad {
    // Of a process still running that was there before its word (its number not reused).
    let live: Vec<&DshLoaded> = markers.iter().filter(|m| started(m.pid).is_some_and(|at| at <= m.at + 1_000)).collect();
    let current = |m: &&&DshLoaded| if m.sha.is_empty() { m.version == version } else { m.sha == sha };
    if let Some(m) = live.iter().filter(current).max_by_key(|m| m.at) {
        return DshLoad::Loaded((*m).clone());
    }
    if let Some(m) = live.iter().max_by_key(|m| m.at) {
        return DshLoad::Older((*m).clone());
    }
    match apps.iter().min() {
        None => DshLoad::Waiting,
        Some(&first) if first < written => DshLoad::Restart,
        Some(&first) if now.saturating_sub(first) >= DSH_LOAD_GRACE_MS => DshLoad::NotLoaded,
        Some(_) => DshLoad::Waiting,
    }
}

// Whether a patch file turns a row of this id off: a row of its list (not
// one inside another's insert) with `id: <id>` and `disabled: true`, as
// DSH's plugin settings write one.
fn row_disabled(text: &str, id: &str) -> bool {
    let (mut is_row, mut off) = (false, false);
    for line in text.lines() {
        let bare = line.trim();
        if bare.is_empty() || bare.starts_with('#') {
            continue;
        }
        // A row's own fields: on its "- " line, or under it by two.
        let field = match line.len() - line.trim_start_matches(' ').len() {
            0 => {
                if is_row && off {
                    return true;
                }
                (is_row, off) = (false, false);
                match bare.strip_prefix("- ") {
                    Some(rest) => rest.trim(),
                    None => continue,
                }
            }
            2 => bare,
            _ => continue,
        };
        if let Some((key, value)) = field.split_once(':') {
            match key.trim() {
                "id" => is_row = unquote(value) == id,
                "disabled" => off = value.trim() == "true",
                _ => {}
            }
        }
    }
    is_row && off
}

// Whether the home-level patch file turns our row off, past our own rows:
// only a patch applied after our insert finds it (a profile's own patch
// file is applied before the home one, and a row of it naming ours finds
// none yet).
fn dsh_turned_off() -> bool {
    fs::read_to_string(dsh_patch_file()).is_ok_and(|text| row_disabled(&dsh_strip(&text).0, DSH_PLUGIN_ID))
}

fn ms_of(time: std::time::SystemTime) -> u64 {
    time.duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

// The Connect page's block for DSH, past its files (settings.rs): the way
// (plugin or bridge, and the setting as it is), hooks absent / unreadable /
// missing / ok / stale / notLoaded, why (stale: bridge, row, files; the
// bridge's: way, hooks, packages, version; notLoaded: off, or "" for no
// reason known), and how DSH stands with it (load: the plugin's loaded,
// older, restart, notLoaded, waiting, with when, which version, which
// profile once loaded; the bridge's waiting, effective, firstEvent).
// seen_at: when a DSH event last came (ms since 1970, 0 for none).
pub fn dsh_view(port: u16, setting: &Value, seen_at: u64) -> Value {
    let (way, status, why) = dsh_rows_status(port, setting);
    let mut view = json!({ "way": way, "via": setting, "hooks": status, "why": why, "dshVersion": dsh_desktop_version() });
    if status != "ok" {
        return view;
    }
    if way == "bridge" {
        let written = fs::metadata(dsh_hooks_file()).and_then(|m| m.modified()).map_or(0, ms_of);
        view["load"] = json!({ "state": dsh_bridge_load(&crate::agents::jump::started_named(DSH_APP), seen_at, written) });
        return view;
    }
    let dir = dsh_plugin_dir();
    let written = fs::metadata(dir.join(DSH_PLUGIN_FILE)).and_then(|m| m.modified()).map_or(0, ms_of);
    let load = dsh_load_of(&dsh_markers(&dir), crate::agents::jump::started_ms, &crate::agents::jump::started_named(DSH_APP), written, ms_of(std::time::SystemTime::now()), &dsh_plugin_sha(), dsh_plugin_version());
    view["load"] = json!({ "state": load.word() });
    if let DshLoad::Loaded(m) | DshLoad::Older(m) = &load {
        view["load"] = json!({ "state": load.word(), "at": m.at, "version": m.version, "profile": m.profile, "pid": m.pid });
    }
    if load == DshLoad::NotLoaded {
        view["hooks"] = json!("notLoaded");
        view["why"] = json!(if dsh_turned_off() { "off" } else { "" });
    }
    view
}

// What keeps our rows out of a patch file, as write_dsh_hooks says it (error_kind: "flowList").
pub(super) const DSH_FLAT: &str = "its list is written as [ … ], not a row a line: our rows cannot be added to it";

// Write the plugin's files where they are not as this version writes them,
// or all of them (`all`: the row changed, so their time says when it was
// written: dsh_load_of).
fn write_dsh_files(dir: &Path, all: bool) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for (name, want) in dsh_files() {
        let file = dir.join(name);
        if all || !same_text(&file, want) {
            fs::write(&file, want).map_err(|e| format!("{}: {e}", file.display()))?;
        }
    }
    Ok(())
}

// Take the plugin's files away (its markers too), and its folder once
// nothing else is in it.
fn remove_dsh_files(dir: &Path) -> Result<(), String> {
    let mut names: Vec<String> = dsh_files().iter().map(|(name, _)| name.to_string()).collect();
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("loaded-") && name.ends_with(".json") {
            names.push(name);
        }
    }
    for name in names {
        match fs::remove_file(dir.join(&name)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(format!("{}: {e}", dir.join(&name).display())),
            _ => {}
        }
    }
    let _ = fs::remove_dir(dir);
    Ok(())
}

// What keeps the bridge's packages out of a profile, as write_dsh_hooks says
// it: this, the kind in brackets (dsh_bridge.rs classify, runtime, busy),
// then what pnpm said (error_kind: "bridge" and the kind).
pub const DSH_BRIDGE_FAILED: &str = "the hook bridge's packages were not installed";

// Her note of the bridge's packages she added to each profile's
// package.json (in her data folder): those she takes away again with it.
fn dsh_bridge_note() -> PathBuf {
    crate::data::folder().join("dsh-bridge.json")
}

fn noted_packages() -> Vec<(PathBuf, Vec<String>)> {
    let Ok(note) = read_json_now(&dsh_bridge_note()) else { return Vec::new() };
    note.get("added")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(profile, names)| (PathBuf::from(profile), names.as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect()))
        .collect()
}

// Added to the note (as JSON values: no map type of its own for it).
fn note_packages(added: &[(PathBuf, Vec<String>)]) {
    let mut all = read_json_now(&dsh_bridge_note()).ok().and_then(|n| n.get("added").cloned()).filter(Value::is_object).unwrap_or_else(|| json!({}));
    for (profile, names) in added.iter().filter(|(_, names)| !names.is_empty()) {
        let key = profile.to_string_lossy();
        let had = &mut all[key.as_ref()];
        if !had.is_array() {
            *had = json!([]);
        }
        for name in names {
            let list = had.as_array_mut().expect("an array");
            if !list.iter().any(|n| n == name.as_str()) {
                list.push(json!(name));
            }
        }
    }
    if all.as_object().is_none_or(|m| m.is_empty()) {
        let _ = fs::remove_file(dsh_bridge_note());
    } else {
        let _ = write_json(&dsh_bridge_note(), &json!({ "added": all }));
    }
}

// Take the bridge's leftovers away: its hook list, and the packages she
// added (best effort: one left behind does no harm, and says why in her log).
fn remove_bridge_leftovers(log: &mut Vec<String>) {
    let _ = fs::remove_file(dsh_hooks_file());
    let noted = noted_packages();
    if noted.is_empty() {
        return;
    }
    let runtime = crate::agents::dsh_bridge::app_dir().and_then(|app| crate::agents::dsh_bridge::runtime_in(&app));
    let mut kept = Vec::new();
    for (profile, names) in noted {
        let gone = match &runtime {
            Some(runtime) => crate::agents::dsh_bridge::remove(runtime, &profile, &names),
            None => Err(crate::agents::dsh_bridge::Failure { kind: "runtime", detail: String::new() }),
        };
        match gone {
            Ok(()) => log.push(format!("dsh: the hook bridge's packages taken out of {}", profile.display())),
            Err(why) => {
                log.push(format!("dsh: the hook bridge's packages left in {} ({}: {})", profile.display(), why.kind, why.detail));
                kept.push((profile, names));
            }
        }
    }
    let _ = fs::remove_file(dsh_bridge_note());
    note_packages(&kept);
}

fn read_patch(file: &Path) -> Result<Option<String>, String> {
    match fs::read_to_string(file) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

// Change DSH's home-level patch file (backed up once beside itself), the
// plugin's folder in DSH's home, and the bridge's hook list in her data
// folder and its packages in DSH's profiles, for a way (plugin, bridge).
// action: install (that way, the other taken away once it is in), remove
// (either). registry: the one the bridge's packages come from, when the
// person picked one (NPMMIRROR); else pnpm's own. Lines for her log in `log`.
pub fn write_dsh_hooks(action: &str, way: &str, port: u16, registry: Option<&str>, log: &mut Vec<String>) -> Result<(), String> {
    let file = dsh_patch_file();
    let text = read_patch(&file)?;
    // A copy of the person's own: none of a file of ours alone.
    let backup = backup_of(&file);
    if text.as_deref().is_some_and(|t| !dsh_strip(t).0.trim().is_empty()) && !backup.exists() {
        fs::copy(&file, &backup).map_err(|e| e.to_string())?;
    }
    match (action, way) {
        ("remove", _) => {
            // The row first: DSH lets the plugin go as it sees it gone.
            if let Some(text) = text.filter(|t| dsh_strip(t).1 != DshRows::None) {
                match dsh_uninstall(&text) {
                    Some(rest) => fs::write(&file, rest).map_err(|e| e.to_string())?,
                    None => fs::remove_file(&file).map_err(|e| e.to_string())?,
                }
            }
            remove_bridge_leftovers(log);
            remove_dsh_files(&dsh_plugin_dir())
        }
        ("install", "bridge") => {
            // The packages first: should they fail, nothing else has changed.
            let app = crate::agents::dsh_bridge::app_dir();
            if !app.as_deref().is_some_and(crate::agents::dsh_bridge::bundled) {
                let failed = |kind: &str, detail: &str| format!("{DSH_BRIDGE_FAILED} [{kind}]: {detail}");
                let runtime = app.as_deref().and_then(crate::agents::dsh_bridge::runtime_in).ok_or_else(|| failed("runtime", "DeepSeek Harness's own Node and pnpm were not found"))?;
                let version = dsh_desktop_version().ok_or_else(|| failed("runtime", "DeepSeek Harness's version could not be read"))?;
                let mut added = Vec::new();
                for profile in crate::agents::dsh_bridge::profiles(&dsh_home()) {
                    match crate::agents::dsh_bridge::install(&runtime, &profile, &version, registry) {
                        Ok(names) => {
                            log.push(format!("dsh: the hook bridge's packages {version} in {}{}", profile.display(), if names.is_empty() { " (there already)" } else { "" }));
                            added.push((profile, names));
                        }
                        Err(why) => {
                            note_packages(&added);
                            return Err(failed(why.kind, &why.detail));
                        }
                    }
                }
                note_packages(&added);
            }
            let hooks = dsh_hooks_file();
            let list = serde_json::to_string_pretty(&dsh_hooks(&launch_command())).map_err(|e| e.to_string())?;
            fs::write(&hooks, format!("{list}\n")).map_err(|e| e.to_string())?;
            forget_read(&hooks);
            let after = dsh_install_bridge(text.as_deref(), &hooks.to_string_lossy())?;
            if text.as_deref() != Some(after.as_str()) {
                fs::write(&file, after).map_err(|e| e.to_string())?;
            }
            remove_dsh_files(&dsh_plugin_dir())
        }
        ("install", _) => {
            let after = dsh_install(text.as_deref(), port)?;
            let changed = text.as_deref() != Some(after.as_str());
            // The files first: DSH may load the plugin as soon as its row is there.
            write_dsh_files(&dsh_plugin_dir(), changed)?;
            if changed {
                fs::write(&file, after).map_err(|e| e.to_string())?;
            }
            remove_bridge_leftovers(log);
            Ok(())
        }
        _ => Err(format!("unknown action {action}")),
    }
}

// What a start sees to (on her own port, with her own data folder: a test's
// copy changes nothing of DSH's), lines for her log:
// - 0.3.2's bridge rows whose packages are missing (DSH 0.2.1 and on), the
//   way not chosen: her plugin in their place (the patch file kept beside
//   itself first): the default the user chose for those upgrading
//   (zhoupengjie/Wakuwaku#61);
// - the bridge as the way (chosen, or 0.3.2's rows that still work): its
//   rows and hook list written again for where this program is now (an exe
//   renamed or moved, one carrying its version in its name); its packages,
//   which need the network, are left to the person's Repair;
// - the plugin as the way: its files written again where an update of hers
//   changed them (DSH takes the new one as it restarts).
// What a start does (dsh_at_start), from the rows, the setting, and lazily
// how the bridge's packages stand: migrate (to the plugin), bridge (its
// rows and hook list again), plugin (its files again, when not as written),
// none.
pub fn dsh_start_plan(rows: &DshRows, setting: &Value, packages: impl FnOnce() -> &'static str, port: u16, plugin_files_ok: bool) -> &'static str {
    match (rows, dsh_way(setting, rows)) {
        (DshRows::Bridge { .. }, "bridge") if setting.is_null() && packages() != "ok" => "migrate",
        (DshRows::Bridge { .. }, "bridge") => "bridge",
        (DshRows::Plugin { name, port: given }, "plugin") if name == DSH_ROW_NAME && *given == Some(port) && !plugin_files_ok => "plugin",
        _ => "none",
    }
}

pub fn dsh_at_start(port: u16, setting: &Value) -> Vec<String> {
    let mut log = Vec::new();
    if !dsh_here() {
        return log;
    }
    let file = dsh_patch_file();
    let Ok(Some(text)) = read_patch(&file) else { return log };
    let rows = dsh_strip(&text).1;
    match (dsh_start_plan(&rows, setting, dsh_packages, port, dsh_files_ok(&dsh_plugin_dir())), &rows) {
        ("migrate", _) => {
            let backup = backup_of(&file);
            if !backup.exists() {
                let _ = fs::copy(&file, &backup);
            }
            match write_dsh_hooks("install", "plugin", port, None, &mut log) {
                Ok(()) => log.push(format!("dsh: 0.3.2's hook bridge rows, its packages not there: her plugin in their place (backup beside it: {})", backup.display())),
                Err(e) => log.push(format!("dsh: 0.3.2's hook bridge rows not replaced: {e}")),
            }
        }
        ("bridge", DshRows::Bridge { config_path }) => {
            let hooks = dsh_hooks_file();
            let want = dsh_hooks(&launch_command());
            if read_json_now(&hooks).ok().as_ref() != Some(&want) {
                match serde_json::to_string_pretty(&want).map_err(|e| e.to_string()).and_then(|list| fs::write(&hooks, format!("{list}\n")).map_err(|e| e.to_string())) {
                    Ok(()) => log.push(format!("dsh: the hook bridge's hook list written again for {}", launch_command())),
                    Err(e) => log.push(format!("dsh: the hook bridge's hook list not written again: {e}")),
                }
                forget_read(&hooks);
            }
            if same_path(config_path) != same_path(&hooks.to_string_lossy()) {
                if let Ok(after) = dsh_install_bridge(Some(&text), &hooks.to_string_lossy()) {
                    match fs::write(&file, after) {
                        Ok(()) => log.push(format!("dsh: the hook bridge's rows pointed at {}", hooks.display())),
                        Err(e) => log.push(format!("dsh: the hook bridge's rows not written again: {e}")),
                    }
                }
            }
        }
        ("plugin", _) => match write_dsh_files(&dsh_plugin_dir(), false) {
            Ok(()) => log.push("dsh: her plugin's files written again for this version (DSH takes it as it restarts)".to_string()),
            Err(e) => log.push(format!("dsh: her plugin's files not written again: {e}")),
        },
        _ => {}
    }
    log
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dsh_plugin_rows_in_and_out_of_its_patch_file() {
        // As DSH's template writes a profile's patch file, rows of the person's own.
        let theirs = "# Your patch layer\n- id: ui-chat\n  name: \"@deepseek-ai/dsh-client-ui-chat\"\n  config:\n    transcriptView: standard\n";
        let with = dsh_install(Some(theirs), 47213).unwrap();
        assert!(with.starts_with(theirs), "{with}");
        assert!(with.contains("\n- insert:\n    - id: wakuwaku-plugin\n      name: './wakuwaku/wakuwaku.js'\n      config:\n        port: 47213\n# wakuwaku end\n"), "{with}");
        assert_eq!(dsh_strip(&with).1, DshRows::Plugin { name: DSH_ROW_NAME.into(), port: Some(47213) });
        // Installing twice replaces, never doubles; removing gives theirs back.
        let again = dsh_install(Some(&with), 47299).unwrap();
        assert_eq!(again.matches(DSH_BEGIN).count(), 1);
        assert!(again.contains("port: 47299") && !again.contains("port: 47213"));
        assert_eq!(dsh_uninstall(&again).as_deref(), Some(theirs));
        // No file, or an empty list: ours alone, then gone, or the list again.
        let alone = dsh_install(None, 47213).unwrap();
        assert!(alone.starts_with(DSH_BEGIN));
        assert_eq!(dsh_uninstall(&alone), None);
        let empty = "# comments\n[]\n";
        let filled = dsh_install(Some(empty), 47213).unwrap();
        assert!(filled.starts_with("# comments\n# wakuwaku begin") && !filled.contains("[]"), "{filled}");
        assert_eq!(dsh_uninstall(&filled).as_deref(), Some(empty));
        // A list written as [ … ] is not added to.
        assert_eq!(super::error_kind(&dsh_install(Some("[ { id: x, disabled: true } ]\n"), 47213).unwrap_err()), "flowList");

        // The old rows, as 0.3.2 wrote them (zhoupengjie/Wakuwaku#61): the
        // bridge, read as such; installing puts the plugin in their place.
        let old = "# wakuwaku begin: Wakuwaku follows DeepSeek Harness through its Claude Code hook bridge (remove it from her settings)\n\
                   - insert:\n    - id: wakuwaku\n      name: '@deepseek-ai/dsh-hooks-claude-code'\n      config:\n        configPath: 'D:\\wakuaku\\wakuwaku-data\\dsh-hooks.json'\n# wakuwaku end\n";
        assert_eq!(dsh_strip(old).1, DshRows::Bridge { config_path: r"D:\wakuaku\wakuwaku-data\dsh-hooks.json".into() });
        assert_eq!(dsh_status_of(&format!("{theirs}{old}"), true, 47213), ("stale", "bridge"));
        let moved = dsh_install(Some(&format!("{theirs}{old}")), 47213).unwrap();
        assert_eq!(moved, with);
        assert_eq!(dsh_uninstall(old), None);

        // How it stands: missing, ok, stale and why.
        assert_eq!(dsh_status_of(theirs, true, 47213), ("missing", ""));
        assert_eq!(dsh_status_of(&with, true, 47213), ("ok", ""));
        assert_eq!(dsh_status_of(&with, true, 47299), ("stale", "row"));
        assert_eq!(dsh_status_of(&with.replace("./wakuwaku/wakuwaku.js", "D:/elsewhere/wakuwaku.js"), true, 47213), ("stale", "row"));
        assert_eq!(dsh_status_of(&with, false, 47213), ("stale", "files"));
        // Quoted otherwise, or not at all: the same.
        assert_eq!(dsh_status_of(&with.replace("'./wakuwaku/wakuwaku.js'", "\"./wakuwaku/wakuwaku.js\""), true, 47213), ("ok", ""));
        assert_eq!(dsh_status_of(&with.replace("'./wakuwaku/wakuwaku.js'", "./wakuwaku/wakuwaku.js"), true, 47213), ("ok", ""));
    }

    #[test]
    fn dsh_bridge_rows_and_hooks_the_other_way() {
        let hooks = r"C:\Users\Me\wakuwaku-data\dsh-hooks.json";
        let theirs = "# Your patch layer\n- id: ui-chat\n  name: \"@deepseek-ai/dsh-client-ui-chat\"\n";
        // The rows as 0.3.2 wrote them, word for word (the issue's file).
        let with = dsh_install_bridge(Some(theirs), hooks).unwrap();
        assert_eq!(
            with,
            format!("{theirs}# wakuwaku begin: Wakuwaku follows DeepSeek Harness through its Claude Code hook bridge (remove it from her settings)\n- insert:\n    - id: wakuwaku\n      name: '@deepseek-ai/dsh-hooks-claude-code'\n      config:\n        configPath: '{hooks}'\n# wakuwaku end\n")
        );
        assert_eq!(dsh_strip(&with).1, DshRows::Bridge { config_path: hooks.into() });
        // Switching: the plugin's rows in place of the bridge's, and back; theirs kept.
        // Each way's row an id of its own, so DSH running lets the one go and loads the other.
        let plugin = dsh_install(Some(&with), 47213).unwrap();
        assert!(plugin.contains("- id: wakuwaku-plugin\n") && !plugin.contains("- id: wakuwaku\n"), "{plugin}");
        assert_eq!(dsh_install_bridge(Some(&plugin), hooks).unwrap(), with);
        assert_eq!(dsh_uninstall(&with).as_deref(), Some(theirs));
        // A quote in the path.
        let quoted = dsh_install_bridge(None, r"D:\it's\dsh-hooks.json").unwrap();
        assert!(quoted.contains(r"configPath: 'D:\it''s\dsh-hooks.json'"));
        assert_eq!(dsh_strip(&quoted).1, DshRows::Bridge { config_path: r"D:\it's\dsh-hooks.json".into() });

        // The hooks: commands for its PowerShell, `| Out-Null` kept (it waits
        // for a windowed program only so), only the tools worth a wait.
        let exe = r"C:\Program Files\Wakuwaku\wakuwaku.exe";
        let want = dsh_hooks(exe);
        let stop = &want["hooks"]["Stop"][0]["hooks"][0];
        assert_eq!(stop["command"], r"& 'C:\Program Files\Wakuwaku\wakuwaku.exe' --wakuwaku-dsh-hook Stop | Out-Null");
        assert_eq!((stop["type"].as_str(), stop["timeout"].as_u64()), (Some("command"), Some(DSH_TIMEOUT)));
        assert_eq!(dsh_command(r"D:\pet\wakuwaku.exe", "PostToolUse"), r"D:\pet\wakuwaku.exe --wakuwaku-dsh-hook PostToolUse | Out-Null");
        assert_eq!(want["hooks"]["PostToolUse"][0]["matcher"], DSH_POST_TOOLS);
        assert_eq!(want["hooks"]["PreToolUse"][0]["matcher"], "ask_user_question");
        assert_eq!(want["hooks"].as_object().unwrap().len(), 5);

        // How it stands.
        let rows = dsh_strip(&with).1;
        assert_eq!(dsh_bridge_status_of(&rows, hooks, Some(&want), &want, "ok"), ("ok", ""));
        assert_eq!(dsh_bridge_status_of(&DshRows::None, hooks, None, &want, "ok"), ("missing", ""));
        assert_eq!(dsh_bridge_status_of(&dsh_strip(&plugin).1, hooks, None, &want, "ok"), ("stale", "way"));
        // Her exe renamed (a version in its name): the hook list no longer this copy's.
        assert_eq!(dsh_bridge_status_of(&rows, hooks, Some(&dsh_hooks(r"D:\w\wakuwaku-v0.3.1-windows-x64.exe")), &want, "ok"), ("stale", "hooks"));
        assert_eq!(dsh_bridge_status_of(&rows, r"D:\elsewhere\dsh-hooks.json", Some(&want), &want, "ok"), ("stale", "hooks"));
        assert_eq!(dsh_bridge_status_of(&rows, hooks, Some(&want), &want, "missing"), ("stale", "packages"));
        assert_eq!(dsh_bridge_status_of(&rows, hooks, Some(&want), &want, "version"), ("stale", "version"));
        // The plugin chosen with the bridge's rows there: to repair.
        assert_eq!(dsh_status_of(&with, true, 47213), ("stale", "bridge"));

        // The way: as chosen; unset, as the rows have it.
        assert_eq!(dsh_way(&json!("bridge"), &DshRows::None), "bridge");
        assert_eq!(dsh_way(&json!("plugin"), &rows), "plugin");
        assert_eq!(dsh_way(&Value::Null, &rows), "bridge");
        assert_eq!(dsh_way(&Value::Null, &DshRows::None), "plugin");
        assert_eq!(dsh_way(&Value::Null, &dsh_strip(&plugin).1), "plugin");

        // DSH and the bridge: an event since it started, or not yet; not running.
        assert_eq!(dsh_bridge_load(&[], 5, 0), "waiting");
        assert_eq!(dsh_bridge_load(&[1_000, 900], 950, 0), "effective");
        assert_eq!(dsh_bridge_load(&[1_000], 500, 0), "firstEvent");
        assert_eq!(dsh_bridge_load(&[1_000], 0, 0), "firstEvent");
        // Switched to while DSH ran: what came before (her plugin's) is not the bridge's.
        assert_eq!(dsh_bridge_load(&[900], 950, 2_000), "firstEvent");
        assert_eq!(dsh_bridge_load(&[900], 2_100, 2_000), "effective");

        // As she starts: 0.3.2's rows whose packages are missing, the way
        // not chosen, become the plugin; chosen, or working, the bridge's
        // rows and hook list are written again; the plugin's files when not
        // as this version writes them.
        let never = || -> &'static str { panic!("not asked") };
        assert_eq!(dsh_start_plan(&rows, &Value::Null, || "missing", 47213, true), "migrate");
        assert_eq!(dsh_start_plan(&rows, &Value::Null, || "version", 47213, true), "migrate");
        assert_eq!(dsh_start_plan(&rows, &Value::Null, || "ok", 47213, true), "bridge");
        assert_eq!(dsh_start_plan(&rows, &json!("bridge"), never, 47213, true), "bridge");
        assert_eq!(dsh_start_plan(&rows, &json!("plugin"), never, 47213, true), "none");
        let plugin_rows = dsh_strip(&plugin).1;
        assert_eq!(dsh_start_plan(&plugin_rows, &Value::Null, never, 47213, false), "plugin");
        assert_eq!(dsh_start_plan(&plugin_rows, &Value::Null, never, 47213, true), "none");
        assert_eq!(dsh_start_plan(&plugin_rows, &Value::Null, never, 47299, false), "none");
        assert_eq!(dsh_start_plan(&DshRows::None, &json!("bridge"), never, 47213, false), "none");

        // Why its packages were not installed, as the page reads it.
        for (kind, want) in [("registry", "bridgeRegistry"), ("version", "bridgeVersion"), ("releaseAge", "bridgeReleaseAge"), ("timeout", "bridgeTimeout"), ("runtime", "bridgeRuntime"), ("busy", "bridgeBusy"), ("x", "bridgeOther")] {
            assert_eq!(super::error_kind(&format!("{DSH_BRIDGE_FAILED} [{kind}]: what pnpm said")), want);
        }
    }

    #[test]
    fn dsh_plugin_files_as_shipped() {
        // An ES module asking nothing of DSH's versions: DSH's compatibility
        // check reads engines.dsh or its DSH peers (none: no constraint; an
        // engines that is no object refuses the package, 0.2.1).
        let package: Value = serde_json::from_str(DSH_PACKAGE).unwrap();
        assert_eq!((package["type"].as_str(), package["main"].as_str()), (Some("module"), Some(DSH_PLUGIN_FILE)));
        assert!(package.get("engines").is_none() && package.get("peerDependencies").is_none() && package.get("dependencies").is_none());
        assert!(DSH_PLUGIN.starts_with("// wakuwaku-dsh-plugin"));
        assert!(dsh_plugin_version() >= 1);
        // Node's own modules alone.
        for line in DSH_PLUGIN.lines().filter(|l| l.starts_with("import ")) {
            assert!(line.contains("'node:"), "{line}");
        }
        assert!(DSH_ROW_NAME.ends_with(&format!("{DSH_DIR}/{DSH_PLUGIN_FILE}")));
    }

    #[test]
    fn dsh_plugin_files_written_and_taken_away() {
        let dir = std::env::temp_dir().join(format!("wakuwaku-dsh-plugin-{}", std::process::id())).join(DSH_DIR);
        let _ = fs::remove_dir_all(&dir);
        assert!(!dsh_files_ok(&dir));
        write_dsh_files(&dir, false).unwrap();
        assert!(dsh_files_ok(&dir));
        // One changed by hand is written again; the others are left as they are.
        fs::write(dir.join(DSH_PLUGIN_FILE), "// not ours\n").unwrap();
        assert!(!dsh_files_ok(&dir));
        write_dsh_files(&dir, false).unwrap();
        assert!(dsh_files_ok(&dir));
        // Its words that DSH loaded it, a broken one passed over.
        fs::write(dir.join("loaded-desktop.json"), r#"{"pid":4321,"at":1792200000000,"version":1,"sha":"0123456789abcdef","profile":"desktop","port":47213}"#).unwrap();
        fs::write(dir.join("loaded-default.json"), "{ half").unwrap();
        assert_eq!(dsh_markers(&dir), vec![DshLoaded { pid: 4321, at: 1792200000000, version: 1, sha: "0123456789abcdef".into(), profile: "desktop".into() }]);
        // Taken away: ours and the words; someone else's file and the folder kept.
        fs::write(dir.join("notes.txt"), "mine").unwrap();
        remove_dsh_files(&dir).unwrap();
        let left: Vec<String> = fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(left, vec!["notes.txt".to_string()]);
        fs::remove_file(dir.join("notes.txt")).unwrap();
        remove_dsh_files(&dir).unwrap();
        assert!(!dir.exists());
        // Nothing there: nothing to do.
        remove_dsh_files(&dir).unwrap();
        let _ = fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn whether_dsh_loaded_the_plugin() {
        let at = 1_792_200_000_000u64;
        let word = |pid: u32, when: u64, sha: &str| DshLoaded { pid, at: when, version: 1, sha: sha.into(), profile: "desktop".into() };
        // pid 10 runs since at-5s; 11 since at+60s (a number reused); 12 is gone.
        let started = |pid: u32| match pid {
            10 => Some(at - 5_000),
            11 => Some(at + 60_000),
            _ => None,
        };
        let written = at - 3_600_000;
        let now = at + 120_000;
        let (new, old) = ("0123456789abcdef", "fedcba9876543210");
        assert_eq!(dsh_load_of(&[word(10, at, new)], started, &[at - 6_000], written, now, new, 1), DshLoad::Loaded(word(10, at, new)));
        // Other code loaded (an older plugin, or the file written again since
        // under the same name: DSH keeps what it imported): a restart takes the new.
        assert_eq!(dsh_load_of(&[word(10, at, old)], started, &[at - 6_000], written, now, new, 1), DshLoad::Older(word(10, at, old)));
        // A word with no digest (none written by that one): by its version.
        assert_eq!(dsh_load_of(&[word(10, at, "")], started, &[at - 6_000], written, now, new, 1), DshLoad::Loaded(word(10, at, "")));
        assert_eq!(dsh_load_of(&[word(10, at, "")], started, &[at - 6_000], written, now, new, 2), DshLoad::Older(word(10, at, "")));
        // Its process gone, or the number another's: not loaded by anything running.
        assert_eq!(dsh_load_of(&[word(12, at, new), word(11, at, new)], started, &[], written, now, new, 1), DshLoad::Waiting);
        // DSH up since before the plugin was written: a restart loads it.
        assert_eq!(dsh_load_of(&[], started, &[written - 1_000, at], written, now, new, 1), DshLoad::Restart);
        // Up a while since, and no word: not loaded; only starting: wait.
        assert_eq!(dsh_load_of(&[], started, &[now - 61_000], written, now, new, 1), DshLoad::NotLoaded);
        assert_eq!(dsh_load_of(&[], started, &[now - 10_000], written, now, new, 1), DshLoad::Waiting);
        assert_eq!(DshLoad::NotLoaded.word(), "notLoaded");
        // The digest as the plugin takes its own: 16 hex digits of the file's SHA-256,
        // the same whether this checkout has its lines ending CRLF or LF.
        assert!(dsh_plugin_sha().len() == 16 && dsh_plugin_sha().chars().all(|c| c.is_ascii_hexdigit()));
        let lf = DSH_PLUGIN.replace("\r\n", "\n");
        assert_eq!(sha_of_text(&lf), dsh_plugin_sha());
        assert_eq!(sha_of_text(&lf.replace('\n', "\r\n")), dsh_plugin_sha());
        // As node's crypto has it (test/dsh.test.js takes the plugin's own the same way).
        assert_eq!(sha_of_text("a\r\nb\r\n"), "911169ddaaf146af");
    }

    #[test]
    fn a_dsh_row_turned_off() {
        // As DSH's plugin settings turn a row off, in a profile's own patch file.
        let off = "# Your patch layer\n- id: ui-chat\n  name: \"@deepseek-ai/dsh-client-ui-chat\"\n- id: wakuwaku\n  disabled: true\n";
        assert!(row_disabled(off, "wakuwaku"));
        assert!(row_disabled("- disabled: true\n  id: 'wakuwaku'\n- id: other\n", "wakuwaku"));
        assert!(!row_disabled("- id: wakuwaku\n  disabled: false\n", "wakuwaku"));
        assert!(!row_disabled("- id: other\n  disabled: true\n- id: wakuwaku\n", "wakuwaku"));
        // One inside another's insert is not a row of this list.
        assert!(!row_disabled("- insert:\n    - id: wakuwaku\n      disabled: true\n", "wakuwaku"));
        // In the home patch file, after our rows (dsh_turned_off reads it past them).
        let ours = dsh_install(None, 47213).unwrap();
        let home = format!("{ours}- id: wakuwaku-plugin\n  disabled: true\n");
        assert!(row_disabled(&dsh_strip(&home).0, DSH_PLUGIN_ID));
        assert!(!row_disabled(&dsh_strip(&ours).0, DSH_PLUGIN_ID));
        // Repair takes a row that only turns ours off away (either way's); the person's rows stay.
        let theirs = "# mine\n- id: ui-chat\n  config:\n    transcriptView: standard\n";
        assert_eq!(dsh_install(Some(&home), 47213).unwrap(), ours);
        assert_eq!(dsh_install(Some(&format!("{theirs}{home}")), 47213).unwrap(), format!("{theirs}{ours}"));
        assert_eq!(dsh_install(Some(&format!("{ours}- id: wakuwaku\n  disabled: true\n")), 47213).unwrap(), ours);
        // One that says more is the person's: kept.
        let more = "- id: wakuwaku-plugin\n  disabled: true\n  config:\n    port: 1\n";
        assert!(dsh_install(Some(&format!("{theirs}{more}")), 47213).unwrap().contains(more));
    }
}
