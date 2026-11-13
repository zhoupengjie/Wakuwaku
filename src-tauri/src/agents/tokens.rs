// Today's tokens, from what Claude Code, Codex, Antigravity, oh-my-pi (omp),
// DeepSeek Harness and Reasonix write down as they go:
//   Claude Code  <config>/projects/**/*.jsonl: each reply's usage, counted
//                once per message id (a reply is written once per part)
//   Codex        <codex home>/sessions/**/*.jsonl: token_count events, each
//                the session's total so far; today's share is its last total
//                today less its last one before today
//   Antigravity  ~/.gemini/<product>/brain/<conversation>/.system_generated/
//                logs/transcript.jsonl (agy's antigravity-cli, the 2.0 app's
//                antigravity): a step a line, each answer of the model
//                (PLANNER_RESPONSE) with its input_tokens, cache_read_tokens
//                and output_tokens (its thinking in them) and created_at;
//                counted once per step. Undocumented, as agy 1.3.4 writes it
//                (2026-10-14); the transcript_full.jsonl beside it, the one
//                the hooks name, has the same steps with all their words.
//   DSH          <DSH home>/sessions/<project>/<session>/session.v4.jsonl.zstd
//                (agent-core dsh.rs session_log): JSONL in Zstandard frames,
//                one appended for each batch of lines; each answer of the
//                model (assistant/message) with its data.usage (inputTokens
//                without the cache's, cacheReadTokens, cacheWriteTokens,
//                outputTokens) and time (ms), counted once per message id.
//                Undocumented, as the desktop app 0.2.1 and 0.2.0-rc.2 write
//                it (2026-10-24): a session's sum is the tokenUsage totals
//                of DSH's own projection cache, to the token. Its titles'
//                calls are written down with no usage: not counted.
//   omp          <agent dir>/sessions/<project>/<time>_<id>.jsonl and a
//                subtask's <session>/<subtask>.jsonl below it (each profile's
//                agent_dir, connection::omp_agent_dirs): each assistant
//                message (type "message", message.role "assistant",
//                message.usage) and each internal call of omp's (type
//                "model_usage", usage), with input, output, cacheRead,
//                cacheWrite and totalTokens (their sum) and timestamp,
//                counted once per file and 8-hex-digit id (as omp 18.12.6
//                writes it).
//   Reasonix     <Reasonix home>/stats/<YYYY-MM-DD>.jsonl (Reasonix 2.x,
//                CLI 2.49 / Studio 2.44; connection::reasonix_home): already
//                split by local date; each model call appends { total,
//                prompt, completion, ... }, and a new turn adds { turn:
//                true, ... } with no total (skipped).
// Only files written to today are read, each from where the last reading
// stopped (and only whole lines, or whole frames). A new day starts from
// nothing.
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use serde_json::{json, Value};

// Never more than this much of one file in one reading.
const MAX_READ: u64 = 64 * 1024 * 1024;
// A Zstandard frame's first bytes, and the most its header may ask the
// decoder to hold (as agent-core's dsh.rs: DSH writes a few MB at most).
const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];
const MAX_ZSTD_WINDOW: u64 = 16 * 1024 * 1024;

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
    // Antigravity's steps counted, per transcript.
    agy_steps: HashMap<PathBuf, HashSet<u64>>,
    agy: Count,
    dsh_ids: HashSet<String>,
    dsh: Count,
    // omp's entries counted, per session or subtask file.
    omp_ids: HashMap<PathBuf, HashSet<String>>,
    omp: Count,
    reasonix: Count,
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

// The files under a folder whose names are a log's written to since a moment.
fn written_since(dir: &Path, since: u64, depth: u32, is_log: fn(&str) -> bool, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        // What the listing itself says of each entry (on Windows its kind
        // and times come with it): no further look at every file.
        let Ok(kind) = entry.file_type() else { continue };
        let path = entry.path();
        if kind.is_dir() {
            if depth > 0 {
                written_since(&path, since, depth - 1, is_log, out);
            }
        } else if entry.file_name().to_str().is_some_and(is_log) {
            let modified = entry.metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as u64);
            if modified >= since {
                out.push(path);
            }
        }
    }
}

fn is_jsonl(name: &str) -> bool {
    name.ends_with(".jsonl")
}

// A DSH session's log, any generation's, compressed or not (not a .tmp
// being written in its place).
fn is_dsh_log(name: &str) -> bool {
    name.starts_with("session.v") && (name.ends_with(".jsonl.zstd") || name.ends_with(".jsonl"))
}

// Antigravity's transcripts written to since a moment: one a conversation,
// in each brain folder (the steps without their words, transcript.jsonl).
fn agy_written_since(brain: &Path, since: u64, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(brain) else { return };
    for entry in entries.flatten() {
        let path = entry.path().join(".system_generated").join("logs").join("transcript.jsonl");
        let modified = std::fs::metadata(&path).ok().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(UNIX_EPOCH).ok());
        if modified.is_some_and(|d| d.as_millis() as u64 >= since) {
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

// The whole Zstandard frames appended to a file since the last reading, as
// their text. A frame still being written (or anything not a frame) ends the
// reading there, to be read whole the next time; where the last one stopped
// no longer starting a frame, the file was written anew: read from its start.
fn new_frames(path: &Path, from: u64) -> Option<(String, u64)> {
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let mut from = if from > len { 0 } else { from };
    let mut bytes = Vec::new();
    loop {
        file.seek(SeekFrom::Start(from)).ok()?;
        bytes.clear();
        (&mut file).take(MAX_READ).read_to_end(&mut bytes).ok()?;
        if from == 0 || bytes.len() < ZSTD_MAGIC.len() || bytes.starts_with(&ZSTD_MAGIC) {
            break;
        }
        from = 0;
    }
    let mut text = Vec::new();
    let mut at = 0;
    while bytes[at..].starts_with(&ZSTD_MAGIC) {
        let mut source = &bytes[at..];
        let mut frame = Vec::new();
        {
            let Ok(decoder) = ruzstd::decoding::StreamingDecoder::new_with_max_window_size(&mut source, MAX_ZSTD_WINDOW) else { break };
            if decoder.take(MAX_READ + 1).read_to_end(&mut frame).is_err() || frame.len() as u64 > MAX_READ {
                break;
            }
        }
        at = bytes.len() - source.len();
        text.extend(frame);
    }
    Some((String::from_utf8_lossy(&text).into_owned(), from + at as u64))
}

// Where the agents keep their logs on this computer.
#[derive(Clone, Default, Debug)]
pub struct AgentDirs {
    pub claude: PathBuf,
    pub codex: PathBuf,
    pub agy: Vec<PathBuf>,
    pub dsh: PathBuf,
    pub omp: Vec<PathBuf>,
    pub reasonix: PathBuf,
}

impl AgentDirs {
    pub fn discover() -> Self {
        Self {
            claude: claude_dir(),
            codex: codex_dir(),
            agy: agy_dirs(),
            dsh: dsh_dir(),
            omp: omp_dirs(),
            reasonix: reasonix_dir(),
        }
    }
}

impl Tokens {
    // Read what is new. day_start: local midnight (ms since 1970); today:
    // local date ("YYYY-MM-DD", for Reasonix's daily stats file).
    pub fn update(&mut self, dirs: &AgentDirs, day_start: u64, today: &str) {
        if day_start != self.day_start {
            *self = Tokens { day_start, ..Tokens::default() };
        }
        let mut files = Vec::new();
        written_since(&dirs.claude, day_start, 4, is_jsonl, &mut files);
        let claude_count = files.len();
        written_since(&dirs.codex, day_start, 5, is_jsonl, &mut files);
        let codex_count = files.len();
        for brain in &dirs.agy {
            agy_written_since(brain, day_start, &mut files);
        }
        let agy_count = files.len();
        for sessions in &dirs.omp {
            written_since(sessions, day_start, 3, is_jsonl, &mut files);
        }
        let omp_count = files.len();
        if !today.is_empty() {
            files.push(dirs.reasonix.join(format!("{today}.jsonl")));
        }
        for (i, path) in files.into_iter().enumerate() {
            let from = self.read_to.get(&path).copied().unwrap_or(0);
            let Some((text, to)) = new_lines(&path, from) else { continue };
            self.read_to.insert(path.clone(), to);
            for line in text.lines() {
                if i < claude_count {
                    self.claude_line(line);
                } else if i < codex_count {
                    self.codex_line(&path, line);
                } else if i < agy_count {
                    self.agy_line(&path, line);
                } else if i < omp_count {
                    self.omp_line(&path, line);
                } else {
                    self.reasonix_line(line);
                }
            }
        }
        let mut logs = Vec::new();
        written_since(&dirs.dsh, day_start, 2, is_dsh_log, &mut logs);
        for path in logs {
            let from = self.read_to.get(&path).copied().unwrap_or(0);
            let read = if path.extension().is_some_and(|e| e == "zstd") { new_frames(&path, from) } else { new_lines(&path, from) };
            let Some((text, to)) = read else { continue };
            self.read_to.insert(path, to);
            for line in text.lines() {
                self.dsh_line(line);
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

    // A step of Antigravity's: an answer of the model's, with what it used.
    // Its input taken to leave out what was read from the cache, as Claude
    // Code's does, the names being the same (every step seen so far read
    // nothing from it).
    fn agy_line(&mut self, path: &Path, line: &str) {
        if !line.contains("\"output_tokens\"") {
            return;
        }
        let Ok(step) = serde_json::from_str::<Value>(line) else { return };
        let at = step["created_at"].as_str().and_then(parse_time).unwrap_or(0);
        let (Some(index), Some(output)) = (step["step_index"].as_u64(), step["output_tokens"].as_u64()) else { return };
        if at < self.day_start || !self.agy_steps.entry(path.to_path_buf()).or_default().insert(index) {
            return;
        }
        let n = |k: &str| step[k].as_u64().unwrap_or(0);
        self.agy.total += n("input_tokens") + n("cache_read_tokens") + output;
        self.agy.output += output;
    }

    // An answer of DSH's model, with what it used, once per message (a
    // session forked or written anew may say it again).
    fn dsh_line(&mut self, line: &str) {
        if !line.contains("\"usage\"") {
            return;
        }
        let Ok(e) = serde_json::from_str::<Value>(line) else { return };
        let at = e["time"].as_u64().unwrap_or(0);
        let usage = &e["data"]["usage"];
        let id = e["data"]["message"]["id"].as_str().unwrap_or("");
        if e["type"] != "assistant/message" || at < self.day_start || !usage.is_object() || id.is_empty() || !self.dsh_ids.insert(id.to_string()) {
            return;
        }
        let n = |k: &str| usage[k].as_u64().unwrap_or(0);
        let output = n("outputTokens");
        self.dsh.total += n("inputTokens") + n("cacheReadTokens") + n("cacheWriteTokens") + output;
        self.dsh.output += output;
    }

    // An assistant reply or internal model_usage call of omp's, counted once
    // per file and id.
    fn omp_line(&mut self, path: &Path, line: &str) {
        if !line.contains("\"usage\"") {
            return;
        }
        let Ok(e) = serde_json::from_str::<Value>(line) else { return };
        let Some(at) = e["timestamp"].as_str().and_then(parse_time) else { return };
        let usage = match e["type"].as_str() {
            Some("message") if e["message"]["role"] == "assistant" => &e["message"]["usage"],
            Some("model_usage") => &e["usage"],
            _ => return,
        };
        let id = e["id"].as_str().or_else(|| e["message"]["id"].as_str()).unwrap_or("");
        if at < self.day_start || !usage.is_object() || id.is_empty() || !self.omp_ids.entry(path.to_path_buf()).or_default().insert(id.to_string()) {
            return;
        }
        let n = |k: &str| usage[k].as_u64().unwrap_or(0);
        let output = n("output");
        self.omp.total += n("input") + n("cacheRead") + n("cacheWrite") + output;
        self.omp.output += output;
    }

    // A model call in Reasonix 2.x's daily stats file (a new turn's marker
    // line has turn: true and no total).
    fn reasonix_line(&mut self, line: &str) {
        if !line.contains("\"total\"") {
            return;
        }
        let Ok(e) = serde_json::from_str::<Value>(line) else { return };
        if e["turn"].as_bool() == Some(true) {
            return;
        }
        let Some(total) = e["total"].as_u64() else { return };
        self.reasonix.total += total;
        self.reasonix.output += e["completion"].as_u64().unwrap_or(0);
    }

    pub fn count(&self) -> Count {
        let mut sum = self.claude;
        sum.total += self.agy.total + self.dsh.total + self.omp.total + self.reasonix.total;
        sum.output += self.agy.output + self.dsh.output + self.omp.output + self.reasonix.output;
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

// Antigravity's, a brain folder for each of its products: agy's, the 2.0
// app's, and (by its docs, which name the folders) the IDE's.
pub fn agy_dirs() -> Vec<PathBuf> {
    let gemini = crate::agents::connection::gemini_dir();
    ["antigravity-cli", "antigravity", "antigravity-ide"].iter().map(|product| gemini.join(product).join("brain")).collect()
}

// DeepSeek Harness's, its sessions' logs by project.
pub fn dsh_dir() -> PathBuf {
    crate::agents::connection::dsh_home().join("sessions")
}

// oh-my-pi's sessions folder in each profile's agent folder.
pub fn omp_dirs() -> Vec<PathBuf> {
    crate::agents::connection::omp_agent_dirs().into_iter().map(|d| d.join("sessions")).collect()
}

// Reasonix 2.x's daily stats folder.
pub fn reasonix_dir() -> PathBuf {
    crate::agents::connection::reasonix_home().join("stats")
}

pub const EVERY: Duration = Duration::from_secs(60);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_and_short_numbers() {
        assert_eq!(parse_time("1970-01-01T00:00:01.250Z"), Some(1250));
        assert_eq!(parse_time("2026-10-09T07:25:43.995Z"), Some(1_791_530_743_995));
        // Antigravity's, to the second.
        assert_eq!(parse_time("2026-10-09T07:25:43Z"), Some(1_791_530_743_000));
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
        // Antigravity's: a conversation's steps, the model's answers with
        // what they used; the same steps again beside them, not counted.
        let brains = [dir.join("agy").join("brain")];
        let logs = brains[0].join("5d94c80b").join(".system_generated").join("logs");
        std::fs::create_dir_all(logs.join("chunks").join("transcript")).unwrap();
        let answer = |i: u64, at: &str, input: u64, out: u64| format!(r#"{{"step_index":{i},"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE","created_at":"{at}","input_tokens":{input},"cache_read_tokens":200,"output_tokens":{out},"content":"ok"}}"#);
        let steps = [
            r#"{"step_index":0,"source":"USER_EXPLICIT","type":"USER_INPUT","status":"DONE","created_at":"2026-10-08T23:59:00Z","content":"say \"output_tokens\""}"#.to_string(),
            answer(1, "2026-10-08T23:59:30Z", 1000, 10), // yesterday
            answer(2, "2026-10-09T08:00:00Z", 1100, 20),
        ];
        for at in [logs.join("transcript.jsonl"), logs.join("transcript_full.jsonl"), logs.join("chunks").join("transcript").join("00000000.jsonl")] {
            std::fs::write(at, steps.join("\n") + "\n").unwrap();
        }

        let mut t = Tokens::default();
        let dirs = AgentDirs {
            claude: dir.join("claude"),
            codex: dir.join("codex"),
            agy: brains.to_vec(),
            dsh: dir.join("dsh"),
            omp: vec![],
            reasonix: dir.join("none"),
        };
        t.update(&dirs, day, "2026-10-09");
        assert_eq!(t.count(), Count { total: 132 + 500 + 1320, output: 20 + 30 + 20 });
        // More is written: only the new lines are read; a half-written one waits.
        let mut f = std::fs::OpenOptions::new().append(true).open(claude.join("s.jsonl")).unwrap();
        std::io::Write::write_all(&mut f, (reply("c", "2026-10-09T10:00:00.000Z", 5) + "\n{\"type\":\"assist").as_bytes()).unwrap();
        let mut f = std::fs::OpenOptions::new().append(true).open(logs.join("transcript.jsonl")).unwrap();
        std::io::Write::write_all(&mut f, (answer(3, "2026-10-09T08:01:00Z", 50, 5) + "\n").as_bytes()).unwrap();
        t.update(&dirs, day, "2026-10-09");
        assert_eq!(t.count(), Count { total: 632 + 117 + 1320 + 255, output: 55 + 25 });
        // Written anew, shorter: read again from its start, each step still once.
        std::fs::write(logs.join("transcript.jsonl"), steps.join("\n") + "\n").unwrap();
        t.update(&dirs, day, "2026-10-09");
        assert_eq!(t.count(), Count { total: 632 + 117 + 1320 + 255, output: 55 + 25 });
        // A new day: nothing yet.
        t.update(&dirs, day + 86_400_000, "2026-10-10");
        assert_eq!(t.count(), Count::default());
        assert_eq!(words(Count { total: 1_234_567, output: 85_000 })["vars"], json!({ "total": "1.2M", "output": "85K" }));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dsh_s_answers_are_read_by_whole_frames_once_each() {
        let dir = std::env::temp_dir().join(format!("wakuwaku-tokens-dsh-{}", std::process::id()));
        let sessions = dir.join("sessions");
        let (one, fork) = (sessions.join("--C-waku--").join("session-1"), sessions.join("--C-waku--").join("session-2"));
        std::fs::create_dir_all(&one).unwrap();
        std::fs::create_dir_all(&fork).unwrap();
        let day = parse_time("2026-10-09T00:00:00.000Z").unwrap();
        // An answer of the model's: 300 read from the cache, 4 written to it.
        let said = |id: &str, at: u64, input: u64, out: u64| format!(r#"{{"type":"assistant/message","seq":1,"time":{at},"data":{{"turn":1,"step":1,"message":{{"role":"assistant","content":[],"id":"{id}"}},"usage":{{"inputTokens":{input},"outputTokens":{out},"cacheReadTokens":300,"cacheWriteTokens":4,"totalTokens":0}}}},"surfaceOp":"append"}}"#);
        let frame = |lines: &[String]| ruzstd::encoding::compress_to_vec((lines.join("\n") + "\n").as_bytes(), ruzstd::encoding::CompressionLevel::Fastest);
        let header = r#"{"type":"session","version":4,"id":"session-1","createdAt":0}"#.to_string();
        let asked = format!(r#"{{"type":"user/message","seq":2,"time":{},"data":{{"text":"say \"usage\""}}}}"#, day + 500);
        let log = [frame(&[header.clone(), said("m1", day - 1000, 1000, 10)]), frame(&[asked, said("m2", day + 1000, 100, 30)])].concat();
        let at = one.join("session.v4.jsonl.zstd");
        std::fs::write(&at, &log).unwrap();
        std::fs::write(one.join("session.v4.jsonl.zstd.tmp"), &log).unwrap();
        // A fork's, not compressed: its parent's answer again, and its own.
        std::fs::write(fork.join("session.v4.jsonl"), [said("m2", day + 1000, 100, 30), said("f1", day + 1500, 1, 1)].join("\n") + "\n").unwrap();

        let none = dir.join("none");
        let dirs = AgentDirs {
            claude: none.clone(),
            codex: none.clone(),
            agy: vec![],
            dsh: sessions.clone(),
            omp: vec![],
            reasonix: none.clone(),
        };
        let mut t = Tokens::default();
        let mut read = || {
            t.update(&dirs, day, "2026-10-09");
            t.count()
        };
        assert_eq!(read(), Count { total: 434 + 306, output: 30 + 1 });
        // A frame appended, and half of another: the whole one read alone.
        let append = |bytes: &[u8]| std::io::Write::write_all(&mut std::fs::OpenOptions::new().append(true).open(&at).unwrap(), bytes).unwrap();
        let m4 = frame(&[said("m4", day + 3000, 20, 2)]);
        let (head, tail) = m4.split_at(m4.len() / 2);
        append(&[frame(&[said("m3", day + 2000, 50, 6)]), head.to_vec()].concat());
        assert_eq!(read(), Count { total: 740 + 360, output: 31 + 6 });
        append(tail);
        assert_eq!(read(), Count { total: 1100 + 326, output: 37 + 2 });
        // Written anew as one frame: read from its start, each answer still once.
        let lines: Vec<String> = [("m1", day - 1000), ("m2", day + 1000), ("m3", day + 2000), ("m4", day + 3000), ("m5", day + 4000)].iter().map(|(id, at)| said(id, *at, 1, 1)).collect();
        std::fs::write(&at, frame(&[[header].as_slice(), &lines].concat())).unwrap();
        assert_eq!(read(), Count { total: 1426 + 306, output: 39 + 1 });
        // Where the last reading stopped is no frame's start, or past the end:
        // from its start.
        let len = std::fs::metadata(&at).unwrap().len();
        for from in [1, len + 10] {
            let (text, to) = new_frames(&at, from).unwrap();
            assert_eq!((text.lines().count(), to), (6, len));
        }
        // A new day: nothing yet.
        t.update(&dirs, day + 86_400_000, "2026-10-10");
        assert_eq!(t.count(), Count::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn omp_and_reasonix_tokens_are_counted_once_with_subtasks_and_turns() {
        let dir = std::env::temp_dir().join(format!("wakuwaku-tokens-omp-rx-{}", std::process::id()));
        let omp_sessions = dir.join("omp").join("sessions");
        let proj = omp_sessions.join("--C-waku--");
        let sub_dir = proj.join("2026-10-09T08-00-00-000Z_a1b2c3d4");
        let rx_stats = dir.join("reasonix").join("stats");
        std::fs::create_dir_all(&sub_dir).unwrap();
        std::fs::create_dir_all(&rx_stats).unwrap();

        let day = parse_time("2026-10-09T00:00:00.000Z").unwrap();
        let omp_msg = |id: &str, at: &str, input: u64, out: u64, read: u64, write: u64| {
            let total = input + out + read + write;
            format!(r#"{{"type":"message","id":"{id}","parentId":"00000000","timestamp":"{at}","message":{{"role":"assistant","content":[],"model":"claude-sonnet-4-5","usage":{{"input":{input},"output":{out},"cacheRead":{read},"cacheWrite":{write},"totalTokens":{total}}},"stopReason":"end_turn"}}}}"#)
        };
        let omp_model = |id: &str, at: &str, input: u64, out: u64| {
            let total = input + out;
            format!(r#"{{"type":"model_usage","id":"{id}","parentId":"11223344","timestamp":"{at}","model":"claude-haiku-4-5","usage":{{"input":{input},"output":{out},"cacheRead":0,"cacheWrite":0,"totalTokens":{total}}}}}"#)
        };

        // Main session file: yesterday's answer, a user line, a timeless line,
        // today's answer (repeated with the same 8-hex-digit id), and an
        // internal model_usage call.
        let main_file = proj.join("2026-10-09T08-00-00-000Z_a1b2c3d4.jsonl");
        let main_lines = [
            r#"{"type":"session","version":3,"id":"a1b2c3d4","timestamp":"2026-10-08T23:59:00.000Z","cwd":"C:\\waku"}"#.to_string(),
            omp_msg("00aa11bb", "2026-10-08T23:59:59.000Z", 500, 50, 0, 0), // yesterday
            r#"{"type":"message","id":"01a2b3c4","parentId":null,"timestamp":"2026-10-09T08:00:00.000Z","message":{"role":"user","content":[{"type":"text","text":"say \"usage\""}]}}"#.to_string(),
            r#"{"type":"model_usage","id":"deadbeef","usage":{"input":999,"output":99,"cacheRead":0,"cacheWrite":0,"totalTokens":1098}}"#.to_string(), // no timestamp
            omp_msg("11223344", "2026-10-09T08:00:05.120Z", 200, 50, 1200, 80), // 1530 (out 50)
            omp_msg("11223344", "2026-10-09T08:00:05.120Z", 200, 50, 1200, 80), // same file + id, skipped
            omp_model("55667788", "2026-10-09T08:00:09.450Z", 1800, 40),       // 1840 (out 40)
        ];
        std::fs::write(&main_file, main_lines.join("\n") + "\n").unwrap();

        // Subtask file in the next directory level: may reuse the same 8-hex
        // id ("11223344") as the main file since deduplication is per file + id.
        let sub_file = sub_dir.join("subtask-explore.jsonl");
        std::fs::write(&sub_file, omp_msg("11223344", "2026-10-09T08:01:00.000Z", 300, 30, 100, 0) + "\n").unwrap(); // 430 (out 30)

        // Reasonix 2.x: yesterday's daily file (ignored) and today's file with
        // turn: true markers and model call totals.
        std::fs::write(rx_stats.join("2026-10-08.jsonl"), r#"{"ts":1791446405000,"model":"deepseek-reasoner","prompt":8000,"completion":1000,"total":9000}"#.to_string() + "\n").unwrap();
        let rx_today = rx_stats.join("2026-10-09.jsonl");
        let rx_lines = [
            r#"{"turn":true,"ts":1791532800000,"model":"deepseek-reasoner"}"#,
            r#"{"ts":1791532805000,"model":"deepseek-reasoner","prompt":1400,"completion":250,"total":1650}"#,
            r#"{"turn":true,"ts":1791532810000,"model":"deepseek-reasoner"}"#,
        ];
        std::fs::write(&rx_today, rx_lines.join("\n") + "\n").unwrap();

        let none = dir.join("none");
        let dirs = AgentDirs {
            claude: none.clone(),
            codex: none.clone(),
            agy: vec![],
            dsh: none,
            omp: vec![omp_sessions],
            reasonix: rx_stats,
        };
        let mut t = Tokens::default();
        t.update(&dirs, day, "2026-10-09");
        // omp: 1530 + 1840 + 430 = 3800 (out: 50 + 40 + 30 = 120)
        // reasonix: 1650 (out: 250)
        assert_eq!(t.count(), Count { total: 3800 + 1650, output: 120 + 250 });

        // Incremental append + half-written line waiting:
        let append = |path: &Path, s: &str| {
            std::io::Write::write_all(&mut std::fs::OpenOptions::new().append(true).open(path).unwrap(), s.as_bytes()).unwrap();
        };
        append(&main_file, &(omp_model("99aabbcc", "2026-10-09T09:00:00.000Z", 90, 10) + "\n"));
        append(&rx_today, "{\"ts\":1791532820000,\"prompt\":300,\"completion\":50,\"total\":350}\n{\"ts\":1791532830000,\"total\":999");
        t.update(&dirs, day, "2026-10-09");
        assert_eq!(t.count(), Count { total: 5450 + 100 + 350, output: 370 + 10 + 50 });

        // Complete the half-written Reasonix line:
        append(&rx_today, ",\"prompt\":899,\"completion\":100}\n");
        t.update(&dirs, day, "2026-10-09");
        assert_eq!(t.count(), Count { total: 5900 + 999, output: 430 + 100 });

        // Written anew, shorter: read again from its start, each entry still once.
        std::fs::write(&main_file, main_lines.join("\n") + "\n").unwrap();
        t.update(&dirs, day, "2026-10-09");
        assert_eq!(t.count(), Count { total: 5900 + 999, output: 430 + 100 });

        // A new day starts from nothing:
        t.update(&dirs, day + 86_400_000, "2026-10-10");
        assert_eq!(t.count(), Count::default());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
