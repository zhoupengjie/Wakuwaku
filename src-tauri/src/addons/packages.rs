// Third-party plugins (addons/mod.rs). Packages: inspected, installed, taken away.
use super::*;

// --- Packages: inspected, installed, taken away -----------------------------------------

// Why a package will not go in: what (zip: not one, or damaged; big; files:
// too many; path: a name out of its folder; type: a file of a type not
// allowed; program: a program, by its first bytes; manifest; read: the
// file; a pack's (packs.rs): packType, a file no pack holds; pet, its
// pet.json or sheet missing or wrong; pictures, none at its top; clash, a
// pet or folder of that name there already that is no pack's), the file in
// it it is about, and the words.
#[derive(Debug, PartialEq)]
pub struct Refused {
    pub reason: &'static str,
    pub file: Option<String>,
    pub detail: String,
}

pub(crate) fn refused(reason: &'static str, file: Option<&str>, detail: impl Into<String>) -> Refused {
    Refused { reason, file: file.map(str::to_string), detail: detail.into() }
}

impl Refused {
    pub(crate) fn json(&self) -> Value {
        json!({ "reason": self.reason, "file": self.file, "detail": self.detail })
    }
}

// A package checked: its manifest, its files (the zip's entry and its path
// in the folder), its SHA-256.
pub struct Package {
    pub manifest: Manifest,
    pub(crate) bytes: Vec<u8>,
    pub(crate) files: Vec<(zip::Entry, String)>,
    pub sha256: String,
}

// A zip read and checked whole, nothing written: its names, the types of
// its files, their number and sizes, its manifest (at its root, after the
// one folder holding all, if there is one), the files the manifest names,
// and every file's first bytes, so a program called something else is
// refused all the same.
pub fn open_package(bytes: Vec<u8>) -> Result<Package, Refused> {
    if bytes.len() as u64 > PACK_ZIP {
        return Err(refused("big", None, "over 50 MB"));
    }
    let limits = zip::Limits { entries: FILES * 2 + 100, file: PACK_ALL, all: PACK_ALL };
    let entries = zip::entries_within(&bytes, &limits).map_err(|e| if e == "too big unpacked" { refused("big", None, "over 200 MB unpacked") } else { refused("zip", None, e) })?;
    let mut named = Vec::new();
    for e in entries.into_iter().filter(|e| !is_litter(&e.name)) {
        let parts = zip::safe(&e.name).ok_or_else(|| refused("path", Some(&e.name), "a name out of its folder"))?;
        let folder = e.is_folder();
        named.push((e, parts, folder));
    }
    let strip = zip::strip_of(&named.iter().map(|(_, p, f)| (p.clone(), *f)).collect::<Vec<_>>());
    let files: Vec<(zip::Entry, String)> = named.into_iter().filter(|(_, p, folder)| !folder && p.len() > strip).map(|(e, p, _)| (e, p[strip..].join("/"))).collect();
    if files.len() > FILES {
        return Err(refused("files", None, format!("{} files, {FILES} at most", files.len())));
    }
    // Two names the same but for case (manifest.json and Manifest.json): on
    // Windows one overwrites the other, so the consent could show one
    // manifest's permissions while another runs. Refused, as the market's
    // check does (finding 1; tools/market/package.js path-duplicate).
    let mut lower: HashMap<String, String> = HashMap::new();
    for (_, path) in &files {
        if let Some(first) = lower.insert(path.to_ascii_lowercase(), path.clone()) {
            return Err(refused("dup", Some(path), format!("the same name as {first} but for case (Windows ignores case)")));
        }
    }
    for (_, path) in &files {
        if !EXTENSIONS.contains(&ext_of(path).as_str()) {
            return Err(refused("type", Some(path), "a type of file a plugin may not hold"));
        }
    }
    let (me, _) = files.iter().find(|(_, p)| p == "manifest.json").ok_or_else(|| refused("manifest", None, "no manifest.json at its root"))?;
    if me.size > MANIFEST_MAX {
        return Err(refused("manifest", Some("manifest.json"), "over 64 KB"));
    }
    let text = zip::bytes_of(&bytes, me).map_err(|e| refused("zip", Some("manifest.json"), e))?;
    let v: Value = serde_json::from_slice(strip_bom(&text)).map_err(|e| refused("manifest", Some("manifest.json"), e.to_string()))?;
    let manifest = manifest_of(&v).map_err(|e| refused("manifest", Some("manifest.json"), e))?;
    let (zipped, all) = manifest.kind.limits();
    if bytes.len() as u64 > zipped {
        return Err(refused("big", None, format!("over {} MB", zipped >> 20)));
    }
    if files.iter().map(|(e, _)| e.size).sum::<u64>() > all {
        return Err(refused("big", None, format!("over {} MB unpacked", all >> 20)));
    }
    if manifest.runs() {
        for f in manifest.files_named() {
            if !files.iter().any(|(_, p)| p == f) {
                return Err(refused("manifest", Some(f), "named in its manifest, not in the package"));
            }
        }
    }
    for (e, path) in &files {
        let data = zip::bytes_of(&bytes, e).map_err(|err| refused("zip", Some(path), err))?;
        if PROGRAMS.iter().any(|m| data.starts_with(m)) {
            return Err(refused("program", Some(path), "a program"));
        }
    }
    // A pack: its kind's files alone (pictures and words), and what its kind needs.
    if manifest.kind != Kind::Plugin {
        crate::packs::check(&manifest, &files, &bytes)?;
    }
    let sha256 = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    Ok(Package { manifest, bytes, files, sha256 })
}

pub(crate) fn strip_bom(b: &[u8]) -> &[u8] {
    b.strip_prefix(b"\xef\xbb\xbf").unwrap_or(b)
}

pub(super) fn read_package(path: &Path) -> Result<Package, Refused> {
    let len = std::fs::metadata(path).map_err(|e| refused("read", None, e.to_string()))?.len();
    if len > PACK_ZIP {
        return Err(refused("big", None, "over 50 MB"));
    }
    open_package(std::fs::read(path).map_err(|e| refused("read", None, e.to_string()))?)
}

// What a package would do, for the page's confirm (nothing written): its
// id, name, version, author, kind, permissions, whether any is one to ask
// about, the version it replaces (a pack's, the pack of its kind here:
// packs.rs); or why it will not go in.
pub fn summary(dir: &Path, pkg: &Result<Package, Refused>, lang: &str, file: &str) -> Value {
    match pkg {
        Ok(p) => {
            let m = &p.manifest;
            let replaces = if m.kind == Kind::Plugin { read(|r| r.manifest(&m.id).map(|old| old.version.clone())) } else { crate::packs::version_here(dir, m.kind, &m.id) };
            json!({
                "ok": true,
                "file": file,
                "id": m.id,
                "name": m.name.get(lang),
                "version": m.version,
                "author": m.author,
                "kind": m.kind.name(),
                // What it asks for, shown whatever its API (finding 7).
                "perms": m.wants.json(),
                "sensitive": m.wants.sensitive(),
                "replaces": replaces,
                "api": m.api,
                "supported": m.api <= API,
                // So the install can refuse a file changed since it was looked
                // at: the page passes this back (finding 12).
                "sha256": p.sha256,
                "refused": null,
            })
        }
        Err(r) => json!({ "ok": false, "file": file, "refused": r.json() }),
    }
}

// Its files written into a folder of their own beside the plugins (one
// beginning with a dot, which no scan takes for one), then put in place of
// the one there: that one moved aside first, and back should the swap
// fail; thrown away once it is done. (A pack's: packs.rs write.)
pub(super) fn unpack(pkg: &Package, root: &Path) -> Result<(), String> {
    let id = &pkg.manifest.id;
    let n = now_ms();
    let staging = root.join(format!(".staging-{id}-{n}"));
    let old = root.join(format!(".old-{id}-{n}"));
    let target = root.join(id);
    let written = (|| -> Result<(), String> {
        std::fs::create_dir_all(&staging).map_err(|e| format!("{}: {e}", staging.display()))?;
        for (e, path) in &pkg.files {
            let to = staging.join(path);
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent).map_err(|err| format!("{path}: {err}"))?;
            }
            std::fs::write(&to, zip::bytes_of(&pkg.bytes, e)?).map_err(|err| format!("{path}: {err}"))?;
        }
        Ok(())
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    let had = target.exists();
    if had {
        if let Err(e) = rename_soon(&target, &old) {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(format!("the copy there could not be moved aside: {e}"));
        }
    }
    if let Err(e) = rename_soon(&staging, &target) {
        if had {
            let _ = rename_soon(&old, &target);
        }
        let _ = std::fs::remove_dir_all(&staging);
        return Err(format!("not put in place: {e}"));
    }
    if had {
        let _ = std::fs::remove_dir_all(&old);
    }
    Ok(())
}

// A folder renamed, tried again a few times: a file in it being read a
// moment (her scheme serving it, an antivirus) keeps Windows from it.
pub(crate) fn rename_soon(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut last = None;
    for _ in 0..10 {
        match std::fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) => last = Some(e),
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(last.unwrap_or_else(|| std::io::Error::other("not renamed")))
}

// A package installed from a file: checked again, then put in (from a
// file: not reviewed). `expect`: the SHA-256 inspect gave the page; a file
// that is no longer that one (swapped since it was looked at and agreed to)
// is refused, so what goes in is what the person saw (finding 12).
pub fn install(sh: &Shared, path: &Path, expect: Option<&str>) -> Result<Value, Refused> {
    let pkg = read_package(path)?;
    if let Some(expect) = expect.filter(|e| !e.is_empty()) {
        if !pkg.sha256.eq_ignore_ascii_case(expect) {
            return Err(refused("sha", None, "the file has changed since it was looked at; look again"));
        }
    }
    let file = path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    put_in(sh, pkg, "file", &path.display().to_string(), &file)
}

// A package checked, unpacked and swapped in, its record kept: where it
// came from (a file, not reviewed; the market, reviewed: market.rs, its
// zip's SHA-256 and size as the index said), the permissions granted as
// it went in (its manifest's). Its settings and storage, if it was here
// before, stay: an update; its frame loads it anew (its record's time in
// its rev). A pack (a pet, wallpapers) goes where its kind goes, marked
// as one (packs.rs put_in). `from`: what the log says it came from;
// `file`: its zip's name.
pub fn put_in(sh: &Shared, pkg: Package, source: &'static str, from: &str, file: &str) -> Result<Value, Refused> {
    if pkg.manifest.kind != Kind::Plugin {
        return crate::packs::put_in(sh, pkg, source, from, file);
    }
    let id = pkg.manifest.id.clone();
    // The id held for the write, so no other install or an uninstall of it runs at the same time (finding 8).
    with_id_lock(&id, || {
        let m = &pkg.manifest;
        let replaces = read(|r| r.manifest(&m.id).map(|old| old.version.clone()));
        unpack(&pkg, &plugins_dir(&sh.dir)).map_err(|e| refused("write", None, e))?;
        // The permissions granted: the manifest's, which the page's confirm
        // showed for a file install and which the market verified against its
        // index item for a market install (finding 1, finding 13). Any widgets
        // the old copy noted kept, so its places are still cleaned on uninstall.
        let widgets = read(|r| r.record(&m.id)["widgets"].clone());
        let rec = json!({ "source": source, "reviewed": source == "market", "sha256": pkg.sha256, "version": m.version, "grantedPerms": m.perms.json(), "installedAt": now_ms(), "widgets": widgets });
        records_write(&sh.dir, |r| {
            r.insert(m.id.clone(), rec);
        });
        // A new copy: what was said of the old one forgotten, and one stopped tried again.
        {
            let mut r = run();
            r.said.remove(&m.id);
            r.stopped.remove(&m.id);
        }
        sh.log(&format!("addons: {} {} installed from {from}{}", m.id, m.version, replaces.as_ref().map(|v| format!(", in place of {v}")).unwrap_or_default()));
        if crate::plugins::sync(sh) {
            sh.redraw_widgets();
        }
        // An update may ask for other settings than the copy before: any now stale stored value pruned.
        prune_plugins(sh);
        let mut out = summary(&sh.dir, &Ok(pkg), sh.lang(), file);
        out["replaces"] = json!(replaces);
        Ok(out)
    })
}

// Those installed from the market, for its updates (market.rs): each id,
// its kind, its name in her language, the version here and the
// permissions granted as it went in (none recorded: those it asks for
// now; a pack's, none). The packs' are packs.rs from_market.
pub struct FromMarket {
    pub id: String,
    pub kind: Kind,
    pub name: String,
    pub version: String,
    pub granted: Perms,
}

pub fn from_market(lang: &str) -> Vec<FromMarket> {
    read(|r| {
        r.found
            .iter()
            .filter(|(id, _)| r.record(id)["source"] == "market")
            .filter_map(|(id, f)| {
                let m = f.manifest.as_ref().ok()?;
                let granted = perms_of(&r.record(id)["grantedPerms"]).unwrap_or_else(|_| m.perms.clone());
                Some(FromMarket { id: id.clone(), kind: Kind::Plugin, name: m.name.get(lang).to_string(), version: m.version.clone(), granted })
            })
            .collect()
    })
}

// One of a plugin's widgets: its own id, or `<id>_<sub>`.
pub(super) fn is_widget_of(id: &str, widget: &str) -> bool {
    widget == id || widget.strip_prefix(id).is_some_and(|rest| rest.starts_with('_') && rest.len() > 1)
}

// The settings without a plugin: its own entry, and it (or its widgets) out
// of every place: the taskbar's runs (a group of it and one other left as
// that one alone), the desktop (a stack likewise; a board keeps its other
// tiles), her capsule, the widgets' order, switches and sizes, and the
// combos kept (one with nothing left, gone). Only the settings it was in.
// `widgets`: the ids of the widgets this plugin actually made (its own id and
// the subs recorded or up now), so a person's own script widget that merely
// begins like the plugin's (`cpu_temp` next to a `cpu` plugin: finding 11)
// is left where it is.
pub fn without_plugin(settings: &Map<String, Value>, id: &str, widgets: &HashSet<String>) -> Map<String, Value> {
    let is_sub = |of: &str| of == id || widgets.contains(of);
    let mine = |m: &Value| match (m["type"].as_str(), m["of"].as_str()) {
        (Some("plugin"), Some(of)) => of == id,
        (Some("widget"), Some(of)) => is_sub(of),
        _ => false,
    };
    let mut patch = Map::new();
    if let Some(Value::Object(all)) = settings.get("plugins") {
        if all.contains_key(id) {
            let mut all = all.clone();
            all.remove(id);
            patch.insert("plugins".into(), Value::Object(all));
        }
    }
    if let Some(Value::Object(runs)) = settings.get("taskbarLayout") {
        let mut out = Map::new();
        for (run, parts) in runs {
            let parts: Vec<Value> = parts
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|p| match p["items"].as_array() {
                    Some(items) => {
                        let left: Vec<&Value> = items.iter().filter(|m| !mine(m)).collect();
                        match left.len() {
                            0 => None,
                            1 => {
                                let mut alone = json!({ "id": p["id"], "type": left[0]["type"], "of": left[0]["of"], "size": "M" });
                                for k in ["name", "icon"] {
                                    if let Some(v) = left[0].get(k) {
                                        alone[k] = v.clone();
                                    }
                                }
                                Some(alone)
                            }
                            _ => {
                                let mut p = p.clone();
                                p["items"] = json!(left);
                                Some(p)
                            }
                        }
                    }
                    None => (!mine(p)).then(|| p.clone()),
                })
                .collect();
            out.insert(run.clone(), json!(parts));
        }
        if Value::Object(out.clone()) != Value::Object(runs.clone()) {
            patch.insert("taskbarLayout".into(), Value::Object(out));
        }
    }
    if let Some(Value::Array(desk)) = settings.get("deskLayout") {
        let out: Vec<Value> = desk
            .iter()
            .filter_map(|e| {
                let Some(items) = e["items"].as_array() else { return (!mine(e)).then(|| e.clone()) };
                let left: Vec<Value> = items.iter().filter(|m| !mine(m)).cloned().collect();
                if e["type"] == "board" {
                    let mut e = e.clone();
                    e["items"] = json!(left);
                    return Some(e);
                }
                match left.len() {
                    0 => None,
                    1 => {
                        let mut alone = left[0].clone();
                        alone["id"] = e["id"].clone();
                        alone["size"] = e["size"].clone();
                        for k in ["d", "at"] {
                            if let Some(v) = e.get(k) {
                                alone[k] = v.clone();
                            }
                        }
                        Some(alone)
                    }
                    _ => {
                        let mut e = e.clone();
                        e["items"] = json!(left);
                        Some(e)
                    }
                }
            })
            .collect();
        if &out != desk {
            patch.insert("deskLayout".into(), json!(out));
        }
    }
    if let Some(Value::Object(capsule)) = settings.get("capsule") {
        if let Some(Value::Array(members)) = capsule.get("members") {
            let left: Vec<Value> = members
                .iter()
                .filter(|m| {
                    let key = m["key"].as_str().unwrap_or("");
                    key != format!("plugin:{id}") && !key.strip_prefix("widget:").is_some_and(is_sub)
                })
                .cloned()
                .collect();
            if &left != members {
                let mut c = capsule.clone();
                c.insert("members".into(), json!(left));
                patch.insert("capsule".into(), Value::Object(c));
            }
        }
    }
    for key in ["widgetsOff", "widgetOrder"] {
        if let Some(Value::Array(list)) = settings.get(key) {
            let left: Vec<Value> = list.iter().filter(|w| !w.as_str().is_some_and(is_sub)).cloned().collect();
            if &left != list {
                patch.insert(key.into(), json!(left));
            }
        }
    }
    if let Some(Value::Object(sizes)) = settings.get("widgetSize") {
        let left: Map<String, Value> = sizes.iter().filter(|(w, _)| !is_sub(w)).map(|(k, v)| (k.clone(), v.clone())).collect();
        if &left != sizes {
            patch.insert("widgetSize".into(), Value::Object(left));
        }
    }
    if let Some(Value::Array(sets)) = settings.get("boardSets") {
        let out: Vec<Value> = sets
            .iter()
            .filter_map(|s| {
                let left: Vec<Value> = s["items"].as_array().into_iter().flatten().filter(|t| !mine(t)).cloned().collect();
                (!left.is_empty()).then(|| {
                    let mut s = s.clone();
                    s["items"] = json!(left);
                    s
                })
            })
            .collect();
        if &out != sets {
            patch.insert("boardSets".into(), json!(out));
        }
    }
    patch
}

// The widget ids a plugin has made: its own, the subs recorded in
// installed.json (so they are known even when it is not running) and any up
// now; never a person's script widget that only begins like it (finding 11).
pub(super) fn widget_ids_of(sh: &Shared, id: &str) -> HashSet<String> {
    let mut ids: HashSet<String> = read(|r| r.record(id)["widgets"].as_array().into_iter().flatten().filter_map(|w| w.as_str().map(str::to_string)).collect());
    ids.extend(sh.widgets.lock().unwrap().addon_ids(|w| is_widget_of(id, w)));
    ids.insert(id.to_string());
    ids
}

// One widget a plugin made noted in its record (installed.json), so its
// places are cleaned on uninstall even when it is not running. Up to the
// most it may have (WIDGETS); nothing without a record (copied in by hand).
pub(super) fn record_widget(sh: &Shared, id: &str, wid: &str) {
    let known = read(|r| r.record(id)["widgets"].as_array().is_some_and(|a| a.iter().any(|w| w.as_str() == Some(wid))));
    if known {
        return;
    }
    records_write(&sh.dir, |r| {
        if let Some(rec) = r.get_mut(id).and_then(Value::as_object_mut) {
            let list = rec.entry("widgets").or_insert_with(|| json!([])).as_array_mut();
            if let Some(arr) = list {
                if arr.len() < WIDGETS && !arr.iter().any(|w| w.as_str() == Some(wid)) {
                    arr.push(json!(wid));
                }
            }
        }
    });
}

// A plugin's settings and places taken out, its widgets gone, its record
// and words forgotten: as it is taken away, or as its folder was by hand.
pub(super) fn forget(sh: &Shared, id: &str, why: &str) {
    let widgets = widget_ids_of(sh, id);
    let patch = without_plugin(sh.config().map(), id, &widgets);
    if !patch.is_empty() {
        // Through the settings' own way (Arc for change): its pages redrawn.
        let sh = shared(&sh.app);
        sh.change(Value::Object(patch));
    }
    if sh.widgets.lock().unwrap().remove_addons(|w| is_widget_of(id, w)) {
        sh.redraw_widgets();
    }
    records_write(&sh.dir, |r| {
        r.remove(id);
    });
    {
        let mut r = run();
        r.said.remove(id);
        r.stopped.remove(id);
    }
    sh.log(&format!("addons: {id} forgotten ({why})"));
}

// A plugin taken away: out of its settings and places, its folder and its
// storage deleted.
pub fn uninstall(sh: &Shared, id: &str) -> Result<(), String> {
    if !is_id(id) || !(is_known(id) || read(|r| r.records.contains_key(id))) {
        return Err(format!("{id}: not installed"));
    }
    // Any market download or update of it called off, and the id held, so a
    // concurrent install does not put it back after it is taken away (finding 8).
    crate::market::cancel(id);
    with_id_lock(id, || {
        let root = plugins_dir(&sh.dir);
        let folder = root.join(id);
        // Its folder gone first; only then is it forgotten, so a rename that
        // fails (a file held) leaves it installed with its record, not half gone.
        if folder.exists() {
            let aside = root.join(format!(".old-{id}-{}", now_ms()));
            if let Err(e) = rename_soon(&folder, &aside) {
                return Err(format!("its folder could not be removed ({e}); still installed"));
            }
            let _ = std::fs::remove_dir_all(&aside);
        }
        forget(sh, id, "taken away");
        let _ = std::fs::remove_file(store_file(&sh.dir, id));
        stores().remove(&store_file(&sh.dir, id));
        if crate::plugins::sync(sh) {
            sh.redraw_widgets();
        }
        Ok(())
    })
}

