// Stocks, exchange rates and coins, as the old stock.ps1 had them:
// each symbol's price and the day's change, from Yahoo Finance's chart data
// (unofficial, no key needed), every 5 minutes. One widget a symbol.
//   Shanghai and Shenzhen shares end in .SS and .SZ: 600519.SS, 000001.SZ
//   exchange rates: USDCNY=X, EURCNY=X; coins: BTC-USD, ETH-USD
//
// Up is red and down green, as Chinese markets show them, or green up (the
// Plugins page's 涨跌颜色, her language's until chosen). The colour it sends
// is the one place that says so: every place the widget is drawn follows it
// (wcards.ts trendOf).
use std::time::Duration;

use serde_json::{json, Value};

use super::fmt::{custom, escape, grouped, id_part, round_to};
use super::web::{self, Answer};
use super::{Fetch, Job, Now, Out, Wait};

const MINUTES: u64 = 5;
const TTL: u64 = MINUTES * 60 + 120;
const RED: &str = "#ff5c6c";
const GREEN: &str = "#34d27b";

pub struct Stock {
    symbols: Vec<String>,
    names: Vec<String>,
    green_up: bool,
    lang: &'static str,
    fetch: Fetch,
}

// "a,b" as typed: its parts, trimmed; the symbols without the empty ones,
// the names with them (each in the place of its symbol).
fn parts(text: &str, keep_empty: bool) -> Vec<String> {
    text.split(',').map(str::trim).filter(|p| keep_empty || !p.is_empty()).map(String::from).collect()
}

impl Stock {
    // No symbol given: AAPL, as the script's own default.
    pub fn new(symbols: &str, names: &str, green_up: bool, lang: &'static str, fetch: Fetch) -> Stock {
        let symbols = if symbols.trim().is_empty() { vec!["AAPL".to_string()] } else { parts(symbols, false) };
        let names = if names.trim().is_empty() { Vec::new() } else { parts(names, true) };
        Stock { symbols, names, green_up, lang, fetch }
    }

    // The day in five-minute steps: its price now and the curve of the day
    // for the larger sizes (the open island's card), one request.
    fn chart(&self, symbol: &str) -> Result<Value, String> {
        let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{}?range=1d&interval=5m", escape(symbol));
        let got = match (self.fetch)(&url, &[("user-agent", "Mozilla/5.0")]) {
            Ok(Answer { status: 200..=299, body }) => serde_json::from_str::<Value>(&body).map_err(|e| e.to_string())?,
            Ok(page) => return Err(web::refused(&page)),
            Err(fail) => return Err(web::trouble(self.lang, &fail)),
        };
        let chart = &got["chart"]["result"][0];
        if chart.is_object() {
            Ok(chart.clone())
        } else {
            Err(got["chart"]["error"]["description"].as_str().unwrap_or("no chart").to_string())
        }
    }
}

impl Job for Stock {
    fn turn(&mut self, _now: Now, out: &mut dyn Out) -> Wait {
        for (i, s) in self.symbols.iter().enumerate() {
            match self.chart(s) {
                Ok(chart) => out.post(widget(s, self.names.get(i).map(String::as_str), self.green_up, &chart)),
                Err(why) => out.say(format!("{s}: {why}")),
            }
        }
        Wait::every(Duration::from_secs(MINUTES * 60))
    }
}

// Few digits for big prices, more for small ones (a rate like 7.1234).
fn price(p: f64) -> String {
    if p >= 1000.0 {
        grouped(p, 0)
    } else if p >= 10.0 {
        grouped(p, 2)
    } else {
        grouped(p, 4)
    }
}

// A count in a few characters: 812, 48.1K, 48.1M, 1.2B.
fn short(n: f64) -> String {
    for (at, unit) in [(1e9, "B"), (1e6, "M"), (1e3, "K")] {
        if n >= at {
            return format!("{}{unit}", custom(n / at, 1));
        }
    }
    custom(n, 0)
}

// A number there, as PowerShell's truthiness has it: there and not nought.
fn some(v: &Value) -> Option<f64> {
    v.as_f64().filter(|n| *n != 0.0)
}

// A coin or a rate (-USD, -CNY, =X at its end, in any case), else a stock.
fn icon_of(symbol: &str) -> &'static str {
    let s = symbol.to_ascii_uppercase();
    if s.ends_with("-USD") || s.ends_with("-CNY") || s.ends_with("=X") {
        "coin"
    } else {
        "stock"
    }
}

// The widget for a symbol's chart, as the script posted it.
pub fn widget(symbol: &str, name: Option<&str>, green_up: bool, chart: &Value) -> Value {
    let meta = &chart["meta"];
    let price_now = meta["regularMarketPrice"].as_f64().unwrap_or(0.0);
    let before = meta["chartPreviousClose"].as_f64().unwrap_or(0.0);
    let change = if before != 0.0 { (price_now - before) / before * 100.0 } else { 0.0 };
    let is_up = change >= 0.0;
    let label = name.filter(|n| !n.is_empty()).unwrap_or(symbol);
    // Its prices that day, 120 at most (a coin's day has 288: every
    // third), the last always.
    let closes: Vec<f64> = chart["indicators"]["quote"][0]["close"].as_array().into_iter().flatten().filter_map(Value::as_f64).collect();
    let step = ((closes.len() as f64 / 120.0).ceil() as usize).max(1);
    let points: Vec<f64> = if closes.is_empty() { Vec::new() } else { closes.iter().skip((closes.len() - 1) % step).step_by(step).map(|c| round_to(*c, 4)).collect() };
    let mut row = json!({ "code": symbol, "name": label, "price": price(price_now), "change": format!("{}{}%", if is_up { "+" } else { "−" }, grouped(change.abs(), 2)) });
    if let Some(volume) = some(&meta["regularMarketVolume"]) {
        row["value"] = json!(short(volume));
    }
    let mut detail = json!({ "sizes": ["s", "m", "l"], "rows": [row] });
    if points.len() > 1 {
        let mut series = json!({ "points": points });
        if let Some(hi) = some(&meta["regularMarketDayHigh"]) {
            series["hi"] = json!(hi);
        }
        if let Some(lo) = some(&meta["regularMarketDayLow"]) {
            series["lo"] = json!(lo);
        }
        detail["series"] = series;
    }
    json!({
        "id": format!("stock-{}", id_part(symbol)),
        "icon": icon_of(symbol),
        "color": if is_up != green_up { RED } else { GREEN },
        "label": label,
        "value": format!("{} {}{}%", price(price_now), if is_up { "▲" } else { "▼" }, grouped(change.abs(), 2)),
        "detail": detail,
        "ttl": TTL,
    })
}

#[cfg(test)]
mod tests {
    use super::super::tests::{golden, record, same, served};
    use super::*;

    const SYMBOLS: &str = "600519.SS,AAPL,NOSUCHSYMBOLX,BTC-USD,USDCNY=X";

    fn serve() -> (Fetch, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        served(&[
            ("https://query1.finance.yahoo.com/v8/finance/chart/600519.SS?", 200, "chart-600519ss.json"),
            ("https://query1.finance.yahoo.com/v8/finance/chart/AAPL?", 200, "chart-aapl.json"),
            ("https://query1.finance.yahoo.com/v8/finance/chart/NOSUCHSYMBOLX?", 404, "chart-none.json"),
            ("https://query1.finance.yahoo.com/v8/finance/chart/BTC-USD?", 200, "chart-btc.json"),
            ("https://query1.finance.yahoo.com/v8/finance/chart/USDCNY%3DX?", 200, "chart-usdcny.json"),
        ])
    }

    // Each symbol's widget as stock.ps1 posted it for the same charts from
    // Yahoo (a Shanghai share, a US one, a coin, a rate; a name given, one
    // left empty, more names than one), red up and green up; and one Yahoo
    // does not know, said under its symbol, the others posted all the same.
    #[test]
    fn charts_read_as_the_script_posted_them() {
        for (green, names, file) in [(false, "茅台,,坏的,比特币", "stock-red"), (true, "", "stock-green")] {
            let (fetch, asked) = serve();
            let (posts, said, wait) = record(&mut Stock::new(SYMBOLS, names, green, "zh", fetch), 0);
            let (theirs, warnings) = golden(file);
            assert_eq!(posts.len(), theirs.len());
            for (mine, (_, theirs)) in posts.iter().zip(&theirs) {
                assert!(same(mine, theirs), "{file}:\n{mine}\n{theirs}");
            }
            assert_eq!(warnings, ["NOSUCHSYMBOLX: {\"chart\":{\"result\":null,\"error\":{\"code\":\"Not Found\",\"description\":\"No data found, symbol may be delisted\"}}}"]);
            // Yahoo's own words, not the whole of its answer.
            assert_eq!(said.as_deref(), Some("NOSUCHSYMBOLX: 404: No data found, symbol may be delisted"));
            assert_eq!(wait, Wait::every(Duration::from_secs(300)));
            assert_eq!(asked.lock().unwrap().len(), 5);
        }
    }

    #[test]
    fn prices_and_counts_in_a_few_characters() {
        assert_eq!((price(1329.0), price(343.46), price(6.67681), price(81326.906)), ("1,329".into(), "343.46".into(), "6.6768".into(), "81,327".into()));
        assert_eq!((short(812.0), short(48_150.0), short(3_850_175.0), short(1.2e9), short(999.5)), ("812".into(), "48.2K".into(), "3.9M".into(), "1.2B".into(), "1000".into()));
        assert_eq!((icon_of("btc-usd"), icon_of("EURCNY=X"), icon_of("ETH-CNY"), icon_of("600519.SS")), ("coin", "coin", "coin", "stock"));
        assert_eq!(parts(" a, ,b ,", true), ["a", "", "b", ""]);
        assert_eq!(parts(" a, ,b ,", false), ["a", "b"]);
        // A day with no prices yet, or one: no curve.
        let flat = widget("X", None, false, &json!({ "meta": { "regularMarketPrice": 2.0, "chartPreviousClose": 2.0 }, "indicators": { "quote": [{ "close": [null, 2.0] }] } }));
        assert_eq!((flat["value"].as_str(), flat["color"].as_str(), flat["detail"]["series"].is_null()), (Some("2.0000 ▲0.00%"), Some(RED), true));
    }
}
