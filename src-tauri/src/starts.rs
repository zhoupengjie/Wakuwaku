// What starts her, and whether it may (zhoupengjie/Wakuwaku#57).
//
// She is started:
//   - by hand (her exe opened from Explorer, a shortcut), or by any program
//     that runs her exe (a terminal, a script);
//   - at login: her logon task or her Run value (logon.rs started_by);
//   - by a session's hook, her not up as its session starts: Claude Code's
//     SessionStart command (--wakuwaku-ensure-running, written into
//     settings.json; Claude's desktop app's Code tab runs the same, in the
//     app's job), omp's extension (the same flag, "omp" and how its session
//     started after it), and Codex's, Antigravity's, ZCode's and Reasonix's
//     hook commands at SessionStart (hooks.rs). DeepSeek Harness's never starts
//     her (she would run in its sandbox); Claude Code's HTTP hooks, its
//     plugin's and OpenCode's plugin cannot start anything;
//   - by herself while she runs: an update's new version, or her exe again
//     once WebView2's browser went (updates.rs hand_over), and her exe out
//     of a starter's job (detach.rs should_leave).
// Her taskbar's guard never starts her: it waits on her, and gives back.
//
// A hook starts her only as its session starts, and never once she was quit
// by choice (data.rs Quit): starts_her. What started her is told in one line
// of her log as she is up ("started by: ..."): a hook, an update and a
// restart say it after BY_FLAG; at login, her logon task's or Run value's
// flag does; else it is the programs above her (explorer.exe: by hand).
use serde_json::Value;

pub const BY_FLAG: &str = "--wakuwaku-by";

// Her hook commands that start her when she is not up (hooks.rs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hook {
    // --wakuwaku-ensure-running: run only as a session starts.
    Ensure,
    Codex,
    Agy,
    Zcode,
    Reasonix,
}

// Whether a hook finding her not up starts her: as its session starts,
// unless she was quit by choice (asked last, and only then: a look into her
// data folder).
pub fn starts_her(hook: Hook, event: &str, quit_by_choice: impl FnOnce() -> bool) -> bool {
    let at_start = match hook {
        Hook::Ensure => true,
        Hook::Codex | Hook::Agy | Hook::Zcode | Hook::Reasonix => event == "SessionStart",
    };
    at_start && !quit_by_choice()
}

// Words safe as one part of a command line (detach.rs quotes each part and
// escapes nothing): no quote, no backslash, nothing unprintable; not long.
fn clean(text: &str) -> String {
    text.chars().filter(|c| !matches!(c, '"' | '\\') && !c.is_control()).take(300).collect()
}

// A hook's words for her log: the agent, its event (and what more it says:
// how the session started, which of Claude Code's front ends), and the
// programs above the hook, the nearest first.
pub fn hook_who(agent: &str, event: &str, more: &[&str], parents: &[String]) -> String {
    let more: Vec<&str> = more.iter().copied().filter(|m| !m.is_empty()).collect();
    let mut who = format!("{agent} {event}");
    if !more.is_empty() {
        who.push_str(&format!(" ({})", more.join(", ")));
    }
    if !parents.is_empty() {
        who.push_str(&format!(", under {}", parents.join(" ← ")));
    }
    clean(&who)
}

// Who ran the ensure command: omp's extension says so after the flag, and
// how its session started; Claude Code's gives its event on stdin, and runs
// it with CLAUDE_CODE_ENTRYPOINT set (`claude`: cli, claude-desktop for the
// desktop app's Code tab, ...; "" when only CLAUDECODE says it is Claude Code).
pub fn ensure_who(after_flag: &[String], event: Option<&Value>, claude: Option<&str>, parents: &[String]) -> String {
    if after_flag.first().map(String::as_str) == Some("omp") {
        return hook_who("omp", "SessionStart", &[after_flag.get(1).map_or("", String::as_str)], parents);
    }
    let field = |key: &str| event.and_then(|e| e.get(key)).and_then(Value::as_str).unwrap_or("");
    let agent = if claude.is_some() { "Claude Code" } else { "an agent" };
    let name = match field("hook_event_name") {
        "" => "--wakuwaku-ensure-running (no event)",
        name => name,
    };
    hook_who(agent, name, &[field("source"), claude.unwrap_or("")], parents)
}

// What a starter said (BY_FLAG's word), if it did.
pub fn by_of(args: &[String]) -> Option<&str> {
    let at = args.iter().position(|a| a == BY_FLAG)?;
    args.get(at + 1).map(String::as_str)
}

// What started her, for "started by: ..." (main.rs): what the starter said
// (BY_FLAG), her logon task or Run value, an older version's update or
// restart (--after-update with no word), or else the programs above her
// (`parents`, asked only then): explorer.exe first, by hand. Started again
// out of a starter's job (detach.rs FLAG): said too.
pub fn starter(args: &[String], parents: impl FnOnce() -> Vec<String>) -> String {
    let mut who = if let Some(by) = by_of(args) {
        by.to_string()
    } else if let Some(by) = crate::logon::started_by(args) {
        if by == crate::logon::BY_TASK { "her logon task" } else { "her Run value, at logon" }.to_string()
    } else if let Some(at) = args.iter().position(|a| a == crate::updates::AFTER_UPDATE) {
        format!("an update or a restart by an older version of hers (pid {})", args.get(at + 1).map_or("?", String::as_str))
    } else {
        let parents = parents();
        match parents.first().map(String::as_str) {
            None => "a program gone since".to_string(),
            Some("explorer.exe") => "hand, from Explorer".to_string(),
            Some(_) => format!("a program: {}", parents.join(" ← ")),
        }
    };
    if args.iter().any(|a| a == crate::detach::FLAG) {
        who.push_str(", started again out of its job");
    }
    clean(&who)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn args(line: &[&str]) -> Vec<String> {
        std::iter::once("wakuwaku.exe").chain(line.iter().copied()).map(String::from).collect()
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    // Each hook that may start her does so only as its session starts, and
    // never once she was quit by choice; her data folder is looked into only
    // when it would start her.
    #[test]
    fn hooks_start_her_only_as_a_session_starts_and_never_after_a_quit_by_choice() {
        for hook in [Hook::Ensure, Hook::Codex, Hook::Agy, Hook::Zcode, Hook::Reasonix] {
            assert!(starts_her(hook, "SessionStart", || false), "{hook:?}");
            assert!(!starts_her(hook, "SessionStart", || true), "{hook:?} after a quit by choice");
        }
        for event in ["UserPromptSubmit", "PreToolUse", "PostToolUse", "PreInvocation", "Stop", "PermissionRequest", "SessionEnd", ""] {
            for hook in [Hook::Codex, Hook::Agy, Hook::Zcode, Hook::Reasonix] {
                assert!(!starts_her(hook, event, || panic!("looked for the mark for {hook:?} {event}")), "{hook:?} {event}");
            }
        }
        // The ensure command runs only at SessionStart (Claude Code's, omp's).
        assert!(starts_her(Hook::Ensure, "", || false));
        assert!(!starts_her(Hook::Ensure, "", || true));
    }

    #[test]
    fn a_hook_says_who_it_is() {
        let parents = names(&["claude.exe", "pwsh.exe", "windowsterminal.exe"]);
        let event = json!({ "hook_event_name": "SessionStart", "source": "resume", "session_id": "s1" });
        assert_eq!(ensure_who(&[], Some(&event), Some("cli"), &parents), "Claude Code SessionStart (resume, cli), under claude.exe ← pwsh.exe ← windowsterminal.exe");
        // The desktop app's Code tab.
        let event = json!({ "hook_event_name": "SessionStart", "source": "compact" });
        assert_eq!(ensure_who(&[], Some(&event), Some("claude-desktop"), &names(&["claude.exe", "claude.exe"])), "Claude Code SessionStart (compact, claude-desktop), under claude.exe ← claude.exe");
        // omp's extension: no event on stdin, its words after the flag.
        assert_eq!(ensure_who(&names(&["omp", "resume"]), None, None, &names(&["omp.exe"])), "omp SessionStart (resume), under omp.exe");
        // Nobody known.
        assert_eq!(ensure_who(&[], None, None, &[]), "an agent --wakuwaku-ensure-running (no event)");
        assert_eq!(ensure_who(&[], None, Some(""), &[]), "Claude Code --wakuwaku-ensure-running (no event)");
        assert_eq!(hook_who("Codex", "SessionStart", &["startup"], &names(&["pwsh.exe", "codex.exe"])), "Codex SessionStart (startup), under pwsh.exe ← codex.exe");
        assert_eq!(hook_who("Antigravity", "SessionStart", &[], &[]), "Antigravity SessionStart");
        // An event's words are the agent's: nothing in them breaks her command line.
        let odd = json!({ "hook_event_name": "SessionStart", "source": "a \"b\" c:\\d\\\nx" });
        let who = ensure_who(&[], Some(&odd), Some("cli"), &[]);
        assert_eq!(who, "Claude Code SessionStart (a b c:dx, cli)");
        assert!(hook_who("x", &"y".repeat(1000), &[], &[]).chars().count() <= 300);
    }

    #[test]
    fn what_started_her_is_told() {
        let nobody = || -> Vec<String> { panic!("the programs above her asked for") };
        // A hook, an update, a restart: their own words.
        assert_eq!(starter(&args(&[BY_FLAG, "a session's hook, Codex SessionStart (startup)"]), nobody), "a session's hook, Codex SessionStart (startup)");
        assert_eq!(starter(&args(&["--after-update", "1234", BY_FLAG, "her update from v0.3.1 (pid 1234)"]), nobody), "her update from v0.3.1 (pid 1234)");
        // An older version's update or restart says nothing.
        assert_eq!(starter(&args(&["--after-update", "1234"]), nobody), "an update or a restart by an older version of hers (pid 1234)");
        // At login.
        assert_eq!(starter(&args(&["--wakuwaku-at-login", "task"]), nobody), "her logon task");
        assert_eq!(starter(&args(&["--wakuwaku-at-login", "run"]), nobody), "her Run value, at logon");
        // Started again out of a job, what started the first copy carried.
        assert_eq!(starter(&args(&["--wakuwaku-at-login", "task", BY_FLAG, "her logon task", "--wakuwaku-out-of-job"]), nobody), "her logon task, started again out of its job");
        // By hand, or by another program: the ones above her.
        assert_eq!(starter(&args(&[]), || names(&["explorer.exe"])), "hand, from Explorer");
        assert_eq!(starter(&args(&[]), || names(&["powershell.exe", "windowsterminal.exe"])), "a program: powershell.exe ← windowsterminal.exe");
        assert_eq!(starter(&args(&[]), Vec::new), "a program gone since");
        // The flag with no word after it: as if not there.
        assert_eq!(starter(&args(&[BY_FLAG]), || names(&["explorer.exe"])), "hand, from Explorer");
        assert_eq!(by_of(&args(&[BY_FLAG, "x"])), Some("x"));
        assert_eq!(by_of(&args(&["--wakuwaku-at-login", "run"])), None);
    }
}
