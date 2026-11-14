// Permission prompts and questions waiting on the person, answered on the
// island or above her: agent-core's asks.rs (what each is, the hook's reply
// for each choice, when one is settled in the agent's own window, how long
// one waits: ASK_TIMEOUT_MS). Here, what becomes of a session whose prompt
// went back to DeepSeek Harness's own window.
use serde_json::{json, Value};

use agent_core::event::{Agent, AgentEvent, AnswerIn, Call, Kind, Wait, What};
pub use agent_core::asks::{Asks, Handed};

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

// A prompt of DSH's handed back to its own window (her time up, "去
// DeepSeek Harness 处理", out of sight): its session still waits, now to be
// answered there ("在 DeepSeek Harness 里批准：写 note.txt", "在 DeepSeek
// Harness 里回答：…": events.rs wait_of), only within its turn still under
// way. Told to the apps listening too (shared.rs push_asks).
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
            crate::agents::events::detail_of(&handed[0]),
            json!({ "key": "detail.approveIn", "vars": { "app": "DeepSeek Harness", "what": { "key": "step.write", "vars": { "file": "probe1.txt" } } } })
        );
        // A question handed back ("去 DeepSeek Harness 处理"): to be answered there.
        let id = asks.add(dsh_question(), none(), 0, 1000).ok().unwrap();
        assert!(asks.dismiss(id));
        let handed = handed_event(&asks.take_handed()[0]);
        assert_eq!(crate::agents::events::detail_of(&handed), json!({ "key": "detail.answerIn", "vars": { "app": "DeepSeek Harness", "what": "test1 or test2?" } }));
        // So the apps listening are told: an event as any.
        assert_eq!(serde_json::to_value(&handed).unwrap()["wait"], json!({ "for": "question", "question": "test1 or test2?", "answerIn": { "in": "app", "name": "DeepSeek Harness" } }));
    }
}
