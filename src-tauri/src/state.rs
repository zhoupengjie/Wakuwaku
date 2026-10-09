// What the pet is doing, from the messages the hooks bring (events.rs,
// events_codex.rs). A message is
//   { session?, project?, mood?, detail?, event?, react?, say?,
//     title?, task?, step?, toolId?, todo?, file?, reply?, error?, agent?, turn? }
//   mood     idle | working | waiting | done | review | error
//   detail   a tool name, or { key, vars } to translate
//   event    turn-start | tool-start | tool-done | tool-failed | turn-end | session-end
//   react    wave | jump | failed: played once over the mood, `say` in the bubble
//   agent    "codex", or none for Claude Code
//   turn     the turn it belongs to (Codex): some of its hooks run in the
//            background, so one can come after its turn ended (turn-end), and
//            changes nothing then
//   title    the session's name; task   the person's words for the work
//   step     what a starting tool does, { key, vars }; toolId pairs it with its end
//   todo     { set: [{ text, active, status }] }, { add: { text, active } }
//            or { update: { id, status?, text?, active? } }
//   file     a file just edited
//   reply    how the turn ended, in Claude's words; error   why it failed
//   chain    the processes it runs in, [[pid, started], ...] (jump.rs)
//
// Each session keeps its own mood; the pet shows the one that most wants you.
// Instead of callbacks and timers, calls return what happened (Outcome) and
// tick() settles the endings that only stay a while.
use std::collections::HashMap;

use serde_json::{json, Map, Value};

use crate::jump;

const MOODS: [&str; 6] = ["idle", "working", "waiting", "done", "review", "error"];
const REACTS: [&str; 3] = ["wave", "jump", "failed"];
// apply_patch: every edit Codex makes.
const EDIT_TOOLS: [&str; 5] = ["Edit", "Write", "MultiEdit", "NotebookEdit", "apply_patch"];
// The turns remembered as ended, per session.
const ENDED_TURNS: usize = 4;

// A session that died mid-turn never says idle: give up on it after this.
pub const STALE_MS: u64 = 15 * 60 * 1000;
// An ending nobody came to see, or a session gone quiet, after this.
pub const FORGET_MS: u64 = 2 * 60 * 60 * 1000;
// Never more than this many of a list kept for a session.
const MAX_KEPT: usize = 100;
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
    // For today's count: turns begun, and how long the ones that ended took.
    pub turns: u64,
    pub worked_ms: u64,
}

// A tool running now, and what it does.
struct Running {
    id: String,
    step: Value,
    since: u64,
}

struct Todo {
    id: String,
    text: String,
    active: String,
    status: String,
}

struct Session {
    id: String,
    project: String,
    mood: String,
    detail: Value,
    at: u64,
    since: Option<u64>,
    took: Option<u64>,
    has_edited: bool,
    settle_at: Option<u64>,
    title: String,
    task: String,
    // The tools running, the latest last (subagents run theirs inside one).
    running: Vec<Running>,
    // When the last tool ended, or the turn began: thinking since then.
    thought_at: Option<u64>,
    todos: Vec<Todo>,
    // To-dos ever added one by one: Claude Code numbers them from 1.
    added: u32,
    // The files this turn edited.
    files: Vec<String>,
    reply: String,
    error: Value,
    agent: String,
    // The processes it runs in, for going to its window.
    chain: jump::Chain,
    // The turn under way, and the last few that ended (Codex).
    turn: String,
    ended_turns: Vec<String>,
}

impl Session {
    fn new(id: String, now: u64) -> Self {
        Session {
            id,
            project: String::new(),
            mood: "idle".into(),
            detail: Value::String(String::new()),
            at: now,
            since: None,
            took: None,
            has_edited: false,
            settle_at: None,
            title: String::new(),
            task: String::new(),
            running: Vec::new(),
            thought_at: None,
            todos: Vec::new(),
            added: 0,
            files: Vec::new(),
            reply: String::new(),
            error: Value::Null,
            agent: String::new(),
            chain: Vec::new(),
            turn: String::new(),
            ended_turns: Vec::new(),
        }
    }

    // What it is called: Claude Code's name for it, or the person's words.
    fn name(&self) -> &str {
        if self.title.is_empty() { &self.task } else { &self.title }
    }

    fn whose(&self) -> String {
        clip(if self.name().is_empty() { &self.project } else { self.name() }, WHOSE_MAX)
    }
}

fn clip(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

// A text a message may carry: a string, or { key, vars } whose vars are
// strings, numbers, or (once) a text of the same kind.
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

fn str_field(msg: &Value, key: &str, max: usize) -> String {
    msg.get(key).and_then(Value::as_str).map(|s| clip(s, max)).unwrap_or_default()
}

// A change to the session's to-do list.
fn apply_todo(s: &mut Session, todo: &Value) {
    let item = |v: &Value, id: String| Todo {
        id,
        text: str_field(v, "text", 80),
        active: str_field(v, "active", 80),
        status: str_field(v, "status", 20),
    };
    if let Some(items) = todo.get("set").and_then(Value::as_array) {
        s.todos = items.iter().take(MAX_KEPT).enumerate().map(|(n, v)| item(v, (n + 1).to_string())).collect();
    } else if let Some(v) = todo.get("add") {
        s.added += 1;
        let mut t = item(v, s.added.to_string());
        if t.status.is_empty() {
            t.status = "pending".into();
        }
        if s.todos.len() < MAX_KEPT {
            s.todos.push(t);
        }
    } else if let Some(v) = todo.get("update") {
        let id = str_field(v, "id", 40);
        let status = str_field(v, "status", 20);
        if status == "deleted" {
            s.todos.retain(|t| t.id != id);
        } else if let Some(t) = s.todos.iter_mut().find(|t| t.id == id) {
            for (key, field) in [("status", &mut t.status), ("text", &mut t.text), ("active", &mut t.active)] {
                let value = str_field(v, key, 80);
                if !value.is_empty() {
                    *field = value;
                }
            }
        }
    }
}

// How far down its to-do list a session is, and what it is on.
fn todo_view(s: &Session) -> Value {
    if s.todos.is_empty() {
        return Value::Null;
    }
    let done = s.todos.iter().filter(|t| t.status == "completed").count();
    let active = s.todos.iter().find(|t| t.status == "in_progress").map_or("", |t| if t.active.is_empty() { &t.text } else { &t.active });
    json!({ "done": done, "total": s.todos.len(), "active": active })
}

// A session as the pages draw it.
fn view(s: &Session) -> Value {
    let is_working = s.mood == "working";
    let (step, step_since) = match s.running.last() {
        _ if !is_working => (Value::Null, None),
        Some(r) => (r.step.clone(), Some(r.since)),
        None => (json!({ "key": "step.thinking", "vars": {} }), s.thought_at),
    };
    json!({
        "id": s.id,
        "project": s.project,
        "agent": s.agent,
        // A window to go to when clicked.
        "jump": !s.chain.is_empty(),
        "mood": s.mood,
        "detail": s.detail,
        "at": s.at,
        "since": s.since,
        "took": s.took,
        "name": s.name(),
        "step": step,
        "stepSince": step_since,
        "todo": todo_view(s),
        "files": s.files.len(),
        "reply": if is_ending(&s.mood) { s.reply.as_str() } else { "" },
        "error": if s.mood == "error" { s.error.clone() } else { Value::Null },
    })
}

#[derive(Default)]
pub struct Pet {
    sessions: HashMap<String, Session>,
}

// Set a session's mood; Some(mood) when it wants a chime.
fn set(s: &mut Session, mood: &str, detail: Value, now: u64, hold: Hold) -> Option<String> {
    let was = std::mem::replace(&mut s.mood, mood.to_string());
    s.settle_at = None;
    s.detail = detail;
    s.at = now;

    if mood == "working" || mood == "waiting" {
        s.since.get_or_insert(now);
        s.took = None;
    } else {
        if let (Some(since), true) = (s.since, is_ending(mood)) {
            s.took = Some(now - since);
        }
        s.since = None;
        s.running.clear();
    }

    if let (true, Hold::Secs(secs)) = (is_ending(mood), hold) {
        s.settle_at = Some(now + secs * 1000);
    }
    (mood != was && (mood == "waiting" || is_ending(mood))).then(|| mood.to_string())
}

impl Pet {
    // Apply a message; None when it carries nothing we know.
    pub fn apply(&mut self, msg: &Value, now: u64, hold: Hold) -> Option<Outcome> {
        let mood = msg.get("mood").and_then(Value::as_str).filter(|m| MOODS.contains(m));
        let react = msg.get("react").and_then(Value::as_str).filter(|r| REACTS.contains(r));
        if mood.is_none() && react.is_none() {
            return None;
        }
        let mut out = Outcome::default();

        if let Some(mood) = mood {
            let id = str_field(msg, "session", 200);
            let project = str_field(msg, "project", 200);
            let detail = text(msg.get("detail"), 80);
            let event = msg.get("event").and_then(Value::as_str).unwrap_or("");
            let agent = str_field(msg, "agent", 20);
            let turn = str_field(msg, "turn", 100);

            if event == "session-end" {
                self.sessions.remove(&id);
            } else {
                let s = self.sessions.entry(id.clone()).or_insert_with(|| Session::new(id, now));
                if !project.is_empty() {
                    s.project = project;
                }
                let chain = jump::chain_from_json(msg.get("chain"));
                if !chain.is_empty() {
                    s.chain = chain;
                }
                if !agent.is_empty() {
                    s.agent = agent;
                }
                let is_edit = event == "tool-done" && detail.as_str().is_some_and(|d| EDIT_TOOLS.contains(&d));
                // Late, from a turn already over: only an edit counts, making its done a review.
                if !turn.is_empty() && s.ended_turns.contains(&turn) {
                    if is_edit && s.mood == "done" {
                        s.mood = "review".into();
                        out.changed = true;
                    }
                    return Some(out);
                }
                for (key, field) in [("title", &mut s.title), ("task", &mut s.task)] {
                    let value = str_field(msg, key, 80);
                    if !value.is_empty() {
                        *field = value;
                    }
                }
                if !turn.is_empty() && event == "turn-end" {
                    s.ended_turns.push(turn.clone());
                    if s.ended_turns.len() > ENDED_TURNS {
                        s.ended_turns.remove(0);
                    }
                }
                // A turn starting anew (Codex may report a step of it first).
                let is_new_turn = turn.is_empty() || turn != s.turn;
                if !turn.is_empty() {
                    s.turn = turn;
                }
                if event == "turn-start" && is_new_turn {
                    out.turns += 1;
                    s.has_edited = false;
                    s.since = None;
                    s.running.clear();
                    s.thought_at = Some(now);
                    s.files.clear();
                    s.reply.clear();
                    s.error = Value::Null;
                    // A list all done belongs to the work before.
                    if s.todos.iter().all(|t| t.status == "completed") {
                        s.todos.clear();
                    }
                }
                let tool_id = str_field(msg, "toolId", 200);
                match event {
                    "tool-start" => {
                        s.running.retain(|r| tool_id.is_empty() || r.id != tool_id);
                        if s.running.len() >= MAX_KEPT {
                            s.running.remove(0);
                        }
                        s.running.push(Running { id: tool_id, step: text(msg.get("step"), 80), since: now });
                    }
                    "tool-done" | "tool-failed" => {
                        // Its own start, or with no id to pair them by, the latest.
                        match s.running.iter().rposition(|r| r.id == tool_id) {
                            Some(at) => drop(s.running.remove(at)),
                            None if tool_id.is_empty() => drop(s.running.pop()),
                            None => {}
                        }
                        if s.running.is_empty() {
                            s.thought_at = Some(now);
                        }
                    }
                    _ => {}
                }
                if is_edit {
                    s.has_edited = true;
                }
                let file = str_field(msg, "file", 400);
                if !file.is_empty() && !s.files.contains(&file) && s.files.len() < MAX_KEPT {
                    s.files.push(file);
                }
                if let Some(todo) = msg.get("todo") {
                    apply_todo(s, todo);
                }
                let reply = str_field(msg, "reply", 160);
                if !reply.is_empty() {
                    s.reply = reply;
                }
                if msg.get("error").is_some() {
                    s.error = text(msg.get("error"), 80);
                }
                let mut mood = mood;
                if mood == "done" && s.has_edited {
                    mood = "review";
                }
                if mood == "done" || mood == "review" {
                    s.has_edited = false;
                }
                let whose = s.whose();
                let was_busy = s.since.is_some();
                out.alerts.extend(set(s, mood, detail, now, hold).map(|m| (m, whose)));
                if was_busy && is_ending(mood) {
                    out.worked_ms += s.took.unwrap_or(0);
                }
            }
            out.changed = true;
        }

        if let Some(react) = react {
            out.react = Some(json!({ "react": react, "say": text(msg.get("say"), 80) }));
        }
        Some(out)
    }

    // You looked: every ending goes back to rest. True when anything did.
    pub fn seen(&mut self, now: u64, hold: Hold) -> bool {
        let mut changed = false;
        for s in self.sessions.values_mut().filter(|s| is_ending(&s.mood)) {
            set(s, "idle", Value::String(String::new()), now, hold);
            changed = true;
        }
        changed
    }

    // Endings held for a number of seconds go back to rest when it is up.
    pub fn tick(&mut self, now: u64, hold: Hold) -> bool {
        let mut changed = false;
        for s in self.sessions.values_mut() {
            if s.settle_at.is_some_and(|t| t <= now) {
                set(s, "idle", Value::String(String::new()), now, hold);
                changed = true;
            }
        }
        changed
    }

    pub fn check_stale(&mut self, now: u64, hold: Hold) -> bool {
        let mut changed = false;
        self.sessions.retain(|_, s| !(s.mood == "idle" && now.saturating_sub(s.at) > FORGET_MS));
        for s in self.sessions.values_mut() {
            let age = now.saturating_sub(s.at);
            let is_stuck = (s.mood == "working" || s.mood == "waiting") && age > STALE_MS;
            if is_stuck || (is_ending(&s.mood) && age > FORGET_MS) {
                set(s, "idle", Value::String(String::new()), now, hold);
                changed = true;
            }
        }
        changed
    }

    // What the pet shows: the session that most wants you, and the rest.
    pub fn get(&self, now: u64) -> Value {
        let top = self
            .sessions
            .values()
            .fold(None::<&Session>, |best, s| match best {
                Some(b) if priority(&s.mood) < priority(&b.mood) => Some(b),
                Some(b) if priority(&s.mood) == priority(&b.mood) && s.at < b.at => Some(b),
                _ => Some(s),
            });
        let Some(top) = top else {
            let mut empty = view(&Session::new(String::new(), now));
            empty["others"] = json!(0);
            empty["sessions"] = json!(0);
            return empty;
        };
        let mut now = view(top);
        now["others"] = json!(self.sessions.values().filter(|s| s.id != top.id && s.mood != "idle").count());
        now["sessions"] = json!(self.sessions.len());
        now
    }

    // Every session, the one that most wants you first.
    pub fn list(&self) -> Value {
        let mut all: Vec<&Session> = self.sessions.values().filter(|s| !s.id.is_empty() || s.mood != "idle").collect();
        all.sort_by(|a, b| priority(&b.mood).cmp(&priority(&a.mood)).then(b.at.cmp(&a.at)));
        Value::Array(all.into_iter().map(view).collect())
    }

    // The mood of the next session that wants something, after the one shown:
    // for the island's second bubble.
    pub fn second(&self) -> Option<String> {
        let mut busy: Vec<&Session> = self.sessions.values().filter(|s| s.mood != "idle").collect();
        busy.sort_by(|a, b| priority(&b.mood).cmp(&priority(&a.mood)).then(b.at.cmp(&a.at)));
        busy.get(1).map(|s| s.mood.clone())
    }

    // Whether a session wants you: waiting on you, or an ending not yet seen.
    // Widgets keep to themselves meanwhile.
    pub fn wants_you(&self) -> bool {
        self.sessions.values().any(|s| s.mood == "waiting" || is_ending(&s.mood))
    }

    // Whether a session already knows where it runs.
    pub fn has_chain(&self, id: &str) -> bool {
        self.sessions.get(id).is_some_and(|s| !s.chain.is_empty())
    }

    // Where to go for a session: its chain, and the words its window's
    // title may hold (its name, its project).
    pub fn jump_target(&self, id: &str) -> Option<(jump::Chain, Vec<String>)> {
        let s = self.sessions.get(id).filter(|s| !s.chain.is_empty())?;
        Some((s.chain.clone(), vec![s.title.clone(), s.task.clone(), s.project.clone()]))
    }

    pub fn mood(&self, now: u64) -> String {
        self.get(now)["mood"].as_str().unwrap_or("idle").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(v: Value) -> Value {
        v
    }

    #[test]
    fn a_turn_that_edited_ends_in_review() {
        let mut pet = Pet::default();
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "turn-start" })), 1, Hold::Seen);
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "detail": "Edit", "event": "tool-done" })), 2, Hold::Seen);
        let out = pet.apply(&msg(json!({ "session": "a", "mood": "done" })), 5, Hold::Seen).unwrap();
        assert_eq!(pet.get(6)["mood"], "review");
        assert_eq!(pet.get(6)["took"], 4);
        assert_eq!(out.alerts, vec![("review".to_string(), String::new())]);
    }

    #[test]
    fn the_session_that_most_wants_you_shows() {
        let mut pet = Pet::default();
        pet.apply(&msg(json!({ "session": "a", "mood": "waiting", "project": "one" })), 1, Hold::Seen);
        pet.apply(&msg(json!({ "session": "b", "mood": "working", "project": "two" })), 2, Hold::Seen);
        let now = pet.get(3);
        assert_eq!(now["mood"], "waiting");
        assert_eq!(now["project"], "one");
        assert_eq!(now["others"], 1);
        assert_eq!(now["sessions"], 2);
    }

    #[test]
    fn endings_stay_until_seen_or_held_time() {
        let mut pet = Pet::default();
        pet.apply(&msg(json!({ "session": "a", "mood": "done" })), 1, Hold::Seen);
        assert!(!pet.tick(1_000_000, Hold::Seen));
        assert!(pet.seen(2, Hold::Seen));
        assert_eq!(pet.get(3)["mood"], "idle");

        pet.apply(&msg(json!({ "session": "a", "mood": "error" })), 10, Hold::Secs(8));
        assert!(!pet.tick(8_009, Hold::Secs(8)));
        assert!(pet.tick(8_010, Hold::Secs(8)));
        assert_eq!(pet.get(8_011)["mood"], "idle");
    }

    #[test]
    fn a_reaction_alone_changes_no_mood() {
        let mut pet = Pet::default();
        let out = pet.apply(&msg(json!({ "react": "jump", "say": { "key": "say.taskDone", "vars": { "task": "x", "n": 3, "bad": [] } } })), 1, Hold::Seen).unwrap();
        assert!(!out.changed);
        assert_eq!(out.react.unwrap()["say"], json!({ "key": "say.taskDone", "vars": { "task": "x", "n": "3" } }));
        assert!(pet.apply(&msg(json!({ "mood": "sleepy" })), 1, Hold::Seen).is_none());
    }

    #[test]
    fn stale_work_gives_up_and_quiet_sessions_are_forgotten() {
        let mut pet = Pet::default();
        pet.apply(&msg(json!({ "session": "a", "mood": "working" })), 0, Hold::Seen);
        pet.apply(&msg(json!({ "session": "b", "mood": "idle" })), 0, Hold::Seen);
        assert!(pet.check_stale(STALE_MS + 1, Hold::Seen));
        assert_eq!(pet.get(STALE_MS + 2)["mood"], "idle");
        pet.check_stale(FORGET_MS + 1, Hold::Seen);
        assert_eq!(pet.get(FORGET_MS + 2)["sessions"], 1);
        pet.apply(&msg(json!({ "session": "a", "mood": "idle", "event": "session-end" })), 1, Hold::Seen);
        assert_eq!(pet.get(FORGET_MS + 3)["sessions"], 0);
    }

    #[test]
    fn the_step_is_the_latest_tool_running_or_thinking() {
        let mut pet = Pet::default();
        let edit = json!({ "key": "step.edit", "vars": { "file": "a.rs" } });
        let agent = json!({ "key": "step.agent", "vars": { "what": "Explore" } });
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "turn-start" })), 10, Hold::Seen);
        assert_eq!(pet.get(11)["step"]["key"], "step.thinking");
        assert_eq!(pet.get(11)["stepSince"], 10);
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "tool-start", "toolId": "1", "step": agent })), 20, Hold::Seen);
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "tool-start", "toolId": "2", "step": edit })), 30, Hold::Seen);
        assert_eq!((pet.get(31)["step"].clone(), pet.get(31)["stepSince"].clone()), (edit, json!(30)));
        // The subagent's tool ends: back to the agent, not to thinking.
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "tool-done", "toolId": "2", "detail": "Edit", "file": "D:/p/a.rs" })), 40, Hold::Seen);
        assert_eq!(pet.get(41)["step"], agent);
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "tool-failed", "toolId": "1" })), 50, Hold::Seen);
        assert_eq!((pet.get(51)["step"]["key"].clone(), pet.get(51)["stepSince"].clone()), (json!("step.thinking"), json!(50)));
        assert_eq!(pet.get(51)["files"], 1);
        // Waiting or done, there is no step.
        pet.apply(&msg(json!({ "session": "a", "mood": "done", "reply": "改好了。" })), 60, Hold::Seen);
        let now = pet.get(61);
        assert_eq!((now["mood"].as_str(), now["step"].is_null(), now["reply"].as_str(), now["files"].as_u64()), (Some("review"), true, Some("改好了。"), Some(1)));
        // The next turn starts clean.
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "turn-start" })), 70, Hold::Seen);
        assert_eq!((pet.get(71)["files"].as_u64(), pet.get(71)["reply"].as_str()), (Some(0), Some("")));
    }

    #[test]
    fn a_session_is_named_by_its_title_or_else_its_task() {
        let mut pet = Pet::default();
        let out = pet.apply(&msg(json!({ "session": "a", "project": "pet", "mood": "waiting" })), 1, Hold::Seen).unwrap();
        assert_eq!(out.alerts, vec![("waiting".to_string(), "pet".to_string())]);
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "turn-start", "task": "修复岛" })), 2, Hold::Seen);
        assert_eq!(pet.get(3)["name"], "修复岛");
        pet.apply(&msg(json!({ "session": "a", "mood": "done", "title": "岛的设置" })), 4, Hold::Seen);
        assert_eq!(pet.get(5)["name"], "岛的设置");
        // A new task does not take over a title.
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "turn-start", "task": "下一件事" })), 6, Hold::Seen);
        assert_eq!(pet.list()[0]["name"], "岛的设置");
    }

    #[test]
    fn to_do_lists_count_what_is_done() {
        let mut pet = Pet::default();
        let todo = |pet: &mut Pet, todo: Value| {
            pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "tool-start", "todo": todo })), 1, Hold::Seen);
            pet.get(2)["todo"].clone()
        };
        let list = json!({ "set": [
            { "text": "A", "active": "Doing A", "status": "completed" },
            { "text": "B", "active": "Doing B", "status": "in_progress" },
            { "text": "C", "active": "", "status": "pending" },
        ] });
        assert_eq!(todo(&mut pet, list), json!({ "done": 1, "total": 3, "active": "Doing B" }));

        let mut pet = Pet::default();
        todo(&mut pet, json!({ "add": { "text": "A", "active": "" } }));
        todo(&mut pet, json!({ "add": { "text": "B", "active": "Doing B" } }));
        todo(&mut pet, json!({ "update": { "id": "1", "status": "completed" } }));
        assert_eq!(todo(&mut pet, json!({ "update": { "id": "2", "status": "in_progress" } })), json!({ "done": 1, "total": 2, "active": "Doing B" }));
        assert_eq!(todo(&mut pet, json!({ "update": { "id": "2", "status": "deleted" } })), json!({ "done": 1, "total": 1, "active": "" }));
        // All done, and a new turn: the list is gone.
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "turn-start" })), 3, Hold::Seen);
        assert!(pet.get(4)["todo"].is_null());
    }

    #[test]
    fn texts_may_hold_one_text_inside() {
        let nested = json!({ "key": "detail.approve", "vars": { "what": { "key": "step.command", "vars": { "what": "git push", "deeper": { "key": "x" } } } } });
        assert_eq!(text(Some(&nested), 80), json!({ "key": "detail.approve", "vars": { "what": { "key": "step.command", "vars": { "what": "git push" } } } }));
    }

    // A Codex message: session c, turn t.
    fn codex(turn: &str, more: Value) -> Value {
        let mut m = json!({ "session": "c", "agent": "codex", "turn": turn });
        for (k, v) in more.as_object().unwrap() {
            m[k] = v.clone();
        }
        m
    }

    #[test]
    fn a_codex_turn_with_a_patch_ends_in_review() {
        let mut pet = Pet::default();
        pet.apply(&codex("t1", json!({ "mood": "working", "event": "turn-start" })), 1, Hold::Seen);
        pet.apply(&codex("t1", json!({ "mood": "working", "detail": "apply_patch", "event": "tool-done" })), 2, Hold::Seen);
        pet.apply(&codex("t1", json!({ "mood": "done", "event": "turn-end" })), 3, Hold::Seen);
        assert_eq!(pet.get(4)["mood"], "review");
        assert_eq!(pet.get(4)["agent"], "codex");
        assert_eq!(pet.list()[0]["agent"], "codex");
    }

    #[test]
    fn late_codex_events_change_nothing_but_a_late_edit() {
        let mut pet = Pet::default();
        // The turn's own start comes after one of its steps: it still counts the edit.
        pet.apply(&codex("t1", json!({ "mood": "working", "detail": "apply_patch", "event": "tool-done" })), 1, Hold::Seen);
        pet.apply(&codex("t1", json!({ "mood": "working", "event": "turn-start" })), 2, Hold::Seen);
        pet.apply(&codex("t1", json!({ "mood": "done", "event": "turn-end" })), 3, Hold::Seen);
        assert_eq!(pet.get(4)["mood"], "review");

        // A step after its turn ended: she stays done, not back at work.
        pet.apply(&codex("t2", json!({ "mood": "working", "event": "turn-start" })), 10, Hold::Seen);
        pet.apply(&codex("t2", json!({ "mood": "done", "event": "turn-end" })), 11, Hold::Seen);
        pet.apply(&codex("t2", json!({ "mood": "working", "detail": "Bash", "event": "tool-start", "step": { "key": "step.command", "vars": { "what": "ls" } } })), 12, Hold::Seen);
        assert_eq!(pet.get(13)["mood"], "done");
        // A late edit makes that done a review.
        pet.apply(&codex("t2", json!({ "mood": "working", "detail": "apply_patch", "event": "tool-done" })), 14, Hold::Seen);
        assert_eq!(pet.get(15)["mood"], "review");

        // Interrupted: back to rest, and its stragglers too.
        pet.apply(&codex("t3", json!({ "mood": "working", "event": "turn-start" })), 20, Hold::Seen);
        pet.apply(&codex("t3", json!({ "mood": "idle", "event": "turn-end" })), 21, Hold::Seen);
        pet.apply(&codex("t3", json!({ "mood": "working", "detail": "Bash" })), 22, Hold::Seen);
        assert_eq!(pet.get(23)["mood"], "idle");
        // The next turn works as ever.
        pet.apply(&codex("t4", json!({ "mood": "working", "event": "turn-start" })), 30, Hold::Seen);
        assert_eq!(pet.get(31)["mood"], "working");
    }

    #[test]
    fn a_session_keeps_where_it_runs() {
        let mut pet = Pet::default();
        pet.apply(&msg(json!({ "session": "a", "mood": "working", "project": "pet", "chain": [[10, 5], [8, 3]] })), 1, Hold::Seen);
        assert!(pet.has_chain("a"));
        assert_eq!(pet.get(2)["jump"], true);
        // A message without one keeps it.
        pet.apply(&msg(json!({ "session": "a", "mood": "done", "title": "岛" })), 3, Hold::Seen);
        assert_eq!(pet.jump_target("a"), Some((vec![(10, 5), (8, 3)], vec!["岛".into(), String::new(), "pet".into()])));
        pet.apply(&msg(json!({ "session": "b", "mood": "working" })), 4, Hold::Seen);
        assert!(!pet.has_chain("b") && pet.jump_target("b").is_none());
        assert_eq!(pet.list().as_array().unwrap().iter().find(|s| s["id"] == "b").unwrap()["jump"], false);
    }

    #[test]
    fn a_turn_counts_for_today_and_an_ending_wants_you() {
        let mut pet = Pet::default();
        let out = pet.apply(&msg(json!({ "session": "a", "mood": "working", "event": "turn-start" })), 1_000, Hold::Seen).unwrap();
        assert_eq!((out.turns, out.worked_ms), (1, 0));
        assert!(!pet.wants_you());
        let out = pet.apply(&msg(json!({ "session": "a", "mood": "done" })), 61_000, Hold::Seen).unwrap();
        assert_eq!((out.turns, out.worked_ms), (0, 60_000));
        assert!(pet.wants_you());
        pet.seen(62_000, Hold::Seen);
        assert!(!pet.wants_you());
    }
}
