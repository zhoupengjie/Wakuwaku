// Third-party plugins (addons/mod.rs). Running: which the island's page runs.
use super::*;

// --- Running: which the island's page runs ----------------------------------------------

#[derive(Default)]
pub(super) struct Run {
    // What the page was told last (addons:changed), to tell it only changes.
    pub(super) sent: Option<Value>,
    // What went wrong last in each (an uncaught error, waku.log's warning).
    pub(super) said: HashMap<String, String>,
    // Those stopped by the island's watchdog (addons.ts), until turned on
    // again (revive) or installed anew.
    pub(super) stopped: HashMap<String, Stopped>,
    // Each plugin's views' messages to its main this second (addon_view
    // send): when it began, how many.
    pub(super) sends: HashMap<String, (u64, u32)>,
}

// A plugin stopped, and why: hung (its frame did not answer the watchdog,
// again within ten minutes of being loaded again for it), crashed (its page
// loaded again and again), busy (over its long tasks' budget); when, since
// when it was not answering, how many times it did not.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Stopped {
    pub(super) why: &'static str,
    pub(super) at: u64,
    pub(super) since: u64,
    pub(super) times: u64,
}

pub(super) static RUN: LazyLock<Lock<Run>> = LazyLock::new(|| Lock::new("addons::running::RUN", Run::default()));

pub(super) fn run() -> crate::lock::Guard<'static, Run> {
    RUN.lock().unwrap_or_else(|e| e.into_inner())
}

// Whether a plugin with a view runs (the desktop's pages are told when their
// views may show only then: desk/ views), and whether the island's
// watchdog has the views taken away for a while (pause_views).
pub(super) static VIEWS: AtomicBool = AtomicBool::new(false);
pub(super) static VIEWS_PAUSED: AtomicBool = AtomicBool::new(false);

pub fn views_running() -> bool {
    VIEWS.load(Ordering::Relaxed)
}

pub fn views_paused() -> bool {
    VIEWS_PAUSED.load(Ordering::Relaxed)
}

// Those the page should run now, each { id, version, rev, main, perms,
// settings }: on (placed), its required settings filled, a plugin of an API
// she has, not stopped.
pub(super) fn wanted(sh: &Shared) -> Value {
    let conf = sh.config().plugins.clone();
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

pub(super) fn ids_of(list: &Value) -> HashSet<String> {
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

pub(super) fn is_running(id: &str) -> bool {
    run().sent.as_ref().is_some_and(|l| ids_of(l).contains(id))
}

// Whether one of those running has a view; one just come, the desktop's
// pages told whether theirs may show (desk/ views).
pub(super) fn note_views(sh: &Shared, running: &HashSet<String>) {
    let views = read(|r| running.iter().any(|id| r.manifest(id).is_some_and(|m| m.view.is_some())));
    if views && !VIEWS.swap(true, Ordering::Relaxed) {
        crate::desk::views(sh);
    }
    VIEWS.store(views, Ordering::Relaxed);
}

// The plugin running whose widget this is, with its view's file: one drawn
// by a view (addon_view).
pub(super) fn view_owner(widget: &str) -> Option<(String, String)> {
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
    let conf = sh.config().plugins.clone();
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
pub(super) fn perms_of_id(id: &str) -> Option<Perms> {
    read(|r| r.manifest(id).map(|m| m.perms.clone()))
}

