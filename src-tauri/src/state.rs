// What the pet is doing, from the messages the hooks bring (events.rs).
// A message is
//   { session?, project?, mood?, detail?, event?, react?, say? }
//   mood     idle | working | waiting | done | review | error
//   detail   a tool name, or { key, vars } to translate
//   event    turn-start | tool-done | session-end
//   react    wave | jump | failed: played once over the mood, `say` in the bubble
//
// Each session keeps its own mood; the pet shows the one that most wants you.
// Instead of callbacks and timers, calls return what happened (Outcome) and
// tick() settles the endings that only stay a while.
use std::collections::HashMap;

use serde_json::{json, Map, Value};

const MOODS: [&str; 6] = ["idle", "working", "waiting", "done", "review", "error"];
const REACTS: [&str; 3] = ["wave", "jump", "failed"];
const EDIT_TOOLS: [&str; 4] = ["Edit", "Write", "MultiEdit", "NotebookEdit"];

// A session that died mid-turn never says idle: give up on it after this.
pub const STALE_MS: u64 = 15 * 60 * 1000;
// An ending nobody came to see, or a session gone quiet, after this.
pub const FORGET_MS: u64 = 2 * 60 * 60 * 1000;

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
    // Moods just arrived that want a chime (and a notification): (mood, project).
    pub alerts: Vec<(String, String)>,
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
}

fn clip(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

// A text a message may carry: a string, or { key, vars } of strings.
fn text(value: Option<&Value>, max: usize) -> Value {
    match value {
        Some(Value::String(s)) => Value::String(clip(s, max)),
        Some(Value::Object(o)) if o.get("key").is_some_and(Value::is_string) => {
            let mut vars = Map::new();
            if let Some(Value::Object(given)) = o.get("vars") {
                for (k, v) in given {
                    match v {
                        Value::String(s) => vars.insert(k.clone(), Value::String(clip(s, max))),
                        Value::Number(n) => vars.insert(k.clone(), Value::String(clip(&n.to_string(), max))),
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

            if event == "session-end" {
                self.sessions.remove(&id);
            } else {
                let s = self.sessions.entry(id.clone()).or_insert_with(|| Session {
                    id,
                    project: String::new(),
                    mood: "idle".into(),
                    detail: Value::String(String::new()),
                    at: now,
                    since: None,
                    took: None,
                    has_edited: false,
                    settle_at: None,
                });
                if !project.is_empty() {
                    s.project = project;
                }
                if event == "turn-start" {
                    s.has_edited = false;
                    s.since = None;
                }
                if event == "tool-done" && detail.as_str().is_some_and(|d| EDIT_TOOLS.contains(&d)) {
                    s.has_edited = true;
                }
                let mut mood = mood;
                if mood == "done" && s.has_edited {
                    mood = "review";
                }
                if mood == "done" || mood == "review" {
                    s.has_edited = false;
                }
                let project = s.project.clone();
                out.alerts.extend(set(s, mood, detail, now, hold).map(|m| (m, project)));
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
            return json!({ "mood": "idle", "detail": "", "project": "", "since": null, "took": null, "at": now, "others": 0, "sessions": 0 });
        };
        let others = self.sessions.values().filter(|s| s.id != top.id && s.mood != "idle").count();
        json!({
            "mood": top.mood,
            "detail": top.detail,
            "project": top.project,
            "since": top.since,
            "took": top.took,
            "at": top.at,
            "others": others,
            "sessions": self.sessions.len(),
        })
    }

    // Every session, the one that most wants you first, for the settings.
    pub fn list(&self) -> Value {
        let mut all: Vec<&Session> = self.sessions.values().filter(|s| !s.id.is_empty() || s.mood != "idle").collect();
        all.sort_by(|a, b| priority(&b.mood).cmp(&priority(&a.mood)).then(b.at.cmp(&a.at)));
        Value::Array(
            all.iter()
                .map(|s| json!({ "id": s.id, "project": s.project, "mood": s.mood, "detail": s.detail, "at": s.at, "since": s.since, "took": s.took }))
                .collect(),
        )
    }

    // The mood of the next session that wants something, after the one shown:
    // for the island's second bubble.
    pub fn second(&self) -> Option<String> {
        let mut busy: Vec<&Session> = self.sessions.values().filter(|s| s.mood != "idle").collect();
        busy.sort_by(|a, b| priority(&b.mood).cmp(&priority(&a.mood)).then(b.at.cmp(&a.at)));
        busy.get(1).map(|s| s.mood.clone())
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
}
