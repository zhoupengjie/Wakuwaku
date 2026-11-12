// DeepSeek Harness's own Claude Code hook bridge, the other way she can
// follow it (the setting dshVia "bridge"; her own plugin, connection/dsh.rs, is
// the one she takes by herself): the bridge's packages put into DSH's
// profiles, through DSH's own Node and pnpm.
//
// The bridge (@deepseek-ai/dsh-hooks-claude-code) runs a Claude Code
// hooks.json's command hooks; it needs @deepseek-ai/dsh-hook-protocol
// beside it, a peer DSH's installation does not provide. The desktop app
// 0.2.0-rc.2 carried both in its bundle (app.asar), where DSH finds them
// before a profile's own; from 0.2.1 they are development dependencies of
// DSH only (zhoupengjie/Wakuwaku#61, and the comment on it), so each
// profile that is to load the bridge needs both in its own node_modules,
// declared in its package.json (a pnpm run of DSH's plugin manager takes
// away what is not), at the version of the DSH that loads them (DSH's
// compatibility check holds a package's DSH peers to its own version).
// Profiles set autoInstallPeers: false, so both are named; pnpm 11 holds a
// release younger than a day back (minimumReleaseAge), so both exact
// versions go into the profile's minimumReleaseAgeExclude first.
//
// They are installed as DSH's plugin manager does it: `pnpm add` in the
// profile's folder, with DSH's own pnpm (resources\runtime\pnpm, as the
// desktop app runs it) under DSH's own Node
// (resources\runtime\primary-runtime\dependencies\node), what it prints in
// her data folder (dsh-pnpm.log), against the registry pnpm's configuration names (or one the person
// picks: npmmirror, when that one can't be reached), hidden, with a time
// limit, holding the desktop app's own profile lock meanwhile. The
// profile's package.json, pnpm-lock.yaml and pnpm-workspace.yaml are kept
// before and put back as they were if anything fails, so a profile is never
// left half changed; what pnpm printed says why (classify).

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;
use crate::lock::Lock;

pub const HOOKS_PACKAGE: &str = "@deepseek-ai/dsh-hooks-claude-code";
pub const PROTOCOL_PACKAGE: &str = "@deepseek-ai/dsh-hook-protocol";
pub const PACKAGES: [&str; 2] = [HOOKS_PACKAGE, PROTOCOL_PACKAGE];
// The registry offered when the configured one can't be reached (DSH's own
// plugin manager falls back to it too); only ever used when the person picks it.
pub const NPMMIRROR: &str = "https://registry.npmmirror.com/";
// One pnpm run at most.
const RUN_TIMEOUT: Duration = Duration::from_secs(240);
// What is kept of what pnpm printed.
const OUTPUT_MAX: usize = 16 * 1024;

// DSH's desktop app, where its installer puts it.
pub fn app_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("Programs").join("DeepSeek Harness");
    dir.join("resources").is_dir().then_some(dir)
}

// DSH's own Node and pnpm.
#[cfg_attr(test, derive(Debug))]
pub struct Runtime {
    pub node: PathBuf,
    pub pnpm: PathBuf,
}

// Where they are in an app folder (the first of each that is there).
pub fn runtime_in(app: &Path) -> Option<Runtime> {
    let runtime = app.join("resources").join("runtime");
    let node = [runtime.join("primary-runtime").join("dependencies").join("node").join("bin").join("node.exe"), runtime.join("primary-runtime").join("dependencies").join("node").join("node.exe")]
        .into_iter()
        .find(|p| p.is_file())?;
    let pnpm = [runtime.join("pnpm").join("bin").join("pnpm.mjs"), runtime.join("primary-runtime").join("dependencies").join("pnpm").join("bin").join("pnpm.mjs")]
        .into_iter()
        .find(|p| p.is_file())?;
    Some(Runtime { node, pnpm })
}

// Whether the app carries the bridge itself (0.2.0-rc.2: in app.asar, under
// dsh/node_modules), read from the archive's index: a 16-byte pickle header
// (the last 4 bytes, the index's length), then the index, JSON.
// Read again only when the archive changed (an update): the settings ask
// for it several times a second while open.
pub fn bundled(app: &Path) -> bool {
    type Known = Option<(PathBuf, Option<std::time::SystemTime>, bool)>;
    static KNOWN: Lock<Known> = Lock::new("dsh_bridge::KNOWN", None);
    let asar = app.join("resources").join("app.asar");
    let stamp = fs::metadata(&asar).and_then(|m| m.modified()).ok();
    let mut known = KNOWN.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((path, at, is)) = known.as_ref() {
        if *path == asar && *at == stamp {
            return *is;
        }
    }
    let is = bundled_in_asar(&asar).unwrap_or(false);
    *known = Some((asar, stamp, is));
    is
}

fn bundled_in_asar(asar: &Path) -> Option<bool> {
    let mut file = fs::File::open(asar).ok()?;
    let mut head = [0u8; 16];
    file.read_exact(&mut head).ok()?;
    let len = u32::from_le_bytes([head[12], head[13], head[14], head[15]]) as usize;
    if len == 0 || len > 64 * 1024 * 1024 {
        return None;
    }
    let mut index = vec![0u8; len];
    file.read_exact(&mut index).ok()?;
    let index: Value = serde_json::from_slice(&index).ok()?;
    Some(names_bridge(&index))
}

// Whether an asar index holds both packages under dsh/node_modules.
fn names_bridge(index: &Value) -> bool {
    let scope = index.pointer("/files/dsh/files/node_modules/files/@deepseek-ai/files");
    PACKAGES.iter().all(|name| scope.and_then(|s| s.get(name.trim_start_matches("@deepseek-ai/"))).is_some())
}

// The profiles to put the bridge into: the desktop app's first, then any
// other one made (the command line's: it names one each run).
pub fn profiles(home: &Path) -> Vec<PathBuf> {
    let root = home.join("profiles");
    let mut out: Vec<PathBuf> = fs::read_dir(&root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("package.json").is_file() && p.file_name().is_some_and(|n| n != "node_modules"))
        .collect();
    out.sort_by_key(|p| (p.file_name().map_or(true, |n| n != "desktop"), p.clone()));
    out
}

// A package's version in a profile's node_modules, if there.
pub fn installed_version(profile: &Path, name: &str) -> Option<String> {
    let mut file = profile.join("node_modules");
    for part in name.split('/') {
        file = file.join(part);
    }
    let v: Value = serde_json::from_str(&fs::read_to_string(file.join("package.json")).ok()?).ok()?;
    v.get("version")?.as_str().map(str::to_string)
}

// How the packages stand in the profiles for a DSH version: ok, missing (one
// not there), version (there, another version).
pub fn packages_state(found: &[[Option<String>; 2]], version: &str) -> &'static str {
    if found.iter().flatten().any(Option::is_none) || found.is_empty() {
        "missing"
    } else if found.iter().flatten().any(|v| v.as_deref() != Some(version)) {
        "version"
    } else {
        "ok"
    }
}

pub fn found_in(profiles: &[PathBuf]) -> Vec<[Option<String>; 2]> {
    profiles.iter().map(|p| [installed_version(p, HOOKS_PACKAGE), installed_version(p, PROTOCOL_PACKAGE)]).collect()
}

// A profile's pnpm-workspace.yaml with these exact releases let through
// pnpm's minimumReleaseAge (its minimumReleaseAgeExclude list, made or
// added to; each once).
pub fn with_release_age_exclusions(text: Option<&str>, specs: &[String]) -> String {
    let text = text.unwrap_or("");
    let quoted = |s: &str| format!("'{s}'");
    let listed = |line: &str| {
        let item = line.trim().trim_start_matches('-').trim().trim_matches(|c| c == '\'' || c == '"');
        item.to_string()
    };
    let lines: Vec<&str> = text.lines().collect();
    let Some(at) = lines.iter().position(|l| l.trim_end() == "minimumReleaseAgeExclude:") else {
        let mut out = text.to_string();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("minimumReleaseAgeExclude:\n");
        for spec in specs {
            out.push_str(&format!("  - {}\n", quoted(spec)));
        }
        return out;
    };
    // The list's items: the lines under it that start with "-" (indented or not).
    let mut end = at + 1;
    while end < lines.len() && lines[end].trim_start().starts_with('-') {
        end += 1;
    }
    let have: Vec<String> = lines[at + 1..end].iter().map(|l| listed(l)).collect();
    let indent = lines.get(at + 1).filter(|l| l.trim_start().starts_with('-')).map_or("  ".to_string(), |l| l[..l.len() - l.trim_start().len()].to_string());
    let mut out: Vec<String> = lines[..end].iter().map(|l| l.to_string()).collect();
    for spec in specs.iter().filter(|s| !have.contains(s)) {
        out.push(format!("{indent}- {}", quoted(spec)));
    }
    out.extend(lines[end..].iter().map(|l| l.to_string()));
    let mut out = out.join("\n");
    out.push('\n');
    out
}

// Why a pnpm run failed, from how it ended and what it printed (pnpm's
// ERR_PNPM_* codes and Node's errno names, whatever the locale; as DSH's
// own plugin manager reads them): registry (it could not be reached),
// version (no such release), notFound (no such package), releaseAge (held
// back by minimumReleaseAge), timeout, disk, permission, other.
pub fn classify(output: &str, timed_out: bool) -> &'static str {
    if timed_out {
        return "timeout";
    }
    let has = |needles: &[&str]| needles.iter().any(|n| output.contains(n));
    if has(&["minimumReleaseAge", "NO_MATURE", "ERR_PNPM_TOO_NEW", "release age"]) {
        "releaseAge"
    } else if has(&["ERR_PNPM_FETCH_404", " E404", "404 Not Found", "Not Found - GET"]) {
        "notFound"
    } else if has(&["ERR_PNPM_NO_MATCHING_VERSION", "ETARGET", "No matching version"]) {
        "version"
    } else if has(&["ENOSPC", "no space left on device"]) {
        "disk"
    } else if has(&["EACCES", "EPERM", "permission denied", "EBUSY"]) {
        "permission"
    } else if has(&["ENOTFOUND", "ECONNRESET", "ETIMEDOUT", "ECONNREFUSED", "EAI_AGAIN", "ERR_PNPM_META_FETCH_FAIL", "ERR_PNPM_FETCH_5", "ERR_PNPM_FETCH_TIMEOUT", "FETCH_ERROR", "socket hang up", "UNABLE_TO_GET_ISSUER_CERT", "SELF_SIGNED_CERT", "CERT_HAS_EXPIRED"]) {
        "registry"
    } else {
        "other"
    }
}

// A failed install: its kind (classify, or runtime: no DSH Node or pnpm;
// busy: DSH is changing the profile itself) and what to show of it.
#[cfg_attr(test, derive(Debug))]
pub struct Failure {
    pub kind: &'static str,
    pub detail: String,
}

impl Failure {
    fn new(kind: &'static str, detail: impl Into<String>) -> Self {
        Failure { kind, detail: detail.into() }
    }
}

// The desktop app's own lock on a profile (its project manager's: a file
// "lock" in the profile holding the owner's pid), taken while pnpm runs:
// DSH's own profile operations wait, and so do we while one of its runs.
struct ProfileLock(PathBuf);

impl ProfileLock {
    fn take(profile: &Path) -> Result<ProfileLock, Failure> {
        let path = profile.join("lock");
        let open = || fs::OpenOptions::new().write(true).create_new(true).open(&path);
        let file = match open() {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let owner: u32 = fs::read_to_string(&path).ok().and_then(|t| t.trim().parse().ok()).unwrap_or(0);
                if owner != 0 && crate::jump::started_ms(owner).is_some() {
                    return Err(Failure::new("busy", format!("{} (pid {owner})", path.display())));
                }
                let _ = fs::remove_file(&path);
                open().map_err(|e| Failure::new("permission", format!("{}: {e}", path.display())))?
            }
            Err(e) => return Err(Failure::new("permission", format!("{}: {e}", path.display()))),
        };
        use std::io::Write;
        let _ = writeln!(&file, "{}", std::process::id());
        Ok(ProfileLock(path))
    }
}

impl Drop for ProfileLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

// A run of DSH's plugin manager still writing the profile (its record,
// .plugin-manager/run.json, naming a process still running).
fn dsh_run_active(profile: &Path) -> bool {
    let Ok(text) = fs::read_to_string(profile.join(".plugin-manager").join("run.json")) else { return false };
    serde_json::from_str::<Value>(&text).ok().and_then(|v| v["pid"].as_u64()).is_some_and(|pid| crate::jump::started_ms(pid as u32).is_some())
}

// The three files a pnpm run changes, as they were (None: not there).
struct Kept(Vec<(PathBuf, Option<Vec<u8>>)>);

impl Kept {
    fn of(profile: &Path) -> Kept {
        Kept(["package.json", "pnpm-lock.yaml", "pnpm-workspace.yaml"].iter().map(|name| (profile.join(name), fs::read(profile.join(name)).ok())).collect())
    }

    fn put_back(&self) {
        for (path, bytes) in &self.0 {
            match bytes {
                Some(bytes) => {
                    let _ = fs::write(path, bytes);
                }
                None => {
                    let _ = fs::remove_file(path);
                }
            }
        }
    }

    // A copy of each beside it (.wakuwaku.bak), once: there after us too
    // (tidy takes them away with the packages). The copies this made.
    fn copy_once(&self) -> Vec<PathBuf> {
        let mut made = Vec::new();
        for (path, bytes) in &self.0 {
            let backup = crate::connection::backup_of(path);
            if let (Some(bytes), false) = (bytes, backup.exists()) {
                if fs::write(&backup, bytes).is_ok() {
                    made.push(backup);
                }
            }
        }
        made
    }
}

// A profile's pnpm-workspace.yaml without the release-age exceptions for the
// bridge's packages (any version), and without the list once nothing else
// is in it: as it was before with_release_age_exclusions, where the person
// added nothing there since.
pub fn without_release_age_exclusions(text: &str) -> String {
    let ours = |line: &str| {
        let item = line.trim().trim_start_matches('-').trim().trim_matches(|c| c == '\'' || c == '"');
        PACKAGES.iter().any(|p| item.strip_prefix(p).is_some_and(|rest| rest.starts_with('@')))
    };
    let lines: Vec<&str> = text.lines().collect();
    let Some(at) = lines.iter().position(|l| l.trim_end() == "minimumReleaseAgeExclude:") else { return text.to_string() };
    let mut end = at + 1;
    while end < lines.len() && lines[end].trim_start().starts_with('-') {
        end += 1;
    }
    let kept: Vec<&str> = lines[at + 1..end].iter().copied().filter(|l| !ours(l)).collect();
    if kept.len() == end - at - 1 {
        return text.to_string();
    }
    let mut out: Vec<&str> = lines[..at].to_vec();
    if !kept.is_empty() {
        out.push(lines[at]);
        out.extend(kept);
    }
    out.extend(&lines[end..]);
    let mut out = out.join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    out
}

// After the bridge's packages were taken away: what installing them changed
// in the profile put back where nothing else changed since. The package.json
// as its backup (pnpm leaves it written its own way, a dependencies list
// emptied gone), the release-age exceptions out of pnpm-workspace.yaml, a
// pnpm-lock.yaml that installing made (none was there to copy as the
// package.json was) and that now locks nothing; then the backups, all of
// them put back, gone too. Anything not as expected stays, backups with it.
fn tidy(profile: &Path) {
    let file = |name: &str| profile.join(name);
    let backup = |name: &str| crate::connection::backup_of(&profile.join(name));
    let json = |path: &Path| -> Option<Value> {
        let mut v: Value = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
        if let Some(fields) = v.as_object_mut() {
            fields.retain(|key, value| !(key.ends_with("ependencies") && value.as_object().is_some_and(|m| m.is_empty())));
        }
        Some(v)
    };
    if !backup("package.json").exists() {
        return;
    }
    let mut all = match (json(&file("package.json")), json(&backup("package.json"))) {
        (Some(now), Some(was)) if now == was => fs::copy(backup("package.json"), file("package.json")).is_ok(),
        _ => false,
    };
    let workspace = file("pnpm-workspace.yaml");
    if let Ok(text) = fs::read_to_string(&workspace) {
        let tidied = without_release_age_exclusions(&text);
        if tidied != text {
            let _ = fs::write(&workspace, &tidied);
        }
    }
    let same = |a: &Path, b: &Path| matches!((fs::read_to_string(a), fs::read_to_string(b)), (Ok(x), Ok(y)) if x.replace("\r\n", "\n") == y.replace("\r\n", "\n"));
    all &= !workspace.exists() || same(&workspace, &backup("pnpm-workspace.yaml"));
    let lock = file("pnpm-lock.yaml");
    if lock.exists() && !backup("pnpm-lock.yaml").exists() {
        let empty = fs::read_to_string(&lock).is_ok_and(|t| !t.lines().any(|l| matches!(l.trim_end(), "packages:" | "snapshots:") || l.trim_start().starts_with("dependencies:")));
        all &= empty && fs::remove_file(&lock).is_ok();
    } else if lock.exists() {
        all &= same(&lock, &backup("pnpm-lock.yaml"));
    }
    if all {
        for name in ["package.json", "pnpm-lock.yaml", "pnpm-workspace.yaml"] {
            let _ = fs::remove_file(backup(name));
        }
    }
}

// Run pnpm in a profile, hidden, for RUN_TIMEOUT at most: whether it ended
// well, what it printed (its tail), whether time ran out. What it prints
// goes to a file in her data folder (dsh-pnpm.log, the last run's, there
// after it too), read when it ends: no threads to read pipes with.
fn run_pnpm(runtime: &Runtime, profile: &Path, args: &[String]) -> Result<(bool, String, bool), Failure> {
    let log = crate::data::folder().join("dsh-pnpm.log");
    let out = fs::File::create(&log).map_err(|e| Failure::new("permission", format!("{}: {e}", log.display())))?;
    let err = out.try_clone().map_err(|e| Failure::new("permission", format!("{}: {e}", log.display())))?;
    let mut command = Command::new(&runtime.node);
    // Her own environment as it is: these two packages run no scripts, so
    // pnpm needs no node of DSH's on PATH (and a command with an
    // environment of its own costs the exe the code that builds one); its
    // output to a file comes without colours.
    command.arg(&runtime.pnpm).args(args).current_dir(profile).stdin(Stdio::null()).stdout(Stdio::from(out)).stderr(Stdio::from(err));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command.spawn().map_err(|e| Failure::new("runtime", format!("{}: {e}", runtime.node.display())))?;
    let until = Instant::now() + RUN_TIMEOUT;
    let (ok, timed_out) = loop {
        match child.try_wait() {
            Ok(Some(status)) => break (status.success(), false),
            Ok(None) if Instant::now() < until => std::thread::sleep(Duration::from_millis(200)),
            _ => {
                // Its whole tree: pnpm's own children too.
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    let _ = Command::new("taskkill").args(["/T", "/F", "/PID", &child.id().to_string()]).creation_flags(0x0800_0000).status();
                }
                let _ = child.kill();
                let _ = child.wait();
                break (false, true);
            }
        }
    };
    let output = fs::read(&log).unwrap_or_default();
    let tail = &output[output.len().saturating_sub(OUTPUT_MAX)..];
    Ok((ok, String::from_utf8_lossy(tail).into_owned(), timed_out))
}

// The last lines of what pnpm printed that say something, for the person.
fn gist(output: &str) -> String {
    let lines: Vec<&str> = output.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let errors: Vec<&str> = lines.iter().copied().filter(|l| l.contains("ERR_") || l.contains("ERR!") || l.contains("Error") || l.contains("error")).collect();
    let pick = if errors.is_empty() { &lines[lines.len().saturating_sub(3)..] } else { &errors[errors.len().saturating_sub(3)..] };
    crate::events::clip(&pick.join(" · "), 400)
}

// Put both packages, at this version, into a profile (none if there already).
// Ok with the names added to its package.json by this run (for taking
// them away again with the bridge; none of the person's own).
pub fn install(runtime: &Runtime, profile: &Path, version: &str, registry: Option<&str>) -> Result<Vec<String>, Failure> {
    let have: Vec<Option<String>> = PACKAGES.iter().map(|p| installed_version(profile, p)).collect();
    if have.iter().all(|v| v.as_deref() == Some(version)) {
        return Ok(Vec::new());
    }
    if dsh_run_active(profile) {
        return Err(Failure::new("busy", profile.join(".plugin-manager").join("run.json").display().to_string()));
    }
    let _lock = ProfileLock::take(profile)?;
    let kept = Kept::of(profile);
    let copies = kept.copy_once();
    let declared = declared(profile);
    let specs: Vec<String> = PACKAGES.iter().map(|p| format!("{p}@{version}")).collect();
    let workspace = profile.join("pnpm-workspace.yaml");
    let before = fs::read_to_string(&workspace).ok();
    if let Err(e) = fs::write(&workspace, with_release_age_exclusions(before.as_deref(), &specs)) {
        return Err(Failure::new("permission", format!("{}: {e}", workspace.display())));
    }
    // Its retries sooner than pnpm's own (10 s, then up to a minute): a
    // registry that can't be reached is said in some seconds, not a minute
    // and more (72 s seen, pnpm 11.7 with DSH 0.2.0-rc.2's runtime).
    let mut args: Vec<String> = vec!["add".into(), "--save-exact".into(), "--config.fetch-retries=2".into(), "--config.fetch-retry-mintimeout=2000".into(), "--config.fetch-retry-maxtimeout=8000".into()];
    args.extend(specs.iter().cloned());
    if let Some(registry) = registry {
        args.push(format!("--registry={registry}"));
    }
    let outcome = run_pnpm(runtime, profile, &args);
    let failure = match outcome {
        Err(failure) => Some(failure),
        Ok((true, _, _)) if PACKAGES.iter().all(|p| installed_version(profile, p).as_deref() == Some(version)) => None,
        Ok((true, output, _)) => Some(Failure::new("other", format!("pnpm ended well, but the packages are not there: {}", gist(&output)))),
        Ok((false, output, timed_out)) => Some(Failure::new(classify(&output, timed_out), gist(&output))),
    };
    if let Some(failure) = failure {
        kept.put_back();
        for copy in copies {
            let _ = fs::remove_file(copy);
        }
        return Err(failure);
    }
    Ok(PACKAGES.iter().filter(|p| !declared.iter().any(|d| d == *p)).map(|p| p.to_string()).collect())
}

// The names a profile's package.json declares as dependencies.
fn declared(profile: &Path) -> Vec<String> {
    let Some(v) = fs::read_to_string(profile.join("package.json")).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else { return Vec::new() };
    ["dependencies", "devDependencies", "optionalDependencies"].iter().filter_map(|k| v.get(*k).and_then(Value::as_object)).flat_map(|m| m.keys().cloned()).collect()
}

// Take packages this added away from a profile again (pnpm remove), and
// what adding them changed (tidy); the profile's files put back if that
// fails.
pub fn remove(runtime: &Runtime, profile: &Path, names: &[String]) -> Result<(), Failure> {
    let names: Vec<String> = names.iter().filter(|n| declared(profile).contains(n)).cloned().collect();
    if names.is_empty() {
        return Ok(());
    }
    if dsh_run_active(profile) {
        return Err(Failure::new("busy", profile.join(".plugin-manager").join("run.json").display().to_string()));
    }
    let _lock = ProfileLock::take(profile)?;
    let kept = Kept::of(profile);
    let mut args: Vec<String> = vec!["remove".into()];
    args.extend(names);
    match run_pnpm(runtime, profile, &args)? {
        (true, _, _) => {
            tidy(profile);
            Ok(())
        }
        (false, output, timed_out) => {
            kept.put_back();
            Err(Failure::new(classify(&output, timed_out), gist(&output)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn minimum_release_age_lets_these_releases_through() {
        let specs = vec!["@deepseek-ai/dsh-hooks-claude-code@0.2.1-rc.2".to_string(), "@deepseek-ai/dsh-hook-protocol@0.2.1-rc.2".to_string()];
        // As the desktop app writes a profile's (0.2.0-rc.2's template).
        let desktop = "packages:\n  - .\n\nnodeLinker: hoisted\nautoInstallPeers: false\n";
        let with = with_release_age_exclusions(Some(desktop), &specs);
        assert_eq!(with, format!("{desktop}minimumReleaseAgeExclude:\n  - '@deepseek-ai/dsh-hooks-claude-code@0.2.1-rc.2'\n  - '@deepseek-ai/dsh-hook-protocol@0.2.1-rc.2'\n"));
        // Again: the same, nothing twice.
        assert_eq!(with_release_age_exclusions(Some(&with), &specs), with);
        // A list there already (as pnpm writes the exemptions it grants): added to, in its indent.
        let theirs = "packages:\n  - .\nminimumReleaseAge: 1440\nminimumReleaseAgeExclude:\n    - some-pkg@1.0.0\nautoInstallPeers: false";
        let added = with_release_age_exclusions(Some(theirs), &specs);
        assert_eq!(added, "packages:\n  - .\nminimumReleaseAge: 1440\nminimumReleaseAgeExclude:\n    - some-pkg@1.0.0\n    - '@deepseek-ai/dsh-hooks-claude-code@0.2.1-rc.2'\n    - '@deepseek-ai/dsh-hook-protocol@0.2.1-rc.2'\nautoInstallPeers: false\n");
        // One of ours listed already, quoted otherwise: the other alone added.
        let one = "minimumReleaseAgeExclude:\n  - \"@deepseek-ai/dsh-hooks-claude-code@0.2.1-rc.2\"\n";
        assert_eq!(with_release_age_exclusions(Some(one), &specs), format!("{one}  - '@deepseek-ai/dsh-hook-protocol@0.2.1-rc.2'\n"));
        // No file.
        assert!(with_release_age_exclusions(None, &specs).starts_with("minimumReleaseAgeExclude:\n  - '"));
        // Taken out again: as it was; the person's own exceptions kept; nothing of ours, untouched.
        assert_eq!(without_release_age_exclusions(&with), desktop);
        assert_eq!(without_release_age_exclusions(&added), "packages:\n  - .\nminimumReleaseAge: 1440\nminimumReleaseAgeExclude:\n    - some-pkg@1.0.0\nautoInstallPeers: false\n");
        assert_eq!(without_release_age_exclusions(desktop), desktop);
        let other = "minimumReleaseAgeExclude:\n  - '@deepseek-ai/dsh-hooks-claude-code-extra@1.0.0'\n";
        assert_eq!(without_release_age_exclusions(other), other);
    }

    // What installing the bridge changed in a profile, put back after its
    // packages are taken away (as pnpm 11 left the files on DSH 0.2.1-rc.2),
    // and nothing put back where the person changed something meanwhile.
    #[test]
    fn a_profile_tidied_after_the_bridge() {
        let dir = std::env::temp_dir().join(format!("wakuwaku-dsh-tidy-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let package = "{\n  \"name\": \"dsh-profile-desktop\",\n  \"private\": true,\n  \"dependencies\": {},\n  \"dsh\": {\n    \"profile\": {\n      \"bundles\": [\n        \"@deepseek-ai/dsh-base\"\n      ]\n    }\n  }\n}\n";
        let workspace = "packages:\n  - .\n\nnodeLinker: hoisted\nautoInstallPeers: false\n";
        let specs = vec!["@deepseek-ai/dsh-hooks-claude-code@0.2.1-rc.2".to_string(), "@deepseek-ai/dsh-hook-protocol@0.2.1-rc.2".to_string()];
        let lock = "lockfileVersion: '9.0'\n\nsettings:\n  autoInstallPeers: false\n  excludeLinksFromLockfile: false\n\nimporters:\n\n  .: {}\n";
        let after = |pkg: &str| {
            fs::write(dir.join("package.json"), pkg).unwrap();
            fs::write(dir.join("pnpm-workspace.yaml"), with_release_age_exclusions(Some(workspace), &specs)).unwrap();
            fs::write(dir.join("pnpm-lock.yaml"), lock).unwrap();
            fs::write(dir.join("package.json.wakuwaku.bak"), package).unwrap();
            fs::write(dir.join("pnpm-workspace.yaml.wakuwaku.bak"), workspace).unwrap();
        };
        // As pnpm remove leaves them: put back, the lock gone, no backups.
        after("{\n  \"name\": \"dsh-profile-desktop\",\n  \"private\": true,\n  \"dsh\": {\n    \"profile\": {\n      \"bundles\": [\n        \"@deepseek-ai/dsh-base\"\n      ]\n    }\n  }\n}\n");
        tidy(&dir);
        assert_eq!(fs::read_to_string(dir.join("package.json")).unwrap(), package);
        assert_eq!(fs::read_to_string(dir.join("pnpm-workspace.yaml")).unwrap(), workspace);
        assert!(!dir.join("pnpm-lock.yaml").exists() && !dir.join("package.json.wakuwaku.bak").exists() && !dir.join("pnpm-workspace.yaml.wakuwaku.bak").exists());
        // A bundle the person added meanwhile: their package.json stays, the backups too.
        let theirs = package.replace("\"@deepseek-ai/dsh-base\"", "\"@deepseek-ai/dsh-base\",\n        \"@someone/else\"");
        after(&theirs);
        tidy(&dir);
        assert_eq!(fs::read_to_string(dir.join("package.json")).unwrap(), theirs);
        assert_eq!(fs::read_to_string(dir.join("pnpm-workspace.yaml")).unwrap(), workspace);
        assert!(dir.join("package.json.wakuwaku.bak").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn why_a_pnpm_run_failed() {
        assert_eq!(classify(" ERR_PNPM_META_FETCH_FAIL  GET https://registry.npmjs.org/@deepseek-ai%2Fdsh-hook-protocol: request to https://registry.npmjs.org/ failed, reason: getaddrinfo ENOTFOUND registry.npmjs.org", false), "registry");
        assert_eq!(classify("connect ECONNREFUSED 127.0.0.1:9", false), "registry");
        assert_eq!(classify(" ERR_PNPM_NO_MATCHING_VERSION  No matching version found for @deepseek-ai/dsh-hook-protocol@0.2.9", false), "version");
        assert_eq!(classify(" ERR_PNPM_FETCH_404  GET https://registry.npmjs.org/@deepseek-ai%2Fdsh-nope: Not Found - 404", false), "notFound");
        assert_eq!(classify(" ERR_PNPM_NO_MATURE_MATCHING_VERSION  Version 0.2.1-rc.2 was published less than minimumReleaseAge ago", false), "releaseAge");
        assert_eq!(classify("ENOSPC: no space left on device", false), "disk");
        assert_eq!(classify("EPERM: operation not permitted, rename", false), "permission");
        assert_eq!(classify("anything", true), "timeout");
        assert_eq!(classify("something else went wrong", false), "other");
        assert_eq!(gist("Progress: resolved 1\n ERR_PNPM_NO_MATCHING_VERSION  No matching version\n\nThis error happened while installing"), "ERR_PNPM_NO_MATCHING_VERSION  No matching version · This error happened while installing");
    }

    #[test]
    fn how_the_packages_stand() {
        let v = "0.2.1-rc.2";
        let both = || [Some(v.to_string()), Some(v.to_string())];
        assert_eq!(packages_state(&[both()], v), "ok");
        assert_eq!(packages_state(&[both(), both()], v), "ok");
        assert_eq!(packages_state(&[both(), [Some(v.to_string()), None]], v), "missing");
        assert_eq!(packages_state(&[[Some("0.2.0-rc.2".into()), Some(v.into())]], v), "version");
        assert_eq!(packages_state(&[], v), "missing");
    }

    #[test]
    fn the_app_carries_the_bridge_or_not() {
        let with = json!({ "files": { "dsh": { "files": { "node_modules": { "files": { "@deepseek-ai": { "files": { "dsh-hooks-claude-code": { "files": {} }, "dsh-hook-protocol": { "files": {} }, "dsh": { "files": {} } } } } } } } } });
        assert!(names_bridge(&with));
        let without = json!({ "files": { "dsh": { "files": { "node_modules": { "files": { "@deepseek-ai": { "files": { "dsh": { "files": {} } } } } } } } } });
        assert!(!names_bridge(&without));
        assert!(!names_bridge(&json!({})));
        // An archive as asar writes one: the index after a 16-byte pickle header.
        let dir = std::env::temp_dir().join(format!("wakuwaku-asar-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let index = with.to_string().into_bytes();
        let mut bytes = Vec::new();
        bytes.extend(4u32.to_le_bytes());
        bytes.extend((index.len() as u32 + 8).to_le_bytes());
        bytes.extend((index.len() as u32 + 4).to_le_bytes());
        bytes.extend((index.len() as u32).to_le_bytes());
        bytes.extend(&index);
        fs::write(dir.join("app.asar"), &bytes).unwrap();
        assert_eq!(bundled_in_asar(&dir.join("app.asar")), Some(true));
        assert_eq!(bundled_in_asar(&dir.join("none.asar")), None);
        let _ = fs::remove_dir_all(&dir);
    }

    // With this computer's DSH desktop app: its pnpm run for real against a
    // registry no one answers, in a profile of the test's own; nothing is
    // downloaded. The profile's files as they were afterwards, the reason
    // told. `cargo test -- --ignored` where DSH is installed.
    #[test]
    #[ignore]
    fn a_failed_install_leaves_the_profile_as_it_was() {
        let runtime = app_dir().and_then(|app| runtime_in(&app)).expect("DSH's desktop app, its Node and pnpm");
        let profile = std::env::temp_dir().join(format!("wakuwaku-dsh-pnpm-{}", std::process::id())).join("desktop");
        let _ = fs::remove_dir_all(&profile);
        fs::create_dir_all(&profile).unwrap();
        let manifest = "{\n  \"name\": \"dsh-profile-desktop\",\n  \"private\": true,\n  \"dependencies\": {}\n}\n";
        let workspace = "packages:\n  - .\n\nnodeLinker: hoisted\nautoInstallPeers: false\n";
        fs::write(profile.join("package.json"), manifest).unwrap();
        fs::write(profile.join("pnpm-workspace.yaml"), workspace).unwrap();
        let started = Instant::now();
        let failure = install(&runtime, &profile, "0.2.1-rc.2", Some("http://127.0.0.1:9/")).unwrap_err();
        eprintln!("{failure:?} after {:?}", started.elapsed());
        assert_eq!(failure.kind, "registry", "{failure:?}");
        assert_eq!(fs::read_to_string(profile.join("package.json")).unwrap(), manifest);
        assert_eq!(fs::read_to_string(profile.join("pnpm-workspace.yaml")).unwrap(), workspace);
        assert!(!profile.join("pnpm-lock.yaml").exists() && !profile.join("lock").exists());
        // No backups of its own left beside them either.
        assert!(!profile.join("package.json.wakuwaku.bak").exists() && !profile.join("pnpm-workspace.yaml.wakuwaku.bak").exists());
        let _ = fs::remove_dir_all(profile.parent().unwrap());
    }

    #[test]
    fn profiles_and_their_packages() {
        let home = std::env::temp_dir().join(format!("wakuwaku-dsh-profiles-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        for name in ["web", "desktop", "node_modules", "empty"] {
            fs::create_dir_all(home.join("profiles").join(name)).unwrap();
        }
        for name in ["web", "desktop"] {
            fs::write(home.join("profiles").join(name).join("package.json"), r#"{"name":"p","dependencies":{"x":"1"}}"#).unwrap();
        }
        fs::write(home.join("profiles").join("node_modules").join("package.json"), "{}").unwrap();
        let found = profiles(&home);
        assert_eq!(found.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect::<Vec<_>>(), ["desktop", "web"]);
        let desktop = &found[0];
        let pkg = desktop.join("node_modules").join("@deepseek-ai").join("dsh-hook-protocol");
        fs::create_dir_all(&pkg).unwrap();
        fs::write(pkg.join("package.json"), r#"{"name":"@deepseek-ai/dsh-hook-protocol","version":"0.2.1-rc.2"}"#).unwrap();
        assert_eq!(installed_version(desktop, PROTOCOL_PACKAGE).as_deref(), Some("0.2.1-rc.2"));
        assert_eq!(installed_version(desktop, HOOKS_PACKAGE), None);
        assert_eq!(found_in(&found), vec![[None, Some("0.2.1-rc.2".to_string())], [None, None]]);
        assert_eq!(declared(desktop), vec!["x".to_string()]);
        // The desktop app's lock: taken, refused while held by a live process, given back.
        let lock = ProfileLock::take(desktop).unwrap();
        assert_eq!(ProfileLock::take(desktop).err().map(|f| f.kind), Some("busy"));
        drop(lock);
        assert!(!desktop.join("lock").exists());
        // One left by a process gone: taken over.
        fs::write(desktop.join("lock"), "4294967290\n").unwrap();
        drop(ProfileLock::take(desktop).unwrap());
        // The files kept and put back as they were (one that was not there, gone again).
        let kept = Kept::of(desktop);
        fs::write(desktop.join("package.json"), "changed").unwrap();
        fs::write(desktop.join("pnpm-lock.yaml"), "new").unwrap();
        kept.put_back();
        assert_eq!(fs::read_to_string(desktop.join("package.json")).unwrap(), r#"{"name":"p","dependencies":{"x":"1"}}"#);
        assert!(!desktop.join("pnpm-lock.yaml").exists());
        let _ = fs::remove_dir_all(&home);
    }
}
