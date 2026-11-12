// What she makes of the agents' events that have nothing else of hers to
// them (events.rs message_of, as agent-core reads them): Codex's, omp's,
// OpenCode's, Reasonix's and ZCode's, each in a module of its own. Claude
// Code's are events.rs's; Antigravity's and DeepSeek Harness's are with
// their log lines (events_agy.rs, events_dsh.rs).

// Held by each test that reads Reasonix's events (the reasonix module here,
// replay.rs): agent-core keeps when each session last asked in one table for
// the whole program (reasonix.rs ASKED), each ask dropping those 30 s older
// than its own time, so a test asking at the time now would drop what one at
// a fixture's time had just asked, if they ran side by side. A plain Mutex,
// not a lock.rs Lock: held through the whole test, while the code it tests
// takes her locks.
pub(crate) fn reasonix_asks_held() -> std::sync::MutexGuard<'static, ()> {
    static HELD: std::sync::Mutex<()> = std::sync::Mutex::new(());
    HELD.lock().unwrap_or_else(|e| e.into_inner())
}

// Codex's hook events: agent-core reads them (codex.rs to_event), events.rs
// tells them in her words. Here, what she makes of them.
mod codex {
    use serde_json::{json, Value};

    fn to_message(e: &Value) -> Option<Value> {
        agent_core::codex::to_event(e).map(|ev| crate::events::message_of(&ev))
    }

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

    // The desktop app's own request (a suggested next message) is no task of
    // the person's: its session gone, its ending nothing; with an image, the
    // person's words after the app's headings are the task (#74).
    #[test]
    fn codex_desktop_asking_of_its_own_accord() {
        let mine = |name: &str, turn: &str, more: Value| {
            let mut e = event(name, more);
            e["session_id"] = json!("desk-74");
            e["turn_id"] = json!(turn);
            to_message(&e)
        };
        let ask = "This is an app-generated request for a suggested next user message. Reply with one short message.";
        assert_eq!(mine("UserPromptSubmit", "a1", json!({ "prompt": ask })).unwrap()["event"], "session-end");
        assert!(mine("Stop", "a1", json!({ "last_assistant_message": "Thanks!" })).is_none());
        let image = "# Files mentioned by the user:\n\n## shot.png: C:\\w\\shot.png\nImage attachment: true\n\n## My request for Codex:\nwhy is this button grey?\n";
        assert_eq!(mine("UserPromptSubmit", "p1", json!({ "prompt": image })).unwrap()["task"], "why is this button grey?");
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

// oh-my-pi's events, from her extension inside omp (integrations/omp/
// wakuwaku.ts, written by connection/omp.rs): agent-core reads them (omp.rs
// to_event), events.rs tells them in her words (an omp call's step,
// step_of_omp: ours for the tools Claude Code has, else the model's intent).
// Here, what she makes of them.
mod omp {
    use serde_json::{json, Value};

    fn to_message(e: &Value) -> Option<Value> {
        agent_core::omp::to_event(e).map(|ev| crate::events::message_of(&ev))
    }

    // As the extension sends them from omp 18.11.1 (trimmed); the folder is no git repository.
    fn event(name: &str, more: Value) -> Value {
        let mut e = json!({ "session_id": "01a14146", "cwd": "Z:\\nowhere\\proj", "hook_event_name": name });
        for (k, v) in more.as_object().unwrap() {
            e[k] = v.clone();
        }
        e
    }

    fn tool(name: &str, args: Value, intent: &str) -> Value {
        to_message(&event("PreToolUse", json!({ "tool_name": name, "tool_input": args, "tool_use_id": "call_1", "intent": intent }))).unwrap()
    }

    #[test]
    fn omp_events_become_messages() {
        let m = to_message(&event("SessionStart", json!({ "source": "startup" }))).unwrap();
        assert_eq!(m, json!({ "mood": "idle", "react": "wave", "say": { "key": "say.hello" }, "session": "01a14146", "project": "proj", "agent": "omp" }));
        let m = to_message(&event("UserPromptSubmit", json!({ "prompt": "Run the shell command echo hi" }))).unwrap();
        assert_eq!((m["mood"].as_str(), m["event"].as_str(), m["task"].as_str()), (Some("working"), Some("turn-start"), Some("Run the shell command echo hi")));
        let m = tool("bash", json!({ "command": "echo hi", "timeout": 30, "cwd": "C:/waku" }), "Running echo hi");
        assert_eq!(
            m,
            json!({ "mood": "working", "detail": "Bash", "event": "tool-start", "step": { "key": "step.run", "vars": { "what": "Running echo hi" } },
                    "toolId": "call_1", "session": "01a14146", "project": "proj", "agent": "omp" })
        );
        let m = to_message(&event("Stop", json!({ "last_assistant_message": "Done.\nMore." }))).unwrap();
        assert_eq!((m["mood"].as_str(), m["reply"].as_str(), m["agent"].as_str()), (Some("done"), Some("Done. More."), Some("omp")));
        let m = to_message(&event("StopFailure", json!({ "error": "", "error_details": "Connection error.\nretry later" }))).unwrap();
        assert_eq!((m["mood"].as_str(), m["error"].as_str()), (Some("error"), Some("Connection error.")));
        let m = to_message(&event("Interrupt", json!({}))).unwrap();
        assert_eq!((m["mood"].as_str(), m["event"].as_str()), (Some("idle"), Some("turn-end")));
        assert_eq!(to_message(&event("SessionEnd", json!({ "reason": "exit" }))).unwrap()["event"], "session-end");
        let m = to_message(&event("Stop", json!({ "session_title": "Echo a greeting" }))).unwrap();
        assert_eq!(m["title"], "Echo a greeting");
    }

    #[test]
    fn omp_tools_say_what_they_do() {
        assert_eq!(tool("read", json!({ "path": "src/a.txt" }), "Reading a.txt")["step"], json!({ "key": "step.read", "vars": { "file": "a.txt" } }));
        assert_eq!(tool("write", json!({ "path": "a.txt", "content": "hi\n" }), "")["step"], json!({ "key": "step.write", "vars": { "file": "a.txt" } }));
        let m = tool("edit", json!({ "path": "a.txt", "old_string": "hi", "new_string": "bye" }), "Changing hi to bye");
        assert_eq!((m["detail"].as_str(), &m["step"]), (Some("Edit"), &json!({ "key": "step.edit", "vars": { "file": "a.txt" } })));
        assert_eq!(tool("grep", json!({ "pattern": "bye", "path": "." }), "")["step"], json!({ "key": "step.search", "vars": { "what": "bye" } }));
        assert_eq!(tool("glob", json!({ "path": "*.txt" }), "Listing *.txt files")["step"], json!({ "key": "step.find", "vars": { "what": "*.txt" } }));
        assert_eq!(tool("bash", json!({ "command": "cd x && cargo test" }), "")["step"], json!({ "key": "step.command", "vars": { "what": "cargo test" } }));
        let task = json!({ "context": "…", "tasks": [{ "name": "SayHi", "agent": "sonic", "task": "Reply hi" }] });
        assert_eq!(tool("task", task.clone(), "Starting one subagent to reply hi")["step"], json!({ "key": "step.agent", "vars": { "what": "Starting one subagent to reply hi" } }));
        assert_eq!(tool("task", task, "")["step"]["vars"]["what"], "SayHi");
        assert_eq!(tool("web_search", json!({ "query": "omp extensions" }), "")["step"], json!({ "key": "step.webSearch", "vars": { "what": "omp extensions" } }));
        assert_eq!(tool("todo", json!({ "ops": [] }), "")["step"]["key"], "step.plan");
        // Its own tools: the model's words for them, else the name.
        assert_eq!(tool("lsp", json!({ "action": "hover" }), "Checking the type")["step"], json!({ "key": "step.run", "vars": { "what": "Checking the type" } }));
        assert_eq!(tool("lsp", json!({}), "")["step"], json!({ "key": "step.tool", "vars": { "tool": "lsp" } }));
        assert_eq!(tool("mcp__github__get_issue", json!({}), "Reading the issue")["step"], json!({ "key": "step.mcp", "vars": { "server": "github", "tool": "get_issue" } }));
        // One of ours with nothing to name: its intent.
        assert_eq!(tool("edit", json!({ "ops": [] }), "Fixing the import")["step"], json!({ "key": "step.run", "vars": { "what": "Fixing the import" } }));
        // A question waits on the person.
        assert_eq!(tool("ask", json!({ "questions": [] }), "")["mood"], "waiting");
    }

    #[test]
    fn an_edit_brings_its_file_and_a_failure_says_so() {
        let done = json!({ "tool_name": "edit", "tool_input": { "path": "a.txt" }, "tool_use_id": "c2", "file": "C:\\waku\\omp\\proj\\a.txt" });
        let m = to_message(&event("PostToolUse", done)).unwrap();
        assert_eq!((m["detail"].as_str(), m["event"].as_str(), m["file"].as_str(), m["toolId"].as_str()), (Some("Edit"), Some("tool-done"), Some("C:\\waku\\omp\\proj\\a.txt"), Some("c2")));
        // A read's path is no edit.
        let m = to_message(&event("PostToolUse", json!({ "tool_name": "read", "tool_input": { "path": "a.txt" }, "file": "C:\\a.txt" }))).unwrap();
        assert!(m.get("file").is_none());
        let m = to_message(&event("PostToolUseFailure", json!({ "tool_name": "bash", "tool_input": { "command": "false" }, "error": "exit 1" }))).unwrap();
        assert_eq!((m["event"].as_str(), m["react"].as_str(), &m["say"]["vars"]["tool"]), (Some("tool-failed"), Some("failed"), &json!("Bash")));
        // The edit made the turn's done a review.
        let mut pet = crate::state::Pet::default();
        for (e, at) in [(event("UserPromptSubmit", json!({ "prompt": "fix it" })), 1), (event("PostToolUse", json!({ "tool_name": "write", "tool_input": { "path": "b.txt" } })), 2), (event("Stop", json!({})), 3)] {
            pet.apply(&agent_core::omp::to_event(&e).unwrap(), None, at, crate::state::Hold::Seen);
        }
        assert_eq!((pet.get(4)["mood"].as_str(), pet.get(4)["agent"].as_str()), (Some("review"), Some("omp")));
    }

    #[test]
    fn approvals_wait_in_omp_and_plans_on_her() {
        let m = to_message(&event("ApprovalRequested", json!({ "tool_name": "bash", "tool_input": { "command": "rm -rf build" }, "intent": "" }))).unwrap();
        assert_eq!(m["mood"], "waiting");
        assert_eq!(m["detail"], json!({ "key": "detail.approveInTerminal", "vars": { "what": { "key": "step.command", "vars": { "what": "rm -rf build" } } } }));
        assert_eq!(to_message(&event("ApprovalResolved", json!({ "approved": true }))).unwrap()["mood"], "working");
        // A plan review: waiting on her, as Claude Code's ExitPlanMode.
        let plan = json!({ "tool_name": "ExitPlanMode", "tool_input": { "plan": "1. Write a.txt" }, "tool_use_id": "r1" });
        let m = to_message(&event("PermissionRequest", plan.clone())).unwrap();
        assert_eq!((m["mood"].as_str(), &m["detail"]), (Some("waiting"), &json!({ "key": "detail.planToConfirm" })));
        let mut answered = plan.clone();
        answered["choice"] = json!("execute");
        assert_eq!(to_message(&event("PostToolUse", answered.clone())).unwrap()["mood"], "working");
        answered["choice"] = json!("save");
        assert_eq!(to_message(&event("PostToolUse", answered.clone())).unwrap()["mood"], "idle");
        answered["choice"] = json!("");
        assert_eq!(to_message(&event("PostToolUse", answered)).unwrap()["mood"], "idle");
    }

    #[test]
    fn omp_events_that_change_nothing() {
        for name in ["Nope", ""] {
            assert!(to_message(&event(name, json!({}))).is_none(), "{name}");
        }
        assert!(to_message(&json!("nope")).is_none());
    }
}

// OpenCode's events, from her plugin in OpenCode's background service
// (integrations/opencode/wakuwaku.js, installed by connection/opencode.rs): agent-core
// reads them (opencode.rs: normalize, by the names Claude Code's hooks have,
// for the prompts and the jump too; to_event), events.rs tells them in her
// words. Here, what she makes of them.
mod opencode {
    use serde_json::{json, Value};

    use agent_core::opencode::normalize;

    fn to_message(e: &Value) -> Option<Value> {
        agent_core::opencode::to_event(e).map(|ev| crate::events::message_of(&ev))
    }

    // As her plugin sends them from OpenCode 2.0.29 (trimmed); the folder is no git repository.
    fn event(kind: &str, more: Value) -> Value {
        let mut e = json!({ "type": kind, "sessionID": "ses_1", "directory": "Z:\\nowhere\\oc3", "title": "Shell echo command" });
        for (k, v) in more.as_object().unwrap() {
            e[k] = v.clone();
        }
        e
    }

    fn message(kind: &str, more: Value) -> Option<Value> {
        to_message(&normalize(&event(kind, more)))
    }

    #[test]
    fn opencode_events_become_messages() {
        let m = message("session.created", json!({})).unwrap();
        assert_eq!(
            m,
            json!({ "mood": "idle", "react": "wave", "say": { "key": "say.hello" }, "agent": "opencode", "session": "ses_1", "project": "oc3", "title": "Shell echo command" })
        );
        let m = message("session.execution.started", json!({ "prompt": "Run the shell command: echo hi" })).unwrap();
        assert_eq!((m["event"].as_str(), m["task"].as_str()), (Some("turn-start"), Some("Run the shell command: echo hi")));
        let call = json!({ "id": "call_00_E9yQ", "tool": "shell", "input": { "command": "cd x && echo hi" } });
        let m = message("session.tool.called", call.clone()).unwrap();
        assert_eq!(
            (m["detail"].as_str(), m["event"].as_str(), m["toolId"].as_str(), &m["step"]),
            (Some("Bash"), Some("tool-start"), Some("call_00_E9yQ"), &json!({ "key": "step.command", "vars": { "what": "echo hi" } }))
        );
        let m = message("session.tool.success", call.clone()).unwrap();
        assert_eq!((m["event"].as_str(), m["toolId"].as_str()), (Some("tool-done"), Some("call_00_E9yQ")));
        let m = message("session.tool.failed", json!({ "tool": "shell", "input": {}, "error": { "type": "tool.execution", "message": "exit 1" } })).unwrap();
        assert_eq!((m["event"].as_str(), m["react"].as_str(), &m["say"]["vars"]["tool"]), (Some("tool-failed"), Some("failed"), &json!("shell")));
        let m = message("session.execution.succeeded", json!({ "text": "Done — the command output `hi`." })).unwrap();
        assert_eq!((m["mood"].as_str(), m["reply"].as_str()), (Some("done"), Some("Done — the command output hi.")));
        let m = message("session.execution.failed", json!({ "error": { "type": "provider.rate-limit", "message": "429" } })).unwrap();
        assert_eq!((m["mood"].as_str(), &m["error"]), (Some("error"), &json!({ "key": "error.rate_limit", "vars": {} })));
        let m = message("session.execution.failed", json!({ "error": { "type": "unknown", "message": "Something broke\nmore" } })).unwrap();
        assert_eq!(m["error"], "Something broke");
        let m = message("session.execution.interrupted", json!({ "reason": "user" })).unwrap();
        assert_eq!((m["mood"].as_str(), m["event"].as_str()), (Some("idle"), Some("turn-end")));
        assert_eq!(message("session.deleted", json!({})).unwrap()["event"], "session-end");
        assert_eq!(message("form.created", json!({})).unwrap()["mood"], "waiting");
        assert_eq!(message("form.replied", json!({})).unwrap()["mood"], "working");
    }

    #[test]
    fn edits_name_their_files() {
        let m = message("session.tool.success", json!({ "tool": "edit", "input": { "path": "Z:\\p\\src\\a.rs", "oldString": "a", "newString": "b" } })).unwrap();
        assert_eq!((m["detail"].as_str(), m["file"].as_str()), (Some("Edit"), Some("Z:\\p\\src\\a.rs")));
        let patch = "*** Begin Patch\n*** Add File: hello.txt\n+hi\n*** Update File: b.txt\n@@\n-a\n+b\n*** End Patch";
        let m = message("session.tool.called", json!({ "tool": "patch", "input": { "patchText": patch } })).unwrap();
        assert_eq!(m["step"], json!({ "key": "step.edit", "vars": { "file": "hello.txt +1" } }));
        let m = message("session.tool.success", json!({ "tool": "patch", "input": { "patchText": patch } })).unwrap();
        // Its first file by its whole path (agent-core keeps every one: zhoupengjie/agent-hub#2).
        assert_eq!((m["detail"].as_str(), m["file"].as_str()), (Some("apply_patch"), Some("Z:\\nowhere\\oc3\\hello.txt")));
        let m = message("session.tool.called", json!({ "tool": "read", "input": { "path": "/x/notes.md" } })).unwrap();
        assert_eq!(m["step"], json!({ "key": "step.read", "vars": { "file": "notes.md" } }));
        // A tool of its own (an MCP server's): by its name.
        let m = message("session.tool.called", json!({ "tool": "github_create_issue", "input": {} })).unwrap();
        assert_eq!(m["step"], json!({ "key": "step.tool", "vars": { "tool": "github_create_issue" } }));
    }

    #[test]
    fn a_permission_and_its_reply() {
        let asked = normalize(&event(
            "permission.asked",
            json!({ "id": "per_1", "action": "shell", "resources": ["echo hi"], "save": ["echo *"], "source": { "type": "tool", "id": "call_1" }, "tool": "shell", "input": { "command": "echo hi" } }),
        ));
        assert_eq!(
            (asked["hook_event_name"].as_str(), asked["tool_name"].as_str(), asked["permission_id"].as_str(), asked["tool_use_id"].as_str()),
            (Some("PermissionRequest"), Some("shell"), Some("per_1"), Some("call_1"))
        );
        assert_eq!(asked["tool_input"], json!({ "command": "echo hi", "description": "" }));
        let m = to_message(&asked).unwrap();
        assert_eq!(m["detail"], json!({ "key": "detail.approve", "vars": { "what": { "key": "step.command", "vars": { "what": "echo hi" } } } }));
        // One not of a call (a folder outside the project): by its action.
        let m = message("permission.asked", json!({ "id": "per_2", "action": "external_directory", "resources": ["C:\\elsewhere\\*"] })).unwrap();
        assert_eq!(m["detail"]["vars"]["what"], json!({ "key": "step.tool", "vars": { "tool": "external_directory" } }));
        let replied = normalize(&event("permission.replied", json!({ "requestID": "per_1", "reply": "once" })));
        assert_eq!((replied["hook_event_name"].as_str(), replied["permission_id"].as_str()), (Some("PermissionReplied"), Some("per_1")));
        assert_eq!(to_message(&replied).unwrap()["mood"], "working");
        // Turned down: no failure of the tool's, and the turn it ends is stopped.
        let m = message("session.tool.failed", json!({ "tool": "shell", "error": { "type": "aborted", "message": "The user declined this tool call" } })).unwrap();
        assert_eq!((m["event"].as_str(), m.get("react")), (Some("tool-done"), None));
        let m = message("session.tool.failed", json!({ "tool": "shell", "error": { "type": "permission.rejected", "message": "no" } })).unwrap();
        assert_eq!((m["event"].as_str(), m.get("react")), (Some("tool-done"), None));
        let m = message("session.execution.failed", json!({ "error": { "type": "permission.rejected", "message": "no" } })).unwrap();
        assert_eq!((m["mood"].as_str(), m["event"].as_str()), (Some("idle"), Some("turn-end")));
    }

    #[test]
    fn opencode_events_that_change_nothing() {
        for kind in ["session.renamed", "session.step.started", "message.updated", "", "session.execution.interrupted"] {
            let reason = if kind == "session.execution.interrupted" { json!({ "reason": "superseded" }) } else { json!({}) };
            assert!(normalize(&event(kind, reason)).is_null(), "{kind}");
        }
        assert!(to_message(&Value::Null).is_none());
    }

    // Each event as the server takes it, applied at a time (ms): the turns it
    // began and the time it counted.
    #[test]
    fn an_opencode_round_counts_its_turn_and_its_time() {
        let mut pet = crate::state::Pet::default();
        let mut feed = |kind: &str, more: Value, at: u64| {
            agent_core::opencode::to_event(&normalize(&event(kind, more))).and_then(|ev| pet.apply(&ev, None, at, crate::state::Hold::Seen)).map_or((0, 0), |out| (out.turns, out.worked_ms))
        };
        assert_eq!(feed("session.created", json!({}), 0), (0, 0));
        assert_eq!(feed("session.execution.started", json!({ "prompt": "Run the shell command: echo hi" }), 1_000), (1, 0));
        assert_eq!(feed("session.tool.called", json!({ "id": "c1", "tool": "shell", "input": { "command": "echo hi" } }), 2_000), (0, 0));
        assert_eq!(feed("permission.asked", json!({ "id": "per_1", "action": "shell", "resources": ["echo hi"], "tool": "shell" }), 2_100), (0, 0));
        assert_eq!(feed("permission.replied", json!({ "requestID": "per_1", "reply": "once" }), 9_000), (0, 0));
        assert_eq!(feed("session.tool.success", json!({ "id": "c1", "tool": "shell", "input": { "command": "echo hi" } }), 9_300), (0, 0));
        assert_eq!(feed("session.execution.succeeded", json!({ "text": "Done." }), 10_000), (0, 9_000));
    }
}

// Reasonix's hook events (zhoupengjie/Wakuwaku#60), handed to her port by
// this program with connection::REASONIX_FLAG (hooks.rs reasonix_hook):
// agent-core reads them (reasonix.rs to_event_at: in Claude Code's words, a
// subagent's steps as its session's, a failure told once, its approvals
// answered in Reasonix), events.rs tells them in her words. Here, what she
// makes of them.
mod reasonix {
    use serde_json::{json, Value};

    use agent_core::reasonix::SAME_ASK_MS;

    fn to_message_at(e: &Value, now: u64) -> Option<Value> {
        agent_core::reasonix::to_event_at(e, now).map(|ev| crate::events::message_of(&ev))
    }

    fn to_message(e: &Value) -> Option<Value> {
        to_message_at(e, crate::now_ms())
    }

    // As Reasonix's hook package writes them (its Payload), in a folder that
    // is no git repository.
    fn event(name: &str, more: Value) -> Value {
        let mut e = json!({ "event": name, "sessionId": "rx-7f3a", "cwd": "Z:\\nowhere\\rx" });
        for (k, v) in more.as_object().unwrap() {
            e[k] = v.clone();
        }
        e
    }

    #[test]
    fn reasonix_events_become_messages() {
        let _asks = super::reasonix_asks_held();
        let m = to_message(&event("SessionStart", json!({ "source": "startup" }))).unwrap();
        assert_eq!(m, json!({ "mood": "idle", "react": "wave", "say": { "key": "say.hello" }, "agent": "reasonix", "session": "rx-7f3a", "project": "rx" }));
        let m = to_message(&event("UserPromptSubmit", json!({ "prompt": "Create a file named note.txt containing hi", "turn": 1 }))).unwrap();
        assert_eq!((m["mood"].as_str(), m["event"].as_str(), m["task"].as_str()), (Some("working"), Some("turn-start"), Some("Create a file named note.txt containing hi")));
        // Its tools by Claude Code's names, their arguments too: the step says the file.
        let write = json!({ "path": "note.txt", "content": "hi\n" });
        let m = to_message(&event("PreToolUse", json!({ "toolName": "write_file", "toolArgs": write }))).unwrap();
        assert_eq!((m["mood"].as_str(), m["detail"].as_str()), (Some("working"), Some("Write")));
        assert_eq!(m["step"], json!({ "key": "step.write", "vars": { "file": "note.txt" } }));
        // Arguments as a string of JSON read the same.
        let m = to_message(&event("PreToolUse", json!({ "toolName": "edit_file", "toolArgs": "{\"path\":\"src\\\\a.rs\"}" }))).unwrap();
        assert_eq!(m["step"], json!({ "key": "step.edit", "vars": { "file": "a.rs" } }));
        let m = to_message(&event("PreToolUse", json!({ "toolName": "bash", "toolArgs": { "command": "cd x && cargo test" } }))).unwrap();
        assert_eq!(m["step"], json!({ "key": "step.command", "vars": { "what": "cargo test" } }));
        // The file it changed, made whole from the session's folder; one whole already as it is.
        let m = to_message(&event("PostToolUse", json!({ "toolName": "write_file", "toolArgs": write, "toolResult": "wrote 3 bytes" }))).unwrap();
        let whole = std::path::Path::new("Z:\\nowhere\\rx").join("note.txt").to_string_lossy().into_owned();
        assert_eq!((m["event"].as_str(), m["file"].as_str()), (Some("tool-done"), Some(whole.as_str())));
        if cfg!(windows) {
            let m = to_message(&event("PostToolUse", json!({ "toolName": "edit_file", "toolArgs": { "path": "D:\\w\\b.rs" } }))).unwrap();
            assert_eq!(m["file"], "D:\\w\\b.rs");
        }
        let m = to_message(&event("PostToolUseFailure", json!({ "toolName": "bash", "toolArgs": { "command": "false" }, "error": "exit status 1" }))).unwrap();
        assert_eq!((m["event"].as_str(), m["react"].as_str()), (Some("tool-failed"), Some("failed")));
        // A subagent's starter, by any of its names.
        for tool in ["task", "explore", "parallel_tasks"] {
            let m = to_message(&event("PreToolUse", json!({ "toolName": tool, "toolArgs": { "description": "Explore the code" } }))).unwrap();
            assert_eq!(m["step"], json!({ "key": "step.agent", "vars": { "what": "Explore the code" } }), "{tool}");
        }
        // Its question to the person, and its to-do list.
        assert_eq!(to_message(&event("PreToolUse", json!({ "toolName": "ask", "toolArgs": { "questions": [] } }))).unwrap()["mood"], "waiting");
        let todo = to_message(&event("PreToolUse", json!({ "toolName": "todo_write", "toolArgs": { "todos": [{ "content": "Run tests", "activeForm": "Running tests", "status": "in_progress" }] } }))).unwrap();
        assert_eq!(todo["todo"], json!({ "set": [{ "text": "Run tests", "active": "Running tests", "status": "in_progress" }] }));
        // A tool of its own, and an MCP one.
        assert_eq!(to_message(&event("PreToolUse", json!({ "toolName": "code_index" }))).unwrap()["step"], json!({ "key": "step.tool", "vars": { "tool": "code_index" } }));
        assert_eq!(to_message(&event("PreToolUse", json!({ "toolName": "mcp__github__create_issue" }))).unwrap()["step"]["key"], "step.mcp");
        // A prompt for its approval: waiting, answered in Reasonix.
        let m = to_message(&event("PermissionRequest", json!({ "toolName": "bash", "toolArgs": { "command": "git push" } }))).unwrap();
        assert_eq!(m["mood"], "waiting");
        assert_eq!(m["detail"], json!({ "key": "detail.approveIn", "vars": { "app": "Reasonix", "what": { "key": "step.command", "vars": { "what": "git push" } } } }));
        // Its end, with its last words; a failure, with why.
        let m = to_message(&event("Stop", json!({ "lastAssistantText": "## Done\nCreated **note.txt** with `hi`.", "turn": 1 }))).unwrap();
        assert_eq!((m["mood"].as_str(), m["reply"].as_str(), m["agent"].as_str()), (Some("done"), Some("Done: Created note.txt with hi."), Some("reasonix")));
        let m = to_message(&event("StopFailure", json!({ "error": "deepseek: 402 Insufficient Balance" }))).unwrap();
        assert_eq!((m["mood"].as_str(), m["error"].as_str()), (Some("error"), Some("deepseek: 402 Insufficient Balance")));
        let m = to_message(&event("SessionEnd", json!({ "reason": "exit" }))).unwrap();
        assert_eq!((m["mood"].as_str(), m["event"].as_str()), (Some("idle"), Some("session-end")));
    }

    #[test]
    fn the_person_s_words_past_reasonix_s_own_blocks() {
        let _asks = super::reasonix_asks_held();
        // As Reasonix 2.44 composes it (trimmed): its blocks, then the person's words.
        // (agent-core's reasonix.rs has the rest of how they are found.)
        let prompt = "<current-date>\nToday's date is 2026-10-18 (Sunday); the local time zone is UTC+02:00.\n</current-date>\n\n<available-skills>\n# Skills\n- explore [🧬 subagent] — Explore…\n</available-skills>\n\nList the files in this folder and reply in one short sentence.";
        let m = to_message(&event("UserPromptSubmit", json!({ "prompt": prompt, "turn": 1 }))).unwrap();
        assert_eq!(m["task"], "List the files in this folder and reply in one short sentence.");
    }

    #[test]
    fn a_failure_is_told_once_and_a_stop_by_the_person_is_no_failure() {
        let _asks = super::reasonix_asks_held();
        // A failed turn: StopFailure, then its Stop hooks again with the error.
        let m = to_message(&event("StopFailure", json!({ "error": "context deadline exceeded", "turn": 2 }))).unwrap();
        assert_eq!(m["mood"], "error");
        assert!(to_message(&event("Stop", json!({ "error": "context deadline exceeded", "turn": 2 }))).is_none());
        // A failed tool: PostToolUseFailure, then PostToolUse again with the error.
        let failed = json!({ "toolName": "edit_file", "toolArgs": { "path": "a.rs" }, "error": "old_string not found" });
        assert_eq!(to_message(&event("PostToolUseFailure", failed.clone())).unwrap()["event"], "tool-failed");
        assert!(to_message(&event("PostToolUse", failed)).is_none());
        // The person stopping the turn: at rest, no error; its Stop again changes nothing.
        let stopped = json!({ "error": "context canceled", "isInterrupt": true });
        let m = to_message(&event("StopFailure", stopped.clone())).unwrap();
        assert_eq!((m["mood"].as_str(), m["event"].as_str(), m.get("error")), (Some("idle"), Some("turn-end"), None));
        assert!(to_message(&event("Stop", stopped)).is_none());
    }

    #[test]
    fn subagents_work_for_their_session() {
        let _asks = super::reasonix_asks_held();
        // A subagent's step: the session's, within a turn of it.
        let m = to_message(&event("PreToolUse", json!({ "sessionId": "rx-7f3a:subagent:call_7", "toolName": "grep", "toolArgs": { "pattern": "TODO" } }))).unwrap();
        assert_eq!((m["session"].as_str(), m["within"].as_bool(), m["mood"].as_str()), (Some("rx-7f3a"), Some(true), Some("working")));
        assert_eq!(m["step"], json!({ "key": "step.search", "vars": { "what": "TODO" } }));
        // Its edits count as the session's; a failure of its tool is said by no reaction of its own.
        let m = to_message(&event("PostToolUse", json!({ "sessionId": "rx-7f3a:subagent:call_7", "toolName": "write_file", "toolArgs": { "path": "C:\\w\\n.md" } }))).unwrap();
        assert_eq!((m["file"].as_str(), m["within"].as_bool()), (Some("C:\\w\\n.md"), Some(true)));
        let m = to_message(&event("PostToolUseFailure", json!({ "sessionId": "rx-7f3a:guardian", "toolName": "bash", "error": "exit 1" }))).unwrap();
        assert!(m.get("react").is_none() && m.get("say").is_none());
        // Never a turn, an ending or a session of its own.
        for (name, more) in [("UserPromptSubmit", json!({ "prompt": "Explore the code for TODOs" })), ("Stop", json!({ "lastAssistantText": "Found 3." })), ("SessionStart", json!({})), ("SessionEnd", json!({}))] {
            let mut more = more;
            more["sessionId"] = json!("rx-7f3a:subagent:call_7");
            assert!(to_message(&event(name, more)).is_none(), "{name}");
        }
    }

    #[test]
    fn an_approval_is_told_once_with_what_waits() {
        let _asks = super::reasonix_asks_held();
        let ask = |name: &str, more: Value, now: u64| to_message_at(&event(name, more), now);
        let session = json!({ "sessionId": "rx-ask-1" });
        let mut pr = session.clone();
        pr["toolName"] = json!("bash");
        pr["toolArgs"] = json!({ "command": "git push" });
        pr["subject"] = json!("git push");
        assert_eq!(ask("PermissionRequest", pr, 1_000).unwrap()["detail"]["key"], "detail.approveIn");
        // Its Notification as the dialog shows: said already.
        let mut note = session.clone();
        note["notificationType"] = json!("permission_prompt");
        note["message"] = json!("Approval needed: bash (git push)");
        assert!(ask("Notification", note.clone(), 1_050).is_none());
        // One with no PermissionRequest before it (a plan's, Auto Guard's): in its words.
        let m = ask("Notification", note.clone(), 1_000 + SAME_ASK_MS + 1).unwrap();
        assert_eq!(m["detail"], json!({ "key": "detail.approveIn", "vars": { "app": "Reasonix", "what": "Approval needed: bash (git push)" } }));
        let mut other = note.clone();
        other["sessionId"] = json!("rx-ask-2");
        other["message"] = json!("Auto Guard: confirm the next action");
        assert_eq!(ask("Notification", other, 1_100).unwrap()["mood"], "waiting");
        // A question: as the ask tool's start said it.
        let mut question = session.clone();
        question["notificationType"] = json!("question_prompt");
        assert_eq!(ask("Notification", question, 2_000).unwrap()["detail"], json!({ "key": "detail.asksQuestion" }));
        // Anything else: in its words, or that it needs the person.
        let mut else_ = session;
        else_["notificationType"] = json!("idle");
        assert_eq!(ask("Notification", else_.clone(), 3_000).unwrap()["detail"], json!({ "key": "detail.needsYou" }));
        else_["message"] = json!("Waiting for your input");
        assert_eq!(ask("Notification", else_, 3_000).unwrap()["detail"], "Waiting for your input");
    }

    #[test]
    fn reasonix_events_that_change_nothing() {
        let _asks = super::reasonix_asks_held();
        // The person stopping a tool is no failure of it.
        assert!(to_message(&event("PostToolUseFailure", json!({ "toolName": "bash", "isInterrupt": true }))).is_none());
        for name in ["SubagentStart", "SubagentStop", "PostLLMCall", "PreCompact", "Nope"] {
            assert!(to_message(&event(name, json!({ "callId": "c1" }))).is_none(), "{name}");
        }
        assert!(to_message(&json!("nope")).is_none());
    }
}

// ZCode's hook events: Claude Code's, its tools by Claude Code's names too
// (agent-core's claude.rs reads them, as the desktop app 3.15.3 sends them:
// 7 events, no SessionEnd or StopFailure; a failed tool's
// PostToolUseFailure carries error as its words; a call the person refused
// in ZCode's own dialog gets no PostToolUse(Failure), and its turn goes on to
// Stop; transcript_path is a file of its own made for each hook, with no
// session name in it; no permission suggestions, so a prompt for it is allow
// or deny: asks.rs), events.rs tells them in her words. Here, what she makes
// of them.
mod zcode {
    use serde_json::{json, Value};

    fn to_message(e: &Value) -> Option<Value> {
        agent_core::claude::to_event(e, agent_core::event::Agent::Zcode).map(|ev| crate::events::message_of(&ev))
    }

    // As the desktop app 3.15.3 sends them (trimmed), in a folder that is no
    // git repository.
    fn event(name: &str, more: Value) -> Value {
        let mut e = json!({
            "cwd": "Z:\\nowhere\\zc", "hookEventName": name, "mode": "build", "sessionId": "sess_fc09",
            "timestamp": "2026-10-15T20:33:22.513Z", "turnId": "turn_22f3",
            "hook_event_name": name, "permission_mode": "build", "session_id": "sess_fc09",
            "transcript_path": "Z:\\nowhere\\zcode-hook-BIXdU1\\transcript.jsonl",
        });
        for (k, v) in more.as_object().unwrap() {
            e[k] = v.clone();
        }
        e
    }

    #[test]
    fn zcode_events_become_messages() {
        let m = to_message(&event("SessionStart", json!({ "model": "deepseek/deepseek-flash", "source": "startup" }))).unwrap();
        assert_eq!(m, json!({ "mood": "idle", "react": "wave", "say": { "key": "say.hello" }, "agent": "zcode", "session": "sess_fc09", "project": "zc" }));
        let m = to_message(&event("UserPromptSubmit", json!({ "prompt": "Create a file named note.txt containing hi" }))).unwrap();
        assert_eq!((m["event"].as_str(), m["task"].as_str()), (Some("turn-start"), Some("Create a file named note.txt containing hi")));
        let write = json!({ "file_path": "Z:\\nowhere\\zc\\note.txt", "content": "hi\n" });
        let call = json!({ "toolCallId": "call_00_UlU5", "toolInput": write, "toolName": "Write", "tool_name": "Write", "tool_input": write, "tool_use_id": "call_00_UlU5", "riskLevel": "medium" });
        let m = to_message(&event("PreToolUse", call.clone())).unwrap();
        assert_eq!((m["step"].clone(), m["toolId"].as_str()), (json!({ "key": "step.write", "vars": { "file": "note.txt" } }), Some("call_00_UlU5")));
        let mut ask = call.clone();
        ask["reason"] = json!("Tool has side effects and requires approval");
        ask["requestId"] = json!("perm_20d3");
        let m = to_message(&event("PermissionRequest", ask)).unwrap();
        assert_eq!((m["mood"].as_str(), m["detail"]["key"].as_str()), (Some("waiting"), Some("detail.approve")));
        let m = to_message(&event("PostToolUse", call.clone())).unwrap();
        assert_eq!((m["event"].as_str(), m["file"].as_str()), (Some("tool-done"), Some("Z:\\nowhere\\zc\\note.txt")));
        let mut failed = call.clone();
        failed["error"] = json!("EACCES");
        failed["is_interrupt"] = json!(false);
        assert_eq!(to_message(&event("PostToolUseFailure", failed)).unwrap()["event"], "tool-failed");
        let m = to_message(&event("Stop", json!({ "responseText": "hello", "last_assistant_message": "hello", "stop_hook_active": false, "toolCallCount": 0 }))).unwrap();
        assert_eq!((m["mood"].as_str(), m["reply"].as_str(), m["agent"].as_str()), (Some("done"), Some("hello"), Some("zcode")));
        // Its subagents' tool, by either name.
        for tool in ["Agent", "Task"] {
            let m = to_message(&event("PreToolUse", json!({ "tool_name": tool, "tool_input": { "description": "Explore the code" } }))).unwrap();
            assert_eq!(m["step"], json!({ "key": "step.agent", "vars": { "what": "Explore the code" } }));
        }
    }

    #[test]
    fn zcode_events_that_change_nothing() {
        let mut stopped = event("PostToolUseFailure", json!({ "tool_name": "Bash", "error": "cancelled" }));
        stopped["is_interrupt"] = json!(true);
        assert!(to_message(&stopped).is_none());
        assert!(to_message(&event("Nope", json!({}))).is_none());
        assert!(to_message(&json!("nope")).is_none());
    }
}
