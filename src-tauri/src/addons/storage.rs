// Third-party plugins (addons/mod.rs). A plugin's storage.
use super::*;

// --- Storage -----------------------------------------------------------------------------

pub(super) static STORES: LazyLock<Lock<HashMap<PathBuf, Map<String, Value>>>> = LazyLock::new(|| Lock::new("addons::storage::STORES", HashMap::new()));

pub(super) fn stores() -> crate::lock::Guard<'static, HashMap<PathBuf, Map<String, Value>>> {
    STORES.lock().unwrap_or_else(|e| e.into_inner())
}

pub(super) fn store_file(dir: &Path, id: &str) -> PathBuf {
    dir.join("plugin-data").join(format!("{id}.json"))
}

// A plugin's storage (waku.storage): get, set, remove a key, or all of it;
// a JSON object in its own file, 1 MB at most, kept across updates.
pub fn storage(dir: &Path, id: &str, op: &str, key: Option<&str>, value: Option<Value>) -> Result<Value, String> {
    let file = store_file(dir, id);
    let mut all = stores();
    let store = all.entry(file.clone()).or_insert_with(|| std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default());
    let key = || key.filter(|k| (1..=200).contains(&k.chars().count()) && !k.chars().any(char::is_control)).ok_or("a key: some words, 200 at most".to_string());
    match op {
        "get" => Ok(store.get(key()?).cloned().unwrap_or(Value::Null)),
        "all" => Ok(Value::Object(store.clone())),
        "set" | "remove" => {
            let mut next = store.clone();
            let k = key()?.to_string();
            match value.filter(|_| op == "set") {
                Some(v) => next.insert(k, v),
                None => next.remove(&k),
            };
            let text = serde_json::to_string(&next).map_err(|e| e.to_string())?;
            if text.len() > STORE_MAX {
                return Err("storage full: 1 MB per plugin".into());
            }
            std::fs::create_dir_all(file.parent().unwrap_or(dir)).map_err(|e| e.to_string())?;
            write_whole(&file, text.as_bytes()).map_err(|e| e.to_string())?;
            *store = next;
            Ok(Value::Null)
        }
        _ => Err(format!("{op}: get, set, remove or all")),
    }
}

