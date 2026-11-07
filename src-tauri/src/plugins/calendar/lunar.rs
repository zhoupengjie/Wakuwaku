// The Chinese calendar (农历) for the calendar plugin: the lunar date of a
// day, its year's stem, branch and animal (丙午马年), its festivals (春节,
// 中秋, 除夕…), the solar terms (节气: 立冬 falls on 2026-11-07), and the
// solar calendar's festivals in each language.
//
// The months come from a table, one number a year for 1901 to 2100, taken
// from .NET's ChineseLunisolarCalendar (the tests check it against dates
// .NET gives): bits 15 to 4 say whether each of the twelve months has 30
// days (else 29), bit 16 whether the leap month has, bits 3 to 0 which
// month a leap month follows (0: none). The layout the widely used
// JavaScript lunar tables have, so a number can be compared with theirs.
// The solar terms by the usual formula for 2001 to 2100 (Y the year's last
// two digits: ⌊Y × 0.2422 + C⌋ − ⌊Y / 4⌋, the January terms with
// ⌊(Y − 1) / 4⌋), with the few years it is known to miss by a day.

const FIRST: i64 = 1901;
// 1901-02-19 is 正月初一 of 1901, where the table starts.
const FIRST_DAYS: i64 = -25_153;

#[rustfmt::skip]
const TABLE: [u32; 200] = [
    0x04ae0, 0x0a570, 0x054d5, 0x0d260, 0x0d950, 0x16554, 0x056a0, 0x09ad0, 0x055d2, 0x04ae0, 0x0a5b6, 0x0a4d0, 0x0d250, 0x1d255, 0x0b540, 0x0d6a0, 0x0ada2, 0x095b0, 0x14977, 0x04970,
    0x0a4b0, 0x0b4b5, 0x06a50, 0x06d40, 0x1ab54, 0x02b60, 0x09570, 0x052f2, 0x04970, 0x06566, 0x0d4a0, 0x0ea50, 0x16a95, 0x05ad0, 0x02b60, 0x186e3, 0x092e0, 0x1c8d7, 0x0c950, 0x0d4a0,
    0x1d8a6, 0x0b550, 0x056a0, 0x1a5b4, 0x025d0, 0x092d0, 0x0d2b2, 0x0a950, 0x0b557, 0x06ca0, 0x0b550, 0x15355, 0x04da0, 0x0a5b0, 0x14573, 0x052b0, 0x0a9a8, 0x0e950, 0x06aa0, 0x0aea6,
    0x0ab50, 0x04b60, 0x0aae4, 0x0a570, 0x05260, 0x0f263, 0x0d950, 0x05b57, 0x056a0, 0x096d0, 0x04dd5, 0x04ad0, 0x0a4d0, 0x0d4d4, 0x0d250, 0x0d558, 0x0b540, 0x0b6a0, 0x195a6, 0x095b0,
    0x049b0, 0x0a974, 0x0a4b0, 0x0b27a, 0x06a50, 0x06d40, 0x0af46, 0x0ab60, 0x09570, 0x04af5, 0x04970, 0x064b0, 0x074a3, 0x0ea50, 0x06b58, 0x05ac0, 0x0ab60, 0x096d5, 0x092e0, 0x0c960,
    0x0d954, 0x0d4a0, 0x0da50, 0x07552, 0x056a0, 0x0abb7, 0x025d0, 0x092d0, 0x0cab5, 0x0a950, 0x0b4a0, 0x0baa4, 0x0ad50, 0x055d9, 0x04ba0, 0x0a5b0, 0x15176, 0x052b0, 0x0a930, 0x07954,
    0x06aa0, 0x0ad50, 0x05b52, 0x04b60, 0x0a6e6, 0x0a4e0, 0x0d260, 0x0ea65, 0x0d530, 0x05aa0, 0x076a3, 0x096d0, 0x04afb, 0x04ad0, 0x0a4d0, 0x1d0b6, 0x0d250, 0x0d520, 0x0dd45, 0x0b5a0,
    0x056d0, 0x055b2, 0x049b0, 0x0a577, 0x0a4b0, 0x0aa50, 0x1b255, 0x06d20, 0x0ada0, 0x14b63, 0x09370, 0x049f8, 0x04970, 0x064b0, 0x168a6, 0x0ea50, 0x06b20, 0x1a6c4, 0x0aae0, 0x092e0,
    0x0d2e3, 0x0c960, 0x0d557, 0x0d4a0, 0x0da50, 0x05d55, 0x056a0, 0x0a6d0, 0x055d4, 0x052d0, 0x0a9b8, 0x0a950, 0x0b4a0, 0x0b6a6, 0x0ad50, 0x055a0, 0x0aba4, 0x0a5b0, 0x052b0, 0x0b273,
    0x06930, 0x07337, 0x06aa0, 0x0ad50, 0x14b55, 0x04b60, 0x0a570, 0x054e4, 0x0d160, 0x0e968, 0x0d520, 0x0daa0, 0x16aa6, 0x056d0, 0x04ae0, 0x0a9d4, 0x0a2d0, 0x0d150, 0x0f252, 0x0d520,
];

const STEMS: [char; 10] = ['甲', '乙', '丙', '丁', '戊', '己', '庚', '辛', '壬', '癸'];
const BRANCHES: [char; 12] = ['子', '丑', '寅', '卯', '辰', '巳', '午', '未', '申', '酉', '戌', '亥'];
const ANIMALS: [char; 12] = ['鼠', '牛', '虎', '兔', '龙', '蛇', '马', '羊', '猴', '鸡', '狗', '猪'];
const MONTHS: [&str; 12] = ["正", "二", "三", "四", "五", "六", "七", "八", "九", "十", "冬", "腊"];
const DAYS: [&str; 30] = [
    "初一", "初二", "初三", "初四", "初五", "初六", "初七", "初八", "初九", "初十", "十一", "十二", "十三", "十四", "十五", "十六", "十七", "十八", "十九", "二十", "廿一", "廿二", "廿三", "廿四", "廿五", "廿六", "廿七", "廿八", "廿九",
    "三十",
];

// A day on the lunar calendar: its year (the one its 正月 began), month (1
// to 12, and whether the leap one after it), day (1 to 30).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lunar {
    pub year: i64,
    pub month: u8,
    pub leap: bool,
    pub day: u8,
}

fn entry(year: i64) -> Option<u32> {
    usize::try_from(year - FIRST).ok().and_then(|i| TABLE.get(i)).copied()
}

// Which month a leap month follows that year; 0 for none.
fn leap_month(year: i64) -> u8 {
    entry(year).map_or(0, |e| (e & 0xf) as u8)
}

fn leap_days(year: i64) -> i64 {
    match (leap_month(year), entry(year)) {
        (0, _) | (_, None) => 0,
        (_, Some(e)) if e & 0x10000 != 0 => 30,
        _ => 29,
    }
}

// The days of a month (1 to 12) that year.
fn month_days(year: i64, month: u8) -> i64 {
    match entry(year) {
        Some(e) if e & (0x10000 >> month) != 0 => 30,
        _ => 29,
    }
}

fn year_days(year: i64) -> i64 {
    (1..=12).map(|m| month_days(year, m)).sum::<i64>() + leap_days(year)
}

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

// The lunar date of a day of the solar calendar; None before 1901-02-19
// and after the table's last day (2101-01-28).
pub fn of(y: i64, m: i64, d: i64) -> Option<Lunar> {
    let mut left = days_from_civil(y, m, d) - FIRST_DAYS;
    if left < 0 {
        return None;
    }
    let mut year = FIRST;
    while year < FIRST + TABLE.len() as i64 && left >= year_days(year) {
        left -= year_days(year);
        year += 1;
    }
    entry(year)?;
    for month in 1..=12u8 {
        let days = month_days(year, month);
        if left < days {
            return Some(Lunar { year, month, leap: false, day: left as u8 + 1 });
        }
        left -= days;
        if leap_month(year) == month {
            let days = leap_days(year);
            if left < days {
                return Some(Lunar { year, month, leap: true, day: left as u8 + 1 });
            }
            left -= days;
        }
    }
    None
}

// The date in words: 九月廿九, 闰六月初一, 正月初一, 腊月三十.
pub fn words(l: &Lunar) -> String {
    let leap = if l.leap { "闰" } else { "" };
    format!("{leap}{}月{}", MONTHS[usize::from(l.month.clamp(1, 12)) - 1], DAYS[usize::from(l.day.clamp(1, 30)) - 1])
}

// The year's stem and branch, and its animal: 丙午马年.
pub fn year_words(year: i64) -> String {
    let n = (year - 4).rem_euclid(60) as usize;
    format!("{}{}{}年", STEMS[n % 10], BRANCHES[n % 12], ANIMALS[n % 12])
}

// A festival of the lunar calendar on that day, if any (none in a leap month).
pub fn festival(l: &Lunar) -> Option<&'static str> {
    if l.leap {
        return None;
    }
    Some(match (l.month, l.day) {
        (1, 1) => "春节",
        (1, 15) => "元宵",
        (2, 2) => "龙抬头",
        (5, 5) => "端午",
        (7, 7) => "七夕",
        (7, 15) => "中元",
        (8, 15) => "中秋",
        (9, 9) => "重阳",
        (12, 8) => "腊八",
        (12, 23) => "小年",
        (12, d) if i64::from(d) == month_days(l.year, 12) => "除夕",
        _ => return None,
    })
}

// --- The solar terms (节气) ---------------------------------------------------------------
const TERMS: [&str; 24] = ["小寒", "大寒", "立春", "雨水", "惊蛰", "春分", "清明", "谷雨", "立夏", "小满", "芒种", "夏至", "小暑", "大暑", "立秋", "处暑", "白露", "秋分", "寒露", "霜降", "立冬", "小雪", "大雪", "冬至"];
// Each term's C for 2001 to 2100.
const C: [f64; 24] = [5.4055, 20.12, 3.87, 18.73, 5.63, 20.646, 4.81, 20.1, 5.52, 21.04, 5.678, 21.37, 7.108, 22.83, 7.5, 23.13, 7.646, 23.042, 8.318, 23.438, 7.438, 22.36, 7.18, 21.94];
// The years the formula misses by a day: year, term, and by how much.
const OFF_BY_ONE: [(i64, usize, i64); 10] = [(2019, 0, 1), (2082, 1, 1), (2026, 3, -1), (2084, 5, 1), (2008, 9, 1), (2016, 12, 1), (2002, 14, 1), (2089, 19, 1), (2089, 20, 1), (2021, 23, -1)];

// The day of the month a term falls on, for the two terms of a month.
fn term_day(year: i64, term: usize) -> i64 {
    let y = year % 100;
    let leap_days = if term < 2 { (y - 1).div_euclid(4) } else { y.div_euclid(4) };
    let day = (y as f64 * 0.2422 + C[term]).floor() as i64 - leap_days;
    day + OFF_BY_ONE.iter().find(|(yy, t, _)| *yy == year && *t == term).map_or(0, |(_, _, by)| *by)
}

// The solar term on that day, if any (2001 to 2100).
pub fn term(y: i64, m: i64, d: i64) -> Option<&'static str> {
    if !(2001..=2100).contains(&y) || !(1..=12).contains(&m) {
        return None;
    }
    let first = (m as usize - 1) * 2;
    [first, first + 1].into_iter().find(|t| term_day(y, *t) == d).map(|t| TERMS[t])
}

// A festival of the solar calendar on that day, in her language.
pub fn solar_festival(m: i64, d: i64, lang: &str) -> Option<&'static str> {
    if lang == "zh" {
        return Some(match (m, d) {
            (1, 1) => "元旦",
            (2, 14) => "情人节",
            (3, 8) => "妇女节",
            (3, 12) => "植树节",
            (4, 1) => "愚人节",
            (5, 1) => "劳动节",
            (5, 4) => "青年节",
            (6, 1) => "儿童节",
            (7, 1) => "建党节",
            (8, 1) => "建军节",
            (9, 10) => "教师节",
            (10, 1) => "国庆节",
            (12, 24) => "平安夜",
            (12, 25) => "圣诞节",
            _ => return None,
        });
    }
    Some(match (m, d) {
        (1, 1) => "New Year's Day",
        (2, 14) => "Valentine's Day",
        (10, 31) => "Halloween",
        (12, 24) => "Christmas Eve",
        (12, 25) => "Christmas",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(y: i64, m: i64, d: i64) -> (i64, u8, bool, u8) {
        let l = of(y, m, d).unwrap();
        (l.year, l.month, l.leap, l.day)
    }

    // Dates as .NET's ChineseLunisolarCalendar gives them, the table's
    // first day to its last, leap months among them (2025's 闰六月, 2033's
    // 闰十一月).
    #[test]
    fn the_lunar_date_is_dot_nets() {
        assert_eq!(at(1901, 2, 19), (1901, 1, false, 1));
        assert_eq!(at(1950, 1, 1), (1949, 11, false, 13));
        assert_eq!(at(2000, 1, 1), (1999, 11, false, 25));
        assert_eq!(at(2024, 2, 10), (2024, 1, false, 1));
        assert_eq!(at(2025, 1, 29), (2025, 1, false, 1));
        assert_eq!(at(2025, 7, 25), (2025, 6, true, 1));
        assert_eq!(at(2026, 2, 16), (2025, 12, false, 29));
        assert_eq!(at(2026, 2, 17), (2026, 1, false, 1));
        assert_eq!(at(2026, 11, 7), (2026, 9, false, 29));
        assert_eq!(at(2027, 2, 6), (2027, 1, false, 1));
        assert_eq!(at(2033, 12, 22), (2033, 11, true, 1));
        assert_eq!(at(2034, 1, 20), (2033, 12, false, 1));
        assert_eq!(at(2100, 12, 31), (2100, 12, false, 1));
        assert_eq!(at(2101, 1, 28), (2100, 12, false, 29));
        assert_eq!(of(2101, 1, 29), None);
        assert_eq!(of(1901, 2, 18), None);
        // Every year of the table has 353 to 385 days.
        for y in 1901..=2100 {
            assert!((353..=385).contains(&year_days(y)), "{y}: {}", year_days(y));
        }
    }

    #[test]
    fn the_words_and_the_years_animal() {
        assert_eq!(words(&of(2026, 11, 7).unwrap()), "九月廿九");
        assert_eq!(words(&of(2025, 7, 25).unwrap()), "闰六月初一");
        assert_eq!(words(&of(2026, 2, 17).unwrap()), "正月初一");
        assert_eq!(words(&of(2026, 2, 16).unwrap()), "腊月廿九");
        assert_eq!(words(&of(2034, 1, 20).unwrap()), "腊月初一");
        assert_eq!(words(&of(2000, 1, 1).unwrap()), "冬月廿五");
        assert_eq!(year_words(2026), "丙午马年");
        assert_eq!(year_words(2025), "乙巳蛇年");
        assert_eq!(year_words(1901), "辛丑牛年");
        assert_eq!(year_words(2024), "甲辰龙年");
    }

    // 除夕 is the last day of the twelfth month, whichever it is that year.
    #[test]
    fn festivals_of_the_lunar_calendar() {
        assert_eq!(festival(&of(2026, 2, 17).unwrap()), Some("春节"));
        assert_eq!(festival(&of(2026, 2, 16).unwrap()), Some("除夕"));
        assert_eq!(festival(&of(2026, 9, 25).unwrap()), Some("中秋"));
        assert_eq!(festival(&of(2026, 6, 19).unwrap()), Some("端午"));
        assert_eq!(festival(&of(2026, 11, 7).unwrap()), None);
        assert_eq!(festival(&Lunar { year: 2025, month: 6, leap: true, day: 1 }), None);
    }

    // The terms of 2026 (立冬 on the 7th of November), and the years the
    // formula alone would miss (2026's 雨水, 2021's 冬至).
    #[test]
    fn the_solar_terms() {
        assert_eq!(term(2026, 1, 5), Some("小寒"));
        assert_eq!(term(2026, 2, 4), Some("立春"));
        assert_eq!(term(2026, 2, 18), Some("雨水"));
        assert_eq!(term(2026, 2, 19), None);
        assert_eq!(term(2026, 4, 5), Some("清明"));
        assert_eq!(term(2026, 6, 21), Some("夏至"));
        assert_eq!(term(2026, 11, 7), Some("立冬"));
        assert_eq!(term(2026, 11, 8), None);
        assert_eq!(term(2026, 12, 22), Some("冬至"));
        assert_eq!(term(2025, 12, 21), Some("冬至"));
        assert_eq!(term(2021, 12, 21), Some("冬至"));
        assert_eq!(term(1999, 12, 22), None, "outside the formula's years");
        assert_eq!(solar_festival(10, 1, "zh"), Some("国庆节"));
        assert_eq!(solar_festival(10, 1, "en"), None);
        assert_eq!(solar_festival(12, 25, "en"), Some("Christmas"));
    }
}
