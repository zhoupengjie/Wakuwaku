// Third-party plugins (addons/mod.rs). What a plugin is: its manifest, read and checked.
use super::*;

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
    pub(super) fn accepts(&self, v: &Value) -> bool {
        match &self.kind {
            FieldKind::Text => v.as_str().is_some_and(|t| t.chars().count() <= 300 && !t.chars().any(char::is_control)),
            FieldKind::Number => v.as_f64().is_some_and(f64::is_finite) || v.as_str().is_some_and(|t| t.trim().is_empty() || number_of(t).is_some()),
            FieldKind::Flag => v.is_boolean(),
            FieldKind::Choice { options, .. } => v.as_str().is_some_and(|o| options.iter().any(|x| x == o)),
        }
    }

    // Its value for the plugin: as set, else its default (else nothing:
    // empty text, no number, false, the first option).
    pub(super) fn value(&self, set: &Value) -> Value {
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
    pub(super) fn is_missing(&self, set: &Value) -> bool {
        self.required
            && match self.value(set) {
                Value::String(s) => s.is_empty(),
                Value::Null => true,
                _ => false,
            }
    }
}

pub(super) fn number_of(t: &str) -> Option<f64> {
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

    pub(super) fn field(&self, key: &str) -> Option<&Field> {
        self.settings.iter().find(|f| f.key == key)
    }

    // The settings' values for it now (Field::value), by their keys.
    pub(super) fn values(&self, conf: &Value) -> Value {
        Value::Object(self.settings.iter().map(|f| (f.key.clone(), f.value(&conf[&f.key]))).collect())
    }

    // The first setting it needs and has not been given.
    pub(super) fn missing(&self, conf: &Value) -> Option<&str> {
        self.settings.iter().find(|f| f.is_missing(&conf[&f.key])).map(|f| f.key.as_str())
    }

    // The files it names: its main, its view, its icon.
    pub(super) fn files_named(&self) -> Vec<&str> {
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

pub(super) fn is_color(c: &str) -> bool {
    let hex = c.strip_prefix('#').unwrap_or("");
    matches!(hex.len(), 3 | 6) && hex.chars().all(|c| c.is_ascii_hexdigit())
}

pub(crate) fn ext_of(name: &str) -> String {
    let last = name.rsplit(['/', '\\']).next().unwrap_or(name);
    last.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default()
}

// A file it names (its main, view, icon): a path in its folder, of these types.
pub(super) fn file_of(v: Option<&Value>, exts: &[&str], what: &str) -> Result<Option<String>, String> {
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

pub(super) fn field_of(v: &Value) -> Result<Field, String> {
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

