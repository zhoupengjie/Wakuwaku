// Where her settings and pets live, the settings themselves, and the pets
// found.
//
// Her folder: WAKUWAKU_USER_DATA (tests), else data/ in the project when run
// from this source tree, else wakuwaku-data beside the .exe (portable). The
// first time, settings and pets come over from %APPDATA%\wakuwaku (the old
// Electron build), without her spot: that one kept it in logical pixels.
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

const SOURCE_TREE: &str = env!("CARGO_MANIFEST_DIR");

// The sizes she comes in.
pub const SCALES: [(&str, f64); 3] = [("small", 0.4), ("medium", 0.55), ("large", 0.75)];

// The source tree's path this many levels up, then `rest`: no "..", which
// the asset protocol's scope would not match.
fn in_tree(up: usize, rest: &str) -> PathBuf {
    let tree = Path::new(SOURCE_TREE);
    tree.ancestors().nth(up).unwrap_or(tree).join(rest)
}

// The example plugin scripts: in the source tree she was built from, while
// it is there, else beside the .exe.
pub fn examples() -> Option<PathBuf> {
    let beside = std::env::current_exe().ok().and_then(|exe| Some(exe.parent()?.join("examples").join("widgets")));
    std::iter::once(in_tree(1, "examples").join("widgets")).chain(beside).find(|d| d.is_dir())
}

pub fn folder() -> PathBuf {
    if let Ok(dir) = std::env::var("WAKUWAKU_USER_DATA") {
        return dir.into();
    }
    let exe = std::env::current_exe().unwrap_or_default();
    if exe.starts_with(Path::new(SOURCE_TREE).join("target")) {
        return in_tree(1, "data");
    }
    exe.parent().map(|p| p.join("wakuwaku-data")).unwrap_or_else(|| "wakuwaku-data".into())
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

// A folder of her own for the first time: start from the old build's.
pub fn adopt(dir: &Path) {
    if dir.exists() {
        return;
    }
    let _ = fs::create_dir_all(dir);
    let Ok(appdata) = std::env::var("APPDATA") else { return };
    let source = Path::new(&appdata).join("wakuwaku");
    if let Ok(text) = fs::read_to_string(source.join("config.json")) {
        if let Ok(Value::Object(mut saved)) = serde_json::from_str(&text) {
            saved.remove("x");
            saved.remove("y");
            let _ = fs::write(dir.join("config.json"), serde_json::to_string_pretty(&saved).unwrap_or_default());
        }
    }
    if source.join("pets").exists() {
        let _ = copy_dir(&source.join("pets"), &dir.join("pets"));
    }
}

// --- Settings ----------------------------------------------------------------------

fn defaults() -> Map<String, Value> {
    let Value::Object(map) = json!({
        "lang": "auto",
        "pet": "claude-chan",
        "scale": 0.55,
        // Her home: corner, island or bar; out, she is on the desktop.
        "display": "corner",
        "out": true,
        "corner": "br",
        "bubble": true,
        "details": true,
        // Widgets (plugins in the island): which are off, their order, how
        // often the island turns to the next, whether they may open it.
        "widgetsOff": ["sys", "net", "battery"],
        "widgetOrder": [],
        "widgetSpin": 8,
        "widgetNudge": true,
        "walk": true,
        "look": true,
        "hold": "seen",
        "notify": { "waiting": false, "done": false, "error": false },
        "sound": false,
        "promptWaitSec": 290,
        "dnd": false,
        "hideInFullscreen": true,
        "onboarded": false,
    }) else {
        unreachable!()
    };
    map
}

pub fn load(dir: &Path) -> Map<String, Value> {
    let mut settings = defaults();
    if let Ok(Value::Object(saved)) = fs::read_to_string(dir.join("config.json")).map_err(|_| ()).and_then(|t| serde_json::from_str(&t).map_err(|_| ())) {
        settings.extend(saved);
    }
    // The capsule of earlier versions is the island now.
    // The pet on her own is now the corner's, with her out.
    if settings.get("display").and_then(Value::as_str) == Some("pet") {
        settings.insert("display".into(), json!("corner"));
        settings.insert("out".into(), json!(true));
    }
    if settings.get("display").and_then(Value::as_str) == Some("capsule") {
        settings.insert("display".into(), json!("island"));
    }
    settings
}

pub fn save(dir: &Path, settings: &Map<String, Value>) {
    let _ = fs::create_dir_all(dir);
    // A settings file we cannot write only costs the remembered settings.
    let _ = fs::write(dir.join("config.json"), serde_json::to_string_pretty(settings).unwrap_or_default());
}

// --- Pets ----------------------------------------------------------------------------

pub struct PetInfo {
    pub id: String,
    pub name: String,
    pub author: String,
    pub sheet: PathBuf,
    pub version: u8,
    // One of the pets Codex has (its desktop app keeps them in the same format).
    pub from_codex: bool,
}

// The pets Codex has: only read, never written.
pub fn codex_pets() -> PathBuf {
    crate::connection::codex_home().join("pets")
}

// Downloaded pets: <her folder>/pets, then the project's pets/ (from source),
// then Codex's.
pub fn pet_dirs(dir: &Path) -> Vec<PathBuf> {
    vec![dir.join("pets"), in_tree(1, "pets"), codex_pets()]
}

pub fn pets(dir: &Path) -> Vec<PetInfo> {
    let mut found: Vec<PetInfo> = Vec::new();
    let codex = codex_pets();
    for root in pet_dirs(dir) {
        let Ok(entries) = fs::read_dir(&root) else { continue };
        let mut ids: Vec<String> = entries
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        ids.sort();
        for id in ids {
            let sheet = root.join(&id).join("spritesheet.webp");
            if found.iter().any(|p| p.id == id) || !sheet.exists() {
                continue;
            }
            let meta: Value = fs::read_to_string(root.join(&id).join("pet.json"))
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or(Value::Null);
            found.push(PetInfo {
                name: meta.get("displayName").and_then(Value::as_str).unwrap_or(&id).to_string(),
                author: meta.get("author").and_then(Value::as_str).unwrap_or("").to_string(),
                version: if meta.get("spriteVersionNumber").and_then(Value::as_u64) == Some(2) { 2 } else { 1 },
                id,
                sheet,
                from_codex: root == codex,
            });
        }
    }
    found
}

// The URL the page loads a sheet from: Tauri's asset protocol, as
// convertFileSrc makes it (encodeURIComponent of the path).
pub fn asset_url(path: &Path) -> String {
    let mut out = String::new();
    for byte in path.to_string_lossy().bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    if cfg!(any(windows, target_os = "android")) {
        format!("http://asset.localhost/{out}")
    } else {
        format!("asset://localhost/{out}")
    }
}
