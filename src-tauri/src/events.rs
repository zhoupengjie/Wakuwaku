// Claude Code hook events → pet messages, and texts as { key, vars } for
// src/shared/i18n.js, so the page says them in her language.
use std::path::Path;

use serde_json::{json, Value};

// What a tool that stops for the person is waiting on.
fn asks_person(tool: &str) -> Option<Value> {
    match tool {
        "AskUserQuestion" => Some(json!({ "key": "detail.asksQuestion" })),
        "ExitPlanMode" => Some(json!({ "key": "detail.planToConfirm" })),
        _ => None,
    }
}

// The mood part of the message for an event, or None for one that changes nothing.
fn mood_of(e: &Value) -> Option<Value> {
    let s = |key: &str| e.get(key).and_then(Value::as_str).unwrap_or("");
    let tool = s("tool_name");
    Some(match s("hook_event_name") {
        // A compaction starts a new context mid-work: nothing changed for the pet.
        "SessionStart" if s("source") == "compact" => return None,
        "SessionStart" => json!({ "mood": "idle", "react": "wave", "say": { "key": "say.hello" } }),
        "SessionEnd" => json!({ "mood": "idle", "event": "session-end" }),
        "UserPromptSubmit" => json!({ "mood": "working", "event": "turn-start" }),
        "PreToolUse" => match asks_person(tool) {
            Some(detail) => json!({ "mood": "waiting", "detail": detail }),
            None => json!({ "mood": "working", "detail": tool }),
        },
        "PostToolUse" => json!({ "mood": "working", "detail": tool, "event": "tool-done" }),
        // An interrupt is the person stopping it, not the tool failing.
        "PostToolUseFailure" if e.get("is_interrupt").and_then(Value::as_bool) == Some(true) => return None,
        "PostToolUseFailure" => json!({
            "mood": "working", "detail": tool, "react": "failed",
            "say": { "key": "say.toolFailed", "vars": { "tool": tool } },
        }),
        "PermissionRequest" => json!({
            "mood": "waiting",
            "detail": asks_person(tool).unwrap_or_else(|| json!({ "key": "detail.needsApproval", "vars": { "tool": tool } })),
        }),
        "Elicitation" => json!({ "mood": "waiting", "detail": { "key": "detail.needsInput", "vars": { "server": s("mcp_server_name") } } }),
        "TaskCompleted" => json!({
            "react": "jump",
            "say": if s("task_subject").is_empty() {
                json!({ "key": "say.taskDoneGeneric" })
            } else {
                json!({ "key": "say.taskDone", "vars": { "task": s("task_subject") } })
            },
        }),
        "Stop" => json!({ "mood": "done" }),
        "StopFailure" => json!({ "mood": "error" }),
        _ => return None,
    })
}

// The message for an event, or None for one that changes nothing.
pub fn to_message(e: &Value) -> Option<Value> {
    if !e.is_object() {
        return None;
    }
    let mut message = mood_of(e)?;
    if let Some(id) = e.get("session_id").and_then(Value::as_str).filter(|s| !s.is_empty()) {
        message["session"] = json!(id);
    }
    if let Some(cwd) = e.get("cwd").and_then(Value::as_str).filter(|s| !s.is_empty()) {
        if let Some(name) = Path::new(cwd).file_name() {
            message["project"] = json!(name.to_string_lossy());
        }
    }
    Some(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_become_messages() {
        let m = to_message(&json!({ "hook_event_name": "PreToolUse", "tool_name": "Bash", "session_id": "s", "cwd": "D:\\work\\wakuwaku" })).unwrap();
        assert_eq!(m, json!({ "mood": "working", "detail": "Bash", "session": "s", "project": "wakuwaku" }));
        let m = to_message(&json!({ "hook_event_name": "PreToolUse", "tool_name": "AskUserQuestion" })).unwrap();
        assert_eq!(m["mood"], "waiting");
        assert!(to_message(&json!({ "hook_event_name": "SessionStart", "source": "compact" })).is_none());
        assert!(to_message(&json!({ "hook_event_name": "PostToolUseFailure", "is_interrupt": true })).is_none());
        assert_eq!(to_message(&json!({ "hook_event_name": "Stop" })).unwrap(), json!({ "mood": "done" }));
        assert!(to_message(&json!("nope")).is_none());
    }
}
