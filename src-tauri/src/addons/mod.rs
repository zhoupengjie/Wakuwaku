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
use std::sync::{Arc, LazyLock};
use std::time::{Duration, SystemTime};

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::{now_ms, shared, zip, Shared};
use crate::lock::Lock;

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

// Its parts, each in a file of its own; named here for the others (each
// has `use super::*`) and, what other modules call, for the crate.
mod files;
mod hosts;
mod manifest;
mod packages;
mod page;
mod registry;
mod running;
mod storage;
#[allow(unused_imports)]
pub use files::*;
#[allow(unused_imports)]
pub use hosts::*;
#[allow(unused_imports)]
pub use manifest::*;
#[allow(unused_imports)]
pub use packages::*;
#[allow(unused_imports)]
pub use page::*;
#[allow(unused_imports)]
pub use registry::*;
#[allow(unused_imports)]
pub use running::*;
#[allow(unused_imports)]
pub use storage::*;

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
