// Third-party plugins (docs/PLUGINS.md; plugins.rs has hers): folders of web
// files in <her folder>/plugins/<id>, each with a manifest.json, a main.js,
// maybe images, and no programs. Installed from a .zip (inspect, then
// install): only the types of file allowed, a program refused by its first
// bytes whatever it is called, no name out of the folder, sizes kept;
// unpacked beside the copy there and swapped in once whole, the old copy
// kept until the swap is done. Taken away with their settings, storage,
// widgets and places (uninstall). installed.json beside them says where each
// came from (a file or the market, reviewed or not), its zip's SHA-256, its
// version and the permissions granted as it was installed.
//
// How they run. The island's page (src/pet/addons.ts) keeps a hidden frame,
// sandboxed with scripts alone, for each one that is on (placed), has its
// required settings filled, asks for an API she has, and is not stopped:
// running(), told again as it changes (addons:changed). A frame's files come
// from her own scheme (serve: http://wakuplugin.localhost/<id>/… on
// Windows), its own folder's alone, under a Content-Security-Policy that
// leaves it no network and nothing of anyone else's; its first page is hers
// (__waku/main.html: the SDK, src/addon/sdk.ts, which loads the plugin's
// main). The rest it reaches by messages to the page, which asks here by
// command in the plugin's name, as the page knows it by its frame: its
// widgets (checked as a script's are, kept while it runs), the web
// (addon_fetch: the hosts its manifest names alone, ports 80 and 443, never
// this computer or the network at home, the way her other requests go), its
// storage (a file, 1 MB at most), a nudge (by its permission). A frame sent
// anywhere but its own first page stays where it was (guard_frames). The
// page's watchdog stops one that hangs or is too busy (stop: stopped until
// turned on again).
//
// Its view (tier 2: view.html, src/pet/views.ts) is drawn in the desktop's
// medium and large cards and the capsule's unfolded card, served with her
// SDK first in its head (view_page); a widget it draws carries a revision
// of what it gave for it (widgets.rs viewRev), the view itself asked for
// as that changes (addon_view data), its messages for its main passed on
// to the island's page (addon_view send, addons:view).
//
// Settings: "plugins": { <id>: { "on": bool, <key>: value } }, as hers are,
// each value by its manifest's field (is_conf_ok).
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex, RwLock};
use std::time::{Duration, SystemTime};

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::{now_ms, shared, zip, Shared};
use crate::config;

// Her scheme for their files, and where her pages find it (WebView2 serves
// a scheme of the app's own at <scheme>.localhost).
pub const SCHEME: &str = "wakuplugin";
pub const ORIGIN: &str = "http://wakuplugin.localhost";
// The plugin API she speaks; a plugin asking for a newer one waits.
pub const API: u64 = 1;

// A package's sizes at most: a plugin's zipped and unpacked, a pack's, and
// files in either.
const PLUGIN_ZIP: u64 = 5 << 20;
const PLUGIN_ALL: u64 = 20 << 20;
const PACK_ZIP: u64 = 50 << 20;
const PACK_ALL: u64 = 200 << 20;
const FILES: usize = 500;
const MANIFEST_MAX: u64 = 64 * 1024;

// The files a package may hold (docs/PLUGINS.md), by their extension.
const EXTENSIONS: [&str; 23] = [
    "js", "mjs", "json", "html", "htm", "css", "svg", "png", "jpg", "jpeg", "webp", "gif", "avif", "woff", "woff2", "ttf", "otf", "mp3", "ogg", "wav", "txt", "md", "wasm",
];
// A program's first bytes: Windows' (MZ), Linux's (ELF), a Mac's (Mach-O,
// either way round, and a universal one).
const PROGRAMS: [&[u8]; 8] = [b"MZ", b"\x7fELF", b"\xfe\xed\xfa\xce", b"\xfe\xed\xfa\xcf", b"\xce\xfa\xed\xfe", b"\xcf\xfa\xed\xfe", b"\xca\xfe\xba\xbe", b"\xbe\xba\xfe\xca"];
// What a Mac's or Windows' own zipping adds, left out: never the plugin's.
fn is_litter(name: &str) -> bool {
    let n = name.replace('\\', "/");
    let last = n.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    n.starts_with("__MACOSX/") || matches!(last, ".DS_Store" | "Thumbs.db" | "desktop.ini")
}

// Her own plugins' and widgets' ids, and the beginnings of her widgets'
// (plugins.rs owner_of: stock-…, ci-…, prs-…; mail.rs: inbox-…; mail…):
// none of them a third-party plugin's.
const RESERVED: [&str; 13] = ["weather", "stock", "pomodoro", "countdown", "stretch", "ci", "devserver", "calendar", "media", "monitor", "today", "tokens", "waku"];
const RESERVED_STARTS: [&str; 5] = ["mail", "stock-", "ci-", "prs-", "inbox-"];

// Names that are this computer's or the network's at home: never a host a
// plugin may name, nor one it may reach.
const HOME_NAMES: [&str; 10] = ["localhost", "local", "internal", "lan", "home", "home.arpa", "localdomain", "intranet", "corp", "private"];

// The web, for a plugin: each request so long at most, its body and its
// answer so large, redirects so many.
const FETCH_TIME: Duration = Duration::from_secs(15);
const FETCH_MAX: usize = 1 << 20;
const HOPS: usize = 5;
// The answer's headers handed back.
const HEADERS_BACK: [&str; 10] = ["content-type", "content-language", "date", "etag", "last-modified", "cache-control", "retry-after", "x-ratelimit-limit", "x-ratelimit-remaining", "x-ratelimit-reset"];
// Request headers that are the connection's, not the plugin's.
const HEADERS_NOT: [&str; 11] = ["host", "content-length", "connection", "transfer-encoding", "upgrade", "te", "trailer", "keep-alive", "proxy-authorization", "proxy-connection", "expect"];

// A plugin's storage, at most.
const STORE_MAX: usize = 1 << 20;
// A description's characters: shown in full up to so many, longer cut with
// …; taken up to so many (docs/PLUGINS.md).
const DESCRIPTION_SHOWN: usize = 60;
const DESCRIPTION_MAX: usize = 500;
// Widgets of one plugin, at most.
const WIDGETS: usize = 8;
// A manifest's widget.sample (the market's page draws its cards by it), as JSON, at most.
pub const SAMPLE_MAX: usize = 4 * 1024;

// --- What a plugin is: its manifest ----------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Plugin,
    // Packs (phase 4): a pet, or wallpapers; no code.
    Pet,
    Wallpaper,
}

impl Kind {
    pub(crate) fn of(s: &str) -> Option<Kind> {
        match s {
            "plugin" => Some(Kind::Plugin),
            "pet" => Some(Kind::Pet),
            "wallpaper" => Some(Kind::Wallpaper),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Plugin => "plugin",
            Kind::Pet => "pet",
            Kind::Wallpaper => "wallpaper",
        }
    }

    // How large its package may be: zipped, unpacked.
    pub(crate) fn limits(self) -> (u64, u64) {
        match self {
            Kind::Plugin => (PLUGIN_ZIP, PLUGIN_ALL),
            Kind::Pet | Kind::Wallpaper => (PACK_ZIP, PACK_ALL),
        }
    }
}

// Words in English, and in Chinese (English where none are given).
#[derive(Clone, Debug, PartialEq)]
pub struct Words {
    pub en: String,
    pub zh: String,
}

impl Words {
    // A string, or { en, zh }; each some words up to `max` characters.
    pub(crate) fn of(v: Option<&Value>, max: usize) -> Result<Words, String> {
        let one = |v: &Value| -> Result<String, String> {
            let s = v.as_str().ok_or("not text")?.trim();
            if s.is_empty() {
                return Err("empty".into());
            }
            if s.chars().count() > max {
                return Err(format!("over {max} characters"));
            }
            if s.chars().any(char::is_control) {
                return Err("a control character".into());
            }
            Ok(s.to_string())
        };
        match v {
            None => Err("missing".into()),
            Some(v @ Value::String(_)) => one(v).map(|s| Words { zh: s.clone(), en: s }),
            Some(Value::Object(m)) => {
                if let Some(k) = m.keys().find(|k| !matches!(k.as_str(), "en" | "zh")) {
                    return Err(format!("{k}: only en and zh"));
                }
                let en = one(m.get("en").ok_or("en: missing (English is the fallback)")?).map_err(|e| format!("en: {e}"))?;
                let zh = match m.get("zh") {
                    Some(z) => one(z).map_err(|e| format!("zh: {e}"))?,
                    None => en.clone(),
                };
                Ok(Words { en, zh })
            }
            Some(_) => Err("text, or { en, zh }".into()),
        }
    }

    pub fn get(&self, lang: &str) -> &str {
        if lang == "zh" {
            &self.zh
        } else {
            &self.en
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FieldKind {
    Text,
    Number,
    Flag,
    // One of a few, each with its own words, if given.
    Choice { options: Vec<String>, labels: Vec<Option<Words>> },
}

// One of its settings, drawn on the Plugins page.
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub key: String,
    pub kind: FieldKind,
    pub required: bool,
    pub label: Words,
    pub hint: Option<Words>,
    // Its value until one is set: text, a number, true or false, an option.
    pub default: Value,
}

impl Field {
    pub fn type_name(&self) -> &'static str {
        match self.kind {
            FieldKind::Text => "text",
            FieldKind::Number => "number",
            FieldKind::Flag => "flag",
            FieldKind::Choice { .. } => "choice",
        }
    }

    // A value as the settings keep it: text without control characters (300
    // at most), a number (or one typed, as text; empty for none), true or
    // false, one of the options.
    fn accepts(&self, v: &Value) -> bool {
        match &self.kind {
            FieldKind::Text => v.as_str().is_some_and(|t| t.chars().count() <= 300 && !t.chars().any(char::is_control)),
            FieldKind::Number => v.as_f64().is_some_and(f64::is_finite) || v.as_str().is_some_and(|t| t.trim().is_empty() || number_of(t).is_some()),
            FieldKind::Flag => v.is_boolean(),
            FieldKind::Choice { options, .. } => v.as_str().is_some_and(|o| options.iter().any(|x| x == o)),
        }
    }

    // Its value for the plugin: as set, else its default (else nothing:
    // empty text, no number, false, the first option).
    fn value(&self, set: &Value) -> Value {
        let set = Some(set).filter(|v| self.accepts(v));
        match &self.kind {
            FieldKind::Text => json!(set.and_then(Value::as_str).map(str::trim).or(self.default.as_str()).unwrap_or("")),
            FieldKind::Number => {
                let n = set.and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(number_of))).or_else(|| self.default.as_f64());
                n.map_or(Value::Null, |n| json!(n))
            }
            FieldKind::Flag => json!(set.and_then(Value::as_bool).or(self.default.as_bool()).unwrap_or(false)),
            FieldKind::Choice { options, .. } => json!(set.and_then(Value::as_str).or(self.default.as_str()).or(options.first().map(String::as_str)).unwrap_or("")),
        }
    }

    // Left empty while it is needed (text, a number; a choice has one always).
    fn is_missing(&self, set: &Value) -> bool {
        self.required
            && match self.value(set) {
                Value::String(s) => s.is_empty(),
                Value::Null => true,
                _ => false,
            }
    }
}

fn number_of(t: &str) -> Option<f64> {
    t.trim().parse::<f64>().ok().filter(|n| n.is_finite())
}

// What it may do beyond its own widgets, settings and storage.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Perms {
    // The hosts waku.fetch may reach: exact names, or *.<domain>.
    pub net: Vec<String>,
    pub agents: bool,
    pub hardware: bool,
    pub notify: bool,
}

impl Perms {
    pub fn json(&self) -> Value {
        json!({ "net": self.net, "agents": self.agents, "hardware": self.hardware, "notify": self.notify })
    }

    // Those asked about as it is installed (docs/design/specs/plugins.md §2):
    // the agents' sessions, the hardware, nudges. The web is listed only.
    pub fn sensitive(&self) -> bool {
        self.agents || self.hardware || self.notify
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Icon {
    None,
    // One of her widgets' icons (widgets.rs ICONS).
    Named(String),
    // An .svg in its folder.
    Svg(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Manifest {
    pub id: String,
    pub kind: Kind,
    pub name: Words,
    pub description: Option<Words>,
    pub version: String,
    pub author: String,
    pub homepage: Option<String>,
    pub api: u64,
    pub main: Option<String>,
    pub view: Option<String>,
    // system, productivity, info: the widget library's 系统 / 效率 / 信息.
    pub category: &'static str,
    pub sizes: Vec<&'static str>,
    pub color: Option<String>,
    pub icon: Icon,
    pub settings: Vec<Field>,
    // What she enforces: the permissions, but none for a plugin of a newer
    // API she does not run yet (so its grant is recorded empty and she asks
    // once a newer Wakuwaku supports it: finding 7).
    pub perms: Perms,
    // What it asks for as written, whatever its API: shown on the detail and
    // install pages even for one she cannot run yet (finding 7). The same as
    // `perms` for one she runs.
    pub wants: Perms,
}

impl Manifest {
    // Whether she can run it: a plugin, of an API she has.
    pub fn runs(&self) -> bool {
        self.kind == Kind::Plugin && self.api <= API
    }

    fn field(&self, key: &str) -> Option<&Field> {
        self.settings.iter().find(|f| f.key == key)
    }

    // The settings' values for it now (Field::value), by their keys.
    fn values(&self, conf: &Value) -> Value {
        Value::Object(self.settings.iter().map(|f| (f.key.clone(), f.value(&conf[&f.key]))).collect())
    }

    // The first setting it needs and has not been given.
    fn missing(&self, conf: &Value) -> Option<&str> {
        self.settings.iter().find(|f| f.is_missing(&conf[&f.key])).map(|f| f.key.as_str())
    }

    // The files it names: its main, its view, its icon.
    fn files_named(&self) -> Vec<&str> {
        let icon = match &self.icon {
            Icon::Svg(f) => Some(f.as_str()),
            _ => None,
        };
        [self.main.as_deref(), self.view.as_deref(), icon].into_iter().flatten().collect()
    }
}

pub fn is_id(id: &str) -> bool {
    let b = id.as_bytes();
    (2..=40).contains(&b.len()) && b[0].is_ascii_lowercase() && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

pub fn is_reserved(id: &str) -> bool {
    RESERVED.contains(&id) || RESERVED_STARTS.iter().any(|p| id.starts_with(p))
}

pub(crate) fn is_version(v: &str) -> bool {
    let parts: Vec<&str> = v.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| (1..=9).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()))
}

fn is_color(c: &str) -> bool {
    let hex = c.strip_prefix('#').unwrap_or("");
    matches!(hex.len(), 3 | 6) && hex.chars().all(|c| c.is_ascii_hexdigit())
}

pub(crate) fn ext_of(name: &str) -> String {
    let last = name.rsplit(['/', '\\']).next().unwrap_or(name);
    last.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default()
}

// A file it names (its main, view, icon): a path in its folder, of these types.
fn file_of(v: Option<&Value>, exts: &[&str], what: &str) -> Result<Option<String>, String> {
    let Some(v) = v else { return Ok(None) };
    let s = v.as_str().ok_or(format!("{what}: a file's name"))?;
    let parts = zip::safe(s).filter(|_| s.len() <= 120 && !s.contains('\\') && !s.ends_with('/')).ok_or(format!("{what}: a file in its folder"))?;
    if !exts.contains(&ext_of(s).as_str()) {
        return Err(format!("{what}: a .{} file", exts.join(" or .")));
    }
    Ok(Some(parts.join("/")))
}

// A manifest as written, checked against docs/PLUGINS.md: Err with what
// will not do. One asking for a newer API is read as far as its id, name,
// version and the like, and waits (Manifest::runs).
pub fn manifest_of(v: &Value) -> Result<Manifest, String> {
    let m = v.as_object().ok_or("not a JSON object")?;
    let id = m.get("id").and_then(Value::as_str).ok_or("id: missing")?;
    if !is_id(id) {
        return Err("id: a letter, then a-z, 0-9 or -, 2 to 40 in all".into());
    }
    if is_reserved(id) {
        return Err(format!("id: {id} is one of Wakuwaku's own"));
    }
    let kind = m.get("kind").and_then(Value::as_str).and_then(Kind::of).ok_or("kind: plugin, pet or wallpaper")?;
    // A pack runs nothing, so it may leave its API out.
    let api = match m.get("api") {
        None if kind != Kind::Plugin => 1,
        api => api.and_then(Value::as_u64).filter(|n| *n >= 1).ok_or("api: 1")?,
    };
    let name = Words::of(m.get("name"), 24).map_err(|e| format!("name: {e}"))?;
    let version = m.get("version").and_then(Value::as_str).filter(|v| is_version(v)).ok_or("version: x.y.z")?.to_string();
    // A description longer than is shown is taken all the same, and cut
    // where it is shown (view): only one past any sense is refused.
    let description = match m.get("description") {
        Some(d) => Some(Words::of(Some(d), DESCRIPTION_MAX).map_err(|e| format!("description: {e}"))?),
        None => None,
    };
    let author = match m.get("author") {
        Some(a) => a.as_str().map(str::trim).filter(|a| a.chars().count() <= 40 && !a.chars().any(char::is_control)).ok_or("author: up to 40 characters")?.to_string(),
        None => String::new(),
    };
    let mut out = Manifest {
        id: id.to_string(),
        kind,
        name,
        description,
        version,
        author,
        homepage: None,
        api,
        main: None,
        view: None,
        category: "info",
        sizes: vec!["s", "m"],
        color: None,
        icon: Icon::None,
        settings: Vec::new(),
        perms: Perms::default(),
        wants: Perms::default(),
    };
    // Newer than hers: the rest is unread and she will not run it, but its
    // permissions are read all the same, to show on its page (finding 7);
    // `perms` (what she would enforce) stays empty, so its grant is recorded
    // empty and she asks once a newer Wakuwaku can run it.
    if api > API {
        if let Some(p) = m.get("permissions") {
            if let Ok(wants) = perms_of(p) {
                out.wants = wants;
            }
        }
        return Ok(out);
    }
    let pack = kind != Kind::Plugin;
    for key in m.keys() {
        let known = matches!(key.as_str(), "id" | "kind" | "name" | "description" | "version" | "author" | "homepage" | "api");
        let plugin_only = matches!(key.as_str(), "main" | "view" | "category" | "widget" | "settings" | "permissions");
        if !(known || plugin_only) {
            return Err(format!("{key}: not a field of a manifest"));
        }
        if plugin_only && pack {
            return Err(format!("{key}: a pack has none"));
        }
    }
    if let Some(h) = m.get("homepage") {
        let h = h.as_str().filter(|h| (h.starts_with("https://") || h.starts_with("http://")) && h.len() <= 200 && !h.chars().any(|c| c.is_whitespace() || c.is_control())).ok_or("homepage: an http(s) address")?;
        out.homepage = Some(h.to_string());
    }
    if pack {
        return Ok(out);
    }
    out.main = Some(file_of(m.get("main"), &["js", "mjs"], "main")?.ok_or("main: missing (a .js file in its folder)")?);
    out.view = file_of(m.get("view"), &["html", "htm"], "view")?;
    out.category = match m.get("category").and_then(Value::as_str) {
        Some("system") => "system",
        Some("productivity") => "productivity",
        Some("info") => "info",
        _ => return Err("category: system, productivity or info".into()),
    };
    if let Some(w) = m.get("widget") {
        let w = w.as_object().ok_or("widget: { sizes, color, icon }")?;
        if let Some(k) = w.keys().find(|k| !matches!(k.as_str(), "sizes" | "color" | "icon" | "sample")) {
            return Err(format!("widget.{k}: not a field of a widget"));
        }
        // What its widget would say, for the market's page (market.rs draws
        // it from the index, checked there as a widget is): nothing of hers
        // once it is installed, so only its shape and size are seen to.
        if let Some(s) = w.get("sample") {
            if !s.is_object() || s.to_string().len() > SAMPLE_MAX {
                return Err("widget.sample: a widget as POST /widget takes one, 4 KB at most".into());
            }
        }
        if let Some(s) = w.get("sizes") {
            let list = s.as_array().filter(|l| !l.is_empty() && l.len() <= 3 && l.iter().all(|z| matches!(z.as_str(), Some("s" | "m" | "l")))).ok_or("widget.sizes: some of s, m, l")?;
            out.sizes = ["s", "m", "l"].into_iter().filter(|z| list.iter().any(|x| x.as_str() == Some(z))).collect();
        }
        if let Some(c) = w.get("color") {
            out.color = Some(c.as_str().filter(|c| is_color(c)).ok_or("widget.color: #rgb or #rrggbb")?.to_string());
        }
        if let Some(i) = w.get("icon") {
            let i = i.as_str().ok_or("widget.icon: one of her icons, or an .svg in its folder")?;
            out.icon = if crate::widgets::ICONS.contains(&i) { Icon::Named(i.to_string()) } else { Icon::Svg(file_of(Some(&json!(i)), &["svg"], "widget.icon")?.unwrap_or_default()) };
        }
    }
    if let Some(s) = m.get("settings") {
        let list = s.as_array().filter(|l| l.len() <= 16).ok_or("settings: a list, 16 at most")?;
        for (i, f) in list.iter().enumerate() {
            let field = field_of(f).map_err(|e| format!("settings[{i}]: {e}"))?;
            if out.settings.iter().any(|g| g.key == field.key) {
                return Err(format!("settings[{i}]: {} twice", field.key));
            }
            out.settings.push(field);
        }
    }
    if let Some(p) = m.get("permissions") {
        out.perms = perms_of(p)?;
    }
    // One she runs asks for exactly what it is granted and shown.
    out.wants = out.perms.clone();
    Ok(out)
}

fn field_of(v: &Value) -> Result<Field, String> {
    let f = v.as_object().ok_or("{ key, type, label }")?;
    if let Some(k) = f.keys().find(|k| !matches!(k.as_str(), "key" | "type" | "label" | "hint" | "required" | "default" | "options" | "optionLabels")) {
        return Err(format!("{k}: not a field of a setting"));
    }
    let key = f.get("key").and_then(Value::as_str).unwrap_or("");
    let key_ok = key.len() <= 32 && key.as_bytes().first().is_some_and(u8::is_ascii_alphabetic) && key.bytes().all(|b| b.is_ascii_alphanumeric()) && key != "on";
    if !key_ok {
        return Err("key: a letter, then letters or digits, 32 at most (not on)".into());
    }
    let label = Words::of(f.get("label"), 40).map_err(|e| format!("{key}: label: {e}"))?;
    let hint = match f.get("hint") {
        Some(h) => Some(Words::of(Some(h), 80).map_err(|e| format!("{key}: hint: {e}"))?),
        None => None,
    };
    let required = match f.get("required") {
        Some(r) => r.as_bool().ok_or(format!("{key}: required: true or false"))?,
        None => false,
    };
    let choice = f.get("type").and_then(Value::as_str) == Some("choice");
    if !choice && (f.contains_key("options") || f.contains_key("optionLabels")) {
        return Err(format!("{key}: options only for a choice"));
    }
    let kind = match f.get("type").and_then(Value::as_str) {
        Some("text") => FieldKind::Text,
        Some("number") => FieldKind::Number,
        Some("flag") => FieldKind::Flag,
        Some("choice") => {
            let options: Vec<String> = f
                .get("options")
                .and_then(Value::as_array)
                .filter(|l| (1..=20).contains(&l.len()))
                .and_then(|l| l.iter().map(|o| o.as_str().filter(|o| (1..=40).contains(&o.chars().count()) && !o.chars().any(char::is_control)).map(str::to_string)).collect())
                .ok_or(format!("{key}: options: 1 to 20, each some words"))?;
            if options.iter().enumerate().any(|(i, o)| options[..i].contains(o)) {
                return Err(format!("{key}: an option twice"));
            }
            let mut labels = vec![None; options.len()];
            if let Some(l) = f.get("optionLabels") {
                let l = l.as_object().ok_or(format!("{key}: optionLabels: {{ option: words }}"))?;
                for (o, w) in l {
                    let at = options.iter().position(|x| x == o).ok_or(format!("{key}: optionLabels: {o} is no option"))?;
                    labels[at] = Some(Words::of(Some(w), 40).map_err(|e| format!("{key}: optionLabels.{o}: {e}"))?);
                }
            }
            FieldKind::Choice { options, labels }
        }
        _ => return Err(format!("{key}: type: text, number, flag or choice")),
    };
    let mut field = Field { key: key.to_string(), kind, required, label, hint, default: Value::Null };
    if let Some(d) = f.get("default") {
        let d = match &field.kind {
            FieldKind::Number => d.as_f64().filter(|n| n.is_finite()).map(|n| json!(n)),
            _ => Some(d.clone()).filter(|d| field.accepts(d)),
        };
        field.default = d.ok_or(format!("{key}: default: a {}", field.type_name()))?;
    }
    Ok(field)
}

pub(crate) fn perms_of(v: &Value) -> Result<Perms, String> {
    let p = v.as_object().ok_or("permissions: { net, agents, hardware, notify }")?;
    let mut out = Perms::default();
    for (k, v) in p {
        match k.as_str() {
            "net" => {
                let list = v.as_array().filter(|l| l.len() <= 16).ok_or("permissions.net: a list of hosts, 16 at most")?;
                for h in list {
                    let rule = h.as_str().map(str::to_ascii_lowercase).unwrap_or_default();
                    if !is_host_rule(&rule) {
                        return Err(format!("permissions.net: {h} is not a host it may name (a name, or *.name; no port, no address, nothing at home)"));
                    }
                    if !out.net.contains(&rule) {
                        out.net.push(rule);
                    }
                }
            }
            "agents" | "hardware" | "notify" => {
                let on = v.as_bool().ok_or(format!("permissions.{k}: true or false"))?;
                match k.as_str() {
                    "agents" => out.agents = on,
                    "hardware" => out.hardware = on,
                    _ => out.notify = on,
                }
            }
            _ => return Err(format!("permissions.{k}: not a permission")),
        }
    }
    Ok(out)
}

// --- Hosts: which a plugin may name and reach ------------------------------------------

// This computer's, or a name of the network's at home: one with no dot, or
// under one of HOME_NAMES.
fn is_home_name(host: &str) -> bool {
    !host.contains('.') || HOME_NAMES.iter().any(|e| host == *e || host.strip_suffix(e).is_some_and(|rest| rest.ends_with('.')))
}

// A name on the internet, lower case: labels of a-z 0-9 -, at least two, the
// last with a letter (so no address, 127.0.0.1 nor 2130706433), not at home.
fn is_public_name(host: &str) -> bool {
    let labels: Vec<&str> = host.split('.').collect();
    host.len() <= 253
        && labels.len() >= 2
        && labels.iter().all(|l| (1..=63).contains(&l.len()) && !l.starts_with('-') && !l.ends_with('-') && l.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'))
        && labels.last().is_some_and(|tld| tld.bytes().any(|b| b.is_ascii_lowercase()))
        && !is_home_name(host)
}

// A host its manifest may name: a name, or *.<name>; no port, no address.
pub fn is_host_rule(rule: &str) -> bool {
    is_public_name(rule.strip_prefix("*.").unwrap_or(rule))
}

// Whether the hosts named let it reach this one: the same name, or one
// under *.<name> (not <name> itself).
pub fn host_allowed(rules: &[String], host: &str) -> bool {
    rules.iter().any(|r| match r.strip_prefix("*.") {
        Some(base) => host.strip_suffix(base).is_some_and(|rest| rest.len() > 1 && rest.ends_with('.')),
        None => host == r,
    })
}

// An address of this computer or of the network at home: loopback, private,
// link-local, carrier-grade NAT, unspecified, multicast, broadcast; IPv6's
// unique-local and link-local, and an IPv4 one reachable through IPv6 (a
// mapped or compatible ::a.b.c.d, a 6to4 2002::/16 or a NAT64 64:ff9b::/96
// address carrying one of those IPv4 addresses).
pub(crate) fn is_inside(ip: std::net::IpAddr) -> bool {
    use std::net::{IpAddr, Ipv4Addr};
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified() || v4.is_broadcast() || v4.is_multicast() || a == 0 || (a == 100 && (64..128).contains(&b))
        }
        IpAddr::V6(v6) => {
            let seg = v6.segments();
            let first = seg[0];
            // An IPv4 address carried in this IPv6 one, if any: a mapped or
            // compatible ::a.b.c.d (to_ipv4), a 6to4 2002:AABB:CCDD::, or the
            // well-known NAT64 prefix 64:ff9b::/96.
            let in_v4 = |a, b, c, d| Ipv4Addr::new(a, b, c, d);
            let carried = if first == 0x2002 {
                Some(in_v4((seg[1] >> 8) as u8, seg[1] as u8, (seg[2] >> 8) as u8, seg[2] as u8))
            } else if first == 0x0064 && seg[1] == 0xff9b && seg[2..6].iter().all(|&s| s == 0) {
                Some(in_v4((seg[6] >> 8) as u8, seg[6] as u8, (seg[7] >> 8) as u8, seg[7] as u8))
            } else {
                v6.to_ipv4()
            };
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80
                || carried.is_some_and(|v4| is_inside(IpAddr::V4(v4)))
        }
    }
}

// Where a request goes: http or https, a name, port 80 or 443, its path.
#[derive(Debug, PartialEq)]
struct Target {
    url: String,
    host: String,
    port: u16,
}

// An address a plugin gives, checked: http(s), a name (no address, no user
// or password), port 80 or 443; its fragment left off.
fn target_of(url: &str) -> Result<Target, String> {
    let url = url.trim();
    let (scheme, rest) = url.split_once("://").ok_or("not an http(s) address")?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err("only http and https".into());
    }
    let rest = rest.split('#').next().unwrap_or("");
    let end = rest.find(['/', '?']).unwrap_or(rest.len());
    let (authority, path) = rest.split_at(end);
    if authority.contains('@') {
        return Err("no user or password in the address".into());
    }
    if authority.starts_with('[') {
        return Err("an address, not a name".into());
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().map_err(|_| "a port that is none")?),
        None => (authority, if scheme == "https" { 443 } else { 80 }),
    };
    if port != 80 && port != 443 {
        return Err(format!("port {port}: only 80 and 443"));
    }
    let host = host.to_ascii_lowercase();
    if host.parse::<std::net::Ipv4Addr>().is_ok() || !is_public_name(&host) {
        return Err(format!("{host}: not a name on the internet"));
    }
    if path.chars().any(|c| c.is_whitespace() || c.is_control() || c == '\\') {
        return Err("spaces in the address: encode them".into());
    }
    let shown_port = if (scheme == "https" && port == 443) || (scheme == "http" && port == 80) { String::new() } else { format!(":{port}") };
    let path = if path.is_empty() { "/" } else { path };
    let path = if path.starts_with('?') { format!("/{path}") } else { path.to_string() };
    Ok(Target { url: format!("{scheme}://{host}{shown_port}{path}"), host, port })
}

// Where a redirect goes from `from`: its Location made whole.
fn redirected(from: &str, to: &str) -> Option<String> {
    let to = to.trim();
    let (scheme, rest) = from.split_once("://")?;
    let origin = format!("{scheme}://{}", rest.split(['/', '?']).next().unwrap_or(""));
    Some(if to.contains("://") {
        to.to_string()
    } else if let Some(net) = to.strip_prefix("//") {
        format!("{scheme}://{net}")
    } else if to.starts_with('/') {
        format!("{origin}{to}")
    } else {
        let path = rest.split('?').next().unwrap_or("");
        let dir = path.rfind('/').map_or(String::new(), |i| path[..=i].to_string());
        let dir = dir.split_once('/').map_or("/".to_string(), |(_, d)| format!("/{d}"));
        format!("{origin}{dir}{to}")
    })
}

// One way to ask the web, for a plugin (addon_fetch).
pub struct Ask {
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

// The request a plugin asked for, made as its permissions let it: each hop
// (the address, then each redirect) a host its manifest names, a name on
// the internet at port 80 or 443, none of whose addresses is this
// computer's or the network's at home; the way her other requests go
// (proxy::web_ways), each tried in turn. { status, ok, headers, text }, or
// what would not do.
pub fn fetch(perms: &Perms, ask: Ask) -> Result<Value, String> {
    let method = ask.method.trim().to_ascii_uppercase();
    let method = if method.is_empty() { "GET".to_string() } else { method };
    if !matches!(method.as_str(), "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD") {
        return Err(format!("{method}: GET, POST, PUT, PATCH, DELETE or HEAD"));
    }
    let mut body = ask.body.filter(|b| !b.is_empty());
    if body.as_ref().is_some_and(|b| b.len() > FETCH_MAX) {
        return Err("a body over 1 MB".into());
    }
    if body.is_some() && matches!(method.as_str(), "GET" | "HEAD") {
        return Err(format!("a {method} has no body"));
    }
    let mut headers = Vec::new();
    for (k, v) in ask.headers.into_iter().take(32) {
        let name = k.trim().to_ascii_lowercase();
        let ok = !name.is_empty() && name.len() <= 64 && name.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_!#$%&'*+.^`|~".contains(&b));
        if !ok || v.len() > 4096 || v.chars().any(|c| c == '\r' || c == '\n' || c == '\0') {
            return Err(format!("{k}: not a header"));
        }
        if !HEADERS_NOT.contains(&name.as_str()) {
            headers.push((name, v));
        }
    }
    if !headers.iter().any(|(k, _)| k == "user-agent") {
        headers.push(("user-agent".into(), format!("wakuwaku/{}", env!("CARGO_PKG_VERSION"))));
    }
    let until = std::time::Instant::now() + FETCH_TIME;
    let mut method = method;
    let mut url = ask.url;
    for _ in 0..=HOPS {
        let target = target_of(&url)?;
        if !host_allowed(&perms.net, &target.host) {
            return Err(format!("{}: not a host its manifest names (permissions.net)", target.host));
        }
        if resolves_inside(&target.host, target.port) {
            return Err(format!("{}: an address of this computer or the network at home", target.host));
        }
        let res = send(&target.url, &method, &headers, body.as_deref(), until)?;
        let status = res.status().as_u16();
        if res.status().is_redirection() && status != 304 {
            let to = res.headers().get("location").and_then(|l| l.to_str().ok()).and_then(|l| redirected(&target.url, l)).ok_or(format!("a {status} to nowhere"))?;
            // As a browser does: 303 (and 301, 302 after a POST) as a GET, without its body.
            if status == 303 || (matches!(status, 301 | 302) && method == "POST") {
                method = "GET".into();
                body = None;
            }
            url = to;
            continue;
        }
        return answer(res);
    }
    Err(format!("more than {HOPS} redirects"))
}

// Whether a name is one of this computer or of the network at home, as the
// system looks it up now (a name of the internet made to point there). Not
// found here, it is left to the way it goes (a proxy may know it).
fn resolves_inside(host: &str, port: u16) -> bool {
    use std::net::ToSocketAddrs;
    (host, port).to_socket_addrs().is_ok_and(|mut addrs| addrs.any(|a| is_inside(a.ip())))
}

fn send(url: &str, method: &str, headers: &[(String, String)], body: Option<&str>, until: std::time::Instant) -> Result<ureq::http::Response<ureq::Body>, String> {
    // A request that is not idempotent (a POST and the like) may have reached
    // the server and been acted on even when its answer never came, so it is
    // tried another way only when the first never opened its connection
    // (a proxy down, the name not found): else one plugin POST could be sent
    // twice. A GET or HEAD may always be tried again.
    let idempotent = matches!(method, "GET" | "HEAD");
    let mut last = "no way to the web".to_string();
    for way in crate::proxy::web_ways(url) {
        let left = until.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            return Err("timed out".into());
        }
        let Some(agent) = way.guarded_agent(left, |c| c.http_status_as_error(false).max_redirects(0).max_redirects_will_error(false)) else { continue };
        let mut req = ureq::http::Request::builder().method(method).uri(url);
        for (k, v) in headers {
            req = req.header(k.as_str(), v.as_str());
        }
        let sent = match body {
            Some(b) => req.body(b.to_string()).map_err(|e| e.to_string()).map(|r| agent.run(r)),
            None => req.body(()).map_err(|e| e.to_string()).map(|r| agent.run(r)),
        };
        match sent {
            Err(e) => return Err(e), // the request could not be built
            Ok(Ok(res)) => return Ok(res),
            // The connection had opened and this is not idempotent: the
            // server may have acted on it, so it is not tried another way.
            Ok(Err(e)) if !idempotent && !before_server(&e) => return Err(words(&e)),
            Ok(Err(e)) => last = words(&e),
        }
    }
    Err(last)
}

// Whether a failure came before the request could reach the server: the
// connection was never opened (the name not found, the connection refused
// or timed out while opening, no proxy to go through). Such a request is
// safe to try another way even when it is not idempotent.
fn before_server(e: &ureq::Error) -> bool {
    match e {
        ureq::Error::Timeout(t) => matches!(t, ureq::Timeout::Connect | ureq::Timeout::Resolve),
        ureq::Error::HostNotFound | ureq::Error::ConnectionFailed => true,
        ureq::Error::Io(io) => matches!(io.kind(), std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::AddrNotAvailable | std::io::ErrorKind::NetworkUnreachable | std::io::ErrorKind::HostUnreachable),
        _ => false,
    }
}

fn words(e: &ureq::Error) -> String {
    match e {
        ureq::Error::Io(io) => io.to_string(),
        ureq::Error::Timeout(_) => "timed out".into(),
        ureq::Error::HostNotFound => "host not found".into(),
        other => other.to_string(),
    }
}

fn answer(mut res: ureq::http::Response<ureq::Body>) -> Result<Value, String> {
    let status = res.status().as_u16();
    let mut headers = Map::new();
    for name in HEADERS_BACK {
        if let Some(v) = res.headers().get(name).and_then(|v| v.to_str().ok()) {
            headers.insert(name.into(), json!(v));
        }
    }
    let bytes = res.body_mut().with_config().limit(FETCH_MAX as u64).read_to_vec().map_err(|e| match e {
        ureq::Error::BodyExceedsLimit(_) => "an answer over 1 MB".to_string(),
        e => words(&e),
    })?;
    Ok(json!({ "status": status, "ok": (200..300).contains(&status), "headers": headers, "text": String::from_utf8_lossy(&bytes) }))
}

// --- Which are here: the registry -------------------------------------------------------

// A folder in plugins/: its manifest read (or why not), as of its file's time and size.
struct Found {
    manifest: Result<Manifest, String>,
    stamp: Option<(SystemTime, u64)>,
}

#[derive(Default)]
pub struct Registry {
    scanned: bool,
    found: BTreeMap<String, Found>,
    // installed.json: { id: { source, reviewed, sha256, version, grantedPerms, installedAt } }.
    records: Map<String, Value>,
}

static REGISTRY: LazyLock<RwLock<Registry>> = LazyLock::new(|| RwLock::new(Registry::default()));

// One lock per package id, held for the whole of an install, update or
// uninstall of it (finding 8): a file install, a market install or update,
// a pack install, and an uninstall of the same id never run at once and
// leave its folder or record half-written. Keyed by id alone across kinds:
// conservative, so even a pet and a plugin of one id do not race.
static LOCKS: LazyLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub(crate) fn with_id_lock<T>(id: &str, f: impl FnOnce() -> T) -> T {
    let lock = LOCKS.lock().unwrap_or_else(|e| e.into_inner()).entry(id.to_string()).or_default().clone();
    let _held = lock.lock().unwrap_or_else(|e| e.into_inner());
    f()
}

fn stamp_of(path: &Path) -> Option<(SystemTime, u64)> {
    let m = std::fs::metadata(path).ok()?;
    Some((m.modified().ok()?, m.len()))
}

fn plugins_dir(dir: &Path) -> PathBuf {
    dir.join("plugins")
}

// The id in a ".old-<id>-<n>" folder's name (<n> the time it was set aside);
// None for any other name.
fn old_id_of(name: &str) -> Option<String> {
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

fn records_file(dir: &Path) -> PathBuf {
    plugins_dir(dir).join("installed.json")
}

// A folder's manifest read, and the files it names there.
fn read_manifest(folder: &Path, name: &str) -> Result<Manifest, String> {
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
    fn refresh(&mut self, dir: &Path) -> (bool, Vec<String>) {
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

    fn manifest(&self, id: &str) -> Option<&Manifest> {
        self.found.get(id).and_then(|f| f.manifest.as_ref().ok())
    }

    // The widget's plugin: `<id>` or `<id>_<sub>`, its folder here.
    fn owner_of(&self, widget: &str) -> Option<String> {
        let id = widget.split_once('_').map_or(widget, |(id, _)| id);
        self.found.contains_key(id).then(|| id.to_string())
    }

    // A "plugins" setting's entry for one here (None: no such folder): "on",
    // and each value by its manifest's field; one whose manifest will not do
    // (or waits for a newer API) keeps what it had, as long as it is text,
    // a number or a switch.
    fn conf_ok(&self, id: &str, conf: &Value) -> Option<bool> {
        let found = self.found.get(id)?;
        let Some(conf) = conf.as_object() else { return Some(false) };
        Some(conf.iter().all(|(k, v)| self.key_ok(found, k, v)))
    }

    // Whether one key of a plugin's entry is kept: "on" a switch; a value by
    // its manifest's field (one of a newer API, or one that will not read,
    // keeps what it had as long as it is text, a number or a switch).
    fn key_ok(&self, found: &Found, k: &str, v: &Value) -> bool {
        let scalar = |v: &Value| v.is_boolean() || v.as_f64().is_some() || v.as_str().is_some_and(|t| t.chars().count() <= 300);
        match (&found.manifest, k) {
            (_, "on") => v.is_boolean(),
            (Ok(m), _) if m.api <= API => m.field(k).is_some_and(|f| f.accepts(v)),
            _ => k.len() <= 32 && k.bytes().all(|b| b.is_ascii_alphanumeric()) && scalar(v),
        }
    }

    // A third-party plugin's entry cleaned: its keys that will not do dropped
    // (finding 3); None when no such folder is here (the id stale).
    fn clean_conf(&self, id: &str, conf: &Map<String, Value>) -> Option<Map<String, Value>> {
        let found = self.found.get(id)?;
        Some(conf.iter().filter(|(k, v)| self.key_ok(found, k, v)).map(|(k, v)| (k.clone(), v.clone())).collect())
    }

    fn record(&self, id: &str) -> &Value {
        self.records.get(id).unwrap_or(&Value::Null)
    }

    // What the plugin was granted when it went in (installed.json
    // grantedPerms): a market install's index permissions, a file install's
    // the confirm layer's (its manifest's), a newer-API one's what little it
    // said of itself. A record from before this was kept, or none at all (a
    // folder copied in by hand), is read as granting its current manifest's,
    // so it is not asked out of the blue (migrate_grants writes it once).
    fn granted(&self, id: &str) -> Perms {
        match self.record(id).get("grantedPerms") {
            Some(g) => perms_of(g).unwrap_or_default(),
            None => self.manifest(id).map(|m| m.perms.clone()).unwrap_or_default(),
        }
    }

    // What a running plugin's manifest asks for beyond what was granted: none
    // means it may run; else it waits for the person to allow it (needsApproval).
    fn needs_approval(&self, id: &str, m: &Manifest) -> Vec<String> {
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
    fn allowed(&self, id: &str) -> Option<Perms> {
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
    fn rev(&self, id: &str) -> String {
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
fn migrate_grants(sh: &Shared) {
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
    let stored = sh.setting(config::PLUGINS);
    let cleaned = crate::plugins::clean(&stored);
    if cleaned != stored {
        shared(&sh.app).change(json!({ "plugins": cleaned }));
    }
}

fn read<T>(f: impl FnOnce(&Registry) -> T) -> T {
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

fn records_write(dir: &Path, f: impl FnOnce(&mut Map<String, Value>)) {
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

// --- Running: which the island's page runs ----------------------------------------------

#[derive(Default)]
struct Run {
    // What the page was told last (addons:changed), to tell it only changes.
    sent: Option<Value>,
    // What went wrong last in each (an uncaught error, waku.log's warning).
    said: HashMap<String, String>,
    // Those stopped by the island's watchdog (addons.ts), until turned on
    // again (revive) or installed anew.
    stopped: HashMap<String, Stopped>,
    // Each plugin's views' messages to its main this second (addon_view
    // send): when it began, how many.
    sends: HashMap<String, (u64, u32)>,
}

// A plugin stopped, and why: hung (its frame did not answer the watchdog,
// again within ten minutes of being loaded again for it), crashed (its page
// loaded again and again), busy (over its long tasks' budget); when, since
// when it was not answering, how many times it did not.
#[derive(Clone, Debug, PartialEq)]
struct Stopped {
    why: &'static str,
    at: u64,
    since: u64,
    times: u64,
}

static RUN: LazyLock<Mutex<Run>> = LazyLock::new(|| Mutex::new(Run::default()));

fn run() -> std::sync::MutexGuard<'static, Run> {
    RUN.lock().unwrap_or_else(|e| e.into_inner())
}

// Whether a plugin with a view runs (the desktop's pages are told when their
// views may show only then: desk.rs views), and whether the island's
// watchdog has the views taken away for a while (pause_views).
static VIEWS: AtomicBool = AtomicBool::new(false);
static VIEWS_PAUSED: AtomicBool = AtomicBool::new(false);

pub fn views_running() -> bool {
    VIEWS.load(Ordering::Relaxed)
}

pub fn views_paused() -> bool {
    VIEWS_PAUSED.load(Ordering::Relaxed)
}

// Those the page should run now, each { id, version, rev, main, perms,
// settings }: on (placed), its required settings filled, a plugin of an API
// she has, not stopped.
fn wanted(sh: &Shared) -> Value {
    let conf = sh.setting(config::PLUGINS);
    let stopped: HashSet<String> = run().stopped.keys().cloned().collect();
    read(|r| {
        Value::Array(
            r.found
                .iter()
                .filter_map(|(id, f)| {
                    let m = f.manifest.as_ref().ok().filter(|m| m.runs())?;
                    let c = &conf[id.as_str()];
                    // It runs only when it is placed, its settings are filled,
                    // it is not stopped, and its manifest asks for nothing
                    // beyond what was granted (else it waits: needsApproval).
                    (c["on"] == true && m.missing(c).is_none() && !stopped.contains(id) && r.needs_approval(id, m).is_empty())
                        .then(|| json!({ "id": id, "version": m.version, "rev": r.rev(id), "main": m.main, "perms": r.allowed(id).unwrap_or_default().json(), "settings": m.values(c) }))
                })
                .collect(),
        )
    })
}

fn ids_of(list: &Value) -> HashSet<String> {
    list.as_array().into_iter().flatten().filter_map(|i| i["id"].as_str().map(str::to_string)).collect()
}

// The folders looked at again, and the page told which to run, when that
// changed; the widgets of those no longer running taken away. True when any
// widget went.
pub fn sync(sh: &Shared) -> bool {
    let (changed, gone) = REGISTRY.write().unwrap_or_else(|e| e.into_inner()).refresh(&sh.dir);
    for id in gone {
        forget(sh, &id, "its folder is gone");
    }
    let list = wanted(sh);
    let before = {
        let mut r = run();
        if r.sent.as_ref() == Some(&list) {
            if changed {
                sh.push_settings();
            }
            return false;
        }
        r.sent.replace(list.clone())
    };
    let _ = sh.app.emit_to("island", "addons:changed", &list);
    let now = ids_of(&list);
    note_views(sh, &now);
    let stopped: Vec<String> = before.map(|b| ids_of(&b)).unwrap_or_default().into_iter().filter(|id| !now.contains(id)).collect();
    sh.push_settings();
    if stopped.is_empty() {
        return false;
    }
    sh.log(&format!("addons: stopped {}", stopped.join(", ")));
    let mine = |w: &str| stopped.iter().any(|id| w == id || w.strip_prefix(id.as_str()).is_some_and(|rest| rest.starts_with('_')));
    sh.widgets.lock().unwrap().remove_addons(mine)
}

fn is_running(id: &str) -> bool {
    run().sent.as_ref().is_some_and(|l| ids_of(l).contains(id))
}

// Whether one of those running has a view; one just come, the desktop's
// pages told whether theirs may show (desk.rs views).
fn note_views(sh: &Shared, running: &HashSet<String>) {
    let views = read(|r| running.iter().any(|id| r.manifest(id).is_some_and(|m| m.view.is_some())));
    if views && !VIEWS.swap(true, Ordering::Relaxed) {
        crate::desk::views(sh);
    }
    VIEWS.store(views, Ordering::Relaxed);
}

// The plugin running whose widget this is, with its view's file: one drawn
// by a view (addon_view).
fn view_owner(widget: &str) -> Option<(String, String)> {
    let id = owner_of(widget)?;
    let view = read(|r| r.manifest(&id).filter(|m| m.runs()).and_then(|m| m.view.clone()))?;
    is_running(&id).then_some((id, view))
}

// The plugins installed, for the settings (the Plugins page), in her
// language: its words, where it came from, its permissions, whether it has
// a view of its own, how it is (state: off, needs (a setting), running,
// stopped, unsupported (a newer API)), what went wrong last, why it was
// stopped, and its settings as the page draws them.
pub fn view(sh: &Shared) -> Value {
    let conf = sh.setting(config::PLUGINS);
    let lang = sh.lang();
    let (said, stopped) = {
        let r = run();
        (r.said.clone(), r.stopped.clone())
    };
    read(|r| {
        Value::Array(
            r.found
                .iter()
                .filter_map(|(id, f)| {
                    let m = f.manifest.as_ref().ok()?;
                    let c = &conf[id.as_str()];
                    let on = c["on"] == true;
                    let missing = m.missing(c);
                    // It asks for more than was granted (an edited folder, a
                    // manifest that changed, a newer-API one now supported):
                    // it waits until the person allows it (finding 1, finding 7).
                    let more = r.needs_approval(id, m);
                    let state = match () {
                        _ if !m.runs() => "unsupported",
                        _ if !on => "off",
                        _ if missing.is_some() => "needs",
                        _ if !more.is_empty() => "needsApproval",
                        _ if stopped.contains_key(id) => "stopped",
                        _ => "running",
                    };
                    let rec = r.record(id);
                    let icon = match &m.icon {
                        Icon::None => Value::Null,
                        Icon::Named(n) => json!({ "name": n }),
                        Icon::Svg(f) => json!({ "url": format!("{ORIGIN}/{id}/{f}") }),
                    };
                    let settings: Vec<Value> = m
                        .settings
                        .iter()
                        .map(|f| {
                            let (options, labels) = match &f.kind {
                                FieldKind::Choice { options, labels } => (json!(options), json!(labels.iter().zip(options).map(|(l, o)| l.as_ref().map_or(o.as_str(), |l| l.get(lang))).collect::<Vec<_>>())),
                                _ => (Value::Null, Value::Null),
                            };
                            let set = &c[&f.key];
                            json!({
                                "key": f.key,
                                "type": f.type_name(),
                                "label": f.label.get(lang),
                                "hint": f.hint.as_ref().map(|h| h.get(lang)),
                                "required": f.required,
                                "value": if f.accepts(set) { set.clone() } else { Value::Null },
                                "default": f.default,
                                "options": options,
                                "optionLabels": labels,
                            })
                        })
                        .collect();
                    Some(json!({
                        "id": id,
                        "name": m.name.get(lang),
                        "description": m.description.as_ref().map_or(String::new(), |d| shown(d.get(lang), DESCRIPTION_SHOWN)),
                        "author": m.author,
                        "version": m.version,
                        "homepage": m.homepage,
                        "kind": m.kind.name(),
                        "api": m.api,
                        "category": m.category,
                        "sizes": m.sizes,
                        "color": m.color,
                        "icon": icon,
                        "view": m.view.is_some(),
                        // What it asks for, shown whatever its API (finding 7).
                        "perms": m.wants.json(),
                        "sensitive": m.wants.sensitive(),
                        // Copied in by hand, with no record: as one from a file.
                        "source": rec["source"].as_str().unwrap_or("file"),
                        "reviewed": rec["reviewed"] == true,
                        "installedAt": rec["installedAt"],
                        "on": on,
                        "state": state,
                        "missing": missing,
                        // The permissions it newly asks for (market.rs's words:
                        // "net:<host>", "agents", "hardware", "notify"), for the
                        // 「允许…」 layer (addonui.ts) when it needsApproval.
                        "newPerms": more,
                        "said": said.get(id),
                        // Stopped by the watchdog: why, when, since when it
                        // did not answer and how many times (addonui.ts says it).
                        "stopped": stopped.get(id).filter(|_| state == "stopped").map(|s| json!({ "why": s.why, "at": s.at, "since": s.since, "times": s.times })),
                        "settings": settings,
                    }))
                })
                .collect(),
        )
    })
}

// Words as they are shown: so many characters at most, longer cut with ….
pub(crate) fn shown(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max - 1).collect();
    out.push('…');
    out
}

// The folders whose manifest will not do, each with why.
pub fn broken_view() -> Value {
    read(|r| Value::Array(r.found.iter().filter_map(|(id, f)| f.manifest.as_ref().err().map(|why| json!({ "folder": id, "reason": why }))).collect()))
}

// What its manifest asks for now: what approve grants. What it may do as it
// runs is Registry::allowed.
fn perms_of_id(id: &str) -> Option<Perms> {
    read(|r| r.manifest(id).map(|m| m.perms.clone()))
}

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

fn read_package(path: &Path) -> Result<Package, Refused> {
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
fn unpack(pkg: &Package, root: &Path) -> Result<(), String> {
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
fn is_widget_of(id: &str, widget: &str) -> bool {
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
fn widget_ids_of(sh: &Shared, id: &str) -> HashSet<String> {
    let mut ids: HashSet<String> = read(|r| r.record(id)["widgets"].as_array().into_iter().flatten().filter_map(|w| w.as_str().map(str::to_string)).collect());
    ids.extend(sh.widgets.lock().unwrap().addon_ids(|w| is_widget_of(id, w)));
    ids.insert(id.to_string());
    ids
}

// One widget a plugin made noted in its record (installed.json), so its
// places are cleaned on uninstall even when it is not running. Up to the
// most it may have (WIDGETS); nothing without a record (copied in by hand).
fn record_widget(sh: &Shared, id: &str, wid: &str) {
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
fn forget(sh: &Shared, id: &str, why: &str) {
    let widgets = widget_ids_of(sh, id);
    let patch = without_plugin(&sh.settings.lock().unwrap().clone(), id, &widgets);
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

// --- Storage -----------------------------------------------------------------------------

static STORES: LazyLock<Mutex<HashMap<PathBuf, Map<String, Value>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn stores() -> std::sync::MutexGuard<'static, HashMap<PathBuf, Map<String, Value>>> {
    STORES.lock().unwrap_or_else(|e| e.into_inner())
}

fn store_file(dir: &Path, id: &str) -> PathBuf {
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

// --- Its files: her scheme -------------------------------------------------------------

// The policy each of its files is served with: scripts, styles, pictures,
// fonts and sounds from its own folder (and data:, blob:), nothing else;
// no network, no frames, no forms, no workers (a worker runs off her page's
// main thread, out of the watchdog's long-task budget: none in v1); framed
// by her pages alone.
fn policy(id: &str) -> String {
    let own = format!("{ORIGIN}/{id}/");
    format!(
        "default-src 'none'; script-src {own} blob: data: 'wasm-unsafe-eval'; style-src {own} blob: data: 'unsafe-inline'; img-src {own} blob: data:; font-src {own} blob: data:; media-src {own} blob: data:; worker-src 'none'; connect-src 'none'; frame-src 'none'; object-src 'none'; form-action 'none'; base-uri 'none'; frame-ancestors http://tauri.localhost https://tauri.localhost"
    )
}

fn content_type(ext: &str) -> &'static str {
    match ext {
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "avif" => "image/avif",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "wav" => "audio/wav",
        "txt" => "text/plain; charset=utf-8",
        "md" => "text/markdown; charset=utf-8",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

// The SDK, the waku object of main.js (docs/PLUGINS.md): src/addon/sdk.ts
// as scripts/build-pages.mjs writes it (build.rs runs it first).
const SDK: &str = include_str!("../../dist/addon/sdk.js");

// A plugin frame's first page: the SDK alone, which then loads its main.
fn host_page(id: &str) -> String {
    format!("<!doctype html>\n<html><head><meta charset=\"utf-8\"><meta name=\"referrer\" content=\"no-referrer\"><title>{id}</title></head><body><script src=\"/{id}/__waku/sdk.js\"></script></body></html>\n")
}

// Its view.html (tier 2) as a card's frame gets it: her SDK the first
// script in its head (a head made for it where it has none), so the waku
// object is there before anything of the plugin's runs; the SDK knows it is
// a view's by its page (any but __waku/main.html).
fn view_page(id: &str, html: &str) -> String {
    let html = html.strip_prefix('\u{feff}').unwrap_or(html);
    let tag = format!("<script src=\"/{id}/__waku/sdk.js\"></script>");
    let lower = html.to_ascii_lowercase();
    let (at, text) = match (tag_end(&lower, "<head"), tag_end(&lower, "<html"), tag_end(&lower, "<!doctype")) {
        (Some(at), _, _) => (at, tag),
        (None, Some(at), _) | (None, None, Some(at)) => (at, format!("<head>{tag}</head>")),
        (None, None, None) => (0, format!("<head>{tag}</head>")),
    };
    format!("{}{text}{}", &html[..at], &html[at..])
}

// Where the first tag of that name (`<head`, not `<header`) ends, just
// after its `>`, in markup lowered as ASCII (its places as the original's).
fn tag_end(lower: &str, name: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(i) = lower[from..].find(name) {
        let at = from + i;
        if matches!(lower.as_bytes().get(at + name.len()), Some(b'>' | b'/' | b' ' | b'\t' | b'\n' | b'\r' | b'\x0c')) {
            return lower[at..].find('>').map(|j| at + j + 1);
        }
        from = at + name.len();
    }
    None
}

// A path in its folder as an address has it: each byte but letters,
// digits, - . _ ~ and / as %xx (decoded's other way).
fn url_path(path: &str) -> String {
    path.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

// %xx in a path made bytes again (UTF-8); None for one that is no text.
fn decoded(path: &str) -> Option<String> {
    let b = path.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

// A plugin's file for its frame, by her scheme: /<id>/<path> from its own
// folder (the types a package may hold, inside it), its view.html with her
// SDK in it (view_page), or her own two (__waku/main.html, __waku/sdk.js);
// each with the plugin's policy.
pub fn serve(app: &AppHandle, req: &tauri::http::Request<Vec<u8>>) -> tauri::http::Response<Vec<u8>> {
    let dir = app.try_state::<Arc<Shared>>().map(|s| s.dir.clone());
    respond(dir.as_deref(), req.method(), req.uri().path(), |id| read(|r| r.manifest(id).filter(|m| m.runs()).map(|m| m.view.clone())))
}

// serve's answer to a method and a path, her data folder known (None: she
// is not up yet); `view_of`: for a plugin here that runs, its view.html if
// it has one (None: no such plugin to serve).
fn respond(dir: Option<&Path>, method: &tauri::http::Method, path: &str, view_of: impl Fn(&str) -> Option<Option<String>>) -> tauri::http::Response<Vec<u8>> {
    let head = method == tauri::http::Method::HEAD;
    if method != tauri::http::Method::GET && !head {
        return status(405, "GET only");
    }
    let Some(dir) = dir else { return status(503, "not up yet") };
    let (id, file) = path.trim_start_matches('/').split_once('/').unwrap_or((path.trim_start_matches('/'), ""));
    let id = id.to_string();
    let Some(view) = (if is_id(&id) { view_of(&id) } else { None }) else {
        return status(404, "no such plugin");
    };
    let (body, kind) = match file {
        "__waku/main.html" => (host_page(&id).into_bytes(), "text/html; charset=utf-8"),
        "__waku/sdk.js" => (SDK.as_bytes().to_vec(), "text/javascript; charset=utf-8"),
        f if f.starts_with("__waku/") => return status(404, "not found"),
        f => match own_file(&plugins_dir(&dir).join(&id), f) {
            Some((bytes, kind)) if view.is_some() && decoded(f) == view => (view_page(&id, &String::from_utf8_lossy(&bytes)).into_bytes(), kind),
            Some(found) => found,
            None => return status(404, "not found"),
        },
    };
    let mut res = tauri::http::Response::builder()
        .status(200)
        .header("content-type", kind)
        .header("content-security-policy", policy(&id))
        .header("access-control-allow-origin", "*")
        .header("x-content-type-options", "nosniff")
        .header("cache-control", "no-store")
        .header("referrer-policy", "no-referrer")
        .header("x-dns-prefetch-control", "off");
    if head {
        res = res.header("content-length", body.len().to_string());
    }
    res.body(if head { Vec::new() } else { body }).unwrap_or_else(|_| status(500, "no answer"))
}

// A file of its folder by the path asked: its parts safe, a type a package
// may hold, inside the folder when looked up whole (no link out of it).
fn own_file(folder: &Path, path: &str) -> Option<(Vec<u8>, &'static str)> {
    let path = decoded(path)?;
    let parts = zip::safe(&path)?;
    let ext = ext_of(&path);
    if !EXTENSIONS.contains(&ext.as_str()) {
        return None;
    }
    let file = parts.iter().fold(folder.to_path_buf(), |p, part| p.join(part));
    let (root, whole) = (folder.canonicalize().ok()?, file.canonicalize().ok()?);
    if !whole.starts_with(&root) || !whole.is_file() || std::fs::metadata(&whole).ok()?.len() > PLUGIN_ALL {
        return None;
    }
    Some((std::fs::read(&whole).ok()?, content_type(&ext)))
}

fn status(code: u16, words: &str) -> tauri::http::Response<Vec<u8>> {
    tauri::http::Response::builder().status(code).header("content-type", "text/plain; charset=utf-8").header("access-control-allow-origin", "*").body(words.as_bytes().to_vec()).unwrap_or_default()
}

// The pages a plugin's frame may show: its own first page (main: in the
// island's page alone), and its view.html (a card's, there or on the
// desktop), as served, nothing after them.
pub fn is_frame_page(uri: &str, main: bool) -> bool {
    read(|r| r.is_frame_page(uri, main))
}

impl Registry {
    fn is_frame_page(&self, uri: &str, main: bool) -> bool {
        let Some(rest) = uri.strip_prefix(ORIGIN).and_then(|r| r.strip_prefix('/')) else { return false };
        let Some((id, file)) = rest.split_once('/') else { return false };
        is_id(id) && self.manifest(id).is_some_and(|m| m.runs() && ((main && file == "__waku/main.html") || (m.view.is_some() && decoded(file) == m.view)))
    }
}

// The pages that may ask for a card's view (addon_view): her island's, and
// the desktop's.
fn may_view(label: &str) -> bool {
    label == "island" || label.strip_prefix("desk-").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

// A plugin's views' messages this second ((since, how many)): one more, if
// it may (ten a second).
fn may_send(sent: &mut (u64, u32), now: u64) -> bool {
    if now.saturating_sub(sent.0) >= 1000 {
        *sent = (now, 0);
    }
    if sent.1 >= 10 {
        return false;
    }
    sent.1 += 1;
    true
}

// A page's frames kept where they were put: a frame (a plugin's) sent
// anywhere but a plugin's own page (is_frame_page; main: the island's,
// which runs their main.js too, the desktop's only views) is not, so one
// cannot leave its sandbox's rules by going to another address, nor carry
// what it knows there in the address. Said in her log, once in a while.
#[cfg(windows)]
pub fn guard_frames(sh: &Arc<Shared>, window: &WebviewWindow, main: bool) {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2NavigationStartingEventArgs;
    use webview2_com::NavigationStartingEventHandler;
    let sh = sh.clone();
    let _ = window.with_webview(move |view| {
        // SAFETY: WebView2's own objects, on its thread, as it hands them.
        unsafe {
            let Ok(core) = view.controller().CoreWebView2() else { return };
            let said = Mutex::new(None::<std::time::Instant>);
            let handler = NavigationStartingEventHandler::create(Box::new(move |_, args: Option<ICoreWebView2NavigationStartingEventArgs>| {
                let Some(args) = args else { return Ok(()) };
                let mut uri = windows_core::PWSTR::null();
                args.Uri(&mut uri)?;
                let uri = webview2_com::take_pwstr(uri);
                if !is_frame_page(&uri, main) {
                    args.SetCancel(true)?;
                    let mut last = said.lock().unwrap_or_else(|e| e.into_inner());
                    if last.is_none_or(|t| t.elapsed() > Duration::from_secs(10)) {
                        *last = Some(std::time::Instant::now());
                        let shown: String = uri.chars().take(80).collect();
                        sh.log(&format!("addons: a frame sent to {shown} was kept where it was"));
                    }
                }
                Ok(())
            }));
            let mut token = 0i64;
            let _ = core.add_FrameNavigationStarting(&handler, &mut token);
        }
    });
}

#[cfg(not(windows))]
pub fn guard_frames(_sh: &Arc<Shared>, _window: &WebviewWindow, _main: bool) {}

// --- From the island's page --------------------------------------------------------------
// Four commands for all of it (each command costs the exe some KB of glue):
// which to run, what a running plugin asks (in its name, as the page knows
// it by its frame) and what the watchdog does to one, what the settings do
// with packages; and from any page holding a card drawn by a view, that
// view's data and its messages (addon_view).

// Which to run, now (the page as it starts; addons:changed as it changes).
#[tauri::command]
pub async fn addons_running(app: AppHandle) -> Value {
    let sh = shared(&app);
    let list = wanted(&sh);
    run().sent = Some(list.clone());
    note_views(&sh, &ids_of(&list));
    list
}

// A running plugin's asking (src/pet/addons.ts), from the island's page
// alone: widget { sub, data }, remove { sub }, fetch { url, method, headers,
// body }, storage { op, key, value }, notify { text }, hardware, said {
// line }. And its watchdog's: stop { why, since, times } (one not answering
// or too busy, stopped until turned on again), views { on } (every view
// taken away while it starts the plugins' frames anew, and back; id "").
// Its answer, or { error }.
#[tauri::command]
pub async fn addon_call(app: AppHandle, window: WebviewWindow, id: String, what: String, args: Value) -> Value {
    if window.label() != "island" {
        return json!({ "error": "the island's page only" });
    }
    let sh = shared(&app);
    tauri::async_runtime::spawn_blocking(move || call(&sh, &id, &what, &args)).await.unwrap_or_else(|e| json!({ "error": e.to_string() }))
}

fn call(sh: &Shared, id: &str, what: &str, a: &Value) -> Value {
    let s = |k: &str| a[k].as_str();
    if what == "said" {
        if is_known(id) {
            said(sh, id, s("line").unwrap_or(""));
        }
        return Value::Null;
    }
    if what == "views" {
        pause_views(sh, a["on"] == false);
        return Value::Null;
    }
    if !is_running(id) {
        return json!({ "error": format!("{id}: not running") });
    }
    let perms = read(|r| r.allowed(id)).unwrap_or_default();
    let got = match what {
        "stop" => stop(sh, id, s("why").unwrap_or(""), a["since"].as_u64(), a["times"].as_u64()),
        "widget" => widget(sh, id, s("sub"), &a["data"]),
        "remove" => widget_id(id, s("sub")).map(|wid| {
            if sh.widgets.lock().unwrap().remove_addons(|w| w == wid) {
                sh.redraw_widgets();
            }
            json!({ "ok": true })
        }),
        "fetch" => {
            let headers = a["headers"].as_object().into_iter().flatten().map(|(k, v)| (k.clone(), v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()))).collect();
            fetch(&perms, Ask { url: s("url").unwrap_or("").to_string(), method: s("method").unwrap_or("").to_string(), headers, body: s("body").map(str::to_string) })
        }
        "storage" => storage(&sh.dir, id, s("op").unwrap_or(""), s("key"), a.get("value").cloned()).map(|value| json!({ "value": value })),
        "notify" if perms.notify => notify(sh, id, s("text").unwrap_or("")),
        "hardware" if perms.hardware => Ok(crate::widgets::reading_now()),
        "notify" | "hardware" => Err(format!("no {what} permission")),
        _ => Err(format!("{what}: not a plugin's asking")),
    };
    got.unwrap_or_else(|e| json!({ "error": e }))
}

// The widget id for a plugin's sub (None or "": its own).
fn widget_id(id: &str, sub: Option<&str>) -> Result<String, String> {
    match sub.filter(|s| !s.is_empty()) {
        None => Ok(id.to_string()),
        Some(s) if s.len() <= 16 && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()) => {
            let w = format!("{id}_{s}");
            if w.len() > 40 {
                return Err(format!("sub: {w} is longer than 40 characters"));
            }
            Ok(w)
        }
        Some(_) => Err("sub: a-z 0-9, 16 at most".into()),
    }
}

// A plugin's widget shown (waku.widget.set): as POST /widget takes one, its
// label its name if none, its manifest's sizes, colour and icon where it
// gives none (its .svg by her scheme, for the pages to draw); 8 at most.
fn widget(sh: &Shared, id: &str, sub: Option<&str>, data: &Value) -> Result<Value, String> {
    let wid = widget_id(id, sub)?;
    let mut data = data.as_object().cloned().ok_or("a widget: { label, value, detail, … }")?;
    let lang = sh.lang();
    let (name, sizes, color, icon, picture, has_view) = read(|r| {
        let m = r.manifest(id);
        let (icon, picture) = match m.map(|m| &m.icon) {
            Some(Icon::Named(n)) => (Some(n.clone()), None),
            Some(Icon::Svg(f)) => (None, Some(format!("{ORIGIN}/{id}/{f}"))),
            _ => (None, None),
        };
        (m.map(|m| m.name.get(lang).to_string()).unwrap_or_default(), m.map(|m| m.sizes.clone()).unwrap_or_default(), m.and_then(|m| m.color.clone()), icon, picture, m.is_some_and(|m| m.view.is_some()))
    });
    for k in ["id", "ttl", "nudge", "remove"] {
        data.remove(k);
    }
    if let (false, Some(c)) = (data.contains_key("color"), color) {
        data.insert("color".into(), json!(c));
    }
    if let (false, Some(i)) = (data.contains_key("icon"), icon) {
        data.insert("icon".into(), json!(i));
    }
    match data.get_mut("detail") {
        Some(Value::Object(d)) if !d.contains_key("sizes") => {
            d.insert("sizes".into(), json!(sizes));
        }
        None => {
            data.insert("detail".into(), json!({ "sizes": sizes }));
        }
        _ => {}
    }
    {
        let mut widgets = sh.widgets.lock().unwrap();
        if widgets.view_of(&wid).is_none() && widgets.addons_of(|w| is_widget_of(id, w)) >= WIDGETS {
            return Err("8 widgets at most".into());
        }
        widgets.put_addon(&wid, &Value::Object(data), &name, picture, has_view, now_ms())?;
    }
    // Noted as one of its own, so its place is cleaned on uninstall.
    record_widget(sh, id, &wid);
    sh.redraw_widgets();
    Ok(json!({ "ok": true }))
}

// One the watchdog gave up on (addons.ts): stopped until turned on again,
// with why (hung, crashed, busy), since when it did not answer, how many
// times; its frame and its widgets gone with it (sync).
fn stop(sh: &Shared, id: &str, why: &str, since: Option<u64>, times: Option<u64>) -> Result<Value, String> {
    let why = match why {
        "hung" => "hung",
        "crashed" => "crashed",
        "busy" => "busy",
        _ => return Err("why: hung, crashed or busy".into()),
    };
    let at = now_ms();
    // Since when: within the last hour, else now.
    let since = since.filter(|t| *t <= at && at - *t <= 3_600_000).unwrap_or(at);
    let times = times.unwrap_or(0).min(99);
    run().stopped.insert(id.to_string(), Stopped { why, at, since, times });
    sh.log(&format!("addons: {id} stopped by the watchdog: {why}{}", if times > 0 { format!(", {times} pings unanswered") } else { String::new() }));
    if crate::plugins::sync(sh) {
        sh.redraw_widgets();
    }
    Ok(json!({ "ok": true }))
}

// Every view taken away a while (the island's watchdog starting the
// plugins' frames anew: the one process they share must end, a view's frame
// too), or back; the desktop's pages told (desk.rs views).
fn pause_views(sh: &Shared, paused: bool) {
    if VIEWS_PAUSED.swap(paused, Ordering::Relaxed) != paused {
        sh.log(&format!("addons: views {}", if paused { "taken away a while" } else { "back" }));
        crate::desk::views(sh);
    }
}

// A card's view (src/pet/views.ts), from her island's page or a desktop's:
// data, the page to show for a widget drawn by its plugin's view and what
// to hand it ({ page, data, rev }); send { message }, a message of the view
// for its plugin's main.js, told the island's page (addons:view { id, sub,
// message }), as the page holding the view knows it by its frame. Ten a
// second for each plugin at most, 64 KB each. { error } when not.
#[tauri::command]
pub async fn addon_view(app: AppHandle, window: WebviewWindow, what: String, widget: String, message: Option<Value>) -> Value {
    if !may_view(window.label()) {
        return json!({ "error": "her pages only" });
    }
    let sh = shared(&app);
    view_call(&sh, &what, &widget, message).unwrap_or_else(|e| json!({ "error": e }))
}

fn view_call(sh: &Shared, what: &str, widget: &str, message: Option<Value>) -> Result<Value, String> {
    let (id, view) = view_owner(widget).ok_or_else(|| format!("{widget}: no view running"))?;
    match what {
        "data" => {
            let (data, rev) = sh.widgets.lock().unwrap().view_shown(widget).ok_or_else(|| format!("{widget}: no view given"))?;
            Ok(json!({ "page": format!("{ORIGIN}/{id}/{}", url_path(&view)), "data": data, "rev": rev }))
        }
        "send" => {
            let message = message.unwrap_or(Value::Null);
            if message.to_string().len() > 64 * 1024 {
                return Err("a message is JSON, up to 64 KB".into());
            }
            let now = now_ms();
            if !may_send(run().sends.entry(id.clone()).or_insert((now, 0)), now) {
                return Err("ten messages a second at most".into());
            }
            let sub = widget.strip_prefix(id.as_str()).and_then(|r| r.strip_prefix('_')).unwrap_or("");
            let _ = sh.app.emit_to("island", "addons:view", json!({ "id": id, "sub": sub, "message": message }));
            Ok(json!({ "ok": true }))
        }
        _ => Err(format!("{what}: data or send")),
    }
}

// A nudge on its widget (waku.notify, by its permission): the way a
// script's is, once a minute at most, none while a session needs you nor
// with plugins' nudges turned off (Shared::nudge_widget).
fn notify(sh: &Shared, id: &str, text: &str) -> Result<Value, String> {
    let words: String = text.trim().chars().filter(|c| !c.is_control()).take(60).collect();
    let shown = sh.widgets_view();
    let to = shown.as_array().into_iter().flatten().filter_map(|w| w["id"].as_str()).find(|w| *w == id).or_else(|| shown.as_array().into_iter().flatten().filter_map(|w| w["id"].as_str()).find(|w| is_widget_of(id, w))).map(str::to_string);
    let to = to.ok_or("no widget of it to nudge")?;
    if !sh.widgets.lock().unwrap().may_nudge(&format!("addon:{id}"), now_ms()) {
        return Err("once a minute at most".into());
    }
    sh.nudge_widget(&to, &words);
    Ok(json!({ "ok": true }))
}

// What went wrong in it last (an uncaught error, a warning), for the
// Plugins page as built-ins' said; in her log too.
fn said(sh: &Shared, id: &str, line: &str) {
    let line: String = line.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").chars().take(160).collect();
    sh.log(&format!("addons: {id}: {line}"));
    run().said.insert(id.to_string(), line);
    sh.push_settings();
}

// The settings' doings with packages: inspect (a .zip looked at for the
// page's confirm, nothing written; a pack whose pet or folder is someone
// else's, refused already), install (after it), uninstall (a plugin taken
// away, with its settings, storage and places), uninstallPack (a pack,
// arg "<kind>:<id>": packs.rs uninstall), pick (a .zip picked in Windows'
// own dialog over her island: { path }, null when closed), revive (one
// stopped turned on again). `arg`: the path, or the id.
#[tauri::command]
pub async fn addon_manage(app: AppHandle, what: String, arg: Option<String>, sha: Option<String>) -> Value {
    let sh = shared(&app);
    tauri::async_runtime::spawn_blocking(move || manage(&sh, &what, arg.as_deref().unwrap_or(""), sha.as_deref())).await.unwrap_or_else(|e| json!({ "ok": false, "error": e.to_string() }))
}

fn manage(sh: &Shared, what: &str, arg: &str, sha: Option<&str>) -> Value {
    let file = || Path::new(arg).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    match what {
        "inspect" => summary(&sh.dir, &read_package(Path::new(arg)).and_then(|p| crate::packs::may_go_in(&sh.dir, &p.manifest).map(|_| p)), sh.lang(), &file()),
        "install" => install(sh, Path::new(arg), sha).unwrap_or_else(|r| {
            sh.log(&format!("addons: {arg} not installed: {} ({})", r.detail, r.reason));
            json!({ "ok": false, "file": file(), "refused": r.json() })
        }),
        "uninstall" => match uninstall(sh, arg) {
            Ok(()) => json!({ "ok": true, "snapshot": crate::settings::snapshot(sh) }),
            Err(e) => json!({ "ok": false, "error": e }),
        },
        "uninstallPack" => {
            let (kind, id) = arg.split_once(':').unwrap_or(("", arg));
            match Kind::of(kind).filter(|k| *k != Kind::Plugin).ok_or_else(|| format!("{arg}: <pet|wallpaper>:<id>")).and_then(|kind| crate::packs::uninstall(sh, kind, id)) {
                Ok(()) => json!({ "ok": true, "snapshot": crate::settings::snapshot(sh) }),
                Err(e) => json!({ "ok": false, "error": e }),
            }
        }
        "pick" => {
            let lang = sh.lang();
            let picked = crate::paper::pick_file(&crate::i18n::t(lang, "addons.pickZip"), &crate::i18n::t(lang, "addons.zips"), "*.zip", crate::island::hwnd(sh));
            json!({ "path": picked })
        }
        "revive" => {
            {
                let mut r = run();
                r.stopped.remove(arg);
                r.said.remove(arg);
            }
            if crate::plugins::sync(sh) {
                sh.redraw_widgets();
            }
            crate::settings::snapshot(sh)
        }
        // The person allows a plugin the permissions its manifest now asks
        // for (the 「允许…」 layer): its grant set to its current manifest's,
        // so it runs again (finding 1).
        "approve" => {
            if let Some(perms) = perms_of_id(arg) {
                records_write(&sh.dir, |r| {
                    if let Some(rec) = r.get_mut(arg).and_then(Value::as_object_mut) {
                        rec.insert("grantedPerms".into(), perms.json());
                    }
                });
                sh.log(&format!("addons: {arg}'s new permissions allowed"));
                if crate::plugins::sync(sh) {
                    sh.redraw_widgets();
                }
            }
            crate::settings::snapshot(sh)
        }
        _ => json!({ "ok": false, "error": format!("{what}: inspect, install, uninstall, uninstallPack, pick, revive or approve") }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zip::tests::zip_of;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wakuwaku-addons-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn rain() -> Value {
        json!({
            "id": "rain-alert",
            "kind": "plugin",
            "name": { "en": "Rain alert", "zh": "下雨提醒" },
            "description": { "en": "Will it rain in the next hours, and when", "zh": "接下来几小时会不会下雨，几点开始" },
            "version": "1.2.0",
            "author": "lin",
            "homepage": "https://github.com/lin/rain-alert",
            "api": 1,
            "main": "main.js",
            "view": "view.html",
            "category": "info",
            "widget": { "sizes": ["s", "m"], "color": "#8cc8ff", "icon": "weather" },
            "settings": [
                { "key": "City", "type": "text", "required": true, "label": { "en": "City", "zh": "城市" }, "hint": { "en": "e.g. Dresden", "zh": "比如 Dresden" } },
                { "key": "Hours", "type": "choice", "options": ["3", "6", "12"], "default": "6", "label": { "en": "Look ahead", "zh": "往后看" } },
                { "key": "Alert", "type": "flag", "default": true, "label": { "en": "Tell me before it rains", "zh": "下雨前提醒我" } }
            ],
            "permissions": { "net": ["api.open-meteo.com"], "notify": true }
        })
    }

    fn with(f: impl FnOnce(&mut Value)) -> Result<Manifest, String> {
        let mut m = rain();
        f(&mut m);
        manifest_of(&m)
    }

    // The manifest in docs/PLUGINS.md reads as it says.
    #[test]
    fn the_documented_manifest_reads() {
        let m = manifest_of(&rain()).unwrap();
        assert_eq!((m.id.as_str(), m.kind, m.version.as_str(), m.api, m.category), ("rain-alert", Kind::Plugin, "1.2.0", 1, "info"));
        assert_eq!((m.name.get("zh"), m.name.get("en")), ("下雨提醒", "Rain alert"));
        assert_eq!((m.main.as_deref(), m.view.as_deref(), m.sizes.clone()), (Some("main.js"), Some("view.html"), vec!["s", "m"]));
        assert_eq!((m.color.as_deref(), m.icon.clone()), (Some("#8cc8ff"), Icon::Named("weather".into())));
        assert_eq!(m.perms, Perms { net: vec!["api.open-meteo.com".into()], agents: false, hardware: false, notify: true });
        assert!(m.perms.sensitive());
        assert_eq!(m.settings.iter().map(|f| (f.key.as_str(), f.type_name())).collect::<Vec<_>>(), [("City", "text"), ("Hours", "choice"), ("Alert", "flag")]);
        // Its values: the defaults until set; City needed.
        assert_eq!(m.values(&json!({})), json!({ "City": "", "Hours": "6", "Alert": true }));
        assert_eq!(m.missing(&json!({ "City": "  " })), Some("City"));
        assert_eq!(m.missing(&json!({ "City": "Dresden" })), None);
        assert_eq!(m.values(&json!({ "City": " Dresden ", "Hours": "12", "Alert": false })), json!({ "City": "Dresden", "Hours": "12", "Alert": false }));
        // A choice set to no option: its default.
        assert_eq!(m.values(&json!({ "Hours": "7" }))["Hours"], "6");
        // Least of all: a name, version, main, category.
        let small = manifest_of(&json!({ "id": "hi", "kind": "plugin", "name": "Hi", "version": "0.1.0", "api": 1, "main": "src/main.mjs", "category": "productivity" })).unwrap();
        assert_eq!((small.sizes.clone(), small.icon.clone(), small.perms.sensitive(), small.description.clone()), (vec!["s", "m"], Icon::None, false, None));
        assert_eq!(small.name.get("zh"), "Hi");
    }

    // The examples in the repository (examples/plugins) read as a folder of
    // hers would.
    #[test]
    fn the_examples_read() {
        for id in ["tally", "rain-alert", "agent-board"] {
            let m = read_manifest(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/plugins").join(id), id).unwrap();
            assert!(m.runs(), "{id}");
        }
        let rain = read_manifest(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/plugins/rain-alert"), "rain-alert").unwrap();
        assert_eq!(rain.perms.net, ["api.open-meteo.com", "geocoding-api.open-meteo.com"]);
        assert!(rain.perms.notify && !rain.perms.agents);
        // The one with a view of its own: the agents alone, and its view's files there.
        let board = read_manifest(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/plugins/agent-board"), "agent-board").unwrap();
        assert_eq!((board.view.as_deref(), board.sizes.clone()), (Some("view.html"), vec!["s", "m", "l"]));
        assert!(board.perms.agents && board.perms.net.is_empty() && !board.perms.notify && !board.perms.hardware);
        let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/plugins/agent-board");
        // Line-ending agnostic: a checkout with core.autocrlf=true has the
        // example's view.html in CRLF, view_page keeps the file's own endings.
        let html = std::fs::read_to_string(folder.join("view.html")).unwrap().replace("\r\n", "\n");
        assert!(view_page("agent-board", &html).contains("<head><script src=\"/agent-board/__waku/sdk.js\"></script>\n    <meta charset=\"utf-8\" />"));
        for f in ["view.js", "view.css"] {
            assert!(own_file(&folder, f).is_some(), "{f}");
        }
    }

    // A view.html as a card's frame gets it: her SDK the first script in
    // its head, whatever the head is like; none, one made for it.
    #[test]
    fn a_view_gets_her_sdk_first() {
        let tag = "<script src=\"/demo/__waku/sdk.js\"></script>";
        assert_eq!(view_page("demo", "<!doctype html><html><head><title>x</title></head><body></body></html>"), format!("<!doctype html><html><head>{tag}<title>x</title></head><body></body></html>"));
        assert_eq!(view_page("demo", "<HTML lang=en><HEAD data-x='1'><script src=a.js></script></HEAD>"), format!("<HTML lang=en><HEAD data-x='1'>{tag}<script src=a.js></script></HEAD>"));
        // <header> is no head; none: one made after <html>, else after the doctype, else first.
        assert_eq!(view_page("demo", "<html><body><header>hi</header></body></html>"), format!("<html><head>{tag}</head><body><header>hi</header></body></html>"));
        assert_eq!(view_page("demo", "<!DOCTYPE html>\n<div>hi</div>"), format!("<!DOCTYPE html><head>{tag}</head>\n<div>hi</div>"));
        assert_eq!(view_page("demo", "<div>你好</div>"), format!("<head>{tag}</head><div>你好</div>"));
        assert_eq!(view_page("demo", "\u{feff}<p>x</p>"), format!("<head>{tag}</head><p>x</p>"));
        assert_eq!(tag_end("<head\n  class=\"a\">", "<head"), Some(18));
        assert_eq!(tag_end("<headx><head>", "<head"), Some(13));
        assert_eq!(url_path("ui/view 1.html"), "ui/view%201.html");
        assert_eq!(url_path("界面.html"), "%E7%95%8C%E9%9D%A2.html");
        assert_eq!(decoded(&url_path("ui/界面 #1.html")).as_deref(), Some("ui/界面 #1.html"));
    }

    // The pages a plugin's frame may be at: its first page in the island's
    // page alone, its view.html (as served) in any; nothing else of its own,
    // nor another's, nor one that is no plugin's or does not run.
    #[test]
    fn a_frame_stays_at_its_own_pages() {
        let mut reg = Registry::default();
        reg.found.insert("rain-alert".into(), Found { manifest: Ok(manifest_of(&rain()).unwrap()), stamp: None });
        let mut ui = rain();
        ui["id"] = json!("board");
        ui["view"] = json!("ui/界面.html");
        reg.found.insert("board".into(), Found { manifest: Ok(manifest_of(&ui).unwrap()), stamp: None });
        let mut plain = rain();
        plain["id"] = json!("plain");
        plain.as_object_mut().unwrap().remove("view");
        reg.found.insert("plain".into(), Found { manifest: Ok(manifest_of(&plain).unwrap()), stamp: None });
        reg.found.insert("later".into(), Found { manifest: Ok(manifest_of(&json!({ "id": "later", "kind": "plugin", "name": "L", "version": "2.0.0", "api": 2 })).unwrap()), stamp: None });
        let at = |uri: &str, main: bool| reg.is_frame_page(uri, main);
        assert!(at("http://wakuplugin.localhost/rain-alert/__waku/main.html", true));
        assert!(!at("http://wakuplugin.localhost/rain-alert/__waku/main.html", false), "no main.js on the desktop");
        assert!(at("http://wakuplugin.localhost/rain-alert/view.html", false) && at("http://wakuplugin.localhost/rain-alert/view.html", true));
        assert!(at(&format!("http://wakuplugin.localhost/board/{}", url_path("ui/界面.html")), false));
        for bad in [
            "http://wakuplugin.localhost/rain-alert/main.js",
            "http://wakuplugin.localhost/rain-alert/view.html?x=1",
            "http://wakuplugin.localhost/rain-alert/other.html",
            "http://wakuplugin.localhost/plain/view.html",
            "http://wakuplugin.localhost/later/__waku/main.html",
            "http://wakuplugin.localhost/nobody/view.html",
            "http://tauri.localhost/pet/index.html",
            "https://example.com/rain-alert/view.html",
            "http://wakuplugin.localhost.example.com/rain-alert/view.html",
            "about:blank",
        ] {
            assert!(!at(bad, true), "{bad}");
        }
        // Who may ask for a card's view: her island's page, the desktop's.
        for (label, may) in [("island", true), ("desk-1", true), ("desk-12", true), ("desk-", false), ("desk-x", false), ("pet", false), ("bar-1", false), ("barpop", false), ("islandx", false)] {
            assert_eq!(may_view(label), may, "{label}");
        }
        // Ten messages a second for a plugin's views, then none until it is up.
        let mut sent = (0, 0);
        assert_eq!((0..12).filter(|_| may_send(&mut sent, 500)).count(), 10);
        assert!(!may_send(&mut sent, 999) && may_send(&mut sent, 1000));
    }

    #[test]
    fn manifests_that_will_not_do_say_why() {
        let bad: Vec<(&str, Box<dyn Fn(&mut Value)>)> = vec![
            ("id", Box::new(|m| m["id"] = json!("Rain"))),
            ("id", Box::new(|m| m["id"] = json!("r"))),
            ("id", Box::new(|m| m["id"] = json!("1rain"))),
            ("id", Box::new(|m| m["id"] = json!("rain_alert"))),
            ("id", Box::new(|m| m["id"] = json!("a".repeat(41)))),
            ("id", Box::new(|m| m["id"] = json!("weather"))),
            ("id", Box::new(|m| m["id"] = json!("mail-checker"))),
            ("id", Box::new(|m| m["id"] = json!("stock-watch"))),
            ("id", Box::new(|m| m["id"] = json!("waku"))),
            ("api", Box::new(|m| m["api"] = json!(0))),
            ("kind", Box::new(|m| m["kind"] = json!("theme"))),
            ("name", Box::new(|m| m["name"] = json!("一二三四五六七八九十一二三四五六七八九十一二三四五"))),
            ("name", Box::new(|m| m["name"] = json!({ "zh": "只有中文" }))),
            ("name", Box::new(|m| m["name"] = json!({ "en": "Rain", "fr": "Pluie" }))),
            ("description", Box::new(|m| m["description"] = json!("x".repeat(DESCRIPTION_MAX + 1)))),
            ("version", Box::new(|m| m["version"] = json!("1.2"))),
            ("version", Box::new(|m| m["version"] = json!("1.2.x"))),
            ("author", Box::new(|m| m["author"] = json!("x".repeat(41)))),
            ("homepage", Box::new(|m| m["homepage"] = json!("ftp://x"))),
            ("main", Box::new(|m| m["main"] = json!("../main.js"))),
            ("main", Box::new(|m| m["main"] = json!("main.exe"))),
            ("main", Box::new(|m| m["main"] = json!("C:/main.js"))),
            ("main", Box::new(|m| m.as_object_mut().unwrap().remove("main").map(|_| ()).unwrap_or(()))),
            ("view", Box::new(|m| m["view"] = json!("view.js"))),
            ("category", Box::new(|m| m["category"] = json!("games"))),
            ("widget.sizes", Box::new(|m| m["widget"]["sizes"] = json!(["xl"]))),
            ("widget.sizes", Box::new(|m| m["widget"]["sizes"] = json!([]))),
            ("widget.color", Box::new(|m| m["widget"]["color"] = json!("blue"))),
            ("widget.icon", Box::new(|m| m["widget"]["icon"] = json!("rocket"))),
            ("widget.big", Box::new(|m| m["widget"]["big"] = json!(true))),
            ("settings[0]", Box::new(|m| m["settings"][0]["key"] = json!("on"))),
            ("settings[0]", Box::new(|m| m["settings"][0]["key"] = json!("2City"))),
            ("settings[0]", Box::new(|m| m["settings"][0]["type"] = json!("date"))),
            ("settings[0]", Box::new(|m| m["settings"][0]["options"] = json!(["a"]))),
            ("settings[1]", Box::new(|m| m["settings"][1]["default"] = json!("7"))),
            ("settings[1]", Box::new(|m| m["settings"][1]["options"] = json!(["3", "3"]))),
            ("settings[1]", Box::new(|m| m["settings"][1]["optionLabels"] = json!({ "9": "nine" }))),
            ("settings[2]", Box::new(|m| m["settings"][2]["default"] = json!("yes"))),
            ("settings[2]", Box::new(|m| m["settings"][2]["key"] = json!("City"))),
            ("settings[0]", Box::new(|m| m["settings"][0]["label"] = json!(""))),
            ("permissions.net", Box::new(|m| m["permissions"]["net"] = json!(["localhost"]))),
            ("permissions.net", Box::new(|m| m["permissions"]["net"] = json!(["127.0.0.1"]))),
            ("permissions.net", Box::new(|m| m["permissions"]["net"] = json!(["api.open-meteo.com:443"]))),
            ("permissions.net", Box::new(|m| m["permissions"]["net"] = json!(["*"]))),
            ("permissions.net", Box::new(|m| m["permissions"]["net"] = json!(["*.com"]))),
            ("permissions.net", Box::new(|m| m["permissions"]["net"] = json!(["router.lan"]))),
            ("permissions.net", Box::new(|m| m["permissions"]["net"] = json!(["https://api.open-meteo.com"]))),
            ("permissions.camera", Box::new(|m| m["permissions"]["camera"] = json!(true))),
            ("permissions.notify", Box::new(|m| m["permissions"]["notify"] = json!("yes"))),
            ("extra", Box::new(|m| m["extra"] = json!(1))),
        ];
        for (what, f) in bad {
            let got = with(f);
            assert!(got.as_ref().is_err_and(|e| e.starts_with(what)), "{what}: {got:?}");
        }
        // A description longer than is shown: taken, and cut where shown.
        let long = with(|m| m["description"] = json!("一".repeat(DESCRIPTION_SHOWN + 1))).unwrap();
        let cut = shown(long.description.as_ref().unwrap().get("zh"), DESCRIPTION_SHOWN);
        assert_eq!((cut.chars().count(), cut.ends_with('…')), (DESCRIPTION_SHOWN, true));
        assert_eq!(shown("接下来几小时会不会下雨", DESCRIPTION_SHOWN), "接下来几小时会不会下雨");
        // A pack: no code, no settings.
        assert!(manifest_of(&json!({ "id": "cat", "kind": "pet", "name": "Cat", "version": "1.0.0", "api": 1 })).is_ok_and(|m| !m.runs()));
        assert!(manifest_of(&json!({ "id": "cat", "kind": "pet", "name": "Cat", "version": "1.0.0", "api": 1, "main": "main.js" })).is_err());
        // A newer API: read as far as it says of itself, and waits.
        let newer = manifest_of(&json!({ "id": "later", "kind": "plugin", "name": "Later", "version": "2.0.0", "api": 2, "whatever": { "new": true } })).unwrap();
        assert_eq!((newer.api, newer.runs()), (2, false));
    }

    #[test]
    fn the_hosts_a_plugin_may_name_and_reach() {
        for ok in ["api.open-meteo.com", "*.example.com", "xn--fiqs8s.xn--fiqs8s", "a-b.co.uk"] {
            assert!(is_host_rule(ok), "{ok}");
        }
        for bad in ["localhost", "app.localhost", "printer.local", "nas", "10.0.0.1", "127.0.0.1", "2130706433", "1.2.3.4.5", "x.123", "*", "*.com", "-a.com", "a..com", "api.example.com:443", "[::1]", "Example.com", "*.internal", "box.home.arpa", ""] {
            assert!(!is_host_rule(bad), "{bad}");
        }
        let rules = vec!["api.open-meteo.com".to_string(), "*.example.org".to_string()];
        assert!(host_allowed(&rules, "api.open-meteo.com"));
        assert!(host_allowed(&rules, "a.example.org") && host_allowed(&rules, "a.b.example.org"));
        assert!(!host_allowed(&rules, "example.org"), "a wildcard is the names under it");
        assert!(!host_allowed(&rules, "evilexample.org"));
        assert!(!host_allowed(&rules, "open-meteo.com") && !host_allowed(&rules, "geocoding-api.open-meteo.com"));
        // Addresses as asked: a name on the internet at 80 or 443.
        let t = target_of("HTTPS://API.open-meteo.com/v1/forecast?x=1#top").unwrap();
        assert_eq!(t, Target { url: "https://api.open-meteo.com/v1/forecast?x=1".into(), host: "api.open-meteo.com".into(), port: 443 });
        assert_eq!(target_of("http://example.com").unwrap().url, "http://example.com/");
        assert_eq!(target_of("https://example.com:443?q").unwrap().url, "https://example.com/?q");
        assert_eq!(target_of("http://example.com:443/").unwrap().port, 443);
        for bad in ["http://localhost/", "http://127.0.0.1:47213/health", "https://example.com:47213/", "http://[::1]/", "https://user:pw@example.com/", "ftp://example.com/", "file:///C:/x", "http://2130706433/", "http://0x7f.0.0.1/", "https://example.com/a b", "http://intranet/", "javascript:alert(1)"] {
            assert!(target_of(bad).is_err(), "{bad}");
        }
        for (ip, inside) in [
            ("127.0.0.1", true),
            ("10.1.2.3", true),
            ("192.168.1.1", true),
            ("172.20.0.1", true),
            ("169.254.1.1", true),
            ("100.64.0.1", true),
            ("0.0.0.0", true),
            ("::1", true),
            ("fd00::1", true),
            ("fe80::1", true),
            ("::ffff:127.0.0.1", true),
            // An IPv4 one reached through IPv6: compatible ::a.b.c.d, 6to4
            // 2002::/16, and the well-known NAT64 prefix 64:ff9b::/96.
            ("::0a00:0001", true),
            ("::ffff:10.0.0.1", true),
            ("2002:0a00:0001::", true),
            ("2002:ac10:0001::", true),
            ("64:ff9b::7f00:1", true),
            ("64:ff9b::0a00:1", true),
            ("1.1.1.1", false),
            ("198.18.0.5", false),
            ("2606:4700::1111", false),
            // NAT64 and 6to4 carrying a public IPv4: not inside.
            ("64:ff9b::808:808", false),
            ("2002:0808:0808::", false),
        ] {
            assert_eq!(is_inside(ip.parse().unwrap()), inside, "{ip}");
        }
        assert!(resolves_inside("localhost", 80));
        assert_eq!(redirected("https://a.com/x/y?z", "/b").as_deref(), Some("https://a.com/b"));
        assert_eq!(redirected("https://a.com/x/y?z", "c").as_deref(), Some("https://a.com/x/c"));
        assert_eq!(redirected("https://a.com/x/y", "//b.com/d").as_deref(), Some("https://b.com/d"));
        assert_eq!(redirected("https://a.com", "http://127.0.0.1/").as_deref(), Some("http://127.0.0.1/"));
    }

    // Asked of a host not named, of this computer, of a named one at another
    // port, or redirected to one of those: refused before anything is sent.
    #[test]
    fn a_fetch_goes_only_where_it_may() {
        let perms = Perms { net: vec!["example.com".into(), "localhost".into()], ..Perms::default() };
        let ask = |url: &str| fetch(&perms, Ask { url: url.into(), method: String::new(), headers: Vec::new(), body: None });
        assert!(ask("https://example.org/").unwrap_err().contains("not a host its manifest names"));
        assert!(ask("http://localhost/").unwrap_err().contains("not a name on the internet"));
        assert!(ask("http://127.0.0.1:47213/health").unwrap_err().contains("47213"));
        assert!(ask("https://example.com:47213/").unwrap_err().contains("only 80 and 443"));
        let post = |m: &str, body: Option<&str>| fetch(&perms, Ask { url: "https://example.com/".into(), method: m.into(), headers: Vec::new(), body: body.map(str::to_string) });
        assert!(post("CONNECT", None).unwrap_err().starts_with("CONNECT"));
        assert!(post("GET", Some("x")).unwrap_err().contains("no body"));
        assert!(post("POST", Some(&"x".repeat(FETCH_MAX + 1))).unwrap_err().contains("over 1 MB"));
        let bad_header = fetch(&perms, Ask { url: "https://example.com/".into(), method: "GET".into(), headers: vec![("x-a".into(), "1\r\nHost: evil".into())], body: None });
        assert!(bad_header.unwrap_err().contains("not a header"));
    }

    fn package(files: &[(&str, &[u8])]) -> Vec<u8> {
        zip_of(&files.iter().map(|(n, d)| (*n, *d, true)).collect::<Vec<_>>())
    }

    fn hello_manifest() -> Vec<u8> {
        serde_json::to_vec(&json!({ "id": "hello", "kind": "plugin", "name": "Hello", "version": "1.0.0", "api": 1, "main": "main.js", "category": "productivity", "widget": { "icon": "icon.svg" } })).unwrap()
    }

    #[test]
    fn packages_are_checked_whole() {
        let m = hello_manifest();
        let ok = package(&[("manifest.json", &m), ("main.js", b"waku.widget.set({ value: '1' })"), ("icon.svg", b"<svg/>"), ("img/a.png", b"\x89PNG....")]);
        let pkg = open_package(ok.clone()).unwrap();
        assert_eq!((pkg.manifest.id.as_str(), pkg.files.len(), pkg.sha256.len()), ("hello", 4, 64));
        // In one folder: the same.
        let nested = package(&[("hello-1.0.0/manifest.json", &m), ("hello-1.0.0/main.js", b"x"), ("hello-1.0.0/icon.svg", b"<svg/>")]);
        assert_eq!(open_package(nested).unwrap().files.iter().map(|f| f.1.as_str()).collect::<Vec<_>>(), ["manifest.json", "main.js", "icon.svg"]);
        // A Mac's litter left out.
        let mac = package(&[("hello/manifest.json", &m), ("hello/main.js", b"x"), ("hello/icon.svg", b"<svg/>"), ("__MACOSX/hello/._main.js", b"\x00\x05"), ("hello/.DS_Store", b"x")]);
        assert_eq!(open_package(mac).unwrap().files.len(), 3);
        let reason = |files: &[(&str, &[u8])]| open_package(package(files)).err().map(|r| (r.reason, r.file));
        // A program, by its name or by its first bytes whatever its name.
        assert_eq!(reason(&[("manifest.json", &m), ("main.js", b"x"), ("icon.svg", b"<svg/>"), ("helper.exe", b"MZ\x90\x00")]), Some(("type", Some("helper.exe".into()))));
        assert_eq!(reason(&[("manifest.json", &m), ("main.js", b"x"), ("icon.svg", b"<svg/>"), ("data.png", b"MZ\x90\x00 pretending")]), Some(("program", Some("data.png".into()))));
        assert_eq!(reason(&[("manifest.json", &m), ("main.js", b"\x7fELF\x02\x01"), ("icon.svg", b"<svg/>")]), Some(("program", Some("main.js".into()))));
        assert_eq!(reason(&[("manifest.json", &m), ("main.js", b"x"), ("icon.svg", b"<svg/>"), ("x.wasm", b"\xcf\xfa\xed\xfe")]), Some(("program", Some("x.wasm".into()))));
        assert_eq!(reason(&[("manifest.json", &m), ("main.js", b"x"), ("icon.svg", b"<svg/>"), ("LICENSE", b"MIT")]), Some(("type", Some("LICENSE".into()))));
        // Names out of the folder.
        for bad in ["../evil.js", "/abs.js", "C:/x.js", "a/../../b.js", "x\\..\\..\\y.js", "a:b.js"] {
            assert_eq!(reason(&[("manifest.json", &m), ("main.js", b"x"), ("icon.svg", b"<svg/>"), (bad, b"x")]).map(|r| r.0), Some("path"), "{bad}");
        }
        // Two manifests that differ only in case: refused, so one cannot show
        // one set of permissions while another runs (finding 1).
        let two = serde_json::to_vec(&json!({ "id": "hello", "kind": "plugin", "name": "Evil", "version": "1.0.0", "api": 1, "main": "main.js", "category": "info", "permissions": { "agents": true } })).unwrap();
        assert_eq!(reason(&[("manifest.json", &m), ("Manifest.json", &two), ("main.js", b"x"), ("icon.svg", b"<svg/>")]).map(|r| r.0), Some("dup"));
        assert_eq!(reason(&[("manifest.json", &m), ("main.js", b"x"), ("icon.svg", b"<svg/>"), ("IMG/a.png", b"x"), ("img/a.png", b"y")]).map(|r| r.0), Some("dup"));
        // Its manifest: at the root, good, naming files it has.
        assert_eq!(reason(&[("sub/manifest.json", &m), ("main.js", b"x")]).map(|r| r.0), Some("manifest"));
        assert_eq!(reason(&[("manifest.json", b"{ not json"), ("main.js", b"x")]).map(|r| r.0), Some("manifest"));
        assert_eq!(reason(&[("manifest.json", &m), ("icon.svg", b"<svg/>")]), Some(("manifest", Some("main.js".into()))));
        // Too many, too big, not a zip, damaged.
        let many: Vec<(String, Vec<u8>)> = (0..=FILES).map(|i| (format!("f{i}.txt"), b"x".to_vec())).collect();
        let mut files: Vec<(&str, &[u8])> = vec![("manifest.json", &m), ("main.js", b"x"), ("icon.svg", b"<svg/>")];
        files.extend(many.iter().map(|(n, d)| (n.as_str(), d.as_slice())));
        assert_eq!(reason(&files).map(|r| r.0), Some("files"));
        let big = vec![b'a'; (PLUGIN_ALL + 1) as usize];
        assert_eq!(reason(&[("manifest.json", &m), ("main.js", b"x"), ("icon.svg", b"<svg/>"), ("big.txt", &big)]).map(|r| r.0), Some("big"));
        let noise: Vec<u8> = (0..(PLUGIN_ZIP + 10)).map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8).collect();
        let stored = zip_of(&[("manifest.json", &m, false), ("main.js", b"x", false), ("icon.svg", b"<svg/>", false), ("noise.png", &noise, false)]);
        assert_eq!(open_package(stored).err().map(|r| r.reason), Some("big"));
        assert_eq!(open_package(b"PK not a zip".to_vec()).err().map(|r| r.reason), Some("zip"));
        // A byte of the manifest's own (after its 30-byte header and its name).
        let mut damaged = ok.clone();
        damaged[30 + "manifest.json".len() + 2] ^= 0xff;
        assert!(open_package(damaged).is_err());
    }

    // Unpacked beside, then swapped in; the old copy gone once it is.
    #[test]
    fn a_package_is_swapped_in_whole() {
        let root = scratch("unpack");
        let m = hello_manifest();
        let one = open_package(package(&[("manifest.json", &m), ("main.js", b"one"), ("icon.svg", b"<svg/>"), ("old.txt", b"old")])).unwrap();
        unpack(&one, &root).unwrap();
        assert_eq!(std::fs::read(root.join("hello").join("main.js")).unwrap(), b"one");
        let two = open_package(package(&[("manifest.json", &m), ("main.js", b"two"), ("icon.svg", b"<svg/>")])).unwrap();
        unpack(&two, &root).unwrap();
        assert_eq!(std::fs::read(root.join("hello").join("main.js")).unwrap(), b"two");
        assert!(!root.join("hello").join("old.txt").exists(), "the new copy alone");
        let left: Vec<String> = std::fs::read_dir(&root).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(left, ["hello"]);
        // The registry reads it; one with another id in its folder is broken.
        let mut reg = Registry::default();
        let dir = root.parent().unwrap().join(format!("wakuwaku-addons-reg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("plugins")).unwrap();
        unpack(&two, &dir.join("plugins")).unwrap();
        std::fs::create_dir_all(dir.join("plugins").join("other")).unwrap();
        std::fs::write(dir.join("plugins").join("other").join("manifest.json"), &m).unwrap();
        std::fs::create_dir_all(dir.join("plugins").join(".staging-x-1")).unwrap();
        let (changed, gone) = reg.refresh(&dir);
        assert!(changed && gone.is_empty());
        assert!(reg.manifest("hello").is_some());
        assert_eq!(reg.found["other"].manifest.as_ref().err().map(String::as_str), Some("its folder is other, its id hello"));
        assert!(!dir.join("plugins").join(".staging-x-1").exists(), "what an install cut short left, gone");
        assert!(!reg.refresh(&dir).0, "nothing changed");
        // Owners and settings by what is here.
        assert_eq!(reg.owner_of("hello").as_deref(), Some("hello"));
        assert_eq!(reg.owner_of("hello_two").as_deref(), Some("hello"));
        assert_eq!(reg.owner_of("other_x").as_deref(), Some("other"), "its folder is there, broken or not");
        assert_eq!(reg.owner_of("nope"), None);
        assert_eq!(reg.conf_ok("nope", &json!({ "on": true })), None);
        assert_eq!(reg.conf_ok("hello", &json!({ "on": true })), Some(true));
        assert_eq!(reg.conf_ok("hello", &json!({ "on": "yes" })), Some(false));
        assert_eq!(reg.conf_ok("hello", &json!({ "Town": "x" })), Some(false));
        assert_eq!(reg.conf_ok("other", &json!({ "on": false, "Kept": "as it was" })), Some(true));
        // A folder taken away by hand while its record is there: gone.
        std::fs::write(records_file(&dir), json!({ "hello": { "source": "file" } }).to_string()).unwrap();
        reg.refresh(&dir);
        std::fs::remove_dir_all(dir.join("plugins").join("hello")).unwrap();
        let (_, gone) = reg.refresh(&dir);
        assert_eq!(gone, ["hello"]);
        for d in [root, dir] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    // A swap cut short between moving the old copy aside and the new one in:
    // the only copy is in .old-<id>-<n>. At start it is renamed back, not
    // deleted; a .old- whose <id> is there again, and a .staging-, go.
    #[test]
    fn an_interrupted_swap_keeps_the_only_copy() {
        let root = scratch("interrupted");
        // <id> missing, its only copy set aside: renamed back.
        let only = root.join(".old-hello-123");
        std::fs::create_dir_all(&only).unwrap();
        std::fs::write(only.join("main.js"), b"kept").unwrap();
        // <id> there and a .old- beside it (a swap that finished all but the
        // tidy-up), plus a leftover staging: both thrown away.
        std::fs::create_dir_all(root.join("world")).unwrap();
        std::fs::create_dir_all(root.join(".old-world-9")).unwrap();
        std::fs::create_dir_all(root.join(".staging-world-9")).unwrap();
        // A pack id with a dash, and a name that is no .old- (left alone).
        std::fs::create_dir_all(root.join(".old-two-cats-77")).unwrap();
        std::fs::create_dir_all(root.join(".old-nope")).unwrap();
        clean_interrupted(&root);
        assert_eq!(std::fs::read(root.join("hello").join("main.js")).unwrap(), b"kept");
        assert!(root.join("world").exists() && !root.join(".old-world-9").exists() && !root.join(".staging-world-9").exists());
        assert!(root.join("two-cats").exists() && !root.join(".old-two-cats-77").exists());
        assert!(root.join(".old-nope").exists(), "no <n>: not a .old- of ours");
        let _ = std::fs::remove_dir_all(root);
    }

    // A stale value in a plugin's entry is dropped (not the whole entry), and
    // a folder gone by hand without a record is reported gone (finding 3).
    #[test]
    fn clean_conf_drops_stale_values_and_a_vanished_folder_is_gone() {
        let m = manifest_of(&rain()).unwrap();
        let mut reg = Registry::default();
        reg.found.insert("rain-alert".into(), Found { manifest: Ok(m), stamp: None });
        let conf = json!({ "on": true, "City": "上海", "Hours": "99", "Nope": "x" });
        let cleaned = reg.clean_conf("rain-alert", conf.as_object().unwrap()).unwrap();
        assert_eq!(Value::Object(cleaned), json!({ "on": true, "City": "上海" }), "a choice no longer an option, and an unknown key, dropped");
        assert!(reg.clean_conf("missing", conf.as_object().unwrap()).is_none(), "no folder of that id here");

        let dir = scratch("vanish");
        let hello = dir.join("plugins").join("hello");
        std::fs::create_dir_all(&hello).unwrap();
        std::fs::write(hello.join("manifest.json"), hello_manifest()).unwrap();
        std::fs::write(hello.join("main.js"), b"x").unwrap();
        std::fs::write(hello.join("icon.svg"), b"<svg/>").unwrap();
        let mut reg2 = Registry::default();
        let (_, gone) = reg2.refresh(&dir);
        assert!(gone.is_empty() && reg2.manifest("hello").is_some());
        std::fs::remove_dir_all(&hello).unwrap();
        let (_, gone) = reg2.refresh(&dir);
        assert_eq!(gone, ["hello"], "no installed.json record, yet gone");
        let _ = std::fs::remove_dir_all(dir);
    }

    // A plugin runs only when it was granted at least what its manifest asks;
    // asking for more makes it wait (needsApproval); a newer-API one never
    // needs approval while she cannot run it (finding 1, finding 7).
    #[test]
    fn a_plugin_waits_when_it_asks_for_more_than_granted() {
        let m = manifest_of(&rain()).unwrap();
        let mut reg = Registry::default();
        reg.found.insert("rain-alert".into(), Found { manifest: Ok(m.clone()), stamp: None });
        reg.records.insert("rain-alert".into(), json!({ "source": "market", "grantedPerms": { "net": ["api.open-meteo.com"], "agents": false, "hardware": false, "notify": true } }));
        assert!(reg.needs_approval("rain-alert", &m).is_empty(), "granted exactly what it asks");
        reg.records.insert("rain-alert".into(), json!({ "source": "file", "grantedPerms": { "net": [], "agents": false, "hardware": false, "notify": false } }));
        assert_eq!(reg.needs_approval("rain-alert", &m), ["net:api.open-meteo.com", "notify"], "waits for what was not granted");
        // No record (copied in by hand): its manifest is taken as granted.
        reg.records.remove("rain-alert");
        assert!(reg.needs_approval("rain-alert", &m).is_empty());
        // A newer-API plugin: its permissions are read for display (wants) but
        // enforced empty (perms), and it never waits while she cannot run it.
        let newer = manifest_of(&json!({ "id": "later", "kind": "plugin", "name": "L", "version": "2.0.0", "api": 2, "permissions": { "agents": true } })).unwrap();
        reg.found.insert("later".into(), Found { manifest: Ok(newer.clone()), stamp: None });
        assert_eq!((newer.wants.agents, newer.perms.agents, newer.runs()), (true, false, false));
        assert!(reg.needs_approval("later", &newer).is_empty());
    }

    // As it runs, what it may do is what it asks for as far as it was
    // granted: a manifest that came to ask for more gets nothing more of her,
    // stopped yet or not (call), and a grant of more gives no more than it asks.
    #[test]
    fn a_plugin_may_do_only_what_was_granted() {
        let asks = |p: Value| with(|m| m["permissions"] = p).unwrap();
        let mut reg = Registry::default();
        let mut put = |m: Manifest, granted: Value| {
            reg.found.insert("rain-alert".into(), Found { manifest: Ok(m), stamp: None });
            reg.records.insert("rain-alert".into(), json!({ "source": "file", "grantedPerms": granted }));
            reg.allowed("rain-alert").unwrap()
        };
        let more = asks(json!({ "net": ["api.open-meteo.com", "evil.example.com"], "agents": true, "hardware": true, "notify": true }));
        let p = put(more.clone(), json!({ "net": ["api.open-meteo.com"], "agents": false, "hardware": true, "notify": false }));
        assert_eq!(p, Perms { net: vec!["api.open-meteo.com".into()], agents: false, hardware: true, notify: false });
        let p = put(asks(json!({ "net": ["api.open-meteo.com"] })), json!({ "net": ["api.open-meteo.com", "other.com"], "agents": true, "hardware": true, "notify": true }));
        assert_eq!(p, Perms { net: vec!["api.open-meteo.com".into()], ..Perms::default() });
        // A wildcard granted: the names under it that it asks for.
        let p = put(asks(json!({ "net": ["a.example.org", "*.b.example.org", "example.org"] })), json!({ "net": ["*.example.org"] }));
        assert_eq!(p.net, ["a.example.org", "*.b.example.org"]);
        // A grant that will not read: nothing.
        assert_eq!(put(more, json!("all")), Perms::default());
        assert!(reg.allowed("no-such").is_none());
    }

    #[test]
    fn settings_hold_what_the_manifest_says() {
        let m = manifest_of(&rain()).unwrap();
        let mut reg = Registry::default();
        reg.found.insert("rain-alert".into(), Found { manifest: Ok(m.clone()), stamp: None });
        assert_eq!(reg.conf_ok("rain-alert", &json!({ "on": true, "City": "上海", "Hours": "3", "Alert": false })), Some(true));
        for bad in [json!({ "City": 5 }), json!({ "City": "a\nb" }), json!({ "City": "x".repeat(301) }), json!({ "Hours": "4" }), json!({ "Alert": "no" }), json!({ "on": 1 }), json!({ "Nope": "x" }), json!("on")] {
            assert_eq!(reg.conf_ok("rain-alert", &bad), Some(false), "{bad}");
        }
        let n = field_of(&json!({ "key": "Every", "type": "number", "default": 5, "label": "Every" })).unwrap();
        assert!(n.accepts(&json!("12.5")) && n.accepts(&json!("")) && n.accepts(&json!(3)) && !n.accepts(&json!("x")) && !n.accepts(&json!(true)));
        assert_eq!((n.value(&json!("")), n.value(&json!(" 7 ")), n.value(&json!("x"))), (json!(5.0), json!(7.0), json!(5.0)));
        let needed = field_of(&json!({ "key": "Key", "type": "number", "required": true, "label": "Key" })).unwrap();
        assert!(needed.is_missing(&json!("")) && !needed.is_missing(&json!("0")));
    }

    #[test]
    fn a_plugin_leaves_no_place_behind() {
        let settings = json!({
            "plugins": { "hello": { "on": true }, "weather": { "on": true } },
            "taskbarLayout": { "left": ["her"], "right": [
                { "id": "wa", "type": "plugin", "of": "hello", "size": "M" },
                { "id": "wb", "type": "stack", "items": [{ "type": "plugin", "of": "hello" }, { "type": "widget", "of": "today", "icon": "today" }] },
                { "id": "wc", "type": "carousel", "items": [{ "type": "widget", "of": "hello_x" }, { "type": "plugin", "of": "weather" }, { "type": "widget", "of": "tokens" }] },
                "clock"
            ] },
            "deskLayout": [
                { "id": "wd", "type": "widget", "of": "hello", "size": "s" },
                { "id": "we", "type": "stack", "size": "m", "d": "d1", "items": [{ "type": "plugin", "of": "hello" }, { "type": "widget", "of": "monitor" }] },
                { "id": "wf", "type": "board", "items": [{ "type": "plugin", "of": "hello", "size": "s", "at": [0, 0] }, { "type": "widget", "of": "today", "size": "s", "at": [2, 0] }] },
                { "id": "wg", "type": "widget", "of": "hellox", "size": "s" }
            ],
            "capsule": { "members": [{ "key": "plugin:hello", "mode": "turn" }, { "key": "widget:hello_x", "mode": "pin" }, { "key": "widget:today", "mode": "turn" }], "spin": 8 },
            "widgetsOff": ["hello_x", "tokens"],
            "widgetOrder": ["hello", "today"],
            "widgetSize": { "hello": "M", "today": "S" },
            "boardSets": [
                { "id": "s1", "name": "a", "items": [{ "type": "plugin", "of": "hello", "size": "s", "at": [0, 0] }], "saved": 1 },
                { "id": "s2", "name": "b", "items": [{ "type": "plugin", "of": "hello", "size": "s", "at": [0, 0] }, { "type": "widget", "of": "today", "size": "s", "at": [2, 0] }], "saved": 1 }
            ],
            "lang": "zh"
        });
        // The widgets this plugin actually made: its own, and the sub `hello_x`.
        // A person's `hellox` / `hellox` widget is not among them, so it stays.
        let widgets = HashSet::from(["hello".to_string(), "hello_x".to_string()]);
        let patch = without_plugin(settings.as_object().unwrap(), "hello", &widgets);
        assert_eq!(patch["plugins"], json!({ "weather": { "on": true } }));
        assert_eq!(
            patch["taskbarLayout"]["right"],
            json!([
                { "id": "wb", "type": "widget", "of": "today", "size": "M", "icon": "today" },
                { "id": "wc", "type": "carousel", "items": [{ "type": "plugin", "of": "weather" }, { "type": "widget", "of": "tokens" }] },
                "clock"
            ])
        );
        assert_eq!(patch["taskbarLayout"]["left"], json!(["her"]));
        assert_eq!(
            patch["deskLayout"],
            json!([
                { "id": "we", "type": "widget", "of": "monitor", "size": "m", "d": "d1" },
                { "id": "wf", "type": "board", "items": [{ "type": "widget", "of": "today", "size": "s", "at": [2, 0] }] },
                { "id": "wg", "type": "widget", "of": "hellox", "size": "s" }
            ])
        );
        assert_eq!(patch["capsule"], json!({ "members": [{ "key": "widget:today", "mode": "turn" }], "spin": 8 }));
        assert_eq!((patch["widgetsOff"].clone(), patch["widgetOrder"].clone(), patch["widgetSize"].clone()), (json!(["tokens"]), json!(["today"]), json!({ "today": "S" })));
        assert_eq!(patch["boardSets"].as_array().unwrap().iter().map(|s| s["id"].clone()).collect::<Vec<_>>(), [json!("s2")]);
        assert!(!patch.contains_key("lang"));
        // What it leaves passes the settings' own checks (her plugins only here).
        assert!(crate::settings::is_desk_layout(&patch["deskLayout"]) && crate::settings::is_board_sets(&patch["boardSets"]) && crate::settings::is_capsule(&patch["capsule"]));
        // A script widget that only begins like the plugin (hello_mine), never
        // made by it, is left in its places.
        let kept = json!({ "widgetsOff": ["hello_mine", "hello_x"], "deskLayout": [{ "id": "wz", "type": "widget", "of": "hello_mine", "size": "s" }] });
        let patch = without_plugin(kept.as_object().unwrap(), "hello", &widgets);
        assert_eq!(patch["widgetsOff"], json!(["hello_mine"]), "hello_x removed, hello_mine kept");
        assert!(!patch.contains_key("deskLayout"), "the person’s hello_mine tile untouched");
        // In nothing: nothing to change.
        assert!(without_plugin(json!({ "lang": "zh", "plugins": {} }).as_object().unwrap(), "hello", &widgets).is_empty());
    }

    #[test]
    fn storage_is_kept_within_its_megabyte() {
        let dir = scratch("store");
        assert_eq!(storage(&dir, "hello", "get", Some("n"), None), Ok(Value::Null));
        storage(&dir, "hello", "set", Some("n"), Some(json!(3))).unwrap();
        storage(&dir, "hello", "set", Some("list"), Some(json!([1, 2]))).unwrap();
        assert_eq!(storage(&dir, "hello", "get", Some("n"), None), Ok(json!(3)));
        assert_eq!(storage(&dir, "hello", "all", None, None), Ok(json!({ "n": 3, "list": [1, 2] })));
        assert_eq!(serde_json::from_str::<Value>(&std::fs::read_to_string(store_file(&dir, "hello")).unwrap()).unwrap()["n"], 3);
        storage(&dir, "hello", "remove", Some("n"), None).unwrap();
        assert_eq!(storage(&dir, "hello", "get", Some("n"), None), Ok(Value::Null));
        // Past a megabyte: refused, and what was there kept.
        assert!(storage(&dir, "hello", "set", Some("big"), Some(json!("x".repeat(STORE_MAX)))).unwrap_err().contains("1 MB"));
        assert_eq!(storage(&dir, "hello", "all", None, None), Ok(json!({ "list": [1, 2] })));
        storage(&dir, "hello", "set", Some("most"), Some(json!("x".repeat(STORE_MAX - 100)))).unwrap();
        assert!(storage(&dir, "hello", "set", Some("more"), Some(json!("x".repeat(200)))).is_err());
        assert!(storage(&dir, "hello", "set", Some(""), Some(json!(1))).is_err());
        assert!(storage(&dir, "hello", "drop", Some("x"), None).is_err());
        // Each its own.
        assert_eq!(storage(&dir, "other", "all", None, None), Ok(json!({})));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn its_files_are_its_own_folders() {
        let dir = scratch("serve");
        let folder = dir.join("hello");
        std::fs::create_dir_all(folder.join("img")).unwrap();
        std::fs::write(folder.join("main.js"), "x").unwrap();
        std::fs::write(folder.join("img").join("一 a.png"), "p").unwrap();
        std::fs::write(folder.join("notes.bat"), "x").unwrap();
        std::fs::write(dir.join("secret.js"), "x").unwrap();
        assert_eq!(own_file(&folder, "main.js").map(|f| f.1), Some("text/javascript; charset=utf-8"));
        assert_eq!(own_file(&folder, "img/%E4%B8%80%20a.png").map(|f| f.0), Some(b"p".to_vec()));
        for bad in ["../secret.js", "..%2Fsecret.js", "img/../../secret.js", "notes.bat", "missing.js", "%ZZ.js", "C:/Windows/win.ini", "img"] {
            assert!(own_file(&folder, bad).is_none(), "{bad}");
        }
        assert!(policy("hello").contains("connect-src 'none'") && policy("hello").contains("script-src http://wakuplugin.localhost/hello/ "));
        assert!(host_page("hello").contains("<script src=\"/hello/__waku/sdk.js\"></script>"));
        assert_eq!(widget_id("hello", None).as_deref(), Ok("hello"));
        assert_eq!(widget_id("hello", Some("two2")).as_deref(), Ok("hello_two2"));
        assert!(widget_id("hello", Some("Two")).is_err() && widget_id("hello", Some(&"a".repeat(17))).is_err() && widget_id(&"a".repeat(30), Some("abcdefghijk")).is_err());
        assert!(is_widget_of("hello", "hello_x") && !is_widget_of("hello", "hellox") && !is_widget_of("hello", "hello_"));
        let _ = std::fs::remove_dir_all(dir);
    }

    // What her plugin scheme answers (serve): a running plugin's own files
    // and her two pages for it, each with the headers that keep it in; for
    // any other plugin, path or method, nothing of it.
    #[test]
    fn the_scheme_gives_a_running_plugin_its_own_files_alone() {
        use tauri::http::{Method, Response};
        let dir = scratch("respond");
        let plugins = plugins_dir(&dir);
        for id in ["hello", "sleepy"] {
            std::fs::create_dir_all(plugins.join(id)).unwrap();
            std::fs::write(plugins.join(id).join("main.js"), "waku.widget.set({})").unwrap();
        }
        let view = "<html><head><title>v</title></head><body>v</body></html>";
        std::fs::write(plugins.join("hello").join("view.html"), view).unwrap();
        std::fs::write(plugins.join("other.js"), "x").unwrap();
        // hello runs, with a view; sleepy is here but does not run.
        let view_of = |id: &str| (id == "hello").then(|| Some("view.html".to_string()));
        let get = |method: &Method, path: &str| respond(Some(&dir), method, path, view_of);
        let header = |res: &Response<Vec<u8>>, k: &str| res.headers().get(k).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
        let res = get(&Method::GET, "/hello/main.js");
        assert_eq!((res.status().as_u16(), res.body().as_slice()), (200, b"waku.widget.set({})".as_slice()));
        assert_eq!(header(&res, "content-type"), "text/javascript; charset=utf-8");
        assert_eq!(header(&res, "content-security-policy"), policy("hello"));
        for (k, v) in [("x-content-type-options", "nosniff"), ("cache-control", "no-store"), ("referrer-policy", "no-referrer"), ("x-dns-prefetch-control", "off")] {
            assert_eq!(header(&res, k), v, "{k}");
        }
        // HEAD: its length, no body.
        let res = get(&Method::HEAD, "/hello/main.js");
        assert_eq!((res.status().as_u16(), header(&res, "content-length"), res.body().len()), (200, "waku.widget.set({})".len().to_string(), 0));
        // Her pages for it: its frame's first and the SDK, nothing else under __waku.
        let res = get(&Method::GET, "/hello/__waku/main.html");
        assert_eq!((header(&res, "content-type"), res.body().clone()), ("text/html; charset=utf-8".to_string(), host_page("hello").into_bytes()));
        assert_eq!(header(&res, "content-security-policy"), policy("hello"));
        assert_eq!(get(&Method::GET, "/hello/__waku/sdk.js").body().as_slice(), SDK.as_bytes());
        // Its view with her SDK first.
        assert_eq!(get(&Method::GET, "/hello/view.html").body().clone(), view_page("hello", view).into_bytes());
        for path in ["/hello/__waku/main.js", "/hello/../other.js", "/hello/%2e%2e/other.js", "/hello/missing.js", "/hello/", "/hello", "/sleepy/main.js", "/nobody/main.js", "/Hello/main.js", "/../hello/main.js", "/"] {
            assert_eq!(get(&Method::GET, path).status().as_u16(), 404, "{path}");
        }
        for method in [Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS] {
            assert_eq!(get(&method, "/hello/main.js").status().as_u16(), 405, "{method}");
        }
        assert_eq!(respond(None, &Method::GET, "/hello/main.js", view_of).status().as_u16(), 503);
        let _ = std::fs::remove_dir_all(dir);
    }
}
