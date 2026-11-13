// Agents' sessions replayed. Each fixture (test/fixtures/replay/<name>.json)
// is a list of events as an agent sends them to /hook, run through what the
// server does with one (server.rs read_event and event_for: agent-core's
// AgentEvent) and into her sessions (state.rs), with now and then what time
// does to them. Every step's AgentEvent, what it did to her, her sessions
// after it, and agent-core's registry as the apps listening are told it
// (/v1/sessions: no seen, no rest) are kept beside it (<name>.out.json):
// what an agent's events come to changes only on purpose. REPLAY_BLESS=1 writes them
// anew; then `git diff` shows what changed.
//
// A fixture:
//   agent    whose events (codex, agy, opencode, omp, zcode, reasonix, dsh;
//            none for Claude Code); a step may say its own
//   hold     how many seconds an ending stays (none: until seen)
//   dsh      DeepSeek Harness's folder as the fixture has it: logs
//            [{ cwd, session, file }] (file under test/fixtures), titles
//            { session: title } (its projection cache)
//   steps    { at (ms), event, agent?, held? (a prompt she holds) }, or
//            { at, do: seen | tick | stale | hide, min? (hide's minutes) }
// "$REPLAY/<file>" in an event's text is that file in the fixtures' folder.
//
// Each session's id is the fixture's own (replay-...): what agent-core
// remembers of a session (its project, DSH's whose, Reasonix's asks) is kept
// for the whole test run.
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use agent_core::registry::SessionRegistry;

use crate::server::{event_for, read_event};
use crate::agents::state::{Hold, Pet, IDLE_HIDE_MIN};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("test").join("fixtures")
}

fn folder() -> PathBuf {
    fixtures().join("replay")
}

// "$REPLAY/x" made that file's path, anywhere in a value.
fn placed(v: &Value, dir: &str) -> Value {
    match v {
        Value::String(s) => s.strip_prefix("$REPLAY/").map_or_else(|| v.clone(), |rest| json!(format!("{dir}/{rest}"))),
        Value::Array(a) => Value::Array(a.iter().map(|x| placed(x, dir)).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), placed(x, dir))).collect()),
        _ => v.clone(),
    }
}

// DeepSeek Harness's folder for a fixture: its sessions' logs where DSH keeps
// them, their names in its projection cache.
fn dsh_home(name: &str, dsh: &Value) -> PathBuf {
    let home = std::env::temp_dir().join(format!("wakuwaku-replay-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    for log in dsh["logs"].as_array().into_iter().flatten() {
        let (cwd, session, file) = (log["cwd"].as_str().unwrap(), log["session"].as_str().unwrap(), log["file"].as_str().unwrap());
        let dir = home.join("sessions").join(agent_core::dsh::project_key(cwd)).join(session);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::copy(fixtures().join(file), dir.join("session.v4.jsonl.zstd")).unwrap();
    }
    for (session, title) in dsh["titles"].as_object().into_iter().flatten() {
        let dir = home.join("storages").join("session_projcache").join("sessions");
        std::fs::create_dir_all(&dir).unwrap();
        let record = json!({ "version": 7, "record": { "rows": { "title": { "ver": 1, "seq": 1, "val": title } } } });
        std::fs::write(dir.join(format!("{session}.json")), record.to_string()).unwrap();
    }
    home
}

// An event's name, whatever the agent calls it.
fn name_of(e: &Value) -> Value {
    ["hook_event_name", "type", "event"].iter().find_map(|k| e.get(*k).filter(|v| v.is_string()).cloned()).unwrap_or(Value::Null)
}

// A fixture run: what each step came to.
fn run(name: &str) -> Value {
    let dir = folder();
    let text = std::fs::read_to_string(dir.join(format!("{name}.json"))).unwrap();
    let fixture: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{name}.json: {e}"));
    let fixture = placed(&fixture, &dir.to_string_lossy().replace('\\', "/"));
    let hold = fixture["hold"].as_u64().map_or(Hold::Seen, Hold::Secs);
    let home = dsh_home(name, &fixture["dsh"]);
    let mut pet = Pet::default();
    // The sessions as agent-core keeps them for the apps listening.
    let mut registry = SessionRegistry::default();
    let mut out = Vec::new();
    for step in fixture["steps"].as_array().unwrap_or_else(|| panic!("{name}: no steps")) {
        let at = step["at"].as_u64().unwrap_or_else(|| panic!("{name}: a step with no at"));
        let mut kept = Map::new();
        kept.insert("at".into(), json!(at));
        if let Some(what) = step["do"].as_str() {
            let changed = match what {
                "seen" => pet.seen(at),
                "tick" => pet.tick(at),
                "stale" => {
                    registry.check_stale(at);
                    pet.check_stale(at)
                }
                "hide" => pet.hide_quiet(at, step["min"].as_u64().unwrap_or(IDLE_HIDE_MIN) * 60_000),
                _ => panic!("{name}: no step {what}"),
            };
            kept.insert("do".into(), json!(what));
            kept.insert("changed".into(), json!(changed));
        } else {
            let agent = step["agent"].as_str().or(fixture["agent"].as_str());
            kept.insert("event".into(), name_of(&step["event"]));
            match read_event(agent, step["event"].clone()) {
                None => drop(kept.insert("taken".into(), json!(false))),
                Some(mut event) => {
                    // OpenCode's in Claude Code's names, as the prompts read it.
                    if agent == Some("opencode") {
                        kept.insert("read".into(), event.clone());
                    }
                    let held = step["held"].as_bool() == Some(true);
                    // What it says the session is doing (agent-core), and what it does to hers.
                    let read = event_for(agent, &mut event, held, at, &home);
                    let outcome = read.as_ref().and_then(|ev| pet.apply(ev, None, at, hold));
                    if let Some(ev) = &read {
                        registry.apply(ev, at);
                    }
                    kept.insert("agentEvent".into(), read.map_or(Value::Null, |ev| serde_json::to_value(ev).unwrap()));
                    let outcome = outcome.map_or(Value::Null, |o| json!({ "changed": o.changed, "react": o.react, "alerts": o.alerts, "turns": o.turns, "workedMs": o.worked_ms }));
                    kept.insert("outcome".into(), outcome);
                }
            }
        }
        let top = pet.get(at);
        kept.insert("shown".into(), json!({ "id": top["id"], "mood": top["mood"], "others": top["others"], "sessions": top["sessions"] }));
        let mut sessions = pet.list().as_array().cloned().unwrap_or_default();
        sessions.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
        kept.insert("sessions".into(), Value::Array(sessions));
        let mut known: Vec<Value> = registry.list().into_iter().map(|s| serde_json::to_value(s).unwrap()).collect();
        known.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
        kept.insert("registry".into(), Value::Array(known));
        out.push(Value::Object(kept));
    }
    let _ = std::fs::remove_dir_all(&home);
    Value::Array(out)
}

fn pretty(v: Option<&Value>) -> String {
    v.map_or("(none)".into(), |v| serde_json::to_string_pretty(v).unwrap())
}

#[test]
fn agents_sessions_replayed() {
    let _asks = crate::agents::events::others::reasonix_asks_held();
    let bless = std::env::var("REPLAY_BLESS").as_deref() == Ok("1");
    let mut names: Vec<String> = std::fs::read_dir(folder())
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|f| f.ends_with(".json") && !f.ends_with(".out.json"))
        .map(|f| f.trim_end_matches(".json").to_string())
        .collect();
    names.sort();
    assert!(!names.is_empty(), "no fixtures in {}", folder().display());
    let mut failed = Vec::new();
    for name in &names {
        let got = run(name);
        let file = folder().join(format!("{name}.out.json"));
        if bless {
            std::fs::write(&file, format!("{}\n", serde_json::to_string_pretty(&got).unwrap())).unwrap();
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&file) else {
            failed.push(format!("{name}: no {name}.out.json (REPLAY_BLESS=1 writes it)"));
            continue;
        };
        let want: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{name}.out.json: {e}"));
        if got != want {
            let (got, want) = (got.as_array().cloned().unwrap_or_default(), want.as_array().cloned().unwrap_or_default());
            let step = (0..got.len().max(want.len())).find(|&i| got.get(i) != want.get(i)).unwrap_or(0);
            failed.push(format!("{name}: step {step} is not what it was\nwas: {}\nnow: {}", pretty(want.get(step)), pretty(got.get(step))));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n\n"));
}
