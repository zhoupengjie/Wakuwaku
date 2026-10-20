// Words for you from a script or an agent, said by her (POST /say):
//
//   POST /say { text, from?, secs?, private? }
//     text     what to say: up to TEXT_MAX characters, cut short past them
//     from     who says it ("Claude Code", "CI"): up to FROM_MAX characters
//     secs     how long it stays, SECS_MIN to SECS_MAX (SECS_DEFAULT)
//     private  true: kept off screen while 「显示具体内容」 is off (the
//              who alone shows), as a widget's own words are
//
// Out on the desktop she says it in a bubble over her head (pet.js say);
// at home, her home opens saying it, as a widget's nudge does (island.js
// onNudge). Not while she is hidden (do not disturb, a full-screen
// program) or a session wants you: those come first, and it is dropped.

use serde_json::Value;

pub const TEXT_MAX: usize = 200;
pub const FROM_MAX: usize = 40;
pub const SECS_MIN: u64 = 2;
pub const SECS_MAX: u64 = 120;
pub const SECS_DEFAULT: u64 = 10;

#[derive(Debug, PartialEq)]
pub struct Say {
    pub text: String,
    pub from: String,
    pub ms: u64,
    pub private: bool,
}

// One line, its spaces run together, cut to max characters (an ellipsis
// for the rest).
fn line(s: &str, max: usize) -> String {
    let flat = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max {
        return flat;
    }
    let mut cut: String = flat.chars().take(max - 1).collect();
    cut.push('…');
    cut
}

pub fn read(v: &Value) -> Result<Say, &'static str> {
    let text = line(v.get("text").and_then(Value::as_str).ok_or("text is missing")?, TEXT_MAX);
    if text.is_empty() {
        return Err("text is empty");
    }
    let from = match v.get("from") {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => line(s, FROM_MAX),
        Some(_) => return Err("from is not text"),
    };
    let secs = match v.get("secs") {
        None | Some(Value::Null) => SECS_DEFAULT,
        Some(n) => n.as_f64().filter(|n| n.is_finite()).ok_or("secs is not a number")?.round().clamp(SECS_MIN as f64, SECS_MAX as f64) as u64,
    };
    Ok(Say { text, from, ms: secs * 1000, private: v.get("private").and_then(Value::as_bool) == Some(true) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn words_for_you_as_sent() {
        let say = read(&json!({ "text": "测试跑完了，3 个失败", "from": "Claude Code", "secs": 20 })).unwrap();
        assert_eq!(say, Say { text: "测试跑完了，3 个失败".into(), from: "Claude Code".into(), ms: 20_000, private: false });
        // Only the words: no one named, the default time.
        assert_eq!(read(&json!({ "text": "hi" })).unwrap(), Say { text: "hi".into(), from: String::new(), ms: SECS_DEFAULT * 1000, private: false });
        assert!(read(&json!({ "text": "x", "private": true })).unwrap().private);
    }

    #[test]
    fn words_kept_to_one_short_line() {
        assert_eq!(read(&json!({ "text": "  two\n lines\tand  spaces " })).unwrap().text, "two lines and spaces");
        let long = read(&json!({ "text": "字".repeat(300), "from": "a".repeat(50) })).unwrap();
        assert_eq!(long.text.chars().count(), TEXT_MAX);
        assert!(long.text.ends_with('…'));
        assert_eq!(long.from.chars().count(), FROM_MAX);
        // Exactly as long as allowed: kept whole.
        assert_eq!(read(&json!({ "text": "a".repeat(TEXT_MAX) })).unwrap().text.len(), TEXT_MAX);
    }

    #[test]
    fn its_time_kept_in_bounds() {
        assert_eq!(read(&json!({ "text": "x", "secs": 0 })).unwrap().ms, SECS_MIN * 1000);
        assert_eq!(read(&json!({ "text": "x", "secs": 9999 })).unwrap().ms, SECS_MAX * 1000);
        assert_eq!(read(&json!({ "text": "x", "secs": 4.6 })).unwrap().ms, 5000);
    }

    #[test]
    fn nothing_to_say_is_turned_down() {
        assert_eq!(read(&json!({})), Err("text is missing"));
        assert_eq!(read(&json!({ "text": 3 })), Err("text is missing"));
        assert_eq!(read(&json!({ "text": " \n " })), Err("text is empty"));
        assert_eq!(read(&json!({ "text": "x", "from": 1 })), Err("from is not text"));
        assert_eq!(read(&json!({ "text": "x", "secs": "soon" })), Err("secs is not a number"));
    }
}
