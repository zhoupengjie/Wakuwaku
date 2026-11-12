// The plugin market (docs/design/specs/plugins.md §1, §2, §5; its files and
// index: tools/market/README.md): other people's plugins, reviewed, got from
// where the market publishes them, the release `market` of the public
// repository wakuwaku-plugins on GitHub, and Gitee's copy of it for China,
// in the order her updates go (updates.rs gitee_first: updateFrom, or
// in_china), the other should the first fail. Through the Network proxy, each
// way in turn, its redirects followed by hand (proxy.rs get, as her updates'),
// only to there.
//
// Its index (index.json) is read when the market's page opens and it is an
// hour old or more, or when the page asks again; kept in memory and in
// <data>/plugins/.market.json with when it was read, so the Plugins page
// knows of updates across a restart. Not there (404): the market has not
// opened yet (its repository is not made), said as that, not as an error.
//
// A plugin got from it: its zip downloaded beside the plugins
// (.download-<id>.zip), its size and SHA-256 those the index gives, its
// manifest's id, version and kind the item's, then put in as one from a
// file is (addons.rs put_in), recorded as the market's (reviewed), the
// permissions it asks for granted. Each step told to the island's page as
// it goes (market:progress { id, got, total, stage }). A pack (a pet,
// wallpapers) the same way, put where its kind goes (packs.rs).
//
// Updates: a sleeping thread looks a few minutes after she starts, then
// once a day; with nothing of the market's installed, it does nothing.
// A newer version asking for nothing more than was granted goes in by
// itself (pluginAutoUpdate, on unless turned off), and its frame loads it
// anew; one asking for more waits, as all do with it off: those waiting are
// the snapshot's marketUpdates, for the Plugins page's 「N 个插件有更新 ·
// 全部更新」, its confirm listing what each newly asks for.
//
// For tests: WAKUWAKU_MARKET_URL, one base in place of both (the fake
// market, tools/market/fake-market.js), and redirects to http:// allowed
// then; WAKUWAKU_MARKET_LOOK=<first>[,<every>], the looks in seconds.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};

use crate::addons::{self, Kind, Perms, Words};
use crate::proxy::Miss;
use crate::{now_ms, shared, Shared};
use crate::lock::Lock;

const GITHUB: &str = "https://github.com/zhoupengjie/wakuwaku-plugins/releases/download/market/";
const GITEE: &str = "https://gitee.com/zhoupengjie/wakuwaku-plugins/releases/download/market/";
const TEST: &str = "WAKUWAKU_MARKET_URL";
const LOOK_TEST: &str = "WAKUWAKU_MARKET_LOOK";
const HOUR_MS: u64 = 3_600_000;
const DAY_MS: u64 = 86_400_000;
// The first look this long after she starts; then, every hour, whether one is due.
const FIRST_LOOK: Duration = Duration::from_secs(180);
const LOOK_EVERY: Duration = Duration::from_secs(3600);
// index.json at most; a zip's download at most so long.
const INDEX_MAX: u64 = 4 << 20;
const DOWNLOAD_SECS: u64 = 300;
const CACHE: &str = ".market.json";
// The page told of a download's progress at most so often.
const TELL_EVERY: Duration = Duration::from_millis(150);

// Where the market's files are: its name for the words, its base URL
// (index.json and the zips under it).
#[derive(Clone, Debug, PartialEq)]
pub struct Base {
    pub name: String,
    pub url: String,
}

fn test_base() -> Option<String> {
    let url = std::env::var(TEST).ok()?.trim().to_string();
    (!url.is_empty()).then(|| if url.ends_with('/') { url } else { format!("{url}/") })
}

// GitHub's and Gitee's, the one preferred first; a test's alone.
pub fn bases_for(gitee_first: bool, test: Option<String>) -> Vec<Base> {
    if let Some(url) = test {
        return vec![Base { name: crate::proxy::host_of(&url).unwrap_or_else(|| "test".into()), url }];
    }
    let github = Base { name: "GitHub".into(), url: GITHUB.into() };
    let gitee = Base { name: "Gitee".into(), url: GITEE.into() };
    if gitee_first {
        vec![gitee, github]
    } else {
        vec![github, gitee]
    }
}

fn bases(sh: &Shared) -> Vec<Base> {
    bases_for(crate::updates::gitee_first(sh), test_base())
}

// A download only from the market's own places (or a test's: `test`).
fn allowed(url: &str, test: Option<&str>) -> bool {
    test.is_some_and(|t| url.starts_with(t)) || crate::proxy::is_under(url, &[GITHUB, GITEE])
}

// --- The index ---------------------------------------------------------------------

// One of its items (tools/market/README.md), checked: what does not read
// right is left out of the list, never half taken.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: String,
    pub kind: Kind,
    pub name: Words,
    pub description: Option<Words>,
    pub author: String,
    pub homepage: Option<String>,
    pub version: String,
    pub api: u64,
    pub category: &'static str,
    pub sizes: Vec<&'static str>,
    pub color: Option<String>,
    pub view: bool,
    pub perms: Perms,
    pub file: String,
    pub size: u64,
    pub sha256: String,
    // Its .svg as a data: URI, her icon's name, what its widget would say
    // (cleaned as a widget is), a pack's picture as a data: URI.
    pub icon: Option<String>,
    pub glyph: Option<String>,
    pub sample: Value,
    pub preview: Option<String>,
    pub updated: String,
}

fn is_color(c: &str) -> bool {
    let hex = c.strip_prefix('#').unwrap_or("");
    matches!(hex.len(), 3 | 6) && hex.chars().all(|c| c.is_ascii_hexdigit())
}

fn is_sha256(h: &str) -> bool {
    h.len() == 64 && h.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

// A zip's name beside index.json: a-z 0-9 . _ -, ending .zip, no way out.
fn is_file(f: &str) -> bool {
    f.len() <= 120 && f.ends_with(".zip") && !f.contains("..") && f.as_bytes().first().is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit()) && f.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-'))
}

// A data: URI of these types, so long at most (base64 of 8 KB for an icon,
// of 64 KB for a preview).
fn data_uri(v: &Value, types: &[&str], max: usize) -> Option<String> {
    let s = v.as_str()?;
    let ok = s.len() <= max && types.iter().any(|t| s.strip_prefix("data:").and_then(|r| r.strip_prefix(t)).is_some_and(|r| r.starts_with(";base64,")));
    ok.then(|| s.to_string())
}

fn cut(s: &str, max: usize) -> String {
    s.trim().chars().take(max).collect()
}

// widget.sample as her pages may draw it: as a widget from a script is
// taken (widgets.rs made, detail_of), what is not of a widget dropped,
// words cut; null for none, or one over 4 KB.
pub fn sample_of(v: &Value) -> Value {
    let Some(o) = v.as_object().filter(|_| v.to_string().len() <= addons::SAMPLE_MAX) else { return Value::Null };
    let words = |k: &str, max: usize| o.get(k).and_then(Value::as_str).map(|t| cut(t, max)).filter(|t| !t.is_empty());
    let mut out = Map::new();
    if let Some(label) = words("label", 40) {
        out.insert("label".into(), json!(label));
    }
    out.insert("value".into(), json!(words("value", 24).unwrap_or_default()));
    if let Some(icon) = o.get("icon").and_then(Value::as_str).filter(|i| crate::widgets::ICONS.contains(i)) {
        out.insert("icon".into(), json!(icon));
    }
    if let Some(color) = o.get("color").and_then(Value::as_str).filter(|c| is_color(c)) {
        out.insert("color".into(), json!(color));
    }
    if o.get("private") == Some(&Value::Bool(true)) {
        out.insert("private".into(), json!(true));
    }
    let detail = crate::widgets::detail_of(o.get("detail"));
    if !detail.is_null() {
        out.insert("detail".into(), detail);
    }
    Value::Object(out)
}

pub fn item_of(v: &Value) -> Result<Item, String> {
    let o = v.as_object().ok_or("not an object")?;
    let s = |k: &str| o.get(k).and_then(Value::as_str);
    let id = s("id").filter(|id| addons::is_id(id) && !addons::is_reserved(id)).ok_or("id")?.to_string();
    let kind = s("kind").and_then(Kind::of).ok_or("kind")?;
    let name = Words::of(o.get("name"), 24).map_err(|e| format!("name: {e}"))?;
    let description = match o.get("description") {
        None | Some(Value::Null) => None,
        d => Some(Words::of(d, 500).map_err(|e| format!("description: {e}"))?),
    };
    let author = s("author").map(str::trim).filter(|a| a.chars().count() <= 40 && !a.chars().any(char::is_control)).unwrap_or("").to_string();
    let homepage = s("homepage").filter(|h| (h.starts_with("https://") || h.starts_with("http://")) && h.len() <= 200 && !h.chars().any(|c| c.is_whitespace() || c.is_control())).map(str::to_string);
    let version = s("version").filter(|v| addons::is_version(v)).ok_or("version")?.to_string();
    let plugin = kind == Kind::Plugin;
    let api = o.get("api").and_then(Value::as_u64).filter(|n| *n >= 1).unwrap_or(1);
    let category = match s("category") {
        Some("system") => "system",
        Some("productivity") => "productivity",
        _ => "info",
    };
    let mut sizes: Vec<&'static str> = ["s", "m", "l"].into_iter().filter(|z| o.get("sizes").and_then(Value::as_array).is_some_and(|l| l.iter().any(|x| x.as_str() == Some(z)))).collect();
    if sizes.is_empty() {
        sizes = vec!["s", "m"];
    }
    let perms = match o.get("permissions") {
        None | Some(Value::Null) => Perms::default(),
        Some(p) if plugin => addons::perms_of(p)?,
        Some(_) => Perms::default(),
    };
    let file = s("file").filter(|f| is_file(f)).ok_or("file")?.to_string();
    let (zipped, _) = kind.limits();
    let size = o.get("size").and_then(Value::as_u64).filter(|n| (1..=zipped).contains(n)).ok_or("size")?;
    let sha256 = s("sha256").filter(|h| is_sha256(h)).ok_or("sha256")?.to_string();
    Ok(Item {
        id,
        kind,
        name,
        description,
        author,
        homepage,
        version,
        api,
        category,
        sizes,
        color: s("color").filter(|c| plugin && is_color(c)).map(str::to_string),
        view: plugin && o.get("view") == Some(&Value::Bool(true)),
        perms,
        file,
        size,
        sha256,
        icon: o.get("icon").filter(|_| plugin).and_then(|i| data_uri(i, &["image/svg+xml"], 11 * 1024)),
        glyph: s("glyph").filter(|g| plugin && crate::widgets::ICONS.contains(g)).map(str::to_string),
        sample: if plugin { o.get("sample").map_or(Value::Null, sample_of) } else { Value::Null },
        preview: o.get("preview").filter(|_| !plugin).and_then(|p| data_uri(p, &["image/webp", "image/png", "image/jpeg"], 88 * 1024)),
        updated: s("updated").map(|u| cut(u, 10)).unwrap_or_default(),
    })
}

// An index.json read: its items, and those left out with why (by their id
// as written). Err when it is not a market's index at all.
pub fn index_of(v: &Value) -> Result<(Vec<Item>, Vec<String>), String> {
    if v["format"] != 1 {
        return Err("not a market's index.json (format 1)".into());
    }
    let list = v["items"].as_array().ok_or("no items")?;
    let mut items: Vec<Item> = Vec::new();
    let mut left = Vec::new();
    for raw in list {
        match item_of(raw) {
            Ok(item) if items.iter().any(|i| i.id == item.id) => left.push(format!("{}: twice", item.id)),
            Ok(item) => items.push(item),
            Err(why) => left.push(format!("{}: {why}", raw["id"].as_str().unwrap_or("?"))),
        }
    }
    Ok((items, left))
}

fn words_json(w: &Words) -> Value {
    json!({ "en": w.en, "zh": w.zh })
}

// For the market's page: each item as it draws it, her widgets' words in both languages.
fn item_json(i: &Item) -> Value {
    json!({
        "id": i.id,
        "kind": i.kind.name(),
        "name": words_json(&i.name),
        "description": i.description.as_ref().map(words_json),
        "author": i.author,
        "homepage": i.homepage,
        "version": i.version,
        "api": i.api,
        "supported": i.api <= addons::API,
        "category": i.category,
        "sizes": i.sizes,
        "color": i.color,
        "view": i.view,
        "perms": i.perms.json(),
        "sensitive": i.perms.sensitive(),
        "size": i.size,
        "icon": i.icon,
        "glyph": i.glyph,
        "sample": i.sample,
        "preview": i.preview,
        "updated": i.updated,
    })
}

// --- What she knows of it ------------------------------------------------------------

struct Index {
    items: Vec<Item>,
    // When it was read (ms), and from where (a base's name).
    at: u64,
    from: String,
}

#[derive(Default)]
struct State {
    index: Option<Index>,
    // What went wrong the last time it was read: { code: closed | failed | bad, detail }.
    error: Option<Value>,
    // When the next look for updates is due (ms).
    next_look: u64,
}

static STATE: LazyLock<Lock<State>> = LazyLock::new(|| Lock::new("market::STATE", State::default()));
// One read of the index at a time: one asked meanwhile takes what it got.
// Held over the read (the network, then STATE and the settings): an outer
// lock (lock.rs), on the market's own threads.
static READING: Lock<()> = Lock::ranked("market::READING", crate::lock::OUTER, ());
// Each download under way, and whether it was called off.
static JOBS: LazyLock<Lock<HashMap<String, Arc<AtomicBool>>>> = LazyLock::new(|| Lock::new("market::JOBS", HashMap::new()));

// A download or update of this id called off, if one is under way (its loop
// checks the flag): so an uninstall of it does not race a market job
// (addons::uninstall, finding 8).
pub fn cancel(id: &str) {
    if let Some(c) = JOBS.lock().unwrap_or_else(|e| e.into_inner()).get(id) {
        c.store(true, Ordering::Relaxed);
    }
}

fn state() -> crate::lock::Guard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

fn cache_file(dir: &Path) -> PathBuf {
    dir.join("plugins").join(CACHE)
}

fn error(code: &str, detail: impl Into<String>) -> Value {
    json!({ "code": code, "detail": detail.into() })
}

// The index from the first place that has it: Ok(the index as read, where
// from); Err { code: closed } when one said it is not there (404: the
// market not opened yet), else { code: failed, detail: "GitHub: <way>,
// <hosts>: <error>; Gitee: …" }, as her updates say it.
pub fn fetch_index(bases: &[Base], plain: bool) -> Result<(Value, String), Value> {
    let mut failed = Vec::new();
    let mut absent = Vec::new();
    for b in bases {
        match crate::proxy::get(&format!("{}index.json", b.url), 20, Some("application/json"), plain) {
            Ok(mut a) => match a.res.body_mut().with_config().limit(INDEX_MAX).read_json::<Value>() {
                Ok(v) if v["format"] == 1 && v["items"].is_array() => return Ok((v, b.name.clone())),
                Ok(_) => failed.push(format!("{}: {}, {}: not a market's index.json", b.name, a.way, a.path)),
                Err(e) => failed.push(format!("{}: {}, {}: {e}", b.name, a.way, a.path)),
            },
            Err(Miss::NotThere) => absent.push(b.name.clone()),
            Err(Miss::Failed(e)) => failed.push(format!("{}: {e}", b.name)),
        }
    }
    if !absent.is_empty() {
        return Err(error("closed", absent.join(", ")));
    }
    Err(error("failed", failed.join("; ")))
}

// The index read now, kept, and written beside the plugins.
fn read_now(sh: &Shared) {
    let got = fetch_index(&bases(sh), test_base().is_some());
    let now = now_ms();
    match got.and_then(|(raw, from)| index_of(&raw).map(|(items, left)| (raw, from, items, left)).map_err(|e| error("bad", e))) {
        Ok((raw, from, items, left)) => {
            sh.log(&format!("market: index from {from}, {} item(s){}", items.len(), if left.is_empty() { String::new() } else { format!("; left out: {}", left.join(", ")) }));
            let cache = json!({ "at": now, "from": from, "index": raw });
            let _ = std::fs::create_dir_all(sh.dir.join("plugins"));
            if let Err(e) = addons::write_whole(&cache_file(&sh.dir), cache.to_string().as_bytes()) {
                sh.log(&format!("market: could not keep its index: {e}"));
            }
            let mut s = state();
            s.index = Some(Index { items, at: now, from });
            s.error = None;
        }
        Err(e) => {
            sh.log(&format!("market: index not read ({}: {})", e["code"].as_str().unwrap_or(""), e["detail"].as_str().unwrap_or("")));
            let mut s = state();
            // Not there at all: what was read before is no market's any more.
            if e["code"] == "closed" {
                s.index = None;
                let _ = std::fs::remove_file(cache_file(&sh.dir));
            }
            s.error = Some(e);
        }
    }
}

// The index as the page draws it, read again first when asked (`force`)
// or an hour old: { items, at, from, error }; items null while none was
// ever read.
pub fn index(sh: &Shared, force: bool) -> Value {
    read_if_older(sh, if force { 0 } else { HOUR_MS });
    let s = state();
    json!({
        "items": s.index.as_ref().map(|i| i.items.iter().map(item_json).collect::<Vec<_>>()),
        "at": s.index.as_ref().map(|i| i.at),
        "from": s.index.as_ref().map(|i| i.from.clone()),
        "error": s.error,
    })
}

// The index read again when it is so old (ms), or was never read.
fn read_if_older(sh: &Shared, age: u64) {
    let _one = READING.lock().unwrap_or_else(|e| e.into_inner());
    let fresh = state().index.as_ref().is_some_and(|i| age > 0 && now_ms().saturating_sub(i.at) < age);
    if !fresh {
        read_now(sh);
    }
}

fn item(id: &str) -> Option<Item> {
    state().index.as_ref().and_then(|i| i.items.iter().find(|x| x.id == id).cloned())
}

// --- Updates -------------------------------------------------------------------------

// One of the market's installed here with a newer version there: its id,
// name, the two versions, and what the new one asks for that was not
// granted ("net:<host>", "agents", "hardware", "notify").
#[derive(Clone, Debug, PartialEq)]
pub struct Update {
    pub id: String,
    pub name: String,
    pub from: String,
    pub to: String,
    pub new_perms: Vec<String>,
}

// What `wanted` asks for beyond `granted`: a host none of those granted
// covers (the same name, or under a *. granted), and each flag not granted.
pub fn new_perms(granted: &Perms, wanted: &Perms) -> Vec<String> {
    let mut out: Vec<String> = wanted.net.iter().filter(|h| !addons::host_allowed(&granted.net, h)).map(|h| format!("net:{h}")).collect();
    for (on, had, name) in [(wanted.agents, granted.agents, "agents"), (wanted.hardware, granted.hardware, "hardware"), (wanted.notify, granted.notify, "notify")] {
        if on && !had {
            out.push(name.into());
        }
    }
    out
}

pub fn updates_of(installed: &[addons::FromMarket], items: &[Item]) -> Vec<Update> {
    installed
        .iter()
        .filter_map(|p| {
            let i = items.iter().find(|i| i.id == p.id && i.kind == p.kind)?;
            crate::updates::newer(&i.version, &p.version).then(|| Update { id: p.id.clone(), name: p.name.clone(), from: p.version.clone(), to: i.version.clone(), new_perms: new_perms(&p.granted, &i.perms) })
        })
        .collect()
}

fn auto_update(sh: &Shared) -> bool {
    sh.config().plugin_auto_update
}

// What of the market's is here: its plugins (addons.rs) and its packs (packs.rs).
fn installed(sh: &Shared) -> Vec<addons::FromMarket> {
    let mut all = addons::from_market(sh.lang());
    all.extend(crate::packs::from_market(&sh.dir, sh.lang()));
    all
}

fn updates_now(sh: &Shared) -> Vec<Update> {
    let installed = installed(sh);
    if installed.is_empty() {
        return Vec::new();
    }
    let s = state();
    s.index.as_ref().map(|i| updates_of(&installed, &i.items)).unwrap_or_default()
}

// Those waiting for the person (the snapshot's marketUpdates): with
// auto-update on, those asking for more; off, all.
fn held(updates: Vec<Update>, auto: bool) -> Vec<Update> {
    updates.into_iter().filter(|u| !auto || !u.new_perms.is_empty()).collect()
}

pub fn updates_view(sh: &Shared) -> Value {
    Value::Array(held(updates_now(sh), auto_update(sh)).iter().map(|u| json!({ "id": u.id, "name": u.name, "from": u.from, "to": u.to, "newPerms": u.new_perms })).collect())
}

// The day's look: nothing at all with none of the market's installed; else
// the index (read again when an hour old, or older than the looks are
// apart), each newer one asking for no more going in by itself (auto-update
// on), the rest left waiting.
fn look(sh: &Arc<Shared>, every: u64) {
    state().next_look = now_ms() + every;
    if installed(sh).is_empty() {
        return;
    }
    read_if_older(sh, HOUR_MS.min(every));
    let all = updates_now(sh);
    if all.is_empty() {
        sh.log("market: looked; the market's plugins here are the newest");
        return;
    }
    let auto = auto_update(sh);
    if auto {
        for u in all.iter().filter(|u| u.new_perms.is_empty()) {
            match install(sh, &u.id) {
                Ok(_) => sh.log(&format!("market: {} {} → {} updated by itself", u.id, u.from, u.to)),
                Err(e) => sh.log(&format!("market: {} {} → {} not updated: {}", u.id, u.from, u.to, e["detail"].as_str().unwrap_or(""))),
            }
        }
    }
    let waiting = held(updates_now(sh), auto);
    if !waiting.is_empty() {
        let why = if auto { "asking for more" } else { "auto-update off" };
        sh.log(&format!("market: {} waiting ({why}): {}", waiting.len(), waiting.iter().map(|u| format!("{} {} → {}", u.id, u.from, u.to)).collect::<Vec<_>>().join(", ")));
    }
    sh.push_settings();
}

// The looks' times: the first after she starts, then how often one is due;
// as a test says (WAKUWAKU_MARKET_LOOK=<first>[,<every>], seconds).
fn look_times() -> (Duration, Duration, u64) {
    let test = std::env::var(LOOK_TEST).unwrap_or_default();
    let mut parts = test.split(',').map(|p| p.trim().parse::<u64>().ok());
    match (parts.next().flatten(), parts.next().flatten()) {
        (Some(first), every) => {
            let every = every.unwrap_or(86_400).max(1);
            (Duration::from_secs(first), Duration::from_secs(every.min(3600)), every * 1000)
        }
        _ => (FIRST_LOOK, LOOK_EVERY, DAY_MS),
    }
}

// As she starts: what was read before, a download cut off cleared away, and
// the thread of the looks (asleep but for one a day).
pub fn start(sh: &Arc<Shared>) {
    let root = sh.dir.join("plugins");
    for entry in std::fs::read_dir(&root).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(".download-") && name.ends_with(".zip") && std::fs::remove_file(entry.path()).is_ok() {
            sh.log(&format!("market: {name}, left by a download cut off, removed"));
        }
    }
    if let Some(kept) = std::fs::read_to_string(cache_file(&sh.dir)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
        if let Ok((items, _)) = index_of(&kept["index"]) {
            let from = kept["from"].as_str().unwrap_or("").to_string();
            state().index = Some(Index { items, at: kept["at"].as_u64().unwrap_or(0), from });
        }
    }
    let (first, every, due) = look_times();
    let sh = sh.clone();
    let _ = std::thread::Builder::new().name("market".into()).spawn(move || {
        std::thread::sleep(first);
        loop {
            if now_ms() >= state().next_look {
                look(&sh, due);
            }
            std::thread::sleep(every);
        }
    });
}

// --- Getting one ---------------------------------------------------------------------

// The page told how a download goes: { id, got, total, stage: download,
// verify, install, done, failed (with error) }.
fn tell(sh: &Shared, id: &str, got: u64, total: u64, stage: &str, more: Value) {
    let mut p = json!({ "id": id, "got": got, "total": total, "stage": stage });
    if let Value::Object(m) = more {
        for (k, v) in m {
            p[k] = v;
        }
    }
    let _ = sh.app.emit_to("island", "market:progress", p);
}

// The same permissions, the hosts in any order (perms_of keeps the JSON's
// order, so a hand-made index could list them another way).
pub fn same_perms(a: &Perms, b: &Perms) -> bool {
    if (a.agents, a.hardware, a.notify) != (b.agents, b.hardware, b.notify) || a.net.len() != b.net.len() {
        return false;
    }
    let (mut x, mut y) = (a.net.clone(), b.net.clone());
    x.sort();
    y.sort();
    x == y
}

// Its size and SHA-256 as the index gives them.
pub fn verify(item: &Item, size: u64, sha256: &str) -> Result<(), Value> {
    if size != item.size {
        return Err(error("size", format!("{size} bytes, not {}", item.size)));
    }
    if sha256 != item.sha256 {
        return Err(error("digest", format!("{sha256} ≠ {}", item.sha256)));
    }
    Ok(())
}

// Its zip from one place into `to`, its SHA-256 as it comes; `step(got)`
// as it does. `test`: a test's base (its http:// redirects allowed). Err:
// what went wrong, with the place's name.
fn download_from(base: &Base, item: &Item, to: &Path, cancel: &AtomicBool, test: Option<&str>, step: &dyn Fn(u64)) -> Result<(u64, String, String), Value> {
    let url = format!("{}{}", base.url, item.file);
    if !allowed(&url, test) {
        return Err(error("download", format!("{}: {url} is not the market's", base.name)));
    }
    let mut a = match crate::proxy::get(&url, DOWNLOAD_SECS, None, test.is_some()) {
        Ok(a) => a,
        Err(Miss::NotThere) => return Err(error("download", format!("{}: {} not there (404)", base.name, item.file))),
        Err(Miss::Failed(e)) => return Err(error("download", format!("{}: {e}", base.name))),
    };
    let how = format!("{}, {}, {}", base.name, a.way, a.path);
    // One byte past its size read: it is bigger than said.
    let mut reader = a.res.body_mut().with_config().limit(item.size + 1).reader();
    let mut file = std::fs::File::create(to).map_err(|e| error("write", format!("{}: {e}", to.display())))?;
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut got = 0u64;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(error("cancelled", ""));
        }
        let n = match reader.read(&mut buf) {
            Ok(n) => n,
            Err(e) => return Err(error("download", format!("{}: cut off at {got} of {} bytes: {e}", base.name, item.size))),
        };
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| error("write", format!("{}: {e}", to.display())))?;
        hash.update(&buf[..n]);
        got += n as u64;
        if got > item.size {
            return Err(error("size", format!("{}: more than the {} bytes said", base.name, item.size)));
        }
        step(got);
    }
    file.flush().map_err(|e| error("write", format!("{}: {e}", to.display())))?;
    Ok((got, hash.finalize().iter().map(|b| format!("{b:02x}")).collect(), how))
}

// Its zip from the place preferred, the other should it fail or not be the
// one the index says: Ok(where it came from, "<base>, <way>, <hosts>").
pub fn fetch_zip(bases: &[Base], item: &Item, to: &Path, cancel: &AtomicBool, test: Option<&str>, step: &dyn Fn(u64)) -> Result<String, Value> {
    let mut failed = error("download", "nowhere to get it");
    for base in bases {
        let got = download_from(base, item, to, cancel, test, step).and_then(|(size, sha, how)| verify(item, size, &sha).map(|_| how));
        match got {
            Ok(how) => return Ok(how),
            Err(e) if e["code"] == "cancelled" => return Err(e),
            Err(e) => {
                let _ = std::fs::remove_file(to);
                failed = e;
            }
        }
    }
    Err(failed)
}

// One from the market, by its id: downloaded, checked, put in (addons.rs
// put_in, as the market's; a pack where its kind goes); the page told each
// step. Ok(what went in, as a file's install says it) or Err { code,
// detail }: notFound, busy, download, size, digest, mismatch, refused
// (with addons.rs's Refused), write, cancelled.
pub fn install(sh: &Arc<Shared>, id: &str) -> Result<Value, Value> {
    if item(id).is_none() {
        index(sh, false);
    }
    let item = item(id).ok_or_else(|| error("notFound", id))?;
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut jobs = JOBS.lock().unwrap_or_else(|e| e.into_inner());
        if jobs.contains_key(id) {
            return Err(error("busy", id));
        }
        jobs.insert(id.to_string(), cancel.clone());
    }
    let got = get(sh, &item, &cancel);
    JOBS.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
    match &got {
        Ok(_) => tell(sh, id, item.size, item.size, "done", json!({ "version": item.version })),
        Err(e) => {
            sh.log(&format!("market: {id} {} not installed: {} ({})", item.version, e["detail"].as_str().unwrap_or(""), e["code"].as_str().unwrap_or("")));
            tell(sh, id, 0, item.size, "failed", json!({ "error": e }));
        }
    }
    sh.push_settings();
    got
}

fn get(sh: &Arc<Shared>, item: &Item, cancel: &AtomicBool) -> Result<Value, Value> {
    let id = item.id.as_str();
    let to = sh.dir.join("plugins").join(format!(".download-{id}.zip"));
    let _ = std::fs::create_dir_all(sh.dir.join("plugins"));
    tell(sh, id, 0, item.size, "download", Value::Null);
    let told = Lock::new("market::download told", Instant::now());
    let step = |got: u64| {
        let mut at = told.lock().unwrap_or_else(|e| e.into_inner());
        if at.elapsed() >= TELL_EVERY || got == item.size {
            *at = Instant::now();
            tell(sh, id, got, item.size, "download", Value::Null);
        }
    };
    let test = test_base();
    let how = fetch_zip(&bases(sh), item, &to, cancel, test.as_deref(), &step);
    let bytes = how.as_ref().map_err(Clone::clone).and_then(|_| std::fs::read(&to).map_err(|e| error("write", format!("{}: {e}", to.display()))));
    let _ = std::fs::remove_file(&to);
    let (how, bytes) = (how?, bytes?);
    tell(sh, id, item.size, item.size, "verify", Value::Null);
    let pkg = addons::open_package(bytes).map_err(|r| json!({ "code": "refused", "detail": r.detail, "refused": { "reason": r.reason, "file": r.file, "detail": r.detail } }))?;
    // Its kind too: a pack's item asks nothing before it goes in.
    if pkg.manifest.id != item.id || pkg.manifest.version != item.version || pkg.manifest.kind != item.kind {
        return Err(error("mismatch", format!("{} {} {} in place of {} {} {}", pkg.manifest.kind.name(), pkg.manifest.id, pkg.manifest.version, item.kind.name(), item.id, item.version)));
    }
    // Its permissions too: the page asked about (and granted) the index
    // item's; a zip whose manifest asks for others must not slip in under
    // them. Compared as what it asks (`wants`), so a plugin of a newer API
    // (whose enforced `perms` are read empty) is still checked.
    if !same_perms(&pkg.manifest.wants, &item.perms) {
        return Err(error("mismatch", format!("{} asks for {}, the market lists {}", pkg.manifest.id, pkg.manifest.wants.json(), item.perms.json())));
    }
    tell(sh, id, item.size, item.size, "install", Value::Null);
    addons::put_in(sh, pkg, "market", &format!("the market ({how})"), &item.file).map_err(|r| json!({ "code": "refused", "detail": r.detail, "refused": { "reason": r.reason, "file": r.file, "detail": r.detail } }))
}

// Every one of the market's here with a newer version (its new
// permissions granted: the page asked first), one after another.
fn update_all(sh: &Arc<Shared>) -> Value {
    let all = updates_now(sh);
    let (mut done, mut failed) = (Vec::new(), Vec::new());
    for u in &all {
        match install(sh, &u.id) {
            Ok(_) => {
                sh.log(&format!("market: {} {} → {} updated", u.id, u.from, u.to));
                done.push(json!(u.id));
            }
            Err(e) => failed.push(json!({ "id": u.id, "name": u.name, "error": e })),
        }
    }
    json!({ "ok": failed.is_empty(), "done": done, "failed": failed, "snapshot": crate::settings::snapshot(sh) })
}

// --- The settings' command ------------------------------------------------------------
// One command for all of it (each costs the exe some KB of glue): index
// (arg "force": read again), install (arg: its id; answered once it is in,
// or why not, its steps told meanwhile), updateAll, cancel (arg: its id).
#[tauri::command]
pub async fn market(app: AppHandle, what: String, arg: Option<String>) -> Value {
    let sh = shared(&app);
    let arg = arg.unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || match what.as_str() {
        "index" => index(&sh, arg == "force"),
        "install" => match install(&sh, &arg) {
            Ok(mut out) => {
                out["snapshot"] = crate::settings::snapshot(&sh);
                out
            }
            Err(e) => json!({ "ok": false, "error": e }),
        },
        "updateAll" => update_all(&sh),
        "cancel" => {
            if let Some(c) = JOBS.lock().unwrap_or_else(|e| e.into_inner()).get(&arg) {
                c.store(true, Ordering::Relaxed);
            }
            json!({ "ok": true })
        }
        _ => json!({ "ok": false, "error": error("what", format!("{what}: index, install, updateAll or cancel")) }),
    })
    .await
    .unwrap_or_else(|e| json!({ "ok": false, "error": error("what", e.to_string()) }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "7812d1ac7812d1ac7812d1ac7812d1ac7812d1ac7812d1ac7812d1ac7812d1ac";

    fn raw(id: &str, more: Value) -> Value {
        let mut v = json!({
            "id": id, "kind": "plugin", "name": { "en": "Rain alert", "zh": "下雨提醒" }, "description": "Rain soon?",
            "author": "lin", "homepage": null, "version": "1.2.0", "api": 1, "category": "info", "sizes": ["s", "m"],
            "color": "#8cc8ff", "view": false, "permissions": { "net": ["api.open-meteo.com"], "notify": true },
            "file": format!("{id}-1.2.0.zip"), "size": 1085, "sha256": SHA, "icon": null, "glyph": "weather",
            "sample": { "value": "70%", "detail": { "extra": "17:00", "series": { "points": [1, 2] } } }, "preview": null, "updated": "2026-10-21",
        });
        for (k, x) in more.as_object().unwrap() {
            v[k] = x.clone();
        }
        v
    }

    #[test]
    fn the_index_is_read_and_what_will_not_do_is_left_out() {
        let index = json!({ "format": 1, "items": [
            raw("rain-alert", json!({})),
            raw("weather", json!({})),
            raw("bad-sha", json!({ "sha256": "ABC" })),
            raw("bad-file", json!({ "file": "../x.zip" })),
            raw("rain-alert", json!({ "version": "1.3.0" })),
            raw("big", json!({ "size": 6 << 20 })),
            raw("shiba", json!({ "kind": "pet", "permissions": {}, "size": 3 << 20, "preview": "data:image/webp;base64,UklGR", "sample": { "value": "x" } })),
            raw("odd-host", json!({ "permissions": { "net": ["localhost"] } })),
        ] });
        let (items, left) = index_of(&index).unwrap();
        assert_eq!(items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), ["rain-alert", "shiba"]);
        assert_eq!(left.len(), 6, "{left:?}");
        let rain = &items[0];
        assert_eq!((rain.kind, rain.name.zh.as_str(), rain.version.as_str(), rain.glyph.as_deref()), (Kind::Plugin, "下雨提醒", "1.2.0", Some("weather")));
        assert!(rain.perms.notify && rain.perms.net == ["api.open-meteo.com"]);
        assert_eq!(rain.sample, json!({ "value": "70%", "detail": { "extra": "17:00", "series": { "points": [1.0, 2.0] } } }));
        // A pack: its picture; no sample, no colour.
        let shiba = &items[1];
        assert_eq!((shiba.kind, shiba.preview.as_deref(), &shiba.sample), (Kind::Pet, Some("data:image/webp;base64,UklGR"), &Value::Null));
        assert!(index_of(&json!({ "format": 2, "items": [] })).is_err());
        assert!(index_of(&json!({ "items": [] })).is_err());
    }

    #[test]
    fn a_sample_is_taken_as_a_widget_is() {
        let s = sample_of(&json!({ "label": "  Dresden  ", "value": "a value far longer than twenty-four", "icon": "cloud", "color": "blue", "ttl": 9, "private": true, "detail": { "rows": [{ "name": "x", "colour": "red" }], "extra": 7 } }));
        assert_eq!(s, json!({ "label": "Dresden", "value": "a value far longer than ", "private": true, "detail": { "rows": [{ "name": "x" }], "extra": "7" } }));
        assert_eq!(sample_of(&json!("70%")), Value::Null);
        assert_eq!(sample_of(&json!({ "value": "x".repeat(5000) })), Value::Null, "over 4 KB");
        assert_eq!(sample_of(&json!({ "icon": "weather" })), json!({ "value": "", "icon": "weather" }));
    }

    #[test]
    fn github_first_but_in_china_and_a_test_alone() {
        let names = |b: Vec<Base>| b.into_iter().map(|b| b.name).collect::<Vec<_>>();
        assert_eq!(names(bases_for(false, None)), ["GitHub", "Gitee"]);
        assert_eq!(names(bases_for(true, None)), ["Gitee", "GitHub"]);
        assert!(bases_for(false, None)[0].url.ends_with("/wakuwaku-plugins/releases/download/market/"));
        assert_eq!(bases_for(true, Some("http://127.0.0.1:47392/releases/download/market/".into())), [Base { name: "127.0.0.1".into(), url: "http://127.0.0.1:47392/releases/download/market/".into() }]);
        assert!(allowed("https://github.com/zhoupengjie/wakuwaku-plugins/releases/download/market/a-1.0.0.zip", None));
        assert!(allowed("https://gitee.com/zhoupengjie/wakuwaku-plugins/releases/download/market/a-1.0.0.zip", None));
        assert!(!allowed("https://github.com/someone/wakuwaku-plugins/releases/download/market/a-1.0.0.zip", None));
        assert!(!allowed("http://github.com/zhoupengjie/wakuwaku-plugins/releases/download/market/a-1.0.0.zip", None));
        assert!(!allowed("http://127.0.0.1:47392/a-1.0.0.zip", None));
        assert!(allowed("http://127.0.0.1:47392/a-1.0.0.zip", Some("http://127.0.0.1:47392/")));
    }

    fn perms(net: &[&str], agents: bool, notify: bool) -> Perms {
        Perms { net: net.iter().map(|h| h.to_string()).collect(), agents, hardware: false, notify }
    }

    #[test]
    fn what_a_new_version_asks_for_beyond_what_was_granted() {
        let granted = perms(&["api.open-meteo.com", "*.github.io"], false, true);
        assert!(new_perms(&granted, &granted).is_empty());
        assert!(new_perms(&granted, &perms(&["api.open-meteo.com"], false, false)).is_empty(), "less is no more");
        assert!(new_perms(&granted, &perms(&["lin.github.io", "*.a.github.io"], false, true)).is_empty(), "under a *. granted");
        assert_eq!(new_perms(&granted, &perms(&["api.open-meteo.com", "github.io", "x.com"], true, true)), ["net:github.io", "net:x.com", "agents"]);
        assert_eq!(new_perms(&Perms::default(), &perms(&[], false, true)), ["notify"]);
    }

    #[test]
    fn newer_versions_by_number_each_with_its_new_permissions() {
        let installed = vec![
            addons::FromMarket { id: "rain-alert".into(), kind: Kind::Plugin, name: "下雨提醒".into(), version: "1.9.0".into(), granted: perms(&["api.open-meteo.com"], false, true) },
            addons::FromMarket { id: "fx".into(), kind: Kind::Plugin, name: "汇率".into(), version: "1.4.0".into(), granted: Perms::default() },
            addons::FromMarket { id: "gone".into(), kind: Kind::Plugin, name: "Gone".into(), version: "1.0.0".into(), granted: Perms::default() },
        ];
        let item = |id: &str, version: &str, p: Perms| {
            let mut i = item_of(&raw(id, json!({ "version": version }))).unwrap();
            i.perms = p;
            i
        };
        let items = vec![item("rain-alert", "1.10.0", perms(&["api.open-meteo.com"], false, true)), item("fx", "1.4.0", Perms::default())];
        let ups = updates_of(&installed, &items);
        assert_eq!(ups, [Update { id: "rain-alert".into(), name: "下雨提醒".into(), from: "1.9.0".into(), to: "1.10.0".into(), new_perms: vec![] }]);
        let asks = updates_of(&installed, &[item("rain-alert", "2.0.0", perms(&["api.open-meteo.com"], true, true))]);
        assert_eq!(asks[0].new_perms, ["agents"]);
        // Auto-update on: only those asking for more wait; off, all.
        assert!(held(ups.clone(), true).is_empty());
        assert_eq!(held(ups, false).len(), 1);
        assert_eq!(held(asks, true).len(), 1);
        // A pack: by its kind too, asking for nothing, so it goes in by itself.
        let cat = addons::FromMarket { id: "pixel-cat".into(), kind: Kind::Pet, name: "像素猫".into(), version: "1.0.0".into(), granted: Perms::default() };
        let newer_cat = item_of(&raw("pixel-cat", json!({ "kind": "pet", "version": "1.1.0", "permissions": {}, "size": 2 << 20 }))).unwrap();
        let a_plugin_of_that_name = item_of(&raw("pixel-cat", json!({ "version": "9.0.0" }))).unwrap();
        let cat_ups = updates_of(std::slice::from_ref(&cat), &[newer_cat]);
        assert_eq!(cat_ups, [Update { id: "pixel-cat".into(), name: "像素猫".into(), from: "1.0.0".into(), to: "1.1.0".into(), new_perms: vec![] }]);
        assert!(held(cat_ups, true).is_empty());
        assert!(updates_of(&[cat], &[a_plugin_of_that_name]).is_empty(), "not another kind's");
    }

    #[test]
    fn a_download_is_its_size_and_sha256_or_nothing() {
        let i = item_of(&raw("rain-alert", json!({ "size": 3 }))).unwrap();
        assert!(verify(&i, 3, SHA).is_ok());
        assert_eq!(verify(&i, 4, SHA).unwrap_err()["code"], "size");
        assert_eq!(verify(&i, 3, &"0".repeat(64)).unwrap_err()["code"], "digest");
    }

    // The index item's permissions and the downloaded manifest's must be the
    // same (the hosts in any order), else the install would run under what
    // the page showed while the manifest asks for other things.
    #[test]
    fn a_manifest_asking_for_other_permissions_than_the_index_is_a_mismatch() {
        let a = perms(&["api.open-meteo.com", "*.github.io"], false, true);
        assert!(same_perms(&a, &perms(&["*.github.io", "api.open-meteo.com"], false, true)), "order does not matter");
        assert!(!same_perms(&a, &perms(&["api.open-meteo.com"], false, true)), "one fewer host");
        assert!(!same_perms(&a, &perms(&["api.open-meteo.com", "*.github.io"], true, true)), "a flag more");
        assert!(!same_perms(&a, &perms(&["api.open-meteo.com", "evil.com"], false, true)), "another host");
    }

    // A local server that answers each connection with the next of
    // `answers` (a whole HTTP response); its base.
    fn serve(answers: Vec<Vec<u8>>) -> Base {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for answer in answers {
                let Ok((mut s, _)) = listener.accept() else { return };
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                let _ = s.write_all(&answer);
            }
        });
        Base { name: format!("local{port}"), url: format!("http://127.0.0.1:{port}/releases/download/market/") }
    }

    fn answer(status: &str, body: &[u8]) -> Vec<u8> {
        let mut a = format!("HTTP/1.1 {status}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", body.len()).into_bytes();
        a.extend_from_slice(body);
        a
    }

    #[test]
    fn not_there_is_the_market_not_open_yet_and_a_failure_says_where() {
        // 404: not open yet, whatever the other said.
        let closed = fetch_index(&[serve(vec![answer("404 Not Found", b"")]), serve(vec![answer("500 Internal Server Error", b"")])], true).unwrap_err();
        assert_eq!(closed["code"], "closed");
        // Neither answering: failed, each place by its name, as her updates say it.
        let a = serve(vec![answer("500 Internal Server Error", b"")]);
        let failed = fetch_index(std::slice::from_ref(&a), true).unwrap_err();
        assert_eq!(failed["code"], "failed");
        assert!(failed["detail"].as_str().unwrap().starts_with(&format!("{}: straight, 127.0.0.1: ", a.name)), "{failed}");
        // Something that is no index: failed too.
        let odd = fetch_index(&[serve(vec![answer("200 OK", b"<html>login</html>")])], true).unwrap_err();
        assert_eq!(odd["code"], "failed");
        // The second place, when the first fails.
        let good = serve(vec![answer("200 OK", br#"{"format":1,"items":[]}"#)]);
        let (v, from) = fetch_index(&[serve(vec![answer("500 Internal Server Error", b"")]), good.clone()], true).unwrap();
        assert_eq!((v["format"].as_u64(), from), (Some(1), good.name));
    }

    #[test]
    fn a_zip_from_the_other_place_when_the_first_gives_another() {
        let zip = b"PK-not-really-a-zip".to_vec();
        let sha: String = Sha256::digest(&zip).iter().map(|b| format!("{b:02x}")).collect();
        let mut i = item_of(&raw("rain-alert", json!({}))).unwrap();
        i.size = zip.len() as u64;
        i.sha256 = sha;
        let mut bad = zip.clone();
        bad[0] ^= 1;
        let dir = std::env::temp_dir().join(format!("wakuwaku-market-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let to = dir.join(".download-rain-alert.zip");
        let no = AtomicBool::new(false);
        let steps = std::sync::Mutex::new(0u64);
        let step = |got: u64| *steps.lock().unwrap() = got;
        // As a test's bases, so allowed: the first gives other bytes, the second hers.
        let test = Some("http://127.0.0.1:");
        let (first, second) = (serve(vec![answer("200 OK", &bad)]), serve(vec![answer("200 OK", &zip)]));
        let how = fetch_zip(&[first, second.clone()], &i, &to, &no, test, &step).unwrap();
        assert!(how.starts_with(&format!("{}, straight, 127.0.0.1", second.name)), "{how}");
        assert_eq!(std::fs::read(&to).unwrap(), zip);
        assert_eq!(*steps.lock().unwrap(), zip.len() as u64);
        // Longer than said, or anywhere but a place of the market's: nothing.
        let longer = serve(vec![answer("200 OK", &[zip.as_slice(), b"x"].concat())]);
        assert_eq!(fetch_zip(&[longer], &i, &to, &no, test, &step).unwrap_err()["code"], "size");
        assert!(!to.exists());
        let elsewhere = serve(vec![answer("200 OK", &zip)]);
        assert_eq!(fetch_zip(&[elsewhere], &i, &to, &no, None, &step).unwrap_err()["code"], "download");
        // Called off.
        let stop = AtomicBool::new(true);
        assert_eq!(fetch_zip(&[serve(vec![answer("200 OK", &zip)])], &i, &to, &stop, test, &step).unwrap_err()["code"], "cancelled");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_looks_as_a_test_says() {
        std::env::remove_var(LOOK_TEST);
        assert_eq!(look_times(), (FIRST_LOOK, LOOK_EVERY, DAY_MS));
        std::env::set_var(LOOK_TEST, "5,30");
        assert_eq!(look_times(), (Duration::from_secs(5), Duration::from_secs(30), 30_000));
        std::env::remove_var(LOOK_TEST);
    }
}
