// The files the agents' settings are in, as all of them read and write
// them: read as last read, written backed up once beside themselves,
// given back with our part taken out; and this program, as they run it.

use super::*;
use crate::lock::Lock;

// The copy write_json keeps of a settings file (made once, beside it).
pub fn backup_of(file: &Path) -> PathBuf {
    PathBuf::from(format!("{}.wakuwaku.bak", file.display()))
}

// A path as the person reads it: ~ for the home folder when it is under it.
pub fn shown_path(file: &Path) -> String {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from);
    match home.as_deref().filter(|h| !h.as_os_str().is_empty()).and_then(|h| file.strip_prefix(h).ok()) {
        Some(rest) => format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display()),
        None => file.display().to_string(),
    }
}

// A JSON settings file: {} when there is none, Err when unreadable.
// A file as last read, kept while its size and time stay (a look at the
// file's metadata, not its contents): the settings' snapshot asks for
// Claude Code's settings.json three times and Codex's hooks.json twice,
// up to four times a second while the settings are open (2026-10-13).
type Stamp = Option<(u64, std::time::SystemTime)>;
static READ: Lock<Option<std::collections::HashMap<PathBuf, (Stamp, Result<Value, String>)>>> = Lock::new("connection::file::READ", None);

pub(super) fn read_json(file: &Path) -> Result<Value, String> {
    let stamp: Stamp = fs::metadata(file).ok().map(|m| (m.len(), m.modified().unwrap_or(std::time::UNIX_EPOCH)));
    let mut read = READ.lock().unwrap_or_else(|e| e.into_inner());
    let read = read.get_or_insert_with(std::collections::HashMap::new);
    if let Some((at, value)) = read.get(file) {
        if *at == stamp {
            return value.clone();
        }
    }
    let value = read_json_now(file);
    read.insert(file.to_path_buf(), (stamp, value.clone()));
    value
}

pub(super) fn read_json_now(file: &Path) -> Result<Value, String> {
    match fs::read_to_string(file) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(e.to_string()),
    }
}

// Write a settings file, backed up once beside itself.
pub(super) fn write_json(file: &Path, value: &Value) -> Result<(), String> {
    let backup = backup_of(file);
    if file.exists() && !backup.exists() {
        fs::copy(file, &backup).map_err(|e| e.to_string())?;
    }
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    let written = fs::write(file, format!("{text}\n")).map_err(|e| e.to_string());
    forget_read(file);
    written
}

// As last read no longer: read again next time, whatever its stamp.
pub(super) fn forget_read(file: &Path) {
    if let Some(read) = READ.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        read.remove(file);
    }
}

// An agent's file with our part taken out: written back with no backup made
// (what is there is ours alone; a backup then held our hooks, seen
// 2026-10-16), or, when she made the file (no backup beside it: there was
// nothing of the person's to keep when she first wrote it) and nothing is
// left in it, taken away, as it was before.
pub(super) fn write_back(file: &Path, value: &Value) -> Result<(), String> {
    if value.as_object().is_some_and(|o| o.is_empty()) && !backup_of(file).exists() {
        let gone = fs::remove_file(file).map_err(|e| e.to_string());
        forget_read(file);
        return gone;
    }
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    let written = fs::write(file, format!("{text}\n")).map_err(|e| e.to_string());
    forget_read(file);
    written
}

// This program, as the hook command starts it.
pub fn launch_command() -> String {
    crate::data::exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
}

// A path as Windows compares them: one slash, any case.
pub(crate) fn same_path(p: &str) -> String {
    if cfg!(windows) { p.replace('\\', "/").to_lowercase() } else { p.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A file she made, left empty by a remove, goes again, with no backup
    // made of what was ours; one the person had (its backup beside it) is
    // written back, its backup kept as it was.
    #[test]
    fn a_remove_leaves_no_file_of_ours_and_no_backup_of_ours() {
        let dir = std::env::temp_dir().join(format!("wakuwaku-writeback-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let made = dir.join("hooks.json");
        write_json(&made, &json!({ "hooks": { "Stop": [] } })).unwrap();
        assert!(!backup_of(&made).exists(), "nothing of the person's to back up");
        write_back(&made, &json!({})).unwrap();
        assert!(!made.exists() && !backup_of(&made).exists());
        // Theirs: backed up as she first wrote it, written back on a remove.
        let theirs = dir.join("config.json");
        fs::write(&theirs, "{ \"model\": \"m\" }").unwrap();
        write_json(&theirs, &json!({ "model": "m", "hooks": {} })).unwrap();
        write_back(&theirs, &json!({ "model": "m" })).unwrap();
        assert_eq!(read_json_now(&theirs).unwrap(), json!({ "model": "m" }));
        assert_eq!(fs::read_to_string(backup_of(&theirs)).unwrap(), "{ \"model\": \"m\" }");
        // Emptied, but the person's once: kept, as {}.
        write_back(&theirs, &json!({})).unwrap();
        assert_eq!(read_json_now(&theirs).unwrap(), json!({}));
        let _ = fs::remove_dir_all(&dir);
    }
}
