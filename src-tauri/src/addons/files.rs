// Third-party plugins (addons/mod.rs). Its files: her scheme (wakuplugin://).
use super::*;

// --- Its files: her scheme -------------------------------------------------------------

// The policy each of its files is served with: scripts, styles, pictures,
// fonts and sounds from its own folder (and data:, blob:), nothing else;
// no network, no frames, no forms, no workers (a worker runs off her page's
// main thread, out of the watchdog's long-task budget: none in v1); framed
// by her pages alone.
pub(super) fn policy(id: &str) -> String {
    let own = format!("{ORIGIN}/{id}/");
    format!(
        "default-src 'none'; script-src {own} blob: data: 'wasm-unsafe-eval'; style-src {own} blob: data: 'unsafe-inline'; img-src {own} blob: data:; font-src {own} blob: data:; media-src {own} blob: data:; worker-src 'none'; connect-src 'none'; frame-src 'none'; object-src 'none'; form-action 'none'; base-uri 'none'; frame-ancestors http://tauri.localhost https://tauri.localhost"
    )
}

pub(super) fn content_type(ext: &str) -> &'static str {
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
pub(super) const SDK: &str = include_str!("../../../dist/addon/sdk.js");

// A plugin frame's first page: the SDK alone, which then loads its main.
pub(super) fn host_page(id: &str) -> String {
    format!("<!doctype html>\n<html><head><meta charset=\"utf-8\"><meta name=\"referrer\" content=\"no-referrer\"><title>{id}</title></head><body><script src=\"/{id}/__waku/sdk.js\"></script></body></html>\n")
}

// Its view.html (tier 2) as a card's frame gets it: her SDK the first
// script in its head (a head made for it where it has none), so the waku
// object is there before anything of the plugin's runs; the SDK knows it is
// a view's by its page (any but __waku/main.html).
pub(super) fn view_page(id: &str, html: &str) -> String {
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
pub(super) fn tag_end(lower: &str, name: &str) -> Option<usize> {
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
pub(super) fn url_path(path: &str) -> String {
    path.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

// %xx in a path made bytes again (UTF-8); None for one that is no text.
pub(super) fn decoded(path: &str) -> Option<String> {
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
pub(super) fn respond(dir: Option<&Path>, method: &tauri::http::Method, path: &str, view_of: impl Fn(&str) -> Option<Option<String>>) -> tauri::http::Response<Vec<u8>> {
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
pub(super) fn own_file(folder: &Path, path: &str) -> Option<(Vec<u8>, &'static str)> {
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

pub(super) fn status(code: u16, words: &str) -> tauri::http::Response<Vec<u8>> {
    tauri::http::Response::builder().status(code).header("content-type", "text/plain; charset=utf-8").header("access-control-allow-origin", "*").body(words.as_bytes().to_vec()).unwrap_or_default()
}

// The pages a plugin's frame may show: its own first page (main: in the
// island's page alone), and its view.html (a card's, there or on the
// desktop), as served, nothing after them.
pub fn is_frame_page(uri: &str, main: bool) -> bool {
    read(|r| r.is_frame_page(uri, main))
}

impl Registry {
    pub(super) fn is_frame_page(&self, uri: &str, main: bool) -> bool {
        let Some(rest) = uri.strip_prefix(ORIGIN).and_then(|r| r.strip_prefix('/')) else { return false };
        let Some((id, file)) = rest.split_once('/') else { return false };
        is_id(id) && self.manifest(id).is_some_and(|m| m.runs() && ((main && file == "__waku/main.html") || (m.view.is_some() && decoded(file) == m.view)))
    }
}

// The pages a letter's HTML is drawn in on the Mail page
// (src/pet/letter.html, without its pictures from elsewhere, and
// letter-pictures.html, with them; settings-letter.ts mailBodyHTML), in her
// island's page alone, where the settings are: exactly them, as her pages
// are served (WebView2's http, or https if asked for), no query nor
// fragment to carry anything. Hers, not a plugin's, and nothing runs in
// them: their frame's sandbox has no scripts, their own policy none either
// (zhoupengjie/Wakuwaku#129: a srcdoc was kept from loading here,
// about:srcdoc being no plugin's page, and the letter showed empty).
const LETTER_PAGES: [&str; 2] = ["/pet/letter.html", "/pet/letter-pictures.html"];

pub fn is_letter_page(uri: &str, main: bool) -> bool {
    let path = uri.strip_prefix("http://tauri.localhost").or_else(|| uri.strip_prefix("https://tauri.localhost"));
    main && path.is_some_and(|p| LETTER_PAGES.contains(&p))
}

// The pages that may ask for a card's view (addon_view): her island's, and
// the desktop's.
pub(super) fn may_view(label: &str) -> bool {
    label == "island" || label.strip_prefix("desk-").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

// A plugin's views' messages this second ((since, how many)): one more, if
// it may (ten a second).
pub(super) fn may_send(sent: &mut (u64, u32), now: u64) -> bool {
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
// what it knows there in the address. The island's page may also show a
// letter in her own letter pages (is_letter_page). Said in her log, once in
// a while.
#[cfg(windows)]
pub fn guard_frames(sh: &Arc<Shared>, window: &WebviewWindow, main: bool) {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2NavigationStartingEventArgs;
    use webview2_com::NavigationStartingEventHandler;
    let sh = sh.clone();
    let _ = window.with_webview(move |view| {
        // SAFETY: WebView2's own objects, on its thread, as it hands them.
        unsafe {
            let Ok(core) = view.controller().CoreWebView2() else { return };
            let said = Lock::new("addons::files::navigation said", None::<std::time::Instant>);
            let handler = NavigationStartingEventHandler::create(Box::new(move |_, args: Option<ICoreWebView2NavigationStartingEventArgs>| {
                let Some(args) = args else { return Ok(()) };
                let mut uri = windows_core::PWSTR::null();
                args.Uri(&mut uri)?;
                let uri = webview2_com::take_pwstr(uri);
                if !is_letter_page(&uri, main) && !is_frame_page(&uri, main) {
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

