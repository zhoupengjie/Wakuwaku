// Her exe run as an agent's hook, by the flags main() reads before anything
// of her starts: the event a hook gives on stdin handed to her port, her
// answer to a prompt printed back for the agent where it takes one, and
// she started as a session starts when she is not up (unless she was quit
// by choice: data.rs Quit). Quick and quiet whatever happens.
use std::io::Write;
use std::time::Duration;

use serde_json::{json, Value};

use crate::{connection, crash, data, detach, events_agy, jump, now_ms, starts};

// --- Started by the SessionStart hook ----------------------------------------------------

// The event a hook gives on stdin, if it comes soon.
fn read_event() -> Option<Value> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut text = String::new();
        let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut text);
        let _ = tx.send(text);
    });
    rx.recv_timeout(Duration::from_millis(1000)).ok().and_then(|text| serde_json::from_str(&text).ok())
}

// Start her, detached so she outlives the hook and the session (she greets on
// her own), out of its agent's job and inheriting no handle (detach.rs);
// told who starts her (`who`, a hook's words: starts.rs), for her log.
fn start_detached(who: &str) {
    let Ok(exe) = data::exe() else { return };
    if !detach::spawn(&exe, &[starts::BY_FLAG, &format!("a session's hook, {who}")]) {
        hook_log(&format!("{who}: she would not start"));
    }
}

// A hook's line in her log, she not up to write it (hers once she is).
fn hook_log(line: &str) {
    crash::tail(&data::folder(), &format!("{} [tauri] hook: {line}", now_ms()));
}

// A hook finding her not up: she is started as its session starts
// (starts.rs starts_her), unless she was quit by choice, which her log says.
fn start_or_say(hook: starts::Hook, event: &str, who: impl FnOnce() -> String) {
    let dir = data::folder();
    let mut note = None;
    let start = starts::starts_her(hook, event, || {
        note = (!data::may_start(&dir)).then(|| data::quit_note(&dir).unwrap_or_default());
        note.is_some()
    });
    if start {
        start_detached(&who());
    } else if let Some(note) = note {
        hook_log(&format!("{}: her not up, and not started: {note}", who()));
    }
}

// The names above a hook, for its words (starts.rs hook_who).
fn hook_parents() -> Vec<String> {
    jump::parent_names(3)
}

// If no pet answers on the port, start one (unless the person quit her:
// data.rs Quit); if one does, hand her the event. Then leave, quick and
// quiet, whatever happens. `after_flag`: omp's extension says it is omp's,
// and how its session started.
pub fn ensure_running(port: u16, after_flag: &[String]) {
    let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_millis(800))).build().new_agent();
    let up = agent
        .get(&format!("http://127.0.0.1:{port}/health"))
        .call()
        .ok()
        .and_then(|mut res| res.body_mut().read_to_string().ok())
        .is_some_and(|body| body.contains("wakuwaku"));
    let event = read_event();
    if up {
        if let Some(event) = event {
            let _ = agent.post(&format!("http://127.0.0.1:{port}/hook?from=wakuwaku")).send_json(&event);
        }
    } else {
        let claude = std::env::var("CLAUDE_CODE_ENTRYPOINT").ok().or_else(|| (std::env::var("CLAUDECODE").as_deref() == Ok("1")).then(String::new));
        start_or_say(starts::Hook::Ensure, "", || starts::ensure_who(after_flag, event.as_ref(), claude.as_deref(), &hook_parents()));
    }
}

// --- Run by Codex's hooks -----------------------------------------------------------------

// Codex runs its hooks as commands: this program with CODEX_FLAG, for every
// event. Hand her the event, and print her answer to a prompt, the decision
// Codex reads; nothing else, since Codex takes what a hook prints as words
// for the model. SessionStart starts her when she is not up (unless the
// person quit her: data.rs Quit). Whatever
// happens, leave with 0, so Codex never shows a failed hook.
// An event handed to her port with no answer waited for (none is needed):
// the request written whole and the connection closed at once. False when
// she is not up: no one listens there (asked of Windows' table in a moment:
// a connection tried would take it 2 s to give up on, and the agent waits
// on its hook meanwhile). Up but not yet answering (her first second or two:
// her port is held at once, answered once she is set up), it waits in her
// port's queue till she is, and nothing is lost.
fn post_and_go(port: u16, agent: &str, event: &Value) -> bool {
    if !jump::is_listening(port) {
        return false;
    }
    let to = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let Ok(mut stream) = std::net::TcpStream::connect_timeout(&to, Duration::from_millis(100)) else { return false };
    let body = event.to_string();
    let head = format!(
        "POST /hook?from=wakuwaku&agent={agent} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let sent = stream.write_all(head.as_bytes()).and_then(|()| stream.write_all(body.as_bytes())).is_ok();
    let _ = stream.shutdown(std::net::Shutdown::Write);
    sent
}

// Codex's hook (connection::CODEX_FLAG): the event handed to her port, with
// where the Codex session runs (the processes above this one, for going to
// its window from the pet, jump.rs). A prompt waits for her answer, which is
// printed for Codex; any other event is handed on and let go (post_and_go).
// Its session starting with her not up starts her, unless the person quit
// her (data.rs Quit).
pub fn codex_hook(port: u16) {
    let Some(mut event) = read_event() else { return };
    if let Some(fields) = event.as_object_mut() {
        fields.insert(jump::CODEX_CHAIN.into(), jump::chain_json(&jump::hook_chain(Some("codex.exe"))));
    }
    let name = event["hook_event_name"].as_str().unwrap_or("").to_string();
    if name != "PermissionRequest" {
        if !post_and_go(port, "codex", &event) {
            let source = event["source"].as_str().unwrap_or("").to_string();
            start_or_say(starts::Hook::Codex, &name, || starts::hook_who("Codex", &name, &[&source], &hook_parents()));
        }
        return;
    }
    // A prompt waits for the person, just under the hook's own timeout.
    let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(connection::CODEX_PROMPT_TIMEOUT - 5))).build().new_agent();
    if let Ok(mut res) = agent.post(&format!("http://127.0.0.1:{port}/hook?from=wakuwaku&agent=codex")).send_json(&event) {
        let body = res.body_mut().read_to_string().unwrap_or_default();
        if serde_json::from_str::<Value>(&body).is_ok_and(|v| v.get("hookSpecificOutput").is_some()) {
            let mut out = std::io::stdout();
            let _ = out.write_all(body.as_bytes());
            let _ = out.flush();
        }
    }
}

// Antigravity's hook (connection::AGY_FLAG and the event's name, which its
// payload does not carry): PreToolUse prompts wait for her answer (printed
// as Antigravity's decision: events_agy.rs to_output), handed on to her
// port; any other hook has its answer printed first, whatever comes after
// (a hook that fails or answers nothing breaks agy's turn: connection/agy.rs),
// then the event handed to her port with its name and let go (post_and_go:
// agy waits on each hook, and her answer is nothing to it). Quit by the
// person, she is not up: nothing tried. Its session starting with her not
// up starts her, as Codex's does. Where the session runs only as it starts
// or a turn begins (kept till then; some 10 ms: most of a hook is its
// process starting, some 50 to 80 ms).
pub fn agy_hook(port: u16, name: &str) {
    if name == "PreToolUse" {
        let allow = || {
            let mut out = std::io::stdout();
            let _ = out.write_all(br#"{"decision":"allow"}"#);
            let _ = out.flush();
        };
        let Some(mut event) = read_event() else {
            return allow();
        };
        if let Some((_pid, cmd)) = jump::agy_cli_info() {
            if let Some(obj) = event.as_object_mut() {
                obj.insert("is_cli".into(), json!(true));
                let words: Vec<&str> = cmd.split_whitespace().collect();
                if words.iter().any(|&w| w == "--dangerously-skip-permissions" || w == "--dangerously_skip_permissions") {
                    obj.insert("dangerously_skip_permissions".into(), json!(true));
                }
                if cmd.contains("--mode accept-edits") || cmd.contains("--mode=accept-edits") {
                    obj.insert("cli_mode".into(), json!("accept-edits"));
                } else if cmd.contains("--mode auto") || cmd.contains("--mode=auto") {
                    obj.insert("cli_mode".into(), json!("auto"));
                }
                if words.iter().any(|&w| w == "--sandbox" || w == "--sandbox=true") {
                    obj.insert("cli_sandbox".into(), json!(true));
                }
            }
        }
        if !events_agy::needs_approval(&event, &events_agy::read_config(), &events_agy::read_cli_settings()) {
            return allow();
        }
        let who = || starts::hook_who("Antigravity", name, &[], &hook_parents());
        if !data::may_start(&data::folder()) {
            allow();
            start_or_say(starts::Hook::Agy, name, who);
            return;
        }
        if !jump::is_listening(port) {
            return allow();
        }
        let mut ask = events_agy::for_ask(&event);
        if let Some(fields) = ask.as_object_mut() {
            fields.insert(jump::CODEX_CHAIN.into(), jump::chain_json(&jump::hook_chain(None)));
        }
        let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(connection::AGY_PROMPT_TIMEOUT - 5))).build().new_agent();
        let reply = match agent.post(&format!("http://127.0.0.1:{port}/hook?from=wakuwaku&agent=agy")).send_json(&ask) {
            Ok(mut res) => {
                let body = res.body_mut().read_to_string().unwrap_or_default();
                events_agy::to_output(&body)
            }
            Err(_) => r#"{"decision":"allow"}"#.to_string(),
        };
        let mut out = std::io::stdout();
        let _ = out.write_all(reply.as_bytes());
        let _ = out.flush();
        return;
    }
    let answer = if name == "Stop" { r#"{"decision":"allow"}"# } else { "{}" };
    let mut out = std::io::stdout();
    let _ = out.write_all(answer.as_bytes());
    let _ = out.flush();
    let Some(mut event) = read_event() else { return };
    let who = || starts::hook_who("Antigravity", name, &[], &hook_parents());
    if !data::may_start(&data::folder()) {
        // Said in her log as its session starts (start_or_say).
        start_or_say(starts::Hook::Agy, name, who);
        return;
    }
    let turn = name == "SessionStart" || (name == "PreInvocation" && event.get("invocationNum").and_then(Value::as_u64) == Some(0));
    if let Some(fields) = event.as_object_mut() {
        fields.insert("hook_event_name".into(), json!(name));
        if turn {
            fields.insert(jump::CODEX_CHAIN.into(), jump::chain_json(&jump::hook_chain(None)));
        }
    }
    if !post_and_go(port, "agy", &event) {
        start_or_say(starts::Hook::Agy, name, who);
    }
}

// ZCode's hook (connection::ZCODE_FLAG and the event's name): the event
// handed to her port and let go (post_and_go: ZCode waits on each hook),
// with where the session runs as it starts or a turn begins (jump.rs); its
// session starting with her not up starts her, unless the person quit her,
// as Codex's does. A prompt waits for her answer, printed for ZCode in Claude
// Code's words (asks.rs); ZCode shows its own dialog meanwhile, takes the
// first answer, and ends this hook should the person answer there. Nothing
// printed otherwise: ZCode takes JSON a hook prints as its output, and fails
// the hook on one that does not fit. Her not up, nothing to wait on.
pub fn zcode_hook(port: u16, name: &str) {
    let Some(mut event) = read_event() else { return };
    let name = event["hook_event_name"].as_str().unwrap_or(name).to_string();
    if let Some(fields) = event.as_object_mut() {
        fields.insert("hook_event_name".into(), json!(name));
        if matches!(name.as_str(), "SessionStart" | "UserPromptSubmit") {
            fields.insert(jump::CODEX_CHAIN.into(), jump::chain_json(&jump::hook_chain(None)));
        }
    }
    if name != "PermissionRequest" {
        if !post_and_go(port, "zcode", &event) {
            let source = event["source"].as_str().unwrap_or("").to_string();
            start_or_say(starts::Hook::Zcode, &name, || starts::hook_who("ZCode", &name, &[&source], &hook_parents()));
        }
        return;
    }
    if !jump::is_listening(port) {
        return;
    }
    // Just under the hook's own timeout.
    let wait = Duration::from_millis(connection::ZCODE_PROMPT_TIMEOUT_MS - 5_000);
    let agent = ureq::Agent::config_builder().timeout_global(Some(wait)).build().new_agent();
    if let Ok(mut res) = agent.post(&format!("http://127.0.0.1:{port}/hook?from=wakuwaku&agent=zcode")).send_json(&event) {
        let body = res.body_mut().read_to_string().unwrap_or_default();
        if serde_json::from_str::<Value>(&body).is_ok_and(|v| v.get("hookSpecificOutput").is_some()) {
            let mut out = std::io::stdout();
            let _ = out.write_all(body.as_bytes());
            let _ = out.flush();
        }
    }
}

// DeepSeek Harness's hook (connection::DSH_FLAG and the event's name), as
// her old rows had it run (until 0.3.2; her own plugin in DSH replaces them,
// connection/dsh.rs, and they are no longer written: this stays for a DSH still
// running them), by its Claude Code bridge through its PowerShell, in its sandbox: the event
// handed to her port and let go. As a session starts or a turn begins it
// waits for her answer instead (some ms), this connection held while she
// looks up where the session runs from this process (server.rs caller_of:
// in its sandbox it may not open the processes above itself). It never
// starts her: she would run in DSH's sandbox (a low-integrity,
// write-restricted token) and could keep nothing. Nothing printed: the
// bridge would read it as a decision.
pub fn dsh_hook(port: u16, name: &str) {
    let Some(mut event) = read_event() else { return };
    let name = event["hook_event_name"].as_str().unwrap_or(name).to_string();
    if let Some(fields) = event.as_object_mut() {
        fields.insert("hook_event_name".into(), json!(name));
    }
    if !matches!(name.as_str(), "SessionStart" | "UserPromptSubmit") {
        post_and_go(port, "dsh", &event);
        return;
    }
    if jump::is_listening(port) {
        let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(3))).build().new_agent();
        let _ = agent.post(&format!("http://127.0.0.1:{port}/hook?from=wakuwaku&agent=dsh")).send_json(&event);
    }
}

// Reasonix's hook (connection::REASONIX_FLAG and the event's name), run by
// Reasonix through cmd.exe for each event of its sessions, its CLI's and
// its Studio's alike: the event handed to her port and let go (post_and_go:
// Reasonix waits on every hook, and one of its gating events that times out
// is blocked), with where the session runs as it starts or a turn begins
// (jump.rs, past cmd); its session starting with her not up starts her,
// unless the person quit her, as Codex's does. Nothing printed, whatever
// the event: Reasonix shows what a hook prints, and its PermissionRequest
// stays its own (zhoupengjie/Wakuwaku#60, phase 1: server.rs answers_none).
// Her not up, nothing tried but that; and always gone with 0.
pub fn reasonix_hook(port: u16, name: &str) {
    let Some(mut event) = read_event() else { return };
    let name = event["event"].as_str().filter(|n| !n.is_empty()).unwrap_or(name).to_string();
    if let Some(fields) = event.as_object_mut() {
        fields.insert("event".into(), json!(name));
        // The name her server reads every agent's events by.
        fields.insert("hook_event_name".into(), json!(name));
        if matches!(name.as_str(), "SessionStart" | "UserPromptSubmit") {
            fields.insert(jump::CODEX_CHAIN.into(), jump::chain_json(&jump::hook_chain(None)));
        }
    }
    if !post_and_go(port, "reasonix", &event) {
        let source = event["source"].as_str().unwrap_or("").to_string();
        start_or_say(starts::Hook::Reasonix, &name, || starts::hook_who("Reasonix", &name, &[&source], &hook_parents()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Handed on and let go while her port is held but not yet answered (her
    // first second or two): the event waits in its queue, whole, and is read
    // once she answers; then as she answers at once.
    #[test]
    fn an_event_let_go_is_read_whole_later() {
        let server = tiny_http::Server::http(("127.0.0.1", 0)).unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let event = json!({ "hook_event_name": "Stop", "conversationId": "c", "text": "x".repeat(20_000) });
        let t = std::time::Instant::now();
        assert!(post_and_go(port, "agy", &event));
        assert!(t.elapsed() < Duration::from_millis(200), "let go at once");
        std::thread::sleep(Duration::from_millis(300));
        let mut req = server.recv_timeout(Duration::from_secs(2)).unwrap().expect("a request");
        assert_eq!(req.url(), "/hook?from=wakuwaku&agent=agy");
        let mut body = String::new();
        std::io::Read::read_to_string(req.as_reader(), &mut body).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&body).unwrap(), event);
        // Her answer goes nowhere, harmlessly.
        let _ = req.respond(tiny_http::Response::from_string("{}"));
        // No one there: nothing tried.
        drop(server);
        assert!(!post_and_go(port, "agy", &event));
    }
}
