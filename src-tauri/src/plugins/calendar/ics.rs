// An iCalendar feed (RFC 5545) read for the calendar plugin: the events in a
// calendar a person subscribes to (Google Calendar's secret iCal address,
// Outlook's published calendar, Feishu, DingTalk and iCloud exports), each
// occurrence of each in the span of time asked for, on the local calendar.
// The plugin downloads the feed; this only reads it: no I/O, nothing but the
// standard library, and nothing a feed may hold makes it panic.
//
// What it reads: VEVENT (SUMMARY, LOCATION, DTSTART, DTEND, DURATION, RRULE,
// RDATE, EXDATE, RECURRENCE-ID, STATUS, UID) and VTIMEZONE (STANDARD and
// DAYLIGHT, each with TZOFFSETTO, DTSTART, a yearly RRULE and RDATE); a
// VALARM inside an event is passed over. Times are UTC, floating, or in a
// zone: one described in the file, or one of a few known by name that keep
// no summer time (Asia/Shanghai, Asia/Tokyo, "China Standard Time", UTC…);
// any other name is taken as floating, that is local. An event edited on its
// own (RECURRENCE-ID) stands in for the series' occurrence at that start; one
// STATUS:CANCELLED is left out.
//
// Of RRULE: FREQ=DAILY, WEEKLY, MONTHLY and YEARLY; INTERVAL; COUNT; UNTIL;
// WKST; BYDAY (weekdays for WEEKLY, and a filter for DAILY; with an ordinal
// for MONTHLY, and for YEARLY within BYMONTH: 2TU, -1FR); BYMONTHDAY (MONTHLY
// and YEARLY, a filter for DAILY); BYMONTH (YEARLY, a filter otherwise);
// BYSETPOS. Not read: BYWEEKNO, BYYEARDAY, BYHOUR, BYMINUTE, BYSECOND,
// FREQ=HOURLY, MINUTELY and SECONDLY, YEARLY BYDAY with an ordinal and no
// BYMONTH, an INTERVAL over 1000, and RECURRENCE-ID;RANGE=THISANDFUTURE (taken
// as that one instance): a rule with one of those gives its first instance
// only. Expansion is bounded (5000 steps an event, a span of 400 days at
// most), and leaps from an old DTSTART to the span asked for unless a COUNT
// has to be counted from the start.
use std::collections::HashMap;

const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;
// A span asked for is cut to this many days; a rule is given up after this
// many steps (candidate instances and periods together).
const MAX_DAYS: i64 = 400;
const MAX_STEPS: usize = 5000;
const MAX_INTERVAL: i64 = 1000;

/// One occurrence of an event, as it falls in the window asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    /// SUMMARY, unescaped (\\, \n, \; and \, read), on one line, trimmed; "" if none.
    pub summary: String,
    /// LOCATION, the same way; "" if none.
    pub location: String,
    /// Local-calendar ms since 1970: the wall clock's reading as a number.
    pub start: i64,
    /// Local-calendar ms, never before `start`. For an all-day event, the
    /// local midnight that ends its last day.
    pub end: i64,
    /// DTSTART;VALUE=DATE: a whole day (or days).
    pub all_day: bool,
}

/// The occurrences of every VEVENT in `text` that overlap [from, to) (local-calendar
/// ms), sorted by start then summary. `utc_offset_ms` is the local time zone's
/// offset from UTC now (8 * 3_600_000 for Asia/Shanghai), used as a fixed offset.
/// At most `max` occurrences are returned (the earliest ones); a span longer
/// than 400 days is cut to its first 400.
pub fn events(text: &str, from: i64, to: i64, utc_offset_ms: i64, max: usize) -> Vec<Event> {
    // The span, within the years read (1970–2100) and 400 days.
    let last = days_from_civil(2101, 1, 1) * DAY;
    let from = from.clamp(0, last);
    let to = to.clamp(0, last).min(from.saturating_add(MAX_DAYS * DAY));
    if to <= from || max == 0 {
        return Vec::new();
    }
    let root = components(&unfold(text));
    let mut zones = HashMap::new();
    let mut found = Vec::new();
    gather(&root, &mut zones, &mut found);
    let ctx = Ctx { zones: &zones, utc_offset: utc_offset_ms };
    let masters: Vec<Master> = found.iter().filter_map(|c| Master::read(c, &ctx)).collect();
    // The instances edited on their own, by the UID of their series: the
    // series' own occurrence at each such start gives way to them (a
    // cancelled one takes the occurrence away).
    let mut edited: HashMap<&str, Vec<Stamp>> = HashMap::new();
    for m in &masters {
        if let Some(id) = m.recurrence_id.filter(|_| !m.uid.is_empty()) {
            edited.entry(m.uid.as_str()).or_default().push(id);
        }
    }
    let mut out = Vec::new();
    for m in &masters {
        if m.cancelled {
            continue;
        }
        let edits = if m.recurrence_id.is_none() { edited.get(m.uid.as_str()) } else { None };
        for start in m.starts(from, to, utc_offset_ms) {
            let end = start.saturating_add(m.duration);
            if start >= to || (end <= from && start < from) {
                continue;
            }
            if m.exdates.iter().any(|x| x.matches(start, m.all_day)) || edits.is_some_and(|e| e.iter().any(|x| x.matches(start, m.all_day))) {
                continue;
            }
            out.push(Event {
                summary: m.summary.clone(),
                location: m.location.clone(),
                start,
                end,
                all_day: m.all_day,
            });
        }
    }
    out.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| a.summary.cmp(&b.summary)));
    out.truncate(max);
    out
}

// --- Lines: unfolded, each its name, parameters and value --------------------------------

// A property as written: its name and its parameters' names in upper case,
// a parameter's quotes taken off.
struct Prop {
    name: String,
    params: Vec<(String, String)>,
    value: String,
}

impl Prop {
    fn param(&self, name: &str) -> Option<&str> {
        self.params.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }
}

// A component (VCALENDAR, VEVENT, VTIMEZONE, STANDARD…): its own properties
// and the components inside it.
struct Component {
    name: String,
    props: Vec<Prop>,
    children: Vec<Component>,
}

impl Component {
    fn prop(&self, name: &str) -> Option<&Prop> {
        self.props.iter().find(|p| p.name == name)
    }

    fn props<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Prop> + 'a {
        self.props.iter().filter(move |p| p.name == name)
    }

    fn value(&self, name: &str) -> Option<&str> {
        self.prop(name).map(|p| p.value.as_str())
    }
}

// The lines, CRLF or LF, a continuation (a line starting with a space or a
// tab) joined to the line before it; a byte order mark and empty lines dropped.
fn unfold(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for raw in text.trim_start_matches('\u{feff}').split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if let (Some(rest), Some(last)) = (line.strip_prefix([' ', '\t']), lines.last_mut()) {
            last.push_str(rest);
        } else if !line.is_empty() {
            lines.push(line.to_string());
        }
    }
    lines
}

// The pieces of `s` between each `sep` outside double quotes.
fn split_unquoted(s: &str, sep: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut quoted, mut start) = (false, 0);
    for (i, c) in s.char_indices() {
        if c == '"' {
            quoted = !quoted;
        } else if c == sep && !quoted {
            parts.push(&s[start..i]);
            start = i + 1;
        }
    }
    parts.push(&s[start..]);
    parts
}

// A line read: NAME;PARAM=VALUE;PARAM2="quoted,value":value. None without a
// colon outside quotes, or without a name.
fn prop(line: &str) -> Option<Prop> {
    let head = split_unquoted(line, ':');
    if head.len() < 2 {
        return None;
    }
    let value = line[head[0].len() + 1..].to_string();
    let mut parts = split_unquoted(head[0], ';').into_iter();
    let name = parts.next()?.trim().to_ascii_uppercase();
    if name.is_empty() {
        return None;
    }
    let params = parts
        .filter_map(|p| {
            let (k, v) = p.split_once('=')?;
            Some((k.trim().to_ascii_uppercase(), v.trim().trim_matches('"').to_string()))
        })
        .collect();
    Some(Prop { name, params, value })
}

// The lines as a tree of components under a nameless root. An END closes the
// open component of its name and whatever was left open inside it; what is
// still open at the end (a feed cut short) is closed as it stands.
fn components(lines: &[String]) -> Component {
    let blank = |name: String| Component { name, props: Vec::new(), children: Vec::new() };
    let close = |stack: &mut Vec<Component>, upto: usize| {
        while stack.len() > upto {
            if let (Some(done), Some(parent)) = (stack.pop(), stack.last_mut()) {
                parent.children.push(done);
            }
        }
    };
    let mut stack = vec![blank(String::new())];
    for line in lines {
        let Some(p) = prop(line) else { continue };
        match p.name.as_str() {
            "BEGIN" => stack.push(blank(p.value.trim().to_ascii_uppercase())),
            "END" => {
                let name = p.value.trim().to_ascii_uppercase();
                if let Some(at) = stack.iter().rposition(|c| c.name == name).filter(|&at| at > 0) {
                    close(&mut stack, at);
                }
            }
            _ => {
                if let Some(open) = stack.last_mut() {
                    open.props.push(p);
                }
            }
        }
    }
    close(&mut stack, 1);
    stack.pop().unwrap_or_else(|| blank(String::new()))
}

// Every VEVENT and VTIMEZONE under `c`, wherever they are (a VALARM inside an
// event is not looked into).
fn gather<'a>(c: &'a Component, zones: &mut HashMap<String, Zone>, events: &mut Vec<&'a Component>) {
    for child in &c.children {
        match child.name.as_str() {
            "VEVENT" => events.push(child),
            "VTIMEZONE" => {
                if let (Some(id), Some(zone)) = (child.value("TZID"), Zone::read(child)) {
                    zones.insert(id.trim().to_ascii_uppercase(), zone);
                }
            }
            _ => gather(child, zones, events),
        }
    }
}

// A text value as it was typed: \\ a backslash, \n (or \N) a line break, \,
// and \; the signs themselves; then on one line, its spaces single, trimmed.
fn text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') | Some('N') => out.push('\n'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

// --- Dates and times ----------------------------------------------------------------------

// Days since 1970-01-01 of a date on the proleptic Gregorian calendar
// (Howard Hinnant's days_from_civil, as countdown.rs has it).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

// The date of a day since 1970 (his civil_from_days, as countdown.rs has
// it): year, month, day.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
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

// The weekday of a day since 1970, Sunday 0 to Saturday 6 (1970-01-01 was a
// Thursday).
fn weekday_of(days: i64) -> i64 {
    (days + 4).rem_euclid(7)
}

// The first day of the week a day is in, weeks starting on `wkst`.
fn week_start(days: i64, wkst: i64) -> i64 {
    days - (weekday_of(days) - wkst).rem_euclid(7)
}

// The nth weekday of a month (2 for the second Tuesday, -1 for the last
// Friday) as a day since 1970; none when the month has no such day.
fn nth_weekday(y: i64, m: i64, nth: i64, weekday: i64) -> Option<i64> {
    let first = days_from_civil(y, m, 1);
    let last = first + days_in(y, m) - 1;
    let day = if nth > 0 { first + (weekday - weekday_of(first)).rem_euclid(7) + (nth - 1) * 7 } else { last - (weekday_of(last) - weekday).rem_euclid(7) + (nth + 1) * 7 };
    (first..=last).contains(&day).then_some(day)
}

// SU, MO… as a weekday number.
fn weekday_named(s: &str) -> Option<i64> {
    ["SU", "MO", "TU", "WE", "TH", "FR", "SA"].iter().position(|d| d.eq_ignore_ascii_case(s)).map(|i| i as i64)
}

// A date or date-time as written: 20261107, 20261107T100000, with a Z after
// it for UTC. Any year 0–9999 (a zone's rules start in 1601 or 1970); the
// events' years are checked where they are read.
#[derive(Clone, Copy)]
struct Written {
    days: i64,
    ms: i64,
    date: bool,
    utc: bool,
}

impl Written {
    fn wall(&self) -> i64 {
        self.days * DAY + self.ms
    }

    fn in_range(&self) -> bool {
        (1970..=2100).contains(&civil_from_days(self.days).0)
    }
}

fn written(value: &str) -> Option<Written> {
    let v = value.trim();
    let (v, utc) = match v.strip_suffix(['Z', 'z']) {
        Some(rest) => (rest, true),
        None => (v, false),
    };
    let (date, time) = match v.split_once(['T', 't']) {
        Some((d, t)) => (d, Some(t)),
        None => (v, None),
    };
    fn digits(s: &str, len: usize) -> Option<&str> {
        (s.len() == len && s.bytes().all(|b| b.is_ascii_digit())).then_some(s)
    }
    let date = digits(date, 8)?;
    let n = |s: &str| s.parse::<i64>().ok();
    let (y, m, d) = (n(&date[..4])?, n(&date[4..6])?, n(&date[6..])?);
    if !(1..=12).contains(&m) || !(1..=days_in(y, m)).contains(&d) {
        return None;
    }
    let days = days_from_civil(y, m, d);
    let Some(time) = time else {
        return (!utc).then_some(Written { days, ms: 0, date: true, utc: false });
    };
    let time = digits(time, 6)?;
    let (h, mi, s) = (n(&time[..2])?, n(&time[2..4])?, n(&time[4..])?);
    if h > 23 || mi > 59 || s > 60 {
        return None;
    }
    Some(Written { days, ms: h * HOUR + mi * MINUTE + s.min(59) * 1000, date: false, utc })
}

// A duration as written: P1D, PT1H30M, PT45M, P1DT2H, P2W. A negative one is
// nothing (an event cannot end before it starts).
fn duration(value: &str) -> Option<i64> {
    let v = value.trim();
    let (negative, v) = match v.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, v.strip_prefix('+').unwrap_or(v)),
    };
    let v = v.strip_prefix(['P', 'p'])?;
    let (date, time) = v.split_once(['T', 't']).unwrap_or((v, ""));
    let mut total: i64 = 0;
    let mut digits = String::new();
    for (part, in_date) in [(date, true), (time, false)] {
        for c in part.chars() {
            if c.is_ascii_digit() {
                digits.push(c);
                continue;
            }
            let n: i64 = digits.parse().ok()?;
            digits.clear();
            let unit = match (c.to_ascii_uppercase(), in_date) {
                ('W', true) => 7 * DAY,
                ('D', true) => DAY,
                ('H', false) => HOUR,
                ('M', false) => MINUTE,
                ('S', false) => 1000,
                _ => return None,
            };
            total = total.checked_add(n.checked_mul(unit)?)?;
        }
        if !digits.is_empty() {
            return None;
        }
    }
    Some(if negative { 0 } else { total })
}

// +0800, -0500, +0530, +053000 as ms; also +08:00.
fn offset_written(value: &str) -> Option<i64> {
    let v = value.trim();
    let (sign, rest) = match v.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, v.strip_prefix('+').unwrap_or(v)),
    };
    let digits: String = rest.chars().filter(|c| *c != ':').collect();
    if !(digits.len() == 4 || digits.len() == 6) || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (h, m) = (digits[..2].parse::<i64>().ok()?, digits[2..4].parse::<i64>().ok()?);
    let s = if digits.len() == 6 { digits[4..].parse::<i64>().ok()? } else { 0 };
    (h <= 23 && m <= 59 && s <= 59).then_some(sign * (h * HOUR + m * MINUTE + s * 1000))
}

// --- Time zones ---------------------------------------------------------------------------

// A zone described in the file (VTIMEZONE): its STANDARD and DAYLIGHT
// shifts, each the offset it brings in (TZOFFSETTO), the one it leaves
// (TZOFFSETFROM), when it first came (DTSTART, a wall time), how it comes
// round (a yearly RRULE: the nth weekday of a month, perhaps until some
// year) and any other times it came (RDATE).
struct Zone {
    shifts: Vec<Shift>,
}

struct Shift {
    to: i64,
    from: i64,
    start: i64,
    yearly: Option<Yearly>,
    dates: Vec<i64>,
}

struct Yearly {
    month: i64,
    nth: i64,
    weekday: i64,
    until: Option<i64>,
}

impl Zone {
    fn read(c: &Component) -> Option<Zone> {
        let shifts: Vec<Shift> = c.children.iter().filter(|s| s.name == "STANDARD" || s.name == "DAYLIGHT").filter_map(Shift::read).collect();
        (!shifts.is_empty()).then_some(Zone { shifts })
    }

    // The zone's offset from UTC at a wall time in it: that of the latest
    // shift that came at or before it; before any, the earliest's TZOFFSETFROM.
    fn offset_at(&self, wall: i64) -> i64 {
        let mut best: Option<(i64, i64)> = None;
        for s in &self.shifts {
            if let Some(t) = s.latest(wall).filter(|&t| best.is_none_or(|(b, _)| t > b)) {
                best = Some((t, s.to));
            }
        }
        match best {
            Some((_, to)) => to,
            None => self.shifts.iter().min_by_key(|s| s.start).map_or(0, |s| s.from),
        }
    }
}

impl Shift {
    fn read(c: &Component) -> Option<Shift> {
        let to = offset_written(c.value("TZOFFSETTO")?)?;
        let from = c.value("TZOFFSETFROM").and_then(offset_written).unwrap_or(to);
        let start = written(c.value("DTSTART")?)?.wall();
        // The rule's UNTIL is written in UTC; a wall time is near enough to
        // tell the year it stops.
        let yearly = c.value("RRULE").map(|r| Rule::parse(r, |u| written(u).map(|w| w.wall()))).and_then(|r| match (r.freq, r.by_month.as_slice(), r.by_day.as_slice()) {
            (Freq::Yearly, [month], [(nth, weekday)]) if *nth != 0 => Some(Yearly { month: *month, nth: *nth, weekday: *weekday, until: r.until }),
            _ => None,
        });
        let dates = c.props("RDATE").flat_map(|p| p.value.split(',')).filter_map(|v| written(v.split('/').next().unwrap_or(v))).map(|w| w.wall()).collect();
        Some(Shift { to, from, start, yearly, dates })
    }

    // When this shift last came, at or before a wall time; none if not yet.
    fn latest(&self, wall: i64) -> Option<i64> {
        let mut best = (self.start <= wall).then_some(self.start);
        for &d in &self.dates {
            if d <= wall && best.is_none_or(|b| d > b) {
                best = Some(d);
            }
        }
        if let Some(y) = &self.yearly {
            let tod = self.start.rem_euclid(DAY);
            let (year, _, _) = civil_from_days(wall.div_euclid(DAY));
            let year = y.until.map_or(year, |u| civil_from_days(u.div_euclid(DAY)).0.min(year));
            // This year's, else last year's. A fifth weekday a month has not
            // got is its last (Windows writes the last week as the fifth).
            for yr in [year, year - 1] {
                let Some(day) = nth_weekday(yr, y.month, y.nth, y.weekday).or_else(|| nth_weekday(yr, y.month, -1, y.weekday)) else { continue };
                let t = day * DAY + tod;
                if t <= wall && t >= self.start && y.until.is_none_or(|u| t <= u) {
                    if best.is_none_or(|b| t > b) {
                        best = Some(t);
                    }
                    break;
                }
            }
        }
        best
    }
}

// Zones known by name that keep no summer time: IANA's and the names
// Windows (Outlook) gives them. Any other zone the file does not describe is
// taken as floating.
const KNOWN_ZONES: &[(&str, i64)] = &[
    ("UTC", 0),
    ("ETC/UTC", 0),
    ("ETC/GMT", 0),
    ("GMT", 0),
    ("Z", 0),
    ("ASIA/SHANGHAI", 8 * HOUR),
    ("ASIA/CHONGQING", 8 * HOUR),
    ("ASIA/HONG_KONG", 8 * HOUR),
    ("ASIA/MACAU", 8 * HOUR),
    ("ASIA/TAIPEI", 8 * HOUR),
    ("ASIA/SINGAPORE", 8 * HOUR),
    ("ASIA/KUALA_LUMPUR", 8 * HOUR),
    ("ASIA/MANILA", 8 * HOUR),
    ("AUSTRALIA/PERTH", 8 * HOUR),
    ("ASIA/TOKYO", 9 * HOUR),
    ("ASIA/SEOUL", 9 * HOUR),
    ("ASIA/KOLKATA", 5 * HOUR + 30 * MINUTE),
    ("ASIA/CALCUTTA", 5 * HOUR + 30 * MINUTE),
    ("ASIA/BANGKOK", 7 * HOUR),
    ("ASIA/JAKARTA", 7 * HOUR),
    ("ASIA/HO_CHI_MINH", 7 * HOUR),
    ("ASIA/DUBAI", 4 * HOUR),
    ("CHINA STANDARD TIME", 8 * HOUR),
    ("TAIPEI STANDARD TIME", 8 * HOUR),
    ("SINGAPORE STANDARD TIME", 8 * HOUR),
    ("W. AUSTRALIA STANDARD TIME", 8 * HOUR),
    ("TOKYO STANDARD TIME", 9 * HOUR),
    ("KOREA STANDARD TIME", 9 * HOUR),
    ("INDIA STANDARD TIME", 5 * HOUR + 30 * MINUTE),
    ("SE ASIA STANDARD TIME", 7 * HOUR),
    ("ARABIAN STANDARD TIME", 4 * HOUR),
];

// The fixed offset of a zone known by its name: one of the table's, found
// also at the end of a path ("/freeassociation.sourceforge.net/Tzfile/Asia/
// Shanghai"), or read from a name that carries it ("(UTC+08:00) Beijing,
// Chongqing, Hong Kong, Urumqi").
fn known_offset(tzid: &str) -> Option<i64> {
    let name = tzid.trim().to_ascii_uppercase();
    if let Some(rest) = name.strip_prefix("(UTC") {
        let inside = &rest[..rest.find(')')?];
        return if inside.is_empty() { Some(0) } else { offset_written(inside) };
    }
    let tails = std::iter::once(0).chain(name.match_indices('/').map(|(i, _)| i + 1));
    tails.map(|i| &name[i..]).find_map(|tail| KNOWN_ZONES.iter().find(|(n, _)| *n == tail).map(|(_, o)| *o))
}

// How a wall time written for an event is brought onto the local calendar.
#[derive(Clone, Copy)]
enum Conv<'a> {
    Floating,
    Utc,
    Fixed(i64),
    Zone(&'a Zone),
}

impl Conv<'_> {
    fn local(&self, wall: i64, utc_offset: i64) -> i64 {
        match self {
            Conv::Floating => wall,
            Conv::Utc => wall + utc_offset,
            Conv::Fixed(offset) => wall - offset + utc_offset,
            Conv::Zone(zone) => wall - zone.offset_at(wall) + utc_offset,
        }
    }
}

// What the file's dates are read against: its zones and the local offset.
struct Ctx<'a> {
    zones: &'a HashMap<String, Zone>,
    utc_offset: i64,
}

// A moment on the local calendar, and whether it was written as a date
// (then it names the whole day).
#[derive(Clone, Copy, Debug)]
struct Stamp {
    at: i64,
    date: bool,
}

impl Stamp {
    // Whether an occurrence starting at `start` is the one this names: the
    // same moment, or (either a date) the same day.
    fn matches(&self, start: i64, all_day: bool) -> bool {
        if self.date || all_day {
            self.at.div_euclid(DAY) == start.div_euclid(DAY)
        } else {
            self.at == start
        }
    }
}

impl<'a> Ctx<'a> {
    // The conversion a property's value takes: UTC if written so, its
    // TZID's zone (the file's, else one known by name), else `fallback`:
    // floating for a DTSTART, the event's own for an EXDATE or UNTIL
    // written without one.
    fn conv(&self, p: &Prop, w: Written, fallback: Conv<'a>) -> Conv<'a> {
        if w.utc {
            return Conv::Utc;
        }
        match p.param("TZID") {
            Some(id) => self.zones.get(&id.trim().to_ascii_uppercase()).map(Conv::Zone).or_else(|| known_offset(id).map(Conv::Fixed)).unwrap_or(Conv::Floating),
            None => fallback,
        }
    }

    fn local(&self, w: Written, conv: Conv) -> i64 {
        if w.date {
            w.days * DAY
        } else {
            conv.local(w.wall(), self.utc_offset)
        }
    }

    // One value of a property (a DTEND, one of an EXDATE's) on the local
    // calendar; none if malformed or outside 1970–2100.
    fn stamp(&self, p: &Prop, value: &str, fallback: Conv<'a>) -> Option<Stamp> {
        let w = written(value)?;
        w.in_range().then(|| Stamp { at: self.local(w, self.conv(p, w, fallback)), date: w.date })
    }

    // Every value of every property of that name (EXDATE and RDATE hold
    // several, with commas, and come on several lines). An RDATE period
    // (start/end) counts by its start.
    fn stamps(&self, c: &Component, name: &str, fallback: Conv<'a>) -> Vec<Stamp> {
        c.props(name).flat_map(|p| p.value.split(',').map(move |v| (p, v))).filter_map(|(p, v)| self.stamp(p, v.split('/').next().unwrap_or(v), fallback)).collect()
    }
}

// --- Recurrence rules ---------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Freq {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

// An RRULE as read. `until` is on the local calendar (a wall time for a
// zone's rule), inclusive; `by_day` pairs an ordinal (0 for none) with a
// weekday. `whole` is false when a part was not read (see the top): the
// first instance only, then.
struct Rule {
    freq: Freq,
    interval: i64,
    count: Option<i64>,
    until: Option<i64>,
    wkst: i64,
    by_day: Vec<(i64, i64)>,
    by_month_day: Vec<i64>,
    by_month: Vec<i64>,
    by_set_pos: Vec<i64>,
    whole: bool,
}

impl Rule {
    // `until` reads the UNTIL value onto the calendar the rule is expanded on.
    fn parse(value: &str, until: impl Fn(&str) -> Option<i64>) -> Rule {
        let mut r = Rule {
            freq: Freq::Daily,
            interval: 1,
            count: None,
            until: None,
            wkst: 1,
            by_day: Vec::new(),
            by_month_day: Vec::new(),
            by_month: Vec::new(),
            by_set_pos: Vec::new(),
            whole: true,
        };
        let mut freq = None;
        // Each list of numbers within its bounds, or the rule is not whole.
        let numbers = |v: &str, ok: &dyn Fn(i64) -> bool, whole: &mut bool| -> Vec<i64> {
            let ns: Vec<i64> = v.split(',').filter_map(|n| n.trim().parse::<i64>().ok().filter(|n| ok(*n))).collect();
            if ns.len() != v.split(',').count() {
                *whole = false;
            }
            ns
        };
        for part in value.split(';') {
            let Some((k, v)) = part.split_once('=') else { continue };
            let v = v.trim();
            match k.trim().to_ascii_uppercase().as_str() {
                "FREQ" => {
                    freq = match v.to_ascii_uppercase().as_str() {
                        "DAILY" => Some(Freq::Daily),
                        "WEEKLY" => Some(Freq::Weekly),
                        "MONTHLY" => Some(Freq::Monthly),
                        "YEARLY" => Some(Freq::Yearly),
                        _ => None,
                    }
                }
                "INTERVAL" => match v.parse::<i64>() {
                    Ok(n) if (1..=MAX_INTERVAL).contains(&n) => r.interval = n,
                    _ => r.whole = false,
                },
                "COUNT" => match v.parse::<i64>() {
                    Ok(n) if n >= 1 => r.count = Some(n),
                    _ => r.whole = false,
                },
                "UNTIL" => match until(v) {
                    Some(u) => r.until = Some(u),
                    None => r.whole = false,
                },
                "WKST" => match weekday_named(v) {
                    Some(d) => r.wkst = d,
                    None => r.whole = false,
                },
                "BYDAY" => {
                    for item in v.split(',') {
                        // The last two characters name the day, what is before them the ordinal.
                        let item = item.trim();
                        let (ordinal, day) = item.char_indices().nth_back(1).map_or(("", item), |(at, _)| item.split_at(at));
                        let nth = if ordinal.is_empty() { Some(0) } else { ordinal.parse::<i64>().ok().filter(|n| (1..=53).contains(&n.abs())) };
                        match (nth, weekday_named(day)) {
                            (Some(nth), Some(wd)) => r.by_day.push((nth, wd)),
                            _ => r.whole = false,
                        }
                    }
                }
                "BYMONTHDAY" => r.by_month_day = numbers(v, &|n| (1..=31).contains(&n.abs()), &mut r.whole),
                "BYMONTH" => r.by_month = numbers(v, &|n| (1..=12).contains(&n), &mut r.whole),
                "BYSETPOS" => r.by_set_pos = numbers(v, &|n| (1..=366).contains(&n.abs()), &mut r.whole),
                "BYWEEKNO" | "BYYEARDAY" | "BYHOUR" | "BYMINUTE" | "BYSECOND" => r.whole = false,
                _ => {}
            }
        }
        match freq {
            Some(f) => r.freq = f,
            None => r.whole = false,
        }
        // The nth weekday of a year (YEARLY, BYDAY with an ordinal, no
        // BYMONTH) is not counted.
        if r.freq == Freq::Yearly && r.by_month.is_empty() && r.by_day.iter().any(|(nth, _)| *nth != 0) {
            r.whole = false;
        }
        r
    }

    // The first period that can hold a day at or after `lower_days`, the
    // start's being the 0th (an old series leaps to the span asked for).
    fn first_period(&self, start_days: i64, lower_days: i64) -> i64 {
        if lower_days <= start_days {
            return 0;
        }
        match self.freq {
            Freq::Daily => (lower_days - start_days) / self.interval,
            Freq::Weekly => (lower_days - week_start(start_days, self.wkst)) / (7 * self.interval),
            Freq::Monthly => {
                let ((y0, m0, _), (y, m, _)) = (civil_from_days(start_days), civil_from_days(lower_days));
                ((y * 12 + m) - (y0 * 12 + m0)) / self.interval
            }
            Freq::Yearly => (civil_from_days(lower_days).0 - civil_from_days(start_days).0) / self.interval,
        }
    }

    // The days (since 1970) the rule names in its k-th period from the
    // start, in order, BYSETPOS applied.
    fn days_in_period(&self, start_days: i64, k: i64) -> Vec<i64> {
        let (y0, m0, d0) = civil_from_days(start_days);
        let in_months = |months: &[i64]| self.by_month.is_empty() || months.iter().all(|m| self.by_month.contains(m));
        let mut days: Vec<i64> = match self.freq {
            Freq::Daily => {
                let day = start_days + k * self.interval;
                let (y, m, d) = civil_from_days(day);
                let on_day = self.by_month_day.is_empty() || self.by_month_day.iter().any(|&md| md == d || md == d - days_in(y, m) - 1);
                let on_weekday = self.by_day.is_empty() || self.by_day.iter().any(|(_, wd)| *wd == weekday_of(day));
                if in_months(&[m]) && on_day && on_weekday {
                    vec![day]
                } else {
                    Vec::new()
                }
            }
            Freq::Weekly => {
                let ws = week_start(start_days, self.wkst) + k * 7 * self.interval;
                let weekdays: Vec<i64> = if self.by_day.is_empty() { vec![weekday_of(start_days)] } else { self.by_day.iter().map(|(_, wd)| *wd).collect() };
                weekdays.iter().map(|wd| ws + (wd - self.wkst).rem_euclid(7)).filter(|&d| in_months(&[civil_from_days(d).1])).collect()
            }
            Freq::Monthly => {
                let mi = y0 * 12 + (m0 - 1) + k * self.interval;
                let (y, m) = (mi.div_euclid(12), mi.rem_euclid(12) + 1);
                if in_months(&[m]) {
                    self.month_days(y, m, d0)
                } else {
                    Vec::new()
                }
            }
            Freq::Yearly => {
                let y = y0 + k * self.interval;
                // BYMONTH's months; without it, every month when BYMONTHDAY or
                // BYDAY say which days, else the start's month.
                let months: Vec<i64> = if !self.by_month.is_empty() {
                    self.by_month.clone()
                } else if !self.by_month_day.is_empty() || !self.by_day.is_empty() {
                    (1..=12).collect()
                } else {
                    vec![m0]
                };
                months.iter().flat_map(|&m| self.month_days(y, m, d0)).collect()
            }
        };
        days.sort_unstable();
        days.dedup();
        if !self.by_set_pos.is_empty() {
            let len = days.len() as i64;
            let mut picked: Vec<i64> = self.by_set_pos.iter().filter_map(|&p| days.get(usize::try_from(if p > 0 { p - 1 } else { len + p }).ok()?).copied()).collect();
            picked.sort_unstable();
            picked.dedup();
            days = picked;
        }
        days
    }

    // The days of a month a MONTHLY or YEARLY rule names: BYMONTHDAY's (-1 the
    // last), else BYDAY's (2TU, -1FR, or every TU), else the start's own day
    // of the month (none in a month too short for it).
    fn month_days(&self, y: i64, m: i64, d0: i64) -> Vec<i64> {
        let first = days_from_civil(y, m, 1);
        let n = days_in(y, m);
        if !self.by_month_day.is_empty() {
            return self.by_month_day.iter().map(|&d| if d > 0 { d } else { n + 1 + d }).filter(|d| (1..=n).contains(d)).map(|d| first + d - 1).collect();
        }
        if !self.by_day.is_empty() {
            return self
                .by_day
                .iter()
                .flat_map(|&(nth, wd)| {
                    if nth == 0 {
                        let start = first + (wd - weekday_of(first)).rem_euclid(7);
                        (start..first + n).step_by(7).collect::<Vec<i64>>()
                    } else {
                        nth_weekday(y, m, nth, wd).into_iter().collect()
                    }
                })
                .collect();
        }
        (d0 <= n).then(|| first + d0 - 1).into_iter().collect()
    }
}

// --- Events -------------------------------------------------------------------------------

// A VEVENT as read: its start as written (a wall time in its own zone, UTC,
// or floating) and how that comes onto the local calendar; its length on
// the local calendar; its rule; the dates taken out and put in; which
// occurrence of its series it stands in for, if any.
struct Master<'a> {
    uid: String,
    summary: String,
    location: String,
    start: i64,
    conv: Conv<'a>,
    all_day: bool,
    duration: i64,
    rule: Option<Rule>,
    exdates: Vec<Stamp>,
    rdates: Vec<Stamp>,
    recurrence_id: Option<Stamp>,
    cancelled: bool,
}

impl<'a> Master<'a> {
    // None without a DTSTART that reads and lies in 1970–2100.
    fn read(c: &Component, ctx: &Ctx<'a>) -> Option<Master<'a>> {
        let dtstart = c.prop("DTSTART")?;
        let w = written(&dtstart.value).filter(Written::in_range)?;
        let conv = ctx.conv(dtstart, w, Conv::Floating);
        let all_day = w.date;
        let start_local = ctx.local(w, conv);
        // DTEND (exclusive), else DURATION, else nothing: a timed event ends
        // as it starts, a whole day lasts the day.
        let duration = match c.prop("DTEND").and_then(|p| ctx.stamp(p, &p.value, conv)) {
            Some(end) => end.at - start_local,
            None => c.value("DURATION").and_then(duration).unwrap_or(if all_day { DAY } else { 0 }),
        };
        let duration = if all_day { duration.max(DAY) } else { duration.max(0) };
        let rule = c.value("RRULE").map(|r| {
            Rule::parse(r, |u| {
                // UNTIL: in UTC, or in the event's own zone; a date names the
                // whole of its last day.
                let w = written(u)?;
                Some(if w.date { w.days * DAY + DAY - 1 } else { ctx.local(w, if w.utc { Conv::Utc } else { conv }) })
            })
        });
        let status = c.value("STATUS").map(|s| s.trim().to_ascii_uppercase()).unwrap_or_default();
        Some(Master {
            uid: c.value("UID").map(|u| u.trim().to_string()).unwrap_or_default(),
            summary: c.value("SUMMARY").map(text).unwrap_or_default(),
            location: c.value("LOCATION").map(text).unwrap_or_default(),
            start: if all_day { w.days * DAY } else { w.wall() },
            conv: if all_day { Conv::Floating } else { conv },
            all_day,
            duration,
            rule,
            exdates: ctx.stamps(c, "EXDATE", conv),
            rdates: ctx.stamps(c, "RDATE", conv),
            recurrence_id: c.prop("RECURRENCE-ID").and_then(|p| ctx.stamp(p, &p.value, conv)),
            cancelled: status == "CANCELLED",
        })
    }

    // Each occurrence's local start that may overlap [from, to), in order of
    // the rule: the start itself, the series, then the RDATEs. An instance
    // edited on its own has only its start.
    fn starts(&self, from: i64, to: i64, utc_offset: i64) -> Vec<i64> {
        let start_local = self.conv.local(self.start, utc_offset);
        let mut out = vec![start_local];
        if self.recurrence_id.is_some() {
            return out;
        }
        if let Some(rule) = self.rule.as_ref().filter(|r| r.whole) {
            if rule.until.is_some_and(|u| start_local > u) {
                out.clear();
            }
            let start_days = self.start.div_euclid(DAY);
            let tod = self.start.rem_euclid(DAY);
            // An occurrence ending after `from` starts after `from` less the
            // length; two days' grace for the zone's offset and the local one.
            let lower_days = (from - self.duration - 2 * DAY).div_euclid(DAY);
            let mut k = if rule.count.is_some() { 0 } else { rule.first_period(start_days, lower_days) };
            let (mut steps, mut n) = (0, 1);
            'periods: while steps < MAX_STEPS {
                steps += 1;
                for day in rule.days_in_period(start_days, k) {
                    steps += 1;
                    let wall = day * DAY + tod;
                    if wall <= self.start {
                        continue;
                    }
                    let local = self.conv.local(wall, utc_offset);
                    if rule.until.is_some_and(|u| local > u) || local >= to || steps > MAX_STEPS {
                        break 'periods;
                    }
                    n += 1;
                    if rule.count.is_some_and(|c| n > c) {
                        break 'periods;
                    }
                    out.push(local);
                }
                k += 1;
            }
        }
        // An RDATE written as a date for a timed event: that day, at the
        // event's own time.
        out.extend(self.rdates.iter().map(|r| if r.date && !self.all_day { r.at + self.start.rem_euclid(DAY) } else { r.at }));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHANGHAI: i64 = 8 * HOUR;

    fn at(y: i64, m: i64, d: i64, h: i64, min: i64) -> i64 {
        days_from_civil(y, m, d) * DAY + h * HOUR + min * MINUTE
    }

    fn feed(body: &str) -> String {
        format!("BEGIN:VCALENDAR\nVERSION:2.0\nPRODID:test\n{body}\nEND:VCALENDAR\n")
    }

    // The local starts of every occurrence in [from, to) at +8.
    fn starts(text: &str, from: i64, to: i64) -> Vec<i64> {
        events(text, from, to, SHANGHAI, usize::MAX).iter().map(|e| e.start).collect()
    }

    // As Google Calendar writes a secret address's feed: its zones in full,
    // a weekly meeting with a day taken out and one moved by an hour, a
    // monthly review on the second Tuesday, a birthday each year, a call
    // stamped in UTC, a two-day offsite, a workshop with a DURATION, a
    // cancelled event, a dinner with a reminder (VALARM) and a long summary
    // folded, and a weekly call from New York.
    const GOOGLE: &str = "BEGIN:VCALENDAR
PRODID:-//Google Inc//Google Calendar 70.9054//EN
VERSION:2.0
CALSCALE:GREGORIAN
METHOD:PUBLISH
X-WR-CALNAME:Work
X-WR-TIMEZONE:Asia/Shanghai
BEGIN:VTIMEZONE
TZID:Asia/Shanghai
X-LIC-LOCATION:Asia/Shanghai
BEGIN:STANDARD
TZOFFSETFROM:+0800
TZOFFSETTO:+0800
TZNAME:CST
DTSTART:19700101T000000
END:STANDARD
END:VTIMEZONE
BEGIN:VTIMEZONE
TZID:America/New_York
X-LIC-LOCATION:America/New_York
BEGIN:DAYLIGHT
TZOFFSETFROM:-0500
TZOFFSETTO:-0400
TZNAME:EDT
DTSTART:19700308T020000
RRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=2SU
END:DAYLIGHT
BEGIN:STANDARD
TZOFFSETFROM:-0400
TZOFFSETTO:-0500
TZNAME:EST
DTSTART:19701101T020000
RRULE:FREQ=YEARLY;BYMONTH=11;BYDAY=1SU
END:STANDARD
END:VTIMEZONE
BEGIN:VEVENT
DTSTART;TZID=Asia/Shanghai:20261005T100000
DTEND;TZID=Asia/Shanghai:20261005T103000
RRULE:FREQ=WEEKLY;BYDAY=MO,WE,FR
EXDATE;TZID=Asia/Shanghai:20261111T100000
DTSTAMP:20261107T080000Z
UID:sync@google.com
CREATED:20260901T010203Z
DESCRIPTION:Agenda:\\nstatus\\, blockers
LAST-MODIFIED:20261101T000000Z
LOCATION:Room A
SEQUENCE:0
STATUS:CONFIRMED
SUMMARY:Team sync\\, Q4\\; room A
TRANSP:OPAQUE
END:VEVENT
BEGIN:VEVENT
DTSTART;TZID=Asia/Shanghai:20261113T110000
DTEND;TZID=Asia/Shanghai:20261113T113000
DTSTAMP:20261107T080000Z
UID:sync@google.com
RECURRENCE-ID;TZID=Asia/Shanghai:20261113T100000
SUMMARY:Team sync\\, Q4\\; room A (moved)
LOCATION:Room B
STATUS:CONFIRMED
END:VEVENT
BEGIN:VEVENT
DTSTART;TZID=Asia/Shanghai:20260113T150000
DTEND;TZID=Asia/Shanghai:20260113T160000
RRULE:FREQ=MONTHLY;BYDAY=2TU
UID:review@google.com
SUMMARY:Monthly review
END:VEVENT
BEGIN:VEVENT
DTSTART;VALUE=DATE:19901112
DTEND;VALUE=DATE:19901113
RRULE:FREQ=YEARLY
UID:birthday@google.com
SUMMARY:Mum's birthday
END:VEVENT
BEGIN:VEVENT
DTSTART:20261110T020000Z
DTEND:20261110T030000Z
UID:utc@google.com
SUMMARY:Vendor call
END:VEVENT
BEGIN:VEVENT
DTSTART;VALUE=DATE:20261114
DTEND;VALUE=DATE:20261116
UID:offsite@google.com
SUMMARY:Offsite
LOCATION:Hangzhou
END:VEVENT
BEGIN:VEVENT
DTSTART;TZID=Asia/Shanghai:20261112T140000
DURATION:PT1H30M
UID:workshop@google.com
SUMMARY:Workshop
END:VEVENT
BEGIN:VEVENT
DTSTART;TZID=Asia/Shanghai:20261112T090000
DTEND;TZID=Asia/Shanghai:20261112T093000
UID:cancelled@google.com
STATUS:CANCELLED
SUMMARY:Cancelled standup
END:VEVENT
BEGIN:VEVENT
DTSTART;TZID=Asia/Shanghai:20261109T180000
DTEND;TZID=Asia/Shanghai:20261109T190000
UID:dinner@google.com
SUMMARY:Dinner with the team\\, a long summary that Google folds at seventy-fi
 ve octets onto the next line
LOCATION:Downtown\\, 5th floor\\nBuilding 2
BEGIN:VALARM
ACTION:DISPLAY
DESCRIPTION:This is an event reminder
TRIGGER:-P0DT0H30M0S
END:VALARM
END:VEVENT
BEGIN:VEVENT
DTSTART;TZID=America/New_York:20261030T090000
DTEND;TZID=America/New_York:20261030T100000
RRULE:FREQ=WEEKLY;BYDAY=WE,FR
UID:ny@google.com
SUMMARY:US call
END:VEVENT
END:VCALENDAR
";

    // As Outlook publishes a calendar: Windows' zone names, a zone starting
    // in 1601, a language on the texts, and a zone known by name only.
    const OUTLOOK: &str = "BEGIN:VCALENDAR\r
METHOD:PUBLISH\r
PRODID:Microsoft Exchange Server 2010\r
VERSION:2.0\r
X-WR-CALNAME:Calendar\r
BEGIN:VTIMEZONE\r
TZID:China Standard Time\r
BEGIN:STANDARD\r
DTSTART:16010101T000000\r
TZOFFSETFROM:+0800\r
TZOFFSETTO:+0800\r
END:STANDARD\r
END:VTIMEZONE\r
BEGIN:VEVENT\r
DESCRIPTION:\\n\r
UID:040000008200E00074C5B7101A82E00800000000A0B1C2D3E4F5DC01000000000000000\r
 010000000ABCDEF0123456789ABCDEF0123456789\r
SUMMARY;LANGUAGE=zh-cn:周会\r
DTSTART;TZID=China Standard Time:20261109T140000\r
DTEND;TZID=China Standard Time:20261109T150000\r
CLASS:PUBLIC\r
PRIORITY:5\r
DTSTAMP:20261107T000000Z\r
TRANSP:OPAQUE\r
STATUS:CONFIRMED\r
SEQUENCE:0\r
LOCATION;LANGUAGE=zh-cn:3 楼会议室\r
X-MICROSOFT-CDO-APPT-SEQUENCE:0\r
X-MICROSOFT-CDO-BUSYSTATUS:BUSY\r
END:VEVENT\r
BEGIN:VEVENT\r
UID:tokyo\r
SUMMARY:Tokyo standup\r
DTSTART;TZID=Tokyo Standard Time:20261110T090000\r
DTEND;TZID=Tokyo Standard Time:20261110T093000\r
END:VEVENT\r
BEGIN:VEVENT\r
UID:mars\r
SUMMARY:Floating\r
DTSTART;TZID=Mars/Olympus:20261110T120000\r
DTEND;TZID=Mars/Olympus:20261110T130000\r
END:VEVENT\r
BEGIN:VEVENT\r
UID:payday\r
SUMMARY:Last weekday\r
DTSTART;TZID=\"(UTC+08:00) Beijing, Chongqing, Hong Kong, Urumqi\":20260130T170000\r
DTEND;TZID=\"(UTC+08:00) Beijing, Chongqing, Hong Kong, Urumqi\":20260130T173000\r
RRULE:FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1\r
END:VEVENT\r
END:VCALENDAR\r
";

    // The week of 2026-11-09 at +8, in order: the sync (Wednesday's taken
    // out, Friday's moved to 11:00 in room B), the dinner with its folded
    // summary and two-line location on one line, the UTC call at 10:00, the
    // review on the second Tuesday, the US call at 22:00 (New York on
    // standard time), the birthday and the offsite as whole days, the
    // workshop's hour and a half; not the cancelled one, nothing of the
    // reminder.
    #[test]
    fn a_google_feed_reads_as_the_week_looks() {
        let got = events(GOOGLE, at(2026, 11, 9, 0, 0), at(2026, 11, 16, 0, 0), SHANGHAI, 100);
        let want = vec![
            Event {
                summary: "Team sync, Q4; room A".into(),
                location: "Room A".into(),
                start: at(2026, 11, 9, 10, 0),
                end: at(2026, 11, 9, 10, 30),
                all_day: false,
            },
            Event {
                summary: "Dinner with the team, a long summary that Google folds at seventy-five octets onto the next line".into(),
                location: "Downtown, 5th floor Building 2".into(),
                start: at(2026, 11, 9, 18, 0),
                end: at(2026, 11, 9, 19, 0),
                all_day: false,
            },
            Event {
                summary: "Vendor call".into(),
                location: "".into(),
                start: at(2026, 11, 10, 10, 0),
                end: at(2026, 11, 10, 11, 0),
                all_day: false,
            },
            Event {
                summary: "Monthly review".into(),
                location: "".into(),
                start: at(2026, 11, 10, 15, 0),
                end: at(2026, 11, 10, 16, 0),
                all_day: false,
            },
            Event {
                summary: "US call".into(),
                location: "".into(),
                start: at(2026, 11, 11, 22, 0),
                end: at(2026, 11, 11, 23, 0),
                all_day: false,
            },
            Event {
                summary: "Mum's birthday".into(),
                location: "".into(),
                start: at(2026, 11, 12, 0, 0),
                end: at(2026, 11, 13, 0, 0),
                all_day: true,
            },
            Event {
                summary: "Workshop".into(),
                location: "".into(),
                start: at(2026, 11, 12, 14, 0),
                end: at(2026, 11, 12, 15, 30),
                all_day: false,
            },
            Event {
                summary: "Team sync, Q4; room A (moved)".into(),
                location: "Room B".into(),
                start: at(2026, 11, 13, 11, 0),
                end: at(2026, 11, 13, 11, 30),
                all_day: false,
            },
            Event {
                summary: "US call".into(),
                location: "".into(),
                start: at(2026, 11, 13, 22, 0),
                end: at(2026, 11, 13, 23, 0),
                all_day: false,
            },
            Event {
                summary: "Offsite".into(),
                location: "Hangzhou".into(),
                start: at(2026, 11, 14, 0, 0),
                end: at(2026, 11, 16, 0, 0),
                all_day: true,
            },
        ];
        assert_eq!(got, want);
        // The same with CRLF line ends, as the wire has them; and capped.
        assert_eq!(events(&GOOGLE.replace('\n', "\r\n"), at(2026, 11, 9, 0, 0), at(2026, 11, 16, 0, 0), SHANGHAI, 100), want);
        assert_eq!(events(GOOGLE, at(2026, 11, 9, 0, 0), at(2026, 11, 16, 0, 0), SHANGHAI, 3), want[..3].to_vec());
        // Another local zone: the UTC call at 03:00 London, the all-day ones unmoved.
        let london = events(GOOGLE, at(2026, 11, 9, 0, 0), at(2026, 11, 16, 0, 0), 0, 100);
        assert_eq!(london.iter().find(|e| e.summary == "Vendor call").map(|e| e.start), Some(at(2026, 11, 10, 2, 0)));
        assert_eq!(london.iter().find(|e| e.summary == "Mum's birthday").map(|e| (e.start, e.end)), Some((at(2026, 11, 12, 0, 0), at(2026, 11, 13, 0, 0))));
    }

    // New York's summer time ends on 2026-11-01: 09:00 there on Friday
    // October 30 is 21:00 in Shanghai, on Wednesday November 4 it is 22:00.
    #[test]
    fn a_zone_with_summer_time_shifts_with_it() {
        let got: Vec<i64> = events(GOOGLE, at(2026, 10, 29, 0, 0), at(2026, 11, 7, 0, 0), SHANGHAI, 100).into_iter().filter(|e| e.summary == "US call").map(|e| e.start).collect();
        assert_eq!(got, vec![at(2026, 10, 30, 21, 0), at(2026, 11, 4, 22, 0), at(2026, 11, 6, 22, 0)]);
        // Spring: the second Sunday of March 2026 is the 8th.
        let text = feed("BEGIN:VTIMEZONE\nTZID:America/New_York\nBEGIN:DAYLIGHT\nTZOFFSETFROM:-0500\nTZOFFSETTO:-0400\nDTSTART:19700308T020000\nRRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=2SU\nEND:DAYLIGHT\nBEGIN:STANDARD\nTZOFFSETFROM:-0400\nTZOFFSETTO:-0500\nDTSTART:19701101T020000\nRRULE:FREQ=YEARLY;BYMONTH=11;BYDAY=1SU\nEND:STANDARD\nEND:VTIMEZONE\nBEGIN:VEVENT\nDTSTART;TZID=\"America/New_York\":20260306T090000\nRRULE:FREQ=DAILY;COUNT=4\nSUMMARY:Daily\nEND:VEVENT");
        assert_eq!(starts(&text, at(2026, 3, 1, 0, 0), at(2026, 3, 20, 0, 0)), vec![at(2026, 3, 6, 22, 0), at(2026, 3, 7, 22, 0), at(2026, 3, 8, 21, 0), at(2026, 3, 9, 21, 0)]);
    }

    #[test]
    fn an_outlook_feed_reads_by_windows_zone_names() {
        let got = events(OUTLOOK, at(2026, 11, 9, 0, 0), at(2026, 11, 16, 0, 0), SHANGHAI, 100);
        assert_eq!(
            got,
            vec![
                Event {
                    summary: "周会".into(),
                    location: "3 楼会议室".into(),
                    start: at(2026, 11, 9, 14, 0),
                    end: at(2026, 11, 9, 15, 0),
                    all_day: false
                },
                // Tokyo, known by name alone: an hour ahead.
                Event {
                    summary: "Tokyo standup".into(),
                    location: "".into(),
                    start: at(2026, 11, 10, 8, 0),
                    end: at(2026, 11, 10, 8, 30),
                    all_day: false
                },
                // A zone nobody knows: as written.
                Event {
                    summary: "Floating".into(),
                    location: "".into(),
                    start: at(2026, 11, 10, 12, 0),
                    end: at(2026, 11, 10, 13, 0),
                    all_day: false
                },
            ]
        );
        // The last weekday of November 2026 is Monday the 30th (BYSETPOS=-1,
        // the zone's offset read from its name).
        assert_eq!(starts(OUTLOOK, at(2026, 11, 23, 0, 0), at(2026, 12, 1, 0, 0)), vec![at(2026, 11, 30, 17, 0)]);
    }

    #[test]
    fn count_and_until_end_a_series() {
        let three = feed("BEGIN:VEVENT\nDTSTART:20261109T090000\nRRULE:FREQ=DAILY;COUNT=3\nSUMMARY:x\nEND:VEVENT");
        assert_eq!(starts(&three, at(2026, 11, 9, 0, 0), at(2026, 11, 16, 0, 0)), vec![at(2026, 11, 9, 9, 0), at(2026, 11, 10, 9, 0), at(2026, 11, 11, 9, 0)]);
        // UNTIL in UTC, inclusive: 10:00 at +8 is 02:00Z.
        let until = |u: &str| feed(&format!("BEGIN:VEVENT\nDTSTART;TZID=Asia/Shanghai:20261109T100000\nRRULE:FREQ=DAILY;UNTIL={u}\nSUMMARY:x\nEND:VEVENT"));
        assert_eq!(starts(&until("20261111T015959Z"), at(2026, 11, 9, 0, 0), at(2026, 11, 16, 0, 0)), vec![at(2026, 11, 9, 10, 0), at(2026, 11, 10, 10, 0)]);
        assert_eq!(starts(&until("20261111T020000Z"), at(2026, 11, 9, 0, 0), at(2026, 11, 16, 0, 0)), vec![at(2026, 11, 9, 10, 0), at(2026, 11, 10, 10, 0), at(2026, 11, 11, 10, 0)]);
        // A date: the whole of that day, for an all-day series or a timed one.
        let days = feed("BEGIN:VEVENT\nDTSTART;VALUE=DATE:20261109\nRRULE:FREQ=DAILY;UNTIL=20261110\nSUMMARY:x\nEND:VEVENT");
        assert_eq!(starts(&days, at(2026, 11, 9, 0, 0), at(2026, 11, 16, 0, 0)), vec![at(2026, 11, 9, 0, 0), at(2026, 11, 10, 0, 0)]);
        assert_eq!(starts(&until("20261110"), at(2026, 11, 9, 0, 0), at(2026, 11, 16, 0, 0)), vec![at(2026, 11, 9, 10, 0), at(2026, 11, 10, 10, 0)]);
        // A COUNT is counted from the start, however old: the 200th week.
        let old = feed("BEGIN:VEVENT\nDTSTART:20230102T090000\nRRULE:FREQ=WEEKLY;COUNT=200\nSUMMARY:x\nEND:VEVENT");
        let last = at(2023, 1, 2, 9, 0) + 199 * 7 * DAY;
        assert_eq!(starts(&old, last - DAY, last + 8 * DAY), vec![last]);
        assert_eq!(starts(&old, at(2026, 11, 9, 0, 0), at(2026, 11, 16, 0, 0)), Vec::<i64>::new());
    }

    #[test]
    fn weekly_meetings_fall_on_their_days() {
        // Every other week from Monday November 2: not the 9th, the 16th.
        let fortnightly = feed("BEGIN:VEVENT\nDTSTART:20261102T090000\nRRULE:FREQ=WEEKLY;INTERVAL=2\nSUMMARY:x\nEND:VEVENT");
        assert_eq!(starts(&fortnightly, at(2026, 11, 9, 0, 0), at(2026, 11, 23, 0, 0)), vec![at(2026, 11, 16, 9, 0)]);
        // RFC 5545's example: which week a Sunday belongs to turns on WKST.
        let wkst = |w: &str| feed(&format!("BEGIN:VEVENT\nDTSTART:19970805T090000\nRRULE:FREQ=WEEKLY;INTERVAL=2;COUNT=4;BYDAY=TU,SU;WKST={w}\nSUMMARY:x\nEND:VEVENT"));
        assert_eq!(starts(&wkst("MO"), at(1997, 8, 1, 0, 0), at(1997, 9, 30, 0, 0)), vec![at(1997, 8, 5, 9, 0), at(1997, 8, 10, 9, 0), at(1997, 8, 19, 9, 0), at(1997, 8, 24, 9, 0)]);
        assert_eq!(starts(&wkst("SU"), at(1997, 8, 1, 0, 0), at(1997, 9, 30, 0, 0)), vec![at(1997, 8, 5, 9, 0), at(1997, 8, 17, 9, 0), at(1997, 8, 19, 9, 0), at(1997, 8, 31, 9, 0)]);
        // Weekdays only, as a DAILY rule with BYDAY.
        let weekdays = feed("BEGIN:VEVENT\nDTSTART:20261109T090000\nRRULE:FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR\nSUMMARY:x\nEND:VEVENT");
        assert_eq!(starts(&weekdays, at(2026, 11, 12, 0, 0), at(2026, 11, 17, 0, 0)), vec![at(2026, 11, 12, 9, 0), at(2026, 11, 13, 9, 0), at(2026, 11, 16, 9, 0)]);
    }

    #[test]
    fn monthly_and_yearly_rules_pick_their_days() {
        let month = (at(2026, 11, 1, 0, 0), at(2026, 12, 1, 0, 0));
        let first_and_fifteenth = feed("BEGIN:VEVENT\nDTSTART:20260101T090000\nRRULE:FREQ=MONTHLY;BYMONTHDAY=1,15\nSUMMARY:x\nEND:VEVENT");
        assert_eq!(starts(&first_and_fifteenth, month.0, month.1), vec![at(2026, 11, 1, 9, 0), at(2026, 11, 15, 9, 0)]);
        let last_friday = feed("BEGIN:VEVENT\nDTSTART:20260130T090000\nRRULE:FREQ=MONTHLY;BYDAY=-1FR\nSUMMARY:x\nEND:VEVENT");
        assert_eq!(starts(&last_friday, month.0, month.1), vec![at(2026, 11, 27, 9, 0)]);
        let every_other_month_end = feed("BEGIN:VEVENT\nDTSTART:20260131T090000\nRRULE:FREQ=MONTHLY;INTERVAL=2;BYMONTHDAY=-1\nSUMMARY:x\nEND:VEVENT");
        assert_eq!(starts(&every_other_month_end, month.0, month.1), vec![at(2026, 11, 30, 9, 0)]);
        // The 31st: skipped in months without one.
        let thirty_first = feed("BEGIN:VEVENT\nDTSTART:20260131T090000\nRRULE:FREQ=MONTHLY\nSUMMARY:x\nEND:VEVENT");
        assert_eq!(starts(&thirty_first, month.0, at(2027, 1, 1, 0, 0)), vec![at(2026, 12, 31, 9, 0)]);
        // Thanksgiving: the fourth Thursday of November.
        let thanksgiving = feed("BEGIN:VEVENT\nDTSTART;VALUE=DATE:20251127\nRRULE:FREQ=YEARLY;BYMONTH=11;BYDAY=4TH\nSUMMARY:x\nEND:VEVENT");
        assert_eq!(starts(&thanksgiving, month.0, month.1), vec![at(2026, 11, 26, 0, 0)]);
        // Quarter days: every year, in four months.
        let quarters = feed("BEGIN:VEVENT\nDTSTART:20200101T090000\nRRULE:FREQ=YEARLY;BYMONTH=1,4,7,10\nSUMMARY:x\nEND:VEVENT");
        assert_eq!(starts(&quarters, at(2026, 1, 1, 0, 0), at(2027, 1, 1, 0, 0)), vec![at(2026, 1, 1, 9, 0), at(2026, 4, 1, 9, 0), at(2026, 7, 1, 9, 0), at(2026, 10, 1, 9, 0)]);
    }

    #[test]
    fn dates_taken_out_and_put_in() {
        let weekly = "BEGIN:VEVENT\nUID:w\nDTSTART;TZID=Asia/Shanghai:20261005T100000\nDTEND;TZID=Asia/Shanghai:20261005T103000\nRRULE:FREQ=WEEKLY;BYDAY=MO,WE,FR\nSUMMARY:x\n";
        let week = (at(2026, 11, 9, 0, 0), at(2026, 11, 16, 0, 0));
        // EXDATE as a date, several on a line, on two lines; an RDATE on a Saturday.
        let text = feed(&format!("{weekly}EXDATE;VALUE=DATE:20261111\nEXDATE;TZID=Asia/Shanghai:20261113T100000,20261120T100000\nEXDATE:20261109T020000Z\nRDATE;TZID=Asia/Shanghai:20261114T100000\nEND:VEVENT"));
        assert_eq!(
            events(&text, week.0, week.1, SHANGHAI, 100),
            vec![Event {
                summary: "x".into(),
                location: "".into(),
                start: at(2026, 11, 14, 10, 0),
                end: at(2026, 11, 14, 10, 30),
                all_day: false
            }]
        );
        // An instance cancelled on its own goes; one moved out of the week
        // goes from it, one moved into it comes.
        let text = feed(&format!("{weekly}END:VEVENT\nBEGIN:VEVENT\nUID:w\nRECURRENCE-ID;TZID=Asia/Shanghai:20261109T100000\nDTSTART;TZID=Asia/Shanghai:20261109T100000\nDTEND;TZID=Asia/Shanghai:20261109T103000\nSTATUS:CANCELLED\nSUMMARY:x\nEND:VEVENT\nBEGIN:VEVENT\nUID:w\nRECURRENCE-ID;TZID=Asia/Shanghai:20261111T100000\nDTSTART;TZID=Asia/Shanghai:20261118T100000\nDTEND;TZID=Asia/Shanghai:20261118T103000\nSUMMARY:x\nEND:VEVENT\nBEGIN:VEVENT\nUID:w\nRECURRENCE-ID;TZID=Asia/Shanghai:20261120T100000\nDTSTART;TZID=Asia/Shanghai:20261112T100000\nDTEND;TZID=Asia/Shanghai:20261112T103000\nSUMMARY:x\nEND:VEVENT"));
        assert_eq!(starts(&text, week.0, week.1), vec![at(2026, 11, 12, 10, 0), at(2026, 11, 13, 10, 0)]);
        // A whole series cancelled.
        let text = feed(&format!("{weekly}STATUS:CANCELLED\nEND:VEVENT"));
        assert_eq!(starts(&text, week.0, week.1), Vec::<i64>::new());
        // An exception whose series is not in the file stands on its own.
        let text = feed("BEGIN:VEVENT\nUID:lone\nRECURRENCE-ID:20261109T100000\nDTSTART:20261110T100000\nSUMMARY:x\nEND:VEVENT");
        assert_eq!(starts(&text, week.0, week.1), vec![at(2026, 11, 10, 10, 0)]);
    }

    #[test]
    fn lengths_and_whole_days() {
        let week = (at(2026, 11, 9, 0, 0), at(2026, 11, 16, 0, 0));
        let one = |body: &str| events(&feed(&format!("BEGIN:VEVENT\n{body}\nSUMMARY:x\nEND:VEVENT")), week.0, week.1, SHANGHAI, 10);
        // No end: as long as a moment; a whole day without one: the day.
        assert_eq!(one("DTSTART:20261110T100000").iter().map(|e| (e.start, e.end, e.all_day)).collect::<Vec<_>>(), vec![(at(2026, 11, 10, 10, 0), at(2026, 11, 10, 10, 0), false)]);
        assert_eq!(one("DTSTART;VALUE=DATE:20261110").iter().map(|e| (e.start, e.end, e.all_day)).collect::<Vec<_>>(), vec![(at(2026, 11, 10, 0, 0), at(2026, 11, 11, 0, 0), true)]);
        // DTEND the same date as DTSTART (some write a day so): still the day.
        assert_eq!(one("DTSTART;VALUE=DATE:20261110\nDTEND;VALUE=DATE:20261110").iter().map(|e| e.end).collect::<Vec<_>>(), vec![at(2026, 11, 11, 0, 0)]);
        // Durations.
        assert_eq!(one("DTSTART:20261110T100000\nDURATION:P1DT2H").iter().map(|e| e.end).collect::<Vec<_>>(), vec![at(2026, 11, 11, 12, 0)]);
        assert_eq!(one("DTSTART:20261110T100000\nDURATION:PT45M").iter().map(|e| e.end).collect::<Vec<_>>(), vec![at(2026, 11, 10, 10, 45)]);
        assert_eq!(one("DTSTART;VALUE=DATE:20261110\nDURATION:P2D").iter().map(|e| e.end).collect::<Vec<_>>(), vec![at(2026, 11, 12, 0, 0)]);
        assert_eq!(one("DTSTART:20261110T100000\nDURATION:-PT1H").iter().map(|e| e.end).collect::<Vec<_>>(), vec![at(2026, 11, 10, 10, 0)]);
        assert_eq!((duration("P1W"), duration("PT1H30M"), duration("P"), duration("PT1X"), duration("1H")), (Some(7 * DAY), Some(HOUR + 30 * MINUTE), Some(0), None, None));
        // An event under way as the span opens is in it; one ending as it
        // opens, or starting as it closes, is not.
        let long = feed("BEGIN:VEVENT\nDTSTART:20261108T220000\nDTEND:20261109T020000\nSUMMARY:x\nEND:VEVENT");
        assert_eq!(starts(&long, week.0, week.1), vec![at(2026, 11, 8, 22, 0)]);
        let edges = feed("BEGIN:VEVENT\nDTSTART:20261108T220000\nDTEND:20261109T000000\nSUMMARY:x\nEND:VEVENT\nBEGIN:VEVENT\nDTSTART:20261116T000000\nSUMMARY:y\nEND:VEVENT");
        assert_eq!(starts(&edges, week.0, week.1), Vec::<i64>::new());
        // A moment at the span's very start is in it.
        let moment = feed("BEGIN:VEVENT\nDTSTART:20261109T000000\nSUMMARY:x\nEND:VEVENT");
        assert_eq!(starts(&moment, week.0, week.1), vec![at(2026, 11, 9, 0, 0)]);
    }

    // A series from 1970 reaches the week asked for in a leap, where day by
    // day the 5000 steps would run out in 1983.
    #[test]
    fn old_series_leap_to_the_span_asked_for() {
        let week = (at(2026, 11, 9, 0, 0), at(2026, 11, 16, 0, 0));
        let daily = feed("BEGIN:VEVENT\nDTSTART:19700105T090000\nRRULE:FREQ=DAILY\nSUMMARY:x\nEND:VEVENT");
        assert_eq!(starts(&daily, week.0, week.1), (9..16).map(|d| at(2026, 11, d, 9, 0)).collect::<Vec<_>>());
        let weekly = feed("BEGIN:VEVENT\nDTSTART:19700105T090000\nRRULE:FREQ=WEEKLY;BYDAY=MO,TU\nSUMMARY:x\nEND:VEVENT");
        assert_eq!(starts(&weekly, week.0, week.1), vec![at(2026, 11, 9, 9, 0), at(2026, 11, 10, 9, 0)]);
        let monthly = feed("BEGIN:VEVENT\nDTSTART:19700112T090000\nRRULE:FREQ=MONTHLY;BYDAY=2TH\nSUMMARY:x\nEND:VEVENT");
        assert_eq!(starts(&monthly, week.0, week.1), vec![at(2026, 11, 12, 9, 0)]);
        let yearly = feed("BEGIN:VEVENT\nDTSTART;VALUE=DATE:19701111\nRRULE:FREQ=YEARLY\nSUMMARY:x\nEND:VEVENT");
        assert_eq!(starts(&yearly, week.0, week.1), vec![at(2026, 11, 11, 0, 0)]);
        // A rule that never names a day gives up without hanging.
        let never = feed("BEGIN:VEVENT\nDTSTART:20260131T090000\nRRULE:FREQ=MONTHLY;BYMONTH=2;BYMONTHDAY=31\nSUMMARY:x\nEND:VEVENT");
        assert_eq!(starts(&never, week.0, week.1), Vec::<i64>::new());
    }

    #[test]
    fn a_long_span_is_cut_to_four_hundred_days() {
        let daily = feed("BEGIN:VEVENT\nDTSTART:20260101T090000\nRRULE:FREQ=DAILY\nSUMMARY:x\nEND:VEVENT");
        let got = starts(&daily, at(2026, 1, 1, 0, 0), at(2029, 1, 1, 0, 0));
        assert_eq!(got.len(), 400);
        assert_eq!(got.last().copied(), Some(at(2027, 2, 4, 9, 0)));
        // Nothing where nothing is (before the feed's first start; its
        // open-ended series go on into 2030); nothing of a span turned
        // round, or empty.
        assert_eq!(starts(GOOGLE, at(1980, 1, 1, 0, 0), at(1980, 1, 8, 0, 0)), Vec::<i64>::new());
        assert_eq!(starts(GOOGLE, at(2030, 1, 1, 0, 0), at(2030, 1, 8, 0, 0)).len(), 5);
        assert_eq!(starts(GOOGLE, at(2026, 11, 16, 0, 0), at(2026, 11, 9, 0, 0)), Vec::<i64>::new());
        assert_eq!(events(GOOGLE, at(2026, 11, 9, 0, 0), at(2026, 11, 16, 0, 0), SHANGHAI, 0), Vec::new());
        assert_eq!(starts(GOOGLE, i64::MIN, i64::MAX).len(), starts(GOOGLE, 0, 400 * DAY).len());
    }

    // What is not read gives the first instance alone; what is not an
    // event, or lies outside 1970–2100, gives nothing; and none of it panics.
    #[test]
    fn what_cannot_be_read_is_left_out_quietly() {
        let week = (at(2026, 11, 9, 0, 0), at(2026, 11, 16, 0, 0));
        for rule in ["FREQ=HOURLY;INTERVAL=2", "FREQ=YEARLY;BYDAY=20MO", "FREQ=WEEKLY;BYDAY=XX", "FREQ=DAILY;BYHOUR=9,10", "FREQ=DAILY;INTERVAL=0", "FREQ=DAILY;COUNT=0", "FREQ=DAILY;UNTIL=soon", "garbage", "FREQ=MONTHLY;BYSETPOS=0", "FREQ=DAILY;INTERVAL=5000"] {
            let text = feed(&format!("BEGIN:VEVENT\nDTSTART:20261109T090000\nRRULE:{rule}\nSUMMARY:x\nEND:VEVENT"));
            assert_eq!(starts(&text, week.0, week.1), vec![at(2026, 11, 9, 9, 0)], "{rule}");
        }
        for text in [
            "",
            "hello world",
            ":::\n;;;\n\"\n",
            "BEGIN:VEVENT\nDTSTART:nope\nSUMMARY:x\nEND:VEVENT",
            "BEGIN:VEVENT\nSUMMARY:no start\nEND:VEVENT",
            "END:VCALENDAR\nEND:VEVENT\nBEGIN:VEVENT\nSUMMARY:x",
            "BEGIN:VEVENT\nDTSTART:19000101T000000\nSUMMARY:old\nEND:VEVENT",
            "BEGIN:VEVENT\nDTSTART:21010101\nSUMMARY:far\nEND:VEVENT",
            "BEGIN:VEVENT\nDTSTART:20261301T000000\nEND:VEVENT",
            "BEGIN:VEVENT\nDTSTART:20261109T250000\nEND:VEVENT",
            "BEGIN:VEVENT\nDTSTART:20260230\nEND:VEVENT",
            "BEGIN:VEVENT\nDTSTART;TZID=\"unterminated:20261109T090000\nEND:VEVENT",
            "BEGIN:VEVENT\nDTSTART:99999999999999999999T000000\nEND:VEVENT",
        ] {
            assert_eq!(starts(text, week.0, week.1), Vec::<i64>::new(), "{text:?}");
        }
        // A duration too long to count is no duration.
        let text = feed("BEGIN:VEVENT\nDTSTART:20261109T090000\nDURATION:P99999999999999999999D\nSUMMARY:x\nEND:VEVENT");
        assert_eq!(events(&text, week.0, week.1, SHANGHAI, 10)[0].end, at(2026, 11, 9, 9, 0));
        // A feed cut short still gives what it had; an event outside any
        // VCALENDAR counts too.
        let cut = &GOOGLE[..GOOGLE.find("LOCATION:Downtown").unwrap_or(0)];
        assert!(events(cut, week.0, week.1, SHANGHAI, 100).iter().any(|e| e.summary.starts_with("Dinner")));
        assert_eq!(starts("BEGIN:VEVENT\nDTSTART:20261110T100000\nEND:VEVENT", week.0, week.1), vec![at(2026, 11, 10, 10, 0)]);
    }

    #[test]
    fn texts_are_unescaped_and_zones_known_by_name() {
        assert_eq!(text("Team sync\\, Q4\\; room A"), "Team sync, Q4; room A");
        assert_eq!(text("  a\\nb\\Nc  \\\\ d\\"), "a b c \\ d\\");
        assert_eq!(text(""), "");
        for (name, offset) in [
            ("Asia/Shanghai", Some(8 * HOUR)),
            ("asia/tokyo", Some(9 * HOUR)),
            ("Asia/Kolkata", Some(5 * HOUR + 30 * MINUTE)),
            ("UTC", Some(0)),
            ("Etc/UTC", Some(0)),
            ("China Standard Time", Some(8 * HOUR)),
            ("/freeassociation.sourceforge.net/Tzfile/Asia/Shanghai", Some(8 * HOUR)),
            ("(UTC+08:00) Beijing, Chongqing, Hong Kong, Urumqi", Some(8 * HOUR)),
            ("(UTC-05:00) Eastern Time (US & Canada)", Some(-5 * HOUR)),
            ("(UTC) Coordinated Universal Time", Some(0)),
            ("America/New_York", None),
            ("Europe/London", None),
            ("", None),
        ] {
            assert_eq!(known_offset(name), offset, "{name}");
        }
        assert_eq!(
            (offset_written("+0800"), offset_written("-0500"), offset_written("+0530"), offset_written("+053000"), offset_written("+08:00"), offset_written("0800"), offset_written("+8")),
            (Some(8 * HOUR), Some(-5 * HOUR), Some(5 * HOUR + 30 * MINUTE), Some(5 * HOUR + 30 * MINUTE), Some(8 * HOUR), Some(8 * HOUR), None)
        );
    }

    #[test]
    fn lines_unfold_and_split_as_written() {
        // One space or tab taken off a continuation, the rest kept.
        let lines = unfold("\u{feff}BEGIN:VCALENDAR\r\nSUMMARY:a long\r\n  line\r\n\tand a tab\r\n\r\nEND:VCALENDAR");
        assert_eq!(lines, vec!["BEGIN:VCALENDAR", "SUMMARY:a long lineand a tab", "END:VCALENDAR"]);
        let p = prop("DTSTART;TZID=\"America/New_York\";VALUE=DATE-TIME:20261030T090000").expect("a property");
        assert_eq!((p.name.as_str(), p.param("TZID"), p.param("VALUE"), p.value.as_str()), ("DTSTART", Some("America/New_York"), Some("DATE-TIME"), "20261030T090000"));
        let p = prop("summary;language=en:Meeting: at 10").expect("a property");
        assert_eq!((p.name.as_str(), p.param("LANGUAGE"), p.value.as_str()), ("SUMMARY", Some("en"), "Meeting: at 10"));
        assert!(prop("no colon here").is_none() && prop(":value").is_none());
    }

    #[test]
    fn days_and_dates() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(civil_from_days(days_from_civil(2026, 11, 9)), (2026, 11, 9));
        assert_eq!(weekday_of(days_from_civil(2026, 11, 1)), 0);
        assert_eq!(weekday_of(0), 4);
        assert_eq!(week_start(days_from_civil(2026, 11, 8), 1), days_from_civil(2026, 11, 2));
        assert_eq!(week_start(days_from_civil(2026, 11, 8), 0), days_from_civil(2026, 11, 8));
        assert_eq!(nth_weekday(2026, 11, 2, 2), Some(days_from_civil(2026, 11, 10)));
        assert_eq!(nth_weekday(2026, 11, -1, 5), Some(days_from_civil(2026, 11, 27)));
        assert_eq!(nth_weekday(2026, 11, 5, 1), Some(days_from_civil(2026, 11, 30)));
        assert_eq!(nth_weekday(2026, 11, 5, 2), None);
        let w = written("20261107T100000Z").expect("a stamp");
        assert_eq!((w.days, w.ms, w.date, w.utc), (days_from_civil(2026, 11, 7), 10 * HOUR, false, true));
        let w = written("20261107").expect("a date");
        assert!(w.date && !w.utc && w.ms == 0);
        assert!(written("20261107Z").is_none() && written("2026-11-07").is_none() && written("20261107T1000").is_none());
    }
}
