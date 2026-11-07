// Plugins in the island, the first kind: widgets. Each is a few words a
// script keeps up to date there (or the pet herself, for the built-in ones),
// shown only while no session needs you: sessions always come first.
//
//   POST /widget { id, label, value?, detail?, icon?, color?, ttl?, nudge? }
//     id     a-z 0-9 _ -, up to 40; the same id again replaces it
//     label  up to 40 characters; value up to 24
//     detail what its larger sizes show (detail_of): { sizes, series, rows,
//            extra }; a string is its extra, up to 24, shown after the value
//            in the open island (the weather's range for the day), as before
//     icon   one of ICONS, else a dot
//     color  #rgb or #rrggbb, for the value
//     ttl    how long (s) it stays without a new one: 300 by default, 10 to 86400
//     nudge  true, or a few words: the island opens for it once, if nothing
//            else wants you; once a minute at most, per widget
//     private  true: the label (and the nudge's words) are kept off screen
//            while the specifics are (the details setting off); the value,
//            like "3 封未读", still shows
//   POST /widget { id, remove: true }
//
// A plugin of hers (plugins.rs) posts the same, and may add buttons to its
// detail (actions: what is playing has previous, play or pause, next),
// drawn on its cards and pressed back to it (plugins::act); a script's
// (nobody to press them back to) are dropped.
//
// Built in (texts as { key, vars } for the page to say in her language):
//   monitor  the machine: CPU, memory, the network's pace, the battery, each
//            on or off ("monitor" setting), read every 1 to 10 seconds, and
//            the hardware's sensors picked, Windows' own (hw.rs) or
//            LibreHardwareMonitor's (lhm.rs), by "monitorSource";
//            its value is { parts: [{ icon, text, kind?, warn? }] }, shown as
//            icon and number
//   today    turns today, how long the agents worked, approvals given on her
//            (Antigravity's never come to her)
//   tokens   today's tokens, Claude Code's, Codex's, Antigravity's, omp's, DSH's and Reasonix's (tokens.rs)
//
// Hers too, never expiring and never a script's: each mail account's
// (mail.rs, "inbox-<id>"). And a third-party plugin's (addons.rs, "<id>"
// and "<id>_<sub>"), checked as a script's, kept while it runs.
use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Value};

pub const ICONS: [&str; 26] = [
    "cpu", "weather", "note", "calendar", "bell", "stock", "today", "clock", "mail", "music", "code", "star", "dot", "chart", "battery", "timer", "flag", "server",
    "check", "terminal", "coin", "gauge", "memory", "down", "up", "bolt",
];
// The built-in ones: id, icon, colour.
pub const BUILT_IN: [(&str, &str, &str); 3] = [("today", "today", "#34d27b"), ("tokens", "code", "#ff9f0a"), ("monitor", "gauge", "#5e9bff")];
const MAX_SCRIPTED: usize = 32;
// A plugin's view (tier 2), as JSON, at most.
const VIEW: usize = 32 * 1024;
const DEFAULT_TTL: u64 = 300;
const NUDGE_EVERY_MS: u64 = 60_000;

pub struct Widget {
    pub id: String,
    label: Value,
    value: Value,
    // What its larger sizes show (detail_of), or null.
    detail: Value,
    icon: String,
    color: String,
    // When it goes, if no new one comes (ms); never, for the built-in ones.
    until: Option<u64>,
    at: u64,
    private: bool,
    // A third-party plugin's (addons.rs): kept while it runs, gone as it
    // stops; and what it gave for its own view (tier 2), with its revision
    // (the pages see the number alone, and ask for the view as it changes:
    // addons.rs addon_view), none for one whose plugin has no view.html.
    addon: bool,
    view: Value,
    view_rev: Option<u64>,
    // Its plugin's own picture for its icon (an .svg in its folder, by her
    // scheme), where it gave none of hers: the pages draw it in one colour.
    picture: Option<String>,
}

#[derive(Default)]
pub struct Widgets {
    // In the order first seen; the built-in ones first.
    list: Vec<Widget>,
    nudged: HashMap<String, u64>,
    // The last revision given a plugin's view (view_rev), counting up.
    view_revs: u64,
}

fn clip(s: &str, max: usize) -> String {
    s.trim().chars().take(max).collect()
}

pub fn is_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

fn is_color(c: &str) -> bool {
    let hex = c.strip_prefix('#').unwrap_or("");
    matches!(hex.len(), 3 | 6) && hex.chars().all(|c| c.is_ascii_hexdigit())
}

// --- A widget's detail: what its larger sizes show ----------------------------------
// A widget is small (its icon and value) at least; its detail says which
// sizes it has and gives them more to show (the page draws them,
// wcards.js):
//   sizes   some of "s", "m", "l", in that order
//   series  { points: [n…], unit, hi?, lo? }: a curve or bars, the last
//           POINTS points kept
//   rows    a list, ROWS at most, each an object of its words: who or what
//           (from, code, name, when, price, change, value, unit, kind, icon:
//           NAME characters each), what about (subject, text: SUBJECT), when
//           (at, ms) and a small curve of its own (points)
//   extra   a few words more (EXTRA): under the value, after it in a line
// So a script cannot swell the island: anything else is dropped, longer
// words cut with "…". A string, as scripts sent before (the weather's range
// for the day), is the extra.
const POINTS: usize = 120;
const ROWS: usize = 12;
const NAME: usize = 16;
const SUBJECT: usize = 60;
const EXTRA: usize = 24;
const ROW_NAMES: [&str; 10] = ["from", "code", "name", "when", "price", "change", "value", "unit", "kind", "icon"];
const ROW_SUBJECTS: [&str; 2] = ["subject", "text"];
// A widget's buttons (a plugin of hers alone): ACTIONS at most, each an id
// (as plugins::act takes it: a-z 0-9 -, 16 at most, a letter first), its
// icon (one of ACTION_ICONS, drawn by the pages as Windows draws them), and
// off (true: greyed, nothing to press: a player with no previous).
const ACTIONS: usize = 4;
const ACTION_ICONS: [&str; 5] = ["play", "pause", "prev", "next", "stop"];

// Words cut to max characters, the last of them "…" when cut.
fn cut(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

// Words, or a number as words (a price sent as one).
fn words_of(v: &Value, max: usize) -> Option<String> {
    let s = match v {
        Value::String(s) => cut(s, max),
        Value::Number(n) => cut(&n.to_string(), max),
        _ => return None,
    };
    (!s.is_empty()).then_some(s)
}

// The buttons of a plugin's widget, checked: those that will do, ACTIONS
// at most, in their order; none, null.
pub fn actions_of(v: Option<&Value>) -> Value {
    let Some(list) = v.and_then(Value::as_array) else { return Value::Null };
    let is_action = |id: &str| !id.is_empty() && id.len() <= 16 && id.starts_with(|c: char| c.is_ascii_lowercase()) && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    let kept: Vec<Value> = list
        .iter()
        .filter_map(|a| {
            let id = a.get("id")?.as_str().filter(|id| is_action(id))?;
            let icon = a.get("icon")?.as_str().filter(|i| ACTION_ICONS.contains(i))?;
            let mut out = json!({ "id": id, "icon": icon });
            if a.get("off") == Some(&Value::Bool(true)) {
                out["off"] = json!(true);
            }
            Some(out)
        })
        .take(ACTIONS)
        .collect();
    if kept.is_empty() {
        Value::Null
    } else {
        Value::Array(kept)
    }
}

// Numbers alone, finite, the last POINTS of them.
fn points_of(v: &Value) -> Option<Value> {
    let all: Vec<f64> = v.as_array()?.iter().filter_map(Value::as_f64).filter(|n| n.is_finite()).collect();
    (!all.is_empty()).then(|| json!(all[all.len().saturating_sub(POINTS)..]))
}

fn finite(v: Option<&Value>) -> Option<f64> {
    v.and_then(Value::as_f64).filter(|n| n.is_finite())
}

// A detail as sent, checked and cut: an object of what is good in it, or
// null for none.
pub fn detail_of(v: Option<&Value>) -> Value {
    let Some(v) = v else { return Value::Null };
    if v.is_string() {
        return words_of(v, EXTRA).map_or(Value::Null, |extra| json!({ "extra": extra }));
    }
    let Some(o) = v.as_object() else { return Value::Null };
    let mut out = serde_json::Map::new();
    if let Some(sizes) = o.get("sizes").and_then(Value::as_array) {
        let list: Vec<&str> = ["s", "m", "l"].into_iter().filter(|z| sizes.iter().any(|s| s.as_str() == Some(z))).collect();
        if !list.is_empty() {
            out.insert("sizes".into(), json!(list));
        }
    }
    if let Some(s) = o.get("series").and_then(Value::as_object) {
        if let Some(points) = s.get("points").and_then(points_of) {
            let mut series = serde_json::Map::new();
            series.insert("points".into(), points);
            if let Some(unit) = s.get("unit").and_then(|u| words_of(u, NAME)) {
                series.insert("unit".into(), json!(unit));
            }
            for k in ["hi", "lo"] {
                if let Some(n) = finite(s.get(k)) {
                    series.insert(k.into(), json!(n));
                }
            }
            out.insert("series".into(), Value::Object(series));
        }
    }
    if let Some(rows) = o.get("rows").and_then(Value::as_array) {
        let rows: Vec<Value> = rows
            .iter()
            .filter_map(Value::as_object)
            .map(|r| {
                let mut row = serde_json::Map::new();
                for (key, v) in r {
                    let kept = match key.as_str() {
                        k if ROW_NAMES.contains(&k) => words_of(v, NAME).map(Value::from),
                        k if ROW_SUBJECTS.contains(&k) => words_of(v, SUBJECT).map(Value::from),
                        "at" => finite(Some(v)).filter(|n| *n >= 0.0).map(|n| json!(n.round() as u64)),
                        "points" => points_of(v),
                        _ => None,
                    };
                    if let Some(kept) = kept {
                        row.insert(key.clone(), kept);
                    }
                }
                Value::Object(row)
            })
            .filter(|r| r.as_object().is_some_and(|r| !r.is_empty()))
            .take(ROWS)
            .collect();
        if !rows.is_empty() {
            out.insert("rows".into(), Value::Array(rows));
        }
    }
    if let Some(extra) = o.get("extra").and_then(|e| words_of(e, EXTRA)) {
        out.insert("extra".into(), json!(extra));
    }
    if out.is_empty() {
        Value::Null
    } else {
        Value::Object(out)
    }
}

// What a script sent, checked.
pub struct Put {
    pub id: String,
    // The words to open the island with, when it asked to.
    pub nudge: Option<String>,
}

impl Widgets {
    // A widget from a script: Err with why, when it will not do.
    pub fn put(&mut self, v: &Value, now: u64) -> Result<Put, &'static str> {
        self.put_from(v, now, false)
    }

    // The same from a plugin of hers (plugins.rs), whose buttons are kept
    // (may_act); a script's are not.
    pub fn put_from(&mut self, v: &Value, now: u64, may_act: bool) -> Result<Put, &'static str> {
        let s = |key: &str| v.get(key).and_then(Value::as_str).unwrap_or("");
        let id = s("id");
        if !is_id(id) {
            return Err("id: a-z 0-9 _ -, up to 40");
        }
        if BUILT_IN.iter().any(|b| b.0 == id) || self.list.iter().any(|w| w.id == id && w.until.is_none()) {
            return Err("id: taken by a built-in widget");
        }
        if v.get("remove").and_then(Value::as_bool) == Some(true) {
            self.list.retain(|w| w.id != id);
            return Ok(Put { id: id.into(), nudge: None });
        }
        let label = clip(s("label"), 40);
        if label.is_empty() {
            return Err("label: some words, up to 40");
        }
        let ttl = v.get("ttl").and_then(Value::as_u64).unwrap_or(DEFAULT_TTL).clamp(10, 86_400);
        let widget = made(v, id, label, Some(now + ttl * 1000), now, may_act);
        let scripted = self.list.iter().filter(|w| w.until.is_some()).count();
        match self.list.iter_mut().find(|w| w.id == id) {
            Some(old) => *old = widget,
            None if scripted >= MAX_SCRIPTED => return Err("too many widgets: 32 at most"),
            None => self.list.push(widget),
        }
        let nudge = match v.get("nudge") {
            Some(Value::Bool(true)) => Some(String::new()),
            Some(Value::String(words)) if !words.trim().is_empty() => Some(clip(words, 60)),
            _ => None,
        };
        // Once a minute at most, per widget.
        let nudge = nudge.filter(|_| self.nudged.get(id).is_none_or(|&t| now.saturating_sub(t) >= NUDGE_EVERY_MS));
        if nudge.is_some() {
            self.nudged.insert(id.into(), now);
        }
        Ok(Put { id: id.into(), nudge })
    }

    // A third-party plugin's widget (addons.rs; its id `<plugin>` or
    // `<plugin>_<sub>`), checked as a script's is, its label the plugin's
    // name when none is given. No time to live: it stays while the plugin
    // runs, and goes as it stops (remove_addons). What it gives for its own
    // view (tier 2) is kept, up to VIEW bytes of JSON; drawn by its plugin's
    // view.html (has_view) once it gives one, a new revision each time it
    // changes. Never a nudge: that is waku.notify's, by its permission.
    // picture: its manifest's .svg for an icon (sent as iconUrl), for one
    // that gives none of hers.
    pub fn put_addon(&mut self, id: &str, v: &Value, name: &str, picture: Option<String>, has_view: bool, now: u64) -> Result<(), &'static str> {
        if !is_id(id) {
            return Err("id: a-z 0-9 _ -, up to 40");
        }
        if BUILT_IN.iter().any(|b| b.0 == id) || self.list.iter().any(|w| w.id == id && w.until.is_none() && !w.addon) {
            return Err("id: taken by a built-in widget");
        }
        let given = clip(v.get("label").and_then(Value::as_str).unwrap_or(""), 40);
        let label = if given.is_empty() { clip(name, 40) } else { given };
        if label.is_empty() {
            return Err("label: some words, up to 40");
        }
        let view = v.get("view").cloned().unwrap_or(Value::Null);
        if view.to_string().len() > VIEW {
            return Err("view: up to 32 KB of JSON");
        }
        let made = made(v, id, label, None, now, false);
        let picture = picture.filter(|_| made.icon == "dot");
        let before = self.list.iter().find(|w| w.id == id && w.addon).map(|w| (&w.view, w.view_rev));
        let view_rev = match before {
            _ if !has_view || view.is_null() => None,
            Some((old, Some(rev))) if *old == view => Some(rev),
            _ => {
                self.view_revs += 1;
                Some(self.view_revs)
            }
        };
        let widget = Widget { addon: true, view, view_rev, picture, ..made };
        match self.list.iter_mut().find(|w| w.id == id) {
            Some(old) => *old = widget,
            None => self.list.push(widget),
        }
        Ok(())
    }

    // How many widgets a plugin has up (`mine`: its ids).
    pub fn addons_of(&self, mine: impl Fn(&str) -> bool) -> usize {
        self.list.iter().filter(|w| w.addon && mine(&w.id)).count()
    }

    // Whether this id is a plugin's widget up now (addon): a script's POST
    // /widget is turned away only for one of these, not for any id that
    // begins like a plugin's (finding 11).
    pub fn is_addon(&self, id: &str) -> bool {
        self.list.iter().any(|w| w.addon && w.id == id)
    }

    // The ids of a plugin's widgets up now (`mine`: its ids): the subs it has
    // actually made, for recording and for cleaning its places on uninstall.
    pub fn addon_ids(&self, mine: impl Fn(&str) -> bool) -> Vec<String> {
        self.list.iter().filter(|w| w.addon && mine(&w.id)).map(|w| w.id.clone()).collect()
    }

    // Plugins' widgets gone with them (stopped, taken away). True when any were there.
    pub fn remove_addons(&mut self, gone: impl Fn(&str) -> bool) -> bool {
        let before = self.list.len();
        self.list.retain(|w| !(w.addon && gone(&w.id)));
        self.list.len() != before
    }

    // What a plugin's widget gave for its own view (tier 2), if it is up.
    pub fn view_of(&self, id: &str) -> Option<Value> {
        self.list.iter().find(|w| w.id == id && w.addon).map(|w| w.view.clone())
    }

    // The same, with its revision, for a widget drawn by its plugin's view
    // (its plugin has one, and it gave a view).
    pub fn view_shown(&self, id: &str) -> Option<(Value, u64)> {
        self.list.iter().find(|w| w.id == id && w.addon).and_then(|w| Some((w.view.clone(), w.view_rev?)))
    }

    // Whether a widget may open the island now, by the same once a minute a
    // script's nudge has (put); counted from now when it may.
    pub fn may_nudge(&mut self, id: &str, now: u64) -> bool {
        let may = self.nudged.get(id).is_none_or(|&t| now.saturating_sub(t) >= NUDGE_EVERY_MS);
        if may {
            self.nudged.insert(id.into(), now);
        }
        may
    }

    // A built-in widget's words, now. True when they changed.
    pub fn set_built_in(&mut self, id: &str, value: Value, now: u64) -> bool {
        let detail = self.list.iter().find(|w| w.id == id).map_or(Value::Null, |w| w.detail.clone());
        self.set_built_in_with(id, value, detail, now)
    }

    // The same, with what its larger sizes show (its own, not a script's:
    // as it is).
    pub fn set_built_in_with(&mut self, id: &str, value: Value, detail: Value, now: u64) -> bool {
        if let Some(w) = self.list.iter_mut().find(|w| w.id == id) {
            if w.value == value && w.detail == detail {
                return false;
            }
            w.value = value;
            w.detail = detail;
            w.at = now;
            return true;
        }
        let at = self.list.iter().take_while(|w| is_built_in(&w.id)).count();
        self.list.insert(at, Widget { value, detail, at: now, ..built_in(id) });
        true
    }

    // One of hers that is not built in (a mail account's): its words now,
    // and its detail (its newest letters), kept until she takes it away.
    // True when they changed.
    #[allow(clippy::too_many_arguments)]
    pub fn put_owned(&mut self, id: &str, icon: &str, color: &str, label: Value, value: Value, detail: Value, private: bool, now: u64) -> bool {
        let widget = Widget { id: id.into(), label, value, detail, icon: icon.into(), color: color.into(), until: None, at: now, private, addon: false, view: Value::Null, view_rev: None, picture: None };
        match self.list.iter_mut().find(|w| w.id == id) {
            Some(old) if old.label == widget.label && old.value == widget.value && old.color == widget.color && old.detail == widget.detail => false,
            Some(old) => {
                *old = widget;
                true
            }
            None => {
                self.list.push(widget);
                true
            }
        }
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.list.len();
        self.list.retain(|w| w.id != id);
        self.list.len() != before
    }

    pub fn remove_built_in(&mut self, id: &str) -> bool {
        let before = self.list.len();
        self.list.retain(|w| w.id != id);
        self.list.len() != before
    }

    // Those whose script stopped sending go. True when any did.
    pub fn expire(&mut self, now: u64) -> bool {
        let before = self.list.len();
        self.list.retain(|w| w.until.is_none_or(|t| t > now));
        self.list.len() != before
    }

    // A widget's words, and whether they are private.
    pub fn label_of(&self, id: &str) -> Option<(Value, Value, bool)> {
        self.list.iter().find(|w| w.id == id).map(|w| (w.label.clone(), w.value.clone(), w.private))
    }

    // A script's widgets that are gone with it. True when any were there.
    pub fn remove_scripted(&mut self, gone: impl Fn(&str) -> bool) -> bool {
        let before = self.list.len();
        self.list.retain(|w| w.until.is_none() || !gone(&w.id));
        self.list.len() != before
    }

    // All of them, in the order the person put them in (then as first seen),
    // each saying whether it is on and which plugin it comes from (`owner`:
    // one she runs, plugins.rs), which places it when the plugin was placed.
    // The built-in ones not measured (off, or nothing to read yet, like a
    // battery on a desktop) are there too, with no value, so they can be
    // turned on.
    pub fn view(&self, now: u64, off: &[String], order: &[String], owner: &dyn Fn(&str) -> Option<String>) -> Value {
        let unread: Vec<Widget> = BUILT_IN.iter().filter(|b| !self.list.iter().any(|w| w.id == b.0)).map(|b| built_in(b.0)).collect();
        let measured = self.list.iter().take_while(|w| is_built_in(&w.id)).count();
        let all: Vec<&Widget> = self.list[..measured].iter().chain(&unread).chain(&self.list[measured..]).collect();
        let mut all: Vec<(&Widget, Option<String>)> = all.into_iter().map(|w| (w, owner(&w.id))).collect();
        let rank = |(w, of): &(&Widget, Option<String>)| order.iter().position(|o| *o == w.id || Some(o) == of.as_ref()).unwrap_or(usize::MAX);
        all.sort_by_key(|w| rank(w));
        Value::Array(
            all.into_iter()
                .map(|(w, of)| {
                    let mut out = json!({
                        "id": w.id,
                        "label": w.label,
                        "value": w.value,
                        "detail": w.detail,
                        "icon": w.icon,
                        "color": w.color,
                        "builtIn": is_built_in(&w.id),
                        // builtIn, mail (hers, a mail account's), addon (a
                        // third-party plugin's: addons.rs), or script.
                        "from": if is_built_in(&w.id) { "builtIn" } else if w.addon { "addon" } else if w.until.is_none() { "mail" } else { "script" },
                        "plugin": of,
                        "on": !off.contains(&w.id),
                        "leftMs": w.until.map(|t| t.saturating_sub(now)),
                        "at": w.at,
                        "private": w.private,
                    });
                    // A plugin's own picture for its icon (none for the rest: not sent).
                    if let Some(p) = &w.picture {
                        out["iconUrl"] = json!(p);
                    }
                    // Drawn by its plugin's view: the revision of what it
                    // gave, the view itself asked for as it changes (not
                    // sent with every widget).
                    if let Some(rev) = w.view_rev {
                        out["viewRev"] = json!(rev);
                    }
                    out
                })
                .collect(),
        )
    }
}

fn is_built_in(id: &str) -> bool {
    BUILT_IN.iter().any(|b| b.0 == id)
}

// A built-in widget with nothing to say yet.
fn built_in(id: &str) -> Widget {
    let (icon, color) = BUILT_IN.iter().find(|b| b.0 == id).map_or(("dot", ""), |b| (b.1, b.2));
    Widget { id: id.into(), label: json!({ "key": format!("widget.{id}") }), value: Value::Null, detail: Value::Null, icon: icon.into(), color: color.into(), until: None, at: 0, private: false, addon: false, view: Value::Null, view_rev: None, picture: None }
}

// A widget as sent (a script's, a plugin's), its label given: its value,
// detail, icon, colour and privacy checked and cut; its buttons kept in its
// detail where it may have them (may_act: a plugin of hers).
fn made(v: &Value, id: &str, label: String, until: Option<u64>, now: u64, may_act: bool) -> Widget {
    let s = |key: &str| v.get(key).and_then(Value::as_str).unwrap_or("");
    let mut detail = detail_of(v.get("detail"));
    let actions = if may_act { actions_of(v.get("detail").and_then(|d| d.get("actions"))) } else { Value::Null };
    if !actions.is_null() {
        if detail.is_null() {
            detail = json!({});
        }
        detail["actions"] = actions;
    }
    Widget {
        id: id.into(),
        label: json!(label),
        value: json!(clip(s("value"), 24)),
        detail,
        icon: if ICONS.contains(&s("icon")) { s("icon").into() } else { "dot".into() },
        color: if is_color(s("color")) { s("color").into() } else { String::new() },
        until,
        at: now,
        private: v.get("private").and_then(Value::as_bool) == Some(true),
        addon: false,
        view: Value::Null,
        view_rev: None,
        picture: None,
    }
}

// --- Today: what the day has seen ---------------------------------------------------

#[derive(Default, Clone, PartialEq, Debug)]
pub struct Today {
    pub date: String,
    pub turns: u64,
    // Of those, Antigravity's and DeepSeek Harness's: their approvals never come to her.
    pub agy_turns: u64,
    pub worked_ms: u64,
    pub approvals: u64,
    // The turns begun in each hour of the day, local time (the widget's bars).
    pub hours: [u64; 24],
}

impl Today {
    pub fn load(dir: &Path, date: &str) -> Today {
        let saved: Value = std::fs::read_to_string(dir.join("today.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
        if saved["date"].as_str() != Some(date) {
            return Today { date: date.into(), ..Today::default() };
        }
        let n = |k: &str| saved[k].as_u64().unwrap_or(0);
        let mut hours = [0; 24];
        for (h, v) in hours.iter_mut().zip(saved["hours"].as_array().into_iter().flatten()) {
            *h = v.as_u64().unwrap_or(0);
        }
        Today { date: date.into(), turns: n("turns"), agy_turns: n("agyTurns"), worked_ms: n("workedMs"), approvals: n("approvals"), hours }
    }

    pub fn save(&self, dir: &Path) {
        let body = json!({ "date": self.date, "turns": self.turns, "agyTurns": self.agy_turns, "workedMs": self.worked_ms, "approvals": self.approvals, "hours": self.hours });
        let _ = std::fs::write(dir.join("today.json"), body.to_string());
    }

    // Turns begun now: counted, and in this hour of the day.
    pub fn begun(&mut self, turns: u64) {
        self.turns += turns;
        self.hours[local_hour()] += turns;
    }

    // What its larger sizes show: the turns hour by hour (bars, the page
    // lights the hour it is). Its tokens are the tokens widget's, which the
    // page puts with it.
    pub fn detail(&self) -> Value {
        json!({ "sizes": ["s", "m", "l"], "series": { "points": self.hours, "unit": "turns" } })
    }

    // A new day starts from nothing. True when it did.
    pub fn roll(&mut self, date: &str) -> bool {
        if self.date == date {
            return false;
        }
        *self = Today { date: date.into(), ..Today::default() };
        true
    }

    // Turns, the time as h:mm (under a minute, in seconds: a few short turns
    // are no 0:00), approvals; with Antigravity's or DeepSeek Harness's turns
    // alone, "—" for them.
    pub fn words(&self) -> Value {
        let minutes = self.worked_ms / 60_000;
        let time = match self.worked_ms {
            1..=59_999 => json!({ "key": "widget.todaySeconds", "vars": { "n": (self.worked_ms / 1000).max(1).to_string() } }),
            _ => json!(format!("{}:{:02}", minutes / 60, minutes % 60)),
        };
        let key = if self.turns > 0 && self.agy_turns == self.turns && self.approvals == 0 { "widget.todayValueAgy" } else { "widget.todayValue" };
        json!({ "key": key, "vars": { "turns": self.turns.to_string(), "time": time, "approvals": self.approvals.to_string() } })
    }
}

// The local date, YYYY-MM-DD.
pub fn local_date() -> String {
    let (y, m, d, _) = local_now();
    format!("{y:04}-{m:02}-{d:02}")
}

// Local midnight, in ms since 1970.
pub fn local_midnight() -> u64 {
    crate::now_ms().saturating_sub(local_now().3)
}

// The hour of the day, local time, 0 to 23.
pub fn local_hour() -> usize {
    ((local_now().3 / 3_600_000) as usize).min(23)
}

// The local year, month, day, and ms since midnight (a deadline's countdown
// too: plugins/countdown.rs).
pub(crate) fn local_now() -> (u16, u16, u16, u64) {
    #[cfg(windows)]
    {
        #[repr(C)]
        #[derive(Default)]
        struct SystemTime {
            year: u16,
            month: u16,
            weekday: u16,
            day: u16,
            hour: u16,
            minute: u16,
            second: u16,
            ms: u16,
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GetLocalTime(time: *mut SystemTime);
        }
        let mut t = SystemTime::default();
        // SAFETY: fills our own struct.
        unsafe { GetLocalTime(&mut t) };
        let ms = ((u64::from(t.hour) * 60 + u64::from(t.minute)) * 60 + u64::from(t.second)) * 1000 + u64::from(t.ms);
        (t.year, t.month, t.day, ms)
    }
    #[cfg(unix)]
    {
        // The system's local time (its time zone, summer time and all).
        let now = crate::now_ms();
        let secs = (now / 1000) as libc::time_t;
        // SAFETY: localtime_r fills our own tm from our own time_t.
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        if unsafe { libc::localtime_r(&secs, &mut tm) }.is_null() {
            return utc_now(now);
        }
        let ms = ((tm.tm_hour as u64 * 60 + tm.tm_min as u64) * 60 + tm.tm_sec as u64) * 1000 + now % 1000;
        ((tm.tm_year + 1900) as u16, (tm.tm_mon + 1) as u16, tm.tm_mday as u16, ms)
    }
    #[cfg(not(any(windows, unix)))]
    {
        utc_now(crate::now_ms())
    }
}

// The day in UTC, when the local time is not to be had.
#[cfg(not(windows))]
fn utc_now(now: u64) -> (u16, u16, u16, u64) {
    // Days since 1970 to a date (Howard Hinnant's civil_from_days, as crash.rs).
    let z = (now / 86_400_000) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y as u16, m as u16, d as u16, now % 86_400_000)
}

// --- The machine: CPU and memory ----------------------------------------------------------

// A network adapter's MIB_IFROW that counts: Ethernet (6) or Wi-Fi (71),
// up (5), with an address of its own: its MAC, bytes in and out (32 bits).
#[cfg_attr(not(windows), allow(dead_code))]
fn counted_row(row: &[u8]) -> Option<([u8; 8], u32, u32)> {
    let dword = |at: usize| row.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let (kind, state) = (dword(516)?, dword(544)?);
    let mac: [u8; 8] = row.get(532..540)?.try_into().ok()?;
    ((kind == 6 || kind == 71) && state == 5 && mac != [0; 8]).then_some((mac, dword(552)?, dword(576)?))
}

// Each adapter once, by its MAC (the most its rows say), summed.
#[cfg_attr(not(windows), allow(dead_code))]
fn by_adapter(rows: impl IntoIterator<Item = ([u8; 8], u32, u32)>) -> (u64, u64) {
    let mut by_mac: Vec<([u8; 8], u32, u32)> = Vec::new();
    for (mac, got, sent) in rows {
        match by_mac.iter_mut().find(|m| m.0 == mac) {
            Some(m) => (m.1, m.2) = (m.1.max(got), m.2.max(sent)),
            None => by_mac.push((mac, got, sent)),
        }
    }
    by_mac.iter().fold((0, 0), |(a, b), &(_, got, sent)| (a + u64::from(got), b + u64::from(sent)))
}

#[cfg(windows)]
mod sys {
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    #[repr(C)]
    struct MemoryStatus {
        length: u32,
        load: u32,
        total_phys: u64,
        avail_phys: u64,
        total_page: u64,
        avail_page: u64,
        total_virtual: u64,
        avail_virtual: u64,
        avail_extended: u64,
    }

    #[repr(C)]
    #[derive(Default)]
    struct PowerStatus {
        ac_line: u8,
        battery_flag: u8,
        percent: u8,
        saver: u8,
        life_secs: u32,
        full_life_secs: u32,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetSystemTimes(idle: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> i32;
        fn GlobalMemoryStatusEx(status: *mut MemoryStatus) -> i32;
        fn GetSystemPowerStatus(status: *mut PowerStatus) -> i32;
    }

    // Windows' callback for an IP interface that came, went or changed.
    type InterfaceChanged = extern "system" fn(context: *mut std::ffi::c_void, row: *mut std::ffi::c_void, kind: i32);

    #[link(name = "iphlpapi")]
    extern "system" {
        fn GetIfTable(table: *mut u8, size: *mut u32, order: i32) -> u32;
        fn GetIfEntry(row: *mut u8) -> u32;
        fn GetNumberOfInterfaces(count: *mut u32) -> u32;
        fn NotifyIpInterfaceChange(family: u16, callback: InterfaceChanged, context: *mut std::ffi::c_void, initial: u8, handle: *mut isize) -> u32;
    }

    // The battery as Windows' quick settings show it, or None without a
    // battery. Its state unknown (255), the level still counts.
    pub fn power() -> Option<super::Power> {
        let mut p = PowerStatus::default();
        // SAFETY: fills our own struct.
        if unsafe { GetSystemPowerStatus(&mut p) } == 0 || (p.battery_flag != 255 && p.battery_flag & 128 != 0) || p.percent > 100 {
            return None;
        }
        let plugged = p.ac_line == 1;
        let known = p.battery_flag != 255;
        Some(super::Power {
            percent: p.percent,
            plugged,
            charging: known && p.battery_flag & 8 != 0,
            saver: p.saver & 1 != 0,
            critical: known && p.battery_flag & 4 != 0,
            left: (p.life_secs != u32::MAX && !plugged).then_some(p.life_secs),
        })
    }

    // Windows' own battery glyphs (SysBatt Fluent Icons), the font its
    // taskbar draws the battery in, from where Windows keeps it; None before
    // Windows 11 has it.
    pub fn battery_font() -> Option<Vec<u8>> {
        let windows = std::env::var_os("WINDIR").map(std::path::PathBuf::from).unwrap_or_else(|| r"C:\Windows".into());
        [r"SystemApps\MicrosoftWindows.Client.Core_cw5n1h2txyewy\SystemTray\Assets\Sysbatt.ttf", r"SystemResources\Windows.UI.ControlCenter\Assets\Sysbatt.ttf"]
            .iter()
            .find_map(|at| std::fs::read(windows.join(at)).ok())
    }

    // Windows' "Battery percentage" (Settings > System > Power & battery):
    // the percentage beside the battery on the taskbar.
    pub fn battery_percent_shown() -> bool {
        use winreg::enums::HKEY_CURRENT_USER;
        winreg::RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced")
            .and_then(|k| k.get_value::<u32, _>("IsBatteryPercentageEnabled"))
            .is_ok_and(|on| on != 0)
    }

    // (percent, charging, seconds left if known), or None without a battery.
    pub fn battery() -> Option<(u8, bool, Option<u32>)> {
        let mut p = PowerStatus::default();
        // SAFETY: fills our own struct.
        if unsafe { GetSystemPowerStatus(&mut p) } == 0 || p.battery_flag & 128 != 0 || p.battery_flag == 255 || p.percent > 100 {
            return None;
        }
        let is_charging = p.ac_line == 1;
        Some((p.percent, is_charging, (p.life_secs != u32::MAX && !is_charging).then_some(p.life_secs)))
    }

    // MIB_IFROW: 860 bytes; its index at 512, the type at 516, the MAC at
    // 532 (8), the state at 544, bytes in at 552, bytes out at 576.
    const ROW: usize = 860;
    const NET_EVERY: std::time::Duration = std::time::Duration::from_secs(60);

    // Set as Windows says an IP interface came, went or changed (a link up
    // or down): the table read whole at the next turn.
    static NET_CHANGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

    extern "system" fn net_changed(_context: *mut std::ffi::c_void, _row: *mut std::ffi::c_void, _kind: i32) {
        NET_CHANGED.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    // The network's bytes in and out, over the adapters that are up: each
    // adapter once (Windows lists one several times, through its filters:
    // 65 rows on the user's PC, 8 of them counted). GetIfTable asks every
    // row's counters of the drivers (2.2 ms there, and it was asked twice,
    // for its size first: 4.5 ms a turn); so the whole table is read only
    // every minute, when the number of interfaces changes, when Windows says
    // one changed (NotifyIpInterfaceChange) or when a row counted is no
    // longer, and in between only the rows counted (GetIfEntry: 0.25 ms),
    // the same rows and the same numbers.
    pub struct Net {
        buf: Vec<u8>,
        // The rows counted at the last whole reading, by their index.
        rows: Vec<u32>,
        interfaces: u32,
        looked: Option<std::time::Instant>,
    }

    impl Net {
        pub fn new() -> Net {
            static WATCHED: std::sync::Once = std::sync::Once::new();
            WATCHED.call_once(|| {
                let mut handle = 0isize;
                // SAFETY: a callback that only sets a flag, for every family
                // (AF_UNSPEC), kept for her whole run (never cancelled).
                unsafe { NotifyIpInterfaceChange(0, net_changed, std::ptr::null_mut(), 0, &mut handle) };
            });
            Net { buf: Vec::new(), rows: Vec::new(), interfaces: 0, looked: None }
        }

        // Bytes in and out so far, and whether the adapters counted changed
        // since the last reading (a rate across that would be wrong: one come
        // up adds its whole count, one gone takes it away).
        pub fn read(&mut self) -> Option<(u64, u64, bool)> {
            let mut interfaces = 0u32;
            // SAFETY: fills our own count.
            unsafe { GetNumberOfInterfaces(&mut interfaces) };
            let changed = NET_CHANGED.swap(false, std::sync::atomic::Ordering::SeqCst);
            let due = self.looked.is_none_or(|t| t.elapsed() >= NET_EVERY);
            if !(due || changed || interfaces != self.interfaces) {
                if let Some(bytes) = self.rows_again() {
                    return Some((bytes.0, bytes.1, false));
                }
            }
            let (bytes, rows) = self.whole()?;
            let first = self.looked.is_none();
            let other = rows != self.rows;
            (self.rows, self.interfaces, self.looked) = (rows, interfaces, Some(std::time::Instant::now()));
            Some((bytes.0, bytes.1, other && !first))
        }

        // The rows counted last time, each asked again; None should one not
        // be there or not count any more.
        fn rows_again(&self) -> Option<(u64, u64)> {
            let mut row = [0u8; ROW];
            let mut counted = Vec::with_capacity(self.rows.len());
            for &index in &self.rows {
                row[512..516].copy_from_slice(&index.to_le_bytes());
                // SAFETY: a MIB_IFROW of our own, its index set.
                if unsafe { GetIfEntry(row.as_mut_ptr()) } != 0 {
                    return None;
                }
                counted.push(super::counted_row(&row)?);
            }
            Some(super::by_adapter(counted))
        }

        // The whole table (its buffer kept, so asked once), and the rows
        // that count, by their index.
        fn whole(&mut self) -> Option<((u64, u64), Vec<u32>)> {
            for _ in 0..3 {
                let mut size = self.buf.len() as u32;
                // SAFETY: a buffer of `size` bytes; one too small is told
                // how large it must be (ERROR_INSUFFICIENT_BUFFER).
                let status = unsafe { GetIfTable(if self.buf.is_empty() { std::ptr::null_mut() } else { self.buf.as_mut_ptr() }, &mut size, 0) };
                if status == 122 {
                    self.buf.resize(size as usize + 4 * ROW, 0);
                    continue;
                }
                if status != 0 {
                    return None;
                }
                let count = u32::from_le_bytes(self.buf[..4].try_into().ok()?) as usize;
                let rows: Vec<&[u8]> = (0..count).map(|i| 4 + i * ROW).take_while(|&at| at + ROW <= self.buf.len()).map(|at| &self.buf[at..at + ROW]).collect();
                let counted: Vec<_> = rows.iter().filter(|r| super::counted_row(r).is_some()).collect();
                let indexes = counted.iter().map(|r| u32::from_le_bytes(r[512..516].try_into().unwrap())).collect();
                return Some((super::by_adapter(counted.iter().filter_map(|r| super::counted_row(r))), indexes));
            }
            None
        }
    }

    fn n(t: FileTime) -> u64 {
        (u64::from(t.high) << 32) | u64::from(t.low)
    }

    // (idle, busy) time so far, all cores. Not what Task Manager shows on
    // every machine: on some Windows 11 22H2 and 23H2 ones the idle time it
    // is made of (NtQuerySystemInformation's SystemProcessorPerformanceInformation)
    // grows while the cores are busy, and a CPU at 45% read 1 to 3% (#51);
    // HWiNFO and MSI Afterburner moved off it for the same. Only where
    // Windows' own count (Counter) is not to be had.
    pub fn times() -> Option<(u64, u64)> {
        let (mut idle, mut kernel, mut user) = (FileTime::default(), FileTime::default(), FileTime::default());
        // SAFETY: fills our own structs.
        let ok = unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } != 0;
        // Kernel time includes the idle time.
        ok.then(|| (n(idle), n(kernel) + n(user) - n(idle)))
    }

    pub fn memory_load() -> Option<u32> {
        memory().map(|m| m.0)
    }

    // The memory's load (%), its total and what is available (bytes).
    pub fn memory() -> Option<(u32, u64, u64)> {
        // SAFETY: a struct of the size the call asks for.
        unsafe {
            let mut s: MemoryStatus = std::mem::zeroed();
            s.length = std::mem::size_of::<MemoryStatus>() as u32;
            (GlobalMemoryStatusEx(&mut s) != 0).then_some((s.load, s.total_phys, s.avail_phys))
        }
    }

    // How long Windows has been up (ms).
    pub fn up_ms() -> u64 {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetTickCount64() -> u64;
        }
        // SAFETY: no arguments.
        unsafe { GetTickCount64() }
    }

    // Windows' build (26100 for 24H2), or 0 when it cannot be read.
    pub fn windows_build() -> u32 {
        use winreg::enums::HKEY_LOCAL_MACHINE;
        winreg::RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
            .and_then(|k| k.get_value::<String, _>("CurrentBuildNumber"))
            .ok()
            .and_then(|b| b.trim().parse().ok())
            .unwrap_or(0)
    }

    // One of Windows' counters, by its English path (the same whatever
    // Windows' language), in a query of its own (hw.rs pdh): opened once
    // (the counters' names looked up then: ~0.3 s on this thread, ~4 MB
    // that PDH keeps), collected each turn (~0.2 ms).
    pub struct Counter(crate::hw::pdh::Query);

    impl Counter {
        pub fn open(path: &str) -> Option<Counter> {
            crate::hw::pdh::Query::open(&[path]).map(Counter)
        }

        // Its value since the last time, or None when Windows has none to
        // give (too soon after the first sample, or the counter failed).
        pub fn value(&mut self) -> Option<f64> {
            if !self.0.collect() {
                return None;
            }
            self.0.value(0, crate::hw::pdh::DOUBLE)
        }
    }
}

#[cfg(not(windows))]
mod sys {
    pub fn times() -> Option<(u64, u64)> {
        None
    }
    pub fn memory_load() -> Option<u32> {
        None
    }
    pub fn memory() -> Option<(u32, u64, u64)> {
        None
    }
    pub fn up_ms() -> u64 {
        0
    }
    pub fn battery() -> Option<(u8, bool, Option<u32>)> {
        None
    }
    pub fn power() -> Option<super::Power> {
        None
    }
    pub fn battery_font() -> Option<Vec<u8>> {
        None
    }
    pub fn battery_percent_shown() -> bool {
        false
    }
    pub struct Net;
    impl Net {
        pub fn new() -> Net {
            Net
        }
        pub fn read(&mut self) -> Option<(u64, u64, bool)> {
            None
        }
    }
    pub fn windows_build() -> u32 {
        0
    }
    pub struct Counter;
    impl Counter {
        pub fn open(_path: &str) -> Option<Counter> {
            None
        }
        pub fn value(&mut self) -> Option<f64> {
            None
        }
    }
}

// The counter Task Manager's CPU figure is, by Windows' build. Its
// Performance and Processes pages showed "% Processor Utility" from
// Windows 8 on: the work done, against the cores' base speed, so past
// 100% (held at 100) while they run faster. Windows 11 24H2 (26100) went
// over to the share of time busy, "% Processor Time", in its May 2025
// update (KB5058411): tried on 26300 here, Task Manager 56% with two of
// four cores busy at 4.5 GHz, % Processor Time 55%, % Processor Utility 101%.
pub fn processor_counter(build: u32) -> &'static str {
    if build >= 26100 {
        r"\Processor Information(_Total)\% Processor Time"
    } else {
        r"\Processor Information(_Total)\% Processor Utility"
    }
}

// The processor's use, 0 to 100: Windows' count when one was read (Task
// Manager's figure; past 100 under Utility, held at 100), else the share
// of time busy between two readings of GetSystemTimes, (idle, busy) passed;
// None with neither.
fn cpu_share(counted: Option<f64>, between: Option<(u64, u64)>) -> Option<u64> {
    if let Some(p) = counted.filter(|p| p.is_finite()) {
        return Some(p.round().clamp(0.0, 100.0) as u64);
    }
    let (idle, busy) = between?;
    Some(if idle + busy == 0 { 0 } else { (busy * 100 + (idle + busy) / 2) / (idle + busy) })
}

// The battery: its level, plugged in, charging (a battery kept from charging
// while plugged in, as a laptop's battery care does at 80%, is plugged in and
// not charging), the battery saver on, critically low, the seconds left when
// Windows knows them.
pub struct Power {
    percent: u8,
    plugged: bool,
    charging: bool,
    saver: bool,
    critical: bool,
    left: Option<u32>,
}

// The battery for the taskbar's quick settings, beside the network and the
// volume as Windows' own shows it there on a laptop (taskbar.rs keys_loop,
// island.js showBattery), with whether Windows shows its percentage beside
// it; null without a battery (a desktop), and none is shown.
pub fn battery_state() -> Value {
    match sys::power() {
        Some(p) => json!({
            "percent": p.percent,
            "plugged": p.plugged,
            "charging": p.charging,
            "saver": p.saver,
            "critical": p.critical,
            "left": p.left,
            "shown": sys::battery_percent_shown(),
        }),
        None => Value::Null,
    }
}

// Windows' battery font (taskbar_battery_font), or nothing to have it from.
pub fn battery_font() -> Vec<u8> {
    sys::battery_font().unwrap_or_default()
}

// Bytes a second, in a few characters: 512B, 8.4K, 80K, 1.2M, 35M, 1.1G.
pub fn rate(bytes_per_sec: f64) -> String {
    let (n, unit) = match bytes_per_sec.max(0.0) {
        b if b >= 1e9 => (b / 1e9, "G"),
        b if b >= 1e6 => (b / 1e6, "M"),
        b if b >= 1e3 => (b / 1e3, "K"),
        b => return format!("{b:.0}B"),
    };
    if n < 10.0 { format!("{n:.1}{unit}") } else { format!("{n:.0}{unit}") }
}

// The machine as a plugin reads it (addons.rs, waku.hardware), when it asks
// and on no thread of its own: the CPU's share of time busy and the
// network's pace since the asking before (GetSystemTimes and the adapters'
// counters, as the monitor reads them where Windows' own count is not to be
// had), and the memory's load. Asked for the first time, or after a long
// while, it reads twice a third of a second apart. The graphics card's use
// is null: its readings (hw.rs) need a query of Windows' counters, some 4 MB
// and a third of a second to open, which only the monitor keeps.
pub fn reading_now() -> Value {
    struct Last {
        at: std::time::Instant,
        times: Option<(u64, u64)>,
        bytes: Option<(u64, u64)>,
        net: sys::Net,
    }
    static LAST: std::sync::Mutex<Option<Last>> = std::sync::Mutex::new(None);
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    // (times, bytes, whether the adapters counted changed, when).
    let take = |net: &mut sys::Net| {
        let read = net.read();
        (sys::times(), read.map(|(a, b, _)| (a, b)), read.is_some_and(|r| r.2), std::time::Instant::now())
    };
    if last.as_ref().is_none_or(|l| l.at.elapsed() > std::time::Duration::from_secs(30)) {
        let mut net = last.take().map_or_else(sys::Net::new, |l| l.net);
        let (times, bytes, _, at) = take(&mut net);
        *last = Some(Last { at, times, bytes, net });
        std::thread::sleep(std::time::Duration::from_millis(330));
    }
    let Some(l) = last.as_mut() else { return Value::Null };
    let (times, bytes, other, at) = take(&mut l.net);
    let secs = at.duration_since(l.at).as_secs_f64().max(0.1);
    let cpu = times.zip(l.times).and_then(|(now, then)| cpu_share(None, Some((now.0.saturating_sub(then.0), now.1.saturating_sub(then.1)))));
    // The counters are 32 bits: they come round again after 4 GB.
    let pace = |a: u64, b: u64| ((a as u32).wrapping_sub(b as u32) as f64 / secs).round();
    let net = bytes.zip(l.bytes).filter(|_| !other).map(|(now, then)| (pace(now.0, then.0), pace(now.1, then.1)));
    (l.at, l.times, l.bytes) = (at, times, bytes);
    json!({ "cpu": cpu, "memory": sys::memory_load(), "gpu": null, "netDown": net.map(|n| n.0), "netUp": net.map(|n| n.1) })
}

// The monitor's settings: which parts are on, and how often it reads; the
// hardware's sensors, built in (hw.rs) or from LibreHardwareMonitor
// (lhm.rs), by the source chosen ("monitorSource"; none chosen, decided as
// it reads, hw::Source::in_effect); whether nobody is at the keyboard, and
// whether the settings are open (their list of sensors then read whole)
// (its thread says, main.rs).
pub struct Watching {
    pub cpu: bool,
    pub memory: bool,
    pub net: bool,
    pub battery: bool,
    pub every: u64,
    // The source chosen, if one was; LHM's old switch turned off.
    pub chosen: Option<crate::hw::Source>,
    pub lhm_off: bool,
    pub hw: crate::hw::Conf,
    pub lhm: crate::lhm::Conf,
    pub quiet: bool,
    pub listing: bool,
    // The pointer on her home (its card may be looked at): the hardware its
    // card shows read too (the cards' GPU, drives and CPU speed), not only
    // what is picked; never while nobody is.
    pub looking: bool,
}

impl Watching {
    // The "monitor" setting and "monitorSource".
    pub fn from(v: &Value, source: &Value) -> Watching {
        let on = |k: &str| v[k] != false;
        let chosen = crate::hw::Source::chosen(source);
        Watching {
            cpu: on("cpu"),
            memory: on("mem"),
            net: on("net"),
            battery: on("battery"),
            every: v["every"].as_u64().unwrap_or(2).clamp(1, 10),
            chosen,
            lhm_off: v["lhm"] == false,
            hw: crate::hw::Conf::from(v),
            // Asked while it is the source chosen, whatever its old switch
            // said, or while none is (to know whether it is running).
            lhm: crate::lhm::Conf { on: crate::hw::Source::asks_lhm(v, source), ..crate::lhm::Conf::from(v) },
            quiet: false,
            listing: false,
            looking: false,
        }
    }
}

// The last minute of a reading, for a curve.
#[derive(Default)]
struct Minute(std::collections::VecDeque<(std::time::Instant, f64)>);

impl Minute {
    const SPAN: std::time::Duration = std::time::Duration::from_secs(60);

    fn push(&mut self, v: f64) {
        let now = std::time::Instant::now();
        while self.0.front().is_some_and(|(at, _)| now.duration_since(*at) > Minute::SPAN) {
            self.0.pop_front();
        }
        self.0.push_back((now, v));
    }

    fn points(&self) -> Vec<f64> {
        self.0.iter().map(|(_, v)| (v * 10.0).round() / 10.0).collect()
    }

    fn clear(&mut self) {
        self.0.clear();
    }
}

// What the monitor widget's larger sizes show, as read this turn: the CPU's
// use (and over the last minute), speed and cores; the memory's load, in
// use and total; the busiest card (its use, name, temperature) and drive
// (its time busy, model), while they are read (Watching::looking); the
// network's pace down and up (down over the last minute); how long Windows
// has been up.
#[derive(Default)]
pub struct Seen {
    pub cpu: Option<u64>,
    pub cpu_minute: Vec<f64>,
    pub mhz: Option<f64>,
    pub cores: Option<(u64, u64)>,
    pub memory: Option<(u32, u64, u64)>,
    pub gpu: Option<(f64, String, Option<f64>)>,
    pub disk: Option<(f64, String)>,
    pub net: Option<(f64, f64)>,
    pub down_minute: Vec<f64>,
    pub up_ms: u64,
}

// Its detail (detail_of's shape): the CPU's minute as the series (0 to
// 100); a row for each of what was read, by its kind (cpu, memory, gpu,
// disk, net, uptime), its value in words as the island has them; one not
// read is left out, and its card says it has no data.
pub fn monitor_detail(s: &Seen) -> Value {
    let gb = |b: u64| b as f64 / 1_073_741_824.0;
    let mut rows = Vec::new();
    if let Some(cpu) = s.cpu {
        let mut row = json!({ "kind": "cpu", "value": format!("{cpu}%") });
        if let Some(mhz) = s.mhz {
            row["text"] = json!(format!("{:.1} GHz", mhz / 1000.0));
        }
        if let Some((cores, threads)) = s.cores {
            (row["name"], row["unit"]) = (json!(cores.to_string()), json!(threads.to_string()));
        }
        rows.push(row);
    }
    if let Some((load, total, avail)) = s.memory {
        rows.push(json!({ "kind": "memory", "value": format!("{load}%"), "text": format!("{:.1} / {:.0} GB", gb(total.saturating_sub(avail)), gb(total)) }));
    }
    if let Some((load, name, temp)) = &s.gpu {
        // Its model without its maker's names (RTX 4080 Laptop GPU).
        let model = ["NVIDIA ", "GeForce ", "AMD ", "Intel(R) "].iter().fold(name.trim(), |n, p| n.strip_prefix(p).unwrap_or(n));
        let mut row = json!({ "kind": "gpu", "value": format!("{:.0}%", load), "name": cut(model, NAME * 2) });
        if let Some(t) = temp {
            row["text"] = json!(format!("{t:.0}°"));
        }
        rows.push(row);
    }
    if let Some((busy, model)) = &s.disk {
        rows.push(json!({ "kind": "disk", "value": format!("{:.0}%", busy), "name": cut(model, NAME * 2) }));
    }
    if let Some((down, up)) = s.net {
        rows.push(json!({ "kind": "net", "value": rate(down), "text": rate(up), "points": s.down_minute }));
    }
    if s.up_ms > 0 {
        rows.push(json!({ "kind": "uptime", "value": (s.up_ms / 60_000 * 60).to_string() }));
    }
    let mut detail = json!({ "sizes": ["s", "m", "l"], "rows": rows });
    if !s.cpu_minute.is_empty() {
        detail["series"] = json!({ "points": s.cpu_minute, "unit": "%", "hi": 100, "lo": 0 });
    }
    detail
}

// Of the built-in source's last turn, what the monitor widget's card shows:
// the CPU's speed and cores, the card with a temperature (a discrete one)
// else the busiest, the busiest drive; and the ids to read for them next
// turn while it may be looked at (each card's use and temperature, each
// drive's time busy, the CPU's speed).
fn card_hardware(round: &crate::hw::Round) -> (Option<f64>, Option<(u64, u64)>, Option<(f64, String, Option<f64>)>, Option<(f64, String)>, Vec<String>) {
    use crate::hw::Class;
    let value = |id: &str| round.readings.iter().find(|r| r.id == id).and_then(|r| r.value);
    let mhz = value("/cpu/0/clock/0");
    let cores = round.hardware.iter().find(|h| h.class == Class::Cpu).and_then(|h| Some((h.about["vars"]["cores"].as_u64()?, h.about["vars"]["threads"].as_u64()?)));
    let mut also = vec!["/cpu/0/clock/0".to_string()];
    let mut gpus = Vec::new();
    let mut disks = Vec::new();
    for h in &round.hardware {
        match h.class {
            Class::Gpu => {
                also.extend([format!("{}/load/0", h.id), format!("{}/temperature/0", h.id)]);
                if let Some(load) = value(&format!("{}/load/0", h.id)) {
                    gpus.push((load, h.name.clone(), value(&format!("{}/temperature/0", h.id))));
                }
            }
            Class::Disk => {
                also.push(format!("{}/load/0", h.id));
                if let Some(busy) = value(&format!("{}/load/0", h.id)) {
                    disks.push((busy, h.name.clone()));
                }
            }
            _ => {}
        }
    }
    let gpu = gpus.iter().find(|g| g.2.is_some()).or_else(|| gpus.iter().max_by(|a, b| a.0.total_cmp(&b.0))).cloned();
    let disk = disks.into_iter().max_by(|a, b| a.0.total_cmp(&b.0));
    (mhz, cores, gpu, disk, also)
}

// The machine as the monitor reads it: CPU use and the network's pace
// between two readings, memory in use, the battery.
pub struct Monitor {
    times: Option<(u64, u64)>,
    // Windows' own count of the CPU's use (processor_counter), while the CPU
    // part is on: opened at its first reading, tried again only once it is
    // turned off and on. Given up after three readings in a row without a
    // value; GetSystemTimes then, as where it could not be opened.
    counter: Option<sys::Counter>,
    counter_tried: bool,
    misses: u8,
    // The network's adapters, read once it is first on.
    adapters: Option<sys::Net>,
    bytes: Option<(u64, u64, std::time::Instant)>,
    // The last CPU use and pace read: kept for a reading that fails once,
    // so a part on stays in sight (pinned, the island keeps its width).
    cpu: Option<u64>,
    net: Option<(f64, f64)>,
    lhm: crate::lhm::Lhm,
    hw: crate::hw::Hw,
    // The hardware's source in effect at the last reading.
    source: crate::hw::Source,
    // The widget's larger sizes (monitor_detail): the last minute of CPU use
    // and of the pace down, the hardware its card asks to read while it may
    // be looked at, and its detail as last read.
    cpu_minute: Minute,
    down_minute: Minute,
    also: Vec<String>,
    detail: Value,
}

impl Monitor {
    pub fn new() -> Self {
        Monitor {
            times: None,
            counter: None,
            counter_tried: false,
            misses: 0,
            adapters: None,
            bytes: None,
            cpu: None,
            net: None,
            lhm: crate::lhm::Lhm::new(),
            hw: crate::hw::Hw::new(),
            source: crate::hw::Source::Builtin,
            cpu_minute: Minute::default(),
            down_minute: Minute::default(),
            also: Vec::new(),
            detail: Value::Null,
        }
    }

    // What the widget's larger sizes show, as last read (monitor_detail).
    pub fn detail(&self) -> Value {
        self.detail.clone()
    }

    // Turned off: the hardware's counters and handles let go of (opened
    // again when it is turned on), LibreHardwareMonitor no longer asked.
    pub fn rest(&mut self) {
        self.hw.stop();
        self.lhm.turn(&crate::lhm::Conf { on: false, ..crate::lhm::Conf::from(&Value::Null) }, false);
        self.cpu_minute.clear();
        self.down_minute.clear();
        self.detail = Value::Null;
    }

    // The hardware's source in effect at the last reading.
    pub fn source(&self) -> crate::hw::Source {
        self.source
    }

    // What the built-in source found, for the dump (hw.rs).
    pub fn seen(&self) -> Value {
        self.hw.seen()
    }

    // The parts that are on, as icon and number: cpu 12%, memory 45%, down
    // 1.2M and up 80K a second, battery (a bolt while charging) 85%; then
    // the hardware's sensors picked, with their kind too: { icon: "gpu",
    // kind: "temp", text: "58°" }, from the source chosen (hw.rs, lhm.rs);
    // a drive's temperature with its own warning: { …, warn: 84 }. A part
    // needing two readings shows from its second, and keeps its last when a
    // reading fails. None with nothing to show.
    pub fn read(&mut self, w: &Watching) -> Option<Value> {
        let mut parts = Vec::new();
        let mut basics = crate::hw::Basics::default();
        let mut part = |icon: &str, text: String| parts.push(json!({ "icon": icon, "text": text }));
        if w.cpu && !self.counter_tried {
            self.counter_tried = true;
            self.counter = sys::Counter::open(processor_counter(sys::windows_build()));
        }
        let counted = self.counter.as_mut().and_then(sys::Counter::value);
        if self.counter.is_some() {
            self.misses = if counted.is_some() { 0 } else { self.misses + 1 };
            if self.misses >= 3 {
                self.counter = None;
            }
        }
        // GetSystemTimes each turn all the same (a call of nothing), so it
        // has a reading to go on from should the counter be given up.
        let times = if w.cpu { sys::times() } else { None };
        let between = times.zip(self.times).map(|(now, (idle, busy))| (now.0.saturating_sub(idle), now.1.saturating_sub(busy)));
        // With the counter open, its value or the last one kept (its first
        // sample has none): never a turn of the other way's.
        if let Some(share) = cpu_share(counted, if self.counter.is_some() { None } else { between }) {
            self.cpu = Some(share);
        }
        if !w.cpu {
            self.cpu = None;
            self.counter = None;
            self.counter_tried = false;
            self.misses = 0;
        }
        if times.is_some() || !w.cpu {
            self.times = times;
        }
        if let Some(cpu) = self.cpu {
            part("cpu", format!("{cpu}%"));
            basics.cpu = Some(cpu as f64);
        }
        let memory = w.memory.then(sys::memory).flatten();
        if let Some((load, _, _)) = memory {
            part("memory", format!("{load}%"));
            basics.memory = Some(f64::from(load));
        }
        let read = if w.net { self.adapters.get_or_insert_with(sys::Net::new).read() } else { None };
        // The adapters counted changed: no pace across that (the last kept).
        let other = read.is_some_and(|r| r.2);
        let bytes = read.map(|(a, b, _)| (a, b, std::time::Instant::now()));
        if let (Some((got, sent, now)), Some((g, s, at)), false) = (bytes, self.bytes, other) {
            let secs = now.duration_since(at).as_secs_f64().max(0.5);
            // The counters are 32 bits: they come round again after 4 GB.
            let d = |a: u64, b: u64| (a as u32).wrapping_sub(b as u32) as f64 / secs;
            self.net = Some((d(got, g), d(sent, s)));
        }
        if !w.net {
            self.net = None;
        }
        if bytes.is_some() || !w.net {
            self.bytes = bytes;
        }
        if let Some((down, up)) = self.net {
            part("down", rate(down));
            part("up", rate(up));
            basics.net = Some((down, up));
        }
        // The battery for its row in the settings even while its part is off.
        let battery = (w.battery || w.listing).then(sys::battery).flatten();
        if let Some((percent, is_charging, _)) = battery {
            if w.battery {
                part(if is_charging { "bolt" } else { "battery" }, format!("{percent}%"));
            }
            basics.battery = Some((percent, is_charging));
        }
        // LibreHardwareMonitor asked only while it is the source chosen, or
        // none is (else its view says off); the source in effect then
        // decided, and only its readings read.
        self.lhm.turn(&w.lhm, w.quiet);
        self.source = crate::hw::Source::in_effect(w.chosen, w.lhm_off, self.lhm.found());
        if self.source == crate::hw::Source::Lhm {
            parts.extend(self.lhm.parts(&w.lhm));
        }
        let mut seen = Seen { cpu: self.cpu, memory, net: self.net, up_ms: sys::up_ms(), ..Seen::default() };
        if self.source == crate::hw::Source::Builtin {
            // The card's hardware too while it may be looked at (found at the
            // turn before: every card and drive is listed, read or not).
            let toggles = [w.cpu, w.memory, w.net, w.battery];
            if w.looking {
                self.hw.turn_also(&w.hw, w.listing, &self.also, &basics, toggles);
            } else {
                self.hw.turn(&w.hw, w.listing, &basics, toggles);
            }
            parts.extend(self.hw.parts(&w.hw));
            (seen.mhz, seen.cores, seen.gpu, seen.disk, self.also) = card_hardware(self.hw.round());
        } else {
            self.hw.stop();
        }
        // The widget's curves, a point a reading.
        if let Some(cpu) = self.cpu {
            self.cpu_minute.push(cpu as f64);
        }
        if let Some((down, _)) = self.net {
            self.down_minute.push(down);
        }
        (seen.cpu_minute, seen.down_minute) = (self.cpu_minute.points(), self.down_minute.points());
        self.detail = monitor_detail(&seen);
        (!parts.is_empty()).then(|| json!({ "parts": parts }))
    }
}

// One whole reading of the built-in source, as the open settings have it,
// for her exe's hw::DUMP_FLAG (main.rs): read twice a second apart (rates
// need two), every part and every sensor on, nothing else of her started;
// with what was found (every display adapter, kept or not; the drives) and
// what each group of readings cost the second time.
pub fn hw_dump() -> Value {
    let mut w = Watching::from(&json!({}), &json!("builtin"));
    w.listing = true;
    let mut m = Monitor::new();
    let t = std::time::Instant::now();
    m.read(&w);
    let first = t.elapsed();
    std::thread::sleep(std::time::Duration::from_secs(1));
    let t = std::time::Instant::now();
    let island = m.read(&w);
    let second = t.elapsed();
    // The monitor's own readings, each timed once more (µs).
    let mut net = sys::Net::new();
    let us = |f: &mut dyn FnMut()| {
        let t = std::time::Instant::now();
        f();
        t.elapsed().as_micros() as u64
    };
    let basics = json!({
        "cpuCounter": us(&mut || {
            let _ = m.counter.as_mut().map(sys::Counter::value);
        }),
        "systemTimes": us(&mut || {
            let _ = sys::times();
        }),
        "memoryLoad": us(&mut || {
            let _ = sys::memory_load();
        }),
        // The adapters' whole table (a new reader: its buffer sized first),
        // then a turn's rows alone.
        "networkWhole": us(&mut || {
            let _ = net.read();
        }),
        "network": us(&mut || {
            let _ = net.read();
        }),
        "battery": us(&mut || {
            let _ = sys::battery();
        }),
    });
    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "windowsBuild": sys::windows_build(),
        "cpuCounter": processor_counter(sys::windows_build()),
        "readMs": [first.as_secs_f64() * 1000.0, second.as_secs_f64() * 1000.0],
        "source": m.source().name(),
        "basicsUs": basics,
        // The island's parts for someone who never picked (the defaults).
        "island": island,
        "seen": m.seen(),
        "view": crate::hw::view(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // The view without the built-in ones, which are always there.
    fn scripted(view: Value) -> Value {
        Value::Array(view.as_array().unwrap().iter().filter(|w| w["builtIn"] == false).cloned().collect())
    }

    #[test]
    fn the_local_date_is_a_real_date() {
        let date = local_date();
        let parts: Vec<u16> = date.split('-').map(|p| p.parse().unwrap()).collect();
        assert_eq!(parts.len(), 3, "{date}");
        assert!(parts[0] >= 2026 && (1..=12).contains(&parts[1]) && (1..=31).contains(&parts[2]), "{date}");
        assert!(crate::now_ms() - local_midnight() < 86_400_000);
    }

    #[cfg(not(windows))]
    #[test]
    fn the_utc_day_is_counted_from_1970() {
        assert_eq!(utc_now(0), (1970, 1, 1, 0));
        // 2026-10-13T11:20:00.250Z
        assert_eq!(utc_now(1_791_890_400_250), (2026, 10, 13, 40_800_250));
        // 2024-02-29T00:00:00Z, a leap day
        assert_eq!(utc_now(1_709_164_800_000), (2024, 2, 29, 0));
    }

    #[test]
    fn a_script_puts_replaces_and_removes_its_widget() {
        let mut w = Widgets::default();
        let put = w.put(&json!({ "id": "weather", "label": "上海 · 多云", "value": "22°", "icon": "weather", "color": "#ffb340" }), 1_000).unwrap();
        assert_eq!((put.id.as_str(), put.nudge), ("weather", None));
        assert_eq!(scripted(w.view(1_000, &[], &[], &|_| None))[0]["detail"], Value::Null, "no detail sent");
        w.put(&json!({ "id": "weather", "label": "上海 · 晴", "value": "24°", "detail": " 14°~22° ", "icon": "nope", "color": "red" }), 2_000).unwrap();
        let view = scripted(w.view(2_000, &[], &[], &|_| None));
        assert_eq!(view.as_array().unwrap().len(), 1);
        assert_eq!((view[0]["label"].as_str(), view[0]["icon"].as_str(), view[0]["color"].as_str()), (Some("上海 · 晴"), Some("dot"), Some("")));
        // A string, as scripts sent it before: the extra.
        assert_eq!(view[0]["detail"], json!({ "extra": "14°~22°" }));
        assert_eq!(view[0]["leftMs"], 300_000);
        w.put(&json!({ "id": "weather", "label": "x", "detail": "一二三四五六七八九十一二三四五六七八九十一二三四五六" }), 2_500).unwrap();
        let extra = scripted(w.view(2_500, &[], &[], &|_| None))[0]["detail"]["extra"].as_str().unwrap().to_string();
        assert_eq!((extra.chars().count(), extra.ends_with('…')), (24, true));
        w.put(&json!({ "id": "weather", "remove": true }), 3_000).unwrap();
        assert!(scripted(w.view(3_000, &[], &[], &|_| None)).as_array().unwrap().is_empty());
    }

    // A third-party plugin's widget (addons.rs): checked as a script's, its
    // name for a label none is given, no time to live, its own and no
    // script's; gone with it. Its plugin's own picture for an icon goes as
    // iconUrl, only while it gives none of hers.
    #[test]
    fn a_plugins_widget_stays_while_it_runs() {
        let mut w = Widgets::default();
        let picture = || Some("http://wakuplugin.localhost/hello/icon.svg".to_string());
        w.put_addon("hello", &json!({ "value": "3", "icon": "star", "detail": { "sizes": ["s", "m"] }, "view": { "n": 3 } }), "数数", picture(), true, 0).unwrap();
        w.put_addon("hello_two", &json!({ "label": "Two", "value": "x".repeat(30) }), "数数", picture(), true, 0).unwrap();
        let view = scripted(w.view(10_000_000, &[], &[], &|id| (id == "hello" || id.starts_with("hello_")).then(|| "hello".to_string())));
        assert_eq!(view.as_array().unwrap().len(), 2);
        assert_eq!((view[0]["label"].as_str(), view[0]["from"].as_str(), view[0]["plugin"].as_str(), view[0]["leftMs"].is_null()), (Some("数数"), Some("addon"), Some("hello"), true));
        assert_eq!((view[1]["label"].as_str(), view[1]["value"].as_str().map(|v| v.chars().count())), (Some("Two"), Some(24)));
        assert!(view[0].get("view").is_none(), "its view kept here, not sent with every widget");
        assert_eq!((view[0].get("iconUrl"), view[1]["icon"].as_str(), view[1]["iconUrl"].as_str()), (None, Some("dot"), Some("http://wakuplugin.localhost/hello/icon.svg")));
        assert_eq!(w.view_of("hello"), Some(json!({ "n": 3 })));
        // Its view's revision sent in its place; one that gave none, none.
        assert_eq!((view[0]["viewRev"].as_u64(), view[1].get("viewRev")), (Some(1), None));
        assert_eq!(w.view_shown("hello"), Some((json!({ "n": 3 }), 1)));
        assert_eq!(w.view_shown("hello_two"), None);
        // The same view again: the same revision; another: the next.
        w.put_addon("hello", &json!({ "value": "4", "view": { "n": 3 } }), "数数", None, true, 0).unwrap();
        assert_eq!(w.view_shown("hello").map(|v| v.1), Some(1));
        w.put_addon("hello", &json!({ "value": "5", "view": { "n": 5 } }), "数数", None, true, 0).unwrap();
        assert_eq!(w.view_shown("hello"), Some((json!({ "n": 5 }), 2)));
        // A plugin with no view.html: kept, never drawn by a view.
        w.put_addon("other", &json!({ "value": "1", "view": { "n": 1 } }), "x", None, false, 0).unwrap();
        assert_eq!((w.view_of("other"), w.view_shown("other")), (Some(json!({ "n": 1 })), None));
        assert!(w.remove_addons(|id| id == "other"));
        assert!(!w.expire(u64::MAX));
        // A script cannot take its id; nor can it take a built-in one's or a big view.
        assert!(w.put(&json!({ "id": "hello", "label": "fake" }), 0).is_err());
        assert!(w.put_addon("monitor", &json!({}), "x", None, true, 0).is_err());
        assert!(w.put_addon("hello", &json!({ "view": "x".repeat(VIEW) }), "x", None, true, 0).is_err());
        assert_eq!(w.addons_of(|id| id.starts_with("hello")), 2);
        assert!(w.may_nudge("addon:hello", 0) && !w.may_nudge("addon:hello", 59_000) && w.may_nudge("addon:hello", 60_000));
        assert!(w.remove_addons(|id| id == "hello_two"));
        assert!(w.remove_addons(|id| id.starts_with("hello")));
        assert!(scripted(w.view(0, &[], &[], &|_| None)).as_array().unwrap().is_empty());
    }

    // A script widget is turned away only for a plugin's widget up now, not
    // for any id that begins like a plugin's: a `cpu` plugin with no
    // `cpu_temp` widget does not block the person's `cpu_temp` (finding 11).
    #[test]
    fn a_plugin_claims_only_its_own_widgets() {
        let mut w = Widgets::default();
        assert!(!w.is_addon("cpu_temp"), "nothing yet");
        // The person's script widget cpu_temp goes in: no plugin made it.
        assert!(w.put(&json!({ "id": "cpu_temp", "label": "CPU 温度", "value": "61°" }), 0).is_ok());
        // A cpu plugin with its own widget (cpu, cpu_load): those are claimed.
        w.put_addon("cpu", &json!({ "value": "20%" }), "CPU", None, false, 0).unwrap();
        w.put_addon("cpu_load", &json!({ "value": "1.2" }), "CPU", None, false, 0).unwrap();
        assert!(w.is_addon("cpu") && w.is_addon("cpu_load"));
        assert!(!w.is_addon("cpu_temp"), "the person’s, not the plugin’s");
        assert_eq!(w.addon_ids(|id| id == "cpu" || id.starts_with("cpu_")).len(), 2, "its two, not cpu_temp");
    }

    #[test]
    fn bad_widgets_say_why() {
        let mut w = Widgets::default();
        assert!(w.put(&json!({ "id": "Has Space", "label": "x" }), 0).is_err());
        assert!(w.put(&json!({ "id": "x" }), 0).is_err());
        assert!(w.put(&json!({ "id": "monitor", "label": "mine" }), 0).is_err());
        for n in 0..MAX_SCRIPTED {
            w.put(&json!({ "id": format!("w{n}"), "label": "x" }), 0).unwrap();
        }
        assert!(w.put(&json!({ "id": "one-more", "label": "x" }), 0).is_err());
    }

    #[test]
    fn widgets_go_when_their_script_stops_and_nudge_once_a_minute() {
        let mut w = Widgets::default();
        let put = w.put(&json!({ "id": "cal", "label": "15:00 设计评审", "ttl": 10, "nudge": "5 分钟后开会" }), 0).unwrap();
        assert_eq!(put.nudge.as_deref(), Some("5 分钟后开会"));
        assert!(w.put(&json!({ "id": "cal", "label": "15:00 设计评审", "ttl": 10, "nudge": true }), 30_000).unwrap().nudge.is_none());
        assert_eq!(w.put(&json!({ "id": "cal", "label": "15:00 设计评审", "ttl": 10, "nudge": true }), 61_000).unwrap().nudge.as_deref(), Some(""));
        assert!(!w.expire(70_000));
        assert!(w.expire(71_001));
    }

    #[test]
    fn built_ins_stay_first_and_the_order_is_the_persons() {
        let mut w = Widgets::default();
        w.put(&json!({ "id": "a", "label": "A" }), 0).unwrap();
        assert!(w.set_built_in("monitor", json!("1"), 0));
        assert!(!w.set_built_in("monitor", json!("1"), 5));
        let ids = |v: Value| v.as_array().unwrap().iter().map(|w| w["id"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        // The ones not measured are listed too, without a value.
        let none = |_: &str| None;
        let all = w.view(0, &[], &[], &none);
        assert_eq!(ids(all.clone()), ["monitor", "today", "tokens", "a"]);
        assert_eq!((all[0]["value"].clone(), all[1]["value"].clone(), all[1]["icon"].clone()), (json!("1"), Value::Null, json!("today")));
        assert_eq!(ids(w.view(0, &[], &["a".into()], &none))[0], "a");
        assert_eq!(w.view(0, &["tokens".into()], &[], &none)[2]["on"], false);
        // A plugin's widgets go where the plugin was put, and say whose they are.
        let mine = |id: &str| (id == "a").then(|| "mine".to_string());
        let view = w.view(0, &[], &["mine".into()], &mine);
        assert_eq!((view[0]["id"].clone(), view[0]["plugin"].clone()), (json!("a"), json!("mine")));
        assert!(w.remove_scripted(|id| id == "a") && !w.remove_scripted(|id| id == "sys"));
        // A mail account's: hers, kept, not a script's to take over.
        assert!(w.put_owned("inbox-1", "mail", "", json!("王总：周五的方案"), json!("3"), Value::Null, true, 0));
        assert!(!w.put_owned("inbox-1", "mail", "", json!("王总：周五的方案"), json!("3"), Value::Null, true, 9));
        assert!(w.put(&json!({ "id": "inbox-1", "label": "mine" }), 0).is_err());
        assert_eq!(w.view(0, &[], &[], &none)[3]["from"], "mail");
        assert!(!w.expire(u64::MAX) && w.remove("inbox-1"));
        w.put(&json!({ "id": "a", "label": "A" }), 0).unwrap();
        // Built-in ones never go; a script's does.
        assert!(w.expire(u64::MAX));
        assert_eq!(ids(w.view(0, &[], &[], &none)), ["monitor", "today", "tokens"]);
    }

    #[test]
    fn private_widgets_say_so_and_rates_read_short() {
        let mut w = Widgets::default();
        w.put(&json!({ "id": "mail", "label": "王总：周五的方案", "value": "3 封未读", "private": true }), 0).unwrap();
        assert_eq!(scripted(w.view(0, &[], &[], &|_| None))[0]["private"], true);
        assert_eq!(w.label_of("mail").map(|l| l.2), Some(true));
        assert_eq!([rate(80.0), rate(8_400.0), rate(81_234.0), rate(1_234_567.0), rate(250_000_000.0)], ["80B", "8.4K", "81K", "1.2M", "250M"]);
        let all = Watching::from(&json!({ "every": 99 }), &Value::Null);
        assert_eq!((all.cpu, all.battery, all.every), (true, true, 10));
        // None chosen: LibreHardwareMonitor asked, to know whether it runs;
        // chosen, asked while it is the one, whatever its old switch said.
        assert_eq!((all.chosen, all.lhm.on), (None, true));
        let theirs = Watching::from(&json!({ "lhm": false, "sensors": ["/amdcpu/0/temperature/2"] }), &json!("lhm"));
        assert_eq!((theirs.chosen, theirs.lhm.on, theirs.lhm.sensors.map(|s| s.len())), (Some(crate::hw::Source::Lhm), true, Some(1)));
        let built_in = Watching::from(&json!({}), &json!("builtin"));
        assert_eq!((built_in.chosen, built_in.lhm.on), (Some(crate::hw::Source::Builtin), false));
    }

    // The network's rows (MIB_IFROW) as the user's PC has them: an adapter
    // listed four times through its filters (each once), one up and one
    // down of each kind, the loopback and one without an address not counted.
    #[test]
    fn the_network_counts_each_adapter_up_once() {
        let row = |kind: u32, state: u32, mac: u8, got: u32, sent: u32| {
            let mut r = vec![0u8; 860];
            r[516..520].copy_from_slice(&kind.to_le_bytes());
            r[544..548].copy_from_slice(&state.to_le_bytes());
            if mac > 0 {
                r[532..538].copy_from_slice(&[0x04, 0x7c, 0x16, 0, 0, mac]);
            }
            r[552..556].copy_from_slice(&got.to_le_bytes());
            r[576..580].copy_from_slice(&sent.to_le_bytes());
            r
        };
        let realtek = (0..4).map(|_| row(6, 5, 1, 4_085_686_149, 843_917_867));
        let hyper_v = (0..3).map(|_| row(6, 5, 2, 1_069_205_980, 3_285_133_868)).chain([row(6, 5, 2, 1_069_205_990, 3_285_133_800)]);
        let others = [row(6, 0, 3, 0, 0), row(71, 2, 4, 9, 9), row(24, 5, 5, 7, 7), row(6, 5, 0, 7, 7)];
        let rows: Vec<Vec<u8>> = realtek.chain(hyper_v).chain(others).collect();
        assert_eq!(rows.iter().filter(|r| counted_row(r).is_some()).count(), 8);
        assert_eq!(counted_row(&row(71, 5, 4, 9, 8)), Some(([0x04, 0x7c, 0x16, 0, 0, 4, 0, 0], 9, 8)));
        assert_eq!(counted_row(&[0u8; 100]), None);
        // The most each adapter's rows say, summed.
        assert_eq!(by_adapter(rows.iter().filter_map(|r| counted_row(r))), (4_085_686_149 + 1_069_205_990, 843_917_867 + 3_285_133_868));
    }

    // On this machine: the whole table first, then the rows counted alone,
    // the adapters the same.
    #[cfg(windows)]
    #[test]
    fn the_network_is_read_again_by_its_rows() {
        let mut net = sys::Net::new();
        let (got, sent, _) = net.read().expect("the table");
        let (again, again_sent, other) = net.read().expect("its rows");
        assert!(!other);
        assert!(again.abs_diff(got) < 1 << 30 && again_sent.abs_diff(sent) < 1 << 30, "{got} {again}");
    }

    // A LibreHardwareMonitor answering and none chosen: its sensors, the
    // source in effect LHM's (nothing saved); none answering: the built-in
    // one; the built-in one chosen: never asked.
    #[test]
    fn none_chosen_is_lhm_while_it_answers() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let body = include_str!("../../test/fixtures/lhm/data.json");
        let asked = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = asked.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let mut request = [0u8; 2048];
                let _ = stream.read(&mut request);
                counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let _ = stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            }
        });
        let parts = |v: Option<Value>| v.map(|v| v["parts"].as_array().unwrap().iter().map(|p| format!("{}:{}", p["icon"].as_str().unwrap(), p["kind"].as_str().unwrap_or(""))).collect::<Vec<_>>()).unwrap_or_default();
        let settings = json!({ "cpu": false, "mem": false, "net": false, "battery": false, "lhmPort": port, "builtinSensors": [] });
        let mut m = Monitor::new();
        assert_eq!(parts(m.read(&Watching::from(&settings, &Value::Null))), ["cpu:temp", "gpu:temp", "gpu:load"]);
        assert_eq!(m.source(), crate::hw::Source::Lhm);
        // The built-in one chosen: LHM not asked again, none of its sensors.
        let before = asked.load(std::sync::atomic::Ordering::SeqCst);
        assert!(parts(m.read(&Watching::from(&settings, &json!("builtin")))).is_empty());
        assert_eq!((m.source(), asked.load(std::sync::atomic::Ordering::SeqCst)), (crate::hw::Source::Builtin, before));
        // Nobody on the port: the built-in one.
        let free = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let mut m = Monitor::new();
        let nobody = json!({ "cpu": false, "mem": false, "net": false, "battery": false, "lhmPort": free, "builtinSensors": [] });
        assert!(parts(m.read(&Watching::from(&nobody, &Value::Null))).is_empty());
        assert_eq!(m.source(), crate::hw::Source::Builtin);
    }

    // On this machine: the second reading has CPU use and the network's pace.
    // Windows' counters (sys): elsewhere there is nothing to read yet. No
    // hardware's sensor picked: none of them.
    #[cfg(windows)]
    #[test]
    fn the_monitor_reads_this_machine() {
        let all = Watching::from(&json!({ "every": 99, "builtinSensors": [] }), &json!("builtin"));
        let mut m = Monitor::new();
        let _ = m.read(&all);
        let icons = |v: Option<Value>| v.map(|v| v["parts"].as_array().unwrap().iter().map(|p| p["icon"].as_str().unwrap().to_string()).collect::<Vec<_>>()).unwrap_or_default();
        let second = icons(m.read(&all));
        assert!(second.starts_with(&["cpu".into(), "memory".into()]), "{second:?}");
        let only = Watching::from(&json!({ "cpu": false, "net": false, "battery": false, "builtinSensors": [] }), &json!("builtin"));
        assert_eq!(icons(m.read(&only)), ["memory"]);
        // The CPU's speed picked: a clock after the machine's own.
        let speed = Watching::from(&json!({ "net": false, "battery": false, "builtinSensors": ["/cpu/0/clock/0"] }), &json!("builtin"));
        let _ = m.read(&speed);
        std::thread::sleep(std::time::Duration::from_millis(300));
        let parts = m.read(&speed).unwrap();
        assert_eq!(parts["parts"].as_array().unwrap().last().unwrap()["kind"], "clock", "{parts}");
    }

    // The CPU's use as Task Manager has it: Windows' count, rounded and held
    // at 100; without it, the time busy between two readings.
    #[test]
    fn the_cpu_is_windows_count_else_the_time_busy() {
        assert_eq!(cpu_share(Some(44.6), Some((990, 10))), Some(45));
        assert_eq!(cpu_share(Some(173.2), None), Some(100));
        assert_eq!(cpu_share(Some(-0.4), None), Some(0));
        // No count (none read yet, the counter given up, or not a number):
        // the time busy, rounded.
        assert_eq!(cpu_share(None, Some((550, 450))), Some(45));
        assert_eq!(cpu_share(Some(f64::NAN), Some((1, 2))), Some(67));
        assert_eq!(cpu_share(None, Some((0, 0))), Some(0));
        assert_eq!(cpu_share(None, None), None);
        // Utility before 24H2, the time busy from it on.
        assert!(processor_counter(22631).ends_with(r"\% Processor Utility"));
        assert!(processor_counter(19045).ends_with(r"\% Processor Utility"));
        assert!(processor_counter(26100).ends_with(r"\% Processor Time"));
        assert!(processor_counter(26300).starts_with(r"\Processor Information(_Total)\"));
    }

    // Windows' counter on this machine: opened (its first sample taken),
    // a share of 0 to 100 at its second; one Windows has not, never opened.
    #[cfg(windows)]
    #[test]
    fn windows_counts_this_machines_cpu() {
        assert!(sys::windows_build() >= 10240);
        let mut counter = sys::Counter::open(processor_counter(sys::windows_build())).expect("the processor's counter");
        std::thread::sleep(std::time::Duration::from_millis(300));
        let share = cpu_share(counter.value(), None).expect("a value at the second sample");
        assert!(share <= 100, "{share}");
        assert!(sys::Counter::open(r"\Processor Information(_Total)\% No Such Counter").is_none());
    }

    #[test]
    fn today_counts_and_starts_again_each_day() {
        let dir = std::env::temp_dir().join(format!("wakuwaku-today-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut t = Today::load(&dir, "2026-10-09");
        t.turns = 12;
        t.worked_ms = 103 * 60_000;
        t.approvals = 5;
        t.save(&dir);
        assert_eq!(Today::load(&dir, "2026-10-09"), t);
        assert_eq!(t.words(), json!({ "key": "widget.todayValue", "vars": { "turns": "12", "time": "1:43", "approvals": "5" } }));
        assert_eq!(Today::load(&dir, "2026-10-10").turns, 0);
        assert!(t.roll("2026-10-10") && t.turns == 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn today_under_a_minute_in_seconds_and_antigravity_s_approvals_never_counted() {
        let words = |turns, agy_turns, worked_ms, approvals| Today { date: String::new(), turns, agy_turns, worked_ms, approvals, hours: [0; 24] }.words();
        let seconds = |n: &str| json!({ "key": "widget.todaySeconds", "vars": { "n": n } });
        assert_eq!(words(3, 0, 45_600, 1)["vars"]["time"], seconds("45"));
        assert_eq!(words(1, 0, 400, 0)["vars"]["time"], seconds("1"));
        assert_eq!((words(0, 0, 0, 0)["vars"]["time"].clone(), words(9, 0, 60_000, 0)["vars"]["time"].clone()), (json!("0:00"), json!("0:01")));
        // Antigravity's turns alone: no approvals to count, "—".
        assert_eq!(words(2, 2, 45_000, 0)["key"], "widget.todayValueAgy");
        assert_eq!(words(3, 2, 45_000, 0)["key"], "widget.todayValue");
        assert_eq!(words(0, 0, 0, 0)["key"], "widget.todayValue");
        let dir = std::env::temp_dir().join(format!("wakuwaku-today-agy-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let t = Today { date: "2026-10-14".into(), turns: 4, agy_turns: 3, worked_ms: 1, approvals: 0, hours: [0; 24] };
        t.save(&dir);
        assert_eq!(Today::load(&dir, "2026-10-14"), t);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // A detail as the spec has it: the sizes in their order, a series, rows,
    // a few words more; anything else dropped, long words cut with "…", so a
    // script cannot swell the island.
    // A plugin's buttons are kept, checked: those that will do, four at
    // most, off kept; a script's are dropped, its detail otherwise as it
    // was (and a detail of nothing but dropped buttons is none).
    #[test]
    fn buttons_are_a_plugins_alone() {
        let sent = json!({ "id": "media", "label": "Yellow", "detail": { "extra": "1:02 / 4:29", "actions": [{ "id": "prev", "icon": "prev", "off": true }, { "id": "toggle", "icon": "pause" }, { "id": "next", "icon": "next", "off": false }, { "id": "Bad", "icon": "next" }, { "id": "noicon" }, { "id": "x", "icon": "heart" }, { "id": "a", "icon": "play" }, { "id": "b", "icon": "play" }, { "id": "c", "icon": "play" }] } });
        let mut w = Widgets::default();
        assert!(w.put_from(&sent, 0, true).is_ok());
        let detail = w.view(0, &[], &[], &|_| None).as_array().unwrap().iter().find(|x| x["id"] == "media").unwrap()["detail"].clone();
        assert_eq!(detail["extra"], "1:02 / 4:29");
        assert_eq!(detail["actions"], json!([{ "id": "prev", "icon": "prev", "off": true }, { "id": "toggle", "icon": "pause" }, { "id": "next", "icon": "next" }, { "id": "a", "icon": "play" }]));
        let mut w = Widgets::default();
        assert!(w.put(&json!({ "id": "mine", "label": "x", "detail": { "actions": [{ "id": "play", "icon": "play" }] } }), 0).is_ok());
        let all = w.view(0, &[], &[], &|_| None);
        let mine = all.as_array().unwrap().iter().find(|x| x["id"] == "mine").unwrap();
        assert_eq!(mine["detail"], Value::Null, "a script's buttons dropped, nothing else of a detail: none");
        assert_eq!(actions_of(Some(&json!("play"))), Value::Null);
        assert_eq!(actions_of(Some(&json!([{ "id": "play", "icon": "play", "label": "Play" }]))), json!([{ "id": "play", "icon": "play" }]), "nothing but id, icon and off");
    }

    #[test]
    fn a_detail_is_checked_and_cut() {
        assert_eq!(detail_of(None), Value::Null);
        assert_eq!(detail_of(Some(&json!("  "))), Value::Null);
        assert_eq!(detail_of(Some(&json!(42))), Value::Null);
        assert_eq!(detail_of(Some(&json!({}))), Value::Null);
        assert_eq!(detail_of(Some(&json!(" 阴 · Dresden "))), json!({ "extra": "阴 · Dresden" }));
        let d = detail_of(Some(&json!({
            "sizes": ["l", "x", "s", "s"],
            "series": { "points": (0..150).map(f64::from).collect::<Vec<_>>(), "unit": "一二三四五六七八九十一二三四五六七", "hi": 100, "lo": "low" },
            "rows": [
                { "from": "Werner Haselmayr-Schneider", "subject": "x".repeat(70), "at": 1_791_890_400_250u64, "secret": "no" },
                { "code": "AAPL", "price": 230.4, "change": "+0.8%", "points": [1, "two", 3] },
                { "nothing": true },
                "not a row",
            ],
            "extra": "最高 16° 最低 8°",
            "colour": "#fff",
        })));
        assert_eq!(d["sizes"], json!(["s", "l"]));
        let points = d["series"]["points"].as_array().unwrap();
        // The last POINTS kept.
        assert_eq!((points.len(), points[0].as_f64(), points[119].as_f64()), (120, Some(30.0), Some(149.0)));
        assert_eq!((d["series"]["unit"].as_str().unwrap().chars().count(), d["series"]["hi"].as_f64(), d["series"].get("lo")), (16, Some(100.0), None));
        let rows = d["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 2, "{d}");
        assert_eq!(rows[0]["from"], "Werner Haselmay…");
        assert_eq!((rows[0]["subject"].as_str().unwrap().chars().count(), rows[0]["at"].as_u64(), rows[0].get("secret")), (60, Some(1_791_890_400_250), None));
        // A number for words is taken as words; the points numbers alone.
        assert_eq!((rows[1]["price"].as_str(), rows[1]["points"].clone()), (Some("230.4"), json!([1.0, 3.0])));
        assert_eq!(d["extra"], "最高 16° 最低 8°");
        assert!(d.get("colour").is_none());
        // Twelve rows at most.
        let many: Vec<_> = (0..20).map(|i| json!({ "name": format!("r{i}") })).collect();
        assert_eq!(detail_of(Some(&json!({ "rows": many })))["rows"].as_array().unwrap().len(), 12);
        // A series with no number in it is none.
        assert_eq!(detail_of(Some(&json!({ "series": { "points": ["a", null] } }))), Value::Null);
        // Sent with a widget, kept with it; another without, gone.
        let mut w = Widgets::default();
        w.put(&json!({ "id": "weather", "label": "Dresden · 阴", "value": "11°", "detail": { "sizes": ["s", "m", "l"], "extra": "8°~16°" } }), 0).unwrap();
        assert_eq!(scripted(w.view(0, &[], &[], &|_| None))[0]["detail"], json!({ "sizes": ["s", "m", "l"], "extra": "8°~16°" }));
        w.put(&json!({ "id": "weather", "label": "Dresden · 阴", "value": "11°" }), 1).unwrap();
        assert_eq!(scripted(w.view(1, &[], &[], &|_| None))[0]["detail"], Value::Null);
    }

    // A built-in widget's detail changes it as its words do; one set without
    // a detail keeps the one it had. A mail account's, the same.
    #[test]
    fn a_built_in_detail_is_kept_with_its_words() {
        let mut w = Widgets::default();
        assert!(w.set_built_in_with("today", json!("1"), json!({ "sizes": ["s"] }), 0));
        assert!(!w.set_built_in_with("today", json!("1"), json!({ "sizes": ["s"] }), 1));
        assert!(w.set_built_in_with("today", json!("1"), json!({ "sizes": ["s", "m"] }), 2));
        assert!(w.set_built_in("today", json!("2"), 3));
        let today = w.view(3, &[], &[], &|_| None).as_array().unwrap().iter().find(|x| x["id"] == "today").cloned().unwrap();
        assert_eq!(today["detail"], json!({ "sizes": ["s", "m"] }));
        let rows = json!({ "rows": [{ "from": "Anna", "subject": "周五见" }] });
        assert!(w.put_owned("inbox-1", "mail", "", json!("Anna：周五见"), json!("1"), rows.clone(), true, 0));
        assert!(!w.put_owned("inbox-1", "mail", "", json!("Anna：周五见"), json!("1"), rows, true, 1));
        assert!(w.put_owned("inbox-1", "mail", "", json!("Anna：周五见"), json!("1"), json!({ "rows": [] }), true, 2));
    }

    // Today's turns by the hour: counted in the hour begun, kept with the
    // day, gone with it; its detail the bars.
    #[test]
    fn today_counts_its_turns_by_the_hour() {
        let dir = std::env::temp_dir().join(format!("wakuwaku-today-hours-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut t = Today::load(&dir, "2026-10-16");
        t.begun(2);
        t.begun(1);
        let hour = local_hour();
        assert_eq!((t.turns, t.hours[hour], t.hours.iter().sum::<u64>()), (3, 3, 3));
        t.save(&dir);
        assert_eq!(Today::load(&dir, "2026-10-16").hours, t.hours);
        assert_eq!(Today::load(&dir, "2026-10-17").hours, [0; 24]);
        let d = t.detail();
        assert_eq!((d["sizes"].clone(), d["series"]["points"].as_array().unwrap().len()), (json!(["s", "m", "l"]), 24));
        assert_eq!(d["series"]["points"][hour], 3);
        assert!(t.roll("2026-10-17") && t.hours == [0; 24]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // The monitor widget's detail: a row for each thing read, by its kind,
    // none for what was not (its card says it has no data); the CPU's minute
    // as the series.
    #[test]
    fn the_monitors_detail_has_what_was_read() {
        let gb = 1_073_741_824u64;
        let seen = Seen {
            cpu: Some(17),
            cpu_minute: vec![12.0, 17.0],
            mhz: Some(3_456.0),
            cores: Some((16, 32)),
            memory: Some((44, 32 * gb, 18 * gb)),
            gpu: Some((23.4, "NVIDIA GeForce RTX 4080 Laptop GPU".into(), Some(52.4))),
            disk: Some((61.0, "Samsung SSD 990 PRO 2TB".into())),
            net: Some((7_400.0, 337_000.0)),
            down_minute: vec![7_400.0],
            up_ms: 273_600_123,
        };
        let d = monitor_detail(&seen);
        let row = |kind: &str| d["rows"].as_array().unwrap().iter().find(|r| r["kind"] == kind).cloned().unwrap_or(Value::Null);
        assert_eq!(row("cpu"), json!({ "kind": "cpu", "value": "17%", "text": "3.5 GHz", "name": "16", "unit": "32" }));
        assert_eq!(row("memory"), json!({ "kind": "memory", "value": "44%", "text": "14.0 / 32 GB" }));
        assert_eq!(row("gpu"), json!({ "kind": "gpu", "value": "23%", "name": "RTX 4080 Laptop GPU", "text": "52°" }));
        assert_eq!(row("disk")["value"], "61%");
        assert_eq!(row("net"), json!({ "kind": "net", "value": "7.4K", "text": "337K", "points": [7400.0] }));
        // Up for 3 days and 4 hours, to the minute.
        assert_eq!(row("uptime")["value"], "273600");
        assert_eq!(d["series"], json!({ "points": [12.0, 17.0], "unit": "%", "hi": 100, "lo": 0 }));
        // Nothing but the CPU read: no rows for the rest, no series yet.
        let alone = monitor_detail(&Seen { cpu: Some(5), ..Seen::default() });
        assert_eq!(alone, json!({ "sizes": ["s", "m", "l"], "rows": [{ "kind": "cpu", "value": "5%" }] }));
    }

    // Of the built-in source's turn, the card's hardware: the card with a
    // temperature (a discrete one) before a busier one without, the busiest
    // drive; and what to read for them, every card and drive.
    #[test]
    fn the_monitors_card_takes_the_discrete_card_and_the_busiest_drive() {
        use crate::hw::{Class, Hardware, Reading, Round};
        use crate::lhm::Kind;
        let hw = |id: &str, class, name: &str| Hardware { id: id.into(), class, name: name.into(), about: Value::Null, health: None };
        let reading = |id: &str, hardware_id: &str, kind, value| Reading { id: id.into(), hardware_id: hardware_id.into(), label: "hw.load", kind, value, of: None, warn: None, toggle: None, info: false };
        let mut cpu = hw("/cpu/0", Class::Cpu, "AMD Ryzen 9 9955HX");
        cpu.about = json!({ "key": "hw.cores", "vars": { "cores": 16, "threads": 32 } });
        let round = Round {
            hardware: vec![cpu, hw("/gpu-amd/0", Class::Gpu, "AMD Radeon(TM) 610M"), hw("/gpu-nvidia/0", Class::Gpu, "RTX 4080"), hw("/disk/0", Class::Disk, "WD"), hw("/disk/1", Class::Disk, "Samsung")],
            readings: vec![
                reading("/cpu/0/clock/0", "/cpu/0", Kind::Clock, Some(3_656.0)),
                reading("/gpu-amd/0/load/0", "/gpu-amd/0", Kind::Load, Some(40.0)),
                reading("/gpu-nvidia/0/load/0", "/gpu-nvidia/0", Kind::Load, Some(15.0)),
                reading("/gpu-nvidia/0/temperature/0", "/gpu-nvidia/0", Kind::Temperature, Some(40.4)),
                reading("/disk/0/load/0", "/disk/0", Kind::Load, Some(3.0)),
                reading("/disk/1/load/0", "/disk/1", Kind::Load, Some(61.0)),
            ],
        };
        let (mhz, cores, gpu, disk, also) = card_hardware(&round);
        assert_eq!((mhz, cores), (Some(3_656.0), Some((16, 32))));
        assert_eq!(gpu, Some((15.0, "RTX 4080".to_string(), Some(40.4))));
        assert_eq!(disk, Some((61.0, "Samsung".to_string())));
        assert_eq!(also, ["/cpu/0/clock/0", "/gpu-amd/0/load/0", "/gpu-amd/0/temperature/0", "/gpu-nvidia/0/load/0", "/gpu-nvidia/0/temperature/0", "/disk/0/load/0", "/disk/1/load/0"]);
        // Nothing read yet: none of them, the CPU's speed still asked for.
        let (_, _, gpu, disk, also) = card_hardware(&Round::default());
        assert_eq!((gpu, disk, also), (None, None, vec!["/cpu/0/clock/0".to_string()]));
    }
}
