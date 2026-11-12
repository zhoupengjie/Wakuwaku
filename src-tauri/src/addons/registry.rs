// Third-party plugins (addons/mod.rs). Which are here: the registry.
use super::*;

// --- Which are here: the registry -------------------------------------------------------

// A folder in plugins/: its manifest read (or why not), as of its file's time and size.
pub(super) struct Found {
    pub(super) manifest: Result<Manifest, String>,
    pub(super) stamp: Option<(SystemTime, u64)>,
}

#[derive(Default)]
pub struct Registry {
    pub(super) scanned: bool,
    pub(super) found: BTreeMap<String, Found>,
    // installed.json: { id: { source, reviewed, sha256, version, grantedPerms, installedAt } }.
    pub(super) records: Map<String, Value>,
}

// Read inside other locks (Shared::widgets_view asks it whose each widget
// is), and nothing taken inside it: what runs under it reads files and
// the registry alone (lock.rs LEAF).
pub(super) static REGISTRY: LazyLock<crate::lock::RwLock<Registry>> = LazyLock::new(|| crate::lock::RwLock::ranked("addons::registry::REGISTRY", crate::lock::LEAF, Registry::default()));

// One lock per package id, held for the whole of an install, update or
// uninstall of it (finding 8): a file install, a market install or update,
// a pack install, and an uninstall of the same id never run at once and
// leave its folder or record half-written. Keyed by id alone across kinds:
// conservative, so even a pet and a plugin of one id do not race. Each
// is taken first, with no other lock held, and is held around work that
// takes ordinary ones (the registry, Shared's): ranked below them (lock.rs).
// Never two at once: no install holds one id's and waits for another's.
pub(super) static LOCKS: LazyLock<Lock<HashMap<String, Arc<Lock<()>>>>> = LazyLock::new(|| Lock::new("addons::registry::LOCKS", HashMap::new()));

pub(crate) fn with_id_lock<T>(id: &str, f: impl FnOnce() -> T) -> T {
    let lock = LOCKS.lock().unwrap_or_else(|e| e.into_inner()).entry(id.to_string()).or_insert_with(|| Arc::new(Lock::ranked("addons::registry::LOCKS id", crate::lock::OUTER, ()))).clone();
    let _held = lock.lock().unwrap_or_else(|e| e.into_inner());
    f()
}

pub(super) fn stamp_of(path: &Path) -> Option<(SystemTime, u64)> {
    let m = std::fs::metadata(path).ok()?;
    Some((m.modified().ok()?, m.len()))
}

pub(super) fn plugins_dir(dir: &Path) -> PathBuf {
    dir.join("plugins")
}

// The id in a ".old-<id>-<n>" folder's name (<n> the time it was set aside);
// None for any other name.
pub(super) fn old_id_of(name: &str) -> Option<String> {
    let (id, n) = name.strip_prefix(".old-")?.rsplit_once('-')?;
    (is_id(id) && !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())).then(|| id.to_string())
}

// What an install, update or uninstall cut short left in a package folder
// (plugins, pets, wallpapers): a `.staging-<id>-<n>` (an incomplete new
// copy) thrown away; a `.old-<id>-<n>` (the copy moved aside between two
// renames) thrown away when `<id>` is there again, or renamed back to `<id>`
// when it is not — so a swap stopped after the move-aside but before the
// move-in does not leave the only copy in a hidden folder a scan ignores.
pub(crate) fn clean_interrupted(root: &Path) {
    for e in std::fs::read_dir(root).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !e.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        if name.starts_with(".staging-") {
            let _ = std::fs::remove_dir_all(e.path());
        } else if let Some(id) = old_id_of(&name) {
            let target = root.join(&id);
            if target.exists() || rename_soon(&e.path(), &target).is_err() {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
}

pub(super) fn records_file(dir: &Path) -> PathBuf {
    plugins_dir(dir).join("installed.json")
}

// A folder's manifest read, and the files it names there.
pub(super) fn read_manifest(folder: &Path, name: &str) -> Result<Manifest, String> {
    let file = folder.join("manifest.json");
    let len = std::fs::metadata(&file).map_err(|_| "no manifest.json".to_string())?.len();
    if len > MANIFEST_MAX {
        return Err("manifest.json over 64 KB".into());
    }
    let text = std::fs::read_to_string(&file).map_err(|e| format!("manifest.json: {e}"))?;
    let v: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("manifest.json: {e}"))?;
    let m = manifest_of(&v)?;
    if m.id != name {
        return Err(format!("its folder is {name}, its id {}", m.id));
    }
    if m.runs() {
        for f in m.files_named() {
            if !folder.join(f).is_file() {
                return Err(format!("{f}: not in its folder"));
            }
        }
    }
    Ok(m)
}

impl Registry {
    // The folders looked at again: a manifest new or changed read, one gone
    // forgotten; installed.json too. The ids gone since (their folder taken
    // away by hand), whose places and settings go with them (sync).
    pub(super) fn refresh(&mut self, dir: &Path) -> (bool, Vec<String>) {
        let root = plugins_dir(dir);
        let first = !std::mem::replace(&mut self.scanned, true);
        // An install, update or removal cut short before: what it left put
        // right, the only copy kept where one was left aside (clean_interrupted).
        if first {
            clean_interrupted(&root);
        }
        let mut changed = first;
        let mut seen = HashSet::new();
        for entry in std::fs::read_dir(&root).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !entry.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            if !is_id(&name) {
                continue;
            }
            seen.insert(name.clone());
            let stamp = stamp_of(&entry.path().join("manifest.json"));
            if self.found.get(&name).is_some_and(|f| f.stamp == stamp && stamp.is_some()) {
                continue;
            }
            let manifest = read_manifest(&entry.path(), &name);
            self.found.insert(name, Found { manifest, stamp });
            changed = true;
        }
        // Folders gone since the last look, whether or not they have a record
        // (one deleted by hand without an installed.json: finding 3's scenario
        // B): their places and settings are cleaned the same (forget).
        let vanished: Vec<String> = self.found.keys().filter(|id| !seen.contains(*id)).cloned().collect();
        self.found.retain(|id, _| seen.contains(id));
        changed |= !vanished.is_empty();
        let records: Map<String, Value> = std::fs::read_to_string(records_file(dir)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        if records != self.records {
            self.records = records;
            changed = true;
        }
        let mut gone: Vec<String> = self.records.keys().filter(|id| !self.found.contains_key(*id)).cloned().collect();
        for id in vanished {
            if !gone.contains(&id) {
                gone.push(id);
            }
        }
        (changed, gone)
    }

    pub(super) fn manifest(&self, id: &str) -> Option<&Manifest> {
        self.found.get(id).and_then(|f| f.manifest.as_ref().ok())
    }

    // The widget's plugin: `<id>` or `<id>_<sub>`, its folder here.
    pub(super) fn owner_of(&self, widget: &str) -> Option<String> {
        let id = widget.split_once('_').map_or(widget, |(id, _)| id);
        self.found.contains_key(id).then(|| id.to_string())
    }

    // A "plugins" setting's entry for one here (None: no such folder): "on",
    // and each value by its manifest's field; one whose manifest will not do
    // (or waits for a newer API) keeps what it had, as long as it is text,
    // a number or a switch.
    pub(super) fn conf_ok(&self, id: &str, conf: &Value) -> Option<bool> {
        let found = self.found.get(id)?;
        let Some(conf) = conf.as_object() else { return Some(false) };
        Some(conf.iter().all(|(k, v)| self.key_ok(found, k, v)))
    }

    // Whether one key of a plugin's entry is kept: "on" a switch; a value by
    // its manifest's field (one of a newer API, or one that will not read,
    // keeps what it had as long as it is text, a number or a switch).
    pub(super) fn key_ok(&self, found: &Found, k: &str, v: &Value) -> bool {
        let scalar = |v: &Value| v.is_boolean() || v.as_f64().is_some() || v.as_str().is_some_and(|t| t.chars().count() <= 300);
        match (&found.manifest, k) {
            (_, "on") => v.is_boolean(),
            (Ok(m), _) if m.api <= API => m.field(k).is_some_and(|f| f.accepts(v)),
            _ => k.len() <= 32 && k.bytes().all(|b| b.is_ascii_alphanumeric()) && scalar(v),
        }
    }

    // A third-party plugin's entry cleaned: its keys that will not do dropped
    // (finding 3); None when no such folder is here (the id stale).
    pub(super) fn clean_conf(&self, id: &str, conf: &Map<String, Value>) -> Option<Map<String, Value>> {
        let found = self.found.get(id)?;
        Some(conf.iter().filter(|(k, v)| self.key_ok(found, k, v)).map(|(k, v)| (k.clone(), v.clone())).collect())
    }

    pub(super) fn record(&self, id: &str) -> &Value {
        self.records.get(id).unwrap_or(&Value::Null)
    }

    // What the plugin was granted when it went in (installed.json
    // grantedPerms): a market install's index permissions, a file install's
    // the confirm layer's (its manifest's), a newer-API one's what little it
    // said of itself. A record from before this was kept, or none at all (a
    // folder copied in by hand), is read as granting its current manifest's,
    // so it is not asked out of the blue (migrate_grants writes it once).
    pub(super) fn granted(&self, id: &str) -> Perms {
        match self.record(id).get("grantedPerms") {
            Some(g) => perms_of(g).unwrap_or_default(),
            None => self.manifest(id).map(|m| m.perms.clone()).unwrap_or_default(),
        }
    }

    // What a running plugin's manifest asks for beyond what was granted: none
    // means it may run; else it waits for the person to allow it (needsApproval).
    pub(super) fn needs_approval(&self, id: &str, m: &Manifest) -> Vec<String> {
        if m.runs() {
            crate::market::new_perms(&self.granted(id), &m.perms)
        } else {
            Vec::new()
        }
    }

    // What a running plugin may do: what its manifest asks for, as far as it
    // was granted. Its asks are held to this (call), not to its manifest, so
    // a manifest that came to ask for more gets nothing more until the person
    // allows it (approve), whether or not it was stopped yet (sync, wanted).
    pub(super) fn allowed(&self, id: &str) -> Option<Perms> {
        let m = &self.manifest(id)?.perms;
        let g = self.granted(id);
        Some(Perms {
            net: m.net.iter().filter(|h| host_allowed(&g.net, h)).cloned().collect(),
            agents: m.agents && g.agents,
            hardware: m.hardware && g.hardware,
            notify: m.notify && g.notify,
        })
    }

    // What the manifest's time says: a manifest changed (a plugin edited in
    // place) starts its frame again.
    pub(super) fn rev(&self, id: &str) -> String {
        let at = self.found.get(id).and_then(|f| f.stamp).and_then(|(t, _)| t.duration_since(SystemTime::UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis());
        let m = self.manifest(id).map_or("", |m| m.version.as_str());
        format!("{m}:{at}:{}", self.record(id)["installedAt"])
    }
}

// The folders looked at now (as she starts: settings and layouts naming a
// plugin are checked against them).
pub fn start(sh: &Arc<Shared>) {
    let (_, gone) = REGISTRY.write().unwrap_or_else(|e| e.into_inner()).refresh(&sh.dir);
    let found = REGISTRY.read().unwrap_or_else(|e| e.into_inner()).found.len();
    if found > 0 {
        sh.log(&format!("addons: {found} in her plugins folder"));
    }
    for id in gone {
        forget(sh, &id, "its folder is gone");
    }
    migrate_grants(sh);
    prune_plugins(sh);
}

// A record from before grantedPerms was kept (an older release) is read as
// granting its current manifest's permissions, written once, so the person
// is not asked about a plugin they already live with — only about one whose
// manifest later asks for more (finding 1's migration).
pub(super) fn migrate_grants(sh: &Shared) {
    let todo: Vec<(String, Value)> = read(|r| {
        r.found
            .iter()
            .filter_map(|(id, f)| {
                let m = f.manifest.as_ref().ok()?;
                let rec = r.record(id);
                (rec.get("source").is_some() && rec.get("grantedPerms").is_none()).then(|| (id.clone(), m.perms.json()))
            })
            .collect()
    });
    if todo.is_empty() {
        return;
    }
    records_write(&sh.dir, |r| {
        for (id, perms) in &todo {
            if let Some(rec) = r.get_mut(id).and_then(Value::as_object_mut) {
                rec.insert("grantedPerms".into(), perms.clone());
            }
        }
    });
}

// The stored "plugins" setting cleaned (plugins::clean): a value a changed
// or removed manifest left stale dropped, written only when it differs — so
// it never loops (clean is idempotent). At start and after an install
// (a refresh may have changed what a plugin's manifest asks for).
pub(crate) fn prune_plugins(sh: &Shared) {
    let stored = sh.config().plugins.clone();
    let cleaned = crate::plugins::clean(&stored);
    if cleaned != stored {
        shared(&sh.app).change(json!({ "plugins": cleaned }));
    }
}

pub(super) fn read<T>(f: impl FnOnce(&Registry) -> T) -> T {
    f(&REGISTRY.read().unwrap_or_else(|e| e.into_inner()))
}

// Which plugin a widget is of: one installed here, by its id or <id>_<sub>.
pub fn owner_of(widget: &str) -> Option<String> {
    read(|r| r.owner_of(widget))
}

// Whether a plugin of that id is here (its manifest good or not).
pub fn is_known(id: &str) -> bool {
    read(|r| r.found.contains_key(id))
}

// A "plugins" entry for a plugin here, checked by its manifest; None: none here.
pub fn is_conf_ok(id: &str, conf: &Value) -> Option<bool> {
    read(|r| r.conf_ok(id, conf))
}

// A third-party plugin's "plugins" entry cleaned (plugins::clean, finding 3):
// the keys that will not do dropped; None when the id is of no folder here.
pub fn clean_conf(id: &str, conf: &Map<String, Value>) -> Option<Map<String, Value>> {
    read(|r| r.clean_conf(id, conf))
}

pub(super) fn records_write(dir: &Path, f: impl FnOnce(&mut Map<String, Value>)) {
    let mut reg = REGISTRY.write().unwrap_or_else(|e| e.into_inner());
    let mut records = std::fs::read_to_string(records_file(dir)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| reg.records.clone());
    f(&mut records);
    let _ = std::fs::create_dir_all(plugins_dir(dir));
    let _ = write_whole(&records_file(dir), serde_json::to_string_pretty(&records).unwrap_or_default().as_bytes());
    reg.records = records;
}

// A file written whole: beside it first, then put in its place.
pub(crate) fn write_whole(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

