// A deadline, as the old countdown.ps1 had it: how long until it;
// the island opens an hour before it, and when it comes. Looked at each
// minute; ten minutes past it, it goes.
//
// The deadline as typed is read each minute, as [datetime]::Parse read it
// (a time alone is today's, so "18:00" comes back each day): the ways
// people write one, here the ones .NET took on a Chinese Windows:
//   2026-12-31 18:00, 2026/12/31 18:00:30, 2026.12.31, 2026-1-5 8:05,
//   2026-12-31T18:00, 2026年12月31日 18:00, 2026年12月31日 18时, 12月31日,
//   12-31, 12/31, 12/31/2026, 18:00, 下午6:00, 6:00 PM
// It is a time on the local calendar, as the clock on the wall has it.
use std::collections::HashSet;
use std::time::Duration;

use serde_json::json;

use super::{Job, Now, Out, Wait};

const TTL: u64 = 180;
const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;

pub struct Countdown {
    at: String,
    title: String,
    lang: &'static str,
    // What the island has said of each deadline: "<at> <title> hour", "… now".
    told: HashSet<String>,
}

impl Countdown {
    // No title given: 截止, as the script's own default.
    pub fn new(at: &str, title: &str, lang: &'static str) -> Countdown {
        let title = if title.trim().is_empty() { "截止" } else { title.trim() };
        Countdown { at: at.trim().into(), title: title.into(), lang, told: HashSet::new() }
    }
}

// Days since 1970-01-01 of a date on the proleptic Gregorian calendar
// (Howard Hinnant's days_from_civil).
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

// The date of a day since 1970 (its civil_from_days): year, month, day.
pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn days_in(y: i64, m: i64) -> i64 {
    match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

// A date and time read: ms on the local calendar since 1970, or None.
// `today`: the local calendar's ms now (for a time alone, a date without
// its year).
pub fn parse(text: &str, today: i64) -> Option<i64> {
    let (ty, _, _) = civil_from_days(today.div_euclid(DAY));
    let text = text.trim();
    // The time: "18:00", "18:00:30", "18:00:00.5", "18时"; with 上午/下午 before
    // it or AM/PM after.
    let time_of = |t: &str| -> Option<i64> {
        let t = t.trim();
        let (pm, t) = match t.strip_prefix("下午").or_else(|| t.strip_prefix("晚上")) {
            Some(rest) => (Some(true), rest.trim()),
            None => match t.strip_prefix("上午").or_else(|| t.strip_prefix("早上")) {
                Some(rest) => (Some(false), rest.trim()),
                None => (None, t),
            },
        };
        let lower = t.to_ascii_lowercase();
        let (pm, t) = match (pm, lower.strip_suffix("pm").or_else(|| lower.strip_suffix("am"))) {
            (None, Some(rest)) => (Some(lower.ends_with("pm")), rest.trim().to_string()),
            (pm, _) => (pm, t.to_string()),
        };
        let (h, m, s, ms) = if let Some(h) = t.strip_suffix('时').or_else(|| t.strip_suffix('点')) {
            (h.trim().parse::<i64>().ok()?, 0, 0, 0)
        } else {
            let mut parts = t.split(':');
            let h = parts.next()?.trim().parse::<i64>().ok()?;
            let m = parts.next()?.trim();
            let m = (m.len() == 2).then(|| m.parse::<i64>().ok()).flatten()?;
            let (s, ms) = match parts.next() {
                None => (0, 0),
                Some(s) => {
                    let (s, frac) = s.split_once('.').unwrap_or((s, ""));
                    let s = (s.len() == 2).then(|| s.parse::<i64>().ok()).flatten()?;
                    let frac: String = frac.chars().take(3).collect();
                    let ms = if frac.is_empty() { 0 } else { format!("{frac:0<3}").parse::<i64>().ok()? };
                    (s, ms)
                }
            };
            if parts.next().is_some() {
                return None;
            }
            (h, m, s, ms)
        };
        let h = match pm {
            Some(true) if (1..=11).contains(&h) => h + 12,
            Some(false) if h == 12 => 0,
            Some(_) if !(1..=12).contains(&h) => return None,
            _ => h,
        };
        ((0..24).contains(&h) && (0..60).contains(&m) && (0..60).contains(&s)).then_some(h * HOUR + m * MINUTE + s * 1000 + ms)
    };
    let date_of = |y: i64, m: i64, d: i64| -> Option<i64> { ((1..=12).contains(&m) && d >= 1 && d <= days_in(y, m) && (1..=9999).contains(&y)).then(|| days_from_civil(y, m, d) * DAY) };
    // A time alone: today's.
    if !text.chars().next()?.is_ascii_digit() || text.contains(':') && !text.contains(['-', '/', '.', '年', '月']) {
        return time_of(text).map(|t| today.div_euclid(DAY) * DAY + t);
    }
    // The date's part, the time's after it (after a T, a space, or 日).
    let (date, time) = if let Some((d, t)) = text.split_once('日') {
        (d.to_string() + "日", t)
    } else if let Some(i) = text.find(['T', ' ']) {
        (text[..i].to_string(), &text[i + 1..])
    } else {
        (text.to_string(), "")
    };
    let time = if time.trim().is_empty() { 0 } else { time_of(time)? };
    let n = |s: &str| -> Option<i64> { (!s.is_empty() && s.len() <= 4 && s.chars().all(|c| c.is_ascii_digit())).then(|| s.parse().ok()).flatten() };
    let day = if date.contains('月') {
        // 2026年12月31日, 12月31日
        let (y, rest) = match date.split_once('年') {
            Some((y, rest)) => (n(y.trim())?, rest),
            None => (ty, date.as_str()),
        };
        let (m, d) = rest.trim().split_once('月')?;
        date_of(y, n(m.trim())?, n(d.trim().trim_end_matches('日').trim())?)?
    } else {
        let parts: Vec<&str> = date.split(['-', '/', '.']).collect();
        match parts.as_slice() {
            // 2026-12-31; 12/31/2026; 26-12-31
            [a, b, c] if a.len() == 4 => date_of(n(a)?, n(b)?, n(c)?)?,
            [a, b, c] if c.len() == 4 => date_of(n(c)?, n(a)?, n(b)?)?,
            [a, b, c] if a.len() <= 2 && b.len() <= 2 && c.len() <= 2 => date_of(2000 + n(a)?, n(b)?, n(c)?)?,
            // 12-31: this year's
            [m, d] if m.len() <= 2 && d.len() <= 2 => date_of(ty, n(m)?, n(d)?)?,
            _ => return None,
        }
    };
    Some(day + time)
}

// The local calendar's ms since 1970 now, as the clock on the wall reads.
pub fn local_ms() -> i64 {
    let (y, m, d, ms) = crate::widgets::local_now();
    days_from_civil(i64::from(y), i64::from(m), i64::from(d)) * DAY + ms as i64
}

// How long, in a few words (left > 0): days and hours, hours and minutes, or minutes.
fn left_words(left: i64) -> String {
    if left >= DAY {
        return format!("还有 {} 天 {} 小时", left / DAY, (left % DAY) / HOUR);
    }
    if left >= HOUR {
        return format!("还有 {}:{:02}", left / HOUR, (left % HOUR) / MINUTE);
    }
    format!("还有 {} 分钟", (left as f64 / MINUTE as f64).ceil() as i64)
}

impl Job for Countdown {
    fn turn(&mut self, now: Now, out: &mut dyn Out) -> Wait {
        let wait = Wait::every(Duration::from_secs(60));
        let Some(at) = parse(&self.at, now.local) else {
            out.say(if self.lang == "zh" { format!("看不懂这个时间：{}（写成 2026-12-31 18:00 这样）", self.at) } else { format!("Not a date and time: {} (write it as 2026-12-31 18:00)", self.at) });
            return wait;
        };
        // Ten minutes past it, it goes.
        if at <= now.local - 10 * MINUTE {
            out.post(json!({ "id": "countdown", "remove": true }));
            return wait;
        }
        let left = at - now.local;
        let key = format!("{at} {}", self.title);
        // Its one size (small): the date under it, and the days left as one
        // number (the hours and minutes under a day).
        let (_, month, day) = civil_from_days(at.div_euclid(DAY));
        let weekday = ["周四", "周五", "周六", "周日", "周一", "周二", "周三"][at.div_euclid(DAY).rem_euclid(7) as usize];
        let date = format!("{month} 月 {day} 日 {weekday}");
        let count = if left >= DAY {
            json!({ "kind": "days", "value": (left / DAY).to_string(), "unit": "天" })
        } else {
            json!({ "kind": "days", "value": if left <= 0 { "0:00".to_string() } else { format!("{}:{:02}", left / HOUR, (left % HOUR) / MINUTE) } })
        };
        let mut widget = json!({
            "id": "countdown",
            "icon": "flag",
            "color": if left < HOUR { "#ff5c6c" } else { "#ffb340" },
            "label": self.title,
            "value": if left <= 0 { "到点了".to_string() } else { left_words(left) },
            "detail": { "sizes": ["s"], "extra": date, "rows": [count] },
            "ttl": TTL,
        });
        if left <= 0 && !self.told.contains(&format!("{key} now")) {
            widget["nudge"] = json!(format!("到点了：{}", self.title));
            self.told.insert(format!("{key} now"));
            self.told.insert(format!("{key} hour"));
        } else if left <= HOUR && !self.told.contains(&format!("{key} hour")) {
            widget["nudge"] = json!(format!("{}：{}", left_words(left), self.title));
            self.told.insert(format!("{key} hour"));
        }
        out.post(widget);
        wait
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{golden, record_at, same};
    use super::*;

    fn at(y: i64, m: i64, d: i64, h: i64, min: i64) -> i64 {
        days_from_civil(y, m, d) * DAY + h * HOUR + min * MINUTE
    }

    // From 16:58:30 to past 18:10, each minute, as countdown.ps1 posted it:
    // an hour left (the island told), a minute, the time come (told), gone.
    #[test]
    fn a_deadline_reads_as_the_script_posted_it() {
        let (theirs, _) = golden("countdown");
        let mut c = Countdown::new(" 2026-10-20 18:00 ", "交周报", "zh");
        let start = at(2026, 10, 20, 16, 58) + 30_000;
        for (i, (t, theirs)) in theirs.iter().enumerate() {
            let (posts, said, wait) = record_at(&mut c, start + (*t * 1000.0) as i64);
            assert_eq!((posts.len(), said, wait), (1, None, Wait::every(Duration::from_secs(60))));
            assert!(same(&posts[0], theirs), "post {i} at {t} s:\n{}\n{theirs}", posts[0]);
        }
        let (theirs, _) = golden("countdown-days");
        let mut c = Countdown::new("2026-12-31", "", "zh");
        for (t, theirs) in &theirs {
            let (posts, _, _) = record_at(&mut c, at(2026, 10, 18, 9, 0) + (*t * 1000.0) as i64);
            assert!(same(&posts[0], theirs), "{}\n{theirs}", posts[0]);
        }
        // Not a time: said in her language (the script's was .NET's exception).
        let (posts, said, _) = record_at(&mut Countdown::new("xyz", "a", "zh"), start);
        assert!(posts.is_empty());
        assert_eq!(said.as_deref(), Some("看不懂这个时间：xyz（写成 2026-12-31 18:00 这样）"));
    }

    // What [datetime]::Parse made of each on a Chinese Windows (2026-10-18).
    #[test]
    fn deadlines_are_read_as_dotnet_read_them() {
        let today = at(2026, 10, 18, 9, 30);
        let ok = |s: &str, want: i64| assert_eq!(parse(s, today), Some(want), "{s}");
        ok("2026-12-31", at(2026, 12, 31, 0, 0));
        ok("2026/12/31 18:00", at(2026, 12, 31, 18, 0));
        ok("12/31/2026", at(2026, 12, 31, 0, 0));
        ok("18:00", at(2026, 10, 18, 18, 0));
        ok("8:05", at(2026, 10, 18, 8, 5));
        ok("2026-12-31T18:00:00", at(2026, 12, 31, 18, 0));
        ok("2026-12-31 18:00:30", at(2026, 12, 31, 18, 0) + 30_000);
        ok("2026.12.31", at(2026, 12, 31, 0, 0));
        ok("2026.12.31 18:00", at(2026, 12, 31, 18, 0));
        ok("2026-1-5 8:05", at(2026, 1, 5, 8, 5));
        ok("2026年12月31日", at(2026, 12, 31, 0, 0));
        ok("2026年12月31日 18:00", at(2026, 12, 31, 18, 0));
        ok("2026年12月31日 18时", at(2026, 12, 31, 18, 0));
        ok("12月31日", at(2026, 12, 31, 0, 0));
        ok("12-31", at(2026, 12, 31, 0, 0));
        ok("12/31", at(2026, 12, 31, 0, 0));
        ok("2026-12-31 6:00 PM", at(2026, 12, 31, 18, 0));
        ok("2026-12-31 下午6:00", at(2026, 12, 31, 18, 0));
        ok("  2026-12-31   18:00 ", at(2026, 12, 31, 18, 0));
        ok("2026-12-31 18:00:00.5", at(2026, 12, 31, 18, 0) + 500);
        ok("26-12-31", at(2026, 12, 31, 0, 0));
        ok("12-31-2026", at(2026, 12, 31, 0, 0));
        ok("2024-02-29", at(2024, 2, 29, 0, 0));
        for bad in ["31/12/2026", "2026-12-31 18", "2026-02-30", "2026-12-31 24:00", "2026-12-31 1800", "20261231", "tomorrow", "2026-12-31 18:00 交周报", "2026-12-31 18:00:61", "2026-13-01", "2026-12-31 18.00", "xyz", ""] {
            assert_eq!(parse(bad, today), None, "{bad}");
        }
    }

    #[test]
    fn days_and_dates() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2026, 10, 18), 20_744);
        assert_eq!(civil_from_days(20_744), (2026, 10, 18));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        assert_eq!((left_words(73 * DAY + 15 * HOUR), left_words(HOUR + 59 * MINUTE + 59_000), left_words(59 * MINUTE + 30_000), left_words(1)), ("还有 73 天 15 小时".into(), "还有 1:59".into(), "还有 60 分钟".into(), "还有 1 分钟".into()));
    }
}
