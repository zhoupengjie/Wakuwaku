// The calendar: today, with the lunar date under it (农历, the year's
// animal, its festivals and the solar terms, 节气: lunar.rs) or the week of
// the year; and, from calendars subscribed to by their address (an .ics
// feed: Google Calendar's secret address, Outlook's published calendar,
// Feishu's, iCloud's), what is coming in the next two weeks (ics.rs), the
// island opening ten minutes before each. Looked at each minute (the date,
// and the minutes to each event); the feeds read every half hour. The
// month's grid is the pages' own (wcards.ts, wdesk.ts), drawn from the
// date; the event rows say which days to mark on it.
//
// Settings: Url (one or more addresses, commas between; webcal:// is
// https://), Second (Lunar or Week: what its second line says; none
// chosen, Lunar in Chinese, Week in any other language).
//
// Its words are in her language.
mod ics;
mod lunar;

use std::collections::HashSet;
use std::time::Duration;

use serde_json::{json, Value};

use super::countdown::civil_from_days;
use super::web::{self, Answer};
use super::{Fetch, Job, Now, Out, Wait};

const COLOR: &str = "#9d8cff";
const MINUTE: i64 = 60_000;
const DAY: i64 = 24 * 60 * MINUTE;
// The feeds read every half hour; what is coming in the next two weeks,
// eight of them shown; the island opens ten minutes before one.
const FEED_EVERY_MS: u64 = 30 * 60 * 1000;
const AHEAD_DAYS: i64 = 14;
const EVENTS: usize = 8;
const REMIND_MS: i64 = 10 * MINUTE;
// Each minute while someone is about; with nobody, every three (a widget
// lasting past that).
const WAIT: Wait = Wait { busy: Duration::from_secs(60), idle: Duration::from_secs(180) };
const TTL: u64 = 300;

const WEEKDAYS_ZH: [&str; 7] = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];
const WEEKDAYS_EN: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTHS_EN: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

pub struct Calendar {
    urls: Vec<String>,
    lunar: bool,
    lang: &'static str,
    fetch: Fetch,
    // When the feeds were last read (ms), and the two weeks' events from them.
    read_at: Option<u64>,
    events: Vec<ics::Event>,
    // The events the island has told of (ten minutes before each).
    told: HashSet<(i64, String)>,
    // What went wrong reading the feeds, said each turn until they read.
    trouble: Option<String>,
}

impl Calendar {
    pub fn new(urls: &str, lunar: bool, lang: &'static str, fetch: Fetch) -> Calendar {
        let urls = urls
            .split([',', ';', '\n', ' '])
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .map(|u| match u.strip_prefix("webcal://") {
                Some(rest) => format!("https://{rest}"),
                None => u.to_string(),
            })
            .collect();
        Calendar { urls, lunar, lang, fetch, read_at: None, events: Vec::new(), told: HashSet::new(), trouble: None }
    }

    fn zh(&self) -> bool {
        self.lang == "zh"
    }

    // Every feed read, its events of the two weeks from today kept in
    // order; the first that fails is said, the rest still read.
    fn read_feeds(&mut self, now: Now) {
        let from = now.local.div_euclid(DAY) * DAY;
        let to = from + AHEAD_DAYS * DAY;
        let offset = offset_of(now);
        let mut events = Vec::new();
        let mut trouble = None;
        for url in &self.urls {
            match (self.fetch)(url, &[("user-agent", "wakuwaku"), ("accept", "text/calendar, */*")]) {
                Ok(Answer { status: 200..=299, body }) => events.extend(ics::events(&body, from, to, offset, 64)),
                Ok(page) => {
                    if trouble.is_none() {
                        trouble = Some(web::refused(&page));
                    }
                }
                Err(fail) => {
                    if trouble.is_none() {
                        trouble = Some(web::trouble(self.lang, &fail));
                    }
                }
            }
        }
        events.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| a.summary.cmp(&b.summary)));
        events.dedup();
        self.events = events;
        self.read_at = Some(now.ms);
        self.trouble = trouble;
    }

    // Which day an event's is, in a word: today, tomorrow, the weekday
    // within the week, else its date.
    fn day_words(&self, days: i64, today: i64) -> String {
        let (_, m, d) = civil_from_days(days);
        let wd = (days + 4).rem_euclid(7) as usize;
        match (days - today, self.zh()) {
            (i64::MIN..=0, true) => "今天".into(),
            (i64::MIN..=0, false) => "Today".into(),
            (1, true) => "明天".into(),
            (1, false) => "Tomorrow".into(),
            (2..=6, true) => WEEKDAYS_ZH[wd].into(),
            (2..=6, false) => WEEKDAYS_EN[wd].into(),
            (_, true) => format!("{m}月{d}日"),
            (_, false) => format!("{} {d}", MONTHS_EN[m as usize - 1]),
        }
    }

    // The widget: the date as its name, the day of the month its value,
    // the lunar date (or the week) and the day's marks its second line;
    // in rows, the date in parts and the events coming, each with its day
    // and times; and the island's word of one about to begin.
    fn widget(&mut self, now: Now) -> Value {
        let today = now.local.div_euclid(DAY);
        let (y, m, d) = civil_from_days(today);
        let wd = (today + 4).rem_euclid(7) as usize;
        let zh = self.zh();
        let label = if zh { format!("{m}月{d}日 {}", WEEKDAYS_ZH[wd]) } else { format!("{}, {} {d}", WEEKDAYS_EN[wd], MONTHS_EN[m as usize - 1]) };
        let mut marks: Vec<String> = Vec::new();
        let lunar = self.lunar.then(|| lunar::of(y, m, d)).flatten();
        if let Some(l) = &lunar {
            marks.extend(lunar::festival(l).map(str::to_string));
        }
        // The solar terms are the Chinese calendar's: in Chinese, whichever
        // the second line says.
        if zh {
            marks.extend(lunar::term(y, m, d).map(str::to_string));
        }
        marks.extend(lunar::solar_festival(m, d, self.lang).map(str::to_string));
        let (week, doy) = week_of(today);
        let second = match &lunar {
            Some(l) => format!("农历{}", lunar::words(l)),
            None if zh => format!("第 {week} 周 · 第 {doy} 天"),
            None => format!("Week {week} · Day {doy}"),
        };
        let mut line = vec![second.clone()];
        line.extend(marks.iter().cloned());
        let extra = line.join(" · ");
        let mut text = match &lunar {
            Some(l) => vec![lunar::year_words(l.year)],
            None => Vec::new(),
        };
        text.extend(line);
        let mut rows = vec![json!({ "kind": "date", "value": d.to_string(), "name": if zh { format!("{m}月") } else { MONTHS_EN[m as usize - 1].to_string() }, "unit": if zh { WEEKDAYS_ZH[wd] } else { WEEKDAYS_EN[wd] }, "text": text.join(" · ") })];
        let offset = offset_of(now);
        let mut nudge: Option<String> = None;
        let untitled = if zh { "（无标题）" } else { "(Untitled)" };
        let coming: Vec<ics::Event> = self.events.iter().filter(|e| e.end > now.local).take(EVENTS).cloned().collect();
        for e in &coming {
            let e_days = e.start.div_euclid(DAY);
            let summary = if e.summary.trim().is_empty() { untitled.to_string() } else { e.summary.trim().to_string() };
            let when = if e.all_day {
                if zh {
                    "全天".to_string()
                } else {
                    "All day".to_string()
                }
            } else {
                hhmm(e.start)
            };
            let unit = if !e.all_day && e.end > e.start && e.end.div_euclid(DAY) == e_days { hhmm(e.end) } else { String::new() };
            rows.push(json!({ "kind": "event", "when": when, "unit": unit, "name": self.day_words(e_days, today), "subject": summary, "text": e.location, "at": e.start - offset }));
            // Ten minutes before one with a time: the island says so, once.
            let left = e.start - now.local;
            if !e.all_day && left > 0 && left <= REMIND_MS && nudge.is_none() && self.told.insert((e.start, summary.clone())) {
                let minutes = (left as f64 / MINUTE as f64).ceil() as i64;
                nudge = Some(if zh { format!("{minutes} 分钟后：{summary}") } else { format!("In {minutes} min: {summary}") });
            }
        }
        // Told of, and past: forgotten, so the set never grows.
        self.told.retain(|(start, _)| *start > now.local - DAY);
        let mut widget = json!({ "id": "calendar", "icon": "calendar", "color": COLOR, "label": label, "value": d.to_string(), "ttl": TTL, "detail": { "sizes": ["s", "m", "l"], "extra": extra, "rows": rows } });
        if let Some(words) = nudge {
            widget["nudge"] = json!(words);
        }
        widget
    }
}

impl Job for Calendar {
    fn turn(&mut self, now: Now, out: &mut dyn Out) -> Wait {
        if !self.urls.is_empty() && self.read_at.is_none_or(|t| now.ms.saturating_sub(t) >= FEED_EVERY_MS) {
            self.read_feeds(now);
        }
        if let Some(trouble) = &self.trouble {
            out.say(trouble.clone());
        }
        out.post(self.widget(now));
        WAIT
    }
}

// The local calendar's offset from UTC now, to the minute (ms).
fn offset_of(now: Now) -> i64 {
    let off = now.local - now.ms as i64;
    (off as f64 / MINUTE as f64).round() as i64 * MINUTE
}

// A time of day on the local calendar, HH:MM.
fn hhmm(local: i64) -> String {
    let t = local.rem_euclid(DAY);
    format!("{:02}:{:02}", t / (60 * MINUTE), (t % (60 * MINUTE)) / MINUTE)
}

// The ISO week of the year a day (since 1970) is in, and which day of
// its year it is.
fn week_of(days: i64) -> (i64, i64) {
    // Monday 1 to Sunday 7 (1970-01-01 was a Thursday).
    let weekday = (days + 3).rem_euclid(7) + 1;
    let thursday = days - (weekday - 1) + 3;
    let (ty, _, _) = civil_from_days(thursday);
    let week = (thursday - super::countdown::days_from_civil(ty, 1, 1)) / 7 + 1;
    let (y, _, _) = civil_from_days(days);
    (week, days - super::countdown::days_from_civil(y, 1, 1) + 1)
}

#[cfg(test)]
mod tests {
    use super::super::tests::{record_at, served_with, Route};
    use super::*;

    const DAY_MS: i64 = 86_400_000;
    // 2026-11-07 (a Saturday) 09:30 on the local calendar.
    const NOV_7: i64 = 20_764 * DAY_MS + 9 * 3_600_000 + 30 * 60_000;

    fn offline() -> Fetch {
        served_with(vec![]).0
    }

    // Today in Chinese with the lunar date: 立冬 falls on it; in English
    // the week. The value is the day of the month; the date in parts in a
    // row; each minute.
    #[test]
    fn today_in_each_language() {
        let mut c = Calendar::new("", true, "zh", offline());
        let (posts, said, wait) = record_at(&mut c, NOV_7);
        assert_eq!((posts.len(), said, wait), (1, None, WAIT));
        let w = &posts[0];
        assert_eq!((w["label"].as_str(), w["value"].as_str(), w["ttl"].as_u64()), (Some("11月7日 周六"), Some("7"), Some(300)));
        assert_eq!(w["detail"]["extra"], "农历九月廿九 · 立冬");
        assert_eq!(w["detail"]["rows"][0], json!({ "kind": "date", "value": "7", "name": "11月", "unit": "周六", "text": "丙午马年 · 农历九月廿九 · 立冬" }));
        assert_eq!(w["detail"]["sizes"], json!(["s", "m", "l"]));
        assert!(w.get("nudge").is_none());
        let mut c = Calendar::new("", false, "zh", offline());
        assert_eq!(record_at(&mut c, NOV_7).0[0]["detail"]["extra"], "第 45 周 · 第 311 天 · 立冬");
        let mut c = Calendar::new("", false, "en", offline());
        let w = record_at(&mut c, NOV_7).0.remove(0);
        assert_eq!((w["label"].as_str(), w["detail"]["extra"].as_str()), (Some("Sat, Nov 7"), Some("Week 45 · Day 311")));
        assert_eq!(w["detail"]["rows"][0]["text"], "Week 45 · Day 311");
        // New Year's Day 2027, a Friday, in English: its festival.
        let w = record_at(&mut c, (20_764 + 55) * DAY_MS).0.remove(0);
        assert_eq!((w["label"].as_str(), w["detail"]["extra"].as_str()), (Some("Fri, Jan 1"), Some("Week 53 · Day 1 · New Year's Day")));
    }

    const FEED: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//test//EN\r\nBEGIN:VEVENT\r\nUID:a\r\nDTSTART:20261107T100000\r\nDTEND:20261107T110000\r\nSUMMARY:周会\r\nLOCATION:会议室 A\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:b\r\nDTSTART;VALUE=DATE:20261108\r\nDTEND;VALUE=DATE:20261109\r\nSUMMARY:Offsite\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:c\r\nDTSTART:20261110T140000\r\nDTEND:20261110T143000\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:d\r\nDTSTART:20261107T080000\r\nDTEND:20261107T090000\r\nSUMMARY:Gone by\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

    // A feed's events: today's with its times, tomorrow's all day, one
    // without a title, one gone by left out; the feed read once a half
    // hour, not each minute; webcal:// is https://.
    #[test]
    fn the_events_coming_from_a_feed() {
        let feed = FEED.to_string();
        let (fetch, asked) = served_with(vec![("https://cal.example/x.ics".to_string(), Box::new(move || (200u16, feed.clone())) as Route)]);
        let mut c = Calendar::new("webcal://cal.example/x.ics", true, "zh", fetch);
        let (posts, said, _) = record_at(&mut c, NOV_7);
        assert_eq!(said, None);
        let rows = posts[0]["detail"]["rows"].as_array().unwrap().clone();
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[1], json!({ "kind": "event", "when": "10:00", "unit": "11:00", "name": "今天", "subject": "周会", "text": "会议室 A", "at": 20_764 * DAY_MS + 10 * 3_600_000 }));
        assert_eq!(rows[2], json!({ "kind": "event", "when": "全天", "unit": "", "name": "明天", "subject": "Offsite", "text": "", "at": (20_764 + 1) * DAY_MS }));
        assert_eq!((rows[3]["when"].as_str(), rows[3]["name"].as_str(), rows[3]["subject"].as_str()), (Some("14:00"), Some("周二"), Some("（无标题）")));
        record_at(&mut c, NOV_7 + 60_000);
        assert_eq!(asked.lock().unwrap().len(), 1, "read once a half hour");
        record_at(&mut c, NOV_7 + 31 * 60_000);
        assert_eq!(asked.lock().unwrap().len(), 2);
        assert_eq!(asked.lock().unwrap()[0], "https://cal.example/x.ics");
    }

    // Ten minutes before a meeting the island says so, once; an all-day
    // event never. In English too.
    #[test]
    fn the_island_says_so_ten_minutes_before() {
        let feed = FEED.to_string();
        let (fetch, _) = served_with(vec![("https://cal.example/".to_string(), Box::new(move || (200u16, feed.clone())) as Route)]);
        let mut c = Calendar::new("https://cal.example/x.ics", true, "zh", fetch);
        let at = |c: &mut Calendar, local: i64| record_at(c, local).0.remove(0)["nudge"].as_str().map(str::to_string);
        assert_eq!(at(&mut c, NOV_7), None, "half an hour before: not yet");
        assert_eq!(at(&mut c, NOV_7 + 20 * 60_000 + 30_000).as_deref(), Some("10 分钟后：周会"));
        assert_eq!(at(&mut c, NOV_7 + 22 * 60_000), None, "once");
        assert_eq!(at(&mut c, NOV_7 + 10 * 3_600_000), None, "tomorrow's all-day event: never");
        let feed = FEED.to_string();
        let (fetch, _) = served_with(vec![("https://cal.example/".to_string(), Box::new(move || (200u16, feed.clone())) as Route)]);
        let mut c = Calendar::new("https://cal.example/x.ics", false, "en", fetch);
        assert_eq!(at(&mut c, NOV_7 + 25 * 60_000).as_deref(), Some("In 5 min: 周会"));
    }

    // A feed that cannot be read is said on the Plugins page, the date
    // shown all the same; one of two failing, the other's events still come.
    #[test]
    fn trouble_with_a_feed_is_said() {
        let mut c = Calendar::new("https://down.example/a.ics", true, "zh", offline());
        let (posts, said, _) = record_at(&mut c, NOV_7);
        assert_eq!(posts[0]["value"], "7");
        assert!(said.is_some_and(|s| !s.is_empty()));
        let feed = FEED.to_string();
        let (fetch, _) = served_with(vec![("https://cal.example/".to_string(), Box::new(move || (200u16, feed.clone())) as Route), ("https://bad.example/".to_string(), Box::new(|| (404u16, "gone".to_string())) as Route)]);
        let mut c = Calendar::new("https://bad.example/x.ics, https://cal.example/x.ics", true, "en", fetch);
        let (posts, said, _) = record_at(&mut c, NOV_7);
        assert!(said.is_some());
        assert_eq!(posts[0]["detail"]["rows"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn weeks_times_and_offsets() {
        assert_eq!(week_of(20_764), (45, 311));
        assert_eq!(week_of(20_454), (1, 1), "2026-01-01, a Thursday, is in week 1");
        assert_eq!(week_of(20_819), (53, 1), "2027-01-01, a Friday, is in 2026's week 53");
        assert_eq!(hhmm(NOV_7), "09:30");
        assert_eq!(hhmm(-60_000), "23:59");
        assert_eq!(offset_of(Now { ms: 1_000, local: 8 * 3_600_000 + 1_000 }), 8 * 3_600_000);
        assert_eq!(offset_of(Now { ms: 1_000, local: 1_000 - 5 * 3_600_000 + 20_000 }), -5 * 3_600_000);
    }
}
