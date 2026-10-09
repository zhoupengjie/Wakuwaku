// Today's tokens, from what Claude Code and Codex write down as they go:
//   Claude Code  <config>/projects/**/*.jsonl: each reply's usage, counted
//                once per message id (a reply is written once per part)
//   Codex        <codex home>/sessions/**/*.jsonl: token_count events, each
//                the session's total so far; today's share is its last total
//                today less its last one before today
// Only files written to today are read, each from where the last reading
// stopped (and only whole lines). A new day starts from nothing.
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use serde_json::{json, Value};

// Never more than this much of one file in one reading.
const MAX_READ: u64 = 64 * 1024 * 1024;

#[derive(Default, Clone, Copy, PartialEq, Debug)]
pub struct Count {
    pub total: u64,
    pub output: u64,
}

#[derive(Default)]
struct Codex {
    before: Count,
    today: Option<Count>,
}

#[derive(Default)]
pub struct Tokens {
    // Local midnight, in ms since 1970.
    day_start: u64,
    read_to: HashMap<PathBuf, u64>,
    claude_ids: HashSet<String>,
    claude: Count,
    codex: HashMap<PathBuf, Codex>,
}

// "2026-10-09T07:25:43.995Z" in ms since 1970.
pub fn parse_time(s: &str) -> Option<u64> {
    let b = s.as_bytes();
    if b.len() < 19 || b[4] != b'-' || b[10] != b'T' {
        return None;
    }
    let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, m, d) = (n(0..4)?, n(5..7)?, n(8..10)?);
    let (hh, mm, ss) = (n(11..13)?, n(14..16)?, n(17..19)?);
    let ms = if b.get(19) == Some(&b'.') { s.get(20..23).and_then(|f| f.parse::<i64>().ok()).unwrap_or(0) } else { 0 };
    // Days from 1970-01-01 to y-m-d (the civil calendar).
    let (y, m) = if m <= 2 { (y - 1, m + 12) } else { (y, m) };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m - 3) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(((days * 24 + hh) * 60 + mm) * 60_000 + ss * 1000 + ms).ok()
}

fn modified_ms(path: &Path) -> u64 {
    path.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as u64)
}

// The .jsonl files under a folder written to since a moment.
fn written_since(dir: &Path, since: u64, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth > 0 {
                written_since(&path, since, depth - 1, out);
            }
        } else if path.extension().is_some_and(|e| e == "jsonl") && modified_ms(&path) >= since {
            out.push(path);
        }
    }
}

// The whole lines added to a file since the last reading.
fn new_lines(path: &Path, from: u64) -> Option<(String, u64)> {
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let from = if from > len { 0 } else { from };
    file.seek(SeekFrom::Start(from)).ok()?;
    let mut bytes = Vec::new();
    file.take(MAX_READ).read_to_end(&mut bytes).ok()?;
    let whole = bytes.iter().rposition(|&c| c == b'\n').map_or(0, |i| i + 1);
    bytes.truncate(whole);
    Some((String::from_utf8_lossy(&bytes).into_owned(), from + whole as u64))
}

impl Tokens {
    // Read what is new. day_start: local midnight (ms since 1970).
    pub fn update(&mut self, claude_dir: &Path, codex_dir: &Path, day_start: u64) {
        if day_start != self.day_start {
            *self = Tokens { day_start, ..Tokens::default() };
        }
        let mut files = Vec::new();
        written_since(claude_dir, day_start, 4, &mut files);
        let claude_count = files.len();
        written_since(codex_dir, day_start, 5, &mut files);
        for (i, path) in files.into_iter().enumerate() {
            let from = self.read_to.get(&path).copied().unwrap_or(0);
            let Some((text, to)) = new_lines(&path, from) else { continue };
            self.read_to.insert(path.clone(), to);
            for line in text.lines() {
                if i < claude_count {
                    self.claude_line(line);
                } else {
                    self.codex_line(&path, line);
                }
            }
        }
    }

    fn claude_line(&mut self, line: &str) {
        if !line.contains("\"usage\"") {
            return;
        }
        let Ok(e) = serde_json::from_str::<Value>(line) else { return };
        let at = e["timestamp"].as_str().and_then(parse_time).unwrap_or(0);
        let usage = &e["message"]["usage"];
        let id = e["message"]["id"].as_str().unwrap_or("");
        if e["type"] != "assistant" || at < self.day_start || !usage.is_object() || id.is_empty() || !self.claude_ids.insert(id.to_string()) {
            return;
        }
        let n = |k: &str| usage[k].as_u64().unwrap_or(0);
        let output = n("output_tokens");
        self.claude.total += n("input_tokens") + n("cache_creation_input_tokens") + n("cache_read_input_tokens") + output;
        self.claude.output += output;
    }

    fn codex_line(&mut self, path: &Path, line: &str) {
        if !line.contains("\"token_count\"") {
            return;
        }
        let Ok(e) = serde_json::from_str::<Value>(line) else { return };
        let usage = &e["payload"]["info"]["total_token_usage"];
        if e["payload"]["type"] != "token_count" || !usage.is_object() {
            return;
        }
        let at = e["timestamp"].as_str().and_then(parse_time).unwrap_or(0);
        let count = Count { total: usage["total_tokens"].as_u64().unwrap_or(0), output: usage["output_tokens"].as_u64().unwrap_or(0) };
        let session = self.codex.entry(path.to_path_buf()).or_default();
        if at < self.day_start {
            session.before = count;
        } else {
            session.today = Some(count);
        }
    }

    pub fn count(&self) -> Count {
        let mut sum = self.claude;
        for s in self.codex.values() {
            if let Some(today) = s.today {
                sum.total += today.total.saturating_sub(s.before.total);
                sum.output += today.output.saturating_sub(s.before.output);
            }
        }
        sum
    }
}

// 1234567 as 1.2M, 85000 as 85K.
pub fn short(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => format!("{}K", trim(n as f64 / 1e3)),
        1_000_000..=999_999_999 => format!("{}M", trim(n as f64 / 1e6)),
        _ => format!("{}B", trim(n as f64 / 1e9)),
    }
}

fn trim(x: f64) -> String {
    if x >= 100.0 { format!("{x:.0}") } else { format!("{x:.1}").trim_end_matches(".0").to_string() }
}

pub fn words(c: Count) -> Value {
    json!({ "key": "widget.tokensValue", "vars": { "total": short(c.total), "output": short(c.output) } })
}

// Where Claude Code and Codex keep their records.
pub fn claude_dir() -> PathBuf {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_default();
    std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from).unwrap_or_else(|| home.join(".claude")).join("projects")
}

pub fn codex_dir() -> PathBuf {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_default();
    std::env::var_os("CODEX_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".codex")).join("sessions")
}

pub const EVERY: Duration = Duration::from_secs(60);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_and_short_numbers() {
        assert_eq!(parse_time("1970-01-01T00:00:01.250Z"), Some(1250));
        assert_eq!(parse_time("2026-10-09T07:25:43.995Z"), Some(1_791_530_743_995));
        assert_eq!(parse_time("nope"), None);
        assert_eq!((short(999), short(85_000), short(1_234_567), short(250_000_000), short(3_400_000_000)), ("999".into(), "85K".into(), "1.2M".into(), "250M".into(), "3.4B".into()));
    }

    #[test]
    fn today_is_counted_once_per_reply_and_from_where_it_stopped() {
        let dir = std::env::temp_dir().join(format!("wakuwaku-tokens-{}", std::process::id()));
        let claude = dir.join("claude").join("proj");
        let codex = dir.join("codex").join("2026").join("10").join("09");
        std::fs::create_dir_all(&claude).unwrap();
        std::fs::create_dir_all(&codex).unwrap();
        let day = parse_time("2026-10-09T00:00:00.000Z").unwrap();
        let reply = |id: &str, at: &str, out: u64| format!(r#"{{"type":"assistant","timestamp":"{at}","message":{{"id":"{id}","usage":{{"input_tokens":2,"cache_creation_input_tokens":10,"cache_read_input_tokens":100,"output_tokens":{out}}}}}}}"#);
        let lines = [
            reply("a", "2026-10-08T23:59:59.000Z", 50), // yesterday
            reply("b", "2026-10-09T08:00:00.000Z", 20),
            reply("b", "2026-10-09T08:00:00.000Z", 20), // the same reply, written again
            r#"{"type":"user","message":{"content":"hi"}}"#.to_string(),
        ];
        std::fs::write(claude.join("s.jsonl"), lines.join("\n") + "\n").unwrap();
        let token = |at: &str, total: u64, out: u64| format!(r#"{{"timestamp":"{at}","type":"event_msg","payload":{{"type":"token_count","info":{{"total_token_usage":{{"total_tokens":{total},"output_tokens":{out}}}}}}}}}"#);
        std::fs::write(codex.join("rollout.jsonl"), [token("2026-10-08T22:00:00.000Z", 1000, 100), token("2026-10-09T09:00:00.000Z", 1500, 130)].join("\n") + "\n").unwrap();

        let mut t = Tokens::default();
        t.update(&dir.join("claude"), &dir.join("codex"), day);
        assert_eq!(t.count(), Count { total: 132 + 500, output: 20 + 30 });
        // More is written: only the new lines are read; a half-written one waits.
        let mut f = std::fs::OpenOptions::new().append(true).open(claude.join("s.jsonl")).unwrap();
        std::io::Write::write_all(&mut f, (reply("c", "2026-10-09T10:00:00.000Z", 5) + "\n{\"type\":\"assist").as_bytes()).unwrap();
        t.update(&dir.join("claude"), &dir.join("codex"), day);
        assert_eq!(t.count(), Count { total: 632 + 117, output: 55 });
        // A new day: nothing yet.
        t.update(&dir.join("claude"), &dir.join("codex"), day + 86_400_000);
        assert_eq!(t.count(), Count::default());
        assert_eq!(words(Count { total: 1_234_567, output: 85_000 })["vars"], json!({ "total": "1.2M", "output": "85K" }));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
