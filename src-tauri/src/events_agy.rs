// Antigravity's hook events, handed to her port by this program with
// connection::AGY_FLAG and the event's name (main.rs agy_hook): agent-core
// reads them (agy.rs to_event, with the person's words and the reply from
// its transcript), events.rs tells them in her words. A PreToolUse that
// Antigravity will ask the person about (needs_approval) comes as Claude
// Code's PermissionRequest (for_ask), answered with auto_approve / deny /
// allow (to_output). Here too, her log's line for a turn.
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

fn s<'a>(e: &'a Value, key: &str) -> &'a str {
    e.get(key).and_then(Value::as_str).unwrap_or("")
}

// Antigravity's user settings (~/.gemini/config/config.json), where its
// permission preset, command policy and "Always Allow" grants live.
pub fn config_file() -> PathBuf {
    crate::connection::gemini_dir().join("config").join("config.json")
}

pub fn read_config() -> Value {
    std::fs::read_to_string(config_file())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| json!({}))
}

// Antigravity CLI's settings (~/.gemini/antigravity-cli/settings.json), where
// toolPermission (always-proceed, request-review, strict, proceed-in-sandbox),
// agentMode (accept-edits, auto), allowNonWorkspaceAccess and permissions.allow live.
pub fn cli_settings_file() -> PathBuf {
    crate::connection::gemini_dir().join("antigravity-cli").join("settings.json")
}

pub fn read_cli_settings() -> Value {
    std::fs::read_to_string(cli_settings_file())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| json!({}))
}

// Whether an Antigravity event comes from `agy` CLI (vs Antigravity 2.0 Desktop).
// CLI events have transcripts/artifacts inside `antigravity-cli/`, or are flagged
// by main.rs via process inspection (jump::agy_cli_info).
pub fn is_cli_event(e: &Value) -> bool {
    if e.get("is_cli").and_then(Value::as_bool) == Some(true) {
        return true;
    }
    let transcript = s(e, "transcriptPath");
    let artifact = s(e, "artifactDirectoryPath");
    transcript.contains("antigravity-cli") || artifact.contains("antigravity-cli")
}

// A line for her log as a turn begins or ends, or a prompt comes: what
// Antigravity said, to tell a turn that never ended from one shown short.
pub fn log_line(e: &Value) -> Option<String> {
    let name = s(e, "hook_event_name");
    let what = match name {
        "SessionStart" => String::new(),
        "PreInvocation" if e.get("invocationNum").and_then(Value::as_u64) == Some(0) => format!(" 0, the person's step {}", e["userStep"]),
        "PermissionRequest" => format!(" {}", s(e, "tool_name")),
        "Stop" => format!(", fullyIdle {}, {}{}", e["fullyIdle"], s(e, "terminationReason"), if s(e, "error").is_empty() { "" } else { ", an error" }),
        _ => return None,
    };
    Some(format!("agy {name}{what} ({})", s(e, "conversationId").chars().take(8).collect::<String>()))
}

fn strip_drive(p: &str) -> &str {
    let bytes = p.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        &p[2..]
    } else {
        p
    }
}

pub fn norm_path(p: &str) -> String {
    let mut s = p.trim().replace('\\', "/");
    if cfg!(windows) {
        s = strip_drive(&s).to_string();
        s = s.to_ascii_lowercase();
    }
    while s.ends_with('/') && s.len() > 1 {
        s.pop();
    }
    s
}

pub fn path_within(path: &str, root: &str) -> bool {
    let (p, r) = (norm_path(path), norm_path(root));
    if p.is_empty() || r.is_empty() {
        return false;
    }
    if r == "/" {
        return p.starts_with('/');
    }
    p == r || p.strip_prefix(&r).is_some_and(|rest| rest.starts_with('/'))
}

pub fn is_relative_path(path: &str) -> bool {
    let p = path.trim();
    !p.is_empty() && !p.starts_with('/') && !p.starts_with('\\') && p.as_bytes().get(1) != Some(&b':')
}

fn resolve_relative_in_ws(ws: &str, rel: &str) -> PathBuf {
    let mut base = PathBuf::from(ws);
    for comp in Path::new(rel).components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                base.pop();
            }
            std::path::Component::Normal(c) => base.push(c),
            _ => {}
        }
    }
    base
}

// Antigravity's isFilePathAllowed allows any path inside workspacePaths,
// artifactDirectoryPath, or its own ~/.gemini/antigravity or ~/.gemini/antigravity-cli folders.
fn is_workspace_or_artifact_path(e: &Value, path: &str) -> bool {
    let p = path.trim();
    if p.is_empty() {
        return true;
    }
    let ws_list = e.get("workspacePaths").and_then(Value::as_array);
    if is_relative_path(p) {
        if let Some(ws) = ws_list {
            for root in ws.iter().filter_map(Value::as_str) {
                let resolved = resolve_relative_in_ws(root, p);
                if path_within(&resolved.to_string_lossy(), root) {
                    return true;
                }
            }
        }
        return false;
    }
    if let Some(ws) = ws_list {
        if ws.iter().filter_map(Value::as_str).any(|root| path_within(p, root)) {
            return true;
        }
    }
    let artifact = s(e, "artifactDirectoryPath");
    if !artifact.is_empty() && path_within(p, artifact) {
        return true;
    }
    let gemini = crate::connection::gemini_dir();
    let agy_dir = gemini.join("antigravity");
    if path_within(p, &agy_dir.to_string_lossy()) {
        return true;
    }
    let cli_dir = gemini.join("antigravity-cli");
    path_within(p, &cli_dir.to_string_lossy())
}

// Extract hostname/domain from a URL (e.g. "https://mail.google.com/a" -> "mail.google.com").
pub fn extract_domain(url: &str) -> &str {
    let mut s = url.trim();
    if let Some(pos) = s.find("://") {
        s = &s[pos + 3..];
    }
    let end = s.find(['/', '?', '#', ':']).unwrap_or(s.len());
    &s[..end]
}

pub fn domain_covers(grant_target: &str, url_or_target: &str) -> bool {
    if grant_target == "*" {
        return true;
    }
    let host = extract_domain(url_or_target).to_ascii_lowercase();
    let g_host = extract_domain(grant_target).to_ascii_lowercase();
    if g_host.is_empty() || host.is_empty() {
        return false;
    }
    host == g_host || host.ends_with(&format!(".{g_host}"))
}

// Matches a command against a rule target.
// agy 1.4.7 requires exact full-line matching (except for wildcard `*`).
fn command_rule_covers(grant_target: &str, cmd: &str) -> bool {
    let grant_target = grant_target.trim();
    let cmd = cmd.trim();
    if grant_target == "*" {
        return true;
    }
    grant_target == cmd
}

// Parse Antigravity's `<action>(<target>)` grant string.
pub fn parse_grant(grant: &str) -> Option<(&str, &str)> {
    let g = grant.trim();
    let open = g.find('(')?;
    let rest = g.strip_suffix(')')?;
    let action = &rest[..open];
    let target = &rest[open + 1..];
    (!action.is_empty()).then_some((action, target))
}

fn path_matches_target(path: &str, target: &str, ws_paths: &[String]) -> bool {
    if target == "*" {
        return true;
    }
    if path_within(path, target) {
        return true;
    }
    // If target is relative (e.g. "src/", ".git/"), check against workspace paths
    if is_relative_path(target) {
        for ws in ws_paths {
            let combined = format!("{ws}/{target}");
            if path_within(path, &combined) {
                return true;
            }
        }
    }
    // Check wildcard pattern like *.log or .env*
    if let Some(prefix) = target.strip_suffix('*') {
        let norm_p = norm_path(path);
        let norm_pre = norm_path(prefix);
        if norm_p.starts_with(&norm_pre) {
            return true;
        }
        let file_name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        if file_name.starts_with(prefix) {
            return true;
        }
    }
    if let Some(suffix) = target.strip_prefix('*') {
        let norm_p = norm_path(path);
        let norm_suf = norm_path(suffix);
        if norm_p.ends_with(&norm_suf) {
            return true;
        }
        let file_name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        if file_name.to_ascii_lowercase().ends_with(&suffix.to_ascii_lowercase()) {
            return true;
        }
    }
    false
}

// Whether a grant covers (action, target).
// Supports `<action>(<target>)` format (e.g. `command(npm test)`, `write_file(/a/b)`),
// `unsandboxed(...)`, `command(*)` wildcard, and plain command rules without parentheses.
pub fn grant_covers(grant: &str, action: &str, target: &str, ws_paths: &[String]) -> bool {
    let grant = grant.trim();
    if let Some((g_action, g_target)) = parse_grant(grant) {
        let action_match = g_action == action || (action == "command" && g_action == "unsandboxed");
        if !action_match {
            return false;
        }
        if g_target == "*" {
            return true;
        }
        match action {
            "command" => command_rule_covers(g_target, target),
            "write_file" | "read_file" => path_matches_target(target, g_target, ws_paths),
            "read_url" => domain_covers(g_target, target),
            "mcp" => {
                if let Some(prefix) = g_target.strip_suffix("/*") {
                    target.strip_prefix(prefix).is_some_and(|rest| rest.starts_with('/'))
                } else {
                    target == g_target
                }
            }
            _ => target == g_target,
        }
    } else if action == "command" {
        command_rule_covers(grant, target)
    } else if action == "read_url" {
        domain_covers(grant, target)
    } else {
        grant == target
    }
}

// Implicit permission rules per official docs (antigravity.google/docs/permissions):
// - Write implies read: Allowing write_file on a path automatically grants read_file.
// - Deny read implies deny write: Denying read_file on a path immediately blocks write_file.
fn grant_matches_allow(grant: &str, action: &str, target: &str, ws_paths: &[String]) -> bool {
    if grant_covers(grant, action, target, ws_paths) {
        return true;
    }
    if action == "read_file" {
        return grant_covers(grant, "write_file", target, ws_paths);
    }
    false
}

fn grant_matches_deny(grant: &str, action: &str, target: &str, ws_paths: &[String]) -> bool {
    if grant_covers(grant, action, target, ws_paths) {
        return true;
    }
    if action == "write_file" {
        return grant_covers(grant, "read_file", target, ws_paths);
    }
    false
}

fn first_str<'a>(args: &'a Value, keys: &[&str]) -> &'a str {
    keys.iter().find_map(|k| args.get(*k).and_then(Value::as_str)).unwrap_or("")
}

// Map an Antigravity toolCall to:
// - Claude Code's (tool_name, tool_input) for asks.rs and events.rs
// - Antigravity's permission grant (action, target) when the tool requires
//   permissions in EnsurePermissions (None for internal tools like
//   task_boundary, view_file_outline, ask_question, etc.).
fn classify_tool(raw_tool: &str, raw_args: &Value) -> (String, Value, Option<(&'static str, String)>) {
    match raw_tool {
        "run_command" | "pwsh" | "bash" => {
            let cmd = first_str(raw_args, &["CommandLine", "command"]);
            let desc = first_str(raw_args, &["toolSummary", "toolAction", "description"]);
            let mut input = json!({ "command": cmd });
            if !desc.is_empty() {
                input["description"] = json!(desc);
            }
            let rule = cmd.trim().to_string();
            ("Bash".into(), input, Some(("command", rule)))
        }
        "send_command_input" => {
            let cmd = first_str(raw_args, &["Input", "CommandId"]);
            let desc = first_str(raw_args, &["toolSummary", "toolAction"]);
            let mut input = json!({ "command": cmd });
            if !desc.is_empty() {
                input["description"] = json!(desc);
            }
            ("Bash".into(), input, None)
        }
        "write_file" | "write_to_file" => {
            let path = first_str(raw_args, &["Path", "TargetFile", "file_path"]);
            let content = first_str(raw_args, &["Content", "CodeContent", "content"]);
            ("Write".into(), json!({ "file_path": path, "content": content }), Some(("write_file", path.into())))
        }
        "edit_file" | "replace_file_content" | "multi_replace_file_content" | "sed_file" => {
            let path = first_str(raw_args, &["Path", "TargetFile", "file_path"]);
            let (mut old, mut new) = (
                first_str(raw_args, &["Target", "TargetContent", "old_string"]).to_string(),
                first_str(raw_args, &["Replacement", "ReplacementContent", "new_string"]).to_string(),
            );
            if old.is_empty() && new.is_empty() {
                if let Some(chunk) = raw_args.get("ReplacementChunks").and_then(Value::as_array).and_then(|a| a.first()) {
                    old = first_str(chunk, &["TargetContent"]).to_string();
                    new = first_str(chunk, &["ReplacementContent"]).to_string();
                }
            }
            ("Edit".into(), json!({ "file_path": path, "old_string": old, "new_string": new }), Some(("write_file", path.into())))
        }
        "notebook_edit" => {
            let path = first_str(raw_args, &["NotebookPath", "Path", "TargetFile", "notebook_path"]);
            let src = first_str(raw_args, &["NewSource", "Content", "new_source"]);
            ("NotebookEdit".into(), json!({ "notebook_path": path, "new_source": src }), Some(("write_file", path.into())))
        }
        "read_file" | "view_file" | "list_dir" | "view_code_item" => {
            let path = first_str(raw_args, &["Path", "File", "AbsolutePath", "DirectoryPath", "file_path"]);
            ("Read".into(), json!({ "file_path": path }), Some(("read_file", path.into())))
        }
        "grep_search" => {
            let path = first_str(raw_args, &["SearchPath", "Path", "SearchDirectory", "Directory", "path"]);
            let pat = first_str(raw_args, &["Query", "pattern"]);
            ("Grep".into(), json!({ "path": path, "pattern": pat }), Some(("read_file", path.into())))
        }
        "find_by_name" => {
            let path = first_str(raw_args, &["SearchDirectory", "DirectoryPath", "Path", "Directory", "path"]);
            let pat = first_str(raw_args, &["Pattern", "pattern"]);
            ("Glob".into(), json!({ "path": path, "pattern": pat }), Some(("read_file", path.into())))
        }
        "read_url_content" => {
            let url = first_str(raw_args, &["Url", "url"]);
            let domain = extract_domain(url);
            ("WebFetch".into(), json!({ "url": url }), Some(("read_url", domain.into())))
        }
        "call_mcp_tool" => {
            let server = first_str(raw_args, &["ServerName", "server"]);
            let tool = first_str(raw_args, &["ToolName", "tool"]);
            let args = raw_args.get("Arguments").cloned().unwrap_or_else(|| raw_args.clone());
            let name = if !server.is_empty() && !tool.is_empty() { format!("mcp__{server}__{tool}") } else { raw_tool.into() };
            let target = if !server.is_empty() && !tool.is_empty() { format!("{server}/{tool}") } else { "*".into() };
            (name, args, Some(("mcp", target)))
        }
        other if other.starts_with("mcp_") => {
            let target = if let Some(rest) = other.strip_prefix("mcp__") {
                rest.split_once("__").map_or_else(|| rest.replace('_', "/"), |(srv, tl)| format!("{srv}/{tl}"))
            } else if let Some(rest) = other.strip_prefix("mcp_") {
                rest.split_once('_').map_or_else(|| rest.to_string(), |(srv, tl)| format!("{srv}/{tl}"))
            } else {
                other.to_string()
            };
            (other.into(), raw_args.clone(), Some(("mcp", target)))
        }
        other => (other.into(), raw_args.clone(), None),
    }
}

// One kind of rule (allow, ask, deny) from a set of grants, as written.
fn rules_of<'a>(grants: &'a Value, key: &str) -> impl Iterator<Item = &'a str> {
    grants.get(key).and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str)
}

// Whether a PreToolUse event in Antigravity will actually prompt the person in EnsurePermissions:
// - Internal tools (ask_question, task_boundary, search_web, view_file_outline) never prompt.
// - `--dangerously-skip-permissions` skips all tool prompts.
// - Strict mode yields directly to agy (decision: allow) without prompting on island.
// - Evaluation order: Deny > Strict > Ask > Allow > Defaults.
// - CLI settings (~/.gemini/antigravity-cli/settings.json):
//   - toolPermission: "always-proceed", "request-review", "strict", "proceed-in-sandbox"
//   - enableTerminalSandbox: true (auto-allows commands in sandbox under proceed-in-sandbox)
//   - allowNonWorkspaceAccess (auto-allows reading outside workspace)
//   - permissions: { allow, ask, deny }
// - Desktop config (~/.gemini/config/config.json):
//   - permissionPreset: "AGENT_PERMISSION_PRESET_TURBO", "AGENT_PERMISSION_PRESET_DEFAULT", etc.
//   - autoExecutionPolicy: "CASCADE_COMMANDS_AUTO_EXECUTION_EAGER"
//   - globalPermissionGrants: { allow, ask, deny }, which agy CLI honours as well
pub fn needs_approval(e: &Value, config: &Value, cli: &Value) -> bool {
    let call = e.get("toolCall").unwrap_or(&Value::Null);
    let raw_tool = s(call, "name");
    let raw_args = call.get("args").unwrap_or(&Value::Null);
    let (_, _, Some((action, target))) = classify_tool(raw_tool, raw_args) else {
        return false;
    };

    if e.get("dangerously_skip_permissions").and_then(Value::as_bool) == Some(true) {
        return false;
    }

    let is_cli = is_cli_event(e);
    let ws_paths: Vec<String> = e
        .get("workspacePaths")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect())
        .unwrap_or_default();

    let settings = config.get("userSettings").unwrap_or(&Value::Null);
    let desktop_grants = settings.get("globalPermissionGrants").unwrap_or(&Value::Null);
    let cli_perms = cli.get("permissions").unwrap_or(&Value::Null);

    // A CLI session's rules are its settings.json's and also the shared ones in
    // config.json: agy CLI reads both, deny and ask too ("stored shared config
    // permissions: allow=… deny=… ask=…" in its log). A desktop session's are
    // config.json's.
    let rules = |key: &str| -> Vec<&str> {
        let cli_rules: Vec<&str> = if is_cli { rules_of(cli_perms, key).collect() } else { Vec::new() };
        cli_rules.into_iter().chain(rules_of(desktop_grants, key)).collect()
    };
    let (grants_deny, grants_ask, grants_allow) = (rules("deny"), rules("ask"), rules("allow"));

    // 1. Deny rules: Deny > Strict > Ask > Allow.
    // If matching deny, return false (Antigravity natively blocks it without asking).
    let is_denied = |cmd_or_target: &str| grants_deny.iter().any(|g| grant_matches_deny(g, action, cmd_or_target, &ws_paths));
    if is_denied(&target) {
        return false;
    }

    // 2. Strict mode: in strict mode, agy CLI rejects auto_approve hook decisions
    // ("soft-denying tool confirmation") and reprompts on the terminal anyway.
    // Return false immediately so it yields to agy natively without showing on island.
    if is_cli {
        let tool_perm = cli.get("toolPermission").and_then(Value::as_str).unwrap_or("request-review");
        if tool_perm == "strict" {
            return false;
        }
    }

    // 3. Ask rules: Ask > Allow.
    let is_asked = |cmd_or_target: &str| grants_ask.iter().any(|g| grant_covers(g, action, cmd_or_target, &ws_paths));
    if is_asked(&target) {
        return true;
    }

    // 4. Allow rules: exact full-line match (commands must match entire line or command(*)).
    let is_allowed = |cmd_or_target: &str| grants_allow.iter().any(|g| grant_matches_allow(g, action, cmd_or_target, &ws_paths));
    if is_allowed(&target) {
        return false;
    }

    // 5. Global bypass presets
    if is_cli {
        let tool_perm = cli.get("toolPermission").and_then(Value::as_str).unwrap_or("request-review");
        if tool_perm == "always-proceed" {
            return false;
        }
    } else {
        let preset = s(settings, "permissionPreset");
        if preset == "AGENT_PERMISSION_PRESET_TURBO" {
            return false;
        }
    }

    // 6. Action-specific and mode defaults
    match action {
        "read_file" => {
            if is_workspace_or_artifact_path(e, &target) {
                return false;
            }
            if is_cli {
                cli.get("allowNonWorkspaceAccess").and_then(Value::as_bool) != Some(true)
            } else {
                s(settings, "nonWorkspaceFileAccessPolicy") != "AGENT_SETTING_POLICY_ALLOW"
            }
        }
        "write_file" => {
            let inside_ws = is_workspace_or_artifact_path(e, &target);
            if is_cli {
                let agent_mode = e
                    .get("cli_mode")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .or_else(|| cli.get("agentMode").and_then(Value::as_str))
                    .unwrap_or("");
                if inside_ws {
                    // In CLI: "accept-edits" and "auto" allow workspace file edits without asking.
                    // Default mode "" in request-review asks for write_file even inside workspace.
                    agent_mode != "accept-edits" && agent_mode != "auto"
                } else {
                    let non_ws = cli.get("allowNonWorkspaceAccess").and_then(Value::as_bool) == Some(true);
                    !(non_ws && (agent_mode == "accept-edits" || agent_mode == "auto"))
                }
            } else {
                // Desktop App:
                // Workspace read/write is allowed without prompting under AGENT_PERMISSION_PRESET_DEFAULT.
                // Only non-workspace file access requires approval.
                !inside_ws
            }
        }
        "command" => {
            if is_cli {
                let tool_perm = cli.get("toolPermission").and_then(Value::as_str).unwrap_or("request-review");
                let sandbox = cli.get("enableTerminalSandbox").and_then(Value::as_bool) == Some(true)
                    || e.get("cli_sandbox").and_then(Value::as_bool) == Some(true);
                if tool_perm == "proceed-in-sandbox" && sandbox {
                    return false;
                }
                true
            } else {
                let policy = s(settings, "autoExecutionPolicy");
                if policy == "CASCADE_COMMANDS_AUTO_EXECUTION_EAGER" {
                    return false;
                }
                let sandbox_enabled = settings.get("enableTerminalSandbox").and_then(Value::as_bool) == Some(true);
                if policy == "CASCADE_COMMANDS_AUTO_EXECUTION_PROCEED_IN_SANDBOX" && sandbox_enabled {
                    return false;
                }
                true
            }
        }
        "read_url" | "mcp" => true,
        _ => false,
    }
}

// A tool call needing approval in Antigravity (PreToolUse): worded for asks.rs
// as Claude Code's PermissionRequest so she shows it on her island/panel.
pub fn for_ask(e: &Value) -> Value {
    let call = e.get("toolCall").cloned().unwrap_or_else(|| json!({}));
    let raw_tool = call.get("name").and_then(Value::as_str).unwrap_or("");
    let raw_args = call.get("args").cloned().unwrap_or_else(|| json!({}));
    let (tool, input, grant) = classify_tool(raw_tool, &raw_args);
    let session = s(e, "conversationId");
    let cwd = e.get("workspacePaths").and_then(Value::as_array).and_then(|a| a.first()).and_then(Value::as_str).unwrap_or("");
    let step = e.get("stepIdx").and_then(Value::as_i64).map(|v| v.to_string()).unwrap_or_default();
    let mut out = json!({
        "hook_event_name": "PermissionRequest",
        "session_id": session,
        "conversationId": session,
        "cwd": cwd,
        "agent": "agy",
        "tool_name": tool,
        "tool_input": input,
        "tool_use_id": format!("{session}-{step}"),
    });
    if let Some((action, target)) = grant {
        if !target.is_empty() {
            out["permission_suggestions"] = json!([{
                "type": "addRules",
                "behavior": "allow",
                "rules": [{ "toolName": action, "ruleContent": target }],
                "destination": "userSettings"
            }]);
        }
    }
    out
}

// Add session_id (and on PostToolUse the normalized tool_use_id, tool_name and
// tool_input) to an Antigravity event so asks.seen() settles any prompt
// answered in Antigravity's client or ended by Stop.
pub fn with_seen(e: &mut Value) {
    let session = s(e, "conversationId").to_string();
    if session.is_empty() {
        return;
    }
    let is_post = s(e, "hook_event_name") == "PostToolUse";
    let step = e.get("stepIdx").and_then(Value::as_i64).map(|v| v.to_string());
    let mapped = is_post.then(|| {
        let call = e.get("toolCall").unwrap_or(&Value::Null);
        classify_tool(s(call, "name"), call.get("args").unwrap_or(&Value::Null))
    });
    if let Some(fields) = e.as_object_mut() {
        fields.entry("session_id".to_string()).or_insert_with(|| json!(session));
        if let Some((tool, input, _)) = mapped {
            fields.entry("tool_name".to_string()).or_insert_with(|| json!(tool));
            fields.entry("tool_input".to_string()).or_insert(input);
            if let Some(step) = step {
                fields.entry("tool_use_id".to_string()).or_insert_with(|| json!(format!("{session}-{step}")));
            }
        }
    }
}

fn atomic_write_file(file: &Path, content: &str) -> std::io::Result<()> {
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp_file = file.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(&tmp_file, content)?;
    if std::fs::rename(&tmp_file, file).is_err() {
        let res = std::fs::write(file, content);
        let _ = std::fs::remove_file(&tmp_file);
        return res;
    }
    Ok(())
}

// Persist "Always Allow" grants into ~/.gemini/config/config.json
// (userSettings.globalPermissionGrants.allow), which language_server watches.
pub fn save_always_grants_to_config(file: &Path, rules: &[String]) {
    if rules.is_empty() {
        return;
    }
    let mut root: Value = if file.exists() {
        let Ok(text) = std::fs::read_to_string(file) else { return };
        let Ok(val) = serde_json::from_str(&text) else { return };
        val
    } else {
        json!({})
    };
    let Some(root_obj) = root.as_object_mut() else { return };
    let settings = root_obj.entry("userSettings").or_insert_with(|| json!({}));
    let Some(settings_obj) = settings.as_object_mut() else { return };
    let grants = settings_obj.entry("globalPermissionGrants").or_insert_with(|| json!({}));
    let Some(grants_obj) = grants.as_object_mut() else { return };
    let allow = grants_obj.entry("allow").or_insert_with(|| json!([]));
    let Some(arr) = allow.as_array_mut() else { return };
    let mut changed = false;
    for rule in rules {
        if parse_grant(rule).is_some() && !arr.iter().any(|v| v.as_str() == Some(rule.as_str())) {
            arr.push(json!(rule));
            changed = true;
        }
    }
    if changed {
        if let Ok(text) = serde_json::to_string_pretty(&root) {
            let _ = atomic_write_file(file, &format!("{text}\n"));
        }
    }
}

// Persist "Always Allow" grants into ~/.gemini/antigravity-cli/settings.json
// (permissions.allow), which `agy` CLI reads natively.
pub fn save_always_grants_to_cli(file: &Path, rules: &[String]) {
    if rules.is_empty() {
        return;
    }
    let mut root: Value = if file.exists() {
        let Ok(text) = std::fs::read_to_string(file) else { return };
        let Ok(val) = serde_json::from_str(&text) else { return };
        val
    } else {
        json!({})
    };
    let Some(root_obj) = root.as_object_mut() else { return };
    let perms = root_obj.entry("permissions").or_insert_with(|| json!({}));
    let Some(perms_obj) = perms.as_object_mut() else { return };
    let allow = perms_obj.entry("allow").or_insert_with(|| json!([]));
    let Some(arr) = allow.as_array_mut() else { return };
    let mut changed = false;
    for rule in rules {
        if !arr.iter().any(|v| v.as_str() == Some(rule.as_str())) {
            arr.push(json!(rule));
            changed = true;
        }
    }
    if changed {
        if let Ok(text) = serde_json::to_string_pretty(&root) {
            let _ = atomic_write_file(file, &format!("{text}\n"));
        }
    }
}

fn extract_rules(perms: &[Value]) -> Vec<String> {
    let mut rules = Vec::new();
    for p in perms {
        if let Some(arr) = p.get("rules").and_then(Value::as_array) {
            for r in arr {
                if let Some(s) = r.as_str() {
                    if !s.is_empty() {
                        rules.push(s.to_string());
                    }
                } else if r.is_object() {
                    let tool = s(r, "toolName");
                    let content = s(r, "ruleContent");
                    if !tool.is_empty() {
                        rules.push(if content.is_empty() { tool.to_string() } else { format!("{tool}({content})") });
                    }
                }
            }
        }
    }
    rules
}

// What to return to Antigravity on stdout for her decision:
// - Allow once -> {"decision":"auto_approve"} (bypasses Antigravity's native dialog)
// - Always allow -> {"decision":"auto_approve"} + saved to config.json & settings.json
// - Deny -> {"decision":"deny","reason":"..."}
// - Dismiss / timeout / out of sight -> {"decision":"allow"} (defers to Antigravity's native dialog)
pub fn to_output(body: &str) -> String {
    #[cfg(not(test))]
    let (cfg, cli) = (Some(config_file()), Some(cli_settings_file()));
    #[cfg(test)]
    let (cfg, cli): (Option<PathBuf>, Option<PathBuf>) = (None, None);
    to_output_with(body, cfg.as_deref(), cli.as_deref())
}

pub fn to_output_with(body: &str, config: Option<&Path>, cli: Option<&Path>) -> String {
    let Ok(val) = serde_json::from_str::<Value>(body) else {
        return r#"{"decision":"allow"}"#.to_string();
    };
    let Some(decision) = val.get("hookSpecificOutput").and_then(|h| h.get("decision")) else {
        return r#"{"decision":"allow"}"#.to_string();
    };
    match decision.get("behavior").and_then(Value::as_str) {
        Some("allow") => {
            if let Some(perms) = decision.get("updatedPermissions").and_then(Value::as_array) {
                let rules = extract_rules(perms);
                if !rules.is_empty() {
                    if let Some(file) = config {
                        save_always_grants_to_config(file, &rules);
                    }
                    if let Some(cli_file) = cli {
                        save_always_grants_to_cli(cli_file, &rules);
                    }
                }
            }
            r#"{"decision":"auto_approve"}"#.to_string()
        }
        Some("deny") => {
            let msg = decision.get("message").and_then(Value::as_str).unwrap_or("The user denied this action on the Dynamic Island.");
            json!({ "decision": "deny", "reason": msg }).to_string()
        }
        _ => r#"{"decision":"allow"}"#.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // As agy sends them (trimmed), with is_cli: true representing CLI sessions.
    fn event(name: &str, more: Value) -> Value {
        let mut e = json!({
            "hook_event_name": name,
            "conversationId": "5064aa2e",
            "modelName": "gemini-3.8-flash-high",
            "workspacePaths": ["Z:/nowhere/agyws"],
            "transcriptPath": "Z:/nowhere/agyws/.gemini/antigravity-cli/transcript.jsonl",
            "is_cli": true,
        });
        if let (Some(base), Some(extra)) = (e.as_object_mut(), more.as_object()) {
            for (k, v) in extra {
                base.insert(k.clone(), v.clone());
            }
        }
        e
    }

    #[test]
    fn turns_begin_and_end_in_her_log() {
        let line = |name: &str, more: Value| log_line(&event(name, more));
        assert_eq!(line("SessionStart", json!({})).as_deref(), Some("agy SessionStart (5064aa2e)"));
        assert_eq!(line("PreInvocation", json!({ "invocationNum": 0, "userStep": 6 })).as_deref(), Some("agy PreInvocation 0, the person's step 6 (5064aa2e)"));
        assert_eq!(line("PermissionRequest", json!({ "tool_name": "Bash" })).as_deref(), Some("agy PermissionRequest Bash (5064aa2e)"));
        assert_eq!(line("Stop", json!({ "fullyIdle": false, "terminationReason": "NO_TOOL_CALL", "error": "" })).as_deref(), Some("agy Stop, fullyIdle false, NO_TOOL_CALL (5064aa2e)"));
        assert!(line("PreInvocation", json!({ "invocationNum": 1 })).is_none() && line("PostToolUse", json!({})).is_none());
    }

    #[test]
    fn agy_pre_tool_use_becomes_ask_and_replies() {
        let call = json!({ "name": "run_command", "args": { "CommandLine": "npm test", "toolSummary": "Run tests" } });
        let pre = json!({ "hook_event_name": "PreToolUse", "conversationId": "5064aa2e", "workspacePaths": ["Z:/nowhere/agyws"], "is_cli": true, "toolCall": call, "stepIdx": 5 });
        let ask = for_ask(&pre);
        assert_eq!((ask["hook_event_name"].as_str(), ask["tool_name"].as_str()), (Some("PermissionRequest"), Some("Bash")));
        assert_eq!(ask["tool_input"]["command"], "npm test");
        assert_eq!(ask["tool_input"]["description"], "Run tests");
        assert_eq!(ask["permission_suggestions"][0]["rules"][0], json!({ "toolName": "command", "ruleContent": "npm test" }));
        assert_eq!(ask["permission_suggestions"][0]["destination"], "userSettings");

        // asks.rs view_of and reply_for round-trip
        let view = agent_core::asks::view_of(&ask).unwrap();
        assert_eq!(view["always"]["rules"], json!(["command(npm test)"]));
        assert_eq!(view["always"]["where"], json!(["userSettings"]));

        // Allow once -> auto_approve (bypasses Antigravity's native dialog)
        let allow = agent_core::asks::reply_for(&ask, &json!({ "action": "allow" }), "denied").unwrap();
        assert_eq!(to_output(&allow.to_string()), r#"{"decision":"auto_approve"}"#);

        // Always allow -> auto_approve (without permissionOverrides) + persisted to BOTH config.json and settings.json
        let cfg_file = std::env::temp_dir().join(format!("wakuwaku-agy-cfg-{}.json", std::process::id()));
        let cli_file = std::env::temp_dir().join(format!("wakuwaku-agy-cli-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&cfg_file);
        let _ = std::fs::remove_file(&cli_file);
        let always = agent_core::asks::reply_for(&ask, &json!({ "action": "always" }), "denied").unwrap();
        assert_eq!(
            to_output_with(&always.to_string(), Some(&cfg_file), Some(&cli_file)),
            r#"{"decision":"auto_approve"}"#
        );
        let saved_cfg: Value = serde_json::from_str(&std::fs::read_to_string(&cfg_file).unwrap()).unwrap();
        assert_eq!(saved_cfg["userSettings"]["globalPermissionGrants"]["allow"], json!(["command(npm test)"]));
        let saved_cli: Value = serde_json::from_str(&std::fs::read_to_string(&cli_file).unwrap()).unwrap();
        assert_eq!(saved_cli["permissions"]["allow"], json!(["command(npm test)"]));

        // Once saved, needs_approval is false for `npm test`, but true for `npm test --watch` (exact full-line matching)
        let empty_cfg = json!({});
        assert!(!needs_approval(&pre, &empty_cfg, &saved_cli));
        let pre_watch = json!({ "hook_event_name": "PreToolUse", "conversationId": "5064aa2e", "workspacePaths": ["Z:/nowhere/agyws"], "is_cli": true, "toolCall": { "name": "run_command", "args": { "CommandLine": "npm test --watch" } } });
        assert!(needs_approval(&pre_watch, &empty_cfg, &saved_cli));
        let pre_other = json!({ "hook_event_name": "PreToolUse", "conversationId": "5064aa2e", "workspacePaths": ["Z:/nowhere/agyws"], "is_cli": true, "toolCall": { "name": "run_command", "args": { "CommandLine": "cargo build" } } });
        assert!(needs_approval(&pre_other, &empty_cfg, &saved_cli));
        let _ = std::fs::remove_file(&cfg_file);
        let _ = std::fs::remove_file(&cli_file);

        // Deny -> deny + reason
        let deny = agent_core::asks::reply_for(&ask, &json!({ "action": "deny" }), "denied by person").unwrap();
        assert_eq!(to_output(&deny.to_string()), r#"{"decision":"deny","reason":"denied by person"}"#);
        let deny_fallback = json!({ "hookSpecificOutput": { "decision": { "behavior": "deny" } } });
        assert_eq!(to_output(&deny_fallback.to_string()), r#"{"decision":"deny","reason":"The user denied this action on the Dynamic Island."}"#);

        // Dismiss / Timeout / Out of sight -> allow (defers to Antigravity's native dialog)
        assert_eq!(to_output("{}"), r#"{"decision":"allow"}"#);
    }

    #[test]
    fn agy_needs_approval_filters_auto_allowed_tools_and_modes() {
        let default_cfg = json!({});
        let default_cli = json!({});

        // Command in default CLI mode (request-review) needs approval
        let cmd = event("PreToolUse", json!({ "toolCall": { "name": "run_command", "args": { "CommandLine": "git status" } } }));
        assert!(needs_approval(&cmd, &default_cfg, &default_cli));

        // CLI settings.json: toolPermission == "always-proceed" auto-allows commands and tools
        let cli_always = json!({ "toolPermission": "always-proceed" });
        assert!(!needs_approval(&cmd, &default_cfg, &cli_always));

        // CLI settings.json: permissions.allow matches rules to auto-allow
        let cli_allowed = json!({ "permissions": { "allow": ["command(git status)"] } });
        assert!(!needs_approval(&cmd, &default_cfg, &cli_allowed));
        let cli_plain_allowed = json!({ "permissions": { "allow": ["git status"] } });
        assert!(!needs_approval(&cmd, &default_cfg, &cli_plain_allowed));

        // Desktop config.json should NOT cross-pollinate into CLI sessions:
        // When Desktop is in Turbo, CLI still requires approval!
        let turbo_cfg = json!({ "userSettings": { "permissionPreset": "AGENT_PERMISSION_PRESET_TURBO" } });
        assert!(needs_approval(&cmd, &turbo_cfg, &default_cli));
        // But for a Desktop session (is_cli: false), Turbo preset auto-allows commands
        let desktop_cmd = json!({ "hook_event_name": "PreToolUse", "conversationId": "5064aa2e", "workspacePaths": ["Z:/nowhere/agyws"], "transcriptPath": "Z:/brain/transcript.jsonl", "toolCall": { "name": "run_command", "args": { "CommandLine": "git status" } } });
        assert!(!needs_approval(&desktop_cmd, &turbo_cfg, &default_cli));
        // config.json's shared grants count for a CLI session too (agy CLI reads
        // them): allowed there, it runs unasked; denied there, agy refuses it
        // itself, so neither comes to the island.
        let shared_allow = json!({ "userSettings": { "globalPermissionGrants": { "allow": ["command(git status)"] } } });
        assert!(!needs_approval(&cmd, &shared_allow, &default_cli));
        let shared_deny = json!({ "userSettings": { "globalPermissionGrants": { "deny": ["command(git status)"] } } });
        assert!(!needs_approval(&cmd, &shared_deny, &default_cli));
        assert!(!needs_approval(&cmd, &shared_deny, &cli_allowed));

        // In CLI request-review mode:
        // - File write inside workspace STILL NEEDS APPROVAL when agentMode is default ""
        let in_ws_write = event("PreToolUse", json!({ "toolCall": { "name": "write_to_file", "args": { "TargetFile": r"Z:\nowhere\agyws\src\lib.rs" } } }));
        assert!(needs_approval(&in_ws_write, &default_cfg, &default_cli));

        // - In CLI agentMode "accept-edits" or "auto": workspace edits are auto-allowed without asking
        // - In CLI with --mode accept-edits or auto: workspace edits are auto-allowed without asking
        let in_ws_write_mode = event("PreToolUse", json!({ "cli_mode": "accept-edits", "toolCall": { "name": "write_to_file", "args": { "TargetFile": r"Z:\nowhere\agyws\src\lib.rs" } } }));
        assert!(!needs_approval(&in_ws_write_mode, &default_cfg, &default_cli));

        // - File write outside workspace needs approval
        let out_ws_write = event("PreToolUse", json!({ "toolCall": { "name": "write_to_file", "args": { "TargetFile": r"C:\outside\secret.txt" } } }));
        assert!(needs_approval(&out_ws_write, &default_cfg, &default_cli));

        // - File write in CLI always-proceed is auto-allowed
        assert!(!needs_approval(&in_ws_write, &default_cfg, &cli_always));

        // - In Desktop session: workspace read/write is allowed by default preset
        let desktop_in_ws_write = json!({ "hook_event_name": "PreToolUse", "conversationId": "5064aa2e", "workspacePaths": ["Z:/nowhere/agyws"], "transcriptPath": "Z:/brain/transcript.jsonl", "toolCall": { "name": "write_to_file", "args": { "TargetFile": r"Z:\nowhere\agyws\src\lib.rs" } } });
        assert!(!needs_approval(&desktop_in_ws_write, &default_cfg, &default_cli));

        // - File read inside workspace is auto-allowed
        let in_ws_read = event("PreToolUse", json!({ "toolCall": { "name": "read_file", "args": { "Path": r"Z:\nowhere\agyws\src\lib.rs" } } }));
        assert!(!needs_approval(&in_ws_read, &default_cfg, &default_cli));

        // - File read outside workspace needs approval by default, unless allowNonWorkspaceAccess is true
        let out_ws_read = event("PreToolUse", json!({ "toolCall": { "name": "read_file", "args": { "Path": r"C:\outside\secret.txt" } } }));
        assert!(needs_approval(&out_ws_read, &default_cfg, &default_cli));
        let cli_non_ws = json!({ "allowNonWorkspaceAccess": true });
        assert!(!needs_approval(&out_ws_read, &default_cfg, &cli_non_ws));

        // Internal tools never need EnsurePermissions approval
        for tool in ["ask_question", "task_boundary", "view_file_outline", "search_web"] {
            let internal = event("PreToolUse", json!({ "toolCall": { "name": tool, "args": {} } }));
            assert!(!needs_approval(&internal, &default_cfg, &default_cli), "{tool}");
        }
    }

    #[test]
    fn rule_precedence_and_compound_commands() {
        let default_cfg = json!({});
        let default_cli = json!({});

        // Priority 1: Deny > Strict > Ask > Allow
        let mixed_cli = json!({
            "permissions": {
                "deny": ["command(rm -rf foo)"],
                "ask": ["command(*)"],
                "allow": ["command(git)"]
            }
        });
        // "rm -rf foo" matches deny -> returns false (blocked without prompt)
        let rm_cmd = event("PreToolUse", json!({ "toolCall": { "name": "run_command", "args": { "CommandLine": "rm -rf foo" } } }));
        assert!(!needs_approval(&rm_cmd, &default_cfg, &mixed_cli));

        // "ls" matches ask(*) -> returns true (needs approval)
        let ls_cmd = event("PreToolUse", json!({ "toolCall": { "name": "run_command", "args": { "CommandLine": "ls" } } }));
        assert!(needs_approval(&ls_cmd, &default_cfg, &mixed_cli));

        // Priority 2: strict mode overrides allow -> yields directly to agy without island prompt
        let strict_cli = json!({
            "toolPermission": "strict",
            "permissions": { "allow": ["command(git status)"] }
        });
        let git_cmd = event("PreToolUse", json!({ "toolCall": { "name": "run_command", "args": { "CommandLine": "git status" } } }));
        assert!(!needs_approval(&git_cmd, &default_cfg, &strict_cli));

        // Exact line matching for commands:
        // "command(echo AAA)" does NOT cover "echo AAA BBB", and "command(echo *)" does NOT wildcard
        let echo_cli = json!({
            "permissions": { "allow": ["command(echo AAA)", "command(echo *)"] }
        });
        let echo_exact = event("PreToolUse", json!({ "toolCall": { "name": "run_command", "args": { "CommandLine": "echo AAA" } } }));
        assert!(!needs_approval(&echo_exact, &default_cfg, &echo_cli));
        let echo_sub = event("PreToolUse", json!({ "toolCall": { "name": "run_command", "args": { "CommandLine": "echo AAA BBB" } } }));
        assert!(needs_approval(&echo_sub, &default_cfg, &echo_cli));

        // "command(whoami)" does NOT cover "whoami /user", "command(whoami /user)" does
        let whoami_cli = json!({
            "permissions": { "allow": ["command(whoami)"] }
        });
        let whoami_user = event("PreToolUse", json!({ "toolCall": { "name": "run_command", "args": { "CommandLine": "whoami /user" } } }));
        assert!(needs_approval(&whoami_user, &default_cfg, &whoami_cli));
        let whoami_user_cli = json!({
            "permissions": { "allow": ["command(whoami /user)"] }
        });
        assert!(!needs_approval(&whoami_user, &default_cfg, &whoami_user_cli));

        // Compound command exact matching:
        // "echo AAA; echo BBB": separate rules ["command(echo AAA)", "command(echo BBB)"] or "command(echo)" do NOT allow it
        let split_rules_cli = json!({
            "permissions": { "allow": ["command(echo AAA)", "command(echo BBB)", "command(echo)"] }
        });
        let compound_cmd = event("PreToolUse", json!({ "toolCall": { "name": "run_command", "args": { "CommandLine": "echo AAA; echo BBB" } } }));
        assert!(needs_approval(&compound_cmd, &default_cfg, &split_rules_cli));

        // But exact whole-line rule "command(echo AAA; echo BBB)" DOES allow it
        let exact_compound_cli = json!({
            "permissions": { "allow": ["command(echo AAA; echo BBB)"] }
        });
        assert!(!needs_approval(&compound_cmd, &default_cfg, &exact_compound_cli));

        // Compound command with Always Allow saves exact full-line rule and re-run does not prompt
        let pre_compound = json!({
            "hook_event_name": "PreToolUse",
            "conversationId": "5064aa2e",
            "workspacePaths": ["Z:/nowhere/agyws"],
            "is_cli": true,
            "toolCall": { "name": "run_command", "args": { "CommandLine": "echo AAA; echo BBB" } },
            "stepIdx": 7
        });
        let ask_compound = for_ask(&pre_compound);
        assert_eq!(ask_compound["permission_suggestions"][0]["rules"][0], json!({ "toolName": "command", "ruleContent": "echo AAA; echo BBB" }));
        let always_reply = agent_core::asks::reply_for(&ask_compound, &json!({ "action": "always" }), "denied").unwrap();
        let cfg_tmp = std::env::temp_dir().join(format!("waku-comp-cfg-{}.json", std::process::id()));
        let cli_tmp = std::env::temp_dir().join(format!("waku-comp-cli-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&cfg_tmp);
        let _ = std::fs::remove_file(&cli_tmp);
        assert_eq!(
            to_output_with(&always_reply.to_string(), Some(&cfg_tmp), Some(&cli_tmp)),
            r#"{"decision":"auto_approve"}"#
        );
        let saved_comp_cli: Value = serde_json::from_str(&std::fs::read_to_string(&cli_tmp).unwrap()).unwrap();
        assert_eq!(saved_comp_cli["permissions"]["allow"], json!(["command(echo AAA; echo BBB)"]));
        assert!(!needs_approval(&pre_compound, &default_cfg, &saved_comp_cli));
        let _ = std::fs::remove_file(&cfg_tmp);
        let _ = std::fs::remove_file(&cli_tmp);

        // Sandbox settings:
        // enableTerminalSandbox: true allows commands under proceed-in-sandbox
        let sandbox_cli = json!({
            "toolPermission": "proceed-in-sandbox",
            "enableTerminalSandbox": true
        });
        assert!(!needs_approval(&echo_exact, &default_cfg, &sandbox_cli));

        // enableTerminalSandbox: false requires approval under proceed-in-sandbox
        let no_sandbox_cli = json!({
            "toolPermission": "proceed-in-sandbox",
            "enableTerminalSandbox": false
        });
        assert!(needs_approval(&echo_exact, &default_cfg, &no_sandbox_cli));

        // --sandbox command line flag allows commands under proceed-in-sandbox
        let flag_sandbox_cli = json!({
            "toolPermission": "proceed-in-sandbox"
        });
        let cmd_with_sandbox_flag = event("PreToolUse", json!({
            "cli_sandbox": true,
            "toolCall": { "name": "run_command", "args": { "CommandLine": "echo AAA" } }
        }));
        assert!(!needs_approval(&cmd_with_sandbox_flag, &default_cfg, &flag_sandbox_cli));

        // Implicit permission rules:
        // 1. Write implies read
        let write_allow_cli = json!({
            "permissions": { "allow": [r"write_file(C:\outside\dir)"] }
        });
        let read_outside = event("PreToolUse", json!({ "toolCall": { "name": "read_file", "args": { "Path": r"C:\outside\dir\file.txt" } } }));
        assert!(!needs_approval(&read_outside, &default_cfg, &write_allow_cli));

        // 2. Deny read implies deny write
        let read_deny_cli = json!({
            "permissions": { "deny": [r"read_file(C:\outside\secret)"] }
        });
        let write_outside = event("PreToolUse", json!({ "toolCall": { "name": "write_file", "args": { "Path": r"C:\outside\secret\evil.txt" } } }));
        assert!(!needs_approval(&write_outside, &default_cfg, &read_deny_cli));

        // URL domain matching
        let url_allow_cli = json!({
            "permissions": { "allow": ["read_url(github.com)"] }
        });
        let fetch_gh = event("PreToolUse", json!({ "toolCall": { "name": "read_url_content", "args": { "Url": "https://api.github.com/repos" } } }));
        assert!(!needs_approval(&fetch_gh, &default_cfg, &url_allow_cli));
        let fetch_other = event("PreToolUse", json!({ "toolCall": { "name": "read_url_content", "args": { "Url": "https://evil.org/steal" } } }));
        assert!(needs_approval(&fetch_other, &default_cfg, &url_allow_cli));

        // Tool classification for view_code_item
        let code_item = event("PreToolUse", json!({ "toolCall": { "name": "view_code_item", "args": { "File": r"C:\outside\lib.rs" } } }));
        assert!(needs_approval(&code_item, &default_cfg, &default_cli));
    }

    #[test]
    fn agy_with_seen_settles_pending_prompts_on_post_tool_use_and_stop() {
        let pre = event("PreToolUse", json!({ "stepIdx": 5, "toolCall": { "name": "run_command", "args": { "CommandLine": "npm test" } } }));
        let ask = for_ask(&pre);
        let mut asks: agent_core::asks::Asks<Box<dyn FnOnce(Option<Value>) + Send>> = agent_core::asks::Asks::default();
        let id = asks.add(ask, Box::new(|_| {}), 1_000, 60_000).ok().unwrap();

        let mut post = event("PostToolUse", json!({ "stepIdx": 5, "error": "", "toolCall": { "name": "run_command", "args": { "CommandLine": "npm test" } } }));
        with_seen(&mut post);
        assert_eq!(asks.seen(&post), vec![id]);
        assert!(asks.is_empty());
    }

    #[test]
    fn security_and_edge_cases() {
        let default_cfg = json!({});
        let default_cli = json!({});

        // 1. Multiline command: single line does NOT bypass, exact multiline allows
        let multiline = event("PreToolUse", json!({ "toolCall": { "name": "run_command", "args": { "CommandLine": "echo safe\nrm -rf /" } } }));
        let first_line_allow = json!({ "permissions": { "allow": ["command(echo safe)"] } });
        assert!(needs_approval(&multiline, &default_cfg, &first_line_allow));
        let exact_multiline = json!({ "permissions": { "allow": ["command(echo safe\nrm -rf /)"] } });
        assert!(!needs_approval(&multiline, &default_cfg, &exact_multiline));

        // 2. Interactive stdin send_command_input never prompts on island
        let send_input = event("PreToolUse", json!({ "toolCall": { "name": "send_command_input", "args": { "CommandId": "123", "Input": "y" } } }));
        assert!(!needs_approval(&send_input, &default_cfg, &default_cli));

        // 3. Suffix wildcard matching (*.log)
        let log_allow = json!({ "permissions": { "allow": [r"read_file(*.log)"] } });
        let read_log = event("PreToolUse", json!({ "toolCall": { "name": "read_file", "args": { "Path": r"C:\outside\debug.log" } } }));
        assert!(!needs_approval(&read_log, &default_cfg, &log_allow));
        let read_rs = event("PreToolUse", json!({ "toolCall": { "name": "read_file", "args": { "Path": r"C:\outside\debug.rs" } } }));
        assert!(needs_approval(&read_rs, &default_cfg, &log_allow));

        // 4. Relative path directory traversal (..\..\outside) is NOT inside workspace
        let traversal = event("PreToolUse", json!({ "toolCall": { "name": "write_file", "args": { "Path": r"..\..\outside\secret.txt" } } }));
        assert!(needs_approval(&traversal, &default_cfg, &default_cli));

        // Valid relative path inside workspace
        let in_ws_rel = event("PreToolUse", json!({ "cli_mode": "accept-edits", "toolCall": { "name": "write_file", "args": { "Path": r"src\lib.rs" } } }));
        assert!(!needs_approval(&in_ws_rel, &default_cfg, &default_cli));

        // 5. MCP tool naming without mcp(*) wildcard regression
        let mcp_call = json!({ "name": "mcp_github_create_issue", "args": {} });
        let mcp_pre = json!({ "hook_event_name": "PreToolUse", "conversationId": "5064aa2e", "workspacePaths": ["Z:/nowhere/agyws"], "is_cli": true, "toolCall": mcp_call, "stepIdx": 8 });
        let mcp_ask = for_ask(&mcp_pre);
        assert_eq!(mcp_ask["permission_suggestions"][0]["rules"][0], json!({ "toolName": "mcp", "ruleContent": "github/create_issue" }));

        // 6. CLI agentMode fallback from settings.json
        let cli_agent_mode = json!({ "agentMode": "accept-edits" });
        let in_ws_write = event("PreToolUse", json!({ "toolCall": { "name": "write_to_file", "args": { "TargetFile": r"Z:\nowhere\agyws\src\lib.rs" } } }));
        assert!(!needs_approval(&in_ws_write, &default_cfg, &cli_agent_mode));
    }
}
