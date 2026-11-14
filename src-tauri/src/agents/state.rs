// What the pet shows of the agents' sessions. What each session is doing is
// agent-core's registry (registry.rs: its status, its turn, the calls
// running, its to-do list, the files its turn changed, how it ended, when it
// was given up or forgotten), kept from the AgentEvents she hears (server.rs
// take; POST /state). Here is only how she shows them (Look):
//   mood     idle | working | waiting | done | review (a turn that changed
//            files) | error; an ending she saw, or whose held time ran
//            out, at rest from then (rested)
//   detail   what the line says beside the mood: the tool, or what it waits
//            on (events.rs detail_of), till the next event
//   step     what the latest call running does, or thinking since the last
//            ended (events.rs step_for)
//   name     its title, else the person's words for the work (task)
//   chain    the processes it runs in, for going to its window (jump.rs)
//   hidden   out of sight, quiet too long (is_shown)
//
// Each session keeps its own mood; the pet shows the one that most wants you.
// Instead of callbacks and timers, calls return what happened (Outcome) and
// tick() settles the endings that only stay a while.
use std::collections::HashMap;

use serde_json::{json, Map, Value};

use agent_core::event::{AgentEvent, Kind};
use agent_core::registry::{Session, SessionRegistry, Status, FORGET_MS};

use crate::agents::events;

// The processes a session runs in, its own first and those above it, each
// a pid with when it started (a number Windows has handed to another
// process since is not taken for it): what going to its window goes by
// (jump.rs).
pub type Chain = Vec<(u32, u64)>;

// A session at rest, out of sight after this many minutes with nothing new
// from it, unless set otherwise (idleHideMin; 0 never).
pub const IDLE_HIDE_MIN: u64 = 30;
// How long a name goes into a notification.
const WHOSE_MAX: usize = 30;

fn priority(mood: &str) -> u8 {
    match mood {
        "waiting" => 5,
        "error" => 4,
        "review" => 3,
        "done" => 2,
        "working" => 1,
        _ => 0,
    }
}

fn is_ending(mood: &str) -> bool {
    matches!(mood, "done" | "review" | "error")
}

// Whether a session is in sight (Pet::hide_quiet). At work or waiting on
// you, always (the registry gives up on one stuck); an ending, until it is
// seen (FORGET_MS at most: check_stale). At rest, until nothing has come
// from it for `hide_ms` (0: always), counted from its last event, or from
// when its ending was seen or its held time ran out, whichever came later.
// One out of sight is still kept, and its next event brings it back as it
// was.
pub fn is_shown(mood: &str, event_at: u64, seen_at: Option<u64>, now: u64, hide_ms: u64) -> bool {
    match mood {
        "idle" => {
            let quiet_since = seen_at.map_or(event_at, |seen| seen.max(event_at));
            hide_ms == 0 || now.saturating_sub(quiet_since) < hide_ms
        }
        _ => true,
    }
}

// How long done / review / error stay: until seen, or this many seconds.
#[derive(Clone, Copy)]
pub enum Hold {
    Seen,
    Secs(u64),
}

#[derive(Default)]
pub struct Outcome {
    // The mood shown may have changed: redraw.
    pub changed: bool,
    // { react, say } to play once.
    pub react: Option<Value>,
    // Moods just arrived that want a chime (and a notification): (mood,
    // whose: the session's name, or its project).
    pub alerts: Vec<(String, String)>,
    // For today's count: turns begun, and how long the ones that ended took
    // (stopped by the person too, and one that never said it ended, up to
    // its last event, when the next begins).
    pub turns: u64,
    pub worked_ms: u64,
}

// How she shows a session, beside what it is doing (the registry's).
struct Look {
    // An ending she saw, or whose held time ran out: at rest since then,
    // whatever the registry still says of how it ended.
    rested: Option<u64>,
    detail: Value,
    // Until when an ending stays (Hold::Secs).
    settle_at: Option<u64>,
    // Its name, given; the person's words for the work.
    title: String,
    task: String,
    // When the last call ended, or the turn began: thinking since then.
    thought_at: Option<u64>,
    chain: Chain,
    // When its ending was seen or its held time ran out (how long it has
    // been quiet), and out of sight as last looked at (is_shown).
    seen_at: Option<u64>,
    hidden: bool,
    recent_steps: Vec<Value>,
}

impl Look {
    fn new() -> Look {
        Look { rested: None, detail: json!(""), settle_at: None, title: String::new(), task: String::new(), thought_at: None, chain: Vec::new(), seen_at: None, hidden: false, recent_steps: Vec::new() }
    }

    // What it is called: its title, or the person's words.
    fn name(&self) -> &str {
        if self.title.is_empty() { &self.task } else { &self.title }
    }

    // At rest from `now`: an ending seen, or held its time.
    fn rest(&mut self, now: u64) {
        self.rested = Some(now);
        self.seen_at = Some(now);
        self.detail = json!("");
        self.settle_at = None;
    }
}

// A session's mood, as she shows it.
fn mood_of(s: &Session, look: &Look) -> &'static str {
    if look.rested.is_some() {
        return "idle";
    }
    match s.status {
        Status::Idle => "idle",
        Status::Working => "working",
        Status::Waiting { .. } => "waiting",
        Status::Done { edited: false } => "done",
        Status::Done { edited: true } => "review",
        Status::Failed { .. } => "error",
    }
}

// When it last changed: its rest, if she put it to rest.
fn at_of(s: &Session, look: &Look) -> u64 {
    look.rested.unwrap_or(s.at)
}

fn whose(s: &Session, look: &Look) -> String {
    clip(if look.name().is_empty() { s.project.as_deref().unwrap_or("") } else { look.name() }, WHOSE_MAX)
}

fn clip(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

// A text she shows: a string, or { key, vars } whose vars are strings,
// numbers, or (once) a text of the same kind.
fn text(value: Option<&Value>, max: usize) -> Value {
    text_in(value, max, true)
}

fn text_in(value: Option<&Value>, max: usize, may_nest: bool) -> Value {
    match value {
        Some(Value::String(s)) => Value::String(clip(s, max)),
        Some(Value::Object(o)) if o.get("key").is_some_and(Value::is_string) => {
            let mut vars = Map::new();
            if let Some(Value::Object(given)) = o.get("vars") {
                for (k, v) in given {
                    match v {
                        Value::String(s) => vars.insert(k.clone(), Value::String(clip(s, max))),
                        Value::Number(n) => vars.insert(k.clone(), Value::String(clip(&n.to_string(), max))),
                        Value::Object(_) if may_nest => vars.insert(k.clone(), text_in(Some(v), max, false)),
                        _ => None,
                    };
                }
            }
            json!({ "key": clip(o["key"].as_str().unwrap_or(""), 100), "vars": vars })
        }
        _ => Value::String(String::new()),
    }
}

// How far down its to-do list a session is, and what it is on.
fn todo_view(s: &Session) -> Value {
    if s.todos.is_empty() {
        return Value::Null;
    }
    let done = s.todos.iter().filter(|t| t.status == "completed").count();
    let active = s.todos.iter().find(|t| t.status == "in_progress").map_or("", |t| if t.active.is_empty() { &t.text } else { &t.active });
    json!({ "done": done, "total": s.todos.len(), "active": events::clip(active, 80) })
}

// A file's path relative to its session's folder (cwd), keeping it whole when outside.
fn relative_file(file: &str, cwd: Option<&str>) -> String {
    let norm_file = file.replace('\\', "/");
    let Some(cwd) = cwd.filter(|c| !c.is_empty()) else {
        return norm_file;
    };
    let file_path = std::path::Path::new(file);
    let cwd_path = std::path::Path::new(cwd);
    if let Ok(rel) = file_path.strip_prefix(cwd_path) {
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if !rel_str.is_empty() {
            return rel_str;
        }
    }
    let norm_cwd = cwd.replace('\\', "/");
    let prefix = if norm_cwd.ends_with('/') { norm_cwd } else { format!("{norm_cwd}/") };
    if let Some(rest) = norm_file.strip_prefix(&prefix) {
        if !rest.is_empty() {
            return rest.to_string();
        }
    }
    if let Some(head) = norm_file.get(..prefix.len()) {
        if head.eq_ignore_ascii_case(&prefix) {
            if let Some(rest) = norm_file.get(prefix.len()..) {
                if !rest.is_empty() {
                    return rest.to_string();
                }
            }
        }
    }
    norm_file
}

// A session as the pages draw it.
fn view(s: &Session, look: &Look) -> Value {
    let mood = mood_of(s, look);
    let (step, step_since) = match s.running.last() {
        _ if mood != "working" => (Value::Null, None),
        Some(r) => (text(Some(&events::step_for(s.agent, &r.call)), 80), Some(r.since)),
        None => (json!({ "key": "step.thinking", "vars": {} }), look.thought_at),
    };
    let reply = if is_ending(mood) { s.reply.as_deref().and_then(events::reply_of).unwrap_or_default() } else { String::new() };
    let error = match (&s.status, mood) {
        (Status::Failed { error }, "error") => text(Some(&events::error_of(error)), 80),
        _ => Value::Null,
    };
    let file_list: Vec<String> = s.files.iter().take(15).map(|f| relative_file(f, s.cwd.as_deref())).collect();
    json!({
        "id": s.id,
        "project": s.project.as_deref().unwrap_or(""),
        "agent": s.agent.tag().unwrap_or(""),
        // A window to go to when clicked.
        "jump": !look.chain.is_empty(),
        "mood": mood,
        "detail": look.detail,
        "at": at_of(s, look),
        "since": s.since,
        "took": s.took,
        "name": look.name(),
        "step": step,
        "stepSince": step_since,
        "todo": todo_view(s),
        "files": s.files.len(),
        "fileList": file_list,
        "recentSteps": look.recent_steps.clone(),
        "reply": reply,
        "error": error,
    })
}

// What the pet shows with no session.
fn no_session(now: u64) -> Value {
    json!({
        "id": "", "project": "", "agent": "", "jump": false, "mood": "idle", "detail": "", "at": now, "since": null, "took": null,
        "name": "", "step": null, "stepSince": null, "todo": null, "files": 0, "fileList": [], "recentSteps": [], "reply": "", "error": null,
    })
}

#[derive(Default)]
pub struct Pet {
    sessions: SessionRegistry,
    looks: HashMap<String, Look>,
}

impl Pet {
    // An event heard at `now`, with where its session runs when that was
    // looked up (server.rs chain_of_caller); None when it is nothing to a
    // session she shows (a subagent's with no turn of its session's under
    // way).
    pub fn apply(&mut self, ev: &AgentEvent, chain: Option<Chain>, now: u64, hold: Hold) -> Option<Outcome> {
        let id = ev.session.as_str();
        let was = self.mood_by_id(id);
        let before = self.sessions.get(id).map(|s| (s.since, s.at));
        let did = self.sessions.apply(ev, now);
        if did.left_out {
            return None;
        }
        let react = events::reaction_of(ev).map(|r| json!({ "react": r["react"], "say": text(r.get("say"), 80) }));
        let mut out = Outcome { react, ..Outcome::default() };
        if matches!(ev.kind, Kind::TaskDone { .. }) {
            return Some(out);
        }
        out.changed = true;
        if ev.kind == Kind::SessionEnd {
            self.looks.remove(id);
            return Some(out);
        }
        let look = self.looks.entry(id.to_string()).or_insert_with(Look::new);
        if let Some(chain) = chain.filter(|c| !c.is_empty()) {
            look.chain = chain;
        }
        let Some(s) = self.sessions.get(id) else { return Some(out) };
        // Late, from a turn already over: only an edit counts, making its done a review.
        if did.late {
            out.changed = did.changed && look.rested.is_none();
            return Some(out);
        }
        // Something new from it: back in sight at once, if it was out.
        look.rested = None;
        look.hidden = false;
        if let Some(title) = ev.title.as_deref().map(|t| events::clip(t, 80)).filter(|t| !t.is_empty()) {
            look.title = title;
        }
        if let Kind::TurnStart { prompt: Some(prompt) } = &ev.kind {
            if let Some(task) = events::task_of(prompt) {
                look.task = task;
            }
        }
        if did.began {
            out.turns += 1;
            // The turn before never said it ended (stopped without a word, or
            // Antigravity's still waiting on a task of its own): it worked
            // until its last event.
            if let Some((Some(since), at)) = before {
                out.worked_ms += at.saturating_sub(since);
            }
            look.thought_at = Some(now);
            look.recent_steps.clear();
        }
        let step_record = match &ev.kind {
            Kind::ToolDone { call } => Some((call, true)),
            Kind::ToolFailed { call, .. } => Some((call, false)),
            _ => None,
        };
        if let Some((call, ok)) = step_record {
            let step = text(Some(&events::step_for(s.agent, call)), 80);
            if step.get("key").and_then(Value::as_str).is_some_and(|k| k != "step.thinking") {
                if look.recent_steps.len() >= 5 {
                    look.recent_steps.remove(0);
                }
                look.recent_steps.push(json!({ "step": step, "ok": ok }));
            }
        }
        if matches!(ev.kind, Kind::ToolDone { .. } | Kind::ToolFailed { .. }) && s.running.is_empty() {
            look.thought_at = Some(now);
        }
        look.detail = text(Some(&events::detail_of(ev)), 80);
        let mood = mood_of(s, look);
        if mood != was && (mood == "waiting" || is_ending(mood)) {
            out.alerts.push((mood.to_string(), whose(s, look)));
        }
        look.settle_at = match hold {
            Hold::Secs(secs) if is_ending(mood) => Some(now + secs * 1000),
            _ => None,
        };
        // Its end: done, failed, or stopped by the person, having worked till then.
        if matches!(ev.kind, Kind::Done | Kind::Failed { .. } | Kind::Stopped) {
            if let Some((Some(since), _)) = before {
                out.worked_ms += now.saturating_sub(since);
            }
        }
        Some(out)
    }

    fn mood_by_id(&self, id: &str) -> &'static str {
        match (self.sessions.get(id), self.looks.get(id)) {
            (Some(s), Some(look)) => mood_of(s, look),
            _ => "idle",
        }
    }

    // You looked: every ending goes back to rest, quiet from now on. True
    // when anything did.
    pub fn seen(&mut self, now: u64) -> bool {
        let mut changed = false;
        for s in self.sessions.sessions() {
            if let Some(look) = self.looks.get_mut(&s.id).filter(|look| is_ending(mood_of(s, look))) {
                look.rest(now);
                changed = true;
            }
        }
        changed
    }

    // Endings held for a number of seconds go back to rest when it is up:
    // the island folds them away then, so they count as seen.
    pub fn tick(&mut self, now: u64) -> bool {
        let mut changed = false;
        for look in self.looks.values_mut().filter(|look| look.settle_at.is_some_and(|t| t <= now)) {
            look.rest(now);
            changed = true;
        }
        changed
    }

    // Sessions gone quiet for `hide_ms` (the idleHideMin setting) out of
    // sight, and those it lets back (a longer setting): left out of what the
    // pages are given, but kept. True when any changed.
    pub fn hide_quiet(&mut self, now: u64, hide_ms: u64) -> bool {
        let mut changed = false;
        for s in self.sessions.sessions() {
            let Some(look) = self.looks.get_mut(&s.id) else { continue };
            let hidden = !is_shown(mood_of(s, look), s.event_at, look.seen_at, now, hide_ms);
            if look.hidden != hidden {
                look.hidden = hidden;
                changed = true;
            }
        }
        changed
    }

    // Work that never said it ended given up, endings nobody saw at rest,
    // and sessions quiet too long forgotten (the registry's rules; an ending
    // she put to rest, from then). True when one in sight changed.
    pub fn check_stale(&mut self, now: u64) -> bool {
        let rested_long: Vec<String> = self.looks.iter().filter(|(_, look)| look.rested.is_some_and(|at| now.saturating_sub(at) > FORGET_MS)).map(|(id, _)| id.clone()).collect();
        for id in rested_long {
            self.sessions.forget(&id);
            self.looks.remove(&id);
        }
        let mut changed = false;
        for id in self.sessions.check_stale(now) {
            if self.sessions.get(&id).is_none() {
                self.looks.remove(&id);
            } else if let Some(look) = self.looks.get_mut(&id).filter(|look| look.rested.is_none()) {
                look.detail = json!("");
                look.settle_at = None;
                changed = true;
            }
        }
        changed
    }

    // The sessions in sight (hide_quiet).
    fn shown(&self) -> impl Iterator<Item = (&Session, &Look)> {
        self.sessions.sessions().filter_map(|s| self.looks.get(&s.id).filter(|look| !look.hidden).map(|look| (s, look)))
    }

    // What the pet shows: the session that most wants you, and the rest in
    // sight.
    pub fn get(&self, now: u64) -> Value {
        let top = self.shown().fold(None::<(&Session, &Look)>, |best, (s, look)| match best {
            Some((b, bl)) if priority(mood_of(s, look)) < priority(mood_of(b, bl)) => best,
            Some((b, bl)) if priority(mood_of(s, look)) == priority(mood_of(b, bl)) && at_of(s, look) < at_of(b, bl) => best,
            _ => Some((s, look)),
        });
        let Some((top, top_look)) = top else {
            let mut empty = no_session(now);
            empty["others"] = json!(0);
            empty["sessions"] = json!(0);
            return empty;
        };
        let mut now = view(top, top_look);
        now["others"] = json!(self.shown().filter(|(s, look)| s.id != top.id && mood_of(s, look) != "idle").count());
        now["sessions"] = json!(self.shown().count());
        now
    }

    // The sessions in sight, the one that most wants you first.
    fn by_want(&self, busy: bool) -> Vec<(&Session, &Look)> {
        let mut all: Vec<(&Session, &Look)> = self.shown().filter(|(s, look)| if busy { mood_of(s, look) != "idle" } else { !s.id.is_empty() || mood_of(s, look) != "idle" }).collect();
        all.sort_by(|(a, al), (b, bl)| priority(mood_of(b, bl)).cmp(&priority(mood_of(a, al))).then(at_of(b, bl).cmp(&at_of(a, al))));
        all
    }

    // Every session in sight, the one that most wants you first: the island's
    // open lines, the settings' list, the marks on the taskbar's buttons.
    pub fn list(&self) -> Value {
        Value::Array(self.by_want(false).into_iter().map(|(s, look)| view(s, look)).collect())
    }

    // The mood of the next session that wants something, after the one shown:
    // for the island's second bubble.
    pub fn second(&self) -> Option<String> {
        self.by_want(true).get(1).map(|(s, look)| mood_of(s, look).to_string())
    }

    // Whether a session wants you: waiting on you, or an ending not yet seen.
    // Widgets keep to themselves meanwhile.
    pub fn wants_you(&self) -> bool {
        self.sessions.sessions().any(|s| self.looks.get(&s.id).is_some_and(|look| matches!(mood_of(s, look), "waiting" | "done" | "review" | "error")))
    }

    // What the sessions are called (projects, titles, tasks): kept out of a
    // crash report (crash.rs).
    pub fn names(&self) -> Vec<String> {
        self.sessions
            .sessions()
            .flat_map(|s| {
                let look = self.looks.get(&s.id);
                [s.project.clone().unwrap_or_default(), look.map(|l| l.title.clone()).unwrap_or_default(), look.map(|l| l.task.clone()).unwrap_or_default()]
            })
            .filter(|n| !n.trim().is_empty())
            .collect()
    }

    // Whether a session already knows where it runs.
    pub fn has_chain(&self, id: &str) -> bool {
        self.looks.get(id).is_some_and(|look| !look.chain.is_empty())
    }

    // Where to go for a session: its chain, and the words its window's
    // title may hold (its name, its project).
    pub fn jump_target(&self, id: &str) -> Option<(Chain, Vec<String>)> {
        let look = self.looks.get(id).filter(|look| !look.chain.is_empty())?;
        let project = self.sessions.get(id).and_then(|s| s.project.clone()).unwrap_or_default();
        Some((look.chain.clone(), vec![look.title.clone(), look.task.clone(), project]))
    }

    pub fn mood(&self, now: u64) -> String {
        self.get(now)["mood"].as_str().unwrap_or("idle").to_string()
    }
}

#[cfg(test)]
mod tests {
    use agent_core::event::{Agent, Call, Failure, Todo, TodoItem, Wait};
    use agent_core::registry::STALE_MS;

    use super::*;

    fn ev(session: &str, kind: Kind) -> AgentEvent {
        AgentEvent { agent: Agent::Claude, session: session.into(), within: false, cwd: None, turn: None, title: None, reply: None, kind }
    }

    // In a project's folder.
    fn ev_in(session: &str, project: &str, kind: Kind) -> AgentEvent {
        AgentEvent { cwd: Some(format!("Z:\\nowhere\\{project}")), ..ev(session, kind) }
    }

    fn call(id: &str, tool: &str) -> Call {
        Call::new(id, tool, json!({}))
    }

    fn start() -> Kind {
        Kind::TurnStart { prompt: None }
    }

    fn apply(pet: &mut Pet, ev: AgentEvent, at: u64) -> Option<Outcome> {
        pet.apply(&ev, None, at, Hold::Seen)
    }

    #[test]
    fn a_turn_that_edited_ends_in_review() {
        let mut pet = Pet::default();
        apply(&mut pet, ev("a", start()), 1);
        apply(&mut pet, ev("a", Kind::ToolDone { call: call("t1", "Edit") }), 2);
        let out = apply(&mut pet, ev("a", Kind::Done), 5).unwrap();
        assert_eq!(pet.get(6)["mood"], "review");
        assert_eq!(pet.get(6)["took"], 4);
        assert_eq!(out.alerts, vec![("review".to_string(), String::new())]);
    }

    #[test]
    fn the_session_that_most_wants_you_shows() {
        let mut pet = Pet::default();
        apply(&mut pet, ev_in("a", "one", Kind::Waiting { wait: Wait::question() }), 1);
        apply(&mut pet, ev_in("b", "two", Kind::Working), 2);
        let now = pet.get(3);
        assert_eq!(now["mood"], "waiting");
        assert_eq!(now["project"], "one");
        assert_eq!(now["detail"], json!({ "key": "detail.asksQuestion", "vars": {} }));
        assert_eq!(now["others"], 1);
        assert_eq!(now["sessions"], 2);
    }

    #[test]
    fn endings_stay_until_seen_or_held_time() {
        let mut pet = Pet::default();
        apply(&mut pet, ev("a", Kind::Done), 1);
        assert!(!pet.tick(1_000_000));
        assert!(pet.seen(2));
        assert_eq!(pet.get(3)["mood"], "idle");

        pet.apply(&ev("a", Kind::Failed { error: Failure::Kind { kind: "rate_limit".into() } }), None, 10, Hold::Secs(8));
        assert_eq!(pet.get(11)["error"], json!({ "key": "error.rate_limit", "vars": {} }));
        assert!(!pet.tick(8_009));
        assert!(pet.tick(8_010));
        assert_eq!((pet.get(8_011)["mood"].as_str(), pet.get(8_011)["at"].as_u64(), pet.get(8_011)["error"].is_null()), (Some("idle"), Some(8_010), true));
    }

    #[test]
    fn a_task_done_is_a_reaction_alone() {
        let mut pet = Pet::default();
        let out = apply(&mut pet, ev("a", Kind::TaskDone { subject: Some("x".repeat(100)) }), 1).unwrap();
        assert!(!out.changed);
        assert_eq!(out.react.unwrap(), json!({ "react": "jump", "say": { "key": "say.taskDone", "vars": { "task": "x".repeat(80) } } }));
        assert_eq!(pet.get(2)["sessions"], 0);
        // A session's start: a wave, and its line.
        let out = apply(&mut pet, ev("a", Kind::SessionStart), 3).unwrap();
        assert_eq!((out.changed, out.react.unwrap()["react"].as_str()), (true, Some("wave")));
    }

    #[test]
    fn stale_work_gives_up_and_quiet_sessions_are_forgotten() {
        let mut pet = Pet::default();
        apply(&mut pet, ev("a", Kind::Working), 0);
        apply(&mut pet, ev("b", Kind::Rest), 0);
        assert!(pet.check_stale(STALE_MS + 1));
        assert_eq!(pet.get(STALE_MS + 2)["mood"], "idle");
        pet.check_stale(FORGET_MS + 1);
        assert_eq!(pet.get(FORGET_MS + 2)["sessions"], 1);
        apply(&mut pet, ev("a", Kind::SessionEnd), 1);
        assert_eq!(pet.get(FORGET_MS + 3)["sessions"], 0);
    }

    // An ending she saw is forgotten FORGET_MS after she saw it, as one at
    // rest is; agent-core's registry alone keeps it as ended longer.
    #[test]
    fn a_seen_ending_is_forgotten_from_when_it_was_seen() {
        let mut pet = Pet::default();
        apply(&mut pet, ev("a", Kind::Done), 0);
        pet.seen(1_000);
        assert!(!pet.check_stale(FORGET_MS + 1_000));
        assert_eq!(pet.get(FORGET_MS + 1_000)["sessions"], 1);
        assert!(!pet.check_stale(FORGET_MS + 1_001));
        assert_eq!(pet.get(FORGET_MS + 1_001)["sessions"], 0);
        assert!(pet.names().is_empty() && pet.looks.is_empty());
    }

    #[test]
    fn the_step_is_the_latest_tool_running_or_thinking() {
        let mut pet = Pet::default();
        let edit = json!({ "key": "step.edit", "vars": { "file": "a.rs" } });
        let agent = json!({ "key": "step.agent", "vars": { "what": "Explore" } });
        apply(&mut pet, ev("a", start()), 10);
        assert_eq!(pet.get(11)["step"]["key"], "step.thinking");
        assert_eq!(pet.get(11)["stepSince"], 10);
        apply(&mut pet, ev("a", Kind::ToolStart { call: Call::new("1", "Agent", json!({ "description": "Explore" })) }), 20);
        apply(&mut pet, ev("a", Kind::ToolStart { call: Call::new("2", "Edit", json!({ "file_path": "D:/p/a.rs" })) }), 30);
        assert_eq!((pet.get(31)["step"].clone(), pet.get(31)["stepSince"].clone(), pet.get(31)["detail"].clone()), (edit, json!(30), json!("Edit")));
        // The subagent's tool ends: back to the agent, not to thinking.
        apply(&mut pet, ev("a", Kind::ToolDone { call: Call { file: Some("D:/p/a.rs".into()), ..Call::new("2", "Edit", json!({ "file_path": "D:/p/a.rs" })) } }), 40);
        assert_eq!(pet.get(41)["step"], agent);
        apply(&mut pet, ev("a", Kind::ToolFailed { call: call("1", "Agent"), quiet: false }), 50);
        assert_eq!((pet.get(51)["step"]["key"].clone(), pet.get(51)["stepSince"].clone()), (json!("step.thinking"), json!(50)));
        assert_eq!(pet.get(51)["files"], 1);
        // Waiting or done, there is no step.
        apply(&mut pet, AgentEvent { reply: Some("## 改好了\n\n改了 **a.rs**。".into()), ..ev("a", Kind::Done) }, 60);
        let now = pet.get(61);
        assert_eq!((now["mood"].as_str(), now["step"].is_null(), now["reply"].as_str(), now["files"].as_u64()), (Some("review"), true, Some("改好了：改了 a.rs。"), Some(1)));
        // The next turn starts clean.
        apply(&mut pet, ev("a", start()), 70);
        assert_eq!((pet.get(71)["files"].as_u64(), pet.get(71)["reply"].as_str()), (Some(0), Some("")));
    }

    #[test]
    fn a_subagent_s_steps_go_only_into_a_turn_under_way() {
        let mut pet = Pet::default();
        let dsh = |kind| AgentEvent { agent: Agent::Dsh, ..ev("a", kind) };
        let edit = AgentEvent { within: true, ..dsh(Kind::ToolDone { call: Call { file: Some("C:/w/notes.md".into()), ..call("c1", "Write") } }) };
        // No such session: none made.
        assert!(pet.apply(&edit, None, 1, Hold::Seen).is_none());
        assert_eq!(pet.get(2)["sessions"], 0);
        // At rest, or its turn ended (an idle review): left as it is.
        apply(&mut pet, dsh(Kind::Rest), 3);
        assert!(pet.apply(&edit, None, 4, Hold::Seen).is_none());
        apply(&mut pet, dsh(Kind::TurnStart { prompt: Some("整理一下笔记".into()) }), 5);
        apply(&mut pet, dsh(Kind::Done), 6);
        assert!(pet.apply(&edit, None, 7, Hold::Seen).is_none());
        assert_eq!((pet.get(8)["mood"].as_str(), pet.get(8)["files"].as_u64()), (Some("done"), Some(0)));
        // Within its turn: its edit is the turn's, and the turn ends in review.
        apply(&mut pet, dsh(start()), 10);
        assert!(pet.apply(&edit, None, 11, Hold::Seen).unwrap().changed);
        assert_eq!(pet.get(12)["files"], 1);
        apply(&mut pet, dsh(Kind::Done), 13);
        assert_eq!(pet.get(14)["mood"], "review");
        // A subagent's own line, should one have been made: gone.
        apply(&mut pet, AgentEvent { session: "9d1f".into(), ..dsh(Kind::Rest) }, 15);
        apply(&mut pet, AgentEvent { session: "9d1f".into(), ..dsh(Kind::SessionEnd) }, 16);
        assert_eq!(pet.get(17)["sessions"], 1);
    }

    #[test]
    fn a_session_is_named_by_its_title_or_else_its_task() {
        let mut pet = Pet::default();
        let out = apply(&mut pet, ev_in("a", "pet", Kind::Waiting { wait: Wait::question() }), 1).unwrap();
        assert_eq!(out.alerts, vec![("waiting".to_string(), "pet".to_string())]);
        apply(&mut pet, ev("a", Kind::TurnStart { prompt: Some("修复岛的设置\n细节……".into()) }), 2);
        assert_eq!(pet.get(3)["name"], "修复岛的设置");
        // A few words that only carry on name nothing.
        apply(&mut pet, ev("a", Kind::TurnStart { prompt: Some("继续".into()) }), 3);
        assert_eq!(pet.get(4)["name"], "修复岛的设置");
        apply(&mut pet, AgentEvent { title: Some("岛的设置".into()), ..ev("a", Kind::Done) }, 4);
        assert_eq!(pet.get(5)["name"], "岛的设置");
        // A new task does not take over a title.
        apply(&mut pet, ev("a", Kind::TurnStart { prompt: Some("下一件事情".into()) }), 6);
        assert_eq!(pet.list()[0]["name"], "岛的设置");
    }

    #[test]
    fn to_do_lists_count_what_is_done() {
        let item = |text: &str, active: &str, status: &str| TodoItem { text: text.into(), active: active.into(), status: status.into() };
        let todo = |pet: &mut Pet, todo: Todo| {
            apply(pet, ev("a", Kind::ToolStart { call: Call { todo: Some(todo), ..call("", "TodoWrite") } }), 1);
            pet.get(2)["todo"].clone()
        };
        let mut pet = Pet::default();
        let list = Todo::Set { items: vec![item("A", "Doing A", "completed"), item("B", "Doing B", "in_progress"), item("C", "", "pending")] };
        assert_eq!(todo(&mut pet, list), json!({ "done": 1, "total": 3, "active": "Doing B" }));

        let mut pet = Pet::default();
        let update = |id: &str, status: &str| Todo::Update { id: id.into(), status: Some(status.into()), text: None, active: None };
        todo(&mut pet, Todo::Add { item: item("A", "", "") });
        todo(&mut pet, Todo::Add { item: item("B", "Doing B", "") });
        todo(&mut pet, update("1", "completed"));
        assert_eq!(todo(&mut pet, update("2", "in_progress")), json!({ "done": 1, "total": 2, "active": "Doing B" }));
        assert_eq!(todo(&mut pet, update("2", "deleted")), json!({ "done": 1, "total": 1, "active": "" }));
        // All done, and a new turn: the list is gone.
        apply(&mut pet, ev("a", start()), 3);
        assert!(pet.get(4)["todo"].is_null());
    }

    #[test]
    fn texts_may_hold_one_text_inside() {
        let nested = json!({ "key": "detail.approve", "vars": { "what": { "key": "step.command", "vars": { "what": "git push", "deeper": { "key": "x" } } } } });
        assert_eq!(text(Some(&nested), 80), json!({ "key": "detail.approve", "vars": { "what": { "key": "step.command", "vars": { "what": "git push" } } } }));
    }

    // A Codex event: session c, turn t.
    fn codex(turn: &str, kind: Kind) -> AgentEvent {
        AgentEvent { agent: Agent::Codex, turn: Some(turn.into()), ..ev("c", kind) }
    }

    fn patch() -> Kind {
        Kind::ToolDone { call: call("", "apply_patch") }
    }

    #[test]
    fn a_codex_turn_with_a_patch_ends_in_review() {
        let mut pet = Pet::default();
        apply(&mut pet, codex("t1", start()), 1);
        apply(&mut pet, codex("t1", patch()), 2);
        apply(&mut pet, codex("t1", Kind::Done), 3);
        assert_eq!(pet.get(4)["mood"], "review");
        assert_eq!(pet.get(4)["agent"], "codex");
        assert_eq!(pet.list()[0]["agent"], "codex");
    }

    #[test]
    fn late_codex_events_change_nothing_but_a_late_edit() {
        let mut pet = Pet::default();
        // The turn's own start comes after one of its steps: it still counts the edit.
        apply(&mut pet, codex("t1", patch()), 1);
        apply(&mut pet, codex("t1", start()), 2);
        apply(&mut pet, codex("t1", Kind::Done), 3);
        assert_eq!(pet.get(4)["mood"], "review");

        // A step after its turn ended: she stays done, not back at work.
        apply(&mut pet, codex("t2", start()), 10);
        apply(&mut pet, codex("t2", Kind::Done), 11);
        let out = apply(&mut pet, codex("t2", Kind::ToolStart { call: Call::new("", "Bash", json!({ "command": "ls" })) }), 12).unwrap();
        assert!(!out.changed);
        assert_eq!(pet.get(13)["mood"], "done");
        // A late edit makes that done a review.
        assert!(apply(&mut pet, codex("t2", patch()), 14).unwrap().changed);
        assert_eq!(pet.get(15)["mood"], "review");
        // Seen, it stays at rest.
        pet.seen(16);
        assert!(!apply(&mut pet, codex("t2", patch()), 17).unwrap().changed);
        assert_eq!(pet.get(18)["mood"], "idle");

        // Interrupted: back to rest, and its stragglers too.
        apply(&mut pet, codex("t3", start()), 20);
        apply(&mut pet, codex("t3", Kind::Stopped), 21);
        apply(&mut pet, codex("t3", Kind::ToolStart { call: call("", "Bash") }), 22);
        assert_eq!(pet.get(23)["mood"], "idle");
        // The next turn works as ever.
        apply(&mut pet, codex("t4", start()), 30);
        assert_eq!(pet.get(31)["mood"], "working");
    }

    #[test]
    fn a_codex_turn_heard_first_by_a_step_starts_anew() {
        let mut pet = Pet::default();
        apply(&mut pet, codex("t1", start()), 1_000);
        apply(&mut pet, codex("t1", Kind::ToolDone { call: Call { file: Some("a.rs".into()), ..call("", "apply_patch") } }), 2_000);
        apply(&mut pet, codex("t1", Kind::Done), 3_000);
        // The next turn's first step before its start: the edit and file before are gone, the turn counted once.
        let out = apply(&mut pet, codex("t2", Kind::ToolStart { call: call("", "Bash") }), 10_000).unwrap();
        assert_eq!(out.turns, 1);
        let out = apply(&mut pet, codex("t2", start()), 10_100).unwrap();
        assert_eq!(out.turns, 0);
        assert_eq!(pet.get(10_200)["files"], 0);
        apply(&mut pet, codex("t2", Kind::Done), 12_000);
        assert_eq!(pet.get(12_100)["mood"], "done");
    }

    #[test]
    fn a_session_keeps_where_it_runs() {
        let mut pet = Pet::default();
        pet.apply(&ev_in("a", "pet", Kind::Working), Some(vec![(10, 5), (8, 3)]), 1, Hold::Seen);
        assert!(pet.has_chain("a"));
        assert_eq!(pet.get(2)["jump"], true);
        // An event without one keeps it.
        apply(&mut pet, AgentEvent { title: Some("岛".into()), ..ev("a", Kind::Done) }, 3);
        assert_eq!(pet.jump_target("a"), Some((vec![(10, 5), (8, 3)], vec!["岛".into(), String::new(), "pet".into()])));
        apply(&mut pet, ev("b", Kind::Working), 4);
        assert!(!pet.has_chain("b") && pet.jump_target("b").is_none());
        assert_eq!(pet.list().as_array().unwrap().iter().find(|s| s["id"] == "b").unwrap()["jump"], false);
    }

    #[test]
    fn a_turn_counts_for_today_and_an_ending_wants_you() {
        let mut pet = Pet::default();
        let out = apply(&mut pet, ev("a", start()), 1_000).unwrap();
        assert_eq!((out.turns, out.worked_ms), (1, 0));
        assert!(!pet.wants_you());
        let out = apply(&mut pet, ev("a", Kind::Done), 61_000).unwrap();
        assert_eq!((out.turns, out.worked_ms), (0, 60_000));
        assert!(pet.wants_you());
        pet.seen(62_000);
        assert!(!pet.wants_you());
    }

    #[test]
    fn a_stopped_turn_and_one_that_never_ended_count_too() {
        let mut pet = Pet::default();
        // Stopped by the person (Codex's Interrupt): it worked till then.
        apply(&mut pet, codex("t1", start()), 1_000);
        let out = apply(&mut pet, codex("t1", Kind::Stopped), 21_000).unwrap();
        assert_eq!(out.worked_ms, 20_000);
        // A turn that never said it ended, then the next: up to its last event.
        apply(&mut pet, ev("a", start()), 30_000);
        apply(&mut pet, ev("a", Kind::ToolDone { call: call("", "Bash") }), 40_000);
        let out = apply(&mut pet, ev("a", start()), 100_000).unwrap();
        assert_eq!((out.turns, out.worked_ms), (1, 10_000));
        let out = apply(&mut pet, ev("a", Kind::Done), 105_000).unwrap();
        assert_eq!(out.worked_ms, 5_000);
        // Rest that ends no turn counts nothing.
        apply(&mut pet, ev("b", start()), 200_000);
        assert_eq!(apply(&mut pet, ev("b", Kind::Rest), 210_000).unwrap().worked_ms, 0);
    }

    const HIDE: u64 = IDLE_HIDE_MIN * 60 * 1000;

    fn ids(pet: &Pet) -> Vec<String> {
        let mut ids: Vec<String> = pet.list().as_array().unwrap().iter().map(|s| s["id"].as_str().unwrap().to_string()).collect();
        ids.sort();
        ids
    }

    #[test]
    fn only_a_session_at_rest_goes_out_of_sight() {
        // At work, waiting on you, an ending not yet seen: in sight however long.
        for mood in ["working", "waiting", "done", "review", "error"] {
            assert!(is_shown(mood, 0, None, FORGET_MS * 10, HIDE), "{mood}");
        }
        // At rest: until quiet that long since its last event,
        assert!(is_shown("idle", 1_000, None, 1_000 + HIDE - 1, HIDE));
        assert!(!is_shown("idle", 1_000, None, 1_000 + HIDE, HIDE));
        // or since its ending was seen, whichever came later.
        assert!(is_shown("idle", 1_000, Some(5_000), 1_000 + HIDE, HIDE));
        assert!(!is_shown("idle", 1_000, Some(5_000), 5_000 + HIDE, HIDE));
        assert!(!is_shown("idle", 9_000, Some(5_000), 9_000 + HIDE, HIDE));
        // Always, when set so.
        assert!(is_shown("idle", 0, None, FORGET_MS * 10, 0));
    }

    #[test]
    fn a_seen_ending_goes_out_of_sight_and_comes_back_as_it_was() {
        let mut pet = Pet::default();
        apply(&mut pet, ev_in("a", "pet", Kind::TurnStart { prompt: Some("修复岛的设置".into()) }), 1_000);
        apply(&mut pet, ev("a", Kind::Done), 61_000);
        apply(&mut pet, ev("b", start()), 61_000);
        // Not seen yet: kept past the time, as is the one at work.
        let seen = 61_000 + HIDE * 2;
        assert!(!pet.hide_quiet(seen, HIDE));
        assert_eq!(ids(&pet), ["a", "b"]);
        // Seen: quiet from then on, and out of sight once it has been that long.
        pet.seen(seen);
        assert!(!pet.hide_quiet(seen + HIDE - 1, HIDE));
        assert!(pet.hide_quiet(seen + HIDE, HIDE));
        assert_eq!(ids(&pet), ["b"]);
        let now = pet.get(seen + HIDE);
        assert_eq!((now["id"].as_str(), now["sessions"].as_u64(), now["others"].as_u64()), (Some("b"), Some(1), Some(0)));
        // Kept all the same: its names stay out of a crash report.
        assert!(pet.names().contains(&"修复岛的设置".to_string()));
        // Its next event brings it back at once, as it was.
        let back = seen + HIDE + 10;
        apply(&mut pet, ev("a", start()), back);
        assert!(!pet.hide_quiet(back, HIDE));
        let a = pet.list().as_array().unwrap().iter().find(|s| s["id"] == "a").cloned().unwrap();
        assert_eq!((a["name"].as_str(), a["project"].as_str(), a["mood"].as_str()), (Some("修复岛的设置"), Some("pet"), Some("working")));
        assert_eq!(pet.get(back)["sessions"], 2);
    }

    #[test]
    fn an_ending_folded_away_counts_from_then() {
        let mut pet = Pet::default();
        pet.apply(&ev("a", Kind::Failed { error: Failure::Said { said: "no".into() } }), None, 0, Hold::Secs(8));
        assert!(pet.tick(8_000));
        assert!(!pet.hide_quiet(8_000 + HIDE - 1, HIDE));
        assert!(pet.hide_quiet(8_000 + HIDE, HIDE));
        assert!(ids(&pet).is_empty());
        assert_eq!(pet.get(8_000 + HIDE)["sessions"], 0);
        // Always shown, set so: back in sight.
        assert!(pet.hide_quiet(8_000 + HIDE, 0));
        assert_eq!(ids(&pet), ["a"]);
    }

    #[test]
    fn work_given_up_and_endings_never_seen_count_from_their_last_event() {
        let mut pet = Pet::default();
        apply(&mut pet, ev("a", Kind::Working), 0);
        apply(&mut pet, ev("b", Kind::Done), 0);
        // Stuck work gone to rest stays in sight until quiet the time from its last event.
        pet.check_stale(STALE_MS + 1);
        assert!(!pet.hide_quiet(STALE_MS + 1, HIDE));
        assert!(pet.hide_quiet(HIDE, HIDE));
        assert_eq!(ids(&pet), ["b"]);
        // The ending nobody saw, given up after FORGET_MS: out of sight at once.
        pet.check_stale(FORGET_MS + 1);
        assert!(pet.hide_quiet(FORGET_MS + 1, HIDE));
        assert!(ids(&pet).is_empty());
        // Forgotten as before, out of sight or not.
        assert_eq!(pet.sessions.sessions().count(), 2);
        pet.check_stale(STALE_MS + 1 + FORGET_MS + 1);
        assert_eq!(pet.sessions.sessions().count(), 1);
    }
    #[test]
    fn file_list_capped_and_relative_to_cwd() {
        let mut pet = Pet::default();
        apply(&mut pet, ev_in("a", "pet", start()), 1);
        for i in 1..=18 {
            let file = if i == 18 {
                "D:\\external\\tools\\config.rs".to_string()
            } else {
                format!("Z:\\nowhere\\pet\\src\\file{i}.rs")
            };
            apply(&mut pet, ev("a", Kind::ToolDone { call: Call { file: Some(file.clone()), ..call(&format!("c{i}"), "Edit") } }), i + 1);
        }
        let now = pet.get(30);
        assert_eq!(now["files"], 18);
        let list = now["fileList"].as_array().unwrap();
        assert_eq!(list.len(), 15);
        assert_eq!(list[0], "src/file1.rs");
        assert_eq!(list[14], "src/file15.rs");
        // Verify external files retain whole path with forward slashes
        assert_eq!(relative_file("D:\\external\\tools\\config.rs", Some("Z:\\nowhere\\pet")), "D:/external/tools/config.rs");
        // Verify non-ASCII (UTF-8) paths do not panic when prefix boundary splits multi-byte char
        assert_eq!(relative_file("C:/项目/main.rs", Some("C:/a")), "C:/项目/main.rs");
    }

    #[test]
    fn recent_steps_records_five_and_clears_on_new_turn() {
        let mut pet = Pet::default();
        apply(&mut pet, ev("a", start()), 1);
        for i in 1..=6 {
            let ok = i % 2 == 1;
            let kind = if ok {
                Kind::ToolDone { call: call(&format!("t{i}"), "Bash") }
            } else {
                Kind::ToolFailed { call: call(&format!("t{i}"), "Bash"), quiet: false }
            };
            apply(&mut pet, ev("a", kind), (i + 1) * 10);
        }
        let s = pet.list().as_array().unwrap()[0].clone();
        let steps = s["recentSteps"].as_array().unwrap();
        assert_eq!(steps.len(), 5);
        // Step 1 was pushed out by step 6
        assert_eq!(steps[0]["ok"], false);
        assert_eq!(steps[4]["ok"], false);
        // New turn begins: cleared
        apply(&mut pet, ev("a", start()), 100);
        let s2 = pet.list().as_array().unwrap()[0].clone();
        assert_eq!(s2["recentSteps"].as_array().unwrap().len(), 0);
    }
}
