// The plugins she runs for you: the weather, stocks, a pomodoro, a deadline,
// a stretch reminder, CI and dev servers, the calendar and what is playing,
// each running while its switch is on (placed is on: the picker writes
// plugins[id].on). The first seven were PowerShell
// scripts she started hidden (examples/widgets, since taken out of the
// repository); they are done here now, inside her, each widget as its
// script made it (the same words, numbers, colours, sizes and nudges, and
// the same settings: their parameters keep the scripts' names). No
// PowerShell, no process of their own: a resident one held 110 MB, starting
// one cost a second of CPU, the Store's PowerShell could vanish from under
// her (#28), there is none on a Mac, and hidden ones look like malware to an
// antivirus (#48).
//
// Settings: "plugins": { <id>: { "on": bool, <Param>: text | bool } }
//
// How they run. One thread ("plugins") keeps their turns. A timer's turn
// (pomodoro, countdown, stretch) is done on it, a few microseconds; a turn
// that asks the web (weather, stock, ci, devserver) on a thread of its own
// for that turn ("plugin-<id>"), so a slow site holds up nothing else, the
// pomodoro's second least of all. Between turns the thread waits on a
// condition variable for the next one due, or to be woken: a plugin turned
// on, off or changed (sync), nobody about any more (Shared::check_quiet),
// her back in sight (Shared::apply_visibility), a look on the web done. With
// nobody about (quiet) or her out of sight, what counts down or looks often
// does so less often (pomodoro every 30 s, not 10; devserver every 45, not
// 15), its widget still outlasting the wait; the ends of things (a pomodoro's
// stretch, a deadline, a reminder) come on time all the same.
//
// What each shows goes through Shared::put_plugin_widget, as a script's
// POST /widget did (and, unlike a script's, with buttons where it has
// them). What went wrong in its last turn (a city not found, the web down)
// is said on the Plugins page (view: said), as a script's last warning
// was.
//
// Buttons. A widget of theirs may carry actions (widgets.rs: detail.actions,
// up to four, each an id and an icon: play, pause, previous, next), drawn
// on its cards; a press on one comes back here (act, from the pages'
// widget_act), is queued for the plugin and delivered on its own thread
// (Job::act) just before a turn taken at once, so what it did shows
// straight away. A plugin that hears of a change on its own (what is
// playing changed: media.rs) asks for a turn at once the same way (poke).
mod calendar;
mod ci;
pub mod countdown;
mod devserver;
pub mod fmt;
mod media;
mod pomodoro;
mod stock;
mod stretch;
mod weather;
pub mod web;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Condvar};
use std::time::Duration;

use serde_json::{json, Value};

use crate::Shared;
use crate::lock::Lock;

// --- What a plugin is ------------------------------------------------------------------

// The time a turn is given: ms since 1970, and the local calendar's ms since
// 1970 (the wall's clock, as a deadline is typed: countdown).
#[derive(Clone, Copy, Debug)]
pub struct Now {
    pub ms: u64,
    pub local: i64,
}

// How long until its next turn: while someone is about, and while nobody is
// (quiet) or she is out of sight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wait {
    pub busy: Duration,
    pub idle: Duration,
}

impl Wait {
    pub fn every(d: Duration) -> Wait {
        Wait { busy: d, idle: d }
    }

    fn of(&self, slow: bool) -> Duration {
        if slow {
            self.idle
        } else {
            self.busy
        }
    }
}

// Where a turn's words go: its widgets, as POST /widget took them; and what
// went wrong, in a line.
pub trait Out {
    fn post(&mut self, widget: Value);
    fn say(&mut self, line: String);
}

// A plugin running: one turn now, and how long until the next; and a
// button of its pressed (act, one with none hears nothing), a turn
// following at once.
pub trait Job: Send {
    fn turn(&mut self, now: Now, out: &mut dyn Out) -> Wait;
    fn act(&mut self, _action: &str, _now: Now, _out: &mut dyn Out) {}
}

// A GET on the web (web::get; a test's recorded answers).
pub type Fetch = Box<dyn Fn(&str, &[(&str, &str)]) -> Result<web::Answer, web::Fail> + Send>;

fn web_fetch() -> Fetch {
    Box::new(|url: &str, headers: &[(&str, &str)]| web::get(url, headers, web::TIMEOUT))
}

// --- Which there are, and their settings -------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Text,
    Number,
    Flag,
    // One of a few (each a switch of the script's: -RedUp); none chosen,
    // the one for her language: zh for Chinese, other for any other.
    Choice { options: &'static [&'static str], zh: &'static str, other: &'static str },
}

pub struct Param {
    pub name: &'static str,
    pub kind: Kind,
    pub required: bool,
}

const fn p(name: &'static str, kind: Kind, required: bool) -> Param {
    Param { name, kind, required }
}

pub struct Plugin {
    pub id: &'static str,
    pub icon: &'static str,
    pub params: &'static [Param],
    // Its words follow her language: a change of it starts it again.
    by_lang: bool,
    // Its turns ask the web: each on a thread of its own.
    on_web: bool,
    make: fn(&Conf<'_>) -> Box<dyn Job>,
}

// Its settings as it starts.
pub struct Conf<'a> {
    plugin: &'static Plugin,
    v: &'a Value,
    lang: &'static str,
}

impl Conf<'_> {
    fn text(&self, name: &str) -> &str {
        text(self.v, name)
    }

    // A number typed, as the script's [double] took it; none, its default.
    fn number(&self, name: &str) -> Option<f64> {
        self.text(name).parse().ok().filter(|n: &f64| n.is_finite())
    }

    fn flag(&self, name: &str) -> bool {
        self.v[name] == true
    }

    fn choice(&self, name: &str) -> &'static str {
        self.plugin.params.iter().find(|p| p.name == name).and_then(|p| choice(p.kind, &self.v[name], self.lang)).unwrap_or("")
    }
}

// The scripts' own parameters (the old <id>.ps1's) and periods; the
// calendar's and the player's their own.
pub static PLUGINS: [Plugin; 9] = [
    Plugin { id: "weather", icon: "weather", params: &[p("City", Kind::Text, false)], by_lang: true, on_web: true, make: |c| Box::new(weather::Weather::new(c.text("City"), c.lang, web_fetch())) },
    Plugin {
        id: "stock",
        icon: "stock",
        // Its up and down colours (the one place every widget of it follows,
        // wcards.ts trendOf): red up as Chinese markets show them, or green
        // up; none chosen, by her language.
        params: &[p("Symbol", Kind::Text, false), p("Name", Kind::Text, false), p("Colors", Kind::Choice { options: &["RedUp", "GreenUp"], zh: "RedUp", other: "GreenUp" }, false)],
        by_lang: false,
        on_web: true,
        make: |c| Box::new(stock::Stock::new(c.text("Symbol"), c.text("Name"), c.choice("Colors") == "GreenUp", c.lang, web_fetch())),
    },
    Plugin { id: "pomodoro", icon: "timer", params: &[p("Work", Kind::Number, false), p("Break", Kind::Number, false)], by_lang: false, on_web: false, make: |c| Box::new(pomodoro::Pomodoro::new(c.number("Work"), c.number("Break"))) },
    Plugin { id: "countdown", icon: "flag", params: &[p("At", Kind::Text, true), p("Title", Kind::Text, false)], by_lang: false, on_web: false, make: |c| Box::new(countdown::Countdown::new(c.text("At"), c.text("Title"), c.lang)) },
    Plugin { id: "stretch", icon: "bell", params: &[p("Minutes", Kind::Number, false)], by_lang: false, on_web: false, make: |c| Box::new(stretch::Stretch::new(fmt::int_of(c.text("Minutes")))) },
    Plugin { id: "ci", icon: "check", params: &[p("Repo", Kind::Text, true), p("Prs", Kind::Flag, false)], by_lang: false, on_web: true, make: |c| Box::new(ci::Ci::new(c.text("Repo"), c.flag("Prs"), None, c.lang, web_fetch())) },
    Plugin {
        id: "devserver",
        icon: "server",
        params: &[p("Url", Kind::Text, false)],
        by_lang: false,
        on_web: true,
        make: |c| Box::new(devserver::Devserver::new(c.text("Url"), c.lang, Box::new(|u: &str| web::answers(u, devserver::ANSWER_IN)))),
    },
    Plugin {
        id: "calendar",
        icon: "calendar",
        // Its second line: the lunar date (农历) or the week of the year;
        // none chosen, by her language.
        params: &[p("Url", Kind::Text, false), p("Second", Kind::Choice { options: &["Lunar", "Week"], zh: "Lunar", other: "Week" }, false)],
        by_lang: true,
        on_web: true,
        make: |c| Box::new(calendar::Calendar::new(c.text("Url"), c.choice("Second") == "Lunar", c.lang, web_fetch())),
    },
    // Its turns ask Windows, not the web; on a thread of their own all the
    // same, so a player slow to answer holds up no timer of hers.
    Plugin { id: "media", icon: "music", params: &[], by_lang: true, on_web: true, make: |c| Box::new(media::Media::new(c.lang)) },
];

fn plugin(id: &str) -> Option<&'static Plugin> {
    PLUGINS.iter().find(|p| p.id == id)
}

// A built-in plugin's parameter value as its settings keep it: a flag true
// or false, text without control characters, a number as text (positive, up
// to 100000, or empty), one of a choice's options.
fn param_ok(p: &Param, v: &Value) -> bool {
    match p.kind {
        Kind::Flag => v.is_boolean(),
        Kind::Text => v.as_str().is_some_and(|t| t.chars().count() <= 300 && !t.chars().any(char::is_control)),
        Kind::Number => v.as_str().is_some_and(|t| t.trim().is_empty() || t.trim().parse::<f64>().is_ok_and(|n| n > 0.0 && n <= 100_000.0)),
        Kind::Choice { options, .. } => v.as_str().is_some_and(|o| options.contains(&o)),
    }
}

// A "plugins" setting: only the plugins here, each with "on" and its own
// parameters, text without control characters, numbers as text; and the
// third-party ones installed, each by its manifest (addons.rs is_conf_ok).
pub fn is_ok(v: &Value) -> bool {
    v.as_object().is_some_and(|all| all.iter().all(|(id, conf)| {
        if plugin(id).is_none() {
            return crate::addons::is_conf_ok(id, conf).unwrap_or(false);
        }
        let (Some(s), Some(conf)) = (plugin(id), conf.as_object()) else { return false };
        conf.iter().all(|(key, v)| match (key.as_str(), s.params.iter().find(|p| p.name == key)) {
            ("on", _) => v.is_boolean(),
            (_, Some(p)) => param_ok(p, v),
            _ => false,
        })
    }))
}

// A "plugins" setting cleaned, entry by entry (zhoupengjie/Wakuwaku's
// plugins review, finding 3): an id that is neither a plugin of hers nor one
// installed is dropped (stale, a plugin taken away), and within a kept entry
// an unknown key, or a value that will not do (a choice whose options
// changed in an update), is dropped — so one stale entry no longer refuses
// the whole patch, which carried a good change with it. The pages send the
// whole object, so this is what is kept.
pub fn clean(v: &Value) -> Value {
    let Some(all) = v.as_object() else { return Value::Object(serde_json::Map::new()) };
    let mut out = serde_json::Map::new();
    for (id, conf) in all {
        let Some(conf) = conf.as_object() else { continue };
        let kept = match plugin(id) {
            Some(s) => {
                let keep = |key: &str, v: &Value| match (key, s.params.iter().find(|p| p.name == key)) {
                    ("on", _) => v.is_boolean(),
                    (_, Some(p)) => param_ok(p, v),
                    _ => false,
                };
                Some(conf.iter().filter(|(k, v)| keep(k, v)).map(|(k, v)| (k.clone(), v.clone())).collect())
            }
            None => crate::addons::clean_conf(id, conf),
        };
        if let Some(kept) = kept {
            out.insert(id.clone(), Value::Object(kept));
        }
    }
    Value::Object(out)
}

fn text<'a>(conf: &'a Value, name: &str) -> &'a str {
    conf[name].as_str().map(str::trim).unwrap_or("")
}

// The first parameter it needs and has not been given.
fn missing(s: &Plugin, conf: &Value) -> Option<&'static str> {
    s.params.iter().find(|p| p.required && text(conf, p.name).is_empty()).map(|p| p.name)
}

// One of a few as chosen, else the one for her language ("zh", "en").
fn choice(kind: Kind, value: &Value, lang: &str) -> Option<&'static str> {
    let Kind::Choice { options, zh, other } = kind else { return None };
    Some(options.iter().copied().find(|o| value.as_str() == Some(*o)).unwrap_or(if lang == "zh" { zh } else { other }))
}

// What it runs with: its parameters as it takes them (a choice none made, as
// her language has it), and her language where its words follow it. Another
// one: it starts again (what it showed goes; it looks at once).
fn key_of(p: &Plugin, conf: &Value, lang: &'static str) -> Value {
    let mut key = serde_json::Map::new();
    for param in p.params {
        let v = match param.kind {
            Kind::Flag => json!(conf[param.name] == true),
            Kind::Choice { .. } => json!(choice(param.kind, &conf[param.name], lang)),
            Kind::Text | Kind::Number => json!(text(conf, param.name)),
        };
        key.insert(param.name.into(), v);
    }
    if p.by_lang {
        key.insert("lang".into(), json!(lang));
    }
    Value::Object(key)
}

// Which plugin a widget comes from, by the ids they give them: one of
// hers, or a third-party one installed (addons.rs owner_of).
pub fn owner_of(widget: &str) -> Option<String> {
    built_in_owner(widget).map(str::to_string).or_else(|| crate::addons::owner_of(widget))
}

fn built_in_owner(widget: &str) -> Option<&'static str> {
    match widget {
        "weather" | "pomodoro" | "countdown" | "stretch" | "devserver" | "calendar" | "media" => plugin(widget).map(|s| s.id),
        w if w.starts_with("stock-") => Some("stock"),
        w if w.starts_with("ci-") || w.starts_with("prs-") => Some("ci"),
        _ => None,
    }
}

// --- Running them ------------------------------------------------------------------------

struct Slot {
    // What it was started with (key_of).
    key: Value,
    // Which start of it: a turn of one before (stopped, or started again
    // with other settings) still under way posts nothing when it ends.
    gen: u64,
    // None while its turn runs on a thread of its own.
    job: Option<Box<dyn Job>>,
    // When its last turn began (a timer's) or ended (a look on the web's), ms.
    last: Option<u64>,
    wait: Wait,
    // What went wrong in its last turn.
    said: Option<String>,
    // Its buttons pressed since its last turn (act), in order.
    pending: Vec<String>,
}

#[derive(Default)]
pub struct Runner {
    slots: HashMap<&'static str, Slot>,
    gens: u64,
    // The thread that keeps their turns, once one was on.
    thread: bool,
    // The scripts she ran them by before, looked for (clean_up).
    cleaned: bool,
}

impl Runner {
    // A button of a widget's pressed: queued for its plugin, due at once,
    // eight at most waiting. False when there is nothing to queue it for
    // (act says which).
    fn press(&mut self, widget: &str, action: &str) -> bool {
        let Some(owner) = built_in_owner(widget).filter(|_| is_action(action)) else { return false };
        let Some(slot) = self.slots.get_mut(owner) else { return false };
        if slot.pending.len() >= 8 {
            return false;
        }
        slot.pending.push(action.to_string());
        slot.last = None;
        true
    }
}

// Taken while a widget of theirs is posted and while sync stops one, so a
// turn ending after its plugin stopped never shows what it found. Taken
// before any of Shared's locks, and never while one is held: an outer lock
// (lock.rs), on the plugins' and the widgets' threads.
static POSTING: Lock<()> = Lock::ranked("plugins::POSTING", crate::lock::OUTER, ());
static WAKE: (Lock<bool>, Condvar) = (Lock::new("plugins::WAKE", false), Condvar::new());
// The plugins that asked for a turn at once (poke), taken up by the thread
// that keeps their turns.
static POKED: Lock<Vec<&'static str>> = Lock::new("plugins::POKED", Vec::new());

// The turns looked at again now: a plugin changed, someone back, her back in sight.
pub fn wake() {
    let (woken, cv) = &WAKE;
    *woken.lock().unwrap_or_else(|e| e.into_inner()) = true;
    cv.notify_all();
}

// A plugin asks for a turn at once (what it shows changed under it: the
// song changed), from any thread, with no lock of Shared's taken.
pub fn poke(id: &'static str) {
    let mut poked = POKED.lock().unwrap_or_else(|e| e.into_inner());
    if !poked.contains(&id) {
        poked.push(id);
    }
    drop(poked);
    wake();
}

// An action as a button names it: a-z 0-9 -, 16 at most, a letter first
// (widgets.rs actions_of takes the same).
fn is_action(action: &str) -> bool {
    !action.is_empty() && action.len() <= 16 && action.starts_with(|c: char| c.is_ascii_lowercase()) && action.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

// A button of a widget's pressed on a page (commands/mod.rs widget_act): queued
// for the plugin the widget belongs to, which takes a turn at once (its
// job is on another thread, or in a turn: never called from here). False
// for a widget of no plugin of hers, one not running, or an action that
// is none.
pub fn act(sh: &Shared, widget: &str, action: &str) -> bool {
    let queued = {
        let mut r = sh.plugins.lock().unwrap();
        r.press(widget, action)
    };
    if queued {
        wake();
    }
    queued
}

fn wait_for(d: Option<Duration>) {
    let (woken, cv) = &WAKE;
    let guard = woken.lock().unwrap_or_else(|e| e.into_inner());
    let mut guard = match d {
        Some(d) => guard.wait_timeout_while(cv, d, |w| !*w).map(|(g, _)| g).unwrap_or_else(|e| e.into_inner().0),
        None => guard.wait_while(cv, |w| !*w).unwrap_or_else(|e| e.into_inner()),
    };
    *guard = false;
}

// When a turn is due: at once for its first, else its wait after its last.
fn due_at(last: Option<u64>, wait: Wait, slow: bool) -> u64 {
    last.map_or(0, |t| t.saturating_add(wait.of(slow).as_millis() as u64))
}

// A line of what went wrong, as the Plugins page shows it: its first, 160
// characters at most.
fn tidy(line: &str) -> String {
    line.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").chars().take(160).collect()
}

// A turn's words, to her widgets; a turn of a plugin since stopped or
// started again shows nothing.
struct Posting<'a> {
    sh: &'a Shared,
    id: &'static str,
    gen: u64,
    said: Option<String>,
}

impl Out for Posting<'_> {
    fn post(&mut self, widget: Value) {
        let _posting = POSTING.lock().unwrap_or_else(|e| e.into_inner());
        let current = self.sh.plugins.lock().unwrap().slots.get(self.id).is_some_and(|s| s.gen == self.gen);
        if !current {
            return;
        }
        if let Err(why) = self.sh.put_plugin_widget(&widget) {
            self.said = Some(tidy(why));
        }
    }

    fn say(&mut self, line: String) {
        self.said = Some(tidy(&line));
    }
}

fn one_turn(sh: &Shared, id: &'static str, gen: u64, mut job: Box<dyn Job>, on_web: bool, pending: Vec<String>) {
    let now = Now { ms: crate::now_ms(), local: countdown::local_ms() };
    let mut out = Posting { sh, id, gen, said: None };
    // Its buttons pressed first, then the turn that shows what they did.
    for action in &pending {
        job.act(action, now, &mut out);
    }
    let wait = job.turn(now, &mut out);
    let said = out.said;
    let mut r = sh.plugins.lock().unwrap();
    if let Some(slot) = r.slots.get_mut(id).filter(|s| s.gen == gen) {
        slot.job = Some(job);
        // A timer counts from when it looked; a look on the web, from when
        // its answer came (the scripts' turns waited from their end).
        slot.last = Some(if on_web { crate::now_ms() } else { now.ms });
        slot.wait = wait;
        slot.said = said;
    }
}

// The thread that keeps their turns.
fn keep_turns(sh: Arc<Shared>) {
    crate::power::thread_eco();
    loop {
        let slow = sh.is_quiet() || !sh.is_visible();
        let now = crate::now_ms();
        let mut due = Vec::new();
        let mut next: Option<u64> = None;
        let poked = std::mem::take(&mut *POKED.lock().unwrap_or_else(|e| e.into_inner()));
        {
            let mut r = sh.plugins.lock().unwrap();
            for (id, slot) in r.slots.iter_mut() {
                // Asked for a turn at once: due now (one in a turn already
                // is due as soon as it is back).
                if poked.contains(id) {
                    slot.last = None;
                }
                if slot.job.is_none() {
                    continue;
                }
                let at = due_at(slot.last, slot.wait, slow);
                if at <= now {
                    if let Some(job) = slot.job.take() {
                        due.push((*id, slot.gen, job, std::mem::take(&mut slot.pending)));
                    }
                } else {
                    next = Some(next.map_or(at, |n| n.min(at)));
                }
            }
        }
        if due.is_empty() {
            wait_for(next.map(|at| Duration::from_millis(at.saturating_sub(crate::now_ms()))));
            continue;
        }
        for (id, gen, job, pending) in due {
            if !plugin(id).is_some_and(|p| p.on_web) {
                one_turn(&sh, id, gen, job, false, pending);
                continue;
            }
            let on_its_own = sh.clone();
            let started = std::thread::Builder::new().name(format!("plugin-{id}")).spawn(move || {
                crate::power::thread_eco();
                one_turn(&on_its_own, id, gen, job, true, pending);
                wake();
            });
            // No thread to be had: it goes (its turn with it), and sync
            // starts it again.
            if let Err(e) = started {
                sh.log(&format!("plugins: {id}: no thread for its turn: {e}"));
                sh.plugins.lock().unwrap().slots.retain(|k, s| !(*k == id && s.gen == gen));
            }
        }
    }
}

// The scripts she ran them by before, written out to <her folder>/plugins
// (<id>.ps1, and <id>.log, what each printed), taken away once as she
// starts: nothing runs them now, and they would only grow old there. waku.ps1
// beside them stays, as anything else of anyone's: she writes it out no more,
// but a terminal set up to run it still can (it only posts to /widget).
fn clean_up(folder: &Path) -> Vec<String> {
    let mut gone = Vec::new();
    for p in &PLUGINS {
        for ext in ["ps1", "log"] {
            let name = format!("{}.{ext}", p.id);
            if std::fs::remove_file(folder.join(&name)).is_ok() {
                gone.push(name);
            }
        }
    }
    gone
}

// Each running as its switch says: started, stopped, started again with new
// settings (what it showed goes with the old one). True when any changed.
pub fn sync(sh: &Shared) -> bool {
    let conf = sh.config().plugins.clone();
    let lang = sh.lang();
    let (mut changed, mut stale, mut start, mut woken) = (false, Vec::new(), false, false);
    let clean;
    let posting = POSTING.lock().unwrap_or_else(|e| e.into_inner());
    {
        let mut r = sh.plugins.lock().unwrap();
        clean = !std::mem::replace(&mut r.cleaned, true);
        for p in &PLUGINS {
            let c = &conf[p.id];
            let want = (c["on"] == true && missing(p, c).is_none()).then(|| key_of(p, c, lang));
            match (r.slots.get(p.id), &want) {
                (Some(slot), Some(key)) if slot.key == *key => continue,
                (None, None) => continue,
                _ => {}
            }
            if r.slots.remove(p.id).is_some() {
                stale.push(p.id);
            }
            if let Some(key) = want {
                r.gens += 1;
                let gen = r.gens;
                let job = (p.make)(&Conf { plugin: p, v: c, lang });
                r.slots.insert(p.id, Slot { key, gen, job: Some(job), last: None, wait: Wait::every(Duration::ZERO), said: None, pending: Vec::new() });
                woken = true;
            }
            changed = true;
        }
        if !r.slots.is_empty() && !r.thread {
            r.thread = true;
            start = true;
        }
    }
    // Stopped, or started again with other settings: what it showed goes.
    if !stale.is_empty() && sh.widgets.lock().unwrap().remove_scripted(|id| built_in_owner(id).is_some_and(|o| stale.contains(&o))) {
        changed = true;
    }
    drop(posting);
    // The third-party ones as their switches say too: the island's page
    // told which to run (addons.rs).
    changed |= crate::addons::sync(sh);
    if clean {
        let gone = clean_up(&sh.dir.join("plugins"));
        if !gone.is_empty() {
            sh.log(&format!("plugins: the scripts she ran them by before taken away: {}", gone.join(", ")));
        }
    }
    if start {
        let keeper = crate::shared(&sh.app);
        crate::spawn_named("plugins", move || keep_turns(keeper));
    }
    if woken {
        wake();
    }
    changed
}

// Each plugin for the settings: its switch, its parameters, and how it is.
//   state  off | needs (a parameter) | running
//   said   what went wrong in its last turn, if anything
pub fn view(sh: &Shared) -> Value {
    let conf = sh.config().plugins.clone();
    let lang = sh.lang();
    let r = sh.plugins.lock().unwrap();
    Value::Array(
        PLUGINS
            .iter()
            .map(|s| {
                let c = &conf[s.id];
                let on = c["on"] == true;
                let state = match () {
                    _ if !on => "off",
                    _ if missing(s, c).is_some() => "needs",
                    _ => "running",
                };
                let said = r.slots.get(s.id).and_then(|slot| slot.said.clone());
                let params: Vec<Value> = s
                    .params
                    .iter()
                    .map(|p| {
                        let kind = match p.kind {
                            Kind::Text => "text",
                            Kind::Number => "number",
                            Kind::Flag => "flag",
                            Kind::Choice { .. } => "choice",
                        };
                        // A choice: its options, and the one in effect (none
                        // made: her language's).
                        match p.kind {
                            Kind::Choice { options, .. } => json!({ "name": p.name, "kind": kind, "required": p.required, "options": options, "value": choice(p.kind, &c[p.name], lang), "chosen": c[p.name].is_string() }),
                            _ => json!({ "name": p.name, "kind": kind, "required": p.required, "value": c[p.name] }),
                        }
                    })
                    .collect();
                json!({ "id": s.id, "icon": s.icon, "on": on, "state": state, "missing": missing(s, c), "said": said, "params": params })
            })
            .collect(),
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    // --- For the plugins' own tests ---

    // What a script posted and warned, run with the web and the clock played
    // back to it (test/fixtures/plugins/ps/<name>.json, recorded while the
    // scripts were still in the repository, by a record.ps1 gone with them):
    // each post's seconds since it started, and its body.
    pub fn golden(name: &str) -> (Vec<(f64, Value)>, Vec<String>) {
        let path = format!("{}/../test/fixtures/plugins/ps/{name}.json", env!("CARGO_MANIFEST_DIR"));
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        // ConvertTo-Json writes a list of one as the one.
        let list = |v: &Value| match v {
            Value::Array(a) => a.clone(),
            Value::Null => Vec::new(),
            one => vec![one.clone()],
        };
        let posts = list(&v["posts"]).into_iter().map(|p| (p["t"].as_f64().unwrap(), p["body"].clone())).collect();
        let warnings = list(&v["warnings"]).into_iter().map(|w| w.as_str().unwrap().to_string()).collect();
        (posts, warnings)
    }

    // Two widgets alike: the same fields, numbers alike as numbers (25 and
    // 25.0), a list of one as PowerShell wrote it either way.
    pub fn same(a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
            (Value::Object(x), Value::Object(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w))),
            (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(v, w)| same(v, w)),
            (Value::Array(x), y) | (y, Value::Array(x)) if x.len() == 1 => same(&x[0], y),
            _ => a == b,
        }
    }

    #[derive(Default)]
    pub struct Record {
        pub posts: Vec<Value>,
        pub said: Option<String>,
    }

    impl Out for Record {
        fn post(&mut self, widget: Value) {
            self.posts.push(widget);
        }
        fn say(&mut self, line: String) {
            self.said = Some(tidy(&line));
        }
    }

    // One turn at a moment (ms since 1970): what it posted, said, and waits.
    pub fn record(job: &mut dyn Job, ms: u64) -> (Vec<Value>, Option<String>, Wait) {
        let mut out = Record::default();
        let wait = job.turn(Now { ms, local: ms as i64 }, &mut out);
        (out.posts, out.said, wait)
    }

    // The same at a moment of the local calendar (countdown).
    pub fn record_at(job: &mut dyn Job, local: i64) -> (Vec<Value>, Option<String>, Wait) {
        let mut out = Record::default();
        let wait = job.turn(Now { ms: local.max(0) as u64, local }, &mut out);
        (out.posts, out.said, wait)
    }

    pub type Route = Box<dyn Fn() -> (u16, String) + Send + Sync>;

    // The web played back: the first address each starts with answers so;
    // none, no answer. With the addresses asked, in turn.
    pub fn served_with(routes: Vec<(String, Route)>) -> (Fetch, Arc<Mutex<Vec<String>>>) {
        let asked = Arc::new(Mutex::new(Vec::new()));
        let heard = asked.clone();
        let fetch: Fetch = Box::new(move |url: &str, _: &[(&str, &str)]| {
            heard.lock().unwrap().push(url.to_string());
            match routes.iter().find(|(at, _)| url.starts_with(at.as_str())) {
                Some((_, answer)) => {
                    let (status, body) = answer();
                    Ok(web::Answer { status, body })
                }
                None => Err(web::Fail::Network("offline".into())),
            }
        });
        (fetch, asked)
    }

    // The same, each answer a recorded one (test/fixtures/plugins/<file>).
    pub fn served(routes: &[(&str, u16, &str)]) -> (Fetch, Arc<Mutex<Vec<String>>>) {
        served_with(
            routes
                .iter()
                .map(|(at, status, file)| {
                    let body = std::fs::read_to_string(format!("{}/../test/fixtures/plugins/{file}", env!("CARGO_MANIFEST_DIR"))).unwrap();
                    let status = *status;
                    (at.to_string(), Box::new(move || (status, body.clone())) as Route)
                })
                .collect(),
        )
    }

    // --- Their settings ---

    #[test]
    fn settings_hold_only_the_plugins_and_their_parameters() {
        assert!(is_ok(&json!({ "weather": { "on": true, "City": "上海" }, "ci": { "Repo": "a/b", "Prs": true } })));
        assert!(is_ok(&json!({ "pomodoro": { "Work": "25", "Break": "" } })));
        assert!(!is_ok(&json!({ "nope": { "on": true } })));
        assert!(!is_ok(&json!({ "mail-imap": { "on": true } })));
        assert!(!is_ok(&json!({ "weather": { "Town": "x" } })));
        assert!(!is_ok(&json!({ "weather": { "City": "a\nb" } })));
        assert!(!is_ok(&json!({ "pomodoro": { "Work": "-1" } })));
        assert!(!is_ok(&json!({ "ci": { "Prs": "yes" } })));
        assert!(is_ok(&json!({ "stock": { "on": true, "Colors": "RedUp" } })));
        assert!(!is_ok(&json!({ "stock": { "Colors": "Blue" } })));
        assert!(!is_ok(&json!({ "stock": { "Colors": true } })));
        let ci = plugin("ci").unwrap();
        assert_eq!(missing(ci, &json!({ "Repo": " " })), Some("Repo"));
        assert_eq!(missing(plugin("weather").unwrap(), &json!({})), None);
    }

    // One stale entry no longer refuses the whole patch (the plugins review,
    // finding 3): a good change to a built-in is kept while an id that is no
    // plugin, a value that will not do, and an unknown key are dropped.
    #[test]
    fn a_stale_entry_does_not_lose_the_good_change() {
        let patch = json!({
            "weather": { "on": true, "City": "上海" },
            "gone-addon": { "on": true, "City": "x" },
            "ci": { "Repo": "a/b", "Prs": "yes", "Bogus": 1 },
        });
        let cleaned = clean(&patch);
        assert_eq!(cleaned["weather"], json!({ "on": true, "City": "上海" }), "the good change is kept");
        assert_eq!(cleaned["ci"], json!({ "Repo": "a/b" }), "the bad value and unknown key dropped, the good kept");
        assert!(cleaned.get("gone-addon").is_none(), "an id that is no plugin here dropped");
        // What survives is itself valid.
        assert!(is_ok(&cleaned));
        // A fully good patch is kept whole; a non-object becomes nothing.
        assert_eq!(clean(&json!({ "weather": { "on": false } })), json!({ "weather": { "on": false } }));
        assert_eq!(clean(&json!("nonsense")), json!({}));
    }

    // Settings made for the scripts keep working: every parameter keeps the
    // name it had when she ran them (the scripts' own, but for the stock's
    // Colors, whose options were two of its script's switches). The
    // calendar's and the player's came after the scripts.
    #[test]
    fn the_parameters_are_the_scripts_own() {
        let scripts: [(&str, &[&str]); 9] = [
            ("weather", &["City"]),
            ("stock", &["Symbol", "Name", "Colors"]),
            ("pomodoro", &["Work", "Break"]),
            ("countdown", &["At", "Title"]),
            ("stretch", &["Minutes"]),
            ("ci", &["Repo", "Prs"]),
            ("devserver", &["Url"]),
            ("calendar", &["Url", "Second"]),
            ("media", &[]),
        ];
        for (p, (id, names)) in PLUGINS.iter().zip(scripts) {
            assert_eq!(p.id, id);
            assert_eq!(p.params.iter().map(|q| q.name).collect::<Vec<_>>(), names, "{id}");
        }
        let Kind::Choice { options, .. } = plugin("stock").unwrap().params[2].kind else { panic!() };
        assert_eq!(options, ["RedUp", "GreenUp"]);
    }

    // The stock plugin's up and down colours (the one place every widget of
    // it follows): none chosen, by her language, red up in Chinese as
    // Chinese markets show them, green up otherwise; chosen, as chosen,
    // whatever her language. Only its two.
    #[test]
    fn a_choice_none_made_is_her_languages() {
        let kind = Kind::Choice { options: &["RedUp", "GreenUp"], zh: "RedUp", other: "GreenUp" };
        assert_eq!(choice(kind, &Value::Null, "zh"), Some("RedUp"));
        assert_eq!(choice(kind, &Value::Null, "en"), Some("GreenUp"));
        assert_eq!(choice(kind, &json!("GreenUp"), "zh"), Some("GreenUp"));
        assert_eq!(choice(kind, &json!("Pink"), "zh"), Some("RedUp"));
        assert_eq!(choice(Kind::Flag, &json!(true), "zh"), None);
        let stock = plugin("stock").unwrap();
        let c = json!({ "Symbol": "AAPL" });
        assert_eq!(Conf { plugin: stock, v: &c, lang: "zh" }.choice("Colors"), "RedUp");
        assert_eq!(Conf { plugin: stock, v: &c, lang: "en" }.choice("Colors"), "GreenUp");
    }

    // What starts a plugin again: another parameter, or her language where
    // its words or colours follow it; never her language for a timer, whose
    // count would start over.
    #[test]
    fn what_starts_one_again() {
        let (weather, stock, pomodoro) = (plugin("weather").unwrap(), plugin("stock").unwrap(), plugin("pomodoro").unwrap());
        assert_ne!(key_of(weather, &json!({ "City": "上海" }), "zh"), key_of(weather, &json!({ "City": "上海" }), "en"));
        assert_eq!(key_of(weather, &json!({ "City": " 上海 ", "on": true }), "zh"), key_of(weather, &json!({ "City": "上海" }), "zh"));
        assert_ne!(key_of(stock, &json!({}), "zh"), key_of(stock, &json!({}), "en"));
        assert_eq!(key_of(stock, &json!({ "Colors": "GreenUp" }), "zh"), key_of(stock, &json!({ "Colors": "GreenUp" }), "en"));
        assert_eq!(key_of(pomodoro, &json!({ "Work": "50" }), "zh"), key_of(pomodoro, &json!({ "Work": "50" }), "en"));
        assert_ne!(key_of(pomodoro, &json!({ "Work": "50" }), "zh"), key_of(pomodoro, &json!({ "Work": "45" }), "zh"));
        assert_ne!(key_of(plugin("ci").unwrap(), &json!({ "Repo": "a/b" }), "zh"), key_of(plugin("ci").unwrap(), &json!({ "Repo": "a/b", "Prs": true }), "zh"));
    }

    #[test]
    fn widgets_are_told_apart_by_plugin() {
        assert_eq!(owner_of("weather").as_deref(), Some("weather"));
        assert_eq!(owner_of("stock-600519-ss").as_deref(), Some("stock"));
        assert_eq!(owner_of("prs-a-b").as_deref(), Some("ci"));
        assert_eq!(owner_of("calendar").as_deref(), Some("calendar"));
        assert_eq!(owner_of("media").as_deref(), Some("media"));
        assert_eq!(owner_of("mail-thunderbird"), None);
        assert_eq!(owner_of("today"), None);
    }

    // A button pressed is queued for its plugin, which is due at once;
    // taken up with its turn, in order, eight at most; a press on a widget
    // of no plugin of hers, one not running, or an action that is none, is
    // nothing. A poke makes a plugin due at once too, once however often.
    #[test]
    fn a_button_pressed_queues_for_its_plugin_and_a_poke_makes_it_due() {
        let mut r = Runner::default();
        let slot = || Slot { key: json!({}), gen: 1, job: None, last: Some(5_000), wait: Wait::every(Duration::from_secs(60)), said: None, pending: Vec::new() };
        r.slots.insert("media", slot());
        assert!(r.press("media", "toggle"));
        assert!(r.press("media", "next"));
        assert!(!r.press("media", "Next"));
        assert!(!r.press("media", "-x"));
        assert!(!r.press("media", ""));
        assert!(!r.press("media", &"a".repeat(17)));
        assert!(!r.press("today", "next"), "no plugin of hers");
        assert!(!r.press("weather", "next"), "not running");
        assert_eq!(r.slots["media"].pending, ["toggle", "next"]);
        assert_eq!(r.slots["media"].last, None, "due at once");
        for _ in 0..6 {
            assert!(r.press("media", "next"));
        }
        assert!(!r.press("media", "next"), "eight at most");
        assert!(is_action("play-pause") && !is_action("1a"));
        POKED.lock().unwrap().clear();
        poke("media");
        poke("media");
        let poked = std::mem::take(&mut *POKED.lock().unwrap());
        assert_eq!(poked, ["media"]);
    }

    // --- Their turns ---

    // The first at once; then its wait from the last, the longer one with
    // nobody about.
    #[test]
    fn turns_come_when_due() {
        let wait = Wait { busy: Duration::from_secs(10), idle: Duration::from_secs(30) };
        assert_eq!(due_at(None, wait, false), 0);
        assert_eq!(due_at(Some(1_000), wait, false), 11_000);
        assert_eq!(due_at(Some(1_000), wait, true), 31_000);
        assert_eq!(tidy("\n  找不到这个城市：x \nmore"), "找不到这个城市：x");
        assert_eq!(tidy(&"字".repeat(200)).chars().count(), 160);
    }

    #[test]
    fn the_old_scripts_are_taken_away_and_nothing_else() {
        let dir = std::env::temp_dir().join(format!("wakuwaku-plugins-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for f in ["weather.ps1", "weather.log", "ci.ps1", "waku.ps1", "mine.ps1"] {
            std::fs::write(dir.join(f), "x").unwrap();
        }
        assert_eq!(clean_up(&dir), ["weather.ps1", "weather.log", "ci.ps1"]);
        let mut left: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left, ["mine.ps1", "waku.ps1"]);
        assert!(clean_up(&dir).is_empty());
        assert!(clean_up(&dir.join("none")).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Each plugin made from its settings, as sync makes it (none of them
    // asks anything of the web or the disk until its first turn).
    #[test]
    fn each_is_made_from_its_settings() {
        for p in &PLUGINS {
            let c = json!({ "on": true, "At": "2026-12-31 18:00", "Repo": "a/b", "Work": "0.5", "Minutes": "2.5" });
            let _job = (p.make)(&Conf { plugin: p, v: &c, lang: "zh" });
        }
        let c = json!({ "Minutes": "2.5" });
        let mut job = (plugin("stretch").unwrap().make)(&Conf { plugin: plugin("stretch").unwrap(), v: &c, lang: "zh" });
        assert_eq!(record(job.as_mut(), 0).0[0]["value"], "2 分钟后");
    }
}
