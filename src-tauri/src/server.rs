// The local HTTP port the Claude Code and Codex hooks report to.
//
//   GET  /health       { ok, app, runtime, state }; with WAKUWAKU_DEBUG=1, the windows and settings too
//   POST /hook         a Claude Code hook event; answers {}, or for a prompt the person's answer
//                      ?agent=codex: a Codex one, handed on by this program (main.rs codex_hook)
//   POST /state        a message (state.rs)
//   POST /widget       a widget for the island, from a script (widgets.rs)
//   POST /come-home    started again while running: back into sight
//   POST /debug/eval   { page, code }: run code in her page or the island's (WAKUWAKU_DEBUG=1 only)
//   POST /debug/walk   { dx, ms }: take a walk now (WAKUWAKU_DEBUG=1 only)
//   POST /quit         quit, as the menu does (a new build replacing her; killing her
//                      would leave the top bar's strip taken)
use std::io::Read;
use std::sync::Arc;

use serde_json::{json, Value};
use tauri::Manager;
use tiny_http::{Header, Request, Response, Server};

use crate::{events, events_codex, island, jump, now_ms, pet, Responder, Shared};

// Hook events carry whole files (an Edit's PostToolUse has the file before
// the edit), so this is generous.
const MAX_BODY: u64 = 16 * 1024 * 1024;
// Codex waits on a prompt's hook before it shows its own dialog: the panel
// holds one this long at most.
const CODEX_ASK_MS: u64 = 60 * 1000;

// Whether the request comes from Codex's hook (?agent=codex).
fn is_from_codex(url: &str) -> bool {
    url.split_once('?').is_some_and(|(_, query)| query.split('&').any(|p| p == "agent=codex"))
}

pub fn serve(sh: Arc<Shared>, server: Server) {
    let is_debug = std::env::var("WAKUWAKU_DEBUG").as_deref() == Ok("1");
    std::thread::spawn(move || {
        for req in server.incoming_requests() {
            handle(&sh, req, is_debug);
        }
    });
}

fn reply(req: Request, code: u16, body: Value) {
    let header = Header::from_bytes("content-type", "application/json").expect("a valid header");
    let _ = req.respond(Response::from_string(body.to_string()).with_status_code(code).with_header(header));
}

fn read_json(req: &mut Request) -> Option<Value> {
    let mut bytes = Vec::new();
    req.as_reader().take(MAX_BODY).read_to_end(&mut bytes).ok()?;
    serde_json::from_slice(&bytes).ok()
}

// The process chain of the session an event comes from, looked up when the
// session has none yet or a turn begins (it may have been resumed in another
// process): Codex's hook command brings its own; for Claude Code, it is the
// process on the other end of this connection.
fn chain_for(sh: &Shared, req: &Request, event: &Value, is_codex: bool) -> Option<jump::Chain> {
    if is_codex {
        let chain = jump::chain_from_json(event.get(jump::CODEX_CHAIN));
        return (!chain.is_empty()).then_some(chain);
    }
    let session = event.get("session_id").and_then(Value::as_str).unwrap_or("");
    let is_turn = matches!(event["hook_event_name"].as_str(), Some("UserPromptSubmit" | "SessionStart"));
    if session.is_empty() || !(is_turn || !sh.pet.lock().unwrap().has_chain(session)) {
        return None;
    }
    let client = jump::client_of(req.remote_addr()?.port(), sh.port)?;
    let chain = jump::chain_of(client);
    (!chain.is_empty()).then_some(chain)
}

fn handle(sh: &Arc<Shared>, mut req: Request, is_debug: bool) {
    let path = req.url().split('?').next().unwrap_or("").to_string();
    let method = req.method().as_str().to_string();
    match (method.as_str(), path.as_str()) {
        ("POST", "/hook") => {
            let Some(mut event) = read_json(&mut req) else { return reply(req, 200, json!({})) };
            // Codex's events, tagged for the prompts (asks.rs) and read their own way.
            let is_codex = is_from_codex(req.url());
            if let (true, Some(fields)) = (is_codex, event.as_object_mut()) {
                fields.insert("agent".into(), json!("codex"));
                sh.codex_seen.store(now_ms(), std::sync::atomic::Ordering::SeqCst);
            }
            let msg = if is_codex { events_codex::to_message(&event) } else { events::to_message(&event) };
            if let Some(mut msg) = msg {
                // Where the session runs, for going to its window (jump.rs).
                if let Some(chain) = chain_for(sh, &req, &event, is_codex) {
                    msg["chain"] = jump::chain_json(&chain);
                }
                sh.apply(&msg);
            }
            // A prompt holds the answer until the person picks on the pet, the
            // terminal settles it, or time runs out; out of sight, it is the
            // terminal's at once.
            if event["hook_event_name"] == "PermissionRequest" && sh.is_visible() {
                let respond: Responder = Box::new(move |out: Option<Value>| match out {
                    Some(out) => reply(req, 200, if out.is_object() { out } else { json!({}) }),
                    None => drop(req),
                });
                let mut wait = sh.setting("promptWaitSec").as_u64().unwrap_or(290) * 1000;
                if is_codex {
                    wait = wait.min(CODEX_ASK_MS);
                }
                let added = sh.asks.lock().unwrap().add(event, respond, now_ms(), wait);
                match added {
                    Ok(id) => {
                        sh.log(&format!("prompt {id} waits on the person"));
                        sh.push_asks();
                    }
                    Err(respond) => respond(Some(json!({}))),
                }
                return;
            }
            if sh.asks.lock().unwrap().seen(&event) {
                sh.push_asks();
            }
            reply(req, 200, json!({}))
        }
        // A widget from a script (widgets.rs).
        ("POST", "/widget") => match read_json(&mut req) {
            Some(v) => match sh.put_widget(&v) {
                Ok(()) => reply(req, 200, json!({ "ok": true })),
                Err(why) => reply(req, 400, json!({ "error": why })),
            },
            None => reply(req, 400, json!({ "error": "bad json" })),
        },
        // Started again while she runs: she comes back into sight.
        ("POST", "/come-home") => {
            crate::come_home(sh);
            reply(req, 200, json!({ "ok": true }))
        }
        ("GET", "/health") => {
            let mut body = json!({ "ok": true, "app": "wakuwaku", "runtime": "tauri", "state": sh.pet.lock().unwrap().get(now_ms()) });
            if is_debug {
                body["window"] = pet::where_(sh);
                body["island"] = island::where_(sh);
                body["settings"] = Value::Object(sh.settings.lock().unwrap().clone());
            }
            reply(req, 200, body)
        }
        ("POST", "/state") => match read_json(&mut req) {
            Some(msg) if sh.apply(&msg) => reply(req, 200, json!({ "ok": true })),
            Some(_) => reply(req, 400, json!({ "error": "nothing to do" })),
            None => reply(req, 400, json!({ "error": "bad json" })),
        },
        ("POST", "/debug/eval") if is_debug => {
            let Some(body) = read_json(&mut req) else { return reply(req, 400, json!({ "error": "bad json" })) };
            let page = body["page"].as_str().unwrap_or("pet");
            let code = body["code"].as_str().unwrap_or("null");
            match evaluate(sh, page, code) {
                Some(value) => reply(req, 200, json!({ "result": value })),
                None => reply(req, 504, json!({ "error": "no answer" })),
            }
        }
        // Quit as the menu does (the bar's strip given back on the way): for
        // replacing her with a new build, which must not just kill her.
        ("POST", "/quit") | ("POST", "/debug/quit") => {
            reply(req, 200, json!({ "ok": true }));
            sh.app.exit(0);
        }
        ("POST", "/debug/walk") if is_debug => {
            let Some(body) = read_json(&mut req) else { return reply(req, 400, json!({ "error": "bad json" })) };
            let (dx, ms) = (body["dx"].as_f64().unwrap_or(0.0), body["ms"].as_f64().unwrap_or(500.0));
            // Walks block; this one answers when it is over.
            let sh = sh.clone();
            std::thread::spawn(move || {
                let arrived = pet::walk(&sh, dx, ms);
                reply(req, 200, json!({ "ok": true, "arrived": arrived }));
            });
        }
        _ => reply(req, 404, json!({ "error": "not found" })),
    }
}

// Run code in a page ("pet" or "island") and wait for what it gives back.
fn evaluate(sh: &Shared, page: &str, code: &str) -> Option<Value> {
    let win = sh.app.get_webview_window(page)?;
    let (tx, rx) = std::sync::mpsc::channel();
    let id = {
        let mut evals = sh.evals.lock().unwrap();
        evals.0 += 1;
        let id = evals.0;
        evals.1.insert(id, tx);
        id
    };
    win.eval(format!("window.__wkEval({id}, () => ({code}))")).ok()?;
    let answer = rx.recv_timeout(std::time::Duration::from_secs(3)).ok();
    sh.evals.lock().unwrap().1.remove(&id);
    answer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_hooks_say_where_they_come_from() {
        assert!(is_from_codex("/hook?from=wakuwaku&agent=codex"));
        assert!(is_from_codex("/hook?agent=codex"));
        assert!(!is_from_codex("/hook?from=wakuwaku"));
        assert!(!is_from_codex("/hook?agent=codexx"));
        assert!(!is_from_codex("/hook"));
    }
}
