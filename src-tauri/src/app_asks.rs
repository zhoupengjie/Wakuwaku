// Approvals another program on this computer asks the person for
// (zhoupengjie/Wakuwaku#149): a bridge that would otherwise post a request
// with Allow and Deny buttons to a chat, where it times out unseen, asks
// her instead, and she shows it on her prompt panel beside the agents'
// own (after them, before a code come by mail) until the person picks.
//
//   POST /ask { id, app, title, text?, expires? }
//     id       the program's own name for the request, 1 to 128 characters
//     app      who asks, as the panel names it (40 at most): "Discord 桥"
//     title    what is to be approved, one line (120 at most)
//     text     more of it, as the panel's box shows it (2000 at most)
//     expires  seconds it waits at most: 300 by default, 10 to 3600
//   POST /ask { id, cancel: true }   the request taken back (answered elsewhere)
//
// The request is held till then, and answered once:
//   { id, decision: "allow" | "deny" }                 the person's pick
//   { id, decision: null, reason }                     nobody's here: the
//     program asks its own way. reason: "expired" (its time up), "dismissed"
//     (交回 on the panel), "cancelled" (taken back), "replaced" (the same id
//     asked again, which waits instead), "away" (she is out of sight, asked
//     for quiet, or full screen: as the agents' prompts go back to their
//     terminals), "off" (the setting appAsks)
//   { id, decision, again: true }   the same id asked again after it was
//     decided (in the last ten minutes): the same decision, asked of nobody,
//     so a program whose connection dropped as she answered gets it again
//   400 { error } (what is wrong with it), 429 { error } (too many waiting)
//
// Nothing of it runs anything: she shows the words and hands back the pick,
// as the program's to act on. Her own words around them (「外部程序」, who
// asks) keep one from passing for an agent's prompt. None comes from a web
// page (server.rs turned_away), so a program on this computer, which could
// do more than ask; no token. What is asked is not said in her log.
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};
use tauri::Emitter;

use crate::lock::Lock;
use crate::{now_ms, Shared};

// How the held request is answered: its status and body.
pub type Reply = Box<dyn FnOnce(u16, Value) + Send>;

const MOST_ID: usize = 128;
const MOST_APP: usize = 40;
const MOST_TITLE: usize = 120;
const MOST_TEXT: usize = 2000;
const WAIT_S: u64 = 300;
const MIN_WAIT_S: u64 = 10;
const MAX_WAIT_S: u64 = 3600;
// Waiting at once, at most: past them a program is told so (429).
const MOST_WAITING: usize = 5;
// A decision kept this long for the same id asked again.
const KEPT_MS: u64 = 10 * 60 * 1000;

// What a POST /ask says, read and checked (asked_of).
#[derive(Debug, PartialEq)]
pub struct Asked {
    pub key: String,
    pub app: String,
    pub title: String,
    pub text: String,
    pub wait_ms: u64,
    pub cancel: bool,
}

// A text field: given (when it must be), a string, no longer than `most`
// characters, its control characters (but the text's line breaks) spaces.
fn field(v: &Value, name: &'static str, most: usize, needed: bool, lines: bool) -> Result<String, String> {
    let raw = match v.get(name) {
        None | Some(Value::Null) if !needed => return Ok(String::new()),
        Some(Value::String(s)) => s.as_str(),
        None | Some(Value::Null) => return Err(format!("{name} is missing")),
        Some(_) => return Err(format!("{name} is not a string")),
    };
    let text: String = raw.replace("\r\n", "\n").chars().map(|c| if c.is_control() && !(lines && c == '\n') { ' ' } else { c }).collect();
    let text = text.trim().to_string();
    if needed && text.is_empty() {
        return Err(format!("{name} is empty"));
    }
    if text.chars().count() > most {
        return Err(format!("{name} is longer than {most} characters"));
    }
    Ok(text)
}

pub fn asked_of(v: &Value) -> Result<Asked, String> {
    if !v.is_object() {
        return Err("not a JSON object".into());
    }
    let key = field(v, "id", MOST_ID, true, false)?;
    if v.get("cancel").and_then(Value::as_bool) == Some(true) {
        return Ok(Asked { key, app: String::new(), title: String::new(), text: String::new(), wait_ms: 0, cancel: true });
    }
    let app = field(v, "app", MOST_APP, true, false)?;
    let title = field(v, "title", MOST_TITLE, true, false)?;
    let text = field(v, "text", MOST_TEXT, false, true)?;
    let wait_s = match v.get("expires") {
        None | Some(Value::Null) => WAIT_S,
        Some(e) => e.as_f64().filter(|s| s.is_finite()).ok_or("expires is not a number")?.round().clamp(MIN_WAIT_S as f64, MAX_WAIT_S as f64) as u64,
    };
    Ok(Asked { key, app, title, text, wait_ms: wait_s * 1000, cancel: false })
}

fn undecided(key: &str, reason: &str) -> Value {
    json!({ "id": key, "decision": Value::Null, "reason": reason })
}

// --- The ones waiting ---------------------------------------------------------------------

struct Waiting<R> {
    id: u64,
    asked: Asked,
    deadline: u64,
    reply: R,
}

// The requests waiting, oldest first, and the decisions of the last ten
// minutes by id. R: what answers a request (the held HTTP request: Reply).
pub struct Store<R: FnOnce(u16, Value)> {
    list: Vec<Waiting<R>>,
    decided: Vec<(String, &'static str, u64)>,
}

// What became of one asked (add).
#[derive(Debug, PartialEq)]
pub enum Added {
    Waits,
    // Decided before: answered so at once.
    Again,
    // Too many waiting: answered 429.
    Full,
}

impl<R: FnOnce(u16, Value)> Store<R> {
    pub const fn new() -> Self {
        Store { list: Vec::new(), decided: Vec::new() }
    }

    fn finish(&mut self, index: usize, body: Value) {
        let w = self.list.remove(index);
        (w.reply)(200, body);
    }

    // One asked, to wait as `id` (the panel's). The same id still waiting:
    // that one is let go (replaced) and this one waits.
    pub fn add(&mut self, id: u64, asked: Asked, reply: R, now: u64) -> Added {
        self.decided.retain(|(_, _, until)| *until > now);
        if let Some((_, decision, _)) = self.decided.iter().find(|(key, _, _)| *key == asked.key) {
            reply(200, json!({ "id": asked.key, "decision": decision, "again": true }));
            return Added::Again;
        }
        if let Some(i) = self.list.iter().position(|w| w.asked.key == asked.key) {
            let key = asked.key.clone();
            self.finish(i, undecided(&key, "replaced"));
        }
        if self.list.len() >= MOST_WAITING {
            reply(429, json!({ "error": format!("{MOST_WAITING} requests are waiting already") }));
            return Added::Full;
        }
        let deadline = now + asked.wait_ms;
        self.list.push(Waiting { id, asked, deadline, reply });
        Added::Waits
    }

    // The person's pick (allow or deny); false for no such one or another pick.
    pub fn answer(&mut self, id: u64, action: &str, now: u64) -> bool {
        let decision = match action {
            "allow" => "allow",
            "deny" => "deny",
            _ => return false,
        };
        let Some(i) = self.list.iter().position(|w| w.id == id) else { return false };
        let key = self.list[i].asked.key.clone();
        self.decided.push((key.clone(), decision, now + KEPT_MS));
        self.finish(i, json!({ "id": key, "decision": decision }));
        true
    }

    // 交回: the program asks its own way.
    pub fn dismiss(&mut self, id: u64) -> bool {
        let Some(i) = self.list.iter().position(|w| w.id == id) else { return false };
        let key = self.list[i].asked.key.clone();
        self.finish(i, undecided(&key, "dismissed"));
        true
    }

    // Taken back by the program that asked.
    pub fn cancel(&mut self, key: &str) -> bool {
        let Some(i) = self.list.iter().position(|w| w.asked.key == key) else { return false };
        self.finish(i, undecided(key, "cancelled"));
        true
    }

    // Those whose time is up; how many.
    pub fn expire(&mut self, now: u64) -> usize {
        let mut n = 0;
        while let Some(i) = self.list.iter().position(|w| w.deadline <= now) {
            let key = self.list[i].asked.key.clone();
            self.finish(i, undecided(&key, "expired"));
            n += 1;
        }
        n
    }

    // All let go (out of sight); how many.
    pub fn clear(&mut self, reason: &str) -> usize {
        let n = self.list.len();
        while !self.list.is_empty() {
            let key = self.list[0].asked.key.clone();
            self.finish(0, undecided(&key, reason));
        }
        n
    }

    pub fn holds(&self, id: u64) -> bool {
        self.list.iter().any(|w| w.id == id)
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    // As the prompt panel shows them (panel.ts appBody).
    pub fn views(&self) -> Vec<Value> {
        self.list
            .iter()
            .map(|w| json!({ "id": w.id, "kind": "app", "session": "", "app": w.asked.app, "title": w.asked.title, "text": w.asked.text, "until": w.deadline }))
            .collect()
    }
}

static STORE: Lock<Store<Reply>> = Lock::new("app_asks::STORE", Store::new());
// Apart from the agents' prompts (agent-core's count from 1) and the codes
// come by mail (mail/codes.rs, from 1 << 40).
static NEXT: AtomicU64 = AtomicU64::new(1 << 41);

fn store() -> crate::lock::Guard<'static, Store<Reply>> {
    STORE.lock().unwrap_or_else(|e| e.into_inner())
}

// --- For her ---------------------------------------------------------------------------------

// A POST /ask read (server.rs): checked, then held for the person, or
// answered at once (wrong, taken back, off, nobody here, decided before,
// too many). The store's lock is let go before her prompts are pushed,
// which read it.
pub fn take(sh: &Shared, body: &Value, reply: Reply) {
    let asked = match asked_of(body) {
        Ok(asked) => asked,
        Err(why) => return reply(400, json!({ "error": why })),
    };
    if asked.cancel {
        let taken = store().cancel(&asked.key);
        if taken {
            sh.log("app ask taken back by the program");
            sh.push_asks();
        }
        return reply(200, json!({ "ok": taken }));
    }
    if !sh.config().app_asks {
        return reply(200, undecided(&asked.key, "off"));
    }
    // Out of sight nobody can answer: the program asks its own way, as an
    // agent's prompt goes back to its terminal.
    if !sh.asks_seen() {
        return reply(200, undecided(&asked.key, "away"));
    }
    let id = NEXT.fetch_add(1, Ordering::SeqCst);
    let app = asked.app.clone();
    let added = store().add(id, asked, reply, now_ms());
    match added {
        Added::Waits => {
            sh.log(&format!("app ask {id} from {app}: waits on the person"));
            sh.push_asks();
            // Her chime for something waiting, the island up, as for an agent's.
            let _ = sh.app.emit_to("island", "pet:alert", json!({ "mood": "waiting" }));
        }
        Added::Again => sh.log(&format!("app ask from {app}: decided before, told again")),
        Added::Full => sh.log(&format!("app ask from {app}: turned away, {MOST_WAITING} waiting")),
    }
}

pub fn answer(id: u64, action: &str) -> bool {
    store().answer(id, action, now_ms())
}

pub fn dismiss(id: u64) -> bool {
    store().dismiss(id)
}

pub fn expire(now: u64) -> usize {
    store().expire(now)
}

pub fn clear(reason: &str) -> usize {
    store().clear(reason)
}

pub fn holds(id: u64) -> bool {
    store().holds(id)
}

pub fn is_empty() -> bool {
    store().is_empty()
}

pub fn views() -> Vec<Value> {
    store().views()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    // A reply that keeps what it was answered with.
    type Got = Arc<Mutex<Vec<(u16, Value)>>>;
    fn replier(got: &Got) -> impl FnOnce(u16, Value) {
        let got = got.clone();
        move |code, body| got.lock().unwrap().push((code, body))
    }

    fn asked(key: &str) -> Asked {
        asked_of(&json!({ "id": key, "app": "Discord 桥", "title": "Claude 想运行 npm test", "expires": 60 })).unwrap()
    }

    #[test]
    fn what_a_program_asks_is_read_and_checked() {
        let a = asked_of(&json!({ "id": "req-1", "app": " Discord 桥 ", "title": "运行\u{7}命令", "text": "rm -rf build/\r\nline two\u{1b}" })).unwrap();
        assert_eq!(a, Asked { key: "req-1".into(), app: "Discord 桥".into(), title: "运行 命令".into(), text: "rm -rf build/\nline two".into(), wait_ms: 300_000, cancel: false });
        assert_eq!(asked_of(&json!({ "id": "x", "app": "a", "title": "t", "expires": 1 })).unwrap().wait_ms, 10_000, "10 s at least");
        assert_eq!(asked_of(&json!({ "id": "x", "app": "a", "title": "t", "expires": 99999 })).unwrap().wait_ms, 3_600_000, "an hour at most");
        assert_eq!(asked_of(&json!({ "id": "x", "app": "a", "title": "t", "expires": 89.6 })).unwrap().wait_ms, 90_000);
        assert!(asked_of(&json!({ "id": "x", "cancel": true })).unwrap().cancel, "taken back needs its id alone");
        for (body, why) in [
            (json!([1]), "not a JSON object"),
            (json!({ "app": "a", "title": "t" }), "id is missing"),
            (json!({ "id": "  ", "app": "a", "title": "t" }), "id is empty"),
            (json!({ "id": 7, "app": "a", "title": "t" }), "id is not a string"),
            (json!({ "id": "x".repeat(129), "app": "a", "title": "t" }), "id is longer than 128 characters"),
            (json!({ "id": "x", "title": "t" }), "app is missing"),
            (json!({ "id": "x", "app": "a".repeat(41), "title": "t" }), "app is longer than 40 characters"),
            (json!({ "id": "x", "app": "a" }), "title is missing"),
            (json!({ "id": "x", "app": "a", "title": "t", "text": "字".repeat(2001) }), "text is longer than 2000 characters"),
            (json!({ "id": "x", "app": "a", "title": "t", "expires": "soon" }), "expires is not a number"),
        ] {
            assert_eq!(asked_of(&body).unwrap_err(), why, "{body}");
        }
    }

    #[test]
    fn a_request_waits_till_the_person_picks_and_is_answered_once() {
        let got = Got::default();
        let mut s = Store::new();
        assert_eq!(s.add(7, asked("req-1"), replier(&got), 1_000), Added::Waits);
        assert!(s.holds(7) && got.lock().unwrap().is_empty(), "held, not answered yet");
        let view = &s.views()[0];
        assert_eq!((view["kind"].as_str(), view["app"].as_str(), view["until"].as_u64()), (Some("app"), Some("Discord 桥"), Some(61_000)));
        assert!(!s.answer(7, "always", 2_000), "allow or deny alone");
        assert!(s.answer(7, "allow", 2_000));
        assert!(!s.answer(7, "deny", 2_000), "answered once");
        assert!(s.is_empty());
        assert_eq!(*got.lock().unwrap(), vec![(200, json!({ "id": "req-1", "decision": "allow" }))]);
    }

    #[test]
    fn the_same_id_asked_again_is_told_its_decision_or_waits_in_the_old_ones_place() {
        let got = Got::default();
        let mut s = Store::new();
        s.add(1, asked("req-1"), replier(&got), 0);
        // Asked again while it waits (the program's connection dropped): the
        // new one waits, the old one is let go.
        assert_eq!(s.add(2, asked("req-1"), replier(&got), 10), Added::Waits);
        assert!(!s.holds(1) && s.holds(2));
        assert!(s.answer(2, "deny", 20));
        // Asked again once decided: told at once, asked of nobody.
        assert_eq!(s.add(3, asked("req-1"), replier(&got), 30), Added::Again);
        assert!(s.is_empty());
        // Ten minutes on, it is asked anew.
        assert_eq!(s.add(4, asked("req-1"), replier(&got), 20 + KEPT_MS), Added::Waits);
        assert_eq!(
            *got.lock().unwrap(),
            vec![
                (200, json!({ "id": "req-1", "decision": Value::Null, "reason": "replaced" })),
                (200, json!({ "id": "req-1", "decision": "deny" })),
                (200, json!({ "id": "req-1", "decision": "deny", "again": true })),
            ]
        );
    }

    #[test]
    fn nobody_answering_lets_the_program_ask_its_own_way() {
        let got = Got::default();
        let mut s = Store::new();
        s.add(1, asked("a"), replier(&got), 0);
        s.add(2, asked("b"), replier(&got), 30_000);
        s.add(3, asked("c"), replier(&got), 0);
        s.add(4, asked("d"), replier(&got), 0);
        assert_eq!(s.expire(59_999), 0);
        assert_eq!(s.expire(60_000), 3, "a, c and d: their minute up");
        assert!(s.cancel("b") && !s.cancel("b"));
        s.add(5, asked("e"), replier(&got), 0);
        s.add(6, asked("f"), replier(&got), 0);
        assert!(s.dismiss(5) && !s.dismiss(5));
        assert_eq!(s.clear("away"), 1);
        let reasons: Vec<(String, String)> = got.lock().unwrap().iter().map(|(_, b)| (b["id"].as_str().unwrap().into(), b["reason"].as_str().unwrap().into())).collect();
        let want = [("a", "expired"), ("c", "expired"), ("d", "expired"), ("b", "cancelled"), ("e", "dismissed"), ("f", "away")];
        assert_eq!(reasons, want.map(|(k, r)| (k.to_string(), r.to_string())));
        // None of those is a decision: asked again, it waits.
        assert_eq!(s.add(7, asked("a"), replier(&got), 70_000), Added::Waits);
    }

    #[test]
    fn five_wait_at_most() {
        let got = Got::default();
        let mut s = Store::new();
        for i in 0..MOST_WAITING as u64 {
            assert_eq!(s.add(i, asked(&format!("r{i}")), replier(&got), 0), Added::Waits);
        }
        assert_eq!(s.add(99, asked("one more"), replier(&got), 0), Added::Full);
        assert_eq!(got.lock().unwrap()[0], (429, json!({ "error": "5 requests are waiting already" })));
        assert_eq!(s.views().len(), MOST_WAITING);
    }

    #[test]
    fn its_ids_are_apart_from_the_agents_and_the_mails() {
        assert!(NEXT.load(Ordering::SeqCst) >= 1 << 41);
    }
}
