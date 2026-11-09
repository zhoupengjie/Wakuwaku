// Permission prompts and questions waiting on the person, answered on the
// island or above her: agent-core's asks.rs (what each is, the hook's reply
// for each choice, when one is settled in the agent's own window, how long
// one waits: ASK_TIMEOUT_MS). Here, what becomes of a session whose prompt
// went back to DeepSeek Harness's own window, and mail verification notices.
use serde_json::{json, Value};

use agent_core::event::{Agent, AgentEvent, AnswerIn, Call, Kind, Wait, What};
pub use agent_core::asks::Handed;

pub struct MailAsk {
    pub id: u64,
    pub kind: String,
    pub sender: String,
    pub code_or_url: String,
    pub domain: Option<String>,
    pub subject: String,
    pub expires_at: u64,
}

pub struct Asks<R: FnOnce(Option<Value>)> {
    core: agent_core::asks::Asks<R>,
    mail_asks: Vec<MailAsk>,
    next_id: u64,
}

impl<R: FnOnce(Option<Value>)> Default for Asks<R> {
    fn default() -> Self {
        Self {
            core: agent_core::asks::Asks::default(),
            mail_asks: Vec::new(),
            next_id: 100_000,
        }
    }
}

impl<R: FnOnce(Option<Value>)> Asks<R> {
    pub fn is_empty(&self) -> bool {
        self.mail_asks.is_empty() && self.core.is_empty()
    }

    pub fn views(&self) -> Value {
        let mut views = Vec::new();
        for m in &self.mail_asks {
            if m.kind == "mail_code" {
                views.push(json!({
                    "id": m.id,
                    "session": "mail",
                    "kind": "mail_code",
                    "sender": m.sender,
                    "code": m.code_or_url,
                    "subject": m.subject,
                }));
            } else {
                let disp = m.domain.as_deref().unwrap_or(&m.code_or_url);
                views.push(json!({
                    "id": m.id,
                    "session": "mail",
                    "kind": "mail_link",
                    "sender": m.sender,
                    "domain": disp,
                    "displayUrl": disp,
                    "url": m.code_or_url,
                    "subject": m.subject,
                }));
            }
        }
        let core_val = self.core.views();
        if let Value::Array(items) = core_val {
            views.extend(items);
        }
        Value::Array(views)
    }

    pub fn take_handed(&mut self) -> Vec<Handed> {
        self.core.take_handed()
    }

    pub fn dismiss_all(&mut self) -> usize {
        let n = self.mail_asks.len();
        self.mail_asks.clear();
        n + self.core.dismiss_all()
    }

    pub fn dismiss(&mut self, id: u64) -> bool {
        if id >= 100_000 {
            if let Some(pos) = self.mail_asks.iter().position(|m| m.id == id) {
                self.mail_asks.remove(pos);
                return true;
            }
            false
        } else {
            self.core.dismiss(id)
        }
    }

    pub fn answer(&mut self, id: u64, choice: &Value, deny_message: &str) -> bool {
        if id >= 100_000 {
            if let Some(pos) = self.mail_asks.iter().position(|m| m.id == id) {
                let ask = self.mail_asks.remove(pos);
                let action = choice["action"].as_str().unwrap_or("");
                if action == "allow" || action == "copy" {
                    #[cfg(windows)]
                    set_win32_clipboard(&ask.code_or_url);
                } else if ask.kind == "mail_link" && (action == "always" || action == "open") {
                    crate::jump::open_url(&ask.code_or_url);
                }
                return true;
            }
            false
        } else {
            self.core.answer(id, choice, deny_message)
        }
    }

    pub fn expire(&mut self, now: u64) -> Vec<u64> {
        let mut expired = Vec::new();
        self.mail_asks.retain(|m| {
            if m.expires_at <= now {
                expired.push(m.id);
                false
            } else {
                true
            }
        });
        let core_expired = self.core.expire(now);
        expired.extend(core_expired);
        expired
    }

    pub fn add(&mut self, event: Value, respond: R, now_ms: u64, wait: u64) -> Result<u64, R> {
        self.core.add(event, respond, now_ms, wait)
    }

    pub fn seen(&mut self, event: &Value) -> Vec<u64> {
        self.core.seen(event)
    }

    pub fn add_mail(&mut self, kind: &str, sender: &str, code_or_url: &str, domain: Option<&str>, subject: &str, expires_at: u64) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.mail_asks.push(MailAsk {
            id,
            kind: kind.to_string(),
            sender: sender.to_string(),
            code_or_url: code_or_url.to_string(),
            domain: domain.map(|s| s.to_string()),
            subject: subject.to_string(),
            expires_at,
        });
        id
    }
}

#[cfg(windows)]
fn set_win32_clipboard(text: &str) {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "user32")]
    extern "system" {
        fn OpenClipboard(hWndNewOwner: *mut std::ffi::c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn EmptyClipboard() -> i32;
        fn SetClipboardData(uFormat: u32, hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> *mut std::ffi::c_void;
        fn GlobalFree(hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
        fn GlobalLock(hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
        fn GlobalUnlock(hMem: *mut std::ffi::c_void) -> i32;
    }

    const CF_UNICODETEXT: u32 = 13;
    const GMEM_MOVEABLE: u32 = 0x0002;

    let wide: Vec<u16> = std::ffi::OsStr::new(text).encode_wide().chain(std::iter::once(0)).collect();
    let size = wide.len() * std::mem::size_of::<u16>();

    unsafe {
        let hmem = GlobalAlloc(GMEM_MOVEABLE, size);
        if hmem.is_null() {
            return;
        }
        let ptr = GlobalLock(hmem);
        if ptr.is_null() {
            GlobalFree(hmem);
            return;
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr as *mut u16, wide.len());
        GlobalUnlock(hmem);
        if OpenClipboard(std::ptr::null_mut()) != 0 {
            EmptyClipboard();
            if SetClipboardData(CF_UNICODETEXT, hmem).is_null() {
                GlobalFree(hmem);
            }
            CloseClipboard();
        } else {
            GlobalFree(hmem);
        }
    }
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

// A prompt of DSH's handed back to its own window (her time up, "去
// DeepSeek Harness 处理", out of sight): its session still waits, now to be
// answered there ("在 DeepSeek Harness 里批准：写 note.txt", "在 DeepSeek
// Harness 里回答：…": events.rs wait_of), only within its turn still under
// way. Told to the apps listening too (main.rs push_asks).
pub fn handed_event(handed: &Handed) -> AgentEvent {
    let there = || AnswerIn::App { name: "DeepSeek Harness".into() };
    let wait = if s(&handed.view, "kind") == "question" {
        let question = handed.view["questions"][0]["question"].as_str().unwrap_or("");
        Wait::Question { question: Some(question.to_string()), answer_in: Some(there()) }
    } else {
        let input = handed.event.get("tool_input").filter(|i| i.is_object()).cloned().unwrap_or_else(|| json!({}));
        Wait::Approval { what: What::Call { call: Call::new(s(&handed.event, "tool_use_id"), s(&handed.event, "tool_name"), input) }, answer_in: there() }
    };
    AgentEvent { agent: Agent::Dsh, session: s(&handed.view, "session").to_string(), within: true, cwd: None, turn: None, title: None, reply: None, kind: Kind::Waiting { wait } }
}

#[cfg(test)]
mod tests {
    use super::*;

    // As her DSH plugin sends them, tagged by the server and put in Claude
    // Code's words (agent-core's dsh.rs for_ask).
    fn dsh_permission() -> Value {
        agent_core::dsh::for_ask(&json!({
            "hook_event_name": "PermissionRequest", "session_id": "session-ed6a", "cwd": "C:\\w\\default-workspace", "agent": "dsh",
            "tool_name": "write", "tool_input": { "file_path": "C:\\waku\\agents\\probe1.txt", "content": "hi" },
            "tool_use_id": "call_1", "permission_id": "9-1", "answerable": true
        }))
    }

    fn dsh_question() -> Value {
        agent_core::dsh::for_ask(&json!({
            "hook_event_name": "PermissionRequest", "session_id": "session-q", "agent": "dsh",
            "tool_name": "ask_user_question", "tool_use_id": "call_q", "permission_id": "9-2", "answerable": true,
            "tool_input": { "questions": [{ "id": "name", "question": "test1 or test2?", "options": [{ "label": "test1" }, { "label": "test2" }] }] }
        }))
    }

    #[test]
    fn a_dsh_prompt_handed_back_says_where_to_answer() {
        let mut asks: Asks<Box<dyn FnOnce(Option<Value>)>> = Asks::default();
        let none = || Box::new(|_: Option<Value>| {}) as Box<dyn FnOnce(Option<Value>)>;
        // Its time up: the session waits on DSH's own window for it.
        asks.add(dsh_permission(), none(), 0, 1000).ok().unwrap();
        asks.expire(1000);
        let handed: Vec<AgentEvent> = asks.take_handed().iter().map(handed_event).collect();
        assert_eq!((handed.len(), handed[0].session.as_str(), handed[0].agent, handed[0].within), (1, "session-ed6a", Agent::Dsh, true));
        assert_eq!(
            crate::events::detail_of(&handed[0]),
            json!({ "key": "detail.approveIn", "vars": { "app": "DeepSeek Harness", "what": { "key": "step.write", "vars": { "file": "probe1.txt" } } } })
        );
        // A question handed back ("去 DeepSeek Harness 处理"): to be answered there.
        let id = asks.add(dsh_question(), none(), 0, 1000).ok().unwrap();
        assert!(asks.dismiss(id));
        let handed = handed_event(&asks.take_handed()[0]);
        assert_eq!(crate::events::detail_of(&handed), json!({ "key": "detail.answerIn", "vars": { "app": "DeepSeek Harness", "what": "test1 or test2?" } }));
        // So the apps listening are told: an event as any.
        assert_eq!(serde_json::to_value(&handed).unwrap()["wait"], json!({ "for": "question", "question": "test1 or test2?", "answerIn": { "in": "app", "name": "DeepSeek Harness" } }));
    }

    #[test]
    fn mail_asks_lifecycle() {
        let mut asks: Asks<Box<dyn FnOnce(Option<Value>)>> = Asks::default();
        let id = asks.add_mail("mail_code", "GitHub", "123456", None, "Your code", 5000);
        assert_eq!(id, 100_000);
        assert!(!asks.is_empty());
        let views = asks.views();
        assert_eq!(views[0]["code"], "123456");
        assert_eq!(views[0]["kind"], "mail_code");
        assert!(asks.answer(id, &json!({ "action": "allow" }), ""));
        assert!(asks.is_empty());
    }
}
