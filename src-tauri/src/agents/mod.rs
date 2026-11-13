// The agents' side of her: the sessions she follows (Claude Code, Codex,
// Antigravity, OpenCode, omp, ZCode, DeepSeek Harness, Reasonix), from
// their hooks to what she says of them. What a session is doing is read
// by agent-core (its own repository: docs/DEVELOPMENT.md); here is how
// she is connected to each, how its events reach her, and what she makes
// of them. Together in this folder since 2026-11-14: these keep to
// themselves (of her other parts they ask only tasks.rs, for a window's
// app), and the rest come in by jump, connection and state.
//
//   connection/    each agent's hooks, plugin or extension, written and read (one file each)
//   hooks.rs       her exe run as an agent's hook: the event handed to her port
//   dsh_bridge.rs  DeepSeek Harness's own hook bridge, set up and looked after
//   events/        the agents' events into her words (mod.rs); agy.rs, dsh.rs: their
//                  log lines; tests.rs: what she makes of the others'
//   state.rs       per-session moods, and the one shown
//   asks.rs        prompts answered on her or the island
//   app_asks.rs    another program's approval on her prompt panel (POST /ask)
//   jump.rs        to a session's window, when you click the session
//   tokens.rs      today's tokens, from each agent's own records
//   replay.rs      her tests' replays of each agent's events (test/fixtures/replay)
pub mod app_asks;
pub mod asks;
pub mod connection;
pub mod dsh_bridge;
pub mod events;
pub mod hooks;
pub mod jump;
#[cfg(test)]
mod replay;
pub mod state;
pub mod tokens;
