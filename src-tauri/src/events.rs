// Claude Code hook events → pet messages, and texts as { key, vars } for
// src/shared/i18n.js, so the page says them in her language.
//
// Besides the mood, a message says what the session is about and where it is:
//   title    Claude Code's name for the session (session_title, or the last
//            title in its transcript)
//   task     the person's words that started the turn, when there is no title
//   step     the tool running, in plain words ({ key, vars }); toolId pairs
//            its start (tool-start) with its end (tool-done, tool-failed)
//   todo     a change to its to-do list: { set: [...] }, { add }, { update }
//   file     a file a tool just edited
//   reply    how the last turn ended, in Claude's words
//   error    why a turn failed
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Mutex;

use serde_json::{json, Map, Value};

// How much of a transcript's end is read for its title: Claude Code writes
// the title again near the end as it goes.
const TITLE_TAIL: u64 = 256 * 1024;

fn s<'a>(e: &'a Value, key: &str) -> &'a str {
    e.get(key).and_then(Value::as_str).unwrap_or("")
}

fn clip(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

// The last part of a path, either slash.
fn base(path: &str) -> &str {
    path.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().unwrap_or(path)
}

// The part of a command that says what it does: its first line, without the
// `cd somewhere &&` in front, up to the next && ; or |.
fn command_core(command: &str) -> String {
    let line = command.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let parts = line.split("&&").flat_map(|p| p.split(';')).map(str::trim).filter(|p| !p.is_empty());
    let core = parts.clone().find(|p| !(p.starts_with("cd ") || p.starts_with("Set-Location "))).or_else(|| parts.clone().next()).unwrap_or("");
    let core = core.split('|').next().unwrap_or("").trim();
    clip(core, 48)
}

fn host(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    rest.split(['/', '?', '#']).next().unwrap_or(rest)
}

fn step(key: &str, vars: Value) -> Value {
    json!({ "key": key, "vars": vars })
}

// What a tool is doing, in plain words.
fn step_of(tool: &str, input: &Value) -> Value {
    let i = |key: &str| s(input, key);
    match tool {
        "Bash" | "PowerShell" => {
            if !i("description").is_empty() {
                step("step.run", json!({ "what": clip(i("description"), 60) }))
            } else {
                step("step.command", json!({ "what": command_core(i("command")) }))
            }
        }
        "Edit" | "MultiEdit" => step("step.edit", json!({ "file": base(i("file_path")) })),
        "NotebookEdit" => step("step.edit", json!({ "file": base(i("notebook_path")) })),
        "Write" => step("step.write", json!({ "file": base(i("file_path")) })),
        "Read" => step("step.read", json!({ "file": base(i("file_path")) })),
        "Grep" => step("step.search", json!({ "what": clip(i("pattern"), 32) })),
        "Glob" => step("step.find", json!({ "what": clip(i("pattern"), 32) })),
        "WebFetch" => step("step.fetch", json!({ "what": host(i("url")) })),
        "WebSearch" => step("step.webSearch", json!({ "what": clip(i("query"), 40) })),
        "Agent" | "Task" => step("step.agent", json!({ "what": clip(if i("description").is_empty() { i("subagent_type") } else { i("description") }, 48) })),
        "TodoWrite" | "TaskCreate" | "TaskUpdate" => step("step.plan", json!({})),
        "Skill" => step("step.skill", json!({ "what": clip(i("skill"), 40) })),
        _ => match tool.strip_prefix("mcp__").and_then(|rest| rest.split_once("__")) {
            Some((server, name)) => step("step.mcp", json!({ "server": server, "tool": name })),
            None => step("step.tool", json!({ "tool": tool })),
        },
    }
}

// A change to the session's to-do list, from the tools that keep it.
fn todo_of(tool: &str, input: &Value) -> Option<Value> {
    let i = |key: &str| s(input, key);
    match tool {
        "TodoWrite" => {
            let items: Vec<Value> = input
                .get("todos")?
                .as_array()?
                .iter()
                .map(|t| json!({ "text": clip(s(t, "content"), 80), "active": clip(s(t, "activeForm"), 80), "status": s(t, "status") }))
                .collect();
            Some(json!({ "set": items }))
        }
        "TaskCreate" => Some(json!({ "add": { "text": clip(i("subject"), 80), "active": clip(i("activeForm"), 80) } })),
        "TaskUpdate" => {
            let id = match input.get("taskId") {
                Some(Value::String(id)) => id.clone(),
                Some(Value::Number(n)) => n.to_string(),
                _ => return None,
            };
            let mut update = Map::new();
            update.insert("id".into(), json!(id));
            for (from, to) in [("status", "status"), ("subject", "text"), ("activeForm", "active")] {
                if !i(from).is_empty() {
                    update.insert(to.into(), json!(clip(i(from), 80)));
                }
            }
            Some(json!({ "update": update }))
        }
        _ => None,
    }
}

// The person's words as the name of what they asked for: the first line, or
// nothing for a few words that only carry on ("ok", "继续").
fn task_of(prompt: &str) -> Option<String> {
    let line = prompt.lines().map(str::trim).find(|l| !l.is_empty())?;
    (line.chars().count() >= 4 && !line.starts_with('<')).then(|| clip(line, 80))
}

// How a turn ended, from Claude's last message: its first lines, without the
// marks of markdown, up to the end of a sentence (or about 40 characters).
fn reply_of(message: &str) -> Option<String> {
    let mut out = String::new();
    let mut after_heading = false;
    for line in message.lines() {
        let is_heading = line.trim_start().starts_with('#');
        let line = line.trim().trim_start_matches(['#', '>', '-', '*', ' ']).trim();
        if line.is_empty() || line.starts_with("```") || line.starts_with('|') {
            continue;
        }
        // A heading leads into what follows it: "Done: ...", "改好了：..."
        if after_heading {
            out.push_str(if out.is_ascii() { ": " } else { "：" });
        } else if !out.is_empty() {
            out.push(' ');
        }
        after_heading = is_heading && !line.ends_with(['。', '！', '？', '.', '!', '?', ':', '：']);
        let line = line.replace("**", "").replace('`', "");
        out.push_str(&line);
        let ends = line.ends_with(['。', '！', '？', '.', '!', '?']);
        if out.chars().count() >= 40 || (ends && out.chars().count() >= 12) {
            break;
        }
    }
    (!out.is_empty()).then(|| clip(&out, 140))
}

// Why a turn failed: one of Claude Code's error kinds, or its own words.
fn error_of(e: &Value) -> Value {
    const KINDS: [&str; 10] = [
        "authentication_failed",
        "oauth_org_not_allowed",
        "account_on_hold",
        "billing_error",
        "rate_limit",
        "overloaded",
        "invalid_request",
        "model_not_found",
        "server_error",
        "max_output_tokens",
    ];
    let kind = s(e, "error");
    if KINDS.contains(&kind) {
        return json!({ "key": format!("error.{kind}"), "vars": {} });
    }
    let details = s(e, "error_details");
    json!(clip(details.lines().next().unwrap_or(""), 80))
}

// Claude Code's name for the session from its transcript: the last name the
// person (or the app) gave it, else the last one Claude made up.
pub fn transcript_title(path: &str) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(TITLE_TAIL))).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let (mut custom, mut ai) = (None, None);
    for line in text.lines().rev() {
        if custom.is_some() {
            break;
        }
        let is_custom = line.contains("\"custom-title\"");
        if !is_custom && (ai.is_some() || !line.contains("\"ai-title\"")) {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<Value>(line) else { continue };
        match s(&entry, "type") {
            "custom-title" => custom = Some(s(&entry, "customTitle").to_string()),
            "ai-title" => ai = Some(s(&entry, "aiTitle").to_string()),
            _ => {}
        }
    }
    custom.or(ai).map(|t| clip(&t, 80)).filter(|t| !t.is_empty())
}

// The project a folder belongs to: the repository's own folder, so a session
// in a git worktree (.claude/worktrees/<name>, or anywhere) still says whose;
// outside git, the folder itself. Remembered per folder.
fn project_of(cwd: &str) -> String {
    static KNOWN: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);
    if let Some(name) = KNOWN.lock().unwrap().as_ref().and_then(|m| m.get(cwd)) {
        return name.clone();
    }
    let name = repo_name(Path::new(cwd)).unwrap_or_else(|| base(cwd).to_string());
    KNOWN.lock().unwrap().get_or_insert_with(HashMap::new).insert(cwd.to_string(), name.clone());
    name
}

fn repo_name(cwd: &Path) -> Option<String> {
    let name = |p: &Path| p.file_name().map(|n| n.to_string_lossy().to_string());
    for dir in cwd.ancestors().take(16) {
        let git = dir.join(".git");
        if git.is_dir() {
            return name(dir);
        }
        if git.is_file() {
            // A worktree's .git says where its repository keeps it:
            // gitdir: <repo>/.git/worktrees/<name>
            let text = std::fs::read_to_string(&git).ok()?;
            let gitdir = text.trim().strip_prefix("gitdir:")?.trim().replace('\\', "/");
            return match gitdir.find("/.git/worktrees/") {
                Some(at) => Some(base(&gitdir[..at]).to_string()),
                None => name(dir),
            };
        }
    }
    None
}

// What a tool that stops for the person is waiting on.
fn asks_person(tool: &str) -> Option<Value> {
    match tool {
        "AskUserQuestion" => Some(json!({ "key": "detail.asksQuestion" })),
        "ExitPlanMode" => Some(json!({ "key": "detail.planToConfirm" })),
        _ => None,
    }
}

const EDITS: [&str; 4] = ["Edit", "Write", "MultiEdit", "NotebookEdit"];

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
            // Only the person's words name the work, not a turn the system began.
            if matches!(s(e, "source"), "" | "user") {
                if let Some(task) = task_of(s(e, "prompt")) {
                    m["task"] = json!(task);
                }
            }
            m
        }
        "PreToolUse" => match asks_person(tool) {
            Some(detail) => json!({ "mood": "waiting", "detail": detail }),
            None => {
                let mut m = json!({ "mood": "working", "detail": tool, "event": "tool-start", "step": step_of(tool, input) });
                if let Some(todo) = todo_of(tool, input) {
                    m["todo"] = todo;
                }
                m
            }
        },
        "PostToolUse" => json!({ "mood": "working", "detail": tool, "event": "tool-done" }),
        // An interrupt is the person stopping it, not the tool failing.
        "PostToolUseFailure" if e.get("is_interrupt").and_then(Value::as_bool) == Some(true) => return None,
        "PostToolUseFailure" => json!({
            "mood": "working", "detail": tool, "event": "tool-failed", "react": "failed",
            "say": { "key": "say.toolFailed", "vars": { "tool": tool } },
        }),
        "PermissionRequest" => json!({
            "mood": "waiting",
            "detail": asks_person(tool).unwrap_or_else(|| json!({ "key": "detail.approve", "vars": { "what": step_of(tool, input) } })),
        }),
        "Elicitation" => json!({ "mood": "waiting", "detail": { "key": "detail.needsInput", "vars": { "server": s(e, "mcp_server_name") } } }),
        "TaskCompleted" => json!({
            "react": "jump",
            "say": if s(e, "task_subject").is_empty() {
                json!({ "key": "say.taskDoneGeneric" })
            } else {
                json!({ "key": "say.taskDone", "vars": { "task": s(e, "task_subject") } })
            },
        }),
        "Stop" => json!({ "mood": "done" }),
        "StopFailure" => json!({ "mood": "error", "error": error_of(e) }),
        _ => return None,
    };
    if s(e, "hook_event_name") == "PostToolUse" && EDITS.contains(&tool) {
        let path = if tool == "NotebookEdit" { s(input, "notebook_path") } else { s(input, "file_path") };
        if !path.is_empty() {
            m["file"] = json!(path);
        }
    }
    if matches!(s(e, "hook_event_name"), "PreToolUse" | "PostToolUse" | "PostToolUseFailure") && !s(e, "tool_use_id").is_empty() {
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
    if let Some(id) = e.get("session_id").and_then(Value::as_str).filter(|s| !s.is_empty()) {
        message["session"] = json!(id);
    }
    if let Some(cwd) = e.get("cwd").and_then(Value::as_str).filter(|s| !s.is_empty()) {
        message["project"] = json!(project_of(cwd));
    }
    // The session's name: given with the event, or else, at the moments it
    // may have changed (a turn begins or ends), read from the transcript.
    let event = s(e, "hook_event_name");
    let title = match s(e, "session_title") {
        "" if matches!(event, "UserPromptSubmit" | "Stop" | "StopFailure" | "SessionStart") => transcript_title(s(e, "transcript_path")),
        "" => None,
        given => Some(clip(given, 80)),
    };
    if let Some(title) = title {
        message["title"] = json!(title);
    }
    Some(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_become_messages() {
        let m = to_message(&json!({ "hook_event_name": "PreToolUse", "tool_name": "Bash", "session_id": "s", "cwd": "Z:\\nowhere\\wakuwaku" })).unwrap();
        assert_eq!(
            m,
            json!({ "mood": "working", "detail": "Bash", "event": "tool-start", "step": { "key": "step.command", "vars": { "what": "" } }, "session": "s", "project": "wakuwaku" })
        );
        let m = to_message(&json!({ "hook_event_name": "PreToolUse", "tool_name": "AskUserQuestion" })).unwrap();
        assert_eq!(m["mood"], "waiting");
        assert!(to_message(&json!({ "hook_event_name": "SessionStart", "source": "compact" })).is_none());
        assert!(to_message(&json!({ "hook_event_name": "PostToolUseFailure", "is_interrupt": true })).is_none());
        assert_eq!(to_message(&json!({ "hook_event_name": "Stop" })).unwrap(), json!({ "mood": "done" }));
        assert!(to_message(&json!("nope")).is_none());
    }

    #[test]
    fn tools_say_what_they_do() {
        let step = |tool: &str, input: Value| to_message(&json!({ "hook_event_name": "PreToolUse", "tool_name": tool, "tool_input": input })).unwrap()["step"].clone();
        assert_eq!(step("Bash", json!({ "command": "cargo test", "description": "Run the tests" })), json!({ "key": "step.run", "vars": { "what": "Run the tests" } }));
        assert_eq!(step("Bash", json!({ "command": "cd src-tauri && cargo test --release | tail" }))["vars"]["what"], "cargo test --release");
        assert_eq!(step("Edit", json!({ "file_path": "D:\\p\\src\\island.rs" })), json!({ "key": "step.edit", "vars": { "file": "island.rs" } }));
        assert_eq!(step("Read", json!({ "file_path": "/home/x/a.js" }))["vars"]["file"], "a.js");
        assert_eq!(step("WebFetch", json!({ "url": "https://docs.rs/serde/latest" }))["vars"]["what"], "docs.rs");
        assert_eq!(step("Agent", json!({ "description": "Explore the code" }))["key"], "step.agent");
        assert_eq!(step("mcp__github__create_issue", json!({})), json!({ "key": "step.mcp", "vars": { "server": "github", "tool": "create_issue" } }));
        assert_eq!(step("Frobnicate", json!({})), json!({ "key": "step.tool", "vars": { "tool": "Frobnicate" } }));
    }

    #[test]
    fn a_turn_brings_its_name_its_edits_and_its_end() {
        let m = to_message(&json!({ "hook_event_name": "UserPromptSubmit", "prompt": "\n修复设置收起后岛不消失\n细节……", "session_title": "岛的设置" })).unwrap();
        assert_eq!(m["task"], "修复设置收起后岛不消失");
        assert_eq!(m["title"], "岛的设置");
        assert!(to_message(&json!({ "hook_event_name": "UserPromptSubmit", "prompt": "继续" })).unwrap().get("task").is_none());
        assert!(to_message(&json!({ "hook_event_name": "UserPromptSubmit", "prompt": "a task notification", "source": "system" })).unwrap().get("task").is_none());

        let m = to_message(&json!({ "hook_event_name": "PostToolUse", "tool_name": "Edit", "tool_input": { "file_path": "a/b.rs" }, "tool_use_id": "t1" })).unwrap();
        assert_eq!((m["file"].as_str(), m["toolId"].as_str()), (Some("a/b.rs"), Some("t1")));

        let m = to_message(&json!({ "hook_event_name": "Stop", "last_assistant_message": "## 改好了\n\n改了 **island.rs** 的 `收起` 逻辑，测试全过。\n\n- 第二点" })).unwrap();
        assert_eq!(m["reply"], "改好了：改了 island.rs 的 收起 逻辑，测试全过。");
        let m = to_message(&json!({ "hook_event_name": "Stop", "last_assistant_message": "# Done\nAll **12** tests pass.\nMore." })).unwrap();
        assert_eq!(m["reply"], "Done: All 12 tests pass.");
        let m = to_message(&json!({ "hook_event_name": "Stop", "last_assistant_message": "Short.\nThen a second line that is long enough to stop." })).unwrap();
        assert_eq!(m["reply"], "Short. Then a second line that is long enough to stop.");
        let m = to_message(&json!({ "hook_event_name": "StopFailure", "error": "rate_limit" })).unwrap();
        assert_eq!(m["error"], json!({ "key": "error.rate_limit", "vars": {} }));
        let m = to_message(&json!({ "hook_event_name": "PermissionRequest", "tool_name": "Bash", "tool_input": { "command": "git push" } })).unwrap();
        assert_eq!(m["detail"], json!({ "key": "detail.approve", "vars": { "what": { "key": "step.command", "vars": { "what": "git push" } } } }));
    }

    #[test]
    fn to_do_lists_come_from_both_kinds_of_tool() {
        let todo = |tool: &str, input: Value| to_message(&json!({ "hook_event_name": "PreToolUse", "tool_name": tool, "tool_input": input })).unwrap()["todo"].clone();
        let set = todo("TodoWrite", json!({ "todos": [{ "content": "Run tests", "activeForm": "Running tests", "status": "in_progress" }] }));
        assert_eq!(set, json!({ "set": [{ "text": "Run tests", "active": "Running tests", "status": "in_progress" }] }));
        assert_eq!(todo("TaskCreate", json!({ "subject": "A", "description": "…" })), json!({ "add": { "text": "A", "active": "" } }));
        assert_eq!(todo("TaskUpdate", json!({ "taskId": "2", "status": "completed" })), json!({ "update": { "id": "2", "status": "completed" } }));
    }

    #[test]
    fn titles_and_projects() {
        let dir = std::env::temp_dir().join(format!("wakuwaku-events-{}", std::process::id()));
        let wt = dir.join("repo").join(".claude").join("worktrees").join("brave-otter");
        std::fs::create_dir_all(dir.join("repo").join(".git")).unwrap();
        std::fs::create_dir_all(wt.join("src")).unwrap();
        let gitdir = dir.join("repo").join(".git").join("worktrees").join("brave-otter");
        std::fs::write(wt.join(".git"), format!("gitdir: {}\n", gitdir.display())).unwrap();
        assert_eq!(project_of(&wt.join("src").to_string_lossy()), "repo");
        assert_eq!(project_of(&dir.join("repo").to_string_lossy()), "repo");

        let transcript = dir.join("t.jsonl");
        let lines = [
            r#"{"type":"ai-title","aiTitle":"Made up","sessionId":"s"}"#,
            r#"{"type":"custom-title","customTitle":"Given","sessionId":"s"}"#,
            r#"{"type":"user","message":"hi"}"#,
            r#"{"type":"ai-title","aiTitle":"Made up later","sessionId":"s"}"#,
        ];
        std::fs::write(&transcript, lines.join("\n")).unwrap();
        assert_eq!(transcript_title(&transcript.to_string_lossy()).as_deref(), Some("Given"));
        std::fs::write(&transcript, [lines[0], lines[3]].join("\n")).unwrap();
        assert_eq!(transcript_title(&transcript.to_string_lossy()).as_deref(), Some("Made up later"));
        assert_eq!(transcript_title("Z:\\no\\such\\file.jsonl"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
