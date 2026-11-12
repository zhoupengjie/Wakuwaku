// DeepSeek Harness's events, from her own plugin inside DSH
// (integrations/dsh/wakuwaku.js, connection/dsh.rs), or, from a DSH still running
// its Claude Code hook bridge's rows, handed on by this program with
// connection::DSH_FLAG (hooks.rs dsh_hook): agent-core reads them (dsh.rs
// to_event: its tools by Claude Code's names, the turn's last words and the
// session's name from DSH's folder when they did not come, a subagent's as
// its session's), events.rs tells them in her words. Its approvals and
// questions are held for her answer where the plugin can take one
// (answerable) and she is in sight (server.rs; for_ask makes them asks.rs's,
// side by side, each closed by its own PermissionReplied); else told only,
// to be answered in DSH (answered_in_dsh). Here, her log's lines for them.
use std::path::Path;

use serde_json::Value;

#[cfg(test)]
use serde_json::json;

use agent_core::dsh::{whose_of, Whose, LOADED};
pub use agent_core::dsh::{for_ask, is_loaded};

fn s<'a>(e: &'a Value, key: &str) -> &'a str {
    e.get(key).and_then(Value::as_str).unwrap_or("")
}

// A session's id, short: its first 8 after "session-".
fn short(id: &str) -> String {
    id.strip_prefix("session-").unwrap_or(id).chars().take(8).collect()
}

// A line for her log as a session starts or a turn begins or ends: what came,
// and whether she found the app it runs in (server.rs caller_of), for
// telling why its window can't be reached; a subagent's, whose it is.
pub fn log_line(e: &Value, found: Option<u32>, home: &Path) -> Option<String> {
    let name = s(e, "hook_event_name");
    if name == LOADED {
        let profile = if s(e, "profile").is_empty() { String::new() } else { format!(", profile {}", s(e, "profile")) };
        return Some(format!("dsh: her plugin loaded (version {}{profile}, pid {})", e["version"].as_u64().unwrap_or(0), e["pid"].as_u64().unwrap_or(0)));
    }
    // A prompt: one she can only show, and what came of each.
    if name == "PermissionRequest" && e.get("answerable") != Some(&Value::Bool(true)) {
        return Some(format!("dsh {name} ({}): her plugin cannot take an answer to it, told only", short(s(e, "session_id"))));
    }
    if name == "PermissionReplied" {
        let by = if s(e, "by") == "her" { "on her island" } else { "in DSH" };
        return Some(format!("dsh {name} ({}): {}, {by}", short(s(e, "session_id")), s(e, "outcome")));
    }
    if !matches!(name, "SessionStart" | "UserPromptSubmit" | "Stop") {
        return None;
    }
    let id = short(s(e, "session_id"));
    if let Whose::Sub(top) = whose_of(e, home) {
        return Some(format!("dsh {name} ({id}): a subagent of {}, not shown", if top.is_empty() { "a session not named".to_string() } else { short(&top) }));
    }
    let app = found.map_or("no app found".to_string(), |pid| format!("its app found (pid {pid})"));
    Some(format!("dsh {name} ({id}){}", if name == "Stop" { String::new() } else { format!(", {app}") }))
}

#[cfg(test)]
fn to_message(e: &Value, home: &Path) -> Option<Value> {
    agent_core::dsh::to_event(e, home).map(|ev| crate::events::message_of(&ev))
}

// The message for a prompt she does not hold: to be answered in DSH.
#[cfg(test)]
fn not_held(e: &Value, home: &Path) -> Value {
    let mut ev = agent_core::dsh::to_event(e, home).unwrap();
    agent_core::dsh::answered_in_dsh(&mut ev);
    crate::events::message_of(&ev)
}
#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::dsh::{project_key, session_log, title_of, whose};

    // As the CLI 0.2.0-rc.2's bridge sends them (from the test VM), in a
    // folder that is no git repository.
    fn event(name: &str, more: Value) -> Value {
        let mut e = json!({ "session_id": "session-c961", "transcript_path": "", "cwd": "Z:\\nowhere\\dsh", "hook_event_name": name });
        for (k, v) in more.as_object().unwrap() {
            e[k] = v.clone();
        }
        e
    }

    fn nowhere() -> std::path::PathBuf {
        std::path::PathBuf::from("Z:\\nowhere\\.dsh")
    }

    #[test]
    fn dsh_events_become_messages() {
        let m = to_message(&event("SessionStart", json!({ "source": "startup" })), &nowhere()).unwrap();
        assert_eq!(m, json!({ "mood": "idle", "react": "wave", "say": { "key": "say.hello" }, "agent": "dsh", "session": "session-c961", "project": "dsh" }));
        let m = to_message(&event("UserPromptSubmit", json!({ "prompt": "Read note.txt, then create copy.txt" })), &nowhere()).unwrap();
        assert_eq!((m["event"].as_str(), m["task"].as_str()), (Some("turn-start"), Some("Read note.txt, then create copy.txt")));
        let write = json!({ "tool_name": "write", "tool_input": { "file_path": "Z:\\nowhere\\dsh\\copy.txt", "content": "hi\n" }, "tool_use_id": "call_00_CKTB", "tool_response": "<path>…</path>" });
        let m = to_message(&event("PostToolUse", write), &nowhere()).unwrap();
        assert_eq!((m["event"].as_str(), m["detail"].as_str(), m["file"].as_str()), (Some("tool-done"), Some("Write"), Some("Z:\\nowhere\\dsh\\copy.txt")));
        // Refused in DSH's own dialog: no edit.
        let refused = json!({ "tool_name": "write", "tool_input": { "file_path": "C:\\waku\\agents\\outside.txt" }, "tool_response": "Error: the user rejected escalating this operation to \"danger-full-access\"" });
        let m = to_message(&event("PostToolUse", refused), &nowhere()).unwrap();
        assert_eq!((m["event"].as_str(), m.get("file")), (Some("tool-failed"), None));
        let m = to_message(&event("PreToolUse", json!({ "tool_name": "pwsh", "tool_input": { "command": "Get-ChildItem", "description": "List files" } })), &nowhere()).unwrap();
        assert_eq!(m["step"], json!({ "key": "step.run", "vars": { "what": "List files" } }));
        let m = to_message(&event("PreToolUse", json!({ "tool_name": "str_replace_editor", "tool_input": { "command": "str_replace", "path": "Z:\\a\\b.rs" } })), &nowhere()).unwrap();
        assert_eq!(m["step"], json!({ "key": "step.edit", "vars": { "file": "b.rs" } }));
        let m = to_message(&event("PreToolUse", json!({ "tool_name": "ask_user_question", "tool_input": { "questions": [] } })), &nowhere()).unwrap();
        assert_eq!(m["mood"], "waiting");
        let todos = json!({ "todos": [{ "content": "Read note.txt", "status": "completed" }, { "content": "Write copy.txt", "status": "in_progress" }] });
        let m = to_message(&event("PostToolUse", json!({ "tool_name": "todo_write", "tool_input": todos })), &nowhere()).unwrap();
        assert_eq!(m["todo"]["set"][1], json!({ "text": "Write copy.txt", "active": "", "status": "in_progress" }));
        let m = to_message(&event("Stop", json!({ "stop_hook_active": false })), &nowhere()).unwrap();
        assert_eq!((m["mood"].as_str(), m.get("reply"), m["agent"].as_str()), (Some("done"), None, Some("dsh")));
        assert!(to_message(&event("SubagentStop", json!({ "agent_id": "a" })), &nowhere()).is_none());
        let mut start = event("UserPromptSubmit", json!({ "prompt": "x" }));
        start["session_id"] = json!("session-6aa5a96d-33fc");
        assert_eq!(log_line(&start, Some(3684), &nowhere()).as_deref(), Some("dsh UserPromptSubmit (6aa5a96d), its app found (pid 3684)"));
        start["hook_event_name"] = json!("Stop");
        assert_eq!(log_line(&start, None, &nowhere()).as_deref(), Some("dsh Stop (6aa5a96d)"));
        assert!(log_line(&event("PostToolUse", json!({})), None, &nowhere()).is_none());
        assert!(to_message(&json!("nope"), &nowhere()).is_none());
    }

    // Real session logs from the test VM (DSH 0.2.0-rc.2, test prompts only):
    // c96167f1 the CLI reading note.txt and writing copy.txt, one turn;
    // 6aa5a96d the desktop app, a question then a write it was refused, two
    // turns. (agent-core's dsh.rs reads their last words.)
    const C96: &[u8] = include_bytes!("../../test/fixtures/dsh/session-c96167f1.v4.jsonl.zstd");
    const A6A: &[u8] = include_bytes!("../../test/fixtures/dsh/session-6aa5a96d.v4.jsonl.zstd");
    #[test]
    fn where_dsh_keeps_a_session_s_log() {
        assert_eq!(project_key("C:\\Users\\waku\\Documents\\deepseek-harness\\default-workspace"), "--C-Users-waku-Documents-deepseek-harness-default-workspace--");
        assert_eq!(project_key("C:\\waku\\agents\\dsh"), "--C-waku-agents-dsh--");
        assert_eq!(project_key("/home/me/my proj~1"), "--home-me-my~0020proj~007E1--");
        assert_eq!(project_key("D:\\宠物\\a.b_c-d"), "--D-~5BA0~7269-a.b_c-d--");
        assert_eq!(project_key("/"), "--root--");
        assert_eq!(project_key(&"x".repeat(300)).len(), 255);

        let home = std::env::temp_dir().join(format!("wakuwaku-dsh-home-{}", std::process::id()));
        let dir = home.join("sessions").join("--C-waku-agents-dsh--").join("session-c96167f1-eb93");
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(session_log(&home, "C:\\waku\\agents\\dsh", "session-c96167f1-eb93"), None);
        // A later generation alone, or plain JSONL: the newest; v4 when there.
        std::fs::write(dir.join("session.v3.jsonl"), b"").unwrap();
        std::fs::write(dir.join("session.v5.jsonl.zstd"), b"").unwrap();
        std::fs::write(dir.join("session.v5.jsonl.zstd.tmp"), b"").unwrap();
        assert_eq!(session_log(&home, "C:\\waku\\agents\\dsh", "session-c96167f1-eb93"), Some(dir.join("session.v5.jsonl.zstd")));
        std::fs::write(dir.join("session.v4.jsonl.zstd"), C96).unwrap();
        let log = session_log(&home, "C:\\waku\\agents\\dsh", "session-c96167f1-eb93").unwrap();
        assert_eq!(log, dir.join("session.v4.jsonl.zstd"));
        assert_eq!(session_log(&home, "", "session-c96167f1-eb93"), None);

        // A turn's end, its last words on the island as Claude Code's are.
        let stop = json!({ "session_id": "session-c96167f1-eb93", "transcript_path": "", "cwd": "C:\\waku\\agents\\dsh", "hook_event_name": "Stop", "stop_hook_active": false });
        let m = to_message(&stop, &home).unwrap();
        assert_eq!((m["mood"].as_str(), m["reply"].as_str()), (Some("done"), Some("Done — note.txt contained hi, and copy.txt now has the same text.")));
        // Another session's, or none: no words, nothing else changed.
        let mut other = stop.clone();
        other["session_id"] = json!("session-none");
        assert_eq!(to_message(&other, &home).unwrap(), json!({ "mood": "done", "event": "turn-end", "agent": "dsh", "session": "session-none", "project": "dsh" }));
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn the_session_s_name_from_dsh_s_cache() {
        let home = std::env::temp_dir().join(format!("wakuwaku-dsh-{}", std::process::id()));
        let dir = home.join("storages").join("session_projcache").join("sessions");
        std::fs::create_dir_all(&dir).unwrap();
        let cache = json!({ "version": 7, "record": { "rows": { "title": { "ver": 1, "seq": 44, "val": "Read note.txt and copy it" } } } });
        std::fs::write(dir.join("session-c961.json"), cache.to_string()).unwrap();
        assert_eq!(title_of(&home, "session-c961").as_deref(), Some("Read note.txt and copy it"));
        let m = to_message(&event("Stop", json!({})), &home).unwrap();
        assert_eq!(m["title"], "Read note.txt and copy it");
        assert_eq!(title_of(&home, "session-none"), None);
        assert_eq!(title_of(&home, "..\\..\\x"), None);
        let _ = std::fs::remove_dir_all(&home);
    }

    // A log as DSH writes it, a Zstandard frame for each batch of lines.
    fn frames(batches: &[Vec<Value>]) -> Vec<u8> {
        let mut out = Vec::new();
        for batch in batches {
            let text: String = batch.iter().map(|v| format!("{v}\n")).collect();
            out.extend(ruzstd::encoding::compress_to_vec(text.as_bytes(), ruzstd::encoding::CompressionLevel::Fastest));
        }
        out
    }

    // A session's header as the log's first line, as DSH 0.2.0-rc.2 writes
    // it (SessionStore.prepare), with more fields of its own.
    fn header(id: &str, more: Value) -> Value {
        let mut h = json!({ "type": "session", "version": 4, "id": id, "createdAt": 1792100300000u64, "cwd": WORKSPACE });
        for (k, v) in more.as_object().unwrap() {
            h[k] = v.clone();
        }
        h
    }

    const WORKSPACE: &str = "C:\\Users\\waku\\Documents\\deepseek-harness\\default-workspace";
    // The desktop app's session in the test VM (the A6A log): the person's.
    const PARENT: &str = "session-6aa5a96d-33fc-4973-b085-bf5c8f30a6b7";

    #[test]
    fn a_subagent_is_no_session_of_its_own() {
        let home = std::env::temp_dir().join(format!("wakuwaku-dsh-sub-{}", std::process::id()));
        let folder = home.join("sessions").join(project_key(WORKSPACE));
        let put = |id: &str, file: &str, bytes: &[u8]| {
            std::fs::create_dir_all(folder.join(id)).unwrap();
            std::fs::write(folder.join(id).join(file), bytes).unwrap();
        };
        put(PARENT, "session.v4.jsonl.zstd", A6A);
        // A subagent it started, as dsh-subagent's in-process driver makes
        // one (childSessionMeta; its id a bare UUID): its header in a frame
        // of its own, its first turn after it (the task it was handed, as the
        // person's words), as a plugin's idle review starts one (#58).
        let child = "9d1f4c2a-5b7e-4f0a-8c3d-2e6b1a7f9c40";
        let task = "Bounded completed checkpoint (untrusted evidence; some context may be omitted):\n[user/message; checkpoint event 11]";
        let child_log = frames(&[
            vec![header(child, json!({ "parentSession": PARENT, "isSeeded": false, "origin": "subagent", "delegationDepth": 1, "agentPreset": "standard" }))],
            vec![
                json!({ "type": "approval/policy", "seq": 0, "time": 1792100300001u64, "data": { "policy": "never", "source": "delegation" } }),
                json!({ "type": "turn/start", "seq": 1, "time": 1792100300002u64, "data": { "turn": 1 } }),
                json!({ "type": "user/message", "seq": 2, "time": 1792100300003u64, "data": { "content": [{ "type": "text", "text": task }], "source": { "kind": "user" }, "role": "user" } }),
                json!({ "type": "subagent/descriptor", "seq": 3, "time": 1792100300004u64, "data": { "version": 3, "mode": "one-shot", "provider": "spawn" } }),
            ],
        ]);
        put(child, "session.v4.jsonl.zstd", &child_log);
        // Its own subagent, in plain JSONL: the top is still the person's session.
        let grandchild = "0e4b7d21-6c3a-4e9f-b1d8-5a2c9f7e3b16";
        let line = header(grandchild, json!({ "parentSession": child, "isSeeded": false, "origin": "subagent", "delegationDepth": 2 }));
        put(grandchild, "session.v4.jsonl", format!("{line}\n").as_bytes());
        // A session the person forked: a parentSession, no origin: the person's.
        let fork = "session-3c8e2a91-7d4f-4b6a-9e1c-8f2d5b7a0c63";
        put(fork, "session.v4.jsonl.zstd", &frames(&[vec![header(fork, json!({ "parentSession": PARENT, "isSeeded": true, "delegationDepth": 0 }))]]));
        // A subagent whose parent is not named.
        let orphan = "7a2c5e8b-1d4f-4c9a-8b3e-6f0d2a9c5e71";
        put(orphan, "session.v4.jsonl.zstd", &frames(&[vec![header(orphan, json!({ "isSeeded": false, "origin": "subagent", "delegationDepth": 1 }))]]));
        // A header of another session's in this one's folder: none read.
        let stray = "session-5e1a9c7b-2f4d-4a8e-b6c3-9d0f1e7a2b58";
        put(stray, "session.v4.jsonl.zstd", &frames(&[vec![header(child, json!({ "parentSession": PARENT, "origin": "subagent" }))]]));
        // DSH's projection cache names both, the subagent by its task.
        let cache = home.join("storages").join("session_projcache").join("sessions");
        std::fs::create_dir_all(&cache).unwrap();
        let record = |title: &str, subagent: Value| json!({ "version": 7, "record": { "identity": { "formatVersion": 4, "createdAt": 1792100300000u64, "cwd": WORKSPACE, "isSeeded": false, "inheritedEventCount": 0 }, "rows": { "title": { "ver": 1, "seq": 48, "val": title }, "subagent": { "ver": 2, "seq": 3, "val": subagent } } } });
        std::fs::write(cache.join(format!("{PARENT}.json")), record("Create a file", json!({})).to_string()).unwrap();
        std::fs::write(cache.join(format!("{child}.json")), record("Bounded completed checkpoint (untrusted", json!({ "identity": { "mode": "one-shot", "seq": 3 } })).to_string()).unwrap();

        assert_eq!(whose(&home, WORKSPACE, PARENT), Whose::Own);
        assert_eq!(whose(&home, WORKSPACE, child), Whose::Sub(PARENT.into()));
        assert_eq!(whose(&home, WORKSPACE, grandchild), Whose::Sub(PARENT.into()));
        assert_eq!(whose(&home, WORKSPACE, fork), Whose::Own);
        assert_eq!(whose(&home, WORKSPACE, orphan), Whose::Sub(String::new()));
        assert_eq!(whose(&home, WORKSPACE, stray), Whose::Own);
        assert_eq!(whose(&home, WORKSPACE, "session-not-there-yet"), Whose::Own);
        assert_eq!(whose(&home, "", child), Whose::Sub(PARENT.into()), "known once read");

        let of = |id: &str, name: &str, more: Value| {
            let mut e = json!({ "session_id": id, "transcript_path": "", "cwd": WORKSPACE, "hook_event_name": name });
            for (k, v) in more.as_object().unwrap() {
                e[k] = v.clone();
            }
            to_message(&e, &home)
        };
        // Its start, its turn and its end: no line of its own, no name, no
        // task, no reply, no wave; any line it had goes.
        let gone = json!({ "mood": "idle", "event": "session-end", "agent": "dsh", "session": child });
        assert_eq!(of(child, "SessionStart", json!({ "source": "startup" })), Some(gone.clone()));
        assert_eq!(of(child, "UserPromptSubmit", json!({ "prompt": task })), Some(gone.clone()));
        assert_eq!(of(child, "Stop", json!({ "stop_hook_active": false })), Some(gone));
        // Its edits are its session's, within a turn of that one's.
        let write = json!({ "tool_name": "write", "tool_input": { "file_path": "C:\\w\\notes.md" }, "tool_use_id": "call_01", "tool_response": "ok" });
        let m = of(child, "PostToolUse", write.clone()).unwrap();
        assert_eq!(m, json!({ "mood": "working", "detail": "Write", "event": "tool-done", "file": "C:\\w\\notes.md", "toolId": "call_01", "agent": "dsh", "session": PARENT, "project": "default-workspace", "within": true }));
        assert_eq!(of(grandchild, "PostToolUse", write.clone()).unwrap()["session"], PARENT);
        let ask = of(child, "PreToolUse", json!({ "tool_name": "ask_user_question", "tool_input": { "questions": [] } })).unwrap();
        assert_eq!((ask["mood"].as_str(), ask["session"].as_str(), ask["within"].as_bool()), (Some("waiting"), Some(PARENT), Some(true)));
        // Its to-do list is its own; one whose parent is not named, nothing.
        assert_eq!(of(child, "PostToolUse", json!({ "tool_name": "todo_write", "tool_input": { "todos": [] } })), None);
        assert_eq!(of(orphan, "PostToolUse", write), None);
        assert_eq!(of(orphan, "UserPromptSubmit", json!({ "prompt": "x" })).unwrap()["event"], "session-end");
        // The person's sessions as they were: a forked one too.
        let m = of(PARENT, "UserPromptSubmit", json!({ "prompt": "Create a file named a.txt" })).unwrap();
        assert_eq!((m["event"].as_str(), m["task"].as_str(), m["title"].as_str(), m.get("within")), (Some("turn-start"), Some("Create a file named a.txt"), Some("Create a file"), None));
        assert_eq!(of(fork, "SessionStart", json!({ "source": "startup" })).unwrap()["react"], "wave");
        // Her log says whose.
        let e = json!({ "session_id": grandchild, "cwd": WORKSPACE, "hook_event_name": "UserPromptSubmit" });
        assert_eq!(log_line(&e, None, &home).as_deref(), Some("dsh UserPromptSubmit (0e4b7d21): a subagent of 6aa5a96d, not shown"));
        let e = json!({ "session_id": orphan, "cwd": WORKSPACE, "hook_event_name": "Stop" });
        assert_eq!(log_line(&e, None, &home).as_deref(), Some("dsh Stop (7a2c5e8b): a subagent of a session not named, not shown"));
        let _ = std::fs::remove_dir_all(&home);
    }

    // As her plugin sends them (integrations/dsh/wakuwaku.js): what the
    // bridge sent, and what only it sees.
    fn plugin(name: &str, session: &str, more: Value) -> Value {
        let mut e = json!({ "session_id": session, "transcript_path": "", "cwd": WORKSPACE, "hook_event_name": name, "origin": "", "parent_session": "" });
        for (k, v) in more.as_object().unwrap() {
            e[k] = v.clone();
        }
        e
    }

    #[test]
    fn what_her_plugin_tells() {
        let home = std::env::temp_dir().join(format!("wakuwaku-dsh-plugin-ev-{}", std::process::id()));
        let folder = home.join("sessions").join(project_key(WORKSPACE));
        std::fs::create_dir_all(folder.join(PARENT)).unwrap();
        std::fs::write(folder.join(PARENT).join("session.v4.jsonl.zstd"), A6A).unwrap();
        // Its last words and its name come with it: neither the log nor the
        // projection cache is read (the log's last words differ).
        let stop = plugin("Stop", PARENT, json!({ "stop_hook_active": false, "last_assistant_message": "All done.", "session_title": "Create a file" }));
        let m = to_message(&stop, &home).unwrap();
        assert_eq!((m["mood"].as_str(), m["reply"].as_str(), m["title"].as_str()), (Some("done"), Some("All done."), Some("Create a file")));
        // A turn with no words: none, not the log's.
        let m = to_message(&plugin("Stop", PARENT, json!({ "last_assistant_message": "" })), &home).unwrap();
        assert_eq!(m.get("reply"), None);
        // Stopped by the person: there again, not done; a turn that failed: an error.
        let m = to_message(&plugin("Stop", PARENT, json!({ "stop_reason": "aborted", "last_assistant_message": "" })), &home).unwrap();
        assert_eq!(m, json!({ "mood": "idle", "agent": "dsh", "session": PARENT, "project": "default-workspace" }));
        let m = to_message(&plugin("StopFailure", PARENT, json!({ "error_details": "401 Unauthorized: invalid api key" })), &home).unwrap();
        assert_eq!((m["mood"].as_str(), m["error"].as_str()), (Some("error"), Some("401 Unauthorized: invalid api key")));
        // A tool that failed, said so.
        let m = to_message(&plugin("PostToolUse", PARENT, json!({ "tool_name": "pwsh", "tool_input": { "command": "exit 1" }, "tool_use_id": "c1", "tool_response": "exit code 1", "is_error": true })), &home).unwrap();
        assert_eq!(m["event"], "tool-failed");
        // Every tool's step, not only the few the bridge had.
        let m = to_message(&plugin("PreToolUse", PARENT, json!({ "tool_name": "grep", "tool_input": { "pattern": "TODO" }, "tool_use_id": "c2" })), &home).unwrap();
        assert_eq!((m["event"].as_str(), m["detail"].as_str()), (Some("tool-start"), Some("Grep")));

        // Whose, as it says: a subagent's, the top of its lineage given; the
        // person's (origin ""), with no log read at all.
        let sub = plugin("PostToolUse", "9d1f-child", json!({ "origin": "subagent", "parent_session": "a1-mid", "top_session": PARENT, "tool_name": "write", "tool_input": { "file_path": "C:\\w\\n.md" }, "tool_use_id": "c3", "tool_response": "ok", "session_title": "List all files in the" }));
        assert_eq!(whose_of(&sub, &home), Whose::Sub(PARENT.into()));
        let m = to_message(&sub, &home).unwrap();
        assert_eq!((m["session"].as_str(), m["within"].as_bool(), m["file"].as_str()), (Some(PARENT), Some(true), Some("C:\\w\\n.md")));
        // Its own name (of its task) does not rename the session it works for
        // (seen in the test VM, 0.2.0-rc.2: the line read "List all files in the").
        assert_eq!(m.get("title"), None);
        let mut start = sub.clone();
        start["hook_event_name"] = json!("UserPromptSubmit");
        assert_eq!(to_message(&start, &home).unwrap()["event"], "session-end");
        let mut failed = sub.clone();
        failed["hook_event_name"] = json!("StopFailure");
        assert_eq!(to_message(&failed, &home).unwrap()["event"], "session-end");
        // Its parent alone (not live in DSH): followed up by the logs (PARENT's own: the person's).
        let mut parent_only = sub.clone();
        parent_only["top_session"] = json!("");
        parent_only["parent_session"] = json!(PARENT);
        assert_eq!(whose_of(&parent_only, &home), Whose::Sub(PARENT.into()));
        parent_only["parent_session"] = json!("");
        assert_eq!(whose_of(&parent_only, &home), Whose::Sub(String::new()));
        assert_eq!(whose_of(&plugin("Stop", "session-not-there", json!({})), &home), Whose::Own);

        // A prompt: waiting, what for (as Claude Code's), answered on her
        // island when she holds it, else in DSH; what came of it lets the
        // session work on, only within a turn under way.
        let asked = plugin("PermissionRequest", PARENT, json!({ "tool_name": "pwsh", "tool_input": { "command": "Get-Content -Path C:\\Windows\\win.ini", "description": "" }, "tool_use_id": "c4", "permission_id": "9-1", "answerable": true }));
        let m = to_message(&asked, &home).unwrap();
        assert_eq!((m["mood"].as_str(), m["detail"]["key"].as_str(), m["detail"]["vars"]["what"]["vars"]["what"].as_str()), (Some("waiting"), Some("detail.approve"), Some("Get-Content -Path C:\\Windows\\win.ini")));
        let m = not_held(&asked, &home);
        assert_eq!((m["detail"]["key"].as_str(), m["detail"]["vars"]["app"].as_str()), (Some("detail.approveIn"), Some("DeepSeek Harness")));
        assert_eq!(m["detail"]["vars"]["what"]["key"], "step.command");
        let write = plugin("PermissionRequest", PARENT, json!({ "tool_name": "write", "tool_input": { "file_path": "C:\\waku\\note.txt" }, "answerable": false }));
        assert_eq!(to_message(&write, &home).unwrap()["detail"]["vars"]["what"], json!({ "key": "step.write", "vars": { "file": "note.txt" } }));
        assert_eq!(log_line(&write, None, &home).as_deref(), Some("dsh PermissionRequest (6aa5a96d): her plugin cannot take an answer to it, told only"));
        // A question: waiting with it, said so whoever answers.
        let question = plugin("PermissionRequest", PARENT, json!({ "tool_name": "ask_user_question", "tool_input": { "questions": [] }, "answerable": true }));
        assert_eq!(not_held(&question, &home)["detail"], json!({ "key": "detail.asksQuestion" }));
        let replied = plugin("PermissionReplied", PARENT, json!({ "permission_id": "9-1", "tool_use_id": "c4", "outcome": "allowed-once", "by": "her" }));
        assert_eq!(to_message(&replied, &home).unwrap(), json!({ "mood": "working", "agent": "dsh", "within": true, "session": PARENT }));
        assert_eq!(log_line(&replied, None, &home).as_deref(), Some("dsh PermissionReplied (6aa5a96d): allowed-once, on her island"));

        // Its word as DSH loads it: nothing for the pet, a line for her log.
        let loaded = json!({ "session_id": "", "hook_event_name": "WakuwakuLoaded", "version": 1, "pid": 4321, "profile": "desktop" });
        assert!(to_message(&loaded, &home).is_none());
        assert_eq!(log_line(&loaded, None, &home).as_deref(), Some("dsh: her plugin loaded (version 1, profile desktop, pid 4321)"));
        assert!(is_loaded(&loaded) && !is_loaded(&json!({ "hook_event_name": "Stop" })));
        let _ = std::fs::remove_dir_all(&home);
    }
}
