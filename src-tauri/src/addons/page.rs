// Third-party plugins (addons/mod.rs). From the island's page: what a plugin asks of her.
use super::*;

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

pub(super) fn call(sh: &Shared, id: &str, what: &str, a: &Value) -> Value {
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
pub(super) fn widget_id(id: &str, sub: Option<&str>) -> Result<String, String> {
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
pub(super) fn widget(sh: &Shared, id: &str, sub: Option<&str>, data: &Value) -> Result<Value, String> {
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
pub(super) fn stop(sh: &Shared, id: &str, why: &str, since: Option<u64>, times: Option<u64>) -> Result<Value, String> {
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
// too), or back; the desktop's pages told (desk/ views).
pub(super) fn pause_views(sh: &Shared, paused: bool) {
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

pub(super) fn view_call(sh: &Shared, what: &str, widget: &str, message: Option<Value>) -> Result<Value, String> {
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
pub(super) fn notify(sh: &Shared, id: &str, text: &str) -> Result<Value, String> {
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
pub(super) fn said(sh: &Shared, id: &str, line: &str) {
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

pub(super) fn manage(sh: &Shared, what: &str, arg: &str, sha: Option<&str>) -> Value {
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

