// How Claude Code and Codex reach her: the plugin, or hooks written into
// Claude Code's settings.json; hooks written into Codex's hooks.json (and
// Antigravity's; OpenCode gets a plugin file of ours, omp an extension in
// its agent folder). Read on demand (the person may change any of them at
// any time), and written only when they ask, from the settings.
//
// Each agent's own in a file of its own (claude.rs, codex.rs, agy.rs,
// opencode.rs, omp.rs, zcode.rs, dsh.rs, reasonix.rs), what they share in
// file.rs, their versions in versions.rs; here, what is of them all.
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use agent_core::event::Agent;
use serde_json::{json, Map, Value};

mod agy;
mod claude;
mod codex;
mod dsh;
mod file;
mod omp;
mod opencode;
mod reasonix;
mod versions;
mod zcode;

pub use agy::*;
pub use claude::*;
pub use codex::*;
pub use dsh::*;
pub use file::*;
pub use omp::*;
pub use opencode::*;
pub use reasonix::*;
pub use versions::*;
pub use zcode::*;

// What kept a settings file from being written, for what to do about it:
// locked (another program has it open), invalid (not JSON), flowList (DSH's
// patch file with its list as [ … ]), path (agy can run no program from
// where she is), pathChars (nor Reasonix's cmd, for a character in that
// path), old (a Reasonix 1.x), other. From
// the error's text: io's ends "(os error 32)" (or 33) whatever the
// language, serde_json's says where, "at line 3 column 5".
pub fn error_kind(error: &str) -> &'static str {
    if error.contains(AGY_NO_NAME) {
        "path"
    } else if error.contains(REASONIX_NO_NAME) {
        "pathChars"
    } else if error.contains(REASONIX_OLD) {
        "old"
    } else if error.contains("(os error 32)") || error.contains("(os error 33)") {
        "locked"
    } else if error.contains(DSH_FLAT) {
        "flowList"
    } else if let Some(rest) = error.strip_prefix(DSH_BRIDGE_FAILED) {
        // The bridge's packages: why, as dsh_bridge.rs tells it.
        match rest.trim_start().strip_prefix('[').and_then(|r| r.split(']').next()).unwrap_or("") {
            "registry" => "bridgeRegistry",
            "version" => "bridgeVersion",
            "notFound" => "bridgeNotFound",
            "releaseAge" => "bridgeReleaseAge",
            "timeout" => "bridgeTimeout",
            "runtime" => "bridgeRuntime",
            "busy" => "bridgeBusy",
            "disk" => "bridgeDisk",
            "permission" => "bridgePermission",
            _ => "bridgeOther",
        }
    } else if error.contains(" at line ") && error.contains(" column ") {
        "invalid"
    } else {
        "other"
    }
}

// Whether any of them is connected, as last looked (at start, as the
// settings read it, after connecting) or heard (any event from them): what
// the island asks for while it is not (island.ts), without reading their
// settings files on every redraw. Connected until looked, so a first draw
// never asks wrongly.
static CONNECTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn note_connected(connected: bool) -> bool {
    CONNECTED.swap(connected, std::sync::atomic::Ordering::SeqCst) != connected
}

// Whether any of them is, as their files say now: for looking (below) and
// for the settings' snapshot alike (settings.rs; its own list there once
// left ZCode, DeepSeek Harness and Reasonix out, and the island asked to
// connect one of them while it was).
pub fn any_connected(port: u16) -> bool {
    ALL.iter().any(|&agent| connected(agent, port))
}

pub fn look_connected(port: u16) -> bool {
    let connected = any_connected(port);
    note_connected(connected);
    connected
}

pub fn seems_connected() -> bool {
    CONNECTED.load(std::sync::atomic::Ordering::SeqCst)
}

// The tools on this computer by name, in their order (the island names
// them while none is connected): Claude Code by its folder, Codex,
// Antigravity, OpenCode, omp, ZCode, DeepSeek Harness and Reasonix by
// theirs; none found, all but omp, ZCode, DeepSeek Harness and Reasonix
// (it can not be told).
pub fn tools_here() -> Vec<&'static str> {
    let here: Vec<&str> = ALL.into_iter().filter(|&agent| here(agent)).map(name).collect();
    if here.is_empty() {
        [Agent::Claude, Agent::Codex, Agent::Agy, Agent::Opencode].map(name).to_vec()
    } else {
        here
    }
}

// --- Each of them, by its name in agent-core (Agent) -------------------------------
//
// One match for each thing asked of them all, so that an agent added to
// agent-core is asked for here, by the compiler, in each.

// The agents she connects, in the order she names them (tools_here; the
// Agent page has its own, agents.ts).
pub const ALL: [Agent; 8] = [Agent::Claude, Agent::Codex, Agent::Agy, Agent::Opencode, Agent::Omp, Agent::Zcode, Agent::Dsh, Agent::Reasonix];

// Its place in ALL: what is kept for each of them goes by it (shared.rs seen).
pub fn slot(agent: Agent) -> usize {
    ALL.iter().position(|&a| a == agent).expect("every agent is in ALL")
}

// Its id, as the pages and her port name it (agent-core's tag; Claude
// Code's, which has none, "claude"), and the agent by it.
pub fn id(agent: Agent) -> &'static str {
    agent.tag().unwrap_or("claude")
}

pub fn by_id(id: &str) -> Option<Agent> {
    ALL.into_iter().find(|&agent| self::id(agent) == id)
}

// Its name, as the island says it.
pub fn name(agent: Agent) -> &'static str {
    match agent {
        Agent::Claude => "Claude Code",
        Agent::Codex => "Codex",
        Agent::Agy => "Antigravity",
        Agent::Opencode => "OpenCode",
        Agent::Omp => "omp",
        Agent::Zcode => "ZCode",
        Agent::Dsh => "DeepSeek Harness",
        Agent::Reasonix => "Reasonix",
    }
}

// Whether it is on this computer, as far as can be told: its folder.
pub fn here(agent: Agent) -> bool {
    match agent {
        Agent::Claude => claude_here(),
        Agent::Codex => codex_home().is_dir(),
        Agent::Agy => agy_here(),
        Agent::Opencode => opencode_here(),
        Agent::Omp => omp_here(),
        Agent::Zcode => zcode_here(),
        Agent::Dsh => dsh_here(),
        Agent::Reasonix => reasonix_here(),
    }
}

// The file ours is written into: omp's in its first agent folder (none
// when it has none), DeepSeek Harness's its home-level patch file.
pub fn file(agent: Agent) -> Option<PathBuf> {
    match agent {
        Agent::Claude => Some(claude_settings_file()),
        Agent::Codex => Some(codex_hooks_file()),
        Agent::Agy => Some(agy_hooks_file()),
        Agent::Opencode => Some(opencode_plugin_file()),
        Agent::Omp => omp_files().into_iter().next(),
        Agent::Zcode => Some(zcode_config_file()),
        Agent::Dsh => Some(dsh_patch_file()),
        Agent::Reasonix => Some(reasonix_settings_file()),
    }
}

// How ours stands in its files, in its own words (each file's): absent (not
// on this computer), unreadable, missing, ok, stale and the like. Claude
// Code's is its settings.json's hooks (its plugin: connection); DeepSeek
// Harness's the way its rows have it (dsh_view tells the way chosen).
pub fn status(agent: Agent, port: u16) -> &'static str {
    match agent {
        Agent::Claude => hooks_status(port),
        Agent::Codex => codex_hooks_status(),
        Agent::Agy => agy_hooks_status(),
        Agent::Opencode => opencode_status(port),
        Agent::Omp => omp_hooks_status(port),
        Agent::Zcode => zcode_hooks_status(),
        Agent::Dsh => dsh_rows_status(port, &Value::Null).1,
        Agent::Reasonix => reasonix_hooks_status(),
    }
}

// Whether ours is there, working or to be repaired: Claude Code's plugin or
// hooks, any other's statuses as counts_in has them.
pub fn connected(agent: Agent, port: u16) -> bool {
    match agent {
        Agent::Claude => connection(port) != "none",
        _ => counts_in(agent, status(agent, port)),
    }
}

// The statuses that count as ours being there: each the page's `in` too
// (agents.ts; its DeepSeek Harness's words its own, from these two).
fn counts_in(agent: Agent, status: &str) -> bool {
    match agent {
        Agent::Claude => !matches!(status, "missing" | "unreadable"),
        Agent::Codex => matches!(status, "ok" | "stale" | "upgradable" | "partial"),
        Agent::Agy | Agent::Opencode | Agent::Dsh => matches!(status, "ok" | "stale"),
        Agent::Omp | Agent::Zcode | Agent::Reasonix => matches!(status, "ok" | "stale" | "partial"),
    }
}

// What a write takes besides the agent and the action: her port, Codex's
// words for a prompt waiting on her (its hooks carry them), and for
// DeepSeek Harness the way it goes (dsh_way) and its lines for her log.
pub struct Writing<'a> {
    pub port: u16,
    pub waiting: &'a str,
    pub dsh_way: &'a str,
    pub log: &'a mut Vec<String>,
}

// Write ours into its files, or take it out. action: install, remove;
// Claude Code's install-http too, DeepSeek Harness's install:npmmirror (the
// bridge's packages from npmmirror).
pub fn write(agent: Agent, action: &str, w: &mut Writing) -> Result<(), String> {
    match agent {
        Agent::Claude => write_hooks(action, w.port),
        Agent::Codex => write_codex_hooks(action, w.waiting),
        Agent::Agy => write_agy_hooks(action),
        Agent::Opencode => write_opencode_plugin(action, w.port),
        Agent::Omp => write_omp_hooks(action, w.port),
        Agent::Zcode => write_zcode_hooks(action),
        Agent::Dsh => {
            let (action, registry) = match action.split_once(':') {
                Some((action, "npmmirror")) => (action, Some(crate::dsh_bridge::NPMMIRROR)),
                _ => (action, None),
            };
            write_dsh_hooks(action, w.dsh_way, w.port, registry, w.log)
        }
        Agent::Reasonix => write_reasonix_hooks(action),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_write_that_failed_says_why() {
        assert_eq!(super::error_kind("另一个程序正在使用此文件，进程无法访问。 (os error 32)"), "locked");
        assert_eq!(super::error_kind("expected `,` or `}` at line 3 column 5"), "invalid");
        assert_eq!(super::error_kind("拒绝访问。 (os error 5)"), "other");
    }

    use super::*;

    // Every agent once, each with a name of its own: what any_connected and
    // tools_here go through (the settings' own list of them once left three
    // out). A new agent goes here as into ALL.
    #[test]
    fn all_of_them_once() {
        let every = [Agent::Claude, Agent::Codex, Agent::Agy, Agent::Opencode, Agent::Omp, Agent::Zcode, Agent::Dsh, Agent::Reasonix];
        for agent in every {
            assert_eq!(ALL.iter().filter(|&&a| a == agent).count(), 1, "{agent:?} in ALL once");
        }
        assert_eq!(ALL.len(), every.len());
        let names: std::collections::HashSet<&str> = ALL.into_iter().map(name).collect();
        assert_eq!(names.len(), ALL.len());
        for (at, agent) in ALL.into_iter().enumerate() {
            assert_eq!(slot(agent), at);
            assert_eq!(by_id(id(agent)), Some(agent));
        }
        assert_eq!(by_id("claude"), Some(Agent::Claude));
        assert_eq!(by_id("gemini"), None);
    }

    // What counts as ours being there, here and on the page (agents.ts, each
    // agent's `in`): the same statuses, so the island and the Agent page
    // never tell one apart. Claude Code's counts by its plugin or hooks
    // (connection), and the page's DeepSeek Harness words come of ok and
    // stale (dsh_view): those two count, and are among its own.
    #[test]
    fn connected_as_the_page_counts_it() {
        let page = include_str!("../../../src/pet/agents.ts");
        let mut ins: Vec<(Agent, Vec<String>)> = Vec::new();
        for part in page.split("\n    id: '").skip(1) {
            let id = &part[..part.find('\'').unwrap()];
            let list = part.split("\n    in: [").nth(1).and_then(|rest| rest.split(']').next()).unwrap_or_else(|| panic!("{id}'s in"));
            let agent = by_id(id).unwrap_or_else(|| panic!("{id}: an id settings_agent knows"));
            ins.push((agent, list.split(',').map(|s| s.trim().trim_matches('\'').to_string()).filter(|s| !s.is_empty()).collect()));
        }
        assert_eq!(ins.len(), ALL.len(), "the page's table has them all");
        let mut every: Vec<String> = ins.iter().flat_map(|(_, list)| list.clone()).collect();
        every.extend(["absent", "missing", "unreadable", "off", "unsupported"].map(String::from));
        for (agent, list) in &ins {
            match agent {
                Agent::Claude => {}
                Agent::Dsh => {
                    let counted: Vec<&String> = every.iter().filter(|s| counts_in(*agent, s)).collect();
                    assert!(counted.iter().all(|s| ["ok", "stale"].contains(&s.as_str())), "{counted:?}");
                    assert!(list.contains(&"ok".to_string()) && list.contains(&"stale".to_string()));
                }
                _ => {
                    for status in &every {
                        assert_eq!(counts_in(*agent, status), list.contains(status), "{agent:?} {status}");
                    }
                }
            }
        }
    }
}
