// An agent's event (agent-core's AgentEvent: what a session is doing) in her
// words, as texts { key, vars } for src/shared/i18n.ts, so the page says them
// in her language. Every agent's events come here so: agent-core reads each
// its own way (its hook's names, its quirks, whose a session is) and keeps
// the sessions (its registry, which her state.rs is built on); here they are
// told in her words:
//   detail   beside the mood: the tool, or what the session waits on (detail_of)
//   step     what a running tool does (step_for)
//   task     the person's words that started the turn, when there is no title
//   reply    how the last turn ended, in the agent's words
//   error    why a turn failed
//   reaction what she plays once over the mood (reaction_of)
use serde_json::{json, Value};

// Antigravity's and DeepSeek Harness's log lines; and, for her tests, what
// she makes of the others' events.
pub mod agy;
pub mod dsh;
#[cfg(test)]
pub(crate) mod others;

use agent_core::codex::patched;
use agent_core::event::{Agent, AgentEvent, AnswerIn, Call, Failure, Kind, Wait, What};
#[cfg(test)]
use agent_core::event::{Todo, TodoItem};
#[cfg(test)]
use agent_core::project::project_of;
#[cfg(test)]
use serde_json::Map;
pub(crate) use agent_core::text::{base, clip};

fn s<'a>(e: &'a Value, key: &str) -> &'a str {
    e.get(key).and_then(Value::as_str).unwrap_or("")
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

pub(crate) fn step(key: &str, vars: Value) -> Value {
    json!({ "key": key, "vars": vars })
}

// What a tool is doing, in plain words (by Claude Code's names for tools).
pub(crate) fn step_of(tool: &str, input: &Value) -> Value {
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

// A patch (Codex's apply_patch, OpenCode's patch) by the files it edits.
fn patch_step(call: &Call) -> Value {
    match patched(s(&call.input, "command")).as_slice() {
        [] => step("step.tool", json!({ "tool": call.own.as_deref().unwrap_or(&call.tool) })),
        [one] => step("step.edit", json!({ "file": base(one) })),
        [first, rest @ ..] => step("step.edit", json!({ "file": format!("{} +{}", base(first), rest.len()) })),
    }
}

// An omp call's step: ours for the tools Claude Code has too, else the
// model's intent, else its name.
fn step_of_omp(tool: &str, input: &Value, intent: &str) -> Value {
    let said = || step("step.run", json!({ "what": clip(intent, 60) }));
    match agent_core::omp::as_claude(tool, input, intent) {
        // A tool of ours with nothing to name (no file, no pattern): its intent.
        Some((name, args)) => {
            let blank = args.as_object().is_some_and(|a| !a.is_empty() && a.values().all(|v| v.as_str().is_some_and(|t| t.trim().is_empty())));
            if blank && !intent.is_empty() { said() } else { step_of(name, &args) }
        }
        None if tool.starts_with("mcp__") || intent.is_empty() => step_of(tool, input),
        None => said(),
    }
}

// What a call is doing, in plain words, as each agent's calls say it best.
pub(crate) fn step_for(agent: Agent, call: &Call) -> Value {
    match agent {
        Agent::Codex | Agent::Opencode if call.tool == "apply_patch" => patch_step(call),
        Agent::Omp => step_of_omp(call.own.as_deref().unwrap_or(&call.tool), call.raw.as_ref().unwrap_or(&call.input), call.intent.as_deref().unwrap_or("")),
        _ => step_of(&call.tool, &call.input),
    }
}

// The person's words as the name of what they asked for: the first line, or
// nothing for a few words that only carry on ("ok", "继续").
pub(crate) fn task_of(prompt: &str) -> Option<String> {
    let line = prompt.lines().map(str::trim).find(|l| !l.is_empty())?;
    (line.chars().count() >= 4 && !line.starts_with('<')).then(|| clip(line, 80))
}

// How a turn ended, from the agent's last message: its first lines, without
// the marks of markdown, up to the end of a sentence (or about 40 characters).
pub(crate) fn reply_of(message: &str) -> Option<String> {
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
        let line = unlink(&line.replace("**", "").replace('`', ""));
        out.push_str(&line);
        let ends = line.ends_with(['。', '！', '？', '.', '!', '?']);
        if out.chars().count() >= 40 || (ends && out.chars().count() >= 12) {
            break;
        }
    }
    (!out.is_empty()).then(|| clip(&out, 140))
}

// A markdown link as its words: "[a.txt](C:\w\a.txt)" → "a.txt" (ZCode
// links each file it names so).
fn unlink(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        let Some((words, tail)) = after.split_once("](") else { break };
        let Some(close) = tail.find(')').filter(|_| !words.contains(['[', ']'])) else {
            out.push_str(&rest[..=open]);
            rest = after;
            continue;
        };
        out.push_str(&rest[..open]);
        out.push_str(words);
        rest = &tail[close + 1..];
    }
    out.push_str(rest);
    out
}

// Why a turn failed: one of the kinds she has words for, or the agent's own
// words (their first line).
pub(crate) fn error_of(error: &Failure) -> Value {
    match error {
        Failure::Kind { kind } => json!({ "key": format!("error.{kind}"), "vars": {} }),
        Failure::Said { said } => json!(clip(said.lines().next().unwrap_or(""), 80)),
    }
}

// A change to the to-do list, in the agents' tests' words.
#[cfg(test)]
fn todo_json(todo: &Todo) -> Value {
    let item = |t: &TodoItem| json!({ "text": clip(&t.text, 80), "active": clip(&t.active, 80), "status": t.status });
    match todo {
        Todo::Set { items } => json!({ "set": items.iter().map(item).collect::<Vec<_>>() }),
        Todo::Add { item } => json!({ "add": { "text": clip(&item.text, 80), "active": clip(&item.active, 80) } }),
        Todo::Update { id, status, text, active } => {
            let mut update = Map::new();
            update.insert("id".into(), json!(id));
            for (key, value) in [("status", status), ("text", text), ("active", active)] {
                if let Some(value) = value {
                    update.insert(key.into(), json!(clip(value, 80)));
                }
            }
            json!({ "update": update })
        }
    }
}

// What a session waits on, in her words.
fn wait_of(agent: Agent, wait: &Wait) -> Value {
    match wait {
        // Handed back to the agent's own window (asks.rs handed_event).
        Wait::Question { question: Some(question), answer_in: Some(AnswerIn::App { name }) } => json!({ "key": "detail.answerIn", "vars": { "app": name, "what": clip(question, 40) } }),
        Wait::Question { .. } => json!({ "key": "detail.asksQuestion" }),
        Wait::Plan => json!({ "key": "detail.planToConfirm" }),
        Wait::Input { server } => json!({ "key": "detail.needsInput", "vars": { "server": server } }),
        Wait::Notice { said: None } => json!({ "key": "detail.needsYou" }),
        Wait::Notice { said: Some(said) } => json!(clip(said, 80)),
        Wait::Approval { what, answer_in } => {
            let what = match what {
                What::Call { call } => step_for(agent, call),
                What::Action { name } => step("step.tool", json!({ "tool": name })),
                What::Said { said } => json!(clip(said, 80)),
            };
            match answer_in {
                AnswerIn::Here => json!({ "key": "detail.approve", "vars": { "what": what } }),
                AnswerIn::App { name } => json!({ "key": "detail.approveIn", "vars": { "app": name, "what": what } }),
                AnswerIn::Terminal => json!({ "key": "detail.approveInTerminal", "vars": { "what": what } }),
            }
        }
    }
}

// What a session's line says beside its mood after an event: the tool, or
// what it waits on; nothing for the rest (state.rs keeps it till the next).
pub(crate) fn detail_of(ev: &AgentEvent) -> Value {
    match &ev.kind {
        Kind::ToolStart { call } | Kind::ToolDone { call } | Kind::ToolFailed { call, .. } => json!(call.tool),
        Kind::Waiting { wait } => wait_of(ev.agent, wait),
        _ => json!(""),
    }
}

// What she plays once over the mood for an event, { react, say }: a wave as
// a session starts, a start as a tool fails (OpenCode's by its own name; a
// subagent's quietly), a jump as a task of its list is done.
pub(crate) fn reaction_of(ev: &AgentEvent) -> Option<Value> {
    match &ev.kind {
        Kind::SessionStart => Some(json!({ "react": "wave", "say": { "key": "say.hello" } })),
        Kind::ToolFailed { call, quiet: false } => {
            let name = if ev.agent == Agent::Opencode { call.own.as_deref().unwrap_or(&call.tool) } else { &call.tool };
            Some(json!({ "react": "failed", "say": { "key": "say.toolFailed", "vars": { "tool": name } } }))
        }
        Kind::TaskDone { subject } => Some(json!({
            "react": "jump",
            "say": match subject {
                Some(task) => json!({ "key": "say.taskDone", "vars": { "task": task } }),
                None => json!({ "key": "say.taskDoneGeneric" }),
            },
        })),
        _ => None,
    }
}

// An event in her words, all in one value (the mood it comes to, before a
// turn's edits make "done" "review"; the words above; what it carries), for
// the agents' tests (events_*.rs): what an agent's event says, at a look.
#[cfg(test)]
pub fn message_of(ev: &AgentEvent) -> Value {
    let mut m = match &ev.kind {
        Kind::SessionStart | Kind::Rest => json!({ "mood": "idle" }),
        Kind::SessionEnd => json!({ "mood": "idle", "event": "session-end" }),
        Kind::TurnStart { prompt } => {
            let mut m = json!({ "mood": "working", "event": "turn-start" });
            if let Some(task) = prompt.as_deref().and_then(task_of) {
                m["task"] = json!(task);
            }
            m
        }
        Kind::ToolStart { call } => json!({ "mood": "working", "event": "tool-start", "step": step_for(ev.agent, call) }),
        Kind::ToolDone { .. } => json!({ "mood": "working", "event": "tool-done" }),
        Kind::ToolFailed { .. } => json!({ "mood": "working", "event": "tool-failed" }),
        Kind::Waiting { .. } => json!({ "mood": "waiting" }),
        Kind::Working => json!({ "mood": "working" }),
        // Stopped by the person: nothing to come and see.
        Kind::Stopped => json!({ "mood": "idle", "event": "turn-end" }),
        Kind::Done => json!({ "mood": "done", "event": "turn-end" }),
        Kind::Failed { error } => json!({ "mood": "error", "event": "turn-end", "error": error_of(error) }),
        Kind::TaskDone { .. } => json!({}),
    };
    let detail = detail_of(ev);
    if detail != json!("") {
        m["detail"] = detail;
    }
    if let Some(reaction) = reaction_of(ev) {
        m["react"] = reaction["react"].clone();
        m["say"] = reaction["say"].clone();
    }
    if let Kind::ToolStart { call } | Kind::ToolDone { call } | Kind::ToolFailed { call, .. } = &ev.kind {
        if let Some(id) = &call.id {
            m["toolId"] = json!(id);
        }
        if let Some(file) = &call.file {
            m["file"] = json!(file);
        }
        if let Some(todo) = &call.todo {
            m["todo"] = todo_json(todo);
        }
    }
    if let Some(reply) = ev.reply.as_deref().and_then(reply_of) {
        m["reply"] = json!(reply);
    }
    if let Some(agent) = ev.agent.tag() {
        m["agent"] = json!(agent);
    }
    if !ev.session.is_empty() {
        m["session"] = json!(ev.session);
    }
    if let Some(turn) = &ev.turn {
        m["turn"] = json!(turn);
    }
    if let Some(cwd) = &ev.cwd {
        m["project"] = json!(project_of(cwd));
    }
    if let Some(title) = &ev.title {
        m["title"] = json!(clip(title, 80));
    }
    if ev.within {
        m["within"] = json!(true);
    }
    m
}

// A Claude Code hook event's message (the agents' files' tests read theirs so).
#[cfg(test)]
pub fn to_message(e: &Value) -> Option<Value> {
    agent_core::claude::to_event(e, Agent::Claude).map(|ev| message_of(&ev))
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
        assert_eq!(to_message(&json!({ "hook_event_name": "Stop" })).unwrap(), json!({ "mood": "done", "event": "turn-end" }));
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
        let m = to_message(&json!({ "hook_event_name": "Stop", "last_assistant_message": "Created [a.txt](C:\\w\\a.txt) and [b](x) with the content `hi`." })).unwrap();
        assert_eq!(m["reply"], "Created a.txt and b with the content hi.");
        assert_eq!(unlink("a [b] c [d](e"), "a [b] c [d](e");
        assert_eq!(unlink("[x [y](z)"), "[x y");
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
}
