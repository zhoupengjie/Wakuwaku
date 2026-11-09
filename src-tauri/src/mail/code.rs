// Verification codes and verification links in new letters: the words and
// patterns a service uses around them, and the credential itself.
//
// Features a Two-Tier Verification Engine:
//   1. Intent qualification filter (anti-promo, anti-order, anti-unsubscribe)
//   2. Precision credential extractor with weighted confidence scoring
//
// Both Verification Codes (2-button prompt: 复制 / 忽略) and Verification Links
// (3-button prompt: 复制 / 在浏览器打开 / 忽略) are detected and surfaced
// directly to the Dynamic Island (#panel).
use std::sync::{Arc, Mutex};

use super::imap::Session;
use super::letters;
use super::store::{Folder, Store};
use crate::Shared;

// How many of the newest letters one look reads whole.
const MOST: usize = 5;

// Code length constraints: 4 to 8 characters.
const SHORTEST: usize = 4;
const LONGEST: usize = 8;
const AFTER: usize = 80;
const BEFORE: usize = 40;
const ENOUGH: i32 = 65;

const KEYWORDS: &[(&str, i32)] = &[
    ("短信验证码", 110),
    ("手机验证码", 110),
    ("邮箱验证码", 110),
    ("验证码", 100),
    ("校验码", 100),
    ("安全验证码", 105),
    ("身份验证码", 105),
    ("动态密码", 100),
    ("动态码", 95),
    ("一次性密码", 105),
    ("账号验证码", 105),
    ("确认码", 90),
    ("安全码", 90),
    ("授权码", 75),
    ("登录码", 85),
    ("verification code", 100),
    ("your code is", 95),
    ("your code", 90),
    ("one-time password", 105),
    ("one time password", 105),
    ("one-time code", 100),
    ("one time code", 100),
    ("security code", 95),
    ("confirmation code", 95),
    ("authentication code", 95),
    ("auth code", 90),
    ("steam guard code", 105),
    ("steam guard", 100),
    ("guard code", 95),
    ("access code", 90),
    ("login code", 90),
    ("verify code", 95),
    ("passcode", 90),
    ("otp", 85),
    ("pin code", 80),
    ("pin", 75),
    ("code", 40),
];

const NOT_A_CODE: &[&str] = &[
    "order", "invoice", "telephone", "fax", "zip", "postal", "tracking",
    "coupon", "promo", "discount", "price", "amount", "total", "balance", "cvv", "cvc",
    "referral", "invite", "invitation", "voucher", "gift", "qr", "area code", "code of conduct",
    "订单", "单号", "编号", "电话", "手机号", "传真", "邮编", "快递", "物流", "运单", "优惠",
    "折扣", "价格", "金额", "余额", "发票", "卡号", "账号：", "账号:", "座位", "航班", "$", "¥", "￥",
    "usd", "cny", "checkout", "redeem", "claim", "offer", "bonus", "%", "off",
];

const EXPIRATION_SIGNALS: &[&str] = &[
    "有效时间", "分钟内有效", "小时内有效", "有效期", "请在", "内完成", "如非本人操作",
    "valid for", "expires in", "expire in", "within", "minutes", "hours", "if you did not request",
];

const LINK_VETO_WORDS: &[&str] = &[
    "unsubscribe", "optout", "opt-out", "preferences", "mail-preferences", "manage-notifications",
    "退订", "取消订阅", "偏好设置", "privacy", "terms", "policy", "legal", "compliance",
    "服务条款", "隐私政策", "帮助中心", "常见问题", "support.", "help.", "contact-us", "about-us",
    "play.google.com", "apps.apple.com", "app store", "google play",
    ".png", ".jpg", ".jpeg", ".gif", ".svg", ".webp", ".ico", ".pdf", ".css", ".js", "open.track", "pixel.gif",
];

const LINK_AUTH_PATH_TOKENS: &[&str] = &[
    "verify", "verification", "confirm", "confirmation", "activate", "activation",
    "magic-link", "magic", "login/confirm", "signin/confirm", "auth/callback",
    "reset-password", "reset_password", "password/reset", "device/verify",
];

const LINK_AUTH_QUERY_KEYS: &[&str] = &[
    "token=", "code=", "key=", "ticket=", "auth=", "sig=", "hash=", "magic=", "secret=",
];

const LINK_ACTION_PHRASES: &[&str] = &[
    "verify email", "verify your email", "confirm your email", "confirm email", "confirm account",
    "activate account", "activate your account", "log in to", "sign in to", "magic link",
    "reset password", "reset your password", "authorize device", "click here to verify",
    "验证邮箱", "确认邮箱", "激活账号", "激活您的账号", "点击验证", "立即登录", "一键登录",
    "确认注册", "重置密码", "完成注册", "授权此设备",
];

#[derive(Clone, Debug)]
struct Candidate {
    text: String,
    at: usize,
    end: usize,
    digits: usize,
    letters: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MailNotice {
    Code(String),
    Link { url: String, display_url: String },
}

// Full-width to half-width digit conversion (０-９ -> 0-9).
fn normalize_digits(s: &str) -> String {
    s.chars()
        .map(|c| {
            let u = c as u32;
            if (0xFF10..=0xFF19).contains(&u) {
                char::from_u32(u - 0xFF10 + 0x30).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

pub fn find_code(subject: &str, body: &str) -> Option<String> {
    let norm_sub = normalize_digits(subject);
    let norm_body = normalize_digits(body);

    if let Some(code) = detect_code(&norm_sub, true) {
        return Some(code);
    }
    if let Some(code) = detect_code(&norm_body, false) {
        return Some(code);
    }
    if norm_sub.trim().is_empty() {
        return None;
    }
    detect_code_with(&format!("{}\n{}", norm_sub.trim(), norm_body.trim()), false, 35)
}

fn detect_code(raw: &str, is_subject: bool) -> Option<String> {
    detect_code_with(raw, is_subject, if is_subject { AFTER } else { 60 })
}

fn detect_code_with(raw: &str, _is_subject: bool, after_max: usize) -> Option<String> {
    let chars: Vec<char> = raw.chars().collect();
    let candidates = find_candidates(&chars);
    let mut best: Option<String> = None;
    let mut best_score = -1i32;

    for c in &candidates {
        if beside_address(&chars, c) || in_link(&chars, c) || is_year(c) {
            continue;
        }

        let mut score = -1i32;
        let mut min_kw_dist = usize::MAX;
        let mut best_kw_weight = 0i32;

        for &(kw, kw_score) in KEYWORDS {
            let Some(dist) = distance(&chars, c, kw) else { continue };

            if dist >= 0 && (dist as usize) <= after_max {
                let d = dist as usize;
                if d < min_kw_dist {
                    min_kw_dist = d;
                    best_kw_weight = kw_score;
                }
                let s = kw_score as f32 + ((after_max - d) as f32) / 4.0;
                let s = s.round() as i32;
                if s > score {
                    score = s;
                }
            } else if dist < 0 && dist.unsigned_abs() as usize <= BEFORE {
                let d = dist.unsigned_abs() as usize;
                if d < min_kw_dist {
                    min_kw_dist = d;
                    best_kw_weight = kw_score;
                }
                let s = (kw_score as f32) * 0.9 + ((BEFORE as f32) + (dist as f32)) / 4.0;
                let s = s.round() as i32;
                if s > score {
                    score = s;
                }
            }
        }

        if score < 50 {
            continue;
        }

        score += length_bonus(c);

        if has_expiration_nearby(&chars, c) {
            score += 25;
        }
        if has_bracket_enclosure(&chars, c) {
            score += 15;
        }

        let mut penalty = not_a_code_penalty(&chars, c);
        // Proximity protection: if an explicit verification keyword sits right next
        // to the candidate (<= 10 chars, e.g. 验证码：123456), suppress generic penalties.
        if min_kw_dist <= 10 && best_kw_weight >= 95 {
            penalty = penalty.min(15);
        }
        score -= penalty;

        if score >= ENOUGH && score > best_score {
            best_score = score;
            best = Some(c.text.clone());
        }
    }
    best
}

fn find_candidates(chars: &[char]) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    let is_digit = |c: char| c.is_ascii_digit();
    let mut i = 0;
    while i < chars.len() {
        if is_candidate_char(chars[i]) {
            let start = i;
            let mut digits = 0usize;
            let mut letters = 0usize;
            let mut text = String::new();
            let mut end = i;

            while end < chars.len() && is_candidate_char(chars[end]) {
                let ch = chars[end];
                if ch.is_ascii_digit() {
                    digits += 1;
                } else if ch.is_ascii_alphabetic() {
                    letters += 1;
                }
                text.push(ch);
                end += 1;

                // Handle space-separated or hyphen-separated digits (e.g. "123 456" or "482-910")
                // Requires both preceding and succeeding character to be digits.
                if end < chars.len()
                    && (chars[end] == ' ' || chars[end] == '-')
                    && end > 0
                    && is_digit(chars[end - 1])
                    && end + 1 < chars.len()
                    && is_digit(chars[end + 1])
                {
                    if digits < 8 {
                        end += 1;
                    }
                }
            }

            let clean_text: String = text.chars().filter(|c| !c.is_whitespace() && *c != '-').collect();
            let len = clean_text.len();
            if (SHORTEST..=LONGEST).contains(&len) && digits >= 1 {
                candidates.push(Candidate {
                    text: clean_text,
                    at: start,
                    end,
                    digits,
                    letters,
                });
            }
            i = end;
        } else {
            i += 1;
        }
    }
    candidates
}

fn is_candidate_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
}

fn beside_address(chars: &[char], c: &Candidate) -> bool {
    (c.at > 0 && chars[c.at - 1] == '@') || (c.end < chars.len() && chars[c.end] == '@')
}

fn in_link(chars: &[char], c: &Candidate) -> bool {
    let edge = |ch: char| ch.is_whitespace() || matches!(ch, '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | '"' | '\'' | '，' | '。' | '、');
    let mut from = c.at;
    while from > 0 && !edge(chars[from - 1]) {
        from -= 1;
    }
    let mut to = c.end;
    while to < chars.len() && !edge(chars[to]) {
        to += 1;
    }
    let token: String = chars[from..to].iter().collect::<String>().to_lowercase();
    token.contains("://")
        || token.starts_with("www.")
        || (token.contains('/') && token.contains('.'))
        || (token.contains('?') && token.contains('='))
        || [".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg", ".html", ".php"].iter().any(|ext| token.contains(ext))
}

fn is_year(c: &Candidate) -> bool {
    c.letters == 0
        && c.digits == 4
        && c.text.parse::<i32>().is_ok_and(|year| (1900..=2035).contains(&year))
}

fn length_bonus(c: &Candidate) -> i32 {
    match c.digits + c.letters {
        6 => 26,
        5 => 22,
        7 => 24,
        4 => 16,
        8 => 20,
        _ => 0,
    }
}

fn distance(chars: &[char], c: &Candidate, kw: &str) -> Option<i32> {
    let kw_chars: Vec<char> = kw.chars().collect();
    if kw_chars.is_empty() || chars.len() < kw_chars.len() {
        return None;
    }
    let mut nearest: Option<i32> = None;
    for i in 0..=(chars.len() - kw_chars.len()) {
        let matches = chars[i..i + kw_chars.len()]
            .iter()
            .zip(kw_chars.iter())
            .all(|(&a, &b)| a.to_lowercase().eq(b.to_lowercase()));
        if matches {
            let found = i;
            let kw_end = i + kw_chars.len();
            let dist = if found >= c.end {
                -((found - c.end) as i32)
            } else if c.at >= kw_end {
                (c.at - kw_end) as i32
            } else {
                0
            };
            if nearest.is_none() || dist.abs() < nearest.unwrap().abs() {
                nearest = Some(dist);
            }
        }
    }
    nearest
}

fn has_expiration_nearby(chars: &[char], c: &Candidate) -> bool {
    let from = c.at.saturating_sub(60);
    let to = (c.end + 60).min(chars.len());
    let snippet: String = chars[from..to].iter().collect::<String>().to_lowercase();
    EXPIRATION_SIGNALS.iter().any(|s| snippet.contains(s))
}

fn has_bracket_enclosure(chars: &[char], c: &Candidate) -> bool {
    if c.at > 0 && c.end < chars.len() {
        let prev = chars[c.at - 1];
        let next = chars[c.end];
        return matches!((prev, next), ('【', '】') | ('[', ']') | ('"', '"') | ('“', '”') | ('`', '`'));
    }
    false
}

fn not_a_code_penalty(chars: &[char], c: &Candidate) -> i32 {
    let from = c.at.saturating_sub(30);
    let to = (c.end + 30).min(chars.len());
    let around: String = chars[from..to].iter().collect::<String>().to_lowercase();
    let mut penalty = 0;
    for word in NOT_A_CODE {
        if around.contains(word) {
            penalty += 60;
        }
    }
    let near_from = c.at.saturating_sub(12);
    let near_to = (c.end + 12).min(chars.len());
    let near_around: String = chars[near_from..near_to].iter().collect::<String>().to_lowercase();
    for veto in &["order", "invoice", "tracking", "postal", "订单", "单号", "运单", "快递", "邮编"] {
        if near_around.contains(veto) {
            penalty += 60;
        }
    }
    if around.contains("http://") || around.contains("https://") || around.contains("www.") {
        penalty += 35;
    }
    penalty.min(150)
}

// -------------------------------------------------------------
// VERIFICATION LINK ENGINE
// -------------------------------------------------------------

#[derive(Debug)]
struct LinkCandidate {
    url: String,
    anchor: String,
    context: String,
}

pub fn truncate_display_url(url: &str, max_len: usize) -> String {
    let cleaned = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.");
    let chars: Vec<char> = cleaned.chars().collect();
    if chars.len() <= max_len {
        cleaned.to_string()
    } else {
        let cut = max_len.saturating_sub(3);
        let prefix: String = chars[..cut].iter().collect();
        format!("{prefix}...")
    }
}

pub fn find_link(subject: &str, body: &str, html: Option<&str>) -> Option<String> {
    let candidates = extract_link_candidates(body, html);
    let mut best: Option<String> = None;
    let mut best_score = -1i32;

    for c in &candidates {
        let url_lower = c.url.to_lowercase();
        let anchor_lower = c.anchor.to_lowercase();

        let has_auth_marker = LINK_AUTH_PATH_TOKENS.iter().any(|t| url_lower.contains(t))
            || LINK_AUTH_QUERY_KEYS.iter().any(|k| url_lower.contains(k));

        if !has_auth_marker && LINK_VETO_WORDS.iter().any(|v| url_lower.contains(v) || anchor_lower.contains(v)) {
            continue;
        }

        let mut score = 0i32;

        for &token in LINK_AUTH_PATH_TOKENS {
            if url_lower.contains(token) {
                score += 45;
            }
        }
        for &qk in LINK_AUTH_QUERY_KEYS {
            if url_lower.contains(qk) {
                score += 35;
            }
        }

        if has_high_entropy_token(&c.url) {
            score += 30;
        }

        for &phrase in LINK_ACTION_PHRASES {
            if anchor_lower.contains(phrase) {
                score += 50;
            }
        }

        let ctx_lower = c.context.to_lowercase();
        if EXPIRATION_SIGNALS.iter().any(|s| ctx_lower.contains(s)) {
            score += 25;
        }
        for &phrase in LINK_ACTION_PHRASES {
            if ctx_lower.contains(phrase) {
                score += 30;
            }
        }

        let sub_lower = subject.to_lowercase();
        for &phrase in LINK_ACTION_PHRASES {
            if sub_lower.contains(phrase) {
                score += 25;
            }
        }
        if sub_lower.contains("verify") || sub_lower.contains("verification") || sub_lower.contains("验证") || sub_lower.contains("激活") || sub_lower.contains("magic") {
            score += 20;
        }

        if score >= 70 && score > best_score {
            best_score = score;
            best = Some(c.url.clone());
        }
    }

    best
}

fn has_high_entropy_token(url: &str) -> bool {
    for prefix in &["token=", "code=", "key=", "ticket=", "sig=", "hash="] {
        if let Some(pos) = url.find(prefix) {
            let start = pos + prefix.len();
            let end = url[start..].find('&').map(|e| start + e).unwrap_or(url.len());
            let val = &url[start..end];
            if val.len() >= 16 && val.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                return true;
            }
        }
    }
    false
}

fn find_next_a_tag(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i + 2 < len {
        if bytes[i] == b'<' && (bytes[i + 1] == b'a' || bytes[i + 1] == b'A') {
            let next_byte = bytes[i + 2];
            if next_byte.is_ascii_whitespace() || next_byte == b'>' {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

fn find_close_a_tag(s: &str) -> Option<(usize, usize)> {
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i + 3 < len {
        if bytes[i] == b'<' && bytes[i + 1] == b'/' && (bytes[i + 2] == b'a' || bytes[i + 2] == b'A') {
            let next_byte = bytes[i + 3];
            if next_byte.is_ascii_whitespace() || next_byte == b'>' {
                if let Some(gt_rel) = s[i + 3..].find('>') {
                    let full_end = i + 3 + gt_rel + 1;
                    return Some((i, full_end));
                }
            }
        }
        i += 1;
    }
    None
}

fn find_href_value(tag_head: &str) -> Option<&str> {
    let lower = tag_head.to_ascii_lowercase();
    let mut search_from = 0;
    while let Some(pos) = lower[search_from..].find("href") {
        let at = search_from + pos;
        let before_ok = at == 0 || tag_head.as_bytes()[at - 1].is_ascii_whitespace();
        let after_href = &tag_head[at + 4..];
        let trimmed_after = after_href.trim_start();
        if before_ok && trimmed_after.starts_with('=') {
            let after_eq = trimmed_after[1..].trim_start();
            let quote = after_eq.chars().next().unwrap_or(' ');
            let raw_url = if quote == '"' || quote == '\'' {
                let inner = &after_eq[quote.len_utf8()..];
                let end_q = inner.find(quote).unwrap_or(inner.len());
                &inner[..end_q]
            } else {
                let end_w = after_eq.find(|c: char| c.is_whitespace() || c == '>').unwrap_or(after_eq.len());
                &after_eq[..end_w]
            };
            return Some(raw_url);
        }
        search_from = at + 4;
    }
    None
}

fn extract_link_candidates(body: &str, html: Option<&str>) -> Vec<LinkCandidate> {
    let mut candidates = Vec::new();
    let mut seen = std::collections::HashSet::new();

    if let Some(h) = html {
        let mut rest = h;
        while let Some(tag_start) = find_next_a_tag(rest) {
            let tag_content = &rest[tag_start..];
            let Some(tag_end) = tag_content.find('>') else { break };
            let tag_head = &tag_content[..tag_end];
            let after_head = &tag_content[tag_end + 1..];
            let Some((close_start, close_end)) = find_close_a_tag(after_head) else { break };
            let inner_html = &after_head[..close_start];

            if let Some(raw_url) = find_href_value(tag_head) {
                let clean_url = raw_url.trim().replace("&amp;", "&");
                if (clean_url.starts_with("http://") || clean_url.starts_with("https://")) && seen.insert(clean_url.clone()) {
                    let clean_anchor: String = inner_html.chars().filter(|&c| c != '<' && c != '>').collect();
                    let abs_tag_start = h.len() - rest.len() + tag_start;
                    let mut start_ctx = abs_tag_start.saturating_sub(100);
                    while start_ctx > 0 && !h.is_char_boundary(start_ctx) {
                        start_ctx -= 1;
                    }
                    let abs_tag_end = h.len() - rest.len() + tag_start + tag_end + 1 + close_end;
                    let mut end_ctx = (abs_tag_end + 100).min(h.len());
                    while end_ctx < h.len() && !h.is_char_boundary(end_ctx) {
                        end_ctx += 1;
                    }
                    let context: String = h[start_ctx..end_ctx].chars().filter(|&c| c != '<' && c != '>').collect();
                    candidates.push(LinkCandidate {
                        url: clean_url,
                        anchor: clean_anchor.trim().to_string(),
                        context,
                    });
                }
            }
            rest = &after_head[close_end..];
        }
    }

    if !body.is_empty() {
        for word in body.split_whitespace() {
            let clean = word.trim_matches(|c: char| matches!(c, '<' | '>' | '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | '，' | '。' | '、' | '.' | ',' | ';' | '!'));
            let clean_url = clean.replace("&amp;", "&");
            if (clean_url.starts_with("http://") || clean_url.starts_with("https://")) && seen.insert(clean_url.clone()) {
                let context = if let Some(pos) = body.find(clean) {
                    let mut start = pos.saturating_sub(80);
                    while start > 0 && !body.is_char_boundary(start) {
                        start -= 1;
                    }
                    let mut end = (pos + clean.len() + 80).min(body.len());
                    while end < body.len() && !body.is_char_boundary(end) {
                        end += 1;
                    }
                    body[start..end].to_string()
                } else {
                    String::new()
                };
                candidates.push(LinkCandidate {
                    url: clean_url,
                    anchor: String::new(),
                    context,
                });
            }
        }
    }

    candidates
}

pub fn detect_mail_notice(subject: &str, body: &str, html: Option<&str>) -> Option<MailNotice> {
    if let Some(code) = find_code(subject, body) {
        return Some(MailNotice::Code(code));
    }
    if let Some(link) = find_link(subject, body, html) {
        let display = truncate_display_url(&link, 38);
        return Some(MailNotice::Link { url: link, display_url: display });
    }
    None
}

// Read the newest letters and notice the verification codes / links among them.
pub fn new_mail(sh: &Arc<Shared>, conn: &mut Session, st: &Mutex<Store>, uids: &[u32], since: u32) {
    for &uid in uids.iter().filter(|uid| **uid > since).take(MOST) {
        let entry = st.lock().unwrap().inbox.letters.get(&uid).cloned();
        let Some(entry) = entry else { continue };
        if entry.seen || entry.from.address.trim().is_empty() {
            continue;
        }

        let kept = st.lock().unwrap().body(Folder::Inbox, uid);
        let raw = match kept {
            Some(raw) => raw,
            None => match conn.letter(uid) {
                Ok(Some((raw, _))) => {
                    st.lock().unwrap().keep_body(Folder::Inbox, uid, &raw, false);
                    raw
                }
                Ok(None) => continue,
                Err(_) => break,
            },
        };

        let Some(message) = letters::parse(&raw) else { continue };
        let body = letters::text(&message);
        let html_body = letters::html(&message);

        if let Some(notice) = detect_mail_notice(&entry.subject, &body, Some(&html_body)) {
            let sender = if !entry.from.name.trim().is_empty() {
                entry.from.name.trim().to_string()
            } else {
                entry.from.address.split('@').nth(1).unwrap_or(&entry.from.address).to_string()
            };
            match notice {
                MailNotice::Code(c) => {
                    sh.push_mail_notice("mail_code", &sender, &c, None, &entry.subject);
                    sh.log(&format!("mail: code ({c}) in letter {uid}"));
                }
                MailNotice::Link { url, display_url } => {
                    sh.push_mail_notice("mail_link", &sender, &url, Some(&display_url), &entry.subject);
                    sh.log(&format!("mail: link ({url}) in letter {uid}"));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chinese_code_is_the_number_after_the_words() {
        assert_eq!(find_code("", "您的验证码为 123456，5 分钟内有效").as_deref(), Some("123456"));
        assert_eq!(find_code("", "校验码：６５４３２１").as_deref(), Some("654321"));
        assert_eq!(find_code("", "动态密码 123 456，请勿告诉他人").as_deref(), Some("123456"));
    }

    #[test]
    fn chinese_cloud_account_verification_email_not_falsely_rejected() {
        assert_eq!(
            find_code("", "【阿里云】验证码：582910。您正在为账号 123@aliyun.com 绑定手机，如非本人操作请忽略。").as_deref(),
            Some("582910")
        );
    }

    #[test]
    fn an_english_code_may_be_letters_and_digits() {
        assert_eq!(find_code("", "Your verification code is A1B2C3.").as_deref(), Some("A1B2C3"));
        assert_eq!(find_code("", "Your security code: 654321").as_deref(), Some("654321"));
        assert_eq!(find_code("", "One-time password: 4821").as_deref(), Some("4821"));
    }

    #[test]
    fn hyphenated_and_spaced_codes_are_handled() {
        assert_eq!(find_code("", "Your code is 12 34 56").as_deref(), Some("123456"));
        assert_eq!(find_code("", "Your security code is 482-910").as_deref(), Some("482910"));
    }

    #[test]
    fn a_code_before_the_words_is_still_a_code() {
        assert_eq!(find_code("", "123456 is your verification code").as_deref(), Some("123456"));
        assert_eq!(find_code("123456 is your verification code", "").as_deref(), Some("123456"));
        assert_eq!(find_code("Your verification code", "123456").as_deref(), Some("123456"));
        assert_eq!(find_code("Your verification code", "Hi,\n\n123456").as_deref(), Some("123456"));
        assert_eq!(find_code("Your verification code", "Your order 12345678 has shipped"), None);
    }

    #[test]
    fn steam_guard_code_is_detected() {
        assert_eq!(find_code("Your Steam Guard Code", "Hello,\n\n9KJ4X\n\nIf you didn't request this...").as_deref(), Some("9KJ4X"));
    }

    #[test]
    fn ordinary_mail_has_no_code() {
        assert_eq!(find_code("", "您的订单号 12345678 已发货"), None);
        assert_eq!(find_code("", "Your order 20261021123456 has shipped"), None);
        assert_eq!(find_code("", "Call me on 13800138000"), None);
        assert_eq!(find_code("", "The date is 2026-10-21"), None);
        assert_eq!(find_code("", "Your invoice total is $1,234.56"), None);
    }

    #[test]
    fn offers_and_order_codes_are_not_verification_codes() {
        assert_eq!(find_code("", "Promo code SAVE20 for your next order"), None);
        assert_eq!(find_code("", "Your order code is 123456"), None);
        assert_eq!(find_code("", "Postal code: 123456"), None);
        assert_eq!(find_code("", "Our code of conduct for 2026 is here"), None);
        assert_eq!(find_code("", "The verification code era in 2026 is here"), None);
        assert_eq!(find_code("", "Use referral code 482913"), None);
        assert_eq!(find_code("", "Enter code MICA10 at checkout to claim the additional 10% discount."), None);
        assert_eq!(find_code("", "Claude Code hacks: https://magic.beehiiv.com/v1/5f7ce6e3-9a71-416b-99a7"), None);
    }

    #[test]
    fn verification_link_detection() {
        let sub = "Log in to Notion";
        let body = "Click the link below to sign in:\nhttps://www.notion.so/login/confirm?token=9f8e7d6c5b4a3210fe87c2b1a0\nThis link will expire in 15 minutes.";
        assert_eq!(find_link(sub, body, None).as_deref(), Some("https://www.notion.so/login/confirm?token=9f8e7d6c5b4a3210fe87c2b1a0"));

        let twitter_verify = "https://x.com/account/verify_email?token=8f9a0b1c2d3e4f5a6b7c";
        assert_eq!(find_link("Confirm your X account", twitter_verify, None).as_deref(), Some(twitter_verify));

        let unsub = "Weekly Digest\nRead stories at https://news.com\nTo unsubscribe: https://news.com/unsubscribe?u=12";
        assert_eq!(find_link("Weekly Digest", unsub, None), None);
    }

    #[test]
    fn disambiguation_code_prioritized() {
        let sub = "Verify your account";
        let body = "Your code is 482910. Or click https://example.com/verify?token=9f8e7d6c5b4a3210fe to confirm.";
        assert_eq!(detect_mail_notice(sub, body, None), Some(MailNotice::Code("482910".to_string())));
    }
}
