// Resource packs (docs/PLUGINS.md, Packs; docs/design/specs/plugins.md §0
// to §3): a pet, or wallpapers, pictures and words alone, no code; nothing
// of one runs and nothing is asked. Installed as a plugin is (addons.rs
// open_package and put_in: the same zip, checked the same way, from a file
// or from the market), into her pets beside the others (<data>/pets/<id>/,
// found as any pet is: data.rs pets) or into <data>/wallpapers/<id>/, a
// folder of pictures for Windows' slideshow (paper.rs set_folder, as
// 选文件夹… sets one).
//
// A pack is told from a pet downloaded from codex-pets.net or made by hand
// by her mark in its folder, pack.json: its id, kind, version and words,
// where it came from (a file, not reviewed; the market, reviewed), its
// zip's SHA-256, when, its size and how many pictures. A folder of that
// name without her mark is the person's own: a pack of that id is refused,
// and nothing of theirs is written over. A pack's own pack.json is hers.
//
// Written as fetch.rs writes a pet: whole, into a folder beside
// (.staging-<id>-<n>, which no look takes for one), then moved in by one
// rename, the one before moved aside first and thrown away once it is; one
// held (Windows' slideshow on its folder, a sheet open) written over in
// place. Its preview (preview.webp|png|jpg, 64 KB at most) is kept beside
// the folders, <root>/.previews/<id>.<ext>, for the Plugins and Look pages:
// neither one of the slideshow's pictures nor part of the pet. A wallpaper
// pack's folder holds its top level alone (the slideshow takes in a
// folder's sub-folders too).
//
// An update (the same id again, from a file or the market's look) keeps the
// person's choice: the setting `pet` still names it, and the slideshow's
// folder is the same folder. Taken away, the pet in use falls back to the
// default pet (claude-chan; else the first there is; else hers written
// anew from the exe), as she does when her pet is gone as she starts (main.rs
// setup); Windows' slideshow on its folder ends on the picture it shows,
// kept (<data>/wallpapers/.kept/), so the desktop stays as it is.
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::SystemTime;

use serde_json::{json, Value};
use tauri::Manager;

use crate::addons::{self, refused, FromMarket, Kind, Manifest, Package, Perms, Refused, Words};
use crate::{now_ms, shared, zip, Shared};

// Her mark in a pack's folder.
const MARK: &str = "pack.json";
// A pack's picture for the pages, at its top, and where it is kept.
const PREVIEWS: [&str; 3] = ["preview.webp", "preview.png", "preview.jpg"];
const PREVIEW_DIR: &str = ".previews";
const PREVIEW_MAX: u64 = 64 * 1024;
// The files each kind holds, by extension: a pet's pictures; a wallpaper's
// (those docs/PLUGINS.md names); the words beside them.
const PET_FILES: [&str; 4] = ["webp", "png", "jpg", "jpeg"];
const PICTURES: [&str; 4] = ["jpg", "jpeg", "png", "webp"];
const WORDS: [&str; 3] = ["json", "md", "txt"];
// A pet as she reads one (data.rs pets).
const PET_SHEET: &str = "spritesheet.webp";
const PET_JSON: &str = "pet.json";
// The picture the desktop showed from a wallpaper pack taken away.
const KEPT: &str = ".kept";

// Where a kind's packs go: among her pets, or her wallpapers.
pub fn root(dir: &Path, kind: Kind) -> PathBuf {
    dir.join(if kind == Kind::Pet { "pets" } else { "wallpapers" })
}

// --- Checked: what a pack holds ------------------------------------------------------------

// One of a wallpaper pack's pictures: at its top, its preview none of them.
fn is_picture(path: &str) -> bool {
    !path.contains('/') && !PREVIEWS.contains(&path) && PICTURES.contains(&addons::ext_of(path).as_str())
}

// A pack's files, as addons.rs open_package reads a package (after its
// manifest and the programs): its kind's types alone, pictures and words;
// a pet's sheet and pet.json at its top, pet.json a JSON object; a
// wallpaper pack's pictures at its top, one at least.
pub fn check(m: &Manifest, files: &[(zip::Entry, String)], bytes: &[u8]) -> Result<(), Refused> {
    let types: &[&str] = if m.kind == Kind::Pet { &PET_FILES } else { &PICTURES };
    for (_, path) in files {
        let ext = addons::ext_of(path);
        if !types.contains(&ext.as_str()) && !WORDS.contains(&ext.as_str()) {
            return Err(refused("packType", Some(path), "a pack holds pictures and words alone"));
        }
    }
    let at_top = |name: &str| files.iter().find(|(_, p)| p == name).map(|(e, _)| e);
    if m.kind == Kind::Pet {
        let (Some(_), Some(meta)) = (at_top(PET_SHEET), at_top(PET_JSON)) else {
            return Err(refused("pet", None, "a pet pack needs pet.json and spritesheet.webp at its top"));
        };
        let text = zip::bytes_of(bytes, meta).map_err(|e| refused("zip", Some(PET_JSON), e))?;
        let v: Value = serde_json::from_slice(addons::strip_bom(&text)).map_err(|e| refused("pet", Some(PET_JSON), e.to_string()))?;
        if !v.is_object() {
            return Err(refused("pet", Some(PET_JSON), "not a JSON object"));
        }
    } else if !files.iter().any(|(_, p)| is_picture(p)) {
        return Err(refused("pictures", None, "no pictures at its top (.jpg .jpeg .png .webp)"));
    }
    Ok(())
}

// Whether a pack may go in: its folder, if there is one, must be a pack's
// of its kind already (then it is an update). A pet of that name wherever
// she finds pets (`pet_dirs`: hers, made by hand or from codex-pets.net,
// the source tree's, Codex's), or a folder of that name among the
// wallpapers, without her mark, is the person's: refused, left as it is.
pub fn may_go_in(dir: &Path, m: &Manifest) -> Result<(), Refused> {
    match m.kind {
        Kind::Plugin => Ok(()),
        Kind::Pet => clash(&crate::data::pet_dirs(dir), &root(dir, Kind::Pet), m),
        Kind::Wallpaper => clash(&[root(dir, Kind::Wallpaper)], &root(dir, Kind::Wallpaper), m),
    }
}

// The same, with the folders to look in given, its own root among them.
fn clash(roots: &[PathBuf], own: &Path, m: &Manifest) -> Result<(), Refused> {
    let mine = own.join(&m.id);
    for folder in roots.iter().map(|r| r.join(&m.id)) {
        if folder.exists() && !(folder == mine && mark_of(&folder, m.kind).is_some()) {
            let what = if m.kind == Kind::Pet { "a pet" } else { "a folder" };
            return Err(refused("clash", Some(&m.id), format!("{} {}: {what} of that name is there already, not a pack's; it is left as it is", m.kind.name(), m.id)));
        }
    }
    Ok(())
}

// --- Marked: the packs here ----------------------------------------------------------------

// A pack here, as her mark in its folder says it.
#[derive(Clone, Debug, PartialEq)]
pub struct Pack {
    pub id: String,
    pub kind: Kind,
    pub version: String,
    pub name: Words,
    pub description: Option<Words>,
    pub author: String,
    pub source: String,
    pub reviewed: bool,
    pub installed_at: u64,
    // Its files' bytes as written; a wallpaper pack's pictures.
    pub size: u64,
    pub pictures: u64,
    pub folder: PathBuf,
    pub preview: Option<PathBuf>,
}

fn words(w: &Words) -> Value {
    json!({ "en": w.en, "zh": w.zh })
}

// A folder's mark read: its id the folder's, of this kind; None for one
// without her mark, or one that does not read right (the person's, then).
fn mark_of(folder: &Path, kind: Kind) -> Option<Pack> {
    let id = folder.file_name()?.to_str()?;
    if !addons::is_id(id) {
        return None;
    }
    let v: Value = serde_json::from_str(&fs::read_to_string(folder.join(MARK)).ok()?).ok()?;
    if v["id"] != id || v["kind"] != kind.name() {
        return None;
    }
    let ext = v["preview"].as_str().filter(|e| PREVIEWS.iter().any(|p| p.ends_with(&format!(".{e}"))));
    let preview = ext.map(|e| folder.parent().unwrap_or(folder).join(PREVIEW_DIR).join(format!("{id}.{e}"))).filter(|p| p.is_file());
    Some(Pack {
        id: id.to_string(),
        kind,
        version: v["version"].as_str().filter(|x| addons::is_version(x))?.to_string(),
        name: Words::of(Some(&v["name"]), 24).ok()?,
        description: Words::of(v.get("description"), 500).ok(),
        author: v["author"].as_str().unwrap_or("").chars().take(40).collect(),
        source: if v["source"] == "market" { "market" } else { "file" }.to_string(),
        reviewed: v["source"] == "market" && v["reviewed"] == true,
        installed_at: v["installedAt"].as_u64().unwrap_or(0),
        size: v["size"].as_u64().unwrap_or(0),
        pictures: v["pictures"].as_u64().unwrap_or(0),
        folder: folder.to_path_buf(),
        preview,
    })
}

// The packs here, pets first, each kind by id: as last found, until a
// kind's folder changes (one put in or taken away moves its time) or she
// writes one (PUT), as data.rs keeps her pets.
static FOUND: Mutex<Option<(PathBuf, u64, Vec<Option<SystemTime>>, Vec<Pack>)>> = Mutex::new(None);
static PUT: AtomicU64 = AtomicU64::new(0);

pub fn list(dir: &Path) -> Vec<Pack> {
    let put = PUT.load(Ordering::SeqCst);
    let stamps: Vec<Option<SystemTime>> = [Kind::Pet, Kind::Wallpaper].iter().map(|k| fs::metadata(root(dir, *k)).and_then(|m| m.modified()).ok()).collect();
    let mut kept = FOUND.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((was_dir, was, at, found)) = kept.as_ref() {
        if was_dir == dir && *was == put && *at == stamps {
            return found.clone();
        }
    }
    let mut found = Vec::new();
    for kind in [Kind::Pet, Kind::Wallpaper] {
        let mut these: Vec<Pack> = fs::read_dir(root(dir, kind)).into_iter().flatten().flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).filter_map(|e| mark_of(&e.path(), kind)).collect();
        these.sort_by(|a, b| a.id.cmp(&b.id));
        found.extend(these);
    }
    *kept = Some((dir.to_path_buf(), put, stamps, found.clone()));
    found
}

fn changed() {
    PUT.fetch_add(1, Ordering::SeqCst);
}

// The version of a kind's pack here (what an install replaces).
pub fn version_here(dir: &Path, kind: Kind, id: &str) -> Option<String> {
    addons::is_id(id).then(|| mark_of(&root(dir, kind).join(id), kind)).flatten().map(|p| p.version)
}

// A pack's folder by its kind and id, as a whole path (for Windows'
// slideshow): a marked pack's alone, never any folder the page names.
pub fn folder_of(dir: &Path, kind: Kind, id: &str) -> Option<PathBuf> {
    let folder = root(dir, kind).join(id);
    addons::is_id(id).then(|| mark_of(&folder, kind)).flatten().map(|_| std::path::absolute(&folder).unwrap_or(folder))
}

// The packs from the market, for its updates (market.rs): none asks for anything.
pub fn from_market(dir: &Path, lang: &str) -> Vec<FromMarket> {
    list(dir).into_iter().filter(|p| p.source == "market").map(|p| FromMarket { id: p.id, kind: p.kind, name: p.name.get(lang).to_string(), version: p.version, granted: Perms::default() }).collect()
}

// Two folders the same, as Windows names them: whole, any case, either slash.
fn same_folder(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf()).to_string_lossy().replace('/', "\\").trim_end_matches('\\').to_lowercase();
    norm(a) == norm(b)
}

// Whether Windows' slideshow is on this folder now (paper.rs now).
fn in_use(paper: &Value, folder: &Path) -> bool {
    paper["slideshow"] == true && paper["folder"].as_str().is_some_and(|f| same_folder(Path::new(f), folder))
}

// The packs for the settings (the Plugins page's 资源包, the Look page's
// rows), in her language: its words, where it came from, its size and
// pictures, its preview by the asset protocol (its time in it, so an
// update shows), whether it is in use (her pet; the slideshow's folder).
pub fn view(sh: &Shared, packs: &[Pack], paper: &Value) -> Value {
    let lang = sh.lang();
    let pet = sh.setting("pet");
    Value::Array(
        packs
            .iter()
            .map(|p| {
                json!({
                    "id": p.id,
                    "kind": p.kind.name(),
                    "name": p.name.get(lang),
                    "description": p.description.as_ref().map(|d| addons::shown(d.get(lang), 60)),
                    "author": p.author,
                    "version": p.version,
                    "source": p.source,
                    "reviewed": p.reviewed,
                    "installedAt": p.installed_at,
                    "size": p.size,
                    "pictures": p.pictures,
                    "preview": p.preview.as_ref().map(|f| format!("{}?v={}", crate::data::asset_url(f), p.installed_at)),
                    "inUse": if p.kind == Kind::Pet { pet == p.id.as_str() } else { in_use(paper, &p.folder) },
                })
            })
            .collect(),
    )
}

// For the Look page's list of pets: the pack a pet is, if it is one
// ({ source, reviewed }), by its sheet's folder.
pub fn of_pet(packs: &[Pack], pet: &crate::data::PetInfo) -> Value {
    packs.iter().find(|k| k.kind == Kind::Pet && k.id == pet.id && pet.sheet.starts_with(&k.folder)).map_or(Value::Null, |k| json!({ "source": k.source, "reviewed": k.reviewed }))
}

// --- Put in, taken away ----------------------------------------------------------------------

// What of a pack goes into its folder: all but its preview and a mark of
// its own; a wallpaper pack's top level alone.
fn keeps(kind: Kind, path: &str) -> bool {
    !PREVIEWS.contains(&path) && path != MARK && (kind == Kind::Pet || !path.contains('/'))
}

fn mark_json(m: &Manifest, source: &str, sha256: &str, size: u64, pictures: u64, preview: Option<&str>) -> Value {
    json!({
        "id": m.id,
        "kind": m.kind.name(),
        "version": m.version,
        "name": words(&m.name),
        "description": m.description.as_ref().map(words),
        "author": m.author,
        "homepage": m.homepage,
        "source": source,
        "reviewed": source == "market",
        "sha256": sha256,
        "installedAt": now_ms(),
        "size": size,
        "pictures": pictures,
        "preview": preview,
    })
}

// A pack checked, its folder put in whole with her mark (an update: the
// one there replaced, the person's choice kept), its preview beside;
// her pets looked for anew. Its summary, as addons.rs put_in's.
pub fn put_in(sh: &Shared, pkg: Package, source: &'static str, from: &str, file: &str) -> Result<Value, Refused> {
    let id = pkg.manifest.id.clone();
    addons::with_id_lock(&id, || put_in_held(sh, pkg, source, from, file))
}

fn put_in_held(sh: &Shared, pkg: Package, source: &'static str, from: &str, file: &str) -> Result<Value, Refused> {
    let m = &pkg.manifest;
    may_go_in(&sh.dir, m)?;
    let root = root(&sh.dir, m.kind);
    let replaces = version_here(&sh.dir, m.kind, &m.id);
    let mut files = Vec::new();
    let mut preview = None;
    for (e, path) in &pkg.files {
        if PREVIEWS.contains(&path.as_str()) && e.size <= PREVIEW_MAX && preview.is_none() {
            preview = Some((addons::ext_of(path), zip::bytes_of(&pkg.bytes, e).map_err(|err| refused("zip", Some(path), err))?));
        }
        if keeps(m.kind, path) {
            files.push((path.clone(), zip::bytes_of(&pkg.bytes, e).map_err(|err| refused("zip", Some(path), err))?));
        }
    }
    let size = files.iter().map(|(_, b)| b.len() as u64).sum();
    let pictures = if m.kind == Kind::Wallpaper { files.iter().filter(|(p, _)| is_picture(p)).count() as u64 } else { 0 };
    let mark = mark_json(m, source, &pkg.sha256, size, pictures, preview.as_ref().map(|(e, _)| e.as_str()));
    fs::create_dir_all(&root).map_err(|e| refused("write", None, format!("{}: {e}", root.display())))?;
    write(&root, &m.id, &files, &mark).map_err(|e| refused("write", None, e))?;
    if let Err(e) = keep_preview(&root, &m.id, preview.as_ref().map(|(e, b)| (e.as_str(), b.as_slice()))) {
        sh.log(&format!("packs: {}'s preview not kept: {e}", m.id));
    }
    // Its preview where her pages may load it (the pets' folders are already).
    let _ = sh.app.asset_protocol_scope().allow_directory(root.join(PREVIEW_DIR), true);
    changed();
    if m.kind == Kind::Pet {
        crate::data::forget_pets();
    }
    sh.log(&format!("packs: {} {} {} installed from {from}{}", m.kind.name(), m.id, m.version, replaces.as_ref().map(|v| format!(", in place of {v}")).unwrap_or_default()));
    // Her pet, if it is this one, drawn anew (its sheet's address has its time).
    sh.redraw();
    let mut out = addons::summary(&sh.dir, &Ok(pkg), sh.lang(), file);
    out["replaces"] = json!(replaces);
    Ok(out)
}

// Its files into a folder beside (.staging-<id>-<n>), her mark with them,
// then put in place: the one there moved aside, and thrown away once the
// new one is in; held (Windows' slideshow on it, its sheet open), written
// over in place instead.
fn write(root: &Path, id: &str, files: &[(String, Vec<u8>)], mark: &Value) -> Result<(), String> {
    let n = now_ms();
    let staging = root.join(format!(".staging-{id}-{n}"));
    let old = root.join(format!(".old-{id}-{n}"));
    let target = root.join(id);
    let written = (|| -> std::io::Result<()> {
        fs::create_dir_all(&staging)?;
        for (path, bytes) in files {
            let to = staging.join(path);
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&to, bytes)?;
        }
        fs::write(staging.join(MARK), serde_json::to_vec_pretty(mark).unwrap_or_default())
    })();
    if let Err(e) = written {
        let _ = fs::remove_dir_all(&staging);
        return Err(format!("{}: {e}", staging.display()));
    }
    if target.exists() && addons::rename_soon(&target, &old).is_err() {
        let over = over_in_place(&staging, &target);
        let _ = fs::remove_dir_all(&staging);
        return over.map_err(|e| format!("written over in place: {e}"));
    }
    if let Err(e) = addons::rename_soon(&staging, &target) {
        if old.exists() {
            let _ = addons::rename_soon(&old, &target);
        }
        let _ = fs::remove_dir_all(&staging);
        return Err(format!("not put in place: {e}"));
    }
    let _ = fs::remove_dir_all(&old);
    Ok(())
}

// The new files over the old in a folder that cannot be moved (Windows'
// slideshow on it, a sheet open): what was in it taken out but its pack.json
// last, then each new one moved in and the new pack.json last of all, so a
// stop mid-way (a locked file) never leaves the folder without a mark — it
// stays a marked pack, stale at worst, never one taken for the person's own.
fn over_in_place(from: &Path, to: &Path) -> std::io::Result<()> {
    let is_mark = |e: &fs::DirEntry| e.file_name() == std::ffi::OsStr::new(MARK);
    for e in fs::read_dir(to)?.flatten().filter(|e| !is_mark(e)) {
        if e.file_type()?.is_dir() {
            fs::remove_dir_all(e.path())?;
        } else {
            fs::remove_file(e.path())?;
        }
    }
    for e in fs::read_dir(from)?.flatten().filter(|e| !is_mark(e)) {
        fs::rename(e.path(), to.join(e.file_name()))?;
    }
    // The mark last, over the old: the moment it is gone the new one is there.
    fs::rename(from.join(MARK), to.join(MARK))
}

// Its preview kept as <root>/.previews/<id>.<ext>, one before of another
// type gone; none, none kept.
fn keep_preview(root: &Path, id: &str, preview: Option<(&str, &[u8])>) -> std::io::Result<()> {
    let dir = root.join(PREVIEW_DIR);
    drop_preview(root, id);
    let Some((ext, bytes)) = preview else { return Ok(()) };
    fs::create_dir_all(&dir)?;
    addons::write_whole(&dir.join(format!("{id}.{ext}")), bytes)
}

fn drop_preview(root: &Path, id: &str) {
    for p in PREVIEWS {
        let _ = fs::remove_file(root.join(PREVIEW_DIR).join(format!("{id}.{}", addons::ext_of(p))));
    }
}

// The pet she falls back to when hers goes (a pack taken away): the default
// pet, else the first other there is; none, None (hers is written anew).
pub(crate) fn fallback_pet(pets: &[crate::data::PetInfo], gone: &str) -> Option<String> {
    let others: Vec<&crate::data::PetInfo> = pets.iter().filter(|p| p.id != gone).collect();
    others.iter().find(|p| p.id == crate::fetch::DEFAULT_PET).or(others.first()).map(|p| p.id.clone())
}

// The picture to keep as a wallpaper pack's folder goes (paper.rs now):
// none unless Windows' slideshow is on that folder; then the one it shows,
// else the folder's first.
pub(crate) fn shown_from(paper: &Value, folder: &Path) -> Option<PathBuf> {
    if !in_use(paper, folder) {
        return None;
    }
    paper["file"].as_str().filter(|f| !f.is_empty()).map(PathBuf::from).filter(|f| f.is_file()).or_else(|| {
        let mut all: Vec<PathBuf> = fs::read_dir(folder).ok()?.flatten().map(|e| e.path()).filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(is_picture)).collect();
        all.sort();
        all.into_iter().next()
    })
}

// That picture copied out of the folder (<root>/.kept/, nothing else kept
// there) and shown alone: the slideshow ends, the desktop as it was.
fn keep_shown(root: &Path, file: &Path) -> Result<PathBuf, String> {
    let kept = root.join(KEPT);
    let _ = fs::remove_dir_all(&kept);
    fs::create_dir_all(&kept).map_err(|e| e.to_string())?;
    let to = kept.join(file.file_name().ok_or("no file name")?);
    fs::copy(file, &to).map_err(|e| e.to_string())?;
    Ok(to)
}

// A pack taken away, by its kind and id: a marked pack's folder alone,
// never the person's own. Her pet, if it was this one, the default one
// first; Windows' slideshow, if on its folder, ended on the picture shown.
pub fn uninstall(sh: &Shared, kind: Kind, id: &str) -> Result<(), String> {
    addons::with_id_lock(id, || uninstall_held(sh, kind, id))
}

fn uninstall_held(sh: &Shared, kind: Kind, id: &str) -> Result<(), String> {
    let root = root(&sh.dir, kind);
    let folder = root.join(id);
    let pack = addons::is_id(id).then(|| mark_of(&folder, kind)).flatten().ok_or_else(|| format!("{id}: no {} pack of that name here", kind.name()))?;
    if kind == Kind::Pet && sh.setting("pet") == id {
        let next = match fallback_pet(&crate::data::pets(&sh.dir), id) {
            Some(next) => next,
            None => crate::fetch::seed_default_pet(&root).map(|_| crate::fetch::DEFAULT_PET.to_string()).map_err(|e| format!("the default pet not written: {e}"))?,
        };
        sh.log(&format!("packs: {id} was her pet; {next} now"));
        shared(&sh.app).change(json!({ "pet": next }));
    }
    if kind == Kind::Wallpaper {
        if let Some(file) = shown_from(&crate::paper::now(), &folder) {
            match keep_shown(&root, &file) {
                Ok(kept) => sh.log(&format!("packs: the slideshow was on {id}; {} kept and shown alone: {}", kept.display(), crate::paper::set_picture(&kept.to_string_lossy()))),
                Err(e) => sh.log(&format!("packs: the slideshow was on {id}; its picture not kept ({e}), Windows' left as it is")),
            }
        }
    }
    let aside = root.join(format!(".old-{id}-{}", now_ms()));
    match addons::rename_soon(&folder, &aside) {
        Ok(()) => {
            let _ = fs::remove_dir_all(&aside);
        }
        Err(_) => fs::remove_dir_all(&folder).map_err(|e| format!("its folder: {e}"))?,
    }
    drop_preview(&root, id);
    changed();
    if kind == Kind::Pet {
        crate::data::forget_pets();
    }
    sh.log(&format!("packs: {} {id} {} taken away", kind.name(), pack.version));
    sh.redraw();
    Ok(())
}

// As she starts: what an install or a removal cut short left beside the
// packs put right (a staging thrown away, the only copy kept where it was
// set aside: addons::clean_interrupted).
pub fn start(dir: &Path) {
    for kind in [Kind::Pet, Kind::Wallpaper] {
        addons::clean_interrupted(&root(dir, kind));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::addons::open_package;
    use crate::zip::tests::zip_of;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wakuwaku-packs-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn package(files: &[(&str, &[u8])]) -> Vec<u8> {
        zip_of(&files.iter().map(|(n, d)| (*n, *d, true)).collect::<Vec<_>>())
    }

    fn manifest(id: &str, kind: &str, version: &str) -> Vec<u8> {
        serde_json::to_vec(&json!({ "id": id, "kind": kind, "name": { "en": "Pixel cat", "zh": "像素猫" }, "description": "A 16-bit kitten", "version": version, "author": "neko" })).unwrap()
    }

    const WEBP: &[u8] = b"RIFF\x10\x00\x00\x00WEBPVP8L....";
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n....";
    const PET_META: &[u8] = br#"{ "id": "pixel-cat", "displayName": "Pixel cat", "author": "neko", "spritesheetPath": "spritesheet.webp" }"#;

    fn cat(version: &str, more: &[(&str, &[u8])]) -> Package {
        let m = manifest("pixel-cat", "pet", version);
        let mut files: Vec<(&str, &[u8])> = vec![("manifest.json", &m), ("pet.json", PET_META), ("spritesheet.webp", WEBP), ("preview.png", PNG)];
        files.extend_from_slice(more);
        open_package(package(&files)).unwrap()
    }

    fn walls(version: &str) -> Package {
        let m = manifest("aurora", "wallpaper", version);
        open_package(package(&[("manifest.json", &m), ("north.png", PNG), ("south.webp", WEBP), ("preview.webp", WEBP), ("README.md", b"# Aurora"), ("more/deep.png", PNG)])).unwrap()
    }

    // Written whole into its root as put_in writes it, its mark with it.
    fn put(dir: &Path, pkg: &Package, source: &str) {
        let m = &pkg.manifest;
        let root = root(dir, m.kind);
        fs::create_dir_all(&root).unwrap();
        let files: Vec<(String, Vec<u8>)> = pkg.files.iter().filter(|(_, p)| keeps(m.kind, p)).map(|(e, p)| (p.clone(), zip::bytes_of(&pkg.bytes, e).unwrap())).collect();
        let pictures = files.iter().filter(|(p, _)| is_picture(p)).count() as u64;
        let size = files.iter().map(|(_, b)| b.len() as u64).sum();
        write(&root, &m.id, &files, &mark_json(m, source, &pkg.sha256, size, pictures, Some("png"))).unwrap();
        changed();
        crate::data::forget_pets();
    }

    // A pack holds its kind's pictures and words alone; a pet its sheet and
    // pet.json; a wallpaper pack a picture at its top. A .js is refused.
    #[test]
    fn a_pack_holds_its_kinds_files_alone() {
        let m = manifest("pixel-cat", "pet", "1.0.0");
        let reason = |files: &[(&str, &[u8])]| open_package(package(files)).err().map(|r| (r.reason, r.file));
        assert!(open_package(package(&[("manifest.json", &m), ("pet.json", PET_META), ("spritesheet.webp", WEBP)])).is_ok());
        assert_eq!(reason(&[("manifest.json", &m), ("pet.json", PET_META), ("spritesheet.webp", WEBP), ("walk.js", b"x")]), Some(("packType", Some("walk.js".into()))));
        assert_eq!(reason(&[("manifest.json", &m), ("pet.json", PET_META), ("spritesheet.webp", WEBP), ("style.css", b"x")]), Some(("packType", Some("style.css".into()))));
        assert_eq!(reason(&[("manifest.json", &m), ("spritesheet.webp", WEBP)]).map(|r| r.0), Some("pet"));
        assert_eq!(reason(&[("manifest.json", &m), ("pet.json", PET_META)]).map(|r| r.0), Some("pet"));
        assert_eq!(reason(&[("manifest.json", &m), ("pet.json", b"[1, 2]"), ("spritesheet.webp", WEBP)]), Some(("pet", Some("pet.json".into()))));
        // A program is refused before all that, whatever its name.
        assert_eq!(reason(&[("manifest.json", &m), ("pet.json", PET_META), ("spritesheet.webp", b"MZ\x90\x00")]).map(|r| r.0), Some("program"));
        let w = manifest("aurora", "wallpaper", "1.0.0");
        assert!(open_package(package(&[("manifest.json", &w), ("north.jpg", b"\xff\xd8\xff..")])).is_ok());
        assert_eq!(reason(&[("manifest.json", &w), ("north.jpg", b"\xff\xd8\xff.."), ("slideshow.js", b"x")]), Some(("packType", Some("slideshow.js".into()))));
        assert_eq!(reason(&[("manifest.json", &w), ("north.gif", b"GIF89a")]).map(|r| r.0), Some("packType"));
        assert_eq!(reason(&[("manifest.json", &w), ("preview.png", PNG), ("deep/north.png", PNG)]).map(|r| r.0), Some("pictures"));
        // A pack leaves its API out; no code, so no main.
        assert!(addons::manifest_of(&json!({ "id": "aurora", "kind": "wallpaper", "name": "Aurora", "version": "1.0.0" })).is_ok_and(|m| m.api == 1 && !m.runs()));
        assert!(addons::manifest_of(&json!({ "id": "x1", "kind": "plugin", "name": "X", "version": "1.0.0", "main": "main.js", "category": "info" })).is_err_and(|e| e.starts_with("api")));
    }

    // Where each goes, marked: a pet among her pets, found as one, its
    // preview beside; wallpapers in their own folder, its top level alone.
    #[test]
    fn packs_go_where_their_kind_goes_marked_as_packs() {
        let dir = scratch("put");
        put(&dir, &cat("1.0.0", &[("art/notes.txt", b"drawn by neko")]), "market");
        let folder = dir.join("pets").join("pixel-cat");
        assert!(folder.join("spritesheet.webp").is_file() && folder.join("pet.json").is_file() && folder.join("art").join("notes.txt").is_file());
        assert!(!folder.join("preview.png").exists(), "the preview is kept beside, not in the pet");
        let found = crate::data::pets(&dir);
        assert!(found.iter().any(|p| p.id == "pixel-cat" && p.name == "Pixel cat" && p.sheet.starts_with(&folder)));
        put(&dir, &walls("1.0.0"), "file");
        let walls = dir.join("wallpapers").join("aurora");
        let mut names: Vec<String> = fs::read_dir(&walls).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        assert_eq!(names, ["README.md", "manifest.json", "north.png", "pack.json", "south.webp"], "the top level, no preview, no sub-folder");
        // The marks: which is which, where from, how many pictures.
        let packs = list(&dir);
        assert_eq!(packs.iter().map(|p| (p.kind, p.id.as_str(), p.source.as_str(), p.reviewed)).collect::<Vec<_>>(), [(Kind::Pet, "pixel-cat", "market", true), (Kind::Wallpaper, "aurora", "file", false)]);
        assert_eq!((packs[1].pictures, packs[1].name.get("zh"), packs[1].author.as_str()), (2, "像素猫", "neko"));
        assert_eq!(version_here(&dir, Kind::Pet, "pixel-cat").as_deref(), Some("1.0.0"));
        assert_eq!(version_here(&dir, Kind::Wallpaper, "pixel-cat"), None, "of its kind alone");
        // A pet made by hand, or one from codex-pets.net: no pack.
        fs::create_dir_all(dir.join("pets").join("mochi")).unwrap();
        fs::write(dir.join("pets").join("mochi").join("spritesheet.webp"), WEBP).unwrap();
        changed();
        assert!(list(&dir).iter().all(|p| p.id != "mochi"));
        // A preview kept, of the type it was; none, none.
        keep_preview(&root(&dir, Kind::Pet), "pixel-cat", Some(("png", PNG))).unwrap();
        assert!(dir.join("pets").join(".previews").join("pixel-cat.png").is_file());
        keep_preview(&root(&dir, Kind::Pet), "pixel-cat", Some(("webp", WEBP))).unwrap();
        assert!(!dir.join("pets").join(".previews").join("pixel-cat.png").exists());
        keep_preview(&root(&dir, Kind::Pet), "pixel-cat", None).unwrap();
        assert!(!dir.join("pets").join(".previews").join("pixel-cat.webp").exists());
        // The market's, for its updates: a pack asks for nothing.
        let market = from_market(&dir, "zh");
        assert_eq!(market.iter().map(|f| (f.id.as_str(), f.kind, f.version.as_str())).collect::<Vec<_>>(), [("pixel-cat", Kind::Pet, "1.0.0")]);
        assert_eq!(market[0].granted, Perms::default());
        let _ = fs::remove_dir_all(dir);
    }

    // The person's own pet or folder of that name: refused, left as it is.
    // Hers, marked: the same id again is its update.
    #[test]
    fn a_pack_never_takes_the_place_of_the_persons_own() {
        let dir = scratch("clash");
        let roots = [dir.join("pets"), dir.join("tree-pets"), dir.join("codex-pets")];
        let own = &roots[0];
        let pkg = cat("1.0.0", &[]);
        assert!(clash(&roots, own, &pkg.manifest).is_ok());
        // Made by hand, here or where Codex keeps its pets.
        for root in [&roots[0], &roots[2]] {
            fs::create_dir_all(root.join("pixel-cat")).unwrap();
            fs::write(root.join("pixel-cat").join("pet.json"), "{}").unwrap();
            let no = clash(&roots, own, &pkg.manifest).unwrap_err();
            assert_eq!((no.reason, no.file.as_deref()), ("clash", Some("pixel-cat")));
            assert!(no.detail.starts_with("pet pixel-cat: a pet of that name"), "{}", no.detail);
            assert_eq!(fs::read_to_string(root.join("pixel-cat").join("pet.json")).unwrap(), "{}", "left as it is");
            fs::remove_dir_all(root.join("pixel-cat")).unwrap();
        }
        // Hers: an update.
        put(&dir, &pkg, "file");
        assert!(clash(&roots, own, &cat("1.1.0", &[]).manifest).is_ok());
        // A wallpaper folder of that name, not a pack's.
        let walls = walls("1.0.0");
        fs::create_dir_all(dir.join("wallpapers").join("aurora")).unwrap();
        let no = may_go_in(&dir, &walls.manifest).unwrap_err();
        assert!(no.reason == "clash" && no.detail.starts_with("wallpaper aurora: a folder"));
        // The default pet in her folder is no pack: none of that id goes in.
        fs::create_dir_all(dir.join("pets").join("claude-chan")).unwrap();
        let chan = open_package(package(&[("manifest.json", &manifest("claude-chan", "pet", "1.0.0")), ("pet.json", PET_META), ("spritesheet.webp", WEBP)])).unwrap();
        assert_eq!(may_go_in(&dir, &chan.manifest).err().map(|r| r.reason), Some("clash"));
        let _ = fs::remove_dir_all(dir);
    }

    // An update replaces the files, its folder and id the same (so the pet
    // in use and the slideshow's folder are the same still); one held is
    // written over in place.
    #[test]
    fn an_update_replaces_the_files_and_keeps_the_place() {
        let dir = scratch("update");
        put(&dir, &cat("1.0.0", &[("old.txt", b"old")]), "market");
        let folder = dir.join("pets").join("pixel-cat");
        put(&dir, &cat("1.1.0", &[]), "market");
        assert_eq!(version_here(&dir, Kind::Pet, "pixel-cat").as_deref(), Some("1.1.0"));
        assert!(!folder.join("old.txt").exists(), "the new files alone");
        let left: Vec<String> = fs::read_dir(dir.join("pets")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(left, ["pixel-cat"], "nothing beside it");
        // Held: the new files over the old in place, pack.json last (finding 9),
        // so a stop mid-way never leaves the folder without a mark.
        let staging = dir.join("staging");
        fs::create_dir_all(staging.join("sub")).unwrap();
        fs::write(staging.join("spritesheet.webp"), b"new").unwrap();
        fs::write(staging.join("sub").join("a.txt"), b"a").unwrap();
        fs::write(staging.join(MARK), br#"{"id":"pixel-cat","kind":"pet","name":"Cat","version":"1.2.0"}"#).unwrap();
        fs::write(folder.join("gone.txt"), b"x").unwrap();
        over_in_place(&staging, &folder).unwrap();
        assert_eq!(fs::read(folder.join("spritesheet.webp")).unwrap(), b"new");
        assert!(folder.join("sub").join("a.txt").is_file() && !folder.join("gone.txt").exists());
        assert_eq!(mark_of(&folder, Kind::Pet).map(|p| p.version), Some("1.2.0".into()), "the new mark, not the old");
        // The folder for Windows' slideshow: a marked wallpaper pack's alone, whole.
        put(&dir, &walls("1.0.0"), "market");
        let walls = folder_of(&dir, Kind::Wallpaper, "aurora").unwrap();
        assert!(walls.is_absolute() && walls.ends_with("aurora"));
        assert_eq!(folder_of(&dir, Kind::Wallpaper, "pixel-cat"), None);
        assert_eq!(folder_of(&dir, Kind::Wallpaper, "../pets/pixel-cat"), None);
        fs::create_dir_all(dir.join("wallpapers").join("mine")).unwrap();
        assert_eq!(folder_of(&dir, Kind::Wallpaper, "mine"), None, "the person's own folder is no pack");
        // As she starts: what an install cut short left, gone.
        fs::create_dir_all(dir.join("wallpapers").join(".staging-aurora-1")).unwrap();
        start(&dir);
        assert!(!dir.join("wallpapers").join(".staging-aurora-1").exists() && walls.is_dir());
        let _ = fs::remove_dir_all(dir);
    }

    fn pet(id: &str) -> crate::data::PetInfo {
        crate::data::PetInfo { id: id.into(), name: id.into(), author: String::new(), sheet: PathBuf::from(format!("{id}/spritesheet.webp")), version: 2, from_codex: false, stamp: 0 }
    }

    // Taken away while in use: her pet the default one, else the first
    // other; the slideshow on its folder ended on the picture it shows.
    #[test]
    fn taken_away_in_use_she_falls_back() {
        assert_eq!(fallback_pet(&[pet("mochi"), pet("pixel-cat"), pet("claude-chan")], "pixel-cat").as_deref(), Some("claude-chan"));
        assert_eq!(fallback_pet(&[pet("pixel-cat"), pet("mochi")], "pixel-cat").as_deref(), Some("mochi"));
        assert_eq!(fallback_pet(&[pet("pixel-cat")], "pixel-cat"), None, "none: hers written anew");
        let dir = scratch("paper");
        let folder = dir.join("wallpapers").join("aurora");
        fs::create_dir_all(&folder).unwrap();
        for f in ["b.png", "a.webp", "preview.png", "notes.md"] {
            fs::write(folder.join(f), PNG).unwrap();
        }
        let shown = folder.join("b.png").to_string_lossy().into_owned();
        let upper = folder.to_string_lossy().to_uppercase().replace('\\', "/") + "/";
        // Windows' slideshow on it, however it names the folder: the picture shown.
        assert_eq!(shown_from(&json!({ "slideshow": true, "folder": upper, "file": shown }), &folder), Some(folder.join("b.png")));
        // Its picture not known: the folder's first.
        assert_eq!(shown_from(&json!({ "slideshow": true, "folder": folder, "file": "" }), &folder), Some(folder.join("a.webp")));
        // One picture, or another folder's slideshow: nothing to keep.
        assert_eq!(shown_from(&json!({ "slideshow": false, "folder": folder, "file": shown }), &folder), None);
        assert_eq!(shown_from(&json!({ "slideshow": true, "folder": dir.join("wallpapers").join("other"), "file": shown }), &folder), None);
        assert_eq!(shown_from(&Value::Null, &folder), None);
        // Kept out of the folder, alone there.
        fs::create_dir_all(dir.join("wallpapers").join(".kept")).unwrap();
        fs::write(dir.join("wallpapers").join(".kept").join("older.png"), PNG).unwrap();
        let kept = keep_shown(&dir.join("wallpapers"), &folder.join("b.png")).unwrap();
        assert_eq!(kept, dir.join("wallpapers").join(".kept").join("b.png"));
        assert!(!dir.join("wallpapers").join(".kept").join("older.png").exists());
        fs::remove_dir_all(&folder).unwrap();
        assert!(kept.is_file(), "there once the pack is gone");
        let _ = fs::remove_dir_all(dir);
    }

    // In use as the Look page says it: the slideshow on its folder.
    #[test]
    fn a_wallpaper_pack_is_in_use_while_the_slideshow_is_on_its_folder() {
        let folder = PathBuf::from(r"C:\data\wallpapers\aurora");
        assert!(in_use(&json!({ "slideshow": true, "folder": r"c:\DATA\wallpapers\aurora\" }), &folder));
        assert!(!in_use(&json!({ "slideshow": true, "folder": r"C:\data\wallpapers\aurora2" }), &folder));
        assert!(!in_use(&json!({ "slideshow": false, "folder": r"C:\data\wallpapers\aurora" }), &folder));
        assert!(!in_use(&json!({ "slideshow": true, "folder": null }), &folder));
    }
}
