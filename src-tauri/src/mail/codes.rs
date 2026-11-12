// Codes and links to sign in with, in letters as they come: the code a
// service mails ("您的验证码为 482910", "482910 is your verification code",
// Steam Guard's on a line of its own) or its link ("Log in to Notion",
// "Verify your email address"), put on the island as it arrives with Copy,
// for a link Open in the browser too, and Ignore (sixthdove's idea,
// zhoupengjie/Wakuwaku#113). Found by the words around it, on this
// computer: nothing is asked of a model, nothing leaves.
//
// Read: only a new letter (unread, come since the newest unread seen after
// the first sync, sync.rs), got by the server in the last ten minutes, and
// small (a code's letter is a few kB). Shown: under any agent's prompt (the
// prompt panel's, shared.rs asks_views), only while she is in sight and not
// asked to keep quiet (a code nobody can see is no use); a minute, then
// gone. Not written down: the log says a code came, never which.
//
// Settings: "mailCodes": true (the default) | false.
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde_json::{json, Value};

use super::imap::Session;
use super::letters;
use super::store::{Folder, Store};
use super::Account;
use crate::{now_ms, Shared};
use tauri::Emitter;
use crate::config;
use crate::lock::Lock;

// --- Finding one ------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    Code(String),
    Link(String),
}

// How much of a letter's text is read: its start, where the code is.
const READ: usize = 6000;
// How much of its HTML is looked through for links.
const READ_HTML: usize = 300_000;

// The words a code is given with. Strong: the code within a sentence of
// them, before ("您的验证码为 482910") or after ("482910 is your
// verification code"). English ones are whole words ("otp", not "otpauth").
const STRONG: &[&str] = &[
    "验证码", "校验码", "确认码", "动态码", "动态密码", "一次性密码", "安全码", "登录码", "认证码", "验证代码",
    "驗證碼", "確認碼", "認証コード", "確認コード", "認証番号", "ワンタイムパスワード",
    "verification code", "verify code", "security code", "confirmation code", "authentication code", "auth code",
    "login code", "log-in code", "sign-in code", "sign in code", "signin code", "one-time code", "one time code",
    "one-time password", "one time password", "one-time passcode", "one time passcode", "passcode", "otp",
    "2fa code", "two-factor code", "steam guard",
];
// Plain ones ("Code: 482910", "your code is 482910", "PIN: 4821"): the code
// right after, and no word of orders, offers or money near.
const WEAK: &[&str] = &["code", "pin"];
// Near a code, these make it something else: an order's, an offer's, a
// price, a parcel's. English ones whole words.
const NOT: &[&str] = &[
    "订单", "单号", "运单", "快递", "物流", "取件", "金额", "价格", "余额", "发票", "优惠", "折扣", "红包", "积分",
    "邀请", "编号", "电话", "手机号", "邮编", "￥", "¥", "$", "€", "%",
    "order", "invoice", "receipt", "tracking", "shipment", "amount", "total", "price", "balance", "coupon", "promo",
    "discount", "referral", "invite", "voucher", "gift", "redeem", "checkout", "zip", "postal", "fax", "error",
    "booking", "reservation", "flight", "seat", "room", "ticket", "off", "offer", "sale",
];

// The code in a letter, else the link to sign in with; None for most.
pub fn find(subject: &str, text: &str, html: &str) -> Option<Found> {
    if let Some(code) = find_code(subject, text) {
        return Some(Found::Code(code));
    }
    find_link(subject, text, html).map(Found::Link)
}

pub fn find_code(subject: &str, text: &str) -> Option<String> {
    let subject = normal(subject, 300);
    let in_subject = best_code(&subject, false);
    if in_subject.is_some() {
        return in_subject.map(|(code, _)| code);
    }
    let said = hits(&lower(&subject)).iter().any(|h| h.strong);
    best_code(&normal(text, READ), said).map(|(code, _)| code)
}

// Full-width digits and letters as ASCII ("６５４３２１"), the spaces that
// are not ASCII's as one, a full-width colon as ":".
fn normal(s: &str, most: usize) -> Vec<char> {
    s.chars()
        .take(most)
        .map(|c| match c {
            '\u{FF10}'..='\u{FF19}' | '\u{FF21}'..='\u{FF3A}' | '\u{FF41}'..='\u{FF5A}' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
            '\u{00A0}' | '\u{2002}'..='\u{200B}' | '\u{3000}' => ' ',
            '：' => ':',
            '\r' => ' ',
            _ => c,
        })
        .collect()
}

fn lower(c: &[char]) -> Vec<char> {
    c.iter().map(char::to_ascii_lowercase).collect()
}

#[derive(Debug)]
struct Hit {
    at: usize,
    end: usize,
    strong: bool,
}

// Where the words are, each once: (start, end, strong).
fn hits(low: &[char]) -> Vec<Hit> {
    let mut out = Vec::new();
    for (words, strong) in [(STRONG, true), (WEAK, false)] {
        for word in words {
            for at in find_all(low, word) {
                out.push(Hit { at, end: at + word.chars().count(), strong });
            }
        }
    }
    out
}

// Each place a word is, a whole word when it is English.
fn find_all(low: &[char], word: &str) -> Vec<usize> {
    let w: Vec<char> = word.chars().collect();
    let english = word.is_ascii();
    let mut out = Vec::new();
    if w.is_empty() || low.len() < w.len() {
        return out;
    }
    for i in 0..=low.len() - w.len() {
        if low[i] != w[0] || low[i..i + w.len()] != w[..] {
            continue;
        }
        if english && (i > 0 && low[i - 1].is_ascii_alphanumeric() || low.get(i + w.len()).is_some_and(|c| c.is_ascii_alphanumeric())) {
            continue;
        }
        out.push(i);
    }
    out
}

#[derive(Debug, Clone)]
struct Cand {
    text: String,
    at: usize,
    end: usize,
    digits: usize,
}

// Runs of letters and digits 4 to 8 long; digits in equal groups a space or
// a hyphen apart are one ("123 456", "482-910", "12 34 56", Slack's
// "ABC-123"), unequal ones are not (a date, 2026-11-09; a phone number).
fn candidates(c: &[char]) -> Vec<Cand> {
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < c.len() {
        if c[i].is_ascii_alphanumeric() {
            let start = i;
            while i < c.len() && c[i].is_ascii_alphanumeric() {
                i += 1;
            }
            runs.push((start, i));
        } else {
            i += 1;
        }
    }
    let mut out = Vec::new();
    let mut k = 0;
    while k < runs.len() {
        let (start, first_end) = runs[k];
        let len = first_end - start;
        let mut j = k;
        while j + 1 < runs.len() && (2..=4).contains(&len) {
            let (next, next_end) = runs[j + 1];
            let sep = c[runs[j].1];
            let digits = |r: (usize, usize)| c[r.0..r.1].iter().all(char::is_ascii_digit);
            let joins = next == runs[j].1 + 1
                && next_end - next == len
                && match sep {
                    ' ' => digits(runs[j]) && digits(runs[j + 1]),
                    '-' => len >= 3 || (digits(runs[j]) && digits(runs[j + 1])),
                    _ => false,
                };
            if !joins {
                break;
            }
            j += 1;
        }
        let end = runs[j].1;
        let text: String = c[start..end].iter().filter(|ch| ch.is_ascii_alphanumeric()).collect();
        if (4..=8).contains(&text.len()) {
            let digits = text.chars().filter(char::is_ascii_digit).count();
            out.push(Cand { text, at: start, end, digits });
        }
        k = j + 1;
    }
    out
}

// A piece of a longer thing: a time (08:30), a date (2026-11-09), a
// decimal, an address, an amount.
fn embedded(c: &[char], x: &Cand) -> bool {
    let before = |n: usize| x.at.checked_sub(n).map(|i| c[i]);
    let after = |n: usize| c.get(x.end + n - 1).copied();
    let digit = |ch: Option<char>| ch.is_some_and(|ch| ch.is_ascii_digit());
    let alnum = |ch: Option<char>| ch.is_some_and(|ch| ch.is_ascii_alphanumeric());
    match before(1) {
        Some('@' | '#' | '$' | '+' | '/' | '\\' | '_') => return true,
        Some('-' | ':' | ',') if digit(before(2)) => return true,
        Some('.') if alnum(before(2)) => return true,
        _ => {}
    }
    match after(1) {
        Some('@' | '%' | '/' | '\\' | '_') => true,
        // Not a comma: "验证码为123456,5分钟内有效".
        Some('-' | ':') if digit(after(2)) => true,
        Some('.') if alnum(after(2)) => true,
        _ => false,
    }
}

// Inside an address on the web ("…/item/482910", "?code=482910").
fn in_link(c: &[char], x: &Cand) -> bool {
    let part = |ch: char| ch.is_ascii_graphic() && !"\"'<>()[]{}".contains(ch);
    let mut from = x.at;
    while from > 0 && part(c[from - 1]) {
        from -= 1;
    }
    let mut to = x.end;
    while to < c.len() && part(c[to]) {
        to += 1;
    }
    let token: String = c[from..to].iter().collect::<String>().to_ascii_lowercase();
    token.contains("://") || token.starts_with("www.") || (token.contains('/') && token.contains('.')) || (token.contains('?') && token.contains('='))
}

// Nothing but "is", "为", a colon or quotes between the word and the code.
fn tight(between: &[char]) -> bool {
    if between.len() > 8 {
        return false;
    }
    let rest: String = between.iter().filter(|c| !c.is_whitespace() && !":-–—=\"'“”‘’「」【】[]()（）<>《》*#".contains(**c)).collect::<String>().to_lowercase();
    matches!(rest.as_str(), "" | "is" | "was" | "为" | "是" | "は")
}

// A word of orders, money or offers in this stretch.
fn says_not(low: &[char], from: usize, to: usize) -> bool {
    let to = to.min(low.len());
    let from = from.min(to);
    let part = &low[from..to];
    NOT.iter().any(|w| !find_all(part, w).is_empty())
}

// On a line of its own (decoration aside): "\n\n  482910\n\n", "**482910**".
fn alone(c: &[char], x: &Cand) -> bool {
    let deco = |ch: char| ch.is_whitespace() && ch != '\n' || "*\"'“”【】[]".contains(ch);
    let mut i = x.at;
    while i > 0 && c[i - 1] != '\n' {
        if !deco(c[i - 1]) {
            return false;
        }
        i -= 1;
    }
    let mut j = x.end;
    while j < c.len() && c[j] != '\n' {
        if !deco(c[j]) {
            return false;
        }
        j += 1;
    }
    true
}

// The shape of a code: digits (not a year, unless the word is right before
// it), letters and digits as codes are (A1B2C3, 9KJ4X), capitals alone
// (Steam Guard's) only where it is plainly one.
fn shaped(x: &Cand, sure: bool) -> bool {
    let n = x.text.len();
    if x.digits == n {
        let year = n == 4 && matches!(x.text.parse::<u32>(), Ok(1900..=2099));
        return !year || sure;
    }
    let capitals = x.text.chars().all(|ch| ch.is_ascii_digit() || ch.is_ascii_uppercase());
    if x.digits > 0 {
        return capitals || (sure && x.digits >= 2);
    }
    sure && capitals && n >= 5
}

// The likeliest code in a text and how sure; `said`: the subject names one
// ("Your verification code"), so one alone on its line is it.
fn best_code(c: &[char], said: bool) -> Option<(String, i32)> {
    let low = lower(c);
    let words = hits(&low);
    if words.is_empty() && !said {
        return None;
    }
    let mut best: Option<(String, i32)> = None;
    for x in candidates(c) {
        if embedded(c, &x) || in_link(c, &x) {
            continue;
        }
        let mut score = i32::MIN;
        let mut sure = false;
        for w in &words {
            if w.end <= x.at {
                // The word, then the code.
                let gap = x.at - w.end;
                let close = tight(&c[w.end..x.at]);
                let s = if w.strong && gap <= 40 {
                    if close || !says_not(&low, w.at.saturating_sub(16), x.end + 16) {
                        100 - gap as i32
                    } else {
                        continue;
                    }
                } else if !w.strong && gap <= 6 && close && !says_not(&low, w.at.saturating_sub(16), x.end + 16) {
                    70 - gap as i32
                } else {
                    continue;
                };
                if s > score {
                    score = s;
                    sure = w.strong && close;
                }
            } else if x.end <= w.at && w.strong {
                // The code, then the word ("482910 是您的验证码").
                let gap = w.at - x.end;
                if gap <= 20 && !says_not(&low, x.at.saturating_sub(16), w.at) {
                    let s = 90 - gap as i32;
                    if s > score {
                        score = s;
                        sure = false;
                    }
                }
            }
        }
        // Alone on its line under the words (Steam Guard's, "Your
        // verification code" with the code on its own further down).
        if alone(c, &x) && (said && x.at < 1500 || words.iter().any(|w| w.strong && w.end <= x.at && x.at - w.end <= 300)) && 80 >= score {
            score = 80;
            sure = true;
        }
        if score == i32::MIN || !shaped(&x, sure) {
            continue;
        }
        score += if x.text.len() == 6 { 5 } else { 0 } + if x.digits == x.text.len() { 3 } else { 0 };
        if best.as_ref().is_none_or(|(_, b)| score > *b) {
            best = Some((x.text, score));
        }
    }
    best
}

// --- A link to sign in with ---------------------------------------------------------------

// What a letter with one is about (its subject).
const LINK_ABOUT: &[&str] = &[
    "verify", "verification", "confirm", "activate", "activation", "sign in", "sign-in", "signin", "log in", "log-in", "login",
    "magic link", "reset your password", "reset password", "password reset", "验证", "确认", "激活", "登录", "重置密码", "認証", "確認", "ログイン",
];
// What the link says or where it goes.
const LINK_SAYS: &[&str] = &[
    "verify", "confirm", "activate", "sign in", "log in", "login", "magic link", "reset password", "reset your password",
    "验证", "确认", "激活", "登录", "重置密码", "認証", "確認", "ログイン",
];
const LINK_PATH: &[&str] = &[
    "verify", "verification", "confirm", "activate", "activation", "magic", "login", "log-in", "signin", "sign-in", "sign_in",
    "auth", "callback", "reset", "password", "token=", "code=", "otp", "ticket=",
];
// Links that are never it: leaving the list, the small print, a parcel, a
// receipt, sharing, the letter on the web, the app stores, pictures.
const LINK_NOT: &[&str] = &[
    "unsubscribe", "optout", "opt-out", "preferences", "privacy", "terms", "policy", "help", "support", "faq", "track", "order",
    "receipt", "invoice", "share", "webversion", "view in browser", "view online", "退订", "隐私", "条款", "帮助",
    "apps.apple.com", "play.google.com", ".png", ".jpg", ".jpeg", ".gif", ".svg", ".webp", ".css", ".js",
];

pub fn find_link(subject: &str, text: &str, html: &str) -> Option<String> {
    let about = subject.to_lowercase();
    if !LINK_ABOUT.iter().any(|w| about.contains(w)) {
        return None;
    }
    let mut best: Option<(String, i32)> = None;
    let mut seen = HashSet::new();
    for (url, says) in anchors(html).into_iter().chain(text_links(text).into_iter().map(|u| (u, String::new()))) {
        if !seen.insert(url.clone()) {
            continue;
        }
        let low = url.to_lowercase();
        let says = says.to_lowercase();
        if !(low.starts_with("https://") || low.starts_with("http://")) || LINK_NOT.iter().any(|w| low.contains(w) || says.contains(w)) || !has_token(&url) {
            continue;
        }
        let said = LINK_SAYS.iter().any(|w| says.contains(w));
        let path = LINK_PATH.iter().filter(|w| low.contains(*w)).count() as i32;
        if !said && path == 0 {
            continue;
        }
        let score = if said { 3 } else { 0 } + path.min(3);
        if best.as_ref().is_none_or(|(_, b)| score > *b) {
            best = Some((url, score));
        }
    }
    best.map(|(url, _)| url)
}

// A long random part, as a link made for one person has (a token).
fn has_token(url: &str) -> bool {
    let mut run = 0usize;
    let (mut letters, mut digits) = (false, false);
    for ch in url.chars().chain(std::iter::once(' ')) {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            run += 1;
            letters |= ch.is_ascii_alphabetic();
            digits |= ch.is_ascii_digit();
        } else {
            if run >= 32 || run >= 16 && letters && digits {
                return true;
            }
            (run, letters, digits) = (0, false, false);
        }
    }
    false
}

// The links in a letter's HTML and what each says: (href, its text).
fn anchors(html: &str) -> Vec<(String, String)> {
    let html = &html[..floor(html, READ_HTML)];
    // The same bytes, ASCII's lowered: what is found in one is at the same
    // place in the other.
    let low = html.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(p) = low[at..].find("<a") {
        let start = at + p + 2;
        at = start;
        if !low[start..].starts_with(|c: char| c.is_ascii_whitespace()) {
            continue;
        }
        let Some(gt) = low[start..].find('>').map(|i| start + i) else { break };
        let Some(close) = low[gt..].find("</a").map(|i| gt + i) else { break };
        if let Some(href) = href_of(&html[start..gt], &low[start..gt]) {
            out.push((href, plain(&html[gt + 1..close])));
        }
        at = close;
    }
    out
}

// The href of a tag's attributes.
fn href_of(head: &str, low: &str) -> Option<String> {
    let mut from = 0;
    while let Some(p) = low[from..].find("href") {
        let at = from + p;
        from = at + 4;
        if at > 0 && !low.as_bytes()[at - 1].is_ascii_whitespace() {
            continue;
        }
        let rest = head[at + 4..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else { continue };
        let rest = rest.trim_start();
        let value = match rest.chars().next() {
            Some(q @ ('"' | '\'')) => rest[1..].split(q).next().unwrap_or(""),
            _ => rest.split(|c: char| c.is_whitespace() || c == '>').next().unwrap_or(""),
        };
        return Some(value.trim().replace("&amp;", "&"));
    }
    None
}

// What a piece of HTML says, its tags taken out.
fn plain(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in html.chars().take(2000) {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ").split_whitespace().collect::<Vec<_>>().join(" ")
}

// The addresses written out in a letter's text.
fn text_links(text: &str) -> Vec<String> {
    let text = &text[..floor(text, READ * 4)];
    let mut out = Vec::new();
    for word in text.split(|c: char| c.is_whitespace() || "<>\"'()[]{}，。、".contains(c)) {
        let Some(p) = word.find("http://").or_else(|| word.find("https://")) else { continue };
        let url = word[p..].trim_end_matches(['.', ',', ';', ':', '!', '?']);
        out.push(url.replace("&amp;", "&"));
    }
    out
}

// The most of `s` up to `most` bytes, cut where a character starts.
fn floor(s: &str, most: usize) -> usize {
    let mut i = most.min(s.len());
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

// A link as the panel shows it: where it goes, and the start of the rest.
fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    host.rsplit('@').next().unwrap_or(host).trim_start_matches("www.").to_string()
}

fn shortened(url: &str, most: usize) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r).trim_start_matches("www.");
    if rest.chars().count() <= most {
        return rest.to_string();
    }
    format!("{}…", rest.chars().take(most.saturating_sub(1)).collect::<String>())
}

// --- On the island ----------------------------------------------------------------------

// The ones up, newest last.
struct Notice {
    id: u64,
    found: Found,
    sender: String,
    account: String,
    until: u64,
    copied: bool,
}

static NOTICES: Lock<Vec<Notice>> = Lock::new("mail::codes::NOTICES", Vec::new());
// Apart from the prompts' (agent-core's count from 1).
static NEXT: AtomicU64 = AtomicU64::new(1 << 40);
// Up this long unless answered; "Copied" this long once copied.
const SHOWN_MS: u64 = 60_000;
const COPIED_MS: u64 = 1_500;

fn notices() -> crate::lock::Guard<'static, Vec<Notice>> {
    NOTICES.lock().unwrap_or_else(|e| e.into_inner())
}

// As the prompt panel shows them (panel.ts mailBody), after the prompts.
pub fn views() -> Vec<Value> {
    notices()
        .iter()
        .map(|n| match &n.found {
            Found::Code(code) => json!({ "id": n.id, "kind": "mailCode", "session": "", "sender": n.sender, "account": n.account, "code": code, "copied": n.copied }),
            Found::Link(url) => json!({ "id": n.id, "kind": "mailLink", "session": "", "sender": n.sender, "account": n.account, "url": url, "host": host_of(url), "shown": shortened(url, 46), "copied": n.copied }),
        })
        .collect()
}

pub fn is_empty() -> bool {
    notices().is_empty()
}

pub fn holds(id: u64) -> bool {
    notices().iter().any(|n| n.id == id)
}

// One put up: on the island, with her chime for something waiting.
fn show(sh: &Shared, found: Found, sender: &str, account: &str) {
    let id = NEXT.fetch_add(1, Ordering::SeqCst);
    notices().push(Notice { id, found, sender: sender.to_string(), account: account.to_string(), until: now_ms() + SHOWN_MS, copied: false });
    sh.push_asks();
    let _ = sh.app.emit_to("island", "pet:alert", json!({ "mood": "waiting" }));
}

// Copy ("allow", the panel's first button and the approve key), Open (a
// link) or Ignore ("deny", the deny key). Copied, it stays a moment saying
// so; the clipboard is written here, as the island never has the focus a
// page's own copy would need.
pub fn answer(sh: &Shared, id: u64, choice: &Value) -> bool {
    let action = choice["action"].as_str().unwrap_or("");
    let Some(found) = notices().iter().find(|n| n.id == id).map(|n| n.found.clone()) else { return false };
    let value = match &found {
        Found::Code(v) | Found::Link(v) => v.clone(),
    };
    match action {
        "allow" | "copy" => {
            let copied = clipboard::set(&value);
            if !copied {
                sh.log("mail: the clipboard would not take a code");
            }
            if let Some(n) = notices().iter_mut().find(|n| n.id == id) {
                n.copied = copied;
                n.until = now_ms() + if copied { COPIED_MS } else { SHOWN_MS };
            }
            copied
        }
        "open" if matches!(found, Found::Link(_)) => {
            dismiss(id);
            crate::spawn_named("open link", move || {
                crate::jump::open_url(&value);
            });
            true
        }
        "deny" | "ignore" => dismiss(id),
        _ => false,
    }
}

pub fn dismiss(id: u64) -> bool {
    let mut list = notices();
    let before = list.len();
    list.retain(|n| n.id != id);
    list.len() != before
}

// Those whose minute is up (or whose "Copied" has been read); whether any.
pub fn expire(now: u64) -> bool {
    let mut list = notices();
    let before = list.len();
    list.retain(|n| n.until > now);
    list.len() != before
}

// All taken down (out of sight, asked for quiet); how many.
pub fn clear() -> usize {
    let mut list = notices();
    let n = list.len();
    list.clear();
    n
}

// --- In what comes ----------------------------------------------------------------------

// Got by the server this long ago at most.
const FRESH_S: i64 = 10 * 60;
// Bigger than this, a letter is not read for a code.
const SMALL: u32 = 1 << 20;
// At most this many letters read at one look.
const AT_ONCE: usize = 3;

// The letters come since the newest unread seen (`since`: None before the
// first sync, when nothing is new), newest first, each read once
// (`looked`, kept across connections), for a code or a link.
pub fn look(sh: &Arc<Shared>, conn: &mut Session, st: &Lock<Store>, account: &Account, since: Option<u32>, looked: &mut HashSet<u32>) {
    let Some(since) = since else { return };
    if sh.setting(config::MAIL_CODES) == false {
        return;
    }
    let now_s = now_ms() as i64 / 1000;
    let fresh: Vec<(u32, String, String)> = {
        let s = st.lock().unwrap();
        s.inbox
            .letters
            .range(since.saturating_add(1)..)
            .rev()
            .filter(|(uid, e)| !e.seen && e.size <= SMALL && !looked.contains(uid) && e.received.or(e.date.map(|d| d / 1000)).is_some_and(|t| now_s - t <= FRESH_S))
            .take(AT_ONCE)
            .map(|(uid, e)| (*uid, e.subject.clone(), if e.from.name.trim().is_empty() { e.from.address.clone() } else { e.from.name.trim().to_string() }))
            .collect()
    };
    for (uid, subject, sender) in fresh {
        looked.insert(uid);
        let kept = st.lock().unwrap().body(Folder::Inbox, uid);
        let raw = match kept {
            Some(raw) => raw,
            None => match conn.letter(uid) {
                Ok(Some((raw, _))) => {
                    st.lock().unwrap().keep_body(Folder::Inbox, uid, &raw, false);
                    raw
                }
                Ok(None) => continue,
                // The connection's trouble: the sync's next step meets it.
                Err(_) => return,
            },
        };
        let Some(m) = letters::parse(&raw) else { continue };
        let html = m.body_html(0).unwrap_or_default();
        let Some(found) = find(&subject, &letters::text(&m), &html) else { continue };
        let what = if matches!(found, Found::Code(_)) { "a code" } else { "a link to sign in with" };
        // Nobody to see it: not put up (and not kept for later: it is a
        // minute's thing).
        if !sh.is_visible() {
            sh.log(&format!("mail {}: {what} in letter {uid}, not shown: out of sight", account.id));
            continue;
        }
        sh.log(&format!("mail {}: {what} in letter {uid}", account.id));
        show(sh, found, &sender, &account.address);
    }
}

// --- The clipboard ----------------------------------------------------------------------

#[cfg(windows)]
mod clipboard {
    use std::ffi::c_void;

    #[link(name = "user32")]
    extern "system" {
        fn OpenClipboard(owner: *mut c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn EmptyClipboard() -> i32;
        fn SetClipboardData(format: u32, mem: *mut c_void) -> *mut c_void;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalAlloc(flags: u32, bytes: usize) -> *mut c_void;
        fn GlobalLock(mem: *mut c_void) -> *mut c_void;
        fn GlobalUnlock(mem: *mut c_void) -> i32;
        fn GlobalFree(mem: *mut c_void) -> *mut c_void;
    }
    const CF_UNICODETEXT: u32 = 13;
    const GMEM_MOVEABLE: u32 = 2;

    // The text as the clipboard's; whether it took it. Another program may
    // have it open a moment: tried a few times.
    pub fn set(text: &str) -> bool {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        for _ in 0..10 {
            // SAFETY: the memory is ours until SetClipboardData takes it;
            // freed here whenever it does not.
            unsafe {
                if OpenClipboard(std::ptr::null_mut()) == 0 {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    continue;
                }
                let mem = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2);
                let mut taken = false;
                if !mem.is_null() {
                    let at = GlobalLock(mem) as *mut u16;
                    if !at.is_null() {
                        std::ptr::copy_nonoverlapping(wide.as_ptr(), at, wide.len());
                        GlobalUnlock(mem);
                        EmptyClipboard();
                        taken = !SetClipboardData(CF_UNICODETEXT, mem).is_null();
                    }
                    if !taken {
                        GlobalFree(mem);
                    }
                }
                CloseClipboard();
                return taken;
            }
        }
        false
    }
}

#[cfg(not(windows))]
mod clipboard {
    pub fn set(_text: &str) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(subject: &str, text: &str) -> Option<String> {
        find_code(subject, text)
    }

    #[test]
    fn a_code_after_its_words_in_chinese() {
        assert_eq!(code("", "您的验证码为 123456，5 分钟内有效").as_deref(), Some("123456"));
        assert_eq!(code("", "您的验证码为123456,5分钟内有效").as_deref(), Some("123456"));
        assert_eq!(code("", "校验码：６５４３２１").as_deref(), Some("654321"));
        assert_eq!(code("", "动态密码 123 456，请勿告诉他人").as_deref(), Some("123456"));
        assert_eq!(code("", "【招商银行】您的一次性密码（OTP）是 482910，请于5分钟内输入").as_deref(), Some("482910"));
        // A phone and an address near do not make it something else.
        assert_eq!(code("", "【阿里云】验证码：582910。您正在为账号 123@aliyun.com 绑定手机，如非本人操作请忽略。").as_deref(), Some("582910"));
        // An order's code is still a code when the word is right before it.
        assert_eq!(code("", "订单验证码：7731").as_deref(), Some("7731"));
    }

    #[test]
    fn a_code_in_english() {
        assert_eq!(code("", "Your verification code is A1B2C3.").as_deref(), Some("A1B2C3"));
        assert_eq!(code("", "Your security code: 654321").as_deref(), Some("654321"));
        assert_eq!(code("", "One-time password: 4821").as_deref(), Some("4821"));
        assert_eq!(code("", "Your code is 12 34 56").as_deref(), Some("123456"));
        assert_eq!(code("", "Your security code is 482-910").as_deref(), Some("482910"));
        assert_eq!(code("", "Your Apple ID Code is: 774120. Don't share it.").as_deref(), Some("774120"));
        assert_eq!(code("", "PIN: 4821").as_deref(), Some("4821"));
        // A year right after the word is the code.
        assert_eq!(code("", "Verification code: 2024").as_deref(), Some("2024"));
    }

    #[test]
    fn a_code_before_its_words() {
        assert_eq!(code("", "123456 is your verification code").as_deref(), Some("123456"));
        assert_eq!(code("", "482910 是您的验证码，5分钟内有效。").as_deref(), Some("482910"));
        assert_eq!(code("", "G-123456 is your Google verification code.").as_deref(), Some("123456"));
        assert_eq!(code("123456 is your verification code", "").as_deref(), Some("123456"));
    }

    #[test]
    fn a_code_alone_on_its_line() {
        assert_eq!(code("Your verification code", "123456").as_deref(), Some("123456"));
        assert_eq!(code("Your verification code", "Hi,\n\n849201\n\nThanks.").as_deref(), Some("849201"));
        assert_eq!(code("Your Steam account: Access from new device", "Here is the Steam Guard code you need to login to account mica:\n\n9KJ4X\n\nIf you didn't request this...").as_deref(), Some("9KJ4X"));
        assert_eq!(code("Your Steam account", "The Steam Guard code you need to login to account mica:\n\nKJHXY\n").as_deref(), Some("KJHXY"));
        assert_eq!(code("Slack confirmation code", "Your confirmation code is below — enter it in your open browser window.\n\n**ABC-123**\n").as_deref(), Some("ABC123"));
        // Not alone, nor near the words: not it.
        assert_eq!(code("Your verification code", "Your order 12345678 has shipped"), None);
    }

    #[test]
    fn ordinary_mail_has_no_code() {
        for (subject, text) in [
            ("", "您的订单号 12345678 已发货"),
            ("", "Your order 20261021123456 has shipped"),
            ("", "Call me on 13800138000"),
            ("", "The date is 2026-11-09"),
            ("", "Your invoice total is $1,234.56"),
            ("", "Promo code SAVE20 for your next order"),
            ("", "Your order code is 123456"),
            ("", "Postal code: 123456"),
            ("", "Our code of conduct for 2026 is here"),
            ("", "The verification code era in 2026 is here"),
            ("", "Use referral code 482913"),
            ("", "Enter code MICA10 at checkout to claim the additional 10% discount."),
            ("", "Claude Code hacks: https://magic.beehiiv.com/v1/5f7ce6e3-9a71-416b-99a7"),
            ("", "Your code is at https://example.com/item/482910"),
            // Found in #113's review: words inside other words, a date taken for a code.
            ("Your weekly digest", "Shopping list: pick up 3 apples. Event at 1530 on Friday."),
            ("", "Free shipping within 48 hours on 1000+ items"),
            ("", "Your PIN code was changed at 2026-11-09 08:30. If you did not request this, contact us."),
            ("", "Spinning class starts at 1800, within 10 minutes walk."),
            ("Re: meeting", "Let's discuss the code review in room 4F21 within the hour."),
            ("Your code", "Thanks for buying! Your opinion matters: survey id 77812."),
            ("", "Code: 48 21 93 12 34"),
            ("", "Claude Code 2026 roadmap"),
            ("", "您的快递取件码 6688，请到驿站领取"),
        ] {
            assert_eq!(code(subject, text), None, "{subject} | {text}");
        }
    }

    // A long letter is read quickly: its start alone, each word found once
    // (#113's took 104 s over 48,000 characters of numbers).
    #[test]
    fn a_long_letter_is_read_quickly() {
        let mut big = String::new();
        for i in 0..4000 {
            big.push_str(&format!("Item A{i:04} promo code 12{i} costs {i}0 pts, see deal {i}x9 now. "));
        }
        let t = std::time::Instant::now();
        assert_eq!(code("Your verification code", &big), None);
        assert!(t.elapsed() < std::time::Duration::from_secs(1), "{:?}", t.elapsed());
    }

    #[test]
    fn a_link_to_sign_in_with() {
        let notion = "https://www.notion.so/loginwithemail?token=9f8e7d6c5b4a3210fe87c2b1a0";
        assert_eq!(find_link("Log in to Notion", &format!("Click the link below to sign in:\n{notion}\nThis link will expire in 15 minutes."), "").as_deref(), Some(notion));
        let x = "https://x.com/account/verify_email?token=8f9a0b1c2d3e4f5a6b7c";
        assert_eq!(find_link("Confirm your X account", x, "").as_deref(), Some(x));
        // In HTML, by what the link says, through a click tracker.
        let html = r#"<p>Please confirm:</p><A HREF = "https://u123.ct.sendgrid.net/ls/click?upn=48f7e6d5c4b3a210fedcba98&amp;u=1">Verify <b>Email</b> Address</A><a href="https://example.com/unsubscribe?u=9f8e7d6c5b4a3210fe">Unsubscribe</a>"#;
        assert_eq!(find_link("Please verify your email address", "", html).as_deref(), Some("https://u123.ct.sendgrid.net/ls/click?upn=48f7e6d5c4b3a210fedcba98&u=1"));
    }

    #[test]
    fn links_that_are_not_to_sign_in_with() {
        // Not about signing in.
        assert_eq!(find_link("Weekly digest", "Read: https://news.example.com/login?token=9f8e7d6c5b4a3210fe87", ""), None);
        // Leaving the list, a parcel, a receipt.
        assert_eq!(find_link("Confirm your subscription", "To unsubscribe: https://news.com/unsubscribe?token=9f8e7d6c5b4a3210fe", ""), None);
        assert_eq!(find_link("Confirm your order", "Track your order: https://shop.example.com/track?code=ABCDEFGHIJKLMNOP1234", ""), None);
        // No token: a page anyone gets, not a link made for this sign-in.
        assert_eq!(find_link("Verify your account", "Read https://example.com/verify-help", ""), None);
        assert_eq!(find_link("Sign in", "", r#"<a href="">click</a><img src="https://track.com/open.gif?id=123">"#), None);
    }

    #[test]
    fn a_code_before_a_link() {
        let body = "Your code is 482910. Or click https://example.com/verify?token=9f8e7d6c5b4a3210fe to confirm.";
        assert_eq!(find("Verify your account", body, ""), Some(Found::Code("482910".into())));
    }

    #[test]
    fn what_the_panel_shows_of_a_link() {
        assert_eq!(host_of("https://www.notion.so/login?token=1"), "notion.so");
        assert_eq!(shortened("https://www.notion.so/loginwithemail?token=9f8e7d6c5b4a3210fe87c2b1a0", 20), "notion.so/loginwith…");
    }

    #[test]
    fn notices_go_by_themselves_and_when_answered() {
        clear();
        let id = NEXT.fetch_add(1, Ordering::SeqCst);
        notices().push(Notice { id, found: Found::Code("482910".into()), sender: "GitHub".into(), account: "a@b.test".into(), until: 1000, copied: false });
        assert!(holds(id) && id > 1 << 39);
        assert_eq!(views()[0]["code"], "482910");
        assert_eq!(views()[0]["kind"], "mailCode");
        assert!(!expire(999));
        assert!(expire(1000) && is_empty());
        notices().push(Notice { id, found: Found::Link("https://a.test/verify?token=1".into()), sender: "A".into(), account: "".into(), until: u64::MAX, copied: false });
        assert_eq!(views()[0]["host"], "a.test");
        assert!(dismiss(id) && !dismiss(id));
    }
}
