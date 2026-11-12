// The weather, as the old weather.ps1 had it: the sky and the
// temperature in a city, and today's lowest and highest (shown when the
// island is open), from Open-Meteo (no key needed), every 10 minutes. The
// city can be named in English or Chinese. Its larger sizes (the open
// island's card): the next day's temperatures hour by hour as a curve,
// every second hour with its sky, tomorrow and the day after; all from the
// same one request. Where the city is, is asked once and kept (the script,
// started anew each turn, asked every time).
//
// Its words are in her language (the script went by Windows'; hers is
// Windows' until she is told another).
use serde_json::{json, Value};

use super::fmt::{deg, escape, round_to};
use super::web::{self, Answer};
use super::{Fetch, Job, Now, Out, Wait};

// Every 10 minutes (the script's -Minutes 10); a widget stays a little
// longer than that, so it is there between two.
const MINUTES: u64 = 10;
const TTL: u64 = MINUTES * 60 + 120;

pub struct Weather {
    city: String,
    lang: &'static str,
    // Its latitude and longitude, once found.
    place: Option<(f64, f64)>,
    fetch: Fetch,
}

impl Weather {
    // No city given: Shanghai, as the script's own default.
    pub fn new(city: &str, lang: &'static str, fetch: Fetch) -> Weather {
        let city = if city.trim().is_empty() { "Shanghai" } else { city.trim() };
        Weather { city: city.into(), lang, place: None, fetch }
    }

    fn get(&self, url: &str) -> Result<Value, String> {
        match (self.fetch)(url, &[("user-agent", "wakuwaku")]) {
            Ok(Answer { status: 200..=299, body }) => serde_json::from_str(&body).map_err(|e| e.to_string()),
            Ok(page) => Err(web::refused(&page)),
            Err(fail) => Err(web::trouble(self.lang, &fail)),
        }
    }

    fn look(&mut self) -> Result<Value, String> {
        let (lat, lon) = match self.place {
            Some(place) => place,
            None => {
                let found = self.get(&format!("https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language={}", escape(&self.city), self.lang))?;
                let place = place_of(&found).ok_or_else(|| if self.lang == "zh" { format!("找不到这个城市：{}", self.city) } else { format!("no city called {}", self.city) })?;
                *self.place.insert(place)
            }
        };
        // Now, today's lowest and highest there (its own time zone), and for
        // the card the hours ahead and the next two days: one request.
        let got = self.get(&format!("https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}&current=temperature_2m,weather_code,is_day&hourly=temperature_2m,weather_code,is_day&daily=temperature_2m_min,temperature_2m_max,weather_code&timezone=auto&forecast_days=3"))?;
        widget(&self.city, self.lang, &got)
    }
}

impl Job for Weather {
    fn turn(&mut self, _now: Now, out: &mut dyn Out) -> Wait {
        match self.look() {
            Ok(w) => out.post(w),
            Err(line) => out.say(line),
        }
        Wait::every(std::time::Duration::from_secs(MINUTES * 60))
    }
}

// The first place a search found.
pub fn place_of(found: &Value) -> Option<(f64, f64)> {
    let first = found["results"].get(0)?;
    Some((first["latitude"].as_f64()?, first["longitude"].as_f64()?))
}

// The sky by its WMO code, in Chinese and in English.
fn sky(code: i64) -> Option<(&'static str, &'static str)> {
    Some(match code {
        0 => ("晴", "Clear"),
        1 => ("晴", "Mostly clear"),
        2 => ("多云", "Partly cloudy"),
        3 => ("阴", "Overcast"),
        45 | 48 => ("雾", "Fog"),
        51 | 53 | 55 => ("毛毛雨", "Drizzle"),
        56 | 57 => ("冻雨", "Freezing drizzle"),
        61 => ("小雨", "Light rain"),
        63 => ("中雨", "Rain"),
        65 => ("大雨", "Heavy rain"),
        66 | 67 => ("冻雨", "Freezing rain"),
        71 => ("小雪", "Light snow"),
        73 => ("中雪", "Snow"),
        75 => ("大雪", "Heavy snow"),
        77 => ("雪粒", "Snow grains"),
        80 | 81 => ("阵雨", "Showers"),
        82 => ("强阵雨", "Heavy showers"),
        85 | 86 => ("阵雪", "Snow showers"),
        95 => ("雷阵雨", "Thunderstorm"),
        96 | 99 => ("雷阵雨伴冰雹", "Thunderstorm, hail"),
        _ => return None,
    })
}

// PowerShell's [int] of what came: a number to the even one, none as 0.
fn int(v: &Value) -> i64 {
    v.as_f64().map_or(0, |n| n.round_ties_even() as i64)
}

// [double] of what came: none as 0.
fn num(v: &Value) -> f64 {
    v.as_f64().unwrap_or(0.0)
}

fn sky_words(code: &Value, lang: &str) -> &'static str {
    sky(int(code)).map_or("", |(zh, en)| if lang == "zh" { zh } else { en })
}

// The sky by its WMO code as one of the island's weather icons (wcards.ts:
// Windows' own glyphs), by day or by night (is_day 0; none is day).
fn sky_icon(code: &Value, is_day: &Value) -> &'static str {
    let night = is_day.as_f64() == Some(0.0);
    match int(code) {
        c if c <= 1 => {
            if night {
                "moon"
            } else {
                "sun"
            }
        }
        2 => {
            if night {
                "moon"
            } else {
                "partly"
            }
        }
        3 => "cloud",
        45 | 48 => "fog",
        71 | 73 | 75 | 77 | 85 | 86 => "snow",
        c if c >= 95 => "storm",
        _ => "rain",
    }
}

// A date's day of the week, in English (yyyy-mm-dd).
fn weekday(date: &str) -> Option<&'static str> {
    let mut parts = date.get(..10)?.split('-').map(|p| p.parse::<i64>().ok());
    let (y, m, d) = (parts.next()??, parts.next()??, parts.next()??);
    let days = super::countdown::days_from_civil(y, m, d);
    Some(["Thursday", "Friday", "Saturday", "Sunday", "Monday", "Tuesday", "Wednesday"][days.rem_euclid(7) as usize])
}

// The widget for a forecast, as the script posted it.
pub fn widget(city: &str, lang: &str, got: &Value) -> Result<Value, String> {
    let dash = if lang == "zh" { "~" } else { "–" };
    let now = &got["current"];
    let daily = &got["daily"];
    let (low, high) = (&daily["temperature_2m_min"][0], &daily["temperature_2m_max"][0]);
    // The range, shown after the temperature in the open island only.
    let range = if !low.is_null() && !high.is_null() { format!("{}{dash}{}", deg(num(low)), deg(num(high))) } else { String::new() };
    let words = sky_words(&now["weather_code"], lang);
    // The hours from this one on: the next 24 as a curve, every second one
    // (seven) with its sky; then tomorrow and the day after.
    let empty = Vec::new();
    let times = got["hourly"]["time"].as_array().unwrap_or(&empty);
    let hour_now = now["time"].as_str().map(|t| t.to_string()).unwrap_or_else(|| now["time"].to_string());
    let hour_now = format!("{}:00", hour_now.get(..13).ok_or_else(|| format!("the time there reads {hour_now}"))?);
    let first = times.iter().position(|t| t.as_str() == Some(hour_now.as_str()));
    let mut rows: Vec<Value> = Vec::new();
    let mut series: Vec<f64> = Vec::new();
    if let Some(first) = first {
        let temps = &got["hourly"]["temperature_2m"];
        let end = times.len().min(first + 24);
        series = (first..end).filter_map(|i| temps[i].as_f64()).map(|t| round_to(t, 1)).collect();
        let mut i = first;
        while i <= first + 12 && i < times.len() {
            let hour: i64 = times[i].as_str().and_then(|t| t.get(11..13)).and_then(|h| h.parse().ok()).ok_or_else(|| format!("the hour there reads {}", times[i]))?;
            let when = if lang == "zh" { format!("{hour}时") } else { format!("{hour:02}:00") };
            rows.push(json!({ "kind": "hour", "when": when, "icon": sky_icon(&got["hourly"]["weather_code"][i], &got["hourly"]["is_day"][i]), "value": deg(num(&temps[i])) }));
            i += 2;
        }
    }
    let days = daily["time"].as_array().map_or(0, Vec::len);
    for d in 1..=2usize {
        if d >= days {
            break;
        }
        let when = if lang == "zh" {
            ["", "明天", "后天"][d].to_string()
        } else if d == 1 {
            "Tomorrow".into()
        } else {
            let date = daily["time"][d].as_str().unwrap_or("");
            weekday(date).ok_or_else(|| format!("the day there reads {date}"))?.into()
        };
        let code = &daily["weather_code"][d];
        let range = format!("{}{dash}{}", deg(num(&daily["temperature_2m_min"][d])), deg(num(&daily["temperature_2m_max"][d])));
        rows.push(json!({ "kind": "day", "when": when, "icon": sky_icon(code, &json!(1)), "value": range, "name": sky_words(code, lang) }));
    }
    let mut detail = json!({ "sizes": ["s", "m", "l"], "extra": range, "rows": rows });
    if !series.is_empty() {
        detail["series"] = json!({ "points": series, "unit": "°" });
    }
    let label = format!("{city} · {words}");
    Ok(json!({
        "id": "weather",
        "icon": "weather",
        "color": "#ffb340",
        "label": label.trim_end_matches([' ', '·']),
        "value": deg(num(&now["temperature_2m"])),
        "detail": detail,
        // A little longer than the wait, so it stays between two updates.
        "ttl": TTL,
    }))
}

#[cfg(test)]
mod tests {
    use super::super::tests::{golden, record, same, served};
    use super::*;

    fn fixture(name: &str) -> Value {
        serde_json::from_str(&std::fs::read_to_string(format!("{}/../test/fixtures/plugins/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()).unwrap()
    }

    // The widget as weather.ps1 posted it for the same answers from
    // Open-Meteo (test/fixtures/plugins/ps, recorded by running the script
    // with the web played back to it), in Chinese and in English.
    #[test]
    fn a_forecast_reads_as_the_script_posted_it() {
        let got = fixture("forecast-shanghai.json");
        for (city, lang, file) in [("上海", "zh", "weather-zh"), ("Shanghai", "en", "weather-en")] {
            let (posts, warnings) = golden(file);
            assert!(warnings.is_empty());
            let mine = widget(city, lang, &got).unwrap();
            assert!(same(&mine, &posts[0].1), "{lang}:\n{mine}\n{}", posts[0].1);
        }
        assert_eq!(place_of(&fixture("geocode-shanghai-zh.json")), Some((31.22222, 121.45806)));
        assert_eq!(place_of(&fixture("geocode-none.json")), None);
    }

    // A whole turn against the recorded answers: the city looked for once
    // (by its name escaped, in her language), the forecast asked for there.
    #[test]
    fn a_turn_finds_the_city_once_and_asks_for_its_forecast() {
        let (fetch, asked) = served(&[("https://geocoding-api.open-meteo.com/", 200, "geocode-shanghai-zh.json"), ("https://api.open-meteo.com/", 200, "forecast-shanghai.json")]);
        let mut w = Weather::new(" 上海 ", "zh", fetch);
        let (posts, said, wait) = record(&mut w, 0);
        assert_eq!((posts.len(), said, wait), (1, None, Wait::every(std::time::Duration::from_secs(600))));
        assert!(same(&posts[0], &golden("weather-zh").0[0].1));
        record(&mut w, 600_000);
        let asked = asked.lock().unwrap();
        assert_eq!(asked.len(), 3, "{asked:?}");
        assert_eq!(asked[0], "https://geocoding-api.open-meteo.com/v1/search?name=%E4%B8%8A%E6%B5%B7&count=1&language=zh");
        assert!(asked[1].starts_with("https://api.open-meteo.com/v1/forecast?latitude=31.22222&longitude=121.45806&current=temperature_2m,weather_code,is_day&"));
        assert!(asked[2].starts_with("https://api.open-meteo.com/"));
    }

    // No such city: the script's own words, in her language; nothing posted.
    // The web down: said so, and looked for again next turn.
    #[test]
    fn a_city_not_found_is_said_in_her_language() {
        let (_, warnings) = golden("weather-none");
        for (city, lang, said) in [("Nowhereville", "en", &warnings[0]), ("无此城", "zh", &warnings[1])] {
            let (fetch, _) = served(&[("https://geocoding-api.open-meteo.com/", 200, "geocode-none.json")]);
            let (posts, mine, _) = record(&mut Weather::new(city, lang, fetch), 0);
            assert!(posts.is_empty());
            assert_eq!(mine.as_deref(), Some(said.as_str()));
        }
        let (fetch, asked) = served(&[]);
        let mut w = Weather::new("", "zh", fetch);
        let (posts, said, _) = record(&mut w, 0);
        assert!(posts.is_empty() && said.unwrap().starts_with("连不上："));
        assert!(asked.lock().unwrap()[0].contains("name=Shanghai&"));
        record(&mut w, 1);
        assert_eq!(asked.lock().unwrap().len(), 2);
    }

    // Night, fog, snow, a storm, a code it does not know; and the days' names.
    #[test]
    fn skies_and_days() {
        assert_eq!((sky_icon(&json!(0), &json!(0)), sky_icon(&json!(1), &json!(1)), sky_icon(&json!(2), &json!(0)), sky_icon(&json!(2), &Value::Null)), ("moon", "sun", "moon", "partly"));
        assert_eq!((sky_icon(&json!(48), &json!(1)), sky_icon(&json!(86), &json!(1)), sky_icon(&json!(99), &json!(1)), sky_icon(&json!(61), &json!(1)), sky_icon(&Value::Null, &Value::Null)), ("fog", "snow", "storm", "rain", "sun"));
        assert_eq!((sky_words(&json!(96), "zh"), sky_words(&json!(96), "en"), sky_words(&json!(4), "en"), sky_words(&Value::Null, "zh")), ("雷阵雨伴冰雹", "Thunderstorm, hail", "", "晴"));
        assert_eq!((weekday("2026-10-20"), weekday("2026-10-18"), weekday("2000-02-29"), weekday("x")), (Some("Tuesday"), Some("Sunday"), Some("Tuesday"), None));
    }
}
