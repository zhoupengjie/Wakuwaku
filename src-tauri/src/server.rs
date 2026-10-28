// The local HTTP port the Claude Code and Codex hooks report to.
//
//   GET  /health       { ok, app, runtime, state, ready: { pet, island }, waiting }; with
//                      WAKUWAKU_DEBUG=1, the windows and settings too
//   POST /hook         a Claude Code hook event; answers {} as soon as it is read, then
//                      takes it (take, one after another in the order they came), or
//                      for a prompt answers with the person's answer
//                      ?agent=codex: a Codex one, handed on by this program (main.rs codex_hook)
//                      ?agent=agy, ?agent=zcode, ?agent=reasonix: the same for
//                      Antigravity's, ZCode's and Reasonix's (main.rs agy_hook)
//                      ?agent=dsh: a DeepSeek Harness one, from her plugin inside DSH
//                      (integrations/dsh/wakuwaku.js, events_dsh.rs), or handed on by
//                      this program for its old hook bridge (main.rs dsh_hook)
//                      ?agent=opencode: an OpenCode event, from her plugin in it (events_tests.rs opencode)
//                      ?agent=omp: an oh-my-pi one, from her extension inside omp
//                      (integrations/omp/wakuwaku.ts, events_tests.rs omp)
//   GET  /v1/info      the agents' sessions told to other apps on this machine (agent-hub,
//   GET  /v1/sessions  its lib.rs says how): the hub; the sessions; each event as it comes
//   GET  /v1/events    (Server-Sent Events). The last two take the token in her data
//                      folder's agent-hub\token (Authorization: Bearer <token>); none is
//                      served to a request whose Host is not this machine (is_local)
//   POST /state        an AgentEvent (agent-core's, as /v1/events sends them): into her
//                      sessions (state.rs) and to the apps listening, as a hook's is
//   POST /widget       a widget for the island, from a script (widgets.rs)
//   POST /come-home    started again while running: back into sight
//   POST /after-update { pid, version }: a new version she started (an update, a restart) is
//                      up, and waits for her to go (updates.rs hand_over)
//   POST /debug/eval   { page, code }: run code in her page or the island's (WAKUWAKU_DEBUG=1 only)
//   POST /debug/walk   { dx, ms }: take a walk now (WAKUWAKU_DEBUG=1 only)
//   POST /quit         quit, as the menu does: by choice, so no session's hook starts her
//                      until she is started otherwise (data.rs Quit, #57); for a new build
//                      replacing her, which starts her (and clears that), or a script (killing
//                      her would leave the taskbar's strip taken)
//                      ?wake=1: quit so that a session's hook may start her again
use std::io::Read;
use std::path::Path;
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use tauri::Manager;
use tiny_http::{Header, Request, Response, Server, StatusCode};

use agent_core::event::{Agent, AgentEvent, Kind};

use crate::{events, events_agy, events_dsh, island, jump, now_ms, pet, Responder, Shared};

// Hook events carry whole files (an Edit's PostToolUse has the file before
// the edit), so this is generous.
const MAX_BODY: u64 = 16 * 1024 * 1024;
// Codex and Antigravity wait on a prompt's hook before showing their own dialog: the panel
// holds one this long at most.
const CODEX_ASK_MS: u64 = 60 * 1000;
// The hooks' events answered and not yet taken, at most (serve): past them
// the next waits to be read.
const HOOKS_WAITING: usize = 256;

// Which agent's hook the request comes from (?agent=codex, ?agent=agy,
// ?agent=zcode, ?agent=dsh, ?agent=reasonix: handed on by this program,
// main.rs; ?agent=opencode: her plugin in OpenCode; ?agent=omp: her
// extension in omp); none for Claude Code's.
fn agent_of(url: &str) -> Option<&'static str> {
    let (_, query) = url.split_once('?')?;
    query.split('&').find_map(|p| match p {
        "agent=codex" => Some("codex"),
        "agent=agy" => Some("agy"),
        "agent=opencode" => Some("opencode"),
        "agent=omp" => Some("omp"),
        "agent=zcode" => Some("zcode"),
        "agent=dsh" => Some("dsh"),
        "agent=reasonix" => Some("reasonix"),
        _ => None,
    })
}

// How POST /quit quits: by choice, unless asked with ?wake=1 (or true) to
// let a session's hook start her again (data.rs Quit); and its words.
fn quit_of(url: &str) -> (crate::data::Quit, &'static str) {
    let query = url.split_once('?').map_or("", |(_, q)| q);
    if query.split('&').any(|p| matches!(p, "wake=1" | "wake=true")) {
        (crate::data::Quit::Wakeable, "POST /quit?wake=1")
    } else {
        (crate::data::Quit::ByChoice, "POST /quit")
    }
}

// Whether an agent's events bring the processes their session runs in:
// Codex's, Antigravity's, ZCode's and Reasonix's hook commands look them up
// (main.rs).
// Claude Code's come from its own process, omp's from her extension inside
// omp.exe, OpenCode's from her plugin in its service and DeepSeek Harness's
// from her plugin in its Host (or, the old way, a hook in its sandbox):
// those she finds from the other end of the connection (caller_of).
fn brings_chain(agent: Option<&str>) -> bool {
    matches!(agent, Some("codex" | "agy" | "zcode" | "reasonix"))
}

// Whether an agent's PermissionRequest is told only, not held for her answer:
// Reasonix's hook hands it on and leaves at once, so nobody waits on the
// reply; the person answers in Reasonix, and she shows the session waiting.
// DeepSeek Harness's, unless her plugin says it can take her answer
// (answerable: it could make the request withdraw DSH's own dialog as she
// answers first; integrations/dsh/wakuwaku.js). Codex's where the person
// let Codex approve for them (Approve for me): Codex asks its own reviewer
// once the hook has not answered, never the person, so nothing is held, nor
// shown (agent-core codex.rs auto_reviewed, zhoupengjie/Wakuwaku#90).
fn answers_none(agent: Option<&str>, event: &Value) -> bool {
    agent == Some("reasonix")
        || (agent == Some("dsh") && event.get("answerable") != Some(&Value::Bool(true)))
        || (agent == Some("codex") && agent_core::codex::auto_reviewed(event["transcript_path"].as_str().unwrap_or("")))
}

// An agent's event as she takes it: OpenCode's by the names Claude Code's
// hooks have (the prompts and the jump read them so), every agent's but
// Claude Code's tagged with its name for the prompts (asks.rs). None for one
// that is nothing to her.
pub(crate) fn read_event(agent: Option<&str>, mut event: Value) -> Option<Value> {
    if agent == Some("opencode") {
        event = agent_core::opencode::normalize(&event);
        if event.is_null() {
            return None;
        }
    }
    if agent == Some("agy") {
        events_agy::with_seen(&mut event);
    }
    if let (Some(agent), Some(fields)) = (agent, event.as_object_mut()) {
        fields.insert("agent".into(), json!(agent));
    }
    Some(event)
}

// What an event she took (read_event) says a session is doing, each agent's
// read its own way (agent-core) at `now`: Antigravity's given the person's
// words and its reply from its transcript first (its PermissionRequest in
// Claude Code's words, from events_agy::for_ask); a DSH prompt she does not
// hold (`held`) to be approved in DSH. Her sessions are kept from it
// (state.rs), and the apps listening are sent it (agent-hub).
// Also the replay tests' way in (replay.rs).
pub(crate) fn event_for(agent: Option<&str>, event: &mut Value, held: bool, now: u64, dsh_home: &Path) -> Option<AgentEvent> {
    if agent == Some("agy") && event["hook_event_name"] == "PermissionRequest" {
        return agent_core::claude::to_event(event, Agent::Agy);
    }
    let mut ev = agent_core::read(Agent::from_tag(agent), event, &agent_core::Context { now, dsh_home })?;
    if agent == Some("dsh") && event["hook_event_name"] == "PermissionRequest" && !held {
        agent_core::dsh::answered_in_dsh(&mut ev);
    }
    Some(ev)
}

// True the first time a line comes; a few kinds are kept, and past them
// nothing more is said.
fn first_time(said: &mut Vec<String>, line: &str) -> bool {
    if said.len() >= 8 || said.iter().any(|s| s == line) {
        return false;
    }
    said.push(line.to_string());
    true
}

pub fn serve(sh: Arc<Shared>, server: Server) {
    let is_debug = std::env::var("WAKUWAKU_DEBUG").as_deref() == Ok("1");
    // What tiny_http tells of a connection it dropped, where 0.12.0 panicked
    // (vendor/tiny_http/README.md), or of its accept thread ending, into her
    // log. Each thing is said once: a socket that a network filter will not
    // let her split fails so for every hook, and would fill the log.
    let (said, teller) = (Mutex::new(Vec::new()), sh.clone());
    tiny_http::tell_failures(move |line| {
        if first_time(&mut said.lock().unwrap_or_else(|e| e.into_inner()), line) {
            teller.log(&format!("http: {line}"));
        }
    });
    // The hooks' events, taken one after another in the order they came
    // (take), each answered first (handle): an agent waits on her no longer
    // than it takes to read its event.
    let (hooks, taken) = std::sync::mpsc::sync_channel::<Hook>(HOOKS_WAITING);
    let taker = sh.clone();
    crate::spawn_named("hooks", move || {
        for hook in taken {
            take(&taker, hook);
        }
    });
    crate::spawn_named("server", move || {
        for req in server.incoming_requests() {
            handle(&sh, req, is_debug, &hooks);
        }
    });
}

fn reply(req: Request, code: u16, body: Value) {
    let header = Header::from_bytes("content-type", "application/json").expect("a valid header");
    let _ = req.respond(Response::from_string(body.to_string()).with_status_code(code).with_header(header));
}

// A stream of the hub's events answered (GET /v1/events): each sent as it
// comes (a chunk of its own, vendor/tiny_http/README.md), for as long as
// the client reads, on a thread of its own; it ends as the client goes (a
// write fails, at the next event or ping) or the hub lets it go.
fn stream(req: Request, events: agent_hub::Events) {
    let headers = vec![
        Header::from_bytes("content-type", "text/event-stream").expect("a valid header"),
        Header::from_bytes("cache-control", "no-cache").expect("a valid header"),
    ];
    crate::spawn_named("hub-events", move || {
        let _ = req.respond(Response::new(StatusCode(200), headers, events, None, None));
    });
}

fn read_json(req: &mut Request) -> Option<Value> {
    let mut bytes = Vec::new();
    req.as_reader().take(MAX_BODY).read_to_end(&mut bytes).ok()?;
    serde_json::from_slice(&bytes).ok()
}

// The process chain of the session an event comes from, looked up when the
// session has none yet or a turn begins (it may have been resumed in another
// process): Codex's, Antigravity's and ZCode's hook command brings its own;
// for Claude Code and omp, it is the process on the other end of this
// connection. For OpenCode that is its background service, which has no
// window: the terminal is that of a program of OpenCode's connected to it
// (jump.rs front_chain), or the service's own when none is (run standalone).
// DeepSeek Harness's events come from her plugin in its Host, a process of
// the app's own (DeepSeek Harness.exe in Node's mode, under the app's main
// one), which waits on this connection while she looks; the old bridge's
// from a hook in its sandbox, which may not open the processes above it, its
// nearest ones its shell and DSH's hook runners, gone a moment later
// (wakuwaku ← pwsh ← runner ← runner ← host ← DeepSeek Harness.exe, seen
// 2026-10-16). Either way the chain is the app's, the outermost by the
// processes' list.
//
// Who sent it is found while the sender is there (caller_of), before the
// hook is answered: the process on the other end of the connection, from
// the TCP table, and for DSH the app above it, before the old bridge's
// hook and the runners under it are gone. Its chain after (chain_of_caller).
enum Caller {
    // The chain its hook command brought.
    Brought(crate::state::Chain),
    // The process it came from (DSH's: the app's outermost).
    Process(u32),
}

fn caller_of(sh: &Shared, req: &Request, event: &Value, agent: Option<&str>) -> Option<Caller> {
    if brings_chain(agent) {
        let chain = jump::chain_from_json(event.get(jump::CODEX_CHAIN));
        return (!chain.is_empty()).then_some(Caller::Brought(chain));
    }
    let session = event.get("session_id").and_then(Value::as_str).unwrap_or("");
    let is_turn = matches!(event["hook_event_name"].as_str(), Some("UserPromptSubmit" | "SessionStart"));
    if session.is_empty() || !(is_turn || !sh.pet.lock().unwrap().has_chain(session)) {
        return None;
    }
    let client = jump::client_of(req.remote_addr()?.port(), sh.port)?;
    Some(Caller::Process(if agent == Some("dsh") { jump::outermost_of(client, "DeepSeek Harness.exe") } else { client }))
}

fn chain_of_caller(caller: Caller, event: &Value, agent: Option<&str>) -> Option<crate::state::Chain> {
    let chain = match caller {
        Caller::Brought(chain) => chain,
        Caller::Process(client) if agent == Some("opencode") => {
            let cwd = event.get("cwd").and_then(Value::as_str).unwrap_or("");
            let hints = [event.get("session_title").and_then(Value::as_str).unwrap_or(""), if cwd.is_empty() { "" } else { events::base(cwd) }];
            jump::front_chain(client, &hints)
        }
        Caller::Process(pid) => jump::chain_of(pid),
    };
    (!chain.is_empty()).then_some(chain)
}

// A hook event read and answered (handle), for the hooks' thread (take); a
// prompt's request kept, to be answered as the person does (or at once
// when it is not held).
pub struct Hook {
    agent: Option<&'static str>,
    event: Value,
    is_prompt: bool,
    caller: Option<Caller>,
    req: Option<Request>,
}

// What a hook's event does, in the order they came (serve): the session's
// message for her, its event to the apps listening (agent-hub), a prompt
// held for the person, prompts the terminal settled let go.
fn take(sh: &Shared, hook: Hook) {
    let Hook { agent, mut event, is_prompt, caller, req } = hook;
    // A prompt held for the person's answer on her (below): known before
    // its message, which for DSH's says where it is answered. Out of sight
    // (her taskbar on another display counts: main.rs asks_seen), it is the
    // terminal's at once.
    let holds = is_prompt && sh.asks_seen();
    let dsh_home = crate::connection::dsh_home();
    let now = now_ms();
    let ev = event_for(agent, &mut event, holds, now, &dsh_home);
    if let Some(ev) = &ev {
        sh.hub.publish(ev, now);
    }
    if agent == Some("agy") {
        if let Some(line) = events_agy::log_line(&event) {
            sh.log(&line);
        }
    }
    if let Some(ev) = ev {
        // Where the session runs, for going to its window (jump.rs);
        // none for one going (a DSH subagent's too: events_dsh.rs whose).
        let chain = if ev.kind == Kind::SessionEnd { None } else { caller.and_then(|c| chain_of_caller(c, &event, agent)) };
        if agent == Some("dsh") {
            if let Some(line) = events_dsh::log_line(&event, chain.as_ref().and_then(|c| c.first()).map(|c| c.0), &dsh_home) {
                sh.log(&line);
            }
        }
        sh.apply_event(&ev, chain);
    } else if agent == Some("dsh") {
        // Her plugin's word as DSH loads it: said in her log, and the open
        // Connect page tells it loaded (connection/dsh.rs dsh_view).
        if let Some(line) = events_dsh::log_line(&event, None, &dsh_home) {
            sh.log(&line);
            sh.push_settings();
        }
    }
    // A prompt holds the answer until the person picks on the pet, the
    // terminal settles it, or time runs out.
    if is_prompt && !holds {
        sh.log("a prompt handed to the terminal: out of sight");
    }
    if !is_prompt && agent == Some("codex") && event["hook_event_name"] == "PermissionRequest" {
        sh.log("a Codex request to approve left to Codex (Approve for me)");
    }
    match req {
        Some(req) if holds => {
            let respond: Responder = Box::new(move |out: Option<Value>| match out {
                Some(out) => reply(req, 200, if out.is_object() { out } else { json!({}) }),
                None => drop(req),
            });
            let mut wait = sh.setting("promptWaitSec").as_u64().unwrap_or(290) * 1000;
            if agent == Some("codex") || agent == Some("agy") {
                wait = wait.min(CODEX_ASK_MS);
            }
            // DSH's in Claude Code's words, as the panel shows them.
            let event = if agent == Some("dsh") { events_dsh::for_ask(&event) } else { event };
            let added = sh.asks.lock().unwrap().add(event, respond, now_ms(), wait);
            match added {
                Ok(id) => {
                    sh.log(&format!("prompt {id} waits on the person"));
                    sh.push_asks();
                }
                Err(respond) => respond(Some(json!({}))),
            }
        }
        req => {
            let settled = sh.asks.lock().unwrap().seen(&event);
            if !settled.is_empty() {
                for id in settled {
                    sh.log(&format!("prompt {id} settled in the terminal ({})", event["hook_event_name"].as_str().unwrap_or("?")));
                }
                sh.push_asks();
            }
            // A prompt not held: the terminal's.
            if let Some(req) = req {
                reply(req, 200, json!({}));
            }
        }
    }
}

// A header's value.
fn header<'a>(req: &'a Request, name: &'static str) -> Option<&'a str> {
    req.headers().iter().find(|h| h.field.equiv(name)).map(|h| h.value.as_str())
}

fn query(url: &str) -> &str {
    url.split_once('?').map_or("", |(_, q)| q)
}

// Whether a request names this machine as its host: a web page's own name
// made to point here (DNS rebinding) does not, and is not served /v1.
fn is_local(req: &Request) -> bool {
    is_local_host(header(req, "Host").unwrap_or(""))
}

fn is_local_host(host: &str) -> bool {
    let name = host.rsplit_once(':').filter(|(_, port)| !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit())).map_or(host, |(name, _)| name);
    matches!(name.to_ascii_lowercase().as_str(), "127.0.0.1" | "localhost" | "[::1]")
}

fn handle(sh: &Arc<Shared>, mut req: Request, is_debug: bool, hooks: &SyncSender<Hook>) {
    let path = req.url().split('?').next().unwrap_or("").to_string();
    let method = req.method().as_str().to_string();
    match (method.as_str(), path.as_str()) {
        ("POST", "/hook") => {
            let Some(event) = read_json(&mut req) else { return reply(req, 200, json!({})) };
            // Heard from one: connected (the island stops asking).
            if crate::connection::note_connected(true) {
                sh.redraw();
            }
            let agent = agent_of(req.url());
            // One that is nothing to her is answered at once.
            let Some(event) = read_event(agent, event) else { return reply(req, 200, json!({})) };
            let seen = &sh.seen[crate::connection::slot(Agent::from_tag(agent))];
            // Her DSH plugin's word on loading is no session's doing.
            if !(agent == Some("dsh") && events_dsh::is_loaded(&event)) {
                seen.store(now_ms(), std::sync::atomic::Ordering::SeqCst);
            }
            // A prompt, which she may hold for the person's answer (take).
            // Reasonix's is not one to answer yet: its hook hands it on and
            // is gone (main.rs reasonix_hook), so she only waits with it
            // (zhoupengjie/Wakuwaku#60, phase 1); nor is one of DSH's her
            // plugin cannot take an answer to.
            let is_prompt = event["hook_event_name"] == "PermissionRequest" && !answers_none(agent, &event);
            let caller = caller_of(sh, &req, &event, agent);
            // Answered now, and taken after (any other the same): a prompt
            // kept to be answered there.
            let req = if is_prompt {
                Some(req)
            } else {
                reply(req, 200, json!({}));
                None
            };
            // The hooks' thread gone (it never ends): a prompt's answered.
            if let Err(gone) = hooks.send(Hook { agent, event, is_prompt, caller, req }) {
                if let Some(req) = gone.0.req {
                    reply(req, 200, json!({}));
                }
            }
        }
        // The agents' sessions, for other apps on this machine (agent-hub).
        // With ?nonce=, the hub's proof that it knows its token (a client's
        // check before it sends the token: agent-hub's proves).
        ("GET", "/v1/info") if is_local(&req) => {
            let info = sh.hub.info_for("wakuwaku", env!("CARGO_PKG_VERSION"), query(req.url()));
            reply(req, 200, info)
        }
        ("GET", "/v1/sessions" | "/v1/events") if is_local(&req) && !sh.hub.allows(header(&req, "Authorization")) => {
            reply(req, 401, json!({ "error": "the hub's token is asked for: Authorization: Bearer <token>", "tokenFile": sh.hub.token_file_named() }))
        }
        // A query it cannot take (agents it does not know): 400, saying why.
        ("GET", "/v1/sessions") if is_local(&req) => match sh.hub.sessions(query(req.url())) {
            Ok(body) => reply(req, 200, body),
            Err(why) => reply(req, 400, json!({ "error": why })),
        },
        ("GET", "/v1/events") if is_local(&req) => match sh.hub.events(query(req.url()), header(&req, "Last-Event-ID")) {
            Ok(events) => stream(req, events),
            Err(why) => reply(req, 400, json!({ "error": why })),
        },
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
        // A new version she started says it is up (updates.rs).
        ("POST", "/after-update") => match read_json(&mut req) {
            Some(word) => {
                let (pid, version) = (word["pid"].as_u64().unwrap_or(0) as u32, word["version"].as_str().unwrap_or(""));
                crate::updates::heard(pid, version);
                reply(req, 200, json!({ "ok": true }))
            }
            None => reply(req, 400, json!({ "error": "bad json" })),
        },
        ("GET", "/health") => {
            // Her pages up (each told pet_ready), and whether this copy waits
            // for another holding the taskbar (taskbar.rs: no island made
            // meanwhile): what a test waits on after starting a copy, not
            // the port nor a line of the log. One killed or quit a second
            // after it started has no page yet, as one that never gets them
            // (the test VM's two "starts with no page", 2026-10-16).
            let ready = json!({ "pet": pet::is_ready(sh), "island": island::is_ready(sh) });
            let mut body = json!({ "ok": true, "app": "wakuwaku", "runtime": "tauri", "state": sh.pet.lock().unwrap().get(now_ms()), "ready": ready, "waiting": crate::taskbar::waiting() });
            if is_debug {
                body["window"] = pet::where_(sh);
                body["island"] = island::where_(sh);
                body["settings"] = Value::Object(sh.settings.lock().unwrap().clone());
            }
            reply(req, 200, body)
        }
        // An AgentEvent from a script, as /v1/events sends them: into her
        // sessions, and to the apps listening, as an agent's hook's is.
        ("POST", "/state") => match read_json(&mut req).as_ref().map(AgentEvent::from_json) {
            Some(Ok(ev)) => {
                sh.hub.publish(&ev, now_ms());
                if sh.apply_event(&ev, None) {
                    reply(req, 200, json!({ "ok": true }))
                } else {
                    reply(req, 400, json!({ "error": "nothing to do" }))
                }
            }
            Some(Err(why)) => reply(req, 400, json!({ "error": format!("not an AgentEvent: {why}") })),
            None => reply(req, 400, json!({ "error": "bad json" })),
        },
        // The sensor helper's CPU temperature and power (sensors.rs, #18 part
        // B): taken only when they carry the token the elevated setup left in
        // her data folder, so another local program cannot feed her numbers.
        ("POST", "/hw") => {
            let ok = read_json(&mut req).is_some_and(|body| crate::sensors::accept(&sh.dir, &body));
            // The switch's line follows the helper while the settings watch.
            if ok && sh.island.lock().unwrap().settings_open {
                sh.push_settings();
            }
            reply(req, if ok { 200 } else { 403 }, json!({ "ok": ok }))
        }
        ("POST", "/debug/eval") if is_debug => {
            let Some(body) = read_json(&mut req) else { return reply(req, 400, json!({ "error": "bad json" })) };
            let page = body["page"].as_str().unwrap_or("pet");
            let code = body["code"].as_str().unwrap_or("null");
            match evaluate(sh, page, code) {
                Some(value) => reply(req, 200, json!({ "result": value })),
                None => reply(req, 504, json!({ "error": "no answer" })),
            }
        }
        // Quit as the menu does (the bar's strip given back on the way), by
        // choice unless ?wake=1: for replacing her with a new build, which
        // must not just kill her, and for scripts.
        ("POST", "/quit") | ("POST", "/debug/quit") => {
            let (how, why) = quit_of(req.url());
            reply(req, 200, json!({ "ok": true, "byChoice": how == crate::data::Quit::ByChoice }));
            sh.quit(how, why);
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
        assert_eq!(agent_of("/hook?from=wakuwaku&agent=codex"), Some("codex"));
        assert_eq!(agent_of("/hook?agent=codex"), Some("codex"));
        assert_eq!(agent_of("/hook?from=wakuwaku&agent=agy"), Some("agy"));
        assert_eq!(agent_of("/hook?from=wakuwaku&agent=opencode"), Some("opencode"));
        assert_eq!(agent_of("/hook?from=wakuwaku&agent=omp"), Some("omp"));
        assert_eq!(agent_of("/hook?from=wakuwaku&agent=zcode"), Some("zcode"));
        assert_eq!(agent_of("/hook?from=wakuwaku&agent=dsh"), Some("dsh"));
        assert_eq!(agent_of("/hook?from=wakuwaku&agent=reasonix"), Some("reasonix"));
        assert_eq!(agent_of("/hook?from=wakuwaku"), None);
        assert_eq!(agent_of("/hook?agent=codexx"), None);
        assert_eq!(agent_of("/hook"), None);
        // omp's come from inside omp.exe: she finds its process, as Claude Code's.
        assert!(brings_chain(Some("codex")) && brings_chain(Some("agy")) && brings_chain(Some("zcode")) && brings_chain(Some("reasonix")));
        assert!(!brings_chain(Some("omp")) && !brings_chain(Some("opencode")) && !brings_chain(Some("dsh")) && !brings_chain(None));
        // Reasonix's PermissionRequest is told, not held: its hook waits on no answer.
        let asked = json!({ "hook_event_name": "PermissionRequest" });
        assert!(answers_none(Some("reasonix"), &asked));
        assert!(!answers_none(None, &asked) && !answers_none(Some("codex"), &asked) && !answers_none(Some("agy"), &asked) && !answers_none(Some("zcode"), &asked) && !answers_none(Some("opencode"), &asked));
        // DSH's is held only when her plugin can take her answer to it.
        assert!(!answers_none(Some("dsh"), &json!({ "hook_event_name": "PermissionRequest", "answerable": true })));
        assert!(answers_none(Some("dsh"), &json!({ "hook_event_name": "PermissionRequest", "answerable": false })));
        assert!(answers_none(Some("dsh"), &asked));
    }

    // POST /quit is a quit by choice (the hooks then start her no more)
    // unless it asks to be woken: ?wake=1.
    #[test]
    fn quit_by_choice_unless_asked_to_wake() {
        use crate::data::Quit;
        assert_eq!(quit_of("/quit"), (Quit::ByChoice, "POST /quit"));
        assert_eq!(quit_of("/quit?"), (Quit::ByChoice, "POST /quit"));
        assert_eq!(quit_of("/debug/quit"), (Quit::ByChoice, "POST /quit"));
        assert_eq!(quit_of("/quit?wake=1"), (Quit::Wakeable, "POST /quit?wake=1"));
        assert_eq!(quit_of("/quit?wake=true"), (Quit::Wakeable, "POST /quit?wake=1"));
        assert_eq!(quit_of("/quit?from=test&wake=1"), (Quit::Wakeable, "POST /quit?wake=1"));
        for url in ["/quit?wake=0", "/quit?wake=", "/quit?wake", "/quit?awake=1", "/quit?wake=10", "/quit?x=wake=1"] {
            assert_eq!(quit_of(url).0, Quit::ByChoice, "{url}");
        }
    }

    // A connection whose socket cannot be split in two (WSADuplicateSocket's
    // WSAEINVAL, as a copy started elevated in the test VM met) is closed
    // unanswered, and the next one is served: tiny_http 0.12.0 unwrapped it
    // on its accept thread, which with panic = "abort" ended her.
    #[test]
    fn a_connection_she_cannot_split_is_dropped_and_the_next_served() {
        use std::io::{ErrorKind, Write};
        use std::time::Duration;
        let (tx, told) = std::sync::mpsc::channel();
        assert!(tiny_http::tell_failures(move |line| drop(tx.send(line.to_string()))));
        let server = Server::http(("127.0.0.1", 0)).unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let ask = move || -> std::io::Result<String> {
            let mut conn = std::net::TcpStream::connect(("127.0.0.1", port))?;
            conn.set_read_timeout(Some(Duration::from_secs(5)))?;
            conn.write_all(b"GET /health HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")?;
            let mut answer = String::new();
            conn.read_to_string(&mut answer)?;
            Ok(answer)
        };
        server.fail_next_split();
        // Closed, or reset for the request left unread; never left open.
        match ask() {
            Ok(answer) => assert_eq!(answer, "", "nothing answered"),
            Err(e) => assert!(!matches!(e.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock), "left open: {e}"),
        }
        assert!(server.try_recv().unwrap().is_none(), "no request from it");
        let line = told.recv_timeout(Duration::from_secs(1)).expect("told");
        assert_eq!(line, "a connection dropped, its socket not split: a split failed on purpose");
        let asker = std::thread::spawn(ask);
        let req = server.recv_timeout(Duration::from_secs(5)).unwrap().expect("the next request");
        assert_eq!(req.url(), "/health");
        req.respond(Response::from_string("{}")).unwrap();
        let answer = asker.join().unwrap().unwrap();
        assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
    }

    // The two halves of a connection share its one socket (no second handle,
    // which a process started from an elevated one may not have): requests
    // one after another on one connection are each read and answered, and it
    // closes when the last asks it to.
    #[test]
    fn one_connection_carries_request_after_request() {
        use std::io::Write;
        use std::time::Duration;
        let server = Server::http(("127.0.0.1", 0)).unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let asker = std::thread::spawn(move || -> std::io::Result<String> {
            let mut conn = std::net::TcpStream::connect(("127.0.0.1", port))?;
            conn.set_read_timeout(Some(Duration::from_secs(5)))?;
            conn.write_all(b"GET /one HTTP/1.1\r\nHost: x\r\n\r\nGET /two HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")?;
            let mut answer = String::new();
            conn.read_to_string(&mut answer)?;
            Ok(answer)
        });
        for url in ["/one", "/two"] {
            let req = server.recv_timeout(Duration::from_secs(5)).unwrap().expect(url);
            assert_eq!(req.url(), url);
            req.respond(Response::from_string(&url[1..])).unwrap();
        }
        let answer = asker.join().unwrap().unwrap();
        assert_eq!(answer.matches("HTTP/1.1 200").count(), 2, "{answer}");
        assert!(answer.ends_with("two"), "{answer}");
    }

    // /v1 is served to a request that names this machine as its host, not to
    // a web page's own name made to point here.
    #[test]
    fn v1_is_for_this_machine_by_name() {
        for host in ["127.0.0.1", "127.0.0.1:47213", "localhost:47213", "LOCALHOST", "[::1]:47213", "[::1]"] {
            assert!(is_local_host(host), "{host}");
        }
        for host in ["", "evil.example:47213", "127.0.0.1.evil.example", "localhost.evil.example:80", "127.0.0.2:47213", "localhost:", "[::1]x"] {
            assert!(!is_local_host(host), "{host}");
        }
    }

    // An event of the hub's reaches a client of /v1/events as it is
    // published, not when the stream ends (tiny_http sent a chunked body's
    // 8 KB at a time, flushed at its end: vendor/tiny_http/README.md).
    #[test]
    fn an_event_reaches_its_reader_as_it_comes() {
        use std::io::{BufRead, BufReader, Write};
        use std::time::{Duration, Instant};
        use agent_core::event::Kind;
        let server = Server::http(("127.0.0.1", 0)).unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let reader = std::thread::spawn(move || -> std::io::Result<(String, Vec<String>)> {
            let mut conn = std::net::TcpStream::connect(("127.0.0.1", port))?;
            conn.set_read_timeout(Some(Duration::from_secs(5)))?;
            conn.write_all(b"GET /v1/events HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")?;
            let (mut head, mut lines) = (String::new(), Vec::new());
            for line in BufReader::new(conn).lines() {
                let line = line?;
                if line.starts_with("data: ") {
                    lines.push(line);
                    break;
                }
                if lines.is_empty() && head.lines().last() != Some("") {
                    head.push_str(&line);
                    head.push('\n');
                }
            }
            Ok((head, lines))
        });
        let hub = agent_hub::Hub::with_token("t".repeat(32), "Z:\\agent-hub\\token".into(), 1);
        let req = server.recv_timeout(Duration::from_secs(5)).unwrap().expect("the stream asked for");
        stream(req, hub.events("", None).unwrap());
        let t = Instant::now();
        let ev = AgentEvent { agent: Agent::Claude, session: "a".into(), within: false, cwd: Some("Z:\\p".into()), turn: None, title: None, reply: None, kind: Kind::Working };
        hub.publish(&ev, 2_000);
        let (head, lines) = reader.join().unwrap().unwrap();
        assert!(t.elapsed() < Duration::from_secs(2), "came at once");
        assert!(head.starts_with("HTTP/1.1 200") && head.to_lowercase().contains("content-type: text/event-stream") && head.to_lowercase().contains("transfer-encoding: chunked"), "{head}");
        let data: Value = serde_json::from_str(&lines[0]["data: ".len()..]).unwrap();
        assert_eq!((data["seq"].as_u64(), data["type"].as_str(), data["session"].as_str()), (Some(1), Some("working"), Some("a")));
    }

    #[test]
    fn what_tiny_http_tells_is_logged_once() {
        let mut said = Vec::new();
        let dropped = "a connection dropped, its socket not split: os error 10022";
        assert!(first_time(&mut said, dropped));
        assert!(!first_time(&mut said, dropped));
        assert!(first_time(&mut said, "the accept thread ended: os error 10055"));
        for n in 0..10 {
            first_time(&mut said, &format!("{n}"));
        }
        assert_eq!(said.len(), 8, "a few kinds kept, no more");
    }
}
