// Codex hook events → pet messages, the same messages as events.rs makes
// (state.rs has the format), plus agent: "codex" and the turn they belong to.
// The words for a step, a task, a reply and the project are events.rs's.
//
// Codex runs its hooks as commands: this program with connection::CODEX_FLAG
// hands each event to /hook?agent=codex (main.rs). What differs from Claude
// Code: the shell tool is Bash and every edit is apply_patch (the patch is
// tool_input.command); a failed tool call gets no PostToolUse, a failed
// request no Stop; Interrupt is the person stopping a turn. Each step's hooks
// run in the background (connection.rs), so one can arrive after its turn is
// over: the turn id tells it apart (state.rs).
use serde_json::{json, Value};

use crate::events::{base, project_of, reply_of, step, step_of, task_of};

fn s<'a>(e: &'a Value, key: &str) -> &'a str {
    e.get(key).and_then(Value::as_str).unwrap_or("")
}

// The files a patch adds, changes or deletes.
fn patched(patch: &str) -> Vec<&str> {
    patch
        .lines()
        .filter_map(|line| ["*** Add File: ", "*** Update File: ", "*** Delete File: "].iter().find_map(|p| line.strip_prefix(p)))
        .map(str::trim)
        .collect()
}

// What a tool is doing, in plain words: a patch by the file it edits.
fn step_of_codex(tool: &str, input: &Value) -> Value {
    if tool != "apply_patch" {
        return step_of(tool, input);
    }
    match patched(s(input, "command")).as_slice() {
        [] => step("step.tool", json!({ "tool": tool })),
        [one] => step("step.edit", json!({ "file": base(one) })),
        [first, rest @ ..] => step("step.edit", json!({ "file": format!("{} +{}", base(first), rest.len()) })),
    }
}

// The mood part of the message for an event, or None for one that changes nothing.
fn mood_of(e: &Value) -> Option<Value> {
    let tool = s(e, "tool_name");
    let input = e.get("tool_input").unwrap_or(&Value::Null);
    let mut m = match s(e, "hook_event_name") {
        // A compaction starts a new context mid-work: nothing changed for the pet.
        "SessionStart" if s(e, "source") == "compact" => return None,
        "SessionStart" => json!({ "mood": "idle", "react": "wave", "say": { "key": "say.hello" } }),
        "SessionEnd" => json!({ "mood": "idle", "event": "session-end" }),
        "UserPromptSubmit" => {
            let mut m = json!({ "mood": "working", "event": "turn-start" });
            if let Some(task) = task_of(s(e, "prompt")) {
                m["task"] = json!(task);
            }
            m
        }
        // Codex's question to the person waits on them like a prompt.
        "PreToolUse" if tool == "request_user_input" => json!({ "mood": "waiting", "detail": { "key": "detail.asksQuestion" } }),
        "PreToolUse" => json!({ "mood": "working", "detail": tool, "event": "tool-start", "step": step_of_codex(tool, input) }),
        "PostToolUse" => {
            let mut m = json!({ "mood": "working", "detail": tool, "event": "tool-done" });
            if let Some(file) = patched(s(input, "command")).first().filter(|_| tool == "apply_patch") {
                m["file"] = json!(file);
            }
            m
        }
        "PermissionRequest" => json!({ "mood": "waiting", "detail": { "key": "detail.approve", "vars": { "what": step_of_codex(tool, input) } } }),
        "Stop" => json!({ "mood": "done", "event": "turn-end" }),
        // Stopped by the person: nothing to come and see.
        "Interrupt" => json!({ "mood": "idle", "event": "turn-end" }),
        _ => return None,
    };
    if matches!(s(e, "hook_event_name"), "PreToolUse" | "PostToolUse") && !s(e, "tool_use_id").is_empty() {
        m["toolId"] = json!(s(e, "tool_use_id"));
    }
    if let Some(reply) = reply_of(s(e, "last_assistant_message")) {
        m["reply"] = json!(reply);
    }
    Some(m)
}

// The message for an event, or None for one that changes nothing.
pub fn to_message(e: &Value) -> Option<Value> {
    if !e.is_object() {
        return None;
    }
    let mut message = mood_of(e)?;
    message["agent"] = json!("codex");
    for (from, to) in [("session_id", "session"), ("turn_id", "turn")] {
        if !s(e, from).is_empty() {
            message[to] = json!(s(e, from));
        }
    }
    if !s(e, "cwd").is_empty() {
        message["project"] = json!(project_of(s(e, "cwd")));
    }
    Some(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    // As Codex 0.146 sends them (trimmed); the folder is no git repository.
    fn event(name: &str, more: Value) -> Value {
        let mut e = json!({ "session_id": "s", "turn_id": "t1", "cwd": "Z:\\nowhere\\pet", "hook_event_name": name, "model": "gpt-5.6-luna" });
        for (k, v) in more.as_object().unwrap() {
            e[k] = v.clone();
        }
        e
    }

    #[test]
    fn codex_events_become_messages() {
        let m = to_message(&event("PreToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": "cd x && cargo test" }, "tool_use_id": "u1" }))).unwrap();
        assert_eq!(
            m,
            json!({ "mood": "working", "detail": "Bash", "event": "tool-start", "step": { "key": "step.command", "vars": { "what": "cargo test" } },
                    "toolId": "u1", "agent": "codex", "session": "s", "turn": "t1", "project": "pet" })
        );
        let patch = "*** Begin Patch\n*** Add File: D:\\w\\note.txt\n+x\n*** End Patch";
        let m = to_message(&event("PostToolUse", json!({ "tool_name": "apply_patch", "tool_input": { "command": patch } }))).unwrap();
        assert_eq!((m["event"].as_str(), m["file"].as_str()), (Some("tool-done"), Some("D:\\w\\note.txt")));
        let m = to_message(&event("PreToolUse", json!({ "tool_name": "apply_patch", "tool_input": { "command": patch } }))).unwrap();
        assert_eq!(m["step"], json!({ "key": "step.edit", "vars": { "file": "note.txt" } }));
        let m = to_message(&event("UserPromptSubmit", json!({ "prompt": "fix the login page" }))).unwrap();
        assert_eq!((m["event"].as_str(), m["task"].as_str()), (Some("turn-start"), Some("fix the login page")));
        let m = to_message(&event("Stop", json!({ "stop_hook_active": false, "last_assistant_message": "All set." }))).unwrap();
        assert_eq!((m["mood"].as_str(), m["event"].as_str(), m["reply"].as_str()), (Some("done"), Some("turn-end"), Some("All set.")));
        let m = to_message(&event("Interrupt", json!({}))).unwrap();
        assert_eq!((m["mood"].as_str(), m["event"].as_str()), (Some("idle"), Some("turn-end")));
        let m = to_message(&event("PermissionRequest", json!({ "tool_name": "Bash", "tool_input": { "command": "rm -rf build" } }))).unwrap();
        assert_eq!(m["detail"], json!({ "key": "detail.approve", "vars": { "what": { "key": "step.command", "vars": { "what": "rm -rf build" } } } }));
        assert_eq!(to_message(&event("PreToolUse", json!({ "tool_name": "request_user_input" }))).unwrap()["mood"], "waiting");
        assert_eq!(to_message(&event("SessionStart", json!({ "source": "startup" }))).unwrap()["react"], "wave");
        assert_eq!(to_message(&event("SessionEnd", json!({ "reason": "other" }))).unwrap()["event"], "session-end");
    }

    #[test]
    fn codex_events_that_change_nothing() {
        assert!(to_message(&event("SessionStart", json!({ "source": "compact" }))).is_none());
        for name in ["PreCompact", "PostCompact", "SubagentStart", "SubagentStop", "Nope"] {
            assert!(to_message(&event(name, json!({}))).is_none(), "{name}");
        }
        assert!(to_message(&json!("nope")).is_none());
    }
}
